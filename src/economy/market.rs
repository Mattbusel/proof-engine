//! Dynamic market simulation.
//!
//! Provides commodity price formation through supply and demand, price
//! elasticity, an order book, three auction types, trade-history ring buffer,
//! market-manipulation detection, and arbitrage scanning.

use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// Primitive IDs
// ---------------------------------------------------------------------------

/// Opaque handle for a registered commodity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommodityId(pub u32);

/// Opaque handle for an open order in the order book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderId(pub u64);

/// Opaque handle for a running auction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuctionId(pub u32);

/// Opaque handle for a market participant (player, faction, AI trader).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParticipantId(pub u32);

// ---------------------------------------------------------------------------
// Commodity
// ---------------------------------------------------------------------------

/// Intrinsic properties of a tradeable commodity.
#[derive(Debug, Clone)]
pub struct Commodity {
    pub id: CommodityId,
    pub name: String,
    /// Base price under neutral supply/demand conditions.
    pub base_price: f64,
    /// How many units are available per tick at baseline production.
    pub natural_supply: f64,
    /// Price elasticity coefficient: higher = price reacts more strongly to
    /// supply/demand imbalance (0.1 = inelastic, 1.0+ = very elastic).
    pub elasticity: f64,
    /// Current spot price.
    pub spot_price: f64,
    /// Accumulated supply this tick (reset each tick after matching).
    pub supply: f64,
    /// Accumulated demand this tick.
    pub demand: f64,
    /// Running 7-tick exponential moving average of price.
    pub ema_price: f64,
    /// Volatility (standard deviation of recent log returns).
    pub volatility: f64,
    /// Recent log returns used to compute volatility.
    recent_log_returns: VecDeque<f64>,
    /// Whether this commodity is currently embargoed (no trades allowed).
    pub embargoed: bool,
}

impl Commodity {
    fn new(id: CommodityId, name: &str, base_price: f64, natural_supply: f64, elasticity: f64) -> Self {
        Self {
            id,
            name: name.to_string(),
            base_price,
            natural_supply,
            elasticity,
            spot_price: base_price,
            supply: natural_supply,
            demand: natural_supply,
            ema_price: base_price,
            volatility: 0.0,
            recent_log_returns: VecDeque::with_capacity(20),
            embargoed: false,
        }
    }

    /// Update the EMA and volatility after a price change.
    fn record_price(&mut self, new_price: f64) {
        let old = self.spot_price.max(1e-9);
        let log_ret = (new_price / old).ln();
        self.recent_log_returns.push_back(log_ret);
        if self.recent_log_returns.len() > 20 {
            self.recent_log_returns.pop_front();
        }
        // EMA alpha = 2 / (7 + 1) ≈ 0.25
        let alpha = 0.25;
        self.ema_price = alpha * new_price + (1.0 - alpha) * self.ema_price;
        // volatility = std-dev of log returns
        let n = self.recent_log_returns.len() as f64;
        if n >= 2.0 {
            let mean = self.recent_log_returns.iter().sum::<f64>() / n;
            let var = self.recent_log_returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (n - 1.0);
            self.volatility = var.sqrt();
        }
        self.spot_price = new_price;
    }

    /// Compute a new price given the current supply/demand imbalance.
    fn recompute_price(&mut self) {
        let effective_supply = self.supply.max(1.0);
        let effective_demand = self.demand.max(1.0);
        // Ratio: > 1 means more demand than supply -> price up
        let ratio = effective_demand / effective_supply;
        // log-ratio scaled by elasticity
        let pressure = ratio.ln() * self.elasticity;
        let raw = self.base_price * pressure.exp();
        // Clamp to [base * 0.01, base * 100]
        let clamped = raw.clamp(self.base_price * 0.01, self.base_price * 100.0);
        self.record_price(clamped);
    }
}

// ---------------------------------------------------------------------------
// Order Book
// ---------------------------------------------------------------------------

/// Which side of the market this order is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Expired,
}

/// A single limit order in the order book.
#[derive(Debug, Clone)]
pub struct Order {
    pub id: OrderId,
    pub participant: ParticipantId,
    pub commodity: CommodityId,
    pub side: OrderSide,
    /// Limit price. Buy orders fill at <= limit; sell orders fill at >= limit.
    pub limit_price: f64,
    pub quantity: f64,
    pub filled: f64,
    pub status: OrderStatus,
    /// Tick on which this order was placed.
    pub placed_tick: u64,
    /// Expire if not filled within this many ticks (0 = good-till-cancelled).
    pub ttl: u64,
}

impl Order {
    pub fn remaining(&self) -> f64 {
        self.quantity - self.filled
    }
}

// ---------------------------------------------------------------------------
// Trade History Ring Buffer
// ---------------------------------------------------------------------------

/// A completed trade record stored in the ring buffer.
#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub tick: u64,
    pub commodity: CommodityId,
    pub price: f64,
    pub quantity: f64,
    pub buyer: ParticipantId,
    pub seller: ParticipantId,
}

/// Fixed-capacity ring buffer for trade history.
pub struct TradeHistory {
    buf: VecDeque<TradeRecord>,
    capacity: usize,
}

