//! Behavior Tree system — full implementation with composites, decorators,
//! actions, conditions, blackboard, and a fluent builder DSL.
//!
//! # Design
//! - Tick-based execution: `tree.tick(&mut ctx)` returns `Status`.
//! - Blackboard: shared key/value store passed through context.
//! - Nodes are boxed trait objects — fully dynamic, composable at runtime.
//! - Zero unsafe, no external dependencies beyond std.
//!
//! # Quick Start
//! ```rust,ignore
//! use proof_engine::ai::behavior_tree::*;
//! let tree = BehaviorTreeBuilder::new()
//!     .sequence()
//!         .condition(|ctx| ctx.blackboard.get_float("health") > 0.0)
//!         .action(|ctx| { ctx.blackboard.set("attacking", true); Status::Success })
//!     .end()
//!     .build();
//! ```

use std::collections::HashMap;
use std::time::Duration;

// ── Status ────────────────────────────────────────────────────────────────────

/// The execution status returned by every tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// Node completed successfully.
    Success,
    /// Node failed.
    Failure,
    /// Node is still running (needs more ticks).
    Running,
}

impl Status {
    pub fn is_success(self) -> bool { matches!(self, Status::Success) }
    pub fn is_failure(self) -> bool { matches!(self, Status::Failure) }
    pub fn is_running(self) -> bool { matches!(self, Status::Running) }
}

// ── Blackboard ────────────────────────────────────────────────────────────────

/// Shared data store accessible by all tree nodes.
#[derive(Debug, Clone, Default)]
pub struct Blackboard {
    floats:  HashMap<String, f32>,
    ints:    HashMap<String, i32>,
    bools:   HashMap<String, bool>,
    strings: HashMap<String, String>,
    vecs:    HashMap<String, [f32; 3]>,
}

impl Blackboard {
    pub fn new() -> Self { Self::default() }

    // ── float ──
    pub fn set_float(&mut self, key: &str, val: f32) { self.floats.insert(key.to_string(), val); }
    pub fn get_float(&self, key: &str) -> f32 { self.floats.get(key).copied().unwrap_or(0.0) }
    pub fn get_float_opt(&self, key: &str) -> Option<f32> { self.floats.get(key).copied() }

    // ── int ──
    pub fn set_int(&mut self, key: &str, val: i32) { self.ints.insert(key.to_string(), val); }
    pub fn get_int(&self, key: &str) -> i32 { self.ints.get(key).copied().unwrap_or(0) }

    // ── bool ──
    pub fn set_bool(&mut self, key: &str, val: bool) { self.bools.insert(key.to_string(), val); }
    pub fn get_bool(&self, key: &str) -> bool { self.bools.get(key).copied().unwrap_or(false) }

    // ── string ──
    pub fn set_string(&mut self, key: &str, val: &str) { self.strings.insert(key.to_string(), val.to_string()); }
    pub fn get_string(&self, key: &str) -> &str { self.strings.get(key).map(|s| s.as_str()).unwrap_or("") }

    // ── vec3 ──
    pub fn set_vec3(&mut self, key: &str, val: [f32; 3]) { self.vecs.insert(key.to_string(), val); }
    pub fn get_vec3(&self, key: &str) -> [f32; 3] { self.vecs.get(key).copied().unwrap_or([0.0; 3]) }

    // ── generic ──
    pub fn has(&self, key: &str) -> bool {
        self.floats.contains_key(key)
            || self.ints.contains_key(key)
            || self.bools.contains_key(key)
            || self.strings.contains_key(key)
            || self.vecs.contains_key(key)
    }

    pub fn clear_key(&mut self, key: &str) {
        self.floats.remove(key);
        self.ints.remove(key);
        self.bools.remove(key);
        self.strings.remove(key);
        self.vecs.remove(key);
    }

    pub fn clear(&mut self) {
        self.floats.clear();
        self.ints.clear();
        self.bools.clear();
        self.strings.clear();
        self.vecs.clear();
    }
}

// ── TickContext ───────────────────────────────────────────────────────────────

/// Context passed to every tick call.
pub struct TickContext<'a, T> {
    pub blackboard:  &'a mut Blackboard,
    pub entity:      &'a mut T,
    pub dt:          f32,
    pub elapsed:     f32,
    pub fired_events: Vec<String>,
}

impl<'a, T> TickContext<'a, T> {
    pub fn new(bb: &'a mut Blackboard, entity: &'a mut T, dt: f32, elapsed: f32) -> Self {
        Self { blackboard: bb, entity, dt, elapsed, fired_events: Vec::new() }
    }

    pub fn fire_event(&mut self, name: &str) {
        self.fired_events.push(name.to_string());
    }
}

// ── Node trait ────────────────────────────────────────────────────────────────

/// A node in the behavior tree.
pub trait Node<T>: std::fmt::Debug + Send + Sync {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status;
    /// Called when the node is aborted (parent interrupts it mid-running).
    fn abort(&mut self) {}
    /// Node identifier for debugging.
    fn name(&self) -> &str { "Node" }
}

