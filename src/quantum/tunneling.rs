use std::f64::consts::PI;
use super::schrodinger::{Complex, WaveFunction1D, SchrodingerSolver1D};
use super::wavefunction::gaussian_wavepacket;

/// Rectangular potential barrier.
#[derive(Clone, Debug)]
pub struct RectangularBarrier {
    pub x_start: f64,
    pub x_end: f64,
    pub height: f64,
}

impl RectangularBarrier {
    pub fn new(x_start: f64, x_end: f64, height: f64) -> Self {
        Self { x_start, x_end, height }
    }

    pub fn width(&self) -> f64 {
        self.x_end - self.x_start
    }
}

/// Analytical transmission coefficient for a rectangular barrier.
/// T = 1 / (1 + V0^2 sinh^2(kappa*a) / (4 E (V0 - E))) for E < V0
/// T = 1 / (1 + V0^2 sin^2(k*a) / (4 E (E - V0))) for E > V0
pub fn transmission_coefficient(energy: f64, barrier: &RectangularBarrier) -> f64 {
    let v0 = barrier.height;
    let a = barrier.width();
    let mass = 1.0;
    let hbar = 1.0;

    if energy <= 0.0 {
        return 0.0;
    }

    if (energy - v0).abs() < 1e-12 {
        // Limiting case
        let m_a_sq = 2.0 * mass * v0 * a * a / (hbar * hbar);
        return 1.0 / (1.0 + m_a_sq / 4.0);
    }

    if energy < v0 {
        let kappa = (2.0 * mass * (v0 - energy)).sqrt() / hbar;
        let sinh_val = (kappa * a).sinh();
        1.0 / (1.0 + v0 * v0 * sinh_val * sinh_val / (4.0 * energy * (v0 - energy)))
    } else {
        let k = (2.0 * mass * (energy - v0)).sqrt() / hbar;
        let sin_val = (k * a).sin();
        1.0 / (1.0 + v0 * v0 * sin_val * sin_val / (4.0 * energy * (energy - v0)))
    }
}

/// Result of a tunneling simulation.
#[derive(Clone, Debug)]
pub struct TunnelingResult {
    pub transmitted_prob: f64,
    pub reflected_prob: f64,
    pub time_steps: Vec<Vec<f64>>,
}

/// Tunneling simulation using the Schrodinger solver.
pub struct TunnelingSimulation {
    pub solver: SchrodingerSolver1D,
    pub barrier: RectangularBarrier,
}

impl TunnelingSimulation {
    pub fn new(
        n_points: usize,
        x_min: f64,
        x_max: f64,
        barrier: RectangularBarrier,
        energy: f64,
        sigma: f64,
        mass: f64,
        hbar: f64,
        dt: f64,
    ) -> Self {
        let dx = (x_max - x_min) / (n_points - 1) as f64;
        let x_grid: Vec<f64> = (0..n_points).map(|i| x_min + i as f64 * dx).collect();

        // Initial Gaussian wave packet to the left of barrier
        let x0 = barrier.x_start - 3.0 * sigma;
        let k0 = (2.0 * mass * energy).sqrt() / hbar;
        let psi = gaussian_wavepacket(x0, k0, sigma, &x_grid);
        let mut wf = WaveFunction1D::new(psi, dx, x_min);
        wf.normalize();

        let potential: Vec<f64> = x_grid
            .iter()
            .map(|&x| {
                if x >= barrier.x_start && x <= barrier.x_end {
                    barrier.height
                } else {
                    0.0
                }
            })
            .collect();

        let solver = SchrodingerSolver1D::new(wf, potential, mass, hbar, dt);
        Self { solver, barrier }
    }

    /// Run the simulation for given number of steps.
    pub fn run(&mut self, steps: usize) -> TunnelingResult {
        let mut time_steps = Vec::new();
        let n = self.solver.psi.n();
        let dx = self.solver.psi.dx;
        let x_min = self.solver.psi.x_min;

        // Find barrier end index
        let barrier_end_idx = ((self.barrier.x_end - x_min) / dx).ceil() as usize;
        let barrier_end_idx = barrier_end_idx.min(n - 1);

        for step in 0..steps {
            self.solver.step();
            if step % (steps / 10).max(1) == 0 {
                let density: Vec<f64> = self.solver.psi.psi.iter().map(|c| c.norm_sq()).collect();
                time_steps.push(density);
            }
        }

        let density: Vec<f64> = self.solver.psi.psi.iter().map(|c| c.norm_sq()).collect();
        let transmitted_prob: f64 = density[barrier_end_idx..].iter().sum::<f64>() * dx;
        let reflected_prob: f64 = density[..barrier_end_idx].iter().sum::<f64>() * dx;

        TunnelingResult {
            transmitted_prob,
            reflected_prob,
            time_steps,
        }
    }
}

