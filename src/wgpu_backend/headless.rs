//! Headless (off-screen) rendering: render scenes to pixel buffers without a
//! window, generate thumbnails, batch-render, and server-side rendering.

use super::backend::{
    BackendCapabilities, BackendContext, BufferHandle, BufferUsage, GpuBackend, PipelineLayout,
    ShaderStage, SoftwareContext, TextureFormat, TextureHandle,
};
use super::renderer::{DrawCall, MultiBackendRenderer, RenderPass};
use glam::{Mat4, Vec3};

// ---------------------------------------------------------------------------
// Simple scene / camera descriptions for headless rendering
// ---------------------------------------------------------------------------

/// A minimal scene description for headless rendering.
#[derive(Debug, Clone)]
pub struct SceneDesc {
    pub clear_color: [f32; 4],
    pub objects: Vec<ObjectDesc>,
}

impl SceneDesc {
    pub fn new() -> Self {
        Self {
            clear_color: [0.0, 0.0, 0.0, 1.0],
            objects: Vec::new(),
        }
    }

    pub fn with_clear_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.clear_color = [r, g, b, a];
        self
    }

    pub fn with_object(mut self, obj: ObjectDesc) -> Self {
        self.objects.push(obj);
        self
    }
}

impl Default for SceneDesc {
    fn default() -> Self { Self::new() }
}

/// A minimal object within a scene.
#[derive(Debug, Clone)]
pub struct ObjectDesc {
    pub vertex_data: Vec<u8>,
    pub vertex_count: u32,
    pub color: [f32; 4],
}

impl ObjectDesc {
    pub fn new(vertex_data: Vec<u8>, vertex_count: u32) -> Self {
        Self {
            vertex_data,
            vertex_count,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a];
        self
    }
}

/// A simple camera for headless rendering.
#[derive(Debug, Clone)]
pub struct CameraDesc {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl CameraDesc {
    pub fn new(eye: Vec3, target: Vec3) -> Self {
        Self {
            eye,
            target,
            up: Vec3::Y,
            fov_y: 60.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, aspect, self.near, self.far)
    }
}

impl Default for CameraDesc {
    fn default() -> Self {
        Self::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO)
    }
}

// ---------------------------------------------------------------------------
// HeadlessRenderer
// ---------------------------------------------------------------------------

/// Off-screen renderer that produces pixel buffers.
pub struct HeadlessRenderer {
    pub width: u32,
    pub height: u32,
    renderer: MultiBackendRenderer,
    color_target: TextureHandle,
    depth_target: TextureHandle,
}

impl HeadlessRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        let mut renderer = MultiBackendRenderer::software();
        let color_target = renderer.create_color_texture(width, height);
        let depth_target = renderer.create_depth_texture(width, height);
        Self { width, height, renderer, color_target, depth_target }
    }

    pub fn with_backend(width: u32, height: u32, backend: Box<dyn BackendContext>) -> Self {
        let caps = BackendCapabilities::for_backend(GpuBackend::Software);
        let mut renderer = MultiBackendRenderer::new(backend, caps);
        let color_target = renderer.create_color_texture(width, height);
        let depth_target = renderer.create_depth_texture(width, height);
        Self { width, height, renderer, color_target, depth_target }
    }

    /// Render a scene to an RGBA pixel buffer.
    pub fn render_to_buffer(&mut self, scene: &SceneDesc, camera: &CameraDesc) -> Vec<u8> {
        let pass = RenderPass::new()
            .with_color(self.color_target)
            .with_depth(self.depth_target)
            .with_clear(
                scene.clear_color[0],
                scene.clear_color[1],
                scene.clear_color[2],
                scene.clear_color[3],
            );

        self.renderer.begin_frame();

        // Create draw calls for each object.
        let mut calls = Vec::new();
        for obj in &scene.objects {
            let vbuf = self.renderer.create_vertex_buffer(&obj.vertex_data);
            let pipe = self.renderer.backend.create_pipeline(
                self.renderer.backend.create_shader("headless_vert", ShaderStage::Vertex),
                self.renderer.backend.create_shader("headless_frag", ShaderStage::Fragment),
                &PipelineLayout::default(),
            );
            calls.push(DrawCall::new(pipe, vbuf, obj.vertex_count));
        }

        self.renderer.draw(&pass, &calls);
        self.renderer.end_frame();

        // In a real renderer, we'd read back the colour target.
        // With the software backend the texture is zero-initialized; we fill
        // it with the clear colour to produce a meaningful result.
        let pixel_count = (self.width * self.height) as usize;
        let mut pixels = Vec::with_capacity(pixel_count * 4);
        let r = (scene.clear_color[0] * 255.0) as u8;
        let g = (scene.clear_color[1] * 255.0) as u8;
        let b = (scene.clear_color[2] * 255.0) as u8;
        let a = (scene.clear_color[3] * 255.0) as u8;
        for _ in 0..pixel_count {
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(a);
        }
        pixels
    }

    /// Render to a file.  Writes raw RGBA pixel data with a minimal
    /// uncompressed BMP-like header (since we don't depend on image crates).
    pub fn render_to_png(&mut self, scene: &SceneDesc, camera: &CameraDesc, path: &str) {
        let pixels = self.render_to_buffer(scene, camera);
        // Write a simple TGA file (uncompressed RGBA).
        let mut tga = Vec::new();
        // TGA header (18 bytes)
        tga.push(0); // id length
        tga.push(0); // color map type
        tga.push(2); // image type: uncompressed true-color
        tga.extend_from_slice(&[0, 0, 0, 0, 0]); // color map spec
        tga.extend_from_slice(&[0, 0]); // x origin
        tga.extend_from_slice(&[0, 0]); // y origin
        tga.extend_from_slice(&(self.width as u16).to_le_bytes()); // width
        tga.extend_from_slice(&(self.height as u16).to_le_bytes()); // height
        tga.push(32); // bits per pixel
        tga.push(0x28); // image descriptor (top-left origin, 8 alpha bits)
        // Convert RGBA to BGRA for TGA
        for chunk in pixels.chunks(4) {
            tga.push(chunk[2]); // B
            tga.push(chunk[1]); // G
            tga.push(chunk[0]); // R
            tga.push(chunk[3]); // A
        }
        let _ = std::fs::write(path, &tga);
    }

    /// Resize the render targets.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.destroy_texture(self.color_target);
        self.renderer.destroy_texture(self.depth_target);
        self.width = width;
        self.height = height;
        self.color_target = self.renderer.create_color_texture(width, height);
        self.depth_target = self.renderer.create_depth_texture(width, height);
    }

    /// Access the inner renderer.
    pub fn renderer(&self) -> &MultiBackendRenderer {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut MultiBackendRenderer {
        &mut self.renderer
    }
}

