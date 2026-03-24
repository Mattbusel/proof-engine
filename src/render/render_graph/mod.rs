//! Render Graph + Deferred Rendering Pipeline for Proof Engine.
//!
//! A declarative render graph system:
//! - Named render passes with input/output attachments
//! - G-Buffer: albedo, normal, roughness/metallic, emissive, depth
//! - Deferred lighting pass: point/spot/directional lights
//! - HDR pipeline: exposure, tone mapping (ACES/Reinhard/Uncharted2)
//! - Post-processing passes: SSAO, bloom, TAA, FXAA, chromatic aberration
//! - Shadow mapping: cascaded shadow maps (CSM)
//! - Automatic dependency ordering and resource aliasing

use std::collections::{HashMap, HashSet, VecDeque};

// ── ResourceHandle ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttachmentId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassId(pub u32);

// ── TextureFormat ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgba8Srgb,
    Rgba16Float,
    Rgba32Float,
    Rg16Float,
    Rg32Float,
    R8Unorm,
    R16Float,
    R32Float,
    Depth24Stencil8,
    Depth32Float,
    Rgb10A2Unorm,
}

impl TextureFormat {
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            TextureFormat::R8Unorm         => 1,
            TextureFormat::R16Float        => 2,
            TextureFormat::R32Float        => 4,
            TextureFormat::Rg16Float       => 4,
            TextureFormat::Rg32Float       => 8,
            TextureFormat::Rgba8Unorm      => 4,
            TextureFormat::Rgba8Srgb       => 4,
            TextureFormat::Rgb10A2Unorm    => 4,
            TextureFormat::Rgba16Float     => 8,
            TextureFormat::Rgba32Float     => 16,
            TextureFormat::Depth24Stencil8 => 4,
            TextureFormat::Depth32Float    => 4,
        }
    }

    pub fn is_depth(self) -> bool {
        matches!(self, TextureFormat::Depth24Stencil8 | TextureFormat::Depth32Float)
    }

    pub fn is_hdr(self) -> bool {
        matches!(self, TextureFormat::Rgba16Float | TextureFormat::Rgba32Float | TextureFormat::R16Float | TextureFormat::R32Float)
    }
}

// ── AttachmentDesc ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttachmentSize {
    /// Same as the output resolution.
    Full,
    /// Half resolution on each axis.
    Half,
    /// Quarter resolution.
    Quarter,
    /// Fixed size.
    Fixed(u32, u32),
    /// Relative to output: width * scale, height * scale.
    Scale(f32),
}

#[derive(Debug, Clone)]
pub struct AttachmentDesc {
    pub id:      AttachmentId,
    pub name:    String,
    pub format:  TextureFormat,
    pub size:    AttachmentSize,
    pub mip_levels: u32,
    pub samples:    u32,
    pub persistent: bool, // kept across frames
    pub clear_value: ClearValue,
}

#[derive(Debug, Clone, Copy)]
pub enum ClearValue {
    Color([f32; 4]),
    Depth(f32, u8),
    None,
}

impl AttachmentDesc {
    pub fn color(name: &str, format: TextureFormat) -> Self {
        Self {
            id: AttachmentId(0), name: name.to_string(), format,
            size: AttachmentSize::Full, mip_levels: 1, samples: 1,
            persistent: false, clear_value: ClearValue::Color([0.0, 0.0, 0.0, 1.0]),
        }
    }

    pub fn depth(name: &str) -> Self {
        Self {
            id: AttachmentId(0), name: name.to_string(),
            format: TextureFormat::Depth32Float,
            size: AttachmentSize::Full, mip_levels: 1, samples: 1,
            persistent: false, clear_value: ClearValue::Depth(1.0, 0),
        }
    }

    pub fn with_size(mut self, size: AttachmentSize) -> Self { self.size = size; self }
    pub fn persistent(mut self) -> Self { self.persistent = true; self }
    pub fn msaa(mut self, samples: u32) -> Self { self.samples = samples; self }
}

// ── PassDesc ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PassKind {
    Graphics,
    Compute,
    Copy,
}

#[derive(Debug, Clone)]
pub struct PassDesc {
    pub id:       PassId,
    pub name:     String,
    pub kind:     PassKind,
    /// Color/depth attachments written to.
    pub outputs:  Vec<AttachmentId>,
    /// Attachments read as textures.
    pub inputs:   Vec<AttachmentId>,
    /// Depth-stencil output.
    pub depth_output: Option<AttachmentId>,
    pub enabled:  bool,
    pub sort_key: i32,
    /// For compute passes.
    pub work_groups: [u32; 3],
}

impl PassDesc {
    pub fn graphics(name: &str) -> Self {
        Self { id: PassId(0), name: name.to_string(), kind: PassKind::Graphics,
               outputs: Vec::new(), inputs: Vec::new(), depth_output: None,
               enabled: true, sort_key: 0, work_groups: [1, 1, 1] }
    }

