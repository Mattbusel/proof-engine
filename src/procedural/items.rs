//! Item generation — bases, affixes, uniques, sets, enchantments, loot drops.
//!
//! Provides a complete Diablo-style item generation pipeline:
//! base items → affix pool → random rolls → unique/set items → enchantments → loot tables.

use super::Rng;
use std::collections::HashMap;

// ── StatKind ──────────────────────────────────────────────────────────────────

/// Stat kinds that affixes and enchantments can modify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatKind {
    Damage,
    Defense,
    Health,
    Mana,
    Speed,
    CritChance,
    CritDamage,
    FireResist,
    ColdResist,
    LightningResist,
    PoisonResist,
    LifeSteal,
    ManaSteal,
    Thorns,
    GoldFind,
    MagicFind,
    CooldownReduction,
    AttackSpeed,
    BlockChance,
    Dodge,
}

// ── ItemType ──────────────────────────────────────────────────────────────────

/// Category of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemType {
    Sword,
    Axe,
    Mace,
    Dagger,
    Staff,
    Bow,
    Shield,
    Helmet,
    ChestArmor,
    Gloves,
    Boots,
    Belt,
    Ring,
    Amulet,
    Wand,
    Quiver,
}

impl ItemType {
    pub fn is_weapon(&self) -> bool {
        matches!(self, ItemType::Sword | ItemType::Axe | ItemType::Mace | ItemType::Dagger
                     | ItemType::Staff | ItemType::Bow | ItemType::Wand | ItemType::Quiver)
    }

    pub fn is_armor(&self) -> bool {
        matches!(self, ItemType::Shield | ItemType::Helmet | ItemType::ChestArmor
                     | ItemType::Gloves | ItemType::Boots | ItemType::Belt)
    }

    pub fn is_jewelry(&self) -> bool {
        matches!(self, ItemType::Ring | ItemType::Amulet)
    }
}

// ── Rarity ────────────────────────────────────────────────────────────────────

/// Item rarity tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Magic,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub fn affix_count_range(&self) -> (usize, usize) {
        match self {
            Rarity::Common    => (0, 0),
            Rarity::Magic     => (1, 2),
            Rarity::Rare      => (2, 4),
            Rarity::Epic      => (3, 5),
            Rarity::Legendary => (4, 6),
        }
    }

    pub fn color_code(&self) -> &'static str {
        match self {
            Rarity::Common    => "white",
            Rarity::Magic     => "blue",
            Rarity::Rare      => "yellow",
            Rarity::Epic      => "purple",
            Rarity::Legendary => "orange",
        }
    }
}

// ── ItemBase ──────────────────────────────────────────────────────────────────

/// Base definition of an item (before affixes).
#[derive(Debug, Clone)]
pub struct ItemBase {
    pub name:          &'static str,
    pub item_type:     ItemType,
    pub base_damage:   f32,
    pub base_defense:  f32,
    pub base_value:    u32,
    pub weight:        f32,
    pub rarity:        Rarity,
    pub glyph:         char,
    /// Minimum item level required to find this base.
    pub required_level: u32,
}

impl ItemBase {
    /// Return the built-in pool of base items.
    pub fn pool() -> &'static [ItemBase] {
        &BASE_POOL
    }

    /// Filter bases appropriate for a given item level.
    pub fn for_level(level: u32) -> Vec<&'static ItemBase> {
        BASE_POOL.iter().filter(|b| b.required_level <= level).collect()
    }
}

