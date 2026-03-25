//! Multi-backend renderer: render passes, draw calls, compute dispatch,
//! buffer upload / readback, and vertex layout descriptions.

use super::backend::{
    BackendCapabilities, BackendContext, BufferHandle, BufferUsage, ComputePipelineHandle,
    GpuBackend, GpuCommand, PipelineHandle, PipelineLayout, ShaderHandle, ShaderStage,
    SoftwareContext, TextureFormat, TextureHandle,
};
use glam::{Vec3, Vec4};

// ---------------------------------------------------------------------------
// Vertex layout
// ---------------------------------------------------------------------------

/// Describes one vertex attribute within a vertex layout.
#[derive(Debug, Clone)]
pub struct VertexAttribute {
    pub location: u32,
    pub format: AttributeFormat,
    pub offset: u32,
}

/// Format of a single vertex attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeFormat {
    Float,
    Float2,
    Float3,
    Float4,
    UInt,
    Int,
}

impl AttributeFormat {
    /// Size in bytes of this attribute format.
    pub fn byte_size(&self) -> u32 {
        match self {
            Self::Float  => 4,
            Self::Float2 => 8,
            Self::Float3 => 12,
            Self::Float4 => 16,
            Self::UInt   => 4,
            Self::Int    => 4,
        }
    }
}

/// A vertex layout describing how vertex data is organized.
#[derive(Debug, Clone)]
pub struct VertexLayout {
    pub attributes: Vec<VertexAttribute>,
}

impl VertexLayout {
    pub fn new() -> Self {
        Self { attributes: Vec::new() }
    }

    pub fn push(mut self, location: u32, format: AttributeFormat, offset: u32) -> Self {
        self.attributes.push(VertexAttribute { location, format, offset });
        self
    }

    /// Total stride in bytes (sum of attribute sizes, or max offset+size).
    pub fn stride(&self) -> u32 {
        self.attributes.iter().map(|a| a.offset + a.format.byte_size()).max().unwrap_or(0)
    }
}

impl Default for VertexLayout {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Uniform block
// ---------------------------------------------------------------------------

/// A block of uniform data to be bound at a specific binding index.
#[derive(Debug, Clone)]
pub struct UniformBlock {
    pub data: Vec<u8>,
    pub binding: u32,
}

impl UniformBlock {
    pub fn new(binding: u32, data: Vec<u8>) -> Self {
        Self { data, binding }
    }

