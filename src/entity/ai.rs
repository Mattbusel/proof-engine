//! Entity AI — Finite State Machine, Behavior Tree, and Utility AI.
//!
//! Three complementary AI models:
//! - `StateMachine<S>`: enum-driven FSM with transition guards
//! - `BehaviorTree`: composable node tree (Sequence, Selector, Decorator)
//! - `UtilityAI`: scores candidate actions and picks the highest

use std::collections::HashMap;
use glam::Vec3;

// ── Blackboard ────────────────────────────────────────────────────────────────

/// Shared working memory for AI nodes.
#[derive(Clone, Debug, Default)]
pub struct Blackboard {
    floats:  HashMap<String, f32>,
    bools:   HashMap<String, bool>,
    vec3s:   HashMap<String, Vec3>,
    strings: HashMap<String, String>,
}

impl Blackboard {
    pub fn new() -> Self { Self::default() }

    pub fn set_float(&mut self, k: &str, v: f32)      { self.floats.insert(k.into(), v); }
    pub fn get_float(&self, k: &str) -> f32           { self.floats.get(k).copied().unwrap_or(0.0) }
    pub fn set_bool(&mut self, k: &str, v: bool)      { self.bools.insert(k.into(), v); }
    pub fn get_bool(&self, k: &str) -> bool           { self.bools.get(k).copied().unwrap_or(false) }
    pub fn set_vec3(&mut self, k: &str, v: Vec3)      { self.vec3s.insert(k.into(), v); }
    pub fn get_vec3(&self, k: &str) -> Vec3           { self.vec3s.get(k).copied().unwrap_or(Vec3::ZERO) }
    pub fn set_str(&mut self, k: &str, v: &str)       { self.strings.insert(k.into(), v.into()); }
    pub fn get_str(&self, k: &str) -> &str            { self.strings.get(k).map(|s| s.as_str()).unwrap_or("") }

    pub fn has_float(&self, k: &str) -> bool { self.floats.contains_key(k) }
    pub fn has_bool(&self, k: &str)  -> bool { self.bools.contains_key(k)  }
    pub fn has_vec3(&self, k: &str)  -> bool { self.vec3s.contains_key(k)  }

    pub fn clear(&mut self) {
        self.floats.clear();
        self.bools.clear();
        self.vec3s.clear();
        self.strings.clear();
    }
}

// ── Finite State Machine ──────────────────────────────────────────────────────

/// Outcome of a state's update tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateResult {
    /// Stay in this state.
    Continue,
    /// Transition to the given state index.
    Transition(usize),
    /// Signal that the FSM should pop (for hierarchical FSMs).
    Pop,
}

/// A single FSM state.
pub trait FsmState<Context>: Send + Sync {
    fn name(&self) -> &str;
    fn on_enter(&mut self, _ctx: &mut Context, _bb: &mut Blackboard) {}
    fn on_exit(&mut self,  _ctx: &mut Context, _bb: &mut Blackboard) {}
    fn tick(&mut self,      ctx: &mut Context,  bb: &mut Blackboard, dt: f32) -> StateResult;
}

/// A generic Finite State Machine.
pub struct StateMachine<Context> {
    states:  Vec<Box<dyn FsmState<Context>>>,
    current: usize,
    pub bb:  Blackboard,
    history: Vec<usize>,
    pub active: bool,
}

impl<Context> StateMachine<Context> {
    pub fn new() -> Self {
        Self {
            states:  Vec::new(),
            current: 0,
            bb:      Blackboard::new(),
            history: Vec::new(),
            active:  false,
        }
    }

    /// Add a state. First added = index 0.
    pub fn add_state(&mut self, state: Box<dyn FsmState<Context>>) -> usize {
        let idx = self.states.len();
        self.states.push(state);
        idx
    }

    /// Start the FSM in state `idx`.
    pub fn start(&mut self, ctx: &mut Context, idx: usize) {
        self.current = idx;
        self.active  = true;
        self.states[self.current].on_enter(ctx, &mut self.bb);
    }