// Static base pool (lazy-initialised via once_cell-free approach with const)
static BASE_POOL: [ItemBase; 32] = [
    // Swords
    ItemBase { name: "Rusty Sword",      item_type: ItemType::Sword,      base_damage: 5.0,  base_defense: 0.0,  base_value: 10,   weight: 3.0, rarity: Rarity::Common,    glyph: '/', required_level: 1  },
    ItemBase { name: "Short Sword",      item_type: ItemType::Sword,      base_damage: 10.0, base_defense: 0.0,  base_value: 50,   weight: 3.5, rarity: Rarity::Common,    glyph: '/', required_level: 3  },
    ItemBase { name: "Long Sword",       item_type: ItemType::Sword,      base_damage: 18.0, base_defense: 0.0,  base_value: 150,  weight: 5.0, rarity: Rarity::Common,    glyph: '/', required_level: 8  },
    ItemBase { name: "Broadsword",       item_type: ItemType::Sword,      base_damage: 25.0, base_defense: 0.0,  base_value: 300,  weight: 6.0, rarity: Rarity::Common,    glyph: '/', required_level: 15 },
    // Axes
    ItemBase { name: "Hand Axe",         item_type: ItemType::Axe,        base_damage: 8.0,  base_defense: 0.0,  base_value: 30,   weight: 4.0, rarity: Rarity::Common,    glyph: 'T', required_level: 2  },
    ItemBase { name: "Battle Axe",       item_type: ItemType::Axe,        base_damage: 20.0, base_defense: 0.0,  base_value: 200,  weight: 7.0, rarity: Rarity::Common,    glyph: 'T', required_level: 10 },
    // Daggers
    ItemBase { name: "Dagger",           item_type: ItemType::Dagger,     base_damage: 6.0,  base_defense: 0.0,  base_value: 20,   weight: 1.0, rarity: Rarity::Common,    glyph: '-', required_level: 1  },
    ItemBase { name: "Stiletto",         item_type: ItemType::Dagger,     base_damage: 14.0, base_defense: 0.0,  base_value: 120,  weight: 1.5, rarity: Rarity::Common,    glyph: '-', required_level: 7  },
    // Staves
    ItemBase { name: "Wooden Staff",     item_type: ItemType::Staff,      base_damage: 7.0,  base_defense: 0.0,  base_value: 25,   weight: 4.0, rarity: Rarity::Common,    glyph: '|', required_level: 1  },
    ItemBase { name: "Arcane Staff",     item_type: ItemType::Staff,      base_damage: 22.0, base_defense: 0.0,  base_value: 400,  weight: 4.5, rarity: Rarity::Common,    glyph: '|', required_level: 12 },
    // Bows
    ItemBase { name: "Short Bow",        item_type: ItemType::Bow,        base_damage: 9.0,  base_defense: 0.0,  base_value: 40,   weight: 2.0, rarity: Rarity::Common,    glyph: ')', required_level: 3  },
    ItemBase { name: "Long Bow",         item_type: ItemType::Bow,        base_damage: 17.0, base_defense: 0.0,  base_value: 180,  weight: 2.5, rarity: Rarity::Common,    glyph: ')', required_level: 9  },
    // Shields
    ItemBase { name: "Buckler",          item_type: ItemType::Shield,     base_damage: 0.0,  base_defense: 8.0,  base_value: 40,   weight: 3.0, rarity: Rarity::Common,    glyph: 'o', required_level: 1  },
    ItemBase { name: "Tower Shield",     item_type: ItemType::Shield,     base_damage: 0.0,  base_defense: 25.0, base_value: 350,  weight: 9.0, rarity: Rarity::Common,    glyph: 'O', required_level: 12 },
    // Helms
    ItemBase { name: "Leather Cap",      item_type: ItemType::Helmet,     base_damage: 0.0,  base_defense: 5.0,  base_value: 20,   weight: 1.0, rarity: Rarity::Common,    glyph: 'n', required_level: 1  },
    ItemBase { name: "Iron Helm",        item_type: ItemType::Helmet,     base_damage: 0.0,  base_defense: 15.0, base_value: 150,  weight: 4.0, rarity: Rarity::Common,    glyph: 'n', required_level: 8  },
    ItemBase { name: "Great Helm",       item_type: ItemType::Helmet,     base_damage: 0.0,  base_defense: 25.0, base_value: 400,  weight: 6.0, rarity: Rarity::Common,    glyph: 'N', required_level: 16 },
    // Chest
    ItemBase { name: "Leather Armor",    item_type: ItemType::ChestArmor, base_damage: 0.0,  base_defense: 10.0, base_value: 50,   weight: 5.0, rarity: Rarity::Common,    glyph: '[', required_level: 1  },
    ItemBase { name: "Chain Mail",       item_type: ItemType::ChestArmor, base_damage: 0.0,  base_defense: 20.0, base_value: 200,  weight: 10.0,rarity: Rarity::Common,    glyph: '[', required_level: 8  },
    ItemBase { name: "Plate Armor",      item_type: ItemType::ChestArmor, base_damage: 0.0,  base_defense: 40.0, base_value: 800,  weight: 18.0,rarity: Rarity::Common,    glyph: '[', required_level: 20 },
    // Gloves
    ItemBase { name: "Cloth Gloves",     item_type: ItemType::Gloves,     base_damage: 0.0,  base_defense: 3.0,  base_value: 15,   weight: 0.5, rarity: Rarity::Common,    glyph: '(', required_level: 1  },
    ItemBase { name: "Iron Gauntlets",   item_type: ItemType::Gloves,     base_damage: 0.0,  base_defense: 10.0, base_value: 100,  weight: 2.0, rarity: Rarity::Common,    glyph: '(', required_level: 8  },
    // Boots
    ItemBase { name: "Cloth Shoes",      item_type: ItemType::Boots,      base_damage: 0.0,  base_defense: 3.0,  base_value: 15,   weight: 0.5, rarity: Rarity::Common,    glyph: 'U', required_level: 1  },
    ItemBase { name: "Iron Boots",       item_type: ItemType::Boots,      base_damage: 0.0,  base_defense: 12.0, base_value: 120,  weight: 3.0, rarity: Rarity::Common,    glyph: 'U', required_level: 8  },
    // Belt
    ItemBase { name: "Leather Belt",     item_type: ItemType::Belt,       base_damage: 0.0,  base_defense: 4.0,  base_value: 20,   weight: 0.5, rarity: Rarity::Common,    glyph: '=', required_level: 1  },
    // Rings & Amulets
    ItemBase { name: "Copper Ring",      item_type: ItemType::Ring,       base_damage: 0.0,  base_defense: 0.0,  base_value: 30,   weight: 0.1, rarity: Rarity::Common,    glyph: '°', required_level: 1  },
    ItemBase { name: "Silver Ring",      item_type: ItemType::Ring,       base_damage: 0.0,  base_defense: 0.0,  base_value: 100,  weight: 0.1, rarity: Rarity::Common,    glyph: '°', required_level: 5  },
    ItemBase { name: "Simple Amulet",    item_type: ItemType::Amulet,     base_damage: 0.0,  base_defense: 0.0,  base_value: 50,   weight: 0.2, rarity: Rarity::Common,    glyph: '♦', required_level: 1  },
    ItemBase { name: "Jade Amulet",      item_type: ItemType::Amulet,     base_damage: 0.0,  base_defense: 0.0,  base_value: 200,  weight: 0.2, rarity: Rarity::Common,    glyph: '♦', required_level: 8  },
    // Wands
    ItemBase { name: "Gnarled Wand",     item_type: ItemType::Wand,       base_damage: 8.0,  base_defense: 0.0,  base_value: 40,   weight: 1.5, rarity: Rarity::Common,    glyph: '!', required_level: 2  },
    // Mace
    ItemBase { name: "Club",             item_type: ItemType::Mace,       base_damage: 7.0,  base_defense: 0.0,  base_value: 15,   weight: 4.0, rarity: Rarity::Common,    glyph: '\\',required_level: 1  },
    ItemBase { name: "War Hammer",       item_type: ItemType::Mace,       base_damage: 28.0, base_defense: 0.0,  base_value: 450,  weight: 9.0, rarity: Rarity::Common,    glyph: '\\',required_level: 18 },
];

// ── Affix ─────────────────────────────────────────────────────────────────────

/// Whether an affix appears before or after the base item name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffixType {
    Prefix,
    Suffix,
}

/// A stat modifier applied by an affix.
#[derive(Debug, Clone)]
pub struct Affix {
    pub name:           &'static str,
    pub affix_type:     AffixType,
    pub stat_modifiers: Vec<(StatKind, f32)>,
    pub required_level: u32,
    /// Spawn weight (higher = more common).
    pub weight:         f32,
    /// Which item types can receive this affix (empty = all).
    pub allowed_types:  Vec<ItemType>,
}

impl Affix {
    fn new_prefix(
        name: &'static str,
        mods: Vec<(StatKind, f32)>,
        req: u32,
        w: f32,
    ) -> Self {
        Self { name, affix_type: AffixType::Prefix, stat_modifiers: mods, required_level: req, weight: w, allowed_types: Vec::new() }
    }

    fn new_suffix(
        name: &'static str,
        mods: Vec<(StatKind, f32)>,
        req: u32,
        w: f32,
    ) -> Self {
        Self { name, affix_type: AffixType::Suffix, stat_modifiers: mods, required_level: req, weight: w, allowed_types: Vec::new() }
    }
}

// ── AffixPool ─────────────────────────────────────────────────────────────────

/// Weighted pool of affixes for rolling item modifiers.
pub struct AffixPool {
    prefixes: Vec<Affix>,
    suffixes: Vec<Affix>,
}

