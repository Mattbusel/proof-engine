use std::f64::consts::PI;
use super::schrodinger::Complex;
use super::entanglement::QubitState;

/// Alias for single qubit.
pub type Qubit = QubitState;

/// Quantum register: n qubits with 2^n amplitudes.
#[derive(Clone, Debug)]
pub struct QuantumRegister {
    pub n_qubits: usize,
    pub state: Vec<Complex>,
}

impl QuantumRegister {
    pub fn new(n_qubits: usize) -> Self {
        let size = 1 << n_qubits;
        let mut state = vec![Complex::zero(); size];
        state[0] = Complex::one(); // |00...0>
        Self { n_qubits, state }
    }

    pub fn from_state(n_qubits: usize, state: Vec<Complex>) -> Self {
        Self { n_qubits, state }
    }

    pub fn size(&self) -> usize {
        self.state.len()
    }

    pub fn norm_sq(&self) -> f64 {
        self.state.iter().map(|c| c.norm_sq()).sum()
    }

    pub fn normalize(&mut self) {
        let n = self.norm_sq().sqrt();
        if n > 1e-30 {
            for c in &mut self.state {
                *c = *c / n;
            }
        }
    }
}

/// Quantum gate: a unitary matrix.
#[derive(Clone, Debug)]
pub struct QuantumGate {
    pub matrix: Vec<Vec<Complex>>,
    pub n_qubits: usize,
    pub name: String,
}

impl QuantumGate {
    pub fn new(matrix: Vec<Vec<Complex>>, n_qubits: usize, name: &str) -> Self {
        Self { matrix, n_qubits, name: name.to_string() }
    }

    pub fn dim(&self) -> usize {
        self.matrix.len()
    }
}

// --- Standard gates ---

pub fn hadamard() -> QuantumGate {
    let s = 1.0 / 2.0_f64.sqrt();
    QuantumGate::new(
        vec![
            vec![Complex::new(s, 0.0), Complex::new(s, 0.0)],
            vec![Complex::new(s, 0.0), Complex::new(-s, 0.0)],
        ],
        1,
        "H",
    )
}

pub fn pauli_x() -> QuantumGate {
    QuantumGate::new(
        vec![
            vec![Complex::zero(), Complex::one()],
            vec![Complex::one(), Complex::zero()],
        ],
        1,
        "X",
    )
}

pub fn pauli_y() -> QuantumGate {
    QuantumGate::new(
        vec![
            vec![Complex::zero(), Complex::new(0.0, -1.0)],
            vec![Complex::new(0.0, 1.0), Complex::zero()],
        ],
        1,
        "Y",
    )
}

pub fn pauli_z() -> QuantumGate {
    QuantumGate::new(
        vec![
            vec![Complex::one(), Complex::zero()],
            vec![Complex::zero(), Complex::new(-1.0, 0.0)],
        ],
        1,
        "Z",
    )
}

pub fn phase(theta: f64) -> QuantumGate {
    QuantumGate::new(
        vec![
            vec![Complex::one(), Complex::zero()],
            vec![Complex::zero(), Complex::from_polar(1.0, theta)],
        ],
        1,
        "P",
    )
}

pub fn t_gate() -> QuantumGate {
    phase(PI / 4.0)
}

pub fn cnot() -> QuantumGate {
    QuantumGate::new(
        vec![
            vec![Complex::one(), Complex::zero(), Complex::zero(), Complex::zero()],
            vec![Complex::zero(), Complex::one(), Complex::zero(), Complex::zero()],
            vec![Complex::zero(), Complex::zero(), Complex::zero(), Complex::one()],
            vec![Complex::zero(), Complex::zero(), Complex::one(), Complex::zero()],
        ],
        2,
        "CNOT",
    )
}

pub fn swap() -> QuantumGate {
    QuantumGate::new(
        vec![
            vec![Complex::one(), Complex::zero(), Complex::zero(), Complex::zero()],
            vec![Complex::zero(), Complex::zero(), Complex::one(), Complex::zero()],
            vec![Complex::zero(), Complex::one(), Complex::zero(), Complex::zero()],
            vec![Complex::zero(), Complex::zero(), Complex::zero(), Complex::one()],
        ],
        2,
        "SWAP",
    )
}

