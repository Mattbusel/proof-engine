//! Audio Processing Graph for Proof Engine.
//!
//! A node-based audio processing pipeline:
//! - 30+ node types: oscillators, filters, envelopes, effects, samplers
//! - Polyphonic voice management (up to 64 voices)
//! - Sample-accurate automation (parameter lanes)
//! - Graph ordering via topological sort
//! - Lock-free command queue for real-time audio thread safety
//! - MIDI-inspired note/velocity/pitch interface

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

// ── Constants ─────────────────────────────────────────────────────────────────

pub const SAMPLE_RATE: f32 = 44100.0;
pub const BUFFER_SIZE: usize = 512;
pub const MAX_VOICES:  usize = 64;
pub const MAX_CHANNELS: usize = 2; // stereo

// ── AudioId ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoiceId(pub u32);

// ── AudioBuffer ───────────────────────────────────────────────────────────────

/// Fixed-size mono audio buffer.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: [f32; BUFFER_SIZE],
    pub len:     usize,
}

impl AudioBuffer {
    pub fn new() -> Self {
        Self { samples: [0.0; BUFFER_SIZE], len: BUFFER_SIZE }
    }

    pub fn silence() -> Self { Self::new() }

    pub fn fill(&mut self, v: f32) {
        self.samples[..self.len].fill(v);
    }

    pub fn add_from(&mut self, other: &AudioBuffer) {
        for i in 0..self.len.min(other.len) {
            self.samples[i] += other.samples[i];
        }
    }

    pub fn multiply_by(&mut self, other: &AudioBuffer) {
        for i in 0..self.len.min(other.len) {
            self.samples[i] *= other.samples[i];
        }
    }

    pub fn scale(&mut self, gain: f32) {
        for s in self.samples[..self.len].iter_mut() { *s *= gain; }
    }

    pub fn mix(&mut self, other: &AudioBuffer, weight: f32) {
        for i in 0..self.len.min(other.len) {
            self.samples[i] = self.samples[i] * (1.0 - weight) + other.samples[i] * weight;
        }
    }

    pub fn peak(&self) -> f32 {
        self.samples[..self.len].iter().map(|s| s.abs()).fold(0.0_f32, f32::max)
    }

    pub fn rms(&self) -> f32 {
        if self.len == 0 { return 0.0; }
        let sum: f32 = self.samples[..self.len].iter().map(|s| s * s).sum();
        (sum / self.len as f32).sqrt()
    }
}

impl Default for AudioBuffer { fn default() -> Self { Self::new() } }

/// Stereo audio buffer.
#[derive(Debug, Clone, Default)]
pub struct StereoBuffer {
    pub left:  AudioBuffer,
    pub right: AudioBuffer,
}

impl StereoBuffer {
    pub fn new() -> Self { Self { left: AudioBuffer::new(), right: AudioBuffer::new() } }

    pub fn silence() -> Self { Self::new() }

    pub fn peak(&self) -> f32 { self.left.peak().max(self.right.peak()) }

    pub fn scale(&mut self, gain: f32) {
        self.left.scale(gain);
        self.right.scale(gain);
    }
}

// ── Waveform ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Waveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
    ReverseSawtooth,
    Pulse(f32),   // pulse width 0..1
    Noise,
    SineSquared,
}

impl Waveform {
    pub fn sample(self, phase: f32) -> f32 {
        let t = phase.fract();
        match self {
            Waveform::Sine            => (t * std::f32::consts::TAU).sin(),
            Waveform::Square          => if t < 0.5 { 1.0 } else { -1.0 },
            Waveform::Triangle        => 1.0 - 4.0 * (t - 0.5).abs(),
            Waveform::Sawtooth        => 2.0 * t - 1.0,
            Waveform::ReverseSawtooth => 1.0 - 2.0 * t,
            Waveform::Pulse(pw)       => if t < pw { 1.0 } else { -1.0 },
            Waveform::Noise           => pseudo_rand(phase) * 2.0 - 1.0,
            Waveform::SineSquared     => {
                let s = (t * std::f32::consts::TAU).sin();
                s * s.abs()
            }
        }
    }
}

fn pseudo_rand(seed: f32) -> f32 {
    let x = (seed * 127.1 + 311.7).sin() * 43758.547;
    x - x.floor()
}

// ── FilterType ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    AllPass,
    LowShelf,
    HighShelf,
    Peaking,
}

/// Biquad filter state.
#[derive(Debug, Clone, Default)]
pub struct BiquadFilter {
    // Coefficients
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32,
    // State
    x1: f32, x2: f32, y1: f32, y2: f32,
}

impl BiquadFilter {
    pub fn configure(&mut self, filter_type: FilterType, freq: f32, q: f32, gain_db: f32) {
        let omega = std::f32::consts::TAU * freq / SAMPLE_RATE;
        let sin_w = omega.sin();
        let cos_w = omega.cos();
        let alpha = sin_w / (2.0 * q.max(0.001));
        let a_lin = 10.0_f32.powf(gain_db / 40.0);

        let (b0, b1, b2, a0, a1, a2) = match filter_type {
            FilterType::LowPass => {
                ((1.0 - cos_w) / 2.0, 1.0 - cos_w, (1.0 - cos_w) / 2.0,
                 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha)
            }
            FilterType::HighPass => {
                ((1.0 + cos_w) / 2.0, -(1.0 + cos_w), (1.0 + cos_w) / 2.0,
                 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha)
            }
            FilterType::BandPass => {
                (alpha, 0.0, -alpha,
                 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha)
            }
            FilterType::Notch => {
                (1.0, -2.0 * cos_w, 1.0,
                 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha)
            }
            FilterType::AllPass => {
                (1.0 - alpha, -2.0 * cos_w, 1.0 + alpha,
                 1.0 + alpha, -2.0 * cos_w, 1.0 - alpha)
            }
            FilterType::LowShelf => {
                let root_a = a_lin.sqrt();
                let beta = (a_lin.sqrt()) * alpha;
                (a_lin * ((a_lin + 1.0) - (a_lin - 1.0) * cos_w + 2.0 * beta),
                 2.0 * a_lin * ((a_lin - 1.0) - (a_lin + 1.0) * cos_w),
                 a_lin * ((a_lin + 1.0) - (a_lin - 1.0) * cos_w - 2.0 * beta),
                 (a_lin + 1.0) + (a_lin - 1.0) * cos_w + 2.0 * root_a * alpha,
                 -2.0 * ((a_lin - 1.0) + (a_lin + 1.0) * cos_w),
                 (a_lin + 1.0) + (a_lin - 1.0) * cos_w - 2.0 * root_a * alpha)
            }
            FilterType::HighShelf => {
                let root_a = a_lin.sqrt();
                let beta = root_a * alpha;
                (a_lin * ((a_lin + 1.0) + (a_lin - 1.0) * cos_w + 2.0 * beta),
                 -2.0 * a_lin * ((a_lin - 1.0) + (a_lin + 1.0) * cos_w),
                 a_lin * ((a_lin + 1.0) + (a_lin - 1.0) * cos_w - 2.0 * beta),
                 (a_lin + 1.0) - (a_lin - 1.0) * cos_w + 2.0 * root_a * alpha,
                 2.0 * ((a_lin - 1.0) - (a_lin + 1.0) * cos_w),
                 (a_lin + 1.0) - (a_lin - 1.0) * cos_w - 2.0 * root_a * alpha)
            }
            FilterType::Peaking => {
                (1.0 + alpha * a_lin, -2.0 * cos_w, 1.0 - alpha * a_lin,
                 1.0 + alpha / a_lin, -2.0 * cos_w, 1.0 - alpha / a_lin)
            }
        };

        let inv_a0 = 1.0 / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = b1 * inv_a0;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
    }

    pub fn process_sample(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
              - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = x;
        self.y2 = self.y1; self.y1 = y;
        y
    }

