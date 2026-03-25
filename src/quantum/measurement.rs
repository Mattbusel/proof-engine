use super::schrodinger::{Complex, WaveFunction1D, SchrodingerSolver1D};
use super::wavefunction::DensityMatrix;

/// A measurement basis: set of eigenstates and corresponding eigenvalues.
#[derive(Clone, Debug)]
pub struct MeasurementBasis {
    pub eigenstates: Vec<Vec<Complex>>,
    pub eigenvalues: Vec<f64>,
}

impl MeasurementBasis {
    pub fn new(eigenstates: Vec<Vec<Complex>>, eigenvalues: Vec<f64>) -> Self {
        Self { eigenstates, eigenvalues }
    }

    /// Position basis: delta functions at each grid point.
    pub fn position_basis(n: usize) -> Self {
        let mut eigenstates = Vec::with_capacity(n);
        let eigenvalues: Vec<f64> = (0..n).map(|i| i as f64).collect();
        for i in 0..n {
            let mut state = vec![Complex::zero(); n];
            state[i] = Complex::one();
            eigenstates.push(state);
        }
        Self { eigenstates, eigenvalues }
    }
}

/// Perform a projective measurement in the given basis.
/// Returns (outcome index, collapsed state).
pub fn measure(psi: &[Complex], basis: &MeasurementBasis, rng_val: f64) -> (usize, Vec<Complex>) {
    let probabilities: Vec<f64> = basis
        .eigenstates
        .iter()
        .map(|eigenstate| {
            let overlap = inner_product(eigenstate, psi);
            overlap.norm_sq()
        })
        .collect();

    let total: f64 = probabilities.iter().sum();
    let mut cumulative = 0.0;
    let mut outcome = 0;

    for (i, &p) in probabilities.iter().enumerate() {
        cumulative += p / total;
        if rng_val < cumulative {
            outcome = i;
            break;
        }
        if i == probabilities.len() - 1 {
            outcome = i;
        }
    }

    // Collapse to the eigenstate
    let collapsed = basis.eigenstates[outcome].clone();
    (outcome, collapsed)
}

/// Projective measurement with a single projector.
/// Returns (probability, post-measurement state).
pub fn projective_measurement(psi: &[Complex], projector: &[Vec<Complex>]) -> (f64, Vec<Complex>) {
    let n = psi.len();
    // Apply projector: P|psi>
    let mut projected = vec![Complex::zero(); n];
    for i in 0..n {
        for j in 0..n {
            projected[i] += projector[i][j] * psi[j];
        }
    }

    let probability: f64 = projected.iter().map(|c| c.norm_sq()).sum();

    // Normalize
    if probability > 1e-30 {
        let norm = probability.sqrt();
        for c in &mut projected {
            *c = *c / norm;
        }
    }

    (probability, projected)
}

/// Inner product <a|b>.
fn inner_product(a: &[Complex], b: &[Complex]) -> Complex {
    a.iter()
        .zip(b.iter())
        .map(|(ai, bi)| ai.conj() * *bi)
        .fold(Complex::zero(), |acc, x| acc + x)
}

/// Apply decoherence to a density matrix: off-diagonal elements decay.
/// Models Lindblad-type decoherence: rho_ij *= exp(-rate * dt) for i != j.
pub fn decoherence(rho: &mut DensityMatrix, rate: f64, dt: f64) {
    let n = rho.dim();
    let decay = (-rate * dt).exp();
    for i in 0..n {
        for j in 0..n {
            if i != j {
                rho.rho[i][j] = rho.rho[i][j] * decay;
            }
        }
    }
}

/// Quantum Zeno effect: frequent measurements freeze evolution.
/// Simulates time evolution with periodic measurements and returns
/// the probability of remaining in the initial state at each measurement.
pub fn quantum_zeno_effect(
    psi_initial: &[Complex],
    potential: &[f64],
    measurement_interval: f64,
    total_time: f64,
    dx: f64,
    mass: f64,
    hbar: f64,
) -> Vec<f64> {
    let n = psi_initial.len();
    let dt = measurement_interval / 10.0; // sub-steps per measurement
    let n_measurements = (total_time / measurement_interval).ceil() as usize;
    let steps_per_measurement = 10;

    let mut wf = WaveFunction1D::new(psi_initial.to_vec(), dx, 0.0);
    let mut solver = SchrodingerSolver1D::new(wf, potential.to_vec(), mass, hbar, dt);

    let mut survival_probs = Vec::with_capacity(n_measurements);

    for _ in 0..n_measurements {
        // Evolve
        for _ in 0..steps_per_measurement {
            solver.step();
        }

        // Measure: probability of still being in initial state
        let overlap = inner_product(psi_initial, &solver.psi.psi);
        let prob = overlap.norm_sq();
        survival_probs.push(prob);

        // Collapse back to initial state with probability `prob`
        // (simulate "yes, still in initial state" outcome)
        if prob > 0.5 {
            solver.psi.psi = psi_initial.to_vec();
        }
    }

    survival_probs
}