    pub fn from_f32_slice(binding: u32, values: &[f32]) -> Self {
        let data: Vec<u8> = values.iter().flat_map(|f| f.to_le_bytes()).collect();
        Self { data, binding }
    }
}

// ---------------------------------------------------------------------------
// Render pass
// ---------------------------------------------------------------------------

/// Describes a render pass with colour and depth attachments.
#[derive(Debug, Clone)]
pub struct RenderPass {
    pub color_attachments: Vec<TextureHandle>,
    pub depth_attachment: Option<TextureHandle>,
    pub clear_color: [f32; 4],
}

impl RenderPass {
    pub fn new() -> Self {
        Self {
            color_attachments: Vec::new(),
            depth_attachment: None,
            clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn with_color(mut self, tex: TextureHandle) -> Self {
        self.color_attachments.push(tex);
        self
    }

    pub fn with_depth(mut self, tex: TextureHandle) -> Self {
        self.depth_attachment = Some(tex);
        self
    }

    pub fn with_clear(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.clear_color = [r, g, b, a];
        self
    }
}

impl Default for RenderPass {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Draw call
// ---------------------------------------------------------------------------

/// A single draw call to execute within a render pass.
#[derive(Debug, Clone)]
pub struct DrawCall {
    pub pipeline: PipelineHandle,
    pub vertex_buffer: BufferHandle,
    pub index_buffer: Option<BufferHandle>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub instance_buffer: Option<BufferHandle>,
    pub instance_count: u32,
    pub uniforms: Vec<UniformBlock>,
}

impl DrawCall {
    pub fn new(pipeline: PipelineHandle, vertex_buffer: BufferHandle, vertex_count: u32) -> Self {
        Self {
            pipeline,
            vertex_buffer,
            index_buffer: None,
            vertex_count,
            index_count: 0,
            instance_buffer: None,
            instance_count: 1,
            uniforms: Vec::new(),
        }
    }

    pub fn with_instances(mut self, buffer: BufferHandle, count: u32) -> Self {
        self.instance_buffer = Some(buffer);
        self.instance_count = count;
        self
    }

    pub fn with_index_buffer(mut self, buffer: BufferHandle, count: u32) -> Self {
        self.index_buffer = Some(buffer);
        self.index_count = count;
        self
    }

    pub fn with_uniform(mut self, block: UniformBlock) -> Self {
        self.uniforms.push(block);
        self
    }
}

// ---------------------------------------------------------------------------
// Frame state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameState {
    Idle,
    Recording,
}

// ---------------------------------------------------------------------------
// MultiBackendRenderer
// ---------------------------------------------------------------------------

/// The main renderer that dispatches draw calls and compute through a
/// `BackendContext`.
pub struct MultiBackendRenderer {
    pub backend: Box<dyn BackendContext>,
    pub capabilities: BackendCapabilities,
    frame_state: FrameState,
    recorded_commands: Vec<GpuCommand>,
    frame_count: u64,
}

impl MultiBackendRenderer {
    pub fn new(backend: Box<dyn BackendContext>, capabilities: BackendCapabilities) -> Self {
        Self {
            backend,
            capabilities,
            frame_state: FrameState::Idle,
            recorded_commands: Vec::new(),
            frame_count: 0,
        }
    }

    /// Create a renderer backed by the software context.
    pub fn software() -> Self {
        let caps = BackendCapabilities::for_backend(GpuBackend::Software);
        Self::new(Box::new(SoftwareContext::new()), caps)
    }

    /// Build a renderer from an existing pipeline's backend selection.
    /// In practice this would inspect the live Pipeline; here we default to Software.
    pub fn from_existing_pipeline(_pipeline: &()) -> Self {
        Self::software()
    }

    // -- frame lifecycle ----------------------------------------------------

    /// Begin recording a new frame.
    pub fn begin_frame(&mut self) {
        self.frame_state = FrameState::Recording;
        self.recorded_commands.clear();
    }

    /// Finish recording and submit all commands.
    pub fn end_frame(&mut self) {
        let cmds: Vec<GpuCommand> = std::mem::take(&mut self.recorded_commands);
        self.backend.submit(&cmds);
        self.frame_state = FrameState::Idle;
        self.frame_count += 1;
    }

    /// Present the current frame to the display.
    pub fn present(&mut self) {
        self.backend.present();
    }

    /// Number of frames completed so far.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    // -- draw ---------------------------------------------------------------

    /// Execute a series of draw calls within a render pass.
    pub fn draw(&mut self, _pass: &RenderPass, calls: &[DrawCall]) {
        for call in calls {
            // Bind uniforms
            for uniform in &call.uniforms {
                let ubuf = self.backend.create_buffer(uniform.data.len(), BufferUsage::UNIFORM);
                self.backend.write_buffer(ubuf, &uniform.data);
                self.recorded_commands.push(GpuCommand::SetBindGroup {
                    index: uniform.binding,
                    buffers: vec![ubuf],
                });
            }

            if let Some(idx_buf) = call.index_buffer {
                self.recorded_commands.push(GpuCommand::DrawIndexed {
                    pipeline: call.pipeline,
                    vertex_buffer: call.vertex_buffer,
                    index_buffer: idx_buf,
                    index_count: call.index_count,
                    instance_count: call.instance_count,
                });
            } else {
                self.recorded_commands.push(GpuCommand::Draw {
                    pipeline: call.pipeline,
                    vertex_buffer: call.vertex_buffer,
                    vertex_count: call.vertex_count,
                    instance_count: call.instance_count,
                });
            }
        }
    }

    // -- compute ------------------------------------------------------------

    /// Dispatch a compute shader.
    pub fn dispatch_compute(
        &mut self,
        pipeline: ComputePipelineHandle,
        workgroups: [u32; 3],
        buffers: &[BufferHandle],
    ) {
        if !buffers.is_empty() {
            self.recorded_commands.push(GpuCommand::SetBindGroup {
                index: 0,
                buffers: buffers.to_vec(),
            });
        }
        self.recorded_commands.push(GpuCommand::Dispatch {
            pipeline,
            x: workgroups[0],
            y: workgroups[1],
            z: workgroups[2],
        });
    }

    // -- buffer operations --------------------------------------------------

    /// Read data back from a GPU buffer.
    pub fn read_buffer(&self, buffer: BufferHandle) -> Vec<u8> {
        self.backend.read_buffer(buffer)
    }

    /// Upload raw bytes to a GPU buffer.
    pub fn upload_buffer(&mut self, buffer: BufferHandle, data: &[u8]) {
        self.backend.write_buffer(buffer, data);
    }

    /// Upload raw pixel data to a texture.
    pub fn upload_texture(&mut self, texture: TextureHandle, data: &[u8]) {
        self.backend.write_texture(texture, data);
    }

    // -- resource creation (convenience) ------------------------------------

    /// Create a vertex buffer and fill it with the given data.
    pub fn create_vertex_buffer(&mut self, data: &[u8]) -> BufferHandle {
        let buf = self.backend.create_buffer(data.len(), BufferUsage::VERTEX);
        self.backend.write_buffer(buf, data);
        buf
    }

    /// Create an index buffer and fill it.
    pub fn create_index_buffer(&mut self, data: &[u8]) -> BufferHandle {
        let buf = self.backend.create_buffer(data.len(), BufferUsage::INDEX);
        self.backend.write_buffer(buf, data);
        buf
    }

    /// Create a uniform buffer and fill it.
    pub fn create_uniform_buffer(&mut self, data: &[u8]) -> BufferHandle {
        let buf = self.backend.create_buffer(data.len(), BufferUsage::UNIFORM);
        self.backend.write_buffer(buf, data);
        buf
    }

    /// Create a storage buffer.
    pub fn create_storage_buffer(&mut self, size: usize) -> BufferHandle {
        self.backend.create_buffer(size, BufferUsage::STORAGE)
    }

    /// Create a colour texture.
    pub fn create_color_texture(&mut self, w: u32, h: u32) -> TextureHandle {
        self.backend.create_texture(w, h, TextureFormat::RGBA8)
    }

    /// Create a depth texture.
    pub fn create_depth_texture(&mut self, w: u32, h: u32) -> TextureHandle {
        self.backend.create_texture(w, h, TextureFormat::Depth32F)
    }

    /// Destroy a buffer.
    pub fn destroy_buffer(&mut self, buf: BufferHandle) {
        self.backend.destroy_buffer(buf);
    }

    /// Destroy a texture.
    pub fn destroy_texture(&mut self, tex: TextureHandle) {
        self.backend.destroy_texture(tex);
    }

    /// Backend name.
    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }
}

// ---------------------------------------------------------------------------
// RenderStats — collected per-frame
// ---------------------------------------------------------------------------

/// Statistics for a single frame.
#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub triangles: u32,
    pub compute_dispatches: u32,
    pub buffer_uploads: u32,
    pub texture_uploads: u32,
}

impl RenderStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_draw(&mut self, tris: u32) {
        self.draw_calls += 1;
        self.triangles += tris;
    }

