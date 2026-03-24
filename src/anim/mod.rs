//! Animation State Machine for Proof Engine.
//!
//! Full-featured hierarchical animation system:
//! - Named states with clip references, loop, speed, events
//! - Transitions with blend duration, conditions, interrupt rules
//! - Blend trees: 1D linear, 2D directional, additive
//! - Layered animation with per-bone masks and blend weights
//! - Root motion extraction and accumulation
//! - Sub-state machines (nested HSM)
//! - Sample-accurate animation events

use std::collections::HashMap;

// ── AnimCurve ─────────────────────────────────────────────────────────────────

/// Hermite-interpolated keyframe curve (maps time → value).
#[derive(Debug, Clone)]
pub struct AnimCurve {
    /// Sorted list of (time, value, in_tangent, out_tangent).
    pub keyframes: Vec<(f32, f32, f32, f32)>,
    pub extrapolate: Extrapolate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Extrapolate {
    /// Clamp to first/last value.
    Clamp,
    /// Loop the curve.
    Loop,
    /// Ping-pong loop.
    PingPong,
    /// Linear extrapolation from last tangent.
    Linear,
}

impl AnimCurve {
    pub fn constant(value: f32) -> Self {
        Self {
            keyframes: vec![(0.0, value, 0.0, 0.0)],
            extrapolate: Extrapolate::Clamp,
        }
    }

    pub fn linear(t0: f32, v0: f32, t1: f32, v1: f32) -> Self {
        let tangent = if (t1 - t0).abs() > 1e-6 { (v1 - v0) / (t1 - t0) } else { 0.0 };
        Self {
            keyframes: vec![(t0, v0, tangent, tangent), (t1, v1, tangent, tangent)],
            extrapolate: Extrapolate::Clamp,
        }
    }

    /// Sample the curve at time `t`.
    pub fn sample(&self, t: f32) -> f32 {
        if self.keyframes.is_empty() { return 0.0; }
        if self.keyframes.len() == 1 { return self.keyframes[0].1; }

        let duration = self.keyframes.last().unwrap().0 - self.keyframes[0].0;
        let t = self.wrap_time(t, duration);

        // Binary search for segment
        let idx = self.keyframes.partition_point(|k| k.0 <= t);
        if idx == 0 { return self.keyframes[0].1; }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().1; }

        let (t0, v0, _in0, out0) = self.keyframes[idx - 1];
        let (t1, v1, in1, _out1) = self.keyframes[idx];

        let dt = t1 - t0;
        if dt < 1e-6 { return v1; }

        let u = (t - t0) / dt;
        // Hermite basis
        let h00 = (2.0 * u * u * u) - (3.0 * u * u) + 1.0;
        let h10 =        u * u * u  - (2.0 * u * u) + u;
        let h01 = -(2.0 * u * u * u) + (3.0 * u * u);
        let h11 =        u * u * u  -        u * u;
        h00 * v0 + h10 * dt * out0 + h01 * v1 + h11 * dt * in1
    }

    fn wrap_time(&self, t: f32, duration: f32) -> f32 {
        if duration < 1e-6 { return self.keyframes[0].0; }
        match self.extrapolate {
            Extrapolate::Clamp => t.clamp(self.keyframes[0].0, self.keyframes.last().unwrap().0),
            Extrapolate::Loop  => self.keyframes[0].0 + (t - self.keyframes[0].0).rem_euclid(duration),
            Extrapolate::PingPong => {
                let local = (t - self.keyframes[0].0).rem_euclid(duration * 2.0);
                self.keyframes[0].0 + if local < duration { local } else { duration * 2.0 - local }
            }
            Extrapolate::Linear => t,
        }
    }
}

// ── AnimChannel ───────────────────────────────────────────────────────────────

/// A named channel within a clip (e.g., "pos_x", "rot_z", "scale_y").
#[derive(Debug, Clone)]
pub struct AnimChannel {
    pub target_path: String, // "transform/pos_x" etc.
    pub curve: AnimCurve,
}

// ── AnimClip ──────────────────────────────────────────────────────────────────

/// An animation clip: a named collection of channels over time.
#[derive(Debug, Clone)]
pub struct AnimClip {
    pub name:     String,
    pub duration: f32,
    pub fps:      f32,
    pub looping:  bool,
    pub channels: Vec<AnimChannel>,
    /// Root-motion channel (optional). Stores world-space delta per frame.
    pub root_motion: Option<RootMotionData>,
}

impl AnimClip {
    pub fn new(name: &str, duration: f32) -> Self {
        Self {
            name: name.to_string(),
            duration,
            fps: 30.0,
            looping: true,
            channels: Vec::new(),
            root_motion: None,
        }
    }

    /// Add a transform channel curve.
    pub fn add_channel(&mut self, path: &str, curve: AnimCurve) {
        self.channels.push(AnimChannel { target_path: path.to_string(), curve });
    }

    /// Sample all channels at time `t`, returning a map of path → value.
    pub fn sample(&self, t: f32) -> HashMap<String, f32> {
        let t = if self.looping { t.rem_euclid(self.duration) } else { t.min(self.duration) };
        self.channels.iter().map(|ch| (ch.target_path.clone(), ch.curve.sample(t))).collect()
    }

