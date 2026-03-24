//! Inventory, equipment, and item system.
//!
//! Items are pure data — stats, effects, and metadata. The inventory is
//! a slot-based container with equipment binding and stat aggregation.

use std::collections::HashMap;
use super::{Element, CombatStats};

// ── Rarity ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    pub fn color(self) -> glam::Vec4 {
        use Rarity::*;
        match self {
            Common    => glam::Vec4::new(0.80, 0.80, 0.80, 1.0),
            Uncommon  => glam::Vec4::new(0.30, 0.90, 0.30, 1.0),
            Rare      => glam::Vec4::new(0.20, 0.40, 1.00, 1.0),
            Epic      => glam::Vec4::new(0.65, 0.20, 0.95, 1.0),
            Legendary => glam::Vec4::new(1.00, 0.65, 0.00, 1.0),
            Mythic    => glam::Vec4::new(1.00, 0.10, 0.10, 1.0),
            Unique    => glam::Vec4::new(0.90, 0.75, 0.20, 1.0),
        }
    }

    pub fn glyph(self) -> char {
        use Rarity::*;
        match self {
            Common    => '·',
            Uncommon  => '◆',
            Rare      => '★',
            Epic      => '✦',
            Legendary => '⬡',
            Mythic    => '⚜',
            Unique    => '◉',
        }
    }

    pub fn stat_multiplier(self) -> f32 {
        use Rarity::*;
        match self {
            Common    => 1.00,
            Uncommon  => 1.20,
            Rare      => 1.50,
            Epic      => 1.90,
            Legendary => 2.50,
            Mythic    => 3.50,
            Unique    => 5.00,
        }
    }

    pub fn name(self) -> &'static str {
        use Rarity::*;
        match self {
            Common    => "Common",
            Uncommon  => "Uncommon",
            Rare      => "Rare",
            Epic      => "Epic",
            Legendary => "Legendary",
            Mythic    => "Mythic",
            Unique    => "Unique",
        }
    }
}

// ── ItemCategory ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    Weapon,
    Offhand,
    Helm,
    Chest,
    Legs,
    Boots,
    Gloves,
    Ring,
    Amulet,
    Belt,
    Consumable,
    Material,
    QuestItem,
    Rune,
    Gem,
}

impl ItemCategory {
    pub fn is_equippable(self) -> bool {
        !matches!(self, ItemCategory::Consumable | ItemCategory::Material | ItemCategory::QuestItem)
    }

    pub fn slot(self) -> Option<EquipSlot> {
        use ItemCategory::*;
        match self {
            Weapon    => Some(EquipSlot::MainHand),
            Offhand   => Some(EquipSlot::OffHand),
            Helm      => Some(EquipSlot::Head),
            Chest     => Some(EquipSlot::Chest),
            Legs      => Some(EquipSlot::Legs),
            Boots     => Some(EquipSlot::Feet),
            Gloves    => Some(EquipSlot::Hands),
            Ring      => Some(EquipSlot::Ring1), // simplified: just ring1
            Amulet    => Some(EquipSlot::Neck),
            Belt      => Some(EquipSlot::Belt),
            _         => None,
        }
    }
}

// ── EquipSlot ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Feet,
    Hands,
    Ring1,
    Ring2,
    Neck,
    Belt,
}

impl EquipSlot {
    pub const ALL: &'static [EquipSlot] = &[
        EquipSlot::MainHand, EquipSlot::OffHand, EquipSlot::Head,
        EquipSlot::Chest, EquipSlot::Legs, EquipSlot::Feet,
        EquipSlot::Hands, EquipSlot::Ring1, EquipSlot::Ring2,
        EquipSlot::Neck, EquipSlot::Belt,
    ];

    pub fn name(self) -> &'static str {
        use EquipSlot::*;
        match self {
            MainHand => "Main Hand", OffHand => "Off Hand", Head => "Head",
            Chest    => "Chest",     Legs    => "Legs",     Feet => "Feet",
            Hands    => "Hands",     Ring1   => "Ring 1",   Ring2 => "Ring 2",
            Neck     => "Neck",      Belt    => "Belt",
        }
    }
}

