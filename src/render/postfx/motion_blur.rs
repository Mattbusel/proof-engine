//! Velocity-based motion blur pass.
//!
//! Glyphs with high velocity are rendered multiple times along their velocity
//! vector with decreasing opacity, creating streak effects.

#[derive(Clone, Debug)]
pub struct MotionBlurParams {
    pub enabled: bool,
    /// Maximum number of blur samples.
    pub samples: u8,
    /// Velocity scale (how much velocity translates to blur amount).
    pub scale: f32,
}

impl Default for MotionBlurParams {
    fn default() -> Self { Self { enabled: true, samples: 8, scale: 0.3 } }
}
