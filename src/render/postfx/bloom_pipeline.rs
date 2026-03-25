//! Proper Bloom Pipeline — multi-level downsample/upsample bloom with brightness threshold.
//!
//! Only intentionally bright things glow. UI text (emission 0.0) never blooms.
//! Damage numbers (emission 1.5), crits (emission 2.5), spells (emission 2.0+),
//! boss auras (emission 1.0) all bloom at appropriate intensities.
//!
//! # Pipeline
//!
//! ```text
//! scene_emission_tex (from glyph fragment shader, attachment 1)
//!   └─ Threshold extract (luminance > threshold) ─▶ bright_tex (full res)
//!       ├─ Downsample ½  ─▶ mip[0] ── H blur ── V blur ─▶ blurred[0]
//!       ├─ Downsample ¼  ─▶ mip[1] ── H blur ── V blur ─▶ blurred[1]
//!       ├─ Downsample ⅛  ─▶ mip[2] ── H blur ── V blur ─▶ blurred[2]
//!       ├─ Downsample 1/16 ─▶ mip[3] ── H blur ── V blur ─▶ blurred[3]
//!       └─ Downsample 1/32 ─▶ mip[4] ── H blur ── V blur ─▶ blurred[4]
//!   Upsample chain: blurred[4] + blurred[3] + ... + blurred[0] ─▶ bloom_result
//!   Composite: scene_color + bloom_result × intensity ─▶ output
//! ```

use super::bloom::{BloomParams, BloomPyramidLevel, compute_pyramid, normalise_pyramid_weights};

// ── Emission presets ────────────────────────────────────────────────────────

/// Standard emission values for different game element categories.
/// These control which elements participate in bloom and how strongly.
pub struct EmissionPresets;

impl EmissionPresets {
    // ── UI / Text ───────────────────────────────────────────────────────
    /// UI text, labels, panels — never blooms.
    pub const UI_TEXT: f32 = 0.0;
    /// Menu highlight — very subtle.
    pub const UI_HIGHLIGHT: f32 = 0.1;

    // ── Environment ─────────────────────────────────────────────────────
    /// Standard world glyphs — no bloom.
    pub const WORLD_GLYPH: f32 = 0.0;
    /// Chaos field background — subtle ambient glow.
    pub const CHAOS_FIELD: f32 = 0.35;
    /// Chaos field intense regions.
    pub const CHAOS_FIELD_INTENSE: f32 = 0.5;
    /// Torch/light source.
    pub const LIGHT_SOURCE: f32 = 0.8;

    // ── Combat ──────────────────────────────────────────────────────────
    /// Normal damage numbers.
    pub const DAMAGE_NUMBER: f32 = 1.5;
    /// Critical hit numbers — strong glow.
    pub const CRIT_NUMBER: f32 = 2.5;
    /// Healing numbers.
    pub const HEAL_NUMBER: f32 = 1.2;
    /// Miss/block text.
    pub const MISS_TEXT: f32 = 0.3;

    // ── Spells & Effects ────────────────────────────────────────────────
    /// Spell cast effect.
    pub const SPELL_EFFECT: f32 = 2.0;
    /// Spell impact flash.
    pub const SPELL_IMPACT: f32 = 3.0;
    /// Buff/debuff aura.
    pub const BUFF_AURA: f32 = 1.0;

    // ── Bosses ──────────────────────────────────────────────────────────
    /// Boss idle aura.
    pub const BOSS_AURA: f32 = 1.0;
    /// Boss attack flash.
    pub const BOSS_ATTACK: f32 = 2.0;
    /// Boss phase transition.
    pub const BOSS_PHASE_CHANGE: f32 = 3.5;

    // ── Special ─────────────────────────────────────────────────────────
    /// Shrine glow.
    pub const SHRINE: f32 = 1.5;
    /// Portal glow.
    pub const PORTAL: f32 = 2.0;
    /// Corruption visual.
    pub const CORRUPTION: f32 = 0.8;
    /// Loot drop sparkle.
    pub const LOOT_SPARKLE: f32 = 1.8;
}

// ── Theme-based bloom profiles ──────────────────────────────────────────────

/// Bloom intensity profiles per game theme.
pub struct ThemeBloom;

impl ThemeBloom {
    /// VOID PROTOCOL: subtle, restrained bloom.
    pub fn void_protocol() -> BloomParams {
        BloomParams {
            enabled: true,
            threshold: 0.8,
            intensity: 0.4,
            radius: 3.0,
            levels: 3,
            knee: 0.1,
            use_emission: true,
            emission_weight: 1.0,
        }
    }

    /// SOLAR FORGE: dramatic, bright bloom.
    pub fn solar_forge() -> BloomParams {
        BloomParams {
            enabled: true,
            threshold: 0.5,
            intensity: 2.0,
            radius: 6.0,
            levels: 5,
            knee: 0.15,
            use_emission: true,
            emission_weight: 2.0,
        }
    }

