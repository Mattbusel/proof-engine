//! Slot-based inventory containers, loot tables, and atomic transactions.
//!
//! # Design
//!
//! An [`Inventory`] holds a fixed array of [`Slot`]s, each of which may be
//! empty or contain an [`ItemInstance`].  Most mutations return
//! `Result<_, InventoryError>` so callers can handle full-bag scenarios
//! gracefully.
//!
//! [`InventoryTransaction`] provides an atomic "apply several changes to one
//! or more inventories" primitive — either every change commits or every
//! change rolls back.
//!
//! [`LootTable`] generates weighted random loot rolls; preset constructors
//! cover the most common dungeon-chest archetypes.

use std::collections::HashMap;
use std::collections::VecDeque;

use super::{ItemId, ItemInstance, ItemCategory, ItemRarity, Rng};

// ── SlotIndex ──────────────────────────────────────────────────────────────────

/// Strongly-typed index into a container's slot array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotIndex(pub u32);

impl SlotIndex {
    pub fn new(i: u32) -> Self { Self(i) }
    pub fn raw(self) -> usize { self.0 as usize }
}

impl std::fmt::Display for SlotIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "slot[{}]", self.0)
    }
}

// ── ContainerConfig ────────────────────────────────────────────────────────────

/// Configuration that shapes how an [`Inventory`] behaves.
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Total number of slots.
    pub max_slots: u32,
    /// If set, the bag cannot hold more than this cumulative item weight.
    pub max_weight: Option<f32>,
    /// If set, only items whose category appears in this list are accepted.
    pub allowed_categories: Option<Vec<ItemCategory>>,
    /// When true no mutations are permitted (read-only container).
    pub locked: bool,
}

impl ContainerConfig {
    pub fn new(max_slots: u32) -> Self {
        Self { max_slots, max_weight: None, allowed_categories: None, locked: false }
    }

    pub fn with_max_weight(mut self, w: f32) -> Self {
        self.max_weight = Some(w); self
    }

    pub fn with_allowed_categories(mut self, cats: Vec<ItemCategory>) -> Self {
        self.allowed_categories = Some(cats); self
    }

    pub fn locked(mut self) -> Self {
        self.locked = true; self
    }

    /// A small personal bag: 20 slots, 50 weight units.
    pub fn small_bag() -> Self {
        Self::new(20).with_max_weight(50.0)
    }

    /// A standard adventurer's pack: 40 slots, 120 weight units.
    pub fn standard_pack() -> Self {
        Self::new(40).with_max_weight(120.0)
    }

    /// A bank vault: 200 slots, unlimited weight.
    pub fn bank_vault() -> Self {
        Self::new(200)
    }

    /// A pouch that holds only consumables: 30 slots, 20 weight units.
    pub fn consumable_pouch() -> Self {
        Self::new(30)
            .with_max_weight(20.0)
            .with_allowed_categories(vec![ItemCategory::Consumable])
    }
}

// ── Slot ───────────────────────────────────────────────────────────────────────

/// A single slot in an inventory.
#[derive(Debug, Clone)]
pub struct Slot {
    pub slot_id: u32,
    pub item:    Option<ItemInstance>,
}

impl Slot {
    pub fn empty(slot_id: u32) -> Self {
        Self { slot_id, item: None }
    }

    pub fn is_empty(&self) -> bool { self.item.is_none() }

    pub fn is_occupied(&self) -> bool { self.item.is_some() }

    /// Current stack size (0 for empty slots).
    pub fn stack_size(&self) -> u32 {
        self.item.as_ref().map(|i| i.stack_size).unwrap_or(0)
    }
}

// ── InventoryError ─────────────────────────────────────────────────────────────

/// All ways an inventory mutation can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryError {
    /// No empty slot available.
    Full,
    /// Adding the item would exceed the weight limit.
    WeightExceeded,
    /// The slot index is out of range.
    InvalidSlot,
    /// The item's category is not permitted by this container.
    CategoryNotAllowed,
    /// The target stack is already at max capacity.
    StackFull,
    /// No matching item was found.
    NotFound,
    /// The inventory is locked.
    Locked,
    /// Not enough of the item to complete the removal.
    InsufficientQuantity,
    /// Tried to split by zero or a quantity ≥ stack size.
    InvalidSplitQuantity,
    /// Source and destination are the same slot.
    SameSlot,
}

impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InventoryError::Full                 => write!(f, "inventory is full"),
            InventoryError::WeightExceeded       => write!(f, "weight limit exceeded"),
            InventoryError::InvalidSlot          => write!(f, "invalid slot index"),
            InventoryError::CategoryNotAllowed   => write!(f, "item category not permitted"),
            InventoryError::StackFull            => write!(f, "stack is full"),
            InventoryError::NotFound             => write!(f, "item not found"),
            InventoryError::Locked               => write!(f, "inventory is locked"),
            InventoryError::InsufficientQuantity => write!(f, "insufficient quantity"),
            InventoryError::InvalidSplitQuantity => write!(f, "invalid split quantity"),
            InventoryError::SameSlot             => write!(f, "source and destination are the same slot"),
        }
    }
}

// ── ItemWeight helper ──────────────────────────────────────────────────────────

/// A minimal view of item weight data that `Inventory` needs without borrowing
/// the full `ItemDatabase`.
#[derive(Debug, Clone, Copy)]
pub struct ItemWeightInfo {
    pub weight_per_unit: f32,
    pub max_stack:       u32,
    pub category:        ItemCategory,
}

// ── Inventory ─────────────────────────────────────────────────────────────────

/// The primary slot-based item container.
///
/// Each [`Inventory`] has:
/// - A fixed array of [`Slot`]s.
/// - An optional weight tracking system.
/// - A gold/currency counter.
/// - An id for multi-inventory setups.
#[derive(Debug, Clone)]
pub struct Inventory {
    pub id:           u32,
    pub config:       ContainerConfig,
    slots:            Vec<Slot>,
    pub gold:         u32,
    total_weight:     f32,
    /// Per-slot cached weights so we can subtract on removal without re-querying.
    slot_weights:     Vec<f32>,
}

impl Inventory {
    pub fn new(id: u32, config: ContainerConfig) -> Self {
        let n = config.max_slots as usize;
        let slots: Vec<Slot> = (0..n).map(|i| Slot::empty(i as u32)).collect();
        let slot_weights = vec![0.0f32; n];
        Self { id, config, slots, gold: 0, total_weight: 0.0, slot_weights }
    }

    // ── Read-only queries ──────────────────────────────────────────────────────

    pub fn slot_count(&self) -> usize { self.slots.len() }

    pub fn get_slot(&self, idx: SlotIndex) -> Option<&Slot> {
        self.slots.get(idx.raw())
    }

    pub fn get_slot_mut(&mut self, idx: SlotIndex) -> Option<&mut Slot> {
        self.slots.get_mut(idx.raw())
    }

    pub fn total_weight(&self) -> f32 { self.total_weight }

    pub fn weight_remaining(&self) -> Option<f32> {
        self.config.max_weight.map(|max| max - self.total_weight)
    }

    pub fn is_full(&self) -> bool {
        self.slots.iter().all(|s| s.is_occupied())
    }

    pub fn is_empty_bag(&self) -> bool {
        self.slots.iter().all(|s| s.is_empty())
    }

    pub fn total_items(&self) -> u32 {
        self.slots.iter().map(|s| s.stack_size()).sum()
    }

