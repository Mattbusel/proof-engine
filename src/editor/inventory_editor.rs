#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// ITEM CATEGORY ENUM (40+ types)
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    // Melee Weapons
    Sword,
    Greatsword,
    Dagger,
    Axe,
    Greataxe,
    Mace,
    Hammer,
    Spear,
    Polearm,
    Whip,
    // Ranged Weapons
    Bow,
    Crossbow,
    Thrown,
    Gun,
    Rifle,
    // Magic Weapons
    Staff,
    Wand,
    Orb,
    Tome,
    // Shields
    Shield,
    Buckler,
    TowerShield,
    // Head Armor
    Helmet,
    Hood,
    Crown,
    // Body Armor
    Chestplate,
    Robe,
    Leather,
    // Leg Armor
    Greaves,
    Leggings,
    Pants,
    // Hand/Foot
    Gauntlets,
    Gloves,
    Boots,
    Sabatons,
    // Accessories
    Ring,
    Amulet,
    Belt,
    Cloak,
    // Consumables
    Potion,
    Food,
    Scroll,
    Bomb,
    // Materials
    Ore,
    Gem,
    Herb,
    Hide,
    Wood,
    Cloth,
    // Misc
    QuestItem,
    Key,
    Currency,
    Junk,
}

impl ItemCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            ItemCategory::Sword => "Sword",
            ItemCategory::Greatsword => "Greatsword",
            ItemCategory::Dagger => "Dagger",
            ItemCategory::Axe => "Axe",
            ItemCategory::Greataxe => "Greataxe",
            ItemCategory::Mace => "Mace",
            ItemCategory::Hammer => "Hammer",
            ItemCategory::Spear => "Spear",
            ItemCategory::Polearm => "Polearm",
            ItemCategory::Whip => "Whip",
            ItemCategory::Bow => "Bow",
            ItemCategory::Crossbow => "Crossbow",
            ItemCategory::Thrown => "Thrown",
            ItemCategory::Gun => "Gun",
            ItemCategory::Rifle => "Rifle",
            ItemCategory::Staff => "Staff",
            ItemCategory::Wand => "Wand",
            ItemCategory::Orb => "Orb",
            ItemCategory::Tome => "Tome",
            ItemCategory::Shield => "Shield",
            ItemCategory::Buckler => "Buckler",
            ItemCategory::TowerShield => "Tower Shield",
            ItemCategory::Helmet => "Helmet",
            ItemCategory::Hood => "Hood",
            ItemCategory::Crown => "Crown",
            ItemCategory::Chestplate => "Chestplate",
            ItemCategory::Robe => "Robe",
            ItemCategory::Leather => "Leather Armor",
            ItemCategory::Greaves => "Greaves",
            ItemCategory::Leggings => "Leggings",
            ItemCategory::Pants => "Pants",
            ItemCategory::Gauntlets => "Gauntlets",
            ItemCategory::Gloves => "Gloves",
            ItemCategory::Boots => "Boots",
            ItemCategory::Sabatons => "Sabatons",
            ItemCategory::Ring => "Ring",
            ItemCategory::Amulet => "Amulet",
            ItemCategory::Belt => "Belt",
            ItemCategory::Cloak => "Cloak",
            ItemCategory::Potion => "Potion",
            ItemCategory::Food => "Food",
            ItemCategory::Scroll => "Scroll",
            ItemCategory::Bomb => "Bomb",
            ItemCategory::Ore => "Ore",
            ItemCategory::Gem => "Gem",
            ItemCategory::Herb => "Herb",
            ItemCategory::Hide => "Hide",
            ItemCategory::Wood => "Wood",
            ItemCategory::Cloth => "Cloth",
            ItemCategory::QuestItem => "Quest Item",
            ItemCategory::Key => "Key",
            ItemCategory::Currency => "Currency",
            ItemCategory::Junk => "Junk",
        }
    }

    pub fn is_weapon(&self) -> bool {
        matches!(self,
            ItemCategory::Sword | ItemCategory::Greatsword | ItemCategory::Dagger |
            ItemCategory::Axe | ItemCategory::Greataxe | ItemCategory::Mace |
            ItemCategory::Hammer | ItemCategory::Spear | ItemCategory::Polearm |
            ItemCategory::Whip | ItemCategory::Bow | ItemCategory::Crossbow |
            ItemCategory::Thrown | ItemCategory::Gun | ItemCategory::Rifle |
            ItemCategory::Staff | ItemCategory::Wand | ItemCategory::Orb | ItemCategory::Tome
        )
    }

    pub fn is_armor(&self) -> bool {
        matches!(self,
            ItemCategory::Helmet | ItemCategory::Hood | ItemCategory::Crown |
            ItemCategory::Chestplate | ItemCategory::Robe | ItemCategory::Leather |
            ItemCategory::Greaves | ItemCategory::Leggings | ItemCategory::Pants |
            ItemCategory::Gauntlets | ItemCategory::Gloves | ItemCategory::Boots |
            ItemCategory::Sabatons | ItemCategory::Shield | ItemCategory::Buckler |
            ItemCategory::TowerShield
        )
    }

    pub fn is_accessory(&self) -> bool {
        matches!(self,
            ItemCategory::Ring | ItemCategory::Amulet | ItemCategory::Belt | ItemCategory::Cloak
        )
    }

    pub fn is_consumable(&self) -> bool {
        matches!(self,
            ItemCategory::Potion | ItemCategory::Food | ItemCategory::Scroll | ItemCategory::Bomb
        )
    }

    pub fn is_material(&self) -> bool {
        matches!(self,
            ItemCategory::Ore | ItemCategory::Gem | ItemCategory::Herb |
            ItemCategory::Hide | ItemCategory::Wood | ItemCategory::Cloth
        )
    }

    pub fn default_stack_size(&self) -> u32 {
        if self.is_material() { 999 }
        else if self.is_consumable() { 20 }
        else if *self == ItemCategory::Currency { 99999 }
        else if *self == ItemCategory::Junk { 10 }
        else { 1 }
    }

    pub fn two_handed(&self) -> bool {
        matches!(self,
            ItemCategory::Greatsword | ItemCategory::Greataxe | ItemCategory::Hammer |
            ItemCategory::Polearm | ItemCategory::Bow | ItemCategory::Rifle |
            ItemCategory::Staff | ItemCategory::TowerShield
        )
    }
}

// ============================================================
// RARITY SYSTEM
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

#[derive(Debug, Clone)]
pub struct RarityConfig {
    pub rarity: Rarity,
    pub color: Vec4,
    pub drop_weight: f32,
    pub min_affixes: u32,
    pub max_affixes: u32,
    pub quality_min: f32,
    pub quality_max: f32,
    pub stat_multiplier: f32,
    pub vendor_price_multiplier: f32,
}

impl RarityConfig {
    pub fn for_rarity(rarity: Rarity) -> Self {
        match rarity {
            Rarity::Common => RarityConfig {
                rarity,
                color: Vec4::new(0.8, 0.8, 0.8, 1.0),
                drop_weight: 10000.0,
                min_affixes: 0,
                max_affixes: 1,
                quality_min: 0.5,
                quality_max: 0.8,
                stat_multiplier: 1.0,
                vendor_price_multiplier: 1.0,
            },
            Rarity::Uncommon => RarityConfig {
                rarity,
                color: Vec4::new(0.1, 0.9, 0.1, 1.0),
                drop_weight: 2000.0,
                min_affixes: 1,
                max_affixes: 2,
                quality_min: 0.7,
                quality_max: 0.9,
                stat_multiplier: 1.2,
                vendor_price_multiplier: 3.0,
            },
            Rarity::Rare => RarityConfig {
                rarity,
                color: Vec4::new(0.2, 0.4, 1.0, 1.0),
                drop_weight: 400.0,
                min_affixes: 2,
                max_affixes: 4,
                quality_min: 0.8,
                quality_max: 1.0,
                stat_multiplier: 1.5,
                vendor_price_multiplier: 10.0,
            },
            Rarity::Epic => RarityConfig {
                rarity,
                color: Vec4::new(0.6, 0.1, 0.9, 1.0),
                drop_weight: 80.0,
                min_affixes: 3,
                max_affixes: 5,
                quality_min: 0.9,
                quality_max: 1.1,
                stat_multiplier: 2.0,
                vendor_price_multiplier: 50.0,
            },
            Rarity::Legendary => RarityConfig {
                rarity,
                color: Vec4::new(1.0, 0.5, 0.0, 1.0),
                drop_weight: 15.0,
                min_affixes: 4,
                max_affixes: 6,
                quality_min: 1.0,
                quality_max: 1.2,
                stat_multiplier: 3.0,
                vendor_price_multiplier: 200.0,
            },
            Rarity::Mythic => RarityConfig {
                rarity,
                color: Vec4::new(1.0, 0.0, 0.3, 1.0),
                drop_weight: 2.0,
                min_affixes: 5,
                max_affixes: 8,
                quality_min: 1.1,
                quality_max: 1.5,
                stat_multiplier: 5.0,
                vendor_price_multiplier: 1000.0,
            },
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self.rarity {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
            Rarity::Mythic => "Mythic",
        }
    }
}

// Alias method for O(1) weighted random sampling
#[derive(Debug, Clone)]
pub struct AliasTable {
    pub prob: Vec<f64>,
    pub alias: Vec<usize>,
    pub n: usize,
}

impl AliasTable {
    pub fn new(weights: &[f64]) -> Self {
        let n = weights.len();
        let total: f64 = weights.iter().sum();
        let avg = total / n as f64;
        let mut prob = vec![0.0f64; n];
        let mut alias = vec![0usize; n];
        let mut small: VecDeque<usize> = VecDeque::new();
        let mut large: VecDeque<usize> = VecDeque::new();

        for (i, &w) in weights.iter().enumerate() {
            prob[i] = w / avg;
            if prob[i] < 1.0 {
                small.push_back(i);
            } else {
                large.push_back(i);
            }
        }

        while !small.is_empty() && !large.is_empty() {
            let s = small.pop_front().unwrap();
            let l = large.pop_front().unwrap();
            alias[s] = l;
            prob[l] -= 1.0 - prob[s];
            if prob[l] < 1.0 {
                small.push_back(l);
            } else {
                large.push_back(l);
            }
        }
        // remaining large/small due to floating point all set to 1
        for l in large { prob[l] = 1.0; }
        for s in small { prob[s] = 1.0; }

        AliasTable { prob, alias, n }
    }

    pub fn sample(&self, u1: f64, u2: f64) -> usize {
        let i = (u1 * self.n as f64) as usize % self.n;
        if u2 < self.prob[i] { i } else { self.alias[i] }
    }
}

// ============================================================
// STAT MODIFIERS
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum StatType {
    Strength,
    Dexterity,
    Intelligence,
    Vitality,
    Endurance,
    Luck,
    AttackSpeed,
    CastSpeed,
    MoveSpeed,
    CritChance,
    CritMultiplier,
    DodgeChance,
    BlockChance,
    LifeMax,
    ManaMax,
    StaminaMax,
    LifeRegen,
    ManaRegen,
    FireResistance,
    ColdResistance,
    LightningResistance,
    PoisonResistance,
    PhysicalDamage,
    FireDamage,
    ColdDamage,
    LightningDamage,
    PoisonDamage,
    PhysicalArmor,
    MagicArmor,
    ThornsMultiplier,
    AreaRadius,
    SkillDuration,
    ResourceCostReduction,
    CooldownReduction,
    ExperienceGain,
    GoldFind,
    MagicFind,
}

impl StatType {
    pub fn display_name(&self) -> &str {
        match self {
            StatType::Strength => "Strength",
            StatType::Dexterity => "Dexterity",
            StatType::Intelligence => "Intelligence",
            StatType::Vitality => "Vitality",
            StatType::Endurance => "Endurance",
            StatType::Luck => "Luck",
            StatType::AttackSpeed => "Attack Speed",
            StatType::CastSpeed => "Cast Speed",
            StatType::MoveSpeed => "Move Speed",
            StatType::CritChance => "Critical Chance",
            StatType::CritMultiplier => "Critical Multiplier",
            StatType::DodgeChance => "Dodge Chance",
            StatType::BlockChance => "Block Chance",
            StatType::LifeMax => "Max Life",
            StatType::ManaMax => "Max Mana",
            StatType::StaminaMax => "Max Stamina",
            StatType::LifeRegen => "Life Regen",
            StatType::ManaRegen => "Mana Regen",
            StatType::FireResistance => "Fire Resistance",
            StatType::ColdResistance => "Cold Resistance",
            StatType::LightningResistance => "Lightning Resistance",
            StatType::PoisonResistance => "Poison Resistance",
            StatType::PhysicalDamage => "Physical Damage",
            StatType::FireDamage => "Fire Damage",
            StatType::ColdDamage => "Cold Damage",
            StatType::LightningDamage => "Lightning Damage",
            StatType::PoisonDamage => "Poison Damage",
            StatType::PhysicalArmor => "Physical Armor",
            StatType::MagicArmor => "Magic Armor",
            StatType::ThornsMultiplier => "Thorns",
            StatType::AreaRadius => "Area Radius",
            StatType::SkillDuration => "Skill Duration",
            StatType::ResourceCostReduction => "Resource Cost Reduction",
            StatType::CooldownReduction => "Cooldown Reduction",
            StatType::ExperienceGain => "Experience Gain",
            StatType::GoldFind => "Gold Find",
            StatType::MagicFind => "Magic Find",
        }
    }

    pub fn is_percentage(&self) -> bool {
        matches!(self,
            StatType::AttackSpeed | StatType::CastSpeed | StatType::MoveSpeed |
            StatType::CritChance | StatType::CritMultiplier | StatType::DodgeChance |
            StatType::BlockChance | StatType::LifeRegen | StatType::ManaRegen |
            StatType::FireResistance | StatType::ColdResistance | StatType::LightningResistance |
            StatType::PoisonResistance | StatType::CooldownReduction |
            StatType::ResourceCostReduction | StatType::ExperienceGain |
            StatType::GoldFind | StatType::MagicFind
        )
    }
}

#[derive(Debug, Clone)]
pub struct StatModifier {
    pub stat: StatType,
    pub flat_value: f32,
    pub percent_value: f32,
    pub more_multiplier: f32, // multiplicative "more" modifier
}

impl StatModifier {
    pub fn flat(stat: StatType, value: f32) -> Self {
        StatModifier { stat, flat_value: value, percent_value: 0.0, more_multiplier: 1.0 }
    }

    pub fn percent(stat: StatType, value: f32) -> Self {
        StatModifier { stat, flat_value: 0.0, percent_value: value, more_multiplier: 1.0 }
    }

    pub fn more(stat: StatType, multiplier: f32) -> Self {
        StatModifier { stat, flat_value: 0.0, percent_value: 0.0, more_multiplier: multiplier }
    }

    pub fn apply(&self, base: f32) -> f32 {
        (base + self.flat_value) * (1.0 + self.percent_value / 100.0) * self.more_multiplier
    }
}

// ============================================================
// REQUIREMENTS
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemRequirements {
    pub level: u32,
    pub strength: u32,
    pub dexterity: u32,
    pub intelligence: u32,
    pub vitality: u32,
    pub class_restrictions: Vec<String>,
    pub quest_flag: Option<String>,
    pub reputation_faction: Option<String>,
    pub reputation_level: i32,
}

impl ItemRequirements {
    pub fn none() -> Self {
        ItemRequirements {
            level: 1,
            strength: 0,
            dexterity: 0,
            intelligence: 0,
            vitality: 0,
            class_restrictions: Vec::new(),
            quest_flag: None,
            reputation_faction: None,
            reputation_level: 0,
        }
    }

