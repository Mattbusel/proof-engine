//! Behavior Tree core — nodes, blackboard, tick engine, and tree builder.
//!
//! # Design
//! Every node returns [`NodeStatus`] on each `tick()` call.  The tree walks
//! from the root, short-circuiting Sequences on Failure and Selectors on
//! Success.  Parallel nodes run all children each tick and aggregate results
//! according to their policy.  Decorators wrap a single child and modify its
//! return value or control how often it runs.
//!
//! The [`Blackboard`] is a typed key-value store shared by all nodes in a
//! tree.  It is passed by mutable reference into every tick call so nodes can
//! read sensor data, write intermediate results, and communicate.
//!
//! The [`TreeBuilder`] provides a fluent API for assembling trees without
//! manually constructing the [`BehaviorNode`] enum.  Subtrees can be stored
//! and reused by name via [`SubtreeRegistry`].

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ── NodeStatus ────────────────────────────────────────────────────────────────

/// The three-valued result every behavior-tree node returns each tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    /// The node is still executing and wants to be ticked again.
    Running,
    /// The node finished successfully.
    Success,
    /// The node failed.
    Failure,
}

impl NodeStatus {
    pub fn is_running(self)  -> bool { self == NodeStatus::Running  }
    pub fn is_success(self)  -> bool { self == NodeStatus::Success  }
    pub fn is_failure(self)  -> bool { self == NodeStatus::Failure  }
    pub fn is_terminal(self) -> bool { self != NodeStatus::Running  }

    /// Invert Success↔Failure; Running passes through unchanged.
    pub fn invert(self) -> NodeStatus {
        match self {
            NodeStatus::Success => NodeStatus::Failure,
            NodeStatus::Failure => NodeStatus::Success,
            NodeStatus::Running => NodeStatus::Running,
        }
    }
}

// ── BlackboardValue ───────────────────────────────────────────────────────────

/// All value types storable on a [`Blackboard`].
#[derive(Debug, Clone, PartialEq)]
pub enum BlackboardValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Vec2(glam::Vec2),
    Vec3(glam::Vec3),
    EntityId(u64),
    List(Vec<BlackboardValue>),
    Map(HashMap<String, BlackboardValue>),
}

