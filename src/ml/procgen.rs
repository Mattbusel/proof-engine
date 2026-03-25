//! Procedural generation via ML models with non-ML fallbacks.

use glam::Vec2;
use super::tensor::Tensor;
use super::model::{Model, Sequential};

/// Enemy formation: list of (position, enemy_type_id).
pub type Formation = Vec<(Vec2, u32)>;

/// Generated room layout.
#[derive(Debug, Clone)]
pub struct RoomLayout {
    /// 2-D grid: 0=empty, 1=wall, 2=door, 3=obstacle, 4=spawn, 5=treasure
    pub grid: Vec<Vec<u8>>,
    pub width: usize,
    pub height: usize,
}

impl RoomLayout {
    pub fn get(&self, x: usize, y: usize) -> u8 {
        if x < self.width && y < self.height { self.grid[y][x] } else { 1 }
    }
}

/// Stats for a generated item.
#[derive(Debug, Clone)]
pub struct ItemStats {
    pub name: String,
    pub damage: f32,
    pub defense: f32,
    pub speed: f32,
    pub magic: f32,
    pub rarity_score: f32,
}

/// Room type hint for generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoomType {
    Combat,
    Puzzle,
    Treasure,
    Boss,
    Corridor,
}

// ── Formation Generator ─────────────────────────────────────────────────

pub struct FormationGenerator {
    pub model: Option<Model>,
}

impl FormationGenerator {
    pub fn new() -> Self {
        Self { model: None }
    }

    pub fn with_model(model: Model) -> Self {
        Self { model: Some(model) }
    }

    /// Generate an enemy formation. `difficulty` in [0, 1], `enemy_types` is the
    /// number of distinct enemy types available.
    pub fn generate_formation(&self, difficulty: f32, enemy_types: u32) -> Formation {
        if let Some(ref model) = self.model {
            return self.generate_from_model(model, difficulty, enemy_types);
        }
        self.generate_fallback(difficulty, enemy_types)
    }

    fn generate_from_model(&self, model: &Model, difficulty: f32, enemy_types: u32) -> Formation {
        let input = Tensor::from_vec(vec![difficulty, enemy_types as f32, 0.5, 0.5], vec![1, 4]);
        let output = model.forward(&input);
        // Interpret output as pairs of (x, y, type) triples
        let mut formation = Vec::new();
        let data = &output.data;
        let mut i = 0;
        while i + 2 < data.len() {
            let x = data[i].abs() * 20.0;
            let y = data[i + 1].abs() * 20.0;
            let t = (data[i + 2].abs() * enemy_types as f32) as u32 % enemy_types.max(1);
            formation.push((Vec2::new(x, y), t));
            i += 3;
        }
        if formation.is_empty() {
            return self.generate_fallback(difficulty, enemy_types);
        }
        formation
    }

    fn generate_fallback(&self, difficulty: f32, enemy_types: u32) -> Formation {
        let count = (3.0 + difficulty * 10.0) as usize;
        let mut formation = Vec::with_capacity(count);
        let mut rng = (difficulty.to_bits() as u64).wrapping_add(1);

        for i in 0..count {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let x = (rng as u32 as f32 / u32::MAX as f32) * 16.0 + 2.0;
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let y = (rng as u32 as f32 / u32::MAX as f32) * 16.0 + 2.0;
            let enemy_type = if enemy_types > 0 { (i as u32) % enemy_types } else { 0 };
            formation.push((Vec2::new(x, y), enemy_type));
        }
        formation
    }
}

// ── Room Layout Generator ───────────────────────────────────────────────

pub struct RoomLayoutGenerator {
    pub model: Option<Model>,
}

impl RoomLayoutGenerator {
    pub fn new() -> Self {
        Self { model: None }
    }

    pub fn with_model(model: Model) -> Self {
        Self { model: Some(model) }
    }

    pub fn generate_room(&self, room_type: RoomType, width: usize, height: usize) -> RoomLayout {
        if let Some(ref model) = self.model {
            return self.generate_from_model(model, room_type, width, height);
        }
        self.generate_fallback(room_type, width, height)
    }

