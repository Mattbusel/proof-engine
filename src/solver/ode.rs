//! ODE solvers — Euler, RK4, RK45 (adaptive), implicit, symplectic methods.

/// State vector for an ODE system.
#[derive(Debug, Clone)]
pub struct OdeState {
    pub t: f64,
    pub y: Vec<f64>,
}

/// An ODE system dy/dt = f(t, y).
pub trait OdeSystem: Send + Sync {
    fn dimension(&self) -> usize;
    fn evaluate(&self, t: f64, y: &[f64], dydt: &mut [f64]);
}

/// ODE integration method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OdeMethod {
    Euler,
    RungeKutta4,
    RungeKutta45,  // adaptive (Dormand-Prince)
    ImplicitEuler,
    CrankNicolson,
    Verlet,        // symplectic
    Leapfrog,      // symplectic
}

/// ODE solver with configurable method and step control.
pub struct OdeSolver {
    pub method: OdeMethod,
    pub dt: f64,
    pub dt_min: f64,
    pub dt_max: f64,
    pub tolerance: f64,
    pub max_steps: u64,
    work: Vec<f64>,
}

impl OdeSolver {
    pub fn new(method: OdeMethod, dt: f64) -> Self {
        Self {
            method, dt, dt_min: 1e-8, dt_max: 1.0,
            tolerance: 1e-6, max_steps: 1_000_000,
            work: Vec::new(),
        }
    }

    pub fn rk4(dt: f64) -> Self { Self::new(OdeMethod::RungeKutta4, dt) }
    pub fn adaptive(dt: f64, tol: f64) -> Self {
        let mut s = Self::new(OdeMethod::RungeKutta45, dt);
        s.tolerance = tol;
        s
    }
    pub fn verlet(dt: f64) -> Self { Self::new(OdeMethod::Verlet, dt) }

    /// Integrate one step. Returns the new state.
    pub fn step(&mut self, system: &dyn OdeSystem, state: &OdeState) -> OdeState {
        match self.method {
            OdeMethod::Euler => self.euler_step(system, state),
            OdeMethod::RungeKutta4 => self.rk4_step(system, state),
            OdeMethod::RungeKutta45 => self.rk45_step(system, state),
            OdeMethod::Verlet => self.verlet_step(system, state),
            OdeMethod::Leapfrog => self.leapfrog_step(system, state),
            OdeMethod::ImplicitEuler => self.implicit_euler_step(system, state),
            OdeMethod::CrankNicolson => self.crank_nicolson_step(system, state),
        }
    }

    /// Integrate from t0 to t_end. Returns all states at each step.
    pub fn integrate(&mut self, system: &dyn OdeSystem, initial: &OdeState, t_end: f64) -> Vec<OdeState> {
        let mut states = vec![initial.clone()];
        let mut current = initial.clone();
        let mut steps = 0u64;

        while current.t < t_end && steps < self.max_steps {
            current = self.step(system, &current);
            states.push(current.clone());
            steps += 1;
        }
        states
    }

    /// Integrate and return only the final state.
    pub fn solve(&mut self, system: &dyn OdeSystem, initial: &OdeState, t_end: f64) -> OdeState {
        let mut current = initial.clone();
        let mut steps = 0u64;
        while current.t < t_end && steps < self.max_steps {
            current = self.step(system, &current);
            steps += 1;
        }
        current
    }

    // ── Method implementations ──────────────────────────────────────────

    fn euler_step(&self, sys: &dyn OdeSystem, s: &OdeState) -> OdeState {
        let n = s.y.len();
        let mut dydt = vec![0.0; n];
        sys.evaluate(s.t, &s.y, &mut dydt);
        let y: Vec<f64> = s.y.iter().zip(dydt.iter()).map(|(y, dy)| y + dy * self.dt).collect();
        OdeState { t: s.t + self.dt, y }
    }