impl BlackboardValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            BlackboardValue::Bool(v)  => Some(*v),
            BlackboardValue::Int(v)   => Some(*v != 0),
            BlackboardValue::Float(v) => Some(*v != 0.0),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            BlackboardValue::Int(v)   => Some(*v),
            BlackboardValue::Bool(v)  => Some(if *v { 1 } else { 0 }),
            BlackboardValue::Float(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            BlackboardValue::Float(v) => Some(*v),
            BlackboardValue::Int(v)   => Some(*v as f64),
            BlackboardValue::Bool(v)  => Some(if *v { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            BlackboardValue::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_vec2(&self) -> Option<glam::Vec2> {
        match self { BlackboardValue::Vec2(v) => Some(*v), _ => None }
    }

    pub fn as_vec3(&self) -> Option<glam::Vec3> {
        match self { BlackboardValue::Vec3(v) => Some(*v), _ => None }
    }

    pub fn as_entity_id(&self) -> Option<u64> {
        match self { BlackboardValue::EntityId(id) => Some(*id), _ => None }
    }
}

impl From<bool>       for BlackboardValue { fn from(v: bool)       -> Self { BlackboardValue::Bool(v)     } }
impl From<i64>        for BlackboardValue { fn from(v: i64)        -> Self { BlackboardValue::Int(v)      } }
impl From<i32>        for BlackboardValue { fn from(v: i32)        -> Self { BlackboardValue::Int(v as i64) } }
impl From<f64>        for BlackboardValue { fn from(v: f64)        -> Self { BlackboardValue::Float(v)    } }
impl From<f32>        for BlackboardValue { fn from(v: f32)        -> Self { BlackboardValue::Float(v as f64) } }
impl From<String>     for BlackboardValue { fn from(v: String)     -> Self { BlackboardValue::Text(v)     } }
impl From<&str>       for BlackboardValue { fn from(v: &str)       -> Self { BlackboardValue::Text(v.to_string()) } }
impl From<glam::Vec2> for BlackboardValue { fn from(v: glam::Vec2) -> Self { BlackboardValue::Vec2(v)    } }
impl From<glam::Vec3> for BlackboardValue { fn from(v: glam::Vec3) -> Self { BlackboardValue::Vec3(v)    } }
impl From<u64>        for BlackboardValue { fn from(v: u64)        -> Self { BlackboardValue::EntityId(v) } }

// ── Blackboard ────────────────────────────────────────────────────────────────

/// A typed key-value store shared by all nodes in a behavior tree.
///
/// Entries can optionally expire after a time-to-live (TTL) measured in
/// seconds.  Call [`Blackboard::tick`] every frame with `dt` to process
/// expirations.
#[derive(Debug, Default)]
pub struct Blackboard {
    entries: HashMap<String, BlackboardEntry>,
    /// Running simulation time in seconds (accumulated from `tick(dt)` calls).
    pub time: f64,
}

#[derive(Debug, Clone)]
struct BlackboardEntry {
    value:      BlackboardValue,
    /// Absolute expiry time (seconds, same clock as `Blackboard::time`).
    expires_at: Option<f64>,
}

impl Blackboard {
    pub fn new() -> Self { Self::default() }

    /// Advance the blackboard clock and expire stale entries.
    pub fn tick(&mut self, dt: f64) {
        self.time += dt;
        let t = self.time;
        self.entries.retain(|_, e| {
            e.expires_at.map_or(true, |exp| t < exp)
        });
    }

    /// Insert a key-value pair with no expiry.
    pub fn set<V: Into<BlackboardValue>>(&mut self, key: &str, value: V) {
        self.entries.insert(key.to_string(), BlackboardEntry {
            value: value.into(),
            expires_at: None,
        });
    }

    /// Insert a key-value pair that will expire after `ttl` seconds.
    pub fn set_with_ttl<V: Into<BlackboardValue>>(&mut self, key: &str, value: V, ttl: f64) {
        self.entries.insert(key.to_string(), BlackboardEntry {
            value: value.into(),
            expires_at: Some(self.time + ttl),
        });
    }

    /// Retrieve a value by key.
    pub fn get(&self, key: &str) -> Option<&BlackboardValue> {
        self.entries.get(key).map(|e| &e.value)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key)?.as_bool()
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_int()
    }

    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_float()
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    pub fn get_vec2(&self, key: &str) -> Option<glam::Vec2> {
        self.get(key)?.as_vec2()
    }

    pub fn get_vec3(&self, key: &str) -> Option<glam::Vec3> {
        self.get(key)?.as_vec3()
    }

    pub fn get_entity(&self, key: &str) -> Option<u64> {
        self.get(key)?.as_entity_id()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<BlackboardValue> {
        self.entries.remove(key).map(|e| e.value)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of live entries.
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// Iterate over all live key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &BlackboardValue)> {
        self.entries.iter().map(|(k, e)| (k.as_str(), &e.value))
    }

    /// Compare a float entry against a threshold.
    pub fn float_gt(&self, key: &str, threshold: f64) -> bool {
        self.get_float(key).map_or(false, |v| v > threshold)
    }

    pub fn float_lt(&self, key: &str, threshold: f64) -> bool {
        self.get_float(key).map_or(false, |v| v < threshold)
    }

    pub fn float_gte(&self, key: &str, threshold: f64) -> bool {
        self.get_float(key).map_or(false, |v| v >= threshold)
    }

    pub fn float_lte(&self, key: &str, threshold: f64) -> bool {
        self.get_float(key).map_or(false, |v| v <= threshold)
    }

    /// Increment an integer counter by `delta`.  Missing keys start at 0.
    pub fn increment(&mut self, key: &str, delta: i64) {
        let current = self.get_int(key).unwrap_or(0);
        self.set(key, current + delta);
    }

    /// Decrement an integer counter by `delta`.
    pub fn decrement(&mut self, key: &str, delta: i64) {
        self.increment(key, -delta);
    }

    /// Toggle a boolean entry.  Missing keys become `true`.
    pub fn toggle(&mut self, key: &str) {
        let current = self.get_bool(key).unwrap_or(false);
        self.set(key, !current);
    }
}

// ── ParallelPolicy ────────────────────────────────────────────────────────────

/// Controls how a [`BehaviorNode::Parallel`] node determines its own status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallelPolicy {
    /// Succeed when ALL children succeed; fail when ANY child fails.
    RequireAll,
    /// Succeed when ANY child succeeds; fail when ALL children fail.
    RequireOne,
    /// Succeed when a minimum count of children succeed.
    RequireN(usize),
}

impl Default for ParallelPolicy {
    fn default() -> Self { ParallelPolicy::RequireAll }
}

// ── DecoratorKind ─────────────────────────────────────────────────────────────

/// The flavour of behavior a decorator applies to its single child.
#[derive(Debug, Clone)]
pub enum DecoratorKind {
    /// Flip Success↔Failure; Running is unchanged.
    Invert,
    /// Always return Success regardless of child result.
    AlwaysSucceed,
    /// Always return Failure regardless of child result.
    AlwaysFail,
    /// Force the child to return Running until it succeeds.
    UntilSuccess,
    /// Force the child to return Running until it fails.
    UntilFailure,
    /// Repeat child `count` times; return its final status.
    Repeat { count: u32 },
    /// Repeat child forever (returns Running indefinitely unless interrupted).
    RepeatForever,
    /// Return Failure if child is still Running after `timeout_secs`.
    Timeout { timeout_secs: f32 },
    /// Rate-limit: only tick child if at least `cooldown_secs` have elapsed.
    Cooldown { cooldown_secs: f32 },
    /// Only tick if a blackboard boolean key is true.
    BlackboardGuard { key: String, expected: bool },
    /// Succeed immediately if blackboard key equals expected; else Failure.
    BlackboardCheck { key: String, expected: BlackboardValue },
}

// ── BehaviorNode ──────────────────────────────────────────────────────────────

/// The complete behavior tree node type.
///
/// Nodes are stored in a `Box<BehaviorNode>` tree; the root is owned by a
/// [`BehaviorTree`].  Leaf nodes contain a closure (`Box<dyn FnMut(…)>`) so
/// they carry arbitrary game-logic without external look-up tables.
pub enum BehaviorNode {
    /// Run children in order; stop and return Failure on the first child that
    /// fails.  Return Success only when all children succeed.
    Sequence {
        name:     String,
        children: Vec<BehaviorNode>,
        /// Index of the currently-active child (persists across ticks).
        cursor:   usize,
    },

    /// Run children in order; stop and return Success on the first child that
    /// succeeds.  Return Failure only when all children fail.
    Selector {
        name:     String,
        children: Vec<BehaviorNode>,
        cursor:   usize,
    },

    /// Tick all children every frame regardless of their individual statuses.
    Parallel {
        name:     String,
        children: Vec<BehaviorNode>,
        policy:   ParallelPolicy,
    },

    /// Wraps exactly one child and modifies how its status is interpreted.
    Decorator {
        name:  String,
        kind:  DecoratorKind,
        child: Box<BehaviorNode>,
        /// Internal counter used by Repeat / Timeout / Cooldown decorators.
        state: DecoratorState,
    },

    /// A leaf with user-supplied tick logic.
    Leaf {
        name:   String,
        /// Called once when the node first becomes active (optional).
        on_enter: Option<Box<dyn FnMut(&mut Blackboard)>>,
        /// Main tick function. Return the node's status.
        on_tick:  Box<dyn FnMut(&mut Blackboard, f32) -> NodeStatus>,
        /// Called when the node exits (success, failure, or abort).
        on_exit:  Option<Box<dyn FnMut(&mut Blackboard, NodeStatus)>>,
        /// Whether on_enter has been called for the current activation.
        entered:  bool,
    },

    /// A subtree stored by name in a [`SubtreeRegistry`].  Resolved at tick
    /// time; if the name is unknown the node returns Failure.
    SubtreeRef {
        name: String,
    },
}

impl std::fmt::Debug for BehaviorNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BehaviorNode::Sequence  { name, .. }     => write!(f, "Sequence({name})"),
            BehaviorNode::Selector  { name, .. }     => write!(f, "Selector({name})"),
            BehaviorNode::Parallel  { name, .. }     => write!(f, "Parallel({name})"),
            BehaviorNode::Decorator { name, kind, .. }=> write!(f, "Decorator({name}, {kind:?})"),
            BehaviorNode::Leaf      { name, .. }     => write!(f, "Leaf({name})"),
            BehaviorNode::SubtreeRef{ name }         => write!(f, "SubtreeRef({name})"),
        }
    }
}