    /// Count total units of a specific item id across all slots.
    pub fn count_item(&self, item_id: ItemId) -> u32 {
        self.slots.iter()
            .filter_map(|s| s.item.as_ref())
            .filter(|i| i.def_id == item_id)
            .map(|i| i.stack_size)
            .sum()
    }

    /// True if there is at least `qty` of the given item.
    pub fn has_item(&self, item_id: ItemId, qty: u32) -> bool {
        self.count_item(item_id) >= qty
    }

    /// Return the index of the first slot containing the given item id.
    pub fn find_item(&self, item_id: ItemId) -> Option<SlotIndex> {
        self.slots.iter().position(|s| {
            s.item.as_ref().map(|i| i.def_id == item_id).unwrap_or(false)
        }).map(|i| SlotIndex(i as u32))
    }

    /// Return all slot indices containing the given item id.
    pub fn find_all_item(&self, item_id: ItemId) -> Vec<SlotIndex> {
        self.slots.iter().enumerate()
            .filter(|(_, s)| s.item.as_ref().map(|i| i.def_id == item_id).unwrap_or(false))
            .map(|(i, _)| SlotIndex(i as u32))
            .collect()
    }

    pub fn find_empty_slot(&self) -> Option<SlotIndex> {
        self.slots.iter().position(|s| s.is_empty())
            .map(|i| SlotIndex(i as u32))
    }

    /// Find a slot containing the same item that has room to stack more.
    pub fn find_partial_stack(&self, item_id: ItemId, max_stack: u32) -> Option<SlotIndex> {
        self.slots.iter().enumerate()
            .find(|(_, s)| {
                s.item.as_ref()
                    .map(|i| i.def_id == item_id && i.stack_size < max_stack)
                    .unwrap_or(false)
            })
            .map(|(i, _)| SlotIndex(i as u32))
    }

    // ── Validation helpers ─────────────────────────────────────────────────────

    fn check_locked(&self) -> Result<(), InventoryError> {
        if self.config.locked { Err(InventoryError::Locked) } else { Ok(()) }
    }

    fn check_slot_valid(&self, idx: SlotIndex) -> Result<(), InventoryError> {
        if idx.raw() < self.slots.len() { Ok(()) } else { Err(InventoryError::InvalidSlot) }
    }

    fn check_category(&self, cat: ItemCategory) -> Result<(), InventoryError> {
        if let Some(allowed) = &self.config.allowed_categories {
            if !allowed.contains(&cat) {
                return Err(InventoryError::CategoryNotAllowed);
            }
        }
        Ok(())
    }

    fn check_weight(&self, extra_weight: f32) -> Result<(), InventoryError> {
        if let Some(max) = self.config.max_weight {
            if self.total_weight + extra_weight > max + f32::EPSILON {
                return Err(InventoryError::WeightExceeded);
            }
        }
        Ok(())
    }

    // ── Mutations ─────────────────────────────────────────────────────────────

    /// Add an item instance to the inventory, auto-stacking where possible.
    ///
    /// `weight_info` must be supplied by the caller who owns the `ItemDatabase`.
    /// Returns the slot index where the (final) item landed.
    pub fn add_item(
        &mut self,
        mut item: ItemInstance,
        weight_info: ItemWeightInfo,
    ) -> Result<SlotIndex, InventoryError> {
        self.check_locked()?;
        self.check_category(weight_info.category)?;

        // Try to fill existing partial stacks first.
        if weight_info.max_stack > 1 {
            let mut remaining = item.stack_size;
            let mut last_used = None;

            // Collect indices of partial stacks to avoid borrow issues.
            let partials: Vec<SlotIndex> = self.slots.iter().enumerate()
                .filter(|(_, s)| s.item.as_ref()
                    .map(|i| i.def_id == item.def_id && i.stack_size < weight_info.max_stack)
                    .unwrap_or(false))
                .map(|(i, _)| SlotIndex(i as u32))
                .collect();

            for idx in partials {
                if remaining == 0 { break; }
                let slot_item = self.slots[idx.raw()].item.as_mut().unwrap();
                let space = weight_info.max_stack - slot_item.stack_size;
                let take = space.min(remaining);

                let extra_w = weight_info.weight_per_unit * take as f32;
                if let Some(max_w) = self.config.max_weight {
                    if self.total_weight + extra_w > max_w + f32::EPSILON {
                        return Err(InventoryError::WeightExceeded);
                    }
                }

                slot_item.stack_size += take;
                self.slot_weights[idx.raw()] += extra_w;
                self.total_weight += extra_w;
                remaining -= take;
                last_used = Some(idx);
            }

            if remaining == 0 {
                return Ok(last_used.unwrap());
            }
            item.stack_size = remaining;
        }

        // Place remainder into an empty slot.
        let extra_w = weight_info.weight_per_unit * item.stack_size as f32;
        self.check_weight(extra_w)?;

        let empty = self.find_empty_slot().ok_or(InventoryError::Full)?;
        self.slot_weights[empty.raw()] = extra_w;
        self.total_weight += extra_w;
        self.slots[empty.raw()].item = Some(item);
        Ok(empty)
    }

    /// Place an item directly into a specific slot.
    ///
    /// Fails if the slot already holds a different item type, unless the slot
    /// is empty.  If the slot holds the same item, attempts to stack.
    pub fn add_to_slot(
        &mut self,
        idx: SlotIndex,
        mut item: ItemInstance,
        weight_info: ItemWeightInfo,
    ) -> Result<(), InventoryError> {
        self.check_locked()?;
        self.check_slot_valid(idx)?;
        self.check_category(weight_info.category)?;

        if let Some(existing) = &mut self.slots[idx.raw()].item {
            if existing.def_id != item.def_id {
                return Err(InventoryError::StackFull); // wrong item type
            }
            let space = weight_info.max_stack.saturating_sub(existing.stack_size);
            if space == 0 { return Err(InventoryError::StackFull); }
            let take = space.min(item.stack_size);
            let extra_w = weight_info.weight_per_unit * take as f32;
            self.check_weight(extra_w)?;
            existing.stack_size += take;
            self.slot_weights[idx.raw()] += extra_w;
            self.total_weight += extra_w;
        } else {
            let extra_w = weight_info.weight_per_unit * item.stack_size as f32;
            self.check_weight(extra_w)?;
            item.stack_size = item.stack_size.min(weight_info.max_stack);
            self.slot_weights[idx.raw()] = extra_w;
            self.total_weight += extra_w;
            self.slots[idx.raw()].item = Some(item);
        }
        Ok(())
    }

    /// Remove and return the item in slot `idx`.
    pub fn remove_from_slot(&mut self, idx: SlotIndex) -> Result<ItemInstance, InventoryError> {
        self.check_locked()?;
        self.check_slot_valid(idx)?;
        let item = self.slots[idx.raw()].item.take()
            .ok_or(InventoryError::NotFound)?;
        let w = self.slot_weights[idx.raw()];
        self.total_weight -= w;
        self.slot_weights[idx.raw()] = 0.0;
        Ok(item)
    }

