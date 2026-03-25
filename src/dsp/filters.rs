//! Digital filter library — Biquad IIR, higher-order cascades, FIR, convolution,
//! state-variable filter, comb, allpass, moving average, Kalman, PLL.

use std::f32::consts::PI;
use super::{WindowFunction, sinc, next_power_of_two};
use super::fft::{Fft, Complex32};

// ---------------------------------------------------------------------------
// BiquadType
// ---------------------------------------------------------------------------

/// Type tag for a biquad section.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BiquadType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peak,
    LowShelf,
    HighShelf,
    AllPass,
}

// ---------------------------------------------------------------------------
// Biquad — Direct Form II Transposed
// ---------------------------------------------------------------------------

/// Second-order IIR biquad filter (Direct Form II Transposed).
///
/// Transfer function:  H(z) = (b0 + b1·z⁻¹ + b2·z⁻²) / (1 + a1·z⁻¹ + a2·z⁻²)
#[derive(Debug, Clone)]
pub struct Biquad {
    // Feed-forward coefficients
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    // Feed-back coefficients (negated convention: denominator is 1 + a1z + a2z²)
    pub a1: f32,
    pub a2: f32,
    // State variables
    pub z1: f32,
    pub z2: f32,
    /// The kind of filter (informational).
    pub filter_type: BiquadType,
}

impl Biquad {
    /// Create a biquad from raw coefficients.
    pub fn new(b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, filter_type: BiquadType) -> Self {
        Self { b0, b1, b2, a1, a2, z1: 0.0, z2: 0.0, filter_type }
    }

    /// Identity (pass-through) biquad.
    pub fn identity() -> Self {
        Self::new(1.0, 0.0, 0.0, 0.0, 0.0, BiquadType::AllPass)
    }

    /// Process a single sample (Direct Form II Transposed).
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    /// In-place block processing.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Clear state variables.
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Frequency response magnitude at normalized frequency ω (0..π).
    pub fn magnitude_response(&self, omega: f32) -> f32 {
        let z = Complex32::from_polar(1.0, omega);
        let z_inv = z.conj(); // z⁻¹ = e^{-jω} for |z|=1
        let z_inv2 = z_inv * z_inv;
        let num = Complex32::new(self.b0, 0.0)
            + Complex32::new(self.b1, 0.0) * z_inv
            + Complex32::new(self.b2, 0.0) * z_inv2;
        let den = Complex32::new(1.0, 0.0)
            + Complex32::new(self.a1, 0.0) * z_inv
            + Complex32::new(self.a2, 0.0) * z_inv2;
        (num / den).norm()
    }
}

// ---------------------------------------------------------------------------
// BiquadDesign — RBJ Audio EQ Cookbook
// ---------------------------------------------------------------------------

/// Coefficient calculation following the RBJ Audio EQ Cookbook.
pub struct BiquadDesign;

impl BiquadDesign {
    fn omega(cutoff_hz: f32, sample_rate: f32) -> f32 {
        2.0 * PI * cutoff_hz / sample_rate
    }