    /// Tick the current state.  Handles transitions automatically.
    pub fn tick(&mut self, ctx: &mut Context, dt: f32) {
        if !self.active || self.states.is_empty() { return; }
        let result = self.states[self.current].tick(ctx, &mut self.bb, dt);
        match result {
            StateResult::Continue => {}
            StateResult::Transition(next) if next < self.states.len() && next != self.current => {
                self.states[self.current].on_exit(ctx, &mut self.bb);
                self.history.push(self.current);
                self.current = next;
                self.states[self.current].on_enter(ctx, &mut self.bb);
            }
            StateResult::Transition(_) => {}
            StateResult::Pop => {
                if let Some(prev) = self.history.pop() {
                    self.states[self.current].on_exit(ctx, &mut self.bb);
                    self.current = prev;
                    self.states[self.current].on_enter(ctx, &mut self.bb);
                } else {
                    self.active = false;
                }
            }
        }
    }

    pub fn current_state_name(&self) -> &str {
        self.states.get(self.current).map(|s| s.name()).unwrap_or("none")
    }

    pub fn current_index(&self) -> usize { self.current }
}

impl<C> Default for StateMachine<C> {
    fn default() -> Self { Self::new() }
}

// ── Behavior Tree ─────────────────────────────────────────────────────────────

/// Status returned by a BT node each tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BtStatus {
    Success,
    Failure,
    Running,
}

/// A node in a behavior tree.
pub trait BtNode: Send + Sync {
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus;
    fn reset(&mut self) {}
    fn name(&self) -> &str { "BtNode" }
}

// ── Composite nodes ───────────────────────────────────────────────────────────

/// Sequence: runs children left-to-right; fails on first child failure.
pub struct Sequence {
    children:   Vec<Box<dyn BtNode>>,
    current:    usize,
}

impl Sequence {
    pub fn new(children: Vec<Box<dyn BtNode>>) -> Self {
        Self { children, current: 0 }
    }
}

impl BtNode for Sequence {
    fn name(&self) -> &str { "Sequence" }

    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus {
        while self.current < self.children.len() {
            match self.children[self.current].tick(bb, dt) {
                BtStatus::Success => self.current += 1,
                BtStatus::Failure => { self.current = 0; return BtStatus::Failure; }
                BtStatus::Running => return BtStatus::Running,
            }
        }
        self.current = 0;
        BtStatus::Success
    }

    fn reset(&mut self) {
        self.current = 0;
        for c in &mut self.children { c.reset(); }
    }
}

/// Selector: runs children left-to-right; succeeds on first child success.
pub struct Selector {
    children: Vec<Box<dyn BtNode>>,
    current:  usize,
}

impl Selector {
    pub fn new(children: Vec<Box<dyn BtNode>>) -> Self {
        Self { children, current: 0 }
    }
}

impl BtNode for Selector {
    fn name(&self) -> &str { "Selector" }

    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus {
        while self.current < self.children.len() {
            match self.children[self.current].tick(bb, dt) {
                BtStatus::Failure => self.current += 1,
                BtStatus::Success => { self.current = 0; return BtStatus::Success; }
                BtStatus::Running => return BtStatus::Running,
            }
        }
        self.current = 0;
        BtStatus::Failure
    }

    fn reset(&mut self) {
        self.current = 0;
        for c in &mut self.children { c.reset(); }
    }
}

/// Parallel: runs all children every tick. Succeeds when `n` succeed.
pub struct Parallel {
    children:      Vec<Box<dyn BtNode>>,
    success_count: usize,
}

impl Parallel {
    pub fn new(children: Vec<Box<dyn BtNode>>, success_count: usize) -> Self {
        Self { children, success_count }
    }

    pub fn all(children: Vec<Box<dyn BtNode>>) -> Self {
        let n = children.len();
        Self::new(children, n)
    }

    pub fn any(children: Vec<Box<dyn BtNode>>) -> Self {
        Self::new(children, 1)
    }
}

impl BtNode for Parallel {
    fn name(&self) -> &str { "Parallel" }

    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus {
        let mut successes = 0;
        let mut failures  = 0;
        for child in &mut self.children {
            match child.tick(bb, dt) {
                BtStatus::Success => successes += 1,
                BtStatus::Failure => failures  += 1,
                BtStatus::Running => {}
            }
        }
        let remaining = self.children.len() - failures;
        if successes >= self.success_count { return BtStatus::Success; }
        if remaining < self.success_count  { return BtStatus::Failure; }
        BtStatus::Running
    }

    fn reset(&mut self) {
        for c in &mut self.children { c.reset(); }
    }
}