// ── StatModifier ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ModifierType {
    FlatAdd(f32),
    PercentAdd(f32),  // additive percent (e.g., 10% + 15% = 25%)
    PercentMul(f32),  // multiplicative (e.g., 1.1 * 1.15)
    FlatSet(f32),     // override to fixed value
}

#[derive(Debug, Clone)]
pub struct StatModifier {
    pub stat:  StatId,
    pub kind:  ModifierType,
    pub source: String,  // for debug/tooltip
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatId {
    Attack,
    Defense,
    MaxHp,
    HpRegen,
    CritChance,
    CritMult,
    Penetration,
    DodgeChance,
    BlockChance,
    BlockAmount,
    MoveSpeed,
    AttackSpeed,
    SkillCooldown,
    ManaMax,
    ManaRegen,
    EntropyAmp,
}

impl StatId {
    pub fn name(self) -> &'static str {
        use StatId::*;
        match self {
            Attack => "Attack", Defense => "Defense", MaxHp => "Max HP",
            HpRegen => "HP Regen", CritChance => "Crit Chance",
            CritMult => "Crit Multiplier", Penetration => "Penetration",
            DodgeChance => "Dodge Chance", BlockChance => "Block Chance",
            BlockAmount => "Block Amount", MoveSpeed => "Move Speed",
            AttackSpeed => "Attack Speed", SkillCooldown => "Skill Cooldown",
            ManaMax => "Max Mana", ManaRegen => "Mana Regen",
            EntropyAmp => "Entropy Amplification",
        }
    }
}

// ── ItemAffix ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ItemAffix {
    pub name:      String,
    pub modifiers: Vec<StatModifier>,
    pub is_prefix: bool,
}

impl ItemAffix {
    pub fn new(name: impl Into<String>, is_prefix: bool) -> Self {
        Self { name: name.into(), modifiers: Vec::new(), is_prefix }
    }

    pub fn add_modifier(mut self, stat: StatId, kind: ModifierType) -> Self {
        self.modifiers.push(StatModifier { stat, kind, source: self.name.clone() });
        self
    }

    // Common affix presets
    pub fn of_the_berserker() -> Self {
        Self::new("of the Berserker", false)
            .add_modifier(StatId::Attack, ModifierType::PercentAdd(0.25))
            .add_modifier(StatId::CritChance, ModifierType::FlatAdd(0.08))
    }

    pub fn of_fortification() -> Self {
        Self::new("of Fortification", false)
            .add_modifier(StatId::Defense, ModifierType::PercentAdd(0.30))
            .add_modifier(StatId::MaxHp, ModifierType::PercentAdd(0.15))
    }

    pub fn enchanted() -> Self {
        Self::new("Enchanted", true)
            .add_modifier(StatId::EntropyAmp, ModifierType::PercentAdd(0.20))
    }

    pub fn swift() -> Self {
        Self::new("Swift", true)
            .add_modifier(StatId::MoveSpeed, ModifierType::PercentAdd(0.15))
            .add_modifier(StatId::AttackSpeed, ModifierType::PercentAdd(0.10))
    }

    pub fn arcane() -> Self {
        Self::new("Arcane", true)
            .add_modifier(StatId::ManaMax, ModifierType::FlatAdd(50.0))
            .add_modifier(StatId::ManaRegen, ModifierType::PercentAdd(0.20))
    }
}

// ── ItemEffect ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ItemEffect {
    /// On hit: apply elemental damage
    OnHitElement { element: Element, amount: f32 },
    /// On kill: restore HP
    OnKillHeal { amount: f32 },
    /// On low HP (<threshold): gain damage boost
    LowHpBoost { threshold: f32, multiplier: f32 },
    /// Proc chance effect
    ProcOnHit { chance: f32, effect: Box<ItemEffect> },
    /// Aura: affect nearby allies
    Aura { radius: f32, stat: StatId, value: f32 },
    /// Thorns: reflect % of damage received
    Thorns { reflect_pct: f32 },
    /// Lifesteal
    Lifesteal { pct: f32 },
    /// On crit: apply status
    OnCritStatus { status_name: String, duration: f32 },
}

