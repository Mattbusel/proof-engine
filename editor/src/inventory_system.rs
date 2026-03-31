// inventory_system.rs — Full Inventory & Item System Editor
// egui-based item library, loot tables, shops, crafting

use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Shape, RichText};
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

// ============================================================
// ENUMS & DATA MODEL
// ============================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ItemCategory {
    Weapon,
    Armor,
    Accessory,
    Consumable,
    Material,
    QuestItem,
    Key,
    Tool,
    Currency,
    Misc,
    Blueprint,
    Spell,
    Mount,
}

impl ItemCategory {
    pub fn name(&self) -> &str {
        match self {
            ItemCategory::Weapon => "Weapon",
            ItemCategory::Armor => "Armor",
            ItemCategory::Accessory => "Accessory",
            ItemCategory::Consumable => "Consumable",
            ItemCategory::Material => "Material",
            ItemCategory::QuestItem => "Quest Item",
            ItemCategory::Key => "Key",
            ItemCategory::Tool => "Tool",
            ItemCategory::Currency => "Currency",
            ItemCategory::Misc => "Misc",
            ItemCategory::Blueprint => "Blueprint",
            ItemCategory::Spell => "Spell",
            ItemCategory::Mount => "Mount",
        }
    }

    pub fn all() -> &'static [ItemCategory] {
        &[
            ItemCategory::Weapon, ItemCategory::Armor, ItemCategory::Accessory,
            ItemCategory::Consumable, ItemCategory::Material, ItemCategory::QuestItem,
            ItemCategory::Key, ItemCategory::Tool, ItemCategory::Currency,
            ItemCategory::Misc, ItemCategory::Blueprint, ItemCategory::Spell, ItemCategory::Mount,
        ]
    }

    pub fn icon(&self) -> char {
        match self {
            ItemCategory::Weapon => '⚔',
            ItemCategory::Armor => '🛡',
            ItemCategory::Accessory => '💍',
            ItemCategory::Consumable => '🧪',
            ItemCategory::Material => '⚙',
            ItemCategory::QuestItem => '📜',
            ItemCategory::Key => '🗝',
            ItemCategory::Tool => '🔧',
            ItemCategory::Currency => '💰',
            ItemCategory::Misc => '?',
            ItemCategory::Blueprint => '📋',
            ItemCategory::Spell => '✨',
            ItemCategory::Mount => '🐴',
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
    Unique,
}

impl Rarity {
    pub fn name(&self) -> &str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
            Rarity::Mythic => "Mythic",
            Rarity::Unique => "Unique",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            Rarity::Common => Color32::from_rgb(160, 160, 160),
            Rarity::Uncommon => Color32::from_rgb(30, 200, 60),
            Rarity::Rare => Color32::from_rgb(50, 120, 255),
            Rarity::Epic => Color32::from_rgb(180, 50, 255),
            Rarity::Legendary => Color32::from_rgb(255, 140, 0),
            Rarity::Mythic => Color32::from_rgb(220, 30, 30),
            Rarity::Unique => Color32::from_rgb(230, 180, 0),
        }
    }

    pub fn all() -> &'static [Rarity] {
        &[
            Rarity::Common, Rarity::Uncommon, Rarity::Rare,
            Rarity::Epic, Rarity::Legendary, Rarity::Mythic, Rarity::Unique,
        ]
    }

    pub fn tier(&self) -> u32 {
        match self {
            Rarity::Common => 0,
            Rarity::Uncommon => 1,
            Rarity::Rare => 2,
            Rarity::Epic => 3,
            Rarity::Legendary => 4,
            Rarity::Mythic => 5,
            Rarity::Unique => 6,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StatScaling {
    None,
    Linear { per_level: f32 },
    Exponential { base: f32, exp: f32 },
}

impl StatScaling {
    pub fn name(&self) -> &str {
        match self {
            StatScaling::None => "None",
            StatScaling::Linear { .. } => "Linear",
            StatScaling::Exponential { .. } => "Exponential",
        }
    }

    pub fn value_at_level(&self, base: f32, level: u32) -> f32 {
        match self {
            StatScaling::None => base,
            StatScaling::Linear { per_level } => base + per_level * level as f32,
            StatScaling::Exponential { base: b, exp } => base * b.powf(level as f32 * exp),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StatFormat {
    Integer,
    Decimal,
    Percent,
    PlusMinus,
}

impl StatFormat {
    pub fn name(&self) -> &str {
        match self {
            StatFormat::Integer => "Integer",
            StatFormat::Decimal => "Decimal",
            StatFormat::Percent => "Percent",
            StatFormat::PlusMinus => "±",
        }
    }

    pub fn format_value(&self, v: f32) -> String {
        match self {
            StatFormat::Integer => format!("{}", v as i64),
            StatFormat::Decimal => format!("{:.2}", v),
            StatFormat::Percent => format!("{:.1}%", v * 100.0),
            StatFormat::PlusMinus => {
                if v >= 0.0 { format!("+{:.1}", v) } else { format!("{:.1}", v) }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemStat {
    pub name: String,
    pub base_value: f32,
    pub scaling: StatScaling,
    pub display_format: StatFormat,
}

impl ItemStat {
    pub fn new(name: &str, base: f32) -> Self {
        Self {
            name: name.to_string(),
            base_value: base,
            scaling: StatScaling::None,
            display_format: StatFormat::Integer,
        }
    }

    pub fn value_at_level(&self, level: u32) -> f32 {
        self.scaling.value_at_level(self.base_value, level)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ItemEffect {
    HealHp(f32),
    HealMp(f32),
    BuffStat { stat: String, amount: f32, duration: f32 },
    Damage { damage_type: String, amount: f32 },
    Spawn { entity: String },
    TriggerQuest(String),
    UnlockAbility(String),
    ApplyStatus { status: String, duration: f32 },
    Teleport(String),
    GiveItem { item_id: u32, count: u32 },
}

impl ItemEffect {
    pub fn type_name(&self) -> &str {
        match self {
            ItemEffect::HealHp(_) => "Heal HP",
            ItemEffect::HealMp(_) => "Heal MP",
            ItemEffect::BuffStat { .. } => "Buff Stat",
            ItemEffect::Damage { .. } => "Damage",
            ItemEffect::Spawn { .. } => "Spawn Entity",
            ItemEffect::TriggerQuest(_) => "Trigger Quest",
            ItemEffect::UnlockAbility(_) => "Unlock Ability",
            ItemEffect::ApplyStatus { .. } => "Apply Status",
            ItemEffect::Teleport(_) => "Teleport",
            ItemEffect::GiveItem { .. } => "Give Item",
        }
    }

    pub fn description(&self) -> String {
        match self {
            ItemEffect::HealHp(v) => format!("Restore {} HP", v),
            ItemEffect::HealMp(v) => format!("Restore {} MP", v),
            ItemEffect::BuffStat { stat, amount, duration } => {
                format!("Buff {} by {:.1} for {:.1}s", stat, amount, duration)
            }
            ItemEffect::Damage { damage_type, amount } => {
                format!("Deal {:.1} {} damage", amount, damage_type)
            }
            ItemEffect::Spawn { entity } => format!("Spawn {}", entity),
            ItemEffect::TriggerQuest(q) => format!("Trigger quest: {}", q),
            ItemEffect::UnlockAbility(a) => format!("Unlock: {}", a),
            ItemEffect::ApplyStatus { status, duration } => {
                format!("Apply {} for {:.1}s", status, duration)
            }
            ItemEffect::Teleport(loc) => format!("Teleport to {}", loc),
            ItemEffect::GiveItem { item_id, count } => {
                format!("Give {} x item#{}", count, item_id)
            }
        }
    }

    pub fn all_type_names() -> &'static [&'static str] {
        &[
            "Heal HP", "Heal MP", "Buff Stat", "Damage",
            "Spawn Entity", "Trigger Quest", "Unlock Ability",
            "Apply Status", "Teleport", "Give Item",
        ]
    }

    pub fn default_for_type(type_name: &str) -> ItemEffect {
        match type_name {
            "Heal HP" => ItemEffect::HealHp(50.0),
            "Heal MP" => ItemEffect::HealMp(30.0),
            "Buff Stat" => ItemEffect::BuffStat { stat: "Strength".to_string(), amount: 10.0, duration: 30.0 },
            "Damage" => ItemEffect::Damage { damage_type: "Physical".to_string(), amount: 25.0 },
            "Spawn Entity" => ItemEffect::Spawn { entity: "Goblin".to_string() },
            "Trigger Quest" => ItemEffect::TriggerQuest("quest_001".to_string()),
            "Unlock Ability" => ItemEffect::UnlockAbility("Fire Bolt".to_string()),
            "Apply Status" => ItemEffect::ApplyStatus { status: "Poisoned".to_string(), duration: 10.0 },
            "Teleport" => ItemEffect::Teleport("Town Square".to_string()),
            "Give Item" => ItemEffect::GiveItem { item_id: 1, count: 1 },
            _ => ItemEffect::HealHp(10.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EquipSlot {
    Head,
    Chest,
    Legs,
    Feet,
    Hands,
    MainHand,
    OffHand,
    Ring1,
    Ring2,
    Neck,
    Back,
    Waist,
}

impl EquipSlot {
    pub fn name(&self) -> &str {
        match self {
            EquipSlot::Head => "Head",
            EquipSlot::Chest => "Chest",
            EquipSlot::Legs => "Legs",
            EquipSlot::Feet => "Feet",
            EquipSlot::Hands => "Hands",
            EquipSlot::MainHand => "Main Hand",
            EquipSlot::OffHand => "Off Hand",
            EquipSlot::Ring1 => "Ring 1",
            EquipSlot::Ring2 => "Ring 2",
            EquipSlot::Neck => "Neck",
            EquipSlot::Back => "Back",
            EquipSlot::Waist => "Waist",
        }
    }

    pub fn all() -> &'static [EquipSlot] {
        &[
            EquipSlot::Head, EquipSlot::Chest, EquipSlot::Legs, EquipSlot::Feet,
            EquipSlot::Hands, EquipSlot::MainHand, EquipSlot::OffHand,
            EquipSlot::Ring1, EquipSlot::Ring2, EquipSlot::Neck, EquipSlot::Back, EquipSlot::Waist,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatRequirement {
    pub stat: String,
    pub min_value: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub ingredients: Vec<(u32, u32)>, // (item_id, count)
    pub result_count: u32,
    pub skill_req: Option<(String, u32)>,
    pub station: Option<String>,
    pub time: f32,
}

impl Recipe {
    pub fn new() -> Self {
        Self {
            ingredients: Vec::new(),
            result_count: 1,
            skill_req: None,
            station: None,
            time: 5.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub lore: String,
    pub category: ItemCategory,
    pub rarity: Rarity,
    pub icon_glyph: char,
    pub icon_color: Color32,
    pub base_value: u32,
    pub weight: f32,
    pub max_stack: u32,
    pub stats: Vec<ItemStat>,
    pub effects: Vec<ItemEffect>,
    pub equip_slot: Option<EquipSlot>,
    pub requirements: Vec<StatRequirement>,
    pub tags: Vec<String>,
    pub craftable: bool,
    pub recipe: Option<Recipe>,
    pub drop_chance: f32,
    pub level_req: u32,
    pub unique: bool,
    pub quest_item: bool,
}

impl Item {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            name: format!("New Item {}", id),
            description: "An item.".to_string(),
            lore: String::new(),
            category: ItemCategory::Misc,
            rarity: Rarity::Common,
            icon_glyph: '?',
            icon_color: Color32::WHITE,
            base_value: 10,
            weight: 0.5,
            max_stack: 1,
            stats: Vec::new(),
            effects: Vec::new(),
            equip_slot: None,
            requirements: Vec::new(),
            tags: Vec::new(),
            craftable: false,
            recipe: None,
            drop_chance: 0.1,
            level_req: 1,
            unique: false,
            quest_item: false,
        }
    }

    pub fn sword(id: u32) -> Self {
        let mut item = Item::new(id);
        item.name = "Iron Sword".to_string();
        item.description = "A dependable iron sword.".to_string();
        item.lore = "Forged in the Ironhold smithy, carried by many adventurers.".to_string();
        item.category = ItemCategory::Weapon;
        item.rarity = Rarity::Common;
        item.icon_glyph = '/';
        item.icon_color = Color32::from_rgb(180, 180, 200);
        item.base_value = 150;
        item.weight = 2.5;
        item.equip_slot = Some(EquipSlot::MainHand);
        item.stats = vec![
            ItemStat { name: "Attack".to_string(), base_value: 18.0, scaling: StatScaling::Linear { per_level: 2.0 }, display_format: StatFormat::Integer },
            ItemStat { name: "Durability".to_string(), base_value: 100.0, scaling: StatScaling::None, display_format: StatFormat::Integer },
        ];
        item
    }

    pub fn health_potion(id: u32) -> Self {
        let mut item = Item::new(id);
        item.name = "Health Potion".to_string();
        item.description = "Restores 50 HP when consumed.".to_string();
        item.lore = "A ruby-red liquid shimmering with healing essence.".to_string();
        item.category = ItemCategory::Consumable;
        item.rarity = Rarity::Common;
        item.icon_glyph = '!';
        item.icon_color = Color32::from_rgb(200, 50, 50);
        item.base_value = 25;
        item.weight = 0.3;
        item.max_stack = 20;
        item.effects = vec![ItemEffect::HealHp(50.0)];
        item
    }

    pub fn flame_staff(id: u32) -> Self {
        let mut item = Item::new(id);
        item.name = "Staff of Flames".to_string();
        item.description = "Channels fire magic with lethal efficiency.".to_string();
        item.lore = "Crafted by the Mage's Guild from dragonbone and ignitite crystal.".to_string();
        item.category = ItemCategory::Weapon;
        item.rarity = Rarity::Rare;
        item.icon_glyph = '|';
        item.icon_color = Color32::from_rgb(255, 120, 30);
        item.base_value = 800;
        item.weight = 1.8;
        item.level_req = 10;
        item.equip_slot = Some(EquipSlot::MainHand);
        item.stats = vec![
            ItemStat { name: "Magic Attack".to_string(), base_value: 45.0, scaling: StatScaling::Linear { per_level: 4.0 }, display_format: StatFormat::Integer },
            ItemStat { name: "Cast Speed".to_string(), base_value: 1.2, scaling: StatScaling::None, display_format: StatFormat::Decimal },
        ];
        item.effects = vec![
            ItemEffect::Damage { damage_type: "Fire".to_string(), amount: 30.0 },
        ];
        item.requirements = vec![
            StatRequirement { stat: "Intelligence".to_string(), min_value: 20.0 },
        ];
        item
    }

    pub fn value_at_level(&self, level: u32) -> u32 {
        let rarity_mult = match self.rarity {
            Rarity::Common => 1.0,
            Rarity::Uncommon => 2.0,
            Rarity::Rare => 4.0,
            Rarity::Epic => 8.0,
            Rarity::Legendary => 16.0,
            Rarity::Mythic => 32.0,
            Rarity::Unique => 50.0,
        };
        (self.base_value as f32 * rarity_mult * (1.0 + level as f32 * 0.1)) as u32
    }

    pub fn total_stat(&self, stat_name: &str, level: u32) -> Option<f32> {
        let st = self.stats.iter().find(|s| s.name.as_str() == stat_name)?;
        Some(st.value_at_level(level))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemLibrary {
    pub items: Vec<Item>,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
}

impl ItemLibrary {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            categories: ItemCategory::all().iter().map(|c| c.name().to_string()).collect(),
            tags: vec![
                "fire".to_string(), "ice".to_string(), "magic".to_string(),
                "melee".to_string(), "ranged".to_string(), "rare-drop".to_string(),
                "crafted".to_string(), "boss".to_string(), "set-item".to_string(),
            ],
        }
    }

    pub fn with_defaults() -> Self {
        let mut lib = Self::new();
        lib.items.push(Item::sword(1));
        lib.items.push(Item::health_potion(2));
        lib.items.push(Item::flame_staff(3));
        // Iron Armor
        let mut armor = Item::new(4);
        armor.name = "Iron Chestplate".to_string();
        armor.description = "Solid iron protection.".to_string();
        armor.lore = "Heavy but reliable.".to_string();
        armor.category = ItemCategory::Armor;
        armor.rarity = Rarity::Common;
        armor.icon_glyph = '[';
        armor.icon_color = Color32::from_rgb(140, 140, 160);
        armor.base_value = 200;
        armor.weight = 8.0;
        armor.equip_slot = Some(EquipSlot::Chest);
        armor.stats = vec![
            ItemStat { name: "Defense".to_string(), base_value: 22.0, scaling: StatScaling::Linear { per_level: 1.5 }, display_format: StatFormat::Integer },
        ];
        lib.items.push(armor);

        // Gold coin
        let mut gold = Item::new(5);
        gold.name = "Gold Coin".to_string();
        gold.description = "Standard currency.".to_string();
        gold.lore = "Minted by the Royal Treasury.".to_string();
        gold.category = ItemCategory::Currency;
        gold.rarity = Rarity::Common;
        gold.icon_glyph = '$';
        gold.icon_color = Color32::from_rgb(220, 180, 30);
        gold.base_value = 1;
        gold.weight = 0.01;
        gold.max_stack = 9999;
        lib.items.push(gold);

        // Dragon scale
        let mut scale = Item::new(6);
        scale.name = "Dragon Scale".to_string();
        scale.description = "A massive scale from an ancient dragon. Used in master crafting.".to_string();
        scale.lore = "Shimmers with residual draconic magic.".to_string();
        scale.category = ItemCategory::Material;
        scale.rarity = Rarity::Legendary;
        scale.icon_glyph = '*';
        scale.icon_color = Color32::from_rgb(255, 60, 30);
        scale.base_value = 5000;
        scale.weight = 0.5;
        scale.max_stack = 10;
        scale.drop_chance = 0.01;
        lib.items.push(scale);

        lib
    }

    pub fn find_by_id(&self, id: u32) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn find_by_id_mut(&mut self, id: u32) -> Option<&mut Item> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    pub fn next_id(&self) -> u32 {
        self.items.iter().map(|i| i.id).max().unwrap_or(0) + 1
    }

    pub fn filtered(&self, category: Option<&ItemCategory>, rarity: Option<&Rarity>, search: &str) -> Vec<usize> {
        self.items.iter().enumerate().filter_map(|(i, item)| {
            if let Some(cat) = category {
                if &item.category != cat { return None; }
            }
            if let Some(rar) = rarity {
                if &item.rarity != rar { return None; }
            }
            if !search.is_empty() {
                let search_lower = search.to_lowercase();
                if !item.name.to_lowercase().contains(&search_lower)
                    && !item.description.to_lowercase().contains(&search_lower)
                    && !item.tags.iter().any(|t| t.to_lowercase().contains(&search_lower)) {
                    return None;
                }
            }
            Some(i)
        }).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LootEntry {
    pub item_id: u32,
    pub weight: f32,
    pub min_count: u32,
    pub max_count: u32,
    pub condition: Option<String>,
}

impl LootEntry {
    pub fn new(item_id: u32) -> Self {
        Self { item_id, weight: 1.0, min_count: 1, max_count: 1, condition: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LootTable {
    pub name: String,
    pub entries: Vec<LootEntry>,
}

impl LootTable {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), entries: Vec::new() }
    }

    pub fn total_weight(&self) -> f32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    /// Simulate a single roll; returns (item_id, count)
    pub fn roll(&self, seed: u64) -> Option<(u32, u32)> {
        if self.entries.is_empty() { return None; }
        let total = self.total_weight();
        if total <= 0.0 { return None; }

        let mut rng = seed;
        rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
        let r = (rng & 0xFFFFFF) as f32 / 16777216.0 * total;

        let mut acc = 0.0_f32;
        for entry in &self.entries {
            acc += entry.weight;
            if r <= acc {
                // roll count
                let range = entry.max_count - entry.min_count;
                let count = if range == 0 {
                    entry.min_count
                } else {
                    rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
                    entry.min_count + (rng % (range as u64 + 1)) as u32
                };
                return Some((entry.item_id, count));
            }
        }
        let last = self.entries.last().unwrap();
        Some((last.item_id, last.min_count))
    }

    pub fn simulate_rolls(&self, n: u32, seed: u64) -> Vec<(u32, u32)> {
        (0..n).filter_map(|i| self.roll(seed.wrapping_add(i as u64 * 137))).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShopEntry {
    pub item_id: u32,
    pub price: u32,
    pub stock: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shop {
    pub name: String,
    pub currency: String,
    pub items: Vec<ShopEntry>,
    pub restock_interval: f32,
    pub markup: f32,
}

impl Shop {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            currency: "Gold".to_string(),
            items: Vec::new(),
            restock_interval: 3600.0,
            markup: 1.2,
        }
    }

    pub fn price_for(&self, item: &Item) -> u32 {
        ((item.base_value as f32) * self.markup) as u32
    }
}

// ============================================================
// SORT & FILTER
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum SortField {
    Name,
    Rarity,
    Value,
    Weight,
    Level,
}

impl SortField {
    pub fn name(&self) -> &str {
        match self {
            SortField::Name => "Name",
            SortField::Rarity => "Rarity",
            SortField::Value => "Value",
            SortField::Weight => "Weight",
            SortField::Level => "Level",
        }
    }

    pub fn all() -> &'static [SortField] {
        &[SortField::Name, SortField::Rarity, SortField::Value, SortField::Weight, SortField::Level]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InvView {
    Items,
    LootTables,
    Shops,
    Crafting,
}

impl InvView {
    pub fn name(&self) -> &str {
        match self {
            InvView::Items => "Items",
            InvView::LootTables => "Loot Tables",
            InvView::Shops => "Shops",
            InvView::Crafting => "Crafting",
        }
    }
}

// ============================================================
// EDITOR STATE
// ============================================================

pub struct InventoryEditor {
    pub library: ItemLibrary,
    pub loot_tables: Vec<LootTable>,
    pub shops: Vec<Shop>,
    pub selected_item: Option<usize>,
    pub selected_loot_table: Option<usize>,
    pub selected_shop: Option<usize>,
    pub view: InvView,
    pub filter_category: Option<ItemCategory>,
    pub filter_rarity: Option<Rarity>,
    pub search: String,
    pub sort_by: SortField,
    pub sort_ascending: bool,
    pub preview_item: Option<usize>,
    pub grid_view: bool,
    pub id_counter: u32,
    // internal
    scroll_to_selected: bool,
    confirm_delete: Option<usize>,
    loot_sim_results: Vec<(u32, u32)>,
    loot_sim_seed: u64,
    loot_sim_n: u32,
    shop_sim_result: Option<String>,
    effect_type_dropdown: String,
    tag_input: String,
    ingredient_search: String,
    show_crafting_tree_for: Option<usize>,
    right_panel_tab: RightTab,
    drag_loot_from: Option<usize>,
    edit_new_stat_name: String,
    edit_new_stat_value: f32,
    edit_new_req_stat: String,
    edit_new_req_val: f32,
}

#[derive(Clone, Debug, PartialEq)]
enum RightTab {
    Preview,
    Stats,
    Effects,
    Recipe,
}

impl InventoryEditor {
    pub fn new() -> Self {
        let library = ItemLibrary::with_defaults();
        let id_counter = library.next_id();

        let mut loot_tables = Vec::new();
        let mut goblin_loot = LootTable::new("Goblin Drop");
        goblin_loot.entries.push(LootEntry { item_id: 1, weight: 3.0, min_count: 1, max_count: 1, condition: None });
        goblin_loot.entries.push(LootEntry { item_id: 2, weight: 5.0, min_count: 1, max_count: 3, condition: None });
        goblin_loot.entries.push(LootEntry { item_id: 5, weight: 8.0, min_count: 5, max_count: 20, condition: None });
        loot_tables.push(goblin_loot);

        let mut boss_loot = LootTable::new("Dragon Hoard");
        boss_loot.entries.push(LootEntry { item_id: 3, weight: 1.0, min_count: 1, max_count: 1, condition: Some("first_kill".to_string()) });
        boss_loot.entries.push(LootEntry { item_id: 6, weight: 2.0, min_count: 1, max_count: 3, condition: None });
        boss_loot.entries.push(LootEntry { item_id: 5, weight: 10.0, min_count: 50, max_count: 200, condition: None });
        loot_tables.push(boss_loot);

        let mut shops = Vec::new();
        let mut general = Shop::new("General Store");
        general.items.push(ShopEntry { item_id: 2, price: 30, stock: Some(10) });
        general.items.push(ShopEntry { item_id: 5, price: 1, stock: None });
        shops.push(general);

        let mut armory = Shop::new("Armory");
        armory.items.push(ShopEntry { item_id: 1, price: 180, stock: Some(3) });
        armory.items.push(ShopEntry { item_id: 4, price: 250, stock: Some(2) });
        shops.push(armory);

        Self {
            library,
            loot_tables,
            shops,
            selected_item: None,
            selected_loot_table: None,
            selected_shop: None,
            view: InvView::Items,
            filter_category: None,
            filter_rarity: None,
            search: String::new(),
            sort_by: SortField::Name,
            sort_ascending: true,
            preview_item: None,
            grid_view: true,
            id_counter,
            scroll_to_selected: false,
            confirm_delete: None,
            loot_sim_results: Vec::new(),
            loot_sim_seed: 42,
            loot_sim_n: 10,
            shop_sim_result: None,
            effect_type_dropdown: "Heal HP".to_string(),
            tag_input: String::new(),
            ingredient_search: String::new(),
            show_crafting_tree_for: None,
            right_panel_tab: RightTab::Preview,
            drag_loot_from: None,
            edit_new_stat_name: "Attack".to_string(),
            edit_new_stat_value: 10.0,
            edit_new_req_stat: "Strength".to_string(),
            edit_new_req_val: 10.0,
        }
    }

    pub fn show_panel(ctx: &egui::Context, editor: &mut InventoryEditor, open: &mut bool) {
        egui::Window::new("Inventory & Item System Editor")
            .open(open)
            .resizable(true)
            .default_size([1200.0, 800.0])
            .show(ctx, |ui| {
                editor.show(ui);
            });
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Top tab bar
        ui.horizontal(|ui| {
            for view in [InvView::Items, InvView::LootTables, InvView::Shops, InvView::Crafting] {
                let selected = self.view == view;
                if ui.selectable_label(selected, view.name()).clicked() {
                    self.view = view;
                }
            }
        });
        ui.separator();

        match self.view {
            InvView::Items => self.show_items_view(ui),
            InvView::LootTables => self.show_loot_tables_view(ui),
            InvView::Shops => self.show_shops_view(ui),
            InvView::Crafting => self.show_crafting_view(ui),
        }
    }

    // --------------------------------------------------------
    // ITEMS VIEW
    // --------------------------------------------------------

    fn show_items_view(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        ui.horizontal(|ui| {
            // LEFT: item grid/list
            let left_w = if self.selected_item.is_some() { available.x * 0.30 } else { available.x * 0.55 };
            ui.vertical(|ui| {
                ui.set_min_width(left_w);
                ui.set_max_width(left_w);
                self.show_item_list(ui);
            });

            if let Some(sel) = self.selected_item {
                ui.separator();
                // CENTER: item editor
                let center_w = available.x * 0.40;
                ui.vertical(|ui| {
                    ui.set_min_width(center_w);
                    ui.set_max_width(center_w);
                    self.show_item_editor(ui, sel);
                });
                ui.separator();
                // RIGHT: preview card
                ui.vertical(|ui| {
                    self.show_item_preview_card(ui, sel);
                });
            }
        });
    }

    fn show_item_list(&mut self, ui: &mut egui::Ui) {
        // Toolbar
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search);
            if ui.small_button("✖").clicked() { self.search.clear(); }
        });
        ui.horizontal(|ui| {
            // Category filter
            let cat_label = self.filter_category.as_ref().map(|c| c.name()).unwrap_or("All Categories");
            egui::ComboBox::from_id_source("cat_filter")
                .selected_text(cat_label)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(self.filter_category.is_none(), "All Categories").clicked() {
                        self.filter_category = None;
                    }
                    for cat in ItemCategory::all() {
                        let sel = self.filter_category.as_ref() == Some(cat);
                        if ui.selectable_label(sel, cat.name()).clicked() {
                            self.filter_category = Some(cat.clone());
                        }
                    }
                });
            // Rarity filter
            let rar_label = self.filter_rarity.as_ref().map(|r| r.name()).unwrap_or("All Rarities");
            egui::ComboBox::from_id_source("rar_filter")
                .selected_text(rar_label)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(self.filter_rarity.is_none(), "All Rarities").clicked() {
                        self.filter_rarity = None;
                    }
                    for rar in Rarity::all() {
                        let sel = self.filter_rarity.as_ref() == Some(rar);
                        if ui.selectable_label(sel, RichText::new(rar.name()).color(rar.color())).clicked() {
                            self.filter_rarity = Some(rar.clone());
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Sort:");
            for sf in SortField::all() {
                let sel = &self.sort_by == sf;
                if ui.selectable_label(sel, sf.name()).clicked() {
                    if self.sort_by == *sf {
                        self.sort_ascending = !self.sort_ascending;
                    } else {
                        self.sort_by = sf.clone();
                        self.sort_ascending = true;
                    }
                }
            }
            let dir = if self.sort_ascending { "↑" } else { "↓" };
            ui.label(dir);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.selectable_label(self.grid_view, "Grid").clicked() { self.grid_view = true; }
                if ui.selectable_label(!self.grid_view, "List").clicked() { self.grid_view = false; }
            });
        });

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("+ New Item").clicked() {
                self.id_counter += 1;
                let new_item = Item::new(self.id_counter);
                self.library.items.push(new_item);
                self.selected_item = Some(self.library.items.len() - 1);
                self.scroll_to_selected = true;
            }
        });

        // Build sorted, filtered index list
        let cat_ref = self.filter_category.as_ref();
        let rar_ref = self.filter_rarity.as_ref();
        let search_clone = self.search.clone();
        let mut indices = self.library.filtered(cat_ref, rar_ref, &search_clone);

        let sort_by = self.sort_by.clone();
        let sort_asc = self.sort_ascending;
        let items = &self.library.items;
        indices.sort_by(|&a, &b| {
            let ia = &items[a];
            let ib = &items[b];
            let cmp = match sort_by {
                SortField::Name => ia.name.cmp(&ib.name),
                SortField::Rarity => ia.rarity.tier().cmp(&ib.rarity.tier()),
                SortField::Value => ia.base_value.cmp(&ib.base_value),
                SortField::Weight => ia.weight.partial_cmp(&ib.weight).unwrap_or(std::cmp::Ordering::Equal),
                SortField::Level => ia.level_req.cmp(&ib.level_req),
            };
            if sort_asc { cmp } else { cmp.reverse() }
        });

        let available_h = ui.available_height() - 10.0;
        egui::ScrollArea::vertical()
            .id_source("item_list_scroll")
            .max_height(available_h)
            .show(ui, |ui| {
                if self.grid_view {
                    self.show_item_grid(ui, &indices);
                } else {
                    self.show_item_list_rows(ui, &indices);
                }
            });
    }

    fn show_item_grid(&mut self, ui: &mut egui::Ui, indices: &[usize]) {
        let cell_size = 58.0;
        let available_w = ui.available_width();
        let cols = ((available_w / cell_size) as usize).max(1);

        let mut to_select: Option<usize> = None;
        let mut to_duplicate: Option<usize> = None;
        let mut to_delete: Option<usize> = None;

        let mut col = 0;
        let mut row_ui: Option<egui::Ui> = None;
        // We'll use a grid layout via columns
        egui::Grid::new("item_grid")
            .min_col_width(cell_size)
            .max_col_width(cell_size)
            .show(ui, |ui| {
                for (grid_pos, &item_idx) in indices.iter().enumerate() {
                    let item = &self.library.items[item_idx];
                    let selected = self.selected_item == Some(item_idx);

                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(cell_size - 4.0, cell_size - 4.0),
                        egui::Sense::click(),
                    );

                    let painter = ui.painter();
                    let rarity_color = item.rarity.color();
                    let bg = if selected {
                        Color32::from_rgba_premultiplied(
                            rarity_color.r() / 3, rarity_color.g() / 3,
                            rarity_color.b() / 3, 200,
                        )
                    } else {
                        Color32::from_rgb(35, 35, 45)
                    };
                    painter.rect_filled(rect, 4.0, bg);
                    painter.rect_stroke(rect, 4.0, Stroke::new(if selected { 2.0 } else { 1.0 }, rarity_color), egui::StrokeKind::Inside);

                    // Icon glyph
                    painter.text(
                        Pos2::new(rect.center().x, rect.center().y - 6.0),
                        egui::Align2::CENTER_CENTER,
                        item.icon_glyph.to_string(),
                        FontId::monospace(22.0),
                        item.icon_color,
                    );

                    // Name (truncated)
                    let display_name = if item.name.len() > 7 {
                        format!("{}…", &item.name[..6])
                    } else {
                        item.name.clone()
                    };
                    painter.text(
                        Pos2::new(rect.center().x, rect.max.y - 10.0),
                        egui::Align2::CENTER_CENTER,
                        display_name,
                        FontId::proportional(8.0),
                        Color32::from_gray(200),
                    );

                    if response.clicked() {
                        to_select = Some(item_idx);
                    }

                    response.context_menu(|ui| {
                        ui.label(&item.name);
                        ui.separator();
                        if ui.button("Edit").clicked() {
                            to_select = Some(item_idx);
                            ui.close_menu();
                        }
                        if ui.button("Duplicate").clicked() {
                            to_duplicate = Some(item_idx);
                            ui.close_menu();
                        }
                        if ui.button("Delete").clicked() {
                            to_delete = Some(item_idx);
                            ui.close_menu();
                        }
                    });

                    if (grid_pos + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });

        if let Some(idx) = to_select { self.selected_item = Some(idx); }
        if let Some(idx) = to_duplicate {
            let mut new_item = self.library.items[idx].clone();
            self.id_counter += 1;
            new_item.id = self.id_counter;
            new_item.name = format!("{} (Copy)", new_item.name);
            self.library.items.push(new_item);
            self.selected_item = Some(self.library.items.len() - 1);
        }
        if let Some(idx) = to_delete {
            self.library.items.remove(idx);
            if self.selected_item == Some(idx) {
                self.selected_item = None;
            } else if let Some(s) = self.selected_item {
                if s > idx { self.selected_item = Some(s - 1); }
            }
        }
    }

    fn show_item_list_rows(&mut self, ui: &mut egui::Ui, indices: &[usize]) {
        let mut to_select: Option<usize> = None;
        let mut to_duplicate: Option<usize> = None;
        let mut to_delete: Option<usize> = None;

        for &item_idx in indices {
            let item = &self.library.items[item_idx];
            let selected = self.selected_item == Some(item_idx);
            let rarity_color = item.rarity.color();

            let response = ui.horizontal(|ui| {
                // Icon
                let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(30, 30, 40));
                ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, rarity_color), egui::StrokeKind::Inside);
                ui.painter().text(
                    rect.center(), egui::Align2::CENTER_CENTER,
                    item.icon_glyph.to_string(), FontId::monospace(14.0), item.icon_color,
                );

                // Name + rarity
                let label = RichText::new(format!("{} [{}] lv.{}", item.name, item.category.name(), item.level_req))
                    .color(rarity_color);
                let resp = ui.selectable_label(selected, label);

                // Value + weight
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("{:.1}kg  {}g", item.weight, item.base_value));
                });
                resp
            }).inner;

            if response.clicked() { to_select = Some(item_idx); }
            response.context_menu(|ui| {
                ui.label(&item.name);
                ui.separator();
                if ui.button("Edit").clicked() { to_select = Some(item_idx); ui.close_menu(); }
                if ui.button("Duplicate").clicked() { to_duplicate = Some(item_idx); ui.close_menu(); }
                if ui.button("Delete").clicked() { to_delete = Some(item_idx); ui.close_menu(); }
            });
        }

        if let Some(idx) = to_select { self.selected_item = Some(idx); }
        if let Some(idx) = to_duplicate {
            let mut new_item = self.library.items[idx].clone();
            self.id_counter += 1;
            new_item.id = self.id_counter;
            new_item.name = format!("{} (Copy)", new_item.name);
            self.library.items.push(new_item);
            self.selected_item = Some(self.library.items.len() - 1);
        }
        if let Some(idx) = to_delete {
            self.library.items.remove(idx);
            if self.selected_item == Some(idx) { self.selected_item = None; }
            else if let Some(s) = self.selected_item { if s > idx { self.selected_item = Some(s - 1); } }
        }
    }

    fn show_item_editor(&mut self, ui: &mut egui::Ui, sel: usize) {
        if sel >= self.library.items.len() {
            self.selected_item = None;
            return;
        }

        ui.horizontal(|ui| {
            ui.heading("Item Editor");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✖ Close").clicked() { self.selected_item = None; }
            });
        });
        ui.separator();

        // Right panel tabs
        ui.horizontal(|ui| {
            for tab in [RightTab::Preview, RightTab::Stats, RightTab::Effects, RightTab::Recipe] {
                let tname = match &tab {
                    RightTab::Preview => "Basic",
                    RightTab::Stats => "Stats",
                    RightTab::Effects => "Effects",
                    RightTab::Recipe => "Recipe",
                };
                let sel_tab = &self.right_panel_tab == &tab;
                if ui.selectable_label(sel_tab, tname).clicked() {
                    self.right_panel_tab = tab;
                }
            }
        });
        ui.separator();

        egui::ScrollArea::vertical().id_source("item_editor_scroll").show(ui, |ui| {
            match self.right_panel_tab {
                RightTab::Preview => self.show_item_basic_editor(ui, sel),
                RightTab::Stats => self.show_item_stats_editor(ui, sel),
                RightTab::Effects => self.show_item_effects_editor(ui, sel),
                RightTab::Recipe => self.show_item_recipe_editor(ui, sel),
            }
        });
    }

    fn show_item_basic_editor(&mut self, ui: &mut egui::Ui, sel: usize) {
        let item = &mut self.library.items[sel];

        // ID (read only)
        ui.horizontal(|ui| {
            ui.label("ID:");
            ui.label(format!("{}", item.id));
        });

        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut item.name);
        });

        ui.label("Description:");
        ui.text_edit_multiline(&mut item.description);

        ui.label("Lore:");
        ui.add(egui::TextEdit::multiline(&mut item.lore).desired_rows(3));

        ui.separator();

        // Category
        ui.horizontal(|ui| {
            ui.label("Category:");
            egui::ComboBox::from_id_source("item_cat")
                .selected_text(item.category.name())
                .show_ui(ui, |ui| {
                    for cat in ItemCategory::all() {
                        let sel = &item.category == cat;
                        if ui.selectable_label(sel, cat.name()).clicked() {
                            item.category = cat.clone();
                        }
                    }
                });
        });

        // Rarity
        ui.horizontal(|ui| {
            ui.label("Rarity:");
            egui::ComboBox::from_id_source("item_rarity")
                .selected_text(RichText::new(item.rarity.name()).color(item.rarity.color()))
                .show_ui(ui, |ui| {
                    for rar in Rarity::all() {
                        let sel = &item.rarity == rar;
                        if ui.selectable_label(sel, RichText::new(rar.name()).color(rar.color())).clicked() {
                            item.rarity = rar.clone();
                        }
                    }
                });
        });

        // Equip slot
        ui.horizontal(|ui| {
            ui.label("Equip Slot:");
            let slot_name = item.equip_slot.as_ref().map(|s| s.name()).unwrap_or("None");
            egui::ComboBox::from_id_source("item_slot")
                .selected_text(slot_name)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(item.equip_slot.is_none(), "None").clicked() {
                        item.equip_slot = None;
                    }
                    for slot in EquipSlot::all() {
                        let sel = item.equip_slot.as_ref() == Some(slot);
                        if ui.selectable_label(sel, slot.name()).clicked() {
                            item.equip_slot = Some(slot.clone());
                        }
                    }
                });
        });

        ui.separator();

        // Icon
        ui.horizontal(|ui| {
            ui.label("Icon Glyph:");
            let mut s = item.icon_glyph.to_string();
            if ui.add(egui::TextEdit::singleline(&mut s).desired_width(30.0)).changed() {
                if let Some(c) = s.chars().next() { item.icon_glyph = c; }
            }
            ui.label("Icon Color:");
            let mut c = [
                item.icon_color.r() as f32 / 255.0,
                item.icon_color.g() as f32 / 255.0,
                item.icon_color.b() as f32 / 255.0,
            ];
            if ui.color_edit_button_rgb(&mut c).changed() {
                item.icon_color = Color32::from_rgb(
                    (c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8,
                );
            }
        });

        ui.separator();

        // Numeric fields
        ui.horizontal(|ui| {
            ui.label("Base Value:");
            ui.add(egui::DragValue::new(&mut item.base_value).range(0..=9999999).speed(1.0));
            ui.label("g");
        });
        ui.horizontal(|ui| {
            ui.label("Weight:");
            ui.add(egui::DragValue::new(&mut item.weight).range(0.0..=100.0).speed(0.01));
            ui.label("kg");
        });
        ui.horizontal(|ui| {
            ui.label("Max Stack:");
            ui.add(egui::DragValue::new(&mut item.max_stack).range(1..=9999).speed(1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Level Req:");
            ui.add(egui::DragValue::new(&mut item.level_req).range(1..=999).speed(1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Drop Chance:");
            ui.add(egui::Slider::new(&mut item.drop_chance, 0.0..=1.0));
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.checkbox(&mut item.unique, "Unique");
            ui.checkbox(&mut item.quest_item, "Quest Item");
            ui.checkbox(&mut item.craftable, "Craftable");
        });

        ui.separator();

        // Tags
        ui.label("Tags:");
        ui.horizontal_wrapped(|ui| {
            let mut to_remove: Option<usize> = None;
            for (ti, tag) in item.tags.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("[{}]", tag));
                    if ui.small_button("✖").clicked() { to_remove = Some(ti); }
                });
            }
            if let Some(ti) = to_remove { item.tags.remove(ti); }
        });
        // Collect tag-add request (deferred to after item borrow ends)
        let mut add_tag: Option<String> = None;
        let add_tag_clicked = ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.tag_input).hint_text("new tag").desired_width(80.0));
            ui.small_button("Add Tag").clicked()
        }).inner;
        if add_tag_clicked && !self.tag_input.is_empty() {
            add_tag = Some(self.tag_input.clone());
        }

        ui.separator();

        // Requirements
        ui.label("Requirements:");
        let mut to_remove_req: Option<usize> = None;
        for (ri, req) in item.requirements.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("Stat:");
                ui.text_edit_singleline(&mut req.stat);
                ui.label("≥");
                ui.add(egui::DragValue::new(&mut req.min_value).range(0.0..=9999.0).speed(0.5));
                if ui.small_button("✖").clicked() { to_remove_req = Some(ri); }
            });
        }
        if let Some(ri) = to_remove_req { item.requirements.remove(ri); }
        // Apply deferred tag add (item borrow ends after this point)
        if let Some(tag) = add_tag {
            if sel < self.library.items.len() {
                self.library.items[sel].tags.push(tag);
            }
            self.tag_input.clear();
        }
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.edit_new_req_stat);
            ui.add(egui::DragValue::new(&mut self.edit_new_req_val).range(0.0..=9999.0).speed(1.0));
            if ui.small_button("+ Req").clicked() {
                let req = StatRequirement { stat: self.edit_new_req_stat.clone(), min_value: self.edit_new_req_val };
                if sel < self.library.items.len() { self.library.items[sel].requirements.push(req); }
            }
        });
    }

    fn show_item_stats_editor(&mut self, ui: &mut egui::Ui, sel: usize) {
        ui.label("Item Stats:");
        let item = &mut self.library.items[sel];
        let mut to_remove: Option<usize> = None;

        egui::Grid::new("item_stats_grid")
            .num_columns(6)
            .striped(true)
            .min_col_width(60.0)
            .show(ui, |ui| {
                ui.label("Name");
                ui.label("Base");
                ui.label("Scaling");
                ui.label("Format");
                ui.label("Lv.10");
                ui.label("Del");
                ui.end_row();
                for (si, stat) in item.stats.iter_mut().enumerate() {
                    ui.text_edit_singleline(&mut stat.name);
                    ui.add(egui::DragValue::new(&mut stat.base_value).range(-9999.0..=99999.0).speed(0.5));
                    egui::ComboBox::from_id_source(format!("stat_scale_{}", si))
                        .selected_text(stat.scaling.name())
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(stat.scaling == StatScaling::None, "None").clicked() {
                                stat.scaling = StatScaling::None;
                            }
                            if ui.selectable_label(matches!(stat.scaling, StatScaling::Linear { .. }), "Linear").clicked() {
                                stat.scaling = StatScaling::Linear { per_level: 1.0 };
                            }
                            if ui.selectable_label(matches!(stat.scaling, StatScaling::Exponential { .. }), "Exponential").clicked() {
                                stat.scaling = StatScaling::Exponential { base: 1.1, exp: 1.0 };
                            }
                        });
                    match &mut stat.scaling {
                        StatScaling::None => { ui.label("—"); }
                        StatScaling::Linear { per_level } => {
                            ui.add(egui::DragValue::new(per_level).range(-100.0..=100.0).speed(0.1).prefix("+"));
                        }
                        StatScaling::Exponential { base, exp } => {
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(base).range(1.0..=3.0).speed(0.01));
                                ui.add(egui::DragValue::new(exp).range(0.1..=3.0).speed(0.01));
                            });
                        }
                    }
                    egui::ComboBox::from_id_source(format!("stat_fmt_{}", si))
                        .selected_text(stat.display_format.name())
                        .show_ui(ui, |ui| {
                            for fmt in [StatFormat::Integer, StatFormat::Decimal, StatFormat::Percent, StatFormat::PlusMinus] {
                                let sel = stat.display_format == fmt;
                                if ui.selectable_label(sel, fmt.name()).clicked() {
                                    stat.display_format = fmt;
                                }
                            }
                        });
                    let val_at_10 = stat.value_at_level(10);
                    ui.label(stat.display_format.format_value(val_at_10));
                    if ui.small_button("✖").clicked() { to_remove = Some(si); }
                    ui.end_row();
                }
            });

        if let Some(si) = to_remove { item.stats.remove(si); }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("New:");
            ui.text_edit_singleline(&mut self.edit_new_stat_name);
            ui.add(egui::DragValue::new(&mut self.edit_new_stat_value).range(-9999.0..=99999.0).speed(1.0));
            if ui.button("+ Add Stat").clicked() {
                let s = ItemStat::new(&self.edit_new_stat_name, self.edit_new_stat_value);
                if sel < self.library.items.len() {
                    self.library.items[sel].stats.push(s);
                }
            }
        });
    }

    fn show_item_effects_editor(&mut self, ui: &mut egui::Ui, sel: usize) {
        ui.label("Item Effects:");
        let item = &mut self.library.items[sel];
        let mut to_remove: Option<usize> = None;

        for (ei, effect) in item.effects.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", effect.type_name()));
                    // inline editor per effect type
                    match effect {
                        ItemEffect::HealHp(v) => { ui.add(egui::DragValue::new(v).range(0.0..=9999.0).speed(1.0)); ui.label("HP"); }
                        ItemEffect::HealMp(v) => { ui.add(egui::DragValue::new(v).range(0.0..=9999.0).speed(1.0)); ui.label("MP"); }
                        ItemEffect::BuffStat { stat, amount, duration } => {
                            ui.text_edit_singleline(stat);
                            ui.add(egui::DragValue::new(amount).range(-999.0..=999.0).speed(0.5));
                            ui.label("for");
                            ui.add(egui::DragValue::new(duration).range(0.0..=3600.0).speed(1.0));
                            ui.label("s");
                        }
                        ItemEffect::Damage { damage_type, amount } => {
                            ui.text_edit_singleline(damage_type);
                            ui.add(egui::DragValue::new(amount).range(0.0..=9999.0).speed(1.0));
                            ui.label("dmg");
                        }
                        ItemEffect::Spawn { entity } => { ui.text_edit_singleline(entity); }
                        ItemEffect::TriggerQuest(q) => { ui.text_edit_singleline(q); }
                        ItemEffect::UnlockAbility(a) => { ui.text_edit_singleline(a); }
                        ItemEffect::ApplyStatus { status, duration } => {
                            ui.text_edit_singleline(status);
                            ui.add(egui::DragValue::new(duration).range(0.0..=3600.0).speed(1.0));
                            ui.label("s");
                        }
                        ItemEffect::Teleport(loc) => { ui.text_edit_singleline(loc); }
                        ItemEffect::GiveItem { item_id, count } => {
                            ui.label("Item ID:");
                            ui.add(egui::DragValue::new(item_id).range(0..=99999).speed(1.0));
                            ui.label("x");
                            ui.add(egui::DragValue::new(count).range(1..=9999).speed(1.0));
                        }
                    }
                    if ui.small_button("✖").clicked() { to_remove = Some(ei); }
                });
            });
        }

        if let Some(ei) = to_remove { item.effects.remove(ei); }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Add Effect:");
            egui::ComboBox::from_id_source("new_effect_type")
                .selected_text(&self.effect_type_dropdown)
                .show_ui(ui, |ui| {
                    for ename in ItemEffect::all_type_names() {
                        if ui.selectable_label(&self.effect_type_dropdown == *ename, *ename).clicked() {
                            self.effect_type_dropdown = ename.to_string();
                        }
                    }
                });
            if ui.button("+ Add").clicked() {
                let new_effect = ItemEffect::default_for_type(&self.effect_type_dropdown);
                if sel < self.library.items.len() {
                    self.library.items[sel].effects.push(new_effect);
                }
            }
        });
    }

    fn show_item_recipe_editor(&mut self, ui: &mut egui::Ui, sel: usize) {
        let craftable = self.library.items[sel].craftable;

        ui.checkbox(&mut self.library.items[sel].craftable, "Craftable");

        if !craftable { return; }

        if self.library.items[sel].recipe.is_none() {
            self.library.items[sel].recipe = Some(Recipe::new());
        }

        // Precompute ingredient names before taking mutable borrow
        let ing_names_precomputed: Vec<String> = self.library.items.get(sel)
            .and_then(|it| it.recipe.as_ref())
            .map(|r| r.ingredients.iter()
                .map(|(id, _)| self.library.find_by_id(*id)
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| format!("ID#{}", id)))
                .collect())
            .unwrap_or_default();

        let item = &mut self.library.items[sel];
        if let Some(recipe) = &mut item.recipe {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Result Count:");
                ui.add(egui::DragValue::new(&mut recipe.result_count).range(1..=999).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Craft Time:");
                ui.add(egui::DragValue::new(&mut recipe.time).range(0.1..=3600.0).speed(0.1));
                ui.label("s");
            });

            // Station
            ui.horizontal(|ui| {
                ui.label("Station:");
                let mut station_str = recipe.station.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut station_str).changed() {
                    recipe.station = if station_str.is_empty() { None } else { Some(station_str) };
                }
            });

            // Skill requirement
            ui.horizontal(|ui| {
                ui.label("Skill Req:");
                let mut skill_str = recipe.skill_req.as_ref().map(|(s, _)| s.clone()).unwrap_or_default();
                let mut skill_level = recipe.skill_req.as_ref().map(|(_, l)| *l).unwrap_or(1);
                if ui.text_edit_singleline(&mut skill_str).changed() || {
                    ui.add(egui::DragValue::new(&mut skill_level).range(1..=100).speed(1.0)).changed()
                } {
                    recipe.skill_req = if skill_str.is_empty() { None } else { Some((skill_str, skill_level)) };
                }
            });

            ui.separator();
            ui.label("Ingredients:");
            let mut to_remove: Option<usize> = None;
            for (ii, ((_ing_id, count), name)) in recipe.ingredients.iter_mut().zip(ing_names_precomputed.iter()).enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}  x", name));
                    ui.add(egui::DragValue::new(count).range(1..=999).speed(1.0));
                    if ui.small_button("✖").clicked() { to_remove = Some(ii); }
                });
            }
            if let Some(ii) = to_remove {
                if let Some(r) = &mut self.library.items[sel].recipe {
                    r.ingredients.remove(ii);
                }
            }

            // Add ingredient picker
            ui.horizontal(|ui| {
                ui.label("Add ingredient:");
                egui::ComboBox::from_id_source("ingredient_picker")
                    .selected_text("Pick item…")
                    .show_ui(ui, |ui| {
                        let ids: Vec<(u32, String)> = self.library.items.iter()
                            .filter(|i| i.id != self.library.items[sel].id)
                            .map(|i| (i.id, i.name.clone()))
                            .collect();
                        for (id, name) in ids {
                            if ui.selectable_label(false, &name).clicked() {
                                if let Some(r) = &mut self.library.items[sel].recipe {
                                    r.ingredients.push((id, 1));
                                }
                                ui.close_menu();
                            }
                        }
                    });
            });
        }
    }

    fn show_item_preview_card(&mut self, ui: &mut egui::Ui, sel: usize) {
        if sel >= self.library.items.len() { return; }
        let item = &self.library.items[sel];
        let rarity_color = item.rarity.color();

        ui.set_min_width(200.0);

        // Card border
        let card_rect = ui.available_rect_before_wrap();
        let painter = ui.painter();

        ui.group(|ui| {
            ui.vertical_centered(|ui| {
                // Large icon box
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(64.0, 64.0), egui::Sense::hover());
                let p = ui.painter();
                p.rect_filled(icon_rect, 8.0, Color32::from_rgb(20, 20, 30));
                p.rect_stroke(icon_rect, 8.0, Stroke::new(3.0, rarity_color), egui::StrokeKind::Inside);
                p.text(
                    icon_rect.center(), egui::Align2::CENTER_CENTER,
                    item.icon_glyph.to_string(), FontId::monospace(36.0), item.icon_color,
                );

                // Name
                ui.add_space(4.0);
                ui.label(RichText::new(&item.name).size(16.0).color(rarity_color).strong());
                // Rarity + category
                ui.label(RichText::new(format!("{} {}", item.rarity.name(), item.category.name())).small().color(rarity_color));
            });

            ui.separator();

            // Level req
            if item.level_req > 1 {
                ui.label(RichText::new(format!("Required Level: {}", item.level_req)).small().color(Color32::from_rgb(200, 180, 100)));
            }

            // Stats list
            if !item.stats.is_empty() {
                ui.separator();
                for stat in &item.stats {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&stat.name).small().color(Color32::from_gray(180)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(stat.display_format.format_value(stat.base_value)).small().color(Color32::from_rgb(100, 220, 100)));
                        });
                    });
                }
            }

            // Effects list
            if !item.effects.is_empty() {
                ui.separator();
                for effect in &item.effects {
                    ui.label(RichText::new(format!("  {}", effect.description())).small().color(Color32::from_rgb(180, 140, 255)));
                }
            }

            // Requirements
            if !item.requirements.is_empty() {
                ui.separator();
                for req in &item.requirements {
                    ui.label(RichText::new(format!("Requires {}: {}", req.stat, req.min_value)).small().color(Color32::from_rgb(255, 150, 100)));
                }
            }

            // Tags
            if !item.tags.is_empty() {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    for tag in &item.tags {
                        ui.label(RichText::new(format!("[{}]", tag)).small().color(Color32::from_gray(150)));
                    }
                });
            }

            ui.separator();

            // Value + weight
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}g", item.base_value)).small().color(Color32::from_rgb(220, 180, 30)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.2}kg", item.weight)).small().color(Color32::from_gray(150)));
                });
            });

            // Stack
            if item.max_stack > 1 {
                ui.label(RichText::new(format!("Stacks up to {}", item.max_stack)).small().color(Color32::from_gray(130)));
            }

            // Flags
            let mut flags = Vec::new();
            if item.unique { flags.push("Unique"); }
            if item.quest_item { flags.push("Quest Item"); }
            if item.craftable { flags.push("Craftable"); }
            if !flags.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for f in flags {
                        ui.label(RichText::new(f).small().color(Color32::from_rgb(100, 200, 255)));
                    }
                });
            }

            // Lore text (italic)
            if !item.lore.is_empty() {
                ui.separator();
                ui.label(RichText::new(format!("\"{}\"", item.lore)).italics().small().color(Color32::from_gray(120)));
            }

            // Description
            if !item.description.is_empty() {
                ui.separator();
                ui.label(RichText::new(&item.description).small().color(Color32::from_gray(180)));
            }
        });
    }

    // --------------------------------------------------------
    // LOOT TABLES VIEW
    // --------------------------------------------------------

    fn show_loot_tables_view(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        ui.horizontal(|ui| {
            // Left: table list
            ui.vertical(|ui| {
                ui.set_min_width(180.0);
                ui.set_max_width(180.0);
                ui.heading("Loot Tables");
                if ui.button("+ New Table").clicked() {
                    self.loot_tables.push(LootTable::new("New Loot Table"));
                    self.selected_loot_table = Some(self.loot_tables.len() - 1);
                }
                ui.separator();
                let mut to_delete: Option<usize> = None;
                let mut to_select: Option<usize> = None;
                for (i, table) in self.loot_tables.iter().enumerate() {
                    let sel = self.selected_loot_table == Some(i);
                    ui.horizontal(|ui| {
                        if ui.selectable_label(sel, &table.name).clicked() { to_select = Some(i); }
                        if ui.small_button("✖").clicked() { to_delete = Some(i); }
                    });
                }
                if let Some(i) = to_select { self.selected_loot_table = Some(i); }
                if let Some(i) = to_delete {
                    self.loot_tables.remove(i);
                    if self.selected_loot_table == Some(i) { self.selected_loot_table = None; }
                    else if let Some(s) = self.selected_loot_table { if s > i { self.selected_loot_table = Some(s - 1); } }
                }
            });

            ui.separator();

            // Right: table editor
            if let Some(tbl_idx) = self.selected_loot_table {
                if tbl_idx < self.loot_tables.len() {
                    ui.vertical(|ui| {
                        self.show_loot_table_editor(ui, tbl_idx);
                    });
                }
            } else {
                ui.label("Select a loot table to edit.");
            }
        });
    }

    fn show_loot_table_editor(&mut self, ui: &mut egui::Ui, tbl_idx: usize) {
        let table = &mut self.loot_tables[tbl_idx];
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut table.name);
        });
        let total_w = table.total_weight();
        ui.label(format!("Entries: {}   Total Weight: {:.1}", table.entries.len(), total_w));

        ui.separator();

        let mut to_remove: Option<usize> = None;
        let mut to_move_up: Option<usize> = None;
        let mut to_move_down: Option<usize> = None;

        // Entries table
        egui::Grid::new("loot_table_grid")
            .num_columns(7)
            .striped(true)
            .min_col_width(60.0)
            .show(ui, |ui| {
                ui.label("Item");
                ui.label("Weight");
                ui.label("%");
                ui.label("Min");
                ui.label("Max");
                ui.label("Condition");
                ui.label("Actions");
                ui.end_row();

                for (ei, entry) in table.entries.iter_mut().enumerate() {
                    let item_name = self.library.find_by_id(entry.item_id)
                        .map(|i| i.name.clone())
                        .unwrap_or_else(|| format!("#{}", entry.item_id));
                    ui.label(&item_name);
                    ui.add(egui::DragValue::new(&mut entry.weight).range(0.0..=9999.0).speed(0.1));
                    let pct = if total_w > 0.0 { entry.weight / total_w * 100.0 } else { 0.0 };
                    ui.label(format!("{:.1}%", pct));
                    ui.add(egui::DragValue::new(&mut entry.min_count).range(1..=9999).speed(1.0));
                    ui.add(egui::DragValue::new(&mut entry.max_count).range(1..=9999).speed(1.0));
                    let mut cond = entry.condition.clone().unwrap_or_default();
                    if ui.add(egui::TextEdit::singleline(&mut cond).desired_width(80.0)).changed() {
                        entry.condition = if cond.is_empty() { None } else { Some(cond) };
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("↑").clicked() { to_move_up = Some(ei); }
                        if ui.small_button("↓").clicked() { to_move_down = Some(ei); }
                        if ui.small_button("✖").clicked() { to_remove = Some(ei); }
                    });
                    ui.end_row();
                }
            });

        if let Some(ei) = to_remove { table.entries.remove(ei); }
        if let Some(ei) = to_move_up { if ei > 0 { table.entries.swap(ei, ei-1); } }
        if let Some(ei) = to_move_down { if ei+1 < table.entries.len() { table.entries.swap(ei, ei+1); } }

        ui.separator();
        // Add entry
        ui.horizontal(|ui| {
            ui.label("Add entry:");
            egui::ComboBox::from_id_source("loot_add_item")
                .selected_text("Pick item…")
                .show_ui(ui, |ui| {
                    let items_snap: Vec<(u32, String)> = self.library.items.iter()
                        .map(|i| (i.id, i.name.clone())).collect();
                    for (id, name) in items_snap {
                        if ui.selectable_label(false, &name).clicked() {
                            self.loot_tables[tbl_idx].entries.push(LootEntry::new(id));
                            ui.close_menu();
                        }
                    }
                });
        });

        ui.separator();
        // Simulate rolls
        ui.label("Simulate Rolls:");
        ui.horizontal(|ui| {
            ui.label("N:");
            ui.add(egui::DragValue::new(&mut self.loot_sim_n).range(1..=1000).speed(1.0));
            ui.label("Seed:");
            ui.add(egui::DragValue::new(&mut self.loot_sim_seed).speed(1.0));
            if ui.button("Roll").clicked() {
                self.loot_sim_results = self.loot_tables[tbl_idx].simulate_rolls(self.loot_sim_n, self.loot_sim_seed);
            }
        });
        if !self.loot_sim_results.is_empty() {
            // aggregate
            let mut counts: HashMap<u32, u32> = HashMap::new();
            for &(item_id, count) in &self.loot_sim_results {
                *counts.entry(item_id).or_insert(0) += count;
            }
            let mut sorted: Vec<(u32, u32)> = counts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            ui.label(format!("Results over {} rolls:", self.loot_sim_n));
            for (item_id, count) in sorted.iter().take(15) {
                let name = self.library.find_by_id(*item_id)
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| format!("#{}", item_id));
                ui.label(format!("  {} x{}", name, count));
            }
        }
    }

    // --------------------------------------------------------
    // SHOPS VIEW
    // --------------------------------------------------------

    fn show_shops_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Shop list
            ui.vertical(|ui| {
                ui.set_min_width(160.0);
                ui.set_max_width(160.0);
                ui.heading("Shops");
                if ui.button("+ New Shop").clicked() {
                    self.shops.push(Shop::new("New Shop"));
                    self.selected_shop = Some(self.shops.len() - 1);
                }
                ui.separator();
                let mut to_delete: Option<usize> = None;
                let mut to_select: Option<usize> = None;
                for (i, shop) in self.shops.iter().enumerate() {
                    let sel = self.selected_shop == Some(i);
                    ui.horizontal(|ui| {
                        if ui.selectable_label(sel, &shop.name).clicked() { to_select = Some(i); }
                        if ui.small_button("✖").clicked() { to_delete = Some(i); }
                    });
                }
                if let Some(i) = to_select { self.selected_shop = Some(i); }
                if let Some(i) = to_delete {
                    self.shops.remove(i);
                    if self.selected_shop == Some(i) { self.selected_shop = None; }
                    else if let Some(s) = self.selected_shop { if s > i { self.selected_shop = Some(s - 1); } }
                }
            });

            ui.separator();

            if let Some(shop_idx) = self.selected_shop {
                if shop_idx < self.shops.len() {
                    ui.vertical(|ui| {
                        self.show_shop_editor(ui, shop_idx);
                    });
                }
            } else {
                ui.label("Select a shop to edit.");
            }
        });
    }

    fn show_shop_editor(&mut self, ui: &mut egui::Ui, shop_idx: usize) {
        {
            let shop = &mut self.shops[shop_idx];
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut shop.name);
            });
            ui.horizontal(|ui| {
                ui.label("Currency:");
                ui.text_edit_singleline(&mut shop.currency);
                ui.label("Markup:");
                ui.add(egui::DragValue::new(&mut shop.markup).range(0.5..=5.0).speed(0.01));
                ui.label("Restock:");
                ui.add(egui::DragValue::new(&mut shop.restock_interval).range(0.0..=86400.0).speed(10.0));
                ui.label("s");
            });
        }

        ui.separator();
        ui.label("Shop Items:");

        let mut to_remove: Option<usize> = None;
        {
            let currency = self.shops[shop_idx].currency.clone();
            egui::Grid::new("shop_items_grid")
                .num_columns(4)
                .striped(true)
                .min_col_width(80.0)
                .show(ui, |ui| {
                    ui.label("Item");
                    ui.label(format!("Price ({})", currency));
                    ui.label("Stock");
                    ui.label("Del");
                    ui.end_row();

                    for (si, entry) in self.shops[shop_idx].items.iter_mut().enumerate() {
                        let name = self.library.find_by_id(entry.item_id)
                            .map(|i| i.name.clone())
                            .unwrap_or_else(|| format!("#{}", entry.item_id));
                        ui.label(&name);
                        ui.add(egui::DragValue::new(&mut entry.price).range(0..=9999999).speed(1.0));
                        let mut has_stock = entry.stock.is_some();
                        if ui.checkbox(&mut has_stock, "").changed() {
                            entry.stock = if has_stock { Some(5) } else { None };
                        }
                        if let Some(stock) = &mut entry.stock {
                            ui.add(egui::DragValue::new(stock).range(0..=9999).speed(1.0));
                        } else {
                            ui.label("∞");
                        }
                        if ui.small_button("✖").clicked() { to_remove = Some(si); }
                        ui.end_row();
                    }
                });
        }
        if let Some(si) = to_remove { self.shops[shop_idx].items.remove(si); }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Add item:");
            egui::ComboBox::from_id_source("shop_add_item")
                .selected_text("Pick item…")
                .show_ui(ui, |ui| {
                    let markup = self.shops[shop_idx].markup;
                    let items_snap: Vec<(u32, String, u32)> = self.library.items.iter()
                        .map(|i| (i.id, i.name.clone(), (i.base_value as f32 * markup) as u32)).collect();
                    for (id, name, price) in items_snap {
                        if ui.selectable_label(false, format!("{} ({}g)", name, price)).clicked() {
                            self.shops[shop_idx].items.push(ShopEntry { item_id: id, price, stock: None });
                            ui.close_menu();
                        }
                    }
                });
        });

        ui.separator();
        // Purchase preview
        ui.label("Purchase Preview:");
        ui.horizontal(|ui| {
            ui.label("Simulate buying:");
            let items_snap: Vec<(u32, String, u32)> = self.shops[shop_idx].items.iter().map(|e| {
                let name = self.library.find_by_id(e.item_id)
                    .map(|i| i.name.clone()).unwrap_or_default();
                (e.item_id, name, e.price)
            }).collect();
            for (id, name, price) in &items_snap {
                if ui.button(format!("{} ({}g)", name, price)).clicked() {
                    let currency = self.shops[shop_idx].currency.clone();
                    self.shop_sim_result = Some(format!(
                        "Purchased '{}' for {} {}.",
                        name, price, currency
                    ));
                }
            }
        });
        if let Some(result) = &self.shop_sim_result {
            ui.label(RichText::new(result).color(Color32::from_rgb(100, 220, 100)));
        }
    }

    // --------------------------------------------------------
    // CRAFTING VIEW
    // --------------------------------------------------------

    fn show_crafting_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Crafting Tree");
        ui.separator();

        let craftable_items: Vec<usize> = self.library.items.iter().enumerate()
            .filter(|(_, i)| i.craftable && i.recipe.is_some())
            .map(|(idx, _)| idx)
            .collect();

        if craftable_items.is_empty() {
            ui.label("No craftable items defined. Mark items as Craftable and add a Recipe.");
            return;
        }

        ui.horizontal(|ui| {
            ui.label("Filter by station:");
            // gather all stations
            let stations: HashSet<String> = self.library.items.iter()
                .filter_map(|i| i.recipe.as_ref().and_then(|r| r.station.clone()))
                .collect();
            for station in &stations {
                ui.label(station);
            }
        });
        ui.separator();

        egui::ScrollArea::vertical().id_source("crafting_scroll").show(ui, |ui| {
            for &item_idx in &craftable_items {
                let item = &self.library.items[item_idx];
                let rarity_color = item.rarity.color();

                let is_open = self.show_crafting_tree_for == Some(item_idx);
                let header = format!("{} {} ({})", item.icon_glyph, item.name, item.rarity.name());
                let header_rich = RichText::new(header).color(rarity_color);

                let resp = ui.horizontal(|ui| {
                    let arrow = if is_open { "▼" } else { "▶" };
                    ui.label(arrow);
                    ui.label(header_rich)
                });

                if resp.response.clicked() || resp.inner.clicked() {
                    self.show_crafting_tree_for = if is_open { None } else { Some(item_idx) };
                }

                if is_open {
                    if let Some(recipe) = &item.recipe {
                        ui.indent("craft_indent", |ui| {
                            if let Some(station) = &recipe.station {
                                ui.label(format!("Station: {}", station));
                            }
                            if let Some((skill, level)) = &recipe.skill_req {
                                ui.label(format!("Requires {} Lv.{}", skill, level));
                            }
                            ui.label(format!("Craft Time: {:.1}s  Yields: {}", recipe.time, recipe.result_count));
                            ui.label("Ingredients:");
                            for &(ing_id, count) in &recipe.ingredients {
                                let ing_name = self.library.find_by_id(ing_id)
                                    .map(|i| format!("{} {}", i.icon_glyph, i.name))
                                    .unwrap_or_else(|| format!("Item #{}", ing_id));
                                // Check if ingredient is also craftable (sub-recipe)
                                let sub_craftable = self.library.find_by_id(ing_id)
                                    .map(|i| i.craftable && i.recipe.is_some())
                                    .unwrap_or(false);
                                ui.horizontal(|ui| {
                                    ui.label(format!("  x{}  {}", count, ing_name));
                                    if sub_craftable {
                                        ui.label(RichText::new("[craftable]").small().color(Color32::from_rgb(100, 200, 100)));
                                    }
                                });
                            }
                        });
                    }
                }
            }
        });

        ui.separator();
        // Crafting stats summary
        ui.label(format!("Total craftable items: {}", craftable_items.len()));
        let mut station_counts: HashMap<String, usize> = HashMap::new();
        for &idx in &craftable_items {
            if let Some(recipe) = &self.library.items[idx].recipe {
                let station = recipe.station.clone().unwrap_or_else(|| "Hand".to_string());
                *station_counts.entry(station).or_insert(0) += 1;
            }
        }
        for (station, count) in &station_counts {
            ui.label(format!("  {}: {} recipes", station, count));
        }
    }
}

