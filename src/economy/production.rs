//! Production chain simulation.
//!
//! Models resource extraction nodes (mines, farms, forests), multi-stage
//! processing buildings that transform inputs to outputs, worker assignment,
//! production quotas, efficiency modifiers, supply chain disruptions, and
//! per-node stockpile management.

use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildingId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkerId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecipeId(pub u32);

/// Commodity reference (mirrors market::CommodityId but avoids cross-module dependency).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommodityRef(pub u32);

// ---------------------------------------------------------------------------
// Resource Nodes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Mine,
    Farm,
    Forest,
    Quarry,
    Fishery,
    OilWell,
    HerbGarden,
}

impl NodeKind {
    /// Base yield variance (±fraction of base yield).
    pub fn yield_variance(&self) -> f64 {
        match self {
            NodeKind::Mine => 0.05,
            NodeKind::Farm => 0.20,
            NodeKind::Forest => 0.10,
            NodeKind::Quarry => 0.03,
            NodeKind::Fishery => 0.25,
            NodeKind::OilWell => 0.08,
            NodeKind::HerbGarden => 0.15,
        }
    }

    /// Natural depletion rate per tick (fraction of reserves consumed).
    pub fn depletion_rate(&self) -> f64 {
        match self {
            NodeKind::Mine => 0.001,
            NodeKind::Farm => 0.0,       // farms are renewable
            NodeKind::Forest => 0.0002,
            NodeKind::Quarry => 0.0008,
            NodeKind::Fishery => 0.0015,
            NodeKind::OilWell => 0.002,
            NodeKind::HerbGarden => 0.0,
        }
    }

    /// Whether this node is renewable (deplete_rate ignored for renewal calculations).
    pub fn is_renewable(&self) -> bool {
        matches!(self, NodeKind::Farm | NodeKind::HerbGarden)
    }
}

/// A resource extraction node in the world.
#[derive(Debug, Clone)]
pub struct ResourceNode {
    pub id: NodeId,
    pub name: String,
    pub kind: NodeKind,
    /// The commodity this node produces.
    pub output_commodity: CommodityRef,
    /// Base output per worker per tick.
    pub base_yield_per_worker: f64,
    /// Remaining reserves (None = infinite for renewable).
    pub reserves: Option<f64>,
    /// Maximum reserves (for computing depletion fraction).
    pub max_reserves: Option<f64>,
    /// Workers currently assigned.
    pub workers_assigned: u32,
    /// Maximum worker capacity.
    pub worker_capacity: u32,
    /// Accumulated stockpile (output not yet transferred).
    pub stockpile: f64,
    /// Maximum stockpile before output is lost.
    pub stockpile_capacity: f64,
    /// Current efficiency (0.0 – 1.0 plus bonuses).
    pub efficiency: f64,
    /// Degradation: amount efficiency decays per tick without maintenance.
    pub degradation_rate: f64,
    /// Ticks until scheduled maintenance (0 = needs maintenance).
    pub maintenance_due_in: u64,
    /// Disruption severity (0 = none, 1 = fully halted).
    pub disruption: f64,
    /// Season modifier (1.0 = normal, farm might be 0.0 in winter).
    pub season_modifier: f64,
    /// Output history ring buffer (last 64 ticks).
    pub output_history: VecDeque<f64>,
    /// Whether this node is active.
    pub active: bool,
}

impl ResourceNode {
    pub fn new(
        id: NodeId,
        name: &str,
        kind: NodeKind,
        output_commodity: CommodityRef,
        base_yield_per_worker: f64,
        reserves: Option<f64>,
        worker_capacity: u32,
        stockpile_capacity: f64,
    ) -> Self {
        let max_reserves = reserves;
        Self {
            id,
            name: name.to_string(),
            kind,
            output_commodity,
            base_yield_per_worker,
            reserves,
            max_reserves,
            workers_assigned: 0,
            worker_capacity,
            stockpile: 0.0,
            stockpile_capacity,
            efficiency: 1.0,
            degradation_rate: 0.001,
            maintenance_due_in: 200,
            disruption: 0.0,
            season_modifier: 1.0,
            output_history: VecDeque::with_capacity(64),
            active: true,
        }
    }

    /// Assign up to `count` additional workers. Returns how many were actually assigned.
    pub fn assign_workers(&mut self, count: u32) -> u32 {
        let available = self.worker_capacity.saturating_sub(self.workers_assigned);
        let added = count.min(available);
        self.workers_assigned += added;
        added
    }

    /// Remove workers.
    pub fn remove_workers(&mut self, count: u32) {
        self.workers_assigned = self.workers_assigned.saturating_sub(count);
    }

    /// Compute gross output for this tick before stockpile and reserve limits.
    pub fn compute_output(&self, rng_variance: f64) -> f64 {
        if !self.active || self.workers_assigned == 0 { return 0.0; }
        // Reserves check
        if let Some(r) = self.reserves {
            if r <= 0.0 { return 0.0; }
        }
        let variance = self.kind.yield_variance();
        let noise = 1.0 + (rng_variance - 0.5) * 2.0 * variance;
        let gross = self.base_yield_per_worker
            * self.workers_assigned as f64
            * self.efficiency
            * self.season_modifier
            * (1.0 - self.disruption)
            * noise;
        gross.max(0.0)
    }

    /// Apply output to the stockpile and consume reserves. Returns actual output.
    pub fn tick_output(&mut self, rng_variance: f64) -> f64 {
        let gross = self.compute_output(rng_variance);
        // Deplete reserves
        if let Some(r) = self.reserves.as_mut() {
            let consumed = gross * self.kind.depletion_rate() * 500.0; // scaling factor
            *r = (*r - consumed).max(0.0);
        }
        // Add to stockpile (capped)
        let actual = (self.stockpile + gross).min(self.stockpile_capacity);
        let produced = actual - self.stockpile;
        self.stockpile = actual;
        // History
        self.output_history.push_back(produced);
        if self.output_history.len() > 64 { self.output_history.pop_front(); }
        // Degrade efficiency
        self.efficiency = (self.efficiency - self.degradation_rate).max(0.1);
        if self.maintenance_due_in > 0 { self.maintenance_due_in -= 1; }
        produced
    }

