//! Inventory system — items, containers, and equipment.
//!
//! This module defines the fundamental item data model and provides
//! re-exports for the two major subsystems:
//!
//! - [`container`] — slot-based inventory bags, loot tables, transactions
//! - [`equipment`] — character equipment slots, set bonuses, loadouts, durability
//!
//! # Overview
//!
//! Items are composed of two layers:
//!
//! 1. **[`ItemDef`]** — static, read-only definition registered in an [`ItemDatabase`].
//! 2. **[`ItemInstance`]** — a live instance pointing to a definition, carrying
//!    per-instance state (stack size, durability, enchantments).
//!
//! All item definitions are identified by [`ItemId`]; all stack groups within
//! a container are identified by [`StackId`].

pub mod container;
pub mod equipment;

// ── Re-exports ─────────────────────────────────────────────────────────────────

pub use container::{
    Inventory, InventoryError, InventoryTransaction, TransactionError,
    Slot, SlotIndex, ContainerConfig,
    Loot, LootTable, LootEntry, LootCondition,
};

pub use equipment::{
    Equipment, EquipError, EquipSlot, EquippedItem, EquipRestriction,
    SetBonus, SetBonusRegistry,
    Loadout, LoadoutManager,
    DurabilitySystem,
};

// ── Lightweight seeded RNG ──────────────────────────────────────────────────────
//
// A self-contained xoshiro256** implementation so the inventory module has no
// external dependency on the `rand` crate.  Mirrors the implementation in
// `crate::procedural`.

/// Lightweight seeded pseudo-random number generator (xoshiro256**).
#[derive(Clone, Debug)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    /// Create a new [`Rng`] from a 64-bit seed.
    pub fn new(seed: u64) -> Self {
        let mut s = seed;
        let mut next = || {
            s = s.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^ (z >> 31)
        };
        Self { state: [next(), next(), next(), next()] }
    }

    #[inline]
    fn rol64(x: u64, k: u32) -> u64 {
        (x << k) | (x >> (64 - k))
    }

    /// Produce the next raw u64.
    pub fn next_u64(&mut self) -> u64 {
        let result = Self::rol64(self.state[1].wrapping_mul(5), 7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = Self::rol64(self.state[3], 45);
        result
    }

    /// f32 in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32
    }

    /// f32 in `[min, max)`.
    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    /// usize in `[0, n)`.
    pub fn range_usize(&mut self, n: usize) -> usize {
        if n == 0 { return 0; }
        (self.next_u64() % n as u64) as usize
    }

    /// u32 in `[min, max]`.
    pub fn range_u32(&mut self, min: u32, max: u32) -> u32 {
        if max <= min { return min; }
        min + (self.next_u64() % (max - min + 1) as u64) as u32
    }

    /// Bool with probability `p ∈ [0, 1]`.
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.range_usize(i + 1);
            slice.swap(i, j);
        }
    }

    /// Pick a random element from a non-empty slice.
    pub fn pick<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() { return None; }
        Some(&slice[self.range_usize(slice.len())])
    }
}

// ── ItemId / StackId ────────────────────────────────────────────────────────────

/// Opaque identifier for an item *definition* in the [`ItemDatabase`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub u32);

impl ItemId {
    pub fn new(id: u32) -> Self { Self(id) }
    pub fn raw(self) -> u32 { self.0 }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "item:{}", self.0)
    }
}

/// Opaque identifier for a particular stack group within a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackId(pub u32);

impl StackId {
    pub fn new(id: u32) -> Self { Self(id) }
    pub fn raw(self) -> u32 { self.0 }
}

// ── Rarity ─────────────────────────────────────────────────────────────────────

