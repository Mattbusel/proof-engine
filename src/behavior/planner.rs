//! Goal-Oriented Action Planning (GOAP).
//!
//! # Overview
//!
//! GOAP lets an AI agent automatically figure out *how* to reach a goal by
//! searching for the cheapest sequence of actions that transforms the current
//! world state into the goal state.
//!
//! ## Core types
//!
//! | Type | Role |
//! |------|------|
//! | [`WorldState`]   | Named boolean + float condition map |
//! | [`Action`]       | Preconditions, effects, cost, duration |
//! | [`GoalStack`]    | Priority-ordered goals for one agent |
//! | [`GoapPlanner`]  | A* plan search |
//! | [`PlanExecutor`] | Runs a plan, detects state drift, replans |
//!
//! ## Example
//! ```ignore
//! let mut state = WorldState::new();
//! state.set_bool("has_weapon", false);
//! state.set_bool("enemy_dead", false);
//!
//! let pick_up = Action::new("pick_up_weapon", 1.0)
//!     .require_bool("has_weapon", false)
//!     .effect_bool("has_weapon", true);
//!
//! let attack = Action::new("attack_enemy", 2.0)
//!     .require_bool("has_weapon", true)
//!     .require_bool("enemy_dead", false)
//!     .effect_bool("enemy_dead", true);
//!
//! let mut goal = WorldState::new();
//! goal.set_bool("enemy_dead", true);
//!
//! let plan = GoapPlanner::plan(&state, &goal, &[pick_up, attack], 10);
//! // plan == Some(["pick_up_weapon", "attack_enemy"])
//! ```

use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Ordering;

// ── WorldState ────────────────────────────────────────────────────────────────

/// A set of named conditions describing the current state of the world from
/// one agent's perspective.
///
/// Conditions have two flavours:
/// - **Bool** — classic GOAP true/false flags.
/// - **Float** — numeric values (health, ammo count, distance…) used for
///   richer precondition checking.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorldState {
    bools:  HashMap<String, bool>,
    floats: HashMap<String, f32>,
}

impl WorldState {
    pub fn new() -> Self { Self::default() }

