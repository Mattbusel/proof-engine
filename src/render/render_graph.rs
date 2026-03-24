//! Render Graph — declarative, dependency-ordered GPU pass scheduling.
//!
//! The render graph describes the full frame as a directed acyclic graph (DAG)
//! of render passes, each consuming and producing named resources (textures,
//! render targets, buffers). The graph compiler topologically sorts passes,
//! deduces resource lifetimes, inserts memory barriers, and drives execution.
//!
//! ## Key Types
//!
//! - [`RenderGraph`]      — builder: declare passes and resources
//! - [`CompiledGraph`]    — sorted, barrier-annotated execution plan
//! - [`PassBuilder`]      — fluent API for declaring a single pass
//! - [`ResourceDesc`]     — describes a texture or buffer resource
//! - [`RenderPass`]       — one node in the graph
//! - [`PassKind`]         — Graphics, Compute, or Transfer
//! - [`ResourceAccess`]   — read / write access mode
//! - [`Barrier`]          — memory / layout transition between passes

use std::collections::{HashMap, HashSet, VecDeque};
use crate::render::compute::ResourceHandle;

// ── ResourceDesc ──────────────────────────────────────────────────────────────

/// Format of a texture resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgba16Float,
    Rgba32Float,
    R32Float,
    Rg16Float,
    Depth24Stencil8,
    Depth32Float,
    Rgb10A2Unorm,
    Bgra8Unorm,
}

impl TextureFormat {
    pub fn is_depth(self) -> bool {
        matches!(self, Self::Depth24Stencil8 | Self::Depth32Float)
    }

    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Rgba8Unorm | Self::Bgra8Unorm | Self::Rgb10A2Unorm | Self::Depth24Stencil8 => 4,
            Self::R32Float => 4,
            Self::Rg16Float => 4,
            Self::Rgba16Float | Self::Depth32Float => 8,
            Self::Rgba32Float => 16,
        }
    }
}

/// Size of a render graph resource, possibly relative to the output framebuffer.
#[derive(Debug, Clone, Copy)]
pub enum ResourceSize {
    /// Absolute pixel dimensions.
    Fixed(u32, u32),
    /// Scale of the output framebuffer (e.g. 0.5 = half res).
    Relative(f32),
    /// Same as the output framebuffer.
    Backbuffer,
}

/// Description of a render graph texture resource.
#[derive(Debug, Clone)]
pub struct ResourceDesc {
    pub name:    String,
    pub format:  TextureFormat,
    pub size:    ResourceSize,
    pub samples: u32,
    /// Mip levels (1 = no mipmapping).
    pub mips:    u32,
    /// Number of array layers.
    pub layers:  u32,
    /// Persistent: survives across frames (e.g. temporal AA history).
    pub persistent: bool,
}

impl ResourceDesc {
    pub fn color(name: impl Into<String>, format: TextureFormat) -> Self {
        Self {
            name:    name.into(),
            format,
            size:    ResourceSize::Backbuffer,
            samples: 1,
            mips:    1,
            layers:  1,
            persistent: false,
        }
    }

    pub fn depth(name: impl Into<String>) -> Self {
        Self::color(name, TextureFormat::Depth24Stencil8)
    }

    pub fn half_res(mut self) -> Self {
        self.size = ResourceSize::Relative(0.5);
        self
    }

    pub fn fixed_size(mut self, w: u32, h: u32) -> Self {
        self.size = ResourceSize::Fixed(w, h);
        self
    }

    pub fn with_mips(mut self, mips: u32) -> Self {
        self.mips = mips;
        self
    }

    pub fn persistent(mut self) -> Self {
        self.persistent = true;
        self
    }

    pub fn msaa(mut self, samples: u32) -> Self {
        self.samples = samples;
        self
    }
}

// ── ResourceAccess ────────────────────────────────────────────────────────────

