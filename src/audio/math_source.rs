//! Math-driven audio sources — the same MathFunctions that drive glyphs also drive sound.
//!
//! A MathAudioSource maps MathFunction output to frequency and amplitude in realtime.
//! The visual and auditory are the same computation expressed through different senses.
//!
//! # Design
//!
//! Each source has:
//!   - A `MathFunction` that evolves over time and produces a scalar in [-1, 1]
//!   - A `frequency_range` mapping that scalar to Hz
//!   - A `Waveform` type (sine, saw, square, etc.)
//!   - An optional `AudioFilter` (LP, HP, BP, notch)
//!   - A 3D `position` for spatial panning
//!   - A `tag` for grouping/stopping sources
//!   - A `lifetime` (-1.0 = infinite, positive = seconds)

use crate::math::MathFunction;
use glam::Vec3;

// ── Waveform ──────────────────────────────────────────────────────────────────

/// Waveform shape for this audio source's oscillator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform {
    Sine,
    Triangle,
    Square,
    Sawtooth,
    ReverseSaw,
    Noise,
    /// Pulse with duty cycle [0, 1].
    Pulse(f32),
}

impl Waveform {
    /// Returns the harmonic richness of this waveform (1.0 = rich, 0.0 = pure).
    pub fn harmonic_richness(&self) -> f32 {
        match self {
            Waveform::Sine => 0.0,
            Waveform::Triangle => 0.3,
            Waveform::Pulse(_) => 0.5,
            Waveform::Square => 0.6,
            Waveform::Sawtooth | Waveform::ReverseSaw => 1.0,
            Waveform::Noise => 1.0,
        }
    }
}

// ── Audio filter ──────────────────────────────────────────────────────────────

/// A filter applied to the oscillator output.
#[derive(Clone, Debug)]
pub enum AudioFilter {
    LowPass  { cutoff_hz: f32, resonance: f32 },
    HighPass { cutoff_hz: f32, resonance: f32 },
    BandPass { center_hz: f32, bandwidth: f32 },
    Notch    { center_hz: f32, bandwidth: f32 },
    /// Formant filter (vowel sound shaping).
    Formant  { f1_hz: f32, f2_hz: f32, f3_hz: f32 },
    /// Comb filter (metallic, string-like resonance).
    Comb     { delay_ms: f32, feedback: f32 },
}

impl AudioFilter {
    /// Whisper low-pass (removes harshness from noise sources).
    pub fn whisper() -> Self { Self::LowPass { cutoff_hz: 1500.0, resonance: 0.5 } }
    /// Telephone band-pass filter (300-3000 Hz telephone band).
    pub fn telephone() -> Self { Self::BandPass { center_hz: 1500.0, bandwidth: 2700.0 } }
    /// Muffled (very low cutoff, heavy felt mute effect).
    pub fn muffled() -> Self { Self::LowPass { cutoff_hz: 400.0, resonance: 0.3 } }
    /// Bright (high-pass to emphasize attack and transients).
    pub fn bright() -> Self { Self::HighPass { cutoff_hz: 2000.0, resonance: 0.7 } }
}

// ── Math audio source ─────────────────────────────────────────────────────────

/// A mathematical audio source — oscillator driven by a MathFunction.
#[derive(Clone, Debug)]
pub struct MathAudioSource {
    /// The function driving this source's frequency/amplitude modulation.
    pub function:         MathFunction,
    /// Maps function output [-1, 1] to Hz: (freq_at_neg1, freq_at_pos1).
    pub frequency_range:  (f32, f32),
    /// Base amplitude [0, 1].
    pub amplitude:        f32,
    /// Waveform shape.
    pub waveform:         Waveform,
    /// Optional filter chain.
    pub filter:           Option<AudioFilter>,
    /// 3D world position for stereo panning and distance attenuation.
    pub position:         Vec3,
    /// Optional second filter (two-pole filtering).
    pub filter2:          Option<AudioFilter>,
    /// Tag for grouping related sources (e.g. "chaos_rift", "music", "sfx").
    pub tag:              Option<String>,
    /// Lifetime in seconds. -1.0 = infinite.
    pub lifetime:         f32,
    /// Frequency detune in cents (+100 = 1 semitone up).
    pub detune_cents:     f32,
    /// Whether this source should spatialize (attenuate with distance from listener).
    pub spatial:          bool,
    /// Maximum distance for spatialization (beyond this = silent).
    pub max_distance:     f32,
    /// Fade-in duration in seconds (0.0 = instant).
    pub fade_in:          f32,
    /// Fade-out duration in seconds before lifetime ends (0.0 = instant).
    pub fade_out:         f32,
}

