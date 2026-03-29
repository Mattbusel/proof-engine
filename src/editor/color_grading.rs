
//! Color grading editor — LUT generation, HDR grading, wheels, color science.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Color spaces
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorSpace {
    Linear,
    Gamma22,
    Gamma18,
    AcesCg,
    AcesAp0,
    Rec709,
    Rec2020,
    DciP3,
    DisplayP3,
    SRgb,
    LogC,
    SLog3,
    RedWideGamut,
}

impl ColorSpace {
    pub fn label(self) -> &'static str {
        match self {
            ColorSpace::Linear => "Linear",
            ColorSpace::Gamma22 => "Gamma 2.2",
            ColorSpace::Gamma18 => "Gamma 1.8",
            ColorSpace::AcesCg => "ACEScg",
            ColorSpace::AcesAp0 => "ACES AP0",
            ColorSpace::Rec709 => "Rec.709",
            ColorSpace::Rec2020 => "Rec.2020",
            ColorSpace::DciP3 => "DCI-P3",
            ColorSpace::DisplayP3 => "Display P3",
            ColorSpace::SRgb => "sRGB",
            ColorSpace::LogC => "Log C",
            ColorSpace::SLog3 => "S-Log3",
            ColorSpace::RedWideGamut => "REDWideGamutRGB",
        }
    }

    pub fn is_hdr(self) -> bool {
        matches!(self, ColorSpace::AcesCg | ColorSpace::AcesAp0 | ColorSpace::Rec2020 | ColorSpace::LogC | ColorSpace::SLog3)
    }

    pub fn to_linear(self, v: f32) -> f32 {
        match self {
            ColorSpace::Linear => v,
            ColorSpace::Gamma22 | ColorSpace::SRgb => v.powf(2.2),
            ColorSpace::Gamma18 => v.powf(1.8),
            _ => v,
        }
    }

    pub fn from_linear(self, v: f32) -> f32 {
        match self {
            ColorSpace::Linear => v,
            ColorSpace::Gamma22 | ColorSpace::SRgb => v.max(0.0).powf(1.0 / 2.2),
            ColorSpace::Gamma18 => v.max(0.0).powf(1.0 / 1.8),
            _ => v,
        }
    }
}

// ---------------------------------------------------------------------------
// Tone mapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToneMappingMode {
    None,
    Aces,
    FilmicAces,
    Reinhard,
    ReinhardExtended,
    Uncharted2,
    Hable,
    CustomCurve,
    AgX,
    Tony,
}

impl ToneMappingMode {
    pub fn label(self) -> &'static str {
        match self {
            ToneMappingMode::None => "None",
            ToneMappingMode::Aces => "ACES",
            ToneMappingMode::FilmicAces => "Filmic ACES",
            ToneMappingMode::Reinhard => "Reinhard",
            ToneMappingMode::ReinhardExtended => "Reinhard Extended",
            ToneMappingMode::Uncharted2 => "Uncharted 2",
            ToneMappingMode::Hable => "Hable",
            ToneMappingMode::CustomCurve => "Custom Curve",
            ToneMappingMode::AgX => "AgX",
            ToneMappingMode::Tony => "Tony",
        }
    }

    pub fn apply(self, x: f32) -> f32 {
        match self {
            ToneMappingMode::None => x,
            ToneMappingMode::Reinhard => x / (1.0 + x),
            ToneMappingMode::ReinhardExtended => {
                let white = 4.0_f32;
                x * (1.0 + x / (white * white)) / (1.0 + x)
            }
            ToneMappingMode::Aces => {
                // ACES fitted approximation
                let a = 2.51_f32;
                let b = 0.03_f32;
                let c = 2.43_f32;
                let d = 0.59_f32;
                let e = 0.14_f32;
                ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
            }
            ToneMappingMode::Uncharted2 | ToneMappingMode::Hable => {
                let a = 0.15_f32;
                let b = 0.50_f32;
                let c = 0.10_f32;
                let d = 0.20_f32;
                let e_val = 0.02_f32;
                let f = 0.30_f32;
                let w = 11.2_f32;
                let tone = |v: f32| ((v * (a * v + c * b) + d * e_val) / (v * (a * v + b) + d * f)) - e_val / f;
                let curr = tone(x * 2.0);
                let white_scale = 1.0 / tone(w);
                (curr * white_scale).clamp(0.0, 1.0)
            }
            _ => x / (1.0 + x),
        }
    }

    pub fn apply_vec3(self, color: Vec3) -> Vec3 {
        Vec3::new(self.apply(color.x), self.apply(color.y), self.apply(color.z))
    }
}