    pub fn meets_requirements(&self, player_level: u32, str: u32, dex: u32, int: u32, vit: u32) -> bool {
        player_level >= self.level
            && str >= self.strength
            && dex >= self.dexterity
            && int >= self.intelligence
            && vit >= self.vitality
    }
}

// ============================================================
// ITEM DEFINITION — 30+ fields
// ============================================================

#[derive(Debug, Clone)]
pub struct DamageRange {
    pub min_damage: f32,
    pub max_damage: f32,
    pub damage_type: DamageType,
    pub crit_multiplier: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageType {
    Physical,
    Fire,
    Cold,
    Lightning,
    Poison,
    Arcane,
    Holy,
    Shadow,
    Chaos,
    True, // bypasses all resistance
}

impl DamageType {
    pub fn color(&self) -> Vec4 {
        match self {
            DamageType::Physical => Vec4::new(0.8, 0.7, 0.6, 1.0),
            DamageType::Fire => Vec4::new(1.0, 0.3, 0.0, 1.0),
            DamageType::Cold => Vec4::new(0.3, 0.7, 1.0, 1.0),
            DamageType::Lightning => Vec4::new(1.0, 1.0, 0.0, 1.0),
            DamageType::Poison => Vec4::new(0.4, 0.9, 0.1, 1.0),
            DamageType::Arcane => Vec4::new(0.8, 0.2, 1.0, 1.0),
            DamageType::Holy => Vec4::new(1.0, 0.95, 0.7, 1.0),
            DamageType::Shadow => Vec4::new(0.3, 0.0, 0.5, 1.0),
            DamageType::Chaos => Vec4::new(0.6, 0.0, 0.2, 1.0),
            DamageType::True => Vec4::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArmorValues {
    pub physical_armor: f32,
    pub magic_armor: f32,
    pub fire_resist: f32,
    pub cold_resist: f32,
    pub lightning_resist: f32,
    pub poison_resist: f32,
    pub block_chance: f32,
    pub block_amount: f32,
    pub evasion_rating: f32,
    pub energy_shield: f32,
}

impl ArmorValues {
    pub fn zero() -> Self {
        ArmorValues {
            physical_armor: 0.0,
            magic_armor: 0.0,
            fire_resist: 0.0,
            cold_resist: 0.0,
            lightning_resist: 0.0,
            poison_resist: 0.0,
            block_chance: 0.0,
            block_amount: 0.0,
            evasion_rating: 0.0,
            energy_shield: 0.0,
        }
    }

    pub fn effective_mitigation(&self, damage_type: DamageType, raw_damage: f32) -> f32 {
        let resist = match damage_type {
            DamageType::Physical => self.physical_armor / (self.physical_armor + 300.0),
            DamageType::Fire => (self.fire_resist / 100.0).min(0.75),
            DamageType::Cold => (self.cold_resist / 100.0).min(0.75),
            DamageType::Lightning => (self.lightning_resist / 100.0).min(0.75),
            DamageType::Poison => (self.poison_resist / 100.0).min(0.75),
            DamageType::Arcane => (self.magic_armor / (self.magic_armor + 200.0)).min(0.6),
            DamageType::Holy | DamageType::Shadow | DamageType::Chaos => 0.0,
            DamageType::True => 0.0,
        };
        raw_damage * (1.0 - resist)
    }
}

#[derive(Debug, Clone)]
pub struct ItemDefinition {
    // Identity — fields 1–8
    pub id: u64,
    pub name: String,
    pub description: String,
    pub flavor_text: String,
    pub category: ItemCategory,
    pub rarity: Rarity,
    pub icon_path: String,
    pub model_path: String,
    // Physical properties — fields 9–14
    pub weight: f32,
    pub volume: f32,
    pub width: u8,   // grid width in cells
    pub height: u8,  // grid height in cells
    pub stack_size: u32,
    pub durability_max: f32,
    // Combat stats — fields 15–22
    pub damage_ranges: Vec<DamageRange>,
    pub attack_speed: f32,
    pub range: f32,
    pub armor_values: ArmorValues,
    pub crit_chance: f32,
    pub crit_multiplier: f32,
    pub life_on_hit: f32,
    pub mana_on_hit: f32,
    // Level / economy — fields 23–28
    pub item_level: u32,
    pub required_level: u32,
    pub base_price: u64,
    pub sell_price_ratio: f32,
    pub vendor_category: Option<String>,
    pub bind_on_pickup: bool,
    // Stat modifiers and requirements — fields 29–35
    pub implicit_modifiers: Vec<StatModifier>,
    pub explicit_modifiers: Vec<StatModifier>,
    pub requirements: ItemRequirements,
    pub set_id: Option<u64>,
    pub unique_effect: Option<String>,
    pub loot_filter_tier: u8,
    pub drop_sound: String,
}

impl ItemDefinition {
    pub fn new(id: u64, name: impl Into<String>, category: ItemCategory) -> Self {
        let cat = category;
        ItemDefinition {
            id,
            name: name.into(),
            description: String::new(),
            flavor_text: String::new(),
            category: cat,
            rarity: Rarity::Common,
            icon_path: String::new(),
            model_path: String::new(),
            weight: 1.0,
            volume: 1.0,
            width: 1,
            height: 1,
            stack_size: cat.default_stack_size(),
            durability_max: 100.0,
            damage_ranges: Vec::new(),
            attack_speed: 1.0,
            range: 1.5,
            armor_values: ArmorValues::zero(),
            crit_chance: 5.0,
            crit_multiplier: 1.5,
            life_on_hit: 0.0,
            mana_on_hit: 0.0,
            item_level: 1,
            required_level: 1,
            base_price: 10,
            sell_price_ratio: 0.25,
            vendor_category: None,
            bind_on_pickup: false,
            implicit_modifiers: Vec::new(),
            explicit_modifiers: Vec::new(),
            requirements: ItemRequirements::none(),
            set_id: None,
            unique_effect: None,
            loot_filter_tier: 1,
            drop_sound: "item_drop_default".to_string(),
        }
    }

    pub fn total_damage_per_second(&self) -> f32 {
        let avg_damage: f32 = self.damage_ranges.iter().map(|d| {
            (d.min_damage + d.max_damage) / 2.0
        }).sum();
        avg_damage * self.attack_speed
    }

    pub fn effective_sell_price(&self) -> u64 {
        (self.base_price as f32 * self.sell_price_ratio) as u64
    }

    pub fn rarity_config(&self) -> RarityConfig {
        RarityConfig::for_rarity(self.rarity)
    }

    pub fn total_stat_budget(&self) -> f32 {
        // Compute weighted point-buy budget for balance validation
        let mut budget = 0.0f32;
        for m in &self.implicit_modifiers {
            budget += stat_budget_cost(&m.stat) * (m.flat_value.abs() + m.percent_value.abs() * 2.0);
        }
        for m in &self.explicit_modifiers {
            budget += stat_budget_cost(&m.stat) * (m.flat_value.abs() + m.percent_value.abs() * 2.0);
        }
        budget
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("Name is empty".to_string()); }
        if self.item_level == 0 { errors.push("Item level must be >= 1".to_string()); }
        if self.weight < 0.0 { errors.push("Weight cannot be negative".to_string()); }
        if self.volume < 0.0 { errors.push("Volume cannot be negative".to_string()); }
        if self.stack_size == 0 { errors.push("Stack size must be >= 1".to_string()); }
        if self.crit_chance < 0.0 || self.crit_chance > 100.0 {
            errors.push("Crit chance must be 0–100".to_string());
        }
        if self.sell_price_ratio > 1.0 {
            errors.push("Sell price ratio > 1 (item sells for more than it costs)".to_string());
        }
        for d in &self.damage_ranges {
            if d.min_damage > d.max_damage {
                errors.push(format!("Damage range min > max for {:?}", d.damage_type));
            }
        }
        errors
    }
}

fn stat_budget_cost(stat: &StatType) -> f32 {
    match stat {
        StatType::CritChance => 4.0,
        StatType::CritMultiplier => 2.0,
        StatType::PhysicalDamage | StatType::FireDamage | StatType::ColdDamage |
        StatType::LightningDamage | StatType::PoisonDamage => 3.0,
        StatType::LifeMax | StatType::ManaMax | StatType::StaminaMax => 1.5,
        StatType::PhysicalArmor | StatType::MagicArmor => 1.2,
        StatType::CooldownReduction | StatType::ResourceCostReduction => 5.0,
        StatType::Strength | StatType::Dexterity | StatType::Intelligence |
        StatType::Vitality | StatType::Endurance => 2.5,
        _ => 1.0,
    }
}

// ============================================================
// AFFIX SYSTEM
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AffixType {
    Prefix,
    Suffix,
    Implicit,
}

#[derive(Debug, Clone)]
pub struct AffixDefinition {
    pub id: u64,
    pub name: String,
    pub affix_type: AffixType,
    pub modifier_template: StatModifier,
    pub flat_min: f32,
    pub flat_max: f32,
    pub percent_min: f32,
    pub percent_max: f32,
    pub item_level_requirement: u32,
    pub weight: f32,
    pub applicable_categories: Vec<ItemCategory>,
    pub conflicts_with: Vec<u64>, // affix IDs that cannot coexist
    pub synergies_with: Vec<u64>, // affix IDs that grant bonus if co-present
    pub synergy_bonus: f32,
    pub tier: u32, // affix tier 1 = worst, 5 = best
}

impl AffixDefinition {
    pub fn roll_flat_value(&self, t: f32) -> f32 {
        // t is normalized [0,1] roll
        self.flat_min + (self.flat_max - self.flat_min) * t
    }

    pub fn roll_percent_value(&self, t: f32) -> f32 {
        self.percent_min + (self.percent_max - self.percent_min) * t
    }

    pub fn applies_to(&self, category: ItemCategory) -> bool {
        self.applicable_categories.contains(&category)
    }

    pub fn has_conflict(&self, other_id: u64) -> bool {
        self.conflicts_with.contains(&other_id)
    }

    pub fn has_synergy(&self, other_id: u64) -> bool {
        self.synergies_with.contains(&other_id)
    }

    pub fn scaled_weight(&self, item_level: u32) -> f32 {
        if item_level < self.item_level_requirement { return 0.0; }
        let level_delta = (item_level - self.item_level_requirement) as f32;
        // Weight decays as newer tiers become available: exponential decay
        self.weight * (-0.05 * level_delta).exp().max(0.1)
    }
}

#[derive(Debug, Clone)]
pub struct AffixPool {
    pub definitions: Vec<AffixDefinition>,
}

impl AffixPool {
    pub fn new() -> Self {
        AffixPool { definitions: Vec::new() }
    }

    pub fn add(&mut self, def: AffixDefinition) {
        self.definitions.push(def);
    }

    pub fn eligible(&self, category: ItemCategory, item_level: u32, affix_type: AffixType) -> Vec<&AffixDefinition> {
        self.definitions.iter().filter(|d|
            d.affix_type == affix_type &&
            d.applies_to(category) &&
            d.scaled_weight(item_level) > 0.0
        ).collect()
    }

    pub fn build_alias_table_for(&self, eligible: &[&AffixDefinition], item_level: u32) -> AliasTable {
        let weights: Vec<f64> = eligible.iter().map(|d| d.scaled_weight(item_level) as f64).collect();
        AliasTable::new(&weights)
    }

    pub fn check_conflicts(&self, selected: &[u64]) -> bool {
        for &id_a in selected {
            if let Some(def) = self.definitions.iter().find(|d| d.id == id_a) {
                for &id_b in selected {
                    if id_a != id_b && def.has_conflict(id_b) {
                        return true; // conflict found
                    }
                }
            }
        }
        false
    }

    pub fn compute_synergy_bonus(&self, selected_ids: &[u64]) -> f32 {
        let mut total_bonus = 0.0f32;
        for &id in selected_ids {
            if let Some(def) = self.definitions.iter().find(|d| d.id == id) {
                for &other_id in selected_ids {
                    if id != other_id && def.has_synergy(other_id) {
                        total_bonus += def.synergy_bonus;
                    }
                }
            }
        }
        total_bonus
    }
}

// ============================================================
// CRAFTING SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct CraftingIngredient {
    pub item_id: u64,
    pub quantity: u32,
    pub quality_threshold: Option<f32>,
    pub consumed: bool,
}

#[derive(Debug, Clone)]
pub struct CraftingStation {
    pub id: u64,
    pub name: String,
    pub tier: u32,
    pub allowed_categories: Vec<ItemCategory>,
}

#[derive(Debug, Clone)]
pub struct SkillRequirement {
    pub skill_name: String,
    pub required_level: u32,
    pub consumed_xp: u32,
}

#[derive(Debug, Clone)]
pub struct QualityOutcome {
    pub quality_value: f32,
    pub weight: f32,
    pub label: String,
}

impl QualityOutcome {
    pub fn standard_outcomes() -> Vec<Self> {
        vec![
            QualityOutcome { quality_value: 0.6, weight: 20.0, label: "Poor".to_string() },
            QualityOutcome { quality_value: 0.8, weight: 40.0, label: "Normal".to_string() },
            QualityOutcome { quality_value: 1.0, weight: 25.0, label: "Superior".to_string() },
            QualityOutcome { quality_value: 1.1, weight: 10.0, label: "Masterwork".to_string() },
            QualityOutcome { quality_value: 1.2, weight: 4.0,  label: "Flawless".to_string() },
            QualityOutcome { quality_value: 1.5, weight: 1.0,  label: "Legendary".to_string() },
        ]
    }
}

#[derive(Debug, Clone)]
pub struct RecipeDefinition {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub ingredients: Vec<CraftingIngredient>,
    pub output_item_id: u64,
    pub output_count: u32,
    pub required_station: CraftingStation,
    pub skill_requirements: Vec<SkillRequirement>,
    pub base_success_chance: f32,
    pub skill_bonus_per_level: f32,   // success chance increase per skill level
    pub quality_outcomes: Vec<QualityOutcome>,
    pub byproducts: Vec<(u64, u32, f32)>, // (item_id, count, chance)
    pub crafting_time_seconds: f32,
    pub unlocked_by_default: bool,
    pub unlock_source: Option<String>,
    pub experience_reward: HashMap<String, u32>,
}

impl RecipeDefinition {
    pub fn success_probability(&self, skill_level: u32) -> f32 {
        let base = self.base_success_chance;
        let bonus = self.skill_bonus_per_level * skill_level as f32;
        (base + bonus).min(0.98).max(0.02)
    }

    pub fn expected_quality(&self, skill_level: u32) -> f32 {
        // Weighted average quality, biased toward higher tiers by skill
        let skill_bias = (skill_level as f32 / 100.0).min(1.0);
        let total_weight: f32 = self.quality_outcomes.iter().map(|q| q.weight).sum();
        let base_avg: f32 = self.quality_outcomes.iter().map(|q| q.quality_value * q.weight / total_weight).sum();
        let max_quality = self.quality_outcomes.iter().map(|q| q.quality_value).fold(f32::NEG_INFINITY, f32::max);
        base_avg + (max_quality - base_avg) * skill_bias * 0.3
    }

    pub fn crafting_time_with_speed(&self, crafting_speed_multiplier: f32) -> f32 {
        self.crafting_time_seconds / crafting_speed_multiplier.max(0.1)
    }

    pub fn sample_quality(&self, roll: f32) -> f32 {
        // roll is [0, total_weight)
        let total: f32 = self.quality_outcomes.iter().map(|q| q.weight).sum();
        let mut acc = 0.0f32;
        let target = roll * total;
        for q in &self.quality_outcomes {
            acc += q.weight;
            if target <= acc { return q.quality_value; }
        }
        self.quality_outcomes.last().map(|q| q.quality_value).unwrap_or(1.0)
    }

