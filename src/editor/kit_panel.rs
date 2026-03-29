//! Kit Parameter Panel — real-time sliders for every rendering kit.
//!
//! # Overview
//!
//! The kit panel exposes every tunable parameter from all eight kits:
//! LightingKit, MaterialKit, ClothingKit, HairKit, PhysicsKit, RenderKit,
//! BoneKit (envelope radii), and ModelKit (cross-section knots), plus the full
//! post-processing pipeline (bloom, AO, SSS, Fresnel, etc.).
//!
//! All changes are immediately reflected in the viewport.  Every slider
//! mutation records a `KitEdit` into the undo stack so changes are reversible.
//!
//! # Slider types
//!
//! - `FloatSlider` — clamped f32 with optional logarithmic scale.
//! - `ColorPicker` — RGBA colour with hex input and HSV wheel.
//! - `Vec3Field`   — three linked floats (drag-any-axis).
//! - `EnumPicker`  — dropdown for tagged enum fields.
//! - `BoolToggle`  — checkbox for on/off flags.
//! - `IntSlider`   — clamped i32.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Slider value types
// ─────────────────────────────────────────────────────────────────────────────

/// A clamped f32 with optional logarithmic mapping.
#[derive(Debug, Clone)]
pub struct FloatSlider {
    pub label:     String,
    pub value:     f32,
    pub min:       f32,
    pub max:       f32,
    pub step:      f32,
    pub logarithmic: bool,
    pub tooltip:   Option<String>,
    pub unit:      Option<&'static str>,
    pub precision: usize,
}

impl FloatSlider {
    pub fn new(label: impl Into<String>, value: f32, min: f32, max: f32) -> Self {
        Self {
            label: label.into(), value, min, max,
            step: (max - min) / 1000.0,
            logarithmic: false,
            tooltip: None,
            unit: None,
            precision: 3,
        }
    }

    pub fn log(mut self) -> Self { self.logarithmic = true; self }
    pub fn unit(mut self, u: &'static str) -> Self { self.unit = Some(u); self }
    pub fn tip(mut self, t: impl Into<String>) -> Self { self.tooltip = Some(t.into()); self }
    pub fn prec(mut self, p: usize) -> Self { self.precision = p; self }

    pub fn set(&mut self, v: f32) {
        self.value = if self.logarithmic {
            v.clamp(self.min.max(1e-9), self.max)
        } else {
            v.clamp(self.min, self.max)
        };
    }

    pub fn normalized(&self) -> f32 {
        if self.logarithmic {
            let a = self.min.max(1e-9).ln();
            let b = self.max.ln();
            (self.value.max(1e-9).ln() - a) / (b - a)
        } else {
            (self.value - self.min) / (self.max - self.min)
        }
    }

    pub fn set_normalized(&mut self, t: f32) {
        let v = if self.logarithmic {
            let a = self.min.max(1e-9).ln();
            let b = self.max.ln();
            (a + t * (b - a)).exp()
        } else {
            self.min + t * (self.max - self.min)
        };
        self.set(v);
    }

    pub fn display(&self) -> String {
        if let Some(u) = self.unit {
            format!("{:.prec$} {u}", self.value, prec = self.precision)
        } else {
            format!("{:.prec$}", self.value, prec = self.precision)
        }
    }
}

/// A clamped i32 slider.
#[derive(Debug, Clone)]
pub struct IntSlider {
    pub label: String,
    pub value: i32,
    pub min:   i32,
    pub max:   i32,
    pub step:  i32,
    pub tooltip: Option<String>,
}

impl IntSlider {
    pub fn new(label: impl Into<String>, value: i32, min: i32, max: i32) -> Self {
        Self { label: label.into(), value, min, max, step: 1, tooltip: None }
    }
    pub fn set(&mut self, v: i32) { self.value = v.clamp(self.min, self.max); }
    pub fn increment(&mut self) { self.set(self.value + self.step); }
    pub fn decrement(&mut self) { self.set(self.value - self.step); }
}

/// An RGBA colour picker.
#[derive(Debug, Clone)]
pub struct ColorPicker {
    pub label: String,
    pub value: Vec4,
    pub hdr:   bool,
    pub tooltip: Option<String>,
}

impl ColorPicker {
    pub fn new(label: impl Into<String>, value: Vec4) -> Self {
        Self { label: label.into(), value, hdr: false, tooltip: None }
    }
    pub fn hdr(mut self) -> Self { self.hdr = true; self }

    pub fn to_hex(&self) -> String {
        let r = (self.value.x.clamp(0.0, 1.0) * 255.0) as u8;
        let g = (self.value.y.clamp(0.0, 1.0) * 255.0) as u8;
        let b = (self.value.z.clamp(0.0, 1.0) * 255.0) as u8;
        let a = (self.value.w.clamp(0.0, 1.0) * 255.0) as u8;
        format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
    }