    fn generate_from_model(&self, model: &Model, room_type: RoomType, width: usize, height: usize) -> RoomLayout {
        let rt = match room_type {
            RoomType::Combat => 0.0,
            RoomType::Puzzle => 0.25,
            RoomType::Treasure => 0.5,
            RoomType::Boss => 0.75,
            RoomType::Corridor => 1.0,
        };
        let input = Tensor::from_vec(vec![rt, width as f32, height as f32, 0.0], vec![1, 4]);
        let output = model.forward(&input);
        // Try to interpret output as grid values
        let mut grid = vec![vec![0u8; width]; height];
        let mut idx = 0;
        for y in 0..height {
            for x in 0..width {
                if idx < output.data.len() {
                    let v = (output.data[idx].abs() * 6.0) as u8;
                    grid[y][x] = v.min(5);
                } else {
                    grid[y][x] = 0;
                }
                idx += 1;
            }
        }
        // Ensure walls on borders
        for x in 0..width {
            grid[0][x] = 1;
            grid[height - 1][x] = 1;
        }
        for y in 0..height {
            grid[y][0] = 1;
            grid[y][width - 1] = 1;
        }
        RoomLayout { grid, width, height }
    }

    fn generate_fallback(&self, room_type: RoomType, width: usize, height: usize) -> RoomLayout {
        let mut grid = vec![vec![0u8; width]; height];

        // Border walls
        for x in 0..width {
            grid[0][x] = 1;
            grid[height - 1][x] = 1;
        }
        for y in 0..height {
            grid[y][0] = 1;
            grid[y][width - 1] = 1;
        }

        // Door on left and right walls
        if height > 2 {
            grid[height / 2][0] = 2;
            grid[height / 2][width - 1] = 2;
        }

        let mut rng = (width * height + room_type as usize) as u64;
        let mut next_rng = || -> u64 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };

        match room_type {
            RoomType::Combat => {
                // Spawn points in center area
                for _ in 0..3 {
                    let x = 2 + (next_rng() as usize % (width.saturating_sub(4)).max(1));
                    let y = 2 + (next_rng() as usize % (height.saturating_sub(4)).max(1));
                    grid[y][x] = 4; // spawn
                }
                // A few obstacles
                for _ in 0..2 {
                    let x = 2 + (next_rng() as usize % (width.saturating_sub(4)).max(1));
                    let y = 2 + (next_rng() as usize % (height.saturating_sub(4)).max(1));
                    grid[y][x] = 3; // obstacle
                }
            }
            RoomType::Puzzle => {
                // Grid of obstacles
                for y in (2..height - 2).step_by(2) {
                    for x in (2..width - 2).step_by(2) {
                        grid[y][x] = 3;
                    }
                }
            }
            RoomType::Treasure => {
                // Treasure in center
                let cx = width / 2;
                let cy = height / 2;
                grid[cy][cx] = 5;
                // Surround with obstacles
                if cx > 0 { grid[cy][cx - 1] = 3; }
                if cx + 1 < width { grid[cy][cx + 1] = 3; }
                if cy > 0 { grid[cy - 1][cx] = 3; }
                if cy + 1 < height { grid[cy + 1][cx] = 3; }
            }
            RoomType::Boss => {
                // Large open area with spawn in center
                let cx = width / 2;
                let cy = height / 2;
                grid[cy][cx] = 4;
                // Pillars in corners
                if width > 4 && height > 4 {
                    grid[2][2] = 3;
                    grid[2][width - 3] = 3;
                    grid[height - 3][2] = 3;
                    grid[height - 3][width - 3] = 3;
                }
            }
            RoomType::Corridor => {
                // Fill top and bottom halves with walls, leave center row open
                for y in 2..height / 2 {
                    for x in 1..width - 1 {
                        grid[y][x] = 1;
                    }
                }
                for y in height / 2 + 2..height - 1 {
                    for x in 1..width - 1 {
                        grid[y][x] = 1;
                    }
                }
            }
        }

        RoomLayout { grid, width, height }
    }
}

// ── Item Stat Generator ─────────────────────────────────────────────────

pub struct ItemStatGenerator {
    pub model: Option<Model>,
}

impl ItemStatGenerator {
    pub fn new() -> Self {
        Self { model: None }
    }