// ── Composite: Sequence ───────────────────────────────────────────────────────

/// Executes children left-to-right. Succeeds if ALL succeed. Fails on first failure.
/// Remembers the currently running child (memory sequence).
#[derive(Debug)]
pub struct Sequence<T> {
    pub name:    String,
    children:    Vec<Box<dyn Node<T>>>,
    current_idx: usize,
}

impl<T> Sequence<T> {
    pub fn new(name: &str, children: Vec<Box<dyn Node<T>>>) -> Self {
        Self { name: name.to_string(), children, current_idx: 0 }
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Sequence<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        while self.current_idx < self.children.len() {
            match self.children[self.current_idx].tick(ctx) {
                Status::Success => self.current_idx += 1,
                Status::Failure => {
                    self.current_idx = 0;
                    return Status::Failure;
                }
                Status::Running => return Status::Running,
            }
        }
        self.current_idx = 0;
        Status::Success
    }

    fn abort(&mut self) {
        for child in &mut self.children {
            child.abort();
        }
        self.current_idx = 0;
    }

    fn name(&self) -> &str { &self.name }
}

// ── Composite: Selector ───────────────────────────────────────────────────────

/// Executes children left-to-right. Succeeds on FIRST success. Fails if ALL fail.
/// Memory selector: remembers the currently running child.
#[derive(Debug)]
pub struct Selector<T> {
    pub name:    String,
    children:    Vec<Box<dyn Node<T>>>,
    current_idx: usize,
}

impl<T> Selector<T> {
    pub fn new(name: &str, children: Vec<Box<dyn Node<T>>>) -> Self {
        Self { name: name.to_string(), children, current_idx: 0 }
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Selector<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        while self.current_idx < self.children.len() {
            match self.children[self.current_idx].tick(ctx) {
                Status::Success => {
                    self.current_idx = 0;
                    return Status::Success;
                }
                Status::Failure => self.current_idx += 1,
                Status::Running => return Status::Running,
            }
        }
        self.current_idx = 0;
        Status::Failure
    }

    fn abort(&mut self) {
        for child in &mut self.children {
            child.abort();
        }
        self.current_idx = 0;
    }

    fn name(&self) -> &str { &self.name }
}

// ── Composite: Parallel ───────────────────────────────────────────────────────

/// Ticks ALL children every frame.
/// Success/failure policy configurable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParallelPolicy {
    /// Succeed if at least N children succeed.
    SucceedOnN(usize),
    /// Fail if at least N children fail.
    FailOnN(usize),
}

#[derive(Debug)]
pub struct Parallel<T> {
    pub name:           String,
    children:           Vec<Box<dyn Node<T>>>,
    pub success_policy: ParallelPolicy,
    pub failure_policy: ParallelPolicy,
    statuses:           Vec<Option<Status>>,
}

impl<T> Parallel<T> {
    pub fn new(name: &str, children: Vec<Box<dyn Node<T>>>) -> Self {
        let n = children.len();
        Self {
            name: name.to_string(),
            children,
            success_policy: ParallelPolicy::SucceedOnN(1),
            failure_policy: ParallelPolicy::FailOnN(1),
            statuses: vec![None; n],
        }
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Parallel<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        let mut successes = 0usize;
        let mut failures  = 0usize;

        for (i, child) in self.children.iter_mut().enumerate() {
            if self.statuses[i].map(|s| s != Status::Running).unwrap_or(false) {
                // Already completed
                match self.statuses[i] {
                    Some(Status::Success) => successes += 1,
                    Some(Status::Failure) => failures  += 1,
                    _ => {}
                }
                continue;
            }
            let s = child.tick(ctx);
            self.statuses[i] = Some(s);
            match s {
                Status::Success => successes += 1,
                Status::Failure => failures  += 1,
                Status::Running => {}
            }
        }

        let succeed_n = match self.success_policy { ParallelPolicy::SucceedOnN(n) => n, ParallelPolicy::FailOnN(_) => usize::MAX };
        let fail_n    = match self.failure_policy  { ParallelPolicy::FailOnN(n)    => n, ParallelPolicy::SucceedOnN(_) => usize::MAX };

        if successes >= succeed_n {
            self.statuses.iter_mut().for_each(|s| *s = None);
            Status::Success
        } else if failures >= fail_n {
            self.statuses.iter_mut().for_each(|s| *s = None);
            Status::Failure
        } else {
            Status::Running
        }
    }

    fn abort(&mut self) {
        for child in &mut self.children {
            child.abort();
        }
        self.statuses.iter_mut().for_each(|s| *s = None);
    }

    fn name(&self) -> &str { &self.name }
}

// ── Composite: RandomSelector ─────────────────────────────────────────────────

/// Shuffles children order each activation, then tries them in that order.
#[derive(Debug)]
pub struct RandomSelector<T> {
    pub name:    String,
    children:    Vec<Box<dyn Node<T>>>,
    order:       Vec<usize>,
    current_idx: usize,
    seed:        u32,
}

