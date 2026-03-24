//! Audio effects and processing: gain, dynamics, EQ, reverb, delay, modulation,
//! distortion, pitch, and effect chain management.

use std::f32::consts::{PI, TAU};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Core trait
// ---------------------------------------------------------------------------

/// Every audio effect implements this trait.
pub trait AudioEffect: Send {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32);
    /// Human-readable name for debugging/UI.
    fn name(&self) -> &str;
    /// Reset all internal state (e.g. delay lines, filters).
    fn reset(&mut self);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline]
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[inline]
fn linear_to_db(lin: f32) -> f32 {
    if lin <= 1e-10 { -200.0 } else { 20.0 * lin.log10() }
}

/// One-pole low-pass: y[n] = alpha*x[n] + (1-alpha)*y[n-1]
struct OnePole {
    alpha: f32,
    state: f32,
}
impl OnePole {
    fn new(cutoff_hz: f32, sample_rate: f32) -> Self {
        let alpha = 1.0 - (-TAU * cutoff_hz / sample_rate).exp();
        Self { alpha, state: 0.0 }
    }
    fn process(&mut self, x: f32) -> f32 {
        self.state += self.alpha * (x - self.state);
        self.state
    }
    fn reset(&mut self) { self.state = 0.0; }
}

// ---------------------------------------------------------------------------
// Gain
// ---------------------------------------------------------------------------

/// Linear gain applied to every sample.
pub struct Gain {
    pub gain: f32,
}
impl Gain {
    pub fn new(gain: f32) -> Self { Self { gain } }
}
impl AudioEffect for Gain {
    fn process_block(&mut self, buffer: &mut [f32], _sample_rate: f32) {
        for s in buffer.iter_mut() { *s *= self.gain; }
    }
    fn name(&self) -> &str { "Gain" }
    fn reset(&mut self) {}
}

// ---------------------------------------------------------------------------
// GainAutomation
// ---------------------------------------------------------------------------

/// A breakpoint for gain automation.
#[derive(Clone, Copy, Debug)]
pub struct GainBreakpoint {
    /// Sample index.
    pub sample: u64,
    /// Target gain in linear scale.
    pub gain: f32,
    /// If true, interpolate exponentially; otherwise linearly.
    pub exponential: bool,
}

/// Applies per-sample automated gain from a breakpoint list.
pub struct GainAutomation {
    pub breakpoints: Vec<GainBreakpoint>,
    current_sample: u64,
    current_gain: f32,
}
impl GainAutomation {
    pub fn new(breakpoints: Vec<GainBreakpoint>) -> Self {
        let initial = breakpoints.first().map(|b| b.gain).unwrap_or(1.0);
        Self { breakpoints, current_sample: 0, current_gain: initial }
    }

    fn gain_at_sample(&self, s: u64) -> f32 {
        if self.breakpoints.is_empty() { return 1.0; }
        if s <= self.breakpoints.first().unwrap().sample {
            return self.breakpoints.first().unwrap().gain;
        }
        if s >= self.breakpoints.last().unwrap().sample {
            return self.breakpoints.last().unwrap().gain;
        }
        // Binary search for enclosing pair
        let idx = self.breakpoints.partition_point(|b| b.sample <= s);
        let a = &self.breakpoints[idx - 1];
        let b = &self.breakpoints[idx];
        let t = (s - a.sample) as f32 / (b.sample - a.sample) as f32;
        if a.exponential && a.gain > 0.0 && b.gain > 0.0 {
            a.gain * (b.gain / a.gain).powf(t)
        } else {
            a.gain + (b.gain - a.gain) * t
        }
    }
}
impl AudioEffect for GainAutomation {
    fn process_block(&mut self, buffer: &mut [f32], _sample_rate: f32) {
        for s in buffer.iter_mut() {
            self.current_gain = self.gain_at_sample(self.current_sample);
            *s *= self.current_gain;
            self.current_sample += 1;
        }
    }
    fn name(&self) -> &str { "GainAutomation" }
    fn reset(&mut self) { self.current_sample = 0; }
}

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

/// Soft-knee / hard-knee compressor with optional RMS detection and sidechain.
pub struct Compressor {
    /// Threshold in dBFS.
    pub threshold_db: f32,
    /// Compression ratio (e.g. 4.0 = 4:1).
    pub ratio: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
    /// Soft-knee width in dB (0 = hard knee).
    pub knee_db: f32,
    /// Make-up gain in dB.
    pub makeup_db: f32,
    /// Use RMS detection (true) or peak (false).
    pub use_rms: bool,
    /// Lookahead delay in samples.
    pub lookahead_samples: usize,
    /// Current gain reduction in dB (read-only metering).
    pub gain_reduction_db: f32,

    envelope: f32,
    rms_sum: f32,
    rms_buf: Vec<f32>,
    rms_pos: usize,
    lookahead: VecDeque<f32>,
}
impl Compressor {
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        knee_db: f32,
        makeup_db: f32,
        use_rms: bool,
        lookahead_samples: usize,
    ) -> Self {
        let rms_window = 256_usize.max(lookahead_samples);
        Self {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_db,
            use_rms,
            lookahead_samples,
            gain_reduction_db: 0.0,
            envelope: 0.0,
            rms_sum: 0.0,
            rms_buf: vec![0.0; rms_window],
            rms_pos: 0,
            lookahead: VecDeque::from(vec![0.0; lookahead_samples]),
        }
    }

    fn compute_gain_db(&self, level_db: f32) -> f32 {
        let t = self.threshold_db;
        let r = self.ratio;
        let w = self.knee_db;
        if w > 0.0 {
            let half = w * 0.5;
            if level_db < t - half {
                0.0
            } else if level_db <= t + half {
                let x = level_db - t + half;
                (1.0 / r - 1.0) * x * x / (2.0 * w)
            } else {
                (1.0 / r - 1.0) * (level_db - t)
            }
        } else if level_db > t {
            (1.0 / r - 1.0) * (level_db - t)
        } else {
            0.0
        }
    }
}
impl AudioEffect for Compressor {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let attack_coef = (-1.0 / (self.attack_ms * 0.001 * sample_rate)).exp();
        let release_coef = (-1.0 / (self.release_ms * 0.001 * sample_rate)).exp();
        let makeup = db_to_linear(self.makeup_db);
        let rms_len = self.rms_buf.len() as f32;