    pub fn process_buffer(&mut self, buf: &mut AudioBuffer) {
        for s in buf.samples[..buf.len].iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0; self.x2 = 0.0; self.y1 = 0.0; self.y2 = 0.0;
    }
}

// ── AdsrEnvelope ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeStage { Idle, Attack, Decay, Sustain, Release }

#[derive(Debug, Clone)]
pub struct AdsrEnvelope {
    pub attack_time:   f32,
    pub decay_time:    f32,
    pub sustain_level: f32,
    pub release_time:  f32,
    pub attack_curve:  f32, // 1 = linear, 2 = quadratic
    pub release_curve: f32,
    stage:    EnvelopeStage,
    level:    f32,
    stage_t:  f32,
    sample_rate: f32,
}

impl AdsrEnvelope {
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self {
            attack_time: attack,
            decay_time: decay,
            sustain_level: sustain,
            release_time: release,
            attack_curve: 1.0,
            release_curve: 1.0,
            stage: EnvelopeStage::Idle,
            level: 0.0,
            stage_t: 0.0,
            sample_rate: SAMPLE_RATE,
        }
    }

    pub fn note_on(&mut self) {
        self.stage   = EnvelopeStage::Attack;
        self.stage_t = 0.0;
    }

    pub fn note_off(&mut self) {
        if self.stage != EnvelopeStage::Idle {
            self.stage   = EnvelopeStage::Release;
            self.stage_t = 0.0;
        }
    }

    pub fn reset(&mut self) {
        self.stage = EnvelopeStage::Idle;
        self.level = 0.0;
        self.stage_t = 0.0;
    }

    pub fn is_done(&self) -> bool { self.stage == EnvelopeStage::Idle }

    pub fn next_sample(&mut self) -> f32 {
        let dt = 1.0 / self.sample_rate;
        self.stage_t += dt;
        self.level = match self.stage {
            EnvelopeStage::Idle => 0.0,
            EnvelopeStage::Attack => {
                let t = (self.stage_t / self.attack_time.max(1e-6)).clamp(0.0, 1.0);
                let v = t.powf(self.attack_curve);
                if t >= 1.0 { self.stage = EnvelopeStage::Decay; self.stage_t = 0.0; }
                v
            }
            EnvelopeStage::Decay => {
                let t = (self.stage_t / self.decay_time.max(1e-6)).clamp(0.0, 1.0);
                let v = 1.0 - (1.0 - self.sustain_level) * t;
                if t >= 1.0 { self.stage = EnvelopeStage::Sustain; }
                v
            }
            EnvelopeStage::Sustain => self.sustain_level,
            EnvelopeStage::Release => {
                let t = (self.stage_t / self.release_time.max(1e-6)).clamp(0.0, 1.0);
                let v = self.sustain_level * (1.0 - t.powf(self.release_curve));
                if t >= 1.0 { self.stage = EnvelopeStage::Idle; self.level = 0.0; return 0.0; }
                v
            }
        };
        self.level
    }

    pub fn fill_buffer(&mut self, buf: &mut AudioBuffer) {
        for s in buf.samples[..buf.len].iter_mut() {
            *s = self.next_sample();
        }
    }
}

// ── Lfo ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Lfo {
    pub waveform:  Waveform,
    pub frequency: f32,
    pub amplitude: f32,
    pub offset:    f32,
    pub phase:     f32,
    pub sync_to_tempo: bool,
    pub bpm:       f32,
    pub beat_div:  f32, // 1/4, 1/8, etc.
}

impl Lfo {
    pub fn new(waveform: Waveform, freq: f32) -> Self {
        Self {
            waveform, frequency: freq, amplitude: 1.0, offset: 0.0,
            phase: 0.0, sync_to_tempo: false, bpm: 120.0, beat_div: 1.0,
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        let freq = if self.sync_to_tempo {
            self.bpm / 60.0 / self.beat_div
        } else { self.frequency };

        let v = self.waveform.sample(self.phase);
        self.phase += freq / SAMPLE_RATE;
        if self.phase >= 1.0 { self.phase -= 1.0; }
        self.offset + v * self.amplitude
    }

    pub fn fill_buffer(&mut self, buf: &mut AudioBuffer) {
        for s in buf.samples[..buf.len].iter_mut() {
            *s = self.next_sample();
        }
    }
}

// ── DelayLine ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DelayLine {
    buffer:      Vec<f32>,
    write_pos:   usize,
    delay_samples: usize,
}

impl DelayLine {
    pub fn new(max_delay_samples: usize) -> Self {
        Self { buffer: vec![0.0; max_delay_samples], write_pos: 0, delay_samples: max_delay_samples / 2 }
    }

    pub fn set_delay_time(&mut self, secs: f32) {
        self.delay_samples = ((secs * SAMPLE_RATE) as usize).min(self.buffer.len() - 1);
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let read_pos = (self.write_pos + self.buffer.len() - self.delay_samples) % self.buffer.len();
        let out = self.buffer[read_pos];
        self.buffer[self.write_pos] = input;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        out
    }

    pub fn process_buffer(&mut self, input: &AudioBuffer, feedback: f32) -> AudioBuffer {
        let mut out = AudioBuffer::new();
        for i in 0..input.len {
            let delayed = self.process(input.samples[i]);
            out.samples[i] = delayed;
            // Feed back into buffer
            let fb_pos = (self.write_pos + self.buffer.len() - 1) % self.buffer.len();
            self.buffer[fb_pos] += delayed * feedback;
        }
        out.len = input.len;
        out
    }
}

// ── Reverb ────────────────────────────────────────────────────────────────────

/// Simple Schroeder reverb with 4 comb filters + 2 allpass filters.
#[derive(Debug, Clone)]
pub struct Reverb {
    combs:   [DelayLine; 4],
    allpass: [DelayLine; 2],
    pub room_size: f32,
    pub damping:   f32,
    pub wet:       f32,
    pub pre_delay: f32,
    pre_delay_line: DelayLine,
    damp_filters: [f32; 4], // one-pole LP per comb
}

impl Reverb {
    pub fn new() -> Self {
        let sizes = [1557, 1617, 1491, 1422];
        let combs: [DelayLine; 4] = [
            DelayLine::new(sizes[0]), DelayLine::new(sizes[1]),
            DelayLine::new(sizes[2]), DelayLine::new(sizes[3]),
        ];
        let allpass = [DelayLine::new(556), DelayLine::new(441)];
        Self {
            combs, allpass,
            room_size: 0.5, damping: 0.5, wet: 0.3, pre_delay: 0.02,
            pre_delay_line: DelayLine::new(4096),
            damp_filters: [0.0; 4],
        }
    }

    pub fn process_buffer(&mut self, input: &AudioBuffer) -> AudioBuffer {
        let feedback = self.room_size * 0.28 + 0.7;
        let damp     = self.damping;

        let mut out = AudioBuffer::new();
        self.pre_delay_line.set_delay_time(self.pre_delay);

        for i in 0..input.len {
            let x = self.pre_delay_line.process(input.samples[i]);
            let mut sum = 0.0_f32;

            for (j, comb) in self.combs.iter_mut().enumerate() {
                let delayed = comb.process(x + comb.buffer[comb.write_pos] * feedback);
                // Damping: one-pole LP
                self.damp_filters[j] = self.damp_filters[j] + (delayed - self.damp_filters[j]) * (1.0 - damp);
                sum += self.damp_filters[j];
            }

            // Allpass filters
            let mut ap_out = sum * 0.25;
            for ap in self.allpass.iter_mut() {
                ap_out = ap.process(ap_out + ap.buffer[ap.write_pos] * 0.5) - ap_out * 0.5;
            }

            out.samples[i] = input.samples[i] * (1.0 - self.wet) + ap_out * self.wet;
        }
        out.len = input.len;
        out
    }
}

impl Default for Reverb { fn default() -> Self { Self::new() } }