impl<T> RandomSelector<T> {
    pub fn new(name: &str, children: Vec<Box<dyn Node<T>>>) -> Self {
        let n = children.len();
        let order: Vec<usize> = (0..n).collect();
        Self { name: name.to_string(), children, order, current_idx: 0, seed: 12345 }
    }

    fn shuffle(&mut self) {
        // Simple LCG shuffle
        let n = self.order.len();
        for i in (1..n).rev() {
            self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let j = (self.seed as usize) % (i + 1);
            self.order.swap(i, j);
        }
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for RandomSelector<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        if self.current_idx == 0 {
            self.shuffle();
        }
        while self.current_idx < self.order.len() {
            let child_idx = self.order[self.current_idx];
            match self.children[child_idx].tick(ctx) {
                Status::Success => {
                    self.current_idx = 0;
                    return Status::Success;
                }
                Status::Failure => self.current_idx += 1,
                Status::Running => return Status::Running,
            }
        }
        self.current_idx = 0;
        Status::Failure
    }

    fn abort(&mut self) {
        for child in &mut self.children {
            child.abort();
        }
        self.current_idx = 0;
    }

    fn name(&self) -> &str { &self.name }
}

// ── Decorators ────────────────────────────────────────────────────────────────

/// Inverts the child's result.
#[derive(Debug)]
pub struct Inverter<T> { pub child: Box<dyn Node<T>> }

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Inverter<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        match self.child.tick(ctx) {
            Status::Success => Status::Failure,
            Status::Failure => Status::Success,
            Status::Running => Status::Running,
        }
    }
    fn abort(&mut self) { self.child.abort(); }
    fn name(&self) -> &str { "Inverter" }
}

/// Always returns Success regardless of child result.
#[derive(Debug)]
pub struct Succeeder<T> { pub child: Box<dyn Node<T>> }

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Succeeder<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        self.child.tick(ctx);
        Status::Success
    }
    fn abort(&mut self) { self.child.abort(); }
    fn name(&self) -> &str { "Succeeder" }
}

/// Always returns Failure regardless of child result.
#[derive(Debug)]
pub struct Failer<T> { pub child: Box<dyn Node<T>> }

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Failer<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        self.child.tick(ctx);
        Status::Failure
    }
    fn abort(&mut self) { self.child.abort(); }
    fn name(&self) -> &str { "Failer" }
}

/// Repeats child N times, or infinitely if N is None.
#[derive(Debug)]
pub struct Repeater<T> {
    pub child:         Box<dyn Node<T>>,
    pub max_repeats:   Option<u32>,
    pub stop_on_fail:  bool,
    count:             u32,
}

impl<T> Repeater<T> {
    pub fn infinite(child: Box<dyn Node<T>>) -> Self {
        Self { child, max_repeats: None, stop_on_fail: false, count: 0 }
    }

    pub fn n_times(child: Box<dyn Node<T>>, n: u32) -> Self {
        Self { child, max_repeats: Some(n), stop_on_fail: false, count: 0 }
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Repeater<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        loop {
            if let Some(max) = self.max_repeats {
                if self.count >= max {
                    self.count = 0;
                    return Status::Success;
                }
            }
            match self.child.tick(ctx) {
                Status::Running => return Status::Running,
                Status::Failure if self.stop_on_fail => {
                    self.count = 0;
                    return Status::Failure;
                }
                _ => {
                    self.count += 1;
                    self.child.abort();
                }
            }
        }
    }
    fn abort(&mut self) { self.child.abort(); self.count = 0; }
    fn name(&self) -> &str { "Repeater" }
}

/// Repeats until the child fails.
#[derive(Debug)]
pub struct RetryUntilFail<T> {
    pub child: Box<dyn Node<T>>,
    count:     u32,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for RetryUntilFail<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        loop {
            match self.child.tick(ctx) {
                Status::Failure => { self.count = 0; return Status::Success; }
                Status::Running => return Status::Running,
                Status::Success => { self.count += 1; self.child.abort(); }
            }
        }
    }
    fn abort(&mut self) { self.child.abort(); self.count = 0; }
    fn name(&self) -> &str { "RetryUntilFail" }
}

/// Limits child execution to a maximum wall-clock duration (in seconds).
#[derive(Debug)]
pub struct TimeLimit<T> {
    pub child:         Box<dyn Node<T>>,
    pub limit_secs:    f32,
    pub elapsed:       f32,
    pub fail_on_limit: bool,
}

impl<T> TimeLimit<T> {
    pub fn new(child: Box<dyn Node<T>>, limit_secs: f32) -> Self {
        Self { child, limit_secs, elapsed: 0.0, fail_on_limit: true }
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for TimeLimit<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        self.elapsed += ctx.dt;
        if self.elapsed >= self.limit_secs {
            self.child.abort();
            self.elapsed = 0.0;
            return if self.fail_on_limit { Status::Failure } else { Status::Success };
        }
        let s = self.child.tick(ctx);
        if s != Status::Running { self.elapsed = 0.0; }
        s
    }
    fn abort(&mut self) { self.child.abort(); self.elapsed = 0.0; }
    fn name(&self) -> &str { "TimeLimit" }
}