    // ── bool conditions ───────────────────────────────────────────────────────

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.bools.insert(key.to_string(), value);
    }

    pub fn get_bool(&self, key: &str) -> bool {
        *self.bools.get(key).unwrap_or(&false)
    }

    pub fn has_bool(&self, key: &str) -> bool {
        self.bools.contains_key(key)
    }

    // ── float conditions ──────────────────────────────────────────────────────

    pub fn set_float(&mut self, key: &str, value: f32) {
        self.floats.insert(key.to_string(), value);
    }

    pub fn get_float(&self, key: &str) -> f32 {
        *self.floats.get(key).unwrap_or(&0.0)
    }

    pub fn has_float(&self, key: &str) -> bool {
        self.floats.contains_key(key)
    }

    // ── satisfaction checking ─────────────────────────────────────────────────

    /// Does `self` satisfy every condition in `goal`?
    ///
    /// Bool conditions must match exactly.
    /// Float conditions in `goal` are treated as *lower bounds* — i.e.
    /// `self.float >= goal.float`.
    pub fn satisfies(&self, goal: &WorldState) -> bool {
        for (k, &v) in &goal.bools {
            if self.get_bool(k) != v { return false; }
        }
        for (k, &v) in &goal.floats {
            if self.get_float(k) < v { return false; }
        }
        true
    }

    /// Number of unsatisfied conditions from `goal`.  Used as A* heuristic.
    pub fn distance_to(&self, goal: &WorldState) -> usize {
        let bool_unsatisfied = goal.bools.iter()
            .filter(|(k, &v)| self.get_bool(k) != v)
            .count();
        let float_unsatisfied = goal.floats.iter()
            .filter(|(k, &v)| self.get_float(k) < v)
            .count();
        bool_unsatisfied + float_unsatisfied
    }

    /// Apply an action's effects to produce the successor state.
    pub fn apply(&self, effects: &ActionEffects) -> WorldState {
        let mut next = self.clone();
        for (k, &v) in &effects.bools { next.bools.insert(k.clone(), v); }
        for (k, &v) in &effects.floats_add {
            let cur = next.get_float(k);
            next.floats.insert(k.clone(), cur + v);
        }
        for (k, &v) in &effects.floats_set {
            next.floats.insert(k.clone(), v);
        }
        next
    }

    /// Merge another state into self, overwriting on conflict.
    pub fn merge_from(&mut self, other: &WorldState) {
        for (k, &v) in &other.bools  { self.bools.insert(k.clone(), v); }
        for (k, &v) in &other.floats { self.floats.insert(k.clone(), v); }
    }

    /// True if this state has no conditions at all.
    pub fn is_empty(&self) -> bool {
        self.bools.is_empty() && self.floats.is_empty()
    }

    /// Create a snapshot key for closed-set deduplication.
    fn snapshot_key(&self) -> StateKey {
        let mut bool_pairs: Vec<(String, bool)> = self.bools.iter()
            .map(|(k, &v)| (k.clone(), v)).collect();
        bool_pairs.sort_by(|a, b| a.0.cmp(&b.0));

        let mut float_pairs: Vec<(String, u32)> = self.floats.iter()
            .map(|(k, &v)| (k.clone(), v.to_bits())).collect();
        float_pairs.sort_by(|a, b| a.0.cmp(&b.0));

        StateKey { bools: bool_pairs, floats: float_pairs }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StateKey {
    bools:  Vec<(String, bool)>,
    floats: Vec<(String, u32)>,
}

// ── ActionEffects ─────────────────────────────────────────────────────────────

/// The effects portion of an [`Action`], separated out so that `WorldState`
/// can apply them without borrowing the whole action.
#[derive(Debug, Clone, Default)]
pub struct ActionEffects {
    /// Set named bool conditions.
    pub bools:      HashMap<String, bool>,
    /// Add a delta to named float conditions.
    pub floats_add: HashMap<String, f32>,
    /// Set named float conditions to an absolute value.
    pub floats_set: HashMap<String, f32>,
}

impl ActionEffects {
    pub fn new() -> Self { Self::default() }

    pub fn set_bool(mut self, key: &str, value: bool) -> Self {
        self.bools.insert(key.to_string(), value);
        self
    }

    pub fn add_float(mut self, key: &str, delta: f32) -> Self {
        self.floats_add.insert(key.to_string(), delta);
        self
    }

    pub fn set_float(mut self, key: &str, value: f32) -> Self {
        self.floats_set.insert(key.to_string(), value);
        self
    }
}

// ── Preconditions ─────────────────────────────────────────────────────────────

/// The preconditions an action requires to be applicable.
#[derive(Debug, Clone, Default)]
pub struct Preconditions {
    pub bools:       HashMap<String, bool>,
    /// Key must be >= threshold.
    pub floats_gte:  HashMap<String, f32>,
    /// Key must be <= threshold.
    pub floats_lte:  HashMap<String, f32>,
    /// Key must be > threshold.
    pub floats_gt:   HashMap<String, f32>,
    /// Key must be < threshold.
    pub floats_lt:   HashMap<String, f32>,
}

impl Preconditions {
    pub fn new() -> Self { Self::default() }

    pub fn require_bool(mut self, key: &str, value: bool) -> Self {
        self.bools.insert(key.to_string(), value);
        self
    }

    pub fn require_float_gte(mut self, key: &str, min: f32) -> Self {
        self.floats_gte.insert(key.to_string(), min);
        self
    }

    pub fn require_float_lte(mut self, key: &str, max: f32) -> Self {
        self.floats_lte.insert(key.to_string(), max);
        self
    }

    pub fn require_float_gt(mut self, key: &str, min: f32) -> Self {
        self.floats_gt.insert(key.to_string(), min);
        self
    }

    pub fn require_float_lt(mut self, key: &str, max: f32) -> Self {
        self.floats_lt.insert(key.to_string(), max);
        self
    }

    /// Returns true if `state` satisfies all preconditions.
    pub fn satisfied_by(&self, state: &WorldState) -> bool {
        for (k, &v) in &self.bools {
            if state.get_bool(k) != v { return false; }
        }
        for (k, &t) in &self.floats_gte { if state.get_float(k) <  t { return false; } }
        for (k, &t) in &self.floats_lte { if state.get_float(k) >  t { return false; } }
        for (k, &t) in &self.floats_gt  { if state.get_float(k) <= t { return false; } }
        for (k, &t) in &self.floats_lt  { if state.get_float(k) >= t { return false; } }
        true
    }
}

// ── Action ────────────────────────────────────────────────────────────────────

/// A GOAP action that an agent can execute.
///
/// Each action has:
/// - A **cost** (lower = preferred by the planner).
/// - A set of **preconditions** that must hold before it can run.
/// - A set of **effects** that it applies to the world state.
/// - An optional **duration** in simulated seconds.
/// - An optional **interruption priority** (higher = harder to interrupt).
#[derive(Debug, Clone)]
pub struct Action {
    /// Unique name identifying this action.
    pub name:          String,
    /// Base cost (used by A* to prefer cheaper plans).
    pub cost:          f32,
    /// Preconditions that must hold.
    pub preconditions: Preconditions,
    /// Effects applied to world state on completion.
    pub effects:       ActionEffects,
    /// Estimated duration in seconds (used by the executor).
    pub duration_secs: f32,
    /// Priority when being interrupted by a higher-priority action.
    pub interrupt_priority: u32,
    /// If true, the planner will not use this action (temporarily disabled).
    pub disabled:      bool,
    /// User data tag for categorizing actions.
    pub tags:          Vec<String>,
}

impl Action {
    pub fn new(name: &str, cost: f32) -> Self {
        Self {
            name:              name.to_string(),
            cost,
            preconditions:     Preconditions::new(),
            effects:           ActionEffects::new(),
            duration_secs:     0.0,
            interrupt_priority: 0,
            disabled:          false,
            tags:              Vec::new(),
        }
    }

    // ── Fluent precondition builders ──────────────────────────────────────────

    pub fn require_bool(mut self, key: &str, value: bool) -> Self {
        self.preconditions = self.preconditions.require_bool(key, value);
        self
    }

    pub fn require_float_gte(mut self, key: &str, min: f32) -> Self {
        self.preconditions = self.preconditions.require_float_gte(key, min);
        self
    }

    pub fn require_float_lte(mut self, key: &str, max: f32) -> Self {
        self.preconditions = self.preconditions.require_float_lte(key, max);
        self
    }

    pub fn require_float_gt(mut self, key: &str, val: f32) -> Self {
        self.preconditions = self.preconditions.require_float_gt(key, val);
        self
    }

    pub fn require_float_lt(mut self, key: &str, val: f32) -> Self {
        self.preconditions = self.preconditions.require_float_lt(key, val);
        self
    }

    // ── Fluent effect builders ────────────────────────────────────────────────

    pub fn effect_bool(mut self, key: &str, value: bool) -> Self {
        self.effects = self.effects.set_bool(key, value);
        self
    }

    pub fn effect_add_float(mut self, key: &str, delta: f32) -> Self {
        self.effects = self.effects.add_float(key, delta);
        self
    }

    pub fn effect_set_float(mut self, key: &str, value: f32) -> Self {
        self.effects = self.effects.set_float(key, value);
        self
    }

    // ── Other builders ────────────────────────────────────────────────────────

    pub fn with_duration(mut self, secs: f32) -> Self {
        self.duration_secs = secs;
        self
    }

    pub fn with_interrupt_priority(mut self, p: u32) -> Self {
        self.interrupt_priority = p;
        self
    }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    // ── Applicability ─────────────────────────────────────────────────────────

    pub fn is_applicable(&self, state: &WorldState) -> bool {
        !self.disabled && self.preconditions.satisfied_by(state)
    }

    /// Compute the successor state by applying this action's effects.
    pub fn apply_effects(&self, state: &WorldState) -> WorldState {
        state.apply(&self.effects)
    }
}

// ── Goal ─────────────────────────────────────────────────────────────────────

/// A named goal with a desired `WorldState` and a priority.
///
/// Higher priority goals pre-empt lower priority ones.
#[derive(Debug, Clone)]
pub struct Goal {
    pub name:     String,
    pub state:    WorldState,
    pub priority: u32,
    /// If set, this goal expires after `ttl_secs` simulation seconds.
    pub ttl_secs: Option<f32>,
    created_at:   f32,
}

impl Goal {
    pub fn new(name: &str, state: WorldState, priority: u32) -> Self {
        Self { name: name.to_string(), state, priority, ttl_secs: None, created_at: 0.0 }
    }

    pub fn with_ttl(mut self, ttl_secs: f32) -> Self {
        self.ttl_secs = Some(ttl_secs);
        self
    }

    pub fn is_expired(&self, sim_time: f32) -> bool {
        self.ttl_secs.map_or(false, |ttl| sim_time - self.created_at > ttl)
    }
}

// ── GoalStack ─────────────────────────────────────────────────────────────────

/// A priority-ordered collection of goals for one agent.
///
/// The active goal is always the one with the highest `priority`.  If two
/// goals share the same priority the one added first wins (stable sort).
#[derive(Debug, Default)]
pub struct GoalStack {
    goals:    Vec<Goal>,
    sim_time: f32,
}

impl GoalStack {
    pub fn new() -> Self { Self::default() }

    /// Push a new goal.  The stack is re-sorted automatically.
    pub fn push(&mut self, mut goal: Goal) {
        goal.created_at = self.sim_time;
        self.goals.push(goal);
        // Stable descending sort by priority.
        self.goals.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove the goal with `name`.
    pub fn remove(&mut self, name: &str) {
        self.goals.retain(|g| g.name != name);
    }

    /// Return the highest-priority active goal, if any.
    pub fn active(&self) -> Option<&Goal> {
        self.goals.iter().find(|g| !g.is_expired(self.sim_time))
    }

    /// Advance simulation time and prune expired goals.
    pub fn tick(&mut self, dt: f32) {
        self.sim_time += dt;
        let t = self.sim_time;
        self.goals.retain(|g| !g.is_expired(t));
    }

    pub fn is_empty(&self) -> bool { self.goals.is_empty() }
    pub fn len(&self)     -> usize { self.goals.len() }

    /// Iterate all goals (highest priority first).
    pub fn iter(&self) -> impl Iterator<Item = &Goal> {
        self.goals.iter()
    }

    /// True if any goal with `name` exists and is not expired.
    pub fn has_goal(&self, name: &str) -> bool {
        self.goals.iter().any(|g| g.name == name && !g.is_expired(self.sim_time))
    }

    pub fn sim_time(&self) -> f32 { self.sim_time }
}

// ── A* search node ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct SearchNode {
    state:     WorldState,
    /// Action names taken to reach this state.
    path:      Vec<String>,
    /// Accumulated cost g(n).
    cost:      f32,
    /// Heuristic estimate h(n).
    heuristic: usize,
}

impl SearchNode {
    fn f(&self)     -> f32 { self.cost + self.heuristic as f32 }
    fn f_ord(&self) -> u64 { (self.f() * 1_000_000.0) as u64 }
}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool { self.f_ord() == other.f_ord() }
}
impl Eq for SearchNode {}

