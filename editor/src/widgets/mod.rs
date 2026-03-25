//! Widget system — self-contained UI elements rendered as engine glyphs.
//!
//! Every widget knows its bounds, handles mouse/keyboard input, and renders
//! itself by spawning short-lived glyphs. Widgets are immediate-mode: they
//! are rebuilt every frame from application state.
//!
//! # Widget types
//!
//! - `Label` — static text
//! - `Button` — clickable, hover highlight, icon support
//! - `Toggle` — on/off checkbox
//! - `Slider` — horizontal value slider with numeric display
//! - `ColorPicker` — RGB color picker with preview swatch
//! - `Dropdown` — expandable list of options
//! - `TextInput` — editable single-line text field
//! - `NumberInput` — editable numeric field with drag-to-adjust
//! - `ScrollArea` — clipped region with vertical scrollbar
//! - `TabBar` — horizontal tab strip
//! - `TreeNode` — expandable hierarchy item
//! - `Separator` — horizontal line divider
//! - `ProgressBar` — fill bar with label
//! - `Tooltip` — hover popup text
//! - `ModalDialog` — centered overlay with buttons
//! - `ContextMenu` — right-click popup menu

pub mod button;
pub mod slider;
pub mod color_picker;
pub mod text_input;
pub mod dropdown;
pub mod tree_node;
pub mod scroll_area;
pub mod tabs;
pub mod dialog;
pub mod common;

use glam::{Vec3, Vec4};
use proof_engine::prelude::*;

// ── Shared types ────────────────────────────────────────────────────────────

/// Rectangle in screen coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self { Self { x, y, w, h } }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) { (self.x + self.w * 0.5, self.y + self.h * 0.5) }
    pub fn right(&self) -> f32 { self.x + self.w }
    pub fn bottom(&self) -> f32 { self.y + self.h }
}

/// Widget interaction result.
#[derive(Debug, Clone)]
pub enum WidgetResponse {
    None,
    Clicked,
    ValueChanged(f32),
    TextChanged(String),
    ColorChanged(Vec4),
    IndexChanged(usize),
    Toggled(bool),
    Expanded(bool),
    Submitted,
    Cancelled,
    DragDelta(f32, f32),
    ContextMenu(f32, f32),
}

/// Theme colors for widgets.
#[derive(Debug, Clone)]
pub struct WidgetTheme {
    pub bg: Vec4,
    pub bg_hover: Vec4,
    pub bg_active: Vec4,
    pub fg: Vec4,
    pub fg_dim: Vec4,
    pub fg_bright: Vec4,
    pub accent: Vec4,
    pub accent_dim: Vec4,
    pub error: Vec4,
    pub success: Vec4,
    pub warning: Vec4,
    pub border: Vec4,
    pub selection: Vec4,
    pub text_cursor: Vec4,
    pub separator: Vec4,
    pub shadow: Vec4,
}

impl Default for WidgetTheme {
    fn default() -> Self { Self::dark() }
}

impl WidgetTheme {
    pub fn dark() -> Self {
        Self {
            bg:          Vec4::new(0.10, 0.10, 0.13, 0.92),
            bg_hover:    Vec4::new(0.15, 0.15, 0.20, 0.95),
            bg_active:   Vec4::new(0.20, 0.22, 0.28, 1.00),
            fg:          Vec4::new(0.80, 0.80, 0.85, 1.00),
            fg_dim:      Vec4::new(0.45, 0.45, 0.50, 0.80),
            fg_bright:   Vec4::new(1.00, 1.00, 1.00, 1.00),
            accent:      Vec4::new(0.30, 0.55, 1.00, 1.00),
            accent_dim:  Vec4::new(0.20, 0.35, 0.65, 0.80),
            error:       Vec4::new(1.00, 0.30, 0.25, 1.00),
            success:     Vec4::new(0.25, 0.90, 0.40, 1.00),
            warning:     Vec4::new(1.00, 0.80, 0.20, 1.00),
            border:      Vec4::new(0.25, 0.25, 0.30, 0.60),
            selection:   Vec4::new(0.30, 0.55, 1.00, 0.30),
            text_cursor: Vec4::new(1.00, 1.00, 1.00, 0.80),
            separator:   Vec4::new(0.20, 0.20, 0.25, 0.50),
            shadow:      Vec4::new(0.00, 0.00, 0.00, 0.40),
        }
    }

