//! Spawn tables — weighted creature/item spawn selection.
//!
//! Provides tiered, depth-scaled spawn tables used by dungeon floors.
//! Each spawn table entry has:
//! - A weight (higher = more common)
//! - A minimum depth (won't appear before this floor)
//! - An optional maximum depth (won't appear after this floor)
//! - An optional group tag for filtering by category

use super::Rng;

// ── SpawnTier ─────────────────────────────────────────────────────────────────

/// Rarity tier of a spawn entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpawnTier {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Boss,
}

impl SpawnTier {
    /// Base weight multiplier for this tier.
    pub fn base_weight(self) -> f32 {
        match self {
            SpawnTier::Common    => 100.0,
            SpawnTier::Uncommon  => 40.0,
            SpawnTier::Rare      => 15.0,
            SpawnTier::Epic      => 5.0,
            SpawnTier::Legendary => 1.5,
            SpawnTier::Boss      => 1.0,
        }
    }

    /// Name string.
    pub fn name(self) -> &'static str {
        match self {
            SpawnTier::Common    => "Common",
            SpawnTier::Uncommon  => "Uncommon",
            SpawnTier::Rare      => "Rare",
            SpawnTier::Epic      => "Epic",
            SpawnTier::Legendary => "Legendary",
            SpawnTier::Boss      => "Boss",
        }
    }

    /// Display colour for this tier.
    pub fn color(self) -> glam::Vec4 {
        match self {
            SpawnTier::Common    => glam::Vec4::new(0.8, 0.8, 0.8, 1.0),
            SpawnTier::Uncommon  => glam::Vec4::new(0.0, 1.0, 0.2, 1.0),
            SpawnTier::Rare      => glam::Vec4::new(0.2, 0.5, 1.0, 1.0),
            SpawnTier::Epic      => glam::Vec4::new(0.7, 0.0, 1.0, 1.0),
            SpawnTier::Legendary => glam::Vec4::new(1.0, 0.6, 0.0, 1.0),
            SpawnTier::Boss      => glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
        }
    }
}

// ── SpawnEntry ────────────────────────────────────────────────────────────────

/// One entry in a spawn table.
#[derive(Debug, Clone)]
pub struct SpawnEntry {
    /// Unique identifier (e.g., "skeleton_archer").
    pub id:        String,
    /// Display name.
    pub name:      String,
    /// Selection weight (positive).
    pub weight:    f32,
    /// Rarity tier.
    pub tier:      SpawnTier,
    /// Minimum dungeon depth to appear.
    pub min_depth: u32,
    /// Maximum dungeon depth (u32::MAX = no limit).
    pub max_depth: u32,
    /// Category tags (e.g., "undead", "melee", "ranged").
    pub tags:      Vec<String>,
    /// Group count: how many of this type spawn at once (min, max).
    pub group:     (u32, u32),
    /// Scaled properties: (base_hp, base_damage, base_xp).
    pub stats:     (f32, f32, f32),
}

impl SpawnEntry {
    pub fn new(id: impl Into<String>, name: impl Into<String>, tier: SpawnTier) -> Self {
        let id = id.into();
        let name = name.into();
        let weight = tier.base_weight();
        Self {
            id, name, weight, tier,
            min_depth: 1,
            max_depth: u32::MAX,
            tags:  Vec::new(),
            group: (1, 1),
            stats: (10.0, 2.0, 5.0),
        }
    }

    pub fn with_depth(mut self, min: u32, max: u32) -> Self {
        self.min_depth = min; self.max_depth = max; self
    }

    pub fn with_weight(mut self, w: f32) -> Self { self.weight = w; self }

    pub fn with_group(mut self, min: u32, max: u32) -> Self {
        self.group = (min, max); self
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|&s| s.to_string()).collect(); self
    }

    pub fn with_stats(mut self, hp: f32, dmg: f32, xp: f32) -> Self {
        self.stats = (hp, dmg, xp); self
    }

    /// Is this entry valid for the given depth?
    pub fn valid_for_depth(&self, depth: u32) -> bool {
        depth >= self.min_depth && depth <= self.max_depth
    }

    /// Does this entry have the given tag?
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Compute depth-scaled stats: hp and xp scale with depth.
    pub fn scaled_stats(&self, depth: u32) -> (f32, f32, f32) {
        let scale = 1.0 + (depth as f32 - 1.0) * 0.15;
        let (hp, dmg, xp) = self.stats;
        (hp * scale, dmg * (1.0 + (depth as f32 - 1.0) * 0.08), xp * scale)
    }
}