impl PartialOrd for SearchNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for SearchNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: reverse order so that smaller f() has higher priority.
        other.f_ord().cmp(&self.f_ord())
    }
}

// ── GoapPlanner ───────────────────────────────────────────────────────────────

/// Stateless A* GOAP planner.
pub struct GoapPlanner;

impl GoapPlanner {
    /// Find the cheapest sequence of action names that transforms `start` into
    /// a state satisfying `goal`, or `None` if no plan is reachable within
    /// `max_depth` actions.
    pub fn plan(
        start:     &WorldState,
        goal:      &WorldState,
        actions:   &[Action],
        max_depth: usize,
    ) -> Option<Vec<String>> {
        if start.satisfies(goal) {
            return Some(Vec::new()); // already satisfied
        }

        let mut open:   BinaryHeap<SearchNode> = BinaryHeap::new();
        let mut closed: HashSet<StateKey>       = HashSet::new();

        open.push(SearchNode {
            state:     start.clone(),
            path:      Vec::new(),
            cost:      0.0,
            heuristic: start.distance_to(goal),
        });

        while let Some(node) = open.pop() {
            if node.state.satisfies(goal) {
                return Some(node.path);
            }

            if node.path.len() >= max_depth { continue; }

            let key = node.state.snapshot_key();
            if closed.contains(&key) { continue; }
            closed.insert(key);

            for action in actions {
                if !action.is_applicable(&node.state) { continue; }

                let next_state = action.apply_effects(&node.state);
                let next_key   = next_state.snapshot_key();
                if closed.contains(&next_key) { continue; }

                let next_cost = node.cost + action.cost;
                let mut next_path = node.path.clone();
                next_path.push(action.name.clone());

                open.push(SearchNode {
                    state:     next_state.clone(),
                    path:      next_path,
                    cost:      next_cost,
                    heuristic: next_state.distance_to(goal),
                });
            }
        }

        None // no plan found
    }