impl TradeHistory {
    pub fn new(capacity: usize) -> Self {
        Self { buf: VecDeque::with_capacity(capacity), capacity }
    }

    pub fn push(&mut self, record: TradeRecord) {
        if self.buf.len() == self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(record);
    }

    pub fn iter(&self) -> impl Iterator<Item = &TradeRecord> {
        self.buf.iter()
    }

    /// Last N trades for a specific commodity.
    pub fn recent_for(&self, commodity: CommodityId, n: usize) -> Vec<&TradeRecord> {
        self.buf.iter().rev().filter(|r| r.commodity == commodity).take(n).collect()
    }

    /// Volume-weighted average price over the last `n` trades for a commodity.
    pub fn vwap(&self, commodity: CommodityId, n: usize) -> Option<f64> {
        let records: Vec<_> = self.recent_for(commodity, n);
        if records.is_empty() { return None; }
        let total_val: f64 = records.iter().map(|r| r.price * r.quantity).sum();
        let total_qty: f64 = records.iter().map(|r| r.quantity).sum();
        if total_qty < 1e-9 { return None; }
        Some(total_val / total_qty)
    }
}

// ---------------------------------------------------------------------------
// Price chart point
// ---------------------------------------------------------------------------

/// A single data point in a commodity's price history.
#[derive(Debug, Clone)]
pub struct PricePoint {
    pub tick: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

// ---------------------------------------------------------------------------
// Auction System
// ---------------------------------------------------------------------------

/// The three supported auction formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuctionType {
    /// Ascending open-cry auction; highest bidder wins at their bid price.
    English,
    /// Price starts high and drops until a bidder accepts.
    Dutch,
    /// All bids sealed; highest bidder wins, pays second-highest price.
    SealedBid,
}

/// Lifecycle state of an auction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuctionState {
    Open,
    Closing,
    Settled,
    Cancelled,
}

/// A single bid in an auction.
#[derive(Debug, Clone)]
pub struct AuctionBid {
    pub bidder: ParticipantId,
    pub amount: f64,
    pub tick: u64,
}

/// An active or completed auction.
#[derive(Debug, Clone)]
pub struct Auction {
    pub id: AuctionId,
    pub auction_type: AuctionType,
    pub commodity: CommodityId,
    pub quantity: f64,
    pub seller: ParticipantId,
    /// Minimum acceptable price (reserve price).
    pub reserve_price: f64,
    /// Dutch: starting price.
    pub start_price: f64,
    /// Dutch: price decrement per tick.
    pub dutch_decrement: f64,
    /// Current Dutch clock price.
    pub current_dutch_price: f64,
    pub state: AuctionState,
    pub bids: Vec<AuctionBid>,
    pub opened_tick: u64,
    /// Auction closes after this many ticks with no activity (English), or
    /// when Dutch clock hits reserve, or at a fixed end tick.
    pub close_tick: u64,
    pub winner: Option<ParticipantId>,
    pub winning_price: Option<f64>,
}

impl Auction {
    fn highest_bid(&self) -> Option<&AuctionBid> {
        self.bids.iter().max_by(|a, b| a.amount.partial_cmp(&b.amount).unwrap())
    }

    fn second_highest_bid(&self) -> Option<&AuctionBid> {
        if self.bids.len() < 2 { return None; }
        let mut sorted: Vec<f64> = self.bids.iter().map(|b| b.amount).collect();
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
        let second = sorted[1];
        self.bids.iter().find(|b| (b.amount - second).abs() < 1e-9)
    }
}

// ---------------------------------------------------------------------------
// Market Manipulation Detection
// ---------------------------------------------------------------------------

/// Evidence of potential market manipulation by a participant.
#[derive(Debug, Clone)]
pub struct ManipulationAlert {
    pub participant: ParticipantId,
    pub commodity: CommodityId,
    pub alert_type: ManipulationKind,
    pub confidence: f64,
    pub detected_tick: u64,
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManipulationKind {
    /// Placing large orders and cancelling before fill (spoofing).
    Spoofing,
    /// Washing trades: participant appears on both sides.
    WashTrading,
    /// Ramping price artificially with a series of small buys.
    PriceRamping,
    /// Cornering the market: participant holds dominant supply.
    Cornering,
}

// ---------------------------------------------------------------------------
// Arbitrage
// ---------------------------------------------------------------------------

/// An identified arbitrage opportunity between two commodities or two
/// markets (for multi-market expansion).
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub buy_commodity: CommodityId,
    pub sell_commodity: CommodityId,
    /// Conversion ratio: 1 unit buy_commodity converts to `ratio` units sell_commodity.
    pub conversion_ratio: f64,
    pub profit_per_unit: f64,
    pub confidence: f64,
    pub detected_tick: u64,
}

// ---------------------------------------------------------------------------
// Participant Activity Tracker (used for manipulation detection)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct ParticipantActivity {
    orders_placed: u32,
    orders_cancelled: u32,
    buy_volume: f64,
    sell_volume: f64,
    /// Ticks on which large cancel events occurred.
    cancel_spikes: VecDeque<u64>,
    /// Prices at which wash patterns were suspected.
    wash_prices: VecDeque<f64>,
}

// ---------------------------------------------------------------------------
// The Market
// ---------------------------------------------------------------------------

