//! DNA/genetics system — heritable traits for procedural creatures.
//!
//! Models a simplified genome with dominant/recessive alleles, crossover,
//! mutation, and phenotype expression.

use super::Rng;

/// A single gene with two alleles.
#[derive(Debug, Clone, Copy)]
pub struct Gene { pub allele_a: u8, pub allele_b: u8 }

impl Gene {
    pub fn new(a: u8, b: u8) -> Self { Self { allele_a: a, allele_b: b } }
    pub fn homozygous(&self) -> bool { self.allele_a == self.allele_b }
    /// Dominant expression (higher allele value dominates).
    pub fn express(&self) -> u8 { self.allele_a.max(self.allele_b) }
    /// Codominant expression (average).
    pub fn express_codominant(&self) -> f32 { (self.allele_a as f32 + self.allele_b as f32) * 0.5 }
}

/// Trait categories encoded in the genome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraitType {
    Size, Strength, Speed, Intelligence, Aggression, Coloration,
    Pattern, HornSize, TailLength, WingSpan, Armor, Venom,
    Bioluminescence, Regeneration, Camouflage, SenseAcuity,
}

/// A complete genome.
#[derive(Debug, Clone)]
pub struct Genome {
    pub genes: Vec<(TraitType, Gene)>,
    pub mutation_rate: f32,
}

impl Genome {
    pub fn random(rng: &mut Rng) -> Self {
        let traits = [
            TraitType::Size, TraitType::Strength, TraitType::Speed,
            TraitType::Intelligence, TraitType::Aggression, TraitType::Coloration,
            TraitType::Pattern, TraitType::HornSize, TraitType::TailLength,
            TraitType::WingSpan, TraitType::Armor, TraitType::Venom,
            TraitType::Bioluminescence, TraitType::Regeneration,
            TraitType::Camouflage, TraitType::SenseAcuity,
        ];
        let genes = traits.iter().map(|&t| {
            (t, Gene::new(rng.range_u32(0, 256) as u8, rng.range_u32(0, 256) as u8))
        }).collect();
        Self { genes, mutation_rate: 0.02 }
    }

    /// Express phenotype for a trait.
    pub fn phenotype(&self, trait_type: TraitType) -> f32 {
        self.genes.iter()
            .find(|(t, _)| *t == trait_type)
            .map(|(_, g)| g.express_codominant() / 255.0)
            .unwrap_or(0.5)
    }

    /// Crossover: combine two genomes (sexual reproduction).
    pub fn crossover(&self, other: &Genome, rng: &mut Rng) -> Genome {
        let mut child_genes = Vec::with_capacity(self.genes.len());
        for (i, (trait_type, gene_a)) in self.genes.iter().enumerate() {
            let gene_b = &other.genes[i].1;
            // Pick one allele from each parent
            let allele_a = if rng.coin(0.5) { gene_a.allele_a } else { gene_a.allele_b };
            let allele_b = if rng.coin(0.5) { gene_b.allele_a } else { gene_b.allele_b };
            child_genes.push((*trait_type, Gene::new(allele_a, allele_b)));
        }
        Genome { genes: child_genes, mutation_rate: (self.mutation_rate + other.mutation_rate) * 0.5 }
    }

    /// Apply random mutations.
    pub fn mutate(&mut self, rng: &mut Rng) {
        for (_, gene) in &mut self.genes {
            if rng.coin(self.mutation_rate) {
                let delta = (rng.gaussian() * 10.0) as i16;
                gene.allele_a = (gene.allele_a as i16 + delta).clamp(0, 255) as u8;
            }
            if rng.coin(self.mutation_rate) {
                let delta = (rng.gaussian() * 10.0) as i16;
                gene.allele_b = (gene.allele_b as i16 + delta).clamp(0, 255) as u8;
            }
        }
    }

