//! Unified GPU abstraction layer: `GpuDevice` and `GpuQueue` traits that
//! provide a single API surface regardless of the underlying graphics backend.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use super::backend::{
    BufferHandle, BufferUsage, ComputePipelineHandle, PipelineHandle, PipelineLayout,
    ShaderHandle, ShaderStage, TextureFormat, TextureHandle,
};

// ---------------------------------------------------------------------------
// Blend / colour-target state
// ---------------------------------------------------------------------------

/// Blend factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendFactor {
    Zero,
    One,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
    SrcColor,
    OneMinusSrcColor,
    DstColor,
    OneMinusDstColor,
}

/// Blend operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendOp {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

/// Blend state for a single colour target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendState {
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
    pub operation: BlendOp,
}

impl BlendState {
    pub const ALPHA: BlendState = BlendState {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOp::Add,
    };

    pub const ADDITIVE: BlendState = BlendState {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::One,
        operation: BlendOp::Add,
    };

    pub const REPLACE: BlendState = BlendState {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
        operation: BlendOp::Add,
    };
}

/// State for a single colour output attachment.
#[derive(Debug, Clone)]
pub struct ColorTargetState {
    pub format: TextureFormat,
    pub blend: Option<BlendState>,
}

// ---------------------------------------------------------------------------
// Depth/stencil
// ---------------------------------------------------------------------------

/// Compare function used for depth/stencil testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareFunction {
    Never,
    Less,
    LessEqual,
    Equal,
    GreaterEqual,
    Greater,
    NotEqual,
    Always,
}

/// Depth-stencil state.
#[derive(Debug, Clone)]
pub struct DepthStencilState {
    pub format: TextureFormat,
    pub depth_write_enabled: bool,
    pub depth_compare: CompareFunction,
}

