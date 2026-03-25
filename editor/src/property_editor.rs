//! Property editor — comprehensive per-node-type property panels.
//!
//! When a node is selected in the hierarchy, the property editor shows
//! all editable fields organized into collapsible sections:
//!
//! # Sections
//!
//! ## Transform
//! - Position (Vec3 sliders)
//! - Rotation (float slider, degrees)
//! - Scale (Vec2 sliders)
//!
//! ## Visual
//! - Character (glyph picker)
//! - Color (color picker with gradient presets)
//! - Emission (float slider)
//! - Glow color + radius
//! - Blend mode (dropdown: Normal, Additive, Multiply, Screen)
//! - Render layer (dropdown)
//! - Opacity (float slider)
//!
//! ## Physics
//! - Mass (float)
//! - Velocity (Vec3)
//! - Charge (float)
//! - Temperature (float)
//! - Entropy (float)
//!
//! ## Math Function
//! - Function type (dropdown: Sine, Lorenz, Perlin, Orbit, etc.)
//! - Per-function parameters
//! - Preview graph
//!
//! ## Force Field (for field nodes)
//! - Field type (dropdown)
//! - Per-field parameters
//! - Falloff type (dropdown)
//! - Visualize toggle
//!
//! ## Entity (for entity nodes)
//! - HP / Max HP
//! - Cohesion strength
//! - Pulse rate / depth
//! - Formation shape (dropdown)
//! - Glyph count

use glam::{Vec2, Vec3, Vec4};
use proof_engine::prelude::*;
use std::collections::HashMap;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect, WidgetResponse};
use crate::widgets::slider::{Slider, NumberInput, Vec3Input};
use crate::widgets::color_picker::ColorPicker;
use crate::widgets::dropdown::Dropdown;
use crate::widgets::common::{Toggle, Label, Separator};
use crate::scene::{SceneNode, SceneDocument, NodeKind, FieldType};

// ── Section ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertySection {
    Transform,
    Visual,
    Physics,
    MathFunction,
    ForceField,
    Entity,
    Tags,
    Advanced,
}

impl PropertySection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Transform => "Transform", Self::Visual => "Visual",
            Self::Physics => "Physics", Self::MathFunction => "Math Function",
            Self::ForceField => "Force Field", Self::Entity => "Entity",
            Self::Tags => "Tags", Self::Advanced => "Advanced",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Transform => "+", Self::Visual => "*", Self::Physics => "~",
            Self::MathFunction => "f", Self::ForceField => "F",
            Self::Entity => "#", Self::Tags => "T", Self::Advanced => ">",
        }
    }

    /// Which sections are relevant for a given node kind.
    pub fn for_kind(kind: NodeKind) -> Vec<PropertySection> {
        match kind {
            NodeKind::Glyph => vec![Self::Transform, Self::Visual, Self::Physics, Self::MathFunction, Self::Tags],
            NodeKind::Field => vec![Self::Transform, Self::ForceField, Self::Tags],
            NodeKind::Entity => vec![Self::Transform, Self::Visual, Self::Entity, Self::Physics, Self::Tags],
            NodeKind::Group => vec![Self::Transform, Self::Tags],
            NodeKind::Camera => vec![Self::Transform, Self::Advanced],
        }
    }
}

// ── PropertyEditor ──────────────────────────────────────────────────────────

pub struct PropertyEditor {
    pub expanded_sections: HashMap<PropertySection, bool>,
    pub position_input: Vec3Input,
    pub rotation_slider: Slider,
    pub scale_slider: Slider,
    pub color_picker: ColorPicker,
    pub emission_slider: Slider,
    pub glow_radius_slider: Slider,
    pub opacity_slider: Slider,
    pub mass_slider: Slider,
    pub charge_slider: Slider,
    pub temperature_slider: Slider,
    pub entropy_slider: Slider,
    pub blend_mode_dropdown: Dropdown,
    pub layer_dropdown: Dropdown,
    pub field_type_dropdown: Dropdown,
    pub field_strength_slider: Slider,
    pub field_radius_slider: Slider,
    pub math_fn_dropdown: Dropdown,
    pub fn_param1_slider: Slider,
    pub fn_param2_slider: Slider,
    pub fn_param3_slider: Slider,
    pub entity_hp_slider: Slider,
    pub entity_cohesion_slider: Slider,
    pub entity_pulse_rate_slider: Slider,
    pub entity_glyph_count: NumberInput,
    pub formation_dropdown: Dropdown,
    pub name_editing: bool,
    pub scroll_offset: f32,
    current_node_id: Option<u32>,
}

