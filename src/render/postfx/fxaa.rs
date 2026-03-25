//! FXAA 3.11 — Fast Approximate Anti-Aliasing.
//!
//! Single post-processing pass after all other effects. Detects high-contrast
//! edges and applies directional blur along the edge, smoothing glyph edges
//! without blurring interiors.
//!
//! # Quality levels
//!
//! - Low:    5 edge search steps — fast, adequate for most cases
//! - Medium: 8 edge search steps — good balance (default)
//! - High:  12 edge search steps — best quality, slight cost
//!
//! # When to disable
//!
//! - CRT scanline mode (scanlines + FXAA conflict)
//! - Player preference for crisp pixels

// ── Quality level ───────────────────────────────────────────────────────────

/// FXAA quality preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxaaQuality {
    /// 5 edge search steps.
    Low,
    /// 8 edge search steps (default).
    Medium,
    /// 12 edge search steps.
    High,
    /// FXAA disabled.
    Off,
}

impl FxaaQuality {
    /// Number of edge search iterations.
    pub fn search_steps(&self) -> u32 {
        match self {
            Self::Low => 5,
            Self::Medium => 8,
            Self::High => 12,
            Self::Off => 0,
        }
    }

    /// GLSL #define value for the quality preset.
    pub fn define_value(&self) -> u32 {
        match self {
            Self::Low => 10,
            Self::Medium => 20,
            Self::High => 39,
            Self::Off => 0,
        }
    }

    pub fn is_enabled(&self) -> bool { *self != Self::Off }
}

impl Default for FxaaQuality {
    fn default() -> Self { Self::Medium }
}

// ── FXAA parameters ─────────────────────────────────────────────────────────

/// Configuration for the FXAA pass.
#[derive(Debug, Clone)]
pub struct FxaaParams {
    /// Quality level.
    pub quality: FxaaQuality,
    /// Edge detection threshold. Lower = more edges detected.
    /// Range: 0.063 (high quality) to 0.333 (low quality, faster).
    pub edge_threshold: f32,
    /// Minimum edge threshold. Prevents processing dark areas.
    /// Range: 0.0312 (visible limit) to 0.0833 (fast).
    pub edge_threshold_min: f32,
    /// Subpixel anti-aliasing amount. Higher = softer.
    /// Range: 0.0 (off) to 1.0 (full).
    pub subpixel: f32,
}

impl Default for FxaaParams {
    fn default() -> Self {
        Self {
            quality: FxaaQuality::Medium,
            edge_threshold: 0.166,
            edge_threshold_min: 0.0625,
            subpixel: 0.75,
        }
    }
}

impl FxaaParams {
    /// High quality preset.
    pub fn high() -> Self {
        Self {
            quality: FxaaQuality::High,
            edge_threshold: 0.063,
            edge_threshold_min: 0.0312,
            subpixel: 1.0,
        }
    }

    /// Low quality preset (faster).
    pub fn low() -> Self {
        Self {
            quality: FxaaQuality::Low,
            edge_threshold: 0.250,
            edge_threshold_min: 0.0833,
            subpixel: 0.5,
        }
    }

    /// Disabled.
    pub fn off() -> Self {
        Self { quality: FxaaQuality::Off, ..Default::default() }
    }

    pub fn is_enabled(&self) -> bool { self.quality.is_enabled() }

    /// Whether FXAA should be disabled due to conflicting settings.
    pub fn should_disable_for_scanlines(&self, scanlines_enabled: bool) -> bool {
        scanlines_enabled
    }
}

// ── FXAA 3.11 GLSL shader ──────────────────────────────────────────────────

/// FXAA fragment shader implementing Timothy Lottes' FXAA 3.11 algorithm.
///
/// Uniforms:
/// - `u_scene`: sampler2D — input scene texture
/// - `u_texel_size`: vec2 — 1.0 / resolution
/// - `u_edge_threshold`: float
/// - `u_edge_threshold_min`: float
/// - `u_subpixel`: float
/// - `u_search_steps`: int
pub const FXAA_FRAG: &str = r#"
#version 330 core

