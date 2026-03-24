// crafting/recipes.rs — Recipe and crafting formula system

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Quality tier
// ---------------------------------------------------------------------------

/// Tiered quality levels for crafted items, with stat multipliers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QualityTier {
    Poor,
    Common,
    Fine,
    Exceptional,
    Masterwork,
    Legendary,
}

impl QualityTier {
    /// Returns the stat multiplier for this quality tier.
    pub fn stat_multiplier(&self) -> f32 {
        match self {
            QualityTier::Poor        => 0.60,
            QualityTier::Common      => 1.00,
            QualityTier::Fine        => 1.20,
            QualityTier::Exceptional => 1.50,
            QualityTier::Masterwork  => 2.00,
            QualityTier::Legendary   => 3.00,
        }
    }

    /// Convert a raw quality byte (0–255) to a tier.
    pub fn from_value(v: u8) -> Self {
        match v {
            0..=39   => QualityTier::Poor,
            40..=99  => QualityTier::Common,
            100..=149 => QualityTier::Fine,
            150..=199 => QualityTier::Exceptional,
            200..=239 => QualityTier::Masterwork,
            240..=255 => QualityTier::Legendary,
        }
    }

    /// The minimum raw quality value that maps to this tier.
    pub fn threshold(&self) -> u8 {
        match self {
            QualityTier::Poor        => 0,
            QualityTier::Common      => 40,
            QualityTier::Fine        => 100,
            QualityTier::Exceptional => 150,
            QualityTier::Masterwork  => 200,
            QualityTier::Legendary   => 240,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            QualityTier::Poor        => "Poor",
            QualityTier::Common      => "Common",
            QualityTier::Fine        => "Fine",
            QualityTier::Exceptional => "Exceptional",
            QualityTier::Masterwork  => "Masterwork",
            QualityTier::Legendary   => "Legendary",
        }
    }
}

// ---------------------------------------------------------------------------
// Recipe category
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RecipeCategory {
    Smithing,
    Alchemy,
    Cooking,
    Enchanting,
    Engineering,
    Tailoring,
    Woodworking,
    Jeweling,
}

impl RecipeCategory {
    pub fn label(&self) -> &'static str {
        match self {
            RecipeCategory::Smithing     => "Smithing",
            RecipeCategory::Alchemy      => "Alchemy",
            RecipeCategory::Cooking      => "Cooking",
            RecipeCategory::Enchanting   => "Enchanting",
            RecipeCategory::Engineering  => "Engineering",
            RecipeCategory::Tailoring    => "Tailoring",
            RecipeCategory::Woodworking  => "Woodworking",
            RecipeCategory::Jeweling     => "Jeweling",
        }
    }
}

// ---------------------------------------------------------------------------
// Ingredient and CraftResult
// ---------------------------------------------------------------------------

/// A single ingredient required for a recipe.
#[derive(Debug, Clone)]
pub struct Ingredient {
    pub item_id: String,
    pub quantity: u32,
    /// Minimum quality byte (0–255) the ingredient must have.
    pub quality_min: u8,
}

impl Ingredient {
    pub fn new(item_id: impl Into<String>, quantity: u32, quality_min: u8) -> Self {
        Self { item_id: item_id.into(), quantity, quality_min }
    }

    /// Simple helper — quality_min = 0.
    pub fn basic(item_id: impl Into<String>, quantity: u32) -> Self {
        Self::new(item_id, quantity, 0)
    }
}

/// One possible output from a crafting operation.
#[derive(Debug, Clone)]
pub struct CraftResult {
    pub item_id: String,
    pub quantity: u32,
    /// Base quality byte before skill adjustments.
    pub quality: u8,
    /// Probability (0.0–1.0) that this result is produced.
    pub chance: f32,
}

impl CraftResult {
    pub fn new(item_id: impl Into<String>, quantity: u32, quality: u8, chance: f32) -> Self {
        Self { item_id: item_id.into(), quantity, quality, chance: chance.clamp(0.0, 1.0) }
    }

