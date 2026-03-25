//! Animation State Machine and Blend Trees.
//!
//! Provides a complete animation runtime for Proof Engine entities:
//!
//! - `AnimationClip`     — named sequence of keyframe channels over time
//! - `AnimationCurve`    — per-property float curve (driven by MathFunction or raw keyframes)
//! - `BlendTree`         — 1D/2D weighted blend of multiple clips
//! - `AnimationLayer`    — masked layer (e.g., upper-body, full-body)
//! - `AnimationState`    — named state that plays a clip or blend tree
//! - `Transition`        — condition-triggered crossfade between states
//! - `AnimatorController`— top-level controller driving all layers
//!
//! ## Quick Start
//! ```rust,no_run
//! use proof_engine::animation::*;
//! let mut ctrl = AnimatorController::new();
//! ctrl.add_state("idle",   AnimationState::clip("idle_clip",   1.0, true));
//! ctrl.add_state("run",    AnimationState::clip("run_clip",    0.8, true));
//! ctrl.add_state("attack", AnimationState::clip("attack_clip", 0.4, false));
//! ctrl.add_transition("idle",   "run",    Condition::float_gt("speed", 0.1));
//! ctrl.add_transition("run",    "idle",   Condition::float_lt("speed", 0.05));
//! ctrl.add_transition("idle",   "attack", Condition::trigger("attack"));
//! ctrl.start("idle");
//! ```

pub mod ik;
pub mod sprite_anim;

use std::collections::HashMap;
use crate::math::MathFunction;

// ── AnimationCurve ─────────────────────────────────────────────────────────────

/// A single float channel over normalized time [0, 1].
#[derive(Debug, Clone)]
pub enum AnimationCurve {
    /// Constant value.
    Constant(f32),
    /// Linear keyframes: list of (time, value) pairs sorted by time.
    Keyframes(Vec<(f32, f32)>),
    /// Driven entirely by a MathFunction evaluated at time t.
    MathDriven(MathFunction),
    /// Cubic bezier keyframes: (time, value, in_tangent, out_tangent).
    BezierKeyframes(Vec<BezierKey>),
}

#[derive(Debug, Clone)]
pub struct BezierKey {
    pub time:        f32,
    pub value:       f32,
    pub in_tangent:  f32,
    pub out_tangent: f32,
}

impl AnimationCurve {
    /// Evaluate the curve at normalized time `t` in [0, 1].
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            AnimationCurve::Constant(v) => *v,
            AnimationCurve::MathDriven(f) => f.evaluate(t, t),
            AnimationCurve::Keyframes(keys) => {
                if keys.is_empty() { return 0.0; }
                if keys.len() == 1 { return keys[0].1; }
                // Find surrounding keys
                let idx = keys.partition_point(|(kt, _)| *kt <= t);
                if idx == 0 { return keys[0].1; }
                if idx >= keys.len() { return keys[keys.len()-1].1; }
                let (t0, v0) = keys[idx-1];
                let (t1, v1) = keys[idx];
                let span = (t1 - t0).max(1e-7);
                let alpha = (t - t0) / span;
                v0 + (v1 - v0) * alpha
            }
            AnimationCurve::BezierKeyframes(keys) => {
                if keys.is_empty() { return 0.0; }
                if keys.len() == 1 { return keys[0].value; }
                let idx = keys.partition_point(|k| k.time <= t);
                if idx == 0 { return keys[0].value; }
                if idx >= keys.len() { return keys[keys.len()-1].value; }
                let k0 = &keys[idx-1];
                let k1 = &keys[idx];
                let span = (k1.time - k0.time).max(1e-7);
                let u = (t - k0.time) / span;
                // Cubic Hermite
                let h00 = 2.0*u*u*u - 3.0*u*u + 1.0;
                let h10 = u*u*u - 2.0*u*u + u;
                let h01 = -2.0*u*u*u + 3.0*u*u;
                let h11 = u*u*u - u*u;
                h00*k0.value + h10*span*k0.out_tangent + h01*k1.value + h11*span*k1.in_tangent
            }
        }
    }

    /// Build a constant curve.
    pub fn constant(v: f32) -> Self { Self::Constant(v) }

    /// Build a linear ramp from `a` at t=0 to `b` at t=1.
    pub fn linear(a: f32, b: f32) -> Self {
        Self::Keyframes(vec![(0.0, a), (1.0, b)])
    }

    /// Build a curve from raw (time, value) pairs.
    pub fn from_keys(mut keys: Vec<(f32, f32)>) -> Self {
        keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        Self::Keyframes(keys)
    }
}