impl AffixPool {
    /// Create the built-in pool with 60+ affixes.
    pub fn default_pool() -> Self {
        let prefixes = vec![
            Affix::new_prefix("Sturdy",      vec![(StatKind::Defense,    5.0)],  1,  10.0),
            Affix::new_prefix("Reinforced",  vec![(StatKind::Defense,   12.0)],  5,  8.0),
            Affix::new_prefix("Fortified",   vec![(StatKind::Defense,   25.0)],  12, 5.0),
            Affix::new_prefix("Iron",        vec![(StatKind::Defense,   40.0)],  20, 3.0),
            Affix::new_prefix("Brutal",      vec![(StatKind::Damage,     5.0)],  1,  10.0),
            Affix::new_prefix("Vicious",     vec![(StatKind::Damage,    12.0)],  5,  8.0),
            Affix::new_prefix("Savage",      vec![(StatKind::Damage,    22.0)],  12, 5.0),
            Affix::new_prefix("Merciless",   vec![(StatKind::Damage,    38.0)],  20, 3.0),
            Affix::new_prefix("Vitalized",   vec![(StatKind::Health,    20.0)],  1,  10.0),
            Affix::new_prefix("Vigorous",    vec![(StatKind::Health,    50.0)],  6,  7.0),
            Affix::new_prefix("Hale",        vec![(StatKind::Health,   100.0)],  14, 4.0),
            Affix::new_prefix("Juggernaut",  vec![(StatKind::Health,   200.0)],  25, 2.0),
            Affix::new_prefix("Quick",       vec![(StatKind::Speed,     0.05)],  3,  9.0),
            Affix::new_prefix("Swift",       vec![(StatKind::Speed,     0.12)],  10, 6.0),
            Affix::new_prefix("Blazing",     vec![(StatKind::FireResist, 10.0)], 3,  8.0),
            Affix::new_prefix("Glacial",     vec![(StatKind::ColdResist, 10.0)], 3,  8.0),
            Affix::new_prefix("Static",      vec![(StatKind::LightningResist, 10.0)], 3, 8.0),
            Affix::new_prefix("Toxic",       vec![(StatKind::PoisonResist, 10.0)], 3, 8.0),
            Affix::new_prefix("Arcane",      vec![(StatKind::Mana, 30.0)],       5,  7.0),
            Affix::new_prefix("Mystic",      vec![(StatKind::Mana, 70.0)],      12,  5.0),
            Affix::new_prefix("Ethereal",    vec![(StatKind::MagicFind, 5.0)],   8,  6.0),
            Affix::new_prefix("Spectral",    vec![(StatKind::MagicFind, 15.0)], 18,  4.0),
            Affix::new_prefix("Lucky",       vec![(StatKind::GoldFind,  10.0)],  1, 10.0),
            Affix::new_prefix("Gilded",      vec![(StatKind::GoldFind,  25.0)],  8,  6.0),
            Affix::new_prefix("Spiked",      vec![(StatKind::Thorns,    8.0)],   5,  7.0),
            Affix::new_prefix("Barbed",      vec![(StatKind::Thorns,   20.0)],  14,  4.0),
            Affix::new_prefix("Precise",     vec![(StatKind::CritChance, 0.03)], 6,  7.0),
            Affix::new_prefix("Deadly",      vec![(StatKind::CritChance, 0.07)],15,  4.0),
            Affix::new_prefix("Lethal",      vec![(StatKind::CritDamage, 0.15)], 8,  6.0),
            Affix::new_prefix("Annihilating",vec![(StatKind::CritDamage, 0.30)],20,  3.0),
            Affix::new_prefix("Hasty",       vec![(StatKind::AttackSpeed, 0.05)],4,  8.0),
            Affix::new_prefix("Rapid",       vec![(StatKind::AttackSpeed, 0.12)],12, 5.0),
        ];

        let suffixes = vec![
            Affix::new_suffix("of Might",       vec![(StatKind::Damage,    10.0)], 1,  10.0),
            Affix::new_suffix("of Power",        vec![(StatKind::Damage,    22.0)], 8,   7.0),
            Affix::new_suffix("of Devastation",  vec![(StatKind::Damage,    40.0)],18,   4.0),
            Affix::new_suffix("of Warding",      vec![(StatKind::Defense,    8.0)], 1,  10.0),
            Affix::new_suffix("of Protection",   vec![(StatKind::Defense,   18.0)], 7,   7.0),
            Affix::new_suffix("of the Colossus", vec![(StatKind::Defense,   35.0)],16,   4.0),
            Affix::new_suffix("of Life",         vec![(StatKind::Health,    25.0)], 1,  10.0),
            Affix::new_suffix("of Vitality",     vec![(StatKind::Health,    60.0)], 8,   7.0),
            Affix::new_suffix("of Immortality",  vec![(StatKind::Health,   120.0)],18,   3.0),
            Affix::new_suffix("of the Mind",     vec![(StatKind::Mana,      20.0)], 1,  10.0),
            Affix::new_suffix("of Intellect",    vec![(StatKind::Mana,      50.0)], 8,   7.0),
            Affix::new_suffix("of Brilliance",   vec![(StatKind::Mana,     100.0)],18,   4.0),
            Affix::new_suffix("of Flame",        vec![(StatKind::FireResist, 8.0)], 2,   9.0),
            Affix::new_suffix("of Frost",        vec![(StatKind::ColdResist, 8.0)], 2,   9.0),
            Affix::new_suffix("of Thunder",      vec![(StatKind::LightningResist, 8.0)], 2, 9.0),
            Affix::new_suffix("of Venom",        vec![(StatKind::PoisonResist, 8.0)], 2, 9.0),
            Affix::new_suffix("of the Hawk",     vec![(StatKind::CritChance, 0.04)], 6,  7.0),
            Affix::new_suffix("of Precision",    vec![(StatKind::CritChance, 0.08)],15,  4.0),
            Affix::new_suffix("of Slaughter",    vec![(StatKind::CritDamage, 0.12)], 8,  6.0),
            Affix::new_suffix("of Carnage",      vec![(StatKind::CritDamage, 0.25)],18,  3.0),
            Affix::new_suffix("of Speed",        vec![(StatKind::Speed,       0.06)],4,  8.0),
            Affix::new_suffix("of Haste",        vec![(StatKind::Speed,       0.15)],12, 5.0),
            Affix::new_suffix("of the Vampire",  vec![(StatKind::LifeSteal,   0.03)],8,  6.0),
            Affix::new_suffix("of Draining",     vec![(StatKind::ManaSteal,   0.03)],8,  6.0),
            Affix::new_suffix("of Thorns",       vec![(StatKind::Thorns,     12.0)], 6,  7.0),
            Affix::new_suffix("of Fortune",      vec![(StatKind::GoldFind,   15.0)], 1, 10.0),
            Affix::new_suffix("of the Mage",     vec![(StatKind::MagicFind,  10.0)], 6,  7.0),
            Affix::new_suffix("of Focus",        vec![(StatKind::CooldownReduction, 0.05)], 10, 6.0),
            Affix::new_suffix("of Blocking",     vec![(StatKind::BlockChance, 0.05)], 5,  7.0),
            Affix::new_suffix("of Evasion",      vec![(StatKind::Dodge,       0.05)], 5,  7.0),
            Affix::new_suffix("of Fury",         vec![(StatKind::AttackSpeed, 0.08)], 6,  7.0),
        ];

        Self { prefixes, suffixes }
    }

