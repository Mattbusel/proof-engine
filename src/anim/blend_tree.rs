//! Blend trees, state machine, animator controller, IK solver, animation events.
//!
//! This module provides the runtime animation evaluation layer:
//! - [`BlendNode`] — recursive blend tree node (Clip, Lerp, Additive, LayerMask, etc.)
//! - [`BlendTree`] — evaluates a node tree to produce a [`Pose`]
//! - [`BlendSpace1D`] / [`BlendSpace2D`] — parameter-driven multi-clip blending
//! - [`StateMachine`] — transition-based state graph
//! - [`AnimatorController`] — top-level character animation driver
//! - [`IkSolver`] — FABRIK two-bone analytical IK with foot placement
//! - [`AnimationEventMarker`] / [`EventDispatcher`] — event callbacks at clip timestamps

use std::collections::HashMap;
use glam::{Quat, Vec3};

use super::skeleton::{BoneId, BoneMask, Pose, Skeleton, Transform3D};
use super::clips::{AnimationClip, AnimationClipSampler, ClipRegistry};

// ── BlendNode ──────────────────────────────────────────────────────────────────

/// A node in the recursive blend tree.
#[derive(Debug, Clone)]
pub enum BlendNode {
    /// Reference to a clip in the registry by name.
    Clip(String),

    /// Linear interpolation between two child nodes.
    Lerp {
        a: Box<BlendNode>,
        b: Box<BlendNode>,
        /// Blend factor [0, 1]: 0 = fully a, 1 = fully b.
        t: f32,
    },

    /// Additive blend: apply `additive` on top of `base` with `weight`.
    Additive {
        base:     Box<BlendNode>,
        additive: Box<BlendNode>,
        weight:   f32,
    },

    /// Layer a node on top of the base, restricted to bones in `mask`.
    LayerMask {
        base:  Box<BlendNode>,
        layer: Box<BlendNode>,
        mask:  BoneMask,
    },

    /// Play a child node at a scaled speed.
    SpeedScaled {
        child: Box<BlendNode>,
        speed: f32,
    },

    /// Override specific bones from `override_pose` based on `mask`.
    Override {
        base:          Box<BlendNode>,
        override_pose: Box<BlendNode>,
        mask:          BoneMask,
    },
}

impl BlendNode {
    /// Evaluate this node, sampling the registry and skeleton to produce a Pose.
    ///
    /// `time` is the raw (unscaled) playback time in seconds.
    pub fn evaluate(
        &self,
        registry: &ClipRegistry,
        skeleton: &Skeleton,
        time: f32,
    ) -> Pose {
        match self {
            BlendNode::Clip(name) => {
                if let Some(clip) = registry.get(name) {
                    let sampler = AnimationClipSampler::new(clip, skeleton);
                    sampler.sample_at(time)
                } else {
                    skeleton.rest_pose()
                }
            }

            BlendNode::Lerp { a, b, t } => {
                let pose_a = a.evaluate(registry, skeleton, time);
                let pose_b = b.evaluate(registry, skeleton, time);
                pose_a.blend(&pose_b, t.clamp(0.0, 1.0))
            }

            BlendNode::Additive { base, additive, weight } => {
                let base_pose = base.evaluate(registry, skeleton, time);
                let add_pose  = additive.evaluate(registry, skeleton, time);
                base_pose.add_pose(&add_pose, *weight)
            }

            BlendNode::LayerMask { base, layer, mask } => {
                let base_pose  = base.evaluate(registry, skeleton, time);
                let layer_pose = layer.evaluate(registry, skeleton, time);
                base_pose.apply_mask(&layer_pose, mask)
            }

            BlendNode::SpeedScaled { child, speed } => {
                child.evaluate(registry, skeleton, time * speed)
            }

            BlendNode::Override { base, override_pose, mask } => {
                let base_pose     = base.evaluate(registry, skeleton, time);
                let override_pose = override_pose.evaluate(registry, skeleton, time);
                base_pose.override_with_mask(&override_pose, mask, 0.5)
            }
        }
    }
}

// ── BlendTree ─────────────────────────────────────────────────────────────────

/// A blend tree wraps a root [`BlendNode`] and provides evaluation.
#[derive(Debug, Clone)]
pub struct BlendTree {
    pub root: BlendNode,
}

impl BlendTree {
    pub fn new(root: BlendNode) -> Self { Self { root } }

    /// Evaluate the blend tree to produce a [`Pose`].
    pub fn evaluate(
        &self,
        registry: &ClipRegistry,
        skeleton: &Skeleton,
        time: f32,
    ) -> Pose {
        self.root.evaluate(registry, skeleton, time)
    }

    /// Build a simple two-clip lerp tree.
    pub fn lerp(clip_a: impl Into<String>, clip_b: impl Into<String>, t: f32) -> Self {
        Self::new(BlendNode::Lerp {
            a: Box::new(BlendNode::Clip(clip_a.into())),
            b: Box::new(BlendNode::Clip(clip_b.into())),
            t,
        })
    }

    /// Build a single-clip tree.
    pub fn clip(name: impl Into<String>) -> Self {
        Self::new(BlendNode::Clip(name.into()))
    }

    /// Set the lerp factor if the root is a Lerp node.
    pub fn set_lerp_t(&mut self, new_t: f32) {
        if let BlendNode::Lerp { ref mut t, .. } = self.root {
            *t = new_t.clamp(0.0, 1.0);
        }
    }
}

// ── BlendSpace1D ──────────────────────────────────────────────────────────────

/// A 1-D blend space: linearly blend between clips based on a parameter value.
///
/// Clips are sorted by their threshold value. The parameter is interpolated
/// between the two nearest neighbours.
#[derive(Debug, Clone)]
pub struct BlendSpace1D {
    /// Sorted list of (threshold, clip_name).
    entries:   Vec<(f32, String)>,
    parameter: f32,
}