/// Item rarity, controlling drop rates and stat multipliers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl ItemRarity {
    pub fn display_name(self) -> &'static str {
        match self {
            ItemRarity::Common    => "Common",
            ItemRarity::Uncommon  => "Uncommon",
            ItemRarity::Rare      => "Rare",
            ItemRarity::Epic      => "Epic",
            ItemRarity::Legendary => "Legendary",
        }
    }

    /// Visual color hint as (r, g, b) bytes.
    pub fn color(self) -> (u8, u8, u8) {
        match self {
            ItemRarity::Common    => (180, 180, 180),
            ItemRarity::Uncommon  => (30,  200,  30),
            ItemRarity::Rare      => (20,  100, 255),
            ItemRarity::Epic      => (160,   0, 255),
            ItemRarity::Legendary => (255, 165,   0),
        }
    }

    /// Stat magnitude multiplier.
    pub fn stat_multiplier(self) -> f32 {
        match self {
            ItemRarity::Common    => 1.00,
            ItemRarity::Uncommon  => 1.25,
            ItemRarity::Rare      => 1.60,
            ItemRarity::Epic      => 2.10,
            ItemRarity::Legendary => 3.00,
        }
    }

    /// Relative drop-weight (higher = more common).
    pub fn drop_weight(self) -> f32 {
        match self {
            ItemRarity::Common    => 60.0,
            ItemRarity::Uncommon  => 25.0,
            ItemRarity::Rare      =>  9.0,
            ItemRarity::Epic      =>  3.0,
            ItemRarity::Legendary =>  1.0,
        }
    }
}

impl std::fmt::Display for ItemRarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

// ── ItemCategory ───────────────────────────────────────────────────────────────

/// Broad gameplay category of an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    Weapon,
    Armor,
    Consumable,
    Material,
    Quest,
    Misc,
}

impl ItemCategory {
    pub fn display_name(self) -> &'static str {
        match self {
            ItemCategory::Weapon     => "Weapon",
            ItemCategory::Armor      => "Armor",
            ItemCategory::Consumable => "Consumable",
            ItemCategory::Material   => "Material",
            ItemCategory::Quest      => "Quest",
            ItemCategory::Misc       => "Miscellaneous",
        }
    }

    /// Whether items of this category can be stacked by default.
    pub fn is_stackable(self) -> bool {
        matches!(self, ItemCategory::Consumable | ItemCategory::Material | ItemCategory::Misc)
    }

    /// Whether items of this category are equippable.
    pub fn is_equippable(self) -> bool {
        matches!(self, ItemCategory::Weapon | ItemCategory::Armor)
    }
}

impl std::fmt::Display for ItemCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

// ── StatKind ───────────────────────────────────────────────────────────────────

/// The kind of character statistic an enchantment or bonus modifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatKind {
    Strength,
    Dexterity,
    Intelligence,
    Vitality,
    Attack,
    Defense,
    Speed,
    MagicPower,
    CritChance,
    CritDamage,
}

impl StatKind {
    pub fn display_name(self) -> &'static str {
        match self {
            StatKind::Strength     => "Strength",
            StatKind::Dexterity    => "Dexterity",
            StatKind::Intelligence => "Intelligence",
            StatKind::Vitality     => "Vitality",
            StatKind::Attack       => "Attack",
            StatKind::Defense      => "Defense",
            StatKind::Speed        => "Speed",
            StatKind::MagicPower   => "Magic Power",
            StatKind::CritChance   => "Crit Chance",
            StatKind::CritDamage   => "Crit Damage",
        }
    }

    /// All variants in a stable order.
    pub fn all() -> &'static [StatKind] {
        use StatKind::*;
        &[
            Strength, Dexterity, Intelligence, Vitality,
            Attack, Defense, Speed, MagicPower, CritChance, CritDamage,
        ]
    }
}

impl std::fmt::Display for StatKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

// ── Enchantment ────────────────────────────────────────────────────────────────