/// How a pass accesses a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceAccess {
    /// Read as a sampler / shader resource.
    ShaderRead,
    /// Written as a render target color attachment.
    RenderTarget,
    /// Written as depth/stencil attachment.
    DepthWrite,
    /// Read as depth attachment (no writes).
    DepthRead,
    /// Read/write via image load/store (compute).
    ImageReadWrite,
    /// Written by compute shader.
    ComputeWrite,
    /// Read by compute shader.
    ComputeRead,
    /// Source of a transfer/blit operation.
    TransferSrc,
    /// Destination of a transfer/blit operation.
    TransferDst,
    /// Presented to the display.
    Present,
}

impl ResourceAccess {
    pub fn is_write(self) -> bool {
        matches!(self,
            Self::RenderTarget
            | Self::DepthWrite
            | Self::ImageReadWrite
            | Self::ComputeWrite
            | Self::TransferDst
        )
    }

    pub fn is_read(self) -> bool { !self.is_write() }
}

// ── Barrier ───────────────────────────────────────────────────────────────────

/// A memory / image-layout barrier between two passes.
#[derive(Debug, Clone)]
pub struct Barrier {
    pub resource:   String,
    pub src_access: ResourceAccess,
    pub dst_access: ResourceAccess,
}

impl Barrier {
    pub fn new(resource: impl Into<String>, src: ResourceAccess, dst: ResourceAccess) -> Self {
        Self { resource: resource.into(), src_access: src, dst_access: dst }
    }
}

// ── PassKind ──────────────────────────────────────────────────────────────────

/// What kind of work a render pass performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassKind {
    /// Rasterization pass (vertex + fragment shaders).
    Graphics,
    /// GPU compute dispatch.
    Compute,
    /// CPU→GPU or GPU→GPU data transfer / blit.
    Transfer,
    /// Presentation to the display.
    Present,
}

// ── RenderPass ────────────────────────────────────────────────────────────────

/// A single node in the render graph.
#[derive(Debug, Clone)]
pub struct RenderPass {
    pub name:     String,
    pub kind:     PassKind,
    /// Resources read by this pass and the access type.
    pub reads:    Vec<(String, ResourceAccess)>,
    /// Resources written by this pass and the access type.
    pub writes:   Vec<(String, ResourceAccess)>,
    /// User-assigned render priority (lower = earlier among peers).
    pub priority: i32,
    /// Whether this pass can be skipped if its outputs are unused.
    pub optional: bool,
    /// Explicit extra dependencies not implied by resource usage.
    pub depends:  Vec<String>,
}

impl RenderPass {
    pub fn new(name: impl Into<String>, kind: PassKind) -> Self {
        Self {
            name:     name.into(),
            kind,
            reads:    Vec::new(),
            writes:   Vec::new(),
            priority: 0,
            optional: false,
            depends:  Vec::new(),
        }
    }

    pub fn reads(&mut self, res: impl Into<String>, access: ResourceAccess) {
        self.reads.push((res.into(), access));
    }

    pub fn writes(&mut self, res: impl Into<String>, access: ResourceAccess) {
        self.writes.push((res.into(), access));
    }
}

// ── PassBuilder ───────────────────────────────────────────────────────────────

/// Fluent builder for a single render pass.
pub struct PassBuilder<'g> {
    graph: &'g mut RenderGraph,
    pass:  RenderPass,
}

impl<'g> PassBuilder<'g> {
    fn new(graph: &'g mut RenderGraph, name: impl Into<String>, kind: PassKind) -> Self {
        Self { graph, pass: RenderPass::new(name, kind) }
    }

    pub fn read(mut self, resource: impl Into<String>) -> Self {
        self.pass.reads(resource, ResourceAccess::ShaderRead);
        self
    }

    pub fn read_depth(mut self, resource: impl Into<String>) -> Self {
        self.pass.reads(resource, ResourceAccess::DepthRead);
        self
    }

    pub fn write(mut self, resource: impl Into<String>) -> Self {
        self.pass.writes(resource, ResourceAccess::RenderTarget);
        self
    }

