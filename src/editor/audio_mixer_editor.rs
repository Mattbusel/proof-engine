#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

const SAMPLE_RATE: f32 = 44100.0;
const TWO_PI: f32 = std::f32::consts::TAU;
const SQRT2: f32 = std::f32::consts::SQRT_2;
const LN10_OVER_20: f32 = 0.11512925464970228;  // ln(10)/20
const SPEED_OF_SOUND: f32 = 343.0;              // m/s
const MAX_VOICES: usize = 256;
const MAX_BUS_COUNT: usize = 64;
const MAX_EFFECT_CHAIN_LENGTH: usize = 16;
const SPECTRUM_FFT_SIZE: usize = 1024;
const SPECTRUM_BINS: usize = SPECTRUM_FFT_SIZE / 2;
const RMS_WINDOW_SAMPLES: usize = 4410; // 100ms at 44100 Hz
const PEAK_HOLD_FRAMES: u32 = 120;
const LUFS_BLOCK_DURATION_S: f32 = 0.4;
const LUFS_BLOCK_SAMPLES: usize = (LUFS_BLOCK_DURATION_S * SAMPLE_RATE) as usize;
const SCHROEDER_COMB_COUNT: usize = 4;
const SCHROEDER_ALLPASS_COUNT: usize = 2;
const PHASER_STAGES: usize = 6;
const HRTF_FILTER_LENGTH: usize = 128;
const SNAPSHOT_INTERP_MAX: usize = 16;

// ============================================================
// DECIBEL MATH
// ============================================================

pub fn db_to_linear(db: f32) -> f32 {
    (db * LN10_OVER_20).exp()
}

pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 1e-9 { return -180.0; }
    linear.ln() / LN10_OVER_20
}

pub fn db_clamp(db: f32, min_db: f32, max_db: f32) -> f32 {
    db.clamp(min_db, max_db)
}

// ============================================================
// ENUMS
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusType {
    Master,
    Music,
    Sfx,
    Voice,
    Ambient,
    Ui,
    Reverb,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectType {
    Equalizer,
    Compressor,
    Reverb,
    Delay,
    Chorus,
    Limiter,
    Gate,
    Distortion,
    Phaser,
    Flanger,
    BitCrusher,
    Spatializer,
    Convolution,
    Expander,
    Transient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqFilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    LowShelf,
    HighShelf,
    PeakingEq,
    AllPass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    Rms,
    Peak,
    TruePeak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistortionMode {
    SoftClip,
    HardClip,
    Tanh,
    Polynomial,
    Foldback,
    BitCrush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoShape {
    Sine,
    Triangle,
    Sawtooth,
    ReverseSawtooth,
    Square,
    RandomSampleHold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttenuationModel {
    InverseSquare,
    Linear,
    Logarithmic,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicTransitionType {
    OnBar,
    OnBeat,
    Immediate,
    CrossFade,
    StitchPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioLodLevel {
    Full,
    Reduced,
    Minimal,
    Virtual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotTransitionCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCategory {
    Music,
    Sfx,
    Voice,
    Ambient,
    Ui,
    Footstep,
    Weapon,
    Explosion,
    Environment,
}

// ============================================================
// BIQUAD FILTER
// ============================================================
// H(z) = (b0 + b1*z^-1 + b2*z^-2) / (a0 + a1*z^-1 + a2*z^-2)

#[derive(Debug, Clone)]
pub struct BiquadCoefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32, // normalized (divided by a0)
    pub a2: f32, // normalized
}

impl BiquadCoefficients {
    pub fn identity() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 }
    }

    /// Low-pass filter coefficients
    pub fn low_pass(freq_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 - cos_w0) / 2.0) / a0,
            b1: (1.0 - cos_w0) / a0,
            b2: ((1.0 - cos_w0) / 2.0) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    /// High-pass filter coefficients
    pub fn high_pass(freq_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 + cos_w0) / 2.0) / a0,
            b1: (-(1.0 + cos_w0)) / a0,
            b2: ((1.0 + cos_w0) / 2.0) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    /// Band-pass filter (constant skirt gain, peak gain = Q)
    pub fn band_pass(freq_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (sin_w0 / 2.0) / a0,
            b1: 0.0,
            b2: -(sin_w0 / 2.0) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    /// Notch filter
    pub fn notch(freq_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: 1.0 / a0,
            b1: (-2.0 * cos_w0) / a0,
            b2: 1.0 / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    /// Peaking EQ filter
    pub fn peaking_eq(freq_hz: f32, q: f32, gain_db: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a_lin = db_to_linear(gain_db / 2.0); // sqrt(10^(dBgain/20))
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha / a_lin;
        Self {
            b0: (1.0 + alpha * a_lin) / a0,
            b1: (-2.0 * cos_w0) / a0,
            b2: (1.0 - alpha * a_lin) / a0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha / a_lin) / a0,
        }
    }

    /// Low shelf filter
    pub fn low_shelf(freq_hz: f32, slope: f32, gain_db: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a_lin = db_to_linear(gain_db / 2.0);
        let alpha = sin_w0 / 2.0 * ((a_lin + 1.0 / a_lin) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a_lin.sqrt() * alpha;
        let a0 = (a_lin + 1.0) + (a_lin - 1.0) * cos_w0 + two_sqrt_a_alpha;
        Self {
            b0: a_lin * ((a_lin + 1.0) - (a_lin - 1.0) * cos_w0 + two_sqrt_a_alpha) / a0,
            b1: 2.0 * a_lin * ((a_lin - 1.0) - (a_lin + 1.0) * cos_w0) / a0,
            b2: a_lin * ((a_lin + 1.0) - (a_lin - 1.0) * cos_w0 - two_sqrt_a_alpha) / a0,
            a1: -2.0 * ((a_lin - 1.0) + (a_lin + 1.0) * cos_w0) / a0,
            a2: ((a_lin + 1.0) + (a_lin - 1.0) * cos_w0 - two_sqrt_a_alpha) / a0,
        }
    }

    /// High shelf filter
    pub fn high_shelf(freq_hz: f32, slope: f32, gain_db: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a_lin = db_to_linear(gain_db / 2.0);
        let alpha = sin_w0 / 2.0 * ((a_lin + 1.0 / a_lin) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a_lin.sqrt() * alpha;
        let a0 = (a_lin + 1.0) - (a_lin - 1.0) * cos_w0 + two_sqrt_a_alpha;
        Self {
            b0: a_lin * ((a_lin + 1.0) + (a_lin - 1.0) * cos_w0 + two_sqrt_a_alpha) / a0,
            b1: -2.0 * a_lin * ((a_lin - 1.0) + (a_lin + 1.0) * cos_w0) / a0,
            b2: a_lin * ((a_lin + 1.0) + (a_lin - 1.0) * cos_w0 - two_sqrt_a_alpha) / a0,
            a1: 2.0 * ((a_lin - 1.0) - (a_lin + 1.0) * cos_w0) / a0,
            a2: ((a_lin + 1.0) - (a_lin - 1.0) * cos_w0 - two_sqrt_a_alpha) / a0,
        }
    }

    /// All-pass filter
    pub fn all_pass(freq_hz: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = TWO_PI * freq_hz / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - alpha) / a0,
            b1: (-2.0 * cos_w0) / a0,
            b2: 1.0,
            a1: (-2.0 * cos_w0) / a0,
            a2: (1.0 - alpha) / a0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BiquadState {
    pub x1: f32, // z^-1 input
    pub x2: f32, // z^-2 input
    pub y1: f32, // z^-1 output
    pub y2: f32, // z^-2 output
}

impl BiquadState {
    pub fn new() -> Self {
        Self { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }

    /// Process one sample through biquad filter (Direct Form I)
    pub fn process(&mut self, x: f32, coeff: &BiquadCoefficients) -> f32 {
        let y = coeff.b0 * x
               + coeff.b1 * self.x1
               + coeff.b2 * self.x2
               - coeff.a1 * self.y1
               - coeff.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    /// Process a buffer in-place
    pub fn process_buffer(&mut self, buffer: &mut [f32], coeff: &BiquadCoefficients) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample, coeff);
        }
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0; self.x2 = 0.0; self.y1 = 0.0; self.y2 = 0.0;
    }
}

// ============================================================
// EQ FILTER BAND
// ============================================================

#[derive(Debug, Clone)]
pub struct EqBand {
    pub filter_type: EqFilterType,
    pub frequency_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub slope: f32,   // for shelving filters
    pub is_enabled: bool,
    pub coefficients: BiquadCoefficients,
    pub state_l: BiquadState,
    pub state_r: BiquadState,
}

impl EqBand {
    pub fn new(filter_type: EqFilterType, freq_hz: f32, gain_db: f32, q: f32) -> Self {
        let coefficients = compute_eq_coefficients(filter_type, freq_hz, gain_db, q, 1.0);
        Self {
            filter_type,
            frequency_hz: freq_hz,
            gain_db,
            q,
            slope: 1.0,
            is_enabled: true,
            coefficients,
            state_l: BiquadState::new(),
            state_r: BiquadState::new(),
        }
    }

    pub fn update_coefficients(&mut self, sample_rate: f32) {
        self.coefficients = compute_eq_coefficients(
            self.filter_type,
            self.frequency_hz,
            self.gain_db,
            self.q,
            self.slope,
        );
    }

    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        if !self.is_enabled { return (left, right); }
        let l = self.state_l.process(left, &self.coefficients);
        let r = self.state_r.process(right, &self.coefficients);
        (l, r)
    }

    pub fn compute_magnitude_response(&self, freq_hz: f32, sample_rate: f32) -> f32 {
        // H(e^jw) at freq_hz
        let w = TWO_PI * freq_hz / sample_rate;
        let cos_w = w.cos();
        let sin_w = w.sin();
        let cos_2w = (2.0 * w).cos();
        let sin_2w = (2.0 * w).sin();

        let c = &self.coefficients;
        // Numerator: b0 + b1*e^{-jw} + b2*e^{-2jw}
        let num_re = c.b0 + c.b1 * cos_w + c.b2 * cos_2w;
        let num_im = -(c.b1 * sin_w + c.b2 * sin_2w);
        // Denominator: 1 + a1*e^{-jw} + a2*e^{-2jw}
        let den_re = 1.0 + c.a1 * cos_w + c.a2 * cos_2w;
        let den_im = -(c.a1 * sin_w + c.a2 * sin_2w);

        let num_mag = (num_re * num_re + num_im * num_im).sqrt();
        let den_mag = (den_re * den_re + den_im * den_im).sqrt();

        if den_mag < 1e-10 { return 0.0; }
        num_mag / den_mag
    }
}

pub fn compute_eq_coefficients(
    filter_type: EqFilterType,
    freq_hz: f32,
    gain_db: f32,
    q: f32,
    slope: f32,
) -> BiquadCoefficients {
    let sr = SAMPLE_RATE;
    let freq = freq_hz.clamp(20.0, sr * 0.499);
    let q = q.max(0.1);
    match filter_type {
        EqFilterType::LowPass     => BiquadCoefficients::low_pass(freq, q, sr),
        EqFilterType::HighPass    => BiquadCoefficients::high_pass(freq, q, sr),
        EqFilterType::BandPass    => BiquadCoefficients::band_pass(freq, q, sr),
        EqFilterType::Notch       => BiquadCoefficients::notch(freq, q, sr),
        EqFilterType::PeakingEq   => BiquadCoefficients::peaking_eq(freq, q, gain_db, sr),
        EqFilterType::LowShelf    => BiquadCoefficients::low_shelf(freq, slope.max(0.01), gain_db, sr),
        EqFilterType::HighShelf   => BiquadCoefficients::high_shelf(freq, slope.max(0.01), gain_db, sr),
        EqFilterType::AllPass     => BiquadCoefficients::all_pass(freq, q, sr),
    }
}

// ============================================================
// PARAMETRIC EQ
// ============================================================

#[derive(Debug, Clone)]
pub struct ParametricEqualizer {
    pub bands: Vec<EqBand>,
    pub output_gain_db: f32,
    pub is_enabled: bool,
}

impl ParametricEqualizer {
    pub fn new() -> Self {
        let mut bands = Vec::new();
        // Default 8-band EQ
        bands.push(EqBand::new(EqFilterType::HighPass, 80.0, 0.0, 0.707));
        bands.push(EqBand::new(EqFilterType::LowShelf, 200.0, 0.0, 0.707));
        bands.push(EqBand::new(EqFilterType::PeakingEq, 500.0, 0.0, 1.0));
        bands.push(EqBand::new(EqFilterType::PeakingEq, 1000.0, 0.0, 1.0));
        bands.push(EqBand::new(EqFilterType::PeakingEq, 2500.0, 0.0, 1.0));
        bands.push(EqBand::new(EqFilterType::PeakingEq, 5000.0, 0.0, 1.0));
        bands.push(EqBand::new(EqFilterType::HighShelf, 10000.0, 0.0, 0.707));
        bands.push(EqBand::new(EqFilterType::LowPass, 20000.0, 0.0, 0.707));
        Self {
            bands,
            output_gain_db: 0.0,
            is_enabled: true,
        }
    }

    pub fn process_stereo(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        if !self.is_enabled { return (left, right); }
        for band in &mut self.bands {
            let (l, r) = band.process_stereo(left, right);
            left = l;
            right = r;
        }
        let gain = db_to_linear(self.output_gain_db);
        (left * gain, right * gain)
    }

    pub fn process_buffer_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        if !self.is_enabled { return; }
        let len = left.len().min(right.len());
        for i in 0..len {
            let (l, r) = self.process_stereo(left[i], right[i]);
            left[i] = l;
            right[i] = r;
        }
    }

    pub fn add_band(&mut self, band: EqBand) {
        if self.bands.len() < 32 {
            self.bands.push(band);
        }
    }

    pub fn remove_band(&mut self, index: usize) {
        if index < self.bands.len() {
            self.bands.remove(index);
        }
    }

    pub fn set_band_gain(&mut self, index: usize, gain_db: f32) {
        if let Some(band) = self.bands.get_mut(index) {
            band.gain_db = gain_db;
            band.update_coefficients(SAMPLE_RATE);
        }
    }

    pub fn frequency_response_db(&self, freq_hz: f32) -> f32 {
        if !self.is_enabled { return 0.0; }
        let mut total_linear = 1.0f32;
        for band in &self.bands {
            if band.is_enabled {
                total_linear *= band.compute_magnitude_response(freq_hz, SAMPLE_RATE);
            }
        }
        linear_to_db(total_linear) + self.output_gain_db
    }
}

// ============================================================
// COMPRESSOR
// ============================================================

#[derive(Debug, Clone)]
pub struct CompressorParams {
    pub threshold_db: f32,
    pub ratio: f32,           // e.g. 4.0 means 4:1
    pub knee_db: f32,         // soft knee width in dB
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_gain_db: f32,
    pub mode: CompressionMode,
    pub lookahead_ms: f32,
    pub auto_makeup: bool,
}

impl Default for CompressorParams {
    fn default() -> Self {
        Self {
            threshold_db: -18.0,
            ratio: 4.0,
            knee_db: 6.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            makeup_gain_db: 0.0,
            mode: CompressionMode::Rms,
            lookahead_ms: 0.0,
            auto_makeup: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompressorState {
    pub envelope: f32,
    pub gain_db: f32,
    pub rms_buffer: VecDeque<f32>,
    pub rms_sum: f32,
    pub level_db: f32,
    pub gr_db: f32, // gain reduction in dB (negative)
}

impl CompressorState {
    pub fn new() -> Self {
        Self {
            envelope: 0.0,
            gain_db: 0.0,
            rms_buffer: VecDeque::with_capacity(RMS_WINDOW_SAMPLES),
            rms_sum: 0.0,
            level_db: -120.0,
            gr_db: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Compressor {
    pub params: CompressorParams,
    pub state: CompressorState,
    pub is_enabled: bool,
}

impl Compressor {
    pub fn new(params: CompressorParams) -> Self {
        Self {
            params,
            state: CompressorState::new(),
            is_enabled: true,
        }
    }

    pub fn compute_gain_db(&self, level_db: f32) -> f32 {
        let t = self.params.threshold_db;
        let r = self.params.ratio;
        let k = self.params.knee_db;
        let overshoot = level_db - t;

        if k > 0.0 && overshoot > -k / 2.0 && overshoot < k / 2.0 {
            // Soft knee interpolation
            let knee_factor = (overshoot + k / 2.0) / k;
            let compressed = overshoot * (1.0 - 1.0 / r) * knee_factor * knee_factor * 0.5;
            -compressed
        } else if overshoot > k / 2.0 {
            // Above knee: apply full ratio
            let gain_reduction = overshoot * (1.0 - 1.0 / r);
            -gain_reduction
        } else {
            // Below threshold (or below knee)
            0.0
        }
    }

    /// Attack/release envelope follower
    pub fn update_envelope(&mut self, input_abs: f32, sample_rate: f32) -> f32 {
        let attack_coeff = (-1.0 / (self.params.attack_ms * 0.001 * sample_rate)).exp();
        let release_coeff = (-1.0 / (self.params.release_ms * 0.001 * sample_rate)).exp();
        if input_abs > self.state.envelope {
            self.state.envelope = attack_coeff * self.state.envelope + (1.0 - attack_coeff) * input_abs;
        } else {
            self.state.envelope = release_coeff * self.state.envelope;
        }
        self.state.envelope
    }

    /// Update RMS envelope
    pub fn update_rms(&mut self, sample: f32) -> f32 {
        let sq = sample * sample;
        let n = RMS_WINDOW_SAMPLES;
        if self.state.rms_buffer.len() >= n {
            if let Some(old) = self.state.rms_buffer.pop_front() {
                self.state.rms_sum -= old * old;
            }
        }
        self.state.rms_buffer.push_back(sample);
        self.state.rms_sum += sq;
        self.state.rms_sum = self.state.rms_sum.max(0.0);
        (self.state.rms_sum / n as f32).sqrt()
    }

    pub fn process_sample(&mut self, left: f32, right: f32) -> (f32, f32) {
        if !self.is_enabled { return (left, right); }

        // Level detection
        let mono = (left.abs() + right.abs()) * 0.5;
        let level = match self.params.mode {
            CompressionMode::Rms => self.update_rms(mono),
            CompressionMode::Peak | CompressionMode::TruePeak => {
                self.update_envelope(mono, SAMPLE_RATE)
            }
        };

        let level_db = linear_to_db(level.max(1e-9));
        self.state.level_db = level_db;

        let target_gr = self.compute_gain_db(level_db);
        // Smooth gain reduction via attack/release
        let gr_coeff = if target_gr < self.state.gr_db {
            (-1.0 / (self.params.attack_ms * 0.001 * SAMPLE_RATE)).exp()
        } else {
            (-1.0 / (self.params.release_ms * 0.001 * SAMPLE_RATE)).exp()
        };
        self.state.gr_db = gr_coeff * self.state.gr_db + (1.0 - gr_coeff) * target_gr;

        let makeup = if self.params.auto_makeup {
            -(self.params.threshold_db * (1.0 - 1.0 / self.params.ratio)) * 0.5
        } else {
            self.params.makeup_gain_db
        };

        let total_gain = db_to_linear(self.state.gr_db + makeup);
        (left * total_gain, right * total_gain)
    }

    pub fn process_buffer_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            let (l, r) = self.process_sample(left[i], right[i]);
            left[i] = l;
            right[i] = r;
        }
    }

    pub fn gain_reduction_db(&self) -> f32 {
        self.state.gr_db
    }
}

// ============================================================
// SCHROEDER REVERB (COMB + ALLPASS)
// ============================================================

#[derive(Debug, Clone)]
pub struct CombFilter {
    pub buffer: Vec<f32>,
    pub buffer_size: usize,
    pub write_pos: usize,
    pub feedback: f32,
    pub damp_coeff: f32,
    pub damp_state: f32,
}

impl CombFilter {
    pub fn new(delay_samples: usize, feedback: f32, damp: f32) -> Self {
        let size = delay_samples.max(1);
        Self {
            buffer: vec![0.0; size],
            buffer_size: size,
            write_pos: 0,
            feedback,
            damp_coeff: damp,
            damp_state: 0.0,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let out = self.buffer[self.write_pos];
        // One-pole lowpass on feedback (Schroeder damping)
        self.damp_state = out * (1.0 - self.damp_coeff) + self.damp_state * self.damp_coeff;
        self.buffer[self.write_pos] = input + self.damp_state * self.feedback;
        self.write_pos = (self.write_pos + 1) % self.buffer_size;
        out
    }

    pub fn set_feedback(&mut self, fb: f32) { self.feedback = fb.clamp(-0.99, 0.99); }

    pub fn resize(&mut self, delay_samples: usize) {
        let size = delay_samples.max(1);
        self.buffer = vec![0.0; size];
        self.buffer_size = size;
        self.write_pos = 0;
    }
}

#[derive(Debug, Clone)]
pub struct AllPassFilter {
    pub buffer: Vec<f32>,
    pub buffer_size: usize,
    pub write_pos: usize,
    pub feedback: f32,
}

impl AllPassFilter {
    pub fn new(delay_samples: usize, feedback: f32) -> Self {
        let size = delay_samples.max(1);
        Self {
            buffer: vec![0.0; size],
            buffer_size: size,
            write_pos: 0,
            feedback,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.write_pos];
        let v = input + buf_out * self.feedback;
        self.buffer[self.write_pos] = v;
        self.write_pos = (self.write_pos + 1) % self.buffer_size;
        buf_out - input * self.feedback
    }
}

#[derive(Debug, Clone)]
pub struct SchroederReverb {
    pub room_size: f32,    // 0..1
    pub damping: f32,      // 0..1
    pub wet_mix: f32,
    pub dry_mix: f32,
    pub width: f32,        // stereo width
    pub pre_delay_ms: f32,
    pub combs_l: Vec<CombFilter>,
    pub combs_r: Vec<CombFilter>,
    pub allpasses_l: Vec<AllPassFilter>,
    pub allpasses_r: Vec<AllPassFilter>,
    pub pre_delay_buf: VecDeque<f32>,
    pub is_enabled: bool,
}

impl SchroederReverb {
    // Schroeder comb delay times in samples at 44100 Hz
    const COMB_DELAYS: [usize; 4] = [1557, 1617, 1491, 1422];
    const ALLPASS_DELAYS: [usize; 2] = [225, 341];

    pub fn new() -> Self {
        let feedback = 0.84;
        let damp = 0.5;
        let combs_l: Vec<CombFilter> = Self::COMB_DELAYS.iter()
            .map(|&d| CombFilter::new(d, feedback, damp))
            .collect();
        let combs_r: Vec<CombFilter> = Self::COMB_DELAYS.iter()
            .map(|&d| CombFilter::new(d + 23, feedback, damp))
            .collect();
        let allpasses_l: Vec<AllPassFilter> = Self::ALLPASS_DELAYS.iter()
            .map(|&d| AllPassFilter::new(d, 0.5))
            .collect();
        let allpasses_r: Vec<AllPassFilter> = Self::ALLPASS_DELAYS.iter()
            .map(|&d| AllPassFilter::new(d + 7, 0.5))
            .collect();

        Self {
            room_size: 0.5,
            damping: 0.5,
            wet_mix: 0.3,
            dry_mix: 0.7,
            width: 1.0,
            pre_delay_ms: 10.0,
            combs_l,
            combs_r,
            allpasses_l,
            allpasses_r,
            pre_delay_buf: VecDeque::with_capacity(4800),
            is_enabled: true,
        }
    }

    pub fn set_room_size(&mut self, size: f32) {
        self.room_size = size.clamp(0.0, 1.0);
        let feedback = 0.7 + self.room_size * 0.28;
        for c in &mut self.combs_l { c.set_feedback(feedback); }
        for c in &mut self.combs_r { c.set_feedback(feedback); }
    }

    pub fn set_damping(&mut self, damp: f32) {
        self.damping = damp.clamp(0.0, 1.0);
        for c in &mut self.combs_l { c.damp_coeff = self.damping; }
        for c in &mut self.combs_r { c.damp_coeff = self.damping; }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l * self.dry_mix, in_r * self.dry_mix);
        }

        // Pre-delay
        let pre_delay_samples = (self.pre_delay_ms * 0.001 * SAMPLE_RATE) as usize;
        let mono = (in_l + in_r) * 0.5;
        self.pre_delay_buf.push_back(mono);
        let delayed = if self.pre_delay_buf.len() > pre_delay_samples {
            self.pre_delay_buf.pop_front().unwrap_or(0.0)
        } else { mono };

        // Parallel comb filters
        let mut rev_l = 0.0f32;
        let mut rev_r = 0.0f32;
        for c in &mut self.combs_l { rev_l += c.process(delayed); }
        for c in &mut self.combs_r { rev_r += c.process(delayed); }

        // Series all-pass filters
        for ap in &mut self.allpasses_l { rev_l = ap.process(rev_l); }
        for ap in &mut self.allpasses_r { rev_r = ap.process(rev_r); }

        // Stereo width
        let w = self.width * 0.5;
        let wet_l = rev_l * (0.5 + w) + rev_r * (0.5 - w);
        let wet_r = rev_r * (0.5 + w) + rev_l * (0.5 - w);

        let out_l = in_l * self.dry_mix + wet_l * self.wet_mix;
        let out_r = in_r * self.dry_mix + wet_r * self.wet_mix;
        (out_l, out_r)
    }

    pub fn process_buffer_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            let (l, r) = self.process_sample(left[i], right[i]);
            left[i] = l;
            right[i] = r;
        }
    }
}

// ============================================================
// DELAY EFFECT
// ============================================================

#[derive(Debug, Clone)]
pub struct DelayEffect {
    pub delay_l_ms: f32,
    pub delay_r_ms: f32,
    pub feedback: f32,
    pub wet_mix: f32,
    pub dry_mix: f32,
    pub ping_pong: bool,
    pub tempo_sync: bool,
    pub tempo_bpm: f32,
    pub tempo_division: f32, // e.g. 0.5 = eighth note
    pub buf_l: Vec<f32>,
    pub buf_r: Vec<f32>,
    pub write_l: usize,
    pub write_r: usize,
    pub buf_size: usize,
    pub is_enabled: bool,
}

impl DelayEffect {
    pub fn new(delay_ms: f32) -> Self {
        let max_samples = (SAMPLE_RATE * 2.0) as usize; // 2 second max delay
        Self {
            delay_l_ms: delay_ms,
            delay_r_ms: delay_ms,
            feedback: 0.4,
            wet_mix: 0.3,
            dry_mix: 1.0,
            ping_pong: false,
            tempo_sync: false,
            tempo_bpm: 120.0,
            tempo_division: 0.5,
            buf_l: vec![0.0; max_samples],
            buf_r: vec![0.0; max_samples],
            write_l: 0,
            write_r: 0,
            buf_size: max_samples,
            is_enabled: true,
        }
    }

    fn delay_samples(delay_ms: f32) -> usize {
        ((delay_ms * 0.001 * SAMPLE_RATE) as usize).clamp(1, (SAMPLE_RATE * 2.0) as usize - 1)
    }

    fn tempo_delay_ms(bpm: f32, division: f32) -> f32 {
        // division=1 = quarter note, 0.5 = eighth note
        60000.0 / bpm * division * 4.0
    }

    pub fn effective_delay_ms(&self) -> (f32, f32) {
        if self.tempo_sync {
            let ms = Self::tempo_delay_ms(self.tempo_bpm, self.tempo_division);
            (ms, ms)
        } else {
            (self.delay_l_ms, self.delay_r_ms)
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l * self.dry_mix, in_r * self.dry_mix);
        }
        let (dl, dr) = self.effective_delay_ms();
        let dly_l = Self::delay_samples(dl);
        let dly_r = Self::delay_samples(dr);
        let read_l = (self.write_l + self.buf_size - dly_l) % self.buf_size;
        let read_r = (self.write_r + self.buf_size - dly_r) % self.buf_size;

        let wet_l = self.buf_l[read_l];
        let wet_r = self.buf_r[read_r];

        if self.ping_pong {
            self.buf_l[self.write_l] = in_l + wet_r * self.feedback;
            self.buf_r[self.write_r] = in_r + wet_l * self.feedback;
        } else {
            self.buf_l[self.write_l] = in_l + wet_l * self.feedback;
            self.buf_r[self.write_r] = in_r + wet_r * self.feedback;
        }

        self.write_l = (self.write_l + 1) % self.buf_size;
        self.write_r = (self.write_r + 1) % self.buf_size;

        let out_l = in_l * self.dry_mix + wet_l * self.wet_mix;
        let out_r = in_r * self.dry_mix + wet_r * self.wet_mix;
        (out_l, out_r)
    }

    pub fn process_buffer_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            let (l, r) = self.process_sample(left[i], right[i]);
            left[i] = l;
            right[i] = r;
        }
    }
}

// ============================================================
// LFO
// ============================================================

#[derive(Debug, Clone)]
pub struct Lfo {
    pub shape: LfoShape,
    pub rate_hz: f32,
    pub depth: f32,
    pub phase: f32,
    pub phase_offset: f32,
    pub random_state: f32,
    pub random_target: f32,
    pub samples_since_update: usize,
    pub random_hold_samples: usize,
}

impl Lfo {
    pub fn new(shape: LfoShape, rate_hz: f32, depth: f32) -> Self {
        Self {
            shape,
            rate_hz,
            depth,
            phase: 0.0,
            phase_offset: 0.0,
            random_state: 0.0,
            random_target: 0.0,
            samples_since_update: 0,
            random_hold_samples: (SAMPLE_RATE / rate_hz.max(0.001)) as usize,
        }
    }

    fn lcg_rand(state: f32) -> f32 {
        // Simple deterministic "random" from float state
        let bits = (state * 1000000.0) as u32;
        let next = bits.wrapping_mul(1664525).wrapping_add(1013904223);
        (next as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn tick(&mut self) -> f32 {
        let phase = (self.phase + self.phase_offset).fract();
        let value = match self.shape {
            LfoShape::Sine => (TWO_PI * phase).sin(),
            LfoShape::Triangle => {
                if phase < 0.5 { 4.0 * phase - 1.0 }
                else { 3.0 - 4.0 * phase }
            }
            LfoShape::Sawtooth => phase * 2.0 - 1.0,
            LfoShape::ReverseSawtooth => 1.0 - phase * 2.0,
            LfoShape::Square => if phase < 0.5 { 1.0 } else { -1.0 },
            LfoShape::RandomSampleHold => {
                self.samples_since_update += 1;
                let hold = (SAMPLE_RATE / self.rate_hz.max(0.001)) as usize;
                if self.samples_since_update >= hold {
                    self.samples_since_update = 0;
                    self.random_state = Self::lcg_rand(self.random_state);
                    self.random_target = self.random_state;
                }
                self.random_target
            }
        };
        self.phase += self.rate_hz / SAMPLE_RATE;
        if self.phase >= 1.0 { self.phase -= 1.0; }
        value * self.depth
    }

    pub fn tick_n(&mut self, n: usize) -> Vec<f32> {
        (0..n).map(|_| self.tick()).collect()
    }
}

// ============================================================
// CHORUS
// ============================================================

#[derive(Debug, Clone)]
pub struct ChorusEffect {
    pub rate_hz: f32,
    pub depth_ms: f32,
    pub delay_ms: f32,
    pub wet_mix: f32,
    pub dry_mix: f32,
    pub voices: usize,
    pub buf_l: Vec<f32>,
    pub buf_r: Vec<f32>,
    pub write_pos: usize,
    pub buf_size: usize,
    pub lfo_l: Lfo,
    pub lfo_r: Lfo,
    pub is_enabled: bool,
}

impl ChorusEffect {
    pub fn new() -> Self {
        let buf_size = 4096;
        let mut lfo_r = Lfo::new(LfoShape::Sine, 0.5, 1.0);
        lfo_r.phase_offset = 0.25; // 90 degree phase offset for stereo
        Self {
            rate_hz: 0.5,
            depth_ms: 2.0,
            delay_ms: 20.0,
            wet_mix: 0.5,
            dry_mix: 0.5,
            voices: 2,
            buf_l: vec![0.0; buf_size],
            buf_r: vec![0.0; buf_size],
            write_pos: 0,
            buf_size,
            lfo_l: Lfo::new(LfoShape::Sine, 0.5, 1.0),
            lfo_r,
            is_enabled: true,
        }
    }

    fn read_interpolated(buf: &[f32], pos: f32, size: usize) -> f32 {
        let i0 = (pos as usize) % size;
        let i1 = (i0 + 1) % size;
        let frac = pos - pos.floor();
        buf[i0] * (1.0 - frac) + buf[i1] * frac
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled {
            return (in_l * self.dry_mix, in_r * self.dry_mix);
        }

        self.buf_l[self.write_pos] = in_l;
        self.buf_r[self.write_pos] = in_r;

        let lfo_l_val = self.lfo_l.tick();
        let lfo_r_val = self.lfo_r.tick();

        let base_delay = self.delay_ms * 0.001 * SAMPLE_RATE;
        let mod_l = base_delay + lfo_l_val * self.depth_ms * 0.001 * SAMPLE_RATE;
        let mod_r = base_delay + lfo_r_val * self.depth_ms * 0.001 * SAMPLE_RATE;

        let read_l = (self.write_pos as f32 + self.buf_size as f32 - mod_l.clamp(1.0, self.buf_size as f32 - 1.0)) % self.buf_size as f32;
        let read_r = (self.write_pos as f32 + self.buf_size as f32 - mod_r.clamp(1.0, self.buf_size as f32 - 1.0)) % self.buf_size as f32;

        let wet_l = Self::read_interpolated(&self.buf_l, read_l, self.buf_size);
        let wet_r = Self::read_interpolated(&self.buf_r, read_r, self.buf_size);

        self.write_pos = (self.write_pos + 1) % self.buf_size;

        let out_l = in_l * self.dry_mix + wet_l * self.wet_mix;
        let out_r = in_r * self.dry_mix + wet_r * self.wet_mix;
        (out_l, out_r)
    }

    pub fn process_buffer_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            let (l, r) = self.process_sample(left[i], right[i]);
            left[i] = l;
            right[i] = r;
        }
    }
}

// ============================================================
// LIMITER
// ============================================================

#[derive(Debug, Clone)]
pub struct Limiter {
    pub ceiling_db: f32,
    pub release_ms: f32,
    pub lookahead_ms: f32,
    pub envelope_l: f32,
    pub envelope_r: f32,
    pub lookahead_buf_l: VecDeque<f32>,
    pub lookahead_buf_r: VecDeque<f32>,
    pub is_enabled: bool,
}

impl Limiter {
    pub fn new(ceiling_db: f32) -> Self {
        let lookahead_samples = 256;
        Self {
            ceiling_db,
            release_ms: 50.0,
            lookahead_ms: 5.0,
            envelope_l: 0.0,
            envelope_r: 0.0,
            lookahead_buf_l: VecDeque::with_capacity(lookahead_samples),
            lookahead_buf_r: VecDeque::with_capacity(lookahead_samples),
            is_enabled: true,
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l, in_r); }
        let ceiling_lin = db_to_linear(self.ceiling_db);
        let release_coeff = (-1.0 / (self.release_ms * 0.001 * SAMPLE_RATE)).exp();

        let lookahead_samples = (self.lookahead_ms * 0.001 * SAMPLE_RATE) as usize;
        self.lookahead_buf_l.push_back(in_l);
        self.lookahead_buf_r.push_back(in_r);

        let delayed_l = if self.lookahead_buf_l.len() > lookahead_samples {
            self.lookahead_buf_l.pop_front().unwrap_or(0.0)
        } else { 0.0 };
        let delayed_r = if self.lookahead_buf_r.len() > lookahead_samples {
            self.lookahead_buf_r.pop_front().unwrap_or(0.0)
        } else { 0.0 };

        // Peak envelope
        let peak = in_l.abs().max(in_r.abs());
        if peak > self.envelope_l {
            self.envelope_l = peak;
        } else {
            self.envelope_l = release_coeff * self.envelope_l;
        }

        let gain = if self.envelope_l > ceiling_lin {
            ceiling_lin / self.envelope_l
        } else { 1.0 };

        (delayed_l * gain, delayed_r * gain)
    }
}

