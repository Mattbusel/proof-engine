//! Digital Signal Processing — module root
//!
//! Provides signal buffers, window functions, envelopes, peak metering,
//! signal generation, and re-exports the FFT, filter, and analysis sub-modules.

pub mod fft;
pub mod filters;
pub mod analysis;

pub use fft::{
    Complex32, Fft, RealFft, FftPlanner, FftPlan, Spectrum, Stft, StftConfig,
    Cqt, Autocorrelation, MelFilterbank, Mfcc, Chroma,
};
pub use filters::{
    Biquad, BiquadType, BiquadDesign, FilterChain, Butterworth, Chebyshev1, Bessel,
    FirFilter, FirDesign, Convolution, OlaConvolver,
    SvfFilter, SvfMode, CombFilter, CombMode, AllpassDelay,
    MovingAverage, KalmanFilter1D, PllFilter,
};
pub use analysis::{
    OnsetDetector, SpectralFluxOnset, HfcOnset, ComplexDomainOnset,
    BeatTracker, PitchDetector, LoudnessMeters, Rms, Leq, Lufs, DynamicRange,
    TransientAnalysis, HarmonicAnalyzer, Correlogram, DynamicsAnalyzer, SignalSimilarity,
};

use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Signal<T>
// ---------------------------------------------------------------------------

/// An owned time-series buffer with sample-rate metadata.
#[derive(Debug, Clone)]
pub struct Signal<T: Clone> {
    pub samples: Vec<T>,
    pub sample_rate: f32,
}

impl<T: Clone + Default> Signal<T> {
    /// Create a new signal from a sample buffer and a sample rate.
    pub fn new(samples: Vec<T>, sample_rate: f32) -> Self {
        assert!(sample_rate > 0.0, "sample_rate must be positive");
        Self { samples, sample_rate }
    }

    /// Duration of the signal in seconds.
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Extract a sub-window starting at `start` with length `len`.
    /// Clamps at the end of the buffer.
    pub fn window(&self, start: usize, len: usize) -> Signal<T> {
        let end = (start + len).min(self.samples.len());
        let start = start.min(end);
        Signal {
            samples: self.samples[start..end].to_vec(),
            sample_rate: self.sample_rate,
        }
    }
}

impl Signal<f32> {
    /// Downsample by integer factor (simple decimation, no anti-alias filter).
    pub fn downsample(&self, factor: usize) -> Signal<f32> {
        assert!(factor >= 1);
        let samples: Vec<f32> = self.samples.iter().step_by(factor).copied().collect();
        Signal {
            samples,
            sample_rate: self.sample_rate / factor as f32,
        }
    }

    /// Upsample by integer factor (zero insertion).
    pub fn upsample(&self, factor: usize) -> Signal<f32> {
        assert!(factor >= 1);
        let mut samples = vec![0.0f32; self.samples.len() * factor];
        for (i, &s) in self.samples.iter().enumerate() {
            samples[i * factor] = s;
        }
        Signal {
            samples,
            sample_rate: self.sample_rate * factor as f32,
        }
    }

    /// Resample to a new sample rate using linear interpolation.
    pub fn resample(&self, new_rate: f32) -> Signal<f32> {
        assert!(new_rate > 0.0);
        if (new_rate - self.sample_rate).abs() < 1e-3 {
            return self.clone();
        }
        let ratio = self.sample_rate / new_rate;
        let new_len = (self.samples.len() as f32 / ratio).round() as usize;
        let mut out = Vec::with_capacity(new_len);
        for i in 0..new_len {
            let pos = i as f32 * ratio;
            let idx = pos as usize;
            let frac = pos - idx as f32;
            let a = self.samples.get(idx).copied().unwrap_or(0.0);
            let b = self.samples.get(idx + 1).copied().unwrap_or(a);
            out.push(a + frac * (b - a));
        }
        Signal { samples: out, sample_rate: new_rate }
    }

    /// Root-mean-square amplitude.
    pub fn rms(&self) -> f32 {
        if self.samples.is_empty() { return 0.0; }
        let sum_sq: f32 = self.samples.iter().map(|&x| x * x).sum();
        (sum_sq / self.samples.len() as f32).sqrt()
    }

    /// Peak absolute value.
    pub fn peak(&self) -> f32 {
        self.samples.iter().map(|&x| x.abs()).fold(0.0f32, f32::max)
    }