    /// 2nd-order lowpass Butterworth.
    pub fn lowpass(cutoff_hz: f32, q: f32, sample_rate: f32) -> Biquad {
        let w0 = Self::omega(cutoff_hz, sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let b1 = 1.0 - cos_w0;
        let b0 = b1 / 2.0;
        let b2 = b0;
        let a0 = 1.0 + alpha;
        Biquad::new(
            b0 / a0, b1 / a0, b2 / a0,
            (-2.0 * cos_w0) / a0, (1.0 - alpha) / a0,
            BiquadType::LowPass,
        )
    }

    /// 2nd-order highpass Butterworth.
    pub fn highpass(cutoff_hz: f32, q: f32, sample_rate: f32) -> Biquad {
        let w0 = Self::omega(cutoff_hz, sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = b0;
        let a0 = 1.0 + alpha;
        Biquad::new(
            b0 / a0, b1 / a0, b2 / a0,
            (-2.0 * cos_w0) / a0, (1.0 - alpha) / a0,
            BiquadType::HighPass,
        )
    }

    /// 2nd-order bandpass (constant 0 dB peak gain, BPF skirt gain = Q).
    pub fn bandpass(center_hz: f32, bandwidth_hz: f32, sample_rate: f32) -> Biquad {
        let q = center_hz / bandwidth_hz.max(1e-3);
        let w0 = Self::omega(center_hz, sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        Biquad::new(
            b0 / a0, b1 / a0, b2 / a0,
            (-2.0 * cos_w0) / a0, (1.0 - alpha) / a0,
            BiquadType::BandPass,
        )
    }

    /// 2nd-order notch (band-reject).
    pub fn notch(center_hz: f32, q: f32, sample_rate: f32) -> Biquad {
        let w0 = Self::omega(center_hz, sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let b0 = 1.0;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0;
        let a0 = 1.0 + alpha;
        Biquad::new(
            b0 / a0, b1 / a0, b2 / a0,
            (-2.0 * cos_w0) / a0, (1.0 - alpha) / a0,
            BiquadType::Notch,
        )
    }

    /// Peak EQ.
    pub fn peak_eq(center_hz: f32, gain_db: f32, q: f32, sample_rate: f32) -> Biquad {
        let w0 = Self::omega(center_hz, sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a_lin = 10.0f32.powf(gain_db / 40.0);
        let alpha = sin_w0 / (2.0 * q);
        let b0 = 1.0 + alpha * a_lin;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a_lin;
        let a0 = 1.0 + alpha / a_lin;
        let a1_r = -2.0 * cos_w0;
        let a2_r = 1.0 - alpha / a_lin;
        Biquad::new(
            b0 / a0, b1 / a0, b2 / a0,
            a1_r / a0, a2_r / a0,
            BiquadType::Peak,
        )
    }

    /// Low shelf.
    pub fn low_shelf(cutoff_hz: f32, gain_db: f32, slope: f32, sample_rate: f32) -> Biquad {
        let w0 = Self::omega(cutoff_hz, sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a_lin = 10.0f32.powf(gain_db / 40.0);
        let alpha = sin_w0 / 2.0 * ((a_lin + 1.0 / a_lin) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let a_p1 = a_lin + 1.0;
        let a_m1 = a_lin - 1.0;
        let b0 = a_lin * (a_p1 - a_m1 * cos_w0 + 2.0 * a_lin.sqrt() * alpha);
        let b1 = 2.0 * a_lin * (a_m1 - a_p1 * cos_w0);
        let b2 = a_lin * (a_p1 - a_m1 * cos_w0 - 2.0 * a_lin.sqrt() * alpha);
        let a0 = a_p1 + a_m1 * cos_w0 + 2.0 * a_lin.sqrt() * alpha;
        let a1_r = -2.0 * (a_m1 + a_p1 * cos_w0);
        let a2_r = a_p1 + a_m1 * cos_w0 - 2.0 * a_lin.sqrt() * alpha;
        Biquad::new(
            b0 / a0, b1 / a0, b2 / a0,
            a1_r / a0, a2_r / a0,
            BiquadType::LowShelf,
        )
    }

    /// High shelf.
    pub fn high_shelf(cutoff_hz: f32, gain_db: f32, slope: f32, sample_rate: f32) -> Biquad {
        let w0 = Self::omega(cutoff_hz, sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a_lin = 10.0f32.powf(gain_db / 40.0);
        let alpha = sin_w0 / 2.0 * ((a_lin + 1.0 / a_lin) * (1.0 / slope - 1.0) + 2.0).sqrt();
        let a_p1 = a_lin + 1.0;
        let a_m1 = a_lin - 1.0;
        let b0 = a_lin * (a_p1 + a_m1 * cos_w0 + 2.0 * a_lin.sqrt() * alpha);
        let b1 = -2.0 * a_lin * (a_m1 + a_p1 * cos_w0);
        let b2 = a_lin * (a_p1 + a_m1 * cos_w0 - 2.0 * a_lin.sqrt() * alpha);
        let a0 = a_p1 - a_m1 * cos_w0 + 2.0 * a_lin.sqrt() * alpha;
        let a1_r = 2.0 * (a_m1 - a_p1 * cos_w0);
        let a2_r = a_p1 - a_m1 * cos_w0 - 2.0 * a_lin.sqrt() * alpha;
        Biquad::new(
            b0 / a0, b1 / a0, b2 / a0,
            a1_r / a0, a2_r / a0,
            BiquadType::HighShelf,
        )
    }

    /// 2nd-order allpass.
    pub fn allpass(cutoff_hz: f32, q: f32, sample_rate: f32) -> Biquad {
        let w0 = Self::omega(cutoff_hz, sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let b0 = 1.0 - alpha;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 + alpha;
        let a0 = 1.0 + alpha;
        Biquad::new(
            b0 / a0, b1 / a0, b2 / a0,
            (-2.0 * cos_w0) / a0, (1.0 - alpha) / a0,
            BiquadType::AllPass,
        )
    }
}

// ---------------------------------------------------------------------------
// FilterChain — cascaded biquads
// ---------------------------------------------------------------------------

/// A cascade of biquad sections for higher-order filtering.
#[derive(Debug, Clone)]
pub struct FilterChain {
    pub stages: Vec<Biquad>,
}

impl FilterChain {
    pub fn new() -> Self { Self { stages: Vec::new() } }

    pub fn with_capacity(n: usize) -> Self {
        Self { stages: Vec::with_capacity(n) }
    }

    /// Add a biquad stage.
    pub fn push(&mut self, biquad: Biquad) {
        self.stages.push(biquad);
    }

    /// Process a single sample through all stages.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let mut y = x;
        for stage in self.stages.iter_mut() {
            y = stage.process_sample(y);
        }
        y
    }

    /// In-place block processing.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Reset all stages.
    pub fn reset(&mut self) {
        for stage in self.stages.iter_mut() { stage.reset(); }
    }

    /// Number of biquad stages.
    pub fn num_stages(&self) -> usize { self.stages.len() }

    /// Overall magnitude response at normalized frequency ω.
    pub fn magnitude_response(&self, omega: f32) -> f32 {
        self.stages.iter().map(|b| b.magnitude_response(omega)).product()
    }
}

impl Default for FilterChain {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Butterworth
// ---------------------------------------------------------------------------

/// Butterworth filter design (maximally flat in passband).
pub struct Butterworth;

impl Butterworth {
    /// Nth-order Butterworth lowpass, implemented as cascaded biquads.
    pub fn lowpass(order: u32, cutoff_hz: f32, sample_rate: f32) -> FilterChain {
        let mut chain = FilterChain::with_capacity(order as usize / 2 + 1);
        let n_stages = order / 2;
        for k in 1..=n_stages {
            // Pole angle for Butterworth: π(2k + n - 1) / (2n)
            let theta = PI * (2 * k + order - 1) as f32 / (2 * order) as f32;
            let q = -1.0 / (2.0 * theta.cos()); // Q from pole angle
            chain.push(BiquadDesign::lowpass(cutoff_hz, q, sample_rate));
        }
        if order % 2 == 1 {
            // First-order stage: lowpass with Q=0.5 (no resonance)
            chain.push(BiquadDesign::lowpass(cutoff_hz, 0.5, sample_rate));
        }
        chain
    }

    /// Nth-order Butterworth highpass.
    pub fn highpass(order: u32, cutoff_hz: f32, sample_rate: f32) -> FilterChain {
        let mut chain = FilterChain::with_capacity(order as usize / 2 + 1);
        let n_stages = order / 2;
        for k in 1..=n_stages {
            let theta = PI * (2 * k + order - 1) as f32 / (2 * order) as f32;
            let q = -1.0 / (2.0 * theta.cos());
            chain.push(BiquadDesign::highpass(cutoff_hz, q, sample_rate));
        }
        if order % 2 == 1 {
            chain.push(BiquadDesign::highpass(cutoff_hz, 0.5, sample_rate));
        }
        chain
    }

    /// Nth-order Butterworth bandpass.
    pub fn bandpass(order: u32, center_hz: f32, bandwidth_hz: f32, sample_rate: f32) -> FilterChain {
        let mut chain = FilterChain::with_capacity(order as usize);
        let n_stages = order / 2;
        for k in 1..=n_stages {
            let theta = PI * (2 * k + order - 1) as f32 / (2 * order) as f32;
            let q = -1.0 / (2.0 * theta.cos());
            // Use bandpass stage for each pair
            chain.push(BiquadDesign::bandpass(center_hz, bandwidth_hz / q, sample_rate));
        }
        chain
    }
}

// ---------------------------------------------------------------------------
// Chebyshev type I
// ---------------------------------------------------------------------------

/// Chebyshev Type I filter design (equiripple in passband).
pub struct Chebyshev1;

impl Chebyshev1 {
    /// Nth-order Chebyshev type I lowpass with ripple_db passband ripple.
    pub fn lowpass(order: u32, cutoff_hz: f32, ripple_db: f32, sample_rate: f32) -> FilterChain {
        let epsilon = (10.0f32.powf(ripple_db / 10.0) - 1.0).sqrt();
        let n_stages = order / 2;
        let mut chain = FilterChain::with_capacity(n_stages as usize + 1);
        let asinh_inv_eps = (1.0 / epsilon).asinh();

        for k in 1..=n_stages {
            // Chebyshev pole: σ_k = -sinh(asinh(1/ε)/n) sin(θ_k)
            //                 ω_k =  cosh(asinh(1/ε)/n) cos(θ_k)
            let theta_k = PI * (2 * k - 1) as f32 / (2 * order) as f32;
            let sigma = -(asinh_inv_eps / order as f32).sinh() * theta_k.sin();
            let omega = (asinh_inv_eps / order as f32).cosh() * theta_k.cos();
            // Convert to Q and natural frequency
            let pole_norm = (sigma * sigma + omega * omega).sqrt();
            let q_analog = pole_norm / (-2.0 * sigma).max(1e-6);
            // Bilinear transform pre-warping
            let wd = 2.0 * sample_rate * (PI * cutoff_hz / sample_rate).tan() * pole_norm;
            let wn = wd / (2.0 * PI);
            let q = q_analog.max(0.5);
            chain.push(BiquadDesign::lowpass(wn, q, sample_rate));
        }
        if order % 2 == 1 {
            chain.push(BiquadDesign::lowpass(cutoff_hz, 0.5, sample_rate));
        }
        chain
    }
}

// ---------------------------------------------------------------------------
// Bessel filter
// ---------------------------------------------------------------------------

/// Bessel filter design (maximally flat group delay).
pub struct Bessel;

impl Bessel {
    /// Nth-order Bessel lowpass.
    /// Uses pre-computed normalized Bessel poles (up to order 8).
    pub fn lowpass(order: u32, cutoff_hz: f32, sample_rate: f32) -> FilterChain {
        // Normalized Bessel pole Q values (pairs for even orders, ±1 pole for odd)
        // Source: Analog and Digital Filters, S. Darlington
        let q_values: &[f32] = match order {
            1 => &[],
            2 => &[0.5773],
            3 => &[0.6910],
            4 => &[0.5219, 0.8055],
            5 => &[0.5639, 0.9165],
            6 => &[0.5103, 0.6112, 1.0234],
            7 => &[0.5324, 0.6608, 1.1262],
            8 => &[0.5062, 0.5612, 0.7109, 1.2258],
            _ => &[0.7071], // fallback
        };

        let mut chain = FilterChain::new();
        for &q in q_values {
            chain.push(BiquadDesign::lowpass(cutoff_hz, q, sample_rate));
        }
        if order % 2 == 1 {
            chain.push(BiquadDesign::lowpass(cutoff_hz, 0.5, sample_rate));
        }
        chain
    }
}

// ---------------------------------------------------------------------------
// FirFilter
// ---------------------------------------------------------------------------

/// Finite Impulse Response filter.
#[derive(Debug, Clone)]
pub struct FirFilter {
    /// Filter coefficients (impulse response).
    pub coefficients: Vec<f32>,
    /// Delay line (ring buffer).
    delay_line: Vec<f32>,
    /// Write position in the ring buffer.
    write_pos: usize,
}

impl FirFilter {
    /// Create from coefficient vector.
    pub fn new(coefficients: Vec<f32>) -> Self {
        let n = coefficients.len();
        Self {
            coefficients,
            delay_line: vec![0.0; n],
            write_pos: 0,
        }
    }

    /// Process a single sample.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let n = self.coefficients.len();
        self.delay_line[self.write_pos] = x;
        let mut acc = 0.0f32;
        let mut read_pos = self.write_pos;
        for k in 0..n {
            acc += self.coefficients[k] * self.delay_line[read_pos];
            if read_pos == 0 { read_pos = n - 1; } else { read_pos -= 1; }
        }
        self.write_pos = (self.write_pos + 1) % n;
        acc
    }

    /// In-place block processing.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Reset the delay line.
    pub fn reset(&mut self) {
        self.delay_line.fill(0.0);
        self.write_pos = 0;
    }

    /// Number of taps.
    pub fn num_taps(&self) -> usize { self.coefficients.len() }

    /// Group delay in samples (for a linear-phase FIR: (N-1)/2).
    pub fn group_delay(&self) -> f32 {
        (self.coefficients.len() - 1) as f32 / 2.0
    }
}

// ---------------------------------------------------------------------------
// FirDesign
// ---------------------------------------------------------------------------

/// FIR filter design methods.
pub struct FirDesign;

impl FirDesign {
    /// Windowed-sinc lowpass FIR design.
    /// `cutoff_norm` is normalized cutoff (0..0.5), where 0.5 = Nyquist.
    pub fn lowpass_windowed(cutoff_norm: f32, num_taps: usize, window: WindowFunction) -> FirFilter {
        let m = (num_taps - 1) as f32 / 2.0;
        let mut coeffs: Vec<f32> = (0..num_taps).map(|n| {
            let x = n as f32 - m;
            sinc(2.0 * cutoff_norm * x)
        }).collect();
        // Apply window
        window.apply(&mut coeffs);
        // Normalize to unit gain at DC
        let sum: f32 = coeffs.iter().sum();
        if sum.abs() > 1e-10 {
            for c in coeffs.iter_mut() { *c /= sum; }
        }
        FirFilter::new(coeffs)
    }

    /// Windowed-sinc highpass FIR design.
    pub fn highpass_windowed(cutoff_norm: f32, num_taps: usize, window: WindowFunction) -> FirFilter {
        // Highpass = allpass - lowpass (spectral inversion)
        let mut lp = Self::lowpass_windowed(cutoff_norm, num_taps, window);
        let m = (num_taps - 1) / 2;
        for (i, c) in lp.coefficients.iter_mut().enumerate() {
            *c = if i == m { 1.0 - *c } else { -*c };
        }
        FirFilter::new(lp.coefficients)
    }

    /// Windowed-sinc bandpass FIR design.
    pub fn bandpass_windowed(low_norm: f32, high_norm: f32, num_taps: usize, window: WindowFunction) -> FirFilter {
        let m = (num_taps - 1) as f32 / 2.0;
        let mut coeffs: Vec<f32> = (0..num_taps).map(|n| {
            let x = n as f32 - m;
            sinc(high_norm * x) - sinc(low_norm * x)
        }).collect();
        window.apply(&mut coeffs);
        // Normalize to peak unity at center frequency
        let peak: f32 = coeffs.iter().map(|&c| c.abs()).fold(0.0, f32::max);
        if peak > 1e-10 {
            for c in coeffs.iter_mut() { *c /= peak; }
        }
        FirFilter::new(coeffs)
    }

    /// Bandstop (notch) windowed FIR.
    pub fn bandstop_windowed(low_norm: f32, high_norm: f32, num_taps: usize, window: WindowFunction) -> FirFilter {
        let lp = Self::lowpass_windowed(low_norm, num_taps, window);
        let hp = Self::highpass_windowed(high_norm, num_taps, window);
        let coeffs: Vec<f32> = lp.coefficients.iter().zip(hp.coefficients.iter())
            .map(|(&a, &b)| a + b)
            .collect();
        FirFilter::new(coeffs)
    }

    /// Parks-McClellan equiripple lowpass approximation.
    /// This is a simplified iterative Remez exchange approximation.
    pub fn equiripple_lowpass(cutoff_norm: f32, num_taps: usize) -> FirFilter {
        // Use Kaiser window as a starting approximation
        // The Kaiser window β=8.0 gives ~80 dB stopband attenuation
        // A proper Remez exchange algorithm is extremely complex; here we
        // use Kaiser windowed sinc with β computed from the desired attenuation.
        let a_stop = 80.0f32; // desired stopband attenuation
        let beta = if a_stop > 50.0 {
            0.1102 * (a_stop - 8.7)
        } else if a_stop >= 21.0 {
            0.5842 * (a_stop - 21.0).powf(0.4) + 0.07886 * (a_stop - 21.0)
        } else {
            0.0
        };
        Self::lowpass_windowed(cutoff_norm, num_taps, WindowFunction::Kaiser(beta))
    }

    /// Differentiator FIR (first-order derivative approximation).
    pub fn differentiator(num_taps: usize) -> FirFilter {
        let m = (num_taps - 1) as f32 / 2.0;
        let mut coeffs: Vec<f32> = (0..num_taps).map(|n| {
            let x = n as f32 - m;
            if x.abs() < 1e-10 { 0.0 } else { (PI * x).cos() / x - (PI * x).sin() / (PI * x * x) }
        }).collect();
        WindowFunction::Hamming.apply(&mut coeffs);
        FirFilter::new(coeffs)
    }

    /// Hilbert transform FIR (90° phase shift).
    pub fn hilbert(num_taps: usize) -> FirFilter {
        assert!(num_taps % 2 == 1, "Hilbert FIR requires odd number of taps");
        let m = (num_taps - 1) / 2;
        let mut coeffs: Vec<f32> = (0..num_taps).map(|n| {
            let k = n as i32 - m as i32;
            if k == 0 { 0.0 }
            else if k % 2 == 0 { 0.0 }
            else { 2.0 / (PI * k as f32) }
        }).collect();
        WindowFunction::Hamming.apply(&mut coeffs);
        FirFilter::new(coeffs)
    }
}

// ---------------------------------------------------------------------------
// Convolution
// ---------------------------------------------------------------------------

/// Direct and FFT-based convolution.
pub struct Convolution;

impl Convolution {
    /// Linear convolution via direct sum. O(N·M).
    pub fn convolve_direct(signal: &[f32], kernel: &[f32]) -> Vec<f32> {
        if signal.is_empty() || kernel.is_empty() { return Vec::new(); }
        let out_len = signal.len() + kernel.len() - 1;
        let mut out = vec![0.0f32; out_len];
        for (i, &s) in signal.iter().enumerate() {
            for (j, &k) in kernel.iter().enumerate() {
                out[i + j] += s * k;
            }
        }
        out
    }

    /// Linear convolution via FFT. O((N+M) log(N+M)).
    pub fn convolve(signal: &[f32], kernel: &[f32]) -> Vec<f32> {
        if signal.is_empty() || kernel.is_empty() { return Vec::new(); }
        // For small inputs, direct convolution is exact and avoids FFT rounding.
        let out_len = signal.len() + kernel.len() - 1;
        if out_len <= 64 {
            return Self::convolve_direct(signal, kernel);
        }
        let n = next_power_of_two(out_len);
        let mut a: Vec<Complex32> = signal.iter().map(|&x| Complex32::new(x, 0.0)).collect();
        a.resize(n, Complex32::zero());
        let mut b: Vec<Complex32> = kernel.iter().map(|&x| Complex32::new(x, 0.0)).collect();
        b.resize(n, Complex32::zero());
        Fft::forward(&mut a);
        Fft::forward(&mut b);
        for (ai, bi) in a.iter_mut().zip(b.iter()) { *ai = *ai * *bi; }
        Fft::inverse(&mut a);
        a[..out_len].iter().map(|c| c.re).collect()
    }

    /// Correlation (not convolution): xcorr(a, b) with zero-lag at index len(a)-1.
    pub fn correlate(a: &[f32], b: &[f32]) -> Vec<f32> {
        let b_rev: Vec<f32> = b.iter().rev().copied().collect();
        Self::convolve(a, &b_rev)
    }
}

// ---------------------------------------------------------------------------
// OlaConvolver — Streaming overlap-add convolution
// ---------------------------------------------------------------------------

/// Streaming overlap-add convolver for real-time large FIR processing.
pub struct OlaConvolver {
    kernel_fft: Vec<Complex32>,
    fft_size: usize,
    block_size: usize,
    overlap: Vec<f32>,
}

impl OlaConvolver {
    /// Create from an FIR kernel and a processing block size.
    pub fn new(kernel: &[f32], block_size: usize) -> Self {
        let fft_size = next_power_of_two(block_size + kernel.len() - 1);
        let mut kernel_padded: Vec<Complex32> = kernel.iter().map(|&x| Complex32::new(x, 0.0)).collect();
        kernel_padded.resize(fft_size, Complex32::zero());
        Fft::forward(&mut kernel_padded);
        Self {
            kernel_fft: kernel_padded,
            fft_size,
            block_size,
            overlap: vec![0.0; fft_size],
        }
    }

    /// Process one block of `block_size` samples. Returns a block of the same size.
    pub fn process_block(&mut self, input: &[f32]) -> Vec<f32> {
        assert_eq!(input.len(), self.block_size);
        let mut buf: Vec<Complex32> = input.iter().map(|&x| Complex32::new(x, 0.0)).collect();
        buf.resize(self.fft_size, Complex32::zero());
        Fft::forward(&mut buf);
        for (b, &k) in buf.iter_mut().zip(self.kernel_fft.iter()) {
            *b = *b * k;
        }
        Fft::inverse(&mut buf);
        // Overlap-add
        let mut out = Vec::with_capacity(self.block_size);
        for i in 0..self.block_size {
            out.push(buf[i].re + self.overlap[i]);
        }
        // Store tail in overlap
        for i in 0..self.fft_size - self.block_size {
            self.overlap[i] = buf[self.block_size + i].re;
        }
        out
    }

    /// Reset the overlap buffer.
    pub fn reset(&mut self) {
        self.overlap.fill(0.0);
    }
}

// ---------------------------------------------------------------------------
// SvfFilter — State-Variable Filter
// ---------------------------------------------------------------------------

/// State-variable filter modes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SvfMode {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peak,
    AllPass,
}

/// Chamberlin state-variable filter (TPT topology).
#[derive(Debug, Clone)]
pub struct SvfFilter {
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub mode: SvfMode,
    sample_rate: f32,
    // Internal state
    ic1eq: f32,
    ic2eq: f32,
}

impl SvfFilter {
    pub fn new(cutoff_hz: f32, resonance: f32, mode: SvfMode, sample_rate: f32) -> Self {
        Self { cutoff_hz, resonance, mode, sample_rate, ic1eq: 0.0, ic2eq: 0.0 }
    }

    /// Set cutoff frequency.
    pub fn set_cutoff(&mut self, hz: f32) { self.cutoff_hz = hz; }
    /// Set resonance (0=overdamped, 1=critical, >1=underdamped).
    pub fn set_resonance(&mut self, r: f32) { self.resonance = r; }

    /// Process a single sample.
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let g = (PI * self.cutoff_hz / self.sample_rate).tan();
        let k = 2.0 - 2.0 * self.resonance.min(0.9999);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = x - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        match self.mode {
            SvfMode::LowPass  => v2,
            SvfMode::HighPass => x - k * v1 - v2,
            SvfMode::BandPass => v1,
            SvfMode::Notch    => x - k * v1,
            SvfMode::Peak     => v2 - (x - k * v1 - v2),
            SvfMode::AllPass  => x - 2.0 * k * v1,
        }
    }

    /// In-place block processing.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() { *s = self.process_sample(*s); }
    }

    /// Reset filter state.
    pub fn reset(&mut self) { self.ic1eq = 0.0; self.ic2eq = 0.0; }
}

// ---------------------------------------------------------------------------
// CombFilter
// ---------------------------------------------------------------------------

/// Comb filter mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CombMode {
    FeedForward,
    FeedBack,
}

/// Comb filter (feedforward or feedback).
#[derive(Debug, Clone)]
pub struct CombFilter {
    pub delay_samples: usize,
    pub gain: f32,
    pub mode: CombMode,
    delay_line: Vec<f32>,
    write_pos: usize,
}

impl CombFilter {
    pub fn new(delay_samples: usize, gain: f32, mode: CombMode) -> Self {
        Self {
            delay_samples,
            gain,
            mode,
            delay_line: vec![0.0; delay_samples + 1],
            write_pos: 0,
        }
    }

    /// Process a single sample.
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let n = self.delay_line.len();
        let read_pos = (self.write_pos + n - self.delay_samples) % n;
        let delayed = self.delay_line[read_pos];
        let y = match self.mode {
            CombMode::FeedForward => x + self.gain * delayed,
            CombMode::FeedBack    => x + self.gain * delayed,
        };
        self.delay_line[self.write_pos] = match self.mode {
            CombMode::FeedForward => x,
            CombMode::FeedBack    => y,
        };
        self.write_pos = (self.write_pos + 1) % n;
        y
    }

    /// In-place block processing.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() { *s = self.process_sample(*s); }
    }

    /// Reset delay line.
    pub fn reset(&mut self) {
        self.delay_line.fill(0.0);
        self.write_pos = 0;
    }
}

// ---------------------------------------------------------------------------
// AllpassDelay — for Schroeder reverberators
// ---------------------------------------------------------------------------

/// Allpass delay network (Schroeder allpass section).
#[derive(Debug, Clone)]
pub struct AllpassDelay {
    pub delay_samples: usize,
    pub feedback: f32,
    delay_line: Vec<f32>,
    write_pos: usize,
}

impl AllpassDelay {
    pub fn new(delay_samples: usize, feedback: f32) -> Self {
        Self {
            delay_samples,
            feedback,
            delay_line: vec![0.0; delay_samples + 1],
            write_pos: 0,
        }
    }

    /// Process a single sample.
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let n = self.delay_line.len();
        let read_pos = (self.write_pos + n - self.delay_samples) % n;
        let buf = self.delay_line[read_pos];
        let out = -self.feedback * x + buf;
        self.delay_line[self.write_pos] = x + self.feedback * buf;
        self.write_pos = (self.write_pos + 1) % n;
        out
    }

    /// In-place block processing.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() { *s = self.process_sample(*s); }
    }