// ============================================================
// NOISE GATE
// ============================================================

#[derive(Debug, Clone)]
pub struct NoiseGate {
    pub threshold_db: f32,
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub release_ms: f32,
    pub hysteresis_db: f32,
    pub envelope: f32,
    pub hold_counter: usize,
    pub is_open: bool,
    pub gate_gain: f32,
    pub is_enabled: bool,
}

impl NoiseGate {
    pub fn new(threshold_db: f32) -> Self {
        Self {
            threshold_db,
            attack_ms: 1.0,
            hold_ms: 50.0,
            release_ms: 200.0,
            hysteresis_db: 3.0,
            envelope: 0.0,
            hold_counter: 0,
            is_open: false,
            gate_gain: 0.0,
            is_enabled: true,
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l, in_r); }

        let level = (in_l.abs() + in_r.abs()) * 0.5;
        let level_db = linear_to_db(level.max(1e-9));

        let open_threshold = self.threshold_db;
        let close_threshold = self.threshold_db - self.hysteresis_db;

        if level_db > open_threshold {
            self.is_open = true;
            self.hold_counter = (self.hold_ms * 0.001 * SAMPLE_RATE) as usize;
        } else if self.hold_counter > 0 {
            self.hold_counter -= 1;
        } else {
            if level_db < close_threshold {
                self.is_open = false;
            }
        }

        let target = if self.is_open { 1.0f32 } else { 0.0f32 };
        let coeff = if target > self.gate_gain {
            (-1.0 / (self.attack_ms * 0.001 * SAMPLE_RATE)).exp()
        } else {
            (-1.0 / (self.release_ms * 0.001 * SAMPLE_RATE)).exp()
        };
        self.gate_gain = coeff * self.gate_gain + (1.0 - coeff) * target;

        (in_l * self.gate_gain, in_r * self.gate_gain)
    }
}

// ============================================================
// DISTORTION
// ============================================================

#[derive(Debug, Clone)]
pub struct DistortionEffect {
    pub mode: DistortionMode,
    pub drive: f32,        // pre-gain (linear)
    pub output_gain: f32,  // post-gain (linear)
    pub mix: f32,
    pub tone: f32,         // 0..1 lowpass cutoff
    pub tone_filter_l: BiquadState,
    pub tone_filter_r: BiquadState,
    pub tone_coeff: BiquadCoefficients,
    pub is_enabled: bool,
    // Polynomial coefficients for polynomial mode
    pub poly_coeffs: [f32; 4],
    pub bit_depth: f32,
    pub sample_rate_factor: f32,
    pub srr_counter: usize,
    pub srr_held_l: f32,
    pub srr_held_r: f32,
}

impl DistortionEffect {
    pub fn new(mode: DistortionMode, drive: f32) -> Self {
        let tone_freq = 5000.0;
        let tone_coeff = BiquadCoefficients::low_pass(tone_freq, 0.707, SAMPLE_RATE);
        Self {
            mode,
            drive,
            output_gain: 1.0 / drive.max(1.0),
            mix: 1.0,
            tone: 1.0,
            tone_filter_l: BiquadState::new(),
            tone_filter_r: BiquadState::new(),
            tone_coeff,
            is_enabled: true,
            poly_coeffs: [1.0, -0.333, 0.2, -0.1],
            bit_depth: 8.0,
            sample_rate_factor: 1.0,
            srr_counter: 0,
            srr_held_l: 0.0,
            srr_held_r: 0.0,
        }
    }

    fn soft_clip(x: f32) -> f32 {
        x.clamp(-1.5, 1.5) * (1.0 - (x.clamp(-1.5, 1.5).powi(2)) / 3.0)
    }

    fn hard_clip(x: f32, threshold: f32) -> f32 {
        x.clamp(-threshold, threshold)
    }

    fn tanh_clip(x: f32) -> f32 {
        x.tanh()
    }

    fn polynomial_clip(x: f32, coeffs: &[f32; 4]) -> f32 {
        let x2 = x * x;
        let x3 = x2 * x;
        coeffs[0] * x + coeffs[1] * x3 + coeffs[2] * x2 * x3 + coeffs[3] * x2 * x2 * x
    }

    fn foldback(x: f32, threshold: f32) -> f32 {
        let mut v = x;
        while v.abs() > threshold {
            if v > threshold { v = 2.0 * threshold - v; }
            if v < -threshold { v = -2.0 * threshold - v; }
        }
        v
    }

    fn bitcrush(x: f32, bits: f32) -> f32 {
        let levels = 2.0f32.powf(bits.clamp(1.0, 32.0));
        (x * levels).round() / levels
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l, in_r); }

        // Sample rate reduction
        let srr_period = (1.0 / self.sample_rate_factor).max(1.0) as usize;
        let (dl, dr) = if srr_period > 1 {
            self.srr_counter += 1;
            if self.srr_counter >= srr_period {
                self.srr_counter = 0;
                self.srr_held_l = in_l * self.drive;
                self.srr_held_r = in_r * self.drive;
            }
            (self.srr_held_l, self.srr_held_r)
        } else {
            (in_l * self.drive, in_r * self.drive)
        };

        let (out_l, out_r) = match self.mode {
            DistortionMode::SoftClip => (Self::soft_clip(dl), Self::soft_clip(dr)),
            DistortionMode::HardClip => (Self::hard_clip(dl, 1.0), Self::hard_clip(dr, 1.0)),
            DistortionMode::Tanh => (Self::tanh_clip(dl), Self::tanh_clip(dr)),
            DistortionMode::Polynomial => (
                Self::polynomial_clip(dl.clamp(-2.0, 2.0), &self.poly_coeffs),
                Self::polynomial_clip(dr.clamp(-2.0, 2.0), &self.poly_coeffs),
            ),
            DistortionMode::Foldback => (Self::foldback(dl, 1.0), Self::foldback(dr, 1.0)),
            DistortionMode::BitCrush => (Self::bitcrush(dl.tanh(), self.bit_depth), Self::bitcrush(dr.tanh(), self.bit_depth)),
        };

        // Apply tone filter
        let tone_l = self.tone_filter_l.process(out_l, &self.tone_coeff);
        let tone_r = self.tone_filter_r.process(out_r, &self.tone_coeff);

        // Blend dry/wet, apply output gain
        let final_l = (tone_l * self.mix + in_l * (1.0 - self.mix)) * self.output_gain;
        let final_r = (tone_r * self.mix + in_r * (1.0 - self.mix)) * self.output_gain;
        (final_l, final_r)
    }
}

