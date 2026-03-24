//! Chromatic aberration pass — RGB channel spatial offset.
//!
//! Splits the R, G, B channels and samples them at slightly offset UV coordinates,
//! creating the color fringing effect seen in cheap lenses, corrupted displays,
//! and high-trauma screen shake. The offset direction is radial from the screen center.
//!
//! Scales with trauma, entropy fields, and Chaos Rift proximity.

/// Chromatic aberration pass parameters.
#[derive(Clone, Debug)]
pub struct ChromaticParams {
    pub enabled: bool,
    /// Radial offset amount in UV units for the red channel (0.002 = subtle).
    pub red_offset: f32,
    /// Radial offset for the blue channel (usually slightly more than red).
    pub blue_offset: f32,
    /// Green channel offset (usually 0 — green is the reference channel).
    pub green_offset: f32,
    /// Whether to scale the offset with distance from screen center (true = realistic lens).
    pub radial_scale: bool,
    /// Direction distortion: 0.0 = purely radial, 1.0 = tangential (rotational aberration).
    pub tangential: f32,
    /// Fringe color mixing: 0.0 = clean R/G/B, 1.0 = smeared spectrum.
    pub spectrum_spread: f32,
    /// Barrel distortion applied before chromatic split (0.0 = none, 0.1 = visible).
    pub barrel_distortion: f32,
}

impl Default for ChromaticParams {
    fn default() -> Self {
        Self {
            enabled:           true,
            red_offset:        0.002,
            blue_offset:       0.003,
            green_offset:      0.0,
            radial_scale:      true,
            tangential:        0.0,
            spectrum_spread:   0.0,
            barrel_distortion: 0.0,
        }
    }
}

impl ChromaticParams {
    /// Disabled chromatic aberration.
    pub fn none() -> Self { Self { enabled: false, ..Default::default() } }

    /// Subtle lens fringing (high quality glass).
    pub fn subtle() -> Self {
        Self {
            enabled: true,
            red_offset: 0.001, blue_offset: 0.0015, green_offset: 0.0,
            radial_scale: true, tangential: 0.0, spectrum_spread: 0.0,
            barrel_distortion: 0.0,
        }
    }

    /// Cheap plastic lens (pronounced aberration at edges).
    pub fn cheap_lens() -> Self {
        Self {
            enabled: true,
            red_offset: 0.006, blue_offset: 0.008, green_offset: 0.0,
            radial_scale: true, tangential: 0.15, spectrum_spread: 0.3,
            barrel_distortion: 0.04,
        }
    }

    /// Glitch / digital corruption (non-radial, strong).
    pub fn glitch() -> Self {
        Self {
            enabled: true,
            red_offset: 0.015, blue_offset: 0.012, green_offset: 0.005,
            radial_scale: false, tangential: 0.5, spectrum_spread: 0.7,
            barrel_distortion: 0.0,
        }
    }

    /// Chaos Rift proximity effect — scales with entropy [0, 1].
    pub fn chaos_rift(entropy: f32) -> Self {
        let s = entropy.clamp(0.0, 1.0);
        Self {
            enabled: s > 0.01,
            red_offset:  0.002 + s * 0.02,
            blue_offset: 0.003 + s * 0.025,
            green_offset: s * 0.005,
            radial_scale: true,
            tangential: s * 0.4,
            spectrum_spread: s * 0.6,
            barrel_distortion: s * 0.08,
        }
    }

    /// Trauma shake chromatic (scales with camera trauma [0, 1]).
    pub fn trauma_shake(trauma: f32) -> Self {
        let t = (trauma * trauma).clamp(0.0, 1.0);  // quadratic for natural falloff
        Self {
            enabled: t > 0.01,
            red_offset:  t * 0.012,
            blue_offset: t * 0.015,
            green_offset: 0.0,
            radial_scale: true,
            tangential: 0.0,
            spectrum_spread: t * 0.2,
            barrel_distortion: 0.0,
        }
    }