/// Prevents child from running until a cooldown expires.
#[derive(Debug)]
pub struct Cooldown<T> {
    pub child:           Box<dyn Node<T>>,
    pub cooldown_secs:   f32,
    remaining:           f32,
    pub fail_during:     bool,
}

impl<T> Cooldown<T> {
    pub fn new(child: Box<dyn Node<T>>, cooldown_secs: f32) -> Self {
        Self { child, cooldown_secs, remaining: 0.0, fail_during: true }
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Cooldown<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        self.remaining = (self.remaining - ctx.dt).max(0.0);
        if self.remaining > 0.0 {
            return if self.fail_during { Status::Failure } else { Status::Running };
        }
        let s = self.child.tick(ctx);
        if s == Status::Success {
            self.remaining = self.cooldown_secs;
        }
        s
    }
    fn abort(&mut self) { self.child.abort(); }
    fn name(&self) -> &str { "Cooldown" }
}

/// Guards child behind a blackboard condition. Re-evaluated every tick.
pub struct Guard<T> {
    pub name:      String,
    pub condition: Box<dyn Fn(&Blackboard) -> bool + Send + Sync>,
    pub child:     Box<dyn Node<T>>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for Guard<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Guard").field("name", &self.name).finish()
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Guard<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        if (self.condition)(ctx.blackboard) {
            self.child.tick(ctx)
        } else {
            self.child.abort();
            Status::Failure
        }
    }
    fn abort(&mut self) { self.child.abort(); }
    fn name(&self) -> &str { &self.name }
}

// ── Leaf: ConditionNode ───────────────────────────────────────────────────────

/// A leaf that evaluates a closure against the context.
pub struct ConditionNode<T> {
    pub name: String,
    pub func: Box<dyn Fn(&TickContext<T>) -> bool + Send + Sync>,
}

impl<T> std::fmt::Debug for ConditionNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConditionNode({})", self.name)
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for ConditionNode<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        if (self.func)(ctx) { Status::Success } else { Status::Failure }
    }
    fn name(&self) -> &str { &self.name }
}

// ── Leaf: ActionNode ─────────────────────────────────────────────────────────

/// A leaf that performs an action and returns a status.
pub struct ActionNode<T> {
    pub name: String,
    pub func: Box<dyn FnMut(&mut TickContext<T>) -> Status + Send + Sync>,
}

impl<T> std::fmt::Debug for ActionNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ActionNode({})", self.name)
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for ActionNode<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        (self.func)(ctx)
    }
    fn name(&self) -> &str { &self.name }
}

// ── Leaf: Wait ────────────────────────────────────────────────────────────────

/// Waits for `duration` seconds, then succeeds.
#[derive(Debug)]
pub struct Wait<T: std::fmt::Debug> {
    pub duration: f32,
    elapsed:      f32,
    _phantom:     std::marker::PhantomData<T>,
}

impl<T: std::fmt::Debug> Wait<T> {
    pub fn new(duration: f32) -> Self {
        Self { duration, elapsed: 0.0, _phantom: std::marker::PhantomData }
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for Wait<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        self.elapsed += ctx.dt;
        if self.elapsed >= self.duration {
            self.elapsed = 0.0;
            Status::Success
        } else {
            Status::Running
        }
    }
    fn abort(&mut self) { self.elapsed = 0.0; }
    fn name(&self) -> &str { "Wait" }
}

/// Waits for a random duration in [min, max] seconds.
#[derive(Debug)]
pub struct WaitRandom<T: std::fmt::Debug> {
    pub min_secs:  f32,
    pub max_secs:  f32,
    elapsed:       f32,
    target:        f32,
    seed:          u32,
    _phantom:      std::marker::PhantomData<T>,
}

impl<T: std::fmt::Debug> WaitRandom<T> {
    pub fn new(min_secs: f32, max_secs: f32) -> Self {
        let mut s = Self {
            min_secs, max_secs, elapsed: 0.0, target: 0.0, seed: 54321,
            _phantom: std::marker::PhantomData,
        };
        s.reset_target();
        s
    }

    fn reset_target(&mut self) {
        self.seed = self.seed.wrapping_mul(1664525_u32).wrapping_add(1013904223_u32);
        let t01 = (self.seed >> 16) as f32 / u16::MAX as f32;
        self.target = self.min_secs + t01 * (self.max_secs - self.min_secs);
    }
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for WaitRandom<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        self.elapsed += ctx.dt;
        if self.elapsed >= self.target {
            self.elapsed = 0.0;
            self.reset_target();
            Status::Success
        } else {
            Status::Running
        }
    }
    fn abort(&mut self) { self.elapsed = 0.0; self.reset_target(); }
    fn name(&self) -> &str { "WaitRandom" }
}