    /// Remove `qty` units of `item_id`, drawing from slots in order.
    ///
    /// Returns all removed instances (may span multiple slots).
    pub fn remove_item(
        &mut self,
        item_id:    ItemId,
        qty:        u32,
        weight_per: f32,
    ) -> Result<Vec<ItemInstance>, InventoryError> {
        self.check_locked()?;
        if !self.has_item(item_id, qty) {
            return Err(InventoryError::InsufficientQuantity);
        }

        let mut remaining = qty;
        let mut removed = Vec::new();

        for i in 0..self.slots.len() {
            if remaining == 0 { break; }
            let matches = self.slots[i].item.as_ref()
                .map(|inst| inst.def_id == item_id)
                .unwrap_or(false);
            if !matches { continue; }

            let stack_size = self.slots[i].item.as_ref().unwrap().stack_size;
            if stack_size <= remaining {
                // Take the whole slot.
                let inst = self.slots[i].item.take().unwrap();
                let w = self.slot_weights[i];
                self.total_weight -= w;
                self.slot_weights[i] = 0.0;
                remaining -= inst.stack_size;
                removed.push(inst);
            } else {
                // Partial take.
                let inst = self.slots[i].item.as_mut().unwrap();
                inst.stack_size -= remaining;
                let w_removed = weight_per * remaining as f32;
                self.slot_weights[i] -= w_removed;
                self.total_weight -= w_removed;
                let mut partial = inst.clone();
                partial.stack_size = remaining;
                removed.push(partial);
                remaining = 0;
            }
        }

        Ok(removed)
    }

    /// Move the item from `from` to `to`.  If `to` is occupied by the same
    /// item type, attempts to stack; otherwise swaps.
    pub fn move_item(
        &mut self,
        from: SlotIndex,
        to:   SlotIndex,
        weight_per: f32,
        max_stack: u32,
    ) -> Result<(), InventoryError> {
        self.check_locked()?;
        self.check_slot_valid(from)?;
        self.check_slot_valid(to)?;
        if from == to { return Err(InventoryError::SameSlot); }

        // Check the source has an item.
        if self.slots[from.raw()].item.is_none() {
            return Err(InventoryError::NotFound);
        }

        let dest_occupied = self.slots[to.raw()].item.is_some();
        let same_type = dest_occupied && {
            let src_id = self.slots[from.raw()].item.as_ref().unwrap().def_id;
            let dst_id = self.slots[to.raw()].item.as_ref().unwrap().def_id;
            src_id == dst_id
        };

        if !dest_occupied || same_type {
            if same_type {
                // Try to merge.
                let src_size = self.slots[from.raw()].item.as_ref().unwrap().stack_size;
                let dst_size = self.slots[to.raw()].item.as_ref().unwrap().stack_size;
                let space = max_stack.saturating_sub(dst_size);
                let take = space.min(src_size);
                let delta_w = weight_per * take as f32;

                self.slots[to.raw()].item.as_mut().unwrap().stack_size += take;
                self.slot_weights[to.raw()] += delta_w;

                let src = self.slots[from.raw()].item.as_mut().unwrap();
                src.stack_size -= take;
                self.slot_weights[from.raw()] -= delta_w;
                if src.stack_size == 0 {
                    self.slots[from.raw()].item = None;
                    self.slot_weights[from.raw()] = 0.0;
                }
            } else {
                // Simple move to empty slot.
                let item = self.slots[from.raw()].item.take().unwrap();
                let w = self.slot_weights[from.raw()];
                self.slot_weights[from.raw()] = 0.0;
                self.slots[to.raw()].item = Some(item);
                self.slot_weights[to.raw()] = w;
            }
        } else {
            // Swap.
            self.slots.swap(from.raw(), to.raw());
            self.slot_weights.swap(from.raw(), to.raw());
        }
        Ok(())
    }

    /// Swap the contents of two slots unconditionally.
    pub fn swap_slots(&mut self, a: SlotIndex, b: SlotIndex) -> Result<(), InventoryError> {
        self.check_locked()?;
        self.check_slot_valid(a)?;
        self.check_slot_valid(b)?;
        if a == b { return Err(InventoryError::SameSlot); }
        self.slots.swap(a.raw(), b.raw());
        self.slot_weights.swap(a.raw(), b.raw());
        Ok(())
    }

    /// Consolidate all stacks of the same item where possible.
    ///
    /// `max_stacks` maps item id → max stack size.
    pub fn stack_items(&mut self, max_stacks: &HashMap<ItemId, u32>) {
        // Group slot indices by item id.
        let mut groups: HashMap<ItemId, Vec<usize>> = HashMap::new();
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some(inst) = &slot.item {
                groups.entry(inst.def_id).or_default().push(i);
            }
        }

