//! Oscillators, ADSR envelope, and basic filters.

/// ADSR envelope.
pub struct Adsr {
    pub attack:  f32,   // seconds
    pub decay:   f32,   // seconds
    pub sustain: f32,   // level [0, 1]
    pub release: f32,   // seconds
}

impl Adsr {
    pub fn level(&self, age: f32, note_off: Option<f32>) -> f32 {
        if let Some(off) = note_off {
            let rel = (age - off).max(0.0);
            return (self.sustain * (1.0 - rel / self.release.max(0.001))).max(0.0);
        }
        if age < self.attack {
            return age / self.attack.max(0.001);
        }
        let after_attack = age - self.attack;
        if after_attack < self.decay {
            let t = after_attack / self.decay.max(0.001);
            return 1.0 + t * (self.sustain - 1.0);
        }
        self.sustain
    }
}

/// Generate one sample of a waveform.
pub fn oscillator(waveform: super::math_source::Waveform, phase: f32) -> f32 {
    use super::math_source::Waveform;
    use std::f32::consts::TAU;
    match waveform {
        Waveform::Sine      => (phase * TAU).sin(),
        Waveform::Triangle  => 2.0 * (phase - (phase + 0.5).floor()).abs() * 2.0 - 1.0,
        Waveform::Square    => if phase < 0.5 { 1.0 } else { -1.0 },
        Waveform::Sawtooth  => 2.0 * (phase - phase.floor()) - 1.0,
        Waveform::Noise     => {
            let h = (phase * 13371.0) as u64;
            let h = h.wrapping_mul(0x9e3779b97f4a7c15);
            ((h >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }
}