    /// Plan and return full detail: path of action names + total estimated cost.
    pub fn plan_with_cost(
        start:     &WorldState,
        goal:      &WorldState,
        actions:   &[Action],
        max_depth: usize,
    ) -> Option<(Vec<String>, f32)> {
        if start.satisfies(goal) {
            return Some((Vec::new(), 0.0));
        }

        let mut open:   BinaryHeap<SearchNode> = BinaryHeap::new();
        let mut closed: HashSet<StateKey>       = HashSet::new();

        open.push(SearchNode {
            state:     start.clone(),
            path:      Vec::new(),
            cost:      0.0,
            heuristic: start.distance_to(goal),
        });

        while let Some(node) = open.pop() {
            if node.state.satisfies(goal) {
                let cost = node.cost;
                return Some((node.path, cost));
            }

            if node.path.len() >= max_depth { continue; }

            let key = node.state.snapshot_key();
            if closed.contains(&key) { continue; }
            closed.insert(key);

            for action in actions {
                if !action.is_applicable(&node.state) { continue; }

                let next_state = action.apply_effects(&node.state);
                let next_key   = next_state.snapshot_key();
                if closed.contains(&next_key) { continue; }

                let next_cost = node.cost + action.cost;
                let mut next_path = node.path.clone();
                next_path.push(action.name.clone());

                open.push(SearchNode {
                    state:     next_state.clone(),
                    path:      next_path,
                    cost:      next_cost,
                    heuristic: next_state.distance_to(goal),
                });
            }
        }

        None
    }

    /// Return all valid plans up to `max_plans` alternatives, sorted by cost.
    pub fn plan_alternatives(
        start:     &WorldState,
        goal:      &WorldState,
        actions:   &[Action],
        max_depth: usize,
        max_plans: usize,
    ) -> Vec<(Vec<String>, f32)> {
        let mut results: Vec<(Vec<String>, f32)> = Vec::new();

        // BFS / bounded DFS collecting all goal-reaching paths.
        let mut queue: VecDeque<SearchNode> = VecDeque::new();
        queue.push_back(SearchNode {
            state:     start.clone(),
            path:      Vec::new(),
            cost:      0.0,
            heuristic: start.distance_to(goal),
        });

        let mut visited: HashSet<StateKey> = HashSet::new();

        while let Some(node) = queue.pop_front() {
            if results.len() >= max_plans { break; }

            if node.state.satisfies(goal) {
                results.push((node.path.clone(), node.cost));
                continue; // don't expand further from a goal node
            }

            if node.path.len() >= max_depth { continue; }

            let key = node.state.snapshot_key();
            if visited.contains(&key) { continue; }
            visited.insert(key);

            for action in actions {
                if !action.is_applicable(&node.state) { continue; }
                let next_state = action.apply_effects(&node.state);
                let mut next_path = node.path.clone();
                next_path.push(action.name.clone());
                queue.push_back(SearchNode {
                    state:     next_state.clone(),
                    path:      next_path,
                    cost:      node.cost + action.cost,
                    heuristic: next_state.distance_to(goal),
                });
            }
        }

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results
    }
}

// ── PlanStep ──────────────────────────────────────────────────────────────────

/// The status of a single step in a running plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStepStatus {
    NotStarted,
    InProgress,
    Completed,
    Failed,
    Interrupted,
}

/// Runtime state of one action being executed.
#[derive(Debug, Clone)]
pub struct PlanStep {
    pub action_name: String,
    pub status:      PlanStepStatus,
    /// Elapsed time since this step started, in seconds.
    pub elapsed:     f32,
    /// Expected duration (from `Action::duration_secs`).
    pub duration:    f32,
}

