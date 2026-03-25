//! Character motivation engine — goals, beliefs, desires as utility functions.

use crate::worldgen::Rng;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Need { Survival, Safety, Belonging, Esteem, SelfActualization, Power, Revenge, Love, Knowledge, Wealth }

#[derive(Debug, Clone)]
pub struct Motivation {
    pub needs: HashMap<Need, f32>,
    pub goals: Vec<Goal>,
    pub beliefs: Vec<Belief>,
    pub personality: Personality,
}

#[derive(Debug, Clone)]
pub struct Goal { pub description: String, pub need: Need, pub priority: f32, pub progress: f32, pub completed: bool }

#[derive(Debug, Clone)]
pub struct Belief { pub subject: String, pub value: f32 }

#[derive(Debug, Clone, Copy)]
pub struct Personality {
    pub openness: f32, pub conscientiousness: f32, pub extraversion: f32,
    pub agreeableness: f32, pub neuroticism: f32,
}

impl Motivation {
    pub fn random(rng: &mut Rng) -> Self {
        let mut needs = HashMap::new();
        for &need in &[Need::Survival, Need::Safety, Need::Belonging, Need::Esteem, Need::Power, Need::Knowledge, Need::Wealth] {
            needs.insert(need, rng.range_f32(0.2, 0.8));
        }
        Self {
            needs,
            goals: Vec::new(),
            beliefs: Vec::new(),
            personality: Personality {
                openness: rng.next_f32(), conscientiousness: rng.next_f32(),
                extraversion: rng.next_f32(), agreeableness: rng.next_f32(),
                neuroticism: rng.next_f32(),
            },
        }
    }

    /// Evaluate utility of an action based on which needs it satisfies.
    pub fn evaluate_action(&self, need_impacts: &[(Need, f32)]) -> f32 {
        need_impacts.iter().map(|(need, impact)| {
            let weight = self.needs.get(need).copied().unwrap_or(0.0);
            weight * impact
        }).sum()
    }

    /// Choose the highest-priority unfinished goal.
    pub fn active_goal(&self) -> Option<&Goal> {
        self.goals.iter().filter(|g| !g.completed)
            .max_by(|a, b| a.priority.partial_cmp(&b.priority).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motivation_random() {
        let mut rng = Rng::new(42);
        let m = Motivation::random(&mut rng);
        assert!(!m.needs.is_empty());
    }

    #[test]
    fn test_evaluate_action() {
        let mut rng = Rng::new(42);
        let m = Motivation::random(&mut rng);
        let score = m.evaluate_action(&[(Need::Survival, 1.0), (Need::Wealth, 0.5)]);
        assert!(score > 0.0);
    }
}