/// RandomSelector: like Selector but shuffles children on each activation.
pub struct RandomSelector {
    children: Vec<Box<dyn BtNode>>,
    order:    Vec<usize>,
    current:  usize,
    rng:      u64,
}

impl RandomSelector {
    pub fn new(children: Vec<Box<dyn BtNode>>) -> Self {
        let n = children.len();
        Self { children, order: (0..n).collect(), current: 0, rng: 9876543210 }
    }

    fn shuffle(&mut self) {
        let n = self.order.len();
        for i in (1..n).rev() {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            let j = (self.rng as usize) % (i + 1);
            self.order.swap(i, j);
        }
    }
}

impl BtNode for RandomSelector {
    fn name(&self) -> &str { "RandomSelector" }

    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus {
        if self.current == 0 { self.shuffle(); }
        while self.current < self.order.len() {
            let idx = self.order[self.current];
            match self.children[idx].tick(bb, dt) {
                BtStatus::Failure => self.current += 1,
                BtStatus::Success => { self.current = 0; return BtStatus::Success; }
                BtStatus::Running => return BtStatus::Running,
            }
        }
        self.current = 0;
        BtStatus::Failure
    }

    fn reset(&mut self) {
        self.current = 0;
        for c in &mut self.children { c.reset(); }
    }
}

// ── Decorator nodes ───────────────────────────────────────────────────────────

/// Inverter: flips Success/Failure.
pub struct Inverter { pub child: Box<dyn BtNode> }

impl BtNode for Inverter {
    fn name(&self) -> &str { "Inverter" }
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus {
        match self.child.tick(bb, dt) {
            BtStatus::Success => BtStatus::Failure,
            BtStatus::Failure => BtStatus::Success,
            BtStatus::Running => BtStatus::Running,
        }
    }
    fn reset(&mut self) { self.child.reset(); }
}

/// Succeeder: always returns Success.
pub struct Succeeder { pub child: Box<dyn BtNode> }

impl BtNode for Succeeder {
    fn name(&self) -> &str { "Succeeder" }
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus {
        self.child.tick(bb, dt);
        BtStatus::Success
    }
    fn reset(&mut self) { self.child.reset(); }
}

/// Repeater: runs child `n` times (0 = infinite).
pub struct Repeater {
    pub child:   Box<dyn BtNode>,
    pub max:     u32,
    count:       u32,
    until_fail:  bool,
}

impl Repeater {
    pub fn n_times(child: Box<dyn BtNode>, n: u32) -> Self {
        Self { child, max: n, count: 0, until_fail: false }
    }

    pub fn until_failure(child: Box<dyn BtNode>) -> Self {
        Self { child, max: 0, count: 0, until_fail: true }
    }

    pub fn forever(child: Box<dyn BtNode>) -> Self {
        Self { child, max: 0, count: 0, until_fail: false }
    }
}

impl BtNode for Repeater {
    fn name(&self) -> &str { "Repeater" }
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus {
        loop {
            let status = self.child.tick(bb, dt);
            if status == BtStatus::Running { return BtStatus::Running; }
            if self.until_fail && status == BtStatus::Failure { return BtStatus::Success; }
            self.child.reset();
            self.count += 1;
            if self.max > 0 && self.count >= self.max {
                self.count = 0;
                return BtStatus::Success;
            }
            // Infinite repeater — only run once per tick to avoid infinite loops
            if self.max == 0 { return BtStatus::Running; }
        }
    }
    fn reset(&mut self) { self.count = 0; self.child.reset(); }
}

/// Cooldown: child can only run once every `cooldown` seconds.
pub struct Cooldown {
    pub child:    Box<dyn BtNode>,
    pub cooldown: f32,
    timer:        f32,
}

impl Cooldown {
    pub fn new(child: Box<dyn BtNode>, cooldown: f32) -> Self {
        Self { child, cooldown, timer: 0.0 }
    }
}

impl BtNode for Cooldown {
    fn name(&self) -> &str { "Cooldown" }
    fn tick(&mut self, bb: &mut Blackboard, dt: f32) -> BtStatus {
        self.timer = (self.timer - dt).max(0.0);
        if self.timer > 0.0 { return BtStatus::Failure; }
        let status = self.child.tick(bb, dt);
        if status == BtStatus::Success {
            self.timer = self.cooldown;
        }
        status
    }
    fn reset(&mut self) { self.timer = 0.0; self.child.reset(); }
}

