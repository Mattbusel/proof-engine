//! Automatic resource management for the render graph.
//!
//! Provides resource descriptors, transient/imported resources, pooling with
//! frame-based lifetime tracking, resource aliasing, versioning for
//! read-after-write hazard detection, and memory budget estimation.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Texture format & usage
// ---------------------------------------------------------------------------

/// Pixel / data format for textures and buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    R8Unorm,
    R16Float,
    R32Float,
    Rg8Unorm,
    Rg16Float,
    Rg32Float,
    Rgba8Unorm,
    Rgba8Srgb,
    Rgba16Float,
    Rgba32Float,
    Bgra8Unorm,
    Bgra8Srgb,
    Depth16Unorm,
    Depth24PlusStencil8,
    Depth32Float,
    R11G11B10Float,
    Rgb10A2Unorm,
    Bc1Unorm,
    Bc3Unorm,
    Bc5Unorm,
    Bc7Unorm,
}

impl TextureFormat {
    /// Bytes per pixel (approximate for block-compressed formats).
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::R8Unorm => 1,
            Self::R16Float => 2,
            Self::R32Float => 4,
            Self::Rg8Unorm => 2,
            Self::Rg16Float => 4,
            Self::Rg32Float => 8,
            Self::Rgba8Unorm | Self::Rgba8Srgb => 4,
            Self::Rgba16Float => 8,
            Self::Rgba32Float => 16,
            Self::Bgra8Unorm | Self::Bgra8Srgb => 4,
            Self::Depth16Unorm => 2,
            Self::Depth24PlusStencil8 => 4,
            Self::Depth32Float => 4,
            Self::R11G11B10Float => 4,
            Self::Rgb10A2Unorm => 4,
            Self::Bc1Unorm => 1, // 0.5 bpp * 2 (rough)
            Self::Bc3Unorm | Self::Bc5Unorm | Self::Bc7Unorm => 1,
        }
    }

    /// Returns true for depth / depth-stencil formats.
    pub fn is_depth(self) -> bool {
        matches!(
            self,
            Self::Depth16Unorm | Self::Depth24PlusStencil8 | Self::Depth32Float
        )
    }
}

/// How a resource can be bound in a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UsageFlags {
    RenderTarget,
    DepthStencil,
    ShaderRead,
    ShaderWrite,
    CopySource,
    CopyDest,
    StorageBuffer,
    UniformBuffer,
}

/// Description of the dimensions / scale of a texture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizePolicy {
    /// Exact pixel dimensions.
    Absolute { width: u32, height: u32 },
    /// Fraction of the backbuffer resolution.
    Relative { width_scale: f32, height_scale: f32 },
}