    /// Withdraw up to `amount` from the stockpile.
    pub fn withdraw(&mut self, amount: f64) -> f64 {
        let taken = amount.min(self.stockpile);
        self.stockpile -= taken;
        taken
    }

    /// Perform maintenance: restore efficiency.
    pub fn perform_maintenance(&mut self) {
        self.efficiency = (self.efficiency + 0.3).min(1.2); // can briefly exceed 1.0 after maintenance
        self.maintenance_due_in = 200;
    }

    /// Apply a disruption event.
    pub fn apply_disruption(&mut self, severity: f64) {
        self.disruption = (self.disruption + severity).min(1.0);
    }

    /// Recover from disruption over time.
    pub fn recover_disruption(&mut self, rate: f64) {
        self.disruption = (self.disruption - rate).max(0.0);
    }

    /// Average output over last n ticks.
    pub fn avg_output(&self, n: usize) -> f64 {
        let slice: Vec<_> = self.output_history.iter().rev().take(n).collect();
        if slice.is_empty() { return 0.0; }
        slice.iter().copied().sum::<f64>() / slice.len() as f64
    }

    /// Reserve depletion fraction (0 = full, 1 = empty). None if unlimited.
    pub fn depletion_fraction(&self) -> Option<f64> {
        let r = self.reserves?;
        let max = self.max_reserves?;
        if max < 1e-9 { return Some(1.0); }
        Some(1.0 - r / max)
    }
}

// ---------------------------------------------------------------------------
// Recipes
// ---------------------------------------------------------------------------

/// An ingredient in a recipe.
#[derive(Debug, Clone)]
pub struct Ingredient {
    pub commodity: CommodityRef,
    pub amount: f64,
}

/// A production recipe: consumes inputs to produce outputs.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub id: RecipeId,
    pub name: String,
    pub inputs: Vec<Ingredient>,
    pub outputs: Vec<Ingredient>,
    /// Base ticks required to complete one batch.
    pub base_ticks: u64,
    /// How many workers are required per batch.
    pub workers_required: u32,
    /// Power/energy units required per batch.
    pub energy_required: f64,
}

impl Recipe {
    /// Check if the given input quantities are sufficient for one batch.
    pub fn can_run(&self, available: &HashMap<CommodityRef, f64>) -> bool {
        self.inputs.iter().all(|ing| {
            available.get(&ing.commodity).copied().unwrap_or(0.0) >= ing.amount
        })
    }
}

// ---------------------------------------------------------------------------
// Production Quota
// ---------------------------------------------------------------------------

/// A production target/quota for a building.
#[derive(Debug, Clone)]
pub struct ProductionQuota {
    pub building: BuildingId,
    /// Target output per tick.
    pub target_per_tick: f64,
    pub commodity: CommodityRef,
    /// Current fulfillment fraction (0.0 – 1.0).
    pub fulfillment: f64,
    /// History of fulfillment fractions.
    pub fulfillment_history: VecDeque<f64>,
    /// Whether the quota is mandatory (shortfall triggers a supply chain event).
    pub mandatory: bool,
}

impl ProductionQuota {
    pub fn new(building: BuildingId, target: f64, commodity: CommodityRef, mandatory: bool) -> Self {
        Self {
            building,
            target_per_tick: target,
            commodity,
            fulfillment: 1.0,
            fulfillment_history: VecDeque::with_capacity(32),
            mandatory,
        }
    }

    pub fn update_fulfillment(&mut self, actual: f64) {
        self.fulfillment = if self.target_per_tick > 0.0 {
            (actual / self.target_per_tick).min(1.0)
        } else {
            1.0
        };
        self.fulfillment_history.push_back(self.fulfillment);
        if self.fulfillment_history.len() > 32 { self.fulfillment_history.pop_front(); }
    }

    pub fn avg_fulfillment(&self, n: usize) -> f64 {
        let slice: Vec<_> = self.fulfillment_history.iter().rev().take(n).collect();
        if slice.is_empty() { return 1.0; }
        slice.iter().copied().sum::<f64>() / slice.len() as f64
    }
}

// ---------------------------------------------------------------------------
// Efficiency Modifiers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKind {
    /// Skilled workforce bonus.
    SkilledLabor,
    /// Advanced machinery.
    Machinery,
    /// Infrastructure (roads, storage).
    Infrastructure,
    /// Negative: weather disruption.
    WeatherPenalty,
    /// Negative: shortage of an input commodity.
    InputShortage,
    /// Positive: research bonus.
    ResearchBonus,
    /// Positive: overseer present.
    OverseerBonus,
    /// Negative: corruption / graft.
    Corruption,
}

#[derive(Debug, Clone)]
pub struct EfficiencyModifier {
    pub kind: ModifierKind,
    pub magnitude: f64,
    /// Ticks remaining; 0 = permanent until explicitly removed.
    pub duration: u64,
    pub description: String,
}

impl EfficiencyModifier {
    pub fn new(kind: ModifierKind, magnitude: f64, duration: u64, description: &str) -> Self {
        Self { kind, magnitude, duration, description: description.to_string() }
    }

