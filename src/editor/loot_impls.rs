
// ============================================================
// IMPL BLOCKS FOR LOOT EDITOR CORE TYPES
// ============================================================

impl ItemRarity {
    pub fn base_weight(&self) -> f32 {
        match self {
            ItemRarity::Common => 50.0, ItemRarity::Uncommon => 25.0,
            ItemRarity::Rare => 10.0, ItemRarity::Epic => 4.0,
            ItemRarity::Legendary => 1.0, ItemRarity::Mythic => 0.2,
            ItemRarity::BossExclusive => 0.5,
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            ItemRarity::Common => "white", ItemRarity::Uncommon => "green",
            ItemRarity::Rare => "blue", ItemRarity::Epic => "purple",
            ItemRarity::Legendary => "orange", ItemRarity::Mythic => "red",
            ItemRarity::BossExclusive => "gold",
        }
    }

    pub fn base_value_mult(&self) -> f32 {
        match self {
            ItemRarity::Common => 1.0, ItemRarity::Uncommon => 3.0,
            ItemRarity::Rare => 10.0, ItemRarity::Epic => 40.0,
            ItemRarity::Legendary => 200.0, ItemRarity::Mythic => 1000.0,
            ItemRarity::BossExclusive => 500.0,
        }
    }

    pub fn drop_rate_pct(&self) -> f32 {
        match self {
            ItemRarity::Common => 0.55, ItemRarity::Uncommon => 0.28,
            ItemRarity::Rare => 0.12, ItemRarity::Epic => 0.04,
            ItemRarity::Legendary => 0.008, ItemRarity::Mythic => 0.002,
            ItemRarity::BossExclusive => 0.005,
        }
    }
}

impl ItemStats {
    pub fn new() -> Self {
        Self { attack: 0.0, defense: 0.0, speed: 0.0, magic: 0.0, hp: 0.0, mp: 0.0, crit_chance: 0.0, crit_damage: 1.5 }
    }

    pub fn weapon(attack: f32) -> Self { let mut s = Self::new(); s.attack = attack; s }
    pub fn armor(defense: f32) -> Self { let mut s = Self::new(); s.defense = defense; s }

    pub fn total_power(&self) -> f32 {
        self.attack + self.defense + self.magic + (self.hp / 10.0) + (self.mp / 10.0) + self.speed * 2.0
    }
}

impl Item {
    pub fn new(id: u32, name: &str, item_type: ItemType, rarity: ItemRarity, base_value: f32) -> Self {
        Self {
            id, name: name.to_string(), description: String::new(),
            item_type, rarity, base_value, weight: 1.0,
            stats: ItemStats::new(), set_id: None, level_requirement: 1,
            zone: "world".to_string(), stackable: false, max_stack: 1, tags: Vec::new(),
        }
    }

    pub fn market_value(&self) -> f32 {
        self.base_value * self.rarity.base_value_mult()
    }

    pub fn sell_value(&self) -> f32 { self.market_value() * 0.3 }

    pub fn with_stats(mut self, stats: ItemStats) -> Self { self.stats = stats; self }
    pub fn with_level(mut self, lvl: u32) -> Self { self.level_requirement = lvl; self }
    pub fn with_zone(mut self, zone: &str) -> Self { self.zone = zone.to_string(); self }
    pub fn with_set(mut self, set_id: u32) -> Self { self.set_id = Some(set_id); self }
    pub fn with_stackable(mut self, max: u32) -> Self { self.stackable = true; self.max_stack = max; self }
    pub fn with_description(mut self, desc: &str) -> Self { self.description = desc.to_string(); self }
    pub fn with_tag(mut self, tag: &str) -> Self { self.tags.push(tag.to_string()); self }
}

impl LootRng {
    pub fn new(seed: u64) -> Self { Self { state: seed.wrapping_add(1) } }

    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    pub fn next_u32(&mut self) -> u32 { (self.next_u64() >> 32) as u32 }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() as f32) / (u64::MAX as f32)
    }

    pub fn next_f32_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }

    pub fn next_u32_range(&mut self, min: u32, max: u32) -> u32 {
        if min >= max { return min; }
        min + (self.next_u32() % (max - min))
    }

    pub fn shuffle<T>(&mut self, slice: &mut Vec<T>) {
        for i in (1..slice.len()).rev() {
            let j = self.next_u32() as usize % (i + 1);
            slice.swap(i, j);
        }
    }
}

impl AliasTable {
    pub fn build(weights: &[f32]) -> Self {
        let n = weights.len();
        let total: f32 = weights.iter().sum();
        let avg = total / n as f32;
        let mut prob = vec![0.0f32; n];
        let mut alias = vec![0usize; n];
        let mut small = Vec::new();
        let mut large = Vec::new();
        for (i, &w) in weights.iter().enumerate() {
            prob[i] = w / avg;
            if prob[i] < 1.0 { small.push(i); } else { large.push(i); }
        }
        while !small.is_empty() && !large.is_empty() {
            let s = small.pop().unwrap();
            let l = large.pop().unwrap();
            alias[s] = l;
            prob[l] -= 1.0 - prob[s];
            if prob[l] < 1.0 { small.push(l); } else { large.push(l); }
        }
        Self { prob, alias, n }
    }

