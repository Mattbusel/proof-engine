//! Tree node widget — expandable hierarchy item with indent.

use glam::Vec4;
use proof_engine::prelude::*;
use super::{Rect, WidgetResponse, WidgetTheme, WidgetDraw};

pub struct TreeNode {
    pub label: String,
    pub icon: char,
    pub id: u32,
    pub depth: usize,
    pub expanded: bool,
    pub selected: bool,
    pub has_children: bool,
    pub visible: bool,
    pub locked: bool,
}

impl TreeNode {
    pub fn new(label: &str, icon: char, id: u32, depth: usize) -> Self {
        Self {
            label: label.to_string(), icon, id, depth,
            expanded: true, selected: false, has_children: false,
            visible: true, locked: false,
        }
    }

    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let indent = self.depth as f32 * 0.8;
        let ix = x + indent;

        // Background on selected
        if self.selected {
            WidgetDraw::fill_rect(engine, Rect::new(x, y, width, 0.55), theme.selection);
        }

        // Expand arrow
        if self.has_children {
            let arrow = if self.expanded { "v" } else { ">" };
            WidgetDraw::text(engine, ix, y, arrow, theme.fg_dim, 0.1);
        }

        // Icon
        let icon_color = if self.selected { theme.accent } else { theme.fg };
        WidgetDraw::text(engine, ix + 0.5, y, &self.icon.to_string(), icon_color, if self.selected { 0.3 } else { 0.15 });

        // Label
        let label_color = if !self.visible {
            theme.fg_dim
        } else if self.selected {
            theme.fg_bright
        } else {
            theme.fg
        };
        WidgetDraw::text(engine, ix + 1.1, y, &self.label, label_color, if self.selected { 0.2 } else { 0.1 });

        // Visibility toggle (right side)
        if !self.visible {
            WidgetDraw::text(engine, x + width - 1.5, y, "H", theme.fg_dim, 0.05);
        }
        if self.locked {
            WidgetDraw::text(engine, x + width - 0.8, y, "L", theme.warning, 0.1);
        }
    }

    pub fn hit_test(&self, x: f32, y: f32, node_x: f32, node_y: f32, width: f32) -> Option<TreeHitZone> {
        let rect = Rect::new(node_x, node_y, width, 0.55);
        if !rect.contains(x, y) { return None; }

        let indent = self.depth as f32 * 0.8;
        let arrow_x = node_x + indent;
        if x < arrow_x + 0.5 && self.has_children {
            Some(TreeHitZone::ExpandArrow)
        } else if x > node_x + width - 1.5 {
            Some(TreeHitZone::VisibilityToggle)
        } else {
            Some(TreeHitZone::Label)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeHitZone {
    ExpandArrow,
    Label,
    VisibilityToggle,
}