    /// Guaranteed (chance = 1.0) result.
    pub fn guaranteed(item_id: impl Into<String>, quantity: u32, quality: u8) -> Self {
        Self::new(item_id, quantity, quality, 1.0)
    }
}

// ---------------------------------------------------------------------------
// Recipe
// ---------------------------------------------------------------------------

/// A complete crafting recipe.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub category: RecipeCategory,
    pub ingredients: Vec<Ingredient>,
    pub results: Vec<CraftResult>,
    /// Minimum crafting skill level required.
    pub required_level: u32,
    /// Tool item IDs required to be at the workbench.
    pub required_tools: Vec<String>,
    /// Base crafting time in seconds.
    pub duration_secs: f32,
    /// Experience awarded on completion.
    pub experience_reward: u32,
    /// Probability (0.0–1.0) that the player learns this recipe upon discovery attempt.
    pub discovery_chance: f32,
}

impl Recipe {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        category: RecipeCategory,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category,
            ingredients: Vec::new(),
            results: Vec::new(),
            required_level: 1,
            required_tools: Vec::new(),
            duration_secs: 5.0,
            experience_reward: 10,
            discovery_chance: 0.0,
        }
    }

    pub fn with_ingredient(mut self, ing: Ingredient) -> Self {
        self.ingredients.push(ing);
        self
    }

    pub fn with_result(mut self, res: CraftResult) -> Self {
        self.results.push(res);
        self
    }

    pub fn with_level(mut self, level: u32) -> Self {
        self.required_level = level;
        self
    }

    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.required_tools.push(tool.into());
        self
    }

    pub fn with_duration(mut self, secs: f32) -> Self {
        self.duration_secs = secs;
        self
    }

    pub fn with_experience(mut self, xp: u32) -> Self {
        self.experience_reward = xp;
        self
    }

    pub fn with_discovery_chance(mut self, chance: f32) -> Self {
        self.discovery_chance = chance.clamp(0.0, 1.0);
        self
    }
}

// ---------------------------------------------------------------------------
// RecipeBook
// ---------------------------------------------------------------------------

/// Central registry of all known recipes.
#[derive(Debug, Clone)]
pub struct RecipeBook {
    recipes: HashMap<String, Recipe>,
    by_category: HashMap<String, Vec<String>>,
}