    pub fn record_compute(&mut self) {
        self.compute_dispatches += 1;
    }

    pub fn record_buffer_upload(&mut self) {
        self.buffer_uploads += 1;
    }

    pub fn record_texture_upload(&mut self) {
        self.texture_uploads += 1;
    }
}

// ---------------------------------------------------------------------------
// StatsCollector — rolling window of frame stats
// ---------------------------------------------------------------------------

/// Keeps a rolling window of per-frame stats for profiling.
pub struct StatsCollector {
    history: Vec<RenderStats>,
    max_history: usize,
}

impl StatsCollector {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Vec::with_capacity(max_history),
            max_history,
        }
    }

    pub fn push(&mut self, stats: RenderStats) {
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(stats);
    }

    pub fn average_draw_calls(&self) -> f32 {
        if self.history.is_empty() { return 0.0; }
        let sum: u32 = self.history.iter().map(|s| s.draw_calls).sum();
        sum as f32 / self.history.len() as f32
    }

    pub fn average_triangles(&self) -> f32 {
        if self.history.is_empty() { return 0.0; }
        let sum: u32 = self.history.iter().map(|s| s.triangles).sum();
        sum as f32 / self.history.len() as f32
    }

    pub fn total_frames(&self) -> usize {
        self.history.len()
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}

// ---------------------------------------------------------------------------
// Render queue — sorted draw calls
// ---------------------------------------------------------------------------