// ── DecoratorState ────────────────────────────────────────────────────────────

/// Mutable state owned by a `Decorator` node.
#[derive(Debug, Default, Clone)]
pub struct DecoratorState {
    /// Ticks/loops completed (used by Repeat).
    pub repeat_count:   u32,
    /// Time the current activation started (used by Timeout).
    pub activated_at:   Option<Instant>,
    /// Wall-clock time of last tick (used by Cooldown).
    pub last_tick_time: Option<Instant>,
    /// Simulation-time of last tick (used by Cooldown, in seconds).
    pub last_tick_sim:  f64,
}

impl DecoratorState {
    fn reset(&mut self) {
        self.repeat_count   = 0;
        self.activated_at   = None;
        self.last_tick_time = None;
        self.last_tick_sim  = 0.0;
    }
}

// ── BehaviorNode tick ─────────────────────────────────────────────────────────

impl BehaviorNode {
    /// Recursively tick this node and return its status.
    ///
    /// `dt` is the elapsed time in seconds since the last tick.
    /// `bb` is the shared blackboard for this tree.
    /// `registry` is used to resolve SubtreeRef nodes.
    pub fn tick(
        &mut self,
        dt:       f32,
        bb:       &mut Blackboard,
        registry: &SubtreeRegistry,
    ) -> NodeStatus {
        match self {
            // ── Sequence ──────────────────────────────────────────────────────
            BehaviorNode::Sequence { children, cursor, .. } => {
                while *cursor < children.len() {
                    let status = children[*cursor].tick(dt, bb, registry);
                    match status {
                        NodeStatus::Success => { *cursor += 1; }
                        NodeStatus::Running => return NodeStatus::Running,
                        NodeStatus::Failure => {
                            *cursor = 0;
                            return NodeStatus::Failure;
                        }
                    }
                }
                *cursor = 0;
                NodeStatus::Success
            }

            // ── Selector ──────────────────────────────────────────────────────
            BehaviorNode::Selector { children, cursor, .. } => {
                while *cursor < children.len() {
                    let status = children[*cursor].tick(dt, bb, registry);
                    match status {
                        NodeStatus::Failure => { *cursor += 1; }
                        NodeStatus::Success => {
                            *cursor = 0;
                            return NodeStatus::Success;
                        }
                        NodeStatus::Running => return NodeStatus::Running,
                    }
                }
                *cursor = 0;
                NodeStatus::Failure
            }

            // ── Parallel ──────────────────────────────────────────────────────
            BehaviorNode::Parallel { children, policy, .. } => {
                let mut successes = 0usize;
                let mut failures  = 0usize;
                let total = children.len();

                for child in children.iter_mut() {
                    match child.tick(dt, bb, registry) {
                        NodeStatus::Success => successes += 1,
                        NodeStatus::Failure => failures  += 1,
                        NodeStatus::Running => {}
                    }
                }

                match *policy {
                    ParallelPolicy::RequireAll => {
                        if failures > 0        { NodeStatus::Failure }
                        else if successes == total { NodeStatus::Success }
                        else                    { NodeStatus::Running  }
                    }
                    ParallelPolicy::RequireOne => {
                        if successes > 0         { NodeStatus::Success }
                        else if failures == total { NodeStatus::Failure }
                        else                     { NodeStatus::Running  }
                    }
                    ParallelPolicy::RequireN(n) => {
                        if successes >= n           { NodeStatus::Success }
                        else if total - failures < n { NodeStatus::Failure }
                        else                         { NodeStatus::Running  }
                    }
                }
            }

            // ── Decorator ─────────────────────────────────────────────────────
            BehaviorNode::Decorator { kind, child, state, .. } => {
                tick_decorator(kind, child, state, dt, bb, registry)
            }

            // ── Leaf ──────────────────────────────────────────────────────────
            BehaviorNode::Leaf { on_enter, on_tick, on_exit, entered, .. } => {
                if !*entered {
                    *entered = true;
                    if let Some(enter_fn) = on_enter {
                        enter_fn(bb);
                    }
                }
                let status = on_tick(bb, dt);
                if status.is_terminal() {
                    *entered = false;
                    if let Some(exit_fn) = on_exit {
                        exit_fn(bb, status);
                    }
                }
                status
            }

            // ── SubtreeRef ────────────────────────────────────────────────────
            BehaviorNode::SubtreeRef { name } => {
                // SubtreeRef is resolved by the caller (BehaviorTree::tick)
                // into an owned copy; here we just return Failure as a
                // safety fallback if somehow ticked directly.
                log::warn!("SubtreeRef({name}) ticked directly — subtree not found");
                NodeStatus::Failure
            }
        }
    }

