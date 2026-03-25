//! SDF Font Atlas — GPU-ready signed distance field texture with per-glyph metrics.
//!
//! The `SdfAtlas` wraps the output of `sdf_generator` and provides:
//!   - Lookup of UV rects and metrics by character
//!   - Configurable SDF spread and generation font size
//!   - Methods for computing screen-space smoothing at render time
//!   - Support for both single-channel SDF and multi-channel MSDF

use std::collections::HashMap;
use glam::Vec2;

use super::sdf_generator::{SdfConfig, SdfGlyphMetric, SdfAtlasData, generate_sdf_atlas};

// ── SdfAtlas ─────────────────────────────────────────────────────────────────

/// A font atlas using Signed Distance Fields for resolution-independent rendering.
///
/// Each pixel in the atlas stores the signed distance to the nearest glyph edge:
///   - 128 (0.5 normalized) = exactly on the edge
///   - 255 (1.0 normalized) = deep inside the glyph
///   - 0   (0.0 normalized) = far outside the glyph
///
/// The fragment shader thresholds this distance to produce a crisp edge at any
/// resolution, rotation, or scale.
pub struct SdfAtlas {
    /// R8 pixel data (each pixel = distance value).
    pub pixels: Vec<u8>,
    /// Atlas dimensions.
    pub width: u32,
    pub height: u32,
    /// Number of channels: 1 for SDF, 3 for MSDF.
    pub channels: u32,
    /// Per-glyph metrics and UV coordinates.
    pub glyph_metrics: HashMap<char, SdfGlyphMetric>,
    /// How many output pixels the distance field extends (typically 8.0).
    pub sdf_spread: f32,
    /// The font size at which the SDF was generated (typically 64px).
    pub font_size_px: f32,
}

impl SdfAtlas {
    /// Build an SDF atlas with default configuration.
    pub fn build() -> Self {
        Self::build_with_config(&SdfConfig::default())
    }

    /// Build an SDF atlas with custom configuration.
    pub fn build_with_config(config: &SdfConfig) -> Self {
        let data = generate_sdf_atlas(config);
        Self::from_atlas_data(data)
    }

    /// Build from pre-computed atlas data.
    pub fn from_atlas_data(data: SdfAtlasData) -> Self {
        Self {
            pixels: data.pixels,
            width: data.width,
            height: data.height,
            channels: data.channels,
            glyph_metrics: data.metrics,
            sdf_spread: data.spread,
            font_size_px: data.font_size_px,
        }
    }

    /// Get the metric for a character, falling back to '?' then a default.
    pub fn metric_for(&self, ch: char) -> SdfGlyphMetric {
        self.glyph_metrics
            .get(&ch)
            .or_else(|| self.glyph_metrics.get(&'?'))
            .copied()
            .unwrap_or(SdfGlyphMetric {
                uv_rect: [0.0, 0.0, 0.01, 0.01],
                size: Vec2::new(1.0, 1.0),
                bearing: Vec2::ZERO,
                advance: self.font_size_px * 0.6,
            })
    }

    /// Get the UV rect as [u_min, v_min, u_max, v_max].
    pub fn uv_for(&self, ch: char) -> [f32; 4] {
        self.metric_for(ch).uv_rect
    }

    /// Get the UV offset (top-left corner) as [u, v].
    pub fn uv_offset(&self, ch: char) -> [f32; 2] {
        let uv = self.uv_for(ch);
        [uv[0], uv[1]]
    }

    /// Get the UV size as [u_width, v_height].
    pub fn uv_size(&self, ch: char) -> [f32; 2] {
        let uv = self.uv_for(ch);
        [uv[2] - uv[0], uv[3] - uv[1]]
    }

