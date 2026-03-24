//! Gaussian bloom pass.
//!
//! Extracts bright pixels (emission > threshold), applies two-pass Gaussian blur
//! at multiple radii, and additively blends back onto the main framebuffer.

/// Bloom pass configuration.
#[derive(Clone, Debug)]
pub struct BloomParams {
    pub enabled: bool,
    pub threshold: f32,   // minimum emission value to bloom
    pub intensity: f32,   // bloom brightness multiplier
    pub radius: f32,      // blur radius in pixels
    pub levels: u8,       // number of blur pyramid levels (more = softer, larger)
}

impl Default for BloomParams {
    fn default() -> Self {
        Self { enabled: true, threshold: 0.5, intensity: 1.0, radius: 4.0, levels: 3 }
    }
}