// ============================================================
// SIMULATION UTILITIES
// ============================================================

/// Simulate opening loot from multiple enemy kills
pub struct LootSimulation {
    pub results: Vec<(String, u32)>, // (item_name, total_count)
    pub total_rolls: u32,
    pub unique_items: usize,
}

pub fn simulate_loot_farming(
    table: &LootTable,
    library: &ItemLibrary,
    kills: u32,
    seed: u64,
) -> LootSimulation {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for i in 0..kills {
        if let Some((item_id, count)) = table.roll(seed.wrapping_add(i as u64 * 7919)) {
            *counts.entry(item_id).or_insert(0) += count;
        }
    }
    let mut results: Vec<(String, u32)> = counts.iter().map(|(&id, &count)| {
        let name = library.find_by_id(id).map(|i| i.name.clone()).unwrap_or_else(|| format!("#{}", id));
        (name, count)
    }).collect();
    results.sort_by(|a, b| b.1.cmp(&a.1));
    let unique = results.len();
    LootSimulation {
        results,
        total_rolls: kills,
        unique_items: unique,
    }
}

/// Compute drop rates for items in a loot table
pub fn compute_drop_rates(table: &LootTable) -> Vec<(u32, f32)> {
    let total = table.total_weight();
    if total <= 0.0 { return Vec::new(); }
    table.entries.iter().map(|e| (e.item_id, e.weight / total)).collect()
}

