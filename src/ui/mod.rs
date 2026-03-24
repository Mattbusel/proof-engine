//! Mathematical UI rendering system.
//!
//! All UI elements are rendered as math-function-driven glyph clusters.
//! There are no bitmaps, no sprite sheets — every UI component is computed.
//!
//! # Architecture
//!
//! - `UiPanel`       — a bordered, optionally titled container
//! - `UiLabel`       — a text string rendered at a fixed position
//! - `UiProgressBar` — a bar that fills based on a 0–1 value
//! - `UiButton`      — a clickable labeled button
//! - `UiLayout`      — anchor-based layout engine for placing elements
//! - `UiRoot`        — manages all UI elements, tick + render each frame
//!
//! All measurements are in world units (same space as glyphs).

pub mod widgets;
pub mod layout;

pub use widgets::{UiLabel, UiProgressBar, UiButton, UiPanel, UiPulseRing};
pub use layout::{UiLayout, Anchor, UiRect};

use crate::ProofEngine;
use crate::glyph::GlyphId;
use glam::{Vec2, Vec3, Vec4};

// ── UiRoot ─────────────────────────────────────────────────────────────────────

/// The root UI manager. Owns all UI elements and drives their tick + render.
///
/// Clears all owned glyphs each frame before re-rendering.
pub struct UiRoot {
    pub labels:       Vec<UiLabel>,
    pub progress_bars: Vec<UiProgressBar>,
    pub buttons:      Vec<UiButton>,
    pub panels:       Vec<UiPanel>,
    pub rings:        Vec<UiPulseRing>,
    glyph_ids:        Vec<GlyphId>,
}

impl UiRoot {
    pub fn new() -> Self {
        Self {
            labels:        Vec::new(),
            progress_bars: Vec::new(),
            buttons:       Vec::new(),
            panels:        Vec::new(),
            rings:         Vec::new(),
            glyph_ids:     Vec::new(),
        }
    }

    /// Tick all UI elements (advances animations, checks input).
    pub fn tick(&mut self, engine: &mut ProofEngine, dt: f32) {
        let mouse_pos = engine.input.mouse_pos();
        let mouse_ndc = engine.input.mouse_ndc;
        let clicked   = engine.input.mouse_left_just_pressed;

        for btn in &mut self.buttons {
            btn.tick(mouse_ndc, clicked, dt);
        }
        for pb in &mut self.progress_bars {
            pb.tick(dt);
        }
        for ring in &mut self.rings {
            ring.tick(dt);
        }
        let _ = mouse_pos; // suppress unused
    }

    /// Render all UI elements as glyphs. Clears previous frame's glyphs.
    pub fn render(&mut self, engine: &mut ProofEngine) {
        // Despawn previous frame's UI glyphs
        for id in self.glyph_ids.drain(..) {
            engine.scene.glyphs.despawn(id);
        }

        let time = engine.scene.time;

        // Render panels (backgrounds, so they go first)
        for panel in &mut self.panels {
            let ids = panel.render(engine, time);
            self.glyph_ids.extend(ids);
        }
        // Render labels
        for label in &mut self.labels {
            let ids = label.render(engine, time);
            self.glyph_ids.extend(ids);
        }
        // Render progress bars
        for pb in &mut self.progress_bars {
            let ids = pb.render(engine, time);
            self.glyph_ids.extend(ids);
        }
        // Render buttons
        for btn in &mut self.buttons {
            let ids = btn.render(engine, time);
            self.glyph_ids.extend(ids);
        }
        // Render pulse rings (HUD indicators)
        for ring in &mut self.rings {
            let ids = ring.render(engine, time);
            self.glyph_ids.extend(ids);
        }
    }

    /// Clear all UI elements.
    pub fn clear(&mut self) {
        self.labels.clear();
        self.progress_bars.clear();
        self.buttons.clear();
        self.panels.clear();
        self.rings.clear();
    }
}

impl Default for UiRoot {
    fn default() -> Self { Self::new() }
}

// ── UiColor presets ────────────────────────────────────────────────────────────

/// Predefined UI color palette.
pub struct UiColors;

impl UiColors {
    pub const HEALTH:     Vec4 = Vec4::new(0.9, 0.2, 0.2, 1.0);
    pub const MANA:       Vec4 = Vec4::new(0.2, 0.4, 1.0, 1.0);
    pub const STAMINA:    Vec4 = Vec4::new(0.2, 0.9, 0.3, 1.0);
    pub const EXPERIENCE: Vec4 = Vec4::new(0.9, 0.7, 0.1, 1.0);
    pub const WARNING:    Vec4 = Vec4::new(1.0, 0.6, 0.0, 1.0);
    pub const CRITICAL:   Vec4 = Vec4::new(1.0, 0.1, 0.1, 1.0);
    pub const TEXT:       Vec4 = Vec4::new(0.9, 0.9, 0.9, 1.0);
    pub const DIM:        Vec4 = Vec4::new(0.5, 0.5, 0.5, 0.7);
    pub const GOLD:       Vec4 = Vec4::new(1.0, 0.85, 0.2, 1.0);
    pub const SILVER:     Vec4 = Vec4::new(0.8, 0.8, 0.9, 0.9);
    pub const BOSS:       Vec4 = Vec4::new(0.8, 0.1, 0.9, 1.0);
    pub const HEAL:       Vec4 = Vec4::new(0.3, 1.0, 0.5, 1.0);
}