// ── AnimationClip ──────────────────────────────────────────────────────────────

/// Named set of channels that animate properties over time.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name:     String,
    pub duration: f32,
    pub looping:  bool,
    /// Property path -> curve. E.g. "position.x", "scale.x", "color.r".
    pub channels: HashMap<String, AnimationCurve>,
    /// Events fired at specific normalized times.
    pub events:   Vec<AnimationEvent>,
}

#[derive(Debug, Clone)]
pub struct AnimationEvent {
    /// Normalized time [0, 1] when this fires.
    pub time:    f32,
    /// Arbitrary tag passed to event handlers.
    pub tag:     String,
    pub payload: f32,
}

impl AnimationClip {
    pub fn new(name: impl Into<String>, duration: f32, looping: bool) -> Self {
        Self {
            name: name.into(),
            duration,
            looping,
            channels: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Add a channel curve.
    pub fn with_channel(mut self, path: impl Into<String>, curve: AnimationCurve) -> Self {
        self.channels.insert(path.into(), curve);
        self
    }

    /// Add an event that fires at normalized time `t`.
    pub fn with_event(mut self, t: f32, tag: impl Into<String>, payload: f32) -> Self {
        self.events.push(AnimationEvent { time: t, tag: tag.into(), payload });
        self
    }

    /// Sample all channels at normalized time `t`, returning (path, value) pairs.
    pub fn sample(&self, t: f32) -> Vec<(String, f32)> {
        self.channels.iter()
            .map(|(path, curve)| (path.clone(), curve.evaluate(t)))
            .collect()
    }
}

// ── BlendTree ─────────────────────────────────────────────────────────────────

/// Blend mode for a BlendTree.
#[derive(Debug, Clone)]
pub enum BlendTreeKind {
    /// Blend between clips based on a single float parameter.
    Linear1D { param: String, thresholds: Vec<f32> },
    /// Blend between clips in a 2D parameter space.
    Cartesian2D { param_x: String, param_y: String, positions: Vec<(f32, f32)> },
    /// Additive blend — base clip plus additive clips.
    Additive { base_index: usize },
    /// Override blend — highest-weight non-zero clip wins.
    Override,
}

#[derive(Debug, Clone)]
pub struct BlendTree {
    pub kind:   BlendTreeKind,
    pub clips:  Vec<AnimationClip>,
    pub weights: Vec<f32>,
}

impl BlendTree {
    /// Compute blend weights given current parameter values.
    pub fn compute_weights(&mut self, params: &HashMap<String, ParamValue>) {
        let n = self.clips.len();
        if n == 0 { return; }
        self.weights.resize(n, 0.0);

        match &self.kind {
            BlendTreeKind::Linear1D { param, thresholds } => {
                let v = params.get(param).and_then(|p| p.as_float()).unwrap_or(0.0);
                let thresholds = thresholds.clone();
                if thresholds.len() != n { return; }
                // Find surrounding pair
                let idx = thresholds.partition_point(|&t| t <= v);
                for w in &mut self.weights { *w = 0.0; }
                if idx == 0 {
                    self.weights[0] = 1.0;
                } else if idx >= n {
                    self.weights[n-1] = 1.0;
                } else {
                    let t0 = thresholds[idx-1];
                    let t1 = thresholds[idx];
                    let span = (t1 - t0).max(1e-7);
                    let alpha = (v - t0) / span;
                    self.weights[idx-1] = 1.0 - alpha;
                    self.weights[idx]   = alpha;
                }
            }
            BlendTreeKind::Cartesian2D { param_x, param_y, positions } => {
                let px = params.get(param_x).and_then(|p| p.as_float()).unwrap_or(0.0);
                let py = params.get(param_y).and_then(|p| p.as_float()).unwrap_or(0.0);
                let positions = positions.clone();
                // Inverse Distance Weighting
                let dists: Vec<f32> = positions.iter()
                    .map(|(x, y)| ((px - x).powi(2) + (py - y).powi(2)).sqrt().max(1e-6))
                    .collect();
                let sum: f32 = dists.iter().map(|d| 1.0 / d).sum();
                for (i, d) in dists.iter().enumerate() {
                    self.weights[i] = (1.0 / d) / sum.max(1e-7);
                }
            }
            BlendTreeKind::Additive { .. } | BlendTreeKind::Override => {
                // Weights set externally
            }
        }
    }

    /// Sample blended output at normalized time `t`.
    pub fn sample(&self, t: f32) -> Vec<(String, f32)> {
        if self.clips.is_empty() { return Vec::new(); }
        let mut accum: HashMap<String, f32> = HashMap::new();
        let mut total_weight = 0.0_f32;

        for (clip, &w) in self.clips.iter().zip(self.weights.iter()) {
            if w < 1e-6 { continue; }
            total_weight += w;
            for (path, val) in clip.sample(t) {
                *accum.entry(path).or_insert(0.0) += val * w;
            }
        }

        if total_weight > 1e-6 {
            for v in accum.values_mut() { *v /= total_weight; }
        }
        accum.into_iter().collect()
    }
}

// ── Condition ─────────────────────────────────────────────────────────────────

/// Condition that must be satisfied for a state transition to fire.
#[derive(Debug, Clone)]
pub enum Condition {
    FloatGt { param: String, threshold: f32 },
    FloatLt { param: String, threshold: f32 },
    FloatGe { param: String, threshold: f32 },
    FloatLe { param: String, threshold: f32 },
    FloatEq { param: String, value: f32, tolerance: f32 },
    BoolTrue  { param: String },
    BoolFalse { param: String },
    /// One-shot: fires once then resets to false.
    Trigger   { param: String },
    /// Always true — transition fires immediately when source state exits.
    Always,
    /// Multiple conditions all true.
    All(Vec<Condition>),
    /// At least one condition true.
    Any(Vec<Condition>),
}

impl Condition {
    pub fn float_gt(param: impl Into<String>, v: f32) -> Self {
        Self::FloatGt { param: param.into(), threshold: v }
    }
    pub fn float_lt(param: impl Into<String>, v: f32) -> Self {
        Self::FloatLt { param: param.into(), threshold: v }
    }
    pub fn float_ge(param: impl Into<String>, v: f32) -> Self {
        Self::FloatGe { param: param.into(), threshold: v }
    }
    pub fn float_le(param: impl Into<String>, v: f32) -> Self {
        Self::FloatLe { param: param.into(), threshold: v }
    }
    pub fn bool_true(param: impl Into<String>) -> Self {
        Self::BoolTrue { param: param.into() }
    }
    pub fn trigger(param: impl Into<String>) -> Self {
        Self::Trigger { param: param.into() }
    }

    /// Evaluate against current params. Returns (satisfied, consumed_triggers).
    pub fn evaluate(&self, params: &HashMap<String, ParamValue>) -> (bool, Vec<String>) {
        match self {
            Self::FloatGt { param, threshold } =>
                (params.get(param).and_then(|p| p.as_float()).unwrap_or(0.0) > *threshold, vec![]),
            Self::FloatLt { param, threshold } =>
                (params.get(param).and_then(|p| p.as_float()).unwrap_or(0.0) < *threshold, vec![]),
            Self::FloatGe { param, threshold } =>
                (params.get(param).and_then(|p| p.as_float()).unwrap_or(0.0) >= *threshold, vec![]),
            Self::FloatLe { param, threshold } =>
                (params.get(param).and_then(|p| p.as_float()).unwrap_or(0.0) <= *threshold, vec![]),
            Self::FloatEq { param, value, tolerance } => {
                let v = params.get(param).and_then(|p| p.as_float()).unwrap_or(0.0);
                ((v - value).abs() <= *tolerance, vec![])
            }
            Self::BoolTrue  { param } =>
                (params.get(param).and_then(|p| p.as_bool()).unwrap_or(false), vec![]),
            Self::BoolFalse { param } =>
                (!params.get(param).and_then(|p| p.as_bool()).unwrap_or(false), vec![]),
            Self::Trigger { param } => {
                let v = params.get(param).and_then(|p| p.as_bool()).unwrap_or(false);
                if v { (true, vec![param.clone()]) } else { (false, vec![]) }
            }
            Self::Always => (true, vec![]),
            Self::All(conds) => {
                let mut consumed = Vec::new();
                for c in conds {
                    let (ok, mut trig) = c.evaluate(params);
                    if !ok { return (false, vec![]); }
                    consumed.append(&mut trig);
                }
                (true, consumed)
            }
            Self::Any(conds) => {
                for c in conds {
                    let (ok, trig) = c.evaluate(params);
                    if ok { return (true, trig); }
                }
                (false, vec![])
            }
        }
    }
}

// ── ParamValue ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ParamValue {
    Float(f32),
    Bool(bool),
    Int(i32),
}

impl ParamValue {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f32),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self { Some(*v) } else { None }
    }
    pub fn as_int(&self) -> Option<i32> {
        if let Self::Int(v) = self { Some(*v) } else { None }
    }
}

