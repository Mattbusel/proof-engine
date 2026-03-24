//! System scheduling for the ECS.
//!
//! Systems are functions that mutate the [`World`]. The [`Schedule`] organizes
//! systems into ordered [`SystemStage`]s and runs them each frame.
//!
//! # Stages (in order)
//! `PreUpdate → Update → PostUpdate → PreRender → Render → PostRender`
//!
//! # Features
//! - Label-based ordering: add systems before/after named systems.
//! - [`SystemSet`]: group multiple systems under one label.
//! - [`FixedTimestep`]: run systems at a fixed rate (e.g., physics at 60 Hz).
//! - Parallel execution hints (annotated but single-threaded execution).

use std::collections::HashMap;
use std::time::Duration;

use super::world::World;

// ---------------------------------------------------------------------------
// SystemLabel
// ---------------------------------------------------------------------------

/// A string label used to identify and order systems.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SystemLabel(pub String);

impl SystemLabel {
    pub fn new(label: impl Into<String>) -> Self {
        Self(label.into())
    }
}

impl<S: Into<String>> From<S> for SystemLabel {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for SystemLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// SystemStage
// ---------------------------------------------------------------------------

/// Defines the execution order of system groups.
///
/// Stages run in ascending order of their `u8` discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SystemStage {
    /// Runs before all game logic. Good for input handling and event flushing.
    PreUpdate = 0,
    /// Main game logic stage.
    Update = 1,
    /// Cleanup and post-processing after Update.
    PostUpdate = 2,
    /// Prepare rendering data.
    PreRender = 3,
    /// Rendering systems.
    Render = 4,
    /// Post-render effects, swap buffers, etc.
    PostRender = 5,
    /// Startup systems — run once on the first [`Schedule::run`] call.
    Startup = 10,
    /// Teardown systems — run once when [`Schedule::shutdown`] is called.
    Shutdown = 11,
}

impl SystemStage {
    /// All frame stages in execution order.
    pub fn frame_stages() -> &'static [SystemStage] {
        &[
            SystemStage::PreUpdate,
            SystemStage::Update,
            SystemStage::PostUpdate,
            SystemStage::PreRender,
            SystemStage::Render,
            SystemStage::PostRender,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            SystemStage::PreUpdate  => "PreUpdate",
            SystemStage::Update     => "Update",
            SystemStage::PostUpdate => "PostUpdate",
            SystemStage::PreRender  => "PreRender",
            SystemStage::Render     => "Render",
            SystemStage::PostRender => "PostRender",
            SystemStage::Startup    => "Startup",
            SystemStage::Shutdown   => "Shutdown",
        }
    }
}

// ---------------------------------------------------------------------------
// System type alias
// ---------------------------------------------------------------------------

/// A boxed, callable system function.
pub type System = Box<dyn FnMut(&mut World) + Send + Sync>;

// ---------------------------------------------------------------------------
// SystemEntry — one system with its metadata
// ---------------------------------------------------------------------------

struct SystemEntry {
    label: Option<SystemLabel>,
    /// Labels this system must run AFTER.
    after: Vec<SystemLabel>,
    /// Labels this system must run BEFORE.
    before: Vec<SystemLabel>,
    system: System,
    /// Whether this system can run in parallel with others (hint only).
    parallel_hint: bool,
    /// Whether this system has been run at all (for startup systems).
    has_run: bool,
    /// Run count (for diagnostics).
    run_count: u64,
    /// Total execution time (for diagnostics).
    total_time_ns: u64,
}

impl SystemEntry {
    fn new(system: System) -> Self {
        Self {
            label: None,
            after: Vec::new(),
            before: Vec::new(),
            system,
            parallel_hint: false,
            has_run: false,
            run_count: 0,
            total_time_ns: 0,
        }
    }

    fn run(&mut self, world: &mut World) {
        let start = std::time::Instant::now();
        (self.system)(world);
        let elapsed = start.elapsed().as_nanos() as u64;
        self.run_count += 1;
        self.has_run = true;
        self.total_time_ns += elapsed;
    }

    fn average_time_us(&self) -> f64 {
        if self.run_count == 0 { return 0.0; }
        self.total_time_ns as f64 / self.run_count as f64 / 1000.0
    }
}

