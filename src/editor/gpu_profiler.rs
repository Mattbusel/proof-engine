
//! GPU profiler — timestamp queries, pipeline statistics, memory tracking,
//! frame graph visualization, shader hot-spot analysis, and heat-map overlays.

use glam::Vec2;
use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// GPU capability flags
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct GpuCapabilities {
    pub vendor: String,
    pub device_name: String,
    pub driver_version: String,
    pub api_version: String,
    pub dedicated_vram_bytes: u64,
    pub shared_memory_bytes: u64,
    pub max_texture_size: u32,
    pub max_compute_invocations: u32,
    pub max_uniform_buffer_range: u32,
    pub max_storage_buffer_range: u64,
    pub supports_timestamp_queries: bool,
    pub supports_pipeline_statistics: bool,
    pub supports_mesh_shaders: bool,
    pub supports_raytracing: bool,
    pub supports_variable_rate_shading: bool,
    pub supports_conservative_rasterization: bool,
    pub supports_sparse_resources: bool,
    pub supports_descriptor_indexing: bool,
    pub max_draw_indirect_count: u32,
    pub subgroup_size: u32,
    pub max_compute_shared_memory: u32,
}

impl GpuCapabilities {
    pub fn mock_discrete() -> Self {
        Self {
            vendor: "NVIDIA".to_string(),
            device_name: "GeForce RTX 4080".to_string(),
            driver_version: "546.01".to_string(),
            api_version: "Vulkan 1.3".to_string(),
            dedicated_vram_bytes: 16 * 1024 * 1024 * 1024,
            shared_memory_bytes: 16 * 1024 * 1024 * 1024,
            max_texture_size: 32768,
            max_compute_invocations: 1024,
            max_uniform_buffer_range: 65536,
            max_storage_buffer_range: u64::MAX,
            supports_timestamp_queries: true,
            supports_pipeline_statistics: true,
            supports_mesh_shaders: true,
            supports_raytracing: true,
            supports_variable_rate_shading: true,
            supports_conservative_rasterization: true,
            supports_sparse_resources: true,
            supports_descriptor_indexing: true,
            max_draw_indirect_count: u32::MAX,
            subgroup_size: 32,
            max_compute_shared_memory: 49152,
        }
    }
}

// ---------------------------------------------------------------------------
// GPU memory categories
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuMemoryHeap {
    DeviceLocal,
    HostVisible,
    HostCoherent,
    HostCached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuResourceKind {
    VertexBuffer,
    IndexBuffer,
    UniformBuffer,
    StorageBuffer,
    Texture2D,
    Texture3D,
    TextureCube,
    TextureArray,
    RenderTarget,
    DepthStencil,
    AccelerationStructure,
    ScratchBuffer,
    UploadHeap,
    ReadbackHeap,
}

impl GpuResourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::VertexBuffer => "Vertex Buffer",
            Self::IndexBuffer => "Index Buffer",
            Self::UniformBuffer => "Uniform Buffer",
            Self::StorageBuffer => "Storage Buffer",
            Self::Texture2D => "Texture 2D",
            Self::Texture3D => "Texture 3D",
            Self::TextureCube => "Texture Cube",
            Self::TextureArray => "Texture Array",
            Self::RenderTarget => "Render Target",
            Self::DepthStencil => "Depth Stencil",
            Self::AccelerationStructure => "Acceleration Structure",
            Self::ScratchBuffer => "Scratch Buffer",
            Self::UploadHeap => "Upload Heap",
            Self::ReadbackHeap => "Readback Heap",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuAllocation {
    pub id: u64,
    pub name: String,
    pub kind: GpuResourceKind,
    pub heap: GpuMemoryHeap,
    pub size_bytes: u64,
    pub alignment: u64,
    pub offset: u64,
    pub alive: bool,
    pub frame_created: u64,
    pub frame_destroyed: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct GpuMemoryStats {
    pub device_local_used: u64,
    pub device_local_total: u64,
    pub host_visible_used: u64,
    pub host_visible_total: u64,
    pub by_kind: HashMap<GpuResourceKind, u64>,
    pub peak_device_local: u64,
    pub allocation_count: u32,
    pub free_count: u32,
}

impl GpuMemoryStats {
    pub fn device_local_pct(&self) -> f32 {
        if self.device_local_total == 0 { return 0.0; }
        self.device_local_used as f32 / self.device_local_total as f32
    }

    pub fn host_visible_pct(&self) -> f32 {
        if self.host_visible_total == 0 { return 0.0; }
        self.host_visible_used as f32 / self.host_visible_total as f32
    }

    pub fn total_used_mb(&self) -> f32 {
        (self.device_local_used + self.host_visible_used) as f32 / (1024.0 * 1024.0)
    }
}

// ---------------------------------------------------------------------------
// Pipeline statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    pub input_assembly_vertices: u64,
    pub input_assembly_primitives: u64,
    pub vertex_shader_invocations: u64,
    pub geometry_shader_invocations: u64,
    pub geometry_shader_primitives: u64,
    pub clipping_invocations: u64,
    pub clipping_primitives: u64,
    pub fragment_shader_invocations: u64,
    pub tessellation_control_shader_patches: u64,
    pub tessellation_eval_shader_invocations: u64,
    pub compute_shader_invocations: u64,
    pub mesh_shader_invocations: u64,
    pub task_shader_invocations: u64,
}

impl PipelineStats {
    pub fn overdraw_factor(&self) -> f32 {
        if self.clipping_primitives == 0 { return 0.0; }
        self.fragment_shader_invocations as f32 / (self.clipping_primitives as f32 * 4.0).max(1.0)
    }

    pub fn vertex_reuse_factor(&self) -> f32 {
        if self.vertex_shader_invocations == 0 { return 0.0; }
        self.input_assembly_vertices as f32 / self.vertex_shader_invocations as f32
    }
}

// ---------------------------------------------------------------------------
// Timestamp queries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TimestampQuery {
    pub name: String,
    pub begin_ns: u64,
    pub end_ns: u64,
    pub pipeline_stage: PipelineStage,
    pub color: u32,
}