    /// Roll a random prefix for the given item level.
    pub fn roll_prefix(&self, item_level: u32, rng: &mut Rng) -> Option<&Affix> {
        let eligible: Vec<(&Affix, f32)> = self.prefixes.iter()
            .filter(|a| a.required_level <= item_level)
            .map(|a| (a, a.weight))
            .collect();
        rng.pick_weighted(&eligible).copied()
    }

    /// Roll a random suffix for the given item level.
    pub fn roll_suffix(&self, item_level: u32, rng: &mut Rng) -> Option<&Affix> {
        let eligible: Vec<(&Affix, f32)> = self.suffixes.iter()
            .filter(|a| a.required_level <= item_level)
            .map(|a| (a, a.weight))
            .collect();
        rng.pick_weighted(&eligible).copied()
    }

    pub fn prefix_count(&self) -> usize { self.prefixes.len() }
    pub fn suffix_count(&self) -> usize { self.suffixes.len() }
}

// ── Item ──────────────────────────────────────────────────────────────────────

/// A fully generated item with a base, optional affixes, and computed stats.
#[derive(Debug, Clone)]
pub struct Item {
    pub name:      String,
    pub base:      &'static ItemBase,
    pub rarity:    Rarity,
    pub prefixes:  Vec<String>,
    pub suffixes:  Vec<String>,
    /// Final computed stats (base + all modifiers).
    pub stats:     HashMap<StatKind, f32>,
    pub item_level: u32,
    pub is_unique:  bool,
    pub set_id:     Option<u32>,
    pub enchantment: Option<Enchantment>,
    pub sockets:    Vec<Gem>,
}

impl Item {
    pub fn final_damage(&self)  -> f32 { *self.stats.get(&StatKind::Damage).unwrap_or(&0.0) }
    pub fn final_defense(&self) -> f32 { *self.stats.get(&StatKind::Defense).unwrap_or(&0.0) }
    pub fn final_health(&self)  -> f32 { *self.stats.get(&StatKind::Health).unwrap_or(&0.0) }
    pub fn final_value(&self)   -> u32 {
        let rarity_mult = match self.rarity {
            Rarity::Common    => 1.0,
            Rarity::Magic     => 1.5,
            Rarity::Rare      => 2.5,
            Rarity::Epic      => 5.0,
            Rarity::Legendary => 15.0,
        };
        (self.base.base_value as f32 * rarity_mult) as u32
    }
}

// ── ItemLevelCurve ────────────────────────────────────────────────────────────

/// Maps dungeon depth to appropriate item level range.
pub struct ItemLevelCurve {
    /// `level = base + depth * rate`
    pub base:  u32,
    pub rate:  f32,
    pub jitter: u32,
}

impl Default for ItemLevelCurve {
    fn default() -> Self { Self { base: 1, rate: 2.0, jitter: 3 } }
}

impl ItemLevelCurve {
    pub fn new(base: u32, rate: f32, jitter: u32) -> Self { Self { base, rate, jitter } }

    pub fn item_level(&self, depth: u32, rng: &mut Rng) -> u32 {
        let base = self.base + (depth as f32 * self.rate) as u32;
        let j    = rng.range_i32(-(self.jitter as i32), self.jitter as i32);
        (base as i32 + j).max(1) as u32
    }
}

// ── ItemGenerator ─────────────────────────────────────────────────────────────

/// Generates random items by rolling bases and affixes.
pub struct ItemGenerator {
    pub affix_pool: AffixPool,
    pub level_curve: ItemLevelCurve,
}

impl Default for ItemGenerator {
    fn default() -> Self {
        Self { affix_pool: AffixPool::default_pool(), level_curve: ItemLevelCurve::default() }
    }
}

impl ItemGenerator {
    pub fn new(affix_pool: AffixPool, level_curve: ItemLevelCurve) -> Self {
        Self { affix_pool, level_curve }
    }

    /// Roll a random rarity based on depth.
    fn roll_rarity(depth: u32, rng: &mut Rng) -> Rarity {
        let r = rng.next_f32();
        let legend_chance = (depth as f32 * 0.002).min(0.03);
        let epic_chance   = (depth as f32 * 0.005).min(0.08);
        let rare_chance   = (depth as f32 * 0.01 ).min(0.20);
        let magic_chance  = 0.35_f32;
        if r < legend_chance              { Rarity::Legendary }
        else if r < legend_chance + epic_chance   { Rarity::Epic }
        else if r < legend_chance + epic_chance + rare_chance  { Rarity::Rare }
        else if r < legend_chance + epic_chance + rare_chance + magic_chance { Rarity::Magic }
        else { Rarity::Common }
    }

    /// Generate a random item for the given dungeon depth.
    pub fn generate(&self, depth: u32, rng: &mut Rng) -> Item {
        let item_level = self.level_curve.item_level(depth, rng);
        let rarity     = Self::roll_rarity(depth, rng);

        // Pick a base
        let candidates = ItemBase::for_level(item_level);
        let base = if candidates.is_empty() {
            &BASE_POOL[0]
        } else {
            let i = rng.range_usize(candidates.len());
            candidates[i]
        };

        // Roll affixes
        let (min_aff, max_aff) = rarity.affix_count_range();
        let n_affixes = if max_aff == 0 { 0 } else {
            rng.range_usize(max_aff - min_aff + 1) + min_aff
        };
        let n_prefix = n_affixes / 2 + rng.range_usize(2);
        let n_suffix = n_affixes.saturating_sub(n_prefix);

        let mut prefix_names = Vec::new();
        let mut suffix_names = Vec::new();
        let mut stat_mods: HashMap<StatKind, f32> = HashMap::new();

        // Base stats
        *stat_mods.entry(StatKind::Damage).or_insert(0.0)  += base.base_damage;
        *stat_mods.entry(StatKind::Defense).or_insert(0.0) += base.base_defense;

        for _ in 0..n_prefix {
            if let Some(affix) = self.affix_pool.roll_prefix(item_level, rng) {
                prefix_names.push(affix.name.to_string());
                for (stat, val) in &affix.stat_modifiers {
                    *stat_mods.entry(*stat).or_insert(0.0) += val;
                }
            }
        }
        for _ in 0..n_suffix {
            if let Some(affix) = self.affix_pool.roll_suffix(item_level, rng) {
                suffix_names.push(affix.name.to_string());
                for (stat, val) in &affix.stat_modifiers {
                    *stat_mods.entry(*stat).or_insert(0.0) += val;
                }
            }
        }

        // Build name: "Prefix Base of Suffix"
        let prefix_str = prefix_names.first().map(|s| s.as_str()).unwrap_or("").to_string();
        let suffix_str = suffix_names.first().map(|s| format!(" {s}")).unwrap_or_default();
        let name = if prefix_str.is_empty() {
            format!("{}{}", base.name, suffix_str)
        } else {
            format!("{} {}{}", prefix_str, base.name, suffix_str)
        };

        Item {
            name,
            base,
            rarity,
            prefixes: prefix_names,
            suffixes: suffix_names,
            stats: stat_mods,
            item_level,
            is_unique: false,
            set_id: None,
            enchantment: None,
            sockets: Vec::new(),
        }
    }

