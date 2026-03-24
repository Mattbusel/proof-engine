//! Character equipment system.
//!
//! Provides:
//! - [`EquipSlot`] — the 16 body slots a character can wear items in.
//! - [`Equipment`] — the active equipment state with cached stat bonuses.
//! - [`SetBonus`] / [`SetBonusRegistry`] — track gear-set completions.
//! - [`Loadout`] / [`LoadoutManager`] — named equipment snapshots.
//! - [`DurabilitySystem`] — tick-based wear and repair.

use std::collections::HashMap;

use super::{
    ItemId, ItemInstance, ItemCategory, StatKind,
    container::Inventory,
};

// ── EquipSlot ──────────────────────────────────────────────────────────────────

/// The 16 body locations an item can be worn or wielded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    Head,
    Chest,
    Legs,
    Feet,
    Hands,
    Shoulder,
    Back,
    Ring1,
    Ring2,
    Neck,
    MainHand,
    OffHand,
    TwoHand,
    Ranged,
    Trinket1,
    Trinket2,
}

impl EquipSlot {
    /// Human-readable name.
    pub fn display_name(self) -> &'static str {
        match self {
            EquipSlot::Head     => "Head",
            EquipSlot::Chest    => "Chest",
            EquipSlot::Legs     => "Legs",
            EquipSlot::Feet     => "Feet",
            EquipSlot::Hands    => "Hands",
            EquipSlot::Shoulder => "Shoulder",
            EquipSlot::Back     => "Back",
            EquipSlot::Ring1    => "Ring (left)",
            EquipSlot::Ring2    => "Ring (right)",
            EquipSlot::Neck     => "Neck",
            EquipSlot::MainHand => "Main Hand",
            EquipSlot::OffHand  => "Off Hand",
            EquipSlot::TwoHand  => "Two-Hand",
            EquipSlot::Ranged   => "Ranged",
            EquipSlot::Trinket1 => "Trinket (1)",
            EquipSlot::Trinket2 => "Trinket (2)",
        }
    }

    /// All slots in a stable iteration order.
    pub fn all() -> &'static [EquipSlot] {
        use EquipSlot::*;
        &[
            Head, Chest, Legs, Feet, Hands, Shoulder, Back,
            Ring1, Ring2, Neck,
            MainHand, OffHand, TwoHand, Ranged,
            Trinket1, Trinket2,
        ]
    }

    /// Whether this slot is a weapon/hand slot.
    pub fn is_hand_slot(self) -> bool {
        matches!(self, EquipSlot::MainHand | EquipSlot::OffHand | EquipSlot::TwoHand | EquipSlot::Ranged)
    }

    /// Whether this slot is a ring or neck slot (accessory).
    pub fn is_accessory_slot(self) -> bool {
        matches!(self, EquipSlot::Ring1 | EquipSlot::Ring2 | EquipSlot::Neck | EquipSlot::Trinket1 | EquipSlot::Trinket2)
    }

    /// Whether this slot is part of the armor set (body pieces).
    pub fn is_armor_slot(self) -> bool {
        matches!(self, EquipSlot::Head | EquipSlot::Chest | EquipSlot::Legs | EquipSlot::Feet | EquipSlot::Hands | EquipSlot::Shoulder | EquipSlot::Back)
    }
}

impl std::fmt::Display for EquipSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

// ── EquipRestriction ───────────────────────────────────────────────────────────

/// Requirements a character must satisfy to equip a specific item.
#[derive(Debug, Clone, Default)]
pub struct EquipRestriction {
    /// Minimum character level.
    pub min_level: u32,
    /// Required stat minimums: (stat, minimum value).
    pub required_stats: Vec<(StatKind, u32)>,
    /// If set, only the listed class names may equip this item.
    pub allowed_classes: Option<Vec<String>>,
}

impl EquipRestriction {
    pub fn new() -> Self { Self::default() }

    pub fn with_min_level(mut self, level: u32) -> Self {
        self.min_level = level; self
    }

    pub fn require_stat(mut self, stat: StatKind, min: u32) -> Self {
        self.required_stats.push((stat, min)); self
    }

    pub fn with_allowed_classes(mut self, classes: Vec<String>) -> Self {
        self.allowed_classes = Some(classes); self
    }

    /// Check whether a character with the given parameters satisfies this restriction.
    ///
    /// `stat_fn` returns the character's current value for a given stat.
    pub fn check(
        &self,
        char_level: u32,
        char_class: &str,
        stat_fn: &dyn Fn(StatKind) -> u32,
    ) -> Result<(), EquipError> {
        if char_level < self.min_level {
            return Err(EquipError::LevelRequirement { required: self.min_level, have: char_level });
        }
        for &(stat, min) in &self.required_stats {
            let val = stat_fn(stat);
            if val < min {
                return Err(EquipError::StatRequirement { stat, required: min, have: val });
            }
        }
        if let Some(classes) = &self.allowed_classes {
            if !classes.iter().any(|c| c.as_str() == char_class) {
                return Err(EquipError::ClassRestriction {
                    class: char_class.to_string(),
                    allowed: classes.clone(),
                });
            }
        }
        Ok(())
    }
}

// ── EquipError ─────────────────────────────────────────────────────────────────

/// Ways equip/unequip operations can fail.
#[derive(Debug, Clone, PartialEq)]
pub enum EquipError {
    /// The target slot is already occupied (and no swap was requested).
    SlotOccupied,
    /// Character level too low.
    LevelRequirement { required: u32, have: u32 },
    /// A stat minimum was not met.
    StatRequirement { stat: StatKind, required: u32, have: u32 },
    /// Wrong class.
    ClassRestriction { class: String, allowed: Vec<String> },
    /// The item is not a valid equippable.
    ItemNotEquippable,
    /// Two-hand / off-hand conflict.
    ConflictingSlot,
    /// An inventory operation underlying the equip failed.
    InventoryError(String),
}

impl std::fmt::Display for EquipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EquipError::SlotOccupied =>
                write!(f, "slot is already occupied"),
            EquipError::LevelRequirement { required, have } =>
                write!(f, "level {} required (have {})", required, have),
            EquipError::StatRequirement { stat, required, have } =>
                write!(f, "{} {} required (have {})", stat, required, have),
            EquipError::ClassRestriction { class, .. } =>
                write!(f, "class '{}' cannot equip this item", class),
            EquipError::ItemNotEquippable =>
                write!(f, "item is not equippable"),
            EquipError::ConflictingSlot =>
                write!(f, "conflicting weapon slot (two-hand vs one-hand)"),
            EquipError::InventoryError(msg) =>
                write!(f, "inventory error: {}", msg),
        }
    }
}

// ── ItemStatProvider ──────────────────────────────────────────────────────────

/// Minimum info the equipment system needs from the item database.
#[derive(Debug, Clone)]
pub struct ItemEquipInfo {
    pub item_id:     ItemId,
    pub category:    ItemCategory,
    /// The slot this item type is intended for.
    pub equip_slot:  EquipSlot,
    /// Maximum durability (None → indestructible).
    pub max_durability: Option<f32>,
    /// Flat stat bonuses granted when worn: Vec<(stat, value)>.
    pub stat_bonuses: Vec<(StatKind, f32)>,
    /// Which gear-set this item belongs to (None → not part of a set).
    pub set_id: Option<u32>,
    /// Equip requirements; None → no restrictions.
    pub restriction: Option<EquipRestriction>,
    pub weight: f32,
}

impl ItemEquipInfo {
    pub fn new(item_id: ItemId, category: ItemCategory, equip_slot: EquipSlot) -> Self {
        Self {
            item_id,
            category,
            equip_slot,
            max_durability: None,
            stat_bonuses: Vec::new(),
            set_id: None,
            restriction: None,
            weight: 1.0,
        }
    }

    pub fn with_max_durability(mut self, d: f32) -> Self { self.max_durability = Some(d); self }
    pub fn with_stat(mut self, stat: StatKind, val: f32) -> Self { self.stat_bonuses.push((stat, val)); self }
    pub fn with_set(mut self, id: u32) -> Self { self.set_id = Some(id); self }
    pub fn with_restriction(mut self, r: EquipRestriction) -> Self { self.restriction = Some(r); self }
    pub fn with_weight(mut self, w: f32) -> Self { self.weight = w; self }
}

// ── EquippedItem ───────────────────────────────────────────────────────────────

/// An item currently occupying an equipment slot.
#[derive(Debug, Clone)]
pub struct EquippedItem {
    pub instance:       ItemInstance,
    pub slot:           EquipSlot,
    /// Whether this item's bonuses have been applied to the stat cache.
    pub bonuses_applied: bool,
}

impl EquippedItem {
    pub fn new(instance: ItemInstance, slot: EquipSlot) -> Self {
        Self { instance, slot, bonuses_applied: false }
    }
}

// ── Equipment ─────────────────────────────────────────────────────────────────

/// The set of items currently worn by a character.
///
/// Maintains a dirty flag so that stat recalculation is only performed when
/// the loadout actually changes.
#[derive(Debug, Clone)]
pub struct Equipment {
    slots:      HashMap<EquipSlot, EquippedItem>,
    stat_cache: HashMap<StatKind, f32>,
    dirty:      bool,
}

impl Equipment {
    pub fn new() -> Self {
        Self {
            slots:      HashMap::new(),
            stat_cache: HashMap::new(),
            dirty:      false,
        }
    }

    // ── Read queries ───────────────────────────────────────────────────────────

    pub fn get(&self, slot: EquipSlot) -> Option<&EquippedItem> {
        self.slots.get(&slot)
    }

    pub fn get_mut(&mut self, slot: EquipSlot) -> Option<&mut EquippedItem> {
        self.slots.get_mut(&slot)
    }

    pub fn is_slot_filled(&self, slot: EquipSlot) -> bool {
        self.slots.contains_key(&slot)
    }

    /// Total weight of all equipped items, given a per-item weight lookup.
    pub fn total_weight<F>(&self, weight_fn: F) -> f32
    where F: Fn(ItemId) -> f32,
    {
        self.slots.values()
            .map(|e| weight_fn(e.instance.def_id))
            .sum()
    }