// ── Leaf nodes ────────────────────────────────────────────────────────────────

/// Always-success leaf.
pub struct AlwaysSuccess;
impl BtNode for AlwaysSuccess {
    fn name(&self) -> &str { "AlwaysSuccess" }
    fn tick(&mut self, _: &mut Blackboard, _: f32) -> BtStatus { BtStatus::Success }
}

/// Always-failure leaf.
pub struct AlwaysFailure;
impl BtNode for AlwaysFailure {
    fn name(&self) -> &str { "AlwaysFailure" }
    fn tick(&mut self, _: &mut Blackboard, _: f32) -> BtStatus { BtStatus::Failure }
}

/// Condition: reads a bool from the blackboard.
pub struct CheckFlag { pub key: String }
impl BtNode for CheckFlag {
    fn name(&self) -> &str { "CheckFlag" }
    fn tick(&mut self, bb: &mut Blackboard, _: f32) -> BtStatus {
        if bb.get_bool(&self.key) { BtStatus::Success } else { BtStatus::Failure }
    }
}

/// Condition: checks if a float exceeds a threshold.
pub struct CheckFloat { pub key: String, pub threshold: f32, pub above: bool }
impl BtNode for CheckFloat {
    fn name(&self) -> &str { "CheckFloat" }
    fn tick(&mut self, bb: &mut Blackboard, _: f32) -> BtStatus {
        let v = bb.get_float(&self.key);
        let ok = if self.above { v >= self.threshold } else { v < self.threshold };
        if ok { BtStatus::Success } else { BtStatus::Failure }
    }
}

/// Action: set a blackboard flag.
pub struct SetFlag { pub key: String, pub value: bool }
impl BtNode for SetFlag {
    fn name(&self) -> &str { "SetFlag" }
    fn tick(&mut self, bb: &mut Blackboard, _: f32) -> BtStatus {
        bb.set_bool(&self.key, self.value);
        BtStatus::Success
    }
}

/// Action: wait for `duration` seconds.
pub struct Wait { pub duration: f32, elapsed: f32 }
impl Wait {
    pub fn new(duration: f32) -> Self { Self { duration, elapsed: 0.0 } }
}
impl BtNode for Wait {
    fn name(&self) -> &str { "Wait" }
    fn tick(&mut self, _: &mut Blackboard, dt: f32) -> BtStatus {
        self.elapsed += dt;
        if self.elapsed >= self.duration {
            self.elapsed = 0.0;
            BtStatus::Success
        } else {
            BtStatus::Running
        }
    }
    fn reset(&mut self) { self.elapsed = 0.0; }
}

// ── Behavior tree root ────────────────────────────────────────────────────────

/// A complete behavior tree with its own blackboard.
pub struct BehaviorTree {
    root:    Box<dyn BtNode>,
    pub bb:  Blackboard,
    pub status: BtStatus,
}

impl BehaviorTree {
    pub fn new(root: Box<dyn BtNode>) -> Self {
        Self { root, bb: Blackboard::new(), status: BtStatus::Running }
    }

    pub fn tick(&mut self, dt: f32) -> BtStatus {
        self.status = self.root.tick(&mut self.bb, dt);
        self.status
    }

    pub fn reset(&mut self) {
        self.root.reset();
        self.status = BtStatus::Running;
    }
}

// ── Utility AI ────────────────────────────────────────────────────────────────

/// An action that utility AI can evaluate and execute.
pub trait UtilityAction: Send + Sync {
    fn name(&self) -> &str;
    /// Score this action [0, 1]. Higher = more desirable.
    fn score(&self, bb: &Blackboard) -> f32;
    /// Execute this action. Returns true if complete.
    fn execute(&mut self, bb: &mut Blackboard, dt: f32) -> bool;
    /// Reset execution state.
    fn reset(&mut self) {}
}

/// Scoring curve applied to a raw utility value.
#[derive(Clone, Copy, Debug)]
pub enum UtilityCurve {
    Linear { m: f32, b: f32 },           // y = m*x + b
    Quadratic { m: f32, k: f32, b: f32 }, // y = m*(x-k)^2 + b
    Logistic { k: f32, x0: f32 },         // sigmoid
    Exponential { k: f32 },               // e^(k*x)
    Constant(f32),
}