    pub fn light() -> Self {
        Self {
            bg:          Vec4::new(0.94, 0.94, 0.96, 0.95),
            bg_hover:    Vec4::new(0.88, 0.88, 0.92, 0.98),
            bg_active:   Vec4::new(0.82, 0.84, 0.90, 1.00),
            fg:          Vec4::new(0.10, 0.10, 0.12, 1.00),
            fg_dim:      Vec4::new(0.40, 0.40, 0.45, 0.80),
            fg_bright:   Vec4::new(0.00, 0.00, 0.00, 1.00),
            accent:      Vec4::new(0.15, 0.45, 0.90, 1.00),
            accent_dim:  Vec4::new(0.30, 0.50, 0.75, 0.70),
            error:       Vec4::new(0.85, 0.20, 0.15, 1.00),
            success:     Vec4::new(0.15, 0.70, 0.30, 1.00),
            warning:     Vec4::new(0.85, 0.65, 0.10, 1.00),
            border:      Vec4::new(0.70, 0.70, 0.75, 0.50),
            selection:   Vec4::new(0.15, 0.45, 0.90, 0.25),
            text_cursor: Vec4::new(0.00, 0.00, 0.00, 0.80),
            separator:   Vec4::new(0.75, 0.75, 0.80, 0.40),
            shadow:      Vec4::new(0.00, 0.00, 0.00, 0.15),
        }
    }

    pub fn high_contrast() -> Self {
        Self {
            bg:          Vec4::new(0.00, 0.00, 0.00, 1.00),
            bg_hover:    Vec4::new(0.15, 0.15, 0.15, 1.00),
            bg_active:   Vec4::new(0.25, 0.25, 0.00, 1.00),
            fg:          Vec4::new(1.00, 1.00, 1.00, 1.00),
            fg_dim:      Vec4::new(0.70, 0.70, 0.70, 1.00),
            fg_bright:   Vec4::new(1.00, 1.00, 0.00, 1.00),
            accent:      Vec4::new(0.00, 1.00, 1.00, 1.00),
            accent_dim:  Vec4::new(0.00, 0.60, 0.60, 1.00),
            error:       Vec4::new(1.00, 0.00, 0.00, 1.00),
            success:     Vec4::new(0.00, 1.00, 0.00, 1.00),
            warning:     Vec4::new(1.00, 1.00, 0.00, 1.00),
            border:      Vec4::new(1.00, 1.00, 1.00, 0.80),
            selection:   Vec4::new(1.00, 1.00, 0.00, 0.40),
            text_cursor: Vec4::new(1.00, 1.00, 0.00, 1.00),
            separator:   Vec4::new(0.50, 0.50, 0.50, 0.80),
            shadow:      Vec4::new(0.00, 0.00, 0.00, 0.00),
        }
    }
}

/// Shared widget drawing utilities.
pub struct WidgetDraw;

impl WidgetDraw {
    /// Draw text at a position on a specific layer, returning the width in world units.
    pub fn text(engine: &mut ProofEngine, x: f32, y: f32, text: &str, color: Vec4, emission: f32, layer: RenderLayer) -> f32 {
        let char_w = 0.42;
        for (i, ch) in text.chars().enumerate() {
            if ch == ' ' { continue; }
            engine.spawn_glyph(Glyph {
                character: ch,
                position: Vec3::new(x + i as f32 * char_w, y, 1.5),
                color,
                emission,
                layer,
                lifetime: 0.02,
                ..Default::default()
            });
        }
        text.len() as f32 * char_w
    }

