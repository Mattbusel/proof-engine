//! Quest generation — objective chains from world state and motivations.

use crate::worldgen::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestType { Fetch, Kill, Escort, Explore, Deliver, Defend, Investigate, Craft, Diplomacy, Rescue }

#[derive(Debug, Clone)]
pub struct QuestObjective { pub description: String, pub completed: bool, pub optional: bool }

#[derive(Debug, Clone)]
pub struct GeneratedQuest {
    pub title: String,
    pub quest_type: QuestType,
    pub giver: String,
    pub description: String,
    pub objectives: Vec<QuestObjective>,
    pub reward_description: String,
    pub difficulty: f32,
    pub moral_complexity: f32,
}

/// Generate a quest from world context.
pub fn generate_quest(giver_name: &str, region_name: &str, threat_level: f32, rng: &mut Rng) -> GeneratedQuest {
    let quest_type = random_quest_type(rng);
    let (title, desc, objectives) = match quest_type {
        QuestType::Fetch => {
            let item = random_item(rng);
            (format!("The Lost {}", item),
             format!("{} asks you to find a {} lost in the wilds of {}.", giver_name, item, region_name),
             vec![
                 QuestObjective { description: format!("Find the {} in {}", item, region_name), completed: false, optional: false },
                 QuestObjective { description: format!("Return the {} to {}", item, giver_name), completed: false, optional: false },
             ])
        }
        QuestType::Kill => {
            let monster = random_monster(rng);
            (format!("Bane of the {}", monster),
             format!("A {} terrorizes {}. {} begs for your aid.", monster, region_name, giver_name),
             vec![
                 QuestObjective { description: format!("Track the {} in {}", monster, region_name), completed: false, optional: false },
                 QuestObjective { description: format!("Defeat the {}", monster), completed: false, optional: false },
                 QuestObjective { description: "Collect proof of the deed".to_string(), completed: false, optional: true },
             ])
        }
        QuestType::Investigate => {
            (format!("Whispers in {}", region_name),
             format!("Strange occurrences plague {}. {} wants answers.", region_name, giver_name),
             vec![
                 QuestObjective { description: "Gather clues from locals".to_string(), completed: false, optional: false },
                 QuestObjective { description: "Investigate the source".to_string(), completed: false, optional: false },
                 QuestObjective { description: format!("Report findings to {}", giver_name), completed: false, optional: false },
             ])
        }
        _ => {
            (format!("Task for {}", giver_name),
             format!("{} needs your help in {}.", giver_name, region_name),
             vec![QuestObjective { description: "Complete the task".to_string(), completed: false, optional: false }])
        }
    };

    GeneratedQuest {
        title, quest_type, giver: giver_name.to_string(), description: desc, objectives,
        reward_description: random_reward(rng),
        difficulty: threat_level * rng.range_f32(0.5, 1.5),
        moral_complexity: rng.next_f32(),
    }
}

fn random_quest_type(rng: &mut Rng) -> QuestType {
    match rng.range_u32(0, 10) {
        0..=2 => QuestType::Fetch, 3..=4 => QuestType::Kill, 5 => QuestType::Escort,
        6 => QuestType::Explore, 7 => QuestType::Investigate, 8 => QuestType::Defend,
        _ => QuestType::Deliver,
    }
}

fn random_item(rng: &mut Rng) -> &'static str {
    let items = ["Amulet", "Tome", "Crystal", "Crown", "Blade", "Chalice", "Map", "Key", "Orb", "Scroll"];
    items[rng.next_u64() as usize % items.len()]
}

fn random_monster(rng: &mut Rng) -> &'static str {
    let monsters = ["Wyvern", "Troll", "Banshee", "Golem", "Hydra", "Wraith", "Basilisk", "Chimera"];
    monsters[rng.next_u64() as usize % monsters.len()]
}

fn random_reward(rng: &mut Rng) -> String {
    let rewards = ["Gold and the gratitude of the people", "A rare enchanted item", "Knowledge of ancient secrets",
        "Political favor and influence", "Access to restricted areas", "A powerful ally"];
    rewards[rng.next_u64() as usize % rewards.len()].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_quest() {
        let mut rng = Rng::new(42);
        let q = generate_quest("Elder Vorn", "Ashwood", 0.5, &mut rng);
        assert!(!q.title.is_empty());
        assert!(!q.objectives.is_empty());
    }
}