    pub fn compute(name: &str, wx: u32, wy: u32, wz: u32) -> Self {
        let mut p = Self::graphics(name);
        p.kind = PassKind::Compute;
        p.work_groups = [wx, wy, wz];
        p
    }

    pub fn writes(mut self, att: AttachmentId) -> Self { self.outputs.push(att); self }
    pub fn reads(mut self,  att: AttachmentId) -> Self { self.inputs.push(att); self }
    pub fn depth(mut self,  att: AttachmentId) -> Self { self.depth_output = Some(att); self }
}

// ── G-Buffer layout ───────────────────────────────────────────────────────────

/// Standard deferred G-Buffer attachment layout.
#[derive(Debug, Clone)]
pub struct GBuffer {
    /// Albedo (RGB) + AO (A) — RGBA8
    pub albedo_ao:    AttachmentId,
    /// World-space normal (RGB packed) + roughness (A) — RGBA16F
    pub normal_rough: AttachmentId,
    /// Emissive (RGB) + metallic (A) — RGBA16F
    pub emissive_met: AttachmentId,
    /// World-space position — RGBA32F
    pub position:     AttachmentId,
    /// Motion vectors for TAA — RG16F
    pub motion:       AttachmentId,
    /// Primary depth buffer
    pub depth:        AttachmentId,
}

// ── Shadow Map Cascade ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CascadedShadowMap {
    pub num_cascades:  usize,
    pub resolution:    u32,
    pub lambda:        f32, // blend between uniform and log splits
    pub bias:          f32,
    pub normal_bias:   f32,
    pub filter_mode:   ShadowFilter,
    pub cascade_maps:  Vec<AttachmentId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadowFilter {
    Hard,
    Pcf2x2,
    Pcf4x4,
    Pcss,
}

impl CascadedShadowMap {
    pub fn new(num_cascades: usize, resolution: u32) -> Self {
        Self {
            num_cascades, resolution,
            lambda: 0.75, bias: 0.005, normal_bias: 0.02,
            filter_mode: ShadowFilter::Pcf4x4,
            cascade_maps: Vec::new(),
        }
    }
}

// ── ToneMap operator ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToneMap {
    Linear,
    Reinhard,
    ReinhardExtended { white: f32 },
    Aces,
    Uncharted2,
    Lottes,
    AgX,
}

impl ToneMap {
    pub fn apply(self, hdr: [f32; 3]) -> [f32; 3] {
        let [r, g, b] = hdr;
        match self {
            ToneMap::Linear => [r.min(1.0), g.min(1.0), b.min(1.0)],
            ToneMap::Reinhard => {
                [r/(1.0+r), g/(1.0+g), b/(1.0+b)]
            }
            ToneMap::ReinhardExtended { white } => {
                let w2 = white * white;
                [r*(1.0+r/w2)/(1.0+r), g*(1.0+g/w2)/(1.0+g), b*(1.0+b/w2)/(1.0+b)]
            }
            ToneMap::Aces => {
                // ACES fitted curve
                let aces = |x: f32| (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
                [aces(r).clamp(0.0, 1.0), aces(g).clamp(0.0, 1.0), aces(b).clamp(0.0, 1.0)]
            }
            ToneMap::Uncharted2 => {
                let u2 = |x: f32| ((x*(0.15*x+0.10*0.50)+0.20*0.02)/(x*(0.15*x+0.50)+0.20*0.30))-0.02/0.30;
                let white_scale = 1.0 / u2(11.2);
                [u2(r*2.0)*white_scale, u2(g*2.0)*white_scale, u2(b*2.0)*white_scale]
            }
            ToneMap::Lottes => {
                let lottes = |x: f32| {
                    let a: f32 = 1.6; let d: f32 = 0.977; let hdr_max: f32 = 8.0;
                    let mid_in: f32 = 0.18; let mid_out: f32 = 0.267;
                    let b = (-mid_in.powf(a) + hdr_max.powf(a) * mid_out) / ((hdr_max.powf(a * d) - mid_in.powf(a * d)) * mid_out);
                    let c = (hdr_max.powf(a * d) * mid_in.powf(a) - hdr_max.powf(a) * mid_in.powf(a * d) * mid_out) / ((hdr_max.powf(a * d) - mid_in.powf(a * d)) * mid_out);
                    x.powf(a) / (x.powf(a * d) * b + c)
                };
                [lottes(r), lottes(g), lottes(b)]
            }
            ToneMap::AgX => {
                // Simplified AgX approximation
                let agx = |x: f32| {
                    let x = x.max(0.0);
                    let log_x = (x + 0.0001).log2();
                    let compressed = (log_x * 0.5 + 0.5).clamp(0.0, 1.0);
                    // S-curve
                    compressed * compressed * (3.0 - 2.0 * compressed)
                };
                [agx(r), agx(g), agx(b)]
            }
        }
    }
}

// ── SSAO parameters ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SsaoParams {
    pub num_samples:    u32,
    pub radius:         f32,
    pub bias:           f32,
    pub intensity:      f32,
    pub blur_passes:    u32,
}