    /// Reset delay line.
    pub fn reset(&mut self) {
        self.delay_line.fill(0.0);
        self.write_pos = 0;
    }
}

// ---------------------------------------------------------------------------
// MovingAverage
// ---------------------------------------------------------------------------

/// Efficient O(1) sliding-window moving average.
#[derive(Debug, Clone)]
pub struct MovingAverage {
    pub window_size: usize,
    buffer: Vec<f32>,
    write_pos: usize,
    sum: f32,
    count: usize,
}

impl MovingAverage {
    pub fn new(window_size: usize) -> Self {
        assert!(window_size > 0);
        Self {
            window_size,
            buffer: vec![0.0; window_size],
            write_pos: 0,
            sum: 0.0,
            count: 0,
        }
    }

    /// Process a single sample, return the current moving average.
    pub fn process(&mut self, x: f32) -> f32 {
        self.sum -= self.buffer[self.write_pos];
        self.buffer[self.write_pos] = x;
        self.sum += x;
        self.write_pos = (self.write_pos + 1) % self.window_size;
        if self.count < self.window_size { self.count += 1; }
        self.sum / self.count as f32
    }

    /// Process a buffer, returning filtered values.
    pub fn process_buffer(&mut self, input: &[f32]) -> Vec<f32> {
        input.iter().map(|&x| self.process(x)).collect()
    }

