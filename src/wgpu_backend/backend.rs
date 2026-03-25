//! Core GPU backend abstraction: enums, handle types, capability queries,
//! and the `BackendContext` trait that every concrete backend implements.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::fmt;

// ---------------------------------------------------------------------------
// Handle types
// ---------------------------------------------------------------------------

/// Opaque handle to a GPU buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle(pub u64);

/// Opaque handle to a GPU texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(pub u64);

/// Opaque handle to a compiled shader module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderHandle(pub u64);

/// Opaque handle to a render pipeline (vertex + fragment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineHandle(pub u64);

/// Opaque handle to a compute pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComputePipelineHandle(pub u64);

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Which GPU API is in use (or Software for CPU fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuBackend {
    OpenGL,
    Vulkan,
    Metal,
    WebGPU,
    Software,
}

impl fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenGL  => write!(f, "OpenGL"),
            Self::Vulkan  => write!(f, "Vulkan"),
            Self::Metal   => write!(f, "Metal"),
            Self::WebGPU  => write!(f, "WebGPU"),
            Self::Software => write!(f, "Software"),
        }
    }
}

/// Buffer usage flags (combinable via bitwise OR on the underlying bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferUsage(pub u32);

impl BufferUsage {
    pub const VERTEX:   BufferUsage = BufferUsage(1 << 0);
    pub const INDEX:    BufferUsage = BufferUsage(1 << 1);
    pub const UNIFORM:  BufferUsage = BufferUsage(1 << 2);
    pub const STORAGE:  BufferUsage = BufferUsage(1 << 3);
    pub const INDIRECT: BufferUsage = BufferUsage(1 << 4);
    pub const COPY_SRC: BufferUsage = BufferUsage(1 << 5);
    pub const COPY_DST: BufferUsage = BufferUsage(1 << 6);

    pub fn contains(self, other: BufferUsage) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for BufferUsage {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { BufferUsage(self.0 | rhs.0) }
}

impl std::ops::BitAnd for BufferUsage {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self { BufferUsage(self.0 & rhs.0) }
}

/// Texture format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    R8,
    RGBA8,
    RGBA16F,
    RGBA32F,
    Depth24,
    Depth32F,
}

impl TextureFormat {
    /// Bytes per pixel for this format.
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::R8       => 1,
            Self::RGBA8    => 4,
            Self::RGBA16F  => 8,
            Self::RGBA32F  => 16,
            Self::Depth24  => 3,
            Self::Depth32F => 4,
        }
    }

    /// Whether the format is a depth format.
    pub fn is_depth(&self) -> bool {
        matches!(self, Self::Depth24 | Self::Depth32F)
    }
}

/// Shader stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

// ---------------------------------------------------------------------------
// Capabilities
// ---------------------------------------------------------------------------

/// Capabilities of the current backend.
#[derive(Debug, Clone)]
pub struct BackendCapabilities {
    pub compute_shaders: bool,
    pub max_texture_size: u32,
    pub max_ssbo_size: u64,
    pub max_workgroup_size: [u32; 3],
    pub indirect_draw: bool,
    pub multi_draw_indirect: bool,
}

impl Default for BackendCapabilities {
    fn default() -> Self {
        Self {
            compute_shaders: false,
            max_texture_size: 4096,
            max_ssbo_size: 128 * 1024 * 1024,
            max_workgroup_size: [256, 256, 64],
            indirect_draw: false,
            multi_draw_indirect: false,
        }
    }
}

