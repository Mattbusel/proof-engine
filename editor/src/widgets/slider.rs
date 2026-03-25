//! Slider widget — horizontal value slider with numeric display and drag.

use glam::Vec4;
use proof_engine::prelude::*;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};

pub struct Slider {
    pub label: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub rect: Rect,
    pub dragging: bool,
    pub precision: usize,
}

impl Slider {
    pub fn new(label: &str, value: f32, min: f32, max: f32, x: f32, y: f32, width: f32) -> Self {
        Self {
            label: label.to_string(),
            value, min, max,
            step: 0.01,
            rect: Rect::new(x, y, width, 0.55),
            dragging: false,
            precision: 2,
        }
    }

    pub fn with_step(mut self, step: f32) -> Self { self.step = step; self }
    pub fn with_precision(mut self, p: usize) -> Self { self.precision = p; self }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, mouse_down: bool, mouse_just_pressed: bool) -> WidgetResponse {
        let hovered = self.rect.contains(mouse_x, mouse_y);

        if mouse_just_pressed && hovered {
            self.dragging = true;
        }
        if !mouse_down {
            self.dragging = false;
        }

        if self.dragging {
            let track_x = self.rect.x + self.label.len() as f32 * 0.42 + 0.5;
            let track_w = self.rect.w - self.label.len() as f32 * 0.42 - 3.5;
            let t = ((mouse_x - track_x) / track_w).clamp(0.0, 1.0);
            let new_value = self.min + t * (self.max - self.min);
            let snapped = (new_value / self.step).round() * self.step;
            if (snapped - self.value).abs() > self.step * 0.1 {
                self.value = snapped.clamp(self.min, self.max);
                return WidgetResponse::ValueChanged(self.value);
            }
        }

        WidgetResponse::None
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        // Label
        WidgetDraw::text(engine, self.rect.x, self.rect.y, &self.label, theme.fg, 0.1);

        // Track
        let track_x = self.rect.x + self.label.len() as f32 * 0.42 + 0.5;
        let track_w = self.rect.w - self.label.len() as f32 * 0.42 - 3.5;
        let fill = (self.value - self.min) / (self.max - self.min);
        WidgetDraw::bar(engine, track_x, self.rect.y, track_w, fill, theme.accent, theme.bg);

        // Handle (bright char at current position)
        let handle_x = track_x + track_w * fill;
        WidgetDraw::text(engine, handle_x - 0.1, self.rect.y, "|", theme.fg_bright, 0.4);

        // Value display
        let val_text = format!("{:.*}", self.precision, self.value);
        let val_x = track_x + track_w + 0.3;
        WidgetDraw::text(engine, val_x, self.rect.y, &val_text, theme.fg, 0.1);
    }

    pub fn set_value(&mut self, v: f32) {
        self.value = v.clamp(self.min, self.max);
    }
}

/// A compact number input with +/- buttons.
pub struct NumberInput {
    pub label: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub rect: Rect,
    pub precision: usize,
}

impl NumberInput {
    pub fn new(label: &str, value: f32, min: f32, max: f32, step: f32, x: f32, y: f32) -> Self {
        Self {
            label: label.to_string(),
            value, min, max, step,
            rect: Rect::new(x, y, label.len() as f32 * 0.42 + 5.0, 0.55),
            precision: 2,
        }
    }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, clicked: bool, theme: &WidgetTheme) -> WidgetResponse {
        if !clicked { return WidgetResponse::None; }

        let val_x = self.rect.x + self.label.len() as f32 * 0.42 + 0.5;
        let minus_rect = Rect::new(val_x, self.rect.y, 0.42, 0.55);
        let plus_rect = Rect::new(val_x + 3.5, self.rect.y, 0.42, 0.55);

        if minus_rect.contains(mouse_x, mouse_y) {
            self.value = (self.value - self.step).max(self.min);
            return WidgetResponse::ValueChanged(self.value);
        }
        if plus_rect.contains(mouse_x, mouse_y) {
            self.value = (self.value + self.step).min(self.max);
            return WidgetResponse::ValueChanged(self.value);
        }

        WidgetResponse::None
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        WidgetDraw::text(engine, self.rect.x, self.rect.y, &self.label, theme.fg, 0.1);
        let val_x = self.rect.x + self.label.len() as f32 * 0.42 + 0.5;
        WidgetDraw::text(engine, val_x, self.rect.y, "-", theme.accent, 0.2);
        let val_text = format!("{:.*}", self.precision, self.value);
        WidgetDraw::text(engine, val_x + 0.7, self.rect.y, &val_text, theme.fg_bright, 0.15);
        WidgetDraw::text(engine, val_x + 3.5, self.rect.y, "+", theme.accent, 0.2);
    }
}

/// Vec3 input (three sliders for X, Y, Z).
pub struct Vec3Input {
    pub label: String,
    pub x_slider: Slider,
    pub y_slider: Slider,
    pub z_slider: Slider,
}

impl Vec3Input {
    pub fn new(label: &str, value: glam::Vec3, min: f32, max: f32, x: f32, y: f32, width: f32) -> Self {
        Self {
            label: label.to_string(),
            x_slider: Slider::new("X", value.x, min, max, x + 2.0, y, width - 2.0),
            y_slider: Slider::new("Y", value.y, min, max, x + 2.0, y - 0.6, width - 2.0),
            z_slider: Slider::new("Z", value.z, min, max, x + 2.0, y - 1.2, width - 2.0),
        }
    }

    pub fn value(&self) -> glam::Vec3 {
        glam::Vec3::new(self.x_slider.value, self.y_slider.value, self.z_slider.value)
    }

    pub fn set_value(&mut self, v: glam::Vec3) {
        self.x_slider.set_value(v.x);
        self.y_slider.set_value(v.y);
        self.z_slider.set_value(v.z);
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        WidgetDraw::text(engine, self.x_slider.rect.x - 2.0, self.x_slider.rect.y, &self.label, theme.fg, 0.15);
        self.x_slider.render(engine, theme);
        self.y_slider.render(engine, theme);
        self.z_slider.render(engine, theme);
    }

    pub fn height(&self) -> f32 { 1.8 }
}