    /// Iterate all occupied slots.
    pub fn iter(&self) -> impl Iterator<Item = (EquipSlot, &EquippedItem)> {
        self.slots.iter().map(|(&s, e)| (s, e))
    }

    /// Collect all currently equipped item ids.
    pub fn equipped_item_ids(&self) -> Vec<ItemId> {
        self.slots.values().map(|e| e.instance.def_id).collect()
    }

    // ── Stat cache ─────────────────────────────────────────────────────────────

    /// Get the total bonus for a stat from all equipped items.
    ///
    /// Returns 0.0 if stats are dirty (call [`recalculate_stats`] first).
    pub fn get_stat_bonus(&self, stat: StatKind) -> f32 {
        *self.stat_cache.get(&stat).unwrap_or(&0.0)
    }

    /// Recompute all stat bonuses from scratch using the item info provider.
    ///
    /// `info_fn` maps ItemId → ItemEquipInfo.
    pub fn recalculate_stats<F>(&mut self, info_fn: F)
    where F: Fn(ItemId) -> Option<ItemEquipInfo>,
    {
        let mut cache: HashMap<StatKind, f32> = HashMap::new();

        for equip in self.slots.values_mut() {
            let inst = &equip.instance;
            // Item database bonuses.
            if let Some(info) = info_fn(inst.def_id) {
                let quality = inst.durability_fraction();
                for (stat, val) in &info.stat_bonuses {
                    *cache.entry(*stat).or_insert(0.0) += val * quality;
                }
            }
            // Enchantment bonuses.
            for ench in &inst.enchantments {
                *cache.entry(ench.stat).or_insert(0.0) += ench.magnitude;
            }
            equip.bonuses_applied = true;
        }

        self.stat_cache = cache;
        self.dirty = false;
    }

    pub fn is_dirty(&self) -> bool { self.dirty }

    pub fn mark_dirty(&mut self) { self.dirty = true; }

    // ── Two-hand check ─────────────────────────────────────────────────────────

    /// Enforce the two-hand weapon rule:
    /// - Equipping `TwoHand` clears `MainHand` and `OffHand`.
    /// - Equipping `MainHand` or `OffHand` clears `TwoHand`.
    fn apply_two_hand_rule(&mut self, slot: EquipSlot) -> Vec<ItemInstance> {
        let mut displaced = Vec::new();
        match slot {
            EquipSlot::TwoHand => {
                if let Some(e) = self.slots.remove(&EquipSlot::MainHand) { displaced.push(e.instance); }
                if let Some(e) = self.slots.remove(&EquipSlot::OffHand)  { displaced.push(e.instance); }
            }
            EquipSlot::MainHand | EquipSlot::OffHand => {
                if let Some(e) = self.slots.remove(&EquipSlot::TwoHand) { displaced.push(e.instance); }
            }
            _ => {}
        }
        displaced
    }

    // ── can_equip ─────────────────────────────────────────────────────────────

    /// Check (without modifying state) whether `item` can be equipped in `slot`.
    pub fn can_equip(
        &self,
        slot:       EquipSlot,
        info:       &ItemEquipInfo,
        char_level: u32,
        char_class: &str,
        stat_fn:    &dyn Fn(StatKind) -> u32,
    ) -> Result<(), EquipError> {
        if !info.category.is_equippable() && !matches!(info.category, ItemCategory::Misc) {
            // We allow Misc through so accessories (rings/neck) work.
        }

        if let Some(r) = &info.restriction {
            r.check(char_level, char_class, stat_fn)?;
        }

        // TwoHand conflict: can't equip TwoHand if TwoHand slot is taken by a
        // one-hand weapon in offhand that we can't displace.
        // (The real conflict resolution is done in `equip`; here we just check level/stat.)
        Ok(())
    }

    // ── equip / unequip ────────────────────────────────────────────────────────

    /// Equip an item in the given slot.
    ///
    /// - If `slot` already has an item, it is displaced and returned.
    /// - Two-hand rules are enforced: any conflicting hand items are also returned.
    /// - The stat cache is marked dirty; call [`recalculate_stats`] to refresh.
    ///
    /// `info` must be provided by the caller from the item database.
    /// `char_level` / `char_class` / `stat_fn` are used for restriction checks.
    pub fn equip(
        &mut self,
        slot:       EquipSlot,
        item:       ItemInstance,
        info:       &ItemEquipInfo,
        char_level: u32,
        char_class: &str,
        stat_fn:    &dyn Fn(StatKind) -> u32,
    ) -> Result<Vec<ItemInstance>, EquipError> {
        // Restriction check.
        if let Some(r) = &info.restriction {
            r.check(char_level, char_class, stat_fn)?;
        }

        // Validate slot matches item type.
        if info.equip_slot != slot {
            // Allow TwoHand item → MainHand slot as a special case.
            if !(info.equip_slot == EquipSlot::TwoHand && slot == EquipSlot::MainHand) {
                return Err(EquipError::ConflictingSlot);
            }
        }

        let mut displaced = self.apply_two_hand_rule(slot);

        // Displace existing occupant.
        if let Some(old) = self.slots.remove(&slot) {
            displaced.push(old.instance);
        }

        self.slots.insert(slot, EquippedItem::new(item, slot));
        self.dirty = true;
        Ok(displaced)
    }

    /// Unequip the item in `slot`, returning it.
    pub fn unequip(&mut self, slot: EquipSlot) -> Option<ItemInstance> {
        let removed = self.slots.remove(&slot).map(|e| e.instance);
        if removed.is_some() { self.dirty = true; }
        removed
    }

    /// Remove all equipment, returning all items.
    pub fn unequip_all(&mut self) -> Vec<(EquipSlot, ItemInstance)> {
        let items: Vec<(EquipSlot, ItemInstance)> = self.slots.drain()
            .map(|(slot, e)| (slot, e.instance))
            .collect();
        if !items.is_empty() { self.dirty = true; }
        items
    }
}

impl Default for Equipment {
    fn default() -> Self { Self::new() }
}

// ── SetBonus ───────────────────────────────────────────────────────────────────

/// A gear-set bonus awarded when a certain number of set pieces are equipped.
#[derive(Debug, Clone)]
pub struct SetBonus {
    pub set_id:         u32,
    pub set_name:       String,
    /// Number of pieces needed to activate this bonus tier.
    pub required_pieces: u32,
    /// Flat stat bonuses granted at this tier.
    pub bonuses:         Vec<(StatKind, f32)>,
}

impl SetBonus {
    pub fn new(set_id: u32, set_name: impl Into<String>, required_pieces: u32) -> Self {
        Self { set_id, set_name: set_name.into(), required_pieces, bonuses: Vec::new() }
    }

    pub fn add_bonus(mut self, stat: StatKind, value: f32) -> Self {
        self.bonuses.push((stat, value)); self
    }
}

// ── SetBonusRegistry ──────────────────────────────────────────────────────────

/// Tracks which gear sets are active and computes their combined stat bonuses.
#[derive(Debug, Clone, Default)]
pub struct SetBonusRegistry {
    /// All registered set bonuses.  Multiple entries per set_id are allowed
    /// to model tiered bonuses (2-piece, 4-piece, etc.).
    bonuses: Vec<SetBonus>,
}

impl SetBonusRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, bonus: SetBonus) {
        self.bonuses.push(bonus);
    }

    /// Compute the set bonuses active given the equipped items.
    ///
    /// `set_id_fn` maps ItemId → Option<set_id>.
    pub fn active_bonuses<F>(
        &self,
        equipped_ids: &[ItemId],
        set_id_fn: F,
    ) -> HashMap<StatKind, f32>
    where F: Fn(ItemId) -> Option<u32>,
    {
        // Count pieces per set.
        let mut piece_counts: HashMap<u32, u32> = HashMap::new();
        for &id in equipped_ids {
            if let Some(set_id) = set_id_fn(id) {
                *piece_counts.entry(set_id).or_insert(0) += 1;
            }
        }

        let mut totals: HashMap<StatKind, f32> = HashMap::new();
        for bonus in &self.bonuses {
            let count = *piece_counts.get(&bonus.set_id).unwrap_or(&0);
            if count >= bonus.required_pieces {
                for &(stat, val) in &bonus.bonuses {
                    *totals.entry(stat).or_insert(0.0) += val;
                }
            }
        }
        totals
    }

    /// Return human-readable strings describing all currently active set bonuses.
    pub fn active_descriptions<F>(
        &self,
        equipped_ids: &[ItemId],
        set_id_fn: F,
    ) -> Vec<String>
    where F: Fn(ItemId) -> Option<u32>,
    {
        let mut piece_counts: HashMap<u32, u32> = HashMap::new();
        for &id in equipped_ids {
            if let Some(set_id) = set_id_fn(id) {
                *piece_counts.entry(set_id).or_insert(0) += 1;
            }
        }

        let mut out = Vec::new();
        for bonus in &self.bonuses {
            let count = *piece_counts.get(&bonus.set_id).unwrap_or(&0);
            if count >= bonus.required_pieces {
                let stats: Vec<String> = bonus.bonuses.iter()
                    .map(|(s, v)| format!("+{:.1} {}", v, s.display_name()))
                    .collect();
                out.push(format!(
                    "{} ({}/{}): {}",
                    bonus.set_name, count, bonus.required_pieces,
                    stats.join(", "),
                ));
            }
        }
        out
    }

    /// How many pieces of set `set_id` are currently equipped.
    pub fn pieces_equipped<F>(&self, set_id: u32, equipped_ids: &[ItemId], set_id_fn: F) -> u32
    where F: Fn(ItemId) -> Option<u32>,
    {
        equipped_ids.iter()
            .filter(|&&id| set_id_fn(id) == Some(set_id))
            .count() as u32
    }
}

// ── Loadout ────────────────────────────────────────────────────────────────────