impl TimestampQuery {
    pub fn duration_ms(&self) -> f32 {
        (self.end_ns - self.begin_ns) as f32 / 1_000_000.0
    }

    pub fn duration_us(&self) -> f32 {
        (self.end_ns - self.begin_ns) as f32 / 1_000.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineStage {
    TopOfPipe,
    DrawIndirect,
    VertexInput,
    VertexShader,
    TessellationControl,
    TessellationEval,
    GeometryShader,
    EarlyFragTest,
    FragmentShader,
    LateFragTest,
    ColorAttachmentOutput,
    ComputeShader,
    Transfer,
    BottomOfPipe,
    AllGraphics,
    AllCommands,
    AccelerationStructureBuild,
    RayTracing,
    MeshShader,
    TaskShader,
}

impl PipelineStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TopOfPipe => "Top of Pipe",
            Self::DrawIndirect => "Draw Indirect",
            Self::VertexInput => "Vertex Input",
            Self::VertexShader => "Vertex Shader",
            Self::TessellationControl => "Tessellation Control",
            Self::TessellationEval => "Tessellation Eval",
            Self::GeometryShader => "Geometry Shader",
            Self::EarlyFragTest => "Early Fragment Test",
            Self::FragmentShader => "Fragment Shader",
            Self::LateFragTest => "Late Fragment Test",
            Self::ColorAttachmentOutput => "Color Attachment Output",
            Self::ComputeShader => "Compute Shader",
            Self::Transfer => "Transfer",
            Self::BottomOfPipe => "Bottom of Pipe",
            Self::AllGraphics => "All Graphics",
            Self::AllCommands => "All Commands",
            Self::AccelerationStructureBuild => "AS Build",
            Self::RayTracing => "Ray Tracing",
            Self::MeshShader => "Mesh Shader",
            Self::TaskShader => "Task Shader",
        }
    }

    pub fn color_rgba(&self) -> u32 {
        match self {
            Self::VertexShader => 0xFF4080FF,
            Self::FragmentShader => 0xFF40FF80,
            Self::ComputeShader => 0xFFFF8040,
            Self::Transfer => 0xFFFFFF40,
            Self::RayTracing => 0xFFFF40FF,
            Self::AccelerationStructureBuild => 0xFF40FFFF,
            _ => 0xFF808080,
        }
    }
}

// ---------------------------------------------------------------------------
// Frame capture
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RenderPassCapture {
    pub name: String,
    pub begin_ns: u64,
    pub end_ns: u64,
    pub draw_calls: u32,
    pub dispatch_calls: u32,
    pub index_count: u64,
    pub vertex_count: u64,
    pub triangle_count: u64,
    pub render_targets: Vec<String>,
    pub depth_target: Option<String>,
    pub pipeline_stats: PipelineStats,
    pub sub_queries: Vec<TimestampQuery>,
    pub color: u32,
}

