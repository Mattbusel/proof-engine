//! Modal dialog widget — centered overlay with title, message, buttons.

use glam::Vec4;
use proof_engine::prelude::*;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult { None, Confirm, Cancel, Option1, Option2, Option3 }

pub struct ModalDialog {
    pub title: String,
    pub message: String,
    pub buttons: Vec<String>,
    pub visible: bool,
    pub rect: Rect,
    pub result: DialogResult,
}

impl ModalDialog {
    pub fn confirm(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            buttons: vec!["OK".to_string(), "Cancel".to_string()],
            visible: true,
            rect: Rect::new(-6.0, 3.0, 12.0, 6.0),
            result: DialogResult::None,
        }
    }

    pub fn info(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
            buttons: vec!["OK".to_string()],
            visible: true,
            rect: Rect::new(-5.0, 2.0, 10.0, 4.0),
            result: DialogResult::None,
        }
    }

    pub fn update(&mut self, mouse_x: f32, mouse_y: f32, clicked: bool, _theme: &WidgetTheme) -> DialogResult {
        if !self.visible { return DialogResult::None; }

        if clicked {
            // Check button clicks
            let btn_y = self.rect.y - self.rect.h + 1.0;
            let mut btn_x = self.rect.x + 1.0;
            for (i, btn) in self.buttons.iter().enumerate() {
                let btn_w = btn.len() as f32 * 0.42 + 1.0;
                let btn_rect = Rect::new(btn_x, btn_y, btn_w, 0.55);
                if btn_rect.contains(mouse_x, mouse_y) {
                    self.visible = false;
                    return match i {
                        0 => DialogResult::Confirm,
                        1 => DialogResult::Cancel,
                        2 => DialogResult::Option1,
                        _ => DialogResult::None,
                    };
                }
                btn_x += btn_w + 0.5;
            }
        }

        DialogResult::None
    }

    pub fn render(&self, engine: &mut ProofEngine, theme: &WidgetTheme) {
        if !self.visible { return; }

        // Backdrop (darken)
        WidgetDraw::fill_rect(engine, Rect::new(-20.0, 15.0, 40.0, 30.0), theme.shadow);

        // Dialog box
        WidgetDraw::fill_rect(engine, self.rect, theme.bg);
        WidgetDraw::border_rect(engine, self.rect, theme.accent);

        // Title
        WidgetDraw::text(engine, self.rect.x + 0.5, self.rect.y - 0.3, &self.title, theme.accent, 0.3);

        // Separator
        WidgetDraw::separator(engine, self.rect.x + 0.3, self.rect.y - 1.0, self.rect.w - 0.6, theme.separator);

        // Message
        WidgetDraw::text(engine, self.rect.x + 0.5, self.rect.y - 1.5, &self.message, theme.fg, 0.1);

        // Buttons
        let btn_y = self.rect.y - self.rect.h + 1.0;
        let mut btn_x = self.rect.x + 1.0;
        for btn in &self.buttons {
            let btn_w = btn.len() as f32 * 0.42 + 1.0;
            WidgetDraw::fill_rect(engine, Rect::new(btn_x, btn_y, btn_w, 0.55), theme.bg_hover);
            WidgetDraw::border_rect(engine, Rect::new(btn_x, btn_y, btn_w, 0.55), theme.border);
            WidgetDraw::text(engine, btn_x + 0.5, btn_y, btn, theme.fg_bright, 0.15);
            btn_x += btn_w + 0.5;
        }
    }
}