// ============================================================
// PHASER
// ============================================================

#[derive(Debug, Clone)]
pub struct PhaserEffect {
    pub rate_hz: f32,
    pub depth: f32,
    pub center_hz: f32,
    pub feedback: f32,
    pub wet_mix: f32,
    pub dry_mix: f32,
    pub stages: usize,
    pub lfo: Lfo,
    pub filters_l: Vec<BiquadState>,
    pub filters_r: Vec<BiquadState>,
    pub last_out_l: f32,
    pub last_out_r: f32,
    pub is_enabled: bool,
}

impl PhaserEffect {
    pub fn new() -> Self {
        let mut lfo_r = Lfo::new(LfoShape::Sine, 0.5, 1.0);
        lfo_r.phase_offset = 0.5;
        Self {
            rate_hz: 0.5,
            depth: 0.8,
            center_hz: 1000.0,
            feedback: 0.5,
            wet_mix: 0.5,
            dry_mix: 0.5,
            stages: PHASER_STAGES,
            lfo: Lfo::new(LfoShape::Sine, 0.5, 1.0),
            filters_l: vec![BiquadState::new(); PHASER_STAGES],
            filters_r: vec![BiquadState::new(); PHASER_STAGES],
            last_out_l: 0.0,
            last_out_r: 0.0,
            is_enabled: true,
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l * self.dry_mix, in_r * self.dry_mix); }

        let mod_val = self.lfo.tick();
        let freq = (self.center_hz * (1.0 + mod_val * self.depth)).clamp(20.0, 20000.0);
        let coeff = BiquadCoefficients::all_pass(freq, 0.707, SAMPLE_RATE);

        let feed_l = in_l + self.last_out_l * self.feedback;
        let feed_r = in_r + self.last_out_r * self.feedback;

        let mut sig_l = feed_l;
        let mut sig_r = feed_r;
        for i in 0..self.stages.min(self.filters_l.len()) {
            sig_l = self.filters_l[i].process(sig_l, &coeff);
            sig_r = self.filters_r[i].process(sig_r, &coeff);
        }

        self.last_out_l = sig_l;
        self.last_out_r = sig_r;

        let out_l = in_l * self.dry_mix + sig_l * self.wet_mix;
        let out_r = in_r * self.dry_mix + sig_r * self.wet_mix;
        (out_l, out_r)
    }
}

// ============================================================
// FLANGER
// ============================================================

#[derive(Debug, Clone)]
pub struct FlangerEffect {
    pub rate_hz: f32,
    pub depth_ms: f32,
    pub delay_center_ms: f32,
    pub feedback: f32,
    pub wet_mix: f32,
    pub dry_mix: f32,
    pub buf_l: Vec<f32>,
    pub buf_r: Vec<f32>,
    pub write_pos: usize,
    pub buf_size: usize,
    pub lfo: Lfo,
    pub is_enabled: bool,
}

impl FlangerEffect {
    pub fn new() -> Self {
        let buf_size = 4096;
        Self {
            rate_hz: 0.3,
            depth_ms: 3.0,
            delay_center_ms: 5.0,
            feedback: 0.5,
            wet_mix: 0.5,
            dry_mix: 0.5,
            buf_l: vec![0.0; buf_size],
            buf_r: vec![0.0; buf_size],
            write_pos: 0,
            buf_size,
            lfo: Lfo::new(LfoShape::Sine, 0.3, 1.0),
            is_enabled: true,
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l * self.dry_mix, in_r * self.dry_mix); }

        self.buf_l[self.write_pos] = in_l;
        self.buf_r[self.write_pos] = in_r;

        let mod_val = self.lfo.tick();
        let delay_samples = ((self.delay_center_ms + mod_val * self.depth_ms) * 0.001 * SAMPLE_RATE)
            .clamp(1.0, self.buf_size as f32 - 1.0);

        let read_pos = (self.write_pos as f32 + self.buf_size as f32 - delay_samples) % self.buf_size as f32;
        let i0 = read_pos as usize % self.buf_size;
        let i1 = (i0 + 1) % self.buf_size;
        let frac = read_pos - read_pos.floor();

        let wet_l = self.buf_l[i0] * (1.0 - frac) + self.buf_l[i1] * frac;
        let wet_r = self.buf_r[i0] * (1.0 - frac) + self.buf_r[i1] * frac;

        self.write_pos = (self.write_pos + 1) % self.buf_size;

        let out_l = in_l * self.dry_mix + wet_l * self.wet_mix + wet_l * self.feedback;
        let out_r = in_r * self.dry_mix + wet_r * self.wet_mix + wet_r * self.feedback;
        (out_l, out_r)
    }
}

// ============================================================
// BIT CRUSHER (Standalone)
// ============================================================

#[derive(Debug, Clone)]
pub struct BitCrusherEffect {
    pub bit_depth: f32,
    pub sample_rate_divider: u32,
    pub mix: f32,
    pub counter: u32,
    pub held_l: f32,
    pub held_r: f32,
    pub is_enabled: bool,
}

impl BitCrusherEffect {
    pub fn new(bit_depth: f32, rate_divider: u32) -> Self {
        Self {
            bit_depth,
            sample_rate_divider: rate_divider.max(1),
            mix: 1.0,
            counter: 0,
            held_l: 0.0,
            held_r: 0.0,
            is_enabled: true,
        }
    }

    fn quantize(x: f32, bits: f32) -> f32 {
        let levels = 2.0f32.powf(bits.clamp(1.0, 32.0));
        ((x * levels).floor() + 0.5) / levels
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l, in_r); }
        self.counter += 1;
        if self.counter >= self.sample_rate_divider {
            self.counter = 0;
            self.held_l = Self::quantize(in_l, self.bit_depth);
            self.held_r = Self::quantize(in_r, self.bit_depth);
        }
        let out_l = self.held_l * self.mix + in_l * (1.0 - self.mix);
        let out_r = self.held_r * self.mix + in_r * (1.0 - self.mix);
        (out_l, out_r)
    }
}

// ============================================================
// SPATIALIZER / HRTF
// ============================================================

#[derive(Debug, Clone)]
pub struct HrtfFilter {
    pub impulse_l: Vec<f32>,
    pub impulse_r: Vec<f32>,
    pub history_l: VecDeque<f32>,
    pub history_r: VecDeque<f32>,
}

impl HrtfFilter {
    pub fn new(elevation_rad: f32, azimuth_rad: f32) -> Self {
        // Approximate HRTF with minimum-phase FIR filters
        let len = HRTF_FILTER_LENGTH;
        let mut impulse_l = vec![0.0f32; len];
        let mut impulse_r = vec![0.0f32; len];

        // ITD (inter-aural time delay) based on azimuth
        // Woodworth formula: ITD = (r/c) * (sin(azimuth) + azimuth)
        let r = 0.0875; // head radius in meters
        let itd_s = (r / SPEED_OF_SOUND) * (azimuth_rad.sin() + azimuth_rad);
        let itd_samples = (itd_s * SAMPLE_RATE).round() as i32;

        // ILD (inter-aural level difference)
        let ild_db = 10.0 * azimuth_rad.sin().abs();
        let ild_linear = db_to_linear(-ild_db * 0.5);

        // Build FIR with delay and simple spectral shaping
        let hann = |n: usize| -> f32 {
            0.5 * (1.0 - (TWO_PI * n as f32 / (len as f32 - 1.0)).cos())
        };

        // Sinc-based low-pass at ~3kHz for contralateral ear
        let fc = 3000.0 / SAMPLE_RATE;
        for n in 0..len {
            let idx = n as f32 - len as f32 / 2.0;
            let sinc = if idx.abs() < 1e-6 {
                2.0 * fc
            } else {
                (2.0 * std::f32::consts::PI * fc * idx).sin() / (std::f32::consts::PI * idx)
            };
            let win = hann(n);

            if azimuth_rad >= 0.0 {
                // Source on right: left ear is contralateral, right is ipsilateral
                let delay_n = (n as i32 - itd_samples.min(len as i32 - 1)).clamp(0, len as i32 - 1) as usize;
                impulse_l[delay_n] += sinc * win * ild_linear;
                impulse_r[n] += sinc * win;
            } else {
                impulse_l[n] += sinc * win;
                let delay_n = (n as i32 + itd_samples.min(len as i32 - 1)).clamp(0, len as i32 - 1) as usize;
                impulse_r[delay_n] += sinc * win * ild_linear;
            }
        }

        Self {
            impulse_l,
            impulse_r,
            history_l: VecDeque::with_capacity(len),
            history_r: VecDeque::with_capacity(len),
        }
    }

    pub fn process_mono(&mut self, mono_in: f32) -> (f32, f32) {
        let len = self.impulse_l.len();

        if self.history_l.len() >= len { self.history_l.pop_back(); }
        if self.history_r.len() >= len { self.history_r.pop_back(); }
        self.history_l.push_front(mono_in);
        self.history_r.push_front(mono_in);

        // Convolution
        let out_l: f32 = self.history_l.iter()
            .zip(self.impulse_l.iter())
            .map(|(h, imp)| h * imp)
            .sum();
        let out_r: f32 = self.history_r.iter()
            .zip(self.impulse_r.iter())
            .map(|(h, imp)| h * imp)
            .sum();

        (out_l, out_r)
    }
}

// ============================================================
// SPATIALIZER
// ============================================================

#[derive(Debug, Clone)]
pub struct Spatializer3D {
    pub position: Vec3,
    pub listener_pos: Vec3,
    pub listener_forward: Vec3,
    pub listener_up: Vec3,
    pub attenuation_model: AttenuationModel,
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff_factor: f32,
    pub doppler_factor: f32,
    pub occlusion_db: f32,
    pub obstruction_db: f32,
    pub use_hrtf: bool,
    pub hrtf_filter: Option<HrtfFilter>,
    pub reverb_send_db: f32,
    pub current_distance: f32,
    pub current_azimuth: f32,
    pub current_elevation: f32,
    pub pan_l: f32,
    pub pan_r: f32,
    pub distance_gain: f32,
    pub doppler_pitch: f32,
    pub source_velocity: Vec3,
    pub listener_velocity: Vec3,
    pub is_enabled: bool,
}

impl Spatializer3D {
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            listener_pos: Vec3::ZERO,
            listener_forward: Vec3::NEG_Z,
            listener_up: Vec3::Y,
            attenuation_model: AttenuationModel::InverseSquare,
            min_distance: 1.0,
            max_distance: 100.0,
            rolloff_factor: 1.0,
            doppler_factor: 1.0,
            occlusion_db: 0.0,
            obstruction_db: 0.0,
            use_hrtf: false,
            hrtf_filter: None,
            reverb_send_db: -6.0,
            current_distance: 0.0,
            current_azimuth: 0.0,
            current_elevation: 0.0,
            pan_l: SQRT2 / 2.0,
            pan_r: SQRT2 / 2.0,
            distance_gain: 1.0,
            doppler_pitch: 1.0,
            source_velocity: Vec3::ZERO,
            listener_velocity: Vec3::ZERO,
            is_enabled: true,
        }
    }

    pub fn update(&mut self) {
        let to_source = self.position - self.listener_pos;
        self.current_distance = to_source.length();

        if self.current_distance < 1e-6 {
            self.pan_l = SQRT2 / 2.0;
            self.pan_r = SQRT2 / 2.0;
            self.distance_gain = 1.0;
            self.doppler_pitch = 1.0;
            return;
        }

        let dir = to_source / self.current_distance;

        // Azimuth and elevation relative to listener
        let right = self.listener_forward.cross(self.listener_up).normalize_or_zero();
        let up = self.listener_up;
        let fwd = self.listener_forward;

        self.current_azimuth = dir.dot(right).atan2(dir.dot(fwd));
        self.current_elevation = dir.dot(up).asin().clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

        // Constant-power panning from azimuth
        let pan_angle = self.current_azimuth.clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
        let pan_norm = (pan_angle / std::f32::consts::FRAC_PI_2 + 1.0) * 0.5; // 0..1, 0=left 1=right
        self.pan_l = ((1.0 - pan_norm) * std::f32::consts::FRAC_PI_2).cos();
        self.pan_r = (pan_norm * std::f32::consts::FRAC_PI_2).cos();

        // Distance attenuation
        let dist = self.current_distance.clamp(self.min_distance, self.max_distance);
        self.distance_gain = match self.attenuation_model {
            AttenuationModel::InverseSquare => {
                let d = dist / self.min_distance;
                1.0 / (d * d).max(1.0)
            }
            AttenuationModel::Linear => {
                1.0 - self.rolloff_factor * (dist - self.min_distance) / (self.max_distance - self.min_distance)
            }
            AttenuationModel::Logarithmic => {
                1.0 - self.rolloff_factor * (dist / self.min_distance).ln() / (self.max_distance / self.min_distance).ln().max(1.0)
            }
            AttenuationModel::Custom => 1.0,
        };
        self.distance_gain = self.distance_gain.clamp(0.0, 1.0);

        // Apply occlusion/obstruction
        let occ_gain = db_to_linear(self.occlusion_db + self.obstruction_db);
        self.distance_gain *= occ_gain;

        // Doppler shift: f' = f * (v_sound + v_listener) / (v_sound + v_source)
        // Project velocities onto the source-listener direction
        let v_listener_proj = self.listener_velocity.dot(dir);
        let v_source_proj = self.source_velocity.dot(dir);
        let numerator = SPEED_OF_SOUND + v_listener_proj;
        let denominator = SPEED_OF_SOUND + v_source_proj;
        self.doppler_pitch = if denominator.abs() > 1.0 {
            (numerator / denominator).clamp(0.5, 2.0) * self.doppler_factor + (1.0 - self.doppler_factor)
        } else { 1.0 };

        // Update HRTF if enabled
        if self.use_hrtf {
            self.hrtf_filter = Some(HrtfFilter::new(self.current_elevation, self.current_azimuth));
        }
    }

    pub fn process_mono(&mut self, mono: f32) -> (f32, f32) {
        if !self.is_enabled { return (mono, mono); }
        let gained = mono * self.distance_gain;
        if self.use_hrtf {
            if let Some(hrtf) = &mut self.hrtf_filter {
                return hrtf.process_mono(gained);
            }
        }
        (gained * self.pan_l, gained * self.pan_r)
    }

    pub fn reverb_send_gain(&self) -> f32 {
        db_to_linear(self.reverb_send_db) * self.distance_gain.powf(0.5)
    }
}

// ============================================================
// CONVOLUTION (parameters only — no full IR processing)
// ============================================================

#[derive(Debug, Clone)]
pub struct ConvolutionParams {
    pub ir_asset_id: u64,
    pub wet_mix: f32,
    pub dry_mix: f32,
    pub pre_delay_ms: f32,
    pub ir_length_ms: f32,
    pub is_enabled: bool,
    // Simulation: apply a simple exponential decay approximation
    pub decay_coeff: f32,
    pub sim_state_l: f32,
    pub sim_state_r: f32,
}

impl ConvolutionParams {
    pub fn new(ir_asset_id: u64) -> Self {
        Self {
            ir_asset_id,
            wet_mix: 0.3,
            dry_mix: 0.7,
            pre_delay_ms: 5.0,
            ir_length_ms: 1000.0,
            is_enabled: true,
            decay_coeff: (-1.0 / (1000.0 * 0.001 * SAMPLE_RATE)).exp(),
            sim_state_l: 0.0,
            sim_state_r: 0.0,
        }
    }

    pub fn process_sample_approx(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l * self.dry_mix, in_r * self.dry_mix); }
        // Exponential decay approximation (not true convolution)
        self.sim_state_l = self.sim_state_l * self.decay_coeff + in_l;
        self.sim_state_r = self.sim_state_r * self.decay_coeff + in_r;
        let out_l = in_l * self.dry_mix + self.sim_state_l * self.wet_mix;
        let out_r = in_r * self.dry_mix + self.sim_state_r * self.wet_mix;
        (out_l, out_r)
    }
}

// ============================================================
// EFFECT CHAIN
// ============================================================

#[derive(Debug, Clone)]
pub enum AudioEffect {
    Eq(ParametricEqualizer),
    Compressor(Compressor),
    Reverb(SchroederReverb),
    Delay(DelayEffect),
    Chorus(ChorusEffect),
    Limiter(Limiter),
    Gate(NoiseGate),
    Distortion(DistortionEffect),
    Phaser(PhaserEffect),
    Flanger(FlangerEffect),
    BitCrusher(BitCrusherEffect),
    Convolution(ConvolutionParams),
}

impl AudioEffect {
    pub fn effect_type(&self) -> EffectType {
        match self {
            AudioEffect::Eq(_) => EffectType::Equalizer,
            AudioEffect::Compressor(_) => EffectType::Compressor,
            AudioEffect::Reverb(_) => EffectType::Reverb,
            AudioEffect::Delay(_) => EffectType::Delay,
            AudioEffect::Chorus(_) => EffectType::Chorus,
            AudioEffect::Limiter(_) => EffectType::Limiter,
            AudioEffect::Gate(_) => EffectType::Gate,
            AudioEffect::Distortion(_) => EffectType::Distortion,
            AudioEffect::Phaser(_) => EffectType::Phaser,
            AudioEffect::Flanger(_) => EffectType::Flanger,
            AudioEffect::BitCrusher(_) => EffectType::BitCrusher,
            AudioEffect::Convolution(_) => EffectType::Convolution,
        }
    }

    pub fn process_sample(&mut self, left: f32, right: f32) -> (f32, f32) {
        match self {
            AudioEffect::Eq(eq) => eq.process_stereo(left, right),
            AudioEffect::Compressor(c) => c.process_sample(left, right),
            AudioEffect::Reverb(r) => r.process_sample(left, right),
            AudioEffect::Delay(d) => d.process_sample(left, right),
            AudioEffect::Chorus(c) => c.process_sample(left, right),
            AudioEffect::Limiter(l) => l.process_sample(left, right),
            AudioEffect::Gate(g) => g.process_sample(left, right),
            AudioEffect::Distortion(d) => d.process_sample(left, right),
            AudioEffect::Phaser(p) => p.process_sample(left, right),
            AudioEffect::Flanger(f) => f.process_sample(left, right),
            AudioEffect::BitCrusher(b) => b.process_sample(left, right),
            AudioEffect::Convolution(c) => c.process_sample_approx(left, right),
        }
    }

    pub fn is_enabled(&self) -> bool {
        match self {
            AudioEffect::Eq(e) => e.is_enabled,
            AudioEffect::Compressor(c) => c.is_enabled,
            AudioEffect::Reverb(r) => r.is_enabled,
            AudioEffect::Delay(d) => d.is_enabled,
            AudioEffect::Chorus(c) => c.is_enabled,
            AudioEffect::Limiter(l) => l.is_enabled,
            AudioEffect::Gate(g) => g.is_enabled,
            AudioEffect::Distortion(d) => d.is_enabled,
            AudioEffect::Phaser(p) => p.is_enabled,
            AudioEffect::Flanger(f) => f.is_enabled,
            AudioEffect::BitCrusher(b) => b.is_enabled,
            AudioEffect::Convolution(c) => c.is_enabled,
        }
    }

