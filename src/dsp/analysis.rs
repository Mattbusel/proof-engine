//! Audio analysis and feature extraction — onset detection, beat tracking,
//! pitch detection, loudness metering, transient analysis, harmonics, DTW.

use std::f32::consts::PI;
use super::fft::{Spectrum, Stft, StftConfig, Autocorrelation, Complex32};
use super::{WindowFunction, next_power_of_two, linear_to_db, freq_to_midi};

// ---------------------------------------------------------------------------
// OnsetDetector
// ---------------------------------------------------------------------------

/// Detect note onsets / transients by different methods.
pub struct OnsetDetector;

impl OnsetDetector {
    /// Detect onsets using spectral flux.
    pub fn spectral_flux(signal: &[f32], sample_rate: f32, threshold: f32) -> Vec<f32> {
        let stft = Stft::new(StftConfig {
            fft_size: 1024,
            hop_size: 256,
            window: WindowFunction::Hann,
        });
        let frames = stft.analyze(signal);
        let mut onsets = Vec::new();
        for i in 1..frames.len() {
            let flux = frames[i - 1].spectral_flux(&frames[i]);
            if flux > threshold {
                let time = i as f32 * 256.0 / sample_rate;
                onsets.push(time);
            }
        }
        onsets
    }
}

/// Spectral flux onset detector with adaptive threshold.
pub struct SpectralFluxOnset {
    /// Threshold multiplier above median.
    pub threshold_factor: f32,
    /// STFT hop size in samples.
    pub hop_size: usize,
    /// STFT FFT size.
    pub fft_size: usize,
}

impl SpectralFluxOnset {
    pub fn new(threshold_factor: f32, fft_size: usize, hop_size: usize) -> Self {
        Self { threshold_factor, hop_size, fft_size }
    }

    /// Detect onsets in `signal`. Returns onset times in seconds.
    pub fn detect(&self, signal: &[f32], sample_rate: f32) -> Vec<f32> {
        let stft = Stft::new(StftConfig {
            fft_size: self.fft_size,
            hop_size: self.hop_size,
            window: WindowFunction::Hann,
        });
        let frames = stft.analyze(signal);
        if frames.len() < 2 { return Vec::new(); }

        // Compute frame-to-frame spectral flux
        let mut flux: Vec<f32> = vec![0.0];
        for i in 1..frames.len() {
            flux.push(frames[i - 1].spectral_flux(&frames[i]));
        }

        // Adaptive threshold: local median * factor
        let window = 8usize;
        let threshold: Vec<f32> = flux.iter().enumerate().map(|(i, _)| {
            let lo = i.saturating_sub(window);
            let hi = (i + window + 1).min(flux.len());
            let mut local: Vec<f32> = flux[lo..hi].to_vec();
            local.sort_by(|a, b| a.partial_cmp(b).unwrap());
            local[local.len() / 2] * self.threshold_factor
        }).collect();

        // Pick peaks above threshold
        let mut onsets = Vec::new();
        for i in 1..flux.len().saturating_sub(1) {
            if flux[i] > threshold[i]
                && flux[i] > flux[i - 1]
                && flux[i] > flux.get(i + 1).copied().unwrap_or(0.0)
            {
                onsets.push(i as f32 * self.hop_size as f32 / sample_rate);
            }
        }
        onsets
    }
}

/// High-Frequency Content onset detector.
pub struct HfcOnset {
    pub threshold: f32,
    pub fft_size: usize,
    pub hop_size: usize,
}

impl HfcOnset {
    pub fn new(threshold: f32, fft_size: usize, hop_size: usize) -> Self {
        Self { threshold, fft_size, hop_size }
    }

    /// Compute HFC measure for each frame.
    pub fn detect(&self, signal: &[f32], sample_rate: f32) -> Vec<f32> {
        let stft = Stft::new(StftConfig {
            fft_size: self.fft_size,
            hop_size: self.hop_size,
            window: WindowFunction::Hann,
        });
        let frames = stft.analyze(signal);
        let mut hfc_curve: Vec<f32> = frames.iter().map(|f| {
            f.bins.iter().enumerate().map(|(k, c)| {
                (k as f32) * c.norm_sq()
            }).sum::<f32>()
        }).collect();

        // Normalize HFC curve
        let max_hfc = hfc_curve.iter().cloned().fold(0.0f32, f32::max);
        if max_hfc > 1e-10 {
            for h in hfc_curve.iter_mut() { *h /= max_hfc; }
        }

        // Detect peaks
        let mut onsets = Vec::new();
        for i in 1..hfc_curve.len().saturating_sub(1) {
            let prev = hfc_curve[i - 1];
            let curr = hfc_curve[i];
            let next = hfc_curve.get(i + 1).copied().unwrap_or(0.0);
            if curr > self.threshold && curr > prev && curr > next {
                onsets.push(i as f32 * self.hop_size as f32 / sample_rate);
            }
        }
        onsets
    }
}

/// Complex-domain onset detector (phase deviation method).
pub struct ComplexDomainOnset {
    pub threshold: f32,
    pub fft_size: usize,
    pub hop_size: usize,
}

impl ComplexDomainOnset {
    pub fn new(threshold: f32, fft_size: usize, hop_size: usize) -> Self {
        Self { threshold, fft_size, hop_size }
    }

    /// Detect onsets using complex domain measure.
    pub fn detect(&self, signal: &[f32], sample_rate: f32) -> Vec<f32> {
        let stft = Stft::new(StftConfig {
            fft_size: self.fft_size,
            hop_size: self.hop_size,
            window: WindowFunction::Hann,
        });
        let frames = stft.analyze(signal);
        if frames.len() < 3 { return Vec::new(); }

        // Complex domain detection function
        let mut cd: Vec<f32> = vec![0.0, 0.0];
        for t in 2..frames.len() {
            let frame_t   = &frames[t];
            let frame_t1  = &frames[t - 1];
            let frame_t2  = &frames[t - 2];
            let n_bins = frame_t.num_bins();
            let mut sum = 0.0f32;
            for k in 0..n_bins {
                let mag    = frame_t.magnitude(k);
                let mag_t1 = frame_t1.magnitude(k);
                let ph     = frame_t.phase(k);
                let ph_t1  = frame_t1.phase(k);
                let ph_t2  = frame_t2.phase(k);
                // Phase prediction
                let predicted_ph = 2.0 * ph_t1 - ph_t2;
                // Complex target from predicted phase and previous magnitude
                let target_re = mag_t1 * predicted_ph.cos();
                let target_im = mag_t1 * predicted_ph.sin();
                let actual_re = mag * ph.cos();
                let actual_im = mag * ph.sin();
                let diff_re = actual_re - target_re;
                let diff_im = actual_im - target_im;
                sum += (diff_re * diff_re + diff_im * diff_im).sqrt();
            }
            cd.push(sum / n_bins as f32);
        }

        let max_cd = cd.iter().cloned().fold(0.0f32, f32::max);
        if max_cd > 1e-10 {
            for v in cd.iter_mut() { *v /= max_cd; }
        }

        let mut onsets = Vec::new();
        for i in 1..cd.len().saturating_sub(1) {
            if cd[i] > self.threshold && cd[i] > cd[i - 1] && cd[i] > cd.get(i + 1).copied().unwrap_or(0.0) {
                onsets.push(i as f32 * self.hop_size as f32 / sample_rate);
            }
        }
        onsets
    }
}

