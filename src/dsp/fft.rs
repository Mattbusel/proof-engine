//! FFT library — Cooley-Tukey radix-2, real FFT, spectral analysis, STFT,
//! CQT, autocorrelation, YIN pitch, Mel filterbank, MFCC, and Chroma.

use std::collections::HashMap;
use std::f32::consts::PI;
use super::{WindowFunction, next_power_of_two};

// ---------------------------------------------------------------------------
// Complex32
// ---------------------------------------------------------------------------

/// 32-bit complex number with full arithmetic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex32 {
    pub re: f32,
    pub im: f32,
}

impl Complex32 {
    #[inline] pub fn new(re: f32, im: f32) -> Self { Self { re, im } }
    #[inline] pub fn zero() -> Self { Self { re: 0.0, im: 0.0 } }
    #[inline] pub fn one()  -> Self { Self { re: 1.0, im: 0.0 } }
    #[inline] pub fn i()    -> Self { Self { re: 0.0, im: 1.0 } }

    /// Modulus (magnitude).
    #[inline] pub fn norm(&self) -> f32 { (self.re * self.re + self.im * self.im).sqrt() }
    /// Squared modulus.
    #[inline] pub fn norm_sq(&self) -> f32 { self.re * self.re + self.im * self.im }
    /// Argument (phase angle) in radians.
    #[inline] pub fn arg(&self) -> f32 { self.im.atan2(self.re) }
    /// Complex conjugate.
    #[inline] pub fn conj(&self) -> Self { Self { re: self.re, im: -self.im } }

    /// Euler's formula: e^(iθ) = cos θ + i sin θ.
    #[inline] pub fn from_polar(r: f32, theta: f32) -> Self {
        Self { re: r * theta.cos(), im: r * theta.sin() }
    }

    /// Natural logarithm of a complex number.
    pub fn ln(&self) -> Self {
        Self { re: self.norm().ln(), im: self.arg() }
    }

    /// Complex exponentiation.
    pub fn exp(&self) -> Self {
        let e_re = self.re.exp();
        Self { re: e_re * self.im.cos(), im: e_re * self.im.sin() }
    }

    /// Complex power z^n (integer exponent).
    pub fn powi(&self, n: i32) -> Self {
        if n == 0 { return Self::one(); }
        let r = self.norm().powi(n);
        let theta = self.arg() * n as f32;
        Self::from_polar(r, theta)
    }
}

impl std::ops::Add for Complex32 {
    type Output = Self;
    #[inline] fn add(self, rhs: Self) -> Self { Self { re: self.re + rhs.re, im: self.im + rhs.im } }
}
impl std::ops::AddAssign for Complex32 {
    #[inline] fn add_assign(&mut self, rhs: Self) { self.re += rhs.re; self.im += rhs.im; }
}
impl std::ops::Sub for Complex32 {
    type Output = Self;
    #[inline] fn sub(self, rhs: Self) -> Self { Self { re: self.re - rhs.re, im: self.im - rhs.im } }
}
impl std::ops::SubAssign for Complex32 {
    #[inline] fn sub_assign(&mut self, rhs: Self) { self.re -= rhs.re; self.im -= rhs.im; }
}
impl std::ops::Mul for Complex32 {
    type Output = Self;
    #[inline] fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}
impl std::ops::MulAssign for Complex32 {
    #[inline] fn mul_assign(&mut self, rhs: Self) { *self = *self * rhs; }
}
impl std::ops::Div for Complex32 {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let denom = rhs.norm_sq();
        Self {
            re: (self.re * rhs.re + self.im * rhs.im) / denom,
            im: (self.im * rhs.re - self.re * rhs.im) / denom,
        }
    }
}
impl std::ops::Neg for Complex32 {
    type Output = Self;
    #[inline] fn neg(self) -> Self { Self { re: -self.re, im: -self.im } }
}
impl std::ops::Mul<f32> for Complex32 {
    type Output = Self;
    #[inline] fn mul(self, rhs: f32) -> Self { Self { re: self.re * rhs, im: self.im * rhs } }
}
impl std::ops::Div<f32> for Complex32 {
    type Output = Self;
    #[inline] fn div(self, rhs: f32) -> Self { Self { re: self.re / rhs, im: self.im / rhs } }
}

// ---------------------------------------------------------------------------
// Fft — Cooley-Tukey in-place radix-2 DIT
// ---------------------------------------------------------------------------

/// Cooley-Tukey radix-2 in-place FFT for power-of-2 sizes.
pub struct Fft;

impl Fft {
    /// In-place DIT FFT. `data.len()` must be a power of 2.
    pub fn forward(data: &mut [Complex32]) {
        let n = data.len();
        debug_assert!(super::is_power_of_two(n), "FFT size must be a power of 2");
        Self::bit_reverse_permute(data);
        let mut len = 2usize;
        while len <= n {
            let half = len / 2;
            let w_step = Complex32::from_polar(1.0, -PI / half as f32);
            for chunk_start in (0..n).step_by(len) {
                let mut w = Complex32::one();
                for k in 0..half {
                    let u = data[chunk_start + k];
                    let v = data[chunk_start + k + half] * w;
                    data[chunk_start + k]        = u + v;
                    data[chunk_start + k + half] = u - v;
                    w *= w_step;
                }
            }
            len <<= 1;
        }
    }

    /// In-place inverse FFT with 1/N scaling.
    pub fn inverse(data: &mut [Complex32]) {
        let n = data.len();
        // Conjugate, forward FFT, conjugate, scale
        for x in data.iter_mut() { *x = x.conj(); }
        Self::forward(data);
        let scale = 1.0 / n as f32;
        for x in data.iter_mut() { *x = x.conj() * scale; }
    }