impl UtilityCurve {
    pub fn evaluate(&self, x: f32) -> f32 {
        let y = match self {
            UtilityCurve::Linear { m, b }         => m * x + b,
            UtilityCurve::Quadratic { m, k, b }   => m * (x - k).powi(2) + b,
            UtilityCurve::Logistic { k, x0 }      => 1.0 / (1.0 + (-k * (x - x0)).exp()),
            UtilityCurve::Exponential { k }        => (k * x).exp().min(1.0),
            UtilityCurve::Constant(c)              => *c,
        };
        y.clamp(0.0, 1.0)
    }
}

/// Consideration: maps a blackboard value through a curve to a partial score.
pub struct Consideration {
    pub name:   String,
    pub key:    String,   // blackboard key
    pub min:    f32,
    pub max:    f32,
    pub curve:  UtilityCurve,
    pub weight: f32,
}

impl Consideration {
    pub fn new(name: &str, key: &str, min: f32, max: f32, curve: UtilityCurve) -> Self {
        Self { name: name.into(), key: key.into(), min, max, curve, weight: 1.0 }
    }

    pub fn with_weight(mut self, w: f32) -> Self { self.weight = w; self }

    pub fn evaluate(&self, bb: &Blackboard) -> f32 {
        let raw = bb.get_float(&self.key);
        let t   = ((raw - self.min) / (self.max - self.min).max(f32::EPSILON)).clamp(0.0, 1.0);
        self.curve.evaluate(t) * self.weight
    }
}

/// An action defined by its considerations and execution function.
pub struct UtilityActionDef {
    pub name:           String,
    pub considerations: Vec<Consideration>,
    /// How to combine multiple consideration scores.
    pub combine:        ConsiderationCombine,
    /// Cooldown timer.
    pub cooldown:       f32,
    cooldown_timer:     f32,
    /// Custom execution callback.
    execute_fn:         Box<dyn Fn(&mut Blackboard, f32) -> bool + Send + Sync>,
    elapsed:            f32,
    pub max_duration:   f32,
}

#[derive(Clone, Copy, Debug)]
pub enum ConsiderationCombine {
    /// Multiply all scores (any 0 = disqualified).
    Multiply,
    /// Average all scores.
    Average,
    /// Minimum score.
    Min,
    /// Maximum score.
    Max,
}

impl UtilityActionDef {
    pub fn new(
        name: &str,
        considerations: Vec<Consideration>,
        execute_fn: impl Fn(&mut Blackboard, f32) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            considerations,
            combine: ConsiderationCombine::Multiply,
            cooldown: 0.0,
            cooldown_timer: 0.0,
            execute_fn: Box::new(execute_fn),
            elapsed: 0.0,
            max_duration: f32::MAX,
        }
    }

    pub fn with_cooldown(mut self, c: f32) -> Self { self.cooldown = c; self }
    pub fn with_max_duration(mut self, d: f32) -> Self { self.max_duration = d; self }
    pub fn with_combine(mut self, c: ConsiderationCombine) -> Self { self.combine = c; self }
}

impl UtilityAction for UtilityActionDef {
    fn name(&self) -> &str { &self.name }

    fn score(&self, bb: &Blackboard) -> f32 {
        if self.cooldown_timer > 0.0 { return 0.0; }
        if self.considerations.is_empty() { return 0.5; }

        let scores: Vec<f32> = self.considerations.iter().map(|c| c.evaluate(bb)).collect();
        match self.combine {
            ConsiderationCombine::Multiply => scores.iter().product(),
            ConsiderationCombine::Average  => scores.iter().sum::<f32>() / scores.len() as f32,
            ConsiderationCombine::Min      => scores.iter().cloned().fold(f32::MAX, f32::min),
            ConsiderationCombine::Max      => scores.iter().cloned().fold(0.0_f32, f32::max),
        }
    }

    fn execute(&mut self, bb: &mut Blackboard, dt: f32) -> bool {
        self.elapsed += dt;
        let done = (self.execute_fn)(bb, dt) || self.elapsed >= self.max_duration;
        if done {
            self.cooldown_timer = self.cooldown;
            self.elapsed = 0.0;
        }
        done
    }

    fn reset(&mut self) { self.elapsed = 0.0; }
}