// ---------------------------------------------------------------------------
// BeatTracker
// ---------------------------------------------------------------------------

/// Tempo and beat detection via autocorrelation of onset envelope.
pub struct BeatTracker;

impl BeatTracker {
    /// Estimate tempo (BPM) from a signal.
    pub fn estimate_tempo(signal: &[f32], sample_rate: f32) -> f32 {
        let onset_env = Self::onset_envelope(signal, sample_rate);
        let autocorr = Autocorrelation::compute_fft(&onset_env);
        // The hop size used in onset_envelope is 512
        let hop_size = 512usize;
        let env_sr = sample_rate / hop_size as f32;
        // Search for tempo between 60 and 200 BPM
        let min_period = (60.0 * env_sr / 200.0) as usize;
        let max_period = (60.0 * env_sr / 60.0) as usize;
        let max_period = max_period.min(autocorr.len() - 1);
        if min_period >= max_period { return 120.0; }
        let (best_lag, _) = autocorr[min_period..=max_period].iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap_or((0, &0.0));
        let period_frames = best_lag + min_period;
        if period_frames == 0 { return 120.0; }
        60.0 * env_sr / period_frames as f32
    }

    /// Detect beat times in seconds.
    pub fn detect_beats(signal: &[f32], sample_rate: f32) -> Vec<f32> {
        let onset_env = Self::onset_envelope(signal, sample_rate);
        let hop_size = 512usize;
        let env_sr = sample_rate / hop_size as f32;
        let bpm = Self::estimate_tempo(signal, sample_rate).max(60.0).min(200.0);
        let beat_period = 60.0 / bpm * env_sr;
        if beat_period < 1.0 { return Vec::new(); }

        // Find initial beat position (first strong onset near expected period)
        let mut beats = Vec::new();
        let mut phase = Self::find_initial_beat_phase(&onset_env, beat_period as usize);
        while phase < onset_env.len() {
            beats.push(phase as f32 * hop_size as f32 / sample_rate);
            phase += beat_period as usize;
        }
        beats
    }

    /// Compute an onset strength envelope using spectral flux.
    pub fn onset_envelope(signal: &[f32], _sample_rate: f32) -> Vec<f32> {
        let hop_size = 512usize;
        let fft_size = 2048usize;
        let stft = Stft::new(StftConfig {
            fft_size,
            hop_size,
            window: WindowFunction::Hann,
        });
        let frames = stft.analyze(signal);
        let mut env = vec![0.0f32; frames.len()];
        for i in 1..frames.len() {
            env[i] = frames[i - 1].spectral_flux(&frames[i]);
        }
        // Half-wave rectify
        for v in env.iter_mut() { if *v < 0.0 { *v = 0.0; } }
        // Low-pass smooth
        let alpha = 0.9f32;
        let mut prev = 0.0f32;
        for v in env.iter_mut() {
            *v = (1.0 - alpha) * *v + alpha * prev;
            prev = *v;
        }
        env
    }

    fn find_initial_beat_phase(env: &[f32], period: usize) -> usize {
        if period == 0 || env.is_empty() { return 0; }
        // Find offset that maximizes sum of env at that phase
        let max_offset = period.min(env.len());
        let (best_offset, _) = (0..max_offset).map(|offset| {
            let mut sum = 0.0f32;
            let mut t = offset;
            while t < env.len() {
                sum += env[t];
                t += period;
            }
            (offset, sum)
        }).max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
          .unwrap_or((0, 0.0));
        best_offset
    }

    /// Compute tempogram (autocorrelation of onset envelope at multiple time scales).
    pub fn tempogram(signal: &[f32], sample_rate: f32) -> Vec<(f32, f32)> {
        let env = Self::onset_envelope(signal, sample_rate);
        let hop_size = 512usize;
        let env_sr = sample_rate / hop_size as f32;
        let autocorr = Autocorrelation::compute_fft(&env);
        let mut tg = Vec::new();
        for lag in 1..autocorr.len() {
            let bpm = 60.0 * env_sr / lag as f32;
            if bpm >= 40.0 && bpm <= 300.0 {
                tg.push((bpm, autocorr[lag]));
            }
        }
        tg.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap()); // sorted by strength
        tg
    }
}

// ---------------------------------------------------------------------------
// PitchDetector
// ---------------------------------------------------------------------------

/// Monophonic pitch detection.
pub struct PitchDetector;

impl PitchDetector {
    /// YIN algorithm pitch detection.
    pub fn yin(signal: &[f32], sample_rate: f32, min_hz: f32, max_hz: f32) -> Option<f32> {
        let freq = Autocorrelation::pitch_yin(signal, sample_rate)?;
        if freq >= min_hz && freq <= max_hz { Some(freq) } else { None }
    }

