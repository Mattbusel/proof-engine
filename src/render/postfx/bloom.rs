//! Bloom post-processing pass.
//!
//! Implements a multi-level Gaussian bloom using a luminance threshold extract
//! followed by a separable ping-pong blur pyramid. The result is additively
//! blended onto the scene framebuffer.
//!
//! ## Pipeline
//! ```text
//! scene_color + scene_emission
//!   ── extract bright pixels ──▶ bright_fbo
//!   ── H blur (radius 1) ──────▶ blur_h[0]
//!   ── V blur (radius 1) ──────▶ blur_v[0]
//!   ── H blur (radius 2) ──────▶ blur_h[1]   ← pyramid level 1
//!   ── V blur (radius 2) ──────▶ blur_v[1]
//!   ─── additive composite ────▶ output
//! ```
//!
//! Each pyramid level is half-resolution, giving a wider, softer halo.

use std::f32;

// ── Bloom parameters ──────────────────────────────────────────────────────────

/// Configuration for the bloom pass.
#[derive(Clone, Debug)]
pub struct BloomParams {
    /// Enable or disable bloom entirely.
    pub enabled:   bool,
    /// Minimum luminance to include in bloom (0=all, 1=only bright pixels).
    pub threshold: f32,
    /// Additive blend weight of the bloom result.
    pub intensity: f32,
    /// Blur kernel radius in pixels (higher = softer/larger).
    pub radius:    f32,
    /// Number of pyramid levels (1=single, 3=multi-scale wide bloom).
    pub levels:    u8,
    /// Knee: soft threshold falloff width (higher = smoother cutoff).
    pub knee:      f32,
    /// Whether to use emission texture as additional bloom input.
    pub use_emission: bool,
    /// Contribution weight of the emission texture in bloom.
    pub emission_weight: f32,
}

impl Default for BloomParams {
    fn default() -> Self {
        Self {
            enabled:          true,
            threshold:        0.5,
            intensity:        1.0,
            radius:           4.0,
            levels:           3,
            knee:             0.1,
            use_emission:     true,
            emission_weight:  1.5,
        }
    }
}

impl BloomParams {
    pub fn disabled() -> Self {
        Self { enabled: false, ..Self::default() }
    }

    pub fn subtle() -> Self {
        Self { threshold: 0.7, intensity: 0.4, radius: 2.0, levels: 2, ..Self::default() }
    }

    pub fn intense() -> Self {
        Self { threshold: 0.3, intensity: 2.5, radius: 8.0, levels: 4, ..Self::default() }
    }

    pub fn retro_crt() -> Self {
        Self {
            threshold:   0.6,
            intensity:   1.2,
            radius:      3.0,
            levels:      3,
            knee:        0.05,
            emission_weight: 2.0,
            ..Self::default()
        }
    }

    /// Validate and clamp parameters to safe ranges.
    pub fn validate(&mut self) {
        self.threshold       = self.threshold.clamp(0.0, 1.0);
        self.intensity       = self.intensity.clamp(0.0, 10.0);
        self.radius          = self.radius.clamp(0.5, 32.0);
        self.levels          = self.levels.clamp(1, 6);
        self.knee            = self.knee.clamp(0.0, 0.5);
        self.emission_weight = self.emission_weight.clamp(0.0, 5.0);
    }
}

// ── Gaussian kernel ───────────────────────────────────────────────────────────