impl PlanStep {
    fn new(action_name: &str, duration: f32) -> Self {
        Self {
            action_name: action_name.to_string(),
            status:      PlanStepStatus::NotStarted,
            elapsed:     0.0,
            duration,
        }
    }
}

// ── PlanExecutorState ─────────────────────────────────────────────────────────

/// The overall state of the plan executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorState {
    /// No plan is loaded.
    Idle,
    /// A plan is loaded and currently executing.
    Executing,
    /// The plan finished successfully (goal satisfied).
    Succeeded,
    /// A step failed and replanning is in progress.
    Replanning,
    /// No plan could be found (goal unreachable).
    Failed,
    /// The executor was explicitly interrupted.
    Interrupted,
}

// ── PlanExecutor ──────────────────────────────────────────────────────────────

/// Drives execution of a GOAP plan, applying each action's effects to the
/// simulated world state, and replanning when the actual state diverges from
/// the expected state.
///
/// # Replanning policy
///
/// After each action completes, the executor compares the **observed** world
/// state (provided by the game) against the **expected** state (computed by
/// applying planned effects).  If they differ in any condition that the next
/// action cares about, the executor discards the remainder of the plan and
/// calls the planner again.
///
/// # Interruption
///
/// An external caller can call [`PlanExecutor::interrupt`] at any time.  The
/// current step is marked `Interrupted` and the executor transitions to the
/// `Interrupted` state.  The caller may then push a new goal and call
/// [`PlanExecutor::start`] to begin fresh.
#[derive(Debug)]
pub struct PlanExecutor {
    /// The action library used for (re)planning.
    actions:           Vec<Action>,
    /// Current plan steps.
    steps:             Vec<PlanStep>,
    /// Index of the currently executing step.
    current:           usize,
    /// The simulated world state as the planner believes it to be.
    sim_state:         WorldState,
    /// The goal the executor is working toward.
    goal:              Option<WorldState>,
    /// Current executor state.
    pub state:         ExecutorState,
    /// Maximum plan depth for the A* search.
    pub max_depth:     usize,
    /// Total simulation time elapsed.
    pub sim_time:      f32,
    /// How many times the executor has replanned this goal.
    pub replan_count:  u32,
    /// Maximum replanning attempts before giving up.
    pub max_replans:   u32,
    /// Snapshot of the world state at the start of the current step, used
    /// to detect unexpected changes.
    step_start_state:  WorldState,
    /// Log of completed action names in order.
    pub history:       Vec<String>,
}

impl PlanExecutor {
    pub fn new(actions: Vec<Action>, max_depth: usize) -> Self {
        Self {
            actions,
            steps:            Vec::new(),
            current:          0,
            sim_state:        WorldState::new(),
            goal:             None,
            state:            ExecutorState::Idle,
            max_depth,
            sim_time:         0.0,
            replan_count:     0,
            max_replans:      5,
            step_start_state: WorldState::new(),
            history:          Vec::new(),
        }
    }

    /// Set the initial world state.
    pub fn set_world_state(&mut self, state: WorldState) {
        self.sim_state = state;
    }

    /// Update a single bool condition in the simulated world state.
    pub fn update_bool(&mut self, key: &str, value: bool) {
        self.sim_state.set_bool(key, value);
    }

    /// Update a single float condition in the simulated world state.
    pub fn update_float(&mut self, key: &str, value: f32) {
        self.sim_state.set_float(key, value);
    }

    /// Push a new goal and (re)plan immediately.
    ///
    /// Returns `Ok(plan_length)` on success or `Err` if no plan exists.
    pub fn start(&mut self, goal: WorldState) -> Result<usize, PlanError> {
        self.goal        = Some(goal.clone());
        self.replan_count = 0;
        self.history.clear();
        self.do_plan(&goal)
    }

    /// Interrupt the current plan.
    pub fn interrupt(&mut self) {
        if let Some(step) = self.steps.get_mut(self.current) {
            step.status = PlanStepStatus::Interrupted;
        }
        self.state = ExecutorState::Interrupted;
    }