/// Central market simulation. Owns all commodities, the order book, auctions,
/// trade history, and analytics.
pub struct Market {
    next_commodity_id: u32,
    next_order_id: u64,
    next_auction_id: u32,
    pub current_tick: u64,

    pub commodities: HashMap<CommodityId, Commodity>,
    /// Named index: name -> CommodityId
    commodity_names: HashMap<String, CommodityId>,

    /// Order book: per commodity, sorted buy (desc) and sell (asc) lists.
    buy_orders: HashMap<CommodityId, Vec<Order>>,
    sell_orders: HashMap<CommodityId, Vec<Order>>,

    pub trade_history: TradeHistory,
    /// OHLCV candles per commodity, most recent last.
    pub price_history: HashMap<CommodityId, VecDeque<PricePoint>>,

    pub auctions: HashMap<AuctionId, Auction>,
    pub settled_auctions: Vec<Auction>,

    pub manipulation_alerts: Vec<ManipulationAlert>,
    pub arbitrage_opportunities: Vec<ArbitrageOpportunity>,

    /// Per-participant activity for manipulation detection.
    participant_activity: HashMap<ParticipantId, HashMap<CommodityId, ParticipantActivity>>,

    /// Commodity exchange rate relationships for arbitrage (a -> b conversion factor).
    conversion_graph: HashMap<(CommodityId, CommodityId), f64>,

    /// Maximum price history candles kept per commodity.
    max_price_history: usize,
}

impl Market {
    /// Create a new empty market.
    pub fn new() -> Self {
        Self {
            next_commodity_id: 1,
            next_order_id: 1,
            next_auction_id: 1,
            current_tick: 0,
            commodities: HashMap::new(),
            commodity_names: HashMap::new(),
            buy_orders: HashMap::new(),
            sell_orders: HashMap::new(),
            trade_history: TradeHistory::new(4096),
            price_history: HashMap::new(),
            auctions: HashMap::new(),
            settled_auctions: Vec::new(),
            manipulation_alerts: Vec::new(),
            arbitrage_opportunities: Vec::new(),
            participant_activity: HashMap::new(),
            conversion_graph: HashMap::new(),
            max_price_history: 512,
        }
    }

    // -----------------------------------------------------------------------
    // Commodity Registration
    // -----------------------------------------------------------------------

    /// Register a new tradeable commodity. Returns its ID.
    pub fn register_commodity(
        &mut self,
        name: &str,
        base_price: f64,
        natural_supply: f64,
        elasticity: f64,
    ) -> CommodityId {
        let id = CommodityId(self.next_commodity_id);
        self.next_commodity_id += 1;
        let c = Commodity::new(id, name, base_price, natural_supply, elasticity);
        self.commodity_names.insert(name.to_string(), id);
        self.commodities.insert(id, c);
        self.buy_orders.insert(id, Vec::new());
        self.sell_orders.insert(id, Vec::new());
        self.price_history.insert(id, VecDeque::with_capacity(self.max_price_history));
        id
    }

    /// Look up a commodity by name.
    pub fn commodity_by_name(&self, name: &str) -> Option<CommodityId> {
        self.commodity_names.get(name).copied()
    }

    /// Register a commodity conversion relationship for arbitrage scanning.
    pub fn register_conversion(&mut self, from: CommodityId, to: CommodityId, ratio: f64) {
        self.conversion_graph.insert((from, to), ratio);
    }

    /// Embargo or lift embargo on a commodity.
    pub fn set_embargo(&mut self, id: CommodityId, embargoed: bool) {
        if let Some(c) = self.commodities.get_mut(&id) {
            c.embargoed = embargoed;
        }
    }

    // -----------------------------------------------------------------------
    // Supply / Demand Injection
    // -----------------------------------------------------------------------

    /// Inject external supply (from production, imports, etc.).
    pub fn inject_supply(&mut self, commodity: CommodityId, amount: f64) {
        if let Some(c) = self.commodities.get_mut(&commodity) {
            c.supply += amount;
        }
    }

    /// Inject external demand (from consumption, export contracts, etc.).
    pub fn inject_demand(&mut self, commodity: CommodityId, amount: f64) {
        if let Some(c) = self.commodities.get_mut(&commodity) {
            c.demand += amount;
        }
    }

    // -----------------------------------------------------------------------
    // Order Book
    // -----------------------------------------------------------------------

    /// Place a limit order. Returns the order ID.
    pub fn place_order(
        &mut self,
        participant: ParticipantId,
        commodity: CommodityId,
        side: OrderSide,
        limit_price: f64,
        quantity: f64,
        ttl: u64,
    ) -> Option<OrderId> {
        if self.commodities.get(&commodity)?.embargoed { return None; }
        let id = OrderId(self.next_order_id);
        self.next_order_id += 1;
        let order = Order {
            id,
            participant,
            commodity,
            side,
            limit_price,
            quantity,
            filled: 0.0,
            status: OrderStatus::Open,
            placed_tick: self.current_tick,
            ttl,
        };
        // Track activity
        let act = self.participant_activity
            .entry(participant)
            .or_default()
            .entry(commodity)
            .or_default();
        act.orders_placed += 1;
        match side {
            OrderSide::Buy => {
                let book = self.buy_orders.entry(commodity).or_default();
                book.push(order);
                // Keep sorted descending by limit price
                book.sort_by(|a, b| b.limit_price.partial_cmp(&a.limit_price).unwrap());
            }
            OrderSide::Sell => {
                let book = self.sell_orders.entry(commodity).or_default();
                book.push(order);
                // Keep sorted ascending by limit price
                book.sort_by(|a, b| a.limit_price.partial_cmp(&b.limit_price).unwrap());
            }
        }
        Some(id)
    }