impl BackendCapabilities {
    /// Build capabilities for a known backend.
    pub fn for_backend(backend: GpuBackend) -> Self {
        match backend {
            GpuBackend::Vulkan => Self {
                compute_shaders: true,
                max_texture_size: 16384,
                max_ssbo_size: 2 * 1024 * 1024 * 1024,
                max_workgroup_size: [1024, 1024, 64],
                indirect_draw: true,
                multi_draw_indirect: true,
            },
            GpuBackend::Metal => Self {
                compute_shaders: true,
                max_texture_size: 16384,
                max_ssbo_size: 1024 * 1024 * 1024,
                max_workgroup_size: [1024, 1024, 64],
                indirect_draw: true,
                multi_draw_indirect: true,
            },
            GpuBackend::WebGPU => Self {
                compute_shaders: true,
                max_texture_size: 8192,
                max_ssbo_size: 256 * 1024 * 1024,
                max_workgroup_size: [256, 256, 64],
                indirect_draw: true,
                multi_draw_indirect: false,
            },
            GpuBackend::OpenGL => Self {
                compute_shaders: true,
                max_texture_size: 8192,
                max_ssbo_size: 128 * 1024 * 1024,
                max_workgroup_size: [512, 512, 64],
                indirect_draw: true,
                multi_draw_indirect: true,
            },
            GpuBackend::Software => Self {
                compute_shaders: true,
                max_texture_size: 4096,
                max_ssbo_size: 512 * 1024 * 1024,
                max_workgroup_size: [256, 256, 64],
                indirect_draw: false,
                multi_draw_indirect: false,
            },
        }
    }

    /// Check whether a given workgroup size fits within the backend limits.
    pub fn workgroup_fits(&self, x: u32, y: u32, z: u32) -> bool {
        x <= self.max_workgroup_size[0]
            && y <= self.max_workgroup_size[1]
            && z <= self.max_workgroup_size[2]
    }
}

// ---------------------------------------------------------------------------
// Detect backend
// ---------------------------------------------------------------------------

/// Detect the best available GPU backend on this platform.
pub fn detect_backend() -> GpuBackend {
    // In a real engine this would probe the system for available APIs.
    // We use compile-time cfg for a reasonable default.
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        GpuBackend::Metal
    } else if cfg!(target_os = "windows") {
        // Windows: prefer Vulkan, fall back to OpenGL.
        GpuBackend::Vulkan
    } else if cfg!(target_os = "linux") {
        GpuBackend::Vulkan
    } else if cfg!(target_arch = "wasm32") {
        GpuBackend::WebGPU
    } else {
        GpuBackend::Software
    }
}

// ---------------------------------------------------------------------------
// Command types
// ---------------------------------------------------------------------------

/// A GPU command that can be recorded and submitted.
#[derive(Debug, Clone)]
pub enum GpuCommand {
    CopyBufferToBuffer {
        src: BufferHandle,
        dst: BufferHandle,
        size: usize,
    },
    CopyBufferToTexture {
        src: BufferHandle,
        dst: TextureHandle,
        width: u32,
        height: u32,
    },
    Draw {
        pipeline: PipelineHandle,
        vertex_buffer: BufferHandle,
        vertex_count: u32,
        instance_count: u32,
    },
    DrawIndexed {
        pipeline: PipelineHandle,
        vertex_buffer: BufferHandle,
        index_buffer: BufferHandle,
        index_count: u32,
        instance_count: u32,
    },
    Dispatch {
        pipeline: ComputePipelineHandle,
        x: u32,
        y: u32,
        z: u32,
    },
    SetBindGroup {
        index: u32,
        buffers: Vec<BufferHandle>,
    },
    Barrier,
}

/// Pipeline layout description.
#[derive(Debug, Clone, Default)]
pub struct PipelineLayout {
    pub bind_group_layouts: Vec<BindGroupLayoutDesc>,
}

/// Describes one bind-group layout.
#[derive(Debug, Clone)]
pub struct BindGroupLayoutDesc {
    pub entries: Vec<BindGroupLayoutEntry>,
}

/// One entry within a bind-group layout.
#[derive(Debug, Clone)]
pub struct BindGroupLayoutEntry {
    pub binding: u32,
    pub visibility: ShaderStage,
    pub ty: BindingType,
}

/// Type of a single binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingType {
    UniformBuffer,
    StorageBuffer,
    Texture,
    Sampler,
}

// ---------------------------------------------------------------------------
// BackendContext trait
// ---------------------------------------------------------------------------

/// Trait that every concrete GPU backend must implement.
pub trait BackendContext: Send + Sync {
    /// Create a GPU buffer and return its handle.
    fn create_buffer(&mut self, size: usize, usage: BufferUsage) -> BufferHandle;

    /// Create a 2-D texture.
    fn create_texture(&mut self, width: u32, height: u32, format: TextureFormat) -> TextureHandle;

