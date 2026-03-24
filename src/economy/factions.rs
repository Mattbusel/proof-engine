//! Faction economy simulation.
//!
//! Each faction has a treasury funded by taxation, income from trade routes,
//! and reparations/tribute. Factions can impose embargoes, conduct economic
//! espionage, and rank themselves by total wealth.

use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FactionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TradeRouteId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TributeAgreementId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EspionageOperationId(pub u32);

// ---------------------------------------------------------------------------
// Treasury
// ---------------------------------------------------------------------------

/// A faction's treasury: tracks gold reserves, income, and expenditure.
#[derive(Debug, Clone)]
pub struct FactionTreasury {
    pub faction: FactionId,
    /// Current gold balance.
    pub gold: f64,
    /// Total gold received over the faction's lifetime.
    pub total_income: f64,
    /// Total gold spent over the faction's lifetime.
    pub total_expenditure: f64,
    /// Per-category income breakdown, last 32 entries per category.
    income_log: HashMap<String, VecDeque<f64>>,
    /// Per-category expenditure breakdown.
    expenditure_log: HashMap<String, VecDeque<f64>>,
}

impl FactionTreasury {
    pub fn new(faction: FactionId, starting_gold: f64) -> Self {
        Self {
            faction,
            gold: starting_gold,
            total_income: starting_gold,
            total_expenditure: 0.0,
            income_log: HashMap::new(),
            expenditure_log: HashMap::new(),
        }
    }

    /// Add income and log it under a category label.
    pub fn receive(&mut self, amount: f64, category: &str) {
        self.gold += amount;
        self.total_income += amount;
        let log = self.income_log.entry(category.to_string()).or_insert_with(|| VecDeque::with_capacity(32));
        log.push_back(amount);
        if log.len() > 32 { log.pop_front(); }
    }

    /// Deduct an expenditure. Returns false if insufficient funds (still deducts to allow debt).
    pub fn spend(&mut self, amount: f64, category: &str) -> bool {
        let had_enough = self.gold >= amount;
        self.gold -= amount;
        self.total_expenditure += amount;
        let log = self.expenditure_log.entry(category.to_string()).or_insert_with(|| VecDeque::with_capacity(32));
        log.push_back(amount);
        if log.len() > 32 { log.pop_front(); }
        had_enough
    }

    /// Moving average income for a category over last n entries.
    pub fn avg_income(&self, category: &str, n: usize) -> f64 {
        match self.income_log.get(category) {
            None => 0.0,
            Some(log) => {
                let slice: Vec<_> = log.iter().rev().take(n).collect();
                if slice.is_empty() { return 0.0; }
                slice.iter().copied().sum::<f64>() / slice.len() as f64
            }
        }
    }

    /// Net income estimate: mean income across all categories per tick.
    pub fn net_income_rate(&self) -> f64 {
        let income: f64 = self.income_log.values()
            .filter_map(|log| {
                if log.is_empty() { return None; }
                Some(log.iter().sum::<f64>() / log.len() as f64)
            })
            .sum();
        let expenditure: f64 = self.expenditure_log.values()
            .filter_map(|log| {
                if log.is_empty() { return None; }
                Some(log.iter().sum::<f64>() / log.len() as f64)
            })
            .sum();
        income - expenditure
    }
}

// ---------------------------------------------------------------------------
// Tax Policy
// ---------------------------------------------------------------------------

/// The tax regime applied to a faction's subjects.
#[derive(Debug, Clone)]
pub struct TaxPolicy {
    pub faction: FactionId,
    /// Flat tax rate applied to all income (0.0 – 1.0).
    pub flat_rate: f64,
    /// Progressive brackets: (threshold, marginal_rate).
    pub brackets: Vec<(f64, f64)>,
    /// Percentage of trade route income taxed.
    pub trade_tax_rate: f64,
    /// Import duty (applied to value of goods arriving from other factions).
    pub import_duty: f64,
    /// Export duty.
    pub export_duty: f64,
    /// Luxury tax multiplier (applied to items tagged as luxury in markets).
    pub luxury_multiplier: f64,
    /// Tax evasion estimate (fraction that slips through).
    pub evasion_rate: f64,
}

impl TaxPolicy {
    pub fn default_for(faction: FactionId) -> Self {
        Self {
            faction,
            flat_rate: 0.15,
            brackets: vec![(1000.0, 0.10), (5000.0, 0.20), (20000.0, 0.35)],
            trade_tax_rate: 0.10,
            import_duty: 0.05,
            export_duty: 0.03,
            luxury_multiplier: 1.5,
            evasion_rate: 0.05,
        }
    }

    /// Compute tax owed on `income` using progressive brackets.
    pub fn compute_tax(&self, income: f64) -> f64 {
        let mut tax = 0.0;
        let mut remaining = income;
        let mut prev_threshold = 0.0;
        for &(threshold, rate) in &self.brackets {
            if remaining <= 0.0 { break; }
            let bracket_income = (threshold - prev_threshold).min(remaining);
            tax += bracket_income * rate;
            remaining -= bracket_income;
            prev_threshold = threshold;
        }
        if remaining > 0.0 {
            tax += remaining * self.flat_rate;
        }
        // Subtract evasion
        tax * (1.0 - self.evasion_rate)
    }