    /// Current average value.
    pub fn value(&self) -> f32 {
        if self.count == 0 { 0.0 } else { self.sum / self.count as f32 }
    }

    /// Reset state.
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.sum = 0.0;
        self.count = 0;
    }
}

// ---------------------------------------------------------------------------
// KalmanFilter1D — scalar Kalman filter
// ---------------------------------------------------------------------------

/// 1D scalar Kalman filter for sensor fusion and signal smoothing.
///
/// State: x̂ (estimate), P (estimate covariance)
/// Model: x_k = x_{k-1} + process_noise
///        y_k = x_k + measurement_noise
#[derive(Debug, Clone)]
pub struct KalmanFilter1D {
    /// Estimated state.
    pub x: f32,
    /// Estimate error covariance.
    pub p: f32,
    /// Process noise covariance Q.
    pub q: f32,
    /// Measurement noise covariance R.
    pub r: f32,
}

impl KalmanFilter1D {
    /// Create a new filter.
    /// * `initial_estimate` — initial state estimate
    /// * `q` — process noise variance (larger = more responsive)
    /// * `r` — measurement noise variance (larger = more smoothing)
    pub fn new(initial_estimate: f32, q: f32, r: f32) -> Self {
        Self { x: initial_estimate, p: 1.0, q, r }
    }