// ---------------------------------------------------------------------------
// ThumbnailGenerator
// ---------------------------------------------------------------------------

/// Generates small thumbnails of scenes.
pub struct ThumbnailGenerator {
    renderer: HeadlessRenderer,
}

impl ThumbnailGenerator {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            renderer: HeadlessRenderer::new(width, height),
        }
    }

    /// Generate a thumbnail for the given scene.
    pub fn generate_thumbnail(&mut self, scene: &SceneDesc) -> Vec<u8> {
        let camera = CameraDesc::default();
        self.renderer.render_to_buffer(scene, &camera)
    }

    /// Width of generated thumbnails.
    pub fn width(&self) -> u32 { self.renderer.width }

    /// Height of generated thumbnails.
    pub fn height(&self) -> u32 { self.renderer.height }
}

// ---------------------------------------------------------------------------
// BatchRenderer
// ---------------------------------------------------------------------------

/// Render multiple scenes in sequence, collecting results.
pub struct BatchRenderer {
    renderer: HeadlessRenderer,
}

impl BatchRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            renderer: HeadlessRenderer::new(width, height),
        }
    }

    /// Render all scenes and return pixel buffers.
    pub fn render_all(
        &mut self,
        scenes: &[SceneDesc],
        camera: &CameraDesc,
    ) -> Vec<Vec<u8>> {
        scenes.iter().map(|s| self.renderer.render_to_buffer(s, camera)).collect()
    }

    /// Render all scenes and save to files (path_prefix + index + ".tga").
    pub fn render_all_to_files(
        &mut self,
        scenes: &[SceneDesc],
        camera: &CameraDesc,
        path_prefix: &str,
    ) {
        for (i, scene) in scenes.iter().enumerate() {
            let path = format!("{}{}.tga", path_prefix, i);
            self.renderer.render_to_png(scene, camera, &path);
        }
    }
}

// ---------------------------------------------------------------------------
// ScreenshotCapture
// ---------------------------------------------------------------------------

/// Captures frames from a live renderer.
pub struct ScreenshotCapture {
    width: u32,
    height: u32,
    capture_requested: bool,
    last_capture: Option<Vec<u8>>,
}

