// crafting/economy.rs — Gold, currency, shops, supply/demand, and trade system

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Currency
// ---------------------------------------------------------------------------

/// Three-tier currency: gold, silver, copper.
/// 100 copper = 1 silver, 100 silver = 1 gold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currency {
    pub gold: u64,
    pub silver: u64,
    pub copper: u64,
}

impl Currency {
    pub fn new(gold: u64, silver: u64, copper: u64) -> Self {
        let mut c = Self { gold, silver, copper };
        c.normalize();
        c
    }

    /// Create currency from a total copper amount.
    pub fn from_copper(total: u64) -> Self {
        let gold = total / 10_000;
        let rem = total % 10_000;
        let silver = rem / 100;
        let copper = rem % 100;
        Self { gold, silver, copper }
    }

    /// Create currency from a gold amount (no silver/copper).
    pub fn gold(amount: u64) -> Self {
        Self { gold: amount, silver: 0, copper: 0 }
    }

    /// Create currency from a silver amount (normalizes automatically).
    pub fn silver(amount: u64) -> Self {
        Self::from_copper(amount * 100)
    }

    /// Create currency from a copper amount (normalizes automatically).
    pub fn copper(amount: u64) -> Self {
        Self::from_copper(amount)
    }

    pub fn zero() -> Self {
        Self { gold: 0, silver: 0, copper: 0 }
    }

    /// Convert 100 copper → 1 silver, 100 silver → 1 gold.
    pub fn normalize(&mut self) {
        let carry_silver = self.copper / 100;
        self.copper %= 100;
        self.silver += carry_silver;

        let carry_gold = self.silver / 100;
        self.silver %= 100;
        self.gold += carry_gold;
    }

    /// Total value expressed in copper.
    pub fn to_copper_total(&self) -> u64 {
        self.gold * 10_000 + self.silver * 100 + self.copper
    }

    /// Check if this wallet has at least `amount`.
    pub fn has_at_least(&self, amount: &Currency) -> bool {
        self.to_copper_total() >= amount.to_copper_total()
    }

    /// Attempt to subtract `amount` from this currency.
    /// Returns true on success (and modifies self), false if insufficient funds.
    pub fn try_subtract(&mut self, amount: &Currency) -> bool {
        let total = self.to_copper_total();
        let cost = amount.to_copper_total();
        if total < cost {
            return false;
        }
        *self = Self::from_copper(total - cost);
        true
    }

    /// Add `amount` to this currency.
    pub fn add(&mut self, amount: &Currency) {
        let total = self.to_copper_total() + amount.to_copper_total();
        *self = Self::from_copper(total);
    }

    /// Multiply the currency by a scalar (e.g. for quantity pricing).
    pub fn multiply(&self, factor: u32) -> Self {
        Self::from_copper(self.to_copper_total() * factor as u64)
    }

    /// Scale by a floating-point factor (for discounts / surcharges).
    pub fn scale(&self, factor: f32) -> Self {
        let scaled = (self.to_copper_total() as f32 * factor).round() as u64;
        Self::from_copper(scaled)
    }

    pub fn is_zero(&self) -> bool {
        self.to_copper_total() == 0
    }
}

impl Default for Currency {
    fn default() -> Self {
        Self::zero()
    }
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}g {}s {}c", self.gold, self.silver, self.copper)
    }
}

impl PartialOrd for Currency {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.to_copper_total().partial_cmp(&other.to_copper_total())
    }
}

impl Ord for Currency {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_copper_total().cmp(&other.to_copper_total())
    }
}

// ---------------------------------------------------------------------------
// PriceModifier
// ---------------------------------------------------------------------------

/// Computes a final price from base price with supply/demand and reputation factors.
#[derive(Debug, Clone)]
pub struct PriceModifier {
    pub base_price: u64,
    /// > 1.0 means high demand (raises price).
    pub demand_factor: f32,
    /// < 1.0 means high supply (lowers price).
    pub supply_factor: f32,
    /// > 1.0 means good rep with seller (lowers price via discount); < 1.0 means bad rep.
    pub player_rep_factor: f32,
}