/// A magical property applied to an [`ItemInstance`].
#[derive(Debug, Clone)]
pub struct Enchantment {
    /// Human-readable enchantment name, e.g. `"Flaming"`.
    pub name: String,
    /// Which stat this enchantment improves.
    pub stat: StatKind,
    /// The numeric bonus magnitude.
    pub magnitude: f32,
    /// If `Some`, the enchantment expires after this many seconds.
    pub duration: Option<f32>,
}

impl Enchantment {
    pub fn new(name: impl Into<String>, stat: StatKind, magnitude: f32) -> Self {
        Self { name: name.into(), stat, magnitude, duration: None }
    }

    pub fn with_duration(mut self, secs: f32) -> Self {
        self.duration = Some(secs);
        self
    }

    /// Whether this enchantment has expired given elapsed seconds.
    pub fn is_expired(&self, elapsed: f32) -> bool {
        self.duration.map(|d| elapsed >= d).unwrap_or(false)
    }

    /// Total effective bonus, scaled by a 0..1 factor (e.g. item quality).
    pub fn effective_bonus(&self, quality_factor: f32) -> f32 {
        self.magnitude * quality_factor.clamp(0.0, 1.0)
    }
}

// ── ItemDef ────────────────────────────────────────────────────────────────────

/// Static, immutable definition of an item type.
///
/// Stored in [`ItemDatabase`]; never modified after registration.  All
/// instances of the same item share one [`ItemDef`].
#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id:          ItemId,
    pub name:        String,
    pub description: String,
    /// Maximum number of copies that can share one inventory slot.
    pub max_stack:   u32,
    /// Weight per single unit, in arbitrary game-world units.
    pub weight:      f32,
    /// Base gold/currency value per single unit.
    pub value:       u32,
    pub rarity:      ItemRarity,
    pub category:    ItemCategory,
}

impl ItemDef {
    pub fn new(
        id:       ItemId,
        name:     impl Into<String>,
        category: ItemCategory,
        rarity:   ItemRarity,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            description: String::new(),
            max_stack: if category.is_stackable() { 99 } else { 1 },
            weight: 1.0,
            value: 10,
            rarity,
            category,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into(); self
    }

    pub fn with_max_stack(mut self, n: u32) -> Self {
        self.max_stack = n.max(1); self
    }

    pub fn with_weight(mut self, w: f32) -> Self {
        self.weight = w.max(0.0); self
    }

    pub fn with_value(mut self, v: u32) -> Self {
        self.value = v; self
    }

    /// True if multiple instances can occupy a single slot.
    pub fn is_stackable(&self) -> bool {
        self.max_stack > 1
    }

    /// True if the item can be worn in an equipment slot.
    pub fn is_equippable(&self) -> bool {
        self.category.is_equippable()
    }

    /// Effective sell price — rarity-scaled value.
    pub fn sell_price(&self) -> u32 {
        ((self.value as f32) * self.rarity.stat_multiplier()) as u32
    }
}

// ── ItemInstance ───────────────────────────────────────────────────────────────

/// A live instance of an item, as stored in inventories or equipment.
#[derive(Debug, Clone)]
pub struct ItemInstance {
    /// Which definition this instance belongs to.
    pub def_id:       ItemId,
    /// Number of copies in this stack (1 for non-stackable items).
    pub stack_size:   u32,
    /// Current durability, if the item type uses a durability model.
    pub durability:   Option<f32>,
    /// Enchantments applied to this specific instance.
    pub enchantments: Vec<Enchantment>,
}

impl ItemInstance {
    pub fn new(def_id: ItemId) -> Self {
        Self { def_id, stack_size: 1, durability: None, enchantments: Vec::new() }
    }

    pub fn new_stack(def_id: ItemId, qty: u32) -> Self {
        Self { def_id, stack_size: qty.max(1), durability: None, enchantments: Vec::new() }
    }

    pub fn with_durability(mut self, dur: f32) -> Self {
        self.durability = Some(dur.max(0.0)); self
    }

    pub fn with_enchantment(mut self, e: Enchantment) -> Self {
        self.enchantments.push(e); self
    }