// ---------------------------------------------------------------------------
// Color wheel / lift-gamma-gain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct ColorWheelValue {
    pub hue_offset: f32,     // degrees
    pub saturation: f32,     // 0..2
    pub lightness: f32,      // -1..1
    pub tint: Vec3,
}

impl Default for ColorWheelValue {
    fn default() -> Self {
        Self { hue_offset: 0.0, saturation: 1.0, lightness: 0.0, tint: Vec3::ZERO }
    }
}

impl ColorWheelValue {
    pub fn apply(&self, color: Vec3) -> Vec3 {
        // Apply lightness
        let c = color + Vec3::splat(self.lightness);
        // Apply saturation
        let lum = c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let c = Vec3::splat(lum).lerp(c, self.saturation);
        // Apply hue rotation via RGB rotation matrix (approximate)
        let h = self.hue_offset * std::f32::consts::PI / 180.0;
        let cos_h = h.cos();
        let sin_h = h.sin();
        let u = Vec3::new(0.213, 0.715, 0.072);
        let w = Vec3::new(0.143, -0.140, -0.283); // cross product like
        let c = Vec3::new(
            c.dot(u + Vec3::new(cos_h * (1.0 - u.x), -sin_h * w.z, sin_h * w.y)),
            c.dot(u + Vec3::new(sin_h * w.z, cos_h * (1.0 - u.y), -sin_h * w.x)),
            c.dot(u + Vec3::new(-sin_h * w.y, sin_h * w.x, cos_h * (1.0 - u.z))),
        );
        // Add tint
        c + self.tint
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LiftGammaGain {
    pub lift: Vec4,   // xyz = RGB, w = master
    pub gamma: Vec4,
    pub gain: Vec4,
}

impl Default for LiftGammaGain {
    fn default() -> Self {
        Self {
            lift: Vec4::new(0.0, 0.0, 0.0, 0.0),
            gamma: Vec4::new(1.0, 1.0, 1.0, 1.0),
            gain: Vec4::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

impl LiftGammaGain {
    pub fn apply(&self, color: Vec3) -> Vec3 {
        let lift = Vec3::new(self.lift.x + self.lift.w, self.lift.y + self.lift.w, self.lift.z + self.lift.w);
        let gamma = Vec3::new(self.gamma.x * self.gamma.w, self.gamma.y * self.gamma.w, self.gamma.z * self.gamma.w);
        let gain = Vec3::new(self.gain.x * self.gain.w, self.gain.y * self.gain.w, self.gain.z * self.gain.w);
        let c = color * gain + lift;
        Vec3::new(
            c.x.max(0.0).powf(1.0 / gamma.x.max(0.001)),
            c.y.max(0.0).powf(1.0 / gamma.y.max(0.001)),
            c.z.max(0.0).powf(1.0 / gamma.z.max(0.001)),
        )
    }
}

// ---------------------------------------------------------------------------
// Curve per-channel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ColorCurveChannel {
    pub points: Vec<Vec2>,
    pub enabled: bool,
}

impl ColorCurveChannel {
    pub fn identity() -> Self {
        Self {
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)],
            enabled: true,
        }
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        if !self.enabled { return t; }
        let t = t.clamp(0.0, 1.0);
        if self.points.len() < 2 { return t; }
        let i = self.points.partition_point(|p| p.x <= t);
        if i == 0 { return self.points[0].y; }
        if i >= self.points.len() { return self.points[self.points.len()-1].y; }
        let a = self.points[i-1];
        let b = self.points[i];
        let u = (t - a.x) / (b.x - a.x).max(1e-6);
        // Smooth cubic interpolation
        let u = u * u * (3.0 - 2.0 * u);
        a.y + (b.y - a.y) * u
    }

    pub fn add_point(&mut self, t: f32, v: f32) {
        let i = self.points.partition_point(|p| p.x < t);
        self.points.insert(i, Vec2::new(t, v));
    }

    pub fn remove_point(&mut self, idx: usize) {
        if self.points.len() > 2 {
            self.points.remove(idx);
        }
    }

    pub fn reset(&mut self) {
        self.points = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)];
    }
}