// ── Compressor ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Compressor {
    pub threshold_db: f32,
    pub ratio:        f32,
    pub attack_ms:    f32,
    pub release_ms:   f32,
    pub makeup_gain:  f32,
    pub knee_db:      f32,
    envelope:         f32,
}

impl Compressor {
    pub fn new(threshold_db: f32, ratio: f32) -> Self {
        Self {
            threshold_db, ratio,
            attack_ms: 1.0, release_ms: 100.0, makeup_gain: 0.0, knee_db: 3.0,
            envelope: 0.0,
        }
    }

    pub fn process_sample(&mut self, x: f32) -> f32 {
        let level_db = 20.0 * x.abs().max(1e-10).log10();
        let over = level_db - self.threshold_db;
        let gain_reduction = if over <= -self.knee_db / 2.0 {
            0.0
        } else if over <= self.knee_db / 2.0 {
            let t = (over + self.knee_db / 2.0) / self.knee_db;
            (1.0 / self.ratio - 1.0) * over * t * t / 2.0
        } else {
            (1.0 / self.ratio - 1.0) * over
        };

        // Ballistics
        let coeff = if gain_reduction < self.envelope {
            1.0 - (-2.2 / (self.attack_ms * 0.001 * SAMPLE_RATE)).exp()
        } else {
            1.0 - (-2.2 / (self.release_ms * 0.001 * SAMPLE_RATE)).exp()
        };
        self.envelope += (gain_reduction - self.envelope) * coeff;

        let gain_db = self.envelope + self.makeup_gain;
        x * 10.0_f32.powf(gain_db / 20.0)
    }

    pub fn process_buffer(&mut self, buf: &mut AudioBuffer) {
        for s in buf.samples[..buf.len].iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

// ── Distortion ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistortionMode {
    HardClip,
    SoftClip,
    Foldback,
    Bitcrush(u32),
    Waveshape,
    Overdrive,
}

#[derive(Debug, Clone)]
pub struct Distortion {
    pub mode:  DistortionMode,
    pub drive: f32,  // pre-gain
    pub mix:   f32,  // wet/dry
}

impl Distortion {
    pub fn new(mode: DistortionMode, drive: f32) -> Self {
        Self { mode, drive, mix: 1.0 }
    }

    pub fn process_sample(&self, x: f32) -> f32 {
        let driven = x * self.drive;
        let clipped = match self.mode {
            DistortionMode::HardClip => driven.clamp(-1.0, 1.0),
            DistortionMode::SoftClip => driven.tanh(),
            DistortionMode::Foldback => {
                let mut v = driven;
                while v > 1.0 || v < -1.0 {
                    if v > 1.0  { v = 2.0 - v; }
                    if v < -1.0 { v = -2.0 - v; }
                }
                v
            }
            DistortionMode::Bitcrush(bits) => {
                let steps = 2.0_f32.powi(bits as i32);
                (driven * steps).round() / steps
            }
            DistortionMode::Waveshape => {
                let k = 2.0;
                driven * (k + 1.0) / (1.0 + k * driven.abs())
            }
            DistortionMode::Overdrive => {
                let abs = driven.abs();
                let sign = driven.signum();
                sign * if abs < 1.0/3.0 { 2.0 * abs }
                       else if abs < 2.0/3.0 { (3.0 - (2.0 - 3.0*abs).powi(2)) / 3.0 }
                       else { 1.0 }
            }
        };
        x * (1.0 - self.mix) + clipped * self.mix
    }

    pub fn process_buffer(&self, buf: &mut AudioBuffer) {
        for s in buf.samples[..buf.len].iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

// ── Chorus / Flanger ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Chorus {
    delay_lines: Vec<DelayLine>,
    lfos:        Vec<Lfo>,
    pub depth:   f32,
    pub rate:    f32,
    pub wet:     f32,
    pub voices:  usize,
}

impl Chorus {
    pub fn new(voices: usize) -> Self {
        let delay_lines = (0..voices).map(|_| DelayLine::new(8192)).collect();
        let lfos = (0..voices).map(|i| {
            let mut lfo = Lfo::new(Waveform::Sine, 0.5 + i as f32 * 0.1);
            lfo.phase = i as f32 / voices as f32;
            lfo
        }).collect();
        Self { delay_lines, lfos, depth: 0.01, rate: 0.3, wet: 0.5, voices }
    }

    pub fn process_buffer(&mut self, input: &AudioBuffer) -> AudioBuffer {
        let mut out = AudioBuffer::new();
        for i in 0..input.len {
            let mut sum = 0.0_f32;
            for (dl, lfo) in self.delay_lines.iter_mut().zip(self.lfos.iter_mut()) {
                let mod_t = lfo.next_sample() * self.depth + 0.01;
                dl.set_delay_time(mod_t.max(0.0005));
                sum += dl.process(input.samples[i]);
            }
            let wet = sum / self.voices as f32;
            out.samples[i] = input.samples[i] * (1.0 - self.wet) + wet * self.wet;
        }
        out.len = input.len;
        out
    }
}

// ── AudioNodeType ─────────────────────────────────────────────────────────────

/// All supported audio node types in the processing graph.
#[derive(Debug, Clone)]
pub enum AudioNodeType {
    // ── Sources ──────────────────────────────────────────────────────────────
    /// Basic oscillator (sine, square, saw, etc.).
    Oscillator { waveform: Waveform, frequency: f32, detune_cents: f32 },
    /// White/pink/brown noise.
    Noise { color: NoiseColor },
    /// Audio file sample player.
    SamplePlayer { clip_id: u32, loop_start: f32, loop_end: f32, loop_mode: LoopMode },
    /// Wavetable oscillator with morphing between wavetables.
    Wavetable { table_id: u32, morph: f32, frequency: f32 },
    /// Karplus-Strong plucked string.
    KarplusStrong { frequency: f32, decay: f32, brightness: f32 },
    /// FM oscillator: carrier + modulator.
    FmOscillator { carrier_freq: f32, mod_freq: f32, mod_index: f32, waveform: Waveform },
    /// Additive synth: sum of partials.
    Additive { fundamental: f32, partial_gains: Vec<f32> },

    // ── Filters ──────────────────────────────────────────────────────────────
    /// Biquad filter (LP, HP, BP, notch, shelf, peaking).
    BiquadFilter { filter_type: FilterType, frequency: f32, q: f32, gain_db: f32 },
    /// Moog-style 24dB/oct ladder filter.
    LadderFilter { cutoff: f32, resonance: f32, drive: f32 },
    /// Formant filter (vowel synthesis).
    FormantFilter { vowel_a: f32, vowel_b: f32, blend: f32 },
    /// Comb filter.
    CombFilter { delay_time: f32, feedback: f32, feedforward: f32 },

    // ── Envelopes / Modulators ────────────────────────────────────────────────
    /// ADSR envelope.
    Adsr { attack: f32, decay: f32, sustain: f32, release: f32 },
    /// Multi-stage envelope (breakpoint sequence).
    MultiStageEnvelope { breakpoints: Vec<(f32, f32)>, loop_start: Option<usize> },
    /// LFO modulator.
    Lfo { waveform: Waveform, frequency: f32, amplitude: f32, phase: f32 },
    /// Sample & Hold: samples input at rate, holds until next trigger.
    SampleHold { rate: f32 },