    /// NEON GRID: cyberpunk-style vivid bloom.
    pub fn neon_grid() -> BloomParams {
        BloomParams {
            enabled: true,
            threshold: 0.6,
            intensity: 1.5,
            radius: 4.0,
            levels: 4,
            knee: 0.1,
            use_emission: true,
            emission_weight: 1.8,
        }
    }

    /// CORRUPTION: everything slightly brighter as corruption rises.
    pub fn corruption(level: f32) -> BloomParams {
        let corruption_boost = (level / 1000.0).clamp(0.0, 1.0);
        BloomParams {
            enabled: true,
            threshold: 0.7 - corruption_boost * 0.3,
            intensity: 0.6 + corruption_boost * 1.5,
            radius: 4.0 + corruption_boost * 3.0,
            levels: 4,
            knee: 0.1 + corruption_boost * 0.1,
            use_emission: true,
            emission_weight: 1.2 + corruption_boost * 0.8,
        }
    }

    /// Boss fight: high contrast, focused bloom.
    pub fn boss_fight() -> BloomParams {
        BloomParams {
            enabled: true,
            threshold: 0.6,
            intensity: 1.8,
            radius: 5.0,
            levels: 4,
            knee: 0.12,
            use_emission: true,
            emission_weight: 1.5,
        }
    }

    /// Death: bloom fades out.
    pub fn death(progress: f32) -> BloomParams {
        BloomParams {
            enabled: true,
            threshold: 0.5 + progress * 0.4,
            intensity: 1.0 * (1.0 - progress),
            radius: 4.0,
            levels: 3,
            knee: 0.1,
            use_emission: true,
            emission_weight: 1.0 * (1.0 - progress * 0.5),
        }
    }

    /// Shrine: warm, soft bloom.
    pub fn shrine() -> BloomParams {
        BloomParams {
            enabled: true,
            threshold: 0.5,
            intensity: 1.2,
            radius: 6.0,
            levels: 5,
            knee: 0.2,
            use_emission: true,
            emission_weight: 1.5,
        }
    }
}

// ── Multi-level bloom GPU pipeline descriptor ───────────────────────────────

/// Describes the full multi-level bloom pipeline state for one frame.
///
/// This is a CPU-side descriptor used to configure the GPU passes.
/// The actual GL rendering is done by PostFxPipeline using these parameters.
#[derive(Debug, Clone)]
pub struct BloomPipelineState {
    /// Active bloom parameters for this frame.
    pub params: BloomParams,
    /// Computed pyramid levels with sizes and weights.
    pub levels: Vec<BloomPyramidLevel>,
    /// Base resolution.
    pub base_width: u32,
    pub base_height: u32,
    /// Total number of draw calls this pipeline will issue.
    pub draw_call_count: u32,
}

impl BloomPipelineState {
    /// Compute the pipeline state for a given frame.
    pub fn compute(params: &BloomParams, width: u32, height: u32) -> Self {
        let mut levels = compute_pyramid(width, height, params);
        normalise_pyramid_weights(&mut levels);

        // Each level: 1 downsample + 2 blur passes (H+V) + 1 upsample = 4 draws
        // Plus: 1 threshold extract + 1 final composite = 2 more
        let draw_call_count = if params.enabled {
            1 + levels.len() as u32 * 4 + 1
        } else {
            0
        };

        Self {
            params: params.clone(),
            levels,
            base_width: width,
            base_height: height,
            draw_call_count,
        }
    }

    /// Whether bloom is active.
    pub fn is_active(&self) -> bool { self.params.enabled && !self.levels.is_empty() }

    /// Number of mip levels.
    pub fn level_count(&self) -> usize { self.levels.len() }
}

// ── GLSL shader sources for multi-level bloom ───────────────────────────────

/// Brightness threshold extraction shader.
///
/// Extracts pixels above a luminance threshold from the emission texture.
/// Uses a soft knee for smooth falloff.
pub const BLOOM_THRESHOLD_FRAG: &str = r#"
#version 330 core

in  vec2 f_uv;
out vec4 frag_color;

uniform sampler2D u_emission;
uniform float     u_threshold;
uniform float     u_knee;
uniform float     u_emission_weight;

const vec3 LUMA = vec3(0.2126, 0.7152, 0.0722);

void main() {
    vec3 emiss = texture(u_emission, f_uv).rgb * u_emission_weight;
    float lum  = dot(emiss, LUMA);

    // Soft threshold with knee
    float lo = u_threshold - u_knee;
    float hi = u_threshold + u_knee;
    float weight;
    if (lum <= lo) weight = 0.0;
    else if (lum >= hi) weight = 1.0;
    else {
        float t = (lum - lo) / (2.0 * u_knee + 0.0001);
        weight = t * t * (3.0 - 2.0 * t);
    }

    frag_color = vec4(emiss * weight, 1.0);
}
"#;

/// Downsample shader (box filter 2x2).
/// Takes a texture and renders it at half resolution.
pub const BLOOM_DOWNSAMPLE_FRAG: &str = r#"
#version 330 core