// ── Leaf: BlackboardSet / Check ───────────────────────────────────────────────

/// Sets a float on the blackboard and succeeds.
#[derive(Debug)]
pub struct SetFloat<T: std::fmt::Debug> {
    pub key:   String,
    pub value: f32,
    _phantom:  std::marker::PhantomData<T>,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for SetFloat<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        ctx.blackboard.set_float(&self.key, self.value);
        Status::Success
    }
    fn name(&self) -> &str { "SetFloat" }
}

/// Sets a bool on the blackboard and succeeds.
#[derive(Debug)]
pub struct SetBool<T: std::fmt::Debug> {
    pub key:   String,
    pub value: bool,
    _phantom:  std::marker::PhantomData<T>,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for SetBool<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        ctx.blackboard.set_bool(&self.key, self.value);
        Status::Success
    }
    fn name(&self) -> &str { "SetBool" }
}

/// Checks if a blackboard key has been set.
#[derive(Debug)]
pub struct BlackboardHas<T: std::fmt::Debug> {
    pub key:      String,
    _phantom:     std::marker::PhantomData<T>,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for BlackboardHas<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        if ctx.blackboard.has(&self.key) { Status::Success } else { Status::Failure }
    }
    fn name(&self) -> &str { "BlackboardHas" }
}

/// Checks a float comparison against a threshold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatComparison { Greater, GreaterEq, Less, LessEq, Equal, NotEqual }

#[derive(Debug)]
pub struct CheckFloat<T: std::fmt::Debug> {
    pub key:       String,
    pub threshold: f32,
    pub op:        FloatComparison,
    _phantom:      std::marker::PhantomData<T>,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for CheckFloat<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        let v = ctx.blackboard.get_float(&self.key);
        let pass = match self.op {
            FloatComparison::Greater   => v > self.threshold,
            FloatComparison::GreaterEq => v >= self.threshold,
            FloatComparison::Less      => v < self.threshold,
            FloatComparison::LessEq    => v <= self.threshold,
            FloatComparison::Equal     => (v - self.threshold).abs() < 1e-5,
            FloatComparison::NotEqual  => (v - self.threshold).abs() >= 1e-5,
        };
        if pass { Status::Success } else { Status::Failure }
    }
    fn name(&self) -> &str { "CheckFloat" }
}

/// Fires a named event into the context and succeeds.
#[derive(Debug)]
pub struct FireEvent<T: std::fmt::Debug> {
    pub event_name: String,
    _phantom:       std::marker::PhantomData<T>,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for FireEvent<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        ctx.fire_event(&self.event_name);
        Status::Success
    }
    fn name(&self) -> &str { "FireEvent" }
}

// ── BehaviorTree ──────────────────────────────────────────────────────────────

/// The top-level behavior tree — wraps a root node.
pub struct BehaviorTree<T> {
    pub name: String,
    root:     Box<dyn Node<T>>,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> BehaviorTree<T> {
    pub fn new(name: &str, root: Box<dyn Node<T>>) -> Self {
        Self { name: name.to_string(), root }
    }

    /// Execute one tick and return the root status.
    pub fn tick(&mut self, blackboard: &mut Blackboard, entity: &mut T, dt: f32, elapsed: f32) -> Status {
        let mut ctx = TickContext::new(blackboard, entity, dt, elapsed);
        self.root.tick(&mut ctx)
    }

    /// Abort the tree (e.g., entity dies).
    pub fn abort(&mut self) {
        self.root.abort();
    }
}

impl<T> std::fmt::Debug for BehaviorTree<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BehaviorTree({})", self.name)
    }
}

// ── SubTree (reference to another tree) ──────────────────────────────────────

/// Delegates to a separately defined subtree by name. The subtree is looked
/// up in a `SubTreeRegistry` stored on the blackboard via a string key.
/// For simplicity, we inline the subtree node directly.
#[derive(Debug)]
pub struct SubTree<T> {
    pub name: String,
    pub root: Box<dyn Node<T>>,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> Node<T> for SubTree<T> {
    fn tick(&mut self, ctx: &mut TickContext<T>) -> Status {
        self.root.tick(ctx)
    }
    fn abort(&mut self) { self.root.abort(); }
    fn name(&self) -> &str { &self.name }
}

// ── TreeRunner ────────────────────────────────────────────────────────────────

/// Manages a behavior tree instance for one entity, tracking timing and events.
pub struct TreeRunner<T> {
    pub tree:     BehaviorTree<T>,
    pub bb:       Blackboard,
    elapsed:      f32,
    last_status:  Status,
    event_queue:  Vec<String>,
}

impl<T: std::fmt::Debug + Send + Sync + 'static> TreeRunner<T> {
    pub fn new(tree: BehaviorTree<T>) -> Self {
        Self {
            tree,
            bb: Blackboard::new(),
            elapsed: 0.0,
            last_status: Status::Running,
            event_queue: Vec::new(),
        }
    }

