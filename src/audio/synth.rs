//! DSP synthesis — oscillators, biquad filters, LFOs, FM synthesis, and effects chain.
//!
//! All sample-rate math uses the engine's fixed sample rate (48 kHz by default).
//! Every component is designed to be sample-accurate and allocation-free per sample.

use std::f32::consts::{PI, TAU};

pub const SAMPLE_RATE: f32 = 48_000.0;
pub const SAMPLE_RATE_INV: f32 = 1.0 / SAMPLE_RATE;

// ── ADSR envelope ─────────────────────────────────────────────────────────────

/// ADSR envelope generator. Produces a gain value in [0, 1] from a note timeline.
#[derive(Clone, Debug)]
pub struct Adsr {
    pub attack:  f32,   // seconds
    pub decay:   f32,   // seconds
    pub sustain: f32,   // level [0, 1]
    pub release: f32,   // seconds
}

impl Adsr {
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self { attack, decay, sustain, release }
    }

    /// Pad: quick attack, short decay, low sustain, short release.
    pub fn pluck() -> Self { Self::new(0.002, 0.1, 0.0, 0.05) }

    /// Slow pad: long attack/release.
    pub fn pad() -> Self { Self::new(0.3, 0.5, 0.7, 0.8) }

    /// Punchy hit: instant attack, fast decay, no sustain.
    pub fn hit() -> Self { Self::new(0.001, 0.05, 0.0, 0.02) }

    /// Drone: instant attack, long sustain.
    pub fn drone() -> Self { Self::new(0.01, 0.1, 0.9, 1.2) }

    /// Evaluate the envelope at `age` seconds since note-on.
    /// `note_off` is the age at which note-off occurred (None = still held).
    pub fn level(&self, age: f32, note_off: Option<f32>) -> f32 {
        if let Some(off) = note_off {
            let rel = (age - off).max(0.0);
            let sustain_level = self.level(off, None);
            let t = (1.0 - rel / self.release.max(0.0001)).max(0.0);
            return sustain_level * t * t;
        }
        if age < self.attack {
            return age / self.attack.max(0.0001);
        }
        let after_attack = age - self.attack;
        if after_attack < self.decay {
            let t = after_attack / self.decay.max(0.0001);
            return 1.0 - t * (1.0 - self.sustain);
        }
        self.sustain
    }

    /// True if the note has fully released and gain is essentially zero.
    pub fn is_silent(&self, age: f32, note_off: Option<f32>) -> bool {
        if let Some(off) = note_off {
            let rel = (age - off).max(0.0);
            return rel >= self.release;
        }
        false
    }
}

// ── Waveform ──────────────────────────────────────────────────────────────────

/// Basic oscillator waveforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform {
    Sine,
    Triangle,
    Square,
    Sawtooth,
    ReverseSaw,
    Noise,
    /// Pulse with variable duty cycle [0, 1].
    Pulse(f32),
}

/// Generate one sample from a waveform at a given phase [0, 1).
pub fn oscillator(waveform: Waveform, phase: f32) -> f32 {
    let p = phase - phase.floor();  // normalize to [0, 1)
    match waveform {
        Waveform::Sine       => (p * TAU).sin(),
        Waveform::Triangle   => {
            if p < 0.5 { 4.0 * p - 1.0 } else { 3.0 - 4.0 * p }
        }
        Waveform::Square     => if p < 0.5 { 1.0 } else { -1.0 },
        Waveform::Sawtooth   => 2.0 * p - 1.0,
        Waveform::ReverseSaw => 1.0 - 2.0 * p,
        Waveform::Pulse(duty) => if p < duty { 1.0 } else { -1.0 },
        Waveform::Noise      => {
            let h = (phase * 13371.333) as u64;
            let h = h.wrapping_mul(0x9e3779b97f4a7c15).wrapping_add(0x62b821756295c58d);
            (h >> 32) as f32 / u32::MAX as f32 * 2.0 - 1.0
        }
    }
}

// ── Stateful oscillator ───────────────────────────────────────────────────────

/// A stateful oscillator that tracks its phase across samples.
#[derive(Clone, Debug)]
pub struct Oscillator {
    pub waveform:  Waveform,
    pub frequency: f32,
    pub amplitude: f32,
    pub phase:     f32,
    /// Phase modulation input (for FM synthesis).
    pub pm_depth:  f32,
}