impl SizePolicy {
    /// Resolve to concrete pixel dimensions given the backbuffer size.
    pub fn resolve(self, backbuffer_width: u32, backbuffer_height: u32) -> (u32, u32) {
        match self {
            Self::Absolute { width, height } => (width, height),
            Self::Relative {
                width_scale,
                height_scale,
            } => {
                let w = ((backbuffer_width as f32) * width_scale).max(1.0) as u32;
                let h = ((backbuffer_height as f32) * height_scale).max(1.0) as u32;
                (w, h)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Resource descriptor
// ---------------------------------------------------------------------------

/// Full description of a GPU resource.
#[derive(Debug, Clone)]
pub struct ResourceDescriptor {
    pub name: String,
    pub size: SizePolicy,
    pub format: TextureFormat,
    pub mip_levels: u32,
    pub array_layers: u32,
    pub sample_count: u32,
    pub usages: Vec<UsageFlags>,
}

impl ResourceDescriptor {
    pub fn new(name: &str, format: TextureFormat) -> Self {
        Self {
            name: name.to_string(),
            size: SizePolicy::Relative {
                width_scale: 1.0,
                height_scale: 1.0,
            },
            format,
            mip_levels: 1,
            array_layers: 1,
            sample_count: 1,
            usages: vec![UsageFlags::RenderTarget, UsageFlags::ShaderRead],
        }
    }

    pub fn with_size(mut self, size: SizePolicy) -> Self {
        self.size = size;
        self
    }

    pub fn with_mip_levels(mut self, levels: u32) -> Self {
        self.mip_levels = levels;
        self
    }

    pub fn with_array_layers(mut self, layers: u32) -> Self {
        self.array_layers = layers;
        self
    }

    pub fn with_sample_count(mut self, count: u32) -> Self {
        self.sample_count = count;
        self
    }

    pub fn with_usages(mut self, usages: Vec<UsageFlags>) -> Self {
        self.usages = usages;
        self
    }

    /// Estimated byte size when resolved against a given backbuffer.
    pub fn estimated_bytes(&self, bb_w: u32, bb_h: u32) -> u64 {
        let (w, h) = self.size.resolve(bb_w, bb_h);
        let bpp = self.format.bytes_per_pixel() as u64;
        let base = (w as u64) * (h as u64) * bpp * (self.array_layers as u64) * (self.sample_count as u64);
        // Mip chain: sum of 1 + 1/4 + 1/16 + ... ~ 4/3 for full chain
        if self.mip_levels > 1 {
            let mut total: u64 = 0;
            let mut mw = w as u64;
            let mut mh = h as u64;
            for _ in 0..self.mip_levels {
                total += mw * mh * bpp * (self.array_layers as u64) * (self.sample_count as u64);
                mw = (mw / 2).max(1);
                mh = (mh / 2).max(1);
            }
            total
        } else {
            base
        }
    }

    /// True if two descriptors are memory-compatible (same size, format, sample count).
    pub fn is_compatible_with(&self, other: &ResourceDescriptor, bb_w: u32, bb_h: u32) -> bool {
        let (sw, sh) = self.size.resolve(bb_w, bb_h);
        let (ow, oh) = other.size.resolve(bb_w, bb_h);
        sw == ow
            && sh == oh
            && self.format == other.format
            && self.mip_levels == other.mip_levels
            && self.array_layers == other.array_layers
            && self.sample_count == other.sample_count
    }
}

// ---------------------------------------------------------------------------
// Resource handle / version
// ---------------------------------------------------------------------------

/// Opaque handle to a resource in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceHandle {
    pub index: u32,
    pub version: u32,
}

impl ResourceHandle {
    pub fn new(index: u32, version: u32) -> Self {
        Self { index, version }
    }

    pub fn next_version(self) -> Self {
        Self {
            index: self.index,
            version: self.version + 1,
        }
    }
}

impl fmt::Display for ResourceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Res({}v{})", self.index, self.version)
    }
}

// ---------------------------------------------------------------------------
// Resource slot (transient vs imported)
// ---------------------------------------------------------------------------

/// Whether a resource is managed by the graph or externally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLifetime {
    /// Created at first use and destroyed after last use each frame.
    Transient,
    /// Externally managed — never allocated/freed by the graph.
    Imported,
}

/// Runtime state of a concrete GPU allocation.
#[derive(Debug, Clone)]
pub struct PhysicalResource {
    pub id: u64,
    pub descriptor: ResourceDescriptor,
    pub lifetime: ResourceLifetime,
    /// Frame index at which this was last used.
    pub last_used_frame: u64,
    /// First pass index (topo-sorted) that writes to this resource this frame.
    pub first_write_pass: Option<usize>,
    /// Last pass index (topo-sorted) that reads from this resource this frame.
    pub last_read_pass: Option<usize>,
}

impl PhysicalResource {
    pub fn new(id: u64, descriptor: ResourceDescriptor, lifetime: ResourceLifetime) -> Self {
        Self {
            id,
            descriptor,
            lifetime,
            last_used_frame: 0,
            first_write_pass: None,
            last_read_pass: None,
        }
    }

    /// True if this resource's lifetime spans the given pass index.
    pub fn is_alive_at(&self, pass_idx: usize) -> bool {
        let first = self.first_write_pass.unwrap_or(usize::MAX);
        let last = self.last_read_pass.unwrap_or(0);
        pass_idx >= first && pass_idx <= last
    }
}