impl BlendSpace1D {
    pub fn new() -> Self {
        Self { entries: Vec::new(), parameter: 0.0 }
    }

    /// Add a clip at the given threshold value.
    pub fn add(mut self, threshold: f32, clip_name: impl Into<String>) -> Self {
        self.entries.push((threshold, clip_name.into()));
        self.entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        self
    }

    /// Set the current parameter value (drives blend weights).
    pub fn set_parameter(&mut self, value: f32) {
        self.parameter = value;
    }

    /// Current parameter value.
    pub fn parameter(&self) -> f32 { self.parameter }

    /// Compute the blend weights for each entry at the current parameter.
    pub fn weights(&self) -> Vec<f32> {
        let n = self.entries.len();
        if n == 0 { return Vec::new(); }
        let mut out = vec![0.0f32; n];
        if n == 1 {
            out[0] = 1.0;
            return out;
        }
        let v = self.parameter;
        let idx = self.entries.partition_point(|(t, _)| *t <= v);
        if idx == 0 {
            out[0] = 1.0;
        } else if idx >= n {
            out[n - 1] = 1.0;
        } else {
            let (t0, _) = &self.entries[idx - 1];
            let (t1, _) = &self.entries[idx];
            let span = (t1 - t0).max(1e-7);
            let alpha = (v - t0) / span;
            out[idx - 1] = 1.0 - alpha;
            out[idx]     = alpha;
        }
        out
    }

    /// Evaluate to a blended pose.
    pub fn evaluate(
        &self,
        registry: &ClipRegistry,
        skeleton: &Skeleton,
        time: f32,
    ) -> Pose {
        let weights = self.weights();
        if weights.is_empty() { return skeleton.rest_pose(); }

        let mut accumulated: Option<Pose> = None;
        let mut total_w = 0.0f32;

        for (i, &w) in weights.iter().enumerate() {
            if w < 1e-6 { continue; }
            let (_, clip_name) = &self.entries[i];
            let pose = if let Some(clip) = registry.get(clip_name) {
                AnimationClipSampler::new(clip, skeleton).sample_at(time)
            } else {
                continue;
            };
            match &accumulated {
                None => {
                    accumulated = Some(pose);
                    total_w = w;
                }
                Some(prev) => {
                    let blend_t = w / (total_w + w);
                    accumulated = Some(prev.blend(&pose, blend_t));
                    total_w += w;
                }
            }
        }

        accumulated.unwrap_or_else(|| skeleton.rest_pose())
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

impl Default for BlendSpace1D {
    fn default() -> Self { Self::new() }
}

// ── BlendSpace2D ──────────────────────────────────────────────────────────────

/// A 2-D blend space: blend between clips via barycentric interpolation in 2D.
///
/// The blend space stores (x, y, clip_name) samples. When evaluated at position
/// (px, py), it finds the three nearest samples and blends them barycentrically.
#[derive(Debug, Clone)]
pub struct BlendSpace2D {
    entries:    Vec<(f32, f32, String)>,
    position_x: f32,
    position_y: f32,
}

impl BlendSpace2D {
    pub fn new() -> Self {
        Self { entries: Vec::new(), position_x: 0.0, position_y: 0.0 }
    }

    /// Add a clip at a 2D position.
    pub fn add(mut self, x: f32, y: f32, clip_name: impl Into<String>) -> Self {
        self.entries.push((x, y, clip_name.into()));
        self
    }

    /// Set the current 2D blend position.
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position_x = x;
        self.position_y = y;
    }

    pub fn position(&self) -> (f32, f32) { (self.position_x, self.position_y) }

    /// Compute distance-weighted blend weights (Inverse Distance Weighting).
    pub fn weights(&self) -> Vec<f32> {
        let n = self.entries.len();
        if n == 0 { return Vec::new(); }
        if n == 1 { return vec![1.0]; }

        let px = self.position_x;
        let py = self.position_y;

        let dists: Vec<f32> = self.entries.iter()
            .map(|(ex, ey, _)| ((px - ex).powi(2) + (py - ey).powi(2)).sqrt())
            .collect();

        // If we're exactly on a sample point, return 1 for that point.
        if let Some(exact) = dists.iter().position(|&d| d < 1e-6) {
            let mut out = vec![0.0f32; n];
            out[exact] = 1.0;
            return out;
        }

        // IDW with power=2.
        let inv_dists: Vec<f32> = dists.iter().map(|&d| 1.0 / (d * d)).collect();
        let sum: f32 = inv_dists.iter().sum();
        inv_dists.iter().map(|&id| id / sum.max(1e-10)).collect()
    }

    /// Evaluate to a blended pose using the 3 nearest clips.
    pub fn evaluate(
        &self,
        registry: &ClipRegistry,
        skeleton: &Skeleton,
        time: f32,
    ) -> Pose {
        let weights = self.weights();
        if weights.is_empty() { return skeleton.rest_pose(); }

        let mut indexed: Vec<(usize, f32)> = weights.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let top3 = &indexed[..3.min(indexed.len())];

        let sum: f32 = top3.iter().map(|(_, w)| w).sum();
        if sum < 1e-6 { return skeleton.rest_pose(); }

        let mut result: Option<Pose> = None;
        let mut acc_w = 0.0f32;

        for &(idx, w) in top3 {
            let norm_w = w / sum;
            if norm_w < 1e-6 { continue; }
            let (_, _, ref clip_name) = self.entries[idx];
            let pose = if let Some(clip) = registry.get(clip_name) {
                AnimationClipSampler::new(clip, skeleton).sample_at(time)
            } else {
                continue;
            };
            match &result {
                None => {
                    result = Some(pose);
                    acc_w  = norm_w;
                }
                Some(prev) => {
                    let blend_t = norm_w / (acc_w + norm_w);
                    result = Some(prev.blend(&pose, blend_t));
                    acc_w += norm_w;
                }
            }
        }

        result.unwrap_or_else(|| skeleton.rest_pose())
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

impl Default for BlendSpace2D {
    fn default() -> Self { Self::new() }
}

// ── Parameter types ───────────────────────────────────────────────────────────

/// The type and value of an animator parameter.
#[derive(Debug, Clone)]
pub enum ParamValue {
    Float(f32),
    Bool(bool),
    Int(i32),
    /// Triggers are one-shot: consumed on the first transition that reads them.
    Trigger,
}

impl ParamValue {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v)   => Some(*v as f32),
            _              => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self { Some(*v) } else { None }
    }
    pub fn as_int(&self) -> Option<i32> {
        if let Self::Int(v) = self { Some(*v) } else { None }
    }
    pub fn is_trigger(&self) -> bool { matches!(self, Self::Trigger) }
}

// ── State / Transition ────────────────────────────────────────────────────────

/// A single state in the [`StateMachine`].
#[derive(Debug, Clone)]
pub struct State {
    pub name:       String,
    pub blend_tree: BlendTree,
    pub speed:      f32,
    pub looping:    bool,
}

impl State {
    pub fn new(name: impl Into<String>, blend_tree: BlendTree) -> Self {
        Self {
            name: name.into(),
            blend_tree,
            speed: 1.0,
            looping: true,
        }
    }