impl Default for MathAudioSource {
    fn default() -> Self {
        Self {
            function:        MathFunction::Constant(0.0),
            frequency_range: (220.0, 440.0),
            amplitude:       0.5,
            waveform:        Waveform::Sine,
            filter:          None,
            position:        Vec3::ZERO,
            filter2:         None,
            tag:             None,
            lifetime:        -1.0,
            detune_cents:    0.0,
            spatial:         true,
            max_distance:    50.0,
            fade_in:         0.0,
            fade_out:        0.0,
        }
    }
}

impl MathAudioSource {
    // ── Factory methods ───────────────────────────────────────────────────────

    /// Sine tone driven by a breathing function (volume/pitch gently pulsates).
    pub fn ambient_tone(freq: f32, amplitude: f32, position: Vec3) -> Self {
        Self {
            function:        MathFunction::Breathing { rate: 0.25, depth: 0.2 },
            frequency_range: (freq * 0.95, freq * 1.05),
            amplitude,
            waveform:        Waveform::Sine,
            filter:          Some(AudioFilter::LowPass { cutoff_hz: freq * 6.0, resonance: 0.4 }),
            position,
            spatial:         true,
            fade_in:         1.0,
            ..Default::default()
        }
    }

    /// Lorenz-driven chaotic tone (for Chaos Rifts and high-entropy regions).
    pub fn chaos_tone(position: Vec3) -> Self {
        Self {
            function:        MathFunction::Lorenz { sigma: 10.0, rho: 28.0, beta: 2.67, scale: 0.1 },
            frequency_range: (80.0, 800.0),
            amplitude:       0.3,
            waveform:        Waveform::Triangle,
            filter:          Some(AudioFilter::BandPass { center_hz: 400.0, bandwidth: 300.0 }),
            position,
            tag:             Some("chaos_rift".to_string()),
            spatial:         true,
            fade_in:         0.5,
            ..Default::default()
        }
    }

    /// Sine sweep — frequency glides between two values over a period.
    pub fn sweep(freq_start: f32, freq_end: f32, period: f32, position: Vec3) -> Self {
        Self {
            function:        MathFunction::Sine { frequency: 1.0 / period, amplitude: 1.0, phase: 0.0 },
            frequency_range: (freq_start, freq_end),
            amplitude:       0.4,
            waveform:        Waveform::Sine,
            spatial:         true,
            position,
            ..Default::default()
        }
    }

    /// Low-frequency drone (sub-bass rumble for boss encounters).
    pub fn boss_drone(position: Vec3) -> Self {
        Self {
            function:        MathFunction::Breathing { rate: 0.08, depth: 0.4 },
            frequency_range: (30.0, 55.0),
            amplitude:       0.6,
            waveform:        Waveform::Sawtooth,
            filter:          Some(AudioFilter::LowPass { cutoff_hz: 80.0, resonance: 0.8 }),
            position,
            tag:             Some("boss_drone".to_string()),
            spatial:         false,  // boss drone fills the whole room
            fade_in:         2.0,
            fade_out:        3.0,
            ..Default::default()
        }
    }

    /// Death knell — descending pitch with exponential decay.
    pub fn death_knell(position: Vec3) -> Self {
        Self {
            function:        MathFunction::Exponential { start: 1.0, target: 0.0, rate: 0.5 },
            frequency_range: (600.0, 80.0),
            amplitude:       0.5,
            waveform:        Waveform::Triangle,
            filter:          Some(AudioFilter::LowPass { cutoff_hz: 400.0, resonance: 0.6 }),
            position,
            tag:             Some("death".to_string()),
            lifetime:        3.0,
            spatial:         true,
            fade_out:        1.0,
            ..Default::default()
        }
    }

    /// Electrical crackle (noise burst for lightning effects).
    pub fn electrical_crackle(position: Vec3, duration: f32) -> Self {
        Self {
            function:        MathFunction::Perlin { frequency: 1.0, octaves: 1, amplitude: 1.0 },
            frequency_range: (800.0, 4000.0),
            amplitude:       0.7,
            waveform:        Waveform::Noise,
            filter:          Some(AudioFilter::BandPass { center_hz: 2000.0, bandwidth: 3000.0 }),
            position,
            tag:             Some("lightning".to_string()),
            lifetime:        duration,
            spatial:         true,
            fade_out:        0.05,
            ..Default::default()
        }
    }

    /// Attractor-driven harmonic resonance (entropic, alien, chaotic but musical).
    pub fn attractor_tone(attractor_scale: f32, root_hz: f32, position: Vec3) -> Self {
        let harmonics = [1.0, 1.5, 2.0, 3.0, 4.0]; // partial series
        let freq = root_hz * harmonics[(attractor_scale as usize) % harmonics.len()];
        Self {
            function:        MathFunction::Lorenz { sigma: 10.0, rho: 28.0, beta: 2.67, scale: attractor_scale },
            frequency_range: (freq * 0.8, freq * 1.2),
            amplitude:       0.25,
            waveform:        Waveform::Sine,
            filter:          Some(AudioFilter::BandPass { center_hz: freq, bandwidth: freq * 0.5 }),
            position,
            tag:             Some("attractor_tone".to_string()),
            spatial:         true,
            ..Default::default()
        }
    }