        for (item_id, mut indices) in groups {
            let max_stack = *max_stacks.get(&item_id).unwrap_or(&1);
            if max_stack <= 1 || indices.len() <= 1 { continue; }
            indices.sort_unstable();

            // Accumulate total and redistribute.
            let total: u32 = indices.iter()
                .filter_map(|&i| self.slots[i].item.as_ref())
                .map(|inst| inst.stack_size)
                .sum();

            let mut remaining = total;
            for &i in &indices {
                if remaining == 0 {
                    self.slots[i].item = None;
                    self.slot_weights[i] = 0.0;
                } else {
                    let take = remaining.min(max_stack);
                    if let Some(inst) = &mut self.slots[i].item {
                        inst.stack_size = take;
                    }
                    remaining -= take;
                }
            }
            // Recalculate total weight for this item (weight_per is unknown here;
            // caller is responsible for calling recalculate_weight() afterwards).
        }
    }

    /// Split `qty` units out of `from` into the first available empty slot.
    pub fn split_stack(
        &mut self,
        from: SlotIndex,
        qty:  u32,
        weight_per: f32,
    ) -> Result<SlotIndex, InventoryError> {
        self.check_locked()?;
        self.check_slot_valid(from)?;

        let src_size = self.slots[from.raw()].item.as_ref()
            .ok_or(InventoryError::NotFound)?.stack_size;

        if qty == 0 || qty >= src_size {
            return Err(InventoryError::InvalidSplitQuantity);
        }

        let to = self.find_empty_slot().ok_or(InventoryError::Full)?;
        let delta_w = weight_per * qty as f32;

        let new_inst = {
            let src = self.slots[from.raw()].item.as_mut().unwrap();
            src.stack_size -= qty;
            let mut ni = src.clone();
            ni.stack_size = qty;
            ni
        };
        self.slot_weights[from.raw()] -= delta_w;
        self.slots[to.raw()].item = Some(new_inst);
        self.slot_weights[to.raw()] = delta_w;
        Ok(to)
    }

    /// Recalculate `total_weight` and `slot_weights` from scratch.
    ///
    /// Requires a closure mapping item_id → weight-per-unit.
    pub fn recalculate_weight<F>(&mut self, weight_fn: F)
    where F: Fn(ItemId) -> f32,
    {
        self.total_weight = 0.0;
        for i in 0..self.slots.len() {
            if let Some(inst) = &self.slots[i].item {
                let w = weight_fn(inst.def_id) * inst.stack_size as f32;
                self.slot_weights[i] = w;
                self.total_weight += w;
            } else {
                self.slot_weights[i] = 0.0;
            }
        }
    }

    // ── Sorting ────────────────────────────────────────────────────────────────

    /// Sort slots by category (using an external category lookup).
    pub fn sort_by_category<F>(&mut self, category_fn: F)
    where F: Fn(ItemId) -> ItemCategory,
    {
        let weights = self.slot_weights.clone();
        let mut pairs: Vec<(Option<ItemInstance>, f32)> = self.slots.drain(..)
            .zip(weights.into_iter())
            .map(|(s, w)| (s.item, w))
            .collect();

        pairs.sort_by_key(|(inst, _)| {
            inst.as_ref().map(|i| category_fn(i.def_id) as u8).unwrap_or(u8::MAX)
        });

        let n = pairs.len();
        self.slots = pairs.iter().enumerate()
            .map(|(i, (inst, _))| Slot { slot_id: i as u32, item: inst.clone() })
            .collect();
        self.slot_weights = pairs.iter().map(|(_, w)| *w).collect();
        let _ = n;
    }

    /// Sort slots by base value descending (most valuable first).
    pub fn sort_by_value<F>(&mut self, value_fn: F)
    where F: Fn(ItemId) -> u32,
    {
        let weights = self.slot_weights.clone();
        let mut pairs: Vec<(Option<ItemInstance>, f32)> = self.slots.drain(..)
            .zip(weights.into_iter())
            .map(|(s, w)| (s.item, w))
            .collect();

        pairs.sort_by(|(a, _), (b, _)| {
            let va = a.as_ref().map(|i| value_fn(i.def_id)).unwrap_or(0);
            let vb = b.as_ref().map(|i| value_fn(i.def_id)).unwrap_or(0);
            vb.cmp(&va)
        });

        self.slots = pairs.iter().enumerate()
            .map(|(i, (inst, _))| Slot { slot_id: i as u32, item: inst.clone() })
            .collect();
        self.slot_weights = pairs.iter().map(|(_, w)| *w).collect();
    }

    /// Sort slots alphabetically by item name.
    pub fn sort_by_name<F>(&mut self, name_fn: F)
    where F: Fn(ItemId) -> String,
    {
        let weights = self.slot_weights.clone();
        let mut pairs: Vec<(Option<ItemInstance>, f32)> = self.slots.drain(..)
            .zip(weights.into_iter())
            .map(|(s, w)| (s.item, w))
            .collect();

        pairs.sort_by(|(a, _), (b, _)| {
            let na = a.as_ref().map(|i| name_fn(i.def_id)).unwrap_or_default();
            let nb = b.as_ref().map(|i| name_fn(i.def_id)).unwrap_or_default();
            na.cmp(&nb)
        });

        self.slots = pairs.iter().enumerate()
            .map(|(i, (inst, _))| Slot { slot_id: i as u32, item: inst.clone() })
            .collect();
        self.slot_weights = pairs.iter().map(|(_, w)| *w).collect();
    }

    /// Compress: consolidate partial stacks and push empty slots to the back.
    pub fn compress(&mut self, max_stacks: &HashMap<ItemId, u32>) {
        self.stack_items(max_stacks);

        // Stable partition: occupied slots first.
        let n = self.slots.len();
        let weights = self.slot_weights.clone();
        let mut pairs: Vec<(Slot, f32)> = self.slots.drain(..)
            .zip(weights.into_iter())
            .collect();

        pairs.sort_by_key(|(s, _)| if s.is_empty() { 1u8 } else { 0u8 });

        self.slots = pairs.iter_mut().enumerate()
            .map(|(i, (s, _))| {
                s.slot_id = i as u32;
                s.clone()
            })
            .collect();
        self.slot_weights = pairs.iter().map(|(_, w)| *w).collect();
        let _ = n;
    }

    // ── Iterators ─────────────────────────────────────────────────────────────

    pub fn iter_occupied(&self) -> impl Iterator<Item = (SlotIndex, &ItemInstance)> {
        self.slots.iter().enumerate()
            .filter_map(|(i, s)| s.item.as_ref().map(|inst| (SlotIndex(i as u32), inst)))
    }

    pub fn iter_all_slots(&self) -> impl Iterator<Item = &Slot> {
        self.slots.iter()
    }
}

// ── InventoryTransaction ───────────────────────────────────────────────────────

/// Describes one pending operation in a transaction.
#[derive(Debug, Clone)]
enum TxOp {
    Add { inv_id: u32, item: ItemInstance, weight_info: ItemWeightInfo },
    Remove { inv_id: u32, item_id: ItemId, qty: u32, weight_per: f32 },
    Transfer { from_inv: u32, to_inv: u32, item_id: ItemId, qty: u32, weight_per: f32, max_stack: u32 },
    AddGold { inv_id: u32, amount: u32 },
    RemoveGold { inv_id: u32, amount: u32 },
}

/// Error conditions for a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionError {
    InventoryNotFound(u32),
    InventoryFull(u32),
    InsufficientItems { inv_id: u32, item_id: ItemId, needed: u32, have: u32 },
    InsufficientGold { inv_id: u32, needed: u32, have: u32 },
    InventoryLocked(u32),
    WeightExceeded(u32),
    CategoryNotAllowed(u32),
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionError::InventoryNotFound(id) =>
                write!(f, "inventory {} not found", id),
            TransactionError::InventoryFull(id) =>
                write!(f, "inventory {} is full", id),
            TransactionError::InsufficientItems { inv_id, item_id, needed, have } =>
                write!(f, "inventory {}: need {} of {:?}, have {}", inv_id, needed, item_id, have),
            TransactionError::InsufficientGold { inv_id, needed, have } =>
                write!(f, "inventory {}: need {} gold, have {}", inv_id, needed, have),
            TransactionError::InventoryLocked(id) =>
                write!(f, "inventory {} is locked", id),
            TransactionError::WeightExceeded(id) =>
                write!(f, "inventory {}: weight limit exceeded", id),
            TransactionError::CategoryNotAllowed(id) =>
                write!(f, "inventory {}: category not allowed", id),
        }
    }
}

/// Atomic multi-inventory operation batch.
///
/// Build up a list of operations, then call [`InventoryTransaction::execute`].
/// If any operation fails, all inventories are restored to their prior state.
///
/// # Example
/// ```ignore
/// let mut tx = InventoryTransaction::new();
/// tx.add(player_inv_id, sword, sword_weight);
/// tx.remove_gold(shop_inv_id, cost);
/// tx.execute(&mut [player, shop])?;
/// ```
#[derive(Debug, Default)]
pub struct InventoryTransaction {
    ops: VecDeque<TxOp>,
}

impl InventoryTransaction {
    pub fn new() -> Self { Self { ops: VecDeque::new() } }

    pub fn add(
        &mut self,
        inv_id: u32,
        item: ItemInstance,
        weight_info: ItemWeightInfo,
    ) -> &mut Self {
        self.ops.push_back(TxOp::Add { inv_id, item, weight_info });
        self
    }

    pub fn remove(
        &mut self,
        inv_id: u32,
        item_id: ItemId,
        qty: u32,
        weight_per: f32,
    ) -> &mut Self {
        self.ops.push_back(TxOp::Remove { inv_id, item_id, qty, weight_per });
        self
    }

    pub fn transfer(
        &mut self,
        from_inv: u32,
        to_inv:   u32,
        item_id:  ItemId,
        qty:      u32,
        weight_per: f32,
        max_stack:  u32,
    ) -> &mut Self {
        self.ops.push_back(TxOp::Transfer { from_inv, to_inv, item_id, qty, weight_per, max_stack });
        self
    }

    pub fn add_gold(&mut self, inv_id: u32, amount: u32) -> &mut Self {
        self.ops.push_back(TxOp::AddGold { inv_id, amount });
        self
    }

    pub fn remove_gold(&mut self, inv_id: u32, amount: u32) -> &mut Self {
        self.ops.push_back(TxOp::RemoveGold { inv_id, amount });
        self
    }