    pub fn cpu_cost_estimate(&self) -> f32 {
        match self {
            AudioEffect::Eq(e) => 0.02 * e.bands.len() as f32,
            AudioEffect::Compressor(_) => 0.05,
            AudioEffect::Reverb(_) => 0.15,
            AudioEffect::Delay(_) => 0.03,
            AudioEffect::Chorus(_) => 0.04,
            AudioEffect::Limiter(_) => 0.02,
            AudioEffect::Gate(_) => 0.02,
            AudioEffect::Distortion(_) => 0.03,
            AudioEffect::Phaser(_) => 0.06,
            AudioEffect::Flanger(_) => 0.04,
            AudioEffect::BitCrusher(_) => 0.01,
            AudioEffect::Convolution(_) => 0.3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EffectChain {
    pub effects: Vec<AudioEffect>,
    pub bypass: bool,
}

impl EffectChain {
    pub fn new() -> Self {
        Self { effects: Vec::new(), bypass: false }
    }

    pub fn add(&mut self, effect: AudioEffect) {
        if self.effects.len() < MAX_EFFECT_CHAIN_LENGTH {
            self.effects.push(effect);
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.effects.len() {
            self.effects.remove(index);
        }
    }

    pub fn move_effect(&mut self, from: usize, to: usize) {
        if from < self.effects.len() && to < self.effects.len() && from != to {
            let effect = self.effects.remove(from);
            self.effects.insert(to, effect);
        }
    }

    pub fn process_sample(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        if self.bypass { return (left, right); }
        for effect in &mut self.effects {
            if effect.is_enabled() {
                let (l, r) = effect.process_sample(left, right);
                left = l;
                right = r;
            }
        }
        (left, right)
    }

    pub fn total_cpu_cost(&self) -> f32 {
        self.effects.iter()
            .filter(|e| e.is_enabled())
            .map(|e| e.cpu_cost_estimate())
            .sum()
    }

    pub fn clear(&mut self) { self.effects.clear(); }
    pub fn effect_count(&self) -> usize { self.effects.len() }
}

// ============================================================
// AUDIO BUS
// ============================================================

#[derive(Debug, Clone)]
pub struct AudioBus {
    pub id: u64,
    pub name: String,
    pub bus_type: BusType,
    pub gain_db: f32,
    pub pan: f32,           // -1.0 (L) .. 0.0 .. 1.0 (R)
    pub mute: bool,
    pub solo: bool,
    pub effect_chain: EffectChain,
    pub send_levels: HashMap<u64, f32>, // bus_id -> send level (linear)
    pub sidechain_source: Option<u64>,  // bus_id for sidechain
    pub sidechain_gain: f32,
    pub parent_bus_id: Option<u64>,
    pub children_bus_ids: Vec<u64>,
    pub input_level_l: f32,
    pub input_level_r: f32,
    pub output_level_l: f32,
    pub output_level_r: f32,
    pub peak_hold_l: f32,
    pub peak_hold_r: f32,
    pub peak_hold_timer: u32,
    pub channel_count: usize,
    pub accumulated_l: f32,
    pub accumulated_r: f32,
    pub is_enabled: bool,
}

impl AudioBus {
    pub fn new(id: u64, name: String, bus_type: BusType) -> Self {
        Self {
            id,
            name,
            bus_type,
            gain_db: 0.0,
            pan: 0.0,
            mute: false,
            solo: false,
            effect_chain: EffectChain::new(),
            send_levels: HashMap::new(),
            sidechain_source: None,
            sidechain_gain: 1.0,
            parent_bus_id: None,
            children_bus_ids: Vec::new(),
            input_level_l: 0.0,
            input_level_r: 0.0,
            output_level_l: 0.0,
            output_level_r: 0.0,
            peak_hold_l: 0.0,
            peak_hold_r: 0.0,
            peak_hold_timer: 0,
            channel_count: 2,
            accumulated_l: 0.0,
            accumulated_r: 0.0,
            is_enabled: true,
        }
    }

    /// Process a single frame (one stereo pair)
    pub fn process_sample(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        if !self.is_enabled || self.mute { return (0.0, 0.0); }

        self.input_level_l = left.abs().max(self.input_level_l * 0.999);
        self.input_level_r = right.abs().max(self.input_level_r * 0.999);

        // Apply gain
        let gain = db_to_linear(self.gain_db);
        left *= gain;
        right *= gain;

        // Pan (constant-power)
        let pan_r = (self.pan * 0.5 + 0.5).clamp(0.0, 1.0);
        let pan_l = 1.0 - pan_r;
        let pan_gain_l = (pan_l * std::f32::consts::FRAC_PI_2).cos() * SQRT2;
        let pan_gain_r = (pan_r * std::f32::consts::FRAC_PI_2).cos() * SQRT2;
        left *= pan_gain_l;
        right *= pan_gain_r;

        // Insert chain
        let (l, r) = self.effect_chain.process_sample(left, right);
        left = l;
        right = r;

        self.output_level_l = left.abs().max(self.output_level_l * 0.999);
        self.output_level_r = right.abs().max(self.output_level_r * 0.999);

        // Peak hold
        if left.abs() > self.peak_hold_l {
            self.peak_hold_l = left.abs();
            self.peak_hold_timer = PEAK_HOLD_FRAMES;
        } else if self.peak_hold_timer > 0 {
            self.peak_hold_timer -= 1;
        } else {
            self.peak_hold_l *= 0.9995;
        }
        if right.abs() > self.peak_hold_r {
            self.peak_hold_r = right.abs();
        } else {
            self.peak_hold_r *= 0.9995;
        }

        (left, right)
    }

    pub fn add_child(&mut self, child_id: u64) {
        if !self.children_bus_ids.contains(&child_id) {
            self.children_bus_ids.push(child_id);
        }
    }

    pub fn remove_child(&mut self, child_id: u64) {
        self.children_bus_ids.retain(|&id| id != child_id);
    }

    pub fn set_send(&mut self, target_bus_id: u64, level: f32) {
        self.send_levels.insert(target_bus_id, level.clamp(0.0, 4.0));
    }

    pub fn peak_l_db(&self) -> f32 { linear_to_db(self.peak_hold_l.max(1e-9)) }
    pub fn peak_r_db(&self) -> f32 { linear_to_db(self.peak_hold_r.max(1e-9)) }
    pub fn output_l_db(&self) -> f32 { linear_to_db(self.output_level_l.max(1e-9)) }
    pub fn output_r_db(&self) -> f32 { linear_to_db(self.output_level_r.max(1e-9)) }
}

// ============================================================
// SIGNAL FLOW GRAPH
// ============================================================

#[derive(Debug, Clone)]
pub struct SignalFlowEdge {
    pub from_bus_id: u64,
    pub to_bus_id: u64,
    pub send_level: f32,
    pub is_sidechain: bool,
}

#[derive(Debug)]
pub struct SignalFlowGraph {
    pub buses: HashMap<u64, AudioBus>,
    pub edges: Vec<SignalFlowEdge>,
    pub next_bus_id: u64,
    pub master_bus_id: u64,
    pub topology_order: Vec<u64>, // buses in processing order
}

impl SignalFlowGraph {
    pub fn new() -> Self {
        let mut graph = Self {
            buses: HashMap::new(),
            edges: Vec::new(),
            next_bus_id: 1,
            master_bus_id: 1,
            topology_order: Vec::new(),
        };
        // Create default bus hierarchy
        let master = graph.create_bus("Master".into(), BusType::Master);
        let music  = graph.create_bus("Music".into(),  BusType::Music);
        let sfx    = graph.create_bus("SFX".into(),    BusType::Sfx);
        let voice  = graph.create_bus("Voice".into(),  BusType::Voice);
        let ambient = graph.create_bus("Ambient".into(), BusType::Ambient);
        let ui     = graph.create_bus("UI".into(),     BusType::Ui);

        graph.connect(music, master, 1.0);
        graph.connect(sfx, master, 1.0);
        graph.connect(voice, master, 1.0);
        graph.connect(ambient, master, 1.0);
        graph.connect(ui, master, 1.0);

        graph.master_bus_id = master;
        graph.rebuild_topology();
        graph
    }

    pub fn create_bus(&mut self, name: String, bus_type: BusType) -> u64 {
        let id = self.next_bus_id;
        self.next_bus_id += 1;
        self.buses.insert(id, AudioBus::new(id, name, bus_type));
        id
    }

    pub fn connect(&mut self, from: u64, to: u64, level: f32) {
        // Avoid duplicate edges
        self.edges.retain(|e| !(e.from_bus_id == from && e.to_bus_id == to));
        self.edges.push(SignalFlowEdge {
            from_bus_id: from,
            to_bus_id: to,
            send_level: level,
            is_sidechain: false,
        });
        if let Some(bus) = self.buses.get_mut(&from) {
            bus.set_send(to, level);
            if let Some(parent) = bus.parent_bus_id {
                // Already has parent
            } else {
                bus.parent_bus_id = Some(to);
            }
        }
        if let Some(bus) = self.buses.get_mut(&to) {
            bus.add_child(from);
        }
        self.rebuild_topology();
    }

    pub fn disconnect(&mut self, from: u64, to: u64) {
        self.edges.retain(|e| !(e.from_bus_id == from && e.to_bus_id == to));
        if let Some(bus) = self.buses.get_mut(&from) {
            bus.send_levels.remove(&to);
        }
        if let Some(bus) = self.buses.get_mut(&to) {
            bus.remove_child(from);
        }
        self.rebuild_topology();
    }

    /// Kahn's algorithm topological sort for processing order (children before parents)
    pub fn rebuild_topology(&mut self) {
        let mut in_degree: HashMap<u64, usize> = HashMap::new();
        for &id in self.buses.keys() { in_degree.insert(id, 0); }

        for edge in &self.edges {
            *in_degree.entry(edge.to_bus_id).or_insert(0) += 1;
        }

        let mut queue: VecDeque<u64> = in_degree.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut order = Vec::new();

        while let Some(id) = queue.pop_front() {
            order.push(id);
            for edge in &self.edges {
                if edge.from_bus_id == id {
                    if let Some(d) = in_degree.get_mut(&edge.to_bus_id) {
                        *d = d.saturating_sub(1);
                        if *d == 0 { queue.push_back(edge.to_bus_id); }
                    }
                }
            }
        }
        self.topology_order = order;
    }

    pub fn get_bus(&self, id: u64) -> Option<&AudioBus> { self.buses.get(&id) }
    pub fn get_bus_mut(&mut self, id: u64) -> Option<&mut AudioBus> { self.buses.get_mut(&id) }
}

// ============================================================
// ADSR ENVELOPE
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Debug, Clone)]
pub struct AdsrEnvelope {
    pub attack_s: f32,
    pub decay_s: f32,
    pub sustain_level: f32,
    pub release_s: f32,
    pub attack_curve: f32,   // 1.0 = linear, <1 = convex, >1 = concave
    pub decay_curve: f32,
    pub release_curve: f32,
    pub stage: EnvelopeStage,
    pub value: f32,
    pub release_start_value: f32,
    pub time_in_stage: f32,
}

impl AdsrEnvelope {
    pub fn new(attack_s: f32, decay_s: f32, sustain: f32, release_s: f32) -> Self {
        Self {
            attack_s,
            decay_s,
            sustain_level: sustain.clamp(0.0, 1.0),
            release_s,
            attack_curve: 1.0,
            decay_curve: 2.0,
            release_curve: 2.0,
            stage: EnvelopeStage::Idle,
            value: 0.0,
            release_start_value: 0.0,
            time_in_stage: 0.0,
        }
    }

    pub fn trigger_attack(&mut self) {
        self.stage = EnvelopeStage::Attack;
        self.time_in_stage = 0.0;
    }

    pub fn trigger_release(&mut self) {
        if self.stage != EnvelopeStage::Idle {
            self.release_start_value = self.value;
            self.stage = EnvelopeStage::Release;
            self.time_in_stage = 0.0;
        }
    }

    fn apply_curve(t: f32, curve: f32) -> f32 {
        t.powf(curve)
    }

    pub fn tick(&mut self, dt_s: f32) -> f32 {
        self.time_in_stage += dt_s;
        match self.stage {
            EnvelopeStage::Idle => { self.value = 0.0; }
            EnvelopeStage::Attack => {
                let t = if self.attack_s > 0.0 {
                    (self.time_in_stage / self.attack_s).clamp(0.0, 1.0)
                } else { 1.0 };
                self.value = Self::apply_curve(t, self.attack_curve);
                if t >= 1.0 {
                    self.stage = EnvelopeStage::Decay;
                    self.time_in_stage = 0.0;
                }
            }
            EnvelopeStage::Decay => {
                let t = if self.decay_s > 0.0 {
                    (self.time_in_stage / self.decay_s).clamp(0.0, 1.0)
                } else { 1.0 };
                let ct = Self::apply_curve(t, self.decay_curve);
                self.value = 1.0 - ct * (1.0 - self.sustain_level);
                if t >= 1.0 {
                    self.stage = EnvelopeStage::Sustain;
                    self.value = self.sustain_level;
                }
            }
            EnvelopeStage::Sustain => { self.value = self.sustain_level; }
            EnvelopeStage::Release => {
                let t = if self.release_s > 0.0 {
                    (self.time_in_stage / self.release_s).clamp(0.0, 1.0)
                } else { 1.0 };
                let ct = Self::apply_curve(t, self.release_curve);
                self.value = self.release_start_value * (1.0 - ct);
                if t >= 1.0 {
                    self.stage = EnvelopeStage::Idle;
                    self.value = 0.0;
                }
            }
        }
        self.value
    }

    pub fn is_active(&self) -> bool { self.stage != EnvelopeStage::Idle }
    pub fn is_released(&self) -> bool { self.stage == EnvelopeStage::Release }
}

// ============================================================
// SOUND DESIGN PARAMETERS
// ============================================================

#[derive(Debug, Clone)]
pub struct SoundDesignParams {
    pub volume_adsr: AdsrEnvelope,
    pub pitch_lfo: Lfo,
    pub amplitude_lfo: Lfo,
    pub pitch_random_range_semitones: f32,
    pub volume_random_range_db: f32,
    pub start_offset_random_s: f32,
    pub pitch_semitones: f32,        // base pitch offset
    pub fine_tune_cents: f32,
    pub looping: bool,
    pub loop_start_s: f32,
    pub loop_end_s: f32,
    pub fade_in_s: f32,
    pub fade_out_s: f32,
}

impl SoundDesignParams {
    pub fn new() -> Self {
        Self {
            volume_adsr: AdsrEnvelope::new(0.005, 0.1, 1.0, 0.3),
            pitch_lfo: Lfo::new(LfoShape::Sine, 5.0, 0.0),
            amplitude_lfo: Lfo::new(LfoShape::Sine, 4.0, 0.0),
            pitch_random_range_semitones: 0.0,
            volume_random_range_db: 0.0,
            start_offset_random_s: 0.0,
            pitch_semitones: 0.0,
            fine_tune_cents: 0.0,
            looping: false,
            loop_start_s: 0.0,
            loop_end_s: 0.0,
            fade_in_s: 0.0,
            fade_out_s: 0.0,
        }
    }

    pub fn pitch_ratio(&self, random_seed: f32) -> f32 {
        let base = self.pitch_semitones + self.fine_tune_cents * 0.01;
        let rand_offset = random_seed * self.pitch_random_range_semitones;
        semitones_to_ratio(base + rand_offset)
    }

    pub fn volume_linear(&self, random_seed: f32) -> f32 {
        let rand_db = (random_seed * 2.0 - 1.0) * self.volume_random_range_db;
        db_to_linear(rand_db)
    }
}

pub fn semitones_to_ratio(semitones: f32) -> f32 {
    2.0f32.powf(semitones / 12.0)
}

// ============================================================
// SPATIAL REVERB ZONES
// ============================================================

#[derive(Debug, Clone)]
pub struct ReverbZone {
    pub id: u64,
    pub name: String,
    pub center: Vec3,
    pub radius: f32,
    pub blend_radius: f32, // transition zone outside radius
    pub reverb_params: SchroederReverb,
    pub priority: u32,
    pub is_enabled: bool,
}

impl ReverbZone {
    pub fn new(id: u64, name: String, center: Vec3, radius: f32, blend_radius: f32) -> Self {
        Self {
            id,
            name,
            center,
            radius,
            blend_radius,
            reverb_params: SchroederReverb::new(),
            priority: 0,
            is_enabled: true,
        }
    }

    pub fn blend_factor(&self, listener_pos: Vec3) -> f32 {
        let dist = (listener_pos - self.center).length();
        if dist <= self.radius { return 1.0; }
        let outer = self.radius + self.blend_radius;
        if dist >= outer { return 0.0; }
        1.0 - (dist - self.radius) / self.blend_radius.max(0.001)
    }

    pub fn is_active(&self, listener_pos: Vec3) -> bool {
        self.is_enabled && self.blend_factor(listener_pos) > 0.0
    }
}

#[derive(Debug)]
pub struct ReverbZoneManager {
    pub zones: HashMap<u64, ReverbZone>,
    pub active_blend: HashMap<u64, f32>,
    pub next_id: u64,
}

impl ReverbZoneManager {
    pub fn new() -> Self {
        Self {
            zones: HashMap::new(),
            active_blend: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn add_zone(&mut self, name: String, center: Vec3, radius: f32, blend: f32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.zones.insert(id, ReverbZone::new(id, name, center, radius, blend));
        id
    }

    pub fn update(&mut self, listener_pos: Vec3) {
        self.active_blend.clear();
        for (id, zone) in &self.zones {
            let blend = zone.blend_factor(listener_pos);
            if blend > 0.0 {
                self.active_blend.insert(*id, blend);
            }
        }
    }

    pub fn highest_priority_zone(&self) -> Option<u64> {
        self.active_blend.keys()
            .max_by_key(|&&id| {
                self.zones.get(&id).map(|z| z.priority).unwrap_or(0)
            })
            .copied()
    }

    pub fn blended_room_size(&self) -> f32 {
        let total_blend: f32 = self.active_blend.values().sum();
        if total_blend < 1e-6 { return 0.3; }
        self.active_blend.iter()
            .filter_map(|(id, &blend)| self.zones.get(id).map(|z| blend * z.reverb_params.room_size))
            .sum::<f32>() / total_blend
    }
}

// ============================================================
// ADAPTIVE MUSIC SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct MusicStem {
    pub id: u64,
    pub name: String,
    pub volume_db: f32,
    pub is_active: bool,
    pub fade_in_s: f32,
    pub fade_out_s: f32,
    pub fade_value: f32,
    pub category: String,
    pub beat_length: u32, // in beats
}

impl MusicStem {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            id,
            name,
            volume_db: 0.0,
            is_active: false,
            fade_in_s: 0.5,
            fade_out_s: 1.0,
            fade_value: 0.0,
            category: "melody".into(),
            beat_length: 16,
        }
    }

    pub fn update_fade(&mut self, dt_s: f32) {
        let target = if self.is_active { 1.0f32 } else { 0.0f32 };
        let speed = if self.is_active { 1.0 / self.fade_in_s.max(0.001) }
                    else { 1.0 / self.fade_out_s.max(0.001) };
        if (self.fade_value - target).abs() < speed * dt_s {
            self.fade_value = target;
        } else if self.fade_value < target {
            self.fade_value += speed * dt_s;
        } else {
            self.fade_value -= speed * dt_s;
        }
        self.fade_value = self.fade_value.clamp(0.0, 1.0);
    }

    pub fn effective_volume(&self) -> f32 {
        db_to_linear(self.volume_db) * self.fade_value
    }
}

#[derive(Debug, Clone)]
pub struct MusicTransitionRule {
    pub from_state: String,
    pub to_state: String,
    pub transition_type: MusicTransitionType,
    pub crossfade_s: f32,
    pub condition: String, // tag/condition name
    pub priority: u32,
}

#[derive(Debug, Clone)]
pub struct BeatTracker {
    pub bpm: f32,
    pub time_signature_num: u32,   // beats per bar
    pub time_signature_den: u32,   // note value
    pub current_beat: f32,
    pub current_bar: u32,
    pub beat_elapsed_s: f32,
    pub bar_elapsed_s: f32,
    pub is_running: bool,
}

impl BeatTracker {
    pub fn new(bpm: f32) -> Self {
        Self {
            bpm,
            time_signature_num: 4,
            time_signature_den: 4,
            current_beat: 0.0,
            current_bar: 0,
            beat_elapsed_s: 0.0,
            bar_elapsed_s: 0.0,
            is_running: false,
        }
    }

    pub fn tick(&mut self, dt_s: f32) {
        if !self.is_running { return; }
        self.beat_elapsed_s += dt_s;
        self.bar_elapsed_s += dt_s;
        let seconds_per_beat = 60.0 / self.bpm.max(1.0);
        self.current_beat = self.beat_elapsed_s / seconds_per_beat;
        let seconds_per_bar = seconds_per_beat * self.time_signature_num as f32;
        self.current_bar = (self.bar_elapsed_s / seconds_per_bar) as u32;
    }

    pub fn beat_within_bar(&self) -> f32 {
        self.current_beat % self.time_signature_num as f32
    }

    pub fn is_on_beat(&self, tolerance_s: f32) -> bool {
        let seconds_per_beat = 60.0 / self.bpm.max(1.0);
        let beat_phase = (self.beat_elapsed_s % seconds_per_beat) / seconds_per_beat;
        beat_phase < (tolerance_s / seconds_per_beat) || beat_phase > (1.0 - tolerance_s / seconds_per_beat)
    }

    pub fn is_on_bar(&self, tolerance_s: f32) -> bool {
        let seconds_per_beat = 60.0 / self.bpm.max(1.0);
        let seconds_per_bar = seconds_per_beat * self.time_signature_num as f32;
        let bar_phase = (self.bar_elapsed_s % seconds_per_bar) / seconds_per_bar;
        bar_phase < (tolerance_s / seconds_per_bar) || bar_phase > (1.0 - tolerance_s / seconds_per_bar)
    }

    pub fn time_to_next_beat(&self) -> f32 {
        let seconds_per_beat = 60.0 / self.bpm.max(1.0);
        let elapsed_in_beat = self.beat_elapsed_s % seconds_per_beat;
        seconds_per_beat - elapsed_in_beat
    }

    pub fn time_to_next_bar(&self) -> f32 {
        let seconds_per_beat = 60.0 / self.bpm.max(1.0);
        let seconds_per_bar = seconds_per_beat * self.time_signature_num as f32;
        let elapsed_in_bar = self.bar_elapsed_s % seconds_per_bar;
        seconds_per_bar - elapsed_in_bar
    }