/// Build a complete item database with all items serialized to JSON-like string (for export)
pub fn export_library_summary(library: &ItemLibrary) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Item Library: {} items", library.items.len()));
    for item in &library.items {
        lines.push(format!(
            "  [{:04}] {} '{}' {} {} {}g {:.2}kg lv.{}",
            item.id, item.icon_glyph, item.name,
            item.rarity.name(), item.category.name(),
            item.base_value, item.weight, item.level_req
        ));
        for stat in &item.stats {
            lines.push(format!("    Stat: {} = {}", stat.name, stat.base_value));
        }
        for effect in &item.effects {
            lines.push(format!("    Effect: {}", effect.description()));
        }
        if item.craftable {
            if let Some(recipe) = &item.recipe {
                lines.push(format!("    Recipe: {} ingredients, {}s craft time", recipe.ingredients.len(), recipe.time));
            }
        }
    }
    lines.join("\n")
}

/// Generate an item set (linked items with bonuses)
pub struct ItemSet {
    pub name: String,
    pub items: Vec<u32>,
    pub set_bonuses: Vec<(u32, Vec<ItemStat>)>, // (pieces_required, bonuses)
}

impl ItemSet {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), items: Vec::new(), set_bonuses: Vec::new() }
    }

    pub fn add_bonus(&mut self, pieces: u32, bonuses: Vec<ItemStat>) {
        self.set_bonuses.push((pieces, bonuses));
    }

    pub fn active_bonuses(&self, equipped_count: u32) -> Vec<&ItemStat> {
        let mut result = Vec::new();
        for (required, stats) in &self.set_bonuses {
            if equipped_count >= *required {
                result.extend(stats.iter());
            }
        }
        result
    }
}