    /// Wind ambience (noise with slow modulation, for outdoor environments).
    pub fn wind(amplitude: f32) -> Self {
        Self {
            function:        MathFunction::Perlin { frequency: 0.3, octaves: 3, amplitude: 1.0 },
            frequency_range: (100.0, 500.0),
            amplitude,
            waveform:        Waveform::Noise,
            filter:          Some(AudioFilter::LowPass { cutoff_hz: 600.0, resonance: 0.3 }),
            position:        Vec3::ZERO,
            tag:             Some("ambient_wind".to_string()),
            lifetime:        -1.0,
            spatial:         false,
            fade_in:         3.0,
            fade_out:        3.0,
            ..Default::default()
        }
    }

    /// Combat pulse — rhythmic hit sound tied to gameplay events.
    pub fn combat_pulse(position: Vec3, frequency_hz: f32) -> Self {
        Self {
            function:        MathFunction::Square { amplitude: 1.0, frequency: frequency_hz / 60.0, duty: 0.1 },
            frequency_range: (120.0, 300.0),
            amplitude:       0.4,
            waveform:        Waveform::Square,
            filter:          Some(AudioFilter::BandPass { center_hz: 200.0, bandwidth: 200.0 }),
            position,
            tag:             Some("combat".to_string()),
            spatial:         true,
            ..Default::default()
        }
    }

    /// Victory fanfare tone — bright, rising, major third.
    pub fn victory(position: Vec3) -> Self {
        Self {
            function:        MathFunction::Sine { frequency: 0.5, amplitude: 1.0, phase: 0.0 },
            frequency_range: (440.0, 660.0),
            amplitude:       0.5,
            waveform:        Waveform::Triangle,
            filter:          Some(AudioFilter::HighPass { cutoff_hz: 200.0, resonance: 0.5 }),
            position,
            tag:             Some("victory".to_string()),
            lifetime:        3.0,
            spatial:         false,
            fade_out:        1.0,
            ..Default::default()
        }
    }

    /// Heartbeat — pulsing low frequency with biological timing.
    pub fn heartbeat(bpm: f32, position: Vec3) -> Self {
        let freq = bpm / 60.0;
        Self {
            function:        MathFunction::Square { amplitude: 1.0, frequency: freq, duty: 0.15 },
            frequency_range: (60.0, 120.0),
            amplitude:       0.5,
            waveform:        Waveform::Sine,
            filter:          Some(AudioFilter::LowPass { cutoff_hz: 150.0, resonance: 1.5 }),
            position,
            tag:             Some("heartbeat".to_string()),
            spatial:         true,
            ..Default::default()
        }
    }

    /// Portal hum — steady resonant tone for dimensional gateways.
    pub fn portal_hum(position: Vec3, frequency_hz: f32) -> Self {
        Self {
            function:        MathFunction::Breathing { rate: 0.3, depth: 0.15 },
            frequency_range: (frequency_hz * 0.98, frequency_hz * 1.02),
            amplitude:       0.35,
            waveform:        Waveform::Sine,
            filter:          Some(AudioFilter::BandPass { center_hz: frequency_hz, bandwidth: 50.0 }),
            filter2:         Some(AudioFilter::Comb { delay_ms: 20.0, feedback: 0.6 }),
            position,
            tag:             Some("portal".to_string()),
            spatial:         true,
            fade_in:         2.0,
            ..Default::default()
        }
    }

    // ── Modifier methods ──────────────────────────────────────────────────────

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn with_lifetime(mut self, secs: f32) -> Self {
        self.lifetime = secs;
        self
    }

    pub fn with_amplitude(mut self, amp: f32) -> Self {
        self.amplitude = amp.clamp(0.0, 1.0);
        self
    }

    pub fn with_position(mut self, pos: Vec3) -> Self {
        self.position = pos;
        self
    }

    pub fn with_detune(mut self, cents: f32) -> Self {
        self.detune_cents = cents;
        self
    }

    pub fn non_spatial(mut self) -> Self {
        self.spatial = false;
        self
    }