    pub fn write_depth(mut self, resource: impl Into<String>) -> Self {
        self.pass.writes(resource, ResourceAccess::DepthWrite);
        self
    }

    pub fn compute_read(mut self, resource: impl Into<String>) -> Self {
        self.pass.reads(resource, ResourceAccess::ComputeRead);
        self
    }

    pub fn compute_write(mut self, resource: impl Into<String>) -> Self {
        self.pass.writes(resource, ResourceAccess::ComputeWrite);
        self
    }

    pub fn transfer_src(mut self, resource: impl Into<String>) -> Self {
        self.pass.reads(resource, ResourceAccess::TransferSrc);
        self
    }

    pub fn transfer_dst(mut self, resource: impl Into<String>) -> Self {
        self.pass.writes(resource, ResourceAccess::TransferDst);
        self
    }

    pub fn priority(mut self, p: i32) -> Self {
        self.pass.priority = p;
        self
    }

    pub fn optional(mut self) -> Self {
        self.pass.optional = true;
        self
    }

    pub fn after(mut self, pass_name: impl Into<String>) -> Self {
        self.pass.depends.push(pass_name.into());
        self
    }

    /// Finalize and register the pass with the graph.
    pub fn build(self) {
        self.graph.add_pass(self.pass);
    }
}

// ── RenderGraph ───────────────────────────────────────────────────────────────

/// Builder for a frame's render graph.
pub struct RenderGraph {
    passes:    Vec<RenderPass>,
    resources: HashMap<String, ResourceDesc>,
    /// The final output resource (usually the backbuffer).
    output:    Option<String>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            passes:    Vec::new(),
            resources: HashMap::new(),
            output:    None,
        }
    }

    /// Declare a transient resource (created and destroyed within this frame).
    pub fn declare_resource(&mut self, desc: ResourceDesc) -> ResourceHandle {
        let id = self.resources.len() as u32 + 1;
        self.resources.insert(desc.name.clone(), desc);
        ResourceHandle(id)
    }

    /// Declare the final output resource.
    pub fn set_output(&mut self, resource: impl Into<String>) {
        self.output = Some(resource.into());
    }

    /// Add a pre-built pass.
    pub fn add_pass(&mut self, pass: RenderPass) {
        self.passes.push(pass);
    }

    /// Start building a graphics pass.
    pub fn graphics_pass<'g>(&'g mut self, name: impl Into<String>) -> PassBuilder<'g> {
        PassBuilder::new(self, name, PassKind::Graphics)
    }

    /// Start building a compute pass.
    pub fn compute_pass<'g>(&'g mut self, name: impl Into<String>) -> PassBuilder<'g> {
        PassBuilder::new(self, name, PassKind::Compute)
    }

    /// Start building a transfer pass.
    pub fn transfer_pass<'g>(&'g mut self, name: impl Into<String>) -> PassBuilder<'g> {
        PassBuilder::new(self, name, PassKind::Transfer)
    }

    /// Compile the graph: topological sort + barrier insertion.
    pub fn compile(self) -> Result<CompiledGraph, GraphError> {
        let compiler = GraphCompiler::new(self);
        compiler.compile()
    }
}

impl Default for RenderGraph {
    fn default() -> Self { Self::new() }
}

// ── GraphError ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum GraphError {
    /// Cycle detected between named passes.
    CycleDetected(Vec<String>),
    /// A pass reads a resource that no pass writes (and it's not declared).
    UnresolvedResource { pass: String, resource: String },
    /// A pass named in `depends` does not exist.
    UnknownDependency { pass: String, dep: String },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CycleDetected(cycle) =>
                write!(f, "render graph cycle: {}", cycle.join(" → ")),
            Self::UnresolvedResource { pass, resource } =>
                write!(f, "pass '{}' reads undeclared resource '{}'", pass, resource),
            Self::UnknownDependency { pass, dep } =>
                write!(f, "pass '{}' depends on unknown pass '{}'", pass, dep),
        }
    }
}