// ── Transition ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Transition {
    pub from:            String,
    pub to:              String,
    pub condition:       Condition,
    /// Crossfade duration in seconds.
    pub duration:        f32,
    /// If true, can interrupt an existing transition.
    pub can_interrupt:   bool,
    /// Minimum time in source state before transition is eligible (seconds).
    pub exit_time:       Option<f32>,
    /// Normalized exit time: transition fires when state reaches this fraction.
    pub normalized_exit: Option<f32>,
}

impl Transition {
    pub fn new(from: impl Into<String>, to: impl Into<String>, cond: Condition) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            condition: cond,
            duration: 0.15,
            can_interrupt: false,
            exit_time: None,
            normalized_exit: None,
        }
    }

    pub fn with_duration(mut self, d: f32) -> Self { self.duration = d; self }
    pub fn interruptible(mut self) -> Self { self.can_interrupt = true; self }
    pub fn exit_at(mut self, t: f32) -> Self { self.exit_time = Some(t); self }
    pub fn exit_normalized(mut self, t: f32) -> Self { self.normalized_exit = Some(t); self }
}

// ── AnimationState ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum StateContent {
    Clip(AnimationClip),
    Tree(BlendTree),
}

#[derive(Debug, Clone)]
pub struct AnimationState {
    pub name:       String,
    pub content:    StateContent,
    pub speed:      f32,
    pub mirror:     bool,
    pub cyclic_offset: f32,
    /// MathFunction modulating playback speed over normalized time.
    pub speed_curve: Option<MathFunction>,
}