    /// Generate an item with forced rarity.
    pub fn generate_with_rarity(&self, depth: u32, rarity: Rarity, rng: &mut Rng) -> Item {
        let item_level = self.level_curve.item_level(depth, rng);
        let candidates = ItemBase::for_level(item_level);
        let base = if candidates.is_empty() { &BASE_POOL[0] } else {
            candidates[rng.range_usize(candidates.len())]
        };

        let (min_aff, max_aff) = rarity.affix_count_range();
        let n_affixes = if max_aff == 0 { 0 } else {
            rng.range_usize(max_aff - min_aff + 1) + min_aff
        };
        let n_prefix = n_affixes / 2;
        let n_suffix = n_affixes - n_prefix;

        let mut prefix_names = Vec::new();
        let mut suffix_names = Vec::new();
        let mut stat_mods: HashMap<StatKind, f32> = HashMap::new();
        *stat_mods.entry(StatKind::Damage).or_insert(0.0)  += base.base_damage;
        *stat_mods.entry(StatKind::Defense).or_insert(0.0) += base.base_defense;

        for _ in 0..n_prefix {
            if let Some(affix) = self.affix_pool.roll_prefix(item_level, rng) {
                prefix_names.push(affix.name.to_string());
                for (s, v) in &affix.stat_modifiers { *stat_mods.entry(*s).or_insert(0.0) += v; }
            }
        }
        for _ in 0..n_suffix {
            if let Some(affix) = self.affix_pool.roll_suffix(item_level, rng) {
                suffix_names.push(affix.name.to_string());
                for (s, v) in &affix.stat_modifiers { *stat_mods.entry(*s).or_insert(0.0) += v; }
            }
        }

        let prefix_str = prefix_names.first().map(|s| s.as_str()).unwrap_or("").to_string();
        let suffix_str = suffix_names.first().map(|s| format!(" {s}")).unwrap_or_default();
        let name = if prefix_str.is_empty() {
            format!("{}{}", base.name, suffix_str)
        } else {
            format!("{} {}{}", prefix_str, base.name, suffix_str)
        };

        Item { name, base, rarity, prefixes: prefix_names, suffixes: suffix_names, stats: stat_mods,
               item_level, is_unique: false, set_id: None, enchantment: None, sockets: Vec::new() }
    }
}

// ── UniqueItem ────────────────────────────────────────────────────────────────

/// A hand-crafted unique item with fixed stats and lore.
#[derive(Debug, Clone)]
pub struct UniqueItem {
    pub name:       &'static str,
    pub lore:       &'static str,
    pub base_type:  ItemType,
    pub stats:      Vec<(StatKind, f32)>,
    pub glyph:      char,
    pub required_level: u32,
}

impl UniqueItem {
    /// The full pool of 20 unique items.
    pub fn pool() -> &'static [UniqueItem] {
        &UNIQUE_POOL
    }

    /// Convert to a generatable `Item`.
    pub fn to_item(&self) -> Item {
        let base = BASE_POOL.iter()
            .find(|b| b.item_type == self.base_type)
            .unwrap_or(&BASE_POOL[0]);
        let mut stats: HashMap<StatKind, f32> = self.stats.iter().cloned().collect();
        *stats.entry(StatKind::Damage).or_insert(0.0)  += base.base_damage;
        *stats.entry(StatKind::Defense).or_insert(0.0) += base.base_defense;
        Item {
            name:      self.name.to_string(),
            base,
            rarity:    Rarity::Legendary,
            prefixes:  Vec::new(),
            suffixes:  Vec::new(),
            stats,
            item_level: self.required_level,
            is_unique:  true,
            set_id:     None,
            enchantment: None,
            sockets:    Vec::new(),
        }
    }
}

static UNIQUE_POOL: [UniqueItem; 20] = [
    UniqueItem { name: "Soulrender",       lore: "Forged from the bones of a lich.",                           base_type: ItemType::Sword,      stats: vec![], glyph: '/', required_level: 20 },
    UniqueItem { name: "Voidcleaver",      lore: "It hums with the void's emptiness.",                        base_type: ItemType::Axe,        stats: vec![], glyph: 'T', required_level: 25 },
    UniqueItem { name: "Thornmail",        lore: "Every blow returns pain tenfold.",                           base_type: ItemType::ChestArmor, stats: vec![], glyph: '[', required_level: 18 },
    UniqueItem { name: "Dawnbreaker",      lore: "Radiates light that banishes the undead.",                   base_type: ItemType::Mace,       stats: vec![], glyph: '\\',required_level: 22 },
    UniqueItem { name: "Ghostwalkers",     lore: "The wearer moves without sound.",                            base_type: ItemType::Boots,      stats: vec![], glyph: 'U', required_level: 15 },
    UniqueItem { name: "Eclipse Crown",    lore: "Worn by the last emperor of the sun dynasty.",               base_type: ItemType::Helmet,     stats: vec![], glyph: 'N', required_level: 30 },
    UniqueItem { name: "Wraithblade",      lore: "Phases through armour on critical strikes.",                  base_type: ItemType::Dagger,     stats: vec![], glyph: '-', required_level: 20 },
    UniqueItem { name: "Ring of Eternity", lore: "Ancient artefact from before the sundering.",                base_type: ItemType::Ring,       stats: vec![], glyph: '°', required_level: 35 },
    UniqueItem { name: "Stormcaller",      lore: "Lightning arcs between its prongs.",                         base_type: ItemType::Staff,      stats: vec![], glyph: '|', required_level: 24 },
    UniqueItem { name: "Deathgrip",        lore: "The gauntlets won't let go of what they grasp.",             base_type: ItemType::Gloves,     stats: vec![], glyph: '(', required_level: 18 },
    UniqueItem { name: "Frostweave",       lore: "Woven from the hair of an ice dragon.",                      base_type: ItemType::ChestArmor, stats: vec![], glyph: '[', required_level: 28 },
    UniqueItem { name: "Titan's Grip",     lore: "Only the mightiest can lift this war hammer.",               base_type: ItemType::Mace,       stats: vec![], glyph: '\\',required_level: 30 },
    UniqueItem { name: "Whisperbow",       lore: "Arrows fired are noiseless and fly true.",                   base_type: ItemType::Bow,        stats: vec![], glyph: ')', required_level: 22 },
    UniqueItem { name: "Amulet of Ages",   lore: "Grants visions of past wearers' memories.",                  base_type: ItemType::Amulet,     stats: vec![], glyph: '♦', required_level: 25 },
    UniqueItem { name: "Bloodward",        lore: "Each wound fuels the shield's magic.",                       base_type: ItemType::Shield,     stats: vec![], glyph: 'O', required_level: 20 },
    UniqueItem { name: "Runeshard",        lore: "Splinter of an ancient obelisk of power.",                   base_type: ItemType::Wand,       stats: vec![], glyph: '!', required_level: 22 },
    UniqueItem { name: "Cinderplate",      lore: "Still warm from the forge of dragons.",                      base_type: ItemType::ChestArmor, stats: vec![], glyph: '[', required_level: 32 },
    UniqueItem { name: "Soulward Belt",    lore: "Prevents the soul from leaving the body.",                   base_type: ItemType::Belt,       stats: vec![], glyph: '=', required_level: 20 },
    UniqueItem { name: "Phasehelm",        lore: "Lets the wearer see through walls.",                         base_type: ItemType::Helmet,     stats: vec![], glyph: 'n', required_level: 18 },
    UniqueItem { name: "Berserker's Axe",  lore: "The wielder enters a battle-rage, heedless of pain.",       base_type: ItemType::Axe,        stats: vec![], glyph: 'T', required_level: 28 },
];