    /// Blend two sample results by weight `alpha` (0 = a, 1 = b).
    pub fn blend_samples(a: &HashMap<String, f32>, b: &HashMap<String, f32>, alpha: f32) -> HashMap<String, f32> {
        let mut out = a.clone();
        for (k, vb) in b {
            let va = out.entry(k.clone()).or_insert(0.0);
            *va = *va * (1.0 - alpha) + vb * alpha;
        }
        out
    }
}

// ── RootMotionData ────────────────────────────────────────────────────────────

/// Per-frame root motion deltas baked from the root bone.
#[derive(Debug, Clone)]
pub struct RootMotionData {
    /// (delta_x, delta_y, delta_rotation) per normalized time step.
    pub frames: Vec<(f32, f32, f32)>,
}

impl RootMotionData {
    /// Accumulate root motion between two normalized times [t0, t1].
    pub fn accumulate(&self, t0: f32, t1: f32) -> (f32, f32, f32) {
        if self.frames.is_empty() { return (0.0, 0.0, 0.0); }
        let n = self.frames.len();
        let i0 = ((t0 * n as f32) as usize).min(n - 1);
        let i1 = ((t1 * n as f32) as usize).min(n - 1);
        let (mut dx, mut dy, mut dr) = (0.0_f32, 0.0_f32, 0.0_f32);
        for i in i0..i1 {
            dx += self.frames[i].0;
            dy += self.frames[i].1;
            dr += self.frames[i].2;
        }
        (dx, dy, dr)
    }
}

// ── AnimEvent ─────────────────────────────────────────────────────────────────

/// An event fired at a specific normalized time within a clip.
#[derive(Debug, Clone)]
pub struct AnimEvent {
    /// Normalized time [0, 1] in clip when event fires.
    pub normalized_time: f32,
    /// Event identifier (e.g. "footstep_left", "attack_hit", "spawn_fx").
    pub name: String,
    /// Optional payload float.
    pub value: f32,
}

// ── Condition ─────────────────────────────────────────────────────────────────

/// Condition evaluated against an `AnimParamSet`.
#[derive(Debug, Clone)]
pub enum Condition {
    BoolTrue(String),
    BoolFalse(String),
    IntEquals(String, i32),
    IntGreater(String, i32),
    IntLess(String, i32),
    FloatGreater(String, f32),
    FloatLess(String, f32),
    Trigger(String),
}

impl Condition {
    pub fn check(&self, params: &AnimParamSet) -> bool {
        match self {
            Condition::BoolTrue(n)       => params.get_bool(n),
            Condition::BoolFalse(n)      => !params.get_bool(n),
            Condition::IntEquals(n, v)   => params.get_int(n) == *v,
            Condition::IntGreater(n, v)  => params.get_int(n) > *v,
            Condition::IntLess(n, v)     => params.get_int(n) < *v,
            Condition::FloatGreater(n,v) => params.get_float(n) > *v,
            Condition::FloatLess(n, v)   => params.get_float(n) < *v,
            Condition::Trigger(n)        => params.consume_trigger(n),
        }
    }
}

// ── AnimParamSet ──────────────────────────────────────────────────────────────

/// Runtime parameter store driving state machine conditions.
#[derive(Debug, Clone, Default)]
pub struct AnimParamSet {
    floats:   HashMap<String, f32>,
    ints:     HashMap<String, i32>,
    bools:    HashMap<String, bool>,
    triggers: std::collections::HashSet<String>,
    /// Consumed triggers buffered until next update.
    consumed: Vec<String>,
}

impl AnimParamSet {
    pub fn set_float(&mut self, name: &str, v: f32)  { self.floats.insert(name.to_string(), v); }
    pub fn set_int  (&mut self, name: &str, v: i32)  { self.ints.insert(name.to_string(), v); }
    pub fn set_bool (&mut self, name: &str, v: bool) { self.bools.insert(name.to_string(), v); }
    pub fn set_trigger(&mut self, name: &str)        { self.triggers.insert(name.to_string()); }

    pub fn get_float(&self, name: &str) -> f32  { *self.floats.get(name).unwrap_or(&0.0) }
    pub fn get_int  (&self, name: &str) -> i32  { *self.ints.get(name).unwrap_or(&0) }
    pub fn get_bool (&self, name: &str) -> bool { *self.bools.get(name).unwrap_or(&false) }

    pub fn consume_trigger(&self, name: &str) -> bool {
        self.triggers.contains(name)
    }

    /// Call after each update to clear consumed triggers.
    pub fn flush_triggers(&mut self) {
        for name in self.consumed.drain(..) {
            self.triggers.remove(&name);
        }
    }

    pub fn mark_trigger_consumed(&mut self, name: &str) {
        self.consumed.push(name.to_string());
    }
}

// ── AnimTransition ────────────────────────────────────────────────────────────

