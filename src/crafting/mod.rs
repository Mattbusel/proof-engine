// crafting/mod.rs — Crafting and economy system module

//! # Crafting & Economy System
//!
//! A full crafting, workbench, economy, and market module for the Proof Engine.
//!
//! ## Sub-modules
//!
//! - [`recipes`] — Recipe definitions, ingredient/result types, quality tiers,
//!   crafting calculator, mastery system, and recipe discovery.
//! - [`workbench`] — Crafting stations, job queues, fuel systems, and auto-crafters.
//! - [`economy`] — Currency, shops, supply/demand simulation, trade offers, tax.
//! - [`market`] — Auction house, bidding, buyouts, market history, and peer trade windows.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use proof_engine::crafting::{
//!     RecipeBook, Economy, AuctionHouse, Workbench, WorkbenchType, WorkbenchTier,
//!     Currency,
//! };
//! use glam::Vec3;
//!
//! // Load default recipes
//! let book = RecipeBook::default_recipes();
//!
//! // Query smithing recipes available at skill level 10
//! let available = book.available_for_level_and_category(
//!     10,
//!     &proof_engine::crafting::recipes::RecipeCategory::Smithing,
//! );
//!
//! // Create a forge workbench
//! let mut bench = Workbench::new(1, Vec3::ZERO, WorkbenchType::Forge, WorkbenchTier::Basic);
//!
//! // Initialise the global economy
//! let mut economy = Economy::default();
//!
//! // Set up an auction house
//! let mut ah = AuctionHouse::new();
//! ```

pub mod recipes;
pub mod workbench;
pub mod economy;
pub mod market;

// ---------------------------------------------------------------------------
// Flat re-exports — bring the most commonly used types to crafting::*
// ---------------------------------------------------------------------------

// recipes
pub use recipes::{
    Recipe,
    RecipeBook,
    RecipeCategory,
    Ingredient,
    CraftResult,
    CraftingCalculator,
    MasterySystem,
    CategoryMastery,
    RecipeDiscovery,
    QualityTier,
};

// workbench
pub use workbench::{
    Workbench,
    WorkbenchType,
    WorkbenchTier,
    WorkbenchState,
    CraftingQueue,
    CraftingJob,
    WorkbenchEvent,
    FuelType,
    FuelSystem,
    CraftingStation,
    AutoCrafter,
    AutoCraftConfig,
};

// economy
pub use economy::{
    Economy,
    Currency,
    PriceModifier,
    PlayerReputation,
    ShopItem,
    ShopInventory,
    TradeOffer,
    TaxSystem,
};

// market
pub use market::{
    AuctionHouse,
    Listing,
    Bid,
    MarketHistory,
    MarketBoard,
    TradeWindow,
    MailMessage,
};

// ---------------------------------------------------------------------------
// System-level helpers
// ---------------------------------------------------------------------------

/// A single crafting session context bundling all subsystems.
#[derive(Debug, Clone)]
pub struct CraftingContext {
    pub recipe_book: recipes::RecipeBook,
    pub mastery: recipes::MasterySystem,
    pub discovery: recipes::RecipeDiscovery,
    pub economy: economy::Economy,
    pub market: market::MarketBoard,
}

impl CraftingContext {
    /// Create a new context with default recipes and a populated economy.
    pub fn new() -> Self {
        Self {
            recipe_book: recipes::RecipeBook::default_recipes(),
            mastery: recipes::MasterySystem::new(),
            discovery: recipes::RecipeDiscovery::new(),
            economy: economy::Economy::default(),
            market: market::MarketBoard::new(),
        }
    }

    /// Advance all time-dependent systems by `dt` seconds.
    ///
    /// `current_time` is the absolute game time in seconds.
    pub fn tick(&mut self, dt: f32, current_time: f32) {
        self.economy.update_prices(dt);
        self.market.tick(current_time);
    }

    /// Award crafting XP in a category and return any newly unlocked recipe IDs.
    pub fn award_crafting_xp(
        &mut self,
        category: &recipes::RecipeCategory,
        xp: u32,
    ) -> Vec<String> {
        self.mastery.award_xp(category, xp)
    }

    /// Attempt to discover a recipe from a set of ingredient ids.
    ///
    /// `rng` should be a value in [0.0, 1.0).
    pub fn attempt_discovery(
        &mut self,
        ingredient_ids: &[String],
        rng: f32,
    ) -> Option<String> {
        self.discovery.attempt_discovery(ingredient_ids, &self.recipe_book, rng)
    }

    /// Look up a recipe by id.
    pub fn get_recipe(&self, id: &str) -> Option<&recipes::Recipe> {
        self.recipe_book.get(id)
    }

    /// Get the current market price for an item (in copper), or None if not tracked.
    pub fn market_price(&self, item_id: &str) -> Option<u64> {
        self.economy.current_price(item_id)
    }

    /// Get the current market price as a `Currency` value.
    pub fn market_currency(&self, item_id: &str) -> Option<currency::CurrencyAlias> {
        self.economy.current_price_currency(item_id)
    }
}

// Alias to avoid the "currency" sub-module naming clash in CraftingContext::market_currency.
mod currency {
    pub type CurrencyAlias = crate::crafting::economy::Currency;
}

impl Default for CraftingContext {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Crafting result summary (returned to callers after a craft completes)
// ---------------------------------------------------------------------------

/// A resolved crafting outcome with final computed values.
#[derive(Debug, Clone)]
pub struct CraftOutcome {
    pub recipe_id: String,
    /// Produced items: (item_id, quantity, quality_byte).
    pub items_produced: Vec<(String, u32, u8)>,
    /// XP awarded.
    pub experience_gained: u32,
    /// Whether any Legendary quality item was produced.
    pub legendary_proc: bool,
    /// Duration the craft actually took (seconds).
    pub actual_duration: f32,
}

impl CraftOutcome {
    pub fn new(recipe_id: impl Into<String>) -> Self {
        Self {
            recipe_id: recipe_id.into(),
            items_produced: Vec::new(),
            experience_gained: 0,
            legendary_proc: false,
            actual_duration: 0.0,
        }
    }