impl Oscillator {
    pub fn new(waveform: Waveform, frequency: f32, amplitude: f32) -> Self {
        Self { waveform, frequency, amplitude, phase: 0.0, pm_depth: 0.0 }
    }

    pub fn sine(frequency: f32) -> Self { Self::new(Waveform::Sine, frequency, 1.0) }
    pub fn saw(frequency: f32)  -> Self { Self::new(Waveform::Sawtooth, frequency, 1.0) }
    pub fn square(frequency: f32) -> Self { Self::new(Waveform::Square, frequency, 1.0) }
    pub fn tri(frequency: f32)  -> Self { Self::new(Waveform::Triangle, frequency, 1.0) }
    pub fn noise()               -> Self { Self::new(Waveform::Noise, 1.0, 1.0) }

    /// Advance by one sample and return the output value.
    pub fn tick(&mut self) -> f32 {
        let sample = oscillator(self.waveform, self.phase);
        self.phase += self.frequency * SAMPLE_RATE_INV;
        if self.phase >= 1.0 { self.phase -= 1.0; }
        sample * self.amplitude
    }

    /// Advance by one sample with external phase modulation (FM).
    pub fn tick_fm(&mut self, modulator: f32) -> f32 {
        let modulated_phase = self.phase + modulator * self.pm_depth;
        let sample = oscillator(self.waveform, modulated_phase);
        self.phase += self.frequency * SAMPLE_RATE_INV;
        if self.phase >= 1.0 { self.phase -= 1.0; }
        sample * self.amplitude
    }

    /// Reset phase to 0 (note retrigger).
    pub fn retrigger(&mut self) { self.phase = 0.0; }
}

// ── Biquad filter ─────────────────────────────────────────────────────────────

/// Biquad filter type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterMode {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    AllPass,
    LowShelf,
    HighShelf,
    Peak,
}

/// Biquad (second-order IIR) filter — used for low-pass, high-pass, resonant, etc.
#[derive(Clone, Debug)]
pub struct BiquadFilter {
    // Coefficients
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    // Delay line
    x1: f32, x2: f32,
    y1: f32, y2: f32,
    // Current parameters
    pub mode:      FilterMode,
    pub cutoff_hz: f32,
    pub resonance: f32,  // Q factor
    pub gain_db:   f32,  // for shelf/peak
}

impl BiquadFilter {
    pub fn new(mode: FilterMode, cutoff_hz: f32, resonance: f32) -> Self {
        let mut f = Self {
            b0: 1.0, b1: 0.0, b2: 0.0,
            a1: 0.0, a2: 0.0,
            x1: 0.0, x2: 0.0,
            y1: 0.0, y2: 0.0,
            mode,
            cutoff_hz,
            resonance,
            gain_db: 0.0,
        };
        f.update_coefficients();
        f
    }

    pub fn low_pass(cutoff_hz: f32, q: f32) -> Self {
        Self::new(FilterMode::LowPass, cutoff_hz, q)
    }

    pub fn high_pass(cutoff_hz: f32, q: f32) -> Self {
        Self::new(FilterMode::HighPass, cutoff_hz, q)
    }

    pub fn band_pass(cutoff_hz: f32, q: f32) -> Self {
        Self::new(FilterMode::BandPass, cutoff_hz, q)
    }

    pub fn notch(cutoff_hz: f32, q: f32) -> Self {
        Self::new(FilterMode::Notch, cutoff_hz, q)
    }

    /// Update filter coefficients after changing mode/cutoff/resonance.
    pub fn update_coefficients(&mut self) {
        let w0 = TAU * self.cutoff_hz / SAMPLE_RATE;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * self.resonance.max(0.0001));
        let a = 10.0f32.powf(self.gain_db / 40.0);