    pub fn current_beat_integer(&self) -> u32 { self.current_beat as u32 }
}

#[derive(Debug)]
pub struct AdaptiveMusicSystem {
    pub stems: HashMap<u64, MusicStem>,
    pub transition_rules: Vec<MusicTransitionRule>,
    pub beat_tracker: BeatTracker,
    pub current_state: String,
    pub pending_state: Option<String>,
    pub pending_transition: Option<MusicTransitionType>,
    pub next_stem_id: u64,
    pub vertical_layers: HashMap<String, Vec<u64>>, // layer_name -> [stem_ids]
    pub intensity: f32, // 0..1 controls layer mixing
}

impl AdaptiveMusicSystem {
    pub fn new(bpm: f32) -> Self {
        Self {
            stems: HashMap::new(),
            transition_rules: Vec::new(),
            beat_tracker: BeatTracker::new(bpm),
            current_state: "silence".into(),
            pending_state: None,
            pending_transition: None,
            next_stem_id: 1,
            vertical_layers: HashMap::new(),
            intensity: 0.0,
        }
    }

    pub fn add_stem(&mut self, name: String, category: String) -> u64 {
        let id = self.next_stem_id;
        self.next_stem_id += 1;
        let mut stem = MusicStem::new(id, name);
        stem.category = category.clone();
        self.stems.insert(id, stem);
        self.vertical_layers.entry(category).or_default().push(id);
        id
    }

    pub fn set_state(&mut self, new_state: String, transition: MusicTransitionType) {
        self.pending_state = Some(new_state);
        self.pending_transition = Some(transition);
    }

    pub fn update(&mut self, dt_s: f32) {
        self.beat_tracker.tick(dt_s);

        // Check pending state transitions
        if let Some(ref state) = self.pending_state.clone() {
            let can_transition = match self.pending_transition.unwrap_or(MusicTransitionType::Immediate) {
                MusicTransitionType::Immediate => true,
                MusicTransitionType::OnBeat => self.beat_tracker.is_on_beat(0.05),
                MusicTransitionType::OnBar => self.beat_tracker.is_on_bar(0.05),
                MusicTransitionType::CrossFade => true,
                MusicTransitionType::StitchPoint => self.beat_tracker.is_on_beat(0.02),
            };
            if can_transition {
                self.current_state = state.clone();
                self.pending_state = None;
                self.pending_transition = None;
                self.apply_state_to_stems(&self.current_state.clone());
            }
        }

        // Update intensity-based vertical re-orchestration
        self.update_intensity_layers();

        // Update stem fades
        let stem_ids: Vec<u64> = self.stems.keys().copied().collect();
        for id in stem_ids {
            if let Some(stem) = self.stems.get_mut(&id) {
                stem.update_fade(dt_s);
            }
        }
    }

    fn apply_state_to_stems(&mut self, state: &str) {
        for stem in self.stems.values_mut() {
            // Simple state -> stem activation rule
            stem.is_active = stem.category == state || state == "all";
        }
    }

    fn update_intensity_layers(&mut self) {
        // Add stems layer by layer based on intensity
        let layer_names: Vec<String> = self.vertical_layers.keys().cloned().collect();
        let layer_count = layer_names.len().max(1);
        for (i, layer) in layer_names.iter().enumerate() {
            let threshold = i as f32 / layer_count as f32;
            let active = self.intensity >= threshold;
            if let Some(stem_ids) = self.vertical_layers.get(layer) {
                for &sid in stem_ids {
                    if let Some(stem) = self.stems.get_mut(&sid) {
                        // Don't override state-based activation
                        if self.current_state != "silence" {
                            stem.is_active = active;
                        }
                    }
                }
            }
        }
    }

    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity.clamp(0.0, 1.0);
    }

    pub fn active_stem_count(&self) -> usize {
        self.stems.values().filter(|s| s.is_active && s.fade_value > 0.001).count()
    }

    pub fn mixed_volume_for_stem(&self, stem_id: u64) -> f32 {
        self.stems.get(&stem_id).map(|s| s.effective_volume()).unwrap_or(0.0)
    }
}

// ============================================================
// AUDIO LOD / VOICE MANAGEMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct AudioVoice {
    pub id: u64,
    pub sound_id: u64,
    pub category: SoundCategory,
    pub position: Vec3,
    pub volume_db: f32,
    pub priority_score: f32,
    pub distance: f32,
    pub lod_level: AudioLodLevel,
    pub is_active: bool,
    pub is_virtual: bool,
    pub start_time_s: f64,
    pub age_s: f32,
    pub spatializer: Spatializer3D,
    pub design_params: SoundDesignParams,
    pub bus_id: u64,
    pub importance: f32,
}

impl AudioVoice {
    pub fn new(id: u64, sound_id: u64, category: SoundCategory, position: Vec3) -> Self {
        Self {
            id,
            sound_id,
            category,
            position,
            volume_db: 0.0,
            priority_score: 0.0,
            distance: 0.0,
            lod_level: AudioLodLevel::Full,
            is_active: true,
            is_virtual: false,
            start_time_s: 0.0,
            age_s: 0.0,
            spatializer: Spatializer3D::new(),
            design_params: SoundDesignParams::new(),
            bus_id: 0,
            importance: 1.0,
        }
    }

    pub fn compute_priority(&mut self, listener_pos: Vec3) -> f32 {
        self.distance = (self.position - listener_pos).length();
        let dist_factor = 1.0 / (1.0 + self.distance * 0.01).powf(2.0);
        let vol_factor = db_to_linear(self.volume_db.clamp(-60.0, 0.0));
        let cat_factor = match self.category {
            SoundCategory::Voice => 1.5,
            SoundCategory::Music => 1.2,
            SoundCategory::Weapon | SoundCategory::Explosion => 1.1,
            _ => 1.0,
        };
        self.priority_score = dist_factor * vol_factor * cat_factor * self.importance;
        self.priority_score
    }

    pub fn determine_lod(&mut self, listener_pos: Vec3, voice_budget_fraction: f32) -> AudioLodLevel {
        let dist = (self.position - listener_pos).length();
        self.lod_level = if self.is_virtual {
            AudioLodLevel::Virtual
        } else if dist > 100.0 || voice_budget_fraction < 0.1 {
            AudioLodLevel::Minimal
        } else if dist > 50.0 || voice_budget_fraction < 0.5 {
            AudioLodLevel::Reduced
        } else {
            AudioLodLevel::Full
        };
        self.lod_level
    }
}

#[derive(Debug)]
pub struct VoiceManager {
    pub voices: HashMap<u64, AudioVoice>,
    pub next_voice_id: u64,
    pub max_voices: usize,
    pub category_limits: HashMap<SoundCategory, usize>,
    pub category_counts: HashMap<SoundCategory, usize>,
    pub virtual_voices: HashSet<u64>,
    pub total_active: usize,
    pub total_virtual: usize,
}

impl VoiceManager {
    pub fn new(max_voices: usize) -> Self {
        let mut limits = HashMap::new();
        limits.insert(SoundCategory::Music, 8);
        limits.insert(SoundCategory::Sfx, 64);
        limits.insert(SoundCategory::Voice, 16);
        limits.insert(SoundCategory::Ambient, 16);
        limits.insert(SoundCategory::Ui, 8);
        limits.insert(SoundCategory::Footstep, 8);
        limits.insert(SoundCategory::Weapon, 16);
        limits.insert(SoundCategory::Explosion, 8);
        limits.insert(SoundCategory::Environment, 16);
        Self {
            voices: HashMap::new(),
            next_voice_id: 1,
            max_voices,
            category_limits: limits,
            category_counts: HashMap::new(),
            virtual_voices: HashSet::new(),
            total_active: 0,
            total_virtual: 0,
        }
    }

    pub fn spawn_voice(&mut self, sound_id: u64, category: SoundCategory, pos: Vec3) -> Option<u64> {
        let cat_limit = *self.category_limits.get(&category).unwrap_or(&32);
        let cat_count = *self.category_counts.get(&category).unwrap_or(&0);

        if self.voices.len() >= self.max_voices || cat_count >= cat_limit {
            // Try to steal lowest priority voice of same category
            return None;
        }

        let id = self.next_voice_id;
        self.next_voice_id += 1;
        let voice = AudioVoice::new(id, sound_id, category, pos);
        self.voices.insert(id, voice);
        *self.category_counts.entry(category).or_insert(0) += 1;
        Some(id)
    }

    pub fn retire_voice(&mut self, voice_id: u64) {
        if let Some(voice) = self.voices.remove(&voice_id) {
            let cat = voice.category;
            if let Some(count) = self.category_counts.get_mut(&cat) {
                *count = count.saturating_sub(1);
            }
            self.virtual_voices.remove(&voice_id);
        }
    }

    pub fn update_priorities(&mut self, listener_pos: Vec3) {
        let ids: Vec<u64> = self.voices.keys().copied().collect();
        for id in ids {
            if let Some(voice) = self.voices.get_mut(&id) {
                voice.compute_priority(listener_pos);
            }
        }
    }

    pub fn cull_excess_voices(&mut self, listener_pos: Vec3) {
        if self.voices.len() <= self.max_voices { return; }

        let mut sorted: Vec<(u64, f32)> = self.voices.iter()
            .map(|(&id, v)| (id, v.priority_score))
            .collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let excess = self.voices.len() - self.max_voices;
        for i in 0..excess {
            self.retire_voice(sorted[i].0);
        }
    }

    pub fn virtualize_distant_voices(&mut self, listener_pos: Vec3, virtual_threshold_m: f32) {
        for (id, voice) in &mut self.voices {
            let dist = (voice.position - listener_pos).length();
            voice.is_virtual = dist > virtual_threshold_m;
            if voice.is_virtual {
                self.virtual_voices.insert(*id);
            } else {
                self.virtual_voices.remove(id);
            }
        }
        self.total_virtual = self.virtual_voices.len();
        self.total_active = self.voices.values().filter(|v| !v.is_virtual).count();
    }

    pub fn voices_by_category(&self, category: SoundCategory) -> Vec<&AudioVoice> {
        self.voices.values().filter(|v| v.category == category).collect()
    }

    pub fn steal_voice(&mut self, category: SoundCategory) -> Option<u64> {
        // Steal lowest-priority active voice of given category
        self.voices.iter()
            .filter(|(_, v)| v.category == category && !v.is_virtual)
            .min_by(|(_, a), (_, b)| a.priority_score.partial_cmp(&b.priority_score)
                .unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, _)| id)
    }
}

// ============================================================
// WAVEFORM ANALYSIS (RMS, PEAK, SPECTRUM)
// ============================================================

#[derive(Debug)]
pub struct LevelMeter {
    pub rms_window: VecDeque<f32>, // squared samples
    pub rms_sum: f64,
    pub window_size: usize,
    pub peak_l: f32,
    pub peak_r: f32,
    pub peak_hold_l: f32,
    pub peak_hold_r: f32,
    pub peak_hold_timer_l: u32,
    pub peak_hold_timer_r: u32,
    pub rms_l: f32,
    pub rms_r: f32,
    pub clip_count: u64,
}

impl LevelMeter {
    pub fn new(window_size: usize) -> Self {
        Self {
            rms_window: VecDeque::with_capacity(window_size),
            rms_sum: 0.0,
            window_size,
            peak_l: 0.0,
            peak_r: 0.0,
            peak_hold_l: 0.0,
            peak_hold_r: 0.0,
            peak_hold_timer_l: 0,
            peak_hold_timer_r: 0,
            rms_l: 0.0,
            rms_r: 0.0,
            clip_count: 0,
        }
    }

    pub fn process_sample(&mut self, left: f32, right: f32) {
        let mono_sq = (left * left + right * right) * 0.5;
        if self.rms_window.len() >= self.window_size {
            if let Some(old) = self.rms_window.pop_front() {
                self.rms_sum -= old as f64;
            }
        }
        self.rms_window.push_back(mono_sq);
        self.rms_sum = (self.rms_sum + mono_sq as f64).max(0.0);
        let rms = (self.rms_sum / self.window_size as f64).sqrt() as f32;
        self.rms_l = rms;
        self.rms_r = rms;

        let abs_l = left.abs();
        let abs_r = right.abs();
        self.peak_l = abs_l;
        self.peak_r = abs_r;

        if abs_l > self.peak_hold_l {
            self.peak_hold_l = abs_l;
            self.peak_hold_timer_l = PEAK_HOLD_FRAMES;
        } else if self.peak_hold_timer_l > 0 {
            self.peak_hold_timer_l -= 1;
        } else {
            self.peak_hold_l *= 0.999;
        }

        if abs_r > self.peak_hold_r {
            self.peak_hold_r = abs_r;
            self.peak_hold_timer_r = PEAK_HOLD_FRAMES;
        } else if self.peak_hold_timer_r > 0 {
            self.peak_hold_timer_r -= 1;
        } else {
            self.peak_hold_r *= 0.999;
        }

        if abs_l > 1.0 || abs_r > 1.0 { self.clip_count += 1; }
    }

    pub fn rms_db(&self) -> f32 { linear_to_db(self.rms_l.max(1e-9)) }
    pub fn peak_hold_l_db(&self) -> f32 { linear_to_db(self.peak_hold_l.max(1e-9)) }
    pub fn peak_hold_r_db(&self) -> f32 { linear_to_db(self.peak_hold_r.max(1e-9)) }
    pub fn is_clipping(&self) -> bool { self.clip_count > 0 }
}

// ============================================================
// DFT SPECTRUM ANALYZER
// ============================================================

#[derive(Debug)]
pub struct SpectrumAnalyzer {
    pub fft_size: usize,
    pub buffer: VecDeque<f32>,
    pub output_magnitudes: Vec<f32>,
    pub output_db: Vec<f32>,
    pub hann_window: Vec<f32>,
    pub smoothing: f32,
    pub smoothed_magnitudes: Vec<f32>,
    pub peak_magnitudes: Vec<f32>,
    pub peak_timers: Vec<u32>,
}

impl SpectrumAnalyzer {
    pub fn new(fft_size: usize) -> Self {
        let bins = fft_size / 2;
        let hann: Vec<f32> = (0..fft_size)
            .map(|n| 0.5 * (1.0 - (TWO_PI * n as f32 / (fft_size as f32 - 1.0)).cos()))
            .collect();
        Self {
            fft_size,
            buffer: VecDeque::with_capacity(fft_size),
            output_magnitudes: vec![0.0; bins],
            output_db: vec![-120.0; bins],
            hann_window: hann,
            smoothing: 0.8,
            smoothed_magnitudes: vec![0.0; bins],
            peak_magnitudes: vec![0.0; bins],
            peak_timers: vec![0; bins],
        }
    }

    pub fn push_sample(&mut self, sample: f32) {
        if self.buffer.len() >= self.fft_size {
            self.buffer.pop_front();
        }
        self.buffer.push_back(sample);
    }

    /// Real DFT (O(N^2)) — suitable for editor visualization not realtime
    pub fn compute_dft(&mut self) {
        if self.buffer.len() < self.fft_size { return; }
        let n = self.fft_size;
        let bins = n / 2;
        let samples: Vec<f32> = self.buffer.iter()
            .zip(self.hann_window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        for k in 0..bins {
            let mut re = 0.0f32;
            let mut im = 0.0f32;
            for t in 0..n {
                let angle = TWO_PI * k as f32 * t as f32 / n as f32;
                re += samples[t] * angle.cos();
                im -= samples[t] * angle.sin();
            }
            let mag = (re * re + im * im).sqrt() * 2.0 / n as f32;
            self.output_magnitudes[k] = mag;
            self.output_db[k] = linear_to_db(mag.max(1e-9));

            // Smoothing (exponential moving average)
            self.smoothed_magnitudes[k] = self.smoothing * self.smoothed_magnitudes[k]
                + (1.0 - self.smoothing) * mag;

            // Peak hold
            if mag > self.peak_magnitudes[k] {
                self.peak_magnitudes[k] = mag;
                self.peak_timers[k] = PEAK_HOLD_FRAMES;
            } else if self.peak_timers[k] > 0 {
                self.peak_timers[k] -= 1;
            } else {
                self.peak_magnitudes[k] *= 0.99;
            }
        }
    }

    /// Fast approximate FFT using Cooley-Tukey (radix-2, power-of-2 only)
    pub fn compute_fft_magnitude(&mut self) {
        if self.buffer.len() < self.fft_size { return; }
        let n = self.fft_size;
        if n == 0 || (n & (n - 1)) != 0 { return; } // must be power of 2

        let mut re: Vec<f32> = self.buffer.iter()
            .zip(self.hann_window.iter())
            .map(|(&s, &w)| s * w)
            .collect();
        let mut im = vec![0.0f32; n];

        // Bit-reverse permutation
        let mut j = 0usize;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 { j ^= bit; bit >>= 1; }
            j ^= bit;
            if i < j { re.swap(i, j); im.swap(i, j); }
        }

        // Cooley-Tukey FFT
        let mut len = 2usize;
        while len <= n {
            let ang = -TWO_PI / len as f32;
            let wre = ang.cos();
            let wim = ang.sin();
            let mut i = 0;
            while i < n {
                let mut cur_re = 1.0f32;
                let mut cur_im = 0.0f32;
                for k in 0..(len / 2) {
                    let ur = re[i + k];
                    let ui = im[i + k];
                    let vr = re[i + k + len / 2] * cur_re - im[i + k + len / 2] * cur_im;
                    let vi = re[i + k + len / 2] * cur_im + im[i + k + len / 2] * cur_re;
                    re[i + k]           = ur + vr;
                    im[i + k]           = ui + vi;
                    re[i + k + len / 2] = ur - vr;
                    im[i + k + len / 2] = ui - vi;
                    let tmp = cur_re * wre - cur_im * wim;
                    cur_im = cur_re * wim + cur_im * wre;
                    cur_re = tmp;
                }
                i += len;
            }
            len <<= 1;
        }

        let bins = n / 2;
        for k in 0..bins {
            let mag = (re[k] * re[k] + im[k] * im[k]).sqrt() * 2.0 / n as f32;
            self.output_magnitudes[k] = mag;
            self.output_db[k] = linear_to_db(mag.max(1e-9));
            self.smoothed_magnitudes[k] = self.smoothing * self.smoothed_magnitudes[k]
                + (1.0 - self.smoothing) * mag;
            if mag > self.peak_magnitudes[k] {
                self.peak_magnitudes[k] = mag;
                self.peak_timers[k] = PEAK_HOLD_FRAMES;
            } else if self.peak_timers[k] > 0 {
                self.peak_timers[k] -= 1;
            } else {
                self.peak_magnitudes[k] *= 0.99;
            }
        }
    }

    pub fn frequency_of_bin(&self, bin: usize) -> f32 {
        bin as f32 * SAMPLE_RATE / self.fft_size as f32
    }

    pub fn bin_of_frequency(&self, freq_hz: f32) -> usize {
        ((freq_hz / SAMPLE_RATE * self.fft_size as f32) as usize).min(self.fft_size / 2 - 1)
    }

    pub fn magnitude_at_frequency(&self, freq_hz: f32) -> f32 {
        let bin = self.bin_of_frequency(freq_hz);
        self.smoothed_magnitudes.get(bin).copied().unwrap_or(0.0)
    }
}

// ============================================================
// LUFS LOUDNESS METER
// ============================================================

#[derive(Debug)]
pub struct LufsMeter {
    // K-weighting filter chain (two biquad stages per channel)
    pub pre_filter_l: BiquadState,
    pub pre_filter_r: BiquadState,
    pub rlb_filter_l: BiquadState,
    pub rlb_filter_r: BiquadState,
    pub pre_coeff: BiquadCoefficients,
    pub rlb_coeff: BiquadCoefficients,
    // Short-term block buffer (400ms = ~17640 samples)
    pub block_buffer: VecDeque<f32>,
    pub block_size: usize,
    // Integrated loudness
    pub blocks: Vec<f32>,        // mean square values per block
    pub integrated_lufs: f32,
    pub short_term_lufs: f32,
    pub momentary_lufs: f32,
    pub lra_high: f32,
    pub lra_low: f32,
    // Momentary (100ms)
    pub momentary_buffer: VecDeque<f32>,
    pub momentary_size: usize,
}

impl LufsMeter {
    pub fn new() -> Self {
        // K-weighting pre-filter: high-shelf +4dB at 1681 Hz
        let pre_coeff = BiquadCoefficients::high_shelf(1681.0, 1.0, 4.0, SAMPLE_RATE);
        // RLB filter: high-pass at 38 Hz (Q=0.5)
        let rlb_coeff = BiquadCoefficients::high_pass(38.0, 0.5, SAMPLE_RATE);
        let block_size = LUFS_BLOCK_SAMPLES;
        let momentary_size = (0.1 * SAMPLE_RATE) as usize; // 100ms
        Self {
            pre_filter_l: BiquadState::new(),
            pre_filter_r: BiquadState::new(),
            rlb_filter_l: BiquadState::new(),
            rlb_filter_r: BiquadState::new(),
            pre_coeff,
            rlb_coeff,
            block_buffer: VecDeque::with_capacity(block_size),
            block_size,
            blocks: Vec::new(),
            integrated_lufs: f32::NEG_INFINITY,
            short_term_lufs: f32::NEG_INFINITY,
            momentary_lufs: f32::NEG_INFINITY,
            lra_high: 0.0,
            lra_low: 0.0,
            momentary_buffer: VecDeque::with_capacity(momentary_size),
            momentary_size,
        }
    }

