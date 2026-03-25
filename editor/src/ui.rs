//! UI renderer — draws text and widgets as engine glyphs.

use proof_engine::prelude::*;
use glam::{Vec3, Vec4};

pub struct UiRenderer;

impl UiRenderer {
    pub fn new() -> Self { Self }

    /// Draw a text string at world position as short-lived glyphs.
    pub fn draw_text(
        &self,
        engine: &mut ProofEngine,
        x: f32,
        y: f32,
        text: &str,
        color: Vec4,
        emission: f32,
        layer: RenderLayer,
    ) {
        for (i, ch) in text.chars().enumerate() {
            if ch == ' ' { continue; }
            engine.spawn_glyph(Glyph {
                character: ch,
                position: Vec3::new(x + i as f32 * 0.42, y, 1.0),
                color,
                emission,
                layer,
                lifetime: 0.02, // one-frame text
                ..Default::default()
            });
        }
    }

    /// Draw a horizontal bar (like HP bar).
    pub fn draw_bar(
        &self,
        engine: &mut ProofEngine,
        x: f32,
        y: f32,
        width: usize,
        fill: f32,
        fill_color: Vec4,
        bg_color: Vec4,
    ) {
        let filled = (width as f32 * fill.clamp(0.0, 1.0)) as usize;
        for i in 0..width {
            let ch = if i < filled { '█' } else { '░' };
            let color = if i < filled { fill_color } else { bg_color };
            engine.spawn_glyph(Glyph {
                character: ch,
                position: Vec3::new(x + i as f32 * 0.42, y, 1.0),
                color,
                emission: if i < filled { 0.2 } else { 0.0 },
                layer: RenderLayer::UI,
                lifetime: 0.02,
                ..Default::default()
            });
        }
    }

    /// Draw a box outline.
    pub fn draw_box(
        &self,
        engine: &mut ProofEngine,
        x: f32,
        y: f32,
        w: usize,
        h: usize,
        color: Vec4,
    ) {
        // Top and bottom
        for i in 0..w {
            let ch = if i == 0 { '┌' } else if i == w - 1 { '┐' } else { '─' };
            self.draw_char(engine, x + i as f32 * 0.42, y, ch, color);
            let ch = if i == 0 { '└' } else if i == w - 1 { '┘' } else { '─' };
            self.draw_char(engine, x + i as f32 * 0.42, y - (h - 1) as f32 * 0.55, ch, color);
        }
        // Sides
        for j in 1..h - 1 {
            self.draw_char(engine, x, y - j as f32 * 0.55, '│', color);
            self.draw_char(engine, x + (w - 1) as f32 * 0.42, y - j as f32 * 0.55, '│', color);
        }
    }

    fn draw_char(&self, engine: &mut ProofEngine, x: f32, y: f32, ch: char, color: Vec4) {
        engine.spawn_glyph(Glyph {
            character: ch,
            position: Vec3::new(x, y, 1.0),
            color,
            emission: 0.1,
            layer: RenderLayer::UI,
            lifetime: 0.02,
            ..Default::default()
        });
    }
}
