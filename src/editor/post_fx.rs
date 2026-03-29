
//! Post-processing stack editor — effects chain, parameter panels, preview modes.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Effect descriptors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PostFxKind {
    // Tone / exposure
    Exposure,
    ToneMapping,
    ColorGrade,
    LiftGammaGain,
    HueSaturation,
    ColorBalance,
    Curves,
    // Bloom / glow
    Bloom,
    LensFlare,
    LightShafts,
    // Depth of field
    DepthOfField,
    TiltShift,
    // Motion
    MotionBlur,
    TemporalAA,
    // Ambient
    Ssao,
    Hbao,
    ScreenSpaceReflections,
    // Vignette / film
    Vignette,
    FilmGrain,
    ChromaticAberration,
    LensDistortion,
    Scanlines,
    // Stylistic
    Pixelate,
    Posterize,
    Crosshatch,
    Halftone,
    Outline,
    // Blur
    GaussianBlur,
    RadialBlur,
    DirectionalBlur,
    // Color filters
    Sepia,
    Grayscale,
    Infrared,
    NightVision,
    ThermalVision,
    // Anti-aliasing
    Fxaa,
    Smaa,
    Taa,
    // HDR
    AutoExposure,
    HistogramEq,
    // Water / weather
    Underwater,
    RainDrops,
    Fog,
    HeatHaze,
    // Sharp
    Sharpen,
    UnsharpMask,
    EdgeEnhance,
    // Overlay
    Scanlines2,
    Crt,
    VhsGlitch,
    DigitalGlitch,
}

