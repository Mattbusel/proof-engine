//! Common small widgets — toggle, separator, label, tooltip, progress bar.

use glam::Vec4;
use proof_engine::prelude::*;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};

// ── Toggle / Checkbox ───────────────────────────────────────────────────────

pub struct Toggle {
    pub label: String,
    pub value: bool,
    pub rect: Rect,
}

impl Toggle {
    pub fn new(label: &str, value: bool, x: f32, y: f32) -> Self {
        Self {
            label: label.to_string(), value,
            rect: Rect::new(x, y, label.len() as f32 * 0.42 + 2.0, 0.55),
        }
    }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, clicked: bool) -> WidgetResponse {
        if clicked && self.rect.contains(mouse_x, mouse_y) {
            self.value = !self.value;
            WidgetResponse::Toggled(self.value)
        } else {
            WidgetResponse::None
        }
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        let check = if self.value { "[X]" } else { "[ ]" };
        let check_color = if self.value { theme.accent } else { theme.fg_dim };
        WidgetDraw::text(engine, self.rect.x, self.rect.y, check, check_color, if self.value { 0.3 } else { 0.1 }, RenderLayer::UI);
        WidgetDraw::text(engine, self.rect.x + 1.5, self.rect.y, &self.label, theme.fg, 0.1, RenderLayer::UI);
    }
}

// ── Label ───────────────────────────────────────────────────────────────────

pub struct Label {
    pub text: String,
    pub color: Vec4,
    pub emission: f32,
}

impl Label {
    pub fn new(text: &str) -> Self {
        Self { text: text.to_string(), color: Vec4::new(0.8, 0.8, 0.85, 1.0), emission: 0.1 }
    }

    pub fn heading(text: &str) -> Self {
        Self { text: text.to_string(), color: Vec4::new(0.5, 0.7, 1.0, 1.0), emission: 0.3 }
    }

    pub fn dim(text: &str) -> Self {
        Self { text: text.to_string(), color: Vec4::new(0.45, 0.45, 0.5, 0.7), emission: 0.05 }
    }

    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32) {
        WidgetDraw::text(engine, x, y, &self.text, self.color, self.emission, RenderLayer::UI);
    }
}

// ── Separator ───────────────────────────────────────────────────────────────

pub struct Separator;

impl Separator {
    pub fn render(engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        WidgetDraw::separator(engine, x, y, width, theme.separator);
    }
}

// ── Tooltip ─────────────────────────────────────────────────────────────────

pub struct Tooltip {
    pub text: String,
    pub visible: bool,
    pub x: f32,
    pub y: f32,
}

impl Tooltip {
    pub fn new(text: &str) -> Self {
        Self { text: text.to_string(), visible: false, x: 0.0, y: 0.0 }
    }

    pub fn show_at(&mut self, x: f32, y: f32) {
        self.visible = true;
        self.x = x;
        self.y = y;
    }

    pub fn hide(&mut self) { self.visible = false; }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        if !self.visible { return; }
        let w = self.text.len() as f32 * 0.42 + 0.6;
        WidgetDraw::fill_rect(engine, Rect::new(self.x, self.y, w, 0.6), theme.bg);
        WidgetDraw::border_rect(engine, Rect::new(self.x, self.y, w, 0.6), theme.border);
        WidgetDraw::text(engine, self.x + 0.3, self.y, &self.text, theme.fg, 0.1, RenderLayer::UI);
    }
}

// ── ProgressBar ─────────────────────────────────────────────────────────────

pub struct ProgressBar {
    pub label: String,
    pub value: f32,
    pub rect: Rect,
}

impl ProgressBar {
    pub fn new(label: &str, value: f32, x: f32, y: f32, width: f32) -> Self {
        Self { label: label.to_string(), value, rect: Rect::new(x, y, width, 0.55) }
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        WidgetDraw::text(engine, self.rect.x, self.rect.y, &self.label, theme.fg, 0.1, RenderLayer::UI);
        let bar_x = self.rect.x + self.label.len() as f32 * 0.42 + 0.5;
        let bar_w = self.rect.w - self.label.len() as f32 * 0.42 - 2.0;
        WidgetDraw::bar(engine, bar_x, self.rect.y, bar_w, self.value, theme.accent, theme.bg);
        let pct = format!("{}%", (self.value * 100.0) as u32);
        WidgetDraw::text(engine, bar_x + bar_w + 0.3, self.rect.y, &pct, theme.fg_dim, 0.1, RenderLayer::UI);
    }
}
