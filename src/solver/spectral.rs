//! Spectral methods — FFT-based PDE solving for periodic domains.

/// 1D Discrete Fourier Transform (naive O(n²) — for correctness; production uses FFT).
pub fn dft(input: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let n = input.len();
    let mut output = vec![(0.0, 0.0); n];
    for k in 0..n {
        let mut re = 0.0;
        let mut im = 0.0;
        for j in 0..n {
            let angle = -2.0 * std::f64::consts::PI * k as f64 * j as f64 / n as f64;
            re += input[j].0 * angle.cos() - input[j].1 * angle.sin();
            im += input[j].0 * angle.sin() + input[j].1 * angle.cos();
        }
        output[k] = (re, im);
    }
    output
}

/// 1D Inverse DFT.
pub fn idft(input: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let n = input.len();
    let mut output = vec![(0.0, 0.0); n];
    for j in 0..n {
        let mut re = 0.0;
        let mut im = 0.0;
        for k in 0..n {
            let angle = 2.0 * std::f64::consts::PI * k as f64 * j as f64 / n as f64;
            re += input[k].0 * angle.cos() - input[k].1 * angle.sin();
            im += input[k].0 * angle.sin() + input[k].1 * angle.cos();
        }
        output[j] = (re / n as f64, im / n as f64);
    }
    output
}

/// Solve the 1D heat equation spectrally: u_t = α * u_xx on periodic domain [0, L].
pub fn spectral_heat_step(u: &mut [f64], alpha: f64, dt: f64, dx: f64) {
    let n = u.len();
    let input: Vec<(f64, f64)> = u.iter().map(|&v| (v, 0.0)).collect();
    let mut spectrum = dft(&input);

    // Multiply by exp(-α * k² * dt) in frequency space
    let l = n as f64 * dx;
    for k in 0..n {
        let kk = if k <= n / 2 { k as f64 } else { k as f64 - n as f64 };
        let freq = 2.0 * std::f64::consts::PI * kk / l;
        let decay = (-alpha * freq * freq * dt).exp();
        spectrum[k].0 *= decay;
        spectrum[k].1 *= decay;
    }

    let result = idft(&spectrum);
    for i in 0..n { u[i] = result[i].0; }
}

/// Power spectrum (magnitude² of DFT coefficients).
pub fn power_spectrum(signal: &[f64]) -> Vec<f64> {
    let input: Vec<(f64, f64)> = signal.iter().map(|&v| (v, 0.0)).collect();
    let spectrum = dft(&input);
    spectrum.iter().map(|(re, im)| re * re + im * im).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dft_idft_roundtrip() {
        let input = vec![(1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 0.0)];
        let spectrum = dft(&input);
        let recovered = idft(&spectrum);
        for (orig, rec) in input.iter().zip(recovered.iter()) {
            assert!((orig.0 - rec.0).abs() < 1e-10, "orig={}, rec={}", orig.0, rec.0);
        }
    }

    #[test]
    fn spectral_heat_smooths() {
        let n = 32;
        let mut u: Vec<f64> = (0..n).map(|i| if i == n / 2 { 1.0 } else { 0.0 }).collect();
        let peak_before = u[n / 2];
        spectral_heat_step(&mut u, 1.0, 0.01, 1.0 / n as f64);
        assert!(u[n / 2] < peak_before, "Heat should smooth the spike");
    }
}