    pub fn sample(&self, rng: &mut LootRng) -> usize {
        let i = rng.next_u32() as usize % self.n;
        if rng.next_f32() < self.prob[i] { i } else { self.alias[i] }
    }
}

impl PitySystem {
    pub fn new(base_rate: f32, soft_pity_start: u32, hard_pity: u32) -> Self {
        Self { base_rate, current_rolls: 0, soft_pity_start, hard_pity, soft_pity_increase: 0.05 }
    }

    pub fn roll(&mut self, rng: &mut LootRng) -> bool {
        self.current_rolls += 1;
        let rate = if self.current_rolls >= self.hard_pity {
            1.0
        } else if self.current_rolls >= self.soft_pity_start {
            let extra = (self.current_rolls - self.soft_pity_start) as f32;
            (self.base_rate + extra * self.soft_pity_increase).min(1.0)
        } else {
            self.base_rate
        };
        if rng.next_f32() < rate {
            self.current_rolls = 0;
            true
        } else {
            false
        }
    }

    pub fn guaranteed_in(&self) -> u32 {
        self.hard_pity.saturating_sub(self.current_rolls)
    }
}

impl DropContext {
    pub fn new(player_level: u32) -> Self {
        Self {
            player_level, zone_id: 1,
            completed_quests: HashSet::new(),
            kill_count: 0, difficulty: 1,
            death_count: 0, rng_value: 0.5,
        }
    }
}

impl LootTable {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id, name: name.to_string(), entries: Vec::new(),
            roll_count: RollCountMode::Constant(1),
            guaranteed_entries: Vec::new(),
            boss_exclusive: false, pity_enabled: false,
            pity_threshold: 100, max_drops: None,
            min_rolls: 1, max_rolls: 1,
        }
    }

    pub fn total_weight(&self) -> f32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn add_entry(&mut self, item_id: u32, weight: f32, min_qty: u32, max_qty: u32) {
        self.entries.push(LootTableEntry {
            kind: LootEntryKind::Item { item_id },
            weight, min_count: min_qty, max_count: max_qty,
            conditions: Vec::new(), guaranteed: false,
            item_id, min_quantity: min_qty, max_quantity: max_qty, condition: None,
        });
    }

    pub fn add_guaranteed(&mut self, item_id: u32) {
        let idx = self.entries.len();
        self.entries.push(LootTableEntry {
            kind: LootEntryKind::Item { item_id },
            weight: 0.0, min_count: 1, max_count: 1,
            conditions: Vec::new(), guaranteed: true,
            item_id, min_quantity: 1, max_quantity: 1, condition: None,
        });
        self.guaranteed_entries.push(idx as u32);
    }

    pub fn roll_simple(&self, rng: &mut LootRng) -> Option<u32> {
        let non_guaranteed: Vec<&LootTableEntry> = self.entries.iter().filter(|e| !e.guaranteed).collect();
        if non_guaranteed.is_empty() { return None; }
        let total: f32 = non_guaranteed.iter().map(|e| e.weight).sum();
        let r = rng.next_f32() * total;
        let mut cum = 0.0f32;
        for e in &non_guaranteed {
            cum += e.weight;
            if r < cum { return Some(e.item_id); }
        }
        non_guaranteed.last().map(|e| e.item_id)
    }
}

impl LootRoller {
    pub fn new(seed: u64) -> Self {
        Self { rng: LootRng::new(seed), pity_trackers: HashMap::new() }
    }

    pub fn roll_table(&mut self, table: &LootTable, _catalog: &[Item], _ctx: &mut DropContext) -> Vec<DropResult> {
        let mut results = Vec::new();
        // Guaranteed drops
        for &idx in &table.guaranteed_entries {
            if let Some(e) = table.entries.get(idx as usize) {
                let count = if e.min_count == e.max_count { e.min_count } else {
                    e.min_count + self.rng.next_u32() % (e.max_count - e.min_count + 1)
                };
                results.push(DropResult { item_id: e.item_id, count, is_guaranteed: true, from_pity: false });
            }
        }
        // Normal roll
        if let Some(item_id) = table.roll_simple(&mut self.rng) {
            let e = table.entries.iter().find(|e| e.item_id == item_id).unwrap();
            let count = e.min_count.max(1);
            results.push(DropResult { item_id, count, is_guaranteed: false, from_pity: false });
        }
        results
    }
}

impl MonteCarloResult {
    pub fn new() -> Self {
        Self { runs: 0, item_frequencies: HashMap::new(), total_value_per_run: Vec::new(), drops_per_run: Vec::new() }
    }

    pub fn drops_mean(&self) -> f32 {
        if self.drops_per_run.is_empty() { return 0.0; }
        self.drops_per_run.iter().sum::<u32>() as f32 / self.drops_per_run.len() as f32
    }

    pub fn expected_value(&self) -> f32 {
        if self.total_value_per_run.is_empty() { return 0.0; }
        self.total_value_per_run.iter().sum::<f32>() / self.total_value_per_run.len() as f32
    }

