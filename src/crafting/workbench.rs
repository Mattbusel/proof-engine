// crafting/workbench.rs — Crafting station and job queue system

use std::collections::HashMap;
use glam::Vec3;
use crate::crafting::recipes::{Recipe, CraftResult, CraftingCalculator};

// ---------------------------------------------------------------------------
// WorkbenchType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorkbenchType {
    Forge,
    AlchemyTable,
    CookingPot,
    EnchantingTable,
    Workbench,
    Loom,
    Jeweler,
}

impl WorkbenchType {
    pub fn label(&self) -> &'static str {
        match self {
            WorkbenchType::Forge          => "Forge",
            WorkbenchType::AlchemyTable   => "Alchemy Table",
            WorkbenchType::CookingPot     => "Cooking Pot",
            WorkbenchType::EnchantingTable => "Enchanting Table",
            WorkbenchType::Workbench      => "Workbench",
            WorkbenchType::Loom           => "Loom",
            WorkbenchType::Jeweler        => "Jeweler",
        }
    }

    /// Whether this bench type consumes fuel.
    pub fn requires_fuel(&self) -> bool {
        matches!(
            self,
            WorkbenchType::Forge | WorkbenchType::CookingPot
        )
    }

    /// Base quality bonus granted by this bench type.
    pub fn base_quality_bonus(&self) -> u32 {
        match self {
            WorkbenchType::Forge          => 5,
            WorkbenchType::AlchemyTable   => 8,
            WorkbenchType::CookingPot     => 3,
            WorkbenchType::EnchantingTable => 12,
            WorkbenchType::Workbench      => 4,
            WorkbenchType::Loom           => 4,
            WorkbenchType::Jeweler        => 10,
        }
    }
}

// ---------------------------------------------------------------------------
// WorkbenchTier
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkbenchTier {
    Basic,
    Improved,
    Advanced,
    Master,
}

impl WorkbenchTier {
    /// Quality bonus multiplier on top of WorkbenchType's base bonus.
    pub fn quality_multiplier(&self) -> f32 {
        match self {
            WorkbenchTier::Basic    => 1.0,
            WorkbenchTier::Improved => 1.25,
            WorkbenchTier::Advanced => 1.60,
            WorkbenchTier::Master   => 2.20,
        }
    }

    /// Speed multiplier (higher = faster crafting).
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            WorkbenchTier::Basic    => 1.0,
            WorkbenchTier::Improved => 1.15,
            WorkbenchTier::Advanced => 1.35,
            WorkbenchTier::Master   => 1.60,
        }
    }

    /// Cost in gold to upgrade to the next tier.
    pub fn upgrade_cost(&self) -> u64 {
        match self {
            WorkbenchTier::Basic    => 500,
            WorkbenchTier::Improved => 2000,
            WorkbenchTier::Advanced => 8000,
            WorkbenchTier::Master   => 0, // already max
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            WorkbenchTier::Basic    => "Basic",
            WorkbenchTier::Improved => "Improved",
            WorkbenchTier::Advanced => "Advanced",
            WorkbenchTier::Master   => "Master",
        }
    }
}

// ---------------------------------------------------------------------------
// WorkbenchState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum WorkbenchState {
    Idle,
    Crafting {
        job_id: u64,
        elapsed: f32,
        duration: f32,
    },
    Broken {
        repair_cost: u64,
    },
}

impl WorkbenchState {
    pub fn is_idle(&self) -> bool {
        matches!(self, WorkbenchState::Idle)
    }

    pub fn is_crafting(&self) -> bool {
        matches!(self, WorkbenchState::Crafting { .. })
    }

    pub fn is_broken(&self) -> bool {
        matches!(self, WorkbenchState::Broken { .. })
    }
}

// ---------------------------------------------------------------------------
// CraftingJob
// ---------------------------------------------------------------------------