// ---------------------------------------------------------------------------
// Resource version tracking
// ---------------------------------------------------------------------------

/// Tracks all versions of a single logical resource within one frame,
/// enabling read-after-write hazard detection.
#[derive(Debug, Clone)]
pub struct ResourceVersionChain {
    pub handle: ResourceHandle,
    pub descriptor: ResourceDescriptor,
    pub lifetime: ResourceLifetime,
    /// Ordered list of (version, writer_pass_name).
    pub versions: Vec<(u32, String)>,
    /// Readers per version: version -> list of pass names.
    pub readers: HashMap<u32, Vec<String>>,
}

impl ResourceVersionChain {
    pub fn new(handle: ResourceHandle, descriptor: ResourceDescriptor, lifetime: ResourceLifetime) -> Self {
        Self {
            handle,
            descriptor,
            lifetime,
            versions: Vec::new(),
            readers: HashMap::new(),
        }
    }

    /// Record a write, bumping version.
    pub fn record_write(&mut self, pass_name: &str) -> u32 {
        let ver = if let Some((last, _)) = self.versions.last() {
            last + 1
        } else {
            0
        };
        self.versions.push((ver, pass_name.to_string()));
        ver
    }

    /// Record a read at a specific version.
    pub fn record_read(&mut self, version: u32, pass_name: &str) {
        self.readers
            .entry(version)
            .or_default()
            .push(pass_name.to_string());
    }

    /// The current (latest) version.
    pub fn current_version(&self) -> u32 {
        self.versions.last().map(|(v, _)| *v).unwrap_or(0)
    }

