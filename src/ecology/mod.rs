//! Mathematical ecology simulation.
//!
//! Lotka-Volterra predator-prey, logistic growth, competition models,
//! food webs, migration, evolution, stability analysis, disease, symbiosis.
//! All dynamics are rendered in real time.

pub mod population;
pub mod food_web;
pub mod migration;
pub mod evolution;
pub mod disease;

use std::collections::HashMap;

/// A species in the ecosystem.
#[derive(Debug, Clone)]
pub struct Species {
    pub id: u32,
    pub name: String,
    pub population: f64,
    pub carrying_capacity: f64,
    pub growth_rate: f64,
    pub trophic_level: TrophicLevel,
    pub traits: SpeciesTraits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrophicLevel { Producer, PrimaryConsumer, SecondaryConsumer, ApexPredator, Decomposer }

#[derive(Debug, Clone, Copy)]
pub struct SpeciesTraits {
    pub size: f64,
    pub speed: f64,
    pub reproduction_rate: f64,
    pub lifespan: f64,
    pub aggression: f64,
}

/// An ecosystem containing multiple interacting species.
#[derive(Debug, Clone)]
pub struct Ecosystem {
    pub species: Vec<Species>,
    pub interactions: Vec<Interaction>,
    pub time: f64,
    pub history: Vec<(f64, Vec<f64>)>,
}

/// Interaction between two species.
#[derive(Debug, Clone)]
pub struct Interaction {
    pub predator: u32,
    pub prey: u32,
    pub interaction_type: InteractionType,
    pub strength: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionType {
    Predation,     // +/- (predator gains, prey loses)
    Competition,   // -/- (both lose)
    Mutualism,     // +/+ (both gain)
    Parasitism,    // +/- (parasite gains, host loses)
    Commensalism,  // +/0 (one gains, other unaffected)
}

impl Ecosystem {
    pub fn new() -> Self {
        Self { species: Vec::new(), interactions: Vec::new(), time: 0.0, history: Vec::new() }
    }

    pub fn add_species(&mut self, species: Species) {
        self.species.push(species);
    }

    pub fn add_interaction(&mut self, interaction: Interaction) {
        self.interactions.push(interaction);
    }

    /// Step the ecosystem by dt using Lotka-Volterra equations.
    pub fn step(&mut self, dt: f64) {
        let n = self.species.len();
        let mut dpop = vec![0.0f64; n];

        for i in 0..n {
            let s = &self.species[i];
            // Logistic growth: dN/dt = rN(1 - N/K)
            dpop[i] += s.growth_rate * s.population * (1.0 - s.population / s.carrying_capacity.max(1.0));
        }

        // Interaction effects
        for inter in &self.interactions {
            let pred_idx = self.species.iter().position(|s| s.id == inter.predator);
            let prey_idx = self.species.iter().position(|s| s.id == inter.prey);
            if let (Some(pi), Some(qi)) = (pred_idx, prey_idx) {
                let pred_pop = self.species[pi].population;
                let prey_pop = self.species[qi].population;
                let alpha = inter.strength;

                match inter.interaction_type {
                    InteractionType::Predation => {
                        dpop[pi] += alpha * pred_pop * prey_pop * 0.01;  // predator gains
                        dpop[qi] -= alpha * pred_pop * prey_pop * 0.01;  // prey loses
                    }
                    InteractionType::Competition => {
                        dpop[pi] -= alpha * pred_pop * prey_pop * 0.005;
                        dpop[qi] -= alpha * pred_pop * prey_pop * 0.005;
                    }
                    InteractionType::Mutualism => {
                        dpop[pi] += alpha * prey_pop * 0.001;
                        dpop[qi] += alpha * pred_pop * 0.001;
                    }
                    InteractionType::Parasitism => {
                        dpop[pi] += alpha * prey_pop * 0.002;
                        dpop[qi] -= alpha * pred_pop * 0.003;
                    }
                    InteractionType::Commensalism => {
                        dpop[pi] += alpha * prey_pop * 0.001;
                    }
                }
            }
        }

        // Apply
        for i in 0..n {
            self.species[i].population = (self.species[i].population + dpop[i] * dt).max(0.0);
        }

        self.time += dt;

        // Record history every ~1 time unit
        if self.history.is_empty() || self.time - self.history.last().unwrap().0 >= 1.0 {
            let pops: Vec<f64> = self.species.iter().map(|s| s.population).collect();
            self.history.push((self.time, pops));
        }
    }