    /// Reset the cursor / state of this node and all its children, so the
    /// tree starts fresh on the next tick.
    pub fn reset(&mut self) {
        match self {
            BehaviorNode::Sequence  { children, cursor, .. } => {
                *cursor = 0;
                for c in children.iter_mut() { c.reset(); }
            }
            BehaviorNode::Selector  { children, cursor, .. } => {
                *cursor = 0;
                for c in children.iter_mut() { c.reset(); }
            }
            BehaviorNode::Parallel  { children, .. } => {
                for c in children.iter_mut() { c.reset(); }
            }
            BehaviorNode::Decorator { child, state, .. } => {
                state.reset();
                child.reset();
            }
            BehaviorNode::Leaf { entered, .. } => {
                *entered = false;
            }
            BehaviorNode::SubtreeRef { .. } => {}
        }
    }

    /// Return the name of this node.
    pub fn name(&self) -> &str {
        match self {
            BehaviorNode::Sequence  { name, .. } => name,
            BehaviorNode::Selector  { name, .. } => name,
            BehaviorNode::Parallel  { name, .. } => name,
            BehaviorNode::Decorator { name, .. } => name,
            BehaviorNode::Leaf      { name, .. } => name,
            BehaviorNode::SubtreeRef{ name }     => name,
        }
    }

    /// Recursively collect the names of all nodes into `out`.
    pub fn collect_names(&self, out: &mut Vec<String>) {
        out.push(self.name().to_string());
        match self {
            BehaviorNode::Sequence  { children, .. }
            | BehaviorNode::Selector{ children, .. }
            | BehaviorNode::Parallel{ children, .. } => {
                for c in children { c.collect_names(out); }
            }
            BehaviorNode::Decorator { child, .. } => child.collect_names(out),
            _ => {}
        }
    }
}

// ── Decorator tick helper ─────────────────────────────────────────────────────

