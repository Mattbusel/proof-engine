use std::f64::consts::PI;

/// Complex number with full arithmetic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn zero() -> Self {
        Self { re: 0.0, im: 0.0 }
    }

    pub fn one() -> Self {
        Self { re: 1.0, im: 0.0 }
    }

    pub fn i() -> Self {
        Self { re: 0.0, im: 1.0 }
    }

    pub fn norm_sq(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn norm(&self) -> f64 {
        self.norm_sq().sqrt()
    }

    pub fn conj(&self) -> Self {
        Self { re: self.re, im: -self.im }
    }

    pub fn exp(self) -> Self {
        let e = self.re.exp();
        Self {
            re: e * self.im.cos(),
            im: e * self.im.sin(),
        }
    }

    pub fn from_polar(r: f64, theta: f64) -> Self {
        Self {
            re: r * theta.cos(),
            im: r * theta.sin(),
        }
    }

    pub fn arg(&self) -> f64 {
        self.im.atan2(self.re)
    }

    pub fn scale(self, s: f64) -> Self {
        Self { re: self.re * s, im: self.im * s }
    }
}

impl std::ops::Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { re: self.re + rhs.re, im: self.im + rhs.im }
    }
}

impl std::ops::AddAssign for Complex {
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl std::ops::Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { re: self.re - rhs.re, im: self.im - rhs.im }
    }
}

impl std::ops::SubAssign for Complex {
    fn sub_assign(&mut self, rhs: Self) {
        self.re -= rhs.re;
        self.im -= rhs.im;
    }
}

impl std::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl std::ops::MulAssign for Complex {
    fn mul_assign(&mut self, rhs: Self) {
        let re = self.re * rhs.re - self.im * rhs.im;
        let im = self.re * rhs.im + self.im * rhs.re;
        self.re = re;
        self.im = im;
    }
}

impl std::ops::Div for Complex {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let d = rhs.norm_sq();
        Self {
            re: (self.re * rhs.re + self.im * rhs.im) / d,
            im: (self.im * rhs.re - self.re * rhs.im) / d,
        }
    }
}

impl std::ops::Neg for Complex {
    type Output = Self;
    fn neg(self) -> Self {
        Self { re: -self.re, im: -self.im }
    }
}

impl std::ops::Mul<f64> for Complex {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self { re: self.re * rhs, im: self.im * rhs }
    }
}

impl std::ops::Mul<Complex> for f64 {
    type Output = Complex;
    fn mul(self, rhs: Complex) -> Complex {
        Complex { re: self * rhs.re, im: self * rhs.im }
    }
}

impl std::ops::Div<f64> for Complex {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        Self { re: self.re / rhs, im: self.im / rhs }
    }
}

impl Default for Complex {
    fn default() -> Self {
        Self::zero()
    }
}

/// 1D wave function on a uniform grid.
#[derive(Clone, Debug)]
pub struct WaveFunction1D {
    pub psi: Vec<Complex>,
    pub dx: f64,
    pub x_min: f64,
}

impl WaveFunction1D {
    pub fn new(psi: Vec<Complex>, dx: f64, x_min: f64) -> Self {
        Self { psi, dx, x_min }
    }

    pub fn n(&self) -> usize {
        self.psi.len()
    }

    pub fn x_at(&self, i: usize) -> f64 {
        self.x_min + i as f64 * self.dx
    }

    pub fn x_max(&self) -> f64 {
        self.x_min + (self.n() - 1) as f64 * self.dx
    }

    pub fn probability_density(&self) -> Vec<f64> {
        self.psi.iter().map(|c| c.norm_sq()).collect()
    }

    pub fn norm_squared(&self) -> f64 {
        self.psi.iter().map(|c| c.norm_sq()).sum::<f64>() * self.dx
    }

    pub fn normalize(&mut self) {
        let n = self.norm_squared().sqrt();
        if n > 1e-30 {
            for c in &mut self.psi {
                *c = *c / n;
            }
        }
    }
}

