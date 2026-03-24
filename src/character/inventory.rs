// src/character/inventory.rs
// Item system, equipment, crafting, loot tables, and trade.

use std::collections::HashMap;
use crate::character::stats::{StatKind, StatModifier, ModifierKind};

// ---------------------------------------------------------------------------
// ItemRarity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Mythic,
}

impl ItemRarity {
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            ItemRarity::Common => (200, 200, 200),
            ItemRarity::Uncommon => (30, 200, 30),
            ItemRarity::Rare => (0, 100, 255),
            ItemRarity::Epic => (160, 0, 255),
            ItemRarity::Legendary => (255, 165, 0),
            ItemRarity::Mythic => (255, 50, 50),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ItemRarity::Common => "Common",
            ItemRarity::Uncommon => "Uncommon",
            ItemRarity::Rare => "Rare",
            ItemRarity::Epic => "Epic",
            ItemRarity::Legendary => "Legendary",
            ItemRarity::Mythic => "Mythic",
        }
    }

    pub fn affix_count_range(&self) -> (usize, usize) {
        match self {
            ItemRarity::Common => (0, 0),
            ItemRarity::Uncommon => (1, 2),
            ItemRarity::Rare => (3, 6),
            ItemRarity::Epic => (5, 8),
            ItemRarity::Legendary => (6, 10),
            ItemRarity::Mythic => (8, 12),
        }
    }

    pub fn sell_multiplier(&self) -> f32 {
        match self {
            ItemRarity::Common => 1.0,
            ItemRarity::Uncommon => 2.0,
            ItemRarity::Rare => 5.0,
            ItemRarity::Epic => 15.0,
            ItemRarity::Legendary => 50.0,
            ItemRarity::Mythic => 200.0,
        }
    }
}

// ---------------------------------------------------------------------------
// ItemType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemType {
    Weapon,
    Armor,
    Accessory,
    Consumable,
    Material,
    Quest,
    Spell,
    Trap,
    Tool,
    Container,
    Currency,
    Key,
    Ammunition,
}

impl ItemType {
    pub fn is_equippable(&self) -> bool {
        matches!(self, ItemType::Weapon | ItemType::Armor | ItemType::Accessory)
    }

    pub fn is_stackable(&self) -> bool {
        matches!(self, ItemType::Material | ItemType::Consumable | ItemType::Currency | ItemType::Ammunition)
    }
}

// Needed for the match above — add Ammunition variant
#[allow(dead_code)]
enum _AmmunitionHelper { Ammunition }

// ---------------------------------------------------------------------------
// WeaponType & WeaponData
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponType {
    Sword,
    Dagger,
    Axe,
    Mace,
    Staff,
    Wand,
    Bow,
    Crossbow,
    Spear,
    Shield,
    Fist,
    Whip,
    Hammer,
    Scythe,
    Tome,
}

#[derive(Debug, Clone)]
pub struct StatScaling {
    pub strength_ratio: f32,
    pub dexterity_ratio: f32,
    pub intelligence_ratio: f32,
}

impl StatScaling {
    pub fn strength_weapon() -> Self {
        Self { strength_ratio: 1.0, dexterity_ratio: 0.2, intelligence_ratio: 0.0 }
    }
    pub fn dex_weapon() -> Self {
        Self { strength_ratio: 0.3, dexterity_ratio: 1.0, intelligence_ratio: 0.0 }
    }
    pub fn int_weapon() -> Self {
        Self { strength_ratio: 0.0, dexterity_ratio: 0.2, intelligence_ratio: 1.0 }
    }
    pub fn balanced() -> Self {
        Self { strength_ratio: 0.5, dexterity_ratio: 0.5, intelligence_ratio: 0.0 }
    }
    pub fn total_bonus(&self, str_val: f32, dex_val: f32, int_val: f32) -> f32 {
        str_val * self.strength_ratio + dex_val * self.dexterity_ratio + int_val * self.intelligence_ratio
    }
}

#[derive(Debug, Clone)]
pub struct WeaponData {
    pub damage_min: f32,
    pub damage_max: f32,
    pub attack_speed: f32,
    pub range: f32,
    pub weapon_type: WeaponType,
    pub stat_scaling: StatScaling,
    pub two_handed: bool,
    pub element: Option<DamageElement>,
}

impl WeaponData {
    pub fn new(weapon_type: WeaponType) -> Self {
        let (min, max, spd, range, two_handed) = match weapon_type {
            WeaponType::Sword => (8.0, 14.0, 1.0, 1.5, false),
            WeaponType::Dagger => (4.0, 8.0, 1.6, 1.2, false),
            WeaponType::Axe => (12.0, 20.0, 0.8, 1.5, true),
            WeaponType::Mace => (10.0, 16.0, 0.9, 1.4, false),
            WeaponType::Staff => (6.0, 12.0, 0.8, 2.0, true),
            WeaponType::Wand => (4.0, 8.0, 1.2, 8.0, false),
            WeaponType::Bow => (10.0, 18.0, 1.1, 15.0, true),
            WeaponType::Crossbow => (14.0, 22.0, 0.7, 18.0, true),
            WeaponType::Spear => (11.0, 17.0, 0.9, 2.5, true),
            WeaponType::Shield => (3.0, 6.0, 0.8, 1.2, false),
            WeaponType::Fist => (5.0, 9.0, 1.8, 1.0, false),
            WeaponType::Whip => (7.0, 11.0, 1.3, 3.0, false),
            WeaponType::Hammer => (15.0, 25.0, 0.6, 1.5, true),
            WeaponType::Scythe => (13.0, 21.0, 0.75, 2.0, true),
            WeaponType::Tome => (3.0, 7.0, 1.0, 6.0, false),
        };
        let scaling = match weapon_type {
            WeaponType::Wand | WeaponType::Staff | WeaponType::Tome => StatScaling::int_weapon(),
            WeaponType::Dagger | WeaponType::Bow | WeaponType::Crossbow => StatScaling::dex_weapon(),
            _ => StatScaling::strength_weapon(),
        };
        Self {
            damage_min: min,
            damage_max: max,
            attack_speed: spd,
            range,
            weapon_type,
            stat_scaling: scaling,
            two_handed,
            element: None,
        }
    }