    /// Tax on a trade transaction.
    pub fn trade_tax(&self, value: f64, is_import: bool, is_luxury: bool) -> f64 {
        let duty = if is_import { self.import_duty } else { self.export_duty };
        let base = value * (self.trade_tax_rate + duty);
        let lux = if is_luxury { base * self.luxury_multiplier } else { base };
        lux * (1.0 - self.evasion_rate)
    }
}

// ---------------------------------------------------------------------------
// Trade Routes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeRouteStatus {
    Active,
    Disrupted,
    Embargoed,
    Closed,
}

/// A trade route connecting two factions, carrying goods in both directions.
#[derive(Debug, Clone)]
pub struct TradeRoute {
    pub id: TradeRouteId,
    pub from_faction: FactionId,
    pub to_faction: FactionId,
    pub status: TradeRouteStatus,
    /// Base gold value transacted per tick.
    pub base_volume: f64,
    /// Current actual volume (may differ due to disruptions).
    pub current_volume: f64,
    /// Disruption factor (0.0 = fully blocked, 1.0 = full volume).
    pub disruption: f64,
    /// Fraction of volume that goes to `from_faction` as export income.
    pub export_share_from: f64,
    /// Fraction of volume that goes to `to_faction` as import income.
    pub import_share_to: f64,
    /// Ticks this route has been active.
    pub age_ticks: u64,
    /// History of per-tick volumes.
    pub volume_history: VecDeque<f64>,
    /// Whether a military escort is assigned (reduces disruption).
    pub escorted: bool,
    /// Infrastructure investment level (improves volume and reduces disruption risk).
    pub infrastructure: f64,
}

impl TradeRoute {
    pub fn new(
        id: TradeRouteId,
        from: FactionId,
        to: FactionId,
        base_volume: f64,
        export_share: f64,
        import_share: f64,
    ) -> Self {
        Self {
            id,
            from_faction: from,
            to_faction: to,
            status: TradeRouteStatus::Active,
            base_volume,
            current_volume: base_volume,
            disruption: 1.0,
            export_share_from: export_share,
            import_share_to: import_share,
            age_ticks: 0,
            volume_history: VecDeque::with_capacity(64),
            escorted: false,
            infrastructure: 0.0,
        }
    }

    /// Apply a disruption event (bandits, war, weather). Value in [0,1].
    pub fn apply_disruption(&mut self, severity: f64) {
        let mitigation = if self.escorted { 0.5 } else { 0.0 };
        let infra_mitigation = (self.infrastructure / 200.0).min(0.3);
        self.disruption = (self.disruption - severity * (1.0 - mitigation - infra_mitigation)).max(0.0);
        self.update_status();
    }

    /// Recover disruption over time.
    pub fn recover(&mut self, rate: f64) {
        self.disruption = (self.disruption + rate).min(1.0);
        self.update_status();
    }

    fn update_status(&mut self) {
        if self.status == TradeRouteStatus::Embargoed || self.status == TradeRouteStatus::Closed {
            return;
        }
        self.status = if self.disruption < 0.1 {
            TradeRouteStatus::Disrupted
        } else {
            TradeRouteStatus::Active
        };
    }

    /// Tick: compute actual volume and return (export_income, import_income).
    pub fn tick(&mut self) -> (f64, f64) {
        self.age_ticks += 1;
        if self.status == TradeRouteStatus::Embargoed || self.status == TradeRouteStatus::Closed {
            self.current_volume = 0.0;
            self.volume_history.push_back(0.0);
            if self.volume_history.len() > 64 { self.volume_history.pop_front(); }
            return (0.0, 0.0);
        }
        // Infrastructure bonus: each 100 infra adds 5% volume
        let infra_bonus = 1.0 + (self.infrastructure / 100.0) * 0.05;
        self.current_volume = self.base_volume * self.disruption * infra_bonus;
        self.volume_history.push_back(self.current_volume);
        if self.volume_history.len() > 64 { self.volume_history.pop_front(); }
        let export_income = self.current_volume * self.export_share_from;
        let import_income = self.current_volume * self.import_share_to;
        (export_income, import_income)
    }

    /// Average volume over last n ticks.
    pub fn avg_volume(&self, n: usize) -> f64 {
        let slice: Vec<_> = self.volume_history.iter().rev().take(n).collect();
        if slice.is_empty() { return 0.0; }
        slice.iter().copied().sum::<f64>() / slice.len() as f64
    }
}

// ---------------------------------------------------------------------------
// Embargo / Sanctions
// ---------------------------------------------------------------------------