    pub fn with_fade(mut self, fade_in: f32, fade_out: f32) -> Self {
        self.fade_in  = fade_in;
        self.fade_out = fade_out;
        self
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Whether this source is a one-shot (has a finite lifetime).
    pub fn is_one_shot(&self) -> bool { self.lifetime > 0.0 }

    /// Whether this source has expired.
    pub fn is_expired(&self, age: f32) -> bool {
        self.lifetime > 0.0 && age >= self.lifetime
    }

    /// Envelope factor accounting for fade-in and fade-out at a given age.
    pub fn envelope(&self, age: f32) -> f32 {
        let fade_in_factor = if self.fade_in > 0.0 {
            (age / self.fade_in).min(1.0)
        } else {
            1.0
        };

        let fade_out_factor = if self.lifetime > 0.0 && self.fade_out > 0.0 {
            let remaining = self.lifetime - age;
            (remaining / self.fade_out).clamp(0.0, 1.0)
        } else {
            1.0
        };

        self.amplitude * fade_in_factor * fade_out_factor
    }

    /// Map a function output value in [-1, 1] to a frequency in Hz.
    pub fn map_to_frequency(&self, value: f32) -> f32 {
        let t = (value.clamp(-1.0, 1.0) + 1.0) * 0.5;
        let (lo, hi) = self.frequency_range;
        // Logarithmic interpolation for musical pitch perception
        let lo_log = lo.max(1.0).ln();
        let hi_log = hi.max(1.0).ln();
        (lo_log + t * (hi_log - lo_log)).exp()
    }
}

// ── Source preset library ──────────────────────────────────────────────────────

/// Quick-access library of common audio source presets.
pub struct AudioPresets;

impl AudioPresets {
    /// Ambient cave drip at a position.
    pub fn cave_drip(position: Vec3) -> MathAudioSource {
        MathAudioSource {
            function:        MathFunction::Square { amplitude: 1.0, frequency: 0.05, duty: 0.02 },
            frequency_range: (800.0, 1200.0),
            amplitude:       0.3,
            waveform:        Waveform::Sine,
            filter:          Some(AudioFilter::LowPass { cutoff_hz: 1000.0, resonance: 2.0 }),
            position,
            tag:             Some("cave_ambient".to_string()),
            lifetime:        -1.0,
            spatial:         true,
            ..Default::default()
        }
    }

    /// Explosion impact — loud, brief, with sub-bass punch.
    pub fn explosion(position: Vec3, scale: f32) -> MathAudioSource {
        MathAudioSource {
            function:        MathFunction::Exponential { start: 1.0, target: 0.0, rate: 2.0 },
            frequency_range: (30.0, 200.0 * scale),
            amplitude:       0.9,
            waveform:        Waveform::Noise,
            filter:          Some(AudioFilter::LowPass { cutoff_hz: 300.0 * scale, resonance: 0.3 }),
            position,
            tag:             Some("explosion".to_string()),
            lifetime:        0.5 + scale * 0.5,
            spatial:         true,
            max_distance:    30.0 * scale,
            fade_out:        0.3,
            ..Default::default()
        }
    }

    /// Magical sparkle — high-frequency sinusoidal shimmer.
    pub fn magic_sparkle(position: Vec3) -> MathAudioSource {
        MathAudioSource {
            function:        MathFunction::Breathing { rate: 8.0, depth: 0.5 },
            frequency_range: (2000.0, 6000.0),
            amplitude:       0.2,
            waveform:        Waveform::Sine,
            filter:          Some(AudioFilter::HighPass { cutoff_hz: 1500.0, resonance: 0.5 }),
            position,
            tag:             Some("magic".to_string()),
            lifetime:        0.8,
            spatial:         true,
            fade_out:        0.3,
            ..Default::default()
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_to_frequency_at_neg1_gives_lo() {
        let src = MathAudioSource::ambient_tone(440.0, 0.5, Vec3::ZERO);
        let f = src.map_to_frequency(-1.0);
        assert!((f - src.frequency_range.0).abs() < 1.0, "Expected ~lo, got {f}");
    }

    #[test]
    fn map_to_frequency_at_pos1_gives_hi() {
        let src = MathAudioSource::ambient_tone(440.0, 0.5, Vec3::ZERO);
        let f = src.map_to_frequency(1.0);
        assert!((f - src.frequency_range.1).abs() < 1.0, "Expected ~hi, got {f}");
    }

    #[test]
    fn envelope_at_zero_is_zero_for_fade_in() {
        let src = MathAudioSource::boss_drone(Vec3::ZERO);
        let env = src.envelope(0.0);
        assert!(env < 0.01, "Should be near zero at start of fade-in, got {env}");
    }

    #[test]
    fn envelope_at_peak_is_amplitude() {
        let src = MathAudioSource { fade_in: 0.0, lifetime: -1.0, amplitude: 0.7, ..Default::default() };
        let env = src.envelope(1.0);
        assert!((env - 0.7).abs() < 0.001);
    }

    #[test]
    fn one_shot_expires() {
        let src = MathAudioSource::death_knell(Vec3::ZERO);
        assert!(!src.is_expired(1.0));
        assert!(src.is_expired(10.0));
    }

    #[test]
    fn non_spatial_builder() {
        let src = MathAudioSource::wind(0.3);
        assert!(!src.spatial);
    }
}