    pub fn with_speed(mut self, speed: f32) -> Self { self.speed = speed; self }
    pub fn with_looping(mut self, looping: bool) -> Self { self.looping = looping; self }
}

/// A condition string evaluated against the parameter map.
///
/// Format: `"param_name op value"` e.g. `"speed > 0.5"`, `"grounded == true"`,
/// `"jump"` (trigger name alone), `"always"`.
#[derive(Debug, Clone)]
pub struct TransitionCondition(pub String);

impl TransitionCondition {
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }

    /// Evaluate the condition against the parameter map.
    /// Returns `(satisfied, trigger_names_to_consume)`.
    pub fn evaluate(&self, params: &HashMap<String, ParamValue>) -> (bool, Vec<String>) {
        let s = self.0.trim();
        if s == "always" {
            return (true, Vec::new());
        }

        let parts: Vec<&str> = s.splitn(3, ' ').collect();
        if parts.len() == 3 {
            let name  = parts[0];
            let op    = parts[1];
            let val_s = parts[2];
            return self.eval_comparison(params, name, op, val_s);
        }

        // Single token: treat as trigger name or bool parameter.
        let name = s;
        if let Some(param) = params.get(name) {
            match param {
                ParamValue::Trigger    => return (true, vec![name.to_owned()]),
                ParamValue::Bool(true) => return (true, Vec::new()),
                _                     => {}
            }
        }
        (false, Vec::new())
    }

    fn eval_comparison(
        &self,
        params: &HashMap<String, ParamValue>,
        name: &str,
        op: &str,
        val_s: &str,
    ) -> (bool, Vec<String>) {
        let param = params.get(name);
        if let Ok(rhs) = val_s.parse::<f32>() {
            let lhs = param.and_then(|p| p.as_float()).unwrap_or(0.0);
            let ok = match op {
                ">"  => lhs > rhs,
                ">=" => lhs >= rhs,
                "<"  => lhs < rhs,
                "<=" => lhs <= rhs,
                "==" => (lhs - rhs).abs() < 1e-6,
                "!=" => (lhs - rhs).abs() >= 1e-6,
                _    => false,
            };
            return (ok, Vec::new());
        }
        if val_s == "true" || val_s == "false" {
            let rhs = val_s == "true";
            let lhs = param.and_then(|p| p.as_bool()).unwrap_or(false);
            let ok = match op {
                "==" => lhs == rhs,
                "!=" => lhs != rhs,
                _    => false,
            };
            return (ok, Vec::new());
        }
        if let Ok(rhs) = val_s.parse::<i32>() {
            let lhs = param.and_then(|p| p.as_int()).unwrap_or(0);
            let ok = match op {
                ">"  => lhs > rhs,
                ">=" => lhs >= rhs,
                "<"  => lhs < rhs,
                "<=" => lhs <= rhs,
                "==" => lhs == rhs,
                "!=" => lhs != rhs,
                _    => false,
            };
            return (ok, Vec::new());
        }
        (false, Vec::new())
    }
}

/// A transition between two states.
#[derive(Debug, Clone)]
pub struct Transition {
    pub from:          String,
    pub to:            String,
    /// Crossfade duration in seconds.
    pub duration:      f32,
    pub condition:     TransitionCondition,
    /// If `true`, the transition only fires after the source state reaches `exit_time`.
    pub has_exit_time: bool,
    /// Normalised exit time [0, 1] in the source clip.
    pub exit_time:     f32,
    /// Priority: higher values are checked first.
    pub priority:      i32,
}

impl Transition {
    pub fn new(from: impl Into<String>, to: impl Into<String>, condition: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            duration: 0.15,
            condition: TransitionCondition::new(condition),
            has_exit_time: false,
            exit_time: 1.0,
            priority: 0,
        }
    }

    pub fn with_duration(mut self, d: f32) -> Self { self.duration = d; self }
    pub fn with_exit_time(mut self, t: f32) -> Self { self.has_exit_time = true; self.exit_time = t; self }
    pub fn with_priority(mut self, p: i32) -> Self { self.priority = p; self }
}

// ── StateMachine ──────────────────────────────────────────────────────────────

/// Active transition data (runtime only).
#[derive(Debug, Clone)]
struct ActiveTransition {
    target:          String,
    progress:        f32,
    duration:        f32,
    prev_state_time: f32,
}

