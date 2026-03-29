
// ============================================================
// SECTION 92: LOOT TABLE INHERITANCE
// ============================================================

pub struct LootTableHierarchy {
    pub tables: std::collections::HashMap<u32, LootTable>,
    pub parent_map: std::collections::HashMap<u32, u32>,
}

impl LootTableHierarchy {
    pub fn new() -> Self { Self { tables: std::collections::HashMap::new(), parent_map: std::collections::HashMap::new() } }

    pub fn add_table(&mut self, table: LootTable, parent_id: Option<u32>) {
        let id = table.id;
        self.tables.insert(id, table);
        if let Some(parent) = parent_id { self.parent_map.insert(id, parent); }
    }

    pub fn get_inherited_entries(&self, table_id: u32) -> Vec<LootTableEntry> {
        let mut entries = Vec::new();
        let mut current_id = table_id;
        let mut visited = std::collections::HashSet::new();
        loop {
            if visited.contains(&current_id) { break; }
            visited.insert(current_id);
            if let Some(table) = self.tables.get(&current_id) {
                entries.extend(table.entries.clone());
            }
            match self.parent_map.get(&current_id) {
                Some(&parent) => current_id = parent,
                None => break,
            }
        }
        entries
    }

    pub fn roll_hierarchical(&self, table_id: u32, rng: &mut LootRng) -> Option<u32> {
        let entries = self.get_inherited_entries(table_id);
        if entries.is_empty() { return None; }
        let total: f32 = entries.iter().map(|e| e.weight).sum();
        let r = rng.next_f32() * total;
        let mut cum = 0.0f32;
        for e in &entries {
            cum += e.weight;
            if r < cum { return Some(e.item_id); }
        }
        entries.last().map(|e| e.item_id)
    }
}

// ============================================================
// SECTION 93: SALVAGE SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct SalvageResult {
    pub item_id: u32,
    pub materials_gained: Vec<(u32, u32)>,  // (material_id, quantity)
    pub gold_gained: u32,
}

pub struct SalvageSystem {
    pub rng: LootRng,
    pub base_material_id: u32,
}

impl SalvageSystem {
    pub fn new(seed: u64, base_material_id: u32) -> Self { Self { rng: LootRng::new(seed), base_material_id } }

    pub fn salvage(&mut self, item: &GeneratedItem) -> SalvageResult {
        let rarity_mat_bonus = match &item.rarity {
            ItemRarity::Common => 1, ItemRarity::Uncommon => 2, ItemRarity::Rare => 4,
            ItemRarity::Epic => 8, ItemRarity::Legendary => 20, ItemRarity::Mythic => 50,
            ItemRarity::BossExclusive => 30,
        };
        let base_qty = 1 + self.rng.next_u32() % 3;
        let total_qty = base_qty * rarity_mat_bonus;
        let gold = item.sell_value / 4;
        SalvageResult {
            item_id: item.base_item_id,
            materials_gained: vec![(self.base_material_id, total_qty)],
            gold_gained: gold,
        }
    }

    pub fn batch_salvage(&mut self, items: &[GeneratedItem]) -> Vec<SalvageResult> {
        items.iter().map(|i| self.salvage(i)).collect()
    }

    pub fn total_materials_from_batch(&mut self, items: &[GeneratedItem]) -> u32 {
        self.batch_salvage(items).iter().map(|r| r.materials_gained.iter().map(|(_, q)| q).sum::<u32>()).sum()
    }
}

// ============================================================
// SECTION 94: DROP STREAK SYSTEM
// ============================================================

pub struct DropStreakTracker {
    pub dry_streak: u32,
    pub hot_streak: u32,
    pub best_streak: u32,
    pub worst_drought: u32,
    pub total_rolls: u32,
    pub total_drops: u32,
    pub last_drop_roll: u32,
}

impl DropStreakTracker {
    pub fn new() -> Self { Self { dry_streak: 0, hot_streak: 0, best_streak: 0, worst_drought: 0, total_rolls: 0, total_drops: 0, last_drop_roll: 0 } }

    pub fn record_roll(&mut self, got_drop: bool) {
        self.total_rolls += 1;
        if got_drop {
            self.total_drops += 1;
            if self.dry_streak > self.worst_drought { self.worst_drought = self.dry_streak; }
            self.dry_streak = 0;
            self.hot_streak += 1;
            if self.hot_streak > self.best_streak { self.best_streak = self.hot_streak; }
            self.last_drop_roll = self.total_rolls;
        } else {
            self.dry_streak += 1;
            self.hot_streak = 0;
        }
    }

    pub fn drop_rate(&self) -> f32 {
        if self.total_rolls == 0 { return 0.0; }
        self.total_drops as f32 / self.total_rolls as f32
    }

    pub fn rolls_since_last_drop(&self) -> u32 { self.total_rolls - self.last_drop_roll }
}

// ============================================================
// SECTION 95: FINAL LOOT SYSTEM VALIDATION
// ============================================================

pub fn validate_loot_system() -> bool {
    // Validate item catalog
    let catalog = build_extended_item_catalog();
    assert!(catalog.len() >= 100, "Catalog too small");

    // Validate crafting
    let mut crafting = CraftingSystem::new();
    crafting.build_standard_recipes();
    assert!(!crafting.recipes.is_empty(), "No crafting recipes");

    // Validate enchantments
    let mut enchants = EnchantmentLibrary::new();
    enchants.build_standard_library();
    assert!(!enchants.enchantments.is_empty(), "No enchantments");

    // Validate item generation
    let mut gen = ItemGenerator::new(1);
    let item = gen.generate_item(100, "Test", 50, 100.0);
    assert!(item.sell_value > 0 || item.rarity == ItemRarity::Common);

    // Validate dungeon
    let run = DungeonRun::generate_standard_dungeon(1, 50, 3);
    assert!(run.rooms.len() >= 5);

    // Validate chest
    let mut chest = TreasureChestSystem::new(1);
    let result = chest.open_chest(&ChestType::Gold, 50, 100.0);
    assert!(result.gold >= 200);

    true
}

#[test]
fn test_loot_system_validation() {
    assert!(validate_loot_system());
}

pub fn loot_editor_version() -> &'static str {
    "LootEditor v2.0 - Comprehensive Drop System"
}