/// A single crafting task in the queue.
#[derive(Debug, Clone)]
pub struct CraftingJob {
    pub id: u64,
    pub recipe_id: String,
    /// Ingredients consumed when this job started, as (item_id, quantity).
    pub ingredients_consumed: Vec<(String, u32)>,
    /// Timestamp (game time seconds) when this job started.
    pub started_at: f32,
    /// Total duration of this job in seconds.
    pub duration: f32,
    /// How many craft cycles to run (batch crafting).
    pub quantity: u32,
    /// Player/entity that owns this job.
    pub owner_id: String,
}

impl CraftingJob {
    pub fn new(
        id: u64,
        recipe_id: impl Into<String>,
        ingredients_consumed: Vec<(String, u32)>,
        started_at: f32,
        duration: f32,
        quantity: u32,
        owner_id: impl Into<String>,
    ) -> Self {
        Self {
            id,
            recipe_id: recipe_id.into(),
            ingredients_consumed,
            started_at,
            duration,
            quantity,
            owner_id: owner_id.into(),
        }
    }

    /// Elapsed progress as a fraction [0.0, 1.0].
    pub fn progress(&self, current_time: f32) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        ((current_time - self.started_at) / self.duration).clamp(0.0, 1.0)
    }

    /// Whether the job is complete given current time.
    pub fn is_complete(&self, current_time: f32) -> bool {
        current_time >= self.started_at + self.duration
    }
}

// ---------------------------------------------------------------------------
// CraftingQueue
// ---------------------------------------------------------------------------

const MAX_QUEUE_SLOTS: usize = 8;

/// Ordered pending job queue with a maximum of 8 slots.
#[derive(Debug, Clone)]
pub struct CraftingQueue {
    jobs: Vec<CraftingJob>,
    next_job_id: u64,
}

impl CraftingQueue {
    pub fn new() -> Self {
        Self {
            jobs: Vec::with_capacity(MAX_QUEUE_SLOTS),
            next_job_id: 1,
        }
    }

    /// Whether the queue has room for another job.
    pub fn has_capacity(&self) -> bool {
        self.jobs.len() < MAX_QUEUE_SLOTS
    }

    /// Number of jobs currently in the queue.
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// Enqueue a new job, returning its id or None if the queue is full.
    pub fn enqueue(
        &mut self,
        recipe_id: impl Into<String>,
        ingredients_consumed: Vec<(String, u32)>,
        started_at: f32,
        duration: f32,
        quantity: u32,
        owner_id: impl Into<String>,
    ) -> Option<u64> {
        if !self.has_capacity() {
            return None;
        }
        let id = self.next_job_id;
        self.next_job_id += 1;
        self.jobs.push(CraftingJob::new(
            id,
            recipe_id,
            ingredients_consumed,
            started_at,
            duration,
            quantity,
            owner_id,
        ));
        Some(id)
    }

    /// Peek at the front job without removing it.
    pub fn front(&self) -> Option<&CraftingJob> {
        self.jobs.first()
    }

    /// Dequeue the front job.
    pub fn dequeue(&mut self) -> Option<CraftingJob> {
        if self.jobs.is_empty() {
            None
        } else {
            Some(self.jobs.remove(0))
        }
    }

    /// Remove a job by id (e.g. on cancellation).
    pub fn cancel(&mut self, job_id: u64) -> Option<CraftingJob> {
        if let Some(pos) = self.jobs.iter().position(|j| j.id == job_id) {
            Some(self.jobs.remove(pos))
        } else {
            None
        }
    }

    /// All jobs currently in the queue.
    pub fn all_jobs(&self) -> &[CraftingJob] {
        &self.jobs
    }
}

impl Default for CraftingQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WorkbenchEvent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum WorkbenchEvent {
    JobStarted {
        job_id: u64,
        recipe_id: String,
    },
    JobCompleted {
        job_id: u64,
        results: Vec<(String, u32, u8)>, // (item_id, quantity, quality)
    },
    JobFailed {
        job_id: u64,
        reason: String,
    },
    FuelEmpty,
    RepairNeeded {
        repair_cost: u64,
    },
    QueueFull,
}