/// Penalty applied by one faction against another.
#[derive(Debug, Clone)]
pub struct EmbargoPenalty {
    pub imposing_faction: FactionId,
    pub target_faction: FactionId,
    /// All trade routes between these two factions are closed.
    pub blocks_trade: bool,
    /// Asset freeze: prevents target from spending from accounts held jointly.
    pub asset_freeze: bool,
    /// Tariff surcharge applied on top of normal duties (0.0 – 1.0).
    pub extra_tariff: f64,
    /// Tick when imposed.
    pub imposed_tick: u64,
    /// Tick when lifted (None = indefinite).
    pub lifted_tick: Option<u64>,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// War Reparations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WarReparations {
    pub paying_faction: FactionId,
    pub receiving_faction: FactionId,
    /// Total amount owed.
    pub total_amount: f64,
    /// Amount paid so far.
    pub paid: f64,
    /// Per-tick installment.
    pub installment: f64,
    pub imposed_tick: u64,
    pub completed: bool,
}

impl WarReparations {
    pub fn new(payer: FactionId, receiver: FactionId, total: f64, installment: f64, tick: u64) -> Self {
        Self {
            paying_faction: payer,
            receiving_faction: receiver,
            total_amount: total,
            paid: 0.0,
            installment,
            imposed_tick: tick,
            completed: false,
        }
    }

    /// Process one installment. Returns amount actually paid.
    pub fn process(&mut self) -> f64 {
        if self.completed { return 0.0; }
        let amount = self.installment.min(self.total_amount - self.paid);
        self.paid += amount;
        if self.paid >= self.total_amount - 1e-9 { self.completed = true; }
        amount
    }

    pub fn remaining(&self) -> f64 {
        self.total_amount - self.paid
    }
}

// ---------------------------------------------------------------------------
// Tribute System
// ---------------------------------------------------------------------------

/// A recurring tribute agreement between two factions.
#[derive(Debug, Clone)]
pub struct TributeAgreement {
    pub id: TributeAgreementId,
    pub paying_faction: FactionId,
    pub receiving_faction: FactionId,
    /// Amount paid per tick.
    pub amount_per_tick: f64,
    /// Tick when agreement starts.
    pub start_tick: u64,
    /// Duration in ticks (None = indefinite).
    pub duration: Option<u64>,
    pub total_paid: f64,
    pub active: bool,
    /// Coercion factor: if > 0, paying faction has no choice but to pay.
    pub coercion_level: f64,
    /// Chance per tick of the paying faction rebelling (refusing tribute).
    pub rebellion_chance: f64,
}

impl TributeAgreement {
    /// Process one tick of tribute. Returns amount paid or 0 if rebelled/ended.
    pub fn process(&mut self, current_tick: u64, rng_val: f64) -> f64 {
        if !self.active { return 0.0; }
        if let Some(dur) = self.duration {
            if current_tick >= self.start_tick + dur {
                self.active = false;
                return 0.0;
            }
        }
        // Check rebellion (only if coercion is low)
        if self.coercion_level < 0.5 && rng_val < self.rebellion_chance {
            self.active = false;
            return 0.0;
        }
        self.total_paid += self.amount_per_tick;
        self.amount_per_tick
    }
}

// ---------------------------------------------------------------------------
// Economic Espionage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspionageObjective {
    /// Steal treasury balance information.
    IntelTreasury,
    /// Sabotage a trade route.
    SabotageRoute,
    /// Steal a tax policy document.
    StealTaxPolicy,
    /// Plant misinformation to inflate enemy trade costs.
    Disinformation,
    /// Corrupt production facilities (handled by production module).
    CorruptProduction,
}

#[derive(Debug, Clone)]
pub struct EspionageOperation {
    pub id: EspionageOperationId,
    pub instigator: FactionId,
    pub target: FactionId,
    pub objective: EspionageObjective,
    pub started_tick: u64,
    pub completion_tick: u64,
    pub success_probability: f64,
    pub resolved: bool,
    pub succeeded: Option<bool>,
}

/// The outcome of a resolved espionage operation.
#[derive(Debug, Clone)]
pub struct EspionageReport {
    pub operation_id: EspionageOperationId,
    pub instigator: FactionId,
    pub target: FactionId,
    pub objective: EspionageObjective,
    pub succeeded: bool,
    pub resolved_tick: u64,
    pub intel: Option<EspionageIntel>,
}

#[derive(Debug, Clone)]
pub enum EspionageIntel {
    TreasuryBalance(f64),
    TaxRates { flat: f64, trade: f64, import_duty: f64 },
    RouteDisrupted { route_id: TradeRouteId, disruption: f64 },
    DisinformationPlanted,
    ProductionCorrupted,
}

// ---------------------------------------------------------------------------
// Wealth Ranking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WealthRanking {
    /// Factions ordered from richest to poorest.
    pub ranked: Vec<(FactionId, f64)>,
    pub computed_tick: u64,
}

impl WealthRanking {
    /// The wealthiest faction.
    pub fn richest(&self) -> Option<(FactionId, f64)> {
        self.ranked.first().copied()
    }

    /// The poorest faction.
    pub fn poorest(&self) -> Option<(FactionId, f64)> {
        self.ranked.last().copied()
    }

    /// Rank of a specific faction (1 = richest).
    pub fn rank_of(&self, faction: FactionId) -> Option<usize> {
        self.ranked.iter().position(|(id, _)| *id == faction).map(|i| i + 1)
    }