impl RenderPassCapture {
    pub fn duration_ms(&self) -> f32 {
        (self.end_ns - self.begin_ns) as f32 / 1_000_000.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct FrameCapture {
    pub frame_index: u64,
    pub begin_ns: u64,
    pub end_ns: u64,
    pub cpu_build_ms: f32,
    pub gpu_present_ms: f32,
    pub render_passes: Vec<RenderPassCapture>,
    pub memory_stats: GpuMemoryStats,
    pub draw_call_count: u32,
    pub dispatch_count: u32,
    pub triangle_count: u64,
    pub byte_uploaded: u64,
    pub byte_downloaded: u64,
    pub pipeline_cache_hits: u32,
    pub pipeline_cache_misses: u32,
    pub barrier_count: u32,
    pub layout_transition_count: u32,
}

impl FrameCapture {
    pub fn gpu_total_ms(&self) -> f32 {
        (self.end_ns - self.begin_ns) as f32 / 1_000_000.0
    }

    pub fn frame_time_ms(&self) -> f32 {
        self.gpu_total_ms() + self.cpu_build_ms
    }

    pub fn fps(&self) -> f32 {
        if self.frame_time_ms() <= 0.0 { return 0.0; }
        1000.0 / self.frame_time_ms()
    }

    pub fn bottleneck(&self) -> &'static str {
        let gpu_pct = self.gpu_total_ms() / self.frame_time_ms().max(0.001);
        if gpu_pct > 0.7 { "GPU-Bound" }
        else if gpu_pct < 0.3 { "CPU-Bound" }
        else { "Balanced" }
    }

    pub fn pass_by_name(&self, name: &str) -> Option<&RenderPassCapture> {
        self.render_passes.iter().find(|p| p.name == name)
    }

    pub fn top_passes_by_duration(&self, n: usize) -> Vec<&RenderPassCapture> {
        let mut sorted: Vec<&RenderPassCapture> = self.render_passes.iter().collect();
        sorted.sort_by(|a, b| b.duration_ms().partial_cmp(&a.duration_ms()).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).collect()
    }

    pub fn synthetic_frame(frame_index: u64) -> Self {
        let base_ns = frame_index * 16_666_666;
        let passes = vec![
            RenderPassCapture {
                name: "ShadowMap".to_string(),
                begin_ns: base_ns,
                end_ns: base_ns + 800_000,
                draw_calls: 42,
                dispatch_calls: 0,
                index_count: 120000,
                vertex_count: 80000,
                triangle_count: 40000,
                render_targets: vec![],
                depth_target: Some("shadow_depth".to_string()),
                pipeline_stats: PipelineStats {
                    vertex_shader_invocations: 80000,
                    fragment_shader_invocations: 320000,
                    ..Default::default()
                },
                sub_queries: vec![],
                color: 0xFF204080,
            },
            RenderPassCapture {
                name: "GBuffer".to_string(),
                begin_ns: base_ns + 900_000,
                end_ns: base_ns + 4_200_000,
                draw_calls: 186,
                dispatch_calls: 0,
                index_count: 1_200_000,
                vertex_count: 800_000,
                triangle_count: 400_000,
                render_targets: vec!["gbuffer_albedo".to_string(), "gbuffer_normal".to_string(), "gbuffer_material".to_string()],
                depth_target: Some("scene_depth".to_string()),
                pipeline_stats: PipelineStats {
                    vertex_shader_invocations: 800_000,
                    fragment_shader_invocations: 2_000_000,
                    ..Default::default()
                },
                sub_queries: vec![],
                color: 0xFF804020,
            },
            RenderPassCapture {
                name: "SSAO".to_string(),
                begin_ns: base_ns + 4_300_000,
                end_ns: base_ns + 5_100_000,
                draw_calls: 1,
                dispatch_calls: 1,
                index_count: 6,
                vertex_count: 4,
                triangle_count: 2,
                render_targets: vec!["ssao_result".to_string()],
                depth_target: None,
                pipeline_stats: PipelineStats {
                    compute_shader_invocations: 1_920 * 1_080,
                    ..Default::default()
                },
                sub_queries: vec![],
                color: 0xFF408040,
            },
            RenderPassCapture {
                name: "DeferredLighting".to_string(),
                begin_ns: base_ns + 5_200_000,
                end_ns: base_ns + 7_500_000,
                draw_calls: 1,
                dispatch_calls: 0,
                index_count: 6,
                vertex_count: 4,
                triangle_count: 2,
                render_targets: vec!["hdr_buffer".to_string()],
                depth_target: None,
                pipeline_stats: PipelineStats {
                    fragment_shader_invocations: 1_920 * 1_080,
                    ..Default::default()
                },
                sub_queries: vec![],
                color: 0xFFFF8040,
            },
            RenderPassCapture {
                name: "Bloom".to_string(),
                begin_ns: base_ns + 7_600_000,
                end_ns: base_ns + 8_900_000,
                draw_calls: 12,
                dispatch_calls: 0,
                index_count: 72,
                vertex_count: 48,
                triangle_count: 24,
                render_targets: vec!["bloom_buffer".to_string()],
                depth_target: None,
                pipeline_stats: PipelineStats {
                    fragment_shader_invocations: 1_920 * 1_080 * 4,
                    ..Default::default()
                },
                sub_queries: vec![],
                color: 0xFFFF4080,
            },
            RenderPassCapture {
                name: "TAA".to_string(),
                begin_ns: base_ns + 9_000_000,
                end_ns: base_ns + 10_200_000,
                draw_calls: 1,
                dispatch_calls: 0,
                index_count: 6,
                vertex_count: 4,
                triangle_count: 2,
                render_targets: vec!["taa_output".to_string()],
                depth_target: None,
                pipeline_stats: PipelineStats {
                    fragment_shader_invocations: 1_920 * 1_080,
                    ..Default::default()
                },
                sub_queries: vec![],
                color: 0xFF4080FF,
            },
            RenderPassCapture {
                name: "PostProcess".to_string(),
                begin_ns: base_ns + 10_300_000,
                end_ns: base_ns + 12_500_000,
                draw_calls: 8,
                dispatch_calls: 0,
                index_count: 48,
                vertex_count: 32,
                triangle_count: 16,
                render_targets: vec!["backbuffer".to_string()],
                depth_target: None,
                pipeline_stats: PipelineStats {
                    fragment_shader_invocations: 1_920 * 1_080 * 5,
                    ..Default::default()
                },
                sub_queries: vec![],
                color: 0xFF8040FF,
            },
        ];

        let total_triangles: u64 = passes.iter().map(|p| p.triangle_count).sum();
        let total_dc: u32 = passes.iter().map(|p| p.draw_calls).sum();

        Self {
            frame_index,
            begin_ns: base_ns,
            end_ns: base_ns + 12_666_666,
            cpu_build_ms: 3.2,
            gpu_present_ms: 0.4,
            render_passes: passes,
            memory_stats: GpuMemoryStats {
                device_local_used: 4_200 * 1024 * 1024,
                device_local_total: 16 * 1024 * 1024 * 1024,
                host_visible_used: 256 * 1024 * 1024,
                host_visible_total: 16 * 1024 * 1024 * 1024,
                by_kind: HashMap::new(),
                peak_device_local: 4_500 * 1024 * 1024,
                allocation_count: 1240,
                free_count: 48,
            },
            draw_call_count: total_dc,
            dispatch_count: 2,
            triangle_count: total_triangles,
            byte_uploaded: 2 * 1024 * 1024,
            byte_downloaded: 0,
            pipeline_cache_hits: 820,
            pipeline_cache_misses: 3,
            barrier_count: 24,
            layout_transition_count: 18,
        }
    }
}

// ---------------------------------------------------------------------------
// Shader compilation stats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShaderCompileStat {
    pub shader_path: String,
    pub stage: ShaderStage,
    pub permutation_key: u64,
    pub instruction_count: u32,
    pub sgpr_count: u32,
    pub vgpr_count: u32,
    pub scratch_bytes: u32,
    pub spill_bytes: u32,
    pub branch_count: u32,
    pub texture_fetch_count: u32,
    pub lds_usage_bytes: u32,
    pub wavefront_occupancy: f32,
    pub compile_ms: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
    Geometry,
    TessellationControl,
    TessellationEval,
    Task,
    Mesh,
    RayGeneration,
    ClosestHit,
    AnyHit,
    Miss,
    Intersection,
}