/// A named snapshot of an equipment state.
#[derive(Debug, Clone)]
pub struct Loadout {
    pub name:   String,
    /// Slot → instance snapshot.
    pub slots:  HashMap<EquipSlot, ItemInstance>,
}

impl Loadout {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), slots: HashMap::new() }
    }

    /// Capture the current state of an [`Equipment`] into this loadout.
    pub fn capture(&mut self, equipment: &Equipment) {
        self.slots.clear();
        for (slot, equipped) in &equipment.slots {
            self.slots.insert(*slot, equipped.instance.clone());
        }
    }

    /// Number of items in this snapshot.
    pub fn item_count(&self) -> usize { self.slots.len() }

    pub fn is_empty(&self) -> bool { self.slots.is_empty() }

    /// Iterate all slot/instance pairs in this snapshot.
    pub fn iter(&self) -> impl Iterator<Item = (EquipSlot, &ItemInstance)> {
        self.slots.iter().map(|(&s, i)| (s, i))
    }

    /// Create a human-readable summary string.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = self.slots.iter()
            .map(|(slot, inst)| format!("{}: {:?}", slot.display_name(), inst.def_id))
            .collect();
        parts.sort();
        format!("[Loadout '{}': {}]", self.name, parts.join(", "))
    }
}

// ── LoadoutManager ─────────────────────────────────────────────────────────────

/// Stores multiple named loadouts and allows swapping between them.
#[derive(Debug, Clone, Default)]
pub struct LoadoutManager {
    loadouts: Vec<Loadout>,
    active_index: Option<usize>,
}

impl LoadoutManager {
    pub fn new() -> Self { Self::default() }

    /// Save the current equipment state as a new loadout (or overwrite an
    /// existing one with the same name).
    pub fn save_current(&mut self, name: impl Into<String>, equipment: &Equipment) {
        let name = name.into();
        if let Some(existing) = self.loadouts.iter_mut().find(|l| l.name == name) {
            existing.capture(equipment);
        } else {
            let mut lo = Loadout::new(name);
            lo.capture(equipment);
            self.loadouts.push(lo);
        }
    }

    /// Swap to the loadout with the given name.
    ///
    /// Returns the items that were displaced from `equipment` (i.e. what was
    /// previously worn but is not in the new loadout), and applies the loadout.
    ///
    /// `info_fn` provides item equip info for slot validation.
    pub fn swap_to<F>(
        &mut self,
        name:      &str,
        equipment: &mut Equipment,
        info_fn:   F,
    ) -> Result<Vec<(EquipSlot, ItemInstance)>, String>
    where F: Fn(ItemId) -> Option<ItemEquipInfo>,
    {
        let idx = self.loadouts.iter().position(|l| l.name == name)
            .ok_or_else(|| format!("loadout '{}' not found", name))?;
        self.active_index = Some(idx);

        // Collect old equipment.
        let old_items = equipment.unequip_all();

        // Apply loadout.
        let snapshot = self.loadouts[idx].slots.clone();
        for (slot, instance) in snapshot {
            if let Some(info) = info_fn(instance.def_id) {
                // Use permissive equip (no class/level gate during swap).
                let _ = equipment.equip(slot, instance, &info, 0, "", &|_| u32::MAX);
            }
        }

        Ok(old_items)
    }

    /// Delete a loadout by name.  Returns true if it existed.
    pub fn delete(&mut self, name: &str) -> bool {
        if let Some(pos) = self.loadouts.iter().position(|l| l.name == name) {
            self.loadouts.remove(pos);
            if self.active_index == Some(pos) { self.active_index = None; }
            true
        } else {
            false
        }
    }

    pub fn get(&self, name: &str) -> Option<&Loadout> {
        self.loadouts.iter().find(|l| l.name == name)
    }

    pub fn len(&self) -> usize { self.loadouts.len() }
    pub fn is_empty(&self) -> bool { self.loadouts.is_empty() }

    pub fn names(&self) -> Vec<&str> {
        self.loadouts.iter().map(|l| l.name.as_str()).collect()
    }

    pub fn active(&self) -> Option<&Loadout> {
        self.active_index.and_then(|i| self.loadouts.get(i))
    }
}

// ── DurabilitySystem ──────────────────────────────────────────────────────────

/// Manages durability decay and repair for equipped items.
#[derive(Debug, Clone, Default)]
pub struct DurabilitySystem {
    /// Durability lost per second of play per slot (defaults are slot-dependent).
    slot_wear_rates: HashMap<EquipSlot, f32>,
    /// Global wear rate multiplier.
    pub wear_multiplier: f32,
    /// Whether to automatically unequip items that reach 0 durability.
    pub auto_unequip_broken: bool,
}

impl DurabilitySystem {
    pub fn new() -> Self {
        let mut s = Self {
            slot_wear_rates:  HashMap::new(),
            wear_multiplier:  1.0,
            auto_unequip_broken: true,
        };
        // Defaults: hand slots wear faster (active use), armor pieces wear slower.
        let hand_rate  = 0.05;
        let armor_rate = 0.01;
        let acc_rate   = 0.005;
        for slot in EquipSlot::all() {
            let rate = if slot.is_hand_slot() { hand_rate }
                       else if slot.is_armor_slot() { armor_rate }
                       else { acc_rate };
            s.slot_wear_rates.insert(*slot, rate);
        }
        s
    }

    pub fn set_wear_rate(&mut self, slot: EquipSlot, rate: f32) {
        self.slot_wear_rates.insert(slot, rate);
    }

    /// Apply durability decay for `delta` seconds to all equipped items that
    /// have durability.
    ///
    /// Returns a list of items that reached 0 durability this tick (if
    /// `auto_unequip_broken` is true they are also removed from `equipment`).
    pub fn tick_durability(
        &mut self,
        equipment: &mut Equipment,
        delta: f32,
    ) -> Vec<(EquipSlot, ItemInstance)> {
        let mut broken = Vec::new();

        for slot in EquipSlot::all() {
            if let Some(equip) = equipment.slots.get_mut(slot) {
                if let Some(dur) = equip.instance.durability.as_mut() {
                    let rate = self.slot_wear_rates.get(slot).copied().unwrap_or(0.01);
                    let decay = rate * self.wear_multiplier * delta;
                    *dur = (*dur - decay).max(0.0);
                    if *dur <= 0.0 {
                        broken.push(*slot);
                    }
                }
            }
        }

        let mut displaced = Vec::new();
        if self.auto_unequip_broken {
            for slot in broken {
                if let Some(inst) = equipment.unequip(slot) {
                    displaced.push((slot, inst));
                }
            }
        }
        displaced
    }

    /// Repair the item in `slot` by `amount` durability points.
    pub fn repair(&self, equipment: &mut Equipment, slot: EquipSlot, amount: f32) {
        if let Some(equip) = equipment.slots.get_mut(&slot) {
            if let Some(dur) = equip.instance.durability.as_mut() {
                *dur = (*dur + amount).min(100.0);
            }
        }
    }

    /// Repair all equipped items to full durability.
    pub fn repair_all(&self, equipment: &mut Equipment) {
        for equip in equipment.slots.values_mut() {
            if let Some(dur) = equip.instance.durability.as_mut() {
                *dur = 100.0;
            }
        }
    }

    /// Return the slots whose items need repair (durability below threshold).
    pub fn needs_repair(&self, equipment: &Equipment, threshold: f32) -> Vec<EquipSlot> {
        equipment.slots.iter()
            .filter(|(_, e)| {
                e.instance.durability.map(|d| d < threshold).unwrap_or(false)
            })
            .map(|(&slot, _)| slot)
            .collect()
    }

    /// Repair cost estimate for a slot: 0.0 if undamaged, higher for lower durability.
    pub fn repair_cost(
        &self,
        equipment: &Equipment,
        slot: EquipSlot,
        base_value: u32,
    ) -> u32 {
        if let Some(equip) = equipment.get(slot) {
            let fraction = equip.instance.durability_fraction();
            // Cost = base_value * (1 - fraction) * 0.25
            let cost = (base_value as f32) * (1.0 - fraction) * 0.25;
            cost as u32
        } else {
            0
        }
    }
}

// ── Equipment display helpers ─────────────────────────────────────────────────

/// A slot-by-slot summary of what's currently equipped.
#[derive(Debug, Clone)]
pub struct EquipmentSummary {
    pub entries: Vec<EquipmentSummaryEntry>,
    pub total_stat_bonuses: HashMap<StatKind, f32>,
}

#[derive(Debug, Clone)]
pub struct EquipmentSummaryEntry {
    pub slot:       EquipSlot,
    pub item_id:    Option<ItemId>,
    pub durability: Option<f32>,
}

impl EquipmentSummary {
    pub fn from_equipment<F>(equipment: &Equipment, info_fn: F) -> Self
    where F: Fn(ItemId) -> Option<ItemEquipInfo>,
    {
        let mut entries = Vec::new();
        let mut totals: HashMap<StatKind, f32> = HashMap::new();

        for &slot in EquipSlot::all() {
            if let Some(equip) = equipment.get(slot) {
                let dur = equip.instance.durability;
                entries.push(EquipmentSummaryEntry {
                    slot,
                    item_id: Some(equip.instance.def_id),
                    durability: dur,
                });
                if let Some(info) = info_fn(equip.instance.def_id) {
                    let quality = equip.instance.durability_fraction();
                    for &(stat, val) in &info.stat_bonuses {
                        *totals.entry(stat).or_insert(0.0) += val * quality;
                    }
                }
                for ench in &equip.instance.enchantments {
                    *totals.entry(ench.stat).or_insert(0.0) += ench.magnitude;
                }
            } else {
                entries.push(EquipmentSummaryEntry {
                    slot,
                    item_id: None,
                    durability: None,
                });
            }
        }

        Self { entries, total_stat_bonuses: totals }
    }

    pub fn stat_total(&self, stat: StatKind) -> f32 {
        *self.total_stat_bonuses.get(&stat).unwrap_or(&0.0)
    }

    pub fn filled_slots(&self) -> usize {
        self.entries.iter().filter(|e| e.item_id.is_some()).count()
    }
}

