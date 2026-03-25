//! Food web simulation — directed graph of trophic relationships.

use std::collections::{HashMap, HashSet};

/// A node in the food web.
#[derive(Debug, Clone)]
pub struct WebNode {
    pub species_id: u32,
    pub name: String,
    pub trophic_level: f32,
    pub biomass: f64,
}

/// An edge in the food web (energy flow from prey to predator).
#[derive(Debug, Clone)]
pub struct WebEdge {
    pub prey: u32,
    pub predator: u32,
    pub transfer_efficiency: f64,
}

/// The food web graph.
#[derive(Debug, Clone)]
pub struct FoodWeb {
    pub nodes: Vec<WebNode>,
    pub edges: Vec<WebEdge>,
}

impl FoodWeb {
    pub fn new() -> Self { Self { nodes: Vec::new(), edges: Vec::new() } }

    pub fn add_node(&mut self, node: WebNode) { self.nodes.push(node); }
    pub fn add_edge(&mut self, edge: WebEdge) { self.edges.push(edge); }

    /// Connectance: fraction of possible links that exist.
    pub fn connectance(&self) -> f64 {
        let n = self.nodes.len() as f64;
        if n < 2.0 { return 0.0; }
        self.edges.len() as f64 / (n * (n - 1.0))
    }

    /// Find all prey of a predator.
    pub fn prey_of(&self, predator_id: u32) -> Vec<u32> {
        self.edges.iter().filter(|e| e.predator == predator_id).map(|e| e.prey).collect()
    }

    /// Find all predators of a prey.
    pub fn predators_of(&self, prey_id: u32) -> Vec<u32> {
        self.edges.iter().filter(|e| e.prey == prey_id).map(|e| e.predator).collect()
    }

    /// Compute trophic levels using shortest path from producers.
    pub fn compute_trophic_levels(&mut self) {
        let producers: Vec<u32> = self.nodes.iter()
            .filter(|n| self.prey_of(n.species_id).is_empty())
            .map(|n| n.species_id)
            .collect();

        for node in &mut self.nodes {
            if producers.contains(&node.species_id) {
                node.trophic_level = 1.0;
            }
        }

        // BFS-like propagation
        for _ in 0..10 {
            for i in 0..self.nodes.len() {
                let id = self.nodes[i].species_id;
                let prey_levels: Vec<f32> = self.prey_of(id).iter()
                    .filter_map(|&pid| self.nodes.iter().find(|n| n.species_id == pid))
                    .map(|n| n.trophic_level)
                    .collect();
                if !prey_levels.is_empty() {
                    let avg: f32 = prey_levels.iter().sum::<f32>() / prey_levels.len() as f32;
                    self.nodes[i].trophic_level = avg + 1.0;
                }
            }
        }
    }

    /// Total energy flow through the web.
    pub fn total_energy_flow(&self) -> f64 {
        self.edges.iter().map(|e| {
            let prey_biomass = self.nodes.iter()
                .find(|n| n.species_id == e.prey)
                .map(|n| n.biomass)
                .unwrap_or(0.0);
            prey_biomass * e.transfer_efficiency
        }).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_food_web() {
        let mut web = FoodWeb::new();
        web.add_node(WebNode { species_id: 0, name: "Grass".into(), trophic_level: 1.0, biomass: 1000.0 });
        web.add_node(WebNode { species_id: 1, name: "Rabbit".into(), trophic_level: 2.0, biomass: 100.0 });
        web.add_node(WebNode { species_id: 2, name: "Fox".into(), trophic_level: 3.0, biomass: 10.0 });
        web.add_edge(WebEdge { prey: 0, predator: 1, transfer_efficiency: 0.1 });
        web.add_edge(WebEdge { prey: 1, predator: 2, transfer_efficiency: 0.1 });

        assert_eq!(web.prey_of(1), vec![0]);
        assert_eq!(web.predators_of(1), vec![2]);
        assert!(web.connectance() > 0.0);
    }
}