    /// Whether this instance is fully broken (durability == 0).
    pub fn is_broken(&self) -> bool {
        self.durability.map(|d| d <= 0.0).unwrap_or(false)
    }

    /// Durability as a 0..1 fraction.  Returns 1.0 for indestructible items.
    pub fn durability_fraction(&self) -> f32 {
        self.durability.map(|d| d.clamp(0.0, 100.0) / 100.0).unwrap_or(1.0)
    }

    /// Total stat bonus contributed by enchantments for a given stat.
    pub fn enchantment_bonus(&self, stat: StatKind) -> f32 {
        self.enchantments
            .iter()
            .filter(|e| e.stat == stat)
            .map(|e| e.magnitude)
            .sum()
    }

    /// Remove enchantments that have expired given total elapsed seconds.
    pub fn prune_expired_enchantments(&mut self, elapsed: f32) {
        self.enchantments.retain(|e| !e.is_expired(elapsed));
    }

    /// Split `qty` units off this stack into a new instance.
    ///
    /// Returns `None` if `qty >= self.stack_size`.
    pub fn split(&mut self, qty: u32) -> Option<ItemInstance> {
        if qty == 0 || qty >= self.stack_size { return None; }
        self.stack_size -= qty;
        let mut child = self.clone();
        child.stack_size = qty;
        Some(child)
    }

    /// Attempt to merge `other` into this stack.
    ///
    /// Returns the leftover stack size that could not be absorbed (0 means
    /// fully merged).  `max_stack` must be provided from the item's [`ItemDef`].
    pub fn merge(&mut self, other: &mut ItemInstance, max_stack: u32) -> u32 {
        if self.def_id != other.def_id { return other.stack_size; }
        let can_absorb = max_stack.saturating_sub(self.stack_size);
        let take = can_absorb.min(other.stack_size);
        self.stack_size  += take;
        other.stack_size -= take;
        other.stack_size
    }
}

// ── ItemDatabase ───────────────────────────────────────────────────────────────

/// Registry mapping [`ItemId`] → [`ItemDef`].
///
/// Constructed once at startup; item definitions are never removed.
#[derive(Debug, Clone, Default)]
pub struct ItemDatabase {
    items: std::collections::HashMap<ItemId, ItemDef>,
}

impl ItemDatabase {
    pub fn new() -> Self {
        Self { items: std::collections::HashMap::new() }
    }

    /// Register a new item definition.  Panics in debug builds if `id` is
    /// already registered.
    pub fn register(&mut self, def: ItemDef) {
        debug_assert!(
            !self.items.contains_key(&def.id),
            "ItemDatabase: duplicate registration of {:?}", def.id,
        );
        self.items.insert(def.id, def);
    }

    /// Look up a definition by id.
    pub fn get(&self, id: ItemId) -> Option<&ItemDef> {
        self.items.get(&id)
    }

    /// Look up by exact name (O(n), use sparingly).
    pub fn lookup_by_name(&self, name: &str) -> Option<&ItemDef> {
        self.items.values().find(|d| d.name == name)
    }

    /// All definitions with the given category.
    pub fn items_of_category(&self, cat: ItemCategory) -> Vec<&ItemDef> {
        self.items.values().filter(|d| d.category == cat).collect()
    }

    /// All definitions of the given rarity.
    pub fn items_of_rarity(&self, rarity: ItemRarity) -> Vec<&ItemDef> {
        self.items.values().filter(|d| d.rarity == rarity).collect()
    }

    /// Number of registered item definitions.
    pub fn len(&self) -> usize { self.items.len() }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    /// Iterate all registered item ids.
    pub fn all_ids(&self) -> impl Iterator<Item = ItemId> + '_ {
        self.items.keys().copied()
    }
}

// ── ItemFilter ─────────────────────────────────────────────────────────────────