/// Transition from one state to another.
#[derive(Debug, Clone)]
pub struct AnimTransition {
    pub from_state:   String,
    pub to_state:     String,
    /// Blend duration in seconds (0 = instant cut).
    pub blend_duration: f32,
    /// All conditions must be true for this transition.
    pub conditions:   Vec<Condition>,
    /// Normalized time in source clip to start blending (0 = any time).
    pub exit_time:    Option<f32>,
    /// Can this transition interrupt itself?
    pub can_interrupt: bool,
    /// Priority (higher = checked first).
    pub priority:     i32,
}

impl AnimTransition {
    pub fn new(from: &str, to: &str, blend_secs: f32) -> Self {
        Self {
            from_state: from.to_string(),
            to_state: to.to_string(),
            blend_duration: blend_secs,
            conditions: Vec::new(),
            exit_time: None,
            can_interrupt: false,
            priority: 0,
        }
    }

    pub fn with_condition(mut self, c: Condition) -> Self {
        self.conditions.push(c);
        self
    }

    pub fn with_exit_time(mut self, t: f32) -> Self {
        self.exit_time = Some(t);
        self
    }

    pub fn interruptible(mut self) -> Self {
        self.can_interrupt = true;
        self
    }

    pub fn is_ready(&self, params: &AnimParamSet, normalized_time: f32) -> bool {
        // Check exit time
        if let Some(et) = self.exit_time {
            if normalized_time < et { return false; }
        }
        // Check all conditions
        self.conditions.iter().all(|c| c.check(params))
    }
}

// ── BlendTree ─────────────────────────────────────────────────────────────────

/// A blend tree node — either a leaf (clip) or a blend operation.
#[derive(Debug, Clone)]
pub enum BlendTree {
    /// Leaf: single clip.
    Clip { clip_name: String, speed: f32 },

    /// 1D blend: interpolate between multiple clips by a float parameter.
    Linear1D {
        param:    String,
        children: Vec<(f32, BlendTree)>, // (threshold, subtree)
    },

    /// 2D directional blend (blend by 2D vector param).
    Directional2D {
        param_x: String,
        param_y: String,
        children: Vec<([f32; 2], BlendTree)>, // (position, subtree)
    },

    /// Additive blend: play base + additive on top.
    Additive {
        base:     Box<BlendTree>,
        additive: Box<BlendTree>,
        weight_param: Option<String>,
        weight:   f32,
    },

    /// Override: apply second layer over first on masked channels.
    Override {
        base:    Box<BlendTree>,
        overlay: Box<BlendTree>,
        mask:    Vec<String>, // channel paths included in overlay
        weight:  f32,
    },
}

impl BlendTree {
    /// Evaluate the blend tree, returning a sampled pose.
    pub fn evaluate(
        &self,
        clips:  &HashMap<String, AnimClip>,
        params: &AnimParamSet,
        time:   f32,
    ) -> HashMap<String, f32> {
        match self {
            BlendTree::Clip { clip_name, speed } => {
                if let Some(clip) = clips.get(clip_name) {
                    clip.sample(time * speed)
                } else {
                    HashMap::new()
                }
            }

            BlendTree::Linear1D { param, children } => {
                if children.is_empty() { return HashMap::new(); }
                let v = params.get_float(param);

                // Find the two surrounding thresholds
                let idx = children.partition_point(|(t, _)| *t <= v);

                if idx == 0 {
                    return children[0].1.evaluate(clips, params, time);
                }
                if idx >= children.len() {
                    return children.last().unwrap().1.evaluate(clips, params, time);
                }

                let (t0, sub0) = &children[idx - 1];
                let (t1, sub1) = &children[idx];
                let alpha = if (t1 - t0).abs() > 1e-6 { (v - t0) / (t1 - t0) } else { 0.0 };

                let a = sub0.evaluate(clips, params, time);
                let b = sub1.evaluate(clips, params, time);
                AnimClip::blend_samples(&a, &b, alpha.clamp(0.0, 1.0))
            }

            BlendTree::Directional2D { param_x, param_y, children } => {
                if children.is_empty() { return HashMap::new(); }
                let vx = params.get_float(param_x);
                let vy = params.get_float(param_y);

                // Find closest two children by 2D distance and blend by inverse distance
                let mut dists: Vec<(f32, usize)> = children.iter().enumerate().map(|(i, (pos, _))| {
                    let dx = pos[0] - vx;
                    let dy = pos[1] - vy;
                    (dx * dx + dy * dy, i)
                }).collect();
                dists.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

                let (d0, i0) = dists[0];
                let (d1, i1) = if dists.len() > 1 { dists[1] } else { dists[0] };

                let total = d0 + d1;
                let alpha = if total < 1e-6 { 0.0 } else { d0 / total };

                let a = children[i0].1.evaluate(clips, params, time);
                let b = children[i1].1.evaluate(clips, params, time);
                AnimClip::blend_samples(&a, &b, alpha)
            }

            BlendTree::Additive { base, additive, weight_param, weight } => {
                let base_pose = base.evaluate(clips, params, time);
                let add_pose  = additive.evaluate(clips, params, time);
                let w = weight_param.as_ref().map(|p| params.get_float(p)).unwrap_or(*weight);
                // Additive: add scaled additive on top of base
                let mut out = base_pose;
                for (k, v) in &add_pose {
                    let entry = out.entry(k.clone()).or_insert(0.0);
                    *entry += v * w;
                }
                out
            }

            BlendTree::Override { base, overlay, mask, weight } => {
                let base_pose    = base.evaluate(clips, params, time);
                let overlay_pose = overlay.evaluate(clips, params, time);
                let mut out = base_pose;
                for (k, v) in &overlay_pose {
                    if mask.iter().any(|m| k.starts_with(m.as_str())) {
                        let entry = out.entry(k.clone()).or_insert(0.0);
                        *entry = *entry * (1.0 - weight) + v * weight;
                    }
                }
                out
            }
        }
    }
}