// ── Item ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Item {
    pub id:          u64,
    pub name:        String,
    pub description: String,
    pub category:    ItemCategory,
    pub rarity:      Rarity,
    pub level:       u32,
    pub base_stats:  Vec<StatModifier>,
    pub affixes:     Vec<ItemAffix>,
    pub effects:     Vec<ItemEffect>,
    pub stack_size:  u32,
    pub max_stack:   u32,
    pub value:       u64,     // gold value
    pub weight:      f32,
    pub glyph:       char,
    pub sockets:     Vec<Option<Item>>,  // gem sockets
    pub element:     Option<Element>,
    pub set_id:      Option<u32>,        // item set membership
}

impl Item {
    pub fn new(name: impl Into<String>, category: ItemCategory, rarity: Rarity, level: u32) -> Self {
        let name = name.into();
        let glyph = match category {
            ItemCategory::Weapon   => '⚔',
            ItemCategory::Offhand  => '🛡',
            ItemCategory::Helm     => '⛑',
            ItemCategory::Chest    => '🛡',
            ItemCategory::Boots    => '👟',
            ItemCategory::Ring     => '◎',
            ItemCategory::Amulet   => '◉',
            ItemCategory::Gem      => '◆',
            ItemCategory::Rune     => '✦',
            _                      => '·',
        };
        Item {
            id: 0,
            name,
            description: String::new(),
            category,
            rarity,
            level,
            base_stats: Vec::new(),
            affixes: Vec::new(),
            effects: Vec::new(),
            stack_size: 1,
            max_stack: if matches!(category, ItemCategory::Consumable | ItemCategory::Material) { 99 } else { 1 },
            value: (level as u64 * 10) * rarity.stat_multiplier() as u64,
            weight: 1.0,
            glyph,
            sockets: Vec::new(),
            element: None,
            set_id: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_stat(mut self, stat: StatId, kind: ModifierType) -> Self {
        self.base_stats.push(StatModifier { stat, kind, source: self.name.clone() });
        self
    }

    pub fn with_affix(mut self, affix: ItemAffix) -> Self {
        self.affixes.push(affix);
        self
    }

    pub fn with_effect(mut self, effect: ItemEffect) -> Self {
        self.effects.push(effect);
        self
    }

    pub fn with_element(mut self, el: Element) -> Self {
        self.element = Some(el);
        self
    }

    pub fn add_socket(mut self) -> Self {
        self.sockets.push(None);
        self
    }

    pub fn socket_gem(&mut self, slot: usize, gem: Item) -> bool {
        if slot < self.sockets.len() {
            self.sockets[slot] = Some(gem);
            true
        } else {
            false
        }
    }

    pub fn display_name(&self) -> String {
        let prefix = self.affixes.iter()
            .find(|a| a.is_prefix)
            .map(|a| format!("{} ", a.name))
            .unwrap_or_default();
        let suffix = self.affixes.iter()
            .find(|a| !a.is_prefix)
            .map(|a| format!(" {}", a.name))
            .unwrap_or_default();
        format!("{}{}{}", prefix, self.name, suffix)
    }

    pub fn all_modifiers(&self) -> Vec<&StatModifier> {
        let mut mods: Vec<&StatModifier> = self.base_stats.iter().collect();
        for affix in &self.affixes {
            mods.extend(affix.modifiers.iter());
        }
        for socket in &self.sockets {
            if let Some(gem) = socket {
                mods.extend(gem.base_stats.iter());
            }
        }
        mods
    }

    pub fn tooltip(&self) -> String {
        let mut lines = vec![
            format!("{} [{}]", self.display_name(), self.rarity.name()),
            format!("Level {} {} | {} glyph", self.level, format!("{:?}", self.category), self.glyph),
        ];
        if !self.description.is_empty() {
            lines.push(self.description.clone());
        }
        lines.push(String::from("---"));
        for m in self.all_modifiers() {
            let val_str = match &m.kind {
                ModifierType::FlatAdd(v)    => format!("+{:.0}", v),
                ModifierType::PercentAdd(v) => format!("+{:.0}%", v * 100.0),
                ModifierType::PercentMul(v) => format!("×{:.2}", v),
                ModifierType::FlatSet(v)    => format!("={:.0}", v),
            };
            lines.push(format!("  {} {}", val_str, m.stat.name()));
        }
        if !self.effects.is_empty() {
            lines.push("Effects:".to_string());
            for eff in &self.effects {
                lines.push(format!("  {:?}", eff));
            }
        }
        lines.push(format!("Value: {} gold | Weight: {:.1}", self.value, self.weight));
        lines.join("\n")
    }

    /// Compute net combat stat bonus for a specific stat.
    pub fn stat_bonus(&self, stat: StatId, base: f32) -> f32 {
        let mut flat   = 0.0f32;
        let mut pct_add = 0.0f32;
        let mut pct_mul = 1.0f32;
        let mut override_val: Option<f32> = None;
        for m in self.all_modifiers() {
            if m.stat == stat {
                match m.kind {
                    ModifierType::FlatAdd(v)    => flat += v,
                    ModifierType::PercentAdd(v) => pct_add += v,
                    ModifierType::PercentMul(v) => pct_mul *= 1.0 + v,
                    ModifierType::FlatSet(v)    => override_val = Some(v),
                }
            }
        }
        if let Some(v) = override_val { return v; }
        (base + flat) * (1.0 + pct_add) * pct_mul
    }
}

// ── ItemStack ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ItemStack {
    pub item:     Item,
    pub quantity: u32,
}

impl ItemStack {
    pub fn single(item: Item) -> Self {
        Self { item, quantity: 1 }
    }

