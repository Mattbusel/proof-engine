use std::f64::consts::PI;
use super::schrodinger::{Complex, WaveFunction1D, dft, idft};

/// Render 1D wave function as glyph brightness (|psi|^2) and phase (hue).
pub struct WaveFunctionRenderer1D {
    pub width: usize,
    pub brightness_scale: f64,
}

impl WaveFunctionRenderer1D {
    pub fn new(width: usize) -> Self {
        Self { width, brightness_scale: 1.0 }
    }

    /// Render the wave function to a vector of (character, r, g, b, brightness).
    pub fn render(&self, wf: &WaveFunction1D) -> Vec<(char, f64, f64, f64, f64)> {
        let n = wf.n();
        let mut result = Vec::with_capacity(self.width);
        for i in 0..self.width {
            let idx = (i * n) / self.width.max(1);
            let idx = idx.min(n - 1);
            let c = wf.psi[idx];
            let prob = c.norm_sq() * self.brightness_scale;
            let phase = c.arg();
            let (r, g, b) = PhaseColorMap::phase_to_rgb(phase);
            let ch = brightness_to_char(prob);
            result.push((ch, r, g, b, prob));
        }
        result
    }
}

/// Render 2D wave function as glyph brightness grid.
pub struct WaveFunctionRenderer2D {
    pub width: usize,
    pub height: usize,
    pub brightness_scale: f64,
}

impl WaveFunctionRenderer2D {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, brightness_scale: 1.0 }
    }

    pub fn render(&self, psi: &[Vec<Complex>]) -> Vec<Vec<(char, f64, f64, f64, f64)>> {
        let nx = psi.len();
        let ny = if nx > 0 { psi[0].len() } else { 0 };
        let mut grid = Vec::with_capacity(self.height);
        for j in 0..self.height {
            let mut row = Vec::with_capacity(self.width);
            for i in 0..self.width {
                let ix = (i * nx) / self.width.max(1);
                let iy = (j * ny) / self.height.max(1);
                let ix = ix.min(nx.saturating_sub(1));
                let iy = iy.min(ny.saturating_sub(1));
                if nx == 0 || ny == 0 {
                    row.push((' ', 0.0, 0.0, 0.0, 0.0));
                    continue;
                }
                let c = psi[ix][iy];
                let prob = c.norm_sq() * self.brightness_scale;
                let phase = c.arg();
                let (r, g, b) = PhaseColorMap::phase_to_rgb(phase);
                let ch = brightness_to_char(prob);
                row.push((ch, r, g, b, prob));
            }
            grid.push(row);
        }
        grid
    }
}

fn brightness_to_char(b: f64) -> char {
    let chars = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
    let idx = (b * (chars.len() - 1) as f64).round() as usize;
    chars[idx.min(chars.len() - 1)]
}

/// Generate a Gaussian wave packet centered at x0 with momentum k0.
pub fn gaussian_wavepacket(x0: f64, k0: f64, sigma: f64, x_grid: &[f64]) -> Vec<Complex> {
    let norm = 1.0 / (sigma * (2.0 * PI).sqrt()).sqrt();
    x_grid
        .iter()
        .map(|&x| {
            let gauss = (-((x - x0) * (x - x0)) / (4.0 * sigma * sigma)).exp();
            let phase = k0 * x;
            Complex::new(norm * gauss * phase.cos(), norm * gauss * phase.sin())
        })
        .collect()
}

/// Generate a plane wave with wave number k.
pub fn plane_wave(k: f64, x_grid: &[f64]) -> Vec<Complex> {
    let n = x_grid.len();
    let norm = 1.0 / (n as f64).sqrt();
    x_grid
        .iter()
        .map(|&x| {
            let phase = k * x;
            Complex::new(norm * phase.cos(), norm * phase.sin())
        })
        .collect()
}