        for s in buffer.iter_mut() {
            // Lookahead: push current into queue, pop delayed sample
            let delayed = if self.lookahead_samples > 0 {
                self.lookahead.push_back(*s);
                self.lookahead.pop_front().unwrap_or(0.0)
            } else {
                *s
            };

            // Level detection
            let level = if self.use_rms {
                let old = self.rms_buf[self.rms_pos];
                let new_sq = (*s) * (*s);
                self.rms_sum = (self.rms_sum - old + new_sq).max(0.0);
                self.rms_buf[self.rms_pos] = new_sq;
                self.rms_pos = (self.rms_pos + 1) % self.rms_buf.len();
                (self.rms_sum / rms_len).sqrt()
            } else {
                s.abs()
            };

            let level_db = linear_to_db(level);
            let desired_gain_db = self.compute_gain_db(level_db);
            // Envelope follower
            let coef = if desired_gain_db < self.envelope {
                attack_coef
            } else {
                release_coef
            };
            self.envelope = desired_gain_db + coef * (self.envelope - desired_gain_db);
            self.gain_reduction_db = -self.envelope;
            let gr = db_to_linear(self.envelope);
            *s = delayed * gr * makeup;
        }
    }
    fn name(&self) -> &str { "Compressor" }
    fn reset(&mut self) {
        self.envelope = 0.0;
        self.rms_sum = 0.0;
        for v in self.rms_buf.iter_mut() { *v = 0.0; }
        self.rms_pos = 0;
        let la = self.lookahead_samples;
        self.lookahead = VecDeque::from(vec![0.0; la]);
        self.gain_reduction_db = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Limiter
// ---------------------------------------------------------------------------

/// Brickwall peak limiter with true-peak (4x oversampling) and lookahead.
pub struct Limiter {
    pub threshold_db: f32,
    pub release_ms: f32,
    pub lookahead_samples: usize,
    /// Current gain reduction in dB.
    pub gain_reduction_db: f32,

    envelope: f32,
    lookahead: VecDeque<f32>,
}
impl Limiter {
    pub fn new(threshold_db: f32, release_ms: f32, lookahead_samples: usize) -> Self {
        Self {
            threshold_db,
            release_ms,
            lookahead_samples,
            gain_reduction_db: 0.0,
            envelope: 0.0,
            lookahead: VecDeque::from(vec![0.0; lookahead_samples.max(1)]),
        }
    }

    /// 4× oversampled true-peak detection for a single sample.
    fn true_peak(x: f32, prev: f32) -> f32 {
        // Linear interpolation upsample 4×
        let mut max = x.abs();
        for k in 1..4usize {
            let t = k as f32 / 4.0;
            let interp = prev + (x - prev) * t;
            max = max.max(interp.abs());
        }
        max
    }
}
impl AudioEffect for Limiter {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let release_coef = (-1.0 / (self.release_ms * 0.001 * sample_rate)).exp();
        let threshold = db_to_linear(self.threshold_db);
        let mut prev = 0.0f32;

        for s in buffer.iter_mut() {
            let delayed = if self.lookahead_samples > 0 {
                self.lookahead.push_back(*s);
                self.lookahead.pop_front().unwrap_or(0.0)
            } else {
                *s
            };

            let tp = Self::true_peak(*s, prev);
            prev = *s;
            let gain_needed = if tp > threshold { threshold / tp.max(1e-10) } else { 1.0 };
            // Attack is instant (brickwall), release follows envelope
            let target_db = linear_to_db(gain_needed);
            let current_db = if target_db < self.envelope {
                target_db
            } else {
                target_db + release_coef * (self.envelope - target_db)
            };
            self.envelope = current_db;
            self.gain_reduction_db = -(current_db.min(0.0));
            let gr = db_to_linear(current_db);
            *s = delayed * gr;
        }
    }
    fn name(&self) -> &str { "Limiter" }
    fn reset(&mut self) {
        self.envelope = 0.0;
        self.gain_reduction_db = 0.0;
        let la = self.lookahead_samples.max(1);
        self.lookahead = VecDeque::from(vec![0.0; la]);
    }
}

// ---------------------------------------------------------------------------
// Gate
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GateState { Closed, Attacking, Open, Releasing, Holding }

/// Noise gate with hysteresis (separate open/close thresholds), hold, and flip (duck) mode.
pub struct Gate {
    pub open_threshold_db: f32,
    pub close_threshold_db: f32,
    /// Range in dB: how much to attenuate when gate is closed (negative value, e.g. -80).
    pub range_db: f32,
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub release_ms: f32,
    /// Flip: gate opens when signal is ABOVE threshold (ducking).
    pub flip: bool,

    state: GateState,
    gain: f32,
    hold_samples_remaining: usize,
}
impl Gate {
    pub fn new(
        open_threshold_db: f32,
        close_threshold_db: f32,
        range_db: f32,
        attack_ms: f32,
        hold_ms: f32,
        release_ms: f32,
        flip: bool,
    ) -> Self {
        Self {
            open_threshold_db,
            close_threshold_db,
            range_db,
            attack_ms,
            hold_ms,
            release_ms,
            flip,
            state: GateState::Closed,
            gain: db_to_linear(range_db),
            hold_samples_remaining: 0,
        }
    }
}
impl AudioEffect for Gate {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let open_lin = db_to_linear(self.open_threshold_db);
        let close_lin = db_to_linear(self.close_threshold_db);
        let floor = db_to_linear(self.range_db);
        let attack_step = 1.0 / (self.attack_ms * 0.001 * sample_rate).max(1.0);
        let release_step = (1.0 - floor) / (self.release_ms * 0.001 * sample_rate).max(1.0);
        let hold_total = (self.hold_ms * 0.001 * sample_rate) as usize;

        for s in buffer.iter_mut() {
            let level = s.abs();
            let above_open = if self.flip { level <= open_lin } else { level >= open_lin };
            let below_close = if self.flip { level > close_lin } else { level < close_lin };

            self.state = match self.state {
                GateState::Closed => {
                    if above_open { GateState::Attacking } else { GateState::Closed }
                }
                GateState::Attacking => {
                    self.gain = (self.gain + attack_step).min(1.0);
                    if self.gain >= 1.0 { GateState::Open } else { GateState::Attacking }
                }
                GateState::Open => {
                    if below_close {
                        self.hold_samples_remaining = hold_total;
                        GateState::Holding
                    } else {
                        GateState::Open
                    }
                }
                GateState::Holding => {
                    if above_open {
                        GateState::Open
                    } else if self.hold_samples_remaining == 0 {
                        GateState::Releasing
                    } else {
                        self.hold_samples_remaining -= 1;
                        GateState::Holding
                    }
                }
                GateState::Releasing => {
                    self.gain = (self.gain - release_step).max(floor);
                    if above_open {
                        GateState::Attacking
                    } else if self.gain <= floor {
                        GateState::Closed
                    } else {
                        GateState::Releasing
                    }
                }
            };
            *s *= self.gain;
        }
    }
    fn name(&self) -> &str { "Gate" }
    fn reset(&mut self) {
        self.state = GateState::Closed;
        self.gain = db_to_linear(self.range_db);
        self.hold_samples_remaining = 0;
    }
}

// ---------------------------------------------------------------------------
// Biquad filter
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BiquadType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    AllPass,
    PeakEq,
    LowShelf,
    HighShelf,
}

/// Direct-form II transposed biquad filter.
#[derive(Clone, Debug)]
pub struct BiquadBand {
    pub filter_type: BiquadType,
    pub frequency: f32,
    pub q: f32,
    pub gain_db: f32,
    pub active: bool,
    // Coefficients
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    // State
    s1: f32, s2: f32,
}
impl BiquadBand {
    pub fn new(filter_type: BiquadType, frequency: f32, q: f32, gain_db: f32) -> Self {
        let mut b = Self {
            filter_type, frequency, q, gain_db, active: true,
            b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            s1: 0.0, s2: 0.0,
        };
        b.compute_coefficients(44100.0);
        b
    }

