//! Film grain overlay — per-pixel random brightness noise.

#[derive(Clone, Debug)]
pub struct GrainParams {
    pub enabled: bool,
    /// Grain strength (0.02 = subtle, 0.1 = visible).
    pub intensity: f32,
}

impl Default for GrainParams {
    fn default() -> Self { Self { enabled: true, intensity: 0.02 } }
}
