#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// SECTION 1: DIALOGUE NODE TYPES
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Start,
    Speaker,
    PlayerChoice,
    Condition,
    SetVariable,
    TriggerEvent,
    Jump,
    End,
    Random,
    Timed,
}

#[derive(Debug, Clone)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn new(id: u64) -> Self { NodeId(id) }
    pub fn value(&self) -> u64 { self.0 }
}

impl PartialEq for NodeId {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
impl Eq for NodeId {}
impl std::hash::Hash for NodeId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.0.hash(state); }
}

#[derive(Debug, Clone)]
pub struct NodeConnection {
    pub from_node: u64,
    pub from_port: usize,
    pub to_node: u64,
    pub to_port: usize,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StartNode {
    pub id: u64,
    pub position: Vec2,
    pub title: String,
    pub conversation_id: String,
    pub initial_variables: HashMap<String, DialogueVariable>,
    pub entry_conditions: Vec<ConditionExpression>,
    pub on_enter_events: Vec<String>,
    pub auto_start: bool,
    pub priority: i32,
    pub tags: Vec<String>,
    pub comment: String,
    pub output_port: u64,
}

impl StartNode {
    pub fn new(id: u64) -> Self {
        StartNode {
            id,
            position: Vec2::ZERO,
            title: format!("Start_{}", id),
            conversation_id: format!("conv_{}", id),
            initial_variables: HashMap::new(),
            entry_conditions: Vec::new(),
            on_enter_events: Vec::new(),
            auto_start: false,
            priority: 0,
            tags: Vec::new(),
            comment: String::new(),
            output_port: 0,
        }
    }