    /// Compile / upload a shader module.
    fn create_shader(&mut self, source: &str, stage: ShaderStage) -> ShaderHandle;

    /// Create a render pipeline (vertex + fragment).
    fn create_pipeline(
        &mut self,
        vertex: ShaderHandle,
        fragment: ShaderHandle,
        layout: &PipelineLayout,
    ) -> PipelineHandle;

    /// Create a compute pipeline.
    fn create_compute_pipeline(
        &mut self,
        shader: ShaderHandle,
        layout: &PipelineLayout,
    ) -> ComputePipelineHandle;

    /// Submit a batch of GPU commands.
    fn submit(&mut self, commands: &[GpuCommand]);

    /// Present the current frame to screen.
    fn present(&mut self);

    /// Write raw data into a buffer.
    fn write_buffer(&mut self, buffer: BufferHandle, data: &[u8]);

    /// Read raw data back from a buffer.
    fn read_buffer(&self, buffer: BufferHandle) -> Vec<u8>;

    /// Write raw pixel data into a texture.
    fn write_texture(&mut self, texture: TextureHandle, data: &[u8]);

    /// Read raw pixel data from a texture.
    fn read_texture(&self, texture: TextureHandle) -> Vec<u8>;

    /// Destroy a buffer, freeing its resources.
    fn destroy_buffer(&mut self, buffer: BufferHandle);

    /// Destroy a texture, freeing its resources.
    fn destroy_texture(&mut self, texture: TextureHandle);

    /// Name of this backend, for logging.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Handle counter (shared)
// ---------------------------------------------------------------------------

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

fn next_handle() -> u64 {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// SoftwareBuffer / SoftwareTexture (used by both Software and OpenGL stubs)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SoftwareBuffer {
    data: Vec<u8>,
    usage: BufferUsage,
}

#[derive(Debug, Clone)]
struct SoftwareTexture {
    data: Vec<u8>,
    width: u32,
    height: u32,
    format: TextureFormat,
}

#[derive(Debug, Clone)]
struct SoftwareShader {
    source: String,
    stage: ShaderStage,
}

// ---------------------------------------------------------------------------
// SoftwareContext — pure-CPU fallback
// ---------------------------------------------------------------------------

/// Pure-CPU backend that stores everything in RAM.  Useful for tests, CI,
/// headless rendering, and platforms without any GPU API.
pub struct SoftwareContext {
    buffers: HashMap<u64, SoftwareBuffer>,
    textures: HashMap<u64, SoftwareTexture>,
    shaders: HashMap<u64, SoftwareShader>,
    command_log: Vec<GpuCommand>,
}

impl SoftwareContext {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            textures: HashMap::new(),
            shaders: HashMap::new(),
            command_log: Vec::new(),
        }
    }

    /// Number of recorded (but not yet cleared) commands.
    pub fn command_count(&self) -> usize {
        self.command_log.len()
    }

    /// Drain the recorded commands for inspection.
    pub fn drain_commands(&mut self) -> Vec<GpuCommand> {
        std::mem::take(&mut self.command_log)
    }
}

impl Default for SoftwareContext {
    fn default() -> Self { Self::new() }
}

impl BackendContext for SoftwareContext {
    fn create_buffer(&mut self, size: usize, usage: BufferUsage) -> BufferHandle {
        let id = next_handle();
        self.buffers.insert(id, SoftwareBuffer {
            data: vec![0u8; size],
            usage,
        });
        BufferHandle(id)
    }

    fn create_texture(&mut self, width: u32, height: u32, format: TextureFormat) -> TextureHandle {
        let id = next_handle();
        let byte_size = (width as usize) * (height as usize) * format.bytes_per_pixel();
        self.textures.insert(id, SoftwareTexture {
            data: vec![0u8; byte_size],
            width,
            height,
            format,
        });
        TextureHandle(id)
    }

    fn create_shader(&mut self, source: &str, stage: ShaderStage) -> ShaderHandle {
        let id = next_handle();
        self.shaders.insert(id, SoftwareShader {
            source: source.to_string(),
            stage,
        });
        ShaderHandle(id)
    }