/// Priority for draw call ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderOrder {
    Background = 0,
    Opaque = 1,
    Transparent = 2,
    Overlay = 3,
    UI = 4,
}

/// A queued draw call with ordering metadata.
#[derive(Debug, Clone)]
pub struct QueuedDraw {
    pub order: RenderOrder,
    pub depth: u32,  // for sorting within the same order (front-to-back or back-to-front)
    pub call: DrawCall,
}

/// A draw-call queue that sorts by order, then depth.
pub struct RenderQueue {
    items: Vec<QueuedDraw>,
}

impl RenderQueue {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, item: QueuedDraw) {
        self.items.push(item);
    }

    pub fn sort(&mut self) {
        self.items.sort_by(|a, b| {
            a.order.cmp(&b.order).then_with(|| {
                if a.order == RenderOrder::Transparent {
                    // Transparent: back to front (higher depth first)
                    b.depth.cmp(&a.depth)
                } else {
                    // Opaque: front to back (lower depth first)
                    a.depth.cmp(&b.depth)
                }
            })
        });
    }

    pub fn drain(&mut self) -> Vec<DrawCall> {
        self.sort();
        self.items.drain(..).map(|q| q.call).collect()
    }

    pub fn len(&self) -> usize { self.items.len() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
    pub fn clear(&mut self) { self.items.clear(); }
}

impl Default for RenderQueue {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu_backend::backend::*;

    fn make_renderer() -> MultiBackendRenderer {
        MultiBackendRenderer::software()
    }

    #[test]
    fn attribute_format_sizes() {
        assert_eq!(AttributeFormat::Float.byte_size(), 4);
        assert_eq!(AttributeFormat::Float3.byte_size(), 12);
        assert_eq!(AttributeFormat::Float4.byte_size(), 16);
    }

    #[test]
    fn vertex_layout_stride() {
        let layout = VertexLayout::new()
            .push(0, AttributeFormat::Float3, 0)   // pos: 0..12
            .push(1, AttributeFormat::Float2, 12);  // uv:  12..20
        assert_eq!(layout.stride(), 20);
    }

    #[test]
    fn uniform_block_from_f32() {
        let block = UniformBlock::from_f32_slice(0, &[1.0, 2.0]);
        assert_eq!(block.data.len(), 8);
        assert_eq!(block.binding, 0);
    }

    #[test]
    fn render_pass_builder() {
        let mut r = make_renderer();
        let color = r.create_color_texture(800, 600);
        let depth = r.create_depth_texture(800, 600);
        let pass = RenderPass::new()
            .with_color(color)
            .with_depth(depth)
            .with_clear(0.1, 0.1, 0.1, 1.0);
        assert_eq!(pass.color_attachments.len(), 1);
        assert!(pass.depth_attachment.is_some());
        assert_eq!(pass.clear_color[0], 0.1);
    }

    #[test]
    fn draw_call_builder() {
        let mut r = make_renderer();
        let vbuf = r.create_vertex_buffer(&[0u8; 64]);
        let pipe = r.backend.create_pipeline(
            r.backend.create_shader("v", ShaderStage::Vertex),
            r.backend.create_shader("f", ShaderStage::Fragment),
            &PipelineLayout::default(),
        );
        let call = DrawCall::new(pipe, vbuf, 3)
            .with_instances(vbuf, 10)
            .with_uniform(UniformBlock::new(0, vec![0u8; 64]));
        assert_eq!(call.instance_count, 10);
        assert_eq!(call.uniforms.len(), 1);
    }

    #[test]
    fn begin_end_frame() {
        let mut r = make_renderer();
        assert_eq!(r.frame_count(), 0);
        r.begin_frame();
        r.end_frame();
        assert_eq!(r.frame_count(), 1);
        r.present();
    }

    #[test]
    fn draw_within_pass() {
        let mut r = make_renderer();
        let vbuf = r.create_vertex_buffer(&[0u8; 48]);
        let pipe = r.backend.create_pipeline(
            r.backend.create_shader("v", ShaderStage::Vertex),
            r.backend.create_shader("f", ShaderStage::Fragment),
            &PipelineLayout::default(),
        );
        let pass = RenderPass::new();
        let call = DrawCall::new(pipe, vbuf, 3);

        r.begin_frame();
        r.draw(&pass, &[call]);
        r.end_frame();
        assert_eq!(r.frame_count(), 1);
    }

