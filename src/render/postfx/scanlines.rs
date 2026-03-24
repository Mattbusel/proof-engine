//! Optional CRT scanline overlay.

#[derive(Clone, Debug)]
pub struct ScanlineParams {
    pub enabled: bool,
    /// Brightness variation per row (0.05 = subtle).
    pub intensity: f32,
}

impl Default for ScanlineParams {
    fn default() -> Self { Self { enabled: false, intensity: 0.05 } }
}