    /// Run for a number of steps.
    pub fn simulate(&mut self, steps: usize, dt: f64) {
        for _ in 0..steps {
            self.step(dt);
        }
    }

    /// Lyapunov stability analysis: compute largest Lyapunov exponent.
    pub fn lyapunov_exponent(&self, dt: f64, steps: usize) -> f64 {
        let mut eco = self.clone();
        let mut perturbed = self.clone();
        // Small perturbation
        if !perturbed.species.is_empty() {
            perturbed.species[0].population *= 1.0001;
        }

        let mut sum_log = 0.0_f64;
        let d0 = (eco.species[0].population - perturbed.species[0].population).abs().max(1e-15);

        for i in 0..steps {
            eco.step(dt);
            perturbed.step(dt);

            let d = (eco.species[0].population - perturbed.species[0].population).abs().max(1e-15);
            sum_log += (d / d0).ln();

            // Renormalize perturbation
            if !perturbed.species.is_empty() {
                let scale = d0 / d;
                for j in 0..perturbed.species.len() {
                    let diff = perturbed.species[j].population - eco.species[j].population;
                    perturbed.species[j].population = eco.species[j].population + diff * scale;
                }
            }
        }

        sum_log / (steps as f64 * dt)
    }

    /// Total biomass.
    pub fn total_biomass(&self) -> f64 {
        self.species.iter().map(|s| s.population * s.traits.size).sum()
    }

    /// Species diversity (Shannon index).
    pub fn shannon_diversity(&self) -> f64 {
        let total: f64 = self.species.iter().map(|s| s.population).sum();
        if total < 1.0 { return 0.0; }
        -self.species.iter()
            .map(|s| {
                let p = s.population / total;
                if p > 0.0 { p * p.ln() } else { 0.0 }
            })
            .sum::<f64>()
    }
}

/// Create a classic predator-prey ecosystem.
pub fn lotka_volterra_example() -> Ecosystem {
    let mut eco = Ecosystem::new();
    eco.add_species(Species {
        id: 0, name: "Rabbit".to_string(), population: 100.0,
        carrying_capacity: 500.0, growth_rate: 0.5,
        trophic_level: TrophicLevel::PrimaryConsumer,
        traits: SpeciesTraits { size: 0.1, speed: 0.6, reproduction_rate: 0.8, lifespan: 5.0, aggression: 0.1 },
    });
    eco.add_species(Species {
        id: 1, name: "Fox".to_string(), population: 20.0,
        carrying_capacity: 100.0, growth_rate: -0.1,
        trophic_level: TrophicLevel::SecondaryConsumer,
        traits: SpeciesTraits { size: 0.3, speed: 0.8, reproduction_rate: 0.3, lifespan: 10.0, aggression: 0.7 },
    });
    eco.add_interaction(Interaction {
        predator: 1, prey: 0,
        interaction_type: InteractionType::Predation,
        strength: 0.5,
    });
    eco
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lotka_volterra() {
        let mut eco = lotka_volterra_example();
        eco.simulate(1000, 0.1);
        // Both species should survive
        assert!(eco.species[0].population > 0.0, "rabbits should survive");
        assert!(eco.species[1].population > 0.0, "foxes should survive");
    }

    #[test]
    fn test_shannon_diversity() {
        let eco = lotka_volterra_example();
        let h = eco.shannon_diversity();
        assert!(h > 0.0, "diversity should be positive with 2 species");
    }

    #[test]
    fn test_history_recording() {
        let mut eco = lotka_volterra_example();
        eco.simulate(100, 0.1);
        assert!(!eco.history.is_empty());
    }
}