impl Default for SsaoParams {
    fn default() -> Self {
        Self { num_samples: 64, radius: 0.5, bias: 0.025, intensity: 1.5, blur_passes: 2 }
    }
}

// ── Bloom parameters ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BloomParams {
    pub threshold:    f32,
    pub knee:         f32,
    pub intensity:    f32,
    pub num_mips:     u32,
    pub filter_mode:  BloomFilter,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BloomFilter {
    Box,
    Tent,
    Kawase,
}

impl Default for BloomParams {
    fn default() -> Self {
        Self { threshold: 1.0, knee: 0.5, intensity: 0.5, num_mips: 6, filter_mode: BloomFilter::Kawase }
    }
}

// ── TAA parameters ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TaaParams {
    pub blend_factor:   f32,
    pub jitter_scale:   f32,
    pub velocity_scale: f32,
    pub sharpen:        f32,
    /// Halton sequence for jitter
    pub jitter_seq:     Vec<[f32; 2]>,
    pub frame:          u32,
}

impl TaaParams {
    pub fn new() -> Self {
        let jitter_seq = (0..16).map(|i| halton_2d(i + 1)).collect();
        Self { blend_factor: 0.1, jitter_scale: 1.0, velocity_scale: 1.0, sharpen: 0.2, jitter_seq, frame: 0 }
    }

    pub fn current_jitter(&self) -> [f32; 2] {
        let i = (self.frame as usize) % self.jitter_seq.len();
        self.jitter_seq[i]
    }

    pub fn advance(&mut self) { self.frame += 1; }
}

impl Default for TaaParams { fn default() -> Self { Self::new() } }

fn halton_2d(i: usize) -> [f32; 2] {
    [halton(i, 2), halton(i, 3)]
}

fn halton(mut i: usize, base: usize) -> f32 {
    let mut f = 1.0_f32;
    let mut r = 0.0_f32;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

// ── Light types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightType {
    Point,
    Spot,
    Directional,
    Area,
}

#[derive(Debug, Clone)]
pub struct RenderLight {
    pub light_type:   LightType,
    pub position:     [f32; 3],
    pub direction:    [f32; 3],
    pub color:        [f32; 3],
    pub intensity:    f32,
    pub radius:       f32,
    pub inner_angle:  f32, // spot inner cone (radians)
    pub outer_angle:  f32, // spot outer cone
    pub cast_shadows: bool,
    pub shadow_bias:  f32,
    pub area_size:    [f32; 2], // for area lights
}

impl RenderLight {
    pub fn point(pos: [f32; 3], color: [f32; 3], intensity: f32, radius: f32) -> Self {
        Self {
            light_type: LightType::Point, position: pos, direction: [0.0, -1.0, 0.0],
            color, intensity, radius, inner_angle: 0.0, outer_angle: 0.0,
            cast_shadows: true, shadow_bias: 0.005, area_size: [1.0; 2],
        }
    }

    pub fn directional(dir: [f32; 3], color: [f32; 3], intensity: f32) -> Self {
        let mut l = Self::point([0.0; 3], color, intensity, 1000.0);
        l.light_type = LightType::Directional;
        l.direction = dir;
        l.cast_shadows = true;
        l
    }

    pub fn spot(pos: [f32; 3], dir: [f32; 3], color: [f32; 3], intensity: f32, outer_deg: f32) -> Self {
        let mut l = Self::point(pos, color, intensity, 20.0);
        l.light_type = LightType::Spot;
        l.direction = dir;
        l.inner_angle = (outer_deg * 0.8).to_radians();
        l.outer_angle = outer_deg.to_radians();
        l
    }

    /// Attenuation at a given distance (physically-based inverse square).
    pub fn attenuation(&self, dist: f32) -> f32 {
        match self.light_type {
            LightType::Directional => 1.0,
            _ => {
                let window = (1.0 - (dist / self.radius).powi(4)).max(0.0).powi(2);
                window / (dist * dist + 1.0)
            }
        }
    }
}

// ── RenderGraph ───────────────────────────────────────────────────────────────

/// Declarative render graph. Add passes and attachments, then compile.
pub struct RenderGraph {
    pub name:        String,
    passes:          HashMap<PassId, PassDesc>,
    attachments:     HashMap<AttachmentId, AttachmentDesc>,
    next_pass_id:    u32,
    next_att_id:     u32,
    /// Topological execution order (computed on compile).
    pub exec_order:  Vec<PassId>,
    compiled:        bool,
}