impl PropertyEditor {
    pub fn new(x: f32, y: f32, w: f32) -> Self {
        let mut sections = HashMap::new();
        for s in &[PropertySection::Transform, PropertySection::Visual, PropertySection::Physics,
                   PropertySection::MathFunction, PropertySection::ForceField, PropertySection::Entity,
                   PropertySection::Tags, PropertySection::Advanced] {
            sections.insert(*s, true);
        }

        Self {
            expanded_sections: sections,
            position_input: Vec3Input::new("Position", Vec3::ZERO, -50.0, 50.0, x, y - 2.0, w),
            rotation_slider: Slider::new("Rotation", 0.0, 0.0, 360.0, x, y - 4.0, w),
            scale_slider: Slider::new("Scale", 1.0, 0.1, 10.0, x, y - 4.6, w),
            color_picker: ColorPicker::new("Color", Vec4::ONE, x, y - 6.0, w),
            emission_slider: Slider::new("Emission", 1.0, 0.0, 5.0, x, y - 7.0, w),
            glow_radius_slider: Slider::new("Glow Rad", 1.0, 0.0, 5.0, x, y - 7.6, w),
            opacity_slider: Slider::new("Opacity", 1.0, 0.0, 1.0, x, y - 8.2, w),
            mass_slider: Slider::new("Mass", 0.1, 0.0, 10.0, x, y - 10.0, w),
            charge_slider: Slider::new("Charge", 0.0, -5.0, 5.0, x, y - 10.6, w),
            temperature_slider: Slider::new("Temp", 0.0, -100.0, 100.0, x, y - 11.2, w),
            entropy_slider: Slider::new("Entropy", 0.0, 0.0, 1.0, x, y - 11.8, w),
            blend_mode_dropdown: Dropdown::new("Blend",
                vec!["Normal".into(), "Additive".into(), "Multiply".into(), "Screen".into()],
                1, x, y - 8.8, w),
            layer_dropdown: Dropdown::new("Layer",
                vec!["Background".into(), "World".into(), "Entity".into(), "Particle".into(), "UI".into(), "Overlay".into()],
                2, x, y - 9.4, w),
            field_type_dropdown: Dropdown::new("Field",
                FieldType::all().iter().map(|f| f.label().to_string()).collect(),
                0, x, y - 6.0, w),
            field_strength_slider: Slider::new("Strength", 2.0, 0.0, 10.0, x, y - 7.0, w),
            field_radius_slider: Slider::new("Radius", 8.0, 0.5, 50.0, x, y - 7.6, w),
            math_fn_dropdown: Dropdown::new("Function",
                vec!["Breathing".into(), "Sine".into(), "Orbit".into(), "Perlin".into(),
                     "Lorenz".into(), "Spiral".into(), "GoldenSpiral".into(), "Lissajous".into(),
                     "LogisticMap".into(), "SpringDamper".into()],
                0, x, y - 13.0, w),
            fn_param1_slider: Slider::new("Param1", 0.5, 0.0, 10.0, x, y - 13.6, w),
            fn_param2_slider: Slider::new("Param2", 0.3, 0.0, 10.0, x, y - 14.2, w),
            fn_param3_slider: Slider::new("Param3", 0.0, -10.0, 10.0, x, y - 14.8, w),
            entity_hp_slider: Slider::new("HP", 100.0, 0.0, 1000.0, x, y - 6.0, w),
            entity_cohesion_slider: Slider::new("Cohesion", 0.7, 0.0, 1.0, x, y - 6.6, w),
            entity_pulse_rate_slider: Slider::new("Pulse", 0.5, 0.0, 5.0, x, y - 7.2, w),
            entity_glyph_count: NumberInput::new("Glyphs", 12.0, 3.0, 100.0, 1.0, x, y - 7.8),
            formation_dropdown: Dropdown::new("Formation",
                vec!["Diamond".into(), "Ring".into(), "Cross".into(), "Star".into(),
                     "Arrow".into(), "Grid".into(), "Spiral".into(), "Helix".into(),
                     "Shield".into(), "Crescent".into(), "Pentagon".into(), "Random".into()],
                0, x, y - 8.4, w),
            name_editing: false,
            scroll_offset: 0.0,
            current_node_id: None,
        }
    }

