//! History generation — agent-based civilization simulation.
//!
//! Simulates civilizations over thousands of years: founding, expansion,
//! warfare, trade, cultural development, and decline.

use super::{Rng};
use super::biomes::BiomeMap;
use super::settlements::Settlement;

/// A civilization in the world.
#[derive(Debug, Clone)]
pub struct Civilization {
    pub id: u32,
    pub name: String,
    pub founding_year: i32,
    pub collapse_year: Option<i32>,
    pub capital_settlement: u32,
    pub settlements: Vec<u32>,
    pub population: u64,
    pub technology_level: f32,
    pub military_strength: f32,
    pub culture_score: f32,
    pub trade_score: f32,
    pub government: GovernmentType,
    pub religion: ReligionType,
    pub relations: Vec<(u32, Relation)>,
    pub historical_events: Vec<HistoricalEvent>,
    pub traits: Vec<CivTrait>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GovernmentType {
    Tribal, Monarchy, Republic, Theocracy, Empire, Oligarchy, Democracy, Anarchy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReligionType {
    Animism, Polytheism, Monotheism, Philosophy, Ancestor, Nature, Void, Chaos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Allied, Friendly, Neutral, Rival, AtWar, Vassal, Overlord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CivTrait {
    Warlike, Peaceful, Mercantile, Scholarly, Nomadic, Seafaring, Religious, Isolationist,
}

/// A historical event.
#[derive(Debug, Clone)]
pub struct HistoricalEvent {
    pub year: i32,
    pub event_type: EventType,
    pub description: String,
    pub participants: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Founding, War, Peace, Trade, Discovery, Plague, Famine, GoldenAge,
    Collapse, Revolution, Migration, Alliance, Betrayal, HeroRise,
    ArtifactCreation, TempleBuilt, CityFounded, GreatWork,
}

/// Simulate civilization history.
pub fn simulate(
    settlements: &[Settlement],
    biome_map: &BiomeMap,
    years: usize,
    num_civs: usize,
    rng: &mut Rng,
) -> Vec<Civilization> {
    let num_civs = num_civs.min(settlements.len());
    if num_civs == 0 { return Vec::new(); }

    // Found initial civilizations at best settlements
    let mut civs: Vec<Civilization> = (0..num_civs)
        .map(|i| {
            let settlement = &settlements[i];
            let traits = vec![random_trait(rng), random_trait(rng)];
            Civilization {
                id: i as u32,
                name: generate_civ_name(rng),
                founding_year: -(rng.range_u32(500, years as u32) as i32),
                collapse_year: None,
                capital_settlement: settlement.id,
                settlements: vec![settlement.id],
                population: rng.range_usize(1000, 10000) as u64,
                technology_level: rng.range_f32(0.1, 0.3),
                military_strength: rng.range_f32(0.1, 0.5),
                culture_score: rng.range_f32(0.1, 0.4),
                trade_score: rng.range_f32(0.1, 0.3),
                government: random_government(rng),
                religion: random_religion(rng),
                relations: Vec::new(),
                historical_events: Vec::new(),
                traits,
            }
        })
        .collect();

    // Simulate year by year
    for year in 0..years as i32 {
        let adjusted_year = year - (years as i32 / 2);

        for ci in 0..civs.len() {
            if civs[ci].collapse_year.is_some() { continue; }

            // Population growth
            let growth = 1.0 + 0.01 * civs[ci].technology_level as f64;
            civs[ci].population = (civs[ci].population as f64 * growth) as u64;

            // Technology advancement
            civs[ci].technology_level += rng.range_f32(0.0, 0.005);

            // Random events
            let event_roll = rng.next_f32();
            if event_roll < 0.01 {
                // War
                if civs.len() > 1 {
                    let target = rng.range_usize(0, civs.len());
                    if target != ci && civs[target].collapse_year.is_none() {
                        civs[ci].historical_events.push(HistoricalEvent {
                            year: adjusted_year,
                            event_type: EventType::War,
                            description: format!("War with {}", civs[target].name),
                            participants: vec![civs[ci].id, civs[target].id],
                        });
                    }
                }
            } else if event_roll < 0.02 {
                // Discovery
                civs[ci].technology_level += 0.05;
                civs[ci].historical_events.push(HistoricalEvent {
                    year: adjusted_year,
                    event_type: EventType::Discovery,
                    description: "A great discovery was made".to_string(),
                    participants: vec![civs[ci].id],
                });
            } else if event_roll < 0.025 {
                // Plague
                civs[ci].population = (civs[ci].population as f64 * 0.7) as u64;
                civs[ci].historical_events.push(HistoricalEvent {
                    year: adjusted_year,
                    event_type: EventType::Plague,
                    description: "A terrible plague swept the land".to_string(),
                    participants: vec![civs[ci].id],
                });
            } else if event_roll < 0.03 {
                // Golden age
                civs[ci].culture_score += 0.1;
                civs[ci].historical_events.push(HistoricalEvent {
                    year: adjusted_year,
                    event_type: EventType::GoldenAge,
                    description: "A golden age of prosperity".to_string(),
                    participants: vec![civs[ci].id],
                });
            } else if event_roll < 0.032 && civs[ci].population < 100 {
                // Collapse
                civs[ci].collapse_year = Some(adjusted_year);
                civs[ci].historical_events.push(HistoricalEvent {
                    year: adjusted_year,
                    event_type: EventType::Collapse,
                    description: format!("The {} civilization collapsed", civs[ci].name),
                    participants: vec![civs[ci].id],
                });
            }
        }
    }

    civs
}

fn random_trait(rng: &mut Rng) -> CivTrait {
    match rng.range_u32(0, 8) {
        0 => CivTrait::Warlike,
        1 => CivTrait::Peaceful,
        2 => CivTrait::Mercantile,
        3 => CivTrait::Scholarly,
        4 => CivTrait::Nomadic,
        5 => CivTrait::Seafaring,
        6 => CivTrait::Religious,
        _ => CivTrait::Isolationist,
    }
}

fn random_government(rng: &mut Rng) -> GovernmentType {
    match rng.range_u32(0, 8) {
        0 => GovernmentType::Tribal,
        1 => GovernmentType::Monarchy,
        2 => GovernmentType::Republic,
        3 => GovernmentType::Theocracy,
        4 => GovernmentType::Empire,
        5 => GovernmentType::Oligarchy,
        6 => GovernmentType::Democracy,
        _ => GovernmentType::Anarchy,
    }
}

fn random_religion(rng: &mut Rng) -> ReligionType {
    match rng.range_u32(0, 8) {
        0 => ReligionType::Animism,
        1 => ReligionType::Polytheism,
        2 => ReligionType::Monotheism,
        3 => ReligionType::Philosophy,
        4 => ReligionType::Ancestor,
        5 => ReligionType::Nature,
        6 => ReligionType::Void,
        _ => ReligionType::Chaos,
    }
}

fn generate_civ_name(rng: &mut Rng) -> String {
    let roots = ["Ael", "Dor", "Val", "Khor", "Thal", "Zer", "Myr", "Nor",
        "Sar", "Eld", "Vor", "Ash", "Ith", "Orn", "Bel", "Fen"];
    let suffixes = ["heim", "gard", "land", "oria", "ium", "eth", "zan",
        "dor", "keth", "mar", "wen", "ost", "uri", "in"];
    let root = roots[rng.next_u64() as usize % roots.len()];
    let suffix = suffixes[rng.next_u64() as usize % suffixes.len()];
    format!("{}{}", root, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulate_history() {
        let settlements: Vec<Settlement> = (0..5).map(|i| Settlement {
            id: i, name: format!("Town{i}"), grid_x: i as usize * 10, grid_y: 10,
            size: super::super::settlements::SettlementSize::Village,
            population: 500, buildings: Vec::new(), roads: Vec::new(),
            biome: super::super::biomes::Biome::Grassland, near_river: false,
            near_coast: false, owner_civ: None, founded_year: -1000,
            resources: 0.5, defense: 0.3,
        }).collect();
        let biome_map = super::super::biomes::BiomeMap {
            width: 64, height: 64,
            biomes: vec![super::super::biomes::Biome::Grassland; 64 * 64],
        };
        let mut rng = Rng::new(42);
        let civs = simulate(&settlements, &biome_map, 1000, 3, &mut rng);
        assert_eq!(civs.len(), 3);
        assert!(civs.iter().all(|c| !c.name.is_empty()));
        assert!(civs.iter().any(|c| !c.historical_events.is_empty()));
    }
}