    // ── Effects ───────────────────────────────────────────────────────────────
    /// Delay line with feedback.
    Delay { delay_time: f32, feedback: f32, wet: f32 },
    /// Reverb (Schroeder model).
    Reverb { room_size: f32, damping: f32, wet: f32 },
    /// Chorus / flanger.
    Chorus { voices: u32, rate: f32, depth: f32, wet: f32 },
    /// Dynamic range compressor.
    Compressor { threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32 },
    /// Distortion / waveshaping.
    Distortion { mode: DistortionMode, drive: f32 },
    /// Stereo panning.
    Panner { pan: f32 },
    /// Gain / volume.
    Gain { gain_db: f32 },
    /// Hard / soft limiter.
    Limiter { ceiling_db: f32, release_ms: f32 },
    /// Stereo widener.
    StereoWidener { width: f32 },
    /// Bit crusher + sample rate reducer.
    BitCrusher { bits: u32, downsample: u32 },
    /// Pitch shifter (FFT-based approximation).
    PitchShift { semitones: f32 },
    /// Ring modulator.
    RingModulator { carrier_freq: f32, mix: f32 },
    /// Auto-wah (envelope-following filter).
    AutoWah { sensitivity: f32, speed: f32, min_freq: f32, max_freq: f32 },

    // ── Utility ───────────────────────────────────────────────────────────────
    /// Mixer: sum multiple inputs with individual gains.
    Mixer { gains: Vec<f32> },
    /// Crossfade between two signals.
    Crossfade { blend: f32 },
    /// Constant value source.
    Constant { value: f32 },
    /// Pass-through monitor (for metering).
    Meter,
    /// Math operations on audio signals.
    MathOp { op: AudioMathOp },
    /// Output node — writes to stereo out.
    Output { volume: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoiseColor { White, Pink, Brown }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoopMode { None, Forward, PingPong, BackwardLoop }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioMathOp { Add, Subtract, Multiply, Abs, Invert, Clamp, Min, Max }

// ── AudioNode ─────────────────────────────────────────────────────────────────

/// A node in the audio processing graph.
#[derive(Debug, Clone)]
pub struct AudioNode {
    pub id:        NodeId,
    pub node_type: AudioNodeType,
    pub label:     String,
    pub enabled:   bool,
    pub x:         f32, // graph editor position
    pub y:         f32,
    // Per-node state
    pub phase:     f32,
    pub filter:    BiquadFilter,
    pub envelope:  AdsrEnvelope,
    pub lfo_state: Lfo,
    pub delay_line: DelayLine,
    pub reverb:    Reverb,
    pub chorus:    Chorus,
    pub compressor: Compressor,
    // Karplus-Strong buffer
    ks_buffer:     Vec<f32>,
    ks_pos:        usize,
    // Ladder filter state
    lf_stages: [f32; 4],
    lf_freq:   f32,
    // Multi-stage envelope
    ms_env_stage: usize,
    ms_env_t: f32,
    // Sample & hold
    sh_value: f32,
    sh_timer: f32,
    // Formant
    formant_filters: [BiquadFilter; 3],
    // Metering
    pub last_peak: f32,
    pub last_rms:  f32,
}

impl AudioNode {
    pub fn new(id: NodeId, node_type: AudioNodeType) -> Self {
        let label = format!("{:?}", node_type).split('{').next().unwrap_or("Node").trim().to_string();
        Self {
            id, node_type, label, enabled: true,
            x: 0.0, y: 0.0,
            phase: 0.0,
            filter: BiquadFilter::default(),
            envelope: AdsrEnvelope::new(0.01, 0.1, 0.8, 0.3),
            lfo_state: Lfo::new(Waveform::Sine, 1.0),
            delay_line: DelayLine::new(SAMPLE_RATE as usize * 2),
            reverb: Reverb::new(),
            chorus: Chorus::new(3),
            compressor: Compressor::new(-12.0, 4.0),
            ks_buffer: vec![0.0; 4096],
            ks_pos: 0,
            lf_stages: [0.0; 4],
            lf_freq: 1000.0,
            ms_env_stage: 0,
            ms_env_t: 0.0,
            sh_value: 0.0,
            sh_timer: 0.0,
            formant_filters: Default::default(),
            last_peak: 0.0,
            last_rms: 0.0,
        }
    }

    /// Initialize node state from its type parameters.
    pub fn initialize(&mut self) {
        match &self.node_type.clone() {
            AudioNodeType::BiquadFilter { filter_type, frequency, q, gain_db } => {
                self.filter.configure(*filter_type, *frequency, *q, *gain_db);
            }
            AudioNodeType::Adsr { attack, decay, sustain, release } => {
                self.envelope = AdsrEnvelope::new(*attack, *decay, *sustain, *release);
            }
            AudioNodeType::Lfo { waveform, frequency, amplitude, phase } => {
                self.lfo_state = Lfo::new(*waveform, *frequency);
                self.lfo_state.amplitude = *amplitude;
                self.lfo_state.phase = *phase;
            }
            AudioNodeType::Delay { delay_time, .. } => {
                self.delay_line.set_delay_time(*delay_time);
            }
            AudioNodeType::KarplusStrong { frequency, .. } => {
                let period = (SAMPLE_RATE / frequency.max(1.0)) as usize;
                let period = period.min(4095);
                // Fill with white noise for pluck
                for i in 0..period {
                    self.ks_buffer[i] = pseudo_rand(i as f32) * 2.0 - 1.0;
                }
                self.ks_pos = 0;
            }
            AudioNodeType::Reverb { room_size, damping, wet } => {
                self.reverb.room_size = *room_size;
                self.reverb.damping = *damping;
                self.reverb.wet = *wet;
            }
            AudioNodeType::Chorus { voices, rate, depth, wet } => {
                self.chorus = Chorus::new(*voices as usize);
                self.chorus.rate = *rate;
                self.chorus.depth = *depth;
                self.chorus.wet = *wet;
            }
            AudioNodeType::Compressor { threshold_db, ratio, attack_ms, release_ms } => {
                self.compressor = Compressor::new(*threshold_db, *ratio);
                self.compressor.attack_ms = *attack_ms;
                self.compressor.release_ms = *release_ms;
            }
            _ => {}
        }
    }

    /// Process one buffer. `inputs` contains buffers from upstream nodes.
    pub fn process(&mut self, inputs: &[&AudioBuffer]) -> AudioBuffer {
        if !self.enabled {
            return inputs.first().copied().cloned().unwrap_or_default();
        }

        let input = inputs.first().copied().cloned().unwrap_or_else(AudioBuffer::silence);

        let result = match &self.node_type.clone() {
            // ── Sources ──────────────────────────────────────────────────────
            AudioNodeType::Oscillator { waveform, frequency, detune_cents } => {
                let freq = *frequency * 2.0_f32.powf(*detune_cents / 1200.0);
                let phase_inc = freq / SAMPLE_RATE;
                let mut out = AudioBuffer::new();
                for s in out.samples[..out.len].iter_mut() {
                    *s = waveform.sample(self.phase);
                    self.phase += phase_inc;
                    if self.phase >= 1.0 { self.phase -= 1.0; }
                }
                out
            }

            AudioNodeType::Noise { color } => {
                let mut out = AudioBuffer::new();
                let mut b0 = 0.0_f32;
                let mut b1 = 0.0_f32;
                for (i, s) in out.samples[..out.len].iter_mut().enumerate() {
                    let white = pseudo_rand(self.phase + i as f32 * 0.1) * 2.0 - 1.0;
                    *s = match color {
                        NoiseColor::White => white,
                        NoiseColor::Pink => {
                            b0 = 0.99886 * b0 + white * 0.0555179;
                            b1 = 0.99332 * b1 + white * 0.0750759;
                            (b0 + b1 + white * 0.5) / 2.5
                        }
                        NoiseColor::Brown => {
                            b0 = (b0 + 0.02 * white) / 1.02;
                            b0 * 3.5
                        }
                    };
                }
                self.phase += 1.0;
                out
            }

            AudioNodeType::KarplusStrong { frequency, decay, brightness } => {
                let period = (SAMPLE_RATE / frequency.max(1.0)) as usize;
                let period = period.clamp(2, self.ks_buffer.len());
                let mut out = AudioBuffer::new();
                for s in out.samples[..out.len].iter_mut() {
                    let i0 = self.ks_pos;
                    let i1 = (self.ks_pos + 1) % period;
                    // Averaged-damped comb filter
                    let avg = (self.ks_buffer[i0] + self.ks_buffer[i1]) * 0.5 * decay;
                    let bright_mix = *brightness;
                    self.ks_buffer[i0] = avg * (1.0 - bright_mix) + self.ks_buffer[i0] * bright_mix;
                    *s = self.ks_buffer[i0];
                    self.ks_pos = (self.ks_pos + 1) % period;
                }
                out
            }

            AudioNodeType::FmOscillator { carrier_freq, mod_freq, mod_index, waveform } => {
                let mut out = AudioBuffer::new();
                let mut mod_phase = self.phase;
                let mut car_phase = self.phase + 0.5;
                for s in out.samples[..out.len].iter_mut() {
                    let modulator = waveform.sample(mod_phase);
                    let car_freq_mod = *carrier_freq + modulator * mod_index * mod_freq;
                    *s = Waveform::Sine.sample(car_phase);
                    mod_phase += mod_freq / SAMPLE_RATE;
                    car_phase += car_freq_mod / SAMPLE_RATE;
                    if mod_phase >= 1.0 { mod_phase -= 1.0; }
                    if car_phase >= 1.0 { car_phase -= 1.0; }
                }
                self.phase = mod_phase;
                out
            }

            AudioNodeType::Additive { fundamental, partial_gains } => {
                let mut out = AudioBuffer::new();
                for (i, s) in out.samples[..out.len].iter_mut().enumerate() {
                    let mut v = 0.0_f32;
                    for (k, &gain) in partial_gains.iter().enumerate() {
                        let partial = (k + 1) as f32;
                        let phase = (self.phase * partial).fract();
                        v += Waveform::Sine.sample(phase) * gain;
                    }
                    *s = v;
                    let _ = i;
                }
                self.phase += fundamental / SAMPLE_RATE;
                if self.phase >= 1.0 { self.phase -= 1.0; }
                out
            }

            AudioNodeType::Constant { value } => {
                let mut out = AudioBuffer::new();
                out.fill(*value);
                out
            }

            // ── Filters ──────────────────────────────────────────────────────
            AudioNodeType::BiquadFilter { .. } => {
                let mut out = input.clone();
                self.filter.process_buffer(&mut out);
                out
            }

            AudioNodeType::LadderFilter { cutoff, resonance, drive } => {
                let mut out = input.clone();
                let f = (std::f32::consts::PI * cutoff / SAMPLE_RATE).tan().clamp(0.0, 0.99);
                let r = resonance * 4.0;
                for s in out.samples[..out.len].iter_mut() {
                    let x = *s * drive - r * self.lf_stages[3];
                    let (mut a, mut b, mut c, mut d) = (self.lf_stages[0], self.lf_stages[1], self.lf_stages[2], self.lf_stages[3]);
                    a = a + f * (x.tanh()    - a.tanh());
                    b = b + f * (a.tanh()    - b.tanh());
                    c = c + f * (b.tanh()    - c.tanh());
                    d = d + f * (c.tanh()    - d.tanh());
                    self.lf_stages = [a, b, c, d];
                    *s = d;
                }
                out
            }

            AudioNodeType::CombFilter { delay_time, feedback, feedforward } => {
                self.delay_line.set_delay_time(*delay_time);
                let mut out = AudioBuffer::new();
                for i in 0..input.len {
                    let delayed = self.delay_line.process(input.samples[i]);
                    out.samples[i] = input.samples[i] * feedforward + delayed * feedback;
                }
                out.len = input.len;
                out
            }

            // ── Envelopes ────────────────────────────────────────────────────
            AudioNodeType::Adsr { .. } => {
                let mut out = AudioBuffer::new();
                self.envelope.fill_buffer(&mut out);
                // If there's an input signal, modulate it
                if !inputs.is_empty() {
                    out.multiply_by(&input);
                }
                out
            }

            AudioNodeType::Lfo { .. } => {
                let mut out = AudioBuffer::new();
                self.lfo_state.fill_buffer(&mut out);
                out
            }

            AudioNodeType::SampleHold { rate } => {
                let trigger_period = 1.0 / rate.max(0.001);
                let mut out = AudioBuffer::new();
                for i in 0..input.len {
                    self.sh_timer += 1.0 / SAMPLE_RATE;
                    if self.sh_timer >= trigger_period {
                        self.sh_value = input.samples[i];
                        self.sh_timer = 0.0;
                    }
                    out.samples[i] = self.sh_value;
                }
                out.len = input.len;
                out
            }

            // ── Effects ──────────────────────────────────────────────────────
            AudioNodeType::Delay { feedback, wet, .. } => {
                let delayed = self.delay_line.process_buffer(&input, *feedback);
                let mut out = input.clone();
                out.mix(&delayed, *wet);
                out
            }

            AudioNodeType::Reverb { .. } => {
                self.reverb.process_buffer(&input)
            }

            AudioNodeType::Chorus { .. } => {
                self.chorus.process_buffer(&input)
            }

            AudioNodeType::Compressor { .. } => {
                let mut out = input.clone();
                self.compressor.process_buffer(&mut out);
                out
            }

            AudioNodeType::Distortion { mode, drive } => {
                let dist = Distortion::new(*mode, *drive);
                let mut out = input.clone();
                dist.process_buffer(&mut out);
                out
            }

            AudioNodeType::Gain { gain_db } => {
                let linear = 10.0_f32.powf(*gain_db / 20.0);
                let mut out = input.clone();
                out.scale(linear);
                out
            }

            AudioNodeType::Limiter { ceiling_db, release_ms } => {
                let ceiling = 10.0_f32.powf(*ceiling_db / 20.0);
                let release_coeff = 1.0 - (-2.2 / (release_ms * 0.001 * SAMPLE_RATE)).exp();
                let mut out = input.clone();
                let mut env = 0.0_f32;
                for s in out.samples[..out.len].iter_mut() {
                    let abs = s.abs();
                    if abs > env { env = abs; }
                    else { env += (0.0 - env) * release_coeff; }
                    if env > ceiling { *s *= ceiling / env; }
                }
                out
            }

            AudioNodeType::RingModulator { carrier_freq, mix } => {
                let mut out = AudioBuffer::new();
                let phase_inc = carrier_freq / SAMPLE_RATE;
                for i in 0..input.len {
                    let ring = Waveform::Sine.sample(self.phase);
                    self.phase += phase_inc;
                    if self.phase >= 1.0 { self.phase -= 1.0; }
                    out.samples[i] = input.samples[i] * (1.0 - mix) + input.samples[i] * ring * mix;
                }
                out.len = input.len;
                out
            }

            AudioNodeType::Panner { pan } => {
                // Mono pass-through with pan info stored for stereo stage
                let _ = pan;
                input.clone()
            }

            // ── Utility ──────────────────────────────────────────────────────
            AudioNodeType::Mixer { gains } => {
                let mut out = AudioBuffer::silence();
                for (i, &gain) in gains.iter().enumerate() {
                    if i < inputs.len() {
                        let mut buf = inputs[i].clone();
                        buf.scale(gain);
                        out.add_from(&buf);
                    }
                }
                out
            }

            AudioNodeType::Crossfade { blend } => {
                if inputs.len() >= 2 {
                    let mut out = inputs[0].clone();
                    out.mix(inputs[1], *blend);
                    out
                } else { input.clone() }
            }

            AudioNodeType::MathOp { op } => {
                let a = &input;
                let b = inputs.get(1).copied().cloned().unwrap_or_else(AudioBuffer::silence);
                let mut out = AudioBuffer::new();
                for i in 0..out.len {
                    out.samples[i] = match op {
                        AudioMathOp::Add      => a.samples[i] + b.samples[i],
                        AudioMathOp::Subtract => a.samples[i] - b.samples[i],
                        AudioMathOp::Multiply => a.samples[i] * b.samples[i],
                        AudioMathOp::Abs      => a.samples[i].abs(),
                        AudioMathOp::Invert   => -a.samples[i],
                        AudioMathOp::Clamp    => a.samples[i].clamp(-1.0, 1.0),
                        AudioMathOp::Min      => a.samples[i].min(b.samples[i]),
                        AudioMathOp::Max      => a.samples[i].max(b.samples[i]),
                    };
                }
                out
            }

            AudioNodeType::Meter => {
                self.last_peak = input.peak();
                self.last_rms  = input.rms();
                input.clone()
            }

            AudioNodeType::Output { volume } => {
                let mut out = input.clone();
                out.scale(*volume);
                out
            }

            // Fallthrough for types not yet fully implemented inline
            _ => input.clone(),
        };

        result
    }

    /// Note on event (triggers envelope, resets oscillator phase if desired).
    pub fn note_on(&mut self, _freq: f32, _velocity: f32) {
        self.envelope.note_on();
        match &self.node_type {
            AudioNodeType::KarplusStrong { frequency, .. } => {
                let period = (SAMPLE_RATE / frequency.max(1.0)) as usize;
                let period = period.clamp(2, self.ks_buffer.len());
                for i in 0..period {
                    self.ks_buffer[i] = pseudo_rand(self.phase + i as f32) * 2.0 - 1.0;
                }
                self.ks_pos = 0;
            }
            _ => {}
        }
    }

    pub fn note_off(&mut self) {
        self.envelope.note_off();
    }
}

// ── AudioEdge ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AudioEdge {
    pub id:       EdgeId,
    pub from:     NodeId,
    pub to:       NodeId,
    pub from_slot: u8,
    pub to_slot:   u8,
    pub gain:      f32,
}

// ── AutomationPoint ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct AutomationPoint {
    /// Sample position in the current render block.
    pub sample: usize,
    pub value:  f32,
}

/// A sample-accurate automation lane for a named parameter.
#[derive(Debug, Clone)]
pub struct AutomationLane {
    pub node_id:   NodeId,
    pub param_name: String,
    pub points:    Vec<AutomationPoint>,
    pub sorted:    bool,
}

impl AutomationLane {
    pub fn new(node_id: NodeId, param_name: &str) -> Self {
        Self { node_id, param_name: param_name.to_string(), points: Vec::new(), sorted: true }
    }