impl PostFxKind {
    pub fn label(&self) -> &'static str {
        match self {
            PostFxKind::Exposure => "Exposure",
            PostFxKind::ToneMapping => "Tone Mapping",
            PostFxKind::ColorGrade => "Color Grade",
            PostFxKind::LiftGammaGain => "Lift / Gamma / Gain",
            PostFxKind::HueSaturation => "Hue / Saturation",
            PostFxKind::ColorBalance => "Color Balance",
            PostFxKind::Curves => "Curves",
            PostFxKind::Bloom => "Bloom",
            PostFxKind::LensFlare => "Lens Flare",
            PostFxKind::LightShafts => "Light Shafts",
            PostFxKind::DepthOfField => "Depth of Field",
            PostFxKind::TiltShift => "Tilt Shift",
            PostFxKind::MotionBlur => "Motion Blur",
            PostFxKind::TemporalAA => "Temporal AA",
            PostFxKind::Ssao => "SSAO",
            PostFxKind::Hbao => "HBAO",
            PostFxKind::ScreenSpaceReflections => "Screen-Space Reflections",
            PostFxKind::Vignette => "Vignette",
            PostFxKind::FilmGrain => "Film Grain",
            PostFxKind::ChromaticAberration => "Chromatic Aberration",
            PostFxKind::LensDistortion => "Lens Distortion",
            PostFxKind::Scanlines | PostFxKind::Scanlines2 => "Scanlines",
            PostFxKind::Pixelate => "Pixelate",
            PostFxKind::Posterize => "Posterize",
            PostFxKind::Crosshatch => "Crosshatch",
            PostFxKind::Halftone => "Halftone",
            PostFxKind::Outline => "Outline",
            PostFxKind::GaussianBlur => "Gaussian Blur",
            PostFxKind::RadialBlur => "Radial Blur",
            PostFxKind::DirectionalBlur => "Directional Blur",
            PostFxKind::Sepia => "Sepia",
            PostFxKind::Grayscale => "Grayscale",
            PostFxKind::Infrared => "Infrared",
            PostFxKind::NightVision => "Night Vision",
            PostFxKind::ThermalVision => "Thermal Vision",
            PostFxKind::Fxaa => "FXAA",
            PostFxKind::Smaa => "SMAA",
            PostFxKind::Taa => "TAA",
            PostFxKind::AutoExposure => "Auto Exposure",
            PostFxKind::HistogramEq => "Histogram Equalization",
            PostFxKind::Underwater => "Underwater",
            PostFxKind::RainDrops => "Rain Drops",
            PostFxKind::Fog => "Fog",
            PostFxKind::HeatHaze => "Heat Haze",
            PostFxKind::Sharpen => "Sharpen",
            PostFxKind::UnsharpMask => "Unsharp Mask",
            PostFxKind::EdgeEnhance => "Edge Enhance",
            PostFxKind::Crt => "CRT",
            PostFxKind::VhsGlitch => "VHS Glitch",
            PostFxKind::DigitalGlitch => "Digital Glitch",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            PostFxKind::Exposure | PostFxKind::ToneMapping | PostFxKind::ColorGrade
            | PostFxKind::LiftGammaGain | PostFxKind::HueSaturation | PostFxKind::ColorBalance
            | PostFxKind::Curves => "Color",
            PostFxKind::Bloom | PostFxKind::LensFlare | PostFxKind::LightShafts => "Glow",
            PostFxKind::DepthOfField | PostFxKind::TiltShift => "Depth of Field",
            PostFxKind::MotionBlur | PostFxKind::TemporalAA => "Motion",
            PostFxKind::Ssao | PostFxKind::Hbao | PostFxKind::ScreenSpaceReflections => "Ambient",
            PostFxKind::Vignette | PostFxKind::FilmGrain | PostFxKind::ChromaticAberration
            | PostFxKind::LensDistortion => "Film",
            PostFxKind::Pixelate | PostFxKind::Posterize | PostFxKind::Crosshatch
            | PostFxKind::Halftone | PostFxKind::Outline => "Stylistic",
            PostFxKind::GaussianBlur | PostFxKind::RadialBlur | PostFxKind::DirectionalBlur => "Blur",
            PostFxKind::Sepia | PostFxKind::Grayscale | PostFxKind::Infrared
            | PostFxKind::NightVision | PostFxKind::ThermalVision => "Filter",
            PostFxKind::Fxaa | PostFxKind::Smaa | PostFxKind::Taa => "Anti-Aliasing",
            PostFxKind::AutoExposure | PostFxKind::HistogramEq => "HDR",
            PostFxKind::Underwater | PostFxKind::RainDrops | PostFxKind::Fog
            | PostFxKind::HeatHaze => "Weather",
            PostFxKind::Sharpen | PostFxKind::UnsharpMask | PostFxKind::EdgeEnhance => "Sharpen",
            PostFxKind::Scanlines | PostFxKind::Scanlines2 | PostFxKind::Crt
            | PostFxKind::VhsGlitch | PostFxKind::DigitalGlitch => "Retro",
        }
    }

    pub fn default_params(&self) -> Vec<PostFxParam> {
        match self {
            PostFxKind::Exposure => vec![
                PostFxParam::float("exposure", 0.0, -10.0, 10.0),
            ],
            PostFxKind::ToneMapping => vec![
                PostFxParam::choice("mode", 1, &["Linear", "ACES", "Filmic", "Reinhard", "Uncharted2"]),
                PostFxParam::float("white_point", 11.2, 1.0, 20.0),
            ],
            PostFxKind::Bloom => vec![
                PostFxParam::float("threshold", 1.0, 0.0, 10.0),
                PostFxParam::float("intensity", 0.5, 0.0, 10.0),
                PostFxParam::float("scatter", 0.7, 0.0, 1.0),
                PostFxParam::float("clamp", 65472.0, 0.0, 65472.0),
                PostFxParam::color("tint", Vec4::ONE),
            ],
            PostFxKind::DepthOfField => vec![
                PostFxParam::float("focus_distance", 10.0, 0.1, 1000.0),
                PostFxParam::float("focal_length", 50.0, 1.0, 300.0),
                PostFxParam::float("aperture", 5.6, 0.7, 32.0),
                PostFxParam::float("blade_count", 5.0, 3.0, 11.0),
                PostFxParam::float("blade_curvature", 1.0, 0.0, 1.0),
                PostFxParam::toggle("near_blur", true),
            ],
            PostFxKind::Ssao => vec![
                PostFxParam::float("radius", 0.5, 0.01, 5.0),
                PostFxParam::float("intensity", 1.0, 0.0, 4.0),
                PostFxParam::float("power", 1.0, 0.5, 4.0),
                PostFxParam::int("samples", 16, 4, 64),
                PostFxParam::float("bias", 0.025, 0.0, 0.2),
            ],
            PostFxKind::Vignette => vec![
                PostFxParam::float("intensity", 0.4, 0.0, 1.0),
                PostFxParam::float("smoothness", 0.4, 0.01, 1.0),
                PostFxParam::float("rounded", 0.0, 0.0, 1.0),
                PostFxParam::color("color", Vec4::new(0.0, 0.0, 0.0, 1.0)),
                PostFxParam::vec2("center", Vec2::new(0.5, 0.5)),
            ],
            PostFxKind::FilmGrain => vec![
                PostFxParam::float("intensity", 0.1, 0.0, 1.0),
                PostFxParam::float("response", 0.8, 0.0, 1.0),
                PostFxParam::toggle("colored", false),
            ],
            PostFxKind::ChromaticAberration => vec![
                PostFxParam::float("intensity", 0.1, 0.0, 1.0),
                PostFxParam::toggle("fast_mode", true),
            ],
            PostFxKind::MotionBlur => vec![
                PostFxParam::float("shutter_angle", 270.0, 0.0, 360.0),
                PostFxParam::int("sample_count", 8, 4, 32),
                PostFxParam::float("quality", 1.0, 0.25, 2.0),
            ],
            PostFxKind::ScreenSpaceReflections => vec![
                PostFxParam::float("max_march_distance", 100.0, 1.0, 500.0),
                PostFxParam::int("max_iterations", 64, 8, 256),
                PostFxParam::float("thickness", 0.5, 0.01, 10.0),
                PostFxParam::float("fade_distance", 0.5, 0.0, 1.0),
            ],
            PostFxKind::Fog => vec![
                PostFxParam::float("density", 0.05, 0.0, 1.0),
                PostFxParam::float("start_distance", 0.0, 0.0, 1000.0),
                PostFxParam::float("end_distance", 300.0, 0.0, 10000.0),
                PostFxParam::color("color", Vec4::new(0.8, 0.9, 1.0, 1.0)),
                PostFxParam::choice("mode", 0, &["Linear", "Exponential", "Exponential Squared"]),
            ],
            _ => vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Parameter types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum PostFxParamValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4), // used as color
    Choice(usize),
}

