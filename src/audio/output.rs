//! rodio/cpal integration for audio output. Phase 1 stub.

/// Stub audio output context. Full implementation in audio phase.
pub struct AudioOutput {
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioOutput {
    pub fn try_new() -> Option<Self> {
        // Phase 1: initialize rodio OutputStream here
        Some(Self { sample_rate: 44100, channels: 2 })
    }
}