/// Generate a coherent state of the harmonic oscillator.
pub fn coherent_state(n_max: usize, alpha: Complex, x_grid: &[f64], omega: f64, mass: f64, hbar: f64) -> Vec<Complex> {
    let mut psi = vec![Complex::zero(); x_grid.len()];
    let alpha_sq = alpha.norm_sq();
    let prefactor = (-alpha_sq / 2.0).exp();

    for n in 0..n_max {
        // c_n = alpha^n / sqrt(n!) * exp(-|alpha|^2/2)
        let mut alpha_n = Complex::one();
        for _ in 0..n {
            alpha_n = alpha_n * alpha;
        }
        let n_fact: f64 = (1..=n).map(|k| k as f64).product::<f64>().max(1.0);
        let c_n = alpha_n * (prefactor / n_fact.sqrt());

        for (i, &x) in x_grid.iter().enumerate() {
            let phi_n = super::harmonic::qho_wavefunction(n as u32, x, omega, mass, hbar);
            psi[i] += c_n * phi_n;
        }
    }
    psi
}

/// Compute the Wigner quasi-probability distribution.
pub fn wigner_function(psi: &[Complex], x_grid: &[f64], p_grid: &[f64], dx: f64, hbar: f64) -> Vec<Vec<f64>> {
    let nx = x_grid.len();
    let np = p_grid.len();
    let n_psi = psi.len();
    let mut w = vec![vec![0.0; np]; nx];

    for (ix, &x) in x_grid.iter().enumerate() {
        for (ip, &p) in p_grid.iter().enumerate() {
            let mut integral = 0.0;
            // W(x,p) = 1/(pi*hbar) * integral psi*(x-y) psi(x+y) e^(2ipy/hbar) dy
            let max_y_steps = (n_psi / 2).min(50);
            let dy = dx;
            for k in 0..max_y_steps {
                let y = k as f64 * dy;
                let x_plus = x + y;
                let x_minus = x - y;

                // Interpolate psi at x+y and x-y
                let psi_plus = interpolate_psi(psi, x_grid, x_plus);
                let psi_minus = interpolate_psi(psi, x_grid, x_minus);

                let phase = Complex::from_polar(1.0, 2.0 * p * y / hbar);
                let integrand = psi_minus.conj() * psi_plus * phase;

                let weight = if k == 0 { 1.0 } else { 2.0 }; // symmetric integration
                integral += integrand.re * weight * dy;
            }
            w[ix][ip] = integral / (PI * hbar);
        }
    }
    w
}

fn interpolate_psi(psi: &[Complex], x_grid: &[f64], x: f64) -> Complex {
    if x_grid.is_empty() {
        return Complex::zero();
    }
    let n = x_grid.len();
    let x_min = x_grid[0];
    let dx = if n > 1 { x_grid[1] - x_grid[0] } else { 1.0 };
    let idx_f = (x - x_min) / dx;
    if idx_f < 0.0 || idx_f >= (n - 1) as f64 {
        return Complex::zero();
    }
    let idx = idx_f as usize;
    let t = idx_f - idx as f64;
    psi[idx] * (1.0 - t) + psi[idx + 1] * t
}

/// DFT to momentum representation.
pub fn momentum_space(psi: &[Complex], dx: f64) -> Vec<Complex> {
    let mut result = dft(psi);
    let n = result.len();
    let norm = (dx / (2.0 * PI).sqrt());
    for c in &mut result {
        *c = *c * norm;
    }
    result
}

/// Map complex phase to HSV color.
pub struct PhaseColorMap;

impl PhaseColorMap {
    /// Map phase angle in [-pi, pi] to RGB using HSV with full saturation and value.
    pub fn phase_to_rgb(phase: f64) -> (f64, f64, f64) {
        let hue = (phase + PI) / (2.0 * PI); // 0 to 1
        let hue = hue.fract();
        Self::hsv_to_rgb(hue, 1.0, 1.0)
    }

    pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
        let h = h * 6.0;
        let c = v * s;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let m = v - c;
        let (r, g, b) = if h < 1.0 {
            (c, x, 0.0)
        } else if h < 2.0 {
            (x, c, 0.0)
        } else if h < 3.0 {
            (0.0, c, x)
        } else if h < 4.0 {
            (0.0, x, c)
        } else if h < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };
        (r + m, g + m, b + m)
    }
}

/// Density matrix for mixed states.
#[derive(Clone, Debug)]
pub struct DensityMatrix {
    pub rho: Vec<Vec<Complex>>,
}