impl AnimationState {
    pub fn clip(clip: AnimationClip) -> Self {
        let name = clip.name.clone();
        Self {
            name,
            content: StateContent::Clip(clip),
            speed: 1.0,
            mirror: false,
            cyclic_offset: 0.0,
            speed_curve: None,
        }
    }

    pub fn tree(name: impl Into<String>, tree: BlendTree) -> Self {
        Self {
            name: name.into(),
            content: StateContent::Tree(tree),
            speed: 1.0,
            mirror: false,
            cyclic_offset: 0.0,
            speed_curve: None,
        }
    }

    pub fn with_speed(mut self, s: f32) -> Self { self.speed = s; self }
    pub fn mirrored(mut self) -> Self { self.mirror = true; self }

    pub fn duration(&self) -> f32 {
        match &self.content {
            StateContent::Clip(c) => c.duration,
            StateContent::Tree(t) => t.clips.iter().map(|c| c.duration).fold(0.0, f32::max),
        }
    }
}

// ── AnimationLayer ────────────────────────────────────────────────────────────

/// A layer runs its own state machine and blends on top of lower layers.
#[derive(Debug, Clone)]
pub struct AnimationLayer {
    pub name:    String,
    pub weight:  f32,
    /// Property paths this layer affects. Empty = all properties.
    pub mask:    Vec<String>,
    pub additive: bool,
    // Runtime state
    pub current_state: Option<String>,
    pub current_time:  f32,
    pub transition:    Option<ActiveTransition>,
}

