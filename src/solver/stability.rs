//! Stability analysis — automatic step size selection and stiff equation detection.

use super::ode::{OdeSystem, OdeState, OdeSolver, OdeMethod};

/// Results of a stability analysis.
#[derive(Debug, Clone)]
pub struct StabilityAnalysis {
    pub is_stiff: bool,
    pub recommended_method: OdeMethod,
    pub recommended_dt: f64,
    pub max_eigenvalue_estimate: f64,
    pub stability_ratio: f64,
}

impl StabilityAnalysis {
    /// Analyze an ODE system at the given state to determine stiffness and recommend solver settings.
    pub fn analyze(system: &dyn OdeSystem, state: &OdeState, dt: f64) -> Self {
        let n = system.dimension();
        let h = 1e-6;

        // Estimate Jacobian eigenvalues via power iteration on finite differences
        let mut max_eig = 0.0f64;
        let mut min_eig = f64::MAX;

        let mut f0 = vec![0.0; n];
        system.evaluate(state.t, &state.y, &mut f0);

        for j in 0..n {
            let mut y_perturbed = state.y.clone();
            y_perturbed[j] += h;
            let mut f1 = vec![0.0; n];
            system.evaluate(state.t, &y_perturbed, &mut f1);

            // Approximate column j of the Jacobian
            let col_norm: f64 = (0..n).map(|i| ((f1[i] - f0[i]) / h).powi(2)).sum::<f64>().sqrt();
            if col_norm > max_eig { max_eig = col_norm; }
            if col_norm < min_eig { min_eig = col_norm; }
        }

        let stiffness_ratio = if min_eig > 1e-15 { max_eig / min_eig } else { 1.0 };
        let is_stiff = stiffness_ratio > 1000.0;

        let stability_limit = if max_eig > 1e-15 { 2.8 / max_eig } else { dt };
        let recommended_dt = (stability_limit * 0.8).min(dt);

        let recommended_method = if is_stiff {
            OdeMethod::ImplicitEuler
        } else if stiffness_ratio > 100.0 {
            OdeMethod::RungeKutta45
        } else {
            OdeMethod::RungeKutta4
        };

        Self {
            is_stiff,
            recommended_method,
            recommended_dt,
            max_eigenvalue_estimate: max_eig,
            stability_ratio: stiffness_ratio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::ode::HarmonicOscillator;

    #[test]
    fn harmonic_oscillator_not_stiff() {
        let sys = HarmonicOscillator { omega: 1.0 };
        let state = OdeState { t: 0.0, y: vec![1.0, 0.0] };
        let analysis = StabilityAnalysis::analyze(&sys, &state, 0.01);
        assert!(!analysis.is_stiff);
        assert!(analysis.recommended_dt > 0.0);
    }
}