    /// Compute the smoothing factor for SDF rendering based on the glyph's
    /// screen-space size.
    ///
    /// At large scales, the smoothing is very small → razor-sharp edges.
    /// At small scales, the smoothing is larger → anti-aliased edges.
    ///
    /// `screen_px_per_unit` is how many screen pixels one world unit occupies.
    /// `glyph_scale` is the scale multiplier on the glyph.
    pub fn compute_smoothing(&self, screen_px_per_unit: f32, glyph_scale: f32) -> f32 {
        let effective_px = screen_px_per_unit * glyph_scale;
        if effective_px <= 0.0 {
            return 0.1;
        }
        // The SDF spread covers `sdf_spread` pixels in the atlas texture.
        // At render time, one atlas texel covers (font_size_px / effective_px) screen pixels.
        // Smoothing should be approximately 1.0 / (effective_px * sdf_spread / font_size_px).
        let texels_per_screen_px = self.font_size_px / effective_px;
        (texels_per_screen_px / self.sdf_spread).clamp(0.001, 0.25)
    }

    /// Compute the threshold for SDF rendering.
    ///
    /// Default is 0.5 (on the edge).
    /// Lower values make glyphs bolder, higher values make them thinner.
    pub fn threshold(&self) -> f32 {
        0.5
    }

    /// Compute the threshold for a bold variant.
    pub fn bold_threshold(&self) -> f32 {
        0.45
    }

    /// Compute the outline range for a given outline width (in SDF-space units).
    ///
    /// Returns (inner_threshold, outer_threshold) for the outline smoothstep.
    pub fn outline_range(&self, outline_width: f32) -> (f32, f32) {
        let inner = 0.5;
        let outer = (0.5 - outline_width / self.sdf_spread).max(0.05);
        (inner, outer)
    }

    /// Compute the shadow UV offset for a drop shadow effect.
    ///
    /// `shadow_offset` is in screen pixels, `glyph_scale` is the current scale.
    pub fn shadow_uv_offset(&self, shadow_offset: Vec2, glyph_scale: f32) -> Vec2 {
        // Convert screen-pixel offset to UV-space offset in the atlas.
        let scale_factor = glyph_scale * self.font_size_px;
        if scale_factor <= 0.0 {
            return Vec2::ZERO;
        }
        Vec2::new(
            shadow_offset.x / (self.width as f32 * scale_factor / self.font_size_px),
            shadow_offset.y / (self.height as f32 * scale_factor / self.font_size_px),
        )
    }

    /// Total number of glyphs in the atlas.
    pub fn glyph_count(&self) -> usize {
        self.glyph_metrics.len()
    }

    /// Check if a character is available in the atlas.
    pub fn has_char(&self, ch: char) -> bool {
        self.glyph_metrics.contains_key(&ch)
    }

    /// Measure the width of a string in world units at a given scale.
    pub fn measure_string_width(&self, text: &str, scale: f32) -> f32 {
        let scale_factor = scale / self.font_size_px;
        text.chars()
            .map(|ch| self.metric_for(ch).advance * scale_factor)
            .sum()
    }

    /// Measure the height of a single line of text in world units at a given scale.
    pub fn line_height(&self, scale: f32) -> f32 {
        scale
    }
}

// ── SDF Effect Parameters ───────────────────────────────────────────────────

/// Per-glyph SDF rendering effects that can be applied in the fragment shader.
#[derive(Clone, Debug)]
pub struct SdfEffects {
    /// Whether to render an outline.
    pub outline: bool,
    /// Outline color (RGBA).
    pub outline_color: glam::Vec4,
    /// Outline width in SDF-space units (0.0 to ~0.2).
    pub outline_width: f32,

    /// Whether to render a drop shadow.
    pub shadow: bool,
    /// Shadow color (RGBA).
    pub shadow_color: glam::Vec4,
    /// Shadow offset in screen pixels.
    pub shadow_offset: Vec2,
    /// Shadow softness (blur radius in SDF-space, 0.0 to ~0.2).
    pub shadow_softness: f32,

    /// Whether to use the distance field as a glow source.
    pub glow: bool,
    /// Glow color (RGBA).
    pub glow_color: glam::Vec4,
    /// Glow radius in SDF-space (how far from edge the glow extends).
    pub glow_radius: f32,