    pub fn item_drop_rate(&self, item_id: u32) -> f32 {
        if self.runs == 0 { return 0.0; }
        *self.item_frequencies.get(&item_id).unwrap_or(&0) as f32 / self.runs as f32
    }

    pub fn p10(&self) -> f32 { self.percentile(0.10) }
    pub fn p50(&self) -> f32 { self.percentile(0.50) }
    pub fn p90(&self) -> f32 { self.percentile(0.90) }
    pub fn p99(&self) -> f32 { self.percentile(0.99) }

    fn percentile(&self, p: f32) -> f32 {
        if self.total_value_per_run.is_empty() { return 0.0; }
        let mut sorted = self.total_value_per_run.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((sorted.len() as f32 * p) as usize).min(sorted.len() - 1);
        sorted[idx]
    }
}

impl LootBudget {
    pub fn new(budget: f32) -> Self { Self { total_budget: budget, remaining_budget: budget, allocated: Vec::new() } }

    pub fn allocate(&mut self, item_id: u32, value: f32) -> bool {
        if value <= self.remaining_budget {
            self.allocated.push((item_id, value));
            self.remaining_budget -= value;
            true
        } else { false }
    }

    pub fn utilization(&self) -> f32 { 1.0 - self.remaining_budget / self.total_budget }
}

impl DifficultyScaler {
    pub fn new(base: f32, scale: f32) -> Self { Self { base_drop_rate: base, difficulty_scale: scale, player_level: 1, zone_level: 1 } }

    pub fn adjusted_rate(&self) -> f32 {
        (self.base_drop_rate * (1.0 + self.difficulty_scale * self.zone_level as f32 * 0.1)).min(1.0)
    }
}

impl LootTableBuilder {
    pub fn new(id: u32, name: &str) -> Self {
        Self { table: LootTable::new(id, name) }
    }

    pub fn with_entry(mut self, item_id: u32, weight: f32) -> Self {
        self.table.add_entry(item_id, weight, 1, 1);
        self
    }

    pub fn build(self) -> LootTable { self.table }
}

impl LootEditor {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            catalog: build_item_catalog(),
            rng: LootRng::new(42),
            selected_table: None,
            show_statistics: false,
        }
    }

    pub fn add_table(&mut self, table: LootTable) {
        self.tables.insert(table.id, table);
    }

    pub fn roll_table(&mut self, table_id: u32) -> Vec<DropResult> {
        if let Some(table) = self.tables.get(&table_id) {
            let table = table.clone();
            let catalog = self.catalog.clone();
            let mut ctx = DropContext::new(20);
            ctx.rng_value = self.rng.next_f32();
            let mut roller = LootRoller::new(self.rng.next_u64());
            roller.roll_table(&table, &catalog, &mut ctx)
        } else { Vec::new() }
    }
}

// ============================================================
// SECTION: EXTENDED LOOT SYSTEM IMPLEMENTATIONS
// ============================================================

pub struct CraftingStation {
    pub name: String,
    pub station_type: String,
    pub recipes: Vec<u32>,
}

pub struct CraftingRecipe {
    pub id: u32,
    pub name: String,
    pub inputs: Vec<(u32, u32)>,
    pub output_item_id: u32,
    pub output_quantity: u32,
    pub skill_required: u32,
    pub success_chance: f32,
    pub byproduct_chance: f32,
    pub category: String,
}

pub enum ItemQuality {
    Broken, Poor, Normal, Fine, Superior, Masterwork, Legendary,
}

pub struct CraftingSystem {
    pub recipes: Vec<CraftingRecipe>,
    pub unlocked_recipes: HashSet<u32>,
    pub recipe_by_output: HashMap<u32, Vec<u32>>,
    pub recipe_by_station: HashMap<String, Vec<u32>>,
}

pub enum CraftResult {
    Success { outputs: Vec<(u32, u32, ItemQuality)>, experience: u32 },
    Failed { experience: u32 },
}

impl CraftingSystem {
    pub fn new() -> Self {
        Self { recipes: Vec::new(), unlocked_recipes: HashSet::new(), recipe_by_output: HashMap::new(), recipe_by_station: HashMap::new() }
    }

    pub fn add_recipe(&mut self, recipe: CraftingRecipe) {
        let id = recipe.id;
        let out = recipe.output_item_id;
        let cat = recipe.category.clone();
        self.recipes.push(recipe);
        self.recipe_by_output.entry(out).or_default().push(id);
        self.recipe_by_station.entry(cat).or_default().push(id);
    }

    pub fn craft(&self, recipe_id: u32, skill: u32, rng: &mut LootRng) -> CraftResult {
        if let Some(recipe) = self.recipes.iter().find(|r| r.id == recipe_id) {
            let chance = (recipe.success_chance + (skill as f32 * 0.01)).min(1.0);
            if rng.next_f32() < chance {
                let quality = if skill > 80 { ItemQuality::Masterwork } else if skill > 50 { ItemQuality::Superior } else { ItemQuality::Normal };
                CraftResult::Success { outputs: vec![(recipe.output_item_id, recipe.output_quantity, quality)], experience: 10 + skill / 5 }
            } else {
                CraftResult::Failed { experience: 5 }
            }
        } else {
            CraftResult::Failed { experience: 0 }
        }
    }