in  vec2 f_uv;
out vec4 frag_color;

uniform sampler2D u_scene;
uniform vec2      u_texel_size;
uniform float     u_edge_threshold;
uniform float     u_edge_threshold_min;
uniform float     u_subpixel;
uniform int       u_search_steps;

const vec3 LUMA = vec3(0.299, 0.587, 0.114);

float luma(vec3 c) {
    return dot(c, LUMA);
}

void main() {
    // ── Sample center and neighbors ─────────────────────────────────────
    vec3 rgbM  = texture(u_scene, f_uv).rgb;
    float lumM = luma(rgbM);

    float lumNW = luma(texture(u_scene, f_uv + vec2(-1.0, -1.0) * u_texel_size).rgb);
    float lumNE = luma(texture(u_scene, f_uv + vec2( 1.0, -1.0) * u_texel_size).rgb);
    float lumSW = luma(texture(u_scene, f_uv + vec2(-1.0,  1.0) * u_texel_size).rgb);
    float lumSE = luma(texture(u_scene, f_uv + vec2( 1.0,  1.0) * u_texel_size).rgb);
    float lumN  = luma(texture(u_scene, f_uv + vec2( 0.0, -1.0) * u_texel_size).rgb);
    float lumS  = luma(texture(u_scene, f_uv + vec2( 0.0,  1.0) * u_texel_size).rgb);
    float lumW  = luma(texture(u_scene, f_uv + vec2(-1.0,  0.0) * u_texel_size).rgb);
    float lumE  = luma(texture(u_scene, f_uv + vec2( 1.0,  0.0) * u_texel_size).rgb);

    // ── Edge detection ──────────────────────────────────────────────────
    float lumMin = min(lumM, min(min(lumN, lumS), min(lumW, lumE)));
    float lumMax = max(lumM, max(max(lumN, lumS), max(lumW, lumE)));
    float lumRange = lumMax - lumMin;

    // Skip if contrast is too low (not an edge)
    if (lumRange < max(u_edge_threshold_min, lumMax * u_edge_threshold)) {
        frag_color = vec4(rgbM, 1.0);
        return;
    }

    // ── Subpixel aliasing test ──────────────────────────────────────────
    float lumL = (lumN + lumS + lumW + lumE) * 0.25;
    float rangeL = abs(lumL - lumM);
    float blendL = max(0.0, (rangeL / lumRange) - 0.25) * (1.0 / 0.75);
    blendL = min(blendL * blendL, 1.0) * u_subpixel;

    // ── Determine edge direction ────────────────────────────────────────
    float edgeH = abs(lumNW + lumNE - 2.0 * lumN)
                + abs(lumW  + lumE  - 2.0 * lumM) * 2.0
                + abs(lumSW + lumSE - 2.0 * lumS);
    float edgeV = abs(lumNW + lumSW - 2.0 * lumW)
                + abs(lumN  + lumS  - 2.0 * lumM) * 2.0
                + abs(lumNE + lumSE - 2.0 * lumE);

    bool isHorizontal = edgeH >= edgeV;

    // ── Choose edge endpoints ───────────────────────────────────────────
    float stepLength = isHorizontal ? u_texel_size.y : u_texel_size.x;

    float lum1 = isHorizontal ? lumN : lumW;
    float lum2 = isHorizontal ? lumS : lumE;
    float grad1 = lum1 - lumM;
    float grad2 = lum2 - lumM;

    bool steeper1 = abs(grad1) >= abs(grad2);
    float gradScaled = 0.25 * max(abs(grad1), abs(grad2));

    if (!steeper1) stepLength = -stepLength;

    // ── Edge search along perpendicular direction ───────────────────────
    vec2 posN = f_uv;
    vec2 posP = f_uv;

    vec2 dir = isHorizontal ? vec2(u_texel_size.x, 0.0) : vec2(0.0, u_texel_size.y);

    float halfStep = isHorizontal ? u_texel_size.y * 0.5 : u_texel_size.x * 0.5;
    if (isHorizontal) {
        posN.y += stepLength * 0.5;
        posP.y += stepLength * 0.5;
    } else {
        posN.x += stepLength * 0.5;
        posP.x += stepLength * 0.5;
    }

    float lumEnd1 = lumM;
    float lumEnd2 = lumM;
    bool reached1 = false;
    bool reached2 = false;

    for (int i = 0; i < u_search_steps; ++i) {
        if (!reached1) {
            posN -= dir;
            lumEnd1 = luma(texture(u_scene, posN).rgb) - lumM;
            reached1 = abs(lumEnd1) >= gradScaled;
        }
        if (!reached2) {
            posP += dir;
            lumEnd2 = luma(texture(u_scene, posP).rgb) - lumM;
            reached2 = abs(lumEnd2) >= gradScaled;
        }
        if (reached1 && reached2) break;
    }

    // ── Compute final blend ─────────────────────────────────────────────
    float distN = isHorizontal ? (f_uv.x - posN.x) : (f_uv.y - posN.y);
    float distP = isHorizontal ? (posP.x - f_uv.x) : (posP.y - f_uv.y);
    float dist = min(distN, distP);
    float spanLength = distN + distP;

    bool goodSpan = (distN < distP) ? (lumEnd1 < 0.0) : (lumEnd2 < 0.0);
    float pixelOffset = goodSpan ? (0.5 - dist / spanLength) : 0.0;

    float finalBlend = max(pixelOffset, blendL);

    // ── Apply ───────────────────────────────────────────────────────────
    vec2 finalUv = f_uv;
    if (isHorizontal) {
        finalUv.y += finalBlend * stepLength;
    } else {
        finalUv.x += finalBlend * stepLength;
    }

    frag_color = vec4(texture(u_scene, finalUv).rgb, 1.0);
}
"#;