// ---------------------------------------------------------------------------
// FuelType and FuelSystem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FuelType {
    Coal,
    Wood,
    MagicCrystal,
}

impl FuelType {
    /// Burn rate in fuel units consumed per second of crafting.
    pub fn burn_rate(&self) -> f32 {
        match self {
            FuelType::Coal         => 0.5,
            FuelType::Wood         => 1.0,
            FuelType::MagicCrystal => 0.1,
        }
    }

    /// Extra heat bonus that increases quality when using this fuel.
    pub fn heat_quality_bonus(&self) -> u32 {
        match self {
            FuelType::Coal         => 3,
            FuelType::Wood         => 1,
            FuelType::MagicCrystal => 15,
        }
    }

    /// Fuel units per physical item of this type.
    pub fn fuel_value(&self) -> f32 {
        match self {
            FuelType::Coal         => 60.0,
            FuelType::Wood         => 20.0,
            FuelType::MagicCrystal => 300.0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FuelType::Coal         => "Coal",
            FuelType::Wood         => "Wood",
            FuelType::MagicCrystal => "Magic Crystal",
        }
    }
}

/// Manages fuel for a workbench that requires heat.
#[derive(Debug, Clone)]
pub struct FuelSystem {
    pub fuel_level: f32,
    pub max_fuel: f32,
    pub current_fuel_type: FuelType,
    pub is_burning: bool,
}

impl FuelSystem {
    pub fn new(max_fuel: f32) -> Self {
        Self {
            fuel_level: 0.0,
            max_fuel,
            current_fuel_type: FuelType::Coal,
            is_burning: false,
        }
    }

    /// Add fuel items of a given type.  Returns overflow (items that wouldn't fit).
    pub fn add_fuel(&mut self, fuel_type: FuelType, items: u32) -> u32 {
        let units = fuel_type.fuel_value() * items as f32;
        let available_space = self.max_fuel - self.fuel_level;
        if units <= available_space {
            self.fuel_level += units;
            self.current_fuel_type = fuel_type;
            0
        } else {
            self.fuel_level = self.max_fuel;
            let fuel_value = fuel_type.fuel_value();
            self.current_fuel_type = fuel_type;
            let overflow_units = units - available_space;
            let overflow_items = (overflow_units / fuel_value).ceil() as u32;
            overflow_items
        }
    }

    /// Consume fuel for `dt` seconds of crafting. Returns true if fuel remains.
    pub fn consume(&mut self, dt: f32) -> bool {
        if !self.is_burning {
            return true;
        }
        let consumed = self.current_fuel_type.burn_rate() * dt;
        if self.fuel_level >= consumed {
            self.fuel_level -= consumed;
            true
        } else {
            self.fuel_level = 0.0;
            self.is_burning = false;
            false
        }
    }

    /// Current heat quality bonus (0 if no fuel).
    pub fn heat_quality_bonus(&self) -> u32 {
        if self.is_burning && self.fuel_level > 0.0 {
            self.current_fuel_type.heat_quality_bonus()
        } else {
            0
        }
    }

    pub fn ignite(&mut self) {
        if self.fuel_level > 0.0 {
            self.is_burning = true;
        }
    }

    pub fn extinguish(&mut self) {
        self.is_burning = false;
    }

    pub fn is_empty(&self) -> bool {
        self.fuel_level <= 0.0
    }

