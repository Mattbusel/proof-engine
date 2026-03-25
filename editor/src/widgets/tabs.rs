//! Tab bar widget — horizontal tab strip.

use glam::Vec4;
use proof_engine::prelude::*;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};

pub struct TabBar {
    pub tabs: Vec<String>,
    pub active: usize,
    pub x: f32,
    pub y: f32,
}

impl TabBar {
    pub fn new(tabs: Vec<String>, x: f32, y: f32) -> Self {
        Self { tabs, active: 0, x, y }
    }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, clicked: bool) -> WidgetResponse {
        if !clicked { return WidgetResponse::None; }

        let mut offset = 0.0_f32;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = tab.len() as f32 * 0.42 + 1.0;
            let rect = Rect::new(self.x + offset, self.y, w, 0.55);
            if rect.contains(mouse_x, mouse_y) {
                self.active = i;
                return WidgetResponse::IndexChanged(i);
            }
            offset += w + 0.3;
        }
        WidgetResponse::None
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        let mut offset = 0.0_f32;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = tab.len() as f32 * 0.42 + 1.0;
            let active = i == self.active;

            let bg = if active { theme.bg_active } else { theme.bg };
            let fg = if active { theme.accent } else { theme.fg };
            let em = if active { 0.25 } else { 0.05 };

            WidgetDraw::fill_rect(engine, Rect::new(self.x + offset, self.y, w, 0.55), bg);
            WidgetDraw::text(engine, self.x + offset + 0.5, self.y, tab, fg, em, RenderLayer::UI);

            if active {
                // Underline
                WidgetDraw::separator(engine, self.x + offset, self.y - 0.5, w, theme.accent);
            }

            offset += w + 0.3;
        }
    }
}