    /// Autocorrelation-based pitch detection.
    pub fn autocorr(signal: &[f32], sample_rate: f32) -> Option<f32> {
        let n = signal.len();
        if n < 2 { return None; }
        let ac = Autocorrelation::compute_fft(signal);
        // Find the first minimum after the first peak, then the first peak after that
        let mut found_first_min = false;
        let mut first_min_idx = 0usize;
        for i in 1..ac.len().saturating_sub(1) {
            if !found_first_min {
                if ac[i] < ac[i - 1] {
                    found_first_min = true;
                    first_min_idx = i;
                }
            }
        }
        if !found_first_min { return None; }
        let (peak_idx, _) = ac[first_min_idx..].iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())?;
        let tau = peak_idx + first_min_idx;
        if tau == 0 { return None; }
        let freq = sample_rate / tau as f32;
        if freq < 20.0 || freq > 20000.0 { return None; }
        Some(freq)
    }

    /// Harmonic Product Spectrum pitch detection.
    pub fn harmonic_product_spectrum(spectrum: &[f32], sample_rate: f32) -> f32 {
        let n = spectrum.len();
        let n_harmonics = 5usize;
        let usable = n / n_harmonics;
        let mut hps: Vec<f32> = spectrum[..usable].to_vec();
        for h in 2..=n_harmonics {
            for k in 0..usable {
                let idx = k * h;
                if idx < n {
                    hps[k] *= spectrum[idx];
                }
            }
        }
        let (peak_k, _) = hps.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap_or((0, &0.0));
        peak_k as f32 * sample_rate / (2 * n) as f32
    }

    /// Convert frequency in Hz to MIDI note number (A4=69=440Hz).
    pub fn midi_note(freq: f32) -> f32 {
        freq_to_midi(freq)
    }

    /// Convert MIDI note to a human-readable name like "A4" or "C#3".
    pub fn note_name(midi: f32) -> String {
        let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let midi_i = midi.round() as i32;
        let octave = midi_i / 12 - 1;
        let pitch_class = ((midi_i % 12) + 12) as usize % 12;
        format!("{}{}", names[pitch_class], octave)
    }

    /// Detect pitch from a signal using STFT + HPS on each frame, returning median.
    pub fn detect_from_signal(signal: &[f32], sample_rate: f32) -> Option<f32> {
        let fft_size = 2048usize;
        let mut buf = signal.to_vec();
        buf.resize(next_power_of_two(signal.len().max(fft_size)), 0.0);
        WindowFunction::Hann.apply(&mut buf[..fft_size]);
        let spectrum = Spectrum::from_real(&buf[..fft_size]);
        let mag_spec = spectrum.magnitude_spectrum();
        let freq = Self::harmonic_product_spectrum(&mag_spec, sample_rate);
        if freq > 20.0 && freq < 20000.0 { Some(freq) } else { None }
    }

    /// Estimate pitch confidence (clarity) for a given frequency from a spectrum.
    pub fn pitch_confidence(spectrum: &Spectrum, fundamental_hz: f32, sample_rate: f32) -> f32 {
        let n_bins = spectrum.num_bins();
        let bin_res = sample_rate / spectrum.fft_size as f32;
        // Look for energy at harmonic bins
        let n_harmonics = 8;
        let mut harmonic_energy = 0.0f32;
        let total_energy: f32 = spectrum.bins.iter().map(|c| c.norm_sq()).sum();
        for h in 1..=n_harmonics {
            let target_bin = (fundamental_hz * h as f32 / bin_res).round() as usize;
            let search_range = 2usize;
            for k in target_bin.saturating_sub(search_range)..=(target_bin + search_range).min(n_bins - 1) {
                harmonic_energy += spectrum.power(k);
            }
        }
        if total_energy < 1e-10 { 0.0 } else { harmonic_energy / total_energy }
    }
}

// ---------------------------------------------------------------------------
// LoudnessMeters
// ---------------------------------------------------------------------------

pub struct LoudnessMeters;

/// Simple RMS meter with a sliding window.
#[derive(Debug, Clone)]
pub struct Rms {
    /// Window length in ms.
    pub window_ms: f32,
    sample_rate: f32,
    buffer: Vec<f32>,
    sum_sq: f32,
    write_pos: usize,
    count: usize,
}

impl Rms {
    pub fn new(window_ms: f32, sample_rate: f32) -> Self {
        let len = ((window_ms / 1000.0) * sample_rate).round() as usize;
        let len = len.max(1);
        Self {
            window_ms,
            sample_rate,
            buffer: vec![0.0; len],
            sum_sq: 0.0,
            write_pos: 0,
            count: 0,
        }
    }

    /// Feed a sample, return current RMS.
    pub fn process(&mut self, x: f32) -> f32 {
        let old = self.buffer[self.write_pos];
        self.sum_sq -= old * old;
        self.sum_sq = self.sum_sq.max(0.0); // guard against float drift
        self.buffer[self.write_pos] = x;
        self.sum_sq += x * x;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        if self.count < self.buffer.len() { self.count += 1; }
        (self.sum_sq / self.count as f32).sqrt()
    }

    /// Process a buffer, returning RMS for each sample.
    pub fn process_buffer(&mut self, buf: &[f32]) -> Vec<f32> {
        buf.iter().map(|&x| self.process(x)).collect()
    }

    /// Current RMS level.
    pub fn level(&self) -> f32 {
        if self.count == 0 { 0.0 } else { (self.sum_sq / self.count as f32).sqrt() }
    }

    /// Current level in dBFS.
    pub fn level_db(&self) -> f32 { linear_to_db(self.level()) }

    /// Reset state.
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.sum_sq = 0.0;
        self.write_pos = 0;
        self.count = 0;
    }
}

/// Equivalent continuous loudness (Leq) — time-averaged RMS in dB.
#[derive(Debug, Clone)]
pub struct Leq {
    sum_sq: f64,
    count: u64,
}

impl Leq {
    pub fn new() -> Self { Self { sum_sq: 0.0, count: 0 } }

    /// Add a block of samples.
    pub fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            self.sum_sq += (s as f64) * (s as f64);
            self.count += 1;
        }
    }

    /// Current Leq in dBFS.
    pub fn leq_db(&self) -> f32 {
        if self.count == 0 { return -f32::INFINITY; }
        let rms = (self.sum_sq / self.count as f64).sqrt() as f32;
        linear_to_db(rms)
    }

    /// Reset.
    pub fn reset(&mut self) { self.sum_sq = 0.0; self.count = 0; }
}

impl Default for Leq { fn default() -> Self { Self::new() } }

/// ITU-R BS.1770-4 integrated loudness (simplified K-weighting).
/// Full implementation would require a true K-weighting filter; here we
/// apply a simplified pre-filter and compute gated loudness.
#[derive(Debug, Clone)]
pub struct Lufs {
    sample_rate: f32,
    // Accumulated gated blocks
    gated_blocks: Vec<f32>,
    // Current 400ms block accumulator
    block_samples: Vec<f32>,
    block_size: usize,
    // K-weighting filter state (simplified 2-stage biquad)
    // Stage 1: high-shelf pre-filter
    z1_1: f32,
    z2_1: f32,
    // Stage 2: highpass
    z1_2: f32,
    z2_2: f32,
}

impl Lufs {
    pub fn new(sample_rate: f32) -> Self {
        let block_size = (0.4 * sample_rate) as usize; // 400ms blocks
        Self {
            sample_rate,
            gated_blocks: Vec::new(),
            block_samples: Vec::with_capacity(block_size),
            block_size,
            z1_1: 0.0, z2_1: 0.0, z1_2: 0.0, z2_2: 0.0,
        }
    }

