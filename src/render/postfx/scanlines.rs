//! CRT scanline overlay — darkened horizontal lines mimicking a CRT phosphor display.
//!
//! Scanlines create the illusion of a retro CRT monitor by darkening every other
//! (or N-th) row of pixels. Additional effects like vertical sync wobble and
//! phosphor persistence can be emulated.

/// CRT scanline pass parameters.
#[derive(Clone, Debug)]
pub struct ScanlineParams {
    pub enabled: bool,
    /// Brightness reduction for scanline pixels (0.0 = no effect, 0.5 = half brightness).
    pub intensity: f32,
    /// How many pixels per scanline period (1.0 = every other pixel, 2.0 = every 2 pixels).
    pub line_width: f32,
    /// Number of screen lines between darkened scanlines (1 = every other, 2 = every 3rd).
    pub spacing: u32,
    /// Scanline orientation: true = horizontal (CRT rows), false = vertical (rotated CRT).
    pub horizontal: bool,
    /// Vertical sync wobble amplitude in pixels (0.0 = none, simulates V-sync instability).
    pub vsync_wobble: f32,
    /// Phosphor persistence: darkened areas slightly glow after scan (0.0 = none, 1.0 = strong).
    pub persistence: f32,
    /// Curvature of the scanline intensity: 0.0 = sharp, 1.0 = smooth gradient.
    pub smoothness: f32,
    /// Scanline color tint (for color CRT monitors, slight green/cyan tint).
    pub tint: [f32; 3],
}

impl Default for ScanlineParams {
    fn default() -> Self {
        Self {
            enabled:      false,
            intensity:    0.05,
            line_width:   1.0,
            spacing:      1,
            horizontal:   true,
            vsync_wobble: 0.0,
            persistence:  0.0,
            smoothness:   0.5,
            tint:         [1.0, 1.0, 1.0],
        }
    }
}

impl ScanlineParams {
    /// Disabled (no scanlines).
    pub fn none() -> Self { Self::default() }

    /// Subtle scanlines (barely visible, just adds texture).
    pub fn subtle() -> Self {
        Self {
            enabled:   true,
            intensity: 0.05,
            line_width: 1.0,
            spacing:   1,
            smoothness: 0.7,
            ..Default::default()
        }
    }

    /// Classic arcade CRT (strong scanlines, slight green tint, minimal wobble).
    pub fn arcade() -> Self {
        Self {
            enabled:   true,
            intensity: 0.25,
            line_width: 1.0,
            spacing:   1,
            smoothness: 0.3,
            tint:      [0.9, 1.0, 0.85],  // slight phosphor green
            ..Default::default()
        }
    }

    /// Damaged CRT (heavy scanlines, V-sync wobble, strong persistence).
    pub fn damaged() -> Self {
        Self {
            enabled:      true,
            intensity:    0.45,
            line_width:   1.5,
            spacing:      1,
            vsync_wobble: 2.5,
            persistence:  0.4,
            smoothness:   0.2,
            tint:         [0.8, 0.9, 0.8],
            ..Default::default()
        }
    }

    /// Wide-spaced scanlines for a lo-fi effect.
    pub fn lofi() -> Self {
        Self {
            enabled:   true,
            intensity: 0.35,
            line_width: 2.0,
            spacing:   2,
            smoothness: 0.1,
            ..Default::default()
        }
    }

    /// Lerp between two scanline configs.
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            enabled:      if t < 0.5 { a.enabled } else { b.enabled },
            intensity:    lerp_f32(a.intensity,    b.intensity,    t),
            line_width:   lerp_f32(a.line_width,   b.line_width,   t),
            spacing:      if t < 0.5 { a.spacing } else { b.spacing },
            horizontal:   a.horizontal,
            vsync_wobble: lerp_f32(a.vsync_wobble, b.vsync_wobble, t),
            persistence:  lerp_f32(a.persistence,  b.persistence,  t),
            smoothness:   lerp_f32(a.smoothness,   b.smoothness,   t),
            tint: [
                lerp_f32(a.tint[0], b.tint[0], t),
                lerp_f32(a.tint[1], b.tint[1], t),
                lerp_f32(a.tint[2], b.tint[2], t),
            ],
        }
    }

    /// CPU preview: evaluate the scanline dimming factor for a pixel at screen Y position.
    ///
    /// Returns a multiplier in [0, 1] — multiply pixel brightness by this value.
    /// `pixel_y` is the pixel row (0 = top), `screen_height` is total height.
    /// `time` drives V-sync wobble animation.
    pub fn evaluate(&self, pixel_y: f32, screen_height: f32, time: f32) -> f32 {
        if !self.enabled { return 1.0; }

        let mut y = pixel_y;

        // V-sync wobble: sine-wave vertical offset
        if self.vsync_wobble > 0.0 {
            y += (time * 60.0).sin() * self.vsync_wobble;
        }

        // Normalize position within a scanline period
        let period = (self.spacing as f32 + 1.0) * self.line_width;
        let phase = (y / period).fract();

        // Scanline darkening: phase near 0.5 gets darkened
        let darkened = if self.smoothness > 0.0 {
            // Smooth: use a cosine dip
            let dip = (phase * std::f32::consts::TAU).cos() * 0.5 + 0.5;
            let alpha = self.smoothness;
            dip * alpha + (1.0 - alpha) * (if phase < 0.5 { 1.0 } else { 0.0 })
        } else {
            if phase < 0.5 { 0.0 } else { 1.0 }
        };

        1.0 - darkened * self.intensity
    }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

// ── Scanline pattern generator ─────────────────────────────────────────────────

/// Generates a 1D scanline pattern texture for GPU upload.
///
/// Returns a Vec of f32 values (one per row), normalized to [0, 1].
/// Upload as a 1D texture sampled by the fragment shader.
pub fn generate_scanline_lut(height: u32, params: &ScanlineParams) -> Vec<f32> {
    (0..height)
        .map(|y| params.evaluate(y as f32, height as f32, 0.0))
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_returns_one() {
        let params = ScanlineParams::none();
        assert_eq!(params.evaluate(5.0, 100.0, 0.0), 1.0);
    }

    #[test]
    fn enabled_dims_some_pixels() {
        let params = ScanlineParams::arcade();
        let values: Vec<f32> = (0..20).map(|y| params.evaluate(y as f32, 100.0, 0.0)).collect();
        // Some pixels should be dimmed (< 1.0)
        let any_dimmed = values.iter().any(|&v| v < 0.99);
        assert!(any_dimmed, "Expected some pixels to be dimmed");
    }

    #[test]
    fn lut_has_correct_length() {
        let params = ScanlineParams::subtle();
        let lut = generate_scanline_lut(256, &params);
        assert_eq!(lut.len(), 256);
    }

    #[test]
    fn all_values_in_range() {
        let params = ScanlineParams::damaged();
        let lut = generate_scanline_lut(480, &params);
        for v in &lut {
            assert!(*v >= 0.0 && *v <= 1.0, "Out of range: {v}");
        }
    }

    #[test]
    fn lerp_halfway() {
        let a = ScanlineParams::none();
        let b = ScanlineParams { enabled: true, intensity: 0.4, ..Default::default() };
        let mid = ScanlineParams::lerp(&a, &b, 0.5);
        assert!((mid.intensity - 0.2).abs() < 0.001);
    }
}
