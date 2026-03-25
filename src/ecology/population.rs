//! Population dynamics models.

/// Logistic growth: dN/dt = rN(1 - N/K).
pub fn logistic_growth(pop: f64, rate: f64, capacity: f64, dt: f64) -> f64 {
    (pop + rate * pop * (1.0 - pop / capacity.max(1.0)) * dt).max(0.0)
}

/// Lotka-Volterra predator-prey step.
/// Returns (new_prey, new_predator).
pub fn lotka_volterra_step(
    prey: f64, predator: f64,
    prey_growth: f64, predation_rate: f64,
    predator_death: f64, conversion_rate: f64,
    dt: f64,
) -> (f64, f64) {
    let dprey = (prey_growth * prey - predation_rate * prey * predator) * dt;
    let dpred = (conversion_rate * prey * predator - predator_death * predator) * dt;
    ((prey + dprey).max(0.0), (predator + dpred).max(0.0))
}

/// Lotka-Volterra competition between two species.
/// Returns (new_pop1, new_pop2).
pub fn competition_step(
    n1: f64, n2: f64,
    r1: f64, r2: f64,
    k1: f64, k2: f64,
    alpha12: f64, alpha21: f64,
    dt: f64,
) -> (f64, f64) {
    let dn1 = r1 * n1 * (1.0 - (n1 + alpha12 * n2) / k1) * dt;
    let dn2 = r2 * n2 * (1.0 - (n2 + alpha21 * n1) / k2) * dt;
    ((n1 + dn1).max(0.0), (n2 + dn2).max(0.0))
}

/// Allee effect: population growth rate decreases at low densities.
pub fn allee_growth(pop: f64, rate: f64, capacity: f64, allee_threshold: f64, dt: f64) -> f64 {
    let growth = rate * pop * (pop / allee_threshold - 1.0) * (1.0 - pop / capacity);
    (pop + growth * dt).max(0.0)
}

/// Beverton-Holt discrete recruitment model.
pub fn beverton_holt(pop: f64, r: f64, k: f64) -> f64 {
    r * pop / (1.0 + (r - 1.0) * pop / k)
}

/// Ricker model (discrete, density-dependent, can produce chaos).
pub fn ricker(pop: f64, r: f64, k: f64) -> f64 {
    pop * (r * (1.0 - pop / k)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logistic_approaches_capacity() {
        let mut pop = 10.0;
        for _ in 0..1000 {
            pop = logistic_growth(pop, 0.5, 100.0, 0.1);
        }
        assert!((pop - 100.0).abs() < 1.0, "should approach K=100: got {pop}");
    }

    #[test]
    fn test_lotka_volterra_oscillation() {
        let (mut prey, mut pred) = (100.0, 20.0);
        let mut max_prey = 0.0_f64;
        let mut min_prey = f64::MAX;
        for _ in 0..10000 {
            let (np, nd) = lotka_volterra_step(prey, pred, 0.5, 0.01, 0.3, 0.005, 0.01);
            prey = np; pred = nd;
            max_prey = max_prey.max(prey);
            min_prey = min_prey.min(prey);
        }
        assert!(max_prey > min_prey * 1.5, "should oscillate");
    }

    #[test]
    fn test_competition_coexistence() {
        let (mut n1, mut n2) = (50.0, 50.0);
        for _ in 0..10000 {
            let (a, b) = competition_step(n1, n2, 0.3, 0.3, 200.0, 200.0, 0.5, 0.5, 0.1);
            n1 = a; n2 = b;
        }
        assert!(n1 > 1.0 && n2 > 1.0, "coexistence with weak competition");
    }

    #[test]
    fn test_ricker_bounded() {
        let mut pop = 10.0;
        for _ in 0..100 {
            pop = ricker(pop, 2.0, 100.0);
            assert!(pop >= 0.0 && pop < 1000.0, "Ricker should stay bounded: {pop}");
        }
    }
}