    pub fn with_item(mut self, item_id: impl Into<String>, quantity: u32, quality: u8) -> Self {
        if quality >= recipes::QualityTier::Legendary.threshold() {
            self.legendary_proc = true;
        }
        self.items_produced.push((item_id.into(), quantity, quality));
        self
    }

    pub fn with_xp(mut self, xp: u32) -> Self {
        self.experience_gained = xp;
        self
    }

    pub fn with_duration(mut self, secs: f32) -> Self {
        self.actual_duration = secs;
        self
    }

    /// Total items produced across all result slots.
    pub fn total_items(&self) -> u32 {
        self.items_produced.iter().map(|(_, qty, _)| qty).sum()
    }

    /// Best quality tier produced.
    pub fn best_quality_tier(&self) -> Option<recipes::QualityTier> {
        self.items_produced
            .iter()
            .map(|(_, _, q)| recipes::QualityTier::from_value(*q))
            .max()
    }
}

// ---------------------------------------------------------------------------
// Crafting error types
// ---------------------------------------------------------------------------

/// Reasons a crafting attempt can fail.
#[derive(Debug, Clone)]
pub enum CraftingError {
    RecipeNotFound { recipe_id: String },
    InsufficientSkill { required: u32, actual: u32 },
    MissingIngredient { item_id: String, required: u32, available: u32 },
    MissingTool { tool_id: String },
    WorkbenchWrongType { expected: WorkbenchType, actual: WorkbenchType },
    WorkbenchBroken { repair_cost: u64 },
    QueueFull,
    InsufficientFuel,
}

impl std::fmt::Display for CraftingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CraftingError::RecipeNotFound { recipe_id } =>
                write!(f, "Recipe not found: {}", recipe_id),
            CraftingError::InsufficientSkill { required, actual } =>
                write!(f, "Skill too low: need {}, have {}", required, actual),
            CraftingError::MissingIngredient { item_id, required, available } =>
                write!(f, "Missing {}: need {}, have {}", item_id, required, available),
            CraftingError::MissingTool { tool_id } =>
                write!(f, "Missing required tool: {}", tool_id),
            CraftingError::WorkbenchWrongType { expected, actual } =>
                write!(f, "Wrong workbench: need {:?}, have {:?}", expected, actual),
            CraftingError::WorkbenchBroken { repair_cost } =>
                write!(f, "Workbench broken (repair cost: {} copper)", repair_cost),
            CraftingError::QueueFull =>
                write!(f, "Crafting queue is full"),
            CraftingError::InsufficientFuel =>
                write!(f, "Workbench has no fuel"),
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience validation helper
// ---------------------------------------------------------------------------

/// Validate whether a crafting attempt can begin given current game state.
///
/// Returns `Ok(())` if all preconditions are met, or a `CraftingError` explaining
/// the first failure.
pub fn validate_craft(
    recipe_id: &str,
    player_skill: u32,
    player_inventory: &std::collections::HashMap<String, u32>,
    player_tools: &[String],
    bench_type: &WorkbenchType,
    recipe_book: &recipes::RecipeBook,
) -> Result<(), CraftingError> {
    let recipe = recipe_book.get(recipe_id).ok_or_else(|| {
        CraftingError::RecipeNotFound { recipe_id: recipe_id.to_string() }
    })?;

    // Skill check
    if player_skill < recipe.required_level {
        return Err(CraftingError::InsufficientSkill {
            required: recipe.required_level,
            actual: player_skill,
        });
    }

    // Ingredient check
    for ing in &recipe.ingredients {
        let available = player_inventory.get(&ing.item_id).copied().unwrap_or(0);
        if available < ing.quantity {
            return Err(CraftingError::MissingIngredient {
                item_id: ing.item_id.clone(),
                required: ing.quantity,
                available,
            });
        }
    }

    // Tool check
    for tool in &recipe.required_tools {
        if !player_tools.contains(tool) {
            return Err(CraftingError::MissingTool { tool_id: tool.clone() });
        }
    }

    // Bench type check: derive expected type from category
    let expected_bench = category_to_bench_type(&recipe.category);
    if let Some(expected) = expected_bench {
        if bench_type != &expected {
            return Err(CraftingError::WorkbenchWrongType {
                expected,
                actual: bench_type.clone(),
            });
        }
    }

    Ok(())
}

/// Map a RecipeCategory to the WorkbenchType it requires (if any).
pub fn category_to_bench_type(category: &recipes::RecipeCategory) -> Option<WorkbenchType> {
    match category {
        recipes::RecipeCategory::Smithing     => Some(WorkbenchType::Forge),
        recipes::RecipeCategory::Alchemy      => Some(WorkbenchType::AlchemyTable),
        recipes::RecipeCategory::Cooking      => Some(WorkbenchType::CookingPot),
        recipes::RecipeCategory::Enchanting   => Some(WorkbenchType::EnchantingTable),
        recipes::RecipeCategory::Engineering  => Some(WorkbenchType::Workbench),
        recipes::RecipeCategory::Tailoring    => Some(WorkbenchType::Loom),
        recipes::RecipeCategory::Woodworking  => Some(WorkbenchType::Workbench),
        recipes::RecipeCategory::Jeweling     => Some(WorkbenchType::Jeweler),
    }
}