    pub fn build_standard_recipes(&mut self) {
        let recipes = vec![
            CraftingRecipe { id: 1, name: "Iron Sword".into(), inputs: vec![(101, 3), (102, 1)], output_item_id: 1, output_quantity: 1, skill_required: 10, success_chance: 0.9, byproduct_chance: 0.1, category: "blacksmith".into() },
            CraftingRecipe { id: 2, name: "Leather Vest".into(), inputs: vec![(201, 5)], output_item_id: 21, output_quantity: 1, skill_required: 5, success_chance: 0.95, byproduct_chance: 0.0, category: "tailor".into() },
            CraftingRecipe { id: 3, name: "Health Potion".into(), inputs: vec![(301, 2), (302, 1)], output_item_id: 51, output_quantity: 3, skill_required: 1, success_chance: 0.99, byproduct_chance: 0.0, category: "alchemy".into() },
            CraftingRecipe { id: 4, name: "Steel Sword".into(), inputs: vec![(101, 5), (103, 2), (102, 1)], output_item_id: 2, output_quantity: 1, skill_required: 30, success_chance: 0.8, byproduct_chance: 0.15, category: "blacksmith".into() },
            CraftingRecipe { id: 5, name: "Mana Potion".into(), inputs: vec![(303, 2), (302, 1)], output_item_id: 52, output_quantity: 3, skill_required: 10, success_chance: 0.95, byproduct_chance: 0.0, category: "alchemy".into() },
        ];
        for r in recipes { self.add_recipe(r); }
    }
}

// ============================================================
// SECTION: ENCHANTMENT LIBRARY
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum EnchantmentTarget { Weapon, Armor, Accessory, Any }

#[derive(Clone, Debug, PartialEq)]
pub enum EnchantmentEffect {
    DamageBonus(f32), DefenseBonus(f32), SpeedBonus(f32), MagicFind(f32),
    GoldFind(f32), FireDamage(f32), IceDamage(f32), LightningDamage(f32),
    PoisonDamage(f32), LifeSteal(f32), ManaSteal(f32), CooldownReduction(f32),
    ExpBonus(f32), CritChance(f32), CritDamage(f32),
}

#[derive(Clone, Debug)]
pub struct Enchantment {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub target: EnchantmentTarget,
    pub effects: Vec<EnchantmentEffect>,
    pub rarity: ItemRarity,
    pub max_rank: u32,
    pub cost: u32,
}

pub struct EnchantmentLibrary {
    pub enchantments: Vec<Enchantment>,
}

impl EnchantmentLibrary {
    pub fn new() -> Self { Self { enchantments: Vec::new() } }

    pub fn add(&mut self, ench: Enchantment) { self.enchantments.push(ench); }

    pub fn for_target(&self, target: &EnchantmentTarget) -> Vec<&Enchantment> {
        self.enchantments.iter().filter(|e| &e.target == target || e.target == EnchantmentTarget::Any).collect()
    }

    pub fn build_standard_library(&mut self) {
        let enchs = vec![
            Enchantment { id: 1, name: "Sharpness".into(), description: "+damage".into(), target: EnchantmentTarget::Weapon, effects: vec![EnchantmentEffect::DamageBonus(0.1)], rarity: ItemRarity::Common, max_rank: 5, cost: 50 },
            Enchantment { id: 2, name: "Fire Aspect".into(), description: "fire damage".into(), target: EnchantmentTarget::Weapon, effects: vec![EnchantmentEffect::FireDamage(15.0)], rarity: ItemRarity::Uncommon, max_rank: 3, cost: 100 },
            Enchantment { id: 3, name: "Protection".into(), description: "+defense".into(), target: EnchantmentTarget::Armor, effects: vec![EnchantmentEffect::DefenseBonus(0.1)], rarity: ItemRarity::Common, max_rank: 5, cost: 50 },
            Enchantment { id: 4, name: "Magic Find".into(), description: "more loot".into(), target: EnchantmentTarget::Accessory, effects: vec![EnchantmentEffect::MagicFind(0.05)], rarity: ItemRarity::Rare, max_rank: 3, cost: 200 },
            Enchantment { id: 5, name: "Lifesteal".into(), description: "heal on hit".into(), target: EnchantmentTarget::Weapon, effects: vec![EnchantmentEffect::LifeSteal(0.03)], rarity: ItemRarity::Epic, max_rank: 2, cost: 500 },
        ];
        for e in enchs { self.add(e); }
    }
}

// ============================================================
// SECTION: ITEM GENERATOR
// ============================================================

#[derive(Clone, Debug)]
pub struct GeneratedItem {
    pub base_item_id: u32,
    pub rarity: ItemRarity,
    pub level: u32,
    pub stats: ItemStats,
    pub sell_value: u32,
    pub enchantments: Vec<u32>,
    pub sockets: u32,
}

pub struct ItemGenerator {
    pub rng: LootRng,
}

impl ItemGenerator {
    pub fn new(seed: u64) -> Self { Self { rng: LootRng::new(seed) } }