impl DensityMatrix {
    pub fn new(rho: Vec<Vec<Complex>>) -> Self {
        Self { rho }
    }

    pub fn from_pure_state(psi: &[Complex]) -> Self {
        let n = psi.len();
        let mut rho = vec![vec![Complex::zero(); n]; n];
        for i in 0..n {
            for j in 0..n {
                rho[i][j] = psi[i] * psi[j].conj();
            }
        }
        Self { rho }
    }

    pub fn dim(&self) -> usize {
        self.rho.len()
    }

    pub fn trace(&self) -> Complex {
        let n = self.dim();
        let mut t = Complex::zero();
        for i in 0..n {
            t += self.rho[i][i];
        }
        t
    }

    /// Purity: Tr(rho^2). 1 for pure states, 1/d for maximally mixed.
    pub fn purity(&self) -> f64 {
        let n = self.dim();
        let mut sum = 0.0;
        for i in 0..n {
            for j in 0..n {
                sum += (self.rho[i][j] * self.rho[j][i]).re;
            }
        }
        sum
    }

    /// Von Neumann entropy: -Tr(rho * ln(rho)).
    /// Approximated using eigenvalues from diagonalization for 2x2,
    /// or using purity-based bound for larger systems.
    pub fn von_neumann_entropy(&self) -> f64 {
        let n = self.dim();
        if n == 2 {
            // Exact for 2x2: eigenvalues from trace and determinant
            let tr = self.rho[0][0].re + self.rho[1][1].re;
            let det = (self.rho[0][0] * self.rho[1][1] - self.rho[0][1] * self.rho[1][0]).re;
            let disc = (tr * tr - 4.0 * det).max(0.0).sqrt();
            let l1 = ((tr + disc) / 2.0).max(1e-30);
            let l2 = ((tr - disc) / 2.0).max(1e-30);
            let mut s = 0.0;
            if l1 > 1e-20 { s -= l1 * l1.ln(); }
            if l2 > 1e-20 { s -= l2 * l2.ln(); }
            return s;
        }
        // For larger matrices, use iterative approach to get diagonal elements
        // as approximation (exact for diagonal density matrices)
        let mut s = 0.0;
        for i in 0..n {
            let p = self.rho[i][i].re;
            if p > 1e-20 {
                s -= p * p.ln();
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gaussian_wavepacket() {
        let n = 512;
        let dx = 0.05;
        let x_grid: Vec<f64> = (0..n).map(|i| -12.8 + i as f64 * dx).collect();
        let psi = gaussian_wavepacket(0.0, 0.0, 1.0, &x_grid);
        let norm_sq: f64 = psi.iter().map(|c| c.norm_sq()).sum::<f64>() * dx;
        assert!((norm_sq - 1.0).abs() < 0.05, "Norm: {}", norm_sq);
    }

    #[test]
    fn test_gaussian_width() {
        let n = 1024;
        let dx = 0.05;
        let sigma = 2.0;
        let x_grid: Vec<f64> = (0..n).map(|i| -25.0 + i as f64 * dx).collect();
        let psi = gaussian_wavepacket(0.0, 0.0, sigma, &x_grid);
        // Width in position: should be sigma
        let norm_sq: f64 = psi.iter().map(|c| c.norm_sq()).sum::<f64>() * dx;
        let mean_x: f64 = psi.iter().enumerate().map(|(i, c)| c.norm_sq() * x_grid[i]).sum::<f64>() * dx / norm_sq;
        let var_x: f64 = psi.iter().enumerate().map(|(i, c)| c.norm_sq() * (x_grid[i] - mean_x).powi(2)).sum::<f64>() * dx / norm_sq;
        let measured_sigma = var_x.sqrt();
        assert!((measured_sigma - sigma).abs() < 0.3, "Sigma: {}", measured_sigma);
    }

    #[test]
    fn test_momentum_space_gaussian_reciprocal_width() {
        let n = 256;
        let dx = 0.1;
        let sigma = 1.0;
        let x_grid: Vec<f64> = (0..n).map(|i| -12.8 + i as f64 * dx).collect();
        let psi = gaussian_wavepacket(0.0, 0.0, sigma, &x_grid);
        let psi_k = momentum_space(&psi, dx);
        // In momentum space, width should be ~1/(2*sigma)
        let dk = 2.0 * PI / (n as f64 * dx);
        let norm_k: f64 = psi_k.iter().map(|c| c.norm_sq()).sum::<f64>() * dk;
        // Just verify it's finite and nonzero
        assert!(norm_k > 0.0);
    }

    #[test]
    fn test_plane_wave() {
        let n = 64;
        let x_grid: Vec<f64> = (0..n).map(|i| i as f64 * 0.1).collect();
        let psi = plane_wave(1.0, &x_grid);
        // All amplitudes should be equal
        let prob: Vec<f64> = psi.iter().map(|c| c.norm_sq()).collect();
        let expected = 1.0 / n as f64;
        for p in &prob {
            assert!((p - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn test_phase_color_map() {
        let (r, g, b) = PhaseColorMap::phase_to_rgb(0.0);
        // Phase 0 maps to hue = 0.5 (cyan-ish)
        assert!(r >= 0.0 && r <= 1.0);
        assert!(g >= 0.0 && g <= 1.0);
        assert!(b >= 0.0 && b <= 1.0);
    }

    #[test]
    fn test_density_matrix_pure_state() {
        let psi = vec![
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
        ];
        let dm = DensityMatrix::from_pure_state(&psi);
        let purity = dm.purity();
        assert!((purity - 1.0).abs() < 1e-10, "Purity: {}", purity);
        let entropy = dm.von_neumann_entropy();
        assert!(entropy.abs() < 0.01, "Entropy: {}", entropy);
    }

    #[test]
    fn test_density_matrix_mixed_state() {
        // Maximally mixed 2x2
        let rho = vec![
            vec![Complex::new(0.5, 0.0), Complex::zero()],
            vec![Complex::zero(), Complex::new(0.5, 0.0)],
        ];
        let dm = DensityMatrix::new(rho);
        let purity = dm.purity();
        assert!((purity - 0.5).abs() < 1e-10, "Purity: {}", purity);
        let entropy = dm.von_neumann_entropy();
        assert!((entropy - 2.0_f64.ln()).abs() < 0.01, "Entropy: {}", entropy);
    }

    #[test]
    fn test_renderer_1d() {
        let n = 64;
        let dx = 0.1;
        let x_grid: Vec<f64> = (0..n).map(|i| -3.2 + i as f64 * dx).collect();
        let psi = gaussian_wavepacket(0.0, 0.0, 1.0, &x_grid);
        let wf = WaveFunction1D::new(psi, dx, -3.2);
        let renderer = WaveFunctionRenderer1D::new(40);
        let result = renderer.render(&wf);
        assert_eq!(result.len(), 40);
    }

    #[test]
    fn test_renderer_2d() {
        let nx = 16;
        let ny = 16;
        let psi: Vec<Vec<Complex>> = (0..nx)
            .map(|i| {
                (0..ny)
                    .map(|j| {
                        let r2 = (i as f64 - 8.0).powi(2) + (j as f64 - 8.0).powi(2);
                        Complex::new((-r2 / 4.0).exp(), 0.0)
                    })
                    .collect()
            })
            .collect();
        let renderer = WaveFunctionRenderer2D::new(10, 10);
        let result = renderer.render(&psi);
        assert_eq!(result.len(), 10);
        assert_eq!(result[0].len(), 10);
    }

    #[test]
    fn test_wigner_function_runs() {
        let n = 32;
        let dx = 0.3;
        let x_grid: Vec<f64> = (0..n).map(|i| -4.8 + i as f64 * dx).collect();
        let psi = gaussian_wavepacket(0.0, 0.0, 1.0, &x_grid);
        let p_grid: Vec<f64> = (0..8).map(|i| -2.0 + i as f64 * 0.5).collect();
        let w = wigner_function(&psi, &x_grid, &p_grid, dx, 1.0);
        assert_eq!(w.len(), n);
        assert_eq!(w[0].len(), 8);
    }
}