    fn create_pipeline(
        &mut self,
        _vertex: ShaderHandle,
        _fragment: ShaderHandle,
        _layout: &PipelineLayout,
    ) -> PipelineHandle {
        PipelineHandle(next_handle())
    }

    fn create_compute_pipeline(
        &mut self,
        _shader: ShaderHandle,
        _layout: &PipelineLayout,
    ) -> ComputePipelineHandle {
        ComputePipelineHandle(next_handle())
    }

    fn submit(&mut self, commands: &[GpuCommand]) {
        for cmd in commands {
            match cmd {
                GpuCommand::CopyBufferToBuffer { src, dst, size } => {
                    let src_data = self.buffers.get(&src.0)
                        .map(|b| b.data[..*size].to_vec())
                        .unwrap_or_default();
                    if let Some(dst_buf) = self.buffers.get_mut(&dst.0) {
                        let len = src_data.len().min(dst_buf.data.len());
                        dst_buf.data[..len].copy_from_slice(&src_data[..len]);
                    }
                }
                GpuCommand::CopyBufferToTexture { src, dst, width, height } => {
                    let src_data = self.buffers.get(&src.0)
                        .map(|b| b.data.clone())
                        .unwrap_or_default();
                    if let Some(tex) = self.textures.get_mut(&dst.0) {
                        let len = src_data.len().min(tex.data.len());
                        tex.data[..len].copy_from_slice(&src_data[..len]);
                    }
                }
                _ => { /* Draw/Dispatch are no-ops in software for now */ }
            }
            self.command_log.push(cmd.clone());
        }
    }

    fn present(&mut self) {
        // Software context: no-op present.
    }

    fn write_buffer(&mut self, buffer: BufferHandle, data: &[u8]) {
        if let Some(buf) = self.buffers.get_mut(&buffer.0) {
            let len = data.len().min(buf.data.len());
            buf.data[..len].copy_from_slice(&data[..len]);
        }
    }

    fn read_buffer(&self, buffer: BufferHandle) -> Vec<u8> {
        self.buffers.get(&buffer.0)
            .map(|b| b.data.clone())
            .unwrap_or_default()
    }

    fn write_texture(&mut self, texture: TextureHandle, data: &[u8]) {
        if let Some(tex) = self.textures.get_mut(&texture.0) {
            let len = data.len().min(tex.data.len());
            tex.data[..len].copy_from_slice(&data[..len]);
        }
    }

    fn read_texture(&self, texture: TextureHandle) -> Vec<u8> {
        self.textures.get(&texture.0)
            .map(|t| t.data.clone())
            .unwrap_or_default()
    }

    fn destroy_buffer(&mut self, buffer: BufferHandle) {
        self.buffers.remove(&buffer.0);
    }

    fn destroy_texture(&mut self, texture: TextureHandle) {
        self.textures.remove(&texture.0);
    }

    fn name(&self) -> &str { "Software" }
}

// ---------------------------------------------------------------------------
// OpenGLContext — wraps glow (stub-level: real GL calls happen in pipeline.rs)
// ---------------------------------------------------------------------------

/// OpenGL backend context. In a headless / test environment this behaves
/// identically to [`SoftwareContext`] since we don't have a live GL context.
/// When a real GL context is available (via the engine pipeline), the handles
/// map to actual GL object names.
pub struct OpenGLContext {
    inner: SoftwareContext,
    /// Optional reference to a live glow context.  When `None` this is pure
    /// software emulation.
    gl: Option<()>, // placeholder — real code would store Arc<glow::Context>
}

impl OpenGLContext {
    /// Create a new OpenGL context.  Pass `true` for `has_gl` if a real GL
    /// context is current on this thread (we just store a flag here; the
    /// actual `glow::Context` would be threaded through in production).
    pub fn new(has_gl: bool) -> Self {
        Self {
            inner: SoftwareContext::new(),
            gl: if has_gl { Some(()) } else { None },
        }
    }

    /// Whether a live GL context is attached.
    pub fn has_gl(&self) -> bool { self.gl.is_some() }
}

impl Default for OpenGLContext {
    fn default() -> Self { Self::new(false) }
}