    /// Detect if a read-after-write hazard exists: a pass reads version N
    /// while another pass writes version N+1.
    pub fn has_raw_hazard(&self) -> bool {
        for (ver, _writer) in &self.versions {
            if *ver == 0 {
                continue;
            }
            let prev = ver - 1;
            if let Some(readers) = self.readers.get(&prev) {
                if !readers.is_empty() {
                    // There are readers of the previous version while a newer version exists.
                    // This is a potential RAW hazard that requires a barrier.
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Resource pool
// ---------------------------------------------------------------------------

/// Manages physical resources with frame-based lifetime tracking.
/// Reuses allocations between frames when descriptors match.
pub struct ResourcePool {
    resources: Vec<PhysicalResource>,
    next_id: u64,
    /// Map from resource name to pool index.
    name_map: HashMap<String, usize>,
    /// Resources that are free and can be reused.
    free_list: Vec<usize>,
    /// Current frame index.
    current_frame: u64,
    /// Maximum number of frames a resource can be unused before being freed.
    pub max_idle_frames: u64,
    /// Version chains for the current frame.
    version_chains: HashMap<u32, ResourceVersionChain>,
    /// Aliasing groups: sets of resource indices that share the same memory.
    alias_groups: Vec<Vec<usize>>,
}

impl ResourcePool {
    pub fn new() -> Self {
        Self {
            resources: Vec::new(),
            next_id: 1,
            name_map: HashMap::new(),
            free_list: Vec::new(),
            current_frame: 0,
            max_idle_frames: 3,
            version_chains: HashMap::new(),
            alias_groups: Vec::new(),
        }
    }

    /// Advance to the next frame. Frees resources that have been idle too long.
    pub fn begin_frame(&mut self) {
        self.current_frame += 1;
        self.version_chains.clear();
        self.alias_groups.clear();

        // Return all transient resources to free list
        let mut to_free = Vec::new();
        for (i, r) in self.resources.iter().enumerate() {
            if r.lifetime == ResourceLifetime::Transient {
                to_free.push(i);
            }
        }
        for idx in to_free {
            if !self.free_list.contains(&idx) {
                self.free_list.push(idx);
            }
        }

        // Evict resources idle for too long
        let max_idle = self.max_idle_frames;
        let cur = self.current_frame;
        let mut evicted = Vec::new();
        for &idx in &self.free_list {
            if cur.saturating_sub(self.resources[idx].last_used_frame) > max_idle {
                evicted.push(idx);
            }
        }
        for idx in &evicted {
            self.free_list.retain(|&i| i != *idx);
        }
        // Mark evicted slots as available (we don't actually shrink the vec)
        for idx in evicted {
            let name = self.resources[idx].descriptor.name.clone();
            self.name_map.remove(&name);
        }

        // Reset pass ranges on all living resources
        for r in &mut self.resources {
            r.first_write_pass = None;
            r.last_read_pass = None;
        }
    }

    /// End the current frame. Reports statistics.
    pub fn end_frame(&self) -> PoolFrameStats {
        let active = self.resources.len() - self.free_list.len();
        let free = self.free_list.len();
        let total_bytes: u64 = self
            .resources
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.free_list.contains(i))
            .map(|(_, r)| r.descriptor.estimated_bytes(1920, 1080))
            .sum();
        PoolFrameStats {
            active_resources: active,
            free_resources: free,
            total_resources: self.resources.len(),
            estimated_memory_bytes: total_bytes,
            alias_groups: self.alias_groups.len(),
        }
    }

    /// Acquire a resource matching the given descriptor. Reuses a free slot if possible.
    pub fn acquire(
        &mut self,
        descriptor: ResourceDescriptor,
        lifetime: ResourceLifetime,
        bb_w: u32,
        bb_h: u32,
    ) -> ResourceHandle {
        // Check for existing resource by name
        if let Some(&idx) = self.name_map.get(&descriptor.name) {
            self.resources[idx].last_used_frame = self.current_frame;
            self.free_list.retain(|&i| i != idx);
            return ResourceHandle::new(idx as u32, 0);
        }

        // Try to reuse a compatible free resource
        let compatible = self.free_list.iter().position(|&idx| {
            self.resources[idx]
                .descriptor
                .is_compatible_with(&descriptor, bb_w, bb_h)
        });

        if let Some(free_pos) = compatible {
            let idx = self.free_list.remove(free_pos);
            self.resources[idx].descriptor.name = descriptor.name.clone();
            self.resources[idx].last_used_frame = self.current_frame;
            self.name_map.insert(descriptor.name, idx);
            return ResourceHandle::new(idx as u32, 0);
        }

        // Allocate new
        let id = self.next_id;
        self.next_id += 1;
        let idx = self.resources.len();
        let name = descriptor.name.clone();
        self.resources
            .push(PhysicalResource::new(id, descriptor, lifetime));
        self.resources[idx].last_used_frame = self.current_frame;
        self.name_map.insert(name, idx);
        ResourceHandle::new(idx as u32, 0)
    }

    /// Release a resource back to the free list.
    pub fn release(&mut self, handle: ResourceHandle) {
        let idx = handle.index as usize;
        if idx < self.resources.len() && !self.free_list.contains(&idx) {
            self.free_list.push(idx);
        }
    }

    /// Mark a pass as writing to a resource.
    pub fn record_write(&mut self, handle: ResourceHandle, pass_idx: usize, pass_name: &str) {
        let idx = handle.index as usize;
        if idx < self.resources.len() {
            let r = &mut self.resources[idx];
            if r.first_write_pass.is_none() {
                r.first_write_pass = Some(pass_idx);
            }
            r.last_read_pass = Some(r.last_read_pass.map_or(pass_idx, |v| v.max(pass_idx)));
        }

        let chain = self
            .version_chains
            .entry(handle.index)
            .or_insert_with(|| {
                let desc = self.resources[idx].descriptor.clone();
                let lt = self.resources[idx].lifetime;
                ResourceVersionChain::new(handle, desc, lt)
            });
        chain.record_write(pass_name);
    }

    /// Mark a pass as reading from a resource.
    pub fn record_read(&mut self, handle: ResourceHandle, pass_idx: usize, _pass_name: &str) {
        let idx = handle.index as usize;
        if idx < self.resources.len() {
            let r = &mut self.resources[idx];
            r.last_read_pass = Some(r.last_read_pass.map_or(pass_idx, |v| v.max(pass_idx)));
        }

        if let Some(chain) = self.version_chains.get_mut(&handle.index) {
            let ver = chain.current_version();
            chain.record_read(ver, _pass_name);
        }
    }

    /// Get the descriptor for a handle.
    pub fn descriptor(&self, handle: ResourceHandle) -> Option<&ResourceDescriptor> {
        self.resources
            .get(handle.index as usize)
            .map(|r| &r.descriptor)
    }

    /// Get a physical resource by handle.
    pub fn physical(&self, handle: ResourceHandle) -> Option<&PhysicalResource> {
        self.resources.get(handle.index as usize)
    }

    /// Compute aliasing groups: resources whose lifetimes don't overlap can share memory.
    pub fn compute_aliasing(&mut self, total_passes: usize) -> &[Vec<usize>] {
        self.alias_groups.clear();

        let transient_indices: Vec<usize> = self
            .resources
            .iter()
            .enumerate()
            .filter(|(i, r)| r.lifetime == ResourceLifetime::Transient && !self.free_list.contains(i))
            .map(|(i, _)| i)
            .collect();

        // Greedy interval colouring
        let mut assigned: Vec<bool> = vec![false; transient_indices.len()];

        for (i, &idx_a) in transient_indices.iter().enumerate() {
            if assigned[i] {
                continue;
            }
            let mut group = vec![idx_a];
            assigned[i] = true;

            for (j, &idx_b) in transient_indices.iter().enumerate().skip(i + 1) {
                if assigned[j] {
                    continue;
                }
                // Check that idx_b doesn't overlap with anything in the group
                let overlaps = group.iter().any(|&g| {
                    self.lifetimes_overlap(g, idx_b, total_passes)
                });
                if !overlaps {
                    group.push(idx_b);
                    assigned[j] = true;
                }
            }

            if group.len() > 1 {
                self.alias_groups.push(group);
            }
        }

        &self.alias_groups
    }

    fn lifetimes_overlap(&self, a: usize, b: usize, _total: usize) -> bool {
        let ra = &self.resources[a];
        let rb = &self.resources[b];
        let a_start = ra.first_write_pass.unwrap_or(0);
        let a_end = ra.last_read_pass.unwrap_or(0);
        let b_start = rb.first_write_pass.unwrap_or(0);
        let b_end = rb.last_read_pass.unwrap_or(0);
        a_start <= b_end && b_start <= a_end
    }

    /// Check all version chains for read-after-write hazards.
    pub fn detect_raw_hazards(&self) -> Vec<(String, u32)> {
        let mut hazards = Vec::new();
        for (_, chain) in &self.version_chains {
            if chain.has_raw_hazard() {
                hazards.push((chain.descriptor.name.clone(), chain.handle.index));
            }
        }
        hazards
    }

    /// Estimate total memory budget for all active resources.
    pub fn estimate_memory_budget(&self, bb_w: u32, bb_h: u32) -> MemoryBudget {
        let mut total = 0u64;
        let mut transient = 0u64;
        let mut imported = 0u64;
        let mut peak = 0u64;

        // Per-pass memory high-water mark
        let max_pass = self
            .resources
            .iter()
            .filter_map(|r| r.last_read_pass)
            .max()
            .unwrap_or(0);

        for pass_idx in 0..=max_pass {
            let mut frame_mem = 0u64;
            for (i, r) in self.resources.iter().enumerate() {
                if self.free_list.contains(&i) {
                    continue;
                }
                if r.is_alive_at(pass_idx) {
                    frame_mem += r.descriptor.estimated_bytes(bb_w, bb_h);
                }
            }
            peak = peak.max(frame_mem);
        }

        for (i, r) in self.resources.iter().enumerate() {
            if self.free_list.contains(&i) {
                continue;
            }
            let bytes = r.descriptor.estimated_bytes(bb_w, bb_h);
            total += bytes;
            match r.lifetime {
                ResourceLifetime::Transient => transient += bytes,
                ResourceLifetime::Imported => imported += bytes,
            }
        }

        MemoryBudget {
            total_bytes: total,
            transient_bytes: transient,
            imported_bytes: imported,
            peak_frame_bytes: peak,
            resource_count: self.resources.len() - self.free_list.len(),
        }
    }

    /// Total number of resources (including free slots).
    pub fn total_slots(&self) -> usize {
        self.resources.len()
    }

    /// Number of active (non-free) resources.
    pub fn active_count(&self) -> usize {
        self.resources.len() - self.free_list.len()
    }

    /// Get the version chain for a resource handle.
    pub fn version_chain(&self, handle: ResourceHandle) -> Option<&ResourceVersionChain> {
        self.version_chains.get(&handle.index)
    }
}

impl Default for ResourcePool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Imported resource wrapper
// ---------------------------------------------------------------------------

/// An externally-managed resource brought into the render graph.
#[derive(Debug, Clone)]
pub struct ImportedResource {
    pub name: String,
    pub descriptor: ResourceDescriptor,
    /// External handle / ID for the actual GPU object.
    pub external_id: u64,
}

impl ImportedResource {
    pub fn new(name: &str, descriptor: ResourceDescriptor, external_id: u64) -> Self {
        Self {
            name: name.to_string(),
            descriptor,
            external_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Per-frame resource pool statistics.
#[derive(Debug, Clone)]
pub struct PoolFrameStats {
    pub active_resources: usize,
    pub free_resources: usize,
    pub total_resources: usize,
    pub estimated_memory_bytes: u64,
    pub alias_groups: usize,
}

impl fmt::Display for PoolFrameStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Pool: {} active, {} free, {} total, {:.2} MB, {} alias groups",
            self.active_resources,
            self.free_resources,
            self.total_resources,
            self.estimated_memory_bytes as f64 / (1024.0 * 1024.0),
            self.alias_groups,
        )
    }
}

/// Memory budget estimate.
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    pub total_bytes: u64,
    pub transient_bytes: u64,
    pub imported_bytes: u64,
    pub peak_frame_bytes: u64,
    pub resource_count: usize,
}

impl MemoryBudget {
    pub fn total_mb(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn peak_mb(&self) -> f64 {
        self.peak_frame_bytes as f64 / (1024.0 * 1024.0)
    }
}

impl fmt::Display for MemoryBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Budget: {:.2} MB total ({:.2} MB transient, {:.2} MB imported), peak {:.2} MB, {} resources",
            self.total_mb(),
            self.transient_bytes as f64 / (1024.0 * 1024.0),
            self.imported_bytes as f64 / (1024.0 * 1024.0),
            self.peak_mb(),
            self.resource_count,
        )
    }
}

// ---------------------------------------------------------------------------
// Transient resource helper
// ---------------------------------------------------------------------------

/// A transient resource that is created at first use and destroyed after last
/// use within a single frame. This is a convenience wrapper; the actual
/// lifetime management is performed by [`ResourcePool`].
#[derive(Debug, Clone)]
pub struct TransientResource {
    pub handle: ResourceHandle,
    pub descriptor: ResourceDescriptor,
}

impl TransientResource {
    pub fn new(handle: ResourceHandle, descriptor: ResourceDescriptor) -> Self {
        Self { handle, descriptor }
    }