    /// Zero-crossing rate (crossings per second).
    pub fn zero_crossing_rate(&self) -> f32 {
        if self.samples.len() < 2 { return 0.0; }
        let crossings = self.samples.windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        crossings as f32 * self.sample_rate / self.samples.len() as f32
    }

    /// Total energy (sum of squared samples).
    pub fn energy(&self) -> f32 {
        self.samples.iter().map(|&x| x * x).sum()
    }

    /// Normalize to peak amplitude of 1.0 (in place).
    pub fn normalize(&mut self) {
        let pk = self.peak();
        if pk > 1e-10 {
            for s in self.samples.iter_mut() {
                *s /= pk;
            }
        }
    }

    /// Mix another signal into this one (adds samples).
    pub fn mix_in(&mut self, other: &Signal<f32>, gain: f32) {
        let len = self.samples.len().min(other.samples.len());
        for i in 0..len {
            self.samples[i] += other.samples[i] * gain;
        }
    }
}

// ---------------------------------------------------------------------------
// ComplexSignal
// ---------------------------------------------------------------------------

/// A complex-valued signal (e.g. FFT output).
#[derive(Debug, Clone)]
pub struct ComplexSignal {
    pub samples: Vec<Complex32>,
    pub sample_rate: f32,
}

impl ComplexSignal {
    pub fn new(samples: Vec<Complex32>, sample_rate: f32) -> Self {
        Self { samples, sample_rate }
    }

    pub fn magnitude_signal(&self) -> Signal<f32> {
        Signal {
            samples: self.samples.iter().map(|c| c.norm()).collect(),
            sample_rate: self.sample_rate,
        }
    }

    pub fn phase_signal(&self) -> Signal<f32> {
        Signal {
            samples: self.samples.iter().map(|c| c.arg()).collect(),
            sample_rate: self.sample_rate,
        }
    }
}

// ---------------------------------------------------------------------------
// SignalGenerator
// ---------------------------------------------------------------------------

/// Factory for generating standard test/synthesis signals.
pub struct SignalGenerator;

impl SignalGenerator {
    /// Pure sine wave.
    pub fn sine(freq_hz: f32, amplitude: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| amplitude * (2.0 * PI * freq_hz * i as f32 / sample_rate).sin())
            .collect();
        Signal::new(samples, sample_rate)
    }

    /// Square wave via Fourier synthesis (up to `harmonics` odd harmonics).
    pub fn square(freq_hz: f32, amplitude: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        let harmonics = 50usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate;
                let mut v = 0.0f32;
                for k in 0..harmonics {
                    let h = 2 * k + 1;
                    v += (2.0 * PI * freq_hz * h as f32 * t).sin() / h as f32;
                }
                amplitude * (4.0 / PI) * v
            })
            .collect();
        Signal::new(samples, sample_rate)
    }

    /// Triangle wave.
    pub fn triangle(freq_hz: f32, amplitude: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        let period = sample_rate / freq_hz;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let phase = (i as f32 % period) / period; // 0..1
                let v = if phase < 0.5 {
                    4.0 * phase - 1.0
                } else {
                    3.0 - 4.0 * phase
                };
                amplitude * v
            })
            .collect();
        Signal::new(samples, sample_rate)
    }

    /// Sawtooth wave.
    pub fn sawtooth(freq_hz: f32, amplitude: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        let period = sample_rate / freq_hz;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let phase = (i as f32 % period) / period; // 0..1
                amplitude * (2.0 * phase - 1.0)
            })
            .collect();
        Signal::new(samples, sample_rate)
    }

    /// White noise (uniform distribution in [-amplitude, amplitude]).
    pub fn noise(amplitude: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        // Simple LCG pseudo-random noise
        let mut state: u32 = 0x12345678;
        let samples: Vec<f32> = (0..n)
            .map(|_| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                let norm = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
                amplitude * norm
            })
            .collect();
        Signal::new(samples, sample_rate)
    }

    /// Linear chirp sweeping from `f0_hz` to `f1_hz` over `duration_secs`.
    pub fn chirp(f0_hz: f32, f1_hz: f32, amplitude: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        let k = (f1_hz - f0_hz) / (2.0 * duration_secs);
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate;
                let phase = 2.0 * PI * (f0_hz * t + k * t * t);
                amplitude * phase.sin()
            })
            .collect();
        Signal::new(samples, sample_rate)
    }

    /// Rectangular pulse: high for `pulse_width_secs` then zero.
    pub fn pulse(amplitude: f32, pulse_width_secs: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        let pulse_samples = (pulse_width_secs * sample_rate) as usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| if i < pulse_samples { amplitude } else { 0.0 })
            .collect();
        Signal::new(samples, sample_rate)
    }

    /// Unit impulse (Dirac delta approximation): 1.0 at sample 0, 0.0 elsewhere.
    pub fn impulse(amplitude: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        let mut samples = vec![0.0f32; n.max(1)];
        if !samples.is_empty() {
            samples[0] = amplitude;
        }
        Signal::new(samples, sample_rate)
    }

    /// Gaussian-modulated sine (Gabor atom) for analysis.
    pub fn gabor(freq_hz: f32, sigma: f32, center_secs: f32, amplitude: f32, duration_secs: f32, sample_rate: f32) -> Signal<f32> {
        let n = (duration_secs * sample_rate) as usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate;
                let gauss = (-((t - center_secs).powi(2)) / (2.0 * sigma * sigma)).exp();
                amplitude * gauss * (2.0 * PI * freq_hz * t).sin()
            })
            .collect();
        Signal::new(samples, sample_rate)
    }
}

