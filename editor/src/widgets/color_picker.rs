//! Color picker widget — RGB sliders + preview swatch + hex input.

use glam::Vec4;
use proof_engine::prelude::*;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};
use super::slider::Slider;

pub struct ColorPicker {
    pub label: String,
    pub color: Vec4,
    pub r_slider: Slider,
    pub g_slider: Slider,
    pub b_slider: Slider,
    pub a_slider: Slider,
    pub rect: Rect,
    pub expanded: bool,
}

impl ColorPicker {
    pub fn new(label: &str, color: Vec4, x: f32, y: f32, width: f32) -> Self {
        Self {
            label: label.to_string(),
            color,
            r_slider: Slider::new("R", color.x, 0.0, 1.0, x + 1.5, y - 0.6, width - 1.5).with_step(0.01),
            g_slider: Slider::new("G", color.y, 0.0, 1.0, x + 1.5, y - 1.2, width - 1.5).with_step(0.01),
            b_slider: Slider::new("B", color.z, 0.0, 1.0, x + 1.5, y - 1.8, width - 1.5).with_step(0.01),
            a_slider: Slider::new("A", color.w, 0.0, 1.0, x + 1.5, y - 2.4, width - 1.5).with_step(0.01),
            rect: Rect::new(x, y, width, 0.55),
            expanded: false,
        }
    }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, mouse_down: bool, clicked: bool) -> WidgetResponse {
        // Toggle expand on label click
        if clicked && self.rect.contains(mouse_x, mouse_y) {
            self.expanded = !self.expanded;
        }

        if !self.expanded { return WidgetResponse::None; }

        let mut changed = false;
        if let WidgetResponse::ValueChanged(v) = self.r_slider.update(mouse_x, mouse_y, mouse_down, clicked) {
            self.color.x = v; changed = true;
        }
        if let WidgetResponse::ValueChanged(v) = self.g_slider.update(mouse_x, mouse_y, mouse_down, clicked) {
            self.color.y = v; changed = true;
        }
        if let WidgetResponse::ValueChanged(v) = self.b_slider.update(mouse_x, mouse_y, mouse_down, clicked) {
            self.color.z = v; changed = true;
        }
        if let WidgetResponse::ValueChanged(v) = self.a_slider.update(mouse_x, mouse_y, mouse_down, clicked) {
            self.color.w = v; changed = true;
        }

        if changed {
            WidgetResponse::ColorChanged(self.color)
        } else {
            WidgetResponse::None
        }
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        // Label + swatch
        let arrow = if self.expanded { "v" } else { ">" };
        WidgetDraw::text(engine, self.rect.x, self.rect.y, arrow, theme.fg_dim, 0.1);
        WidgetDraw::text(engine, self.rect.x + 0.5, self.rect.y, &self.label, theme.fg, 0.1);
        WidgetDraw::color_swatch(engine, self.rect.right() - 1.5, self.rect.y, self.color);

        if self.expanded {
            self.r_slider.render(engine, theme);
            self.g_slider.render(engine, theme);
            self.b_slider.render(engine, theme);
            self.a_slider.render(engine, theme);

            // Hex display
            let hex = format!("#{:02X}{:02X}{:02X}",
                (self.color.x * 255.0) as u8,
                (self.color.y * 255.0) as u8,
                (self.color.z * 255.0) as u8,
            );
            WidgetDraw::text(engine, self.rect.x + 1.5, self.rect.y - 3.0, &hex, theme.fg_dim, 0.1);

            // Preset swatches
            let presets = [
                Vec4::new(1.0, 0.0, 0.0, 1.0), Vec4::new(0.0, 1.0, 0.0, 1.0),
                Vec4::new(0.0, 0.0, 1.0, 1.0), Vec4::new(1.0, 1.0, 0.0, 1.0),
                Vec4::new(1.0, 0.0, 1.0, 1.0), Vec4::new(0.0, 1.0, 1.0, 1.0),
                Vec4::new(1.0, 1.0, 1.0, 1.0), Vec4::new(0.5, 0.5, 0.5, 1.0),
            ];
            for (i, &c) in presets.iter().enumerate() {
                WidgetDraw::color_swatch(engine, self.rect.x + 1.5 + i as f32 * 1.5, self.rect.y - 3.6, c);
            }
        }
    }

    pub fn height(&self) -> f32 {
        if self.expanded { 4.2 } else { 0.55 }
    }

    pub fn set_color(&mut self, c: Vec4) {
        self.color = c;
        self.r_slider.set_value(c.x);
        self.g_slider.set_value(c.y);
        self.b_slider.set_value(c.z);
        self.a_slider.set_value(c.w);
    }
}