#[derive(Debug, Clone)]
pub struct ActiveTransition {
    pub target_state: String,
    pub progress:     f32,   // 0..1
    pub duration:     f32,
    pub prev_time:    f32,
    pub prev_state:   String,
}

impl AnimationLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            weight: 1.0,
            mask: Vec::new(),
            additive: false,
            current_state: None,
            current_time: 0.0,
            transition: None,
        }
    }

    pub fn with_weight(mut self, w: f32) -> Self { self.weight = w; self }
    pub fn with_mask(mut self, mask: Vec<String>) -> Self { self.mask = mask; self }
    pub fn as_additive(mut self) -> Self { self.additive = true; self }
}

// ── AnimatorController ────────────────────────────────────────────────────────

/// Drives one or more AnimationLayers with shared parameter space.
pub struct AnimatorController {
    pub states:      HashMap<String, AnimationState>,
    pub transitions: Vec<Transition>,
    pub params:      HashMap<String, ParamValue>,
    pub layers:      Vec<AnimationLayer>,
    /// Fired events since last call to drain_events().
    events:          Vec<FiredEvent>,
    /// Clip library for fetching clips by name.
    pub clips:       HashMap<String, AnimationClip>,
}

#[derive(Debug, Clone)]
pub struct FiredEvent {
    pub layer:   String,
    pub state:   String,
    pub tag:     String,
    pub payload: f32,
}

/// Sampled property output from a full controller tick.
#[derive(Debug, Default, Clone)]
pub struct AnimationOutput {
    pub channels: HashMap<String, f32>,
}

impl AnimatorController {
    pub fn new() -> Self {
        let mut layers = vec![AnimationLayer::new("Base Layer")];
        layers[0].weight = 1.0;
        Self {
            states: HashMap::new(),
            transitions: Vec::new(),
            params: HashMap::new(),
            layers,
            events: Vec::new(),
            clips: HashMap::new(),
        }
    }

    // ── Builder ──────────────────────────────────────────────────────────────

    pub fn add_state(&mut self, state: AnimationState) {
        self.states.insert(state.name.clone(), state);
    }

    pub fn add_clip(&mut self, clip: AnimationClip) {
        self.clips.insert(clip.name.clone(), clip);
    }

    pub fn add_transition(&mut self, t: Transition) {
        self.transitions.push(t);
    }

    pub fn add_layer(&mut self, layer: AnimationLayer) {
        self.layers.push(layer);
    }

    // ── Parameters ───────────────────────────────────────────────────────────

    pub fn set_float(&mut self, name: &str, v: f32) {
        self.params.insert(name.to_owned(), ParamValue::Float(v));
    }

    pub fn set_bool(&mut self, name: &str, v: bool) {
        self.params.insert(name.to_owned(), ParamValue::Bool(v));
    }

    pub fn set_int(&mut self, name: &str, v: i32) {
        self.params.insert(name.to_owned(), ParamValue::Int(v));
    }

    /// Set a trigger (one-shot bool that resets after being consumed).
    pub fn set_trigger(&mut self, name: &str) {
        self.params.insert(name.to_owned(), ParamValue::Bool(true));
    }

    pub fn get_float(&self, name: &str) -> f32 {
        self.params.get(name).and_then(|p| p.as_float()).unwrap_or(0.0)
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.params.get(name).and_then(|p| p.as_bool()).unwrap_or(false)
    }

    // ── Entry point ──────────────────────────────────────────────────────────

