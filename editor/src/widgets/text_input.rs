//! Text input widget — single-line editable text field.

use glam::Vec4;
use proof_engine::prelude::*;
use proof_engine::input::Key;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};

pub struct TextInput {
    pub label: String,
    pub text: String,
    pub placeholder: String,
    pub rect: Rect,
    pub focused: bool,
    pub cursor: usize,
    pub max_length: usize,
    blink_timer: f32,
}

impl TextInput {
    pub fn new(label: &str, text: &str, x: f32, y: f32, width: f32) -> Self {
        Self {
            label: label.to_string(),
            text: text.to_string(),
            placeholder: String::new(),
            rect: Rect::new(x, y, width, 0.55),
            focused: false,
            cursor: text.len(),
            max_length: 128,
            blink_timer: 0.0,
        }
    }

    pub fn with_placeholder(mut self, p: &str) -> Self { self.placeholder = p.to_string(); self }

    pub fn update(&mut self, input: &proof_engine::input::InputState, dt: f32) -> WidgetResponse {
        self.blink_timer += dt;

        // Focus on click
        if input.mouse_left_just_pressed {
            self.focused = self.rect.contains(input.mouse_x, input.mouse_y);
            if self.focused {
                self.cursor = self.text.len();
                self.blink_timer = 0.0;
            }
        }

        if !self.focused { return WidgetResponse::None; }

        let mut changed = false;

        // Typed characters — check printable keys via just_pressed
        // (InputState doesn't have a chars_typed field; we check keys directly)
        let printable: &[Key] = &[
            Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H,
            Key::I, Key::J, Key::K, Key::L, Key::M, Key::N, Key::O, Key::P,
            Key::Q, Key::R, Key::S, Key::T, Key::U, Key::V, Key::W, Key::X,
            Key::Y, Key::Z, Key::Num0, Key::Num1, Key::Num2, Key::Num3,
            Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9,
            Key::Space,
        ];
        for &key in printable {
            if input.just_pressed(key) && self.text.len() < self.max_length {
                let ch = key.display_name().chars().next().unwrap_or(' ');
                let ch = if input.shift() { ch } else { ch.to_ascii_lowercase() };
                self.text.insert(self.cursor, ch);
                self.cursor += 1;
                changed = true;
            }
        }

        // Backspace
        if input.just_pressed(Key::Backspace) && self.cursor > 0 {
            self.cursor -= 1;
            self.text.remove(self.cursor);
            changed = true;
        }

        // Delete
        if input.just_pressed(Key::Delete) && self.cursor < self.text.len() {
            self.text.remove(self.cursor);
            changed = true;
        }

        // Arrow keys
        if input.just_pressed(Key::Left) && self.cursor > 0 { self.cursor -= 1; }
        if input.just_pressed(Key::Right) && self.cursor < self.text.len() { self.cursor += 1; }
        if input.just_pressed(Key::Home) { self.cursor = 0; }
        if input.just_pressed(Key::End) { self.cursor = self.text.len(); }

        // Enter = submit
        if input.just_pressed(Key::Enter) {
            self.focused = false;
            return WidgetResponse::Submitted;
        }

        // Escape = cancel
        if input.just_pressed(Key::Escape) {
            self.focused = false;
            return WidgetResponse::Cancelled;
        }

        if changed {
            self.blink_timer = 0.0;
            WidgetResponse::TextChanged(self.text.clone())
        } else {
            WidgetResponse::None
        }
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        let label_w = self.label.len() as f32 * 0.42;
        WidgetDraw::text(engine, self.rect.x, self.rect.y, &self.label, theme.fg, 0.1);

        let field_x = self.rect.x + label_w + 0.5;
        let field_w = self.rect.w - label_w - 0.5;
        let field_rect = Rect::new(field_x, self.rect.y, field_w, 0.55);

        // Background
        let bg = if self.focused { theme.bg_active } else { theme.bg };
        WidgetDraw::fill_rect(engine, field_rect, bg);
        WidgetDraw::border_rect(engine, field_rect, if self.focused { theme.accent } else { theme.border });

        // Text or placeholder
        let display_text = if self.text.is_empty() && !self.focused {
            &self.placeholder
        } else {
            &self.text
        };
        let text_color = if self.text.is_empty() && !self.focused { theme.fg_dim } else { theme.fg };
        WidgetDraw::text(engine, field_x + 0.2, self.rect.y, display_text, text_color, 0.1);

        // Cursor
        if self.focused && ((self.blink_timer * 2.0) as u32 % 2 == 0) {
            let cursor_x = field_x + 0.2 + self.cursor as f32 * 0.42;
            WidgetDraw::text(engine, cursor_x, self.rect.y, "|", theme.text_cursor, 0.4);
        }
    }
}