    pub fn validate_ingredients(&self, available: &HashMap<u64, u32>) -> bool {
        for ing in &self.ingredients {
            let have = available.get(&ing.item_id).copied().unwrap_or(0);
            if have < ing.quantity { return false; }
        }
        true
    }
}

// ============================================================
// LOOT TABLES
// ============================================================

#[derive(Debug, Clone)]
pub enum DropCondition {
    Always,
    PlayerLevelRange { min: u32, max: u32 },
    QuestCompleted(String),
    QuestActive(String),
    FactionStanding { faction: String, min_rep: i32 },
    ChancePercent(f32),
    FirstKill,
    DifficultyAbove(u32),
}

impl DropCondition {
    pub fn evaluate(&self, ctx: &LootContext) -> bool {
        match self {
            DropCondition::Always => true,
            DropCondition::PlayerLevelRange { min, max } =>
                ctx.player_level >= *min && ctx.player_level <= *max,
            DropCondition::QuestCompleted(q) => ctx.completed_quests.contains(q),
            DropCondition::QuestActive(q) => ctx.active_quests.contains(q),
            DropCondition::FactionStanding { faction, min_rep } =>
                ctx.faction_standings.get(faction).copied().unwrap_or(0) >= *min_rep,
            DropCondition::ChancePercent(c) => ctx.random_float < *c / 100.0,
            DropCondition::FirstKill => ctx.first_kill,
            DropCondition::DifficultyAbove(d) => ctx.difficulty >= *d,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LootContext {
    pub player_level: u32,
    pub magic_find: f32,
    pub completed_quests: HashSet<String>,
    pub active_quests: HashSet<String>,
    pub faction_standings: HashMap<String, i32>,
    pub random_float: f32,
    pub first_kill: bool,
    pub difficulty: u32,
}

impl LootContext {
    pub fn default() -> Self {
        LootContext {
            player_level: 1,
            magic_find: 0.0,
            completed_quests: HashSet::new(),
            active_quests: HashSet::new(),
            faction_standings: HashMap::new(),
            random_float: 0.5,
            first_kill: false,
            difficulty: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LootEntry {
    pub item_id: u64,
    pub weight: f32,
    pub min_quantity: u32,
    pub max_quantity: u32,
    pub condition: DropCondition,
    pub guaranteed: bool,
    pub rarity_override: Option<Rarity>,
}

impl LootEntry {
    pub fn simple(item_id: u64, weight: f32) -> Self {
        LootEntry {
            item_id,
            weight,
            min_quantity: 1,
            max_quantity: 1,
            condition: DropCondition::Always,
            guaranteed: false,
            rarity_override: None,
        }
    }

    pub fn roll_quantity(&self, t: f32) -> u32 {
        if self.min_quantity == self.max_quantity { return self.min_quantity; }
        let range = self.max_quantity - self.min_quantity;
        self.min_quantity + (t * range as f32) as u32
    }
}

// Negative binomial distribution for drop count
fn negative_binomial_sample(r: f32, p: f32, u: f32) -> u32 {
    // PMF: P(X=k) = C(k+r-1, k) * (1-p)^r * p^k
    // Use CDF inversion
    let mut cdf = 0.0f64;
    let p64 = p as f64;
    let r64 = r as f64;
    let u64 = u as f64;
    for k in 0..=100u32 {
        let k64 = k as f64;
        // log(C(k+r-1, k)) = lgamma(k+r) - lgamma(r) - lgamma(k+1)
        let log_binom = lgamma(k64 + r64) - lgamma(r64) - lgamma(k64 + 1.0);
        let pmf = (log_binom + r64 * (1.0 - p64).ln() + k64 * p64.ln()).exp();
        cdf += pmf;
        if cdf >= u64 { return k; }
    }
    50 // fallback
}

fn lgamma(x: f64) -> f64 {
    // Stirling's approximation for lgamma
    if x <= 0.0 { return 0.0; }
    if x < 0.5 {
        return std::f64::consts::PI.ln() - (std::f64::consts::PI * x).sin().ln() - lgamma(1.0 - x);
    }
    let x = x - 1.0;
    let tmp = x + 7.5;
    let ser: f64 = 0.999999999999997092
        + 57.1562356658629235 / (x + 1.0)
        - 59.5979603554754912 / (x + 2.0)
        + 14.1360979747417471 / (x + 3.0)
        - 0.491913816097620199 / (x + 4.0)
        + 0.339946499848118887e-4 / (x + 5.0)
        + 0.465236289270485756e-4 / (x + 6.0)
        - 0.983744753048795646e-4 / (x + 7.0)
        + 0.158088703224912494e-3 / (x + 8.0)
        - 0.210264441724104883e-3 / (x + 9.0)
        + 0.217439618115212643e-3 / (x + 10.0)
        - 0.164318106536763890e-3 / (x + 11.0)
        + 0.844182239838527433e-4 / (x + 12.0)
        - 0.261908384015814087e-4 / (x + 13.0)
        + 0.368991826595316234e-5 / (x + 14.0);
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * tmp.ln() - tmp + ser.ln()
}

#[derive(Debug, Clone)]
pub struct LootTable {
    pub id: u64,
    pub name: String,
    pub entries: Vec<LootEntry>,
    pub drop_count_distribution: DropCountDistribution,
    pub magic_find_scaling: f32, // extra weight per 1% magic find
}

#[derive(Debug, Clone)]
pub enum DropCountDistribution {
    Fixed(u32),
    Uniform { min: u32, max: u32 },
    NegativeBinomial { r: f32, p: f32 },
}

impl DropCountDistribution {
    pub fn sample(&self, u: f32) -> u32 {
        match self {
            DropCountDistribution::Fixed(n) => *n,
            DropCountDistribution::Uniform { min, max } => {
                min + (u * (max - min + 1) as f32) as u32
            }
            DropCountDistribution::NegativeBinomial { r, p } => {
                negative_binomial_sample(*r, *p, u)
            }
        }
    }
}

impl LootTable {
    pub fn roll_drops(&self, ctx: &LootContext, random_values: &[f32]) -> Vec<(u64, u32)> {
        let mut results: Vec<(u64, u32)> = Vec::new();
        let mut rv_index = 0usize;
        let next_r = |idx: &mut usize| -> f32 {
            let v = if *idx < random_values.len() { random_values[*idx] } else { 0.5 };
            *idx += 1; v
        };

        // Guaranteed drops first
        for entry in &self.entries {
            if entry.guaranteed && entry.condition.evaluate(ctx) {
                let qty = entry.roll_quantity(next_r(&mut rv_index));
                results.push((entry.item_id, qty));
            }
        }

        // Determine drop count
        let drop_count = self.drop_count_distribution.sample(next_r(&mut rv_index));

        // Build alias table from eligible non-guaranteed entries
        let eligible: Vec<&LootEntry> = self.entries.iter().filter(|e|
            !e.guaranteed && e.condition.evaluate(ctx)
        ).collect();

        if eligible.is_empty() { return results; }

        let magic_find_multiplier = 1.0 + ctx.magic_find * self.magic_find_scaling / 100.0;
        let weights: Vec<f64> = eligible.iter().map(|e| {
            (e.weight * magic_find_multiplier) as f64
        }).collect();
        let alias = AliasTable::new(&weights);

        for _ in 0..drop_count {
            let u1 = next_r(&mut rv_index) as f64;
            let u2 = next_r(&mut rv_index) as f64;
            let idx = alias.sample(u1, u2);
            let entry = eligible[idx];
            let qty = entry.roll_quantity(next_r(&mut rv_index));
            results.push((entry.item_id, qty));
        }

        results
    }
}

// ============================================================
// ECONOMY SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct PriceElasticity {
    pub base_demand: f32,
    pub demand_slope: f32,   // price change per unit of supply/demand ratio
    pub min_price_ratio: f32,
    pub max_price_ratio: f32,
}

impl PriceElasticity {
    pub fn price_ratio(&self, current_supply: f32, current_demand: f32) -> f32 {
        if current_supply <= 0.0 { return self.max_price_ratio; }
        let ratio = current_demand / current_supply;
        let price_ratio = 1.0 + self.demand_slope * (ratio - 1.0);
        price_ratio.clamp(self.min_price_ratio, self.max_price_ratio)
    }
}

#[derive(Debug, Clone)]
pub struct VendorInventoryEntry {
    pub item_id: u64,
    pub stock: u32,
    pub max_stock: u32,
    pub restock_rate: f32, // units per in-game hour
    pub current_price_multiplier: f32,
    pub special_offer: bool,
    pub offer_expiry: f32,
}

impl VendorInventoryEntry {
    pub fn restock_tick(&mut self, hours_elapsed: f32) {
        let new_stock = self.stock as f32 + self.restock_rate * hours_elapsed;
        self.stock = (new_stock as u32).min(self.max_stock);
    }

    pub fn effective_buy_price(&self, base_price: u64, vendor_markup: f32) -> u64 {
        let multiplier = self.current_price_multiplier * vendor_markup;
        if self.special_offer {
            (base_price as f32 * multiplier * 0.75) as u64
        } else {
            (base_price as f32 * multiplier) as u64
        }
    }
}

#[derive(Debug, Clone)]
pub struct VendorDefinition {
    pub id: u64,
    pub name: String,
    pub faction: String,
    pub inventory: Vec<VendorInventoryEntry>,
    pub buy_markup: f32,
    pub sell_markup: f32,
    pub barter_ratio: f32,
    pub refresh_interval_hours: f32,
    pub last_refresh_time: f32,
    pub price_elasticity: PriceElasticity,
    pub supply: f32,
    pub demand: f32,
}

impl VendorDefinition {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        VendorDefinition {
            id,
            name: name.into(),
            faction: "Neutral".to_string(),
            inventory: Vec::new(),
            buy_markup: 2.5,
            sell_markup: 0.25,
            barter_ratio: 0.8,
            refresh_interval_hours: 24.0,
            last_refresh_time: 0.0,
            price_elasticity: PriceElasticity {
                base_demand: 1.0,
                demand_slope: 0.5,
                min_price_ratio: 0.5,
                max_price_ratio: 3.0,
            },
            supply: 1.0,
            demand: 1.0,
        }
    }

    pub fn dynamic_price_ratio(&self) -> f32 {
        self.price_elasticity.price_ratio(self.supply, self.demand)
    }

    pub fn buy_price(&self, item: &ItemDefinition) -> u64 {
        let rarity_config = item.rarity_config();
        let price = item.base_price as f32
            * self.buy_markup
            * rarity_config.vendor_price_multiplier
            * self.dynamic_price_ratio();
        price as u64
    }

    pub fn sell_price(&self, item: &ItemDefinition) -> u64 {
        let rarity_config = item.rarity_config();
        let price = item.base_price as f32
            * self.sell_markup
            * rarity_config.vendor_price_multiplier;
        price as u64
    }

    pub fn barter_value(&self, item: &ItemDefinition) -> u64 {
        let price = self.buy_price(item) as f32 * self.barter_ratio;
        price as u64
    }

    pub fn refresh_inventory(&mut self, current_time: f32) {
        if current_time - self.last_refresh_time >= self.refresh_interval_hours {
            let hours = current_time - self.last_refresh_time;
            for entry in &mut self.inventory {
                entry.restock_tick(hours);
            }
            self.last_refresh_time = current_time;
        }
    }

    pub fn adjust_supply_demand(&mut self, transaction: EconomyTransaction) {
        match transaction {
            EconomyTransaction::PlayerBought { quantity } => {
                self.supply -= quantity as f32 * 0.1;
                self.demand += quantity as f32 * 0.05;
            }
            EconomyTransaction::PlayerSold { quantity } => {
                self.supply += quantity as f32 * 0.1;
                self.demand -= quantity as f32 * 0.03;
            }
        }
        self.supply = self.supply.max(0.1);
        self.demand = self.demand.max(0.1);
    }
}

#[derive(Debug, Clone)]
pub enum EconomyTransaction {
    PlayerBought { quantity: u32 },
    PlayerSold { quantity: u32 },
}

// ============================================================
// EQUIPMENT SLOTS
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipmentSlot {
    Head,
    Neck,
    LeftShoulder,
    RightShoulder,
    Chest,
    Back,
    Wrists,
    Hands,
    Waist,
    Legs,
    Feet,
    MainHand,
    OffHand,
    Ring1,
    Ring2,
    Trinket1,
    Trinket2,
}

impl EquipmentSlot {
    pub fn all() -> &'static [EquipmentSlot] {
        &[
            EquipmentSlot::Head,
            EquipmentSlot::Neck,
            EquipmentSlot::LeftShoulder,
            EquipmentSlot::RightShoulder,
            EquipmentSlot::Chest,
            EquipmentSlot::Back,
            EquipmentSlot::Wrists,
            EquipmentSlot::Hands,
            EquipmentSlot::Waist,
            EquipmentSlot::Legs,
            EquipmentSlot::Feet,
            EquipmentSlot::MainHand,
            EquipmentSlot::OffHand,
            EquipmentSlot::Ring1,
            EquipmentSlot::Ring2,
            EquipmentSlot::Trinket1,
            EquipmentSlot::Trinket2,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            EquipmentSlot::Head => "Head",
            EquipmentSlot::Neck => "Neck",
            EquipmentSlot::LeftShoulder => "Left Shoulder",
            EquipmentSlot::RightShoulder => "Right Shoulder",
            EquipmentSlot::Chest => "Chest",
            EquipmentSlot::Back => "Back",
            EquipmentSlot::Wrists => "Wrists",
            EquipmentSlot::Hands => "Hands",
            EquipmentSlot::Waist => "Waist",
            EquipmentSlot::Legs => "Legs",
            EquipmentSlot::Feet => "Feet",
            EquipmentSlot::MainHand => "Main Hand",
            EquipmentSlot::OffHand => "Off Hand",
            EquipmentSlot::Ring1 => "Ring 1",
            EquipmentSlot::Ring2 => "Ring 2",
            EquipmentSlot::Trinket1 => "Trinket 1",
            EquipmentSlot::Trinket2 => "Trinket 2",
        }
    }

    pub fn compatible_categories(&self) -> &'static [ItemCategory] {
        match self {
            EquipmentSlot::Head => &[ItemCategory::Helmet, ItemCategory::Hood, ItemCategory::Crown],
            EquipmentSlot::Neck => &[ItemCategory::Amulet],
            EquipmentSlot::LeftShoulder | EquipmentSlot::RightShoulder => &[],
            EquipmentSlot::Chest => &[ItemCategory::Chestplate, ItemCategory::Robe, ItemCategory::Leather],
            EquipmentSlot::Back => &[ItemCategory::Cloak],
            EquipmentSlot::Wrists => &[],
            EquipmentSlot::Hands => &[ItemCategory::Gauntlets, ItemCategory::Gloves],
            EquipmentSlot::Waist => &[ItemCategory::Belt],
            EquipmentSlot::Legs => &[ItemCategory::Greaves, ItemCategory::Leggings, ItemCategory::Pants],
            EquipmentSlot::Feet => &[ItemCategory::Boots, ItemCategory::Sabatons],
            EquipmentSlot::MainHand => &[
                ItemCategory::Sword, ItemCategory::Greatsword, ItemCategory::Dagger,
                ItemCategory::Axe, ItemCategory::Greataxe, ItemCategory::Mace,
                ItemCategory::Hammer, ItemCategory::Spear, ItemCategory::Polearm,
                ItemCategory::Bow, ItemCategory::Crossbow, ItemCategory::Gun,
                ItemCategory::Rifle, ItemCategory::Staff, ItemCategory::Wand,
            ],
            EquipmentSlot::OffHand => &[
                ItemCategory::Shield, ItemCategory::Buckler, ItemCategory::TowerShield,
                ItemCategory::Orb, ItemCategory::Tome, ItemCategory::Dagger,
            ],
            EquipmentSlot::Ring1 | EquipmentSlot::Ring2 => &[ItemCategory::Ring],
            EquipmentSlot::Trinket1 | EquipmentSlot::Trinket2 => &[],
        }
    }

    pub fn is_compatible(&self, category: ItemCategory) -> bool {
        let compat = self.compatible_categories();
        if compat.is_empty() {
            // Shoulders/Wrists/Trinkets accept special slot items
            return false;
        }
        compat.contains(&category)
    }

    pub fn visual_attachment_bone(&self) -> &'static str {
        match self {
            EquipmentSlot::Head => "head_attach",
            EquipmentSlot::Neck => "neck_attach",
            EquipmentSlot::LeftShoulder => "shoulder_l_attach",
            EquipmentSlot::RightShoulder => "shoulder_r_attach",
            EquipmentSlot::Chest => "chest_attach",
            EquipmentSlot::Back => "back_attach",
            EquipmentSlot::Wrists => "wrist_attach",
            EquipmentSlot::Hands => "hand_attach",
            EquipmentSlot::Waist => "waist_attach",
            EquipmentSlot::Legs => "leg_attach",
            EquipmentSlot::Feet => "foot_attach",
            EquipmentSlot::MainHand => "weapon_r_attach",
            EquipmentSlot::OffHand => "weapon_l_attach",
            EquipmentSlot::Ring1 => "ring_r_attach",
            EquipmentSlot::Ring2 => "ring_l_attach",
            EquipmentSlot::Trinket1 | EquipmentSlot::Trinket2 => "trinket_attach",
        }
    }
}

// ============================================================
// SET BONUS SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct SetBonus {
    pub pieces_required: u32,
    pub description: String,
    pub modifiers: Vec<StatModifier>,
}

#[derive(Debug, Clone)]
pub struct ItemSet {
    pub id: u64,
    pub name: String,
    pub item_ids: Vec<u64>,
    pub bonuses: Vec<SetBonus>, // sorted by pieces_required
    pub color: Vec4,
}

impl ItemSet {
    pub fn count_equipped_pieces(&self, equipped: &HashMap<EquipmentSlot, u64>) -> u32 {
        let equipped_ids: HashSet<&u64> = equipped.values().collect();
        self.item_ids.iter().filter(|id| equipped_ids.contains(id)).count() as u32
    }

    pub fn active_bonuses(&self, equipped: &HashMap<EquipmentSlot, u64>) -> Vec<&SetBonus> {
        let count = self.count_equipped_pieces(equipped);
        self.bonuses.iter().filter(|b| count >= b.pieces_required).collect()
    }

    pub fn next_bonus(&self, equipped: &HashMap<EquipmentSlot, u64>) -> Option<(&SetBonus, u32)> {
        let count = self.count_equipped_pieces(equipped);
        self.bonuses.iter()
            .filter(|b| b.pieces_required > count)
            .min_by_key(|b| b.pieces_required)
            .map(|b| (b, b.pieces_required - count))
    }
}

// ============================================================
// CONTAINER / GRID INVENTORY
// ============================================================

#[derive(Debug, Clone)]
pub struct GridCell {
    pub item_instance_id: Option<u64>,
    pub is_anchor: bool, // top-left cell of item occupancy
}

#[derive(Debug, Clone)]
pub struct ItemInstance {
    pub instance_id: u64,
    pub definition_id: u64,
    pub quantity: u32,
    pub durability: f32,
    pub quality: f32,
    pub affixes: Vec<(u64, f32)>, // (affix_id, rolled_value)
    pub position: Vec2,           // grid position (col, row)
    pub rotated: bool,
    pub custom_name: Option<String>,
    pub loot_filter_override: Option<u8>,
}

impl ItemInstance {
    pub fn new(instance_id: u64, definition_id: u64) -> Self {
        ItemInstance {
            instance_id,
            definition_id,
            quantity: 1,
            durability: 100.0,
            quality: 1.0,
            affixes: Vec::new(),
            position: Vec2::ZERO,
            rotated: false,
            custom_name: None,
            loot_filter_override: None,
        }
    }

    pub fn effective_width(&self, def: &ItemDefinition) -> u8 {
        if self.rotated { def.height } else { def.width }
    }

    pub fn effective_height(&self, def: &ItemDefinition) -> u8 {
        if self.rotated { def.width } else { def.height }
    }

    pub fn display_name<'a>(&'a self, def: &'a ItemDefinition) -> &'a str {
        self.custom_name.as_deref().unwrap_or(&def.name)
    }

    pub fn durability_fraction(&self) -> f32 {
        if self.durability <= 0.0 { return 0.0; }
        (self.durability / 100.0).min(1.0)
    }

    pub fn is_broken(&self) -> bool {
        self.durability <= 0.0
    }