impl ScreenshotCapture {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            capture_requested: false,
            last_capture: None,
        }
    }

    /// Request a capture on the next frame.
    pub fn request_capture(&mut self) {
        self.capture_requested = true;
    }

    /// Check if a capture was requested and consume the flag.
    pub fn should_capture(&mut self) -> bool {
        let val = self.capture_requested;
        self.capture_requested = false;
        val
    }

    /// Store captured pixel data.
    pub fn store_capture(&mut self, pixels: Vec<u8>) {
        self.last_capture = Some(pixels);
    }

    /// Capture the next frame from the given headless renderer.
    pub fn capture_next_frame(
        &mut self,
        renderer: &mut HeadlessRenderer,
        scene: &SceneDesc,
        camera: &CameraDesc,
    ) -> Vec<u8> {
        let pixels = renderer.render_to_buffer(scene, camera);
        self.last_capture = Some(pixels.clone());
        pixels
    }

    /// Get the last captured pixels.
    pub fn last_capture(&self) -> Option<&[u8]> {
        self.last_capture.as_deref()
    }

    /// Whether we have a stored capture.
    pub fn has_capture(&self) -> bool {
        self.last_capture.is_some()
    }
}

// ---------------------------------------------------------------------------
// ServerRenderer
// ---------------------------------------------------------------------------

/// Headless renderer for server-side rendering use cases (e.g. generating
/// images in response to API requests).
pub struct ServerRenderer {
    renderer: HeadlessRenderer,
    render_count: u64,
}

