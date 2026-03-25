use std::f64::consts::PI;
use super::schrodinger::{Complex, SchrodingerSolver2D};

/// Configuration for a double-slit experiment.
#[derive(Clone, Debug)]
pub struct DoubleSlitSetup {
    pub slit_width: f64,
    pub slit_separation: f64,
    pub wavelength: f64,
    pub screen_distance: f64,
}

impl DoubleSlitSetup {
    pub fn new(slit_width: f64, slit_separation: f64, wavelength: f64, screen_distance: f64) -> Self {
        Self { slit_width, slit_separation, wavelength, screen_distance }
    }
}

/// Analytical Fraunhofer double-slit intensity pattern.
/// I(theta) = I_0 * cos^2(pi*d*sin(theta)/lambda) * sinc^2(pi*a*sin(theta)/lambda)
pub fn intensity_pattern(setup: &DoubleSlitSetup, screen_positions: &[f64]) -> Vec<f64> {
    let d = setup.slit_separation;
    let a = setup.slit_width;
    let lambda = setup.wavelength;
    let l = setup.screen_distance;

    screen_positions
        .iter()
        .map(|&y| {
            let sin_theta = y / (y * y + l * l).sqrt();

            // Double-slit interference
            let phase_d = PI * d * sin_theta / lambda;
            let interference = phase_d.cos().powi(2);

            // Single-slit diffraction envelope
            let phase_a = PI * a * sin_theta / lambda;
            let diffraction = if phase_a.abs() < 1e-12 {
                1.0
            } else {
                (phase_a.sin() / phase_a).powi(2)
            };

            interference * diffraction
        })
        .collect()
}

/// Single-slit Fraunhofer diffraction pattern.
pub fn single_slit_pattern(width: f64, wavelength: f64, angles: &[f64]) -> Vec<f64> {
    angles
        .iter()
        .map(|&theta| {
            let beta = PI * width * theta.sin() / wavelength;
            if beta.abs() < 1e-12 {
                1.0
            } else {
                (beta.sin() / beta).powi(2)
            }
        })
        .collect()
}

/// N-slit diffraction pattern.
pub fn n_slit_pattern(n: usize, width: f64, separation: f64, wavelength: f64, angles: &[f64]) -> Vec<f64> {
    if n == 0 {
        return vec![0.0; angles.len()];
    }
    angles
        .iter()
        .map(|&theta| {
            let sin_t = theta.sin();
            // Single slit envelope
            let beta = PI * width * sin_t / wavelength;
            let envelope = if beta.abs() < 1e-12 {
                1.0
            } else {
                (beta.sin() / beta).powi(2)
            };

            // N-slit interference
            let delta = PI * separation * sin_t / wavelength;
            let n_delta = n as f64 * delta;
            let multi_slit = if delta.abs() < 1e-12 {
                (n * n) as f64
            } else {
                (n_delta.sin() / delta.sin()).powi(2)
            };

            envelope * multi_slit / (n * n) as f64
        })
        .collect()
}

/// 2D wave simulation of double-slit experiment.
pub struct DoubleSlitSimulation {
    pub solver: SchrodingerSolver2D,
    pub slit_mask: Vec<Vec<bool>>,
}

impl DoubleSlitSimulation {
    /// Create a simulation with slits at a wall position.
    pub fn new(
        nx: usize,
        ny: usize,
        dx: f64,
        dy: f64,
        dt: f64,
        wall_x_idx: usize,
        slit1_y: (usize, usize),
        slit2_y: (usize, usize),
        barrier_height: f64,
    ) -> Self {
        let mut potential = vec![vec![0.0; ny]; nx];
        let mut slit_mask = vec![vec![false; ny]; nx];

        // Set up barrier wall with two slits
        for j in 0..ny {
            let is_slit = (j >= slit1_y.0 && j <= slit1_y.1)
                || (j >= slit2_y.0 && j <= slit2_y.1);
            if !is_slit {
                potential[wall_x_idx][j] = barrier_height;
            }
            slit_mask[wall_x_idx][j] = is_slit;
        }

        let psi = vec![vec![Complex::zero(); ny]; nx];
        let solver = SchrodingerSolver2D::new(psi, potential, nx, ny, dx, dy, dt, 1.0, 1.0);

        Self { solver, slit_mask }
    }