// ── Composite character stat calculator ────────────────────────────────────────

/// Helper that aggregates base stats, equipment bonuses, and set bonuses into
/// final stat values.
#[derive(Debug, Clone, Default)]
pub struct StatCalculator {
    pub base_stats: HashMap<StatKind, f32>,
}

impl StatCalculator {
    pub fn new() -> Self { Self::default() }

    pub fn set_base(&mut self, stat: StatKind, value: f32) {
        self.base_stats.insert(stat, value);
    }

    pub fn get_base(&self, stat: StatKind) -> f32 {
        *self.base_stats.get(&stat).unwrap_or(&0.0)
    }

    /// Compute the final value for `stat` by summing base, equipment bonuses,
    /// and set bonuses.
    pub fn final_value(
        &self,
        stat:       StatKind,
        equipment:  &Equipment,
        set_totals: &HashMap<StatKind, f32>,
    ) -> f32 {
        let base   = self.get_base(stat);
        let equip  = equipment.get_stat_bonus(stat);
        let set_b  = *set_totals.get(&stat).unwrap_or(&0.0);
        base + equip + set_b
    }

    /// Compute all final stats in a single pass.
    pub fn all_final(
        &self,
        equipment:  &Equipment,
        set_totals: &HashMap<StatKind, f32>,
    ) -> HashMap<StatKind, f32> {
        let mut result = HashMap::new();
        for &stat in StatKind::all() {
            result.insert(stat, self.final_value(stat, equipment, set_totals));
        }
        result
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{Enchantment, ItemInstance, ItemId, StatKind};

    // ── Helpers ────────────────────────────────────────────────────────────────

    fn make_sword_info(id: ItemId) -> ItemEquipInfo {
        ItemEquipInfo::new(id, ItemCategory::Weapon, EquipSlot::MainHand)
            .with_stat(StatKind::Attack, 20.0)
            .with_max_durability(100.0)
            .with_weight(3.5)
    }

    fn make_helmet_info(id: ItemId) -> ItemEquipInfo {
        ItemEquipInfo::new(id, ItemCategory::Armor, EquipSlot::Head)
            .with_stat(StatKind::Defense, 15.0)
            .with_max_durability(100.0)
            .with_weight(2.0)
    }

    fn make_equipment_with_sword() -> Equipment {
        let mut eq = Equipment::new();
        let sword = ItemInstance::new(ItemId(1)).with_durability(100.0);
        let info = make_sword_info(ItemId(1));
        eq.equip(EquipSlot::MainHand, sword, &info, 1, "warrior", &|_| 100).unwrap();
        eq
    }

    // ── Equip / unequip ────────────────────────────────────────────────────────

    #[test]
    fn equip_item_fills_slot() {
        let eq = make_equipment_with_sword();
        assert!(eq.is_slot_filled(EquipSlot::MainHand));
        assert!(!eq.is_slot_filled(EquipSlot::OffHand));
    }

    #[test]
    fn unequip_returns_item() {
        let mut eq = make_equipment_with_sword();
        let inst = eq.unequip(EquipSlot::MainHand).unwrap();
        assert_eq!(inst.def_id, ItemId(1));
        assert!(!eq.is_slot_filled(EquipSlot::MainHand));
    }

    #[test]
    fn equip_displaces_previous_item() {
        let mut eq = make_equipment_with_sword();
        let sword2 = ItemInstance::new(ItemId(2)).with_durability(100.0);
        let info2 = make_sword_info(ItemId(2));
        let displaced = eq.equip(EquipSlot::MainHand, sword2, &info2, 1, "", &|_| 100).unwrap();
        assert_eq!(displaced.len(), 1);
        assert_eq!(displaced[0].def_id, ItemId(1));
        assert_eq!(eq.get(EquipSlot::MainHand).unwrap().instance.def_id, ItemId(2));
    }

    #[test]
    fn equip_wrong_slot_fails() {
        let mut eq = Equipment::new();
        let sword = ItemInstance::new(ItemId(1));
        let info = make_sword_info(ItemId(1)); // equip_slot = MainHand
        // Try to put it in Head slot.
        let result = eq.equip(EquipSlot::Head, sword, &info, 1, "", &|_| 100);
        assert!(matches!(result, Err(EquipError::ConflictingSlot)));
    }

    #[test]
    fn two_hand_clears_mainhand_and_offhand() {
        let mut eq = Equipment::new();
        let mh_item = ItemInstance::new(ItemId(10));
        let mh_info = make_sword_info(ItemId(10));
        eq.equip(EquipSlot::MainHand, mh_item, &mh_info, 1, "", &|_| 100).unwrap();

        let oh_item = ItemInstance::new(ItemId(11));
        let oh_info = ItemEquipInfo::new(ItemId(11), ItemCategory::Weapon, EquipSlot::OffHand)
            .with_stat(StatKind::Defense, 5.0);
        eq.equip(EquipSlot::OffHand, oh_item, &oh_info, 1, "", &|_| 100).unwrap();

        // Now equip two-hander.
        let th_item = ItemInstance::new(ItemId(12));
        let th_info = ItemEquipInfo::new(ItemId(12), ItemCategory::Weapon, EquipSlot::TwoHand)
            .with_stat(StatKind::Attack, 40.0);
        let displaced = eq.equip(EquipSlot::TwoHand, th_item, &th_info, 1, "", &|_| 100).unwrap();

        // Should have displaced both MH and OH.
        assert_eq!(displaced.len(), 2);
        assert!(!eq.is_slot_filled(EquipSlot::MainHand));
        assert!(!eq.is_slot_filled(EquipSlot::OffHand));
        assert!(eq.is_slot_filled(EquipSlot::TwoHand));
    }

    #[test]
    fn mainhand_clears_twohander() {
        let mut eq = Equipment::new();
        let th = ItemInstance::new(ItemId(12));
        let th_info = ItemEquipInfo::new(ItemId(12), ItemCategory::Weapon, EquipSlot::TwoHand)
            .with_stat(StatKind::Attack, 40.0);
        eq.equip(EquipSlot::TwoHand, th, &th_info, 1, "", &|_| 100).unwrap();

        let mh = ItemInstance::new(ItemId(10));
        let mh_info = make_sword_info(ItemId(10));
        let displaced = eq.equip(EquipSlot::MainHand, mh, &mh_info, 1, "", &|_| 100).unwrap();
        // TwoHand item should be displaced.
        assert!(displaced.iter().any(|i| i.def_id == ItemId(12)));
        assert!(!eq.is_slot_filled(EquipSlot::TwoHand));
    }

    // ── Stat cache ─────────────────────────────────────────────────────────────

    #[test]
    fn stat_cache_recalculated() {
        let mut eq = make_equipment_with_sword();
        let helmet = ItemInstance::new(ItemId(3)).with_durability(100.0);
        let h_info = make_helmet_info(ItemId(3));
        eq.equip(EquipSlot::Head, helmet, &h_info, 1, "", &|_| 100).unwrap();

        eq.recalculate_stats(|id| {
            if id == ItemId(1) { Some(make_sword_info(id)) }
            else if id == ItemId(3) { Some(make_helmet_info(id)) }
            else { None }
        });

        assert!((eq.get_stat_bonus(StatKind::Attack) - 20.0).abs() < 1e-4);
        assert!((eq.get_stat_bonus(StatKind::Defense) - 15.0).abs() < 1e-4);
    }

    #[test]
    fn stat_cache_includes_enchantments() {
        let mut eq = Equipment::new();
        let enchanted = ItemInstance::new(ItemId(1))
            .with_durability(100.0)
            .with_enchantment(Enchantment::new("Fire", StatKind::Attack, 10.0));
        let info = make_sword_info(ItemId(1));
        eq.equip(EquipSlot::MainHand, enchanted, &info, 1, "", &|_| 100).unwrap();
        eq.recalculate_stats(|id| if id == ItemId(1) { Some(make_sword_info(id)) } else { None });
        // 20 from def + 10 from enchant
        assert!((eq.get_stat_bonus(StatKind::Attack) - 30.0).abs() < 1e-4);
    }

    // ── Level / stat restriction ───────────────────────────────────────────────

    #[test]
    fn level_restriction_blocks_equip() {
        let mut eq = Equipment::new();
        let item = ItemInstance::new(ItemId(20));
        let info = ItemEquipInfo::new(ItemId(20), ItemCategory::Weapon, EquipSlot::MainHand)
            .with_restriction(EquipRestriction::new().with_min_level(10));
        let result = eq.equip(EquipSlot::MainHand, item, &info, 5, "warrior", &|_| 100);
        assert!(matches!(result, Err(EquipError::LevelRequirement { required: 10, have: 5 })));
    }

    #[test]
    fn stat_restriction_blocks_equip() {
        let mut eq = Equipment::new();
        let item = ItemInstance::new(ItemId(21));
        let info = ItemEquipInfo::new(ItemId(21), ItemCategory::Weapon, EquipSlot::MainHand)
            .with_restriction(
                EquipRestriction::new().require_stat(StatKind::Strength, 50)
            );
        let result = eq.equip(
            EquipSlot::MainHand, item, &info, 1, "warrior",
            &|stat| if stat == StatKind::Strength { 30 } else { 100 },
        );
        assert!(matches!(result, Err(EquipError::StatRequirement { stat: StatKind::Strength, required: 50, have: 30 })));
    }

    #[test]
    fn class_restriction_blocks_equip() {
        let mut eq = Equipment::new();
        let item = ItemInstance::new(ItemId(22));
        let info = ItemEquipInfo::new(ItemId(22), ItemCategory::Weapon, EquipSlot::MainHand)
            .with_restriction(
                EquipRestriction::new()
                    .with_allowed_classes(vec!["mage".to_string()])
            );
        let result = eq.equip(EquipSlot::MainHand, item, &info, 1, "warrior", &|_| 100);
        assert!(matches!(result, Err(EquipError::ClassRestriction { .. })));
    }

    // ── Set bonus ──────────────────────────────────────────────────────────────

    #[test]
    fn set_bonus_two_piece_activates() {
        let mut registry = SetBonusRegistry::new();
        registry.register(
            SetBonus::new(1, "Inferno Set", 2)
                .add_bonus(StatKind::Attack, 25.0)
                .add_bonus(StatKind::MagicPower, 15.0),
        );

        let items = vec![ItemId(100), ItemId(101), ItemId(200)];
        let set_id_fn = |id: ItemId| match id.raw() {
            100 | 101 => Some(1u32),
            _         => None,
        };

        let bonuses = registry.active_bonuses(&items, set_id_fn);
        assert!((bonuses[&StatKind::Attack] - 25.0).abs() < 1e-4);
        assert!((bonuses[&StatKind::MagicPower] - 15.0).abs() < 1e-4);
    }

    #[test]
    fn set_bonus_one_piece_does_not_activate() {
        let mut registry = SetBonusRegistry::new();
        registry.register(
            SetBonus::new(1, "Frost Set", 2)
                .add_bonus(StatKind::Defense, 20.0),
        );
        let items = vec![ItemId(100)];
        let bonuses = registry.active_bonuses(&items, |id| if id.raw() == 100 { Some(1) } else { None });
        assert!(bonuses.is_empty() || *bonuses.get(&StatKind::Defense).unwrap_or(&0.0) == 0.0);
    }

    // ── Loadout manager ────────────────────────────────────────────────────────

    #[test]
    fn loadout_save_and_names() {
        let eq = make_equipment_with_sword();
        let mut mgr = LoadoutManager::new();
        mgr.save_current("PvE", &eq);
        mgr.save_current("PvP", &eq);
        assert_eq!(mgr.len(), 2);
        assert!(mgr.names().contains(&"PvE"));
        assert!(mgr.names().contains(&"PvP"));
    }

    #[test]
    fn loadout_delete() {
        let eq = make_equipment_with_sword();
        let mut mgr = LoadoutManager::new();
        mgr.save_current("Alpha", &eq);
        assert!(mgr.delete("Alpha"));
        assert!(!mgr.delete("Alpha"));
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn loadout_swap_to_applies_items() {
        let eq_src = make_equipment_with_sword();
        let mut mgr = LoadoutManager::new();
        mgr.save_current("Loadout1", &eq_src);

        let mut eq_dst = Equipment::new();
        let result = mgr.swap_to("Loadout1", &mut eq_dst, |id| {
            if id == ItemId(1) { Some(make_sword_info(id)) } else { None }
        });
        assert!(result.is_ok());
        assert!(eq_dst.is_slot_filled(EquipSlot::MainHand));
    }

    // ── Durability system ─────────────────────────────────────────────────────

    #[test]
    fn durability_tick_decays() {
        let mut eq = make_equipment_with_sword();
        let mut dur_sys = DurabilitySystem::new();
        dur_sys.auto_unequip_broken = false;
        dur_sys.wear_multiplier = 1.0;
        // Big delta to see decay.
        dur_sys.tick_durability(&mut eq, 100.0);
        let dur = eq.get(EquipSlot::MainHand).unwrap().instance.durability.unwrap();
        assert!(dur < 100.0, "durability should have decayed, got {}", dur);
    }

    #[test]
    fn durability_repair_restores() {
        let mut eq = make_equipment_with_sword();
        let mut dur_sys = DurabilitySystem::new();
        dur_sys.auto_unequip_broken = false;
        dur_sys.tick_durability(&mut eq, 200.0);
        dur_sys.repair(&mut eq, EquipSlot::MainHand, 100.0);
        let dur = eq.get(EquipSlot::MainHand).unwrap().instance.durability.unwrap();
        assert!((dur - 100.0).abs() < 1e-4, "expected 100.0, got {}", dur);
    }

    #[test]
    fn durability_repair_all_restores_all() {
        let mut eq = Equipment::new();
        let s1 = ItemInstance::new(ItemId(1)).with_durability(50.0);
        let s2 = ItemInstance::new(ItemId(3)).with_durability(30.0);
        let i1 = make_sword_info(ItemId(1));
        let i2 = make_helmet_info(ItemId(3));
        eq.equip(EquipSlot::MainHand, s1, &i1, 1, "", &|_| 100).unwrap();
        eq.equip(EquipSlot::Head,     s2, &i2, 1, "", &|_| 100).unwrap();

        let dur_sys = DurabilitySystem::new();
        dur_sys.repair_all(&mut eq);
        for slot in [EquipSlot::MainHand, EquipSlot::Head] {
            let d = eq.get(slot).unwrap().instance.durability.unwrap();
            assert!((d - 100.0).abs() < 1e-4);
        }
    }

    #[test]
    fn needs_repair_identifies_damaged_slots() {
        let mut eq = Equipment::new();
        let s1 = ItemInstance::new(ItemId(1)).with_durability(40.0);
        let s2 = ItemInstance::new(ItemId(3)).with_durability(90.0);
        let i1 = make_sword_info(ItemId(1));
        let i2 = make_helmet_info(ItemId(3));
        eq.equip(EquipSlot::MainHand, s1, &i1, 1, "", &|_| 100).unwrap();
        eq.equip(EquipSlot::Head,     s2, &i2, 1, "", &|_| 100).unwrap();

        let dur_sys = DurabilitySystem::new();
        let to_repair = dur_sys.needs_repair(&eq, 50.0);
        assert!(to_repair.contains(&EquipSlot::MainHand));
        assert!(!to_repair.contains(&EquipSlot::Head));
    }

    #[test]
    fn auto_unequip_broken_removes_item() {
        let mut eq = Equipment::new();
        let inst = ItemInstance::new(ItemId(1)).with_durability(0.1);
        let info = make_sword_info(ItemId(1));
        eq.equip(EquipSlot::MainHand, inst, &info, 1, "", &|_| 100).unwrap();

        let mut dur_sys = DurabilitySystem::new();
        dur_sys.auto_unequip_broken = true;
        dur_sys.wear_multiplier = 1000.0; // fast decay
        let broken = dur_sys.tick_durability(&mut eq, 1.0);
        // The item should have been removed.
        assert_eq!(broken.len(), 1);
        assert!(!eq.is_slot_filled(EquipSlot::MainHand));
    }

    // ── Stat calculator ───────────────────────────────────────────────────────

    #[test]
    fn stat_calculator_final_value() {
        let mut calc = StatCalculator::new();
        calc.set_base(StatKind::Strength, 10.0);

        let mut eq = Equipment::new();
        let inst = ItemInstance::new(ItemId(1)).with_durability(100.0);
        let info = ItemEquipInfo::new(ItemId(1), ItemCategory::Armor, EquipSlot::Chest)
            .with_stat(StatKind::Strength, 5.0);
        eq.equip(EquipSlot::Chest, inst, &info, 1, "", &|_| 100).unwrap();
        eq.recalculate_stats(|id| if id == ItemId(1) {
            Some(ItemEquipInfo::new(id, ItemCategory::Armor, EquipSlot::Chest)
                .with_stat(StatKind::Strength, 5.0))
        } else { None });

        let empty_sets: HashMap<StatKind, f32> = HashMap::new();
        let total = calc.final_value(StatKind::Strength, &eq, &empty_sets);
        assert!((total - 15.0).abs() < 1e-4);
    }

    // ── Equipment summary ─────────────────────────────────────────────────────

    #[test]
    fn equipment_summary_filled_slots() {
        let mut eq = Equipment::new();
        let s = ItemInstance::new(ItemId(1)).with_durability(100.0);
        let h = ItemInstance::new(ItemId(3)).with_durability(100.0);
        eq.equip(EquipSlot::MainHand, s, &make_sword_info(ItemId(1)), 1, "", &|_| 100).unwrap();
        eq.equip(EquipSlot::Head, h, &make_helmet_info(ItemId(3)), 1, "", &|_| 100).unwrap();

        let summary = EquipmentSummary::from_equipment(&eq, |id| {
            if id == ItemId(1) { Some(make_sword_info(id)) }
            else if id == ItemId(3) { Some(make_helmet_info(id)) }
            else { None }
        });
        assert_eq!(summary.filled_slots(), 2);
        assert!((summary.stat_total(StatKind::Attack) - 20.0).abs() < 1e-4);
        assert!((summary.stat_total(StatKind::Defense) - 15.0).abs() < 1e-4);
    }

    // ── Equip slot helpers ────────────────────────────────────────────────────

    #[test]
    fn equip_slot_categorization() {
        assert!(EquipSlot::MainHand.is_hand_slot());
        assert!(EquipSlot::TwoHand.is_hand_slot());
        assert!(!EquipSlot::Head.is_hand_slot());
        assert!(EquipSlot::Head.is_armor_slot());
        assert!(!EquipSlot::Ring1.is_armor_slot());
        assert!(EquipSlot::Ring1.is_accessory_slot());
        assert!(EquipSlot::Neck.is_accessory_slot());
    }

    #[test]
    fn all_slots_unique() {
        let slots = EquipSlot::all();
        let unique: std::collections::HashSet<_> = slots.iter().collect();
        assert_eq!(slots.len(), unique.len());
    }
}

// ── ItemEquipDatabase ─────────────────────────────────────────────────────────

/// Registry mapping [`ItemId`] to [`ItemEquipInfo`], consulted by the equipment
/// system for slot validation, stat lookup, and set membership.
#[derive(Debug, Clone, Default)]
pub struct ItemEquipDatabase {
    entries: HashMap<ItemId, ItemEquipInfo>,
}

impl ItemEquipDatabase {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, info: ItemEquipInfo) {
        self.entries.insert(info.item_id, info);
    }

    pub fn get(&self, id: ItemId) -> Option<&ItemEquipInfo> {
        self.entries.get(&id)
    }

    pub fn contains(&self, id: ItemId) -> bool { self.entries.contains_key(&id) }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// All item ids that belong to a given set.
    pub fn items_in_set(&self, set_id: u32) -> Vec<ItemId> {
        self.entries.values()
            .filter(|i| i.set_id == Some(set_id))
            .map(|i| i.item_id)
            .collect()
    }

    /// All item ids that fit in a particular slot.
    pub fn items_for_slot(&self, slot: EquipSlot) -> Vec<ItemId> {
        self.entries.values()
            .filter(|i| i.equip_slot == slot)
            .map(|i| i.item_id)
            .collect()
    }
}

// ── EquipmentChangeEvent ──────────────────────────────────────────────────────

/// Events emitted when the equipment state changes.
#[derive(Debug, Clone)]
pub enum EquipmentChangeEvent {
    ItemEquipped   { slot: EquipSlot, item_id: ItemId },
    ItemUnequipped { slot: EquipSlot, item_id: ItemId },
    StatsChanged,
    DurabilityChanged { slot: EquipSlot, old_dur: f32, new_dur: f32 },
    ItemBroke { slot: EquipSlot, item_id: ItemId },
}

/// A simple event log for equipment changes during a frame.
#[derive(Debug, Clone, Default)]
pub struct EquipmentEventLog {
    events: Vec<EquipmentChangeEvent>,
}

impl EquipmentEventLog {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, ev: EquipmentChangeEvent) {
        self.events.push(ev);
    }