    /// Execute all operations atomically.
    ///
    /// On failure, all supplied inventories are rolled back to their
    /// pre-execution state via `Clone`.
    pub fn execute(
        &mut self,
        inventories: &mut [&mut Inventory],
    ) -> Result<(), TransactionError> {
        // Snapshot for rollback.
        let snapshots: Vec<Inventory> = inventories.iter().map(|inv| (*inv).clone()).collect();

        let result = self.apply_all(inventories);
        if result.is_err() {
            // Rollback: restore every inventory from its snapshot.
            for (inv, snap) in inventories.iter_mut().zip(snapshots.into_iter()) {
                **inv = snap;
            }
        }
        result
    }

    fn find_inv<'a>(
        inventories: &'a mut [&'a mut Inventory],
        id: u32,
    ) -> Option<&'a mut Inventory> {
        inventories.iter_mut().find(|inv| inv.id == id).map(|r| &mut **r)
    }

    fn apply_all(&mut self, inventories: &mut [&mut Inventory]) -> Result<(), TransactionError> {
        for op in self.ops.iter() {
            match op {
                TxOp::Add { inv_id, item, weight_info } => {
                    let id = *inv_id;
                    let inv = inventories.iter_mut()
                        .find(|inv| inv.id == id)
                        .map(|r| &mut **r)
                        .ok_or(TransactionError::InventoryNotFound(id))?;
                    inv.add_item(item.clone(), *weight_info).map_err(|e| match e {
                        InventoryError::Full           => TransactionError::InventoryFull(id),
                        InventoryError::Locked         => TransactionError::InventoryLocked(id),
                        InventoryError::WeightExceeded => TransactionError::WeightExceeded(id),
                        InventoryError::CategoryNotAllowed => TransactionError::CategoryNotAllowed(id),
                        _ => TransactionError::InventoryFull(id),
                    })?;
                }

                TxOp::Remove { inv_id, item_id, qty, weight_per } => {
                    let id = *inv_id;
                    let iid = *item_id;
                    let q = *qty;
                    let wp = *weight_per;
                    let inv = inventories.iter_mut()
                        .find(|inv| inv.id == id)
                        .map(|r| &mut **r)
                        .ok_or(TransactionError::InventoryNotFound(id))?;
                    let have = inv.count_item(iid);
                    if have < q {
                        return Err(TransactionError::InsufficientItems {
                            inv_id: id, item_id: iid, needed: q, have,
                        });
                    }
                    inv.remove_item(iid, q, wp).map_err(|_| TransactionError::InventoryFull(id))?;
                }

                TxOp::Transfer { from_inv, to_inv, item_id, qty, weight_per, max_stack } => {
                    let fid = *from_inv;
                    let tid = *to_inv;
                    let iid = *item_id;
                    let q = *qty;
                    let wp = *weight_per;
                    let ms = *max_stack;

                    // Validate source has enough.
                    let from = inventories.iter_mut()
                        .find(|inv| inv.id == fid)
                        .map(|r| &mut **r)
                        .ok_or(TransactionError::InventoryNotFound(fid))?;
                    let have = from.count_item(iid);
                    if have < q {
                        return Err(TransactionError::InsufficientItems {
                            inv_id: fid, item_id: iid, needed: q, have,
                        });
                    }
                    let removed = from.remove_item(iid, q, wp)
                        .map_err(|_| TransactionError::InventoryFull(fid))?;

                    // Add to destination.
                    let wi = ItemWeightInfo {
                        weight_per_unit: wp,
                        max_stack: ms,
                        category: ItemCategory::Misc, // caller supplies real category via weight_info
                    };
                    let to = inventories.iter_mut()
                        .find(|inv| inv.id == tid)
                        .map(|r| &mut **r)
                        .ok_or(TransactionError::InventoryNotFound(tid))?;
                    for inst in removed {
                        to.add_item(inst, wi).map_err(|e| match e {
                            InventoryError::Full           => TransactionError::InventoryFull(tid),
                            InventoryError::Locked         => TransactionError::InventoryLocked(tid),
                            InventoryError::WeightExceeded => TransactionError::WeightExceeded(tid),
                            _ => TransactionError::InventoryFull(tid),
                        })?;
                    }
                }

                TxOp::AddGold { inv_id, amount } => {
                    let id = *inv_id;
                    let amt = *amount;
                    let inv = inventories.iter_mut()
                        .find(|inv| inv.id == id)
                        .map(|r| &mut **r)
                        .ok_or(TransactionError::InventoryNotFound(id))?;
                    inv.gold = inv.gold.saturating_add(amt);
                }

                TxOp::RemoveGold { inv_id, amount } => {
                    let id = *inv_id;
                    let amt = *amount;
                    let inv = inventories.iter_mut()
                        .find(|inv| inv.id == id)
                        .map(|r| &mut **r)
                        .ok_or(TransactionError::InventoryNotFound(id))?;
                    if inv.gold < amt {
                        return Err(TransactionError::InsufficientGold {
                            inv_id: id, needed: amt, have: inv.gold,
                        });
                    }
                    inv.gold -= amt;
                }
            }
        }
        Ok(())
    }
}

// ── Loot ───────────────────────────────────────────────────────────────────────

/// A collection of item drops, as produced by a loot table roll.
#[derive(Debug, Clone, Default)]
pub struct Loot {
    pub drops: Vec<(ItemId, u32)>,
    pub gold:  u32,
}

impl Loot {
    pub fn new() -> Self { Self::default() }

    /// Add a drop, merging with an existing entry for the same item if present.
    pub fn add(&mut self, id: ItemId, qty: u32) {
        if let Some(entry) = self.drops.iter_mut().find(|(i, _)| *i == id) {
            entry.1 += qty;
        } else {
            self.drops.push((id, qty));
        }
    }

    pub fn add_gold(&mut self, amount: u32) {
        self.gold += amount;
    }

    /// Merge another [`Loot`] into this one.
    pub fn merge(&mut self, other: Loot) {
        for (id, qty) in other.drops {
            self.add(id, qty);
        }
        self.gold += other.gold;
    }

    pub fn is_empty(&self) -> bool { self.drops.is_empty() && self.gold == 0 }

    pub fn item_count(&self) -> usize { self.drops.len() }

    pub fn total_units(&self) -> u32 { self.drops.iter().map(|(_, q)| q).sum() }
}

// ── LootCondition ─────────────────────────────────────────────────────────────

/// A predicate gating whether a loot entry is eligible this roll.
#[derive(Debug, Clone)]
pub enum LootCondition {
    /// Player must be at least this level.
    MinLevel(u32),
    /// A named flag must be present on the roller.
    HasFlag(String),
    /// Additional flat probability gate (0 to 1).
    Chance(f32),
}

impl LootCondition {
    pub fn is_met(&self, level: u32, flags: &[String], rng: &mut Rng) -> bool {
        match self {
            LootCondition::MinLevel(min)  => level >= *min,
            LootCondition::HasFlag(flag)  => flags.iter().any(|f| f == flag),
            LootCondition::Chance(p)      => rng.chance(*p),
        }
    }
}

// ── LootEntry ─────────────────────────────────────────────────────────────────

/// One line in a loot table.
#[derive(Debug, Clone)]
pub struct LootEntry {
    pub item_id:   ItemId,
    /// Relative probability weight (higher = more likely to be selected).
    pub weight:    f32,
    pub min_qty:   u32,
    pub max_qty:   u32,
    pub condition: Option<LootCondition>,
}