/// Item rarity upgrade: chance to promote an item to next rarity
pub fn try_upgrade_rarity(item: &mut Item, chance: f32, seed: u64) -> bool {
    let mut rng = seed;
    rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
    let r = (rng & 0xFFFFFF) as f32 / 16777216.0;
    if r < chance {
        item.rarity = match item.rarity {
            Rarity::Common => Rarity::Uncommon,
            Rarity::Uncommon => Rarity::Rare,
            Rarity::Rare => Rarity::Epic,
            Rarity::Epic => Rarity::Legendary,
            Rarity::Legendary => Rarity::Mythic,
            Rarity::Mythic => Rarity::Unique,
            Rarity::Unique => Rarity::Unique,
        };
        true
    } else {
        false
    }
}

/// Scale item stats based on level
pub fn scale_item_for_level(item: &Item, level: u32) -> Vec<(String, f32)> {
    item.stats.iter().map(|s| (s.name.clone(), s.value_at_level(level))).collect()
}

/// Check if a character meets all requirements to use an item
pub fn can_use_item(item: &Item, char_level: u32, char_stats: &HashMap<String, f32>) -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    if char_level < item.level_req {
        reasons.push(format!("Requires level {}", item.level_req));
    }
    for req in &item.requirements {
        let val = char_stats.get(&req.stat).copied().unwrap_or(0.0);
        if val < req.min_value {
            reasons.push(format!("Requires {} >= {}", req.stat, req.min_value));
        }
    }
    (reasons.is_empty(), reasons)
}

/// Generate a random item with given rarity constraints
pub fn generate_random_item(
    id: u32,
    rarity: Rarity,
    category: Option<ItemCategory>,
    level: u32,
    seed: u64,
) -> Item {
    let mut rng = seed ^ (id as u64 * 6364136223846793005);
    let rng_f = |r: &mut u64| -> f32 {
        *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
        (*r & 0xFFFFFF) as f32 / 16777216.0
    };

    let mut item = Item::new(id);
    item.rarity = rarity.clone();
    item.level_req = level;
    item.category = category.unwrap_or(ItemCategory::Weapon);

    let rarity_mult = match rarity {
        Rarity::Common => 1.0,
        Rarity::Uncommon => 1.5,
        Rarity::Rare => 2.5,
        Rarity::Epic => 4.0,
        Rarity::Legendary => 7.0,
        Rarity::Mythic => 12.0,
        Rarity::Unique => 20.0,
    };

    item.base_value = ((10 + level * 5) as f32 * rarity_mult) as u32;

    // Generate stats based on category
    match &item.category {
        ItemCategory::Weapon => {
            let base_atk = (10.0 + level as f32 * 2.0) * rarity_mult;
            item.stats.push(ItemStat {
                name: "Attack".to_string(),
                base_value: base_atk * (0.8 + rng_f(&mut rng) * 0.4),
                scaling: StatScaling::Linear { per_level: 1.5 + rng_f(&mut rng) * 1.5 },
                display_format: StatFormat::Integer,
            });
            item.equip_slot = Some(EquipSlot::MainHand);
        }
        ItemCategory::Armor => {
            let base_def = (8.0 + level as f32 * 1.5) * rarity_mult;
            item.stats.push(ItemStat {
                name: "Defense".to_string(),
                base_value: base_def * (0.8 + rng_f(&mut rng) * 0.4),
                scaling: StatScaling::Linear { per_level: 1.0 + rng_f(&mut rng) },
                display_format: StatFormat::Integer,
            });
            let slots = [EquipSlot::Head, EquipSlot::Chest, EquipSlot::Legs, EquipSlot::Feet];
            let slot_idx = (rng_f(&mut rng) * slots.len() as f32) as usize % slots.len();
            item.equip_slot = Some(slots[slot_idx].clone());
        }
        ItemCategory::Consumable => {
            item.max_stack = 20;
            let heal = (25.0 + level as f32 * 5.0) * rarity_mult;
            item.effects.push(ItemEffect::HealHp(heal * (0.8 + rng_f(&mut rng) * 0.4)));
        }
        _ => {}
    }

    let glyphs = ['!', '/', '[', '?', '*', '+', '-', '|', '^', '%'];
    let glyph_idx = (rng_f(&mut rng) * glyphs.len() as f32) as usize % glyphs.len();
    item.icon_glyph = glyphs[glyph_idx];

    let r = (rng_f(&mut rng) * 200.0 + 55.0) as u8;
    let g = (rng_f(&mut rng) * 200.0 + 55.0) as u8;
    let b = (rng_f(&mut rng) * 200.0 + 55.0) as u8;
    item.icon_color = Color32::from_rgb(r, g, b);

    item
}

/// Validate an item's recipe (all ingredients exist in library)
pub fn validate_recipe(item: &Item, library: &ItemLibrary) -> Vec<String> {
    let mut errors = Vec::new();
    if !item.craftable { return errors; }
    if let Some(recipe) = &item.recipe {
        if recipe.ingredients.is_empty() {
            errors.push("Recipe has no ingredients.".to_string());
        }
        for &(ing_id, count) in &recipe.ingredients {
            if library.find_by_id(ing_id).is_none() {
                errors.push(format!("Ingredient item #{} not found in library.", ing_id));
            }
            if count == 0 {
                errors.push(format!("Ingredient #{} has count 0.", ing_id));
            }
        }
        if recipe.result_count == 0 {
            errors.push("Recipe result_count is 0.".to_string());
        }
        if recipe.time <= 0.0 {
            errors.push("Recipe time must be > 0.".to_string());
        }
    } else {
        errors.push("Item is marked craftable but has no recipe.".to_string());
    }
    errors
}

/// Compute value-to-weight ratio for inventory optimization
pub fn value_weight_ratio(item: &Item) -> f32 {
    if item.weight <= 0.0 { return item.base_value as f32; }
    item.base_value as f32 / item.weight
}