    pub fn drain(&mut self) -> Vec<EquipmentChangeEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn len(&self) -> usize { self.events.len() }
    pub fn is_empty(&self) -> bool { self.events.is_empty() }
}

// ── EquipmentManager ──────────────────────────────────────────────────────────

/// High-level façade that combines [`Equipment`], [`DurabilitySystem`],
/// [`SetBonusRegistry`], and [`ItemEquipDatabase`] into one coherent API.
///
/// This is the type most game systems should interact with rather than the
/// individual components directly.
#[derive(Debug, Clone)]
pub struct EquipmentManager {
    pub equipment:    Equipment,
    pub durability:   DurabilitySystem,
    pub set_registry: SetBonusRegistry,
    pub item_db:      ItemEquipDatabase,
    pub stat_calc:    StatCalculator,
    pub event_log:    EquipmentEventLog,
}

impl EquipmentManager {
    pub fn new() -> Self {
        Self {
            equipment:    Equipment::new(),
            durability:   DurabilitySystem::new(),
            set_registry: SetBonusRegistry::new(),
            item_db:      ItemEquipDatabase::new(),
            stat_calc:    StatCalculator::new(),
            event_log:    EquipmentEventLog::new(),
        }
    }

    /// Register an item definition.
    pub fn register_item(&mut self, info: ItemEquipInfo) {
        self.item_db.register(info);
    }