impl PriceModifier {
    pub fn new(base_price: u64) -> Self {
        Self {
            base_price,
            demand_factor: 1.0,
            supply_factor: 1.0,
            player_rep_factor: 1.0,
        }
    }

    /// Compute the final price applying all modifiers.
    pub fn final_price(&self) -> u64 {
        let base = self.base_price as f32;
        // Demand raises price, supply lowers it, high rep gives discount
        let rep_discount = 1.0 + (1.0 - self.player_rep_factor.clamp(0.5, 1.5)) * 0.3;
        let modified = base
            * self.demand_factor.clamp(0.5, 3.0)
            * self.supply_factor.clamp(0.25, 2.0)
            * rep_discount;
        modified.round().max(1.0) as u64
    }

    /// Compute price as a Currency value.
    pub fn final_currency(&self) -> Currency {
        Currency::from_copper(self.final_price())
    }
}

// ---------------------------------------------------------------------------
// PlayerReputation
// ---------------------------------------------------------------------------

/// A player's standing with a specific faction, affecting shop prices.
#[derive(Debug, Clone)]
pub struct PlayerReputation {
    pub faction_id: String,
    /// -1.0 (hostile) to 1.0 (exalted).
    pub value: f32,
}

impl PlayerReputation {
    pub fn new(faction_id: impl Into<String>) -> Self {
        Self { faction_id: faction_id.into(), value: 0.0 }
    }

    /// Adjust reputation by delta, clamped to [-1.0, 1.0].
    pub fn adjust(&mut self, delta: f32) {
        self.value = (self.value + delta).clamp(-1.0, 1.0);
    }

    /// Price factor: higher rep = cheaper prices.
    /// Returns a value between 0.7 (exalted) and 1.4 (hostile).
    pub fn price_factor(&self) -> f32 {
        // value=1 → 0.7, value=0 → 1.0, value=-1 → 1.4
        1.0 - self.value * 0.3
    }

    pub fn label(&self) -> &'static str {
        match self.value {
            v if v >= 0.8  => "Exalted",
            v if v >= 0.5  => "Revered",
            v if v >= 0.2  => "Honored",
            v if v >= -0.2 => "Neutral",
            v if v >= -0.5 => "Unfriendly",
            v if v >= -0.8 => "Hostile",
            _              => "Hated",
        }
    }
}

// ---------------------------------------------------------------------------
// TaxSystem
// ---------------------------------------------------------------------------

/// Region-based sales tax and black market premium.
#[derive(Debug, Clone)]
pub struct TaxSystem {
    /// Tax rate per region id (0.0–1.0).
    region_taxes: HashMap<String, f32>,
    /// Black market premium (multiplier on top of base).
    pub black_market_premium: f32,
}

impl TaxSystem {
    pub fn new() -> Self {
        let mut t = Self {
            region_taxes: HashMap::new(),
            black_market_premium: 1.35,
        };
        // Default regions
        t.region_taxes.insert("capital".into(), 0.08);
        t.region_taxes.insert("frontier".into(), 0.03);
        t.region_taxes.insert("merchant_district".into(), 0.12);
        t.region_taxes.insert("black_market".into(), 0.00);
        t
    }

    /// Set the tax rate for a region.
    pub fn set_region_tax(&mut self, region_id: impl Into<String>, rate: f32) {
        self.region_taxes.insert(region_id.into(), rate.clamp(0.0, 0.5));
    }

    /// Compute the final price after tax and black market premium for a region.
    pub fn apply_tax(&self, base_price: u64, region_id: &str) -> u64 {
        let tax_rate = self.region_taxes.get(region_id).copied().unwrap_or(0.05);
        let taxed = base_price as f32 * (1.0 + tax_rate);
        let black_market = if region_id == "black_market" {
            taxed * self.black_market_premium
        } else {
            taxed
        };
        black_market.round() as u64
    }

