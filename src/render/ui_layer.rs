//! Screen-space UI layer — bypasses the 3D camera and renders in pixel coordinates.
//!
//! The UI layer renders AFTER the 3D scene and post-processing but BEFORE the
//! final composite.  UI elements are pixel-perfect, unaffected by bloom or
//! distortion, and positioned in screen coordinates: (0,0) = top-left.
//!
//! # Architecture
//!
//! ```text
//! 3D scene → PostFx (bloom, CA, grain) → UI Layer (ortho, no FX) → screen
//! ```
//!
//! The UI layer collects draw commands each frame via `UiLayer::draw_*` methods,
//! then flushes them all in one pass via `UiLayerRenderer`.

use glam::{Vec2, Vec3, Vec4, Mat4};
use std::collections::VecDeque;

// ── Draw Commands ───────────────────────────────────────────────────────────

/// A single UI draw command, queued and executed in order.
#[derive(Clone, Debug)]
pub enum UiDrawCommand {
    Text {
        text: String,
        x: f32,
        y: f32,
        scale: f32,
        color: Vec4,
        emission: f32,
        alignment: TextAlign,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: Vec4,
        filled: bool,
    },
    Panel {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        border: BorderStyle,
        fill_color: Vec4,
        border_color: Vec4,
    },
    Bar {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill_pct: f32,
        fill_color: Vec4,
        bg_color: Vec4,
        ghost_pct: Option<f32>,
        ghost_color: Vec4,
    },
    Sprite {
        lines: Vec<String>,
        x: f32,
        y: f32,
        color: Vec4,
    },
}

/// Text alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Border drawing styles for panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderStyle {
    /// Single line: ┌─┐│└─┘
    Single,
    /// Double line: ╔═╗║╚═╝
    Double,
    /// Rounded corners: ╭─╮│╰─╯
    Rounded,
    /// Heavy line: ┏━┓┃┗━┛
    Heavy,
    /// Dashed: ┌╌┐╎└╌┘
    Dashed,
}

impl BorderStyle {
    /// Get the 8 border characters: [top-left, top, top-right, left, right, bottom-left, bottom, bottom-right]
    pub fn chars(&self) -> [char; 8] {
        match self {
            BorderStyle::Single  => ['┌', '─', '┐', '│', '│', '└', '─', '┘'],
            BorderStyle::Double  => ['╔', '═', '╗', '║', '║', '╚', '═', '╝'],
            BorderStyle::Rounded => ['╭', '─', '╮', '│', '│', '╰', '─', '╯'],
            BorderStyle::Heavy   => ['┏', '━', '┓', '┃', '┃', '┗', '━', '┛'],
            BorderStyle::Dashed  => ['┌', '╌', '┐', '╎', '╎', '└', '╌', '┘'],
        }
    }
}

// ── UiLayer ─────────────────────────────────────────────────────────────────

/// The screen-space UI layer.  Collects draw commands each frame, then renders
/// them all in a single pass with an orthographic projection.
pub struct UiLayer {
    /// Screen dimensions (updated on resize).
    pub screen_width: f32,
    pub screen_height: f32,
    /// Character cell dimensions in screen pixels.
    pub char_width: f32,
    pub char_height: f32,
    /// Queued draw commands for this frame.
    draw_queue: Vec<UiDrawCommand>,
    /// Whether the UI layer is enabled.
    pub enabled: bool,
}

