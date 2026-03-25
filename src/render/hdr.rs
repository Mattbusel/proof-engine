//! HDR Render Target — 16-bit float framebuffer with tone mapping and exposure control.
//!
//! Changes the scene framebuffer from RGBA8 to RGBA16F, allowing color values above 1.0.
//! Crits and spell effects can be 10x brighter than normal without clipping, creating
//! dramatic contrast when tone mapped back to SDR for display.
//!
//! # Tone mapping operators
//!
//! - **Reinhard**: Simple, preserves colors well. `c / (1 + c)`
//! - **ACES**: Academy Color Encoding System filmic curve. Industry standard.
//! - **Uncharted2**: John Hable's filmic operator from Uncharted 2.
//!
//! # Exposure control
//!
//! - **Auto-exposure**: Adapts based on scene average luminance (eye adaptation).
//! - **Manual**: Fixed exposure value for specific moments (death, boss intro).

// ── Tone map operator ───────────────────────────────────────────────────────

/// Available tone mapping operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneMapOperator {
    /// No tone mapping (clamp to [0,1]).
    None,
    /// Reinhard: simple, preserves color.
    Reinhard,
    /// Extended Reinhard with white point.
    ReinhardExtended,
    /// ACES filmic curve (industry standard).
    Aces,
    /// Uncharted 2 / John Hable filmic.
    Uncharted2,
    /// Neutral filmic (softer than ACES).
    NeutralFilmic,
}

impl Default for ToneMapOperator {
    fn default() -> Self { Self::Aces }
}

impl ToneMapOperator {
    /// Apply tone mapping to a linear HDR color.
    pub fn apply(&self, color: glam::Vec3, exposure: f32) -> glam::Vec3 {
        let c = color * exposure;
        match self {
            Self::None => clamp01(c),
            Self::Reinhard => reinhard(c),
            Self::ReinhardExtended => reinhard_extended(c, 4.0),
            Self::Aces => aces_filmic(c),
            Self::Uncharted2 => uncharted2(c),
            Self::NeutralFilmic => neutral_filmic(c),
        }
    }

    /// GLSL function name for this operator (used in shader source generation).
    pub fn glsl_func_name(&self) -> &'static str {
        match self {
            Self::None => "tonemap_none",
            Self::Reinhard => "tonemap_reinhard",
            Self::ReinhardExtended => "tonemap_reinhard_ext",
            Self::Aces => "tonemap_aces",
            Self::Uncharted2 => "tonemap_uncharted2",
            Self::NeutralFilmic => "tonemap_neutral",
        }
    }
}

// ── Tone mapping functions ──────────────────────────────────────────────────

fn clamp01(c: glam::Vec3) -> glam::Vec3 {
    glam::Vec3::new(c.x.clamp(0.0, 1.0), c.y.clamp(0.0, 1.0), c.z.clamp(0.0, 1.0))
}

/// Reinhard: `c / (1 + c)`. Simple, good color preservation.
fn reinhard(c: glam::Vec3) -> glam::Vec3 {
    glam::Vec3::new(
        c.x / (1.0 + c.x),
        c.y / (1.0 + c.y),
        c.z / (1.0 + c.z),
    )
}

/// Extended Reinhard with adjustable white point.
fn reinhard_extended(c: glam::Vec3, white_point: f32) -> glam::Vec3 {
    let wp2 = white_point * white_point;
    glam::Vec3::new(
        c.x * (1.0 + c.x / wp2) / (1.0 + c.x),
        c.y * (1.0 + c.y / wp2) / (1.0 + c.y),
        c.z * (1.0 + c.z / wp2) / (1.0 + c.z),
    )
}

/// ACES filmic tone mapping (approximation by Krzysztof Narkowicz).
fn aces_filmic(c: glam::Vec3) -> glam::Vec3 {
    let a = 2.51;
    let b = 0.03;
    let cc = 2.43;
    let d = 0.59;
    let e = 0.14;
    let f = |x: f32| -> f32 {
        ((x * (a * x + b)) / (x * (cc * x + d) + e)).clamp(0.0, 1.0)
    };
    glam::Vec3::new(f(c.x), f(c.y), f(c.z))
}