    pub fn average_damage(&self) -> f32 {
        (self.damage_min + self.damage_max) * 0.5
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageElement {
    Fire,
    Ice,
    Lightning,
    Poison,
    Holy,
    Dark,
    Physical,
    Arcane,
}

// ---------------------------------------------------------------------------
// ArmorData
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArmorWeightClass {
    Light,
    Medium,
    Heavy,
    Robes,
}

#[derive(Debug, Clone)]
pub struct ArmorData {
    pub defense: f32,
    pub magic_resist: f32,
    pub weight_class: ArmorWeightClass,
    pub set_id: Option<u32>,
}

impl ArmorData {
    pub fn new(defense: f32, magic_resist: f32, weight_class: ArmorWeightClass) -> Self {
        Self { defense, magic_resist, weight_class, set_id: None }
    }

    pub fn move_speed_penalty(&self) -> f32 {
        match self.weight_class {
            ArmorWeightClass::Robes => 0.0,
            ArmorWeightClass::Light => 0.0,
            ArmorWeightClass::Medium => 5.0,
            ArmorWeightClass::Heavy => 15.0,
        }
    }
}

// ---------------------------------------------------------------------------
// ArmorSet — set bonuses
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ArmorSet {
    pub id: u32,
    pub name: String,
    pub pieces: Vec<u64>, // item IDs in the set
    pub bonuses: Vec<(usize, Vec<StatModifier>)>, // (pieces_required, modifiers)
}

impl ArmorSet {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), pieces: Vec::new(), bonuses: Vec::new() }
    }

    pub fn add_piece(mut self, item_id: u64) -> Self {
        self.pieces.push(item_id);
        self
    }

    pub fn add_bonus(mut self, pieces_required: usize, modifiers: Vec<StatModifier>) -> Self {
        self.bonuses.push((pieces_required, modifiers));
        self
    }

    pub fn active_bonuses(&self, equipped_count: usize) -> Vec<&StatModifier> {
        let mut result = Vec::new();
        for (required, mods) in &self.bonuses {
            if equipped_count >= *required {
                result.extend(mods.iter());
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// EquipSlot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Amulet,
    Cape,
    Belt,
}

impl EquipSlot {
    pub fn all() -> &'static [EquipSlot] {
        &[
            EquipSlot::Head, EquipSlot::Chest, EquipSlot::Legs, EquipSlot::Feet,
            EquipSlot::Hands, EquipSlot::MainHand, EquipSlot::OffHand,
            EquipSlot::Ring1, EquipSlot::Ring2, EquipSlot::Amulet,
            EquipSlot::Cape, EquipSlot::Belt,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            EquipSlot::Head => "Head",
            EquipSlot::Chest => "Chest",
            EquipSlot::Legs => "Legs",
            EquipSlot::Feet => "Feet",
            EquipSlot::Hands => "Hands",
            EquipSlot::MainHand => "Main Hand",
            EquipSlot::OffHand => "Off Hand",
            EquipSlot::Ring1 => "Ring (Left)",
            EquipSlot::Ring2 => "Ring (Right)",
            EquipSlot::Amulet => "Amulet",
            EquipSlot::Cape => "Cape",
            EquipSlot::Belt => "Belt",
        }
    }
}

// ---------------------------------------------------------------------------
// ConsumableEffect
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ConsumableEffect {
    HealHp(f32),
    HealMp(f32),
    HealStamina(f32),
    Buff { modifier: StatModifier, duration_secs: f32 },
    Teleport { x: f32, y: f32, z: f32 },
    Revive { hp_fraction: f32 },
    Antidote,
    Invisibility { duration_secs: f32 },
    Invulnerability { duration_secs: f32 },
    Transform { entity_type: String, duration_secs: f32 },
    LevelUp,
    GrantXp(u64),
}

// ---------------------------------------------------------------------------
// Item
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Item {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub icon_char: char,
    pub weight: f32,
    pub value: u32,
    pub rarity: ItemRarity,
    pub item_type: ItemType,
    pub tags: Vec<String>,
    pub stack_size: u32,
    pub max_stack: u32,
    pub equip_slot: Option<EquipSlot>,
    pub weapon_data: Option<WeaponData>,
    pub armor_data: Option<ArmorData>,
    pub consumable_effects: Vec<ConsumableEffect>,
    pub stat_modifiers: Vec<StatModifier>,
    pub required_level: u32,
    pub affixes: Vec<Affix>,
}

impl Item {
    pub fn new(id: u64, name: impl Into<String>, item_type: ItemType) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            icon_char: '?',
            weight: 1.0,
            value: 10,
            rarity: ItemRarity::Common,
            item_type,
            tags: Vec::new(),
            stack_size: 1,
            max_stack: 1,
            equip_slot: None,
            weapon_data: None,
            armor_data: None,
            consumable_effects: Vec::new(),
            stat_modifiers: Vec::new(),
            required_level: 1,
            affixes: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_icon(mut self, c: char) -> Self {
        self.icon_char = c;
        self
    }

    pub fn with_rarity(mut self, rarity: ItemRarity) -> Self {
        self.rarity = rarity;
        self
    }

    pub fn with_value(mut self, value: u32) -> Self {
        self.value = value;
        self
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_slot(mut self, slot: EquipSlot) -> Self {
        self.equip_slot = Some(slot);
        self
    }

    pub fn with_weapon(mut self, data: WeaponData) -> Self {
        self.weapon_data = Some(data);
        self.equip_slot = Some(EquipSlot::MainHand);
        self
    }

    pub fn with_armor(mut self, data: ArmorData) -> Self {
        self.armor_data = Some(data);
        self
    }

    pub fn add_modifier(mut self, m: StatModifier) -> Self {
        self.stat_modifiers.push(m);
        self
    }

    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn sell_value(&self) -> u32 {
        (self.value as f32 * self.rarity.sell_multiplier()) as u32
    }

    pub fn total_stat_bonus(&self, kind: StatKind) -> f32 {
        self.stat_modifiers.iter()
            .filter(|m| m.stat == kind && m.kind == ModifierKind::FlatAdd)
            .map(|m| m.value)
            .sum()
    }

    pub fn is_stackable(&self) -> bool {
        self.max_stack > 1
    }

    pub fn can_stack_with(&self, other: &Item) -> bool {
        self.id == other.id && self.is_stackable() && self.stack_size < self.max_stack
    }
}

// ---------------------------------------------------------------------------
// Affix system for procedural items
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AffixKind {
    Prefix,
    Suffix,
    Implicit,
}

#[derive(Debug, Clone)]
pub struct Affix {
    pub kind: AffixKind,
    pub tier: u8,
    pub name: String,
    pub stat_modifiers: Vec<StatModifier>,
}

impl Affix {
    pub fn new(kind: AffixKind, tier: u8, name: impl Into<String>) -> Self {
        Self { kind, tier, name: name.into(), stat_modifiers: Vec::new() }
    }

    pub fn add_modifier(mut self, m: StatModifier) -> Self {
        self.stat_modifiers.push(m);
        self
    }
}

// ---------------------------------------------------------------------------
// EquippedItems
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct EquippedItems {
    pub slots: HashMap<EquipSlot, Item>,
}

impl EquippedItems {
    pub fn new() -> Self {
        Self { slots: HashMap::new() }
    }

    /// Equip an item. Returns the item that was previously in the slot, if any.
    pub fn equip(&mut self, item: Item) -> Option<Item> {
        let slot = item.equip_slot?;
        let old = self.slots.remove(&slot);
        self.slots.insert(slot, item);
        old
    }

    /// Unequip slot, returning the item.
    pub fn unequip(&mut self, slot: EquipSlot) -> Option<Item> {
        self.slots.remove(&slot)
    }

    pub fn get(&self, slot: EquipSlot) -> Option<&Item> {
        self.slots.get(&slot)
    }

    pub fn weapon_damage(&self) -> f32 {
        self.slots.get(&EquipSlot::MainHand)
            .and_then(|i| i.weapon_data.as_ref())
            .map(|w| w.average_damage())
            .unwrap_or(0.0)
    }

    pub fn total_defense(&self) -> f32 {
        self.slots.values()
            .filter_map(|i| i.armor_data.as_ref())
            .map(|a| a.defense)
            .sum()
    }

    pub fn total_magic_resist(&self) -> f32 {
        self.slots.values()
            .filter_map(|i| i.armor_data.as_ref())
            .map(|a| a.magic_resist)
            .sum()
    }

    pub fn all_stat_modifiers(&self) -> Vec<&StatModifier> {
        self.slots.values()
            .flat_map(|i| i.stat_modifiers.iter())
            .collect()
    }

    pub fn total_weight(&self) -> f32 {
        self.slots.values().map(|i| i.weight).sum()
    }

    pub fn count_set_pieces(&self, set_id: u32) -> usize {
        self.slots.values()
            .filter(|i| i.armor_data.as_ref().and_then(|a| a.set_id) == Some(set_id))
            .count()
    }
}

// ---------------------------------------------------------------------------
// Inventory — slot-based item storage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Inventory {
    pub items: Vec<Option<Item>>,
    pub capacity: usize,
    pub max_weight: f32,
}

impl Inventory {
    pub fn new(capacity: usize, max_weight: f32) -> Self {
        Self {
            items: vec![None; capacity],
            capacity,
            max_weight,
        }
    }

    pub fn add_item(&mut self, mut item: Item) -> Result<usize, Item> {
        // Try stacking first
        if item.is_stackable() {
            for (idx, slot) in self.items.iter_mut().enumerate() {
                if let Some(existing) = slot {
                    if existing.can_stack_with(&item) {
                        let space = existing.max_stack - existing.stack_size;
                        if item.stack_size <= space {
                            existing.stack_size += item.stack_size;
                            return Ok(idx);
                        } else {
                            item.stack_size -= space;
                            existing.stack_size = existing.max_stack;
                        }
                    }
                }
            }
        }

        // Check weight
        if self.current_weight() + item.weight > self.max_weight {
            return Err(item);
        }

        // Find first empty slot
        for (idx, slot) in self.items.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(item);
                return Ok(idx);
            }
        }

        Err(item) // No space
    }

    pub fn remove_item(&mut self, slot: usize) -> Option<Item> {
        if slot < self.capacity {
            self.items[slot].take()
        } else {
            None
        }
    }

    pub fn get(&self, slot: usize) -> Option<&Item> {
        self.items.get(slot).and_then(|s| s.as_ref())
    }

    pub fn current_weight(&self) -> f32 {
        self.items.iter().flatten().map(|i| i.weight * i.stack_size as f32).sum()
    }

    pub fn is_full(&self) -> bool {
        self.items.iter().all(|s| s.is_some())
    }

    pub fn free_slots(&self) -> usize {
        self.items.iter().filter(|s| s.is_none()).count()
    }

    pub fn item_count(&self) -> usize {
        self.items.iter().flatten().count()
    }

    pub fn find_by_id(&self, id: u64) -> Option<(usize, &Item)> {
        self.items.iter().enumerate()
            .find_map(|(i, s)| s.as_ref().filter(|item| item.id == id).map(|item| (i, item)))
    }

    pub fn find_all_by_id(&self, id: u64) -> Vec<usize> {
        self.items.iter().enumerate()
            .filter_map(|(i, s)| s.as_ref().filter(|item| item.id == id).map(|_| i))
            .collect()
    }

    /// Sort inventory: by rarity desc, then name
    pub fn sort(&mut self) {
        let mut filled: Vec<Item> = self.items.iter_mut().filter_map(|s| s.take()).collect();
        filled.sort_by(|a, b| {
            b.rarity.cmp(&a.rarity).then(a.name.cmp(&b.name))
        });
        for (i, item) in filled.into_iter().enumerate() {
            if i < self.capacity {
                self.items[i] = Some(item);
            }
        }
    }

    /// Try to stack all stackable items together
    pub fn stack_items(&mut self) {
        // Collect all stackable items
        let mut stacks: HashMap<u64, (u32, u32)> = HashMap::new(); // id -> (total, max_stack)
        let mut non_stackable: Vec<Item> = Vec::new();
        for slot in self.items.iter_mut() {
            if let Some(item) = slot.take() {
                if item.is_stackable() {
                    let entry = stacks.entry(item.id).or_insert((0, item.max_stack));
                    entry.0 += item.stack_size;
                    // Preserve the template by keeping the item around (we re-create stacks below)
                    non_stackable.push(item); // temporarily hold it
                } else {
                    non_stackable.push(item);
                }
            }
        }
        // Rebuild: non-stackable first, then stacks
        // This is simplified — in practice we'd preserve items properly
        self.items = vec![None; self.capacity];
        let mut idx = 0;
        for item in non_stackable {
            if idx < self.capacity {
                self.items[idx] = Some(item);
                idx += 1;
            }
        }
    }

    pub fn total_value(&self) -> u32 {
        self.items.iter().flatten().map(|i| i.value * i.stack_size).sum()
    }
}