    pub fn generate_item(&mut self, item_id: u32, _name: &str, level: u32, magic_find: f32) -> GeneratedItem {
        let rarity = self.roll_rarity(magic_find);
        let mult = rarity.base_value_mult();
        let base_sell = (level as f32 * 10.0 * mult) as u32;
        let stats = self.generate_stats(level, &rarity);
        GeneratedItem { base_item_id: item_id, rarity, level, stats, sell_value: base_sell, enchantments: Vec::new(), sockets: 0 }
    }

    fn roll_rarity(&mut self, magic_find: f32) -> ItemRarity {
        let r = self.rng.next_f32() / (1.0 + magic_find * 0.01);
        if r < 0.002 { ItemRarity::Mythic }
        else if r < 0.01 { ItemRarity::Legendary }
        else if r < 0.05 { ItemRarity::Epic }
        else if r < 0.15 { ItemRarity::Rare }
        else if r < 0.35 { ItemRarity::Uncommon }
        else { ItemRarity::Common }
    }

    fn generate_stats(&mut self, level: u32, rarity: &ItemRarity) -> ItemStats {
        let base = level as f32 * rarity.base_value_mult().sqrt();
        let vary = || base * (0.9 + self.rng.next_f32() * 0.2);
        ItemStats { attack: vary(), defense: vary(), speed: vary() * 0.1, magic: vary(), hp: vary() * 5.0, mp: vary() * 2.0, crit_chance: self.rng.next_f32() * 0.1, crit_damage: 1.5 + self.rng.next_f32() * 0.5 }
    }
}

// ============================================================
// SECTION: ECONOMY SIMULATOR
// ============================================================

pub struct EconomyParams {
    pub money_supply: f64,
    pub velocity: f64,
    pub price_level: f64,
    pub output: f64,
}

pub struct EconomySimParams {
    pub initial_money_supply: f64,
    pub velocity: f64,
    pub output_growth_rate: f64,
    pub inflation_target: f64,
}

impl Default for EconomySimParams {
    fn default() -> Self {
        Self { initial_money_supply: 1_000_000.0, velocity: 5.0, output_growth_rate: 0.02, inflation_target: 0.02 }
    }
}

pub struct EconomySimulator {
    pub params: EconomySimParams,
    pub current_money_supply: f64,
    pub current_price_level: f64,
    pub current_output: f64,
    pub tick: u32,
    pub history: Vec<EconomyParams>,
}

impl EconomySimulator {
    pub fn new(params: EconomySimParams) -> Self {
        let ms = params.initial_money_supply;
        let v = params.velocity;
        let out = ms * v / 1.0;
        Self { params, current_money_supply: ms, current_price_level: 1.0, current_output: out, tick: 0, history: Vec::new() }
    }

    pub fn step(&mut self, money_injection: f64) {
        self.current_money_supply += money_injection;
        self.current_output *= 1.0 + self.params.output_growth_rate / 12.0;
        self.current_price_level = (self.current_money_supply * self.params.velocity) / self.current_output;
        self.history.push(EconomyParams {
            money_supply: self.current_money_supply,
            velocity: self.params.velocity,
            price_level: self.current_price_level,
            output: self.current_output,
        });
        self.tick += 1;
    }

    pub fn inflation_rate(&self) -> f64 {
        if self.history.len() < 2 { return 0.0; }
        let n = self.history.len();
        (self.history[n-1].price_level / self.history[n-2].price_level) - 1.0
    }
}

// ============================================================
// SECTION: PRESTIGE SYSTEM
// ============================================================

pub struct PrestigeLevel {
    pub level: u32,
    pub bonus_magic_find: f32,
    pub bonus_gold: f32,
    pub bonus_xp: f32,
    pub title: &'static str,
}

pub struct PrestigeSystem {
    pub current_prestige: u32,
    pub total_xp: u64,
    pub levels: Vec<PrestigeLevel>,
}

impl PrestigeSystem {
    pub fn new() -> Self {
        let levels = vec![
            PrestigeLevel { level: 0, bonus_magic_find: 0.0, bonus_gold: 0.0, bonus_xp: 0.0, title: "Novice" },
            PrestigeLevel { level: 1, bonus_magic_find: 0.05, bonus_gold: 0.05, bonus_xp: 0.1, title: "Apprentice" },
            PrestigeLevel { level: 2, bonus_magic_find: 0.10, bonus_gold: 0.10, bonus_xp: 0.2, title: "Journeyman" },
            PrestigeLevel { level: 3, bonus_magic_find: 0.20, bonus_gold: 0.20, bonus_xp: 0.35, title: "Expert" },
            PrestigeLevel { level: 4, bonus_magic_find: 0.35, bonus_gold: 0.35, bonus_xp: 0.5, title: "Master" },
            PrestigeLevel { level: 5, bonus_magic_find: 0.5, bonus_gold: 0.5, bonus_xp: 0.75, title: "Grandmaster" },
        ];
        Self { current_prestige: 0, total_xp: 0, levels }
    }

    pub fn add_xp(&mut self, xp: u64) {
        self.total_xp += xp;
    }