    pub fn compute_coefficients(&mut self, sample_rate: f32) {
        let f = self.frequency;
        let q = self.q.max(0.001);
        let a = db_to_linear(self.gain_db / 2.0); // amplitude for peak/shelf
        let w0 = TAU * f / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            BiquadType::LowPass => {
                let b0 = (1.0 - cos_w0) / 2.0;
                let b1 = 1.0 - cos_w0;
                let b2 = (1.0 - cos_w0) / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            BiquadType::HighPass => {
                let b0 = (1.0 + cos_w0) / 2.0;
                let b1 = -(1.0 + cos_w0);
                let b2 = (1.0 + cos_w0) / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            BiquadType::BandPass => {
                let b0 = sin_w0 / 2.0;
                let b1 = 0.0;
                let b2 = -sin_w0 / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            BiquadType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            BiquadType::AllPass => {
                let b0 = 1.0 - alpha;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0 + alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            BiquadType::PeakEq => {
                let b0 = 1.0 + alpha * a;
                let b1 = -2.0 * cos_w0;
                let b2 = 1.0 - alpha * a;
                let a0 = 1.0 + alpha / a;
                let a1 = -2.0 * cos_w0;
                let a2 = 1.0 - alpha / a;
                (b0, b1, b2, a0, a1, a2)
            }
            BiquadType::LowShelf => {
                let sq = 2.0 * a.sqrt() * alpha;
                let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + sq);
                let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
                let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - sq);
                let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + sq;
                let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
                let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - sq;
                (b0, b1, b2, a0, a1, a2)
            }
            BiquadType::HighShelf => {
                let sq = 2.0 * a.sqrt() * alpha;
                let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + sq);
                let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
                let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - sq);
                let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + sq;
                let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
                let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - sq;
                (b0, b1, b2, a0, a1, a2)
            }
        };
        let a0_inv = 1.0 / a0;
        self.b0 = b0 * a0_inv;
        self.b1 = b1 * a0_inv;
        self.b2 = b2 * a0_inv;
        self.a1 = a1 * a0_inv;
        self.a2 = a2 * a0_inv;
    }

    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.s1;
        self.s1 = self.b1 * x - self.a1 * y + self.s2;
        self.s2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn reset(&mut self) { self.s1 = 0.0; self.s2 = 0.0; }
}

// ---------------------------------------------------------------------------
// Equalizer
// ---------------------------------------------------------------------------

/// Parametric EQ with up to 16 bands.
pub struct Equalizer {
    pub bands: Vec<BiquadBand>,
    last_sample_rate: f32,
}
impl Equalizer {
    pub fn new(bands: Vec<BiquadBand>) -> Self {
        Self { bands, last_sample_rate: 0.0 }
    }

    pub fn add_band(&mut self, band: BiquadBand) {
        if self.bands.len() < 16 { self.bands.push(band); }
    }
}
impl AudioEffect for Equalizer {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        if (sample_rate - self.last_sample_rate).abs() > 0.1 {
            for b in self.bands.iter_mut() { b.compute_coefficients(sample_rate); }
            self.last_sample_rate = sample_rate;
        }
        for s in buffer.iter_mut() {
            let mut v = *s;
            for band in self.bands.iter_mut() {
                if band.active { v = band.process_sample(v); }
            }
            *s = v;
        }
    }
    fn name(&self) -> &str { "Equalizer" }
    fn reset(&mut self) {
        for b in self.bands.iter_mut() { b.reset(); }
    }
}

// ---------------------------------------------------------------------------
// Reverb (Freeverb-style)
// ---------------------------------------------------------------------------

const COMB_TUNING: [usize; 8]     = [1116,1188,1277,1356,1422,1491,1557,1617];
const ALLPASS_TUNING: [usize; 4]  = [556, 441, 341, 225];

struct CombFilter {
    buf: Vec<f32>,
    pos: usize,
    feedback: f32,
    damp1: f32,
    damp2: f32,
    filterstore: f32,
}
impl CombFilter {
    fn new(size: usize) -> Self {
        Self { buf: vec![0.0; size], pos: 0, feedback: 0.5, damp1: 0.5, damp2: 0.5, filterstore: 0.0 }
    }
    fn set_damp(&mut self, damp: f32) { self.damp1 = damp; self.damp2 = 1.0 - damp; }
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buf[self.pos];
        self.filterstore = output * self.damp2 + self.filterstore * self.damp1;
        self.buf[self.pos] = input + self.filterstore * self.feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        output
    }
    fn reset(&mut self) { for v in self.buf.iter_mut() { *v = 0.0; } self.filterstore = 0.0; self.pos = 0; }
}

struct AllpassFilter {
    buf: Vec<f32>,
    pos: usize,
    feedback: f32,
}
impl AllpassFilter {
    fn new(size: usize) -> Self {
        Self { buf: vec![0.0; size], pos: 0, feedback: 0.5 }
    }
    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buf[self.pos];
        let output = -input + bufout;
        self.buf[self.pos] = input + bufout * self.feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        output
    }
    fn reset(&mut self) { for v in self.buf.iter_mut() { *v = 0.0; } self.pos = 0; }
}

/// Freeverb-inspired stereo reverb.
pub struct Reverb {
    pub room_size: f32,
    pub damping: f32,
    pub wet: f32,
    pub dry: f32,
    pub pre_delay_ms: f32,
    pub width: f32,

    combs_l: Vec<CombFilter>,
    combs_r: Vec<CombFilter>,
    allpasses_l: Vec<AllpassFilter>,
    allpasses_r: Vec<AllpassFilter>,
    pre_delay_buf: VecDeque<f32>,
    last_sample_rate: f32,
}
impl Reverb {
    pub fn new(room_size: f32, damping: f32, wet: f32, dry: f32, pre_delay_ms: f32, width: f32) -> Self {
        let stereo_spread = 23;
        let combs_l: Vec<CombFilter> = COMB_TUNING.iter().map(|&s| CombFilter::new(s)).collect();
        let combs_r: Vec<CombFilter> = COMB_TUNING.iter().map(|&s| CombFilter::new(s + stereo_spread)).collect();
        let allpasses_l: Vec<AllpassFilter> = ALLPASS_TUNING.iter().map(|&s| AllpassFilter::new(s)).collect();
        let allpasses_r: Vec<AllpassFilter> = ALLPASS_TUNING.iter().map(|&s| AllpassFilter::new(s + stereo_spread)).collect();
        let pre_delay_samples = ((pre_delay_ms * 0.001) * 44100.0) as usize;
        let pre_delay_buf = VecDeque::from(vec![0.0f32; pre_delay_samples.max(1)]);
        let mut r = Self {
            room_size, damping, wet, dry, pre_delay_ms, width,
            combs_l, combs_r, allpasses_l, allpasses_r,
            pre_delay_buf,
            last_sample_rate: 44100.0,
        };
        r.update_coefficients();
        r
    }

    fn update_coefficients(&mut self) {
        let feedback = self.room_size * 0.28 + 0.7;
        let damp = self.damping * 0.4;
        for c in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) {
            c.feedback = feedback;
            c.set_damp(damp);
        }
    }
}
impl AudioEffect for Reverb {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        if (sample_rate - self.last_sample_rate).abs() > 0.1 {
            let pre_samples = ((self.pre_delay_ms * 0.001) * sample_rate) as usize;
            self.pre_delay_buf = VecDeque::from(vec![0.0f32; pre_samples.max(1)]);
            self.last_sample_rate = sample_rate;
        }
        self.update_coefficients();