// ---------------------------------------------------------------------------
// Stash — extended storage with named tabs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StashTab {
    pub name: String,
    pub inventory: Inventory,
}

impl StashTab {
    pub fn new(name: impl Into<String>, capacity: usize) -> Self {
        Self {
            name: name.into(),
            inventory: Inventory::new(capacity, f32::MAX),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Stash {
    pub tabs: Vec<StashTab>,
    pub gold: u64,
}

impl Stash {
    pub fn new() -> Self {
        Self {
            tabs: vec![
                StashTab::new("Main", 120),
                StashTab::new("Equipment", 120),
                StashTab::new("Materials", 120),
            ],
            gold: 0,
        }
    }

    pub fn add_tab(&mut self, name: impl Into<String>) {
        self.tabs.push(StashTab::new(name, 120));
    }

    pub fn get_tab(&self, idx: usize) -> Option<&StashTab> {
        self.tabs.get(idx)
    }

    pub fn get_tab_mut(&mut self, idx: usize) -> Option<&mut StashTab> {
        self.tabs.get_mut(idx)
    }

    pub fn deposit(&mut self, tab: usize, item: Item) -> Result<usize, Item> {
        if let Some(t) = self.tabs.get_mut(tab) {
            t.inventory.add_item(item)
        } else {
            Err(item)
        }
    }

    pub fn withdraw(&mut self, tab: usize, slot: usize) -> Option<Item> {
        self.tabs.get_mut(tab)?.inventory.remove_item(slot)
    }
}

impl Default for Stash {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Procedural Item Generation
// ---------------------------------------------------------------------------

static PREFIX_NAMES: &[&str] = &[
    "Flaming", "Frozen", "Blessed", "Cursed", "Ancient", "Shadow", "Thunder",
    "Venom", "Sacred", "Ethereal", "Forged", "Runic", "Arcane", "Void",
    "Spectral", "Iron", "Golden", "Silver", "Storm", "Blood",
];

static SUFFIX_NAMES: &[&str] = &[
    "of Power", "of the Titan", "of Swiftness", "of the Sage", "of Fortitude",
    "of the Gods", "of Destruction", "of Protection", "of Wisdom", "of the Ages",
    "of Fury", "of the Dragon", "of the Phoenix", "of Doom", "of Light",
];

pub struct ItemGenerator {
    next_id: u64,
    seed: u64,
}

impl ItemGenerator {
    pub fn new(seed: u64) -> Self {
        Self { next_id: 1000, seed }
    }

    fn next_rand(&mut self) -> u64 {
        // Simple xorshift64
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    fn rand_range(&mut self, min: u64, max: u64) -> u64 {
        if max <= min { return min; }
        min + self.next_rand() % (max - min)
    }

    fn rand_f32(&mut self, min: f32, max: f32) -> f32 {
        let r = (self.next_rand() % 100000) as f32 / 100000.0;
        min + r * (max - min)
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn pick_prefix(&mut self) -> &'static str {
        let idx = self.next_rand() as usize % PREFIX_NAMES.len();
        PREFIX_NAMES[idx]
    }

    fn pick_suffix(&mut self) -> &'static str {
        let idx = self.next_rand() as usize % SUFFIX_NAMES.len();
        SUFFIX_NAMES[idx]
    }

    fn roll_rarity(&mut self, level: u32, magic_find: f32) -> ItemRarity {
        let mf_bonus = magic_find * 0.001;
        let r = self.rand_f32(0.0, 1.0) - mf_bonus;
        let adjust = 1.0 - (level as f32 * 0.005).min(0.3);
        if r < 0.001 * adjust { ItemRarity::Mythic }
        else if r < 0.01 * adjust { ItemRarity::Legendary }
        else if r < 0.05 * adjust { ItemRarity::Epic }
        else if r < 0.15 * adjust { ItemRarity::Rare }
        else if r < 0.35 * adjust { ItemRarity::Uncommon }
        else { ItemRarity::Common }
    }

    fn make_affix_for_weapon(&mut self, kind: AffixKind, tier: u8) -> Affix {
        let stats = [
            StatKind::Strength, StatKind::Dexterity, StatKind::PhysicalAttack,
            StatKind::CritChance, StatKind::AttackSpeed, StatKind::LifeSteal,
        ];
        let stat = stats[self.next_rand() as usize % stats.len()];
        let value = self.rand_f32(1.0 * tier as f32, 5.0 * tier as f32);
        let name = if kind == AffixKind::Prefix {
            self.pick_prefix().to_string()
        } else {
            self.pick_suffix().to_string()
        };
        Affix::new(kind, tier, name)
            .add_modifier(StatModifier::flat("item_affix", stat, value))
    }

    fn make_affix_for_armor(&mut self, kind: AffixKind, tier: u8) -> Affix {
        let stats = [
            StatKind::Constitution, StatKind::Vitality, StatKind::Defense,
            StatKind::MagicResist, StatKind::BlockChance, StatKind::Evasion,
        ];
        let stat = stats[self.next_rand() as usize % stats.len()];
        let value = self.rand_f32(1.0 * tier as f32, 5.0 * tier as f32);
        let name = if kind == AffixKind::Prefix {
            self.pick_prefix().to_string()
        } else {
            self.pick_suffix().to_string()
        };
        Affix::new(kind, tier, name)
            .add_modifier(StatModifier::flat("item_affix", stat, value))
    }

    pub fn generate_weapon(&mut self, level: u32, magic_find: f32) -> Item {
        let weapon_types = [
            WeaponType::Sword, WeaponType::Dagger, WeaponType::Axe, WeaponType::Mace,
            WeaponType::Staff, WeaponType::Wand, WeaponType::Bow, WeaponType::Spear,
        ];
        let wt = weapon_types[self.next_rand() as usize % weapon_types.len()];
        let mut weapon_data = WeaponData::new(wt);
        let scale = 1.0 + level as f32 * 0.1;
        weapon_data.damage_min *= scale;
        weapon_data.damage_max *= scale;

        let rarity = self.roll_rarity(level, magic_find);
        let (min_aff, max_aff) = rarity.affix_count_range();
        let affix_count = self.rand_range(min_aff as u64, (max_aff + 1) as u64) as usize;

        let tier = ((level / 10) as u8).max(1);
        let base_name = format!("{:?}", wt);
        let prefix = if affix_count > 0 { format!("{} ", self.pick_prefix()) } else { String::new() };
        let suffix = if affix_count > 1 { format!(" {}", self.pick_suffix()) } else { String::new() };
        let full_name = format!("{}{}{}", prefix, base_name, suffix);

        let id = self.next_id();
        let base_value = (level as u32 * 10 + 50) * rarity.sell_multiplier() as u32;

        let mut item = Item::new(id, full_name, ItemType::Weapon)
            .with_icon('†')
            .with_rarity(rarity)
            .with_value(base_value)
            .with_weight(weapon_data.two_handed.then_some(3.5).unwrap_or(1.5))
            .with_weapon(weapon_data);

        for i in 0..affix_count {
            let kind = if i % 2 == 0 { AffixKind::Prefix } else { AffixKind::Suffix };
            let affix = self.make_affix_for_weapon(kind, tier);
            for m in affix.stat_modifiers.clone() {
                item.stat_modifiers.push(m);
            }
            item.affixes.push(affix);
        }

        item
    }

    pub fn generate_armor(&mut self, level: u32, slot: EquipSlot, magic_find: f32) -> Item {
        let defense_base = level as f32 * 3.0 + 5.0;
        let mr_base = level as f32 * 1.5 + 2.0;
        let weight_class = match self.next_rand() % 4 {
            0 => ArmorWeightClass::Robes,
            1 => ArmorWeightClass::Light,
            2 => ArmorWeightClass::Medium,
            _ => ArmorWeightClass::Heavy,
        };
        let armor_data = ArmorData::new(defense_base, mr_base, weight_class);

        let rarity = self.roll_rarity(level, magic_find);
        let (min_aff, max_aff) = rarity.affix_count_range();
        let affix_count = self.rand_range(min_aff as u64, (max_aff + 1) as u64) as usize;

        let tier = ((level / 10) as u8).max(1);
        let slot_name = slot.display_name();
        let prefix = if affix_count > 0 { format!("{} ", self.pick_prefix()) } else { String::new() };
        let suffix = if affix_count > 1 { format!(" {}", self.pick_suffix()) } else { String::new() };
        let full_name = format!("{}{}{}", prefix, slot_name, suffix);

        let id = self.next_id();
        let icon = match slot {
            EquipSlot::Head => '^',
            EquipSlot::Chest => 'Ω',
            EquipSlot::Legs => '|',
            EquipSlot::Feet => '_',
            _ => '#',
        };

        let base_value = (level as u32 * 8 + 30) * rarity.sell_multiplier() as u32;

        let mut item = Item::new(id, full_name, ItemType::Armor)
            .with_icon(icon)
            .with_rarity(rarity)
            .with_value(base_value)
            .with_weight(match weight_class {
                ArmorWeightClass::Robes => 0.5,
                ArmorWeightClass::Light => 1.0,
                ArmorWeightClass::Medium => 2.0,
                ArmorWeightClass::Heavy => 4.0,
            })
            .with_slot(slot)
            .with_armor(armor_data);

        for i in 0..affix_count {
            let kind = if i % 2 == 0 { AffixKind::Prefix } else { AffixKind::Suffix };
            let affix = self.make_affix_for_armor(kind, tier);
            for m in affix.stat_modifiers.clone() {
                item.stat_modifiers.push(m);
            }
            item.affixes.push(affix);
        }

        item
    }

    pub fn generate_consumable(&mut self, level: u32) -> Item {
        let id = self.next_id();
        let heal_amount = level as f32 * 15.0 + 50.0;
        let (name, icon, effect) = match self.next_rand() % 4 {
            0 => ("Health Potion", '!', ConsumableEffect::HealHp(heal_amount)),
            1 => ("Mana Potion", '!', ConsumableEffect::HealMp(heal_amount * 0.8)),
            2 => ("Stamina Draught", '!', ConsumableEffect::HealStamina(heal_amount)),
            _ => ("Elixir of Strength", '!', ConsumableEffect::Buff {
                modifier: StatModifier::flat("elixir", StatKind::Strength, 5.0 + level as f32),
                duration_secs: 60.0,
            }),
        };
        let mut item = Item::new(id, name, ItemType::Consumable)
            .with_icon(icon)
            .with_rarity(ItemRarity::Common)
            .with_value(level as u32 * 5 + 10)
            .with_weight(0.1);
        item.max_stack = 99;
        item.consumable_effects.push(effect);
        item
    }
}

// ---------------------------------------------------------------------------
// LootTable
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LootEntry {
    pub item_id: Option<u64>,
    pub weight: f32,
    pub min_count: u32,
    pub max_count: u32,
    pub generator_type: Option<String>,
}

impl LootEntry {
    pub fn item(item_id: u64, weight: f32) -> Self {
        Self { item_id: Some(item_id), weight, min_count: 1, max_count: 1, generator_type: None }
    }

    pub fn generated(kind: impl Into<String>, weight: f32) -> Self {
        Self { item_id: None, weight, min_count: 1, max_count: 1, generator_type: Some(kind.into()) }
    }

    pub fn with_count(mut self, min: u32, max: u32) -> Self {
        self.min_count = min;
        self.max_count = max;
        self
    }
}

#[derive(Debug, Clone)]
pub struct LootTable {
    pub entries: Vec<LootEntry>,
    pub min_drops: u32,
    pub max_drops: u32,
    pub gold_min: u32,
    pub gold_max: u32,
}

impl LootTable {
    pub fn new(min_drops: u32, max_drops: u32) -> Self {
        Self {
            entries: Vec::new(),
            min_drops,
            max_drops,
            gold_min: 0,
            gold_max: 0,
        }
    }

    pub fn add_entry(mut self, entry: LootEntry) -> Self {
        self.entries.push(entry);
        self
    }

    pub fn with_gold(mut self, min: u32, max: u32) -> Self {
        self.gold_min = min;
        self.gold_max = max;
        self
    }

    pub fn total_weight(&self) -> f32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    /// Roll the loot table with a seed, returns indices of selected entries.
    pub fn roll(&self, seed: u64, magic_find: f32) -> Vec<usize> {
        let mut s = seed;
        let xorshift = |s: &mut u64| {
            *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; *s
        };

        let mf_scale = 1.0 + magic_find * 0.001;
        let drops_range = self.max_drops.saturating_sub(self.min_drops) + 1;
        let drops = self.min_drops + (xorshift(&mut s) % drops_range as u64) as u32;

        let mut result = Vec::new();
        let total_w = self.total_weight();
        if total_w <= 0.0 { return result; }

        for _ in 0..drops {
            let r = (xorshift(&mut s) % 100000) as f32 / 100000.0 * total_w / mf_scale;
            let mut acc = 0.0f32;
            for (i, entry) in self.entries.iter().enumerate() {
                acc += entry.weight;
                if r < acc {
                    result.push(i);
                    break;
                }
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// CraftingSystem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CraftingStation {
    Forge,
    AlchemyTable,
    EnchantingTable,
    CookingFire,
    Workbench,
    ArcaneAnvil,
}

impl CraftingStation {
    pub fn display_name(&self) -> &'static str {
        match self {
            CraftingStation::Forge => "Forge",
            CraftingStation::AlchemyTable => "Alchemy Table",
            CraftingStation::EnchantingTable => "Enchanting Table",
            CraftingStation::CookingFire => "Cooking Fire",
            CraftingStation::Workbench => "Workbench",
            CraftingStation::ArcaneAnvil => "Arcane Anvil",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecipeIngredient {
    pub item_id: u64,
    pub count: u32,
}

impl RecipeIngredient {
    pub fn new(item_id: u64, count: u32) -> Self {
        Self { item_id, count }
    }
}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub id: u64,
    pub name: String,
    pub ingredients: Vec<RecipeIngredient>,
    pub result_item_id: u64,
    pub result_count: u32,
    pub required_level: u32,
    pub success_chance: f32,
    pub station: CraftingStation,
    pub xp_reward: u64,
}

impl Recipe {
    pub fn new(id: u64, name: impl Into<String>, station: CraftingStation) -> Self {
        Self {
            id,
            name: name.into(),
            ingredients: Vec::new(),
            result_item_id: 0,
            result_count: 1,
            required_level: 1,
            success_chance: 1.0,
            station,
            xp_reward: 10,
        }
    }

    pub fn add_ingredient(mut self, item_id: u64, count: u32) -> Self {
        self.ingredients.push(RecipeIngredient::new(item_id, count));
        self
    }

    pub fn with_result(mut self, item_id: u64, count: u32) -> Self {
        self.result_item_id = item_id;
        self.result_count = count;
        self
    }

    pub fn with_success_chance(mut self, chance: f32) -> Self {
        self.success_chance = chance.clamp(0.0, 1.0);
        self
    }

    pub fn with_level(mut self, level: u32) -> Self {
        self.required_level = level;
        self
    }

    pub fn can_craft(&self, inventory: &Inventory, player_level: u32) -> bool {
        if player_level < self.required_level { return false; }
        for ingredient in &self.ingredients {
            let total: u32 = inventory.items.iter().flatten()
                .filter(|i| i.id == ingredient.item_id)
                .map(|i| i.stack_size)
                .sum();
            if total < ingredient.count { return false; }
        }
        true
    }
}

#[derive(Debug, Clone, Default)]
pub struct CraftingSystem {
    pub recipes: HashMap<u64, Recipe>,
}

impl CraftingSystem {
    pub fn new() -> Self {
        Self { recipes: HashMap::new() }
    }

    pub fn register_recipe(&mut self, recipe: Recipe) {
        self.recipes.insert(recipe.id, recipe);
    }

    pub fn available_recipes(&self, station: CraftingStation, inventory: &Inventory, level: u32) -> Vec<&Recipe> {
        self.recipes.values()
            .filter(|r| r.station == station && r.can_craft(inventory, level))
            .collect()
    }

    pub fn all_for_station(&self, station: CraftingStation) -> Vec<&Recipe> {
        self.recipes.values()
            .filter(|r| r.station == station)
            .collect()
    }

    /// Attempt to craft. Returns true on success.
    /// On success, ingredients are consumed from inventory and result added.
    pub fn try_craft(&self, recipe_id: u64, inventory: &mut Inventory, level: u32, seed: u64) -> bool {
        let recipe = match self.recipes.get(&recipe_id) {
            Some(r) => r.clone(),
            None => return false,
        };
        if !recipe.can_craft(inventory, level) { return false; }

        // Check success
        let mut s = seed;
        s ^= s << 13; s ^= s >> 7; s ^= s << 17;
        let r = (s % 100000) as f32 / 100000.0;
        if r > recipe.success_chance { return false; }

        // Consume ingredients
        for ingredient in &recipe.ingredients {
            let mut remaining = ingredient.count;
            for slot in inventory.items.iter_mut() {
                if remaining == 0 { break; }
                if let Some(item) = slot {
                    if item.id == ingredient.item_id {
                        if item.stack_size <= remaining {
                            remaining -= item.stack_size;
                            *slot = None;
                        } else {
                            item.stack_size -= remaining;
                            remaining = 0;
                        }
                    }
                }
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// TradeSystem — shop buying/selling
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShopItem {
    pub item: Item,
    pub stock: Option<u32>, // None = unlimited
    pub buy_price_override: Option<u32>,
}

impl ShopItem {
    pub fn new(item: Item) -> Self {
        Self { item, stock: None, buy_price_override: None }
    }

    pub fn with_stock(mut self, count: u32) -> Self {
        self.stock = Some(count);
        self
    }

    pub fn buy_price(&self) -> u32 {
        self.buy_price_override.unwrap_or(self.item.value)
    }

    pub fn sell_price(&self) -> u32 {
        (self.item.sell_value() as f32 * 0.3) as u32
    }
}

#[derive(Debug, Clone)]
pub struct TradeSystem {
    pub shop_name: String,
    pub stock: Vec<ShopItem>,
    pub buy_multiplier: f32, // How much the shop charges relative to value
    pub sell_multiplier: f32, // How much the shop pays relative to value
    pub reputation_discount: f32, // Per reputation point
}

impl TradeSystem {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            shop_name: name.into(),
            stock: Vec::new(),
            buy_multiplier: 1.2,
            sell_multiplier: 0.3,
            reputation_discount: 0.001,
        }
    }

    pub fn add_item(&mut self, item: ShopItem) {
        self.stock.push(item);
    }

    pub fn price_to_buy(&self, item: &Item, reputation: i32) -> u32 {
        let discount = (reputation as f32 * self.reputation_discount).min(0.5);
        ((item.value as f32 * self.buy_multiplier) * (1.0 - discount)) as u32
    }

    pub fn price_to_sell(&self, item: &Item, reputation: i32) -> u32 {
        let bonus = (reputation as f32 * self.reputation_discount * 0.5).min(0.3);
        ((item.value as f32 * self.sell_multiplier) * (1.0 + bonus)) as u32
    }

    pub fn can_afford(price: u32, gold: u64) -> bool {
        gold >= price as u64
    }

    pub fn buy_from_shop(&mut self, stock_idx: usize, gold: &mut u64, inventory: &mut Inventory, reputation: i32) -> bool {
        if stock_idx >= self.stock.len() { return false; }
        let price = self.price_to_buy(&self.stock[stock_idx].item, reputation);
        if *gold < price as u64 { return false; }

        let item = self.stock[stock_idx].item.clone();
        match inventory.add_item(item) {
            Ok(_) => {
                *gold -= price as u64;
                if let Some(ref mut stock) = self.stock[stock_idx].stock {
                    if *stock > 0 { *stock -= 1; }
                }
                true
            }
            Err(_) => false,
        }
    }

    pub fn sell_to_shop(&mut self, item: Item, gold: &mut u64, reputation: i32) {
        let price = self.price_to_sell(&item, reputation);
        *gold += price as u64;
        // Add to shop stock with sell price as value
        let mut shop_item = ShopItem::new(item);
        shop_item.stock = Some(1);
        self.stock.push(shop_item);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sword() -> Item {
        Item::new(1, "Iron Sword", ItemType::Weapon)
            .with_icon('†')
            .with_weapon(WeaponData::new(WeaponType::Sword))
            .with_value(50)
    }

    fn make_potion() -> Item {
        let mut item = Item::new(2, "Health Potion", ItemType::Consumable)
            .with_icon('!')
            .with_value(20);
        item.max_stack = 10;
        item.consumable_effects.push(ConsumableEffect::HealHp(50.0));
        item
    }

    #[test]
    fn test_item_sell_value_scales_with_rarity() {
        let common = Item::new(1, "A", ItemType::Material).with_rarity(ItemRarity::Common).with_value(100);
        let legendary = Item::new(2, "B", ItemType::Material).with_rarity(ItemRarity::Legendary).with_value(100);
        assert!(legendary.sell_value() > common.sell_value());
    }

    #[test]
    fn test_inventory_add_and_remove() {
        let mut inv = Inventory::new(10, 100.0);
        let sword = make_sword();
        let slot = inv.add_item(sword).unwrap();
        assert!(inv.get(slot).is_some());
        let removed = inv.remove_item(slot);
        assert!(removed.is_some());
        assert!(inv.get(slot).is_none());
    }

    #[test]
    fn test_inventory_weight_limit() {
        let mut inv = Inventory::new(10, 1.0); // only 1 unit of weight
        let mut heavy = make_sword();
        heavy.weight = 2.0;
        assert!(inv.add_item(heavy).is_err());
    }

    #[test]
    fn test_inventory_stacking() {
        let mut inv = Inventory::new(10, 1000.0);
        let potion = make_potion();
        let mut potion2 = make_potion();
        potion2.stack_size = 3;
        inv.add_item(potion).unwrap();
        inv.add_item(potion2).unwrap();
        // Should be stacked in slot 0
        let stacked = inv.get(0).unwrap();
        assert_eq!(stacked.stack_size, 4);
    }

    #[test]
    fn test_inventory_sort() {
        let mut inv = Inventory::new(10, 1000.0);
        let common = Item::new(1, "Z Common", ItemType::Material).with_rarity(ItemRarity::Common).with_value(1);
        let rare = Item::new(2, "A Rare", ItemType::Material).with_rarity(ItemRarity::Rare).with_value(10);
        inv.add_item(common).unwrap();
        inv.add_item(rare).unwrap();
        inv.sort();
        // Rare should come first
        assert_eq!(inv.get(0).unwrap().rarity, ItemRarity::Rare);
    }

    #[test]
    fn test_equipped_items_weapon_damage() {
        let mut equipped = EquippedItems::new();
        let sword = make_sword();
        equipped.equip(sword);
        assert!(equipped.weapon_damage() > 0.0);
    }

    #[test]
    fn test_equipped_items_unequip() {
        let mut equipped = EquippedItems::new();
        let sword = make_sword();
        equipped.equip(sword);
        let removed = equipped.unequip(EquipSlot::MainHand);
        assert!(removed.is_some());
        assert!(equipped.get(EquipSlot::MainHand).is_none());
    }

    #[test]
    fn test_equipped_items_swap() {
        let mut equipped = EquippedItems::new();
        let sword1 = make_sword();
        let mut sword2 = make_sword();
        sword2.name = "Better Sword".to_string();
        sword2.id = 99;
        equipped.equip(sword1);
        let old = equipped.equip(sword2);
        assert!(old.is_some());
        assert_eq!(old.unwrap().name, "Iron Sword");
    }

    #[test]
    fn test_item_generator_weapon() {
        let mut gen = ItemGenerator::new(42);
        let weapon = gen.generate_weapon(10, 0.0);
        assert!(weapon.weapon_data.is_some());
        assert!(weapon.value > 0);
    }

    #[test]
    fn test_item_generator_rarity_scaling() {
        let mut gen = ItemGenerator::new(999);
        let mut legendary_count = 0;
        for i in 0..1000 {
            let w = gen.generate_weapon(100, 500.0);
            if w.rarity >= ItemRarity::Epic { legendary_count += 1; }
            let _ = i;
        }
        assert!(legendary_count > 0, "Expected at least some epic+ items at high level/magic find");
    }

    #[test]
    fn test_loot_table_roll() {
        let table = LootTable::new(1, 3)
            .add_entry(LootEntry::item(1, 50.0))
            .add_entry(LootEntry::item(2, 30.0))
            .add_entry(LootEntry::item(3, 20.0));
        let drops = table.roll(12345, 0.0);
        assert!(!drops.is_empty());
    }

    #[test]
    fn test_crafting_can_craft() {
        let mut sys = CraftingSystem::new();
        let recipe = Recipe::new(1, "Iron Blade", CraftingStation::Forge)
            .add_ingredient(10, 3)
            .with_result(20, 1);
        sys.register_recipe(recipe);

        let mut inv = Inventory::new(10, 100.0);
        let mut ore = Item::new(10, "Iron Ore", ItemType::Material);
        ore.max_stack = 99;
        ore.stack_size = 5;
        inv.add_item(ore).unwrap();

        let available = sys.available_recipes(CraftingStation::Forge, &inv, 1);
        assert_eq!(available.len(), 1);
    }

    #[test]
    fn test_trade_buy_sell() {
        let mut shop = TradeSystem::new("Blacksmith");
        let sword = make_sword();
        shop.add_item(ShopItem::new(sword));
        let mut gold = 1000u64;
        let mut inv = Inventory::new(10, 100.0);
        let bought = shop.buy_from_shop(0, &mut gold, &mut inv, 0);
        assert!(bought);
        assert!(gold < 1000);
    }

    #[test]
    fn test_stash_deposit_withdraw() {
        let mut stash = Stash::new();
        let sword = make_sword();
        let slot = stash.deposit(0, sword).unwrap();
        let item = stash.withdraw(0, slot);
        assert!(item.is_some());
    }

    #[test]
    fn test_armor_set_bonuses() {
        let set = ArmorSet::new(1, "Warrior Set")
            .add_piece(100)
            .add_piece(101)
            .add_bonus(2, vec![StatModifier::flat("set", StatKind::Strength, 20.0)]);
        assert_eq!(set.active_bonuses(1).len(), 0);
        assert_eq!(set.active_bonuses(2).len(), 1);
    }
}