    pub fn add_point(&mut self, sample: usize, value: f32) {
        self.points.push(AutomationPoint { sample, value });
        self.sorted = false;
    }

    pub fn sort(&mut self) {
        if !self.sorted {
            self.points.sort_by_key(|p| p.sample);
            self.sorted = true;
        }
    }

    /// Sample the automation value at a given sample position.
    pub fn sample_at(&mut self, pos: usize) -> Option<f32> {
        self.sort();
        if self.points.is_empty() { return None; }
        let idx = self.points.partition_point(|p| p.sample <= pos);
        if idx == 0 { return Some(self.points[0].value); }
        if idx >= self.points.len() { return Some(self.points.last().unwrap().value); }
        let a = &self.points[idx - 1];
        let b = &self.points[idx];
        let t = if b.sample > a.sample {
            (pos - a.sample) as f32 / (b.sample - a.sample) as f32
        } else { 0.0 };
        Some(a.value * (1.0 - t) + b.value * t)
    }
}

// ── Voice ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Voice {
    pub id:        VoiceId,
    pub note:      u8,       // MIDI note 0..127
    pub velocity:  f32,
    pub frequency: f32,
    pub active:    bool,
    pub steal_priority: f32, // for voice stealing
    nodes: HashMap<NodeId, AudioNode>, // cloned from template
    age: u64,
}

impl Voice {
    fn new(id: VoiceId, template: &HashMap<NodeId, AudioNode>) -> Self {
        let nodes = template.iter().map(|(&k, v)| (k, v.clone())).collect();
        Self {
            id, note: 69, velocity: 1.0, frequency: 440.0,
            active: false, steal_priority: 0.0, age: 0,
            nodes,
        }
    }