    pub fn stacked(item: Item, qty: u32) -> Self {
        let qty = qty.min(item.max_stack);
        Self { item, quantity: qty }
    }

    pub fn can_stack_with(&self, other: &ItemStack) -> bool {
        self.item.id == other.item.id && self.item.max_stack > 1
    }

    pub fn add_to_stack(&mut self, qty: u32) -> u32 {
        let space = self.item.max_stack - self.quantity;
        let added = qty.min(space);
        self.quantity += added;
        qty - added  // leftover
    }

    pub fn remove_from_stack(&mut self, qty: u32) -> u32 {
        let removed = qty.min(self.quantity);
        self.quantity -= removed;
        removed
    }

    pub fn is_empty(&self) -> bool { self.quantity == 0 }
}

// ── Inventory ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Inventory {
    slots:     Vec<Option<ItemStack>>,
    capacity:  usize,
    gold:      u64,
    weight:    f32,
    max_weight: f32,
}

impl Inventory {
    pub fn new(capacity: usize, max_weight: f32) -> Self {
        Inventory {
            slots: vec![None; capacity],
            capacity,
            gold: 0,
            weight: 0.0,
            max_weight,
        }
    }

    pub fn add_item(&mut self, item: Item) -> bool {
        if self.weight + item.weight > self.max_weight {
            return false;
        }
        // Try stacking first
        if item.max_stack > 1 {
            for slot in self.slots.iter_mut().flatten() {
                if slot.item.id == item.id {
                    let leftover = slot.add_to_stack(1);
                    if leftover == 0 {
                        self.weight += item.weight;
                        return true;
                    }
                }
            }
        }
        // Find empty slot
        for slot in &mut self.slots {
            if slot.is_none() {
                self.weight += item.weight;
                *slot = Some(ItemStack::single(item));
                return true;
            }
        }
        false  // No room
    }

    pub fn remove_item(&mut self, slot_idx: usize) -> Option<Item> {
        if slot_idx >= self.capacity { return None; }
        let stack = self.slots[slot_idx].take()?;
        self.weight -= stack.item.weight;
        Some(stack.item)
    }