impl RecipeBook {
    pub fn new() -> Self {
        Self {
            recipes: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Register a recipe in the book.
    pub fn register(&mut self, recipe: Recipe) {
        let cat_key = recipe.category.label().to_string();
        self.by_category
            .entry(cat_key)
            .or_insert_with(Vec::new)
            .push(recipe.id.clone());
        self.recipes.insert(recipe.id.clone(), recipe);
    }

    /// Look up a recipe by id.
    pub fn get(&self, id: &str) -> Option<&Recipe> {
        self.recipes.get(id)
    }

    /// All recipes in a given category.
    pub fn by_category(&self, category: &RecipeCategory) -> Vec<&Recipe> {
        let key = category.label();
        match self.by_category.get(key) {
            None => Vec::new(),
            Some(ids) => ids
                .iter()
                .filter_map(|id| self.recipes.get(id))
                .collect(),
        }
    }

    /// Recipes available to a player of `skill_level`.
    pub fn available_for_level(&self, skill_level: u32) -> Vec<&Recipe> {
        self.recipes
            .values()
            .filter(|r| r.required_level <= skill_level)
            .collect()
    }

    /// Recipes available to a player of `skill_level` in a given category.
    pub fn available_for_level_and_category(
        &self,
        skill_level: u32,
        category: &RecipeCategory,
    ) -> Vec<&Recipe> {
        self.by_category(category)
            .into_iter()
            .filter(|r| r.required_level <= skill_level)
            .collect()
    }

    /// Total number of registered recipes.
    pub fn count(&self) -> usize {
        self.recipes.len()
    }

    /// All recipe ids.
    pub fn ids(&self) -> Vec<&str> {
        self.recipes.keys().map(|s| s.as_str()).collect()
    }

    /// Build a RecipeBook pre-populated with ~30 default recipes across all categories.
    pub fn default_recipes() -> Self {
        let mut book = Self::new();

        // --- SMITHING ---
        book.register(
            Recipe::new("iron_sword", "Iron Sword", RecipeCategory::Smithing)
                .with_ingredient(Ingredient::basic("iron_ingot", 3))
                .with_ingredient(Ingredient::basic("leather_strip", 1))
                .with_result(CraftResult::guaranteed("iron_sword", 1, 80))
                .with_level(5)
                .with_tool("forge_hammer")
                .with_duration(12.0)
                .with_experience(25),
        );
        book.register(
            Recipe::new("iron_shield", "Iron Shield", RecipeCategory::Smithing)
                .with_ingredient(Ingredient::basic("iron_ingot", 4))
                .with_ingredient(Ingredient::basic("wooden_plank", 2))
                .with_result(CraftResult::guaranteed("iron_shield", 1, 75))
                .with_level(8)
                .with_tool("forge_hammer")
                .with_duration(18.0)
                .with_experience(30),
        );
        book.register(
            Recipe::new("steel_ingot", "Steel Ingot", RecipeCategory::Smithing)
                .with_ingredient(Ingredient::basic("iron_ingot", 2))
                .with_ingredient(Ingredient::basic("coal", 3))
                .with_result(CraftResult::guaranteed("steel_ingot", 1, 90))
                .with_level(15)
                .with_duration(8.0)
                .with_experience(15),
        );
        book.register(
            Recipe::new("steel_sword", "Steel Sword", RecipeCategory::Smithing)
                .with_ingredient(Ingredient::new("steel_ingot", 3, 80))
                .with_ingredient(Ingredient::basic("leather_strip", 1))
                .with_result(CraftResult::guaranteed("steel_sword", 1, 110))
                .with_level(20)
                .with_tool("forge_hammer")
                .with_duration(20.0)
                .with_experience(50),
        );
        book.register(
            Recipe::new("iron_helmet", "Iron Helmet", RecipeCategory::Smithing)
                .with_ingredient(Ingredient::basic("iron_ingot", 5))
                .with_result(CraftResult::guaranteed("iron_helmet", 1, 75))
                .with_level(10)
                .with_tool("forge_hammer")
                .with_duration(15.0)
                .with_experience(35),
        );

        // --- ALCHEMY ---
        book.register(
            Recipe::new("health_potion_minor", "Minor Health Potion", RecipeCategory::Alchemy)
                .with_ingredient(Ingredient::basic("red_herb", 2))
                .with_ingredient(Ingredient::basic("clean_water", 1))
                .with_result(CraftResult::guaranteed("health_potion_minor", 1, 70))
                .with_level(1)
                .with_duration(5.0)
                .with_experience(8)
                .with_discovery_chance(0.3),
        );
        book.register(
            Recipe::new("health_potion_major", "Major Health Potion", RecipeCategory::Alchemy)
                .with_ingredient(Ingredient::new("red_herb", 4, 60))
                .with_ingredient(Ingredient::basic("clean_water", 1))
                .with_ingredient(Ingredient::basic("golden_root", 1))
                .with_result(CraftResult::guaranteed("health_potion_major", 1, 100))
                .with_level(12)
                .with_duration(10.0)
                .with_experience(20),
        );
        book.register(
            Recipe::new("mana_potion", "Mana Potion", RecipeCategory::Alchemy)
                .with_ingredient(Ingredient::basic("blue_herb", 2))
                .with_ingredient(Ingredient::basic("spring_water", 1))
                .with_result(CraftResult::guaranteed("mana_potion", 1, 70))
                .with_level(5)
                .with_duration(6.0)
                .with_experience(12),
        );
        book.register(
            Recipe::new("poison_vial", "Poison Vial", RecipeCategory::Alchemy)
                .with_ingredient(Ingredient::basic("viper_fang", 1))
                .with_ingredient(Ingredient::basic("dark_mushroom", 2))
                .with_result(CraftResult::guaranteed("poison_vial", 2, 80))
                .with_result(CraftResult::new("potent_poison_vial", 1, 120, 0.15))
                .with_level(18)
                .with_duration(14.0)
                .with_experience(40)
                .with_discovery_chance(0.1),
        );

        // --- COOKING ---
        book.register(
            Recipe::new("roasted_meat", "Roasted Meat", RecipeCategory::Cooking)
                .with_ingredient(Ingredient::basic("raw_meat", 2))
                .with_ingredient(Ingredient::basic("salt", 1))
                .with_result(CraftResult::guaranteed("roasted_meat", 2, 60))
                .with_level(1)
                .with_duration(4.0)
                .with_experience(5),
        );
        book.register(
            Recipe::new("hearty_stew", "Hearty Stew", RecipeCategory::Cooking)
                .with_ingredient(Ingredient::basic("raw_meat", 1))
                .with_ingredient(Ingredient::basic("carrot", 2))
                .with_ingredient(Ingredient::basic("potato", 2))
                .with_ingredient(Ingredient::basic("salt", 1))
                .with_result(CraftResult::guaranteed("hearty_stew", 1, 80))
                .with_level(8)
                .with_duration(10.0)
                .with_experience(18),
        );
        book.register(
            Recipe::new("energy_bread", "Energy Bread", RecipeCategory::Cooking)
                .with_ingredient(Ingredient::basic("flour", 3))
                .with_ingredient(Ingredient::basic("honey", 1))
                .with_result(CraftResult::guaranteed("energy_bread", 2, 65))
                .with_level(4)
                .with_duration(6.0)
                .with_experience(10),
        );

        // --- ENCHANTING ---
        book.register(
            Recipe::new("enchant_sharpness_i", "Sharpness I Enchant", RecipeCategory::Enchanting)
                .with_ingredient(Ingredient::new("iron_sword", 1, 70))
                .with_ingredient(Ingredient::basic("magic_dust", 5))
                .with_result(CraftResult::guaranteed("iron_sword_sharpened", 1, 90))
                .with_level(10)
                .with_tool("enchanting_focus")
                .with_duration(30.0)
                .with_experience(60),
        );
        book.register(
            Recipe::new("enchant_fire_i", "Fire I Enchant", RecipeCategory::Enchanting)
                .with_ingredient(Ingredient::new("steel_sword", 1, 90))
                .with_ingredient(Ingredient::basic("fire_essence", 3))
                .with_ingredient(Ingredient::basic("magic_dust", 8))
                .with_result(CraftResult::guaranteed("fire_steel_sword", 1, 120))
                .with_result(CraftResult::new("ember_steel_sword", 1, 140, 0.10))
                .with_level(25)
                .with_tool("enchanting_focus")
                .with_duration(45.0)
                .with_experience(100),
        );
        book.register(
            Recipe::new("enchant_ward_armor", "Ward Armor Enchant", RecipeCategory::Enchanting)
                .with_ingredient(Ingredient::basic("iron_helmet", 1))
                .with_ingredient(Ingredient::basic("light_rune", 2))
                .with_ingredient(Ingredient::basic("magic_dust", 6))
                .with_result(CraftResult::guaranteed("warded_iron_helmet", 1, 95))
                .with_level(20)
                .with_tool("enchanting_focus")
                .with_duration(40.0)
                .with_experience(80),
        );

        // --- ENGINEERING ---
        book.register(
            Recipe::new("gear_small", "Small Gear", RecipeCategory::Engineering)
                .with_ingredient(Ingredient::basic("iron_ingot", 1))
                .with_result(CraftResult::guaranteed("gear_small", 3, 70))
                .with_level(3)
                .with_tool("wrench")
                .with_duration(4.0)
                .with_experience(8),
        );
        book.register(
            Recipe::new("clockwork_device", "Clockwork Device", RecipeCategory::Engineering)
                .with_ingredient(Ingredient::basic("gear_small", 6))
                .with_ingredient(Ingredient::basic("copper_wire", 4))
                .with_ingredient(Ingredient::basic("spring_coil", 2))
                .with_result(CraftResult::guaranteed("clockwork_device", 1, 85))
                .with_level(20)
                .with_tool("wrench")
                .with_duration(25.0)
                .with_experience(55),
        );
        book.register(
            Recipe::new("bomb_smoke", "Smoke Bomb", RecipeCategory::Engineering)
                .with_ingredient(Ingredient::basic("charcoal_powder", 2))
                .with_ingredient(Ingredient::basic("sulfur", 1))
                .with_ingredient(Ingredient::basic("cloth_scrap", 1))
                .with_result(CraftResult::guaranteed("bomb_smoke", 2, 70))
                .with_level(15)
                .with_duration(8.0)
                .with_experience(22),
        );

        // --- TAILORING ---
        book.register(
            Recipe::new("cloth_tunic", "Cloth Tunic", RecipeCategory::Tailoring)
                .with_ingredient(Ingredient::basic("cloth_bolt", 3))
                .with_ingredient(Ingredient::basic("thread_spool", 2))
                .with_result(CraftResult::guaranteed("cloth_tunic", 1, 60))
                .with_level(1)
                .with_tool("sewing_needle")
                .with_duration(8.0)
                .with_experience(10),
        );
        book.register(
            Recipe::new("leather_armor", "Leather Armor", RecipeCategory::Tailoring)
                .with_ingredient(Ingredient::basic("tanned_hide", 4))
                .with_ingredient(Ingredient::basic("thread_spool", 2))
                .with_ingredient(Ingredient::basic("iron_buckle", 2))
                .with_result(CraftResult::guaranteed("leather_armor", 1, 75))
                .with_level(10)
                .with_tool("sewing_needle")
                .with_duration(15.0)
                .with_experience(30),
        );
        book.register(
            Recipe::new("silk_robe", "Silk Robe", RecipeCategory::Tailoring)
                .with_ingredient(Ingredient::new("silk_bolt", 5, 80))
                .with_ingredient(Ingredient::basic("thread_spool", 3))
                .with_ingredient(Ingredient::basic("gem_dust", 1))
                .with_result(CraftResult::guaranteed("silk_robe", 1, 100))
                .with_level(22)
                .with_tool("sewing_needle")
                .with_duration(22.0)
                .with_experience(55),
        );

        // --- WOODWORKING ---
        book.register(
            Recipe::new("wooden_bow", "Wooden Bow", RecipeCategory::Woodworking)
                .with_ingredient(Ingredient::basic("flexible_branch", 2))
                .with_ingredient(Ingredient::basic("sinew_string", 1))
                .with_result(CraftResult::guaranteed("wooden_bow", 1, 65))
                .with_level(5)
                .with_tool("carving_knife")
                .with_duration(10.0)
                .with_experience(18),
        );
        book.register(
            Recipe::new("wooden_staff", "Wooden Staff", RecipeCategory::Woodworking)
                .with_ingredient(Ingredient::basic("hardwood_log", 1))
                .with_result(CraftResult::guaranteed("wooden_staff", 1, 70))
                .with_level(3)
                .with_tool("carving_knife")
                .with_duration(7.0)
                .with_experience(12),
        );
        book.register(
            Recipe::new("arrow_bundle", "Arrow Bundle", RecipeCategory::Woodworking)
                .with_ingredient(Ingredient::basic("straight_stick", 10))
                .with_ingredient(Ingredient::basic("feather", 10))
                .with_ingredient(Ingredient::basic("iron_tip", 10))
                .with_result(CraftResult::guaranteed("arrow", 10, 60))
                .with_level(2)
                .with_duration(12.0)
                .with_experience(14),
        );

        // --- JEWELING ---
        book.register(
            Recipe::new("silver_ring", "Silver Ring", RecipeCategory::Jeweling)
                .with_ingredient(Ingredient::basic("silver_ingot", 1))
                .with_result(CraftResult::guaranteed("silver_ring", 1, 70))
                .with_level(5)
                .with_tool("jeweler_loupe")
                .with_duration(10.0)
                .with_experience(20),
        );
        book.register(
            Recipe::new("ruby_necklace", "Ruby Necklace", RecipeCategory::Jeweling)
                .with_ingredient(Ingredient::basic("gold_chain", 1))
                .with_ingredient(Ingredient::new("ruby_gem", 1, 100))
                .with_result(CraftResult::guaranteed("ruby_necklace", 1, 120))
                .with_result(CraftResult::new("radiant_ruby_necklace", 1, 180, 0.08))
                .with_level(20)
                .with_tool("jeweler_loupe")
                .with_duration(25.0)
                .with_experience(70),
        );
        book.register(
            Recipe::new("sapphire_amulet", "Sapphire Amulet of Clarity", RecipeCategory::Jeweling)
                .with_ingredient(Ingredient::basic("mithril_chain", 1))
                .with_ingredient(Ingredient::new("sapphire_gem", 2, 110))
                .with_ingredient(Ingredient::basic("magic_dust", 4))
                .with_result(CraftResult::guaranteed("sapphire_amulet", 1, 140))
                .with_level(30)
                .with_tool("jeweler_loupe")
                .with_duration(35.0)
                .with_experience(110),
        );

        book
    }

    /// Attempt to find a recipe matching these ingredients exactly (for discovery).
    pub fn find_by_ingredients(&self, ingredient_ids: &[String]) -> Option<&Recipe> {
        let mut sorted_input = ingredient_ids.to_vec();
        sorted_input.sort();
        for recipe in self.recipes.values() {
            let mut sorted_recipe: Vec<String> =
                recipe.ingredients.iter().map(|i| i.item_id.clone()).collect();
            sorted_recipe.sort();
            if sorted_recipe == sorted_input {
                return Some(recipe);
            }
        }
        None
    }
}

impl Default for RecipeBook {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CraftingCalculator
// ---------------------------------------------------------------------------

/// Computes actual output values given player stats.
#[derive(Debug, Clone)]
pub struct CraftingCalculator;

impl CraftingCalculator {
    /// Compute the actual output quality (0–255) from base quality, skill, and tool bonus.
    ///
    /// Formula: clamp(base + skill_bonus + tool_bonus, 0, 255)
    /// where skill_bonus = skill_level * 0.4 (capped at +80)
    pub fn calculate_quality(base: u8, skill_level: u32, tool_bonus: u32) -> u8 {
        let skill_bonus = ((skill_level as f32 * 0.4) as u32).min(80);
        let raw = base as u32 + skill_bonus + tool_bonus;
        raw.min(255) as u8
    }

    /// Compute the actual output quantity from base quantity, skill level, and luck.
    ///
    /// Uses linear interpolation `a + t * (b - a)` where a = base, b = base * 2,
    /// t is derived from skill_level and luck.
    pub fn calculate_quantity(base: u32, skill_level: u32, luck: f32) -> u32 {
        // t ranges from 0 to 1 as skill_level goes from 0 to 100, modified by luck
        let t = ((skill_level as f32 / 100.0) + luck * 0.2).clamp(0.0, 1.0);
        let a = base as f32;
        let b = (base * 2) as f32;
        // a + t * (b - a) — explicit form, no lerp
        let result = a + t * (b - a);
        result.round() as u32
    }

    /// Compute the actual crafting duration (seconds) from base duration, skill, and tool speed.
    ///
    /// Higher skill and tool speed reduce the duration (capped at 10% of base).
    pub fn calculate_duration(base: f32, skill_level: u32, tool_speed: f32) -> f32 {
        let skill_reduction = (skill_level as f32 * 0.005).min(0.5);
        let tool_reduction = tool_speed.clamp(0.0, 0.4);
        let total_reduction = (skill_reduction + tool_reduction).min(0.9);
        let t = 1.0 - total_reduction;
        // a + t * (b - a) where a = base, b = base * 0.1  →  base * t
        // Simplified: final = base * t, but spelled in lerp form for clarity
        let a = base;
        let b = base * 0.1;
        let result = a + (1.0 - t) * (b - a);  // t here means "how reduced"
        result.max(0.5)
    }

    /// Evaluate whether a probabilistic CraftResult fires given skill and rng value (0..1).
    pub fn evaluate_chance(result: &CraftResult, skill_level: u32, rng: f32) -> bool {
        let bonus = (skill_level as f32 * 0.001).min(0.1);
        let effective_chance = (result.chance + bonus).min(1.0);
        rng <= effective_chance
    }

    /// Compute XP gained for completing a recipe, boosted by quality.
    pub fn calculate_experience(base_xp: u32, quality: u8) -> u32 {
        let quality_factor = 1.0 + (quality as f32 - 80.0).max(0.0) * 0.005;
        ((base_xp as f32) * quality_factor).round() as u32
    }
}

// ---------------------------------------------------------------------------
// MasterySystem
// ---------------------------------------------------------------------------

/// Per-category mastery XP and level tracking.
#[derive(Debug, Clone)]
pub struct CategoryMastery {
    pub category: RecipeCategory,
    pub current_xp: u32,
    pub level: u32,
    /// Recipe IDs unlocked through mastery thresholds.
    pub unlocked_recipes: Vec<String>,
}

impl CategoryMastery {
    pub fn new(category: RecipeCategory) -> Self {
        Self {
            category,
            current_xp: 0,
            level: 1,
            unlocked_recipes: Vec::new(),
        }
    }

    /// XP needed to reach the next level.
    pub fn xp_to_next_level(&self) -> u32 {
        (self.level * self.level * 100).max(100)
    }

    /// Add XP, returning true if a level-up occurred.
    pub fn add_xp(&mut self, xp: u32) -> bool {
        self.current_xp += xp;
        let threshold = self.xp_to_next_level();
        if self.current_xp >= threshold {
            self.current_xp -= threshold;
            self.level += 1;
            return true;
        }
        false
    }
}

/// Tracks crafting mastery across all categories and unlocks bonus recipes at thresholds.
#[derive(Debug, Clone)]
pub struct MasterySystem {
    masteries: HashMap<String, CategoryMastery>,
    /// Bonus recipes keyed by (category_label, required_level).
    bonus_recipes: Vec<(String, u32, String)>,
}

impl MasterySystem {
    pub fn new() -> Self {
        let mut s = Self {
            masteries: HashMap::new(),
            bonus_recipes: Vec::new(),
        };
        // Initialize all categories
        for cat in Self::all_categories() {
            let label = cat.label().to_string();
            s.masteries.insert(label, CategoryMastery::new(cat));
        }
        // Register bonus recipe unlock thresholds
        s.bonus_recipes = vec![
            ("Smithing".into(), 10, "iron_sword".into()),
            ("Smithing".into(), 25, "steel_sword".into()),
            ("Alchemy".into(), 5,  "health_potion_major".into()),
            ("Alchemy".into(), 20, "poison_vial".into()),
            ("Enchanting".into(), 15, "enchant_sharpness_i".into()),
            ("Jeweling".into(), 10, "silver_ring".into()),
        ];
        s
    }

    fn all_categories() -> Vec<RecipeCategory> {
        vec![
            RecipeCategory::Smithing,
            RecipeCategory::Alchemy,
            RecipeCategory::Cooking,
            RecipeCategory::Enchanting,
            RecipeCategory::Engineering,
            RecipeCategory::Tailoring,
            RecipeCategory::Woodworking,
            RecipeCategory::Jeweling,
        ]
    }

    /// Award XP to a category, returning any newly unlocked recipe IDs.
    pub fn award_xp(&mut self, category: &RecipeCategory, xp: u32) -> Vec<String> {
        let key = category.label().to_string();
        let mut newly_unlocked = Vec::new();
        if let Some(mastery) = self.masteries.get_mut(&key) {
            let leveled_up = mastery.add_xp(xp);
            if leveled_up {
                let new_level = mastery.level;
                // Check all bonus recipes for this category
                let unlocks: Vec<String> = self.bonus_recipes.iter()
                    .filter(|(cat, thresh, _)| cat == &key && *thresh == new_level)
                    .map(|(_, _, recipe_id)| recipe_id.clone())
                    .collect();
                for recipe_id in &unlocks {
                    if let Some(m) = self.masteries.get_mut(&key) {
                        if !m.unlocked_recipes.contains(recipe_id) {
                            m.unlocked_recipes.push(recipe_id.clone());
                        }
                    }
                }
                newly_unlocked = unlocks;
            }
        }
        newly_unlocked
    }

    /// Get the mastery for a category.
    pub fn get_mastery(&self, category: &RecipeCategory) -> Option<&CategoryMastery> {
        self.masteries.get(category.label())
    }

    /// Get the level for a category.
    pub fn level(&self, category: &RecipeCategory) -> u32 {
        self.masteries
            .get(category.label())
            .map(|m| m.level)
            .unwrap_or(1)
    }

    /// Get all unlocked bonus recipe IDs for a category.
    pub fn unlocked_recipes(&self, category: &RecipeCategory) -> Vec<String> {
        self.masteries
            .get(category.label())
            .map(|m| m.unlocked_recipes.clone())
            .unwrap_or_default()
    }
}

impl Default for MasterySystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RecipeDiscovery — alchemy-style probabilistic discovery
// ---------------------------------------------------------------------------

/// Tracks which ingredient combinations a player has already attempted.
#[derive(Debug, Clone)]
pub struct RecipeDiscovery {
    /// Set of sorted ingredient id lists that have been attempted.
    attempted_combinations: Vec<Vec<String>>,
    /// Recipes the player has already discovered.
    discovered_recipe_ids: Vec<String>,
}

impl RecipeDiscovery {
    pub fn new() -> Self {
        Self {
            attempted_combinations: Vec::new(),
            discovered_recipe_ids: Vec::new(),
        }
    }

    /// Attempt to discover a recipe from a list of ingredient ids.
    ///
    /// Returns `Some(recipe_id)` if the combination matches a discoverable recipe
    /// AND the random roll succeeds.  `rng` should be in [0.0, 1.0).
    pub fn attempt_discovery(
        &mut self,
        ingredient_ids: &[String],
        recipe_book: &RecipeBook,
        rng: f32,
    ) -> Option<String> {
        let mut sorted = ingredient_ids.to_vec();
        sorted.sort();

        // Already attempted this exact combination
        if self.attempted_combinations.contains(&sorted) {
            return None;
        }
        self.attempted_combinations.push(sorted.clone());

        // Look for a matching recipe
        let recipe = recipe_book.find_by_ingredients(ingredient_ids)?;

        // Already discovered
        if self.discovered_recipe_ids.contains(&recipe.id) {
            return None;
        }

        // Recipe must have a positive discovery chance
        if recipe.discovery_chance <= 0.0 {
            return None;
        }

        // Roll the dice
        if rng <= recipe.discovery_chance {
            self.discovered_recipe_ids.push(recipe.id.clone());
            return Some(recipe.id.clone());
        }

        None
    }

    /// Check whether a recipe has been discovered.
    pub fn is_discovered(&self, recipe_id: &str) -> bool {
        self.discovered_recipe_ids.iter().any(|id| id == recipe_id)
    }

    /// Force-add a recipe to discovered (e.g. from a recipe book item).
    pub fn learn_recipe(&mut self, recipe_id: String) {
        if !self.discovered_recipe_ids.contains(&recipe_id) {
            self.discovered_recipe_ids.push(recipe_id);
        }
    }

    /// All discovered recipe IDs.
    pub fn all_discovered(&self) -> &[String] {
        &self.discovered_recipe_ids
    }
}

impl Default for RecipeDiscovery {
    fn default() -> Self {
        Self::new()
    }
}