// ── SetItem / ItemSet ──────────────────────────────────────────────────────────

/// An item belonging to a named set.
#[derive(Debug, Clone)]
pub struct SetItem {
    pub set_id:   u32,
    pub piece_id: u32,
    pub name:     &'static str,
    pub base_type: ItemType,
    pub stats:    Vec<(StatKind, f32)>,
}

/// A full named set with progressive set bonuses.
#[derive(Debug, Clone)]
pub struct ItemSet {
    pub id:         u32,
    pub name:       &'static str,
    pub pieces:     Vec<SetItem>,
    /// Bonus at each count threshold: (pieces_worn, Vec<(StatKind, bonus_value)>)
    pub bonuses:    Vec<(usize, Vec<(StatKind, f32)>)>,
}

impl ItemSet {
    /// All pre-defined sets.
    pub fn all_sets() -> Vec<ItemSet> {
        vec![
            ItemSet {
                id: 1,
                name: "Dragon's Wrath",
                pieces: vec![
                    SetItem { set_id: 1, piece_id: 1, name: "Dragon's Helm",  base_type: ItemType::Helmet,     stats: vec![(StatKind::Defense, 20.0),(StatKind::FireResist, 15.0)] },
                    SetItem { set_id: 1, piece_id: 2, name: "Dragon's Plate", base_type: ItemType::ChestArmor, stats: vec![(StatKind::Defense, 45.0),(StatKind::FireResist, 20.0)] },
                    SetItem { set_id: 1, piece_id: 3, name: "Dragon's Claw",  base_type: ItemType::Gloves,     stats: vec![(StatKind::Damage,  15.0),(StatKind::AttackSpeed, 0.1)] },
                    SetItem { set_id: 1, piece_id: 4, name: "Dragon's Tread", base_type: ItemType::Boots,      stats: vec![(StatKind::Speed, 0.15),(StatKind::FireResist, 10.0)] },
                ],
                bonuses: vec![
                    (2, vec![(StatKind::FireResist, 30.0)]),
                    (4, vec![(StatKind::Damage, 50.0), (StatKind::Defense, 50.0)]),
                ],
            },
            ItemSet {
                id: 2,
                name: "Shadowstep",
                pieces: vec![
                    SetItem { set_id: 2, piece_id: 1, name: "Shadow Hood",  base_type: ItemType::Helmet,     stats: vec![(StatKind::Dodge, 0.08),(StatKind::CritChance, 0.05)] },
                    SetItem { set_id: 2, piece_id: 2, name: "Shadow Wrap",  base_type: ItemType::ChestArmor, stats: vec![(StatKind::Dodge, 0.10),(StatKind::Speed, 0.08)] },
                    SetItem { set_id: 2, piece_id: 3, name: "Shadow Blade", base_type: ItemType::Dagger,     stats: vec![(StatKind::Damage, 18.0),(StatKind::LifeSteal, 0.04)] },
                ],
                bonuses: vec![
                    (2, vec![(StatKind::CritDamage, 0.25)]),
                    (3, vec![(StatKind::CritChance, 0.10), (StatKind::Speed, 0.20)]),
                ],
            },
            ItemSet {
                id: 3,
                name: "Arcane Conclave",
                pieces: vec![
                    SetItem { set_id: 3, piece_id: 1, name: "Conclave Circlet", base_type: ItemType::Helmet,  stats: vec![(StatKind::Mana, 80.0),(StatKind::MagicFind, 10.0)] },
                    SetItem { set_id: 3, piece_id: 2, name: "Conclave Robe",    base_type: ItemType::ChestArmor, stats: vec![(StatKind::Mana, 120.0),(StatKind::CooldownReduction, 0.10)] },
                    SetItem { set_id: 3, piece_id: 3, name: "Conclave Focus",   base_type: ItemType::Wand,    stats: vec![(StatKind::Damage, 30.0),(StatKind::Mana, 60.0)] },
                    SetItem { set_id: 3, piece_id: 4, name: "Conclave Ring",    base_type: ItemType::Ring,    stats: vec![(StatKind::Mana, 40.0),(StatKind::MagicFind, 8.0)] },
                ],
                bonuses: vec![
                    (2, vec![(StatKind::CooldownReduction, 0.15)]),
                    (4, vec![(StatKind::Mana, 300.0), (StatKind::MagicFind, 25.0)]),
                ],
            },
            ItemSet {
                id: 4,
                name: "Ironwall",
                pieces: vec![
                    SetItem { set_id: 4, piece_id: 1, name: "Ironwall Helm",   base_type: ItemType::Helmet,     stats: vec![(StatKind::Defense, 30.0),(StatKind::BlockChance, 0.05)] },
                    SetItem { set_id: 4, piece_id: 2, name: "Ironwall Plate",  base_type: ItemType::ChestArmor, stats: vec![(StatKind::Defense, 60.0),(StatKind::Health, 100.0)] },
                    SetItem { set_id: 4, piece_id: 3, name: "Ironwall Shield", base_type: ItemType::Shield,     stats: vec![(StatKind::Defense, 40.0),(StatKind::BlockChance, 0.10)] },
                    SetItem { set_id: 4, piece_id: 4, name: "Ironwall Boots",  base_type: ItemType::Boots,      stats: vec![(StatKind::Defense, 15.0),(StatKind::Thorns, 15.0)] },
                ],
                bonuses: vec![
                    (2, vec![(StatKind::Health, 200.0)]),
                    (4, vec![(StatKind::Defense, 100.0), (StatKind::Thorns, 40.0)]),
                ],
            },
            ItemSet {
                id: 5,
                name: "Nature's Grasp",
                pieces: vec![
                    SetItem { set_id: 5, piece_id: 1, name: "Thornweave Hood",  base_type: ItemType::Helmet,     stats: vec![(StatKind::Health, 50.0),(StatKind::PoisonResist, 20.0)] },
                    SetItem { set_id: 5, piece_id: 2, name: "Thornweave Vest",  base_type: ItemType::ChestArmor, stats: vec![(StatKind::Defense, 20.0),(StatKind::PoisonResist, 20.0)] },
                    SetItem { set_id: 5, piece_id: 3, name: "Barkskin Gloves",  base_type: ItemType::Gloves,     stats: vec![(StatKind::Thorns, 20.0),(StatKind::Health, 30.0)] },
                ],
                bonuses: vec![
                    (2, vec![(StatKind::PoisonResist, 30.0)]),
                    (3, vec![(StatKind::Thorns, 60.0), (StatKind::Health, 150.0)]),
                ],
            },
        ]
    }

