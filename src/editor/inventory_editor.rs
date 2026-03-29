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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
            self.search_history = self.search_history.drain(..).collect::<Vec<_>>().into_iter().fold(std::collections::VecDeque::new(), |mut acc, x| { if acc.back() != Some(&x) { acc.push_back(x); } acc });
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
            let mut buy_events: Vec<(u64, u32)> = Vec::new();
            for entry in &vendor.inventory {
                let buy_p = lcg_f32(&mut seed);
                if buy_p < 0.1 && entry.stock > 0 {
                    buy_events.push((entry.item_id, 1u32));
                }
            }
            for (item_id, qty) in buy_events {
                if let Some(def) = item_defs.get(&item_id) {
                    let price = vendor.buy_price(def);
                    self.price_history
                        .entry(item_id)
                        .or_insert_with(Vec::new)
                        .push((self.time, price));
                    self.transaction_log.push_back(EconomyEvent {
                        time: self.time,
                        vendor_id: vendor.id,
                        item_id,
                        transaction: EconomyTransaction::PlayerBought { quantity: qty },
                        price,
                        quantity: qty,
                    });
                }
                vendor.adjust_supply_demand(EconomyTransaction::PlayerBought { quantity: qty });
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
// EXTENDED: ITEM POWER CURVE ANALYSIS
// ============================================================

#[derive(Debug, Clone)]
pub struct PowerCurvePoint {
    pub item_level: u32,
    pub rarity: Rarity,
    pub expected_dps: f32,
    pub expected_armor: f32,
    pub expected_stat_budget: f32,
}

pub fn generate_power_curve(min_level: u32, max_level: u32) -> Vec<PowerCurvePoint> {
    let rarities = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic, Rarity::Legendary, Rarity::Mythic];
    let mut points = Vec::new();
    for level in min_level..=max_level {
        for &rarity in &rarities {
            let cfg = RarityConfig::for_rarity(rarity);
            let expected_dps = (5.0 + level as f32 * 2.0) * cfg.stat_multiplier;
            let expected_armor = (10.0 + level as f32 * 3.0) * cfg.stat_multiplier;
            let expected_budget = budget_expectation(level, rarity);
            points.push(PowerCurvePoint {
                item_level: level,
                rarity,
                expected_dps,
                expected_armor,
                expected_stat_budget: expected_budget,
            });
        }
    }
    points
}

pub fn item_level_from_dps(dps: f32, rarity: Rarity) -> u32 {
    let cfg = RarityConfig::for_rarity(rarity);
    let base_dps_at_1 = 5.0 * cfg.stat_multiplier;
    let dps_per_level = 2.0 * cfg.stat_multiplier;
    if dps_per_level <= 0.0 { return 1; }
    let level = (dps - base_dps_at_1) / dps_per_level;
    (level.max(1.0) as u32).min(100)
}

pub fn recommended_item_level_for_zone(zone_difficulty: u32) -> u32 {
    let base = (zone_difficulty.saturating_sub(1)) * 10 + 1;
    base
}

// ============================================================
// EXTENDED: ITEM ENCHANTMENT SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct EnchantmentDefinition {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub modifier: StatModifier,
    pub cost_gold: u64,
    pub cost_materials: Vec<(u64, u32)>,
    pub applicable_slots: Vec<EquipmentSlot>,
    pub max_enchant_level: u32,
    pub scaling_per_level: f32,
    pub visual_glow_color: Vec4,
    pub incompatible_with: Vec<u64>,
}

impl EnchantmentDefinition {
    pub fn modifier_at_level(&self, level: u32) -> StatModifier {
        let mul = self.scaling_per_level.powf(level.saturating_sub(1) as f32);
        let mut m = self.modifier.clone();
        m.flat_value *= mul;
        m.percent_value *= mul;
        m
    }

    pub fn cost_at_level(&self, level: u32) -> u64 {
        let mul = (2.0f32).powf(level as f32 - 1.0);
        (self.cost_gold as f32 * mul) as u64
    }