    /// K-weighting pre-filter coefficients (simplified, from ITU-R BS.1770).
    fn kw_coeffs(sample_rate: f32) -> (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32) {
        // Stage 1: high-shelf (pre-filter)
        let f0 = 1681.974450955533;
        let g  = 3.999843853973347;
        let q1 = 0.7071752369554196;
        let k1 = (PI * f0 / sample_rate).tan();
        let a_lin = 10.0f32.powf(g / 40.0);
        let b0_1 = a_lin * ((a_lin / q1) * k1 + k1 * k1 + 1.0) / (k1 * k1 + (1.0 / q1) * k1 + 1.0);
        let b1_1 = 2.0 * a_lin * (k1 * k1 - 1.0) / (k1 * k1 + (1.0 / q1) * k1 + 1.0);
        let b2_1 = a_lin * (k1 * k1 - (a_lin / q1) * k1 + 1.0) / (k1 * k1 + (1.0 / q1) * k1 + 1.0);
        let a1_1 = 2.0 * (k1 * k1 - 1.0) / (k1 * k1 + (1.0 / q1) * k1 + 1.0);
        let a2_1 = (k1 * k1 - (1.0 / q1) * k1 + 1.0) / (k1 * k1 + (1.0 / q1) * k1 + 1.0);
        // Stage 2: highpass at 38 Hz
        let f2 = 38.13547087602444;
        let q2 = 0.5003270373238773;
        let k2 = (PI * f2 / sample_rate).tan();
        let b0_2 = 1.0 / (1.0 + k2 / q2 + k2 * k2);
        let b1_2 = -2.0 * b0_2;
        let b2_2 = b0_2;
        let a1_2 = 2.0 * (k2 * k2 - 1.0) * b0_2;
        let a2_2 = (1.0 - k2 / q2 + k2 * k2) * b0_2;
        (b0_1, b1_1, b2_1, a1_1, a2_1, b0_2, b1_2, b2_2, a1_2, a2_2)
    }

    /// Apply K-weighting filter to one sample.
    fn k_weight(&mut self, x: f32) -> f32 {
        let (b0_1, b1_1, b2_1, a1_1, a2_1, b0_2, b1_2, b2_2, a1_2, a2_2) =
            Self::kw_coeffs(self.sample_rate);
        // Stage 1 (DF2T)
        let y1 = b0_1 * x + self.z1_1;
        self.z1_1 = b1_1 * x - a1_1 * y1 + self.z2_1;
        self.z2_1 = b2_1 * x - a2_1 * y1;
        // Stage 2 (DF2T)
        let y2 = b0_2 * y1 + self.z1_2;
        self.z1_2 = b1_2 * y1 - a1_2 * y2 + self.z2_2;
        self.z2_2 = b2_2 * y1 - a2_2 * y2;
        y2
    }

    /// Push a block of samples.
    pub fn push(&mut self, samples: &[f32]) {
        for &s in samples {
            let kw = self.k_weight(s);
            self.block_samples.push(kw);
            if self.block_samples.len() >= self.block_size {
                let power: f32 = self.block_samples.iter().map(|&x| x * x).sum::<f32>()
                    / self.block_size as f32;
                self.gated_blocks.push(power);
                self.block_samples.clear();
            }
        }
    }

    /// Compute integrated loudness with -70 LUFS absolute gate.
    pub fn integrated_lufs(&self) -> f32 {
        if self.gated_blocks.is_empty() { return -f32::INFINITY; }
        // Absolute gate: -70 LUFS = 10^((-70 + 0.691) / 10) relative power
        let absolute_gate = 10.0f32.powf((-70.0 + 0.691) / 10.0);
        let ungated: Vec<f32> = self.gated_blocks.iter().copied()
            .filter(|&p| p > absolute_gate)
            .collect();
        if ungated.is_empty() { return -f32::INFINITY; }
        // Relative gate: -10 LU below the ungated mean
        let ungated_mean: f32 = ungated.iter().sum::<f32>() / ungated.len() as f32;
        let relative_gate = ungated_mean * 10.0f32.powf(-10.0 / 10.0);
        let gated: Vec<f32> = ungated.iter().copied()
            .filter(|&p| p > relative_gate)
            .collect();
        if gated.is_empty() { return -f32::INFINITY; }
        let mean: f32 = gated.iter().sum::<f32>() / gated.len() as f32;
        -0.691 + 10.0 * mean.log10()
    }

    /// Reset state.
    pub fn reset(&mut self) {
        self.gated_blocks.clear();
        self.block_samples.clear();
        self.z1_1 = 0.0; self.z2_1 = 0.0;
        self.z1_2 = 0.0; self.z2_2 = 0.0;
    }
}

/// Dynamic range analysis: crest factor, dynamic range in dB.
#[derive(Debug, Clone)]
pub struct DynamicRange;

impl DynamicRange {
    /// Crest factor: peak / RMS (linear).
    pub fn crest_factor(signal: &[f32]) -> f32 {
        if signal.is_empty() { return 0.0; }
        let peak = signal.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        let rms = (signal.iter().map(|&x| x * x).sum::<f32>() / signal.len() as f32).sqrt();
        if rms < 1e-10 { 0.0 } else { peak / rms }
    }

    /// Crest factor in dB.
    pub fn crest_factor_db(signal: &[f32]) -> f32 {
        let cf = Self::crest_factor(signal);
        if cf <= 0.0 { return -f32::INFINITY; }
        20.0 * cf.log10()
    }

    /// Dynamic range: difference between peak and low-percentile RMS level (dB).
    pub fn dynamic_range_db(signal: &[f32]) -> f32 {
        if signal.is_empty() { return 0.0; }
        let peak_db = {
            let peak = signal.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
            linear_to_db(peak)
        };
        let rms_db = {
            let rms = (signal.iter().map(|&x| x * x).sum::<f32>() / signal.len() as f32).sqrt();
            linear_to_db(rms)
        };
        (peak_db - rms_db).max(0.0)
    }

    /// Loudness range (LRA) approximation: 95th minus 10th percentile of short-term loudness.
    pub fn loudness_range(signal: &[f32], sample_rate: f32) -> f32 {
        let block_size = (0.3 * sample_rate) as usize;
        if block_size == 0 || signal.len() < block_size { return 0.0; }
        let mut levels: Vec<f32> = signal.chunks(block_size).map(|block| {
            let rms = (block.iter().map(|&x| x * x).sum::<f32>() / block.len() as f32).sqrt();
            linear_to_db(rms)
        }).filter(|&db| db > -70.0)
          .collect();
        if levels.len() < 3 { return 0.0; }
        levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let lo = levels[(levels.len() as f32 * 0.10) as usize];
        let hi = levels[(levels.len() as f32 * 0.95) as usize];
        (hi - lo).max(0.0)
    }
}

// ---------------------------------------------------------------------------
// TransientAnalysis
// ---------------------------------------------------------------------------

/// Transient (attack/release) analysis.
pub struct TransientAnalysis;