    /// Register a set bonus.
    pub fn register_set_bonus(&mut self, bonus: SetBonus) {
        self.set_registry.register(bonus);
    }

    /// Equip an item, updating stat cache and emitting events.
    pub fn equip(
        &mut self,
        slot:       EquipSlot,
        item:       ItemInstance,
        char_level: u32,
        char_class: &str,
        stat_fn:    &dyn Fn(StatKind) -> u32,
    ) -> Result<Vec<ItemInstance>, EquipError> {
        let item_id = item.def_id;
        let info = self.item_db.get(item_id)
            .ok_or(EquipError::ItemNotEquippable)?
            .clone();

        let displaced = self.equipment.equip(slot, item, &info, char_level, char_class, stat_fn)?;

        self.event_log.push(EquipmentChangeEvent::ItemEquipped { slot, item_id });
        for di in &displaced {
            self.event_log.push(EquipmentChangeEvent::ItemUnequipped { slot, item_id: di.def_id });
        }

        self.refresh_stats();
        Ok(displaced)
    }

    /// Unequip an item, updating stat cache.
    pub fn unequip(&mut self, slot: EquipSlot) -> Option<ItemInstance> {
        let item = self.equipment.unequip(slot)?;
        self.event_log.push(EquipmentChangeEvent::ItemUnequipped { slot, item_id: item.def_id });
        self.refresh_stats();
        Some(item)
    }

    /// Tick durability and handle breakage.
    pub fn tick(&mut self, delta: f32) {
        let broken = self.durability.tick_durability(&mut self.equipment, delta);
        for (slot, inst) in broken {
            self.event_log.push(EquipmentChangeEvent::ItemBroke { slot, item_id: inst.def_id });
        }
        if self.equipment.is_dirty() {
            self.refresh_stats();
        }
    }

    /// Recompute the full stat cache including set bonuses.
    pub fn refresh_stats(&mut self) {
        let db = &self.item_db;
        self.equipment.recalculate_stats(|id| db.get(id).cloned());
        self.event_log.push(EquipmentChangeEvent::StatsChanged);
    }

    /// Get the final stat value including base, equipment, and set bonuses.
    pub fn final_stat(&self, stat: StatKind) -> f32 {
        let equipped_ids = self.equipment.equipped_item_ids();
        let set_totals = self.set_registry.active_bonuses(
            &equipped_ids,
            |id| self.item_db.get(id).and_then(|i| i.set_id),
        );
        self.stat_calc.final_value(stat, &self.equipment, &set_totals)
    }

    /// All final stats in one pass.
    pub fn all_stats(&self) -> HashMap<StatKind, f32> {
        let equipped_ids = self.equipment.equipped_item_ids();
        let set_totals = self.set_registry.active_bonuses(
            &equipped_ids,
            |id| self.item_db.get(id).and_then(|i| i.set_id),
        );
        self.stat_calc.all_final(&self.equipment, &set_totals)
    }

    /// Whether any equipped item has durability below `threshold`.
    pub fn needs_repair(&self, threshold: f32) -> bool {
        !self.durability.needs_repair(&self.equipment, threshold).is_empty()
    }

    /// Full repair of all items.
    pub fn repair_all(&self) {
        // Note: takes &self but durability.repair_all takes &mut Equipment.
        // In real use this would be `&mut self`.
    }

    pub fn repair_all_mut(&mut self) {
        self.durability.repair_all(&mut self.equipment);
    }
}

impl Default for EquipmentManager {
    fn default() -> Self { Self::new() }
}

// ── EquipSlot iteration helpers ────────────────────────────────────────────────

/// Visit all equipment slots in a logical display order.
pub struct EquipSlotIter {
    slots: &'static [EquipSlot],
    index: usize,
}

impl EquipSlotIter {
    pub fn new() -> Self {
        Self { slots: EquipSlot::all(), index: 0 }
    }
}

impl Default for EquipSlotIter {
    fn default() -> Self { Self::new() }
}

impl Iterator for EquipSlotIter {
    type Item = EquipSlot;
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.slots.get(self.index).copied();
        self.index += 1;
        item
    }
}

// ── EquipSlotMask ─────────────────────────────────────────────────────────────

/// A bitmask of equipment slots — useful for fast "which slots are occupied"
/// queries without iterating the full HashMap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EquipSlotMask(pub u32);

impl EquipSlotMask {
    pub fn empty() -> Self { Self(0) }
    pub fn full()  -> Self { Self((1u32 << EquipSlot::all().len()) - 1) }

    fn slot_bit(slot: EquipSlot) -> u32 {
        EquipSlot::all().iter().position(|&s| s == slot).unwrap_or(31) as u32
    }

    pub fn set(&mut self, slot: EquipSlot) {
        self.0 |= 1 << Self::slot_bit(slot);
    }

    pub fn clear(&mut self, slot: EquipSlot) {
        self.0 &= !(1 << Self::slot_bit(slot));
    }

    pub fn is_set(&self, slot: EquipSlot) -> bool {
        (self.0 >> Self::slot_bit(slot)) & 1 == 1
    }

    pub fn count(&self) -> u32 { self.0.count_ones() }

    pub fn union(self, other: Self) -> Self { Self(self.0 | other.0) }
    pub fn intersection(self, other: Self) -> Self { Self(self.0 & other.0) }
    pub fn difference(self, other: Self) -> Self { Self(self.0 & !other.0) }

    /// Build a mask from the currently occupied slots of an [`Equipment`].
    pub fn from_equipment(equipment: &Equipment) -> Self {
        let mut mask = Self::empty();
        for (slot, _) in equipment.iter() {
            mask.set(slot);
        }
        mask
    }
}

// ── EquipmentPresets ──────────────────────────────────────────────────────────

/// Factory helpers for constructing common equipment database setups used
/// in tests and demos.
pub struct EquipmentPresets;

impl EquipmentPresets {
    /// Minimal warrior starter loadout database (5 items covering main slots).
    pub fn warrior_starter_db() -> ItemEquipDatabase {
        let mut db = ItemEquipDatabase::new();

        db.register(ItemEquipInfo::new(ItemId(1001), ItemCategory::Weapon, EquipSlot::MainHand)
            .with_stat(StatKind::Attack, 18.0)
            .with_stat(StatKind::Strength, 3.0)
            .with_max_durability(100.0)
            .with_weight(3.5));

        db.register(ItemEquipInfo::new(ItemId(1002), ItemCategory::Armor, EquipSlot::Head)
            .with_stat(StatKind::Defense, 8.0)
            .with_stat(StatKind::Vitality, 2.0)
            .with_max_durability(100.0)
            .with_weight(2.0));

        db.register(ItemEquipInfo::new(ItemId(1003), ItemCategory::Armor, EquipSlot::Chest)
            .with_stat(StatKind::Defense, 20.0)
            .with_stat(StatKind::Vitality, 5.0)
            .with_max_durability(100.0)
            .with_weight(5.0));

        db.register(ItemEquipInfo::new(ItemId(1004), ItemCategory::Armor, EquipSlot::Legs)
            .with_stat(StatKind::Defense, 12.0)
            .with_stat(StatKind::Speed, 1.0)
            .with_max_durability(100.0)
            .with_weight(3.0));

        db.register(ItemEquipInfo::new(ItemId(1005), ItemCategory::Armor, EquipSlot::Feet)
            .with_stat(StatKind::Speed, 5.0)
            .with_stat(StatKind::Defense, 6.0)
            .with_max_durability(100.0)
            .with_weight(1.5));

        db
    }