    /// Gini coefficient (0 = perfect equality, 1 = total inequality).
    pub fn gini(&self) -> f64 {
        let n = self.ranked.len();
        if n == 0 { return 0.0; }
        let mut sorted: Vec<f64> = self.ranked.iter().map(|(_, w)| w.max(0.0)).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let total: f64 = sorted.iter().sum();
        if total < 1e-9 { return 0.0; }
        let sum_of_ranks: f64 = sorted.iter().enumerate()
            .map(|(i, w)| (2 * (i + 1) - n - 1) as f64 * w)
            .sum();
        sum_of_ranks / (n as f64 * total)
    }
}

// ---------------------------------------------------------------------------
// Faction descriptor
// ---------------------------------------------------------------------------

/// Full descriptor for a single faction in the economy.
#[derive(Debug, Clone)]
pub struct Faction {
    pub id: FactionId,
    pub name: String,
    pub treasury: FactionTreasury,
    pub tax_policy: TaxPolicy,
    /// IDs of trade routes this faction participates in.
    pub trade_route_ids: Vec<TradeRouteId>,
    /// Factions this faction has imposed embargoes on.
    pub embargoes: Vec<EmbargoPenalty>,
    /// Active reparations this faction must pay.
    pub reparations_paying: Vec<WarReparations>,
    /// Reparations this faction is receiving.
    pub reparations_receiving: Vec<WarReparations>,
    /// Tribute agreements where this faction pays.
    pub tribute_paying: Vec<TributeAgreementId>,
    /// Tribute agreements where this faction receives.
    pub tribute_receiving: Vec<TributeAgreementId>,
    /// Ongoing espionage operations this faction launched.
    pub espionage_ops: Vec<EspionageOperationId>,
    /// Disinformation penalty to outgoing trade routes (fraction of volume lost).
    pub disinformation_penalty: f64,
    /// Cumulative GDP proxy (sum of all trade route volumes over lifetime).
    pub gdp_proxy: f64,
    /// Whether this faction is at war with any others.
    pub at_war_with: Vec<FactionId>,
}

impl Faction {
    pub fn new(id: FactionId, name: &str, starting_gold: f64) -> Self {
        Self {
            id,
            name: name.to_string(),
            treasury: FactionTreasury::new(id, starting_gold),
            tax_policy: TaxPolicy::default_for(id),
            trade_route_ids: Vec::new(),
            embargoes: Vec::new(),
            reparations_paying: Vec::new(),
            reparations_receiving: Vec::new(),
            tribute_paying: Vec::new(),
            tribute_receiving: Vec::new(),
            espionage_ops: Vec::new(),
            disinformation_penalty: 0.0,
            gdp_proxy: 0.0,
            at_war_with: Vec::new(),
        }
    }

    pub fn net_worth(&self) -> f64 {
        self.treasury.gold
    }
}

// ---------------------------------------------------------------------------
// FactionEconomy — the top-level manager
// ---------------------------------------------------------------------------

/// Manages all factions, their trade routes, tribute, reparations,
/// espionage operations, and wealth rankings.
pub struct FactionEconomy {
    next_faction_id: u32,
    next_route_id: u32,
    next_tribute_id: u32,
    next_espionage_id: u32,
    pub current_tick: u64,

    pub factions: HashMap<FactionId, Faction>,
    pub trade_routes: HashMap<TradeRouteId, TradeRoute>,
    pub tribute_agreements: HashMap<TributeAgreementId, TributeAgreement>,
    pub espionage_ops: HashMap<EspionageOperationId, EspionageOperation>,
    pub espionage_reports: Vec<EspionageReport>,
    pub wealth_ranking: Option<WealthRanking>,

    /// Simple deterministic pseudo-random state for rebellion/espionage rolls.
    rng_state: u64,
}

impl FactionEconomy {
    pub fn new() -> Self {
        Self {
            next_faction_id: 1,
            next_route_id: 1,
            next_tribute_id: 1,
            next_espionage_id: 1,
            current_tick: 0,
            factions: HashMap::new(),
            trade_routes: HashMap::new(),
            tribute_agreements: HashMap::new(),
            espionage_ops: HashMap::new(),
            espionage_reports: Vec::new(),
            wealth_ranking: None,
            rng_state: 0xDEAD_BEEF_CAFE_1234,
        }
    }