    pub fn start(&mut self, state_name: &str) {
        for layer in &mut self.layers {
            layer.current_state = Some(state_name.to_owned());
            layer.current_time  = 0.0;
            layer.transition    = None;
        }
    }

    pub fn start_layer(&mut self, layer_name: &str, state_name: &str) {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.name == layer_name) {
            layer.current_state = Some(state_name.to_owned());
            layer.current_time  = 0.0;
            layer.transition    = None;
        }
    }

    // ── Tick ─────────────────────────────────────────────────────────────────

    /// Advance all layers by `dt` seconds and evaluate transitions.
    /// Returns blended output across all layers.
    pub fn tick(&mut self, dt: f32) -> AnimationOutput {
        let mut output = AnimationOutput::default();

        // Collect triggered params to reset
        let mut consumed_triggers: Vec<String> = Vec::new();

        for layer in &mut self.layers {
            if layer.current_state.is_none() { continue; }
            let cur_name = layer.current_state.clone().unwrap();

            // Advance transition if active
            if let Some(ref mut tr) = layer.transition {
                tr.progress += dt / tr.duration.max(1e-4);
                tr.prev_time += dt;
                if tr.progress >= 1.0 {
                    // Transition complete
                    let new_state = tr.target_state.clone();
                    layer.current_time = 0.0;
                    layer.current_state = Some(new_state);
                    layer.transition = None;
                }
            } else {
                // Check transitions from current state
                let applicable: Vec<Transition> = self.transitions.iter()
                    .filter(|t| t.from == cur_name || t.from == "*")
                    .cloned()
                    .collect();

                let state_dur = self.states.get(&cur_name).map(|s| s.duration()).unwrap_or(1.0);

                for trans in applicable {
                    // Check exit time constraints
                    if let Some(min_exit) = trans.exit_time {
                        if layer.current_time < min_exit { continue; }
                    }
                    if let Some(norm_exit) = trans.normalized_exit {
                        let norm = if state_dur > 1e-6 { layer.current_time / state_dur } else { 1.0 };
                        if norm < norm_exit { continue; }
                    }

                    let (ok, mut trig) = trans.condition.evaluate(&self.params);
                    if ok {
                        consumed_triggers.append(&mut trig);
                        let prev = cur_name.clone();
                        layer.transition = Some(ActiveTransition {
                            target_state: trans.to.clone(),
                            progress: 0.0,
                            duration: trans.duration,
                            prev_time: layer.current_time,
                            prev_state: prev,
                        });
                        break;
                    }
                }

                // Advance current state time
                if let Some(state) = self.states.get(&cur_name) {
                    let speed_mod = if let Some(ref sf) = state.speed_curve {
                        let norm = if state.duration() > 1e-6 { layer.current_time / state.duration() } else { 0.0 };
                        sf.evaluate(norm, norm)
                    } else {
                        1.0
                    };
                    layer.current_time += dt * state.speed * speed_mod;
                    if state.duration() > 1e-6 {
                        if let StateContent::Clip(ref clip) = state.content {
                            if clip.looping {
                                layer.current_time %= clip.duration.max(1e-4);
                            } else {
                                layer.current_time = layer.current_time.min(clip.duration);
                            }
                        }
                    }
                }
            }

            // Sample current state
            let sample_t = {
                let dur = self.states.get(layer.current_state.as_deref().unwrap_or(""))
                    .map(|s| s.duration()).unwrap_or(1.0).max(1e-4);
                (layer.current_time / dur).clamp(0.0, 1.0)
            };

            if let Some(state) = self.states.get(layer.current_state.as_deref().unwrap_or("")) {
                let samples = match &state.content {
                    StateContent::Clip(c) => c.sample(sample_t),
                    StateContent::Tree(t) => t.sample(sample_t),
                };
                for (path, val) in samples {
                    let entry = output.channels.entry(path).or_insert(0.0);
                    if layer.additive {
                        *entry += val * layer.weight;
                    } else {
                        *entry = *entry * (1.0 - layer.weight) + val * layer.weight;
                    }
                }
            }
        }

        // Reset consumed triggers
        for key in consumed_triggers {
            self.params.insert(key, ParamValue::Bool(false));
        }

        output
    }

    /// Drain all fired animation events since last call.
    pub fn drain_events(&mut self) -> Vec<FiredEvent> {
        std::mem::take(&mut self.events)
    }

    /// Force the base layer into a specific state immediately.
    pub fn play(&mut self, state_name: &str) {
        if let Some(layer) = self.layers.first_mut() {
            layer.current_state = Some(state_name.to_owned());
            layer.current_time  = 0.0;
            layer.transition    = None;
        }
    }

    /// Cross-fade to a state over `duration` seconds.
    pub fn cross_fade(&mut self, state_name: &str, duration: f32) {
        if let Some(layer) = self.layers.first_mut() {
            let prev = layer.current_state.clone().unwrap_or_default();
            layer.transition = Some(ActiveTransition {
                target_state: state_name.to_owned(),
                progress: 0.0,
                duration,
                prev_time: layer.current_time,
                prev_state: prev,
            });
        }
    }

    /// Current normalized time of the base layer's active state.
    pub fn normalized_time(&self) -> f32 {
        if let Some(layer) = self.layers.first() {
            if let Some(name) = layer.current_state.as_deref() {
                if let Some(state) = self.states.get(name) {
                    let dur = state.duration().max(1e-4);
                    return (layer.current_time / dur).clamp(0.0, 1.0);
                }
            }
        }
        0.0
    }

    /// Name of the currently active state on the base layer.
    pub fn current_state(&self) -> Option<&str> {
        self.layers.first()?.current_state.as_deref()
    }

    /// Whether the base layer is transitioning.
    pub fn is_transitioning(&self) -> bool {
        self.layers.first().map(|l| l.transition.is_some()).unwrap_or(false)
    }
}