impl ShaderStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Vertex => "Vertex",
            Self::Fragment => "Fragment",
            Self::Compute => "Compute",
            Self::Geometry => "Geometry",
            Self::TessellationControl => "Tess. Control",
            Self::TessellationEval => "Tess. Eval",
            Self::Task => "Task",
            Self::Mesh => "Mesh",
            Self::RayGeneration => "RayGen",
            Self::ClosestHit => "ClosestHit",
            Self::AnyHit => "AnyHit",
            Self::Miss => "Miss",
            Self::Intersection => "Intersection",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Vertex => "vert",
            Self::Fragment => "frag",
            Self::Compute => "comp",
            Self::Geometry => "geom",
            Self::TessellationControl => "tesc",
            Self::TessellationEval => "tese",
            Self::Task => "task",
            Self::Mesh => "mesh",
            _ => "rgen",
        }
    }
}

// ---------------------------------------------------------------------------
// Performance counter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PerfCounter {
    pub name: String,
    pub category: String,
    pub value: f64,
    pub unit: String,
    pub description: String,
    pub higher_is_better: bool,
    pub warning_threshold: f64,
    pub critical_threshold: f64,
}

impl PerfCounter {
    pub fn status(&self) -> CounterStatus {
        if (!self.higher_is_better && self.value >= self.critical_threshold)
            || (self.higher_is_better && self.value <= self.critical_threshold)
        {
            CounterStatus::Critical
        } else if (!self.higher_is_better && self.value >= self.warning_threshold)
            || (self.higher_is_better && self.value <= self.warning_threshold)
        {
            CounterStatus::Warning
        } else {
            CounterStatus::Ok
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CounterStatus { Ok, Warning, Critical }

// ---------------------------------------------------------------------------
// Heatmap overlay data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeatmapMode {
    Overdraw,
    FragmentCost,
    DepthComplexity,
    LightingCost,
    ShadowCost,
    VertexDensity,
    TextureCacheMiss,
    MipVisualize,
}

impl HeatmapMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Overdraw => "Overdraw",
            Self::FragmentCost => "Fragment Cost",
            Self::DepthComplexity => "Depth Complexity",
            Self::LightingCost => "Lighting Cost",
            Self::ShadowCost => "Shadow Cost",
            Self::VertexDensity => "Vertex Density",
            Self::TextureCacheMiss => "Texture Cache Miss",
            Self::MipVisualize => "Mip Level",
        }
    }

    pub fn shader_define(&self) -> &'static str {
        match self {
            Self::Overdraw => "DEBUG_OVERDRAW",
            Self::FragmentCost => "DEBUG_FRAG_COST",
            Self::DepthComplexity => "DEBUG_DEPTH_COMPLEXITY",
            Self::LightingCost => "DEBUG_LIGHTING_COST",
            Self::ShadowCost => "DEBUG_SHADOW_COST",
            Self::VertexDensity => "DEBUG_VERTEX_DENSITY",
            Self::TextureCacheMiss => "DEBUG_TEXTURE_MISS",
            Self::MipVisualize => "DEBUG_MIP_LEVEL",
        }
    }
}