    /// Tick the executor by `dt` seconds.
    ///
    /// `observe_state` should return the *current observed world state*,
    /// incorporating any changes that happened outside the planner (e.g.
    /// enemy died unexpectedly, health changed).
    ///
    /// Returns the name of the action that should be executing this tick, or
    /// `None` if idle/finished.
    pub fn tick(
        &mut self,
        dt:            f32,
        observe_state: impl Fn(&WorldState) -> WorldState,
    ) -> Option<&str> {
        self.sim_time += dt;

        match self.state {
            ExecutorState::Idle
            | ExecutorState::Succeeded
            | ExecutorState::Failed
            | ExecutorState::Interrupted => return None,

            ExecutorState::Replanning => {
                // Try to replan.
                if let Some(goal) = self.goal.clone() {
                    match self.do_plan(&goal) {
                        Ok(_)  => {} // state set to Executing inside do_plan
                        Err(_) => {
                            self.state = ExecutorState::Failed;
                            return None;
                        }
                    }
                } else {
                    self.state = ExecutorState::Idle;
                    return None;
                }
            }

            ExecutorState::Executing => {}
        }

        if self.current >= self.steps.len() {
            self.state = ExecutorState::Succeeded;
            return None;
        }

        {
            let step = &mut self.steps[self.current];
            if step.status == PlanStepStatus::NotStarted {
                step.status = PlanStepStatus::InProgress;
            }
            step.elapsed += dt;
        }
        self.step_start_state = self.sim_state.clone();

        // Check if the observed state has drifted from the expected state in
        // ways that invalidate the current step's preconditions.
        let observed = observe_state(&self.sim_state);
        if self.state_has_drifted(&observed) {
            // Merge observed changes into sim_state.
            self.sim_state.merge_from(&observed);
            self.steps[self.current].status = PlanStepStatus::Interrupted;

            if self.replan_count >= self.max_replans {
                self.state = ExecutorState::Failed;
                return None;
            }

            self.replan_count += 1;
            self.state = ExecutorState::Replanning;
            return Some(&self.steps[self.current].action_name);
        }

        // Merge observed state normally.
        self.sim_state.merge_from(&observed);

        // Check if the step's duration has elapsed (simple time-based
        // completion model — real games would use action callbacks).
        let step = &self.steps[self.current];
        let action_name = step.action_name.clone();
        let elapsed     = step.elapsed;
        let duration    = step.duration;

        if duration > 0.0 && elapsed < duration {
            return Some(&self.steps[self.current].action_name);
        }

        // Step completed: apply its effects to the simulated state.
        if let Some(action) = self.find_action(&action_name) {
            let effects = action.effects.clone();
            self.sim_state = self.sim_state.apply(&effects);
        }

        self.steps[self.current].status = PlanStepStatus::Completed;
        self.history.push(action_name);
        self.current += 1;

        // Check if we're done.
        if self.current >= self.steps.len() {
            self.state = ExecutorState::Succeeded;
            return None;
        }

        // Verify the next step's preconditions hold in the current sim state.
        let next_name = self.steps[self.current].action_name.clone();
        if let Some(action) = self.find_action(&next_name) {
            if !action.is_applicable(&self.sim_state) {
                if self.replan_count >= self.max_replans {
                    self.state = ExecutorState::Failed;
                    return None;
                }
                self.replan_count += 1;
                self.state = ExecutorState::Replanning;
            }
        }

        Some(&self.steps[self.current.saturating_sub(1)].action_name)
    }

    /// The name of the action currently executing, if any.
    pub fn current_action(&self) -> Option<&str> {
        if self.state == ExecutorState::Executing && self.current < self.steps.len() {
            Some(&self.steps[self.current].action_name)
        } else {
            None
        }
    }

    /// The full current plan as action names.
    pub fn plan_names(&self) -> Vec<&str> {
        self.steps.iter().map(|s| s.action_name.as_str()).collect()
    }

    /// Progress through the current plan: `(completed_steps, total_steps)`.
    pub fn progress(&self) -> (usize, usize) {
        (self.current, self.steps.len())
    }

    /// True if the executor has successfully completed its goal.
    pub fn has_succeeded(&self) -> bool { self.state == ExecutorState::Succeeded }

    /// True if the executor has permanently failed.
    pub fn has_failed(&self) -> bool { self.state == ExecutorState::Failed }

    /// Reset the executor to idle without changing the action library.
    pub fn reset(&mut self) {
        self.steps     = Vec::new();
        self.current   = 0;
        self.goal      = None;
        self.state     = ExecutorState::Idle;
        self.replan_count = 0;
        self.history.clear();
    }

    /// Access the current simulated world state.
    pub fn world_state(&self) -> &WorldState { &self.sim_state }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn do_plan(&mut self, goal: &WorldState) -> Result<usize, PlanError> {
        match GoapPlanner::plan(&self.sim_state, goal, &self.actions, self.max_depth) {
            Some(names) => {
                self.steps = names.iter().map(|n| {
                    let dur = self.find_action(n).map(|a| a.duration_secs).unwrap_or(0.0);
                    PlanStep::new(n, dur)
                }).collect();
                self.current = 0;
                self.state   = ExecutorState::Executing;
                let len = self.steps.len();
                Ok(len)
            }
            None => {
                self.state = ExecutorState::Failed;
                Err(PlanError::NoPlanFound)
            }
        }
    }

    fn find_action(&self, name: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.name == name)
    }

    /// Check if observed world state has drifted from our expected sim state
    /// in ways that affect the *next* action's preconditions.
    fn state_has_drifted(&self, observed: &WorldState) -> bool {
        // Only check conditions relevant to the current step's action.
        if self.current >= self.steps.len() { return false; }
        let name = &self.steps[self.current].action_name;
        if let Some(action) = self.find_action(name) {
            // Check bool preconditions.
            for (k, &expected) in &action.preconditions.bools {
                if observed.has_bool(k) && observed.get_bool(k) != expected {
                    return true;
                }
            }
            // Check float preconditions.
            for (k, &min) in &action.preconditions.floats_gte {
                if observed.has_float(k) && observed.get_float(k) < min {
                    return true;
                }
            }
            for (k, &max) in &action.preconditions.floats_lte {
                if observed.has_float(k) && observed.get_float(k) > max {
                    return true;
                }
            }
        }
        false
    }
}