    /// Tax amount (not total) for a transaction.
    pub fn tax_amount(&self, base_price: u64, region_id: &str) -> u64 {
        let final_price = self.apply_tax(base_price, region_id);
        final_price.saturating_sub(base_price)
    }
}

impl Default for TaxSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ShopItem
// ---------------------------------------------------------------------------

/// A single item stocked in a shop.
#[derive(Debug, Clone)]
pub struct ShopItem {
    pub item_id: String,
    /// Current stock (-1 means unlimited).
    pub stock: i32,
    /// Maximum stock (0 means sell-only / no auto-restock).
    pub max_stock: i32,
    /// Buy price (what the player pays to buy from shop).
    pub price: Currency,
    /// How many units restock per real second.
    pub restock_rate: f32,
    /// Game time of last restock.
    pub last_restock: f32,
}

impl ShopItem {
    pub fn new(
        item_id: impl Into<String>,
        stock: i32,
        max_stock: i32,
        price: Currency,
        restock_rate: f32,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            stock,
            max_stock,
            price,
            restock_rate,
            last_restock: 0.0,
        }
    }

    /// Price the shop pays the player to buy an item (typically 40% of sell price).
    pub fn buy_back_price(&self) -> Currency {
        self.price.scale(0.40)
    }

    pub fn is_in_stock(&self) -> bool {
        self.stock == -1 || self.stock > 0
    }

    pub fn is_unlimited(&self) -> bool {
        self.stock == -1
    }
}

// ---------------------------------------------------------------------------
// ShopInventory
// ---------------------------------------------------------------------------

/// A merchant's shop with items, buying, selling, and restocking.
#[derive(Debug, Clone)]
pub struct ShopInventory {
    pub shop_id: String,
    pub shop_name: String,
    pub faction_id: String,
    items: HashMap<String, ShopItem>,
    /// Region the shop is in (for tax purposes).
    pub region_id: String,
    pub tax_system: TaxSystem,
}

impl ShopInventory {
    pub fn new(
        shop_id: impl Into<String>,
        shop_name: impl Into<String>,
        faction_id: impl Into<String>,
        region_id: impl Into<String>,
    ) -> Self {
        Self {
            shop_id: shop_id.into(),
            shop_name: shop_name.into(),
            faction_id: faction_id.into(),
            items: HashMap::new(),
            region_id: region_id.into(),
            tax_system: TaxSystem::new(),
        }
    }

    /// Add or replace an item in the shop.
    pub fn add_item(&mut self, item: ShopItem) {
        self.items.insert(item.item_id.clone(), item);
    }

    /// Get a shop item by id.
    pub fn get_item(&self, item_id: &str) -> Option<&ShopItem> {
        self.items.get(item_id)
    }

    /// All items in the shop.
    pub fn all_items(&self) -> Vec<&ShopItem> {
        self.items.values().collect()
    }

    /// Player buys `qty` units of `item_id` from the shop.
    ///
    /// `player_currency` is modified in place.
    /// Returns Ok(total_cost) or Err with a description.
    pub fn buy(
        &mut self,
        item_id: &str,
        qty: u32,
        player_currency: &mut Currency,
        player_rep: f32,
    ) -> Result<Currency, String> {
        let item = self.items.get_mut(item_id)
            .ok_or_else(|| format!("Item '{}' not found in shop", item_id))?;

        if !item.is_unlimited() && (item.stock as u32) < qty {
            return Err(format!("Not enough stock: have {}, need {}", item.stock, qty));
        }

        // Apply reputation discount
        let rep_scale = 1.0 - player_rep.clamp(-1.0, 1.0) * 0.3;
        let unit_price = item.price.scale(rep_scale);
        let total_cost = unit_price.multiply(qty);

        // Apply tax
        let taxed_total_copper = self.tax_system.apply_tax(total_cost.to_copper_total(), &self.region_id);
        let taxed_total = Currency::from_copper(taxed_total_copper);

        if !player_currency.has_at_least(&taxed_total) {
            return Err(format!("Insufficient funds: need {}, have {}", taxed_total, player_currency));
        }

        player_currency.try_subtract(&taxed_total);
        if !item.is_unlimited() {
            item.stock -= qty as i32;
        }

        Ok(taxed_total)
    }