/// Builder for filtering sets of item ids against an [`ItemDatabase`].
///
/// ```rust
/// # use proof_engine::inventory::{ItemFilter, ItemRarity, ItemCategory, ItemDatabase};
/// # let db = ItemDatabase::new();
/// let rare_weapons: Vec<_> = ItemFilter::new()
///     .by_rarity(ItemRarity::Rare)
///     .by_category(ItemCategory::Weapon)
///     .apply_db(&db);
/// ```
#[derive(Debug, Clone, Default)]
pub struct ItemFilter {
    rarity:    Option<ItemRarity>,
    category:  Option<ItemCategory>,
    min_value: Option<u32>,
    max_value: Option<u32>,
    min_weight: Option<f32>,
    max_weight: Option<f32>,
    stackable_only: bool,
    equippable_only: bool,
}

impl ItemFilter {
    pub fn new() -> Self { Self::default() }

    pub fn by_rarity(mut self, r: ItemRarity) -> Self {
        self.rarity = Some(r); self
    }

    pub fn by_category(mut self, c: ItemCategory) -> Self {
        self.category = Some(c); self
    }

    pub fn by_min_value(mut self, v: u32) -> Self {
        self.min_value = Some(v); self
    }

    pub fn by_max_value(mut self, v: u32) -> Self {
        self.max_value = Some(v); self
    }

    pub fn by_min_weight(mut self, w: f32) -> Self {
        self.min_weight = Some(w); self
    }

    pub fn by_max_weight(mut self, w: f32) -> Self {
        self.max_weight = Some(w); self
    }

    pub fn stackable_only(mut self) -> Self {
        self.stackable_only = true; self
    }

    pub fn equippable_only(mut self) -> Self {
        self.equippable_only = true; self
    }

    /// Apply filter to an explicit list of [`ItemId`]s against `db`.
    pub fn apply(&self, ids: &[ItemId], db: &ItemDatabase) -> Vec<ItemId> {
        ids.iter().copied().filter(|&id| {
            if let Some(def) = db.get(id) {
                self.matches(def)
            } else {
                false
            }
        }).collect()
    }

    /// Apply filter to *all* items registered in `db`.
    pub fn apply_db(&self, db: &ItemDatabase) -> Vec<ItemId> {
        db.items.values()
            .filter(|def| self.matches(def))
            .map(|def| def.id)
            .collect()
    }