#[derive(Debug, Clone)]
pub struct ColorCurves {
    pub master: ColorCurveChannel,
    pub red: ColorCurveChannel,
    pub green: ColorCurveChannel,
    pub blue: ColorCurveChannel,
    pub hue_vs_hue: ColorCurveChannel,
    pub hue_vs_sat: ColorCurveChannel,
    pub hue_vs_lum: ColorCurveChannel,
    pub lum_vs_sat: ColorCurveChannel,
    pub sat_vs_sat: ColorCurveChannel,
}

impl Default for ColorCurves {
    fn default() -> Self {
        Self {
            master: ColorCurveChannel::identity(),
            red: ColorCurveChannel::identity(),
            green: ColorCurveChannel::identity(),
            blue: ColorCurveChannel::identity(),
            hue_vs_hue: ColorCurveChannel::identity(),
            hue_vs_sat: ColorCurveChannel::identity(),
            hue_vs_lum: ColorCurveChannel::identity(),
            lum_vs_sat: ColorCurveChannel::identity(),
            sat_vs_sat: ColorCurveChannel::identity(),
        }
    }
}

impl ColorCurves {
    pub fn apply(&self, color: Vec3) -> Vec3 {
        Vec3::new(
            self.red.evaluate(self.master.evaluate(color.x)),
            self.green.evaluate(self.master.evaluate(color.y)),
            self.blue.evaluate(self.master.evaluate(color.z)),
        )
    }
}

// ---------------------------------------------------------------------------
// Shadow / midtone / highlight controls
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub struct ShadowsMidtonesHighlights {
    pub shadows: Vec4,     // rgb offset, w = weight
    pub midtones: Vec4,
    pub highlights: Vec4,
    pub shadows_start: f32,
    pub shadows_end: f32,
    pub highlights_start: f32,
    pub highlights_end: f32,
}

impl ShadowsMidtonesHighlights {
    pub fn new() -> Self {
        Self {
            shadows: Vec4::new(0.0, 0.0, 0.0, 1.0),
            midtones: Vec4::new(0.0, 0.0, 0.0, 1.0),
            highlights: Vec4::new(0.0, 0.0, 0.0, 1.0),
            shadows_start: 0.0,
            shadows_end: 0.3,
            highlights_start: 0.55,
            highlights_end: 1.0,
        }
    }

    pub fn apply(&self, color: Vec3) -> Vec3 {
        let lum = color.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let shadow_w = (1.0 - (lum - self.shadows_start) / (self.shadows_end - self.shadows_start).max(1e-6)).clamp(0.0, 1.0);
        let highlight_w = ((lum - self.highlights_start) / (self.highlights_end - self.highlights_start).max(1e-6)).clamp(0.0, 1.0);
        let midtone_w = (1.0 - shadow_w - highlight_w).clamp(0.0, 1.0);
        let s = Vec3::new(self.shadows.x, self.shadows.y, self.shadows.z) * self.shadows.w;
        let m = Vec3::new(self.midtones.x, self.midtones.y, self.midtones.z) * self.midtones.w;
        let h = Vec3::new(self.highlights.x, self.highlights.y, self.highlights.z) * self.highlights.w;
        color + s * shadow_w + m * midtone_w + h * highlight_w
    }
}