    /// Generate item stats from a rarity level [0,1] and a type hint string.
    pub fn generate_item(&self, rarity: f32, type_hint: &str) -> ItemStats {
        let type_val = match type_hint {
            "sword" => 0.0,
            "shield" => 0.2,
            "staff" => 0.4,
            "bow" => 0.6,
            "armor" => 0.8,
            _ => 0.5,
        };

        if let Some(ref model) = self.model {
            let input = Tensor::from_vec(vec![rarity, type_val, 0.5, 0.5], vec![1, 4]);
            let out = model.forward(&input);
            let d = &out.data;
            return ItemStats {
                name: format!("{}_r{}", type_hint, (rarity * 100.0) as u32),
                damage: d.first().copied().unwrap_or(0.0).abs() * 100.0 * rarity,
                defense: d.get(1).copied().unwrap_or(0.0).abs() * 80.0 * rarity,
                speed: d.get(2).copied().unwrap_or(0.5).abs() * 50.0,
                magic: d.get(3).copied().unwrap_or(0.0).abs() * 60.0 * rarity,
                rarity_score: rarity,
            };
        }

        // Fallback: heuristic generation
        let base_mult = 1.0 + rarity * 4.0;
        let (dmg, def, spd, mag) = match type_hint {
            "sword" => (20.0 * base_mult, 5.0, 10.0 * base_mult, 0.0),
            "shield" => (0.0, 30.0 * base_mult, 5.0, 5.0),
            "staff" => (5.0, 5.0, 8.0, 25.0 * base_mult),
            "bow" => (15.0 * base_mult, 0.0, 15.0 * base_mult, 0.0),
            "armor" => (0.0, 25.0 * base_mult, 3.0, 10.0),
            _ => (10.0 * base_mult, 10.0 * base_mult, 10.0, 10.0),
        };

        let prefix = match rarity {
            r if r > 0.9 => "Legendary",
            r if r > 0.7 => "Epic",
            r if r > 0.4 => "Rare",
            r if r > 0.2 => "Uncommon",
            _ => "Common",
        };

        ItemStats {
            name: format!("{} {}", prefix, type_hint),
            damage: dmg,
            defense: def,
            speed: spd,
            magic: mag,
            rarity_score: rarity,
        }
    }
}

// ── Name Generator ──────────────────────────────────────────────────────

/// Character-level RNN name generator. Falls back to Markov chain heuristic.
pub struct NameGenerator {
    pub model: Option<Model>,
    /// Markov chain: for each pair of chars, probability of next char.
    pub bigram_table: Vec<Vec<(char, f32)>>,
}

impl NameGenerator {
    pub fn new() -> Self {
        // Build a simple bigram table from common fantasy name patterns
        let seed_names = [
            "aerin", "balor", "celia", "darin", "elena", "feron", "galia",
            "heron", "irene", "jorah", "kiera", "loric", "miren", "norin",
            "orion", "perin", "quill", "rohan", "siren", "torin", "uriel",
            "valen", "wren", "xyla", "yoren", "zara",
        ];

        let mut bigrams: std::collections::HashMap<char, std::collections::HashMap<char, u32>> =
            std::collections::HashMap::new();

        for name in &seed_names {
            let chars: Vec<char> = name.chars().collect();
            for w in chars.windows(2) {
                *bigrams.entry(w[0]).or_default().entry(w[1]).or_insert(0) += 1;
            }
        }

        let mut bigram_table = Vec::new();
        let mut all_chars: Vec<char> = bigrams.keys().cloned().collect();
        all_chars.sort();
        for &ch in &all_chars {
            if let Some(nexts) = bigrams.get(&ch) {
                let total: u32 = nexts.values().sum();
                let probs: Vec<(char, f32)> = nexts.iter()
                    .map(|(&c, &count)| (c, count as f32 / total as f32))
                    .collect();
                bigram_table.push(probs);
            } else {
                bigram_table.push(vec![]);
            }
        }

        Self { model: None, bigram_table }
    }

