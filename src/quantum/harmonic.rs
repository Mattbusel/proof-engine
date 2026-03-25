use std::f64::consts::PI;
use super::schrodinger::Complex;

/// Energy of the nth level of a quantum harmonic oscillator.
pub fn qho_energy(n: u32, omega: f64, hbar: f64) -> f64 {
    hbar * omega * (n as f64 + 0.5)
}

/// Physicist's Hermite polynomial H_n(x) via recurrence.
/// H_0(x) = 1, H_1(x) = 2x, H_{n+1}(x) = 2x H_n(x) - 2n H_{n-1}(x)
pub fn hermite_polynomial(n: u32, x: f64) -> f64 {
    if n == 0 {
        return 1.0;
    }
    if n == 1 {
        return 2.0 * x;
    }
    let mut h_prev = 1.0;
    let mut h_curr = 2.0 * x;
    for k in 1..n {
        let h_next = 2.0 * x * h_curr - 2.0 * k as f64 * h_prev;
        h_prev = h_curr;
        h_curr = h_next;
    }
    h_curr
}

/// QHO wave function psi_n(x) = N_n * H_n(xi) * exp(-xi^2/2)
/// where xi = sqrt(m*omega/hbar) * x
pub fn qho_wavefunction(n: u32, x: f64, omega: f64, mass: f64, hbar: f64) -> f64 {
    let alpha = (mass * omega / hbar).sqrt();
    let xi = alpha * x;

    // Normalization: N_n = (alpha / (sqrt(pi) * 2^n * n!))^{1/2}
    let n_fact: f64 = (1..=n as u64).map(|k| k as f64).product::<f64>().max(1.0);
    let two_n: f64 = 2.0_f64.powi(n as i32);
    let norm = (alpha / (PI.sqrt() * two_n * n_fact)).sqrt();

    norm * hermite_polynomial(n, xi) * (-xi * xi / 2.0).exp()
}

/// Creation (raising) operator: a+ |n> = sqrt(n+1) |n+1>
/// Applied numerically: a+ psi(x) = (1/sqrt(2)) * (xi - d/dxi) psi(x) in scaled coords
pub fn qho_ladder_up(psi: &[Complex], x_grid: &[f64], omega: f64, mass: f64, hbar: f64) -> Vec<Complex> {
    let n = psi.len();
    let alpha = (mass * omega / hbar).sqrt();
    let dx = if n > 1 { x_grid[1] - x_grid[0] } else { 1.0 };
    let factor = 1.0 / (2.0_f64).sqrt();

    let mut result = vec![Complex::zero(); n];
    for i in 0..n {
        let xi = alpha * x_grid[i];
        // Numerical derivative
        let dpsi = if i == 0 {
            (psi[1] - psi[0]) / dx
        } else if i == n - 1 {
            (psi[n - 1] - psi[n - 2]) / dx
        } else {
            (psi[i + 1] - psi[i - 1]) / (2.0 * dx)
        };
        // a+ = (1/sqrt(2)) * (xi * psi - (1/alpha) * dpsi/dx ... actually in position rep:
        // a+ = sqrt(m*omega/(2*hbar)) * x - i*p/(sqrt(2*m*omega*hbar))
        // = sqrt(m*omega/(2*hbar)) * x - (1/sqrt(2*m*omega*hbar)) * (-i*hbar) d/dx
        // = (alpha/sqrt(2)) * x - (1/(alpha*sqrt(2))) * d/dx
        let coeff_x = alpha / (2.0_f64).sqrt();
        let coeff_d = 1.0 / (alpha * (2.0_f64).sqrt());
        result[i] = psi[i] * coeff_x * x_grid[i] - dpsi * coeff_d;
    }
    result
}

/// Annihilation (lowering) operator: a |n> = sqrt(n) |n-1>
/// a = (alpha/sqrt(2)) * x + (1/(alpha*sqrt(2))) * d/dx
pub fn qho_ladder_down(psi: &[Complex], x_grid: &[f64], omega: f64, mass: f64, hbar: f64) -> Vec<Complex> {
    let n = psi.len();
    let alpha = (mass * omega / hbar).sqrt();
    let dx = if n > 1 { x_grid[1] - x_grid[0] } else { 1.0 };

    let mut result = vec![Complex::zero(); n];
    for i in 0..n {
        let dpsi = if i == 0 {
            (psi[1] - psi[0]) / dx
        } else if i == n - 1 {
            (psi[n - 1] - psi[n - 2]) / dx
        } else {
            (psi[i + 1] - psi[i - 1]) / (2.0 * dx)
        };
        let coeff_x = alpha / (2.0_f64).sqrt();
        let coeff_d = 1.0 / (alpha * (2.0_f64).sqrt());
        result[i] = psi[i] * coeff_x * x_grid[i] + dpsi * coeff_d;
    }
    result
}