/// Uncharted 2 filmic curve (John Hable).
fn uncharted2(c: glam::Vec3) -> glam::Vec3 {
    let f = |x: f32| -> f32 {
        let a = 0.15;
        let b = 0.50;
        let cc = 0.10;
        let d = 0.20;
        let e = 0.02;
        let ff = 0.30;
        ((x * (a * x + cc * b) + d * e) / (x * (a * x + b) + d * ff)) - e / ff
    };
    let white_scale = 1.0 / f(11.2);
    glam::Vec3::new(
        f(c.x) * white_scale,
        f(c.y) * white_scale,
        f(c.z) * white_scale,
    )
}

/// Neutral filmic (softer than ACES, less saturation shift).
fn neutral_filmic(c: glam::Vec3) -> glam::Vec3 {
    let f = |x: f32| -> f32 {
        let a = x.max(0.0);
        (a * (a * 6.2 + 0.5)) / (a * (a * 6.2 + 1.7) + 0.06)
    };
    glam::Vec3::new(f(c.x), f(c.y), f(c.z))
}

// ── HDR parameters ──────────────────────────────────────────────────────────

/// HDR rendering parameters.
#[derive(Debug, Clone)]
pub struct HdrParams {
    /// Whether to use RGBA16F framebuffer.
    pub enabled: bool,
    /// Tone mapping operator.
    pub tone_map: ToneMapOperator,
    /// Exposure mode.
    pub exposure_mode: ExposureMode,
    /// Manual exposure value (used when mode = Manual).
    pub manual_exposure: f32,
    /// Auto-exposure adaptation speed (seconds to adapt 63%).
    pub adaptation_speed: f32,
    /// Minimum auto-exposure EV.
    pub min_ev: f32,
    /// Maximum auto-exposure EV.
    pub max_ev: f32,
    /// Exposure compensation (added to auto-exposure result).
    pub compensation: f32,
}

impl Default for HdrParams {
    fn default() -> Self {
        Self {
            enabled: true,
            tone_map: ToneMapOperator::Aces,
            exposure_mode: ExposureMode::Auto,
            manual_exposure: 1.0,
            adaptation_speed: 1.5,
            min_ev: -2.0,
            max_ev: 4.0,
            compensation: 0.0,
        }
    }
}

impl HdrParams {
    pub fn disabled() -> Self {
        Self { enabled: false, ..Default::default() }
    }

    pub fn manual(exposure: f32) -> Self {
        Self {
            exposure_mode: ExposureMode::Manual,
            manual_exposure: exposure,
            ..Default::default()
        }
    }
}

/// Exposure control mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExposureMode {
    /// Automatic eye-adaptation based on scene luminance.
    Auto,
    /// Fixed exposure value.
    Manual,
    /// Auto with locked range (doesn't adapt above/below bounds).
    AutoClamped,
}

// ── Auto-exposure calculator ────────────────────────────────────────────────

/// Tracks scene luminance and computes auto-exposure.
pub struct AutoExposure {
    /// Current adapted luminance.
    current_lum: f32,
    /// Current exposure value.
    pub exposure: f32,
    /// Smoothed average luminance (for display).
    pub avg_luminance: f32,
}

impl AutoExposure {
    pub fn new() -> Self {
        Self {
            current_lum: 0.5,
            exposure: 1.0,
            avg_luminance: 0.5,
        }
    }

    /// Update with the scene's average luminance this frame.
    ///
    /// `avg_lum`: average scene luminance (log-average preferred).
    /// `dt`: frame delta time.
    /// `params`: HDR parameters.
    pub fn update(&mut self, avg_lum: f32, dt: f32, params: &HdrParams) {
        match params.exposure_mode {
            ExposureMode::Manual => {
                self.exposure = params.manual_exposure;
                self.avg_luminance = avg_lum;
            }
            ExposureMode::Auto | ExposureMode::AutoClamped => {
                // Exponential moving average adaptation
                let speed = 1.0 - (-dt / params.adaptation_speed.max(0.01)).exp();
                self.current_lum += (avg_lum.max(0.001) - self.current_lum) * speed;

                // Convert luminance to EV and apply bounds
                let ev = (self.current_lum / 0.18).log2();
                let clamped_ev = ev.clamp(params.min_ev, params.max_ev);

                // Convert EV back to exposure multiplier
                self.exposure = 1.0 / (2.0_f32.powf(clamped_ev) * 1.2);
                self.exposure *= 2.0_f32.powf(params.compensation);

                self.avg_luminance = self.current_lum;
            }
        }
    }

    /// Reset to default state.
    pub fn reset(&mut self) {
        self.current_lum = 0.5;
        self.exposure = 1.0;
    }
}