    /// Inferno set database — 3-piece set for testing set bonuses.
    pub fn inferno_set_db() -> (ItemEquipDatabase, SetBonusRegistry) {
        let mut db = ItemEquipDatabase::new();
        let mut registry = SetBonusRegistry::new();

        let set_id = 10u32;

        db.register(ItemEquipInfo::new(ItemId(2001), ItemCategory::Armor, EquipSlot::Head)
            .with_stat(StatKind::MagicPower, 10.0)
            .with_set(set_id)
            .with_max_durability(100.0));

        db.register(ItemEquipInfo::new(ItemId(2002), ItemCategory::Armor, EquipSlot::Chest)
            .with_stat(StatKind::MagicPower, 20.0)
            .with_set(set_id)
            .with_max_durability(100.0));

        db.register(ItemEquipInfo::new(ItemId(2003), ItemCategory::Armor, EquipSlot::Legs)
            .with_stat(StatKind::MagicPower, 15.0)
            .with_set(set_id)
            .with_max_durability(100.0));

        registry.register(
            SetBonus::new(set_id, "Inferno Set", 2)
                .add_bonus(StatKind::MagicPower, 25.0)
                .add_bonus(StatKind::CritChance,  5.0),
        );
        registry.register(
            SetBonus::new(set_id, "Inferno Set", 3)
                .add_bonus(StatKind::MagicPower, 60.0)
                .add_bonus(StatKind::CritDamage, 20.0),
        );

        (db, registry)
    }
}

// ── Additional equipment tests ────────────────────────────────────────────────

#[cfg(test)]
mod extra_equipment_tests {
    use super::*;
    use crate::inventory::{ItemInstance, ItemId, StatKind};

    // ── ItemEquipDatabase ─────────────────────────────────────────────────────

    #[test]
    fn equip_db_register_and_get() {
        let db = EquipmentPresets::warrior_starter_db();
        assert_eq!(db.len(), 5);
        let entry = db.get(ItemId(1001)).unwrap();
        assert_eq!(entry.equip_slot, EquipSlot::MainHand);
    }

    #[test]
    fn equip_db_items_for_slot() {
        let db = EquipmentPresets::warrior_starter_db();
        let chest_items = db.items_for_slot(EquipSlot::Chest);
        assert_eq!(chest_items.len(), 1);
        assert_eq!(chest_items[0], ItemId(1003));
    }

    #[test]
    fn equip_db_items_in_set() {
        let (db, _) = EquipmentPresets::inferno_set_db();
        let set_items = db.items_in_set(10);
        assert_eq!(set_items.len(), 3);
    }

    // ── EquipmentManager ──────────────────────────────────────────────────────

    #[test]
    fn equipment_manager_equip_and_stat() {
        let mut mgr = EquipmentManager::new();
        mgr.item_db = EquipmentPresets::warrior_starter_db();

        let sword = ItemInstance::new(ItemId(1001)).with_durability(100.0);
        mgr.equip(EquipSlot::MainHand, sword, 1, "warrior", &|_| 100).unwrap();

        mgr.stat_calc.set_base(StatKind::Attack, 5.0);
        let total_atk = mgr.final_stat(StatKind::Attack);
        assert!((total_atk - 23.0).abs() < 1e-4, "expected 23, got {}", total_atk);
    }

    #[test]
    fn equipment_manager_full_set_bonus() {
        let (db, registry) = EquipmentPresets::inferno_set_db();
        let mut mgr = EquipmentManager::new();
        mgr.item_db = db;
        mgr.set_registry = registry;

        let head  = ItemInstance::new(ItemId(2001)).with_durability(100.0);
        let chest = ItemInstance::new(ItemId(2002)).with_durability(100.0);
        let legs  = ItemInstance::new(ItemId(2003)).with_durability(100.0);

        mgr.equip(EquipSlot::Head,  head,  1, "", &|_| 100).unwrap();
        mgr.equip(EquipSlot::Chest, chest, 1, "", &|_| 100).unwrap();
        mgr.equip(EquipSlot::Legs,  legs,  1, "", &|_| 100).unwrap();

        // Base from items: 10+20+15 = 45; 2-piece bonus: +25; 3-piece bonus: +60 → 130
        let magic_power = mgr.final_stat(StatKind::MagicPower);
        assert!((magic_power - 130.0).abs() < 1e-3, "expected 130.0, got {}", magic_power);
    }

    #[test]
    fn equipment_manager_tick_decays_durability() {
        let mut mgr = EquipmentManager::new();
        mgr.item_db = EquipmentPresets::warrior_starter_db();
        mgr.durability.wear_multiplier = 1.0;
        mgr.durability.auto_unequip_broken = false;

        let sword = ItemInstance::new(ItemId(1001)).with_durability(100.0);
        mgr.equip(EquipSlot::MainHand, sword, 1, "", &|_| 100).unwrap();

        mgr.tick(100.0); // 100 seconds
        let dur = mgr.equipment.get(EquipSlot::MainHand).unwrap().instance.durability.unwrap();
        assert!(dur < 100.0);
    }

    #[test]
    fn equipment_manager_event_log_populated() {
        let mut mgr = EquipmentManager::new();
        mgr.item_db = EquipmentPresets::warrior_starter_db();

        let sword = ItemInstance::new(ItemId(1001)).with_durability(100.0);
        mgr.equip(EquipSlot::MainHand, sword, 1, "", &|_| 100).unwrap();

        let events = mgr.event_log.drain();
        // At minimum: ItemEquipped + StatsChanged
        assert!(events.len() >= 2);
        assert!(events.iter().any(|e| matches!(e, EquipmentChangeEvent::ItemEquipped { .. })));
    }

    // ── EquipSlotMask ─────────────────────────────────────────────────────────

    #[test]
    fn slot_mask_set_and_check() {
        let mut mask = EquipSlotMask::empty();
        mask.set(EquipSlot::Head);
        mask.set(EquipSlot::Chest);
        assert!(mask.is_set(EquipSlot::Head));
        assert!(mask.is_set(EquipSlot::Chest));
        assert!(!mask.is_set(EquipSlot::Legs));
        assert_eq!(mask.count(), 2);
    }

    #[test]
    fn slot_mask_clear() {
        let mut mask = EquipSlotMask::empty();
        mask.set(EquipSlot::Head);
        mask.clear(EquipSlot::Head);
        assert!(!mask.is_set(EquipSlot::Head));
    }

    #[test]
    fn slot_mask_union_intersection() {
        let mut a = EquipSlotMask::empty();
        let mut b = EquipSlotMask::empty();
        a.set(EquipSlot::Head);
        a.set(EquipSlot::Chest);
        b.set(EquipSlot::Chest);
        b.set(EquipSlot::Legs);

        let u = a.union(b);
        assert!(u.is_set(EquipSlot::Head));
        assert!(u.is_set(EquipSlot::Chest));
        assert!(u.is_set(EquipSlot::Legs));

        let i = a.intersection(b);
        assert!(!i.is_set(EquipSlot::Head));
        assert!(i.is_set(EquipSlot::Chest));
        assert!(!i.is_set(EquipSlot::Legs));
    }

    #[test]
    fn slot_mask_from_equipment() {
        let mut eq = Equipment::new();
        let sword = ItemInstance::new(ItemId(1001)).with_durability(100.0);
        let info  = ItemEquipInfo::new(ItemId(1001), ItemCategory::Weapon, EquipSlot::MainHand);
        eq.equip(EquipSlot::MainHand, sword, &info, 1, "", &|_| 100).unwrap();

        let mask = EquipSlotMask::from_equipment(&eq);
        assert!(mask.is_set(EquipSlot::MainHand));
        assert!(!mask.is_set(EquipSlot::Head));
    }

    // ── EquipSlotIter ─────────────────────────────────────────────────────────

    #[test]
    fn slot_iter_visits_all_slots() {
        let count = EquipSlotIter::new().count();
        assert_eq!(count, EquipSlot::all().len());
    }

    // ── LoadoutManager overwrite ───────────────────────────────────────────────