    /// Player sells `qty` units of `item_id` to the shop.
    ///
    /// `player_currency` is credited the buy-back price.
    /// Returns Ok(total_received) or Err with a description.
    pub fn sell(
        &mut self,
        item_id: &str,
        qty: u32,
        player_currency: &mut Currency,
    ) -> Result<Currency, String> {
        let item = self.items.get_mut(item_id)
            .ok_or_else(|| format!("Shop does not buy '{}'", item_id))?;

        // Check if shop has room
        if item.max_stock > 0 && item.stock >= item.max_stock {
            return Err(format!("Shop is full for item '{}'", item_id));
        }

        let payout = item.buy_back_price().multiply(qty);
        player_currency.add(&payout);

        // Add stock to shop (limited by max_stock)
        if item.max_stock > 0 {
            item.stock = (item.stock + qty as i32).min(item.max_stock);
        }

        Ok(payout)
    }

    /// Advance time, restocking items that have restock_rate > 0.
    pub fn restock(&mut self, dt: f32, current_time: f32) {
        for item in self.items.values_mut() {
            if item.restock_rate <= 0.0 || item.max_stock <= 0 {
                continue;
            }
            if item.stock >= item.max_stock {
                continue;
            }
            let time_since = current_time - item.last_restock;
            let units_to_add = (time_since * item.restock_rate).floor() as i32;
            if units_to_add >= 1 {
                item.stock = (item.stock + units_to_add).min(item.max_stock);
                item.last_restock = current_time;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TradeOffer
// ---------------------------------------------------------------------------

/// A barter proposal between two parties.
#[derive(Debug, Clone)]
pub struct TradeOffer {
    pub id: u64,
    /// Items offered by the proposer (item_id, quantity).
    pub from_items: Vec<(String, u32)>,
    /// Items requested in return (item_id, quantity).
    pub to_items: Vec<(String, u32)>,
    /// Gold component: positive means gold flows from proposer to receiver,
    /// negative means gold flows from receiver to proposer.
    pub gold_delta: i64,
    /// Game time at which this offer expires.
    pub expires_at: f32,
    pub proposer_id: String,
    pub receiver_id: String,
    pub accepted: bool,
    pub rejected: bool,
}

impl TradeOffer {
    pub fn new(
        id: u64,
        proposer_id: impl Into<String>,
        receiver_id: impl Into<String>,
        from_items: Vec<(String, u32)>,
        to_items: Vec<(String, u32)>,
        gold_delta: i64,
        expires_at: f32,
    ) -> Self {
        Self {
            id,
            from_items,
            to_items,
            gold_delta,
            expires_at,
            proposer_id: proposer_id.into(),
            receiver_id: receiver_id.into(),
            accepted: false,
            rejected: false,
        }
    }

    pub fn is_expired(&self, current_time: f32) -> bool {
        current_time > self.expires_at
    }

    pub fn is_pending(&self) -> bool {
        !self.accepted && !self.rejected
    }
}

// ---------------------------------------------------------------------------
// Moving average ring buffer for price smoothing
// ---------------------------------------------------------------------------

const PRICE_HISTORY_SIZE: usize = 20;

/// Ring buffer storing the last N prices for moving average computation.
#[derive(Debug, Clone)]
struct PriceRingBuffer {
    values: [u64; PRICE_HISTORY_SIZE],
    head: usize,
    count: usize,
}

impl PriceRingBuffer {
    fn new() -> Self {
        Self { values: [0u64; PRICE_HISTORY_SIZE], head: 0, count: 0 }
    }

    fn push(&mut self, price: u64) {
        self.values[self.head] = price;
        self.head = (self.head + 1) % PRICE_HISTORY_SIZE;
        if self.count < PRICE_HISTORY_SIZE {
            self.count += 1;
        }
    }

    fn moving_average(&self) -> f32 {
        if self.count == 0 {
            return 0.0;
        }
        let sum: u64 = self.values[..self.count].iter().sum();
        sum as f32 / self.count as f32
    }

    fn last(&self) -> Option<u64> {
        if self.count == 0 {
            return None;
        }
        let last_idx = if self.head == 0 { PRICE_HISTORY_SIZE - 1 } else { self.head - 1 };
        Some(self.values[last_idx])
    }
}

// ---------------------------------------------------------------------------
// Economy — global supply/demand simulation
// ---------------------------------------------------------------------------

/// Per-item economic state.
#[derive(Debug, Clone)]
struct ItemEconomy {
    /// Units sold recently (demand pressure).
    recent_sales: f32,
    /// Units purchased from producers recently (supply pressure).
    recent_supply: f32,
    /// Current equilibrium price in copper.
    equilibrium_price: u64,
    /// Base (floor) price in copper.
    base_price: u64,
    price_history: PriceRingBuffer,
}

impl ItemEconomy {
    fn new(base_price: u64) -> Self {
        Self {
            recent_sales: 0.0,
            recent_supply: 1.0,
            equilibrium_price: base_price,
            base_price,
            price_history: PriceRingBuffer::new(),
        }
    }

    /// Demand factor: sales / supply, clamped.
    fn demand_factor(&self) -> f32 {
        if self.recent_supply <= 0.0 {
            return 3.0;
        }
        (self.recent_sales / self.recent_supply).clamp(0.25, 3.0)
    }

    /// Update equilibrium price based on current supply/demand.
    fn update_price(&mut self) {
        let factor = self.demand_factor();
        // Smoothly adjust equilibrium price toward target
        let target = (self.base_price as f32 * factor).round() as u64;
        // Lerp form: eq = eq + t * (target - eq) with t = 0.1
        let t = 0.10_f32;
        let eq = self.equilibrium_price as f32;
        let new_eq = eq + t * (target as f32 - eq);
        self.equilibrium_price = new_eq.round().max(1.0) as u64;
        self.price_history.push(self.equilibrium_price);
    }

    /// Decay sales/supply figures toward zero (time decay).
    fn decay(&mut self, dt: f32) {
        let decay_rate = 0.02_f32 * dt;
        self.recent_sales = (self.recent_sales * (1.0 - decay_rate)).max(0.0);
        self.recent_supply = (self.recent_supply * (1.0 - decay_rate * 0.5)).max(0.01);
    }
}

/// The global economy — tracks supply, demand, and prices for all items.
#[derive(Debug, Clone)]
pub struct Economy {
    items: HashMap<String, ItemEconomy>,
    pub tax_system: TaxSystem,
    /// Accumulated dt for periodic price updates (updates every ~30s).
    update_accumulator: f32,
    update_interval: f32,
}

impl Economy {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            tax_system: TaxSystem::new(),
            update_accumulator: 0.0,
            update_interval: 30.0,
        }
    }

    /// Register an item with a base price (in copper).
    pub fn register_item(&mut self, item_id: impl Into<String>, base_price_copper: u64) {
        self.items.insert(item_id.into(), ItemEconomy::new(base_price_copper));
    }

    /// Get the current equilibrium price for an item (in copper).
    pub fn current_price(&self, item_id: &str) -> Option<u64> {
        self.items.get(item_id).map(|e| e.equilibrium_price)
    }

    /// Get the current price as Currency.
    pub fn current_price_currency(&self, item_id: &str) -> Option<Currency> {
        self.current_price(item_id).map(Currency::from_copper)
    }

    /// Get the moving average price for an item.
    pub fn average_price(&self, item_id: &str) -> Option<f32> {
        self.items.get(item_id).map(|e| e.price_history.moving_average())
    }

    /// Get the demand factor for an item.
    pub fn demand_factor(&self, item_id: &str) -> f32 {
        self.items.get(item_id).map(|e| e.demand_factor()).unwrap_or(1.0)
    }

    /// Record a sale event (player buys item), increasing demand pressure.
    pub fn record_sale(&mut self, item_id: &str, qty: u32, _price_copper: u64) {
        if let Some(item) = self.items.get_mut(item_id) {
            item.recent_sales += qty as f32;
        }
    }

    /// Record a supply event (item produced or imported), increasing supply pressure.
    pub fn record_purchase(&mut self, item_id: &str, qty: u32) {
        if let Some(item) = self.items.get_mut(item_id) {
            item.recent_supply += qty as f32;
        }
    }

    /// Update all item prices based on supply/demand, called each game tick.
    pub fn update_prices(&mut self, dt: f32) {
        // Decay all items every tick
        for item in self.items.values_mut() {
            item.decay(dt);
        }

        // Periodically recompute equilibrium prices
        self.update_accumulator += dt;
        if self.update_accumulator >= self.update_interval {
            self.update_accumulator = 0.0;
            for item in self.items.values_mut() {
                item.update_price();
            }
        }
    }

    /// Build a PriceModifier for an item and player reputation factor.
    pub fn price_modifier(&self, item_id: &str, player_rep_factor: f32) -> PriceModifier {
        let item = self.items.get(item_id);
        let base_price = item.map(|e| e.base_price).unwrap_or(100);
        let demand_factor = item.map(|e| e.demand_factor()).unwrap_or(1.0);
        let supply_factor = if let Some(e) = item {
            if e.recent_sales > 0.0 {
                (e.recent_supply / e.recent_sales).clamp(0.25, 2.0)
            } else {
                1.0
            }
        } else {
            1.0
        };
        PriceModifier {
            base_price,
            demand_factor,
            supply_factor,
            player_rep_factor,
        }
    }

    /// Populate the economy with default items and prices.
    pub fn register_defaults(&mut self) {
        // Smithing materials
        self.register_item("iron_ingot", 200);
        self.register_item("steel_ingot", 600);
        self.register_item("coal", 30);
        self.register_item("leather_strip", 50);
        self.register_item("wooden_plank", 40);
        // Equipment
        self.register_item("iron_sword", 1500);
        self.register_item("iron_shield", 1800);
        self.register_item("steel_sword", 4500);
        self.register_item("iron_helmet", 1200);
        // Alchemy
        self.register_item("red_herb", 60);
        self.register_item("blue_herb", 80);
        self.register_item("clean_water", 10);
        self.register_item("spring_water", 25);
        self.register_item("golden_root", 250);
        self.register_item("health_potion_minor", 500);
        self.register_item("health_potion_major", 1500);
        self.register_item("mana_potion", 700);
        // Cooking
        self.register_item("raw_meat", 80);
        self.register_item("salt", 20);
        self.register_item("roasted_meat", 180);
        self.register_item("hearty_stew", 400);
        // Enchanting
        self.register_item("magic_dust", 300);
        self.register_item("fire_essence", 800);
        self.register_item("light_rune", 600);
        // Jeweling
        self.register_item("silver_ingot", 500);
        self.register_item("gold_chain", 2000);
        self.register_item("ruby_gem", 3000);
        self.register_item("sapphire_gem", 3500);
        self.register_item("silver_ring", 1200);
        self.register_item("ruby_necklace", 8000);
    }
}

impl Default for Economy {
    fn default() -> Self {
        let mut e = Self::new();
        e.register_defaults();
        e
    }
}