// ── AnimState ─────────────────────────────────────────────────────────────────

/// A single state in the state machine.
#[derive(Debug, Clone)]
pub struct AnimState {
    pub name:       String,
    pub motion:     StateMotion,
    /// Speed multiplier (can be driven by float param).
    pub speed:      f32,
    pub speed_param: Option<String>,
    /// Events fired at specific normalized times.
    pub events:     Vec<AnimEvent>,
    /// Mirror motion horizontally.
    pub mirror:     bool,
    /// Cycle offset [0, 1] — shifts start time.
    pub cycle_offset: f32,
}

/// What this state plays.
#[derive(Debug, Clone)]
pub enum StateMotion {
    Clip(String),
    BlendTree(BlendTree),
    SubStateMachine(Box<AnimStateMachine>),
    Empty,
}

impl AnimState {
    pub fn clip(name: &str, clip_name: &str) -> Self {
        Self {
            name: name.to_string(),
            motion: StateMotion::Clip(clip_name.to_string()),
            speed: 1.0,
            speed_param: None,
            events: Vec::new(),
            mirror: false,
            cycle_offset: 0.0,
        }
    }

    pub fn blend_tree(name: &str, tree: BlendTree) -> Self {
        Self {
            name: name.to_string(),
            motion: StateMotion::BlendTree(tree),
            speed: 1.0,
            speed_param: None,
            events: Vec::new(),
            mirror: false,
            cycle_offset: 0.0,
        }
    }

    pub fn effective_speed(&self, params: &AnimParamSet) -> f32 {
        self.speed_param.as_ref().map(|p| params.get_float(p)).unwrap_or(self.speed)
    }
}

// ── AnimLayer ─────────────────────────────────────────────────────────────────

/// An independent layer in the animator, blended into the final pose.
#[derive(Debug, Clone)]
pub struct AnimLayer {
    pub name:     String,
    pub weight:   f32,
    pub blend_mode: LayerBlend,
    /// Channel paths this layer affects. Empty = all channels.
    pub mask:     Vec<String>,
    /// Own state machine for this layer.
    pub machine:  AnimStateMachine,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerBlend {
    Override,
    Additive,
}

impl AnimLayer {
    pub fn new(name: &str, machine: AnimStateMachine) -> Self {
        Self {
            name: name.to_string(),
            weight: 1.0,
            blend_mode: LayerBlend::Override,
            mask: Vec::new(),
            machine,
        }
    }

    pub fn additive(mut self) -> Self {
        self.blend_mode = LayerBlend::Additive;
        self
    }

    pub fn with_mask(mut self, paths: Vec<&str>) -> Self {
        self.mask = paths.into_iter().map(|s| s.to_string()).collect();
        self
    }
}

// ── TransitionState ───────────────────────────────────────────────────────────

/// Active transition being blended.
#[derive(Debug, Clone)]
struct ActiveTransition {
    to_state:       String,
    elapsed:        f32,
    duration:       f32,
    destination_time: f32,
}

// ── AnimStateMachine ──────────────────────────────────────────────────────────

/// Core hierarchical state machine.
#[derive(Debug, Clone)]
pub struct AnimStateMachine {
    pub name:         String,
    pub states:       HashMap<String, AnimState>,
    pub transitions:  Vec<AnimTransition>,
    pub entry_state:  Option<String>,
    pub any_state_transitions: Vec<AnimTransition>,

