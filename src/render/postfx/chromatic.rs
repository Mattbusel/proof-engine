//! Chromatic aberration pass — RGB channel offset.
//!
//! Scales with screen shake, corruption level, and proximity to Chaos Rifts.

#[derive(Clone, Debug)]
pub struct ChromaticParams {
    pub enabled: bool,
    /// Offset amount in UV units (0.002 = subtle, 0.02 = disorienting).
    pub intensity: f32,
}

impl Default for ChromaticParams {
    fn default() -> Self { Self { enabled: true, intensity: 0.002 } }
}