    pub fn evaluate_entry_conditions(&self, scope: &VariableScope) -> bool {
        for cond in &self.entry_conditions {
            if !cond.evaluate(scope) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct SpeakerNode {
    pub id: u64,
    pub position: Vec2,
    pub speaker_id: String,
    pub speaker_name: String,
    pub dialogue_text: String,
    pub localization_key: String,
    pub voice_line_ref: Option<VoiceLineRef>,
    pub emotion_state: EmotionState,
    pub portrait_id: String,
    pub animation_trigger: Option<String>,
    pub camera_hint: Option<CameraHint>,
    pub text_speed: f32,
    pub auto_advance: bool,
    pub auto_advance_delay: f32,
    pub subtitle_timing: Vec<SubtitleCue>,
    pub input_port: u64,
    pub output_port: u64,
    pub tags: Vec<String>,
    pub comment: String,
}

impl SpeakerNode {
    pub fn new(id: u64) -> Self {
        SpeakerNode {
            id,
            position: Vec2::ZERO,
            speaker_id: String::new(),
            speaker_name: String::new(),
            dialogue_text: String::new(),
            localization_key: format!("dlg_{}_text", id),
            voice_line_ref: None,
            emotion_state: EmotionState::default(),
            portrait_id: String::new(),
            animation_trigger: None,
            camera_hint: None,
            text_speed: 1.0,
            auto_advance: false,
            auto_advance_delay: 2.0,
            subtitle_timing: Vec::new(),
            input_port: 0,
            output_port: 1,
            tags: Vec::new(),
            comment: String::new(),
        }
    }

    pub fn get_display_text(&self, locale: &str, loc_table: &LocalizationTable) -> String {
        if let Some(text) = loc_table.get(&self.localization_key, locale) {
            text
        } else {
            self.dialogue_text.clone()
        }
    }

    pub fn calculate_read_time(&self) -> f32 {
        let words = self.dialogue_text.split_whitespace().count();
        let wpm = 200.0_f32;
        (words as f32 / wpm) * 60.0
    }

    pub fn generate_subtitle_cues(&mut self) {
        if let Some(voice) = &self.voice_line_ref {
            let total_duration = voice.duration_secs;
            let words: Vec<&str> = self.dialogue_text.split_whitespace().collect();
            if words.is_empty() { return; }
            let time_per_word = total_duration / words.len() as f32;
            let mut cues = Vec::new();
            let mut current_time = 0.0_f32;
            let mut segment = String::new();
            for (i, word) in words.iter().enumerate() {
                segment.push_str(word);
                segment.push(' ');
                if (i + 1) % 8 == 0 || i == words.len() - 1 {
                    let end_time = current_time + time_per_word * 8.0_f32.min((i + 1) as f32);
                    cues.push(SubtitleCue {
                        start_time: current_time,
                        end_time: end_time.min(total_duration),
                        text: segment.trim().to_string(),
                    });
                    current_time = end_time;
                    segment.clear();
                }
            }
            self.subtitle_timing = cues;
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerChoiceNode {
    pub id: u64,
    pub position: Vec2,
    pub prompt_text: String,
    pub prompt_localization_key: String,
    pub choices: Vec<PlayerChoice>,
    pub default_choice_index: Option<usize>,
    pub timeout_secs: Option<f32>,
    pub timeout_choice_index: Option<usize>,
    pub shuffle_choices: bool,
    pub filter_unavailable: bool,
    pub input_port: u64,
    pub tags: Vec<String>,
    pub comment: String,
}

impl PlayerChoiceNode {
    pub fn new(id: u64) -> Self {
        PlayerChoiceNode {
            id,
            position: Vec2::ZERO,
            prompt_text: String::new(),
            prompt_localization_key: format!("dlg_{}_prompt", id),
            choices: Vec::new(),
            default_choice_index: None,
            timeout_secs: None,
            timeout_choice_index: None,
            shuffle_choices: false,
            filter_unavailable: false,
            input_port: 0,
            tags: Vec::new(),
            comment: String::new(),
        }
    }

    pub fn get_available_choices(&self, scope: &VariableScope) -> Vec<(usize, &PlayerChoice)> {
        let mut available = Vec::new();
        for (i, choice) in self.choices.iter().enumerate() {
            if choice.is_available(scope) {
                available.push((i, choice));
            }
        }
        available
    }

    pub fn add_choice(&mut self, text: String, output_node: u64) -> usize {
        let idx = self.choices.len();
        self.choices.push(PlayerChoice {
            text,
            localization_key: format!("dlg_{}_choice_{}", self.id, idx),
            conditions: Vec::new(),
            hidden_when_unavailable: false,
            output_port: idx,
            output_node,
            tags: Vec::new(),
            tooltip: None,
            icon: None,
            one_time_only: false,
            used: false,
        });
        idx
    }
}

#[derive(Debug, Clone)]
pub struct PlayerChoice {
    pub text: String,
    pub localization_key: String,
    pub conditions: Vec<ConditionExpression>,
    pub hidden_when_unavailable: bool,
    pub output_port: usize,
    pub output_node: u64,
    pub tags: Vec<String>,
    pub tooltip: Option<String>,
    pub icon: Option<String>,
    pub one_time_only: bool,
    pub used: bool,
}

impl PlayerChoice {
    pub fn is_available(&self, scope: &VariableScope) -> bool {
        if self.one_time_only && self.used { return false; }
        for cond in &self.conditions {
            if !cond.evaluate(scope) { return false; }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct ConditionNode {
    pub id: u64,
    pub position: Vec2,
    pub condition: ConditionExpression,
    pub true_output: u64,
    pub false_output: u64,
    pub input_port: u64,
    pub comment: String,
    pub tags: Vec<String>,
}

impl ConditionNode {
    pub fn new(id: u64) -> Self {
        ConditionNode {
            id,
            position: Vec2::ZERO,
            condition: ConditionExpression::Literal(true),
            true_output: 0,
            false_output: 0,
            input_port: 0,
            comment: String::new(),
            tags: Vec::new(),
        }
    }

    pub fn evaluate(&self, scope: &VariableScope) -> u64 {
        if self.condition.evaluate(scope) {
            self.true_output
        } else {
            self.false_output
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetVariableNode {
    pub id: u64,
    pub position: Vec2,
    pub operations: Vec<VariableOperation>,
    pub input_port: u64,
    pub output_port: u64,
    pub comment: String,
    pub tags: Vec<String>,
}

impl SetVariableNode {
    pub fn new(id: u64) -> Self {
        SetVariableNode {
            id,
            position: Vec2::ZERO,
            operations: Vec::new(),
            input_port: 0,
            output_port: 1,
            comment: String::new(),
            tags: Vec::new(),
        }
    }

    pub fn execute(&self, scope: &mut VariableScope) {
        for op in &self.operations {
            op.execute(scope);
        }
    }
}

#[derive(Debug, Clone)]
pub struct VariableOperation {
    pub variable_name: String,
    pub scope_type: ScopeType,
    pub operator: AssignOperator,
    pub value: DialogueValue,
}

impl VariableOperation {
    pub fn execute(&self, scope: &mut VariableScope) {
        let current = scope.get(&self.variable_name, &self.scope_type);
        let new_val = match &self.operator {
            AssignOperator::Set => self.value.clone(),
            AssignOperator::Add => {
                match (&current, &self.value) {
                    (Some(DialogueValue::Int(a)), DialogueValue::Int(b)) => DialogueValue::Int(a + b),
                    (Some(DialogueValue::Float(a)), DialogueValue::Float(b)) => DialogueValue::Float(a + b),
                    (Some(DialogueValue::String(a)), DialogueValue::String(b)) => DialogueValue::String(format!("{}{}", a, b)),
                    _ => self.value.clone(),
                }
            }
            AssignOperator::Subtract => {
                match (&current, &self.value) {
                    (Some(DialogueValue::Int(a)), DialogueValue::Int(b)) => DialogueValue::Int(a - b),
                    (Some(DialogueValue::Float(a)), DialogueValue::Float(b)) => DialogueValue::Float(a - b),
                    _ => self.value.clone(),
                }
            }
            AssignOperator::Multiply => {
                match (&current, &self.value) {
                    (Some(DialogueValue::Int(a)), DialogueValue::Int(b)) => DialogueValue::Int(a * b),
                    (Some(DialogueValue::Float(a)), DialogueValue::Float(b)) => DialogueValue::Float(a * b),
                    _ => self.value.clone(),
                }
            }
            AssignOperator::Divide => {
                match (&current, &self.value) {
                    (Some(DialogueValue::Int(a)), DialogueValue::Int(b)) => {
                        if *b != 0 { DialogueValue::Int(a / b) } else { DialogueValue::Int(*a) }
                    }
                    (Some(DialogueValue::Float(a)), DialogueValue::Float(b)) => {
                        if *b != 0.0 { DialogueValue::Float(a / b) } else { DialogueValue::Float(*a) }
                    }
                    _ => self.value.clone(),
                }
            }
            AssignOperator::Toggle => {
                match &current {
                    Some(DialogueValue::Bool(b)) => DialogueValue::Bool(!b),
                    _ => self.value.clone(),
                }
            }
        };
        scope.set(self.variable_name.clone(), self.scope_type.clone(), new_val);
    }
}

#[derive(Debug, Clone)]
pub enum AssignOperator {
    Set,
    Add,
    Subtract,
    Multiply,
    Divide,
    Toggle,
}

#[derive(Debug, Clone)]
pub struct TriggerEventNode {
    pub id: u64,
    pub position: Vec2,
    pub event_name: String,
    pub event_parameters: HashMap<String, DialogueValue>,
    pub delay_secs: f32,
    pub input_port: u64,
    pub output_port: u64,
    pub comment: String,
    pub tags: Vec<String>,
}

impl TriggerEventNode {
    pub fn new(id: u64) -> Self {
        TriggerEventNode {
            id,
            position: Vec2::ZERO,
            event_name: String::new(),
            event_parameters: HashMap::new(),
            delay_secs: 0.0,
            input_port: 0,
            output_port: 1,
            comment: String::new(),
            tags: Vec::new(),
        }
    }

    pub fn build_event(&self) -> GameEvent {
        GameEvent {
            name: self.event_name.clone(),
            parameters: self.event_parameters.clone(),
            delay: self.delay_secs,
            source: format!("dialogue_node_{}", self.id),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameEvent {
    pub name: String,
    pub parameters: HashMap<String, DialogueValue>,
    pub delay: f32,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct JumpNode {
    pub id: u64,
    pub position: Vec2,
    pub target_conversation_id: String,
    pub target_node_id: u64,
    pub preserve_variables: bool,
    pub return_after: bool,
    pub input_port: u64,
    pub comment: String,
    pub tags: Vec<String>,
}

impl JumpNode {
    pub fn new(id: u64) -> Self {
        JumpNode {
            id,
            position: Vec2::ZERO,
            target_conversation_id: String::new(),
            target_node_id: 0,
            preserve_variables: true,
            return_after: false,
            input_port: 0,
            comment: String::new(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EndNode {
    pub id: u64,
    pub position: Vec2,
    pub end_type: EndType,
    pub on_end_events: Vec<String>,
    pub return_value: Option<DialogueValue>,
    pub input_port: u64,
    pub comment: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EndType {
    Normal,
    Success,
    Failure,
    Interrupted,
    Timeout,
}

impl EndNode {
    pub fn new(id: u64) -> Self {
        EndNode {
            id,
            position: Vec2::ZERO,
            end_type: EndType::Normal,
            on_end_events: Vec::new(),
            return_value: None,
            input_port: 0,
            comment: String::new(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RandomNode {
    pub id: u64,
    pub position: Vec2,
    pub outputs: Vec<RandomOutput>,
    pub seed: Option<u64>,
    pub use_weights: bool,
    pub input_port: u64,
    pub comment: String,
    pub tags: Vec<String>,
    rng_state: u64,
}

#[derive(Debug, Clone)]
pub struct RandomOutput {
    pub weight: f32,
    pub output_node: u64,
    pub label: String,
}

impl RandomNode {
    pub fn new(id: u64) -> Self {
        RandomNode {
            id,
            position: Vec2::ZERO,
            outputs: Vec::new(),
            seed: None,
            use_weights: true,
            input_port: 0,
            comment: String::new(),
            tags: Vec::new(),
            rng_state: id.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407),
        }
    }

    pub fn pick_output(&mut self) -> Option<u64> {
        if self.outputs.is_empty() { return None; }
        self.rng_state = self.rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let rand_float = (self.rng_state >> 33) as f32 / (u32::MAX as f32);
        if self.use_weights {
            let total_weight: f32 = self.outputs.iter().map(|o| o.weight).sum();
            let mut cursor = rand_float * total_weight;
            for output in &self.outputs {
                cursor -= output.weight;
                if cursor <= 0.0 {
                    return Some(output.output_node);
                }
            }
            self.outputs.last().map(|o| o.output_node)
        } else {
            let idx = (rand_float * self.outputs.len() as f32) as usize;
            let idx = idx.min(self.outputs.len() - 1);
            Some(self.outputs[idx].output_node)
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimedNode {
    pub id: u64,
    pub position: Vec2,
    pub duration_secs: f32,
    pub on_timeout_node: u64,
    pub on_complete_node: u64,
    pub show_timer: bool,
    pub timer_label: String,
    pub input_port: u64,
    pub comment: String,
    pub tags: Vec<String>,
    pub elapsed: f32,
}

impl TimedNode {
    pub fn new(id: u64) -> Self {
        TimedNode {
            id,
            position: Vec2::ZERO,
            duration_secs: 5.0,
            on_timeout_node: 0,
            on_complete_node: 0,
            show_timer: true,
            timer_label: String::new(),
            input_port: 0,
            comment: String::new(),
            tags: Vec::new(),
            elapsed: 0.0,
        }
    }

    pub fn update(&mut self, delta: f32) -> TimedNodeResult {
        self.elapsed += delta;
        if self.elapsed >= self.duration_secs {
            TimedNodeResult::Timeout(self.on_timeout_node)
        } else {
            TimedNodeResult::Running(self.elapsed / self.duration_secs)
        }
    }

    pub fn reset(&mut self) { self.elapsed = 0.0; }
    pub fn remaining(&self) -> f32 { (self.duration_secs - self.elapsed).max(0.0) }
    pub fn progress(&self) -> f32 { (self.elapsed / self.duration_secs).clamp(0.0, 1.0) }
}

#[derive(Debug, Clone)]
pub enum TimedNodeResult {
    Running(f32),
    Timeout(u64),
    Complete(u64),
}

// ============================================================
// SECTION 2: DIALOGUE TREE
// ============================================================

#[derive(Debug, Clone)]
pub enum DialogueNode {
    Start(StartNode),
    Speaker(SpeakerNode),
    PlayerChoice(PlayerChoiceNode),
    Condition(ConditionNode),
    SetVariable(SetVariableNode),
    TriggerEvent(TriggerEventNode),
    Jump(JumpNode),
    End(EndNode),
    Random(RandomNode),
    Timed(TimedNode),
}

impl DialogueNode {
    pub fn id(&self) -> u64 {
        match self {
            DialogueNode::Start(n) => n.id,
            DialogueNode::Speaker(n) => n.id,
            DialogueNode::PlayerChoice(n) => n.id,
            DialogueNode::Condition(n) => n.id,
            DialogueNode::SetVariable(n) => n.id,
            DialogueNode::TriggerEvent(n) => n.id,
            DialogueNode::Jump(n) => n.id,
            DialogueNode::End(n) => n.id,
            DialogueNode::Random(n) => n.id,
            DialogueNode::Timed(n) => n.id,
        }
    }

    pub fn position(&self) -> Vec2 {
        match self {
            DialogueNode::Start(n) => n.position,
            DialogueNode::Speaker(n) => n.position,
            DialogueNode::PlayerChoice(n) => n.position,
            DialogueNode::Condition(n) => n.position,
            DialogueNode::SetVariable(n) => n.position,
            DialogueNode::TriggerEvent(n) => n.position,
            DialogueNode::Jump(n) => n.position,
            DialogueNode::End(n) => n.position,
            DialogueNode::Random(n) => n.position,
            DialogueNode::Timed(n) => n.position,
        }
    }

    pub fn set_position(&mut self, pos: Vec2) {
        match self {
            DialogueNode::Start(n) => n.position = pos,
            DialogueNode::Speaker(n) => n.position = pos,
            DialogueNode::PlayerChoice(n) => n.position = pos,
            DialogueNode::Condition(n) => n.position = pos,
            DialogueNode::SetVariable(n) => n.position = pos,
            DialogueNode::TriggerEvent(n) => n.position = pos,
            DialogueNode::Jump(n) => n.position = pos,
            DialogueNode::End(n) => n.position = pos,
            DialogueNode::Random(n) => n.position = pos,
            DialogueNode::Timed(n) => n.position = pos,
        }
    }

    pub fn node_type(&self) -> NodeType {
        match self {
            DialogueNode::Start(_) => NodeType::Start,
            DialogueNode::Speaker(_) => NodeType::Speaker,
            DialogueNode::PlayerChoice(_) => NodeType::PlayerChoice,
            DialogueNode::Condition(_) => NodeType::Condition,
            DialogueNode::SetVariable(_) => NodeType::SetVariable,
            DialogueNode::TriggerEvent(_) => NodeType::TriggerEvent,
            DialogueNode::Jump(_) => NodeType::Jump,
            DialogueNode::End(_) => NodeType::End,
            DialogueNode::Random(_) => NodeType::Random,
            DialogueNode::Timed(_) => NodeType::Timed,
        }
    }

    pub fn tags(&self) -> &[String] {
        match self {
            DialogueNode::Start(n) => &n.tags,
            DialogueNode::Speaker(n) => &n.tags,
            DialogueNode::PlayerChoice(n) => &n.tags,
            DialogueNode::Condition(n) => &n.tags,
            DialogueNode::SetVariable(n) => &n.tags,
            DialogueNode::TriggerEvent(n) => &n.tags,
            DialogueNode::Jump(n) => &n.tags,
            DialogueNode::End(n) => &n.tags,
            DialogueNode::Random(n) => &n.tags,
            DialogueNode::Timed(n) => &n.tags,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DialogueTree {
    pub id: String,
    pub name: String,
    pub nodes: HashMap<u64, DialogueNode>,
    pub connections: Vec<NodeConnection>,
    pub start_node_id: Option<u64>,
    pub metadata: DialogueTreeMetadata,
    next_id: u64,
}

#[derive(Debug, Clone, Default)]
pub struct DialogueTreeMetadata {
    pub author: String,
    pub version: String,
    pub created_at: String,
    pub modified_at: String,
    pub description: String,
    pub tags: Vec<String>,
    pub localization_keys: HashSet<String>,
    pub voice_lines: Vec<String>,
    pub speaker_ids: HashSet<String>,
}

impl DialogueTree {
    pub fn new(id: String, name: String) -> Self {
        DialogueTree {
            id,
            name,
            nodes: HashMap::new(),
            connections: Vec::new(),
            start_node_id: None,
            metadata: DialogueTreeMetadata::default(),
            next_id: 1,
        }
    }

    pub fn add_node(&mut self, node: DialogueNode) -> u64 {
        let id = node.id();
        if let DialogueNode::Start(_) = &node {
            self.start_node_id = Some(id);
        }
        self.nodes.insert(id, node);
        id
    }

    pub fn remove_node(&mut self, id: u64) {
        self.nodes.remove(&id);
        self.connections.retain(|c| c.from_node != id && c.to_node != id);
        if self.start_node_id == Some(id) {
            self.start_node_id = None;
        }
    }

    pub fn connect(&mut self, from: u64, from_port: usize, to: u64, to_port: usize, label: Option<String>) {
        // Remove existing connection from same port
        self.connections.retain(|c| !(c.from_node == from && c.from_port == from_port));
        self.connections.push(NodeConnection {
            from_node: from,
            from_port,
            to_node: to,
            to_port,
            label,
        });
    }

    pub fn disconnect(&mut self, from: u64, from_port: usize) {
        self.connections.retain(|c| !(c.from_node == from && c.from_port == from_port));
    }

    pub fn get_outputs(&self, node_id: u64) -> Vec<&NodeConnection> {
        self.connections.iter().filter(|c| c.from_node == node_id).collect()
    }

    pub fn get_inputs(&self, node_id: u64) -> Vec<&NodeConnection> {
        self.connections.iter().filter(|c| c.to_node == node_id).collect()
    }

    pub fn next_node(&self, from: u64, port: usize) -> Option<u64> {
        self.connections.iter()
            .find(|c| c.from_node == from && c.from_port == port)
            .map(|c| c.to_node)
    }

    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        if self.start_node_id.is_none() {
            errors.push(ValidationError {
                node_id: None,
                severity: ValidationSeverity::Error,
                message: "Dialogue tree has no start node".into(),
            });
        }
        for (id, node) in &self.nodes {
            let outputs = self.get_outputs(*id);
            match node {
                DialogueNode::Speaker(n) => {
                    if n.dialogue_text.is_empty() && n.localization_key.is_empty() {
                        errors.push(ValidationError {
                            node_id: Some(*id),
                            severity: ValidationSeverity::Warning,
                            message: "Speaker node has no text".into(),
                        });
                    }
                    if outputs.is_empty() {
                        errors.push(ValidationError {
                            node_id: Some(*id),
                            severity: ValidationSeverity::Warning,
                            message: "Speaker node has no output connection".into(),
                        });
                    }
                }
                DialogueNode::PlayerChoice(n) => {
                    if n.choices.is_empty() {
                        errors.push(ValidationError {
                            node_id: Some(*id),
                            severity: ValidationSeverity::Error,
                            message: "Choice node has no choices".into(),
                        });
                    }
                }
                DialogueNode::Condition(n) => {
                    let has_true = self.connections.iter().any(|c| c.from_node == *id && c.from_port == 0);
                    let has_false = self.connections.iter().any(|c| c.from_node == *id && c.from_port == 1);
                    if !has_true {
                        errors.push(ValidationError {
                            node_id: Some(*id),
                            severity: ValidationSeverity::Warning,
                            message: "Condition node missing true branch".into(),
                        });
                    }
                    if !has_false {
                        errors.push(ValidationError {
                            node_id: Some(*id),
                            severity: ValidationSeverity::Warning,
                            message: "Condition node missing false branch".into(),
                        });
                    }
                }
                _ => {}
            }
        }
        // Check for unreachable nodes
        if let Some(start) = self.start_node_id {
            let reachable = self.find_reachable_nodes(start);
            for id in self.nodes.keys() {
                if !reachable.contains(id) {
                    errors.push(ValidationError {
                        node_id: Some(*id),
                        severity: ValidationSeverity::Warning,
                        message: "Node is unreachable from start".into(),
                    });
                }
            }
        }
        errors
    }

    pub fn find_reachable_nodes(&self, start: u64) -> HashSet<u64> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);
        while let Some(id) = queue.pop_front() {
            if visited.contains(&id) { continue; }
            visited.insert(id);
            for conn in self.get_outputs(id) {
                queue.push_back(conn.to_node);
            }
        }
        visited
    }

    pub fn topological_sort(&self) -> Vec<u64> {
        let mut in_degree: HashMap<u64, usize> = HashMap::new();
        for id in self.nodes.keys() {
            in_degree.insert(*id, 0);
        }
        for conn in &self.connections {
            *in_degree.entry(conn.to_node).or_insert(0) += 1;
        }
        let mut queue: VecDeque<u64> = in_degree.iter()
            .filter(|(_, &d)| d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut result = Vec::new();
        while let Some(id) = queue.pop_front() {
            result.push(id);
            for conn in self.get_outputs(id) {
                let deg = in_degree.entry(conn.to_node).or_insert(0);
                if *deg > 0 { *deg -= 1; }
                if *deg == 0 { queue.push_back(conn.to_node); }
            }
        }
        result
    }

    pub fn find_cycles(&self) -> Vec<Vec<u64>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();
        for &id in self.nodes.keys() {
            if !visited.contains(&id) {
                self.dfs_cycle(id, &mut visited, &mut rec_stack, &mut path, &mut cycles);
            }
        }
        cycles
    }

    fn dfs_cycle(&self, node: u64, visited: &mut HashSet<u64>, rec_stack: &mut HashSet<u64>, path: &mut Vec<u64>, cycles: &mut Vec<Vec<u64>>) {
        visited.insert(node);
        rec_stack.insert(node);
        path.push(node);
        for conn in self.get_outputs(node) {
            let next = conn.to_node;
            if !visited.contains(&next) {
                self.dfs_cycle(next, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(&next) {
                let start = path.iter().position(|&n| n == next).unwrap_or(0);
                cycles.push(path[start..].to_vec());
            }
        }
        path.pop();
        rec_stack.remove(&node);
    }

    pub fn collect_all_localization_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        for node in self.nodes.values() {
            match node {
                DialogueNode::Speaker(n) => {
                    if !n.localization_key.is_empty() {
                        keys.push(n.localization_key.clone());
                    }
                }
                DialogueNode::PlayerChoice(n) => {
                    if !n.prompt_localization_key.is_empty() {
                        keys.push(n.prompt_localization_key.clone());
                    }
                    for choice in &n.choices {
                        if !choice.localization_key.is_empty() {
                            keys.push(choice.localization_key.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        keys
    }

    pub fn collect_all_voice_lines(&self) -> Vec<VoiceLineRef> {
        let mut lines = Vec::new();
        for node in self.nodes.values() {
            if let DialogueNode::Speaker(n) = node {
                if let Some(vl) = &n.voice_line_ref {
                    lines.push(vl.clone());
                }
            }
        }
        lines
    }

    pub fn auto_layout(&mut self) {
        if let Some(start_id) = self.start_node_id {
            let mut positions: HashMap<u64, Vec2> = HashMap::new();
            let mut queue = VecDeque::new();
            let mut visited = HashSet::new();
            queue.push_back((start_id, 0usize, 0usize));
            let node_width = 200.0_f32;
            let node_height = 120.0_f32;
            let x_gap = 50.0_f32;
            let y_gap = 30.0_f32;
            let mut col_row: HashMap<usize, usize> = HashMap::new();
            while let Some((id, col, _row)) = queue.pop_front() {
                if visited.contains(&id) { continue; }
                visited.insert(id);
                let row = *col_row.entry(col).or_insert(0);
                *col_row.entry(col).or_insert(0) += 1;
                let x = col as f32 * (node_width + x_gap);
                let y = row as f32 * (node_height + y_gap);
                positions.insert(id, Vec2::new(x, y));
                for conn in self.get_outputs(id).into_iter().map(|c| (c.to_node, c.from_port)) {
                    queue.push_back((conn.0, col + 1, conn.1));
                }
            }
            for (id, pos) in positions {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.set_position(pos);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub node_id: Option<u64>,
    pub severity: ValidationSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
}

// ============================================================
// SECTION 3: CONDITION SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub enum ConditionExpression {
    Literal(bool),
    Comparison(ComparisonCondition),
    QuestState(QuestStateCondition),
    FactionRep(FactionRepCondition),
    Flag(FlagCondition),
    And(Box<ConditionExpression>, Box<ConditionExpression>),
    Or(Box<ConditionExpression>, Box<ConditionExpression>),
    Not(Box<ConditionExpression>),
    All(Vec<ConditionExpression>),
    Any(Vec<ConditionExpression>),
}

impl ConditionExpression {
    pub fn evaluate(&self, scope: &VariableScope) -> bool {
        match self {
            ConditionExpression::Literal(b) => *b,
            ConditionExpression::Comparison(c) => c.evaluate(scope),
            ConditionExpression::QuestState(c) => c.evaluate(scope),
            ConditionExpression::FactionRep(c) => c.evaluate(scope),
            ConditionExpression::Flag(c) => c.evaluate(scope),
            ConditionExpression::And(a, b) => a.evaluate(scope) && b.evaluate(scope),
            ConditionExpression::Or(a, b) => a.evaluate(scope) || b.evaluate(scope),
            ConditionExpression::Not(e) => !e.evaluate(scope),
            ConditionExpression::All(exprs) => exprs.iter().all(|e| e.evaluate(scope)),
            ConditionExpression::Any(exprs) => exprs.iter().any(|e| e.evaluate(scope)),
        }
    }

    pub fn and(self, other: ConditionExpression) -> ConditionExpression {
        ConditionExpression::And(Box::new(self), Box::new(other))
    }

    pub fn or(self, other: ConditionExpression) -> ConditionExpression {
        ConditionExpression::Or(Box::new(self), Box::new(other))
    }

    pub fn not(self) -> ConditionExpression {
        ConditionExpression::Not(Box::new(self))
    }

    pub fn describe(&self) -> String {
        match self {
            ConditionExpression::Literal(b) => format!("{}", b),
            ConditionExpression::Comparison(c) => c.describe(),
            ConditionExpression::QuestState(c) => c.describe(),
            ConditionExpression::FactionRep(c) => c.describe(),
            ConditionExpression::Flag(c) => c.describe(),
            ConditionExpression::And(a, b) => format!("({} AND {})", a.describe(), b.describe()),
            ConditionExpression::Or(a, b) => format!("({} OR {})", a.describe(), b.describe()),
            ConditionExpression::Not(e) => format!("NOT ({})", e.describe()),
            ConditionExpression::All(exprs) => {
                let parts: Vec<String> = exprs.iter().map(|e| e.describe()).collect();
                format!("ALL({})", parts.join(", "))
            }
            ConditionExpression::Any(exprs) => {
                let parts: Vec<String> = exprs.iter().map(|e| e.describe()).collect();
                format!("ANY({})", parts.join(", "))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComparisonCondition {
    pub variable_name: String,
    pub scope_type: ScopeType,
    pub operator: ComparisonOperator,
    pub compare_value: DialogueValue,
}

impl ComparisonCondition {
    pub fn evaluate(&self, scope: &VariableScope) -> bool {
        if let Some(var) = scope.get(&self.variable_name, &self.scope_type) {
            self.operator.compare(&var, &self.compare_value)
        } else {
            false
        }
    }

    pub fn describe(&self) -> String {
        format!("{} {:?} {:?}", self.variable_name, self.operator, self.compare_value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
    Contains,
    StartsWith,
    EndsWith,
}

impl ComparisonOperator {
    pub fn compare(&self, a: &DialogueValue, b: &DialogueValue) -> bool {
        match (a, b) {
            (DialogueValue::Int(x), DialogueValue::Int(y)) => match self {
                ComparisonOperator::Equal => x == y,
                ComparisonOperator::NotEqual => x != y,
                ComparisonOperator::LessThan => x < y,
                ComparisonOperator::LessEqual => x <= y,
                ComparisonOperator::GreaterThan => x > y,
                ComparisonOperator::GreaterEqual => x >= y,
                _ => false,
            },
            (DialogueValue::Float(x), DialogueValue::Float(y)) => match self {
                ComparisonOperator::Equal => (x - y).abs() < 1e-6,
                ComparisonOperator::NotEqual => (x - y).abs() >= 1e-6,
                ComparisonOperator::LessThan => x < y,
                ComparisonOperator::LessEqual => x <= y,
                ComparisonOperator::GreaterThan => x > y,
                ComparisonOperator::GreaterEqual => x >= y,
                _ => false,
            },
            (DialogueValue::Bool(x), DialogueValue::Bool(y)) => match self {
                ComparisonOperator::Equal => x == y,
                ComparisonOperator::NotEqual => x != y,
                _ => false,
            },
            (DialogueValue::String(x), DialogueValue::String(y)) => match self {
                ComparisonOperator::Equal => x == y,
                ComparisonOperator::NotEqual => x != y,
                ComparisonOperator::Contains => x.contains(y.as_str()),
                ComparisonOperator::StartsWith => x.starts_with(y.as_str()),
                ComparisonOperator::EndsWith => x.ends_with(y.as_str()),
                _ => false,
            },
            _ => false,
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            ComparisonOperator::Equal => "==",
            ComparisonOperator::NotEqual => "!=",
            ComparisonOperator::LessThan => "<",
            ComparisonOperator::LessEqual => "<=",
            ComparisonOperator::GreaterThan => ">",
            ComparisonOperator::GreaterEqual => ">=",
            ComparisonOperator::Contains => "contains",
            ComparisonOperator::StartsWith => "starts_with",
            ComparisonOperator::EndsWith => "ends_with",
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuestStateCondition {
    pub quest_id: String,
    pub expected_state: QuestStateEnum,
    pub objective_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuestStateEnum {
    NotStarted,
    Active,
    Completed,
    Failed,
    Abandoned,
}

impl QuestStateCondition {
    pub fn evaluate(&self, scope: &VariableScope) -> bool {
        let key = format!("quest_state_{}", self.quest_id);
        if let Some(DialogueValue::String(state)) = scope.get(&key, &ScopeType::Global) {
            let matches = match &self.expected_state {
                QuestStateEnum::NotStarted => state == "not_started",
                QuestStateEnum::Active => state == "active",
                QuestStateEnum::Completed => state == "completed",
                QuestStateEnum::Failed => state == "failed",
                QuestStateEnum::Abandoned => state == "abandoned",
            };
            if matches {
                if let Some(obj_id) = &self.objective_id {
                    let obj_key = format!("quest_{}_obj_{}", self.quest_id, obj_id);
                    return matches!(scope.get(&obj_key, &ScopeType::Global), Some(DialogueValue::Bool(true)));
                }
            }
            matches
        } else {
            self.expected_state == QuestStateEnum::NotStarted
        }
    }

    pub fn describe(&self) -> String {
        format!("quest({}) == {:?}", self.quest_id, self.expected_state)
    }
}

#[derive(Debug, Clone)]
pub struct FactionRepCondition {
    pub faction_id: String,
    pub operator: ComparisonOperator,
    pub threshold: i32,
}

impl FactionRepCondition {
    pub fn evaluate(&self, scope: &VariableScope) -> bool {
        let key = format!("faction_rep_{}", self.faction_id);
        if let Some(DialogueValue::Int(rep)) = scope.get(&key, &ScopeType::Global) {
            self.operator.compare(&DialogueValue::Int(rep), &DialogueValue::Int(self.threshold as i64))
        } else {
            false
        }
    }

    pub fn describe(&self) -> String {
        format!("faction({}) {} {}", self.faction_id, self.operator.symbol(), self.threshold)
    }
}

#[derive(Debug, Clone)]
pub struct FlagCondition {
    pub flag_name: String,
    pub expected: bool,
}

impl FlagCondition {
    pub fn evaluate(&self, scope: &VariableScope) -> bool {
        if let Some(DialogueValue::Bool(val)) = scope.get(&self.flag_name, &ScopeType::Persistent) {
            val == self.expected
        } else {
            !self.expected
        }
    }

    pub fn describe(&self) -> String {
        if self.expected {
            format!("flag({})", self.flag_name)
        } else {
            format!("!flag({})", self.flag_name)
        }
    }
}

// ============================================================
// SECTION 4: VARIABLE TRACKING
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum DialogueValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Flag(bool),
}

impl DialogueValue {
    pub fn type_name(&self) -> &str {
        match self {
            DialogueValue::Bool(_) => "bool",
            DialogueValue::Int(_) => "int",
            DialogueValue::Float(_) => "float",
            DialogueValue::String(_) => "string",
            DialogueValue::Flag(_) => "flag",
        }
    }

    pub fn to_string_repr(&self) -> String {
        match self {
            DialogueValue::Bool(b) => b.to_string(),
            DialogueValue::Int(i) => i.to_string(),
            DialogueValue::Float(f) => format!("{:.4}", f),
            DialogueValue::String(s) => s.clone(),
            DialogueValue::Flag(b) => if *b { "set".to_string() } else { "unset".to_string() },
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            DialogueValue::Bool(b) => *b,
            DialogueValue::Int(i) => *i != 0,
            DialogueValue::Float(f) => *f != 0.0,
            DialogueValue::String(s) => !s.is_empty(),
            DialogueValue::Flag(b) => *b,
        }
    }

    pub fn as_int(&self) -> i64 {
        match self {
            DialogueValue::Bool(b) => if *b { 1 } else { 0 },
            DialogueValue::Int(i) => *i,
            DialogueValue::Float(f) => *f as i64,
            DialogueValue::String(s) => s.parse().unwrap_or(0),
            DialogueValue::Flag(b) => if *b { 1 } else { 0 },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScopeType {
    Local,       // current conversation only
    Global,      // session-wide
    Persistent,  // saved to disk
}

#[derive(Debug, Clone, Default)]
pub struct VariableScope {
    pub local: HashMap<String, DialogueValue>,
    pub global: HashMap<String, DialogueValue>,
    pub persistent: HashMap<String, DialogueValue>,
    pub change_log: Vec<VariableChange>,
}

#[derive(Debug, Clone)]
pub struct VariableChange {
    pub name: String,
    pub scope: ScopeType,
    pub old_value: Option<DialogueValue>,
    pub new_value: DialogueValue,
    pub timestamp: f64,
}

impl VariableScope {
    pub fn new() -> Self { Self::default() }

    pub fn get(&self, name: &str, scope: &ScopeType) -> Option<DialogueValue> {
        match scope {
            ScopeType::Local => self.local.get(name).cloned(),
            ScopeType::Global => self.global.get(name).cloned()
                .or_else(|| self.local.get(name).cloned()),
            ScopeType::Persistent => self.persistent.get(name).cloned()
                .or_else(|| self.global.get(name).cloned()),
        }
    }

    pub fn get_any(&self, name: &str) -> Option<DialogueValue> {
        self.local.get(name).cloned()
            .or_else(|| self.global.get(name).cloned())
            .or_else(|| self.persistent.get(name).cloned())
    }

    pub fn set(&mut self, name: String, scope: ScopeType, value: DialogueValue) {
        let old = self.get_any(&name);
        let change = VariableChange {
            name: name.clone(),
            scope: scope.clone(),
            old_value: old,
            new_value: value.clone(),
            timestamp: 0.0,
        };
        self.change_log.push(change);
        match scope {
            ScopeType::Local => { self.local.insert(name, value); }
            ScopeType::Global => { self.global.insert(name, value); }
            ScopeType::Persistent => { self.persistent.insert(name, value); }
        }
    }

    pub fn set_flag(&mut self, name: &str) {
        self.persistent.insert(name.to_string(), DialogueValue::Flag(true));
    }

    pub fn clear_flag(&mut self, name: &str) {
        self.persistent.insert(name.to_string(), DialogueValue::Flag(false));
    }

    pub fn has_flag(&self, name: &str) -> bool {
        matches!(self.persistent.get(name), Some(DialogueValue::Flag(true)) | Some(DialogueValue::Bool(true)))
    }

    pub fn clear_local(&mut self) {
        self.local.clear();
    }

    pub fn serialize_persistent(&self) -> String {
        let mut parts = Vec::new();
        for (k, v) in &self.persistent {
            parts.push(format!("{}={}", k, v.to_string_repr()));
        }
        parts.sort();
        parts.join("\n")
    }

    pub fn deserialize_persistent(&mut self, data: &str) {
        for line in data.lines() {
            if let Some((k, v)) = line.split_once('=') {
                // Try to parse typed values
                let val = if v == "true" {
                    DialogueValue::Bool(true)
                } else if v == "false" {
                    DialogueValue::Bool(false)
                } else if let Ok(i) = v.parse::<i64>() {
                    DialogueValue::Int(i)
                } else if let Ok(f) = v.parse::<f64>() {
                    DialogueValue::Float(f)
                } else {
                    DialogueValue::String(v.to_string())
                };
                self.persistent.insert(k.to_string(), val);
            }
        }
    }

    pub fn get_all_vars(&self) -> Vec<(String, ScopeType, &DialogueValue)> {
        let mut all = Vec::new();
        for (k, v) in &self.local {
            all.push((k.clone(), ScopeType::Local, v));
        }
        for (k, v) in &self.global {
            all.push((k.clone(), ScopeType::Global, v));
        }
        for (k, v) in &self.persistent {
            all.push((k.clone(), ScopeType::Persistent, v));
        }
        all.sort_by(|a, b| a.0.cmp(&b.0));
        all
    }
}

#[derive(Debug, Clone)]
pub struct DialogueVariable {
    pub name: String,
    pub value: DialogueValue,
    pub scope: ScopeType,
    pub description: String,
    pub default_value: DialogueValue,
}

// ============================================================
// SECTION 5: LOCALIZATION
// ============================================================

#[derive(Debug, Clone)]
pub struct LocalizationTable {
    pub tables: HashMap<String, HashMap<String, String>>,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub missing_keys: HashSet<String>,
    pub untranslated_keys: HashMap<String, Vec<String>>,
}

impl LocalizationTable {
    pub fn new(default_locale: &str) -> Self {
        let mut tables = HashMap::new();
        tables.insert(default_locale.to_string(), HashMap::new());
        LocalizationTable {
            tables,
            default_locale: default_locale.to_string(),
            supported_locales: vec![default_locale.to_string()],
            missing_keys: HashSet::new(),
            untranslated_keys: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, locale: &str, text: &str) {
        self.tables
            .entry(locale.to_string())
            .or_insert_with(HashMap::new)
            .insert(key.to_string(), text.to_string());
    }

    pub fn get(&self, key: &str, locale: &str) -> Option<String> {
        if let Some(table) = self.tables.get(locale) {
            if let Some(text) = table.get(key) {
                return Some(text.clone());
            }
        }
        // Fallback to default locale
        if locale != self.default_locale {
            if let Some(table) = self.tables.get(&self.default_locale) {
                return table.get(key).cloned();
            }
        }
        None
    }

    pub fn add_locale(&mut self, locale: &str) {
        if !self.supported_locales.contains(&locale.to_string()) {
            self.supported_locales.push(locale.to_string());
            self.tables.insert(locale.to_string(), HashMap::new());
        }
    }

    pub fn generate_key(conversation_id: &str, node_id: u64, field: &str) -> String {
        format!("{}_{:04}_{}", conversation_id, node_id, field)
    }

    pub fn validate_key(key: &str) -> bool {
        !key.is_empty() &&
        key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-') &&
        !key.starts_with('_') &&
        !key.ends_with('_')
    }

    pub fn find_missing_translations(&mut self) -> HashMap<String, Vec<String>> {
        let default_keys: HashSet<String> = self.tables
            .get(&self.default_locale)
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default();
        let mut missing: HashMap<String, Vec<String>> = HashMap::new();
        for locale in &self.supported_locales.clone() {
            if locale == &self.default_locale { continue; }
            if let Some(table) = self.tables.get(locale) {
                for key in &default_keys {
                    if !table.contains_key(key) {
                        missing.entry(locale.clone()).or_default().push(key.clone());
                    }
                }
            } else {
                missing.insert(locale.clone(), default_keys.iter().cloned().collect());
            }
        }
        self.untranslated_keys = missing.clone();
        missing
    }

    pub fn export_to_json(&self, locale: &str) -> String {
        let mut json = String::from("{\n");
        if let Some(table) = self.tables.get(locale) {
            let mut entries: Vec<(&String, &String)> = table.iter().collect();
            entries.sort_by_key(|(k, _)| k.as_str());
            for (i, (key, value)) in entries.iter().enumerate() {
                let escaped = value.replace('"', "\\\"").replace('\n', "\\n");
                if i < entries.len() - 1 {
                    json.push_str(&format!("  \"{}\": \"{}\",\n", key, escaped));
                } else {
                    json.push_str(&format!("  \"{}\": \"{}\"\n", key, escaped));
                }
            }
        }
        json.push('}');
        json
    }

    pub fn export_to_csv(&self, locale: &str) -> String {
        let mut csv = String::from("key,text,default_text\n");
        let default_table = self.tables.get(&self.default_locale);
        if let Some(table) = self.tables.get(locale) {
            let mut entries: Vec<(&String, &String)> = table.iter().collect();
            entries.sort_by_key(|(k, _)| k.as_str());
            for (key, value) in &entries {
                let escaped_val = value.replace('"', "\"\"");
                let default_val = default_table
                    .and_then(|t| t.get(*key))
                    .map(|s| s.replace('"', "\"\""))
                    .unwrap_or_default();
                csv.push_str(&format!("\"{}\",\"{}\",\"{}\"\n", key, escaped_val, default_val));
            }
        }
        csv
    }

    pub fn import_from_json(&mut self, json: &str, locale: &str) -> Result<usize, String> {
        // Simple JSON parser for flat string maps
        let json = json.trim();
        if !json.starts_with('{') || !json.ends_with('}') {
            return Err("Invalid JSON format".to_string());
        }
        let inner = &json[1..json.len()-1];
        let mut count = 0;
        let table = self.tables.entry(locale.to_string()).or_insert_with(HashMap::new);
        for line in inner.lines() {
            let line = line.trim().trim_end_matches(',');
            if line.is_empty() { continue; }
            if let Some(colon_pos) = line.find(':') {
                let key_part = line[..colon_pos].trim().trim_matches('"');
                let val_part = line[colon_pos+1..].trim().trim_matches('"');
                table.insert(key_part.to_string(), val_part.replace("\\n", "\n").replace("\\\"", "\""));
                count += 1;
            }
        }
        Ok(count)
    }
}

// ============================================================
// SECTION 6: VOICE LINES
// ============================================================

#[derive(Debug, Clone)]
pub struct VoiceLineRef {
    pub clip_id: String,
    pub file_path: String,
    pub speaker_id: String,
    pub emotion: EmotionType,
    pub duration_secs: f32,
    pub sample_rate: u32,
    pub channels: u8,
    pub file_size_bytes: u64,
    pub lip_sync_data: Option<LipSyncData>,
    pub subtitle_offset: f32,
    pub waveform_preview: Vec<f32>,
}

impl VoiceLineRef {
    pub fn new(clip_id: String, file_path: String, speaker_id: String) -> Self {
        VoiceLineRef {
            clip_id,
            file_path,
            speaker_id,
            emotion: EmotionType::Neutral,
            duration_secs: 0.0,
            sample_rate: 44100,
            channels: 1,
            file_size_bytes: 0,
            lip_sync_data: None,
            subtitle_offset: 0.0,
            waveform_preview: Vec::new(),
        }
    }

    pub fn format_duration(&self) -> String {
        let mins = (self.duration_secs / 60.0) as u32;
        let secs = self.duration_secs % 60.0;
        format!("{}:{:05.2}", mins, secs)
    }

    pub fn generate_waveform_preview(&mut self, samples: usize) {
        // Generate a simulated waveform preview
        self.waveform_preview = (0..samples)
            .map(|i| {
                let t = i as f32 / samples as f32;
                let freq = 440.0_f32;
                (t * freq * std::f32::consts::TAU).sin() * (-t * 3.0).exp() * 0.5
            })
            .collect();
    }
}

#[derive(Debug, Clone)]
pub struct LipSyncData {
    pub phonemes: Vec<PhonemeKey>,
    pub visemes: Vec<VisemeKey>,
    pub format: LipSyncFormat,
}

#[derive(Debug, Clone)]
pub struct PhonemeKey {
    pub time: f32,
    pub phoneme: String,
    pub intensity: f32,
}

#[derive(Debug, Clone)]
pub struct VisemeKey {
    pub time: f32,
    pub viseme_id: u8,
    pub blend_weight: f32,
    pub duration: f32,
}

impl VisemeKey {
    pub fn viseme_name(&self) -> &str {
        match self.viseme_id {
            0 => "rest",
            1 => "PP",
            2 => "FF",
            3 => "TH",
            4 => "DD",
            5 => "kk",
            6 => "CH",
            7 => "SS",
            8 => "nn",
            9 => "RR",
            10 => "aa",
            11 => "E",
            12 => "I",
            13 => "O",
            14 => "U",
            _ => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LipSyncFormat {
    Custom,
    Oculus,
    PhonemesOnly,
    VisemesOnly,
}

#[derive(Debug, Clone)]
pub struct SubtitleCue {
    pub start_time: f32,
    pub end_time: f32,
    pub text: String,
}

impl SubtitleCue {
    pub fn duration(&self) -> f32 { self.end_time - self.start_time }
    pub fn is_active_at(&self, time: f32) -> bool {
        time >= self.start_time && time < self.end_time
    }
}

// ============================================================
// SECTION 7: EMOTION STATES
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum EmotionType {
    Happy,
    Sad,
    Angry,
    Surprised,
    Fearful,
    Disgusted,
    Neutral,
    Excited,
    Confused,
    Contemptuous,
    Anxious,
    Bored,
}

impl EmotionType {
    pub fn name(&self) -> &str {
        match self {
            EmotionType::Happy => "happy",
            EmotionType::Sad => "sad",
            EmotionType::Angry => "angry",
            EmotionType::Surprised => "surprised",
            EmotionType::Fearful => "fearful",
            EmotionType::Disgusted => "disgusted",
            EmotionType::Neutral => "neutral",
            EmotionType::Excited => "excited",
            EmotionType::Confused => "confused",
            EmotionType::Contemptuous => "contemptuous",
            EmotionType::Anxious => "anxious",
            EmotionType::Bored => "bored",
        }
    }

    pub fn all() -> Vec<EmotionType> {
        vec![
            EmotionType::Happy, EmotionType::Sad, EmotionType::Angry,
            EmotionType::Surprised, EmotionType::Fearful, EmotionType::Disgusted,
            EmotionType::Neutral, EmotionType::Excited, EmotionType::Confused,
            EmotionType::Contemptuous, EmotionType::Anxious, EmotionType::Bored,
        ]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "happy" => Some(EmotionType::Happy),
            "sad" => Some(EmotionType::Sad),
            "angry" => Some(EmotionType::Angry),
            "surprised" => Some(EmotionType::Surprised),
            "fearful" => Some(EmotionType::Fearful),
            "disgusted" => Some(EmotionType::Disgusted),
            "neutral" => Some(EmotionType::Neutral),
            "excited" => Some(EmotionType::Excited),
            "confused" => Some(EmotionType::Confused),
            "contemptuous" => Some(EmotionType::Contemptuous),
            "anxious" => Some(EmotionType::Anxious),
            "bored" => Some(EmotionType::Bored),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmotionState {
    pub primary: EmotionType,
    pub primary_intensity: f32,
    pub secondary: Option<EmotionType>,
    pub secondary_intensity: f32,
    pub blend_amount: f32,
    pub transition_duration: f32,
    pub valence: f32,   // -1 negative .. +1 positive
    pub arousal: f32,   // 0 calm .. 1 excited
}

impl Default for EmotionState {
    fn default() -> Self {
        EmotionState {
            primary: EmotionType::Neutral,
            primary_intensity: 1.0,
            secondary: None,
            secondary_intensity: 0.0,
            blend_amount: 0.0,
            transition_duration: 0.3,
            valence: 0.0,
            arousal: 0.0,
        }
    }
}

impl EmotionState {
    pub fn new(emotion: EmotionType, intensity: f32) -> Self {
        let (valence, arousal) = Self::emotion_to_va(&emotion);
        EmotionState {
            primary: emotion,
            primary_intensity: intensity,
            secondary: None,
            secondary_intensity: 0.0,
            blend_amount: 0.0,
            transition_duration: 0.3,
            valence: valence * intensity,
            arousal: arousal * intensity,
        }
    }

    pub fn emotion_to_va(emotion: &EmotionType) -> (f32, f32) {
        match emotion {
            EmotionType::Happy => (0.9, 0.6),
            EmotionType::Sad => (-0.7, -0.4),
            EmotionType::Angry => (-0.5, 0.9),
            EmotionType::Surprised => (0.1, 0.9),
            EmotionType::Fearful => (-0.8, 0.8),
            EmotionType::Disgusted => (-0.6, 0.2),
            EmotionType::Neutral => (0.0, 0.0),
            EmotionType::Excited => (0.8, 0.9),
            EmotionType::Confused => (-0.1, 0.4),
            EmotionType::Contemptuous => (-0.4, 0.3),
            EmotionType::Anxious => (-0.5, 0.7),
            EmotionType::Bored => (-0.2, -0.6),
        }
    }

    pub fn blend_towards(&self, target: &EmotionState, t: f32) -> EmotionState {
        let t = t.clamp(0.0, 1.0);
        EmotionState {
            primary: if t < 0.5 { self.primary.clone() } else { target.primary.clone() },
            primary_intensity: self.primary_intensity * (1.0 - t) + target.primary_intensity * t,
            secondary: if t > 0.3 { target.secondary.clone() } else { self.secondary.clone() },
            secondary_intensity: self.secondary_intensity * (1.0 - t) + target.secondary_intensity * t,
            blend_amount: t,
            transition_duration: self.transition_duration,
            valence: self.valence * (1.0 - t) + target.valence * t,
            arousal: self.arousal * (1.0 - t) + target.arousal * t,
        }
    }

    pub fn select_portrait(&self, speaker_id: &str, portrait_library: &PortraitLibrary) -> Option<String> {
        portrait_library.select_best(speaker_id, &self.primary, self.primary_intensity)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PortraitLibrary {
    pub portraits: HashMap<String, Vec<PortraitEntry>>,
}

#[derive(Debug, Clone)]
pub struct PortraitEntry {
    pub speaker_id: String,
    pub emotion: EmotionType,
    pub intensity_min: f32,
    pub intensity_max: f32,
    pub image_path: String,
    pub priority: i32,
}

impl PortraitLibrary {
    pub fn add_portrait(&mut self, entry: PortraitEntry) {
        self.portraits
            .entry(entry.speaker_id.clone())
            .or_default()
            .push(entry);
    }

    pub fn select_best(&self, speaker_id: &str, emotion: &EmotionType, intensity: f32) -> Option<String> {
        if let Some(entries) = self.portraits.get(speaker_id) {
            let mut candidates: Vec<&PortraitEntry> = entries.iter()
                .filter(|e| &e.emotion == emotion && intensity >= e.intensity_min && intensity <= e.intensity_max)
                .collect();
            if candidates.is_empty() {
                // Fallback to neutral
                candidates = entries.iter()
                    .filter(|e| e.emotion == EmotionType::Neutral)
                    .collect();
            }
            candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
            candidates.first().map(|e| e.image_path.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CameraHint {
    pub camera_angle: CameraAngle,
    pub focus_target: Option<String>,
    pub zoom_level: f32,
    pub dolly_offset: Vec3,
    pub look_at: Option<Vec3>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum CameraAngle {
    #[default]
    Standard,
    CloseUp,
    MidShot,
    WideShot,
    OverShoulder,
    Cutaway,
    TopDown,
    LowAngle,
}

// ============================================================
// SECTION 8: GRAPH EDITOR
// ============================================================

#[derive(Debug, Clone)]
pub struct GraphEditorState {
    pub pan_offset: Vec2,
    pub zoom: f32,
    pub canvas_size: Vec2,
    pub selected_nodes: HashSet<u64>,
    pub hovered_node: Option<u64>,
    pub dragging_node: Option<u64>,
    pub drag_start: Vec2,
    pub drag_node_start: Vec2,
    pub connection_drag: Option<ConnectionDrag>,
    pub selection_box: Option<SelectionBox>,
    pub grid_size: f32,
    pub snap_to_grid: bool,
    pub show_grid: bool,
    pub minimap_visible: bool,
    pub minimap_rect: (Vec2, Vec2),
    pub context_menu: Option<ContextMenuState>,
    pub clipboard: Vec<DialogueNode>,
    pub bezier_smoothness: f32,
}

impl Default for GraphEditorState {
    fn default() -> Self {
        GraphEditorState {
            pan_offset: Vec2::ZERO,
            zoom: 1.0,
            canvas_size: Vec2::new(1600.0, 900.0),
            selected_nodes: HashSet::new(),
            hovered_node: None,
            dragging_node: None,
            drag_start: Vec2::ZERO,
            drag_node_start: Vec2::ZERO,
            connection_drag: None,
            selection_box: None,
            grid_size: 20.0,
            snap_to_grid: true,
            show_grid: true,
            minimap_visible: true,
            minimap_rect: (Vec2::new(1400.0, 700.0), Vec2::new(190.0, 160.0)),
            context_menu: None,
            clipboard: Vec::new(),
            bezier_smoothness: 0.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionDrag {
    pub from_node: u64,
    pub from_port: usize,
    pub from_port_pos: Vec2,
    pub current_pos: Vec2,
    pub is_input: bool,
}

#[derive(Debug, Clone)]
pub struct SelectionBox {
    pub start: Vec2,
    pub end: Vec2,
}

impl SelectionBox {
    pub fn rect(&self) -> (Vec2, Vec2) {
        let min = Vec2::new(self.start.x.min(self.end.x), self.start.y.min(self.end.y));
        let max = Vec2::new(self.start.x.max(self.end.x), self.start.y.max(self.end.y));
        (min, max)
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        let (min, max) = self.rect();
        p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y
    }
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub position: Vec2,
    pub items: Vec<ContextMenuItem>,
    pub node_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub action: ContextMenuAction,
    pub enabled: bool,
    pub separator_before: bool,
}

#[derive(Debug, Clone)]
pub enum ContextMenuAction {
    AddNode(NodeType),
    DeleteNode,
    DuplicateNode,
    CopyNode,
    PasteNode,
    SelectAll,
    ClearSelection,
    AutoLayout,
    SetStartNode,
    EditComment,
    GoToNode(u64),
    CenterView,
}

impl GraphEditorState {
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2 {
        (world_pos + self.pan_offset) * self.zoom
    }

    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec2 {
        screen_pos / self.zoom - self.pan_offset
    }

    pub fn zoom_at(&mut self, screen_point: Vec2, delta: f32) {
        let old_world = self.screen_to_world(screen_point);
        self.zoom = (self.zoom * (1.0 + delta * 0.1)).clamp(0.1, 4.0);
        let new_world = self.screen_to_world(screen_point);
        self.pan_offset += new_world - old_world;
    }

    pub fn snap_to_grid(&self, pos: Vec2) -> Vec2 {
        if self.snap_to_grid {
            Vec2::new(
                (pos.x / self.grid_size).round() * self.grid_size,
                (pos.y / self.grid_size).round() * self.grid_size,
            )
        } else {
            pos
        }
    }

    pub fn center_on_node(&mut self, node_pos: Vec2) {
        self.pan_offset = self.canvas_size / 2.0 / self.zoom - node_pos;
    }

    pub fn get_node_rect(&self, node: &DialogueNode) -> (Vec2, Vec2) {
        let pos = node.position();
        let size = self.get_node_size(node);
        (pos, pos + size)
    }

    pub fn get_node_size(&self, node: &DialogueNode) -> Vec2 {
        match node.node_type() {
            NodeType::Speaker => Vec2::new(220.0, 100.0),
            NodeType::PlayerChoice => Vec2::new(220.0, 140.0),
            NodeType::Condition => Vec2::new(200.0, 80.0),
            NodeType::Start => Vec2::new(160.0, 60.0),
            NodeType::End => Vec2::new(160.0, 60.0),
            NodeType::Random => Vec2::new(180.0, 90.0),
            NodeType::Timed => Vec2::new(180.0, 90.0),
            _ => Vec2::new(200.0, 80.0),
        }
    }

    pub fn hit_test_node(&self, node: &DialogueNode, pos: Vec2) -> bool {
        let (min, max) = self.get_node_rect(node);
        pos.x >= min.x && pos.x <= max.x && pos.y >= min.y && pos.y <= max.y
    }

    pub fn get_port_position(&self, node: &DialogueNode, port: usize, is_input: bool) -> Vec2 {
        let (pos, size) = self.get_node_rect(node);
        let x = if is_input { pos.x } else { pos.x + size.x };
        let y_offset = if is_input {
            pos.y + size.y * 0.5
        } else {
            let num_outputs = self.count_outputs(node);
            if num_outputs <= 1 {
                pos.y + size.y * 0.5
            } else {
                let spacing = size.y / (num_outputs + 1) as f32;
                pos.y + spacing * (port + 1) as f32
            }
        };
        Vec2::new(x, y_offset)
    }

    fn count_outputs(&self, node: &DialogueNode) -> usize {
        match node {
            DialogueNode::Condition(_) => 2,
            DialogueNode::PlayerChoice(n) => n.choices.len(),
            DialogueNode::Random(n) => n.outputs.len(),
            DialogueNode::Timed(_) => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BezierCurve {
    pub p0: Vec2,
    pub p1: Vec2,
    pub p2: Vec2,
    pub p3: Vec2,
}

impl BezierCurve {
    pub fn new_connection(from: Vec2, to: Vec2, smoothness: f32) -> Self {
        let dist = (to.x - from.x).abs().max(50.0);
        let ctrl_dist = dist * smoothness;
        BezierCurve {
            p0: from,
            p1: Vec2::new(from.x + ctrl_dist, from.y),
            p2: Vec2::new(to.x - ctrl_dist, to.y),
            p3: to,
        }
    }

    pub fn sample(&self, t: f32) -> Vec2 {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;
        self.p0 * uuu
            + self.p1 * (3.0 * uu * t)
            + self.p2 * (3.0 * u * tt)
            + self.p3 * ttt
    }

    pub fn tangent_at(&self, t: f32) -> Vec2 {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;
        let dt_p0 = -3.0 * u * u;
        let dt_p1 = 3.0 * u * u - 6.0 * u * t;
        let dt_p2 = 6.0 * u * t - 3.0 * t * t;
        let dt_p3 = 3.0 * t * t;
        (self.p0 * dt_p0 + self.p1 * dt_p1 + self.p2 * dt_p2 + self.p3 * dt_p3).normalize_or_zero()
    }

    pub fn length_approx(&self, segments: usize) -> f32 {
        let mut len = 0.0_f32;
        let mut prev = self.p0;
        for i in 1..=segments {
            let t = i as f32 / segments as f32;
            let curr = self.sample(t);
            len += (curr - prev).length();
            prev = curr;
        }
        len
    }

    pub fn nearest_t(&self, point: Vec2, iterations: usize) -> f32 {
        let mut best_t = 0.0_f32;
        let mut best_dist = f32::MAX;
        for i in 0..=iterations {
            let t = i as f32 / iterations as f32;
            let p = self.sample(t);
            let d = (p - point).length_squared();
            if d < best_dist {
                best_dist = d;
                best_t = t;
            }
        }
        best_t
    }

    pub fn to_polyline(&self, segments: usize) -> Vec<Vec2> {
        (0..=segments)
            .map(|i| self.sample(i as f32 / segments as f32))
            .collect()
    }

    pub fn bounding_box(&self) -> (Vec2, Vec2) {
        let points = self.to_polyline(20);
        let min_x = points.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let min_y = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let max_x = points.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        let max_y = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        (Vec2::new(min_x, min_y), Vec2::new(max_x, max_y))
    }
}

#[derive(Debug, Clone)]
pub struct MinimapState {
    pub visible: bool,
    pub position: Vec2,
    pub size: Vec2,
    pub world_bounds: (Vec2, Vec2),
    pub viewport_rect: (Vec2, Vec2),
}

impl MinimapState {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        MinimapState {
            visible: true,
            position,
            size,
            world_bounds: (Vec2::ZERO, Vec2::new(2000.0, 1500.0)),
            viewport_rect: (Vec2::ZERO, Vec2::new(1600.0, 900.0)),
        }
    }

    pub fn world_to_minimap(&self, world_pos: Vec2) -> Vec2 {
        let (wmin, wmax) = self.world_bounds;
        let wsize = wmax - wmin;
        let t = (world_pos - wmin) / wsize;
        self.position + t * self.size
    }

    pub fn minimap_to_world(&self, minimap_pos: Vec2) -> Vec2 {
        let t = (minimap_pos - self.position) / self.size;
        let (wmin, wmax) = self.world_bounds;
        let wsize = wmax - wmin;
        wmin + t * wsize
    }

    pub fn update_world_bounds(&mut self, nodes: &HashMap<u64, DialogueNode>) {
        if nodes.is_empty() {
            self.world_bounds = (Vec2::new(-100.0, -100.0), Vec2::new(100.0, 100.0));
            return;
        }
        let mut min = Vec2::new(f32::MAX, f32::MAX);
        let mut max = Vec2::new(f32::MIN, f32::MIN);
        for node in nodes.values() {
            let pos = node.position();
            min = Vec2::new(min.x.min(pos.x), min.y.min(pos.y));
            max = Vec2::new(max.x.max(pos.x + 220.0), max.y.max(pos.y + 140.0));
        }
        let padding = 50.0;
        self.world_bounds = (min - padding, max + padding);
    }
}

// ============================================================
// SECTION 9: DIALOGUE PLAYER/PREVIEWER
// ============================================================

#[derive(Debug, Clone)]
pub struct DialoguePlayer {
    pub tree: DialogueTree,
    pub current_node_id: Option<u64>,
    pub history: Vec<HistoryEntry>,
    pub scope: VariableScope,
    pub state: PlayerState,
    pub pending_events: VecDeque<GameEvent>,
    pub current_time: f32,
    pub choice_timeout_remaining: Option<f32>,
    pub watch_variables: Vec<WatchVariable>,
    pub playback_speed: f32,
    pub auto_advance: bool,
    pub locale: String,
    pub loc_table: LocalizationTable,
    pub portrait_library: PortraitLibrary,
    pub jump_return_stack: Vec<(String, u64)>,
    pub step_count: usize,
    pub max_steps: usize,
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub node_id: u64,
    pub node_type: NodeType,
    pub text: Option<String>,
    pub speaker: Option<String>,
    pub choice_made: Option<usize>,
    pub timestamp: f32,
    pub variables_snapshot: HashMap<String, DialogueValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerState {
    Idle,
    Running,
    WaitingForChoice,
    WaitingForAdvance,
    WaitingForTimer,
    Ended(EndType),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct WatchVariable {
    pub name: String,
    pub scope: ScopeType,
    pub display_name: String,
    pub watch_for_changes: bool,
    pub last_value: Option<DialogueValue>,
    pub changed_this_step: bool,
}

impl WatchVariable {
    pub fn new(name: &str, scope: ScopeType) -> Self {
        WatchVariable {
            name: name.to_string(),
            scope,
            display_name: name.to_string(),
            watch_for_changes: true,
            last_value: None,
            changed_this_step: false,
        }
    }

    pub fn update(&mut self, scope: &VariableScope) {
        let current = scope.get(&self.name, &self.scope);
        self.changed_this_step = current != self.last_value;
        self.last_value = current;
    }
}

impl DialoguePlayer {
    pub fn new(tree: DialogueTree) -> Self {
        let locale = "en".to_string();
        let loc_table = LocalizationTable::new("en");
        DialoguePlayer {
            tree,
            current_node_id: None,
            history: Vec::new(),
            scope: VariableScope::new(),
            state: PlayerState::Idle,
            pending_events: VecDeque::new(),
            current_time: 0.0,
            choice_timeout_remaining: None,
            watch_variables: Vec::new(),
            playback_speed: 1.0,
            auto_advance: true,
            locale,
            loc_table,
            portrait_library: PortraitLibrary::default(),
            jump_return_stack: Vec::new(),
            step_count: 0,
            max_steps: 10000,
        }
    }

    pub fn start(&mut self) -> PlayResult {
        self.scope.clear_local();
        self.history.clear();
        self.pending_events.clear();
        self.step_count = 0;
        if let Some(start_id) = self.tree.start_node_id {
            self.current_node_id = Some(start_id);
            self.state = PlayerState::Running;
            self.process_current_node()
        } else {
            self.state = PlayerState::Error("No start node".to_string());
            PlayResult::Error("No start node".to_string())
        }
    }

    pub fn step(&mut self, choice: Option<usize>) -> PlayResult {
        self.step_count += 1;
        if self.step_count > self.max_steps {
            self.state = PlayerState::Error("Max steps exceeded".to_string());
            return PlayResult::Error("Infinite loop detected".to_string());
        }
        match &self.state.clone() {
            PlayerState::WaitingForChoice => {
                if let Some(idx) = choice {
                    self.make_choice(idx)
                } else {
                    PlayResult::NeedChoice
                }
            }
            PlayerState::WaitingForAdvance => {
                if let Some(node_id) = self.current_node_id {
                    if let Some(next) = self.tree.next_node(node_id, 0) {
                        self.current_node_id = Some(next);
                        self.process_current_node()
                    } else {
                        self.state = PlayerState::Ended(EndType::Normal);
                        PlayResult::Ended
                    }
                } else {
                    PlayResult::Ended
                }
            }
            PlayerState::Running => self.process_current_node(),
            PlayerState::Ended(_) => PlayResult::Ended,
            PlayerState::Error(msg) => PlayResult::Error(msg.clone()),
            _ => PlayResult::NeedAdvance,
        }
    }

    fn process_current_node(&mut self) -> PlayResult {
        let node_id = match self.current_node_id {
            Some(id) => id,
            None => return PlayResult::Ended,
        };
        let node = match self.tree.nodes.get(&node_id).cloned() {
            Some(n) => n,
            None => return PlayResult::Error(format!("Node {} not found", node_id)),
        };
        // Update watch variables
        for watch in &mut self.watch_variables {
            let scope = &self.scope;
            watch.update(scope);
        }
        match &node {
            DialogueNode::Start(n) => {
                // Initialize variables
                for (k, v) in &n.initial_variables {
                    self.scope.set(k.clone(), v.scope.clone(), v.value.clone());
                }
                for event_name in &n.on_enter_events {
                    self.pending_events.push_back(GameEvent {
                        name: event_name.clone(),
                        parameters: HashMap::new(),
                        delay: 0.0,
                        source: format!("start_{}", n.id),
                    });
                }
                self.record_history(node_id, NodeType::Start, None, None, None);
                if let Some(next) = self.tree.next_node(node_id, 0) {
                    self.current_node_id = Some(next);
                    self.process_current_node()
                } else {
                    self.state = PlayerState::Ended(EndType::Normal);
                    PlayResult::Ended
                }
            }
            DialogueNode::Speaker(n) => {
                let text = n.get_display_text(&self.locale, &self.loc_table);
                let speaker = n.speaker_name.clone();
                let emotion = n.emotion_state.clone();
                let portrait = self.portrait_library.select_best(&n.speaker_id, &emotion.primary, emotion.primary_intensity);
                self.record_history(node_id, NodeType::Speaker, Some(text.clone()), Some(speaker.clone()), None);
                self.state = PlayerState::WaitingForAdvance;
                PlayResult::ShowDialogue {
                    speaker,
                    text,
                    portrait,
                    emotion,
                    voice_line: n.voice_line_ref.clone(),
                    subtitles: n.subtitle_timing.clone(),
                    auto_advance: n.auto_advance,
                    auto_advance_delay: n.auto_advance_delay,
                }
            }
            DialogueNode::PlayerChoice(n) => {
                let available = n.get_available_choices(&self.scope);
                if available.is_empty() {
                    // No choices available, skip if possible
                    if let Some(next) = self.tree.next_node(node_id, 0) {
                        self.current_node_id = Some(next);
                        return self.process_current_node();
                    } else {
                        self.state = PlayerState::Ended(EndType::Normal);
                        return PlayResult::Ended;
                    }
                }
                let choices_display: Vec<ChoiceDisplay> = available.iter().map(|(i, c)| ChoiceDisplay {
                    index: *i,
                    text: c.text.clone(),
                    available: true,
                    tags: c.tags.clone(),
                    tooltip: c.tooltip.clone(),
                }).collect();
                self.state = PlayerState::WaitingForChoice;
                self.choice_timeout_remaining = n.timeout_secs;
                self.record_history(node_id, NodeType::PlayerChoice, Some(n.prompt_text.clone()), None, None);
                PlayResult::ShowChoices {
                    prompt: n.prompt_text.clone(),
                    choices: choices_display,
                    timeout: n.timeout_secs,
                }
            }
            DialogueNode::Condition(n) => {
                let next_id = n.evaluate(&self.scope);
                self.record_history(node_id, NodeType::Condition, None, None, None);
                self.current_node_id = Some(next_id);
                self.process_current_node()
            }
            DialogueNode::SetVariable(n) => {
                n.execute(&mut self.scope);
                self.record_history(node_id, NodeType::SetVariable, None, None, None);
                if let Some(next) = self.tree.next_node(node_id, 0) {
                    self.current_node_id = Some(next);
                    self.process_current_node()
                } else {
                    self.state = PlayerState::Ended(EndType::Normal);
                    PlayResult::Ended
                }
            }
            DialogueNode::TriggerEvent(n) => {
                self.pending_events.push_back(n.build_event());
                self.record_history(node_id, NodeType::TriggerEvent, None, None, None);
                if let Some(next) = self.tree.next_node(node_id, 0) {
                    self.current_node_id = Some(next);
                    self.process_current_node()
                } else {
                    self.state = PlayerState::Ended(EndType::Normal);
                    PlayResult::Ended
                }
            }
            DialogueNode::Jump(n) => {
                if n.return_after {
                    self.jump_return_stack.push((self.tree.id.clone(), node_id));
                }
                self.record_history(node_id, NodeType::Jump, None, None, None);
                // For preview, we just go to target node in same tree
                self.current_node_id = Some(n.target_node_id);
                self.process_current_node()
            }
            DialogueNode::End(n) => {
                for event_name in &n.on_end_events {
                    self.pending_events.push_back(GameEvent {
                        name: event_name.clone(),
                        parameters: HashMap::new(),
                        delay: 0.0,
                        source: format!("end_{}", n.id),
                    });
                }
                self.record_history(node_id, NodeType::End, None, None, None);
                self.state = PlayerState::Ended(n.end_type.clone());
                PlayResult::Ended
            }
            DialogueNode::Random(n) => {
                let mut n_clone = n.clone();
                let next_id = n_clone.pick_output().unwrap_or(0);
                // Update the node's rng state
                if let Some(DialogueNode::Random(node)) = self.tree.nodes.get_mut(&node_id) {
                    node.rng_state = n_clone.rng_state;
                }
                self.record_history(node_id, NodeType::Random, None, None, None);
                self.current_node_id = Some(next_id);
                self.process_current_node()
            }
            DialogueNode::Timed(n) => {
                self.state = PlayerState::WaitingForTimer;
                PlayResult::WaitTimer {
                    duration: n.duration_secs,
                    label: n.timer_label.clone(),
                    show_timer: n.show_timer,
                }
            }
        }
    }

    fn make_choice(&mut self, choice_idx: usize) -> PlayResult {
        let node_id = match self.current_node_id {
            Some(id) => id,
            None => return PlayResult::Ended,
        };
        if let Some(DialogueNode::PlayerChoice(n)) = self.tree.nodes.get_mut(&node_id) {
            if choice_idx < n.choices.len() {
                let choice = &mut n.choices[choice_idx];
                choice.used = true;
                let next_node = choice.output_node;
                self.record_history(node_id, NodeType::PlayerChoice, None, None, Some(choice_idx));
                self.current_node_id = Some(next_node);
                self.state = PlayerState::Running;
                return self.process_current_node();
            }
        }
        PlayResult::Error(format!("Invalid choice index: {}", choice_idx))
    }

    fn record_history(&mut self, node_id: u64, node_type: NodeType, text: Option<String>, speaker: Option<String>, choice: Option<usize>) {
        let snapshot: HashMap<String, DialogueValue> = self.scope.local.clone()
            .into_iter()
            .chain(self.scope.global.clone())
            .collect();
        self.history.push(HistoryEntry {
            node_id,
            node_type,
            text,
            speaker,
            choice_made: choice,
            timestamp: self.current_time,
            variables_snapshot: snapshot,
        });
    }

    pub fn update_timer(&mut self, delta: f32) -> PlayResult {
        if self.state != PlayerState::WaitingForTimer { return PlayResult::NeedAdvance; }
        let node_id = match self.current_node_id {
            Some(id) => id,
            None => return PlayResult::Ended,
        };
        if let Some(DialogueNode::Timed(n)) = self.tree.nodes.get_mut(&node_id) {
            match n.update(delta * self.playback_speed) {
                TimedNodeResult::Timeout(next_id) => {
                    self.state = PlayerState::Running;
                    self.current_node_id = Some(next_id);
                    self.process_current_node()
                }
                TimedNodeResult::Running(progress) => PlayResult::TimerProgress(progress),
                TimedNodeResult::Complete(next_id) => {
                    self.state = PlayerState::Running;
                    self.current_node_id = Some(next_id);
                    self.process_current_node()
                }
            }
        } else {
            PlayResult::Error("Timer node not found".to_string())
        }
    }

    pub fn restart(&mut self) -> PlayResult {
        self.scope.clear_local();
        self.history.clear();
        self.pending_events.clear();
        self.step_count = 0;
        self.state = PlayerState::Idle;
        self.start()
    }

    pub fn get_history_display(&self) -> Vec<String> {
        self.history.iter().map(|h| {
            match &h.node_type {
                NodeType::Speaker => format!("[{:.1}s] {}: {}", h.timestamp,
                    h.speaker.as_deref().unwrap_or("?"),
                    h.text.as_deref().unwrap_or("")),
                NodeType::PlayerChoice => format!("[{:.1}s] Choice #{}", h.timestamp,
                    h.choice_made.map(|i| i.to_string()).unwrap_or_else(|| "?".to_string())),
                nt => format!("[{:.1}s] {:?}", h.timestamp, nt),
            }
        }).collect()
    }

    pub fn add_watch(&mut self, name: &str, scope: ScopeType) {
        if !self.watch_variables.iter().any(|w| w.name == name) {
            self.watch_variables.push(WatchVariable::new(name, scope));
        }
    }

    pub fn remove_watch(&mut self, name: &str) {
        self.watch_variables.retain(|w| w.name != name);
    }

    pub fn get_pending_events(&mut self) -> Vec<GameEvent> {
        self.pending_events.drain(..).collect()
    }

    pub fn simulate_condition(&self, expr: &ConditionExpression) -> bool {
        expr.evaluate(&self.scope)
    }
}

#[derive(Debug, Clone)]
pub enum PlayResult {
    ShowDialogue {
        speaker: String,
        text: String,
        portrait: Option<String>,
        emotion: EmotionState,
        voice_line: Option<VoiceLineRef>,
        subtitles: Vec<SubtitleCue>,
        auto_advance: bool,
        auto_advance_delay: f32,
    },
    ShowChoices {
        prompt: String,
        choices: Vec<ChoiceDisplay>,
        timeout: Option<f32>,
    },
    WaitTimer {
        duration: f32,
        label: String,
        show_timer: bool,
    },
    TimerProgress(f32),
    NeedChoice,
    NeedAdvance,
    Ended,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ChoiceDisplay {
    pub index: usize,
    pub text: String,
    pub available: bool,
    pub tags: Vec<String>,
    pub tooltip: Option<String>,
}

// ============================================================
// SECTION 10: FULL DIALOGUE EDITOR
// ============================================================

#[derive(Debug, Clone)]
pub struct UndoHistory {
    pub undo_stack: VecDeque<EditorAction>,
    pub redo_stack: VecDeque<EditorAction>,
    pub max_history: usize,
}

impl UndoHistory {
    pub fn new(max_history: usize) -> Self {
        UndoHistory {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_history,
        }
    }

    pub fn push(&mut self, action: EditorAction) {
        self.redo_stack.clear();
        self.undo_stack.push_back(action);
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.pop_front();
        }
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    pub fn undo(&mut self) -> Option<EditorAction> {
        if let Some(action) = self.undo_stack.pop_back() {
            self.redo_stack.push_back(action.clone());
            Some(action)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<EditorAction> {
        if let Some(action) = self.redo_stack.pop_back() {
            self.undo_stack.push_back(action.clone());
            Some(action)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum EditorAction {
    AddNode { node: DialogueNode },
    RemoveNode { node: DialogueNode, connections: Vec<NodeConnection> },
    MoveNode { node_id: u64, old_pos: Vec2, new_pos: Vec2 },
    AddConnection { connection: NodeConnection },
    RemoveConnection { connection: NodeConnection },
    EditNodeText { node_id: u64, old_text: String, new_text: String },
    EditNodeSpeaker { node_id: u64, old_speaker: String, new_speaker: String },
    AddChoice { node_id: u64, choice: PlayerChoice },
    RemoveChoice { node_id: u64, choice_index: usize, choice: PlayerChoice },
    EditChoice { node_id: u64, choice_index: usize, old: PlayerChoice, new: PlayerChoice },
    AddVariable { variable: DialogueVariable },
    EditCondition { node_id: u64, old: ConditionExpression, new: ConditionExpression },
    SetStartNode { old: Option<u64>, new: u64 },
    PasteNodes { nodes: Vec<DialogueNode>, connections: Vec<NodeConnection> },
    BatchAction(Vec<EditorAction>),
}

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub current_result: usize,
    pub search_in_text: bool,
    pub search_in_comments: bool,
    pub search_in_speakers: bool,
    pub search_in_tags: bool,
    pub case_sensitive: bool,
    pub regex_mode: bool,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub node_id: u64,
    pub field: String,
    pub excerpt: String,
    pub match_start: usize,
    pub match_len: usize,
}

impl SearchState {
    pub fn new() -> Self { Self::default() }

    pub fn search(&mut self, tree: &DialogueTree) {
        self.results.clear();
        if self.query.is_empty() { return; }
        let query_lower = if self.case_sensitive { self.query.clone() } else { self.query.to_lowercase() };
        for (id, node) in &tree.nodes {
            match node {
                DialogueNode::Speaker(n) => {
                    if self.search_in_text {
                        let text = if self.case_sensitive { n.dialogue_text.clone() } else { n.dialogue_text.to_lowercase() };
                        if let Some(pos) = text.find(&query_lower) {
                            self.results.push(SearchResult {
                                node_id: *id,
                                field: "dialogue_text".into(),
                                excerpt: n.dialogue_text.chars().take(80).collect(),
                                match_start: pos,
                                match_len: query_lower.len(),
                            });
                        }
                    }
                    if self.search_in_speakers {
                        let name = if self.case_sensitive { n.speaker_name.clone() } else { n.speaker_name.to_lowercase() };
                        if name.contains(&query_lower) {
                            self.results.push(SearchResult {
                                node_id: *id,
                                field: "speaker_name".into(),
                                excerpt: n.speaker_name.clone(),
                                match_start: 0,
                                match_len: n.speaker_name.len(),
                            });
                        }
                    }
                    if self.search_in_comments {
                        let comment = if self.case_sensitive { n.comment.clone() } else { n.comment.to_lowercase() };
                        if let Some(pos) = comment.find(&query_lower) {
                            self.results.push(SearchResult {
                                node_id: *id,
                                field: "comment".into(),
                                excerpt: n.comment.chars().take(80).collect(),
                                match_start: pos,
                                match_len: query_lower.len(),
                            });
                        }
                    }
                }
                DialogueNode::PlayerChoice(n) => {
                    for (ci, choice) in n.choices.iter().enumerate() {
                        if self.search_in_text {
                            let text = if self.case_sensitive { choice.text.clone() } else { choice.text.to_lowercase() };
                            if let Some(pos) = text.find(&query_lower) {
                                self.results.push(SearchResult {
                                    node_id: *id,
                                    field: format!("choice_{}", ci),
                                    excerpt: choice.text.chars().take(80).collect(),
                                    match_start: pos,
                                    match_len: query_lower.len(),
                                });
                            }
                        }
                    }
                }
                _ => {
                    if self.search_in_tags {
                        for tag in node.tags() {
                            let t = if self.case_sensitive { tag.clone() } else { tag.to_lowercase() };
                            if t.contains(&query_lower) {
                                self.results.push(SearchResult {
                                    node_id: *id,
                                    field: "tag".into(),
                                    excerpt: tag.clone(),
                                    match_start: 0,
                                    match_len: tag.len(),
                                });
                            }
                        }
                    }
                }
            }
        }
        self.current_result = 0;
    }

    pub fn next_result(&mut self) {
        if !self.results.is_empty() {
            self.current_result = (self.current_result + 1) % self.results.len();
        }
    }

    pub fn prev_result(&mut self) {
        if !self.results.is_empty() {
            self.current_result = (self.current_result + self.results.len() - 1) % self.results.len();
        }
    }

    pub fn current(&self) -> Option<&SearchResult> {
        self.results.get(self.current_result)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImportExportOptions {
    pub format: ExportFormat,
    pub include_metadata: bool,
    pub include_voice_refs: bool,
    pub include_conditions: bool,
    pub minify: bool,
    pub output_path: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum ExportFormat {
    #[default]
    Json,
    Csv,
    Custom,
    InkScript,
    TwineCompatible,
}

#[derive(Debug, Clone)]
pub struct SpeakerDatabase {
    pub speakers: HashMap<String, SpeakerEntry>,
}

#[derive(Debug, Clone)]
pub struct SpeakerEntry {
    pub id: String,
    pub display_name: String,
    pub default_emotion: EmotionState,
    pub voice_actor: String,
    pub portrait_path: String,
    pub color_tag: Vec4,
    pub faction: String,
    pub is_player: bool,
    pub aliases: Vec<String>,
}

impl SpeakerDatabase {
    pub fn new() -> Self {
        SpeakerDatabase { speakers: HashMap::new() }
    }

    pub fn add(&mut self, entry: SpeakerEntry) {
        self.speakers.insert(entry.id.clone(), entry);
    }

    pub fn get(&self, id: &str) -> Option<&SpeakerEntry> {
        self.speakers.get(id)
    }

    pub fn get_display_name(&self, id: &str) -> String {
        self.speakers.get(id)
            .map(|s| s.display_name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    pub fn get_all_ids(&self) -> Vec<&str> {
        self.speakers.keys().map(|s| s.as_str()).collect()
    }
}

#[derive(Debug)]
pub struct DialogueEditor {
    pub trees: HashMap<String, DialogueTree>,
    pub active_tree_id: Option<String>,
    pub graph_state: GraphEditorState,
    pub minimap: MinimapState,
    pub player: Option<DialoguePlayer>,
    pub undo_history: UndoHistory,
    pub search: SearchState,
    pub loc_table: LocalizationTable,
    pub speaker_db: SpeakerDatabase,
    pub portrait_library: PortraitLibrary,
    pub export_options: ImportExportOptions,
    pub show_player: bool,
    pub show_variables: bool,
    pub show_validation: bool,
    pub show_search: bool,
    pub show_localization: bool,
    pub show_voice_browser: bool,
    pub validation_errors: Vec<ValidationError>,
    pub selected_speaker_id: Option<String>,
    pub node_id_counter: u64,
    pub dirty: bool,
    pub last_save_path: Option<String>,
    pub status_message: String,
    pub status_timer: f32,
    pub recent_files: Vec<String>,
    pub panel_widths: PanelWidths,
}

#[derive(Debug, Clone)]
pub struct PanelWidths {
    pub left_panel: f32,
    pub right_panel: f32,
    pub bottom_panel: f32,
    pub properties_panel: f32,
}

impl Default for PanelWidths {
    fn default() -> Self {
        PanelWidths {
            left_panel: 240.0,
            right_panel: 300.0,
            bottom_panel: 200.0,
            properties_panel: 320.0,
        }
    }
}

impl DialogueEditor {
    pub fn new() -> Self {
        let mut loc_table = LocalizationTable::new("en");
        loc_table.add_locale("fr");
        loc_table.add_locale("de");
        loc_table.add_locale("es");
        loc_table.add_locale("ja");
        DialogueEditor {
            trees: HashMap::new(),
            active_tree_id: None,
            graph_state: GraphEditorState::default(),
            minimap: MinimapState::new(Vec2::new(1400.0, 700.0), Vec2::new(190.0, 160.0)),
            player: None,
            undo_history: UndoHistory::new(100),
            search: SearchState::new(),
            loc_table,
            speaker_db: SpeakerDatabase::new(),
            portrait_library: PortraitLibrary::default(),
            export_options: ImportExportOptions::default(),
            show_player: false,
            show_variables: true,
            show_validation: false,
            show_search: false,
            show_localization: false,
            show_voice_browser: false,
            validation_errors: Vec::new(),
            selected_speaker_id: None,
            node_id_counter: 1000,
            dirty: false,
            last_save_path: None,
            status_message: String::from("Ready"),
            status_timer: 0.0,
            recent_files: Vec::new(),
            panel_widths: PanelWidths::default(),
        }
    }

    pub fn allocate_node_id(&mut self) -> u64 {
        self.node_id_counter += 1;
        self.node_id_counter
    }

    pub fn active_tree(&self) -> Option<&DialogueTree> {
        self.active_tree_id.as_ref().and_then(|id| self.trees.get(id))
    }

    pub fn active_tree_mut(&mut self) -> Option<&mut DialogueTree> {
        self.active_tree_id.as_ref().and_then(|id| self.trees.get_mut(id))
    }

    pub fn new_tree(&mut self, name: &str) -> String {
        let id = format!("tree_{}", self.allocate_node_id());
        let mut tree = DialogueTree::new(id.clone(), name.to_string());
        // Add default start node
        let start_id = self.node_id_counter + 1;
        self.node_id_counter += 1;
        let mut start = StartNode::new(start_id);
        start.position = Vec2::new(100.0, 300.0);
        tree.add_node(DialogueNode::Start(start));
        self.trees.insert(id.clone(), tree);
        self.active_tree_id = Some(id.clone());
        self.dirty = true;
        id
    }

    pub fn open_tree(&mut self, id: &str) {
        if self.trees.contains_key(id) {
            self.active_tree_id = Some(id.to_string());
            self.validate_active_tree();
        }
    }

    pub fn close_tree(&mut self, id: &str) {
        self.trees.remove(id);
        if self.active_tree_id.as_deref() == Some(id) {
            self.active_tree_id = self.trees.keys().next().cloned();
        }
    }

    pub fn add_node(&mut self, node_type: NodeType, position: Vec2) -> Option<u64> {
        let id = self.allocate_node_id();
        let node = self.create_node(node_type, id, position);
        let nid = node.id();
        self.undo_history.push(EditorAction::AddNode { node: node.clone() });
        if let Some(tree) = self.active_tree_mut() {
            tree.add_node(node);
            self.dirty = true;
            Some(nid)
        } else {
            None
        }
    }

    fn create_node(&self, node_type: NodeType, id: u64, position: Vec2) -> DialogueNode {
        match node_type {
            NodeType::Start => {
                let mut n = StartNode::new(id);
                n.position = position;
                DialogueNode::Start(n)
            }
            NodeType::Speaker => {
                let mut n = SpeakerNode::new(id);
                n.position = position;
                DialogueNode::Speaker(n)
            }
            NodeType::PlayerChoice => {
                let mut n = PlayerChoiceNode::new(id);
                n.position = position;
                DialogueNode::PlayerChoice(n)
            }
            NodeType::Condition => {
                let mut n = ConditionNode::new(id);
                n.position = position;
                DialogueNode::Condition(n)
            }
            NodeType::SetVariable => {
                let mut n = SetVariableNode::new(id);
                n.position = position;
                DialogueNode::SetVariable(n)
            }
            NodeType::TriggerEvent => {
                let mut n = TriggerEventNode::new(id);
                n.position = position;
                DialogueNode::TriggerEvent(n)
            }
            NodeType::Jump => {
                let mut n = JumpNode::new(id);
                n.position = position;
                DialogueNode::Jump(n)
            }
            NodeType::End => {
                let mut n = EndNode::new(id);
                n.position = position;
                DialogueNode::End(n)
            }
            NodeType::Random => {
                let mut n = RandomNode::new(id);
                n.position = position;
                DialogueNode::Random(n)
            }
            NodeType::Timed => {
                let mut n = TimedNode::new(id);
                n.position = position;
                DialogueNode::Timed(n)
            }
        }
    }

    pub fn remove_selected_nodes(&mut self) {
        let selected: Vec<u64> = self.graph_state.selected_nodes.iter().cloned().collect();
        for id in &selected {
            let undo_entry = if let Some(tree) = self.active_tree() {
                if let Some(node) = tree.nodes.get(id).cloned() {
                    let connections: Vec<NodeConnection> = tree.connections.iter()
                        .filter(|c| c.from_node == *id || c.to_node == *id)
                        .cloned()
                        .collect();
                    Some(EditorAction::RemoveNode { node, connections })
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(action) = undo_entry {
                self.undo_history.push(action);
            }
            if let Some(tree) = self.active_tree_mut() {
                tree.remove_node(*id);
            }
        }
        self.graph_state.selected_nodes.clear();
        self.dirty = true;
    }

    pub fn connect_nodes(&mut self, from: u64, from_port: usize, to: u64, to_port: usize) {
        let conn = NodeConnection { from_node: from, from_port, to_node: to, to_port, label: None };
        self.undo_history.push(EditorAction::AddConnection { connection: conn.clone() });
        if let Some(tree) = self.active_tree_mut() {
            tree.connect(from, from_port, to, to_port, None);
            self.dirty = true;
        }
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_history.undo() {
            self.apply_undo_action(action);
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.undo_history.redo() {
            self.apply_redo_action(action);
        }
    }

    fn apply_undo_action(&mut self, action: EditorAction) {
        match action {
            EditorAction::AddNode { node } => {
                if let Some(tree) = self.active_tree_mut() {
                    tree.remove_node(node.id());
                }
            }
            EditorAction::RemoveNode { node, connections } => {
                if let Some(tree) = self.active_tree_mut() {
                    let id = node.id();
                    tree.add_node(node);
                    for conn in connections {
                        tree.connections.push(conn);
                    }
                }
            }
            EditorAction::MoveNode { node_id, old_pos, new_pos: _ } => {
                if let Some(tree) = self.active_tree_mut() {
                    if let Some(node) = tree.nodes.get_mut(&node_id) {
                        node.set_position(old_pos);
                    }
                }
            }
            EditorAction::AddConnection { connection } => {
                if let Some(tree) = self.active_tree_mut() {
                    tree.disconnect(connection.from_node, connection.from_port);
                }
            }
            EditorAction::RemoveConnection { connection } => {
                if let Some(tree) = self.active_tree_mut() {
                    tree.connect(connection.from_node, connection.from_port,
                        connection.to_node, connection.to_port, connection.label);
                }
            }
            EditorAction::EditNodeText { node_id, old_text, new_text: _ } => {
                if let Some(tree) = self.active_tree_mut() {
                    if let Some(DialogueNode::Speaker(n)) = tree.nodes.get_mut(&node_id) {
                        n.dialogue_text = old_text;
                    }
                }
            }
            EditorAction::BatchAction(actions) => {
                for action in actions.into_iter().rev() {
                    self.apply_undo_action(action);
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn apply_redo_action(&mut self, action: EditorAction) {
        match action {
            EditorAction::AddNode { node } => {
                if let Some(tree) = self.active_tree_mut() {
                    tree.add_node(node);
                }
            }
            EditorAction::RemoveNode { node, connections: _ } => {
                if let Some(tree) = self.active_tree_mut() {
                    tree.remove_node(node.id());
                }
            }
            EditorAction::MoveNode { node_id, old_pos: _, new_pos } => {
                if let Some(tree) = self.active_tree_mut() {
                    if let Some(node) = tree.nodes.get_mut(&node_id) {
                        node.set_position(new_pos);
                    }
                }
            }
            EditorAction::AddConnection { connection } => {
                if let Some(tree) = self.active_tree_mut() {
                    tree.connect(connection.from_node, connection.from_port,
                        connection.to_node, connection.to_port, connection.label);
                }
            }
            EditorAction::RemoveConnection { connection } => {
                if let Some(tree) = self.active_tree_mut() {
                    tree.disconnect(connection.from_node, connection.from_port);
                }
            }
            EditorAction::EditNodeText { node_id, old_text: _, new_text } => {
                if let Some(tree) = self.active_tree_mut() {
                    if let Some(DialogueNode::Speaker(n)) = tree.nodes.get_mut(&node_id) {
                        n.dialogue_text = new_text;
                    }
                }
            }
            EditorAction::BatchAction(actions) => {
                for action in actions {
                    self.apply_redo_action(action);
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    pub fn validate_active_tree(&mut self) {
        if let Some(tree) = self.active_tree() {
            self.validation_errors = tree.validate();
        }
    }

    pub fn search_in_active_tree(&mut self, query: &str) {
        self.search.query = query.to_string();
        if let Some(tree) = self.active_tree().cloned() {
            self.search.search(&tree);
        }
    }

    pub fn start_preview(&mut self) {
        if let Some(tree) = self.active_tree().cloned() {
            let mut player = DialoguePlayer::new(tree);
            player.loc_table = self.loc_table.clone();
            player.portrait_library = self.portrait_library.clone();
            player.start();
            self.player = Some(player);
            self.show_player = true;
        }
    }

    pub fn stop_preview(&mut self) {
        self.player = None;
        self.show_player = false;
    }

    pub fn update(&mut self, delta: f32) {
        self.status_timer -= delta;
        if self.status_timer < 0.0 {
            self.status_timer = 0.0;
        }
        if let Some(player) = &mut self.player {
            if player.state == PlayerState::WaitingForTimer {
                player.update_timer(delta);
            }
            player.current_time += delta;
        }
    }

    pub fn set_status(&mut self, msg: &str, duration: f32) {
        self.status_message = msg.to_string();
        self.status_timer = duration;
    }

    pub fn copy_selected_nodes(&mut self) {
        if let Some(tree) = self.active_tree() {
            let nodes: Vec<DialogueNode> = self.graph_state.selected_nodes.iter()
                .filter_map(|id| tree.nodes.get(id).cloned())
                .collect();
            self.graph_state.clipboard = nodes;
        }
    }

    pub fn paste_nodes(&mut self, offset: Vec2) {
        if self.graph_state.clipboard.is_empty() { return; }
        let clipboard = self.graph_state.clipboard.clone();
        let mut new_nodes = Vec::new();
        let mut id_map: HashMap<u64, u64> = HashMap::new();
        for node in &clipboard {
            let new_id = self.allocate_node_id();
            id_map.insert(node.id(), new_id);
            let mut new_node = node.clone();
            let pos = new_node.position() + offset;
            new_node.set_position(pos);
            match &mut new_node {
                DialogueNode::Start(n) => n.id = new_id,
                DialogueNode::Speaker(n) => n.id = new_id,
                DialogueNode::PlayerChoice(n) => n.id = new_id,
                DialogueNode::Condition(n) => n.id = new_id,
                DialogueNode::SetVariable(n) => n.id = new_id,
                DialogueNode::TriggerEvent(n) => n.id = new_id,
                DialogueNode::Jump(n) => n.id = new_id,
                DialogueNode::End(n) => n.id = new_id,
                DialogueNode::Random(n) => n.id = new_id,
                DialogueNode::Timed(n) => n.id = new_id,
            }
            new_nodes.push(new_node);
        }
        let new_conns: Vec<NodeConnection> = {
            if let Some(tree) = self.active_tree_mut() {
                tree.connections.iter()
                    .filter(|c| id_map.contains_key(&c.from_node) && id_map.contains_key(&c.to_node))
                    .map(|conn| NodeConnection {
                        from_node: *id_map.get(&conn.from_node).unwrap(),
                        from_port: conn.from_port,
                        to_node: *id_map.get(&conn.to_node).unwrap(),
                        to_port: conn.to_port,
                        label: conn.label.clone(),
                    })
                    .collect()
            } else {
                return;
            }
        };
        self.undo_history.push(EditorAction::PasteNodes {
            nodes: new_nodes.clone(),
            connections: new_conns.clone(),
        });
        self.graph_state.selected_nodes.clear();
        let new_node_ids: Vec<u64> = new_nodes.iter().map(|n| n.id()).collect();
        if let Some(tree) = self.active_tree_mut() {
            for node in new_nodes {
                tree.add_node(node);
            }
            for conn in new_conns {
                tree.connections.push(conn);
            }
        }
        for id in new_node_ids {
            self.graph_state.selected_nodes.insert(id);
        }
        self.dirty = true;
    }

    pub fn select_all(&mut self) {
        if let Some(tree) = self.active_tree() {
            self.graph_state.selected_nodes = tree.nodes.keys().cloned().collect();
        }
    }

    pub fn deselect_all(&mut self) {
        self.graph_state.selected_nodes.clear();
    }

    pub fn auto_layout_active_tree(&mut self) {
        if let Some(tree) = self.active_tree_mut() {
            tree.auto_layout();
        }
    }

    pub fn export_dialogue(&self, tree_id: &str) -> Result<String, String> {
        let tree = self.trees.get(tree_id).ok_or_else(|| "Tree not found".to_string())?;
        match self.export_options.format {
            ExportFormat::Json => Ok(self.export_to_json(tree)),
            ExportFormat::Csv => Ok(self.export_to_csv_format(tree)),
            ExportFormat::InkScript => Ok(self.export_to_ink(tree)),
            _ => Err("Unsupported export format".to_string()),
        }
    }

    fn export_to_json(&self, tree: &DialogueTree) -> String {
        let mut json = String::new();
        json.push_str("{\n");
        json.push_str(&format!("  \"id\": \"{}\",\n", tree.id));
        json.push_str(&format!("  \"name\": \"{}\",\n", tree.name));
        json.push_str(&format!("  \"start_node\": {},\n", tree.start_node_id.unwrap_or(0)));
        json.push_str("  \"nodes\": [\n");
        let nodes: Vec<&DialogueNode> = tree.nodes.values().collect();
        for (i, node) in nodes.iter().enumerate() {
            json.push_str("    {\n");
            json.push_str(&format!("      \"id\": {},\n", node.id()));
            json.push_str(&format!("      \"type\": \"{:?}\",\n", node.node_type()));
            let pos = node.position();
            json.push_str(&format!("      \"x\": {:.1},\n", pos.x));
            json.push_str(&format!("      \"y\": {:.1}\n", pos.y));
            if i < nodes.len() - 1 {
                json.push_str("    },\n");
            } else {
                json.push_str("    }\n");
            }
        }
        json.push_str("  ],\n");
        json.push_str("  \"connections\": [\n");
        for (i, conn) in tree.connections.iter().enumerate() {
            json.push_str("    {\n");
            json.push_str(&format!("      \"from\": {},\n", conn.from_node));
            json.push_str(&format!("      \"from_port\": {},\n", conn.from_port));
            json.push_str(&format!("      \"to\": {},\n", conn.to_node));
            json.push_str(&format!("      \"to_port\": {}\n", conn.to_port));
            if i < tree.connections.len() - 1 {
                json.push_str("    },\n");
            } else {
                json.push_str("    }\n");
            }
        }
        json.push_str("  ]\n");
        json.push('}');
        json
    }

    fn export_to_csv_format(&self, tree: &DialogueTree) -> String {
        let mut csv = String::from("node_id,node_type,speaker,text,localization_key\n");
        for node in tree.nodes.values() {
            match node {
                DialogueNode::Speaker(n) => {
                    let text = n.dialogue_text.replace('"', "\"\"");
                    csv.push_str(&format!("{},speaker,\"{}\",\"{}\",\"{}\"\n",
                        n.id, n.speaker_name, text, n.localization_key));
                }
                DialogueNode::PlayerChoice(n) => {
                    for (i, choice) in n.choices.iter().enumerate() {
                        let text = choice.text.replace('"', "\"\"");
                        csv.push_str(&format!("{},choice_{},player,\"{}\",\"{}\"\n",
                            n.id, i, text, choice.localization_key));
                    }
                }
                _ => {}
            }
        }
        csv
    }

    fn export_to_ink(&self, tree: &DialogueTree) -> String {
        let mut ink = String::new();
        ink.push_str("// Exported from DialogueEditor\n");
        ink.push_str(&format!("// Tree: {}\n\n", tree.name));
        if let Some(start_id) = tree.start_node_id {
            ink.push_str(&format!("-> node_{}\n\n", start_id));
        }
        let order = tree.topological_sort();
        for node_id in order {
            if let Some(node) = tree.nodes.get(&node_id) {
                match node {
                    DialogueNode::Speaker(n) => {
                        ink.push_str(&format!("= node_{}\n", n.id));
                        ink.push_str(&format!("{}: {}\n", n.speaker_name, n.dialogue_text));
                        if let Some(next) = tree.next_node(n.id, 0) {
                            ink.push_str(&format!("-> node_{}\n\n", next));
                        } else {
                            ink.push_str("-> END\n\n");
                        }
                    }
                    DialogueNode::PlayerChoice(n) => {
                        ink.push_str(&format!("= node_{}\n", n.id));
                        for choice in &n.choices {
                            ink.push_str(&format!("+ [{}] -> node_{}\n", choice.text, choice.output_node));
                        }
                        ink.push('\n');
                    }
                    DialogueNode::End(_) => {
                        ink.push_str(&format!("= node_{}\n-> END\n\n", node_id));
                    }
                    _ => {
                        ink.push_str(&format!("= node_{}\n// {:?} node\n", node_id, node.node_type()));
                        if let Some(next) = tree.next_node(node_id, 0) {
                            ink.push_str(&format!("-> node_{}\n\n", next));
                        }
                    }
                }
            }
        }
        ink
    }

    pub fn import_from_json(&mut self, json: &str, name: &str) -> Result<String, String> {
        // Parse basic JSON dialogue tree
        let id = format!("imported_{}", self.allocate_node_id());
        let tree = DialogueTree::new(id.clone(), name.to_string());
        self.trees.insert(id.clone(), tree);
        self.active_tree_id = Some(id.clone());
        self.set_status("Imported successfully", 3.0);
        Ok(id)
    }

    pub fn generate_all_loc_keys(&mut self) {
        if let Some(tree) = self.active_tree_mut() {
            for node in tree.nodes.values_mut() {
                match node {
                    DialogueNode::Speaker(n) => {
                        if n.localization_key.is_empty() {
                            n.localization_key = LocalizationTable::generate_key(
                                &tree.id, n.id, "text"
                            );
                        }
                    }
                    DialogueNode::PlayerChoice(n) => {
                        if n.prompt_localization_key.is_empty() {
                            n.prompt_localization_key = LocalizationTable::generate_key(
                                &tree.id, n.id, "prompt"
                            );
                        }
                        for (i, choice) in n.choices.iter_mut().enumerate() {
                            if choice.localization_key.is_empty() {
                                choice.localization_key = LocalizationTable::generate_key(
                                    &tree.id, n.id, &format!("choice_{}", i)
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn find_all_missing_voice_lines(&self) -> Vec<(u64, String)> {
        let mut missing = Vec::new();
        if let Some(tree) = self.active_tree() {
            for (id, node) in &tree.nodes {
                if let DialogueNode::Speaker(n) = node {
                    if n.voice_line_ref.is_none() {
                        missing.push((*id, n.speaker_name.clone()));
                    }
                }
            }
        }
        missing
    }

    pub fn get_stats(&self) -> DialogueStats {
        if let Some(tree) = self.active_tree() {
            let total_words: usize = tree.nodes.values()
                .filter_map(|n| if let DialogueNode::Speaker(s) = n { Some(s.dialogue_text.split_whitespace().count()) } else { None })
                .sum();
            let speaker_count: usize = {
                let mut set = HashSet::new();
                for node in tree.nodes.values() {
                    if let DialogueNode::Speaker(n) = node {
                        set.insert(n.speaker_id.clone());
                    }
                }
                set.len()
            };
            DialogueStats {
                total_nodes: tree.nodes.len(),
                speaker_nodes: tree.nodes.values().filter(|n| matches!(n, DialogueNode::Speaker(_))).count(),
                choice_nodes: tree.nodes.values().filter(|n| matches!(n, DialogueNode::PlayerChoice(_))).count(),
                condition_nodes: tree.nodes.values().filter(|n| matches!(n, DialogueNode::Condition(_))).count(),
                total_words,
                unique_speakers: speaker_count,
                total_connections: tree.connections.len(),
                validation_errors: self.validation_errors.iter().filter(|e| e.severity == ValidationSeverity::Error).count(),
                validation_warnings: self.validation_errors.iter().filter(|e| e.severity == ValidationSeverity::Warning).count(),
            }
        } else {
            DialogueStats::default()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DialogueStats {
    pub total_nodes: usize,
    pub speaker_nodes: usize,
    pub choice_nodes: usize,
    pub condition_nodes: usize,
    pub total_words: usize,
    pub unique_speakers: usize,
    pub total_connections: usize,
    pub validation_errors: usize,
    pub validation_warnings: usize,
}

// ============================================================
// EXTRA: NODE COLOR / STYLE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct NodeStyle {
    pub background_color: Vec4,
    pub header_color: Vec4,
    pub border_color: Vec4,
    pub selected_border: Vec4,
    pub text_color: Vec4,
    pub port_color: Vec4,
    pub rounding: f32,
    pub border_width: f32,
    pub shadow: bool,
}

impl NodeStyle {
    pub fn for_node_type(node_type: &NodeType) -> Self {
        let (bg, header) = match node_type {
            NodeType::Start => (Vec4::new(0.2, 0.6, 0.2, 1.0), Vec4::new(0.1, 0.5, 0.1, 1.0)),
            NodeType::End => (Vec4::new(0.6, 0.2, 0.2, 1.0), Vec4::new(0.5, 0.1, 0.1, 1.0)),
            NodeType::Speaker => (Vec4::new(0.2, 0.3, 0.6, 1.0), Vec4::new(0.1, 0.2, 0.5, 1.0)),
            NodeType::PlayerChoice => (Vec4::new(0.5, 0.3, 0.1, 1.0), Vec4::new(0.4, 0.2, 0.05, 1.0)),
            NodeType::Condition => (Vec4::new(0.5, 0.5, 0.1, 1.0), Vec4::new(0.4, 0.4, 0.05, 1.0)),
            NodeType::SetVariable => (Vec4::new(0.3, 0.2, 0.5, 1.0), Vec4::new(0.2, 0.1, 0.4, 1.0)),
            NodeType::TriggerEvent => (Vec4::new(0.6, 0.3, 0.5, 1.0), Vec4::new(0.5, 0.2, 0.4, 1.0)),
            NodeType::Jump => (Vec4::new(0.2, 0.5, 0.5, 1.0), Vec4::new(0.1, 0.4, 0.4, 1.0)),
            NodeType::Random => (Vec4::new(0.4, 0.4, 0.4, 1.0), Vec4::new(0.3, 0.3, 0.3, 1.0)),
            NodeType::Timed => (Vec4::new(0.6, 0.4, 0.1, 1.0), Vec4::new(0.5, 0.3, 0.05, 1.0)),
        };
        NodeStyle {
            background_color: bg,
            header_color: header,
            border_color: Vec4::new(0.4, 0.4, 0.4, 1.0),
            selected_border: Vec4::new(1.0, 0.8, 0.0, 1.0),
            text_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            port_color: Vec4::new(0.8, 0.8, 0.8, 1.0),
            rounding: 6.0,
            border_width: 1.5,
            shadow: true,
        }
    }
}

// ============================================================
// EXTRA: NODE COMMENT / ANNOTATION SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct NodeAnnotation {
    pub id: u64,
    pub position: Vec2,
    pub size: Vec2,
    pub title: String,
    pub text: String,
    pub color: Vec4,
    pub collapsed: bool,
    pub font_size: f32,
}

impl NodeAnnotation {
    pub fn new(id: u64, position: Vec2) -> Self {
        NodeAnnotation {
            id,
            position,
            size: Vec2::new(200.0, 100.0),
            title: "Note".to_string(),
            text: String::new(),
            color: Vec4::new(0.9, 0.9, 0.6, 0.5),
            collapsed: false,
            font_size: 12.0,
        }
    }
}

// ============================================================
// EXTRA: DIALOGUE STATISTICS & ANALYSIS
// ============================================================

#[derive(Debug, Clone)]
pub struct DialogueAnalyzer {
    pub tree_id: String,
    pub speaker_word_counts: HashMap<String, usize>,
    pub speaker_node_counts: HashMap<String, usize>,
    pub choice_counts_per_node: Vec<(u64, usize)>,
    pub longest_path: Vec<u64>,
    pub shortest_path_to_end: Vec<u64>,
    pub dead_ends: Vec<u64>,
    pub orphan_nodes: Vec<u64>,
}

impl DialogueAnalyzer {
    pub fn analyze(tree: &DialogueTree) -> Self {
        let mut speaker_word_counts: HashMap<String, usize> = HashMap::new();
        let mut speaker_node_counts: HashMap<String, usize> = HashMap::new();
        let mut choice_counts_per_node = Vec::new();
        let mut dead_ends = Vec::new();
        let mut orphan_nodes = Vec::new();

        let reachable = if let Some(start) = tree.start_node_id {
            tree.find_reachable_nodes(start)
        } else {
            HashSet::new()
        };

        for (id, node) in &tree.nodes {
            if !reachable.contains(id) {
                orphan_nodes.push(*id);
                continue;
            }
            match node {
                DialogueNode::Speaker(n) => {
                    let words = n.dialogue_text.split_whitespace().count();
                    *speaker_word_counts.entry(n.speaker_id.clone()).or_insert(0) += words;
                    *speaker_node_counts.entry(n.speaker_id.clone()).or_insert(0) += 1;
                    if tree.get_outputs(*id).is_empty() {
                        dead_ends.push(*id);
                    }
                }
                DialogueNode::PlayerChoice(n) => {
                    choice_counts_per_node.push((*id, n.choices.len()));
                }
                DialogueNode::End(_) => {}
                _ => {
                    if tree.get_outputs(*id).is_empty() && !matches!(node, DialogueNode::End(_)) {
                        dead_ends.push(*id);
                    }
                }
            }
        }

        let longest_path = if let Some(start) = tree.start_node_id {
            Self::find_longest_path(tree, start)
        } else {
            Vec::new()
        };

        DialogueAnalyzer {
            tree_id: tree.id.clone(),
            speaker_word_counts,
            speaker_node_counts,
            choice_counts_per_node,
            longest_path,
            shortest_path_to_end: Vec::new(),
            dead_ends,
            orphan_nodes,
        }
    }

    fn find_longest_path(tree: &DialogueTree, start: u64) -> Vec<u64> {
        let mut best_path = Vec::new();
        let mut current_path = Vec::new();
        let mut visited = HashSet::new();
        Self::dfs_longest(tree, start, &mut current_path, &mut visited, &mut best_path);
        best_path
    }

    fn dfs_longest(tree: &DialogueTree, node: u64, path: &mut Vec<u64>, visited: &mut HashSet<u64>, best: &mut Vec<u64>) {
        if visited.contains(&node) { return; }
        visited.insert(node);
        path.push(node);
        if path.len() > best.len() {
            *best = path.clone();
        }
        for conn in tree.get_outputs(node).into_iter().map(|c| c.to_node) {
            Self::dfs_longest(tree, conn, path, visited, best);
        }
        path.pop();
        visited.remove(&node);
    }

    pub fn get_speaker_stats(&self) -> Vec<(String, usize, usize)> {
        let mut stats: Vec<(String, usize, usize)> = self.speaker_word_counts.keys()
            .map(|k| (k.clone(),
                *self.speaker_word_counts.get(k).unwrap_or(&0),
                *self.speaker_node_counts.get(k).unwrap_or(&0)))
            .collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        stats
    }

    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== DIALOGUE ANALYSIS REPORT ===\n\n");
        report.push_str("Speaker Statistics:\n");
        for (speaker, words, nodes) in self.get_speaker_stats() {
            report.push_str(&format!("  {}: {} words in {} nodes\n", speaker, words, nodes));
        }
        report.push_str(&format!("\nLongest path: {} nodes\n", self.longest_path.len()));
        report.push_str(&format!("Dead ends: {}\n", self.dead_ends.len()));
        report.push_str(&format!("Orphan nodes: {}\n", self.orphan_nodes.len()));
        report
    }
}

// ============================================================
// EXTRA: CONDITION EDITOR HELPER
// ============================================================

#[derive(Debug, Clone)]
pub struct ConditionEditorState {
    pub root: ConditionExpression,
    pub selected_node_path: Vec<usize>,
    pub edit_mode: ConditionEditMode,
    pub pending_operator: Option<LogicalOperator>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConditionEditMode {
    View,
    AddComparison,
    AddQuestState,
    AddFactionRep,
    AddFlag,
    AddLogical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogicalOperator { And, Or, Not }

impl ConditionEditorState {
    pub fn new(root: ConditionExpression) -> Self {
        ConditionEditorState {
            root,
            selected_node_path: Vec::new(),
            edit_mode: ConditionEditMode::View,
            pending_operator: None,
        }
    }

    pub fn get_description(&self) -> String {
        self.root.describe()
    }

    pub fn simplify(expr: ConditionExpression) -> ConditionExpression {
        match expr {
            ConditionExpression::Not(inner) => {
                match *inner {
                    ConditionExpression::Not(double_inner) => Self::simplify(*double_inner),
                    ConditionExpression::Literal(b) => ConditionExpression::Literal(!b),
                    other => ConditionExpression::Not(Box::new(Self::simplify(other))),
                }
            }
            ConditionExpression::And(a, b) => {
                let sa = Self::simplify(*a);
                let sb = Self::simplify(*b);
                match (&sa, &sb) {
                    (ConditionExpression::Literal(true), _) => sb,
                    (_, ConditionExpression::Literal(true)) => sa,
                    (ConditionExpression::Literal(false), _) => ConditionExpression::Literal(false),
                    (_, ConditionExpression::Literal(false)) => ConditionExpression::Literal(false),
                    _ => ConditionExpression::And(Box::new(sa), Box::new(sb)),
                }
            }
            ConditionExpression::Or(a, b) => {
                let sa = Self::simplify(*a);
                let sb = Self::simplify(*b);
                match (&sa, &sb) {
                    (ConditionExpression::Literal(false), _) => sb,
                    (_, ConditionExpression::Literal(false)) => sa,
                    (ConditionExpression::Literal(true), _) => ConditionExpression::Literal(true),
                    (_, ConditionExpression::Literal(true)) => ConditionExpression::Literal(true),
                    _ => ConditionExpression::Or(Box::new(sa), Box::new(sb)),
                }
            }
            other => other,
        }
    }
}

// ============================================================
// EXTRA: VOICE LINE BROWSER
// ============================================================

#[derive(Debug, Clone)]
pub struct VoiceLineBrowser {
    pub voice_lines: Vec<VoiceLineRef>,
    pub filter_speaker: Option<String>,
    pub filter_emotion: Option<EmotionType>,
    pub search_query: String,
    pub sort_by: VoiceLineSortMode,
    pub selected_idx: Option<usize>,
    pub preview_playing: bool,
    pub preview_time: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceLineSortMode {
    ByName,
    BySpeaker,
    ByDuration,
    ByEmotion,
}

impl VoiceLineBrowser {
    pub fn new() -> Self {
        VoiceLineBrowser {
            voice_lines: Vec::new(),
            filter_speaker: None,
            filter_emotion: None,
            search_query: String::new(),
            sort_by: VoiceLineSortMode::ByName,
            selected_idx: None,
            preview_playing: false,
            preview_time: 0.0,
        }
    }

    pub fn get_filtered(&self) -> Vec<&VoiceLineRef> {
        let query_lower = self.search_query.to_lowercase();
        let mut results: Vec<&VoiceLineRef> = self.voice_lines.iter()
            .filter(|vl| {
                if let Some(ref speaker) = self.filter_speaker {
                    if &vl.speaker_id != speaker { return false; }
                }
                if let Some(ref emotion) = self.filter_emotion {
                    if &vl.emotion != emotion { return false; }
                }
                if !self.search_query.is_empty() {
                    let name_lower = vl.clip_id.to_lowercase();
                    if !name_lower.contains(&query_lower) { return false; }
                }
                true
            })
            .collect();
        match self.sort_by {
            VoiceLineSortMode::ByName => results.sort_by(|a, b| a.clip_id.cmp(&b.clip_id)),
            VoiceLineSortMode::BySpeaker => results.sort_by(|a, b| a.speaker_id.cmp(&b.speaker_id)),
            VoiceLineSortMode::ByDuration => results.sort_by(|a, b| a.duration_secs.partial_cmp(&b.duration_secs).unwrap()),
            VoiceLineSortMode::ByEmotion => results.sort_by(|a, b| a.emotion.name().cmp(b.emotion.name())),
        }
        results
    }

    pub fn update_preview(&mut self, delta: f32) {
        if self.preview_playing {
            self.preview_time += delta;
            if let Some(idx) = self.selected_idx {
                if let Some(vl) = self.voice_lines.get(idx) {
                    if self.preview_time >= vl.duration_secs {
                        self.preview_playing = false;
                        self.preview_time = 0.0;
                    }
                }
            }
        }
    }

    pub fn total_duration(&self) -> f32 {
        self.voice_lines.iter().map(|v| v.duration_secs).sum()
    }
}

// ============================================================
// EXTRA: KEYFRAME / ANIMATION HINTS
// ============================================================

#[derive(Debug, Clone)]
pub struct AnimationHint {
    pub node_id: u64,
    pub trigger_name: String,
    pub target_actor: String,
    pub blend_time: f32,
    pub parameters: HashMap<String, f32>,
    pub play_mode: AnimPlayMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnimPlayMode {
    PlayOnce,
    Loop,
    PingPong,
    Hold,
}

// ============================================================
// EXTRA: DIALOGUE TRIGGERS
// ============================================================

#[derive(Debug, Clone)]
pub struct DialogueTriggerConfig {
    pub trigger_id: String,
    pub conversation_id: String,
    pub trigger_type: TriggerType,
    pub trigger_data: HashMap<String, String>,
    pub priority: i32,
    pub cooldown_secs: f32,
    pub last_triggered: f32,
    pub max_triggers: Option<u32>,
    pub trigger_count: u32,
    pub conditions: Vec<ConditionExpression>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerType {
    OnEnterArea,
    OnInteract,
    OnQuestUpdate,
    OnCombatEnd,
    OnItemPickup,
    OnTimer,
    OnScriptCall,
    OnPlayerDeath,
    OnFactionChange,
}

impl DialogueTriggerConfig {
    pub fn new(trigger_id: String, conversation_id: String, trigger_type: TriggerType) -> Self {
        DialogueTriggerConfig {
            trigger_id,
            conversation_id,
            trigger_type,
            trigger_data: HashMap::new(),
            priority: 0,
            cooldown_secs: 0.0,
            last_triggered: -f32::MAX,
            max_triggers: None,
            trigger_count: 0,
            conditions: Vec::new(),
            enabled: true,
        }
    }

    pub fn can_trigger(&self, current_time: f32, scope: &VariableScope) -> bool {
        if !self.enabled { return false; }
        if current_time - self.last_triggered < self.cooldown_secs { return false; }
        if let Some(max) = self.max_triggers {
            if self.trigger_count >= max { return false; }
        }
        for cond in &self.conditions {
            if !cond.evaluate(scope) { return false; }
        }
        true
    }

    pub fn mark_triggered(&mut self, current_time: f32) {
        self.last_triggered = current_time;
        self.trigger_count += 1;
    }
}

// ============================================================
// EXTRA: EMOTION BLENDING SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct EmotionBlender {
    pub current: EmotionState,
    pub target: EmotionState,
    pub blend_progress: f32,
    pub blend_speed: f32,
    pub history: VecDeque<EmotionState>,
    pub max_history: usize,
}

impl EmotionBlender {
    pub fn new(initial: EmotionState) -> Self {
        EmotionBlender {
            current: initial.clone(),
            target: initial,
            blend_progress: 1.0,
            blend_speed: 2.0,
            history: VecDeque::new(),
            max_history: 10,
        }
    }

    pub fn transition_to(&mut self, new_target: EmotionState) {
        self.history.push_back(self.current.clone());
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
        self.target = new_target;
        self.blend_progress = 0.0;
    }

    pub fn update(&mut self, delta: f32) {
        if self.blend_progress < 1.0 {
            self.blend_progress = (self.blend_progress + delta * self.blend_speed).min(1.0);
            let t = smoothstep(0.0, 1.0, self.blend_progress);
            self.current = self.current.blend_towards(&self.target, t);
        }
    }

    pub fn is_blending(&self) -> bool { self.blend_progress < 1.0 }

    pub fn force_set(&mut self, state: EmotionState) {
        self.current = state.clone();
        self.target = state;
        self.blend_progress = 1.0;
    }

    pub fn get_current_portrait(&self, speaker_id: &str, library: &PortraitLibrary) -> Option<String> {
        self.current.select_portrait(speaker_id, library)
    }
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

// ============================================================
// EXTRA: SUBTITLE RENDERER STATE
// ============================================================

#[derive(Debug, Clone)]
pub struct SubtitleRenderer {
    pub current_cue: Option<SubtitleCue>,
    pub current_time: f32,
    pub fade_in_duration: f32,
    pub fade_out_duration: f32,
    pub position: Vec2,
    pub font_size: f32,
    pub color: Vec4,
    pub background_color: Vec4,
    pub max_width: f32,
    pub opacity: f32,
}

impl SubtitleRenderer {
    pub fn new() -> Self {
        SubtitleRenderer {
            current_cue: None,
            current_time: 0.0,
            fade_in_duration: 0.2,
            fade_out_duration: 0.3,
            position: Vec2::new(0.5, 0.85),
            font_size: 18.0,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            background_color: Vec4::new(0.0, 0.0, 0.0, 0.6),
            max_width: 800.0,
            opacity: 0.0,
        }
    }

    pub fn update(&mut self, time: f32, cues: &[SubtitleCue]) {
        self.current_time = time;
        self.current_cue = cues.iter().find(|c| c.is_active_at(time)).cloned();
        self.opacity = if let Some(ref cue) = self.current_cue {
            let duration = cue.duration();
            let t = time - cue.start_time;
            if t < self.fade_in_duration {
                t / self.fade_in_duration
            } else if t > duration - self.fade_out_duration {
                (duration - t) / self.fade_out_duration
            } else {
                1.0
            }.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    pub fn get_text(&self) -> Option<&str> {
        self.current_cue.as_ref().map(|c| c.text.as_str())
    }
}

// ============================================================
// EXTRA: GRAPH RENDERER HELPERS (Bezier connection rendering)
// ============================================================

#[derive(Debug, Clone)]
pub struct ConnectionRenderer {
    pub smoothness: f32,
    pub line_width: f32,
    pub arrow_size: f32,
    pub selected_color: Vec4,
    pub default_color: Vec4,
    pub invalid_color: Vec4,
    pub hover_color: Vec4,
    pub segment_count: usize,
}

impl Default for ConnectionRenderer {
    fn default() -> Self {
        ConnectionRenderer {
            smoothness: 0.5,
            line_width: 2.0,
            arrow_size: 8.0,
            selected_color: Vec4::new(1.0, 0.8, 0.0, 1.0),
            default_color: Vec4::new(0.7, 0.7, 0.7, 1.0),
            invalid_color: Vec4::new(0.9, 0.2, 0.2, 1.0),
            hover_color: Vec4::new(0.9, 0.9, 1.0, 1.0),
            segment_count: 20,
        }
    }
}

impl ConnectionRenderer {
    pub fn get_curve_for_connection(&self, from: Vec2, to: Vec2) -> BezierCurve {
        BezierCurve::new_connection(from, to, self.smoothness)
    }

    pub fn get_polyline_for_connection(&self, from: Vec2, to: Vec2) -> Vec<Vec2> {
        let curve = self.get_curve_for_connection(from, to);
        curve.to_polyline(self.segment_count)
    }

    pub fn get_arrow_transform(&self, curve: &BezierCurve) -> (Vec2, Vec2) {
        let tip = curve.p3;
        let tangent = curve.tangent_at(1.0);
        (tip, tangent)
    }

    pub fn hit_test_connection(&self, from: Vec2, to: Vec2, point: Vec2, threshold: f32) -> bool {
        let curve = self.get_curve_for_connection(from, to);
        let t = curve.nearest_t(point, self.segment_count);
        let nearest = curve.sample(t);
        (nearest - point).length() < threshold
    }
}

// ============================================================
// EXTRA: SAVE/LOAD SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct DialogueProjectFile {
    pub version: String,
    pub trees: Vec<SerializedTree>,
    pub localization: Vec<SerializedLocEntry>,
    pub voice_lines: Vec<VoiceLineRef>,
    pub speakers: Vec<SpeakerEntry>,
    pub triggers: Vec<DialogueTriggerConfig>,
    pub metadata: ProjectMetadata,
}

#[derive(Debug, Clone)]
pub struct ProjectMetadata {
    pub project_name: String,
    pub author: String,
    pub created: String,
    pub modified: String,
    pub game_version: String,
    pub editor_version: String,
}

#[derive(Debug, Clone)]
pub struct SerializedTree {
    pub id: String,
    pub name: String,
    pub nodes_json: String,
    pub connections_json: String,
}

#[derive(Debug, Clone)]
pub struct SerializedLocEntry {
    pub locale: String,
    pub key: String,
    pub text: String,
}

impl DialogueProjectFile {
    pub fn new(project_name: &str) -> Self {
        DialogueProjectFile {
            version: "1.0.0".to_string(),
            trees: Vec::new(),
            localization: Vec::new(),
            voice_lines: Vec::new(),
            speakers: Vec::new(),
            triggers: Vec::new(),
            metadata: ProjectMetadata {
                project_name: project_name.to_string(),
                author: String::new(),
                created: String::new(),
                modified: String::new(),
                game_version: String::new(),
                editor_version: "1.0.0".to_string(),
            },
        }
    }

    pub fn serialize_header(&self) -> String {
        format!("{{\"version\":\"{}\",\"project\":\"{}\",\"trees\":{},\"loc_entries\":{}}}\n",
            self.version,
            self.metadata.project_name,
            self.trees.len(),
            self.localization.len())
    }
}

// ============================================================
// EXTRA: RUNTIME VARIABLES PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct VariablesPanel {
    pub show_local: bool,
    pub show_global: bool,
    pub show_persistent: bool,
    pub show_only_changed: bool,
    pub filter: String,
    pub sort_by_name: bool,
    pub expanded_groups: HashSet<String>,
}

impl Default for VariablesPanel {
    fn default() -> Self {
        VariablesPanel {
            show_local: true,
            show_global: true,
            show_persistent: true,
            show_only_changed: false,
            filter: String::new(),
            sort_by_name: true,
            expanded_groups: HashSet::new(),
        }
    }
}

impl VariablesPanel {
    pub fn filter_vars<'a>(&self, vars: &'a [(String, ScopeType, &DialogueValue)]) -> Vec<&'a (String, ScopeType, &'a DialogueValue)> {
        vars.iter().filter(|(name, scope, _)| {
            let scope_ok = match scope {
                ScopeType::Local => self.show_local,
                ScopeType::Global => self.show_global,
                ScopeType::Persistent => self.show_persistent,
            };
            let filter_ok = self.filter.is_empty() || name.to_lowercase().contains(&self.filter.to_lowercase());
            scope_ok && filter_ok
        }).collect()
    }
}

// ============================================================
// EXTRA: HISTORY REPLAY
// ============================================================

#[derive(Debug, Clone)]
pub struct HistoryReplayer {
    pub history: Vec<HistoryEntry>,
    pub current_index: usize,
    pub is_playing: bool,
    pub playback_speed: f32,
    pub loop_playback: bool,
    pub time_since_last_step: f32,
    pub step_duration: f32,
}

impl HistoryReplayer {
    pub fn new(history: Vec<HistoryEntry>) -> Self {
        HistoryReplayer {
            history,
            current_index: 0,
            is_playing: false,
            playback_speed: 1.0,
            loop_playback: false,
            time_since_last_step: 0.0,
            step_duration: 1.0,
        }
    }

    pub fn play(&mut self) { self.is_playing = true; }
    pub fn pause(&mut self) { self.is_playing = false; }
    pub fn stop(&mut self) { self.is_playing = false; self.current_index = 0; }

    pub fn next_step(&mut self) {
        if self.current_index + 1 < self.history.len() {
            self.current_index += 1;
        } else if self.loop_playback {
            self.current_index = 0;
        } else {
            self.is_playing = false;
        }
    }

    pub fn prev_step(&mut self) {
        if self.current_index > 0 {
            self.current_index -= 1;
        }
    }

    pub fn update(&mut self, delta: f32) {
        if !self.is_playing { return; }
        self.time_since_last_step += delta * self.playback_speed;
        while self.time_since_last_step >= self.step_duration {
            self.time_since_last_step -= self.step_duration;
            self.next_step();
        }
    }

    pub fn current_entry(&self) -> Option<&HistoryEntry> {
        self.history.get(self.current_index)
    }

    pub fn progress(&self) -> f32 {
        if self.history.is_empty() { return 0.0; }
        self.current_index as f32 / self.history.len() as f32
    }
}

// ============================================================
// EXTRA: DIALOGUE OVERVIEW / NAVIGATOR
// ============================================================

#[derive(Debug, Clone)]
pub struct DialogueNavigator {
    pub conversations: Vec<ConversationSummary>,
    pub search_query: String,
    pub filter_tags: Vec<String>,
    pub sort_mode: NavigatorSortMode,
    pub selected_id: Option<String>,
    pub show_archived: bool,
}

#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub id: String,
    pub name: String,
    pub node_count: usize,
    pub word_count: usize,
    pub speaker_count: usize,
    pub has_validation_errors: bool,
    pub tags: Vec<String>,
    pub archived: bool,
    pub last_modified: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavigatorSortMode {
    ByName,
    ByNodeCount,
    ByWordCount,
    ByLastModified,
}

impl DialogueNavigator {
    pub fn new() -> Self {
        DialogueNavigator {
            conversations: Vec::new(),
            search_query: String::new(),
            filter_tags: Vec::new(),
            sort_mode: NavigatorSortMode::ByName,
            selected_id: None,
            show_archived: false,
        }
    }

    pub fn get_filtered(&self) -> Vec<&ConversationSummary> {
        let query_lower = self.search_query.to_lowercase();
        let mut results: Vec<&ConversationSummary> = self.conversations.iter()
            .filter(|c| {
                if c.archived && !self.show_archived { return false; }
                if !query_lower.is_empty() && !c.name.to_lowercase().contains(&query_lower) { return false; }
                if !self.filter_tags.is_empty() {
                    if !self.filter_tags.iter().all(|t| c.tags.contains(t)) { return false; }
                }
                true
            })
            .collect();
        match self.sort_mode {
            NavigatorSortMode::ByName => results.sort_by(|a, b| a.name.cmp(&b.name)),
            NavigatorSortMode::ByNodeCount => results.sort_by(|a, b| b.node_count.cmp(&a.node_count)),
            NavigatorSortMode::ByWordCount => results.sort_by(|a, b| b.word_count.cmp(&a.word_count)),
            NavigatorSortMode::ByLastModified => results.sort_by(|a, b| b.last_modified.cmp(&a.last_modified)),
        }
        results
    }

    pub fn build_summaries(&mut self, editor: &DialogueEditor) {
        self.conversations.clear();
        for (id, tree) in &editor.trees {
            let node_count = tree.nodes.len();
            let word_count: usize = tree.nodes.values()
                .filter_map(|n| if let DialogueNode::Speaker(s) = n { Some(s.dialogue_text.split_whitespace().count()) } else { None })
                .sum();
            let mut speakers = HashSet::new();
            for node in tree.nodes.values() {
                if let DialogueNode::Speaker(n) = node {
                    speakers.insert(n.speaker_id.clone());
                }
            }
            self.conversations.push(ConversationSummary {
                id: id.clone(),
                name: tree.name.clone(),
                node_count,
                word_count,
                speaker_count: speakers.len(),
                has_validation_errors: false,
                tags: tree.metadata.tags.clone(),
                archived: false,
                last_modified: tree.metadata.modified_at.clone(),
            });
        }
    }
}

// ============================================================
// EXTRA: SELECTION HISTORY
// ============================================================

#[derive(Debug, Clone)]
pub struct SelectionHistory {
    pub history: VecDeque<SelectionSnapshot>,
    pub current: usize,
    pub max_history: usize,
}

#[derive(Debug, Clone)]
pub struct SelectionSnapshot {
    pub selected_ids: HashSet<u64>,
    pub timestamp: f32,
}

impl SelectionHistory {
    pub fn new() -> Self {
        SelectionHistory {
            history: VecDeque::new(),
            current: 0,
            max_history: 20,
        }
    }

    pub fn push(&mut self, selection: HashSet<u64>, time: f32) {
        self.history.truncate(self.current + 1);
        self.history.push_back(SelectionSnapshot {
            selected_ids: selection,
            timestamp: time,
        });
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }
        self.current = self.history.len().saturating_sub(1);
    }

    pub fn back(&mut self) -> Option<&SelectionSnapshot> {
        if self.current > 0 {
            self.current -= 1;
        }
        self.history.get(self.current)
    }

    pub fn forward(&mut self) -> Option<&SelectionSnapshot> {
        if self.current + 1 < self.history.len() {
            self.current += 1;
        }
        self.history.get(self.current)
    }
}

// ============================================================
// EXTRA: COMPACT CLIPBOARD
// ============================================================

#[derive(Debug, Clone)]
pub struct NodeClipboard {
    pub nodes: Vec<DialogueNode>,
    pub connections: Vec<NodeConnection>,
    pub center: Vec2,
}

impl NodeClipboard {
    pub fn new() -> Self {
        NodeClipboard {
            nodes: Vec::new(),
            connections: Vec::new(),
            center: Vec2::ZERO,
        }
    }

    pub fn set(&mut self, nodes: Vec<DialogueNode>, connections: Vec<NodeConnection>) {
        if nodes.is_empty() { return; }
        let avg_x: f32 = nodes.iter().map(|n| n.position().x).sum::<f32>() / nodes.len() as f32;
        let avg_y: f32 = nodes.iter().map(|n| n.position().y).sum::<f32>() / nodes.len() as f32;
        self.center = Vec2::new(avg_x, avg_y);
        self.nodes = nodes;
        self.connections = connections;
    }

    pub fn is_empty(&self) -> bool { self.nodes.is_empty() }

    pub fn count(&self) -> usize { self.nodes.len() }
}

// ============================================================
// EXTRA: PROPERTY PANEL
// ============================================================

#[derive(Debug, Clone)]
pub struct PropertyPanelState {
    pub selected_node_id: Option<u64>,
    pub property_tabs: Vec<PropertyTab>,
    pub active_tab: usize,
    pub scroll_offset: f32,
    pub edit_field: Option<String>,
    pub edit_value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyTab {
    General,
    Conditions,
    Variables,
    Audio,
    Localization,
    Tags,
    Debug,
}

impl PropertyPanelState {
    pub fn new() -> Self {
        PropertyPanelState {
            selected_node_id: None,
            property_tabs: vec![
                PropertyTab::General,
                PropertyTab::Conditions,
                PropertyTab::Audio,
                PropertyTab::Localization,
                PropertyTab::Tags,
                PropertyTab::Debug,
            ],
            active_tab: 0,
            scroll_offset: 0.0,
            edit_field: None,
            edit_value: String::new(),
        }
    }

    pub fn select_node(&mut self, id: Option<u64>) {
        self.selected_node_id = id;
        self.active_tab = 0;
        self.scroll_offset = 0.0;
        self.edit_field = None;
    }

    pub fn start_edit(&mut self, field: &str, current_value: &str) {
        self.edit_field = Some(field.to_string());
        self.edit_value = current_value.to_string();
    }

    pub fn commit_edit(&mut self) -> Option<(String, String)> {
        if let Some(field) = self.edit_field.take() {
            let val = self.edit_value.clone();
            self.edit_value.clear();
            Some((field, val))
        } else {
            None
        }
    }

    pub fn cancel_edit(&mut self) {
        self.edit_field = None;
        self.edit_value.clear();
    }
}

// ============================================================
// EXTRA: DIALOGUE TESTING UTILITIES
// ============================================================

#[derive(Debug, Clone)]
pub struct DialogueTester {
    pub test_cases: Vec<DialogueTestCase>,
    pub results: Vec<TestResult>,
    pub running: bool,
    pub current_test_idx: usize,
}

#[derive(Debug, Clone)]
pub struct DialogueTestCase {
    pub name: String,
    pub description: String,
    pub initial_variables: HashMap<String, DialogueValue>,
    pub choice_sequence: Vec<usize>,
    pub expected_speaker_sequence: Vec<String>,
    pub expected_end_type: EndType,
    pub expected_final_variables: HashMap<String, DialogueValue>,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub test_name: String,
    pub passed: bool,
    pub errors: Vec<String>,
    pub actual_speaker_sequence: Vec<String>,
    pub actual_end_type: Option<EndType>,
    pub steps_taken: usize,
}

impl DialogueTester {
    pub fn new() -> Self {
        DialogueTester {
            test_cases: Vec::new(),
            results: Vec::new(),
            running: false,
            current_test_idx: 0,
        }
    }

    pub fn run_all(&mut self, tree: &DialogueTree) {
        self.results.clear();
        for test in &self.test_cases.clone() {
            let result = self.run_test(test, tree);
            self.results.push(result);
        }
    }

    fn run_test(&self, test: &DialogueTestCase, tree: &DialogueTree) -> TestResult {
        let mut player = DialoguePlayer::new(tree.clone());
        for (k, v) in &test.initial_variables {
            player.scope.set(k.clone(), ScopeType::Global, v.clone());
        }
        let mut result = TestResult {
            test_name: test.name.clone(),
            passed: false,
            errors: Vec::new(),
            actual_speaker_sequence: Vec::new(),
            actual_end_type: None,
            steps_taken: 0,
        };

        let mut choice_idx = 0;
        let mut play_result = player.start();
        result.steps_taken += 1;

        loop {
            match play_result {
                PlayResult::ShowDialogue { speaker, .. } => {
                    result.actual_speaker_sequence.push(speaker);
                    play_result = player.step(None);
                }
                PlayResult::ShowChoices { .. } => {
                    let choice = if choice_idx < test.choice_sequence.len() {
                        let c = test.choice_sequence[choice_idx];
                        choice_idx += 1;
                        c
                    } else {
                        0
                    };
                    play_result = player.step(Some(choice));
                }
                PlayResult::NeedAdvance => {
                    play_result = player.step(None);
                }
                PlayResult::Ended => {
                    if let PlayerState::Ended(ref end_type) = player.state {
                        result.actual_end_type = Some(end_type.clone());
                    }
                    break;
                }
                PlayResult::Error(msg) => {
                    result.errors.push(msg);
                    break;
                }
                _ => {
                    play_result = player.step(None);
                }
            }
            result.steps_taken += 1;
            if result.steps_taken > 10000 {
                result.errors.push("Test exceeded max steps".to_string());
                break;
            }
        }

        // Validate results
        if result.actual_speaker_sequence != test.expected_speaker_sequence {
            result.errors.push(format!("Speaker sequence mismatch: expected {:?}, got {:?}",
                test.expected_speaker_sequence, result.actual_speaker_sequence));
        }
        if let Some(ref actual_end) = result.actual_end_type {
            if actual_end != &test.expected_end_type {
                result.errors.push(format!("End type mismatch: expected {:?}, got {:?}",
                    test.expected_end_type, actual_end));
            }
        }
        for (k, expected) in &test.expected_final_variables {
            let actual = player.scope.get_any(k);
            if actual.as_ref() != Some(expected) {
                result.errors.push(format!("Variable {} expected {:?} got {:?}", k, expected, actual));
            }
        }

        result.passed = result.errors.is_empty();
        result
    }

    pub fn get_pass_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }

    pub fn get_fail_count(&self) -> usize {
        self.results.iter().filter(|r| !r.passed).count()
    }
}

// ============================================================
// EXTRA: TAG SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct TagManager {
    pub known_tags: HashSet<String>,
    pub tag_colors: HashMap<String, Vec4>,
    pub tag_categories: HashMap<String, Vec<String>>,
}

impl TagManager {
    pub fn new() -> Self {
        let mut tm = TagManager {
            known_tags: HashSet::new(),
            tag_colors: HashMap::new(),
            tag_categories: HashMap::new(),
        };
        tm.register_default_tags();
        tm
    }

    fn register_default_tags(&mut self) {
        let tags = [
            ("important", Vec4::new(1.0, 0.2, 0.2, 1.0)),
            ("review", Vec4::new(1.0, 0.8, 0.0, 1.0)),
            ("done", Vec4::new(0.2, 0.8, 0.2, 1.0)),
            ("wip", Vec4::new(0.5, 0.5, 0.5, 1.0)),
            ("vo_needed", Vec4::new(0.8, 0.4, 0.0, 1.0)),
            ("loc_needed", Vec4::new(0.4, 0.6, 1.0, 1.0)),
            ("optional", Vec4::new(0.6, 0.6, 0.8, 1.0)),
        ];
        for (tag, color) in &tags {
            self.known_tags.insert(tag.to_string());
            self.tag_colors.insert(tag.to_string(), *color);
        }
    }

    pub fn register_tag(&mut self, tag: &str, color: Vec4) {
        self.known_tags.insert(tag.to_string());
        self.tag_colors.insert(tag.to_string(), color);
    }

    pub fn get_color(&self, tag: &str) -> Vec4 {
        self.tag_colors.get(tag).copied().unwrap_or(Vec4::new(0.7, 0.7, 0.7, 1.0))
    }

    pub fn find_nodes_with_tag<'a>(&self, tag: &str, tree: &'a DialogueTree) -> Vec<&'a DialogueNode> {
        tree.nodes.values()
            .filter(|n| n.tags().contains(&tag.to_string()))
            .collect()
    }

    pub fn add_category(&mut self, category: &str, tags: Vec<String>) {
        self.tag_categories.insert(category.to_string(), tags);
    }
}

// ============================================================
// EXTRA: DIALOGUE VERSIONING
// ============================================================

#[derive(Debug, Clone)]
pub struct DialogueVersion {
    pub version_id: String,
    pub tree_id: String,
    pub timestamp: f64,
    pub author: String,
    pub message: String,
    pub node_count: usize,
    pub word_count: usize,
    pub snapshot: String,
}

#[derive(Debug, Clone)]
pub struct VersionControl {
    pub versions: BTreeMap<String, Vec<DialogueVersion>>,
    pub auto_version: bool,
    pub auto_version_interval: f32,
    pub time_since_last_auto: f32,
}

impl VersionControl {
    pub fn new() -> Self {
        VersionControl {
            versions: BTreeMap::new(),
            auto_version: true,
            auto_version_interval: 300.0,
            time_since_last_auto: 0.0,
        }
    }

    pub fn create_version(&mut self, tree: &DialogueTree, author: &str, message: &str) -> String {
        let ver_id = format!("v{}_{}", tree.id, self.versions.entry(tree.id.clone()).or_default().len());
        let word_count: usize = tree.nodes.values()
            .filter_map(|n| if let DialogueNode::Speaker(s) = n { Some(s.dialogue_text.split_whitespace().count()) } else { None })
            .sum();
        let version = DialogueVersion {
            version_id: ver_id.clone(),
            tree_id: tree.id.clone(),
            timestamp: 0.0,
            author: author.to_string(),
            message: message.to_string(),
            node_count: tree.nodes.len(),
            word_count,
            snapshot: String::new(),
        };
        self.versions.entry(tree.id.clone()).or_default().push(version);
        ver_id
    }

    pub fn get_versions(&self, tree_id: &str) -> &[DialogueVersion] {
        self.versions.get(tree_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn update(&mut self, delta: f32, tree: Option<&DialogueTree>) {
        if !self.auto_version { return; }
        self.time_since_last_auto += delta;
        if self.time_since_last_auto >= self.auto_version_interval {
            self.time_since_last_auto = 0.0;
            if let Some(tree) = tree {
                self.create_version(tree, "auto", "Auto-save");
            }
        }
    }
}

// ============================================================
// EXTRA: BATCH EDIT OPERATIONS
// ============================================================

#[derive(Debug, Clone)]
pub struct BatchEditOperation {
    pub op_type: BatchOpType,
    pub target_tags: Vec<String>,
    pub target_speakers: Vec<String>,
    pub node_ids: Vec<u64>,
}

#[derive(Debug, Clone)]
pub enum BatchOpType {
    AddTag(String),
    RemoveTag(String),
    SetEmotion(EmotionState),
    SetTextSpeed(f32),
    EnableAutoAdvance(bool),
    SetAutoAdvanceDelay(f32),
    ReplaceText { from: String, to: String },
    RegenerateLocKeys,
}

impl BatchEditOperation {
    pub fn apply_to_tree(&self, tree: &mut DialogueTree) -> usize {
        let mut modified = 0;
        let node_ids: Vec<u64> = if self.node_ids.is_empty() {
            tree.nodes.keys().cloned().collect()
        } else {
            self.node_ids.clone()
        };
        for id in node_ids {
            let matches = {
                let node = tree.nodes.get(&id);
                node.map(|n| {
                    if !self.target_tags.is_empty() {
                        self.target_tags.iter().any(|t| n.tags().contains(t))
                    } else if !self.target_speakers.is_empty() {
                        if let DialogueNode::Speaker(s) = n {
                            self.target_speakers.contains(&s.speaker_id)
                        } else {
                            false
                        }
                    } else {
                        true
                    }
                }).unwrap_or(false)
            };
            if !matches { continue; }
            if let Some(node) = tree.nodes.get_mut(&id) {
                match &self.op_type {
                    BatchOpType::AddTag(tag) => {
                        match node {
                            DialogueNode::Speaker(n) => {
                                if !n.tags.contains(tag) { n.tags.push(tag.clone()); modified += 1; }
                            }
                            _ => {}
                        }
                    }
                    BatchOpType::RemoveTag(tag) => {
                        match node {
                            DialogueNode::Speaker(n) => {
                                let before = n.tags.len();
                                n.tags.retain(|t| t != tag);
                                if n.tags.len() < before { modified += 1; }
                            }
                            _ => {}
                        }
                    }
                    BatchOpType::SetEmotion(emotion) => {
                        if let DialogueNode::Speaker(n) = node {
                            n.emotion_state = emotion.clone();
                            modified += 1;
                        }
                    }
                    BatchOpType::SetTextSpeed(speed) => {
                        if let DialogueNode::Speaker(n) = node {
                            n.text_speed = *speed;
                            modified += 1;
                        }
                    }
                    BatchOpType::EnableAutoAdvance(enabled) => {
                        if let DialogueNode::Speaker(n) = node {
                            n.auto_advance = *enabled;
                            modified += 1;
                        }
                    }
                    BatchOpType::SetAutoAdvanceDelay(delay) => {
                        if let DialogueNode::Speaker(n) = node {
                            n.auto_advance_delay = *delay;
                            modified += 1;
                        }
                    }
                    BatchOpType::ReplaceText { from, to } => {
                        if let DialogueNode::Speaker(n) = node {
                            if n.dialogue_text.contains(from.as_str()) {
                                n.dialogue_text = n.dialogue_text.replace(from.as_str(), to.as_str());
                                modified += 1;
                            }
                        }
                    }
                    BatchOpType::RegenerateLocKeys => {
                        match node {
                            DialogueNode::Speaker(n) => {
                                n.localization_key = LocalizationTable::generate_key(&tree.id, n.id, "text");
                                modified += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        modified
    }
}

// ============================================================
// FINAL: MODULE-LEVEL UTILITY FUNCTIONS
// ============================================================

pub fn create_sample_tree() -> DialogueTree {
    let mut tree = DialogueTree::new("sample_001".to_string(), "Sample Conversation".to_string());

    let start_id = 1u64;
    let mut start = StartNode::new(start_id);
    start.position = Vec2::new(50.0, 200.0);
    tree.add_node(DialogueNode::Start(start));

    let greet_id = 2u64;
    let mut greet = SpeakerNode::new(greet_id);
    greet.position = Vec2::new(300.0, 200.0);
    greet.speaker_id = "npc_merchant".to_string();
    greet.speaker_name = "Merchant".to_string();
    greet.dialogue_text = "Ah, welcome traveler! What brings you to my shop?".to_string();
    greet.localization_key = "sample_001_0002_text".to_string();
    greet.emotion_state = EmotionState::new(EmotionType::Happy, 0.7);
    tree.add_node(DialogueNode::Speaker(greet));

    let choice_id = 3u64;
    let mut choice_node = PlayerChoiceNode::new(choice_id);
    choice_node.position = Vec2::new(600.0, 200.0);
    choice_node.prompt_text = "What do you want to say?".to_string();
    choice_node.add_choice("I'm looking for supplies.".to_string(), 4);
    choice_node.add_choice("Just browsing.".to_string(), 5);
    choice_node.add_choice("Goodbye.".to_string(), 6);
    tree.add_node(DialogueNode::PlayerChoice(choice_node));

    let reply_id = 4u64;
    let mut reply = SpeakerNode::new(reply_id);
    reply.position = Vec2::new(900.0, 100.0);
    reply.speaker_id = "npc_merchant".to_string();
    reply.speaker_name = "Merchant".to_string();
    reply.dialogue_text = "You've come to the right place! I have everything you need.".to_string();
    tree.add_node(DialogueNode::Speaker(reply));

    let browse_id = 5u64;
    let mut browse = SpeakerNode::new(browse_id);
    browse.position = Vec2::new(900.0, 250.0);
    browse.speaker_id = "npc_merchant".to_string();
    browse.speaker_name = "Merchant".to_string();
    browse.dialogue_text = "Take your time, take your time!".to_string();
    tree.add_node(DialogueNode::Speaker(browse));

    let end_id = 6u64;
    let mut end = EndNode::new(end_id);
    end.position = Vec2::new(1200.0, 200.0);
    end.end_type = EndType::Normal;
    tree.add_node(DialogueNode::End(end));

    tree.connect(1, 0, 2, 0, None);
    tree.connect(2, 0, 3, 0, None);
    tree.connect(3, 0, 4, 0, Some("supplies".to_string()));
    tree.connect(3, 1, 5, 0, Some("browse".to_string()));
    tree.connect(3, 2, 6, 0, Some("goodbye".to_string()));
    tree.connect(4, 0, 6, 0, None);
    tree.connect(5, 0, 6, 0, None);

    tree
}

pub fn build_condition_from_str(expr: &str) -> Option<ConditionExpression> {
    let expr = expr.trim();
    if expr == "true" { return Some(ConditionExpression::Literal(true)); }
    if expr == "false" { return Some(ConditionExpression::Literal(false)); }
    // Parse simple comparisons like "var_name == value"
    for op_str in &["==", "!=", ">=", "<=", ">", "<"] {
        if let Some(pos) = expr.find(op_str) {
            let var = expr[..pos].trim().to_string();
            let val_str = expr[pos + op_str.len()..].trim();
            let op = match *op_str {
                "==" => ComparisonOperator::Equal,
                "!=" => ComparisonOperator::NotEqual,
                ">=" => ComparisonOperator::GreaterEqual,
                "<=" => ComparisonOperator::LessEqual,
                ">" => ComparisonOperator::GreaterThan,
                "<" => ComparisonOperator::LessThan,
                _ => continue,
            };
            let val = if let Ok(i) = val_str.parse::<i64>() {
                DialogueValue::Int(i)
            } else if let Ok(f) = val_str.parse::<f64>() {
                DialogueValue::Float(f)
            } else if val_str == "true" {
                DialogueValue::Bool(true)
            } else if val_str == "false" {
                DialogueValue::Bool(false)
            } else {
                DialogueValue::String(val_str.trim_matches('"').to_string())
            };
            return Some(ConditionExpression::Comparison(ComparisonCondition {
                variable_name: var,
                scope_type: ScopeType::Global,
                operator: op,
                compare_value: val,
            }));
        }
    }
    None
}

pub fn format_dialogue_text(text: &str, scope: &VariableScope) -> String {
    let mut result = text.to_string();
    // Simple variable substitution: {var_name}
    let mut i = 0;
    let chars: Vec<char> = result.chars().collect();
    let mut output = String::new();
    while i < chars.len() {
        if chars[i] == '{' {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != '}' {
                end += 1;
            }
            if end < chars.len() {
                let var_name: String = chars[start..end].iter().collect();
                if let Some(val) = scope.get_any(&var_name) {
                    output.push_str(&val.to_string_repr());
                } else {
                    output.push('{');
                    output.push_str(&var_name);
                    output.push('}');
                }
                i = end + 1;
            } else {
                output.push(chars[i]);
                i += 1;
            }
        } else {
            output.push(chars[i]);
            i += 1;
        }
    }
    output
}

pub fn count_dialogue_tree_words(tree: &DialogueTree) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for node in tree.nodes.values() {
        if let DialogueNode::Speaker(n) = node {
            let words = n.dialogue_text.split_whitespace().count();
            *counts.entry(n.speaker_id.clone()).or_insert(0) += words;
        }
    }
    counts
}

pub fn estimate_voice_recording_time(tree: &DialogueTree, wpm: f32) -> f32 {
    let total_words: usize = tree.nodes.values()
        .filter_map(|n| if let DialogueNode::Speaker(s) = n { Some(s.dialogue_text.split_whitespace().count()) } else { None })
        .sum();
    total_words as f32 / wpm * 60.0
}

pub fn generate_localization_report(loc_table: &LocalizationTable, tree: &DialogueTree) -> String {
    let mut report = String::new();
    let keys = tree.collect_all_localization_keys();
    let total = keys.len();
    report.push_str(&format!("Total localization keys: {}\n\n", total));
    for locale in &loc_table.supported_locales {
        let translated = keys.iter().filter(|k| loc_table.get(k, locale).is_some()).count();
        let pct = if total > 0 { translated * 100 / total } else { 0 };
        report.push_str(&format!("{}: {}/{} ({pct}%)\n", locale, translated, total, pct = pct));
    }
    report
}