    pub fn expected_dps_bonus(&self, attack_speed: f32) -> f32 {
        if self.modifier.stat == StatType::PhysicalDamage {
            self.modifier.flat_value * attack_speed
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemEnchantment {
    pub enchant_id: u64,
    pub level: u32,
    pub applied_at_item_level: u32,
}

impl ItemEnchantment {
    pub fn new(enchant_id: u64, level: u32, item_level: u32) -> Self {
        ItemEnchantment { enchant_id, level, applied_at_item_level: item_level }
    }

    pub fn effective_modifier(&self, def: &EnchantmentDefinition) -> StatModifier {
        def.modifier_at_level(self.level)
    }
}

// ============================================================
// EXTENDED: ITEM SOCKET SYSTEM
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketColor {
    Red,
    Blue,
    Yellow,
    Green,
    White,
    Black,
}

impl SocketColor {
    pub fn display_name(&self) -> &'static str {
        match self {
            SocketColor::Red => "Red",
            SocketColor::Blue => "Blue",
            SocketColor::Yellow => "Yellow",
            SocketColor::Green => "Green",
            SocketColor::White => "White",
            SocketColor::Black => "Black",
        }
    }

    pub fn color(&self) -> Vec4 {
        match self {
            SocketColor::Red => Vec4::new(1.0, 0.1, 0.1, 1.0),
            SocketColor::Blue => Vec4::new(0.1, 0.3, 1.0, 1.0),
            SocketColor::Yellow => Vec4::new(1.0, 0.9, 0.0, 1.0),
            SocketColor::Green => Vec4::new(0.1, 0.8, 0.1, 1.0),
            SocketColor::White => Vec4::new(0.9, 0.9, 0.9, 1.0),
            SocketColor::Black => Vec4::new(0.1, 0.0, 0.2, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GemDefinition {
    pub id: u64,
    pub name: String,
    pub socket_color: SocketColor,
    pub modifier: StatModifier,
    pub quality: f32,
    pub item_level_requirement: u32,
    pub icon_path: String,
}

impl GemDefinition {
    pub fn effective_modifier(&self) -> StatModifier {
        let mut m = self.modifier.clone();
        m.flat_value *= self.quality;
        m.percent_value *= self.quality;
        m
    }

    pub fn stat_value(&self) -> f32 {
        (self.modifier.flat_value.abs() + self.modifier.percent_value.abs()) * self.quality
    }
}

#[derive(Debug, Clone)]
pub struct ItemSocket {
    pub color: SocketColor,
    pub gem_id: Option<u64>,
}

impl ItemSocket {
    pub fn empty(color: SocketColor) -> Self {
        ItemSocket { color, gem_id: None }
    }

    pub fn is_filled(&self) -> bool {
        self.gem_id.is_some()
    }

    pub fn can_accept(&self, gem: &GemDefinition) -> bool {
        self.color == SocketColor::White
            || gem.socket_color == SocketColor::White
            || self.color == gem.socket_color
    }

    pub fn insert_gem(&mut self, gem_id: u64) -> Option<u64> {
        let old = self.gem_id;
        self.gem_id = Some(gem_id);
        old
    }

    pub fn remove_gem(&mut self) -> Option<u64> {
        self.gem_id.take()
    }
}

#[derive(Debug, Clone)]
pub struct SocketBonus {
    pub description: String,
    pub modifier: StatModifier,
    pub requires_all_matching: bool,
}

pub fn compute_socket_bonus(sockets: &[ItemSocket], gems: &HashMap<u64, GemDefinition>, bonus: &SocketBonus) -> Option<StatModifier> {
    if bonus.requires_all_matching {
        let all_match = sockets.iter().all(|s| {
            if let Some(gem_id) = s.gem_id {
                gems.get(&gem_id)
                    .map(|g| g.socket_color == s.color || g.socket_color == SocketColor::White || s.color == SocketColor::White)
                    .unwrap_or(false)
            } else {
                false
            }
        });
        if all_match { Some(bonus.modifier.clone()) } else { None }
    } else {
        let any_filled = sockets.iter().any(|s| s.gem_id.is_some());
        if any_filled { Some(bonus.modifier.clone()) } else { None }
    }
}

pub fn compute_all_gem_bonuses(sockets: &[ItemSocket], gems: &HashMap<u64, GemDefinition>) -> Vec<StatModifier> {
    sockets.iter()
        .filter_map(|s| s.gem_id.and_then(|id| gems.get(&id)))
        .map(|g| g.effective_modifier())
        .collect()
}

// ============================================================
// EXTENDED: ITEM UPGRADE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct UpgradePath {
    pub source_item_id: u64,
    pub result_item_id: u64,
    pub required_materials: Vec<(u64, u32)>,
    pub gold_cost: u64,
    pub required_smith_level: u32,
    pub preserves_affixes: bool,
    pub preserves_quality: bool,
    pub success_chance: f32,
    pub failure_downgrade: Option<u64>,
}

impl UpgradePath {
    pub fn success_probability_with_level(&self, smith_level: u32) -> f32 {
        let bonus = ((smith_level as f32 - self.required_smith_level as f32).max(0.0)) * 0.01;
        (self.success_chance + bonus).min(0.99)
    }

    pub fn expected_attempts_to_succeed(&self, smith_level: u32) -> f32 {
        let p = self.success_probability_with_level(smith_level);
        if p <= 0.0 { return f32::INFINITY; }
        1.0 / p
    }

    pub fn expected_gold_cost_per_success(&self, smith_level: u32) -> u64 {
        let attempts = self.expected_attempts_to_succeed(smith_level);
        (self.gold_cost as f32 * attempts) as u64
    }
}

// ============================================================
// EXTENDED: DISENCHANTING SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct DisenchantOutcome {
    pub material_id: u64,
    pub min_quantity: u32,
    pub max_quantity: u32,
    pub chance: f32,
}

pub fn disenchant_item(item: &ItemDefinition, rolls: &[f32]) -> Vec<(u64, u32)> {
    let outcomes: Vec<DisenchantOutcome> = match item.rarity {
        Rarity::Common => vec![
            DisenchantOutcome { material_id: 1001, min_quantity: 1, max_quantity: 3, chance: 1.0 },
        ],
        Rarity::Uncommon => vec![
            DisenchantOutcome { material_id: 1002, min_quantity: 1, max_quantity: 2, chance: 0.8 },
            DisenchantOutcome { material_id: 1001, min_quantity: 2, max_quantity: 5, chance: 0.2 },
        ],
        Rarity::Rare => vec![
            DisenchantOutcome { material_id: 1003, min_quantity: 1, max_quantity: 2, chance: 0.7 },
            DisenchantOutcome { material_id: 1002, min_quantity: 2, max_quantity: 4, chance: 0.3 },
        ],
        Rarity::Epic => vec![
            DisenchantOutcome { material_id: 1004, min_quantity: 1, max_quantity: 1, chance: 0.6 },
            DisenchantOutcome { material_id: 1003, min_quantity: 1, max_quantity: 3, chance: 0.4 },
        ],
        Rarity::Legendary => vec![
            DisenchantOutcome { material_id: 1005, min_quantity: 1, max_quantity: 1, chance: 0.5 },
            DisenchantOutcome { material_id: 1004, min_quantity: 1, max_quantity: 2, chance: 0.5 },
        ],
        Rarity::Mythic => vec![
            DisenchantOutcome { material_id: 1006, min_quantity: 1, max_quantity: 1, chance: 1.0 },
        ],
    };

    let mut results = Vec::new();
    let mut roll_idx = 0usize;

    for outcome in &outcomes {
        let roll = if roll_idx < rolls.len() { rolls[roll_idx] } else { 0.5 };
        roll_idx += 1;
        if roll < outcome.chance {
            let qty_roll = if roll_idx < rolls.len() { rolls[roll_idx] } else { 0.5 };
            roll_idx += 1;
            let range = outcome.max_quantity - outcome.min_quantity;
            let qty = outcome.min_quantity + (qty_roll * (range + 1) as f32) as u32;
            results.push((outcome.material_id, qty.min(outcome.max_quantity)));
        }
    }

    results
}

pub fn disenchant_expected_value(item: &ItemDefinition, material_prices: &HashMap<u64, u64>) -> f64 {
    let rolls: Vec<f32> = vec![0.5; 20];
    let outcomes = disenchant_item(item, &rolls);
    outcomes.iter().map(|(mat_id, qty)| {
        let price = material_prices.get(mat_id).copied().unwrap_or(1);
        price as f64 * *qty as f64
    }).sum()
}

// ============================================================
// EXTENDED: WORLD DROP SCHEDULER
// ============================================================

#[derive(Debug, Clone)]
pub struct WorldDropSchedule {
    pub zone_id: u64,
    pub zone_name: String,
    pub loot_table_id: u64,
    pub boss_loot_table_id: Option<u64>,
    pub respawn_time_minutes: f32,
    pub is_boss_zone: bool,
    pub min_player_level: u32,
    pub max_player_level: u32,
    pub guaranteed_drops: Vec<(u64, f32)>,
}

impl WorldDropSchedule {
    pub fn should_reward(&self, player_level: u32) -> bool {
        player_level >= self.min_player_level && player_level <= self.max_player_level
    }

    pub fn get_loot_table<'a>(&self, is_boss: bool, loot_tables: &'a HashMap<u64, LootTable>) -> Option<&'a LootTable> {
        if is_boss {
            self.boss_loot_table_id
                .and_then(|id| loot_tables.get(&id))
                .or_else(|| loot_tables.get(&self.loot_table_id))
        } else {
            loot_tables.get(&self.loot_table_id)
        }
    }

    pub fn zone_difficulty(&self) -> u32 {
        (self.min_player_level + self.max_player_level) / 2
    }

    pub fn is_level_appropriate(&self, level: u32) -> bool {
        let mid = self.zone_difficulty();
        let delta = if level > mid { level - mid } else { mid - level };
        delta <= 5
    }
}

// ============================================================
// EXTENDED: MERCHANT HAGGLING
// ============================================================

#[derive(Debug, Clone)]
pub struct HaggleState {
    pub base_price: u64,
    pub current_offer: u64,
    pub minimum_accept_price: u64,
    pub rounds_remaining: u32,
    pub vendor_patience: f32,
    pub player_charisma: u32,
    pub outcome: HaggleOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaggleOutcome {
    InProgress,
    Accepted,
    Rejected,
    Abandoned,
}

impl HaggleState {
    pub fn new(base_price: u64, player_charisma: u32) -> Self {
        let charm_discount = 1.0 - (player_charisma as f32 * 0.01).min(0.3);
        let minimum = (base_price as f32 * charm_discount) as u64;
        HaggleState {
            base_price,
            current_offer: base_price,
            minimum_accept_price: minimum,
            rounds_remaining: 3 + player_charisma / 10,
            vendor_patience: 1.0,
            player_charisma,
            outcome: HaggleOutcome::InProgress,
        }
    }

    pub fn make_offer(&mut self, offered_price: u64) -> HaggleResponse {
        if self.outcome != HaggleOutcome::InProgress {
            return HaggleResponse::AlreadyConcluded;
        }
        self.rounds_remaining = self.rounds_remaining.saturating_sub(1);
        let offer_ratio = offered_price as f32 / self.base_price as f32;
        let aggression = 1.0 - offer_ratio;
        self.vendor_patience -= aggression * 0.4;
        self.vendor_patience = self.vendor_patience.max(0.0);

        if offered_price >= self.minimum_accept_price && self.vendor_patience > 0.1 {
            self.current_offer = offered_price;
            self.outcome = HaggleOutcome::Accepted;
            return HaggleResponse::Accept { final_price: offered_price };
        }
        if self.vendor_patience < 0.2 || self.rounds_remaining == 0 {
            self.outcome = HaggleOutcome::Rejected;
            return HaggleResponse::Reject { final_price: self.base_price };
        }
        let counter = ((offered_price as f32 + self.minimum_accept_price as f32) / 2.0) as u64;
        self.current_offer = counter;
        HaggleResponse::Counter { counter_price: counter, rounds_left: self.rounds_remaining }
    }

    pub fn abandon(&mut self) {
        self.outcome = HaggleOutcome::Abandoned;
    }

    pub fn savings_if_accepted(&self) -> i64 {
        self.base_price as i64 - self.current_offer as i64
    }
}

#[derive(Debug, Clone)]
pub enum HaggleResponse {
    Accept { final_price: u64 },
    Reject { final_price: u64 },
    Counter { counter_price: u64, rounds_left: u32 },
    AlreadyConcluded,
}

// ============================================================
// EXTENDED: ITEM HISTORY AND PROVENANCE
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemHistoryEntry {
    pub timestamp: f32,
    pub event: ItemEvent,
    pub player_id: Option<u64>,
    pub location: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ItemEvent {
    Generated { seed: u64 },
    Dropped { from_mob: String },
    Purchased { from_vendor: String, price: u64 },
    Crafted { recipe_name: String },
    Enchanted { enchant_name: String },
    Upgraded { from_tier: u32, to_tier: u32 },
    Repaired { cost: u64 },
    SocketGemInserted { gem_name: String },
    AffixRerolled { old_affix: String, new_affix: String },
    Identified,
    Destroyed,
    Traded { to_player: u64 },
}

#[derive(Debug, Clone)]
pub struct ItemProvenance {
    pub instance_id: u64,
    pub history: Vec<ItemHistoryEntry>,
}

impl ItemProvenance {
    pub fn new(instance_id: u64) -> Self {
        ItemProvenance { instance_id, history: Vec::new() }
    }

    pub fn record(&mut self, event: ItemEvent, time: f32) {
        self.history.push(ItemHistoryEntry {
            timestamp: time,
            event,
            player_id: None,
            location: None,
        });
    }

    pub fn origin_event(&self) -> Option<&ItemHistoryEntry> {
        self.history.first()
    }

    pub fn times_repaired(&self) -> u32 {
        self.history.iter().filter(|e| matches!(e.event, ItemEvent::Repaired { .. })).count() as u32
    }

    pub fn times_traded(&self) -> u32 {
        self.history.iter().filter(|e| matches!(e.event, ItemEvent::Traded { .. })).count() as u32
    }

    pub fn age(&self, current_time: f32) -> f32 {
        self.history.first().map(|e| current_time - e.timestamp).unwrap_or(0.0)
    }
}

// ============================================================
// EXTENDED: MATERIAL COST TABLES
// ============================================================

pub fn material_cost_for_upgrade(item_level: u32, rarity: Rarity) -> HashMap<String, u32> {
    let mut costs = HashMap::new();
    let base = item_level;
    match rarity {
        Rarity::Common => {
            costs.insert("Iron Ore".to_string(), base * 3);
        }
        Rarity::Uncommon => {
            costs.insert("Iron Ore".to_string(), base * 5);
            costs.insert("Magic Dust".to_string(), base);
        }
        Rarity::Rare => {
            costs.insert("Mithril Ore".to_string(), base * 3);
            costs.insert("Arcane Shard".to_string(), base * 2);
        }
        Rarity::Epic => {
            costs.insert("Adamantite".to_string(), base * 2);
            costs.insert("Soul Fragment".to_string(), base);
            costs.insert("Arcane Shard".to_string(), base * 3);
        }
        Rarity::Legendary => {
            costs.insert("Void Steel".to_string(), base);
            costs.insert("Primordial Essence".to_string(), base);
            costs.insert("Dragon Scale".to_string(), base / 2 + 1);
        }
        Rarity::Mythic => {
            costs.insert("Aether Core".to_string(), base);
            costs.insert("Chaos Crystal".to_string(), base * 2);
        }
    }
    costs
}

pub fn crafting_material_value(mat_name: &str, quantity: u32) -> u64 {
    let per_unit: u64 = match mat_name {
        "Iron Ore" => 2,
        "Magic Dust" => 8,
        "Mithril Ore" => 15,
        "Arcane Shard" => 25,
        "Adamantite" => 50,
        "Soul Fragment" => 75,
        "Void Steel" => 200,
        "Primordial Essence" => 350,
        "Dragon Scale" => 500,
        "Aether Core" => 1000,
        "Chaos Crystal" => 800,
        _ => 5,
    };
    per_unit * quantity as u64
}

// ============================================================
// EXTENDED: INVENTORY TRANSACTION LOG
// ============================================================

#[derive(Debug, Clone)]
pub struct InventoryTransaction {
    pub id: u64,
    pub timestamp: f32,
    pub transaction_type: TransactionType,
    pub item_instance_id: u64,
    pub item_def_id: u64,
    pub quantity: u32,
    pub source_container: Option<u64>,
    pub dest_container: Option<u64>,
    pub gold_delta: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Pickup,
    Drop,
    VendorBuy,
    VendorSell,
    Craft,
    Disenchant,
    Upgrade,
    Enchant,
    SocketGem,
    Move,
    Split,
    Merge,
    Destroy,
    QuestTurn,
    Trade,
    Loot,
}

#[derive(Debug, Clone)]
pub struct InventoryTransactionLog {
    pub entries: VecDeque<InventoryTransaction>,
    pub max_entries: usize,
    pub next_id: u64,
    pub total_gold_earned: u64,
    pub total_gold_spent: u64,
    pub items_looted: u64,
    pub items_crafted: u64,
    pub items_sold: u64,
}

impl InventoryTransactionLog {
    pub fn new() -> Self {
        InventoryTransactionLog {
            entries: VecDeque::new(),
            max_entries: 1000,
            next_id: 1,
            total_gold_earned: 0,
            total_gold_spent: 0,
            items_looted: 0,
            items_crafted: 0,
            items_sold: 0,
        }
    }

    pub fn record(&mut self, txn: InventoryTransaction) {
        if txn.gold_delta > 0 {
            self.total_gold_earned += txn.gold_delta as u64;
        } else {
            self.total_gold_spent += (-txn.gold_delta) as u64;
        }
        match txn.transaction_type {
            TransactionType::Loot | TransactionType::Pickup => {
                self.items_looted += txn.quantity as u64;
            }
            TransactionType::Craft => {
                self.items_crafted += txn.quantity as u64;
            }
            TransactionType::VendorSell => {
                self.items_sold += txn.quantity as u64;
            }
            _ => {}
        }
        self.entries.push_back(txn);
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn filter_by_type(&self, tt: TransactionType) -> Vec<&InventoryTransaction> {
        self.entries.iter().filter(|e| e.transaction_type == tt).collect()
    }

    pub fn net_gold(&self) -> i64 {
        self.total_gold_earned as i64 - self.total_gold_spent as i64
    }

    pub fn total_for_item(&self, item_def_id: u64) -> (u32, u32) {
        let gained: u32 = self.entries.iter()
            .filter(|e| e.item_def_id == item_def_id && matches!(
                e.transaction_type,
                TransactionType::Loot | TransactionType::VendorBuy |
                TransactionType::Craft | TransactionType::Pickup
            ))
            .map(|e| e.quantity).sum();
        let lost: u32 = self.entries.iter()
            .filter(|e| e.item_def_id == item_def_id && matches!(
                e.transaction_type,
                TransactionType::VendorSell | TransactionType::Disenchant |
                TransactionType::Destroy | TransactionType::Drop
            ))
            .map(|e| e.quantity).sum();
        (gained, lost)
    }
}

// ============================================================
// EXTENDED: ITEM CATALOG (sorted views)
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemCatalog {
    pub by_id: BTreeMap<u64, ItemDefinition>,
    pub by_name: BTreeMap<String, u64>,
    pub by_category_and_level: BTreeMap<(String, u32), Vec<u64>>,
    pub by_rarity_and_level: BTreeMap<(u8, u32), Vec<u64>>,
}

impl ItemCatalog {
    pub fn build(db: &ItemDatabase) -> Self {
        let mut catalog = ItemCatalog {
            by_id: BTreeMap::new(),
            by_name: BTreeMap::new(),
            by_category_and_level: BTreeMap::new(),
            by_rarity_and_level: BTreeMap::new(),
        };
        for (id, def) in &db.definitions {
            catalog.by_id.insert(*id, def.clone());
            catalog.by_name.insert(def.name.clone(), *id);
            catalog.by_category_and_level
                .entry((def.category.display_name().to_string(), def.item_level))
                .or_insert_with(Vec::new)
                .push(*id);
            let rarity_ord: u8 = match def.rarity {
                Rarity::Common => 0, Rarity::Uncommon => 1, Rarity::Rare => 2,
                Rarity::Epic => 3, Rarity::Legendary => 4, Rarity::Mythic => 5,
            };
            catalog.by_rarity_and_level
                .entry((rarity_ord, def.item_level))
                .or_insert_with(Vec::new)
                .push(*id);
        }
        catalog
    }

    pub fn items_in_level_range(&self, min: u32, max: u32, category: Option<&str>) -> Vec<&ItemDefinition> {
        let mut results = Vec::new();
        for ((cat, level), ids) in &self.by_category_and_level {
            if *level < min || *level > max { continue; }
            if let Some(c) = category { if cat != c { continue; } }
            for id in ids {
                if let Some(def) = self.by_id.get(id) { results.push(def); }
            }
        }
        results.sort_by_key(|d| d.item_level);
        results
    }

    pub fn items_by_rarity(&self, rarity: Rarity) -> Vec<&ItemDefinition> {
        let rarity_ord: u8 = match rarity {
            Rarity::Common => 0, Rarity::Uncommon => 1, Rarity::Rare => 2,
            Rarity::Epic => 3, Rarity::Legendary => 4, Rarity::Mythic => 5,
        };
        let mut results = Vec::new();
        for ((r, _), ids) in &self.by_rarity_and_level {
            if *r != rarity_ord { continue; }
            for id in ids {
                if let Some(def) = self.by_id.get(id) { results.push(def); }
            }
        }
        results
    }

    pub fn find_by_name_prefix(&self, prefix: &str) -> Vec<&ItemDefinition> {
        let lower = prefix.to_lowercase();
        self.by_name.iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&lower))
            .filter_map(|(_, id)| self.by_id.get(id))
            .collect()
    }

    pub fn total_catalog_value(&self) -> u64 {
        self.by_id.values().map(|d| d.base_price).sum()
    }

    pub fn most_valuable_items(&self, n: usize) -> Vec<&ItemDefinition> {
        let mut items: Vec<&ItemDefinition> = self.by_id.values().collect();
        items.sort_by(|a, b| b.base_price.cmp(&a.base_price));
        items.truncate(n);
        items
    }

    pub fn average_item_level_by_rarity(&self) -> HashMap<u8, f32> {
        let mut sums: HashMap<u8, (u64, u32)> = HashMap::new();
        for (_, def) in &self.by_id {
            let r = match def.rarity {
                Rarity::Common => 0u8, Rarity::Uncommon => 1, Rarity::Rare => 2,
                Rarity::Epic => 3, Rarity::Legendary => 4, Rarity::Mythic => 5,
            };
            let e = sums.entry(r).or_insert((0, 0));
            e.0 += def.item_level as u64;
            e.1 += 1;
        }
        sums.iter().map(|(r, (s, c))| (*r, *s as f32 / (*c).max(1) as f32)).collect()
    }
}

// ============================================================
// EXTENDED: LOOT FILTER
// ============================================================

#[derive(Debug, Clone)]
pub struct LootFilterRule {
    pub id: u64,
    pub name: String,
    pub priority: u32,
    pub conditions: Vec<LootFilterCondition>,
    pub action: LootFilterAction,
    pub highlight_color: Option<Vec4>,
    pub play_sound: Option<String>,
    pub label_override: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LootFilterCondition {
    Rarity(Rarity),
    Category(ItemCategory),
    ItemLevelAbove(u32),
    ItemLevelBelow(u32),
    BasePriceAbove(u64),
    StatAbove { stat: StatType, value: f32 },
    DPSAbove(f32),
    WeightBelow(f32),
    IsBoundOnPickup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootFilterAction {
    Show,
    Hide,
    Highlight,
    AutoPickup,
}

impl LootFilterRule {
    pub fn matches(&self, item: &ItemDefinition) -> bool {
        self.conditions.iter().all(|cond| match cond {
            LootFilterCondition::Rarity(r) => item.rarity == *r,
            LootFilterCondition::Category(c) => item.category == *c,
            LootFilterCondition::ItemLevelAbove(l) => item.item_level > *l,
            LootFilterCondition::ItemLevelBelow(l) => item.item_level < *l,
            LootFilterCondition::BasePriceAbove(p) => item.base_price > *p,
            LootFilterCondition::StatAbove { stat, value } => {
                let total: f32 = item.explicit_modifiers.iter()
                    .chain(item.implicit_modifiers.iter())
                    .filter(|m| &m.stat == stat)
                    .map(|m| m.flat_value + m.percent_value)
                    .sum();
                total > *value
            }
            LootFilterCondition::DPSAbove(dps) => item.total_damage_per_second() > *dps,
            LootFilterCondition::WeightBelow(w) => item.weight < *w,
            LootFilterCondition::IsBoundOnPickup => item.bind_on_pickup,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LootFilterProfile {
    pub id: u64,
    pub name: String,
    pub rules: Vec<LootFilterRule>,
    pub default_action: LootFilterAction,
}

impl LootFilterProfile {
    pub fn evaluate(&self, item: &ItemDefinition) -> LootFilterAction {
        let mut sorted_rules: Vec<&LootFilterRule> = self.rules.iter().collect();
        sorted_rules.sort_by_key(|r| r.priority);
        for rule in sorted_rules {
            if rule.matches(item) {
                return rule.action;
            }
        }
        self.default_action
    }

    pub fn all_shown_items<'a>(&self, items: &'a [&ItemDefinition]) -> Vec<&'a ItemDefinition> {
        items.iter().filter(|&&item| {
            !matches!(self.evaluate(item), LootFilterAction::Hide)
        }).copied().collect()
    }

    pub fn autopickup_items<'a>(&self, items: &'a [&ItemDefinition]) -> Vec<&'a ItemDefinition> {
        items.iter().filter(|&&item| {
            matches!(self.evaluate(item), LootFilterAction::AutoPickup)
        }).copied().collect()
    }
}

// ============================================================
// EXTENDED: MULTI-PLAYER TRADE WINDOW
// ============================================================

#[derive(Debug, Clone)]
pub struct TradeWindow {
    pub trade_id: u64,
    pub player_a_id: u64,
    pub player_b_id: u64,
    pub player_a_items: Vec<ItemInstance>,
    pub player_b_items: Vec<ItemInstance>,
    pub player_a_gold: u64,
    pub player_b_gold: u64,
    pub player_a_confirmed: bool,
    pub player_b_confirmed: bool,
    pub state: TradeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeState {
    Open,
    Confirmed,
    Completed,
    Cancelled,
}

impl TradeWindow {
    pub fn new(trade_id: u64, player_a: u64, player_b: u64) -> Self {
        TradeWindow {
            trade_id,
            player_a_id: player_a,
            player_b_id: player_b,
            player_a_items: Vec::new(),
            player_b_items: Vec::new(),
            player_a_gold: 0,
            player_b_gold: 0,
            player_a_confirmed: false,
            player_b_confirmed: false,
            state: TradeState::Open,
        }
    }

    pub fn add_item_a(&mut self, item: ItemInstance, def: &ItemDefinition) -> bool {
        if def.bind_on_pickup { return false; }
        self.player_a_confirmed = false;
        self.player_b_confirmed = false;
        self.player_a_items.push(item);
        true
    }

    pub fn add_item_b(&mut self, item: ItemInstance, def: &ItemDefinition) -> bool {
        if def.bind_on_pickup { return false; }
        self.player_a_confirmed = false;
        self.player_b_confirmed = false;
        self.player_b_items.push(item);
        true
    }

    pub fn set_gold_a(&mut self, gold: u64) {
        self.player_a_gold = gold;
        self.player_a_confirmed = false;
        self.player_b_confirmed = false;
    }

    pub fn set_gold_b(&mut self, gold: u64) {
        self.player_b_gold = gold;
        self.player_a_confirmed = false;
        self.player_b_confirmed = false;
    }

    pub fn confirm(&mut self, player_id: u64) {
        if player_id == self.player_a_id { self.player_a_confirmed = true; }
        if player_id == self.player_b_id { self.player_b_confirmed = true; }
        if self.player_a_confirmed && self.player_b_confirmed {
            self.state = TradeState::Confirmed;
        }
    }

    pub fn complete(&mut self) -> bool {
        if self.state == TradeState::Confirmed {
            self.state = TradeState::Completed;
            true
        } else {
            false
        }
    }

    pub fn cancel(&mut self) {
        self.state = TradeState::Cancelled;
        self.player_a_confirmed = false;
        self.player_b_confirmed = false;
    }

    pub fn value_a(&self, defs: &HashMap<u64, ItemDefinition>) -> u64 {
        let item_value: u64 = self.player_a_items.iter()
            .filter_map(|i| defs.get(&i.definition_id))
            .map(|d| d.base_price).sum();
        item_value + self.player_a_gold
    }

    pub fn value_b(&self, defs: &HashMap<u64, ItemDefinition>) -> u64 {
        let item_value: u64 = self.player_b_items.iter()
            .filter_map(|i| defs.get(&i.definition_id))
            .map(|d| d.base_price).sum();
        item_value + self.player_b_gold
    }

    pub fn value_ratio(&self, defs: &HashMap<u64, ItemDefinition>) -> f32 {
        let va = self.value_a(defs) as f32;
        let vb = self.value_b(defs) as f32;
        if vb < 1.0 { return f32::INFINITY; }
        va / vb
    }

    pub fn is_fair_trade(&self, defs: &HashMap<u64, ItemDefinition>, tolerance: f32) -> bool {
        let ratio = self.value_ratio(defs);
        (ratio - 1.0).abs() <= tolerance
    }
}

// ============================================================
// EXTENDED: ITEM STATISTICS GRAPHS
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemStatGraph {
    pub stat_type: StatType,
    pub data_points: Vec<(f32, f32)>,
    pub regression_slope: f32,
    pub regression_intercept: f32,
    pub r_squared: f32,
}

impl ItemStatGraph {
    pub fn from_database(db: &ItemDatabase, stat_type: StatType) -> Self {
        let mut points: Vec<(f32, f32)> = Vec::new();
        for def in db.definitions.values() {
            let value: f32 = def.explicit_modifiers.iter()
                .chain(def.implicit_modifiers.iter())
                .filter(|m| m.stat == stat_type)
                .map(|m| m.flat_value + m.percent_value)
                .sum();
            if value > 0.0 {
                points.push((def.item_level as f32, value));
            }
        }
        let (slope, intercept, r2) = linear_regression(&points);
        ItemStatGraph {
            stat_type,
            data_points: points,
            regression_slope: slope,
            regression_intercept: intercept,
            r_squared: r2,
        }
    }

    pub fn predict(&self, item_level: f32) -> f32 {
        self.regression_slope * item_level + self.regression_intercept
    }

    pub fn outliers(&self, z_threshold: f32) -> Vec<(f32, f32)> {
        if self.data_points.len() < 3 { return Vec::new(); }
        let mean: f32 = self.data_points.iter().map(|(_, v)| v).sum::<f32>() / self.data_points.len() as f32;
        let variance: f32 = self.data_points.iter().map(|(_, v)| (v - mean).powi(2)).sum::<f32>() / self.data_points.len() as f32;
        let stddev = variance.sqrt();
        if stddev < 1e-6 { return Vec::new(); }
        self.data_points.iter().filter(|(_, v)| {
            ((v - mean) / stddev).abs() > z_threshold
        }).copied().collect()
    }

    pub fn trend_description(&self) -> String {
        if self.regression_slope > 1.0 {
            format!("Strongly increasing: +{:.2} per level", self.regression_slope)
        } else if self.regression_slope > 0.1 {
            format!("Gently increasing: +{:.2} per level", self.regression_slope)
        } else if self.regression_slope < -1.0 {
            format!("Strongly decreasing: {:.2} per level", self.regression_slope)
        } else {
            format!("Roughly flat: {:.2} per level", self.regression_slope)
        }
    }
}

fn linear_regression(points: &[(f32, f32)]) -> (f32, f32, f32) {
    let n = points.len() as f32;
    if n < 2.0 { return (0.0, 0.0, 0.0); }
    let sum_x: f32 = points.iter().map(|(x, _)| x).sum();
    let sum_y: f32 = points.iter().map(|(_, y)| y).sum();
    let sum_xx: f32 = points.iter().map(|(x, _)| x * x).sum();
    let sum_xy: f32 = points.iter().map(|(x, y)| x * y).sum();
    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < 1e-9 { return (0.0, sum_y / n, 0.0); }
    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;
    let mean_y = sum_y / n;
    let ss_tot: f32 = points.iter().map(|(_, y)| (y - mean_y).powi(2)).sum();
    let ss_res: f32 = points.iter().map(|(x, y)| (y - (slope * x + intercept)).powi(2)).sum();
    let r2 = if ss_tot < 1e-9 { 1.0 } else { 1.0 - ss_res / ss_tot };
    (slope, intercept, r2)
}

// ============================================================
// EXTENDED: CRAFTING QUEUE
// ============================================================

#[derive(Debug, Clone)]
pub struct CraftingQueueEntry {
    pub recipe_id: u64,
    pub quantity: u32,
    pub started_at: f32,
    pub estimated_finish: f32,
    pub progress: f32,
    pub worker_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CraftingQueue {
    pub entries: VecDeque<CraftingQueueEntry>,
    pub completed: Vec<CraftingQueueEntry>,
    pub max_concurrent: u32,
    pub current_time: f32,
}

impl CraftingQueue {
    pub fn new(max_concurrent: u32) -> Self {
        CraftingQueue {
            entries: VecDeque::new(),
            completed: Vec::new(),
            max_concurrent,
            current_time: 0.0,
        }
    }

    pub fn queue_recipe(&mut self, recipe_id: u64, quantity: u32, crafting_time: f32) -> bool {
        if self.entries.len() >= 20 { return false; }
        let start = self.current_time;
        let finish = start + crafting_time * quantity as f32;
        self.entries.push_back(CraftingQueueEntry {
            recipe_id,
            quantity,
            started_at: start,
            estimated_finish: finish,
            progress: 0.0,
            worker_id: None,
        });
        true
    }

    pub fn tick(&mut self, dt: f32) -> Vec<CraftingQueueEntry> {
        self.current_time += dt;
        let active = self.max_concurrent as usize;
        for entry in self.entries.iter_mut().take(active) {
            let total_time = entry.estimated_finish - entry.started_at;
            if total_time > 0.0 {
                entry.progress = ((self.current_time - entry.started_at) / total_time).min(1.0);
            }
        }
        let mut finished = Vec::new();
        while let Some(front) = self.entries.front() {
            if self.current_time >= front.estimated_finish {
                if let Some(done) = self.entries.pop_front() {
                    self.completed.push(done.clone());
                    finished.push(done);
                }
            } else {
                break;
            }
        }
        finished
    }

    pub fn estimated_completion_time(&self) -> f32 {
        self.entries.back().map(|e| e.estimated_finish).unwrap_or(self.current_time)
    }

    pub fn total_remaining_time(&self) -> f32 {
        (self.estimated_completion_time() - self.current_time).max(0.0)
    }

    pub fn cancel_recipe(&mut self, recipe_id: u64) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.recipe_id == recipe_id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }
}

// ============================================================
// EXTENDED: DROP ANALYTICS
// ============================================================

#[derive(Debug, Clone)]
pub struct DropAnalytics {
    pub item_id: u64,
    pub times_seen: u32,
    pub times_picked_up: u32,
    pub times_sold: u32,
    pub times_crafted: u32,
    pub times_disenchanted: u32,
    pub average_item_level: f32,
    pub rarity_distribution: HashMap<Rarity, u32>,
    pub first_seen: f32,
    pub last_seen: f32,
}

impl DropAnalytics {
    pub fn new(item_id: u64) -> Self {
        DropAnalytics {
            item_id,
            times_seen: 0,
            times_picked_up: 0,
            times_sold: 0,
            times_crafted: 0,
            times_disenchanted: 0,
            average_item_level: 0.0,
            rarity_distribution: HashMap::new(),
            first_seen: 0.0,
            last_seen: 0.0,
        }
    }

    pub fn pickup_rate(&self) -> f32 {
        if self.times_seen == 0 { return 0.0; }
        self.times_picked_up as f32 / self.times_seen as f32
    }

    pub fn keep_rate(&self) -> f32 {
        let disposed = self.times_sold + self.times_disenchanted;
        if self.times_picked_up == 0 { return 0.0; }
        1.0 - disposed as f32 / self.times_picked_up as f32
    }

    pub fn predominant_rarity(&self) -> Option<Rarity> {
        self.rarity_distribution.iter()
            .max_by_key(|(_, &v)| v)
            .map(|(r, _)| *r)
    }
}

#[derive(Debug, Clone)]
pub struct DropAnalyticsRegistry {
    pub data: HashMap<u64, DropAnalytics>,
    pub total_drops_processed: u64,
}

impl DropAnalyticsRegistry {
    pub fn new() -> Self {
        DropAnalyticsRegistry { data: HashMap::new(), total_drops_processed: 0 }
    }

    pub fn record_drop(&mut self, item_id: u64, item_level: f32, rarity: Rarity, time: f32) {
        self.total_drops_processed += 1;
        let entry = self.data.entry(item_id).or_insert_with(|| {
            let mut a = DropAnalytics::new(item_id);
            a.first_seen = time;
            a
        });
        entry.times_seen += 1;
        entry.last_seen = time;
        let n = entry.times_seen as f32;
        entry.average_item_level = entry.average_item_level * (n - 1.0) / n + item_level / n;
        *entry.rarity_distribution.entry(rarity).or_insert(0) += 1;
    }

    pub fn record_pickup(&mut self, item_id: u64) {
        if let Some(entry) = self.data.get_mut(&item_id) {
            entry.times_picked_up += 1;
        }
    }

    pub fn most_common_drops(&self, top_n: usize) -> Vec<(u64, u32)> {
        let mut sorted: Vec<(u64, u32)> = self.data.iter()
            .map(|(id, a)| (*id, a.times_seen)).collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(top_n);
        sorted
    }

    pub fn rarest_drops(&self, top_n: usize) -> Vec<(u64, u32)> {
        let mut sorted: Vec<(u64, u32)> = self.data.iter()
            .filter(|(_, a)| a.times_seen > 0)
            .map(|(id, a)| (*id, a.times_seen)).collect();
        sorted.sort_by(|a, b| a.1.cmp(&b.1));
        sorted.truncate(top_n);
        sorted
    }

    pub fn average_pickup_rate(&self) -> f32 {
        if self.data.is_empty() { return 0.0; }
        let sum: f32 = self.data.values().map(|a| a.pickup_rate()).sum();
        sum / self.data.len() as f32
    }
}

// ============================================================
// EXTENDED: NAMING HELPERS
// ============================================================

pub fn generate_unique_item_name(base_name: &str, _suffix_tables: &[&str]) -> String {
    let titles = [
        "the Forsaken", "of Doom", "the Eternal", "of Legends", "the Undying",
        "of Chaos", "the Ancient", "the Merciless", "of the Fallen King",
        "of Twilight", "the Immortal", "of the Void", "the Cursed", "the Divine",
        "of the First Age", "the Reborn", "of Ruin", "the Unbroken", "of the Storm",
        "of Shadows",
    ];
    let idx = base_name.len() % titles.len();
    format!("{}, {}", base_name, titles[idx])
}

pub fn apply_name_title(name: &str, title: &str) -> String {
    format!("{} {}", title, name)
}

pub fn is_name_unique_in_database(name: &str, db: &ItemDatabase) -> bool {
    !db.definitions.values().any(|d| d.name == name)
}

pub fn suggest_item_name(category: ItemCategory, rarity: Rarity, db: &ItemDatabase) -> String {
    let seed = category as u64 * 1000 + rarity as u64 * 100 + db.definitions.len() as u64;
    let name = generate_item_name(
        (seed as usize) % WEAPON_PREFIXES.len(),
        ((seed / 2) + 1) as usize % WEAPON_BASES.len(),
        ((seed / 4) + 3) as usize % WEAPON_SUFFIXES.len(),
    );
    if is_name_unique_in_database(&name, db) {
        name
    } else {
        format!("{} ({})", name, db.definitions.len())
    }
}

// ============================================================
// EXTENDED: ITEM TAG SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemTagSystem {
    pub tags: HashMap<String, HashSet<u64>>,
    pub item_tags: HashMap<u64, HashSet<String>>,
}

impl ItemTagSystem {
    pub fn new() -> Self {
        ItemTagSystem {
            tags: HashMap::new(),
            item_tags: HashMap::new(),
        }
    }

    pub fn tag_item(&mut self, item_id: u64, tag: String) {
        self.tags.entry(tag.clone()).or_insert_with(HashSet::new).insert(item_id);
        self.item_tags.entry(item_id).or_insert_with(HashSet::new).insert(tag);
    }

    pub fn untag_item(&mut self, item_id: u64, tag: &str) {
        if let Some(set) = self.tags.get_mut(tag) { set.remove(&item_id); }
        if let Some(set) = self.item_tags.get_mut(&item_id) { set.remove(tag); }
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<u64> {
        self.tags.get(tag).map(|s| s.iter().copied().collect()).unwrap_or_default()
    }

    pub fn tags_for_item(&self, item_id: u64) -> Vec<String> {
        self.item_tags.get(&item_id).map(|s| s.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn items_with_all_tags(&self, tags: &[&str]) -> Vec<u64> {
        if tags.is_empty() { return Vec::new(); }
        let mut result: Option<HashSet<u64>> = None;
        for &tag in tags {
            let set: HashSet<u64> = self.tags.get(tag).map(|s| s.clone()).unwrap_or_default();
            result = Some(match result {
                None => set,
                Some(prev) => prev.intersection(&set).copied().collect(),
            });
        }
        result.map(|s| s.into_iter().collect()).unwrap_or_default()
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.tags.keys().cloned().collect();
        tags.sort();
        tags
    }

    pub fn rename_tag(&mut self, old: &str, new: String) {
        if let Some(items) = self.tags.remove(old) {
            for &item_id in &items {
                if let Some(item_tags) = self.item_tags.get_mut(&item_id) {
                    item_tags.remove(old);
                    item_tags.insert(new.clone());
                }
            }
            self.tags.insert(new, items);
        }
    }
}

// ============================================================
// FINAL ENTRY POINT
// ============================================================

pub fn inventory_editor_main() {
    let mut editor = build_editor();

    // Generate random items
    for i in 0..5u64 {
        editor.generation_params.seed = i * 1337 + 42;
        editor.generation_params.item_level = (i * 3 + 1) as u32;
        editor.generate_random_item();
    }

    // Power curve analysis
    let _curve = generate_power_curve(1, 20);

    // Build a catalog
    let catalog = ItemCatalog::build(&editor.database);
    let _common_items = catalog.items_by_rarity(Rarity::Common);

    // Test loot table
    let ctx = LootContext::default();
    let random_values: Vec<f32> = (0..40).map(|i| (i as f32 * 0.617) % 1.0).collect();
    let tables: Vec<&LootTable> = editor.database.loot_tables.values().collect();
    for table in &tables {
        let _drops = table.roll_drops(&ctx, &random_values);
    }

    // Stat comparison
    let all_defs: Vec<&ItemDefinition> = editor.database.definitions.values().collect();
    if all_defs.len() >= 2 {
        let _table = stat_comparison_table(&all_defs[..2]);
    }

    // Rarity table test
    let rarity_table = RarityDropTable::standard();
    let mf_table = RarityDropTable::magic_find_adjusted(100.0);
    for i in 0u64..10 {
        let _r = rarity_table.sample(i as f64 / 10.0, (i as f64 + 0.5) / 10.0);
    }

    // Tag system usage
    let mut tags = ItemTagSystem::new();
    for (id, def) in &editor.database.definitions {
        if def.rarity >= Rarity::Rare {
            tags.tag_item(*id, "high-value".to_string());
        }
        if def.category.is_weapon() {
            tags.tag_item(*id, "weapon".to_string());
        }
    }

    // Crafting queue simulation
    let mut queue = CraftingQueue::new(2);
    for (id, recipe) in &editor.database.recipes {
        queue.queue_recipe(*id, 1, recipe.crafting_time_seconds);
    }
    for _ in 0..10 {
        let _finished = queue.tick(1.0);
    }

    // Drop analytics
    let mut analytics = DropAnalyticsRegistry::new();
    for (id, def) in &editor.database.definitions {
        analytics.record_drop(*id, def.item_level as f32, def.rarity, 0.0);
    }
    let _most_common = analytics.most_common_drops(5);
}

// ============================================================
// EXTENDED: ITEM MODIFIER REROLL SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct RerollCost {
    pub gold: u64,
    pub materials: Vec<(u64, u32)>,
    pub randomizer_item_id: Option<u64>,
}

impl RerollCost {
    pub fn for_item(def: &ItemDefinition) -> Self {
        let base = (def.base_price as f32 * 0.3) as u64;
        let mat_count = match def.rarity {
            Rarity::Common => 1,
            Rarity::Uncommon => 2,
            Rarity::Rare => 3,
            Rarity::Epic => 5,
            Rarity::Legendary => 8,
            Rarity::Mythic => 15,
        };
        RerollCost {
            gold: base,
            materials: vec![(2001, mat_count)],
            randomizer_item_id: None,
        }
    }
}

pub fn reroll_explicit_modifiers(
    item: &mut ItemDefinition,
    affix_pool: &AffixPool,
    seed: u64,
) {
    let rarity_cfg = RarityConfig::for_rarity(item.rarity);
    item.explicit_modifiers.clear();

    let mut s = seed;
    let num_affixes = rarity_cfg.min_affixes
        + (lcg_f32_local(&mut s) * (rarity_cfg.max_affixes - rarity_cfg.min_affixes + 1) as f32) as u32;

    let eligible_prefixes = affix_pool.eligible(item.category, item.item_level, AffixType::Prefix);
    let eligible_suffixes = affix_pool.eligible(item.category, item.item_level, AffixType::Suffix);

    let half = num_affixes / 2;
    let mut selected_ids: Vec<u64> = Vec::new();
    roll_affixes_into(&mut item.explicit_modifiers, &mut selected_ids, &eligible_prefixes, half, &mut s, affix_pool);
    roll_affixes_into(&mut item.explicit_modifiers, &mut selected_ids, &eligible_suffixes, num_affixes - half, &mut s, affix_pool);
}

fn lcg_f32_local(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 32) as f32) / (u32::MAX as f32)
}

pub fn reroll_single_modifier(
    item: &mut ItemDefinition,
    modifier_index: usize,
    affix_pool: &AffixPool,
    seed: u64,
) -> bool {
    if modifier_index >= item.explicit_modifiers.len() { return false; }

    let mut s = seed;
    let eligible = affix_pool.eligible(item.category, item.item_level, AffixType::Suffix);
    if eligible.is_empty() { return false; }

    let weights: Vec<f64> = eligible.iter().map(|d| d.weight as f64).collect();
    let alias = AliasTable::new(&weights);
    let u1 = lcg_f32_local(&mut s) as f64;
    let u2 = lcg_f32_local(&mut s) as f64;
    let idx = alias.sample(u1, u2);
    let affix = eligible[idx];

    let roll_t = lcg_f32_local(&mut s);
    let mut m = affix.modifier_template.clone();
    m.flat_value = affix.roll_flat_value(roll_t);
    m.percent_value = affix.roll_percent_value(roll_t);
    item.explicit_modifiers[modifier_index] = m;
    true
}

// ============================================================
// EXTENDED: ITEM CORRUPTION SYSTEM
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CorruptionType {
    StatBuff,
    StatNerf,
    AddSocket,
    ChangeRarity,
    AddImplicit,
    CorruptedBlood, // ongoing damage penalty
    ChaosDamageBonus,
    BreakItem,
}

#[derive(Debug, Clone)]
pub struct CorruptionOutcome {
    pub corruption_type: CorruptionType,
    pub weight: f32,
    pub description: String,
}

pub fn corrupt_item(item: &mut ItemDefinition, seed: u64) -> CorruptionType {
    let outcomes = vec![
        CorruptionOutcome { corruption_type: CorruptionType::StatBuff, weight: 25.0, description: "One modifier is enhanced by 20-30%".to_string() },
        CorruptionOutcome { corruption_type: CorruptionType::StatNerf, weight: 25.0, description: "One modifier is reduced by 20-30%".to_string() },
        CorruptionOutcome { corruption_type: CorruptionType::AddSocket, weight: 15.0, description: "An additional socket is added".to_string() },
        CorruptionOutcome { corruption_type: CorruptionType::ChangeRarity, weight: 10.0, description: "Rarity shifts up or down one tier".to_string() },
        CorruptionOutcome { corruption_type: CorruptionType::AddImplicit, weight: 10.0, description: "A powerful but dangerous implicit is added".to_string() },
        CorruptionOutcome { corruption_type: CorruptionType::CorruptedBlood, weight: 8.0, description: "Wearer takes 1% of max health per second as damage".to_string() },
        CorruptionOutcome { corruption_type: CorruptionType::ChaosDamageBonus, weight: 5.0, description: "Adds 10-20% chaos damage bonus to all attacks".to_string() },
        CorruptionOutcome { corruption_type: CorruptionType::BreakItem, weight: 2.0, description: "Item is destroyed".to_string() },
    ];

    let weights: Vec<f64> = outcomes.iter().map(|o| o.weight as f64).collect();
    let alias = AliasTable::new(&weights);
    let mut s = seed;
    let u1 = lcg_f32_local(&mut s) as f64;
    let u2 = lcg_f32_local(&mut s) as f64;
    let idx = alias.sample(u1, u2);
    let ct = outcomes[idx].corruption_type;

    let roll = lcg_f32_local(&mut s);
    match ct {
        CorruptionType::StatBuff => {
            if let Some(m) = item.explicit_modifiers.first_mut() {
                let boost = 1.2 + roll * 0.1;
                m.flat_value *= boost;
                m.percent_value *= boost;
            }
        }
        CorruptionType::StatNerf => {
            if let Some(m) = item.explicit_modifiers.first_mut() {
                let reduce = 0.7 + roll * 0.1;
                m.flat_value *= reduce;
                m.percent_value *= reduce;
            }
        }
        CorruptionType::ChangeRarity => {
            item.rarity = if roll < 0.5 {
                match item.rarity {
                    Rarity::Common => Rarity::Uncommon,
                    Rarity::Uncommon => Rarity::Rare,
                    Rarity::Rare => Rarity::Epic,
                    Rarity::Epic => Rarity::Legendary,
                    Rarity::Legendary => Rarity::Mythic,
                    Rarity::Mythic => Rarity::Mythic,
                }
            } else {
                match item.rarity {
                    Rarity::Mythic => Rarity::Legendary,
                    Rarity::Legendary => Rarity::Epic,
                    Rarity::Epic => Rarity::Rare,
                    Rarity::Rare => Rarity::Uncommon,
                    Rarity::Uncommon => Rarity::Common,
                    Rarity::Common => Rarity::Common,
                }
            };
        }
        CorruptionType::AddImplicit => {
            item.implicit_modifiers.push(StatModifier::percent(StatType::CritChance, 5.0));
            // Add a downside
            item.implicit_modifiers.push(StatModifier::percent(StatType::LifeMax, -10.0));
        }
        CorruptionType::ChaosDamageBonus => {
            item.implicit_modifiers.push(StatModifier::percent(StatType::PhysicalDamage, 10.0 + roll * 10.0));
        }
        _ => {}
    }

    ct
}

// ============================================================
// EXTENDED: ITEM TRANSMUTATION TABLE
// ============================================================

#[derive(Debug, Clone)]
pub struct TransmutationRule {
    pub source_category: ItemCategory,
    pub target_category: ItemCategory,
    pub cost_items: Vec<(u64, u32)>,
    pub success_rate: f32,
    pub preserves_level: bool,
    pub preserves_modifiers: bool,
    pub station_required: Option<String>,
}

impl TransmutationRule {
    pub fn can_apply(&self, item: &ItemDefinition, station: Option<&str>) -> bool {
        if item.category != self.source_category { return false; }
        if let Some(req) = &self.station_required {
            if station.map(|s| s != req.as_str()).unwrap_or(true) { return false; }
        }
        true
    }

    pub fn transmute(&self, item: &ItemDefinition, new_id: u64, seed: u64) -> ItemDefinition {
        let mut new_item = ItemDefinition::new(new_id, &item.name, self.target_category);
        if self.preserves_level {
            new_item.item_level = item.item_level;
            new_item.required_level = item.required_level;
        }
        new_item.rarity = item.rarity;
        if self.preserves_modifiers {
            new_item.explicit_modifiers = item.explicit_modifiers.clone();
            new_item.implicit_modifiers = item.implicit_modifiers.clone();
        }
        new_item
    }
}

// ============================================================
// EXTENDED: ITEM DROP SOUND MAPPING
// ============================================================

pub fn drop_sound_for_item(item: &ItemDefinition) -> &'static str {
    match item.category {
        ItemCategory::Currency => "drop_coin",
        ItemCategory::Gem => "drop_gem",
        ItemCategory::Potion => "drop_potion",
        ItemCategory::Sword | ItemCategory::Dagger | ItemCategory::Axe => "drop_blade",
        ItemCategory::Greatsword | ItemCategory::Greataxe | ItemCategory::Hammer => "drop_heavy_weapon",
        ItemCategory::Bow | ItemCategory::Crossbow => "drop_bow",
        ItemCategory::Staff | ItemCategory::Wand | ItemCategory::Orb | ItemCategory::Tome => "drop_magic",
        ItemCategory::Chestplate | ItemCategory::Helmet | ItemCategory::Greaves => "drop_armor_heavy",
        ItemCategory::Robe | ItemCategory::Hood | ItemCategory::Leggings => "drop_cloth",
        ItemCategory::Ring | ItemCategory::Amulet => "drop_jewel",
        _ => "drop_generic",
    }
}

pub fn drop_visual_for_rarity(rarity: Rarity) -> (Vec4, f32, &'static str) {
    // (glow_color, glow_intensity, particle_vfx)
    match rarity {
        Rarity::Common => (Vec4::new(0.8, 0.8, 0.8, 0.3), 0.2, "drop_common"),
        Rarity::Uncommon => (Vec4::new(0.2, 0.8, 0.2, 0.5), 0.4, "drop_uncommon"),
        Rarity::Rare => (Vec4::new(0.2, 0.4, 1.0, 0.7), 0.6, "drop_rare"),
        Rarity::Epic => (Vec4::new(0.6, 0.1, 0.9, 0.8), 0.8, "drop_epic"),
        Rarity::Legendary => (Vec4::new(1.0, 0.5, 0.0, 1.0), 1.0, "drop_legendary"),
        Rarity::Mythic => (Vec4::new(1.0, 0.0, 0.3, 1.0), 1.5, "drop_mythic"),
    }
}

// ============================================================
// EXTENDED: INVENTORY WEIGHT SIMULATION
// ============================================================

pub fn simulate_inventory_weight(
    containers: &[&Container],
    travel_speed_normal: f32,
) -> InventoryWeightReport {
    let total_weight: f32 = containers.iter().map(|c| c.current_weight).sum();
    let max_weight: f32 = containers.iter().map(|c| c.max_weight).sum();
    let fraction = if max_weight > 0.0 { total_weight / max_weight } else { 0.0 };

    // Speed penalty formula: linear drop, 50% speed at 100% encumbrance, 0% at 150%
    let speed_multiplier = if fraction <= 0.5 {
        1.0
    } else if fraction <= 1.0 {
        1.0 - (fraction - 0.5) * 0.5 / 0.5
    } else {
        0.0 // immobile
    };

    let effective_speed = travel_speed_normal * speed_multiplier;
    let heavy_items: Vec<usize> = containers.iter().enumerate()
        .flat_map(|(i, c)| c.items.values().map(move |inst| i))
        .collect();

    InventoryWeightReport {
        total_weight,
        max_weight,
        weight_fraction: fraction,
        speed_multiplier,
        effective_speed,
        is_encumbered: fraction > 0.7,
        is_overloaded: fraction > 1.0,
    }
}

#[derive(Debug, Clone)]
pub struct InventoryWeightReport {
    pub total_weight: f32,
    pub max_weight: f32,
    pub weight_fraction: f32,
    pub speed_multiplier: f32,
    pub effective_speed: f32,
    pub is_encumbered: bool,
    pub is_overloaded: bool,
}

// ============================================================
// EXTENDED: ITEM ATTRIBUTE INHERITANCE (set crafting)
// ============================================================

pub fn inherit_attributes_from_parent(
    child: &mut ItemDefinition,
    parent: &ItemDefinition,
    inherit_mask: AttributeInheritMask,
) {
    if inherit_mask.rarity { child.rarity = parent.rarity; }
    if inherit_mask.item_level { child.item_level = parent.item_level; }
    if inherit_mask.implicit_modifiers { child.implicit_modifiers = parent.implicit_modifiers.clone(); }
    if inherit_mask.explicit_modifiers { child.explicit_modifiers = parent.explicit_modifiers.clone(); }
    if inherit_mask.sockets { /* would copy sockets */ }
    if inherit_mask.enchantments { /* would copy enchants */ }
    if inherit_mask.quality_tier {
        // Inherit quality via sell price ratio
        child.sell_price_ratio = parent.sell_price_ratio;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AttributeInheritMask {
    pub rarity: bool,
    pub item_level: bool,
    pub implicit_modifiers: bool,
    pub explicit_modifiers: bool,
    pub sockets: bool,
    pub enchantments: bool,
    pub quality_tier: bool,
}

impl AttributeInheritMask {
    pub fn all() -> Self {
        AttributeInheritMask {
            rarity: true, item_level: true, implicit_modifiers: true,
            explicit_modifiers: true, sockets: true, enchantments: true,
            quality_tier: true,
        }
    }

    pub fn none() -> Self {
        AttributeInheritMask {
            rarity: false, item_level: false, implicit_modifiers: false,
            explicit_modifiers: false, sockets: false, enchantments: false,
            quality_tier: false,
        }
    }

    pub fn affixes_only() -> Self {
        let mut m = AttributeInheritMask::none();
        m.implicit_modifiers = true;
        m.explicit_modifiers = true;
        m
    }
}

// ============================================================
// EXTENDED: ITEM SEARCH TOKENIZER
// ============================================================

pub struct ItemSearchTokenizer {
    pub stopwords: HashSet<String>,
}

impl ItemSearchTokenizer {
    pub fn new() -> Self {
        let stops = ["the", "of", "a", "an", "and", "or", "in", "on", "at"]
            .iter().map(|s| s.to_string()).collect();
        ItemSearchTokenizer { stopwords: stops }
    }

    pub fn tokenize(&self, text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|w| w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty() && !self.stopwords.contains(w))
            .collect()
    }

    pub fn build_index(&self, defs: &HashMap<u64, ItemDefinition>) -> HashMap<String, Vec<u64>> {
        let mut idx: HashMap<String, Vec<u64>> = HashMap::new();
        for (id, def) in defs {
            for token in self.tokenize(&def.name) {
                idx.entry(token).or_insert_with(Vec::new).push(*id);
            }
            for token in self.tokenize(&def.description) {
                idx.entry(token).or_insert_with(Vec::new).push(*id);
            }
        }
        // Remove duplicates
        for list in idx.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        idx
    }

    pub fn search_index(&self, index: &HashMap<String, Vec<u64>>, query: &str) -> Vec<u64> {
        let tokens = self.tokenize(query);
        if tokens.is_empty() { return Vec::new(); }

        let sets: Vec<&Vec<u64>> = tokens.iter()
            .filter_map(|t| index.get(t))
            .collect();
        if sets.is_empty() { return Vec::new(); }

        // Intersection
        let mut result: HashSet<u64> = sets[0].iter().copied().collect();
        for set in &sets[1..] {
            let s: HashSet<u64> = set.iter().copied().collect();
            result = result.intersection(&s).copied().collect();
        }
        let mut v: Vec<u64> = result.into_iter().collect();
        v.sort();
        v
    }

    pub fn fuzzy_search(&self, index: &HashMap<String, Vec<u64>>, query: &str, max_edit_dist: usize) -> Vec<u64> {
        let query_tokens = self.tokenize(query);
        let mut results: HashSet<u64> = HashSet::new();
        for token in &query_tokens {
            for (key, ids) in index {
                if edit_distance(token, key) <= max_edit_dist {
                    results.extend(ids.iter().copied());
                }
            }
        }
        let mut v: Vec<u64> = results.into_iter().collect();
        v.sort();
        v
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 { return m; }
    if m == 0 { return n; }
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n { dp[i][0] = i; }
    for j in 0..=m { dp[0][j] = j; }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if a[i-1] == b[j-1] { 0 } else { 1 };
            dp[i][j] = (dp[i-1][j] + 1).min(dp[i][j-1] + 1).min(dp[i-1][j-1] + cost);
        }
    }
    dp[n][m]
}

// ============================================================
// EXTENDED: VENDOR PRICE SIMULATION OVER TIME
// ============================================================

#[derive(Debug, Clone)]
pub struct PriceSimPoint {
    pub time: f32,
    pub price: u64,
    pub supply: f32,
    pub demand: f32,
}

pub fn simulate_vendor_prices(
    vendor: &mut VendorDefinition,
    item: &ItemDefinition,
    duration_hours: f32,
    events: &[(f32, EconomyTransaction)], // (time, event)
) -> Vec<PriceSimPoint> {
    let mut history = Vec::new();
    let step = 1.0f32;
    let steps = (duration_hours / step) as u32;
    let mut event_idx = 0;

    for s in 0..steps {
        let t = s as f32 * step;

        // Process events at this time
        while event_idx < events.len() && events[event_idx].0 <= t {
            vendor.adjust_supply_demand(events[event_idx].1.clone());
            event_idx += 1;
        }

        vendor.refresh_inventory(t);
        let price = vendor.buy_price(item);
        history.push(PriceSimPoint {
            time: t,
            price,
            supply: vendor.supply,
            demand: vendor.demand,
        });
    }

    history
}

// ============================================================
// EXTENDED: ITEM DURABILITY DEGRADATION
// ============================================================

#[derive(Debug, Clone)]
pub struct DurabilitySystem {
    pub base_loss_per_hit: f32,
    pub base_loss_per_death: f32,
    pub level_scaling: f32,  // higher level items lose less per hit
    pub repair_efficiency: f32, // fraction of max durability restored per repair
    pub indestructible_threshold: f32, // below this % the item breaks but isn't destroyed
}

impl DurabilitySystem {
    pub fn standard() -> Self {
        DurabilitySystem {
            base_loss_per_hit: 0.01,
            base_loss_per_death: 5.0,
            level_scaling: 0.02,
            repair_efficiency: 1.0,
            indestructible_threshold: 0.0,
        }
    }

    pub fn hit_damage(&self, item_level: u32) -> f32 {
        self.base_loss_per_hit * (1.0 - self.level_scaling * item_level as f32 * 0.01).max(0.1)
    }

    pub fn apply_hit(&self, instance: &mut ItemInstance, item_level: u32) {
        let loss = self.hit_damage(item_level);
        instance.durability = (instance.durability - loss).max(0.0);
    }

    pub fn apply_death(&self, instance: &mut ItemInstance) {
        instance.durability = (instance.durability - self.base_loss_per_death).max(0.0);
    }

    pub fn repair(&self, instance: &mut ItemInstance) {
        instance.durability = (instance.durability + 100.0 * self.repair_efficiency).min(100.0);
    }

    pub fn partial_repair(&self, instance: &mut ItemInstance, fraction: f32) {
        let gain = 100.0 * self.repair_efficiency * fraction.clamp(0.0, 1.0);
        instance.durability = (instance.durability + gain).min(100.0);
    }
}

// ============================================================
// EXTENDED: CRAFTING SKILL PROGRESSION
// ============================================================

#[derive(Debug, Clone)]
pub struct CraftingSkill {
    pub name: String,
    pub current_level: u32,
    pub current_xp: u64,
    pub xp_to_next_level: u64,
    pub total_xp: u64,
    pub max_level: u32,
    pub recipes_unlocked_at: HashMap<u32, Vec<u64>>, // level -> recipe ids
    pub special_perks_at: HashMap<u32, String>, // level -> perk description
}

impl CraftingSkill {
    pub fn new(name: impl Into<String>) -> Self {
        let mut skill = CraftingSkill {
            name: name.into(),
            current_level: 1,
            current_xp: 0,
            xp_to_next_level: 100,
            total_xp: 0,
            max_level: 100,
            recipes_unlocked_at: HashMap::new(),
            special_perks_at: HashMap::new(),
        };
        skill.special_perks_at.insert(10, "+5% crafting success rate".to_string());
        skill.special_perks_at.insert(25, "Can craft Rare quality items".to_string());
        skill.special_perks_at.insert(50, "+10% crafting speed".to_string());
        skill.special_perks_at.insert(75, "Can craft Epic quality items".to_string());
        skill.special_perks_at.insert(100, "Grandmaster: Can craft Legendary quality items".to_string());
        skill
    }

    pub fn xp_required_for_level(level: u32) -> u64 {
        // Exponential growth: xp = 100 * 1.15^(level-1)
        (100.0 * (1.15f64).powi(level as i32 - 1)) as u64
    }

    pub fn add_xp(&mut self, xp: u64) -> Vec<u32> {
        let mut levels_gained = Vec::new();
        self.current_xp += xp;
        self.total_xp += xp;

        while self.current_xp >= self.xp_to_next_level && self.current_level < self.max_level {
            self.current_xp -= self.xp_to_next_level;
            self.current_level += 1;
            levels_gained.push(self.current_level);
            self.xp_to_next_level = Self::xp_required_for_level(self.current_level + 1);
        }

        levels_gained
    }

    pub fn level_fraction(&self) -> f32 {
        if self.xp_to_next_level == 0 { return 1.0; }
        self.current_xp as f32 / self.xp_to_next_level as f32
    }

    pub fn success_rate_bonus(&self) -> f32 {
        // +0.5% per 10 levels
        (self.current_level as f32 / 10.0) * 0.5
    }

    pub fn quality_bonus(&self) -> f32 {
        // +1% quality multiplier per 5 levels
        self.current_level as f32 * 0.01
    }
}

// ============================================================
// EXTENDED: LOADOUT SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct EquipmentLoadout {
    pub id: u64,
    pub name: String,
    pub slots: HashMap<EquipmentSlot, Option<ItemInstance>>,
    pub total_weight: f32,
    pub stat_totals: HashMap<StatType, f32>,
    pub active_set_piece_counts: HashMap<u64, u32>,
}

impl EquipmentLoadout {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        let mut slots = HashMap::new();
        for &slot in EquipmentSlot::all() {
            slots.insert(slot, None);
        }
        EquipmentLoadout {
            id,
            name: name.into(),
            slots,
            total_weight: 0.0,
            stat_totals: HashMap::new(),
            active_set_piece_counts: HashMap::new(),
        }
    }

    pub fn equip(&mut self, slot: EquipmentSlot, inst: ItemInstance, def: &ItemDefinition) -> Option<ItemInstance> {
        let old = self.slots.insert(slot, Some(inst));
        self.recalculate_totals_simple(def);
        old.flatten()
    }

    pub fn unequip(&mut self, slot: EquipmentSlot) -> Option<ItemInstance> {
        self.slots.get_mut(&slot).and_then(|s| s.take())
    }

    pub fn recalculate_totals_simple(&mut self, last_def: &ItemDefinition) {
        // Simplified: just add the last def's modifiers
        for m in &last_def.explicit_modifiers {
            *self.stat_totals.entry(m.stat.clone()).or_insert(0.0) += m.flat_value;
        }
        self.total_weight += last_def.weight;
    }

    pub fn is_slot_filled(&self, slot: EquipmentSlot) -> bool {
        self.slots.get(&slot).and_then(|s| s.as_ref()).is_some()
    }

    pub fn empty_slots(&self) -> Vec<EquipmentSlot> {
        EquipmentSlot::all().iter()
            .filter(|&&slot| !self.is_slot_filled(slot))
            .copied()
            .collect()
    }

    pub fn equipped_count(&self) -> u32 {
        self.slots.values().filter(|s| s.is_some()).count() as u32
    }

    pub fn loadout_score(&self, defs: &HashMap<u64, ItemDefinition>) -> f32 {
        let mut score = 0.0f32;
        for inst in self.slots.values().filter_map(|s| s.as_ref()) {
            if let Some(def) = defs.get(&inst.definition_id) {
                score += item_power_score(def) * inst.quality;
            }
        }
        score
    }
}

// ============================================================
// EXTENDED: LOOT TABLE VISUAL EDITOR HELPERS
// ============================================================

#[derive(Debug, Clone)]
pub struct LootTableEditorLayout {
    pub entry_rects: Vec<Vec4>,       // one per entry
    pub probability_bars: Vec<Vec4>,  // visual bar for each entry's probability
    pub add_button_rect: Vec4,
    pub simulate_button_rect: Vec4,
    pub total_weight: f32,
}

impl LootTableEditorLayout {
    pub fn compute(table: &LootTable, panel_rect: Vec4) -> Self {
        let x = panel_rect.x + 8.0;
        let mut y = panel_rect.y + 40.0;
        let row_h = 30.0;
        let w = panel_rect.z - 16.0;
        let total_weight: f32 = table.entries.iter().filter(|e| !e.guaranteed).map(|e| e.weight).sum();

        let mut entry_rects = Vec::new();
        let mut prob_bars = Vec::new();

        for entry in &table.entries {
            entry_rects.push(Vec4::new(x, y, w, row_h));
            let prob = if entry.guaranteed { 1.0 } else if total_weight > 0.0 { entry.weight / total_weight } else { 0.0 };
            let bar_w = (w - 120.0) * prob;
            prob_bars.push(Vec4::new(x + 120.0, y + 8.0, bar_w, row_h - 16.0));
            y += row_h + 4.0;
        }

        let add_btn = Vec4::new(x, y + 8.0, 80.0, 24.0);
        let sim_btn = Vec4::new(x + 90.0, y + 8.0, 100.0, 24.0);

        LootTableEditorLayout {
            entry_rects,
            probability_bars: prob_bars,
            add_button_rect: add_btn,
            simulate_button_rect: sim_btn,
            total_weight,
        }
    }
}

// ============================================================
// EXTENDED: STAT PROGRESSION PREVIEW
// ============================================================

pub fn stat_projection_over_levels(
    stat: StatType,
    min_level: u32,
    max_level: u32,
    rarity: Rarity,
) -> Vec<(u32, f32)> {
    (min_level..=max_level).map(|lvl| {
        let expected = budget_expectation(lvl, rarity);
        let cost = stat_budget_cost(&stat);
        let allocated_to_stat = expected / (4.0 * cost); // rough portion
        (lvl, allocated_to_stat)
    }).collect()
}

pub fn compute_ideal_item_set_for_level(level: u32) -> HashMap<EquipmentSlot, (Rarity, f32)> {
    // Returns (rarity, expected_power) for each slot at given level
    let mut recommendation = HashMap::new();
    let rarity = if level < 10 { Rarity::Common }
        else if level < 25 { Rarity::Uncommon }
        else if level < 50 { Rarity::Rare }
        else if level < 75 { Rarity::Epic }
        else { Rarity::Legendary };

    for &slot in EquipmentSlot::all() {
        let power = budget_expectation(level, rarity);
        recommendation.insert(slot, (rarity, power));
    }
    recommendation
}

// ============================================================
// EXTENDED: GOLD SINK SIMULATION
// ============================================================

#[derive(Debug, Clone)]
pub struct GoldSinkSimulator {
    pub repair_cost_per_session: u64,
    pub vendor_purchases_per_session: u64,
    pub crafting_cost_per_session: u64,
    pub enchanting_cost_per_session: u64,
    pub trading_fees_per_session: u64,
    pub tax_rate: f32, // fraction of vendor sales taken as tax
}

impl GoldSinkSimulator {
    pub fn default_economy() -> Self {
        GoldSinkSimulator {
            repair_cost_per_session: 50,
            vendor_purchases_per_session: 200,
            crafting_cost_per_session: 150,
            enchanting_cost_per_session: 100,
            trading_fees_per_session: 30,
            tax_rate: 0.05,
        }
    }

    pub fn total_gold_sunk_per_session(&self, vendor_sales: u64) -> u64 {
        let tax = (vendor_sales as f32 * self.tax_rate) as u64;
        self.repair_cost_per_session
            + self.vendor_purchases_per_session
            + self.crafting_cost_per_session
            + self.enchanting_cost_per_session
            + self.trading_fees_per_session
            + tax
    }

    pub fn gold_income_per_session(&self, mobs_killed: u32, avg_gold_per_mob: u64, vendor_sales: u64) -> u64 {
        mobs_killed as u64 * avg_gold_per_mob + vendor_sales
    }

    pub fn net_gold_per_session(&self, mobs_killed: u32, avg_gold_per_mob: u64, vendor_sales: u64) -> i64 {
        let income = self.gold_income_per_session(mobs_killed, avg_gold_per_mob, vendor_sales) as i64;
        let sunk = self.total_gold_sunk_per_session(vendor_sales) as i64;
        income - sunk
    }

    pub fn inflation_factor(&self, sessions: u32, mobs_per_session: u32, avg_gold: u64, vendor_sales: u64) -> f32 {
        let net = self.net_gold_per_session(mobs_per_session, avg_gold, vendor_sales);
        // Positive net = inflation over time
        let total_net = net * sessions as i64;
        if total_net > 0 {
            1.0 + (total_net as f32 / (1_000_000.0 * sessions as f32)).min(2.0)
        } else {
            1.0
        }
    }
}

// ============================================================
// EXTENDED: ITEM COMPARISON MATRIX
// ============================================================

pub fn build_comparison_matrix(items: &[&ItemDefinition]) -> Vec<Vec<f32>> {
    let n = items.len();
    let mut matrix = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in 0..n {
            if i == j { matrix[i][j] = 1.0; continue; }
            let score_i = item_power_score(items[i]);
            let score_j = item_power_score(items[j]);
            if score_j > 0.0 {
                matrix[i][j] = score_i / score_j; // ratio > 1 means i is better than j
            }
        }
    }
    matrix
}

pub fn best_item_for_slot<'a>(
    slot: EquipmentSlot,
    available: &[&'a ItemDefinition],
    player_level: u32,
) -> Option<&'a ItemDefinition> {
    available.iter()
        .filter(|&&def| {
            slot.is_compatible(def.category) &&
            def.requirements.level <= player_level
        })
        .max_by(|a, b| {
            item_power_score(a).partial_cmp(&item_power_score(b)).unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
}

// ============================================================
// EXTENDED: FULL DATABASE PERSISTENCE HELPERS
// ============================================================

pub fn serialize_item_to_string(item: &ItemDefinition) -> String {
    let mods: String = item.explicit_modifiers.iter().map(|m| {
        format!("{}:{}:{}", m.stat.display_name(), m.flat_value, m.percent_value)
    }).collect::<Vec<_>>().join(",");

    format!(
        "id={};name={};cat={};rarity={};ilvl={};price={};weight={};w={};h={};mods=[{}]",
        item.id,
        item.name,
        item.category.display_name(),
        RarityConfig::for_rarity(item.rarity).display_name(),
        item.item_level,
        item.base_price,
        item.weight,
        item.width,
        item.height,
        mods
    )
}

pub fn count_items_by_slot_compatibility(db: &ItemDatabase) -> HashMap<String, u32> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for def in db.definitions.values() {
        for &slot in EquipmentSlot::all() {
            if slot.is_compatible(def.category) {
                *counts.entry(slot.display_name().to_string()).or_insert(0) += 1;
                break;
            }
        }
    }
    counts
}

pub fn database_integrity_check(db: &ItemDatabase) -> Vec<String> {
    let mut issues = Vec::new();

    // Check recipes reference valid items
    for recipe in db.recipes.values() {
        if !db.definitions.contains_key(&recipe.output_item_id) {
            issues.push(format!("Recipe {} references non-existent output item {}", recipe.name, recipe.output_item_id));
        }
        for ing in &recipe.ingredients {
            if !db.definitions.contains_key(&ing.item_id) {
                issues.push(format!("Recipe {} references non-existent ingredient {}", recipe.name, ing.item_id));
            }
        }
    }

    // Check loot tables reference valid items
    for table in db.loot_tables.values() {
        for entry in &table.entries {
            if !db.definitions.contains_key(&entry.item_id) {
                issues.push(format!("Loot table {} references non-existent item {}", table.name, entry.item_id));
            }
        }
    }

    // Check item sets reference valid items
    for set in db.item_sets.values() {
        for &item_id in &set.item_ids {
            if !db.definitions.contains_key(&item_id) {
                issues.push(format!("Set {} references non-existent item {}", set.name, item_id));
            }
        }
    }

    // Check item ID uniqueness
    let mut ids: HashSet<u64> = HashSet::new();
    for &id in db.definitions.keys() {
        if !ids.insert(id) {
            issues.push(format!("Duplicate item ID: {}", id));
        }
    }

    issues
}

// ============================================================
// EXTENDED: ITEM LOCALIZATION SUPPORT
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemLocalization {
    pub item_id: u64,
    pub locale: String,
    pub name: String,
    pub description: String,
    pub flavor_text: String,
}

#[derive(Debug, Clone)]
pub struct LocalizationDatabase {
    pub entries: HashMap<(u64, String), ItemLocalization>,
    pub supported_locales: Vec<String>,
    pub fallback_locale: String,
}

impl LocalizationDatabase {
    pub fn new() -> Self {
        LocalizationDatabase {
            entries: HashMap::new(),
            supported_locales: vec!["en".to_string(), "fr".to_string(), "de".to_string(), "ja".to_string()],
            fallback_locale: "en".to_string(),
        }
    }