// ---------------------------------------------------------------------------
// WindowFunction
// ---------------------------------------------------------------------------

/// Standard window functions for spectral analysis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowFunction {
    /// Rectangular / boxcar — no windowing.
    Rectangular,
    /// Hann window (raised cosine).
    Hann,
    /// Hamming window.
    Hamming,
    /// Blackman window.
    Blackman,
    /// Kaiser window with parameter β.
    Kaiser(f32),
    /// Flat-top window for accurate amplitude measurement.
    FlatTop,
    /// Nuttall window (minimum 4-term Blackman-Harris).
    Nuttall,
    /// Gaussian window with parameter σ (fraction of half-window).
    Gaussian(f32),
    /// Bartlett (triangle) window.
    Bartlett,
    /// Welch window.
    Welch,
}

impl WindowFunction {
    /// Compute the window coefficient for sample `n` in a window of length `len`.
    pub fn coefficient(&self, n: usize, len: usize) -> f32 {
        if len <= 1 { return 1.0; }
        let n = n as f32;
        let n_minus_1 = (len - 1) as f32;
        match self {
            WindowFunction::Rectangular => 1.0,
            WindowFunction::Hann => {
                0.5 * (1.0 - (2.0 * PI * n / n_minus_1).cos())
            }
            WindowFunction::Hamming => {
                0.54 - 0.46 * (2.0 * PI * n / n_minus_1).cos()
            }
            WindowFunction::Blackman => {
                0.42 - 0.5 * (2.0 * PI * n / n_minus_1).cos()
                    + 0.08 * (4.0 * PI * n / n_minus_1).cos()
            }
            WindowFunction::Kaiser(beta) => {
                let half = n_minus_1 / 2.0;
                let arg = beta * (1.0 - ((n - half) / half).powi(2)).sqrt();
                Self::bessel_i0(arg) / Self::bessel_i0(*beta)
            }
            WindowFunction::FlatTop => {
                let a0 = 0.21557895;
                let a1 = 0.41663158;
                let a2 = 0.277263158;
                let a3 = 0.083578947;
                let a4 = 0.006947368;
                a0 - a1 * (2.0 * PI * n / n_minus_1).cos()
                    + a2 * (4.0 * PI * n / n_minus_1).cos()
                    - a3 * (6.0 * PI * n / n_minus_1).cos()
                    + a4 * (8.0 * PI * n / n_minus_1).cos()
            }
            WindowFunction::Nuttall => {
                let a0 = 0.355768;
                let a1 = 0.487396;
                let a2 = 0.144232;
                let a3 = 0.012604;
                a0 - a1 * (2.0 * PI * n / n_minus_1).cos()
                    + a2 * (4.0 * PI * n / n_minus_1).cos()
                    - a3 * (6.0 * PI * n / n_minus_1).cos()
            }
            WindowFunction::Gaussian(sigma) => {
                let half = n_minus_1 / 2.0;
                let exponent = -0.5 * ((n - half) / (sigma * half)).powi(2);
                exponent.exp()
            }
            WindowFunction::Bartlett => {
                let half = n_minus_1 / 2.0;
                1.0 - ((n - half) / half).abs()
            }
            WindowFunction::Welch => {
                let half = n_minus_1 / 2.0;
                1.0 - ((n - half) / half).powi(2)
            }
        }
    }

