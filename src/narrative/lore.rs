//! Lore generation — create histories, myths, and cultural artifacts from simulation data.

use crate::worldgen::Rng;
use crate::worldgen::history::Civilization;

/// A lore entry.
#[derive(Debug, Clone)]
pub struct LoreEntry {
    pub title: String,
    pub category: LoreCategory,
    pub text: String,
    pub source_civ: Option<u32>,
    pub reliability: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoreCategory { History, Legend, Rumor, Scripture, Chronicle, Folklore, Science, Forbidden }

/// Generate lore entries from civilization data.
pub fn generate_lore(civs: &[Civilization], rng: &mut Rng) -> Vec<LoreEntry> {
    let mut entries = Vec::new();

    for civ in civs {
        // Historical chronicle
        entries.push(LoreEntry {
            title: format!("Chronicles of {}", civ.name),
            category: LoreCategory::Chronicle,
            text: format!(
                "The {} civilization was founded in the year {}. \
                 Under {} rule, they grew to number {} souls. \
                 Their legacy endures in {} great works.",
                civ.name, civ.founding_year,
                format!("{:?}", civ.government).to_lowercase(),
                civ.population,
                (civ.culture_score * 10.0) as u32,
            ),
            source_civ: Some(civ.id),
            reliability: 0.8,
        });

        // Folklore
        if rng.coin(0.7) {
            entries.push(LoreEntry {
                title: format!("Tales of the {}", civ.name),
                category: LoreCategory::Folklore,
                text: format!(
                    "The people of {} tell stories of a time before time, \
                     when the world was young and {} walked among mortals.",
                    civ.name,
                    if civ.religion == super::super::worldgen::history::ReligionType::Polytheism { "the gods" }
                    else { "the divine" },
                ),
                source_civ: Some(civ.id),
                reliability: 0.3,
            });
        }

        // Forbidden knowledge
        if civ.technology_level > 0.7 && rng.coin(0.3) {
            entries.push(LoreEntry {
                title: format!("The Forbidden Texts of {}", civ.name),
                category: LoreCategory::Forbidden,
                text: format!(
                    "Hidden in the deepest vaults of {}, these texts describe \
                     experiments that blur the line between mathematics and madness.",
                    civ.name,
                ),
                source_civ: Some(civ.id),
                reliability: 0.5,
            });
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::history::*;

    #[test]
    fn test_generate_lore() {
        let mut rng = Rng::new(42);
        let civs = vec![Civilization {
            id: 0, name: "Eldheim".to_string(), founding_year: -2000, collapse_year: None,
            capital_settlement: 0, settlements: vec![0], population: 50000,
            technology_level: 0.8, military_strength: 0.5, culture_score: 0.7,
            trade_score: 0.4, government: GovernmentType::Republic,
            religion: ReligionType::Polytheism, relations: Vec::new(),
            historical_events: Vec::new(), traits: Vec::new(),
        }];
        let lore = generate_lore(&civs, &mut rng);
        assert!(!lore.is_empty());
        assert!(lore.iter().any(|l| l.category == LoreCategory::Chronicle));
    }
}
