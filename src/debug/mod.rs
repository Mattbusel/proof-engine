//! Debug overlay and performance profiling.
//!
//! `DebugOverlay` renders FPS counters, entity counts, force field visualizations,
//! collision bounds, and a math-function grapher directly into the scene as glyphs.
//!
//! Disable in release builds by not calling `DebugOverlay::render()`.
//!
//! # Example
//!
//! ```rust,no_run
//! use proof_engine::prelude::*;
//! use proof_engine::debug::DebugOverlay;
//!
//! let mut overlay = DebugOverlay::new();
//! // In your update loop:
//! // overlay.render(engine, dt);
//! ```

pub mod profiler;
pub mod graph;
pub mod console;

pub use profiler::FrameProfiler;
pub use graph::MathGraph;

use crate::{ProofEngine, Glyph, RenderLayer, MathFunction};
use crate::render::pipeline::FrameStats;
use glam::{Vec3, Vec4};

// ── DebugOverlay ──────────────────────────────────────────────────────────────

/// Debug overlay: HUD glyph rendering of engine statistics.
///
/// All output is rendered as in-world glyphs at a fixed screen-space position
/// using `RenderLayer::UI`.  Each call to `render()` clears the previous overlay
/// glyphs and redraws fresh ones.
pub struct DebugOverlay {
    pub enabled:        bool,
    pub show_fps:       bool,
    pub show_counts:    bool,
    pub show_fields:    bool,
    pub show_camera:    bool,
    pub show_time:      bool,
    pub text_color:     Vec4,
    pub warning_color:  Vec4,
    pub critical_color: Vec4,
    /// Glyph IDs spawned this frame (cleared each render()).
    glyph_ids: Vec<crate::glyph::GlyphId>,
    profiler:  FrameProfiler,
    graph:     Option<MathGraph>,
}

impl DebugOverlay {
    pub fn new() -> Self {
        Self {
            enabled:        true,
            show_fps:       true,
            show_counts:    true,
            show_fields:    false,
            show_camera:    false,
            show_time:      true,
            text_color:     Vec4::new(0.6, 1.0, 0.6, 0.8),
            warning_color:  Vec4::new(1.0, 0.8, 0.2, 1.0),
            critical_color: Vec4::new(1.0, 0.2, 0.2, 1.0),
            glyph_ids:      Vec::new(),
            profiler:       FrameProfiler::new(120),
            graph:          None,
        }
    }

    /// Attach a math graph to display in the corner.
    pub fn with_graph(mut self, func: MathFunction, range: (f32, f32)) -> Self {
        self.graph = Some(MathGraph::new(func, range));
        self
    }

    /// Clear the previous overlay and render the current frame's debug info.
    pub fn render(&mut self, engine: &mut ProofEngine, stats: &FrameStats) {
        if !self.enabled { return; }

        // Remove previous overlay glyphs
        for id in self.glyph_ids.drain(..) {
            engine.scene.glyphs.despawn(id);
        }

        // Camera "screen space" anchor — top-left corner of the visible area
        let cam_pos  = engine.camera.target.position();
        let cam_z    = engine.camera.position.position().z;
        let fov_rad  = engine.camera.fov.position.to_radians();
        let half_h   = cam_z * (fov_rad * 0.5).tan();
        let aspect   = 16.0 / 9.0; // assume 16:9 until resize tracked
        let half_w   = half_h * aspect;
        let origin   = cam_pos + Vec3::new(-half_w + 0.5, half_h - 0.5, 0.0);

        let mut line = 0;
        let line_h   = -0.65_f32;
        let char_w   =  0.40_f32;

        // ── FPS ────────────────────────────────────────────────────────────────
        if self.show_fps {
            let fps = stats.fps;
            let color = if fps >= 55.0 { self.text_color }
                        else if fps >= 30.0 { self.warning_color }
                        else { self.critical_color };
            let label = format!("FPS:{:>5.1}  dt:{:.1}ms", fps, stats.dt * 1000.0);
            self.draw_string(engine, &label, origin + Vec3::new(0.0, line as f32 * line_h, 0.0), char_w, color);
            line += 1;
        }

        // ── Counts ─────────────────────────────────────────────────────────────
        if self.show_counts {
            let label = format!(
                "G:{:<4} P:{:<4} F:{:<2}",
                stats.glyph_count, stats.particle_count, stats.frame_number % 10000
            );
            self.draw_string(engine, &label, origin + Vec3::new(0.0, line as f32 * line_h, 0.0), char_w, self.text_color);
            line += 1;
        }

        // ── Camera position ────────────────────────────────────────────────────
        if self.show_camera {
            let pos = engine.camera.position.position();
            let label = format!("CAM ({:.1},{:.1},{:.1})", pos.x, pos.y, pos.z);
            self.draw_string(engine, &label, origin + Vec3::new(0.0, line as f32 * line_h, 0.0), char_w, self.text_color);
            line += 1;
        }

        // ── Scene time ─────────────────────────────────────────────────────────
        if self.show_time {
            let label = format!("T:{:.2}s", engine.scene.time);
            self.draw_string(engine, &label, origin + Vec3::new(0.0, line as f32 * line_h, 0.0), char_w, self.text_color);
        }

        // ── Math graph ─────────────────────────────────────────────────────────
        if let Some(ref graph) = self.graph {
            let graph_origin = origin + Vec3::new(0.0, -4.0, 0.0);
            let ids = graph.render(engine, graph_origin, 20.0, 6.0, engine.scene.time);
            self.glyph_ids.extend(ids);
        }
    }

    // ── Internal string renderer ───────────────────────────────────────────────

    fn draw_string(
        &mut self,
        engine:   &mut ProofEngine,
        text:     &str,
        position: Vec3,
        char_w:   f32,
        color:    Vec4,
    ) {
        for (i, ch) in text.chars().enumerate() {
            let x = position.x + i as f32 * char_w;
            let id = engine.scene.spawn_glyph(Glyph {
                character: ch,
                position:  Vec3::new(x, position.y, position.z),
                color,
                emission:  0.2,
                glow_color: Vec3::new(color.x, color.y, color.z),
                glow_radius: 0.0,
                scale:     glam::Vec2::splat(0.55),
                layer:     RenderLayer::UI,
                ..Default::default()
            });
            self.glyph_ids.push(id);
        }
    }
}

impl Default for DebugOverlay {
    fn default() -> Self { Self::new() }
}