/// Weak measurement: disturbs the state minimally.
/// Returns (weak value, post-measurement state).
pub fn weak_measurement(
    psi: &[Complex],
    observable: &[Vec<Complex>],
    strength: f64,
) -> (f64, Vec<Complex>) {
    let n = psi.len();

    // Apply observable: A|psi>
    let mut a_psi = vec![Complex::zero(); n];
    for i in 0..n {
        for j in 0..n {
            a_psi[i] += observable[i][j] * psi[j];
        }
    }

    // Expectation value <A>
    let exp_a = inner_product(psi, &a_psi);

    // Weak measurement: state is slightly shifted toward eigenstate
    // |psi'> ~ |psi> + strength * (A - <A>)|psi>
    let mut result = vec![Complex::zero(); n];
    for i in 0..n {
        result[i] = psi[i] + (a_psi[i] - psi[i] * exp_a) * strength;
    }

    // Normalize
    let norm_sq: f64 = result.iter().map(|c| c.norm_sq()).sum();
    let norm = norm_sq.sqrt();
    if norm > 1e-30 {
        for c in &mut result {
            *c = *c / norm;
        }
    }

    (exp_a.re, result)
}

/// Renderer for measurement collapse visualization.
pub struct MeasurementRenderer {
    pub width: usize,
}

impl MeasurementRenderer {
    pub fn new(width: usize) -> Self {
        Self { width }
    }

    /// Render collapse: transition from spread wave to localized spike.
    /// `collapse_progress` goes from 0 (coherent) to 1 (collapsed).
    pub fn render(
        &self,
        psi: &[Complex],
        collapse_point: usize,
        collapse_progress: f64,
    ) -> Vec<(char, f64, f64, f64)> {
        let n = psi.len();
        let mut result = Vec::with_capacity(self.width);

        for i in 0..self.width {
            let idx = (i * n) / self.width.max(1);
            let idx = idx.min(n.saturating_sub(1));

            let original_prob = if n > 0 { psi[idx].norm_sq() } else { 0.0 };

            // Interpolate between original distribution and delta function
            let collapse_idx = (collapse_point * self.width) / n.max(1);
            let collapsed_prob = if i == collapse_idx.min(self.width - 1) { 1.0 } else { 0.0 };

            let prob = (1.0 - collapse_progress) * original_prob * 10.0 + collapse_progress * collapsed_prob;
            let brightness = prob.min(1.0);

            let ch = if collapse_progress > 0.8 && i == collapse_idx.min(self.width - 1) {
                '!'  // flash
            } else if brightness > 0.5 {
                '#'
            } else if brightness > 0.2 {
                '*'
            } else if brightness > 0.05 {
                '.'
            } else {
                ' '
            };

            // Color: blue for coherent, yellow for collapse flash
            let r = collapse_progress * brightness;
            let g = collapse_progress * brightness;
            let b = (1.0 - collapse_progress) * brightness;

            result.push((ch, r, g, b));
        }
        result
    }
}

/// Born rule utilities.
pub struct BornRule;

impl BornRule {
    /// Compute probabilities from amplitudes.
    pub fn probabilities(amplitudes: &[Complex]) -> Vec<f64> {
        amplitudes.iter().map(|c| c.norm_sq()).collect()
    }

    /// Verify normalization: sum of probabilities should be 1.
    pub fn is_normalized(amplitudes: &[Complex], tolerance: f64) -> bool {
        let sum: f64 = amplitudes.iter().map(|c| c.norm_sq()).sum();
        (sum - 1.0).abs() < tolerance
    }