fn tick_decorator(
    kind:     &mut DecoratorKind,
    child:    &mut BehaviorNode,
    state:    &mut DecoratorState,
    dt:       f32,
    bb:       &mut Blackboard,
    registry: &SubtreeRegistry,
) -> NodeStatus {
    match kind {
        DecoratorKind::Invert => child.tick(dt, bb, registry).invert(),

        DecoratorKind::AlwaysSucceed => {
            child.tick(dt, bb, registry);
            NodeStatus::Success
        }

        DecoratorKind::AlwaysFail => {
            child.tick(dt, bb, registry);
            NodeStatus::Failure
        }

        DecoratorKind::UntilSuccess => {
            match child.tick(dt, bb, registry) {
                NodeStatus::Success => NodeStatus::Success,
                _ => NodeStatus::Running,
            }
        }

        DecoratorKind::UntilFailure => {
            match child.tick(dt, bb, registry) {
                NodeStatus::Failure => NodeStatus::Failure,
                _ => NodeStatus::Running,
            }
        }

        DecoratorKind::Repeat { count } => {
            let target = *count;
            loop {
                match child.tick(dt, bb, registry) {
                    NodeStatus::Running => return NodeStatus::Running,
                    NodeStatus::Failure => {
                        state.repeat_count = 0;
                        child.reset();
                        return NodeStatus::Failure;
                    }
                    NodeStatus::Success => {
                        state.repeat_count += 1;
                        child.reset();
                        if state.repeat_count >= target {
                            state.repeat_count = 0;
                            return NodeStatus::Success;
                        }
                    }
                }
            }
        }

        DecoratorKind::RepeatForever => {
            match child.tick(dt, bb, registry) {
                NodeStatus::Success | NodeStatus::Failure => {
                    child.reset();
                }
                NodeStatus::Running => {}
            }
            NodeStatus::Running
        }

        DecoratorKind::Timeout { timeout_secs } => {
            let limit = *timeout_secs;
            let now = Instant::now();
            let started = state.activated_at.get_or_insert(now);
            if now.duration_since(*started) >= Duration::from_secs_f32(limit) {
                state.activated_at = None;
                child.reset();
                return NodeStatus::Failure;
            }
            let status = child.tick(dt, bb, registry);
            if status.is_terminal() {
                state.activated_at = None;
            }
            status
        }

        DecoratorKind::Cooldown { cooldown_secs } => {
            let cooldown = Duration::from_secs_f32(*cooldown_secs);
            let now = Instant::now();
            if let Some(last) = state.last_tick_time {
                if now.duration_since(last) < cooldown {
                    return NodeStatus::Failure;
                }
            }
            state.last_tick_time = Some(now);
            child.tick(dt, bb, registry)
        }

        DecoratorKind::BlackboardGuard { key, expected } => {
            let key_clone = key.clone();
            let exp = *expected;
            let ok = bb.get_bool(&key_clone).unwrap_or(false) == exp;
            if ok { child.tick(dt, bb, registry) } else { NodeStatus::Failure }
        }

        DecoratorKind::BlackboardCheck { key, expected } => {
            let key_clone = key.clone();
            match bb.get(&key_clone) {
                Some(v) if v == expected => NodeStatus::Success,
                _ => NodeStatus::Failure,
            }
        }
    }
}

// ── SubtreeRegistry ───────────────────────────────────────────────────────────

/// A named registry that maps string keys to reusable subtree factories.
///
/// Subtrees are stored as closures that produce a fresh `BehaviorNode` on
/// demand; this avoids shared mutable state while still allowing reuse.
pub struct SubtreeRegistry {
    builders: HashMap<String, Box<dyn Fn() -> BehaviorNode>>,
}

impl Default for SubtreeRegistry {
    fn default() -> Self { Self { builders: HashMap::new() } }
}

impl SubtreeRegistry {
    pub fn new() -> Self { Self::default() }

    /// Register a subtree factory under `name`.
    pub fn register<F>(&mut self, name: &str, factory: F)
    where
        F: Fn() -> BehaviorNode + 'static,
    {
        self.builders.insert(name.to_string(), Box::new(factory));
    }

    /// Instantiate the subtree named `name`, or `None` if not registered.
    pub fn instantiate(&self, name: &str) -> Option<BehaviorNode> {
        self.builders.get(name).map(|f| f())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.builders.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.builders.keys().map(|s| s.as_str())
    }
}

impl std::fmt::Debug for SubtreeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.names().collect();
        f.debug_struct("SubtreeRegistry").field("subtrees", &names).finish()
    }
}

// ── BehaviorTree ──────────────────────────────────────────────────────────────

/// The top-level behavior tree that owns a root node, a blackboard, and an
/// optional subtree registry.
#[derive(Debug)]
pub struct BehaviorTree {
    /// Human-readable label for debugging.
    pub name:     String,
    root:         BehaviorNode,
    pub bb:       Blackboard,
    pub registry: SubtreeRegistry,
    /// Status returned by the last `tick()` call.
    pub last_status: NodeStatus,
    /// Total accumulated simulation time in seconds.
    pub sim_time: f64,
    /// Whether the tree is currently active.
    pub active:   bool,
}

impl BehaviorTree {
    pub fn new(name: &str, root: BehaviorNode) -> Self {
        Self {
            name:        name.to_string(),
            root,
            bb:          Blackboard::new(),
            registry:    SubtreeRegistry::new(),
            last_status: NodeStatus::Running,
            sim_time:    0.0,
            active:      true,
        }
    }