        let (b0, b1, b2, a0, a1, a2) = match self.mode {
            FilterMode::LowPass => (
                (1.0 - cos_w0) / 2.0,
                1.0 - cos_w0,
                (1.0 - cos_w0) / 2.0,
                1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha,
            ),
            FilterMode::HighPass => (
                (1.0 + cos_w0) / 2.0,
                -(1.0 + cos_w0),
                (1.0 + cos_w0) / 2.0,
                1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha,
            ),
            FilterMode::BandPass => (
                sin_w0 / 2.0, 0.0, -sin_w0 / 2.0,
                1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha,
            ),
            FilterMode::Notch => (
                1.0, -2.0 * cos_w0, 1.0,
                1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha,
            ),
            FilterMode::AllPass => (
                1.0 - alpha, -2.0 * cos_w0, 1.0 + alpha,
                1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha,
            ),
            FilterMode::LowShelf => {
                let sq = (a * ((a + 1.0 / a) * (1.0 / 1.0 - 1.0) + 2.0)).sqrt();
                (
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha),
                    (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
                    (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha + sq * 0.0,
                )
            }
            FilterMode::HighShelf => {
                (
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha),
                    (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
                    (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha,
                )
            }
            FilterMode::Peak => (
                1.0 + alpha * a,
                -2.0 * cos_w0,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos_w0,
                1.0 - alpha / a,
            ),
        };

        let a0_inv = 1.0 / a0;
        self.b0 = b0 * a0_inv;
        self.b1 = b1 * a0_inv;
        self.b2 = b2 * a0_inv;
        self.a1 = a1 * a0_inv;
        self.a2 = a2 * a0_inv;
    }

    /// Set cutoff frequency and update coefficients.
    pub fn set_cutoff(&mut self, hz: f32) {
        self.cutoff_hz = hz.clamp(20.0, SAMPLE_RATE * 0.49);
        self.update_coefficients();
    }

    /// Set resonance (Q) and update coefficients.
    pub fn set_resonance(&mut self, q: f32) {
        self.resonance = q.max(0.1);
        self.update_coefficients();
    }

    /// Process one sample.
    pub fn tick(&mut self, input: f32) -> f32 {
        let y = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
              - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    /// Reset filter state (useful between notes).
    pub fn reset(&mut self) {
        self.x1 = 0.0; self.x2 = 0.0;
        self.y1 = 0.0; self.y2 = 0.0;
    }
}

// ── LFO ──────────────────────────────────────────────────────────────────────

/// Low-frequency oscillator for modulating parameters.
#[derive(Clone, Debug)]
pub struct Lfo {
    pub waveform:  Waveform,
    pub rate_hz:   f32,
    pub depth:     f32,
    pub offset:    f32,  // DC offset added to output
    phase:         f32,
}

impl Lfo {
    pub fn new(waveform: Waveform, rate_hz: f32, depth: f32) -> Self {
        Self { waveform, rate_hz, depth, offset: 0.0, phase: 0.0 }
    }

    pub fn sine(rate_hz: f32, depth: f32) -> Self { Self::new(Waveform::Sine, rate_hz, depth) }
    pub fn tri(rate_hz: f32, depth: f32)  -> Self { Self::new(Waveform::Triangle, rate_hz, depth) }
    pub fn square(rate_hz: f32, depth: f32) -> Self { Self::new(Waveform::Square, rate_hz, depth) }

    /// Advance by one sample and return modulation value.
    pub fn tick(&mut self) -> f32 {
        let val = oscillator(self.waveform, self.phase);
        self.phase += self.rate_hz * SAMPLE_RATE_INV;
        if self.phase >= 1.0 { self.phase -= 1.0; }
        val * self.depth + self.offset
    }

    /// Set phase (0..1) for synchronizing multiple LFOs.
    pub fn set_phase(&mut self, phase: f32) { self.phase = phase.fract(); }
}

// ── FM operator ───────────────────────────────────────────────────────────────

/// A single FM synthesis operator (carrier or modulator).
#[derive(Clone, Debug)]
pub struct FmOperator {
    pub osc:          Oscillator,
    pub adsr:         Adsr,
    pub age:          f32,
    pub note_off_age: Option<f32>,
    pub output_level: f32,
}

impl FmOperator {
    pub fn new(frequency: f32, adsr: Adsr, output_level: f32) -> Self {
        Self {
            osc: Oscillator::sine(frequency),
            adsr,
            age: 0.0,
            note_off_age: None,
            output_level,
        }
    }

    /// Tick with optional FM modulator input. Returns output sample.
    pub fn tick(&mut self, modulator: f32) -> f32 {
        self.age += SAMPLE_RATE_INV;
        let env = self.adsr.level(self.age, self.note_off_age);
        let sample = self.osc.tick_fm(modulator);
        sample * env * self.output_level
    }

    pub fn note_on(&mut self) {
        self.age = 0.0;
        self.note_off_age = None;
        self.osc.retrigger();
    }

    pub fn note_off(&mut self) {
        self.note_off_age = Some(self.age);
    }

    pub fn is_silent(&self) -> bool {
        self.adsr.is_silent(self.age, self.note_off_age)
    }
}

// ── 2-operator FM voice ───────────────────────────────────────────────────────

/// A simple 2-op FM synthesis voice: modulator → carrier.
#[derive(Clone, Debug)]
pub struct FmVoice {
    pub carrier:        FmOperator,
    pub modulator:      FmOperator,
    /// Ratio of modulator frequency to carrier (e.g. 2.0 = octave above).
    pub mod_ratio:      f32,
    /// Modulation index — how strongly modulator affects carrier.
    pub mod_index:      f32,
}

impl FmVoice {
    pub fn new(base_freq: f32, mod_ratio: f32, mod_index: f32) -> Self {
        let carrier_adsr = Adsr::drone();
        let mod_adsr     = Adsr::new(0.01, 0.2, 0.5, 0.5);
        Self {
            carrier:   FmOperator::new(base_freq, carrier_adsr, 1.0),
            modulator: FmOperator::new(base_freq * mod_ratio, mod_adsr, mod_index),
            mod_ratio,
            mod_index,
        }
    }

    pub fn tick(&mut self) -> f32 {
        let mod_out = self.modulator.tick(0.0);
        self.carrier.tick(mod_out)
    }

    pub fn note_on(&mut self, frequency: f32) {
        self.carrier.osc.frequency = frequency;
        self.modulator.osc.frequency = frequency * self.mod_ratio;
        self.carrier.note_on();
        self.modulator.note_on();
    }

    pub fn note_off(&mut self) {
        self.carrier.note_off();
        self.modulator.note_off();
    }

    pub fn is_silent(&self) -> bool {
        self.carrier.is_silent()
    }
}

// ── Effects chain ─────────────────────────────────────────────────────────────

/// Simple delay line for reverb/echo effects.
#[derive(Clone, Debug)]
pub struct DelayLine {
    buffer:     Vec<f32>,
    write_pos:  usize,
    delay_samples: usize,
}

impl DelayLine {
    pub fn new(max_delay_ms: f32) -> Self {
        let max_samples = (SAMPLE_RATE * max_delay_ms * 0.001) as usize + 1;
        Self {
            buffer: vec![0.0; max_samples],
            write_pos: 0,
            delay_samples: max_samples / 2,
        }
    }

    pub fn set_delay_ms(&mut self, ms: f32) {
        self.delay_samples = ((SAMPLE_RATE * ms * 0.001) as usize)
            .clamp(1, self.buffer.len() - 1);
    }

    pub fn tick(&mut self, input: f32) -> f32 {
        let read_pos = (self.write_pos + self.buffer.len() - self.delay_samples) % self.buffer.len();
        let out = self.buffer[read_pos];
        self.buffer[self.write_pos] = input;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        out
    }

    pub fn clear(&mut self) {
        self.buffer.iter_mut().for_each(|x| *x = 0.0);
    }
}

/// Feedback delay (echo) effect.
#[derive(Clone, Debug)]
pub struct Echo {
    pub delay:    DelayLine,
    pub feedback: f32,  // [0, 1)
    pub wet:      f32,  // [0, 1]
}

impl Echo {
    pub fn new(delay_ms: f32, feedback: f32, wet: f32) -> Self {
        Self {
            delay: DelayLine::new(delay_ms + 50.0),
            feedback: feedback.clamp(0.0, 0.99),
            wet: wet.clamp(0.0, 1.0),
        }
    }

    pub fn tick(&mut self, input: f32) -> f32 {
        let delayed = self.delay.tick(input + self.delay.buffer[self.delay.write_pos] * self.feedback);
        input * (1.0 - self.wet) + delayed * self.wet
    }
}

/// Schroeder-style mono reverb using 4 comb + 2 allpass filters.
#[derive(Debug)]
pub struct Reverb {
    comb:    [CombFilter; 4],
    allpass: [AllpassFilter; 2],
    pub wet:    f32,
    pub room:   f32,  // [0, 1] — controls comb feedback
    pub damp:   f32,  // [0, 1] — high-frequency damping
}

impl Reverb {
    pub fn new() -> Self {
        let room = 0.5;
        let damp = 0.5;
        Self {
            comb: [
                CombFilter::new(1116, room, damp),
                CombFilter::new(1188, room, damp),
                CombFilter::new(1277, room, damp),
                CombFilter::new(1356, room, damp),
            ],
            allpass: [
                AllpassFilter::new(556, 0.5),
                AllpassFilter::new(441, 0.5),
            ],
            wet:  0.3,
            room,
            damp,
        }
    }

    pub fn set_room(&mut self, room: f32) {
        self.room = room.clamp(0.0, 1.0);
        for c in &mut self.comb { c.feedback = self.room * 0.9; }
    }

    pub fn set_damp(&mut self, damp: f32) {
        self.damp = damp.clamp(0.0, 1.0);
        for c in &mut self.comb { c.damp = self.damp; }
    }

    pub fn tick(&mut self, input: f32) -> f32 {
        let mut out = 0.0f32;
        for c in &mut self.comb {
            out += c.tick(input);
        }
        for a in &mut self.allpass {
            out = a.tick(out);
        }
        input * (1.0 - self.wet) + out * self.wet * 0.25
    }
}

#[derive(Debug)]
struct CombFilter {
    buffer:   Vec<f32>,
    pos:      usize,
    pub feedback: f32,
    pub damp:     f32,
    last:     f32,
}

impl CombFilter {
    fn new(delay_samples: usize, feedback: f32, damp: f32) -> Self {
        Self {
            buffer: vec![0.0; delay_samples],
            pos: 0,
            feedback,
            damp,
            last: 0.0,
        }
    }

    fn tick(&mut self, input: f32) -> f32 {
        let out = self.buffer[self.pos];
        self.last = out * (1.0 - self.damp) + self.last * self.damp;
        self.buffer[self.pos] = input + self.last * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        out
    }
}

#[derive(Debug)]
struct AllpassFilter {
    buffer:   Vec<f32>,
    pos:      usize,
    feedback: f32,
}

impl AllpassFilter {
    fn new(delay_samples: usize, feedback: f32) -> Self {
        Self { buffer: vec![0.0; delay_samples], pos: 0, feedback }
    }

    fn tick(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.pos];
        let output = -input + buffered;
        self.buffer[self.pos] = input + buffered * self.feedback;
        self.pos = (self.pos + 1) % self.buffer.len();
        output
    }
}

/// Soft-clipping saturator / waveshaper.
#[derive(Clone, Debug)]
pub struct Saturator {
    pub drive:  f32,  // [1, 10] — pre-gain
    pub output: f32,  // post-gain
}

impl Saturator {
    pub fn new(drive: f32) -> Self {
        Self { drive, output: 1.0 / drive.max(1.0) }
    }