    pub fn repair_cost(&self, def: &ItemDefinition) -> u64 {
        let missing = (100.0 - self.durability).max(0.0);
        let fraction = missing / 100.0;
        (def.base_price as f32 * 0.1 * fraction * self.quality) as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BagFilterType {
    All,
    Weapons,
    Armor,
    Consumables,
    Materials,
    QuestItems,
    Custom,
}

#[derive(Debug, Clone)]
pub struct Container {
    pub id: u64,
    pub name: String,
    pub grid_width: u32,
    pub grid_height: u32,
    pub cells: Vec<Vec<GridCell>>,
    pub items: HashMap<u64, ItemInstance>,
    pub max_weight: f32,
    pub current_weight: f32,
    pub max_volume: f32,
    pub current_volume: f32,
    pub filter_type: BagFilterType,
    pub is_locked: bool,
}

impl Container {
    pub fn new(id: u64, name: impl Into<String>, width: u32, height: u32) -> Self {
        let cells: Vec<Vec<GridCell>> = (0..height).map(|_| {
            (0..width).map(|_| GridCell { item_instance_id: None, is_anchor: false }).collect()
        }).collect();
        Container {
            id,
            name: name.into(),
            grid_width: width,
            grid_height: height,
            cells,
            items: HashMap::new(),
            max_weight: 100.0,
            current_weight: 0.0,
            max_volume: 50.0,
            current_volume: 0.0,
            filter_type: BagFilterType::All,
            is_locked: false,
        }
    }

    pub fn can_place(&self, col: u32, row: u32, w: u32, h: u32) -> bool {
        if col + w > self.grid_width || row + h > self.grid_height { return false; }
        for r in row..row+h {
            for c in col..col+w {
                if self.cells[r as usize][c as usize].item_instance_id.is_some() {
                    return false;
                }
            }
        }
        true
    }

    pub fn find_free_slot(&self, w: u32, h: u32) -> Option<(u32, u32)> {
        for row in 0..self.grid_height {
            for col in 0..self.grid_width {
                if self.can_place(col, row, w, h) {
                    return Some((col, row));
                }
            }
        }
        None
    }

    pub fn place_item(&mut self, inst: ItemInstance, col: u32, row: u32, def: &ItemDefinition) -> bool {
        let w = inst.effective_width(def) as u32;
        let h = inst.effective_height(def) as u32;
        if !self.can_place(col, row, w, h) { return false; }
        let iid = inst.instance_id;
        for r in row..row+h {
            for c in col..col+w {
                self.cells[r as usize][c as usize].item_instance_id = Some(iid);
                self.cells[r as usize][c as usize].is_anchor = r == row && c == col;
            }
        }
        self.current_weight += def.weight * inst.quantity as f32;
        self.current_volume += def.volume * inst.quantity as f32;
        self.items.insert(iid, inst);
        true
    }

    pub fn remove_item(&mut self, instance_id: u64, def: &ItemDefinition) -> Option<ItemInstance> {
        if let Some(inst) = self.items.remove(&instance_id) {
            // Clear cells
            for row in &mut self.cells {
                for cell in row {
                    if cell.item_instance_id == Some(instance_id) {
                        cell.item_instance_id = None;
                        cell.is_anchor = false;
                    }
                }
            }
            self.current_weight -= def.weight * inst.quantity as f32;
            self.current_volume -= def.volume * inst.quantity as f32;
            Some(inst)
        } else {
            None
        }
    }

    pub fn sort_by_category(&mut self, defs: &HashMap<u64, ItemDefinition>) {
        // Collect all items, clear grid, re-place sorted
        let mut all_items: Vec<ItemInstance> = self.items.drain().map(|(_, v)| v).collect();
        // Clear all cells
        for row in &mut self.cells {
            for cell in row {
                cell.item_instance_id = None;
                cell.is_anchor = false;
            }
        }
        self.current_weight = 0.0;
        self.current_volume = 0.0;

        // Sort by category display name then by item name
        all_items.sort_by(|a, b| {
            let def_a = defs.get(&a.definition_id);
            let def_b = defs.get(&b.definition_id);
            match (def_a, def_b) {
                (Some(da), Some(db)) => {
                    let cat_cmp = da.category.display_name().cmp(db.category.display_name());
                    if cat_cmp == std::cmp::Ordering::Equal {
                        da.name.cmp(&db.name)
                    } else {
                        cat_cmp
                    }
                }
                _ => std::cmp::Ordering::Equal,
            }
        });

        // Re-place items
        for mut item in all_items {
            if let Some(def) = defs.get(&item.definition_id) {
                let w = item.effective_width(def) as u32;
                let h = item.effective_height(def) as u32;
                if let Some((col, row)) = self.find_free_slot(w, h) {
                    item.position = Vec2::new(col as f32, row as f32);
                    self.place_item(item, col, row, def);
                }
            }
        }
    }

    pub fn weight_fraction(&self) -> f32 {
        if self.max_weight <= 0.0 { return 0.0; }
        (self.current_weight / self.max_weight).min(1.0)
    }

    pub fn is_overweight(&self) -> bool {
        self.current_weight > self.max_weight
    }
}

// ============================================================
// PROCEDURAL ITEM GENERATION
// ============================================================

const WEAPON_PREFIXES: &[&str] = &[
    "Ancient", "Brutal", "Cursed", "Dark", "Eldritch", "Fierce", "Gleaming",
    "Hallowed", "Infernal", "Jagged", "Keen", "Lethal", "Malevolent", "Null",
    "Ominous", "Piercing", "Quick", "Raging", "Savage", "Tempered", "Unholy",
    "Vicious", "Wicked", "Xeric", "Yearning", "Zealous", "Ashen", "Blazing",
    "Cracked", "Dreadful", "Enchanted", "Frostbitten", "Grim", "Haunted",
    "Icy", "Jade", "Kingsguard", "Luminous", "Molten", "Necrotic", "Obsidian",
    "Plagued", "Quenched", "Runed", "Spectral", "Twisted", "Umbral", "Volcanic",
    "Withered", "Ancient", "Baleful", "Corroded", "Defiled", "Energized",
    "Flickering", "Glacial", "Hexed", "Incandescent", "Jinxed", "Knotted",
    "Lightforged", "Mystic", "Nightshroud", "Oathbound", "Pristine", "Quivering",
    "Radiant", "Shadowed", "Thornbound", "Undying", "Venomous", "Whispering",
    "Xerograph", "Yearlong", "Zenith", "Abyss-touched", "Broken", "Chiseled",
    "Darkened", "Edged", "Flaming", "Gutted", "Heavy", "Inlaid", "Jagged",
    "Kindled", "Laced", "Marred", "Noble", "Outlaw", "Pitted", "Quenched",
    "Refined", "Serrated", "Tainted", "Uncanny", "Valiant", "Warped",
    "Xiphoid", "Yearner", "Zeal-wrought", "Accursed", "Battered", "Carven",
    "Dawnforged", "Ebonsteel", "Frosted", "Gilded", "Honed", "Ironclad",
    "Jeweled", "Krakenforged", "Lightweave", "Moonforged", "Nightfall",
    "Opal-set", "Petrified", "Quicksilver", "Runesteel", "Stormforged",
    "Titanforged", "Ursa", "Vindictive", "Wolfbound", "Xenos", "Yarrow",
    "Zenithborn", "Abyssal", "Bonded", "Condensed", "Deepsteel", "Ember",
    "Frostfire", "Gossamer", "Heartseeker", "Infused", "Judgement",
    "Kingslayer", "Lifesteel", "Mournful", "Noxious", "Outrider",
    "Purified", "Quickened", "Ruinous", "Soulbound", "Terrifying",
    "Undead", "Voidshard", "Wardancer", "Xenolith", "Yearning", "Zarathos",
    "Brightedge", "Crystalforged", "Desolation", "Ember-quenched", "Foxfire",
    "Galeforce", "Hexbound", "Ironweave", "Jaded", "Killerwhale", "Lodestar",
    "Mystical", "Nameless", "Otherworldly", "Parallax", "Quietus", "Riftborn",
    "Sanguine", "Tempestborn", "Unified", "Vesper", "Wanderer", "Xeric",
    "Yggdrasil", "Zero-point", "Aetheric", "Boneshatter", "Crimsonblade",
    "Dawnbreaker", "Eclipseborn", "Frostmantle", "Grimoire", "Hallowsteel",
    "Ironheart", "Jadeblade", "Knightfall", "Lifeguard", "Moonshatter",
    "Nightsong", "Oathkeeper", "Platinumcore", "Quickdraw", "Ragnarok",
    "Soulfire", "Thunderstrike", "Umbragen", "Veilwalker", "Worldender",
    "Xiphos", "Yieldless", "Zodiac", "Arcanite", "Bloodmire", "Cometfall",
    "Duskwalker", "Edenfall", "Fateweaver", "Ghoststeel", "Hellforged",
    "Immortal", "Jadefire", "Keysteel", "Lavaborn", "Moonfire", "Netherforged",
    "Overcharge", "Pyroborn", "Radiantfall", "Starborn", "Tidecaller",
    "Unbreakable", "Voidwalker", "Worldbreaker", "Xeric", "Youthful", "Zenith",
];

const WEAPON_BASES: &[&str] = &[
    "Sword", "Blade", "Edge", "Cutter", "Slicer", "Cleaver", "Chopper", "Striker",
    "Biter", "Fang", "Talon", "Claw", "Piercer", "Impaler", "Skewer", "Lancer",
    "Prodder", "Stabber", "Poker", "Pricker", "Hammer", "Crusher", "Pounder",
    "Smasher", "Basher", "Bludgeon", "Club", "Maul", "Mallet", "Staff",
    "Rod", "Wand", "Scepter", "Orb", "Focus", "Conduit", "Channel", "Vessel",
    "Bow", "Stringer", "Launcher", "Thrower", "Shooter", "Axe", "Hatchet",
    "Cleaver", "Chopper", "Hewer", "Splitter", "Dagger", "Stiletto", "Dirk",
    "Kris", "Kukri", "Macuahuitl", "Naginata", "Odachi", "Partisan", "Ranseur",
    "Sabre", "Tulwar", "Ulfberht", "Vouge", "Warstaff", "Xiphos", "Yatagan",
    "Zweihander", "Arming Sword", "Bastard Sword", "Cavalry Sabre", "Dueling Blade",
    "Executioner", "Falchion", "Gladius", "Hauptmann", "Infantry Sword",
    "Jagged Shard", "Khopesh", "Longsword", "Montante", "Nightblade",
    "Officer's Sword", "Pallash", "Quickblade", "Reaper", "Shortsword",
    "Tomahawk", "Urgash", "Viking Sword", "Wakizashi", "Xenoblade",
    "Yellowback", "Zulfiqar", "Arcane Staff", "Bonechill Scepter", "Curse Wand",
    "Death Rod", "Ethereal Focus", "Fire Staff", "Grimoire Staff", "Hex Wand",
    "Ironwood Staff", "Jeweled Scepter", "Knotted Staff", "Lich Staff",
    "Moonstaff", "Null Rod", "Orb of Storms", "Plague Staff", "Quicksilver Rod",
    "Runestaff", "Shadow Wand", "Tome Staff", "Umbra Focus", "Venom Rod",
    "Wraith Staff", "Xenomancer's Orb", "Yew Wand", "Zephyr Staff",
    "Ancient Blade", "Battle Cleaver", "Champion's Edge", "Dragon Fang",
    "Elder Sword", "Flame Saber", "Ghost Blade", "Hero's Sword", "Ice Blade",
    "Judge's Sword", "Knight's Edge", "Lord's Blade", "Master Sword",
    "Noble Sword", "Old Blade", "Paladin's Sword", "Quest Blade", "Royal Edge",
    "Soldier's Sword", "Temple Blade", "Unknown Sword", "Veteran's Blade",
    "War Sword", "Xeric Blade", "Youth Cutter", "Zenith Sword",
    "Armageddon Blade", "Blood Sword", "Chaos Edge", "Death Sword",
    "Eternal Blade", "Fury Edge", "Glory Sword", "Honor Blade", "Inferno Sword",
    "Justice Edge", "King's Sword", "Legacy Blade", "Miracle Edge", "Night Sword",
    "Omega Blade", "Prophet's Sword", "Quiet Edge", "Ruin Sword", "Sacred Blade",
    "Truth Edge", "Unity Sword", "Victory Blade", "Warlord's Edge",
    "Xtreme Blade", "Yearning Sword", "Zero Blade", "Apex Sword",
    "Blazing Edge", "Crystal Sword", "Diamond Blade", "Emerald Edge",
    "Frost Sword", "Gale Blade", "Holy Sword", "Iron Edge", "Jade Sword",
    "Kraken Blade", "Lightning Edge", "Moon Sword", "Nova Blade",
    "Obsidian Edge", "Prism Sword", "Quartz Blade", "Rainbow Edge",
    "Solar Sword", "Thunder Blade", "Ultraviolet Edge", "Void Sword",
    "Wind Blade", "Xenon Edge", "Yellow Sword", "Zeal Blade",
];

const WEAPON_SUFFIXES: &[&str] = &[
    "of Agony", "of Blight", "of Carnage", "of Destruction", "of Eternity",
    "of Flame", "of Glory", "of Havoc", "of Ice", "of Justice",
    "of Killing", "of Light", "of Madness", "of Night", "of Oblivion",
    "of Pestilence", "of Quake", "of Ruin", "of Storms", "of Thunder",
    "of Undeath", "of Vengeance", "of War", "of Xerxes", "of Yesterday",
    "of Zenith", "of the Abyss", "of the Bear", "of the Cosmos", "of the Deep",
    "of the Eagle", "of the Fox", "of the Gods", "of the Hawk", "of the Infinite",
    "of the Jaguar", "of the Kingdom", "of the Lion", "of the Moon", "of the Night",
    "of the Oracle", "of the Phoenix", "of the Queen", "of the Raven", "of the Sun",
    "of the Tiger", "of the Undying", "of the Void", "of the Wolf", "of the Wyrm",
    "of Xeric Power", "of Yearning", "of Zeal", "of Absolute Power", "of Balance",
    "of Chaos", "of Dominion", "of Elemental Force", "of Finality", "of Grace",
    "of Heroism", "of Immortality", "of Judgment", "of Kinship", "of Lore",
    "of Mastery", "of Nobility", "of Order", "of Purity", "of Quickness",
    "of Resolve", "of Supremacy", "of Time", "of Unity", "of Valor",
    "of Wrath", "of Xenomorph", "of Youth", "of Zealotry", "of Acrimony",
    "of Bliss", "of Cruelty", "of Daemonhood", "of Embers", "of Frost",
    "of Gales", "of Hatred", "of Inferno", "of Judgement", "of Kindness",
    "of Lunacy", "of Mirth", "of Nightfall", "of Oaths", "of Plague",
    "of Quickening", "of Rage", "of Sorrow", "of Torment", "of Utter Darkness",
    "of Virtue", "of Whispers", "of Xtreme Velocity", "of Yearlong Battle",
    "of Zero Mercy", "of Ancient Times", "of Battle Rage", "of Cold Fury",
    "of Deadly Precision", "of Endless War", "of Fallen Angels", "of Great Victory",
    "of Howling Wind", "of Inner Strength", "of Just Cause", "of Known Legends",
    "of Lost Souls", "of Mighty Blows", "of Noble Purpose", "of Old Magic",
    "of Perfect Form", "of Quiet Power", "of Righteous Fury", "of Sacred Ground",
    "of True Purpose", "of Undying Will", "of Vast Power", "of Warrior's Heart",
    "of Xeric Sands", "of Yearning Hearts", "of Zenith Power", "of Arcane Fury",
    "of Blazing Hearts", "of Crystal Clarity", "of Dark Purpose", "of Endless Night",
    "of Frozen Fury", "of Golden Light", "of Hidden Power", "of Iron Will",
    "of Jade Winds", "of King's Fury", "of Lasting Power", "of Moonlight",
    "of Nameless Dread", "of Otherworldly Power", "of Perfect Balance",
    "of Quick Strikes", "of Relentless Fury", "of Shining Light", "of True Power",
    "of Undying Power", "of Vast Destruction", "of World's Edge", "of Xenophobia",
    "of Youthful Energy", "of Zealous Fury", "of Absolute Destruction",
    "of Bright Horizons", "of Celestial Power", "of Deep Magic", "of Eternal Flame",
    "of Forgotten Lore", "of Grand Purpose", "of Hidden Depths", "of Ironbound Will",
    "of Just Rewards", "of King's Legacy", "of Lost Kingdoms", "of Many Battles",
    "of Noble Heritage", "of Old Kingdoms", "of Primal Fury", "of Quick Death",
    "of Red Fury", "of Starborn Power", "of Timeless Grace", "of Ultimate Power",
    "of Void Energy", "of War's End", "of Xenon Power", "of Younger Days",
    "of Zero Tolerance", "of Absolute Glory", "of Battle Hardened",
    "of Celestial Light", "of Dragon's Heart", "of Eternal Battle",
    "of Furious Heart", "of Ghost Steel", "of Hallowed Ground",
    "of Infinite Power", "of Just Battle", "of King's Honor",
];

fn generate_item_name(prefix_idx: usize, base_idx: usize, suffix_idx: usize) -> String {
    let prefix = WEAPON_PREFIXES[prefix_idx % WEAPON_PREFIXES.len()];
    let base = WEAPON_BASES[base_idx % WEAPON_BASES.len()];
    let suffix = WEAPON_SUFFIXES[suffix_idx % WEAPON_SUFFIXES.len()];
    format!("{} {} {}", prefix, base, suffix)
}

#[derive(Debug, Clone)]
pub struct ItemGenerationParams {
    pub item_level: u32,
    pub category: ItemCategory,
    pub rarity: Option<Rarity>,
    pub force_affixes: Vec<u64>,
    pub seed: u64,
}

fn lcg_next(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *seed
}

fn lcg_f32(seed: &mut u64) -> f32 {
    (lcg_next(seed) >> 32) as f32 / u32::MAX as f32
}

pub fn generate_item(params: &ItemGenerationParams, affix_pool: &AffixPool, defs: &mut HashMap<u64, ItemDefinition>) -> ItemInstance {
    let mut seed = params.seed;
    let mut item_id = lcg_next(&mut seed);
    // Avoid collision
    while defs.contains_key(&item_id) { item_id = lcg_next(&mut seed); }

    let rarity = params.rarity.unwrap_or_else(|| {
        let roll = lcg_f32(&mut seed);
        rarity_from_roll(roll)
    });

    let rarity_cfg = RarityConfig::for_rarity(rarity);
    let num_affixes = rarity_cfg.min_affixes + (lcg_f32(&mut seed) * (rarity_cfg.max_affixes - rarity_cfg.min_affixes + 1) as f32) as u32;
    let quality = rarity_cfg.quality_min + lcg_f32(&mut seed) * (rarity_cfg.quality_max - rarity_cfg.quality_min);

    let prefix_idx = lcg_next(&mut seed) as usize;
    let base_idx = lcg_next(&mut seed) as usize;
    let suffix_idx = lcg_next(&mut seed) as usize;
    let name = generate_item_name(prefix_idx, base_idx, suffix_idx);

    let mut def = ItemDefinition::new(item_id, &name, params.category);
    def.rarity = rarity;
    def.item_level = params.item_level;
    def.required_level = (params.item_level as f32 * 0.8).floor() as u32;
    def.base_price = compute_base_price(params.item_level, rarity);

    // Roll weapon damage if applicable
    if params.category.is_weapon() {
        let base_dps = 5.0 + params.item_level as f32 * 2.0 * rarity_cfg.stat_multiplier;
        let speed = 0.8 + lcg_f32(&mut seed) * 0.8;
        let avg_dmg = base_dps / speed;
        let variance = 0.2 + lcg_f32(&mut seed) * 0.3;
        let min_dmg = avg_dmg * (1.0 - variance);
        let max_dmg = avg_dmg * (1.0 + variance);
        def.damage_ranges.push(DamageRange {
            min_damage: min_dmg,
            max_damage: max_dmg,
            damage_type: DamageType::Physical,
            crit_multiplier: 1.5,
        });
        def.attack_speed = speed;
    }

    // Roll armor if applicable
    if params.category.is_armor() {
        let base_armor = 10.0 + params.item_level as f32 * 3.0 * rarity_cfg.stat_multiplier;
        def.armor_values.physical_armor = base_armor * quality;
    }

    // Roll explicit affixes
    let mut selected_affix_ids: Vec<u64> = params.force_affixes.clone();
    let eligible_prefixes = affix_pool.eligible(params.category, params.item_level, AffixType::Prefix);
    let eligible_suffixes = affix_pool.eligible(params.category, params.item_level, AffixType::Suffix);

    let half_affixes = num_affixes / 2;
    roll_affixes_into(&mut def.explicit_modifiers, &mut selected_affix_ids, &eligible_prefixes, half_affixes, &mut seed, affix_pool);
    roll_affixes_into(&mut def.explicit_modifiers, &mut selected_affix_ids, &eligible_suffixes, num_affixes - half_affixes, &mut seed, affix_pool);

    defs.insert(item_id, def);

    let mut instance = ItemInstance::new(lcg_next(&mut seed), item_id);
    instance.quality = quality;
    instance
}

fn roll_affixes_into(
    modifiers: &mut Vec<StatModifier>,
    selected_ids: &mut Vec<u64>,
    eligible: &[&AffixDefinition],
    count: u32,
    seed: &mut u64,
    pool: &AffixPool,
) {
    if eligible.is_empty() { return; }
    let item_level = 1; // dummy, actual is in eligible filter
    let weights: Vec<f64> = eligible.iter().map(|d| d.weight as f64).collect();
    if weights.is_empty() { return; }
    let alias = AliasTable::new(&weights);

    let mut attempts = 0u32;
    let mut rolled = 0u32;
    while rolled < count && attempts < 50 {
        attempts += 1;
        let u1 = lcg_f32(seed) as f64;
        let u2 = lcg_f32(seed) as f64;
        let idx = alias.sample(u1, u2);
        let affix = eligible[idx];
        // Check conflicts
        if selected_ids.iter().any(|&id| affix.has_conflict(id)) { continue; }
        if selected_ids.contains(&affix.id) { continue; }

        let roll_t = lcg_f32(seed);
        let mut m = affix.modifier_template.clone();
        m.flat_value = affix.roll_flat_value(roll_t);
        m.percent_value = affix.roll_percent_value(roll_t);
        modifiers.push(m);
        selected_ids.push(affix.id);
        rolled += 1;
    }
}

fn rarity_from_roll(roll: f32) -> Rarity {
    // Cumulative probability thresholds (must sum to 1)
    const MYTHIC_THRESH: f32 = 0.0002;
    const LEGENDARY_THRESH: f32 = 0.0017;
    const EPIC_THRESH: f32 = 0.009;
    const RARE_THRESH: f32 = 0.041;
    const UNCOMMON_THRESH: f32 = 0.200;
    if roll < MYTHIC_THRESH { Rarity::Mythic }
    else if roll < LEGENDARY_THRESH { Rarity::Legendary }
    else if roll < EPIC_THRESH { Rarity::Epic }
    else if roll < RARE_THRESH { Rarity::Rare }
    else if roll < UNCOMMON_THRESH { Rarity::Uncommon }
    else { Rarity::Common }
}

fn compute_base_price(item_level: u32, rarity: Rarity) -> u64 {
    let level_factor = (item_level as f64).powf(1.8);
    let rarity_mul = match rarity {
        Rarity::Common => 1.0,
        Rarity::Uncommon => 3.0,
        Rarity::Rare => 10.0,
        Rarity::Epic => 50.0,
        Rarity::Legendary => 250.0,
        Rarity::Mythic => 1000.0,
    };
    (level_factor * rarity_mul * 10.0) as u64
}

// ============================================================
// POINT-BUY BALANCE VALIDATION
// ============================================================

#[derive(Debug, Clone)]
pub struct BalanceReport {
    pub total_budget: f32,
    pub budget_per_slot: f32,
    pub budget_level_expected: f32,
    pub is_overpowered: bool,
    pub is_underpowered: bool,
    pub violations: Vec<String>,
    pub stat_breakdown: Vec<(String, f32)>,
}

pub fn validate_item_balance(item: &ItemDefinition) -> BalanceReport {
    let total_budget = item.total_stat_budget();
    let expected_budget = budget_expectation(item.item_level, item.rarity);
    let tolerance = expected_budget * 0.25;

    let mut violations = Vec::new();
    if total_budget > expected_budget + tolerance {
        violations.push(format!(
            "Item is overpowered: budget {:.1} exceeds expected {:.1} + tolerance {:.1}",
            total_budget, expected_budget, tolerance
        ));
    }
    if total_budget < expected_budget - tolerance {
        violations.push(format!(
            "Item is underpowered: budget {:.1} below expected {:.1} - tolerance {:.1}",
            total_budget, expected_budget, tolerance
        ));
    }

    let mut stat_breakdown = Vec::new();
    for m in &item.implicit_modifiers {
        let cost = stat_budget_cost(&m.stat) * (m.flat_value.abs() + m.percent_value.abs() * 2.0);
        stat_breakdown.push((format!("{} (implicit)", m.stat.display_name()), cost));
    }
    for m in &item.explicit_modifiers {
        let cost = stat_budget_cost(&m.stat) * (m.flat_value.abs() + m.percent_value.abs() * 2.0);
        stat_breakdown.push((format!("{} (explicit)", m.stat.display_name()), cost));
    }

    BalanceReport {
        total_budget,
        budget_per_slot: total_budget / EquipmentSlot::all().len() as f32,
        budget_level_expected: expected_budget,
        is_overpowered: total_budget > expected_budget + tolerance,
        is_underpowered: total_budget < expected_budget - tolerance,
        violations,
        stat_breakdown,
    }
}

fn budget_expectation(item_level: u32, rarity: Rarity) -> f32 {
    let base = 5.0 + item_level as f32 * 1.5;
    let rarity_mul = match rarity {
        Rarity::Common => 1.0,
        Rarity::Uncommon => 1.5,
        Rarity::Rare => 2.5,
        Rarity::Epic => 4.0,
        Rarity::Legendary => 7.0,
        Rarity::Mythic => 12.0,
    };
    base * rarity_mul
}

// ============================================================
// SEARCH / FILTER SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemFilter {
    pub name_query: String,
    pub categories: HashSet<ItemCategory>,
    pub rarities: HashSet<Rarity>,
    pub min_item_level: u32,
    pub max_item_level: u32,
    pub min_dps: f32,
    pub max_dps: f32,
    pub required_stats: Vec<(StatType, f32)>,
    pub show_stackable_only: bool,
    pub show_equippable_only: bool,
}

impl ItemFilter {
    pub fn all() -> Self {
        ItemFilter {
            name_query: String::new(),
            categories: HashSet::new(),
            rarities: HashSet::new(),
            min_item_level: 0,
            max_item_level: u32::MAX,
            min_dps: 0.0,
            max_dps: f32::MAX,
            required_stats: Vec::new(),
            show_stackable_only: false,
            show_equippable_only: false,
        }
    }