// ---------------------------------------------------------------------------
// Frame time graph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FrameTimeGraph {
    pub samples: VecDeque<f32>,
    pub capacity: usize,
    pub target_ms: f32,
    pub warning_ms: f32,
}

impl FrameTimeGraph {
    pub fn new(capacity: usize, target_ms: f32) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
            target_ms,
            warning_ms: target_ms * 1.5,
        }
    }

    pub fn push(&mut self, ms: f32) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(ms);
    }

    pub fn avg(&self) -> f32 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f32>() / self.samples.len() as f32
    }

    pub fn min(&self) -> f32 { self.samples.iter().cloned().fold(f32::MAX, f32::min) }
    pub fn max(&self) -> f32 { self.samples.iter().cloned().fold(f32::MIN, f32::max) }

    pub fn percentile_99(&self) -> f32 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted: Vec<f32> = self.samples.iter().cloned().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((sorted.len() as f32 * 0.99) as usize).min(sorted.len() - 1);
        sorted[idx]
    }

    pub fn fps(&self) -> f32 {
        let avg = self.avg();
        if avg <= 0.0 { 0.0 } else { 1000.0 / avg }
    }

    /// Normalize samples to [0, 1] range for graph display.
    pub fn normalized(&self, max_ms: f32) -> Vec<f32> {
        self.samples.iter().map(|&s| (s / max_ms).min(1.0)).collect()
    }
}

// ---------------------------------------------------------------------------
// Barrier and synchronization analysis
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BarrierRecord {
    pub frame_index: u64,
    pub src_stage: PipelineStage,
    pub dst_stage: PipelineStage,
    pub src_access: u32,
    pub dst_access: u32,
    pub layout_old: Option<String>,
    pub layout_new: Option<String>,
    pub resource_name: String,
    pub redundant: bool,
    pub stall_ns: u64,
}

impl BarrierRecord {
    pub fn stall_us(&self) -> f32 { self.stall_ns as f32 / 1_000.0 }
}

// ---------------------------------------------------------------------------
// Resource state tracker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ResourceState {
    Undefined,
    General,
    ColorAttachment,
    DepthWrite,
    DepthRead,
    ShaderReadOnly,
    TransferSrc,
    TransferDst,
    Present,
    ComputeReadWrite,
    AccelerationStructure,
}

impl ResourceState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Undefined => "Undefined",
            Self::General => "General",
            Self::ColorAttachment => "Color Attachment",
            Self::DepthWrite => "Depth Write",
            Self::DepthRead => "Depth Read",
            Self::ShaderReadOnly => "Shader Read-Only",
            Self::TransferSrc => "Transfer Src",
            Self::TransferDst => "Transfer Dst",
            Self::Present => "Present",
            Self::ComputeReadWrite => "Compute R/W",
            Self::AccelerationStructure => "Acceleration Structure",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceStateTracker {
    pub states: HashMap<String, ResourceState>,
    pub history: Vec<(String, ResourceState, ResourceState)>,
}

impl Default for ResourceStateTracker {
    fn default() -> Self {
        Self { states: HashMap::new(), history: Vec::new() }
    }
}

impl ResourceStateTracker {
    pub fn transition(&mut self, resource: &str, new_state: ResourceState) {
        let old = self.states.get(resource).cloned().unwrap_or(ResourceState::Undefined);
        if old != new_state {
            self.history.push((resource.to_string(), old.clone(), new_state.clone()));
            self.states.insert(resource.to_string(), new_state);
        }
    }

    pub fn current_state(&self, resource: &str) -> ResourceState {
        self.states.get(resource).cloned().unwrap_or(ResourceState::Undefined)
    }