impl RenderGraph {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            passes: HashMap::new(),
            attachments: HashMap::new(),
            next_pass_id: 1,
            next_att_id: 1,
            exec_order: Vec::new(),
            compiled: false,
        }
    }

    pub fn add_attachment(&mut self, mut desc: AttachmentDesc) -> AttachmentId {
        let id = AttachmentId(self.next_att_id);
        self.next_att_id += 1;
        desc.id = id;
        self.attachments.insert(id, desc);
        id
    }

    pub fn add_pass(&mut self, mut desc: PassDesc) -> PassId {
        let id = PassId(self.next_pass_id);
        self.next_pass_id += 1;
        desc.id = id;
        self.passes.insert(id, desc);
        self.compiled = false;
        id
    }

    pub fn pass(&self, id: PassId) -> Option<&PassDesc> { self.passes.get(&id) }
    pub fn attachment(&self, id: AttachmentId) -> Option<&AttachmentDesc> { self.attachments.get(&id) }

    pub fn attachment_by_name(&self, name: &str) -> Option<AttachmentId> {
        self.attachments.values().find(|a| a.name == name).map(|a| a.id)
    }

    /// Topological sort of passes by their attachment dependencies.
    pub fn compile(&mut self) -> Result<(), String> {
        // Build adjacency: pass A → pass B if A writes to att that B reads.
        let mut writes: HashMap<AttachmentId, PassId> = HashMap::new();
        for (pid, pass) in &self.passes {
            for &att in &pass.outputs {
                writes.insert(att, *pid);
            }
            if let Some(d) = pass.depth_output {
                writes.insert(d, *pid);
            }
        }

        // In-degree and edges
        let mut in_degree: HashMap<PassId, usize> = self.passes.keys().map(|&id| (id, 0)).collect();
        let mut adj: HashMap<PassId, Vec<PassId>> = HashMap::new();

        for (pid, pass) in &self.passes {
            for &att in &pass.inputs {
                if let Some(&src_pass) = writes.get(&att) {
                    adj.entry(src_pass).or_default().push(*pid);
                    *in_degree.entry(*pid).or_insert(0) += 1;
                }
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<PassId> = in_degree.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            order.push(id);
            if let Some(deps) = adj.get(&id) {
                for &dep in deps {
                    let d = in_degree.entry(dep).or_insert(1);
                    *d -= 1;
                    if *d == 0 { queue.push_back(dep); }
                }
            }
        }

        if order.len() != self.passes.len() {
            return Err(format!("Render graph '{}' has a cycle", self.name));
        }

        self.exec_order = order;
        self.compiled = true;
        Ok(())
    }

    pub fn is_compiled(&self) -> bool { self.compiled }

    /// Total memory estimate for all attachments at a given resolution.
    pub fn estimate_memory_bytes(&self, width: u32, height: u32) -> u64 {
        self.attachments.values().map(|a| {
            let (w, h) = match a.size {
                AttachmentSize::Full         => (width, height),
                AttachmentSize::Half         => (width / 2, height / 2),
                AttachmentSize::Quarter      => (width / 4, height / 4),
                AttachmentSize::Fixed(w, h)  => (w, h),
                AttachmentSize::Scale(s)     => ((width as f32 * s) as u32, (height as f32 * s) as u32),
            };
            w as u64 * h as u64 * a.format.bytes_per_pixel() as u64 * a.samples as u64
        }).sum()
    }
}

// ── DeferredPipeline ──────────────────────────────────────────────────────────

/// Complete deferred rendering pipeline configuration.
pub struct DeferredPipeline {
    pub graph:     RenderGraph,
    pub gbuffer:   GBuffer,
    pub csm:       CascadedShadowMap,
    pub ssao:      SsaoParams,
    pub bloom:     BloomParams,
    pub taa:       TaaParams,
    pub tone_map:  ToneMap,
    pub exposure:  f32,
    pub lights:    Vec<RenderLight>,

    // Key pass IDs
    pub gbuffer_pass:   PassId,
    pub shadow_pass:    PassId,
    pub ssao_pass:      PassId,
    pub lighting_pass:  PassId,
    pub bloom_pass:     PassId,
    pub taa_pass:       PassId,
    pub tonemap_pass:   PassId,
    pub fxaa_pass:      PassId,

    // Intermediate attachments
    pub hdr_buffer:     AttachmentId,
    pub ldr_buffer:     AttachmentId,
    pub ssao_buffer:    AttachmentId,
    pub bloom_buffer:   AttachmentId,
    pub taa_history:    AttachmentId,
}

