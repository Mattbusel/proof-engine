//! Loot tables — item drops with rarity-tiered probability.

use super::{Rng, spawn::SpawnTier};

// ── LootTier ──────────────────────────────────────────────────────────────────

/// Rarity tier of a loot drop.
pub type LootTier = SpawnTier;

// ── LootDrop ──────────────────────────────────────────────────────────────────

/// One possible drop from a loot table.
#[derive(Debug, Clone)]
pub struct LootDrop {
    pub id:       String,
    pub name:     String,
    pub tier:     LootTier,
    pub quantity: (u32, u32),   // min, max stack size
    pub weight:   f32,
    pub depth_min: u32,
    pub depth_max: u32,
}

impl LootDrop {
    pub fn new(id: impl Into<String>, name: impl Into<String>, tier: LootTier) -> Self {
        let weight = tier.base_weight();
        Self {
            id: id.into(), name: name.into(), tier, quantity: (1, 1),
            weight, depth_min: 1, depth_max: u32::MAX,
        }
    }

    pub fn with_quantity(mut self, min: u32, max: u32) -> Self {
        self.quantity = (min, max); self
    }

    pub fn with_depth(mut self, min: u32, max: u32) -> Self {
        self.depth_min = min; self.depth_max = max; self
    }

    pub fn with_weight(mut self, w: f32) -> Self { self.weight = w; self }

    pub fn is_valid_at(&self, depth: u32) -> bool {
        depth >= self.depth_min && depth <= self.depth_max
    }
}

// ── LootTable ─────────────────────────────────────────────────────────────────

/// A loot table mapping items to weights.
#[derive(Debug, Clone, Default)]
pub struct LootTable {
    drops: Vec<LootDrop>,
    /// Probability that any drop occurs at all (0=always empty, 1=always drop).
    pub drop_chance: f32,
    /// Number of rolls per drop event.
    pub rolls: u32,
}

/// Rolled loot result.
#[derive(Debug, Clone)]
pub struct RolledLoot {
    pub drop:     LootDrop,
    pub quantity: u32,
}

impl LootTable {
    pub fn new(drop_chance: f32, rolls: u32) -> Self {
        Self { drops: Vec::new(), drop_chance: drop_chance.clamp(0.0, 1.0), rolls }
    }

    pub fn add(&mut self, drop: LootDrop) -> &mut Self {
        self.drops.push(drop); self
    }

    /// Roll loot at given depth. Returns a vec of dropped items.
    pub fn roll(&self, rng: &mut Rng, depth: u32) -> Vec<RolledLoot> {
        let mut result = Vec::new();
        for _ in 0..self.rolls {
            if !rng.chance(self.drop_chance) { continue; }

            let valid: Vec<(&LootDrop, f32)> = self.drops.iter()
                .filter(|d| d.is_valid_at(depth))
                .map(|d| (d, d.weight))
                .collect();

            if let Some(drop) = rng.pick_weighted(&valid).copied() {
                let qty = rng.range_i32(drop.quantity.0 as i32, drop.quantity.1 as i32) as u32;
                result.push(RolledLoot { drop: drop.clone(), quantity: qty });
            }
        }
        result
    }

    /// Roll from a specific tier only.
    pub fn roll_tier(&self, rng: &mut Rng, depth: u32, tier: LootTier) -> Option<RolledLoot> {
        let valid: Vec<(&LootDrop, f32)> = self.drops.iter()
            .filter(|d| d.tier == tier && d.is_valid_at(depth))
            .map(|d| (d, d.weight))
            .collect();
        rng.pick_weighted(&valid).copied().map(|drop| {
            let qty = rng.range_i32(drop.quantity.0 as i32, drop.quantity.1 as i32) as u32;
            RolledLoot { drop: drop.clone(), quantity: qty }
        })
    }

    pub fn len(&self) -> usize { self.drops.len() }
}

/// Default chaos-rpg general loot table.
pub fn chaos_rpg_loot() -> LootTable {
    let mut t = LootTable::new(0.65, 2);

    // Consumables (very common)
    t.add(LootDrop::new("health_potion",  "Health Potion",    LootTier::Common).with_quantity(1, 3));
    t.add(LootDrop::new("mana_potion",    "Mana Potion",      LootTier::Common).with_quantity(1, 2));
    t.add(LootDrop::new("antidote",       "Antidote",         LootTier::Common).with_quantity(1, 2));

    // Gold
    t.add(LootDrop::new("gold",           "Gold",             LootTier::Common).with_quantity(1, 50));

    // Equipment
    t.add(LootDrop::new("rusty_dagger",   "Rusty Dagger",     LootTier::Common).with_depth(1, 3));
    t.add(LootDrop::new("iron_sword",     "Iron Sword",       LootTier::Common).with_depth(1, 5));
    t.add(LootDrop::new("chainmail",      "Chainmail",        LootTier::Uncommon).with_depth(2, 7));
    t.add(LootDrop::new("steel_sword",    "Steel Sword",      LootTier::Uncommon).with_depth(3, 10));
    t.add(LootDrop::new("chaos_blade",    "Chaos Blade",      LootTier::Rare).with_depth(5, u32::MAX));
    t.add(LootDrop::new("void_staff",     "Void Staff",       LootTier::Epic).with_depth(7, u32::MAX));
    t.add(LootDrop::new("crown_of_chaos", "Crown of Chaos",   LootTier::Legendary).with_depth(9, u32::MAX));

    // Materials
    t.add(LootDrop::new("chaos_shard",    "Chaos Shard",      LootTier::Rare).with_quantity(1, 3));
    t.add(LootDrop::new("void_crystal",   "Void Crystal",     LootTier::Epic).with_depth(7, u32::MAX));

    t
}