pub fn toffoli() -> QuantumGate {
    let mut m = vec![vec![Complex::zero(); 8]; 8];
    for i in 0..6 {
        m[i][i] = Complex::one();
    }
    // Swap |110> and |111>
    m[6][7] = Complex::one();
    m[7][6] = Complex::one();
    QuantumGate::new(m, 3, "Toffoli")
}

/// Apply a gate to specific target qubits in a register.
pub fn apply_gate(register: &QuantumRegister, gate: &QuantumGate, target_qubits: &[usize]) -> QuantumRegister {
    let n = register.n_qubits;
    let size = register.size();
    let g_dim = gate.dim();

    let mut new_state = vec![Complex::zero(); size];

    for i in 0..size {
        if register.state[i].norm_sq() < 1e-30 {
            continue;
        }

        // Extract target qubit values from state index i
        let target_val = extract_bits(i, target_qubits, n);

        // For each possible output of the gate
        for out_val in 0..g_dim {
            let coeff = gate.matrix[out_val][target_val];
            if coeff.norm_sq() < 1e-30 {
                continue;
            }
            // Compute the output state index
            let j = replace_bits(i, target_qubits, out_val, n);
            new_state[j] += register.state[i] * coeff;
        }
    }

    QuantumRegister::from_state(n, new_state)
}

/// Extract bits at given positions from a state index.
fn extract_bits(state_idx: usize, positions: &[usize], n_qubits: usize) -> usize {
    let mut val = 0;
    for (k, &pos) in positions.iter().enumerate() {
        let bit = (state_idx >> (n_qubits - 1 - pos)) & 1;
        val |= bit << (positions.len() - 1 - k);
    }
    val
}

/// Replace bits at given positions in a state index.
fn replace_bits(state_idx: usize, positions: &[usize], new_val: usize, n_qubits: usize) -> usize {
    let mut result = state_idx;
    for (k, &pos) in positions.iter().enumerate() {
        let bit = (new_val >> (positions.len() - 1 - k)) & 1;
        let mask = 1 << (n_qubits - 1 - pos);
        if bit == 1 {
            result |= mask;
        } else {
            result &= !mask;
        }
    }
    result
}

/// Measure all qubits in a register.
pub fn measure_register(register: &QuantumRegister, rng_val: f64) -> (Vec<u8>, QuantumRegister) {
    let n = register.n_qubits;
    let size = register.size();
    let mut cumulative = 0.0;
    let mut outcome = 0;

    for i in 0..size {
        cumulative += register.state[i].norm_sq();
        if rng_val < cumulative {
            outcome = i;
            break;
        }
        if i == size - 1 {
            outcome = i;
        }
    }

    let bits: Vec<u8> = (0..n)
        .map(|bit| ((outcome >> (n - 1 - bit)) & 1) as u8)
        .collect();

    let mut new_state = vec![Complex::zero(); size];
    new_state[outcome] = Complex::one();
    let new_reg = QuantumRegister::from_state(n, new_state);

    (bits, new_reg)
}

/// Measure a single qubit in a register.
pub fn measure_qubit_in_register(
    register: &QuantumRegister,
    qubit_index: usize,
    rng_val: f64,
) -> (u8, QuantumRegister) {
    let n = register.n_qubits;
    let size = register.size();
    let mask = 1 << (n - 1 - qubit_index);

    // Probability of measuring 0
    let p0: f64 = (0..size)
        .filter(|&i| (i & mask) == 0)
        .map(|i| register.state[i].norm_sq())
        .sum();

    let outcome = if rng_val < p0 { 0u8 } else { 1u8 };

    // Collapse
    let mut new_state = vec![Complex::zero(); size];
    let norm = if outcome == 0 { p0.sqrt() } else { (1.0 - p0).sqrt() };

    for i in 0..size {
        let bit = if (i & mask) == 0 { 0 } else { 1 };
        if bit == outcome {
            new_state[i] = register.state[i] / norm.max(1e-30);
        }
    }

    (outcome, QuantumRegister::from_state(n, new_state))
}

/// Quantum circuit: sequence of gate applications.
#[derive(Clone, Debug)]
pub struct QuantumCircuit {
    pub gates: Vec<(QuantumGate, Vec<usize>)>,
}

impl QuantumCircuit {
    pub fn new() -> Self {
        Self { gates: Vec::new() }
    }