    // Simple xorshift64 for deterministic rolls
    fn next_rand(&mut self) -> f64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        (x as f64) / (u64::MAX as f64)
    }

    // -----------------------------------------------------------------------
    // Faction Management
    // -----------------------------------------------------------------------

    pub fn add_faction(&mut self, name: &str, starting_gold: f64) -> FactionId {
        let id = FactionId(self.next_faction_id);
        self.next_faction_id += 1;
        self.factions.insert(id, Faction::new(id, name, starting_gold));
        id
    }

    pub fn faction(&self, id: FactionId) -> Option<&Faction> {
        self.factions.get(&id)
    }

    pub fn faction_mut(&mut self, id: FactionId) -> Option<&mut Faction> {
        self.factions.get_mut(&id)
    }

    /// Declare war between two factions (disables existing trade routes between them).
    pub fn declare_war(&mut self, faction_a: FactionId, faction_b: FactionId) {
        if let Some(f) = self.factions.get_mut(&faction_a) {
            if !f.at_war_with.contains(&faction_b) { f.at_war_with.push(faction_b); }
        }
        if let Some(f) = self.factions.get_mut(&faction_b) {
            if !f.at_war_with.contains(&faction_a) { f.at_war_with.push(faction_a); }
        }
        // Close trade routes between them
        let tick = self.current_tick;
        for route in self.trade_routes.values_mut() {
            let involves_a = route.from_faction == faction_a || route.to_faction == faction_a;
            let involves_b = route.from_faction == faction_b || route.to_faction == faction_b;
            if involves_a && involves_b {
                route.status = TradeRouteStatus::Closed;
                let _ = tick; // suppress unused warning
            }
        }
    }

    /// End war between two factions.
    pub fn end_war(&mut self, faction_a: FactionId, faction_b: FactionId) {
        if let Some(f) = self.factions.get_mut(&faction_a) {
            f.at_war_with.retain(|&id| id != faction_b);
        }
        if let Some(f) = self.factions.get_mut(&faction_b) {
            f.at_war_with.retain(|&id| id != faction_a);
        }
    }

    // -----------------------------------------------------------------------
    // Trade Routes
    // -----------------------------------------------------------------------

    pub fn add_trade_route(
        &mut self,
        from: FactionId,
        to: FactionId,
        base_volume: f64,
        export_share: f64,
        import_share: f64,
    ) -> TradeRouteId {
        let id = TradeRouteId(self.next_route_id);
        self.next_route_id += 1;
        let route = TradeRoute::new(id, from, to, base_volume, export_share, import_share);
        self.trade_routes.insert(id, route);
        if let Some(f) = self.factions.get_mut(&from) { f.trade_route_ids.push(id); }
        if let Some(f) = self.factions.get_mut(&to) { f.trade_route_ids.push(id); }
        id
    }

    pub fn disrupt_route(&mut self, id: TradeRouteId, severity: f64) {
        if let Some(route) = self.trade_routes.get_mut(&id) {
            route.apply_disruption(severity);
        }
    }

    pub fn close_route(&mut self, id: TradeRouteId) {
        if let Some(route) = self.trade_routes.get_mut(&id) {
            route.status = TradeRouteStatus::Closed;
        }
    }

    pub fn reopen_route(&mut self, id: TradeRouteId) {
        if let Some(route) = self.trade_routes.get_mut(&id) {
            if route.status == TradeRouteStatus::Closed {
                route.status = TradeRouteStatus::Active;
            }
        }
    }

    pub fn invest_infrastructure(&mut self, id: TradeRouteId, amount: f64) {
        if let Some(route) = self.trade_routes.get_mut(&id) {
            route.infrastructure += amount;
        }
    }

    // -----------------------------------------------------------------------
    // Embargo / Sanctions
    // -----------------------------------------------------------------------

    pub fn impose_embargo(
        &mut self,
        imposing: FactionId,
        target: FactionId,
        extra_tariff: f64,
        blocks_trade: bool,
        asset_freeze: bool,
        reason: &str,
    ) {
        let tick = self.current_tick;
        let embargo = EmbargoPenalty {
            imposing_faction: imposing,
            target_faction: target,
            blocks_trade,
            asset_freeze,
            extra_tariff,
            imposed_tick: tick,
            lifted_tick: None,
            reason: reason.to_string(),
        };
        if blocks_trade {
            // Mark all routes between them as embargoed
            for route in self.trade_routes.values_mut() {
                let involves_i = route.from_faction == imposing || route.to_faction == imposing;
                let involves_t = route.from_faction == target || route.to_faction == target;
                if involves_i && involves_t {
                    route.status = TradeRouteStatus::Embargoed;
                }
            }
        }
        if let Some(f) = self.factions.get_mut(&imposing) {
            f.embargoes.push(embargo);
        }
    }

    pub fn lift_embargo(&mut self, imposing: FactionId, target: FactionId) {
        let tick = self.current_tick;
        if let Some(f) = self.factions.get_mut(&imposing) {
            for emb in f.embargoes.iter_mut() {
                if emb.target_faction == target && emb.lifted_tick.is_none() {
                    emb.lifted_tick = Some(tick);
                }
            }
        }
        // Restore routes if both sides agree
        for route in self.trade_routes.values_mut() {
            let involves_i = route.from_faction == imposing || route.to_faction == imposing;
            let involves_t = route.from_faction == target || route.to_faction == target;
            if involves_i && involves_t && route.status == TradeRouteStatus::Embargoed {
                route.status = TradeRouteStatus::Active;
            }
        }
    }

    // -----------------------------------------------------------------------
    // War Reparations
    // -----------------------------------------------------------------------

    pub fn impose_reparations(
        &mut self,
        payer: FactionId,
        receiver: FactionId,
        total: f64,
        installment: f64,
    ) {
        let tick = self.current_tick;
        let reps = WarReparations::new(payer, receiver, total, installment, tick);
        if let Some(f) = self.factions.get_mut(&payer) {
            f.reparations_paying.push(reps.clone());
        }
        if let Some(f) = self.factions.get_mut(&receiver) {
            f.reparations_receiving.push(reps);
        }
    }

    // -----------------------------------------------------------------------
    // Tribute
    // -----------------------------------------------------------------------

    pub fn add_tribute(
        &mut self,
        payer: FactionId,
        receiver: FactionId,
        amount_per_tick: f64,
        duration: Option<u64>,
        coercion_level: f64,
        rebellion_chance: f64,
    ) -> TributeAgreementId {
        let id = TributeAgreementId(self.next_tribute_id);
        self.next_tribute_id += 1;
        let tick = self.current_tick;
        let agreement = TributeAgreement {
            id,
            paying_faction: payer,
            receiving_faction: receiver,
            amount_per_tick,
            start_tick: tick,
            duration,
            total_paid: 0.0,
            active: true,
            coercion_level,
            rebellion_chance,
        };
        self.tribute_agreements.insert(id, agreement);
        if let Some(f) = self.factions.get_mut(&payer) { f.tribute_paying.push(id); }
        if let Some(f) = self.factions.get_mut(&receiver) { f.tribute_receiving.push(id); }
        id
    }

    // -----------------------------------------------------------------------
    // Espionage
    // -----------------------------------------------------------------------

    /// Launch an espionage operation. It completes after `duration_ticks`.
    pub fn launch_espionage(
        &mut self,
        instigator: FactionId,
        target: FactionId,
        objective: EspionageObjective,
        success_probability: f64,
        duration_ticks: u64,
    ) -> EspionageOperationId {
        let id = EspionageOperationId(self.next_espionage_id);
        self.next_espionage_id += 1;
        let tick = self.current_tick;
        let op = EspionageOperation {
            id,
            instigator,
            target,
            objective,
            started_tick: tick,
            completion_tick: tick + duration_ticks,
            success_probability,
            resolved: false,
            succeeded: None,
        };
        self.espionage_ops.insert(id, op);
        if let Some(f) = self.factions.get_mut(&instigator) {
            f.espionage_ops.push(id);
        }
        id
    }

    /// Resolve operations that have completed this tick.
    fn resolve_espionage(&mut self) {
        let tick = self.current_tick;
        let ready: Vec<EspionageOperationId> = self.espionage_ops.iter()
            .filter(|(_, op)| !op.resolved && op.completion_tick <= tick)
            .map(|(id, _)| *id)
            .collect();

        for op_id in ready {
            let roll = self.next_rand();
            let op = match self.espionage_ops.get_mut(&op_id) {
                Some(o) => o,
                None => continue,
            };
            let success = roll < op.success_probability;
            op.resolved = true;
            op.succeeded = Some(success);

            let instigator = op.instigator;
            let target = op.target;
            let objective = op.objective;

            let intel = if success {
                match objective {
                    EspionageObjective::IntelTreasury => {
                        let balance = self.factions.get(&target)
                            .map(|f| f.treasury.gold)
                            .unwrap_or(0.0);
                        Some(EspionageIntel::TreasuryBalance(balance))
                    }
                    EspionageObjective::StealTaxPolicy => {
                        let (flat, trade, import) = self.factions.get(&target)
                            .map(|f| (f.tax_policy.flat_rate, f.tax_policy.trade_tax_rate, f.tax_policy.import_duty))
                            .unwrap_or((0.0, 0.0, 0.0));
                        Some(EspionageIntel::TaxRates { flat, trade, import_duty: import })
                    }
                    EspionageObjective::SabotageRoute => {
                        // Find a random route involving target
                        let route_ids: Vec<TradeRouteId> = self.trade_routes.keys()
                            .filter(|&&rid| {
                                let r = &self.trade_routes[&rid];
                                r.from_faction == target || r.to_faction == target
                            })
                            .copied()
                            .collect();
                        if let Some(&rid) = route_ids.first() {
                            let disruption_applied = 0.4;
                            if let Some(route) = self.trade_routes.get_mut(&rid) {
                                route.apply_disruption(disruption_applied);
                                Some(EspionageIntel::RouteDisrupted { route_id: rid, disruption: route.disruption })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    EspionageObjective::Disinformation => {
                        if let Some(f) = self.factions.get_mut(&target) {
                            f.disinformation_penalty = (f.disinformation_penalty + 0.15).min(0.5);
                        }
                        Some(EspionageIntel::DisinformationPlanted)
                    }
                    EspionageObjective::CorruptProduction => {
                        Some(EspionageIntel::ProductionCorrupted)
                    }
                }
            } else {
                None
            };

            self.espionage_reports.push(EspionageReport {
                operation_id: op_id,
                instigator,
                target,
                objective,
                succeeded: success,
                resolved_tick: tick,
                intel,
            });
        }
        // Keep reports to last 128
        if self.espionage_reports.len() > 128 {
            let drain = self.espionage_reports.len() - 128;
            self.espionage_reports.drain(0..drain);
        }
    }

    // -----------------------------------------------------------------------
    // Wealth Ranking
    // -----------------------------------------------------------------------

    pub fn compute_wealth_ranking(&mut self) {
        let tick = self.current_tick;
        let mut ranked: Vec<(FactionId, f64)> = self.factions.iter()
            .map(|(&id, f)| (id, f.net_worth()))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        self.wealth_ranking = Some(WealthRanking { ranked, computed_tick: tick });
    }

    // -----------------------------------------------------------------------
    // Taxation Collection
    // -----------------------------------------------------------------------

    /// Collect taxes for all factions based on their trade route income.
    fn collect_taxes(&mut self, route_incomes: &HashMap<FactionId, f64>) {
        for (&faction_id, &income) in route_incomes {
            let tax = self.factions.get(&faction_id)
                .map(|f| f.tax_policy.trade_tax(income, false, false))
                .unwrap_or(0.0);
            if let Some(f) = self.factions.get_mut(&faction_id) {
                f.treasury.receive(tax, "trade_tax");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Main Tick
    // -----------------------------------------------------------------------

    /// Advance the faction economy by one tick.
    ///
    /// - Ticks all trade routes and distributes income.
    /// - Processes reparation installments.
    /// - Processes tribute payments.
    /// - Collects trade taxes.
    /// - Decays disinformation penalties.
    /// - Resolves completed espionage operations.
    /// - Recomputes wealth rankings.
    pub fn tick(&mut self) {
        self.current_tick += 1;
        let tick = self.current_tick;

        // --- Trade routes ---
        let route_ids: Vec<TradeRouteId> = self.trade_routes.keys().copied().collect();
        let mut route_incomes: HashMap<FactionId, f64> = HashMap::new();
        for &rid in &route_ids {
            let (from_id, to_id, export_income, import_income) = {
                let route = self.trade_routes.get_mut(&rid).unwrap();
                let (exp, imp) = route.tick();
                (route.from_faction, route.to_faction, exp, imp)
            };
            // Apply disinformation penalty to exporter
            let dis_penalty = self.factions.get(&from_id).map(|f| f.disinformation_penalty).unwrap_or(0.0);
            let actual_export = export_income * (1.0 - dis_penalty);
            let actual_import = import_income;

            if let Some(f) = self.factions.get_mut(&from_id) {
                f.treasury.receive(actual_export, "trade_export");
                f.gdp_proxy += actual_export;
            }
            if let Some(f) = self.factions.get_mut(&to_id) {
                f.treasury.receive(actual_import, "trade_import");
                f.gdp_proxy += actual_import;
            }
            *route_incomes.entry(from_id).or_insert(0.0) += actual_export;
            *route_incomes.entry(to_id).or_insert(0.0) += actual_import;
            // Route recovery over time
            if let Some(route) = self.trade_routes.get_mut(&rid) {
                route.recover(0.01);
            }
        }

        // Collect taxes on route income
        self.collect_taxes(&route_incomes);

        // --- Reparations ---
        // Collect paying faction IDs so we can process them separately
        let paying_faction_ids: Vec<FactionId> = self.factions.keys().copied().collect();
        for fid in &paying_faction_ids {
            // Process reparations the faction is paying
            let mut to_transfer: Vec<(FactionId, f64)> = Vec::new();
            if let Some(f) = self.factions.get_mut(fid) {
                for rep in f.reparations_paying.iter_mut() {
                    if rep.completed { continue; }
                    let amount = rep.process();
                    if amount > 0.0 {
                        f.treasury.spend(amount, "reparations");
                        to_transfer.push((rep.receiving_faction, amount));
                    }
                }
            }
            for (receiver_id, amount) in to_transfer {
                if let Some(rf) = self.factions.get_mut(&receiver_id) {
                    rf.treasury.receive(amount, "reparations_received");
                }
            }
        }

        // --- Tribute ---
        let rng_val = self.next_rand();
        let tribute_ids: Vec<TributeAgreementId> = self.tribute_agreements.keys().copied().collect();
        for &tid in &tribute_ids {
            let (payer, receiver, amount) = {
                let agr = self.tribute_agreements.get_mut(&tid).unwrap();
                let amount = agr.process(tick, rng_val);
                (agr.paying_faction, agr.receiving_faction, amount)
            };
            if amount > 0.0 {
                if let Some(f) = self.factions.get_mut(&payer) {
                    f.treasury.spend(amount, "tribute");
                }
                if let Some(f) = self.factions.get_mut(&receiver) {
                    f.treasury.receive(amount, "tribute_received");
                }
            }
        }

        // --- Decay disinformation ---
        for f in self.factions.values_mut() {
            f.disinformation_penalty = (f.disinformation_penalty - 0.005).max(0.0);
        }

        // --- Espionage ---
        self.resolve_espionage();

        // --- Wealth Ranking ---
        self.compute_wealth_ranking();
    }

    // -----------------------------------------------------------------------
    // Query Helpers
    // -----------------------------------------------------------------------

    /// All trade routes involving a specific faction.
    pub fn routes_for_faction(&self, faction: FactionId) -> Vec<&TradeRoute> {
        self.trade_routes.values()
            .filter(|r| r.from_faction == faction || r.to_faction == faction)
            .collect()
    }

    /// Total GDP proxy across all factions.
    pub fn global_gdp(&self) -> f64 {
        self.factions.values().map(|f| f.gdp_proxy).sum()
    }

    /// Dominant faction (highest gold).
    pub fn dominant_faction(&self) -> Option<FactionId> {
        self.wealth_ranking.as_ref()?.richest().map(|(id, _)| id)
    }

    /// Active embargoes imposed by a faction.
    pub fn active_embargoes(&self, faction: FactionId) -> Vec<&EmbargoPenalty> {
        self.factions.get(&faction)
            .map(|f| f.embargoes.iter().filter(|e| e.lifted_tick.is_none()).collect())
            .unwrap_or_default()
    }

    /// Espionage reports involving a faction (as instigator or target).
    pub fn reports_for(&self, faction: FactionId) -> Vec<&EspionageReport> {
        self.espionage_reports.iter()
            .filter(|r| r.instigator == faction || r.target == faction)
            .collect()
    }
}

impl Default for FactionEconomy {
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
    fn test_trade_route_income() {
        let mut fe = FactionEconomy::new();
        let a = fe.add_faction("Alpha", 10_000.0);
        let b = fe.add_faction("Beta", 8_000.0);
        let _rid = fe.add_trade_route(a, b, 1000.0, 0.4, 0.3);
        fe.tick();
        let alpha = fe.faction(a).unwrap();
        // Alpha should have received export income
        assert!(alpha.treasury.gold > 10_000.0);
    }

    #[test]
    fn test_embargo_blocks_route() {
        let mut fe = FactionEconomy::new();
        let a = fe.add_faction("Aland", 5_000.0);
        let b = fe.add_faction("Bland", 5_000.0);
        let rid = fe.add_trade_route(a, b, 500.0, 0.5, 0.5);
        fe.impose_embargo(a, b, 0.2, true, false, "Political dispute");
        let route = &fe.trade_routes[&rid];
        assert_eq!(route.status, TradeRouteStatus::Embargoed);
        fe.tick();
        // No income should flow
        let fa = fe.faction(a).unwrap();
        assert!(fa.treasury.gold <= 5_001.0, "embargoed route should not produce income");
    }

    #[test]
    fn test_reparations() {
        let mut fe = FactionEconomy::new();
        let loser = fe.add_faction("Loser", 10_000.0);
        let winner = fe.add_faction("Winner", 5_000.0);
        fe.impose_reparations(loser, winner, 1_000.0, 100.0);
        for _ in 0..10 { fe.tick(); }
        let w = fe.faction(winner).unwrap();
        assert!(w.treasury.gold > 5_000.0 + 999.0);
        let l = fe.faction(loser).unwrap();
        assert!(l.treasury.gold < 10_000.0);
    }

    #[test]
    fn test_wealth_ranking_gini() {
        let mut fe = FactionEconomy::new();
        fe.add_faction("Rich", 100_000.0);
        fe.add_faction("Poor", 1_000.0);
        fe.compute_wealth_ranking();
        let wk = fe.wealth_ranking.as_ref().unwrap();
        let gini = wk.gini();
        assert!(gini > 0.3, "highly unequal distribution should yield gini > 0.3: {}", gini);
    }

    #[test]
    fn test_progressive_tax() {
        let f = FactionId(1);
        let policy = TaxPolicy {
            faction: f,
            flat_rate: 0.40,
            brackets: vec![(1_000.0, 0.10), (5_000.0, 0.25)],
            trade_tax_rate: 0.10,
            import_duty: 0.05,
            export_duty: 0.03,
            luxury_multiplier: 1.5,
            evasion_rate: 0.0,
        };
        let tax = policy.compute_tax(6_000.0);
        // Bracket 1: 1000 * 0.10 = 100
        // Bracket 2: 4000 * 0.25 = 1000
        // Flat:      1000 * 0.40 = 400
        let expected = 100.0 + 1000.0 + 400.0;
        assert!((tax - expected).abs() < 1e-6, "tax={} expected={}", tax, expected);
    }

    #[test]
    fn test_tribute_rebellion() {
        let mut fe = FactionEconomy::new();
        let payer = fe.add_faction("Vassal", 3_000.0);
        let recvr = fe.add_faction("Overlord", 10_000.0);
        // Very high rebellion chance with zero coercion -> will likely rebel quickly
        fe.add_tribute(payer, recvr, 100.0, None, 0.0, 1.0);
        // With rebellion_chance=1.0, the agreement should collapse immediately
        fe.tick();
        let agreement = fe.tribute_agreements.values().next().unwrap();
        assert!(!agreement.active, "tribute should have been rebelled against");
    }

    #[test]
    fn test_war_closes_routes() {
        let mut fe = FactionEconomy::new();
        let a = fe.add_faction("North", 5_000.0);
        let b = fe.add_faction("South", 5_000.0);
        fe.add_trade_route(a, b, 500.0, 0.4, 0.4);
        fe.declare_war(a, b);
        let route = fe.trade_routes.values().next().unwrap();
        assert_eq!(route.status, TradeRouteStatus::Closed);
    }
}