// ---------------------------------------------------------------------------
// 3D LUT
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LutSize {
    Lut16,
    Lut32,
    Lut48,
    Lut64,
}

impl LutSize {
    pub fn dim(self) -> usize {
        match self {
            LutSize::Lut16 => 16,
            LutSize::Lut32 => 32,
            LutSize::Lut48 => 48,
            LutSize::Lut64 => 64,
        }
    }

    pub fn byte_size(self) -> usize {
        let d = self.dim();
        d * d * d * 4 * 4 // RGBA f32
    }
}

#[derive(Debug, Clone)]
pub struct Lut3D {
    pub size: LutSize,
    pub data: Vec<Vec3>, // size^3 entries
    pub name: String,
    pub source_space: ColorSpace,
    pub target_space: ColorSpace,
}

impl Lut3D {
    pub fn identity(size: LutSize) -> Self {
        let dim = size.dim();
        let n = dim * dim * dim;
        let mut data = Vec::with_capacity(n);
        for b in 0..dim {
            for g in 0..dim {
                for r in 0..dim {
                    data.push(Vec3::new(
                        r as f32 / (dim - 1) as f32,
                        g as f32 / (dim - 1) as f32,
                        b as f32 / (dim - 1) as f32,
                    ));
                }
            }
        }
        Self {
            size,
            data,
            name: "Identity".into(),
            source_space: ColorSpace::Linear,
            target_space: ColorSpace::SRgb,
        }
    }

    pub fn apply(&self, color: Vec3) -> Vec3 {
        let dim = self.size.dim();
        let c = color.clamp(Vec3::ZERO, Vec3::ONE);
        let sc = c * (dim - 1) as f32;
        let x0 = sc.x.floor() as usize;
        let y0 = sc.y.floor() as usize;
        let z0 = sc.z.floor() as usize;
        let x1 = (x0 + 1).min(dim - 1);
        let y1 = (y0 + 1).min(dim - 1);
        let z1 = (z0 + 1).min(dim - 1);
        let fx = sc.x.fract();
        let fy = sc.y.fract();
        let fz = sc.z.fract();
        let idx = |r: usize, g: usize, b: usize| b * dim * dim + g * dim + r;
        // Trilinear interpolation
        let c000 = self.data[idx(x0, y0, z0)];
        let c100 = self.data[idx(x1, y0, z0)];
        let c010 = self.data[idx(x0, y1, z0)];
        let c110 = self.data[idx(x1, y1, z0)];
        let c001 = self.data[idx(x0, y0, z1)];
        let c101 = self.data[idx(x1, y0, z1)];
        let c011 = self.data[idx(x0, y1, z1)];
        let c111 = self.data[idx(x1, y1, z1)];
        let c00 = c000.lerp(c100, fx);
        let c01 = c010.lerp(c110, fx);
        let c10 = c001.lerp(c101, fx);
        let c11 = c011.lerp(c111, fx);
        let c0 = c00.lerp(c01, fy);
        let c1 = c10.lerp(c11, fy);
        c0.lerp(c1, fz)
    }

    pub fn bake_from_grade(&mut self, grade: &ColorGrade) {
        let dim = self.size.dim();
        for b in 0..dim {
            for g in 0..dim {
                for r in 0..dim {
                    let input = Vec3::new(
                        r as f32 / (dim - 1) as f32,
                        g as f32 / (dim - 1) as f32,
                        b as f32 / (dim - 1) as f32,
                    );
                    let output = grade.apply(input);
                    self.data[b * dim * dim + g * dim + r] = output;
                }
            }
        }
    }