    pub fn transitions_for(&self, resource: &str) -> Vec<(&ResourceState, &ResourceState)> {
        self.history.iter()
            .filter(|(r, _, _)| r == resource)
            .map(|(_, old, new)| (old, new))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// GPU profiler state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfilerPanel {
    FrameTimeline,
    MemoryBudget,
    PipelineStats,
    ShaderStats,
    BarrierAnalysis,
    Counters,
    Heatmap,
    Captures,
}

impl ProfilerPanel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FrameTimeline => "Frame Timeline",
            Self::MemoryBudget => "Memory",
            Self::PipelineStats => "Pipeline Stats",
            Self::ShaderStats => "Shader Stats",
            Self::BarrierAnalysis => "Barriers",
            Self::Counters => "Counters",
            Self::Heatmap => "Heatmap",
            Self::Captures => "Captures",
        }
    }
}

#[derive(Debug)]
pub struct GpuProfilerEditor {
    pub gpu_caps: GpuCapabilities,
    pub frame_captures: Vec<FrameCapture>,
    pub selected_frame: Option<usize>,
    pub selected_pass: Option<usize>,
    pub frame_time_graph: FrameTimeGraph,
    pub gpu_time_graph: FrameTimeGraph,
    pub allocations: Vec<GpuAllocation>,
    pub memory_stats: GpuMemoryStats,
    pub shader_stats: Vec<ShaderCompileStat>,
    pub perf_counters: Vec<PerfCounter>,
    pub barrier_records: Vec<BarrierRecord>,
    pub resource_tracker: ResourceStateTracker,
    pub active_panel: ProfilerPanel,
    pub capturing: bool,
    pub capture_frame_count: u32,
    pub pause_on_capture: bool,
    pub heatmap_mode: HeatmapMode,
    pub show_heatmap: bool,
    pub frame_index: u64,
    pub paused: bool,
    pub zoom_range: (f32, f32),
    pub timeline_scroll: f32,
    pub search_filter: String,
    pub expanded_passes: std::collections::HashSet<String>,
    pub show_redundant_barriers: bool,
    pub sort_shaders_by_instructions: bool,
}