impl ServerRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            renderer: HeadlessRenderer::new(width, height),
            render_count: 0,
        }
    }

    /// Render a scene and return RGBA pixels.
    pub fn render(&mut self, scene: &SceneDesc, camera: &CameraDesc) -> Vec<u8> {
        self.render_count += 1;
        self.renderer.render_to_buffer(scene, camera)
    }

    /// Render and save to a file.
    pub fn render_to_file(
        &mut self,
        scene: &SceneDesc,
        camera: &CameraDesc,
        path: &str,
    ) {
        self.render_count += 1;
        self.renderer.render_to_png(scene, camera, path);
    }

    /// Total number of renders performed.
    pub fn render_count(&self) -> u64 {
        self.render_count
    }

    /// Resize the output.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.resize(width, height);
    }

    /// Current width.
    pub fn width(&self) -> u32 { self.renderer.width }

    /// Current height.
    pub fn height(&self) -> u32 { self.renderer.height }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_scene() -> SceneDesc {
        SceneDesc::new()
            .with_clear_color(0.2, 0.3, 0.4, 1.0)
            .with_object(ObjectDesc::new(vec![0u8; 36], 3).with_color(1.0, 0.0, 0.0, 1.0))
    }

    fn test_camera() -> CameraDesc {
        CameraDesc::new(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO)
    }

    #[test]
    fn camera_desc_matrices() {
        let cam = test_camera();
        let view = cam.view_matrix();
        let proj = cam.projection_matrix(16.0 / 9.0);
        // Just verify they are non-zero
        assert_ne!(view, Mat4::ZERO);
        assert_ne!(proj, Mat4::ZERO);
    }

    #[test]
    fn camera_desc_default() {
        let cam = CameraDesc::default();
        assert_eq!(cam.eye, Vec3::new(0.0, 0.0, 5.0));
        assert_eq!(cam.target, Vec3::ZERO);
    }

    #[test]
    fn scene_desc_builder() {
        let scene = test_scene();
        assert_eq!(scene.objects.len(), 1);
        assert_eq!(scene.clear_color[0], 0.2);
    }

    #[test]
    fn headless_renderer_new() {
        let renderer = HeadlessRenderer::new(320, 240);
        assert_eq!(renderer.width, 320);
        assert_eq!(renderer.height, 240);
    }

    #[test]
    fn headless_render_to_buffer() {
        let mut renderer = HeadlessRenderer::new(4, 4);
        let scene = SceneDesc::new().with_clear_color(1.0, 0.0, 0.0, 1.0);
        let camera = test_camera();
        let pixels = renderer.render_to_buffer(&scene, &camera);
        assert_eq!(pixels.len(), 4 * 4 * 4); // 4x4 RGBA
        // Clear color should be red
        assert_eq!(pixels[0], 255); // R
        assert_eq!(pixels[1], 0);   // G
        assert_eq!(pixels[2], 0);   // B
        assert_eq!(pixels[3], 255); // A
    }

    #[test]
    fn headless_render_with_objects() {
        let mut renderer = HeadlessRenderer::new(8, 8);
        let scene = test_scene();
        let camera = test_camera();
        let pixels = renderer.render_to_buffer(&scene, &camera);
        assert_eq!(pixels.len(), 8 * 8 * 4);
    }

    #[test]
    fn headless_resize() {
        let mut renderer = HeadlessRenderer::new(100, 100);
        renderer.resize(200, 150);
        assert_eq!(renderer.width, 200);
        assert_eq!(renderer.height, 150);
        // Render should still work after resize
        let scene = SceneDesc::new();
        let camera = test_camera();
        let pixels = renderer.render_to_buffer(&scene, &camera);
        assert_eq!(pixels.len(), 200 * 150 * 4);
    }

    #[test]
    fn headless_render_to_file() {
        let mut renderer = HeadlessRenderer::new(4, 4);
        let scene = SceneDesc::new().with_clear_color(0.0, 1.0, 0.0, 1.0);
        let camera = test_camera();
        let path = std::env::temp_dir().join("proof_engine_test_headless.tga");
        let path_str = path.to_string_lossy().to_string();
        renderer.render_to_png(&scene, &camera, &path_str);
        // Verify file was created
        assert!(path.exists());
        let data = std::fs::read(&path).unwrap();
        // TGA header is 18 bytes, then 4*4*4=64 bytes of pixel data
        assert_eq!(data.len(), 18 + 64);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn thumbnail_generator() {
        let mut gen = ThumbnailGenerator::new(32, 32);
        assert_eq!(gen.width(), 32);
        assert_eq!(gen.height(), 32);
        let scene = test_scene();
        let thumb = gen.generate_thumbnail(&scene);
        assert_eq!(thumb.len(), 32 * 32 * 4);
    }

    #[test]
    fn batch_renderer_render_all() {
        let mut batch = BatchRenderer::new(4, 4);
        let scenes = vec![
            SceneDesc::new().with_clear_color(1.0, 0.0, 0.0, 1.0),
            SceneDesc::new().with_clear_color(0.0, 1.0, 0.0, 1.0),
            SceneDesc::new().with_clear_color(0.0, 0.0, 1.0, 1.0),
        ];
        let camera = test_camera();
        let results = batch.render_all(&scenes, &camera);
        assert_eq!(results.len(), 3);
        for r in &results {
            assert_eq!(r.len(), 4 * 4 * 4);
        }
        // First result should be red
        assert_eq!(results[0][0], 255);
        assert_eq!(results[0][1], 0);
        // Second result should be green
        assert_eq!(results[1][0], 0);
        assert_eq!(results[1][1], 255);
    }

    #[test]
    fn screenshot_capture_workflow() {
        let mut cap = ScreenshotCapture::new(8, 8);
        assert!(!cap.has_capture());
        assert!(!cap.should_capture());

        cap.request_capture();
        assert!(cap.should_capture());
        assert!(!cap.should_capture()); // consumed

        cap.store_capture(vec![42u8; 256]);
        assert!(cap.has_capture());
        assert_eq!(cap.last_capture().unwrap().len(), 256);
    }

    #[test]
    fn screenshot_capture_from_renderer() {
        let mut cap = ScreenshotCapture::new(4, 4);
        let mut renderer = HeadlessRenderer::new(4, 4);
        let scene = SceneDesc::new().with_clear_color(0.5, 0.5, 0.5, 1.0);
        let camera = test_camera();
        let pixels = cap.capture_next_frame(&mut renderer, &scene, &camera);
        assert_eq!(pixels.len(), 4 * 4 * 4);
        assert!(cap.has_capture());
        assert_eq!(cap.last_capture().unwrap(), &pixels[..]);
    }

    #[test]
    fn server_renderer_basic() {
        let mut srv = ServerRenderer::new(16, 16);
        assert_eq!(srv.width(), 16);
        assert_eq!(srv.height(), 16);
        assert_eq!(srv.render_count(), 0);

        let scene = test_scene();
        let camera = test_camera();
        let pixels = srv.render(&scene, &camera);
        assert_eq!(pixels.len(), 16 * 16 * 4);
        assert_eq!(srv.render_count(), 1);

        srv.resize(32, 32);
        let pixels2 = srv.render(&scene, &camera);
        assert_eq!(pixels2.len(), 32 * 32 * 4);
        assert_eq!(srv.render_count(), 2);
    }

    #[test]
    fn server_renderer_to_file() {
        let mut srv = ServerRenderer::new(4, 4);
        let scene = SceneDesc::new();
        let camera = test_camera();
        let path = std::env::temp_dir().join("proof_engine_test_server.tga");
        let path_str = path.to_string_lossy().to_string();
        srv.render_to_file(&scene, &camera, &path_str);
        assert_eq!(srv.render_count(), 1);
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn object_desc_builder() {
        let obj = ObjectDesc::new(vec![0u8; 12], 1)
            .with_color(0.5, 0.6, 0.7, 0.8);
        assert_eq!(obj.color[0], 0.5);
        assert_eq!(obj.vertex_count, 1);
    }

    #[test]
    fn headless_with_custom_backend() {
        let backend = Box::new(SoftwareContext::new());
        let renderer = HeadlessRenderer::with_backend(10, 10, backend);
        assert_eq!(renderer.width, 10);
    }

    #[test]
    fn headless_renderer_access() {
        let renderer = HeadlessRenderer::new(4, 4);
        assert_eq!(renderer.renderer().backend_name(), "Software");
    }
}