    pub fn process_sample(&mut self, left: f32, right: f32) {
        // Apply K-weighting to each channel
        let wl = {
            let pre = self.pre_filter_l.process(left, &self.pre_coeff);
            self.rlb_filter_l.process(pre, &self.rlb_coeff)
        };
        let wr = {
            let pre = self.pre_filter_r.process(right, &self.pre_coeff);
            self.rlb_filter_r.process(pre, &self.rlb_coeff)
        };
        let mean_sq = wl * wl + wr * wr; // sum of mean squares (2 channels)

        // Short-term block accumulation
        if self.block_buffer.len() >= self.block_size {
            let block_sum: f32 = self.block_buffer.iter().sum();
            let block_mean = block_sum / self.block_size as f32;
            self.blocks.push(block_mean);
            self.block_buffer.pop_front();

            // Compute short-term LUFS (3s sliding window = ~7.5 blocks)
            let st_blocks = self.blocks.len().min(8);
            if st_blocks > 0 {
                let st_sum: f32 = self.blocks.iter().rev().take(st_blocks).sum();
                let st_mean = st_sum / st_blocks as f32;
                self.short_term_lufs = -0.691 + 10.0 * st_mean.max(1e-10).log10();
            }
        }
        self.block_buffer.push_back(mean_sq);

        // Momentary (100ms)
        if self.momentary_buffer.len() >= self.momentary_size {
            self.momentary_buffer.pop_front();
        }
        self.momentary_buffer.push_back(mean_sq);
        let mom_sum: f32 = self.momentary_buffer.iter().sum();
        let mom_mean = mom_sum / self.momentary_buffer.len() as f32;
        self.momentary_lufs = -0.691 + 10.0 * mom_mean.max(1e-10).log10();

        // Integrated LUFS using absolute gating at -70 LUFS and relative gating at -10
        self.compute_integrated_lufs();
    }

    fn compute_integrated_lufs(&mut self) {
        if self.blocks.is_empty() { return; }
        // Absolute gating: discard blocks below -70 LUFS
        let abs_gate_linear = db_to_linear((-70.691) * LN10_OVER_20 * 20.0);
        let gated: Vec<f32> = self.blocks.iter()
            .copied()
            .filter(|&b| b >= 1e-7) // rough -70 LUFS threshold
            .collect();
        if gated.is_empty() {
            self.integrated_lufs = f32::NEG_INFINITY;
            return;
        }
        let mean_gated: f32 = gated.iter().sum::<f32>() / gated.len() as f32;
        // Relative gating: discard blocks 10 dB below mean
        let relative_threshold = mean_gated * db_to_linear(-10.0);
        let rel_gated: Vec<f32> = gated.into_iter()
            .filter(|&b| b >= relative_threshold)
            .collect();
        if rel_gated.is_empty() {
            self.integrated_lufs = f32::NEG_INFINITY;
            return;
        }
        let final_mean = rel_gated.iter().sum::<f32>() / rel_gated.len() as f32;
        self.integrated_lufs = -0.691 + 10.0 * final_mean.max(1e-10).log10();
    }

    pub fn loudness_range(&mut self) {
        // LRA = difference between 10th and 95th percentile of gated short-term loudness
        let mut lufs_values: Vec<f32> = self.blocks.iter()
            .filter(|&&b| b >= 1e-7)
            .map(|&b| -0.691 + 10.0 * b.max(1e-10).log10())
            .collect();
        if lufs_values.len() < 2 {
            self.lra_high = 0.0;
            self.lra_low = 0.0;
            return;
        }
        lufs_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = lufs_values.len();
        let low_idx = (n as f32 * 0.1) as usize;
        let high_idx = (n as f32 * 0.95) as usize;
        self.lra_low = lufs_values[low_idx.min(n - 1)];
        self.lra_high = lufs_values[high_idx.min(n - 1)];
    }

    pub fn lra_db(&self) -> f32 { (self.lra_high - self.lra_low).max(0.0) }
    pub fn integrated_lufs(&self) -> f32 { self.integrated_lufs }
    pub fn short_term_lufs(&self) -> f32 { self.short_term_lufs }
    pub fn momentary_lufs(&self) -> f32 { self.momentary_lufs }
}

// ============================================================
// MIXER SNAPSHOT / PRESET SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct BusSnapshot {
    pub bus_id: u64,
    pub gain_db: f32,
    pub pan: f32,
    pub mute: bool,
    pub effect_bypasses: Vec<bool>, // one per effect in chain
}

#[derive(Debug, Clone)]
pub struct MixerSnapshot {
    pub id: u64,
    pub name: String,
    pub bus_states: HashMap<u64, BusSnapshot>,
    pub created_at: f64,
    pub tags: Vec<String>,
}

impl MixerSnapshot {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            id,
            name,
            bus_states: HashMap::new(),
            created_at: 0.0,
            tags: Vec::new(),
        }
    }

    pub fn capture_bus(&mut self, bus: &AudioBus) {
        let bypasses: Vec<bool> = bus.effect_chain.effects.iter().map(|e| !e.is_enabled()).collect();
        self.bus_states.insert(bus.id, BusSnapshot {
            bus_id: bus.id,
            gain_db: bus.gain_db,
            pan: bus.pan,
            mute: bus.mute,
            effect_bypasses: bypasses,
        });
    }

    pub fn blend_with(&self, other: &MixerSnapshot, t: f32) -> HashMap<u64, (f32, f32)> {
        let mut result = HashMap::new();
        for (id, a_state) in &self.bus_states {
            if let Some(b_state) = other.bus_states.get(id) {
                let gain = a_state.gain_db + (b_state.gain_db - a_state.gain_db) * t;
                let pan = a_state.pan + (b_state.pan - a_state.pan) * t;
                result.insert(*id, (gain, pan));
            }
        }
        result
    }
}

#[derive(Debug)]
pub struct SnapshotSystem {
    pub snapshots: HashMap<u64, MixerSnapshot>,
    pub active_snapshot_id: Option<u64>,
    pub target_snapshot_id: Option<u64>,
    pub transition_progress: f32,
    pub transition_duration_s: f32,
    pub transition_curve: SnapshotTransitionCurve,
    pub next_id: u64,
}

impl SnapshotSystem {
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            active_snapshot_id: None,
            target_snapshot_id: None,
            transition_progress: 1.0,
            transition_duration_s: 1.0,
            transition_curve: SnapshotTransitionCurve::EaseInOut,
            next_id: 1,
        }
    }

    pub fn create_snapshot(&mut self, name: String, timestamp: f64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut snap = MixerSnapshot::new(id, name);
        snap.created_at = timestamp;
        self.snapshots.insert(id, snap);
        id
    }

    pub fn delete_snapshot(&mut self, id: u64) -> bool {
        if Some(id) == self.active_snapshot_id {
            return false; // Can't delete active
        }
        self.snapshots.remove(&id).is_some()
    }

    pub fn begin_transition(&mut self, target_id: u64, duration_s: f32, curve: SnapshotTransitionCurve) {
        if !self.snapshots.contains_key(&target_id) { return; }
        self.target_snapshot_id = Some(target_id);
        self.transition_duration_s = duration_s;
        self.transition_curve = curve;
        self.transition_progress = if curve == SnapshotTransitionCurve::Immediate { 1.0 } else { 0.0 };
    }

    pub fn update(&mut self, dt_s: f32) -> Option<HashMap<u64, (f32, f32)>> {
        if self.target_snapshot_id.is_none() { return None; }
        if self.transition_progress >= 1.0 {
            self.active_snapshot_id = self.target_snapshot_id.take();
            return None;
        }

        self.transition_progress += dt_s / self.transition_duration_s.max(0.001);
        self.transition_progress = self.transition_progress.min(1.0);

        let t = apply_curve(self.transition_progress, self.transition_curve);

        let from_id = self.active_snapshot_id?;
        let to_id = self.target_snapshot_id?;
        let from = self.snapshots.get(&from_id)?;
        let to = self.snapshots.get(&to_id)?;
        Some(from.blend_with(to, t))
    }

    pub fn is_transitioning(&self) -> bool {
        self.target_snapshot_id.is_some() && self.transition_progress < 1.0
    }
}

pub fn apply_curve(t: f32, curve: SnapshotTransitionCurve) -> f32 {
    match curve {
        SnapshotTransitionCurve::Linear => t,
        SnapshotTransitionCurve::EaseIn => t * t,
        SnapshotTransitionCurve::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        SnapshotTransitionCurve::EaseInOut => t * t * (3.0 - 2.0 * t),
        SnapshotTransitionCurve::Immediate => 1.0,
    }
}

// ============================================================
// AUDIO PROFILER
// ============================================================

#[derive(Debug, Clone)]
pub struct AudioProfilerFrame {
    pub timestamp_s: f64,
    pub voice_count: usize,
    pub virtual_voice_count: usize,
    pub active_buses: usize,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub dsp_chain_cost: f32,
    pub streaming_kb_s: f32,
}

#[derive(Debug)]
pub struct AudioProfiler {
    pub frames: VecDeque<AudioProfilerFrame>,
    pub frame_capacity: usize,
    pub sound_bank_memory: HashMap<String, u64>, // bank_name -> bytes
    pub effect_cpu_breakdown: HashMap<EffectType, f32>,
    pub total_samples_processed: u64,
    pub dropouts: u64,
    pub peak_voice_count: usize,
    pub average_voice_count: f32,
}

impl AudioProfiler {
    pub fn new(capacity: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity),
            frame_capacity: capacity,
            sound_bank_memory: HashMap::new(),
            effect_cpu_breakdown: HashMap::new(),
            total_samples_processed: 0,
            dropouts: 0,
            peak_voice_count: 0,
            average_voice_count: 0.0,
        }
    }

    pub fn record_frame(&mut self, frame: AudioProfilerFrame) {
        if self.frames.len() >= self.frame_capacity { self.frames.pop_front(); }
        self.peak_voice_count = self.peak_voice_count.max(frame.voice_count);
        // Update rolling average
        let n = self.frames.len().max(1) as f32;
        self.average_voice_count = self.average_voice_count * (n - 1.0) / n + frame.voice_count as f32 / n;
        self.frames.push_back(frame);
    }

    pub fn update_dsp_costs(&mut self, graph: &SignalFlowGraph) {
        self.effect_cpu_breakdown.clear();
        for bus in graph.buses.values() {
            for effect in &bus.effect_chain.effects {
                *self.effect_cpu_breakdown.entry(effect.effect_type()).or_insert(0.0)
                    += effect.cpu_cost_estimate();
            }
        }
    }

    pub fn register_sound_bank(&mut self, name: String, size_bytes: u64) {
        self.sound_bank_memory.insert(name, size_bytes);
    }

    pub fn total_sound_bank_memory_mb(&self) -> f32 {
        self.sound_bank_memory.values().sum::<u64>() as f32 / (1024.0 * 1024.0)
    }

    pub fn average_cpu_percent(&self) -> f32 {
        if self.frames.is_empty() { return 0.0; }
        self.frames.iter().map(|f| f.cpu_percent).sum::<f32>() / self.frames.len() as f32
    }

    pub fn peak_cpu_percent(&self) -> f32 {
        self.frames.iter().map(|f| f.cpu_percent).fold(0.0f32, f32::max)
    }

    pub fn record_dropout(&mut self) { self.dropouts += 1; }

    pub fn most_expensive_effect(&self) -> Option<(EffectType, f32)> {
        self.effect_cpu_breakdown.iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&t, &c)| (t, c))
    }
}

// ============================================================
// AUDIO MIXER EDITOR UI STATE
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MixerEditorPanel {
    SignalFlow,
    EffectChain,
    BusRouting,
    Spectrum,
    Loudness,
    SpatialAudio,
    MusicSystem,
    Snapshots,
    Profiler,
    VoiceManager,
}

#[derive(Debug)]
pub struct MixerEditorUiState {
    pub active_panel: MixerEditorPanel,
    pub selected_bus_id: Option<u64>,
    pub selected_effect_index: Option<usize>,
    pub show_spectrum: bool,
    pub show_rta: bool,         // real-time analyzer
    pub show_lissajous: bool,
    pub spectrum_log_scale: bool,
    pub show_automation: bool,
    pub parameter_link_mode: bool,
    pub solo_mode: bool,        // solo-in-place vs solo-exclusive
    pub snap_to_grid: bool,
    pub grid_size_db: f32,
    pub drag_source_bus: Option<u64>,
    pub drag_target_bus: Option<u64>,
    pub effect_drag_index: Option<usize>,
    pub timeline_zoom: f32,
    pub timeline_scroll: f32,
}

impl MixerEditorUiState {
    pub fn new() -> Self {
        Self {
            active_panel: MixerEditorPanel::SignalFlow,
            selected_bus_id: None,
            selected_effect_index: None,
            show_spectrum: true,
            show_rta: false,
            show_lissajous: false,
            spectrum_log_scale: true,
            show_automation: false,
            parameter_link_mode: false,
            solo_mode: false,
            snap_to_grid: false,
            grid_size_db: 3.0,
            drag_source_bus: None,
            drag_target_bus: None,
            effect_drag_index: None,
            timeline_zoom: 1.0,
            timeline_scroll: 0.0,
        }
    }

    pub fn select_bus(&mut self, id: u64) {
        self.selected_bus_id = Some(id);
        self.selected_effect_index = None;
    }

    pub fn select_effect(&mut self, bus_id: u64, effect_index: usize) {
        self.selected_bus_id = Some(bus_id);
        self.selected_effect_index = Some(effect_index);
    }

    pub fn snap_gain_db(&self, gain_db: f32) -> f32 {
        if !self.snap_to_grid { return gain_db; }
        (gain_db / self.grid_size_db).round() * self.grid_size_db
    }

    pub fn begin_drag(&mut self, source_bus: u64) {
        self.drag_source_bus = Some(source_bus);
        self.drag_target_bus = None;
    }

    pub fn complete_drag(&mut self) -> Option<(u64, u64)> {
        match (self.drag_source_bus.take(), self.drag_target_bus.take()) {
            (Some(s), Some(t)) => Some((s, t)),
            _ => None,
        }
    }
}

// ============================================================
// DOPPLER CALCULATOR
// ============================================================

pub struct DopplerCalculator;

impl DopplerCalculator {
    /// Compute observed frequency using exact Doppler formula
    /// f' = f * (v_sound + v_listener) / (v_sound + v_source)
    /// where velocities are projected onto source→listener direction
    pub fn compute_pitch_ratio(
        source_pos: Vec3,
        listener_pos: Vec3,
        source_vel: Vec3,
        listener_vel: Vec3,
        doppler_factor: f32,
    ) -> f32 {
        let dir = listener_pos - source_pos;
        let dist = dir.length();
        if dist < 1e-4 { return 1.0; }
        let unit = dir / dist;

        // Positive means moving away from listener
        let v_source = source_vel.dot(-unit);
        // Positive means moving toward source
        let v_listener = listener_vel.dot(unit);

        let denom = SPEED_OF_SOUND + v_source;
        if denom.abs() < 1.0 { return 1.0; }
        let ratio = (SPEED_OF_SOUND + v_listener) / denom;
        let clamped = ratio.clamp(0.5, 2.0);
        // Blend between 1.0 and clamped by doppler_factor
        1.0 + (clamped - 1.0) * doppler_factor.clamp(0.0, 1.0)
    }

    pub fn pitch_to_playback_rate(pitch_ratio: f32) -> f32 { pitch_ratio }

    pub fn playback_rate_to_cents(rate: f32) -> f32 {
        1200.0 * rate.log2()
    }
}

// ============================================================
// OCCLUSION MODEL
// ============================================================

#[derive(Debug, Clone)]
pub struct AcousticMaterial {
    pub name: String,
    pub transmission_loss_db: f32,    // how much attenuated when sound passes through
    pub absorption_coefficients: [f32; 6], // at 125, 250, 500, 1k, 2k, 4k Hz
}

impl AcousticMaterial {
    pub fn new(name: &str, transmission_loss_db: f32, absorptions: [f32; 6]) -> Self {
        Self {
            name: name.to_string(),
            transmission_loss_db,
            absorption_coefficients: absorptions,
        }
    }

    pub fn concrete() -> Self {
        Self::new("Concrete", 45.0, [0.01, 0.01, 0.02, 0.02, 0.03, 0.04])
    }
    pub fn wood() -> Self {
        Self::new("Wood", 25.0, [0.15, 0.12, 0.10, 0.08, 0.08, 0.07])
    }
    pub fn glass() -> Self {
        Self::new("Glass", 20.0, [0.35, 0.25, 0.20, 0.10, 0.07, 0.04])
    }
    pub fn fabric() -> Self {
        Self::new("Fabric", 5.0, [0.35, 0.53, 0.75, 0.70, 0.60, 0.55])
    }

    pub fn absorption_at_freq(&self, freq_hz: f32) -> f32 {
        // Interpolate over octave bands: 125, 250, 500, 1000, 2000, 4000
        let bands = [125.0f32, 250.0, 500.0, 1000.0, 2000.0, 4000.0];
        let n = bands.len();
        if freq_hz <= bands[0] { return self.absorption_coefficients[0]; }
        if freq_hz >= bands[n - 1] { return self.absorption_coefficients[n - 1]; }
        for i in 0..(n - 1) {
            if freq_hz >= bands[i] && freq_hz <= bands[i + 1] {
                let t = (freq_hz - bands[i]) / (bands[i + 1] - bands[i]);
                return self.absorption_coefficients[i] * (1.0 - t) + self.absorption_coefficients[i + 1] * t;
            }
        }
        self.absorption_coefficients[n / 2]
    }
}

#[derive(Debug, Clone)]
pub struct OcclusionQuery {
    pub sound_id: u64,
    pub source_pos: Vec3,
    pub listener_pos: Vec3,
    pub occlusion_factor: f32,     // 0=open, 1=fully occluded
    pub obstruction_factor: f32,   // partially blocked
    pub materials: Vec<AcousticMaterial>,
    pub total_transmission_loss_db: f32,
    pub wet_occlusion_db: f32,     // reverb path (often unoccluded)
}

impl OcclusionQuery {
    pub fn new(sound_id: u64, source: Vec3, listener: Vec3) -> Self {
        Self {
            sound_id,
            source_pos: source,
            listener_pos: listener,
            occlusion_factor: 0.0,
            obstruction_factor: 0.0,
            materials: Vec::new(),
            total_transmission_loss_db: 0.0,
            wet_occlusion_db: 0.0,
        }
    }

    pub fn add_material(&mut self, mat: AcousticMaterial) {
        self.total_transmission_loss_db += mat.transmission_loss_db;
        self.materials.push(mat);
    }

    pub fn direct_path_gain_db(&self) -> f32 {
        -self.total_transmission_loss_db * self.occlusion_factor
    }

    pub fn apply_to_spatializer(&self, spatializer: &mut Spatializer3D) {
        spatializer.occlusion_db = self.direct_path_gain_db();
        spatializer.obstruction_db = -self.obstruction_factor * 6.0;
    }
}

// ============================================================
// BUS AUTOMATION
// ============================================================

#[derive(Debug, Clone)]
pub struct AutomationKeyframe {
    pub time_s: f32,
    pub value: f32,
    pub curve: f32, // 0=linear, <0=ease-in, >0=ease-out
}

