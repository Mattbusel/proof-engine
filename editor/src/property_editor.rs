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
            NodeKind::Glyph => vec![Self::Transform, Self::Visual, Self::Physics, Self::MathFunction, Self::Tags, Self::Advanced],
            NodeKind::Field => vec![Self::Transform, Self::ForceField, Self::Tags, Self::Advanced],
            NodeKind::Entity => vec![Self::Transform, Self::Visual, Self::Entity, Self::Physics, Self::Tags, Self::Advanced],
            NodeKind::Group => vec![Self::Transform, Self::Tags, Self::Advanced],
            NodeKind::Camera => vec![Self::Transform, Self::Advanced],
        }
    }
}

// ── Tag Category ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagCategory {
    System,
    User,
    Trigger,
    Group,
    Effect,
}

impl TagCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::System  => "System",
            Self::User    => "User",
            Self::Trigger => "Trigger",
            Self::Group   => "Group",
            Self::Effect  => "Effect",
        }
    }

    pub fn color(self) -> Vec4 {
        match self {
            Self::System  => Vec4::new(0.4, 0.4, 0.8, 1.0),
            Self::User    => Vec4::new(0.3, 0.7, 0.4, 1.0),
            Self::Trigger => Vec4::new(0.8, 0.5, 0.2, 1.0),
            Self::Group   => Vec4::new(0.5, 0.3, 0.7, 1.0),
            Self::Effect  => Vec4::new(0.7, 0.3, 0.5, 1.0),
        }
    }

    pub fn all() -> &'static [TagCategory] {
        &[Self::System, Self::User, Self::Trigger, Self::Group, Self::Effect]
    }

    pub fn classify(tag: &str) -> TagCategory {
        match tag {
            t if t.starts_with("sys:")     => TagCategory::System,
            t if t.starts_with("trigger:") => TagCategory::Trigger,
            t if t.starts_with("group:")   => TagCategory::Group,
            t if t.starts_with("fx:")      => TagCategory::Effect,
            _                              => TagCategory::User,
        }
    }
}

/// System-defined tag suggestions per node kind.
pub fn suggested_tags(kind: NodeKind) -> Vec<&'static str> {
    match kind {
        NodeKind::Glyph => vec![
            "sys:static", "sys:interactive", "sys:hidden",
            "trigger:on_click", "trigger:on_hover",
            "fx:glow", "fx:pulse", "fx:trail",
            "group:foreground", "group:background",
        ],
        NodeKind::Entity => vec![
            "sys:player", "sys:enemy", "sys:npc", "sys:boss",
            "trigger:on_death", "trigger:on_damage", "trigger:on_spawn",
            "fx:explosion", "fx:death_anim",
            "group:faction_a", "group:faction_b",
        ],
        NodeKind::Field => vec![
            "sys:active", "sys:disabled", "sys:area_effect",
            "trigger:on_enter", "trigger:on_exit",
            "group:zone", "group:hazard",
        ],
        NodeKind::Group => vec![
            "sys:layer", "sys:scene_root",
            "group:environment", "group:ui", "group:gameplay",
        ],
        NodeKind::Camera => vec![
            "sys:main_cam", "sys:cutscene_cam", "sys:debug_cam",
        ],
    }
}

// ── Blend Mode / Render Layer ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Additive,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    Difference,
}

impl BlendMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal    => "Normal",
            Self::Additive  => "Additive",
            Self::Multiply  => "Multiply",
            Self::Screen    => "Screen",
            Self::Overlay   => "Overlay",
            Self::SoftLight => "SoftLight",
            Self::Difference => "Difference",
        }
    }
    pub fn all() -> &'static [BlendMode] {
        &[Self::Normal, Self::Additive, Self::Multiply, Self::Screen,
          Self::Overlay, Self::SoftLight, Self::Difference]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRenderLayer {
    Background,
    Entity,
    Overlay,
    UI,
}

impl NodeRenderLayer {
    pub fn label(self) -> &'static str {
        match self {
            Self::Background => "Background",
            Self::Entity     => "Entity",
            Self::Overlay    => "Overlay",
            Self::UI         => "UI",
        }
    }
    pub fn all() -> &'static [NodeRenderLayer] {
        &[Self::Background, Self::Entity, Self::Overlay, Self::UI]
    }
}

// ── Physics Override ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionResponse {
    Bounce,
    Absorb,
    PassThrough,
}

impl CollisionResponse {
    pub fn label(self) -> &'static str {
        match self {
            Self::Bounce      => "Bounce",
            Self::Absorb      => "Absorb",
            Self::PassThrough => "PassThrough",
        }
    }
    pub fn all() -> &'static [CollisionResponse] {
        &[Self::Bounce, Self::Absorb, Self::PassThrough]
    }
}

#[derive(Debug, Clone)]
pub struct PhysicsOverride {
    pub mass_override: f32,
    pub use_mass_override: bool,
    pub is_static: bool,
    pub is_trigger: bool,
    pub collision_response: CollisionResponse,
}

impl Default for PhysicsOverride {
    fn default() -> Self {
        Self {
            mass_override: 1.0,
            use_mass_override: false,
            is_static: false,
            is_trigger: false,
            collision_response: CollisionResponse::Bounce,
        }
    }
}

// ── Script Slot ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ScriptSlot {
    pub slot_name: String,
    pub script_name: String,
    pub enabled: bool,
}

impl ScriptSlot {
    pub fn new(slot: &str) -> Self {
        Self {
            slot_name: slot.to_string(),
            script_name: String::new(),
            enabled: true,
        }
    }
}

// ── Advanced Node Settings ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AdvancedNodeSettings {
    pub blend_mode: BlendMode,
    pub render_layer: NodeRenderLayer,
    pub z_order: f32,
    // Lifetime
    pub finite_lifetime: bool,
    pub lifetime_seconds: f32,
    // Physics override
    pub physics: PhysicsOverride,
    // Scripts
    pub script_slots: Vec<ScriptSlot>,
}

impl Default for AdvancedNodeSettings {
    fn default() -> Self {
        Self {
            blend_mode: BlendMode::Normal,
            render_layer: NodeRenderLayer::Entity,
            z_order: 0.0,
            finite_lifetime: false,
            lifetime_seconds: 5.0,
            physics: PhysicsOverride::default(),
            script_slots: vec![
                ScriptSlot::new("OnSpawn"),
                ScriptSlot::new("OnUpdate"),
                ScriptSlot::new("OnDeath"),
            ],
        }
    }
}

// ── Math Function Types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathFunctionType {
    Breathing,
    Floating,
    Pulsing,
    Spinning,
    Orbiting,
    WaveMotion,
    StrangeAttractor,
    Lissajous,
    Spiral,
    Pendulum,
}