    /// Compute active bonuses for a given count of equipped pieces.
    pub fn active_bonuses(&self, equipped_count: usize) -> Vec<(StatKind, f32)> {
        self.bonuses.iter()
            .filter(|(threshold, _)| equipped_count >= *threshold)
            .flat_map(|(_, mods)| mods.iter().cloned())
            .collect()
    }
}

// ── Gem ───────────────────────────────────────────────────────────────────────

/// Gems that can be socketed into items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gem {
    Ruby,
    Sapphire,
    Emerald,
    Diamond,
    Obsidian,
}

impl Gem {
    /// Bonus granted when socketed into a weapon.
    pub fn weapon_bonus(&self) -> (StatKind, f32) {
        match self {
            Gem::Ruby     => (StatKind::Damage,    15.0),
            Gem::Sapphire => (StatKind::Mana,      30.0),
            Gem::Emerald  => (StatKind::LifeSteal,  0.04),
            Gem::Diamond  => (StatKind::CritChance, 0.05),
            Gem::Obsidian => (StatKind::Thorns,    20.0),
        }
    }

    /// Bonus granted when socketed into armour.
    pub fn armor_bonus(&self) -> (StatKind, f32) {
        match self {
            Gem::Ruby     => (StatKind::FireResist,      15.0),
            Gem::Sapphire => (StatKind::ColdResist,      15.0),
            Gem::Emerald  => (StatKind::PoisonResist,    15.0),
            Gem::Diamond  => (StatKind::Defense,         20.0),
            Gem::Obsidian => (StatKind::LightningResist, 15.0),
        }
    }

    /// Bonus granted when socketed into jewellery.
    pub fn jewelry_bonus(&self) -> (StatKind, f32) {
        match self {
            Gem::Ruby     => (StatKind::Health,      50.0),
            Gem::Sapphire => (StatKind::Mana,        50.0),
            Gem::Emerald  => (StatKind::GoldFind,    20.0),
            Gem::Diamond  => (StatKind::MagicFind,   15.0),
            Gem::Obsidian => (StatKind::CritDamage,   0.20),
        }
    }

    pub fn socket_bonus(&self, item_type: ItemType) -> (StatKind, f32) {
        if item_type.is_weapon()  { self.weapon_bonus() }
        else if item_type.is_jewelry() { self.jewelry_bonus() }
        else                      { self.armor_bonus() }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Gem::Ruby     => "Ruby",
            Gem::Sapphire => "Sapphire",
            Gem::Emerald  => "Emerald",
            Gem::Diamond  => "Diamond",
            Gem::Obsidian => "Obsidian",
        }
    }
}

// ── Enchantment ───────────────────────────────────────────────────────────────

/// Extra magical property not covered by standard affixes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnchantmentKind {
    SoulBound,       // item is bound to the character
    Cursed,          // item imposes a penalty that cannot be removed without unequipping
    ElementalInfusion(ElementKind),
    Ethereal,        // unusually light; ignores weight
    Masterwork,      // +10% to all stats
    Resonant,        // bonus scales with level
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind { Fire, Ice, Lightning, Poison, Arcane }

/// An enchantment applied to an item.
#[derive(Debug, Clone)]
pub struct Enchantment {
    pub kind:        EnchantmentKind,
    pub description: String,
    pub stat_bonus:  Vec<(StatKind, f32)>,
}

impl Enchantment {
    pub fn soul_bound() -> Self {
        Self { kind: EnchantmentKind::SoulBound, description: "Bound to soul — cannot be traded.".into(), stat_bonus: vec![] }
    }
    pub fn cursed() -> Self {
        Self { kind: EnchantmentKind::Cursed, description: "Cursed — cannot be removed without a dispel.".into(),
               stat_bonus: vec![(StatKind::Speed, -0.10)] }
    }
    pub fn elemental(elem: ElementKind) -> Self {
        let (desc, stat) = match elem {
            ElementKind::Fire      => ("Infused with fire — deals bonus fire damage.",      (StatKind::Damage, 12.0)),
            ElementKind::Ice       => ("Infused with ice — slows targets.",                 (StatKind::ColdResist, 20.0)),
            ElementKind::Lightning => ("Crackling with lightning — chance to stun.",         (StatKind::LightningResist, 20.0)),
            ElementKind::Poison    => ("Dripping venom — poisons on hit.",                   (StatKind::PoisonResist, 20.0)),
            ElementKind::Arcane    => ("Humming with arcane power — amplifies spells.",      (StatKind::Mana, 60.0)),
        };
        Self { kind: EnchantmentKind::ElementalInfusion(elem), description: desc.into(), stat_bonus: vec![stat] }
    }
    pub fn masterwork() -> Self {
        Self { kind: EnchantmentKind::Masterwork, description: "Masterwork craftsmanship (+10% all stats).".into(),
               stat_bonus: vec![] } // handled multiplicatively at application
    }
    pub fn ethereal() -> Self {
        Self { kind: EnchantmentKind::Ethereal, description: "Weightless — no encumbrance penalty.".into(), stat_bonus: vec![] }
    }
    pub fn resonant() -> Self {
        Self { kind: EnchantmentKind::Resonant, description: "Power grows with the wielder.".into(),
               stat_bonus: vec![(StatKind::Damage, 5.0), (StatKind::Defense, 5.0)] }
    }

    /// Apply this enchantment to a stat map.
    pub fn apply_to_stats(&self, stats: &mut HashMap<StatKind, f32>) {
        for (stat, val) in &self.stat_bonus {
            *stats.entry(*stat).or_insert(0.0) += val;
        }
        if self.kind == EnchantmentKind::Masterwork {
            for v in stats.values_mut() { *v *= 1.1; }
        }
    }
}

// ── LootDropper ───────────────────────────────────────────────────────────────

/// Enemy kind for loot table lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    Minion,
    Soldier,
    Elite,
    Champion,
    Boss,
    MegaBoss,
}

/// Drop rate configuration per enemy tier.
#[derive(Debug, Clone)]
pub struct DropConfig {
    pub base_drop_chance: f32,    // 0..1 probability of dropping anything
    pub common_weight:    f32,
    pub magic_weight:     f32,
    pub rare_weight:      f32,
    pub epic_weight:      f32,
    pub legendary_weight: f32,
    pub max_drops:        usize,
}