// ── FXAA statistics ─────────────────────────────────────────────────────────

/// Per-frame FXAA statistics.
#[derive(Debug, Clone, Default)]
pub struct FxaaStats {
    pub enabled: bool,
    pub quality: &'static str,
    pub search_steps: u32,
}

impl FxaaStats {
    pub fn from_params(params: &FxaaParams) -> Self {
        Self {
            enabled: params.is_enabled(),
            quality: match params.quality {
                FxaaQuality::Low => "Low",
                FxaaQuality::Medium => "Medium",
                FxaaQuality::High => "High",
                FxaaQuality::Off => "Off",
            },
            search_steps: params.quality.search_steps(),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_medium() {
        let params = FxaaParams::default();
        assert_eq!(params.quality, FxaaQuality::Medium);
        assert!(params.is_enabled());
    }

    #[test]
    fn off_is_disabled() {
        let params = FxaaParams::off();
        assert!(!params.is_enabled());
        assert_eq!(params.quality.search_steps(), 0);
    }

    #[test]
    fn quality_search_steps() {
        assert!(FxaaQuality::Low.search_steps() < FxaaQuality::Medium.search_steps());
        assert!(FxaaQuality::Medium.search_steps() < FxaaQuality::High.search_steps());
    }

    #[test]
    fn scanlines_conflict() {
        let params = FxaaParams::default();
        assert!(params.should_disable_for_scanlines(true));
        assert!(!params.should_disable_for_scanlines(false));
    }

    #[test]
    fn edge_thresholds_in_range() {
        let high = FxaaParams::high();
        let low = FxaaParams::low();
        assert!(high.edge_threshold < low.edge_threshold);
        assert!(high.edge_threshold_min < low.edge_threshold_min);
    }

    #[test]
    fn stats_from_params() {
        let stats = FxaaStats::from_params(&FxaaParams::default());
        assert!(stats.enabled);
        assert_eq!(stats.quality, "Medium");
        assert_eq!(stats.search_steps, 8);
    }
}