/// Sort items by value-to-weight ratio (descending) — for merchant AI
pub fn sort_by_value_density(items: &mut Vec<Item>) {
    items.sort_by(|a, b| {
        let ra = value_weight_ratio(a);
        let rb = value_weight_ratio(b);
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_stat_scaling() {
        let stat = ItemStat {
            name: "Attack".to_string(),
            base_value: 10.0,
            scaling: StatScaling::Linear { per_level: 2.0 },
            display_format: StatFormat::Integer,
        };
        assert!((stat.value_at_level(5) - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_item_stat_exponential() {
        let stat = ItemStat {
            name: "Magic".to_string(),
            base_value: 10.0,
            scaling: StatScaling::Exponential { base: 1.1, exp: 1.0 },
            display_format: StatFormat::Decimal,
        };
        let v = stat.value_at_level(0);
        assert!((v - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_rarity_tier_order() {
        assert!(Rarity::Common.tier() < Rarity::Uncommon.tier());
        assert!(Rarity::Uncommon.tier() < Rarity::Rare.tier());
        assert!(Rarity::Rare.tier() < Rarity::Epic.tier());
        assert!(Rarity::Epic.tier() < Rarity::Legendary.tier());
        assert!(Rarity::Legendary.tier() < Rarity::Mythic.tier());
        assert!(Rarity::Mythic.tier() < Rarity::Unique.tier());
    }

    #[test]
    fn test_loot_table_roll() {
        let mut table = LootTable::new("test");
        table.entries.push(LootEntry { item_id: 1, weight: 7.0, min_count: 1, max_count: 1, condition: None });
        table.entries.push(LootEntry { item_id: 2, weight: 3.0, min_count: 1, max_count: 5, condition: None });
        let total = table.total_weight();
        assert!((total - 10.0).abs() < 0.001);
        // roll many times and check both items appear
        let mut seen: HashSet<u32> = HashSet::new();
        for i in 0..1000 {
            if let Some((id, _)) = table.roll(i * 1337) { seen.insert(id); }
        }
        assert!(seen.contains(&1));
        assert!(seen.contains(&2));
    }

    #[test]
    fn test_library_filter() {
        let lib = ItemLibrary::with_defaults();
        let weapons = lib.filtered(Some(&ItemCategory::Weapon), None, "");
        assert!(weapons.len() >= 2); // sword + staff
        let legendary = lib.filtered(None, Some(&Rarity::Legendary), "");
        assert!(legendary.len() >= 1); // dragon scale
    }

    #[test]
    fn test_item_value_at_level() {
        let item = Item::sword(1);
        let v0 = item.value_at_level(1);
        let v10 = item.value_at_level(10);
        assert!(v10 > v0);
    }

    #[test]
    fn test_can_use_item() {
        let item = Item::flame_staff(3);
        let mut stats = HashMap::new();
        stats.insert("Intelligence".to_string(), 25.0_f32);
        let (ok, _) = can_use_item(&item, 10, &stats);
        assert!(ok);
        let (fail, reasons) = can_use_item(&item, 5, &stats);
        assert!(!fail);
        assert!(!reasons.is_empty());
    }

    #[test]
    fn test_validate_recipe() {
        let lib = ItemLibrary::with_defaults();
        let mut item = Item::new(100);
        item.craftable = true;
        let mut recipe = Recipe::new();
        recipe.ingredients.push((1, 2)); // sword x2
        recipe.ingredients.push((999, 1)); // invalid id
        item.recipe = Some(recipe);
        let errors = validate_recipe(&item, &lib);
        assert!(!errors.is_empty()); // should report missing item 999
    }

    #[test]
    fn test_blend_mode_all() {
        let bm = BlendMode::Screen;
        let result = bm.apply(0.5, 0.5);
        assert!((result - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_generate_random_item() {
        let item = generate_random_item(42, Rarity::Rare, Some(ItemCategory::Weapon), 10, 12345);
        assert_eq!(item.id, 42);
        assert_eq!(item.rarity, Rarity::Rare);
        assert!(!item.stats.is_empty());
        assert!(item.equip_slot.is_some());
    }

    #[test]
    fn test_item_set_bonuses() {
        let mut set = ItemSet::new("Flame Set");
        set.add_bonus(2, vec![ItemStat::new("Fire Resistance", 15.0)]);
        set.add_bonus(4, vec![ItemStat::new("Fire Damage", 25.0)]);
        assert_eq!(set.active_bonuses(1).len(), 0);
        assert_eq!(set.active_bonuses(2).len(), 1);
        assert_eq!(set.active_bonuses(4).len(), 2);
    }

    #[test]
    fn test_value_weight_ratio() {
        let item = Item::sword(1);
        let ratio = value_weight_ratio(&item);
        assert!(ratio > 0.0);
    }

    #[test]
    fn test_stat_format() {
        assert_eq!(StatFormat::Integer.format_value(42.9), "42");
        assert_eq!(StatFormat::Percent.format_value(0.5), "50.0%");
        assert_eq!(StatFormat::PlusMinus.format_value(5.0), "+5.0");
        assert_eq!(StatFormat::PlusMinus.format_value(-3.0), "-3.0");
    }

    #[test]
    fn test_shop_price_calculation() {
        let shop = Shop::new("Test");
        let item = Item::sword(1); // base_value = 150
        let price = shop.price_for(&item);
        assert_eq!(price, (150.0 * 1.2) as u32);
    }

    #[test]
    fn test_drop_rates_sum_to_one() {
        let mut table = LootTable::new("test");
        table.entries.push(LootEntry { item_id: 1, weight: 5.0, min_count: 1, max_count: 1, condition: None });
        table.entries.push(LootEntry { item_id: 2, weight: 3.0, min_count: 1, max_count: 1, condition: None });
        table.entries.push(LootEntry { item_id: 3, weight: 2.0, min_count: 1, max_count: 1, condition: None });
        let rates = compute_drop_rates(&table);
        let sum: f32 = rates.iter().map(|(_, r)| r).sum();
        assert!((sum - 1.0).abs() < 0.001);
    }
}

// ============================================================
// ENCHANTMENT SYSTEM
// ============================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EnchantmentTarget {
    Weapon,
    Armor,
    Accessory,
    Any,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Enchantment {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub target: EnchantmentTarget,
    pub min_rarity: Rarity,
    pub max_level: u32,
    pub stats_per_level: Vec<ItemStat>,
    pub conflicts: Vec<u32>, // enchantment IDs that conflict
    pub cost_multiplier: f32,
    pub color: Color32,
}

impl Enchantment {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id, name: name.to_string(),
            description: String::new(),
            target: EnchantmentTarget::Any,
            min_rarity: Rarity::Common,
            max_level: 5,
            stats_per_level: Vec::new(),
            conflicts: Vec::new(),
            cost_multiplier: 1.0,
            color: Color32::from_rgb(120, 80, 200),
        }
    }
}

pub struct EnchantmentLibrary {
    pub enchantments: Vec<Enchantment>,
}

impl EnchantmentLibrary {
    pub fn new() -> Self {
        let mut lib = EnchantmentLibrary { enchantments: Vec::new() };

        let mut sharpness = Enchantment::new(1, "Sharpness");
        sharpness.description = "Increases weapon damage".to_string();
        sharpness.target = EnchantmentTarget::Weapon;
        sharpness.stats_per_level = vec![
            ItemStat { name: "Attack".to_string(), base_value: 3.0, scaling: StatScaling::Linear { per_level: 3.0 }, display_format: StatFormat::Integer },
        ];
        sharpness.color = Color32::from_rgb(200, 100, 50);
        lib.enchantments.push(sharpness);

        let mut protection = Enchantment::new(2, "Protection");
        protection.description = "Reduces incoming damage".to_string();
        protection.target = EnchantmentTarget::Armor;
        protection.stats_per_level = vec![
            ItemStat { name: "Defense".to_string(), base_value: 2.0, scaling: StatScaling::Linear { per_level: 2.0 }, display_format: StatFormat::Integer },
            ItemStat { name: "Damage Reduction".to_string(), base_value: 0.02, scaling: StatScaling::Linear { per_level: 0.02 }, display_format: StatFormat::Percent },
        ];
        protection.color = Color32::from_rgb(50, 100, 200);
        lib.enchantments.push(protection);

        let mut fire = Enchantment::new(3, "Flame");
        fire.description = "Adds fire damage to attacks".to_string();
        fire.target = EnchantmentTarget::Weapon;
        fire.stats_per_level = vec![
            ItemStat { name: "Fire Damage".to_string(), base_value: 5.0, scaling: StatScaling::Linear { per_level: 5.0 }, display_format: StatFormat::Integer },
        ];
        fire.conflicts = vec![4]; // conflicts with frost
        fire.color = Color32::from_rgb(255, 80, 30);
        lib.enchantments.push(fire);

        let mut frost = Enchantment::new(4, "Frost");
        frost.description = "Adds ice damage and slows enemies".to_string();
        frost.target = EnchantmentTarget::Weapon;
        frost.stats_per_level = vec![
            ItemStat { name: "Ice Damage".to_string(), base_value: 4.0, scaling: StatScaling::Linear { per_level: 4.0 }, display_format: StatFormat::Integer },
            ItemStat { name: "Slow Duration".to_string(), base_value: 2.0, scaling: StatScaling::Linear { per_level: 0.5 }, display_format: StatFormat::Decimal },
        ];
        frost.conflicts = vec![3]; // conflicts with flame
        frost.color = Color32::from_rgb(100, 180, 255);
        lib.enchantments.push(frost);

        let mut swift = Enchantment::new(5, "Swiftness");
        swift.description = "Increases movement speed when equipped".to_string();
        swift.target = EnchantmentTarget::Any;
        swift.stats_per_level = vec![
            ItemStat { name: "Move Speed".to_string(), base_value: 0.05, scaling: StatScaling::Linear { per_level: 0.05 }, display_format: StatFormat::Percent },
        ];
        swift.color = Color32::from_rgb(100, 220, 100);
        lib.enchantments.push(swift);

        lib
    }

    pub fn find(&self, id: u32) -> Option<&Enchantment> {
        self.enchantments.iter().find(|e| e.id == id)
    }

    pub fn valid_for_item(&self, item: &Item) -> Vec<&Enchantment> {
        self.enchantments.iter().filter(|e| {
            match e.target {
                EnchantmentTarget::Any => true,
                EnchantmentTarget::Weapon => item.category == ItemCategory::Weapon,
                EnchantmentTarget::Armor => item.category == ItemCategory::Armor,
                EnchantmentTarget::Accessory => item.category == ItemCategory::Accessory,
            }
        }).collect()
    }
}

/// Applied enchantment on a specific item instance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppliedEnchantment {
    pub enchantment_id: u32,
    pub level: u32,
}

/// Item instance (a specific copy of an item, possibly enchanted)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemInstance {
    pub base_item_id: u32,
    pub instance_id: u64,
    pub enchantments: Vec<AppliedEnchantment>,
    pub durability: f32,
    pub quality_modifier: f32,
    pub custom_name: Option<String>,
    pub soulbound: bool,
}

impl ItemInstance {
    pub fn new(base_item_id: u32, instance_id: u64) -> Self {
        Self {
            base_item_id,
            instance_id,
            enchantments: Vec::new(),
            durability: 1.0,
            quality_modifier: 1.0,
            custom_name: None,
            soulbound: false,
        }
    }

    pub fn display_name<'a>(&'a self, base_item: &'a Item) -> &'a str {
        self.custom_name.as_deref().unwrap_or(&base_item.name)
    }

    pub fn effective_stats(&self, base_item: &Item, ench_lib: &EnchantmentLibrary, level: u32) -> Vec<(String, f32)> {
        let mut stats: HashMap<String, f32> = HashMap::new();

        // Base stats
        for stat in &base_item.stats {
            *stats.entry(stat.name.clone()).or_insert(0.0) += stat.value_at_level(level) * self.quality_modifier;
        }

        // Enchantment stats
        for applied in &self.enchantments {
            if let Some(ench) = ench_lib.find(applied.enchantment_id) {
                for stat in &ench.stats_per_level {
                    *stats.entry(stat.name.clone()).or_insert(0.0) += stat.value_at_level(applied.level);
                }
            }
        }

        stats.into_iter().collect()
    }

    pub fn can_add_enchantment(&self, ench_id: u32, ench_lib: &EnchantmentLibrary) -> (bool, String) {
        // check if already enchanted with this
        if self.enchantments.iter().any(|a| a.enchantment_id == ench_id) {
            return (false, "Already has this enchantment.".to_string());
        }
        // check conflicts
        if let Some(ench) = ench_lib.find(ench_id) {
            for applied in &self.enchantments {
                if ench.conflicts.contains(&applied.enchantment_id) {
                    let conflict_name = ench_lib.find(applied.enchantment_id)
                        .map(|e| e.name.as_str()).unwrap_or("unknown");
                    return (false, format!("Conflicts with enchantment: {}", conflict_name));
                }
            }
            // Check max enchantments (simplified: 3 max)
            if self.enchantments.len() >= 3 {
                return (false, "Item already has maximum enchantments.".to_string());
            }
        } else {
            return (false, "Enchantment not found.".to_string());
        }
        (true, "OK".to_string())
    }
}

// ============================================================
// INVENTORY CONTAINER & EQUIPMENT SLOTS
// ============================================================

/// A character's equipment loadout
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Equipment {
    pub slots: HashMap<String, ItemInstance>,
}

impl Equipment {
    pub fn new() -> Self {
        Self { slots: HashMap::new() }
    }

    pub fn equip(&mut self, slot: &EquipSlot, instance: ItemInstance) -> Option<ItemInstance> {
        let key = slot.name().to_string();
        self.slots.insert(key, instance)
    }

    pub fn unequip(&mut self, slot: &EquipSlot) -> Option<ItemInstance> {
        self.slots.remove(slot.name())
    }

    pub fn get(&self, slot: &EquipSlot) -> Option<&ItemInstance> {
        self.slots.get(slot.name())
    }

    pub fn total_weight(&self, library: &ItemLibrary) -> f32 {
        self.slots.values().filter_map(|inst| {
            library.find_by_id(inst.base_item_id).map(|i| i.weight)
        }).sum()
    }

    pub fn combined_stats(&self, library: &ItemLibrary, ench_lib: &EnchantmentLibrary, level: u32) -> HashMap<String, f32> {
        let mut total: HashMap<String, f32> = HashMap::new();
        for inst in self.slots.values() {
            if let Some(item) = library.find_by_id(inst.base_item_id) {
                for (name, val) in inst.effective_stats(item, ench_lib, level) {
                    *total.entry(name).or_insert(0.0) += val;
                }
            }
        }
        total
    }
}

/// Inventory bag/container
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryBag {
    pub name: String,
    pub max_slots: usize,
    pub max_weight: f32,
    pub items: Vec<(ItemInstance, u32)>, // (instance, stack_count)
}

impl InventoryBag {
    pub fn new(name: &str, max_slots: usize, max_weight: f32) -> Self {
        Self { name: name.to_string(), max_slots, max_weight, items: Vec::new() }
    }

    pub fn current_weight(&self, library: &ItemLibrary) -> f32 {
        self.items.iter().filter_map(|(inst, count)| {
            library.find_by_id(inst.base_item_id).map(|i| i.weight * *count as f32)
        }).sum()
    }

    pub fn has_space(&self) -> bool {
        self.items.len() < self.max_slots
    }

    pub fn try_add(&mut self, instance: ItemInstance, count: u32, library: &ItemLibrary) -> Result<(), String> {
        // Try to stack first
        let item_id = instance.base_item_id;
        if let Some(item) = library.find_by_id(item_id) {
            if item.max_stack > 1 {
                for (existing, existing_count) in &mut self.items {
                    if existing.base_item_id == item_id && *existing_count < item.max_stack {
                        let can_add = (item.max_stack - *existing_count).min(count);
                        *existing_count += can_add;
                        if can_add == count { return Ok(()); }
                    }
                }
            }
        }
        // New slot
        if !self.has_space() {
            return Err("Inventory full.".to_string());
        }
        let new_weight = self.current_weight(library) + library.find_by_id(item_id).map(|i| i.weight).unwrap_or(0.0) * count as f32;
        if new_weight > self.max_weight {
            return Err("Too heavy.".to_string());
        }
        self.items.push((instance, count));
        Ok(())
    }

    pub fn remove(&mut self, slot: usize, count: u32) -> Option<(ItemInstance, u32)> {
        if slot >= self.items.len() { return None; }
        let (inst, existing_count) = &mut self.items[slot];
        if count >= *existing_count {
            Some(self.items.remove(slot))
        } else {
            *existing_count -= count;
            Some((inst.clone(), count))
        }
    }

    pub fn find_item(&self, item_id: u32) -> Option<(usize, u32)> {
        self.items.iter().enumerate().find_map(|(i, (inst, count))| {
            if inst.base_item_id == item_id { Some((i, *count)) } else { None }
        })
    }

    pub fn total_count(&self, item_id: u32) -> u32 {
        self.items.iter().filter_map(|(inst, count)| {
            if inst.base_item_id == item_id { Some(*count) } else { None }
        }).sum()
    }
}

// ============================================================
// MATERIAL GRADE SYSTEM
// ============================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MaterialGrade {
    Crude,
    Standard,
    Fine,
    Superior,
    Masterwork,
    Legendary,
}

impl MaterialGrade {
    pub fn name(&self) -> &str {
        match self {
            MaterialGrade::Crude => "Crude",
            MaterialGrade::Standard => "Standard",
            MaterialGrade::Fine => "Fine",
            MaterialGrade::Superior => "Superior",
            MaterialGrade::Masterwork => "Masterwork",
            MaterialGrade::Legendary => "Legendary",
        }
    }

    pub fn stat_multiplier(&self) -> f32 {
        match self {
            MaterialGrade::Crude => 0.7,
            MaterialGrade::Standard => 1.0,
            MaterialGrade::Fine => 1.15,
            MaterialGrade::Superior => 1.35,
            MaterialGrade::Masterwork => 1.6,
            MaterialGrade::Legendary => 2.0,
        }
    }

    pub fn value_multiplier(&self) -> f32 {
        match self {
            MaterialGrade::Crude => 0.5,
            MaterialGrade::Standard => 1.0,
            MaterialGrade::Fine => 2.0,
            MaterialGrade::Superior => 4.0,
            MaterialGrade::Masterwork => 10.0,
            MaterialGrade::Legendary => 25.0,
        }
    }
}

// ============================================================
// QUEST ITEM PROGRESSION
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestItemState {
    pub item_id: u32,
    pub obtained: bool,
    pub quest_id: String,
    pub stage: u32,
    pub notes: String,
}

pub struct QuestItemTracker {
    pub states: Vec<QuestItemState>,
}

impl QuestItemTracker {
    pub fn new() -> Self {
        Self { states: Vec::new() }
    }

    pub fn register(&mut self, item_id: u32, quest_id: &str) {
        if !self.states.iter().any(|s| s.item_id == item_id) {
            self.states.push(QuestItemState {
                item_id,
                obtained: false,
                quest_id: quest_id.to_string(),
                stage: 0,
                notes: String::new(),
            });
        }
    }

    pub fn mark_obtained(&mut self, item_id: u32) -> bool {
        if let Some(state) = self.states.iter_mut().find(|s| s.item_id == item_id) {
            state.obtained = true;
            true
        } else {
            false
        }
    }

    pub fn advance_stage(&mut self, item_id: u32) {
        if let Some(state) = self.states.iter_mut().find(|s| s.item_id == item_id) {
            state.stage += 1;
        }
    }

    pub fn all_for_quest(&self, quest_id: &str) -> Vec<&QuestItemState> {
        self.states.iter().filter(|s| s.quest_id == quest_id).collect()
    }

    pub fn quest_complete(&self, quest_id: &str) -> bool {
        self.all_for_quest(quest_id).iter().all(|s| s.obtained)
    }
}

// ============================================================
// ITEM MARKET ECONOMY SIMULATION
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketPrice {
    pub item_id: u32,
    pub buy_price: u32,
    pub sell_price: u32,
    pub supply: u32,
    pub demand: u32,
    pub trend: f32, // -1.0 = falling, 0 = stable, 1.0 = rising
}

impl MarketPrice {
    pub fn from_item(item: &Item, markup: f32) -> Self {
        Self {
            item_id: item.id,
            buy_price: (item.base_value as f32 * markup) as u32,
            sell_price: (item.base_value as f32 * 0.6) as u32,
            supply: 10,
            demand: 5,
            trend: 0.0,
        }
    }

    pub fn effective_buy_price(&self) -> u32 {
        let demand_factor = if self.demand > self.supply {
            1.0 + (self.demand - self.supply) as f32 * 0.05
        } else { 1.0 };
        (self.buy_price as f32 * demand_factor * (1.0 + self.trend * 0.1)) as u32
    }
}

pub struct EconomySimulator {
    pub prices: Vec<MarketPrice>,
    pub tick: u64,
}

impl EconomySimulator {
    pub fn new(library: &ItemLibrary, base_markup: f32) -> Self {
        let prices = library.items.iter()
            .map(|item| MarketPrice::from_item(item, base_markup))
            .collect();
        Self { prices, tick: 0 }
    }

    pub fn simulate_tick(&mut self, seed: u64) {
        let mut rng = seed.wrapping_add(self.tick * 6364136223846793005);
        let rng_f = |r: &mut u64| -> f32 {
            *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
            (*r & 0xFFFFFF) as f32 / 16777216.0 * 2.0 - 1.0
        };

        for price in &mut self.prices {
            // Random demand/supply fluctuation
            let delta_demand = (rng_f(&mut rng) * 2.0).round() as i32;
            let delta_supply = (rng_f(&mut rng) * 2.0).round() as i32;
            price.demand = (price.demand as i32 + delta_demand).max(0) as u32;
            price.supply = (price.supply as i32 + delta_supply).max(1) as u32;

            // Update trend
            let market_pressure = (price.demand as f32 - price.supply as f32) / (price.supply as f32 + 1.0);
            price.trend = (price.trend * 0.8 + market_pressure * 0.2).clamp(-1.0, 1.0);
        }
        self.tick += 1;
    }

    pub fn get_price(&self, item_id: u32) -> Option<&MarketPrice> {
        self.prices.iter().find(|p| p.item_id == item_id)
    }
}

// ============================================================
// ITEM COMPARISON UTILITY
// ============================================================

#[derive(Clone, Debug)]
pub struct ItemComparison {
    pub item_a: u32,
    pub item_b: u32,
    pub stat_diffs: Vec<(String, f32, f32)>, // (name, a_val, b_val)
    pub value_diff: i64,
    pub weight_diff: f32,
}

pub fn compare_items(a: &Item, b: &Item, level: u32) -> ItemComparison {
    let mut stat_diffs = Vec::new();
    let mut all_stats: HashSet<String> = HashSet::new();
    for s in &a.stats { all_stats.insert(s.name.clone()); }
    for s in &b.stats { all_stats.insert(s.name.clone()); }

    for stat_name in &all_stats {
        let a_val = a.total_stat(stat_name, level).unwrap_or(0.0);
        let b_val = b.total_stat(stat_name, level).unwrap_or(0.0);
        stat_diffs.push((stat_name.clone(), a_val, b_val));
    }
    stat_diffs.sort_by(|x, y| x.0.cmp(&y.0));

    ItemComparison {
        item_a: a.id,
        item_b: b.id,
        stat_diffs,
        value_diff: a.base_value as i64 - b.base_value as i64,
        weight_diff: a.weight - b.weight,
    }
}

/// Show item comparison as a formatted report
pub fn format_comparison(comp: &ItemComparison, a_name: &str, b_name: &str) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Comparison: {} vs {}", a_name, b_name));
    lines.push(format!("Value: {} vs {} ({:+})",
        comp.item_a, comp.item_b, comp.value_diff));
    lines.push(format!("Weight diff: {:+.2}kg", comp.weight_diff));
    for (name, a_val, b_val) in &comp.stat_diffs {
        let diff = a_val - b_val;
        lines.push(format!("  {}: {:.1} vs {:.1} ({:+.1})", name, a_val, b_val, diff));
    }
    lines.join("\n")
}

// ============================================================
// ITEM TIER PROGRESSION TABLE
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TierTable {
    pub tiers: Vec<ItemTier>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemTier {
    pub tier: u32,
    pub name: String,
    pub level_range: (u32, u32),
    pub stat_multiplier: f32,
    pub available_rarities: Vec<Rarity>,
}

impl TierTable {
    pub fn default_rpg() -> Self {
        Self {
            tiers: vec![
                ItemTier { tier: 1, name: "Starter".to_string(), level_range: (1, 10), stat_multiplier: 1.0, available_rarities: vec![Rarity::Common, Rarity::Uncommon] },
                ItemTier { tier: 2, name: "Apprentice".to_string(), level_range: (11, 20), stat_multiplier: 1.5, available_rarities: vec![Rarity::Common, Rarity::Uncommon, Rarity::Rare] },
                ItemTier { tier: 3, name: "Journeyman".to_string(), level_range: (21, 35), stat_multiplier: 2.2, available_rarities: vec![Rarity::Uncommon, Rarity::Rare, Rarity::Epic] },
                ItemTier { tier: 4, name: "Expert".to_string(), level_range: (36, 50), stat_multiplier: 3.5, available_rarities: vec![Rarity::Rare, Rarity::Epic, Rarity::Legendary] },
                ItemTier { tier: 5, name: "Master".to_string(), level_range: (51, 70), stat_multiplier: 5.0, available_rarities: vec![Rarity::Epic, Rarity::Legendary, Rarity::Mythic] },
                ItemTier { tier: 6, name: "Grandmaster".to_string(), level_range: (71, 100), stat_multiplier: 8.0, available_rarities: vec![Rarity::Legendary, Rarity::Mythic, Rarity::Unique] },
            ],
        }
    }

    pub fn tier_for_level(&self, level: u32) -> Option<&ItemTier> {
        self.tiers.iter().find(|t| level >= t.level_range.0 && level <= t.level_range.1)
    }

    pub fn scale_stat(&self, base: f32, level: u32) -> f32 {
        if let Some(tier) = self.tier_for_level(level) {
            let level_progress = (level - tier.level_range.0) as f32 / (tier.level_range.1 - tier.level_range.0 + 1) as f32;
            base * tier.stat_multiplier * (1.0 + level_progress * 0.5)
        } else {
            base
        }
    }
}

// ============================================================
// LEGENDARY ITEM SPECIAL PROCS
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcEffect {
    pub name: String,
    pub trigger: ProcTrigger,
    pub chance: f32,
    pub effect: ItemEffect,
    pub cooldown: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProcTrigger {
    OnHit,
    OnKill,
    OnReceiveDamage,
    OnHeal,
    OnSkillUse,
    OnLevelUp,
}

impl ProcTrigger {
    pub fn name(&self) -> &str {
        match self {
            ProcTrigger::OnHit => "On Hit",
            ProcTrigger::OnKill => "On Kill",
            ProcTrigger::OnReceiveDamage => "On Receive Damage",
            ProcTrigger::OnHeal => "On Heal",
            ProcTrigger::OnSkillUse => "On Skill Use",
            ProcTrigger::OnLevelUp => "On Level Up",
        }
    }
}

impl ProcEffect {
    pub fn new(name: &str, trigger: ProcTrigger, chance: f32, effect: ItemEffect) -> Self {
        Self { name: name.to_string(), trigger, chance, effect, cooldown: 0.0 }
    }

    pub fn try_proc(&self, seed: u64) -> bool {
        let mut rng = seed;
        rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
        let r = (rng & 0xFFFFFF) as f32 / 16777216.0;
        r < self.chance
    }
}

// ============================================================
// FULL INVENTORY EDITOR EXTENDED PANEL
// ============================================================

/// Enchantment editor panel (embedded in item editor)
pub fn show_enchantment_panel(ui: &mut egui::Ui, instance: &mut ItemInstance, ench_lib: &EnchantmentLibrary, item: &Item) {
    ui.label("Enchantments:");
    let mut to_remove: Option<usize> = None;

    for (ai, applied) in instance.enchantments.iter_mut().enumerate() {
        if let Some(ench) = ench_lib.find(applied.enchantment_id) {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let rect = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover()).0;
                    ui.painter().rect_filled(rect, 2.0, ench.color);
                    ui.label(RichText::new(format!("{} Lv.{}", ench.name, applied.level)).color(ench.color));
                    if ui.small_button("↑").clicked() && applied.level < ench.max_level { applied.level += 1; }
                    if ui.small_button("↓").clicked() && applied.level > 1 { applied.level -= 1; }
                    if ui.small_button("✖").clicked() { to_remove = Some(ai); }
                });
                // show stat preview
                for stat in &ench.stats_per_level {
                    let val = stat.value_at_level(applied.level);
                    ui.label(RichText::new(format!("  +{} {}", stat.display_format.format_value(val), stat.name)).small().color(Color32::from_rgb(150, 200, 150)));
                }
            });
        }
    }
    if let Some(ai) = to_remove { instance.enchantments.remove(ai); }

    ui.separator();
    // Add enchantment
    let valid_enchs = ench_lib.valid_for_item(item);
    if !valid_enchs.is_empty() {
        egui::ComboBox::from_id_source("add_enchantment")
            .selected_text("Add enchantment…")
            .show_ui(ui, |ui| {
                for ench in &valid_enchs {
                    let (can, reason) = instance.can_add_enchantment(ench.id, ench_lib);
                    let label = if can {
                        RichText::new(&ench.name).color(ench.color)
                    } else {
                        RichText::new(format!("{} ({})", ench.name, reason)).color(Color32::GRAY)
                    };
                    if ui.selectable_label(false, label).clicked() && can {
                        instance.enchantments.push(AppliedEnchantment { enchantment_id: ench.id, level: 1 });
                    }
                }
            });
    } else {
        ui.label("No valid enchantments for this item type.");
    }
}

/// Inventory bag display panel
pub fn show_inventory_bag(ui: &mut egui::Ui, bag: &mut InventoryBag, library: &ItemLibrary) -> Option<(usize, u32)> {
    let mut clicked = None;
    ui.label(format!("{} — {}/{} slots", bag.name, bag.items.len(), bag.max_slots));
    let current_w = bag.current_weight(library);
    ui.label(format!("Weight: {:.1}/{:.1}kg", current_w, bag.max_weight));
    let fill = current_w / bag.max_weight;
    let bar_w = ui.available_width();
    let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 8.0), egui::Sense::hover());
    ui.painter().rect_filled(bar_rect, 2.0, Color32::from_rgb(40, 40, 50));
    let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(bar_w * fill.clamp(0.0, 1.0), 8.0));
    let bar_color = if fill > 0.9 { Color32::from_rgb(200, 60, 40) } else { Color32::from_rgb(80, 160, 80) };
    ui.painter().rect_filled(fill_rect, 2.0, bar_color);

    ui.separator();
    egui::ScrollArea::vertical().id_source("bag_scroll").max_height(200.0).show(ui, |ui| {
        for (slot, (inst, count)) in bag.items.iter().enumerate() {
            if let Some(item) = library.find_by_id(inst.base_item_id) {
                let rarity_color = item.rarity.color();
                let response = ui.horizontal(|ui| {
                    let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), egui::Sense::hover());
                    ui.painter().rect_filled(icon_rect, 2.0, Color32::from_rgb(25, 25, 35));
                    ui.painter().rect_stroke(icon_rect, 2.0, Stroke::new(1.0, rarity_color), egui::StrokeKind::Inside);
                    ui.painter().text(icon_rect.center(), egui::Align2::CENTER_CENTER, item.icon_glyph.to_string(), FontId::monospace(12.0), item.icon_color);
                    let name = inst.custom_name.as_deref().unwrap_or(&item.name);
                    ui.selectable_label(false, RichText::new(format!("{} x{}", name, count)).color(rarity_color))
                }).inner;
                if response.clicked() { clicked = Some((slot, *count)); }
            }
        }
    });
    clicked
}

