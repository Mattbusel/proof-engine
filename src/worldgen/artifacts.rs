//! Artifact generation — items with histories tied to cultures and events.

use super::Rng;
use super::history::Civilization;
use super::mythology::Myth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactType { Weapon, Armor, Jewelry, Tome, Relic, Instrument, Tool, Crown, Staff, Amulet }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactRarity { Common, Uncommon, Rare, Legendary, Mythic }

#[derive(Debug, Clone)]
pub struct Artifact {
    pub id: u32,
    pub name: String,
    pub artifact_type: ArtifactType,
    pub rarity: ArtifactRarity,
    pub description: String,
    pub origin_civ: u32,
    pub creation_year: i32,
    pub creator_name: String,
    pub related_myth: Option<u32>,
    pub power_level: f32,
    pub properties: Vec<String>,
}

pub fn generate(civs: &[Civilization], myths: &[Myth], rng: &mut Rng) -> Vec<Artifact> {
    let mut artifacts = Vec::new();
    let mut next_id = 0u32;

    for civ in civs {
        let num = (civ.culture_score * 5.0) as usize + 1;
        for _ in 0..num {
            let atype = random_type(rng);
            let rarity = if rng.coin(0.05) { ArtifactRarity::Mythic }
                else if rng.coin(0.1) { ArtifactRarity::Legendary }
                else if rng.coin(0.2) { ArtifactRarity::Rare }
                else if rng.coin(0.3) { ArtifactRarity::Uncommon }
                else { ArtifactRarity::Common };

            let related_myth = myths.iter()
                .filter(|m| m.source_civ == civ.id)
                .next()
                .map(|m| m.id);

            let (name, desc, props) = generate_artifact_details(atype, rarity, &civ.name, rng);

            artifacts.push(Artifact {
                id: next_id,
                name, artifact_type: atype, rarity, description: desc,
                origin_civ: civ.id,
                creation_year: civ.founding_year + rng.range_u32(0, 1000) as i32,
                creator_name: format!("{}smith", &civ.name[..civ.name.len().min(4)]),
                related_myth,
                power_level: rarity_power(rarity) * rng.range_f32(0.8, 1.2),
                properties: props,
            });
            next_id += 1;
        }
    }
    artifacts
}

fn random_type(rng: &mut Rng) -> ArtifactType {
    match rng.range_u32(0, 10) {
        0 => ArtifactType::Weapon, 1 => ArtifactType::Armor, 2 => ArtifactType::Jewelry,
        3 => ArtifactType::Tome, 4 => ArtifactType::Relic, 5 => ArtifactType::Instrument,
        6 => ArtifactType::Tool, 7 => ArtifactType::Crown, 8 => ArtifactType::Staff,
        _ => ArtifactType::Amulet,
    }
}

fn rarity_power(r: ArtifactRarity) -> f32 {
    match r {
        ArtifactRarity::Common => 1.0, ArtifactRarity::Uncommon => 2.0,
        ArtifactRarity::Rare => 4.0, ArtifactRarity::Legendary => 8.0, ArtifactRarity::Mythic => 16.0,
    }
}

fn generate_artifact_details(atype: ArtifactType, rarity: ArtifactRarity, civ_name: &str, rng: &mut Rng) -> (String, String, Vec<String>) {
    let prefixes = ["Shadow", "Storm", "Void", "Star", "Moon", "Sun", "Blood", "Soul", "Dream", "Fate"];
    let prefix = prefixes[rng.next_u64() as usize % prefixes.len()];

    let type_name = match atype {
        ArtifactType::Weapon => "Blade", ArtifactType::Armor => "Shield",
        ArtifactType::Jewelry => "Ring", ArtifactType::Tome => "Codex",
        ArtifactType::Relic => "Fragment", ArtifactType::Instrument => "Horn",
        ArtifactType::Tool => "Compass", ArtifactType::Crown => "Crown",
        ArtifactType::Staff => "Staff", ArtifactType::Amulet => "Amulet",
    };

    let name = format!("{}{} of {}", prefix, type_name, civ_name);
    let desc = format!("A {} {} forged in the {} tradition.", rarity_adj(rarity), type_name.to_lowercase(), civ_name);
    let props = match rarity {
        ArtifactRarity::Mythic => vec!["Reality-warping".to_string(), "Sentient".to_string(), "Indestructible".to_string()],
        ArtifactRarity::Legendary => vec!["Elemental mastery".to_string(), "Soul-bound".to_string()],
        ArtifactRarity::Rare => vec!["Enhanced".to_string(), "Self-repairing".to_string()],
        ArtifactRarity::Uncommon => vec!["Durable".to_string()],
        ArtifactRarity::Common => vec![],
    };
    (name, desc, props)
}

fn rarity_adj(r: ArtifactRarity) -> &'static str {
    match r {
        ArtifactRarity::Common => "simple", ArtifactRarity::Uncommon => "finely crafted",
        ArtifactRarity::Rare => "exquisite", ArtifactRarity::Legendary => "legendary",
        ArtifactRarity::Mythic => "mythic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_generate_artifacts() {
        let mut rng = Rng::new(42);
        let civs = vec![Civilization {
            id: 0, name: "TestCiv".to_string(), founding_year: -1000, collapse_year: None,
            capital_settlement: 0, settlements: vec![0], population: 10000,
            technology_level: 0.5, military_strength: 0.3, culture_score: 0.6,
            trade_score: 0.3, government: super::super::history::GovernmentType::Monarchy,
            religion: super::super::history::ReligionType::Polytheism,
            relations: Vec::new(), historical_events: Vec::new(), traits: Vec::new(),
        }];
        let artifacts = generate(&civs, &[], &mut rng);
        assert!(!artifacts.is_empty());
    }
}