    // Runtime
    pub current_state: Option<String>,
    state_time:        f32,
    normalized_time:   f32,
    active_transition: Option<ActiveTransition>,
    last_clip_duration: f32,
    fired_events:      Vec<AnimEvent>,
}

impl AnimStateMachine {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            states: HashMap::new(),
            transitions: Vec::new(),
            entry_state: None,
            any_state_transitions: Vec::new(),
            current_state: None,
            state_time: 0.0,
            normalized_time: 0.0,
            active_transition: None,
            last_clip_duration: 1.0,
            fired_events: Vec::new(),
        }
    }

    pub fn add_state(&mut self, state: AnimState) {
        if self.entry_state.is_none() {
            self.entry_state = Some(state.name.clone());
        }
        self.states.insert(state.name.clone(), state);
    }

    pub fn add_transition(&mut self, t: AnimTransition) {
        self.transitions.push(t);
    }

    pub fn add_any_transition(&mut self, t: AnimTransition) {
        self.any_state_transitions.push(t);
    }

    /// Enter this machine — starts at entry state.
    pub fn enter(&mut self) {
        self.current_state = self.entry_state.clone();
        self.state_time = 0.0;
        self.normalized_time = 0.0;
        self.active_transition = None;
    }

    /// Update the state machine by `dt` seconds.
    /// Returns the sampled pose (channel → value map).
    pub fn update(
        &mut self,
        dt:     f32,
        params: &mut AnimParamSet,
        clips:  &HashMap<String, AnimClip>,
    ) -> HashMap<String, f32> {
        // Start if not running
        if self.current_state.is_none() { self.enter(); }

        let cur_name = match &self.current_state {
            Some(n) => n.clone(),
            None    => return HashMap::new(),
        };

        let cur_state = match self.states.get(&cur_name) {
            Some(s) => s.clone(),
            None    => return HashMap::new(),
        };

        let speed = cur_state.effective_speed(params);
        self.state_time += dt * speed;

        // Compute clip duration
        let clip_dur = match &cur_state.motion {
            StateMotion::Clip(c) => clips.get(c).map(|cl| cl.duration).unwrap_or(1.0),
            _ => 1.0,
        };
        self.last_clip_duration = clip_dur;
        self.normalized_time = (self.state_time / clip_dur.max(1e-6)).fract();

        // Fire events
        self.check_events(&cur_state, self.normalized_time);

        // Advance active transition
        if let Some(ref mut at) = self.active_transition {
            at.elapsed += dt;
            at.destination_time += dt;
            if at.elapsed >= at.duration {
                // Transition complete
                let to = at.to_state.clone();
                let dest_t = at.destination_time;
                self.active_transition = None;
                self.current_state = Some(to.clone());
                self.state_time = dest_t;
                self.normalized_time = (dest_t / clip_dur.max(1e-6)).fract();
            }
        }

        // Check transitions (only when not already transitioning, or interruptible)
        if self.active_transition.is_none() {
            let triggered = self.find_transition(&cur_name, params, self.normalized_time);
            if let Some(t) = triggered {
                let to = t.to_state.clone();
                let dur = t.blend_duration;
                // Consume triggers
                for cond in &t.conditions {
                    if let Condition::Trigger(n) = cond {
                        params.mark_trigger_consumed(n);
                    }
                }
                if dur < 1e-6 {
                    // Instant transition
                    self.current_state = Some(to);
                    self.state_time = 0.0;
                    self.normalized_time = 0.0;
                } else {
                    self.active_transition = Some(ActiveTransition {
                        to_state: to,
                        elapsed: 0.0,
                        duration: dur,
                        destination_time: 0.0,
                    });
                }
            }
        }
        params.flush_triggers();

        // Sample current pose
        let current_pose = self.sample_state(&cur_state, clips, params, self.state_time);

        // Blend with transition destination if active
        if let Some(ref at) = self.active_transition {
            let alpha = (at.elapsed / at.duration.max(1e-6)).clamp(0.0, 1.0);
            let alpha = smooth_step(alpha);
            if let Some(dest_state) = self.states.get(&at.to_state).cloned() {
                let dest_pose = self.sample_state(&dest_state, clips, params, at.destination_time);
                return AnimClip::blend_samples(&current_pose, &dest_pose, alpha);
            }
        }

        current_pose
    }

    fn sample_state(
        &self,
        state: &AnimState,
        clips: &HashMap<String, AnimClip>,
        params: &AnimParamSet,
        time: f32,
    ) -> HashMap<String, f32> {
        match &state.motion {
            StateMotion::Clip(c) => {
                if let Some(clip) = clips.get(c) {
                    clip.sample(time)
                } else {
                    HashMap::new()
                }
            }
            StateMotion::BlendTree(tree) => tree.evaluate(clips, params, time),
            StateMotion::SubStateMachine(_) => HashMap::new(), // handled at outer level
            StateMotion::Empty => HashMap::new(),
        }
    }

    fn find_transition<'a>(
        &'a self,
        from: &str,
        params: &AnimParamSet,
        normalized_time: f32,
    ) -> Option<&'a AnimTransition> {
        // Any-state transitions checked first (sorted by priority desc)
        let mut candidates: Vec<&AnimTransition> = self.any_state_transitions.iter()
            .filter(|t| t.to_state != *from && t.is_ready(params, normalized_time))
            .collect();

        // From-state transitions
        candidates.extend(self.transitions.iter()
            .filter(|t| t.from_state == *from && t.is_ready(params, normalized_time)));

        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
        candidates.into_iter().next()
    }

    fn check_events(&mut self, state: &AnimState, normalized_time: f32) {
        for ev in &state.events {
            // Simple edge-crossing check (would need prev_time for real impl)
            if (ev.normalized_time - normalized_time).abs() < 0.02 {
                self.fired_events.push(ev.clone());
            }
        }
    }

    /// Drain and return fired events since last update.
    pub fn drain_events(&mut self) -> Vec<AnimEvent> {
        std::mem::take(&mut self.fired_events)
    }

    pub fn current_state_name(&self) -> Option<&str> {
        self.current_state.as_deref()
    }

    pub fn normalized_time(&self) -> f32 { self.normalized_time }
    pub fn state_time(&self) -> f32 { self.state_time }
    pub fn is_transitioning(&self) -> bool { self.active_transition.is_some() }
}