    pub fn current_level(&self) -> &PrestigeLevel {
        let idx = self.current_prestige.min(self.levels.len() as u32 - 1) as usize;
        &self.levels[idx]
    }

    pub fn prestige(&mut self) -> bool {
        if self.current_prestige < self.levels.len() as u32 - 1 {
            self.current_prestige += 1;
            self.total_xp = 0;
            true
        } else { false }
    }
}

// ============================================================
// SECTION: ITEM EFFECT SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub enum EffectTrigger {
    OnEquip, OnUnequip, OnHit, OnKill, OnTakeDamage, OnHeal, OnLevelUp,
}

#[derive(Clone, Debug)]
pub struct ItemEffect {
    pub id: u32,
    pub name: String,
    pub trigger: EffectTrigger,
    pub effect_type: String,
    pub value: f32,
    pub duration: f32,
    pub cooldown: f32,
}

impl ItemEffect {
    pub fn new(id: u32, name: &str, trigger: EffectTrigger, effect_type: &str, value: f32) -> Self {
        Self { id, name: name.to_string(), trigger, effect_type: effect_type.to_string(), value, duration: 0.0, cooldown: 0.0 }
    }

    pub fn with_duration(mut self, d: f32) -> Self { self.duration = d; self }
    pub fn with_cooldown(mut self, c: f32) -> Self { self.cooldown = c; self }
}

// ============================================================
// SECTION: LOOT MODIFIERS (TRAIT-BASED PIPELINE)
// ============================================================

pub trait LootModifier {
    fn apply(&self, rolls: &mut Vec<DropResult>, rng: &mut LootRng);
    fn name(&self) -> &str;
    fn priority(&self) -> i32 { 0 }
}

pub struct MagicFindModifier {
    pub bonus_pct: f32,
}

impl LootModifier for MagicFindModifier {
    fn apply(&self, rolls: &mut Vec<DropResult>, _rng: &mut LootRng) {
        // Increases drop count by a small amount
        let extra = (rolls.len() as f32 * self.bonus_pct) as usize;
        for i in 0..extra.min(rolls.len()) {
            rolls[i].count += 1;
        }
    }
    fn name(&self) -> &str { "MagicFind" }
    fn priority(&self) -> i32 { 10 }
}

pub struct BossKillModifier {
    pub extra_roll_chance: f32,
}

impl LootModifier for BossKillModifier {
    fn apply(&self, rolls: &mut Vec<DropResult>, rng: &mut LootRng) {
        if rng.next_f32() < self.extra_roll_chance {
            if let Some(first) = rolls.first().cloned() {
                rolls.push(DropResult { item_id: first.item_id + 1, count: 1, is_guaranteed: false, from_pity: false });
            }
        }
    }
    fn name(&self) -> &str { "BossKill" }
    fn priority(&self) -> i32 { 5 }
}

pub struct LootPipeline {
    pub modifiers: Vec<Box<dyn LootModifier>>,
    pub rng: LootRng,
}

impl LootPipeline {
    pub fn new(seed: u64) -> Self { Self { modifiers: Vec::new(), rng: LootRng::new(seed) } }

    pub fn add_modifier(&mut self, m: Box<dyn LootModifier>) { self.modifiers.push(m); }

    pub fn process(&mut self, mut rolls: Vec<DropResult>) -> Vec<DropResult> {
        for modifier in &self.modifiers {
            modifier.apply(&mut rolls, &mut self.rng);
        }
        rolls
    }
}

// ============================================================
// SECTION: EXTENDED ITEM CATALOG (100+ items)
// ============================================================