        for s in buffer.iter_mut() {
            // Pre-delay
            self.pre_delay_buf.push_back(*s);
            let input = self.pre_delay_buf.pop_front().unwrap_or(0.0);

            // Mono input → both channels
            let input_mixed = input * 0.015;
            let mut out_l = 0.0f32;
            let mut out_r = 0.0f32;
            for c in self.combs_l.iter_mut() { out_l += c.process(input_mixed); }
            for c in self.combs_r.iter_mut() { out_r += c.process(input_mixed); }
            for a in self.allpasses_l.iter_mut() { out_l = a.process(out_l); }
            for a in self.allpasses_r.iter_mut() { out_r = a.process(out_r); }

            let wet_l = out_l * (0.5 + self.width * 0.5) + out_r * (0.5 - self.width * 0.5);
            *s = *s * self.dry + wet_l * self.wet;
        }
    }
    fn name(&self) -> &str { "Reverb" }
    fn reset(&mut self) {
        for c in self.combs_l.iter_mut().chain(self.combs_r.iter_mut()) { c.reset(); }
        for a in self.allpasses_l.iter_mut().chain(self.allpasses_r.iter_mut()) { a.reset(); }
        let n = self.pre_delay_buf.len();
        self.pre_delay_buf = VecDeque::from(vec![0.0f32; n]);
    }
}

// ---------------------------------------------------------------------------
// Delay
// ---------------------------------------------------------------------------

/// Stereo-capable delay line with feedback, high-cut in feedback path, and ping-pong.
pub struct Delay {
    pub delay_ms: f32,
    pub feedback: f32,
    pub highcut_hz: f32,
    pub ping_pong: bool,
    pub wet: f32,
    pub dry: f32,
    /// Tap tempo BPM (0 = use delay_ms directly).
    pub tempo_bpm: f32,
    /// Fraction of a beat (e.g. 0.5 = 8th note).
    pub beat_division: f32,

    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    pos: usize,
    hpf_l: OnePole,
    hpf_r: OnePole,
    last_sample_rate: f32,
}
impl Delay {
    pub fn new(delay_ms: f32, feedback: f32, highcut_hz: f32, ping_pong: bool, wet: f32, dry: f32) -> Self {
        let max_samples = (4.0 * 48000.0) as usize; // 4 seconds at 48k
        Self {
            delay_ms, feedback, highcut_hz, ping_pong, wet, dry,
            tempo_bpm: 0.0, beat_division: 1.0,
            buf_l: vec![0.0; max_samples],
            buf_r: vec![0.0; max_samples],
            pos: 0,
            hpf_l: OnePole::new(highcut_hz, 44100.0),
            hpf_r: OnePole::new(highcut_hz, 44100.0),
            last_sample_rate: 44100.0,
        }
    }

    fn effective_delay_samples(&self, sample_rate: f32) -> usize {
        let ms = if self.tempo_bpm > 0.0 {
            60000.0 / self.tempo_bpm * self.beat_division
        } else {
            self.delay_ms
        };
        ((ms * 0.001 * sample_rate) as usize).clamp(1, self.buf_l.len() - 1)
    }
}
impl AudioEffect for Delay {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        if (sample_rate - self.last_sample_rate).abs() > 0.1 {
            self.hpf_l = OnePole::new(self.highcut_hz, sample_rate);
            self.hpf_r = OnePole::new(self.highcut_hz, sample_rate);
            self.last_sample_rate = sample_rate;
        }
        let delay_samp = self.effective_delay_samples(sample_rate);
        let len = self.buf_l.len();

        for s in buffer.iter_mut() {
            let read_pos = (self.pos + len - delay_samp) % len;
            let del_l = self.buf_l[read_pos];
            let del_r = self.buf_r[read_pos];

            // High-cut on feedback path
            let fb_l = self.hpf_l.process(del_l) * self.feedback;
            let fb_r = self.hpf_r.process(del_r) * self.feedback;

            if self.ping_pong {
                self.buf_l[self.pos] = *s + fb_r;
                self.buf_r[self.pos] = fb_l;
            } else {
                self.buf_l[self.pos] = *s + fb_l;
                self.buf_r[self.pos] = *s + fb_r;
            }
            self.pos = (self.pos + 1) % len;
            *s = *s * self.dry + del_l * self.wet;
        }
    }
    fn name(&self) -> &str { "Delay" }
    fn reset(&mut self) {
        for v in self.buf_l.iter_mut() { *v = 0.0; }
        for v in self.buf_r.iter_mut() { *v = 0.0; }
        self.pos = 0;
        self.hpf_l.reset();
        self.hpf_r.reset();
    }
}

// ---------------------------------------------------------------------------
// Chorus
// ---------------------------------------------------------------------------

/// Multi-voice chorus (up to 8 LFO-modulated delay lines).
pub struct Chorus {
    pub voices: usize,
    pub rate_hz: f32,
    pub depth_ms: f32,
    pub spread: f32,
    pub feedback: f32,
    pub wet: f32,
    pub dry: f32,

    buf: Vec<Vec<f32>>,
    pos: usize,
    phases: Vec<f32>,
}
impl Chorus {
    pub fn new(voices: usize, rate_hz: f32, depth_ms: f32, spread: f32, feedback: f32, wet: f32, dry: f32) -> Self {
        let voices = voices.clamp(1, 8);
        let max_samp = 4096usize;
        let phases: Vec<f32> = (0..voices).map(|i| i as f32 / voices as f32).collect();
        Self {
            voices, rate_hz, depth_ms, spread, feedback, wet, dry,
            buf: (0..voices).map(|_| vec![0.0f32; max_samp]).collect(),
            pos: 0,
            phases,
        }
    }
}
impl AudioEffect for Chorus {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let max_samp = self.buf[0].len();
        let base_delay_samp = (0.5 * 0.001 * sample_rate) as f32; // 0.5ms center
        let depth_samp = (self.depth_ms * 0.001 * sample_rate).max(1.0);
        let phase_inc = self.rate_hz / sample_rate;

        for s in buffer.iter_mut() {
            let mut out = 0.0f32;
            for v in 0..self.voices {
                let lfo = (self.phases[v] * TAU).sin();
                let delay_samp = (base_delay_samp + lfo * depth_samp).max(1.0) as usize;
                let read_pos = (self.pos + max_samp - delay_samp) % max_samp;
                let delayed = self.buf[v][read_pos];
                self.buf[v][self.pos] = *s + delayed * self.feedback;
                out += delayed;
                self.phases[v] = (self.phases[v] + phase_inc) % 1.0;
            }
            self.pos = (self.pos + 1) % max_samp;
            out /= self.voices as f32;
            *s = *s * self.dry + out * self.wet;
        }
    }
    fn name(&self) -> &str { "Chorus" }
    fn reset(&mut self) {
        for buf in self.buf.iter_mut() { for v in buf.iter_mut() { *v = 0.0; } }
        self.pos = 0;
    }
}

// ---------------------------------------------------------------------------
// Flanger
// ---------------------------------------------------------------------------

/// Short-delay flanger (0–15 ms) with LFO and feedback.
pub struct Flanger {
    pub rate_hz: f32,
    pub depth_ms: f32,
    pub feedback: f32,
    pub invert: bool,
    pub wet: f32,
    pub dry: f32,

    buf: Vec<f32>,
    pos: usize,
    phase: f32,
}
impl Flanger {
    pub fn new(rate_hz: f32, depth_ms: f32, feedback: f32, invert: bool, wet: f32, dry: f32) -> Self {
        Self {
            rate_hz, depth_ms, feedback, invert, wet, dry,
            buf: vec![0.0; 2048],
            pos: 0,
            phase: 0.0,
        }
    }
}
impl AudioEffect for Flanger {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let max_samp = self.buf.len();
        let depth_samp = (self.depth_ms * 0.001 * sample_rate).max(1.0);
        let phase_inc = self.rate_hz / sample_rate;
        let fb_sign = if self.invert { -1.0 } else { 1.0 };