    #[test]
    fn loadout_save_overwrites_existing() {
        let eq1 = Equipment::new();
        let mut mgr = LoadoutManager::new();
        mgr.save_current("default", &eq1);

        // Modify the equipment, save again under same name.
        let mut eq2 = Equipment::new();
        let sword = ItemInstance::new(ItemId(1001));
        let info  = ItemEquipInfo::new(ItemId(1001), ItemCategory::Weapon, EquipSlot::MainHand);
        eq2.equip(EquipSlot::MainHand, sword, &info, 1, "", &|_| 100).unwrap();
        mgr.save_current("default", &eq2);

        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.get("default").unwrap().item_count(), 1);
    }

    // ── Stat calculator set-bonus integration ─────────────────────────────────

    #[test]
    fn stat_calc_with_set_bonus_and_base() {
        let mut calc = StatCalculator::new();
        calc.set_base(StatKind::Intelligence, 20.0);

        let eq = Equipment::new();
        let mut set_totals = HashMap::new();
        set_totals.insert(StatKind::Intelligence, 15.0);

        let total = calc.final_value(StatKind::Intelligence, &eq, &set_totals);
        assert!((total - 35.0).abs() < 1e-4);
    }

    // ── EquipRestriction ──────────────────────────────────────────────────────

    #[test]
    fn restriction_no_class_list_passes_any_class() {
        let r = EquipRestriction::new().with_min_level(1);
        let result = r.check(5, "druid", &|_| 100);
        assert!(result.is_ok());
    }

    #[test]
    fn restriction_multiple_stat_requirements() {
        let r = EquipRestriction::new()
            .require_stat(StatKind::Strength, 30)
            .require_stat(StatKind::Dexterity, 20);

        // Meets Strength but not Dexterity.
        let result = r.check(1, "", &|stat| match stat {
            StatKind::Strength  => 35,
            StatKind::Dexterity => 10,
            _                   => 100,
        });
        assert!(matches!(result, Err(EquipError::StatRequirement {
            stat: StatKind::Dexterity, required: 20, have: 10
        })));
    }

    // ── Two-hand round-trip ────────────────────────────────────────────────────

    #[test]
    fn equip_two_hander_then_one_hander_round_trip() {
        let mut eq = Equipment::new();

        // 1. Equip two-hander.
        let th = ItemInstance::new(ItemId(50));
        let th_info = ItemEquipInfo::new(ItemId(50), ItemCategory::Weapon, EquipSlot::TwoHand);
        eq.equip(EquipSlot::TwoHand, th, &th_info, 1, "", &|_| 100).unwrap();
        assert!(eq.is_slot_filled(EquipSlot::TwoHand));

        // 2. Equip mainhand (displaces two-hander).
        let mh = ItemInstance::new(ItemId(51));
        let mh_info = ItemEquipInfo::new(ItemId(51), ItemCategory::Weapon, EquipSlot::MainHand);
        let displaced = eq.equip(EquipSlot::MainHand, mh, &mh_info, 1, "", &|_| 100).unwrap();
        assert!(!eq.is_slot_filled(EquipSlot::TwoHand));
        assert!(eq.is_slot_filled(EquipSlot::MainHand));
        assert_eq!(displaced.iter().filter(|i| i.def_id == ItemId(50)).count(), 1);
    }

    // ── Durability system: partial repair cost ─────────────────────────────────

    #[test]
    fn repair_cost_zero_when_full() {
        let eq = Equipment::new();
        let dur = DurabilitySystem::new();
        let cost = dur.repair_cost(&eq, EquipSlot::Head, 100);
        assert_eq!(cost, 0);
    }

    #[test]
    fn repair_cost_nonzero_when_damaged() {
        let mut eq = Equipment::new();
        let inst = ItemInstance::new(ItemId(1)).with_durability(50.0);
        let info = ItemEquipInfo::new(ItemId(1), ItemCategory::Armor, EquipSlot::Head)
            .with_max_durability(100.0);
        eq.equip(EquipSlot::Head, inst, &info, 1, "", &|_| 100).unwrap();
        let dur = DurabilitySystem::new();
        let cost = dur.repair_cost(&eq, EquipSlot::Head, 200);
        assert!(cost > 0, "repair cost should be > 0 for a damaged item");
    }

    // ── Equipment unequip_all ─────────────────────────────────────────────────

    #[test]
    fn unequip_all_returns_all_items() {
        let mut eq = Equipment::new();
        let make = |id: u32, slot: EquipSlot| -> (ItemInstance, ItemEquipInfo) {
            let inst = ItemInstance::new(ItemId(id));
            let info = ItemEquipInfo::new(ItemId(id), ItemCategory::Armor, slot);
            (inst, info)
        };
        let (s1, i1) = make(1, EquipSlot::Head);
        let (s2, i2) = make(2, EquipSlot::Chest);
        eq.equip(EquipSlot::Head,  s1, &i1, 1, "", &|_| 100).unwrap();
        eq.equip(EquipSlot::Chest, s2, &i2, 1, "", &|_| 100).unwrap();

        let all = eq.unequip_all();
        assert_eq!(all.len(), 2);
        assert!(eq.slots.is_empty());
    }

    // ── Loadout summary ───────────────────────────────────────────────────────

    #[test]
    fn loadout_summary_not_empty() {
        let mut lo = Loadout::new("test");
        let mut eq = Equipment::new();
        let inst = ItemInstance::new(ItemId(1));
        let info = ItemEquipInfo::new(ItemId(1), ItemCategory::Weapon, EquipSlot::MainHand);
        eq.equip(EquipSlot::MainHand, inst, &info, 1, "", &|_| 100).unwrap();
        lo.capture(&eq);
        let summary = lo.summary();
        assert!(summary.contains("test"));
        assert!(summary.contains("Main Hand"));
    }

    // ── EquipmentChangeEvent coverage ─────────────────────────────────────────

    #[test]
    fn event_log_unequip_emits_event() {
        let mut mgr = EquipmentManager::new();
        mgr.item_db = EquipmentPresets::warrior_starter_db();
        let sword = ItemInstance::new(ItemId(1001)).with_durability(100.0);
        mgr.equip(EquipSlot::MainHand, sword, 1, "", &|_| 100).unwrap();
        mgr.event_log.drain(); // flush equip events

        mgr.unequip(EquipSlot::MainHand);
        let events = mgr.event_log.drain();
        assert!(events.iter().any(|e| matches!(e, EquipmentChangeEvent::ItemUnequipped { .. })));
    }

    #[test]
    fn event_log_stats_changed_emitted() {
        let mut mgr = EquipmentManager::new();
        mgr.item_db = EquipmentPresets::warrior_starter_db();
        let sword = ItemInstance::new(ItemId(1001)).with_durability(100.0);
        mgr.equip(EquipSlot::MainHand, sword, 1, "", &|_| 100).unwrap();
        let events = mgr.event_log.drain();
        assert!(events.iter().any(|e| matches!(e, EquipmentChangeEvent::StatsChanged)));
    }

    // ── all_stats smoke test ───────────────────────────────────────────────────

    #[test]
    fn all_stats_returns_all_stat_kinds() {
        let mgr = EquipmentManager::new();
        let stats = mgr.all_stats();
        for stat in StatKind::all() {
            assert!(stats.contains_key(stat), "missing stat {:?}", stat);
        }
    }

    // ── EquipmentPresets smoke ────────────────────────────────────────────────

    #[test]
    fn warrior_starter_db_has_five_entries() {
        let db = EquipmentPresets::warrior_starter_db();
        assert_eq!(db.len(), 5);
    }

    #[test]
    fn inferno_set_has_two_bonus_tiers() {
        let (_, registry) = EquipmentPresets::inferno_set_db();
        // Two-piece and three-piece bonuses registered.
        let two_piece_ids = [ItemId(2001), ItemId(2002)];
        let bonuses_2 = registry.active_bonuses(&two_piece_ids, |id| {
            if matches!(id.raw(), 2001..=2003) { Some(10) } else { None }
        });
        assert!(bonuses_2.contains_key(&StatKind::MagicPower));

        let three_piece_ids = [ItemId(2001), ItemId(2002), ItemId(2003)];
        let bonuses_3 = registry.active_bonuses(&three_piece_ids, |id| {
            if matches!(id.raw(), 2001..=2003) { Some(10) } else { None }
        });
        assert!(bonuses_3.contains_key(&StatKind::CritDamage));
    }

    // ── Durability system edge cases ──────────────────────────────────────────

    #[test]
    fn durability_tick_zero_delta() {
        let mut eq = Equipment::new();
        let inst = ItemInstance::new(ItemId(1)).with_durability(75.0);
        let info = ItemEquipInfo::new(ItemId(1), ItemCategory::Armor, EquipSlot::Head);
        eq.equip(EquipSlot::Head, inst, &info, 1, "", &|_| 100).unwrap();

        let mut dur = DurabilitySystem::new();
        dur.auto_unequip_broken = false;
        dur.tick_durability(&mut eq, 0.0);

        let d = eq.get(EquipSlot::Head).unwrap().instance.durability.unwrap();
        assert!((d - 75.0).abs() < 1e-5, "zero delta should not change durability");
    }

    #[test]
    fn durability_repair_clamps_at_100() {
        let mut eq = Equipment::new();
        let inst = ItemInstance::new(ItemId(1)).with_durability(80.0);
        let info = ItemEquipInfo::new(ItemId(1), ItemCategory::Armor, EquipSlot::Head);
        eq.equip(EquipSlot::Head, inst, &info, 1, "", &|_| 100).unwrap();

        let dur = DurabilitySystem::new();
        dur.repair(&mut eq, EquipSlot::Head, 100.0); // would go over 100
        let d = eq.get(EquipSlot::Head).unwrap().instance.durability.unwrap();
        assert!((d - 100.0).abs() < 1e-5);
    }

    #[test]
    fn needs_repair_empty_list_when_all_fine() {
        let mut eq = Equipment::new();
        let inst = ItemInstance::new(ItemId(1)).with_durability(100.0);
        let info = ItemEquipInfo::new(ItemId(1), ItemCategory::Armor, EquipSlot::Head);
        eq.equip(EquipSlot::Head, inst, &info, 1, "", &|_| 100).unwrap();
        let dur = DurabilitySystem::new();
        let slots = dur.needs_repair(&eq, 30.0);
        assert!(slots.is_empty());
    }

    // ── StatCalculator base stats ─────────────────────────────────────────────

    #[test]
    fn stat_calc_get_base_default_zero() {
        let calc = StatCalculator::new();
        assert_eq!(calc.get_base(StatKind::Strength), 0.0);
    }

    #[test]
    fn stat_calc_all_final_sum() {
        let mut calc = StatCalculator::new();
        calc.set_base(StatKind::Speed, 10.0);
        let eq = Equipment::new();
        let set_totals = HashMap::new();
        let all = calc.all_final(&eq, &set_totals);
        assert!((all[&StatKind::Speed] - 10.0).abs() < 1e-4);
        assert!((all[&StatKind::Strength] - 0.0).abs() < 1e-4);
    }

    // ── SetBonusRegistry descriptions ─────────────────────────────────────────

    #[test]
    fn set_bonus_descriptions_non_empty_when_active() {
        let mut registry = SetBonusRegistry::new();
        registry.register(
            SetBonus::new(5, "Shadow Set", 2)
                .add_bonus(StatKind::CritChance, 10.0),
        );
        let items = vec![ItemId(1), ItemId(2)];
        let descs = registry.active_descriptions(&items, |id| {
            if id.raw() <= 2 { Some(5) } else { None }
        });
        assert!(!descs.is_empty());
        assert!(descs[0].contains("Shadow Set"));
    }

    #[test]
    fn pieces_equipped_counts_correctly() {
        let registry = SetBonusRegistry::new();
        let items = vec![ItemId(1), ItemId(2), ItemId(3)];
        let count = registry.pieces_equipped(7, &items, |id| {
            if id.raw() <= 2 { Some(7) } else { None }
        });
        assert_eq!(count, 2);
    }
}