    pub fn remove_count(&mut self, slot_idx: usize, count: u32) -> u32 {
        if slot_idx >= self.capacity { return 0; }
        if let Some(stack) = &mut self.slots[slot_idx] {
            let removed = stack.remove_from_stack(count);
            self.weight -= stack.item.weight * removed as f32;
            if stack.is_empty() { self.slots[slot_idx] = None; }
            return removed;
        }
        0
    }

    pub fn get(&self, slot_idx: usize) -> Option<&ItemStack> {
        self.slots.get(slot_idx)?.as_ref()
    }

    pub fn get_mut(&mut self, slot_idx: usize) -> Option<&mut ItemStack> {
        self.slots.get_mut(slot_idx)?.as_mut()
    }

    pub fn find_item_by_id(&self, id: u64) -> Option<usize> {
        self.slots.iter().position(|s| s.as_ref().map(|st| st.item.id == id).unwrap_or(false))
    }

    pub fn find_items_by_category(&self, cat: ItemCategory) -> Vec<usize> {
        self.slots.iter().enumerate()
            .filter(|(_, s)| s.as_ref().map(|st| st.item.category == cat).unwrap_or(false))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn used_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    pub fn free_slots(&self) -> usize {
        self.capacity - self.used_slots()
    }

    pub fn is_full(&self) -> bool { self.free_slots() == 0 }

    pub fn total_value(&self) -> u64 {
        self.slots.iter().flatten()
            .map(|s| s.item.value * s.quantity as u64)
            .sum()
    }

    pub fn add_gold(&mut self, amount: u64) { self.gold += amount; }
    pub fn remove_gold(&mut self, amount: u64) -> bool {
        if self.gold >= amount { self.gold -= amount; true } else { false }
    }
    pub fn gold(&self) -> u64 { self.gold }
    pub fn weight(&self) -> f32 { self.weight }
    pub fn max_weight(&self) -> f32 { self.max_weight }
    pub fn capacity(&self) -> usize { self.capacity }

    /// Sort inventory: by rarity desc, then by level desc.
    pub fn sort(&mut self) {
        let mut items: Vec<ItemStack> = self.slots.iter_mut()
            .filter_map(|s| s.take())
            .collect();
        items.sort_by(|a, b| {
            b.item.rarity.cmp(&a.item.rarity)
                .then(b.item.level.cmp(&a.item.level))
                .then(a.item.name.cmp(&b.item.name))
        });
        for (i, item) in items.into_iter().enumerate() {
            if i < self.capacity { self.slots[i] = Some(item); }
        }
    }

    /// Transfer item from this inventory to another.
    pub fn transfer(&mut self, slot_idx: usize, target: &mut Inventory) -> bool {
        if let Some(item) = self.remove_item(slot_idx) {
            if target.add_item(item.clone()) {
                return true;
            }
            // Put back if transfer failed
            let _ = self.add_item(item);
        }
        false
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &ItemStack)> {
        self.slots.iter().enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|st| (i, st)))
    }
}

// ── Equipment ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct Equipment {
    pub slots: HashMap<EquipSlot, Item>,
}

impl Equipment {
    pub fn new() -> Self {
        Self { slots: HashMap::new() }
    }

    pub fn equip(&mut self, item: Item) -> Option<Item> {
        let slot = item.category.slot()?;
        let old = self.slots.remove(&slot);
        self.slots.insert(slot, item);
        old
    }

    pub fn unequip(&mut self, slot: EquipSlot) -> Option<Item> {
        self.slots.remove(&slot)
    }

    pub fn get(&self, slot: EquipSlot) -> Option<&Item> {
        self.slots.get(&slot)
    }

    pub fn is_slot_filled(&self, slot: EquipSlot) -> bool {
        self.slots.contains_key(&slot)
    }

    /// Aggregate all stat modifiers across all equipped items.
    pub fn all_modifiers(&self) -> Vec<&StatModifier> {
        self.slots.values()
            .flat_map(|item| item.all_modifiers())
            .collect()
    }