impl Default for GpuProfilerEditor {
    fn default() -> Self {
        let mut captures: Vec<FrameCapture> = (0..32)
            .map(|i| FrameCapture::synthetic_frame(i))
            .collect();
        let frame_times: Vec<f32> = captures.iter().map(|f| f.gpu_total_ms()).collect();

        let mut ft_graph = FrameTimeGraph::new(256, 16.666);
        let mut gpu_graph = FrameTimeGraph::new(256, 16.666);
        for cap in &captures {
            ft_graph.push(cap.frame_time_ms());
            gpu_graph.push(cap.gpu_total_ms());
        }

        let mut memory_stats = GpuMemoryStats {
            device_local_used: 4_200 * 1024 * 1024,
            device_local_total: 16 * 1024 * 1024 * 1024,
            host_visible_used: 256 * 1024 * 1024,
            host_visible_total: 16 * 1024 * 1024 * 1024,
            peak_device_local: 4_500 * 1024 * 1024,
            allocation_count: 1240,
            free_count: 48,
            by_kind: HashMap::new(),
        };
        memory_stats.by_kind.insert(GpuResourceKind::Texture2D, 2_800 * 1024 * 1024);
        memory_stats.by_kind.insert(GpuResourceKind::RenderTarget, 512 * 1024 * 1024);
        memory_stats.by_kind.insert(GpuResourceKind::DepthStencil, 128 * 1024 * 1024);
        memory_stats.by_kind.insert(GpuResourceKind::VertexBuffer, 400 * 1024 * 1024);
        memory_stats.by_kind.insert(GpuResourceKind::IndexBuffer, 200 * 1024 * 1024);
        memory_stats.by_kind.insert(GpuResourceKind::StorageBuffer, 160 * 1024 * 1024);

        let shader_stats = vec![
            ShaderCompileStat {
                shader_path: "shaders/deferred_lighting.frag".to_string(),
                stage: ShaderStage::Fragment,
                permutation_key: 0xABCD1234,
                instruction_count: 1420,
                sgpr_count: 84,
                vgpr_count: 96,
                scratch_bytes: 0,
                spill_bytes: 0,
                branch_count: 42,
                texture_fetch_count: 12,
                lds_usage_bytes: 0,
                wavefront_occupancy: 0.5,
                compile_ms: 48.2,
            },
            ShaderCompileStat {
                shader_path: "shaders/gbuffer.vert".to_string(),
                stage: ShaderStage::Vertex,
                permutation_key: 0x00000001,
                instruction_count: 280,
                sgpr_count: 32,
                vgpr_count: 48,
                scratch_bytes: 0,
                spill_bytes: 0,
                branch_count: 6,
                texture_fetch_count: 0,
                lds_usage_bytes: 0,
                wavefront_occupancy: 0.75,
                compile_ms: 8.4,
            },
            ShaderCompileStat {
                shader_path: "shaders/ssao.comp".to_string(),
                stage: ShaderStage::Compute,
                permutation_key: 0x00000001,
                instruction_count: 640,
                sgpr_count: 48,
                vgpr_count: 64,
                scratch_bytes: 256,
                spill_bytes: 0,
                branch_count: 18,
                texture_fetch_count: 8,
                lds_usage_bytes: 4096,
                wavefront_occupancy: 0.625,
                compile_ms: 22.1,
            },
        ];

        let perf_counters = vec![
            PerfCounter {
                name: "GPU Utilization".to_string(),
                category: "General".to_string(),
                value: 78.4,
                unit: "%".to_string(),
                description: "Percentage of time the GPU is doing useful work".to_string(),
                higher_is_better: true,
                warning_threshold: 50.0,
                critical_threshold: 30.0,
            },
            PerfCounter {
                name: "Memory Controller Usage".to_string(),
                category: "Memory".to_string(),
                value: 62.1,
                unit: "%".to_string(),
                description: "Memory controller bus utilization".to_string(),
                higher_is_better: false,
                warning_threshold: 75.0,
                critical_threshold: 90.0,
            },
            PerfCounter {
                name: "L2 Cache Hit Rate".to_string(),
                category: "Cache".to_string(),
                value: 84.5,
                unit: "%".to_string(),
                description: "L2 cache hit percentage".to_string(),
                higher_is_better: true,
                warning_threshold: 60.0,
                critical_threshold: 40.0,
            },
            PerfCounter {
                name: "Texture Cache Miss Rate".to_string(),
                category: "Cache".to_string(),
                value: 5.2,
                unit: "%".to_string(),
                description: "Percentage of texture fetches that miss the cache".to_string(),
                higher_is_better: false,
                warning_threshold: 15.0,
                critical_threshold: 30.0,
            },
            PerfCounter {
                name: "Pipeline Stalls".to_string(),
                category: "Pipeline".to_string(),
                value: 2.8,
                unit: "%".to_string(),
                description: "Percentage of cycles wasted in pipeline stalls".to_string(),
                higher_is_better: false,
                warning_threshold: 10.0,
                critical_threshold: 20.0,
            },
            PerfCounter {
                name: "Shader Occupancy".to_string(),
                category: "Shader".to_string(),
                value: 62.5,
                unit: "%".to_string(),
                description: "Average shader occupancy across all dispatches".to_string(),
                higher_is_better: true,
                warning_threshold: 40.0,
                critical_threshold: 25.0,
            },
        ];

        Self {
            gpu_caps: GpuCapabilities::mock_discrete(),
            selected_frame: captures.last().map(|_| captures.len() - 1),
            frame_time_graph: ft_graph,
            gpu_time_graph: gpu_graph,
            frame_captures: captures,
            allocations: Vec::new(),
            memory_stats,
            shader_stats,
            perf_counters,
            barrier_records: Vec::new(),
            resource_tracker: ResourceStateTracker::default(),
            active_panel: ProfilerPanel::FrameTimeline,
            capturing: false,
            capture_frame_count: 1,
            pause_on_capture: true,
            heatmap_mode: HeatmapMode::Overdraw,
            show_heatmap: false,
            frame_index: 32,
            paused: false,
            zoom_range: (0.0, 16.666),
            timeline_scroll: 0.0,
            search_filter: String::new(),
            expanded_passes: std::collections::HashSet::new(),
            show_redundant_barriers: false,
            sort_shaders_by_instructions: true,
            selected_pass: None,
        }
    }
}

impl GpuProfilerEditor {
    pub fn selected_frame_capture(&self) -> Option<&FrameCapture> {
        self.selected_frame.and_then(|i| self.frame_captures.get(i))
    }

    pub fn push_frame(&mut self, cap: FrameCapture) {
        if self.paused { return; }
        self.frame_time_graph.push(cap.frame_time_ms());
        self.gpu_time_graph.push(cap.gpu_total_ms());
        self.frame_captures.push(cap);
        if self.frame_captures.len() > 256 {
            self.frame_captures.remove(0);
            if let Some(ref mut sel) = self.selected_frame {
                if *sel > 0 { *sel -= 1; }
            }
        }
        self.frame_index += 1;
    }

    pub fn begin_capture(&mut self) {
        self.capturing = true;
    }

    pub fn end_capture(&mut self) {
        self.capturing = false;
        let cap = FrameCapture::synthetic_frame(self.frame_index);
        let last = self.frame_captures.len().saturating_sub(1);
        self.selected_frame = Some(last);
        if self.pause_on_capture {
            self.paused = true;
        }
        self.push_frame(cap);
    }

    pub fn frame_avg_ms(&self) -> f32 {
        self.frame_time_graph.avg()
    }

    pub fn gpu_avg_ms(&self) -> f32 {
        self.gpu_time_graph.avg()
    }

    pub fn avg_fps(&self) -> f32 {
        self.frame_time_graph.fps()
    }

