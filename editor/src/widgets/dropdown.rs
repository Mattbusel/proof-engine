//! Dropdown widget — expandable list of options.

use glam::Vec4;
use proof_engine::prelude::*;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};

pub struct Dropdown {
    pub label: String,
    pub options: Vec<String>,
    pub selected: usize,
    pub rect: Rect,
    pub expanded: bool,
}

impl Dropdown {
    pub fn new(label: &str, options: Vec<String>, selected: usize, x: f32, y: f32, width: f32) -> Self {
        Self {
            label: label.to_string(),
            options, selected,
            rect: Rect::new(x, y, width, 0.55),
            expanded: false,
        }
    }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, clicked: bool, _theme: &WidgetTheme) -> WidgetResponse {
        if clicked && self.rect.contains(mouse_x, mouse_y) {
            self.expanded = !self.expanded;
            return WidgetResponse::None;
        }

        if self.expanded && clicked {
            // Check option clicks
            for (i, _) in self.options.iter().enumerate() {
                let opt_y = self.rect.y - (i + 1) as f32 * 0.55;
                let opt_rect = Rect::new(self.rect.x, opt_y, self.rect.w, 0.55);
                if opt_rect.contains(mouse_x, mouse_y) {
                    self.selected = i;
                    self.expanded = false;
                    return WidgetResponse::IndexChanged(i);
                }
            }
            // Clicked outside — close
            self.expanded = false;
        }

        WidgetResponse::None
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        // Label
        let label_w = self.label.len() as f32 * 0.42;
        WidgetDraw::text(engine, self.rect.x, self.rect.y, &self.label, theme.fg, 0.1);

        // Current selection
        let sel_x = self.rect.x + label_w + 0.5;
        let sel_text = self.options.get(self.selected).map(|s| s.as_str()).unwrap_or("---");
        let arrow = if self.expanded { "v" } else { ">" };
        WidgetDraw::fill_rect(engine, Rect::new(sel_x, self.rect.y, self.rect.w - label_w - 0.5, 0.55), theme.bg);
        WidgetDraw::text(engine, sel_x + 0.2, self.rect.y, sel_text, theme.fg_bright, 0.15);
        WidgetDraw::text(engine, self.rect.right() - 0.6, self.rect.y, arrow, theme.fg_dim, 0.1);

        // Dropdown list
        if self.expanded {
            for (i, opt) in self.options.iter().enumerate() {
                let opt_y = self.rect.y - (i + 1) as f32 * 0.55;
                let bg = if i == self.selected { theme.selection } else { theme.bg };
                WidgetDraw::fill_rect(engine, Rect::new(sel_x, opt_y, self.rect.w - label_w - 0.5, 0.55), bg);
                let fg = if i == self.selected { theme.accent } else { theme.fg };
                WidgetDraw::text(engine, sel_x + 0.2, opt_y, opt, fg, 0.1);
            }
        }
    }

    pub fn height(&self) -> f32 {
        if self.expanded {
            0.55 + self.options.len() as f32 * 0.55
        } else {
            0.55
        }
    }

    pub fn selected_text(&self) -> &str {
        self.options.get(self.selected).map(|s| s.as_str()).unwrap_or("")
    }
}