pub fn build_extended_item_catalog() -> Vec<Item> {
    let mut items = build_item_catalog();
    let start_id = items.len() as u32 + 1;
    let mut id = start_id;

    macro_rules! add_item {
        ($name:expr, $ty:expr, $rar:expr, $val:expr) => {{
            items.push(Item::new(id, $name, $ty, $rar, $val));
            id += 1;
        }};
    }

    // Boss exclusive items
    add_item!("Dragon Emperor's Crown", ItemType::Armor, ItemRarity::BossExclusive, 50000.0);
    add_item!("Lich King Scepter", ItemType::Weapon, ItemRarity::BossExclusive, 45000.0);
    add_item!("Phoenix Feather Cloak", ItemType::Armor, ItemRarity::BossExclusive, 40000.0);
    add_item!("Ancient Guardian Shield", ItemType::Armor, ItemRarity::BossExclusive, 38000.0);
    add_item!("Titan's Waraxe", ItemType::Weapon, ItemRarity::BossExclusive, 42000.0);

    // Materials
    add_item!("Iron Ore", ItemType::Material, ItemRarity::Common, 1.0);
    add_item!("Coal", ItemType::Material, ItemRarity::Common, 0.5);
    add_item!("Steel Ingot", ItemType::Material, ItemRarity::Uncommon, 5.0);
    add_item!("Leather", ItemType::Material, ItemRarity::Common, 2.0);
    add_item!("Silk Thread", ItemType::Material, ItemRarity::Uncommon, 8.0);
    add_item!("Dragon Scale", ItemType::Material, ItemRarity::Epic, 200.0);
    add_item!("Phoenix Ash", ItemType::Material, ItemRarity::Legendary, 1000.0);
    add_item!("Herb Bundle", ItemType::Material, ItemRarity::Common, 3.0);
    add_item!("Magic Crystal", ItemType::Material, ItemRarity::Rare, 50.0);
    add_item!("Void Fragment", ItemType::Material, ItemRarity::Epic, 300.0);

    // Consumables
    add_item!("Health Potion", ItemType::Consumable, ItemRarity::Common, 5.0);
    add_item!("Mana Potion", ItemType::Consumable, ItemRarity::Common, 5.0);
    add_item!("Elixir of Speed", ItemType::Consumable, ItemRarity::Uncommon, 25.0);
    add_item!("Scroll of Fireball", ItemType::Consumable, ItemRarity::Rare, 100.0);
    add_item!("Resurrection Stone", ItemType::Consumable, ItemRarity::Epic, 500.0);

    // Accessories
    add_item!("Silver Ring", ItemType::Accessory, ItemRarity::Common, 15.0);
    add_item!("Gold Amulet", ItemType::Accessory, ItemRarity::Uncommon, 80.0);
    add_item!("Sapphire Pendant", ItemType::Accessory, ItemRarity::Rare, 400.0);
    add_item!("Ring of Power", ItemType::Accessory, ItemRarity::Epic, 2000.0);
    add_item!("Amulet of Eternity", ItemType::Accessory, ItemRarity::Legendary, 8000.0);

    // Gems
    add_item!("Ruby", ItemType::Gem, ItemRarity::Uncommon, 100.0);
    add_item!("Sapphire", ItemType::Gem, ItemRarity::Uncommon, 100.0);
    add_item!("Emerald", ItemType::Gem, ItemRarity::Uncommon, 100.0);
    add_item!("Diamond", ItemType::Gem, ItemRarity::Rare, 500.0);
    add_item!("Onyx", ItemType::Gem, ItemRarity::Rare, 400.0);

    // Quest items
    add_item!("Ancient Map Fragment", ItemType::Quest, ItemRarity::Common, 0.0);
    add_item!("Cursed Skull", ItemType::Quest, ItemRarity::Uncommon, 0.0);
    add_item!("Soul Stone", ItemType::Quest, ItemRarity::Rare, 0.0);

    items
}

// ============================================================
// SECTION: LOOT TABLE HIERARCHY
// ============================================================

pub struct LootTableHierarchy {
    pub tables: HashMap<u32, LootTable>,
    pub parent_map: HashMap<u32, u32>,
}

impl LootTableHierarchy {
    pub fn new() -> Self { Self { tables: HashMap::new(), parent_map: HashMap::new() } }

    pub fn add_table(&mut self, table: LootTable, parent_id: Option<u32>) {
        let id = table.id;
        self.tables.insert(id, table);
        if let Some(parent) = parent_id { self.parent_map.insert(id, parent); }
    }

    pub fn get_inherited_entries(&self, table_id: u32) -> Vec<LootTableEntry> {
        let mut entries = Vec::new();
        let mut current_id = table_id;
        let mut visited = HashSet::new();
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
}

// ============================================================
// SECTION: SALVAGE SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct SalvageResult {
    pub item_id: u32,
    pub materials_gained: Vec<(u32, u32)>,
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
        SalvageResult { item_id: item.base_item_id, materials_gained: vec![(self.base_material_id, total_qty)], gold_gained: item.sell_value / 4 }
    }

    pub fn batch_salvage(&mut self, items: &[GeneratedItem]) -> Vec<SalvageResult> {
        items.iter().map(|i| self.salvage(i)).collect()
    }
}

// ============================================================
// SECTION: DROP STREAK TRACKER
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
}

// ============================================================
// SECTION: ITEM AFFINITY SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct AffinityBonus { pub stat: &'static str, pub bonus_pct: f32 }

#[derive(Clone, Debug)]
pub struct ItemAffinity {
    pub affinity_id: u32,
    pub name: &'static str,
    pub required_items: Vec<u32>,
    pub bonuses: Vec<AffinityBonus>,
}

pub struct AffinityRegistry {
    pub affinities: Vec<ItemAffinity>,
}

impl AffinityRegistry {
    pub fn new() -> Self { Self { affinities: Vec::new() } }

    pub fn register(&mut self, affinity: ItemAffinity) { self.affinities.push(affinity); }

    pub fn check_active(&self, equipped: &[u32]) -> Vec<&ItemAffinity> {
        let equipped_set: HashSet<u32> = equipped.iter().copied().collect();
        self.affinities.iter().filter(|a| a.required_items.iter().all(|id| equipped_set.contains(id))).collect()
    }

    pub fn total_bonus(&self, equipped: &[u32], stat: &str) -> f32 {
        self.check_active(equipped).iter().flat_map(|a| a.bonuses.iter()).filter(|b| b.stat == stat).map(|b| b.bonus_pct).sum()
    }
}

// ============================================================
// SECTION: ACHIEVEMENT TRACKER
// ============================================================