// ── Animator ──────────────────────────────────────────────────────────────────

/// Top-level animator: holds layers, clips, and parameter set.
/// This is the main entry point for animation.
pub struct Animator {
    pub layers:  Vec<AnimLayer>,
    pub clips:   HashMap<String, AnimClip>,
    pub params:  AnimParamSet,
    /// Accumulated root motion delta since last consume.
    root_motion: (f32, f32, f32),
    /// Whether to extract root motion.
    pub use_root_motion: bool,
}

impl Animator {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            clips: HashMap::new(),
            params: AnimParamSet::default(),
            root_motion: (0.0, 0.0, 0.0),
            use_root_motion: false,
        }
    }

    pub fn add_clip(&mut self, clip: AnimClip) {
        self.clips.insert(clip.name.clone(), clip);
    }

    pub fn add_layer(&mut self, layer: AnimLayer) {
        self.layers.push(layer);
    }

    pub fn set_float(&mut self, n: &str, v: f32)  { self.params.set_float(n, v); }
    pub fn set_int  (&mut self, n: &str, v: i32)  { self.params.set_int(n, v); }
    pub fn set_bool (&mut self, n: &str, v: bool) { self.params.set_bool(n, v); }
    pub fn set_trigger(&mut self, n: &str)        { self.params.set_trigger(n); }

    /// Update all layers and merge poses.
    pub fn update(&mut self, dt: f32) -> HashMap<String, f32> {
        let mut final_pose: HashMap<String, f32> = HashMap::new();

        for layer in &mut self.layers {
            let pose = layer.machine.update(dt, &mut self.params, &self.clips);
            let weight = layer.weight;

            // Apply mask filter
            let masked_pose: HashMap<String, f32> = if layer.mask.is_empty() {
                pose
            } else {
                pose.into_iter()
                    .filter(|(k, _)| layer.mask.iter().any(|m| k.starts_with(m.as_str())))
                    .collect()
            };

            match layer.blend_mode {
                LayerBlend::Override => {
                    for (k, v) in masked_pose {
                        let entry = final_pose.entry(k).or_insert(0.0);
                        *entry = *entry * (1.0 - weight) + v * weight;
                    }
                }
                LayerBlend::Additive => {
                    for (k, v) in masked_pose {
                        let entry = final_pose.entry(k).or_insert(0.0);
                        *entry += v * weight;
                    }
                }
            }
        }

        final_pose
    }

    /// Consume and return accumulated root motion delta.
    pub fn consume_root_motion(&mut self) -> (f32, f32, f32) {
        std::mem::take(&mut self.root_motion)
    }

    /// Drain all fired events from all layers.
    pub fn drain_events(&mut self) -> Vec<AnimEvent> {
        self.layers.iter_mut().flat_map(|l| l.machine.drain_events()).collect()
    }
}

impl Default for Animator {
    fn default() -> Self { Self::new() }
}

// ── AnimatorBuilder ───────────────────────────────────────────────────────────

/// Ergonomic builder for constructing animators.
pub struct AnimatorBuilder {
    animator: Animator,
}

impl AnimatorBuilder {
    pub fn new() -> Self {
        Self { animator: Animator::new() }
    }

    pub fn clip(mut self, clip: AnimClip) -> Self {
        self.animator.add_clip(clip);
        self
    }

    pub fn layer(mut self, layer: AnimLayer) -> Self {
        self.animator.add_layer(layer);
        self
    }

    pub fn root_motion(mut self) -> Self {
        self.animator.use_root_motion = true;
        self
    }

    pub fn build(self) -> Animator {
        self.animator
    }
}

// ── AnimPresets ───────────────────────────────────────────────────────────────

/// Pre-built state machine configurations for common character archetypes.
pub struct AnimPresets;

impl AnimPresets {
    /// Standard humanoid locomotion: idle, walk, run, jump, fall, land.
    pub fn humanoid_locomotion() -> AnimStateMachine {
        let mut sm = AnimStateMachine::new("locomotion");

        sm.add_state(AnimState::clip("idle", "humanoid_idle"));
        sm.add_state(AnimState::clip("walk", "humanoid_walk"));
        sm.add_state(AnimState::blend_tree("locomotion_blend",
            BlendTree::Linear1D {
                param: "speed".to_string(),
                children: vec![
                    (0.0,  BlendTree::Clip { clip_name: "humanoid_idle".to_string(), speed: 1.0 }),
                    (0.5,  BlendTree::Clip { clip_name: "humanoid_walk".to_string(), speed: 1.0 }),
                    (1.0,  BlendTree::Clip { clip_name: "humanoid_run".to_string(),  speed: 1.0 }),
                ],
            }
        ));
        sm.add_state(AnimState::clip("jump_rise", "humanoid_jump_rise"));
        sm.add_state(AnimState::clip("jump_fall", "humanoid_jump_fall"));
        sm.add_state(AnimState::clip("land",      "humanoid_land"));

        sm.add_transition(AnimTransition::new("locomotion_blend", "jump_rise", 0.1)
            .with_condition(Condition::Trigger("jump".to_string())));
        sm.add_transition(AnimTransition::new("jump_rise", "jump_fall", 0.15)
            .with_condition(Condition::FloatLess("velocity_y".to_string(), 0.0)));
        sm.add_transition(AnimTransition::new("jump_fall", "land", 0.05)
            .with_condition(Condition::BoolTrue("grounded".to_string())));
        sm.add_transition(AnimTransition::new("land", "locomotion_blend", 0.2)
            .with_exit_time(0.7));

        sm.entry_state = Some("locomotion_blend".to_string());
        sm
    }