// ── SpawnResult ───────────────────────────────────────────────────────────────

/// Result of a spawn roll.
#[derive(Debug, Clone)]
pub struct SpawnResult {
    pub entry:   SpawnEntry,
    pub count:   u32,
    pub position: Option<(i32, i32)>,
}

// ── SpawnTable ────────────────────────────────────────────────────────────────

/// A weighted spawn table.
///
/// Usage:
/// ```text
/// let mut table = SpawnTable::new();
/// table.add(SpawnEntry::new("goblin", "Goblin", SpawnTier::Common));
/// table.add(SpawnEntry::new("orc", "Orc", SpawnTier::Uncommon).with_depth(3, u32::MAX));
/// let result = table.roll(&mut rng, 5);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SpawnTable {
    entries: Vec<SpawnEntry>,
}

impl SpawnTable {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn add(&mut self, entry: SpawnEntry) -> &mut Self {
        self.entries.push(entry);
        self
    }

    /// Add multiple entries from a slice.
    pub fn add_many(&mut self, entries: Vec<SpawnEntry>) -> &mut Self {
        self.entries.extend(entries);
        self
    }

    /// Roll for a single spawn at `depth`, optionally filtered by `tag`.
    pub fn roll_one(&self, rng: &mut Rng, depth: u32, tag: Option<&str>) -> Option<&SpawnEntry> {
        let valid: Vec<(&SpawnEntry, f32)> = self.entries.iter()
            .filter(|e| e.valid_for_depth(depth))
            .filter(|e| tag.map_or(true, |t| e.has_tag(t)))
            .map(|e| (e, e.weight))
            .collect();
        rng.pick_weighted(&valid).copied()
    }

    /// Roll for `n` spawns at `depth`. May return fewer if table is small.
    pub fn roll(&self, rng: &mut Rng, n: usize, depth: u32) -> Vec<SpawnResult> {
        (0..n).filter_map(|_| {
            self.roll_one(rng, depth, None).map(|entry| {
                let count = rng.range_i32(entry.group.0 as i32, entry.group.1 as i32) as u32;
                SpawnResult { entry: entry.clone(), count, position: None }
            })
        }).collect()
    }

    /// Roll guaranteeing at least one entry from each tier in `tiers`.
    pub fn roll_guaranteed(&self, rng: &mut Rng, depth: u32, tiers: &[SpawnTier]) -> Vec<SpawnResult> {
        let mut results = Vec::new();
        for &tier in tiers {
            let valid: Vec<(&SpawnEntry, f32)> = self.entries.iter()
                .filter(|e| e.tier == tier && e.valid_for_depth(depth))
                .map(|e| (e, e.weight))
                .collect();
            if let Some(entry) = rng.pick_weighted(&valid).copied() {
                let count = rng.range_i32(entry.group.0 as i32, entry.group.1 as i32) as u32;
                results.push(SpawnResult { entry: entry.clone(), count, position: None });
            }
        }
        results
    }