    /// Draw a filled rectangle.
    pub fn fill_rect(engine: &mut ProofEngine, rect: Rect, color: Vec4) {
        let char_w = 0.42;
        let char_h = 0.55;
        let cols = (rect.w / char_w) as usize;
        let rows = (rect.h / char_h) as usize;
        for row in 0..rows.max(1) {
            for col in 0..cols.max(1) {
                engine.spawn_glyph(Glyph {
                    character: '█',
                    position: Vec3::new(rect.x + col as f32 * char_w, rect.y - row as f32 * char_h, 1.2),
                    color,
                    emission: 0.0,
                    layer: RenderLayer::UI,
                    lifetime: 0.02,
                    ..Default::default()
                });
            }
        }
    }

    /// Draw a border rectangle.
    pub fn border_rect(engine: &mut ProofEngine, rect: Rect, color: Vec4) {
        let char_w = 0.42;
        let char_h = 0.55;
        let cols = (rect.w / char_w) as usize;
        let rows = (rect.h / char_h) as usize;

        for col in 0..cols {
            let ch_top = if col == 0 { '┌' } else if col == cols - 1 { '┐' } else { '─' };
            let ch_bot = if col == 0 { '└' } else if col == cols - 1 { '┘' } else { '─' };
            engine.spawn_glyph(Glyph {
                character: ch_top,
                position: Vec3::new(rect.x + col as f32 * char_w, rect.y, 1.3),
                color, emission: 0.05, layer: RenderLayer::UI, lifetime: 0.02,
                ..Default::default()
            });
            engine.spawn_glyph(Glyph {
                character: ch_bot,
                position: Vec3::new(rect.x + col as f32 * char_w, rect.y - (rows - 1).max(1) as f32 * char_h, 1.3),
                color, emission: 0.05, layer: RenderLayer::UI, lifetime: 0.02,
                ..Default::default()
            });
        }
        for row in 1..rows.saturating_sub(1) {
            engine.spawn_glyph(Glyph {
                character: '│',
                position: Vec3::new(rect.x, rect.y - row as f32 * char_h, 1.3),
                color, emission: 0.05, layer: RenderLayer::UI, lifetime: 0.02,
                ..Default::default()
            });
            engine.spawn_glyph(Glyph {
                character: '│',
                position: Vec3::new(rect.x + (cols - 1).max(1) as f32 * char_w, rect.y - row as f32 * char_h, 1.3),
                color, emission: 0.05, layer: RenderLayer::UI, lifetime: 0.02,
                ..Default::default()
            });
        }
    }

    /// Draw a horizontal separator line.
    pub fn separator(engine: &mut ProofEngine, x: f32, y: f32, width: f32, color: Vec4) {
        let char_w = 0.42;
        let cols = (width / char_w) as usize;
        for col in 0..cols {
            engine.spawn_glyph(Glyph {
                character: '─',
                position: Vec3::new(x + col as f32 * char_w, y, 1.3),
                color, emission: 0.02, layer: RenderLayer::UI, lifetime: 0.02,
                ..Default::default()
            });
        }
    }

    /// Draw a horizontal progress/fill bar.
    pub fn bar(engine: &mut ProofEngine, x: f32, y: f32, width: f32, fill: f32, fill_color: Vec4, bg_color: Vec4) {
        let char_w = 0.42;
        let cols = (width / char_w) as usize;
        let filled = (cols as f32 * fill.clamp(0.0, 1.0)) as usize;
        for col in 0..cols {
            let (ch, color) = if col < filled {
                ('█', fill_color)
            } else {
                ('░', bg_color)
            };
            engine.spawn_glyph(Glyph {
                character: ch,
                position: Vec3::new(x + col as f32 * char_w, y, 1.3),
                color, emission: if col < filled { 0.15 } else { 0.0 },
                layer: RenderLayer::UI, lifetime: 0.02,
                ..Default::default()
            });
        }
    }

    /// Draw a color swatch (small filled rectangle).
    pub fn color_swatch(engine: &mut ProofEngine, x: f32, y: f32, color: Vec4) {
        for dx in 0..3 {
            engine.spawn_glyph(Glyph {
                character: '█',
                position: Vec3::new(x + dx as f32 * 0.42, y, 1.3),
                color, emission: 0.2, layer: RenderLayer::UI, lifetime: 0.02,
                ..Default::default()
            });
        }
    }
}