impl TransientAnalysis {
    /// Attack time: from signal start to 90% of peak, in seconds.
    pub fn attack_time(signal: &[f32], sample_rate: f32) -> f32 {
        if signal.is_empty() { return 0.0; }
        let peak = signal.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        if peak < 1e-10 { return 0.0; }
        let target = peak * 0.9;
        for (i, &s) in signal.iter().enumerate() {
            if s.abs() >= target {
                return i as f32 / sample_rate;
            }
        }
        signal.len() as f32 / sample_rate
    }

    /// Release time: from peak to 10% of peak, in seconds.
    pub fn release_time(signal: &[f32], sample_rate: f32) -> f32 {
        if signal.is_empty() { return 0.0; }
        let peak = signal.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        if peak < 1e-10 { return 0.0; }
        let target = peak * 0.1;
        // Find peak position
        let peak_idx = signal.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        // Find first sample after peak below target
        for i in peak_idx..signal.len() {
            if signal[i].abs() <= target {
                return (i - peak_idx) as f32 / sample_rate;
            }
        }
        (signal.len() - peak_idx) as f32 / sample_rate
    }

    /// Ratio of transient energy (first 10%) to steady-state energy (remaining 90%).
    pub fn transient_to_steady_ratio(signal: &[f32]) -> f32 {
        if signal.len() < 10 { return 0.0; }
        let transient_end = signal.len() / 10;
        let transient_energy: f32 = signal[..transient_end].iter().map(|&x| x * x).sum();
        let steady_energy: f32 = signal[transient_end..].iter().map(|&x| x * x).sum();
        if steady_energy < 1e-15 { return f32::INFINITY; }
        transient_energy / steady_energy
    }

    /// Detect all transients (fast energy increases) in a signal.
    /// Returns transient times in seconds.
    pub fn detect_transients(signal: &[f32], sample_rate: f32, sensitivity: f32) -> Vec<f32> {
        let block_size = 256usize;
        let mut prev_energy = 0.0f32;
        let mut transients = Vec::new();
        for (i, chunk) in signal.chunks(block_size).enumerate() {
            let energy: f32 = chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32;
            if energy > prev_energy * (1.0 + sensitivity) && energy > 1e-6 {
                transients.push(i as f32 * block_size as f32 / sample_rate);
            }
            prev_energy = energy;
        }
        transients
    }

    /// Compute instantaneous energy envelope (short-term RMS in dB).
    pub fn energy_envelope(signal: &[f32], sample_rate: f32, window_ms: f32) -> Vec<f32> {
        let win_samples = ((window_ms / 1000.0) * sample_rate) as usize;
        let win_samples = win_samples.max(1);
        let step = win_samples / 2;
        let mut env = Vec::new();
        let mut i = 0;
        while i + win_samples <= signal.len() {
            let energy: f32 = signal[i..i + win_samples].iter().map(|&x| x * x).sum::<f32>()
                / win_samples as f32;
            env.push(linear_to_db(energy.sqrt()));
            i += step;
        }
        env
    }
}

// ---------------------------------------------------------------------------
// HarmonicAnalyzer
// ---------------------------------------------------------------------------

/// Harmonic analysis: fundamentals, overtones, THD.
pub struct HarmonicAnalyzer;

impl HarmonicAnalyzer {
    /// Detect the fundamental frequency from a spectrum.
    pub fn fundamental(spectrum: &Spectrum, sample_rate: f32) -> f32 {
        spectrum.dominant_frequency(sample_rate)
    }

    /// Get amplitudes of the first `n` harmonics of `fundamental_hz`.
    pub fn harmonics(spectrum: &Spectrum, fundamental_hz: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        let bin_res = sample_rate / spectrum.fft_size as f32;
        let mut result = Vec::with_capacity(n);
        for h in 1..=n {
            let target_hz = fundamental_hz * h as f32;
            let target_bin = (target_hz / bin_res).round() as usize;
            // Find peak in a small window around the target bin
            let search = 3usize;
            let lo = target_bin.saturating_sub(search);
            let hi = (target_bin + search + 1).min(spectrum.num_bins());
            let peak_mag = if lo < hi {
                spectrum.bins[lo..hi].iter().map(|c| c.norm()).fold(0.0f32, f32::max)
            } else {
                0.0
            };
            result.push(peak_mag);
        }
        result
    }

    /// Total Harmonic Distortion: THD = sqrt(sum of h2..hN squared) / h1.
    pub fn thd(harmonics: &[f32]) -> f32 {
        if harmonics.len() < 2 || harmonics[0] < 1e-10 { return 0.0; }
        let fundamental = harmonics[0];
        let harmonic_sum_sq: f32 = harmonics[1..].iter().map(|&h| h * h).sum();
        (harmonic_sum_sq.sqrt() / fundamental).min(100.0)
    }

    /// Total Harmonic Distortion in percent.
    pub fn thd_percent(harmonics: &[f32]) -> f32 {
        Self::thd(harmonics) * 100.0
    }

    /// Inharmonicity: deviation of harmonic frequencies from ideal integer multiples.
    pub fn inharmonicity(spectrum: &Spectrum, fundamental_hz: f32, sample_rate: f32, n: usize) -> f32 {
        let bin_res = sample_rate / spectrum.fft_size as f32;
        if fundamental_hz < 1.0 { return 0.0; }
        let mut total_deviation = 0.0f32;
        let mut count = 0usize;
        for h in 2..=n {
            let ideal_hz = fundamental_hz * h as f32;
            let ideal_bin = (ideal_hz / bin_res).round() as usize;
            let search = 5usize;
            let lo = ideal_bin.saturating_sub(search);
            let hi = (ideal_bin + search + 1).min(spectrum.num_bins());
            if lo >= hi { continue; }
            // Find actual peak bin
            let (peak_k, _) = spectrum.bins[lo..hi].iter().enumerate()
                .max_by(|(_, a), (_, b)| a.norm().partial_cmp(&b.norm()).unwrap())
                .map(|(k, c)| (k + lo, c))
                .unwrap_or((ideal_bin, &Complex32::zero()));
            let actual_hz = spectrum.frequency_of_bin(peak_k, sample_rate);
            let deviation = ((actual_hz - ideal_hz) / fundamental_hz).abs();
            total_deviation += deviation;
            count += 1;
        }
        if count == 0 { 0.0 } else { total_deviation / count as f32 }
    }

    /// Spectral centroid as a timbre measure.
    pub fn spectral_centroid(spectrum: &Spectrum, sample_rate: f32) -> f32 {
        spectrum.spectral_centroid(sample_rate)
    }