/// Compute a 1D Gaussian kernel of given `radius` (standard deviation).
/// Returns weights summing to 1 for a kernel of `2*size+1` taps.
pub fn gaussian_kernel(sigma: f32, size: usize) -> Vec<f32> {
    let mut weights: Vec<f32> = (0..=(size as i32 * 2))
        .map(|i| {
            let x = (i - size as i32) as f32;
            (-x * x / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f32 = weights.iter().sum();
    weights.iter_mut().for_each(|w| *w /= sum);
    weights
}

/// Separable Gaussian weights optimised for bilinear texture fetches.
/// Returns `(offsets, weights)` for a half-kernel (center + positive taps).
/// Linear sampling combines two adjacent texels, halving the tap count.
pub fn linear_gaussian_kernel(sigma: f32, taps: usize) -> (Vec<f32>, Vec<f32>) {
    let full = gaussian_kernel(sigma, taps);
    let half = taps + 1; // center + positive side

    let mut offsets = Vec::with_capacity(half);
    let mut weights = Vec::with_capacity(half);

    // Center tap
    offsets.push(0.0);
    weights.push(full[taps]);

    // Bilinear taps: each combines tap[k] and tap[k+1]
    let mut k = taps + 1;
    while k < full.len() - 1 {
        let w0 = full[k];
        let w1 = full[k + 1];
        let w  = w0 + w1;
        let o  = (k as f32 - taps as f32) + w1 / w;
        offsets.push(o);
        weights.push(w);
        k += 2;
    }
    if k < full.len() {
        offsets.push((k - taps) as f32);
        weights.push(full[k]);
    }

    (offsets, weights)
}

// ── Luminance utilities ───────────────────────────────────────────────────────

/// ITU-R BT.709 luminance coefficients.
const LUM_R: f32 = 0.2126;
const LUM_G: f32 = 0.7152;
const LUM_B: f32 = 0.0722;

/// Compute perceptual luminance from linear RGB.
#[inline]
pub fn luminance(r: f32, g: f32, b: f32) -> f32 {
    LUM_R * r + LUM_G * g + LUM_B * b
}

/// Soft-threshold a luminance value with knee falloff.
/// Pixels below `threshold - knee` contribute 0, above `threshold + knee` contribute fully.
pub fn soft_threshold(lum: f32, threshold: f32, knee: f32) -> f32 {
    if knee < 1e-5 {
        return if lum > threshold { 1.0 } else { 0.0 };
    }
    let lo = threshold - knee;
    let hi = threshold + knee;
    if lum <= lo  { return 0.0; }
    if lum >= hi  { return 1.0; }
    let t = (lum - lo) / (2.0 * knee);
    t * t * (3.0 - 2.0 * t) // smoothstep
}

/// Extract the bloom contribution from a pixel with a soft threshold.
/// Returns `(r, g, b)` with the threshold applied.
pub fn extract_bloom_pixel(r: f32, g: f32, b: f32, threshold: f32, knee: f32) -> (f32, f32, f32) {
    let lum    = luminance(r, g, b);
    let weight = soft_threshold(lum, threshold, knee);
    (r * weight, g * weight, b * weight)
}

// ── Pyramid level descriptor ──────────────────────────────────────────────────

/// Descriptor for one level of the bloom pyramid.
#[derive(Debug, Clone)]
pub struct BloomPyramidLevel {
    /// Width of this level in pixels.
    pub width:  u32,
    /// Height of this level in pixels.
    pub height: u32,
    /// Blur sigma (standard deviation) for this level.
    pub sigma:  f32,
    /// Contribution weight when compositing all levels.
    pub weight: f32,
}

/// Compute the pyramid levels for a given base resolution and params.
pub fn compute_pyramid(
    base_width:  u32,
    base_height: u32,
    params:      &BloomParams,
) -> Vec<BloomPyramidLevel> {
    let n = params.levels as usize;
    let mut levels = Vec::with_capacity(n);

    for i in 0..n {
        let scale  = 1u32 << (i + 1); // level 0 = half-res, level 1 = quarter-res ...
        let w      = (base_width  / scale).max(1);
        let h      = (base_height / scale).max(1);
        let sigma  = params.radius * (i as f32 * 0.5 + 1.0);
        // Higher pyramid levels contribute less (exponential decay)
        let weight = 1.0 / (i as f32 + 1.0);
        levels.push(BloomPyramidLevel { width: w, height: h, sigma, weight });
    }
    levels
}

/// Normalise pyramid weights so they sum to 1.
pub fn normalise_pyramid_weights(levels: &mut [BloomPyramidLevel]) {
    let total: f32 = levels.iter().map(|l| l.weight).sum();
    if total > 0.0 {
        for l in levels.iter_mut() { l.weight /= total; }
    }
}

// ── GLSL shader source fragments ──────────────────────────────────────────────

/// GLSL fragment shader source for the bright-extract pass.
/// Expects:
///   `u_scene`:     sampler2D — full scene color
///   `u_emission`:  sampler2D — emission texture (optional)
///   `u_threshold`: float
///   `u_knee`:      float
///   `u_emission_weight`: float
pub const EXTRACT_FRAG: &str = r#"
#version 330 core

in  vec2 v_uv;
out vec4 frag_color;

uniform sampler2D u_scene;
uniform sampler2D u_emission;
uniform float     u_threshold;
uniform float     u_knee;
uniform float     u_emission_weight;

const vec3 LUMA = vec3(0.2126, 0.7152, 0.0722);

float soft_threshold(float lum) {
    float lo = u_threshold - u_knee;
    float hi = u_threshold + u_knee;
    if (lum <= lo) return 0.0;
    if (lum >= hi) return 1.0;
    float t = (lum - lo) / (2.0 * u_knee + 0.0001);
    return t * t * (3.0 - 2.0 * t);
}

void main() {
    vec3 scene = texture(u_scene, v_uv).rgb;
    vec3 emiss = texture(u_emission, v_uv).rgb * u_emission_weight;
    vec3 combined = scene + emiss;

    float lum    = dot(combined, LUMA);
    float weight = soft_threshold(lum);

    frag_color = vec4(combined * weight, 1.0);
}
"#;

/// GLSL fragment shader source for the separable Gaussian blur pass.
/// Expects:
///   `u_texture`:    sampler2D — input texture
///   `u_texel_size`: vec2      — 1/resolution
///   `u_direction`:  vec2      — (1,0) for H, (0,1) for V
///   `u_sigma`:      float     — Gaussian sigma in pixels
pub const BLUR_FRAG: &str = r#"
#version 330 core

in  vec2 v_uv;
out vec4 frag_color;

uniform sampler2D u_texture;
uniform vec2      u_texel_size;
uniform vec2      u_direction;
uniform float     u_sigma;

// Fixed 9-tap kernel weights + offsets (radius 4, precomputed for sigma≈1.5)
// For variable sigma, you'd compute these on the CPU and upload as uniforms.
const int  N_TAPS    = 5;
const float OFFSETS[5] = float[](0.0, 1.3846153846, 3.2307692308, 5.0769230769, 6.9230769231);
const float WEIGHTS[5] = float[](0.2270270270, 0.3162162162, 0.0702702703, 0.0162162162, 0.0054054054);

void main() {
    vec4 result = texture(u_texture, v_uv) * WEIGHTS[0];
    for (int i = 1; i < N_TAPS; ++i) {
        vec2 off = u_direction * u_texel_size * OFFSETS[i] * (u_sigma / 1.5);
        result += texture(u_texture, v_uv + off) * WEIGHTS[i];
        result += texture(u_texture, v_uv - off) * WEIGHTS[i];
    }
    frag_color = result;
}
"#;

/// GLSL fragment shader source for the bloom composite pass.
/// Expects:
///   `u_scene`:     sampler2D — original scene
///   `u_bloom`:     sampler2D — blurred bloom
///   `u_intensity`: float     — additive blend weight
///   `u_dirt`:      sampler2D — optional lens dirt mask
///   `u_dirt_intensity`: float
pub const COMPOSITE_FRAG: &str = r#"
#version 330 core

in  vec2 v_uv;
out vec4 frag_color;

uniform sampler2D u_scene;
uniform sampler2D u_bloom;
uniform float     u_intensity;

void main() {
    vec3 scene = texture(u_scene, v_uv).rgb;
    vec3 bloom = texture(u_bloom, v_uv).rgb;
    // Additive bloom blend
    vec3 result = scene + bloom * u_intensity;
    frag_color = vec4(result, 1.0);
}
"#;

// ── CPU-side bloom simulation (for testing / software path) ───────────────────

/// Simulate one horizontal Gaussian blur pass on a flat `width × height` RGBA buffer.
/// `buffer` is `RGBA` interleaved (stride = width * 4).
pub fn cpu_blur_h(src: &[f32], dst: &mut [f32], width: usize, height: usize, sigma: f32) {
    let (offsets, weights) = linear_gaussian_kernel(sigma, (sigma * 3.0) as usize + 1);
    for y in 0..height {
        for x in 0..width {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = 0.0f32;
            for (i, &w) in weights.iter().enumerate() {
                let offset = offsets[i];
                let xi = (x as f32 + offset).round() as isize;
                let xi = xi.clamp(0, width as isize - 1) as usize;
                let idx = (y * width + xi) * 4;
                r += src[idx    ] * w;
                g += src[idx + 1] * w;
                b += src[idx + 2] * w;
                a += src[idx + 3] * w;
                if i > 0 {
                    let xim = (x as f32 - offset).round() as isize;
                    let xim = xim.clamp(0, width as isize - 1) as usize;
                    let idxm = (y * width + xim) * 4;
                    r += src[idxm    ] * w;
                    g += src[idxm + 1] * w;
                    b += src[idxm + 2] * w;
                    a += src[idxm + 3] * w;
                }
            }
            let out = (y * width + x) * 4;
            dst[out    ] = r;
            dst[out + 1] = g;
            dst[out + 2] = b;
            dst[out + 3] = a;
        }
    }
}

/// Simulate one vertical Gaussian blur pass on a flat RGBA buffer.
pub fn cpu_blur_v(src: &[f32], dst: &mut [f32], width: usize, height: usize, sigma: f32) {
    let (offsets, weights) = linear_gaussian_kernel(sigma, (sigma * 3.0) as usize + 1);
    for y in 0..height {
        for x in 0..width {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut a = 0.0f32;
            for (i, &w) in weights.iter().enumerate() {
                let offset = offsets[i];
                let yi  = (y as f32 + offset).round() as isize;
                let yi  = yi.clamp(0, height as isize - 1) as usize;
                let idx = (yi * width + x) * 4;
                r += src[idx    ] * w;
                g += src[idx + 1] * w;
                b += src[idx + 2] * w;
                a += src[idx + 3] * w;
                if i > 0 {
                    let yim = (y as f32 - offset).round() as isize;
                    let yim = yim.clamp(0, height as isize - 1) as usize;
                    let idxm = (yim * width + x) * 4;
                    r += src[idxm    ] * w;
                    g += src[idxm + 1] * w;
                    b += src[idxm + 2] * w;
                    a += src[idxm + 3] * w;
                }
            }
            let out = (y * width + x) * 4;
            dst[out    ] = r;
            dst[out + 1] = g;
            dst[out + 2] = b;
            dst[out + 3] = a;
        }
    }
}

/// Full CPU bloom simulation (extract → H blur → V blur → composite).
/// Returns a new RGBA buffer with bloom composited onto the input.
pub fn cpu_bloom(
    input:  &[f32],
    width:  usize,
    height: usize,
    params: &BloomParams,
) -> Vec<f32> {
    let n = width * height * 4;
    let mut extracted = vec![0.0f32; n];
    let mut blurred   = vec![0.0f32; n];

    // Extract bright pixels
    for i in 0..(width * height) {
        let base  = i * 4;
        let (r, g, b) = extract_bloom_pixel(input[base], input[base + 1], input[base + 2],
                                            params.threshold, params.knee);
        extracted[base    ] = r;
        extracted[base + 1] = g;
        extracted[base + 2] = b;
        extracted[base + 3] = input[base + 3];
    }

    // H blur
    let mut tmp = vec![0.0f32; n];
    cpu_blur_h(&extracted, &mut tmp, width, height, params.radius);
    // V blur
    cpu_blur_v(&tmp, &mut blurred, width, height, params.radius);

    // Composite: input + bloom * intensity
    let mut output = input.to_vec();
    for i in 0..(width * height) {
        let base = i * 4;
        output[base    ] = (output[base    ] + blurred[base    ] * params.intensity).min(1.0);
        output[base + 1] = (output[base + 1] + blurred[base + 1] * params.intensity).min(1.0);
        output[base + 2] = (output[base + 2] + blurred[base + 2] * params.intensity).min(1.0);
    }
    output
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_kernel_sums_to_one() {
        let k = gaussian_kernel(2.0, 4);
        let sum: f32 = k.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "sum={sum}");
    }

    #[test]
    fn soft_threshold_at_zero_knee() {
        assert_eq!(soft_threshold(0.4, 0.5, 0.0), 0.0);
        assert_eq!(soft_threshold(0.6, 0.5, 0.0), 1.0);
    }

    #[test]
    fn soft_threshold_smooth_at_knee() {
        let t = soft_threshold(0.5, 0.5, 0.1);
        assert!(t > 0.0 && t < 1.0, "expected soft transition, got {t}");
    }

    #[test]
    fn pyramid_has_correct_level_count() {
        let params = BloomParams { levels: 3, ..Default::default() };
        let levels = compute_pyramid(1280, 720, &params);
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0].width, 640);
        assert_eq!(levels[1].width, 320);
    }

    #[test]
    fn cpu_bloom_preserves_size() {
        let w = 4usize; let h = 4usize;
        let input: Vec<f32> = vec![0.5; w * h * 4];
        let output = cpu_bloom(&input, w, h, &BloomParams::default());
        assert_eq!(output.len(), input.len());
    }
}
