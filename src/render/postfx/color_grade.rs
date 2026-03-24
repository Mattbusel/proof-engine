//! Color grading pass — hue shift, saturation, contrast, tint, vignette.

use glam::Vec3;

/// Color grading parameters for this frame.
#[derive(Clone, Debug)]
pub struct ColorGradeParams {
    pub enabled: bool,
    /// Overall tint multiplied onto the final image (RGB, 1.0 = neutral).
    pub tint: Vec3,
    /// Saturation multiplier (1.0 = normal, 0.0 = greyscale, >1 = oversaturated).
    pub saturation: f32,
    /// Contrast multiplier (1.0 = normal).
    pub contrast: f32,
    /// Brightness offset (0.0 = normal).
    pub brightness: f32,
    /// Hue rotation in degrees (0.0 = none).
    pub hue_shift: f32,
    /// Vignette strength (0.0 = none, 1.0 = full black edges).
    pub vignette: f32,
}

impl Default for ColorGradeParams {
    fn default() -> Self {
        Self {
            enabled: true,
            tint: Vec3::ONE,
            saturation: 1.0,
            contrast: 1.0,
            brightness: 0.0,
            hue_shift: 0.0,
            vignette: 0.15,
        }
    }
}

impl ColorGradeParams {
    /// Create a red-tinted grade for hit flash effects.
    pub fn hit_flash(intensity: f32) -> Self {
        Self {
            tint: Vec3::new(1.0 + intensity, 0.8, 0.8),
            saturation: 1.2,
            vignette: 0.15 + intensity * 0.3,
            ..Default::default()
        }
    }

    /// Create a desaturated grade for death sequence.
    pub fn death(progress: f32) -> Self {
        Self {
            saturation: 1.0 - progress,
            brightness: -progress * 0.3,
            vignette: 0.15 + progress * 0.6,
            tint: Vec3::new(0.8, 0.7, 0.7),
            ..Default::default()
        }
    }

    /// Lerp between two color grades.
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            enabled: a.enabled || b.enabled,
            tint: a.tint + (b.tint - a.tint) * t,
            saturation: a.saturation + (b.saturation - a.saturation) * t,
            contrast: a.contrast + (b.contrast - a.contrast) * t,
            brightness: a.brightness + (b.brightness - a.brightness) * t,
            hue_shift: a.hue_shift + (b.hue_shift - a.hue_shift) * t,
            vignette: a.vignette + (b.vignette - a.vignette) * t,
        }
    }
}