    pub fn add_entry(&mut self, entry: ItemLocalization) {
        self.entries.insert((entry.item_id, entry.locale.clone()), entry);
    }

    pub fn get_name(&self, item_id: u64, locale: &str) -> Option<&str> {
        self.entries.get(&(item_id, locale.to_string()))
            .map(|e| e.name.as_str())
    }

    pub fn get_localized_name(&self, item: &ItemDefinition, locale: &str) -> String {
        self.get_name(item.id, locale)
            .or_else(|| self.get_name(item.id, &self.fallback_locale))
            .map(|s| s.to_string())
            .unwrap_or_else(|| item.name.clone())
    }

    pub fn missing_translations(&self, db: &ItemDatabase, locale: &str) -> Vec<u64> {
        db.definitions.keys()
            .filter(|&&id| !self.entries.contains_key(&(id, locale.to_string())))
            .copied()
            .collect()
    }

    pub fn localization_coverage(&self, db: &ItemDatabase, locale: &str) -> f32 {
        let missing = self.missing_translations(db, locale).len();
        let total = db.definitions.len();
        if total == 0 { return 1.0; }
        1.0 - missing as f32 / total as f32
    }
}

// ============================================================
// EXTENDED: BATCH IMPORT VALIDATION
// ============================================================

#[derive(Debug, Clone)]
pub struct BatchImportRecord {
    pub row_index: usize,
    pub raw_data: String,
    pub parsed_item: Option<ItemDefinition>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn validate_batch_import(records: &mut Vec<BatchImportRecord>, db: &ItemDatabase) {
    for record in records.iter_mut() {
        if let Some(ref item) = record.parsed_item {
            // Check for duplicate IDs
            if db.definitions.contains_key(&item.id) {
                record.warnings.push(format!("Item ID {} already exists — will overwrite", item.id));
            }
            // Run item validation
            let errs = item.validate();
            record.errors.extend(errs);
            // Check balance
            let report = validate_item_balance(item);
            if report.is_overpowered {
                record.warnings.push(format!("Item appears overpowered: budget={:.1}, expected={:.1}", report.total_budget, report.budget_level_expected));
            }
            if report.is_underpowered {
                record.warnings.push(format!("Item appears underpowered: budget={:.1}, expected={:.1}", report.total_budget, report.budget_level_expected));
            }
        } else {
            record.errors.push("Failed to parse item data".to_string());
        }
    }
}

pub fn batch_import_summary(records: &[BatchImportRecord]) -> BatchImportSummary {
    let total = records.len();
    let valid = records.iter().filter(|r| r.errors.is_empty() && r.parsed_item.is_some()).count();
    let with_warnings = records.iter().filter(|r| !r.warnings.is_empty()).count();
    let failed = records.iter().filter(|r| !r.errors.is_empty()).count();

    BatchImportSummary { total, valid, with_warnings, failed }
}

#[derive(Debug, Clone)]
pub struct BatchImportSummary {
    pub total: usize,
    pub valid: usize,
    pub with_warnings: usize,
    pub failed: usize,
}


#[derive(Debug, Clone)]
pub struct EnchantmentRegistry {
    pub enchantments: HashMap<u64, EnchantmentDefinition>,
}

impl EnchantmentRegistry {
    pub fn new() -> Self { Self { enchantments: HashMap::new() } }