/// Selects the highest-scoring available action and runs it.
pub struct UtilityAI {
    actions:        Vec<Box<dyn UtilityAction>>,
    pub bb:         Blackboard,
    current_action: Option<usize>,
    /// Re-evaluate scores every `reeval_interval` seconds.
    pub reeval_interval: f32,
    reeval_timer:   f32,
    /// Inertia: current action needs to be beaten by this margin to switch.
    pub inertia:    f32,
}

impl UtilityAI {
    pub fn new() -> Self {
        Self {
            actions:        Vec::new(),
            bb:             Blackboard::new(),
            current_action: None,
            reeval_interval: 0.1,
            reeval_timer:   0.0,
            inertia:        0.05,
        }
    }

    pub fn add_action(&mut self, action: Box<dyn UtilityAction>) {
        self.actions.push(action);
    }

    pub fn tick(&mut self, dt: f32) {
        self.reeval_timer -= dt;

        // Update cooldown timers
        // (UtilityActionDef handles its own timers internally)

        // Re-evaluate
        let should_reeval = self.reeval_timer <= 0.0;
        if should_reeval { self.reeval_timer = self.reeval_interval; }

        if should_reeval || self.current_action.is_none() {
            let bb         = &self.bb;
            let inertia    = self.inertia;
            let current    = self.current_action;
            let mut best_score = -1.0_f32;
            let mut best_idx   = None;

            for (i, action) in self.actions.iter().enumerate() {
                let mut score = action.score(bb);
                // Boost current action by inertia to prevent thrashing
                if Some(i) == current { score += inertia; }
                if score > best_score {
                    best_score = score;
                    best_idx   = Some(i);
                }
            }

            if best_idx != self.current_action {
                if let Some(old) = self.current_action {
                    self.actions[old].reset();
                }
                self.current_action = best_idx;
            }
        }

        // Execute current action
        if let Some(idx) = self.current_action {
            let bb = &mut self.bb;
            let done = self.actions[idx].execute(bb, dt);
            if done {
                self.current_action = None;
            }
        }
    }

    pub fn current_action_name(&self) -> Option<&str> {
        self.current_action.and_then(|i| self.actions.get(i)).map(|a| a.name())
    }

    pub fn scores(&self) -> Vec<(&str, f32)> {
        self.actions.iter().map(|a| (a.name(), a.score(&self.bb))).collect()
    }
}

impl Default for UtilityAI {
    fn default() -> Self { Self::new() }
}

// ── Common AI contexts ────────────────────────────────────────────────────────

/// Standard blackboard keys used by built-in behaviors.
pub mod keys {
    pub const HEALTH:           &str = "health";
    pub const MAX_HEALTH:       &str = "max_health";
    pub const DISTANCE_TO_PLAYER: &str = "dist_player";
    pub const DISTANCE_TO_COVER:  &str = "dist_cover";
    pub const AMMO:             &str = "ammo";
    pub const IN_COVER:         &str = "in_cover";
    pub const PLAYER_VISIBLE:   &str = "player_visible";
    pub const TARGET_POS:       &str = "target_pos";
    pub const SELF_POS:         &str = "self_pos";
    pub const ALERT_LEVEL:      &str = "alert_level";
    pub const AGGRESSION:       &str = "aggression";
    pub const CAN_ATTACK:       &str = "can_attack";
    pub const IS_FLEEING:       &str = "is_fleeing";
    pub const TIME_IN_STATE:    &str = "time_in_state";
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Blackboard ──

    #[test]
    fn blackboard_set_get() {
        let mut bb = Blackboard::new();
        bb.set_float("hp", 80.0);
        bb.set_bool("alive", true);
        bb.set_vec3("pos", Vec3::new(1.0, 2.0, 3.0));
        assert!((bb.get_float("hp") - 80.0).abs() < 1e-5);
        assert!(bb.get_bool("alive"));
        assert_eq!(bb.get_vec3("pos"), Vec3::new(1.0, 2.0, 3.0));
    }

    // ── BT nodes ──

    #[test]
    fn sequence_succeeds_when_all_succeed() {
        let mut seq = Sequence::new(vec![
            Box::new(AlwaysSuccess),
            Box::new(AlwaysSuccess),
        ]);
        let mut bb = Blackboard::new();
        assert_eq!(seq.tick(&mut bb, 0.016), BtStatus::Success);
    }