    /// Apply the window function in-place to a slice of samples.
    pub fn apply(&self, samples: &mut [f32]) {
        let len = samples.len();
        for (i, s) in samples.iter_mut().enumerate() {
            *s *= self.coefficient(i, len);
        }
    }

    /// Generate the full window vector.
    pub fn generate(&self, len: usize) -> Vec<f32> {
        (0..len).map(|i| self.coefficient(i, len)).collect()
    }

    /// Coherent gain (mean of window coefficients) — used for amplitude correction.
    pub fn coherent_gain(&self, len: usize) -> f32 {
        let sum: f32 = (0..len).map(|i| self.coefficient(i, len)).sum();
        sum / len as f32
    }

    /// Power gain — RMS of window coefficients.
    pub fn power_gain(&self, len: usize) -> f32 {
        let sum_sq: f32 = (0..len).map(|i| {
            let c = self.coefficient(i, len);
            c * c
        }).sum();
        (sum_sq / len as f32).sqrt()
    }

    /// Modified Bessel function of the first kind, order 0 (used for Kaiser window).
    fn bessel_i0(x: f32) -> f32 {
        // Polynomial approximation
        let ax = x.abs();
        if ax < 3.75 {
            let y = (x / 3.75).powi(2);
            1.0 + y * (3.5156229 + y * (3.0899424 + y * (1.2067492
                + y * (0.2659732 + y * (0.0360768 + y * 0.0045813)))))
        } else {
            let y = 3.75 / ax;
            (ax.exp() / ax.sqrt())
                * (0.39894228 + y * (0.01328592 + y * (0.00225319
                    + y * (-0.00157565 + y * (0.00916281
                        + y * (-0.02057706 + y * (0.02635537
                            + y * (-0.01647633 + y * 0.00392377))))))))
        }
    }
}

// ---------------------------------------------------------------------------
// Envelope follower
// ---------------------------------------------------------------------------

/// Amplitude envelope follower with attack, hold, and release stages.
#[derive(Debug, Clone)]
pub struct Envelope {
    /// Attack time constant in seconds.
    pub attack_secs: f32,
    /// Release time constant in seconds.
    pub release_secs: f32,
    /// Hold time in seconds before release begins.
    pub hold_secs: f32,
    sample_rate: f32,
    // Internal state
    level: f32,
    hold_counter: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl Envelope {
    pub fn new(attack_secs: f32, release_secs: f32, hold_secs: f32, sample_rate: f32) -> Self {
        let attack_coeff = Self::time_to_coeff(attack_secs, sample_rate);
        let release_coeff = Self::time_to_coeff(release_secs, sample_rate);
        Self {
            attack_secs,
            release_secs,
            hold_secs,
            sample_rate,
            level: 0.0,
            hold_counter: 0.0,
            attack_coeff,
            release_coeff,
        }
    }

    fn time_to_coeff(time_secs: f32, sample_rate: f32) -> f32 {
        if time_secs <= 0.0 { return 0.0; }
        (-1.0 / (time_secs * sample_rate)).exp()
    }

    /// Process one sample, return the envelope level.
    pub fn process(&mut self, input: f32) -> f32 {
        let abs_in = input.abs();
        if abs_in >= self.level {
            // Attack
            self.level = self.attack_coeff * self.level + (1.0 - self.attack_coeff) * abs_in;
            self.hold_counter = self.hold_secs * self.sample_rate;
        } else if self.hold_counter > 0.0 {
            // Hold — level stays put
            self.hold_counter -= 1.0;
        } else {
            // Release
            self.level = self.release_coeff * self.level + (1.0 - self.release_coeff) * abs_in;
        }
        self.level
    }

    /// Process a buffer, returning the envelope for each sample.
    pub fn process_buffer(&mut self, buf: &[f32]) -> Vec<f32> {
        buf.iter().map(|&x| self.process(x)).collect()
    }

    /// Reset the envelope state.
    pub fn reset(&mut self) {
        self.level = 0.0;
        self.hold_counter = 0.0;
    }

    /// Current envelope level.
    pub fn level(&self) -> f32 {
        self.level
    }
}

// ---------------------------------------------------------------------------
// PeakMeter
// ---------------------------------------------------------------------------

/// dBFS peak meter with configurable hold time and fallback rate.
#[derive(Debug, Clone)]
pub struct PeakMeter {
    /// Hold time in seconds before the peak indicator starts falling.
    pub hold_secs: f32,
    /// Fall rate in dB per second.
    pub fallback_db_per_sec: f32,
    sample_rate: f32,
    peak_linear: f32,
    hold_counter: f32,
    display_level_db: f32,
    clip: bool,
}

impl PeakMeter {
    pub fn new(hold_secs: f32, fallback_db_per_sec: f32, sample_rate: f32) -> Self {
        Self {
            hold_secs,
            fallback_db_per_sec,
            sample_rate,
            peak_linear: 0.0,
            hold_counter: 0.0,
            display_level_db: -f32::INFINITY,
            clip: false,
        }
    }