    /// Bit-reversal permutation.
    fn bit_reverse_permute(data: &mut [Complex32]) {
        let n = data.len();
        let bits = n.trailing_zeros() as usize;
        for i in 0..n {
            let j = Self::reverse_bits(i, bits);
            if i < j {
                data.swap(i, j);
            }
        }
    }

    fn reverse_bits(mut x: usize, bits: usize) -> usize {
        let mut result = 0usize;
        for _ in 0..bits {
            result = (result << 1) | (x & 1);
            x >>= 1;
        }
        result
    }
}

// ---------------------------------------------------------------------------
// RealFft
// ---------------------------------------------------------------------------

/// Optimized FFT for real-valued input using the half-spectrum trick.
pub struct RealFft;

impl RealFft {
    /// Forward FFT of a real signal. Returns the N/2+1 complex bins.
    pub fn forward_real(data: &[f32]) -> Vec<Complex32> {
        let n = next_power_of_two(data.len());
        let mut buf: Vec<Complex32> = data.iter()
            .map(|&x| Complex32::new(x, 0.0))
            .collect();
        buf.resize(n, Complex32::zero());
        Fft::forward(&mut buf);
        // Return only the first half + DC + Nyquist
        buf.truncate(n / 2 + 1);
        buf
    }

    /// Inverse FFT from half-spectrum back to real signal of length `n`.
    pub fn inverse_real(spectrum: &[Complex32], n: usize) -> Vec<f32> {
        let n_fft = next_power_of_two(n);
        let mut buf = Vec::with_capacity(n_fft);
        buf.extend_from_slice(spectrum);
        // Reconstruct conjugate-symmetric upper half
        let half = n_fft / 2;
        for k in 1..half {
            buf.push(buf[half - k].conj());
        }
        buf.resize(n_fft, Complex32::zero());
        Fft::inverse(&mut buf);
        buf[..n].iter().map(|c| c.re).collect()
    }
}

// ---------------------------------------------------------------------------
// FftPlanner
// ---------------------------------------------------------------------------

/// A plan produced by `FftPlanner` for a specific transform size.
pub struct FftPlan {
    /// The FFT size.
    pub size: usize,
    /// Pre-computed twiddle factors w_n^k for k = 0..size/2.
    pub twiddles: Vec<Complex32>,
}

impl FftPlan {
    /// Execute the forward FFT using pre-computed twiddles.
    pub fn forward(&self, data: &mut [Complex32]) {
        debug_assert_eq!(data.len(), self.size);
        Fft::forward(data); // Uses inline twiddle computation for correctness; planner twiddles can be used for optimization
    }

    /// Execute the inverse FFT.
    pub fn inverse(&self, data: &mut [Complex32]) {
        debug_assert_eq!(data.len(), self.size);
        Fft::inverse(data);
    }
}

/// Pre-computes twiddle factors and caches plans by size.
pub struct FftPlanner {
    cache: HashMap<usize, Vec<Complex32>>,
}

impl FftPlanner {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    /// Return a plan for a given (power-of-2) size, building twiddles if needed.
    pub fn plan(&mut self, size: usize) -> FftPlan {
        let n = next_power_of_two(size);
        let twiddles = self.cache.entry(n).or_insert_with(|| {
            (0..n / 2)
                .map(|k| Complex32::from_polar(1.0, -2.0 * PI * k as f32 / n as f32))
                .collect()
        });
        FftPlan { size: n, twiddles: twiddles.clone() }
    }

    /// Clear the twiddle-factor cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl Default for FftPlanner {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Spectrum
// ---------------------------------------------------------------------------

/// Post-FFT frequency-domain representation.
#[derive(Debug, Clone)]
pub struct Spectrum {
    /// Complex FFT bins (full N/2+1 half-spectrum or full N).
    pub bins: Vec<Complex32>,
    /// The FFT size used.
    pub fft_size: usize,
}

impl Spectrum {
    pub fn new(bins: Vec<Complex32>, fft_size: usize) -> Self {
        Self { bins, fft_size }
    }

    /// Compute from real signal. Uses RealFft internally.
    pub fn from_real(signal: &[f32]) -> Self {
        let fft_size = next_power_of_two(signal.len());
        let bins = RealFft::forward_real(signal);
        Self { bins, fft_size }
    }

    /// Number of bins (N/2+1 for real input).
    pub fn num_bins(&self) -> usize { self.bins.len() }

    /// Magnitude of bin `k`.
    pub fn magnitude(&self, k: usize) -> f32 { self.bins[k].norm() }

    /// Phase of bin `k` in radians.
    pub fn phase(&self, k: usize) -> f32 { self.bins[k].arg() }

    /// Power of bin `k` (magnitude²).
    pub fn power(&self, k: usize) -> f32 { self.bins[k].norm_sq() }

    /// Frequency of bin `k` given sample rate.
    pub fn frequency_of_bin(&self, k: usize, sample_rate: f32) -> f32 {
        k as f32 * sample_rate / self.fft_size as f32
    }

    /// All magnitudes.
    pub fn magnitude_spectrum(&self) -> Vec<f32> {
        self.bins.iter().map(|c| c.norm()).collect()
    }

    /// Power spectrum (magnitude²).
    pub fn power_spectrum(&self) -> Vec<f32> {
        self.bins.iter().map(|c| c.norm_sq()).collect()
    }

    /// dBFS spectrum with noise floor.
    pub fn to_db(&self, floor_db: f32) -> Vec<f32> {
        self.bins.iter().map(|c| {
            let mag = c.norm();
            if mag <= 0.0 { return floor_db; }
            (20.0 * mag.log10()).max(floor_db)
        }).collect()
    }

    /// Frequency of the dominant (peak) bin.
    pub fn dominant_frequency(&self, sample_rate: f32) -> f32 {
        let (peak_k, _) = self.bins.iter().enumerate()
            .map(|(k, c)| (k, c.norm_sq()))
            .fold((0, 0.0f32), |(ak, av), (k, v)| if v > av { (k, v) } else { (ak, av) });
        self.frequency_of_bin(peak_k, sample_rate)
    }