impl Default for AutoExposure {
    fn default() -> Self { Self::new() }
}

// ── Game-specific exposure presets ──────────────────────────────────────────

/// Exposure presets for specific game moments.
pub struct ExposurePresets;

impl ExposurePresets {
    /// Bright scene (shrine, victory, surface).
    pub fn bright_scene() -> HdrParams {
        HdrParams {
            exposure_mode: ExposureMode::Auto,
            min_ev: 0.0,
            max_ev: 3.0,
            compensation: -0.5,
            ..Default::default()
        }
    }

    /// Dark scene (deep floors, boss rooms).
    pub fn dark_scene() -> HdrParams {
        HdrParams {
            exposure_mode: ExposureMode::Auto,
            min_ev: -2.0,
            max_ev: 1.0,
            compensation: 0.5,
            ..Default::default()
        }
    }

    /// Death sequence: slowly reduce exposure to black.
    pub fn death_sequence(progress: f32) -> HdrParams {
        HdrParams {
            exposure_mode: ExposureMode::Manual,
            manual_exposure: (1.0 - progress * 0.95).max(0.05),
            ..Default::default()
        }
    }

    /// Boss entrance: brief flash then settle.
    pub fn boss_entrance() -> HdrParams {
        HdrParams {
            exposure_mode: ExposureMode::Manual,
            manual_exposure: 2.0,
            ..Default::default()
        }
    }

    /// Victory celebration: bright, warm.
    pub fn victory() -> HdrParams {
        HdrParams {
            exposure_mode: ExposureMode::Manual,
            manual_exposure: 1.3,
            ..Default::default()
        }
    }

    /// Normal gameplay.
    pub fn normal() -> HdrParams {
        HdrParams::default()
    }

    /// Shrine: slightly brighter, serene.
    pub fn shrine() -> HdrParams {
        HdrParams {
            exposure_mode: ExposureMode::Auto,
            compensation: 0.3,
            ..Default::default()
        }
    }
}

// ── GLSL shader sources ─────────────────────────────────────────────────────

/// HDR tone mapping + exposure fragment shader.
/// Applied as the final post-processing pass before display.
pub const HDR_TONEMAP_FRAG: &str = r#"
#version 330 core

in  vec2 f_uv;
out vec4 frag_color;

uniform sampler2D u_hdr_scene;
uniform float     u_exposure;
uniform int       u_tonemap_op;  // 0=none, 1=reinhard, 2=aces, 3=uncharted2

// Reinhard
vec3 tonemap_reinhard(vec3 c) {
    return c / (vec3(1.0) + c);
}

// ACES (Narkowicz approximation)
vec3 tonemap_aces(vec3 c) {
    float a = 2.51;
    float b = 0.03;
    float cc = 2.43;
    float d = 0.59;
    float e = 0.14;
    return clamp((c * (a * c + b)) / (c * (cc * c + d) + e), 0.0, 1.0);
}

// Uncharted 2 (Hable)
vec3 uc2_curve(vec3 x) {
    float A = 0.15; float B = 0.50; float C = 0.10;
    float D = 0.20; float E = 0.02; float F = 0.30;
    return ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - E / F;
}

vec3 tonemap_uncharted2(vec3 c) {
    float W = 11.2;
    return uc2_curve(c) / uc2_curve(vec3(W));
}

void main() {
    vec3 hdr = texture(u_hdr_scene, f_uv).rgb;

    // Apply exposure
    vec3 exposed = hdr * u_exposure;

    // Tone map
    vec3 mapped;
    if (u_tonemap_op == 0)      mapped = clamp(exposed, 0.0, 1.0);
    else if (u_tonemap_op == 1) mapped = tonemap_reinhard(exposed);
    else if (u_tonemap_op == 2) mapped = tonemap_aces(exposed);
    else if (u_tonemap_op == 3) mapped = tonemap_uncharted2(exposed);
    else                        mapped = tonemap_aces(exposed);

    // Gamma correction (linear → sRGB)
    mapped = pow(mapped, vec3(1.0 / 2.2));

    frag_color = vec4(mapped, 1.0);
}
"#;

/// Luminance computation shader for auto-exposure.
/// Computes log-average luminance of the scene.
pub const LUMINANCE_FRAG: &str = r#"
#version 330 core

in  vec2 f_uv;
out vec4 frag_color;

uniform sampler2D u_scene;

