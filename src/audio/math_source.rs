//! Math-driven audio source.
//!
//! The same MathFunction that drives a glyph's position also drives its sound.

use crate::math::MathFunction;
use glam::Vec3;

/// Waveform type for the oscillator.
#[derive(Clone, Copy, Debug)]
pub enum Waveform {
    Sine,
    Triangle,
    Square,
    Sawtooth,
    Noise,
}

/// An audio filter type.
#[derive(Clone, Debug)]
pub enum AudioFilter {
    LowPass  { cutoff_hz: f32, resonance: f32 },
    HighPass { cutoff_hz: f32, resonance: f32 },
    BandPass { center_hz: f32, bandwidth: f32 },
}

/// A mathematical audio source — output of a MathFunction mapped to sound.
#[derive(Clone, Debug)]
pub struct MathAudioSource {
    /// The function driving this source's frequency and amplitude.
    pub function: MathFunction,
    /// Frequency range the function output maps to (Hz).
    pub frequency_range: (f32, f32),
    /// Amplitude [0, 1].
    pub amplitude: f32,
    /// Waveform type.
    pub waveform: Waveform,
    /// Optional filter.
    pub filter: Option<AudioFilter>,
    /// 3D position for stereo/surround panning.
    pub position: Vec3,
    /// Tag for grouping/stopping sources.
    pub tag: Option<String>,
    /// Lifetime in seconds (-1 = infinite).
    pub lifetime: f32,
}

impl MathAudioSource {
    /// Create a simple sine tone driven by a breathing function.
    pub fn ambient_tone(freq: f32, amplitude: f32, position: Vec3) -> Self {
        Self {
            function: MathFunction::Breathing { rate: 0.25, depth: 0.2 },
            frequency_range: (freq * 0.9, freq * 1.1),
            amplitude,
            waveform: Waveform::Sine,
            filter: Some(AudioFilter::LowPass { cutoff_hz: freq * 4.0, resonance: 0.5 }),
            position,
            tag: None,
            lifetime: -1.0,
        }
    }

    /// Create a Lorenz-driven chaotic tone (for Chaos Rifts).
    pub fn chaos_tone(position: Vec3) -> Self {
        Self {
            function: MathFunction::Lorenz { sigma: 10.0, rho: 28.0, beta: 2.67, scale: 0.1 },
            frequency_range: (80.0, 800.0),
            amplitude: 0.3,
            waveform: Waveform::Triangle,
            filter: Some(AudioFilter::BandPass { center_hz: 400.0, bandwidth: 300.0 }),
            position,
            tag: Some("chaos_rift".to_string()),
            lifetime: -1.0,
        }
    }
}