impl MathFunctionType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Breathing       => "Breathing",
            Self::Floating        => "Floating",
            Self::Pulsing         => "Pulsing",
            Self::Spinning        => "Spinning",
            Self::Orbiting        => "Orbiting",
            Self::WaveMotion      => "WaveMotion",
            Self::StrangeAttractor => "StrangeAttractor",
            Self::Lissajous       => "Lissajous",
            Self::Spiral          => "Spiral",
            Self::Pendulum        => "Pendulum",
        }
    }

    pub fn param_labels(self) -> &'static [&'static str] {
        match self {
            Self::Breathing       => &["Rate", "Depth"],
            Self::Floating        => &["Amp X", "Amp Y", "Speed"],
            Self::Pulsing         => &["Rate", "Min Scale", "Max Scale"],
            Self::Spinning        => &["Angular Speed", "Axis"],
            Self::Orbiting        => &["Radius", "Speed", "Center X"],
            Self::WaveMotion      => &["Freq X", "Freq Y", "Phase"],
            Self::StrangeAttractor => &["Sigma", "Rho", "Beta"],
            Self::Lissajous       => &["Freq A", "Freq B", "Delta"],
            Self::Spiral          => &["Rate", "Tightness"],
            Self::Pendulum        => &["Length", "Damping", "Angle"],
        }
    }

    pub fn all() -> &'static [MathFunctionType] {
        &[
            Self::Breathing, Self::Floating, Self::Pulsing, Self::Spinning,
            Self::Orbiting, Self::WaveMotion, Self::StrangeAttractor,
            Self::Lissajous, Self::Spiral, Self::Pendulum,
        ]
    }
}

// ── Math Function State ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MathFunctionState {
    pub fn_type: MathFunctionType,
    pub params: [f32; 4],
    pub preview_time: f32,
    pub preview_points: Vec<(f32, f32)>,
}

impl Default for MathFunctionState {
    fn default() -> Self {
        Self {
            fn_type: MathFunctionType::Breathing,
            params: [0.5, 0.3, 1.0, 0.0],
            preview_time: 0.0,
            preview_points: Vec::new(),
        }
    }
}

impl MathFunctionState {
    pub fn sample(&self, t: f32) -> (f32, f32) {
        let p = &self.params;
        match self.fn_type {
            MathFunctionType::Breathing => {
                let rate = p[0].max(0.01);
                let depth = p[1];
                let v = (t * rate * std::f32::consts::TAU).sin() * depth;
                (t, v)
            }
            MathFunctionType::Floating => {
                let ax = p[0]; let ay = p[1]; let speed = p[2].max(0.01);
                let x = (t * speed).sin() * ax;
                let y = (t * speed * 0.7).cos() * ay;
                (x, y)
            }
            MathFunctionType::Pulsing => {
                let rate = p[0].max(0.01);
                let mn = p[1]; let mx = p[2];
                let scale = mn + (mx - mn) * 0.5 * (1.0 + (t * rate * std::f32::consts::TAU).sin());
                (t, scale)
            }
            MathFunctionType::Spinning => {
                let speed = p[0];
                let angle = t * speed;
                (angle.cos(), angle.sin())
            }
            MathFunctionType::Orbiting => {
                let r = p[0].max(0.01); let speed = p[1];
                let angle = t * speed;
                (angle.cos() * r, angle.sin() * r)
            }
            MathFunctionType::WaveMotion => {
                let fx = p[0].max(0.01); let fy = p[1].max(0.01); let phase = p[2];
                let x = (t * fx).sin();
                let y = (t * fy + phase).cos();
                (x, y)
            }
            MathFunctionType::StrangeAttractor => {
                // Simplified Lorenz projection
                let sigma = p[0].max(1.0);
                let y = (t * sigma * 0.1).sin();
                (t, y)
            }
            MathFunctionType::Lissajous => {
                let a = p[0].max(0.01); let b = p[1].max(0.01); let delta = p[2];
                ((t * a).sin(), (t * b + delta).sin())
            }
            MathFunctionType::Spiral => {
                let rate = p[0].max(0.01); let tight = p[1];
                let r = (t * rate).exp() * tight * 0.1;
                ((t * 2.0).cos() * r, (t * 2.0).sin() * r)
            }
            MathFunctionType::Pendulum => {
                let len = p[0].max(0.1); let damp = p[1]; let angle0 = p[2];
                let freq = (9.8 / len).sqrt();
                let x = angle0 * (-damp * t).exp() * (freq * t).cos();
                (t, x)
            }
        }
    }

    pub fn rebuild_preview(&mut self, steps: usize) {
        self.preview_points.clear();
        for i in 0..steps {
            let t = i as f32 / steps as f32 * 8.0;
            self.preview_points.push(self.sample(t));
        }
    }
}

// ── Formation Definition ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationShape {
    Diamond,
    Ring,
    Cross,
    Star,
    Arrow,
    Grid,
    Spiral,
    Helix,
    Shield,
    Crescent,
}