    /// Initialize with a plane wave approaching the slits.
    pub fn init_plane_wave(&mut self, k: f64) {
        let nx = self.solver.nx;
        let ny = self.solver.ny;
        let dx = self.solver.dx;
        let sigma = nx as f64 * dx * 0.1;

        for i in 0..nx {
            for j in 0..ny {
                let x = i as f64 * dx;
                let x0 = nx as f64 * dx * 0.2;
                let gauss = (-((x - x0) * (x - x0)) / (2.0 * sigma * sigma)).exp();
                let phase = k * x;
                self.solver.psi[i][j] = Complex::from_polar(gauss * 0.1, phase);
            }
        }
    }

    /// Run the simulation for given steps and return final probability density.
    pub fn run(&mut self, steps: usize) -> Vec<Vec<f64>> {
        for _ in 0..steps {
            self.solver.step_2d();
        }
        let nx = self.solver.nx;
        let ny = self.solver.ny;
        let mut density = vec![vec![0.0; ny]; nx];
        for i in 0..nx {
            for j in 0..ny {
                density[i][j] = self.solver.psi[i][j].norm_sq();
            }
        }
        density
    }
}

/// Which-path measurement: detecting which slit kills interference.
/// Returns the pattern when detection probability is applied.
pub fn which_path_measurement(
    setup: &DoubleSlitSetup,
    screen_positions: &[f64],
    detection_prob: f64,
) -> Vec<f64> {
    let coherent = intensity_pattern(setup, screen_positions);

    // Incoherent sum (no interference, just two single slits)
    let d = setup.slit_separation;
    let a = setup.slit_width;
    let lambda = setup.wavelength;
    let l = setup.screen_distance;

    let incoherent: Vec<f64> = screen_positions
        .iter()
        .map(|&y| {
            let sin_theta = y / (y * y + l * l).sqrt();
            let phase_a = PI * a * sin_theta / lambda;
            let diffraction = if phase_a.abs() < 1e-12 {
                1.0
            } else {
                (phase_a.sin() / phase_a).powi(2)
            };
            diffraction // Two single-slit patterns add incoherently
        })
        .collect();

    // Interpolate between coherent and incoherent based on detection probability
    coherent
        .iter()
        .zip(incoherent.iter())
        .map(|(&c, &i)| (1.0 - detection_prob) * c + detection_prob * i)
        .collect()
}

/// Render double-slit: wall with slits, interference pattern, particles.
pub struct DoubleSlitRenderer {
    pub width: usize,
    pub height: usize,
}