    /// Spectral centroid — weighted mean frequency.
    pub fn spectral_centroid(&self, sample_rate: f32) -> f32 {
        let mut num = 0.0f32;
        let mut denom = 0.0f32;
        for (k, c) in self.bins.iter().enumerate() {
            let mag = c.norm();
            let freq = self.frequency_of_bin(k, sample_rate);
            num += freq * mag;
            denom += mag;
        }
        if denom < 1e-10 { 0.0 } else { num / denom }
    }

    /// Spectral spread — weighted standard deviation of frequency.
    pub fn spectral_spread(&self, sample_rate: f32) -> f32 {
        let centroid = self.spectral_centroid(sample_rate);
        let mut num = 0.0f32;
        let mut denom = 0.0f32;
        for (k, c) in self.bins.iter().enumerate() {
            let mag = c.norm();
            let freq = self.frequency_of_bin(k, sample_rate);
            num += (freq - centroid).powi(2) * mag;
            denom += mag;
        }
        if denom < 1e-10 { 0.0 } else { (num / denom).sqrt() }
    }

    /// Spectral flux: sum of positive differences between consecutive frames.
    pub fn spectral_flux(&self, other: &Spectrum) -> f32 {
        let len = self.bins.len().min(other.bins.len());
        let mut flux = 0.0f32;
        for i in 0..len {
            let diff = other.bins[i].norm() - self.bins[i].norm();
            if diff > 0.0 { flux += diff; }
        }
        flux
    }

    /// Spectral flatness (Wiener entropy): geometric mean / arithmetic mean.
    pub fn spectral_flatness(&self) -> f32 {
        let mags: Vec<f32> = self.bins.iter().map(|c| c.norm()).collect();
        let n = mags.len();
        if n == 0 { return 0.0; }
        let arithmetic_mean: f32 = mags.iter().sum::<f32>() / n as f32;
        if arithmetic_mean < 1e-10 { return 1.0; }
        let log_sum: f32 = mags.iter()
            .map(|&m| if m > 1e-10 { m.ln() } else { -23.0 }) // ln(1e-10)
            .sum();
        let geometric_mean = (log_sum / n as f32).exp();
        (geometric_mean / arithmetic_mean).min(1.0)
    }

    /// Spectral rolloff: frequency below which `threshold` fraction of energy is contained.
    pub fn spectral_rolloff(&self, threshold: f32) -> f32 {
        // Computed over bins 0..num_bins
        let total: f32 = self.bins.iter().map(|c| c.norm_sq()).sum();
        if total < 1e-10 { return 0.0; }
        let target = threshold * total;
        let mut accum = 0.0f32;
        for (k, c) in self.bins.iter().enumerate() {
            accum += c.norm_sq();
            if accum >= target {
                // Linearly interpolate between bins k-1 and k
                if k == 0 { return 0.0; }
                let prev = accum - c.norm_sq();
                let frac = (target - prev) / c.norm_sq().max(1e-30);
                return (k as f32 - 1.0 + frac.min(1.0)) / self.fft_size as f32;
            }
        }
        (self.bins.len() - 1) as f32 / self.fft_size as f32
    }

    /// Spectral rolloff frequency in Hz (not normalized).
    pub fn spectral_rolloff_hz(&self, threshold: f32, sample_rate: f32) -> f32 {
        self.spectral_rolloff(threshold) * sample_rate
    }

    /// Reconstruct the time-domain signal via IFFT.
    pub fn to_signal(&self) -> Vec<f32> {
        RealFft::inverse_real(&self.bins, self.fft_size)
    }
}

// ---------------------------------------------------------------------------
// Stft — Short-Time Fourier Transform
// ---------------------------------------------------------------------------

/// Configuration for the STFT.
#[derive(Debug, Clone)]
pub struct StftConfig {
    pub fft_size: usize,
    pub hop_size: usize,
    pub window: WindowFunction,
}

impl Default for StftConfig {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            hop_size: 512,
            window: WindowFunction::Hann,
        }
    }
}

/// Short-Time Fourier Transform: sliding-window analysis and synthesis.
pub struct Stft {
    pub config: StftConfig,
}

impl Stft {
    pub fn new(config: StftConfig) -> Self {
        assert!(super::is_power_of_two(config.fft_size), "fft_size must be power of 2");
        Self { config }
    }

    /// Analyze a signal into a sequence of Spectrum frames.
    pub fn analyze(&self, signal: &[f32]) -> Vec<Spectrum> {
        let cfg = &self.config;
        let win = cfg.window.generate(cfg.fft_size);
        let mut frames = Vec::new();
        let mut pos = 0usize;
        while pos + cfg.fft_size <= signal.len() {
            let mut frame: Vec<f32> = signal[pos..pos + cfg.fft_size].to_vec();
            for (s, &w) in frame.iter_mut().zip(win.iter()) {
                *s *= w;
            }
            let bins = RealFft::forward_real(&frame);
            frames.push(Spectrum::new(bins, cfg.fft_size));
            pos += cfg.hop_size;
        }
        // Handle the last partial frame with zero-padding
        if pos < signal.len() {
            let mut frame = vec![0.0f32; cfg.fft_size];
            let rem = signal.len() - pos;
            frame[..rem].copy_from_slice(&signal[pos..]);
            for (s, &w) in frame.iter_mut().zip(win.iter()) {
                *s *= w;
            }
            let bins = RealFft::forward_real(&frame);
            frames.push(Spectrum::new(bins, cfg.fft_size));
        }
        frames
    }