    /// Load a node's properties into the editor fields.
    pub fn load_node(&mut self, node: &SceneNode) {
        self.current_node_id = Some(node.id);
        self.position_input.set_value(node.position);
        self.rotation_slider.set_value(node.rotation);
        self.scale_slider.set_value(node.scale);
        self.color_picker.set_color(node.color);
        self.emission_slider.set_value(node.emission);
        self.glow_radius_slider.set_value(node.glow_radius);
    }

    /// Write the editor fields back to a node.
    pub fn write_to_node(&self, node: &mut SceneNode) {
        node.position = self.position_input.value();
        node.rotation = self.rotation_slider.value;
        node.scale = self.scale_slider.value;
        node.color = self.color_picker.color;
        node.emission = self.emission_slider.value;
        node.glow_radius = self.glow_radius_slider.value;
    }

    /// Render the property editor for a given node kind.
    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, kind: NodeKind, theme: &WidgetTheme) {
        let sections = PropertySection::for_kind(kind);
        let mut cy = y;

        for section in &sections {
            let expanded = self.expanded_sections.get(section).copied().unwrap_or(true);
            let arrow = if expanded { "v" } else { ">" };
            let header = format!("{} {} {}", arrow, section.icon(), section.label());
            WidgetDraw::text(engine, x, cy, &header, theme.accent, 0.2, RenderLayer::UI);
            WidgetDraw::separator(engine, x, cy - 0.4, width, theme.separator);
            cy -= 0.6;

            if !expanded { continue; }

            match section {
                PropertySection::Transform => {
                    self.position_input.render(engine, theme);
                    self.rotation_slider.render(engine, theme);
                    self.scale_slider.render(engine, theme);
                    cy -= 3.5;
                }
                PropertySection::Visual => {
                    self.color_picker.render(engine, theme);
                    cy -= self.color_picker.height();
                    self.emission_slider.render(engine, theme);
                    self.glow_radius_slider.render(engine, theme);
                    self.opacity_slider.render(engine, theme);
                    self.blend_mode_dropdown.render(engine, theme);
                    self.layer_dropdown.render(engine, theme);
                    cy -= 3.5;
                }
                PropertySection::Physics => {
                    self.mass_slider.render(engine, theme);
                    self.charge_slider.render(engine, theme);
                    self.temperature_slider.render(engine, theme);
                    self.entropy_slider.render(engine, theme);
                    cy -= 2.8;
                }
                PropertySection::MathFunction => {
                    self.math_fn_dropdown.render(engine, theme);
                    self.fn_param1_slider.render(engine, theme);
                    self.fn_param2_slider.render(engine, theme);
                    self.fn_param3_slider.render(engine, theme);
                    cy -= 2.5;
                }
                PropertySection::ForceField => {
                    self.field_type_dropdown.render(engine, theme);
                    self.field_strength_slider.render(engine, theme);
                    self.field_radius_slider.render(engine, theme);
                    cy -= 2.0;
                }
                PropertySection::Entity => {
                    self.entity_hp_slider.render(engine, theme);
                    self.entity_cohesion_slider.render(engine, theme);
                    self.entity_pulse_rate_slider.render(engine, theme);
                    self.entity_glyph_count.render(engine, theme);
                    self.formation_dropdown.render(engine, theme);
                    cy -= 3.5;
                }
                PropertySection::Tags => {
                    WidgetDraw::text(engine, x + 0.3, cy, "(tags editor)", theme.fg_dim, 0.05, RenderLayer::UI);
                    cy -= 0.6;
                }
                PropertySection::Advanced => {
                    WidgetDraw::text(engine, x + 0.3, cy, "(advanced settings)", theme.fg_dim, 0.05, RenderLayer::UI);
                    cy -= 0.6;
                }
            }
            cy -= 0.3; // gap between sections
        }
    }
}