impl FormationShape {
    pub fn label(self) -> &'static str {
        match self {
            Self::Diamond  => "Diamond",
            Self::Ring     => "Ring",
            Self::Cross    => "Cross",
            Self::Star     => "Star",
            Self::Arrow    => "Arrow",
            Self::Grid     => "Grid",
            Self::Spiral   => "Spiral",
            Self::Helix    => "Helix",
            Self::Shield   => "Shield",
            Self::Crescent => "Crescent",
        }
    }

    pub fn all() -> &'static [FormationShape] {
        &[Self::Diamond, Self::Ring, Self::Cross, Self::Star, Self::Arrow,
          Self::Grid, Self::Spiral, Self::Helix, Self::Shield, Self::Crescent]
    }

    /// Generate normalized dot positions (0..1 space) for preview.
    pub fn dot_positions(self, count: usize) -> Vec<(f32, f32)> {
        let n = count.max(1);
        let mut pts = Vec::with_capacity(n);
        match self {
            Self::Ring => {
                for i in 0..n {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    pts.push((a.cos() * 0.45 + 0.5, a.sin() * 0.45 + 0.5));
                }
            }
            Self::Diamond => {
                let half = (n / 4).max(1);
                for i in 0..n {
                    let q = i / half;
                    let t = (i % half) as f32 / half as f32;
                    let (x, y) = match q % 4 {
                        0 => (0.5 + t * 0.45, 0.5 + (1.0 - t) * 0.45),
                        1 => (0.5 + (1.0 - t) * 0.45, 0.5 - t * 0.45),
                        2 => (0.5 - t * 0.45, 0.5 - (1.0 - t) * 0.45),
                        _ => (0.5 - (1.0 - t) * 0.45, 0.5 + t * 0.45),
                    };
                    pts.push((x, y));
                }
            }
            Self::Cross => {
                let arm = n / 4;
                for i in 0..n {
                    let t = (i % arm.max(1)) as f32 / arm.max(1) as f32 * 0.9;
                    let (x, y) = match i / arm.max(1) % 4 {
                        0 => (0.5, 0.05 + t),
                        1 => (0.5, 0.95 - t),
                        2 => (0.05 + t, 0.5),
                        _ => (0.95 - t, 0.5),
                    };
                    pts.push((x, y));
                }
            }
            Self::Arrow => {
                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let x = if t < 0.5 { t * 2.0 * 0.5 } else { (1.0 - t) * 2.0 * 0.5 };
                    let y = t;
                    pts.push((x + 0.25, y * 0.9 + 0.05));
                }
            }
            Self::Grid => {
                let cols = ((n as f32).sqrt().ceil() as usize).max(1);
                let rows = (n + cols - 1) / cols;
                for i in 0..n {
                    let col = i % cols;
                    let row = i / cols;
                    let x = col as f32 / cols as f32 * 0.9 + 0.05;
                    let y = row as f32 / rows.max(1) as f32 * 0.9 + 0.05;
                    pts.push((x, y));
                }
            }
            Self::Spiral => {
                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let angle = t * std::f32::consts::TAU * 3.0;
                    let r = t * 0.45;
                    pts.push((angle.cos() * r + 0.5, angle.sin() * r + 0.5));
                }
            }
            Self::Star => {
                let points = 5;
                for i in 0..n {
                    let idx = i % (points * 2);
                    let a = idx as f32 / (points * 2) as f32 * std::f32::consts::TAU;
                    let r = if idx % 2 == 0 { 0.45 } else { 0.2 };
                    pts.push((a.cos() * r + 0.5, a.sin() * r + 0.5));
                }
            }
            Self::Helix => {
                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let x = (t * std::f32::consts::TAU * 2.0).sin() * 0.4 + 0.5;
                    let y = t * 0.9 + 0.05;
                    pts.push((x, y));
                }
            }
            Self::Shield => {
                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let a = t * std::f32::consts::PI;
                    let r = if t < 0.5 { 0.45 } else { 0.45 * (1.0 - (t - 0.5) * 2.0) };
                    pts.push((a.sin() * r + 0.5, -a.cos() * r + 0.5));
                }
            }
            Self::Crescent => {
                for i in 0..n {
                    let t = i as f32 / n as f32;
                    let a = t * std::f32::consts::TAU;
                    let outer = 0.45;
                    let inner_off = 0.15;
                    let inner = if a.cos() > 0.0 { outer - inner_off * a.cos() } else { outer };
                    pts.push((a.cos() * inner + 0.5, a.sin() * inner + 0.5));
                }
            }
        }
        pts
    }
}

// ── Formation State ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FormationState {
    pub shape: FormationShape,
    pub glyph_char: char,
    pub slot_color: Vec4,
    pub count: usize,
    pub size: f32,
    pub density: f32,
}

impl Default for FormationState {
    fn default() -> Self {
        Self {
            shape: FormationShape::Ring,
            glyph_char: '@',
            slot_color: Vec4::new(0.5, 1.0, 0.7, 1.0),
            count: 12,
            size: 1.0,
            density: 1.0,
        }
    }
}

// ── Tags State ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TagsState {
    pub tags: Vec<String>,
    pub new_tag_input: String,
    pub autocomplete_visible: bool,
    pub autocomplete_matches: Vec<String>,
    pub active_category_filter: Option<TagCategory>,
}

impl TagsState {
    pub fn add_tag(&mut self, tag: String) {
        let tag = tag.trim().to_string();
        if !tag.is_empty() && !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
        self.new_tag_input.clear();
        self.autocomplete_visible = false;
    }

    pub fn remove_tag(&mut self, idx: usize) {
        if idx < self.tags.len() {
            self.tags.remove(idx);
        }
    }

    pub fn update_autocomplete(&mut self, kind: NodeKind, global_tags: &[String]) {
        let input_lower = self.new_tag_input.to_lowercase();
        if input_lower.is_empty() {
            // Show suggestions based on node type
            let suggested = suggested_tags(kind);
            self.autocomplete_matches = suggested.iter()
                .filter(|&&t| !self.tags.contains(&t.to_string()))
                .map(|&t| t.to_string())
                .collect();
        } else {
            // Filter global tags + suggestions
            let mut matches: Vec<String> = suggested_tags(kind)
                .iter()
                .map(|&t| t.to_string())
                .chain(global_tags.iter().cloned())
                .filter(|t| t.to_lowercase().contains(&input_lower) && !self.tags.contains(t))
                .collect();
            matches.dedup();
            self.autocomplete_matches = matches;
        }
        self.autocomplete_visible = !self.autocomplete_matches.is_empty();
    }

    pub fn visible_tags(&self) -> Vec<&String> {
        match self.active_category_filter {
            None => self.tags.iter().collect(),
            Some(cat) => self.tags.iter()
                .filter(|t| TagCategory::classify(t) == cat)
                .collect(),
        }
    }
}

// ── Multi-Selection State ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MultiSelectState {
    /// Which node IDs are selected.
    pub selected_ids: Vec<u32>,
    /// Per-field "apply to all" pending flag.
    pub apply_pending: HashMap<String, bool>,
}

impl MultiSelectState {
    pub fn is_multi(&self) -> bool {
        self.selected_ids.len() > 1
    }