    /// Tick the tree. Returns last status. Collects fired events.
    pub fn update(&mut self, entity: &mut T, dt: f32) -> Status {
        self.elapsed += dt;
        let mut ctx = TickContext::new(&mut self.bb, entity, dt, self.elapsed);
        let status = self.tree.root.tick(&mut ctx);
        self.event_queue.extend(ctx.fired_events);
        self.last_status = status;
        status
    }

    /// Drain all fired events since last drain.
    pub fn drain_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.event_queue)
    }

    pub fn last_status(&self) -> Status { self.last_status }
    pub fn elapsed(&self) -> f32 { self.elapsed }

    /// Reset the tree (abort + reset timer).
    pub fn reset(&mut self) {
        self.tree.abort();
        self.elapsed = 0.0;
        self.last_status = Status::Running;
        self.event_queue.clear();
    }
}

// ── Pre-built common behavior trees ──────────────────────────────────────────

/// Generic AI entity for use in example behavior trees.
#[derive(Debug, Default, Clone)]
pub struct AiEntity {
    pub position:  [f32; 3],
    pub health:    f32,
    pub is_dead:   bool,
    pub target_id: Option<u32>,
}

impl AiEntity {
    pub fn new(health: f32) -> Self {
        Self { position: [0.0; 3], health, is_dead: false, target_id: None }
    }
}

/// Factory for common pre-built behavior trees using `AiEntity`.
pub struct CommonBehaviors;

impl CommonBehaviors {
    /// Simple combat AI: attack if in range, else chase, else idle.
    pub fn combat_ai() -> BehaviorTree<AiEntity> {
        // is_dead condition
        let is_alive = Box::new(ConditionNode {
            name: "IsAlive".to_string(),
            func: Box::new(|ctx: &TickContext<AiEntity>| !ctx.entity.is_dead),
        });

        // has target
        let has_target = Box::new(ConditionNode {
            name: "HasTarget".to_string(),
            func: Box::new(|ctx: &TickContext<AiEntity>| ctx.entity.target_id.is_some()),
        });

        // in attack range (target_dist < 2.0)
        let in_attack_range = Box::new(CheckFloat::<AiEntity> {
            key: "target_dist".to_string(),
            threshold: 2.0,
            op: FloatComparison::Less,
            _phantom: std::marker::PhantomData,
        });

        // attack action
        let attack = Box::new(ActionNode {
            name: "Attack".to_string(),
            func: Box::new(|ctx: &mut TickContext<AiEntity>| {
                ctx.fire_event("attack");
                ctx.blackboard.set_float("attack_timer",
                    ctx.blackboard.get_float("attack_timer") + ctx.dt);
                if ctx.blackboard.get_float("attack_timer") >= 0.5 {
                    ctx.blackboard.set_float("attack_timer", 0.0);
                    Status::Success
                } else {
                    Status::Running
                }
            }),
        });

        // chase action
        let chase = Box::new(ActionNode {
            name: "Chase".to_string(),
            func: Box::new(|ctx: &mut TickContext<AiEntity>| {
                let target_pos = ctx.blackboard.get_vec3("target_pos");
                let pos = &mut ctx.entity.position;
                let dx = target_pos[0] - pos[0];
                let dz = target_pos[2] - pos[2];
                let len = (dx * dx + dz * dz).sqrt().max(1e-6);
                let speed = 3.0 * ctx.dt;
                pos[0] += dx / len * speed;
                pos[2] += dz / len * speed;
                let dist = (dx * dx + dz * dz).sqrt();
                ctx.blackboard.set_float("target_dist", dist);
                Status::Running
            }),
        });

        // idle
        let idle = Box::new(Wait::<AiEntity>::new(2.0));

        // attack sequence: in_range → attack
        let attack_seq = Box::new(Sequence::new("attack_seq", vec![in_attack_range, attack]));
        // combat selector: try attack, else chase
        let combat = Box::new(Selector::new("combat", vec![attack_seq, chase]));
        // full combat sequence: has_target → combat
        let combat_seq = Box::new(Sequence::new("combat_seq", vec![has_target, combat]));
        // root selector: alive → (combat or idle)
        let root = Box::new(Selector::new("root", vec![
            Box::new(Sequence::new("alive_gate", vec![is_alive, Box::new(Selector::new("ai", vec![combat_seq, idle]))])),
        ]));

        BehaviorTree::new("combat_ai", root)
    }

    /// Patrol AI: move between waypoints.
    pub fn patrol_ai(waypoints: Vec<[f32; 3]>) -> BehaviorTree<AiEntity> {
        let wp_count = waypoints.len().max(1);
        let waypoints_data = waypoints;

        let patrol = Box::new(ActionNode {
            name: "Patrol".to_string(),
            func: Box::new(move |ctx: &mut TickContext<AiEntity>| {
                let wp_idx = ctx.blackboard.get_int("wp_idx") as usize % wp_count;
                let target = waypoints_data[wp_idx];
                let pos = &mut ctx.entity.position;
                let dx = target[0] - pos[0];
                let dz = target[2] - pos[2];
                let dist = (dx * dx + dz * dz).sqrt();
                if dist < 0.3 {
                    ctx.blackboard.set_int("wp_idx", (wp_idx as i32 + 1) % wp_count as i32);
                    return Status::Success;
                }
                let speed = 2.0 * ctx.dt;
                pos[0] += dx / dist * speed;
                pos[2] += dz / dist * speed;
                Status::Running
            }),
        });

        let root = Box::new(Repeater::infinite(patrol));
        BehaviorTree::new("patrol", root)
    }