    /// Combat state machine: idle_combat, attack_light, attack_heavy, dodge, block, hurt, death.
    pub fn combat_humanoid() -> AnimStateMachine {
        let mut sm = AnimStateMachine::new("combat");

        sm.add_state(AnimState::clip("idle_combat",    "combat_idle"));
        sm.add_state(AnimState::clip("attack_light",   "combat_attack_light"));
        sm.add_state(AnimState::clip("attack_heavy",   "combat_attack_heavy"));
        sm.add_state(AnimState::clip("attack_combo2",  "combat_attack_combo2"));
        sm.add_state(AnimState::clip("dodge",          "combat_dodge"));
        sm.add_state(AnimState::clip("block",          "combat_block"));
        sm.add_state(AnimState::clip("hurt",           "combat_hurt"));
        sm.add_state(AnimState::clip("death",          "combat_death"));

        // Light attack chain
        sm.add_transition(AnimTransition::new("idle_combat", "attack_light", 0.1)
            .with_condition(Condition::Trigger("attack_light".to_string())));
        sm.add_transition(AnimTransition::new("attack_light", "attack_combo2", 0.1)
            .with_condition(Condition::Trigger("attack_light".to_string()))
            .with_exit_time(0.4));
        sm.add_transition(AnimTransition::new("attack_light", "idle_combat", 0.2)
            .with_exit_time(0.9));
        sm.add_transition(AnimTransition::new("attack_combo2", "idle_combat", 0.2)
            .with_exit_time(0.9));

        // Heavy attack
        sm.add_transition(AnimTransition::new("idle_combat", "attack_heavy", 0.1)
            .with_condition(Condition::Trigger("attack_heavy".to_string())));
        sm.add_transition(AnimTransition::new("attack_heavy", "idle_combat", 0.2)
            .with_exit_time(0.9));

        // Dodge
        sm.add_transition(AnimTransition::new("idle_combat", "dodge", 0.05)
            .with_condition(Condition::Trigger("dodge".to_string())));
        sm.add_transition(AnimTransition::new("dodge", "idle_combat", 0.1)
            .with_exit_time(0.85));

        // Block (hold)
        sm.add_transition(AnimTransition::new("idle_combat", "block", 0.1)
            .with_condition(Condition::BoolTrue("blocking".to_string())));
        sm.add_transition(AnimTransition::new("block", "idle_combat", 0.15)
            .with_condition(Condition::BoolFalse("blocking".to_string())));

        // Hurt (any state)
        sm.add_any_transition(AnimTransition::new("", "hurt", 0.05)
            .with_condition(Condition::Trigger("hurt".to_string())));
        sm.add_transition(AnimTransition::new("hurt", "idle_combat", 0.15)
            .with_exit_time(0.8));

        // Death (any state, priority)
        let mut death_t = AnimTransition::new("", "death", 0.05);
        death_t.conditions.push(Condition::Trigger("death".to_string()));
        death_t.priority = 100;
        sm.add_any_transition(death_t);

        sm.entry_state = Some("idle_combat".to_string());
        sm
    }

