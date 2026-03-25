//! Evolution simulation — mutation, selection, genetic drift.

use crate::worldgen::Rng;

/// Simplified trait for evolution (not full genome — see worldgen::genetics for that).
#[derive(Debug, Clone)]
pub struct EvolvingTrait {
    pub name: String,
    pub value: f64,
    pub heritability: f64,
    pub mutation_variance: f64,
}

/// Evolve a trait over one generation given selection pressure.
pub fn evolve_trait(trait_val: &mut EvolvingTrait, selection_optimum: f64, pop_size: usize, rng: &mut Rng) {
    // Selection: move toward optimum
    let selection_diff = selection_optimum - trait_val.value;
    let selection_response = selection_diff * trait_val.heritability * 0.1;

    // Genetic drift: random fluctuation inversely proportional to pop size
    let drift = rng.gaussian() * (1.0 / (pop_size as f64).sqrt()) * trait_val.mutation_variance;

    // Mutation
    let mutation = rng.gaussian() * trait_val.mutation_variance * 0.01;

    trait_val.value += selection_response + drift + mutation;
}

/// Simulate speciation check: if trait divergence exceeds threshold.
pub fn check_speciation(pop_a_trait: f64, pop_b_trait: f64, threshold: f64) -> bool {
    (pop_a_trait - pop_b_trait).abs() > threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evolve_trait_toward_optimum() {
        let mut rng = Rng::new(42);
        let mut t = EvolvingTrait {
            name: "size".to_string(), value: 0.5, heritability: 0.8, mutation_variance: 0.1,
        };
        for _ in 0..1000 {
            evolve_trait(&mut t, 0.9, 100, &mut rng);
        }
        assert!(t.value > 0.6, "trait should move toward optimum: {}", t.value);
    }
}
