//! Button widget — clickable with hover/active states.

use glam::Vec4;
use proof_engine::prelude::*;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};

/// A button's visual state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState { Normal, Hovered, Active, Disabled }

/// Button configuration.
pub struct Button {
    pub label: String,
    pub icon: Option<char>,
    pub rect: Rect,
    pub state: ButtonState,
    pub selected: bool,
    pub shortcut: Option<String>,
}

impl Button {
    pub fn new(label: &str, x: f32, y: f32, width: f32) -> Self {
        Self {
            label: label.to_string(),
            icon: None,
            rect: Rect::new(x, y, width, 0.55),
            state: ButtonState::Normal,
            selected: false,
            shortcut: None,
        }
    }

    pub fn with_icon(mut self, icon: char) -> Self { self.icon = Some(icon); self }
    pub fn with_shortcut(mut self, shortcut: &str) -> Self { self.shortcut = Some(shortcut.to_string()); self }
    pub fn with_selected(mut self, selected: bool) -> Self { self.selected = selected; self }

    /// Update state from mouse position and click.
    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, clicked: bool, _theme: &WidgetTheme) -> WidgetResponse {
        let hovered = self.rect.contains(mouse_x, mouse_y);
        self.state = if self.state == ButtonState::Disabled {
            ButtonState::Disabled
        } else if hovered && clicked {
            ButtonState::Active
        } else if hovered {
            ButtonState::Hovered
        } else {
            ButtonState::Normal
        };

        if hovered && clicked {
            WidgetResponse::Clicked
        } else {
            WidgetResponse::None
        }
    }

    /// Render the button.
    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        let bg = match self.state {
            ButtonState::Normal => if self.selected { theme.bg_active } else { theme.bg },
            ButtonState::Hovered => theme.bg_hover,
            ButtonState::Active => theme.accent_dim,
            ButtonState::Disabled => {
                let mut c = theme.bg;
                c.w *= 0.5;
                c
            }
        };

        let fg = match self.state {
            ButtonState::Disabled => theme.fg_dim,
            ButtonState::Active | ButtonState::Hovered => theme.fg_bright,
            _ => if self.selected { theme.accent } else { theme.fg },
        };

        // Background
        WidgetDraw::fill_rect(engine, self.rect, bg);

        // Icon + label
        let mut text_x = self.rect.x + 0.2;
        if let Some(icon) = self.icon {
            WidgetDraw::text(engine, text_x, self.rect.y, &icon.to_string(), fg, 0.15);
            text_x += 0.6;
        }
        WidgetDraw::text(engine, text_x, self.rect.y, &self.label, fg, if self.selected { 0.3 } else { 0.1 });

        // Shortcut hint (right-aligned)
        if let Some(ref sc) = self.shortcut {
            let sc_x = self.rect.right() - sc.len() as f32 * 0.42 - 0.2;
            WidgetDraw::text(engine, sc_x, self.rect.y, sc, theme.fg_dim, 0.05);
        }

        // Border on hover/selected
        if self.state == ButtonState::Hovered || self.selected {
            WidgetDraw::border_rect(engine, self.rect, theme.border);
        }
    }
}

/// A toolbar of buttons (horizontal row).
pub struct Toolbar {
    pub buttons: Vec<Button>,
    pub x: f32,
    pub y: f32,
    pub spacing: f32,
}

impl Toolbar {
    pub fn new(x: f32, y: f32) -> Self {
        Self { buttons: Vec::new(), x, y, spacing: 0.3 }
    }

    pub fn add(&mut self, label: &str, width: f32) -> usize {
        let offset: f32 = self.buttons.iter().map(|b| b.rect.w + self.spacing).sum();
        let btn = Button::new(label, self.x + offset, self.y, width);
        self.buttons.push(btn);
        self.buttons.len() - 1
    }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, clicked: bool, theme: &WidgetTheme) -> Option<usize> {
        for (i, btn) in self.buttons.iter_mut().enumerate() {
            if let WidgetResponse::Clicked = btn.update(mouse_x, mouse_y, clicked, theme) {
                return Some(i);
            }
        }
        None
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        for btn in &self.buttons {
            btn.render(engine, theme);
        }
    }
}

/// An icon button (single character, square).
pub struct IconButton {
    pub icon: char,
    pub tooltip: String,
    pub rect: Rect,
    pub state: ButtonState,
    pub active: bool,
}

impl IconButton {
    pub fn new(icon: char, tooltip: &str, x: f32, y: f32) -> Self {
        Self {
            icon, tooltip: tooltip.to_string(),
            rect: Rect::new(x, y, 0.55, 0.55),
            state: ButtonState::Normal,
            active: false,
        }
    }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, clicked: bool) -> WidgetResponse {
        let hovered = self.rect.contains(mouse_x, mouse_y);
        self.state = if hovered && clicked { ButtonState::Active }
            else if hovered { ButtonState::Hovered }
            else { ButtonState::Normal };
        if hovered && clicked { WidgetResponse::Clicked } else { WidgetResponse::None }
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        let bg = if self.active { theme.accent_dim } else if self.state == ButtonState::Hovered { theme.bg_hover } else { theme.bg };
        let fg = if self.active { theme.accent } else { theme.fg };
        WidgetDraw::fill_rect(engine, self.rect, bg);
        WidgetDraw::text(engine, self.rect.x + 0.08, self.rect.y, &self.icon.to_string(), fg, if self.active { 0.3 } else { 0.1 });
    }
}