    /// Get all entries with a given tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&SpawnEntry> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    /// Get entries valid for depth, sorted by weight descending.
    pub fn available_at_depth(&self, depth: u32) -> Vec<&SpawnEntry> {
        let mut entries: Vec<&SpawnEntry> = self.entries.iter()
            .filter(|e| e.valid_for_depth(depth))
            .collect();
        entries.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());
        entries
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

// ── Default spawn tables ───────────────────────────────────────────────────────

/// Build the default creature spawn table for chaos-rpg.
pub fn chaos_rpg_creatures() -> SpawnTable {
    let mut t = SpawnTable::new();

    // Floor 1-3: weak undead
    t.add(SpawnEntry::new("skeleton",       "Skeleton",        SpawnTier::Common)
           .with_depth(1, 8).with_group(1, 3).with_tags(&["undead", "melee"])
           .with_stats(8.0, 2.0, 4.0));
    t.add(SpawnEntry::new("zombie",         "Zombie",          SpawnTier::Common)
           .with_depth(1, 6).with_group(1, 2).with_tags(&["undead", "melee"])
           .with_stats(15.0, 1.5, 3.0));
    t.add(SpawnEntry::new("skeleton_archer","Skeleton Archer",  SpawnTier::Uncommon)
           .with_depth(2, 10).with_group(1, 2).with_tags(&["undead", "ranged"])
           .with_stats(6.0, 3.0, 6.0));

    // Floor 3-7: mid-tier
    t.add(SpawnEntry::new("cave_troll",     "Cave Troll",      SpawnTier::Uncommon)
           .with_depth(3, 12).with_group(1, 1).with_tags(&["beast", "melee"])
           .with_stats(30.0, 5.0, 15.0));
    t.add(SpawnEntry::new("shadow_wraith",  "Shadow Wraith",   SpawnTier::Rare)
           .with_depth(4, 15).with_group(1, 1).with_tags(&["undead", "shadow", "melee"])
           .with_stats(20.0, 6.0, 25.0));
    t.add(SpawnEntry::new("chaos_imp",      "Chaos Imp",       SpawnTier::Uncommon)
           .with_depth(3, 20).with_group(2, 4).with_tags(&["demon", "chaos"])
           .with_stats(5.0, 4.0, 8.0));

    // Floor 5+: strong
    t.add(SpawnEntry::new("stone_golem",    "Stone Golem",     SpawnTier::Rare)
           .with_depth(5, u32::MAX).with_group(1, 1).with_tags(&["construct", "melee"])
           .with_stats(50.0, 8.0, 40.0));
    t.add(SpawnEntry::new("lich",           "Lich",            SpawnTier::Epic)
           .with_depth(6, u32::MAX).with_group(1, 1).with_tags(&["undead", "caster"])
           .with_stats(40.0, 12.0, 80.0));
    t.add(SpawnEntry::new("void_stalker",   "Void Stalker",    SpawnTier::Rare)
           .with_depth(7, u32::MAX).with_group(1, 2).with_tags(&["void", "ranged"])
           .with_stats(25.0, 10.0, 55.0));

    // Rare elites
    t.add(SpawnEntry::new("chaos_champion", "Chaos Champion",  SpawnTier::Epic)
           .with_depth(8, u32::MAX).with_group(1, 1).with_tags(&["demon", "melee", "elite"])
           .with_stats(80.0, 15.0, 120.0));
    t.add(SpawnEntry::new("elder_dragon",   "Elder Dragon",    SpawnTier::Legendary)
           .with_depth(10, u32::MAX).with_group(1, 1).with_tags(&["dragon", "elite"])
           .with_stats(200.0, 25.0, 500.0));

    // Boss-only
    t.add(SpawnEntry::new("bone_king",      "Bone King",       SpawnTier::Boss)
           .with_depth(3, 3).with_group(1, 1).with_tags(&["undead", "boss"])
           .with_stats(120.0, 18.0, 300.0));
    t.add(SpawnEntry::new("chaos_archon",   "Chaos Archon",    SpawnTier::Boss)
           .with_depth(6, 6).with_group(1, 1).with_tags(&["demon", "chaos", "boss"])
           .with_stats(250.0, 30.0, 800.0));
    t.add(SpawnEntry::new("void_sovereign", "Void Sovereign",  SpawnTier::Boss)
           .with_depth(10, 10).with_group(1, 1).with_tags(&["void", "boss"])
           .with_stats(500.0, 50.0, 2000.0));

    t
}

/// Build the default item spawn table.
pub fn chaos_rpg_items() -> SpawnTable {
    let mut t = SpawnTable::new();

    t.add(SpawnEntry::new("health_potion",  "Health Potion",   SpawnTier::Common)
           .with_stats(0.0, 0.0, 0.0));
    t.add(SpawnEntry::new("mana_potion",    "Mana Potion",     SpawnTier::Common));
    t.add(SpawnEntry::new("iron_sword",     "Iron Sword",      SpawnTier::Common)
           .with_depth(1, 4));
    t.add(SpawnEntry::new("steel_sword",    "Steel Sword",     SpawnTier::Uncommon)
           .with_depth(3, 8));
    t.add(SpawnEntry::new("chaos_shard",    "Chaos Shard",     SpawnTier::Rare)
           .with_depth(4, u32::MAX));
    t.add(SpawnEntry::new("void_crystal",   "Void Crystal",    SpawnTier::Epic)
           .with_depth(7, u32::MAX));
    t.add(SpawnEntry::new("amulet_of_chaos","Amulet of Chaos", SpawnTier::Legendary)
           .with_depth(9, u32::MAX));

    t
}