    /// Synthesize a signal from STFT frames using overlap-add (OLA).
    pub fn synthesize(&self, frames: &[Spectrum]) -> Vec<f32> {
        let cfg = &self.config;
        if frames.is_empty() { return Vec::new(); }
        let total_len = (frames.len() - 1) * cfg.hop_size + cfg.fft_size;
        let mut output = vec![0.0f32; total_len];
        let mut norm = vec![0.0f32; total_len];
        let win = cfg.window.generate(cfg.fft_size);
        // Compute normalization factor for OLA
        for (frame_idx, spectrum) in frames.iter().enumerate() {
            let pos = frame_idx * cfg.hop_size;
            let time_signal = spectrum.to_signal();
            for (i, (&s, &w)) in time_signal.iter().zip(win.iter()).enumerate() {
                if pos + i < output.len() {
                    output[pos + i] += s * w;
                    norm[pos + i] += w * w;
                }
            }
        }
        // Normalize
        for (s, &n) in output.iter_mut().zip(norm.iter()) {
            if n > 1e-10 { *s /= n; }
        }
        output
    }

    /// Number of frames for a signal of given length.
    pub fn num_frames(&self, signal_len: usize) -> usize {
        if signal_len < self.config.fft_size { return 0; }
        1 + (signal_len - self.config.fft_size) / self.config.hop_size
    }

    /// Time in seconds of the center of frame `idx`.
    pub fn frame_time(&self, idx: usize, sample_rate: f32) -> f32 {
        (idx * self.config.hop_size + self.config.fft_size / 2) as f32 / sample_rate
    }
}

// ---------------------------------------------------------------------------
// Cqt — Constant-Q Transform
// ---------------------------------------------------------------------------

/// Constant-Q Transform — logarithmically-spaced frequency bins.
pub struct Cqt {
    pub sample_rate: f32,
    pub min_freq: f32,
    pub bins_per_octave: u32,
    pub n_bins: u32,
}

impl Cqt {
    pub fn new(sample_rate: f32, min_freq: f32, bins_per_octave: u32, n_octaves: u32) -> Self {
        Self {
            sample_rate,
            min_freq,
            bins_per_octave,
            n_bins: bins_per_octave * n_octaves,
        }
    }

    /// Q factor for this transform.
    pub fn q_factor(&self) -> f32 {
        1.0 / (2.0f32.powf(1.0 / self.bins_per_octave as f32) - 1.0)
    }

    /// Analyze signal using the direct CQT kernel approach.
    pub fn analyze(&self, signal: &[f32], min_freq: f32, bins_per_octave: u32) -> Vec<f32> {
        let n_bins = self.n_bins as usize;
        let q = 1.0 / (2.0f32.powf(1.0 / bins_per_octave as f32) - 1.0);
        let mut result = vec![0.0f32; n_bins];

        for bin in 0..n_bins {
            let freq = min_freq * 2.0f32.powf(bin as f32 / bins_per_octave as f32);
            let window_len = (q * self.sample_rate / freq).round() as usize;
            if window_len == 0 || window_len > signal.len() {
                continue;
            }
            // Direct DFT at this frequency
            let mut re = 0.0f32;
            let mut im = 0.0f32;
            for n in 0..window_len {
                // Hann window
                let win = 0.5 * (1.0 - (2.0 * PI * n as f32 / (window_len - 1) as f32).cos());
                let sample = signal.get(n).copied().unwrap_or(0.0);
                let phase = 2.0 * PI * q * n as f32 / window_len as f32;
                re += win * sample * phase.cos();
                im -= win * sample * phase.sin();
            }
            result[bin] = (re * re + im * im).sqrt() / window_len as f32;
        }
        result
    }

    /// Frequency of CQT bin `k`.
    pub fn bin_frequency(&self, k: usize) -> f32 {
        self.min_freq * 2.0f32.powf(k as f32 / self.bins_per_octave as f32)
    }