    fn matches(&self, def: &ItemDef) -> bool {
        if let Some(r) = self.rarity {
            if def.rarity != r { return false; }
        }
        if let Some(c) = self.category {
            if def.category != c { return false; }
        }
        if let Some(min) = self.min_value {
            if def.value < min { return false; }
        }
        if let Some(max) = self.max_value {
            if def.value > max { return false; }
        }
        if let Some(min) = self.min_weight {
            if def.weight < min { return false; }
        }
        if let Some(max) = self.max_weight {
            if def.weight > max { return false; }
        }
        if self.stackable_only && !def.is_stackable() { return false; }
        if self.equippable_only && !def.is_equippable() { return false; }
        true
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Linearly interpolate between `a` and `b` by factor `t ∈ [0, 1]`.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_db() -> ItemDatabase {
        let mut db = ItemDatabase::new();
        db.register(
            ItemDef::new(ItemId(1), "Iron Sword", ItemCategory::Weapon, ItemRarity::Common)
                .with_value(50)
                .with_weight(3.5),
        );
        db.register(
            ItemDef::new(ItemId(2), "Health Potion", ItemCategory::Consumable, ItemRarity::Common)
                .with_max_stack(20)
                .with_value(25)
                .with_weight(0.2),
        );
        db.register(
            ItemDef::new(ItemId(3), "Dragon Scale", ItemCategory::Armor, ItemRarity::Rare)
                .with_value(500)
                .with_weight(8.0),
        );
        db
    }

    #[test]
    fn item_db_register_and_get() {
        let db = make_db();
        assert_eq!(db.len(), 3);
        let sword = db.get(ItemId(1)).unwrap();
        assert_eq!(sword.name, "Iron Sword");
        assert_eq!(sword.category, ItemCategory::Weapon);
    }

    #[test]
    fn item_db_lookup_by_name() {
        let db = make_db();
        assert!(db.lookup_by_name("Health Potion").is_some());
        assert!(db.lookup_by_name("Nonexistent").is_none());
    }

    #[test]
    fn item_db_category_filter() {
        let db = make_db();
        let consumables = db.items_of_category(ItemCategory::Consumable);
        assert_eq!(consumables.len(), 1);
        assert_eq!(consumables[0].name, "Health Potion");
    }

    #[test]
    fn item_instance_split_merge() {
        let mut a = ItemInstance::new_stack(ItemId(2), 10);
        let b = a.split(4).unwrap();
        assert_eq!(a.stack_size, 6);
        assert_eq!(b.stack_size, 4);

        let mut a2 = ItemInstance::new_stack(ItemId(2), 15);
        let mut b2 = ItemInstance::new_stack(ItemId(2), 10);
        let leftover = a2.merge(&mut b2, 20);
        assert_eq!(a2.stack_size, 20);
        assert_eq!(leftover, 5);
    }

    #[test]
    fn item_instance_enchantment_bonus() {
        let inst = ItemInstance::new(ItemId(1))
            .with_enchantment(Enchantment::new("Flaming", StatKind::Attack, 15.0))
            .with_enchantment(Enchantment::new("Sharp",   StatKind::Attack, 10.0));
        assert!((inst.enchantment_bonus(StatKind::Attack) - 25.0).abs() < 1e-5);
        assert!((inst.enchantment_bonus(StatKind::Defense)).abs() < 1e-5);
    }

    #[test]
    fn item_filter_by_category_and_rarity() {
        let db = make_db();
        let ids: Vec<ItemId> = db.all_ids().collect();
        let result = ItemFilter::new()
            .by_category(ItemCategory::Weapon)
            .apply(&ids, &db);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ItemId(1));
    }

    #[test]
    fn item_filter_by_min_value() {
        let db = make_db();
        let result = ItemFilter::new().by_min_value(100).apply_db(&db);
        assert_eq!(result.len(), 1); // only Dragon Scale
    }

    #[test]
    fn rng_range_u32_bounds() {
        let mut rng = Rng::new(0xDEAD_BEEF);
        for _ in 0..1000 {
            let v = rng.range_u32(5, 10);
            assert!(v >= 5 && v <= 10);
        }
    }

    #[test]
    fn rarity_ordering() {
        assert!(ItemRarity::Common < ItemRarity::Uncommon);
        assert!(ItemRarity::Uncommon < ItemRarity::Rare);
        assert!(ItemRarity::Rare < ItemRarity::Epic);
        assert!(ItemRarity::Epic < ItemRarity::Legendary);
    }

    #[test]
    fn item_def_stackable_flag() {
        let db = make_db();
        let sword = db.get(ItemId(1)).unwrap();
        let potion = db.get(ItemId(2)).unwrap();
        assert!(!sword.is_stackable());
        assert!(potion.is_stackable());
    }

    #[test]
    fn enchantment_expiry() {
        let e = Enchantment::new("Temp", StatKind::Speed, 5.0).with_duration(10.0);
        assert!(!e.is_expired(9.9));
        assert!(e.is_expired(10.0));
        assert!(e.is_expired(15.0));
    }

    #[test]
    fn lerp_helper() {
        assert!((lerp(0.0, 100.0, 0.5) - 50.0).abs() < 1e-5);
        assert!((lerp(10.0, 20.0, 0.0) - 10.0).abs() < 1e-5);
        assert!((lerp(10.0, 20.0, 1.0) - 20.0).abs() < 1e-5);
    }
}