impl LootEntry {
    pub fn new(item_id: ItemId, weight: f32, min_qty: u32, max_qty: u32) -> Self {
        Self { item_id, weight, min_qty, max_qty, condition: None }
    }

    pub fn with_condition(mut self, c: LootCondition) -> Self {
        self.condition = Some(c); self
    }

    pub fn is_eligible(&self, level: u32, flags: &[String], rng: &mut Rng) -> bool {
        match &self.condition {
            None    => true,
            Some(c) => c.is_met(level, flags, rng),
        }
    }
}

// ── LootTable ─────────────────────────────────────────────────────────────────

/// A weighted loot table that produces [`Loot`] on each roll.
#[derive(Debug, Clone)]
pub struct LootTable {
    pub entries:      Vec<LootEntry>,
    /// How many independent picks are made per roll.
    pub rolls:        u32,
    /// Flat probability (0..1) that any drop occurs at all.
    pub drop_chance:  f32,
    /// Optional gold range dropped alongside items.
    pub gold_min:     u32,
    pub gold_max:     u32,
    /// If true, each entry can only be picked once per roll.
    pub no_duplicates: bool,
}

impl LootTable {
    pub fn new() -> Self {
        Self {
            entries:       Vec::new(),
            rolls:         1,
            drop_chance:   1.0,
            gold_min:      0,
            gold_max:      0,
            no_duplicates: false,
        }
    }

    pub fn with_rolls(mut self, n: u32) -> Self { self.rolls = n.max(1); self }
    pub fn with_drop_chance(mut self, p: f32) -> Self { self.drop_chance = p.clamp(0.0, 1.0); self }
    pub fn with_gold(mut self, min: u32, max: u32) -> Self { self.gold_min = min; self.gold_max = max; self }
    pub fn no_duplicates(mut self) -> Self { self.no_duplicates = true; self }

    pub fn add_entry(&mut self, entry: LootEntry) {
        self.entries.push(entry);
    }

    pub fn add_item(&mut self, id: ItemId, weight: f32, qty: u32) -> &mut Self {
        self.entries.push(LootEntry::new(id, weight, qty, qty));
        self
    }

    pub fn add_item_range(&mut self, id: ItemId, weight: f32, min: u32, max: u32) -> &mut Self {
        self.entries.push(LootEntry::new(id, weight, min, max));
        self
    }

    // ── Roll ──────────────────────────────────────────────────────────────────

    /// Roll the loot table and return the generated [`Loot`].
    pub fn roll(&self, rng: &mut Rng, level: u32, flags: &[String]) -> Loot {
        let mut loot = Loot::new();

        if !rng.chance(self.drop_chance) {
            return loot;
        }

        // Filter eligible entries.
        let eligible: Vec<&LootEntry> = self.entries.iter()
            .filter(|e| e.is_eligible(level, flags, rng))
            .collect();

        if eligible.is_empty() { return loot; }

        let total_weight: f32 = eligible.iter().map(|e| e.weight).sum();
        if total_weight <= 0.0 { return loot; }

        let mut used_indices: Vec<usize> = Vec::new();

        for _ in 0..self.rolls {
            let eligible_now: Vec<(usize, &LootEntry)> = eligible.iter().enumerate()
                .filter(|(i, _)| !self.no_duplicates || !used_indices.contains(i))
                .map(|(i, e)| (i, *e))
                .collect();

            if eligible_now.is_empty() { break; }

            let current_total: f32 = eligible_now.iter().map(|(_, e)| e.weight).sum();
            let roll = rng.range_f32(0.0, current_total);

            let mut cumulative = 0.0f32;
            for (orig_i, entry) in &eligible_now {
                cumulative += entry.weight;
                if roll < cumulative {
                    let qty = rng.range_u32(entry.min_qty, entry.max_qty.max(entry.min_qty));
                    loot.add(entry.item_id, qty);
                    if self.no_duplicates {
                        used_indices.push(*orig_i);
                    }
                    break;
                }
            }
        }

        // Gold.
        if self.gold_max > 0 {
            let gold = rng.range_u32(self.gold_min, self.gold_max);
            loot.add_gold(gold);
        }

        loot
    }

    // ── Presets ───────────────────────────────────────────────────────────────

    /// An entirely empty table — rolls always produce nothing.
    pub fn empty_chest() -> Self {
        let mut t = Self::new();
        t.drop_chance = 0.0;
        t
    }

    /// A common chest: mix of consumables and materials, small gold.
    /// Item ids 1001..1010 are assumed to represent common consumables/materials.
    pub fn common_chest() -> Self {
        let mut t = Self::new().with_rolls(3).with_drop_chance(0.95).with_gold(5, 30);
        for id in 1001u32..=1010 {
            t.add_item(ItemId(id), 10.0, 1);
        }
        t
    }

    /// A rare chest: guaranteed drop of 1 rare item, plus some consumables.
    pub fn rare_chest() -> Self {
        let mut t = Self::new().with_rolls(5).with_drop_chance(1.0).with_gold(50, 200);
        for id in 2001u32..=2005 {
            t.add_item(ItemId(id), 5.0, 1);
        }
        for id in 1001u32..=1005 {
            t.add_item(ItemId(id), 15.0, 1);
        }
        t
    }

    /// A boss chest: many rolls, high-value items, large gold reward.
    pub fn boss_chest() -> Self {
        let mut t = Self::new()
            .with_rolls(8)
            .with_drop_chance(1.0)
            .with_gold(500, 2000)
            .no_duplicates();
        for id in 3001u32..=3010 {
            t.add_item_range(ItemId(id), 3.0, 1, 2);
        }
        for id in 2001u32..=2010 {
            t.add_item(ItemId(id), 8.0, 1);
        }
        // Legendary entry with level gate.
        t.add_entry(
            LootEntry::new(ItemId(9001), 1.0, 1, 1)
                .with_condition(LootCondition::MinLevel(20)),
        );
        t
    }
}

impl Default for LootTable {
    fn default() -> Self { Self::new() }
}

// ── Inventory statistics helper ────────────────────────────────────────────────

/// A snapshot of an inventory's occupancy and weight for UI display.
#[derive(Debug, Clone)]
pub struct InventoryStats {
    pub total_slots:     u32,
    pub used_slots:      u32,
    pub total_items:     u32,
    pub total_weight:    f32,
    pub max_weight:      Option<f32>,
    pub gold:            u32,
}

impl InventoryStats {
    pub fn from_inventory(inv: &Inventory) -> Self {
        let used_slots = inv.slots.iter().filter(|s| s.is_occupied()).count() as u32;
        Self {
            total_slots:  inv.config.max_slots,
            used_slots,
            total_items:  inv.total_items(),
            total_weight: inv.total_weight(),
            max_weight:   inv.config.max_weight,
            gold:         inv.gold,
        }
    }

    pub fn slots_free(&self) -> u32 { self.total_slots - self.used_slots }