    pub fn register(&mut self, enchant: EnchantmentDefinition) {
        self.enchantments.insert(enchant.id, enchant);
    }

    pub fn applicable_to_slot(&self, slot: &EquipmentSlot) -> Vec<&EnchantmentDefinition> {
        self.enchantments.values().filter(|e| e.applicable_slots.contains(slot)).collect()
    }

    pub fn can_apply(&self, enchant_id: u64, current_enchants: &[u64]) -> bool {
        let enchant = match self.enchantments.get(&enchant_id) { Some(e) => e, None => return false };
        for existing_id in current_enchants {
            if enchant.incompatible_with.contains(existing_id) { return false; }
            if let Some(existing) = self.enchantments.get(existing_id) {
                if existing.incompatible_with.contains(&enchant_id) { return false; }
            }
        }
        true
    }

    pub fn total_cost(&self, enchant_ids: &[u64]) -> f32 {
        enchant_ids.iter().filter_map(|id| self.enchantments.get(id)).map(|e| e.cost_gold as f32).sum()
    }
}

// ============================================================
// SECTION: ITEM AUDIT LOG
// ============================================================

#[derive(Debug, Clone)]
pub enum ItemHistoryEvent {
    Created { seed: u64 },
    ModifiedField { field: String, old_value: String, new_value: String },
    Corrupted { corruption_type: String },
    Enchanted { enchant_id: u64 },
    Socketed { socket_index: usize, gem_id: u64 },
    Repaired { durability_restored: f32 },
    Upgraded { upgrade_level: u32 },
    Traded { from_player: u64, to_player: u64 },
}

#[derive(Debug, Clone)]
pub struct ItemAuditEntry {
    pub timestamp: u64,
    pub event: ItemHistoryEvent,
    pub actor_id: u64,
}

#[derive(Debug, Clone)]
pub struct ItemAuditLog {
    pub item_id: u64,
    pub entries: Vec<ItemAuditEntry>,
}

impl ItemAuditLog {
    pub fn new(item_id: u64) -> Self { Self { item_id, entries: Vec::new() } }

