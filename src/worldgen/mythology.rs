//! Mythology generation — narrative grammars producing creation myths,
//! hero stories, and prophecies from civilization and language data.

use super::Rng;
use super::history::{Civilization, EventType};
use super::language::Language;

/// Myth category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MythType {
    Creation, Flood, HeroJourney, Prophecy, Trickster, War,
    Origin, Apocalypse, Romance, Curse, Gift, Transformation,
}

/// A generated myth.
#[derive(Debug, Clone)]
pub struct Myth {
    pub id: u32,
    pub myth_type: MythType,
    pub title: String,
    pub narrative: String,
    pub source_civ: u32,
    pub characters: Vec<MythCharacter>,
    pub moral: String,
    pub related_events: Vec<i32>,
}

/// A character in a myth.
#[derive(Debug, Clone)]
pub struct MythCharacter {
    pub name: String,
    pub role: MythRole,
    pub epithet: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MythRole {
    Creator, Destroyer, Hero, Trickster, Monster, Lover, Sage, Martyr, King, Prophet,
}

/// Generate myths from civilizations and languages.
pub fn generate(civs: &[Civilization], languages: &[Language], rng: &mut Rng) -> Vec<Myth> {
    let mut myths = Vec::new();
    let mut next_id = 0u32;

    for civ in civs {
        let lang = languages.iter().find(|l| l.owner_civ == civ.id)
            .or_else(|| languages.first());

        // Every civilization gets a creation myth
        myths.push(generate_creation_myth(next_id, civ, lang, rng));
        next_id += 1;

        // Hero myths from major events
        for event in &civ.historical_events {
            if matches!(event.event_type, EventType::War | EventType::HeroRise | EventType::GoldenAge) {
                if rng.coin(0.4) {
                    myths.push(generate_hero_myth(next_id, civ, event, lang, rng));
                    next_id += 1;
                }
            }
        }

        // Prophecy
        if rng.coin(0.6) {
            myths.push(generate_prophecy(next_id, civ, lang, rng));
            next_id += 1;
        }
    }

    myths
}

fn generate_creation_myth(id: u32, civ: &Civilization, lang: Option<&Language>, rng: &mut Rng) -> Myth {
    let creator_name = myth_name(lang, rng);
    let world_name = myth_name(lang, rng);

    let templates = [
        format!("In the beginning, {} shaped {} from the void. From chaos came order, and from order came life.", creator_name, world_name),
        format!("{} dreamed, and the dream became {}. The mountains are {} sleeping thoughts, and the rivers are tears of joy.", creator_name, world_name, creator_name),
        format!("Two forces clashed: {} the maker and the void. Where they met, {} was born — imperfect but alive.", creator_name, world_name),
        format!("{} spoke a single word, and {} erupted from silence. Each syllable became a mountain, each pause a valley.", creator_name, world_name),
        format!("From the death of the old world, {} gathered the fragments and forged {}. We are the children of destruction reborn.", creator_name, world_name),
    ];

    let narrative = templates[rng.next_u64() as usize % templates.len()].clone();

    Myth {
        id,
        myth_type: MythType::Creation,
        title: format!("The Making of {}", world_name),
        narrative,
        source_civ: civ.id,
        characters: vec![MythCharacter {
            name: creator_name,
            role: MythRole::Creator,
            epithet: "the First".to_string(),
        }],
        moral: "From nothing, all things come.".to_string(),
        related_events: Vec::new(),
    }
}

fn generate_hero_myth(id: u32, civ: &Civilization, event: &super::history::HistoricalEvent, lang: Option<&Language>, rng: &mut Rng) -> Myth {
    let hero_name = myth_name(lang, rng);
    let villain_name = myth_name(lang, rng);

    let narrative = format!(
        "In the year of {}, {} rose from humble origins. Armed with nothing but courage, \
         {} faced the terrible {} and prevailed through {}. The people of {} remember this deed in song.",
        event.year, hero_name, hero_name, villain_name,
        if rng.coin(0.5) { "wisdom" } else { "strength" },
        civ.name
    );

    Myth {
        id,
        myth_type: MythType::HeroJourney,
        title: format!("The Saga of {}", hero_name),
        narrative,
        source_civ: civ.id,
        characters: vec![
            MythCharacter { name: hero_name, role: MythRole::Hero, epithet: "the Brave".to_string() },
            MythCharacter { name: villain_name, role: MythRole::Monster, epithet: "the Terrible".to_string() },
        ],
        moral: "Even the smallest can change the fate of the world.".to_string(),
        related_events: vec![event.year],
    }
}

fn generate_prophecy(id: u32, civ: &Civilization, lang: Option<&Language>, rng: &mut Rng) -> Myth {
    let prophet_name = myth_name(lang, rng);
    let harbinger = myth_name(lang, rng);

    let templates = [
        format!("When {} walks the earth again, the old order shall crumble and a new age shall dawn.", harbinger),
        format!("{} foretold: 'Three signs shall mark the end — a star that bleeds, a king who weeps, and a door that opens into nothing.'", prophet_name),
        format!("The scrolls of {} speak of a convergence, when all rivers flow backward and the dead remember their names.", prophet_name),
    ];

    let narrative = templates[rng.next_u64() as usize % templates.len()].clone();

    Myth {
        id,
        myth_type: MythType::Prophecy,
        title: format!("The Prophecy of {}", prophet_name),
        narrative,
        source_civ: civ.id,
        characters: vec![
            MythCharacter { name: prophet_name, role: MythRole::Prophet, epithet: "the Seer".to_string() },
        ],
        moral: "The future is written but never read clearly.".to_string(),
        related_events: Vec::new(),
    }
}

fn myth_name(lang: Option<&Language>, rng: &mut Rng) -> String {
    if let Some(lang) = lang {
        let word = lang.generate_word(rng, rng.range_usize(2, 4));
        capitalize(&word)
    } else {
        let syllables = ["Zar", "Keth", "Mor", "Ael", "Vor", "Thi", "Dra", "Nym"];
        let s1 = syllables[rng.next_u64() as usize % syllables.len()];
        let s2 = syllables[rng.next_u64() as usize % syllables.len()];
        format!("{}{}", s1, s2.to_lowercase())
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_myths() {
        let mut rng = Rng::new(42);
        let civs = vec![Civilization {
            id: 0, name: "TestCiv".to_string(), founding_year: -1000,
            collapse_year: None, capital_settlement: 0, settlements: vec![0],
            population: 10000, technology_level: 0.5, military_strength: 0.3,
            culture_score: 0.5, trade_score: 0.3,
            government: super::super::history::GovernmentType::Monarchy,
            religion: super::super::history::ReligionType::Polytheism,
            relations: Vec::new(),
            historical_events: vec![super::super::history::HistoricalEvent {
                year: -500, event_type: EventType::War,
                description: "War".to_string(), participants: vec![0],
            }],
            traits: Vec::new(),
        }];
        let langs = super::super::language::generate(1, &civs, &mut rng);
        let myths = generate(&civs, &langs, &mut rng);
        assert!(!myths.is_empty());
        assert!(myths.iter().any(|m| m.myth_type == MythType::Creation));
    }
}
