//! Asset browser — browse and place glyphs, fields, entities, particles,
//! formations, color palettes, and scene presets.

use glam::{Vec3, Vec4};
use proof_engine::prelude::*;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};
use crate::scene::FieldType;
use crate::tools::{CHAR_PALETTES, COLOR_PALETTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCategory {
    Glyphs,
    Colors,
    Fields,
    Particles,
    Formations,
    Presets,
}

impl AssetCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Glyphs => "Glyphs", Self::Colors => "Colors", Self::Fields => "Fields",
            Self::Particles => "Particles", Self::Formations => "Formations", Self::Presets => "Presets",
        }
    }
    pub fn all() -> &'static [AssetCategory] {
        &[Self::Glyphs, Self::Colors, Self::Fields, Self::Particles, Self::Formations, Self::Presets]
    }
}

pub struct AssetBrowser {
    pub active_category: AssetCategory,
    pub search: String,
    pub scroll: f32,
    pub selected_glyph: Option<char>,
    pub selected_color: Option<Vec4>,
    pub selected_field: Option<FieldType>,
    pub selected_preset: Option<String>,
}

impl AssetBrowser {
    pub fn new() -> Self {
        Self {
            active_category: AssetCategory::Glyphs,
            search: String::new(),
            scroll: 0.0,
            selected_glyph: None,
            selected_color: None,
            selected_field: None,
            selected_preset: None,
        }
    }

    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, height: f32, theme: &WidgetTheme) {
        // Background
        WidgetDraw::fill_rect(engine, Rect::new(x, y, width, height), theme.bg);
        WidgetDraw::text(engine, x + 0.3, y - 0.1, "ASSETS", theme.accent, 0.25, RenderLayer::UI);

        // Category tabs
        let mut tx = x + 0.3;
        let tab_y = y - 0.7;
        for cat in AssetCategory::all() {
            let active = *cat == self.active_category;
            let color = if active { theme.accent } else { theme.fg_dim };
            WidgetDraw::text(engine, tx, tab_y, cat.label(), color, if active { 0.2 } else { 0.06 }, RenderLayer::UI);
            tx += cat.label().len() as f32 * 0.42 + 0.5;
        }
        WidgetDraw::separator(engine, x + 0.2, tab_y - 0.4, width - 0.4, theme.separator);

        // Content
        let content_y = tab_y - 0.8;
        match self.active_category {
            AssetCategory::Glyphs => self.render_glyphs(engine, x, content_y, width, theme),
            AssetCategory::Colors => self.render_colors(engine, x, content_y, width, theme),
            AssetCategory::Fields => self.render_fields(engine, x, content_y, width, theme),
            AssetCategory::Particles => self.render_particles(engine, x, content_y, width, theme),
            AssetCategory::Formations => self.render_formations(engine, x, content_y, width, theme),
            AssetCategory::Presets => self.render_presets(engine, x, content_y, width, theme),
        }
    }

    fn render_glyphs(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut cy = y;
        for (name, chars) in CHAR_PALETTES {
            WidgetDraw::text(engine, x + 0.3, cy, name, theme.fg, 0.12, RenderLayer::UI);
            cy -= 0.5;
            let mut cx = x + 0.5;
            for &ch in *chars {
                let selected = self.selected_glyph == Some(ch);
                let color = if selected { theme.accent } else { theme.fg };
                WidgetDraw::text(engine, cx, cy, &ch.to_string(), color, if selected { 0.4 } else { 0.15 }, RenderLayer::UI);
                cx += 0.6;
            }
            cy -= 0.7;
        }
    }

    fn render_colors(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut cy = y;
        for (name, colors) in COLOR_PALETTES {
            WidgetDraw::text(engine, x + 0.3, cy, name, theme.fg, 0.1, RenderLayer::UI);
            cy -= 0.5;
            let mut cx = x + 0.5;
            for &(r, g, b) in *colors {
                WidgetDraw::color_swatch(engine, cx, cy, Vec4::new(r, g, b, 1.0));
                cx += 1.5;
            }
            cy -= 0.7;
        }
    }

    fn render_fields(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let mut cy = y;
        for ft in FieldType::all() {
            let selected = self.selected_field == Some(*ft);
            let color = if selected { theme.accent } else { theme.fg };
            WidgetDraw::text(engine, x + 0.3, cy, &format!("~ {}", ft.label()), color, if selected { 0.25 } else { 0.08 }, RenderLayer::UI);
            cy -= 0.55;
        }
    }

    fn render_particles(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let presets = [
            "Death Explosion", "Level Up", "Crit Burst", "Hit Sparks",
            "Fire Burst", "Smoke Puff", "Electric Discharge", "Ice Shatter",
            "Poison Cloud", "Heal Spiral", "Shield Hit", "Confetti",
        ];
        let mut cy = y;
        for name in &presets {
            WidgetDraw::text(engine, x + 0.3, cy, &format!("* {}", name), theme.fg, 0.08, RenderLayer::UI);
            cy -= 0.55;
        }
    }

    fn render_formations(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let formations = [
            "Diamond", "Ring", "Cross", "Star", "Arrow", "Grid",
            "Spiral", "Helix", "Shield", "Crescent", "Skull", "Heart",
        ];
        let mut cy = y;
        for name in &formations {
            WidgetDraw::text(engine, x + 0.3, cy, &format!("# {}", name), theme.fg, 0.08, RenderLayer::UI);
            cy -= 0.55;
        }
    }

    fn render_presets(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) {
        let presets = [
            ("Galaxy", "Spiral arm formation"), ("Heartbeat", "Pulsing entity"),
            ("Supernova", "Explosion sequence"), ("Math Rain", "Matrix-style cascade"),
            ("Attractor Garden", "Multiple attractors"), ("Force Field Lab", "Interactive fields"),
        ];
        let mut cy = y;
        for (name, desc) in &presets {
            WidgetDraw::text(engine, x + 0.3, cy, name, theme.accent, 0.15, RenderLayer::UI);
            WidgetDraw::text(engine, x + 0.3, cy - 0.4, desc, theme.fg_dim, 0.05, RenderLayer::UI);
            cy -= 0.9;
        }
    }
}