    pub fn record(&mut self, timestamp: u64, event: ItemHistoryEvent, actor_id: u64) {
        self.entries.push(ItemAuditEntry { timestamp, event, actor_id });
    }

    pub fn count_event_type(&self, event_type: &str) -> usize {
        self.entries.iter().filter(|e| {
            let ty = match &e.event {
                ItemHistoryEvent::Created { .. } => "Created",
                ItemHistoryEvent::ModifiedField { .. } => "ModifiedField",
                ItemHistoryEvent::Corrupted { .. } => "Corrupted",
                ItemHistoryEvent::Enchanted { .. } => "Enchanted",
                ItemHistoryEvent::Socketed { .. } => "Socketed",
                ItemHistoryEvent::Repaired { .. } => "Repaired",
                ItemHistoryEvent::Upgraded { .. } => "Upgraded",
                ItemHistoryEvent::Traded { .. } => "Traded",
            };
            ty == event_type
        }).count()
    }

    pub fn last_traded(&self) -> Option<(u64, u64)> {
        self.entries.iter().rev().find_map(|e| {
            if let ItemHistoryEvent::Traded { from_player, to_player } = &e.event {
                Some((*from_player, *to_player))
            } else { None }
        })
    }
}

// ============================================================
// SECTION: ARMOR NAME GENERATION TABLES
// ============================================================

pub const ARMOR_PREFIXES: &[&str] = &[
    "Adamantine", "Ancient", "Arcane", "Astral", "Awakened", "Azure", "Battleforged",
    "Blessed", "Bound", "Brilliant", "Bronze", "Burning", "Carved", "Celestial",
    "Chained", "Chromatic", "Cobalt", "Consecrated", "Corrupted", "Crystalline",
    "Cursed", "Dark", "Dawnforged", "Deadened", "Defiant", "Dense", "Divine",
    "Draconic", "Dread", "Dusk", "Echoing", "Elder", "Embossed", "Empowered",
    "Enchanted", "Ethereal", "Exalted", "Fabled", "Fallen", "Flared", "Flickering",
    "Forged", "Frostborn", "Ghostly", "Gilded", "Glorious", "Glowing", "Golden",
    "Granite", "Hallowed", "Hardened", "Haunted", "Heavy", "Heralded", "Hollow",
    "Holy", "Honored", "Horned", "Imbued", "Immortal", "Imperial", "Infernal",
    "Inlaid", "Iron", "Jade", "Jagged", "Jeweled", "Kingly", "Lacquered",
    "Layered", "Leathered", "Light", "Lightning", "Linked", "Living", "Lunar",
    "Lustrous", "Mageweaved", "Marbled", "Marked", "Massive", "Merciless",
    "Metallic", "Midnight", "Misted", "Moonlit", "Mottled", "Mystic",
    "Nimble", "Noble", "Oaken", "Obsidian", "Opal", "Ornate", "Overlaid",
    "Pale", "Patched", "Phantom", "Plated", "Polished", "Pristine", "Radiant",
    "Raging", "Refined", "Reinforced", "Resplendent", "Rimed", "Rippling",
    "Risen", "Rooted", "Royal", "Runed", "Sacred", "Sainted", "Scaled",
    "Scorched", "Searing", "Shadow", "Shattered", "Shielded", "Shining",
    "Silver", "Singed", "Skeletal", "Spiked", "Spirit", "Stalwart", "Steel",
    "Stoneborn", "Storm", "Sturdy", "Sublime", "Sunforged", "Tempest",
    "Tempered", "Thorned", "Thunder", "Titanium", "Torn", "Towering",
    "Tranquil", "True", "Twisted", "Umbral", "Undead", "Unholy", "Unrelenting",
    "Verdant", "Vesper", "Vibrant", "Vile", "Violet", "Void", "Volcanic",
    "Warded", "Warlord", "Wicked", "Wild", "Woven", "Wyrmscale", "Zenith",
];

pub const ARMOR_BASES: &[&str] = &[
    "Breastplate", "Chainmail", "Coat", "Cowl", "Cuirass", "Doublet", "Gambeson",
    "Gauntlets", "Gorget", "Greaves", "Hauberk", "Helm", "Hood", "Jacket",
    "Jerkin", "Kite Shield", "Lamellar", "Leggings", "Lorica", "Mantle",
    "Pauldrons", "Pavise", "Plate Armor", "Robe", "Round Shield", "Sabatons",
    "Scale Mail", "Sallet", "Sash", "Shoulderguards", "Skirt", "Skull Cap",
    "Splint Mail", "Surcoat", "Tower Shield", "Tunic", "Vambraces", "Vest",
    "Visage", "Warplate", "Wrap", "Brigandine", "Barbute", "Buckler",
    "Camail", "Chausses", "Coif", "Cuisse", "Elbow Cops", "Fauld",
    "Full Plate", "Harness", "Haubergeon", "Kettle Hat", "Knee Cops",
    "Lames", "Laminar", "Mail Coif", "Mitten Gauntlets", "Nasal Helm",
    "Pavis", "Poleyn", "Pot Helm", "Rerebrace", "Roundel", "Sabaton",
    "Spangenhelm", "Spaulders", "Tasset", "Vambrace", "War Mask",
    "Winged Helm", "Wyrm Coil", "Leather Vest", "Silk Robe", "Wool Cloak",
];

pub const ARMOR_SUFFIXES: &[&str] = &[
    "of Absorption", "of Aegis", "of Agility", "of Atonement", "of Aversion",
    "of Bravery", "of Bulwark", "of Calm", "of Castigation", "of Clarity",
    "of Composure", "of Conviction", "of Courage", "of Dauntlessness", "of Defense",
    "of Deflection", "of Denial", "of Devotion", "of Dominance", "of Durability",
    "of Endurance", "of Fortification", "of Fortitude", "of Fury", "of Grace",
    "of Hardiness", "of Immunity", "of Impenetrability", "of Indomitability",
    "of Iron Will", "of Justice", "of Might", "of Persistence", "of Power",
    "of Protection", "of Purity", "of Readiness", "of Recovery", "of Resilience",
    "of Resolve", "of Retaliation", "of Retribution", "of Salvation", "of Sanctity",
    "of Serenity", "of Shelter", "of Shielding", "of Solace", "of Solidarity",
    "of Stability", "of Steadfastness", "of Stone", "of Strength", "of Stubbornness",
    "of Tenacity", "of the Bear", "of the Bull", "of the Colossus", "of the Defender",
    "of the Dragon", "of the Earth", "of the Fortress", "of the Giant", "of the Guardian",
    "of the Iron Giant", "of the Knight", "of the Leviathan", "of the Lion",
    "of the Mountain", "of the Paladin", "of the Rampart", "of the Rock",
    "of the Sentinel", "of the Titan", "of the Tortoise", "of the Unyielding",
    "of the Vanguard", "of the Wall", "of the Warden", "of the Warrior",
    "of Toughness", "of Valor", "of Vengeance", "of Vigilance", "of Vitality",
    "of Ward", "of Warding", "of Will", "of Withstanding", "of Wrath", "of Zeal",
];

pub fn generate_armor_name(seed: &mut u64) -> String {
    let prefix_idx = (lcg_next(seed) as usize) % ARMOR_PREFIXES.len();
    let base_idx = (lcg_next(seed) as usize) % ARMOR_BASES.len();
    let suffix_roll = lcg_next(seed) % 3;
    if suffix_roll == 0 {
        let suffix_idx = (lcg_next(seed) as usize) % ARMOR_SUFFIXES.len();
        format!("{} {} {}", ARMOR_PREFIXES[prefix_idx], ARMOR_BASES[base_idx], ARMOR_SUFFIXES[suffix_idx])
    } else {
        format!("{} {}", ARMOR_PREFIXES[prefix_idx], ARMOR_BASES[base_idx])
    }
}

// ============================================================
// SECTION: SET BONUS CALCULATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct SetBonusTier {
    pub pieces_required: usize,
    pub modifiers: Vec<StatModifier>,
    pub special_effect: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ItemSetDefinition {
    pub id: u64,
    pub name: String,
    pub members: Vec<u64>,
    pub bonus_tiers: Vec<SetBonusTier>,
}

impl ItemSetDefinition {
    pub fn active_bonuses(&self, equipped_count: usize) -> Vec<&SetBonusTier> {
        self.bonus_tiers.iter().filter(|t| equipped_count >= t.pieces_required).collect()
    }

