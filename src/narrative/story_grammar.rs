//! Story grammar system — generate plot structures from narrative grammars.
//!
//! Uses context-free grammars with weighted productions to generate
//! narrative arcs: setup, rising action, climax, falling action, resolution.

use crate::worldgen::Rng;
use std::collections::HashMap;

/// A story beat (atomic narrative unit).
#[derive(Debug, Clone)]
pub struct StoryBeat {
    pub beat_type: BeatType,
    pub description: String,
    pub tension_delta: f32,
    pub characters_involved: Vec<String>,
    pub location: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BeatType {
    Introduction, Incitement, RisingAction, Complication, Crisis,
    Climax, Reversal, FallingAction, Resolution, Denouement,
    Reveal, Betrayal, Sacrifice, Reunion, Discovery, Loss, Victory, Defeat,
}

impl BeatType {
    pub fn tension_contribution(self) -> f32 {
        match self {
            Self::Introduction => 0.1, Self::Incitement => 0.3, Self::RisingAction => 0.2,
            Self::Complication => 0.3, Self::Crisis => 0.4, Self::Climax => 0.5,
            Self::Reversal => -0.2, Self::FallingAction => -0.3, Self::Resolution => -0.4,
            Self::Denouement => -0.2, Self::Reveal => 0.2, Self::Betrayal => 0.4,
            Self::Sacrifice => 0.1, Self::Reunion => -0.1, Self::Discovery => 0.15,
            Self::Loss => 0.3, Self::Victory => -0.3, Self::Defeat => 0.2,
        }
    }
}

/// A production rule in the story grammar.
#[derive(Debug, Clone)]
pub struct Production {
    pub symbol: String,
    pub expansions: Vec<(Vec<String>, f32)>, // (expansion, weight)
}

/// A story grammar.
#[derive(Debug, Clone)]
pub struct StoryGrammar {
    pub productions: HashMap<String, Vec<(Vec<String>, f32)>>,
    pub terminals: HashMap<String, Vec<String>>,
}

impl StoryGrammar {
    /// Create the default narrative grammar.
    pub fn default_grammar() -> Self {
        let mut productions = HashMap::new();
        let mut terminals = HashMap::new();

        // Story structure
        productions.insert("STORY".to_string(), vec![
            (vec!["SETUP".into(), "CONFLICT".into(), "RESOLUTION".into()], 0.5),
            (vec!["SETUP".into(), "CONFLICT".into(), "TWIST".into(), "RESOLUTION".into()], 0.3),
            (vec!["IN_MEDIAS_RES".into(), "FLASHBACK".into(), "CONFLICT".into(), "RESOLUTION".into()], 0.2),
        ]);

        productions.insert("SETUP".to_string(), vec![
            (vec!["INTRODUCE_HERO".into(), "ESTABLISH_WORLD".into()], 0.5),
            (vec!["ESTABLISH_WORLD".into(), "INTRODUCE_HERO".into(), "INTRODUCE_MENTOR".into()], 0.3),
            (vec!["INTRODUCE_HERO".into(), "ORDINARY_WORLD".into()], 0.2),
        ]);

        productions.insert("CONFLICT".to_string(), vec![
            (vec!["INCITING_INCIDENT".into(), "RISING_ACTION".into(), "CLIMAX".into()], 0.4),
            (vec!["INCITING_INCIDENT".into(), "COMPLICATION".into(), "RISING_ACTION".into(), "CRISIS".into(), "CLIMAX".into()], 0.4),
            (vec!["CALL_TO_ADVENTURE".into(), "TRIALS".into(), "ORDEAL".into()], 0.2),
        ]);

        productions.insert("RESOLUTION".to_string(), vec![
            (vec!["FALLING_ACTION".into(), "DENOUEMENT".into()], 0.5),
            (vec!["AFTERMATH".into(), "NEW_NORMAL".into()], 0.3),
            (vec!["BITTERSWEET_END".into()], 0.2),
        ]);

        // Terminal beat templates
        terminals.insert("INTRODUCE_HERO".into(), vec![
            "A {hero} lives in {place}, unaware of their destiny.".into(),
            "In {place}, {hero} goes about their ordinary life.".into(),
        ]);
        terminals.insert("INCITING_INCIDENT".into(), vec![
            "A {threat} appears, shattering the peace of {place}.".into(),
            "{hero} discovers a {secret} that changes everything.".into(),
        ]);
        terminals.insert("CLIMAX".into(), vec![
            "{hero} faces {villain} in a final confrontation.".into(),
            "Everything comes to a head at {place}.".into(),
        ]);

        Self { productions, terminals }
    }

    /// Generate a story outline from the grammar.
    pub fn generate(&self, rng: &mut Rng) -> Vec<String> {
        let mut result = Vec::new();
        self.expand("STORY", rng, &mut result, 0);
        result
    }

    fn expand(&self, symbol: &str, rng: &mut Rng, result: &mut Vec<String>, depth: usize) {
        if depth > 20 { return; } // prevent infinite recursion

        if let Some(templates) = self.terminals.get(symbol) {
            if let Some(t) = rng.pick(templates) {
                result.push(t.clone());
            }
            return;
        }

        if let Some(expansions) = self.productions.get(symbol) {
            // Weighted random selection
            let total: f32 = expansions.iter().map(|(_, w)| w).sum();
            let mut target = rng.next_f32() * total;
            for (expansion, weight) in expansions {
                target -= weight;
                if target <= 0.0 {
                    for sym in expansion {
                        self.expand(sym, rng, result, depth + 1);
                    }
                    return;
                }
            }
            // Fallback: first expansion
            if let Some((expansion, _)) = expansions.first() {
                for sym in expansion {
                    self.expand(sym, rng, result, depth + 1);
                }
            }
        } else {
            // Unknown symbol: treat as terminal
            result.push(format!("[{}]", symbol));
        }
    }
}

/// A complete generated story.
#[derive(Debug, Clone)]
pub struct Story {
    pub title: String,
    pub beats: Vec<StoryBeat>,
    pub theme: String,
    pub arc_type: ArcType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcType { HeroJourney, Tragedy, Comedy, Redemption, Mystery, Quest, Revenge }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grammar_generates() {
        let grammar = StoryGrammar::default_grammar();
        let mut rng = Rng::new(42);
        let outline = grammar.generate(&mut rng);
        assert!(!outline.is_empty(), "grammar should produce output");
    }

    #[test]
    fn test_beat_tension() {
        assert!(BeatType::Climax.tension_contribution() > BeatType::Introduction.tension_contribution());
        assert!(BeatType::Resolution.tension_contribution() < 0.0);
    }
}