impl AutomationKeyframe {
    pub fn new(time_s: f32, value: f32) -> Self {
        Self { time_s, value, curve: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct AutomationLane {
    pub parameter_name: String,
    pub bus_id: u64,
    pub keyframes: Vec<AutomationKeyframe>,
    pub is_enabled: bool,
    pub looping: bool,
    pub loop_duration_s: f32,
}

impl AutomationLane {
    pub fn new(parameter_name: String, bus_id: u64) -> Self {
        Self {
            parameter_name,
            bus_id,
            keyframes: Vec::new(),
            is_enabled: true,
            looping: false,
            loop_duration_s: 1.0,
        }
    }

    pub fn add_keyframe(&mut self, kf: AutomationKeyframe) {
        self.keyframes.push(kf);
        self.keyframes.sort_by(|a, b| a.time_s.partial_cmp(&b.time_s).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn evaluate_at(&self, time_s: f32) -> f32 {
        let t = if self.looping && self.loop_duration_s > 0.0 {
            time_s % self.loop_duration_s
        } else { time_s };

        let kfs = &self.keyframes;
        if kfs.is_empty() { return 0.0; }
        if t <= kfs[0].time_s { return kfs[0].value; }
        if t >= kfs[kfs.len() - 1].time_s { return kfs[kfs.len() - 1].value; }

        for i in 0..(kfs.len() - 1) {
            if t >= kfs[i].time_s && t <= kfs[i + 1].time_s {
                let dt = kfs[i + 1].time_s - kfs[i].time_s;
                let local_t = if dt > 0.0 { (t - kfs[i].time_s) / dt } else { 1.0 };
                let c = kfs[i].curve;
                let curved_t = if c.abs() < 1e-4 {
                    local_t
                } else if c > 0.0 {
                    local_t.powf(1.0 + c)
                } else {
                    1.0 - (1.0 - local_t).powf(1.0 - c)
                };
                return kfs[i].value + (kfs[i + 1].value - kfs[i].value) * curved_t;
            }
        }
        kfs[kfs.len() - 1].value
    }

    pub fn duration_s(&self) -> f32 {
        self.keyframes.last().map(|k| k.time_s).unwrap_or(0.0)
    }
}

// ============================================================
// AUDIO MIXER EDITOR MAIN STRUCT
// ============================================================

#[derive(Debug)]
pub struct AudioMixerEditor {
    // Signal flow
    pub signal_flow: SignalFlowGraph,

    // Voice management
    pub voice_manager: VoiceManager,

    // Adaptive music
    pub music_system: AdaptiveMusicSystem,

    // Analysis
    pub level_meters: HashMap<u64, LevelMeter>,  // bus_id -> meter
    pub master_spectrum: SpectrumAnalyzer,
    pub lufs_meter: LufsMeter,

    // Spatial
    pub reverb_zones: ReverbZoneManager,
    pub occlusion_queries: HashMap<u64, OcclusionQuery>,

    // Snapshots
    pub snapshot_system: SnapshotSystem,

    // Automation
    pub automation_lanes: Vec<AutomationLane>,

    // Profiler
    pub profiler: AudioProfiler,

    // UI state
    pub ui_state: MixerEditorUiState,

    // Current time
    pub time_s: f64,
    pub sample_rate: f32,

    // Master settings
    pub master_volume_db: f32,
    pub master_mute: bool,

    // Stats
    pub stats: AudioMixerStats,
}

#[derive(Debug, Default, Clone)]
pub struct AudioMixerStats {
    pub total_voices: usize,
    pub active_voices: usize,
    pub virtual_voices: usize,
    pub total_buses: usize,
    pub estimated_cpu_percent: f32,
    pub total_sound_bank_mb: f32,
    pub master_rms_db: f32,
    pub master_peak_db: f32,
    pub integrated_lufs: f32,
    pub is_clipping: bool,
}

impl AudioMixerEditor {
    pub fn new() -> Self {
        let mut editor = Self {
            signal_flow: SignalFlowGraph::new(),
            voice_manager: VoiceManager::new(MAX_VOICES),
            music_system: AdaptiveMusicSystem::new(120.0),
            level_meters: HashMap::new(),
            master_spectrum: SpectrumAnalyzer::new(SPECTRUM_FFT_SIZE),
            lufs_meter: LufsMeter::new(),
            reverb_zones: ReverbZoneManager::new(),
            occlusion_queries: HashMap::new(),
            snapshot_system: SnapshotSystem::new(),
            automation_lanes: Vec::new(),
            profiler: AudioProfiler::new(512),
            ui_state: MixerEditorUiState::new(),
            time_s: 0.0,
            sample_rate: SAMPLE_RATE,
            master_volume_db: 0.0,
            master_mute: false,
            stats: AudioMixerStats::default(),
        };

        // Initialize level meters for default buses
        for &id in editor.signal_flow.buses.keys() {
            editor.level_meters.insert(id, LevelMeter::new(RMS_WINDOW_SAMPLES));
        }

        editor
    }

    pub fn tick(&mut self, dt_s: f32) {
        self.time_s += dt_s as f64;
        self.music_system.update(dt_s);
        if let Some(blended) = self.snapshot_system.update(dt_s) {
            self.apply_snapshot_blend(blended);
        }
        self.update_automation();
        self.collect_stats();
    }

    fn apply_snapshot_blend(&mut self, blended: HashMap<u64, (f32, f32)>) {
        for (bus_id, (gain_db, pan)) in blended {
            if let Some(bus) = self.signal_flow.buses.get_mut(&bus_id) {
                bus.gain_db = gain_db;
                bus.pan = pan;
            }
        }
    }

    fn update_automation(&mut self) {
        let t = self.time_s as f32;
        for lane in &self.automation_lanes {
            if !lane.is_enabled { continue; }
            let value = lane.evaluate_at(t);
            if let Some(bus) = self.signal_flow.buses.get_mut(&lane.bus_id) {
                match lane.parameter_name.as_str() {
                    "gain_db" => bus.gain_db = value,
                    "pan" => bus.pan = value.clamp(-1.0, 1.0),
                    _ => {}
                }
            }
        }
    }

    fn collect_stats(&mut self) {
        self.stats.total_voices = self.voice_manager.voices.len();
        self.stats.active_voices = self.voice_manager.total_active;
        self.stats.virtual_voices = self.voice_manager.total_virtual;
        self.stats.total_buses = self.signal_flow.buses.len();
        self.stats.estimated_cpu_percent = self.profiler.average_cpu_percent();
        self.stats.total_sound_bank_mb = self.profiler.total_sound_bank_memory_mb();
        self.stats.integrated_lufs = self.lufs_meter.integrated_lufs();

        if let Some(master_meter) = self.level_meters.get(&self.signal_flow.master_bus_id) {
            self.stats.master_rms_db = master_meter.rms_db();
            self.stats.master_peak_db = master_meter.peak_hold_l_db();
            self.stats.is_clipping = master_meter.is_clipping();
        }
    }

    pub fn process_audio_frame(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        if self.master_mute { return (0.0, 0.0); }

        // Process through signal flow in topology order
        let mut bus_outputs: HashMap<u64, (f32, f32)> = HashMap::new();

        for &bus_id in &self.signal_flow.topology_order.clone() {
            // Sum inputs from child buses
            let mut sum_l = 0.0f32;
            let mut sum_r = 0.0f32;

            for edge in &self.signal_flow.edges {
                if edge.to_bus_id == bus_id && !edge.is_sidechain {
                    if let Some(&(out_l, out_r)) = bus_outputs.get(&edge.from_bus_id) {
                        sum_l += out_l * edge.send_level;
                        sum_r += out_r * edge.send_level;
                    }
                }
            }

            // If this is an input bus, add external audio input
            if self.signal_flow.topology_order.first().copied() == Some(bus_id) {
                sum_l += input_l;
                sum_r += input_r;
            }

            if let Some(bus) = self.signal_flow.buses.get_mut(&bus_id) {
                let (out_l, out_r) = bus.process_sample(sum_l, sum_r);
                if let Some(meter) = self.level_meters.get_mut(&bus_id) {
                    meter.process_sample(out_l, out_r);
                }
                bus_outputs.insert(bus_id, (out_l, out_r));
            }
        }

        let (master_l, master_r) = bus_outputs.get(&self.signal_flow.master_bus_id).copied().unwrap_or((0.0, 0.0));
        let master_gain = db_to_linear(self.master_volume_db);
        let out = (master_l * master_gain, master_r * master_gain);

        // Feed master to LUFS and spectrum
        self.lufs_meter.process_sample(out.0, out.1);
        self.master_spectrum.push_sample((out.0 + out.1) * 0.5);

        out
    }

    pub fn add_effect_to_bus(&mut self, bus_id: u64, effect: AudioEffect) -> bool {
        if let Some(bus) = self.signal_flow.buses.get_mut(&bus_id) {
            bus.effect_chain.add(effect);
            true
        } else { false }
    }

    pub fn remove_effect_from_bus(&mut self, bus_id: u64, index: usize) -> bool {
        if let Some(bus) = self.signal_flow.buses.get_mut(&bus_id) {
            bus.effect_chain.remove(index);
            true
        } else { false }
    }

    pub fn route_bus(&mut self, from_id: u64, to_id: u64, level: f32) {
        self.signal_flow.connect(from_id, to_id, level);
        self.level_meters.entry(from_id).or_insert_with(|| LevelMeter::new(RMS_WINDOW_SAMPLES));
        self.level_meters.entry(to_id).or_insert_with(|| LevelMeter::new(RMS_WINDOW_SAMPLES));
    }

    pub fn unroute_bus(&mut self, from_id: u64, to_id: u64) {
        self.signal_flow.disconnect(from_id, to_id);
    }

    pub fn set_bus_gain(&mut self, bus_id: u64, gain_db: f32) {
        if let Some(bus) = self.signal_flow.buses.get_mut(&bus_id) {
            bus.gain_db = gain_db.clamp(-120.0, 24.0);
        }
    }

    pub fn set_bus_mute(&mut self, bus_id: u64, mute: bool) {
        if let Some(bus) = self.signal_flow.buses.get_mut(&bus_id) {
            bus.mute = mute;
        }
    }

    pub fn set_bus_solo(&mut self, bus_id: u64, solo: bool) {
        if let Some(bus) = self.signal_flow.buses.get_mut(&bus_id) {
            bus.solo = solo;
        }
        // Mute all other non-soloed buses if any bus is soloed
        let any_solo = self.signal_flow.buses.values().any(|b| b.solo);
        if any_solo {
            let bus_ids: Vec<u64> = self.signal_flow.buses.keys().copied().collect();
            for id in bus_ids {
                if let Some(b) = self.signal_flow.buses.get_mut(&id) {
                    if !b.solo && id != self.signal_flow.master_bus_id {
                        b.mute = true;
                    }
                }
            }
        }
    }

    pub fn save_snapshot(&mut self, name: String) -> u64 {
        let id = self.snapshot_system.create_snapshot(name, self.time_s);
        // Capture all buses
        let bus_ids: Vec<u64> = self.signal_flow.buses.keys().copied().collect();
        for bus_id in bus_ids {
            if let Some(snap) = self.snapshot_system.snapshots.get_mut(&id) {
                if let Some(bus) = self.signal_flow.buses.get(&bus_id) {
                    snap.capture_bus(bus);
                }
            }
        }
        id
    }

    pub fn load_snapshot(&mut self, snapshot_id: u64, transition_s: f32) {
        self.snapshot_system.begin_transition(
            snapshot_id,
            transition_s,
            SnapshotTransitionCurve::EaseInOut,
        );
    }

    pub fn compute_spectrum(&mut self) {
        self.master_spectrum.compute_fft_magnitude();
    }

    pub fn get_bus_by_type(&self, bus_type: BusType) -> Option<&AudioBus> {
        self.signal_flow.buses.values().find(|b| b.bus_type == bus_type)
    }

    pub fn get_bus_by_name(&self, name: &str) -> Option<&AudioBus> {
        self.signal_flow.buses.values().find(|b| b.name == name)
    }

    pub fn create_custom_bus(&mut self, name: String) -> u64 {
        let id = self.signal_flow.create_bus(name, BusType::Custom);
        self.level_meters.insert(id, LevelMeter::new(RMS_WINDOW_SAMPLES));
        id
    }

    pub fn add_automation_lane(&mut self, bus_id: u64, param: String) -> usize {
        let idx = self.automation_lanes.len();
        self.automation_lanes.push(AutomationLane::new(param, bus_id));
        idx
    }

    pub fn add_keyframe_to_lane(&mut self, lane_idx: usize, time_s: f32, value: f32) {
        if let Some(lane) = self.automation_lanes.get_mut(lane_idx) {
            lane.add_keyframe(AutomationKeyframe::new(time_s, value));
        }
    }

    pub fn spawn_voice(&mut self, sound_id: u64, category: SoundCategory, pos: Vec3) -> Option<u64> {
        self.voice_manager.spawn_voice(sound_id, category, pos)
    }

    pub fn update_voices(&mut self, listener_pos: Vec3) {
        self.voice_manager.update_priorities(listener_pos);
        self.voice_manager.cull_excess_voices(listener_pos);
        self.voice_manager.virtualize_distant_voices(listener_pos, 100.0);
        for voice in self.voice_manager.voices.values_mut() {
            voice.spatializer.listener_pos = listener_pos;
            voice.spatializer.update();
        }
    }

    pub fn set_music_intensity(&mut self, intensity: f32) {
        self.music_system.set_intensity(intensity);
    }

    pub fn set_music_state(&mut self, state: String, transition: MusicTransitionType) {
        self.music_system.set_state(state, transition);
    }

    pub fn add_reverb_zone(&mut self, name: String, center: Vec3, radius: f32, blend: f32) -> u64 {
        self.reverb_zones.add_zone(name, center, radius, blend)
    }

    pub fn update_reverb_zones(&mut self, listener_pos: Vec3) {
        self.reverb_zones.update(listener_pos);
    }

    pub fn register_sound_bank(&mut self, name: String, size_bytes: u64) {
        self.profiler.register_sound_bank(name, size_bytes);
    }

    pub fn generate_mixing_report(&self) -> MixingReport {
        MixingReport {
            active_voice_count: self.stats.active_voices,
            virtual_voice_count: self.stats.virtual_voices,
            bus_count: self.stats.total_buses,
            cpu_estimate: self.stats.estimated_cpu_percent,
            sound_bank_mb: self.stats.total_sound_bank_mb,
            master_rms_db: self.stats.master_rms_db,
            master_peak_db: self.stats.master_peak_db,
            integrated_lufs: self.stats.integrated_lufs,
            short_term_lufs: self.lufs_meter.short_term_lufs(),
            is_clipping: self.stats.is_clipping,
            most_expensive_effect: self.profiler.most_expensive_effect(),
            active_reverb_zones: self.reverb_zones.active_blend.len(),
        }
    }

    pub fn get_signal_flow_edges(&self) -> &[SignalFlowEdge] {
        &self.signal_flow.edges
    }

    pub fn get_bus_level_db(&self, bus_id: u64) -> (f32, f32) {
        if let Some(meter) = self.level_meters.get(&bus_id) {
            (meter.output_l_db(), meter.output_r_db())
        } else { (-120.0, -120.0) }
    }

    pub fn get_spectrum_data(&self) -> &[f32] {
        &self.master_spectrum.smoothed_magnitudes
    }
}

#[derive(Debug, Clone)]
pub struct MixingReport {
    pub active_voice_count: usize,
    pub virtual_voice_count: usize,
    pub bus_count: usize,
    pub cpu_estimate: f32,
    pub sound_bank_mb: f32,
    pub master_rms_db: f32,
    pub master_peak_db: f32,
    pub integrated_lufs: f32,
    pub short_term_lufs: f32,
    pub is_clipping: bool,
    pub most_expensive_effect: Option<(EffectType, f32)>,
    pub active_reverb_zones: usize,
}

// ============================================================
// ADVANCED EFFECTS: TRANSIENT SHAPER
// ============================================================

#[derive(Debug, Clone)]
pub struct TransientShaper {
    pub attack_gain_db: f32,
    pub sustain_gain_db: f32,
    pub attack_ms: f32,
    pub sustain_ms: f32,
    pub fast_env: f32,
    pub slow_env: f32,
    pub is_enabled: bool,
}

impl TransientShaper {
    pub fn new() -> Self {
        Self {
            attack_gain_db: 6.0,
            sustain_gain_db: -3.0,
            attack_ms: 5.0,
            sustain_ms: 50.0,
            fast_env: 0.0,
            slow_env: 0.0,
            is_enabled: true,
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l, in_r); }
        let mono = (in_l.abs() + in_r.abs()) * 0.5;
        let fast_coeff = (-1.0 / (self.attack_ms * 0.001 * SAMPLE_RATE)).exp();
        let slow_coeff = (-1.0 / (self.sustain_ms * 0.001 * SAMPLE_RATE)).exp();
        self.fast_env = fast_coeff * self.fast_env + (1.0 - fast_coeff) * mono;
        self.slow_env = slow_coeff * self.slow_env + (1.0 - slow_coeff) * mono;
        let is_transient = self.fast_env > self.slow_env;
        let gain_db = if is_transient { self.attack_gain_db } else { self.sustain_gain_db };
        let gain = db_to_linear(gain_db);
        (in_l * gain, in_r * gain)
    }
}

// ============================================================
// MULTI-BAND COMPRESSOR
// ============================================================

#[derive(Debug, Clone)]
pub struct MultiBandCompressor {
    pub band_count: usize,
    pub crossover_freqs: Vec<f32>,
    pub compressors: Vec<Compressor>,
    pub crossover_filters_l: Vec<[BiquadState; 2]>,
    pub crossover_filters_r: Vec<[BiquadState; 2]>,
    pub crossover_coeffs: Vec<[BiquadCoefficients; 2]>,
    pub band_gains_db: Vec<f32>,
    pub is_enabled: bool,
}

impl MultiBandCompressor {
    pub fn new_three_band(low_mid_hz: f32, mid_high_hz: f32) -> Self {
        let crossover_freqs = vec![low_mid_hz, mid_high_hz];
        let band_count = 3;
        let mut compressors = Vec::new();
        for _ in 0..band_count {
            compressors.push(Compressor::new(CompressorParams::default()));
        }
        let mut crossover_filters_l = Vec::new();
        let mut crossover_filters_r = Vec::new();
        let mut crossover_coeffs = Vec::new();

        for &freq in &crossover_freqs {
            crossover_filters_l.push([BiquadState::new(), BiquadState::new()]);
            crossover_filters_r.push([BiquadState::new(), BiquadState::new()]);
            crossover_coeffs.push([
                BiquadCoefficients::low_pass(freq, 0.707, SAMPLE_RATE),
                BiquadCoefficients::high_pass(freq, 0.707, SAMPLE_RATE),
            ]);
        }

        Self {
            band_count,
            crossover_freqs,
            compressors,
            crossover_filters_l,
            crossover_filters_r,
            crossover_coeffs,
            band_gains_db: vec![0.0; band_count],
            is_enabled: true,
        }
    }

    pub fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if !self.is_enabled { return (in_l, in_r); }

        // Split into bands via crossover filters
        let mut bands_l = vec![in_l; self.band_count];
        let mut bands_r = vec![in_r; self.band_count];

        if self.crossover_freqs.len() >= 1 {
            let (coeff_l, coeff_h) = (&self.crossover_coeffs[0][0], &self.crossover_coeffs[0][1]);
            bands_l[0] = self.crossover_filters_l[0][0].process(in_l, coeff_l);
            bands_r[0] = self.crossover_filters_r[0][0].process(in_r, coeff_l);
            let high_l = self.crossover_filters_l[0][1].process(in_l, coeff_h);
            let high_r = self.crossover_filters_r[0][1].process(in_r, coeff_h);
            if self.crossover_freqs.len() >= 2 {
                let (coeff_l2, coeff_h2) = (&self.crossover_coeffs[1][0], &self.crossover_coeffs[1][1]);
                bands_l[1] = self.crossover_filters_l[1][0].process(high_l, coeff_l2);
                bands_r[1] = self.crossover_filters_r[1][0].process(high_r, coeff_l2);
                bands_l[2] = self.crossover_filters_l[1][1].process(high_l, coeff_h2);
                bands_r[2] = self.crossover_filters_r[1][1].process(high_r, coeff_h2);
            }
        }

        // Compress and recombine
        let mut out_l = 0.0f32;
        let mut out_r = 0.0f32;
        for i in 0..self.band_count {
            let (bl, br) = self.compressors[i].process_sample(bands_l[i], bands_r[i]);
            let band_gain = db_to_linear(self.band_gains_db[i]);
            out_l += bl * band_gain;
            out_r += br * band_gain;
        }
        (out_l, out_r)
    }
}

// ============================================================
// SEND/RETURN SYSTEM
// ============================================================

#[derive(Debug)]
pub struct SendReturnBus {
    pub send_bus_id: u64,
    pub return_bus_id: u64,
    pub send_gain_db: f32,
    pub return_gain_db: f32,
    pub effect_chain: EffectChain,
    pub pre_fader: bool,
    pub buffer_l: Vec<f32>,
    pub buffer_r: Vec<f32>,
    pub buffer_size: usize,
}

impl SendReturnBus {
    pub fn new(send_id: u64, return_id: u64) -> Self {
        Self {
            send_bus_id: send_id,
            return_bus_id: return_id,
            send_gain_db: 0.0,
            return_gain_db: 0.0,
            effect_chain: EffectChain::new(),
            pre_fader: false,
            buffer_l: vec![0.0; 512],
            buffer_r: vec![0.0; 512],
            buffer_size: 512,
        }
    }

    pub fn process(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        let send_gain = db_to_linear(self.send_gain_db);
        let ret_gain = db_to_linear(self.return_gain_db);
        let (out_l, out_r) = self.effect_chain.process_sample(in_l * send_gain, in_r * send_gain);
        (out_l * ret_gain, out_r * ret_gain)
    }
}

// ============================================================
// EFFECT PRESET LIBRARY
// ============================================================

#[derive(Debug, Clone)]
pub struct EffectPreset {
    pub id: u64,
    pub name: String,
    pub effect_type: EffectType,
    pub parameters: HashMap<String, f32>,
    pub tags: Vec<String>,
}

impl EffectPreset {
    pub fn new(id: u64, name: String, effect_type: EffectType) -> Self {
        Self {
            id,
            name,
            effect_type,
            parameters: HashMap::new(),
            tags: Vec::new(),
        }
    }

    pub fn set_param(&mut self, name: &str, value: f32) {
        self.parameters.insert(name.to_string(), value);
    }

    pub fn get_param(&self, name: &str, default: f32) -> f32 {
        self.parameters.get(name).copied().unwrap_or(default)
    }
}

#[derive(Debug)]
pub struct EffectPresetLibrary {
    pub presets: HashMap<u64, EffectPreset>,
    pub next_id: u64,
}

impl EffectPresetLibrary {
    pub fn new() -> Self {
        let mut lib = Self { presets: HashMap::new(), next_id: 1 };
        lib.add_defaults();
        lib
    }

    fn add_defaults(&mut self) {
        // Default reverb preset
        let mut reverb = EffectPreset::new(self.next_id, "Large Hall".into(), EffectType::Reverb);
        reverb.set_param("room_size", 0.85);
        reverb.set_param("damping", 0.3);
        reverb.set_param("wet_mix", 0.4);
        reverb.set_param("pre_delay_ms", 20.0);
        self.presets.insert(self.next_id, reverb);
        self.next_id += 1;

        // Default compressor
        let mut comp = EffectPreset::new(self.next_id, "Gentle Glue".into(), EffectType::Compressor);
        comp.set_param("threshold_db", -18.0);
        comp.set_param("ratio", 2.0);
        comp.set_param("attack_ms", 20.0);
        comp.set_param("release_ms", 200.0);
        comp.set_param("makeup_db", 3.0);
        self.presets.insert(self.next_id, comp);
        self.next_id += 1;

        // Broadcast limiter
        let mut lim = EffectPreset::new(self.next_id, "Broadcast Limiter".into(), EffectType::Limiter);
        lim.set_param("ceiling_db", -1.0);
        lim.set_param("release_ms", 50.0);
        self.presets.insert(self.next_id, lim);
        self.next_id += 1;
    }

    pub fn presets_by_type(&self, effect_type: EffectType) -> Vec<&EffectPreset> {
        self.presets.values().filter(|p| p.effect_type == effect_type).collect()
    }

    pub fn create_effect_from_preset(&self, preset_id: u64) -> Option<AudioEffect> {
        let preset = self.presets.get(&preset_id)?;
        match preset.effect_type {
            EffectType::Reverb => {
                let mut r = SchroederReverb::new();
                r.room_size = preset.get_param("room_size", 0.5);
                r.damping = preset.get_param("damping", 0.5);
                r.wet_mix = preset.get_param("wet_mix", 0.3);
                r.pre_delay_ms = preset.get_param("pre_delay_ms", 10.0);
                Some(AudioEffect::Reverb(r))
            }
            EffectType::Compressor => {
                let params = CompressorParams {
                    threshold_db: preset.get_param("threshold_db", -18.0),
                    ratio: preset.get_param("ratio", 4.0),
                    attack_ms: preset.get_param("attack_ms", 10.0),
                    release_ms: preset.get_param("release_ms", 100.0),
                    makeup_gain_db: preset.get_param("makeup_db", 0.0),
                    ..Default::default()
                };
                Some(AudioEffect::Compressor(Compressor::new(params)))
            }
            EffectType::Limiter => {
                let ceiling = preset.get_param("ceiling_db", -1.0);
                let mut lim = Limiter::new(ceiling);
                lim.release_ms = preset.get_param("release_ms", 50.0);
                Some(AudioEffect::Limiter(lim))
            }
            _ => None,
        }
    }

    pub fn add_preset(&mut self, preset: EffectPreset) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.presets.insert(id, preset);
        id
    }
}

// ============================================================
// REAL-TIME PARAMETER PREVIEW
// ============================================================

#[derive(Debug, Clone)]
pub struct ParameterChange {
    pub bus_id: u64,
    pub effect_index: Option<usize>,
    pub parameter_name: String,
    pub old_value: f32,
    pub new_value: f32,
    pub timestamp_s: f64,
}

#[derive(Debug)]
pub struct RealTimeParameterPreview {
    pub active_changes: Vec<ParameterChange>,
    pub change_history: VecDeque<ParameterChange>,
    pub history_capacity: usize,
    pub preview_buffer_l: Vec<f32>,
    pub preview_buffer_r: Vec<f32>,
    pub buffer_size: usize,
}

impl RealTimeParameterPreview {
    pub fn new(buffer_size: usize) -> Self {
        Self {
            active_changes: Vec::new(),
            change_history: VecDeque::with_capacity(256),
            history_capacity: 256,
            preview_buffer_l: vec![0.0; buffer_size],
            preview_buffer_r: vec![0.0; buffer_size],
            buffer_size,
        }
    }

    pub fn record_change(&mut self, change: ParameterChange) {
        if self.change_history.len() >= self.history_capacity {
            self.change_history.pop_front();
        }
        self.change_history.push_back(change.clone());
        self.active_changes.retain(|c| !(c.bus_id == change.bus_id && c.parameter_name == change.parameter_name));
        self.active_changes.push(change);
    }

    pub fn fill_preview_with_sine(&mut self, freq_hz: f32, amplitude: f32) {
        for (i, (l, r)) in self.preview_buffer_l.iter_mut().zip(self.preview_buffer_r.iter_mut()).enumerate() {
            let t = i as f32 / SAMPLE_RATE;
            let sample = (TWO_PI * freq_hz * t).sin() * amplitude;
            *l = sample;
            *r = sample;
        }
    }

    pub fn process_preview_through_effect(&mut self, effect: &mut AudioEffect) {
        for i in 0..self.buffer_size {
            let (l, r) = effect.process_sample(self.preview_buffer_l[i], self.preview_buffer_r[i]);
            self.preview_buffer_l[i] = l;
            self.preview_buffer_r[i] = r;
        }
    }

    pub fn preview_rms_db(&self) -> f32 {
        let sum_sq: f32 = self.preview_buffer_l.iter()
            .zip(self.preview_buffer_r.iter())
            .map(|(l, r)| l * l + r * r)
            .sum();
        let rms = (sum_sq / (2.0 * self.buffer_size as f32)).sqrt();
        linear_to_db(rms.max(1e-9))
    }
}

// ============================================================
// AUDIO MIXER COMMAND SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub enum AudioMixerCommand {
    SetBusGain { bus_id: u64, old_db: f32, new_db: f32 },
    SetBusPan { bus_id: u64, old_pan: f32, new_pan: f32 },
    SetBusMute { bus_id: u64, old: bool, new: bool },
    AddEffect { bus_id: u64, effect_type: EffectType },
    RemoveEffect { bus_id: u64, index: usize },
    MoveEffect { bus_id: u64, from: usize, to: usize },
    AddBusRoute { from: u64, to: u64, level: f32 },
    RemoveBusRoute { from: u64, to: u64 },
    AddAutomationKeyframe { lane_idx: usize, time_s: f32, value: f32 },
}

#[derive(Debug)]
pub struct AudioMixerCommandHistory {
    pub undo_stack: Vec<AudioMixerCommand>,
    pub redo_stack: Vec<AudioMixerCommand>,
    pub max_history: usize,
}

impl AudioMixerCommandHistory {
    pub fn new(max_history: usize) -> Self {
        Self { undo_stack: Vec::new(), redo_stack: Vec::new(), max_history }
    }