    pub fn tick(&self, input: f32) -> f32 {
        let x = input * self.drive;
        // Cubic soft clip: tanh approximation
        let shaped = x / (1.0 + x.abs());
        shaped * self.output
    }
}

/// DC blocker (high-pass at ~10 Hz) to prevent offset accumulation.
#[derive(Clone, Debug, Default)]
pub struct DcBlocker {
    x_prev: f32,
    y_prev: f32,
}

impl DcBlocker {
    pub fn tick(&mut self, input: f32) -> f32 {
        let y = input - self.x_prev + 0.9975 * self.y_prev;
        self.x_prev = input;
        self.y_prev = y;
        y
    }
}

// ── Pitch utilities ───────────────────────────────────────────────────────────

/// Convert a MIDI note number to a frequency in Hz.
pub fn midi_to_hz(note: u8) -> f32 {
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

/// Convert frequency to nearest MIDI note number.
pub fn hz_to_midi(hz: f32) -> u8 {
    (69.0 + 12.0 * (hz / 440.0).log2()).round().clamp(0.0, 127.0) as u8
}

/// Detune a frequency by `cents` (100 cents = 1 semitone).
pub fn detune_cents(hz: f32, cents: f32) -> f32 {
    hz * 2.0f32.powf(cents / 1200.0)
}

/// Convert dB to linear gain.
pub fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

/// Convert linear gain to dB.
pub fn linear_to_db(gain: f32) -> f32 {
    20.0 * gain.abs().max(1e-10).log10()
}

// ── Chord utilities ───────────────────────────────────────────────────────────

/// Major scale intervals in semitones.
pub const MAJOR_SCALE:    [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Natural minor scale intervals.
pub const MINOR_SCALE:    [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
/// Pentatonic major intervals.
pub const PENTATONIC_MAJ: [i32; 5] = [0, 2, 4, 7, 9];
/// Pentatonic minor intervals.
pub const PENTATONIC_MIN: [i32; 5] = [0, 3, 5, 7, 10];
/// Whole tone scale.
pub const WHOLE_TONE:     [i32; 6] = [0, 2, 4, 6, 8, 10];
/// Diminished (half-whole) scale.
pub const DIMINISHED:     [i32; 8] = [0, 1, 3, 4, 6, 7, 9, 10];

/// Build a chord as frequencies given a root note (MIDI), scale intervals, and chord degrees.
pub fn chord_freqs(root_midi: u8, intervals: &[i32], degrees: &[usize]) -> Vec<f32> {
    degrees.iter()
        .filter_map(|&d| intervals.get(d))
        .map(|&semi| midi_to_hz((root_midi as i32 + semi).clamp(0, 127) as u8))
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adsr_attack_phase() {
        let adsr = Adsr::new(1.0, 0.5, 0.7, 0.5);
        assert!((adsr.level(0.5, None) - 0.5).abs() < 0.01);
        assert!((adsr.level(1.0, None) - 1.0).abs() < 0.01);
    }

    #[test]
    fn adsr_sustain_phase() {
        let adsr = Adsr::new(0.01, 0.01, 0.7, 0.5);
        assert!((adsr.level(0.1, None) - 0.7).abs() < 0.01);
    }

    #[test]
    fn adsr_release_decays_to_zero() {
        let adsr = Adsr::new(0.01, 0.01, 0.7, 1.0);
        let level_at_release = adsr.level(1.0, Some(0.1));
        assert!(level_at_release < 0.01);
    }

    #[test]
    fn oscillator_sine_at_zero_phase() {
        let s = oscillator(Waveform::Sine, 0.0);
        assert!((s - 0.0).abs() < 0.001);
    }

    #[test]
    fn oscillator_square_is_one_or_neg_one() {
        let s0 = oscillator(Waveform::Square, 0.25);
        let s1 = oscillator(Waveform::Square, 0.75);
        assert!((s0 - 1.0).abs() < 0.001);
        assert!((s1 + 1.0).abs() < 0.001);
    }

    #[test]
    fn biquad_low_pass_attenuates_high_freq() {
        let mut f = BiquadFilter::low_pass(1000.0, 0.707);
        // A 10kHz signal should be heavily attenuated
        let mut osc = Oscillator::sine(10_000.0);
        // Warm up
        for _ in 0..4800 { let s = osc.tick(); f.tick(s); }
        let rms: f32 = (0..480).map(|_| { let s = osc.tick(); f.tick(s).powi(2) }).sum::<f32>() / 480.0;
        let rms = rms.sqrt();
        // A 1kHz LPF should strongly attenuate a 10kHz signal
        assert!(rms < 0.5, "rms was {rms}");
    }

    #[test]
    fn midi_hz_roundtrip() {
        let hz = midi_to_hz(69);
        assert!((hz - 440.0).abs() < 0.01);
        assert_eq!(hz_to_midi(440.0), 69);
    }

    #[test]
    fn fm_voice_produces_output() {
        let mut voice = FmVoice::new(220.0, 2.0, 1.5);
        voice.note_on(220.0);
        let samples: Vec<f32> = (0..100).map(|_| voice.tick()).collect();
        let any_nonzero = samples.iter().any(|&s| s.abs() > 0.001);
        assert!(any_nonzero);
    }

    #[test]
    fn delay_line_delays_signal() {
        let mut dl = DelayLine::new(100.0);
        dl.set_delay_ms(10.0);
        let delay_samples = (SAMPLE_RATE * 0.01) as usize;
        // Push an impulse
        dl.tick(1.0);
        for _ in 1..delay_samples {
            let out = dl.tick(0.0);
            let _ = out;
        }
        let out = dl.tick(0.0);
        assert!(out.abs() > 0.5, "Expected delayed impulse, got {out}");
    }
}