/// Resonant tunneling through a double barrier. Returns transmission vs energy.
pub fn double_barrier(
    b1: &RectangularBarrier,
    b2: &RectangularBarrier,
    energies: &[f64],
) -> Vec<f64> {
    let mass = 1.0;
    let hbar = 1.0;

    energies
        .iter()
        .map(|&e| {
            if e <= 0.0 {
                return 0.0;
            }
            // Transfer matrix method for double barrier
            let k0 = (2.0 * mass * e).sqrt() / hbar;

            let compute_barrier_matrix = |barrier: &RectangularBarrier, e: f64| -> [[Complex; 2]; 2] {
                let a = barrier.width();
                if e < barrier.height {
                    let kappa = (2.0 * mass * (barrier.height - e)).sqrt() / hbar;
                    let ch = (kappa * a).cosh();
                    let sh = (kappa * a).sinh();
                    let r = kappa / k0;
                    [
                        [Complex::new(ch, 0.0), Complex::new(sh * (r - 1.0 / r) / 2.0, 0.0)],
                        [Complex::new(sh * (1.0 / r - r) / 2.0, 0.0), Complex::new(ch, 0.0)],
                    ]
                } else {
                    let k = (2.0 * mass * (e - barrier.height)).sqrt() / hbar;
                    let c = (k * a).cos();
                    let s = (k * a).sin();
                    let r = k / k0;
                    [
                        [Complex::new(c, 0.0), Complex::new(s * (r - 1.0 / r) / 2.0, 0.0)],
                        [Complex::new(-s * (1.0 / r - r) / 2.0, 0.0), Complex::new(c, 0.0)],
                    ]
                }
            };

            // Free propagation between barriers
            let gap = b2.x_start - b1.x_end;
            let phase = k0 * gap;
            let prop = [
                [Complex::from_polar(1.0, phase), Complex::zero()],
                [Complex::zero(), Complex::from_polar(1.0, -phase)],
            ];

            let m1 = compute_barrier_matrix(b1, e);
            let m2 = compute_barrier_matrix(b2, e);

            // Multiply m2 * prop * m1
            let mul = |a: &[[Complex; 2]; 2], b: &[[Complex; 2]; 2]| -> [[Complex; 2]; 2] {
                [
                    [
                        a[0][0] * b[0][0] + a[0][1] * b[1][0],
                        a[0][0] * b[0][1] + a[0][1] * b[1][1],
                    ],
                    [
                        a[1][0] * b[0][0] + a[1][1] * b[1][0],
                        a[1][0] * b[0][1] + a[1][1] * b[1][1],
                    ],
                ]
            };

            let temp = mul(&prop, &m1);
            let total = mul(&m2, &temp);
            let t11 = total[0][0];
            1.0 / t11.norm_sq()
        })
        .collect()
}

/// WKB approximation for tunneling through an arbitrary potential.
/// T ~ exp(-2/hbar * integral sqrt(2m(V(x)-E)) dx) from x1 to x2.
pub fn wkb_approximation(potential: &[f64], energy: f64, x_grid: &[f64]) -> f64 {
    let mass = 1.0;
    let hbar = 1.0;
    let n = potential.len();
    if n < 2 {
        return 0.0;
    }
    let dx = if n > 1 { x_grid[1] - x_grid[0] } else { 1.0 };

    let mut integral = 0.0;
    for i in 0..n {
        if potential[i] > energy {
            let kappa = (2.0 * mass * (potential[i] - energy)).sqrt() / hbar;
            integral += kappa * dx;
        }
    }
    (-2.0 * integral).exp()
}

/// Render tunneling: barrier as wall glyphs, wave packet as brightness.
pub struct TunnelingRenderer {
    pub width: usize,
}

impl TunnelingRenderer {
    pub fn new(width: usize) -> Self {
        Self { width }
    }

    pub fn render(
        &self,
        solver: &SchrodingerSolver1D,
        barrier: &RectangularBarrier,
    ) -> Vec<(char, f64, f64, f64)> {
        let n = solver.psi.n();
        let dx = solver.psi.dx;
        let x_min = solver.psi.x_min;
        let mut result = Vec::with_capacity(self.width);

        for i in 0..self.width {
            let idx = (i * n) / self.width.max(1);
            let idx = idx.min(n - 1);
            let x = x_min + idx as f64 * dx;

            if x >= barrier.x_start && x <= barrier.x_end {
                result.push(('#', 0.5, 0.5, 0.5));
            } else {
                let prob = solver.psi.psi[idx].norm_sq();
                let brightness = (prob * 100.0).min(1.0);
                let phase = solver.psi.psi[idx].arg();
                let hue = (phase + PI) / (2.0 * PI);
                result.push(('.', brightness, hue, 1.0));
            }
        }
        result
    }
}