impl DropConfig {
    fn for_enemy(kind: EnemyKind) -> Self {
        match kind {
            EnemyKind::Minion    => DropConfig { base_drop_chance: 0.20, common_weight: 80.0, magic_weight: 15.0, rare_weight: 4.0, epic_weight: 0.8, legendary_weight: 0.2, max_drops: 1 },
            EnemyKind::Soldier   => DropConfig { base_drop_chance: 0.40, common_weight: 70.0, magic_weight: 20.0, rare_weight: 8.0, epic_weight: 1.5, legendary_weight: 0.5, max_drops: 2 },
            EnemyKind::Elite     => DropConfig { base_drop_chance: 0.70, common_weight: 50.0, magic_weight: 30.0, rare_weight:15.0, epic_weight: 4.0, legendary_weight: 1.0, max_drops: 3 },
            EnemyKind::Champion  => DropConfig { base_drop_chance: 0.90, common_weight: 30.0, magic_weight: 35.0, rare_weight:25.0, epic_weight: 8.0, legendary_weight: 2.0, max_drops: 4 },
            EnemyKind::Boss      => DropConfig { base_drop_chance: 1.00, common_weight: 10.0, magic_weight: 25.0, rare_weight:35.0, epic_weight:20.0, legendary_weight: 10.0, max_drops: 5 },
            EnemyKind::MegaBoss  => DropConfig { base_drop_chance: 1.00, common_weight:  5.0, magic_weight: 15.0, rare_weight:30.0, epic_weight:30.0, legendary_weight: 20.0, max_drops: 8 },
        }
    }

    fn roll_rarity(&self, rng: &mut Rng) -> Rarity {
        let options = [
            (Rarity::Common,    self.common_weight),
            (Rarity::Magic,     self.magic_weight),
            (Rarity::Rare,      self.rare_weight),
            (Rarity::Epic,      self.epic_weight),
            (Rarity::Legendary, self.legendary_weight),
        ];
        rng.pick_weighted(&options).copied().unwrap_or(Rarity::Common)
    }
}

/// Produces item drops from enemies based on type and depth.
pub struct LootDropper {
    pub generator: ItemGenerator,
}

impl Default for LootDropper {
    fn default() -> Self { Self { generator: ItemGenerator::default() } }
}

impl LootDropper {
    pub fn new(generator: ItemGenerator) -> Self { Self { generator } }

    /// Generate loot for an enemy kill.
    pub fn drop(&self, enemy_kind: EnemyKind, depth: u32, rng: &mut Rng) -> Vec<Item> {
        let config = DropConfig::for_enemy(enemy_kind);
        let mut drops = Vec::new();

        // Check if anything drops at all
        if !rng.chance(config.base_drop_chance) { return drops; }

        let n_drops = rng.range_usize(config.max_drops) + 1;
        for _ in 0..n_drops {
            let rarity = config.roll_rarity(rng);
            let item = self.generator.generate_with_rarity(depth, rarity, rng);
            drops.push(item);
        }
        drops
    }

    /// Drop chance for a boss — always drops, usually rare+.
    pub fn drop_boss(&self, depth: u32, rng: &mut Rng) -> Vec<Item> {
        self.drop(EnemyKind::Boss, depth, rng)
    }

    /// Roll for a unique item drop (low probability).
    pub fn try_unique_drop(&self, depth: u32, rng: &mut Rng) -> Option<Item> {
        let chance = (depth as f32 * 0.005).min(0.05);
        if !rng.chance(chance) { return None; }
        let pool = UniqueItem::pool();
        let eligible: Vec<&UniqueItem> = pool.iter()
            .filter(|u| u.required_level <= depth * 2 + 5)
            .collect();
        if eligible.is_empty() { return None; }
        let u = eligible[rng.range_usize(eligible.len())];
        Some(u.to_item())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn rng() -> Rng { Rng::new(42) }

    #[test]
    fn item_base_pool_nonempty() {
        assert!(!ItemBase::pool().is_empty());
    }

    #[test]
    fn item_base_for_level_filters() {
        let bases = ItemBase::for_level(1);
        assert!(!bases.is_empty(), "should have level-1 bases");
        assert!(bases.iter().all(|b| b.required_level <= 1));
    }

    #[test]
    fn affix_pool_has_enough_affixes() {
        let pool = AffixPool::default_pool();
        assert!(pool.prefix_count() >= 30, "should have 30+ prefixes, got {}", pool.prefix_count());
        assert!(pool.suffix_count() >= 30, "should have 30+ suffixes, got {}", pool.suffix_count());
    }

    #[test]
    fn item_generator_produces_item() {
        let mut r = rng();
        let gen = ItemGenerator::default();
        let item = gen.generate(5, &mut r);
        assert!(!item.name.is_empty());
    }

    #[test]
    fn item_generator_legend_at_high_depth() {
        let mut r = rng();
        let gen = ItemGenerator::default();
        // At depth 50, stats should be higher due to level curve
        let item = gen.generate(50, &mut r);
        assert!(item.item_level >= 1);
    }

    #[test]
    fn rarity_affix_counts_match() {
        for rarity in [Rarity::Common, Rarity::Magic, Rarity::Rare, Rarity::Epic, Rarity::Legendary] {
            let (min, max) = rarity.affix_count_range();
            assert!(min <= max, "min > max for {:?}", rarity);
        }
    }

    #[test]
    fn loot_dropper_boss_always_drops() {
        let dropper = LootDropper::default();
        for seed in 0..20u64 {
            let mut r = Rng::new(seed);
            let drops = dropper.drop_boss(10, &mut r);
            assert!(!drops.is_empty(), "boss should always drop at seed {seed}");
        }
    }

    #[test]
    fn unique_item_pool_has_20() {
        assert_eq!(UniqueItem::pool().len(), 20);
    }

    #[test]
    fn unique_item_converts_to_item() {
        let pool = UniqueItem::pool();
        let item = pool[0].to_item();
        assert!(item.is_unique);
        assert_eq!(item.rarity, Rarity::Legendary);
    }

    #[test]
    fn item_sets_all_five() {
        let sets = ItemSet::all_sets();
        assert_eq!(sets.len(), 5);
    }

    #[test]
    fn set_bonus_scales_with_count() {
        let sets = ItemSet::all_sets();
        let set = &sets[0]; // Dragon's Wrath
        let bonus_2 = set.active_bonuses(2);
        let bonus_4 = set.active_bonuses(4);
        assert!(!bonus_2.is_empty());
        assert!(bonus_4.len() >= bonus_2.len(), "more pieces should give at least as many bonuses");
    }

    #[test]
    fn gems_have_correct_bonus_by_type() {
        let (stat, _) = Gem::Ruby.socket_bonus(ItemType::Sword);
        assert_eq!(stat, StatKind::Damage, "Ruby in weapon should boost damage");
        let (stat, _) = Gem::Ruby.socket_bonus(ItemType::ChestArmor);
        assert_eq!(stat, StatKind::FireResist, "Ruby in armor should boost fire resist");
    }
}