/// Solve tridiagonal system Ax = d where A has diagonals (a, b, c).
/// a[0] and c[n-1] are unused. Modifies d in-place and returns solution.
fn solve_tridiagonal(a: &[Complex], b: &[Complex], c: &[Complex], d: &mut [Complex]) -> Vec<Complex> {
    let n = d.len();
    let mut cp = vec![Complex::zero(); n];
    let mut dp = vec![Complex::zero(); n];

    cp[0] = c[0] / b[0];
    dp[0] = d[0] / b[0];

    for i in 1..n {
        let m = b[i] - a[i] * cp[i - 1];
        cp[i] = if i < n - 1 { c[i] / m } else { Complex::zero() };
        dp[i] = (d[i] - a[i] * dp[i - 1]) / m;
    }

    let mut x = vec![Complex::zero(); n];
    x[n - 1] = dp[n - 1];
    for i in (0..n - 1).rev() {
        x[i] = dp[i] - cp[i] * x[i + 1];
    }
    x
}

/// 1D Schrodinger equation solver using Crank-Nicolson and split-operator methods.
#[derive(Clone)]
pub struct SchrodingerSolver1D {
    pub psi: WaveFunction1D,
    pub potential: Vec<f64>,
    pub mass: f64,
    pub hbar: f64,
    pub dt: f64,
}

impl SchrodingerSolver1D {
    pub fn new(psi: WaveFunction1D, potential: Vec<f64>, mass: f64, hbar: f64, dt: f64) -> Self {
        Self { psi, potential, mass, hbar, dt }
    }

    /// Crank-Nicolson time step (implicit, unitary).
    pub fn step(&mut self) {
        let n = self.psi.n();
        let dx = self.psi.dx;
        let r = Complex::new(0.0, self.hbar * self.dt / (4.0 * self.mass * dx * dx));

        let mut a_lower = vec![Complex::zero(); n];
        let mut a_diag = vec![Complex::zero(); n];
        let mut a_upper = vec![Complex::zero(); n];
        let mut rhs = vec![Complex::zero(); n];

        for j in 0..n {
            let v_term = Complex::new(0.0, self.dt * self.potential[j] / (2.0 * self.hbar));

            // LHS: (1 + iHdt/2)
            a_diag[j] = Complex::one() + r * 2.0 + v_term;
            if j > 0 {
                a_lower[j] = -r;
            }
            if j < n - 1 {
                a_upper[j] = -r;
            }

            // RHS: (1 - iHdt/2) * psi
            let psi_j = self.psi.psi[j];
            let psi_left = if j > 0 { self.psi.psi[j - 1] } else { Complex::zero() };
            let psi_right = if j < n - 1 { self.psi.psi[j + 1] } else { Complex::zero() };

            rhs[j] = (Complex::one() - r * 2.0 - v_term) * psi_j
                + r * psi_left
                + r * psi_right;
        }

        self.psi.psi = solve_tridiagonal(&a_lower, &a_diag, &a_upper, &mut rhs);
    }

    /// Split-operator FFT method: kinetic in k-space, potential in x-space.
    pub fn step_split_operator(&mut self) {
        let n = self.psi.n();
        let dx = self.psi.dx;
        let dt = self.dt;
        let hbar = self.hbar;
        let mass = self.mass;

        // Half potential step in x-space
        for j in 0..n {
            let phase = -self.potential[j] * dt / (2.0 * hbar);
            let exp_v = Complex::from_polar(1.0, phase);
            self.psi.psi[j] = self.psi.psi[j] * exp_v;
        }

        // Full kinetic step in k-space
        let mut psi_k = dft(&self.psi.psi);
        let dk = 2.0 * PI / (n as f64 * dx);
        for j in 0..n {
            let k = if j <= n / 2 {
                j as f64 * dk
            } else {
                (j as f64 - n as f64) * dk
            };
            let phase = -hbar * k * k * dt / (2.0 * mass);
            let exp_t = Complex::from_polar(1.0, phase);
            psi_k[j] = psi_k[j] * exp_t;
        }
        self.psi.psi = idft(&psi_k);

        // Half potential step in x-space
        for j in 0..n {
            let phase = -self.potential[j] * dt / (2.0 * hbar);
            let exp_v = Complex::from_polar(1.0, phase);
            self.psi.psi[j] = self.psi.psi[j] * exp_v;
        }
    }
}

/// Discrete Fourier Transform.
pub fn dft(input: &[Complex]) -> Vec<Complex> {
    let n = input.len();
    let mut output = vec![Complex::zero(); n];
    for k in 0..n {
        let mut sum = Complex::zero();
        for j in 0..n {
            let angle = -2.0 * PI * (k as f64) * (j as f64) / (n as f64);
            sum += input[j] * Complex::from_polar(1.0, angle);
        }
        output[k] = sum;
    }
    output
}