    /// Tick the entire tree by `dt` seconds.  Advances the blackboard clock
    /// and resolves any `SubtreeRef` nodes via the registry.
    pub fn tick(&mut self, dt: f32) -> NodeStatus {
        if !self.active {
            return self.last_status;
        }
        self.sim_time += dt as f64;
        self.bb.tick(dt as f64);
        let status = tick_with_registry(&mut self.root, dt, &mut self.bb, &self.registry);
        self.last_status = status;
        status
    }

    /// Reset the whole tree so it starts fresh next tick.
    pub fn reset(&mut self) {
        self.root.reset();
        self.last_status = NodeStatus::Running;
    }

    /// Pause / resume ticking.
    pub fn set_active(&mut self, active: bool) { self.active = active; }

    /// Access the blackboard immutably.
    pub fn blackboard(&self) -> &Blackboard { &self.bb }

    /// Access the blackboard mutably (useful for injecting sensor data).
    pub fn blackboard_mut(&mut self) -> &mut Blackboard { &mut self.bb }

    /// Access the subtree registry mutably (register subtrees after build).
    pub fn registry_mut(&mut self) -> &mut SubtreeRegistry { &mut self.registry }

    /// Collect all node names in the tree (depth-first).
    pub fn node_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.root.collect_names(&mut names);
        names
    }
}

/// Tick a node, expanding SubtreeRef nodes via the registry.
fn tick_with_registry(
    node:     &mut BehaviorNode,
    dt:       f32,
    bb:       &mut Blackboard,
    registry: &SubtreeRegistry,
) -> NodeStatus {
    // SubtreeRef is special: we need to instantiate a temporary subtree.
    // Since we cannot replace the node in place without unsafe, we detect
    // SubtreeRef here and drive it separately.
    if let BehaviorNode::SubtreeRef { name } = node {
        match registry.instantiate(name) {
            Some(mut subtree) => subtree.tick(dt, bb, registry),
            None => {
                log::warn!("SubtreeRef: unknown subtree '{name}'");
                NodeStatus::Failure
            }
        }
    } else {
        node.tick(dt, bb, registry)
    }
}

// ── TreeBuilder ───────────────────────────────────────────────────────────────

/// Fluent builder for assembling [`BehaviorNode`] trees.
///
/// # Example
/// ```ignore
/// let root = TreeBuilder::sequence("root")
///     .leaf("idle", |_bb, _dt| NodeStatus::Success)
///     .selector("patrol_or_attack")
///         .leaf("patrol", patrol_fn)
///         .leaf("attack", attack_fn)
///     .end()
///     .build();
/// ```
pub struct TreeBuilder {
    /// Stack of in-progress composite nodes (name, kind, children).
    stack: Vec<BuildFrame>,
    /// The final completed node, set when the outermost frame is popped.
    result: Option<BehaviorNode>,
}

struct BuildFrame {
    kind:     FrameKind,
    name:     String,
    children: Vec<BehaviorNode>,
    /// For Decorator frames, the decorator kind.
    dec_kind: Option<DecoratorKind>,
    /// For Parallel frames, the policy.
    par_policy: Option<ParallelPolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameKind { Sequence, Selector, Parallel, Decorator }

impl TreeBuilder {
    fn new_frame(kind: FrameKind, name: &str) -> BuildFrame {
        BuildFrame { kind, name: name.to_string(), children: Vec::new(),
                     dec_kind: None, par_policy: None }
    }

    /// Begin a new top-level Sequence.
    pub fn sequence(name: &str) -> Self {
        let mut b = TreeBuilder { stack: Vec::new(), result: None };
        b.stack.push(Self::new_frame(FrameKind::Sequence, name));
        b
    }

    /// Begin a new top-level Selector.
    pub fn selector(name: &str) -> Self {
        let mut b = TreeBuilder { stack: Vec::new(), result: None };
        b.stack.push(Self::new_frame(FrameKind::Selector, name));
        b
    }

    /// Begin a new top-level Parallel.
    pub fn parallel(name: &str, policy: ParallelPolicy) -> Self {
        let mut b = TreeBuilder { stack: Vec::new(), result: None };
        let mut frame = Self::new_frame(FrameKind::Parallel, name);
        frame.par_policy = Some(policy);
        b.stack.push(frame);
        b
    }

    /// Push a Sequence child onto the current composite.
    pub fn sequence_child(mut self, name: &str) -> Self {
        self.stack.push(Self::new_frame(FrameKind::Sequence, name));
        self
    }

    /// Push a Selector child onto the current composite.
    pub fn selector_child(mut self, name: &str) -> Self {
        self.stack.push(Self::new_frame(FrameKind::Selector, name));
        self
    }