    /// Fuel as a fraction of max [0.0, 1.0].
    pub fn level_fraction(&self) -> f32 {
        if self.max_fuel <= 0.0 {
            return 0.0;
        }
        (self.fuel_level / self.max_fuel).clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------------------
// Workbench
// ---------------------------------------------------------------------------

/// A single crafting station in the world.
#[derive(Debug, Clone)]
pub struct Workbench {
    pub id: u64,
    pub position: Vec3,
    pub bench_type: WorkbenchType,
    pub tier: WorkbenchTier,
    pub state: WorkbenchState,
    pub queue: CraftingQueue,
    pub fuel: FuelSystem,
    /// Overall efficiency multiplier (0.0–2.0), affected by repairs and upgrades.
    pub efficiency: f32,
    /// Accumulated wear (0.0–1.0). At 1.0 the bench breaks.
    pub wear: f32,
    /// Skill level assumed for quality calculations (e.g. owner's skill).
    pub operator_skill: u32,
    /// Pending events generated during tick().
    events_buffer: Vec<WorkbenchEvent>,
    /// Game time tracker (seconds since session start).
    current_time: f32,
}

impl Workbench {
    pub fn new(id: u64, position: Vec3, bench_type: WorkbenchType, tier: WorkbenchTier) -> Self {
        let needs_fuel = bench_type.requires_fuel();
        let max_fuel = if needs_fuel { 600.0 } else { 0.0 };
        Self {
            id,
            position,
            bench_type,
            tier,
            state: WorkbenchState::Idle,
            queue: CraftingQueue::new(),
            fuel: FuelSystem::new(max_fuel),
            efficiency: 1.0,
            wear: 0.0,
            operator_skill: 1,
            events_buffer: Vec::new(),
            current_time: 0.0,
        }
    }

    /// Try to start the next queued job.
    fn try_start_next_job(&mut self) {
        if !self.state.is_idle() {
            return;
        }
        if self.state.is_broken() {
            return;
        }
        if let Some(job) = self.queue.front() {
            let job_id = job.id;
            let duration = job.duration;
            let recipe_id = job.recipe_id.clone();
            self.state = WorkbenchState::Crafting {
                job_id,
                elapsed: 0.0,
                duration,
            };
            self.events_buffer.push(WorkbenchEvent::JobStarted { job_id, recipe_id });

            // Ignite fuel if needed
            if self.bench_type.requires_fuel() {
                self.fuel.ignite();
            }
        }
    }

    /// Advance the workbench by `dt` seconds.
    pub fn tick(&mut self, dt: f32) -> Vec<WorkbenchEvent> {
        self.events_buffer.clear();
        self.current_time += dt;

        if self.state.is_broken() {
            return self.events_buffer.clone();
        }

        // Try to start a job if idle and queue has items
        if self.state.is_idle() && !self.queue.is_empty() {
            self.try_start_next_job();
        }

        match self.state.clone() {
            WorkbenchState::Idle => {}
            WorkbenchState::Broken { .. } => {}
            WorkbenchState::Crafting { job_id, elapsed, duration } => {
                // Consume fuel if required
                if self.bench_type.requires_fuel() {
                    let fuel_ok = self.fuel.consume(dt);
                    if !fuel_ok {
                        // Out of fuel — pause the job (keep it in queue, go idle)
                        self.state = WorkbenchState::Idle;
                        self.events_buffer.push(WorkbenchEvent::FuelEmpty);
                        return self.events_buffer.clone();
                    }
                }

                let new_elapsed = (elapsed + dt * self.efficiency).min(duration);
                self.state = WorkbenchState::Crafting {
                    job_id,
                    elapsed: new_elapsed,
                    duration,
                };

                // Accumulate wear
                self.wear += dt * 0.0002;
                if self.wear >= 1.0 {
                    self.wear = 1.0;
                    let repair_cost = self.compute_repair_cost();
                    self.state = WorkbenchState::Broken { repair_cost };
                    self.events_buffer.push(WorkbenchEvent::RepairNeeded { repair_cost });
                    return self.events_buffer.clone();
                }

                // Check completion
                if new_elapsed >= duration {
                    // Dequeue the job
                    if let Some(job) = self.queue.dequeue() {
                        let results = self.compute_job_results(&job);
                        self.events_buffer.push(WorkbenchEvent::JobCompleted {
                            job_id: job.id,
                            results,
                        });
                    }
                    self.state = WorkbenchState::Idle;
                    // Immediately try to start the next queued job
                    self.try_start_next_job();
                }
            }
        }

        self.events_buffer.clone()
    }

    /// Compute output items for a completed job.
    fn compute_job_results(&self, job: &CraftingJob) -> Vec<(String, u32, u8)> {
        let quality_bonus = self.tier_quality_bonus();
        let fuel_bonus = self.fuel.heat_quality_bonus();
        let mut results = Vec::new();

        // For simplicity we generate results based on job.quantity iterations
        for _ in 0..job.quantity {
            // Base quality per result is encoded in the recipe; we use a default of 80
            let base_quality: u8 = 80;
            let computed_quality = CraftingCalculator::calculate_quality(
                base_quality,
                self.operator_skill,
                quality_bonus + fuel_bonus,
            );
            let item_id = format!("{}_product", job.recipe_id);
            results.push((item_id, 1, computed_quality));
        }
        results
    }

    /// Compute results for a job using a known recipe.
    pub fn compute_results_for_recipe(
        &self,
        recipe: &Recipe,
        quantity: u32,
        rng_values: &[f32],
    ) -> Vec<(String, u32, u8)> {
        let quality_bonus = self.tier_quality_bonus();
        let fuel_bonus = self.fuel.heat_quality_bonus();
        let mut results = Vec::new();
        let mut rng_idx = 0;

        for _ in 0..quantity {
            for craft_result in &recipe.results {
                let rng = rng_values.get(rng_idx).copied().unwrap_or(0.5);
                rng_idx += 1;

                if CraftingCalculator::evaluate_chance(craft_result, self.operator_skill, rng) {
                    let computed_quality = CraftingCalculator::calculate_quality(
                        craft_result.quality,
                        self.operator_skill,
                        quality_bonus + fuel_bonus,
                    );
                    let computed_quantity = CraftingCalculator::calculate_quantity(
                        craft_result.quantity,
                        self.operator_skill,
                        0.0,
                    );
                    results.push((craft_result.item_id.clone(), computed_quantity, computed_quality));
                }
            }
        }
        results
    }

    /// Effective quality bonus considering tier and bench type.
    pub fn tier_quality_bonus(&self) -> u32 {
        let base = self.bench_type.base_quality_bonus();
        let multiplied = base as f32 * self.tier.quality_multiplier();
        multiplied.round() as u32
    }

    /// Submit a new crafting job to the queue.
    ///
    /// Returns the job_id on success, or an event explaining why it failed.
    pub fn submit_job(
        &mut self,
        recipe_id: impl Into<String>,
        ingredients: Vec<(String, u32)>,
        duration: f32,
        quantity: u32,
        owner_id: impl Into<String>,
    ) -> Result<u64, WorkbenchEvent> {
        if self.state.is_broken() {
            let repair_cost = match &self.state {
                WorkbenchState::Broken { repair_cost } => *repair_cost,
                _ => 0,
            };
            return Err(WorkbenchEvent::RepairNeeded { repair_cost });
        }
        if !self.queue.has_capacity() {
            return Err(WorkbenchEvent::QueueFull);
        }
        let adjusted_duration = duration / self.tier.speed_multiplier();
        let job_id = self.queue.enqueue(
            recipe_id,
            ingredients,
            self.current_time,
            adjusted_duration,
            quantity,
            owner_id,
        );
        match job_id {
            Some(id) => {
                self.try_start_next_job();
                Ok(id)
            }
            None => Err(WorkbenchEvent::QueueFull),
        }
    }

    /// Repair a broken workbench (costs gold handled externally).
    pub fn repair(&mut self) {
        self.wear = 0.0;
        self.state = WorkbenchState::Idle;
        self.efficiency = 1.0;
    }

    /// Compute repair cost based on tier and wear level.
    pub fn compute_repair_cost(&self) -> u64 {
        let base: u64 = match self.tier {
            WorkbenchTier::Basic    => 100,
            WorkbenchTier::Improved => 400,
            WorkbenchTier::Advanced => 1200,
            WorkbenchTier::Master   => 3500,
        };
        let wear_factor = (self.wear * 3.0) as u64;
        base + wear_factor * 10
    }

    /// Attempt to upgrade the bench to the next tier.  Returns the cost or None if already Master.
    pub fn upgrade_cost(&self) -> Option<u64> {
        match self.tier {
            WorkbenchTier::Master => None,
            _ => Some(self.tier.upgrade_cost()),
        }
    }

    /// Perform the upgrade (payment handled externally).
    pub fn upgrade(&mut self) {
        self.tier = match self.tier {
            WorkbenchTier::Basic    => WorkbenchTier::Improved,
            WorkbenchTier::Improved => WorkbenchTier::Advanced,
            WorkbenchTier::Advanced => WorkbenchTier::Master,
            WorkbenchTier::Master   => WorkbenchTier::Master,
        };
    }

    /// Cancel a queued job by id, returning the job if found.
    pub fn cancel_job(&mut self, job_id: u64) -> Option<CraftingJob> {
        // If it's the active job, stop crafting
        if let WorkbenchState::Crafting { job_id: active_id, .. } = &self.state {
            if *active_id == job_id {
                self.state = WorkbenchState::Idle;
            }
        }
        self.queue.cancel(job_id)
    }

    /// Progress of the current job [0.0, 1.0], or None if idle/broken.
    pub fn current_progress(&self) -> Option<f32> {
        match &self.state {
            WorkbenchState::Crafting { elapsed, duration, .. } => {
                if *duration <= 0.0 {
                    Some(1.0)
                } else {
                    Some((elapsed / duration).clamp(0.0, 1.0))
                }
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// CraftingStation — group of workbenches at one location
// ---------------------------------------------------------------------------

/// A named location containing multiple workbenches with a shared inventory.
#[derive(Debug, Clone)]
pub struct CraftingStation {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
    pub benches: Vec<Workbench>,
    /// Shared item inventory: item_id -> quantity.
    pub inventory: HashMap<String, u32>,
}

impl CraftingStation {
    pub fn new(id: u64, name: impl Into<String>, position: Vec3) -> Self {
        Self {
            id,
            name: name.into(),
            position,
            benches: Vec::new(),
            inventory: HashMap::new(),
        }
    }

    /// Add a workbench to the station.
    pub fn add_bench(&mut self, bench: Workbench) {
        self.benches.push(bench);
    }

    /// Add items to the shared inventory.
    pub fn add_item(&mut self, item_id: impl Into<String>, quantity: u32) {
        *self.inventory.entry(item_id.into()).or_insert(0) += quantity;
    }

    /// Remove items from inventory. Returns false if not enough stock.
    pub fn remove_item(&mut self, item_id: &str, quantity: u32) -> bool {
        if let Some(stock) = self.inventory.get_mut(item_id) {
            if *stock >= quantity {
                *stock -= quantity;
                return true;
            }
        }
        false
    }

    /// How many of an item are in inventory.
    pub fn item_count(&self, item_id: &str) -> u32 {
        self.inventory.get(item_id).copied().unwrap_or(0)
    }

    /// Tick all benches, collecting events.
    pub fn tick(&mut self, dt: f32) -> Vec<(u64, WorkbenchEvent)> {
        let mut all_events = Vec::new();
        for bench in &mut self.benches {
            let bench_id = bench.id;
            let events = bench.tick(dt);
            for event in events {
                // If a job completed, deposit results into shared inventory
                if let WorkbenchEvent::JobCompleted { ref results, .. } = event {
                    for (item_id, qty, _quality) in results {
                        *self.inventory.entry(item_id.clone()).or_insert(0) += qty;
                    }
                }
                all_events.push((bench_id, event));
            }
        }
        all_events
    }

    /// Find a workbench of a given type.
    pub fn find_bench_of_type(&self, bench_type: &WorkbenchType) -> Option<&Workbench> {
        self.benches.iter().find(|b| &b.bench_type == bench_type)
    }

    /// Find a mutable workbench of a given type.
    pub fn find_bench_of_type_mut(&mut self, bench_type: &WorkbenchType) -> Option<&mut Workbench> {
        self.benches.iter_mut().find(|b| &b.bench_type == bench_type)
    }
}

// ---------------------------------------------------------------------------
// AutoCrafter — AI-driven batch production loop
// ---------------------------------------------------------------------------

/// Configuration for an AutoCrafter run.
#[derive(Debug, Clone)]
pub struct AutoCraftConfig {
    pub recipe_id: String,
    pub target_quantity: u32,
    pub owner_id: String,
    /// Minimum inventory of the output item before stopping.
    pub stop_at_stock: u32,
}

impl AutoCraftConfig {
    pub fn new(
        recipe_id: impl Into<String>,
        target_quantity: u32,
        owner_id: impl Into<String>,
    ) -> Self {
        Self {
            recipe_id: recipe_id.into(),
            target_quantity,
            owner_id: owner_id.into(),
            stop_at_stock: u32::MAX,
        }
    }
}

/// Drives a CraftingStation to produce items automatically.
#[derive(Debug, Clone)]
pub struct AutoCrafter {
    pub config: AutoCraftConfig,
    pub produced: u32,
    pub running: bool,
    pub last_submit_time: f32,
    /// Minimum seconds between job submissions.
    pub submit_interval: f32,
}

impl AutoCrafter {
    pub fn new(config: AutoCraftConfig) -> Self {
        Self {
            config,
            produced: 0,
            running: true,
            last_submit_time: 0.0,
            submit_interval: 1.0,
        }
    }

    /// Tick the auto crafter, submitting jobs to the station as capacity allows.
    ///
    /// Returns the number of new jobs submitted.
    pub fn tick(
        &mut self,
        current_time: f32,
        station: &mut CraftingStation,
        recipe_duration: f32,
        ingredients: Vec<(String, u32)>,
    ) -> u32 {
        if !self.running {
            return 0;
        }
        if self.produced >= self.config.target_quantity {
            self.running = false;
            return 0;
        }
        if current_time - self.last_submit_time < self.submit_interval {
            return 0;
        }

        // Check output stock stop condition
        let recipe_output_key = format!("{}_product", self.config.recipe_id);
        let current_stock = station.item_count(&recipe_output_key);
        if current_stock >= self.config.stop_at_stock {
            self.running = false;
            return 0;
        }

        let mut submitted = 0u32;
        let bench_count = station.benches.len();
        let mut i = 0;
        while i < bench_count {
            if self.produced >= self.config.target_quantity { break; }
            let remaining = self.config.target_quantity - self.produced;
            if !station.benches[i].queue.has_capacity() { i += 1; continue; }
            let can_craft = ingredients.iter().all(|(item_id, qty)| {
                station.inventory.get(item_id).copied().unwrap_or(0) >= *qty
            });
            if !can_craft { break; }
            for (item_id, qty) in &ingredients {
                if let Some(stock) = station.inventory.get_mut(item_id) {
                    *stock = stock.saturating_sub(*qty);
                }
            }
            let batch = remaining.min(8);
            let result = station.benches[i].submit_job(
                self.config.recipe_id.clone(),
                ingredients.clone(),
                recipe_duration,
                batch,
                self.config.owner_id.clone(),
            );
            if result.is_ok() {
                self.produced += batch;
                submitted += 1;
                self.last_submit_time = current_time;
            }
            i += 1;
        }
        submitted
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn restart(&mut self) {
        self.running = true;
        self.produced = 0;
    }
}