// ── CompiledPass ──────────────────────────────────────────────────────────────

/// A pass in the compiled execution order, with pre-computed barriers.
#[derive(Debug, Clone)]
pub struct CompiledPass {
    pub pass:         RenderPass,
    /// Barriers to insert before this pass executes.
    pub pre_barriers: Vec<Barrier>,
}

// ── CompiledGraph ─────────────────────────────────────────────────────────────

/// The output of graph compilation: passes in execution order with barriers.
pub struct CompiledGraph {
    /// Passes sorted in dependency order (topological sort, priority-stable).
    pub passes:    Vec<CompiledPass>,
    pub resources: HashMap<String, ResourceDesc>,
    pub output:    Option<String>,
    /// Stats from the last compilation.
    pub stats:     CompileStats,
}

#[derive(Debug, Default, Clone)]
pub struct CompileStats {
    pub pass_count:    usize,
    pub barrier_count: usize,
    pub culled_passes: usize,
}

impl CompiledGraph {
    /// Iterate over passes in execution order.
    pub fn iter(&self) -> impl Iterator<Item = &CompiledPass> {
        self.passes.iter()
    }

    pub fn pass_count(&self) -> usize { self.passes.len() }

    /// Look up a resource descriptor by name.
    pub fn resource(&self, name: &str) -> Option<&ResourceDesc> {
        self.resources.get(name)
    }

    /// Compute the concrete pixel size of a resource given the backbuffer dimensions.
    pub fn resolve_size(&self, name: &str, bb_w: u32, bb_h: u32) -> Option<(u32, u32)> {
        let desc = self.resources.get(name)?;
        Some(match desc.size {
            ResourceSize::Fixed(w, h)   => (w, h),
            ResourceSize::Backbuffer    => (bb_w, bb_h),
            ResourceSize::Relative(s)   => (
                ((bb_w as f32 * s) as u32).max(1),
                ((bb_h as f32 * s) as u32).max(1),
            ),
        })
    }
}

// ── GraphCompiler ─────────────────────────────────────────────────────────────

struct GraphCompiler {
    graph: RenderGraph,
}

impl GraphCompiler {
    fn new(graph: RenderGraph) -> Self { Self { graph } }