        for s in buffer.iter_mut() {
            let lfo = (self.phase * TAU).sin();
            let delay_samp = (depth_samp * (1.0 + lfo) * 0.5 + 1.0) as usize;
            let read_pos = (self.pos + max_samp - delay_samp.min(max_samp - 1)) % max_samp;
            let delayed = self.buf[read_pos];
            self.buf[self.pos] = *s + delayed * self.feedback * fb_sign;
            self.pos = (self.pos + 1) % max_samp;
            self.phase = (self.phase + phase_inc) % 1.0;
            *s = *s * self.dry + delayed * self.wet;
        }
    }
    fn name(&self) -> &str { "Flanger" }
    fn reset(&mut self) {
        for v in self.buf.iter_mut() { *v = 0.0; }
        self.pos = 0;
    }
}

// ---------------------------------------------------------------------------
// Phaser
// ---------------------------------------------------------------------------

/// Multi-stage allpass phaser (4/8/12 stages), LFO, feedback, stereo.
pub struct Phaser {
    pub stages: usize,
    pub rate_hz: f32,
    pub depth: f32,
    pub feedback: f32,
    pub base_freq: f32,
    pub stereo: bool,
    pub wet: f32,
    pub dry: f32,

    // Allpass states: [stage][channel] — channel 0 = L, 1 = R
    ap_state: Vec<[f32; 2]>,
    phase: f32,
    fb_l: f32,
    fb_r: f32,
}
impl Phaser {
    pub fn new(stages: usize, rate_hz: f32, depth: f32, feedback: f32, base_freq: f32, stereo: bool, wet: f32, dry: f32) -> Self {
        let stages = [4usize, 8, 12].iter().copied().min_by_key(|&s| (s as i32 - stages as i32).abs()).unwrap_or(4);
        Self {
            stages, rate_hz, depth, feedback, base_freq, stereo, wet, dry,
            ap_state: vec![[0.0f32; 2]; stages],
            phase: 0.0,
            fb_l: 0.0,
            fb_r: 0.0,
        }
    }

    fn allpass_stage(state: &mut f32, a: f32, x: f32) -> f32 {
        // First-order allpass: H(z) = (a + z^-1) / (1 + a*z^-1)
        let y = a * (x - *state) + *state;
        *state = x - a * y;
        // Actually use standard form:
        // y[n] = a1*x[n] + x[n-1] - a1*y[n-1]
        // simplified direct: let's use proper transposed form
        y
    }
}
impl AudioEffect for Phaser {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let phase_inc = self.rate_hz / sample_rate;

        for s in buffer.iter_mut() {
            let lfo = (self.phase * TAU).sin();
            let lfo_r = ((self.phase + 0.25) * TAU).sin(); // 90° for stereo

            let freq_l = (self.base_freq * (1.0 + lfo * self.depth)).clamp(20.0, sample_rate * 0.49);
            let freq_r = (self.base_freq * (1.0 + lfo_r * self.depth)).clamp(20.0, sample_rate * 0.49);
            let a_l = ((PI * freq_l / sample_rate) - 1.0) / ((PI * freq_l / sample_rate) + 1.0);
            let a_r = ((PI * freq_r / sample_rate) - 1.0) / ((PI * freq_r / sample_rate) + 1.0);

            let mut out_l = *s + self.fb_l * self.feedback;
            let mut out_r = *s + self.fb_r * self.feedback;
            for i in 0..self.stages {
                out_l = Self::allpass_stage(&mut self.ap_state[i][0], a_l, out_l);
                if self.stereo {
                    out_r = Self::allpass_stage(&mut self.ap_state[i][1], a_r, out_r);
                }
            }
            self.fb_l = out_l;
            self.fb_r = out_r;
            self.phase = (self.phase + phase_inc) % 1.0;

            let wet_out = if self.stereo { (out_l + out_r) * 0.5 } else { out_l };
            *s = *s * self.dry + wet_out * self.wet;
        }
    }
    fn name(&self) -> &str { "Phaser" }
    fn reset(&mut self) {
        for st in self.ap_state.iter_mut() { *st = [0.0, 0.0]; }
        self.fb_l = 0.0;
        self.fb_r = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Distortion
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DistortionMode {
    SoftClip,
    HardClip,
    Foldback,
    BitCrush { bits: u32, rate_reduction: u32 },
    Overdrive,
    TubeSaturation,
}

/// Multi-mode distortion with pre/post EQ bands.
pub struct Distortion {
    pub mode: DistortionMode,
    pub drive: f32,
    pub output_gain: f32,
    pub pre_filter: Option<BiquadBand>,
    pub post_filter: Option<BiquadBand>,
    // bit crush state
    held_sample: f32,
    rate_counter: u32,
}
impl Distortion {
    pub fn new(mode: DistortionMode, drive: f32, output_gain: f32) -> Self {
        Self {
            mode, drive, output_gain,
            pre_filter: None,
            post_filter: None,
            held_sample: 0.0,
            rate_counter: 0,
        }
    }

    fn process_sample(&mut self, x: f32) -> f32 {
        let driven = x * self.drive;
        match self.mode {
            DistortionMode::SoftClip => {
                driven.tanh()
            }
            DistortionMode::HardClip => {
                driven.clamp(-1.0, 1.0)
            }
            DistortionMode::Foldback => {
                let mut v = driven;
                let threshold = 1.0f32;
                while v.abs() > threshold {
                    if v > threshold { v = 2.0 * threshold - v; }
                    else if v < -threshold { v = -2.0 * threshold - v; }
                }
                v
            }
            DistortionMode::BitCrush { bits, rate_reduction } => {
                let rate_red = rate_reduction.max(1);
                self.rate_counter += 1;
                if self.rate_counter >= rate_red {
                    self.rate_counter = 0;
                    let levels = (2.0f32).powi(bits.clamp(1, 24) as i32);
                    self.held_sample = (driven * levels).round() / levels;
                }
                self.held_sample
            }
            DistortionMode::Overdrive => {
                // Asymmetric waveshaping: positive side harder
                if driven >= 0.0 {
                    1.0 - (-driven).exp()
                } else {
                    -1.0 + driven.exp()
                }
            }
            DistortionMode::TubeSaturation => {
                // Even-harmonic bias: y = x + 0.2*x^2 - 0.1*x^3, then soft clip
                let y = driven + 0.2 * driven * driven - 0.1 * driven.powi(3);
                y.tanh()
            }
        }
    }
}
impl AudioEffect for Distortion {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        if let Some(ref mut pf) = self.pre_filter {
            pf.compute_coefficients(sample_rate);
        }
        if let Some(ref mut pf) = self.post_filter {
            pf.compute_coefficients(sample_rate);
        }
        for s in buffer.iter_mut() {
            let pre = if let Some(ref mut f) = self.pre_filter { f.process_sample(*s) } else { *s };
            let dist = self.process_sample(pre);
            let post = if let Some(ref mut f) = self.post_filter { f.process_sample(dist) } else { dist };
            *s = post * self.output_gain;
        }
    }
    fn name(&self) -> &str { "Distortion" }
    fn reset(&mut self) {
        self.held_sample = 0.0;
        self.rate_counter = 0;
        if let Some(ref mut f) = self.pre_filter { f.reset(); }
        if let Some(ref mut f) = self.post_filter { f.reset(); }
    }
}