/// Market price board display
pub fn show_market_board(ui: &mut egui::Ui, economy: &EconomySimulator, library: &ItemLibrary) {
    ui.heading("Market Prices");
    egui::Grid::new("market_grid")
        .num_columns(5)
        .striped(true)
        .show(ui, |ui| {
            ui.label("Item");
            ui.label("Buy");
            ui.label("Sell");
            ui.label("Supply/Demand");
            ui.label("Trend");
            ui.end_row();
            for price in economy.prices.iter().take(20) {
                if let Some(item) = library.find_by_id(price.item_id) {
                    ui.label(RichText::new(&item.name).color(item.rarity.color()));
                    ui.label(format!("{}g", price.effective_buy_price()));
                    ui.label(format!("{}g", price.sell_price));
                    ui.label(format!("{}/{}", price.supply, price.demand));
                    let trend_str = if price.trend > 0.1 { "↑" } else if price.trend < -0.1 { "↓" } else { "→" };
                    let trend_color = if price.trend > 0.1 { Color32::from_rgb(100, 220, 100) }
                        else if price.trend < -0.1 { Color32::from_rgb(220, 100, 100) }
                        else { Color32::GRAY };
                    ui.label(RichText::new(trend_str).color(trend_color));
                    ui.end_row();
                }
            }
        });
}

// ============================================================
// ADDITIONAL TESTS
// ============================================================

#[cfg(test)]
mod extended_tests {
    use super::*;

    #[test]
    fn test_enchantment_library() {
        let lib = EnchantmentLibrary::new();
        assert!(!lib.enchantments.is_empty());
        let sword = Item::sword(1);
        let valid = lib.valid_for_item(&sword);
        assert!(!valid.is_empty());
    }

    #[test]
    fn test_enchantment_conflict() {
        let lib = EnchantmentLibrary::new();
        let mut instance = ItemInstance::new(3, 1);
        // add flame (id=3)
        instance.enchantments.push(AppliedEnchantment { enchantment_id: 3, level: 1 });
        // try to add frost (id=4) — should conflict
        let (can, reason) = instance.can_add_enchantment(4, &lib);
        assert!(!can);
        assert!(reason.contains("Conflicts"));
    }

    #[test]
    fn test_item_instance_effective_stats() {
        let lib = ItemLibrary::with_defaults();
        let ench_lib = EnchantmentLibrary::new();
        let mut instance = ItemInstance::new(1, 1); // sword
        instance.enchantments.push(AppliedEnchantment { enchantment_id: 1, level: 2 }); // sharpness lv2
        let item = lib.find_by_id(1).unwrap();
        let stats = instance.effective_stats(item, &ench_lib, 1);
        let attack = stats.iter().find(|(n, _)| n == "Attack").map(|(_, v)| *v).unwrap_or(0.0);
        // sword base attack is 18, sharpness lv2 = 3 + 3*2 = 9 bonus
        assert!(attack > 18.0);
    }

    #[test]
    fn test_inventory_bag_add_remove() {
        let lib = ItemLibrary::with_defaults();
        let mut bag = InventoryBag::new("Test Bag", 10, 100.0);
        let inst = ItemInstance::new(5, 1); // gold coin
        let result = bag.try_add(inst, 50, &lib);
        assert!(result.is_ok());
        let count = bag.total_count(5);
        assert_eq!(count, 50);
    }

    #[test]
    fn test_inventory_bag_weight_limit() {
        let lib = ItemLibrary::with_defaults();
        let mut bag = InventoryBag::new("Small Bag", 10, 5.0);
        let inst = ItemInstance::new(1, 1); // sword (2.5kg)
        let r1 = bag.try_add(inst.clone(), 1, &lib);
        assert!(r1.is_ok());
        let r2 = bag.try_add(inst, 1, &lib); // another sword = 5kg, over limit
        assert!(r2.is_err());
    }

    #[test]
    fn test_equipment_combined_stats() {
        let lib = ItemLibrary::with_defaults();
        let ench_lib = EnchantmentLibrary::new();
        let mut equipment = Equipment::new();
        let sword_inst = ItemInstance::new(1, 1);
        let armor_inst = ItemInstance::new(4, 2);
        equipment.equip(&EquipSlot::MainHand, sword_inst);
        equipment.equip(&EquipSlot::Chest, armor_inst);
        let stats = equipment.combined_stats(&lib, &ench_lib, 1);
        assert!(stats.contains_key("Attack"));
        assert!(stats.contains_key("Defense"));
    }

    #[test]
    fn test_item_comparison() {
        let lib = ItemLibrary::with_defaults();
        let sword = lib.find_by_id(1).unwrap();
        let staff = lib.find_by_id(3).unwrap();
        let comp = compare_items(sword, staff, 1);
        assert!(comp.item_a == 1);
        assert!(comp.item_b == 3);
    }

    #[test]
    fn test_tier_table() {
        let table = TierTable::default_rpg();
        let tier1 = table.tier_for_level(5);
        assert!(tier1.is_some());
        assert_eq!(tier1.unwrap().tier, 1);
        let tier6 = table.tier_for_level(80);
        assert!(tier6.is_some());
        assert_eq!(tier6.unwrap().tier, 6);
        let scaled = table.scale_stat(10.0, 80);
        assert!(scaled > 10.0);
    }

    #[test]
    fn test_proc_effect() {
        let proc = ProcEffect::new("Fire Burst", ProcTrigger::OnHit, 0.3, ItemEffect::Damage { damage_type: "Fire".to_string(), amount: 20.0 });
        // With very many rolls, some should proc
        let mut any_proc = false;
        for i in 0..1000 {
            if proc.try_proc(i * 1234567) { any_proc = true; break; }
        }
        assert!(any_proc);
    }

    #[test]
    fn test_economy_simulator() {
        let lib = ItemLibrary::with_defaults();
        let mut economy = EconomySimulator::new(&lib, 1.3);
        let original_price = economy.get_price(1).map(|p| p.buy_price).unwrap_or(0);
        economy.simulate_tick(42);
        economy.simulate_tick(43);
        // prices should still be valid after ticks
        let new_price = economy.get_price(1);
        assert!(new_price.is_some());
    }

    #[test]
    fn test_quest_tracker() {
        let mut tracker = QuestItemTracker::new();
        tracker.register(1, "main_quest");
        tracker.register(2, "main_quest");
        assert!(!tracker.quest_complete("main_quest"));
        tracker.mark_obtained(1);
        assert!(!tracker.quest_complete("main_quest"));
        tracker.mark_obtained(2);
        assert!(tracker.quest_complete("main_quest"));
    }

    #[test]
    fn test_material_grade() {
        assert!(MaterialGrade::Masterwork.stat_multiplier() > MaterialGrade::Standard.stat_multiplier());
        assert!(MaterialGrade::Legendary.value_multiplier() > MaterialGrade::Masterwork.value_multiplier());
    }

    #[test]
    fn test_loot_farming_simulation() {
        let lib = ItemLibrary::with_defaults();
        let mut table = LootTable::new("test");
        table.entries.push(LootEntry { item_id: 1, weight: 5.0, min_count: 1, max_count: 1, condition: None });
        table.entries.push(LootEntry { item_id: 5, weight: 10.0, min_count: 10, max_count: 30, condition: None });
        let sim = simulate_loot_farming(&table, &lib, 100, 42);
        assert!(sim.total_rolls == 100);
        assert!(sim.unique_items > 0);
    }

    #[test]
    fn test_try_upgrade_rarity() {
        let mut item = Item::new(1);
        item.rarity = Rarity::Common;
        // force upgrade with 100% chance
        let upgraded = try_upgrade_rarity(&mut item, 1.0, 42);
        assert!(upgraded);
        assert_eq!(item.rarity, Rarity::Uncommon);
    }

    #[test]
    fn test_export_library_summary() {
        let lib = ItemLibrary::with_defaults();
        let summary = export_library_summary(&lib);
        assert!(summary.contains("Item Library"));
        assert!(summary.contains("Iron Sword"));
    }

    #[test]
    fn test_generate_random_item_by_tier() {
        let table = TierTable::default_rpg();
        for tier in &table.tiers {
            let level = (tier.level_range.0 + tier.level_range.1) / 2;
            let item = generate_random_item(99, Rarity::Rare, Some(ItemCategory::Weapon), level, level as u64 * 137);
            assert_eq!(item.level_req, level);
            let scaled_val = table.scale_stat(10.0, level);
            assert!(scaled_val >= 10.0);
        }
    }
}

// ============================================================
// BUFF / DEBUFF STATUS EFFECT SYSTEM
// ============================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StatusEffectType {
    Buff,
    Debuff,
    DoT,  // damage over time
    HoT,  // heal over time
    CC,   // crowd control
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusEffect {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub effect_type: StatusEffectType,
    pub icon_glyph: char,
    pub color: Color32,
    pub stat_modifiers: Vec<(String, f32, bool)>, // (stat_name, value, is_percent)
    pub tick_damage: f32,
    pub tick_heal: f32,
    pub tick_interval: f32,
    pub duration: f32,
    pub max_stacks: u32,
    pub is_dispellable: bool,
    pub is_unique: bool,
}

impl StatusEffect {
    pub fn new(id: u32, name: &str, effect_type: StatusEffectType) -> Self {
        Self {
            id, name: name.to_string(),
            description: String::new(),
            effect_type,
            icon_glyph: '!',
            color: Color32::from_rgb(200, 100, 50),
            stat_modifiers: Vec::new(),
            tick_damage: 0.0, tick_heal: 0.0,
            tick_interval: 1.0,
            duration: 10.0,
            max_stacks: 1,
            is_dispellable: true,
            is_unique: false,
        }
    }
}

pub struct StatusEffectLibrary {
    pub effects: Vec<StatusEffect>,
}

impl StatusEffectLibrary {
    pub fn new() -> Self {
        let mut lib = StatusEffectLibrary { effects: Vec::new() };

        let mut poison = StatusEffect::new(1, "Poison", StatusEffectType::DoT);
        poison.description = "Deals damage over time".to_string();
        poison.icon_glyph = 'P';
        poison.color = Color32::from_rgb(80, 200, 80);
        poison.tick_damage = 5.0;
        poison.tick_interval = 1.0;
        poison.duration = 10.0;
        poison.max_stacks = 3;
        lib.effects.push(poison);

        let mut burn = StatusEffect::new(2, "Burning", StatusEffectType::DoT);
        burn.description = "Fire damage over time".to_string();
        burn.icon_glyph = 'B';
        burn.color = Color32::from_rgb(255, 100, 30);
        burn.tick_damage = 8.0;
        burn.tick_interval = 0.5;
        burn.duration = 5.0;
        lib.effects.push(burn);

        let mut regen = StatusEffect::new(3, "Regeneration", StatusEffectType::HoT);
        regen.description = "Heals HP over time".to_string();
        regen.icon_glyph = 'R';
        regen.color = Color32::from_rgb(50, 200, 50);
        regen.tick_heal = 10.0;
        regen.tick_interval = 1.0;
        regen.duration = 20.0;
        lib.effects.push(regen);

        let mut strength_buff = StatusEffect::new(4, "Strength Up", StatusEffectType::Buff);
        strength_buff.description = "Increases attack power".to_string();
        strength_buff.icon_glyph = 'S';
        strength_buff.color = Color32::from_rgb(200, 50, 50);
        strength_buff.stat_modifiers = vec![("Attack".to_string(), 20.0, false)];
        strength_buff.duration = 30.0;
        lib.effects.push(strength_buff);

        let mut slow = StatusEffect::new(5, "Slow", StatusEffectType::CC);
        slow.description = "Reduces movement speed".to_string();
        slow.icon_glyph = 'S';
        slow.color = Color32::from_rgb(100, 100, 220);
        slow.stat_modifiers = vec![("Move Speed".to_string(), -0.3, true)];
        slow.duration = 5.0;
        lib.effects.push(slow);

        let mut freeze = StatusEffect::new(6, "Frozen", StatusEffectType::CC);
        freeze.description = "Cannot move or act".to_string();
        freeze.icon_glyph = 'F';
        freeze.color = Color32::from_rgb(150, 200, 255);
        freeze.stat_modifiers = vec![("Move Speed".to_string(), -1.0, true)];
        freeze.duration = 2.0;
        freeze.is_dispellable = false;
        lib.effects.push(freeze);

        lib
    }

    pub fn find(&self, id: u32) -> Option<&StatusEffect> {
        self.effects.iter().find(|e| e.id == id)
    }
}

// ============================================================
// SKILL TREE INTEGRATION
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillNode {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub icon_glyph: char,
    pub color: Color32,
    pub max_rank: u32,
    pub required_nodes: Vec<u32>,
    pub effects_per_rank: Vec<ItemEffect>,
    pub stat_bonuses_per_rank: Vec<ItemStat>,
    pub cost_per_rank: Vec<u32>,
    pub position: [f32; 2], // position in tree UI
}

impl SkillNode {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id, name: name.to_string(),
            description: String::new(),
            icon_glyph: '+',
            color: Color32::from_rgb(100, 150, 220),
            max_rank: 5,
            required_nodes: Vec::new(),
            effects_per_rank: Vec::new(),
            stat_bonuses_per_rank: Vec::new(),
            cost_per_rank: vec![1, 2, 3, 4, 5],
            position: [0.0, 0.0],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillTree {
    pub name: String,
    pub nodes: Vec<SkillNode>,
    pub tree_type: String,
}

impl SkillTree {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), nodes: Vec::new(), tree_type: "generic".to_string() }
    }

    pub fn find_node(&self, id: u32) -> Option<&SkillNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn can_learn(&self, node_id: u32, learned: &HashMap<u32, u32>) -> (bool, Vec<String>) {
        let mut reasons = Vec::new();
        if let Some(node) = self.find_node(node_id) {
            let current_rank = learned.get(&node_id).copied().unwrap_or(0);
            if current_rank >= node.max_rank {
                reasons.push("Already at max rank.".to_string());
            }
            for &req_id in &node.required_nodes {
                if learned.get(&req_id).copied().unwrap_or(0) == 0 {
                    if let Some(req_node) = self.find_node(req_id) {
                        reasons.push(format!("Requires: {}", req_node.name));
                    }
                }
            }
        } else {
            reasons.push("Skill not found.".to_string());
        }
        (reasons.is_empty(), reasons)
    }
}

// ============================================================
// INVENTORY EDITOR: STATUS EFFECTS PANEL
// ============================================================

pub fn show_status_effects_panel(ui: &mut egui::Ui, effect_lib: &StatusEffectLibrary) {
    ui.heading("Status Effects");
    egui::Grid::new("status_effects_grid")
        .num_columns(5)
        .striped(true)
        .show(ui, |ui| {
            ui.label("Name");
            ui.label("Type");
            ui.label("Duration");
            ui.label("Tick");
            ui.label("Stacks");
            ui.end_row();
            for effect in &effect_lib.effects {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 1.0, effect.color);
                    ui.label(RichText::new(&effect.name).color(effect.color));
                });
                let type_str = match effect.effect_type {
                    StatusEffectType::Buff => "Buff",
                    StatusEffectType::Debuff => "Debuff",
                    StatusEffectType::DoT => "DoT",
                    StatusEffectType::HoT => "HoT",
                    StatusEffectType::CC => "CC",
                };
                ui.label(type_str);
                ui.label(format!("{:.1}s", effect.duration));
                if effect.tick_damage > 0.0 { ui.label(format!("-{}/{}s", effect.tick_damage, effect.tick_interval)); }
                else if effect.tick_heal > 0.0 { ui.label(format!("+{}/{}s", effect.tick_heal, effect.tick_interval)); }
                else { ui.label("—"); }
                ui.label(format!("x{}", effect.max_stacks));
                ui.end_row();
            }
        });
}

// ============================================================
// INVENTORY EDITOR: FULL ITEM SEARCH SYSTEM
// ============================================================

pub struct ItemSearchQuery {
    pub text: String,
    pub category: Option<ItemCategory>,
    pub rarity: Option<Rarity>,
    pub min_level: u32,
    pub max_level: u32,
    pub min_value: u32,
    pub max_value: u32,
    pub must_have_tags: Vec<String>,
    pub must_be_craftable: Option<bool>,
    pub equip_slot: Option<EquipSlot>,
    pub has_stat: Option<String>,
}

impl ItemSearchQuery {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            category: None,
            rarity: None,
            min_level: 0,
            max_level: 999,
            min_value: 0,
            max_value: u32::MAX,
            must_have_tags: Vec::new(),
            must_be_craftable: None,
            equip_slot: None,
            has_stat: None,
        }
    }

    pub fn matches(&self, item: &Item) -> bool {
        if !self.text.is_empty() {
            let t = self.text.to_lowercase();
            if !item.name.to_lowercase().contains(&t)
                && !item.description.to_lowercase().contains(&t)
                && !item.tags.iter().any(|tag| tag.to_lowercase().contains(&t)) {
                return false;
            }
        }
        if let Some(cat) = &self.category {
            if &item.category != cat { return false; }
        }
        if let Some(rar) = &self.rarity {
            if &item.rarity != rar { return false; }
        }
        if item.level_req < self.min_level || item.level_req > self.max_level { return false; }
        if item.base_value < self.min_value || item.base_value > self.max_value { return false; }
        for tag in &self.must_have_tags {
            if !item.tags.contains(tag) { return false; }
        }
        if let Some(craftable) = self.must_be_craftable {
            if item.craftable != craftable { return false; }
        }
        if let Some(slot) = &self.equip_slot {
            if item.equip_slot.as_ref() != Some(slot) { return false; }
        }
        if let Some(stat_name) = &self.has_stat {
            if !item.stats.iter().any(|s| &s.name == stat_name) { return false; }
        }
        true
    }
}

pub fn advanced_search(library: &ItemLibrary, query: &ItemSearchQuery) -> Vec<usize> {
    library.items.iter().enumerate()
        .filter_map(|(i, item)| if query.matches(item) { Some(i) } else { None })
        .collect()
}

// ============================================================
// INVENTORY EDITOR: SHOW ADVANCED SEARCH PANEL
// ============================================================

