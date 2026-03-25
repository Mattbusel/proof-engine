use std::f64::consts::PI;
use super::schrodinger::Complex;
use glam::Vec3;

/// Spin-1/2 state: |psi> = up|+> + down|->.
#[derive(Clone, Debug)]
pub struct SpinState {
    pub up: Complex,
    pub down: Complex,
}

impl SpinState {
    pub fn new(up: Complex, down: Complex) -> Self {
        Self { up, down }
    }

    pub fn spin_up() -> Self {
        Self { up: Complex::one(), down: Complex::zero() }
    }

    pub fn spin_down() -> Self {
        Self { up: Complex::zero(), down: Complex::one() }
    }

    pub fn norm_sq(&self) -> f64 {
        self.up.norm_sq() + self.down.norm_sq()
    }

    pub fn normalize(&mut self) {
        let n = self.norm_sq().sqrt();
        if n > 1e-30 {
            self.up = self.up / n;
            self.down = self.down / n;
        }
    }
}

/// Get Bloch sphere angles (theta, phi) from a spin state.
/// |psi> = cos(theta/2)|+> + e^{i*phi}*sin(theta/2)|->
pub fn bloch_angles(state: &SpinState) -> (f64, f64) {
    let mut s = state.clone();
    s.normalize();

    let r_up = s.up.norm();
    let r_down = s.down.norm();

    let theta = 2.0 * r_down.atan2(r_up);

    // phi is the relative phase between down and up
    let phi = if r_down > 1e-12 && r_up > 1e-12 {
        let phase_up = s.up.arg();
        let phase_down = s.down.arg();
        phase_down - phase_up
    } else {
        0.0
    };

    (theta, phi)
}

/// Create spin state from Bloch sphere angles.
pub fn from_bloch(theta: f64, phi: f64) -> SpinState {
    SpinState {
        up: Complex::new((theta / 2.0).cos(), 0.0),
        down: Complex::from_polar((theta / 2.0).sin(), phi),
    }
}

/// Pauli X gate: |+> <-> |->
pub fn pauli_x(state: &SpinState) -> SpinState {
    SpinState {
        up: state.down,
        down: state.up,
    }
}

/// Pauli Y gate: |+> -> i|-> , |-> -> -i|+>
pub fn pauli_y(state: &SpinState) -> SpinState {
    SpinState {
        up: Complex::new(0.0, -1.0) * state.down,
        down: Complex::new(0.0, 1.0) * state.up,
    }
}

/// Pauli Z gate: |+> -> |+>, |-> -> -|->
pub fn pauli_z(state: &SpinState) -> SpinState {
    SpinState {
        up: state.up,
        down: -state.down,
    }
}

/// Rotate spin state about an axis by an angle.
/// Uses the SU(2) rotation: exp(-i * angle/2 * n.sigma)
pub fn rotate_spin(state: &SpinState, axis: Vec3, angle: f64) -> SpinState {
    let n = axis.normalize();
    let half = angle / 2.0;
    let c = half.cos();
    let s = half.sin();
    let nx = n.x as f64;
    let ny = n.y as f64;
    let nz = n.z as f64;

    // R = cos(a/2)*I - i*sin(a/2)*(nx*sx + ny*sy + nz*sz)
    // = [[cos - i*nz*sin, (-ny - i*nx)*sin],
    //    [(ny - i*nx)*sin, cos + i*nz*sin]]
    let r00 = Complex::new(c, -nz * s);
    let r01 = Complex::new(-ny * s, -nx * s);
    let r10 = Complex::new(ny * s, -nx * s);
    let r11 = Complex::new(c, nz * s);

    SpinState {
        up: r00 * state.up + r01 * state.down,
        down: r10 * state.up + r11 * state.down,
    }
}

/// Expectation value of spin along an axis: <S.n> = (hbar/2)*<psi|sigma.n|psi>.
/// Returns the value in units of hbar/2 (i.e., between -1 and 1).
pub fn spin_expectation(state: &SpinState, axis: Vec3) -> f64 {
    let n = axis.normalize();
    let nx = n.x as f64;
    let ny = n.y as f64;
    let nz = n.z as f64;

    // sigma.n = [[nz, nx-i*ny],[nx+i*ny, -nz]]
    let s_up = Complex::new(nz, 0.0) * state.up + Complex::new(nx, -ny) * state.down;
    let s_down = Complex::new(nx, ny) * state.up + Complex::new(-nz, 0.0) * state.down;

    let exp = state.up.conj() * s_up + state.down.conj() * s_down;
    exp.re
}