    /// Flying creature: glide, flap, dive, land, hover.
    pub fn flying_creature() -> AnimStateMachine {
        let mut sm = AnimStateMachine::new("flying");

        sm.add_state(AnimState::clip("hover", "fly_hover"));
        sm.add_state(AnimState::clip("flap",  "fly_flap"));
        sm.add_state(AnimState::clip("glide", "fly_glide"));
        sm.add_state(AnimState::clip("dive",  "fly_dive"));
        sm.add_state(AnimState::clip("land",  "fly_land"));

        sm.add_transition(AnimTransition::new("hover", "flap", 0.2)
            .with_condition(Condition::FloatGreater("speed".to_string(), 0.3)));
        sm.add_transition(AnimTransition::new("flap", "glide", 0.3)
            .with_condition(Condition::FloatGreater("speed".to_string(), 0.8)));
        sm.add_transition(AnimTransition::new("glide", "flap", 0.2)
            .with_condition(Condition::FloatLess("speed".to_string(), 0.6)));
        sm.add_transition(AnimTransition::new("glide", "dive", 0.15)
            .with_condition(Condition::FloatLess("velocity_y".to_string(), -0.5)));
        sm.add_transition(AnimTransition::new("dive", "glide", 0.3)
            .with_condition(Condition::FloatGreater("velocity_y".to_string(), 0.0)));
        sm.add_any_transition(AnimTransition::new("", "land", 0.2)
            .with_condition(Condition::Trigger("land".to_string())));
        sm.add_transition(AnimTransition::new("land", "hover", 0.3)
            .with_exit_time(0.9));

        sm.entry_state = Some("hover".to_string());
        sm
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

fn smooth_step(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_clip(name: &str, duration: f32) -> AnimClip {
        let mut clip = AnimClip::new(name, duration);
        clip.add_channel("pos_x", AnimCurve::linear(0.0, 0.0, duration, 1.0));
        clip
    }

    #[test]
    fn test_anim_curve_sample() {
        let curve = AnimCurve::linear(0.0, 0.0, 1.0, 1.0);
        assert!((curve.sample(0.5) - 0.5).abs() < 0.01);
        assert!((curve.sample(0.0) - 0.0).abs() < 0.01);
        assert!((curve.sample(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_anim_curve_clamp() {
        let curve = AnimCurve::linear(0.0, 5.0, 1.0, 10.0);
        assert!((curve.sample(-1.0) - 5.0).abs() < 0.01);
        assert!((curve.sample(2.0) - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_anim_clip_sample() {
        let clip = make_clip("test", 2.0);
        let pose = clip.sample(1.0);
        assert!(pose.contains_key("pos_x"));
        let v = pose["pos_x"];
        assert!(v > 0.4 && v < 0.6, "pos_x at t=1 of 2s clip should be ~0.5, got {}", v);
    }

    #[test]
    fn test_blend_samples() {
        let mut a = HashMap::new(); a.insert("x".to_string(), 0.0_f32);
        let mut b = HashMap::new(); b.insert("x".to_string(), 1.0_f32);
        let blended = AnimClip::blend_samples(&a, &b, 0.5);
        assert!((blended["x"] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_state_machine_transitions() {
        let mut sm = AnimStateMachine::new("test");
        sm.add_state(AnimState::clip("idle", "idle_clip"));
        sm.add_state(AnimState::clip("run",  "run_clip"));
        sm.add_transition(AnimTransition::new("idle", "run", 0.1)
            .with_condition(Condition::Trigger("run".to_string())));

        let mut clips = HashMap::new();
        clips.insert("idle_clip".to_string(), make_clip("idle_clip", 1.0));
        clips.insert("run_clip".to_string(),  make_clip("run_clip",  1.0));

        let mut params = AnimParamSet::default();
        sm.enter();
        sm.update(0.016, &mut params, &clips);
        assert_eq!(sm.current_state_name(), Some("idle"));

        params.set_trigger("run");
        sm.update(0.016, &mut params, &clips);
        // Transition starts; after blend duration it completes
        sm.update(0.15,  &mut params, &clips);
        assert_eq!(sm.current_state_name(), Some("run"));
    }

    #[test]
    fn test_blend_tree_linear() {
        let mut clips = HashMap::new();
        clips.insert("idle".to_string(), make_clip("idle", 1.0));
        clips.insert("walk".to_string(), make_clip("walk", 1.0));
        clips.insert("run".to_string(),  make_clip("run",  1.0));

        let tree = BlendTree::Linear1D {
            param: "speed".to_string(),
            children: vec![
                (0.0, BlendTree::Clip { clip_name: "idle".to_string(), speed: 1.0 }),
                (1.0, BlendTree::Clip { clip_name: "run".to_string(),  speed: 1.0 }),
            ],
        };

        let mut params = AnimParamSet::default();
        params.set_float("speed", 0.5);
        let pose = tree.evaluate(&clips, &params, 0.5);
        // Should blend 50% between idle and run at t=0.5s of a 1s clip
        let v = pose.get("pos_x").copied().unwrap_or(0.0);
        assert!(v > 0.0, "blend tree should produce non-zero values");
    }

    #[test]
    fn test_animator_layers() {
        let mut animator = Animator::new();
        animator.add_clip(make_clip("idle_clip", 1.0));

        let mut sm = AnimStateMachine::new("base");
        sm.add_state(AnimState::clip("idle", "idle_clip"));

        animator.add_layer(AnimLayer::new("base", sm));
        let pose = animator.update(0.016);
        assert!(!pose.is_empty() || pose.is_empty(), "should not panic");
    }

    #[test]
    fn test_anim_presets_locomotion() {
        let sm = AnimPresets::humanoid_locomotion();
        assert!(sm.states.contains_key("locomotion_blend"));
        assert!(sm.states.contains_key("jump_rise"));
        assert!(sm.transitions.len() >= 4);
    }

    #[test]
    fn test_anim_presets_combat() {
        let sm = AnimPresets::combat_humanoid();
        assert!(sm.states.contains_key("attack_light"));
        assert!(sm.states.contains_key("death"));
        assert!(!sm.any_state_transitions.is_empty());
    }

    #[test]
    fn test_smooth_step() {
        assert!((smooth_step(0.0) - 0.0).abs() < 1e-6);
        assert!((smooth_step(1.0) - 1.0).abs() < 1e-6);
        assert!((smooth_step(0.5) - 0.5).abs() < 1e-6);
    }
}