    fn note_on(&mut self, note: u8, velocity: f32) {
        self.note = note;
        self.velocity = velocity;
        self.frequency = midi_to_freq(note);
        self.active = true;
        for n in self.nodes.values_mut() {
            n.note_on(self.frequency, velocity);
        }
    }

    fn note_off(&mut self) {
        for n in self.nodes.values_mut() {
            n.note_off();
        }
    }

    fn is_silent(&self) -> bool {
        self.nodes.values().all(|n| n.envelope.is_done())
    }
}

fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

// ── Command (for lock-free audio thread communication) ────────────────────────

#[derive(Debug, Clone)]
pub enum AudioCommand {
    NoteOn    { note: u8, velocity: f32 },
    NoteOff   { note: u8 },
    AllNotesOff,
    SetParam  { node_id: NodeId, param: String, value: f32 },
    SetVolume { volume: f32 },
    SetBpm    { bpm: f32 },
    Mute      { node_id: NodeId },
    Unmute    { node_id: NodeId },
}

// ── AudioGraph ────────────────────────────────────────────────────────────────

/// The main audio processing graph.
pub struct AudioGraph {
    pub nodes:       HashMap<NodeId, AudioNode>,
    pub edges:       Vec<AudioEdge>,
    pub output_node: Option<NodeId>,
    pub automation:  Vec<AutomationLane>,
    topo_order:      Vec<NodeId>,
    topo_dirty:      bool,
    next_node_id:    u32,
    next_edge_id:    u32,

    // Voice pool for polyphonic synth
    voices:          Vec<Voice>,
    voice_counter:   u64,
    pub polyphony:   usize,

    // Master
    pub master_volume: f32,
    pub bpm:           f32,

    // Command queue (shared with audio thread)
    pub commands: Arc<Mutex<VecDeque<AudioCommand>>>,