impl Default for AnimatorController {
    fn default() -> Self { Self::new() }
}

// ── RootMotion ────────────────────────────────────────────────────────────────

/// Root motion extracted from animation, applied to entity transform.
#[derive(Debug, Clone, Default)]
pub struct RootMotion {
    pub delta_position: glam::Vec3,
    pub delta_rotation: f32,
    pub delta_scale:    glam::Vec3,
}

impl RootMotion {
    pub fn from_output(output: &AnimationOutput) -> Self {
        let get = |key: &str| output.channels.get(key).copied().unwrap_or(0.0);
        Self {
            delta_position: glam::Vec3::new(get("root.dx"), get("root.dy"), get("root.dz")),
            delta_rotation: get("root.dr"),
            delta_scale:    glam::Vec3::ONE,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.delta_position.length_squared() < 1e-10 && self.delta_rotation.abs() < 1e-6
    }
}

// ── AnimationMirror ───────────────────────────────────────────────────────────

/// Mirrors an AnimationOutput left/right (negate X-axis channels).
pub fn mirror_output(output: &mut AnimationOutput) {
    for (key, val) in &mut output.channels {
        if key.ends_with(".x") || key.ends_with("_x") || key.contains("left") {
            *val = -*val;
        }
    }
}

// ── AnimationBlend helpers ────────────────────────────────────────────────────

/// Linearly blend two AnimationOutputs by alpha (0 = a, 1 = b).
pub fn blend_outputs(a: &AnimationOutput, b: &AnimationOutput, alpha: f32) -> AnimationOutput {
    let mut out = a.clone();
    for (key, bv) in &b.channels {
        let av = a.channels.get(key).copied().unwrap_or(0.0);
        out.channels.insert(key.clone(), av + (bv - av) * alpha);
    }
    out
}

/// Additively layer `additive` on top of `base` with `weight`.
pub fn add_output(base: &AnimationOutput, additive: &AnimationOutput, weight: f32) -> AnimationOutput {
    let mut out = base.clone();
    for (key, av) in &additive.channels {
        *out.channels.entry(key.clone()).or_insert(0.0) += av * weight;
    }
    out
}