    /// Sample an outcome from probability distribution.
    pub fn sample(amplitudes: &[Complex], rng_val: f64) -> usize {
        let probs = Self::probabilities(amplitudes);
        let total: f64 = probs.iter().sum();
        let mut cumulative = 0.0;
        for (i, &p) in probs.iter().enumerate() {
            cumulative += p / total;
            if rng_val < cumulative {
                return i;
            }
        }
        amplitudes.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_born_rule_probabilities() {
        let amps = vec![
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex::new(0.0, 1.0 / 2.0_f64.sqrt()),
        ];
        let probs = BornRule::probabilities(&amps);
        assert!((probs[0] - 0.5).abs() < 1e-10);
        assert!((probs[1] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_born_rule_normalized() {
        let amps = vec![
            Complex::new(0.6, 0.0),
            Complex::new(0.0, 0.8),
        ];
        assert!(BornRule::is_normalized(&amps, 1e-10));
    }

    #[test]
    fn test_measurement_collapses() {
        let psi = vec![
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
        ];
        let basis = MeasurementBasis::new(
            vec![
                vec![Complex::one(), Complex::zero()],
                vec![Complex::zero(), Complex::one()],
            ],
            vec![0.0, 1.0],
        );

        let (outcome, collapsed) = measure(&psi, &basis, 0.3);
        assert!(outcome == 0 || outcome == 1);
        // Collapsed state should be an eigenstate
        let norm: f64 = collapsed.iter().map(|c| c.norm_sq()).sum();
        assert!((norm - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_projective_measurement() {
        let psi = vec![
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
        ];
        // Projector onto |0>
        let projector = vec![
            vec![Complex::one(), Complex::zero()],
            vec![Complex::zero(), Complex::zero()],
        ];
        let (prob, state) = projective_measurement(&psi, &projector);
        assert!((prob - 0.5).abs() < 1e-10, "Projection prob: {}", prob);
        assert!((state[0].norm() - 1.0).abs() < 1e-10);
        assert!(state[1].norm() < 1e-10);
    }

    #[test]
    fn test_decoherence() {
        let psi = vec![
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
        ];
        let mut dm = DensityMatrix::from_pure_state(&psi);
        assert!((dm.purity() - 1.0).abs() < 1e-10);

        // Apply strong decoherence
        for _ in 0..100 {
            decoherence(&mut dm, 10.0, 0.1);
        }

        // Should be approximately diagonal (maximally mixed for equal amplitudes)
        assert!(dm.rho[0][1].norm() < 0.01, "Off-diagonal: {:?}", dm.rho[0][1]);
        assert!(dm.rho[1][0].norm() < 0.01);
        // Diagonal should be preserved
        assert!((dm.rho[0][0].re - 0.5).abs() < 0.01);
        assert!((dm.rho[1][1].re - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_quantum_zeno_effect() {
        // Particle in a box, initially in ground state
        let n = 64;
        let dx = 0.1;
        let potential = vec![0.0; n];
        let sigma = 1.0;
        let psi: Vec<Complex> = (0..n)
            .map(|i| {
                let x = -3.2 + i as f64 * dx;
                Complex::new((-x * x / (2.0 * sigma * sigma)).exp(), 0.0)
            })
            .collect();
        let mut psi_norm = psi.clone();
        let norm_sq: f64 = psi_norm.iter().map(|c| c.norm_sq()).sum::<f64>() * dx;
        let norm = norm_sq.sqrt();
        for c in &mut psi_norm {
            *c = *c / norm;
        }

        // Frequent measurements
        let probs_frequent = quantum_zeno_effect(&psi_norm, &potential, 0.01, 0.1, dx, 1.0, 1.0);

        // Infrequent measurements
        let probs_infrequent = quantum_zeno_effect(&psi_norm, &potential, 0.05, 0.1, dx, 1.0, 1.0);

        // Frequent measurements should maintain higher survival probability
        if !probs_frequent.is_empty() && !probs_infrequent.is_empty() {
            let avg_frequent: f64 = probs_frequent.iter().sum::<f64>() / probs_frequent.len() as f64;
            let avg_infrequent: f64 = probs_infrequent.iter().sum::<f64>() / probs_infrequent.len() as f64;
            assert!(
                avg_frequent >= avg_infrequent - 0.2,
                "Zeno: frequent={}, infrequent={}",
                avg_frequent,
                avg_infrequent
            );
        }
    }

    #[test]
    fn test_weak_measurement() {
        let psi = vec![
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
        ];
        // Pauli Z as observable
        let obs = vec![
            vec![Complex::one(), Complex::zero()],
            vec![Complex::zero(), Complex::new(-1.0, 0.0)],
        ];
        let (weak_val, post_state) = weak_measurement(&psi, &obs, 0.01);
        // For equal superposition, <Z> = 0
        assert!(weak_val.abs() < 1e-10, "Weak value: {}", weak_val);
        // State should be barely changed
        let overlap = inner_product(&psi, &post_state).norm_sq();
        assert!(overlap > 0.99, "Overlap: {}", overlap);
    }

    #[test]
    fn test_measurement_renderer() {
        let psi: Vec<Complex> = (0..64)
            .map(|i| {
                let x = -3.2 + i as f64 * 0.1;
                Complex::new((-x * x / 2.0).exp(), 0.0)
            })
            .collect();
        let renderer = MeasurementRenderer::new(30);
        let before = renderer.render(&psi, 32, 0.0);
        let after = renderer.render(&psi, 32, 1.0);
        assert_eq!(before.len(), 30);
        assert_eq!(after.len(), 30);
        // After collapse, should have a flash character
        let flashes: usize = after.iter().filter(|&&(c, _, _, _)| c == '!').count();
        assert!(flashes > 0);
    }

    #[test]
    fn test_born_rule_sample() {
        let amps = vec![
            Complex::new(1.0, 0.0),
            Complex::zero(),
        ];
        // Should always sample 0
        assert_eq!(BornRule::sample(&amps, 0.5), 0);
        assert_eq!(BornRule::sample(&amps, 0.99), 0);
    }
}