impl DeferredPipeline {
    /// Build a complete deferred rendering pipeline.
    pub fn new() -> Self {
        let mut graph = RenderGraph::new("deferred");

        // ── G-Buffer attachments ──────────────────────────────────────────────
        let albedo_ao = graph.add_attachment(
            AttachmentDesc::color("gbuf_albedo_ao", TextureFormat::Rgba8Unorm)
        );
        let normal_rough = graph.add_attachment(
            AttachmentDesc::color("gbuf_normal_rough", TextureFormat::Rgba16Float)
        );
        let emissive_met = graph.add_attachment(
            AttachmentDesc::color("gbuf_emissive_met", TextureFormat::Rgba16Float)
        );
        let position = graph.add_attachment(
            AttachmentDesc::color("gbuf_position", TextureFormat::Rgba32Float)
        );
        let motion = graph.add_attachment(
            AttachmentDesc::color("gbuf_motion", TextureFormat::Rg16Float)
        );
        let depth = graph.add_attachment(AttachmentDesc::depth("gbuf_depth"));

        let gbuffer = GBuffer { albedo_ao, normal_rough, emissive_met, position, motion, depth };

        // ── Shadow map attachments ────────────────────────────────────────────
        let mut csm = CascadedShadowMap::new(4, 2048);
        for i in 0..4 {
            let id = graph.add_attachment(
                AttachmentDesc::depth(&format!("shadow_cascade_{}", i))
                    .with_size(AttachmentSize::Fixed(2048, 2048))
                    .persistent()
            );
            csm.cascade_maps.push(id);
        }

        // ── Intermediate attachments ──────────────────────────────────────────
        let ssao_buffer  = graph.add_attachment(
            AttachmentDesc::color("ssao", TextureFormat::R8Unorm).with_size(AttachmentSize::Half)
        );
        let hdr_buffer   = graph.add_attachment(
            AttachmentDesc::color("hdr", TextureFormat::Rgba16Float)
        );
        let bloom_buffer = graph.add_attachment(
            AttachmentDesc::color("bloom", TextureFormat::Rgba16Float).with_size(AttachmentSize::Half)
        );
        let taa_history  = graph.add_attachment(
            AttachmentDesc::color("taa_history", TextureFormat::Rgba16Float).persistent()
        );
        let ldr_buffer   = graph.add_attachment(
            AttachmentDesc::color("ldr", TextureFormat::Rgba8Srgb)
        );

        // ── Passes ────────────────────────────────────────────────────────────

        // 1. G-Buffer fill
        let gbuffer_pass = graph.add_pass(
            PassDesc::graphics("gbuffer_fill")
                .writes(albedo_ao).writes(normal_rough).writes(emissive_met)
                .writes(position).writes(motion)
                .depth(depth)
        );

        // 2. Shadow map (per cascade)
        let shadow_pass = graph.add_pass(
            PassDesc::graphics("shadow_map")
                .depth(csm.cascade_maps[0])
        );

        // 3. SSAO
        let ssao_pass = graph.add_pass(
            PassDesc::graphics("ssao")
                .reads(normal_rough).reads(depth)
                .writes(ssao_buffer)
        );

        // 4. Deferred lighting
        let lighting_pass = graph.add_pass(
            PassDesc::graphics("deferred_lighting")
                .reads(albedo_ao).reads(normal_rough).reads(emissive_met)
                .reads(position).reads(ssao_buffer)
                .reads(csm.cascade_maps[0])
                .writes(hdr_buffer)
        );

        // 5. Bloom
        let bloom_pass = graph.add_pass(
            PassDesc::graphics("bloom")
                .reads(hdr_buffer)
                .writes(bloom_buffer)
        );

        // 6. TAA
        let taa_pass = graph.add_pass(
            PassDesc::graphics("taa")
                .reads(hdr_buffer).reads(motion).reads(taa_history)
                .writes(hdr_buffer)
        );

        // 7. Tone mapping
        let tonemap_pass = graph.add_pass(
            PassDesc::graphics("tonemap")
                .reads(hdr_buffer).reads(bloom_buffer)
                .writes(ldr_buffer)
        );

        // 8. FXAA
        let fxaa_pass = graph.add_pass(
            PassDesc::graphics("fxaa")
                .reads(ldr_buffer)
                .writes(ldr_buffer)
        );

        let _ = graph.compile(); // Initial compile

        Self {
            graph, gbuffer, csm, ssao: SsaoParams::default(),
            bloom: BloomParams::default(), taa: TaaParams::new(),
            tone_map: ToneMap::Aces, exposure: 1.0,
            lights: Vec::new(),
            gbuffer_pass, shadow_pass, ssao_pass, lighting_pass,
            bloom_pass, taa_pass, tonemap_pass, fxaa_pass,
            hdr_buffer, ldr_buffer, ssao_buffer, bloom_buffer, taa_history,
        }
    }

    pub fn add_light(&mut self, light: RenderLight) {
        self.lights.push(light);
    }

    pub fn add_point_light(&mut self, pos: [f32; 3], color: [f32; 3], intensity: f32, radius: f32) {
        self.lights.push(RenderLight::point(pos, color, intensity, radius));
    }

    pub fn add_sun_light(&mut self, dir: [f32; 3]) {
        self.lights.push(RenderLight::directional(dir, [1.0, 0.95, 0.8], 5.0));
    }

    pub fn set_tone_map(&mut self, tm: ToneMap) { self.tone_map = tm; }
    pub fn set_exposure(&mut self, exp: f32) { self.exposure = exp; }