    pub fn add_gate(&mut self, gate: QuantumGate, targets: Vec<usize>) {
        self.gates.push((gate, targets));
    }
}

/// Execute a circuit on a register.
pub fn execute(circuit: &QuantumCircuit, register: &QuantumRegister) -> QuantumRegister {
    let mut reg = register.clone();
    for (gate, targets) in &circuit.gates {
        reg = apply_gate(&reg, gate, targets);
    }
    reg
}

/// Deutsch-Jozsa algorithm: determine if an oracle function is constant or balanced.
/// oracle_fn: takes an n-bit input and returns 0 or 1.
/// Returns true if balanced, false if constant.
pub fn deutsch_jozsa(n_qubits: usize, oracle_fn: &dyn Fn(usize) -> u8) -> bool {
    let n = n_qubits;
    let total = n + 1; // n input qubits + 1 output qubit
    let size = 1 << total;

    // Initial state: |0...0>|1>
    let mut reg = QuantumRegister::new(total);
    // Set last qubit to |1>
    reg.state[0] = Complex::zero();
    reg.state[1] = Complex::one(); // |0...01>

    // Apply H to all qubits
    for i in 0..total {
        reg = apply_gate(&reg, &hadamard(), &[i]);
    }

    // Apply oracle: |x>|y> -> |x>|y XOR f(x)>
    let mut new_state = vec![Complex::zero(); size];
    for i in 0..size {
        let input = i >> 1; // first n qubits
        let output_bit = i & 1; // last qubit
        let f_x = oracle_fn(input % (1 << n)) as usize;
        let new_output = output_bit ^ f_x;
        let j = (input << 1) | new_output;
        new_state[j] += reg.state[i];
    }
    reg.state = new_state;

    // Apply H to input qubits
    for i in 0..n {
        reg = apply_gate(&reg, &hadamard(), &[i]);
    }

    // Measure input qubits: if all 0, function is constant
    let input_zero_prob: f64 = (0..size)
        .filter(|&i| (i >> 1) == 0)
        .map(|i| reg.state[i].norm_sq())
        .sum();

    // If probability of |0...0> is ~1, function is constant
    input_zero_prob < 0.5
}

/// Grover's search algorithm.
/// oracle: marks the target state (returns true for target).
/// Returns the found state index.
pub fn grover_search(oracle: &dyn Fn(usize) -> bool, n_qubits: usize, iterations: usize) -> usize {
    let size = 1 << n_qubits;
    let mut reg = QuantumRegister::new(n_qubits);

    // Apply H to all qubits to create uniform superposition
    for i in 0..n_qubits {
        reg = apply_gate(&reg, &hadamard(), &[i]);
    }

    for _ in 0..iterations {
        // Oracle: flip sign of target states
        for i in 0..size {
            if oracle(i) {
                reg.state[i] = -reg.state[i];
            }
        }

        // Diffusion operator: 2|s><s| - I
        // Apply H to all
        for q in 0..n_qubits {
            reg = apply_gate(&reg, &hadamard(), &[q]);
        }
        // Flip all except |0>
        for i in 1..size {
            reg.state[i] = -reg.state[i];
        }
        // Apply H to all
        for q in 0..n_qubits {
            reg = apply_gate(&reg, &hadamard(), &[q]);
        }
    }

    // Find most probable state
    let mut max_prob = 0.0;
    let mut max_idx = 0;
    for i in 0..size {
        let prob = reg.state[i].norm_sq();
        if prob > max_prob {
            max_prob = prob;
            max_idx = i;
        }
    }
    max_idx
}

/// Quantum Fourier Transform on a register.
pub fn qft(register: &QuantumRegister) -> QuantumRegister {
    let n = register.n_qubits;
    let mut reg = register.clone();

    for i in 0..n {
        // Apply Hadamard to qubit i
        reg = apply_gate(&reg, &hadamard(), &[i]);

        // Apply controlled phase rotations
        for j in (i + 1)..n {
            let k = j - i;
            let theta = PI / (1 << k) as f64;
            // Controlled phase: if qubit j is 1, apply phase to qubit i
            let cp = controlled_phase(theta);
            reg = apply_gate(&reg, &cp, &[j, i]);
        }
    }

    // Reverse qubit order
    for i in 0..n / 2 {
        reg = apply_gate(&reg, &swap(), &[i, n - 1 - i]);
    }

    reg
}