void main() {
    vec3 color = texture(u_scene, f_uv).rgb;
    float lum = dot(color, vec3(0.2126, 0.7152, 0.0722));
    // Output log luminance for averaging
    float logLum = log(max(lum, 0.0001));
    frag_color = vec4(logLum, lum, 0.0, 1.0);
}
"#;

// ── HDR statistics ──────────────────────────────────────────────────────────

/// Per-frame HDR rendering statistics.
#[derive(Debug, Clone, Default)]
pub struct HdrStats {
    pub enabled: bool,
    pub tone_map: &'static str,
    pub exposure: f32,
    pub avg_luminance: f32,
    pub exposure_mode: &'static str,
}

impl HdrStats {
    pub fn from_state(params: &HdrParams, auto_exp: &AutoExposure) -> Self {
        Self {
            enabled: params.enabled,
            tone_map: match params.tone_map {
                ToneMapOperator::None => "None",
                ToneMapOperator::Reinhard => "Reinhard",
                ToneMapOperator::ReinhardExtended => "Reinhard Ext",
                ToneMapOperator::Aces => "ACES",
                ToneMapOperator::Uncharted2 => "Uncharted2",
                ToneMapOperator::NeutralFilmic => "Neutral",
            },
            exposure: auto_exp.exposure,
            avg_luminance: auto_exp.avg_luminance,
            exposure_mode: match params.exposure_mode {
                ExposureMode::Auto => "Auto",
                ExposureMode::Manual => "Manual",
                ExposureMode::AutoClamped => "Auto (Clamped)",
            },
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn reinhard_preserves_zero() {
        let result = ToneMapOperator::Reinhard.apply(Vec3::ZERO, 1.0);
        assert!(result.length() < 1e-6);
    }

    #[test]
    fn reinhard_maps_bright_below_one() {
        let result = ToneMapOperator::Reinhard.apply(Vec3::new(10.0, 10.0, 10.0), 1.0);
        assert!(result.x < 1.0);
        assert!(result.x > 0.9);
    }

    #[test]
    fn aces_maps_to_unit_range() {
        let result = ToneMapOperator::Aces.apply(Vec3::new(5.0, 3.0, 1.0), 1.0);
        assert!(result.x >= 0.0 && result.x <= 1.0);
        assert!(result.y >= 0.0 && result.y <= 1.0);
        assert!(result.z >= 0.0 && result.z <= 1.0);
    }

    #[test]
    fn uncharted2_maps_to_unit_range() {
        let result = ToneMapOperator::Uncharted2.apply(Vec3::new(5.0, 3.0, 1.0), 1.0);
        assert!(result.x >= 0.0 && result.x <= 1.0);
        assert!(result.y >= 0.0 && result.y <= 1.0);
    }

    #[test]
    fn exposure_scales_output() {
        let low = ToneMapOperator::Aces.apply(Vec3::ONE, 0.5);
        let high = ToneMapOperator::Aces.apply(Vec3::ONE, 2.0);
        assert!(high.x > low.x);
    }

    #[test]
    fn auto_exposure_adapts() {
        let params = HdrParams::default();
        let mut ae = AutoExposure::new();

        // Bright scene
        for _ in 0..60 {
            ae.update(2.0, 0.016, &params);
        }
        let bright_exp = ae.exposure;

        // Dark scene
        ae.reset();
        for _ in 0..60 {
            ae.update(0.01, 0.016, &params);
        }
        let dark_exp = ae.exposure;

        // Dark scene should have higher exposure (brighten)
        assert!(dark_exp > bright_exp, "dark={dark_exp} should > bright={bright_exp}");
    }

    #[test]
    fn manual_exposure_fixed() {
        let params = HdrParams::manual(2.5);
        let mut ae = AutoExposure::new();
        ae.update(0.5, 0.016, &params);
        assert_eq!(ae.exposure, 2.5);
    }

    #[test]
    fn death_exposure_dims() {
        let start = ExposurePresets::death_sequence(0.0);
        let end = ExposurePresets::death_sequence(1.0);
        assert!(end.manual_exposure < start.manual_exposure);
        assert!(end.manual_exposure > 0.0);
    }

    #[test]
    fn none_tonemap_clamps() {
        let result = ToneMapOperator::None.apply(Vec3::new(2.0, -0.5, 0.5), 1.0);
        assert_eq!(result.x, 1.0);
        assert_eq!(result.y, 0.0);
        assert_eq!(result.z, 0.5);
    }
}