    pub fn export_cube_format(&self) -> String {
        let dim = self.size.dim();
        let mut out = String::new();
        out.push_str(&format!("LUT_3D_SIZE {}\n\n", dim));
        for entry in &self.data {
            out.push_str(&format!("{:.6} {:.6} {:.6}\n", entry.x, entry.y, entry.z));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Full color grade pipeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ColorGrade {
    pub exposure: f32,
    pub contrast: f32,
    pub brightness: f32,
    pub saturation: f32,
    pub hue_shift: f32,
    pub temperature: f32,   // Kelvin offset
    pub tint: f32,
    pub tone_mapping: ToneMappingMode,
    pub lift_gamma_gain: LiftGammaGain,
    pub shadows_midtones_highlights: ShadowsMidtonesHighlights,
    pub curves: ColorCurves,
    pub color_filter: Vec3,
    pub channel_mixer_r: Vec3,
    pub channel_mixer_g: Vec3,
    pub channel_mixer_b: Vec3,
    pub color_wheels: [ColorWheelValue; 3], // shadows/midtones/highlights
    pub lut_contribution: f32,
    pub output_colorspace: ColorSpace,
    pub post_exposure: f32,
}

impl Default for ColorGrade {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 0.0,
            brightness: 0.0,
            saturation: 1.0,
            hue_shift: 0.0,
            temperature: 0.0,
            tint: 0.0,
            tone_mapping: ToneMappingMode::Aces,
            lift_gamma_gain: LiftGammaGain::default(),
            shadows_midtones_highlights: ShadowsMidtonesHighlights::new(),
            curves: ColorCurves::default(),
            color_filter: Vec3::ONE,
            channel_mixer_r: Vec3::new(1.0, 0.0, 0.0),
            channel_mixer_g: Vec3::new(0.0, 1.0, 0.0),
            channel_mixer_b: Vec3::new(0.0, 0.0, 1.0),
            color_wheels: [ColorWheelValue::default(); 3],
            lut_contribution: 1.0,
            output_colorspace: ColorSpace::SRgb,
            post_exposure: 0.0,
        }
    }
}

impl ColorGrade {
    pub fn apply(&self, input: Vec3) -> Vec3 {
        // 1. Exposure
        let exposure_mult = 2.0_f32.powf(self.exposure);
        let c = input * exposure_mult;
        // 2. White balance (approximate temperature/tint)
        let temp_k = self.temperature * 100.0;
        let wb_r = 1.0 + temp_k * 0.0002;
        let wb_b = 1.0 - temp_k * 0.0002;
        let wb_g = 1.0 + self.tint * 0.001;
        let c = Vec3::new(c.x * wb_r, c.y * wb_g, c.z * wb_b);
        // 3. Contrast
        let c = (c - Vec3::splat(0.5)) * (1.0 + self.contrast * 0.01) + Vec3::splat(0.5);
        let c = c + Vec3::splat(self.brightness * 0.01);
        // 4. Saturation
        let lum = c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let c = Vec3::splat(lum).lerp(c, self.saturation);
        // 5. Color filter
        let c = c * self.color_filter;
        // 6. Channel mixer
        let r = c.dot(self.channel_mixer_r);
        let g = c.dot(self.channel_mixer_g);
        let b = c.dot(self.channel_mixer_b);
        let c = Vec3::new(r, g, b);
        // 7. Lift / Gamma / Gain
        let c = self.lift_gamma_gain.apply(c);
        // 8. Shadows/Midtones/Highlights
        let c = self.shadows_midtones_highlights.apply(c);
        // 9. Curves
        let c = self.curves.apply(c);
        // 10. Tone mapping
        let c = self.tone_mapping.apply_vec3(c);
        // 11. Post exposure
        let c = c * 2.0_f32.powf(self.post_exposure);
        // 12. Gamma
        Vec3::new(
            self.output_colorspace.from_linear(c.x),
            self.output_colorspace.from_linear(c.y),
            self.output_colorspace.from_linear(c.z),
        ).max(Vec3::ZERO)
    }
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ColorGradePreset {
    pub name: String,
    pub category: String,
    pub description: String,
    pub grade: ColorGrade,
    pub thumbnail: Option<Vec<Vec3>>,
}

impl ColorGradePreset {
    pub fn cinematic() -> Self {
        let mut grade = ColorGrade::default();
        grade.contrast = 15.0;
        grade.saturation = 0.85;
        grade.temperature = -10.0;
        grade.lift_gamma_gain.lift = Vec4::new(-0.02, -0.02, 0.02, 0.0);
        grade.lift_gamma_gain.gain = Vec4::new(1.1, 1.05, 0.95, 1.0);
        grade.tone_mapping = ToneMappingMode::FilmicAces;
        Self {
            name: "Cinematic".into(),
            category: "Film".into(),
            description: "Filmic contrast with desaturated cool tones".into(),
            grade,
            thumbnail: None,
        }
    }