    /// Push a Parallel child onto the current composite.
    pub fn parallel_child(mut self, name: &str, policy: ParallelPolicy) -> Self {
        let mut frame = Self::new_frame(FrameKind::Parallel, name);
        frame.par_policy = Some(policy);
        self.stack.push(frame);
        self
    }

    /// Push a Decorator child onto the current composite.
    pub fn decorator(mut self, name: &str, kind: DecoratorKind) -> Self {
        let mut frame = Self::new_frame(FrameKind::Decorator, name);
        frame.dec_kind = Some(kind);
        self.stack.push(frame);
        self
    }

    /// Add a leaf node to the current composite or decorator.
    pub fn leaf<F>(mut self, name: &str, tick_fn: F) -> Self
    where
        F: FnMut(&mut Blackboard, f32) -> NodeStatus + 'static,
    {
        let node = BehaviorNode::Leaf {
            name:     name.to_string(),
            on_enter: None,
            on_tick:  Box::new(tick_fn),
            on_exit:  None,
            entered:  false,
        };
        self.push_child(node);
        self
    }

    /// Add a full leaf with enter/exit callbacks.
    pub fn leaf_full<Enter, Tick, Exit>(
        mut self,
        name:     &str,
        enter_fn: Enter,
        tick_fn:  Tick,
        exit_fn:  Exit,
    ) -> Self
    where
        Enter: FnMut(&mut Blackboard) + 'static,
        Tick:  FnMut(&mut Blackboard, f32) -> NodeStatus + 'static,
        Exit:  FnMut(&mut Blackboard, NodeStatus) + 'static,
    {
        let node = BehaviorNode::Leaf {
            name:     name.to_string(),
            on_enter: Some(Box::new(enter_fn)),
            on_tick:  Box::new(tick_fn),
            on_exit:  Some(Box::new(exit_fn)),
            entered:  false,
        };
        self.push_child(node);
        self
    }

    /// Add a subtree reference node.
    pub fn subtree_ref(mut self, name: &str) -> Self {
        self.push_child(BehaviorNode::SubtreeRef { name: name.to_string() });
        self
    }

    /// Add an already-built node as a child.
    pub fn node(mut self, node: BehaviorNode) -> Self {
        self.push_child(node);
        self
    }

    /// Close the current composite/decorator and return to the parent.
    pub fn end(mut self) -> Self {
        let completed = self.pop_frame();
        if self.stack.is_empty() {
            self.result = Some(completed);
        } else {
            self.push_child(completed);
        }
        self
    }

    /// Finish building and return the root `BehaviorNode`.
    /// Panics if not at the outermost frame.
    pub fn build(mut self) -> BehaviorNode {
        // Implicitly close any remaining open frames.
        while !self.stack.is_empty() {
            let completed = self.pop_frame();
            if self.stack.is_empty() {
                self.result = Some(completed);
            } else {
                self.push_child(completed);
            }
        }
        self.result.expect("TreeBuilder::build called with no nodes")
    }