/// Time evolution of coherent state parameter: alpha(t) = alpha_0 * exp(-i*omega*t)
pub fn coherent_state_evolution(alpha: Complex, omega: f64, t: f64) -> Complex {
    let phase = Complex::from_polar(1.0, -omega * t);
    alpha * phase
}

/// Probability of finding n photons in a coherent state |alpha>: P(n) = |alpha|^{2n} e^{-|alpha|^2} / n!
pub fn number_state_probability(coherent_alpha: Complex, n: u32) -> f64 {
    let alpha_sq = coherent_alpha.norm_sq();
    let n_fact: f64 = (1..=n as u64).map(|k| k as f64).product::<f64>().max(1.0);
    alpha_sq.powi(n as i32) * (-alpha_sq).exp() / n_fact
}

/// Render QHO energy levels and wavefunctions.
pub struct QHORenderer {
    pub width: usize,
    pub height: usize,
    pub n_levels: usize,
    pub omega: f64,
    pub mass: f64,
    pub hbar: f64,
}

impl QHORenderer {
    pub fn new(width: usize, height: usize, n_levels: usize) -> Self {
        Self {
            width,
            height,
            n_levels,
            omega: 1.0,
            mass: 1.0,
            hbar: 1.0,
        }
    }

    /// Render energy levels as horizontal lines with wavefunctions overlaid.
    /// Returns a grid of (char, r, g, b).
    pub fn render(&self) -> Vec<Vec<(char, f64, f64, f64)>> {
        let x_min = -5.0;
        let x_max = 5.0;
        let e_max = qho_energy(self.n_levels as u32, self.omega, self.hbar);

        let mut grid = vec![vec![(' ', 0.0, 0.0, 0.0); self.width]; self.height];

        for level in 0..self.n_levels {
            let e = qho_energy(level as u32, self.omega, self.hbar);
            let y_frac = e / e_max;
            let row = self.height - 1 - ((y_frac * (self.height - 1) as f64) as usize).min(self.height - 1);

            // Color based on level
            let hue = level as f64 / self.n_levels as f64;
            let (r, g, b) = super::wavefunction::PhaseColorMap::hsv_to_rgb(hue, 0.8, 1.0);

            for col in 0..self.width {
                let x = x_min + (col as f64 / self.width as f64) * (x_max - x_min);
                let psi = qho_wavefunction(level as u32, x, self.omega, self.mass, self.hbar);
                let offset = (psi * 3.0) as i32;
                let draw_row = (row as i32 - offset) as usize;
                if draw_row < self.height {
                    let brightness = psi.abs().min(1.0);
                    if brightness > 0.05 {
                        grid[draw_row][col] = ('*', r * brightness, g * brightness, b * brightness);
                    }
                }
                // Draw energy level line
                if grid[row][col].0 == ' ' {
                    grid[row][col] = ('-', r * 0.3, g * 0.3, b * 0.3);
                }
            }
        }

        // Draw potential (parabola)
        for col in 0..self.width {
            let x = x_min + (col as f64 / self.width as f64) * (x_max - x_min);
            let v = 0.5 * self.mass * self.omega * self.omega * x * x;
            let y_frac = v / e_max;
            if y_frac <= 1.0 {
                let row = self.height - 1 - ((y_frac * (self.height - 1) as f64) as usize).min(self.height - 1);
                if grid[row][col].0 == ' ' || grid[row][col].0 == '-' {
                    grid[row][col] = ('|', 0.3, 0.3, 0.3);
                }
            }
        }

        grid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qho_energy_levels() {
        assert!((qho_energy(0, 1.0, 1.0) - 0.5).abs() < 1e-10);
        assert!((qho_energy(1, 1.0, 1.0) - 1.5).abs() < 1e-10);
        assert!((qho_energy(2, 1.0, 1.0) - 2.5).abs() < 1e-10);
        assert!((qho_energy(5, 2.0, 1.0) - 11.0).abs() < 1e-10);
    }

    #[test]
    fn test_hermite_polynomials() {
        assert!((hermite_polynomial(0, 1.0) - 1.0).abs() < 1e-10);
        assert!((hermite_polynomial(1, 1.0) - 2.0).abs() < 1e-10);
        // H_2(x) = 4x^2 - 2
        assert!((hermite_polynomial(2, 1.0) - 2.0).abs() < 1e-10);
        assert!((hermite_polynomial(2, 0.0) - (-2.0)).abs() < 1e-10);
        // H_3(x) = 8x^3 - 12x
        assert!((hermite_polynomial(3, 1.0) - (-4.0)).abs() < 1e-10);
    }

    #[test]
    fn test_wavefunction_normalization() {
        let dx = 0.01;
        let n_points = 2000;
        let x_min = -10.0;
        for n in 0..5 {
            let integral: f64 = (0..n_points)
                .map(|i| {
                    let x = x_min + i as f64 * dx;
                    let psi = qho_wavefunction(n, x, 1.0, 1.0, 1.0);
                    psi * psi * dx
                })
                .sum();
            assert!(
                (integral - 1.0).abs() < 0.02,
                "n={}: integral={}",
                n,
                integral
            );
        }
    }

    #[test]
    fn test_orthogonality() {
        let dx = 0.01;
        let n_points = 2000;
        let x_min = -10.0;

        // <0|1> should be 0
        let integral: f64 = (0..n_points)
            .map(|i| {
                let x = x_min + i as f64 * dx;
                let psi0 = qho_wavefunction(0, x, 1.0, 1.0, 1.0);
                let psi1 = qho_wavefunction(1, x, 1.0, 1.0, 1.0);
                psi0 * psi1 * dx
            })
            .sum();
        assert!(integral.abs() < 0.02, "<0|1> = {}", integral);

        // <0|2> should be 0
        let integral: f64 = (0..n_points)
            .map(|i| {
                let x = x_min + i as f64 * dx;
                let psi0 = qho_wavefunction(0, x, 1.0, 1.0, 1.0);
                let psi2 = qho_wavefunction(2, x, 1.0, 1.0, 1.0);
                psi0 * psi2 * dx
            })
            .sum();
        assert!(integral.abs() < 0.02, "<0|2> = {}", integral);
    }

    #[test]
    fn test_ladder_operators() {
        // a|0> should be ~0
        let n_pts = 512;
        let dx = 0.05;
        let x_grid: Vec<f64> = (0..n_pts).map(|i| -12.8 + i as f64 * dx).collect();
        let psi0: Vec<Complex> = x_grid
            .iter()
            .map(|&x| Complex::new(qho_wavefunction(0, x, 1.0, 1.0, 1.0), 0.0))
            .collect();
        let a_psi0 = qho_ladder_down(&psi0, &x_grid, 1.0, 1.0, 1.0);
        let norm: f64 = a_psi0.iter().map(|c| c.norm_sq()).sum::<f64>() * dx;
        assert!(norm < 0.1, "a|0> norm = {}", norm);

        // a+|0> should be proportional to |1>
        let a_up_psi0 = qho_ladder_up(&psi0, &x_grid, 1.0, 1.0, 1.0);
        let norm_up: f64 = a_up_psi0.iter().map(|c| c.norm_sq()).sum::<f64>() * dx;
        // Should be ~1 (since a+|0> = |1>)
        assert!((norm_up - 1.0).abs() < 0.3, "a+|0> norm = {}", norm_up);
    }

    #[test]
    fn test_ground_state_uncertainty() {
        // For ground state: dx * dp = hbar/2
        let n_pts = 1024;
        let dx_grid = 0.02;
        let x_grid: Vec<f64> = (0..n_pts).map(|i| -10.0 + i as f64 * dx_grid).collect();
        let psi: Vec<Complex> = x_grid
            .iter()
            .map(|&x| Complex::new(qho_wavefunction(0, x, 1.0, 1.0, 1.0), 0.0))
            .collect();
        let wf = super::super::schrodinger::WaveFunction1D::new(psi, dx_grid, -10.0);

        let dx_unc = super::super::schrodinger::uncertainty_x(&wf);
        let dp_unc = super::super::schrodinger::uncertainty_p(&wf, 1.0);
        let product = dx_unc * dp_unc;
        // Should be hbar/2 = 0.5
        assert!(
            (product - 0.5).abs() < 0.15,
            "dx*dp = {} (expected 0.5)",
            product
        );
    }

    #[test]
    fn test_coherent_state_evolution() {
        let alpha = Complex::new(1.0, 0.0);
        let evolved = coherent_state_evolution(alpha, 1.0, PI);
        // After half period, alpha -> -alpha
        assert!((evolved.re - (-1.0)).abs() < 1e-10);
        assert!(evolved.im.abs() < 1e-10);
    }

    #[test]
    fn test_number_state_probability() {
        let alpha = Complex::new(2.0, 0.0);
        let total: f64 = (0..20).map(|n| number_state_probability(alpha, n)).sum();
        assert!((total - 1.0).abs() < 0.01, "Total prob: {}", total);

        // Mean should be |alpha|^2 = 4
        let mean: f64 = (0..20)
            .map(|n| n as f64 * number_state_probability(alpha, n))
            .sum();
        assert!((mean - 4.0).abs() < 0.1, "Mean: {}", mean);
    }

    #[test]
    fn test_renderer() {
        let renderer = QHORenderer::new(40, 20, 4);
        let grid = renderer.render();
        assert_eq!(grid.len(), 20);
        assert_eq!(grid[0].len(), 40);
    }
}