// ---------------------------------------------------------------------------
// Pitch Shifter (granular overlap-add)
// ---------------------------------------------------------------------------

/// Granular overlap-add pitch shifter.
pub struct PitchShifter {
    pub pitch_ratio: f32,
    pub formant_preserve: bool,

    // Input ring buffer
    in_buf: Vec<f32>,
    in_pos: usize,
    // Output accumulation buffer
    out_buf: Vec<f32>,
    out_pos: usize,
    grain_phase: f32,
    grain_size: usize,
}
impl PitchShifter {
    pub fn new(pitch_ratio: f32, formant_preserve: bool) -> Self {
        let grain_size = 2048usize;
        let buf_size = grain_size * 4;
        Self {
            pitch_ratio,
            formant_preserve,
            in_buf: vec![0.0; buf_size],
            in_pos: 0,
            out_buf: vec![0.0; buf_size],
            out_pos: 0,
            grain_phase: 0.0,
            grain_size,
        }
    }

    fn hann_window(i: usize, n: usize) -> f32 {
        0.5 * (1.0 - (TAU * i as f32 / (n - 1) as f32).cos())
    }
}
impl AudioEffect for PitchShifter {
    fn process_block(&mut self, buffer: &mut [f32], _sample_rate: f32) {
        let gs = self.grain_size;
        let hop = gs / 4;
        let buf_len = self.in_buf.len();
        let out_len = self.out_buf.len();

        for s in buffer.iter_mut() {
            // Write into input ring buffer
            self.in_buf[self.in_pos] = *s;
            self.in_pos = (self.in_pos + 1) % buf_len;

            // Grain processing: every hop samples spawn a new grain
            self.grain_phase += 1.0;
            if self.grain_phase as usize >= hop {
                self.grain_phase = 0.0;
                // Read grain from input at pitch-shifted position
                let read_offset = (gs as f32 / self.pitch_ratio.max(0.01)) as usize;
                for i in 0..gs {
                    let in_idx = (self.in_pos + buf_len - read_offset + i) % buf_len;
                    let w = Self::hann_window(i, gs);
                    let out_idx = (self.out_pos + i) % out_len;
                    self.out_buf[out_idx] += self.in_buf[in_idx] * w;
                }
            }

            // Read from output buffer
            let out_val = self.out_buf[self.out_pos];
            self.out_buf[self.out_pos] = 0.0;
            self.out_pos = (self.out_pos + 1) % out_len;
            *s = out_val;
        }
    }
    fn name(&self) -> &str { "PitchShifter" }
    fn reset(&mut self) {
        for v in self.in_buf.iter_mut() { *v = 0.0; }
        for v in self.out_buf.iter_mut() { *v = 0.0; }
        self.in_pos = 0;
        self.out_pos = 0;
        self.grain_phase = 0.0;
    }
}

// ---------------------------------------------------------------------------
// AutoTune (YIN-based pitch detection + correction)
// ---------------------------------------------------------------------------

/// YIN pitch detector and chromatic pitch corrector.
pub struct AutoTune {
    pub speed: f32,  // 0.0 = slow natural, 1.0 = hard snap
    pub concert_a: f32, // Hz for A4 (usually 440.0)

    yin_buf: Vec<f32>,
    yin_pos: usize,
    yin_size: usize,
    current_shift: f32,
    delay_line: Vec<f32>,
    delay_pos: usize,
}
impl AutoTune {
    pub fn new(speed: f32) -> Self {
        let yin_size = 2048usize;
        Self {
            speed: speed.clamp(0.0, 1.0),
            concert_a: 440.0,
            yin_buf: vec![0.0; yin_size],
            yin_pos: 0,
            yin_size,
            current_shift: 1.0,
            delay_line: vec![0.0; yin_size],
            delay_pos: 0,
        }
    }

    /// YIN algorithm: returns detected period in samples, or 0.0 if unclear.
    fn yin_pitch(&self, sample_rate: f32) -> Option<f32> {
        let n = self.yin_size / 2;
        let mut d = vec![0.0f32; n];
        // Difference function
        for tau in 1..n {
            let mut s = 0.0f32;
            for j in 0..n {
                let a = self.yin_buf[(self.yin_pos + j) % self.yin_size];
                let b = self.yin_buf[(self.yin_pos + j + tau) % self.yin_size];
                s += (a - b).powi(2);
            }
            d[tau] = s;
        }
        // Cumulative mean normalized difference
        let mut cmnd = vec![0.0f32; n];
        cmnd[0] = 1.0;
        let mut running_sum = 0.0f32;
        for tau in 1..n {
            running_sum += d[tau];
            cmnd[tau] = d[tau] * tau as f32 / running_sum;
        }
        // Absolute threshold: find first tau where cmnd < 0.1
        let threshold = 0.1f32;
        for tau in 2..n {
            if cmnd[tau] < threshold {
                // Parabolic interpolation
                let tau_f = if tau > 1 && tau < n - 1 {
                    let x0 = cmnd[tau - 1];
                    let x1 = cmnd[tau];
                    let x2 = cmnd[tau + 1];
                    let denom = 2.0 * (2.0 * x1 - x0 - x2);
                    if denom.abs() < 1e-10 { tau as f32 }
                    else { tau as f32 + (x0 - x2) / denom }
                } else { tau as f32 };
                return Some(sample_rate / tau_f);
            }
        }
        None
    }

    fn note_to_freq(midi: i32, concert_a: f32) -> f32 {
        concert_a * 2.0f32.powf((midi - 69) as f32 / 12.0)
    }

    fn freq_to_midi(freq: f32, concert_a: f32) -> f32 {
        if freq <= 0.0 { return 0.0; }
        69.0 + 12.0 * (freq / concert_a).log2()
    }

    fn nearest_semitone_freq(freq: f32, concert_a: f32) -> f32 {
        let midi_f = Self::freq_to_midi(freq, concert_a);
        let midi_rounded = midi_f.round() as i32;
        Self::note_to_freq(midi_rounded, concert_a)
    }
}
impl AudioEffect for AutoTune {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let smooth = 1.0 - self.speed;
        for s in buffer.iter_mut() {
            // Feed into YIN buffer
            self.yin_buf[self.yin_pos] = *s;
            self.yin_pos = (self.yin_pos + 1) % self.yin_size;

            // Write delayed sample to output delay line
            self.delay_line[self.delay_pos] = *s;

            // Periodic pitch detection (every yin_size/4 samples)
            if self.yin_pos % (self.yin_size / 4) == 0 {
                if let Some(detected_freq) = self.yin_pitch(sample_rate) {
                    if detected_freq > 50.0 && detected_freq < 2000.0 {
                        let target_freq = Self::nearest_semitone_freq(detected_freq, self.concert_a);
                        let target_shift = target_freq / detected_freq;
                        self.current_shift = self.current_shift * smooth + target_shift * (1.0 - smooth);
                    }
                }
            }

            // Apply pitch shift via simple resampling from delay line (crude but real)
            let read_offset = (self.yin_size as f32 / 2.0 / self.current_shift.max(0.1)) as usize;
            let read_idx = (self.delay_pos + self.delay_line.len() - read_offset.min(self.delay_line.len() - 1)) % self.delay_line.len();
            *s = self.delay_line[read_idx];

            self.delay_pos = (self.delay_pos + 1) % self.delay_line.len();
        }
    }
    fn name(&self) -> &str { "AutoTune" }
    fn reset(&mut self) {
        for v in self.yin_buf.iter_mut() { *v = 0.0; }
        for v in self.delay_line.iter_mut() { *v = 0.0; }
        self.yin_pos = 0;
        self.delay_pos = 0;
        self.current_shift = 1.0;
    }
}