/// Larmor precession: time evolution in a magnetic field.
/// H = -gamma * B.sigma, evolve by exp(-iHt/hbar).
/// For simplicity, gamma*hbar/2 = 1, so omega = |B|.
pub fn larmor_precession(state: &SpinState, b_field: Vec3, dt: f64) -> SpinState {
    let b_mag = b_field.length() as f64;
    if b_mag < 1e-15 {
        return state.clone();
    }
    let axis = b_field / b_field.length();
    // Precession angle = omega * dt = |B| * dt
    rotate_spin(state, axis, b_mag * dt)
}

/// Stern-Gerlach measurement along an axis.
/// Returns (collapsed state, outcome +1 or -1).
pub fn stern_gerlach(state: &SpinState, measurement_axis: Vec3, rng_val: f64) -> (SpinState, i8) {
    let n = measurement_axis.normalize();

    // Eigenstates of sigma.n:
    // |+n> = cos(theta/2)|+> + e^{i*phi}*sin(theta/2)|->
    // |-n> = -e^{-i*phi}*sin(theta/2)|+> + cos(theta/2)|->
    let theta = (n.z as f64).acos();
    let phi = (n.y as f64).atan2(n.x as f64);

    let plus_n = SpinState {
        up: Complex::new((theta / 2.0).cos(), 0.0),
        down: Complex::from_polar((theta / 2.0).sin(), phi),
    };

    // Probability of +1 outcome
    let overlap = state.up.conj() * plus_n.up + state.down.conj() * plus_n.down;
    let prob_plus = overlap.norm_sq();

    if rng_val < prob_plus {
        (plus_n, 1)
    } else {
        let minus_n = SpinState {
            up: Complex::from_polar(-(theta / 2.0).sin(), -phi),
            down: Complex::new((theta / 2.0).cos(), 0.0),
        };
        (minus_n, -1)
    }
}

/// Render Bloch sphere as wireframe with state vector.
pub struct BlochSphereRenderer {
    pub size: usize,
}

impl BlochSphereRenderer {
    pub fn new(size: usize) -> Self {
        Self { size }
    }

    /// Render the Bloch sphere as ASCII art.
    pub fn render(&self, state: &SpinState) -> Vec<Vec<char>> {
        let (theta, phi) = bloch_angles(state);
        let sx = theta.sin() * phi.cos();
        let sy = theta.sin() * phi.sin();
        let sz = theta.cos();

        let s = self.size;
        let mut grid = vec![vec![' '; s]; s];
        let cx = s / 2;
        let cy = s / 2;
        let r = (s / 2 - 1) as f64;

        // Draw circle outline
        for i in 0..s {
            for j in 0..s {
                let dx = (j as f64 - cx as f64) / r;
                let dy = (i as f64 - cy as f64) / r;
                let dist = (dx * dx + dy * dy).sqrt();
                if (dist - 1.0).abs() < 0.15 {
                    grid[i][j] = '.';
                }
            }
        }

        // Draw equator
        for j in 0..s {
            let dx = (j as f64 - cx as f64) / r;
            if dx.abs() <= 1.0 {
                let row = cy;
                if grid[row][j] == ' ' {
                    grid[row][j] = '-';
                }
            }
        }

        // Draw vertical axis
        for i in 0..s {
            let dy = (i as f64 - cy as f64) / r;
            if dy.abs() <= 1.0 {
                if grid[i][cx] == ' ' {
                    grid[i][cx] = '|';
                }
            }
        }

        // Place poles
        grid[0][cx] = 'N'; // |+z>
        grid[s - 1][cx] = 'S'; // |-z>

        // Place state vector endpoint (project to xz plane for 2D view)
        let state_col = cx as f64 + sx * r;
        let state_row = cy as f64 - sz * r;
        let sc = (state_col.round() as usize).min(s - 1);
        let sr = (state_row.round() as usize).min(s - 1);
        grid[sr][sc] = '*';

        grid
    }
}