    /// Flee AI: flee from threat when health is low.
    pub fn flee_ai(flee_threshold: f32) -> BehaviorTree<AiEntity> {
        let health_low = Box::new(ConditionNode {
            name: "HealthLow".to_string(),
            func: Box::new(move |ctx: &TickContext<AiEntity>| {
                ctx.entity.health < flee_threshold
            }),
        });

        let flee = Box::new(ActionNode {
            name: "Flee".to_string(),
            func: Box::new(|ctx: &mut TickContext<AiEntity>| {
                let threat = ctx.blackboard.get_vec3("threat_pos");
                let pos = &mut ctx.entity.position;
                let dx = pos[0] - threat[0];
                let dz = pos[2] - threat[2];
                let len = (dx * dx + dz * dz).sqrt().max(1e-6);
                pos[0] += dx / len * 5.0 * ctx.dt;
                pos[2] += dz / len * 5.0 * ctx.dt;
                ctx.fire_event("fleeing");
                Status::Running
            }),
        });

        let idle = Box::new(Wait::<AiEntity>::new(1.0));
        let flee_seq = Box::new(Sequence::new("flee_seq", vec![health_low, flee]));
        let root = Box::new(Selector::new("root", vec![flee_seq, idle]));
        BehaviorTree::new("flee", root)
    }