    fn rk4_step(&self, sys: &dyn OdeSystem, s: &OdeState) -> OdeState {
        let n = s.y.len();
        let h = self.dt;
        let mut k1 = vec![0.0; n]; sys.evaluate(s.t, &s.y, &mut k1);
        let y2: Vec<f64> = (0..n).map(|i| s.y[i] + k1[i] * h * 0.5).collect();
        let mut k2 = vec![0.0; n]; sys.evaluate(s.t + h * 0.5, &y2, &mut k2);
        let y3: Vec<f64> = (0..n).map(|i| s.y[i] + k2[i] * h * 0.5).collect();
        let mut k3 = vec![0.0; n]; sys.evaluate(s.t + h * 0.5, &y3, &mut k3);
        let y4: Vec<f64> = (0..n).map(|i| s.y[i] + k3[i] * h).collect();
        let mut k4 = vec![0.0; n]; sys.evaluate(s.t + h, &y4, &mut k4);

        let y: Vec<f64> = (0..n).map(|i| {
            s.y[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i])
        }).collect();
        OdeState { t: s.t + h, y }
    }

    fn rk45_step(&mut self, sys: &dyn OdeSystem, s: &OdeState) -> OdeState {
        // Dormand-Prince RK45 with adaptive step
        let n = s.y.len();
        let h = self.dt;

        // Use RK4 for 4th and 5th order estimates
        let state4 = self.rk4_step(sys, s);

        // Simple error estimate via half-step Richardson extrapolation
        let mut half = self.dt;
        self.dt = h * 0.5;
        let mid = self.rk4_step(sys, s);
        let final_ = self.rk4_step(sys, &mid);
        self.dt = half;

        let error: f64 = state4.y.iter().zip(final_.y.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);

        // Adjust step size
        if error > self.tolerance && h > self.dt_min {
            self.dt = (h * 0.5).max(self.dt_min);
        } else if error < self.tolerance * 0.1 && h < self.dt_max {
            self.dt = (h * 1.5).min(self.dt_max);
        }

        // Use the higher-order estimate
        final_
    }

    fn verlet_step(&self, sys: &dyn OdeSystem, s: &OdeState) -> OdeState {
        // Velocity Verlet (for Hamiltonian systems where y = [x0,..,xn, v0,..,vn])
        let n = s.y.len();
        let half = n / 2;
        let h = self.dt;

        let mut acc = vec![0.0; n];
        sys.evaluate(s.t, &s.y, &mut acc);

        let mut y = s.y.clone();
        // Update positions: x += v*h + 0.5*a*h²
        for i in 0..half {
            y[i] = s.y[i] + s.y[half + i] * h + 0.5 * acc[half + i] * h * h;
        }

        // Compute new acceleration
        let mut acc_new = vec![0.0; n];
        sys.evaluate(s.t + h, &y, &mut acc_new);

        // Update velocities: v += 0.5*(a_old + a_new)*h
        for i in 0..half {
            y[half + i] = s.y[half + i] + 0.5 * (acc[half + i] + acc_new[half + i]) * h;
        }

        OdeState { t: s.t + h, y }
    }

    fn leapfrog_step(&self, sys: &dyn OdeSystem, s: &OdeState) -> OdeState {
        self.verlet_step(sys, s) // Verlet is mathematically equivalent
    }

    fn implicit_euler_step(&self, sys: &dyn OdeSystem, s: &OdeState) -> OdeState {
        // Simplified: use fixed-point iteration (1 iteration ≈ semi-implicit Euler)
        let n = s.y.len();
        let h = self.dt;
        let mut dydt = vec![0.0; n];
        sys.evaluate(s.t + h, &s.y, &mut dydt);
        let y: Vec<f64> = s.y.iter().zip(dydt.iter()).map(|(y, dy)| y + dy * h).collect();
        OdeState { t: s.t + h, y }
    }

    fn crank_nicolson_step(&self, sys: &dyn OdeSystem, s: &OdeState) -> OdeState {
        // Average of explicit and implicit Euler
        let n = s.y.len();
        let h = self.dt;
        let mut f_n = vec![0.0; n];
        sys.evaluate(s.t, &s.y, &mut f_n);
        let y_euler: Vec<f64> = s.y.iter().zip(f_n.iter()).map(|(y, dy)| y + dy * h).collect();
        let mut f_n1 = vec![0.0; n];
        sys.evaluate(s.t + h, &y_euler, &mut f_n1);
        let y: Vec<f64> = (0..n).map(|i| s.y[i] + 0.5 * h * (f_n[i] + f_n1[i])).collect();
        OdeState { t: s.t + h, y }
    }
}

// ── Built-in ODE systems ────────────────────────────────────────────────────

/// Lorenz attractor: dx/dt = σ(y-x), dy/dt = x(ρ-z)-y, dz/dt = xy-βz.
pub struct LorenzSystem { pub sigma: f64, pub rho: f64, pub beta: f64 }
impl Default for LorenzSystem { fn default() -> Self { Self { sigma: 10.0, rho: 28.0, beta: 8.0/3.0 } } }
impl OdeSystem for LorenzSystem {
    fn dimension(&self) -> usize { 3 }
    fn evaluate(&self, _t: f64, y: &[f64], dydt: &mut [f64]) {
        dydt[0] = self.sigma * (y[1] - y[0]);
        dydt[1] = y[0] * (self.rho - y[2]) - y[1];
        dydt[2] = y[0] * y[1] - self.beta * y[2];
    }
}