    /// Convenience: build and wrap in a [`BehaviorTree`].
    pub fn into_tree(self, tree_name: &str) -> BehaviorTree {
        let root = self.build();
        BehaviorTree::new(tree_name, root)
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    fn push_child(&mut self, node: BehaviorNode) {
        if let Some(frame) = self.stack.last_mut() {
            frame.children.push(node);
        } else {
            // No open frame; this becomes the sole result.
            self.result = Some(node);
        }
    }

    fn pop_frame(&mut self) -> BehaviorNode {
        let frame = self.stack.pop().expect("TreeBuilder: pop_frame on empty stack");
        match frame.kind {
            FrameKind::Sequence => BehaviorNode::Sequence {
                name:     frame.name,
                children: frame.children,
                cursor:   0,
            },
            FrameKind::Selector => BehaviorNode::Selector {
                name:     frame.name,
                children: frame.children,
                cursor:   0,
            },
            FrameKind::Parallel => BehaviorNode::Parallel {
                name:     frame.name,
                children: frame.children,
                policy:   frame.par_policy.unwrap_or_default(),
            },
            FrameKind::Decorator => {
                let kind  = frame.dec_kind.expect("Decorator frame missing kind");
                let child = frame.children.into_iter().next()
                    .expect("Decorator frame must have exactly one child");
                BehaviorNode::Decorator {
                    name:  frame.name,
                    kind,
                    child: Box::new(child),
                    state: DecoratorState::default(),
                }
            }
        }
    }
}

// ── Convenience constructors ──────────────────────────────────────────────────

/// Create a simple leaf node from a closure.
pub fn leaf<F>(name: &str, tick_fn: F) -> BehaviorNode
where
    F: FnMut(&mut Blackboard, f32) -> NodeStatus + 'static,
{
    BehaviorNode::Leaf {
        name:     name.to_string(),
        on_enter: None,
        on_tick:  Box::new(tick_fn),
        on_exit:  None,
        entered:  false,
    }
}

/// Create a Sequence node from a list of children.
pub fn sequence(name: &str, children: Vec<BehaviorNode>) -> BehaviorNode {
    BehaviorNode::Sequence { name: name.to_string(), children, cursor: 0 }
}

/// Create a Selector node from a list of children.
pub fn selector(name: &str, children: Vec<BehaviorNode>) -> BehaviorNode {
    BehaviorNode::Selector { name: name.to_string(), children, cursor: 0 }
}

/// Create a Parallel node from a list of children.
pub fn parallel(name: &str, children: Vec<BehaviorNode>, policy: ParallelPolicy) -> BehaviorNode {
    BehaviorNode::Parallel { name: name.to_string(), children, policy }
}

/// Wrap a node in an Invert decorator.
pub fn invert(name: &str, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::Invert,
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

/// Wrap a node in a Repeat(n) decorator.
pub fn repeat(name: &str, count: u32, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::Repeat { count },
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

/// Wrap a node in a Timeout decorator.
pub fn timeout(name: &str, secs: f32, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::Timeout { timeout_secs: secs },
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

/// Wrap a node in a Cooldown decorator.
pub fn cooldown(name: &str, secs: f32, child: BehaviorNode) -> BehaviorNode {
    BehaviorNode::Decorator {
        name:  name.to_string(),
        kind:  DecoratorKind::Cooldown { cooldown_secs: secs },
        child: Box::new(child),
        state: DecoratorState::default(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn succeed() -> BehaviorNode {
        leaf("succeed", |_, _| NodeStatus::Success)
    }
    fn fail() -> BehaviorNode {
        leaf("fail", |_, _| NodeStatus::Failure)
    }

    #[test]
    fn sequence_all_succeed() {
        let mut bb  = Blackboard::new();
        let reg = SubtreeRegistry::new();
        let mut node = sequence("s", vec![succeed(), succeed(), succeed()]);
        assert_eq!(node.tick(0.016, &mut bb, &reg), NodeStatus::Success);
    }

    #[test]
    fn sequence_first_fails() {
        let mut bb  = Blackboard::new();
        let reg = SubtreeRegistry::new();
        let mut node = sequence("s", vec![fail(), succeed()]);
        assert_eq!(node.tick(0.016, &mut bb, &reg), NodeStatus::Failure);
    }

    #[test]
    fn selector_first_succeeds() {
        let mut bb  = Blackboard::new();
        let reg = SubtreeRegistry::new();
        let mut node = selector("sel", vec![succeed(), fail()]);
        assert_eq!(node.tick(0.016, &mut bb, &reg), NodeStatus::Success);
    }

    #[test]
    fn selector_all_fail() {
        let mut bb  = Blackboard::new();
        let reg = SubtreeRegistry::new();
        let mut node = selector("sel", vec![fail(), fail()]);
        assert_eq!(node.tick(0.016, &mut bb, &reg), NodeStatus::Failure);
    }

    #[test]
    fn invert_decorator() {
        let mut bb  = Blackboard::new();
        let reg = SubtreeRegistry::new();
        let mut node = invert("inv", succeed());
        assert_eq!(node.tick(0.016, &mut bb, &reg), NodeStatus::Failure);
    }

    #[test]
    fn blackboard_set_get() {
        let mut bb = Blackboard::new();
        bb.set("health", 80.0f64);
        assert!(bb.float_gt("health", 50.0));
        assert!(!bb.float_lt("health", 50.0));
    }

    #[test]
    fn blackboard_ttl_expiry() {
        let mut bb = Blackboard::new();
        bb.set_with_ttl("temp", true, 1.0);
        assert!(bb.contains("temp"));
        bb.tick(2.0);
        assert!(!bb.contains("temp"));
    }

    #[test]
    fn repeat_decorator() {
        let mut bb  = Blackboard::new();
        let reg = SubtreeRegistry::new();
        let mut node = repeat("rep", 3, succeed());
        assert_eq!(node.tick(0.016, &mut bb, &reg), NodeStatus::Success);
    }

    #[test]
    fn parallel_require_all() {
        let mut bb  = Blackboard::new();
        let reg = SubtreeRegistry::new();
        let mut node = parallel("par", vec![succeed(), succeed()], ParallelPolicy::RequireAll);
        assert_eq!(node.tick(0.016, &mut bb, &reg), NodeStatus::Success);
    }

    #[test]
    fn parallel_require_one_first_succeeds() {
        let mut bb  = Blackboard::new();
        let reg = SubtreeRegistry::new();
        let mut node = parallel("par", vec![succeed(), fail()], ParallelPolicy::RequireOne);
        assert_eq!(node.tick(0.016, &mut bb, &reg), NodeStatus::Success);
    }

    #[test]
    fn tree_builder_round_trip() {
        let root = TreeBuilder::sequence("root")
            .leaf("a", |_, _| NodeStatus::Success)
            .leaf("b", |_, _| NodeStatus::Success)
            .build();
        let mut tree = BehaviorTree::new("test", root);
        assert_eq!(tree.tick(0.016), NodeStatus::Success);
    }
}