// ── PlanError ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanError {
    /// The planner exhausted the search space with no plan found.
    NoPlanFound,
    /// The action library is empty.
    NoActions,
    /// The goal is already satisfied.
    AlreadySatisfied,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::NoPlanFound      => write!(f, "GOAP: no plan found"),
            PlanError::NoActions        => write!(f, "GOAP: action library is empty"),
            PlanError::AlreadySatisfied => write!(f, "GOAP: goal already satisfied"),
        }
    }
}

impl std::error::Error for PlanError {}

// ── ActionLibrary ─────────────────────────────────────────────────────────────

/// A named, searchable collection of [`Action`]s.
#[derive(Debug, Default, Clone)]
pub struct ActionLibrary {
    actions: Vec<Action>,
}

impl ActionLibrary {
    pub fn new() -> Self { Self::default() }

    pub fn add(&mut self, action: Action) { self.actions.push(action); }

    pub fn remove(&mut self, name: &str) {
        self.actions.retain(|a| a.name != name);
    }

    pub fn get(&self, name: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Action> {
        self.actions.iter_mut().find(|a| a.name == name)
    }

    pub fn enable(&mut self, name: &str) {
        if let Some(a) = self.get_mut(name) { a.disabled = false; }
    }

    pub fn disable(&mut self, name: &str) {
        if let Some(a) = self.get_mut(name) { a.disabled = true; }
    }

    pub fn all(&self) -> &[Action] { &self.actions }

    pub fn by_tag(&self, tag: &str) -> Vec<&Action> {
        self.actions.iter().filter(|a| a.tags.iter().any(|t| t == tag)).collect()
    }

    pub fn applicable(&self, state: &WorldState) -> Vec<&Action> {
        self.actions.iter().filter(|a| a.is_applicable(state)).collect()
    }

    pub fn plan(
        &self,
        start: &WorldState,
        goal:  &WorldState,
        max_depth: usize,
    ) -> Option<Vec<String>> {
        GoapPlanner::plan(start, goal, &self.actions, max_depth)
    }
}

// ── GoapAgent ─────────────────────────────────────────────────────────────────

/// A self-contained GOAP-driven agent that combines a goal stack, an action
/// library, and a plan executor.
///
/// Each tick:
/// 1. The goal stack is updated (goals may expire).
/// 2. If the active goal has changed, a new plan is computed.
/// 3. The executor is ticked, driving the current action.
/// 4. The name of the current action is returned for the game to execute.
#[derive(Debug)]
pub struct GoapAgent {
    pub name:     String,
    pub goals:    GoalStack,
    pub library:  ActionLibrary,
    pub executor: PlanExecutor,
    active_goal:  Option<String>,
}

impl GoapAgent {
    pub fn new(name: &str, actions: Vec<Action>, max_depth: usize) -> Self {
        let library  = ActionLibrary { actions: actions.clone() };
        let executor = PlanExecutor::new(actions, max_depth);
        Self {
            name: name.to_string(),
            goals: GoalStack::new(),
            library,
            executor,
            active_goal: None,
        }
    }

    /// Push a goal onto the agent's goal stack.
    pub fn push_goal(&mut self, goal: Goal) {
        self.goals.push(goal);
    }

    /// Set the observed world state (called from game logic each frame with
    /// fresh sensor data).
    pub fn set_world_state(&mut self, state: WorldState) {
        self.executor.set_world_state(state);
    }

    /// Update a single bool in the world state.
    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.executor.update_bool(key, value);
    }

    /// Update a single float in the world state.
    pub fn set_float(&mut self, key: &str, value: f32) {
        self.executor.update_float(key, value);
    }

    /// Tick the agent.  Returns the action name the agent wants to perform
    /// this frame, or `None` if idle.
    pub fn tick(&mut self, dt: f32) -> Option<&str> {
        self.goals.tick(dt);

        // Check if active goal has changed.
        let desired = self.goals.active().map(|g| g.name.clone());
        if desired != self.active_goal {
            self.active_goal = desired.clone();
            if let Some(goal_name) = desired {
                if let Some(goal) = self.goals.iter()
                    .find(|g| g.name == goal_name)
                    .map(|g| g.state.clone())
                {
                    // Start a new plan toward the new goal.
                    let _ = self.executor.start(goal);
                }
            } else {
                self.executor.reset();
            }
        }

        // Tick the executor.
        let world = self.executor.sim_state.clone();
        self.executor.tick(dt, |_| world.clone())
    }