    #[test]
    fn dispatch_compute() {
        let mut r = make_renderer();
        let cs = r.backend.create_shader("compute", ShaderStage::Compute);
        let cp = r.backend.create_compute_pipeline(cs, &PipelineLayout::default());
        let buf = r.create_storage_buffer(256);

        r.begin_frame();
        r.dispatch_compute(cp, [4, 1, 1], &[buf]);
        r.end_frame();
    }

    #[test]
    fn upload_and_read_buffer() {
        let mut r = make_renderer();
        let buf = r.backend.create_buffer(8, BufferUsage::STORAGE);
        r.upload_buffer(buf, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(r.read_buffer(buf), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn upload_texture() {
        let mut r = make_renderer();
        let tex = r.create_color_texture(2, 2);
        let pixels = vec![128u8; 16];
        r.upload_texture(tex, &pixels);
    }

    #[test]
    fn backend_name() {
        let r = make_renderer();
        assert_eq!(r.backend_name(), "Software");
    }

    #[test]
    fn render_stats() {
        let mut stats = RenderStats::new();
        stats.record_draw(100);
        stats.record_draw(200);
        stats.record_compute();
        assert_eq!(stats.draw_calls, 2);
        assert_eq!(stats.triangles, 300);
        assert_eq!(stats.compute_dispatches, 1);
    }

    #[test]
    fn stats_collector_rolling() {
        let mut collector = StatsCollector::new(3);
        for i in 0..5 {
            let mut s = RenderStats::new();
            s.record_draw(i * 10);
            collector.push(s);
        }
        assert_eq!(collector.total_frames(), 3);
        // last 3 frames: 20, 30, 40 draw calls=1 each
        assert!((collector.average_draw_calls() - 1.0).abs() < 0.01);
    }

    #[test]
    fn render_queue_sorting() {
        let mut r = make_renderer();
        let vbuf = r.create_vertex_buffer(&[0u8; 12]);
        let pipe = r.backend.create_pipeline(
            r.backend.create_shader("v", ShaderStage::Vertex),
            r.backend.create_shader("f", ShaderStage::Fragment),
            &PipelineLayout::default(),
        );

        let mut queue = RenderQueue::new();
        queue.push(QueuedDraw {
            order: RenderOrder::Transparent,
            depth: 10,
            call: DrawCall::new(pipe, vbuf, 3),
        });
        queue.push(QueuedDraw {
            order: RenderOrder::Opaque,
            depth: 5,
            call: DrawCall::new(pipe, vbuf, 3),
        });
        queue.push(QueuedDraw {
            order: RenderOrder::Background,
            depth: 0,
            call: DrawCall::new(pipe, vbuf, 3),
        });
        queue.push(QueuedDraw {
            order: RenderOrder::Transparent,
            depth: 20,
            call: DrawCall::new(pipe, vbuf, 3),
        });

        let calls = queue.drain();
        assert_eq!(calls.len(), 4);
    }

    #[test]
    fn render_queue_empty() {
        let queue = RenderQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn draw_indexed() {
        let mut r = make_renderer();
        let vbuf = r.create_vertex_buffer(&[0u8; 48]);
        let ibuf = r.create_index_buffer(&[0u8; 12]);
        let pipe = r.backend.create_pipeline(
            r.backend.create_shader("v", ShaderStage::Vertex),
            r.backend.create_shader("f", ShaderStage::Fragment),
            &PipelineLayout::default(),
        );
        let call = DrawCall::new(pipe, vbuf, 3)
            .with_index_buffer(ibuf, 6);
        assert_eq!(call.index_count, 6);

        r.begin_frame();
        r.draw(&RenderPass::new(), &[call]);
        r.end_frame();
    }

    #[test]
    fn destroy_resources() {
        let mut r = make_renderer();
        let buf = r.create_vertex_buffer(&[0u8; 16]);
        let tex = r.create_color_texture(4, 4);
        r.destroy_buffer(buf);
        r.destroy_texture(tex);
        assert!(r.read_buffer(buf).is_empty());
    }

    #[test]
    fn from_existing_pipeline() {
        let unit = ();
        let r = MultiBackendRenderer::from_existing_pipeline(&unit);
        assert_eq!(r.backend_name(), "Software");
    }
}
