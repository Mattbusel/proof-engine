use std::f64::consts::PI;
use super::schrodinger::Complex;

/// Single qubit state: |psi> = alpha|0> + beta|1>.
#[derive(Clone, Debug)]
pub struct QubitState {
    pub alpha: Complex,
    pub beta: Complex,
}

impl QubitState {
    pub fn new(alpha: Complex, beta: Complex) -> Self {
        Self { alpha, beta }
    }

    pub fn zero() -> Self {
        Self { alpha: Complex::one(), beta: Complex::zero() }
    }

    pub fn one() -> Self {
        Self { alpha: Complex::zero(), beta: Complex::one() }
    }

    pub fn norm_sq(&self) -> f64 {
        self.alpha.norm_sq() + self.beta.norm_sq()
    }

    pub fn normalize(&mut self) {
        let n = self.norm_sq().sqrt();
        if n > 1e-30 {
            self.alpha = self.alpha / n;
            self.beta = self.beta / n;
        }
    }
}

/// Two-qubit state: amplitudes for |00>, |01>, |10>, |11>.
#[derive(Clone, Debug)]
pub struct TwoQubitState {
    pub amplitudes: [Complex; 4],
}

impl TwoQubitState {
    pub fn new(amplitudes: [Complex; 4]) -> Self {
        Self { amplitudes }
    }

    /// Product state |a>|b>.
    pub fn product(a: &QubitState, b: &QubitState) -> Self {
        Self {
            amplitudes: [
                a.alpha * b.alpha, // |00>
                a.alpha * b.beta,  // |01>
                a.beta * b.alpha,  // |10>
                a.beta * b.beta,   // |11>
            ],
        }
    }

    pub fn norm_sq(&self) -> f64 {
        self.amplitudes.iter().map(|c| c.norm_sq()).sum()
    }

    pub fn normalize(&mut self) {
        let n = self.norm_sq().sqrt();
        if n > 1e-30 {
            for a in &mut self.amplitudes {
                *a = *a / n;
            }
        }
    }
}

/// Create a Bell state.
/// 0: Phi+ = (|00> + |11>)/sqrt(2)
/// 1: Phi- = (|00> - |11>)/sqrt(2)
/// 2: Psi+ = (|01> + |10>)/sqrt(2)
/// 3: Psi- = (|01> - |10>)/sqrt(2)
pub fn bell_state(which: u8) -> TwoQubitState {
    let s = 1.0 / 2.0_f64.sqrt();
    match which {
        0 => TwoQubitState::new([
            Complex::new(s, 0.0), Complex::zero(),
            Complex::zero(), Complex::new(s, 0.0),
        ]),
        1 => TwoQubitState::new([
            Complex::new(s, 0.0), Complex::zero(),
            Complex::zero(), Complex::new(-s, 0.0),
        ]),
        2 => TwoQubitState::new([
            Complex::zero(), Complex::new(s, 0.0),
            Complex::new(s, 0.0), Complex::zero(),
        ]),
        _ => TwoQubitState::new([
            Complex::zero(), Complex::new(s, 0.0),
            Complex::new(-s, 0.0), Complex::zero(),
        ]),
    }
}

/// Measure one qubit of a two-qubit state.
/// Returns (outcome, collapsed state of the other qubit).
pub fn measure_qubit(state: &TwoQubitState, which: usize, rng_val: f64) -> (u8, QubitState) {
    let a = &state.amplitudes;
    if which == 0 {
        // Measuring first qubit
        let p0 = a[0].norm_sq() + a[1].norm_sq(); // prob of first qubit = 0
        if rng_val < p0 {
            // Outcome 0: remaining state is alpha|0> + beta|1> from a[0]|0> + a[1]|1>
            let mut q = QubitState::new(a[0], a[1]);
            q.normalize();
            (0, q)
        } else {
            let mut q = QubitState::new(a[2], a[3]);
            q.normalize();
            (1, q)
        }
    } else {
        // Measuring second qubit
        let p0 = a[0].norm_sq() + a[2].norm_sq();
        if rng_val < p0 {
            let mut q = QubitState::new(a[0], a[2]);
            q.normalize();
            (0, q)
        } else {
            let mut q = QubitState::new(a[1], a[3]);
            q.normalize();
            (1, q)
        }
    }
}

/// 2x2 density matrix.
#[derive(Clone, Debug)]
pub struct DensityMatrix2x2 {
    pub rho: [[Complex; 2]; 2],
}

impl DensityMatrix2x2 {
    pub fn trace(&self) -> f64 {
        (self.rho[0][0] + self.rho[1][1]).re
    }

    pub fn purity(&self) -> f64 {
        let mut sum = Complex::zero();
        for i in 0..2 {
            for j in 0..2 {
                sum += self.rho[i][j] * self.rho[j][i];
            }
        }
        sum.re
    }

    pub fn is_mixed(&self) -> bool {
        self.purity() < 1.0 - 1e-6
    }
}