    #[test]
    fn sequence_fails_on_child_failure() {
        let mut seq = Sequence::new(vec![
            Box::new(AlwaysSuccess),
            Box::new(AlwaysFailure),
            Box::new(AlwaysSuccess),
        ]);
        let mut bb = Blackboard::new();
        assert_eq!(seq.tick(&mut bb, 0.016), BtStatus::Failure);
    }

    #[test]
    fn selector_succeeds_on_first_success() {
        let mut sel = Selector::new(vec![
            Box::new(AlwaysFailure),
            Box::new(AlwaysSuccess),
        ]);
        let mut bb = Blackboard::new();
        assert_eq!(sel.tick(&mut bb, 0.016), BtStatus::Success);
    }

    #[test]
    fn selector_fails_when_all_fail() {
        let mut sel = Selector::new(vec![
            Box::new(AlwaysFailure),
            Box::new(AlwaysFailure),
        ]);
        let mut bb = Blackboard::new();
        assert_eq!(sel.tick(&mut bb, 0.016), BtStatus::Failure);
    }

    #[test]
    fn inverter_flips_success() {
        let mut inv = Inverter { child: Box::new(AlwaysSuccess) };
        let mut bb = Blackboard::new();
        assert_eq!(inv.tick(&mut bb, 0.016), BtStatus::Failure);
    }

    #[test]
    fn wait_runs_and_completes() {
        let mut w  = Wait::new(0.1);
        let mut bb = Blackboard::new();
        assert_eq!(w.tick(&mut bb, 0.05), BtStatus::Running);
        assert_eq!(w.tick(&mut bb, 0.06), BtStatus::Success);
    }

    #[test]
    fn check_flag_reads_blackboard() {
        let mut bb = Blackboard::new();
        bb.set_bool("alive", true);
        let mut node = CheckFlag { key: "alive".into() };
        assert_eq!(node.tick(&mut bb, 0.016), BtStatus::Success);
        bb.set_bool("alive", false);
        assert_eq!(node.tick(&mut bb, 0.016), BtStatus::Failure);
    }

    #[test]
    fn check_float_threshold() {
        let mut bb = Blackboard::new();
        bb.set_float("hp", 20.0);
        let mut node = CheckFloat { key: "hp".into(), threshold: 50.0, above: false };
        assert_eq!(node.tick(&mut bb, 0.016), BtStatus::Success); // 20 < 50
    }

    #[test]
    fn cooldown_blocks_repeat() {
        let mut cd = Cooldown::new(Box::new(AlwaysSuccess), 1.0);
        let mut bb = Blackboard::new();
        assert_eq!(cd.tick(&mut bb, 0.016), BtStatus::Success);  // fires
        assert_eq!(cd.tick(&mut bb, 0.016), BtStatus::Failure);  // on cooldown
        // Advance past cooldown
        cd.tick(&mut bb, 1.0);
        assert_eq!(cd.tick(&mut bb, 0.016), BtStatus::Success);  // fires again
    }

    // ── Utility AI ──

    #[test]
    fn utility_curve_linear() {
        let c = UtilityCurve::Linear { m: 1.0, b: 0.0 };
        assert!((c.evaluate(0.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn utility_curve_clamps() {
        let c = UtilityCurve::Linear { m: 2.0, b: 0.0 };
        assert!((c.evaluate(1.0) - 1.0).abs() < 1e-5); // clamped to 1
        assert!((c.evaluate(-1.0) - 0.0).abs() < 1e-5); // clamped to 0
    }

    #[test]
    fn utility_ai_selects_best() {
        let mut ai = UtilityAI::new();
        // Action A scores 0.9 if "prefer_a" is true
        ai.add_action(Box::new(UtilityActionDef::new(
            "action_a",
            vec![Consideration::new("pref_a", "prefer_a",
                0.0, 1.0, UtilityCurve::Linear { m: 1.0, b: 0.0 })],
            |_, _| true,
        )));
        // Action B always scores 0.1
        ai.add_action(Box::new(UtilityActionDef::new(
            "action_b",
            vec![Consideration::new("const", "dummy",
                0.0, 1.0, UtilityCurve::Constant(0.1))],
            |_, _| true,
        )));

        ai.bb.set_float("prefer_a", 0.9);
        ai.bb.set_float("dummy", 0.5);
        ai.tick(0.016);

        assert_eq!(ai.current_action_name(), Some("action_a"));
    }
}