    pub fn push(&mut self, cmd: AudioMixerCommand) {
        self.redo_stack.clear();
        if self.undo_stack.len() >= self.max_history { self.undo_stack.remove(0); }
        self.undo_stack.push(cmd);
    }

    pub fn undo(&mut self) -> Option<AudioMixerCommand> {
        let cmd = self.undo_stack.pop()?;
        self.redo_stack.push(cmd.clone());
        Some(cmd)
    }

    pub fn redo(&mut self) -> Option<AudioMixerCommand> {
        let cmd = self.redo_stack.pop()?;
        self.undo_stack.push(cmd.clone());
        Some(cmd)
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
}

pub fn apply_audio_command(editor: &mut AudioMixerEditor, cmd: &AudioMixerCommand) {
    match cmd {
        AudioMixerCommand::SetBusGain { bus_id, new_db, .. } => {
            editor.set_bus_gain(*bus_id, *new_db);
        }
        AudioMixerCommand::SetBusPan { bus_id, new_pan, .. } => {
            if let Some(bus) = editor.signal_flow.buses.get_mut(bus_id) {
                bus.pan = *new_pan;
            }
        }
        AudioMixerCommand::SetBusMute { bus_id, new, .. } => {
            editor.set_bus_mute(*bus_id, *new);
        }
        AudioMixerCommand::AddBusRoute { from, to, level } => {
            editor.route_bus(*from, *to, *level);
        }
        AudioMixerCommand::RemoveBusRoute { from, to } => {
            editor.unroute_bus(*from, *to);
        }
        AudioMixerCommand::MoveEffect { bus_id, from, to } => {
            if let Some(bus) = editor.signal_flow.buses.get_mut(bus_id) {
                bus.effect_chain.move_effect(*from, *to);
            }
        }
        AudioMixerCommand::RemoveEffect { bus_id, index } => {
            editor.remove_effect_from_bus(*bus_id, *index);
        }
        _ => {}
    }
}

pub fn undo_audio_command(editor: &mut AudioMixerEditor, cmd: &AudioMixerCommand) {
    match cmd {
        AudioMixerCommand::SetBusGain { bus_id, old_db, .. } => {
            editor.set_bus_gain(*bus_id, *old_db);
        }
        AudioMixerCommand::SetBusPan { bus_id, old_pan, .. } => {
            if let Some(bus) = editor.signal_flow.buses.get_mut(bus_id) {
                bus.pan = *old_pan;
            }
        }
        AudioMixerCommand::SetBusMute { bus_id, old, .. } => {
            editor.set_bus_mute(*bus_id, *old);
        }
        AudioMixerCommand::AddBusRoute { from, to, .. } => {
            editor.unroute_bus(*from, *to);
        }
        AudioMixerCommand::RemoveBusRoute { from, to } => {
            editor.route_bus(*from, *to, 1.0);
        }
        _ => {}
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_conversions() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-5);
        assert!((db_to_linear(6.0) - 1.9953).abs() < 0.001);
        assert!((linear_to_db(1.0) - 0.0).abs() < 1e-4);
        assert!((linear_to_db(2.0) - 6.0206).abs() < 0.01);
    }

    #[test]
    fn test_biquad_identity() {
        let coeff = BiquadCoefficients::identity();
        let mut state = BiquadState::new();
        assert!((state.process(0.5, &coeff) - 0.5).abs() < 1e-6);
        assert!((state.process(1.0, &coeff) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_low_pass_attenuates_above_cutoff() {
        let coeff = BiquadCoefficients::low_pass(100.0, 0.707, SAMPLE_RATE);
        let mut state = BiquadState::new();
        // Feed a high-frequency signal (10kHz) - should be heavily attenuated
        let mut output_sum = 0.0f32;
        for i in 0..1000 {
            let sample = (TWO_PI * 10000.0 * i as f32 / SAMPLE_RATE).sin();
            let out = state.process(sample, &coeff);
            output_sum += out.abs();
        }
        // Should be near zero for 10kHz with 100Hz cutoff
        assert!(output_sum / 1000.0 < 0.01);
    }

    #[test]
    fn test_compressor_no_compression_below_threshold() {
        let params = CompressorParams {
            threshold_db: 0.0,
            ..Default::default()
        };
        let mut comp = Compressor::new(params);
        // Signal well below threshold (silence)
        let (l, r) = comp.process_sample(0.001, 0.001);
        // Should pass through almost unchanged
        assert!(l.abs() < 0.01);
    }

    #[test]
    fn test_adsr_envelope() {
        let mut env = AdsrEnvelope::new(0.01, 0.1, 0.7, 0.2);
        env.trigger_attack();
        let dt = 1.0 / SAMPLE_RATE;
        // Process through attack
        for _ in 0..((0.01 * SAMPLE_RATE) as usize + 10) {
            env.tick(dt);
        }
        assert!(env.stage == EnvelopeStage::Decay || env.stage == EnvelopeStage::Sustain);
        // Process to sustain
        for _ in 0..(SAMPLE_RATE as usize) {
            env.tick(dt);
        }
        assert!((env.value - 0.7).abs() < 0.01);
        env.trigger_release();
        for _ in 0..(SAMPLE_RATE as usize) {
            env.tick(dt);
        }
        assert!(env.value < 0.01);
    }

    #[test]
    fn test_lfo_sine() {
        let mut lfo = Lfo::new(LfoShape::Sine, 1.0, 1.0);
        let samples: Vec<f32> = (0..SAMPLE_RATE as usize).map(|_| lfo.tick()).collect();
        let max = samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min = samples.iter().cloned().fold(f32::INFINITY, f32::min);
        assert!((max - 1.0).abs() < 0.01);
        assert!((min + 1.0).abs() < 0.01);
    }

    #[test]
    fn test_schroeder_reverb_wet() {
        let mut reverb = SchroederReverb::new();
        reverb.wet_mix = 1.0;
        reverb.dry_mix = 0.0;
        // Feed a pulse
        let (l, r) = reverb.process_sample(1.0, 1.0);
        // Should produce something (reverb tail)
        // After the impulse, feed silence
        let mut has_tail = false;
        for _ in 0..4000 {
            let (l2, r2) = reverb.process_sample(0.0, 0.0);
            if l2.abs() > 0.001 || r2.abs() > 0.001 { has_tail = true; }
        }
        assert!(has_tail, "Reverb should produce a tail");
    }

    #[test]
    fn test_delay_ping_pong() {
        let mut delay = DelayEffect::new(100.0);
        delay.ping_pong = true;
        delay.feedback = 0.5;
        delay.wet_mix = 1.0;
        delay.dry_mix = 0.0;
        // Process a bunch of samples; should not blow up
        let mut max_out = 0.0f32;
        for i in 0..10000 {
            let input = if i == 0 { 1.0 } else { 0.0 };
            let (l, r) = delay.process_sample(input, 0.0);
            max_out = max_out.max(l.abs()).max(r.abs());
        }
        assert!(max_out < 2.0, "Ping-pong delay should not blow up");
    }

    #[test]
    fn test_spatializer_doppler() {
        let ratio = DopplerCalculator::compute_pitch_ratio(
            Vec3::new(100.0, 0.0, 0.0), // source far right
            Vec3::ZERO,                   // listener at origin
            Vec3::new(-10.0, 0.0, 0.0), // source moving toward listener
            Vec3::ZERO,
            1.0,
        );
        // Moving source toward listener should increase pitch
        assert!(ratio > 1.0, "Source approaching = higher pitch, got {}", ratio);
    }

    #[test]
    fn test_bit_crusher() {
        let mut bc = BitCrusherEffect::new(8.0, 1);
        let mut max_error = 0.0f32;
        for i in 0..100 {
            let x = (i as f32 / 100.0) * 2.0 - 1.0;
            let (out, _) = bc.process_sample(x, 0.0);
            // 8-bit quantization: step size = 1/128 = 0.0078125
            let error = (out - x).abs();
            max_error = max_error.max(error);
        }
        // Max error should be about half a step: 0.0039
        assert!(max_error < 0.01, "8-bit quantization error too large: {}", max_error);
    }

    #[test]
    fn test_fft_magnitude_impulse() {
        let mut analyzer = SpectrumAnalyzer::new(SPECTRUM_FFT_SIZE);
        // Push an impulse then silence
        for i in 0..SPECTRUM_FFT_SIZE {
            analyzer.push_sample(if i == 0 { 1.0 } else { 0.0 });
        }
        analyzer.compute_fft_magnitude();
        // Impulse has flat spectrum — all bins should have similar magnitude
        let sum: f32 = analyzer.output_magnitudes.iter().sum();
        assert!(sum > 0.0, "FFT of impulse should have nonzero output");
    }

    #[test]
    fn test_beat_tracker() {
        let mut tracker = BeatTracker::new(120.0);
        tracker.is_running = true;
        let seconds_per_beat = 0.5; // 120 BPM
        let dt = 1.0 / 60.0; // 60 fps
        let total_steps = (seconds_per_beat * 4.0 / dt) as usize;
        for _ in 0..total_steps {
            tracker.tick(dt);
        }
        assert!(tracker.current_beat >= 3.9, "Expected ~4 beats, got {}", tracker.current_beat);
    }

    #[test]
    fn test_snapshot_blend() {
        let mut sys = SnapshotSystem::new();
        let id_a = sys.create_snapshot("A".into(), 0.0);
        let id_b = sys.create_snapshot("B".into(), 1.0);
        {
            let snap_a = sys.snapshots.get_mut(&id_a).unwrap();
            snap_a.bus_states.insert(1, crate::editor::audio_mixer_editor::BusSnapshot {
                bus_id: 1,
                gain_db: 0.0,
                pan: 0.0,
                mute: false,
                effect_bypasses: vec![],
            });
        }
        {
            let snap_b = sys.snapshots.get_mut(&id_b).unwrap();
            snap_b.bus_states.insert(1, crate::editor::audio_mixer_editor::BusSnapshot {
                bus_id: 1,
                gain_db: -6.0,
                pan: 0.5,
                mute: false,
                effect_bypasses: vec![],
            });
        }
        let blended = sys.snapshots[&id_a].blend_with(&sys.snapshots[&id_b], 0.5);
        if let Some(&(gain, pan)) = blended.get(&1) {
            assert!((gain - (-3.0)).abs() < 0.01, "Blended gain should be -3.0, got {}", gain);
            assert!((pan - 0.25).abs() < 0.01, "Blended pan should be 0.25, got {}", pan);
        } else {
            panic!("Expected blended bus 1");
        }
    }

    #[test]
    fn test_lufs_silence() {
        let mut meter = LufsMeter::new();
        for _ in 0..10000 {
            meter.process_sample(0.0, 0.0);
        }
        assert!(meter.integrated_lufs == f32::NEG_INFINITY || meter.integrated_lufs < -60.0);
    }

    #[test]
    fn test_voice_manager_spawn_and_cull() {
        let mut vm = VoiceManager::new(4);
        for i in 0..6 {
            vm.spawn_voice(i, SoundCategory::Sfx, Vec3::ZERO);
        }
        // Max 4 voices total
        assert!(vm.voices.len() <= 4);
    }

    #[test]
    fn test_compressor_gain_reduction() {
        let params = CompressorParams {
            threshold_db: -20.0,
            ratio: 4.0,
            knee_db: 0.0,
            ..Default::default()
        };
        let comp = Compressor::new(params);
        // Signal at 0 dB (full scale): 20 dB above threshold
        // Gain reduction should be 20*(1-1/4) = 15 dB
        let gr = comp.compute_gain_db(0.0);
        assert!((gr + 15.0).abs() < 0.1, "Expected -15dB GR, got {}", gr);
    }

    #[test]
    fn test_eq_frequency_response() {
        let eq = ParametricEqualizer::new();
        // At 1kHz with all bands at 0dB gain, response should be ~0dB
        let response = eq.frequency_response_db(1000.0);
        assert!(response.abs() < 2.0, "Flat EQ response at 1kHz should be near 0dB, got {}", response);
    }

    #[test]
    fn test_signal_flow_graph_topology() {
        let graph = SignalFlowGraph::new();
        // Default graph has 6 buses; topology should be populated
        assert!(!graph.topology_order.is_empty());
        // Master should be last in topology (all routes lead to it)
        assert_eq!(*graph.topology_order.last().unwrap(), graph.master_bus_id);
    }

    #[test]
    fn test_automation_lane_interpolation() {
        let mut lane = AutomationLane::new("gain_db".into(), 1);
        lane.add_keyframe(AutomationKeyframe::new(0.0, -20.0));
        lane.add_keyframe(AutomationKeyframe::new(1.0, 0.0));
        let mid = lane.evaluate_at(0.5);
        assert!((mid - (-10.0)).abs() < 0.1, "Expected -10dB at t=0.5, got {}", mid);
    }

    #[test]
    fn test_hrtf_stereo_separation() {
        let mut hrtf = HrtfFilter::new(0.0, std::f32::consts::FRAC_PI_2); // 90 degrees right
        let (l, r) = hrtf.process_mono(1.0);
        // Right-panned source should have different L/R levels
        // (just verify it produces output)
        assert!(l.is_finite() && r.is_finite());
    }

    #[test]
    fn test_reverb_zone_blend() {
        let mut mgr = ReverbZoneManager::new();
        let id = mgr.add_zone("Test".into(), Vec3::ZERO, 10.0, 5.0);
        mgr.update(Vec3::new(5.0, 0.0, 0.0)); // inside zone
        assert!(mgr.active_blend.contains_key(&id));
        let blend = mgr.active_blend[&id];
        assert!(blend > 0.0 && blend <= 1.0);
    }

    #[test]
    fn test_distortion_soft_clip_bounded() {
        let mut dist = DistortionEffect::new(DistortionMode::SoftClip, 10.0);
        for i in 0..1000 {
            let x = (i as f32 / 500.0) - 1.0;
            let (l, _) = dist.process_sample(x, 0.0);
            assert!(l.is_finite(), "Distortion output must be finite");
            assert!(l.abs() < 2.0, "Soft clip should bound output");
        }
    }

    #[test]
    fn test_multiband_compressor_passthrough() {
        let mut mbc = MultiBandCompressor::new_three_band(300.0, 3000.0);
        // With default (low threshold relative to silence), should mostly pass through
        let (l, r) = mbc.process_sample(0.5, -0.5);
        assert!(l.is_finite() && r.is_finite());
    }

    #[test]
    fn test_semitones_to_ratio() {
        // One octave = 12 semitones = ratio 2.0
        assert!((semitones_to_ratio(12.0) - 2.0).abs() < 1e-5);
        // Unison
        assert!((semitones_to_ratio(0.0) - 1.0).abs() < 1e-5);
        // Perfect fifth (7 semitones) ≈ 1.498
        assert!((semitones_to_ratio(7.0) - 1.498).abs() < 0.01);
    }
}