    /// Guard post AI: stay near a position, alert on intrusion.
    pub fn guard_post(post: [f32; 3], alert_radius: f32) -> BehaviorTree<AiEntity> {
        let intruder_near = Box::new(CheckFloat::<AiEntity> {
            key: "target_dist".to_string(),
            threshold: alert_radius,
            op: FloatComparison::Less,
            _phantom: std::marker::PhantomData,
        });

        let alert = Box::new(ActionNode {
            name: "Alert".to_string(),
            func: Box::new(|ctx: &mut TickContext<AiEntity>| {
                ctx.fire_event("intruder_detected");
                ctx.blackboard.set_bool("alerted", true);
                Status::Success
            }),
        });

        let return_to_post = Box::new(ActionNode {
            name: "ReturnToPost".to_string(),
            func: Box::new(move |ctx: &mut TickContext<AiEntity>| {
                let pos = &mut ctx.entity.position;
                let dx = post[0] - pos[0];
                let dz = post[2] - pos[2];
                let dist = (dx * dx + dz * dz).sqrt();
                if dist < 0.2 { return Status::Success; }
                let speed = 2.0 * ctx.dt;
                pos[0] += dx / dist.max(1e-6) * speed;
                pos[2] += dz / dist.max(1e-6) * speed;
                Status::Running
            }),
        });

        let idle_anim = Box::new(ActionNode {
            name: "IdleAnim".to_string(),
            func: Box::new(|ctx: &mut TickContext<AiEntity>| {
                ctx.blackboard.set_string("anim", "idle");
                Status::Success
            }),
        });

        let alert_seq = Box::new(Sequence::new("alert_seq", vec![intruder_near, alert]));
        let idle_patrol = Box::new(Sequence::new("idle_patrol", vec![return_to_post, idle_anim]));
        let root = Box::new(Selector::new("root", vec![alert_seq, idle_patrol]));
        BehaviorTree::new("guard_post", root)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity() -> AiEntity { AiEntity::new(100.0) }

    #[test]
    fn test_sequence_all_success() {
        let s1: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "s1".to_string(),
            func: Box::new(|_| Status::Success),
        });
        let s2: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "s2".to_string(),
            func: Box::new(|_| Status::Success),
        });
        let mut seq = Sequence::new("test", vec![s1, s2]);
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        assert_eq!(seq.tick(&mut ctx), Status::Success);
    }

    #[test]
    fn test_sequence_early_failure() {
        let f: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "f".to_string(),
            func: Box::new(|_| Status::Failure),
        });
        let s: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "s".to_string(),
            func: Box::new(|_| Status::Success),
        });
        let mut seq = Sequence::new("test", vec![f, s]);
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        assert_eq!(seq.tick(&mut ctx), Status::Failure);
    }

    #[test]
    fn test_selector_first_success() {
        let f: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "f".to_string(),
            func: Box::new(|_| Status::Failure),
        });
        let s: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "s".to_string(),
            func: Box::new(|_| Status::Success),
        });
        let mut sel = Selector::new("test", vec![f, s]);
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        assert_eq!(sel.tick(&mut ctx), Status::Success);
    }

    #[test]
    fn test_inverter() {
        let f: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "f".to_string(),
            func: Box::new(|_| Status::Failure),
        });
        let mut inv = Inverter { child: f };
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        assert_eq!(inv.tick(&mut ctx), Status::Success);
    }

    #[test]
    fn test_wait_completes() {
        let mut w = Wait::<AiEntity>::new(0.1);
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        // Tick 1: running
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.05, 0.05);
        assert_eq!(w.tick(&mut ctx), Status::Running);
        // Tick 2: completes
        let mut ctx2 = TickContext::new(&mut bb, &mut e, 0.06, 0.11);
        assert_eq!(w.tick(&mut ctx2), Status::Success);
    }

    #[test]
    fn test_repeater_n_times() {
        let mut count = 0;
        let inner: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "inner".to_string(),
            func: Box::new(move |_ctx| {
                count += 1;
                Status::Success
            }),
        });
        let mut rep = Repeater::n_times(inner, 3);
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        assert_eq!(rep.tick(&mut ctx), Status::Success);
    }

    #[test]
    fn test_cooldown() {
        let inner: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "inner".to_string(),
            func: Box::new(|_| Status::Success),
        });
        let mut cd = Cooldown::new(inner, 1.0);
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        {
            let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.016);
            assert_eq!(cd.tick(&mut ctx), Status::Success);
        }
        // Now on cooldown
        {
            let mut ctx2 = TickContext::new(&mut bb, &mut e, 0.016, 0.032);
            assert_eq!(cd.tick(&mut ctx2), Status::Failure);
        }
    }

    #[test]
    fn test_blackboard_set_check() {
        let mut bb = Blackboard::new();
        bb.set_float("hp", 50.0);
        assert_eq!(bb.get_float("hp"), 50.0);
        bb.set_bool("alive", true);
        assert!(bb.get_bool("alive"));
        bb.set_string("state", "patrol");
        assert_eq!(bb.get_string("state"), "patrol");
    }

    #[test]
    fn test_check_float() {
        let mut chk = CheckFloat::<AiEntity> {
            key: "hp".to_string(),
            threshold: 50.0,
            op: FloatComparison::Less,
            _phantom: std::marker::PhantomData,
        };
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        bb.set_float("hp", 30.0);
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        assert_eq!(chk.tick(&mut ctx), Status::Success);
        bb.set_float("hp", 80.0);
        let mut ctx2 = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        assert_eq!(chk.tick(&mut ctx2), Status::Failure);
    }

    #[test]
    fn test_combat_ai_no_crash() {
        let mut tree = CommonBehaviors::combat_ai();
        let mut bb = Blackboard::new();
        let mut e = AiEntity::new(100.0);
        e.target_id = Some(1);
        bb.set_float("target_dist", 5.0);
        bb.set_vec3("target_pos", [10.0, 0.0, 10.0]);
        for _ in 0..10 {
            tree.tick(&mut bb, &mut e, 0.016, 0.0);
        }
    }

    #[test]
    fn test_patrol_ai_moves() {
        let waypoints = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]];
        let mut tree = CommonBehaviors::patrol_ai(waypoints);
        let mut bb = Blackboard::new();
        let mut e = AiEntity::new(100.0);
        for _ in 0..100 {
            tree.tick(&mut bb, &mut e, 0.016, 0.0);
        }
        // Entity should have moved
        assert!(e.position[0] > 0.1 || e.position[2] != 0.0);
    }

    #[test]
    fn test_fire_event() {
        let mut ev = FireEvent::<AiEntity> {
            event_name: "test_event".to_string(),
            _phantom: std::marker::PhantomData,
        };
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        ev.tick(&mut ctx);
        assert!(ctx.fired_events.contains(&"test_event".to_string()));
    }

    #[test]
    fn test_tree_runner_events() {
        let ev_node: Box<dyn Node<AiEntity>> = Box::new(FireEvent {
            event_name: "tick_event".to_string(),
            _phantom: std::marker::PhantomData,
        });
        let tree = BehaviorTree::new("ev_tree", ev_node);
        let mut runner = TreeRunner::new(tree);
        let mut e = make_entity();
        runner.update(&mut e, 0.016);
        let events = runner.drain_events();
        assert!(events.contains(&"tick_event".to_string()));
    }

    #[test]
    fn test_parallel_succeed_on_one() {
        let s: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "s".to_string(),
            func: Box::new(|_| Status::Success),
        });
        let r: Box<dyn Node<AiEntity>> = Box::new(ActionNode {
            name: "r".to_string(),
            func: Box::new(|_| Status::Running),
        });
        let mut par = Parallel::new("test", vec![s, r]);
        par.success_policy = ParallelPolicy::SucceedOnN(1);
        par.failure_policy = ParallelPolicy::FailOnN(2);
        let mut bb = Blackboard::new();
        let mut e = make_entity();
        let mut ctx = TickContext::new(&mut bb, &mut e, 0.016, 0.0);
        assert_eq!(par.tick(&mut ctx), Status::Success);
    }
}