    pub fn vintage() -> Self {
        let mut grade = ColorGrade::default();
        grade.saturation = 0.7;
        grade.temperature = 25.0;
        grade.tint = 5.0;
        grade.lift_gamma_gain.lift = Vec4::new(0.03, 0.02, 0.0, 0.0);
        grade.lift_gamma_gain.gain = Vec4::new(1.0, 0.95, 0.85, 1.0);
        grade.tone_mapping = ToneMappingMode::Hable;
        Self {
            name: "Vintage".into(),
            category: "Stylistic".into(),
            description: "Warm desaturated vintage look".into(),
            grade,
            thumbnail: None,
        }
    }

    pub fn horror() -> Self {
        let mut grade = ColorGrade::default();
        grade.saturation = 0.3;
        grade.contrast = 25.0;
        grade.temperature = -30.0;
        grade.lift_gamma_gain.lift = Vec4::new(0.0, -0.03, 0.0, -0.05);
        grade.tone_mapping = ToneMappingMode::Reinhard;
        Self {
            name: "Horror".into(),
            category: "Stylistic".into(),
            description: "Desaturated cold high-contrast horror look".into(),
            grade,
            thumbnail: None,
        }
    }

    pub fn neon_noir() -> Self {
        let mut grade = ColorGrade::default();
        grade.saturation = 1.4;
        grade.contrast = 20.0;
        grade.temperature = -15.0;
        grade.lift_gamma_gain.lift = Vec4::new(-0.05, 0.0, 0.1, 0.0);
        grade.lift_gamma_gain.gain = Vec4::new(0.9, 0.95, 1.15, 1.0);
        grade.tone_mapping = ToneMappingMode::ReinhardExtended;
        Self {
            name: "Neon Noir".into(),
            category: "Stylistic".into(),
            description: "High-saturation cyberpunk neon look".into(),
            grade,
            thumbnail: None,
        }
    }