#[derive(Debug, Clone)]
pub struct PostFxParam {
    pub name: String,
    pub value: PostFxParamValue,
    pub min_float: f32,
    pub max_float: f32,
    pub choices: Vec<String>,
}

impl PostFxParam {
    pub fn float(name: &str, default: f32, min: f32, max: f32) -> Self {
        Self { name: name.to_string(), value: PostFxParamValue::Float(default), min_float: min, max_float: max, choices: vec![] }
    }
    pub fn int(name: &str, default: i32, min: i32, max: i32) -> Self {
        Self { name: name.to_string(), value: PostFxParamValue::Int(default), min_float: min as f32, max_float: max as f32, choices: vec![] }
    }
    pub fn toggle(name: &str, default: bool) -> Self {
        Self { name: name.to_string(), value: PostFxParamValue::Bool(default), min_float: 0.0, max_float: 1.0, choices: vec![] }
    }
    pub fn color(name: &str, default: Vec4) -> Self {
        Self { name: name.to_string(), value: PostFxParamValue::Vec4(default), min_float: 0.0, max_float: 1.0, choices: vec![] }
    }
    pub fn vec2(name: &str, default: Vec2) -> Self {
        Self { name: name.to_string(), value: PostFxParamValue::Vec2(default), min_float: 0.0, max_float: 1.0, choices: vec![] }
    }
    pub fn choice(name: &str, default: usize, options: &[&str]) -> Self {
        Self { name: name.to_string(), value: PostFxParamValue::Choice(default), min_float: 0.0, max_float: options.len() as f32, choices: options.iter().map(|s| s.to_string()).collect() }
    }

    pub fn as_float(&self) -> f32 {
        match &self.value {
            PostFxParamValue::Float(v) => *v,
            PostFxParamValue::Int(v) => *v as f32,
            _ => 0.0,
        }
    }