in  vec2 f_uv;
out vec4 frag_color;

uniform sampler2D u_texture;
uniform vec2      u_texel_size;

void main() {
    // 4-tap box downsample (tent filter)
    vec3 a = texture(u_texture, f_uv + u_texel_size * vec2(-0.5, -0.5)).rgb;
    vec3 b = texture(u_texture, f_uv + u_texel_size * vec2( 0.5, -0.5)).rgb;
    vec3 c = texture(u_texture, f_uv + u_texel_size * vec2(-0.5,  0.5)).rgb;
    vec3 d = texture(u_texture, f_uv + u_texel_size * vec2( 0.5,  0.5)).rgb;
    frag_color = vec4((a + b + c + d) * 0.25, 1.0);
}
"#;

/// Upsample shader (bilinear + additive blend).
/// Combines the current mip level with the upsampled lower mip.
pub const BLOOM_UPSAMPLE_FRAG: &str = r#"
#version 330 core

in  vec2 f_uv;
out vec4 frag_color;

uniform sampler2D u_lower_mip;   // smaller, more blurred
uniform sampler2D u_current_mip; // current resolution
uniform float     u_weight;      // blend weight for the lower mip

void main() {
    vec3 lower   = texture(u_lower_mip, f_uv).rgb;
    vec3 current = texture(u_current_mip, f_uv).rgb;
    frag_color = vec4(current + lower * u_weight, 1.0);
}
"#;

/// Separable Gaussian blur with variable sigma.
/// Direction (1,0) for horizontal, (0,1) for vertical.
pub const BLOOM_BLUR_FRAG: &str = r#"
#version 330 core

in  vec2 f_uv;
out vec4 frag_color;

uniform sampler2D u_texture;
uniform vec2      u_texel_size;
uniform vec2      u_direction;
uniform float     u_sigma;

// 9-tap kernel (bilinear optimized to 5 taps)
const int  N_TAPS = 5;
const float OFFSETS[5] = float[](0.0, 1.3846, 3.2308, 5.0769, 6.9231);
const float WEIGHTS[5] = float[](0.2270, 0.3162, 0.0703, 0.0162, 0.0054);

void main() {
    vec3 result = texture(u_texture, f_uv).rgb * WEIGHTS[0];
    float scale = u_sigma / 1.5;

    for (int i = 1; i < N_TAPS; ++i) {
        vec2 off = u_direction * u_texel_size * OFFSETS[i] * scale;
        result += texture(u_texture, f_uv + off).rgb * WEIGHTS[i];
        result += texture(u_texture, f_uv - off).rgb * WEIGHTS[i];
    }
    frag_color = vec4(result, 1.0);
}
"#;

// ── Bloom pipeline statistics ───────────────────────────────────────────────

/// Per-frame bloom statistics.
#[derive(Debug, Clone, Default)]
pub struct BloomStats {
    pub enabled: bool,
    pub levels: u8,
    pub threshold: f32,
    pub intensity: f32,
    pub draw_calls: u32,
    pub brightest_pixel_lum: f32,
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emission_presets_hierarchy() {
        assert_eq!(EmissionPresets::UI_TEXT, 0.0);
        assert!(EmissionPresets::CHAOS_FIELD < EmissionPresets::DAMAGE_NUMBER);
        assert!(EmissionPresets::DAMAGE_NUMBER < EmissionPresets::CRIT_NUMBER);
        assert!(EmissionPresets::BOSS_AURA < EmissionPresets::BOSS_PHASE_CHANGE);
    }

    #[test]
    fn theme_bloom_void_protocol_is_subtle() {
        let params = ThemeBloom::void_protocol();
        assert!(params.intensity < 1.0);
        assert!(params.threshold > 0.7);
    }

    #[test]
    fn theme_bloom_solar_forge_is_dramatic() {
        let params = ThemeBloom::solar_forge();
        assert!(params.intensity > 1.5);
        assert!(params.levels >= 4);
    }

    #[test]
    fn corruption_bloom_scales() {
        let low = ThemeBloom::corruption(0.0);
        let high = ThemeBloom::corruption(1000.0);
        assert!(high.intensity > low.intensity);
        assert!(high.threshold < low.threshold);
    }

    #[test]
    fn pipeline_state_computes_levels() {
        let params = BloomParams::default();
        let state = BloomPipelineState::compute(&params, 1280, 720);
        assert_eq!(state.levels.len(), params.levels as usize);
        assert!(state.draw_call_count > 0);
    }

    #[test]
    fn pipeline_state_disabled() {
        let params = BloomParams::disabled();
        let state = BloomPipelineState::compute(&params, 1280, 720);
        assert!(!state.is_active());
        assert_eq!(state.draw_call_count, 0);
    }

    #[test]
    fn death_bloom_fades() {
        let start = ThemeBloom::death(0.0);
        let end = ThemeBloom::death(1.0);
        assert!(end.intensity < start.intensity);
    }
}