#[derive(Clone, Debug)]
pub struct LootAchievement {
    pub id: u32,
    pub name: &'static str,
    pub description: &'static str,
    pub target_count: u32,
    pub current_count: u32,
    pub completed: bool,
    pub reward_item_id: Option<u32>,
}

impl LootAchievement {
    pub fn new(id: u32, name: &'static str, description: &'static str, target: u32, reward: Option<u32>) -> Self {
        Self { id, name, description, target_count: target, current_count: 0, completed: false, reward_item_id: reward }
    }

    pub fn increment(&mut self) -> bool {
        if self.completed { return false; }
        self.current_count += 1;
        if self.current_count >= self.target_count { self.completed = true; return true; }
        false
    }
}

pub struct AchievementTracker {
    pub achievements: Vec<LootAchievement>,
    pub completed_ids: Vec<u32>,
}

impl AchievementTracker {
    pub fn new() -> Self {
        let achievements = vec![
            LootAchievement::new(1, "First Blood", "Get your first drop", 1, Some(9001)),
            LootAchievement::new(2, "Collector", "Collect 100 items", 100, Some(9002)),
            LootAchievement::new(3, "Legendary Hunter", "Obtain 10 legendary items", 10, Some(9003)),
            LootAchievement::new(4, "Crafter", "Craft 50 items", 50, Some(9004)),
            LootAchievement::new(5, "Boss Slayer", "Kill 100 bosses", 100, Some(9005)),
        ];
        Self { achievements, completed_ids: Vec::new() }
    }

    pub fn record_event(&mut self, _event: &str, amount: u32) -> Vec<u32> {
        let mut newly_completed = Vec::new();
        for ach in &mut self.achievements {
            for _ in 0..amount {
                if ach.increment() { newly_completed.push(ach.id); }
            }
        }
        self.completed_ids.extend_from_slice(&newly_completed);
        newly_completed
    }

    pub fn completion_rate(&self) -> f32 {
        let done = self.achievements.iter().filter(|a| a.completed).count();
        done as f32 / self.achievements.len() as f32
    }
}

// ============================================================
// SECTION: VALIDATION AND VERSION
// ============================================================

pub fn validate_loot_system() -> bool {
    let catalog = build_extended_item_catalog();
    assert!(catalog.len() >= 100, "Catalog too small");
    let mut crafting = CraftingSystem::new();
    crafting.build_standard_recipes();
    assert!(!crafting.recipes.is_empty(), "No crafting recipes");
    let mut enchants = EnchantmentLibrary::new();
    enchants.build_standard_library();
    assert!(!enchants.enchantments.is_empty(), "No enchantments");
    let mut gen = ItemGenerator::new(1);
    let item = gen.generate_item(100, "Test", 50, 100.0);
    assert!(item.sell_value > 0 || item.rarity == ItemRarity::Common);
    true
}

pub fn loot_editor_version() -> &'static str { "LootEditor v2.0 - Full System" }
pub fn loot_editor_full_version() -> &'static str { "LootEditor v2.1 - Complete" }

#[test]
fn test_loot_system_validation() { assert!(validate_loot_system()); }

#[test]
fn test_alias_table() {
    let weights = vec![1.0, 2.0, 3.0, 4.0];
    let table = AliasTable::build(&weights);
    let mut rng = LootRng::new(42);
    let sample = table.sample(&mut rng);
    assert!(sample < 4);
}

#[test]
fn test_pity_system() {
    let mut pity = PitySystem::new(0.01, 50, 100);
    let mut rng = LootRng::new(42);
    pity.current_rolls = 99;
    let result = pity.roll(&mut rng);
    assert!(result); // Should always be true at hard pity
}

#[test]
fn test_monte_carlo() {
    let catalog = build_item_catalog();
    let mut table = LootTable::new(1, "test");
    table.add_entry(catalog[0].id, 50.0, 1, 1);
    table.add_entry(catalog[1].id, 30.0, 1, 1);
    let result = run_monte_carlo(&table, &catalog, 100, 42);
    assert_eq!(result.runs, 100);
}

#[test]
fn test_item_generator() {
    let mut gen = ItemGenerator::new(12345);
    let item = gen.generate_item(1, "Test Sword", 30, 50.0);
    assert!(item.sell_value > 0);
}

#[test]
fn test_economy_simulator() {
    let mut sim = EconomySimulator::new(EconomySimParams::default());
    for _ in 0..12 { sim.step(1000.0); }
    assert!(sim.current_price_level > 0.0);
}

#[test]
fn test_prestige_system() {
    let mut ps = PrestigeSystem::new();
    ps.add_xp(1000);
    let result = ps.prestige();
    assert!(result);
    assert_eq!(ps.current_prestige, 1);
}

#[test]
fn test_crafting_system() {
    let mut cs = CraftingSystem::new();
    cs.build_standard_recipes();
    let mut rng = LootRng::new(42);
    assert!(!cs.recipes.is_empty());
}

#[test]
fn test_enchantment_library() {
    let mut lib = EnchantmentLibrary::new();
    lib.build_standard_library();
    let weapons = lib.for_target(&EnchantmentTarget::Weapon);
    assert!(!weapons.is_empty());
}