    pub fn from_hex(s: &str) -> Option<Vec4> {
        let s = s.trim_start_matches('#');
        if s.len() < 6 { return None; }
        let r = u8::from_str_radix(&s[0..2], 16).ok()? as f32 / 255.0;
        let g = u8::from_str_radix(&s[2..4], 16).ok()? as f32 / 255.0;
        let b = u8::from_str_radix(&s[4..6], 16).ok()? as f32 / 255.0;
        let a = if s.len() >= 8 {
            u8::from_str_radix(&s[6..8], 16).ok()? as f32 / 255.0
        } else { 1.0 };
        Some(Vec4::new(r, g, b, a))
    }

    pub fn to_hsv(&self) -> (f32, f32, f32) {
        let r = self.value.x; let g = self.value.y; let b = self.value.z;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let v = max;
        let s = if max > 0.0 { delta / max } else { 0.0 };
        let h = if delta < 1e-6 { 0.0 }
        else if max == r { 60.0 * (((g - b) / delta).rem_euclid(6.0)) }
        else if max == g { 60.0 * ((b - r) / delta + 2.0) }
        else             { 60.0 * ((r - g) / delta + 4.0) };
        (h, s, v)
    }
}

/// A boolean toggle.
#[derive(Debug, Clone)]
pub struct BoolToggle {
    pub label: String,
    pub value: bool,
    pub tooltip: Option<String>,
}

impl BoolToggle {
    pub fn new(label: impl Into<String>, value: bool) -> Self {
        Self { label: label.into(), value, tooltip: None }
    }
    pub fn toggle(&mut self) { self.value = !self.value; }
}

/// A 3-component Vec3 field with linked/unlinked dragging.
#[derive(Debug, Clone)]
pub struct Vec3Field {
    pub label:  String,
    pub value:  Vec3,
    pub min:    f32,
    pub max:    f32,
    pub step:   f32,
    pub linked: bool,
    pub tooltip: Option<String>,
}

impl Vec3Field {
    pub fn new(label: impl Into<String>, value: Vec3, min: f32, max: f32) -> Self {
        Self { label: label.into(), value, min, max, step: 0.01, linked: false, tooltip: None }
    }

    pub fn set_x(&mut self, v: f32) {
        let v = v.clamp(self.min, self.max);
        if self.linked {
            let scale = if self.value.x.abs() > 1e-6 { v / self.value.x } else { 1.0 };
            self.value *= scale;
        } else {
            self.value.x = v;
        }
    }
    pub fn set_y(&mut self, v: f32) {
        let v = v.clamp(self.min, self.max);
        if self.linked {
            let scale = if self.value.y.abs() > 1e-6 { v / self.value.y } else { 1.0 };
            self.value *= scale;
        } else {
            self.value.y = v;
        }
    }
    pub fn set_z(&mut self, v: f32) {
        let v = v.clamp(self.min, self.max);
        if self.linked {
            let scale = if self.value.z.abs() > 1e-6 { v / self.value.z } else { 1.0 };
            self.value *= scale;
        } else {
            self.value.z = v;
        }
    }
}

/// An enum dropdown field.
#[derive(Debug, Clone)]
pub struct EnumPicker {
    pub label:   String,
    pub options: Vec<String>,
    pub index:   usize,
    pub tooltip: Option<String>,
}

impl EnumPicker {
    pub fn new(label: impl Into<String>, options: Vec<String>, index: usize) -> Self {
        Self { label: label.into(), options, index, tooltip: None }
    }
    pub fn selected(&self) -> &str {
        self.options.get(self.index).map(|s| s.as_str()).unwrap_or("")
    }
    pub fn select(&mut self, idx: usize) { self.index = idx.min(self.options.len().saturating_sub(1)); }
    pub fn next(&mut self) { self.select(self.index + 1); }
    pub fn prev(&mut self) { self.select(self.index.saturating_sub(1)); }
}

// ─────────────────────────────────────────────────────────────────────────────
// KitParam — a single parameter in the panel
// ─────────────────────────────────────────────────────────────────────────────

/// The discriminant for a kit parameter widget.
#[derive(Debug, Clone)]
pub enum KitParam {
    Float(FloatSlider),
    Int(IntSlider),
    Color(ColorPicker),
    Vec3(Vec3Field),
    Bool(BoolToggle),
    Enum(EnumPicker),
    Separator(String),
    Header(String),
}

impl KitParam {
    pub fn label(&self) -> &str {
        match self {
            KitParam::Float(s) => &s.label,
            KitParam::Int(s)   => &s.label,
            KitParam::Color(s) => &s.label,
            KitParam::Vec3(s)  => &s.label,
            KitParam::Bool(s)  => &s.label,
            KitParam::Enum(s)  => &s.label,
            KitParam::Separator(l) => l,
            KitParam::Header(l)    => l,
        }
    }