impl BackendContext for OpenGLContext {
    fn create_buffer(&mut self, size: usize, usage: BufferUsage) -> BufferHandle {
        // In production with a live GL: glGenBuffers + glBufferData.
        self.inner.create_buffer(size, usage)
    }

    fn create_texture(&mut self, width: u32, height: u32, format: TextureFormat) -> TextureHandle {
        self.inner.create_texture(width, height, format)
    }

    fn create_shader(&mut self, source: &str, stage: ShaderStage) -> ShaderHandle {
        self.inner.create_shader(source, stage)
    }

    fn create_pipeline(
        &mut self,
        vertex: ShaderHandle,
        fragment: ShaderHandle,
        layout: &PipelineLayout,
    ) -> PipelineHandle {
        self.inner.create_pipeline(vertex, fragment, layout)
    }

    fn create_compute_pipeline(
        &mut self,
        shader: ShaderHandle,
        layout: &PipelineLayout,
    ) -> ComputePipelineHandle {
        self.inner.create_compute_pipeline(shader, layout)
    }

    fn submit(&mut self, commands: &[GpuCommand]) {
        self.inner.submit(commands);
    }

    fn present(&mut self) {
        // In production: eglSwapBuffers / wglSwapBuffers.
        self.inner.present();
    }

    fn write_buffer(&mut self, buffer: BufferHandle, data: &[u8]) {
        self.inner.write_buffer(buffer, data);
    }

    fn read_buffer(&self, buffer: BufferHandle) -> Vec<u8> {
        self.inner.read_buffer(buffer)
    }

    fn write_texture(&mut self, texture: TextureHandle, data: &[u8]) {
        self.inner.write_texture(texture, data);
    }

    fn read_texture(&self, texture: TextureHandle) -> Vec<u8> {
        self.inner.read_texture(texture)
    }

    fn destroy_buffer(&mut self, buffer: BufferHandle) {
        self.inner.destroy_buffer(buffer);
    }

    fn destroy_texture(&mut self, texture: TextureHandle) {
        self.inner.destroy_texture(texture);
    }

    fn name(&self) -> &str { "OpenGL" }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_usage_flags_combine() {
        let usage = BufferUsage::VERTEX | BufferUsage::COPY_DST;
        assert!(usage.contains(BufferUsage::VERTEX));
        assert!(usage.contains(BufferUsage::COPY_DST));
        assert!(!usage.contains(BufferUsage::INDEX));
    }

    #[test]
    fn texture_format_bytes() {
        assert_eq!(TextureFormat::R8.bytes_per_pixel(), 1);
        assert_eq!(TextureFormat::RGBA8.bytes_per_pixel(), 4);
        assert_eq!(TextureFormat::RGBA32F.bytes_per_pixel(), 16);
        assert!(TextureFormat::Depth32F.is_depth());
        assert!(!TextureFormat::RGBA8.is_depth());
    }

    #[test]
    fn backend_display() {
        assert_eq!(format!("{}", GpuBackend::Vulkan), "Vulkan");
        assert_eq!(format!("{}", GpuBackend::Software), "Software");
    }

    #[test]
    fn capabilities_for_backend() {
        let caps = BackendCapabilities::for_backend(GpuBackend::Vulkan);
        assert!(caps.compute_shaders);
        assert_eq!(caps.max_texture_size, 16384);
        assert!(caps.multi_draw_indirect);

        let sw = BackendCapabilities::for_backend(GpuBackend::Software);
        assert!(!sw.indirect_draw);
    }

    #[test]
    fn workgroup_fits() {
        let caps = BackendCapabilities::for_backend(GpuBackend::Vulkan);
        assert!(caps.workgroup_fits(1024, 1, 1));
        assert!(!caps.workgroup_fits(2048, 1, 1));
    }

    #[test]
    fn detect_backend_is_deterministic() {
        let a = detect_backend();
        let b = detect_backend();
        assert_eq!(a, b);
    }

    #[test]
    fn software_context_create_buffer() {
        let mut ctx = SoftwareContext::new();
        let buf = ctx.create_buffer(128, BufferUsage::VERTEX);
        assert_ne!(buf.0, 0);
        let data = ctx.read_buffer(buf);
        assert_eq!(data.len(), 128);
        assert!(data.iter().all(|&b| b == 0));
    }