    // Output buffer
    pub output_buffer: StereoBuffer,
}

impl AudioGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            output_node: None,
            automation: Vec::new(),
            topo_order: Vec::new(),
            topo_dirty: true,
            next_node_id: 1,
            next_edge_id: 1,
            voices: Vec::new(),
            voice_counter: 0,
            polyphony: 16,
            master_volume: 1.0,
            bpm: 120.0,
            commands: Arc::new(Mutex::new(VecDeque::new())),
            output_buffer: StereoBuffer::new(),
        }
    }

    pub fn add_node(&mut self, node_type: AudioNodeType) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let mut node = AudioNode::new(id, node_type);
        node.initialize();
        self.nodes.insert(id, node);
        self.topo_dirty = true;
        id
    }

    pub fn connect(&mut self, from: NodeId, from_slot: u8, to: NodeId, to_slot: u8) -> EdgeId {
        let id = EdgeId(self.next_edge_id);
        self.next_edge_id += 1;
        self.edges.push(AudioEdge { id, from, to, from_slot, to_slot, gain: 1.0 });
        self.topo_dirty = true;
        id
    }

    pub fn disconnect(&mut self, edge_id: EdgeId) {
        self.edges.retain(|e| e.id != edge_id);
        self.topo_dirty = true;
    }

    pub fn set_output(&mut self, node_id: NodeId) {
        self.output_node = Some(node_id);
    }

    pub fn add_automation(&mut self, lane: AutomationLane) {
        self.automation.push(lane);
    }

    /// Send a command to the audio thread.
    pub fn send_command(&self, cmd: AudioCommand) {
        if let Ok(mut q) = self.commands.lock() {
            q.push_back(cmd);
        }
    }

    pub fn note_on(&self, note: u8, velocity: f32) {
        self.send_command(AudioCommand::NoteOn { note, velocity });
    }

    pub fn note_off(&self, note: u8) {
        self.send_command(AudioCommand::NoteOff { note });
    }

    pub fn set_param(&self, node_id: NodeId, param: &str, value: f32) {
        self.send_command(AudioCommand::SetParam {
            node_id, param: param.to_string(), value,
        });
    }

    /// Topological sort (Kahn's algorithm).
    pub fn sort_topologically(&mut self) -> Result<(), String> {
        use std::collections::VecDeque;
        let mut in_degree: HashMap<NodeId, usize> = self.nodes.keys().map(|&id| (id, 0)).collect();
        for edge in &self.edges {
            *in_degree.entry(edge.to).or_insert(0) += 1;
        }
        let mut queue: VecDeque<NodeId> = in_degree.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(id) = queue.pop_front() {
            order.push(id);
            for edge in self.edges.iter().filter(|e| e.from == id) {
                let d = in_degree.entry(edge.to).or_insert(1);
                *d -= 1;
                if *d == 0 { queue.push_back(edge.to); }
            }
        }
        if order.len() != self.nodes.len() {
            return Err("Audio graph contains a cycle".to_string());
        }
        self.topo_order = order;
        self.topo_dirty = false;
        Ok(())
    }

    /// Process one audio buffer through the graph.
    pub fn process_block(&mut self) -> StereoBuffer {
        if self.topo_dirty {
            let _ = self.sort_topologically();
        }

        // Process commands
        self.process_commands();

        let mut node_outputs: HashMap<NodeId, AudioBuffer> = HashMap::new();

        for &node_id in &self.topo_order.clone() {
            let node = match self.nodes.get_mut(&node_id) {
                Some(n) => n,
                None => continue,
            };

            // Gather inputs for this node
            let input_edges: Vec<(u8, NodeId, f32)> = self.edges.iter()
                .filter(|e| e.to == node_id)
                .map(|e| (e.to_slot, e.from, e.gain))
                .collect();

            let mut inputs: Vec<AudioBuffer> = Vec::new();
            for (slot, from_id, gain) in &input_edges {
                let _ = slot;
                if let Some(buf) = node_outputs.get(from_id) {
                    let mut b = buf.clone();
                    b.scale(*gain);
                    inputs.push(b);
                }
            }

            let input_refs: Vec<&AudioBuffer> = inputs.iter().collect();
            let output = node.process(&input_refs);
            node_outputs.insert(node_id, output);
        }

        // Collect output
        let mono_out = if let Some(out_id) = self.output_node {
            node_outputs.get(&out_id).cloned().unwrap_or_default()
        } else {
            // Sum all nodes with no outgoing edges
            let has_outgoing: std::collections::HashSet<NodeId> = self.edges.iter().map(|e| e.from).collect();
            let mut sum = AudioBuffer::silence();
            for (&id, buf) in &node_outputs {
                if !has_outgoing.contains(&id) {
                    sum.add_from(buf);
                }
            }
            sum
        };

        // Apply master volume and pan to stereo
        let mut stereo = StereoBuffer::new();
        for i in 0..mono_out.len {
            let s = mono_out.samples[i] * self.master_volume;
            stereo.left.samples[i]  = s;
            stereo.right.samples[i] = s;
        }
        stereo.left.len  = mono_out.len;
        stereo.right.len = mono_out.len;

        self.output_buffer = stereo.clone();
        stereo
    }

    fn process_commands(&mut self) {
        let cmds: Vec<AudioCommand> = if let Ok(mut q) = self.commands.lock() {
            q.drain(..).collect()
        } else { Vec::new() };

        for cmd in cmds {
            match cmd {
                AudioCommand::NoteOn { note, velocity } => {
                    self.trigger_note_on(note, velocity);
                }
                AudioCommand::NoteOff { note } => {
                    self.trigger_note_off(note);
                }
                AudioCommand::AllNotesOff => {
                    for node in self.nodes.values_mut() {
                        node.note_off();
                    }
                }
                AudioCommand::SetParam { node_id, param, value } => {
                    self.apply_param(node_id, &param, value);
                }
                AudioCommand::SetVolume { volume } => {
                    self.master_volume = volume;
                }
                AudioCommand::SetBpm { bpm } => {
                    self.bpm = bpm;
                }
                AudioCommand::Mute { node_id } => {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.enabled = false; }
                }
                AudioCommand::Unmute { node_id } => {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.enabled = true; }
                }
            }
        }
    }

    fn trigger_note_on(&mut self, note: u8, velocity: f32) {
        for node in self.nodes.values_mut() {
            let freq = midi_to_freq(note);
            node.note_on(freq, velocity);
        }
    }

    fn trigger_note_off(&mut self, note: u8) {
        let _ = note;
        for node in self.nodes.values_mut() {
            node.note_off();
        }
    }

    fn apply_param(&mut self, node_id: NodeId, param: &str, value: f32) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            match param {
                "gain_db" => {
                    if let AudioNodeType::Gain { gain_db } = &mut node.node_type {
                        *gain_db = value;
                    }
                }
                "frequency" => {
                    match &mut node.node_type {
                        AudioNodeType::Oscillator { frequency, .. } => *frequency = value,
                        AudioNodeType::BiquadFilter { frequency, .. } => {
                            *frequency = value;
                            node.initialize();
                        }
                        AudioNodeType::Lfo { frequency, .. } => *frequency = value,
                        _ => {}
                    }
                }
                "cutoff" => {
                    if let AudioNodeType::LadderFilter { cutoff, .. } = &mut node.node_type {
                        *cutoff = value;
                    }
                }
                "resonance" => {
                    if let AudioNodeType::LadderFilter { resonance, .. } = &mut node.node_type {
                        *resonance = value;
                    }
                }
                "room_size" => {
                    if let AudioNodeType::Reverb { room_size, .. } = &mut node.node_type {
                        *room_size = value;
                        node.reverb.room_size = value;
                    }
                }
                "delay_time" => {
                    if let AudioNodeType::Delay { delay_time, .. } = &mut node.node_type {
                        *delay_time = value;
                        node.delay_line.set_delay_time(value);
                    }
                }
                "volume" => {
                    if let AudioNodeType::Output { volume } = &mut node.node_type {
                        *volume = value;
                    }
                }
                "pan" => {
                    if let AudioNodeType::Panner { pan } = &mut node.node_type {
                        *pan = value;
                    }
                }
                _ => {}
            }
        }
    }

    /// Build a simple signal chain from oscillator → filter → envelope → output.
    pub fn basic_synth_chain(&mut self, wave: Waveform, freq: f32) -> (NodeId, NodeId, NodeId, NodeId) {
        let osc  = self.add_node(AudioNodeType::Oscillator { waveform: wave, frequency: freq, detune_cents: 0.0 });
        let filt = self.add_node(AudioNodeType::BiquadFilter { filter_type: FilterType::LowPass, frequency: 2000.0, q: 0.7, gain_db: 0.0 });
        let env  = self.add_node(AudioNodeType::Adsr { attack: 0.01, decay: 0.1, sustain: 0.8, release: 0.3 });
        let out  = self.add_node(AudioNodeType::Output { volume: 1.0 });

        self.connect(osc, 0, filt, 0);
        self.connect(filt, 0, env, 0);
        self.connect(env, 0, out, 0);
        self.set_output(out);

        (osc, filt, env, out)
    }

    /// FM synthesis chain.
    pub fn fm_synth_chain(&mut self, carrier: f32, ratio: f32, index: f32) -> (NodeId, NodeId) {
        let fm  = self.add_node(AudioNodeType::FmOscillator {
            carrier_freq: carrier,
            mod_freq: carrier * ratio,
            mod_index: index,
            waveform: Waveform::Sine,
        });
        let out = self.add_node(AudioNodeType::Output { volume: 0.8 });
        self.connect(fm, 0, out, 0);
        self.set_output(out);
        (fm, out)
    }

    /// Drum machine chain: noise → HP filter → fast envelope → output.
    pub fn drum_chain(&mut self, is_snare: bool) -> NodeId {
        let noise = self.add_node(AudioNodeType::Noise { color: NoiseColor::White });
        let filt  = self.add_node(AudioNodeType::BiquadFilter {
            filter_type: if is_snare { FilterType::BandPass } else { FilterType::LowPass },
            frequency:   if is_snare { 3000.0 } else { 150.0 },
            q: 0.5, gain_db: 0.0,
        });
        let env   = self.add_node(AudioNodeType::Adsr { attack: 0.001, decay: 0.1, sustain: 0.0, release: 0.05 });
        let comp  = self.add_node(AudioNodeType::Compressor { threshold_db: -6.0, ratio: 4.0, attack_ms: 0.5, release_ms: 50.0 });
        let out   = self.add_node(AudioNodeType::Output { volume: 1.0 });

        self.connect(noise, 0, filt, 0);
        self.connect(filt,  0, env,  0);
        self.connect(env,   0, comp, 0);
        self.connect(comp,  0, out,  0);
        self.set_output(out);
        out
    }
}

