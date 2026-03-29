
//! Animation state machine editor — states, transitions, blend trees, parameters.

use glam::{Vec2, Vec3};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AnimParam {
    Float(f32),
    Int(i32),
    Bool(bool),
    Trigger(bool),
}

impl AnimParam {
    pub fn as_float(&self) -> f32 {
        match self { AnimParam::Float(v) => *v, AnimParam::Int(v) => *v as f32, AnimParam::Bool(v) => if *v { 1.0 } else { 0.0 }, AnimParam::Trigger(v) => if *v { 1.0 } else { 0.0 } }
    }
    pub fn as_bool(&self) -> bool {
        match self { AnimParam::Bool(v) | AnimParam::Trigger(v) => *v, AnimParam::Float(v) => *v != 0.0, AnimParam::Int(v) => *v != 0 }
    }
    pub fn type_label(&self) -> &'static str {
        match self { AnimParam::Float(_) => "Float", AnimParam::Int(_) => "Int", AnimParam::Bool(_) => "Bool", AnimParam::Trigger(_) => "Trigger" }
    }
}

// ---------------------------------------------------------------------------
// Transition conditions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConditionOp { Greater, Less, Equal, NotEqual, True, False }

impl ConditionOp {
    pub fn label(self) -> &'static str {
        match self { ConditionOp::Greater => ">", ConditionOp::Less => "<", ConditionOp::Equal => "==", ConditionOp::NotEqual => "!=", ConditionOp::True => "is true", ConditionOp::False => "is false" }
    }
    pub fn evaluate(&self, lhs: f32, rhs: f32) -> bool {
        match self { ConditionOp::Greater => lhs > rhs, ConditionOp::Less => lhs < rhs, ConditionOp::Equal => (lhs - rhs).abs() < 1e-4, ConditionOp::NotEqual => (lhs - rhs).abs() >= 1e-4, ConditionOp::True => lhs != 0.0, ConditionOp::False => lhs == 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct TransitionCondition {
    pub parameter: String,
    pub op: ConditionOp,
    pub threshold: f32,
}

impl TransitionCondition {
    pub fn new(param: impl Into<String>, op: ConditionOp, threshold: f32) -> Self {
        Self { parameter: param.into(), op, threshold }
    }

    pub fn evaluate(&self, params: &HashMap<String, AnimParam>) -> bool {
        let val = params.get(&self.parameter).map(|p| p.as_float()).unwrap_or(0.0);
        self.op.evaluate(val, self.threshold)
    }
}

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterruptionSource { None, Source, Destination, SourceThenDestination, DestinationThenSource }

#[derive(Debug, Clone)]
pub struct StateTransition {
    pub id: u32,
    pub from_state: u32,
    pub to_state: u32,
    pub conditions: Vec<TransitionCondition>,
    pub duration: f32,
    pub offset: f32,
    pub has_exit_time: bool,
    pub exit_time: f32,
    pub can_transition_to_self: bool,
    pub interruption_source: InterruptionSource,
    pub ordered_interruption: bool,
    pub blend_curve: BlendCurveType,
    pub mute: bool,
    pub solo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendCurveType { Linear, Fixed, EaseIn, EaseOut, EaseInOut, Custom }

impl StateTransition {
    pub fn new(id: u32, from: u32, to: u32) -> Self {
        Self {
            id, from_state: from, to_state: to,
            conditions: Vec::new(),
            duration: 0.25, offset: 0.0,
            has_exit_time: false, exit_time: 0.75,
            can_transition_to_self: false,
            interruption_source: InterruptionSource::None,
            ordered_interruption: true,
            blend_curve: BlendCurveType::Linear,
            mute: false, solo: false,
        }
    }

    pub fn can_trigger(&self, params: &HashMap<String, AnimParam>, normalized_time: f32) -> bool {
        if self.mute { return false; }
        if self.has_exit_time && normalized_time < self.exit_time { return false; }
        if self.conditions.is_empty() { return self.has_exit_time && normalized_time >= self.exit_time; }
        self.conditions.iter().all(|c| c.evaluate(params))
    }

    pub fn blend_weight(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self.blend_curve {
            BlendCurveType::Linear => t,
            BlendCurveType::Fixed => if t > 0.5 { 1.0 } else { 0.0 },
            BlendCurveType::EaseIn => t * t,
            BlendCurveType::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            BlendCurveType::EaseInOut => t * t * (3.0 - 2.0 * t),
            BlendCurveType::Custom => t,
        }
    }
}

// ---------------------------------------------------------------------------
// Blend tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendTreeType { Simple1D, SimpleDirectional2D, FreeformDirectional2D, FreeformCartesian2D, Direct }

#[derive(Debug, Clone)]
pub struct BlendTreeChild {
    pub clip_name: String,
    pub threshold: f32,
    pub position_2d: Vec2,
    pub direct_blend_param: String,
    pub speed: f32,
    pub mirror: bool,
    pub time_scale: f32,
}

impl BlendTreeChild {
    pub fn new_1d(clip: impl Into<String>, threshold: f32) -> Self {
        Self {
            clip_name: clip.into(), threshold,
            position_2d: Vec2::ZERO, direct_blend_param: String::new(),
            speed: 1.0, mirror: false, time_scale: 1.0,
        }
    }
    pub fn new_2d(clip: impl Into<String>, pos: Vec2) -> Self {
        Self {
            clip_name: clip.into(), threshold: 0.0,
            position_2d: pos, direct_blend_param: String::new(),
            speed: 1.0, mirror: false, time_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlendTree {
    pub name: String,
    pub blend_type: BlendTreeType,
    pub blend_param_x: String,
    pub blend_param_y: String,
    pub children: Vec<BlendTreeChild>,
    pub use_auto_thresholds: bool,
    pub compute_threshold_automatically: bool,
}

impl BlendTree {
    pub fn new_1d(name: impl Into<String>, param: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            blend_type: BlendTreeType::Simple1D,
            blend_param_x: param.into(),
            blend_param_y: String::new(),
            children: Vec::new(),
            use_auto_thresholds: true,
            compute_threshold_automatically: true,
        }
    }

    pub fn new_2d(name: impl Into<String>, param_x: impl Into<String>, param_y: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            blend_type: BlendTreeType::SimpleDirectional2D,
            blend_param_x: param_x.into(),
            blend_param_y: param_y.into(),
            children: Vec::new(),
            use_auto_thresholds: false,
            compute_threshold_automatically: false,
        }
    }

    pub fn add_child(&mut self, child: BlendTreeChild) {
        self.children.push(child);
    }

    /// Compute weights for 1D blend.
    pub fn compute_weights_1d(&self, value: f32) -> Vec<f32> {
        if self.children.is_empty() { return Vec::new(); }
        let n = self.children.len();
        let mut weights = vec![0.0_f32; n];
        let thresholds: Vec<f32> = self.children.iter().map(|c| c.threshold).collect();
        let clamped = value.clamp(thresholds[0], thresholds[n - 1]);
        let i = thresholds.partition_point(|&t| t <= clamped);
        if i == 0 { weights[0] = 1.0; }
        else if i >= n { weights[n-1] = 1.0; }
        else {
            let t0 = thresholds[i-1];
            let t1 = thresholds[i];
            let u = (clamped - t0) / (t1 - t0).max(1e-6);
            weights[i-1] = 1.0 - u;
            weights[i] = u;
        }
        weights
    }

    /// Compute weights for 2D directional blend.
    pub fn compute_weights_2d(&self, x: f32, y: f32) -> Vec<f32> {
        if self.children.is_empty() { return Vec::new(); }
        let n = self.children.len();
        let query = Vec2::new(x, y);
        // Simple nearest-neighbor with distance weighting
        let inv_dists: Vec<f32> = self.children.iter()
            .map(|c| 1.0 / (c.position_2d.distance(query) + 0.001))
            .collect();
        let total: f32 = inv_dists.iter().sum();
        if total < 1e-6 { return vec![1.0 / n as f32; n]; }
        inv_dists.iter().map(|&d| d / total).collect()
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StateKind {
    Normal,
    BlendTree,
    Empty,
    Any,
    Entry,
    Exit,
}

#[derive(Debug, Clone)]
pub struct AnimState {
    pub id: u32,
    pub name: String,
    pub kind: StateKind,
    pub position: Vec2,
    pub clip_name: Option<String>,
    pub blend_tree: Option<BlendTree>,
    pub speed: f32,
    pub speed_multiplier_param: Option<String>,
    pub mirror: bool,
    pub mirror_param: Option<String>,
    pub foot_ik: bool,
    pub write_defaults: bool,
    pub loop_time: bool,
    pub cyclic_offset: f32,
    pub tag: String,
    pub motion_time_param: Option<String>,
    pub color: [u8; 3],
}

impl AnimState {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id, name: name.into(), kind: StateKind::Normal,
            position: Vec2::ZERO,
            clip_name: None, blend_tree: None,
            speed: 1.0, speed_multiplier_param: None,
            mirror: false, mirror_param: None,
            foot_ik: false, write_defaults: true,
            loop_time: true, cyclic_offset: 0.0,
            tag: String::new(), motion_time_param: None,
            color: [150, 150, 150],
        }
    }

    pub fn with_clip(mut self, clip: impl Into<String>) -> Self {
        self.clip_name = Some(clip.into());
        self
    }

    pub fn with_blend_tree(mut self, tree: BlendTree) -> Self {
        self.blend_tree = Some(tree);
        self.kind = StateKind::BlendTree;
        self
    }

    pub fn effective_speed(&self, params: &HashMap<String, AnimParam>) -> f32 {
        let mult = self.speed_multiplier_param.as_ref()
            .and_then(|p| params.get(p))
            .map(|p| p.as_float())
            .unwrap_or(1.0);
        self.speed * mult
    }
}

// ---------------------------------------------------------------------------
// Layer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AvatarMask { None, Body, UpperBody, LowerBody, LeftHand, RightHand, LeftFoot, RightFoot, Custom }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerBlendType { Override, Additive }

#[derive(Debug, Clone)]
pub struct AnimLayer {
    pub name: String,
    pub weight: f32,
    pub blend_type: LayerBlendType,
    pub avatar_mask: AvatarMask,
    pub sync: bool,
    pub sync_layer: Option<usize>,
    pub ik_pass: bool,
    pub states: Vec<AnimState>,
    pub transitions: Vec<StateTransition>,
    pub default_state: u32,
    pub current_state: u32,
    pub next_state: Option<u32>,
    pub transition_progress: f32,
    pub current_time: f32,
}

impl AnimLayer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            weight: 1.0,
            blend_type: LayerBlendType::Override,
            avatar_mask: AvatarMask::None,
            sync: false,
            sync_layer: None,
            ik_pass: false,
            states: Vec::new(),
            transitions: Vec::new(),
            default_state: 0,
            current_state: 0,
            next_state: None,
            transition_progress: 0.0,
            current_time: 0.0,
        }
    }

    pub fn add_state(&mut self, state: AnimState) -> u32 {
        let id = state.id;
        if self.states.is_empty() { self.default_state = id; self.current_state = id; }
        self.states.push(state);
        id
    }

    pub fn add_transition(&mut self, t: StateTransition) { self.transitions.push(t); }

    pub fn current_state(&self) -> Option<&AnimState> {
        self.states.iter().find(|s| s.id == self.current_state)
    }

    pub fn update(&mut self, dt: f32, params: &HashMap<String, AnimParam>) {
        let speed = self.current_state()
            .map(|s| s.effective_speed(params))
            .unwrap_or(1.0);
        self.current_time += dt * speed;

        // Check transitions
        if self.next_state.is_none() {
            let outgoing: Vec<StateTransition> = self.transitions.iter()
                .filter(|t| t.from_state == self.current_state)
                .cloned()
                .collect();
            for t in &outgoing {
                if t.can_trigger(params, self.current_time) {
                    self.next_state = Some(t.to_state);
                    self.transition_progress = 0.0;
                    // Consume triggers
                    break;
                }
            }
        }

        if let Some(next) = self.next_state {
            let dur = self.transitions.iter()
                .find(|t| t.from_state == self.current_state && t.to_state == next)
                .map(|t| t.duration)
                .unwrap_or(0.25);
            self.transition_progress += dt / dur.max(0.001);
            if self.transition_progress >= 1.0 {
                self.current_state = next;
                self.next_state = None;
                self.transition_progress = 0.0;
                self.current_time = 0.0;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Animator controller
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AnimatorController {
    pub name: String,
    pub layers: Vec<AnimLayer>,
    pub parameters: HashMap<String, AnimParam>,
    pub default_values: HashMap<String, AnimParam>,
}

impl AnimatorController {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            layers: Vec::new(),
            parameters: HashMap::new(),
            default_values: HashMap::new(),
        }
    }

    pub fn add_layer(&mut self, layer: AnimLayer) {
        self.layers.push(layer);
    }

    pub fn add_param(&mut self, name: impl Into<String>, value: AnimParam) {
        let name = name.into();
        self.default_values.insert(name.clone(), value.clone());
        self.parameters.insert(name, value);
    }

    pub fn set_float(&mut self, name: &str, value: f32) {
        if let Some(p) = self.parameters.get_mut(name) {
            *p = AnimParam::Float(value);
        }
    }
    pub fn set_int(&mut self, name: &str, value: i32) {
        if let Some(p) = self.parameters.get_mut(name) {
            *p = AnimParam::Int(value);
        }
    }
    pub fn set_bool(&mut self, name: &str, value: bool) {
        if let Some(p) = self.parameters.get_mut(name) {
            *p = AnimParam::Bool(value);
        }
    }
    pub fn set_trigger(&mut self, name: &str) {
        if let Some(p) = self.parameters.get_mut(name) {
            *p = AnimParam::Trigger(true);
        }
    }
    pub fn reset_trigger(&mut self, name: &str) {
        if let Some(p) = self.parameters.get_mut(name) {
            if matches!(p, AnimParam::Trigger(_)) {
                *p = AnimParam::Trigger(false);
            }
        }
    }

    pub fn update(&mut self, dt: f32) {
        for layer in &mut self.layers {
            layer.update(dt, &self.parameters);
        }
        // Reset triggers after evaluation
        for p in self.parameters.values_mut() {
            if matches!(p, AnimParam::Trigger(true)) {
                *p = AnimParam::Trigger(false);
            }
        }
    }

    pub fn reset_to_defaults(&mut self) {
        for (k, v) in &self.default_values {
            self.parameters.insert(k.clone(), v.clone());
        }
        for layer in &mut self.layers {
            layer.current_state = layer.default_state;
            layer.next_state = None;
            layer.current_time = 0.0;
        }
    }

    /// Build a sample biped controller.
    pub fn build_biped_controller() -> Self {
        let mut ctrl = AnimatorController::new("BipedController");
        ctrl.add_param("Speed", AnimParam::Float(0.0));
        ctrl.add_param("Direction", AnimParam::Float(0.0));
        ctrl.add_param("IsGrounded", AnimParam::Bool(true));
        ctrl.add_param("Jump", AnimParam::Trigger(false));
        ctrl.add_param("IsAiming", AnimParam::Bool(false));
        ctrl.add_param("AttackIndex", AnimParam::Int(0));

        let mut base_layer = AnimLayer::new("Base Layer");
        let mut id = 1u32;

        // Locomotion blend tree
        let mut loco_tree = BlendTree::new_2d("Locomotion", "Speed", "Direction");
        loco_tree.add_child(BlendTreeChild::new_2d("Idle", Vec2::ZERO));
        loco_tree.add_child(BlendTreeChild::new_2d("Walk_Forward", Vec2::new(0.0, 0.5)));
        loco_tree.add_child(BlendTreeChild::new_2d("Run_Forward", Vec2::new(0.0, 1.0)));
        loco_tree.add_child(BlendTreeChild::new_2d("Walk_Left", Vec2::new(-0.5, 0.5)));
        loco_tree.add_child(BlendTreeChild::new_2d("Walk_Right", Vec2::new(0.5, 0.5)));
        loco_tree.add_child(BlendTreeChild::new_2d("Run_Left", Vec2::new(-1.0, 1.0)));
        loco_tree.add_child(BlendTreeChild::new_2d("Run_Right", Vec2::new(1.0, 1.0)));

        let loco = AnimState::new(id, "Locomotion")
            .with_blend_tree(loco_tree);
        id += 1;
        let loco_id = base_layer.add_state(loco);

        let jump = AnimState::new(id, "Jump").with_clip("Jump");
        id += 1;
        let jump_id = base_layer.add_state(jump);

        let fall = AnimState::new(id, "Fall").with_clip("Fall");
        id += 1;
        let fall_id = base_layer.add_state(fall);

        let land = AnimState::new(id, "Land").with_clip("Land");
        id += 1;
        let land_id = base_layer.add_state(land);

        // Transitions
        let mut t1 = StateTransition::new(id, loco_id, jump_id);
        t1.conditions.push(TransitionCondition::new("Jump", ConditionOp::True, 1.0));
        t1.duration = 0.1;
        base_layer.add_transition(t1);
        id += 1;

        let mut t2 = StateTransition::new(id, jump_id, fall_id);
        t2.has_exit_time = true;
        t2.exit_time = 0.5;
        t2.duration = 0.1;
        base_layer.add_transition(t2);
        id += 1;

        let mut t3 = StateTransition::new(id, fall_id, land_id);
        t3.conditions.push(TransitionCondition::new("IsGrounded", ConditionOp::True, 1.0));
        t3.duration = 0.1;
        base_layer.add_transition(t3);
        id += 1;

        let mut t4 = StateTransition::new(id, land_id, loco_id);
        t4.has_exit_time = true;
        t4.exit_time = 0.9;
        t4.duration = 0.15;
        base_layer.add_transition(t4);

        ctrl.add_layer(base_layer);
        ctrl
    }
}

// ---------------------------------------------------------------------------
// State machine editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StateMachineEditorTool { Select, AddState, AddTransition, Pan }

#[derive(Debug, Clone)]
pub struct StateMachineEditor {
    pub controller: AnimatorController,
    pub active_layer: usize,
    pub selected_state: Option<u32>,
    pub selected_transition: Option<u32>,
    pub tool: StateMachineEditorTool,
    pub zoom: f32,
    pub pan: Vec2,
    pub pending_transition_from: Option<u32>,
    pub show_parameters: bool,
    pub show_grid: bool,
    pub preview_dt: f32,
    pub preview_playing: bool,
    pub state_name_input: String,
    pub search_query: String,
}

impl StateMachineEditor {
    pub fn new() -> Self {
        let controller = AnimatorController::build_biped_controller();
        Self {
            controller,
            active_layer: 0,
            selected_state: None,
            selected_transition: None,
            tool: StateMachineEditorTool::Select,
            zoom: 1.0,
            pan: Vec2::ZERO,
            pending_transition_from: None,
            show_parameters: true,
            show_grid: true,
            preview_dt: 0.016,
            preview_playing: false,
            state_name_input: String::new(),
            search_query: String::new(),
        }
    }

    pub fn active_layer(&self) -> Option<&AnimLayer> {
        self.controller.layers.get(self.active_layer)
    }

    pub fn update(&mut self, dt: f32) {
        if self.preview_playing {
            self.controller.update(dt);
        }
    }

    pub fn active_state_name(&self) -> Option<&str> {
        self.active_layer()
            .and_then(|l| l.current_state())
            .map(|s| s.name.as_str())
    }

    pub fn parameter_count(&self) -> usize {
        self.controller.parameters.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_tree_1d() {
        let mut tree = BlendTree::new_1d("Speed", "speed");
        tree.add_child(BlendTreeChild::new_1d("Idle", 0.0));
        tree.add_child(BlendTreeChild::new_1d("Walk", 0.5));
        tree.add_child(BlendTreeChild::new_1d("Run", 1.0));
        let w = tree.compute_weights_1d(0.25);
        assert_eq!(w.len(), 3);
        assert!((w[0] + w[1] + w[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_transition_condition() {
        let mut params = HashMap::new();
        params.insert("Speed".to_string(), AnimParam::Float(5.0));
        let cond = TransitionCondition::new("Speed", ConditionOp::Greater, 3.0);
        assert!(cond.evaluate(&params));
        let cond2 = TransitionCondition::new("Speed", ConditionOp::Less, 3.0);
        assert!(!cond2.evaluate(&params));
    }

    #[test]
    fn test_controller() {
        let mut ctrl = AnimatorController::build_biped_controller();
        assert!(!ctrl.layers.is_empty());
        ctrl.set_float("Speed", 1.0);
        ctrl.update(0.016);
    }

    #[test]
    fn test_editor() {
        let mut ed = StateMachineEditor::new();
        assert!(ed.active_layer().is_some());
        ed.update(0.016);
    }
}