    pub fn matches(&self, item: &ItemDefinition) -> bool {
        // Name filter
        if !self.name_query.is_empty() {
            let query_lower = self.name_query.to_lowercase();
            if !item.name.to_lowercase().contains(&query_lower) &&
               !item.description.to_lowercase().contains(&query_lower) {
                return false;
            }
        }

        // Category filter
        if !self.categories.is_empty() && !self.categories.contains(&item.category) {
            return false;
        }

        // Rarity filter
        if !self.rarities.is_empty() && !self.rarities.contains(&item.rarity) {
            return false;
        }

        // Item level range
        if item.item_level < self.min_item_level || item.item_level > self.max_item_level {
            return false;
        }

        // DPS filter
        let dps = item.total_damage_per_second();
        if dps < self.min_dps || dps > self.max_dps {
            // Only apply dps filter for weapons
            if item.category.is_weapon() { return false; }
        }

        // Stack filter
        if self.show_stackable_only && item.stack_size <= 1 {
            return false;
        }

        // Equippable filter
        if self.show_equippable_only && !item.category.is_weapon() && !item.category.is_armor() && !item.category.is_accessory() {
            return false;
        }

        // Required stat filter
        for (stat, min_val) in &self.required_stats {
            let total: f32 = item.explicit_modifiers.iter()
                .filter(|m| &m.stat == stat)
                .map(|m| m.flat_value + m.percent_value)
                .sum();
            if total < *min_val { return false; }
        }

        true
    }
}

// ============================================================
// IMPORT / EXPORT
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemExportRecord {
    pub id: u64,
    pub name: String,
    pub category: String,
    pub rarity: String,
    pub item_level: u32,
    pub base_price: u64,
    pub weight: f32,
    pub width: u8,
    pub height: u8,
    pub stack_size: u32,
    pub dps: f32,
    pub armor: f32,
    pub modifiers_csv: String,
}

impl ItemExportRecord {
    pub fn from_definition(def: &ItemDefinition) -> Self {
        let mods_csv: String = def.explicit_modifiers.iter().map(|m| {
            format!("{}:{:.1}f:{:.1}p", m.stat.display_name(), m.flat_value, m.percent_value)
        }).collect::<Vec<_>>().join(";");

        ItemExportRecord {
            id: def.id,
            name: def.name.clone(),
            category: def.category.display_name().to_string(),
            rarity: RarityConfig::for_rarity(def.rarity).display_name().to_string(),
            item_level: def.item_level,
            base_price: def.base_price,
            weight: def.weight,
            width: def.width,
            height: def.height,
            stack_size: def.stack_size,
            dps: def.total_damage_per_second(),
            armor: def.armor_values.physical_armor,
            modifiers_csv: mods_csv,
        }
    }

    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{:.2},{:.1},\"{}\"",
            self.id, self.name, self.category, self.rarity,
            self.item_level, self.base_price, self.weight,
            self.width, self.height, self.stack_size,
            self.dps, self.armor, self.modifiers_csv
        )
    }