/// Gamow model for alpha decay lifetime.
/// Uses WKB approximation for Coulomb barrier tunneling.
pub fn alpha_decay_lifetime(z_daughter: u32, e_alpha_mev: f64, r_nucleus_fm: f64) -> f64 {
    let z = z_daughter as f64;
    let e = e_alpha_mev; // MeV

    // Coulomb barrier turning point
    let e_sq = 1.44; // e^2/(4 pi eps0) in MeV*fm
    let r_turn = 2.0 * z * e_sq / e; // fm

    if r_turn <= r_nucleus_fm {
        return 0.0; // No barrier
    }

    // Gamow factor
    let eta = z * e_sq * (2.0_f64 * 931.5 * 4.0).sqrt() / (2.0 * 197.3); // dimensionless, approximate
    let gamow = 2.0 * PI * eta / e.sqrt();

    // Simplified: T ~ exp(-G) where G is the Gamow factor
    // Approximate integral of Coulomb barrier
    let r_n = r_nucleus_fm;
    let r_t = r_turn;
    let mu = 4.0 * 931.5; // reduced mass in MeV/c^2 (alpha on heavy nucleus)
    let hbar_c = 197.3; // MeV*fm

    let g = (2.0_f64 * mu).sqrt() / hbar_c
        * 2.0 * z * e_sq
        * ((r_t / r_n).sqrt().acos() - (r_n / r_t * (1.0 - r_n / r_t)).sqrt())
        * r_t.sqrt();

    let transmission = (-2.0_f64 * g).exp();

    // Frequency of alpha hitting barrier ~ v/R
    let v = (2.0 * e / mu).sqrt() * 3e23; // fm/s (very rough)
    let freq = v / r_n; // attempts per second

    if transmission > 0.0 {
        1.0 / (freq * transmission) // lifetime in seconds
    } else {
        f64::INFINITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transmission_high_energy() {
        let barrier = RectangularBarrier::new(0.0, 1.0, 5.0);
        let t = transmission_coefficient(50.0, &barrier);
        // E >> V0, should be close to 1 (with oscillations)
        assert!(t > 0.5, "T at high energy: {}", t);
    }

    #[test]
    fn test_transmission_low_energy() {
        let barrier = RectangularBarrier::new(0.0, 2.0, 10.0);
        let t = transmission_coefficient(0.1, &barrier);
        // E << V0, should be very small
        assert!(t < 0.01, "T at low energy: {}", t);
    }

    #[test]
    fn test_transmission_equal_energy() {
        let barrier = RectangularBarrier::new(0.0, 1.0, 5.0);
        let t = transmission_coefficient(5.0, &barrier);
        assert!(t > 0.0 && t <= 1.0, "T at E=V0: {}", t);
    }

    #[test]
    fn test_wkb_approximation() {
        let n = 100;
        let x_grid: Vec<f64> = (0..n).map(|i| i as f64 * 0.1).collect();
        let potential: Vec<f64> = x_grid
            .iter()
            .map(|&x| if x >= 3.0 && x <= 5.0 { 10.0 } else { 0.0 })
            .collect();
        let t_high = wkb_approximation(&potential, 9.0, &x_grid);
        let t_low = wkb_approximation(&potential, 1.0, &x_grid);
        assert!(t_high > t_low, "Higher energy should tunnel more");
    }

    #[test]
    fn test_double_barrier_resonances() {
        let b1 = RectangularBarrier::new(0.0, 0.5, 10.0);
        let b2 = RectangularBarrier::new(2.0, 2.5, 10.0);
        let energies: Vec<f64> = (1..100).map(|i| i as f64 * 0.1).collect();
        let trans = double_barrier(&b1, &b2, &energies);
        // Should have resonance peaks where T approaches 1
        let max_t = trans.iter().cloned().fold(0.0_f64, f64::max);
        assert!(max_t > 0.1, "Should have resonance peak, max T: {}", max_t);
    }

    #[test]
    fn test_tunneling_simulation() {
        let barrier = RectangularBarrier::new(3.0, 3.5, 5.0);
        let mut sim = TunnelingSimulation::new(
            256, -5.0, 10.0, barrier, 3.0, 1.0, 1.0, 1.0, 0.001,
        );
        let result = sim.run(100);
        let total = result.transmitted_prob + result.reflected_prob;
        // Total probability should be approximately conserved
        assert!(total > 0.5 && total < 1.5, "Total prob: {}", total);
    }

    #[test]
    fn test_renderer() {
        let barrier = RectangularBarrier::new(3.0, 3.5, 5.0);
        let sim = TunnelingSimulation::new(
            128, -5.0, 10.0, barrier.clone(), 3.0, 1.0, 1.0, 1.0, 0.001,
        );
        let renderer = TunnelingRenderer::new(40);
        let rendered = renderer.render(&sim.solver, &barrier);
        assert_eq!(rendered.len(), 40);
        // Should have some barrier characters
        let barrier_chars: usize = rendered.iter().filter(|&&(c, _, _, _)| c == '#').count();
        assert!(barrier_chars > 0);
    }

    #[test]
    fn test_alpha_decay() {
        // Polonium-212 alpha decay: Z_daughter=82, E_alpha~8.78 MeV, R~7.1 fm
        let lifetime = alpha_decay_lifetime(82, 8.78, 7.1);
        // Should be a very short lifetime (sub-microsecond)
        assert!(lifetime > 0.0 && lifetime.is_finite());
    }
}