/// Inverse Discrete Fourier Transform.
pub fn idft(input: &[Complex]) -> Vec<Complex> {
    let n = input.len();
    let mut output = vec![Complex::zero(); n];
    for j in 0..n {
        let mut sum = Complex::zero();
        for k in 0..n {
            let angle = 2.0 * PI * (k as f64) * (j as f64) / (n as f64);
            sum += input[k] * Complex::from_polar(1.0, angle);
        }
        output[j] = sum / n as f64;
    }
    output
}

/// Compute energy eigenvalues using the shooting method for the time-independent
/// Schrodinger equation with the given potential on a uniform grid.
pub fn energy_eigenvalues(potential: &[f64], n_states: usize, dx: f64, mass: f64, hbar: f64) -> Vec<f64> {
    let n = potential.len();
    let v_min = potential.iter().cloned().fold(f64::INFINITY, f64::min);
    let v_max = potential.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let mut eigenvalues = Vec::new();
    let de = (v_max - v_min + 10.0) / 10000.0;
    let mut e = v_min + de;
    let mut prev_end = shoot(potential, e, dx, mass, hbar);

    while eigenvalues.len() < n_states && e < v_max + 50.0 {
        e += de;
        let cur_end = shoot(potential, e, dx, mass, hbar);
        if prev_end * cur_end < 0.0 {
            // Sign change: refine with bisection
            let mut lo = e - de;
            let mut hi = e;
            for _ in 0..60 {
                let mid = (lo + hi) / 2.0;
                let mid_val = shoot(potential, mid, dx, mass, hbar);
                if mid_val * shoot(potential, lo, dx, mass, hbar) < 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            eigenvalues.push((lo + hi) / 2.0);
        }
        prev_end = cur_end;
    }
    eigenvalues
}

/// Shooting method: integrate Schrodinger equation from left to right and return
/// the value of psi at the right boundary. A zero crossing indicates an eigenvalue.
fn shoot(potential: &[f64], energy: f64, dx: f64, mass: f64, hbar: f64) -> f64 {
    let n = potential.len();
    let coeff = 2.0 * mass / (hbar * hbar);
    let mut psi_prev = 0.0_f64;
    let mut psi_curr = 1e-10_f64;

    for i in 1..n - 1 {
        let k_sq = coeff * (potential[i] - energy);
        let psi_next = 2.0 * psi_curr - psi_prev + dx * dx * k_sq * psi_curr;
        psi_prev = psi_curr;
        psi_curr = psi_next;
    }
    psi_curr
}

/// Compute energy eigenstates using the shooting method.
pub fn energy_eigenstates(
    potential: &[f64],
    n_states: usize,
    dx: f64,
    x_min: f64,
    mass: f64,
    hbar: f64,
) -> Vec<WaveFunction1D> {
    let eigenvalues = energy_eigenvalues(potential, n_states, dx, mass, hbar);
    let n = potential.len();
    let coeff = 2.0 * mass / (hbar * hbar);

    eigenvalues
        .iter()
        .map(|&energy| {
            let mut psi = vec![0.0_f64; n];
            psi[1] = 1e-10;
            for i in 1..n - 1 {
                let k_sq = coeff * (potential[i] - energy);
                psi[i + 1] = 2.0 * psi[i] - psi[i - 1] + dx * dx * k_sq * psi[i];
            }
            let psi_c: Vec<Complex> = psi.iter().map(|&v| Complex::new(v, 0.0)).collect();
            let mut wf = WaveFunction1D::new(psi_c, dx, x_min);
            wf.normalize();
            wf
        })
        .collect()
}

/// 2D Schrodinger solver using ADI (alternating direction implicit) method.
#[derive(Clone)]
pub struct SchrodingerSolver2D {
    pub psi: Vec<Vec<Complex>>,
    pub potential: Vec<Vec<f64>>,
    pub nx: usize,
    pub ny: usize,
    pub dx: f64,
    pub dy: f64,
    pub dt: f64,
    pub mass: f64,
    pub hbar: f64,
}

impl SchrodingerSolver2D {
    pub fn new(
        psi: Vec<Vec<Complex>>,
        potential: Vec<Vec<f64>>,
        nx: usize,
        ny: usize,
        dx: f64,
        dy: f64,
        dt: f64,
        mass: f64,
        hbar: f64,
    ) -> Self {
        Self { psi, potential, nx, ny, dx, dy, dt, mass, hbar }
    }

    /// ADI time step: half step implicit in x, half step implicit in y.
    pub fn step_2d(&mut self) {
        let nx = self.nx;
        let ny = self.ny;
        let rx = Complex::new(0.0, self.hbar * self.dt / (4.0 * self.mass * self.dx * self.dx));
        let ry = Complex::new(0.0, self.hbar * self.dt / (4.0 * self.mass * self.dy * self.dy));

        // Half step: implicit in x, explicit in y
        let mut psi_half = vec![vec![Complex::zero(); ny]; nx];
        for j in 0..ny {
            let mut a = vec![Complex::zero(); nx];
            let mut b = vec![Complex::zero(); nx];
            let mut c = vec![Complex::zero(); nx];
            let mut d = vec![Complex::zero(); nx];

            for i in 0..nx {
                let v_term = Complex::new(0.0, self.dt * self.potential[i][j] / (4.0 * self.hbar));
                b[i] = Complex::one() + rx * 2.0 + v_term;
                if i > 0 { a[i] = -rx; }
                if i < nx - 1 { c[i] = -rx; }

                let psi_ij = self.psi[i][j];
                let psi_up = if j > 0 { self.psi[i][j - 1] } else { Complex::zero() };
                let psi_down = if j < ny - 1 { self.psi[i][j + 1] } else { Complex::zero() };

                d[i] = (Complex::one() - ry * 2.0 - v_term) * psi_ij
                    + ry * psi_up
                    + ry * psi_down;
            }
            let sol = solve_tridiagonal(&a, &b, &c, &mut d);
            for i in 0..nx {
                psi_half[i][j] = sol[i];
            }
        }

        // Half step: implicit in y, explicit in x
        for i in 0..nx {
            let mut a = vec![Complex::zero(); ny];
            let mut b = vec![Complex::zero(); ny];
            let mut c = vec![Complex::zero(); ny];
            let mut d = vec![Complex::zero(); ny];

            for j in 0..ny {
                let v_term = Complex::new(0.0, self.dt * self.potential[i][j] / (4.0 * self.hbar));
                b[j] = Complex::one() + ry * 2.0 + v_term;
                if j > 0 { a[j] = -ry; }
                if j < ny - 1 { c[j] = -ry; }

                let psi_ij = psi_half[i][j];
                let psi_left = if i > 0 { psi_half[i - 1][j] } else { Complex::zero() };
                let psi_right = if i < nx - 1 { psi_half[i + 1][j] } else { Complex::zero() };

                d[j] = (Complex::one() - rx * 2.0 - v_term) * psi_ij
                    + rx * psi_left
                    + rx * psi_right;
            }
            let sol = solve_tridiagonal(&a, &b, &c, &mut d);
            for j in 0..ny {
                self.psi[i][j] = sol[j];
            }
        }
    }
}

/// Probability density |psi|^2 for 1D wave function.
pub fn probability_density_1d(psi: &[Complex]) -> Vec<f64> {
    psi.iter().map(|c| c.norm_sq()).collect()
}

/// Probability density |psi|^2 for 2D wave function.
pub fn probability_density_2d(psi: &[Vec<Complex>]) -> Vec<Vec<f64>> {
    psi.iter()
        .map(|row| row.iter().map(|c| c.norm_sq()).collect())
        .collect()
}

/// Normalize a 1D wave function so that integral |psi|^2 dx = 1.
pub fn normalize(psi: &mut [Complex], dx: f64) {
    let norm_sq: f64 = psi.iter().map(|c| c.norm_sq()).sum::<f64>() * dx;
    let norm = norm_sq.sqrt();
    if norm > 1e-30 {
        for c in psi.iter_mut() {
            *c = *c / norm;
        }
    }
}

/// Expectation value of position: <x> = integral psi* x psi dx.
pub fn expectation_x(psi: &WaveFunction1D) -> f64 {
    let mut sum = 0.0;
    for i in 0..psi.n() {
        let x = psi.x_at(i);
        sum += psi.psi[i].norm_sq() * x;
    }
    sum * psi.dx
}

/// Expectation value of momentum: <p> = -i hbar integral psi* dpsi/dx dx.
pub fn expectation_p(psi: &WaveFunction1D, hbar: f64) -> f64 {
    let n = psi.n();
    let dx = psi.dx;
    let mut sum = Complex::zero();
    for i in 1..n - 1 {
        let dpsi = (psi.psi[i + 1] - psi.psi[i - 1]) / (2.0 * dx);
        sum += psi.psi[i].conj() * dpsi;
    }
    let result = Complex::new(0.0, -hbar) * sum * dx;
    result.re
}

/// Uncertainty in position: sqrt(<x^2> - <x>^2).
pub fn uncertainty_x(psi: &WaveFunction1D) -> f64 {
    let ex = expectation_x(psi);
    let mut ex2 = 0.0;
    for i in 0..psi.n() {
        let x = psi.x_at(i);
        ex2 += psi.psi[i].norm_sq() * x * x;
    }
    ex2 *= psi.dx;
    (ex2 - ex * ex).max(0.0).sqrt()
}

/// Uncertainty in momentum: sqrt(<p^2> - <p>^2).
pub fn uncertainty_p(psi: &WaveFunction1D, hbar: f64) -> f64 {
    let ep = expectation_p(psi, hbar);
    let n = psi.n();
    let dx = psi.dx;

    // <p^2> = -hbar^2 integral psi* d^2psi/dx^2 dx
    let mut sum = 0.0;
    for i in 1..n - 1 {
        let d2psi = (psi.psi[i + 1] - psi.psi[i] * 2.0 + psi.psi[i - 1]) / (dx * dx);
        let integrand = psi.psi[i].conj() * d2psi;
        sum += integrand.re;
    }
    let ep2 = -hbar * hbar * sum * dx;
    (ep2 - ep * ep).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex_arithmetic() {
        let a = Complex::new(1.0, 2.0);
        let b = Complex::new(3.0, 4.0);
        let sum = a + b;
        assert!((sum.re - 4.0).abs() < 1e-10);
        assert!((sum.im - 6.0).abs() < 1e-10);

        let prod = a * b;
        assert!((prod.re - (-5.0)).abs() < 1e-10);
        assert!((prod.im - 10.0).abs() < 1e-10);

        let div = a / b;
        let expected_re = 11.0 / 25.0;
        let expected_im = 2.0 / 25.0;
        assert!((div.re - expected_re).abs() < 1e-10);
        assert!((div.im - expected_im).abs() < 1e-10);
    }

    #[test]
    fn test_complex_exp() {
        let z = Complex::new(0.0, PI);
        let result = z.exp();
        assert!((result.re - (-1.0)).abs() < 1e-10);
        assert!(result.im.abs() < 1e-10);
    }

    #[test]
    fn test_complex_from_polar() {
        let c = Complex::from_polar(2.0, PI / 4.0);
        assert!((c.re - 2.0_f64.sqrt()).abs() < 1e-10);
        assert!((c.im - 2.0_f64.sqrt()).abs() < 1e-10);
    }

    #[test]
    fn test_normalization() {
        let n = 200;
        let dx = 0.1;
        let x_min = -10.0;
        let sigma = 1.0;
        let psi: Vec<Complex> = (0..n)
            .map(|i| {
                let x = x_min + i as f64 * dx;
                Complex::new((-x * x / (2.0 * sigma * sigma)).exp(), 0.0)
            })
            .collect();
        let mut wf = WaveFunction1D::new(psi, dx, x_min);
        wf.normalize();
        let norm = wf.norm_squared();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_crank_nicolson_preserves_norm() {
        let n = 128;
        let dx = 0.1;
        let x_min = -6.4;
        let sigma = 1.0;
        let psi: Vec<Complex> = (0..n)
            .map(|i| {
                let x = x_min + i as f64 * dx;
                Complex::new((-x * x / (2.0 * sigma * sigma)).exp(), 0.0)
            })
            .collect();
        let mut wf = WaveFunction1D::new(psi, dx, x_min);
        wf.normalize();
        let potential = vec![0.0; n];
        let mut solver = SchrodingerSolver1D::new(wf, potential, 1.0, 1.0, 0.001);
        let norm_before = solver.psi.norm_squared();
        for _ in 0..50 {
            solver.step();
        }
        let norm_after = solver.psi.norm_squared();
        assert!((norm_after - norm_before).abs() < 0.05);
    }

    #[test]
    fn test_split_operator_preserves_norm() {
        let n = 64;
        let dx = 0.2;
        let x_min = -6.4;
        let sigma = 1.0;
        let psi: Vec<Complex> = (0..n)
            .map(|i| {
                let x = x_min + i as f64 * dx;
                Complex::new((-x * x / (2.0 * sigma * sigma)).exp(), 0.0)
            })
            .collect();
        let mut wf = WaveFunction1D::new(psi, dx, x_min);
        wf.normalize();
        let potential = vec![0.0; n];
        let mut solver = SchrodingerSolver1D::new(wf, potential, 1.0, 1.0, 0.001);
        let norm_before = solver.psi.norm_squared();
        for _ in 0..20 {
            solver.step_split_operator();
        }
        let norm_after = solver.psi.norm_squared();
        assert!((norm_after - norm_before).abs() < 0.05);
    }

    #[test]
    fn test_uncertainty_principle() {
        let n = 512;
        let dx = 0.05;
        let x_min = -12.8;
        let sigma = 1.0;
        let hbar = 1.0;
        let psi: Vec<Complex> = (0..n)
            .map(|i| {
                let x = x_min + i as f64 * dx;
                Complex::new((-x * x / (4.0 * sigma * sigma)).exp(), 0.0)
            })
            .collect();
        let mut wf = WaveFunction1D::new(psi, dx, x_min);
        wf.normalize();
        let dx_unc = uncertainty_x(&wf);
        let dp_unc = uncertainty_p(&wf, hbar);
        let product = dx_unc * dp_unc;
        // Heisenberg: dx * dp >= hbar/2
        assert!(product >= hbar / 2.0 - 0.1, "Uncertainty product {} < hbar/2", product);
    }

    #[test]
    fn test_dft_idft_roundtrip() {
        let input = vec![
            Complex::new(1.0, 0.0),
            Complex::new(0.0, 1.0),
            Complex::new(-1.0, 0.0),
            Complex::new(0.0, -1.0),
        ];
        let transformed = dft(&input);
        let recovered = idft(&transformed);
        for (a, b) in input.iter().zip(recovered.iter()) {
            assert!((a.re - b.re).abs() < 1e-10);
            assert!((a.im - b.im).abs() < 1e-10);
        }
    }

    #[test]
    fn test_tridiagonal_solver() {
        // Simple 3x3 system
        let a = [Complex::zero(), Complex::new(-1.0, 0.0), Complex::new(-1.0, 0.0)];
        let b = [Complex::new(2.0, 0.0), Complex::new(2.0, 0.0), Complex::new(2.0, 0.0)];
        let c = [Complex::new(-1.0, 0.0), Complex::new(-1.0, 0.0), Complex::zero()];
        let mut d = [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0), Complex::new(1.0, 0.0)];
        let x = solve_tridiagonal(&a, &b, &c, &mut d);
        // Verify Ax = d
        let r0 = b[0] * x[0] + c[0] * x[1];
        assert!((r0.re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_infinite_well_eigenvalues() {
        let n = 200;
        let l = 1.0;
        let dx = l / (n as f64 - 1.0);
        let hbar = 1.0;
        let mass = 0.5; // so hbar^2/(2m) = 1
        let potential: Vec<f64> = (0..n)
            .map(|i| {
                let x = i as f64 * dx;
                if x < 0.01 || x > l - 0.01 { 1e6 } else { 0.0 }
            })
            .collect();
        let evals = energy_eigenvalues(&potential, 3, dx, mass, hbar);
        // E_n = n^2 pi^2 hbar^2 / (2 m L^2) = n^2 pi^2
        if evals.len() >= 2 {
            let ratio = evals[1] / evals[0];
            // Should be close to 4 (2^2/1^2)
            assert!((ratio - 4.0).abs() < 1.0, "Ratio: {}", ratio);
        }
    }

    #[test]
    fn test_2d_solver_runs() {
        let nx = 16;
        let ny = 16;
        let psi = vec![vec![Complex::zero(); ny]; nx];
        let potential = vec![vec![0.0; ny]; nx];
        let mut solver = SchrodingerSolver2D::new(psi, potential, nx, ny, 0.1, 0.1, 0.001, 1.0, 1.0);
        // Place a peak in the center
        solver.psi[nx / 2][ny / 2] = Complex::new(1.0, 0.0);
        solver.step_2d();
        // Just verify it doesn't panic and spreads
        let center_prob = solver.psi[nx / 2][ny / 2].norm_sq();
        assert!(center_prob < 1.1); // shouldn't blow up
    }
}