    fn compile(mut self) -> Result<CompiledGraph, GraphError> {
        // ── 1. Validate explicit dependencies ──────────────────────────────
        let pass_names: HashSet<String> = self.graph.passes.iter()
            .map(|p| p.name.clone())
            .collect();

        for pass in &self.graph.passes {
            for dep in &pass.depends {
                if !pass_names.contains(dep) {
                    return Err(GraphError::UnknownDependency {
                        pass: pass.name.clone(),
                        dep:  dep.clone(),
                    });
                }
            }
        }

        // ── 2. Build writer map: resource → list of pass names that write it ─
        let mut writers: HashMap<String, Vec<String>> = HashMap::new();
        for pass in &self.graph.passes {
            for (res, _) in &pass.writes {
                writers.entry(res.clone()).or_default().push(pass.name.clone());
            }
        }

        // ── 3. Cull optional passes not reachable from output ──────────────
        let live_passes = self.compute_live_passes(&writers);
        let original_count = self.graph.passes.len();
        let culled = original_count - live_passes.len();
        self.graph.passes.retain(|p| live_passes.contains(&p.name));

        // ── 4. Build dependency edges for topological sort ─────────────────
        // Edge: pass A must run before pass B if B reads something A writes.
        let mut adj: HashMap<String, HashSet<String>> = HashMap::new(); // A -> set of B that depend on A
        let mut in_deg: HashMap<String, usize> = HashMap::new();
        for pass in &self.graph.passes {
            adj.entry(pass.name.clone()).or_default();
            in_deg.entry(pass.name.clone()).or_insert(0);
        }

        for pass_b in &self.graph.passes {
            for (res, _) in &pass_b.reads {
                if let Some(ws) = writers.get(res) {
                    for pass_a in ws {
                        if pass_a != &pass_b.name && live_passes.contains(pass_a) {
                            if adj.get(pass_a).map_or(true, |s| !s.contains(&pass_b.name)) {
                                adj.entry(pass_a.clone()).or_default().insert(pass_b.name.clone());
                                *in_deg.entry(pass_b.name.clone()).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
            // Explicit depends_on edges
            for dep in &pass_b.depends {
                if live_passes.contains(dep) {
                    if adj.get(dep).map_or(true, |s| !s.contains(&pass_b.name)) {
                        adj.entry(dep.clone()).or_default().insert(pass_b.name.clone());
                        *in_deg.entry(pass_b.name.clone()).or_insert(0) += 1;
                    }
                }
            }
        }

        // ── 5. Kahn's algorithm (priority-stable) ─────────────────────────
        let pass_map: HashMap<String, RenderPass> = self.graph.passes.drain(..)
            .map(|p| (p.name.clone(), p))
            .collect();

        let mut queue: VecDeque<String> = in_deg.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(n, _)| n.clone())
            .collect();

        // Sort by priority to make the initial order deterministic.
        let mut sorted: Vec<String> = Vec::new();
        let mut cycle_check = 0usize;

        while !queue.is_empty() {
            // Pick the lowest-priority node from the ready queue.
            let best_idx = queue.iter().enumerate()
                .min_by_key(|(_, n)| pass_map.get(*n).map_or(0, |p| p.priority))
                .map(|(i, _)| i)
                .unwrap_or(0);
            let node = queue.remove(best_idx).unwrap();
            sorted.push(node.clone());
            cycle_check += 1;

            if let Some(successors) = adj.get(&node) {
                for succ in successors {
                    let deg = in_deg.get_mut(succ).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(succ.clone());
                    }
                }
            }
        }

        if cycle_check != pass_map.len() {
            // Collect the cycle nodes (those with in_deg > 0 still)
            let cycle_nodes: Vec<String> = in_deg.iter()
                .filter(|(_, &d)| d > 0)
                .map(|(n, _)| n.clone())
                .collect();
            return Err(GraphError::CycleDetected(cycle_nodes));
        }

        // ── 6. Insert barriers ────────────────────────────────────────────
        // Track the last access mode for each resource.
        let mut last_access: HashMap<String, ResourceAccess> = HashMap::new();
        let mut compiled: Vec<CompiledPass> = Vec::new();
        let mut total_barriers = 0usize;

        for pass_name in &sorted {
            let pass = pass_map.get(pass_name).unwrap().clone();
            let mut pre_barriers = Vec::new();

            // Emit barriers for resources this pass reads.
            for (res, access) in &pass.reads {
                if let Some(&prev) = last_access.get(res) {
                    if needs_barrier(prev, *access) {
                        pre_barriers.push(Barrier::new(res, prev, *access));
                        total_barriers += 1;
                    }
                }
                last_access.insert(res.clone(), *access);
            }

            // Emit barriers for resources this pass writes.
            for (res, access) in &pass.writes {
                if let Some(&prev) = last_access.get(res) {
                    if needs_barrier(prev, *access) {
                        pre_barriers.push(Barrier::new(res, prev, *access));
                        total_barriers += 1;
                    }
                }
                last_access.insert(res.clone(), *access);
            }

            compiled.push(CompiledPass { pass, pre_barriers });
        }

        let stats = CompileStats {
            pass_count:    compiled.len(),
            barrier_count: total_barriers,
            culled_passes: culled,
        };

        Ok(CompiledGraph {
            passes:    compiled,
            resources: self.graph.resources,
            output:    self.graph.output,
            stats,
        })
    }

    /// Determine which passes are "live" (reachable backward from the output).
    fn compute_live_passes(&self, writers: &HashMap<String, Vec<String>>) -> HashSet<String> {
        // All non-optional passes are always live.
        let mut live: HashSet<String> = self.graph.passes.iter()
            .filter(|p| !p.optional)
            .map(|p| p.name.clone())
            .collect();

        // If there's an output, trace backward from it.
        if let Some(output) = &self.graph.output {
            let mut stack: Vec<String> = Vec::new();
            if let Some(ws) = writers.get(output) {
                stack.extend(ws.clone());
            }
            while let Some(pass_name) = stack.pop() {
                if live.insert(pass_name.clone()) {
                    // Trace its inputs too.
                    if let Some(pass) = self.graph.passes.iter().find(|p| p.name == pass_name) {
                        for (res, _) in &pass.reads {
                            if let Some(ws) = writers.get(res) {
                                for w in ws {
                                    if !live.contains(w) {
                                        stack.push(w.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        live
    }
}

/// Returns true if a pipeline barrier is needed between `src` and `dst` access.
fn needs_barrier(src: ResourceAccess, dst: ResourceAccess) -> bool {
    use ResourceAccess::*;
    // Write followed by anything → barrier.
    // Anything followed by write → barrier.
    // Read followed by read (same) → no barrier.
    if src == dst && src.is_read() { return false; }
    src.is_write() || dst.is_write()
}

// ── Standard Frame Graph ──────────────────────────────────────────────────────

/// Build the standard proof-engine frame render graph.
///
/// Passes (in dependency order):
/// 1. `depth_prepass`    — writes `depth`
/// 2. `particle_update`  — compute, reads/writes `particle_buf`
/// 3. `gbuffer`          — writes `gbuffer_albedo`, `gbuffer_normal`, `gbuffer_emissive`, reads `depth`
/// 4. `ssao`             — compute, reads `depth` + `gbuffer_normal`, writes `ssao`
/// 5. `lighting`         — reads gbuffer + ssao, writes `hdr`
/// 6. `particle_draw`    — reads `particle_buf` + `depth`, writes `hdr`
/// 7. `bloom_down`       — reads `hdr`, writes `bloom_half`
/// 8. `bloom_up`         — reads `bloom_half`, writes `bloom`
/// 9. `tonemap`          — reads `hdr` + `bloom`, writes `ldr`
/// 10. `fxaa`            — reads `ldr`, writes `backbuffer`
pub fn standard_frame_graph() -> RenderGraph {
    let mut g = RenderGraph::new();

    // ── Resource declarations ─────────────────────────────────────────────
    g.declare_resource(ResourceDesc::depth("depth").persistent());
    g.declare_resource(ResourceDesc::color("gbuffer_albedo",   TextureFormat::Rgba8Unorm));
    g.declare_resource(ResourceDesc::color("gbuffer_normal",   TextureFormat::Rgba16Float));
    g.declare_resource(ResourceDesc::color("gbuffer_emissive", TextureFormat::Rgba16Float));
    g.declare_resource(ResourceDesc::color("ssao",             TextureFormat::R32Float).half_res());
    g.declare_resource(ResourceDesc::color("hdr",              TextureFormat::Rgba16Float));
    g.declare_resource(ResourceDesc::color("bloom_half",       TextureFormat::Rgba16Float).half_res());
    g.declare_resource(ResourceDesc::color("bloom",            TextureFormat::Rgba16Float).half_res());
    g.declare_resource(ResourceDesc::color("ldr",              TextureFormat::Rgba8Unorm));
    g.declare_resource(ResourceDesc::color("particle_buf",     TextureFormat::Rgba32Float).persistent());

    g.set_output("backbuffer");

    // ── Passes ────────────────────────────────────────────────────────────
    g.graphics_pass("depth_prepass")
        .write_depth("depth")
        .priority(-100)
        .build();

    g.compute_pass("particle_update")
        .compute_read("particle_buf")
        .compute_write("particle_buf")
        .priority(-90)
        .build();

    g.graphics_pass("gbuffer")
        .write("gbuffer_albedo")
        .write("gbuffer_normal")
        .write("gbuffer_emissive")
        .read_depth("depth")
        .priority(-80)
        .build();

    g.compute_pass("ssao")
        .read("gbuffer_normal")
        .read_depth("depth")
        .compute_write("ssao")
        .priority(-70)
        .build();

    g.graphics_pass("lighting")
        .read("gbuffer_albedo")
        .read("gbuffer_normal")
        .read("gbuffer_emissive")
        .read("ssao")
        .write("hdr")
        .priority(-60)
        .build();

    g.graphics_pass("particle_draw")
        .compute_read("particle_buf")
        .read_depth("depth")
        .write("hdr")
        .priority(-50)
        .build();

    g.graphics_pass("bloom_down")
        .read("hdr")
        .write("bloom_half")
        .priority(-40)
        .build();

    g.graphics_pass("bloom_up")
        .read("bloom_half")
        .write("bloom")
        .priority(-30)
        .build();

    g.graphics_pass("tonemap")
        .read("hdr")
        .read("bloom")
        .write("ldr")
        .priority(-20)
        .build();

    g.graphics_pass("fxaa")
        .read("ldr")
        .write("backbuffer")
        .priority(-10)
        .build();

    g
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_graph_compiles() {
        let g = standard_frame_graph();
        let compiled = g.compile().expect("standard graph should compile");
        assert!(compiled.pass_count() >= 9);
    }

    #[test]
    fn test_barrier_inserted_between_write_and_read() {
        let mut g = RenderGraph::new();
        g.declare_resource(ResourceDesc::color("tex", TextureFormat::Rgba8Unorm));
        g.graphics_pass("writer").write("tex").build();
        g.graphics_pass("reader").read("tex").build();
        let compiled = g.compile().unwrap();
        let reader = compiled.passes.iter().find(|p| p.pass.name == "reader").unwrap();
        assert!(!reader.pre_barriers.is_empty(), "barrier expected before reader");
    }

    #[test]
    fn test_cycle_detection() {
        let mut g = RenderGraph::new();
        g.declare_resource(ResourceDesc::color("a", TextureFormat::Rgba8Unorm));
        g.declare_resource(ResourceDesc::color("b", TextureFormat::Rgba8Unorm));
        // A writes 'a', reads 'b'; B writes 'b', reads 'a' → cycle
        g.graphics_pass("pass_a").write("a").read("b").build();
        g.graphics_pass("pass_b").write("b").read("a").build();
        assert!(matches!(g.compile(), Err(GraphError::CycleDetected(_))));
    }

    #[test]
    fn test_no_barrier_between_two_reads() {
        let mut g = RenderGraph::new();
        g.declare_resource(ResourceDesc::color("tex", TextureFormat::Rgba8Unorm));
        g.declare_resource(ResourceDesc::color("out1", TextureFormat::Rgba8Unorm));
        g.declare_resource(ResourceDesc::color("out2", TextureFormat::Rgba8Unorm));
        // Write tex first
        g.graphics_pass("init").write("tex").build();
        // Two independent readers
        g.graphics_pass("r1").read("tex").write("out1").after("init").build();
        g.graphics_pass("r2").read("tex").write("out2").after("init").build();
        let compiled = g.compile().unwrap();
        // The second reader of 'tex' (whichever comes second) should not need a barrier for tex
        // (read→read same mode = no barrier)
        let barriers: Vec<_> = compiled.passes.iter()
            .flat_map(|p| p.pre_barriers.iter())
            .filter(|b| b.resource == "tex"
                && b.src_access == ResourceAccess::ShaderRead
                && b.dst_access == ResourceAccess::ShaderRead)
            .collect();
        assert!(barriers.is_empty(), "read→read should not emit a barrier");
    }

    #[test]
    fn test_resolve_size() {
        let g = standard_frame_graph();
        let compiled = g.compile().unwrap();
        let (w, h) = compiled.resolve_size("ssao", 1920, 1080).unwrap();
        assert_eq!(w, 960);
        assert_eq!(h, 540);
    }
}