    pub fn total_modifiers_at_count(&self, equipped_count: usize) -> Vec<StatModifier> {
        let mut mods = Vec::new();
        for tier in self.active_bonuses(equipped_count) {
            mods.extend(tier.modifiers.clone());
        }
        mods
    }
}

pub fn detect_active_sets(equipped: &[&ItemDefinition], sets: &HashMap<u64, ItemSetDefinition>) -> Vec<(u64, usize)> {
    let mut set_counts: HashMap<u64, usize> = HashMap::new();
    for item in equipped {
        if let Some(set_id) = item.set_id {
            *set_counts.entry(set_id).or_insert(0) += 1;
        }
    }
    set_counts.into_iter()
        .filter(|(set_id, count)| {
            sets.get(set_id).map(|s| !s.active_bonuses(*count).is_empty()).unwrap_or(false)
        })
        .collect()
}

// ============================================================
// SECTION: UPGRADE MATERIAL SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct UpgradeMaterial {
    pub id: u64,
    pub name: String,
    pub tier: u32,
    pub material_type: String,
    pub stack_size: u32,
    pub base_value: f32,
}

#[derive(Debug, Clone)]
pub struct UpgradeRecipe {
    pub target_item_category: ItemCategory,
    pub current_upgrade_level: u32,
    pub materials: Vec<(u64, u32)>,
    pub gold_cost: u64,
    pub success_probability: f32,
    pub stat_gain_per_upgrade: Vec<StatModifier>,
}

pub fn compute_upgrade_success_probability(base_prob: f32, upgrade_level: u32, blessing_bonus: f32) -> f32 {
    let level_penalty = (upgrade_level as f32 * 0.05).min(0.4);
    ((base_prob - level_penalty) + blessing_bonus).clamp(0.05, 1.0)
}

pub fn apply_upgrade(item: &mut ItemDefinition, recipe: &UpgradeRecipe, success: bool, upgrade_level: &mut u32) {
    if success {
        *upgrade_level += 1;
        for modifier in &recipe.stat_gain_per_upgrade {
            item.implicit_modifiers.push(modifier.clone());
        }
    }
    // Failure: no change (item not destroyed at lower levels typically)
}

// ============================================================
// SECTION: INVENTORY WEIGHT BUDGET PLANNER
// ============================================================

#[derive(Debug, Clone)]
pub struct WeightBudgetPlanner {
    pub max_carry_weight: f32,
    pub current_items: Vec<(u64, f32)>,
}

impl WeightBudgetPlanner {
    pub fn new(max_carry_weight: f32) -> Self {
        Self { max_carry_weight, current_items: Vec::new() }
    }