    pub fn memory_device_local_mb(&self) -> f32 {
        self.memory_stats.device_local_used as f32 / (1024.0 * 1024.0)
    }

    pub fn memory_device_local_total_mb(&self) -> f32 {
        self.memory_stats.device_local_total as f32 / (1024.0 * 1024.0)
    }

    pub fn vram_usage_pct(&self) -> f32 {
        self.memory_stats.device_local_pct()
    }

    pub fn top_shaders_by_instructions(&self, n: usize) -> Vec<&ShaderCompileStat> {
        let mut shaders: Vec<&ShaderCompileStat> = self.shader_stats.iter().collect();
        shaders.sort_by(|a, b| b.instruction_count.cmp(&a.instruction_count));
        shaders.into_iter().take(n).collect()
    }

    pub fn counters_by_status(&self, status: CounterStatus) -> Vec<&PerfCounter> {
        self.perf_counters.iter().filter(|c| c.status() == status).collect()
    }

    pub fn has_critical_counters(&self) -> bool {
        self.perf_counters.iter().any(|c| c.status() == CounterStatus::Critical)
    }

    pub fn has_warning_counters(&self) -> bool {
        self.perf_counters.iter().any(|c| c.status() == CounterStatus::Warning)
    }

    pub fn simulate_tick(&mut self, dt: f32) {
        if self.paused { return; }
        let cap = FrameCapture::synthetic_frame(self.frame_index);
        self.push_frame(cap);
    }

    pub fn pass_timeline_rects(&self, timeline_width: f32) -> Vec<(String, f32, f32, u32)> {
        let cap = match self.selected_frame_capture() {
            Some(c) => c,
            None => return Vec::new(),
        };
        let total_ns = (cap.end_ns - cap.begin_ns).max(1) as f32;
        cap.render_passes.iter().map(|p| {
            let x = (p.begin_ns - cap.begin_ns) as f32 / total_ns * timeline_width;
            let w = (p.end_ns - p.begin_ns) as f32 / total_ns * timeline_width;
            (p.name.clone(), x, w, p.color)
        }).collect()
    }

    pub fn timeline_zoom_in(&mut self) {
        let center = (self.zoom_range.0 + self.zoom_range.1) * 0.5;
        let half = (self.zoom_range.1 - self.zoom_range.0) * 0.4;
        self.zoom_range = ((center - half).max(0.0), (center + half).min(33.333));
    }

    pub fn timeline_zoom_out(&mut self) {
        let center = (self.zoom_range.0 + self.zoom_range.1) * 0.5;
        let half = (self.zoom_range.1 - self.zoom_range.0) * 0.625;
        self.zoom_range = ((center - half).max(0.0), (center + half).min(33.333));
    }

    pub fn toggle_pass_expand(&mut self, name: &str) {
        if self.expanded_passes.contains(name) {
            self.expanded_passes.remove(name);
        } else {
            self.expanded_passes.insert(name.to_string());
        }
    }

    pub fn filtered_passes(&self) -> Vec<&RenderPassCapture> {
        let cap = match self.selected_frame_capture() {
            Some(c) => c,
            None => return Vec::new(),
        };
        let q = self.search_filter.to_lowercase();
        if q.is_empty() {
            cap.render_passes.iter().collect()
        } else {
            cap.render_passes.iter().filter(|p| p.name.to_lowercase().contains(&q)).collect()
        }
    }

    pub fn generate_summary_report(&self) -> String {
        let mut lines = Vec::new();
        lines.push("=== GPU Profiler Summary ===".to_string());
        lines.push(format!("Device: {}", self.gpu_caps.device_name));
        lines.push(format!("Average Frame Time: {:.2} ms ({:.1} fps)", self.frame_avg_ms(), self.avg_fps()));
        lines.push(format!("Average GPU Time: {:.2} ms", self.gpu_avg_ms()));
        lines.push(format!("VRAM Usage: {:.0} MB / {:.0} MB ({:.1}%)",
            self.memory_device_local_mb(),
            self.memory_device_local_total_mb(),
            self.vram_usage_pct() * 100.0
        ));
        if let Some(cap) = self.selected_frame_capture() {
            lines.push(format!("Selected Frame #{}", cap.frame_index));
            lines.push(format!("  Bottleneck: {}", cap.bottleneck()));
            lines.push(format!("  Draw Calls: {}", cap.draw_call_count));
            lines.push(format!("  Triangles: {}", cap.triangle_count));
            lines.push(format!("  Render Passes: {}", cap.render_passes.len()));
            for pass in cap.top_passes_by_duration(5) {
                lines.push(format!("  - {}: {:.2} ms", pass.name, pass.duration_ms()));
            }
        }
        if self.has_critical_counters() {
            lines.push("CRITICAL: Some performance counters are in critical state!".to_string());
        }
        lines.join("\n")
    }
}