    /// Odd/even harmonic ratio (timbre indicator).
    pub fn odd_even_ratio(harmonics: &[f32]) -> f32 {
        if harmonics.len() < 2 { return 1.0; }
        let odd: f32 = harmonics.iter().step_by(2).map(|&h| h * h).sum::<f32>().sqrt();
        let even: f32 = harmonics.iter().skip(1).step_by(2).map(|&h| h * h).sum::<f32>().sqrt();
        if even < 1e-10 { f32::INFINITY } else { odd / even }
    }
}

// ---------------------------------------------------------------------------
// Correlogram
// ---------------------------------------------------------------------------

/// Running autocorrelation matrix: each row is the autocorrelation of a frame.
pub struct Correlogram {
    frames: Vec<Vec<f32>>,
    max_lag: usize,
}

impl Correlogram {
    pub fn new(max_lag: usize) -> Self {
        Self { frames: Vec::new(), max_lag }
    }

    /// Push a signal frame and compute its autocorrelation.
    pub fn push_frame(&mut self, frame: &[f32]) {
        let ac = Autocorrelation::compute(frame);
        let row: Vec<f32> = ac[..ac.len().min(self.max_lag + 1)].to_vec();
        self.frames.push(row);
    }

    /// Get the autocorrelation for frame `t`.
    pub fn get_frame(&self, t: usize) -> Option<&Vec<f32>> {
        self.frames.get(t)
    }

    /// Number of frames.
    pub fn num_frames(&self) -> usize { self.frames.len() }

    /// Max lag.
    pub fn max_lag(&self) -> usize { self.max_lag }

    /// Average autocorrelation across all frames (pitch salience curve).
    pub fn average(&self) -> Vec<f32> {
        if self.frames.is_empty() { return Vec::new(); }
        let n_lags = self.frames[0].len();
        let mut avg = vec![0.0f32; n_lags];
        for frame in &self.frames {
            for (lag, &v) in frame.iter().enumerate().take(n_lags) {
                avg[lag] += v;
            }
        }
        let n = self.frames.len() as f32;
        for a in avg.iter_mut() { *a /= n; }
        avg
    }

    /// Clear all frames.
    pub fn clear(&mut self) { self.frames.clear(); }
}

// ---------------------------------------------------------------------------
// DynamicsAnalyzer
// ---------------------------------------------------------------------------

/// Amplitude distribution and dynamics analysis.
pub struct DynamicsAnalyzer;

impl DynamicsAnalyzer {
    /// Compute an amplitude histogram over `bins` equal-width bins in [-1, 1].
    pub fn histogram(signal: &[f32], bins: usize) -> Vec<f32> {
        let mut hist = vec![0u32; bins];
        for &s in signal {
            let clamped = s.max(-1.0).min(1.0);
            let idx = ((clamped + 1.0) / 2.0 * bins as f32) as usize;
            let idx = idx.min(bins - 1);
            hist[idx] += 1;
        }
        let total = signal.len().max(1) as f32;
        hist.iter().map(|&c| c as f32 / total).collect()
    }

    /// Compute the Nth percentile amplitude (0..100).
    pub fn percentile(signal: &[f32], p: f32) -> f32 {
        if signal.is_empty() { return 0.0; }
        let mut sorted: Vec<f32> = signal.iter().map(|&x| x.abs()).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = (p / 100.0 * (sorted.len() - 1) as f32).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    /// Estimate integrated LUFS for a signal (convenience wrapper).
    pub fn lufs_integrated(signal: &[f32], sample_rate: f32) -> f32 {
        let mut meter = Lufs::new(sample_rate);
        meter.push(signal);
        meter.integrated_lufs()
    }

    /// DC offset (mean of samples).
    pub fn dc_offset(signal: &[f32]) -> f32 {
        if signal.is_empty() { return 0.0; }
        signal.iter().sum::<f32>() / signal.len() as f32
    }

    /// Remove DC offset in place.
    pub fn remove_dc_offset(signal: &mut [f32]) {
        let dc = Self::dc_offset(signal);
        for s in signal.iter_mut() { *s -= dc; }
    }

    /// Compute the probability density function (smoothed histogram).
    pub fn pdf(signal: &[f32], bins: usize) -> Vec<f32> {
        let hist = Self::histogram(signal, bins);
        // Gaussian smoothing with window=3
        let mut smoothed = hist.clone();
        let kernel = [0.25, 0.5, 0.25];
        for i in 1..hist.len().saturating_sub(1) {
            smoothed[i] = kernel[0] * hist[i - 1] + kernel[1] * hist[i] + kernel[2] * hist[i + 1];
        }
        // Normalize
        let sum: f32 = smoothed.iter().sum();
        if sum > 1e-10 { for s in smoothed.iter_mut() { *s /= sum; } }
        smoothed
    }

    /// Sample entropy (approximate complexity measure).
    pub fn sample_entropy(signal: &[f32], m: usize, r: f32) -> f32 {
        let n = signal.len();
        if n < m + 2 { return 0.0; }
        let mut a = 0u64; // template matches of length m+1
        let mut b = 0u64; // template matches of length m
        for i in 0..n - m {
            for j in i + 1..n - m {
                let match_m = (0..m).all(|k| {
                    (signal[i + k] - signal[j + k]).abs() <= r
                });
                if match_m {
                    b += 1;
                    if (signal[i + m] - signal[j + m]).abs() <= r {
                        a += 1;
                    }
                }
            }
        }
        if b == 0 { return 0.0; }
        -((a as f32 / b as f32).max(1e-10)).ln()
    }
}

// ---------------------------------------------------------------------------
// SignalSimilarity
// ---------------------------------------------------------------------------

/// Signal similarity measures: cross-correlation and dynamic time warping.
pub struct SignalSimilarity;

impl SignalSimilarity {
    /// Full cross-correlation of `a` and `b`.
    pub fn cross_correlation(a: &[f32], b: &[f32]) -> Vec<f32> {
        let b_rev: Vec<f32> = b.iter().rev().copied().collect();
        // Use FFT convolution
        let out_len = a.len() + b.len() - 1;
        let n = next_power_of_two(out_len);
        let mut fa: Vec<Complex32> = a.iter().map(|&x| Complex32::new(x, 0.0)).collect();
        fa.resize(n, Complex32::zero());
        let mut fb: Vec<Complex32> = b_rev.iter().map(|&x| Complex32::new(x, 0.0)).collect();
        fb.resize(n, Complex32::zero());
        use super::fft::Fft;
        Fft::forward(&mut fa);
        Fft::forward(&mut fb);
        for (ai, bi) in fa.iter_mut().zip(fb.iter()) { *ai = *ai * *bi; }
        Fft::inverse(&mut fa);
        fa[..out_len].iter().map(|c| c.re).collect()
    }

