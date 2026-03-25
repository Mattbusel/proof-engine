//! Consequence tracking — player actions create ripples through the narrative.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Consequence {
    pub action: String,
    pub effects: Vec<Effect>,
    pub timestamp: f64,
    pub resolved: bool,
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub target: String,
    pub change_type: ChangeType,
    pub magnitude: f32,
    pub delayed_until: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Reputation, Relationship, WorldState, QuestState, DialogueUnlock, AreaAccess, PriceChange,
}

/// Tracks all consequences of player actions.
#[derive(Debug, Clone)]
pub struct ConsequenceTracker {
    pub consequences: Vec<Consequence>,
    pub reputation: HashMap<String, f32>,
    pub world_flags: HashMap<String, bool>,
}

impl ConsequenceTracker {
    pub fn new() -> Self {
        Self { consequences: Vec::new(), reputation: HashMap::new(), world_flags: HashMap::new() }
    }

    pub fn record(&mut self, action: &str, effects: Vec<Effect>, time: f64) {
        for effect in &effects {
            match effect.change_type {
                ChangeType::Reputation => {
                    *self.reputation.entry(effect.target.clone()).or_insert(0.0) += effect.magnitude;
                }
                ChangeType::WorldState => {
                    self.world_flags.insert(effect.target.clone(), effect.magnitude > 0.0);
                }
                _ => {}
            }
        }
        self.consequences.push(Consequence { action: action.to_string(), effects, timestamp: time, resolved: false });
    }

    pub fn reputation_with(&self, faction: &str) -> f32 {
        self.reputation.get(faction).copied().unwrap_or(0.0)
    }

    pub fn flag_set(&self, flag: &str) -> bool {
        self.world_flags.get(flag).copied().unwrap_or(false)
    }

    /// Get pending delayed consequences at the given time.
    pub fn pending_at(&self, time: f64) -> Vec<&Consequence> {
        self.consequences.iter().filter(|c| !c.resolved && c.effects.iter().any(|e| e.delayed_until.map_or(false, |t| t <= time))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consequence_tracking() {
        let mut tracker = ConsequenceTracker::new();
        tracker.record("helped_farmer", vec![
            Effect { target: "village".into(), change_type: ChangeType::Reputation, magnitude: 10.0, delayed_until: None },
        ], 0.0);
        assert_eq!(tracker.reputation_with("village"), 10.0);
    }
}