    pub fn name(&self) -> &str {
        &self.descriptor.name
    }

    pub fn format(&self) -> TextureFormat {
        self.descriptor.format
    }

    pub fn estimated_bytes(&self, bb_w: u32, bb_h: u32) -> u64 {
        self.descriptor.estimated_bytes(bb_w, bb_h)
    }
}

// ---------------------------------------------------------------------------
// Resource table (used during graph building)
// ---------------------------------------------------------------------------

/// Bookkeeping structure used while constructing a render graph.
/// Maps logical resource names to handles and descriptors.
pub struct ResourceTable {
    entries: Vec<ResourceTableEntry>,
    name_to_index: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct ResourceTableEntry {
    pub name: String,
    pub descriptor: ResourceDescriptor,
    pub handle: ResourceHandle,
    pub lifetime: ResourceLifetime,
    /// Which passes write to this resource.
    pub writers: Vec<String>,
    /// Which passes read from this resource.
    pub readers: Vec<String>,
}

impl ResourceTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            name_to_index: HashMap::new(),
        }
    }

    /// Declare a transient resource.
    pub fn declare_transient(&mut self, descriptor: ResourceDescriptor) -> ResourceHandle {
        let name = descriptor.name.clone();
        if let Some(&idx) = self.name_to_index.get(&name) {
            return self.entries[idx].handle;
        }
        let idx = self.entries.len();
        let handle = ResourceHandle::new(idx as u32, 0);
        self.entries.push(ResourceTableEntry {
            name: name.clone(),
            descriptor,
            handle,
            lifetime: ResourceLifetime::Transient,
            writers: Vec::new(),
            readers: Vec::new(),
        });
        self.name_to_index.insert(name, idx);
        handle
    }

    /// Declare an imported resource.
    pub fn declare_imported(&mut self, descriptor: ResourceDescriptor) -> ResourceHandle {
        let name = descriptor.name.clone();
        if let Some(&idx) = self.name_to_index.get(&name) {
            return self.entries[idx].handle;
        }
        let idx = self.entries.len();
        let handle = ResourceHandle::new(idx as u32, 0);
        self.entries.push(ResourceTableEntry {
            name: name.clone(),
            descriptor,
            handle,
            lifetime: ResourceLifetime::Imported,
            writers: Vec::new(),
            readers: Vec::new(),
        });
        self.name_to_index.insert(name, idx);
        handle
    }

    /// Record that a pass writes to a resource.
    pub fn add_writer(&mut self, handle: ResourceHandle, pass_name: &str) {
        if let Some(entry) = self.entries.get_mut(handle.index as usize) {
            if !entry.writers.contains(&pass_name.to_string()) {
                entry.writers.push(pass_name.to_string());
            }
        }
    }

    /// Record that a pass reads from a resource.
    pub fn add_reader(&mut self, handle: ResourceHandle, pass_name: &str) {
        if let Some(entry) = self.entries.get_mut(handle.index as usize) {
            if !entry.readers.contains(&pass_name.to_string()) {
                entry.readers.push(pass_name.to_string());
            }
        }
    }

    /// Look up a resource by name.
    pub fn lookup(&self, name: &str) -> Option<ResourceHandle> {
        self.name_to_index
            .get(name)
            .map(|&idx| self.entries[idx].handle)
    }

    /// Get entry by handle.
    pub fn entry(&self, handle: ResourceHandle) -> Option<&ResourceTableEntry> {
        self.entries.get(handle.index as usize)
    }

    /// Iterate all entries.
    pub fn entries(&self) -> &[ResourceTableEntry] {
        &self.entries
    }

    /// Find dangling resources: declared but never written or never read.
    pub fn find_dangling(&self) -> Vec<DanglingResource> {
        let mut result = Vec::new();
        for entry in &self.entries {
            if entry.writers.is_empty() && entry.lifetime == ResourceLifetime::Transient {
                result.push(DanglingResource {
                    name: entry.name.clone(),
                    kind: DanglingKind::NeverWritten,
                });
            }
            if entry.readers.is_empty() && entry.lifetime == ResourceLifetime::Transient {
                result.push(DanglingResource {
                    name: entry.name.clone(),
                    kind: DanglingKind::NeverRead,
                });
            }
        }
        result
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ResourceTable {
    fn default() -> Self {
        Self::new()
    }
}

/// A resource that is improperly connected.
#[derive(Debug, Clone)]
pub struct DanglingResource {
    pub name: String,
    pub kind: DanglingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DanglingKind {
    NeverWritten,
    NeverRead,
}

impl fmt::Display for DanglingResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DanglingKind::NeverWritten => write!(f, "'{}' is never written", self.name),
            DanglingKind::NeverRead => write!(f, "'{}' is never read", self.name),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_desc(name: &str) -> ResourceDescriptor {
        ResourceDescriptor::new(name, TextureFormat::Rgba16Float)
    }

    #[test]
    fn test_pool_acquire_release() {
        let mut pool = ResourcePool::new();
        pool.begin_frame();
        let h = pool.acquire(make_desc("color"), ResourceLifetime::Transient, 1920, 1080);
        assert_eq!(h.index, 0);
        assert_eq!(pool.active_count(), 1);
        pool.release(h);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_pool_reuse() {
        let mut pool = ResourcePool::new();
        pool.begin_frame();
        let h1 = pool.acquire(make_desc("a"), ResourceLifetime::Transient, 1920, 1080);
        pool.release(h1);
        let h2 = pool.acquire(make_desc("b"), ResourceLifetime::Transient, 1920, 1080);
        // Should reuse the same slot
        assert_eq!(h1.index, h2.index);
    }

    #[test]
    fn test_memory_budget() {
        let mut pool = ResourcePool::new();
        pool.begin_frame();
        let _h = pool.acquire(make_desc("color"), ResourceLifetime::Transient, 1920, 1080);
        let budget = pool.estimate_memory_budget(1920, 1080);
        assert!(budget.total_bytes > 0);
        assert!(budget.transient_bytes > 0);
        assert_eq!(budget.imported_bytes, 0);
    }

    #[test]
    fn test_resource_table_dangling() {
        let mut table = ResourceTable::new();
        let h = table.declare_transient(make_desc("unused"));
        // Never written, never read
        let dangling = table.find_dangling();
        assert_eq!(dangling.len(), 2); // never written + never read
        // Now add a writer
        table.add_writer(h, "some_pass");
        let dangling = table.find_dangling();
        assert_eq!(dangling.len(), 1); // still never read
    }

    #[test]
    fn test_version_chain_hazard() {
        let handle = ResourceHandle::new(0, 0);
        let desc = make_desc("test");
        let mut chain = ResourceVersionChain::new(handle, desc, ResourceLifetime::Transient);
        let v0 = chain.record_write("pass_a");
        chain.record_read(v0, "pass_b");
        let _v1 = chain.record_write("pass_c");
        // pass_b reads v0, pass_c writes v1 -> RAW hazard
        assert!(chain.has_raw_hazard());
    }

    #[test]
    fn test_size_policy_resolve() {
        let abs = SizePolicy::Absolute {
            width: 512,
            height: 256,
        };
        assert_eq!(abs.resolve(1920, 1080), (512, 256));

        let rel = SizePolicy::Relative {
            width_scale: 0.5,
            height_scale: 0.25,
        };
        assert_eq!(rel.resolve(1920, 1080), (960, 270));
    }

    #[test]
    fn test_descriptor_estimated_bytes() {
        let desc = ResourceDescriptor::new("color", TextureFormat::Rgba16Float)
            .with_size(SizePolicy::Absolute {
                width: 1920,
                height: 1080,
            });
        let bytes = desc.estimated_bytes(1920, 1080);
        // 1920 * 1080 * 8 bytes = 16,588,800
        assert_eq!(bytes, 1920 * 1080 * 8);
    }

    #[test]
    fn test_descriptor_mip_chain_bytes() {
        let desc = ResourceDescriptor::new("color", TextureFormat::Rgba8Unorm)
            .with_size(SizePolicy::Absolute {
                width: 256,
                height: 256,
            })
            .with_mip_levels(3);
        let bytes = desc.estimated_bytes(256, 256);
        // mip0: 256*256*4 = 262144, mip1: 128*128*4 = 65536, mip2: 64*64*4 = 16384
        assert_eq!(bytes, 262144 + 65536 + 16384);
    }
}
