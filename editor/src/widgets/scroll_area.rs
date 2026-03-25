//! Scroll area — clipped region with vertical scrollbar.

use glam::Vec4;
use proof_engine::prelude::*;
use proof_engine::prelude::RenderLayer;
use super::{Rect, WidgetTheme, WidgetDraw};

pub struct ScrollArea {
    pub rect: Rect,
    pub content_height: f32,
    pub scroll_offset: f32,
    pub scrollbar_dragging: bool,
}

impl ScrollArea {
    pub fn new(rect: Rect) -> Self {
        Self { rect, content_height: 0.0, scroll_offset: 0.0, scrollbar_dragging: false }
    }

    pub fn set_content_height(&mut self, h: f32) {
        self.content_height = h;
        let max_scroll = (self.content_height - self.rect.h).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);
    }

    pub fn scroll_by(&mut self, delta: f32) {
        self.scroll_offset += delta;
        let max = (self.content_height - self.rect.h).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max);
    }

    pub fn needs_scrollbar(&self) -> bool {
        self.content_height > self.rect.h
    }

    /// Transform a content Y position to screen Y (for clipping).
    pub fn content_to_screen_y(&self, content_y: f32) -> f32 {
        self.rect.y - content_y + self.scroll_offset
    }

    /// Is a content Y position visible?
    pub fn is_visible(&self, content_y: f32, item_height: f32) -> bool {
        let screen_y = self.content_to_screen_y(content_y);
        screen_y > self.rect.y - self.rect.h && screen_y + item_height < self.rect.y + 0.1
    }

    pub fn render_scrollbar(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        if !self.needs_scrollbar() { return; }

        let bar_x = self.rect.right() - 0.42;
        let bar_h = self.rect.h;
        let thumb_frac = self.rect.h / self.content_height;
        let thumb_h = (bar_h * thumb_frac).max(1.0);
        let thumb_offset = (self.scroll_offset / self.content_height) * bar_h;

        // Track
        let rows = (bar_h / 0.55) as usize;
        for i in 0..rows {
            WidgetDraw::text(engine, bar_x, self.rect.y - i as f32 * 0.55, "│",
                theme.bg, 0.0, RenderLayer::UI);
        }

        // Thumb
        let thumb_rows = (thumb_h / 0.55) as usize;
        for i in 0..thumb_rows.max(1) {
            WidgetDraw::text(engine, bar_x, self.rect.y - thumb_offset - i as f32 * 0.55, "█",
                theme.fg_dim, 0.1, RenderLayer::UI);
        }
    }
}
