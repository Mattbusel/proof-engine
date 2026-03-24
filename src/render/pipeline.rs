//! Render pipeline — orchestrates glyph batching, post-processing, and swap.
//!
//! Phase 1 stub. Full OpenGL pipeline to be implemented in Phase 1.

use crate::config::EngineConfig;
use crate::scene::Scene;
use crate::render::camera::ProofCamera;
use crate::input::InputState;

/// The full render pipeline. Owns the OpenGL context and window.
pub struct Pipeline {
    pub width: u32,
    pub height: u32,
    /// True until the window is closed.
    running: bool,
}

impl Pipeline {
    /// Initialize the window and OpenGL context.
    pub fn init(config: &EngineConfig) -> Self {
        log::info!(
            "Pipeline::init() — {}x{} '{}' (Phase 1 stub)",
            config.window_width, config.window_height, config.window_title
        );
        Self {
            width: config.window_width,
            height: config.window_height,
            running: true,
        }
    }

    /// Poll window events. Returns false if the window was closed.
    pub fn poll_events(&mut self, input: &mut InputState) -> bool {
        // Phase 1: full winit event loop here
        input.clear_frame();
        self.running
    }

    /// Render the scene. Phase 1: clears the screen and draws glyph instances.
    pub fn render(&mut self, scene: &Scene, camera: &ProofCamera) {
        // Phase 1: OpenGL clear + glyph batch draw
    }

    /// Swap the back buffer to the screen. Returns false if the window was closed.
    pub fn swap(&mut self) -> bool {
        self.running
    }
}