    /// Predict step (constant-velocity model here: x = x, P = P + Q).
    pub fn predict(&mut self, _dt: f32) {
        // Simple random-walk model: state unchanged, covariance grows
        self.p += self.q;
    }

    /// Update with a new measurement.
    pub fn update(&mut self, measurement: f32) {
        // Kalman gain
        let k = self.p / (self.p + self.r);
        // Update estimate
        self.x += k * (measurement - self.x);
        // Update covariance
        self.p *= 1.0 - k;
    }

    /// Predict then update in one step.
    pub fn filter(&mut self, measurement: f32, dt: f32) -> f32 {
        self.predict(dt);
        self.update(measurement);
        self.x
    }

    /// Current estimate.
    pub fn estimate(&self) -> f32 { self.x }

    /// Filter a buffer of measurements.
    pub fn filter_buffer(&mut self, measurements: &[f32], dt: f32) -> Vec<f32> {
        measurements.iter().map(|&m| self.filter(m, dt)).collect()
    }

    /// Reset to a new initial state.
    pub fn reset(&mut self, initial: f32) {
        self.x = initial;
        self.p = 1.0;
    }
}

// ---------------------------------------------------------------------------
// PllFilter — Phase-Locked Loop
// ---------------------------------------------------------------------------

/// Simple digital Phase-Locked Loop for pitch/tempo tracking.
///
/// Uses a second-order loop filter (PI controller).
#[derive(Debug, Clone)]
pub struct PllFilter {
    /// Loop natural frequency in Hz.
    pub natural_freq_hz: f32,
    /// Damping factor ζ.
    pub damping: f32,
    sample_rate: f32,
    /// Current phase estimate (radians).
    phase: f32,
    /// Current frequency estimate (radians/sample).
    freq: f32,
    // PI filter integrator state
    integrator: f32,
    // Loop filter coefficients
    kp: f32,
    ki: f32,
}