    /// Normalized cross-correlation: peak value of NCC (0..1).
    pub fn normalized_cross_correlation(a: &[f32], b: &[f32]) -> f32 {
        let xcorr = Self::cross_correlation(a, b);
        let peak = xcorr.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        let norm_a: f32 = a.iter().map(|&x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|&x| x * x).sum::<f32>().sqrt();
        let denom = norm_a * norm_b;
        if denom < 1e-10 { 0.0 } else { peak / denom }
    }

    /// Dynamic Time Warping distance between two sequences.
    pub fn dtw_distance(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len();
        let m = b.len();
        if n == 0 || m == 0 { return f32::INFINITY; }
        let mut dtw = vec![vec![f32::INFINITY; m]; n];
        dtw[0][0] = (a[0] - b[0]).abs();
        for i in 1..n {
            dtw[i][0] = dtw[i - 1][0] + (a[i] - b[0]).abs();
        }
        for j in 1..m {
            dtw[0][j] = dtw[0][j - 1] + (a[0] - b[j]).abs();
        }
        for i in 1..n {
            for j in 1..m {
                let cost = (a[i] - b[j]).abs();
                dtw[i][j] = cost + dtw[i - 1][j].min(dtw[i][j - 1]).min(dtw[i - 1][j - 1]);
            }
        }
        dtw[n - 1][m - 1]
    }

    /// DTW with Sakoe-Chiba band constraint (faster for long sequences).
    pub fn dtw_distance_windowed(a: &[f32], b: &[f32], window: usize) -> f32 {
        let n = a.len();
        let m = b.len();
        if n == 0 || m == 0 { return f32::INFINITY; }
        let mut dtw = vec![vec![f32::INFINITY; m]; n];
        for i in 0..n {
            let j_lo = i.saturating_sub(window);
            let j_hi = (i + window + 1).min(m);
            for j in j_lo..j_hi {
                let cost = (a[i] - b[j]).abs();
                let prev = if i == 0 && j == 0 { 0.0 }
                    else if i == 0 { dtw[0][j - 1] }
                    else if j == 0 { dtw[i - 1][0] }
                    else { dtw[i - 1][j].min(dtw[i][j - 1]).min(dtw[i - 1][j - 1]) };
                dtw[i][j] = cost + prev;
            }
        }
        dtw[n - 1][m - 1]
    }

    /// Pearson correlation coefficient between two signals.
    pub fn pearson(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len().min(b.len());
        if n == 0 { return 0.0; }
        let mean_a = a[..n].iter().sum::<f32>() / n as f32;
        let mean_b = b[..n].iter().sum::<f32>() / n as f32;
        let mut cov = 0.0f32;
        let mut var_a = 0.0f32;
        let mut var_b = 0.0f32;
        for i in 0..n {
            let da = a[i] - mean_a;
            let db = b[i] - mean_b;
            cov += da * db;
            var_a += da * da;
            var_b += db * db;
        }
        let denom = (var_a * var_b).sqrt();
        if denom < 1e-10 { 0.0 } else { cov / denom }
    }

    /// Mean Squared Error between two signals.
    pub fn mse(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len().min(b.len());
        if n == 0 { return 0.0; }
        a[..n].iter().zip(b[..n].iter()).map(|(&x, &y)| (x - y).powi(2)).sum::<f32>() / n as f32
    }

    /// Signal-to-Noise Ratio (dB): SNR = 10 log10(signal_power / noise_power).
    pub fn snr_db(signal: &[f32], noise: &[f32]) -> f32 {
        let n = signal.len().min(noise.len());
        if n == 0 { return 0.0; }
        let sig_power: f32 = signal[..n].iter().map(|&x| x * x).sum::<f32>() / n as f32;
        let noise_power: f32 = noise[..n].iter().map(|&x| x * x).sum::<f32>() / n as f32;
        if noise_power < 1e-30 { return f32::INFINITY; }
        10.0 * (sig_power / noise_power).log10()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::SignalGenerator;

    fn sine(freq: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| (2.0 * PI * freq * i as f32 / sr).sin()).collect()
    }

    // --- OnsetDetector ---

    #[test]
    fn test_onset_spectral_flux_finds_onset() {
        let sr = 44100.0;
        // Silence then loud sine — should detect an onset
        let mut signal = vec![0.0f32; 2048];
        signal.extend(sine(440.0, sr, 4096));
        let detector = SpectralFluxOnset::new(1.5, 1024, 256);
        let onsets = detector.detect(&signal, sr);
        assert!(!onsets.is_empty(), "Should detect at least one onset");
    }

    #[test]
    fn test_hfc_onset_basic() {
        let sr = 44100.0;
        let mut signal = vec![0.0f32; 4096];
        signal.extend(sine(440.0, sr, 4096));
        let detector = HfcOnset::new(0.3, 1024, 256);
        let onsets = detector.detect(&signal, sr);
        // Should not crash and may find onsets
        let _ = onsets;
    }

    #[test]
    fn test_complex_domain_onset_basic() {
        let sr = 44100.0;
        let mut signal = vec![0.0f32; 4096];
        signal.extend(sine(440.0, sr, 4096));
        let detector = ComplexDomainOnset::new(0.3, 1024, 256);
        let onsets = detector.detect(&signal, sr);
        let _ = onsets;
    }

    // --- BeatTracker ---

    #[test]
    fn test_beat_tracker_estimate_tempo_basic() {
        let sr = 44100.0;
        // Create a simple pulse at 120 BPM (every 22050 samples)
        let mut signal = vec![0.0f32; sr as usize * 4];
        let bpm = 120.0f32;
        let beat_samples = (60.0 * sr / bpm) as usize;
        for k in (0..signal.len()).step_by(beat_samples) {
            if k < signal.len() { signal[k] = 1.0; }
        }
        let estimated = BeatTracker::estimate_tempo(&signal, sr);
        // Should be in a reasonable range
        assert!(estimated > 50.0 && estimated < 250.0, "estimated_bpm={}", estimated);
    }

    #[test]
    fn test_beat_tracker_detect_beats() {
        let sr = 44100.0;
        let mut signal = vec![0.0f32; sr as usize * 2];
        let beat_samples = (60.0 * sr / 120.0) as usize;
        for k in (0..signal.len()).step_by(beat_samples) {
            if k < signal.len() { signal[k] = 1.0; }
        }
        let beats = BeatTracker::detect_beats(&signal, sr);
        // Should find multiple beats
        assert!(beats.len() > 0);
    }

    // --- PitchDetector ---

    #[test]
    fn test_pitch_detector_yin() {
        let sr = 44100.0;
        let sig = sine(440.0, sr, 4096);
        let pitch = PitchDetector::yin(&sig, sr, 20.0, 2000.0);
        assert!(pitch.is_some());
        let p = pitch.unwrap();
        assert!((p - 440.0).abs() < 20.0, "pitch={}", p);
    }