impl std::fmt::Debug for SystemEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemEntry")
            .field("label", &self.label)
            .field("after", &self.after)
            .field("run_count", &self.run_count)
            .field("avg_us", &self.average_time_us())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// StageData
// ---------------------------------------------------------------------------

struct StageData {
    entries: Vec<SystemEntry>,
    sorted: bool,
}

impl StageData {
    fn new() -> Self {
        Self { entries: Vec::new(), sorted: false }
    }

    fn add(&mut self, entry: SystemEntry) {
        self.sorted = false;
        self.entries.push(entry);
    }

    /// Topological sort based on `before`/`after` constraints.
    fn sort(&mut self) {
        if self.sorted { return; }
        let n = self.entries.len();
        if n <= 1 { self.sorted = true; return; }

        // Build a label → index map.
        let label_map: HashMap<SystemLabel, usize> = self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.label.as_ref().map(|l| (l.clone(), i)))
            .collect();

        // Build adjacency (must-run-after edges).
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_degree: Vec<usize> = vec![0; n];

        for (i, entry) in self.entries.iter().enumerate() {
            for after in &entry.after {
                if let Some(&j) = label_map.get(after) {
                    // i must run after j → j → i edge
                    adj[j].push(i);
                    in_degree[i] += 1;
                }
            }
            for before in &entry.before {
                if let Some(&j) = label_map.get(before) {
                    // i must run before j → i → j edge
                    adj[i].push(j);
                    in_degree[j] += 1;
                }
            }
        }

        // Kahn's algorithm.
        let mut queue: std::collections::VecDeque<usize> = (0..n)
            .filter(|&i| in_degree[i] == 0)
            .collect();
        let mut order = Vec::with_capacity(n);

        while let Some(i) = queue.pop_front() {
            order.push(i);
            for &j in &adj[i] {
                in_degree[j] -= 1;
                if in_degree[j] == 0 {
                    queue.push_back(j);
                }
            }
        }

        if order.len() != n {
            // Cycle detected — fall back to insertion order.
            eprintln!("ECS Schedule: cycle detected in system ordering, using insertion order");
            self.sorted = true;
            return;
        }

        // Reorder entries.
        let mut new_entries: Vec<Option<SystemEntry>> = self.entries.drain(..).map(Some).collect();
        self.entries = order.into_iter().map(|i| new_entries[i].take().unwrap()).collect();
        self.sorted = true;
    }

    fn run_all(&mut self, world: &mut World) {
        self.sort();
        for entry in &mut self.entries {
            entry.run(world);
        }
    }

    fn total_systems(&self) -> usize {
        self.entries.len()
    }
}