/// A hierarchical state machine that drives blend-tree evaluation.
#[derive(Debug)]
pub struct StateMachine {
    pub states:      HashMap<String, State>,
    pub transitions: Vec<Transition>,
    pub params:      HashMap<String, ParamValue>,
    current:         Option<String>,
    current_time:    f32,
    active_trans:    Option<ActiveTransition>,
    default_state:   Option<String>,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            states:        HashMap::new(),
            transitions:   Vec::new(),
            params:        HashMap::new(),
            current:       None,
            current_time:  0.0,
            active_trans:  None,
            default_state: None,
        }
    }

    // ── Builder helpers ───────────────────────────────────────────────────

    pub fn add_state(&mut self, state: State) {
        if self.default_state.is_none() {
            self.default_state = Some(state.name.clone());
        }
        self.states.insert(state.name.clone(), state);
    }

    pub fn add_transition(&mut self, t: Transition) {
        self.transitions.push(t);
        self.transitions.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn set_default(&mut self, state_name: impl Into<String>) {
        self.default_state = Some(state_name.into());
    }

    // ── Parameters ───────────────────────────────────────────────────────

    pub fn set_float(&mut self, name: &str, v: f32) {
        self.params.insert(name.to_owned(), ParamValue::Float(v));
    }

    pub fn set_bool(&mut self, name: &str, v: bool) {
        self.params.insert(name.to_owned(), ParamValue::Bool(v));
    }

    pub fn set_int(&mut self, name: &str, v: i32) {
        self.params.insert(name.to_owned(), ParamValue::Int(v));
    }

    /// Set a one-shot trigger parameter.
    pub fn set_trigger(&mut self, name: &str) {
        self.params.insert(name.to_owned(), ParamValue::Trigger);
    }

    pub fn get_float(&self, name: &str) -> f32 {
        self.params.get(name).and_then(|p| p.as_float()).unwrap_or(0.0)
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.params.get(name).and_then(|p| p.as_bool()).unwrap_or(false)
    }

    // ── Playback ──────────────────────────────────────────────────────────

    /// Start the state machine in a specific state.
    pub fn play(&mut self, state_name: &str) {
        self.current      = Some(state_name.to_owned());
        self.current_time = 0.0;
        self.active_trans = None;
    }

    /// Start in the default state (first added).
    pub fn start(&mut self) {
        if let Some(name) = self.default_state.clone() {
            self.play(&name);
        }
    }

    /// The name of the currently active state.
    pub fn current_state(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Whether a transition is in progress.
    pub fn is_transitioning(&self) -> bool { self.active_trans.is_some() }

    // ── Update ────────────────────────────────────────────────────────────

    /// Advance the state machine by `dt` seconds. Returns consumed trigger names.
    pub fn update(&mut self, dt: f32) -> Vec<String> {
        if self.current.is_none() {
            self.start();
        }

        let mut consumed_triggers: Vec<String> = Vec::new();

        // Advance/complete existing transition.
        if let Some(ref mut trans) = self.active_trans {
            trans.progress += dt / trans.duration.max(1e-4);
            trans.prev_state_time += dt;
            if trans.progress >= 1.0 {
                let target = trans.target.clone();
                self.current      = Some(target);
                self.current_time = 0.0;
                self.active_trans = None;
            }
        }

        if self.active_trans.is_none() {
            // Advance current state time.
            let speed = self.current.as_deref()
                .and_then(|n| self.states.get(n))
                .map(|s| s.speed)
                .unwrap_or(1.0);
            self.current_time += dt * speed;

            let cur_name = match &self.current {
                Some(n) => n.clone(),
                None    => return consumed_triggers,
            };

            // Compute normalised time for exit-time checks.
            let norm_t = {
                let dur = 1.0_f32; // Default duration for exit-time comparison
                if dur > 1e-6 { (self.current_time / dur).clamp(0.0, 1.0) } else { 1.0 }
            };

            // Collect applicable transitions (clone to avoid borrow issues).
            let applicable: Vec<Transition> = self.transitions.iter()
                .filter(|t| t.from == cur_name || t.from == "*")
                .cloned()
                .collect();

            for trans in applicable {
                if trans.has_exit_time && norm_t < trans.exit_time {
                    continue;
                }
                let (ok, mut trig) = trans.condition.evaluate(&self.params);
                if ok {
                    consumed_triggers.append(&mut trig);
                    // Consume triggers from params.
                    for name in &trig {
                        self.params.remove(name);
                    }
                    let prev_time = self.current_time;
                    self.active_trans = Some(ActiveTransition {
                        target: trans.to.clone(),
                        progress: 0.0,
                        duration: trans.duration,
                        prev_state_time: prev_time,
                    });
                    break;
                }
            }
        }

        consumed_triggers
    }

    /// Evaluate the current state (or blended transition) to a Pose.
    pub fn evaluate(
        &self,
        registry: &ClipRegistry,
        skeleton: &Skeleton,
    ) -> Pose {
        let cur_name = match &self.current {
            Some(n) => n.as_str(),
            None    => return skeleton.rest_pose(),
        };

        let cur_pose = if let Some(state) = self.states.get(cur_name) {
            state.blend_tree.evaluate(registry, skeleton, self.current_time)
        } else {
            skeleton.rest_pose()
        };

        // If transitioning, blend with target.
        if let Some(ref trans) = self.active_trans {
            let target_pose = if let Some(state) = self.states.get(trans.target.as_str()) {
                state.blend_tree.evaluate(registry, skeleton, 0.0)
            } else {
                skeleton.rest_pose()
            };
            cur_pose.blend(&target_pose, trans.progress.clamp(0.0, 1.0))
        } else {
            cur_pose
        }
    }
}

impl Default for StateMachine {
    fn default() -> Self { Self::new() }
}

// ── AnimatorController ────────────────────────────────────────────────────────

/// Top-level character animation controller.
///
/// Wraps a [`StateMachine`], [`ClipRegistry`], and [`Skeleton`] and exposes a
/// simple API for game-code use.
pub struct AnimatorController {
    pub state_machine: StateMachine,
    pub registry:      ClipRegistry,
    pub skeleton:      Skeleton,
    current_pose:      Pose,
}

impl AnimatorController {
    pub fn new(skeleton: Skeleton) -> Self {
        let pose = skeleton.rest_pose();
        Self {
            state_machine: StateMachine::new(),
            registry:      ClipRegistry::new(),
            skeleton,
            current_pose:  pose,
        }
    }

    // ── Clip management ───────────────────────────────────────────────────

    pub fn register_clip(&mut self, clip: AnimationClip) {
        self.registry.register(clip);
    }

    // ── State machine setup ───────────────────────────────────────────────

    pub fn add_state(&mut self, state: State) {
        self.state_machine.add_state(state);
    }

    pub fn add_transition(&mut self, t: Transition) {
        self.state_machine.add_transition(t);
    }

    // ── Parameters ───────────────────────────────────────────────────────

    pub fn set_float(&mut self, name: &str, v: f32) {
        self.state_machine.set_float(name, v);
    }

    pub fn set_bool(&mut self, name: &str, v: bool) {
        self.state_machine.set_bool(name, v);
    }

    pub fn set_int(&mut self, name: &str, v: i32) {
        self.state_machine.set_int(name, v);
    }

    /// Set a one-shot trigger.
    pub fn set_trigger(&mut self, name: &str) {
        self.state_machine.set_trigger(name);
    }

    // ── Update ────────────────────────────────────────────────────────────

    /// Advance the animator by `dt` seconds and re-evaluate the pose.
    pub fn update(&mut self, dt: f32) {
        self.state_machine.update(dt);
        self.current_pose = self.state_machine.evaluate(&self.registry, &self.skeleton);
    }

    /// The most recently evaluated pose.
    pub fn current_pose(&self) -> &Pose {
        &self.current_pose
    }

    /// Immediately play a state.
    pub fn play(&mut self, state_name: &str) {
        self.state_machine.play(state_name);
    }

    pub fn current_state(&self) -> Option<&str> {
        self.state_machine.current_state()
    }

    pub fn is_transitioning(&self) -> bool {
        self.state_machine.is_transitioning()
    }
}

// ── IkSolver ─────────────────────────────────────────────────────────────────

/// IK chain definition: an ordered list of bone ids from root to tip.
#[derive(Debug, Clone)]
pub struct IkChain {
    pub bones: Vec<BoneId>,
}

impl IkChain {
    pub fn new(bones: Vec<BoneId>) -> Self { Self { bones } }

    /// Two-bone chain (shoulder -> elbow -> wrist).
    pub fn two_bone(root: BoneId, mid: BoneId, tip: BoneId) -> Self {
        Self { bones: vec![root, mid, tip] }
    }
}

/// IK solver supporting FABRIK and analytical two-bone IK.
#[derive(Debug, Default)]
pub struct IkSolver;

impl IkSolver {
    pub fn new() -> Self { Self }

    /// FABRIK solver: iterates forward and backward passes until the tip is
    /// within `tolerance` of the `target`, or `iterations` is exhausted.
    ///
    /// Modifies `pose` in-place.
    pub fn solve_fabrik(
        &self,
        chain:      &IkChain,
        target:     Vec3,
        skeleton:   &Skeleton,
        pose:       &mut Pose,
        iterations: u32,
        tolerance:  f32,
    ) {
        let n = chain.bones.len();
        if n < 2 { return; }

        // Extract current world positions from pose.
        let mut positions = self.chain_world_positions(chain, skeleton, pose);

        // Compute segment lengths.
        let lengths: Vec<f32> = positions.windows(2)
            .map(|w| (w[1] - w[0]).length().max(1e-6))
            .collect();
        let total_length: f32 = lengths.iter().sum();

        let root_pos = positions[0];

        // If target is too far, fully extend the chain.
        if (target - root_pos).length() >= total_length {
            let dir = (target - root_pos).normalize_or_zero();
            let mut acc = root_pos;
            for i in 1..n {
                acc += dir * lengths[i - 1];
                positions[i] = acc;
            }
            self.apply_positions(chain, &positions, skeleton, pose);
            return;
        }

        for _ in 0..iterations {
            // Forward pass: move tip to target.
            positions[n - 1] = target;
            for i in (0..n - 1).rev() {
                let dir = (positions[i] - positions[i + 1]).normalize_or_zero();
                positions[i] = positions[i + 1] + dir * lengths[i];
            }

            // Backward pass: restore root.
            positions[0] = root_pos;
            for i in 0..n - 1 {
                let dir = (positions[i + 1] - positions[i]).normalize_or_zero();
                positions[i + 1] = positions[i] + dir * lengths[i];
            }

            // Check convergence.
            if (positions[n - 1] - target).length() < tolerance {
                break;
            }
        }

        self.apply_positions(chain, &positions, skeleton, pose);
    }

    /// Analytical two-bone IK (law of cosines).
    ///
    /// Solves for a 3-bone chain (root, mid, tip) analytically.
    /// `pole_hint` is a hint vector pointing toward the bend direction.
    pub fn solve_two_bone(
        &self,
        root_id:   BoneId,
        mid_id:    BoneId,
        tip_id:    BoneId,
        target:    Vec3,
        pole_hint: Vec3,
        skeleton:  &Skeleton,
        pose:      &mut Pose,
    ) {
        let chain = IkChain::two_bone(root_id, mid_id, tip_id);
        let positions = self.chain_world_positions(&chain, skeleton, pose);
        if positions.len() < 3 { return; }

        let root_pos = positions[0];
        let mid_pos  = positions[1];
        let tip_pos  = positions[2];

        let len_a = (mid_pos - root_pos).length();
        let len_b = (tip_pos - mid_pos).length();

        let to_target = target - root_pos;
        let dist = to_target.length().max(1e-6);
        let dist_clamped = dist.clamp((len_a - len_b).abs() + 1e-4, len_a + len_b - 1e-4);

        // Angle at root using law of cosines: cos(A) = (a^2 + c^2 - b^2) / (2ac)
        let cos_a = ((len_a * len_a + dist_clamped * dist_clamped - len_b * len_b)
            / (2.0 * len_a * dist_clamped))
            .clamp(-1.0, 1.0);
        let angle_a = cos_a.acos();

        // Axis perpendicular to target direction.
        let fwd  = to_target.normalize_or_zero();
        let side = fwd.cross(pole_hint).normalize_or_zero();
        let up   = side.cross(fwd).normalize_or_zero();

        // New mid position.
        let new_mid = root_pos + fwd * (len_a * cos_a) + up * (len_a * angle_a.sin());

        // Root rotation: look from root to new_mid.
        if let Some(root_xform) = pose.local_transforms.get_mut(root_id.index()) {
            let new_dir = (new_mid - root_pos).normalize_or_zero();
            let old_dir = (mid_pos  - root_pos).normalize_or_zero();
            if new_dir.length() > 1e-6 && old_dir.length() > 1e-6 {
                let rot = Quat::from_rotation_arc(old_dir, new_dir);
                root_xform.rotation = (rot * root_xform.rotation).normalize();
            }
        }

        // Mid rotation: look from new_mid to target.
        if let Some(mid_xform) = pose.local_transforms.get_mut(mid_id.index()) {
            let new_dir = (target  - new_mid).normalize_or_zero();
            let old_dir = (tip_pos - mid_pos).normalize_or_zero();
            if new_dir.length() > 1e-6 && old_dir.length() > 1e-6 {
                let rot = Quat::from_rotation_arc(old_dir, new_dir);
                mid_xform.rotation = (rot * mid_xform.rotation).normalize();
            }
        }
    }

    /// Foot placement IK: adjusts foot bones using a terrain height callback.
    ///
    /// `terrain_height_fn` receives a (x, z) position and returns the y height.
    pub fn foot_placement<F: Fn(f32, f32) -> f32>(
        &self,
        thigh_id: BoneId,
        calf_id:  BoneId,
        foot_id:  BoneId,
        skeleton: &Skeleton,
        pose:     &mut Pose,
        terrain_height_fn: &F,
    ) {
        let chain = IkChain::new(vec![thigh_id, calf_id, foot_id]);
        let positions = self.chain_world_positions(&chain, skeleton, pose);
        if positions.len() < 3 { return; }

        let foot_pos = positions[2];
        let terrain_y = terrain_height_fn(foot_pos.x, foot_pos.z);
        let target = Vec3::new(foot_pos.x, terrain_y, foot_pos.z);

        self.solve_fabrik(&chain, target, skeleton, pose, 10, 0.01);
    }

    /// Solve an IK chain generically (delegates to FABRIK).
    pub fn solve(
        &self,
        chain:      &IkChain,
        target:     Vec3,
        skeleton:   &Skeleton,
        pose:       &mut Pose,
        iterations: u32,
    ) {
        self.solve_fabrik(chain, target, skeleton, pose, iterations, 0.001);
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn chain_world_positions(
        &self,
        chain:    &IkChain,
        skeleton: &Skeleton,
        pose:     &Pose,
    ) -> Vec<Vec3> {
        let n = skeleton.len();
        let mut world_matrices = vec![glam::Mat4::IDENTITY; n.max(1)];

        for bone in &skeleton.bones {
            let idx = bone.id.index();
            let local = pose.local_transforms.get(idx)
                .copied()
                .unwrap_or_else(Transform3D::identity)
                .to_mat4();
            world_matrices[idx] = match bone.parent {
                None         => local,
                Some(parent) => world_matrices[parent.index()] * local,
            };
        }

        chain.bones.iter().map(|id| {
            let m = world_matrices.get(id.index()).copied().unwrap_or(glam::Mat4::IDENTITY);
            m.transform_point3(Vec3::ZERO)
        }).collect()
    }

    fn apply_positions(
        &self,
        chain:     &IkChain,
        positions: &[Vec3],
        _skeleton: &Skeleton,
        pose:      &mut Pose,
    ) {
        let n = chain.bones.len();
        for i in 0..n.saturating_sub(1) {
            let bone_id = chain.bones[i];
            let idx = bone_id.index();
            if idx >= pose.local_transforms.len() { continue; }

            let cur  = positions[i];
            let next = positions[i + 1];
            let new_dir = (next - cur).normalize_or_zero();
            if new_dir.length_squared() < 1e-6 { continue; }

            let natural_dir = Vec3::Y;
            let rot = Quat::from_rotation_arc(natural_dir, new_dir);
            pose.local_transforms[idx].rotation = rot.normalize();
        }
    }
}

// ── AnimationEventMarker ──────────────────────────────────────────────────────

/// An animation event that fires at a specific time within a clip.
#[derive(Debug, Clone)]
pub struct AnimationEventMarker {
    /// Absolute time in seconds within the clip.
    pub time:    f32,
    pub name:    String,
    pub payload: String,
}

impl AnimationEventMarker {
    pub fn new(time: f32, name: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { time, name: name.into(), payload: payload.into() }
    }
}

/// Callback signature for animation events.
pub type EventCallback = Box<dyn FnMut(&str, &str) + Send + Sync>;

/// Fires animation event callbacks when playback crosses event timestamps.
pub struct EventDispatcher {
    callbacks: Vec<(String, EventCallback)>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self { callbacks: Vec::new() }
    }

    /// Register a callback for events with the given name pattern.
    /// Use `"*"` to receive all events.
    pub fn on(&mut self, event_name: impl Into<String>, cb: EventCallback) {
        self.callbacks.push((event_name.into(), cb));
    }

    /// Fire all matching callbacks for a batch of events.
    pub fn dispatch(&mut self, events: &[AnimationEventMarker]) {
        for event in events {
            for (pattern, cb) in &mut self.callbacks {
                if pattern == "*" || *pattern == event.name {
                    cb(&event.name, &event.payload);
                }
            }
        }
    }

    /// Scan markers in the time window (prev_t, cur_t] and dispatch them.
    pub fn tick(
        &mut self,
        markers: &[AnimationEventMarker],
        prev_t:  f32,
        cur_t:   f32,
    ) {
        let fired: Vec<usize> = markers.iter()
            .enumerate()
            .filter(|(_, e)| e.time > prev_t && e.time <= cur_t)
            .map(|(i, _)| i)
            .collect();
        for idx in fired {
            let event = &markers[idx];
            for (pattern, cb) in &mut self.callbacks {
                if pattern == "*" || *pattern == event.name {
                    cb(&event.name, &event.payload);
                }
            }
        }
    }
}

impl Default for EventDispatcher {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::skeleton::{BoneId, SkeletonBuilder, Transform3D};
    use super::super::clips::{AnimationChannel, AnimationClip, Vec3Key};
    use glam::Vec3;

    fn make_skeleton() -> Skeleton {
        SkeletonBuilder::new()
            .add_bone("root",  None,          Transform3D::identity())
            .add_bone("spine", Some("root"),  Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .build()
    }

    fn make_registry_with_clips(names: &[&str]) -> ClipRegistry {
        let mut registry = ClipRegistry::new();
        for &name in names {
            let keys = vec![
                Vec3Key { time: 0.0, value: Vec3::ZERO, in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
                Vec3Key { time: 1.0, value: Vec3::new(1.0, 0.0, 0.0), in_tangent: Vec3::ZERO, out_tangent: Vec3::ZERO },
            ];
            let clip = AnimationClip::new(name, 1.0)
                .with_channel(AnimationChannel::translation(BoneId(0), keys));
            registry.register(clip);
        }
        registry
    }

    #[test]
    fn test_blend_node_clip_samples_rest_on_missing() {
        let skeleton = make_skeleton();
        let registry = ClipRegistry::new();
        let node = BlendNode::Clip("missing".to_string());
        let pose = node.evaluate(&registry, &skeleton, 0.0);
        assert_eq!(pose.len(), skeleton.len());
    }

    #[test]
    fn test_blend_node_lerp() {
        let skeleton = make_skeleton();
        let registry = make_registry_with_clips(&["clip_a", "clip_b"]);
        let node = BlendNode::Lerp {
            a: Box::new(BlendNode::Clip("clip_a".into())),
            b: Box::new(BlendNode::Clip("clip_b".into())),
            t: 0.5,
        };
        let pose = node.evaluate(&registry, &skeleton, 1.0);
        // At t=1.0, both clips have translation x=1.0; blend should also be ~1.0
        assert!((pose.local_transforms[0].translation.x - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_blend_tree_clip_direct() {
        let skeleton = make_skeleton();
        let registry = make_registry_with_clips(&["idle"]);
        let tree = BlendTree::clip("idle");
        let pose = tree.evaluate(&registry, &skeleton, 0.0);
        assert_eq!(pose.len(), skeleton.len());
    }

    #[test]
    fn test_blend_tree_lerp_set_t() {
        let mut tree = BlendTree::lerp("a", "b", 0.0);
        tree.set_lerp_t(0.75);
        if let BlendNode::Lerp { t, .. } = &tree.root {
            assert!((*t - 0.75).abs() < 1e-6);
        } else {
            panic!("Expected Lerp node");
        }
    }

    #[test]
    fn test_blend_space_1d_weights_midpoint() {
        let space = BlendSpace1D::new()
            .add(0.0, "idle")
            .add(1.0, "run");
        let mut space = space;
        space.set_parameter(0.5);
        let weights = space.weights();
        assert_eq!(weights.len(), 2);
        assert!((weights[0] - 0.5).abs() < 1e-5);
        assert!((weights[1] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_blend_space_1d_weights_at_boundary() {
        let mut space = BlendSpace1D::new()
            .add(0.0, "idle")
            .add(1.0, "run");
        space.set_parameter(0.0);
        let w = space.weights();
        assert!((w[0] - 1.0).abs() < 1e-5);
        assert!(w[1].abs() < 1e-5);
    }

    #[test]
    fn test_blend_space_2d_weights_exact_hit() {
        let mut space = BlendSpace2D::new()
            .add(0.0, 0.0, "idle")
            .add(1.0, 0.0, "run")
            .add(0.0, 1.0, "strafe");
        space.set_position(0.0, 0.0);
        let w = space.weights();
        assert!((w[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_blend_space_2d_evaluate_returns_pose() {
        let skeleton = make_skeleton();
        let registry = make_registry_with_clips(&["idle", "run", "strafe"]);
        let mut space = BlendSpace2D::new()
            .add(0.0, 0.0, "idle")
            .add(1.0, 0.0, "run")
            .add(0.0, 1.0, "strafe");
        space.set_position(0.5, 0.5);
        let pose = space.evaluate(&registry, &skeleton, 0.5);
        assert_eq!(pose.len(), skeleton.len());
    }

    #[test]
    fn test_state_machine_basic_transition() {
        let skeleton = make_skeleton();
        let registry = make_registry_with_clips(&["idle", "run"]);
        let mut sm = StateMachine::new();
        sm.add_state(State::new("idle", BlendTree::clip("idle")));
        sm.add_state(State::new("run",  BlendTree::clip("run")));
        sm.add_transition(
            Transition::new("idle", "run", "speed > 0.5").with_duration(0.1),
        );
        sm.start();
        sm.set_float("speed", 1.0);

        for _ in 0..20 {
            sm.update(0.01);
        }

        let _ = sm.evaluate(&registry, &skeleton);
        assert!(sm.current_state() == Some("run") || sm.is_transitioning());
    }

    #[test]
    fn test_state_machine_trigger_consumed() {
        let mut sm = StateMachine::new();
        sm.add_state(State::new("idle",   BlendTree::clip("idle")));
        sm.add_state(State::new("attack", BlendTree::clip("attack")));
        sm.add_transition(Transition::new("idle", "attack", "attack_trigger"));
        sm.start();
        sm.set_trigger("attack_trigger");
        sm.update(0.016);
        assert!(!sm.params.contains_key("attack_trigger") || sm.is_transitioning());
    }

    #[test]
    fn test_animator_controller_update_no_panic() {
        let skeleton = make_skeleton();
        let mut ctrl = AnimatorController::new(skeleton);
        ctrl.register_clip(AnimationClip::new("idle", 1.0));
        ctrl.add_state(State::new("idle", BlendTree::clip("idle")));
        ctrl.play("idle");
        ctrl.update(0.016);
        let pose = ctrl.current_pose();
        assert_eq!(pose.len(), ctrl.skeleton.len());
    }

    #[test]
    fn test_ik_solver_fabrik_no_panic() {
        let skeleton = SkeletonBuilder::new()
            .add_bone("root",   None,           Transform3D::new(Vec3::ZERO,               Quat::IDENTITY, Vec3::ONE))
            .add_bone("bone_a", Some("root"),   Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("bone_b", Some("bone_a"), Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .build();
        let mut pose = skeleton.rest_pose();
        let chain = IkChain::new(vec![BoneId(0), BoneId(1), BoneId(2)]);
        let solver = IkSolver::new();
        let target = Vec3::new(1.0, 1.0, 0.0);
        solver.solve_fabrik(&chain, target, &skeleton, &mut pose, 20, 0.01);
        assert_eq!(pose.len(), skeleton.len());
    }

    #[test]
    fn test_ik_solver_two_bone_no_panic() {
        let skeleton = SkeletonBuilder::new()
            .add_bone("root", None,          Transform3D::new(Vec3::ZERO,               Quat::IDENTITY, Vec3::ONE))
            .add_bone("mid",  Some("root"),  Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("tip",  Some("mid"),   Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .build();
        let mut pose = skeleton.rest_pose();
        let solver = IkSolver::new();
        solver.solve_two_bone(
            BoneId(0), BoneId(1), BoneId(2),
            Vec3::new(1.0, 1.5, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            &skeleton, &mut pose,
        );
        assert_eq!(pose.len(), skeleton.len());
    }

    #[test]
    fn test_event_dispatcher_fires_callback() {
        let mut dispatcher = EventDispatcher::new();
        let fired = std::sync::Arc::new(std::sync::Mutex::new(false));
        let fired_clone = fired.clone();
        dispatcher.on("footstep", Box::new(move |_name, _payload| {
            *fired_clone.lock().unwrap() = true;
        }));
        let events = vec![AnimationEventMarker::new(0.5, "footstep", "left")];
        dispatcher.dispatch(&events);
        assert!(*fired.lock().unwrap());
    }

    #[test]
    fn test_event_dispatcher_wildcard() {
        let mut dispatcher = EventDispatcher::new();
        let count = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let count_clone = count.clone();
        dispatcher.on("*", Box::new(move |_name, _payload| {
            *count_clone.lock().unwrap() += 1;
        }));
        let events = vec![
            AnimationEventMarker::new(0.1, "step_l", ""),
            AnimationEventMarker::new(0.6, "step_r", ""),
        ];
        dispatcher.dispatch(&events);
        assert_eq!(*count.lock().unwrap(), 2);
    }

    #[test]
    fn test_transition_condition_always() {
        let cond = TransitionCondition::new("always");
        let (ok, _) = cond.evaluate(&HashMap::new());
        assert!(ok);
    }

    #[test]
    fn test_transition_condition_float_gt() {
        let cond = TransitionCondition::new("speed > 0.5");
        let mut params = HashMap::new();
        params.insert("speed".to_owned(), ParamValue::Float(1.0));
        let (ok, _) = cond.evaluate(&params);
        assert!(ok);
        params.insert("speed".to_owned(), ParamValue::Float(0.2));
        let (ok2, _) = cond.evaluate(&params);
        assert!(!ok2);
    }

    #[test]
    fn test_blend_node_additive() {
        let skeleton = make_skeleton();
        let registry = make_registry_with_clips(&["base", "add"]);
        let node = BlendNode::Additive {
            base:     Box::new(BlendNode::Clip("base".into())),
            additive: Box::new(BlendNode::Clip("add".into())),
            weight:   0.5,
        };
        let pose = node.evaluate(&registry, &skeleton, 0.0);
        assert_eq!(pose.len(), skeleton.len());
    }

    #[test]
    fn test_ik_foot_placement() {
        let skeleton = SkeletonBuilder::new()
            .add_bone("thigh", None,           Transform3D::new(Vec3::new(0.0, 1.0, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("calf",  Some("thigh"),  Transform3D::new(Vec3::new(0.0, -0.5, 0.0), Quat::IDENTITY, Vec3::ONE))
            .add_bone("foot",  Some("calf"),   Transform3D::new(Vec3::new(0.0, -0.5, 0.0), Quat::IDENTITY, Vec3::ONE))
            .build();
        let mut pose = skeleton.rest_pose();
        let solver = IkSolver::new();
        solver.foot_placement(
            BoneId(0), BoneId(1), BoneId(2),
            &skeleton, &mut pose,
            &|_x, _z| 0.0_f32,
        );
        assert_eq!(pose.len(), skeleton.len());
    }
}