    pub fn mark_apply(&mut self, field: &str) {
        self.apply_pending.insert(field.to_string(), true);
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

    // ── Extended state ───────────────────────────────────────────────────────
    /// Physics: computed centre-of-mass offset display
    pub center_of_mass: Vec3,
    /// Physics: charge affects EM fields
    pub charge_value: f32,
    /// Physics: temperature affects thermal fields
    pub temperature_value: f32,
    /// Physics: entropy affects chaos systems
    pub entropy_value: f32,
    /// Mass with real unit (kg)
    pub mass_kg: f32,

    // Tags section
    pub tags_state: TagsState,
    /// Known tags from the global scene (updated externally)
    pub global_tags: Vec<String>,

    // Advanced section
    pub advanced: AdvancedNodeSettings,
    /// Index into BlendMode::all() for UI
    pub blend_mode_idx: usize,
    /// Index into NodeRenderLayer::all() for UI
    pub render_layer_idx: usize,
    /// Index into CollisionResponse::all() for UI
    pub collision_response_idx: usize,
    /// New script name buffer
    pub new_script_name: String,

    // Math function section
    pub math_fn_state: MathFunctionState,
    pub math_fn_type_idx: usize,

    // Formation section
    pub formation_state: FormationState,
    pub formation_shape_idx: usize,

    // Multi-select
    pub multi_select: MultiSelectState,
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
                vec!["Normal".into(), "Additive".into(), "Multiply".into(), "Screen".into(),
                     "Overlay".into(), "SoftLight".into(), "Difference".into()],
                0, x, y - 8.8, w),
            layer_dropdown: Dropdown::new("Layer",
                vec!["Background".into(), "Entity".into(), "Overlay".into(), "UI".into()],
                1, x, y - 9.4, w),
            field_type_dropdown: Dropdown::new("Field",
                FieldType::all().iter().map(|f| f.label().to_string()).collect(),
                0, x, y - 6.0, w),
            field_strength_slider: Slider::new("Strength", 2.0, 0.0, 10.0, x, y - 7.0, w),
            field_radius_slider: Slider::new("Radius", 8.0, 0.5, 50.0, x, y - 7.6, w),
            math_fn_dropdown: Dropdown::new("Function",
                MathFunctionType::all().iter().map(|f| f.label().to_string()).collect(),
                0, x, y - 13.0, w),
            fn_param1_slider: Slider::new("Param1", 0.5, 0.0, 10.0, x, y - 13.6, w),
            fn_param2_slider: Slider::new("Param2", 0.3, 0.0, 10.0, x, y - 14.2, w),
            fn_param3_slider: Slider::new("Param3", 0.0, -10.0, 10.0, x, y - 14.8, w),
            entity_hp_slider: Slider::new("HP", 100.0, 0.0, 1000.0, x, y - 6.0, w),
            entity_cohesion_slider: Slider::new("Cohesion", 0.7, 0.0, 1.0, x, y - 6.6, w),
            entity_pulse_rate_slider: Slider::new("Pulse", 0.5, 0.0, 5.0, x, y - 7.2, w),
            entity_glyph_count: NumberInput::new("Glyphs", 12.0, 3.0, 100.0, 1.0, x, y - 7.8),
            formation_dropdown: Dropdown::new("Formation",
                FormationShape::all().iter().map(|f| f.label().to_string()).collect(),
                0, x, y - 8.4, w),
            name_editing: false,
            scroll_offset: 0.0,
            current_node_id: None,

            // Extended state defaults
            center_of_mass: Vec3::ZERO,
            charge_value: 0.0,
            temperature_value: 0.0,
            entropy_value: 0.0,
            mass_kg: 1.0,
            tags_state: TagsState::default(),
            global_tags: Vec::new(),
            advanced: AdvancedNodeSettings::default(),
            blend_mode_idx: 0,
            render_layer_idx: 1,
            collision_response_idx: 0,
            new_script_name: String::new(),
            math_fn_state: MathFunctionState::default(),
            math_fn_type_idx: 0,
            formation_state: FormationState::default(),
            formation_shape_idx: 0,
            multi_select: MultiSelectState::default(),
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

    // ── Tags Rendering ─────────────────────────────────────────────────────

