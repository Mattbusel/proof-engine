//! Conservation law verification — energy, momentum, mass conservation checks.

use super::ode::OdeState;

/// A conservation law to verify during simulation.
pub trait ConservationLaw: Send + Sync {
    fn name(&self) -> &str;
    fn evaluate(&self, state: &OdeState) -> f64;
}

/// Conservation check result.
#[derive(Debug, Clone)]
pub struct ConservationCheck {
    pub law_name: String,
    pub initial_value: f64,
    pub current_value: f64,
    pub absolute_error: f64,
    pub relative_error: f64,
    pub violated: bool,
}

/// Monitor for tracking conservation laws.
pub struct ConservationMonitor {
    laws: Vec<Box<dyn ConservationLaw>>,
    initial_values: Vec<f64>,
    tolerance: f64,
}

impl ConservationMonitor {
    pub fn new(tolerance: f64) -> Self {
        Self { laws: Vec::new(), initial_values: Vec::new(), tolerance }
    }

    pub fn add_law(&mut self, law: Box<dyn ConservationLaw>) {
        self.laws.push(law);
        self.initial_values.push(f64::NAN);
    }

    /// Initialize with the starting state.
    pub fn initialize(&mut self, state: &OdeState) {
        for (i, law) in self.laws.iter().enumerate() {
            self.initial_values[i] = law.evaluate(state);
        }
    }

    /// Check all conservation laws at the current state.
    pub fn check(&self, state: &OdeState) -> Vec<ConservationCheck> {
        self.laws.iter().zip(self.initial_values.iter()).map(|(law, &initial)| {
            let current = law.evaluate(state);
            let abs_err = (current - initial).abs();
            let rel_err = if initial.abs() > 1e-15 { abs_err / initial.abs() } else { abs_err };
            ConservationCheck {
                law_name: law.name().to_string(),
                initial_value: initial,
                current_value: current,
                absolute_error: abs_err,
                relative_error: rel_err,
                violated: rel_err > self.tolerance,
            }
        }).collect()
    }
}

/// Built-in: total energy for harmonic oscillator (E = 0.5*x² + 0.5*v²).
pub struct HarmonicEnergy;
impl ConservationLaw for HarmonicEnergy {
    fn name(&self) -> &str { "Harmonic Energy" }
    fn evaluate(&self, state: &OdeState) -> f64 {
        if state.y.len() >= 2 { 0.5 * state.y[0].powi(2) + 0.5 * state.y[1].powi(2) } else { 0.0 }
    }
}

/// Built-in: L2 norm (total "mass").
pub struct L2Norm;
impl ConservationLaw for L2Norm {
    fn name(&self) -> &str { "L2 Norm" }
    fn evaluate(&self, state: &OdeState) -> f64 {
        state.y.iter().map(|v| v * v).sum::<f64>().sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::ode::{OdeSolver, OdeMethod, HarmonicOscillator, OdeState};

    #[test]
    fn conservation_monitor_detects_good_solver() {
        let sys = HarmonicOscillator { omega: 1.0 };
        let initial = OdeState { t: 0.0, y: vec![1.0, 0.0] };

        let mut monitor = ConservationMonitor::new(0.01);
        monitor.add_law(Box::new(HarmonicEnergy));
        monitor.initialize(&initial);

        let mut solver = OdeSolver::rk4(0.01);
        let final_state = solver.solve(&sys, &initial, 10.0);
        let checks = monitor.check(&final_state);

        assert!(!checks[0].violated, "RK4 should conserve energy well: err={}", checks[0].relative_error);
    }
}