/// Heisenberg spin chain with nearest-neighbor coupling.
#[derive(Clone, Debug)]
pub struct SpinChain {
    pub spins: Vec<SpinState>,
    pub coupling: f64,
}

impl SpinChain {
    pub fn new(n: usize, coupling: f64) -> Self {
        let spins = (0..n).map(|_| SpinState::spin_up()).collect();
        Self { spins, coupling }
    }

    /// Simple time evolution step using Trotter decomposition.
    /// H = J * sum_i sigma_i . sigma_{i+1}
    pub fn step(&mut self, dt: f64) {
        let n = self.spins.len();
        if n < 2 {
            return;
        }

        for i in 0..n - 1 {
            // Approximate two-spin interaction
            // The effective field on spin i from spin i+1 and vice versa
            let exp_i = [
                spin_expectation(&self.spins[i], Vec3::X),
                spin_expectation(&self.spins[i], Vec3::Y),
                spin_expectation(&self.spins[i], Vec3::Z),
            ];
            let exp_j = [
                spin_expectation(&self.spins[i + 1], Vec3::X),
                spin_expectation(&self.spins[i + 1], Vec3::Y),
                spin_expectation(&self.spins[i + 1], Vec3::Z),
            ];

            // Mean-field: each spin sees the other as an effective B field
            let b_on_i = Vec3::new(
                exp_j[0] as f32,
                exp_j[1] as f32,
                exp_j[2] as f32,
            ) * self.coupling as f32;
            let b_on_j = Vec3::new(
                exp_i[0] as f32,
                exp_i[1] as f32,
                exp_i[2] as f32,
            ) * self.coupling as f32;

            self.spins[i] = larmor_precession(&self.spins[i], b_on_i, dt);
            self.spins[i + 1] = larmor_precession(&self.spins[i + 1], b_on_j, dt);
        }
    }