impl UiLayer {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            screen_width,
            screen_height,
            char_width: 10.0,
            char_height: 18.0,
            draw_queue: Vec::with_capacity(256),
            enabled: true,
        }
    }

    /// Update screen dimensions (call on resize).
    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Set the character cell size in screen pixels.
    pub fn set_char_size(&mut self, width: f32, height: f32) {
        self.char_width = width;
        self.char_height = height;
    }

    /// Clear all queued commands. Call at the start of each frame.
    pub fn begin_frame(&mut self) {
        self.draw_queue.clear();
    }

    /// Get the orthographic projection matrix for this UI layer.
    /// Maps (0,0) at top-left to (screen_width, screen_height) at bottom-right.
    pub fn projection(&self) -> Mat4 {
        Mat4::orthographic_rh_gl(
            0.0,
            self.screen_width,
            self.screen_height,
            0.0,
            -1.0,
            1.0,
        )
    }

    /// Get the draw queue for rendering.
    pub fn draw_queue(&self) -> &[UiDrawCommand] {
        &self.draw_queue
    }

    /// Number of pending draw commands.
    pub fn command_count(&self) -> usize {
        self.draw_queue.len()
    }

    // ── Drawing API ─────────────────────────────────────────────────────────

    /// Draw text at screen coordinates.
    pub fn draw_text(&mut self, x: f32, y: f32, text: &str, scale: f32, color: Vec4) {
        self.draw_queue.push(UiDrawCommand::Text {
            text: text.to_string(),
            x, y, scale,
            color,
            emission: 0.0,
            alignment: TextAlign::Left,
        });
    }

    /// Draw text with emission (for bloom-capable UI text).
    pub fn draw_text_glowing(&mut self, x: f32, y: f32, text: &str, scale: f32, color: Vec4, emission: f32) {
        self.draw_queue.push(UiDrawCommand::Text {
            text: text.to_string(),
            x, y, scale,
            color,
            emission,
            alignment: TextAlign::Left,
        });
    }

    /// Draw text with alignment.
    pub fn draw_text_aligned(&mut self, x: f32, y: f32, text: &str, scale: f32, color: Vec4, align: TextAlign) {
        self.draw_queue.push(UiDrawCommand::Text {
            text: text.to_string(),
            x, y, scale,
            color,
            emission: 0.0,
            alignment: align,
        });
    }

    /// Draw centered text (centers horizontally at the given y).
    pub fn draw_centered_text(&mut self, y: f32, text: &str, scale: f32, color: Vec4) {
        self.draw_text_aligned(self.screen_width / 2.0, y, text, scale, color, TextAlign::Center);
    }

    /// Draw word-wrapped text within a max width (in pixels).
    pub fn draw_wrapped_text(&mut self, x: f32, y: f32, max_width: f32, text: &str, scale: f32, color: Vec4) {
        let char_w = self.char_width * scale;
        let max_chars = (max_width / char_w.max(1.0)) as usize;
        let lines = wrap_text_ui(text, max_chars);
        let line_h = self.char_height * scale;
        for (i, line) in lines.iter().enumerate() {
            self.draw_text(x, y + i as f32 * line_h, line, scale, color);
        }
    }

    /// Measure text dimensions in screen pixels.
    pub fn measure_text(&self, text: &str, scale: f32) -> (f32, f32) {
        let lines: Vec<&str> = text.lines().collect();
        let max_cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let width = max_cols as f32 * self.char_width * scale;
        let height = lines.len() as f32 * self.char_height * scale;
        (width, height)
    }

    /// Draw a filled or outlined rectangle.
    pub fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Vec4, filled: bool) {
        self.draw_queue.push(UiDrawCommand::Rect {
            x, y, w, h, color, filled,
        });
    }

    /// Draw a panel with a border and optional fill.
    pub fn draw_panel(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        border: BorderStyle,
        fill_color: Vec4,
        border_color: Vec4,
    ) {
        self.draw_queue.push(UiDrawCommand::Panel {
            x, y, w, h, border, fill_color, border_color,
        });
    }

    /// Draw a progress bar using █ and ░ characters.
    pub fn draw_bar(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill_pct: f32,
        fill_color: Vec4,
        bg_color: Vec4,
    ) {
        self.draw_queue.push(UiDrawCommand::Bar {
            x, y, w, h,
            fill_pct: fill_pct.clamp(0.0, 1.0),
            fill_color,
            bg_color,
            ghost_pct: None,
            ghost_color: Vec4::ZERO,
        });
    }

    /// Draw a progress bar with a ghost bar (recent damage indicator).
    pub fn draw_bar_with_ghost(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill_pct: f32,
        fill_color: Vec4,
        bg_color: Vec4,
        ghost_pct: f32,
        ghost_color: Vec4,
    ) {
        self.draw_queue.push(UiDrawCommand::Bar {
            x, y, w, h,
            fill_pct: fill_pct.clamp(0.0, 1.0),
            fill_color,
            bg_color,
            ghost_pct: Some(ghost_pct.clamp(0.0, 1.0)),
            ghost_color,
        });
    }

    /// Draw multi-line ASCII art sprite.
    pub fn draw_sprite(&mut self, x: f32, y: f32, lines: &[&str], color: Vec4) {
        self.draw_queue.push(UiDrawCommand::Sprite {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            x, y, color,
        });
    }
}

// ── Word wrapping for UI ────────────────────────────────────────────────────

fn wrap_text_ui(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut line = String::new();
        for word in words {
            if line.is_empty() {
                if word.len() > max_chars {
                    let mut w = word;
                    while w.len() > max_chars {
                        lines.push(w[..max_chars].to_string());
                        w = &w[max_chars..];
                    }
                    line = w.to_string();
                } else {
                    line = word.to_string();
                }
            } else if line.len() + 1 + word.len() <= max_chars {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_layer_projection_is_orthographic() {
        let ui = UiLayer::new(1280.0, 800.0);
        let proj = ui.projection();
        // Top-left (0,0) should map to (-1, 1) in clip space.
        let tl = proj * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!((tl.x / tl.w - (-1.0)).abs() < 0.01);
        assert!((tl.y / tl.w - 1.0).abs() < 0.01);
    }

    #[test]
    fn ui_layer_draw_and_clear() {
        let mut ui = UiLayer::new(1280.0, 800.0);
        ui.draw_text(0.0, 0.0, "Hello", 1.0, Vec4::ONE);
        assert_eq!(ui.command_count(), 1);
        ui.begin_frame();
        assert_eq!(ui.command_count(), 0);
    }

    #[test]
    fn measure_text_single_line() {
        let ui = UiLayer::new(1280.0, 800.0);
        let (w, h) = ui.measure_text("Hello", 1.0);
        assert_eq!(w, 5.0 * ui.char_width);
        assert_eq!(h, ui.char_height);
    }

    #[test]
    fn measure_text_multi_line() {
        let ui = UiLayer::new(1280.0, 800.0);
        let (_, h) = ui.measure_text("Line1\nLine2\nLine3", 1.0);
        assert_eq!(h, 3.0 * ui.char_height);
    }

    #[test]
    fn border_style_chars() {
        let chars = BorderStyle::Single.chars();
        assert_eq!(chars[0], '┌');
        assert_eq!(chars[7], '┘');
    }

    #[test]
    fn wrap_text_ui_basic() {
        let lines = wrap_text_ui("Hello world foo bar", 10);
        for l in &lines {
            assert!(l.len() <= 10, "Line too long: '{}'", l);
        }
    }

    #[test]
    fn bar_pct_clamped() {
        let mut ui = UiLayer::new(1280.0, 800.0);
        ui.draw_bar(0.0, 0.0, 100.0, 10.0, 1.5, Vec4::ONE, Vec4::ZERO);
        if let UiDrawCommand::Bar { fill_pct, .. } = &ui.draw_queue()[0] {
            assert_eq!(*fill_pct, 1.0);
        }
    }
}