    pub fn point_light_count(&self) -> usize {
        self.lights.iter().filter(|l| l.light_type == LightType::Point).count()
    }

    pub fn shadow_casters(&self) -> Vec<&RenderLight> {
        self.lights.iter().filter(|l| l.cast_shadows).collect()
    }

    /// Memory estimate for all render targets at given resolution.
    pub fn memory_estimate_mb(&self, w: u32, h: u32) -> f32 {
        self.graph.estimate_memory_bytes(w, h) as f32 / (1024.0 * 1024.0)
    }

    pub fn advance_frame(&mut self) {
        self.taa.advance();
    }
}

impl Default for DeferredPipeline {
    fn default() -> Self { Self::new() }
}

// ── GLSL: G-Buffer fill vertex shader ─────────────────────────────────────────

pub const GBUFFER_VERT_GLSL: &str = r#"
#version 450 core

layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;
layout(location = 3) in vec3 a_tangent;

layout(std140, binding = 0) uniform PerFrame {
    mat4 view;
    mat4 proj;
    mat4 jitter_proj; // TAA-jittered projection
    vec3 camera_pos;
    float time;
};

layout(std140, binding = 1) uniform PerObject {
    mat4 model;
    mat4 prev_model;
};

out vec3 v_world_pos;
out vec3 v_normal;
out vec2 v_uv;
out vec3 v_tangent;
out vec4 v_cur_clip;
out vec4 v_prev_clip;

void main() {
    vec4 world_pos  = model * vec4(a_pos, 1.0);
    v_world_pos     = world_pos.xyz;
    v_normal        = normalize(mat3(transpose(inverse(model))) * a_normal);
    v_uv            = a_uv;
    v_tangent       = normalize(mat3(model) * a_tangent);
    v_cur_clip      = jitter_proj * view * world_pos;
    v_prev_clip     = proj * view * prev_model * vec4(a_pos, 1.0);
    gl_Position     = v_cur_clip;
}
"#;

/// G-Buffer fill fragment shader.
pub const GBUFFER_FRAG_GLSL: &str = r#"
#version 450 core

in vec3 v_world_pos;
in vec3 v_normal;
in vec2 v_uv;
in vec3 v_tangent;
in vec4 v_cur_clip;
in vec4 v_prev_clip;

layout(location = 0) out vec4 out_albedo_ao;
layout(location = 1) out vec4 out_normal_rough;
layout(location = 2) out vec4 out_emissive_met;
layout(location = 3) out vec4 out_position;
layout(location = 4) out vec2 out_motion;

uniform sampler2D tex_albedo;
uniform sampler2D tex_normal;
uniform sampler2D tex_roughness;
uniform sampler2D tex_metallic;
uniform sampler2D tex_emissive;
uniform sampler2D tex_ao;

void main() {
    // Albedo + AO
    vec4 albedo = texture(tex_albedo, v_uv);
    if (albedo.a < 0.5) discard;
    float ao = texture(tex_ao, v_uv).r;
    out_albedo_ao = vec4(albedo.rgb, ao);

    // Normal mapping
    vec3 tn = texture(tex_normal, v_uv).xyz * 2.0 - 1.0;
    vec3 bitangent = cross(v_normal, v_tangent);
    mat3 tbn = mat3(v_tangent, bitangent, v_normal);
    vec3 world_normal = normalize(tbn * tn);

    float roughness = texture(tex_roughness, v_uv).r;
    out_normal_rough = vec4(world_normal * 0.5 + 0.5, roughness);

    // Emissive + metallic
    vec3 emissive = texture(tex_emissive, v_uv).rgb;
    float metallic = texture(tex_metallic, v_uv).r;
    out_emissive_met = vec4(emissive, metallic);

    // World position
    out_position = vec4(v_world_pos, 1.0);

    // Motion vectors (clip space delta → screen space)
    vec2 cur  = (v_cur_clip.xy  / v_cur_clip.w)  * 0.5 + 0.5;
    vec2 prev = (v_prev_clip.xy / v_prev_clip.w) * 0.5 + 0.5;
    out_motion = cur - prev;
}
"#;

/// Deferred lighting fragment shader (PBR with Cook-Torrance BRDF).
pub const DEFERRED_LIGHTING_GLSL: &str = r#"
#version 450 core

out vec4 frag_color;
in vec2 v_uv;

uniform sampler2D gbuf_albedo_ao;
uniform sampler2D gbuf_normal_rough;
uniform sampler2D gbuf_emissive_met;
uniform sampler2D gbuf_position;
uniform sampler2D tex_ssao;
uniform sampler2DArray shadow_maps;

struct Light {
    vec4 position;   // xyz + type (0=dir, 1=point, 2=spot)
    vec4 direction;  // xyz + inner_angle
    vec4 color;      // rgb + intensity
    vec4 params;     // radius, outer_angle, bias, shadow_idx
};