    pub fn effective_multiplier(&self) -> f64 {
        match self.kind {
            ModifierKind::WeatherPenalty | ModifierKind::InputShortage | ModifierKind::Corruption => {
                1.0 - self.magnitude.abs().min(0.9)
            }
            _ => 1.0 + self.magnitude.abs().min(1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Supply Chain Event
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupplyChainEventKind {
    InputShortage,
    OutputBacklog,
    WorkerStrike,
    EquipmentFailure,
    RouteInterruption,
    QuotaShortfall,
    ReserveDepletion,
}

#[derive(Debug, Clone)]
pub struct SupplyChainEvent {
    pub kind: SupplyChainEventKind,
    pub building_or_node: u32,
    pub commodity: Option<CommodityRef>,
    pub severity: f64,
    pub tick: u64,
    pub description: String,
    pub resolved: bool,
}

// ---------------------------------------------------------------------------
// Stockpile
// ---------------------------------------------------------------------------

/// A multi-commodity stockpile, used by processing buildings to hold inputs and outputs.
#[derive(Debug, Clone)]
pub struct Stockpile {
    pub owner: BuildingId,
    /// Map of commodity -> current amount.
    pub contents: HashMap<CommodityRef, f64>,
    /// Per-commodity capacity limits (None = unlimited).
    pub capacities: HashMap<CommodityRef, f64>,
    /// Per-commodity reserved amounts (reserved for a production batch in progress).
    pub reserved: HashMap<CommodityRef, f64>,
    /// Total weight/volume capacity (0 = no limit).
    pub total_capacity: f64,
    /// Whether to auto-reorder when an input falls below threshold.
    pub auto_reorder: bool,
    /// Reorder threshold per commodity (fraction of capacity).
    pub reorder_threshold: HashMap<CommodityRef, f64>,
}

impl Stockpile {
    pub fn new(owner: BuildingId, total_capacity: f64) -> Self {
        Self {
            owner,
            contents: HashMap::new(),
            capacities: HashMap::new(),
            reserved: HashMap::new(),
            total_capacity,
            auto_reorder: true,
            reorder_threshold: HashMap::new(),
        }
    }

    pub fn amount(&self, commodity: CommodityRef) -> f64 {
        self.contents.get(&commodity).copied().unwrap_or(0.0)
    }

    pub fn available(&self, commodity: CommodityRef) -> f64 {
        let total = self.amount(commodity);
        let res = self.reserved.get(&commodity).copied().unwrap_or(0.0);
        (total - res).max(0.0)
    }

    /// Deposit `amount` of commodity. Returns amount actually accepted.
    pub fn deposit(&mut self, commodity: CommodityRef, amount: f64) -> f64 {
        let cap = self.capacities.get(&commodity).copied();
        let current = self.amount(commodity);
        let accept = match cap {
            Some(c) => (c - current).max(0.0).min(amount),
            None => amount,
        };
        *self.contents.entry(commodity).or_insert(0.0) += accept;
        accept
    }

    /// Withdraw `amount` of commodity. Returns amount taken.
    pub fn withdraw(&mut self, commodity: CommodityRef, amount: f64) -> f64 {
        let current = self.amount(commodity);
        let taken = current.min(amount);
        *self.contents.entry(commodity).or_insert(0.0) -= taken;
        taken
    }

    /// Reserve commodity for a batch in progress.
    pub fn reserve(&mut self, commodity: CommodityRef, amount: f64) -> bool {
        if self.available(commodity) < amount { return false; }
        *self.reserved.entry(commodity).or_insert(0.0) += amount;
        true
    }

    /// Consume reserved commodity (remove from both reserved and contents).
    pub fn consume_reserved(&mut self, commodity: CommodityRef, amount: f64) {
        let res = self.reserved.entry(commodity).or_insert(0.0);
        *res = (*res - amount).max(0.0);
        let cnt = self.contents.entry(commodity).or_insert(0.0);
        *cnt = (*cnt - amount).max(0.0);
    }

    pub fn total_items(&self) -> f64 {
        self.contents.values().sum()
    }

    /// Check which commodities are below their reorder threshold.
    pub fn reorder_needed(&self) -> Vec<CommodityRef> {
        if !self.auto_reorder { return Vec::new(); }
        self.reorder_threshold.iter().filter_map(|(&com, &thresh)| {
            let cap = self.capacities.get(&com).copied().unwrap_or(1000.0);
            let current = self.amount(com);
            if current < cap * thresh { Some(com) } else { None }
        }).collect()
    }
}

// ---------------------------------------------------------------------------
// Production Batch (in-progress)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ProductionBatch {
    recipe_id: RecipeId,
    started_tick: u64,
    complete_tick: u64,
    worker_count: u32,
    energy_used: f64,
    efficiency_at_start: f64,
}

// ---------------------------------------------------------------------------
// Processing Building
// ---------------------------------------------------------------------------

/// A building that transforms input commodities into output commodities.
#[derive(Debug, Clone)]
pub struct ProcessingBuilding {
    pub id: BuildingId,
    pub name: String,
    /// Available recipes.
    pub recipes: Vec<RecipeId>,
    /// Currently running batch, if any.
    active_batch: Option<ProductionBatch>,
    /// Recipe queue: which recipe to run next.
    pub recipe_queue: VecDeque<RecipeId>,
    /// Input/output stockpile.
    pub stockpile: Stockpile,
    /// Workers assigned to this building.
    pub workers: Vec<WorkerId>,
    /// Max worker slots.
    pub worker_slots: u32,
    /// Efficiency modifiers currently in effect.
    pub modifiers: Vec<EfficiencyModifier>,
    /// Production quota.
    pub quota: Option<ProductionQuota>,
    /// Throughput per tick (units produced).
    pub throughput_history: VecDeque<f64>,
    /// Total lifetime output (per commodity).
    pub lifetime_output: HashMap<CommodityRef, f64>,
    /// Whether this building is operational.
    pub operational: bool,
    /// Ticks this building has been running.
    pub age_ticks: u64,
    /// Total batches completed.
    pub batches_completed: u64,
}

impl ProcessingBuilding {
    pub fn new(id: BuildingId, name: &str, worker_slots: u32, stockpile_capacity: f64) -> Self {
        Self {
            id,
            name: name.to_string(),
            recipes: Vec::new(),
            active_batch: None,
            recipe_queue: VecDeque::new(),
            stockpile: Stockpile::new(id, stockpile_capacity),
            workers: Vec::new(),
            worker_slots,
            modifiers: Vec::new(),
            quota: None,
            throughput_history: VecDeque::with_capacity(64),
            lifetime_output: HashMap::new(),
            operational: true,
            age_ticks: 0,
            batches_completed: 0,
        }
    }

    pub fn add_recipe(&mut self, recipe_id: RecipeId) {
        if !self.recipes.contains(&recipe_id) {
            self.recipes.push(recipe_id);
        }
    }

    pub fn assign_worker(&mut self, worker: WorkerId) -> bool {
        if self.workers.len() as u32 >= self.worker_slots { return false; }
        if self.workers.contains(&worker) { return false; }
        self.workers.push(worker);
        true
    }

    pub fn remove_worker(&mut self, worker: WorkerId) -> bool {
        if let Some(pos) = self.workers.iter().position(|&w| w == worker) {
            self.workers.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn worker_count(&self) -> u32 {
        self.workers.len() as u32
    }

    /// Composite efficiency from all active modifiers.
    pub fn composite_efficiency(&self) -> f64 {
        let base = 1.0f64;
        self.modifiers.iter().fold(base, |acc, m| acc * m.effective_multiplier()).clamp(0.05, 3.0)
    }

    pub fn add_modifier(&mut self, modifier: EfficiencyModifier) {
        self.modifiers.push(modifier);
    }

    pub fn remove_modifier(&mut self, kind: ModifierKind) {
        self.modifiers.retain(|m| m.kind != kind);
    }

    /// Tick modifiers: decrement durations and remove expired ones.
    fn tick_modifiers(&mut self) {
        for m in self.modifiers.iter_mut() {
            if m.duration > 0 { m.duration -= 1; }
        }
        self.modifiers.retain(|m| m.duration != 1); // keep permanent (0) and still-active
    }

    /// Attempt to start a new production batch using a recipe.
    /// Returns true if batch was started.
    fn start_batch(&mut self, recipe: &Recipe, current_tick: u64) -> bool {
        if self.active_batch.is_some() { return false; }
        if self.worker_count() < recipe.workers_required { return false; }
        // Check and reserve inputs
        let available: HashMap<CommodityRef, f64> = self.stockpile.contents.clone();
        if !recipe.can_run(&available) { return false; }
        // Reserve inputs
        for ing in &recipe.inputs {
            if !self.stockpile.reserve(ing.commodity, ing.amount) { return false; }
        }
        let eff = self.composite_efficiency();
        let actual_ticks = (recipe.base_ticks as f64 / eff).round().max(1.0) as u64;
        self.active_batch = Some(ProductionBatch {
            recipe_id: recipe.id,
            started_tick: current_tick,
            complete_tick: current_tick + actual_ticks,
            worker_count: recipe.workers_required,
            energy_used: recipe.energy_required,
            efficiency_at_start: eff,
        });
        true
    }

    /// Complete a batch: consume reserved inputs and produce outputs.
    fn complete_batch(&mut self, recipe: &Recipe) -> HashMap<CommodityRef, f64> {
        let batch = match self.active_batch.take() {
            Some(b) => b,
            None => return HashMap::new(),
        };
        // Consume reserved inputs
        for ing in &recipe.inputs {
            self.stockpile.consume_reserved(ing.commodity, ing.amount);
        }
        // Produce outputs scaled by efficiency
        let mut produced: HashMap<CommodityRef, f64> = HashMap::new();
        for ing in &recipe.outputs {
            let amount = ing.amount * batch.efficiency_at_start;
            self.stockpile.deposit(ing.commodity, amount);
            *produced.entry(ing.commodity).or_insert(0.0) += amount;
            *self.lifetime_output.entry(ing.commodity).or_insert(0.0) += amount;
        }
        self.batches_completed += 1;
        produced
    }

    /// Full tick: advance modifier timers, check batch completion, start new batch.
    pub fn tick(
        &mut self,
        recipes: &HashMap<RecipeId, Recipe>,
        current_tick: u64,
    ) -> (HashMap<CommodityRef, f64>, Vec<SupplyChainEvent>) {
        self.age_ticks += 1;
        self.tick_modifiers();

        if !self.operational {
            self.throughput_history.push_back(0.0);
            if self.throughput_history.len() > 64 { self.throughput_history.pop_front(); }
            return (HashMap::new(), Vec::new());
        }

        let mut produced: HashMap<CommodityRef, f64> = HashMap::new();
        let mut events: Vec<SupplyChainEvent> = Vec::new();

        // Check batch completion
        let batch_complete = self.active_batch.as_ref()
            .map(|b| current_tick >= b.complete_tick)
            .unwrap_or(false);

        if batch_complete {
            if let Some(batch) = &self.active_batch {
                let rid = batch.recipe_id;
                if let Some(recipe) = recipes.get(&rid) {
                    let batch_recipe = recipe.clone();
                    produced = self.complete_batch(&batch_recipe);
                }
            }
        }

        // Try to start next batch
        if self.active_batch.is_none() {
            let next_recipe_id = self.recipe_queue.front().copied()
                .or_else(|| self.recipes.first().copied());

            if let Some(rid) = next_recipe_id {
                if let Some(recipe) = recipes.get(&rid) {
                    let recipe_clone = recipe.clone();
                    if !self.start_batch(&recipe_clone, current_tick) {
                        // Could not start: check why
                        let available: HashMap<CommodityRef, f64> = self.stockpile.contents.clone();
                        if !recipe_clone.can_run(&available) {
                            for ing in &recipe_clone.inputs {
                                let have = available.get(&ing.commodity).copied().unwrap_or(0.0);
                                if have < ing.amount {
                                    events.push(SupplyChainEvent {
                                        kind: SupplyChainEventKind::InputShortage,
                                        building_or_node: self.id.0,
                                        commodity: Some(ing.commodity),
                                        severity: 1.0 - have / ing.amount,
                                        tick: current_tick,
                                        description: format!(
                                            "Building {:?}: needs {:.1} of {:?}, has {:.1}",
                                            self.id, ing.amount, ing.commodity, have
                                        ),
                                        resolved: false,
                                    });
                                }
                            }
                        }
                        if self.worker_count() < recipe_clone.workers_required {
                            events.push(SupplyChainEvent {
                                kind: SupplyChainEventKind::WorkerStrike,
                                building_or_node: self.id.0,
                                commodity: None,
                                severity: 1.0 - self.worker_count() as f64 / recipe_clone.workers_required as f64,
                                tick: current_tick,
                                description: format!(
                                    "Building {:?}: needs {} workers, has {}",
                                    self.id, recipe_clone.workers_required, self.worker_count()
                                ),
                                resolved: false,
                            });
                        }
                    } else if !self.recipe_queue.is_empty() {
                        self.recipe_queue.pop_front();
                    }
                }
            }
        }

        // Check stockpile backlog
        if self.stockpile.total_capacity > 0.0
            && self.stockpile.total_items() > self.stockpile.total_capacity * 0.95
        {
            events.push(SupplyChainEvent {
                kind: SupplyChainEventKind::OutputBacklog,
                building_or_node: self.id.0,
                commodity: None,
                severity: 0.5,
                tick: current_tick,
                description: format!("Building {:?}: stockpile near capacity", self.id),
                resolved: false,
            });
        }

        // Update quota
        let total_produced: f64 = produced.values().sum();
        if let Some(ref mut quota) = self.quota {
            quota.update_fulfillment(total_produced);
            if quota.mandatory && quota.fulfillment < 0.8 {
                events.push(SupplyChainEvent {
                    kind: SupplyChainEventKind::QuotaShortfall,
                    building_or_node: self.id.0,
                    commodity: Some(quota.commodity),
                    severity: 1.0 - quota.fulfillment,
                    tick: current_tick,
                    description: format!(
                        "Building {:?}: quota shortfall {:.0}%",
                        self.id, quota.fulfillment * 100.0
                    ),
                    resolved: false,
                });
            }
        }

        self.throughput_history.push_back(total_produced);
        if self.throughput_history.len() > 64 { self.throughput_history.pop_front(); }

        (produced, events)
    }

    pub fn avg_throughput(&self, n: usize) -> f64 {
        let slice: Vec<_> = self.throughput_history.iter().rev().take(n).collect();
        if slice.is_empty() { return 0.0; }
        slice.iter().copied().sum::<f64>() / slice.len() as f64
    }

    /// Halt the building (equipment failure, strike, etc.).
    pub fn halt(&mut self, reason: &str, tick: u64) -> SupplyChainEvent {
        self.operational = false;
        // Release any reserved stock
        let reserved: Vec<(CommodityRef, f64)> = self.stockpile.reserved
            .iter().map(|(&k, &v)| (k, v)).collect();
        for (com, _) in reserved {
            self.stockpile.reserved.insert(com, 0.0);
        }
        self.active_batch = None;
        SupplyChainEvent {
            kind: SupplyChainEventKind::EquipmentFailure,
            building_or_node: self.id.0,
            commodity: None,
            severity: 1.0,
            tick,
            description: reason.to_string(),
            resolved: false,
        }
    }

    /// Resume halted building.
    pub fn resume(&mut self) {
        self.operational = true;
    }
}

// ---------------------------------------------------------------------------
// Worker Registry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    Idle,
    Assigned,
    Striking,
    Injured,
}

#[derive(Debug, Clone)]
pub struct Worker {
    pub id: WorkerId,
    pub skill_level: f64,
    pub status: WorkerStatus,
    pub assigned_building: Option<BuildingId>,
    pub assigned_node: Option<NodeId>,
    pub fatigue: f64,
    pub lifetime_ticks_worked: u64,
}

impl Worker {
    pub fn new(id: WorkerId, skill_level: f64) -> Self {
        Self {
            id,
            skill_level,
            status: WorkerStatus::Idle,
            assigned_building: None,
            assigned_node: None,
            fatigue: 0.0,
            lifetime_ticks_worked: 0,
        }
    }

    /// Efficiency contribution of this worker based on skill and fatigue.
    pub fn effective_skill(&self) -> f64 {
        self.skill_level * (1.0 - self.fatigue * 0.5)
    }

    pub fn tick(&mut self) {
        if self.status == WorkerStatus::Assigned {
            self.fatigue = (self.fatigue + 0.002).min(1.0);
            self.lifetime_ticks_worked += 1;
        } else {
            // Rest
            self.fatigue = (self.fatigue - 0.01).max(0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Production Report
// ---------------------------------------------------------------------------

/// Summary of one production tick.
#[derive(Debug, Clone)]
pub struct ProductionReport {
    pub tick: u64,
    /// Commodity -> total units produced this tick.
    pub total_produced: HashMap<CommodityRef, f64>,
    /// Commodity -> total units extracted from nodes.
    pub total_extracted: HashMap<CommodityRef, f64>,
    /// All supply chain events raised this tick.
    pub events: Vec<SupplyChainEvent>,
    /// Number of buildings operational.
    pub buildings_operational: u32,
    /// Number of nodes active.
    pub nodes_active: u32,
    /// Total workers assigned.
    pub workers_assigned: u32,
}

// ---------------------------------------------------------------------------
// ProductionManager — top-level coordinator
// ---------------------------------------------------------------------------

/// Manages all resource nodes, processing buildings, workers, recipes,
/// and coordinates supply chain logic.
pub struct ProductionManager {
    next_node_id: u32,
    next_building_id: u32,
    next_worker_id: u32,
    next_recipe_id: u32,
    pub current_tick: u64,

    pub nodes: HashMap<NodeId, ResourceNode>,
    pub buildings: HashMap<BuildingId, ProcessingBuilding>,
    pub workers: HashMap<WorkerId, Worker>,
    pub recipes: HashMap<RecipeId, Recipe>,

    /// Accumulated supply chain events for this manager's lifetime.
    pub supply_chain_events: Vec<SupplyChainEvent>,

    /// Simple xorshift64 RNG for yield variance.
    rng_state: u64,

    /// Global production history (last 128 ticks).
    pub production_history: VecDeque<ProductionReport>,
}

impl ProductionManager {
    pub fn new() -> Self {
        Self {
            next_node_id: 1,
            next_building_id: 1,
            next_worker_id: 1,
            next_recipe_id: 1,
            current_tick: 0,
            nodes: HashMap::new(),
            buildings: HashMap::new(),
            workers: HashMap::new(),
            recipes: HashMap::new(),
            supply_chain_events: Vec::new(),
            rng_state: 0x1234_ABCD_5678_EF00,
            production_history: VecDeque::with_capacity(128),
        }
    }

    fn next_rand(&mut self) -> f64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        (x as f64) / (u64::MAX as f64)
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    pub fn add_node(
        &mut self,
        name: &str,
        kind: NodeKind,
        output_commodity: CommodityRef,
        base_yield: f64,
        reserves: Option<f64>,
        worker_capacity: u32,
        stockpile_capacity: f64,
    ) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        self.nodes.insert(id, ResourceNode::new(id, name, kind, output_commodity, base_yield, reserves, worker_capacity, stockpile_capacity));
        id
    }

    pub fn add_building(
        &mut self,
        name: &str,
        worker_slots: u32,
        stockpile_capacity: f64,
    ) -> BuildingId {
        let id = BuildingId(self.next_building_id);
        self.next_building_id += 1;
        self.buildings.insert(id, ProcessingBuilding::new(id, name, worker_slots, stockpile_capacity));
        id
    }

    pub fn add_recipe(
        &mut self,
        name: &str,
        inputs: Vec<Ingredient>,
        outputs: Vec<Ingredient>,
        base_ticks: u64,
        workers_required: u32,
        energy_required: f64,
    ) -> RecipeId {
        let id = RecipeId(self.next_recipe_id);
        self.next_recipe_id += 1;
        self.recipes.insert(id, Recipe { id, name: name.to_string(), inputs, outputs, base_ticks, workers_required, energy_required });
        id
    }

    pub fn add_worker(&mut self, skill_level: f64) -> WorkerId {
        let id = WorkerId(self.next_worker_id);
        self.next_worker_id += 1;
        self.workers.insert(id, Worker::new(id, skill_level));
        id
    }

    // -----------------------------------------------------------------------
    // Assignment
    // -----------------------------------------------------------------------

    pub fn assign_worker_to_building(&mut self, worker: WorkerId, building: BuildingId) -> bool {
        if let Some(b) = self.buildings.get_mut(&building) {
            if b.assign_worker(worker) {
                if let Some(w) = self.workers.get_mut(&worker) {
                    w.status = WorkerStatus::Assigned;
                    w.assigned_building = Some(building);
                }
                return true;
            }
        }
        false
    }

    pub fn assign_worker_to_node(&mut self, worker: WorkerId, node: NodeId) -> bool {
        if let Some(n) = self.nodes.get_mut(&node) {
            if n.assign_workers(1) == 1 {
                if let Some(w) = self.workers.get_mut(&worker) {
                    w.status = WorkerStatus::Assigned;
                    w.assigned_node = Some(node);
                }
                return true;
            }
        }
        false
    }

    pub fn assign_recipe_to_building(&mut self, building: BuildingId, recipe: RecipeId) {
        if let Some(b) = self.buildings.get_mut(&building) {
            b.add_recipe(recipe);
        }
    }

    pub fn queue_recipe(&mut self, building: BuildingId, recipe: RecipeId) {
        if let Some(b) = self.buildings.get_mut(&building) {
            b.recipe_queue.push_back(recipe);
        }
    }

    // -----------------------------------------------------------------------
    // Supply Transfer
    // -----------------------------------------------------------------------

    /// Transfer output from a node's stockpile to a building's input stockpile.
    pub fn transfer_node_to_building(
        &mut self,
        node: NodeId,
        building: BuildingId,
        commodity: CommodityRef,
        max_amount: f64,
    ) -> f64 {
        let taken = match self.nodes.get_mut(&node) {
            Some(n) => n.withdraw(max_amount),
            None => return 0.0,
        };
        if taken > 0.0 {
            if let Some(b) = self.buildings.get_mut(&building) {
                let accepted = b.stockpile.deposit(commodity, taken);
                let _ = accepted;
            }
        }
        taken
    }

    /// Withdraw output from a building's stockpile (for downstream delivery).
    pub fn withdraw_from_building(
        &mut self,
        building: BuildingId,
        commodity: CommodityRef,
        amount: f64,
    ) -> f64 {
        self.buildings.get_mut(&building)
            .map(|b| b.stockpile.withdraw(commodity, amount))
            .unwrap_or(0.0)
    }

    // -----------------------------------------------------------------------
    // Disruption
    // -----------------------------------------------------------------------

    pub fn disrupt_node(&mut self, node: NodeId, severity: f64) {
        if let Some(n) = self.nodes.get_mut(&node) {
            n.apply_disruption(severity);
        }
    }

    pub fn disrupt_building(&mut self, building: BuildingId, reason: &str) {
        let tick = self.current_tick;
        if let Some(b) = self.buildings.get_mut(&building) {
            let evt = b.halt(reason, tick);
            self.supply_chain_events.push(evt);
        }
    }

    pub fn repair_building(&mut self, building: BuildingId) {
        if let Some(b) = self.buildings.get_mut(&building) {
            b.resume();
        }
    }

    pub fn maintain_node(&mut self, node: NodeId) {
        if let Some(n) = self.nodes.get_mut(&node) {
            n.perform_maintenance();
        }
    }

    // -----------------------------------------------------------------------
    // Season Update
    // -----------------------------------------------------------------------

    /// Set season modifier for all nodes of a given kind.
    pub fn set_season(&mut self, kind: NodeKind, modifier: f64) {
        for n in self.nodes.values_mut() {
            if n.kind == kind {
                n.season_modifier = modifier.clamp(0.0, 2.0);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Main Tick
    // -----------------------------------------------------------------------

    /// Advance production by one tick.
    ///
    /// - Ticks all workers (fatigue).
    /// - Ticks all resource nodes (yield, depletion, disruption recovery).
    /// - Ticks all processing buildings (batch progress, input matching).
    /// - Accumulates supply chain events.
    /// - Produces a ProductionReport.
    pub fn tick(&mut self) -> ProductionReport {
        self.current_tick += 1;
        let tick = self.current_tick;

        // --- Workers ---
        for w in self.workers.values_mut() {
            w.tick();
        }

        let mut total_produced: HashMap<CommodityRef, f64> = HashMap::new();
        let mut total_extracted: HashMap<CommodityRef, f64> = HashMap::new();
        let mut all_events: Vec<SupplyChainEvent> = Vec::new();

        // --- Resource Nodes ---
        let node_ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        let mut nodes_active = 0u32;
        for &nid in &node_ids {
            let rng_v = self.next_rand();
            let node = self.nodes.get_mut(&nid).unwrap();
            if node.active { nodes_active += 1; }
            let output = node.tick_output(rng_v);
            *total_extracted.entry(node.output_commodity).or_insert(0.0) += output;
            node.recover_disruption(0.02);
            // Emit reserve depletion event if nearly depleted
            if let Some(frac) = node.depletion_fraction() {
                if frac > 0.90 && output > 0.0 {
                    all_events.push(SupplyChainEvent {
                        kind: SupplyChainEventKind::ReserveDepletion,
                        building_or_node: nid.0,
                        commodity: Some(node.output_commodity),
                        severity: frac,
                        tick,
                        description: format!("Node {:?} reserves at {:.1}% depletion", nid, frac * 100.0),
                        resolved: false,
                    });
                }
            }
        }

        // --- Processing Buildings ---
        let building_ids: Vec<BuildingId> = self.buildings.keys().copied().collect();
        let mut buildings_operational = 0u32;
        for &bid in &building_ids {
            // Borrow recipes separately
            let recipes_snapshot: HashMap<RecipeId, Recipe> = self.recipes.clone();
            let building = self.buildings.get_mut(&bid).unwrap();
            if building.operational { buildings_operational += 1; }
            let (produced, events) = building.tick(&recipes_snapshot, tick);
            for (com, amt) in produced {
                *total_produced.entry(com).or_insert(0.0) += amt;
            }
            all_events.extend(events);
        }

        // Count workers assigned
        let workers_assigned = self.workers.values()
            .filter(|w| w.status == WorkerStatus::Assigned)
            .count() as u32;

        // Accumulate global events
        self.supply_chain_events.extend(all_events.iter().cloned());
        // Keep last 512 events
        if self.supply_chain_events.len() > 512 {
            let drain = self.supply_chain_events.len() - 512;
            self.supply_chain_events.drain(0..drain);
        }

        let report = ProductionReport {
            tick,
            total_produced,
            total_extracted,
            events: all_events,
            buildings_operational,
            nodes_active,
            workers_assigned,
        };

        self.production_history.push_back(report.clone());
        if self.production_history.len() > 128 { self.production_history.pop_front(); }
        report
    }

    // -----------------------------------------------------------------------
    // Query Helpers
    // -----------------------------------------------------------------------

    /// Total current stockpile across all nodes for a commodity.
    pub fn node_stockpile(&self, commodity: CommodityRef) -> f64 {
        self.nodes.values()
            .filter(|n| n.output_commodity == commodity)
            .map(|n| n.stockpile)
            .sum()
    }

    /// Total current stockpile in buildings for a commodity (output side).
    pub fn building_stockpile(&self, commodity: CommodityRef) -> f64 {
        self.buildings.values()
            .map(|b| b.stockpile.amount(commodity))
            .sum()
    }

    /// All unresolved supply chain events.
    pub fn unresolved_events(&self) -> Vec<&SupplyChainEvent> {
        self.supply_chain_events.iter().filter(|e| !e.resolved).collect()
    }

    /// Resolve an event by index.
    pub fn resolve_event(&mut self, index: usize) {
        if let Some(e) = self.supply_chain_events.get_mut(index) {
            e.resolved = true;
        }
    }

    /// Summary of average throughput across last n ticks per commodity.
    pub fn avg_throughput(&self, commodity: CommodityRef, n: usize) -> f64 {
        let samples: Vec<f64> = self.production_history.iter().rev().take(n)
            .map(|r| r.total_produced.get(&commodity).copied().unwrap_or(0.0))
            .collect();
        if samples.is_empty() { return 0.0; }
        samples.iter().sum::<f64>() / samples.len() as f64
    }

    /// Find nodes near depletion.
    pub fn near_depletion_nodes(&self, threshold: f64) -> Vec<NodeId> {
        self.nodes.values()
            .filter(|n| n.depletion_fraction().map(|f| f > threshold).unwrap_or(false))
            .map(|n| n.id)
            .collect()
    }

    /// Idle workers.
    pub fn idle_workers(&self) -> Vec<WorkerId> {
        self.workers.values()
            .filter(|w| w.status == WorkerStatus::Idle)
            .map(|w| w.id)
            .collect()
    }

    /// Buildings with open worker slots.
    pub fn understaffed_buildings(&self) -> Vec<BuildingId> {
        self.buildings.values()
            .filter(|b| b.worker_count() < b.worker_slots)
            .map(|b| b.id)
            .collect()
    }

    /// Auto-assign idle workers to understaffed buildings (greedy).
    pub fn auto_assign_workers(&mut self) {
        let idle: Vec<WorkerId> = self.idle_workers();
        let understaffed: Vec<BuildingId> = self.understaffed_buildings();
        let mut worker_iter = idle.into_iter();
        for bid in understaffed {
            let slots_needed = {
                let b = &self.buildings[&bid];
                b.worker_slots - b.worker_count()
            };
            for _ in 0..slots_needed {
                match worker_iter.next() {
                    Some(wid) => { self.assign_worker_to_building(wid, bid); }
                    None => return,
                }
            }
        }
    }
}

impl Default for ProductionManager {
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
    fn test_node_extraction() {
        let mut pm = ProductionManager::new();
        let iron = CommodityRef(1);
        let nid = pm.add_node("Iron Mine", NodeKind::Mine, iron, 10.0, Some(100_000.0), 10, 5000.0);
        // Assign 5 workers
        for _ in 0..5 {
            let wid = pm.add_worker(1.0);
            pm.assign_worker_to_node(wid, nid);
        }
        let report = pm.tick();
        let extracted = report.total_extracted.get(&iron).copied().unwrap_or(0.0);
        assert!(extracted > 0.0, "mine should produce output: {}", extracted);
    }

    #[test]
    fn test_building_recipe_completion() {
        let mut pm = ProductionManager::new();
        let ore = CommodityRef(1);
        let steel = CommodityRef(2);
        let bid = pm.add_building("Smelter", 4, 10000.0);
        let rid = pm.add_recipe(
            "Smelt Iron",
            vec![Ingredient { commodity: ore, amount: 10.0 }],
            vec![Ingredient { commodity: steel, amount: 5.0 }],
            3,
            2,
            0.0,
        );
        pm.assign_recipe_to_building(bid, rid);
        // Add workers
        for _ in 0..2 {
            let wid = pm.add_worker(1.0);
            pm.assign_worker_to_building(wid, bid);
        }
        // Pre-load ore into stockpile
        pm.buildings.get_mut(&bid).unwrap().stockpile.deposit(ore, 50.0);
        // Tick enough times for the batch to complete (base_ticks=3)
        for _ in 0..5 {
            pm.tick();
        }
        let steel_in_stockpile = pm.buildings[&bid].stockpile.amount(steel);
        assert!(steel_in_stockpile > 0.0, "smelter should have produced steel: {}", steel_in_stockpile);
    }

    #[test]
    fn test_supply_chain_input_shortage() {
        let mut pm = ProductionManager::new();
        let ore = CommodityRef(1);
        let steel = CommodityRef(2);
        let bid = pm.add_building("Smelter", 4, 10000.0);
        let rid = pm.add_recipe(
            "Smelt Iron",
            vec![Ingredient { commodity: ore, amount: 100.0 }],
            vec![Ingredient { commodity: steel, amount: 50.0 }],
            2,
            1,
            0.0,
        );
        pm.assign_recipe_to_building(bid, rid);
        let wid = pm.add_worker(1.0);
        pm.assign_worker_to_building(wid, bid);
        // No ore in stockpile -> shortage event
        let report = pm.tick();
        let has_shortage = report.events.iter().any(|e| e.kind == SupplyChainEventKind::InputShortage);
        assert!(has_shortage, "should report input shortage");
    }

    #[test]
    fn test_depletion_event() {
        let mut pm = ProductionManager::new();
        let coal = CommodityRef(3);
        let nid = pm.add_node("Coal Mine", NodeKind::Mine, coal, 1000.0, Some(10.0), 5, 50000.0);
        for _ in 0..5 {
            let wid = pm.add_worker(1.0);
            pm.assign_worker_to_node(wid, nid);
        }
        // Manually deplete reserves
        pm.nodes.get_mut(&nid).unwrap().reserves = Some(0.5);
        let report = pm.tick();
        let depleted = report.events.iter().any(|e| e.kind == SupplyChainEventKind::ReserveDepletion);
        assert!(depleted, "near-empty reserves should trigger depletion event");
    }

    #[test]
    fn test_maintenance_restores_efficiency() {
        let mut pm = ProductionManager::new();
        let gold = CommodityRef(4);
        let nid = pm.add_node("Gold Mine", NodeKind::Mine, gold, 5.0, Some(1_000_000.0), 3, 1000.0);
        let node = pm.nodes.get_mut(&nid).unwrap();
        node.efficiency = 0.4;
        node.perform_maintenance();
        assert!(node.efficiency > 0.6, "maintenance should restore efficiency");
    }

    #[test]
    fn test_worker_fatigue() {
        let mut pm = ProductionManager::new();
        let wood = CommodityRef(5);
        let nid = pm.add_node("Forest", NodeKind::Forest, wood, 8.0, None, 10, 5000.0);
        let wid = pm.add_worker(1.0);
        pm.assign_worker_to_node(wid, nid);
        for _ in 0..100 { pm.tick(); }
        let worker = &pm.workers[&wid];
        assert!(worker.fatigue > 0.1, "worker should accumulate fatigue: {}", worker.fatigue);
    }

    #[test]
    fn test_auto_assign_workers() {
        let mut pm = ProductionManager::new();
        let bid = pm.add_building("Workshop", 3, 1000.0);
        for _ in 0..3 { pm.add_worker(1.0); }
        pm.auto_assign_workers();
        let b = &pm.buildings[&bid];
        assert_eq!(b.worker_count(), 3);
    }

    #[test]
    fn test_season_modifier() {
        let mut pm = ProductionManager::new();
        let grain = CommodityRef(6);
        let nid = pm.add_node("Farm", NodeKind::Farm, grain, 20.0, None, 5, 2000.0);
        for _ in 0..5 {
            let wid = pm.add_worker(1.0);
            pm.assign_worker_to_node(wid, nid);
        }
        pm.set_season(NodeKind::Farm, 0.0); // winter
        let report = pm.tick();
        let extracted = report.total_extracted.get(&grain).copied().unwrap_or(0.0);
        assert_eq!(extracted, 0.0, "winter should halt farm output");
    }

    #[test]
    fn test_node_to_building_transfer() {
        let mut pm = ProductionManager::new();
        let ore = CommodityRef(1);
        let nid = pm.add_node("Mine", NodeKind::Mine, ore, 10.0, Some(10_000.0), 5, 1000.0);
        let bid = pm.add_building("Smelter", 2, 5000.0);
        // Put ore directly into node stockpile
        pm.nodes.get_mut(&nid).unwrap().stockpile = 200.0;
        let transferred = pm.transfer_node_to_building(nid, bid, ore, 100.0);
        assert!((transferred - 100.0).abs() < 1e-9);
        assert!((pm.buildings[&bid].stockpile.amount(ore) - 100.0).abs() < 1e-9);
    }
}