    pub fn natural() -> Self {
        Self {
            name: "Natural".into(),
            category: "Neutral".into(),
            description: "Neutral natural grading".into(),
            grade: ColorGrade::default(),
            thumbnail: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorGradingTab {
    Grading,
    ToneMapping,
    Curves,
    Wheels,
    Lut,
    Presets,
    Scopes,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScopeMode {
    Waveform,
    Parade,
    Histogram,
    Vectorscope,
}

#[derive(Debug, Clone)]
pub struct ColorGradingEditor {
    pub grade: ColorGrade,
    pub active_tab: ColorGradingTab,
    pub active_lut: Option<Lut3D>,
    pub lut_size: LutSize,
    pub presets: Vec<ColorGradePreset>,
    pub history: Vec<ColorGrade>,
    pub history_pos: usize,
    pub scope_mode: ScopeMode,
    pub show_clipping: bool,
    pub show_scopes: bool,
    pub preview_split: bool,
    pub split_position: f32,
    pub input_space: ColorSpace,
    pub working_space: ColorSpace,
    pub output_space: ColorSpace,
    pub preset_search: String,
}

impl ColorGradingEditor {
    pub fn new() -> Self {
        Self {
            grade: ColorGrade::default(),
            active_tab: ColorGradingTab::Grading,
            active_lut: None,
            lut_size: LutSize::Lut32,
            presets: vec![
                ColorGradePreset::natural(),
                ColorGradePreset::cinematic(),
                ColorGradePreset::vintage(),
                ColorGradePreset::horror(),
                ColorGradePreset::neon_noir(),
            ],
            history: Vec::new(),
            history_pos: 0,
            scope_mode: ScopeMode::Waveform,
            show_clipping: false,
            show_scopes: true,
            preview_split: false,
            split_position: 0.5,
            input_space: ColorSpace::Linear,
            working_space: ColorSpace::AcesCg,
            output_space: ColorSpace::SRgb,
            preset_search: String::new(),
        }
    }

    pub fn snapshot(&mut self) {
        self.history.truncate(self.history_pos);
        self.history.push(self.grade.clone());
        self.history_pos = self.history.len();
    }

    pub fn undo(&mut self) {
        if self.history_pos > 1 {
            self.history_pos -= 1;
            self.grade = self.history[self.history_pos - 1].clone();
        }
    }

    pub fn redo(&mut self) {
        if self.history_pos < self.history.len() {
            self.grade = self.history[self.history_pos].clone();
            self.history_pos += 1;
        }
    }

    pub fn apply_preset(&mut self, preset: &ColorGradePreset) {
        self.snapshot();
        self.grade = preset.grade.clone();
    }

    pub fn generate_lut(&mut self) {
        let mut lut = Lut3D::identity(self.lut_size);
        lut.bake_from_grade(&self.grade);
        lut.name = "Baked Grade".into();
        self.active_lut = Some(lut);
    }

    pub fn export_lut_cube(&self) -> Option<String> {
        self.active_lut.as_ref().map(|l| l.export_cube_format())
    }

    pub fn search_presets(&self, query: &str) -> Vec<&ColorGradePreset> {
        let q = query.to_lowercase();
        self.presets.iter().filter(|p| {
            p.name.to_lowercase().contains(&q) ||
            p.category.to_lowercase().contains(&q) ||
            p.description.to_lowercase().contains(&q)
        }).collect()
    }

    pub fn grade_pixel(&self, input: Vec3) -> Vec3 {
        let graded = self.grade.apply(input);
        if let Some(lut) = &self.active_lut {
            let from_lut = lut.apply(graded);
            graded.lerp(from_lut, self.grade.lut_contribution)
        } else {
            graded
        }
    }

    pub fn reset_to_identity(&mut self) {
        self.snapshot();
        self.grade = ColorGrade::default();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tone_mapping() {
        let tm = ToneMappingMode::Aces;
        let v = tm.apply(5.0);
        assert!(v >= 0.0 && v <= 1.0);
    }

    #[test]
    fn test_color_grade() {
        let grade = ColorGrade::default();
        let c = grade.apply(Vec3::new(0.5, 0.5, 0.5));
        assert!(c.x >= 0.0 && c.x <= 1.0);
    }

    #[test]
    fn test_lut_identity() {
        let lut = Lut3D::identity(LutSize::Lut16);
        let c = lut.apply(Vec3::new(0.5, 0.5, 0.5));
        assert!((c.x - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_lut_bake() {
        let mut lut = Lut3D::identity(LutSize::Lut16);
        let grade = ColorGrade::default();
        lut.bake_from_grade(&grade);
        assert!(!lut.data.is_empty());
    }

    #[test]
    fn test_editor() {
        let mut ed = ColorGradingEditor::new();
        ed.grade.exposure = 1.0;
        ed.snapshot();
        ed.undo();
        assert!((ed.grade.exposure).abs() < 1e-5);
        ed.generate_lut();
        assert!(ed.active_lut.is_some());
    }
}