    /// Apply equipment bonuses to a CombatStats block.
    pub fn apply_to_stats(&self, base: &CombatStats) -> CombatStats {
        let mut result = base.clone();
        let mods = self.all_modifiers();
        result.attack       = apply_mods(&mods, StatId::Attack,       base.attack);
        result.armor        = apply_mods(&mods, StatId::Defense,      base.armor);
        result.max_hp       = apply_mods(&mods, StatId::MaxHp,        base.max_hp);
        result.crit_chance  = apply_mods(&mods, StatId::CritChance,   base.crit_chance);
        result.crit_mult    = apply_mods(&mods, StatId::CritMult,     base.crit_mult);
        result.penetration  = apply_mods(&mods, StatId::Penetration,  base.penetration);
        result.dodge_chance = apply_mods(&mods, StatId::DodgeChance,  base.dodge_chance);
        result.entropy_amp  = apply_mods(&mods, StatId::EntropyAmp,   base.entropy_amp);
        result
    }

    /// Total weight of all equipped items.
    pub fn total_weight(&self) -> f32 {
        self.slots.values().map(|i| i.weight).sum()
    }

    /// Display equipped items summary.
    pub fn summary(&self) -> Vec<String> {
        EquipSlot::ALL.iter().map(|&slot| {
            if let Some(item) = self.slots.get(&slot) {
                format!("[{}] {} ({})", slot.name(), item.display_name(), item.rarity.name())
            } else {
                format!("[{}] — empty —", slot.name())
            }
        }).collect()
    }
}

fn apply_mods(mods: &[&StatModifier], target: StatId, base: f32) -> f32 {
    let mut flat   = 0.0f32;
    let mut pct_add = 0.0f32;
    let mut pct_mul = 1.0f32;
    let mut override_val: Option<f32> = None;
    for m in mods {
        if m.stat == target {
            match m.kind {
                ModifierType::FlatAdd(v)    => flat += v,
                ModifierType::PercentAdd(v) => pct_add += v,
                ModifierType::PercentMul(v) => pct_mul *= 1.0 + v,
                ModifierType::FlatSet(v)    => override_val = Some(v),
            }
        }
    }
    if let Some(v) = override_val { return v; }
    (base + flat) * (1.0 + pct_add) * pct_mul
}

// ── LootTable ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LootEntry {
    pub item_builder: fn(u32) -> Item,  // fn(level) -> Item
    pub weight:       f32,
    pub min_qty:      u32,
    pub max_qty:      u32,
    pub min_level:    u32,
    pub requires_rare_roll: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LootTable {
    pub entries:     Vec<LootEntry>,
    pub gold_min:    u64,
    pub gold_max:    u64,
    pub drop_chance: f32,  // [0,1] — chance any loot drops at all
    pub item_count_min: u32,
    pub item_count_max: u32,
}

impl LootTable {
    pub fn new() -> Self {
        LootTable {
            entries: Vec::new(),
            gold_min: 0,
            gold_max: 0,
            drop_chance: 1.0,
            item_count_min: 1,
            item_count_max: 3,
        }
    }

    pub fn add_entry(&mut self, builder: fn(u32) -> Item, weight: f32, min_qty: u32, max_qty: u32) {
        self.entries.push(LootEntry {
            item_builder: builder, weight, min_qty, max_qty,
            min_level: 0, requires_rare_roll: false,
        });
    }

    pub fn set_gold(&mut self, min: u64, max: u64) {
        self.gold_min = min;
        self.gold_max = max;
    }