/// Partial trace: trace out one qubit to get the density matrix of the other.
pub fn partial_trace(state: &TwoQubitState, trace_out: usize) -> DensityMatrix2x2 {
    let a = &state.amplitudes;
    if trace_out == 1 {
        // Trace out second qubit -> density matrix of first
        let rho00 = a[0] * a[0].conj() + a[1] * a[1].conj();
        let rho01 = a[0] * a[2].conj() + a[1] * a[3].conj();
        let rho10 = a[2] * a[0].conj() + a[3] * a[1].conj();
        let rho11 = a[2] * a[2].conj() + a[3] * a[3].conj();
        DensityMatrix2x2 { rho: [[rho00, rho01], [rho10, rho11]] }
    } else {
        // Trace out first qubit -> density matrix of second
        let rho00 = a[0] * a[0].conj() + a[2] * a[2].conj();
        let rho01 = a[0] * a[1].conj() + a[2] * a[3].conj();
        let rho10 = a[1] * a[0].conj() + a[3] * a[2].conj();
        let rho11 = a[1] * a[1].conj() + a[3] * a[3].conj();
        DensityMatrix2x2 { rho: [[rho00, rho01], [rho10, rho11]] }
    }
}

/// Concurrence: entanglement measure for two-qubit pure states.
/// C = 2|ad - bc| where state = a|00> + b|01> + c|10> + d|11>.
pub fn concurrence(state: &TwoQubitState) -> f64 {
    let a = state.amplitudes[0];
    let b = state.amplitudes[1];
    let c = state.amplitudes[2];
    let d = state.amplitudes[3];
    2.0 * (a * d - b * c).norm()
}

/// CHSH correlation: S = E(a1,b1) - E(a1,b2) + E(a2,b1) + E(a2,b2).
/// Each angle specifies a measurement axis in the XZ plane.
/// Returns |S|, which violates Bell inequality when > 2.
pub fn chsh_correlation(
    state: &TwoQubitState,
    a1: f64,
    a2: f64,
    b1: f64,
    b2: f64,
) -> f64 {
    let e = |a_angle: f64, b_angle: f64| -> f64 {
        // E(a,b) = <psi| (sigma_a tensor sigma_b) |psi>
        // sigma_n = cos(theta)*sigma_z + sin(theta)*sigma_x for angle theta
        let ca = a_angle.cos();
        let sa = a_angle.sin();
        let cb = b_angle.cos();
        let sb = b_angle.sin();

        let amp = &state.amplitudes;
        // Compute <psi| A tensor B |psi>
        // A = [[ca, sa],[sa, -ca]], B = [[cb, sb],[sb, -cb]]
        // A tensor B is 4x4
        let mut result = Complex::zero();
        let a_mat = [[ca, sa], [sa, -ca]];
        let b_mat = [[cb, sb], [sb, -cb]];

        for i in 0..2 {
            for j in 0..2 {
                let bra_idx = i * 2 + j;
                for k in 0..2 {
                    for l in 0..2 {
                        let ket_idx = k * 2 + l;
                        let coeff = a_mat[i][k] * b_mat[j][l];
                        result += amp[bra_idx].conj() * amp[ket_idx] * coeff;
                    }
                }
            }
        }
        result.re
    };

    let s = e(a1, b1) - e(a1, b2) + e(a2, b1) + e(a2, b2);
    s.abs()
}

/// Renderer for entanglement visualization.
pub struct EntanglementRenderer {
    pub width: usize,
}

impl EntanglementRenderer {
    pub fn new(width: usize) -> Self {
        Self { width }
    }

    /// Render two particles with correlated states.
    pub fn render(&self, state: &TwoQubitState, measured: Option<(u8, u8)>) -> Vec<(char, f64, f64, f64)> {
        let mut result = Vec::with_capacity(self.width);
        let mid = self.width / 2;

        for i in 0..self.width {
            if let Some((m0, m1)) = measured {
                // After measurement: show definite states
                if i < mid {
                    let ch = if m0 == 0 { '0' } else { '1' };
                    result.push((ch, 0.0, 1.0, 0.0));
                } else {
                    let ch = if m1 == 0 { '0' } else { '1' };
                    result.push((ch, 1.0, 0.0, 0.0));
                }
            } else {
                // Before measurement: show superposition
                if i == mid - 2 || i == mid + 1 {
                    let prob = if i < mid {
                        state.amplitudes[0].norm_sq() + state.amplitudes[1].norm_sq()
                    } else {
                        state.amplitudes[0].norm_sq() + state.amplitudes[2].norm_sq()
                    };
                    let brightness = prob.min(1.0);
                    result.push(('*', brightness, brightness, 0.5));
                } else if i == mid - 1 || i == mid {
                    result.push(('~', 0.3, 0.3, 0.8)); // entanglement link
                } else {
                    result.push((' ', 0.0, 0.0, 0.0));
                }
            }
        }
        result
    }
}

/// N-particle GHZ state: (|00...0> + |11...1>)/sqrt(2).
#[derive(Clone, Debug)]
pub struct GHZState {
    pub n_qubits: usize,
    pub amplitudes: Vec<Complex>,
}