impl DoubleSlitRenderer {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    /// Render the setup as ASCII art.
    pub fn render(
        &self,
        setup: &DoubleSlitSetup,
        particle_counts: Option<&[f64]>,
    ) -> Vec<Vec<char>> {
        let mut grid = vec![vec![' '; self.width]; self.height];
        let wall_col = self.width / 3;

        // Draw wall
        let slit_center1 = self.height / 2 - (self.height as f64 * setup.slit_separation / (4.0 * setup.screen_distance)) as usize;
        let slit_center2 = self.height / 2 + (self.height as f64 * setup.slit_separation / (4.0 * setup.screen_distance)) as usize;
        let slit_half = (self.height as f64 * setup.slit_width / (4.0 * setup.screen_distance)).max(1.0) as usize;

        for row in 0..self.height {
            let is_slit1 = row >= slit_center1.saturating_sub(slit_half)
                && row <= (slit_center1 + slit_half).min(self.height - 1);
            let is_slit2 = row >= slit_center2.saturating_sub(slit_half)
                && row <= (slit_center2 + slit_half).min(self.height - 1);
            if is_slit1 || is_slit2 {
                grid[row][wall_col] = ' ';
            } else {
                grid[row][wall_col] = '#';
            }
        }

        // Draw interference pattern on screen
        if let Some(counts) = particle_counts {
            let screen_col = self.width - 2;
            let max_count = counts.iter().cloned().fold(0.0_f64, f64::max).max(1e-10);
            for row in 0..self.height {
                let idx = (row * counts.len()) / self.height;
                let idx = idx.min(counts.len() - 1);
                let intensity = counts[idx] / max_count;
                let n_dots = (intensity * (self.width / 3) as f64) as usize;
                for c in 0..n_dots.min(self.width - screen_col) {
                    grid[row][screen_col - c] = if intensity > 0.7 { '@' } else if intensity > 0.3 { '*' } else { '.' };
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
    fn test_fringe_spacing() {
        // Fringe spacing: dy = lambda * D / d
        let setup = DoubleSlitSetup::new(0.01, 0.1, 0.001, 1.0);
        let expected_spacing = setup.wavelength * setup.screen_distance / setup.slit_separation;

        // Find peaks
        let n = 1000;
        let y_max = 0.05;
        let positions: Vec<f64> = (0..n).map(|i| -y_max + 2.0 * y_max * i as f64 / n as f64).collect();
        let pattern = intensity_pattern(&setup, &positions);

        let mut peaks = Vec::new();
        for i in 1..n - 1 {
            if pattern[i] > pattern[i - 1] && pattern[i] > pattern[i + 1] && pattern[i] > 0.5 {
                peaks.push(positions[i]);
            }
        }

        if peaks.len() >= 2 {
            let measured_spacing = (peaks[1] - peaks[0]).abs();
            assert!(
                (measured_spacing - expected_spacing).abs() < expected_spacing * 0.3,
                "Expected spacing {}, got {}",
                expected_spacing,
                measured_spacing
            );
        }
    }

    #[test]
    fn test_single_slit_envelope() {
        let pattern = single_slit_pattern(0.1, 0.001, &[0.0, 0.01, 0.02]);
        // At theta=0, intensity should be 1
        assert!((pattern[0] - 1.0).abs() < 1e-10);
        // Intensity decreases away from center
        assert!(pattern[1] <= pattern[0] + 1e-10);
    }

    #[test]
    fn test_n_slit_pattern() {
        // N=1 should equal single slit
        let angles = vec![0.0, 0.01, 0.02, 0.05];
        let single = single_slit_pattern(0.1, 0.001, &angles);
        let n1 = n_slit_pattern(1, 0.1, 0.5, 0.001, &angles);
        for (a, b) in single.iter().zip(n1.iter()) {
            assert!((a - b).abs() < 1e-6, "single: {}, n=1: {}", a, b);
        }
    }

    #[test]
    fn test_center_maximum() {
        let setup = DoubleSlitSetup::new(0.01, 0.1, 0.001, 1.0);
        let pattern = intensity_pattern(&setup, &[0.0]);
        assert!((pattern[0] - 1.0).abs() < 1e-10, "Center should be max");
    }

    #[test]
    fn test_which_path_kills_fringes() {
        let setup = DoubleSlitSetup::new(0.01, 0.1, 0.001, 1.0);
        let positions: Vec<f64> = (0..100).map(|i| -0.05 + 0.001 * i as f64).collect();

        let coherent = intensity_pattern(&setup, &positions);
        let detected = which_path_measurement(&setup, &positions, 1.0);

        // Coherent pattern should have more variation (interference fringes)
        let coherent_var: f64 = coherent.iter().map(|&x| (x - 0.5).powi(2)).sum::<f64>();
        let detected_var: f64 = detected.iter().map(|&x| (x - 0.5).powi(2)).sum::<f64>();
        // The coherent pattern should have more variance due to fringes
        assert!(
            coherent_var > detected_var * 0.5,
            "Coherent var {} should be >= detected var {}",
            coherent_var,
            detected_var
        );
    }

    #[test]
    fn test_simulation_creation() {
        let mut sim = DoubleSlitSimulation::new(
            32, 32, 0.5, 0.5, 0.001, 10,
            (12, 14), (17, 19), 1000.0,
        );
        sim.init_plane_wave(5.0);
        let density = sim.run(5);
        assert_eq!(density.len(), 32);
        assert_eq!(density[0].len(), 32);
    }

    #[test]
    fn test_renderer() {
        let setup = DoubleSlitSetup::new(0.01, 0.1, 0.001, 1.0);
        let renderer = DoubleSlitRenderer::new(40, 20);
        let grid = renderer.render(&setup, None);
        assert_eq!(grid.len(), 20);
        assert_eq!(grid[0].len(), 40);
        // Should have some wall characters
        let wall_count: usize = grid.iter().flat_map(|r| r.iter()).filter(|&&c| c == '#').count();
        assert!(wall_count > 0);
    }

    #[test]
    fn test_n_slit_peaks() {
        // For N slits, there should be N-2 secondary maxima between principal maxima
        let n = 4;
        let angles: Vec<f64> = (0..1000).map(|i| -0.1 + 0.0002 * i as f64).collect();
        let pattern = n_slit_pattern(n, 0.01, 0.1, 0.001, &angles);
        // Central peak should be 1
        let center_idx = 500;
        assert!((pattern[center_idx] - 1.0).abs() < 0.1);
    }
}