    pub fn weight_fraction(&self) -> f32 {
        match self.max_weight {
            Some(max) if max > 0.0 => (self.total_weight / max).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    pub fn is_encumbered(&self) -> bool {
        self.max_weight.map(|max| self.total_weight > max * 0.9).unwrap_or(false)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn wi(weight: f32, max_stack: u32, cat: ItemCategory) -> ItemWeightInfo {
        ItemWeightInfo { weight_per_unit: weight, max_stack, category: cat }
    }

    fn make_inv(slots: u32) -> Inventory {
        Inventory::new(1, ContainerConfig::new(slots))
    }

    fn make_weighted_inv(slots: u32, max_w: f32) -> Inventory {
        Inventory::new(1, ContainerConfig::new(slots).with_max_weight(max_w))
    }

    // ── Basic add / remove ─────────────────────────────────────────────────────

    #[test]
    fn add_item_to_empty_inv() {
        let mut inv = make_inv(10);
        let item = ItemInstance::new(ItemId(1));
        let idx = inv.add_item(item, wi(1.0, 1, ItemCategory::Weapon)).unwrap();
        assert_eq!(idx, SlotIndex(0));
        assert!(inv.has_item(ItemId(1), 1));
    }

    #[test]
    fn add_stackable_item_fills_existing_stack() {
        let mut inv = make_inv(5);
        let a = ItemInstance::new_stack(ItemId(2), 5);
        let b = ItemInstance::new_stack(ItemId(2), 3);
        let wi_potions = wi(0.2, 10, ItemCategory::Consumable);
        inv.add_item(a, wi_potions).unwrap();
        inv.add_item(b, wi_potions).unwrap();
        assert_eq!(inv.count_item(ItemId(2)), 8);
        // Should still be in one slot.
        let occupied: Vec<_> = inv.iter_occupied().collect();
        assert_eq!(occupied.len(), 1);
    }

    #[test]
    fn add_item_overflows_stack_into_new_slot() {
        let mut inv = make_inv(5);
        let wi_mat = wi(0.1, 5, ItemCategory::Material);
        inv.add_item(ItemInstance::new_stack(ItemId(3), 5), wi_mat).unwrap();
        inv.add_item(ItemInstance::new_stack(ItemId(3), 3), wi_mat).unwrap();
        assert_eq!(inv.count_item(ItemId(3)), 8);
        let occupied: Vec<_> = inv.iter_occupied().collect();
        assert_eq!(occupied.len(), 2); // 5 in first, 3 in second
    }

    #[test]
    fn add_item_fails_when_full() {
        let mut inv = make_inv(2);
        let wi_sword = wi(3.0, 1, ItemCategory::Weapon);
        inv.add_item(ItemInstance::new(ItemId(1)), wi_sword).unwrap();
        inv.add_item(ItemInstance::new(ItemId(1)), wi_sword).unwrap();
        let result = inv.add_item(ItemInstance::new(ItemId(1)), wi_sword);
        assert_eq!(result, Err(InventoryError::Full));
    }

    #[test]
    fn add_item_fails_on_weight_exceeded() {
        let mut inv = make_weighted_inv(10, 5.0);
        let wi_heavy = wi(4.0, 1, ItemCategory::Weapon);
        inv.add_item(ItemInstance::new(ItemId(1)), wi_heavy).unwrap();
        let result = inv.add_item(ItemInstance::new(ItemId(2)), wi_heavy);
        assert_eq!(result, Err(InventoryError::WeightExceeded));
    }

    #[test]
    fn remove_from_slot_clears_slot() {
        let mut inv = make_inv(5);
        inv.add_item(ItemInstance::new(ItemId(1)), wi(1.0, 1, ItemCategory::Weapon)).unwrap();
        let removed = inv.remove_from_slot(SlotIndex(0)).unwrap();
        assert_eq!(removed.def_id, ItemId(1));
        assert!(inv.is_empty_bag());
    }

    #[test]
    fn remove_item_by_id_partial() {
        let mut inv = make_inv(5);
        inv.add_item(ItemInstance::new_stack(ItemId(4), 10), wi(0.1, 20, ItemCategory::Consumable)).unwrap();
        let removed = inv.remove_item(ItemId(4), 3, 0.1).unwrap();
        let total_removed: u32 = removed.iter().map(|i| i.stack_size).sum();
        assert_eq!(total_removed, 3);
        assert_eq!(inv.count_item(ItemId(4)), 7);
    }

    #[test]
    fn remove_item_insufficient_returns_error() {
        let mut inv = make_inv(5);
        inv.add_item(ItemInstance::new_stack(ItemId(4), 2), wi(0.1, 20, ItemCategory::Consumable)).unwrap();
        let result = inv.remove_item(ItemId(4), 5, 0.1);
        assert_eq!(result, Err(InventoryError::InsufficientQuantity));
    }

    // ── Stack / split ─────────────────────────────────────────────────────────

    #[test]
    fn split_stack_creates_new_slot() {
        let mut inv = make_inv(5);
        inv.add_item(ItemInstance::new_stack(ItemId(5), 8), wi(0.2, 10, ItemCategory::Material)).unwrap();
        let new_slot = inv.split_stack(SlotIndex(0), 3, 0.2).unwrap();
        assert_eq!(inv.get_slot(SlotIndex(0)).unwrap().stack_size(), 5);
        assert_eq!(inv.get_slot(new_slot).unwrap().stack_size(), 3);
    }

    #[test]
    fn split_stack_invalid_qty() {
        let mut inv = make_inv(5);
        inv.add_item(ItemInstance::new_stack(ItemId(5), 4), wi(0.2, 10, ItemCategory::Material)).unwrap();
        assert_eq!(inv.split_stack(SlotIndex(0), 0, 0.2), Err(InventoryError::InvalidSplitQuantity));
        assert_eq!(inv.split_stack(SlotIndex(0), 4, 0.2), Err(InventoryError::InvalidSplitQuantity));
        assert_eq!(inv.split_stack(SlotIndex(0), 5, 0.2), Err(InventoryError::InvalidSplitQuantity));
    }

    #[test]
    fn compress_consolidates_partials() {
        let mut inv = make_inv(5);
        let wi_mat = wi(0.1, 10, ItemCategory::Material);
        // Add three partial stacks manually.
        inv.add_item(ItemInstance::new_stack(ItemId(6), 3), wi_mat).unwrap();
        // Split to create another slot.
        inv.split_stack(SlotIndex(0), 1, 0.1).unwrap();
        inv.split_stack(SlotIndex(0), 1, 0.1).unwrap();
        // Now have 3 slots with sizes ~1, 1, 1.
        let mut max_stacks = HashMap::new();
        max_stacks.insert(ItemId(6), 10u32);
        inv.compress(&max_stacks);
        // After compress: one slot with size 3, rest empty.
        let occupied: Vec<_> = inv.iter_occupied().collect();
        assert_eq!(occupied.len(), 1);
        assert_eq!(occupied[0].1.stack_size, 3);
    }

    // ── Move / swap ───────────────────────────────────────────────────────────

    #[test]
    fn move_item_to_empty_slot() {
        let mut inv = make_inv(5);
        inv.add_item(ItemInstance::new(ItemId(7)), wi(1.0, 1, ItemCategory::Weapon)).unwrap();
        inv.move_item(SlotIndex(0), SlotIndex(3), 1.0, 1).unwrap();
        assert!(inv.get_slot(SlotIndex(0)).unwrap().is_empty());
        assert!(inv.get_slot(SlotIndex(3)).unwrap().is_occupied());
    }

    #[test]
    fn swap_slots() {
        let mut inv = make_inv(5);
        let wi_w = wi(1.0, 1, ItemCategory::Weapon);
        inv.add_item(ItemInstance::new(ItemId(1)), wi_w).unwrap();
        inv.add_item(ItemInstance::new(ItemId(2)), wi_w).unwrap();
        inv.swap_slots(SlotIndex(0), SlotIndex(1)).unwrap();
        assert_eq!(inv.get_slot(SlotIndex(0)).unwrap().item.as_ref().unwrap().def_id, ItemId(2));
        assert_eq!(inv.get_slot(SlotIndex(1)).unwrap().item.as_ref().unwrap().def_id, ItemId(1));
    }

    // ── Locked inventory ──────────────────────────────────────────────────────

    #[test]
    fn locked_inventory_rejects_add() {
        let mut inv = Inventory::new(1, ContainerConfig::new(10).locked());
        let result = inv.add_item(ItemInstance::new(ItemId(1)), wi(1.0, 1, ItemCategory::Weapon));
        assert_eq!(result, Err(InventoryError::Locked));
    }

    // ── Category filter ───────────────────────────────────────────────────────

    #[test]
    fn category_restricted_inventory() {
        let mut inv = Inventory::new(
            1,
            ContainerConfig::new(10).with_allowed_categories(vec![ItemCategory::Consumable]),
        );
        let result = inv.add_item(ItemInstance::new(ItemId(1)), wi(1.0, 1, ItemCategory::Weapon));
        assert_eq!(result, Err(InventoryError::CategoryNotAllowed));
        let ok = inv.add_item(ItemInstance::new(ItemId(2)), wi(0.2, 10, ItemCategory::Consumable));
        assert!(ok.is_ok());
    }

    // ── Transaction ───────────────────────────────────────────────────────────

    #[test]
    fn transaction_commit_success() {
        let mut inv1 = Inventory::new(1, ContainerConfig::new(10));
        let mut inv2 = Inventory::new(2, ContainerConfig::new(10));
        inv1.add_item(ItemInstance::new_stack(ItemId(10), 5), wi(0.1, 10, ItemCategory::Material)).unwrap();

        let mut tx = InventoryTransaction::new();
        tx.remove(1, ItemId(10), 3, 0.1);
        tx.add(2, ItemInstance::new_stack(ItemId(10), 3), wi(0.1, 10, ItemCategory::Material));

        let result = tx.execute(&mut [&mut inv1, &mut inv2]);
        assert!(result.is_ok());
        assert_eq!(inv1.count_item(ItemId(10)), 2);
        assert_eq!(inv2.count_item(ItemId(10)), 3);
    }

    #[test]
    fn transaction_rollback_on_failure() {
        let mut inv1 = Inventory::new(1, ContainerConfig::new(10));
        let mut inv2 = Inventory::new(2, ContainerConfig::new(1)); // only 1 slot
        inv1.add_item(ItemInstance::new(ItemId(11)), wi(1.0, 1, ItemCategory::Weapon)).unwrap();
        inv2.add_item(ItemInstance::new(ItemId(99)), wi(1.0, 1, ItemCategory::Weapon)).unwrap(); // fill inv2

        let mut tx = InventoryTransaction::new();
        tx.remove(1, ItemId(11), 1, 1.0);
        tx.add(2, ItemInstance::new(ItemId(11)), wi(1.0, 1, ItemCategory::Weapon));

        let result = tx.execute(&mut [&mut inv1, &mut inv2]);
        assert!(result.is_err());
        // inv1 should be rolled back — item still present.
        assert_eq!(inv1.count_item(ItemId(11)), 1);
    }

    #[test]
    fn transaction_add_remove_gold() {
        let mut inv = Inventory::new(1, ContainerConfig::new(5));
        inv.gold = 100;
        let mut tx = InventoryTransaction::new();
        tx.remove_gold(1, 40);
        tx.add_gold(1, 10);
        tx.execute(&mut [&mut inv]).unwrap();
        assert_eq!(inv.gold, 70);
    }

    // ── Loot table ────────────────────────────────────────────────────────────

    #[test]
    fn loot_table_roll_produces_items() {
        let mut t = LootTable::new().with_rolls(3).with_drop_chance(1.0);
        t.add_item(ItemId(100), 1.0, 1);
        t.add_item(ItemId(101), 1.0, 1);
        let mut rng = Rng::new(42);
        let loot = t.roll(&mut rng, 1, &[]);
        assert!(!loot.is_empty());
    }

    #[test]
    fn loot_table_empty_chest_produces_nothing() {
        let t = LootTable::empty_chest();
        let mut rng = Rng::new(0);
        for _ in 0..10 {
            assert!(t.roll(&mut rng, 1, &[]).is_empty());
        }
    }

    #[test]
    fn loot_condition_min_level_gates() {
        let mut t = LootTable::new().with_rolls(1).with_drop_chance(1.0);
        t.add_entry(
            LootEntry::new(ItemId(200), 1.0, 1, 1)
                .with_condition(LootCondition::MinLevel(10)),
        );
        let mut rng = Rng::new(1);
        // Under-level: no drops.
        let loot_low = t.roll(&mut rng, 5, &[]);
        // The eligible list will be empty → no drops.
        // Over-level: may get drops.
        let loot_high = t.roll(&mut rng, 15, &[]);
        assert!(loot_low.is_empty());
        assert!(!loot_high.is_empty());
    }

    #[test]
    fn loot_table_gold_range() {
        let t = LootTable::new()
            .with_drop_chance(1.0)
            .with_gold(10, 50);
        let mut rng = Rng::new(99);
        // Add a dummy entry so we get a drop.
        let mut t2 = t;
        t2.add_item(ItemId(1), 1.0, 1);
        for _ in 0..20 {
            let loot = t2.roll(&mut rng, 1, &[]);
            assert!(loot.gold >= 10 && loot.gold <= 50,
                "gold {} out of range", loot.gold);
        }
    }

    #[test]
    fn loot_merge() {
        let mut a = Loot::new();
        a.add(ItemId(1), 3);
        a.add_gold(10);
        let mut b = Loot::new();
        b.add(ItemId(1), 2);
        b.add(ItemId(2), 5);
        b.add_gold(20);
        a.merge(b);
        assert_eq!(a.gold, 30);
        assert_eq!(a.drops.iter().find(|(i, _)| *i == ItemId(1)).unwrap().1, 5);
        assert_eq!(a.drops.iter().find(|(i, _)| *i == ItemId(2)).unwrap().1, 5);
    }

    #[test]
    fn inventory_stats_weight_fraction() {
        let mut inv = make_weighted_inv(10, 100.0);
        inv.add_item(ItemInstance::new_stack(ItemId(1), 10), wi(5.0, 10, ItemCategory::Material)).unwrap();
        let stats = InventoryStats::from_inventory(&inv);
        assert!((stats.weight_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn find_item_returns_correct_slot() {
        let mut inv = make_inv(5);
        inv.add_item(ItemInstance::new(ItemId(1)), wi(1.0, 1, ItemCategory::Weapon)).unwrap();
        inv.add_item(ItemInstance::new(ItemId(2)), wi(1.0, 1, ItemCategory::Weapon)).unwrap();
        assert_eq!(inv.find_item(ItemId(2)), Some(SlotIndex(1)));
        assert_eq!(inv.find_item(ItemId(99)), None);
    }

    #[test]
    fn no_duplicates_loot_table() {
        let mut t = LootTable::new()
            .with_rolls(5)
            .with_drop_chance(1.0)
            .no_duplicates();
        t.add_item(ItemId(1), 1.0, 1);
        t.add_item(ItemId(2), 1.0, 1);
        t.add_item(ItemId(3), 1.0, 1);
        let mut rng = Rng::new(777);
        let loot = t.roll(&mut rng, 1, &[]);
        // At most 3 distinct items.
        assert!(loot.item_count() <= 3);
    }
}