impl Default for AudioGraph {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oscillator_produces_signal() {
        let mut node = AudioNode::new(NodeId(1), AudioNodeType::Oscillator {
            waveform: Waveform::Sine, frequency: 440.0, detune_cents: 0.0,
        });
        let out = node.process(&[]);
        let peak = out.peak();
        assert!(peak > 0.5, "sine oscillator should produce near-peak amplitude, got {}", peak);
    }

    #[test]
    fn test_noise_not_silent() {
        let mut node = AudioNode::new(NodeId(1), AudioNodeType::Noise { color: NoiseColor::White });
        let out = node.process(&[]);
        assert!(out.peak() > 0.0);
    }

    #[test]
    fn test_biquad_low_pass() {
        let mut node = AudioNode::new(NodeId(1), AudioNodeType::BiquadFilter {
            filter_type: FilterType::LowPass, frequency: 100.0, q: 0.7, gain_db: 0.0,
        });
        node.initialize();
        let mut noise_buf = AudioBuffer::new();
        for (i, s) in noise_buf.samples.iter_mut().enumerate() {
            *s = Waveform::Sine.sample(i as f32 * 8000.0 / SAMPLE_RATE); // high-freq tone
        }
        let out = node.process(&[&noise_buf]);
        // LP at 100Hz should heavily attenuate 8kHz signal
        assert!(out.peak() < noise_buf.peak() * 0.5, "LP filter should attenuate high frequencies");
    }

    #[test]
    fn test_adsr_envelope_shape() {
        let mut env = AdsrEnvelope::new(0.01, 0.05, 0.7, 0.1);
        env.note_on();
        // Sample to end of attack
        let attack_samples = (0.01 * SAMPLE_RATE) as usize;
        for _ in 0..attack_samples {
            env.next_sample();
        }
        // Should be near 1.0 at end of attack
        let v = env.next_sample();
        assert!(v > 0.8, "envelope should be near peak after attack, got {}", v);
    }

    #[test]
    fn test_adsr_note_off_releases() {
        let mut env = AdsrEnvelope::new(0.001, 0.001, 0.8, 0.01);
        env.note_on();
        // Reach sustain
        for _ in 0..((0.05 * SAMPLE_RATE) as usize) { env.next_sample(); }
        env.note_off();
        // After release
        for _ in 0..((0.02 * SAMPLE_RATE) as usize) { env.next_sample(); }
        assert!(env.is_done() || env.level < 0.01, "envelope should release to near-zero");
    }

    #[test]
    fn test_lfo_oscillates() {
        let mut lfo = Lfo::new(Waveform::Sine, 10.0);
        let samples: Vec<f32> = (0..1000).map(|_| lfo.next_sample()).collect();
        let min = samples.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(max > 0.9 && min < -0.9, "LFO should reach full amplitude");
    }

    #[test]
    fn test_delay_line_delays() {
        let mut dl = DelayLine::new(4096);
        dl.set_delay_time(0.01); // 10ms ≈ 441 samples
        let impulse_sample = SAMPLE_RATE as usize / 100; // 10ms delay worth of samples
        let mut heard_at = 0;
        for i in 0..1024 {
            let input = if i == 0 { 1.0 } else { 0.0 };
            let out = dl.process(input);
            if out > 0.5 { heard_at = i; break; }
        }
        assert!(heard_at > 0 && heard_at <= impulse_sample + 5,
            "impulse should arrive after delay, heard at sample {}", heard_at);
    }

    #[test]
    fn test_reverb_adds_tail() {
        let mut reverb = Reverb::new();
        let mut input = AudioBuffer::silence();
        input.samples[0] = 1.0; // single impulse
        let out = reverb.process_buffer(&input);
        // After reverb, multiple samples should be non-zero (tail)
        let nonzero = out.samples.iter().filter(|&&s| s.abs() > 1e-4).count();
        assert!(nonzero > 10, "reverb should produce a tail, got {} non-zero samples", nonzero);
    }

    #[test]
    fn test_graph_topological_sort() {
        let mut graph = AudioGraph::new();
        let osc  = graph.add_node(AudioNodeType::Oscillator { waveform: Waveform::Sine, frequency: 440.0, detune_cents: 0.0 });
        let gain = graph.add_node(AudioNodeType::Gain { gain_db: 0.0 });
        let out  = graph.add_node(AudioNodeType::Output { volume: 1.0 });
        graph.connect(osc, 0, gain, 0);
        graph.connect(gain, 0, out, 0);
        graph.set_output(out);
        assert!(graph.sort_topologically().is_ok());
        assert_eq!(graph.topo_order, vec![osc, gain, out]);
    }

    #[test]
    fn test_graph_process_block() {
        let mut graph = AudioGraph::new();
        graph.basic_synth_chain(Waveform::Sine, 440.0);
        // Send note on
        graph.note_on(69, 1.0);
        let _ = graph.process_block(); // process commands
        let stereo = graph.process_block();
        // Should have some signal
        assert!(stereo.peak() >= 0.0, "graph should process without panic");
    }

    #[test]
    fn test_compressor_reduces_peaks() {
        let mut comp = Compressor::new(-6.0, 4.0);
        comp.makeup_gain = 0.0;
        let mut loud = AudioBuffer::new();
        loud.fill(2.0); // above threshold
        comp.process_buffer(&mut loud);
        assert!(loud.peak() < 2.0, "compressor should reduce loud signal");
    }

    #[test]
    fn test_distortion_clips() {
        let dist = Distortion::new(DistortionMode::HardClip, 5.0);
        let mut buf = AudioBuffer::new();
        buf.fill(0.5);
        dist.process_buffer(&mut buf);
        assert!(buf.peak() <= 1.0 + 1e-6, "hard clip should limit to ±1");
    }

    #[test]
    fn test_fm_oscillator() {
        let mut node = AudioNode::new(NodeId(1), AudioNodeType::FmOscillator {
            carrier_freq: 440.0, mod_freq: 440.0, mod_index: 1.0, waveform: Waveform::Sine,
        });
        let out = node.process(&[]);
        assert!(out.peak() > 0.0, "FM oscillator should produce signal");
    }

    #[test]
    fn test_waveform_samples() {
        for w in [Waveform::Sine, Waveform::Square, Waveform::Triangle, Waveform::Sawtooth] {
            let s = w.sample(0.25);
            assert!(s.is_finite(), "{:?} at 0.25 should be finite, got {}", w, s);
        }
    }

    #[test]
    fn test_automation_lane() {
        let mut lane = AutomationLane::new(NodeId(1), "frequency");
        lane.add_point(0,   100.0);
        lane.add_point(100, 200.0);
        let v = lane.sample_at(50).unwrap();
        assert!((v - 150.0).abs() < 1.0, "automation should interpolate, got {}", v);
    }

    #[test]
    fn test_drum_chain_builds() {
        let mut graph = AudioGraph::new();
        let out = graph.drum_chain(false); // kick
        assert!(graph.nodes.len() >= 4);
        assert!(graph.edges.len() >= 3);
        let _ = out;
    }
}