impl std::fmt::Debug for StageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StageData")
            .field("systems", &self.entries.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// FixedTimestep
// ---------------------------------------------------------------------------

/// Runs a system at a fixed logical rate regardless of frame rate.
///
/// Add to a [`Schedule`] using [`Schedule::add_fixed_system`].
pub struct FixedTimestep {
    /// Target rate in Hz.
    pub hz: f64,
    /// Accumulated time in seconds.
    accumulator: f64,
    /// Whether to cap the accumulator to prevent spiral of death.
    pub max_steps_per_frame: usize,
    /// Number of steps taken lifetime.
    pub steps_taken: u64,
    /// The system to run at fixed intervals.
    system: System,
    pub label: Option<SystemLabel>,
}

impl FixedTimestep {
    pub fn new(hz: f64, system: impl FnMut(&mut World) + Send + Sync + 'static) -> Self {
        Self {
            hz,
            accumulator: 0.0,
            max_steps_per_frame: 10,
            steps_taken: 0,
            system: Box::new(system),
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<SystemLabel>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_max_steps(mut self, max: usize) -> Self {
        self.max_steps_per_frame = max;
        self
    }

    /// Tick by `dt` seconds, running the system as many times as warranted.
    /// Returns the number of steps taken.
    pub fn tick(&mut self, dt: f64, world: &mut World) -> usize {
        self.accumulator += dt;
        let step = 1.0 / self.hz;
        let mut steps = 0;
        while self.accumulator >= step && steps < self.max_steps_per_frame {
            (self.system)(world);
            self.accumulator -= step;
            steps += 1;
            self.steps_taken += 1;
        }
        steps
    }

    /// How much time (0..1) is between the last step and the next.
    pub fn interpolation_alpha(&self) -> f64 {
        self.accumulator * self.hz
    }
}

impl std::fmt::Debug for FixedTimestep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixedTimestep")
            .field("hz", &self.hz)
            .field("accumulator", &self.accumulator)
            .field("steps_taken", &self.steps_taken)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// SystemSet
// ---------------------------------------------------------------------------

/// A named group of systems that can be collectively enabled/disabled or ordered.
pub struct SystemSet {
    pub label: SystemLabel,
    pub stage: SystemStage,
    pub enabled: bool,
    systems: Vec<System>,
}

impl SystemSet {
    pub fn new(label: impl Into<SystemLabel>, stage: SystemStage) -> Self {
        Self {
            label: label.into(),
            stage,
            enabled: true,
            systems: Vec::new(),
        }
    }

    pub fn add(mut self, system: impl FnMut(&mut World) + Send + Sync + 'static) -> Self {
        self.systems.push(Box::new(system));
        self
    }

    pub fn run_all(&mut self, world: &mut World) {
        if !self.enabled { return; }
        for sys in &mut self.systems {
            sys(world);
        }
    }

    pub fn len(&self) -> usize {
        self.systems.len()
    }

    pub fn enable(&mut self) { self.enabled = true; }
    pub fn disable(&mut self) { self.enabled = false; }
}

impl std::fmt::Debug for SystemSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemSet")
            .field("label", &self.label)
            .field("stage", &self.stage)
            .field("enabled", &self.enabled)
            .field("systems", &self.systems.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Schedule
// ---------------------------------------------------------------------------

/// Holds all systems organized by stage and runs them each frame.
///
/// # Usage
/// ```rust,ignore
/// let mut schedule = Schedule::new();
/// schedule.add_system(SystemStage::Update, |world: &mut World| {
///     // movement system
/// });
/// schedule.run(&mut world);
/// ```
pub struct Schedule {
    stages: HashMap<SystemStage, StageData>,
    /// Fixed-timestep subsystems.
    fixed: Vec<FixedTimestep>,
    /// System sets (named groups).
    sets: Vec<SystemSet>,
    /// Whether startup systems have been run.
    startup_done: bool,
    /// Frame counter.
    frame_count: u64,
    /// Whether to collect diagnostics.
    pub diagnostics_enabled: bool,
    /// Frame time diagnostics (last N frames).
    frame_times_ns: std::collections::VecDeque<u64>,
    frame_times_limit: usize,
}

impl Schedule {
    /// Create an empty schedule.
    pub fn new() -> Self {
        let mut stages = HashMap::new();
        for &stage in SystemStage::frame_stages() {
            stages.insert(stage, StageData::new());
        }
        stages.insert(SystemStage::Startup, StageData::new());
        stages.insert(SystemStage::Shutdown, StageData::new());
        Self {
            stages,
            fixed: Vec::new(),
            sets: Vec::new(),
            startup_done: false,
            frame_count: 0,
            diagnostics_enabled: false,
            frame_times_ns: std::collections::VecDeque::new(),
            frame_times_limit: 128,
        }
    }

    // -----------------------------------------------------------------------
    // System registration
    // -----------------------------------------------------------------------

    /// Add a system to a stage.
    pub fn add_system(
        &mut self,
        stage: SystemStage,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        let entry = SystemEntry::new(Box::new(system));
        self.stages
            .entry(stage)
            .or_insert_with(StageData::new)
            .add(entry);
    }

    /// Add a system with a label (for ordering).
    pub fn add_system_with_label(
        &mut self,
        stage: SystemStage,
        label: impl Into<SystemLabel>,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        let mut entry = SystemEntry::new(Box::new(system));
        entry.label = Some(label.into());
        self.stages
            .entry(stage)
            .or_insert_with(StageData::new)
            .add(entry);
    }

    /// Add a system that must run after `after_label`.
    pub fn add_system_after(
        &mut self,
        stage: SystemStage,
        after_label: impl Into<SystemLabel>,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        let mut entry = SystemEntry::new(Box::new(system));
        entry.after.push(after_label.into());
        self.stages
            .entry(stage)
            .or_insert_with(StageData::new)
            .add(entry);
    }

    /// Add a system that must run before `before_label`.
    pub fn add_system_before(
        &mut self,
        stage: SystemStage,
        before_label: impl Into<SystemLabel>,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        let mut entry = SystemEntry::new(Box::new(system));
        entry.before.push(before_label.into());
        self.stages
            .entry(stage)
            .or_insert_with(StageData::new)
            .add(entry);
    }

    /// Add a system with full ordering constraints.
    pub fn add_system_ordered(
        &mut self,
        stage: SystemStage,
        label: Option<impl Into<SystemLabel>>,
        after: Vec<SystemLabel>,
        before: Vec<SystemLabel>,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        let mut entry = SystemEntry::new(Box::new(system));
        entry.label = label.map(|l| l.into());
        entry.after = after;
        entry.before = before;
        self.stages
            .entry(stage)
            .or_insert_with(StageData::new)
            .add(entry);
    }

    /// Add a startup system (runs exactly once before the first frame).
    pub fn add_startup_system(
        &mut self,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        self.add_system(SystemStage::Startup, system);
    }

    /// Add a shutdown system.
    pub fn add_shutdown_system(
        &mut self,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        self.add_system(SystemStage::Shutdown, system);
    }

    /// Add a fixed-timestep system.
    pub fn add_fixed_system(
        &mut self,
        hz: f64,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        self.fixed.push(FixedTimestep::new(hz, system));
    }

    /// Add a fixed-timestep system with a label.
    pub fn add_fixed_system_with_label(
        &mut self,
        hz: f64,
        label: impl Into<SystemLabel>,
        system: impl FnMut(&mut World) + Send + Sync + 'static,
    ) {
        self.fixed.push(FixedTimestep::new(hz, system).with_label(label));
    }

    /// Add a system set.
    pub fn add_set(&mut self, set: SystemSet) {
        self.sets.push(set);
    }

    /// Mark a system as having a parallel hint.
    pub fn mark_parallel(
        &mut self,
        stage: SystemStage,
        label: &SystemLabel,
    ) {
        if let Some(data) = self.stages.get_mut(&stage) {
            for entry in &mut data.entries {
                if entry.label.as_ref() == Some(label) {
                    entry.parallel_hint = true;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Execution
    // -----------------------------------------------------------------------

    /// Run all frame stages. Runs startup systems on the first call.
    pub fn run(&mut self, world: &mut World) {
        let frame_start = std::time::Instant::now();

        // Startup systems run exactly once.
        if !self.startup_done {
            self.run_stage(SystemStage::Startup, world);
            self.startup_done = true;
        }

        // Run all frame stages.
        for &stage in SystemStage::frame_stages() {
            self.run_stage(stage, world);
        }

        // Advance world tick.
        world.advance_tick();

        self.frame_count += 1;

        if self.diagnostics_enabled {
            let elapsed = frame_start.elapsed().as_nanos() as u64;
            if self.frame_times_ns.len() >= self.frame_times_limit {
                self.frame_times_ns.pop_front();
            }
            self.frame_times_ns.push_back(elapsed);
        }
    }

    /// Run all frame stages, including fixed-timestep systems, given `dt`.
    pub fn run_with_dt(&mut self, world: &mut World, dt: f64) {
        let frame_start = std::time::Instant::now();

        if !self.startup_done {
            self.run_stage(SystemStage::Startup, world);
            self.startup_done = true;
        }

        // Run fixed systems first (before PreUpdate).
        for fixed in &mut self.fixed {
            fixed.tick(dt, world);
        }

        for &stage in SystemStage::frame_stages() {
            self.run_stage(stage, world);
        }

        // Run sets at their designated stage (already added to stages above,
        // but explicit sets run here).
        for set in &mut self.sets {
            set.run_all(world);
        }

        world.advance_tick();
        self.frame_count += 1;

        if self.diagnostics_enabled {
            let elapsed = frame_start.elapsed().as_nanos() as u64;
            if self.frame_times_ns.len() >= self.frame_times_limit {
                self.frame_times_ns.pop_front();
            }
            self.frame_times_ns.push_back(elapsed);
        }
    }

    /// Run a specific stage.
    pub fn run_stage(&mut self, stage: SystemStage, world: &mut World) {
        if let Some(data) = self.stages.get_mut(&stage) {
            data.run_all(world);
        }
    }

    /// Run the shutdown stage.
    pub fn shutdown(&mut self, world: &mut World) {
        self.run_stage(SystemStage::Shutdown, world);
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Number of systems in a stage.
    pub fn system_count(&self, stage: SystemStage) -> usize {
        self.stages.get(&stage).map_or(0, |d| d.total_systems())
    }

    /// Total systems across all stages.
    pub fn total_system_count(&self) -> usize {
        self.stages.values().map(|d| d.total_systems()).sum()
    }

    /// Frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Average frame time in milliseconds (requires diagnostics enabled).
    pub fn average_frame_time_ms(&self) -> f64 {
        if self.frame_times_ns.is_empty() { return 0.0; }
        let sum: u64 = self.frame_times_ns.iter().sum();
        sum as f64 / self.frame_times_ns.len() as f64 / 1_000_000.0
    }

    /// Enable timing diagnostics.
    pub fn enable_diagnostics(&mut self) {
        self.diagnostics_enabled = true;
    }

    /// Whether the schedule has been started (startup systems run).
    pub fn is_started(&self) -> bool {
        self.startup_done
    }

    /// Enable/disable a named system set.
    pub fn set_enabled(&mut self, label: &SystemLabel, enabled: bool) {
        for set in &mut self.sets {
            if &set.label == label {
                set.enabled = enabled;
            }
        }
    }

    /// Find and remove all systems with a given label.
    pub fn remove_system(&mut self, stage: SystemStage, label: &SystemLabel) {
        if let Some(data) = self.stages.get_mut(&stage) {
            data.entries.retain(|e| e.label.as_ref() != Some(label));
            data.sorted = false;
        }
    }

    /// Print a diagnostic summary.
    pub fn print_diagnostics(&self) {
        println!("=== Schedule Diagnostics ===");
        println!("Frames: {}", self.frame_count);
        println!("Avg frame: {:.2} ms", self.average_frame_time_ms());
        for &stage in SystemStage::frame_stages() {
            if let Some(data) = self.stages.get(&stage) {
                println!(
                    "  Stage {:12}: {:3} systems",
                    stage.name(),
                    data.total_systems()
                );
                for entry in &data.entries {
                    let label = entry.label.as_ref()
                        .map(|l| l.0.as_str())
                        .unwrap_or("<unlabeled>");
                    println!(
                        "    [{:20}] runs={:6}  avg={:.2} µs",
                        label,
                        entry.run_count,
                        entry.average_time_us()
                    );
                }
            }
        }
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Schedule")
            .field("frame_count", &self.frame_count)
            .field("total_systems", &self.total_system_count())
            .field("fixed_systems", &self.fixed.len())
            .field("startup_done", &self.startup_done)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ScheduleBuilder — ergonomic construction
// ---------------------------------------------------------------------------

/// Fluent builder for a [`Schedule`].
pub struct ScheduleBuilder {
    schedule: Schedule,
}

impl ScheduleBuilder {
    pub fn new() -> Self {
        Self { schedule: Schedule::new() }
    }

    pub fn system(mut self, stage: SystemStage, f: impl FnMut(&mut World) + Send + Sync + 'static) -> Self {
        self.schedule.add_system(stage, f);
        self
    }

    pub fn labeled_system(
        mut self,
        stage: SystemStage,
        label: impl Into<SystemLabel>,
        f: impl FnMut(&mut World) + Send + Sync + 'static,
    ) -> Self {
        self.schedule.add_system_with_label(stage, label, f);
        self
    }

    pub fn startup(mut self, f: impl FnMut(&mut World) + Send + Sync + 'static) -> Self {
        self.schedule.add_startup_system(f);
        self
    }

    pub fn fixed(mut self, hz: f64, f: impl FnMut(&mut World) + Send + Sync + 'static) -> Self {
        self.schedule.add_fixed_system(hz, f);
        self
    }

    pub fn with_diagnostics(mut self) -> Self {
        self.schedule.enable_diagnostics();
        self
    }

    pub fn build(self) -> Schedule {
        self.schedule
    }
}

impl Default for ScheduleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RunCriteria — conditional system execution
// ---------------------------------------------------------------------------

/// Determines whether a system should run this frame.
pub trait RunCriteria: Send + Sync + 'static {
    fn should_run(&mut self, world: &World) -> bool;
}

/// Always-run criterion.
pub struct Always;
impl RunCriteria for Always {
    fn should_run(&mut self, _: &World) -> bool { true }
}

/// Never-run criterion (useful for disabling systems temporarily).
pub struct Never;
impl RunCriteria for Never {
    fn should_run(&mut self, _: &World) -> bool { false }
}

/// Run once criterion.
pub struct RunOnce {
    ran: bool,
}
impl RunOnce {
    pub fn new() -> Self { Self { ran: false } }
}
impl Default for RunOnce {
    fn default() -> Self { Self::new() }
}
impl RunCriteria for RunOnce {
    fn should_run(&mut self, _: &World) -> bool {
        if self.ran { return false; }
        self.ran = true;
        true
    }
}

/// Run every N frames.
pub struct EveryNFrames {
    n: u64,
    current: u64,
}
impl EveryNFrames {
    pub fn new(n: u64) -> Self { Self { n, current: 0 } }
}
impl RunCriteria for EveryNFrames {
    fn should_run(&mut self, _: &World) -> bool {
        self.current += 1;
        if self.current >= self.n {
            self.current = 0;
            true
        } else {
            false
        }
    }
}

/// Run when a resource exists.
pub struct WhenResource<T: super::world::Resource> {
    _marker: std::marker::PhantomData<T>,
}
impl<T: super::world::Resource> WhenResource<T> {
    pub fn new() -> Self { Self { _marker: std::marker::PhantomData } }
}
impl<T: super::world::Resource> Default for WhenResource<T> {
    fn default() -> Self { Self::new() }
}
impl<T: super::world::Resource> RunCriteria for WhenResource<T> {
    fn should_run(&mut self, world: &World) -> bool {
        world.has_resource::<T>()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Counter(i32);

    #[test]
    fn test_basic_schedule_run() {
        let mut world = World::new();
        world.insert_resource(Counter(0));

        let mut schedule = Schedule::new();
        schedule.add_system(SystemStage::Update, |w: &mut World| {
            w.resource_mut::<Counter>().0 += 1;
        });

        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().0, 1);
        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().0, 2);
    }

    #[test]
    fn test_startup_system_runs_once() {
        let mut world = World::new();
        world.insert_resource(Counter(0));

        let mut schedule = Schedule::new();
        schedule.add_startup_system(|w: &mut World| {
            w.resource_mut::<Counter>().0 += 100;
        });
        schedule.add_system(SystemStage::Update, |w: &mut World| {
            w.resource_mut::<Counter>().0 += 1;
        });

        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().0, 101);
        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().0, 102); // startup not re-run
    }

    #[test]
    fn test_stage_ordering() {
        let mut world = World::new();
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));

        let mut schedule = Schedule::new();

        let o1 = order.clone();
        schedule.add_system(SystemStage::PostUpdate, move |_| { o1.lock().unwrap().push("PostUpdate"); });
        let o2 = order.clone();
        schedule.add_system(SystemStage::PreUpdate, move |_| { o2.lock().unwrap().push("PreUpdate"); });
        let o3 = order.clone();
        schedule.add_system(SystemStage::Update, move |_| { o3.lock().unwrap().push("Update"); });

        schedule.run(&mut world);

        let result = order.lock().unwrap().clone();
        assert_eq!(result, vec!["PreUpdate", "Update", "PostUpdate"]);
    }

    #[test]
    fn test_system_label_ordering() {
        let mut world = World::new();
        let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<i32>::new()));

        let mut schedule = Schedule::new();

        let o1 = order.clone();
        schedule.add_system_with_label(SystemStage::Update, "second", move |_| {
            o1.lock().unwrap().push(2);
        });
        let o2 = order.clone();
        let mut e = SystemEntry::new(Box::new(move |_: &mut World| {
            o2.lock().unwrap().push(1);
        }));
        e.label = Some("first".into());
        e.before = vec!["second".into()];
        schedule.stages.get_mut(&SystemStage::Update).unwrap().add(e);

        schedule.run(&mut world);

        let result = order.lock().unwrap().clone();
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn test_fixed_timestep() {
        let mut world = World::new();
        world.insert_resource(Counter(0));

        let mut ts = FixedTimestep::new(10.0, |w: &mut World| {
            w.resource_mut::<Counter>().0 += 1;
        });

        // 0.25 seconds at 10 Hz = 2.5 steps → 2 full steps
        ts.tick(0.25, &mut world);
        assert_eq!(world.resource::<Counter>().0, 2);

        // 0.1 more → 1 step (accumulated 0.05 + 0.1 = 0.15 → 1 step at 0.1)
        ts.tick(0.1, &mut world);
        assert_eq!(world.resource::<Counter>().0, 3);
    }

    #[test]
    fn test_schedule_builder() {
        let mut world = World::new();
        world.insert_resource(Counter(0));

        let mut schedule = ScheduleBuilder::new()
            .startup(|w: &mut World| { w.resource_mut::<Counter>().0 += 10; })
            .system(SystemStage::Update, |w: &mut World| { w.resource_mut::<Counter>().0 += 1; })
            .build();

        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().0, 11);
    }

    #[test]
    fn test_system_count() {
        let mut schedule = Schedule::new();
        schedule.add_system(SystemStage::Update, |_: &mut World| {});
        schedule.add_system(SystemStage::Update, |_: &mut World| {});
        schedule.add_system(SystemStage::PreUpdate, |_: &mut World| {});
        assert_eq!(schedule.system_count(SystemStage::Update), 2);
        assert_eq!(schedule.system_count(SystemStage::PreUpdate), 1);
        assert_eq!(schedule.total_system_count(), 3);
    }

    #[test]
    fn test_remove_system() {
        let mut world = World::new();
        world.insert_resource(Counter(0));

        let mut schedule = Schedule::new();
        schedule.add_system_with_label(SystemStage::Update, "counter", |w: &mut World| {
            w.resource_mut::<Counter>().0 += 1;
        });

        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().0, 1);

        schedule.remove_system(SystemStage::Update, &"counter".into());
        schedule.run(&mut world);
        assert_eq!(world.resource::<Counter>().0, 1); // system was removed
    }

    #[test]
    fn test_system_set() {
        let mut world = World::new();
        world.insert_resource(Counter(0));

        let o = std::sync::Arc::new(std::sync::Mutex::new(Vec::<i32>::new()));
        let o1 = o.clone();
        let o2 = o.clone();

        let set = SystemSet::new("my_set", SystemStage::Update)
            .add(move |_: &mut World| { o1.lock().unwrap().push(1); })
            .add(move |_: &mut World| { o2.lock().unwrap().push(2); });

        let mut schedule = Schedule::new();
        schedule.add_set(set);
        schedule.run_with_dt(&mut world, 0.016);

        let result = o.lock().unwrap().clone();
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn test_run_once_criterion() {
        let mut world = World::new();
        let mut criterion = RunOnce::new();
        assert!(criterion.should_run(&world));
        assert!(!criterion.should_run(&world));
        assert!(!criterion.should_run(&world));
    }

    #[test]
    fn test_every_n_frames() {
        let world = World::new();
        let mut criterion = EveryNFrames::new(3);
        assert!(!criterion.should_run(&world)); // frame 1
        assert!(!criterion.should_run(&world)); // frame 2
        assert!(criterion.should_run(&world));  // frame 3
        assert!(!criterion.should_run(&world)); // frame 4
        assert!(!criterion.should_run(&world)); // frame 5
        assert!(criterion.should_run(&world));  // frame 6
    }

    #[test]
    fn test_world_tick_advances() {
        let mut world = World::new();
        let mut schedule = Schedule::new();
        assert_eq!(world.tick(), 0);
        schedule.run(&mut world);
        assert_eq!(world.tick(), 1);
        schedule.run(&mut world);
        assert_eq!(world.tick(), 2);
    }
}