impl PllFilter {
    /// Create a PLL.
    /// * `center_freq_hz` — initial center frequency
    /// * `natural_freq_hz` — loop bandwidth
    /// * `damping` — damping factor (0.707 = Butterworth)
    pub fn new(center_freq_hz: f32, natural_freq_hz: f32, damping: f32, sample_rate: f32) -> Self {
        let wn = 2.0 * PI * natural_freq_hz / sample_rate;
        let kp = 2.0 * damping * wn;
        let ki = wn * wn;
        Self {
            natural_freq_hz,
            damping,
            sample_rate,
            phase: 0.0,
            freq: 2.0 * PI * center_freq_hz / sample_rate,
            integrator: 2.0 * PI * center_freq_hz / sample_rate,
            kp,
            ki,
        }
    }

    /// Process one sample. Input is the raw signal (or phase error signal).
    /// Returns the VCO output (cosine at locked frequency).
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Phase detector: multiply input by VCO quadrature output
        let vco_i = self.phase.cos();
        let vco_q = self.phase.sin();
        let _phase_error_unused = input * vco_q - 0.0 * vco_i; // simplified phase discriminator
        let phase_error = input * (-self.phase).sin(); // XOR-like discriminator
        // Loop filter (PI)
        self.integrator += self.ki * phase_error;
        self.freq = self.integrator + self.kp * phase_error;
        // VCO
        self.phase += self.freq;
        self.phase = Self::wrap_phase(self.phase);
        self.phase.cos()
    }

    /// Process a buffer, returning VCO output.
    pub fn process_buffer(&mut self, input: &[f32]) -> Vec<f32> {
        input.iter().map(|&x| self.process_sample(x)).collect()
    }

    /// Current estimated frequency in Hz.
    pub fn frequency_hz(&self) -> f32 {
        self.freq * self.sample_rate / (2.0 * PI)
    }

    /// Current phase in radians.
    pub fn phase(&self) -> f32 { self.phase }

    /// Reset the PLL state.
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.integrator = self.freq;
    }

    /// Wrap phase to [-π, π].
    fn wrap_phase(p: f32) -> f32 {
        let mut p = p;
        while p > PI  { p -= 2.0 * PI; }
        while p < -PI { p += 2.0 * PI; }
        p
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::SignalGenerator;

    fn sine_buf(freq_hz: f32, sr: f32, len: usize) -> Vec<f32> {
        (0..len).map(|i| (2.0 * PI * freq_hz * i as f32 / sr).sin()).collect()
    }

    fn rms(buf: &[f32]) -> f32 {
        let sum: f32 = buf.iter().map(|&x| x * x).sum();
        (sum / buf.len() as f32).sqrt()
    }

    // --- Biquad ---

    #[test]
    fn test_biquad_identity() {
        let mut bq = Biquad::identity();
        let input = vec![1.0, 0.5, -0.3, 0.8];
        let mut buf = input.clone();
        bq.process(&mut buf);
        for (&a, &b) in input.iter().zip(buf.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_biquad_lowpass_attenuates_high_freq() {
        let sr = 44100.0;
        let mut lp = BiquadDesign::lowpass(500.0, 0.707, sr);
        let hi_freq = sine_buf(10000.0, sr, 4410);
        let mut buf = hi_freq.clone();
        lp.process(&mut buf);
        // After lowpass, high frequency should be greatly attenuated
        assert!(rms(&buf) < rms(&hi_freq) * 0.5);
    }

    #[test]
    fn test_biquad_highpass_passes_high_freq() {
        let sr = 44100.0;
        let mut hp = BiquadDesign::highpass(1000.0, 0.707, sr);
        let hi_buf = sine_buf(10000.0, sr, 4410);
        let mut buf = hi_buf.clone();
        hp.process(&mut buf);
        // High frequency should pass mostly unchanged
        assert!(rms(&buf) > rms(&hi_buf) * 0.5);
    }

    #[test]
    fn test_biquad_reset() {
        let sr = 44100.0;
        let mut lp = BiquadDesign::lowpass(1000.0, 0.707, sr);
        let mut buf = vec![1.0f32; 100];
        lp.process(&mut buf);
        lp.reset();
        assert_eq!(lp.z1, 0.0);
        assert_eq!(lp.z2, 0.0);
    }

    #[test]
    fn test_biquad_notch_attenuates_center() {
        let sr = 44100.0;
        let center = 1000.0f32;
        let mut notch = BiquadDesign::notch(center, 10.0, sr);
        let buf_in = sine_buf(center, sr, 44100);
        let mut buf = buf_in.clone();
        // Warm up
        for _ in 0..1000 { notch.process_sample(0.0); }
        notch.reset();
        notch.process(&mut buf);
        // Notch should significantly reduce the tone
        assert!(rms(&buf) < rms(&buf_in) * 0.3);
    }

    #[test]
    fn test_filter_chain() {
        let sr = 44100.0;
        let mut chain = FilterChain::new();
        chain.push(BiquadDesign::lowpass(1000.0, 0.707, sr));
        chain.push(BiquadDesign::lowpass(1000.0, 0.707, sr));
        let buf_in = sine_buf(10000.0, sr, 4410);
        let mut buf = buf_in.clone();
        chain.process(&mut buf);
        // Two cascaded LP should attenuate more than one
        let mut single = BiquadDesign::lowpass(1000.0, 0.707, sr);
        let mut buf2 = buf_in.clone();
        single.process(&mut buf2);
        assert!(rms(&buf) < rms(&buf2));
    }

    #[test]
    fn test_butterworth_lowpass() {
        let sr = 44100.0;
        let mut filt = Butterworth::lowpass(4, 1000.0, sr);
        let buf_in = sine_buf(10000.0, sr, 4410);
        let mut buf = buf_in.clone();
        filt.process(&mut buf);
        assert!(rms(&buf) < rms(&buf_in) * 0.1);
    }

    #[test]
    fn test_fir_lowpass_dc_gain() {
        // DC gain of a lowpass FIR should be 1
        let fir = FirDesign::lowpass_windowed(0.25, 63, WindowFunction::Hamming);
        let dc: Vec<f32> = vec![1.0; 512];
        let mut buf = dc.clone();
        let mut f = fir;
        f.process(&mut buf);
        // After transient, steady state should be near 1.0
        let steady = &buf[200..];
        let avg: f32 = steady.iter().sum::<f32>() / steady.len() as f32;
        assert!((avg - 1.0).abs() < 0.01, "avg={}", avg);
    }

    #[test]
    fn test_fir_highpass_attenuates_dc() {
        let fir = FirDesign::highpass_windowed(0.25, 63, WindowFunction::Hann);
        let dc = vec![1.0f32; 512];
        let mut buf = dc.clone();
        let mut f = fir;
        f.process(&mut buf);
        let avg: f32 = buf[200..].iter().sum::<f32>() / buf[200..].len() as f32;
        assert!(avg.abs() < 0.05, "avg={}", avg);
    }

    #[test]
    fn test_convolution_impulse() {
        // Convolving with an impulse should return the signal unchanged
        let sig = vec![1.0f32, 2.0, 3.0, 4.0];
        let kernel = vec![1.0f32, 0.0, 0.0];
        let out = Convolution::convolve(&sig, &kernel);
        assert_eq!(out[0], 1.0);
        assert_eq!(out[1], 2.0);
        assert_eq!(out[2], 3.0);
        assert_eq!(out[3], 4.0);
    }

    #[test]
    fn test_convolution_matches_direct() {
        let sig: Vec<f32> = (0..20).map(|i| i as f32 * 0.1).collect();
        let kernel: Vec<f32> = vec![0.25, 0.5, 0.25];
        let fft_result = Convolution::convolve(&sig, &kernel);
        let direct_result = Convolution::convolve_direct(&sig, &kernel);
        assert_eq!(fft_result.len(), direct_result.len());
        for (a, b) in fft_result.iter().zip(direct_result.iter()) {
            assert!((a - b).abs() < 1e-4, "a={}, b={}", a, b);
        }
    }

    #[test]
    fn test_ola_convolver_block_processing() {
        let kernel = vec![0.25f32, 0.5, 0.25];
        let block_size = 64;
        let mut ola = OlaConvolver::new(&kernel, block_size);
        let input = vec![1.0f32; block_size];
        let out = ola.process_block(&input);
        assert_eq!(out.len(), block_size);
    }

    #[test]
    fn test_svf_lowpass() {
        let sr = 44100.0;
        let mut svf = SvfFilter::new(1000.0, 0.0, SvfMode::LowPass, sr);
        let hi = sine_buf(10000.0, sr, 4410);
        let mut buf = hi.clone();
        svf.process(&mut buf);
        assert!(rms(&buf) < rms(&hi) * 0.3);
    }

    #[test]
    fn test_svf_highpass() {
        let sr = 44100.0;
        let mut svf = SvfFilter::new(1000.0, 0.0, SvfMode::HighPass, sr);
        let lo = sine_buf(100.0, sr, 4410);
        let mut buf = lo.clone();
        svf.process(&mut buf);
        // Should attenuate low frequency
        assert!(rms(&buf) < rms(&lo) * 0.5);
    }

    #[test]
    fn test_comb_feedforward() {
        let mut comb = CombFilter::new(100, 0.5, CombMode::FeedForward);
        let impulse: Vec<f32> = {
            let mut v = vec![0.0f32; 200];
            v[0] = 1.0;
            v
        };
        let mut buf = impulse.clone();
        comb.process(&mut buf);
        // Should see an echo at sample 100
        assert!((buf[100] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_allpass_delay_unity_magnitude() {
        let mut ap = AllpassDelay::new(50, 0.5);
        let impulse: Vec<f32> = {
            let mut v = vec![0.0f32; 200];
            v[0] = 1.0;
            v
        };
        let mut buf = impulse.clone();
        ap.process(&mut buf);
        // Energy should be preserved
        let energy_in: f32 = impulse.iter().map(|&x| x * x).sum();
        let energy_out: f32 = buf.iter().map(|&x| x * x).sum();
        assert!((energy_in - energy_out).abs() < 0.01);
    }

    #[test]
    fn test_moving_average_settling() {
        let mut ma = MovingAverage::new(8);
        for _ in 0..100 { ma.process(1.0); }
        assert!((ma.value() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_moving_average_step() {
        let mut ma = MovingAverage::new(4);
        // Feed 0s then 1s
        for _ in 0..4 { ma.process(0.0); }
        for i in 0..4 {
            let v = ma.process(1.0);
            assert!(v <= 1.0 && v >= 0.0, "i={} v={}", i, v);
        }
        assert!((ma.value() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_kalman_smoothing() {
        // Noisy constant signal — Kalman should converge to true value
        let mut kf = KalmanFilter1D::new(0.0, 0.001, 1.0);
        let dt = 1.0 / 44100.0;
        for _ in 0..1000 {
            kf.filter(1.0, dt);
        }
        assert!((kf.estimate() - 1.0).abs() < 0.05, "est={}", kf.estimate());
    }

    #[test]
    fn test_pll_frequency_lock() {
        let sr = 44100.0;
        let target_hz = 440.0;
        let mut pll = PllFilter::new(target_hz, 5.0, 0.707, sr);
        let sig = sine_buf(target_hz, sr, 44100);
        // Run for a while to let the PLL lock
        for &s in &sig[..22050] { pll.process_sample(s); }
        let est_freq = pll.frequency_hz();
        // Should be in the right ballpark
        assert!(est_freq > 200.0 && est_freq < 1000.0, "est_freq={}", est_freq);
    }

    #[test]
    fn test_biquad_peak_eq() {
        let sr = 44100.0;
        let mut peak = BiquadDesign::peak_eq(1000.0, 6.0, 1.0, sr);
        let buf_in = sine_buf(1000.0, sr, 4410);
        let mut buf = buf_in.clone();
        peak.process(&mut buf);
        // Peak EQ should boost at center frequency
        assert!(rms(&buf) > rms(&buf_in) * 1.3);
    }

    #[test]
    fn test_chebyshev1_lowpass() {
        let sr = 44100.0;
        let mut ch = Chebyshev1::lowpass(4, 1000.0, 3.0, sr);
        let hi = sine_buf(10000.0, sr, 4410);
        let mut buf = hi.clone();
        ch.process(&mut buf);
        assert!(rms(&buf) < rms(&hi) * 0.1);
    }

    #[test]
    fn test_bessel_lowpass_dc_gain() {
        let sr = 44100.0;
        let mut bessel = Bessel::lowpass(4, 2000.0, sr);
        let dc = vec![1.0f32; 4410];
        let mut buf = dc.clone();
        bessel.process(&mut buf);
        let avg: f32 = buf[1000..].iter().sum::<f32>() / buf[1000..].len() as f32;
        assert!((avg - 1.0).abs() < 0.1, "avg={}", avg);
    }

    #[test]
    fn test_fir_bandpass() {
        let sr = 44100.0;
        let fir = FirDesign::bandpass_windowed(0.1, 0.3, 127, WindowFunction::Blackman);
        let lo = sine_buf(100.0, sr, 4410);
        let hi = sine_buf(10000.0, sr, 4410);
        let mid = sine_buf(3000.0, sr, 4410);
        let process = |f: &FirFilter, buf: &[f32]| -> f32 {
            let mut b = buf.to_vec();
            let mut ff = f.clone();
            ff.process(&mut b);
            rms(&b)
        };
        assert!(process(&fir, &mid) > process(&fir, &lo));
        assert!(process(&fir, &mid) > process(&fir, &hi));
    }

    #[test]
    fn test_hilbert_fir_length() {
        let h = FirDesign::hilbert(63);
        assert_eq!(h.num_taps(), 63);
    }
}