    /// Generate a name given a seed and max length.
    pub fn generate_name(&self, seed: u64, max_len: usize) -> String {
        let vowels = ['a', 'e', 'i', 'o', 'u'];
        let consonants = ['b', 'c', 'd', 'f', 'g', 'h', 'k', 'l', 'm', 'n', 'p', 'r', 's', 't', 'v', 'w', 'z'];

        let mut rng = seed.wrapping_add(1);
        let mut next = || -> u64 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };

        let max_len = max_len.max(3).min(20);
        let name_len = 3 + (next() as usize % (max_len - 2));

        let mut name = String::with_capacity(name_len);
        let mut use_vowel = next() % 2 == 0;

        for i in 0..name_len {
            let ch = if use_vowel {
                vowels[next() as usize % vowels.len()]
            } else {
                consonants[next() as usize % consonants.len()]
            };
            if i == 0 {
                name.push(ch.to_uppercase().next().unwrap());
            } else {
                name.push(ch);
            }
            // Alternate consonant/vowel with occasional doubles
            if next() % 5 != 0 {
                use_vowel = !use_vowel;
            }
        }

        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formation_fallback() {
        let gen = FormationGenerator::new();
        let formation = gen.generate_formation(0.5, 3);
        assert!(!formation.is_empty());
        for &(pos, etype) in &formation {
            assert!(pos.x >= 0.0 && pos.y >= 0.0);
            assert!(etype < 3);
        }
    }

    #[test]
    fn test_formation_difficulty_scales() {
        let gen = FormationGenerator::new();
        let easy = gen.generate_formation(0.0, 2);
        let hard = gen.generate_formation(1.0, 2);
        assert!(hard.len() > easy.len());
    }

    #[test]
    fn test_room_layout_combat() {
        let gen = RoomLayoutGenerator::new();
        let room = gen.generate_room(RoomType::Combat, 10, 10);
        assert_eq!(room.width, 10);
        assert_eq!(room.height, 10);
        // Borders should be walls (1) or doors (2)
        for x in 0..10 {
            assert!(room.get(x, 0) == 1 || room.get(x, 0) == 2);
        }
        // Should contain at least one spawn point
        let has_spawn = room.grid.iter().flatten().any(|&v| v == 4);
        assert!(has_spawn);
    }

    #[test]
    fn test_room_layout_treasure() {
        let gen = RoomLayoutGenerator::new();
        let room = gen.generate_room(RoomType::Treasure, 8, 8);
        let has_treasure = room.grid.iter().flatten().any(|&v| v == 5);
        assert!(has_treasure);
    }

    #[test]
    fn test_room_layout_all_types() {
        let gen = RoomLayoutGenerator::new();
        for rt in &[RoomType::Combat, RoomType::Puzzle, RoomType::Treasure, RoomType::Boss, RoomType::Corridor] {
            let room = gen.generate_room(*rt, 12, 12);
            assert_eq!(room.width, 12);
            assert_eq!(room.height, 12);
        }
    }

    #[test]
    fn test_item_stat_generator() {
        let gen = ItemStatGenerator::new();
        let sword = gen.generate_item(0.5, "sword");
        assert!(sword.damage > 0.0);
        assert!(sword.name.contains("sword") || sword.name.contains("Sword"));

        let shield = gen.generate_item(0.8, "shield");
        assert!(shield.defense > 0.0);
    }

    #[test]
    fn test_item_rarity_scaling() {
        let gen = ItemStatGenerator::new();
        let common = gen.generate_item(0.1, "sword");
        let legendary = gen.generate_item(0.95, "sword");
        assert!(legendary.damage > common.damage);
        assert!(legendary.name.contains("Legendary"));
        assert!(common.name.contains("Common"));
    }

    #[test]
    fn test_name_generator() {
        let gen = NameGenerator::new();
        let name1 = gen.generate_name(42, 8);
        let name2 = gen.generate_name(99, 8);
        assert!(name1.len() >= 3);
        assert!(name2.len() >= 3);
        assert_ne!(name1, name2);
        // First character should be uppercase
        assert!(name1.chars().next().unwrap().is_uppercase());
    }

    #[test]
    fn test_name_generator_different_seeds() {
        let gen = NameGenerator::new();
        let names: Vec<String> = (0..10).map(|s| gen.generate_name(s, 6)).collect();
        // At least some should be unique
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert!(unique.len() > 3);
    }
}