    pub fn csv_header() -> &'static str {
        "id,name,category,rarity,item_level,base_price,weight,width,height,stack_size,dps,armor,modifiers"
    }
}

pub fn export_database_csv(defs: &HashMap<u64, ItemDefinition>) -> String {
    let mut lines = vec![ItemExportRecord::csv_header().to_string()];
    let mut sorted: Vec<&ItemDefinition> = defs.values().collect();
    sorted.sort_by_key(|d| d.id);
    for def in sorted {
        lines.push(ItemExportRecord::from_definition(def).to_csv_row());
    }
    lines.join("\n")
}

// ============================================================
// ITEM DATABASE
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemDatabase {
    pub definitions: HashMap<u64, ItemDefinition>,
    pub recipes: HashMap<u64, RecipeDefinition>,
    pub loot_tables: HashMap<u64, LootTable>,
    pub item_sets: HashMap<u64, ItemSet>,
    pub affix_pool: AffixPool,
    pub vendors: HashMap<u64, VendorDefinition>,
    pub next_id: u64,
}

impl ItemDatabase {
    pub fn new() -> Self {
        ItemDatabase {
            definitions: HashMap::new(),
            recipes: HashMap::new(),
            loot_tables: HashMap::new(),
            item_sets: HashMap::new(),
            affix_pool: AffixPool::new(),
            vendors: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_definition(&mut self, def: ItemDefinition) {
        self.definitions.insert(def.id, def);
    }

    pub fn add_recipe(&mut self, recipe: RecipeDefinition) {
        self.recipes.insert(recipe.id, recipe);
    }

    pub fn add_loot_table(&mut self, lt: LootTable) {
        self.loot_tables.insert(lt.id, lt);
    }

    pub fn add_set(&mut self, set: ItemSet) {
        self.item_sets.insert(set.id, set);
    }

    pub fn get_definition(&self, id: u64) -> Option<&ItemDefinition> {
        self.definitions.get(&id)
    }

    pub fn search(&self, filter: &ItemFilter) -> Vec<&ItemDefinition> {
        let mut results: Vec<&ItemDefinition> = self.definitions.values()
            .filter(|d| filter.matches(d))
            .collect();
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    pub fn bulk_update_prices(&mut self, factor: f32) {
        for def in self.definitions.values_mut() {
            def.base_price = (def.base_price as f32 * factor) as u64;
        }
    }

    pub fn stats_summary(&self) -> DatabaseStats {
        let mut by_rarity: HashMap<Rarity, u32> = HashMap::new();
        let mut by_category: HashMap<String, u32> = HashMap::new();
        let mut total_value: u64 = 0;
        for def in self.definitions.values() {
            *by_rarity.entry(def.rarity).or_insert(0) += 1;
            *by_category.entry(def.category.display_name().to_string()).or_insert(0) += 1;
            total_value += def.base_price;
        }
        DatabaseStats {
            total_items: self.definitions.len(),
            total_recipes: self.recipes.len(),
            total_loot_tables: self.loot_tables.len(),
            total_sets: self.item_sets.len(),
            by_rarity,
            by_category,
            total_catalog_value: total_value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_items: usize,
    pub total_recipes: usize,
    pub total_loot_tables: usize,
    pub total_sets: usize,
    pub by_rarity: HashMap<Rarity, u32>,
    pub by_category: HashMap<String, u32>,
    pub total_catalog_value: u64,
}

// ============================================================
// INVENTORY EDITOR STATE
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTab {
    ItemBrowser,
    ItemEditor,
    RecipeEditor,
    LootTableEditor,
    SetEditor,
    VendorEditor,
    AffixEditor,
    Statistics,
}

#[derive(Debug, Clone)]
pub struct ItemEditorState {
    pub current_item: Option<ItemDefinition>,
    pub is_dirty: bool,
    pub validation_errors: Vec<String>,
    pub balance_report: Option<BalanceReport>,
    pub undo_stack: VecDeque<ItemDefinition>,
    pub redo_stack: VecDeque<ItemDefinition>,
}

impl ItemEditorState {
    pub fn new() -> Self {
        ItemEditorState {
            current_item: None,
            is_dirty: false,
            validation_errors: Vec::new(),
            balance_report: None,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    pub fn open_item(&mut self, item: ItemDefinition) {
        self.current_item = Some(item);
        self.is_dirty = false;
        self.validation_errors.clear();
        self.balance_report = None;
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
        if let Some(ref item) = self.current_item {
            if self.undo_stack.len() >= 50 { self.undo_stack.pop_front(); }
            self.undo_stack.push_back(item.clone());
            self.redo_stack.clear();
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop_back() {
            if let Some(ref current) = self.current_item {
                if self.redo_stack.len() >= 50 { self.redo_stack.pop_front(); }
                self.redo_stack.push_back(current.clone());
            }
            self.current_item = Some(prev);
            self.is_dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop_back() {
            if let Some(ref current) = self.current_item {
                if self.undo_stack.len() >= 50 { self.undo_stack.pop_front(); }
                self.undo_stack.push_back(current.clone());
            }
            self.current_item = Some(next);
            self.is_dirty = true;
        }
    }

    pub fn validate(&mut self) {
        if let Some(ref item) = self.current_item {
            self.validation_errors = item.validate();
            self.balance_report = Some(validate_item_balance(item));
        }
    }
}

#[derive(Debug, Clone)]
pub struct BrowserState {
    pub filter: ItemFilter,
    pub filtered_ids: Vec<u64>,
    pub selected_ids: HashSet<u64>,
    pub sort_column: BrowserSortColumn,
    pub sort_ascending: bool,
    pub scroll_offset: f32,
    pub row_height: f32,
    pub visible_rows: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSortColumn {
    Name,
    Category,
    Rarity,
    ItemLevel,
    DPS,
    Price,
}

impl BrowserState {
    pub fn new() -> Self {
        BrowserState {
            filter: ItemFilter::all(),
            filtered_ids: Vec::new(),
            selected_ids: HashSet::new(),
            sort_column: BrowserSortColumn::Name,
            sort_ascending: true,
            scroll_offset: 0.0,
            row_height: 22.0,
            visible_rows: 30,
        }
    }

    pub fn refresh(&mut self, db: &ItemDatabase) {
        let results = db.search(&self.filter);
        self.filtered_ids = results.iter().map(|d| d.id).collect();
        self.sort_results(db);
    }

    pub fn sort_results(&mut self, db: &ItemDatabase) {
        let col = self.sort_column;
        let asc = self.sort_ascending;
        self.filtered_ids.sort_by(|&a, &b| {
            let da = db.definitions.get(&a);
            let db_def = db.definitions.get(&b);
            match (da, db_def) {
                (Some(da), Some(db)) => {
                    let cmp = match col {
                        BrowserSortColumn::Name => da.name.cmp(&db.name),
                        BrowserSortColumn::Category => da.category.display_name().cmp(db.category.display_name()),
                        BrowserSortColumn::Rarity => da.rarity.cmp(&db.rarity),
                        BrowserSortColumn::ItemLevel => da.item_level.cmp(&db.item_level),
                        BrowserSortColumn::DPS => da.total_damage_per_second().partial_cmp(&db.total_damage_per_second()).unwrap_or(std::cmp::Ordering::Equal),
                        BrowserSortColumn::Price => da.base_price.cmp(&db.base_price),
                    };
                    if asc { cmp } else { cmp.reverse() }
                }
                _ => std::cmp::Ordering::Equal,
            }
        });
    }

    pub fn visible_item_ids(&self) -> &[u64] {
        let start = (self.scroll_offset / self.row_height) as usize;
        let end = (start + self.visible_rows as usize).min(self.filtered_ids.len());
        &self.filtered_ids[start..end]
    }

    pub fn total_height(&self) -> f32 {
        self.filtered_ids.len() as f32 * self.row_height
    }

    pub fn select_all(&mut self) {
        for &id in &self.filtered_ids {
            self.selected_ids.insert(id);
        }
    }

    pub fn deselect_all(&mut self) {
        self.selected_ids.clear();
    }

    pub fn toggle_select(&mut self, id: u64) {
        if self.selected_ids.contains(&id) {
            self.selected_ids.remove(&id);
        } else {
            self.selected_ids.insert(id);
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecipeEditorState {
    pub current_recipe: Option<RecipeDefinition>,
    pub is_dirty: bool,
    pub preview_quality: f32,
    pub preview_skill_level: u32,
    pub preview_success_chance: f32,
    pub preview_expected_quality: f32,
}

impl RecipeEditorState {
    pub fn new() -> Self {
        RecipeEditorState {
            current_recipe: None,
            is_dirty: false,
            preview_quality: 1.0,
            preview_skill_level: 50,
            preview_success_chance: 0.75,
            preview_expected_quality: 1.0,
        }
    }

    pub fn update_preview(&mut self) {
        if let Some(ref recipe) = self.current_recipe {
            self.preview_success_chance = recipe.success_probability(self.preview_skill_level);
            self.preview_expected_quality = recipe.expected_quality(self.preview_skill_level);
        }
    }
}

#[derive(Debug, Clone)]
pub struct LootTableEditorState {
    pub current_table: Option<LootTable>,
    pub is_dirty: bool,
    pub simulate_context: LootContext,
    pub simulated_drops: Vec<(u64, u32)>,
    pub simulation_count: u32,
    pub drop_frequency: HashMap<u64, u32>,
}

impl LootTableEditorState {
    pub fn new() -> Self {
        LootTableEditorState {
            current_table: None,
            is_dirty: false,
            simulate_context: LootContext::default(),
            simulated_drops: Vec::new(),
            simulation_count: 1000,
            drop_frequency: HashMap::new(),
        }
    }

    pub fn run_simulation(&mut self) {
        self.drop_frequency.clear();
        if let Some(ref table) = self.current_table {
            let mut seed: u64 = 12345678;
            for _ in 0..self.simulation_count {
                let mut rvs = Vec::new();
                for _ in 0..20 {
                    rvs.push(lcg_f32(&mut seed));
                }
                let ctx = self.simulate_context.clone();
                let drops = table.roll_drops(&ctx, &rvs);
                for (item_id, qty) in drops {
                    *self.drop_frequency.entry(item_id).or_insert(0) += qty;
                }
            }
        }
    }

    pub fn drop_rate_percent(&self, item_id: u64) -> f32 {
        let count = self.drop_frequency.get(&item_id).copied().unwrap_or(0);
        count as f32 / self.simulation_count as f32 * 100.0
    }
}

#[derive(Debug, Clone)]
pub struct AffixEditorState {
    pub current_affix: Option<AffixDefinition>,
    pub is_dirty: bool,
    pub preview_rolls: Vec<f32>,
}

impl AffixEditorState {
    pub fn new() -> Self {
        AffixEditorState {
            current_affix: None,
            is_dirty: false,
            preview_rolls: vec![0.0, 0.25, 0.5, 0.75, 1.0],
        }
    }

    pub fn preview_values(&self) -> Vec<(f32, f32, f32)> {
        if let Some(ref affix) = self.current_affix {
            self.preview_rolls.iter().map(|&t| {
                (t, affix.roll_flat_value(t), affix.roll_percent_value(t))
            }).collect()
        } else {
            Vec::new()
        }
    }
}

// ============================================================
// MAIN INVENTORY EDITOR
// ============================================================

#[derive(Debug)]
pub struct InventoryEditor {
    pub database: ItemDatabase,
    pub active_tab: EditorTab,
    pub item_editor: ItemEditorState,
    pub browser: BrowserState,
    pub recipe_editor: RecipeEditorState,
    pub loot_table_editor: LootTableEditorState,
    pub affix_editor: AffixEditorState,
    pub generation_params: ItemGenerationParams,
    pub last_generated_instance: Option<ItemInstance>,
    pub window_size: Vec2,
    pub panel_split: f32,
    pub status_message: String,
    pub status_timer: f32,
    pub bulk_edit_field: String,
    pub bulk_edit_value: String,
    pub preview_item_id: Option<u64>,
    pub clipboard_item: Option<ItemDefinition>,
    pub search_history: VecDeque<String>,
    pub tag_registry: HashMap<String, Vec<u64>>,
    pub recently_modified: VecDeque<u64>,
}

impl InventoryEditor {
    pub fn new() -> Self {
        let db = ItemDatabase::new();
        InventoryEditor {
            database: db,
            active_tab: EditorTab::ItemBrowser,
            item_editor: ItemEditorState::new(),
            browser: BrowserState::new(),
            recipe_editor: RecipeEditorState::new(),
            loot_table_editor: LootTableEditorState::new(),
            affix_editor: AffixEditorState::new(),
            generation_params: ItemGenerationParams {
                item_level: 1,
                category: ItemCategory::Sword,
                rarity: None,
                force_affixes: Vec::new(),
                seed: 42,
            },
            last_generated_instance: None,
            window_size: Vec2::new(1280.0, 720.0),
            panel_split: 0.35,
            status_message: String::new(),
            status_timer: 0.0,
            bulk_edit_field: String::new(),
            bulk_edit_value: String::new(),
            preview_item_id: None,
            clipboard_item: None,
            search_history: VecDeque::new(),
            tag_registry: HashMap::new(),
            recently_modified: VecDeque::new(),
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_timer = 3.0;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.status_timer > 0.0 {
            self.status_timer = (self.status_timer - dt).max(0.0);
        }
    }

    pub fn open_item_by_id(&mut self, id: u64) {
        if let Some(def) = self.database.definitions.get(&id).cloned() {
            self.item_editor.open_item(def);
            self.active_tab = EditorTab::ItemEditor;
            self.recently_modified.push_front(id);
            if self.recently_modified.len() > 20 { self.recently_modified.pop_back(); }
        }
    }

    pub fn save_current_item(&mut self) -> bool {
        if let Some(item) = self.item_editor.current_item.clone() {
            self.item_editor.validate();
            if !self.item_editor.validation_errors.is_empty() {
                self.set_status(format!("Cannot save: {} validation error(s)", self.item_editor.validation_errors.len()));
                return false;
            }
            let id = item.id;
            self.database.definitions.insert(id, item);
            self.item_editor.is_dirty = false;
            self.set_status("Item saved successfully.");
            self.browser.refresh(&self.database);
            true
        } else {
            false
        }
    }

    pub fn create_new_item(&mut self, category: ItemCategory) {
        let id = self.database.alloc_id();
        let def = ItemDefinition::new(id, "New Item", category);
        self.item_editor.open_item(def);
        self.active_tab = EditorTab::ItemEditor;
    }

    pub fn delete_selected_items(&mut self) {
        let to_delete: Vec<u64> = self.browser.selected_ids.iter().cloned().collect();
        for id in &to_delete {
            self.database.definitions.remove(id);
        }
        self.browser.selected_ids.clear();
        self.browser.refresh(&self.database);
        self.set_status(format!("Deleted {} item(s).", to_delete.len()));
    }

    pub fn duplicate_item(&mut self, id: u64) {
        if let Some(def) = self.database.definitions.get(&id).cloned() {
            let new_id = self.database.alloc_id();
            let mut new_def = def;
            new_def.id = new_id;
            new_def.name = format!("{} (Copy)", new_def.name);
            self.database.definitions.insert(new_id, new_def);
            self.browser.refresh(&self.database);
            self.set_status("Item duplicated.");
        }
    }

    pub fn generate_random_item(&mut self) {
        let params = self.generation_params.clone();
        let inst = generate_item(&params, &self.database.affix_pool, &mut self.database.definitions);
        self.last_generated_instance = Some(inst);
        self.browser.refresh(&self.database);
        self.set_status("Random item generated.");
        // Increment seed for next generation
        self.generation_params.seed = self.generation_params.seed.wrapping_add(1);
    }

    pub fn copy_item(&mut self) {
        if let Some(ref item) = self.item_editor.current_item {
            self.clipboard_item = Some(item.clone());
            self.set_status("Item copied to clipboard.");
        }
    }

    pub fn paste_item(&mut self) {
        if let Some(item) = self.clipboard_item.clone() {
            let new_id = self.database.alloc_id();
            let mut new_item = item;
            new_item.id = new_id;
            new_item.name = format!("{} (Paste)", new_item.name);
            self.database.definitions.insert(new_id, new_item.clone());
            self.item_editor.open_item(new_item);
            self.browser.refresh(&self.database);
            self.set_status("Item pasted from clipboard.");
        }
    }

    pub fn bulk_update_selected(&mut self, field: &str, value_str: &str) -> usize {
        let ids: Vec<u64> = self.browser.selected_ids.iter().cloned().collect();
        let mut changed = 0usize;
        for id in ids {
            if let Some(def) = self.database.definitions.get_mut(&id) {
                match field {
                    "required_level" => {
                        if let Ok(v) = value_str.parse::<u32>() {
                            def.required_level = v; changed += 1;
                        }
                    }
                    "base_price" => {
                        if let Ok(v) = value_str.parse::<u64>() {
                            def.base_price = v; changed += 1;
                        }
                    }
                    "weight" => {
                        if let Ok(v) = value_str.parse::<f32>() {
                            def.weight = v; changed += 1;
                        }
                    }
                    "rarity" => {
                        match value_str {
                            "Common" => { def.rarity = Rarity::Common; changed += 1; }
                            "Uncommon" => { def.rarity = Rarity::Uncommon; changed += 1; }
                            "Rare" => { def.rarity = Rarity::Rare; changed += 1; }
                            "Epic" => { def.rarity = Rarity::Epic; changed += 1; }
                            "Legendary" => { def.rarity = Rarity::Legendary; changed += 1; }
                            "Mythic" => { def.rarity = Rarity::Mythic; changed += 1; }
                            _ => {}
                        }
                    }
                    "bind_on_pickup" => {
                        match value_str {
                            "true" | "1" => { def.bind_on_pickup = true; changed += 1; }
                            "false" | "0" => { def.bind_on_pickup = false; changed += 1; }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        self.browser.refresh(&self.database);
        self.set_status(format!("Bulk updated {} items.", changed));
        changed
    }

    pub fn export_to_csv(&self) -> String {
        export_database_csv(&self.database.definitions)
    }

    pub fn add_tag_to_item(&mut self, item_id: u64, tag: String) {
        self.tag_registry.entry(tag).or_insert_with(Vec::new).push(item_id);
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<u64> {
        self.tag_registry.get(tag).cloned().unwrap_or_default()
    }

    pub fn search_with_history(&mut self, query: String) {
        if !query.is_empty() {
            self.search_history.push_front(query.clone());
            if self.search_history.len() > 20 { self.search_history.pop_back(); }
            self.search_history.make_contiguous().dedup();
        }
        self.browser.filter.name_query = query;
        self.browser.refresh(&self.database);
    }

    pub fn get_item_tooltip(&self, id: u64) -> Option<ItemTooltip> {
        let def = self.database.definitions.get(&id)?;
        let cfg = RarityConfig::for_rarity(def.rarity);
        let mut lines = Vec::new();
        lines.push((def.name.clone(), cfg.color));
        lines.push((format!("{} | {}", def.category.display_name(), cfg.display_name()), Vec4::new(0.7,0.7,0.7,1.0)));
        lines.push((format!("Item Level: {}", def.item_level), Vec4::new(0.8,0.8,0.8,1.0)));
        if def.category.is_weapon() {
            let dps = def.total_damage_per_second();
            lines.push((format!("DPS: {:.1}", dps), Vec4::new(1.0,0.9,0.5,1.0)));
            for d in &def.damage_ranges {
                lines.push((format!("  {:?}: {:.0}-{:.0}", d.damage_type, d.min_damage, d.max_damage), d.damage_type.color()));
            }
            lines.push((format!("Speed: {:.2}", def.attack_speed), Vec4::new(0.7,0.7,0.7,1.0)));
        }
        if def.armor_values.physical_armor > 0.0 {
            lines.push((format!("Armor: {:.0}", def.armor_values.physical_armor), Vec4::new(0.7,0.9,1.0,1.0)));
        }
        for m in &def.implicit_modifiers {
            let sign = if m.flat_value >= 0.0 { "+" } else { "" };
            lines.push((format!("{}{:.0} {}", sign, m.flat_value, m.stat.display_name()), Vec4::new(0.8,0.8,0.6,1.0)));
        }
        for m in &def.explicit_modifiers {
            let sign = if m.flat_value >= 0.0 { "+" } else { "" };
            let color = if m.flat_value >= 0.0 { Vec4::new(0.5,1.0,0.5,1.0) } else { Vec4::new(1.0,0.5,0.5,1.0) };
            lines.push((format!("{}{:.0} {}", sign, m.flat_value, m.stat.display_name()), color));
        }
        if def.requirements.level > 1 {
            lines.push((format!("Required Level: {}", def.requirements.level), Vec4::new(0.9,0.6,0.3,1.0)));
        }
        if !def.flavor_text.is_empty() {
            lines.push((def.flavor_text.clone(), Vec4::new(0.6,0.6,0.8,1.0)));
        }
        lines.push((format!("Value: {} gold", def.base_price), Vec4::new(1.0,0.85,0.0,1.0)));

        Some(ItemTooltip { lines })
    }

    pub fn layout(&self) -> EditorLayout {
        let left_panel_width = self.window_size.x * self.panel_split;
        let right_panel_width = self.window_size.x - left_panel_width - 2.0;
        let header_height = 40.0;
        let footer_height = 24.0;
        let content_height = self.window_size.y - header_height - footer_height;

        EditorLayout {
            header_rect: Vec4::new(0.0, 0.0, self.window_size.x, header_height),
            left_panel_rect: Vec4::new(0.0, header_height, left_panel_width, content_height),
            right_panel_rect: Vec4::new(left_panel_width + 2.0, header_height, right_panel_width, content_height),
            footer_rect: Vec4::new(0.0, self.window_size.y - footer_height, self.window_size.x, footer_height),
            divider_x: left_panel_width,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemTooltip {
    pub lines: Vec<(String, Vec4)>,
}

#[derive(Debug, Clone)]
pub struct EditorLayout {
    pub header_rect: Vec4,
    pub left_panel_rect: Vec4,
    pub right_panel_rect: Vec4,
    pub footer_rect: Vec4,
    pub divider_x: f32,
}

// ============================================================
// GRID INVENTORY PREVIEW RENDERER (coordinate math)
// ============================================================

#[derive(Debug, Clone)]
pub struct GridRenderer {
    pub cell_size: f32,
    pub origin: Vec2,
    pub padding: f32,
}

impl GridRenderer {
    pub fn new(cell_size: f32, origin: Vec2) -> Self {
        GridRenderer { cell_size, origin, padding: 2.0 }
    }

    pub fn cell_rect(&self, col: u32, row: u32) -> Vec4 {
        let x = self.origin.x + col as f32 * (self.cell_size + self.padding);
        let y = self.origin.y + row as f32 * (self.cell_size + self.padding);
        Vec4::new(x, y, self.cell_size, self.cell_size)
    }

    pub fn item_rect(&self, col: u32, row: u32, width: u32, height: u32) -> Vec4 {
        let x = self.origin.x + col as f32 * (self.cell_size + self.padding);
        let y = self.origin.y + row as f32 * (self.cell_size + self.padding);
        let w = width as f32 * (self.cell_size + self.padding) - self.padding;
        let h = height as f32 * (self.cell_size + self.padding) - self.padding;
        Vec4::new(x, y, w, h)
    }

    pub fn total_size(&self, grid_width: u32, grid_height: u32) -> Vec2 {
        let w = grid_width as f32 * (self.cell_size + self.padding);
        let h = grid_height as f32 * (self.cell_size + self.padding);
        Vec2::new(w, h)
    }

    pub fn hit_test(&self, point: Vec2, grid_width: u32, grid_height: u32) -> Option<(u32, u32)> {
        let rel = point - self.origin;
        if rel.x < 0.0 || rel.y < 0.0 { return None; }
        let col = (rel.x / (self.cell_size + self.padding)) as u32;
        let row = (rel.y / (self.cell_size + self.padding)) as u32;
        if col >= grid_width || row >= grid_height { return None; }
        Some((col, row))
    }

    pub fn snap_to_grid(&self, point: Vec2) -> Vec2 {
        let step = self.cell_size + self.padding;
        let col = (point.x / step).floor();
        let row = (point.y / step).floor();
        Vec2::new(col * step + self.origin.x, row * step + self.origin.y)
    }
}

// ============================================================
// SORTING ALGORITHMS (tetromino bin-packing)
// ============================================================

// Skyline algorithm for 2D bin packing
#[derive(Debug, Clone)]
pub struct SkylinePacker {
    pub width: u32,
    pub skyline: Vec<u32>, // height at each column
}

impl SkylinePacker {
    pub fn new(width: u32) -> Self {
        SkylinePacker { width, skyline: vec![0; width as usize] }
    }

    pub fn find_placement(&self, item_w: u32, item_h: u32) -> Option<(u32, u32)> {
        if item_w > self.width { return None; }
        let max_col = self.width - item_w;
        let mut best: Option<(u32, u32, u32)> = None; // (col, row, cost)

        for start_col in 0..=max_col {
            let max_height = (start_col..start_col + item_w)
                .map(|c| self.skyline[c as usize])
                .max()
                .unwrap_or(0);
            let placement_row = max_height;
            // Cost is placement_row (prefer lowest placements)
            if best.map(|(_, _, c)| placement_row < c).unwrap_or(true) {
                best = Some((start_col, placement_row, placement_row));
            }
        }

        best.map(|(c, r, _)| (c, r))
    }

    pub fn place(&mut self, col: u32, row: u32, item_w: u32, item_h: u32) {
        let new_height = row + item_h;
        for c in col..col + item_w {
            if (c as usize) < self.skyline.len() {
                self.skyline[c as usize] = self.skyline[c as usize].max(new_height);
            }
        }
    }

    pub fn pack_items(grid_width: u32, items: &mut Vec<(u32, u32, u64)>) -> Vec<(u64, u32, u32)> {
        // items = Vec<(width, height, id)>
        // returns Vec<(id, col, row)>
        let mut packer = SkylinePacker::new(grid_width);
        let mut placements = Vec::new();

        // Sort by area desc (larger items first for better packing)
        items.sort_by(|a, b| (b.0 * b.1).cmp(&(a.0 * a.1)));

        for &(w, h, id) in items.iter() {
            if let Some((col, row)) = packer.find_placement(w, h) {
                packer.place(col, row, w, h);
                placements.push((id, col, row));
            }
        }
        placements
    }
}

// ============================================================
// ITEM COMPARISON TOOL
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemComparison {
    pub item_a: ItemDefinition,
    pub item_b: ItemDefinition,
    pub stat_diffs: Vec<StatDiff>,
    pub armor_diff: ArmorDiff,
    pub dps_diff: f32,
    pub price_diff: i64,
    pub weight_diff: f32,
}

#[derive(Debug, Clone)]
pub struct StatDiff {
    pub stat: StatType,
    pub value_a: f32,
    pub value_b: f32,
    pub delta: f32,
    pub is_better_on_b: bool,
}

#[derive(Debug, Clone)]
pub struct ArmorDiff {
    pub physical_armor_delta: f32,
    pub magic_armor_delta: f32,
    pub fire_resist_delta: f32,
    pub cold_resist_delta: f32,
    pub lightning_resist_delta: f32,
    pub poison_resist_delta: f32,
}

pub fn compare_items(a: &ItemDefinition, b: &ItemDefinition) -> ItemComparison {
    let mut stat_map_a: HashMap<String, f32> = HashMap::new();
    let mut stat_map_b: HashMap<String, f32> = HashMap::new();

    for m in &a.explicit_modifiers {
        *stat_map_a.entry(m.stat.display_name().to_string()).or_insert(0.0) += m.flat_value + m.percent_value;
    }
    for m in &b.explicit_modifiers {
        *stat_map_b.entry(m.stat.display_name().to_string()).or_insert(0.0) += m.flat_value + m.percent_value;
    }

    let mut all_stats: HashSet<String> = HashSet::new();
    all_stats.extend(stat_map_a.keys().cloned());
    all_stats.extend(stat_map_b.keys().cloned());

    let mut stat_diffs: Vec<StatDiff> = all_stats.into_iter().filter_map(|stat_name| {
        let va = stat_map_a.get(&stat_name).copied().unwrap_or(0.0);
        let vb = stat_map_b.get(&stat_name).copied().unwrap_or(0.0);
        // Find the StatType enum value — use damage mod as fallback
        let st = StatType::PhysicalDamage; // placeholder, actual lookup would be needed
        Some(StatDiff {
            stat: st,
            value_a: va,
            value_b: vb,
            delta: vb - va,
            is_better_on_b: vb > va,
        })
    }).collect();
    stat_diffs.sort_by(|x, y| y.delta.abs().partial_cmp(&x.delta.abs()).unwrap_or(std::cmp::Ordering::Equal));

    let armor_diff = ArmorDiff {
        physical_armor_delta: b.armor_values.physical_armor - a.armor_values.physical_armor,
        magic_armor_delta: b.armor_values.magic_armor - a.armor_values.magic_armor,
        fire_resist_delta: b.armor_values.fire_resist - a.armor_values.fire_resist,
        cold_resist_delta: b.armor_values.cold_resist - a.armor_values.cold_resist,
        lightning_resist_delta: b.armor_values.lightning_resist - a.armor_values.lightning_resist,
        poison_resist_delta: b.armor_values.poison_resist - a.armor_values.poison_resist,
    };

    ItemComparison {
        item_a: a.clone(),
        item_b: b.clone(),
        stat_diffs,
        armor_diff,
        dps_diff: b.total_damage_per_second() - a.total_damage_per_second(),
        price_diff: b.base_price as i64 - a.base_price as i64,
        weight_diff: b.weight - a.weight,
    }
}

// ============================================================
// STAT CALCULATOR (final stat sheet)
// ============================================================

#[derive(Debug, Clone)]
pub struct CharacterStats {
    pub base_stats: HashMap<String, f32>,
    pub equipped_items: Vec<ItemDefinition>,
    pub active_set_bonuses: Vec<SetBonus>,
}

impl CharacterStats {
    pub fn new() -> Self {
        let mut base_stats = HashMap::new();
        base_stats.insert("strength".to_string(), 10.0);
        base_stats.insert("dexterity".to_string(), 10.0);
        base_stats.insert("intelligence".to_string(), 10.0);
        base_stats.insert("vitality".to_string(), 10.0);
        CharacterStats {
            base_stats,
            equipped_items: Vec::new(),
            active_set_bonuses: Vec::new(),
        }
    }

    pub fn compute_final_stat(&self, stat: &StatType) -> f32 {
        let base = match stat {
            StatType::LifeMax => 100.0 + self.base_stats.get("vitality").copied().unwrap_or(10.0) * 10.0,
            StatType::ManaMax => 50.0 + self.base_stats.get("intelligence").copied().unwrap_or(10.0) * 5.0,
            StatType::PhysicalDamage => self.base_stats.get("strength").copied().unwrap_or(10.0) * 2.0,
            _ => 0.0,
        };

        let mut flat_sum = 0.0f32;
        let mut percent_sum = 0.0f32;
        let mut more_product = 1.0f32;

        for item in &self.equipped_items {
            for m in &item.explicit_modifiers {
                if &m.stat == stat {
                    flat_sum += m.flat_value;
                    percent_sum += m.percent_value;
                    more_product *= m.more_multiplier;
                }
            }
            for m in &item.implicit_modifiers {
                if &m.stat == stat {
                    flat_sum += m.flat_value;
                    percent_sum += m.percent_value;
                    more_product *= m.more_multiplier;
                }
            }
        }

        for bonus in &self.active_set_bonuses {
            for m in &bonus.modifiers {
                if &m.stat == stat {
                    flat_sum += m.flat_value;
                    percent_sum += m.percent_value;
                    more_product *= m.more_multiplier;
                }
            }
        }

        (base + flat_sum) * (1.0 + percent_sum / 100.0) * more_product
    }

    pub fn effective_dps_against(&self, target_armor: f32, target_resist: f32) -> f32 {
        let raw_dps: f32 = self.equipped_items.iter().map(|item| item.total_damage_per_second()).sum();
        let phys_dmg = self.compute_final_stat(&StatType::PhysicalDamage);
        let dmg_multiplier = 1.0 + phys_dmg / 100.0;
        let armor_reduction = target_armor / (target_armor + 300.0);
        raw_dps * dmg_multiplier * (1.0 - armor_reduction) * (1.0 - target_resist / 100.0)
    }
}

// ============================================================
// DEFAULT ITEM DEFINITIONS (starter set)
// ============================================================

pub fn create_starter_item_database() -> ItemDatabase {
    let mut db = ItemDatabase::new();

    // Basic sword
    let sword_id = db.alloc_id();
    let mut sword = ItemDefinition::new(sword_id, "Iron Sword", ItemCategory::Sword);
    sword.item_level = 1;
    sword.required_level = 1;
    sword.base_price = 50;
    sword.weight = 2.5;
    sword.width = 1;
    sword.height = 3;
    sword.attack_speed = 1.2;
    sword.damage_ranges.push(DamageRange {
        min_damage: 5.0,
        max_damage: 12.0,
        damage_type: DamageType::Physical,
        crit_multiplier: 1.5,
    });
    sword.description = "A simple iron sword, reliable and well-balanced.".to_string();
    db.add_definition(sword);

    // Iron helmet
    let helmet_id = db.alloc_id();
    let mut helmet = ItemDefinition::new(helmet_id, "Iron Helmet", ItemCategory::Helmet);
    helmet.item_level = 1;
    helmet.base_price = 40;
    helmet.weight = 3.0;
    helmet.armor_values.physical_armor = 15.0;
    helmet.description = "A sturdy iron helmet.".to_string();
    db.add_definition(helmet);

    // Health potion
    let potion_id = db.alloc_id();
    let mut potion = ItemDefinition::new(potion_id, "Health Potion", ItemCategory::Potion);
    potion.item_level = 1;
    potion.base_price = 20;
    potion.weight = 0.3;
    potion.stack_size = 20;
    potion.implicit_modifiers.push(StatModifier::flat(StatType::LifeMax, 50.0));
    potion.description = "Restores 50 health points.".to_string();
    db.add_definition(potion);

    // Iron ore material
    let ore_id = db.alloc_id();
    let mut ore = ItemDefinition::new(ore_id, "Iron Ore", ItemCategory::Ore);
    ore.item_level = 1;
    ore.base_price = 5;
    ore.weight = 0.5;
    ore.stack_size = 999;
    ore.description = "Raw iron ore, used in smithing.".to_string();
    db.add_definition(ore);

    // Ring with magic find
    let ring_id = db.alloc_id();
    let mut ring = ItemDefinition::new(ring_id, "Ring of Fortune", ItemCategory::Ring);
    ring.item_level = 5;
    ring.rarity = Rarity::Uncommon;
    ring.base_price = 150;
    ring.weight = 0.1;
    ring.explicit_modifiers.push(StatModifier::percent(StatType::MagicFind, 10.0));
    ring.description = "A lucky ring that attracts rare loot.".to_string();
    db.add_definition(ring);

    // Legendary greataxe
    let axe_id = db.alloc_id();
    let mut axe = ItemDefinition::new(axe_id, "Ravager's Greataxe", ItemCategory::Greataxe);
    axe.item_level = 20;
    axe.rarity = Rarity::Legendary;
    axe.base_price = 5000;
    axe.weight = 8.0;
    axe.width = 2;
    axe.height = 4;
    axe.attack_speed = 0.7;
    axe.damage_ranges.push(DamageRange {
        min_damage: 85.0,
        max_damage: 140.0,
        damage_type: DamageType::Physical,
        crit_multiplier: 2.0,
    });
    axe.damage_ranges.push(DamageRange {
        min_damage: 20.0,
        max_damage: 35.0,
        damage_type: DamageType::Fire,
        crit_multiplier: 1.8,
    });
    axe.explicit_modifiers.push(StatModifier::flat(StatType::Strength, 15.0));
    axe.explicit_modifiers.push(StatModifier::percent(StatType::PhysicalDamage, 40.0));
    axe.explicit_modifiers.push(StatModifier::flat(StatType::CritChance, 8.0));
    axe.unique_effect = Some("On kill: gain 10% attack speed for 5s".to_string());
    axe.description = "A massive axe that seems to hunger for combat.".to_string();
    axe.bind_on_pickup = true;
    db.add_definition(axe);

    // Basic recipe
    let sword_recipe_id = db.alloc_id();
    let sword_recipe = RecipeDefinition {
        id: sword_recipe_id,
        name: "Forge Iron Sword".to_string(),
        description: "Create an iron sword at the forge.".to_string(),
        ingredients: vec![
            CraftingIngredient { item_id: ore_id, quantity: 5, quality_threshold: None, consumed: true },
        ],
        output_item_id: sword_id,
        output_count: 1,
        required_station: CraftingStation {
            id: 1,
            name: "Smithing Forge".to_string(),
            tier: 1,
            allowed_categories: vec![ItemCategory::Sword, ItemCategory::Axe, ItemCategory::Hammer],
        },
        skill_requirements: vec![
            SkillRequirement { skill_name: "Smithing".to_string(), required_level: 1, consumed_xp: 0 },
        ],
        base_success_chance: 0.9,
        skill_bonus_per_level: 0.005,
        quality_outcomes: QualityOutcome::standard_outcomes(),
        byproducts: vec![],
        crafting_time_seconds: 5.0,
        unlocked_by_default: true,
        unlock_source: None,
        experience_reward: {
            let mut m = HashMap::new();
            m.insert("Smithing".to_string(), 15);
            m
        },
    };
    db.add_recipe(sword_recipe);

    // Basic loot table
    let loot_id = db.alloc_id();
    let loot_table = LootTable {
        id: loot_id,
        name: "Basic Goblin Loot".to_string(),
        entries: vec![
            LootEntry { item_id: sword_id, weight: 10.0, min_quantity: 1, max_quantity: 1, condition: DropCondition::Always, guaranteed: false, rarity_override: None },
            LootEntry { item_id: potion_id, weight: 50.0, min_quantity: 1, max_quantity: 3, condition: DropCondition::Always, guaranteed: false, rarity_override: None },
            LootEntry { item_id: ore_id, weight: 80.0, min_quantity: 1, max_quantity: 5, condition: DropCondition::Always, guaranteed: false, rarity_override: None },
            LootEntry { item_id: ring_id, weight: 5.0, min_quantity: 1, max_quantity: 1, condition: DropCondition::ChancePercent(25.0), guaranteed: false, rarity_override: None },
        ],
        drop_count_distribution: DropCountDistribution::NegativeBinomial { r: 2.0, p: 0.4 },
        magic_find_scaling: 0.01,
    };
    db.add_loot_table(loot_table);

    // Item set
    let set = ItemSet {
        id: db.alloc_id(),
        name: "Iron Warrior's Set".to_string(),
        item_ids: vec![sword_id, helmet_id],
        bonuses: vec![
            SetBonus {
                pieces_required: 2,
                description: "+10% Physical Damage".to_string(),
                modifiers: vec![StatModifier::percent(StatType::PhysicalDamage, 10.0)],
            },
        ],
        color: Vec4::new(0.6, 0.6, 0.6, 1.0),
    };
    db.add_set(set);

    // Vendor
    let mut vendor = VendorDefinition::new(db.alloc_id(), "Aldric the Merchant");
    vendor.inventory.push(VendorInventoryEntry {
        item_id: sword_id,
        stock: 3,
        max_stock: 5,
        restock_rate: 0.2,
        current_price_multiplier: 1.0,
        special_offer: false,
        offer_expiry: 0.0,
    });
    vendor.inventory.push(VendorInventoryEntry {
        item_id: potion_id,
        stock: 20,
        max_stock: 50,
        restock_rate: 2.0,
        current_price_multiplier: 1.0,
        special_offer: true,
        offer_expiry: 24.0,
    });
    db.vendors.insert(vendor.id, vendor);

    // Sample affixes
    let affix_str = AffixDefinition {
        id: 1,
        name: "of Strength".to_string(),
        affix_type: AffixType::Suffix,
        modifier_template: StatModifier::flat(StatType::Strength, 0.0),
        flat_min: 1.0,
        flat_max: 15.0,
        percent_min: 0.0,
        percent_max: 0.0,
        item_level_requirement: 1,
        weight: 1000.0,
        applicable_categories: vec![
            ItemCategory::Sword, ItemCategory::Axe, ItemCategory::Hammer,
            ItemCategory::Chestplate, ItemCategory::Helmet, ItemCategory::Ring,
        ],
        conflicts_with: vec![],
        synergies_with: vec![2],
        synergy_bonus: 5.0,
        tier: 1,
    };
    db.affix_pool.add(affix_str);

    let affix_dex = AffixDefinition {
        id: 2,
        name: "of Dexterity".to_string(),
        affix_type: AffixType::Suffix,
        modifier_template: StatModifier::flat(StatType::Dexterity, 0.0),
        flat_min: 1.0,
        flat_max: 15.0,
        percent_min: 0.0,
        percent_max: 0.0,
        item_level_requirement: 1,
        weight: 1000.0,
        applicable_categories: vec![
            ItemCategory::Dagger, ItemCategory::Bow, ItemCategory::Gloves,
            ItemCategory::Boots, ItemCategory::Ring,
        ],
        conflicts_with: vec![],
        synergies_with: vec![1],
        synergy_bonus: 5.0,
        tier: 1,
    };
    db.affix_pool.add(affix_dex);

    let affix_fire_prefix = AffixDefinition {
        id: 3,
        name: "Flaming".to_string(),
        affix_type: AffixType::Prefix,
        modifier_template: StatModifier::flat(StatType::FireDamage, 0.0),
        flat_min: 5.0,
        flat_max: 30.0,
        percent_min: 0.0,
        percent_max: 0.0,
        item_level_requirement: 3,
        weight: 600.0,
        applicable_categories: vec![
            ItemCategory::Sword, ItemCategory::Dagger, ItemCategory::Staff, ItemCategory::Wand,
        ],
        conflicts_with: vec![4], // conflicts with Frozen prefix
        synergies_with: vec![],
        synergy_bonus: 0.0,
        tier: 1,
    };
    db.affix_pool.add(affix_fire_prefix);

    let affix_cold_prefix = AffixDefinition {
        id: 4,
        name: "Frozen".to_string(),
        affix_type: AffixType::Prefix,
        modifier_template: StatModifier::flat(StatType::ColdDamage, 0.0),
        flat_min: 5.0,
        flat_max: 30.0,
        percent_min: 0.0,
        percent_max: 0.0,
        item_level_requirement: 3,
        weight: 600.0,
        applicable_categories: vec![
            ItemCategory::Sword, ItemCategory::Dagger, ItemCategory::Staff, ItemCategory::Wand,
        ],
        conflicts_with: vec![3], // conflicts with Flaming prefix
        synergies_with: vec![],
        synergy_bonus: 0.0,
        tier: 1,
    };
    db.affix_pool.add(affix_cold_prefix);

    let affix_crit = AffixDefinition {
        id: 5,
        name: "Razor".to_string(),
        affix_type: AffixType::Prefix,
        modifier_template: StatModifier::flat(StatType::CritChance, 0.0),
        flat_min: 2.0,
        flat_max: 8.0,
        percent_min: 0.0,
        percent_max: 0.0,
        item_level_requirement: 5,
        weight: 400.0,
        applicable_categories: vec![
            ItemCategory::Sword, ItemCategory::Dagger, ItemCategory::Axe,
        ],
        conflicts_with: vec![],
        synergies_with: vec![],
        synergy_bonus: 0.0,
        tier: 2,
    };
    db.affix_pool.add(affix_crit);

    db
}

// ============================================================
// ITEM TOOLTIP RENDERER
// ============================================================

#[derive(Debug, Clone)]
pub struct TooltipRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub lines: Vec<TooltipLine>,
}

#[derive(Debug, Clone)]
pub struct TooltipLine {
    pub text: String,
    pub color: Vec4,
    pub indent: f32,
    pub font_size: f32,
    pub separator: bool,
}

pub fn build_item_tooltip_rect(def: &ItemDefinition, anchor: Vec2, max_width: f32) -> TooltipRect {
    let line_height = 18.0;
    let padding = 8.0;
    let mut lines: Vec<TooltipLine> = Vec::new();
    let rarity_cfg = RarityConfig::for_rarity(def.rarity);

    // Title
    lines.push(TooltipLine {
        text: def.name.clone(),
        color: rarity_cfg.color,
        indent: 0.0,
        font_size: 16.0,
        separator: false,
    });

    // Category + Rarity
    lines.push(TooltipLine {
        text: format!("{} — {}", def.category.display_name(), rarity_cfg.display_name()),
        color: Vec4::new(0.75, 0.75, 0.75, 1.0),
        indent: 0.0,
        font_size: 14.0,
        separator: false,
    });

    // Separator
    lines.push(TooltipLine { text: String::new(), color: Vec4::ONE, indent: 0.0, font_size: 0.0, separator: true });

    // Combat stats
    if def.category.is_weapon() {
        let dps = def.total_damage_per_second();
        lines.push(TooltipLine {
            text: format!("DPS: {:.1}  ({:.2} attacks/sec)", dps, def.attack_speed),
            color: Vec4::new(1.0, 0.9, 0.5, 1.0),
            indent: 0.0, font_size: 14.0, separator: false,
        });
        for dmg in &def.damage_ranges {
            let avg = (dmg.min_damage + dmg.max_damage) / 2.0;
            lines.push(TooltipLine {
                text: format!("  {:?}: {:.0}–{:.0} ({:.1} avg)", dmg.damage_type, dmg.min_damage, dmg.max_damage, avg),
                color: dmg.damage_type.color(),
                indent: 8.0, font_size: 13.0, separator: false,
            });
        }
        lines.push(TooltipLine {
            text: format!("Crit: {:.1}% × {:.1}x", def.crit_chance, def.crit_multiplier),
            color: Vec4::new(1.0, 0.7, 0.4, 1.0),
            indent: 0.0, font_size: 13.0, separator: false,
        });
    }

    if def.armor_values.physical_armor > 0.0 {
        lines.push(TooltipLine {
            text: format!("Armor: {:.0}", def.armor_values.physical_armor),
            color: Vec4::new(0.7, 0.85, 1.0, 1.0),
            indent: 0.0, font_size: 14.0, separator: false,
        });
    }
    if def.armor_values.evasion_rating > 0.0 {
        lines.push(TooltipLine {
            text: format!("Evasion: {:.0}", def.armor_values.evasion_rating),
            color: Vec4::new(0.5, 1.0, 0.7, 1.0),
            indent: 0.0, font_size: 14.0, separator: false,
        });
    }
    if def.armor_values.energy_shield > 0.0 {
        lines.push(TooltipLine {
            text: format!("Energy Shield: {:.0}", def.armor_values.energy_shield),
            color: Vec4::new(0.6, 0.8, 1.0, 1.0),
            indent: 0.0, font_size: 14.0, separator: false,
        });
    }

    // Resistances
    let mut resists = Vec::new();
    if def.armor_values.fire_resist != 0.0 { resists.push(format!("Fire: {:.0}%", def.armor_values.fire_resist)); }
    if def.armor_values.cold_resist != 0.0 { resists.push(format!("Cold: {:.0}%", def.armor_values.cold_resist)); }
    if def.armor_values.lightning_resist != 0.0 { resists.push(format!("Lit: {:.0}%", def.armor_values.lightning_resist)); }
    if def.armor_values.poison_resist != 0.0 { resists.push(format!("Poi: {:.0}%", def.armor_values.poison_resist)); }
    if !resists.is_empty() {
        lines.push(TooltipLine {
            text: format!("Resist: {}", resists.join(" | ")),
            color: Vec4::new(1.0, 0.6, 0.2, 1.0),
            indent: 0.0, font_size: 13.0, separator: false,
        });
    }

    if !def.implicit_modifiers.is_empty() || !def.explicit_modifiers.is_empty() {
        lines.push(TooltipLine { text: String::new(), color: Vec4::ONE, indent: 0.0, font_size: 0.0, separator: true });
    }

    // Implicit modifiers
    for m in &def.implicit_modifiers {
        let (text, color) = format_modifier(m);
        lines.push(TooltipLine { text, color, indent: 0.0, font_size: 13.0, separator: false });
    }

    if !def.implicit_modifiers.is_empty() && !def.explicit_modifiers.is_empty() {
        lines.push(TooltipLine { text: String::new(), color: Vec4::ONE, indent: 0.0, font_size: 0.0, separator: true });
    }

    // Explicit modifiers
    for m in &def.explicit_modifiers {
        let (text, color) = format_modifier(m);
        lines.push(TooltipLine { text, color, indent: 0.0, font_size: 13.0, separator: false });
    }

    // Unique effect
    if let Some(ref fx) = def.unique_effect {
        lines.push(TooltipLine { text: String::new(), color: Vec4::ONE, indent: 0.0, font_size: 0.0, separator: true });
        lines.push(TooltipLine {
            text: fx.clone(),
            color: Vec4::new(1.0, 0.6, 0.1, 1.0),
            indent: 0.0, font_size: 13.0, separator: false,
        });
    }

    // Flavor text
    if !def.flavor_text.is_empty() {
        lines.push(TooltipLine { text: String::new(), color: Vec4::ONE, indent: 0.0, font_size: 0.0, separator: true });
        lines.push(TooltipLine {
            text: format!("\"{}\"", def.flavor_text),
            color: Vec4::new(0.6, 0.6, 0.8, 1.0),
            indent: 0.0, font_size: 12.0, separator: false,
        });
    }

    // Requirements
    lines.push(TooltipLine { text: String::new(), color: Vec4::ONE, indent: 0.0, font_size: 0.0, separator: true });
    lines.push(TooltipLine {
        text: format!("Requires Level {}", def.requirements.level),
        color: Vec4::new(0.9, 0.6, 0.3, 1.0),
        indent: 0.0, font_size: 13.0, separator: false,
    });
    if def.requirements.strength > 0 {
        lines.push(TooltipLine {
            text: format!("  Strength {}", def.requirements.strength),
            color: Vec4::new(0.9, 0.6, 0.3, 1.0),
            indent: 8.0, font_size: 12.0, separator: false,
        });
    }
    if def.requirements.dexterity > 0 {
        lines.push(TooltipLine {
            text: format!("  Dexterity {}", def.requirements.dexterity),
            color: Vec4::new(0.9, 0.6, 0.3, 1.0),
            indent: 8.0, font_size: 12.0, separator: false,
        });
    }

    // Weight and item level
    lines.push(TooltipLine {
        text: format!("Weight: {:.1}  |  Item Level: {}", def.weight, def.item_level),
        color: Vec4::new(0.6, 0.6, 0.6, 1.0),
        indent: 0.0, font_size: 12.0, separator: false,
    });
    lines.push(TooltipLine {
        text: format!("Value: {} gold  |  Sell: {} gold", def.base_price, def.effective_sell_price()),
        color: Vec4::new(1.0, 0.85, 0.0, 1.0),
        indent: 0.0, font_size: 12.0, separator: false,
    });

    if def.bind_on_pickup {
        lines.push(TooltipLine {
            text: "Binds when Picked Up".to_string(),
            color: Vec4::new(0.8, 0.4, 0.4, 1.0),
            indent: 0.0, font_size: 12.0, separator: false,
        });
    }

    // Compute total height
    let total_height = lines.iter().map(|l| {
        if l.separator { 8.0 } else { l.font_size * 1.3 }
    }).sum::<f32>() + padding * 2.0;

    TooltipRect {
        x: anchor.x,
        y: anchor.y,
        width: max_width.min(260.0),
        height: total_height,
        lines,
    }
}

fn format_modifier(m: &StatModifier) -> (String, Vec4) {
    let name = m.stat.display_name();
    let is_pct = m.stat.is_percentage();
    let positive = m.flat_value >= 0.0 && m.percent_value >= 0.0;
    let color = if positive { Vec4::new(0.5, 1.0, 0.5, 1.0) } else { Vec4::new(1.0, 0.4, 0.4, 1.0) };
    let sign = if positive { "+" } else { "" };

    let text = if m.flat_value != 0.0 && m.percent_value != 0.0 {
        format!("{}{:.0} / {}{:.0}% {}", sign, m.flat_value, sign, m.percent_value, name)
    } else if m.percent_value != 0.0 {
        format!("{}{:.0}% {}", sign, m.percent_value, name)
    } else if is_pct {
        format!("{}{:.1}% {}", sign, m.flat_value, name)
    } else {
        format!("{}{:.0} {}", sign, m.flat_value, name)
    };

    (text, color)
}

// ============================================================
// LOOT TABLE PROBABILITY DISPLAY
// ============================================================

#[derive(Debug, Clone)]
pub struct DropProbabilityEntry {
    pub item_id: u64,
    pub weight: f32,
    pub total_weight: f32,
    pub base_probability: f32,
    pub magic_find_adjusted: f32,
    pub conditional: bool,
}

impl DropProbabilityEntry {
    pub fn probability_percent(&self) -> f32 {
        self.base_probability * 100.0
    }

    pub fn magic_find_probability_percent(&self) -> f32 {
        self.magic_find_adjusted * 100.0
    }
}

pub fn compute_drop_probabilities(table: &LootTable, magic_find: f32) -> Vec<DropProbabilityEntry> {
    let total_weight: f32 = table.entries.iter().filter(|e| !e.guaranteed).map(|e| e.weight).sum();
    let mf_mul = 1.0 + magic_find * table.magic_find_scaling / 100.0;

    table.entries.iter().map(|entry| {
        let base_prob = if entry.guaranteed { 1.0 } else if total_weight > 0.0 { entry.weight / total_weight } else { 0.0 };
        let mf_total: f32 = table.entries.iter().filter(|e| !e.guaranteed).map(|e| e.weight * mf_mul).sum();
        let mf_prob = if entry.guaranteed { 1.0 } else if mf_total > 0.0 { entry.weight * mf_mul / mf_total } else { 0.0 };
        DropProbabilityEntry {
            item_id: entry.item_id,
            weight: entry.weight,
            total_weight,
            base_probability: base_prob,
            magic_find_adjusted: mf_prob,
            conditional: !matches!(entry.condition, DropCondition::Always),
        }
    }).collect()
}

// ============================================================
// ECONOMY SIMULATION
// ============================================================

#[derive(Debug, Clone)]
pub struct EconomySimulator {
    pub vendors: Vec<VendorDefinition>,
    pub transaction_log: VecDeque<EconomyEvent>,
    pub time: f32,
    pub price_history: HashMap<u64, Vec<(f32, u64)>>, // (time, price)
}

#[derive(Debug, Clone)]
pub struct EconomyEvent {
    pub time: f32,
    pub vendor_id: u64,
    pub item_id: u64,
    pub transaction: EconomyTransaction,
    pub price: u64,
    pub quantity: u32,
}

impl EconomySimulator {
    pub fn new() -> Self {
        EconomySimulator {
            vendors: Vec::new(),
            transaction_log: VecDeque::new(),
            time: 0.0,
            price_history: HashMap::new(),
        }
    }

    pub fn simulate_step(&mut self, dt_hours: f32, item_defs: &HashMap<u64, ItemDefinition>) {
        self.time += dt_hours;

        for vendor in &mut self.vendors {
            vendor.refresh_inventory(self.time);

            // Simulate random player transactions
            let mut seed: u64 = (self.time * 1000.0) as u64 ^ vendor.id;
            for entry in &vendor.inventory {
                let buy_p = lcg_f32(&mut seed);
                if buy_p < 0.1 && entry.stock > 0 {
                    // Simulate a buy event
                    let qty = 1u32;
                    if let Some(def) = item_defs.get(&entry.item_id) {
                        let price = vendor.buy_price(def);
                        self.price_history
                            .entry(entry.item_id)
                            .or_insert_with(Vec::new)
                            .push((self.time, price));
                        self.transaction_log.push_back(EconomyEvent {
                            time: self.time,
                            vendor_id: vendor.id,
                            item_id: entry.item_id,
                            transaction: EconomyTransaction::PlayerBought { quantity: qty },
                            price,
                            quantity: qty,
                        });
                    }
                    vendor.adjust_supply_demand(EconomyTransaction::PlayerBought { quantity: qty });
                }
            }
        }

        // Trim transaction log to last 500 events
        while self.transaction_log.len() > 500 {
            self.transaction_log.pop_front();
        }
    }

    pub fn average_price_for_item(&self, item_id: u64, last_n_events: usize) -> f32 {
        if let Some(history) = self.price_history.get(&item_id) {
            let slice = if history.len() > last_n_events {
                &history[history.len() - last_n_events..]
            } else {
                history.as_slice()
            };
            if slice.is_empty() { return 0.0; }
            let sum: u64 = slice.iter().map(|(_, p)| p).sum();
            sum as f32 / slice.len() as f32
        } else {
            0.0
        }
    }

    pub fn price_trend(&self, item_id: u64) -> f32 {
        // Returns positive if price trending up, negative if down, 0 if stable
        if let Some(history) = self.price_history.get(&item_id) {
            if history.len() < 2 { return 0.0; }
            let n = history.len();
            let recent_avg = history[n/2..].iter().map(|(_, p)| *p as f32).sum::<f32>() / (n - n/2) as f32;
            let older_avg = history[..n/2].iter().map(|(_, p)| *p as f32).sum::<f32>() / (n/2) as f32;
            recent_avg - older_avg
        } else {
            0.0
        }
    }
}

// ============================================================
// ITEM PREVIEW RENDERER (3D transform math)
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemPreviewTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub auto_rotate: bool,
    pub auto_rotate_speed: f32,
    pub accumulated_angle: f32,
}

impl ItemPreviewTransform {
    pub fn default_preview() -> Self {
        ItemPreviewTransform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            auto_rotate: true,
            auto_rotate_speed: 0.5,
            accumulated_angle: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.auto_rotate {
            self.accumulated_angle += self.auto_rotate_speed * dt;
            self.rotation = Quat::from_rotation_y(self.accumulated_angle);
        }
    }

    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        let yaw = Quat::from_rotation_y(delta_yaw);
        let right = self.rotation * Vec3::X;
        let pitch = Quat::from_axis_angle(right, delta_pitch);
        self.rotation = (pitch * yaw * self.rotation).normalize();
        self.auto_rotate = false;
    }

    pub fn reset(&mut self) {
        *self = ItemPreviewTransform::default_preview();
    }
}

// ============================================================
// INVENTORY DRAG-AND-DROP STATE
// ============================================================

#[derive(Debug, Clone)]
pub enum DragState {
    Idle,
    Dragging {
        instance_id: u64,
        source_container: u64,
        cursor_offset: Vec2,
        current_pos: Vec2,
        drop_valid: bool,
        preview_col: Option<u32>,
        preview_row: Option<u32>,
    },
}

impl DragState {
    pub fn start_drag(instance_id: u64, source: u64, offset: Vec2) -> Self {
        DragState::Dragging {
            instance_id,
            source_container: source,
            cursor_offset: offset,
            current_pos: Vec2::ZERO,
            drop_valid: false,
            preview_col: None,
            preview_row: None,
        }
    }

    pub fn is_dragging(&self) -> bool {
        matches!(self, DragState::Dragging { .. })
    }

    pub fn update_position(&mut self, pos: Vec2) {
        if let DragState::Dragging { ref mut current_pos, .. } = self {
            *current_pos = pos;
        }
    }

    pub fn end_drag(&mut self) -> Option<(u64, u64, Option<(u32, u32)>)> {
        if let DragState::Dragging { instance_id, source_container, preview_col, preview_row, .. } = self.clone() {
            *self = DragState::Idle;
            let target = preview_col.and_then(|c| preview_row.map(|r| (c, r)));
            Some((instance_id, source_container, target))
        } else {
            None
        }
    }
}

// ============================================================
// HOTKEY / KEYBINDING SYSTEM
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorAction {
    NewItem,
    OpenItem,
    SaveItem,
    DeleteItem,
    DuplicateItem,
    Undo,
    Redo,
    Copy,
    Paste,
    Search,
    SelectAll,
    DeselectAll,
    GenerateRandom,
    ExportCsv,
    ToggleFilter,
    SwitchToBrowser,
    SwitchToItemEditor,
    SwitchToRecipes,
}

#[derive(Debug, Clone)]
pub struct Keybinding {
    pub action: EditorAction,
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub description: String,
}

pub fn default_keybindings() -> Vec<Keybinding> {
    vec![
        Keybinding { action: EditorAction::NewItem, key: "N".to_string(), ctrl: true, shift: false, alt: false, description: "Create new item".to_string() },
        Keybinding { action: EditorAction::SaveItem, key: "S".to_string(), ctrl: true, shift: false, alt: false, description: "Save current item".to_string() },
        Keybinding { action: EditorAction::DeleteItem, key: "Delete".to_string(), ctrl: false, shift: false, alt: false, description: "Delete selected items".to_string() },
        Keybinding { action: EditorAction::DuplicateItem, key: "D".to_string(), ctrl: true, shift: false, alt: false, description: "Duplicate item".to_string() },
        Keybinding { action: EditorAction::Undo, key: "Z".to_string(), ctrl: true, shift: false, alt: false, description: "Undo".to_string() },
        Keybinding { action: EditorAction::Redo, key: "Y".to_string(), ctrl: true, shift: false, alt: false, description: "Redo".to_string() },
        Keybinding { action: EditorAction::Copy, key: "C".to_string(), ctrl: true, shift: false, alt: false, description: "Copy item".to_string() },
        Keybinding { action: EditorAction::Paste, key: "V".to_string(), ctrl: true, shift: false, alt: false, description: "Paste item".to_string() },
        Keybinding { action: EditorAction::Search, key: "F".to_string(), ctrl: true, shift: false, alt: false, description: "Focus search".to_string() },
        Keybinding { action: EditorAction::SelectAll, key: "A".to_string(), ctrl: true, shift: false, alt: false, description: "Select all".to_string() },
        Keybinding { action: EditorAction::GenerateRandom, key: "G".to_string(), ctrl: true, shift: false, alt: false, description: "Generate random item".to_string() },
        Keybinding { action: EditorAction::ExportCsv, key: "E".to_string(), ctrl: true, shift: false, alt: false, description: "Export to CSV".to_string() },
    ]
}

// ============================================================
// ADDITIONAL CRAFTING HELPERS
// ============================================================

pub fn list_recipes_for_output(db: &ItemDatabase, item_id: u64) -> Vec<&RecipeDefinition> {
    db.recipes.values().filter(|r| r.output_item_id == item_id).collect()
}

pub fn list_recipes_using_ingredient(db: &ItemDatabase, ingredient_id: u64) -> Vec<&RecipeDefinition> {
    db.recipes.values().filter(|r| r.ingredients.iter().any(|i| i.item_id == ingredient_id)).collect()
}

pub fn crafting_tree(db: &ItemDatabase, target_id: u64, depth: u32) -> Vec<(u32, u64, u32)> {
    // Returns (depth, item_id, quantity) for full dependency tree
    let mut result = Vec::new();
    let mut queue: VecDeque<(u32, u64, u32)> = VecDeque::new();
    queue.push_back((0, target_id, 1));

    let mut visited: HashSet<u64> = HashSet::new();

    while let Some((d, item_id, qty)) = queue.pop_front() {
        if d > depth { continue; }
        result.push((d, item_id, qty));
        if visited.contains(&item_id) { continue; }
        visited.insert(item_id);

        if let Some(recipe) = db.recipes.values().find(|r| r.output_item_id == item_id) {
            for ing in &recipe.ingredients {
                queue.push_back((d + 1, ing.item_id, ing.quantity * qty));
            }
        }
    }

    result
}

// ============================================================
// ADVANCED SET BONUS DISPLAY
// ============================================================

pub fn format_set_bonus_tooltip(set: &ItemSet, equipped_count: u32) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("=== {} ===", set.name));
    lines.push(format!("{}/{} pieces equipped", equipped_count, set.item_ids.len()));
    for bonus in &set.bonuses {
        let active = equipped_count >= bonus.pieces_required;
        let prefix = if active { "[ACTIVE]" } else { "[LOCKED]" };
        lines.push(format!("{} ({} pc): {}", prefix, bonus.pieces_required, bonus.description));
    }
    lines
}

// ============================================================
// STAT COMPARISON TABLE
// ============================================================

pub fn stat_comparison_table(items: &[&ItemDefinition]) -> Vec<Vec<String>> {
    let all_stats = [
        StatType::Strength, StatType::Dexterity, StatType::Intelligence,
        StatType::Vitality, StatType::CritChance, StatType::CritMultiplier,
        StatType::PhysicalDamage, StatType::FireDamage, StatType::LifeMax,
        StatType::ManaMax, StatType::PhysicalArmor, StatType::CooldownReduction,
    ];

    let mut header = vec!["Stat".to_string()];
    for item in items { header.push(item.name.clone()); }

    let mut table = vec![header];
    for stat in &all_stats {
        let mut row = vec![stat.display_name().to_string()];
        for item in items {
            let total: f32 = item.explicit_modifiers.iter()
                .chain(item.implicit_modifiers.iter())
                .filter(|m| &m.stat == stat)
                .map(|m| m.flat_value + m.percent_value)
                .sum();
            if total == 0.0 { row.push("—".to_string()); }
            else { row.push(format!("{:.1}", total)); }
        }
        table.push(row);
    }

    // DPS row
    let mut dps_row = vec!["DPS".to_string()];
    for item in items { dps_row.push(format!("{:.1}", item.total_damage_per_second())); }
    table.push(dps_row);

    // Armor row
    let mut armor_row = vec!["Armor".to_string()];
    for item in items { armor_row.push(format!("{:.0}", item.armor_values.physical_armor)); }
    table.push(armor_row);

    // Price row
    let mut price_row = vec!["Price".to_string()];
    for item in items { price_row.push(format!("{}", item.base_price)); }
    table.push(price_row);

    table
}

// ============================================================
// AFFIX GENERATION PRESET HELPERS
// ============================================================

pub fn generate_affix_pool_preset() -> AffixPool {
    let mut pool = AffixPool::new();
    let all_weapon_cats = vec![
        ItemCategory::Sword, ItemCategory::Greatsword, ItemCategory::Dagger,
        ItemCategory::Axe, ItemCategory::Greataxe, ItemCategory::Mace,
        ItemCategory::Hammer, ItemCategory::Spear, ItemCategory::Staff, ItemCategory::Wand,
    ];
    let all_armor_cats = vec![
        ItemCategory::Helmet, ItemCategory::Chestplate, ItemCategory::Greaves,
        ItemCategory::Gauntlets, ItemCategory::Boots, ItemCategory::Shield,
    ];
    let all_acc_cats = vec![ItemCategory::Ring, ItemCategory::Amulet, ItemCategory::Belt];

    // Generate a large batch of affixes for the pool
    let stat_pairs: &[(StatType, &str, f32, f32, f32, f32, AffixType, &[usize])] = &[
        // (stat, name, flat_min, flat_max, pct_min, pct_max, type, applicable_group_indices)
        // 0=weapons, 1=armor, 2=accessories
    ];

    // Add weapon damage affixes
    for (i, &(dt, name)) in [
        (DamageType::Fire, "Fiery"), (DamageType::Cold, "Glacial"),
        (DamageType::Lightning, "Shocking"), (DamageType::Poison, "Toxic"),
        (DamageType::Arcane, "Arcane"),
    ].iter().enumerate() {
        let stat = match dt {
            DamageType::Fire => StatType::FireDamage,
            DamageType::Cold => StatType::ColdDamage,
            DamageType::Lightning => StatType::LightningDamage,
            DamageType::Poison => StatType::PoisonDamage,
            _ => StatType::PhysicalDamage,
        };
        pool.add(AffixDefinition {
            id: 100 + i as u64,
            name: name.to_string(),
            affix_type: AffixType::Prefix,
            modifier_template: StatModifier::flat(stat.clone(), 0.0),
            flat_min: 3.0 + i as f32 * 2.0,
            flat_max: 15.0 + i as f32 * 5.0,
            percent_min: 0.0,
            percent_max: 0.0,
            item_level_requirement: 1 + i as u32 * 2,
            weight: 500.0 - i as f32 * 50.0,
            applicable_categories: all_weapon_cats.clone(),
            conflicts_with: vec![],
            synergies_with: vec![],
            synergy_bonus: 0.0,
            tier: 1 + (i / 2) as u32,
        });
    }

    // Add armor resistance affixes
    for (i, (stat, name)) in [
        (StatType::FireResistance, "of Fire Ward"),
        (StatType::ColdResistance, "of Cold Ward"),
        (StatType::LightningResistance, "of Lightning Ward"),
        (StatType::PoisonResistance, "of Toxin Ward"),
    ].iter().enumerate() {
        pool.add(AffixDefinition {
            id: 200 + i as u64,
            name: name.to_string(),
            affix_type: AffixType::Suffix,
            modifier_template: StatModifier::percent(stat.clone(), 0.0),
            flat_min: 0.0,
            flat_max: 0.0,
            percent_min: 5.0,
            percent_max: 25.0,
            item_level_requirement: 1,
            weight: 400.0,
            applicable_categories: all_armor_cats.clone(),
            conflicts_with: vec![],
            synergies_with: vec![],
            synergy_bonus: 0.0,
            tier: 1,
        });
    }

    // Add life/mana modifiers for accessories
    pool.add(AffixDefinition {
        id: 300,
        name: "of Vitality".to_string(),
        affix_type: AffixType::Suffix,
        modifier_template: StatModifier::flat(StatType::LifeMax, 0.0),
        flat_min: 10.0,
        flat_max: 80.0,
        percent_min: 0.0,
        percent_max: 0.0,
        item_level_requirement: 1,
        weight: 800.0,
        applicable_categories: all_acc_cats.clone(),
        conflicts_with: vec![],
        synergies_with: vec![301],
        synergy_bonus: 10.0,
        tier: 1,
    });
    pool.add(AffixDefinition {
        id: 301,
        name: "of the Mind".to_string(),
        affix_type: AffixType::Suffix,
        modifier_template: StatModifier::flat(StatType::ManaMax, 0.0),
        flat_min: 10.0,
        flat_max: 60.0,
        percent_min: 0.0,
        percent_max: 0.0,
        item_level_requirement: 1,
        weight: 700.0,
        applicable_categories: all_acc_cats.clone(),
        conflicts_with: vec![],
        synergies_with: vec![300],
        synergy_bonus: 8.0,
        tier: 1,
    });

    pool
}

// ============================================================
// FINAL PUBLIC API / ENTRY POINTS
// ============================================================

pub fn build_editor() -> InventoryEditor {
    let mut editor = InventoryEditor::new();
    editor.database = create_starter_item_database();
    // Augment with the full affix preset
    let more_affixes = generate_affix_pool_preset();
    for affix in more_affixes.definitions {
        editor.database.affix_pool.add(affix);
    }
    editor.browser.refresh(&editor.database);
    editor
}

pub fn run_editor_frame(editor: &mut InventoryEditor, dt: f32, input: &EditorInput) {
    editor.tick(dt);

    // Update preview transform
    // (would update if we had a preview transform stored in editor)

    // Handle tab switching
    if let Some(ref action) = input.action {
        match action {
            EditorAction::NewItem => editor.create_new_item(ItemCategory::Sword),
            EditorAction::SaveItem => { editor.save_current_item(); }
            EditorAction::DeleteItem => editor.delete_selected_items(),
            EditorAction::Undo => editor.item_editor.undo(),
            EditorAction::Redo => editor.item_editor.redo(),
            EditorAction::Copy => editor.copy_item(),
            EditorAction::Paste => editor.paste_item(),
            EditorAction::GenerateRandom => editor.generate_random_item(),
            EditorAction::ExportCsv => { let _csv = editor.export_to_csv(); }
            EditorAction::SelectAll => editor.browser.select_all(),
            EditorAction::DeselectAll => editor.browser.deselect_all(),
            _ => {}
        }
    }

    if let Some(search) = input.search_query.clone() {
        editor.search_with_history(search);
    }

    if let Some(id) = input.open_item_id {
        editor.open_item_by_id(id);
    }
}

#[derive(Debug, Clone)]
pub struct EditorInput {
    pub action: Option<EditorAction>,
    pub search_query: Option<String>,
    pub open_item_id: Option<u64>,
    pub mouse_pos: Vec2,
    pub delta_time: f32,
}

impl EditorInput {
    pub fn idle() -> Self {
        EditorInput {
            action: None,
            search_query: None,
            open_item_id: None,
            mouse_pos: Vec2::ZERO,
            delta_time: 0.016,
        }
    }
}

// ============================================================
// UNIT-TESTABLE MATH HELPERS
// ============================================================

pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub fn inv_lerp_f32(a: f32, b: f32, v: f32) -> f32 {
    if (b - a).abs() < 1e-7 { return 0.0; }
    ((v - a) / (b - a)).clamp(0.0, 1.0)
}

pub fn smooth_step(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn smoother_step(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

pub fn damage_reduction_formula(armor: f32, attacker_level: f32, defender_level: f32) -> f32 {
    // WoW-style armor mitigation: armor / (armor + K) where K is level-based
    let k = 467.5 * defender_level - 22167.5;
    let k = k.max(1.0);
    let raw = armor / (armor + k);
    // Apply attacker level penalty
    let level_ratio = attacker_level / defender_level;
    (raw * level_ratio.min(1.0)).clamp(0.0, 0.85)
}

pub fn critical_strike_damage(base: f32, crit_multiplier: f32, crit_chance: f32, roll: f32) -> f32 {
    if roll < crit_chance / 100.0 {
        base * crit_multiplier
    } else {
        base
    }
}

pub fn resist_capped(resist: f32, cap: f32) -> f32 {
    resist.min(cap)
}

pub fn elemental_damage_after_resist(raw: f32, resist_pct: f32) -> f32 {
    let r = resist_capped(resist_pct, 75.0) / 100.0;
    raw * (1.0 - r)
}

pub fn diminishing_returns_cdr(base_cdr_pct: f32) -> f32 {
    // DR formula: effective = 1 - (1/(1 + cdr/100))
    let raw = base_cdr_pct / 100.0;
    (1.0 - 1.0 / (1.0 + raw)) * 100.0
}

pub fn stat_scaling_value(base: f32, scaling_stat: f32, coefficient: f32, exponent: f32) -> f32 {
    base + coefficient * scaling_stat.powf(exponent)
}

pub fn item_power_score(def: &ItemDefinition) -> f32 {
    let dps_score = def.total_damage_per_second() * 0.5;
    let armor_score = def.armor_values.physical_armor * 0.3;
    let stat_score = def.total_stat_budget();
    let rarity_bonus = match def.rarity {
        Rarity::Common => 0.0,
        Rarity::Uncommon => 5.0,
        Rarity::Rare => 15.0,
        Rarity::Epic => 30.0,
        Rarity::Legendary => 60.0,
        Rarity::Mythic => 120.0,
    };
    dps_score + armor_score + stat_score + rarity_bonus + def.item_level as f32 * 2.0
}

// ============================================================
// RARITY DROP WEIGHT TABLE
// ============================================================

pub struct RarityDropTable {
    pub alias: AliasTable,
    pub rarities: Vec<Rarity>,
}

impl RarityDropTable {
    pub fn standard() -> Self {
        let rarities = vec![
            Rarity::Common,
            Rarity::Uncommon,
            Rarity::Rare,
            Rarity::Epic,
            Rarity::Legendary,
            Rarity::Mythic,
        ];
        let weights: Vec<f64> = rarities.iter().map(|r| {
            RarityConfig::for_rarity(*r).drop_weight as f64
        }).collect();
        RarityDropTable {
            alias: AliasTable::new(&weights),
            rarities,
        }
    }

    pub fn magic_find_adjusted(magic_find: f32) -> Self {
        let base_rarities = vec![
            Rarity::Common,
            Rarity::Uncommon,
            Rarity::Rare,
            Rarity::Epic,
            Rarity::Legendary,
            Rarity::Mythic,
        ];
        let mf_mul = 1.0 + magic_find / 100.0;
        let weights: Vec<f64> = base_rarities.iter().map(|r| {
            let cfg = RarityConfig::for_rarity(*r);
            // Common items' weight is not boosted by magic find
            let boost = match r {
                Rarity::Common => 1.0f64,
                _ => mf_mul as f64,
            };
            cfg.drop_weight as f64 * boost
        }).collect();
        RarityDropTable {
            alias: AliasTable::new(&weights),
            rarities: base_rarities,
        }
    }

    pub fn sample(&self, u1: f64, u2: f64) -> Rarity {
        let idx = self.alias.sample(u1, u2);
        self.rarities[idx]
    }
}

// ============================================================
// END OF FILE
// ============================================================