    fn render_tags_section(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, kind: NodeKind, theme: &WidgetTheme) -> f32 {
        let mut cy = y;

        // Category filter pills
        WidgetDraw::text(engine, x + 0.3, cy, "Category:", theme.fg_dim, 0.06, RenderLayer::UI);
        let mut pill_x = x + 1.6;
        for cat in TagCategory::all() {
            let active = self.tags_state.active_category_filter == Some(*cat);
            let bg = if active { cat.color() } else { Vec4::new(0.2, 0.2, 0.25, 0.8) };
            WidgetDraw::fill_rect(engine, Rect::new(pill_x, cy + 0.05, 1.2, 0.4), bg);
            WidgetDraw::text(engine, pill_x + 0.05, cy, cat.label(), if active { Vec4::new(1.0, 1.0, 1.0, 1.0) } else { theme.fg_dim }, 0.05, RenderLayer::UI);
            pill_x += 1.35;
        }
        cy -= 0.55;

        // Existing tag pills
        if self.tags_state.tags.is_empty() {
            WidgetDraw::text(engine, x + 0.3, cy, "(no tags)", theme.fg_dim, 0.06, RenderLayer::UI);
            cy -= 0.45;
        } else {
            let mut px = x + 0.3;
            let row_h = 0.48;
            for tag in &self.tags_state.tags {
                let cat = TagCategory::classify(tag);
                // Filter by category if active
                if let Some(cf) = self.tags_state.active_category_filter {
                    if cf != cat { continue; }
                }
                let pill_w = tag.len() as f32 * 0.085 + 0.5;
                if px + pill_w > x + width - 0.3 {
                    px = x + 0.3;
                    cy -= row_h;
                }
                WidgetDraw::fill_rect(engine, Rect::new(px, cy + 0.04, pill_w, 0.38), cat.color() * Vec4::new(0.5, 0.5, 0.5, 0.9));
                WidgetDraw::text(engine, px + 0.08, cy + 0.05, tag, Vec4::new(1.0, 1.0, 1.0, 0.9), 0.055, RenderLayer::UI);
                // × remove button placeholder
                WidgetDraw::text(engine, px + pill_w - 0.22, cy + 0.05, "x", Vec4::new(1.0, 0.4, 0.4, 0.8), 0.055, RenderLayer::UI);
                px += pill_w + 0.12;
            }
            cy -= row_h + 0.1;
        }

        // Input field
        cy -= 0.1;
        WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, cy, width - 0.6, 0.4), Vec4::new(0.12, 0.12, 0.16, 0.9));
        let hint = if self.tags_state.new_tag_input.is_empty() { "Add tag..." } else { &self.tags_state.new_tag_input };
        WidgetDraw::text(engine, x + 0.4, cy + 0.04, hint, theme.fg_dim, 0.065, RenderLayer::UI);
        cy -= 0.5;

        // Autocomplete dropdown
        if self.tags_state.autocomplete_visible && !self.tags_state.autocomplete_matches.is_empty() {
            let max_show = 5.min(self.tags_state.autocomplete_matches.len());
            let dropdown_h = max_show as f32 * 0.38;
            WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, cy, width - 0.6, dropdown_h), Vec4::new(0.1, 0.12, 0.18, 0.95));
            for (i, sug) in self.tags_state.autocomplete_matches.iter().take(max_show).enumerate() {
                let sy = cy - i as f32 * 0.38;
                WidgetDraw::text(engine, x + 0.5, sy + 0.05, sug, theme.fg, 0.065, RenderLayer::UI);
            }
            cy -= dropdown_h + 0.1;
        }

        // Suggestions header
        WidgetDraw::text(engine, x + 0.3, cy, "Suggestions:", theme.fg_dim, 0.06, RenderLayer::UI);
        cy -= 0.4;
        let mut sx = x + 0.3;
        for sug in suggested_tags(kind).iter().take(8) {
            if self.tags_state.tags.iter().any(|t| t == sug) { continue; }
            let sw = sug.len() as f32 * 0.075 + 0.35;
            let cat = TagCategory::classify(sug);
            WidgetDraw::fill_rect(engine, Rect::new(sx, cy + 0.04, sw, 0.32), cat.color() * Vec4::splat(0.35));
            WidgetDraw::text(engine, sx + 0.07, cy + 0.05, sug, theme.fg_dim, 0.05, RenderLayer::UI);
            sx += sw + 0.1;
            if sx > x + width - 0.5 { sx = x + 0.3; cy -= 0.38; }
        }
        cy -= 0.45;

        y - cy // return total height consumed
    }

    // ── Advanced Section Rendering ─────────────────────────────────────────

    fn render_advanced_section(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) -> f32 {
        let mut cy = y;
        let col2 = x + width * 0.5;

        // ─ Blend Mode ─
        WidgetDraw::text(engine, x + 0.3, cy, "Blend Mode:", theme.fg_dim, 0.07, RenderLayer::UI);
        let blend_label = BlendMode::all().get(self.blend_mode_idx)
            .map(|b| b.label()).unwrap_or("Normal");
        WidgetDraw::fill_rect(engine, Rect::new(col2, cy, width * 0.48, 0.38), Vec4::new(0.15, 0.15, 0.2, 0.9));
        WidgetDraw::text(engine, col2 + 0.1, cy + 0.04, blend_label, theme.accent, 0.08, RenderLayer::UI);
        WidgetDraw::text(engine, col2 + width * 0.38, cy + 0.04, "v", theme.fg_dim, 0.08, RenderLayer::UI);
        cy -= 0.5;

        // ─ Render Layer ─
        WidgetDraw::text(engine, x + 0.3, cy, "Render Layer:", theme.fg_dim, 0.07, RenderLayer::UI);
        let layer_label = NodeRenderLayer::all().get(self.render_layer_idx)
            .map(|l| l.label()).unwrap_or("Entity");
        WidgetDraw::fill_rect(engine, Rect::new(col2, cy, width * 0.48, 0.38), Vec4::new(0.15, 0.15, 0.2, 0.9));
        WidgetDraw::text(engine, col2 + 0.1, cy + 0.04, layer_label, theme.accent, 0.08, RenderLayer::UI);
        cy -= 0.5;

        // ─ Z-Order ─
        WidgetDraw::text(engine, x + 0.3, cy, "Z-Order:", theme.fg_dim, 0.07, RenderLayer::UI);
        let z_pct = (self.advanced.z_order + 100.0) / 200.0;
        WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.46, z_pct, theme.accent, theme.bg);
        WidgetDraw::text(engine, col2 + width * 0.48, cy + 0.04, &format!("{:.0}", self.advanced.z_order), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.52;

        // ─ Lifetime ─
        WidgetDraw::separator(engine, x + 0.2, cy, width - 0.4, theme.separator);
        cy -= 0.2;
        WidgetDraw::text(engine, x + 0.3, cy, "LIFETIME", theme.fg_dim, 0.08, RenderLayer::UI);
        cy -= 0.42;
        let finite_icon = if self.advanced.finite_lifetime { "[x]" } else { "[ ]" };
        WidgetDraw::text(engine, x + 0.3, cy, &format!("{} Finite Lifetime", finite_icon), theme.fg, 0.07, RenderLayer::UI);
        if self.advanced.finite_lifetime {
            cy -= 0.42;
            WidgetDraw::text(engine, x + 0.3, cy, "Duration:", theme.fg_dim, 0.07, RenderLayer::UI);
            let life_pct = (self.advanced.lifetime_seconds / 60.0).min(1.0);
            WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.46, life_pct, theme.success, theme.bg);
            WidgetDraw::text(engine, col2 + width * 0.48, cy + 0.04, &format!("{:.1}s", self.advanced.lifetime_seconds), theme.fg_dim, 0.07, RenderLayer::UI);
            cy -= 0.48;
        }

        // ─ Physics Override ─
        WidgetDraw::separator(engine, x + 0.2, cy, width - 0.4, theme.separator);
        cy -= 0.2;
        WidgetDraw::text(engine, x + 0.3, cy, "PHYSICS OVERRIDE", theme.fg_dim, 0.08, RenderLayer::UI);
        cy -= 0.42;

        let mo_icon = if self.advanced.physics.use_mass_override { "[x]" } else { "[ ]" };
        WidgetDraw::text(engine, x + 0.3, cy, &format!("{} Override Mass:", mo_icon), theme.fg, 0.07, RenderLayer::UI);
        if self.advanced.physics.use_mass_override {
            WidgetDraw::text(engine, col2, cy + 0.04, &format!("{:.2} kg", self.advanced.physics.mass_override), theme.accent, 0.08, RenderLayer::UI);
        }
        cy -= 0.42;

        let static_icon = if self.advanced.physics.is_static { "[x]" } else { "[ ]" };
        WidgetDraw::text(engine, x + 0.3, cy, &format!("{} Is Static", static_icon), theme.fg, 0.07, RenderLayer::UI);
        let trig_icon = if self.advanced.physics.is_trigger { "[x]" } else { "[ ]" };
        WidgetDraw::text(engine, col2, cy, &format!("{} Is Trigger", trig_icon), theme.fg, 0.07, RenderLayer::UI);
        cy -= 0.44;

        WidgetDraw::text(engine, x + 0.3, cy, "Collision:", theme.fg_dim, 0.07, RenderLayer::UI);
        let cr_label = CollisionResponse::all().get(self.collision_response_idx)
            .map(|c| c.label()).unwrap_or("Bounce");
        WidgetDraw::fill_rect(engine, Rect::new(col2, cy, width * 0.48, 0.38), Vec4::new(0.15, 0.15, 0.2, 0.9));
        WidgetDraw::text(engine, col2 + 0.1, cy + 0.04, cr_label, theme.accent, 0.08, RenderLayer::UI);
        cy -= 0.5;

        // ─ Script Bindings ─
        WidgetDraw::separator(engine, x + 0.2, cy, width - 0.4, theme.separator);
        cy -= 0.2;
        WidgetDraw::text(engine, x + 0.3, cy, "SCRIPT BINDINGS", theme.fg_dim, 0.08, RenderLayer::UI);
        cy -= 0.42;

        for slot in &self.advanced.script_slots {
            let en_icon = if slot.enabled { ">" } else { "." };
            let script_display = if slot.script_name.is_empty() { "(none)" } else { &slot.script_name };
            WidgetDraw::text(engine, x + 0.3, cy, &format!("{} {}:", en_icon, slot.slot_name), theme.fg_dim, 0.065, RenderLayer::UI);
            WidgetDraw::text(engine, col2, cy, script_display, if slot.script_name.is_empty() { theme.fg_dim } else { theme.accent }, 0.07, RenderLayer::UI);
            cy -= 0.42;
        }

        // Add script slot button
        WidgetDraw::text(engine, x + 0.3, cy, "[+] Add Script Slot", theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.42;

        y - cy // total height used
    }

    // ── Physics Properties Rendering ────────────────────────────────────────

    fn render_physics_section(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) -> f32 {
        let mut cy = y;
        let col2 = x + width * 0.45;

        // Mass (kg)
        WidgetDraw::text(engine, x + 0.3, cy, "Mass:", theme.fg_dim, 0.07, RenderLayer::UI);
        let mass_pct = (self.mass_kg / 100.0).clamp(0.0, 1.0);
        WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.48, mass_pct, theme.accent * Vec4::new(0.7, 0.9, 1.0, 1.0), theme.bg);
        WidgetDraw::text(engine, col2 + width * 0.5, cy + 0.04, &format!("{:.2} kg", self.mass_kg), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Charge
        WidgetDraw::text(engine, x + 0.3, cy, "Charge:", theme.fg_dim, 0.07, RenderLayer::UI);
        let charge_pct = (self.charge_value + 5.0) / 10.0;
        let charge_col = if self.charge_value > 0.0 {
            Vec4::new(0.3, 0.7, 1.0, 1.0)
        } else if self.charge_value < 0.0 {
            Vec4::new(1.0, 0.4, 0.4, 1.0)
        } else {
            theme.fg_dim
        };
        WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.48, charge_pct, charge_col, theme.bg);
        WidgetDraw::text(engine, col2 + width * 0.5, cy + 0.04, &format!("{:.2}C", self.charge_value), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Temperature
        WidgetDraw::text(engine, x + 0.3, cy, "Temperature:", theme.fg_dim, 0.07, RenderLayer::UI);
        let temp_pct = (self.temperature_value + 100.0) / 200.0;
        let temp_col = if self.temperature_value > 50.0 {
            Vec4::new(1.0, 0.3, 0.1, 1.0)
        } else if self.temperature_value < -50.0 {
            Vec4::new(0.3, 0.5, 1.0, 1.0)
        } else {
            Vec4::new(0.7, 0.7, 0.3, 1.0)
        };
        WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.48, temp_pct, temp_col, theme.bg);
        WidgetDraw::text(engine, col2 + width * 0.5, cy + 0.04, &format!("{:.1}°", self.temperature_value), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Entropy
        WidgetDraw::text(engine, x + 0.3, cy, "Entropy:", theme.fg_dim, 0.07, RenderLayer::UI);
        let entropy_col = Vec4::new(
            0.4 + self.entropy_value * 0.6,
            0.8 - self.entropy_value * 0.5,
            0.5 - self.entropy_value * 0.2,
            1.0,
        );
        WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.48, self.entropy_value, entropy_col, theme.bg);
        WidgetDraw::text(engine, col2 + width * 0.5, cy + 0.04, &format!("{:.3}", self.entropy_value), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Centre of mass display
        WidgetDraw::separator(engine, x + 0.2, cy, width - 0.4, theme.separator);
        cy -= 0.2;
        WidgetDraw::text(engine, x + 0.3, cy, "Centre of Mass:", theme.fg_dim, 0.065, RenderLayer::UI);
        cy -= 0.38;
        WidgetDraw::text(engine, x + 0.5, cy,
            &format!("X:{:.2}  Y:{:.2}  Z:{:.2}", self.center_of_mass.x, self.center_of_mass.y, self.center_of_mass.z),
            theme.fg, 0.065, RenderLayer::UI);
        cy -= 0.42;

        y - cy
    }

    // ── Entity Formation Rendering ──────────────────────────────────────────

    fn render_formation_section(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) -> f32 {
        let mut cy = y;
        let col2 = x + width * 0.45;

        // Formation shape picker (grid of labeled buttons)
        WidgetDraw::text(engine, x + 0.3, cy, "Shape:", theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.42;
        let btn_w = 1.4;
        let btn_h = 0.38;
        let cols = 5;
        for (i, shape) in FormationShape::all().iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let bx = x + 0.3 + col as f32 * (btn_w + 0.08);
            let by = cy - row as f32 * (btn_h + 0.06);
            let selected = self.formation_state.shape == *shape;
            let bg = if selected { theme.accent } else { Vec4::new(0.15, 0.15, 0.2, 0.9) };
            WidgetDraw::fill_rect(engine, Rect::new(bx, by, btn_w, btn_h), bg);
            WidgetDraw::text(engine, bx + 0.08, by + 0.06, shape.label(), if selected { Vec4::ONE } else { theme.fg_dim }, 0.06, RenderLayer::UI);
        }
        let rows = (FormationShape::all().len() + cols - 1) / cols;
        cy -= rows as f32 * (btn_h + 0.06) + 0.2;

        // Glyph palette
        WidgetDraw::text(engine, x + 0.3, cy, "Glyph:", theme.fg_dim, 0.07, RenderLayer::UI);
        let glyph_chars: &[char] = &['@', '#', '*', '+', 'o', 'O', '0', 'x', 'X', '.', '~', '^', '|', '-'];
        let mut gx = col2;
        for &ch in glyph_chars {
            let selected = self.formation_state.glyph_char == ch;
            let bg = if selected { theme.accent } else { Vec4::new(0.12, 0.12, 0.18, 0.9) };
            WidgetDraw::fill_rect(engine, Rect::new(gx, cy - 0.02, 0.38, 0.4), bg);
            WidgetDraw::text(engine, gx + 0.08, cy + 0.03, &ch.to_string(), if selected { Vec4::ONE } else { theme.fg }, 0.09, RenderLayer::UI);
            gx += 0.44;
            if gx > x + width - 0.5 { gx = col2; cy -= 0.46; }
        }
        cy -= 0.5;

        // Slot color
        WidgetDraw::text(engine, x + 0.3, cy, "Color:", theme.fg_dim, 0.07, RenderLayer::UI);
        let c = self.formation_state.slot_color;
        WidgetDraw::fill_rect(engine, Rect::new(col2, cy, 0.8, 0.38), c);
        WidgetDraw::text(engine, col2 + 0.9, cy + 0.04,
            &format!("({:.2},{:.2},{:.2})", c.x, c.y, c.z),
            theme.fg_dim, 0.065, RenderLayer::UI);
        cy -= 0.5;

        // Count
        WidgetDraw::text(engine, x + 0.3, cy, "Count:", theme.fg_dim, 0.07, RenderLayer::UI);
        let cnt_pct = (self.formation_state.count as f32 - 3.0) / 97.0;
        WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.46, cnt_pct, theme.accent, theme.bg);
        WidgetDraw::text(engine, col2 + width * 0.48, cy + 0.04,
            &format!("{}", self.formation_state.count), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Size
        WidgetDraw::text(engine, x + 0.3, cy, "Size:", theme.fg_dim, 0.07, RenderLayer::UI);
        let size_pct = (self.formation_state.size / 5.0).clamp(0.0, 1.0);
        WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.46, size_pct, theme.accent * Vec4::new(0.6, 1.0, 0.6, 1.0), theme.bg);
        WidgetDraw::text(engine, col2 + width * 0.48, cy + 0.04,
            &format!("{:.2}", self.formation_state.size), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Density
        WidgetDraw::text(engine, x + 0.3, cy, "Density:", theme.fg_dim, 0.07, RenderLayer::UI);
        WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.46,
            (self.formation_state.density / 3.0).clamp(0.0, 1.0),
            theme.accent * Vec4::new(1.0, 0.8, 0.4, 1.0), theme.bg);
        WidgetDraw::text(engine, col2 + width * 0.48, cy + 0.04,
            &format!("{:.2}", self.formation_state.density), theme.fg_dim, 0.07, RenderLayer::UI);
        cy -= 0.5;

        // Live preview — draw dots
        WidgetDraw::separator(engine, x + 0.2, cy, width - 0.4, theme.separator);
        cy -= 0.25;
        WidgetDraw::text(engine, x + 0.3, cy, "Preview:", theme.fg_dim, 0.065, RenderLayer::UI);
        cy -= 0.35;

        let preview_size = 3.5;
        let preview_rect = Rect::new(x + (width - preview_size) * 0.5, cy - preview_size, preview_size, preview_size);
        WidgetDraw::fill_rect(engine, preview_rect, Vec4::new(0.06, 0.06, 0.1, 0.9));

        let dots = self.formation_state.shape.dot_positions(self.formation_state.count);
        let scale = self.formation_state.size * self.formation_state.density;
        for (nx, ny) in dots {
            let px = preview_rect.x + nx * preview_rect.w * scale.min(1.0) + preview_rect.w * (1.0 - scale.min(1.0)) * 0.5;
            let py = preview_rect.y - ny * preview_rect.h * scale.min(1.0) - preview_rect.h * (1.0 - scale.min(1.0)) * 0.5;
            let c = self.formation_state.slot_color;
            WidgetDraw::text(engine, px, py,
                &self.formation_state.glyph_char.to_string(),
                c, 0.09, RenderLayer::UI);
        }
        cy -= preview_size + 0.3;

        y - cy
    }

    // ── Math Function Rendering ────────────────────────────────────────────

    fn render_math_fn_section(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, theme: &WidgetTheme) -> f32 {
        let mut cy = y;
        let col2 = x + width * 0.42;

        // Function type dropdown
        WidgetDraw::text(engine, x + 0.3, cy, "Function:", theme.fg_dim, 0.07, RenderLayer::UI);
        let fn_label = MathFunctionType::all().get(self.math_fn_type_idx)
            .map(|f| f.label()).unwrap_or("Breathing");
        WidgetDraw::fill_rect(engine, Rect::new(col2, cy - 0.02, width * 0.55, 0.42), Vec4::new(0.14, 0.14, 0.2, 0.9));
        WidgetDraw::text(engine, col2 + 0.1, cy + 0.03, fn_label, theme.accent, 0.08, RenderLayer::UI);
        WidgetDraw::text(engine, col2 + width * 0.48, cy + 0.03, "v", theme.fg_dim, 0.08, RenderLayer::UI);
        cy -= 0.52;

        // Per-function parameter sliders
        let fn_type = MathFunctionType::all()[self.math_fn_type_idx.min(MathFunctionType::all().len() - 1)];
        let param_labels = fn_type.param_labels();
        for (i, &plabel) in param_labels.iter().enumerate() {
            let val = self.math_fn_state.params.get(i).copied().unwrap_or(0.0);
            WidgetDraw::text(engine, x + 0.3, cy, &format!("{}:", plabel), theme.fg_dim, 0.07, RenderLayer::UI);
            let range = if plabel.contains("Scale") { 5.0 } else if plabel.contains("Radius") { 20.0 } else { 10.0 };
            let pct = (val / range).clamp(0.0, 1.0);
            WidgetDraw::bar(engine, col2, cy + 0.08, width * 0.5, pct, theme.accent, theme.bg);
            WidgetDraw::text(engine, col2 + width * 0.52, cy + 0.04, &format!("{:.2}", val), theme.fg_dim, 0.07, RenderLayer::UI);
            cy -= 0.48;
        }

        // Preview canvas
        WidgetDraw::separator(engine, x + 0.2, cy, width - 0.4, theme.separator);
        cy -= 0.25;
        WidgetDraw::text(engine, x + 0.3, cy, "Preview:", theme.fg_dim, 0.065, RenderLayer::UI);
        cy -= 0.35;

        let preview_w = width - 0.8;
        let preview_h = 2.5;
        WidgetDraw::fill_rect(engine, Rect::new(x + 0.3, cy - preview_h, preview_w, preview_h), Vec4::new(0.06, 0.06, 0.1, 0.9));

        // Draw function path
        if self.math_fn_state.preview_points.len() >= 2 {
            let pts = &self.math_fn_state.preview_points;
            // Normalize range
            let min_x = pts.iter().map(|p| p.0).fold(f32::MAX, f32::min);
            let max_x = pts.iter().map(|p| p.0).fold(f32::MIN, f32::max);
            let min_y = pts.iter().map(|p| p.1).fold(f32::MAX, f32::min);
            let max_y = pts.iter().map(|p| p.1).fold(f32::MIN, f32::max);
            let rx = (max_x - min_x).max(0.001);
            let ry = (max_y - min_y).max(0.001);
            for (idx, &(px, py)) in pts.iter().enumerate() {
                let nx = (px - min_x) / rx;
                let ny = 1.0 - (py - min_y) / ry;
                let dot_x = x + 0.3 + nx * preview_w;
                let dot_y = cy - ny * preview_h;
                let t = idx as f32 / pts.len() as f32;
                let col = Vec4::new(0.4 + t * 0.6, 0.8 - t * 0.4, 1.0 - t * 0.3, 0.9);
                WidgetDraw::text(engine, dot_x, dot_y, ".", col, 0.05, RenderLayer::UI);
            }
        } else {
            WidgetDraw::text(engine, x + width * 0.3, cy - preview_h * 0.5, "(no preview)", theme.fg_dim, 0.07, RenderLayer::UI);
        }
        cy -= preview_h + 0.3;

        y - cy
    }

    // ── Multi-Select Banner ────────────────────────────────────────────────

    fn render_multi_select_banner(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, field_name: &str, theme: &WidgetTheme) {
        if !self.multi_select.is_multi() { return; }
        WidgetDraw::fill_rect(engine, Rect::new(x, y, width, 0.35), Vec4::new(0.3, 0.25, 0.05, 0.5));
        WidgetDraw::text(engine, x + 0.2, y + 0.03,
            &format!("(multiple) {}", field_name), Vec4::new(1.0, 0.8, 0.2, 0.9), 0.065, RenderLayer::UI);
    }

    /// Render the property editor for a given node kind.
    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, kind: NodeKind, theme: &WidgetTheme) {
        let sections = PropertySection::for_kind(kind);
        let mut cy = y;

        // Multi-select top banner
        if self.multi_select.is_multi() {
            WidgetDraw::fill_rect(engine, Rect::new(x, cy, width, 0.4), Vec4::new(0.2, 0.18, 0.05, 0.7));
            WidgetDraw::text(engine, x + 0.3, cy + 0.05,
                &format!("{} nodes selected — editing all", self.multi_select.selected_ids.len()),
                Vec4::new(1.0, 0.85, 0.3, 1.0), 0.08, RenderLayer::UI);
            cy -= 0.5;
        }

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
                    let used = self.render_physics_section(engine, x, cy, width, theme);
                    cy -= used;
                }
                PropertySection::MathFunction => {
                    let used = self.render_math_fn_section(engine, x, cy, width, theme);
                    cy -= used;
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
                    let used = self.render_formation_section(engine, x, cy, width, theme);
                    cy -= 3.5 + used;
                }
                PropertySection::Tags => {
                    let used = self.render_tags_section(engine, x, cy, width, kind, theme);
                    cy -= used;
                }
                PropertySection::Advanced => {
                    let used = self.render_advanced_section(engine, x, cy, width, theme);
                    cy -= used;
                }
            }
            cy -= 0.3; // gap between sections
        }
    }
}