    #[test]
    fn software_context_write_read_buffer() {
        let mut ctx = SoftwareContext::new();
        let buf = ctx.create_buffer(8, BufferUsage::STORAGE);
        ctx.write_buffer(buf, &[1, 2, 3, 4, 5, 6, 7, 8]);
        let out = ctx.read_buffer(buf);
        assert_eq!(out, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn software_context_texture() {
        let mut ctx = SoftwareContext::new();
        let tex = ctx.create_texture(2, 2, TextureFormat::RGBA8);
        // 2x2 RGBA8 = 16 bytes
        let data = ctx.read_texture(tex);
        assert_eq!(data.len(), 16);

        let pixels = vec![255u8; 16];
        ctx.write_texture(tex, &pixels);
        assert_eq!(ctx.read_texture(tex), pixels);
    }

    #[test]
    fn software_context_shader_and_pipeline() {
        let mut ctx = SoftwareContext::new();
        let vs = ctx.create_shader("void main(){}", ShaderStage::Vertex);
        let fs = ctx.create_shader("void main(){}", ShaderStage::Fragment);
        let layout = PipelineLayout::default();
        let pipe = ctx.create_pipeline(vs, fs, &layout);
        assert_ne!(pipe.0, 0);
    }

    #[test]
    fn software_context_compute_pipeline() {
        let mut ctx = SoftwareContext::new();
        let cs = ctx.create_shader("void main(){}", ShaderStage::Compute);
        let layout = PipelineLayout::default();
        let cp = ctx.create_compute_pipeline(cs, &layout);
        assert_ne!(cp.0, 0);
    }

    #[test]
    fn software_context_submit_copy() {
        let mut ctx = SoftwareContext::new();
        let src = ctx.create_buffer(4, BufferUsage::COPY_SRC);
        let dst = ctx.create_buffer(4, BufferUsage::COPY_DST);
        ctx.write_buffer(src, &[10, 20, 30, 40]);
        ctx.submit(&[GpuCommand::CopyBufferToBuffer {
            src,
            dst,
            size: 4,
        }]);
        assert_eq!(ctx.read_buffer(dst), vec![10, 20, 30, 40]);
        assert_eq!(ctx.command_count(), 1);
    }

    #[test]
    fn software_context_destroy() {
        let mut ctx = SoftwareContext::new();
        let buf = ctx.create_buffer(8, BufferUsage::VERTEX);
        ctx.destroy_buffer(buf);
        assert!(ctx.read_buffer(buf).is_empty());
    }

    #[test]
    fn opengl_context_delegates() {
        let mut ctx = OpenGLContext::new(false);
        assert!(!ctx.has_gl());
        assert_eq!(ctx.name(), "OpenGL");
        let buf = ctx.create_buffer(16, BufferUsage::UNIFORM);
        ctx.write_buffer(buf, &[0xAA; 16]);
        assert_eq!(ctx.read_buffer(buf), vec![0xAA; 16]);
    }

    #[test]
    fn pipeline_layout_default_empty() {
        let layout = PipelineLayout::default();
        assert!(layout.bind_group_layouts.is_empty());
    }

    #[test]
    fn binding_type_equality() {
        assert_eq!(BindingType::UniformBuffer, BindingType::UniformBuffer);
        assert_ne!(BindingType::Texture, BindingType::Sampler);
    }

    #[test]
    fn gpu_command_clone() {
        let cmd = GpuCommand::Barrier;
        let _cmd2 = cmd.clone();
    }

    #[test]
    fn handles_are_unique() {
        let mut ctx = SoftwareContext::new();
        let a = ctx.create_buffer(1, BufferUsage::VERTEX);
        let b = ctx.create_buffer(1, BufferUsage::VERTEX);
        assert_ne!(a, b);
    }

    #[test]
    fn write_buffer_truncates_to_size() {
        let mut ctx = SoftwareContext::new();
        let buf = ctx.create_buffer(4, BufferUsage::STORAGE);
        // Write more than the buffer can hold — should truncate
        ctx.write_buffer(buf, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(ctx.read_buffer(buf), vec![1, 2, 3, 4]);
    }
}