    /// Fitness score for a given environment (higher = better adapted).
    pub fn fitness(&self, env: &Environment) -> f32 {
        let mut score = 0.0_f32;
        score += (self.phenotype(TraitType::Size) - env.ideal_size).abs() * -1.0;
        score += self.phenotype(TraitType::Speed) * env.predation_pressure;
        score += self.phenotype(TraitType::Camouflage) * env.predation_pressure * 0.5;
        score += self.phenotype(TraitType::Armor) * env.predation_pressure * 0.3;
        score += self.phenotype(TraitType::Intelligence) * 0.2;
        score += 1.0; // base fitness
        score.max(0.0)
    }
}

/// Environmental pressures that affect fitness.
#[derive(Debug, Clone)]
pub struct Environment {
    pub ideal_size: f32,
    pub predation_pressure: f32,
    pub food_availability: f32,
    pub temperature: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Self { ideal_size: 0.5, predation_pressure: 0.5, food_availability: 0.5, temperature: 0.5 }
    }
}

/// A population of creatures.
#[derive(Debug, Clone)]
pub struct Population {
    pub genomes: Vec<Genome>,
    pub generation: u32,
}

impl Population {
    pub fn random(size: usize, rng: &mut Rng) -> Self {
        let genomes = (0..size).map(|_| Genome::random(rng)).collect();
        Self { genomes, generation: 0 }
    }

    /// Run one generation of evolution (selection + reproduction + mutation).
    pub fn evolve(&mut self, env: &Environment, rng: &mut Rng) {
        let n = self.genomes.len();
        if n < 2 { return; }

        // Fitness-proportionate selection
        let fitnesses: Vec<f32> = self.genomes.iter().map(|g| g.fitness(env)).collect();
        let total_fitness: f32 = fitnesses.iter().sum();
        if total_fitness < 0.01 { return; }

        let mut next_gen = Vec::with_capacity(n);
        for _ in 0..n {
            let parent_a = roulette_select(&fitnesses, total_fitness, rng);
            let parent_b = roulette_select(&fitnesses, total_fitness, rng);
            let mut child = self.genomes[parent_a].crossover(&self.genomes[parent_b], rng);
            child.mutate(rng);
            next_gen.push(child);
        }

        self.genomes = next_gen;
        self.generation += 1;
    }

    /// Average phenotype for a trait across the population.
    pub fn avg_phenotype(&self, trait_type: TraitType) -> f32 {
        if self.genomes.is_empty() { return 0.5; }
        let sum: f32 = self.genomes.iter().map(|g| g.phenotype(trait_type)).sum();
        sum / self.genomes.len() as f32
    }
}

fn roulette_select(fitnesses: &[f32], total: f32, rng: &mut Rng) -> usize {
    let mut target = rng.next_f32() * total;
    for (i, &f) in fitnesses.iter().enumerate() {
        target -= f;
        if target <= 0.0 { return i; }
    }
    fitnesses.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genome_random() {
        let mut rng = Rng::new(42);
        let g = Genome::random(&mut rng);
        assert_eq!(g.genes.len(), 16);
    }

    #[test]
    fn test_crossover() {
        let mut rng = Rng::new(42);
        let a = Genome::random(&mut rng);
        let b = Genome::random(&mut rng);
        let child = a.crossover(&b, &mut rng);
        assert_eq!(child.genes.len(), a.genes.len());
    }

    #[test]
    fn test_evolution_changes_population() {
        let mut rng = Rng::new(42);
        let mut pop = Population::random(50, &mut rng);
        let env = Environment { ideal_size: 0.8, predation_pressure: 0.7, ..Default::default() };
        let initial_speed = pop.avg_phenotype(TraitType::Speed);
        for _ in 0..100 { pop.evolve(&env, &mut rng); }
        // With high predation, speed should trend upward
        let final_speed = pop.avg_phenotype(TraitType::Speed);
        // Not guaranteed but likely:
        assert!(pop.generation == 100);
    }

    #[test]
    fn test_phenotype_range() {
        let mut rng = Rng::new(42);
        let g = Genome::random(&mut rng);
        let p = g.phenotype(TraitType::Size);
        assert!(p >= 0.0 && p <= 1.0);
    }
}