impl GHZState {
    pub fn new(n_qubits: usize) -> Self {
        let size = 1 << n_qubits;
        let mut amplitudes = vec![Complex::zero(); size];
        let s = 1.0 / 2.0_f64.sqrt();
        amplitudes[0] = Complex::new(s, 0.0);             // |00...0>
        amplitudes[size - 1] = Complex::new(s, 0.0);       // |11...1>
        Self { n_qubits, amplitudes }
    }

    pub fn norm_sq(&self) -> f64 {
        self.amplitudes.iter().map(|c| c.norm_sq()).sum()
    }

    /// Measure all qubits. Returns bit string.
    pub fn measure(&self, rng_val: f64) -> Vec<u8> {
        let n = self.amplitudes.len();
        let mut cumulative = 0.0;
        let mut outcome = 0;
        for i in 0..n {
            cumulative += self.amplitudes[i].norm_sq();
            if rng_val < cumulative {
                outcome = i;
                break;
            }
        }
        // Convert outcome to bits
        (0..self.n_qubits)
            .map(|bit| ((outcome >> (self.n_qubits - 1 - bit)) & 1) as u8)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bell_states_normalized() {
        for i in 0..4 {
            let state = bell_state(i);
            let norm = state.norm_sq();
            assert!((norm - 1.0).abs() < 1e-10, "Bell state {} norm: {}", i, norm);
        }
    }

    #[test]
    fn test_bell_state_maximally_entangled() {
        for i in 0..4 {
            let state = bell_state(i);
            let c = concurrence(&state);
            assert!((c - 1.0).abs() < 1e-10, "Bell state {} concurrence: {}", i, c);
        }
    }

    #[test]
    fn test_product_state_not_entangled() {
        let a = QubitState::zero();
        let b = QubitState::zero();
        let state = TwoQubitState::product(&a, &b);
        let c = concurrence(&state);
        assert!(c < 1e-10, "Product state concurrence: {}", c);
    }

    #[test]
    fn test_measurement_correlation() {
        // For Phi+, measuring qubit 0 as 0 should give qubit 1 as 0
        let state = bell_state(0); // Phi+
        let (outcome, remaining) = measure_qubit(&state, 0, 0.1); // force outcome 0
        if outcome == 0 {
            // Remaining qubit should be |0>
            assert!(remaining.alpha.norm_sq() > 0.9);
        } else {
            // Remaining qubit should be |1>
            assert!(remaining.beta.norm_sq() > 0.9);
        }
    }

    #[test]
    fn test_partial_trace_bell_gives_mixed() {
        let state = bell_state(0);
        let rho = partial_trace(&state, 1);
        assert!(rho.is_mixed(), "Partial trace of Bell state should be mixed");
        let purity = rho.purity();
        assert!((purity - 0.5).abs() < 1e-10, "Purity: {}", purity);
    }

    #[test]
    fn test_partial_trace_product_gives_pure() {
        let a = QubitState::new(
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
            Complex::new(1.0 / 2.0_f64.sqrt(), 0.0),
        );
        let b = QubitState::zero();
        let state = TwoQubitState::product(&a, &b);
        let rho = partial_trace(&state, 1);
        let purity = rho.purity();
        assert!((purity - 1.0).abs() < 1e-10, "Product state purity: {}", purity);
    }

    #[test]
    fn test_chsh_violation() {
        // For Phi+, optimal angles give S = 2*sqrt(2) ~ 2.828
        let state = bell_state(0);
        let s = chsh_correlation(&state, 0.0, PI / 2.0, PI / 4.0, -PI / 4.0);
        assert!(s > 2.0, "CHSH S = {} should violate Bell inequality (> 2)", s);
        assert!((s - 2.0 * 2.0_f64.sqrt()).abs() < 0.3, "S = {} should be ~2.828", s);
    }

    #[test]
    fn test_chsh_classical_bound() {
        // Product state should not violate
        let state = TwoQubitState::product(&QubitState::zero(), &QubitState::zero());
        let s = chsh_correlation(&state, 0.0, PI / 2.0, PI / 4.0, -PI / 4.0);
        assert!(s <= 2.1, "Product state S = {} should be <= 2", s);
    }

    #[test]
    fn test_ghz_state() {
        let ghz = GHZState::new(3);
        assert_eq!(ghz.amplitudes.len(), 8);
        let norm = ghz.norm_sq();
        assert!((norm - 1.0).abs() < 1e-10);

        // Measurement should give all 0s or all 1s
        let result_0 = ghz.measure(0.1);
        assert!(result_0.iter().all(|&b| b == 0) || result_0.iter().all(|&b| b == 1));
        let result_1 = ghz.measure(0.9);
        assert!(result_1.iter().all(|&b| b == 0) || result_1.iter().all(|&b| b == 1));
    }

    #[test]
    fn test_renderer() {
        let state = bell_state(0);
        let renderer = EntanglementRenderer::new(20);
        let result = renderer.render(&state, None);
        assert_eq!(result.len(), 20);
        let measured = renderer.render(&state, Some((0, 0)));
        assert_eq!(measured.len(), 20);
    }
}
