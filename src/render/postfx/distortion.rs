//! Screen-space distortion pass.
//!
//! Force fields write to a distortion map. Each pixel's UV is offset by this map
//! when sampling the scene, creating gravitational lens, heat shimmer, and entropy distortion.

/// Distortion pass parameters.
#[derive(Clone, Debug)]
pub struct DistortionParams {
    pub enabled: bool,
    /// Global distortion scale multiplier.
    pub scale: f32,
}

impl Default for DistortionParams {
    fn default() -> Self { Self { enabled: true, scale: 1.0 } }
}