impl Default for DepthStencilState {
    fn default() -> Self {
        Self {
            format: TextureFormat::Depth32F,
            depth_write_enabled: true,
            depth_compare: CompareFunction::Less,
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline descriptors
// ---------------------------------------------------------------------------

/// Describes a render pipeline.
#[derive(Debug, Clone)]
pub struct RenderPipelineDesc {
    pub vertex: ShaderHandle,
    pub fragment: ShaderHandle,
    pub vertex_layout: Vec<VertexBufferLayout>,
    pub color_targets: Vec<ColorTargetState>,
    pub depth_stencil: Option<DepthStencilState>,
}

/// Layout of one vertex buffer.
#[derive(Debug, Clone)]
pub struct VertexBufferLayout {
    pub stride: u32,
    pub step_mode: StepMode,
    pub attributes: Vec<VertexAttr>,
}

/// Per-vertex or per-instance stepping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    Vertex,
    Instance,
}

/// A single vertex attribute descriptor.
#[derive(Debug, Clone)]
pub struct VertexAttr {
    pub location: u32,
    pub offset: u32,
    pub format: super::renderer::AttributeFormat,
}

/// Describes a compute pipeline.
#[derive(Debug, Clone)]
pub struct ComputePipelineDesc {
    pub compute_shader: ShaderHandle,
    pub bind_groups: Vec<BindGroup>,
}

// ---------------------------------------------------------------------------
// Bind groups
// ---------------------------------------------------------------------------

/// A concrete bind group with bound resources.
#[derive(Debug, Clone)]
pub struct BindGroup {
    pub entries: Vec<BindGroupEntry>,
}

impl BindGroup {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn push(mut self, entry: BindGroupEntry) -> Self {
        self.entries.push(entry);
        self
    }
}

impl Default for BindGroup {
    fn default() -> Self { Self::new() }
}

/// One entry in a bind group.
#[derive(Debug, Clone)]
pub struct BindGroupEntry {
    pub binding: u32,
    pub resource: BoundResource,
}

/// A resource bound at a specific slot.
#[derive(Debug, Clone)]
pub enum BoundResource {
    Buffer(BufferHandle),
    Texture(TextureHandle),
    Sampler(u64), // sampler handle
}

// ---------------------------------------------------------------------------
// GpuDevice trait
// ---------------------------------------------------------------------------

/// The unified GPU device trait.  Every concrete backend implements this.
pub trait GpuDevice: Send + Sync {
    // Resource creation
    fn create_buffer(&mut self, size: usize, usage: BufferUsage) -> BufferHandle;
    fn create_texture(&mut self, width: u32, height: u32, format: TextureFormat) -> TextureHandle;
    fn create_shader(&mut self, source: &str, stage: ShaderStage) -> ShaderHandle;
    fn create_pipeline(&mut self, desc: &RenderPipelineDesc) -> PipelineHandle;
    fn create_compute_pipeline(&mut self, desc: &ComputePipelineDesc) -> ComputePipelineHandle;

    // Data transfer
    fn write_buffer(&mut self, buffer: BufferHandle, offset: usize, data: &[u8]);
    fn read_buffer(&self, buffer: BufferHandle) -> Vec<u8>;
    fn write_texture(&mut self, texture: TextureHandle, data: &[u8]);

    // Render pass
    fn begin_render_pass(&mut self, color: &[TextureHandle], depth: Option<TextureHandle>);
    fn end_render_pass(&mut self);

    // Compute pass
    fn begin_compute_pass(&mut self);
    fn end_compute_pass(&mut self);

    // Draw commands (only valid between begin/end render pass)
    fn set_pipeline(&mut self, pipeline: PipelineHandle);
    fn set_vertex_buffer(&mut self, slot: u32, buffer: BufferHandle);
    fn set_index_buffer(&mut self, buffer: BufferHandle);
    fn set_bind_group(&mut self, index: u32, group: &BindGroup);
    fn draw(&mut self, vertex_count: u32, instance_count: u32);
    fn draw_indexed(&mut self, index_count: u32, instance_count: u32);
    fn draw_indirect(&mut self, buffer: BufferHandle, offset: u64);

    // Compute commands (only valid between begin/end compute pass)
    fn set_compute_pipeline(&mut self, pipeline: ComputePipelineHandle);
    fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32);
    fn dispatch_indirect(&mut self, buffer: BufferHandle, offset: u64);

    // Destruction
    fn destroy_buffer(&mut self, buffer: BufferHandle);
    fn destroy_texture(&mut self, texture: TextureHandle);

    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// GpuQueue trait
// ---------------------------------------------------------------------------

/// Command queue for submitting work to the GPU.
pub trait GpuQueue: Send + Sync {
    fn submit(&mut self, commands: Vec<RecordedCommand>);
    fn on_completion(&mut self, callback: Box<dyn FnOnce() + Send>);
}

/// A recorded GPU command (opaque).
#[derive(Debug, Clone)]
pub enum RecordedCommand {
    Draw { pipeline: PipelineHandle, vertex_count: u32, instance_count: u32 },
    DrawIndexed { pipeline: PipelineHandle, index_count: u32, instance_count: u32 },
    Dispatch { pipeline: ComputePipelineHandle, x: u32, y: u32, z: u32 },
    WriteBuffer { buffer: BufferHandle, data: Vec<u8> },
    CopyBuffer { src: BufferHandle, dst: BufferHandle, size: usize },
}

// ---------------------------------------------------------------------------
// NullDevice — no-op implementation for testing
// ---------------------------------------------------------------------------

/// A device that does nothing. Every call is a successful no-op.
pub struct NullDevice {
    next_id: AtomicU64,
}

impl NullDevice {
    pub fn new() -> Self {
        Self { next_id: AtomicU64::new(1) }
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for NullDevice {
    fn default() -> Self { Self::new() }
}

impl GpuDevice for NullDevice {
    fn create_buffer(&mut self, _size: usize, _usage: BufferUsage) -> BufferHandle {
        BufferHandle(self.next_id())
    }
    fn create_texture(&mut self, _w: u32, _h: u32, _fmt: TextureFormat) -> TextureHandle {
        TextureHandle(self.next_id())
    }
    fn create_shader(&mut self, _src: &str, _stage: ShaderStage) -> ShaderHandle {
        ShaderHandle(self.next_id())
    }
    fn create_pipeline(&mut self, _desc: &RenderPipelineDesc) -> PipelineHandle {
        PipelineHandle(self.next_id())
    }
    fn create_compute_pipeline(&mut self, _desc: &ComputePipelineDesc) -> ComputePipelineHandle {
        ComputePipelineHandle(self.next_id())
    }
    fn write_buffer(&mut self, _buf: BufferHandle, _off: usize, _data: &[u8]) {}
    fn read_buffer(&self, _buf: BufferHandle) -> Vec<u8> { Vec::new() }
    fn write_texture(&mut self, _tex: TextureHandle, _data: &[u8]) {}
    fn begin_render_pass(&mut self, _color: &[TextureHandle], _depth: Option<TextureHandle>) {}
    fn end_render_pass(&mut self) {}
    fn begin_compute_pass(&mut self) {}
    fn end_compute_pass(&mut self) {}
    fn set_pipeline(&mut self, _p: PipelineHandle) {}
    fn set_vertex_buffer(&mut self, _slot: u32, _buf: BufferHandle) {}
    fn set_index_buffer(&mut self, _buf: BufferHandle) {}
    fn set_bind_group(&mut self, _idx: u32, _grp: &BindGroup) {}
    fn draw(&mut self, _vc: u32, _ic: u32) {}
    fn draw_indexed(&mut self, _ic: u32, _inst: u32) {}
    fn draw_indirect(&mut self, _buf: BufferHandle, _off: u64) {}
    fn set_compute_pipeline(&mut self, _p: ComputePipelineHandle) {}
    fn dispatch_workgroups(&mut self, _x: u32, _y: u32, _z: u32) {}
    fn dispatch_indirect(&mut self, _buf: BufferHandle, _off: u64) {}
    fn destroy_buffer(&mut self, _buf: BufferHandle) {}
    fn destroy_texture(&mut self, _tex: TextureHandle) {}
    fn name(&self) -> &str { "Null" }
}

// ---------------------------------------------------------------------------
// SoftwareDevice — CPU fallback
// ---------------------------------------------------------------------------

struct SwBuffer {
    data: Vec<u8>,
    usage: BufferUsage,
}

struct SwTexture {
    data: Vec<u8>,
    width: u32,
    height: u32,
    format: TextureFormat,
}

/// CPU-backed device.
pub struct SoftwareDevice {
    next_id: u64,
    buffers: HashMap<u64, SwBuffer>,
    textures: HashMap<u64, SwTexture>,
    in_render_pass: bool,
    in_compute_pass: bool,
    current_pipeline: Option<PipelineHandle>,
    current_compute_pipeline: Option<ComputePipelineHandle>,
    current_vertex_buffers: HashMap<u32, BufferHandle>,
    current_index_buffer: Option<BufferHandle>,
    draw_log: Vec<RecordedCommand>,
}

impl SoftwareDevice {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            buffers: HashMap::new(),
            textures: HashMap::new(),
            in_render_pass: false,
            in_compute_pass: false,
            current_pipeline: None,
            current_compute_pipeline: None,
            current_vertex_buffers: HashMap::new(),
            current_index_buffer: None,
            draw_log: Vec::new(),
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Return all recorded commands since last drain.
    pub fn drain_log(&mut self) -> Vec<RecordedCommand> {
        std::mem::take(&mut self.draw_log)
    }
}

impl Default for SoftwareDevice {
    fn default() -> Self { Self::new() }
}

impl GpuDevice for SoftwareDevice {
    fn create_buffer(&mut self, size: usize, usage: BufferUsage) -> BufferHandle {
        let id = self.alloc_id();
        self.buffers.insert(id, SwBuffer { data: vec![0u8; size], usage });
        BufferHandle(id)
    }

    fn create_texture(&mut self, width: u32, height: u32, format: TextureFormat) -> TextureHandle {
        let id = self.alloc_id();
        let size = (width as usize) * (height as usize) * format.bytes_per_pixel();
        self.textures.insert(id, SwTexture { data: vec![0u8; size], width, height, format });
        TextureHandle(id)
    }

    fn create_shader(&mut self, _src: &str, _stage: ShaderStage) -> ShaderHandle {
        ShaderHandle(self.alloc_id())
    }

    fn create_pipeline(&mut self, _desc: &RenderPipelineDesc) -> PipelineHandle {
        PipelineHandle(self.alloc_id())
    }

    fn create_compute_pipeline(&mut self, _desc: &ComputePipelineDesc) -> ComputePipelineHandle {
        ComputePipelineHandle(self.alloc_id())
    }

    fn write_buffer(&mut self, buffer: BufferHandle, offset: usize, data: &[u8]) {
        if let Some(buf) = self.buffers.get_mut(&buffer.0) {
            let end = (offset + data.len()).min(buf.data.len());
            let len = end.saturating_sub(offset);
            if len > 0 {
                buf.data[offset..offset + len].copy_from_slice(&data[..len]);
            }
        }
    }

    fn read_buffer(&self, buffer: BufferHandle) -> Vec<u8> {
        self.buffers.get(&buffer.0).map(|b| b.data.clone()).unwrap_or_default()
    }

    fn write_texture(&mut self, texture: TextureHandle, data: &[u8]) {
        if let Some(tex) = self.textures.get_mut(&texture.0) {
            let len = data.len().min(tex.data.len());
            tex.data[..len].copy_from_slice(&data[..len]);
        }
    }

    fn begin_render_pass(&mut self, _color: &[TextureHandle], _depth: Option<TextureHandle>) {
        self.in_render_pass = true;
    }

    fn end_render_pass(&mut self) {
        self.in_render_pass = false;
        self.current_pipeline = None;
        self.current_vertex_buffers.clear();
        self.current_index_buffer = None;
    }

    fn begin_compute_pass(&mut self) {
        self.in_compute_pass = true;
    }

    fn end_compute_pass(&mut self) {
        self.in_compute_pass = false;
        self.current_compute_pipeline = None;
    }

    fn set_pipeline(&mut self, pipeline: PipelineHandle) {
        self.current_pipeline = Some(pipeline);
    }

    fn set_vertex_buffer(&mut self, slot: u32, buffer: BufferHandle) {
        self.current_vertex_buffers.insert(slot, buffer);
    }

    fn set_index_buffer(&mut self, buffer: BufferHandle) {
        self.current_index_buffer = Some(buffer);
    }

    fn set_bind_group(&mut self, _index: u32, _group: &BindGroup) {
        // In software: bind group state is noted but not used for actual rendering.
    }

    fn draw(&mut self, vertex_count: u32, instance_count: u32) {
        if let Some(pipe) = self.current_pipeline {
            self.draw_log.push(RecordedCommand::Draw {
                pipeline: pipe,
                vertex_count,
                instance_count,
            });
        }
    }

    fn draw_indexed(&mut self, index_count: u32, instance_count: u32) {
        if let Some(pipe) = self.current_pipeline {
            self.draw_log.push(RecordedCommand::DrawIndexed {
                pipeline: pipe,
                index_count,
                instance_count,
            });
        }
    }

    fn draw_indirect(&mut self, buffer: BufferHandle, _offset: u64) {
        // Read indirect args from buffer: vertex_count(u32), instance_count(u32)
        if let Some(buf) = self.buffers.get(&buffer.0) {
            if buf.data.len() >= 8 {
                let vc = u32::from_le_bytes([buf.data[0], buf.data[1], buf.data[2], buf.data[3]]);
                let ic = u32::from_le_bytes([buf.data[4], buf.data[5], buf.data[6], buf.data[7]]);
                self.draw(vc, ic);
            }
        }
    }

    fn set_compute_pipeline(&mut self, pipeline: ComputePipelineHandle) {
        self.current_compute_pipeline = Some(pipeline);
    }

    fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) {
        if let Some(pipe) = self.current_compute_pipeline {
            self.draw_log.push(RecordedCommand::Dispatch {
                pipeline: pipe,
                x, y, z,
            });
        }
    }

    fn dispatch_indirect(&mut self, buffer: BufferHandle, _offset: u64) {
        if let Some(buf) = self.buffers.get(&buffer.0) {
            if buf.data.len() >= 12 {
                let x = u32::from_le_bytes([buf.data[0], buf.data[1], buf.data[2], buf.data[3]]);
                let y = u32::from_le_bytes([buf.data[4], buf.data[5], buf.data[6], buf.data[7]]);
                let z = u32::from_le_bytes([buf.data[8], buf.data[9], buf.data[10], buf.data[11]]);
                self.dispatch_workgroups(x, y, z);
            }
        }
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
// OpenGLDevice — wraps glow (delegates to SoftwareDevice in headless)
// ---------------------------------------------------------------------------

/// OpenGL device implementation.  Without a live GL context this delegates
/// entirely to `SoftwareDevice`.
pub struct OpenGLDevice {
    inner: SoftwareDevice,
}

impl OpenGLDevice {
    pub fn new() -> Self {
        Self { inner: SoftwareDevice::new() }
    }
}

impl Default for OpenGLDevice {
    fn default() -> Self { Self::new() }
}

impl GpuDevice for OpenGLDevice {
    fn create_buffer(&mut self, size: usize, usage: BufferUsage) -> BufferHandle { self.inner.create_buffer(size, usage) }
    fn create_texture(&mut self, w: u32, h: u32, f: TextureFormat) -> TextureHandle { self.inner.create_texture(w, h, f) }
    fn create_shader(&mut self, s: &str, st: ShaderStage) -> ShaderHandle { self.inner.create_shader(s, st) }
    fn create_pipeline(&mut self, d: &RenderPipelineDesc) -> PipelineHandle { self.inner.create_pipeline(d) }
    fn create_compute_pipeline(&mut self, d: &ComputePipelineDesc) -> ComputePipelineHandle { self.inner.create_compute_pipeline(d) }
    fn write_buffer(&mut self, b: BufferHandle, o: usize, d: &[u8]) { self.inner.write_buffer(b, o, d) }
    fn read_buffer(&self, b: BufferHandle) -> Vec<u8> { self.inner.read_buffer(b) }
    fn write_texture(&mut self, t: TextureHandle, d: &[u8]) { self.inner.write_texture(t, d) }
    fn begin_render_pass(&mut self, c: &[TextureHandle], d: Option<TextureHandle>) { self.inner.begin_render_pass(c, d) }
    fn end_render_pass(&mut self) { self.inner.end_render_pass() }
    fn begin_compute_pass(&mut self) { self.inner.begin_compute_pass() }
    fn end_compute_pass(&mut self) { self.inner.end_compute_pass() }
    fn set_pipeline(&mut self, p: PipelineHandle) { self.inner.set_pipeline(p) }
    fn set_vertex_buffer(&mut self, s: u32, b: BufferHandle) { self.inner.set_vertex_buffer(s, b) }
    fn set_index_buffer(&mut self, b: BufferHandle) { self.inner.set_index_buffer(b) }
    fn set_bind_group(&mut self, i: u32, g: &BindGroup) { self.inner.set_bind_group(i, g) }
    fn draw(&mut self, v: u32, i: u32) { self.inner.draw(v, i) }
    fn draw_indexed(&mut self, i: u32, inst: u32) { self.inner.draw_indexed(i, inst) }
    fn draw_indirect(&mut self, b: BufferHandle, o: u64) { self.inner.draw_indirect(b, o) }
    fn set_compute_pipeline(&mut self, p: ComputePipelineHandle) { self.inner.set_compute_pipeline(p) }
    fn dispatch_workgroups(&mut self, x: u32, y: u32, z: u32) { self.inner.dispatch_workgroups(x, y, z) }
    fn dispatch_indirect(&mut self, b: BufferHandle, o: u64) { self.inner.dispatch_indirect(b, o) }
    fn destroy_buffer(&mut self, b: BufferHandle) { self.inner.destroy_buffer(b) }
    fn destroy_texture(&mut self, t: TextureHandle) { self.inner.destroy_texture(t) }
    fn name(&self) -> &str { "OpenGL" }
}

// ---------------------------------------------------------------------------
// SimpleQueue — basic command queue
// ---------------------------------------------------------------------------

/// A simple in-order queue that executes commands immediately.
pub struct SimpleQueue {
    pending_callbacks: Vec<Box<dyn FnOnce() + Send>>,
}

impl SimpleQueue {
    pub fn new() -> Self {
        Self { pending_callbacks: Vec::new() }
    }

    /// Flush all pending completion callbacks.
    pub fn flush(&mut self) {
        for cb in self.pending_callbacks.drain(..) {
            cb();
        }
    }
}

impl Default for SimpleQueue {
    fn default() -> Self { Self::new() }
}

impl GpuQueue for SimpleQueue {
    fn submit(&mut self, _commands: Vec<RecordedCommand>) {
        // In a real implementation this would encode and submit to the GPU.
        // Here we just note completion.
    }

    fn on_completion(&mut self, callback: Box<dyn FnOnce() + Send>) {
        self.pending_callbacks.push(callback);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::renderer::AttributeFormat;

    fn make_shader_handle(id: u64) -> ShaderHandle { ShaderHandle(id) }

    #[test]
    fn blend_state_presets() {
        assert_eq!(BlendState::ALPHA.src_factor, BlendFactor::SrcAlpha);
        assert_eq!(BlendState::ADDITIVE.operation, BlendOp::Add);
        assert_eq!(BlendState::REPLACE.dst_factor, BlendFactor::Zero);
    }

    #[test]
    fn null_device_creates_unique_handles() {
        let mut dev = NullDevice::new();
        let a = dev.create_buffer(64, BufferUsage::VERTEX);
        let b = dev.create_buffer(64, BufferUsage::VERTEX);
        assert_ne!(a, b);
    }

    #[test]
    fn null_device_all_operations() {
        let mut dev = NullDevice::new();
        let buf = dev.create_buffer(16, BufferUsage::UNIFORM);
        dev.write_buffer(buf, 0, &[1, 2, 3]);
        assert!(dev.read_buffer(buf).is_empty()); // Null device returns nothing
        let tex = dev.create_texture(4, 4, TextureFormat::RGBA8);
        dev.write_texture(tex, &[0; 64]);

        dev.begin_render_pass(&[tex], None);
        let vs = dev.create_shader("v", ShaderStage::Vertex);
        let fs = dev.create_shader("f", ShaderStage::Fragment);
        let pipe = dev.create_pipeline(&RenderPipelineDesc {
            vertex: vs,
            fragment: fs,
            vertex_layout: Vec::new(),
            color_targets: Vec::new(),
            depth_stencil: None,
        });
        dev.set_pipeline(pipe);
        dev.set_vertex_buffer(0, buf);
        dev.set_index_buffer(buf);
        dev.set_bind_group(0, &BindGroup::new());
        dev.draw(3, 1);
        dev.draw_indexed(6, 1);
        dev.draw_indirect(buf, 0);
        dev.end_render_pass();

        dev.begin_compute_pass();
        let cp = dev.create_compute_pipeline(&ComputePipelineDesc {
            compute_shader: vs,
            bind_groups: Vec::new(),
        });
        dev.set_compute_pipeline(cp);
        dev.dispatch_workgroups(4, 1, 1);
        dev.dispatch_indirect(buf, 0);
        dev.end_compute_pass();

        dev.destroy_buffer(buf);
        dev.destroy_texture(tex);
        assert_eq!(dev.name(), "Null");
    }

    #[test]
    fn software_device_buffer_write_read() {
        let mut dev = SoftwareDevice::new();
        let buf = dev.create_buffer(16, BufferUsage::STORAGE);
        dev.write_buffer(buf, 0, &[10, 20, 30, 40]);
        let data = dev.read_buffer(buf);
        assert_eq!(&data[..4], &[10, 20, 30, 40]);
        assert_eq!(data.len(), 16);
    }

    #[test]
    fn software_device_buffer_write_with_offset() {
        let mut dev = SoftwareDevice::new();
        let buf = dev.create_buffer(8, BufferUsage::STORAGE);
        dev.write_buffer(buf, 4, &[0xAA, 0xBB, 0xCC, 0xDD]);
        let data = dev.read_buffer(buf);
        assert_eq!(&data[4..8], &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn software_device_texture() {
        let mut dev = SoftwareDevice::new();
        let tex = dev.create_texture(2, 2, TextureFormat::RGBA8);
        let pixels = vec![255u8; 16];
        dev.write_texture(tex, &pixels);
    }

    #[test]
    fn software_device_render_pass() {
        let mut dev = SoftwareDevice::new();
        let vs = dev.create_shader("v", ShaderStage::Vertex);
        let fs = dev.create_shader("f", ShaderStage::Fragment);
        let pipe = dev.create_pipeline(&RenderPipelineDesc {
            vertex: vs,
            fragment: fs,
            vertex_layout: Vec::new(),
            color_targets: vec![ColorTargetState {
                format: TextureFormat::RGBA8,
                blend: Some(BlendState::ALPHA),
            }],
            depth_stencil: None,
        });
        let vbuf = dev.create_buffer(48, BufferUsage::VERTEX);
        let tex = dev.create_texture(800, 600, TextureFormat::RGBA8);

        dev.begin_render_pass(&[tex], None);
        dev.set_pipeline(pipe);
        dev.set_vertex_buffer(0, vbuf);
        dev.draw(3, 1);
        dev.end_render_pass();

        let log = dev.drain_log();
        assert_eq!(log.len(), 1);
        assert!(matches!(log[0], RecordedCommand::Draw { vertex_count: 3, instance_count: 1, .. }));
    }

    #[test]
    fn software_device_compute_pass() {
        let mut dev = SoftwareDevice::new();
        let cs = dev.create_shader("c", ShaderStage::Compute);
        let cp = dev.create_compute_pipeline(&ComputePipelineDesc {
            compute_shader: cs,
            bind_groups: Vec::new(),
        });

        dev.begin_compute_pass();
        dev.set_compute_pipeline(cp);
        dev.dispatch_workgroups(4, 2, 1);
        dev.end_compute_pass();

        let log = dev.drain_log();
        assert_eq!(log.len(), 1);
        assert!(matches!(log[0], RecordedCommand::Dispatch { x: 4, y: 2, z: 1, .. }));
    }

    #[test]
    fn software_device_draw_indirect() {
        let mut dev = SoftwareDevice::new();
        let vs = dev.create_shader("v", ShaderStage::Vertex);
        let fs = dev.create_shader("f", ShaderStage::Fragment);
        let pipe = dev.create_pipeline(&RenderPipelineDesc {
            vertex: vs, fragment: fs,
            vertex_layout: Vec::new(), color_targets: Vec::new(), depth_stencil: None,
        });

        // Indirect args: vertex_count=6, instance_count=2
        let mut args = Vec::new();
        args.extend_from_slice(&6u32.to_le_bytes());
        args.extend_from_slice(&2u32.to_le_bytes());
        let buf = dev.create_buffer(8, BufferUsage::INDIRECT);
        dev.write_buffer(buf, 0, &args);

        dev.begin_render_pass(&[], None);
        dev.set_pipeline(pipe);
        dev.draw_indirect(buf, 0);
        dev.end_render_pass();

        let log = dev.drain_log();
        assert_eq!(log.len(), 1);
        assert!(matches!(log[0], RecordedCommand::Draw { vertex_count: 6, instance_count: 2, .. }));
    }

    #[test]
    fn software_device_dispatch_indirect() {
        let mut dev = SoftwareDevice::new();
        let cs = dev.create_shader("c", ShaderStage::Compute);
        let cp = dev.create_compute_pipeline(&ComputePipelineDesc {
            compute_shader: cs, bind_groups: Vec::new(),
        });

        let mut args = Vec::new();
        args.extend_from_slice(&8u32.to_le_bytes());
        args.extend_from_slice(&4u32.to_le_bytes());
        args.extend_from_slice(&1u32.to_le_bytes());
        let buf = dev.create_buffer(12, BufferUsage::INDIRECT);
        dev.write_buffer(buf, 0, &args);

        dev.begin_compute_pass();
        dev.set_compute_pipeline(cp);
        dev.dispatch_indirect(buf, 0);
        dev.end_compute_pass();

        let log = dev.drain_log();
        assert_eq!(log.len(), 1);
        assert!(matches!(log[0], RecordedCommand::Dispatch { x: 8, y: 4, z: 1, .. }));
    }

    #[test]
    fn software_device_destroy() {
        let mut dev = SoftwareDevice::new();
        let buf = dev.create_buffer(8, BufferUsage::VERTEX);
        dev.destroy_buffer(buf);
        assert!(dev.read_buffer(buf).is_empty());
    }

    #[test]
    fn opengl_device_delegates() {
        let mut dev = OpenGLDevice::new();
        assert_eq!(dev.name(), "OpenGL");
        let buf = dev.create_buffer(8, BufferUsage::UNIFORM);
        dev.write_buffer(buf, 0, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(dev.read_buffer(buf), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn bind_group_builder() {
        let bg = BindGroup::new()
            .push(BindGroupEntry { binding: 0, resource: BoundResource::Buffer(BufferHandle(1)) })
            .push(BindGroupEntry { binding: 1, resource: BoundResource::Texture(TextureHandle(2)) });
        assert_eq!(bg.entries.len(), 2);
    }

    #[test]
    fn simple_queue_flush() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let mut queue = SimpleQueue::new();
        let called = Arc::new(AtomicBool::new(false));
        let called2 = called.clone();
        queue.submit(Vec::new());
        queue.on_completion(Box::new(move || {
            called2.store(true, Ordering::SeqCst);
        }));
        queue.flush();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn depth_stencil_default() {
        let ds = DepthStencilState::default();
        assert!(ds.depth_write_enabled);
        assert_eq!(ds.depth_compare, CompareFunction::Less);
        assert_eq!(ds.format, TextureFormat::Depth32F);
    }

    #[test]
    fn color_target_with_blend() {
        let ct = ColorTargetState {
            format: TextureFormat::RGBA8,
            blend: Some(BlendState::ALPHA),
        };
        assert_eq!(ct.format, TextureFormat::RGBA8);
        assert_eq!(ct.blend.unwrap().src_factor, BlendFactor::SrcAlpha);
    }
}