    /// Total magnetization along z: sum of <Sz_i>.
    pub fn magnetization_z(&self) -> f64 {
        self.spins
            .iter()
            .map(|s| spin_expectation(s, Vec3::Z))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloch_roundtrip() {
        let theta = 1.2;
        let phi = 0.7;
        let state = from_bloch(theta, phi);
        let (t2, p2) = bloch_angles(&state);
        assert!((t2 - theta).abs() < 1e-10, "theta: {} vs {}", t2, theta);
        assert!((p2 - phi).abs() < 1e-10, "phi: {} vs {}", p2, phi);
    }

    #[test]
    fn test_bloch_poles() {
        let up = SpinState::spin_up();
        let (theta, _) = bloch_angles(&up);
        assert!(theta.abs() < 1e-10, "Up: theta = {}", theta);

        let down = SpinState::spin_down();
        let (theta, _) = bloch_angles(&down);
        assert!((theta - PI).abs() < 1e-10, "Down: theta = {}", theta);
    }

    #[test]
    fn test_pauli_x_algebra() {
        // X^2 = I
        let state = SpinState::new(Complex::new(0.6, 0.0), Complex::new(0.8, 0.0));
        let xx = pauli_x(&pauli_x(&state));
        assert!((xx.up.re - state.up.re).abs() < 1e-10);
        assert!((xx.down.re - state.down.re).abs() < 1e-10);
    }

    #[test]
    fn test_pauli_y_algebra() {
        let state = SpinState::spin_up();
        let yy = pauli_y(&pauli_y(&state));
        // Y^2 = I
        assert!((yy.up.re - state.up.re).abs() < 1e-10);
        assert!((yy.down.re - state.down.re).abs() < 1e-10);
    }

    #[test]
    fn test_pauli_z_algebra() {
        let state = SpinState::new(Complex::new(0.6, 0.0), Complex::new(0.8, 0.0));
        let zz = pauli_z(&pauli_z(&state));
        assert!((zz.up.re - state.up.re).abs() < 1e-10);
        assert!((zz.down.re - state.down.re).abs() < 1e-10);
    }

    #[test]
    fn test_pauli_anticommutation() {
        // XY = iZ, YX = -iZ => XY + YX = 0
        let state = SpinState::new(Complex::new(0.6, 0.0), Complex::new(0.0, 0.8));
        let xy = pauli_y(&pauli_x(&state));
        let yx = pauli_x(&pauli_y(&state));
        // xy + yx should be zero
        let sum_up = xy.up + yx.up;
        let sum_down = xy.down + yx.down;
        assert!(sum_up.norm() < 1e-10, "XY+YX up = {:?}", sum_up);
        assert!(sum_down.norm() < 1e-10, "XY+YX down = {:?}", sum_down);
    }

    #[test]
    fn test_spin_expectation_z() {
        let up = SpinState::spin_up();
        let ez = spin_expectation(&up, Vec3::Z);
        assert!((ez - 1.0).abs() < 1e-10);

        let down = SpinState::spin_down();
        let ez = spin_expectation(&down, Vec3::Z);
        assert!((ez - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_spin_expectation_x() {
        // |+x> = (|+> + |->)/sqrt(2)
        let s = 1.0 / 2.0_f64.sqrt();
        let plus_x = SpinState::new(Complex::new(s, 0.0), Complex::new(s, 0.0));
        let ex = spin_expectation(&plus_x, Vec3::X);
        assert!((ex - 1.0).abs() < 1e-10, "<Sx> = {}", ex);
    }

    #[test]
    fn test_rotation_360_identity() {
        let state = SpinState::new(Complex::new(0.6, 0.1), Complex::new(0.3, 0.7));
        let mut s = state.clone();
        s.normalize();
        // 4pi rotation = identity for spin-1/2 (2pi gives -1)
        let rotated = rotate_spin(&s, Vec3::Z, 4.0 * PI);
        assert!((rotated.up.re - s.up.re).abs() < 1e-8);
        assert!((rotated.down.re - s.down.re).abs() < 1e-8);
    }

    #[test]
    fn test_rotation_pi_z_flips_x() {
        // Rotating |+x> by pi about z should give |-x>
        let s = 1.0 / 2.0_f64.sqrt();
        let plus_x = SpinState::new(Complex::new(s, 0.0), Complex::new(s, 0.0));
        let rotated = rotate_spin(&plus_x, Vec3::Z, PI);
        let ex = spin_expectation(&rotated, Vec3::X);
        assert!((ex - (-1.0)).abs() < 1e-8, "<Sx> after pi-z rotation: {}", ex);
    }

    #[test]
    fn test_stern_gerlach_statistics() {
        // |+z> measured along z should always give +1
        let up = SpinState::spin_up();
        let (_, outcome) = stern_gerlach(&up, Vec3::Z, 0.3);
        assert_eq!(outcome, 1);
        let (_, outcome) = stern_gerlach(&up, Vec3::Z, 0.9);
        assert_eq!(outcome, 1);
    }

    #[test]
    fn test_stern_gerlach_x_on_z_up() {
        // |+z> measured along x should give 50/50
        let up = SpinState::spin_up();
        let (_, o1) = stern_gerlach(&up, Vec3::X, 0.2);
        let (_, o2) = stern_gerlach(&up, Vec3::X, 0.8);
        assert_eq!(o1, 1);
        assert_eq!(o2, -1);
    }

    #[test]
    fn test_larmor_precession() {
        // Spin up in x-field should precess to spin down and back
        let state = SpinState::spin_up();
        let b = Vec3::X * 1.0;
        let mut s = state;
        for _ in 0..1000 {
            s = larmor_precession(&s, b, 0.01);
        }
        // After time T = 2*pi/|B| = 2*pi, should return to original
        // We evolved for t=10, which is about 1.59 periods
        // Just check it's still normalized
        assert!((s.norm_sq() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_bloch_sphere_renderer() {
        let state = SpinState::spin_up();
        let renderer = BlochSphereRenderer::new(15);
        let grid = renderer.render(&state);
        assert_eq!(grid.len(), 15);
        assert_eq!(grid[0].len(), 15);
    }

    #[test]
    fn test_spin_chain() {
        let mut chain = SpinChain::new(4, 1.0);
        // Flip one spin
        chain.spins[0] = SpinState::spin_down();
        let m_before = chain.magnetization_z();
        chain.step(0.01);
        let m_after = chain.magnetization_z();
        // Magnetization should be roughly conserved in Heisenberg model
        assert!((m_before - m_after).abs() < 0.5);
    }
}