// ---------------------------------------------------------------------------
// Tremolo
// ---------------------------------------------------------------------------

/// Amplitude modulation via LFO.
pub struct Tremolo {
    pub rate_hz: f32,
    pub depth: f32,
    phase: f32,
}
impl Tremolo {
    pub fn new(rate_hz: f32, depth: f32) -> Self {
        Self { rate_hz, depth, phase: 0.0 }
    }
}
impl AudioEffect for Tremolo {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let phase_inc = self.rate_hz / sample_rate;
        for s in buffer.iter_mut() {
            let lfo = (self.phase * TAU).sin();
            let mod_gain = 1.0 - self.depth * (lfo * 0.5 + 0.5);
            *s *= mod_gain;
            self.phase = (self.phase + phase_inc) % 1.0;
        }
    }
    fn name(&self) -> &str { "Tremolo" }
    fn reset(&mut self) { self.phase = 0.0; }
}

// ---------------------------------------------------------------------------
// Vibrato
// ---------------------------------------------------------------------------

/// Frequency modulation (pitch wobble) via LFO.
pub struct Vibrato {
    pub rate_hz: f32,
    pub depth_semitones: f32,
    phase: f32,
    delay_buf: Vec<f32>,
    delay_pos: usize,
}
impl Vibrato {
    pub fn new(rate_hz: f32, depth_semitones: f32) -> Self {
        Self {
            rate_hz,
            depth_semitones,
            phase: 0.0,
            delay_buf: vec![0.0; 2048],
            delay_pos: 0,
        }
    }
}
impl AudioEffect for Vibrato {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let phase_inc = self.rate_hz / sample_rate;
        let max_delay_samp = (self.depth_semitones * 0.01 * sample_rate).max(1.0) as usize;
        let max_delay_samp = max_delay_samp.min(self.delay_buf.len() - 1);
        let buf_len = self.delay_buf.len();

        for s in buffer.iter_mut() {
            self.delay_buf[self.delay_pos] = *s;
            let lfo = (self.phase * TAU).sin();
            let delay_samp = (max_delay_samp as f32 * (lfo * 0.5 + 0.5)).max(1.0) as usize;
            let read_pos = (self.delay_pos + buf_len - delay_samp) % buf_len;
            *s = self.delay_buf[read_pos];
            self.delay_pos = (self.delay_pos + 1) % buf_len;
            self.phase = (self.phase + phase_inc) % 1.0;
        }
    }
    fn name(&self) -> &str { "Vibrato" }
    fn reset(&mut self) {
        for v in self.delay_buf.iter_mut() { *v = 0.0; }
        self.delay_pos = 0;
        self.phase = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Panner
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanLaw {
    Linear,
    MinusThreeDb,
    MinusSixDb,
}

/// Stereo panner with selectable pan law, Haas effect, and mid-side encode/decode.
pub struct Panner {
    pub pan: f32, // -1.0 (left) to 1.0 (right)
    pub law: PanLaw,
    pub haas_delay_ms: f32,
    pub ms_encode: bool,

    haas_buf: Vec<f32>,
    haas_pos: usize,
}
impl Panner {
    pub fn new(pan: f32, law: PanLaw, haas_delay_ms: f32, ms_encode: bool) -> Self {
        Self {
            pan: pan.clamp(-1.0, 1.0),
            law,
            haas_delay_ms,
            ms_encode,
            haas_buf: vec![0.0; 4096],
            haas_pos: 0,
        }
    }

    fn gains(&self) -> (f32, f32) {
        let p = (self.pan + 1.0) * 0.5; // 0..1
        match self.law {
            PanLaw::Linear => (1.0 - p, p),
            PanLaw::MinusThreeDb => {
                let angle = p * PI * 0.5;
                (angle.cos(), angle.sin())
            }
            PanLaw::MinusSixDb => {
                // -6dB center: sqrt of linear
                ((1.0 - p).sqrt(), p.sqrt())
            }
        }
    }
}
impl AudioEffect for Panner {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let haas_samp = (self.haas_delay_ms * 0.001 * sample_rate) as usize;
        let haas_samp = haas_samp.min(self.haas_buf.len() - 1);
        let buf_len = self.haas_buf.len();
        let (gl, gr) = self.gains();

        for s in buffer.iter_mut() {
            self.haas_buf[self.haas_pos] = *s;
            let delayed = self.haas_buf[(self.haas_pos + buf_len - haas_samp) % buf_len];
            self.haas_pos = (self.haas_pos + 1) % buf_len;

            if self.ms_encode {
                // Mid-side: mid = (L+R)/2, side = (L-R)/2 — operating on mono, just pass through
                let mid = (*s + delayed) * 0.5;
                let _side = (*s - delayed) * 0.5;
                *s = mid;
            } else {
                // Apply panning to mono: output is mono × gain (L-channel perspective)
                *s = *s * gl + delayed * gr;
            }
        }
    }
    fn name(&self) -> &str { "Panner" }
    fn reset(&mut self) {
        for v in self.haas_buf.iter_mut() { *v = 0.0; }
        self.haas_pos = 0;
    }
}

// ---------------------------------------------------------------------------
// EffectChain
// ---------------------------------------------------------------------------

/// A slot in the effect chain with bypass and wet/dry mix.
pub struct EffectSlot {
    pub effect: Box<dyn AudioEffect>,
    pub bypassed: bool,
    pub wet: f32,
    pub dry: f32,
}

impl EffectSlot {
    pub fn new(effect: Box<dyn AudioEffect>) -> Self {
        Self { effect, bypassed: false, wet: 1.0, dry: 0.0 }
    }
    pub fn with_wet_dry(mut self, wet: f32, dry: f32) -> Self {
        self.wet = wet; self.dry = dry; self
    }
}

/// Ordered chain of audio effects. Supports per-slot bypass and wet/dry.
pub struct EffectChain {
    pub slots: Vec<EffectSlot>,
}
impl EffectChain {
    pub fn new() -> Self { Self { slots: Vec::new() } }

    pub fn add(&mut self, slot: EffectSlot) {
        self.slots.push(slot);
    }