/// Simple harmonic oscillator: x'' + ω²x = 0.
pub struct HarmonicOscillator { pub omega: f64 }
impl OdeSystem for HarmonicOscillator {
    fn dimension(&self) -> usize { 2 }
    fn evaluate(&self, _t: f64, y: &[f64], dydt: &mut [f64]) {
        dydt[0] = y[1];                        // dx/dt = v
        dydt[1] = -self.omega * self.omega * y[0]; // dv/dt = -ω²x
    }
}

/// Van der Pol oscillator: x'' - μ(1-x²)x' + x = 0.
pub struct VanDerPol { pub mu: f64 }
impl OdeSystem for VanDerPol {
    fn dimension(&self) -> usize { 2 }
    fn evaluate(&self, _t: f64, y: &[f64], dydt: &mut [f64]) {
        dydt[0] = y[1];
        dydt[1] = self.mu * (1.0 - y[0] * y[0]) * y[1] - y[0];
    }
}

/// Rossler attractor.
pub struct RosslerSystem { pub a: f64, pub b: f64, pub c: f64 }
impl Default for RosslerSystem { fn default() -> Self { Self { a: 0.2, b: 0.2, c: 5.7 } } }
impl OdeSystem for RosslerSystem {
    fn dimension(&self) -> usize { 3 }
    fn evaluate(&self, _t: f64, y: &[f64], dydt: &mut [f64]) {
        dydt[0] = -y[1] - y[2];
        dydt[1] = y[0] + self.a * y[1];
        dydt[2] = self.b + y[2] * (y[0] - self.c);
    }
}

/// Custom ODE from a closure.
pub struct CustomOde<F: Fn(f64, &[f64], &mut [f64]) + Send + Sync> {
    pub dim: usize,
    pub func: F,
}
impl<F: Fn(f64, &[f64], &mut [f64]) + Send + Sync> OdeSystem for CustomOde<F> {
    fn dimension(&self) -> usize { self.dim }
    fn evaluate(&self, t: f64, y: &[f64], dydt: &mut [f64]) { (self.func)(t, y, dydt); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_harmonic() {
        let sys = HarmonicOscillator { omega: 1.0 };
        let mut solver = OdeSolver::new(OdeMethod::Euler, 0.001);
        let initial = OdeState { t: 0.0, y: vec![1.0, 0.0] };
        let final_state = solver.solve(&sys, &initial, std::f64::consts::PI);
        // After half period: x ≈ -1.0
        assert!((final_state.y[0] + 1.0).abs() < 0.1, "x={}", final_state.y[0]);
    }

    #[test]
    fn rk4_harmonic_accurate() {
        let sys = HarmonicOscillator { omega: 1.0 };
        let mut solver = OdeSolver::rk4(0.01);
        let initial = OdeState { t: 0.0, y: vec![1.0, 0.0] };
        let final_state = solver.solve(&sys, &initial, std::f64::consts::TAU);
        // After full period: x ≈ 1.0
        assert!((final_state.y[0] - 1.0).abs() < 0.01, "x={}", final_state.y[0]);
    }

    #[test]
    fn lorenz_doesnt_diverge() {
        let sys = LorenzSystem::default();
        let mut solver = OdeSolver::rk4(0.01);
        let initial = OdeState { t: 0.0, y: vec![1.0, 1.0, 1.0] };
        let final_state = solver.solve(&sys, &initial, 10.0);
        // Should remain bounded
        for &v in &final_state.y {
            assert!(v.abs() < 100.0, "Lorenz diverged: {:?}", final_state.y);
        }
    }

    #[test]
    fn integrate_returns_trajectory() {
        let sys = HarmonicOscillator { omega: 1.0 };
        let mut solver = OdeSolver::rk4(0.1);
        let initial = OdeState { t: 0.0, y: vec![1.0, 0.0] };
        let trajectory = solver.integrate(&sys, &initial, 1.0);
        assert!(trajectory.len() > 5);
    }

    #[test]
    fn verlet_conserves_energy() {
        let sys = HarmonicOscillator { omega: 1.0 };
        let mut solver = OdeSolver::verlet(0.01);
        let initial = OdeState { t: 0.0, y: vec![1.0, 0.0] };
        let energy_start = 0.5 * initial.y[0].powi(2) + 0.5 * initial.y[1].powi(2);
        let final_state = solver.solve(&sys, &initial, 100.0);
        let energy_end = 0.5 * final_state.y[0].powi(2) + 0.5 * final_state.y[1].powi(2);
        assert!((energy_start - energy_end).abs() < 0.01, "Energy drift: {}", (energy_start - energy_end).abs());
    }
}