pub fn show_advanced_search_panel(ui: &mut egui::Ui, query: &mut ItemSearchQuery) {
    ui.heading("Advanced Search");
    ui.horizontal(|ui| {
        ui.label("Text:");
        ui.text_edit_singleline(&mut query.text);
    });
    ui.horizontal(|ui| {
        ui.label("Category:");
        egui::ComboBox::from_id_source("search_cat")
            .selected_text(query.category.as_ref().map(|c| c.name()).unwrap_or("Any"))
            .show_ui(ui, |ui| {
                if ui.selectable_label(query.category.is_none(), "Any").clicked() { query.category = None; }
                for cat in ItemCategory::all() {
                    let sel = query.category.as_ref() == Some(cat);
                    if ui.selectable_label(sel, cat.name()).clicked() { query.category = Some(cat.clone()); }
                }
            });
        ui.label("Rarity:");
        egui::ComboBox::from_id_source("search_rar")
            .selected_text(query.rarity.as_ref().map(|r| r.name()).unwrap_or("Any"))
            .show_ui(ui, |ui| {
                if ui.selectable_label(query.rarity.is_none(), "Any").clicked() { query.rarity = None; }
                for rar in Rarity::all() {
                    let sel = query.rarity.as_ref() == Some(rar);
                    if ui.selectable_label(sel, RichText::new(rar.name()).color(rar.color())).clicked() {
                        query.rarity = Some(rar.clone());
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Level:");
        ui.add(egui::DragValue::new(&mut query.min_level).range(0..=999).speed(1.0));
        ui.label("—");
        ui.add(egui::DragValue::new(&mut query.max_level).range(0..=999).speed(1.0));
    });
    ui.horizontal(|ui| {
        ui.label("Value:");
        ui.add(egui::DragValue::new(&mut query.min_value).range(0..=9999999).speed(1.0));
        ui.label("—");
        ui.add(egui::DragValue::new(&mut query.max_value).range(0..=9999999).speed(1.0));
    });
    ui.horizontal(|ui| {
        ui.label("Equip Slot:");
        egui::ComboBox::from_id_source("search_slot")
            .selected_text(query.equip_slot.as_ref().map(|s| s.name()).unwrap_or("Any"))
            .show_ui(ui, |ui| {
                if ui.selectable_label(query.equip_slot.is_none(), "Any").clicked() { query.equip_slot = None; }
                for slot in EquipSlot::all() {
                    let sel = query.equip_slot.as_ref() == Some(slot);
                    if ui.selectable_label(sel, slot.name()).clicked() { query.equip_slot = Some(slot.clone()); }
                }
            });
    });
    ui.horizontal(|ui| {
        let mut craftable_str = match query.must_be_craftable {
            None => "Any",
            Some(true) => "Craftable",
            Some(false) => "Not Craftable",
        };
        ui.label("Craftable:");
        egui::ComboBox::from_id_source("search_craft")
            .selected_text(craftable_str)
            .show_ui(ui, |ui| {
                if ui.selectable_label(query.must_be_craftable.is_none(), "Any").clicked() { query.must_be_craftable = None; }
                if ui.selectable_label(query.must_be_craftable == Some(true), "Craftable").clicked() { query.must_be_craftable = Some(true); }
                if ui.selectable_label(query.must_be_craftable == Some(false), "Not Craftable").clicked() { query.must_be_craftable = Some(false); }
            });
    });
}

// ============================================================
// ITEM BALANCE ANALYZER
// ============================================================

pub struct ItemBalanceReport {
    pub item_id: u32,
    pub item_name: String,
    pub rarity: Rarity,
    pub level_req: u32,
    pub dps: Option<f32>,
    pub eff_hp: Option<f32>,
    pub value_efficiency: f32, // stats per gold
    pub flags: Vec<String>,    // balance warnings
}

impl ItemBalanceReport {
    pub fn analyze(item: &Item, tier_table: &TierTable) -> Self {
        let mut flags = Vec::new();
        let level = item.level_req;

        // DPS estimate (for weapons)
        let dps = if item.category == ItemCategory::Weapon {
            let atk = item.stats.iter().find(|s| s.name.to_lowercase().contains("attack"))
                .map(|s| s.value_at_level(level));
            atk.map(|a| a * 1.0) // simplified: DPS = attack (assume 1 attack/s)
        } else { None };

        // Effective HP (for armor)
        let eff_hp = if item.category == ItemCategory::Armor {
            let def = item.stats.iter().find(|s| s.name.to_lowercase().contains("defense"))
                .map(|s| s.value_at_level(level));
            def.map(|d| 100.0 + d * 2.0) // simplified EHP formula
        } else { None };

        // Value efficiency: total stat points / gold
        let total_stat_points: f32 = item.stats.iter().map(|s| s.value_at_level(level).abs()).sum();
        let value_efficiency = if item.base_value > 0 {
            total_stat_points / item.base_value as f32
        } else { 0.0 };

        // Compare against tier expectations
        if let Some(tier) = tier_table.tier_for_level(level) {
            let expected_stat = 10.0 * tier.stat_multiplier;
            if total_stat_points > expected_stat * 2.0 {
                flags.push(format!("OVERPOWERED: stats {:.1} >> expected {:.1}", total_stat_points, expected_stat));
            } else if total_stat_points < expected_stat * 0.4 && item.stats.len() > 0 {
                flags.push(format!("UNDERPOWERED: stats {:.1} << expected {:.1}", total_stat_points, expected_stat));
            }
        }

        if item.drop_chance > 0.5 && matches!(item.rarity, Rarity::Legendary | Rarity::Mythic) {
            flags.push("HIGH DROP CHANCE for rarity".to_string());
        }
        if item.base_value == 0 && !item.quest_item {
            flags.push("Zero value (non-quest item)".to_string());
        }

        ItemBalanceReport {
            item_id: item.id,
            item_name: item.name.clone(),
            rarity: item.rarity.clone(),
            level_req: level,
            dps, eff_hp, value_efficiency, flags,
        }
    }
}

pub fn analyze_library_balance(library: &ItemLibrary, tier_table: &TierTable) -> Vec<ItemBalanceReport> {
    library.items.iter().map(|item| ItemBalanceReport::analyze(item, tier_table)).collect()
}

pub fn show_balance_report(ui: &mut egui::Ui, reports: &[ItemBalanceReport]) {
    ui.heading("Balance Analysis");
    let flagged: Vec<&ItemBalanceReport> = reports.iter().filter(|r| !r.flags.is_empty()).collect();
    ui.label(format!("{} items analyzed, {} with warnings", reports.len(), flagged.len()));
    ui.separator();

    egui::ScrollArea::vertical().id_source("balance_scroll").show(ui, |ui| {
        for report in reports {
            if report.flags.is_empty() { continue; }
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&report.item_name).color(report.rarity.color()).strong());
                    ui.label(format!("lv.{}", report.level_req));
                    if let Some(dps) = report.dps { ui.label(format!("DPS:{:.1}", dps)); }
                    if let Some(eff_hp) = report.eff_hp { ui.label(format!("EHP:{:.1}", eff_hp)); }
                    ui.label(format!("VE:{:.3}", report.value_efficiency));
                });
                for flag in &report.flags {
                    ui.label(RichText::new(format!("  ⚠ {}", flag)).color(Color32::from_rgb(255, 180, 50)).small());
                }
            });
        }
    });
}

// ============================================================
// LOOT PROBABILITY CALCULATOR
// ============================================================

pub struct LootProbabilityCalc {
    pub table: LootTable,
}

impl LootProbabilityCalc {
    pub fn new(table: LootTable) -> Self {
        Self { table }
    }

    /// Probability of getting at least 1 of item_id in N rolls
    pub fn prob_at_least_one(&self, item_id: u32, num_rolls: u32) -> f32 {
        let rates = compute_drop_rates(&self.table);
        let p_single = rates.iter().find(|(id, _)| *id == item_id).map(|(_, r)| *r).unwrap_or(0.0);
        if p_single <= 0.0 { return 0.0; }
        1.0 - (1.0 - p_single).powi(num_rolls as i32)
    }

    /// Expected rolls to get item_id
    pub fn expected_rolls_for(&self, item_id: u32) -> f32 {
        let rates = compute_drop_rates(&self.table);
        let p_single = rates.iter().find(|(id, _)| *id == item_id).map(|(_, r)| *r).unwrap_or(0.0);
        if p_single <= 0.0 { return f32::MAX; }
        1.0 / p_single
    }

    /// Probability of getting exactly N of item_id in K rolls (binomial)
    pub fn prob_exactly(&self, item_id: u32, n: u32, k: u32) -> f32 {
        let rates = compute_drop_rates(&self.table);
        let p = rates.iter().find(|(id, _)| *id == item_id).map(|(_, r)| *r).unwrap_or(0.0);
        if p <= 0.0 && n > 0 { return 0.0; }
        let binom_coeff = binomial_coefficient(k, n) as f32;
        binom_coeff * p.powi(n as i32) * (1.0 - p).powi((k - n) as i32)
    }
}

fn binomial_coefficient(n: u32, k: u32) -> u64 {
    if k > n { return 0; }
    if k == 0 || k == n { return 1; }
    let k = k.min(n - k);
    let mut result = 1u64;
    for i in 0..k {
        result = result * (n - i) as u64 / (i + 1) as u64;
    }
    result
}

// ============================================================
// ITEM GENERATOR TEMPLATES
// ============================================================

pub struct ItemTemplate {
    pub template_name: String,
    pub category: ItemCategory,
    pub equip_slot: Option<EquipSlot>,
    pub stat_names: Vec<String>,
    pub base_stats: Vec<f32>,
    pub stat_scaling: Vec<StatScaling>,
    pub effects_template: Vec<ItemEffect>,
    pub name_prefixes: Vec<String>,
    pub name_cores: Vec<String>,
    pub name_suffixes: Vec<String>,
}

impl ItemTemplate {
    pub fn sword_template() -> Self {
        Self {
            template_name: "Sword".to_string(),
            category: ItemCategory::Weapon,
            equip_slot: Some(EquipSlot::MainHand),
            stat_names: vec!["Attack".to_string(), "Critical Chance".to_string(), "Attack Speed".to_string()],
            base_stats: vec![15.0, 0.05, 1.0],
            stat_scaling: vec![
                StatScaling::Linear { per_level: 2.0 },
                StatScaling::Linear { per_level: 0.005 },
                StatScaling::None,
            ],
            effects_template: Vec::new(),
            name_prefixes: vec!["Iron".to_string(), "Steel".to_string(), "Silver".to_string(), "Shadow".to_string(), "Flame".to_string()],
            name_cores: vec!["Sword".to_string(), "Blade".to_string(), "Saber".to_string(), "Rapier".to_string()],
            name_suffixes: vec!["of Striking".to_string(), "of the Warrior".to_string(), "".to_string(), "the Vile".to_string()],
        }
    }

    pub fn staff_template() -> Self {
        Self {
            template_name: "Staff".to_string(),
            category: ItemCategory::Weapon,
            equip_slot: Some(EquipSlot::MainHand),
            stat_names: vec!["Magic Attack".to_string(), "Spell Power".to_string(), "Mana Regen".to_string()],
            base_stats: vec![20.0, 10.0, 2.0],
            stat_scaling: vec![
                StatScaling::Linear { per_level: 3.5 },
                StatScaling::Linear { per_level: 1.5 },
                StatScaling::None,
            ],
            effects_template: Vec::new(),
            name_prefixes: vec!["Arcane".to_string(), "Mystic".to_string(), "Elder".to_string(), "Void".to_string()],
            name_cores: vec!["Staff".to_string(), "Wand".to_string(), "Rod".to_string(), "Scepter".to_string()],
            name_suffixes: vec!["of Power".to_string(), "of Wisdom".to_string(), "".to_string(), "of Eternity".to_string()],
        }
    }

    pub fn generate(&self, id: u32, rarity: Rarity, level: u32, seed: u64) -> Item {
        let mut rng = seed.wrapping_add(id as u64 * 31337);
        let rng_f = |r: &mut u64| -> f32 {
            *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
            (*r & 0xFFFFFF) as f32 / 16777216.0
        };
        let rng_i = |r: &mut u64, max: usize| -> usize {
            *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
            (*r as usize) % max.max(1)
        };

        let prefix_idx = rng_i(&mut rng, self.name_prefixes.len());
        let core_idx = rng_i(&mut rng, self.name_cores.len());
        let suffix_idx = rng_i(&mut rng, self.name_suffixes.len());
        let suffix = &self.name_suffixes[suffix_idx];
        let name = if suffix.is_empty() {
            format!("{} {}", self.name_prefixes[prefix_idx], self.name_cores[core_idx])
        } else {
            format!("{} {} {}", self.name_prefixes[prefix_idx], self.name_cores[core_idx], suffix)
        };

        let rarity_mult = match rarity {
            Rarity::Common => 1.0, Rarity::Uncommon => 1.3, Rarity::Rare => 1.7,
            Rarity::Epic => 2.2, Rarity::Legendary => 3.0, Rarity::Mythic => 4.0, Rarity::Unique => 5.5,
        };

        let mut item = Item::new(id);
        item.name = name;
        item.category = self.category.clone();
        item.rarity = rarity;
        item.equip_slot = self.equip_slot.clone();
        item.level_req = level;

        for (i, stat_name) in self.stat_names.iter().enumerate() {
            let base = self.base_stats.get(i).copied().unwrap_or(10.0);
            let variation = 0.85 + rng_f(&mut rng) * 0.3;
            item.stats.push(ItemStat {
                name: stat_name.clone(),
                base_value: base * rarity_mult * variation,
                scaling: self.stat_scaling.get(i).cloned().unwrap_or(StatScaling::None),
                display_format: if stat_name.contains("Chance") || stat_name.contains("Rate") {
                    StatFormat::Percent
                } else { StatFormat::Integer },
            });
        }

        item.base_value = ((level as f32 * 10.0 + 20.0) * rarity_mult) as u32;
        item.weight = 0.5 + rng_f(&mut rng) * 3.0;
        item.drop_chance = 0.1 / rarity_mult;

        item
    }
}

// ============================================================
// MORE TESTS
// ============================================================

#[cfg(test)]
mod inv_extra_tests {
    use super::*;

    #[test]
    fn test_status_effect_library() {
        let lib = StatusEffectLibrary::new();
        assert!(!lib.effects.is_empty());
        let poison = lib.find(1);
        assert!(poison.is_some());
        assert!(poison.unwrap().tick_damage > 0.0);
    }

    #[test]
    fn test_skill_tree_can_learn() {
        let mut tree = SkillTree::new("Test Tree");
        let mut node_a = SkillNode::new(1, "Power Strike");
        let mut node_b = SkillNode::new(2, "Heavy Blow");
        node_b.required_nodes = vec![1];
        tree.nodes.push(node_a);
        tree.nodes.push(node_b);

        let mut learned = HashMap::new();
        let (can, reasons) = tree.can_learn(2, &learned);
        assert!(!can); // requires node 1
        assert!(!reasons.is_empty());

        learned.insert(1, 1);
        let (can2, _) = tree.can_learn(2, &learned);
        assert!(can2);
    }

    #[test]
    fn test_item_search_query() {
        let lib = ItemLibrary::with_defaults();
        let mut query = ItemSearchQuery::new();
        query.text = "sword".to_string();
        let results = advanced_search(&lib, &query);
        assert!(!results.is_empty());
        for &idx in &results {
            assert!(lib.items[idx].name.to_lowercase().contains("sword") ||
                lib.items[idx].description.to_lowercase().contains("sword"));
        }
    }

    #[test]
    fn test_item_search_by_slot() {
        let lib = ItemLibrary::with_defaults();
        let mut query = ItemSearchQuery::new();
        query.equip_slot = Some(EquipSlot::MainHand);
        let results = advanced_search(&lib, &query);
        assert!(!results.is_empty());
        for &idx in &results {
            assert_eq!(lib.items[idx].equip_slot, Some(EquipSlot::MainHand));
        }
    }

    #[test]
    fn test_balance_report() {
        let lib = ItemLibrary::with_defaults();
        let table = TierTable::default_rpg();
        let reports = analyze_library_balance(&lib, &table);
        assert_eq!(reports.len(), lib.items.len());
    }

    #[test]
    fn test_loot_probability_calc() {
        let mut table = LootTable::new("test");
        table.entries.push(LootEntry { item_id: 1, weight: 1.0, min_count: 1, max_count: 1, condition: None });
        table.entries.push(LootEntry { item_id: 2, weight: 9.0, min_count: 1, max_count: 1, condition: None });
        let calc = LootProbabilityCalc::new(table);
        let p = calc.prob_at_least_one(1, 10);
        assert!(p > 0.0 && p <= 1.0);
        // 10% drop rate, 10 rolls: P(at least 1) = 1 - 0.9^10 ≈ 0.651
        assert!(p > 0.5 && p < 0.8);
        let exp = calc.expected_rolls_for(1);
        assert!((exp - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_binomial_coefficient() {
        assert_eq!(binomial_coefficient(5, 2), 10);
        assert_eq!(binomial_coefficient(10, 3), 120);
        assert_eq!(binomial_coefficient(0, 0), 1);
    }

    #[test]
    fn test_item_template_generation() {
        let template = ItemTemplate::sword_template();
        let item = template.generate(100, Rarity::Rare, 15, 42);
        assert_eq!(item.category, ItemCategory::Weapon);
        assert_eq!(item.level_req, 15);
        assert!(!item.stats.is_empty());
        assert!(item.stats.iter().any(|s| s.name == "Attack"));
    }

    #[test]
    fn test_staff_template() {
        let template = ItemTemplate::staff_template();
        let item = template.generate(200, Rarity::Epic, 30, 99);
        assert!(item.stats.iter().any(|s| s.name == "Magic Attack"));
        assert!(item.base_value > 0);
    }

    #[test]
    fn test_item_balance_overpowered_detection() {
        let mut item = Item::new(99);
        item.rarity = Rarity::Common;
        item.level_req = 1;
        item.category = ItemCategory::Weapon;
        // Absurdly high attack for level 1
        item.stats.push(ItemStat {
            name: "Attack".to_string(), base_value: 10000.0,
            scaling: StatScaling::None, display_format: StatFormat::Integer,
        });
        let table = TierTable::default_rpg();
        let report = ItemBalanceReport::analyze(&item, &table);
        assert!(!report.flags.is_empty());
        assert!(report.flags.iter().any(|f| f.contains("OVERPOWERED")));
    }

    #[test]
    fn test_equipment_unequip() {
        let lib = ItemLibrary::with_defaults();
        let ench_lib = EnchantmentLibrary::new();
        let mut equip = Equipment::new();
        let inst = ItemInstance::new(1, 1);
        equip.equip(&EquipSlot::MainHand, inst);
        let removed = equip.unequip(&EquipSlot::MainHand);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().base_item_id, 1);
        assert!(equip.get(&EquipSlot::MainHand).is_none());
    }

    #[test]
    fn test_item_instance_display_name() {
        let item = Item::sword(1);
        let mut instance = ItemInstance::new(1, 1);
        assert_eq!(instance.display_name(&item), "Iron Sword");
        instance.custom_name = Some("Sword of Legends".to_string());
        assert_eq!(instance.display_name(&item), "Sword of Legends");
    }

    #[test]
    fn test_sort_by_value_density() {
        let mut items = vec![
            { let mut i = Item::new(1); i.base_value = 100; i.weight = 10.0; i },
            { let mut i = Item::new(2); i.base_value = 100; i.weight = 1.0; i },
            { let mut i = Item::new(3); i.base_value = 50; i.weight = 0.5; i },
        ];
        sort_by_value_density(&mut items);
        // highest ratio first: item2 = 100/1 = 100, item3 = 50/0.5 = 100, item1 = 100/10 = 10
        assert!(items.last().unwrap().id == 1);
    }
}

// ============================================================
// CURRENCY & ECONOMY TYPES
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrencyType {
    pub id: u32,
    pub name: String,
    pub symbol: char,
    pub color: Color32,
    pub base_exchange_rate: f32, // relative to gold
    pub description: String,
}

impl CurrencyType {
    pub fn gold() -> Self {
        Self { id: 1, name: "Gold".to_string(), symbol: 'G', color: Color32::from_rgb(220, 180, 30), base_exchange_rate: 1.0, description: "Standard currency.".to_string() }
    }
    pub fn silver() -> Self {
        Self { id: 2, name: "Silver".to_string(), symbol: 'S', color: Color32::from_rgb(190, 190, 210), base_exchange_rate: 0.01, description: "Small currency.".to_string() }
    }
    pub fn platinum() -> Self {
        Self { id: 3, name: "Platinum".to_string(), symbol: 'P', color: Color32::from_rgb(180, 220, 240), base_exchange_rate: 100.0, description: "High-value currency.".to_string() }
    }
    pub fn soul_coin() -> Self {
        Self { id: 4, name: "Soul Coin".to_string(), symbol: '*', color: Color32::from_rgb(150, 100, 220), base_exchange_rate: 500.0, description: "Rare currency from defeated monsters.".to_string() }
    }
}

pub struct CurrencyWallet {
    pub currencies: HashMap<u32, u64>,
}

impl CurrencyWallet {
    pub fn new() -> Self {
        Self { currencies: HashMap::new() }
    }

    pub fn add(&mut self, currency_id: u32, amount: u64) {
        *self.currencies.entry(currency_id).or_insert(0) += amount;
    }

    pub fn spend(&mut self, currency_id: u32, amount: u64) -> Result<(), String> {
        let balance = self.currencies.get(&currency_id).copied().unwrap_or(0);
        if balance < amount {
            Err(format!("Insufficient funds: have {}, need {}", balance, amount))
        } else {
            *self.currencies.entry(currency_id).or_insert(0) -= amount;
            Ok(())
        }
    }

    pub fn balance(&self, currency_id: u32) -> u64 {
        self.currencies.get(&currency_id).copied().unwrap_or(0)
    }

    pub fn total_in_gold(&self, currency_types: &[CurrencyType]) -> f32 {
        self.currencies.iter().map(|(&id, &amount)| {
            let rate = currency_types.iter().find(|c| c.id == id)
                .map(|c| c.base_exchange_rate).unwrap_or(1.0);
            amount as f32 * rate
        }).sum()
    }
}

// ============================================================
// FULL CRAFTING SYSTEM WITH SKILL PROGRESSION
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftingSkill {
    pub skill_name: String,
    pub current_xp: u32,
    pub level: u32,
}

impl CraftingSkill {
    pub fn new(skill_name: &str) -> Self {
        Self { skill_name: skill_name.to_string(), current_xp: 0, level: 1 }
    }

    pub fn xp_for_next_level(&self) -> u32 {
        (self.level as f32 * 100.0 * (1.0 + self.level as f32 * 0.1)) as u32
    }

    pub fn add_xp(&mut self, xp: u32) {
        self.current_xp += xp;
        while self.current_xp >= self.xp_for_next_level() && self.level < 100 {
            self.current_xp -= self.xp_for_next_level();
            self.level += 1;
        }
    }

    pub fn can_craft(&self, recipe: &Recipe) -> bool {
        if let Some((_, req_level)) = &recipe.skill_req {
            self.level >= *req_level
        } else { true }
    }
}

#[derive(Clone, Debug)]
pub struct CraftingSession {
    pub recipe: Recipe,
    pub result_item_id: u32,
    pub progress: f32,  // 0.0 - 1.0
    pub quality: f32,   // 0.0 - 1.0
    pub skill_snapshot: u32,
}

impl CraftingSession {
    pub fn new(recipe: Recipe, result_item_id: u32, skill_level: u32) -> Self {
        let quality_base = (skill_level as f32 / 100.0).clamp(0.1, 1.0);
        Self {
            recipe,
            result_item_id,
            progress: 0.0,
            quality: quality_base,
            skill_snapshot: skill_level,
        }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        self.progress += dt / self.recipe.time;
        self.progress >= 1.0
    }

    pub fn finish(&self) -> (u32, f32) {
        (self.result_item_id, self.quality.clamp(0.0, 1.0))
    }
}

pub fn show_crafting_session(ui: &mut egui::Ui, session: &CraftingSession) {
    ui.label(format!("Crafting Item #{} ...", session.result_item_id));
    let bar_w = ui.available_width();
    let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 16.0), egui::Sense::hover());
    ui.painter().rect_filled(bar_rect, 4.0, Color32::from_rgb(30, 30, 40));
    let fill = Rect::from_min_size(bar_rect.min, Vec2::new(bar_w * session.progress.clamp(0.0, 1.0), 16.0));
    let color = if session.quality > 0.8 { Color32::from_rgb(50, 200, 220) }
        else if session.quality > 0.5 { Color32::from_rgb(50, 180, 50) }
        else { Color32::from_rgb(180, 140, 50) };
    ui.painter().rect_filled(fill, 4.0, color);
    ui.label(format!("Quality: {:.0}%  Time: {:.1}/{:.1}s",
        session.quality * 100.0, session.progress * session.recipe.time, session.recipe.time));
}