    pub fn set_float(&mut self, v: f32) {
        match &mut self.value {
            PostFxParamValue::Float(x) => *x = v.clamp(self.min_float, self.max_float),
            PostFxParamValue::Int(x) => *x = v.round() as i32,
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// PostFx Effect instance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PostFxEffect {
    pub id: u32,
    pub kind: PostFxKind,
    pub enabled: bool,
    pub params: Vec<PostFxParam>,
    pub blend_weight: f32,
    pub priority: i32,
    pub name_override: Option<String>,
}

impl PostFxEffect {
    pub fn new(id: u32, kind: PostFxKind) -> Self {
        Self {
            id,
            params: kind.default_params(),
            kind,
            enabled: true,
            blend_weight: 1.0,
            priority: 0,
            name_override: None,
        }
    }

    pub fn display_name(&self) -> &str {
        self.name_override.as_deref().unwrap_or_else(|| self.kind.label())
    }

    pub fn param(&self, name: &str) -> Option<&PostFxParam> {
        self.params.iter().find(|p| p.name == name)
    }

    pub fn param_mut(&mut self, name: &str) -> Option<&mut PostFxParam> {
        self.params.iter_mut().find(|p| p.name == name)
    }

    pub fn set_float(&mut self, name: &str, value: f32) {
        if let Some(p) = self.param_mut(name) {
            p.set_float(value);
        }
    }

    pub fn get_float(&self, name: &str) -> f32 {
        self.param(name).map(|p| p.as_float()).unwrap_or(0.0)
    }

    pub fn glsl_defines(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("#define POST_FX_{:?}\n", self.kind));
        for p in &self.params {
            match &p.value {
                PostFxParamValue::Float(v) => out.push_str(&format!("#define PARAM_{} {:.6}\n", p.name.to_uppercase(), v)),
                PostFxParamValue::Int(v) => out.push_str(&format!("#define PARAM_{} {}\n", p.name.to_uppercase(), v)),
                PostFxParamValue::Bool(v) => out.push_str(&format!("#define PARAM_{} {}\n", p.name.to_uppercase(), if *v { 1 } else { 0 })),
                _ => {}
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Volume blending (like Unity's Volume system)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PostFxVolume {
    pub name: String,
    pub is_global: bool,
    pub priority: f32,
    pub blend_distance: f32,
    pub weight: f32,
    pub position: Vec3,
    pub radius: f32,
    pub effects: Vec<PostFxEffect>,
    pub enabled: bool,
}

impl PostFxVolume {
    pub fn new_global(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_global: true,
            priority: 0.0,
            blend_distance: 0.0,
            weight: 1.0,
            position: Vec3::ZERO,
            radius: 0.0,
            effects: Vec::new(),
            enabled: true,
        }
    }

    pub fn new_local(name: impl Into<String>, position: Vec3, radius: f32) -> Self {
        Self {
            name: name.into(),
            is_global: false,
            priority: 1.0,
            blend_distance: 2.0,
            weight: 1.0,
            position,
            radius,
            effects: Vec::new(),
            enabled: true,
        }
    }

    pub fn influence_at(&self, point: Vec3) -> f32 {
        if self.is_global { return self.weight; }
        let dist = point.distance(self.position);
        if dist >= self.radius + self.blend_distance { return 0.0; }
        if dist <= self.radius { return self.weight; }
        let t = 1.0 - (dist - self.radius) / self.blend_distance.max(0.001);
        t * self.weight
    }

    pub fn add_effect(&mut self, kind: PostFxKind) -> u32 {
        let id = self.effects.len() as u32 + 1;
        self.effects.push(PostFxEffect::new(id, kind));
        id
    }
}

// ---------------------------------------------------------------------------
// Post-processing stack
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PostFxStack {
    pub volumes: Vec<PostFxVolume>,
    pub camera_position: Vec3,
    pub enabled: bool,
    pub render_scale: f32,
    pub output_width: u32,
    pub output_height: u32,
}

impl PostFxStack {
    pub fn new() -> Self {
        let mut stack = Self {
            volumes: Vec::new(),
            camera_position: Vec3::ZERO,
            enabled: true,
            render_scale: 1.0,
            output_width: 1920,
            output_height: 1080,
        };
        // Default global volume
        let mut global = PostFxVolume::new_global("Global");
        global.add_effect(PostFxKind::ToneMapping);
        global.add_effect(PostFxKind::Bloom);
        global.add_effect(PostFxKind::Vignette);
        global.add_effect(PostFxKind::FilmGrain);
        stack.volumes.push(global);
        stack
    }

    pub fn active_volumes(&self) -> Vec<(&PostFxVolume, f32)> {
        let mut result: Vec<(&PostFxVolume, f32)> = self.volumes.iter()
            .filter(|v| v.enabled)
            .map(|v| (v, v.influence_at(self.camera_position)))
            .filter(|(_, influence)| *influence > 0.0)
            .collect();
        result.sort_by(|a, b| a.0.priority.partial_cmp(&b.0.priority).unwrap_or(std::cmp::Ordering::Equal));
        result
    }

    pub fn compute_blended_param(&self, kind: PostFxKind, param_name: &str) -> f32 {
        let mut blended = 0.0_f32;
        let mut total_weight = 0.0_f32;
        for (vol, influence) in self.active_volumes() {
            if let Some(fx) = vol.effects.iter().find(|e| e.kind == kind && e.enabled) {
                let v = fx.get_float(param_name);
                blended += v * influence;
                total_weight += influence;
            }
        }
        if total_weight > 0.0 { blended / total_weight } else { 0.0 }
    }

    pub fn generate_shader_permutation_key(&self) -> u64 {
        let active = self.active_volumes();
        let mut key = 0u64;
        for (vol, _) in &active {
            for fx in &vol.effects {
                if fx.enabled {
                    key ^= fx.kind as u64 * 2654435761;
                    key = key.rotate_left(7);
                }
            }
        }
        key
    }
}

// ---------------------------------------------------------------------------
// Post-FX editor panel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PostFxPreviewMode {
    Final,
    BeforePost,
    SplitScreen,
    Channel(u8), // R/G/B/A = 0/1/2/3
    Depth,
    Normals,
    Albedo,
    Roughness,
    Ao,
}

#[derive(Debug, Clone)]
pub struct PostFxEditorState {
    pub stack: PostFxStack,
    pub selected_volume: Option<usize>,
    pub selected_effect: Option<u32>,
    pub preview_mode: PostFxPreviewMode,
    pub search_query: String,
    pub show_all_categories: bool,
    pub category_filter: Option<String>,
    pub split_x: f32,
    pub history: Vec<PostFxStack>,
    pub history_pos: usize,
}

impl PostFxEditorState {
    pub fn new() -> Self {
        Self {
            stack: PostFxStack::new(),
            selected_volume: Some(0),
            selected_effect: None,
            preview_mode: PostFxPreviewMode::Final,
            search_query: String::new(),
            show_all_categories: true,
            category_filter: None,
            split_x: 0.5,
            history: Vec::new(),
            history_pos: 0,
        }
    }

    pub fn snapshot(&mut self) {
        self.history.truncate(self.history_pos);
        self.history.push(self.stack.clone());
        self.history_pos = self.history.len();
    }

    pub fn undo(&mut self) {
        if self.history_pos > 1 {
            self.history_pos -= 1;
            self.stack = self.history[self.history_pos - 1].clone();
        }
    }

    pub fn redo(&mut self) {
        if self.history_pos < self.history.len() {
            self.stack = self.history[self.history_pos].clone();
            self.history_pos += 1;
        }
    }

    pub fn selected_volume_mut(&mut self) -> Option<&mut PostFxVolume> {
        self.selected_volume.and_then(|i| self.stack.volumes.get_mut(i))
    }

    pub fn add_effect_to_selected(&mut self, kind: PostFxKind) {
        self.snapshot();
        if let Some(vol) = self.selected_volume_mut() {
            let id = vol.add_effect(kind);
            self.selected_effect = Some(id);
        }
    }

    pub fn remove_selected_effect(&mut self) {
        self.snapshot();
        if let (Some(vi), Some(eid)) = (self.selected_volume, self.selected_effect) {
            if let Some(vol) = self.stack.volumes.get_mut(vi) {
                vol.effects.retain(|e| e.id != eid);
            }
            self.selected_effect = None;
        }
    }

    pub fn move_effect_up(&mut self) {
        if let (Some(vi), Some(eid)) = (self.selected_volume, self.selected_effect) {
            if let Some(vol) = self.stack.volumes.get_mut(vi) {
                if let Some(i) = vol.effects.iter().position(|e| e.id == eid) {
                    if i > 0 { vol.effects.swap(i, i - 1); }
                }
            }
        }
    }

    pub fn move_effect_down(&mut self) {
        if let (Some(vi), Some(eid)) = (self.selected_volume, self.selected_effect) {
            if let Some(vol) = self.stack.volumes.get_mut(vi) {
                if let Some(i) = vol.effects.iter().position(|e| e.id == eid) {
                    if i + 1 < vol.effects.len() { vol.effects.swap(i, i + 1); }
                }
            }
        }
    }

    pub fn set_effect_param(&mut self, param_name: &str, value: f32) {
        self.snapshot();
        if let (Some(vi), Some(eid)) = (self.selected_volume, self.selected_effect) {
            if let Some(vol) = self.stack.volumes.get_mut(vi) {
                if let Some(fx) = vol.effects.iter_mut().find(|e| e.id == eid) {
                    fx.set_float(param_name, value);
                }
            }
        }
    }

    pub fn search_effects(&self, query: &str) -> Vec<PostFxKind> {
        use PostFxKind::*;
        let all = [
            Exposure, ToneMapping, ColorGrade, LiftGammaGain, HueSaturation, ColorBalance, Curves,
            Bloom, LensFlare, LightShafts, DepthOfField, TiltShift, MotionBlur, TemporalAA,
            Ssao, Hbao, ScreenSpaceReflections, Vignette, FilmGrain, ChromaticAberration,
            LensDistortion, Scanlines, Pixelate, Posterize, Crosshatch, Halftone, Outline,
            GaussianBlur, RadialBlur, DirectionalBlur, Sepia, Grayscale, Infrared, NightVision,
            ThermalVision, Fxaa, Smaa, Taa, AutoExposure, HistogramEq, Underwater, RainDrops,
            Fog, HeatHaze, Sharpen, UnsharpMask, EdgeEnhance, Crt, VhsGlitch, DigitalGlitch,
        ];
        let q = query.to_lowercase();
        all.iter().copied().filter(|k| {
            let cat_match = self.category_filter.as_deref().map(|c| k.category() == c).unwrap_or(true);
            let text_match = q.is_empty() || k.label().to_lowercase().contains(&q) || k.category().to_lowercase().contains(&q);
            cat_match && text_match
        }).collect()
    }

    pub fn categories() -> Vec<&'static str> {
        vec!["Color", "Glow", "Depth of Field", "Motion", "Ambient", "Film", "Stylistic",
             "Blur", "Filter", "Anti-Aliasing", "HDR", "Weather", "Sharpen", "Retro"]
    }

    pub fn generate_full_pass_glsl(&self) -> String {
        let mut src = String::from("// AUTO-GENERATED POST-PROCESS PASS\n");
        src.push_str("#version 450\n");
        src.push_str("layout(location=0) in vec2 v_uv;\n");
        src.push_str("layout(location=0) out vec4 out_color;\n");
        src.push_str("layout(binding=0) uniform sampler2D u_hdr;\n");
        src.push_str("layout(binding=1) uniform sampler2D u_depth;\n");
        src.push_str("layout(binding=2) uniform sampler2D u_normals;\n\n");
        // Emit per-volume defines
        for (vol, influence) in self.stack.active_volumes() {
            if influence < 0.001 { continue; }
            for fx in &vol.effects {
                if fx.enabled {
                    src.push_str(&fx.glsl_defines());
                }
            }
        }
        src.push_str("void main() {\n");
        src.push_str("    vec4 color = texture(u_hdr, v_uv);\n");
        src.push_str("    // ... pass chain evaluated in C++ shader pipeline\n");
        src.push_str("    out_color = color;\n");
        src.push_str("}\n");
        src
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postfx_defaults() {
        let fx = PostFxEffect::new(1, PostFxKind::Bloom);
        assert!(fx.get_float("threshold") > 0.0);
        assert!(fx.enabled);
    }

    #[test]
    fn test_volume_influence() {
        let global = PostFxVolume::new_global("G");
        assert!((global.influence_at(Vec3::ZERO) - 1.0).abs() < 1e-6);
        let local = PostFxVolume::new_local("L", Vec3::ZERO, 5.0);
        assert!((local.influence_at(Vec3::ZERO) - 1.0).abs() < 1e-6);
        assert_eq!(local.influence_at(Vec3::new(10.0, 0.0, 0.0)), 0.0);
    }

    #[test]
    fn test_stack_blending() {
        let stack = PostFxStack::new();
        let _v = stack.compute_blended_param(PostFxKind::Bloom, "threshold");
    }

    #[test]
    fn test_editor_undo_redo() {
        let mut ed = PostFxEditorState::new();
        ed.add_effect_to_selected(PostFxKind::Fog);
        let count_before = ed.stack.volumes[0].effects.len();
        ed.undo();
        let count_after = ed.stack.volumes[0].effects.len();
        assert!(count_after <= count_before);
    }

    #[test]
    fn test_search_effects() {
        let ed = PostFxEditorState::new();
        let results = ed.search_effects("blur");
        assert!(!results.is_empty());
    }
}