    /// Lerp between two chromatic configs.
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            enabled:           if t < 0.5 { a.enabled } else { b.enabled },
            red_offset:        lerp_f32(a.red_offset,        b.red_offset,        t),
            blue_offset:       lerp_f32(a.blue_offset,       b.blue_offset,       t),
            green_offset:      lerp_f32(a.green_offset,      b.green_offset,      t),
            radial_scale:      a.radial_scale,
            tangential:        lerp_f32(a.tangential,        b.tangential,        t),
            spectrum_spread:   lerp_f32(a.spectrum_spread,   b.spectrum_spread,   t),
            barrel_distortion: lerp_f32(a.barrel_distortion, b.barrel_distortion, t),
        }
    }

    /// CPU preview: compute the UV offset for each channel at a given screen UV.
    ///
    /// Returns (r_uv, g_uv, b_uv) — sample the texture at each of these UVs for the
    /// corresponding channel.
    /// `uv` is in [0, 1], center is (0.5, 0.5).
    pub fn channel_uvs(&self, uv: [f32; 2]) -> ([f32; 2], [f32; 2], [f32; 2]) {
        if !self.enabled {
            return (uv, uv, uv);
        }

        let cx = uv[0] - 0.5;
        let cy = uv[1] - 0.5;
        let radial_dist = (cx * cx + cy * cy).sqrt();

        // Radial direction (outward from center)
        let (rx, ry) = if radial_dist > 0.0001 {
            (cx / radial_dist, cy / radial_dist)
        } else {
            (0.0, 0.0)
        };

        // Tangential direction (perpendicular, clockwise)
        let (tx, ty) = (-ry, rx);

        // Scale factor
        let scale = if self.radial_scale { radial_dist * 2.0 } else { 1.0 };

        // Apply barrel distortion
        let barrel_r = self.barrel_distortion;
        let barrel_factor = |u: f32, v: f32| -> [f32; 2] {
            let dx = u - 0.5;
            let dy = v - 0.5;
            let r2 = dx * dx + dy * dy;
            let bd = 1.0 + barrel_r * r2;
            [0.5 + dx * bd, 0.5 + dy * bd]
        };

        let offset_uv = |channel_offset: f32| -> [f32; 2] {
            let radial_component = channel_offset * scale;
            let tang_component = channel_offset * self.tangential * scale;
            let ou = cx + (rx * radial_component + tx * tang_component);
            let ov = cy + (ry * radial_component + ty * tang_component);
            let base = [0.5 + ou, 0.5 + ov];
            barrel_factor(base[0], base[1])
        };

        (
            offset_uv( self.red_offset),
            offset_uv( self.green_offset),
            offset_uv(-self.blue_offset),  // blue shifts opposite to red
        )
    }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_returns_same_uv() {
        let params = ChromaticParams::none();
        let uv = [0.3, 0.7];
        let (r, g, b) = params.channel_uvs(uv);
        assert_eq!(r, uv);
        assert_eq!(g, uv);
        assert_eq!(b, uv);
    }

    #[test]
    fn enabled_offsets_channels_differently() {
        let params = ChromaticParams::cheap_lens();
        let uv = [0.8, 0.5];  // Right side of screen (non-zero radial)
        let (r, g, b) = params.channel_uvs(uv);
        // R and B should be at different positions
        assert!((r[0] - b[0]).abs() > 0.001, "R and B should differ at r={:?} b={:?}", r, b);
    }

    #[test]
    fn center_pixel_has_zero_offset() {
        let params = ChromaticParams::cheap_lens();
        let uv = [0.5, 0.5];  // exact center
        let (r, g, b) = params.channel_uvs(uv);
        // At center, radial direction is zero → no offset
        assert!((r[0] - 0.5).abs() < 0.001);
        assert!((b[0] - 0.5).abs() < 0.001);
        let _ = g;
    }

    #[test]
    fn chaos_rift_scales_with_entropy() {
        let low  = ChromaticParams::chaos_rift(0.1);
        let high = ChromaticParams::chaos_rift(0.9);
        assert!(high.red_offset > low.red_offset);
    }

    #[test]
    fn trauma_quadratic_at_zero_is_disabled() {
        let params = ChromaticParams::trauma_shake(0.0);
        assert!(!params.enabled);
    }
}