    pub fn current_action(&self) -> Option<&str> { self.executor.current_action() }
    pub fn is_idle(&self)        -> bool          { self.executor.state == ExecutorState::Idle }
    pub fn has_succeeded(&self)  -> bool          { self.executor.has_succeeded() }
    pub fn has_failed(&self)     -> bool          { self.executor.has_failed()    }
    pub fn plan_names(&self)     -> Vec<&str>     { self.executor.plan_names()    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_actions() -> Vec<Action> {
        vec![
            Action::new("pick_up_weapon", 1.0)
                .require_bool("has_weapon", false)
                .effect_bool("has_weapon", true),

            Action::new("attack_enemy", 2.0)
                .require_bool("has_weapon", true)
                .require_bool("enemy_dead", false)
                .effect_bool("enemy_dead", true),

            Action::new("flee", 0.5)
                .require_bool("enemy_dead", false)
                .effect_bool("safe", true),
        ]
    }

    #[test]
    fn plan_pick_up_then_attack() {
        let mut start = WorldState::new();
        start.set_bool("has_weapon", false);
        start.set_bool("enemy_dead", false);

        let mut goal = WorldState::new();
        goal.set_bool("enemy_dead", true);

        let actions = build_test_actions();
        let plan = GoapPlanner::plan(&start, &goal, &actions, 5);
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert_eq!(plan, vec!["pick_up_weapon", "attack_enemy"]);
    }

    #[test]
    fn plan_already_satisfied() {
        let mut start = WorldState::new();
        start.set_bool("enemy_dead", true);

        let mut goal = WorldState::new();
        goal.set_bool("enemy_dead", true);

        let actions = build_test_actions();
        let plan = GoapPlanner::plan(&start, &goal, &actions, 5).unwrap();
        assert!(plan.is_empty(), "Goal already satisfied — plan should be empty");
    }

    #[test]
    fn plan_no_solution() {
        let start   = WorldState::new();
        let mut goal = WorldState::new();
        goal.set_bool("magic_flag", true);

        let actions = build_test_actions();
        let plan = GoapPlanner::plan(&start, &goal, &actions, 5);
        assert!(plan.is_none());
    }

    #[test]
    fn world_state_satisfies() {
        let mut s = WorldState::new();
        s.set_bool("a", true);
        s.set_float("hp", 80.0);

        let mut g = WorldState::new();
        g.set_bool("a", true);
        g.set_float("hp", 50.0); // requires hp >= 50

        assert!(s.satisfies(&g));

        s.set_float("hp", 30.0);
        assert!(!s.satisfies(&g));
    }

    #[test]
    fn action_effects_applied() {
        let action = Action::new("test", 1.0)
            .effect_bool("door_open", true)
            .effect_set_float("energy", 0.0)
            .effect_add_float("gold", 10.0);

        let mut state = WorldState::new();
        state.set_bool("door_open", false);
        state.set_float("energy", 100.0);
        state.set_float("gold", 5.0);

        let next = action.apply_effects(&state);
        assert_eq!(next.get_bool("door_open"),  true);
        assert_eq!(next.get_float("energy"),    0.0);
        assert_eq!(next.get_float("gold"),      15.0);
    }

    #[test]
    fn goal_stack_priority_order() {
        let mut stack = GoalStack::new();

        let mut g1 = WorldState::new(); g1.set_bool("low_priority", true);
        let mut g2 = WorldState::new(); g2.set_bool("high_priority", true);

        stack.push(Goal::new("low",  g1, 1));
        stack.push(Goal::new("high", g2, 10));

        assert_eq!(stack.active().map(|g| g.name.as_str()), Some("high"));
    }

    #[test]
    fn goal_ttl_expiry() {
        let mut stack = GoalStack::new();
        let mut g = WorldState::new(); g.set_bool("x", true);
        stack.push(Goal::new("temp", g, 1).with_ttl(0.5));

        assert!(stack.active().is_some());
        stack.tick(1.0); // advance past TTL
        assert!(stack.active().is_none());
    }

    #[test]
    fn executor_completes_plan() {
        let actions = build_test_actions();
        let mut executor = PlanExecutor::new(actions, 10);

        let mut ws = WorldState::new();
        ws.set_bool("has_weapon", false);
        ws.set_bool("enemy_dead", false);
        executor.set_world_state(ws);

        let mut goal = WorldState::new();
        goal.set_bool("enemy_dead", true);

        let result = executor.start(goal);
        assert!(result.is_ok(), "Expected a plan to be found");
        assert_eq!(result.unwrap(), 2, "Expected 2-step plan");

        let names = executor.plan_names();
        assert_eq!(names, vec!["pick_up_weapon", "attack_enemy"]);
    }

    #[test]
    fn plan_alternatives_returns_multiple() {
        let actions = build_test_actions();
        let mut start = WorldState::new();
        start.set_bool("has_weapon", false);
        start.set_bool("enemy_dead", false);

        let mut goal = WorldState::new();
        goal.set_bool("enemy_dead", true);

        let alts = GoapPlanner::plan_alternatives(&start, &goal, &actions, 5, 3);
        assert!(!alts.is_empty());
    }

    #[test]
    fn action_library_applicable() {
        let mut lib = ActionLibrary::new();
        for a in build_test_actions() { lib.add(a); }

        let mut ws = WorldState::new();
        ws.set_bool("has_weapon", false);
        ws.set_bool("enemy_dead", false);

        let applicable = lib.applicable(&ws);
        let names: Vec<&str> = applicable.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"pick_up_weapon"));
        assert!(names.contains(&"flee"));
        assert!(!names.contains(&"attack_enemy")); // needs has_weapon=true
    }

    #[test]
    fn float_precondition_plan() {
        let actions = vec![
            Action::new("heal", 1.0)
                .require_float_lte("hp", 50.0)
                .effect_set_float("hp", 100.0),
        ];

        let mut start = WorldState::new();
        start.set_float("hp", 30.0);

        let mut goal = WorldState::new();
        goal.set_float("hp", 80.0);

        let plan = GoapPlanner::plan(&start, &goal, &actions, 3);
        assert!(plan.is_some());
        assert_eq!(plan.unwrap(), vec!["heal"]);
    }
}