    /// Bold mode: shifts the threshold to make glyphs thicker.
    pub bold: bool,

    /// UV distortion effects: wave, shake, glitch.
    pub wave_amplitude: f32,
    pub wave_frequency: f32,
    pub shake_amount: f32,
    pub glitch_intensity: f32,
}

impl Default for SdfEffects {
    fn default() -> Self {
        Self {
            outline: false,
            outline_color: glam::Vec4::new(0.0, 0.0, 0.0, 1.0),
            outline_width: 0.1,
            shadow: false,
            shadow_color: glam::Vec4::new(0.0, 0.0, 0.0, 0.6),
            shadow_offset: Vec2::new(2.0, -2.0),
            shadow_softness: 0.05,
            glow: false,
            glow_color: glam::Vec4::new(1.0, 1.0, 1.0, 0.5),
            glow_radius: 0.3,
            bold: false,
            wave_amplitude: 0.0,
            wave_frequency: 0.0,
            shake_amount: 0.0,
            glitch_intensity: 0.0,
        }
    }
}

impl SdfEffects {
    /// No effects — plain SDF rendering.
    pub fn none() -> Self {
        Self::default()
    }

    /// Outline only.
    pub fn outline(color: glam::Vec4, width: f32) -> Self {
        Self {
            outline: true,
            outline_color: color,
            outline_width: width,
            ..Self::default()
        }
    }

    /// Drop shadow only.
    pub fn shadow(color: glam::Vec4, offset: Vec2, softness: f32) -> Self {
        Self {
            shadow: true,
            shadow_color: color,
            shadow_offset: offset,
            shadow_softness: softness,
            ..Self::default()
        }
    }

    /// Glow only.
    pub fn glow(color: glam::Vec4, radius: f32) -> Self {
        Self {
            glow: true,
            glow_color: color,
            glow_radius: radius,
            ..Self::default()
        }
    }

    /// Bold text.
    pub fn bold() -> Self {
        Self {
            bold: true,
            ..Self::default()
        }
    }

    /// Wavy text animation.
    pub fn wave(amplitude: f32, frequency: f32) -> Self {
        Self {
            wave_amplitude: amplitude,
            wave_frequency: frequency,
            ..Self::default()
        }
    }

    /// Glitch effect.
    pub fn glitch(intensity: f32) -> Self {
        Self {
            glitch_intensity: intensity,
            ..Self::default()
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothing_inversely_proportional_to_scale() {
        let atlas = SdfAtlas {
            pixels: vec![],
            width: 512,
            height: 512,
            channels: 1,
            glyph_metrics: HashMap::new(),
            sdf_spread: 8.0,
            font_size_px: 64.0,
        };

        let small = atlas.compute_smoothing(10.0, 1.0);
        let large = atlas.compute_smoothing(100.0, 1.0);
        assert!(small > large, "Small scale should have more smoothing: {} vs {}", small, large);
    }

    #[test]
    fn outline_range_valid() {
        let atlas = SdfAtlas {
            pixels: vec![],
            width: 512,
            height: 512,
            channels: 1,
            glyph_metrics: HashMap::new(),
            sdf_spread: 8.0,
            font_size_px: 64.0,
        };

        let (inner, outer) = atlas.outline_range(1.0);
        assert!(inner > outer, "Inner threshold should be > outer: {} vs {}", inner, outer);
    }

    #[test]
    fn measure_string_empty() {
        let atlas = SdfAtlas {
            pixels: vec![],
            width: 512,
            height: 512,
            channels: 1,
            glyph_metrics: HashMap::new(),
            sdf_spread: 8.0,
            font_size_px: 64.0,
        };
        assert_eq!(atlas.measure_string_width("", 1.0), 0.0);
    }

    #[test]
    fn sdf_effects_defaults() {
        let fx = SdfEffects::default();
        assert!(!fx.outline);
        assert!(!fx.shadow);
        assert!(!fx.glow);
        assert!(!fx.bold);
    }
}