    /// Convert CQT output to dB.
    pub fn to_db(cqt: &[f32], floor_db: f32) -> Vec<f32> {
        cqt.iter().map(|&x| {
            if x <= 0.0 { floor_db } else { (20.0 * x.log10()).max(floor_db) }
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// Autocorrelation
// ---------------------------------------------------------------------------

/// Autocorrelation functions and YIN pitch detection.
pub struct Autocorrelation;

impl Autocorrelation {
    /// Compute the normalized autocorrelation of a signal.
    pub fn compute(signal: &[f32]) -> Vec<f32> {
        let n = signal.len();
        let mut out = vec![0.0f32; n];
        let energy: f32 = signal.iter().map(|&x| x * x).sum();
        if energy < 1e-10 { return out; }
        for lag in 0..n {
            let mut sum = 0.0f32;
            for i in 0..n - lag {
                sum += signal[i] * signal[i + lag];
            }
            out[lag] = sum / energy;
        }
        out
    }

    /// Autocorrelation via FFT (O(N log N)).
    pub fn compute_fft(signal: &[f32]) -> Vec<f32> {
        let n = next_power_of_two(signal.len() * 2);
        let mut buf: Vec<Complex32> = signal.iter()
            .map(|&x| Complex32::new(x, 0.0))
            .collect();
        buf.resize(n, Complex32::zero());
        Fft::forward(&mut buf);
        // Multiply by conjugate = power spectrum
        for c in buf.iter_mut() {
            *c = Complex32::new(c.norm_sq(), 0.0);
        }
        Fft::inverse(&mut buf);
        let scale = 1.0 / buf[0].re.max(1e-30);
        buf[..signal.len()].iter().map(|c| c.re * scale).collect()
    }

    /// YIN pitch detection algorithm.
    /// Returns the fundamental frequency in Hz, or None if no pitch found.
    pub fn pitch_yin(signal: &[f32], sample_rate: f32) -> Option<f32> {
        let n = signal.len();
        if n < 2 { return None; }
        let half = n / 2;

        // Step 1: difference function
        let mut d = vec![0.0f32; half];
        for tau in 0..half {
            for j in 0..half {
                let diff = signal[j] - signal[j + tau];
                d[tau] += diff * diff;
            }
        }
        d[0] = 0.0;

        // Step 2: cumulative mean normalized difference
        let mut cmnd = vec![0.0f32; half];
        cmnd[0] = 1.0;
        let mut running_sum = 0.0f32;
        for tau in 1..half {
            running_sum += d[tau];
            cmnd[tau] = if running_sum > 1e-10 {
                d[tau] * tau as f32 / running_sum
            } else {
                1.0
            };
        }

        // Step 3: absolute threshold — find first local minimum below threshold
        let threshold = 0.1f32;
        let mut tau_min = None;
        for tau in 2..half - 1 {
            if cmnd[tau] < threshold && cmnd[tau] < cmnd[tau - 1] && cmnd[tau] < cmnd[tau + 1] {
                tau_min = Some(tau);
                break;
            }
        }

        // If no strong minimum found, use global minimum
        let tau = tau_min.unwrap_or_else(|| {
            cmnd[1..].iter().enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(i, _)| i + 1)
                .unwrap_or(0)
        });

        if tau == 0 { return None; }

        // Step 4: parabolic interpolation around the minimum
        let tau_f = if tau > 0 && tau < half - 1 {
            let alpha = cmnd[tau - 1];
            let beta  = cmnd[tau];
            let gamma = cmnd[tau + 1];
            let denom = alpha - 2.0 * beta + gamma;
            if denom.abs() > 1e-10 {
                tau as f32 - 0.5 * (gamma - alpha) / denom
            } else {
                tau as f32
            }
        } else {
            tau as f32
        };

        if tau_f < 1.0 { return None; }
        let freq = sample_rate / tau_f;
        if freq < 20.0 || freq > 20000.0 { return None; }
        Some(freq)
    }
}

// ---------------------------------------------------------------------------
// MelFilterbank
// ---------------------------------------------------------------------------

/// Mel-scale filterbank for perceptual frequency analysis.
pub struct MelFilterbank {
    /// Number of Mel filters.
    pub n_filters: usize,
    /// FFT size.
    pub fft_size: usize,
    /// Sample rate.
    pub sample_rate: f32,
    /// Filter weights: n_filters × (fft_size/2+1).
    filterbank: Vec<Vec<f32>>,
}

impl MelFilterbank {
    /// Create a Mel filterbank.
    pub fn new(n_filters: usize, fft_size: usize, sample_rate: f32) -> Self {
        let n_bins = fft_size / 2 + 1;
        let min_mel = super::hz_to_mel(0.0);
        let max_mel = super::hz_to_mel(sample_rate / 2.0);

        // Linearly-spaced Mel points (n_filters + 2 for lower and upper edges)
        let mel_points: Vec<f32> = (0..n_filters + 2)
            .map(|i| min_mel + i as f32 * (max_mel - min_mel) / (n_filters + 1) as f32)
            .collect();
        let hz_points: Vec<f32> = mel_points.iter().map(|&m| super::mel_to_hz(m)).collect();
        // Convert Hz to FFT bin indices
        let bin_points: Vec<f32> = hz_points.iter()
            .map(|&hz| hz * fft_size as f32 / sample_rate)
            .collect();

        let mut filterbank = vec![vec![0.0f32; n_bins]; n_filters];
        for m in 0..n_filters {
            let left   = bin_points[m];
            let center = bin_points[m + 1];
            let right  = bin_points[m + 2];
            for k in 0..n_bins {
                let k_f = k as f32;
                if k_f >= left && k_f <= center {
                    filterbank[m][k] = (k_f - left) / (center - left).max(1e-10);
                } else if k_f > center && k_f <= right {
                    filterbank[m][k] = (right - k_f) / (right - center).max(1e-10);
                }
            }
        }

        Self { n_filters, fft_size, sample_rate, filterbank }
    }

    /// Apply the filterbank to a power spectrum. Returns n_filters values.
    pub fn apply(&self, spectrum: &[f32]) -> Vec<f32> {
        let n_bins = self.fft_size / 2 + 1;
        self.filterbank.iter().map(|filter| {
            filter.iter().zip(spectrum.iter().take(n_bins))
                .map(|(&w, &s)| w * s)
                .sum()
        }).collect()
    }

    /// Apply to a Spectrum struct.
    pub fn apply_spectrum(&self, spectrum: &Spectrum) -> Vec<f32> {
        let power: Vec<f32> = spectrum.power_spectrum();
        self.apply(&power)
    }
}

// ---------------------------------------------------------------------------
// Mfcc — Mel-Frequency Cepstral Coefficients
// ---------------------------------------------------------------------------

/// Mel-Frequency Cepstral Coefficient computation.
pub struct Mfcc {
    pub n_coeffs: usize,
    pub sample_rate: f32,
    pub fft_size: usize,
    pub n_mel: usize,
    filterbank: MelFilterbank,
}

impl Mfcc {
    pub fn new(n_coeffs: usize, fft_size: usize, sample_rate: f32) -> Self {
        let n_mel = 40;
        let filterbank = MelFilterbank::new(n_mel, fft_size, sample_rate);
        Self { n_coeffs, sample_rate, fft_size, n_mel, filterbank }
    }

    /// Compute MFCCs for a signal block.
    pub fn compute(&self, signal: &[f32], _sample_rate: f32, n_coeffs: usize) -> Vec<f32> {
        // 1. Windowed FFT
        let n = next_power_of_two(signal.len().max(self.fft_size));
        let mut frame: Vec<f32> = signal.to_vec();
        frame.resize(n, 0.0);
        WindowFunction::Hann.apply(&mut frame);
        let spectrum = Spectrum::from_real(&frame);

        // 2. Mel filterbank
        let mel_energies = self.filterbank.apply_spectrum(&spectrum);

        // 3. Log
        let log_mel: Vec<f32> = mel_energies.iter()
            .map(|&e| if e > 1e-10 { e.ln() } else { -23.0 })
            .collect();

        // 4. DCT type-II
        Self::dct_ii(&log_mel, n_coeffs)
    }

    /// DCT type-II: N input → n_coeffs output.
    pub fn dct_ii(input: &[f32], n_coeffs: usize) -> Vec<f32> {
        let n = input.len();
        let mut out = Vec::with_capacity(n_coeffs);
        let scale0 = (1.0 / n as f32).sqrt();
        let scale_k = (2.0 / n as f32).sqrt();
        for k in 0..n_coeffs {
            let scale = if k == 0 { scale0 } else { scale_k };
            let sum: f32 = input.iter().enumerate().map(|(n_idx, &x)| {
                x * (PI * k as f32 * (2 * n_idx + 1) as f32 / (2 * n) as f32).cos()
            }).sum();
            out.push(scale * sum);
        }
        out
    }

    /// Inverse DCT type-II.
    pub fn idct_ii(coeffs: &[f32], n_out: usize) -> Vec<f32> {
        let n_coeffs = coeffs.len();
        let mut out = vec![0.0f32; n_out];
        let scale0 = (1.0 / n_out as f32).sqrt();
        let scale_k = (2.0 / n_out as f32).sqrt();
        for (n_idx, s) in out.iter_mut().enumerate() {
            let mut sum = 0.0f32;
            for k in 0..n_coeffs {
                let scale = if k == 0 { scale0 } else { scale_k };
                sum += scale * coeffs[k]
                    * (PI * k as f32 * (2 * n_idx + 1) as f32 / (2 * n_out) as f32).cos();
            }
            *s = sum;
        }
        out
    }

    /// Compute delta (first-order difference) of a feature matrix (rows = frames).
    pub fn delta(features: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let n_frames = features.len();
        if n_frames < 3 { return features.to_vec(); }
        let n_feats = features[0].len();
        let width = 2usize; // ±2 frames
        let denom: f32 = (1..=width as i32).map(|m| (m * m) as f32).sum::<f32>() * 2.0;
        features.iter().enumerate().map(|(t, _)| {
            (0..n_feats).map(|d| {
                let mut num = 0.0f32;
                for m in 1..=width {
                    let fwd = features.get(t + m).map(|f| f[d]).unwrap_or_else(|| features[n_frames - 1][d]);
                    let bwd = if t >= m { features[t - m][d] } else { features[0][d] };
                    num += m as f32 * (fwd - bwd);
                }
                num / denom
            }).collect()
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// Chroma
// ---------------------------------------------------------------------------

/// Chroma feature — pitch class profile (12 semitone bins).
pub struct Chroma;

impl Chroma {
    /// Compute the 12-bin chroma vector from a Spectrum.
    pub fn compute(spectrum: &Spectrum, sample_rate: f32) -> [f32; 12] {
        let mut chroma = [0.0f32; 12];
        let n_bins = spectrum.num_bins();
        // Ignore DC (bin 0)
        for k in 1..n_bins {
            let freq = spectrum.frequency_of_bin(k, sample_rate);
            if freq <= 0.0 { continue; }
            // Convert to pitch class: midi note mod 12
            let midi = super::freq_to_midi(freq);
            if midi < 0.0 { continue; }
            let pitch_class = (midi.round() as i32).rem_euclid(12) as usize;
            chroma[pitch_class] += spectrum.magnitude(k);
        }
        // Normalize to sum to 1
        let sum: f32 = chroma.iter().sum();
        if sum > 1e-10 {
            for c in chroma.iter_mut() { *c /= sum; }
        }
        chroma
    }

    /// Distance between two chroma vectors (cosine distance, 0=identical, 1=orthogonal).
    pub fn cosine_distance(a: &[f32; 12], b: &[f32; 12]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|&x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|&x| x * x).sum::<f32>().sqrt();
        let denom = norm_a * norm_b;
        if denom < 1e-10 { return 1.0; }
        1.0 - (dot / denom).min(1.0)
    }

    /// Rotate chroma vector by `semitones` (for transposition-invariant comparison).
    pub fn rotate(chroma: &[f32; 12], semitones: i32) -> [f32; 12] {
        let mut out = [0.0f32; 12];
        for i in 0..12 {
            out[i] = chroma[(i as i32 - semitones).rem_euclid(12) as usize];
        }
        out
    }

    /// Find the key (0=C, 1=C#, ..., 11=B) that best matches the chroma.
    /// Uses the Krumhansl-Schmuckler key profiles.
    pub fn estimate_key(chroma: &[f32; 12]) -> (usize, bool) {
        // Major and minor key profiles (Krumhansl-Schmuckler)
        let major = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
        let minor = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

        let mut best_key = 0usize;
        let mut best_is_minor = false;
        let mut best_corr = -f32::INFINITY;

        for root in 0..12 {
            // Major
            let maj_corr = Self::profile_correlation(chroma, &major, root);
            if maj_corr > best_corr {
                best_corr = maj_corr;
                best_key = root;
                best_is_minor = false;
            }
            // Minor
            let min_corr = Self::profile_correlation(chroma, &minor, root);
            if min_corr > best_corr {
                best_corr = min_corr;
                best_key = root;
                best_is_minor = true;
            }
        }
        (best_key, best_is_minor)
    }

    fn profile_correlation(chroma: &[f32; 12], profile: &[f64; 12], root: usize) -> f32 {
        // Pearson correlation
        let c_mean: f32 = chroma.iter().sum::<f32>() / 12.0;
        let p_mean: f64 = profile.iter().sum::<f64>() / 12.0;
        let mut num = 0.0f64;
        let mut var_c = 0.0f64;
        let mut var_p = 0.0f64;
        for i in 0..12 {
            let ci = (chroma[(i + root) % 12] - c_mean) as f64;
            let pi = profile[i] - p_mean;
            num += ci * pi;
            var_c += ci * ci;
            var_p += pi * pi;
        }
        let denom = (var_c * var_p).sqrt();
        if denom < 1e-10 { 0.0 } else { (num / denom) as f32 }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::SignalGenerator;

    fn nearly_eq(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    // --- Complex32 arithmetic ---

    #[test]
    fn test_complex_add_sub() {
        let a = Complex32::new(1.0, 2.0);
        let b = Complex32::new(3.0, -1.0);
        let s = a + b;
        assert!(nearly_eq(s.re, 4.0, 1e-6));
        assert!(nearly_eq(s.im, 1.0, 1e-6));
        let d = a - b;
        assert!(nearly_eq(d.re, -2.0, 1e-6));
        assert!(nearly_eq(d.im, 3.0, 1e-6));
    }

    #[test]
    fn test_complex_mul() {
        let a = Complex32::new(1.0, 2.0);
        let b = Complex32::new(3.0, 4.0);
        let m = a * b;
        assert!(nearly_eq(m.re, -5.0, 1e-5));
        assert!(nearly_eq(m.im, 10.0, 1e-5));
    }

    #[test]
    fn test_complex_norm_and_conj() {
        let c = Complex32::new(3.0, 4.0);
        assert!(nearly_eq(c.norm(), 5.0, 1e-5));
        let conj = c.conj();
        assert!(nearly_eq(conj.im, -4.0, 1e-6));
    }

    #[test]
    fn test_fft_roundtrip() {
        let n = 64usize;
        let signal: Vec<f32> = (0..n).map(|i| (2.0 * PI * 3.0 * i as f32 / n as f32).sin()).collect();
        let mut buf: Vec<Complex32> = signal.iter().map(|&x| Complex32::new(x, 0.0)).collect();
        Fft::forward(&mut buf);
        Fft::inverse(&mut buf);
        for (orig, recovered) in signal.iter().zip(buf.iter()) {
            assert!(nearly_eq(*orig, recovered.re, 1e-4), "orig={}, rec={}", orig, recovered.re);
        }
    }

    #[test]
    fn test_fft_impulse() {
        // FFT of an impulse should be all-ones in magnitude
        let n = 16usize;
        let mut buf = vec![Complex32::zero(); n];
        buf[0] = Complex32::one();
        Fft::forward(&mut buf);
        for c in &buf {
            assert!(nearly_eq(c.norm(), 1.0, 1e-5));
        }
    }

    #[test]
    fn test_real_fft_roundtrip() {
        let signal: Vec<f32> = (0..256).map(|i| (2.0 * PI * 10.0 * i as f32 / 256.0).sin()).collect();
        let spectrum = RealFft::forward_real(&signal);
        let recovered = RealFft::inverse_real(&spectrum, 256);
        for (orig, rec) in signal.iter().zip(recovered.iter()) {
            assert!(nearly_eq(*orig, *rec, 1e-3), "orig={}, rec={}", orig, rec);
        }
    }

    #[test]
    fn test_spectrum_dominant_frequency() {
        let sr = 44100.0f32;
        let freq = 440.0f32;
        let sig = SignalGenerator::sine(freq, 1.0, 1.0, sr);
        let win_size = 4096;
        let frame: Vec<f32> = sig.samples[..win_size].to_vec();
        let spectrum = Spectrum::from_real(&frame);
        let dom = spectrum.dominant_frequency(sr);
        // Should be within a couple bins of 440 Hz
        assert!((dom - freq).abs() < sr / win_size as f32 * 2.0, "dom={}", dom);
    }

    #[test]
    fn test_spectrum_centroid_noise() {
        // Flat spectrum -> centroid near Nyquist/2
        let mut buf: Vec<Complex32> = (0..513).map(|_| Complex32::new(1.0, 0.0)).collect();
        let spectrum = Spectrum::new(buf.clone(), 1024);
        let centroid = spectrum.spectral_centroid(44100.0);
        assert!(centroid > 10000.0 && centroid < 15000.0, "centroid={}", centroid);
    }

    #[test]
    fn test_spectrum_flatness_flat() {
        let buf: Vec<Complex32> = (0..513).map(|_| Complex32::new(1.0, 0.0)).collect();
        let spectrum = Spectrum::new(buf, 1024);
        let flatness = spectrum.spectral_flatness();
        // All-ones magnitude → flatness ≈ 1.0
        assert!(flatness > 0.9, "flatness={}", flatness);
    }

    #[test]
    fn test_spectrum_flux() {
        let buf1: Vec<Complex32> = (0..65).map(|_| Complex32::new(1.0, 0.0)).collect();
        let buf2: Vec<Complex32> = (0..65).map(|_| Complex32::new(2.0, 0.0)).collect();
        let s1 = Spectrum::new(buf1, 128);
        let s2 = Spectrum::new(buf2, 128);
        let flux = s1.spectral_flux(&s2);
        assert!(flux > 0.0);
    }

    #[test]
    fn test_stft_analyze_synthesize() {
        let sr = 44100.0;
        let sig = SignalGenerator::sine(440.0, 1.0, 0.1, sr);
        let stft = Stft::new(StftConfig { fft_size: 1024, hop_size: 256, window: WindowFunction::Hann });
        let frames = stft.analyze(&sig.samples);
        assert!(!frames.is_empty());
        let reconstructed = stft.synthesize(&frames);
        // Reconstruction should have similar RMS
        let orig_rms: f32 = {
            let s: f32 = sig.samples.iter().map(|&x| x * x).sum();
            (s / sig.samples.len() as f32).sqrt()
        };
        let rec_len = reconstructed.len().min(sig.samples.len());
        let rec_rms: f32 = {
            let s: f32 = reconstructed[..rec_len].iter().map(|&x| x * x).sum();
            (s / rec_len as f32).sqrt()
        };
        // Allow significant tolerance for OLA boundary effects
        assert!((orig_rms - rec_rms).abs() < 0.3, "orig_rms={}, rec_rms={}", orig_rms, rec_rms);
    }

    #[test]
    fn test_yin_pitch_detection() {
        let sr = 44100.0;
        let freq = 220.0f32;
        let sig = SignalGenerator::sine(freq, 1.0, 0.05, sr);
        let detected = Autocorrelation::pitch_yin(&sig.samples, sr);
        assert!(detected.is_some(), "YIN should detect pitch");
        let detected_freq = detected.unwrap();
        assert!((detected_freq - freq).abs() < 10.0, "detected={}, expected={}", detected_freq, freq);
    }

    #[test]
    fn test_mel_filterbank_shape() {
        let fb = MelFilterbank::new(40, 2048, 44100.0);
        assert_eq!(fb.filterbank.len(), 40);
        assert_eq!(fb.filterbank[0].len(), 1025);
    }

    #[test]
    fn test_mel_filterbank_apply() {
        let fb = MelFilterbank::new(40, 2048, 44100.0);
        let flat_spectrum = vec![1.0f32; 1025];
        let mel = fb.apply(&flat_spectrum);
        assert_eq!(mel.len(), 40);
        for &v in &mel {
            assert!(v >= 0.0, "All mel values must be non-negative");
        }
    }

    #[test]
    fn test_mfcc_shape() {
        let sr = 44100.0;
        let sig = SignalGenerator::sine(440.0, 1.0, 0.05, sr);
        let mfcc = Mfcc::new(13, 2048, sr);
        let coeffs = mfcc.compute(&sig.samples, sr, 13);
        assert_eq!(coeffs.len(), 13);
    }

    #[test]
    fn test_chroma_unit_sum() {
        let sr = 44100.0;
        let sig = SignalGenerator::sine(440.0, 1.0, 0.1, sr);
        let spectrum = Spectrum::from_real(&sig.samples[..2048]);
        let chroma = Chroma::compute(&spectrum, sr);
        let sum: f32 = chroma.iter().sum();
        // Should be approximately 1 (normalized)
        if sum > 1e-10 {
            assert!((sum - 1.0).abs() < 1e-4, "sum={}", sum);
        }
    }

    #[test]
    fn test_fft_planner_caching() {
        let mut planner = FftPlanner::new();
        let plan1 = planner.plan(512);
        let plan2 = planner.plan(512);
        assert_eq!(plan1.size, plan2.size);
    }

    #[test]
    fn test_autocorr_unity_at_zero() {
        let sig: Vec<f32> = (0..256).map(|i| (0.1 * i as f32).sin()).collect();
        let ac = Autocorrelation::compute(&sig);
        assert!(nearly_eq(ac[0], 1.0, 1e-4));
    }

    #[test]
    fn test_cqt_output_shape() {
        let cqt = Cqt::new(44100.0, 55.0, 12, 7);
        let sig: Vec<f32> = (0..44100).map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin()).collect();
        let result = cqt.analyze(&sig, 55.0, 12);
        assert_eq!(result.len(), (12 * 7) as usize);
    }

    #[test]
    fn test_spectrum_rolloff() {
        let n_bins = 513;
        // Bins 0..256 have energy 1, rest have 0
        let bins: Vec<Complex32> = (0..n_bins).map(|k| {
            if k < 256 { Complex32::new(1.0, 0.0) } else { Complex32::zero() }
        }).collect();
        let spec = Spectrum::new(bins, 1024);
        let rolloff = spec.spectral_rolloff(0.85);
        // Should be around bin 256 / 1024 ≈ 0.25
        assert!(rolloff <= 0.5, "rolloff={}", rolloff);
    }

    #[test]
    fn test_chroma_cosine_distance_identical() {
        let a = [1.0f32; 12].map(|x| x / 12.0);
        let dist = Chroma::cosine_distance(&a, &a);
        assert!(dist.abs() < 1e-5, "dist={}", dist);
    }

    #[test]
    fn test_dct_round_trip() {
        let input: Vec<f32> = (0..20).map(|i| i as f32 * 0.1).collect();
        let coeffs = Mfcc::dct_ii(&input, 20);
        let recovered = Mfcc::idct_ii(&coeffs, 20);
        for (a, b) in input.iter().zip(recovered.iter()) {
            assert!(nearly_eq(*a, *b, 1e-3), "a={}, b={}", a, b);
        }
    }

    #[test]
    fn test_complex_from_polar_roundtrip() {
        let r = 3.5f32;
        let theta = 1.2f32;
        let c = Complex32::from_polar(r, theta);
        assert!(nearly_eq(c.norm(), r, 1e-5));
        assert!(nearly_eq(c.arg(), theta, 1e-5));
    }

    #[test]
    fn test_fft_linearity() {
        let n = 32;
        let a: Vec<Complex32> = (0..n).map(|i| Complex32::new((i as f32 * 0.1).sin(), 0.0)).collect();
        let b: Vec<Complex32> = (0..n).map(|i| Complex32::new((i as f32 * 0.2).cos(), 0.0)).collect();
        let mut fa = a.clone();
        let mut fb = b.clone();
        Fft::forward(&mut fa);
        Fft::forward(&mut fb);
        // F(a + b) == F(a) + F(b)
        let mut ab: Vec<Complex32> = a.iter().zip(b.iter()).map(|(&x, &y)| x + y).collect();
        Fft::forward(&mut ab);
        for ((&fa_k, &fb_k), &fab_k) in fa.iter().zip(fb.iter()).zip(ab.iter()) {
            let diff = (fa_k + fb_k) - fab_k;
            assert!(diff.norm() < 1e-3, "linearity violation at bin: {}", diff.norm());
        }
    }
}