    /// Feed a block of samples. Updates the peak display.
    pub fn process(&mut self, buf: &[f32]) {
        let block_peak = buf.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        if block_peak >= 1.0 {
            self.clip = true;
        }
        if block_peak >= self.peak_linear {
            self.peak_linear = block_peak;
            self.hold_counter = self.hold_secs * self.sample_rate;
        } else if self.hold_counter > 0.0 {
            self.hold_counter -= buf.len() as f32;
        } else {
            // Fall back at fallback_db_per_sec
            let fall_db = self.fallback_db_per_sec * buf.len() as f32 / self.sample_rate;
            let current_db = linear_to_db(self.peak_linear).max(-144.0);
            let new_db = current_db - fall_db;
            self.peak_linear = db_to_linear(new_db).max(0.0);
        }
        self.display_level_db = linear_to_db(self.peak_linear);
    }

    /// Current peak level in dBFS.
    pub fn peak_db(&self) -> f32 {
        self.display_level_db
    }

    /// Returns true if the signal has clipped (exceeded 0 dBFS).
    pub fn is_clipping(&self) -> bool {
        self.clip
    }

    /// Reset clip indicator.
    pub fn reset_clip(&mut self) {
        self.clip = false;
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.peak_linear = 0.0;
        self.hold_counter = 0.0;
        self.display_level_db = -f32::INFINITY;
        self.clip = false;
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Convert linear amplitude to dBFS.
#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 { return -f32::INFINITY; }
    20.0 * linear.log10()
}

/// Convert dBFS to linear amplitude.
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

/// Next power of two >= n.
#[inline]
pub fn next_power_of_two(n: usize) -> usize {
    if n <= 1 { return 1; }
    let mut p = 1usize;
    while p < n { p <<= 1; }
    p
}

/// Check if n is a power of two.
#[inline]
pub fn is_power_of_two(n: usize) -> bool {
    n > 0 && (n & (n - 1)) == 0
}

/// Sinc function (normalized): sinc(x) = sin(π·x) / (π·x).
#[inline]
pub fn sinc(x: f32) -> f32 {
    if x.abs() < 1e-10 { return 1.0; }
    let px = PI * x;
    px.sin() / px
}

/// Frequency in Hz to MIDI note number (A4 = 69, 440 Hz).
#[inline]
pub fn freq_to_midi(freq: f32) -> f32 {
    69.0 + 12.0 * (freq / 440.0).log2()
}

/// MIDI note number to frequency in Hz.
#[inline]
pub fn midi_to_freq(midi: f32) -> f32 {
    440.0 * 2.0f32.powf((midi - 69.0) / 12.0)
}

/// Convert frequency to Mel scale.
#[inline]
pub fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

/// Convert Mel scale to frequency.
#[inline]
pub fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10.0f32.powf(mel / 2595.0) - 1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_new_and_properties() {
        let sig = Signal::new(vec![0.0f32; 44100], 44100.0);
        assert_eq!(sig.len(), 44100);
        assert!((sig.duration_secs() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_signal_window() {
        let sig = Signal::new((0..100).map(|i| i as f32).collect(), 100.0);
        let w = sig.window(10, 20);
        assert_eq!(w.len(), 20);
        assert!((w.samples[0] - 10.0).abs() < 1e-5);
    }

    #[test]
    fn test_signal_rms() {
        // RMS of a 1.0 amplitude sine over full period ≈ 1/√2
        let sig = SignalGenerator::sine(440.0, 1.0, 1.0, 44100.0);
        let rms = sig.rms();
        assert!((rms - (1.0f32 / 2.0f32.sqrt())).abs() < 0.01, "rms={}", rms);
    }

    #[test]
    fn test_signal_peak() {
        let sig = SignalGenerator::sine(440.0, 0.5, 0.1, 44100.0);
        let pk = sig.peak();
        assert!(pk <= 0.5 + 1e-4);
        assert!(pk > 0.4);
    }

    #[test]
    fn test_signal_energy() {
        let sig = Signal::new(vec![1.0f32; 100], 100.0);
        assert!((sig.energy() - 100.0).abs() < 1e-5);
    }

    #[test]
    fn test_signal_downsample() {
        let sig = Signal::new(vec![1.0f32; 100], 100.0);
        let ds = sig.downsample(2);
        assert_eq!(ds.len(), 50);
        assert!((ds.sample_rate - 50.0).abs() < 1e-5);
    }

    #[test]
    fn test_signal_upsample() {
        let sig = Signal::new(vec![1.0f32; 10], 10.0);
        let us = sig.upsample(3);
        assert_eq!(us.len(), 30);
        assert!((us.sample_rate - 30.0).abs() < 1e-5);
    }

    #[test]
    fn test_signal_resample() {
        let sig = SignalGenerator::sine(440.0, 1.0, 1.0, 44100.0);
        let resampled = sig.resample(22050.0);
        assert_eq!(resampled.len(), 22050);
        assert!((resampled.sample_rate - 22050.0).abs() < 1e-3);
    }

    #[test]
    fn test_window_hann_endpoints() {
        let w = WindowFunction::Hann;
        // Hann window starts and ends at (approximately) 0
        assert!((w.coefficient(0, 1024)).abs() < 1e-5);
    }

    #[test]
    fn test_window_rectangular() {
        let w = WindowFunction::Rectangular;
        for i in 0..64 {
            assert!((w.coefficient(i, 64) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_window_apply() {
        let mut buf = vec![1.0f32; 64];
        WindowFunction::Hann.apply(&mut buf);
        // Middle sample should be near 1.0
        let mid = buf[32];
        assert!(mid > 0.9, "mid={}", mid);
    }

    #[test]
    fn test_envelope() {
        let mut env = Envelope::new(0.001, 0.1, 0.0, 44100.0);
        env.process(1.0);
        assert!(env.level() > 0.0);
        // After silence, should decay
        for _ in 0..10000 {
            env.process(0.0);
        }
        assert!(env.level() < 0.5);
    }

    #[test]
    fn test_peak_meter() {
        let mut meter = PeakMeter::new(1.0, 60.0, 44100.0);
        let buf: Vec<f32> = vec![0.5; 512];
        meter.process(&buf);
        let db = meter.peak_db();
        assert!(db > -7.0 && db < -5.0, "db={}", db);
    }

    #[test]
    fn test_db_conversions() {
        assert!((linear_to_db(1.0) - 0.0).abs() < 1e-5);
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-5);
        assert!((linear_to_db(0.5) - (-6.0206)).abs() < 0.01);
    }

    #[test]
    fn test_next_power_of_two() {
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(5), 8);
        assert_eq!(next_power_of_two(8), 8);
        assert_eq!(next_power_of_two(1000), 1024);
    }

    #[test]
    fn test_midi_freq_roundtrip() {
        let midi = 69.0;
        let freq = midi_to_freq(midi);
        assert!((freq - 440.0).abs() < 0.01);
        let back = freq_to_midi(freq);
        assert!((back - midi).abs() < 0.01);
    }

    #[test]
    fn test_signal_generator_chirp() {
        let sig = SignalGenerator::chirp(20.0, 2000.0, 1.0, 1.0, 44100.0);
        assert_eq!(sig.len(), 44100);
        let pk = sig.peak();
        assert!(pk > 0.9 && pk <= 1.0 + 1e-4);
    }

    #[test]
    fn test_signal_generator_impulse() {
        let sig = SignalGenerator::impulse(1.0, 0.1, 44100.0);
        assert_eq!(sig.samples[0], 1.0);
        assert_eq!(sig.samples[1], 0.0);
    }

    #[test]
    fn test_zero_crossing_rate() {
        let sig = SignalGenerator::sine(440.0, 1.0, 1.0, 44100.0);
        let zcr = sig.zero_crossing_rate();
        // Expected: ~880 crossings/sec (2 per cycle)
        assert!(zcr > 800.0 && zcr < 1000.0, "zcr={}", zcr);
    }
}