    /// Roll the loot table with a pseudo-random seed. Returns items and gold.
    pub fn roll(&self, level: u32, luck_bonus: f32, seed: u64) -> LootDrop {
        let mut rng = SimpleRng::new(seed);

        if rng.next_f32() > self.drop_chance * (1.0 + luck_bonus) {
            return LootDrop::empty();
        }

        let gold = if self.gold_max > 0 {
            self.gold_min + rng.next_u64() % (self.gold_max - self.gold_min + 1)
        } else { 0 };

        let count = if self.item_count_max > self.item_count_min {
            self.item_count_min + (rng.next_u32() % (self.item_count_max - self.item_count_min + 1))
        } else {
            self.item_count_min
        };

        let total_weight: f32 = self.entries.iter()
            .filter(|e| level >= e.min_level)
            .map(|e| e.weight)
            .sum();

        let mut items = Vec::new();
        for _ in 0..count {
            if total_weight <= 0.0 { break; }
            let roll = rng.next_f32() * total_weight;
            let mut accum = 0.0;
            for entry in &self.entries {
                if level < entry.min_level { continue; }
                accum += entry.weight;
                if roll <= accum {
                    let qty = if entry.max_qty > entry.min_qty {
                        entry.min_qty + rng.next_u32() % (entry.max_qty - entry.min_qty + 1)
                    } else {
                        entry.min_qty
                    };
                    for _ in 0..qty {
                        let mut item = (entry.item_builder)(level);
                        item.id = rng.next_u64();
                        items.push(item);
                    }
                    break;
                }
            }
        }

        LootDrop { items, gold }
    }
}

#[derive(Debug, Clone)]
pub struct LootDrop {
    pub items: Vec<Item>,
    pub gold:  u64,
}

impl LootDrop {
    pub fn empty() -> Self { Self { items: Vec::new(), gold: 0 } }
    pub fn is_empty(&self) -> bool { self.items.is_empty() && self.gold == 0 }
}

// ── SimpleRng (for loot rolls) ─────────────────────────────────────────────────

struct SimpleRng { state: u64 }

impl SimpleRng {
    fn new(seed: u64) -> Self { Self { state: seed ^ 0x6a09e667f3bcc908 } }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    fn next_u32(&mut self) -> u32 { (self.next_u64() >> 32) as u32 }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sword(level: u32) -> Item {
        Item::new("Iron Sword", ItemCategory::Weapon, Rarity::Common, level)
            .with_stat(StatId::Attack, ModifierType::FlatAdd(10.0))
    }

    #[test]
    fn test_item_display_name() {
        let item = Item::new("Blade", ItemCategory::Weapon, Rarity::Rare, 10)
            .with_affix(ItemAffix::swift())
            .with_affix(ItemAffix::of_the_berserker());
        assert!(item.display_name().contains("Swift"));
        assert!(item.display_name().contains("Berserker"));
    }

    #[test]
    fn test_inventory_add_remove() {
        let mut inv = Inventory::new(10, 100.0);
        let sword = make_sword(1);
        assert!(inv.add_item(sword));
        assert_eq!(inv.used_slots(), 1);
        let removed = inv.remove_item(0);
        assert!(removed.is_some());
        assert_eq!(inv.used_slots(), 0);
    }

    #[test]
    fn test_equipment_stat_apply() {
        let mut equip = Equipment::new();
        let item = Item::new("Power Helm", ItemCategory::Helm, Rarity::Rare, 5)
            .with_stat(StatId::MaxHp, ModifierType::FlatAdd(100.0))
            .with_stat(StatId::Defense, ModifierType::PercentAdd(0.20));
        equip.equip(item);

        let base = CombatStats::default();
        let boosted = equip.apply_to_stats(&base);
        assert!(boosted.max_hp > base.max_hp);
        assert!(boosted.armor >= base.armor);
    }

    #[test]
    fn test_loot_table_roll() {
        let mut table = LootTable::new();
        table.add_entry(|lvl| make_sword(lvl), 1.0, 1, 1);
        table.set_gold(10, 50);
        let drop = table.roll(5, 0.0, 12345);
        // Either got items or didn't (RNG dependent), just check it doesn't panic
        let _ = drop.is_empty();
    }

    #[test]
    fn test_rarity_ordering() {
        assert!(Rarity::Legendary > Rarity::Rare);
        assert!(Rarity::Common < Rarity::Epic);
    }
}