// ── Global tag registry helper ────────────────────────────────────────────────

/// Collect all unique tags from all scene nodes.
pub fn collect_global_tags(doc: &SceneDocument) -> Vec<String> {
    let mut tags: Vec<String> = doc.nodes()
        .flat_map(|n| n.tags.iter().cloned())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

// ── Preset advanced settings per node kind ───────────────────────────────────

pub fn default_advanced_for_kind(kind: NodeKind) -> AdvancedNodeSettings {
    match kind {
        NodeKind::Glyph => AdvancedNodeSettings {
            blend_mode: BlendMode::Additive,
            render_layer: NodeRenderLayer::Entity,
            z_order: 0.0,
            finite_lifetime: false,
            lifetime_seconds: 5.0,
            physics: PhysicsOverride {
                mass_override: 1.0,
                use_mass_override: false,
                is_static: false,
                is_trigger: false,
                collision_response: CollisionResponse::Bounce,
            },
            script_slots: vec![
                ScriptSlot::new("OnSpawn"),
                ScriptSlot::new("OnUpdate"),
                ScriptSlot::new("OnDeath"),
            ],
        },
        NodeKind::Entity => AdvancedNodeSettings {
            blend_mode: BlendMode::Normal,
            render_layer: NodeRenderLayer::Entity,
            z_order: 10.0,
            finite_lifetime: false,
            lifetime_seconds: 0.0,
            physics: PhysicsOverride {
                mass_override: 5.0,
                use_mass_override: true,
                is_static: false,
                is_trigger: false,
                collision_response: CollisionResponse::Bounce,
            },
            script_slots: vec![
                ScriptSlot::new("OnSpawn"),
                ScriptSlot::new("OnUpdate"),
                ScriptSlot::new("OnDeath"),
                ScriptSlot::new("OnHit"),
                ScriptSlot::new("OnKill"),
            ],
        },
        NodeKind::Field => AdvancedNodeSettings {
            blend_mode: BlendMode::Screen,
            render_layer: NodeRenderLayer::Background,
            z_order: -10.0,
            finite_lifetime: false,
            lifetime_seconds: 0.0,
            physics: PhysicsOverride {
                mass_override: 0.0,
                use_mass_override: false,
                is_static: true,
                is_trigger: true,
                collision_response: CollisionResponse::PassThrough,
            },
            script_slots: vec![
                ScriptSlot::new("OnEnter"),
                ScriptSlot::new("OnExit"),
                ScriptSlot::new("OnTick"),
            ],
        },
        NodeKind::Group => AdvancedNodeSettings {
            blend_mode: BlendMode::Normal,
            render_layer: NodeRenderLayer::Entity,
            z_order: 0.0,
            finite_lifetime: false,
            lifetime_seconds: 0.0,
            physics: PhysicsOverride::default(),
            script_slots: vec![ScriptSlot::new("OnGroupTrigger")],
        },
        NodeKind::Camera => AdvancedNodeSettings {
            blend_mode: BlendMode::Normal,
            render_layer: NodeRenderLayer::UI,
            z_order: 1000.0,
            finite_lifetime: false,
            lifetime_seconds: 0.0,
            physics: PhysicsOverride {
                mass_override: 0.0,
                use_mass_override: false,
                is_static: true,
                is_trigger: false,
                collision_response: CollisionResponse::PassThrough,
            },
            script_slots: vec![
                ScriptSlot::new("OnTransition"),
                ScriptSlot::new("OnCut"),
            ],
        },
    }
}

// ── Tick: update math function preview ───────────────────────────────────────

impl PropertyEditor {
    pub fn tick(&mut self, dt: f32) {
        self.math_fn_state.preview_time += dt;
        if (self.math_fn_state.preview_time * 2.0) as u32 % 30 == 0 {
            self.math_fn_state.rebuild_preview(64);
        }
    }

    /// Call this whenever the function type or params change.
    pub fn refresh_math_preview(&mut self) {
        self.math_fn_state.fn_type = MathFunctionType::all()
            [self.math_fn_type_idx.min(MathFunctionType::all().len() - 1)];
        self.math_fn_state.rebuild_preview(64);
    }

    /// Call this when formation shape/count changes.
    pub fn refresh_formation(&mut self) {
        self.formation_state.shape = FormationShape::all()
            [self.formation_shape_idx.min(FormationShape::all().len() - 1)];
    }

    /// Call when global tags are updated.
    pub fn sync_global_tags(&mut self, doc: &SceneDocument) {
        self.global_tags = collect_global_tags(doc);
    }

    /// Load advanced settings from serialized node data (if stored in metadata).
    pub fn load_advanced(&mut self, _node: &SceneNode) {
        // In a full implementation this would deserialize from node metadata.
        // For now, apply kind-appropriate defaults.
        // self.advanced = default_advanced_for_kind(node.kind);
    }
}