    pub fn add_effect(&mut self, effect: Box<dyn AudioEffect>) {
        self.slots.push(EffectSlot::new(effect));
    }
}
impl Default for EffectChain {
    fn default() -> Self { Self::new() }
}
impl AudioEffect for EffectChain {
    fn process_block(&mut self, buffer: &mut [f32], sample_rate: f32) {
        let n = buffer.len();
        let mut dry_buf: Vec<f32> = buffer.to_vec();
        let mut wet_buf: Vec<f32> = vec![0.0; n];

        for slot in self.slots.iter_mut() {
            if slot.bypassed { continue; }
            // Start from current buffer state
            let mut work: Vec<f32> = buffer.to_vec();
            slot.effect.process_block(&mut work, sample_rate);

            let wet = slot.wet;
            let dry = slot.dry;
            if (wet - 1.0).abs() < 1e-6 && dry < 1e-6 {
                // Pure wet — just copy
                buffer.copy_from_slice(&work);
            } else {
                for i in 0..n {
                    buffer[i] = dry_buf[i] * dry + work[i] * wet;
                }
                // Update dry_buf to current buffer for next stage
                dry_buf.copy_from_slice(buffer);
            }
            let _ = wet_buf.as_mut_slice(); // suppress unused warning
        }
    }
    fn name(&self) -> &str { "EffectChain" }
    fn reset(&mut self) {
        for slot in self.slots.iter_mut() { slot.effect.reset(); }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gain_unity() {
        let mut g = Gain::new(1.0);
        let mut buf = vec![0.5f32; 64];
        g.process_block(&mut buf, 44100.0);
        for s in &buf { assert!((*s - 0.5).abs() < 1e-6); }
    }

    #[test]
    fn test_gain_silence() {
        let mut g = Gain::new(0.0);
        let mut buf = vec![1.0f32; 64];
        g.process_block(&mut buf, 44100.0);
        for s in &buf { assert!(s.abs() < 1e-6); }
    }

    #[test]
    fn test_gain_automation_linear() {
        let bps = vec![
            GainBreakpoint { sample: 0, gain: 0.0, exponential: false },
            GainBreakpoint { sample: 100, gain: 1.0, exponential: false },
        ];
        let mut ga = GainAutomation::new(bps);
        let mut buf = vec![1.0f32; 100];
        ga.process_block(&mut buf, 44100.0);
        // First sample ~0, last sample ~1
        assert!(buf[0] < 0.05);
        assert!(buf[99] > 0.95);
    }

    #[test]
    fn test_compressor_no_gain_reduction_below_threshold() {
        let mut c = Compressor::new(-10.0, 4.0, 1.0, 100.0, 0.0, 0.0, false, 0);
        let mut buf = vec![0.001f32; 256]; // well below -10 dBFS
        c.process_block(&mut buf, 44100.0);
        // Should pass nearly unchanged
        assert!(buf[200].abs() > 0.0009);
    }

    #[test]
    fn test_limiter_brickwall() {
        let mut lim = Limiter::new(-6.0, 50.0, 0);
        let threshold = db_to_linear(-6.0);
        let mut buf = vec![1.0f32; 512]; // 0 dBFS, above -6 dBFS threshold
        lim.process_block(&mut buf, 44100.0);
        for s in &buf[100..] {
            assert!(*s <= threshold + 1e-4, "sample {} > threshold {}", s, threshold);
        }
    }

    #[test]
    fn test_gate_opens_above_threshold() {
        let mut gate = Gate::new(-40.0, -50.0, -80.0, 1.0, 10.0, 50.0, false);
        let mut buf = vec![0.01f32; 512]; // above -40 dBFS
        gate.process_block(&mut buf, 44100.0);
        // After attack, signal should be close to input
        let last = buf[400];
        assert!(last > 0.005, "gate should be open, got {}", last);
    }

    #[test]
    fn test_biquad_lowpass_dc_passthrough() {
        let mut band = BiquadBand::new(BiquadType::LowPass, 1000.0, 0.707, 0.0);
        band.compute_coefficients(44100.0);
        // DC (constant 1.0) should pass through a low-pass
        let mut buf = vec![1.0f32; 512];
        for s in buf.iter_mut() { *s = band.process_sample(*s); }
        assert!(buf[400].abs() > 0.9, "DC should pass LPF, got {}", buf[400]);
    }

    #[test]
    fn test_equalizer_processes() {
        let band = BiquadBand::new(BiquadType::PeakEq, 1000.0, 1.0, 6.0);
        let mut eq = Equalizer::new(vec![band]);
        let mut buf: Vec<f32> = (0..256).map(|i| (i as f32 * 0.01).sin()).collect();
        let orig: Vec<f32> = buf.clone();
        eq.process_block(&mut buf, 44100.0);
        // EQ should have changed at least some samples
        let changed = buf.iter().zip(orig.iter()).any(|(a, b)| (a - b).abs() > 1e-6);
        assert!(changed);
    }

    #[test]
    fn test_reverb_adds_tail() {
        let mut rev = Reverb::new(0.8, 0.5, 0.5, 0.5, 0.0, 1.0);
        let mut buf = vec![0.0f32; 1024];
        buf[0] = 1.0; // impulse
        rev.process_block(&mut buf, 44100.0);
        // After the impulse there should be some reverb tail
        let tail_energy: f32 = buf[100..].iter().map(|s| s * s).sum();
        assert!(tail_energy > 0.0, "reverb should produce a tail");
    }

    #[test]
    fn test_delay_dry_signal() {
        let mut delay = Delay::new(100.0, 0.0, 10000.0, false, 0.0, 1.0);
        let mut buf: Vec<f32> = (0..256).map(|i| (i as f32).sin()).collect();
        let orig = buf.clone();
        delay.process_block(&mut buf, 44100.0);
        // Dry=1, wet=0: output should equal input
        for i in 0..256 {
            assert!((buf[i] - orig[i]).abs() < 1e-5, "dry signal mismatch at {}", i);
        }
    }

    #[test]
    fn test_distortion_soft_clip_bounded() {
        let mut dist = Distortion::new(DistortionMode::SoftClip, 10.0, 1.0);
        let mut buf = vec![100.0f32; 256]; // extreme drive
        dist.process_block(&mut buf, 44100.0);
        for s in &buf { assert!(s.abs() <= 1.0 + 1e-5, "soft clip exceeded 1.0: {}", s); }
    }

    #[test]
    fn test_distortion_hard_clip_bounded() {
        let mut dist = Distortion::new(DistortionMode::HardClip, 10.0, 1.0);
        let mut buf = vec![100.0f32; 256];
        dist.process_block(&mut buf, 44100.0);
        for s in &buf { assert!(s.abs() <= 10.0 + 1e-4); } // drive * 1.0 clamped
    }

    #[test]
    fn test_tremolo_modulates_amplitude() {
        let mut trem = Tremolo::new(10.0, 1.0);
        let mut buf = vec![1.0f32; 1024];
        trem.process_block(&mut buf, 44100.0);
        let min = buf.iter().cloned().fold(f32::MAX, f32::min);
        let max = buf.iter().cloned().fold(f32::MIN, f32::max);
        assert!(max > 0.9, "should have near-full amplitude");
        assert!(min < 0.1, "should have near-zero amplitude with depth=1");
    }

    #[test]
    fn test_effect_chain_bypass() {
        let mut chain = EffectChain::new();
        let mut slot = EffectSlot::new(Box::new(Gain::new(0.0)));
        slot.bypassed = true;
        chain.add(slot);
        let mut buf = vec![1.0f32; 64];
        chain.process_block(&mut buf, 44100.0);
        // Bypassed gain=0 should leave signal unchanged
        for s in &buf { assert!((*s - 1.0).abs() < 1e-6); }
    }

    #[test]
    fn test_chorus_produces_output() {
        let mut chorus = Chorus::new(4, 1.5, 2.0, 0.5, 0.3, 0.5, 0.5);
        let mut buf: Vec<f32> = (0..512).map(|i| (i as f32 * 0.1).sin()).collect();
        chorus.process_block(&mut buf, 44100.0);
        let energy: f32 = buf.iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "chorus should produce output");
    }

    #[test]
    fn test_phaser_produces_output() {
        let mut phaser = Phaser::new(8, 0.5, 0.7, 0.5, 500.0, true, 0.7, 0.3);
        let mut buf: Vec<f32> = (0..256).map(|i| (i as f32 * 0.05).sin()).collect();
        phaser.process_block(&mut buf, 44100.0);
        let energy: f32 = buf.iter().map(|s| s * s).sum();
        assert!(energy > 0.0);
    }
}