fn controlled_phase(theta: f64) -> QuantumGate {
    QuantumGate::new(
        vec![
            vec![Complex::one(), Complex::zero(), Complex::zero(), Complex::zero()],
            vec![Complex::zero(), Complex::one(), Complex::zero(), Complex::zero()],
            vec![Complex::zero(), Complex::zero(), Complex::one(), Complex::zero()],
            vec![Complex::zero(), Complex::zero(), Complex::zero(), Complex::from_polar(1.0, theta)],
        ],
        2,
        "CP",
    )
}

/// Render circuit diagram as ASCII.
pub struct CircuitRenderer;

impl CircuitRenderer {
    pub fn render(circuit: &QuantumCircuit, n_qubits: usize) -> Vec<String> {
        let mut lines: Vec<String> = (0..n_qubits)
            .map(|i| format!("q{}: ", i))
            .collect();

        for (gate, targets) in &circuit.gates {
            let max_target = targets.iter().copied().max().unwrap_or(0);
            let min_target = targets.iter().copied().min().unwrap_or(0);

            // Add gate symbol
            for q in 0..n_qubits {
                if targets.contains(&q) {
                    if targets.len() == 1 {
                        lines[q].push_str(&format!("[{}]", gate.name));
                    } else if q == targets[0] {
                        lines[q].push_str(&format!("[{}]", gate.name));
                    } else {
                        lines[q].push_str(" * ");
                    }
                } else if q > min_target && q < max_target {
                    lines[q].push_str(" | ");
                } else {
                    lines[q].push_str("---");
                }
            }

            // Separator
            for q in 0..n_qubits {
                lines[q].push('-');
            }
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hadamard_squared_is_identity() {
        let mut reg = QuantumRegister::new(1);
        reg = apply_gate(&reg, &hadamard(), &[0]);
        reg = apply_gate(&reg, &hadamard(), &[0]);
        // Should be back to |0>
        assert!((reg.state[0].re - 1.0).abs() < 1e-10);
        assert!(reg.state[1].norm() < 1e-10);
    }

    #[test]
    fn test_hadamard_creates_superposition() {
        let mut reg = QuantumRegister::new(1);
        reg = apply_gate(&reg, &hadamard(), &[0]);
        let s = 1.0 / 2.0_f64.sqrt();
        assert!((reg.state[0].re - s).abs() < 1e-10);
        assert!((reg.state[1].re - s).abs() < 1e-10);
    }

    #[test]
    fn test_pauli_x_flips() {
        let mut reg = QuantumRegister::new(1);
        reg = apply_gate(&reg, &pauli_x(), &[0]);
        assert!(reg.state[0].norm() < 1e-10);
        assert!((reg.state[1].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cnot_entangles() {
        // |00> -> H on qubit 0 -> (|0>+|1>)/sqrt(2) |0> -> CNOT -> (|00>+|11>)/sqrt(2)
        let mut reg = QuantumRegister::new(2);
        reg = apply_gate(&reg, &hadamard(), &[0]);
        reg = apply_gate(&reg, &cnot(), &[0, 1]);

        let s = 1.0 / 2.0_f64.sqrt();
        assert!((reg.state[0].re - s).abs() < 1e-10, "|00>: {}", reg.state[0].re); // |00>
        assert!(reg.state[1].norm() < 1e-10);  // |01>
        assert!(reg.state[2].norm() < 1e-10);  // |10>
        assert!((reg.state[3].re - s).abs() < 1e-10, "|11>: {}", reg.state[3].re); // |11>
    }

    #[test]
    fn test_measure_register() {
        let mut reg = QuantumRegister::new(2);
        // Set to |01>
        reg.state[0] = Complex::zero();
        reg.state[1] = Complex::one();

        let (bits, _) = measure_register(&reg, 0.5);
        assert_eq!(bits, vec![0, 1]);
    }

    #[test]
    fn test_measure_qubit() {
        let mut reg = QuantumRegister::new(2);
        reg = apply_gate(&reg, &hadamard(), &[0]);
        // State is (|00> + |10>)/sqrt(2)

        let (outcome, new_reg) = measure_qubit_in_register(&reg, 0, 0.3);
        let norm = new_reg.norm_sq();
        assert!((norm - 1.0).abs() < 1e-6, "Post-measurement norm: {}", norm);
    }

    #[test]
    fn test_swap_gate() {
        let mut reg = QuantumRegister::new(2);
        // Set to |01>
        reg.state[0] = Complex::zero();
        reg.state[1] = Complex::one();

        reg = apply_gate(&reg, &swap(), &[0, 1]);
        // Should be |10>
        assert!(reg.state[1].norm() < 1e-10);
        assert!((reg.state[2].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_circuit_execution() {
        let mut circuit = QuantumCircuit::new();
        circuit.add_gate(hadamard(), vec![0]);
        circuit.add_gate(cnot(), vec![0, 1]);

        let reg = QuantumRegister::new(2);
        let result = execute(&circuit, &reg);

        let s = 1.0 / 2.0_f64.sqrt();
        assert!((result.state[0].re - s).abs() < 1e-10);
        assert!((result.state[3].re - s).abs() < 1e-10);
    }

    #[test]
    fn test_deutsch_jozsa_constant() {
        // Constant function: f(x) = 0
        let is_balanced = deutsch_jozsa(2, &|_x| 0);
        assert!(!is_balanced, "Constant function should not be balanced");
    }

    #[test]
    fn test_deutsch_jozsa_balanced() {
        // Balanced function: f(x) = x & 1 (LSB)
        let is_balanced = deutsch_jozsa(2, &|x| (x & 1) as u8);
        assert!(is_balanced, "Balanced function should be detected");
    }

    #[test]
    fn test_grover_search() {
        // Search for state |3> = |11> in 2-qubit space
        let target = 3;
        let result = grover_search(&|x| x == target, 2, 1);
        assert_eq!(result, target, "Grover should find target {}, got {}", target, result);
    }

    #[test]
    fn test_grover_larger() {
        // 3-qubit search for |5> = |101>
        let target = 5;
        let n = 3;
        let iterations = ((PI / 4.0) * (8.0_f64).sqrt()).floor() as usize;
        let result = grover_search(&|x| x == target, n, iterations);
        assert_eq!(result, target, "Grover should find {}, got {}", target, result);
    }

    #[test]
    fn test_qft_basic() {
        // QFT of |0> should give uniform superposition
        let reg = QuantumRegister::new(2);
        let result = qft(&reg);
        let expected = 0.5; // 1/sqrt(4) squared
        for i in 0..4 {
            assert!(
                (result.state[i].norm_sq() - expected).abs() < 0.1,
                "QFT |0>[{}] prob: {}",
                i,
                result.state[i].norm_sq()
            );
        }
    }

    #[test]
    fn test_qft_preserves_norm() {
        let mut reg = QuantumRegister::new(3);
        reg = apply_gate(&reg, &hadamard(), &[0]);
        reg = apply_gate(&reg, &pauli_x(), &[1]);
        let norm_before = reg.norm_sq();
        let result = qft(&reg);
        let norm_after = result.norm_sq();
        assert!((norm_after - norm_before).abs() < 0.1, "QFT norm: {} -> {}", norm_before, norm_after);
    }

    #[test]
    fn test_toffoli() {
        // Toffoli only flips target when both controls are 1
        let mut reg = QuantumRegister::new(3);
        // Set to |110>
        reg.state[0] = Complex::zero();
        reg.state[6] = Complex::one(); // |110> = index 6
        reg = apply_gate(&reg, &toffoli(), &[0, 1, 2]);
        // Should be |111> = index 7
        assert!((reg.state[7].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_circuit_renderer() {
        let mut circuit = QuantumCircuit::new();
        circuit.add_gate(hadamard(), vec![0]);
        circuit.add_gate(cnot(), vec![0, 1]);
        let lines = CircuitRenderer::render(&circuit, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("H"));
    }

    #[test]
    fn test_phase_gate() {
        let mut reg = QuantumRegister::new(1);
        // Set to |1>
        reg.state[0] = Complex::zero();
        reg.state[1] = Complex::one();
        reg = apply_gate(&reg, &phase(PI), &[0]);
        // Should get -|1>
        assert!((reg.state[1].re - (-1.0)).abs() < 1e-10);
    }
}