    /// Cancel an open order. Returns true if found and cancelled.
    pub fn cancel_order(&mut self, order_id: OrderId) -> bool {
        for orders in self.buy_orders.values_mut().chain(self.sell_orders.values_mut()) {
            if let Some(o) = orders.iter_mut().find(|o| o.id == order_id) {
                if o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled {
                    // Track cancellation for manipulation detection
                    let commodity = o.commodity;
                    let participant = o.participant;
                    let act = self.participant_activity
                        .entry(participant)
                        .or_default()
                        .entry(commodity)
                        .or_default();
                    act.orders_cancelled += 1;
                    act.cancel_spikes.push_back(self.current_tick);
                    if act.cancel_spikes.len() > 20 { act.cancel_spikes.pop_front(); }
                    o.status = OrderStatus::Cancelled;
                    return true;
                }
            }
        }
        false
    }

    /// Get all open orders for a participant.
    pub fn orders_for_participant(&self, participant: ParticipantId) -> Vec<&Order> {
        let mut result = Vec::new();
        for orders in self.buy_orders.values().chain(self.sell_orders.values()) {
            for o in orders {
                if o.participant == participant
                    && (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
                {
                    result.push(o);
                }
            }
        }
        result
    }

    /// Best bid (highest buy limit) for a commodity.
    pub fn best_bid(&self, commodity: CommodityId) -> Option<f64> {
        self.buy_orders.get(&commodity)?
            .iter()
            .filter(|o| o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
            .map(|o| o.limit_price)
            .reduce(f64::max)
    }

    /// Best ask (lowest sell limit) for a commodity.
    pub fn best_ask(&self, commodity: CommodityId) -> Option<f64> {
        self.sell_orders.get(&commodity)?
            .iter()
            .filter(|o| o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled)
            .map(|o| o.limit_price)
            .reduce(f64::min)
    }

    /// Bid-ask spread.
    pub fn spread(&self, commodity: CommodityId) -> Option<f64> {
        Some(self.best_ask(commodity)? - self.best_bid(commodity)?)
    }

    // -----------------------------------------------------------------------
    // Auction System
    // -----------------------------------------------------------------------

    /// Open a new auction. Returns its ID.
    pub fn open_auction(
        &mut self,
        auction_type: AuctionType,
        commodity: CommodityId,
        quantity: f64,
        seller: ParticipantId,
        reserve_price: f64,
        start_price: f64,
        dutch_decrement: f64,
        duration_ticks: u64,
    ) -> AuctionId {
        let id = AuctionId(self.next_auction_id);
        self.next_auction_id += 1;
        let auction = Auction {
            id,
            auction_type,
            commodity,
            quantity,
            seller,
            reserve_price,
            start_price,
            dutch_decrement,
            current_dutch_price: start_price,
            state: AuctionState::Open,
            bids: Vec::new(),
            opened_tick: self.current_tick,
            close_tick: self.current_tick + duration_ticks,
            winner: None,
            winning_price: None,
        };
        self.auctions.insert(id, auction);
        id
    }

    /// Place a bid in an auction.
    pub fn bid_auction(
        &mut self,
        auction_id: AuctionId,
        bidder: ParticipantId,
        amount: f64,
    ) -> bool {
        let tick = self.current_tick;
        let auction = match self.auctions.get_mut(&auction_id) {
            Some(a) if a.state == AuctionState::Open => a,
            _ => return false,
        };
        match auction.auction_type {
            AuctionType::English => {
                let current_high = auction.bids.iter().map(|b| b.amount).fold(0.0_f64, f64::max);
                if amount <= current_high.max(auction.reserve_price) { return false; }
                auction.bids.push(AuctionBid { bidder, amount, tick });
                true
            }
            AuctionType::Dutch => {
                // Accept the current clock price
                if amount >= auction.current_dutch_price {
                    auction.bids.push(AuctionBid { bidder, amount: auction.current_dutch_price, tick });
                    auction.state = AuctionState::Closing;
                    true
                } else {
                    false
                }
            }
            AuctionType::SealedBid => {
                // One bid per participant; blind
                if auction.bids.iter().any(|b| b.bidder == bidder) { return false; }
                auction.bids.push(AuctionBid { bidder, amount, tick });
                true
            }
        }
    }

    /// Settle an auction that has closed. Returns winning price if any.
    fn settle_auction(&mut self, auction_id: AuctionId) -> Option<f64> {
        let tick = self.current_tick;
        let auction = self.auctions.get_mut(&auction_id)?;
        if auction.state == AuctionState::Settled || auction.state == AuctionState::Cancelled {
            return None;
        }
        let (winner, winning_price) = match auction.auction_type {
            AuctionType::English | AuctionType::Dutch => {
                let best = auction.highest_bid()?;
                if best.amount < auction.reserve_price { return None; }
                (best.bidder, best.amount)
            }
            AuctionType::SealedBid => {
                // Vickrey: highest bid wins, pays second-highest price
                let best = auction.highest_bid()?;
                if best.amount < auction.reserve_price { return None; }
                let winner = best.bidder;
                let price = auction.second_highest_bid()
                    .map(|b| b.amount)
                    .unwrap_or(best.amount);
                (winner, price)
            }
        };
        auction.winner = Some(winner);
        auction.winning_price = Some(winning_price);
        auction.state = AuctionState::Settled;

        // Record in trade history
        let commodity = auction.commodity;
        let quantity = auction.quantity;
        let seller = auction.seller;
        self.trade_history.push(TradeRecord {
            tick,
            commodity,
            price: winning_price,
            quantity,
            buyer: winner,
            seller,
        });
        // Influence supply/demand
        if let Some(c) = self.commodities.get_mut(&commodity) {
            c.demand += quantity;
        }
        Some(winning_price)
    }

    // -----------------------------------------------------------------------
    // Order Matching Engine
    // -----------------------------------------------------------------------

    fn match_orders_for(&mut self, commodity: CommodityId) {
        let tick = self.current_tick;
        let mut new_trades: Vec<TradeRecord> = Vec::new();
        loop {
            let best_buy = {
                let buys = self.buy_orders.get(&commodity);
                buys.and_then(|b| b.iter().find(|o| {
                    o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled
                }).map(|o| (o.id, o.limit_price, o.participant, o.remaining())))
            };
            let best_sell = {
                let sells = self.sell_orders.get(&commodity);
                sells.and_then(|s| s.iter().find(|o| {
                    o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled
                }).map(|o| (o.id, o.limit_price, o.participant, o.remaining())))
            };
            match (best_buy, best_sell) {
                (Some((bid_id, bid_price, buyer, bid_rem)),
                 Some((ask_id, ask_price, seller, ask_rem))) => {
                    if bid_price < ask_price { break; }
                    // Price: midpoint between bid and ask
                    let exec_price = (bid_price + ask_price) * 0.5;
                    let fill_qty = bid_rem.min(ask_rem);
                    new_trades.push(TradeRecord {
                        tick,
                        commodity,
                        price: exec_price,
                        quantity: fill_qty,
                        buyer,
                        seller,
                    });
                    // Update buy order
                    if let Some(orders) = self.buy_orders.get_mut(&commodity) {
                        if let Some(o) = orders.iter_mut().find(|o| o.id == bid_id) {
                            o.filled += fill_qty;
                            o.status = if o.remaining() < 1e-9 { OrderStatus::Filled } else { OrderStatus::PartiallyFilled };
                        }
                    }
                    // Update sell order
                    if let Some(orders) = self.sell_orders.get_mut(&commodity) {
                        if let Some(o) = orders.iter_mut().find(|o| o.id == ask_id) {
                            o.filled += fill_qty;
                            o.status = if o.remaining() < 1e-9 { OrderStatus::Filled } else { OrderStatus::PartiallyFilled };
                        }
                    }
                    // Update supply/demand
                    if let Some(c) = self.commodities.get_mut(&commodity) {
                        c.supply += fill_qty;
                        c.demand += fill_qty;
                    }
                    // Track wash trading
                    if buyer == seller {
                        let act = self.participant_activity.entry(buyer).or_default().entry(commodity).or_default();
                        act.wash_prices.push_back(exec_price);
                        if act.wash_prices.len() > 10 { act.wash_prices.pop_front(); }
                    }
                    // Update activity volumes
                    {
                        let act_buy = self.participant_activity.entry(buyer).or_default().entry(commodity).or_default();
                        act_buy.buy_volume += fill_qty;
                    }
                    {
                        let act_sell = self.participant_activity.entry(seller).or_default().entry(commodity).or_default();
                        act_sell.sell_volume += fill_qty;
                    }
                }
                _ => break,
            }
        }
        for trade in new_trades {
            if let Some(c) = self.commodities.get_mut(&trade.commodity) {
                c.record_price(trade.price);
            }
            self.trade_history.push(trade);
        }
    }

    // -----------------------------------------------------------------------
    // Candle / Price History
    // -----------------------------------------------------------------------

    fn open_candle(&self, commodity: CommodityId) -> PricePoint {
        let price = self.commodities.get(&commodity).map(|c| c.spot_price).unwrap_or(0.0);
        PricePoint {
            tick: self.current_tick,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: 0.0,
        }
    }

    fn update_candle_from_trades(&mut self) {
        // For each commodity, find trades this tick and update the open candle.
        let ids: Vec<CommodityId> = self.commodities.keys().copied().collect();
        for id in ids {
            let trades: Vec<(f64, f64)> = self.trade_history.iter()
                .filter(|t| t.commodity == id && t.tick == self.current_tick)
                .map(|t| (t.price, t.quantity))
                .collect();
            if trades.is_empty() { continue; }
            let hist = self.price_history.entry(id).or_default();
            let candle = match hist.back_mut() {
                Some(c) if c.tick == self.current_tick => c,
                _ => {
                    let c = self.open_candle(id);
                    // Can't borrow self mutably twice; push and get back
                    hist.push_back(c);
                    hist.back_mut().unwrap()
                }
            };
            for (price, qty) in trades {
                if price > candle.high { candle.high = price; }
                if price < candle.low { candle.low = price; }
                candle.close = price;
                candle.volume += qty;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Manipulation Detection
    // -----------------------------------------------------------------------

    fn detect_manipulation(&mut self) {
        let tick = self.current_tick;
        let mut alerts: Vec<ManipulationAlert> = Vec::new();

        for (&participant, commodity_map) in &self.participant_activity {
            for (&commodity, act) in commodity_map {
                // Spoofing: cancel rate > 80% of placed orders and recent cancel spikes
                if act.orders_placed >= 5 {
                    let cancel_rate = act.orders_cancelled as f64 / act.orders_placed as f64;
                    if cancel_rate >= 0.80 {
                        let recent_cancels = act.cancel_spikes.iter()
                            .filter(|&&t| tick.saturating_sub(t) <= 10)
                            .count();
                        if recent_cancels >= 3 {
                            alerts.push(ManipulationAlert {
                                participant,
                                commodity,
                                alert_type: ManipulationKind::Spoofing,
                                confidence: (cancel_rate * 100.0).min(100.0),
                                detected_tick: tick,
                                details: format!(
                                    "cancel_rate={:.0}% orders_placed={} recent_cancels={}",
                                    cancel_rate * 100.0, act.orders_placed, recent_cancels
                                ),
                            });
                        }
                    }
                }
                // Wash trading: wash_prices non-empty
                if !act.wash_prices.is_empty() {
                    alerts.push(ManipulationAlert {
                        participant,
                        commodity,
                        alert_type: ManipulationKind::WashTrading,
                        confidence: (act.wash_prices.len() as f64 * 20.0).min(100.0),
                        detected_tick: tick,
                        details: format!("wash_trade_events={}", act.wash_prices.len()),
                    });
                }
                // Price ramping: buy_volume >> sell_volume and price moved > 20%
                if act.buy_volume > act.sell_volume * 5.0 && act.buy_volume > 100.0 {
                    if let Some(c) = self.commodities.get(&commodity) {
                        let price_move = (c.spot_price - c.base_price) / c.base_price.max(1e-9);
                        if price_move > 0.20 {
                            alerts.push(ManipulationAlert {
                                participant,
                                commodity,
                                alert_type: ManipulationKind::PriceRamping,
                                confidence: (price_move * 200.0).min(100.0),
                                detected_tick: tick,
                                details: format!(
                                    "buy_vol={:.1} sell_vol={:.1} price_move={:.1}%",
                                    act.buy_volume, act.sell_volume, price_move * 100.0
                                ),
                            });
                        }
                    }
                }
                // Cornering: single participant holds > 70% of buy-side volume
                let total_buy_vol: f64 = self.participant_activity.values()
                    .filter_map(|cm| cm.get(&commodity))
                    .map(|a| a.buy_volume)
                    .sum();
                if total_buy_vol > 0.0 {
                    let share = act.buy_volume / total_buy_vol;
                    if share > 0.70 && act.buy_volume > 500.0 {
                        alerts.push(ManipulationAlert {
                            participant,
                            commodity,
                            alert_type: ManipulationKind::Cornering,
                            confidence: (share * 100.0).min(100.0),
                            detected_tick: tick,
                            details: format!("market_share={:.1}%", share * 100.0),
                        });
                    }
                }
            }
        }
        self.manipulation_alerts.extend(alerts);
        // Keep only last 256 alerts
        if self.manipulation_alerts.len() > 256 {
            let drain_count = self.manipulation_alerts.len() - 256;
            self.manipulation_alerts.drain(0..drain_count);
        }
    }

    // -----------------------------------------------------------------------
    // Arbitrage Scanning
    // -----------------------------------------------------------------------

    fn scan_arbitrage(&mut self) {
        let tick = self.current_tick;
        let mut opportunities: Vec<ArbitrageOpportunity> = Vec::new();
        let conversions: Vec<((CommodityId, CommodityId), f64)> =
            self.conversion_graph.iter().map(|(&k, &v)| (k, v)).collect();

        for ((from, to), ratio) in conversions {
            let buy_price = match self.commodities.get(&from) {
                Some(c) if !c.embargoed => c.spot_price,
                _ => continue,
            };
            let sell_price = match self.commodities.get(&to) {
                Some(c) if !c.embargoed => c.spot_price,
                _ => continue,
            };
            // Cost to buy 1 unit of `from`, convert to `ratio` units of `to`, sell
            let revenue = sell_price * ratio;
            let profit_per_unit = revenue - buy_price;
            if profit_per_unit > buy_price * 0.02 {
                // > 2% profit margin
                let confidence = (profit_per_unit / buy_price * 10.0).min(1.0);
                opportunities.push(ArbitrageOpportunity {
                    buy_commodity: from,
                    sell_commodity: to,
                    conversion_ratio: ratio,
                    profit_per_unit,
                    confidence,
                    detected_tick: tick,
                });
            }
        }
        self.arbitrage_opportunities = opportunities;
    }

    // -----------------------------------------------------------------------
    // Expire Stale Orders
    // -----------------------------------------------------------------------

    fn expire_orders(&mut self) {
        let tick = self.current_tick;
        for orders in self.buy_orders.values_mut().chain(self.sell_orders.values_mut()) {
            for o in orders.iter_mut() {
                if o.ttl > 0 && (o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled) {
                    if tick.saturating_sub(o.placed_tick) >= o.ttl {
                        o.status = OrderStatus::Expired;
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Auction Tick
    // -----------------------------------------------------------------------

    fn tick_auctions(&mut self) {
        let tick = self.current_tick;
        let ids: Vec<AuctionId> = self.auctions.keys().copied().collect();
        let mut to_settle: Vec<AuctionId> = Vec::new();
        let mut to_cancel: Vec<AuctionId> = Vec::new();

        for &id in &ids {
            let auction = match self.auctions.get_mut(&id) {
                Some(a) if a.state == AuctionState::Open || a.state == AuctionState::Closing => a,
                _ => continue,
            };
            match auction.auction_type {
                AuctionType::Dutch => {
                    // Advance the Dutch clock
                    auction.current_dutch_price -= auction.dutch_decrement;
                    if auction.current_dutch_price <= auction.reserve_price {
                        auction.current_dutch_price = auction.reserve_price;
                        // No bidder -> cancel
                        if auction.bids.is_empty() {
                            to_cancel.push(id);
                        } else {
                            to_settle.push(id);
                        }
                    } else if auction.state == AuctionState::Closing {
                        to_settle.push(id);
                    }
                }
                AuctionType::English | AuctionType::SealedBid => {
                    if tick >= auction.close_tick {
                        if auction.bids.is_empty() {
                            to_cancel.push(id);
                        } else {
                            to_settle.push(id);
                        }
                    }
                }
            }
        }
        for id in to_cancel {
            if let Some(a) = self.auctions.get_mut(&id) {
                a.state = AuctionState::Cancelled;
            }
            if let Some(a) = self.auctions.remove(&id) {
                self.settled_auctions.push(a);
            }
        }
        for id in to_settle {
            self.settle_auction(id);
            if let Some(a) = self.auctions.remove(&id) {
                self.settled_auctions.push(a);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Main Tick
    // -----------------------------------------------------------------------

    /// Advance the market by one simulation tick.
    ///
    /// This:
    /// 1. Expires stale orders.
    /// 2. Matches the order book for every commodity.
    /// 3. Recomputes supply/demand prices.
    /// 4. Updates OHLCV candles.
    /// 5. Ticks all running auctions.
    /// 6. Scans for manipulation and arbitrage.
    /// 7. Resets per-tick supply/demand accumulators.
    pub fn tick(&mut self) {
        self.current_tick += 1;
        self.expire_orders();
        let ids: Vec<CommodityId> = self.commodities.keys().copied().collect();
        for id in &ids {
            self.match_orders_for(*id);
        }
        for id in &ids {
            if let Some(c) = self.commodities.get_mut(id) {
                c.recompute_price();
            }
        }
        self.update_candle_from_trades();
        // Trim price histories
        for hist in self.price_history.values_mut() {
            while hist.len() > self.max_price_history {
                hist.pop_front();
            }
        }
        self.tick_auctions();
        self.detect_manipulation();
        self.scan_arbitrage();
        // Reset per-tick supply/demand to natural levels
        for c in self.commodities.values_mut() {
            c.supply = c.natural_supply;
            c.demand = c.natural_supply;
        }
    }

    // -----------------------------------------------------------------------
    // Query Helpers
    // -----------------------------------------------------------------------

    /// Current spot price for a commodity.
    pub fn spot_price(&self, commodity: CommodityId) -> Option<f64> {
        self.commodities.get(&commodity).map(|c| c.spot_price)
    }

    /// Volume-weighted average price over last n trades.
    pub fn vwap(&self, commodity: CommodityId, n: usize) -> Option<f64> {
        self.trade_history.vwap(commodity, n)
    }

    /// Price history (OHLCV candles), most recent last.
    pub fn price_history(&self, commodity: CommodityId) -> Option<&VecDeque<PricePoint>> {
        self.price_history.get(&commodity)
    }

    /// Current open auctions.
    pub fn open_auctions(&self) -> impl Iterator<Item = &Auction> {
        self.auctions.values()
    }

    /// All alerts raised this market's lifetime.
    pub fn all_alerts(&self) -> &[ManipulationAlert] {
        &self.manipulation_alerts
    }

    /// Arbitrage opportunities detected last tick.
    pub fn arbitrage(&self) -> &[ArbitrageOpportunity] {
        &self.arbitrage_opportunities
    }

    /// Summary statistics for a commodity.
    pub fn commodity_stats(&self, commodity: CommodityId) -> Option<CommodityStats> {
        let c = self.commodities.get(&commodity)?;
        Some(CommodityStats {
            id: commodity,
            name: c.name.clone(),
            spot_price: c.spot_price,
            base_price: c.base_price,
            ema_price: c.ema_price,
            volatility: c.volatility,
            supply: c.supply,
            demand: c.demand,
            bid: self.best_bid(commodity),
            ask: self.best_ask(commodity),
            spread: self.spread(commodity),
            embargoed: c.embargoed,
        })
    }

    /// Depth of book: how many open buy/sell orders exist.
    pub fn book_depth(&self, commodity: CommodityId) -> (usize, usize) {
        let buys = self.buy_orders.get(&commodity).map(|b| {
            b.iter().filter(|o| o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled).count()
        }).unwrap_or(0);
        let sells = self.sell_orders.get(&commodity).map(|s| {
            s.iter().filter(|o| o.status == OrderStatus::Open || o.status == OrderStatus::PartiallyFilled).count()
        }).unwrap_or(0);
        (buys, sells)
    }
}

/// Snapshot of a commodity's current market state.
#[derive(Debug, Clone)]
pub struct CommodityStats {
    pub id: CommodityId,
    pub name: String,
    pub spot_price: f64,
    pub base_price: f64,
    pub ema_price: f64,
    pub volatility: f64,
    pub supply: f64,
    pub demand: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub spread: Option<f64>,
    pub embargoed: bool,
}

impl Default for Market {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_price() {
        let mut m = Market::new();
        let id = m.register_commodity("Gold", 100.0, 500.0, 0.5);
        assert_eq!(m.spot_price(id), Some(100.0));
        m.inject_demand(id, 2000.0);
        m.tick();
        let price = m.spot_price(id).unwrap();
        assert!(price > 100.0, "price should rise with excess demand: {}", price);
    }

    #[test]
    fn test_order_matching() {
        let mut m = Market::new();
        let id = m.register_commodity("Iron", 50.0, 100.0, 0.3);
        let buyer = ParticipantId(1);
        let seller = ParticipantId(2);
        m.place_order(buyer, id, OrderSide::Buy, 55.0, 10.0, 0);
        m.place_order(seller, id, OrderSide::Sell, 45.0, 10.0, 0);
        m.tick();
        // A trade should have occurred
        let trades: Vec<_> = m.trade_history.iter().collect();
        assert!(!trades.is_empty());
    }

    #[test]
    fn test_english_auction() {
        let mut m = Market::new();
        let id = m.register_commodity("Silk", 200.0, 50.0, 0.4);
        let seller = ParticipantId(10);
        let bidder1 = ParticipantId(11);
        let bidder2 = ParticipantId(12);
        let aid = m.open_auction(AuctionType::English, id, 100.0, seller, 150.0, 150.0, 0.0, 10);
        m.bid_auction(aid, bidder1, 160.0);
        m.bid_auction(aid, bidder2, 175.0);
        // Advance past close_tick
        for _ in 0..11 { m.tick(); }
        let settled: Vec<_> = m.settled_auctions.iter().filter(|a| a.id == aid).collect();
        assert!(!settled.is_empty());
        let auction = &settled[0];
        assert_eq!(auction.winner, Some(bidder2));
        assert!((auction.winning_price.unwrap() - 175.0).abs() < 1e-9);
    }

    #[test]
    fn test_sealed_bid_vickrey() {
        let mut m = Market::new();
        let id = m.register_commodity("Gems", 500.0, 10.0, 0.6);
        let seller = ParticipantId(20);
        let bidder1 = ParticipantId(21);
        let bidder2 = ParticipantId(22);
        let aid = m.open_auction(AuctionType::SealedBid, id, 5.0, seller, 400.0, 400.0, 0.0, 5);
        m.bid_auction(aid, bidder1, 600.0);
        m.bid_auction(aid, bidder2, 550.0);
        for _ in 0..6 { m.tick(); }
        let settled: Vec<_> = m.settled_auctions.iter().filter(|a| a.id == aid).collect();
        let auction = &settled[0];
        assert_eq!(auction.winner, Some(bidder1));
        // Vickrey: pays second-highest price
        assert!((auction.winning_price.unwrap() - 550.0).abs() < 1e-9);
    }

    #[test]
    fn test_embargo() {
        let mut m = Market::new();
        let id = m.register_commodity("Spice", 75.0, 200.0, 0.4);
        m.set_embargo(id, true);
        let buyer = ParticipantId(30);
        let result = m.place_order(buyer, id, OrderSide::Buy, 80.0, 10.0, 0);
        assert!(result.is_none(), "orders on embargoed commodity should be rejected");
    }

    #[test]
    fn test_arbitrage_detection() {
        let mut m = Market::new();
        let wheat = m.register_commodity("Wheat", 10.0, 1000.0, 0.3);
        let bread = m.register_commodity("Bread", 35.0, 200.0, 0.5);
        // 1 wheat -> 3 bread, cost 10, revenue 105 => massive profit
        m.register_conversion(wheat, bread, 3.0);
        m.tick();
        assert!(!m.arbitrage_opportunities.is_empty());
    }

    #[test]
    fn test_vwap() {
        let mut m = Market::new();
        let id = m.register_commodity("Wood", 20.0, 500.0, 0.3);
        let b = ParticipantId(1);
        let s = ParticipantId(2);
        m.place_order(b, id, OrderSide::Buy, 25.0, 100.0, 0);
        m.place_order(s, id, OrderSide::Sell, 15.0, 100.0, 0);
        m.tick();
        assert!(m.vwap(id, 10).is_some());
    }
}