    #[test]
    fn test_pitch_detector_autocorr() {
        let sr = 44100.0;
        let sig = sine(220.0, sr, 4096);
        let pitch = PitchDetector::autocorr(&sig, sr);
        if let Some(p) = pitch {
            assert!((p - 220.0).abs() < 40.0, "pitch={}", p);
        }
    }

    #[test]
    fn test_note_name_a4() {
        let midi = PitchDetector::midi_note(440.0);
        let name = PitchDetector::note_name(midi);
        assert_eq!(name, "A4");
    }

    #[test]
    fn test_note_name_c4() {
        let name = PitchDetector::note_name(60.0);
        assert_eq!(name, "C4");
    }

    // --- Rms meter ---

    #[test]
    fn test_rms_meter_converges() {
        let sr = 44100.0;
        let mut rms = Rms::new(100.0, sr);
        let sig = sine(440.0, sr, 44100);
        for &s in &sig {
            rms.process(s);
        }
        let level = rms.level();
        // RMS of a sine at amplitude 1 is 1/√2 ≈ 0.707
        assert!((level - 0.707).abs() < 0.05, "level={}", level);
    }

    #[test]
    fn test_leq() {
        let mut leq = Leq::new();
        let sig = vec![0.5f32; 44100];
        leq.push(&sig);
        let db = leq.leq_db();
        // 0.5 RMS = -6 dBFS
        assert!((db - (-6.0206)).abs() < 0.1, "db={}", db);
    }

    #[test]
    fn test_lufs_push_no_crash() {
        let mut lufs = Lufs::new(44100.0);
        let sig: Vec<f32> = (0..44100 * 5).map(|i| {
            (2.0 * PI * 1000.0 * i as f32 / 44100.0).sin() * 0.5
        }).collect();
        lufs.push(&sig);
        let db = lufs.integrated_lufs();
        // Should be a finite (possibly negative) value
        assert!(db.is_finite() || db == -f32::INFINITY);
    }

    // --- DynamicRange ---

    #[test]
    fn test_crest_factor_sine() {
        let sig = sine(440.0, 44100.0, 44100);
        let cf = DynamicRange::crest_factor(&sig);
        // Sine crest factor = √2 ≈ 1.414
        assert!((cf - 2.0f32.sqrt()).abs() < 0.05, "cf={}", cf);
    }

    // --- TransientAnalysis ---

    #[test]
    fn test_attack_time() {
        let sr = 44100.0;
        // Amplitude ramp: reaches peak at 0.1s
        let n = sr as usize;
        let sig: Vec<f32> = (0..n).map(|i| {
            if i < (0.1 * sr) as usize { i as f32 / (0.1 * sr) } else { 1.0 }
        }).collect();
        let at = TransientAnalysis::attack_time(&sig, sr);
        assert!(at >= 0.0 && at <= 0.15, "attack_time={}", at);
    }

    #[test]
    fn test_transient_to_steady_ratio() {
        // Signal that's loud at start and quiet later
        let mut sig = vec![1.0f32; 100];
        sig.extend(vec![0.001f32; 900]);
        let ratio = TransientAnalysis::transient_to_steady_ratio(&sig);
        assert!(ratio > 1.0, "ratio={}", ratio);
    }

    // --- HarmonicAnalyzer ---

    #[test]
    fn test_harmonic_analyzer_thd() {
        // Pure sine: only fundamental, THD = 0
        let harmonics = vec![1.0, 0.0, 0.0, 0.0];
        let thd = HarmonicAnalyzer::thd(&harmonics);
        assert!(thd.abs() < 1e-5);
    }

    #[test]
    fn test_harmonic_analyzer_thd_nonzero() {
        let harmonics = vec![1.0, 0.1, 0.05, 0.02];
        let thd = HarmonicAnalyzer::thd(&harmonics);
        assert!(thd > 0.0 && thd < 1.0);
    }

    // --- Correlogram ---

    #[test]
    fn test_correlogram_frames() {
        let mut corr = Correlogram::new(64);
        let frame = sine(440.0, 44100.0, 256);
        corr.push_frame(&frame);
        corr.push_frame(&frame);
        assert_eq!(corr.num_frames(), 2);
        let avg = corr.average();
        assert!(!avg.is_empty());
    }

    // --- DynamicsAnalyzer ---

    #[test]
    fn test_histogram_sums_to_one() {
        let sig = sine(440.0, 44100.0, 4410);
        let hist = DynamicsAnalyzer::histogram(&sig, 32);
        let sum: f32 = hist.iter().sum();
        assert!((sum - 1.0).abs() < 0.01, "sum={}", sum);
    }

    #[test]
    fn test_percentile_50th() {
        let sig: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let p50 = DynamicsAnalyzer::percentile(&sig, 50.0);
        assert!((p50 - 0.5).abs() < 0.1, "p50={}", p50);
    }

    // --- SignalSimilarity ---

    #[test]
    fn test_ncc_identical() {
        let sig = sine(440.0, 44100.0, 1024);
        let ncc = SignalSimilarity::normalized_cross_correlation(&sig, &sig);
        assert!(ncc > 0.9, "ncc={}", ncc);
    }

    #[test]
    fn test_dtw_self_zero() {
        let sig: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4];
        let d = SignalSimilarity::dtw_distance(&sig, &sig);
        assert!(d.abs() < 1e-5, "dtw_self={}", d);
    }

    #[test]
    fn test_dtw_different_lengths() {
        let a: Vec<f32> = vec![0.0, 1.0, 0.0];
        let b: Vec<f32> = vec![0.0, 1.0, 1.0, 0.0];
        let d = SignalSimilarity::dtw_distance(&a, &b);
        assert!(d >= 0.0);
    }

    #[test]
    fn test_pearson_perfect_positive() {
        let a: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let b: Vec<f32> = vec![2.0, 4.0, 6.0, 8.0]; // 2x
        let p = SignalSimilarity::pearson(&a, &b);
        assert!((p - 1.0).abs() < 1e-5, "pearson={}", p);
    }

    #[test]
    fn test_mse_zero_identical() {
        let sig: Vec<f32> = vec![1.0, -0.5, 0.3];
        let mse = SignalSimilarity::mse(&sig, &sig);
        assert!(mse.abs() < 1e-10);
    }

    #[test]
    fn test_snr_basic() {
        let signal: Vec<f32> = vec![1.0; 100];
        let noise: Vec<f32> = vec![0.1; 100]; // SNR = 20 dB
        let snr = SignalSimilarity::snr_db(&signal, &noise);
        assert!((snr - 20.0).abs() < 0.1, "snr={}", snr);
    }
}