    pub fn is_editable(&self) -> bool {
        !matches!(self, KitParam::Separator(_) | KitParam::Header(_))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// KitGroup
// ─────────────────────────────────────────────────────────────────────────────

/// A collapsible group of parameters (one per kit).
#[derive(Debug, Clone)]
pub struct KitGroup {
    pub name:      String,
    pub collapsed: bool,
    pub params:    Vec<(String, KitParam)>,
}

impl KitGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), collapsed: false, params: Vec::new() }
    }

    pub fn push(&mut self, key: impl Into<String>, param: KitParam) {
        self.params.push((key.into(), param));
    }

    pub fn get(&self, key: &str) -> Option<&KitParam> {
        self.params.iter().find(|(k, _)| k == key).map(|(_, p)| p)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut KitParam> {
        self.params.iter_mut().find(|(k, _)| k == key).map(|(_, p)| p)
    }

    pub fn get_float(&self, key: &str) -> Option<f32> {
        match self.get(key)? {
            KitParam::Float(s) => Some(s.value),
            _ => None,
        }
    }

    pub fn set_float(&mut self, key: &str, v: f32) {
        if let Some(KitParam::Float(s)) = self.get_mut(key) { s.set(v); }
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            KitParam::Bool(b) => Some(b.value),
            _ => None,
        }
    }

    pub fn get_color(&self, key: &str) -> Option<Vec4> {
        match self.get(key)? {
            KitParam::Color(c) => Some(c.value),
            _ => None,
        }
    }

    pub fn get_vec3(&self, key: &str) -> Option<Vec3> {
        match self.get(key)? {
            KitParam::Vec3(v) => Some(v.value),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// KitPanelState — the full state of all kit sliders
// ─────────────────────────────────────────────────────────────────────────────

/// A single mutation event for undo purposes.
#[derive(Debug, Clone)]
pub struct KitEdit {
    pub group:    String,
    pub key:      String,
    pub old:      KitEditValue,
    pub new:      KitEditValue,
}

#[derive(Debug, Clone)]
pub enum KitEditValue {
    Float(f32),
    Int(i32),
    Color(Vec4),
    Vec3(Vec3),
    Bool(bool),
    Enum(usize),
}

/// The top-level kit parameter panel.
#[derive(Debug)]
pub struct KitPanel {
    pub groups:    Vec<KitGroup>,
    undo_stack:    Vec<KitEdit>,
    redo_stack:    Vec<KitEdit>,
    pub search:    String,
    pub dirty:     bool,
}

impl KitPanel {
    pub fn new() -> Self {
        let mut panel = Self {
            groups: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search: String::new(),
            dirty: false,
        };
        panel.build_all_groups();
        panel
    }

    fn build_all_groups(&mut self) {
        self.groups.push(Self::build_lighting_kit());
        self.groups.push(Self::build_bloom_group());
        self.groups.push(Self::build_ao_group());
        self.groups.push(Self::build_sss_group());
        self.groups.push(Self::build_hair_kit());
        self.groups.push(Self::build_cloth_kit());
        self.groups.push(Self::build_physics_kit());
        self.groups.push(Self::build_render_kit());
        self.groups.push(Self::build_postfx_group());
        self.groups.push(Self::build_camera_group());
        self.groups.push(Self::build_env_group());
    }

    // ── Kit builders ──────────────────────────────────────────────────────

    fn build_lighting_kit() -> KitGroup {
        let mut g = KitGroup::new("LightingKit");
        g.push("header_key",      KitParam::Header("Key Light".into()));
        g.push("key_dir",         KitParam::Vec3(Vec3Field::new("Key Direction", Vec3::new(-0.6, -0.8, 0.4), -1.0, 1.0)));
        g.push("key_color",       KitParam::Color(ColorPicker::new("Key Color", Vec4::new(1.0, 0.97, 0.90, 1.0))));
        g.push("key_intensity",   KitParam::Float(FloatSlider::new("Key Intensity", 2.2, 0.0, 10.0).unit("lx")));
        g.push("sep_fills",       KitParam::Separator("Fill Lights".into()));
        g.push("fill1_dir",       KitParam::Vec3(Vec3Field::new("Fill1 Dir", Vec3::new(0.8, -0.3, 0.5), -1.0, 1.0)));
        g.push("fill1_color",     KitParam::Color(ColorPicker::new("Fill1 Color", Vec4::new(0.5, 0.6, 0.9, 1.0))));
        g.push("fill1_intensity", KitParam::Float(FloatSlider::new("Fill1 Intensity", 0.6, 0.0, 5.0)));
        g.push("fill2_dir",       KitParam::Vec3(Vec3Field::new("Fill2 Dir", Vec3::new(-0.2, 0.5, -0.8), -1.0, 1.0)));
        g.push("fill2_color",     KitParam::Color(ColorPicker::new("Fill2 Color", Vec4::new(0.2, 0.3, 0.5, 1.0))));
        g.push("fill2_intensity", KitParam::Float(FloatSlider::new("Fill2 Intensity", 0.3, 0.0, 5.0)));
        g.push("fill3_dir",       KitParam::Vec3(Vec3Field::new("Fill3 Dir", Vec3::new(0.0, 0.8, 0.0), -1.0, 1.0)));
        g.push("fill3_color",     KitParam::Color(ColorPicker::new("Fill3 Color", Vec4::new(0.3, 0.4, 0.6, 1.0))));
        g.push("fill3_intensity", KitParam::Float(FloatSlider::new("Fill3 Intensity", 0.2, 0.0, 5.0)));
        g.push("sep_rim",         KitParam::Separator("Rim Light".into()));
        g.push("rim_dir",         KitParam::Vec3(Vec3Field::new("Rim Dir", Vec3::new(0.0, 0.2, -1.0), -1.0, 1.0)));
        g.push("rim_color",       KitParam::Color(ColorPicker::new("Rim Color", Vec4::new(0.9, 0.95, 1.0, 1.0))));
        g.push("rim_intensity",   KitParam::Float(FloatSlider::new("Rim Intensity", 1.8, 0.0, 8.0)));
        g.push("sep_ambient",     KitParam::Separator("Hemisphere Ambient".into()));
        g.push("ambient_sky",     KitParam::Color(ColorPicker::new("Sky Color", Vec4::new(0.3, 0.45, 0.7, 1.0))));
        g.push("ambient_ground",  KitParam::Color(ColorPicker::new("Ground Color", Vec4::new(0.2, 0.18, 0.15, 1.0))));
        g.push("ambient_strength",KitParam::Float(FloatSlider::new("Ambient Strength", 0.4, 0.0, 2.0)));
        g.push("tonemap",         KitParam::Enum(EnumPicker::new("Tonemap",
            vec!["ACES".into(),"Reinhard".into(),"Filmic".into(),"Uncharted2".into(),"Linear".into()], 0)));
        g.push("exposure",        KitParam::Float(FloatSlider::new("Exposure", 1.0, 0.1, 8.0).log()));
        g
    }

    fn build_bloom_group() -> KitGroup {
        let mut g = KitGroup::new("Bloom");
        g.push("enabled",     KitParam::Bool(BoolToggle::new("Bloom Enabled", true)));
        g.push("intensity",   KitParam::Float(FloatSlider::new("Intensity", 2.8, 0.0, 20.0)));
        g.push("radius",      KitParam::Float(FloatSlider::new("Radius", 12.0, 0.5, 128.0)));
        g.push("threshold",   KitParam::Float(FloatSlider::new("Threshold", 0.8, 0.0, 4.0)));
        g.push("scatter",     KitParam::Float(FloatSlider::new("Scatter", 0.7, 0.0, 1.0)));
        g.push("tint",        KitParam::Color(ColorPicker::new("Tint", Vec4::new(1.0, 0.95, 0.85, 1.0))));
        g.push("spectral",    KitParam::Bool(BoolToggle::new("Spectral Dispersion", true)));
        g.push("lut_enabled", KitParam::Bool(BoolToggle::new("LUT Grade", false)));
        g.push("chromatic",   KitParam::Float(FloatSlider::new("Chromatic Aberration", 0.0028, 0.0, 0.2).prec(4)));
        g.push("grain",       KitParam::Float(FloatSlider::new("Film Grain", 0.025, 0.0, 0.5)));
        g.push("vignette",    KitParam::Float(FloatSlider::new("Vignette", 0.3, 0.0, 1.0)));
        g
    }

    fn build_ao_group() -> KitGroup {
        let mut g = KitGroup::new("AmbientOcclusion");
        g.push("enabled",    KitParam::Bool(BoolToggle::new("SDF AO Enabled", true)));
        g.push("strength",   KitParam::Float(FloatSlider::new("AO Strength", 1.0, 0.0, 3.0)));
        g.push("steps",      KitParam::Int(IntSlider::new("AO Steps", 5, 2, 12)));
        g.push("step_size",  KitParam::Float(FloatSlider::new("Step Size", 0.012, 0.001, 0.1)));
        g.push("decay",      KitParam::Float(FloatSlider::new("Geometric Decay", 0.82, 0.1, 1.0)));
        g.push("falloff",    KitParam::Float(FloatSlider::new("Falloff Power", 1.2, 0.1, 4.0)));
        g.push("tint",       KitParam::Color(ColorPicker::new("AO Tint", Vec4::new(0.05, 0.05, 0.08, 1.0))));
        g
    }

    fn build_sss_group() -> KitGroup {
        let mut g = KitGroup::new("SubsurfaceScattering");
        g.push("enabled",    KitParam::Bool(BoolToggle::new("SSS Enabled", true)));
        g.push("strength",   KitParam::Float(FloatSlider::new("SSS Strength", 0.6, 0.0, 2.0)));
        g.push("thickness",  KitParam::Float(FloatSlider::new("Probe Depth", 0.04, 0.001, 0.2)));
        g.push("scatter_r",  KitParam::Float(FloatSlider::new("Scatter R", 0.8, 0.0, 4.0)));
        g.push("scatter_g",  KitParam::Float(FloatSlider::new("Scatter G", 0.4, 0.0, 4.0)));
        g.push("scatter_b",  KitParam::Float(FloatSlider::new("Scatter B", 0.3, 0.0, 4.0)));
        g.push("tint",       KitParam::Color(ColorPicker::new("SSS Tint", Vec4::new(0.9, 0.4, 0.3, 1.0))));
        g.push("distortion", KitParam::Float(FloatSlider::new("Normal Distortion", 0.1, 0.0, 1.0)));
        g.push("power",      KitParam::Float(FloatSlider::new("Scatter Power", 1.4, 0.1, 8.0)));
        g
    }

    fn build_hair_kit() -> KitGroup {
        let mut g = KitGroup::new("HairKit");
        g.push("strands",      KitParam::Int(IntSlider::new("Strand Count", 500, 10, 2000)));
        g.push("length",       KitParam::Float(FloatSlider::new("Length", 0.18, 0.01, 1.0)));
        g.push("width",        KitParam::Float(FloatSlider::new("Width", 0.004, 0.001, 0.04)));
        g.push("wave_freq",    KitParam::Float(FloatSlider::new("Wave Frequency", 2.5, 0.0, 20.0)));
        g.push("wave_amp",     KitParam::Float(FloatSlider::new("Wave Amplitude", 0.015, 0.0, 0.1)));
        g.push("root_color",   KitParam::Color(ColorPicker::new("Root Color", Vec4::new(0.12, 0.08, 0.05, 1.0))));
        g.push("tip_color",    KitParam::Color(ColorPicker::new("Tip Color", Vec4::new(0.20, 0.15, 0.09, 1.0))));
        g.push("kk_primary",   KitParam::Float(FloatSlider::new("KK Primary Spec", 0.4, 0.0, 2.0)));
        g.push("kk_secondary", KitParam::Float(FloatSlider::new("KK Secondary Spec", 0.2, 0.0, 1.0)));
        g.push("kk_shift",     KitParam::Float(FloatSlider::new("KK Spec Shift", 0.03, -0.1, 0.1)));
        g.push("anisotropy",   KitParam::Float(FloatSlider::new("Anisotropy", 0.85, 0.0, 1.0)));
        g.push("clump",        KitParam::Float(FloatSlider::new("Clump Factor", 0.3, 0.0, 1.0)));
        g.push("scatter",      KitParam::Float(FloatSlider::new("Scatter Density", 12.0, 1.0, 30.0)));
        g
    }

    fn build_cloth_kit() -> KitGroup {
        let mut g = KitGroup::new("ClothingKit");
        g.push("push_radius",  KitParam::Float(FloatSlider::new("Garment Push Radius", 0.015, 0.0, 0.1)));
        g.push("seam_dark",    KitParam::Float(FloatSlider::new("Seam Darkening", 0.3, 0.0, 1.0)));
        g.push("contact_shad", KitParam::Float(FloatSlider::new("Contact Shadow", 0.4, 0.0, 1.0)));
        g.push("wrinkle_amp",  KitParam::Float(FloatSlider::new("Wrinkle Amplitude", 0.008, 0.0, 0.05)));
        g.push("wrinkle_freq", KitParam::Float(FloatSlider::new("Wrinkle Frequency", 18.0, 1.0, 60.0)));
        g.push("fabric_roughness", KitParam::Float(FloatSlider::new("Fabric Roughness", 0.85, 0.0, 1.0)));
        g.push("iridescence",  KitParam::Float(FloatSlider::new("Leather Iridescence", 0.1, 0.0, 1.0)));
        g.push("thin_film_ior",KitParam::Float(FloatSlider::new("Thin-Film IOR", 1.45, 1.0, 3.0)));
        g
    }

    fn build_physics_kit() -> KitGroup {
        let mut g = KitGroup::new("PhysicsKit");
        g.push("inertia_hair",   KitParam::Float(FloatSlider::new("Hair Inertia", 0.92, 0.0, 1.0)));
        g.push("inertia_cloth",  KitParam::Float(FloatSlider::new("Cloth Inertia", 0.85, 0.0, 1.0)));
        g.push("inertia_skin",   KitParam::Float(FloatSlider::new("Skin Inertia", 0.98, 0.0, 1.0)));
        g.push("gravity",        KitParam::Vec3(Vec3Field::new("Gravity", Vec3::new(0.0, -9.81, 0.0), -20.0, 20.0)));
        g.push("wind_dir",       KitParam::Vec3(Vec3Field::new("Wind Direction", Vec3::new(1.0, 0.0, 0.0), -1.0, 1.0)));
        g.push("wind_strength",  KitParam::Float(FloatSlider::new("Wind Strength", 0.0, 0.0, 5.0)));
        g.push("drag",           KitParam::Float(FloatSlider::new("Air Drag", 0.02, 0.0, 1.0)));
        g.push("collision",      KitParam::Bool(BoolToggle::new("SDF Collision", true)));
        g.push("substeps",       KitParam::Int(IntSlider::new("Substeps", 4, 1, 16)));
        g
    }

    fn build_render_kit() -> KitGroup {
        let mut g = KitGroup::new("RenderKit");
        g.push("render_scale",   KitParam::Float(FloatSlider::new("Render Scale", 1.0, 0.25, 8.0)));
        g.push("n_copies",       KitParam::Int(IntSlider::new("Oversampling (n_copies)", 1, 1, 32)));
        g.push("dof_enabled",    KitParam::Bool(BoolToggle::new("Depth of Field", false)));
        g.push("dof_focus",      KitParam::Float(FloatSlider::new("DoF Focus Dist", 2.0, 0.1, 50.0)));
        g.push("dof_aperture",   KitParam::Float(FloatSlider::new("DoF Aperture", 0.05, 0.0, 1.0)));
        g.push("dof_jitter",     KitParam::Float(FloatSlider::new("DoF Bokeh Jitter", 0.012, 0.0, 0.1)));
        g.push("alpha_scale",    KitParam::Float(FloatSlider::new("Alpha Scale", 1.0, 0.01, 4.0)));
        g.push("emission_scale", KitParam::Float(FloatSlider::new("Emission Scale", 1.0, 0.0, 8.0)));
        g.push("taa",            KitParam::Bool(BoolToggle::new("TAA Jitter", true)));
        g.push("fxaa",           KitParam::Bool(BoolToggle::new("FXAA", true)));
        g.push("motion_blur",    KitParam::Bool(BoolToggle::new("Motion Blur", true)));
        g.push("motion_samples", KitParam::Int(IntSlider::new("Motion Blur Samples", 8, 2, 16)));
        g.push("scanlines",      KitParam::Bool(BoolToggle::new("Scanlines", false)));
        g.push("scanline_int",   KitParam::Float(FloatSlider::new("Scanline Intensity", 0.06, 0.0, 1.0)));
        g
    }

    fn build_postfx_group() -> KitGroup {
        let mut g = KitGroup::new("PostFX");
        g.push("ssr",             KitParam::Bool(BoolToggle::new("SSR (SDF)", true)));
        g.push("god_rays",        KitParam::Bool(BoolToggle::new("God Rays", true)));
        g.push("god_ray_density", KitParam::Float(FloatSlider::new("God Ray Density", 0.4, 0.0, 1.0)));
        g.push("heat_haze",       KitParam::Bool(BoolToggle::new("Heat Haze", false)));
        g.push("heat_strength",   KitParam::Float(FloatSlider::new("Heat Strength", 0.005, 0.0, 0.05)));
        g.push("auto_expose",     KitParam::Bool(BoolToggle::new("Auto Exposure", true)));
        g.push("ae_speed",        KitParam::Float(FloatSlider::new("Auto Exposure Speed", 2.0, 0.1, 10.0)));
        g.push("ae_min",          KitParam::Float(FloatSlider::new("AE Min EV", -3.0, -8.0, 0.0)));
        g.push("ae_max",          KitParam::Float(FloatSlider::new("AE Max EV",  3.0,  0.0, 8.0)));
        g.push("edge_sharpen",    KitParam::Float(FloatSlider::new("Edge Sharpen", 0.3, 0.0, 2.0)));
        g.push("atm_scatter",     KitParam::Bool(BoolToggle::new("Atmospheric Scatter", true)));
        g.push("atm_density",     KitParam::Float(FloatSlider::new("Atmosphere Density", 0.05, 0.0, 0.5)));
        g.push("gi_bleed",        KitParam::Bool(BoolToggle::new("Cross-Material GI Bleed", true)));
        g.push("gi_strength",     KitParam::Float(FloatSlider::new("GI Bleed Strength", 0.2, 0.0, 1.0)));
        g
    }

    fn build_camera_group() -> KitGroup {
        let mut g = KitGroup::new("Camera");
        g.push("fov",        KitParam::Float(FloatSlider::new("Field of View", 65.0, 10.0, 120.0).unit("°")));
        g.push("near",       KitParam::Float(FloatSlider::new("Near Plane", 0.01, 0.001, 1.0).log()));
        g.push("far",        KitParam::Float(FloatSlider::new("Far Plane", 1000.0, 10.0, 50000.0).log()));
        g.push("orbit_dist", KitParam::Float(FloatSlider::new("Orbit Distance", 3.0, 0.1, 100.0)));
        g.push("orbit_speed",KitParam::Float(FloatSlider::new("Orbit Speed", 1.5, 0.01, 10.0)));
        g.push("pan_speed",  KitParam::Float(FloatSlider::new("Pan Speed", 2.0, 0.01, 20.0)));
        g.push("fly_speed",  KitParam::Float(FloatSlider::new("Fly Speed", 5.0, 0.1, 50.0)));
        g.push("spring_k",   KitParam::Float(FloatSlider::new("Spring K (trauma)", 8.0, 0.0, 40.0)));
        g.push("trauma_dec", KitParam::Float(FloatSlider::new("Trauma Decay", 3.0, 0.1, 20.0)));
        g
    }

    fn build_env_group() -> KitGroup {
        let mut g = KitGroup::new("Environment");
        g.push("sky_enabled",  KitParam::Bool(BoolToggle::new("Sky Enabled", true)));
        g.push("sky_type",     KitParam::Enum(EnumPicker::new("Sky Type",
            vec!["Nishita".into(),"Solid".into(),"HDRI".into(),"Gradient".into()], 0)));
        g.push("sky_color",    KitParam::Color(ColorPicker::new("Sky Color", Vec4::new(0.1, 0.2, 0.5, 1.0))));
        g.push("sun_dir",      KitParam::Vec3(Vec3Field::new("Sun Direction", Vec3::new(0.5, 0.8, 0.3), -1.0, 1.0)));
        g.push("sun_color",    KitParam::Color(ColorPicker::new("Sun Color", Vec4::new(1.0, 0.97, 0.85, 1.0))));
        g.push("sun_strength", KitParam::Float(FloatSlider::new("Sun Strength", 5.0, 0.0, 20.0)));
        g.push("fog_enabled",  KitParam::Bool(BoolToggle::new("Volumetric Fog", false)));
        g.push("fog_density",  KitParam::Float(FloatSlider::new("Fog Density", 0.01, 0.0, 0.5)));
        g.push("fog_color",    KitParam::Color(ColorPicker::new("Fog Color", Vec4::new(0.7, 0.75, 0.85, 1.0))));
        g.push("fog_height",   KitParam::Float(FloatSlider::new("Fog Height Falloff", 1.0, 0.01, 10.0)));
        g.push("grid",         KitParam::Bool(BoolToggle::new("Ground Grid", true)));
        g.push("grid_color",   KitParam::Color(ColorPicker::new("Grid Color", Vec4::new(0.3, 0.3, 0.3, 0.5))));
        g
    }

    // ── Lookup helpers ────────────────────────────────────────────────────

    pub fn group(&self, name: &str) -> Option<&KitGroup> {
        self.groups.iter().find(|g| g.name == name)
    }

    pub fn group_mut(&mut self, name: &str) -> Option<&mut KitGroup> {
        self.groups.iter_mut().find(|g| g.name == name)
    }

    pub fn get_float(&self, group: &str, key: &str) -> Option<f32> {
        self.group(group)?.get_float(key)
    }

    pub fn set_float(&mut self, group: &str, key: &str, v: f32) {
        let old = self.get_float(group, key);
        if let Some(g) = self.group_mut(group) {
            g.set_float(key, v);
            self.dirty = true;
            if let Some(old_v) = old {
                self.undo_stack.push(KitEdit {
                    group: group.into(), key: key.into(),
                    old: KitEditValue::Float(old_v),
                    new: KitEditValue::Float(v),
                });
                self.redo_stack.clear();
            }
        }
    }

    pub fn get_bool(&self, group: &str, key: &str) -> Option<bool> {
        self.group(group)?.get_bool(key)
    }

    pub fn toggle_bool(&mut self, group: &str, key: &str) {
        let old = self.get_bool(group, key);
        if let Some(g) = self.group_mut(group) {
            if let Some(KitParam::Bool(b)) = g.get_mut(key) {
                b.toggle();
                self.dirty = true;
                if let Some(ov) = old {
                    self.undo_stack.push(KitEdit {
                        group: group.into(), key: key.into(),
                        old: KitEditValue::Bool(ov),
                        new: KitEditValue::Bool(!ov),
                    });
                    self.redo_stack.clear();
                }
            }
        }
    }

    pub fn get_color(&self, group: &str, key: &str) -> Option<Vec4> {
        self.group(group)?.get_color(key)
    }

    pub fn set_color(&mut self, group: &str, key: &str, v: Vec4) {
        let old = self.get_color(group, key);
        if let Some(g) = self.group_mut(group) {
            if let Some(KitParam::Color(c)) = g.get_mut(key) {
                c.value = v;
                self.dirty = true;
                if let Some(ov) = old {
                    self.undo_stack.push(KitEdit {
                        group: group.into(), key: key.into(),
                        old: KitEditValue::Color(ov),
                        new: KitEditValue::Color(v),
                    });
                    self.redo_stack.clear();
                }
            }
        }
    }

    // ── Undo / redo ───────────────────────────────────────────────────────

    pub fn undo(&mut self) {
        if let Some(edit) = self.undo_stack.pop() {
            // Revert the change
            let rev_val = match &edit.old {
                KitEditValue::Float(v) => { self.group_mut(&edit.group).map(|g| g.set_float(&edit.key, *v)); }
                KitEditValue::Bool(v)  => {
                    if let Some(g) = self.group_mut(&edit.group) {
                        if let Some(KitParam::Bool(b)) = g.get_mut(&edit.key) { b.value = *v; }
                    }
                }
                KitEditValue::Color(v) => {
                    if let Some(g) = self.group_mut(&edit.group) {
                        if let Some(KitParam::Color(c)) = g.get_mut(&edit.key) { c.value = *v; }
                    }
                }
                _ => {}
            };
            let _ = rev_val;
            self.redo_stack.push(edit);
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(edit) = self.redo_stack.pop() {
            match &edit.new {
                KitEditValue::Float(v) => { self.group_mut(&edit.group).map(|g| g.set_float(&edit.key, *v)); }
                KitEditValue::Bool(v) => {
                    if let Some(g) = self.group_mut(&edit.group) {
                        if let Some(KitParam::Bool(b)) = g.get_mut(&edit.key) { b.value = *v; }
                    }
                }
                KitEditValue::Color(v) => {
                    if let Some(g) = self.group_mut(&edit.group) {
                        if let Some(KitParam::Color(c)) = g.get_mut(&edit.key) { c.value = *v; }
                    }
                }
                _ => {}
            };
            self.undo_stack.push(edit);
            self.dirty = true;
        }
    }

    // ── Search ────────────────────────────────────────────────────────────

    pub fn search_results(&self) -> Vec<(&str, &str, &KitParam)> {
        if self.search.is_empty() { return Vec::new(); }
        let query = self.search.to_lowercase();
        let mut results = Vec::new();
        for group in &self.groups {
            for (key, param) in &group.params {
                if param.label().to_lowercase().contains(&query) || key.to_lowercase().contains(&query) {
                    results.push((group.name.as_str(), key.as_str(), param));
                }
            }
        }
        results
    }

    // ── Display ───────────────────────────────────────────────────────────

    pub fn status_line(&self) -> String {
        let total_params: usize = self.groups.iter().map(|g| g.params.len()).sum();
        format!("Kit Panel — {} groups {} params | {} edits in history",
            self.groups.len(), total_params, self.undo_stack.len())
    }
}

impl Default for KitPanel { fn default() -> Self { Self::new() } }

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_groups_build() {
        let panel = KitPanel::new();
        assert!(panel.groups.len() >= 8);
    }

    #[test]
    fn get_set_float() {
        let mut panel = KitPanel::new();
        panel.set_float("Bloom", "intensity", 5.0);
        assert!((panel.get_float("Bloom", "intensity").unwrap() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn undo_float() {
        let mut panel = KitPanel::new();
        panel.set_float("Bloom", "intensity", 5.0);
        panel.undo();
        let v = panel.get_float("Bloom", "intensity").unwrap();
        assert!((v - 2.8).abs() < 1e-5); // default value restored
    }

    #[test]
    fn toggle_bool() {
        let mut panel = KitPanel::new();
        let before = panel.get_bool("Bloom", "enabled").unwrap();
        panel.toggle_bool("Bloom", "enabled");
        let after = panel.get_bool("Bloom", "enabled").unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn slider_normalized() {
        let s = FloatSlider::new("x", 0.5, 0.0, 1.0);
        assert!((s.normalized() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn color_hex_roundtrip() {
        let c = ColorPicker::new("col", Vec4::new(1.0, 0.0, 0.5, 1.0));
        let hex = c.to_hex();
        let back = ColorPicker::from_hex(&hex).unwrap();
        assert!((back.x - 1.0).abs() < 0.01);
    }

    #[test]
    fn search_finds_result() {
        let mut panel = KitPanel::new();
        panel.search = "bloom".into();
        let results = panel.search_results();
        assert!(!results.is_empty());
    }
}