// ============================================================
// ITEM TOOLTIP SYSTEM
// ============================================================

pub fn show_rich_item_tooltip(ui: &mut egui::Ui, item: &Item, level: u32) {
    let rarity_color = item.rarity.color();
    ui.set_max_width(250.0);

    // Header
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), egui::Sense::hover());
        ui.painter().rect_filled(r, 3.0, Color32::from_rgb(20, 20, 30));
        ui.painter().rect_stroke(r, 3.0, Stroke::new(2.0, rarity_color), egui::StrokeKind::Inside);
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, item.icon_glyph.to_string(), FontId::monospace(16.0), item.icon_color);
        ui.vertical(|ui| {
            ui.label(RichText::new(&item.name).color(rarity_color).strong());
            ui.label(RichText::new(format!("{} {}", item.rarity.name(), item.category.name())).small().color(rarity_color));
        });
    });

    if item.level_req > 1 {
        ui.label(RichText::new(format!("Item Level: {}", item.level_req)).small().color(Color32::from_rgb(200, 170, 80)));
    }

    if let Some(slot) = &item.equip_slot {
        ui.label(RichText::new(format!("Equip: {}", slot.name())).small().color(Color32::from_gray(160)));
    }

    if !item.stats.is_empty() {
        ui.separator();
        for stat in &item.stats {
            let val = stat.value_at_level(level);
            ui.horizontal(|ui| {
                ui.label(RichText::new(&stat.name).small().color(Color32::from_gray(180)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(stat.display_format.format_value(val)).small().color(Color32::from_rgb(120, 220, 120)));
                });
            });
        }
    }

    if !item.effects.is_empty() {
        ui.separator();
        for eff in &item.effects {
            ui.label(RichText::new(eff.description()).small().color(Color32::from_rgb(160, 120, 220)));
        }
    }

    if !item.requirements.is_empty() {
        ui.separator();
        for req in &item.requirements {
            ui.label(RichText::new(format!("Requires {} {}", req.stat, req.min_value)).small().color(Color32::from_rgb(220, 120, 80)));
        }
    }

    if !item.tags.is_empty() {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            for tag in &item.tags {
                ui.label(RichText::new(format!("[{}]", tag)).small().color(Color32::from_gray(120)));
            }
        });
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}g", item.base_value)).small().color(Color32::from_rgb(200, 160, 30)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{:.2}kg", item.weight)).small().color(Color32::from_gray(140)));
        });
    });

    if !item.lore.is_empty() {
        ui.separator();
        ui.label(RichText::new(format!("\"{}\"", &item.lore)).italics().small().color(Color32::from_gray(110)));
    }
}

// ============================================================
// INVENTORY EDITOR: ADDITIONAL STAT VISUALIZATION
// ============================================================

pub fn show_stat_radar_chart(ui: &mut egui::Ui, item: &Item, level: u32, max_vals: &HashMap<String, f32>) {
    let size = 120.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), egui::Sense::hover());
    let painter = ui.painter();
    let center = rect.center();
    let r = size * 0.45;
    let n_stats = item.stats.len().min(6);
    if n_stats < 3 { return; }

    painter.rect_filled(rect, 4.0, Color32::from_rgb(15, 15, 25));

    let angle_step = std::f32::consts::TAU / n_stats as f32;

    // Draw grid circles
    for ring in 1..=4 {
        let ring_r = r * ring as f32 / 4.0;
        let mut pts = Vec::new();
        for i in 0..n_stats {
            let a = i as f32 * angle_step - std::f32::consts::FRAC_PI_2;
            pts.push(Pos2::new(center.x + a.cos() * ring_r, center.y + a.sin() * ring_r));
        }
        pts.push(pts[0]);
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(0.5, Color32::from_rgba_premultiplied(100, 100, 120, 60)));
        }
    }

    // Draw stat spokes
    for i in 0..n_stats {
        let a = i as f32 * angle_step - std::f32::consts::FRAC_PI_2;
        painter.line_segment(
            [center, Pos2::new(center.x + a.cos() * r, center.y + a.sin() * r)],
            Stroke::new(0.5, Color32::from_rgba_premultiplied(120, 120, 140, 80)),
        );
    }

    // Draw filled polygon for item stats
    let rarity_color = item.rarity.color();
    let mut stat_pts = Vec::new();
    for i in 0..n_stats {
        let stat = &item.stats[i];
        let val = stat.value_at_level(level);
        let max = max_vals.get(&stat.name).copied().unwrap_or(val.max(1.0));
        let normalized = (val / max).clamp(0.0, 1.0);
        let a = i as f32 * angle_step - std::f32::consts::FRAC_PI_2;
        stat_pts.push(Pos2::new(center.x + a.cos() * r * normalized, center.y + a.sin() * r * normalized));
    }
    if stat_pts.len() >= 3 {
        let fill_color = Color32::from_rgba_premultiplied(rarity_color.r() / 3, rarity_color.g() / 3, rarity_color.b() / 3, 120);
        // Draw fill triangles from center
        for i in 0..stat_pts.len() {
            let next = (i + 1) % stat_pts.len();
            painter.add(Shape::convex_polygon(
                vec![center, stat_pts[i], stat_pts[next]],
                fill_color, Stroke::NONE,
            ));
        }
        // Draw outline
        let mut outline = stat_pts.clone();
        outline.push(outline[0]);
        for w in outline.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(1.5, rarity_color));
        }
    }

    // Stat labels
    for i in 0..n_stats {
        let a = i as f32 * angle_step - std::f32::consts::FRAC_PI_2;
        let label_r = r + 12.0;
        let lx = center.x + a.cos() * label_r;
        let ly = center.y + a.sin() * label_r;
        let stat = &item.stats[i];
        let short = if stat.name.len() > 4 { &stat.name[..4] } else { &stat.name };
        painter.text(Pos2::new(lx, ly), egui::Align2::CENTER_CENTER, short, FontId::proportional(7.0), Color32::from_gray(160));
    }
}

// ============================================================
// ITEM HISTORY/CHANGELOG TRACKING
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemChangeLog {
    pub item_id: u32,
    pub changes: Vec<ItemChange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemChange {
    pub timestamp: String,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub author: String,
}

impl ItemChangeLog {
    pub fn new(item_id: u32) -> Self {
        Self { item_id, changes: Vec::new() }
    }

    pub fn record(&mut self, field: &str, old: &str, new: &str, author: &str) {
        self.changes.push(ItemChange {
            timestamp: "2026-03-31".to_string(),
            field: field.to_string(),
            old_value: old.to_string(),
            new_value: new.to_string(),
            author: author.to_string(),
        });
    }

    pub fn recent(&self, n: usize) -> &[ItemChange] {
        let start = self.changes.len().saturating_sub(n);
        &self.changes[start..]
    }
}

pub fn show_item_changelog(ui: &mut egui::Ui, log: &ItemChangeLog) {
    if log.changes.is_empty() {
        ui.label("No changes recorded.");
        return;
    }
    for change in log.recent(20) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&change.timestamp).small().color(Color32::from_gray(100)));
            ui.label(RichText::new(format!("{}: {} → {}", change.field, change.old_value, change.new_value)).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(&change.author).small().color(Color32::from_gray(130)));
            });
        });
    }
}

// ============================================================
// COMPLETE INVENTORY SYSTEM TEST SUITE (INTEGRATION)
// ============================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// Full integration: create items, add to shop, simulate purchase, track in inventory
    #[test]
    fn test_full_purchase_flow() {
        let lib = ItemLibrary::with_defaults();
        let mut wallet = CurrencyWallet::new();
        wallet.add(1, 1000); // 1000 gold

        let shop = &lib.items;
        let sword = lib.find_by_id(1).unwrap();
        let price = 180u64;

        let result = wallet.spend(1, price);
        assert!(result.is_ok());
        assert_eq!(wallet.balance(1), 820);
    }

    #[test]
    fn test_full_crafting_flow() {
        let lib = ItemLibrary::with_defaults();
        let mut skill = CraftingSkill::new("Smithing");
        skill.add_xp(500);
        assert!(skill.level > 1);

        let mut recipe = Recipe::new();
        recipe.ingredients = vec![(5, 10)]; // 10 gold coins
        recipe.skill_req = Some(("Smithing".to_string(), 2));
        recipe.time = 5.0;
        assert!(skill.can_craft(&recipe));
    }

    #[test]
    fn test_enchant_and_compare() {
        let lib = ItemLibrary::with_defaults();
        let ench_lib = EnchantmentLibrary::new();

        let mut sword_a = ItemInstance::new(1, 1);
        let mut sword_b = ItemInstance::new(1, 2);
        sword_a.enchantments.push(AppliedEnchantment { enchantment_id: 1, level: 3 }); // sharpness 3

        let item = lib.find_by_id(1).unwrap();
        let stats_a = sword_a.effective_stats(item, &ench_lib, 1);
        let stats_b = sword_b.effective_stats(item, &ench_lib, 1);

        let atk_a = stats_a.iter().find(|(n, _)| n == "Attack").map(|(_, v)| *v).unwrap_or(0.0);
        let atk_b = stats_b.iter().find(|(n, _)| n == "Attack").map(|(_, v)| *v).unwrap_or(0.0);
        assert!(atk_a > atk_b);
    }

    #[test]
    fn test_loot_table_with_conditions() {
        let mut table = LootTable::new("Conditional Loot");
        table.entries.push(LootEntry {
            item_id: 3, weight: 1.0, min_count: 1, max_count: 1,
            condition: Some("is_first_kill".to_string()),
        });
        table.entries.push(LootEntry {
            item_id: 5, weight: 5.0, min_count: 10, max_count: 50,
            condition: None,
        });
        assert_eq!(table.entries.len(), 2);
        assert!(table.entries[0].condition.is_some());
        assert!(table.entries[1].condition.is_none());
    }

    #[test]
    fn test_currency_exchange() {
        let currencies = vec![CurrencyType::gold(), CurrencyType::silver(), CurrencyType::platinum()];
        let mut wallet = CurrencyWallet::new();
        wallet.add(1, 100); // 100 gold
        wallet.add(2, 5000); // 5000 silver = 50 gold
        wallet.add(3, 1); // 1 platinum = 100 gold
        let total = wallet.total_in_gold(&currencies);
        assert!((total - 250.0).abs() < 0.1); // 100 + 50 + 100 = 250 gold
    }

    #[test]
    fn test_bag_stacking_behavior() {
        let lib = ItemLibrary::with_defaults();
        let mut bag = InventoryBag::new("Potion Bag", 5, 50.0);

        // health potion: max_stack = 20
        for i in 0..3 {
            let inst = ItemInstance::new(2, i);
            let _ = bag.try_add(inst, 7, &lib);
        }
        // First 20 should be in one slot, rest in another
        let count = bag.total_count(2);
        assert!(count > 0);
    }

    #[test]
    fn test_recipe_validation_full() {
        let lib = ItemLibrary::with_defaults();
        let mut item = Item::new(99);
        item.craftable = true;
        let mut recipe = Recipe::new();
        recipe.ingredients.push((1, 1)); // sword
        recipe.ingredients.push((5, 100)); // 100 gold
        recipe.time = 10.0;
        recipe.station = Some("Forge".to_string());
        item.recipe = Some(recipe);
        let errors = validate_recipe(&item, &lib);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_item_requirements_check() {
        let staff = Item::flame_staff(3);
        let mut stats = HashMap::new();
        stats.insert("Intelligence".to_string(), 19.0_f32); // below requirement of 20
        let (can, reasons) = can_use_item(&staff, 10, &stats);
        assert!(!can);
        assert!(reasons.iter().any(|r| r.contains("Intelligence")));
    }

    #[test]
    fn test_crafting_skill_level_up() {
        let mut skill = CraftingSkill::new("Alchemy");
        let initial_level = skill.level;
        skill.add_xp(10000);
        assert!(skill.level > initial_level);
    }

    #[test]
    fn test_item_set_full_scenario() {
        let mut set = ItemSet::new("Iron Knight Set");
        set.items = vec![1, 4]; // sword and armor
        set.add_bonus(1, vec![ItemStat::new("Defense", 5.0)]);
        set.add_bonus(2, vec![ItemStat::new("Armor Mastery", 10.0)]);

        // With 0 pieces
        assert_eq!(set.active_bonuses(0).len(), 0);
        // With 1 piece — first bonus active
        assert_eq!(set.active_bonuses(1).len(), 1);
        // With 2 pieces — both active
        assert_eq!(set.active_bonuses(2).len(), 2);
    }

    #[test]
    fn test_changelog_recording() {
        let mut log = ItemChangeLog::new(1);
        log.record("name", "Old Sword", "Iron Sword", "editor");
        log.record("base_value", "100", "150", "editor");
        assert_eq!(log.changes.len(), 2);
        assert_eq!(log.recent(1).len(), 1);
        assert_eq!(log.recent(5).len(), 2);
    }

    #[test]
    fn test_proc_trigger_types() {
        for trigger in [
            ProcTrigger::OnHit,
            ProcTrigger::OnKill,
            ProcTrigger::OnReceiveDamage,
            ProcTrigger::OnHeal,
            ProcTrigger::OnSkillUse,
            ProcTrigger::OnLevelUp,
        ] {
            assert!(!trigger.name().is_empty());
        }
    }
}

// ============================================================
// ITEM EXPORT / IMPORT SERIALIZATION HELPERS
// ============================================================

/// Serialize an Item to a simple key=value string format (for config files)
pub fn serialize_item_simple(item: &Item) -> String {
    let mut lines = Vec::new();
    lines.push(format!("id={}", item.id));
    lines.push(format!("name={}", item.name));
    lines.push(format!("category={}", item.category.name()));
    lines.push(format!("rarity={}", item.rarity.name()));
    lines.push(format!("base_value={}", item.base_value));
    lines.push(format!("weight={:.3}", item.weight));
    lines.push(format!("max_stack={}", item.max_stack));
    lines.push(format!("level_req={}", item.level_req));
    lines.push(format!("drop_chance={:.4}", item.drop_chance));
    lines.push(format!("icon_glyph={}", item.icon_glyph));
    lines.push(format!("craftable={}", item.craftable));
    lines.push(format!("unique={}", item.unique));
    lines.push(format!("quest_item={}", item.quest_item));
    for stat in &item.stats {
        lines.push(format!("stat={}:{:.2}", stat.name, stat.base_value));
    }
    for tag in &item.tags {
        lines.push(format!("tag={}", tag));
    }
    lines.join("\n")
}

/// Serialize entire library to tab-separated values (TSV) for spreadsheet export
pub fn export_library_tsv(library: &ItemLibrary) -> String {
    let mut rows = Vec::new();
    rows.push("ID\tName\tCategory\tRarity\tValue\tWeight\tLevel\tDrop%\tSlot\tStats\tEffects\tCraftable\tTags".to_string());
    for item in &library.items {
        let slot = item.equip_slot.as_ref().map(|s| s.name()).unwrap_or("");
        let stats = item.stats.iter().map(|s| format!("{}:{:.1}", s.name, s.base_value)).collect::<Vec<_>>().join("|");
        let effects = item.effects.iter().map(|e| e.description()).collect::<Vec<_>>().join("|");
        let tags = item.tags.join("|");
        rows.push(format!("{}\t{}\t{}\t{}\t{}\t{:.2}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}",
            item.id, item.name, item.category.name(), item.rarity.name(),
            item.base_value, item.weight, item.level_req, item.drop_chance,
            slot, stats, effects, item.craftable, tags,
        ));
    }
    rows.join("\n")
}

/// Quick stats summary for the whole library
pub struct LibraryStats {
    pub total_items: usize,
    pub by_category: HashMap<String, usize>,
    pub by_rarity: HashMap<String, usize>,
    pub avg_value: f32,
    pub avg_weight: f32,
    pub craftable_count: usize,
    pub quest_count: usize,
    pub unique_count: usize,
    pub max_level_item: Option<(u32, String)>,
    pub min_level_item: Option<(u32, String)>,
}

pub fn compute_library_stats(library: &ItemLibrary) -> LibraryStats {
    let mut by_category: HashMap<String, usize> = HashMap::new();
    let mut by_rarity: HashMap<String, usize> = HashMap::new();
    let mut total_value = 0.0_f32;
    let mut total_weight = 0.0_f32;
    let mut craftable_count = 0;
    let mut quest_count = 0;
    let mut unique_count = 0;
    let mut max_level = 0u32;
    let mut min_level = u32::MAX;
    let mut max_level_item = None;
    let mut min_level_item = None;

    for item in &library.items {
        *by_category.entry(item.category.name().to_string()).or_insert(0) += 1;
        *by_rarity.entry(item.rarity.name().to_string()).or_insert(0) += 1;
        total_value += item.base_value as f32;
        total_weight += item.weight;
        if item.craftable { craftable_count += 1; }
        if item.quest_item { quest_count += 1; }
        if item.unique { unique_count += 1; }
        if item.level_req > max_level {
            max_level = item.level_req;
            max_level_item = Some((item.id, item.name.clone()));
        }
        if item.level_req < min_level {
            min_level = item.level_req;
            min_level_item = Some((item.id, item.name.clone()));
        }
    }

    let n = library.items.len().max(1) as f32;
    LibraryStats {
        total_items: library.items.len(),
        by_category, by_rarity,
        avg_value: total_value / n,
        avg_weight: total_weight / n,
        craftable_count, quest_count, unique_count,
        max_level_item, min_level_item,
    }
}

pub fn show_library_stats_panel(ui: &mut egui::Ui, stats: &LibraryStats) {
    ui.heading("Library Statistics");
    ui.label(format!("Total Items: {}", stats.total_items));
    ui.label(format!("Avg Value: {:.1}g   Avg Weight: {:.2}kg", stats.avg_value, stats.avg_weight));
    ui.label(format!("Craftable: {}   Quest: {}   Unique: {}", stats.craftable_count, stats.quest_count, stats.unique_count));

    ui.separator();
    ui.label("By Category:");
    let mut cats: Vec<(&String, &usize)> = stats.by_category.iter().collect();
    cats.sort_by(|a, b| b.1.cmp(a.1));
    for (cat, count) in &cats {
        ui.horizontal(|ui| {
            ui.label(format!("  {}:", cat));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(count.to_string());
            });
        });
    }

    ui.separator();
    ui.label("By Rarity:");
    let mut rarities: Vec<(&String, &usize)> = stats.by_rarity.iter().collect();
    for (rar_name, count) in &rarities {
        let color = Rarity::all().iter().find(|r| r.name() == rar_name.as_str()).map(|r| r.color()).unwrap_or(Color32::GRAY);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("  {}:", rar_name)).color(color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(count.to_string()).color(color));
            });
        });
    }
}

// ============================================================
// FINAL EXTRA TESTS
// ============================================================

#[cfg(test)]
mod final_tests {
    use super::*;

    #[test]
    fn test_serialize_item_simple() {
        let item = Item::sword(1);
        let s = serialize_item_simple(&item);
        assert!(s.contains("id=1"));
        assert!(s.contains("name=Iron Sword"));
        assert!(s.contains("category=Weapon"));
    }

    #[test]
    fn test_export_library_tsv() {
        let lib = ItemLibrary::with_defaults();
        let tsv = export_library_tsv(&lib);
        let rows: Vec<&str> = tsv.lines().collect();
        assert!(rows.len() > 1); // header + at least 1 item
        assert!(rows[0].contains("ID\tName"));
    }

    #[test]
    fn test_library_stats() {
        let lib = ItemLibrary::with_defaults();
        let stats = compute_library_stats(&lib);
        assert_eq!(stats.total_items, lib.items.len());
        assert!(stats.avg_value > 0.0);
        assert!(!stats.by_category.is_empty());
        assert!(!stats.by_rarity.is_empty());
    }

    #[test]
    fn test_currency_wallet_overspend() {
        let mut wallet = CurrencyWallet::new();
        wallet.add(1, 50);
        let result = wallet.spend(1, 100);
        assert!(result.is_err());
        assert_eq!(wallet.balance(1), 50); // unchanged
    }

    #[test]
    fn test_crafting_session_tick() {
        let recipe = Recipe { ingredients: vec![], result_count: 1, skill_req: None, station: None, time: 2.0 };
        let mut session = CraftingSession::new(recipe, 1, 10);
        assert!(!session.tick(1.0)); // not done yet (1/2 s)
        assert!(session.tick(1.5)); // done
    }

    #[test]
    fn test_crafting_skill_xp_multiple_levels() {
        let mut skill = CraftingSkill::new("Woodcutting");
        for _ in 0..100 {
            skill.add_xp(200);
        }
        assert!(skill.level >= 5);
    }

    #[test]
    fn test_rarity_color_distinct() {
        let colors: Vec<Color32> = Rarity::all().iter().map(|r| r.color()).collect();
        // All rarities should have distinct colors
        for i in 0..colors.len() {
            for j in (i+1)..colors.len() {
                assert_ne!(colors[i], colors[j], "Rarities {} and {} have same color", i, j);
            }
        }
    }

    #[test]
    fn test_item_effect_descriptions_non_empty() {
        for type_name in ItemEffect::all_type_names() {
            let eff = ItemEffect::default_for_type(type_name);
            assert!(!eff.description().is_empty(), "Empty description for {}", type_name);
        }
    }

    #[test]
    fn test_equip_slot_names_unique() {
        let names: Vec<&str> = EquipSlot::all().iter().map(|s| s.name()).collect();
        let unique: HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), unique.len());
    }

    #[test]
    fn test_sort_field_names_unique() {
        let names: Vec<&str> = SortField::all().iter().map(|s| s.name()).collect();
        let unique: HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), unique.len());
    }
}