    pub fn add_item(&mut self, item_id: u64, weight: f32) -> bool {
        let total: f32 = self.current_items.iter().map(|(_, w)| w).sum::<f32>() + weight;
        if total <= self.max_carry_weight {
            self.current_items.push((item_id, weight));
            true
        } else { false }
    }

    pub fn remove_item(&mut self, item_id: u64) {
        self.current_items.retain(|(id, _)| *id != item_id);
    }

    pub fn current_weight(&self) -> f32 {
        self.current_items.iter().map(|(_, w)| w).sum()
    }

    pub fn remaining_capacity(&self) -> f32 {
        self.max_carry_weight - self.current_weight()
    }

    pub fn encumbrance_penalty(&self) -> f32 {
        let ratio = self.current_weight() / self.max_carry_weight;
        if ratio < 0.5 { 0.0 }
        else if ratio < 0.75 { (ratio - 0.5) * 0.4 }
        else if ratio < 1.0 { 0.1 + (ratio - 0.75) * 0.8 }
        else { 0.3 + (ratio - 1.0) * 2.0 }
    }

    pub fn knapsack_optimal_subset(&self, target_weight: f32) -> Vec<u64> {
        // Greedy by weight approximation
        let mut sorted = self.current_items.clone();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut result = Vec::new();
        let mut remaining = target_weight;
        for (id, w) in sorted {
            if w <= remaining {
                remaining -= w;
                result.push(id);
            }
        }
        result
    }
}

// ============================================================
// SECTION: ITEM GENERATION VALIDATION
// ============================================================

#[derive(Debug, Clone)]
pub struct ItemGenerationReport {
    pub item_id: u64,
    pub passed: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub balance_score: f32,
}

pub fn full_generation_validation(item: &ItemDefinition, item_level: u32) -> ItemGenerationReport {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    if item.name.is_empty() { errors.push("Item has no name".to_string()); }
    if item.base_price == 0 { errors.push("Zero base price".to_string()); }
    if item.weight < 0.0 { errors.push("Negative weight".to_string()); }
    if item.explicit_modifiers.len() > 6 { warnings.push("Item has more than 6 explicit modifiers".to_string()); }
    if item.requirements.level > item_level + 5 { warnings.push("Item level requirement significantly exceeds item_level".to_string()); }

    let balance = validate_item_balance(item);
    let balance_ok = !balance.is_overpowered && !balance.is_underpowered;
    let balance_score = if balance_ok { 1.0 } else { 0.5 };
    if !balance_ok { warnings.push(format!("Balance check failed: {:?}", balance.violations)); }

    ItemGenerationReport {
        item_id: item.id,
        passed: errors.is_empty(),
        warnings,
        errors,
        balance_score,
    }
}

// ============================================================
// SECTION: VENDOR INVENTORY ROTATION
// ============================================================

#[derive(Debug, Clone)]
pub struct VendorSlot {
    pub item_id: u64,
    pub stock: u32,
    pub listed_price: f32,
    pub refresh_at_tick: u64,
}

#[derive(Debug, Clone)]
pub struct VendorState {
    pub vendor_id: u64,
    pub slots: Vec<VendorSlot>,
    pub gold: u64,
    pub refresh_interval_ticks: u64,
    pub last_refresh_tick: u64,
    pub specialty_category: Option<ItemCategory>,
}

impl VendorState {
    pub fn new(vendor_id: u64, refresh_interval_ticks: u64) -> Self {
        Self { vendor_id, slots: Vec::new(), gold: 1000, refresh_interval_ticks, last_refresh_tick: 0, specialty_category: None }
    }