layout(std140, binding = 2) uniform Lights {
    Light lights[64];
    int num_lights;
};

layout(std140, binding = 0) uniform PerFrame {
    mat4 view;
    mat4 proj;
    mat4 jitter_proj;
    vec3 camera_pos;
    float exposure;
};

const float PI = 3.14159265359;

// PBR functions
float distribution_ggx(vec3 N, vec3 H, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float NdH = max(dot(N, H), 0.0);
    float denom = NdH * NdH * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

float geometry_schlick_ggx(float NdV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    return NdV / (NdV * (1.0 - k) + k);
}

vec3 fresnel_schlick(float cos_theta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

void main() {
    // Sample G-Buffer
    vec4 albedo_ao     = texture(gbuf_albedo_ao, v_uv);
    vec4 normal_rough  = texture(gbuf_normal_rough, v_uv);
    vec4 emissive_met  = texture(gbuf_emissive_met, v_uv);
    vec4 pos_sample    = texture(gbuf_position, v_uv);
    float ao           = texture(tex_ssao, v_uv).r;

    vec3 albedo   = albedo_ao.rgb;
    float geo_ao  = albedo_ao.a;
    vec3 N        = normalize(normal_rough.rgb * 2.0 - 1.0);
    float rough   = max(normal_rough.a, 0.04);
    vec3 emissive = emissive_met.rgb;
    float metal   = emissive_met.a;
    vec3 P        = pos_sample.xyz;

    vec3 V  = normalize(camera_pos - P);
    vec3 F0 = mix(vec3(0.04), albedo, metal);

    vec3 Lo = vec3(0.0);

    for (int i = 0; i < num_lights; i++) {
        Light L = lights[i];
        vec3 lpos = L.position.xyz;
        vec3 lcol = L.color.rgb * L.color.w * exposure;

        vec3 l_dir;
        float attenuation = 1.0;
        int ltype = int(L.position.w);

        if (ltype == 0) {
            // Directional
            l_dir = normalize(-L.direction.xyz);
        } else {
            // Point / Spot
            vec3 delta = lpos - P;
            float dist = length(delta);
            l_dir = delta / dist;
            float r = L.params.x;
            float window = pow(max(1.0 - pow(dist/r, 4.0), 0.0), 2.0);
            attenuation = window / (dist * dist + 1.0);
        }

        // Spot cone
        if (ltype == 2) {
            float theta = dot(l_dir, normalize(-L.direction.xyz));
            float inner = L.direction.w;
            float outer = L.params.y;
            attenuation *= clamp((theta - outer) / (inner - outer + 0.001), 0.0, 1.0);
        }

        vec3 H  = normalize(V + l_dir);
        float NdL = max(dot(N, l_dir), 0.0);
        float NdV = max(dot(N, V), 0.0);

        // Cook-Torrance BRDF
        float D = distribution_ggx(N, H, rough);
        float G = geometry_schlick_ggx(NdV, rough) * geometry_schlick_ggx(NdL, rough);
        vec3  F = fresnel_schlick(max(dot(H, V), 0.0), F0);

        vec3 kd = (vec3(1.0) - F) * (1.0 - metal);
        vec3 diffuse  = kd * albedo / PI;
        vec3 specular = (D * G * F) / max(4.0 * NdV * NdL, 0.001);

        Lo += (diffuse + specular) * lcol * attenuation * NdL;
    }

    // Ambient
    vec3 ambient = vec3(0.03) * albedo * min(ao, geo_ao);

    vec3 color = ambient + Lo + emissive;
    frag_color = vec4(color, 1.0);
}
"#;

/// SSAO compute shader.
pub const SSAO_FRAG_GLSL: &str = r#"
#version 450 core

out float frag_ao;
in vec2 v_uv;

uniform sampler2D gbuf_normal;
uniform sampler2D gbuf_depth;
uniform sampler2D tex_noise;

layout(std140, binding = 0) uniform SsaoParams {
    vec3  samples[64];
    mat4  proj;
    mat4  view;
    float radius;
    float bias;
    float intensity;
    int   num_samples;
};

uniform vec2 noise_scale;

void main() {
    // Reconstruct position from depth
    float depth    = texture(gbuf_depth, v_uv).r;
    vec4  ndc      = vec4(v_uv * 2.0 - 1.0, depth * 2.0 - 1.0, 1.0);
    vec4  view_pos = inverse(proj) * ndc;
    view_pos /= view_pos.w;

    vec3 normal    = normalize(mat3(view) * (texture(gbuf_normal, v_uv).rgb * 2.0 - 1.0));
    vec3 rand_vec  = normalize(texture(tex_noise, v_uv * noise_scale).xyz);

    // TBN to orient hemisphere
    vec3 tangent   = normalize(rand_vec - normal * dot(rand_vec, normal));
    vec3 bitangent = cross(normal, tangent);
    mat3 TBN       = mat3(tangent, bitangent, normal);

    float occlusion = 0.0;
    for (int i = 0; i < num_samples; i++) {
        vec3 sample_pos = TBN * samples[i];
        sample_pos = view_pos.xyz + sample_pos * radius;

        // Project to get UV
        vec4 offset = proj * vec4(sample_pos, 1.0);
        offset.xyz /= offset.w;
        offset.xyz = offset.xyz * 0.5 + 0.5;

        float sample_depth = texture(gbuf_depth, offset.xy).r;
        vec4 s_ndc = vec4(offset.xy * 2.0 - 1.0, sample_depth * 2.0 - 1.0, 1.0);
        vec4 s_view = inverse(proj) * s_ndc;
        s_view /= s_view.w;

        float range_check = smoothstep(0.0, 1.0, radius / abs(view_pos.z - s_view.z));
        occlusion += (s_view.z >= sample_pos.z + bias ? 1.0 : 0.0) * range_check;
    }

    frag_ao = 1.0 - (occlusion / float(num_samples)) * intensity;
}
"#;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deferred_pipeline_builds() {
        let pipeline = DeferredPipeline::new();
        assert!(pipeline.graph.is_compiled());
        assert!(!pipeline.graph.exec_order.is_empty());
    }

    #[test]
    fn test_render_graph_compiles() {
        let mut graph = RenderGraph::new("test");
        let a = graph.add_attachment(AttachmentDesc::color("color", TextureFormat::Rgba8Unorm));
        let b = graph.add_attachment(AttachmentDesc::color("out", TextureFormat::Rgba8Srgb));
        let p1 = graph.add_pass(PassDesc::graphics("fill").writes(a));
        let _p2 = graph.add_pass(PassDesc::graphics("post").reads(a).writes(b));
        assert!(graph.compile().is_ok());
        assert_eq!(graph.exec_order[0], p1);
    }

    #[test]
    fn test_tone_map_aces() {
        let tm = ToneMap::Aces;
        let result = tm.apply([2.0, 1.0, 0.5]);
        // All values should be in [0, 1]
        for v in result { assert!(v >= 0.0 && v <= 1.0, "ACES out of range: {}", v); }
    }

    #[test]
    fn test_tone_map_reinhard() {
        let tm = ToneMap::Reinhard;
        let result = tm.apply([1.0, 0.5, 0.0]);
        assert!((result[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_all_tone_maps() {
        let maps = [
            ToneMap::Linear, ToneMap::Reinhard, ToneMap::Aces,
            ToneMap::Uncharted2, ToneMap::Lottes, ToneMap::AgX,
        ];
        for tm in maps {
            let result = tm.apply([1.0, 0.5, 0.1]);
            for v in result { assert!(v.is_finite(), "{:?} produced NaN/Inf", tm); }
        }
    }

    #[test]
    fn test_taa_halton_sequence() {
        let taa = TaaParams::new();
        assert_eq!(taa.jitter_seq.len(), 16);
        // All jitter values in [0, 1)
        for j in &taa.jitter_seq {
            assert!(j[0] >= 0.0 && j[0] < 1.0);
            assert!(j[1] >= 0.0 && j[1] < 1.0);
        }
    }

    #[test]
    fn test_light_attenuation() {
        let light = RenderLight::point([0.0; 3], [1.0; 3], 1.0, 10.0);
        let att_near = light.attenuation(0.5);
        let att_far  = light.attenuation(9.5);
        assert!(att_near > att_far, "attenuation should decrease with distance");
    }

    #[test]
    fn test_directional_light_constant_attenuation() {
        let light = RenderLight::directional([0.0, -1.0, 0.0], [1.0; 3], 1.0);
        assert_eq!(light.attenuation(100.0), 1.0);
        assert_eq!(light.attenuation(1000.0), 1.0);
    }

    #[test]
    fn test_memory_estimate() {
        let pipeline = DeferredPipeline::new();
        let mb = pipeline.memory_estimate_mb(1920, 1080);
        assert!(mb > 10.0, "1080p deferred should use at least 10 MB for RTs");
        assert!(mb < 2000.0, "should not estimate more than 2GB for standard 1080p");
    }

    #[test]
    fn test_texture_format_depth_flag() {
        assert!(TextureFormat::Depth32Float.is_depth());
        assert!(!TextureFormat::Rgba16Float.is_depth());
    }

    #[test]
    fn test_gbuffer_shader_sources_not_empty() {
        assert!(!GBUFFER_VERT_GLSL.is_empty());
        assert!(!GBUFFER_FRAG_GLSL.is_empty());
        assert!(!DEFERRED_LIGHTING_GLSL.is_empty());
        assert!(!SSAO_FRAG_GLSL.is_empty());
    }

    #[test]
    fn test_csm_cascades() {
        let pipeline = DeferredPipeline::new();
        assert_eq!(pipeline.csm.num_cascades, 4);
        assert_eq!(pipeline.csm.cascade_maps.len(), 4);
    }
}