    pub fn add_slot(&mut self, item_id: u64, stock: u32, price: f32, current_tick: u64) {
        self.slots.push(VendorSlot { item_id, stock, listed_price: price, refresh_at_tick: current_tick + self.refresh_interval_ticks });
    }

    pub fn buy_item(&mut self, slot_index: usize, player_gold: &mut u64) -> Option<u64> {
        if slot_index >= self.slots.len() { return None; }
        let slot = &mut self.slots[slot_index];
        let price = slot.listed_price as u64;
        if *player_gold < price || slot.stock == 0 { return None; }
        *player_gold -= price;
        self.gold += price;
        slot.stock -= 1;
        Some(slot.item_id)
    }

    pub fn sell_item_to_vendor(&mut self, item_id: u64, sell_price: f32, player_gold: &mut u64) {
        let gain = (sell_price * 0.3) as u64;
        if self.gold >= gain {
            self.gold -= gain;
            *player_gold += gain;
            self.slots.push(VendorSlot { item_id, stock: 1, listed_price: sell_price * 1.2, refresh_at_tick: u64::MAX });
        }
    }

    pub fn needs_refresh(&self, current_tick: u64) -> bool {
        current_tick >= self.last_refresh_tick + self.refresh_interval_ticks
    }

    pub fn refresh(&mut self, current_tick: u64, new_items: Vec<(u64, u32, f32)>) {
        self.slots.retain(|s| s.refresh_at_tick > current_tick);
        for (item_id, stock, price) in new_items {
            self.add_slot(item_id, stock, price, current_tick);
        }
        self.last_refresh_tick = current_tick;
    }

    pub fn apply_reputation_discount(&mut self, reputation: f32) {
        // reputation 0..100 gives up to 20% discount
        let discount = (reputation / 100.0 * 0.2).min(0.2);
        for slot in &mut self.slots {
            slot.listed_price *= 1.0 - discount;
        }
    }
}

// ============================================================
// SECTION: STASH TAB MANAGEMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct StashTab {
    pub id: u64,
    pub name: String,
    pub tab_type: StashTabType,
    pub items: Vec<ItemInstance>,
    pub grid_width: u32,
    pub grid_height: u32,
    pub color: Vec4,
    pub is_premium: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StashTabType { Normal, Currency, Map, Fragment, Quad, Divination, Gem, Flask, Unique }

impl StashTab {
    pub fn new(id: u64, name: &str, tab_type: StashTabType) -> Self {
        let (w, h) = match &tab_type {
            StashTabType::Quad => (24, 24),
            _ => (12, 12),
        };
        Self { id, name: name.to_string(), tab_type, items: Vec::new(), grid_width: w, grid_height: h, color: Vec4::new(0.5, 0.5, 0.5, 1.0), is_premium: false }
    }

    pub fn add_item(&mut self, item: ItemInstance) { self.items.push(item); }

    pub fn remove_item(&mut self, instance_id: u64) -> Option<ItemInstance> {
        if let Some(pos) = self.items.iter().position(|i| i.instance_id == instance_id) {
            Some(self.items.remove(pos))
        } else { None }
    }

    pub fn item_count(&self) -> usize { self.items.len() }
    pub fn is_full(&self) -> bool { self.items.len() >= (self.grid_width * self.grid_height) as usize }

    pub fn total_value(&self, db: &ItemDatabase) -> f32 {
        self.items.iter()
            .filter_map(|inst| db.definitions.get(&inst.definition_id))
            .map(|d| d.base_price as f32)
            .sum()
    }

    pub fn search(&self, query: &str) -> Vec<&ItemInstance> {
        let q = query.to_lowercase();
        self.items.iter().filter(|inst| {
            inst.instance_id.to_string().contains(&q)
        }).collect()
    }
}

#[derive(Debug)]
pub struct StashManager {
    pub tabs: Vec<StashTab>,
    pub active_tab_id: u64,
}

impl StashManager {
    pub fn new() -> Self { Self { tabs: Vec::new(), active_tab_id: 0 } }

    pub fn add_tab(&mut self, tab: StashTab) { self.tabs.push(tab); }

    pub fn active_tab(&self) -> Option<&StashTab> {
        self.tabs.iter().find(|t| t.id == self.active_tab_id)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut StashTab> {
        self.tabs.iter_mut().find(|t| t.id == self.active_tab_id)
    }

    pub fn find_item(&self, instance_id: u64) -> Option<(&StashTab, &ItemInstance)> {
        for tab in &self.tabs {
            if let Some(item) = tab.items.iter().find(|i| i.instance_id == instance_id) {
                return Some((tab, item));
            }
        }
        None
    }

    pub fn move_item(&mut self, instance_id: u64, to_tab_id: u64) -> bool {
        let from_tab_idx = self.tabs.iter().position(|t| t.items.iter().any(|i| i.instance_id == instance_id));
        if let Some(from_idx) = from_tab_idx {
            let item = self.tabs[from_idx].remove_item(instance_id);
            if let Some(item) = item {
                if let Some(to_tab) = self.tabs.iter_mut().find(|t| t.id == to_tab_id) {
                    to_tab.add_item(item);
                    return true;
                }
            }
        }
        false
    }

    pub fn total_value_all_tabs(&self, db: &ItemDatabase) -> f32 {
        self.tabs.iter().map(|t| t.total_value(db)).sum()
    }
}

// ============================================================
// END OF FILE
// ============================================================
