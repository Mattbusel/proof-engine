use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Align2, Shape};
use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Serialize, Deserialize};

// ============================================================
// CONSTANTS
// ============================================================

const NODE_WIDTH: f32 = 180.0;
const NODE_MIN_HEIGHT: f32 = 60.0;
const NODE_HEADER_HEIGHT: f32 = 28.0;
const NODE_BODY_LINE_HEIGHT: f32 = 18.0;
const NODE_CORNER_RADIUS: f32 = 6.0;
const GRID_SPACING: f32 = 32.0;
const GRID_DOT_RADIUS: f32 = 1.5;
const BEZIER_CONTROL_OFFSET: f32 = 80.0;
const ARROW_SIZE: f32 = 8.0;
const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 3.0;
const ZOOM_STEP: f32 = 0.1;
const EXECUTION_LOG_MAX: usize = 200;
const DEFAULT_CANVAS_ZOOM: f32 = 1.0;
const NODE_PALETTE_WIDTH: f32 = 200.0;
const PROPERTIES_PANEL_WIDTH: f32 = 260.0;
const BLACKBOARD_PANEL_HEIGHT: f32 = 220.0;
const SIMULATION_PANEL_HEIGHT: f32 = 180.0;
const CONNECTION_THICKNESS: f32 = 2.0;
const CONNECTION_THICKNESS_HOVER: f32 = 3.5;
const SNAP_GRID: f32 = 16.0;

// Colors
const COLOR_COMPOSITE: Color32 = Color32::from_rgb(70, 130, 200);
const COLOR_COMPOSITE_DARK: Color32 = Color32::from_rgb(40, 90, 160);
const COLOR_ACTION: Color32 = Color32::from_rgb(60, 180, 90);
const COLOR_ACTION_DARK: Color32 = Color32::from_rgb(30, 130, 55);
const COLOR_CONDITION: Color32 = Color32::from_rgb(210, 180, 50);
const COLOR_CONDITION_DARK: Color32 = Color32::from_rgb(160, 130, 20);
const COLOR_DECORATOR: Color32 = Color32::from_rgb(170, 80, 200);
const COLOR_DECORATOR_DARK: Color32 = Color32::from_rgb(120, 40, 155);
const COLOR_LEAF: Color32 = Color32::from_rgb(150, 150, 160);
const COLOR_LEAF_DARK: Color32 = Color32::from_rgb(100, 100, 110);
const COLOR_STATUS_SUCCESS: Color32 = Color32::from_rgb(80, 220, 80);
const COLOR_STATUS_FAILURE: Color32 = Color32::from_rgb(220, 60, 60);
const COLOR_STATUS_RUNNING: Color32 = Color32::from_rgb(220, 150, 30);
const COLOR_STATUS_IDLE: Color32 = Color32::from_rgb(120, 120, 130);
const COLOR_BREAKPOINT: Color32 = Color32::from_rgb(230, 50, 50);
const COLOR_SELECTED_BORDER: Color32 = Color32::from_rgb(255, 230, 80);
const COLOR_HOVERED_BORDER: Color32 = Color32::from_rgb(200, 200, 255);
const COLOR_CANVAS_BG: Color32 = Color32::from_rgb(28, 28, 32);
const COLOR_GRID_DOT: Color32 = Color32::from_rgb(60, 60, 70);
const COLOR_NODE_BG: Color32 = Color32::from_rgb(45, 45, 52);
const COLOR_NODE_TEXT: Color32 = Color32::from_rgb(230, 230, 235);
const COLOR_NODE_SUBTEXT: Color32 = Color32::from_rgb(170, 170, 180);
const COLOR_CONNECTION: Color32 = Color32::from_rgb(140, 140, 160);
const COLOR_CONNECTION_ACTIVE: Color32 = Color32::from_rgb(80, 200, 255);
const COLOR_PANEL_BG: Color32 = Color32::from_rgb(35, 35, 42);
const COLOR_PANEL_HEADER: Color32 = Color32::from_rgb(50, 50, 60);
const COLOR_SELECTION_BOX: Color32 = Color32::from_rgba_premultiplied(100, 160, 255, 40);
const COLOR_SELECTION_BORDER: Color32 = Color32::from_rgb(100, 160, 255);

// ============================================================
// ENUMS — CORE TYPES
// ============================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParallelPolicy {
    RequireAll,
    RequireOne,
    RequireN(usize),
}

impl std::fmt::Display for ParallelPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParallelPolicy::RequireAll => write!(f, "Require All"),
            ParallelPolicy::RequireOne => write!(f, "Require One"),
            ParallelPolicy::RequireN(n) => write!(f, "Require {}", n),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BtNode {
    // Composites
    Sequence,
    Selector,
    Parallel(ParallelPolicy),
    RandomSelector,
    WeightedSelector(Vec<f32>),
    // Decorators
    Inverter,
    Succeeder,
    Failer,
    Repeat(u32),
    RepeatUntilFail,
    Cooldown { duration_secs: f32, shared_key: Option<String> },
    Timeout(f32),
    Limiter { max_per_interval: u32, interval_secs: f32 },
    // Leaves
    Action(ActionKind),
    Condition(ConditionKind),
    Wait(f32),
    Log(String),
    SetBlackboard { key: String, value: BlackboardValue },
    CheckBlackboard { key: String, op: CompareOp, value: BlackboardValue },
}

impl BtNode {
    pub fn category(&self) -> NodeCategory {
        match self {
            BtNode::Sequence | BtNode::Selector | BtNode::Parallel(_)
            | BtNode::RandomSelector | BtNode::WeightedSelector(_) => NodeCategory::Composite,
            BtNode::Inverter | BtNode::Succeeder | BtNode::Failer
            | BtNode::Repeat(_) | BtNode::RepeatUntilFail
            | BtNode::Cooldown { .. } | BtNode::Timeout(_) | BtNode::Limiter { .. } => NodeCategory::Decorator,
            BtNode::Action(_) => NodeCategory::Action,
            BtNode::Condition(_) | BtNode::CheckBlackboard { .. } => NodeCategory::Condition,
            BtNode::Wait(_) | BtNode::Log(_) | BtNode::SetBlackboard { .. } => NodeCategory::Leaf,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            BtNode::Sequence => "Sequence",
            BtNode::Selector => "Selector",
            BtNode::Parallel(_) => "Parallel",
            BtNode::RandomSelector => "Random Selector",
            BtNode::WeightedSelector(_) => "Weighted Selector",
            BtNode::Inverter => "Inverter",
            BtNode::Succeeder => "Succeeder",
            BtNode::Failer => "Failer",
            BtNode::Repeat(_) => "Repeat",
            BtNode::RepeatUntilFail => "Repeat Until Fail",
            BtNode::Cooldown { .. } => "Cooldown",
            BtNode::Timeout(_) => "Timeout",
            BtNode::Limiter { .. } => "Limiter",
            BtNode::Action(_) => "Action",
            BtNode::Condition(_) => "Condition",
            BtNode::Wait(_) => "Wait",
            BtNode::Log(_) => "Log",
            BtNode::SetBlackboard { .. } => "Set Blackboard",
            BtNode::CheckBlackboard { .. } => "Check Blackboard",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            BtNode::Sequence => "→",
            BtNode::Selector => "?",
            BtNode::Parallel(_) => "⇉",
            BtNode::RandomSelector => "🎲",
            BtNode::WeightedSelector(_) => "⚖",
            BtNode::Inverter => "!",
            BtNode::Succeeder => "✓",
            BtNode::Failer => "✗",
            BtNode::Repeat(_) => "↺",
            BtNode::RepeatUntilFail => "↻",
            BtNode::Cooldown { .. } => "⏱",
            BtNode::Timeout(_) => "⌛",
            BtNode::Limiter { .. } => "🔒",
            BtNode::Action(_) => "▶",
            BtNode::Condition(_) => "◇",
            BtNode::Wait(_) => "⏳",
            BtNode::Log(_) => "📝",
            BtNode::SetBlackboard { .. } => "✎",
            BtNode::CheckBlackboard { .. } => "≡",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            BtNode::Sequence => "Runs children left-to-right. Returns Success only if all children succeed. Returns Failure on first child failure.",
            BtNode::Selector => "Runs children left-to-right. Returns Success on first child success. Returns Failure if all children fail.",
            BtNode::Parallel(_) => "Runs all children simultaneously. Succeeds/fails based on policy.",
            BtNode::RandomSelector => "Picks a random child to execute each tick.",
            BtNode::WeightedSelector(_) => "Randomly selects a child weighted by assigned weights.",
            BtNode::Inverter => "Inverts the result of its child: Success→Failure, Failure→Success.",
            BtNode::Succeeder => "Always returns Success regardless of child result.",
            BtNode::Failer => "Always returns Failure regardless of child result.",
            BtNode::Repeat(_) => "Repeats its child node N times, then returns Success.",
            BtNode::RepeatUntilFail => "Repeats its child until the child returns Failure.",
            BtNode::Cooldown { .. } => "Prevents child from running more than once per cooldown period.",
            BtNode::Timeout(_) => "Fails the child if it doesn't complete within the time limit.",
            BtNode::Limiter { .. } => "Limits how many times the child can run per time interval.",
            BtNode::Action(_) => "Executes a specific action in the game world.",
            BtNode::Condition(_) => "Checks a condition, returning Success or Failure.",
            BtNode::Wait(_) => "Waits for a specified number of seconds, then returns Success.",
            BtNode::Log(_) => "Logs a message to the debug output, always returns Success.",
            BtNode::SetBlackboard { .. } => "Sets a key-value pair in the blackboard.",
            BtNode::CheckBlackboard { .. } => "Checks a blackboard value against a constant.",
        }
    }

    pub fn can_have_children(&self) -> bool {
        matches!(self,
            BtNode::Sequence | BtNode::Selector | BtNode::Parallel(_)
            | BtNode::RandomSelector | BtNode::WeightedSelector(_)
            | BtNode::Inverter | BtNode::Succeeder | BtNode::Failer
            | BtNode::Repeat(_) | BtNode::RepeatUntilFail
            | BtNode::Cooldown { .. } | BtNode::Timeout(_) | BtNode::Limiter { .. }
        )
    }

    pub fn is_decorator(&self) -> bool {
        matches!(self,
            BtNode::Inverter | BtNode::Succeeder | BtNode::Failer
            | BtNode::Repeat(_) | BtNode::RepeatUntilFail
            | BtNode::Cooldown { .. } | BtNode::Timeout(_) | BtNode::Limiter { .. }
        )
    }

    pub fn max_children(&self) -> Option<usize> {
        if self.is_decorator() { Some(1) } else { None }
    }

    pub fn default_label(&self) -> String {
        match self {
            BtNode::Action(k) => format!("{}", k.display_name()),
            BtNode::Condition(k) => format!("{}", k.display_name()),
            BtNode::Wait(s) => format!("Wait {}s", s),
            BtNode::Log(m) => if m.is_empty() { "Log".to_string() } else { format!("Log: {}", &m[..m.len().min(20)]) },
            BtNode::Repeat(n) => format!("Repeat x{}", n),
            BtNode::Cooldown { duration_secs, .. } => format!("Cooldown {}s", duration_secs),
            BtNode::Timeout(s) => format!("Timeout {}s", s),
            BtNode::Parallel(p) => format!("Parallel ({})", p),
            _ => self.type_name().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionKind {
    MoveTo { target: (f32, f32), speed: f32, tolerance: f32 },
    Attack { target_key: String, damage: f32, range: f32 },
    Flee { from_key: String, speed: f32, distance: f32 },
    Patrol { waypoints: Vec<(f32, f32)>, loop_patrol: bool },
    Idle { duration: f32 },
    PlayAnimation { clip_name: String, blend_time: f32, looping: bool },
    EmitParticles { emitter_name: String, count: u32, duration: f32 },
    TriggerEvent { event_name: String, payload: String },
    CallScript { script_path: String, function_name: String, args: String },
    UseAbility { ability_name: String, target_key: String },
    PickupItem { item_name: String, search_radius: f32 },
    DropItem { item_name: String },
    OpenDoor { door_key: String, force: bool },
    AlertAllies { radius: f32, alert_type: String },
    Surrender,
    FaceTarget { target_key: String, turn_speed: f32 },
    SetNavTarget { target_key: String, priority: i32 },
    StopMovement,
    PlaySound { sound_name: String, volume: f32, spatial: bool },
    SpawnEntity { entity_name: String, offset: (f32, f32) },
}

impl ActionKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            ActionKind::MoveTo { .. } => "Move To",
            ActionKind::Attack { .. } => "Attack",
            ActionKind::Flee { .. } => "Flee",
            ActionKind::Patrol { .. } => "Patrol",
            ActionKind::Idle { .. } => "Idle",
            ActionKind::PlayAnimation { .. } => "Play Animation",
            ActionKind::EmitParticles { .. } => "Emit Particles",
            ActionKind::TriggerEvent { .. } => "Trigger Event",
            ActionKind::CallScript { .. } => "Call Script",
            ActionKind::UseAbility { .. } => "Use Ability",
            ActionKind::PickupItem { .. } => "Pickup Item",
            ActionKind::DropItem { .. } => "Drop Item",
            ActionKind::OpenDoor { .. } => "Open Door",
            ActionKind::AlertAllies { .. } => "Alert Allies",
            ActionKind::Surrender => "Surrender",
            ActionKind::FaceTarget { .. } => "Face Target",
            ActionKind::SetNavTarget { .. } => "Set Nav Target",
            ActionKind::StopMovement => "Stop Movement",
            ActionKind::PlaySound { .. } => "Play Sound",
            ActionKind::SpawnEntity { .. } => "Spawn Entity",
        }
    }

    pub fn all_variants() -> Vec<ActionKind> {
        vec![
            ActionKind::MoveTo { target: (0.0, 0.0), speed: 5.0, tolerance: 0.5 },
            ActionKind::Attack { target_key: "target".to_string(), damage: 10.0, range: 2.0 },
            ActionKind::Flee { from_key: "threat".to_string(), speed: 7.0, distance: 15.0 },
            ActionKind::Patrol { waypoints: vec![], loop_patrol: true },
            ActionKind::Idle { duration: 1.0 },
            ActionKind::PlayAnimation { clip_name: "idle".to_string(), blend_time: 0.2, looping: true },
            ActionKind::EmitParticles { emitter_name: "sparks".to_string(), count: 20, duration: 1.0 },
            ActionKind::TriggerEvent { event_name: "on_spotted".to_string(), payload: "{}".to_string() },
            ActionKind::CallScript { script_path: "scripts/ai.lua".to_string(), function_name: "on_idle".to_string(), args: "".to_string() },
            ActionKind::UseAbility { ability_name: "fireball".to_string(), target_key: "target".to_string() },
            ActionKind::PickupItem { item_name: "sword".to_string(), search_radius: 3.0 },
            ActionKind::DropItem { item_name: "sword".to_string() },
            ActionKind::OpenDoor { door_key: "door_1".to_string(), force: false },
            ActionKind::AlertAllies { radius: 20.0, alert_type: "enemy_spotted".to_string() },
            ActionKind::Surrender,
            ActionKind::FaceTarget { target_key: "target".to_string(), turn_speed: 180.0 },
            ActionKind::SetNavTarget { target_key: "waypoint".to_string(), priority: 1 },
            ActionKind::StopMovement,
            ActionKind::PlaySound { sound_name: "alert.wav".to_string(), volume: 1.0, spatial: true },
            ActionKind::SpawnEntity { entity_name: "ally_guard".to_string(), offset: (0.0, 0.0) },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConditionKind {
    IsHealthBelow { threshold: f32, use_percentage: bool },
    IsTargetInRange { target_key: String, range: f32 },
    IsTargetVisible { target_key: String, use_los: bool },
    HasItem { item_name: String, min_count: u32 },
    IsOnCooldown { ability_name: String },
    IsAlerted,
    IsPathClear { direction: (f32, f32), distance: f32 },
    CanSeePlayer { fov_degrees: f32, max_distance: f32 },
    IsStunned,
    HasBlackboardKey { key: String },
    CheckFlag { flag_name: String },
    IsTargetDead { target_key: String },
    IsNearWall { distance: f32 },
    IsFacingTarget { target_key: String, tolerance_degrees: f32 },
    IsHealthAbove { threshold: f32, use_percentage: bool },
    IsTimeOfDay { hour_min: f32, hour_max: f32 },
}

impl ConditionKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            ConditionKind::IsHealthBelow { .. } => "Health Below",
            ConditionKind::IsTargetInRange { .. } => "Target In Range",
            ConditionKind::IsTargetVisible { .. } => "Target Visible",
            ConditionKind::HasItem { .. } => "Has Item",
            ConditionKind::IsOnCooldown { .. } => "Is On Cooldown",
            ConditionKind::IsAlerted => "Is Alerted",
            ConditionKind::IsPathClear { .. } => "Path Clear",
            ConditionKind::CanSeePlayer { .. } => "Can See Player",
            ConditionKind::IsStunned => "Is Stunned",
            ConditionKind::HasBlackboardKey { .. } => "Has BB Key",
            ConditionKind::CheckFlag { .. } => "Check Flag",
            ConditionKind::IsTargetDead { .. } => "Target Dead",
            ConditionKind::IsNearWall { .. } => "Near Wall",
            ConditionKind::IsFacingTarget { .. } => "Facing Target",
            ConditionKind::IsHealthAbove { .. } => "Health Above",
            ConditionKind::IsTimeOfDay { .. } => "Time Of Day",
        }
    }

    pub fn all_variants() -> Vec<ConditionKind> {
        vec![
            ConditionKind::IsHealthBelow { threshold: 0.3, use_percentage: true },
            ConditionKind::IsTargetInRange { target_key: "target".to_string(), range: 10.0 },
            ConditionKind::IsTargetVisible { target_key: "target".to_string(), use_los: true },
            ConditionKind::HasItem { item_name: "sword".to_string(), min_count: 1 },
            ConditionKind::IsOnCooldown { ability_name: "fireball".to_string() },
            ConditionKind::IsAlerted,
            ConditionKind::IsPathClear { direction: (1.0, 0.0), distance: 5.0 },
            ConditionKind::CanSeePlayer { fov_degrees: 90.0, max_distance: 20.0 },
            ConditionKind::IsStunned,
            ConditionKind::HasBlackboardKey { key: "target_pos".to_string() },
            ConditionKind::CheckFlag { flag_name: "quest_started".to_string() },
            ConditionKind::IsTargetDead { target_key: "target".to_string() },
            ConditionKind::IsNearWall { distance: 1.5 },
            ConditionKind::IsFacingTarget { target_key: "target".to_string(), tolerance_degrees: 15.0 },
            ConditionKind::IsHealthAbove { threshold: 0.8, use_percentage: true },
            ConditionKind::IsTimeOfDay { hour_min: 20.0, hour_max: 6.0 },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompareOp {
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
    Contains,
    NotContains,
}

impl CompareOp {
    pub fn symbol(&self) -> &'static str {
        match self {
            CompareOp::Equal => "==",
            CompareOp::NotEqual => "!=",
            CompareOp::LessThan => "<",
            CompareOp::LessOrEqual => "<=",
            CompareOp::GreaterThan => ">",
            CompareOp::GreaterOrEqual => ">=",
            CompareOp::Contains => "contains",
            CompareOp::NotContains => "!contains",
        }
    }

    pub fn all() -> Vec<CompareOp> {
        vec![
            CompareOp::Equal, CompareOp::NotEqual,
            CompareOp::LessThan, CompareOp::LessOrEqual,
            CompareOp::GreaterThan, CompareOp::GreaterOrEqual,
            CompareOp::Contains, CompareOp::NotContains,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BtNodeStatus {
    Idle,
    Running,
    Success,
    Failure,
}

impl BtNodeStatus {
    pub fn color(&self) -> Color32 {
        match self {
            BtNodeStatus::Idle => COLOR_STATUS_IDLE,
            BtNodeStatus::Running => COLOR_STATUS_RUNNING,
            BtNodeStatus::Success => COLOR_STATUS_SUCCESS,
            BtNodeStatus::Failure => COLOR_STATUS_FAILURE,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            BtNodeStatus::Idle => "Idle",
            BtNodeStatus::Running => "Running",
            BtNodeStatus::Success => "Success",
            BtNodeStatus::Failure => "Failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlackboardValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Vec2(f32, f32),
    EntityRef(String),
    List(Vec<BlackboardValue>),
}

impl BlackboardValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            BlackboardValue::Bool(_) => "Bool",
            BlackboardValue::Int(_) => "Int",
            BlackboardValue::Float(_) => "Float",
            BlackboardValue::Str(_) => "String",
            BlackboardValue::Vec2(_, _) => "Vec2",
            BlackboardValue::EntityRef(_) => "EntityRef",
            BlackboardValue::List(_) => "List",
        }
    }

    pub fn display_string(&self) -> String {
        match self {
            BlackboardValue::Bool(b) => b.to_string(),
            BlackboardValue::Int(i) => i.to_string(),
            BlackboardValue::Float(f) => format!("{:.3}", f),
            BlackboardValue::Str(s) => format!("\"{}\"", s),
            BlackboardValue::Vec2(x, y) => format!("({:.2}, {:.2})", x, y),
            BlackboardValue::EntityRef(r) => format!("@{}", r),
            BlackboardValue::List(items) => format!("[{} items]", items.len()),
        }
    }

    pub fn all_type_names() -> &'static [&'static str] {
        &["Bool", "Int", "Float", "String", "Vec2", "EntityRef"]
    }

    pub fn default_for_type(type_name: &str) -> BlackboardValue {
        match type_name {
            "Bool" => BlackboardValue::Bool(false),
            "Int" => BlackboardValue::Int(0),
            "Float" => BlackboardValue::Float(0.0),
            "String" => BlackboardValue::Str(String::new()),
            "Vec2" => BlackboardValue::Vec2(0.0, 0.0),
            "EntityRef" => BlackboardValue::EntityRef(String::new()),
            _ => BlackboardValue::Bool(false),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeCategory {
    Composite,
    Decorator,
    Action,
    Condition,
    Leaf,
}

impl NodeCategory {
    pub fn label(&self) -> &'static str {
        match self {
            NodeCategory::Composite => "Composites",
            NodeCategory::Decorator => "Decorators",
            NodeCategory::Action => "Actions",
            NodeCategory::Condition => "Conditions",
            NodeCategory::Leaf => "Leaves",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            NodeCategory::Composite => COLOR_COMPOSITE,
            NodeCategory::Decorator => COLOR_DECORATOR,
            NodeCategory::Action => COLOR_ACTION,
            NodeCategory::Condition => COLOR_CONDITION,
            NodeCategory::Leaf => COLOR_LEAF,
        }
    }

    pub fn dark_color(&self) -> Color32 {
        match self {
            NodeCategory::Composite => COLOR_COMPOSITE_DARK,
            NodeCategory::Decorator => COLOR_DECORATOR_DARK,
            NodeCategory::Action => COLOR_ACTION_DARK,
            NodeCategory::Condition => COLOR_CONDITION_DARK,
            NodeCategory::Leaf => COLOR_LEAF_DARK,
        }
    }

    pub fn all() -> Vec<NodeCategory> {
        vec![
            NodeCategory::Composite,
            NodeCategory::Decorator,
            NodeCategory::Action,
            NodeCategory::Condition,
            NodeCategory::Leaf,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutMode {
    Manual,
    AutoHorizontal,
    AutoVertical,
}

// ============================================================
// BLACKBOARD
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blackboard {
    pub entries: HashMap<String, BlackboardValue>,
    pub metadata: HashMap<String, BlackboardEntryMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEntryMeta {
    pub description: String,
    pub read_by: Vec<BtNodeId>,
    pub written_by: Vec<BtNodeId>,
    pub is_persistent: bool,
}

impl Default for BlackboardEntryMeta {
    fn default() -> Self {
        Self {
            description: String::new(),
            read_by: Vec::new(),
            written_by: Vec::new(),
            is_persistent: false,
        }
    }
}

impl Blackboard {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: BlackboardValue) {
        self.entries.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&BlackboardValue> {
        self.entries.get(key)
    }

    pub fn remove(&mut self, key: &str) {
        self.entries.remove(key);
        self.metadata.remove(key);
    }

    pub fn has_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn compare(&self, key: &str, op: &CompareOp, val: &BlackboardValue) -> bool {
        let Some(entry) = self.entries.get(key) else { return false; };
        match (entry, op, val) {
            (BlackboardValue::Bool(a), CompareOp::Equal, BlackboardValue::Bool(b)) => a == b,
            (BlackboardValue::Bool(a), CompareOp::NotEqual, BlackboardValue::Bool(b)) => a != b,
            (BlackboardValue::Int(a), CompareOp::Equal, BlackboardValue::Int(b)) => a == b,
            (BlackboardValue::Int(a), CompareOp::NotEqual, BlackboardValue::Int(b)) => a != b,
            (BlackboardValue::Int(a), CompareOp::LessThan, BlackboardValue::Int(b)) => a < b,
            (BlackboardValue::Int(a), CompareOp::LessOrEqual, BlackboardValue::Int(b)) => a <= b,
            (BlackboardValue::Int(a), CompareOp::GreaterThan, BlackboardValue::Int(b)) => a > b,
            (BlackboardValue::Int(a), CompareOp::GreaterOrEqual, BlackboardValue::Int(b)) => a >= b,
            (BlackboardValue::Float(a), CompareOp::LessThan, BlackboardValue::Float(b)) => a < b,
            (BlackboardValue::Float(a), CompareOp::LessOrEqual, BlackboardValue::Float(b)) => a <= b,
            (BlackboardValue::Float(a), CompareOp::GreaterThan, BlackboardValue::Float(b)) => a > b,
            (BlackboardValue::Float(a), CompareOp::GreaterOrEqual, BlackboardValue::Float(b)) => a >= b,
            (BlackboardValue::Float(a), CompareOp::Equal, BlackboardValue::Float(b)) => (a - b).abs() < 1e-6,
            (BlackboardValue::Str(a), CompareOp::Equal, BlackboardValue::Str(b)) => a == b,
            (BlackboardValue::Str(a), CompareOp::NotEqual, BlackboardValue::Str(b)) => a != b,
            (BlackboardValue::Str(a), CompareOp::Contains, BlackboardValue::Str(b)) => a.contains(b.as_str()),
            (BlackboardValue::Str(a), CompareOp::NotContains, BlackboardValue::Str(b)) => !a.contains(b.as_str()),
            _ => false,
        }
    }

    pub fn update_dependency(&mut self, key: &str, node_id: BtNodeId, is_write: bool) {
        let meta = self.metadata.entry(key.to_string()).or_default();
        if is_write {
            if !meta.written_by.contains(&node_id) {
                meta.written_by.push(node_id);
            }
        } else if !meta.read_by.contains(&node_id) {
            meta.read_by.push(node_id);
        }
    }
}

// ============================================================
// NODE DATA
// ============================================================

pub type BtNodeId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BtNodeData {
    pub id: BtNodeId,
    pub node_type: BtNode,
    pub children: Vec<BtNodeId>,
    pub parent: Option<BtNodeId>,
    pub position: (f32, f32),
    pub collapsed: bool,
    pub label: String,
    pub comment: String,
    pub status: BtNodeStatus,
    pub enabled: bool,
    pub custom_color: Option<[u8; 3]>,
    pub tags: Vec<String>,
    pub execution_count: u32,
    pub last_execution_tick: u32,
}

impl BtNodeData {
    pub fn new(id: BtNodeId, node_type: BtNode, position: (f32, f32)) -> Self {
        let label = node_type.default_label();
        Self {
            id,
            node_type,
            children: Vec::new(),
            parent: None,
            position,
            collapsed: false,
            label,
            comment: String::new(),
            status: BtNodeStatus::Idle,
            enabled: true,
            custom_color: None,
            tags: Vec::new(),
            execution_count: 0,
            last_execution_tick: 0,
        }
    }

    pub fn header_color(&self) -> Color32 {
        if let Some([r, g, b]) = self.custom_color {
            return Color32::from_rgb(r, g, b);
        }
        self.node_type.category().color()
    }

    pub fn header_dark_color(&self) -> Color32 {
        if let Some([r, g, b]) = self.custom_color {
            let c = Color32::from_rgb(r, g, b);
            return Color32::from_rgb(
                (r as f32 * 0.6) as u8,
                (g as f32 * 0.6) as u8,
                (b as f32 * 0.6) as u8,
            );
        }
        self.node_type.category().dark_color()
    }

    pub fn compute_height(&self) -> f32 {
        if self.collapsed {
            return NODE_HEADER_HEIGHT + 4.0;
        }
        let param_lines = self.param_line_count();
        NODE_HEADER_HEIGHT + param_lines as f32 * NODE_BODY_LINE_HEIGHT + 8.0
    }

    pub fn param_line_count(&self) -> usize {
        match &self.node_type {
            BtNode::Action(k) => match k {
                ActionKind::MoveTo { .. } => 3,
                ActionKind::Attack { .. } => 3,
                ActionKind::Flee { .. } => 3,
                ActionKind::Patrol { waypoints, .. } => 2 + waypoints.len().min(3),
                ActionKind::Idle { .. } => 1,
                ActionKind::PlayAnimation { .. } => 3,
                ActionKind::EmitParticles { .. } => 3,
                ActionKind::TriggerEvent { .. } => 2,
                ActionKind::CallScript { .. } => 3,
                ActionKind::UseAbility { .. } => 2,
                ActionKind::PickupItem { .. } => 2,
                ActionKind::DropItem { .. } => 1,
                ActionKind::OpenDoor { .. } => 2,
                ActionKind::AlertAllies { .. } => 2,
                ActionKind::Surrender => 0,
                ActionKind::FaceTarget { .. } => 2,
                ActionKind::SetNavTarget { .. } => 2,
                ActionKind::StopMovement => 0,
                ActionKind::PlaySound { .. } => 3,
                ActionKind::SpawnEntity { .. } => 2,
            },
            BtNode::Condition(k) => match k {
                ConditionKind::IsHealthBelow { .. } => 2,
                ConditionKind::IsTargetInRange { .. } => 2,
                ConditionKind::IsTargetVisible { .. } => 2,
                ConditionKind::HasItem { .. } => 2,
                ConditionKind::IsOnCooldown { .. } => 1,
                ConditionKind::IsAlerted => 0,
                ConditionKind::IsPathClear { .. } => 2,
                ConditionKind::CanSeePlayer { .. } => 2,
                ConditionKind::IsStunned => 0,
                ConditionKind::HasBlackboardKey { .. } => 1,
                ConditionKind::CheckFlag { .. } => 1,
                ConditionKind::IsTargetDead { .. } => 1,
                ConditionKind::IsNearWall { .. } => 1,
                ConditionKind::IsFacingTarget { .. } => 2,
                ConditionKind::IsHealthAbove { .. } => 2,
                ConditionKind::IsTimeOfDay { .. } => 2,
            },
            BtNode::Parallel(_) => 1,
            BtNode::Repeat(_) => 1,
            BtNode::Cooldown { .. } => 2,
            BtNode::Timeout(_) => 1,
            BtNode::Limiter { .. } => 2,
            BtNode::Wait(_) => 1,
            BtNode::Log(_) => 1,
            BtNode::SetBlackboard { .. } => 2,
            BtNode::CheckBlackboard { .. } => 3,
            _ => 0,
        }
    }

    pub fn param_lines(&self) -> Vec<(String, String)> {
        match &self.node_type {
            BtNode::Action(k) => match k {
                ActionKind::MoveTo { target, speed, tolerance } => vec![
                    ("Target".into(), format!("({:.1}, {:.1})", target.0, target.1)),
                    ("Speed".into(), format!("{:.1}", speed)),
                    ("Tolerance".into(), format!("{:.2}", tolerance)),
                ],
                ActionKind::Attack { target_key, damage, range } => vec![
                    ("Target".into(), target_key.clone()),
                    ("Damage".into(), format!("{:.1}", damage)),
                    ("Range".into(), format!("{:.1}", range)),
                ],
                ActionKind::Flee { from_key, speed, distance } => vec![
                    ("From".into(), from_key.clone()),
                    ("Speed".into(), format!("{:.1}", speed)),
                    ("Distance".into(), format!("{:.1}", distance)),
                ],
                ActionKind::Patrol { waypoints, loop_patrol } => {
                    let mut lines = vec![
                        ("Loop".into(), loop_patrol.to_string()),
                        ("Points".into(), waypoints.len().to_string()),
                    ];
                    for (i, wp) in waypoints.iter().take(3).enumerate() {
                        lines.push((format!("  [{}]", i), format!("({:.1},{:.1})", wp.0, wp.1)));
                    }
                    lines
                },
                ActionKind::Idle { duration } => vec![("Duration".into(), format!("{:.1}s", duration))],
                ActionKind::PlayAnimation { clip_name, blend_time, looping } => vec![
                    ("Clip".into(), clip_name.clone()),
                    ("Blend".into(), format!("{:.2}s", blend_time)),
                    ("Loop".into(), looping.to_string()),
                ],
                ActionKind::EmitParticles { emitter_name, count, duration } => vec![
                    ("Emitter".into(), emitter_name.clone()),
                    ("Count".into(), count.to_string()),
                    ("Duration".into(), format!("{:.1}s", duration)),
                ],
                ActionKind::TriggerEvent { event_name, payload } => vec![
                    ("Event".into(), event_name.clone()),
                    ("Payload".into(), if payload.len() > 16 { format!("{}...", &payload[..13]) } else { payload.clone() }),
                ],
                ActionKind::CallScript { script_path, function_name, args } => vec![
                    ("Script".into(), script_path.split('/').last().unwrap_or(script_path).to_string()),
                    ("Fn".into(), function_name.clone()),
                    ("Args".into(), if args.is_empty() { "none".into() } else { args.clone() }),
                ],
                ActionKind::UseAbility { ability_name, target_key } => vec![
                    ("Ability".into(), ability_name.clone()),
                    ("Target".into(), target_key.clone()),
                ],
                ActionKind::PickupItem { item_name, search_radius } => vec![
                    ("Item".into(), item_name.clone()),
                    ("Radius".into(), format!("{:.1}", search_radius)),
                ],
                ActionKind::DropItem { item_name } => vec![("Item".into(), item_name.clone())],
                ActionKind::OpenDoor { door_key, force } => vec![
                    ("Door".into(), door_key.clone()),
                    ("Force".into(), force.to_string()),
                ],
                ActionKind::AlertAllies { radius, alert_type } => vec![
                    ("Radius".into(), format!("{:.1}", radius)),
                    ("Type".into(), alert_type.clone()),
                ],
                ActionKind::Surrender => vec![],
                ActionKind::FaceTarget { target_key, turn_speed } => vec![
                    ("Target".into(), target_key.clone()),
                    ("Speed".into(), format!("{:.0}°/s", turn_speed)),
                ],
                ActionKind::SetNavTarget { target_key, priority } => vec![
                    ("Target".into(), target_key.clone()),
                    ("Priority".into(), priority.to_string()),
                ],
                ActionKind::StopMovement => vec![],
                ActionKind::PlaySound { sound_name, volume, spatial } => vec![
                    ("Sound".into(), sound_name.clone()),
                    ("Vol".into(), format!("{:.2}", volume)),
                    ("Spatial".into(), spatial.to_string()),
                ],
                ActionKind::SpawnEntity { entity_name, offset } => vec![
                    ("Entity".into(), entity_name.clone()),
                    ("Offset".into(), format!("({:.1},{:.1})", offset.0, offset.1)),
                ],
            },
            BtNode::Condition(k) => match k {
                ConditionKind::IsHealthBelow { threshold, use_percentage } => vec![
                    ("Threshold".into(), if *use_percentage { format!("{:.0}%", threshold * 100.0) } else { format!("{:.1}", threshold) }),
                    ("Mode".into(), if *use_percentage { "Percent".into() } else { "Absolute".into() }),
                ],
                ConditionKind::IsTargetInRange { target_key, range } => vec![
                    ("Target".into(), target_key.clone()),
                    ("Range".into(), format!("{:.1}", range)),
                ],
                ConditionKind::IsTargetVisible { target_key, use_los } => vec![
                    ("Target".into(), target_key.clone()),
                    ("LOS".into(), use_los.to_string()),
                ],
                ConditionKind::HasItem { item_name, min_count } => vec![
                    ("Item".into(), item_name.clone()),
                    ("Min".into(), min_count.to_string()),
                ],
                ConditionKind::IsOnCooldown { ability_name } => vec![("Ability".into(), ability_name.clone())],
                ConditionKind::IsAlerted => vec![],
                ConditionKind::IsPathClear { direction, distance } => vec![
                    ("Dir".into(), format!("({:.1},{:.1})", direction.0, direction.1)),
                    ("Dist".into(), format!("{:.1}", distance)),
                ],
                ConditionKind::CanSeePlayer { fov_degrees, max_distance } => vec![
                    ("FOV".into(), format!("{:.0}°", fov_degrees)),
                    ("Dist".into(), format!("{:.1}", max_distance)),
                ],
                ConditionKind::IsStunned => vec![],
                ConditionKind::HasBlackboardKey { key } => vec![("Key".into(), key.clone())],
                ConditionKind::CheckFlag { flag_name } => vec![("Flag".into(), flag_name.clone())],
                ConditionKind::IsTargetDead { target_key } => vec![("Target".into(), target_key.clone())],
                ConditionKind::IsNearWall { distance } => vec![("Distance".into(), format!("{:.1}", distance))],
                ConditionKind::IsFacingTarget { target_key, tolerance_degrees } => vec![
                    ("Target".into(), target_key.clone()),
                    ("Tol".into(), format!("{:.0}°", tolerance_degrees)),
                ],
                ConditionKind::IsHealthAbove { threshold, use_percentage } => vec![
                    ("Threshold".into(), if *use_percentage { format!("{:.0}%", threshold * 100.0) } else { format!("{:.1}", threshold) }),
                    ("Mode".into(), if *use_percentage { "Percent".into() } else { "Absolute".into() }),
                ],
                ConditionKind::IsTimeOfDay { hour_min, hour_max } => vec![
                    ("From".into(), format!("{:.0}:00", hour_min)),
                    ("To".into(), format!("{:.0}:00", hour_max)),
                ],
            },
            BtNode::Parallel(policy) => vec![("Policy".into(), policy.to_string())],
            BtNode::Repeat(n) => vec![("Count".into(), n.to_string())],
            BtNode::Cooldown { duration_secs, shared_key } => vec![
                ("Duration".into(), format!("{:.1}s", duration_secs)),
                ("Key".into(), shared_key.clone().unwrap_or_else(|| "local".into())),
            ],
            BtNode::Timeout(s) => vec![("Limit".into(), format!("{:.1}s", s))],
            BtNode::Limiter { max_per_interval, interval_secs } => vec![
                ("Max".into(), max_per_interval.to_string()),
                ("Per".into(), format!("{:.1}s", interval_secs)),
            ],
            BtNode::Wait(s) => vec![("Secs".into(), format!("{:.2}", s))],
            BtNode::Log(msg) => vec![("Msg".into(), if msg.len() > 18 { format!("{}...", &msg[..15]) } else { msg.clone() })],
            BtNode::SetBlackboard { key, value } => vec![
                ("Key".into(), key.clone()),
                ("Value".into(), value.display_string()),
            ],
            BtNode::CheckBlackboard { key, op, value } => vec![
                ("Key".into(), key.clone()),
                ("Op".into(), op.symbol().into()),
                ("Value".into(), value.display_string()),
            ],
            _ => vec![],
        }
    }
}

// ============================================================
// BEHAVIOR TREE
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorTree {
    pub name: String,
    pub description: String,
    pub root: Option<BtNodeId>,
    pub nodes: HashMap<BtNodeId, BtNodeData>,
    pub tags: Vec<String>,
    pub version: u32,
    pub author: String,
    pub created_at: String,
    pub modified_at: String,
}

impl BehaviorTree {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            root: None,
            nodes: HashMap::new(),
            tags: Vec::new(),
            version: 1,
            author: String::new(),
            created_at: "2026-01-01".into(),
            modified_at: "2026-01-01".into(),
        }
    }

    pub fn new_with_root(name: impl Into<String>, id_counter: &mut BtNodeId) -> Self {
        let mut tree = Self::new(name);
        let root_id = *id_counter;
        *id_counter += 1;
        let mut root = BtNodeData::new(root_id, BtNode::Sequence, (400.0, 60.0));
        root.label = "Root".to_string();
        tree.root = Some(root_id);
        tree.nodes.insert(root_id, root);
        tree
    }

    pub fn get_node(&self, id: BtNodeId) -> Option<&BtNodeData> {
        self.nodes.get(&id)
    }

    pub fn get_node_mut(&mut self, id: BtNodeId) -> Option<&mut BtNodeData> {
        self.nodes.get_mut(&id)
    }

    pub fn add_node(&mut self, node: BtNodeData) {
        self.nodes.insert(node.id, node);
    }

    pub fn remove_node(&mut self, id: BtNodeId) {
        if let Some(node) = self.nodes.remove(&id) {
            // Remove from parent's children list
            if let Some(parent_id) = node.parent {
                if let Some(parent) = self.nodes.get_mut(&parent_id) {
                    parent.children.retain(|&c| c != id);
                }
            }
            // Update root if we removed it
            if self.root == Some(id) {
                self.root = None;
            }
            // Recursively remove children
            let children: Vec<BtNodeId> = node.children.clone();
            for child_id in children {
                self.remove_node(child_id);
            }
        }
    }

    pub fn add_child(&mut self, parent_id: BtNodeId, child_id: BtNodeId) -> bool {
        let can_add = self.nodes.get(&parent_id)
            .map(|p| {
                let max = p.node_type.max_children();
                max.map_or(true, |m| p.children.len() < m)
            })
            .unwrap_or(false);

        if can_add {
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                parent.children.push(child_id);
            }
            if let Some(child) = self.nodes.get_mut(&child_id) {
                child.parent = Some(parent_id);
            }
            true
        } else {
            false
        }
    }

    pub fn remove_child(&mut self, parent_id: BtNodeId, child_id: BtNodeId) {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.retain(|&c| c != child_id);
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = None;
        }
    }

    pub fn reorder_child(&mut self, parent_id: BtNodeId, child_id: BtNodeId, delta: i32) {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if let Some(idx) = parent.children.iter().position(|&c| c == child_id) {
                let new_idx = (idx as i32 + delta).max(0) as usize;
                let new_idx = new_idx.min(parent.children.len() - 1);
                parent.children.remove(idx);
                parent.children.insert(new_idx, child_id);
            }
        }
    }

    pub fn node_depth(&self, id: BtNodeId) -> u32 {
        let mut depth = 0u32;
        let mut current = id;
        loop {
            match self.nodes.get(&current).and_then(|n| n.parent) {
                Some(parent) => {
                    depth += 1;
                    current = parent;
                    if depth > 1000 { break; } // cycle guard
                },
                None => break,
            }
        }
        depth
    }

    pub fn collect_subtree(&self, root_id: BtNodeId) -> Vec<BtNodeId> {
        let mut result = Vec::new();
        let mut stack = vec![root_id];
        while let Some(id) = stack.pop() {
            result.push(id);
            if let Some(node) = self.nodes.get(&id) {
                for &child in &node.children {
                    stack.push(child);
                }
            }
        }
        result
    }

    pub fn bounds(&self) -> Option<Rect> {
        if self.nodes.is_empty() { return None; }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for node in self.nodes.values() {
            let h = node.compute_height();
            min_x = min_x.min(node.position.0);
            min_y = min_y.min(node.position.1);
            max_x = max_x.max(node.position.0 + NODE_WIDTH);
            max_y = max_y.max(node.position.1 + h);
        }
        Some(Rect::from_min_max(
            Pos2::new(min_x, min_y),
            Pos2::new(max_x, max_y),
        ))
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.root.is_none() {
            errors.push("Tree has no root node".into());
        }
        for node in self.nodes.values() {
            // Check children exist
            for &child_id in &node.children {
                if !self.nodes.contains_key(&child_id) {
                    errors.push(format!("Node '{}' references non-existent child {}", node.label, child_id));
                }
            }
            // Check parent exists
            if let Some(parent_id) = node.parent {
                if !self.nodes.contains_key(&parent_id) {
                    errors.push(format!("Node '{}' references non-existent parent {}", node.label, parent_id));
                }
            }
            // Check decorator max children
            if node.node_type.is_decorator() && node.children.len() > 1 {
                errors.push(format!("Decorator node '{}' has {} children (max 1)", node.label, node.children.len()));
            }
            // Check leaf nodes have no children
            match &node.node_type {
                BtNode::Action(_) | BtNode::Condition(_) | BtNode::Wait(_)
                | BtNode::Log(_) | BtNode::SetBlackboard { .. } | BtNode::CheckBlackboard { .. } => {
                    if !node.children.is_empty() {
                        errors.push(format!("Leaf node '{}' has children", node.label));
                    }
                },
                _ => {}
            }
        }
        errors
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
    }

    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

// ============================================================
// LAYOUT ALGORITHMS
// ============================================================

fn auto_layout_horizontal(tree: &mut BehaviorTree) {
    let Some(root_id) = tree.root else { return; };
    let start_x = 100.0_f32;
    let start_y = 60.0_f32;
    let y_step = 120.0_f32;
    let x_padding = 40.0_f32;

    fn measure_subtree_width(tree: &BehaviorTree, id: BtNodeId, x_padding: f32) -> f32 {
        let Some(node) = tree.nodes.get(&id) else { return NODE_WIDTH; };
        if node.children.is_empty() {
            NODE_WIDTH + x_padding
        } else {
            let total: f32 = node.children.iter()
                .map(|&c| measure_subtree_width(tree, c, x_padding))
                .sum();
            total.max(NODE_WIDTH + x_padding)
        }
    }

    fn place_nodes(tree: &mut BehaviorTree, id: BtNodeId, x: f32, y: f32, x_padding: f32, y_step: f32) -> f32 {
        let children: Vec<BtNodeId> = tree.nodes.get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        if children.is_empty() {
            if let Some(node) = tree.nodes.get_mut(&id) {
                node.position = (x, y);
            }
            return NODE_WIDTH + x_padding;
        }

        let child_y = y + y_step;
        let mut child_x = x;
        let mut total_width = 0.0_f32;
        let mut child_centers = Vec::new();

        for child_id in &children {
            let w = {
                // Re-borrow safely
                fn measure(tree: &BehaviorTree, id: BtNodeId, xpad: f32) -> f32 {
                    let Some(node) = tree.nodes.get(&id) else { return NODE_WIDTH; };
                    if node.children.is_empty() {
                        NODE_WIDTH + xpad
                    } else {
                        let t: f32 = node.children.iter().map(|&c| measure(tree, c, xpad)).sum();
                        t.max(NODE_WIDTH + xpad)
                    }
                }
                measure(tree, *child_id, x_padding)
            };
            let placed_w = place_nodes(tree, *child_id, child_x, child_y, x_padding, y_step);
            child_centers.push(child_x + w / 2.0 - x_padding / 2.0);
            child_x += placed_w;
            total_width += placed_w;
        }

        // Center parent over children
        let center_x = if !child_centers.is_empty() {
            (child_centers.first().unwrap() + child_centers.last().unwrap()) / 2.0 - NODE_WIDTH / 2.0
        } else {
            x
        };

        if let Some(node) = tree.nodes.get_mut(&id) {
            node.position = (center_x, y);
        }

        total_width.max(NODE_WIDTH + x_padding)
    }

    let _ = x_padding; // used below
    place_nodes(tree, root_id, start_x, start_y, 40.0, y_step);
}

fn auto_layout_vertical(tree: &mut BehaviorTree) {
    let Some(root_id) = tree.root else { return; };
    let start_x = 60.0_f32;
    let start_y = 100.0_f32;
    let x_step = 220.0_f32;
    let y_padding = 20.0_f32;

    fn measure_subtree_height(tree: &BehaviorTree, id: BtNodeId, y_padding: f32) -> f32 {
        let Some(node) = tree.nodes.get(&id) else { return NODE_MIN_HEIGHT; };
        let h = node.compute_height();
        if node.children.is_empty() {
            h + y_padding
        } else {
            let total: f32 = node.children.iter()
                .map(|&c| measure_subtree_height(tree, c, y_padding))
                .sum();
            total.max(h + y_padding)
        }
    }

    fn place_nodes_vertical(tree: &mut BehaviorTree, id: BtNodeId, x: f32, y: f32, x_step: f32, y_padding: f32) -> f32 {
        let children: Vec<BtNodeId> = tree.nodes.get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        let self_height = tree.nodes.get(&id).map(|n| n.compute_height()).unwrap_or(NODE_MIN_HEIGHT);

        if children.is_empty() {
            if let Some(node) = tree.nodes.get_mut(&id) {
                node.position = (x, y);
            }
            return self_height + y_padding;
        }

        let child_x = x + x_step;
        let mut child_y = y;
        let mut total_height = 0.0_f32;

        for child_id in &children {
            let placed_h = place_nodes_vertical(tree, *child_id, child_x, child_y, x_step, y_padding);
            child_y += placed_h;
            total_height += placed_h;
        }

        // Center parent vertically over children
        let center_y = y + total_height / 2.0 - self_height / 2.0;
        if let Some(node) = tree.nodes.get_mut(&id) {
            node.position = (x, center_y);
        }

        total_height.max(self_height + y_padding)
    }

    let _ = y_padding;
    place_nodes_vertical(tree, root_id, start_x, start_y, x_step, 20.0);
}

fn fit_to_view(tree: &BehaviorTree, canvas_rect: Rect, zoom: &mut f32, offset: &mut Vec2) {
    let Some(bounds) = tree.bounds() else { return; };
    let padding = 60.0_f32;
    let bounds_w = bounds.width() + padding * 2.0;
    let bounds_h = bounds.height() + padding * 2.0;
    let canvas_w = canvas_rect.width();
    let canvas_h = canvas_rect.height();

    *zoom = (canvas_w / bounds_w).min(canvas_h / bounds_h).min(MAX_ZOOM).max(MIN_ZOOM);
    let center_x = bounds.center().x * *zoom;
    let center_y = bounds.center().y * *zoom;
    *offset = Vec2::new(
        canvas_rect.center().x - center_x,
        canvas_rect.center().y - center_y,
    );
}

// ============================================================
// SIMULATION
// ============================================================

#[derive(Debug, Clone)]
pub struct SimulationState {
    pub running: bool,
    pub tick: u32,
    pub speed: f32,
    pub accumulated_time: f32,
    pub active_node_path: Vec<BtNodeId>,
}

impl SimulationState {
    pub fn new() -> Self {
        Self {
            running: false,
            tick: 0,
            speed: 1.0,
            accumulated_time: 0.0,
            active_node_path: Vec::new(),
        }
    }

    pub fn tick_interval(&self) -> f32 {
        1.0 / (4.0 * self.speed)
    }
}

fn simulate_tick(
    tree: &mut BehaviorTree,
    blackboard: &mut Blackboard,
    breakpoints: &HashSet<BtNodeId>,
    log: &mut Vec<(u32, BtNodeId, BtNodeStatus)>,
    tick: u32,
) -> bool {
    let Some(root_id) = tree.root else { return false; };
    let hit_breakpoint = false;
    let _ = breakpoints;
    let result = evaluate_node(tree, blackboard, root_id, tick, log);
    if let Some(node) = tree.nodes.get_mut(&root_id) {
        node.status = result;
    }
    hit_breakpoint
}

fn evaluate_node(
    tree: &mut BehaviorTree,
    blackboard: &mut Blackboard,
    id: BtNodeId,
    tick: u32,
    log: &mut Vec<(u32, BtNodeId, BtNodeStatus)>,
) -> BtNodeStatus {
    let node_type = tree.nodes.get(&id).map(|n| n.node_type.clone());
    let node_enabled = tree.nodes.get(&id).map(|n| n.enabled).unwrap_or(false);

    if !node_enabled {
        return BtNodeStatus::Failure;
    }

    let Some(nt) = node_type else { return BtNodeStatus::Failure; };
    let children: Vec<BtNodeId> = tree.nodes.get(&id).map(|n| n.children.clone()).unwrap_or_default();

    let status = match &nt {
        BtNode::Sequence => {
            let mut result = BtNodeStatus::Success;
            for child_id in &children {
                let child_result = evaluate_node(tree, blackboard, *child_id, tick, log);
                match child_result {
                    BtNodeStatus::Failure => { result = BtNodeStatus::Failure; break; },
                    BtNodeStatus::Running => { result = BtNodeStatus::Running; break; },
                    BtNodeStatus::Success => {},
                    BtNodeStatus::Idle => {},
                }
            }
            result
        },
        BtNode::Selector => {
            let mut result = BtNodeStatus::Failure;
            for child_id in &children {
                let child_result = evaluate_node(tree, blackboard, *child_id, tick, log);
                match child_result {
                    BtNodeStatus::Success => { result = BtNodeStatus::Success; break; },
                    BtNodeStatus::Running => { result = BtNodeStatus::Running; break; },
                    BtNodeStatus::Failure => {},
                    BtNodeStatus::Idle => {},
                }
            }
            result
        },
        BtNode::Parallel(policy) => {
            let mut successes = 0usize;
            let mut failures = 0usize;
            let n = children.len();
            for child_id in &children {
                let child_result = evaluate_node(tree, blackboard, *child_id, tick, log);
                match child_result {
                    BtNodeStatus::Success => successes += 1,
                    BtNodeStatus::Failure => failures += 1,
                    _ => {},
                }
            }
            match policy {
                ParallelPolicy::RequireAll => {
                    if successes == n { BtNodeStatus::Success }
                    else if failures > 0 { BtNodeStatus::Failure }
                    else { BtNodeStatus::Running }
                },
                ParallelPolicy::RequireOne => {
                    if successes > 0 { BtNodeStatus::Success }
                    else if failures == n { BtNodeStatus::Failure }
                    else { BtNodeStatus::Running }
                },
                ParallelPolicy::RequireN(req) => {
                    if successes >= *req { BtNodeStatus::Success }
                    else if failures > n - req { BtNodeStatus::Failure }
                    else { BtNodeStatus::Running }
                },
            }
        },
        BtNode::RandomSelector => {
            if !children.is_empty() {
                let idx = (tick as usize + id as usize) % children.len();
                evaluate_node(tree, blackboard, children[idx], tick, log)
            } else {
                BtNodeStatus::Failure
            }
        },
        BtNode::WeightedSelector(weights) => {
            if !children.is_empty() {
                let total: f32 = weights.iter().copied().sum::<f32>().max(0.0001);
                let r = ((tick * 6271 + id * 1337) % 10000) as f32 / 10000.0 * total;
                let mut acc = 0.0_f32;
                let mut chosen = children[0];
                for (i, &w) in weights.iter().enumerate() {
                    acc += w;
                    if r <= acc {
                        chosen = children[i.min(children.len() - 1)];
                        break;
                    }
                }
                evaluate_node(tree, blackboard, chosen, tick, log)
            } else {
                BtNodeStatus::Failure
            }
        },
        BtNode::Inverter => {
            if let Some(&child_id) = children.first() {
                match evaluate_node(tree, blackboard, child_id, tick, log) {
                    BtNodeStatus::Success => BtNodeStatus::Failure,
                    BtNodeStatus::Failure => BtNodeStatus::Success,
                    s => s,
                }
            } else { BtNodeStatus::Failure }
        },
        BtNode::Succeeder => {
            if let Some(&child_id) = children.first() {
                let _ = evaluate_node(tree, blackboard, child_id, tick, log);
            }
            BtNodeStatus::Success
        },
        BtNode::Failer => {
            if let Some(&child_id) = children.first() {
                let _ = evaluate_node(tree, blackboard, child_id, tick, log);
            }
            BtNodeStatus::Failure
        },
        BtNode::Repeat(count) => {
            let execution_count = tree.nodes.get(&id).map(|n| n.execution_count).unwrap_or(0);
            if execution_count >= *count {
                if let Some(node) = tree.nodes.get_mut(&id) {
                    node.execution_count = 0;
                }
                BtNodeStatus::Success
            } else {
                if let Some(&child_id) = children.first() {
                    let r = evaluate_node(tree, blackboard, child_id, tick, log);
                    if r == BtNodeStatus::Success {
                        if let Some(node) = tree.nodes.get_mut(&id) {
                            node.execution_count += 1;
                        }
                    }
                    BtNodeStatus::Running
                } else {
                    BtNodeStatus::Failure
                }
            }
        },
        BtNode::RepeatUntilFail => {
            if let Some(&child_id) = children.first() {
                match evaluate_node(tree, blackboard, child_id, tick, log) {
                    BtNodeStatus::Failure => BtNodeStatus::Success,
                    _ => BtNodeStatus::Running,
                }
            } else { BtNodeStatus::Success }
        },
        BtNode::Cooldown { duration_secs, shared_key } => {
            let last_tick = tree.nodes.get(&id).map(|n| n.last_execution_tick).unwrap_or(0);
            let ticks_needed = (*duration_secs * 4.0) as u32;
            if tick.saturating_sub(last_tick) < ticks_needed {
                BtNodeStatus::Failure
            } else {
                if let Some(&child_id) = children.first() {
                    let r = evaluate_node(tree, blackboard, child_id, tick, log);
                    if r == BtNodeStatus::Success {
                        if let Some(node) = tree.nodes.get_mut(&id) {
                            node.last_execution_tick = tick;
                        }
                    }
                    r
                } else {
                    BtNodeStatus::Success
                }
            }
        },
        BtNode::Timeout(secs) => {
            let start_tick = tree.nodes.get(&id).map(|n| n.last_execution_tick).unwrap_or(tick);
            let max_ticks = (*secs * 4.0) as u32;
            if tick.saturating_sub(start_tick) > max_ticks {
                BtNodeStatus::Failure
            } else {
                if let Some(&child_id) = children.first() {
                    evaluate_node(tree, blackboard, child_id, tick, log)
                } else { BtNodeStatus::Success }
            }
        },
        BtNode::Limiter { max_per_interval, interval_secs } => {
            let exec_count = tree.nodes.get(&id).map(|n| n.execution_count).unwrap_or(0);
            let ticks_per_interval = (*interval_secs * 4.0) as u32;
            let interval_num = tick / ticks_per_interval.max(1);
            let last_interval = tree.nodes.get(&id).map(|n| n.last_execution_tick).unwrap_or(0);
            let current_count = if last_interval == interval_num { exec_count } else { 0 };
            if current_count >= *max_per_interval {
                BtNodeStatus::Failure
            } else {
                if let Some(&child_id) = children.first() {
                    let r = evaluate_node(tree, blackboard, child_id, tick, log);
                    if let Some(node) = tree.nodes.get_mut(&id) {
                        node.last_execution_tick = interval_num;
                        node.execution_count = current_count + 1;
                    }
                    r
                } else { BtNodeStatus::Success }
            }
        },
        BtNode::Action(kind) => {
            // Simulate: most actions return Running for a few ticks, then Success
            let exec_count = tree.nodes.get(&id).map(|n| n.execution_count).unwrap_or(0);
            let done_ticks = match kind {
                ActionKind::Idle { duration } => (*duration * 4.0) as u32,
                ActionKind::MoveTo { .. } => 8,
                ActionKind::Attack { .. } => 3,
                ActionKind::PlayAnimation { .. } => 6,
                _ => 2,
            };
            if exec_count >= done_ticks {
                if let Some(node) = tree.nodes.get_mut(&id) {
                    node.execution_count = 0;
                }
                BtNodeStatus::Success
            } else {
                if let Some(node) = tree.nodes.get_mut(&id) {
                    node.execution_count += 1;
                }
                BtNodeStatus::Running
            }
        },
        BtNode::Condition(kind) => {
            let result = match kind {
                ConditionKind::IsAlerted => blackboard.get("alerted").map(|v| matches!(v, BlackboardValue::Bool(true))).unwrap_or(false),
                ConditionKind::IsStunned => blackboard.get("stunned").map(|v| matches!(v, BlackboardValue::Bool(true))).unwrap_or(false),
                ConditionKind::HasBlackboardKey { key } => blackboard.has_key(key),
                ConditionKind::IsHealthBelow { threshold, use_percentage } => {
                    if let Some(BlackboardValue::Float(hp)) = blackboard.get("health") {
                        let val = if *use_percentage { *hp as f32 } else { *hp as f32 };
                        val < *threshold
                    } else { false }
                },
                ConditionKind::IsHealthAbove { threshold, use_percentage } => {
                    if let Some(BlackboardValue::Float(hp)) = blackboard.get("health") {
                        (*hp as f32) > *threshold
                    } else { false }
                },
                _ => (tick + id) % 3 != 0, // Pseudo-random for simulation
            };
            if result { BtNodeStatus::Success } else { BtNodeStatus::Failure }
        },
        BtNode::Wait(secs) => {
            let exec = tree.nodes.get(&id).map(|n| n.execution_count).unwrap_or(0);
            let needed = (*secs * 4.0) as u32;
            if exec >= needed {
                if let Some(node) = tree.nodes.get_mut(&id) { node.execution_count = 0; }
                BtNodeStatus::Success
            } else {
                if let Some(node) = tree.nodes.get_mut(&id) { node.execution_count += 1; }
                BtNodeStatus::Running
            }
        },
        BtNode::Log(msg) => {
            // In editor simulation, we just note it ran
            BtNodeStatus::Success
        },
        BtNode::SetBlackboard { key, value } => {
            blackboard.set(key.clone(), value.clone());
            BtNodeStatus::Success
        },
        BtNode::CheckBlackboard { key, op, value } => {
            if blackboard.compare(key, op, value) {
                BtNodeStatus::Success
            } else {
                BtNodeStatus::Failure
            }
        },
    };

    if let Some(node) = tree.nodes.get_mut(&id) {
        node.status = status.clone();
        node.last_execution_tick = tick;
    }

    while log.len() >= EXECUTION_LOG_MAX {
        log.remove(0);
    }
    log.push((tick, id, status.clone()));

    status
}

// ============================================================
// EDITOR STATE
// ============================================================

#[derive(Debug, Clone)]
pub struct SelectionBox {
    pub start: Pos2,
    pub end: Pos2,
}

impl SelectionBox {
    pub fn rect(&self) -> Rect {
        Rect::from_two_pos(self.start, self.end)
    }
}

#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    Node(BtNodeId),
    Canvas(Pos2),
    Connection(BtNodeId, BtNodeId),
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub target: ContextMenuTarget,
    pub position: Pos2,
    pub open: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DragState {
    None,
    DraggingNode { id: BtNodeId, offset: Vec2 },
    PanCanvas { last_pos: Pos2 },
    SelectionBox { start: Pos2 },
    ConnectingFrom { from_id: BtNodeId, current_pos: Pos2 },
}

#[derive(Debug, Clone)]
pub struct NewNodeDialog {
    pub open: bool,
    pub category: NodeCategory,
    pub position: Pos2,
    pub parent_id: Option<BtNodeId>,
}

#[derive(Debug, Clone)]
pub struct BlackboardEditorState {
    pub new_key: String,
    pub new_type: String,
    pub editing_key: Option<String>,
    pub edit_buffer: String,
    pub show_add_row: bool,
}

impl Default for BlackboardEditorState {
    fn default() -> Self {
        Self {
            new_key: String::new(),
            new_type: "Bool".into(),
            editing_key: None,
            edit_buffer: String::new(),
            show_add_row: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeRenameState {
    pub editing_idx: Option<usize>,
    pub buffer: String,
}

impl Default for TreeRenameState {
    fn default() -> Self {
        Self { editing_idx: None, buffer: String::new() }
    }
}

#[derive(Debug, Clone)]
pub struct ImportExportState {
    pub show_export: bool,
    pub show_import: bool,
    pub export_text: String,
    pub import_text: String,
    pub import_error: Option<String>,
}

impl Default for ImportExportState {
    fn default() -> Self {
        Self {
            show_export: false,
            show_import: false,
            export_text: String::new(),
            import_text: String::new(),
            import_error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidationState {
    pub show_errors: bool,
    pub errors: Vec<String>,
    pub last_validated_tick: u32,
}

impl Default for ValidationState {
    fn default() -> Self {
        Self { show_errors: false, errors: Vec::new(), last_validated_tick: 0 }
    }
}

pub struct BehaviorTreeEditor {
    pub trees: Vec<BehaviorTree>,
    pub active_tree: usize,
    pub selected_nodes: HashSet<BtNodeId>,
    pub selected_node: Option<BtNodeId>,
    pub hovered_node: Option<BtNodeId>,
    pub canvas_offset: Vec2,
    pub canvas_zoom: f32,
    pub drag_state: DragState,
    pub drag_offset: Vec2,
    pub connecting_from: Option<BtNodeId>,
    pub show_blackboard: bool,
    pub show_node_palette: bool,
    pub show_properties: bool,
    pub show_simulation: bool,
    pub search_filter: String,
    pub palette_search: String,
    pub simulation: SimulationState,
    pub breakpoints: HashSet<BtNodeId>,
    pub execution_log: Vec<(u32, BtNodeId, BtNodeStatus)>,
    pub blackboard: Blackboard,
    pub clipboard_node: Option<BtNodeData>,
    pub clipboard_subtree: Option<Vec<BtNodeData>>,
    pub layout_mode: LayoutMode,
    pub node_id_counter: BtNodeId,
    pub context_menu: Option<ContextMenuState>,
    pub selection_box: Option<SelectionBox>,
    pub new_node_dialog: Option<NewNodeDialog>,
    pub bb_editor: BlackboardEditorState,
    pub tree_rename: TreeRenameState,
    pub import_export: ImportExportState,
    pub validation: ValidationState,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub show_minimap: bool,
    pub palette_collapsed_categories: HashSet<String>,
    pub properties_scroll: f32,
    pub log_scroll_to_bottom: bool,
    pub frame_counter: u64,
    pub last_tick_time: f64,
}

impl BehaviorTreeEditor {
    pub fn new() -> Self {
        let mut counter = 1u32;
        let mut default_tree = BehaviorTree::new_with_root("Main", &mut counter);

        // Add some example nodes
        let seq_id = counter;
        counter += 1;
        let mut seq = BtNodeData::new(seq_id, BtNode::Selector, (300.0, 200.0));
        seq.label = "Combat Selector".to_string();
        default_tree.nodes.insert(seq_id, seq);

        let cond_id = counter;
        counter += 1;
        let mut cond = BtNodeData::new(cond_id, BtNode::Condition(
            ConditionKind::CanSeePlayer { fov_degrees: 90.0, max_distance: 20.0 }
        ), (180.0, 340.0));
        cond.label = "Can See Player".to_string();
        default_tree.nodes.insert(cond_id, cond);

        let act_id = counter;
        counter += 1;
        let mut act = BtNodeData::new(act_id, BtNode::Action(
            ActionKind::Attack { target_key: "player".to_string(), damage: 10.0, range: 2.0 }
        ), (420.0, 340.0));
        act.label = "Attack Player".to_string();
        default_tree.nodes.insert(act_id, act);

        let idle_id = counter;
        counter += 1;
        let mut idle = BtNodeData::new(idle_id, BtNode::Action(
            ActionKind::Idle { duration: 2.0 }
        ), (640.0, 340.0));
        idle.label = "Idle".to_string();
        default_tree.nodes.insert(idle_id, idle);

        // Wire them up
        if let Some(root_id) = default_tree.root {
            default_tree.add_child(root_id, seq_id);
        }
        default_tree.add_child(seq_id, cond_id);
        default_tree.add_child(seq_id, act_id);
        default_tree.add_child(seq_id, idle_id);

        let mut editor = Self {
            trees: vec![default_tree],
            active_tree: 0,
            selected_nodes: HashSet::new(),
            selected_node: None,
            hovered_node: None,
            canvas_offset: Vec2::new(0.0, 0.0),
            canvas_zoom: DEFAULT_CANVAS_ZOOM,
            drag_state: DragState::None,
            drag_offset: Vec2::ZERO,
            connecting_from: None,
            show_blackboard: true,
            show_node_palette: true,
            show_properties: true,
            show_simulation: true,
            search_filter: String::new(),
            palette_search: String::new(),
            simulation: SimulationState::new(),
            breakpoints: HashSet::new(),
            execution_log: Vec::new(),
            blackboard: Blackboard::new(),
            clipboard_node: None,
            clipboard_subtree: None,
            layout_mode: LayoutMode::Manual,
            node_id_counter: counter,
            context_menu: None,
            selection_box: None,
            new_node_dialog: None,
            bb_editor: BlackboardEditorState::default(),
            tree_rename: TreeRenameState::default(),
            import_export: ImportExportState::default(),
            validation: ValidationState::default(),
            show_grid: true,
            snap_to_grid: false,
            show_minimap: true,
            palette_collapsed_categories: HashSet::new(),
            properties_scroll: 0.0,
            log_scroll_to_bottom: true,
            frame_counter: 0,
            last_tick_time: 0.0,
        };

        // Default blackboard values for simulation
        editor.blackboard.set("health", BlackboardValue::Float(1.0));
        editor.blackboard.set("alerted", BlackboardValue::Bool(false));
        editor.blackboard.set("target", BlackboardValue::EntityRef("player".to_string()));
        editor.blackboard.set("patrol_index", BlackboardValue::Int(0));

        editor
    }

    fn active_tree(&self) -> Option<&BehaviorTree> {
        self.trees.get(self.active_tree)
    }

    fn active_tree_mut(&mut self) -> Option<&mut BehaviorTree> {
        self.trees.get_mut(self.active_tree)
    }

    fn next_id(&mut self) -> BtNodeId {
        let id = self.node_id_counter;
        self.node_id_counter += 1;
        id
    }

    fn canvas_to_screen(&self, canvas_pos: Pos2) -> Pos2 {
        Pos2::new(
            canvas_pos.x * self.canvas_zoom + self.canvas_offset.x,
            canvas_pos.y * self.canvas_zoom + self.canvas_offset.y,
        )
    }

    fn screen_to_canvas(&self, screen_pos: Pos2) -> Pos2 {
        Pos2::new(
            (screen_pos.x - self.canvas_offset.x) / self.canvas_zoom,
            (screen_pos.y - self.canvas_offset.y) / self.canvas_zoom,
        )
    }

    fn canvas_rect_to_screen(&self, canvas_rect: Rect) -> Rect {
        Rect::from_min_max(
            self.canvas_to_screen(canvas_rect.min),
            self.canvas_to_screen(canvas_rect.max),
        )
    }

    fn node_screen_rect(&self, node: &BtNodeData) -> Rect {
        let pos = Pos2::new(node.position.0, node.position.1);
        let h = node.compute_height();
        let canvas_rect = Rect::from_min_size(pos, Vec2::new(NODE_WIDTH, h));
        self.canvas_rect_to_screen(canvas_rect)
    }

    fn node_at_screen_pos(&self, screen_pos: Pos2) -> Option<BtNodeId> {
        let tree = self.trees.get(self.active_tree)?;
        // Iterate in reverse Z order (later nodes on top)
        let mut candidates: Vec<(BtNodeId, Rect)> = tree.nodes.iter()
            .map(|(&id, node)| (id, self.node_screen_rect(node)))
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        for (id, rect) in candidates {
            if rect.contains(screen_pos) {
                return Some(id);
            }
        }
        None
    }

    fn add_node_to_tree(&mut self, node_type: BtNode, canvas_pos: Pos2, parent_id: Option<BtNodeId>) -> BtNodeId {
        let id = self.next_id();
        let mut canvas_x = canvas_pos.x;
        let mut canvas_y = canvas_pos.y;
        if self.snap_to_grid {
            canvas_x = (canvas_x / SNAP_GRID).round() * SNAP_GRID;
            canvas_y = (canvas_y / SNAP_GRID).round() * SNAP_GRID;
        }
        let node = BtNodeData::new(id, node_type, (canvas_x, canvas_y));
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            tree.nodes.insert(id, node);
            if let Some(pid) = parent_id {
                tree.add_child(pid, id);
            }
            if tree.root.is_none() {
                tree.root = Some(id);
            }
        }
        id
    }

    fn duplicate_node(&mut self, id: BtNodeId) -> Option<BtNodeId> {
        let tree = self.trees.get(self.active_tree)?;
        let original = tree.nodes.get(&id)?.clone();
        let new_id = self.node_id_counter;
        self.node_id_counter += 1;
        let mut new_node = original.clone();
        new_node.id = new_id;
        new_node.position = (original.position.0 + 30.0, original.position.1 + 30.0);
        new_node.parent = None;
        new_node.children = Vec::new();
        new_node.status = BtNodeStatus::Idle;
        new_node.execution_count = 0;
        let parent = original.parent;
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            tree.nodes.insert(new_id, new_node);
            if let Some(pid) = parent {
                tree.add_child(pid, new_id);
            }
        }
        Some(new_id)
    }

    fn duplicate_subtree(&mut self, root_id: BtNodeId) -> Option<BtNodeId> {
        let tree = self.trees.get(self.active_tree)?;
        let subtree: Vec<BtNodeData> = tree.collect_subtree(root_id).iter()
            .filter_map(|&id| tree.nodes.get(&id).cloned())
            .collect();
        if subtree.is_empty() { return None; }

        // Remap IDs
        let mut id_map: HashMap<BtNodeId, BtNodeId> = HashMap::new();
        for node in &subtree {
            let new_id = self.node_id_counter;
            self.node_id_counter += 1;
            id_map.insert(node.id, new_id);
        }

        let new_root_id = *id_map.get(&root_id)?;

        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            for node in &subtree {
                let new_id = *id_map.get(&node.id).unwrap();
                let mut new_node = node.clone();
                new_node.id = new_id;
                new_node.position = (node.position.0 + 40.0, node.position.1 + 40.0);
                new_node.children = node.children.iter()
                    .filter_map(|&c| id_map.get(&c).copied())
                    .collect();
                new_node.parent = node.parent.and_then(|p| id_map.get(&p).copied());
                new_node.status = BtNodeStatus::Idle;
                new_node.execution_count = 0;
                tree.nodes.insert(new_id, new_node);
            }
            // Fix the new root's parent
            if let Some(new_root) = tree.nodes.get_mut(&new_root_id) {
                new_root.parent = None;
            }
        }

        Some(new_root_id)
    }

    fn delete_selected(&mut self) {
        let to_delete: Vec<BtNodeId> = self.selected_nodes.iter().copied().collect();
        for id in to_delete {
            if let Some(tree) = self.trees.get_mut(self.active_tree) {
                tree.remove_node(id);
            }
            self.breakpoints.remove(&id);
        }
        self.selected_nodes.clear();
        if let Some(sel) = self.selected_node {
            if !self.trees.get(self.active_tree)
                .map(|t| t.nodes.contains_key(&sel))
                .unwrap_or(false)
            {
                self.selected_node = None;
            }
        }
    }

    fn select_all(&mut self) {
        if let Some(tree) = self.trees.get(self.active_tree) {
            self.selected_nodes = tree.nodes.keys().copied().collect();
        }
    }

    fn apply_layout(&mut self) {
        match &self.layout_mode {
            LayoutMode::AutoHorizontal => {
                if let Some(tree) = self.trees.get_mut(self.active_tree) {
                    auto_layout_horizontal(tree);
                }
            },
            LayoutMode::AutoVertical => {
                if let Some(tree) = self.trees.get_mut(self.active_tree) {
                    auto_layout_vertical(tree);
                }
            },
            LayoutMode::Manual => {},
        }
    }

    fn do_fit_to_view(&mut self, canvas_rect: Rect) {
        if let Some(tree) = self.trees.get(self.active_tree) {
            fit_to_view(tree, canvas_rect, &mut self.canvas_zoom, &mut self.canvas_offset);
        }
    }

    fn simulation_step(&mut self) {
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            let hit_bp = simulate_tick(
                tree,
                &mut self.blackboard,
                &self.breakpoints,
                &mut self.execution_log,
                self.simulation.tick,
            );
            self.simulation.tick += 1;
            if hit_bp {
                self.simulation.running = false;
            }
            self.log_scroll_to_bottom = true;
        }
    }

    fn simulation_reset(&mut self) {
        self.simulation.tick = 0;
        self.simulation.running = false;
        self.execution_log.clear();
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            for node in tree.nodes.values_mut() {
                node.status = BtNodeStatus::Idle;
                node.execution_count = 0;
                node.last_execution_tick = 0;
            }
        }
    }

    fn add_new_tree(&mut self) {
        let name = format!("Tree {}", self.trees.len() + 1);
        let tree = BehaviorTree::new_with_root(name, &mut self.node_id_counter);
        self.trees.push(tree);
        self.active_tree = self.trees.len() - 1;
    }

    fn delete_tree(&mut self, idx: usize) {
        if self.trees.len() <= 1 { return; }
        self.trees.remove(idx);
        self.active_tree = self.active_tree.min(self.trees.len() - 1);
    }

    fn copy_subtree(&mut self, id: BtNodeId) {
        if let Some(tree) = self.trees.get(self.active_tree) {
            let nodes: Vec<BtNodeData> = tree.collect_subtree(id).iter()
                .filter_map(|&nid| tree.nodes.get(&nid).cloned())
                .collect();
            self.clipboard_subtree = Some(nodes);
            self.clipboard_node = tree.nodes.get(&id).cloned();
        }
    }

    fn paste_subtree(&mut self, canvas_pos: Pos2) {
        let Some(clipboard) = self.clipboard_subtree.clone() else { return; };
        let Some(root) = clipboard.first() else { return; };
        let root_id = root.id;
        let offset_x = canvas_pos.x - root.position.0;
        let offset_y = canvas_pos.y - root.position.1;

        let mut id_map: HashMap<BtNodeId, BtNodeId> = HashMap::new();
        for node in &clipboard {
            let new_id = self.node_id_counter;
            self.node_id_counter += 1;
            id_map.insert(node.id, new_id);
        }

        let new_root_id = *id_map.get(&root_id).unwrap_or(&0);

        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            for node in &clipboard {
                let new_id = *id_map.get(&node.id).unwrap();
                let mut new_node = node.clone();
                new_node.id = new_id;
                new_node.position = (node.position.0 + offset_x, node.position.1 + offset_y);
                new_node.children = node.children.iter()
                    .filter_map(|&c| id_map.get(&c).copied())
                    .collect();
                new_node.parent = None; // pasted subtree is detached
                new_node.status = BtNodeStatus::Idle;
                new_node.execution_count = 0;
                tree.nodes.insert(new_id, new_node);
            }
        }

        self.selected_node = Some(new_root_id);
        self.selected_nodes.clear();
        self.selected_nodes.insert(new_root_id);
    }

    fn collapse_all_children(&mut self, id: BtNodeId) {
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            let subtree = tree.collect_subtree(id);
            for node_id in subtree {
                if node_id != id {
                    if let Some(node) = tree.nodes.get_mut(&node_id) {
                        node.collapsed = true;
                    }
                }
            }
        }
    }

    fn expand_all_children(&mut self, id: BtNodeId) {
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            let subtree = tree.collect_subtree(id);
            for node_id in subtree {
                if let Some(node) = tree.nodes.get_mut(&node_id) {
                    node.collapsed = false;
                }
            }
        }
    }

    fn reorder_child_up(&mut self, child_id: BtNodeId) {
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            if let Some(parent_id) = tree.nodes.get(&child_id).and_then(|n| n.parent) {
                tree.reorder_child(parent_id, child_id, -1);
            }
        }
    }

    fn reorder_child_down(&mut self, child_id: BtNodeId) {
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            if let Some(parent_id) = tree.nodes.get(&child_id).and_then(|n| n.parent) {
                tree.reorder_child(parent_id, child_id, 1);
            }
        }
    }

    fn set_root(&mut self, id: BtNodeId) {
        if let Some(tree) = self.trees.get_mut(self.active_tree) {
            tree.root = Some(id);
        }
    }

    fn validate_active_tree(&mut self) {
        if let Some(tree) = self.trees.get(self.active_tree) {
            self.validation.errors = tree.validate();
        }
    }
}

// ============================================================
// DRAWING HELPERS
// ============================================================

fn draw_bezier_connection(
    painter: &Painter,
    from: Pos2,
    to: Pos2,
    color: Color32,
    thickness: f32,
    zoom: f32,
) {
    let ctrl_offset = BEZIER_CONTROL_OFFSET * zoom;
    let from_ctrl = Pos2::new(from.x, from.y + ctrl_offset);
    let to_ctrl = Pos2::new(to.x, to.y - ctrl_offset);

    let points: Vec<Pos2> = (0..=20).map(|i| {
        let t = i as f32 / 20.0;
        let u = 1.0 - t;
        let x = u*u*u*from.x + 3.0*u*u*t*from_ctrl.x + 3.0*u*t*t*to_ctrl.x + t*t*t*to.x;
        let y = u*u*u*from.y + 3.0*u*u*t*from_ctrl.y + 3.0*u*t*t*to_ctrl.y + t*t*t*to.y;
        Pos2::new(x, y)
    }).collect();

    for i in 0..points.len().saturating_sub(1) {
        painter.line_segment([points[i], points[i+1]], Stroke::new(thickness, color));
    }

    // Arrow at end
    if let (Some(last), Some(second_last)) = (points.last(), points.get(points.len().saturating_sub(2))) {
        let dir = (*last - *second_last).normalized();
        let perp = Vec2::new(-dir.y, dir.x);
        let tip = *last;
        let base1 = tip - dir * ARROW_SIZE * zoom + perp * ARROW_SIZE * 0.5 * zoom;
        let base2 = tip - dir * ARROW_SIZE * zoom - perp * ARROW_SIZE * 0.5 * zoom;
        painter.add(Shape::convex_polygon(
            vec![tip, base1, base2],
            color,
            Stroke::NONE,
        ));
    }
}

fn draw_node(
    painter: &Painter,
    node: &BtNodeData,
    screen_rect: Rect,
    is_selected: bool,
    is_hovered: bool,
    has_breakpoint: bool,
    zoom: f32,
    frame: u64,
) {
    let rounding = egui::Rounding::same((NODE_CORNER_RADIUS * zoom) as u8);
    let header_h = NODE_HEADER_HEIGHT * zoom;

    // Shadow
    let shadow_offset = Vec2::new(3.0 * zoom, 4.0 * zoom);
    let shadow_rect = screen_rect.translate(shadow_offset);
    painter.rect_filled(shadow_rect, rounding, Color32::from_rgba_premultiplied(0, 0, 0, 80));

    // Node background
    painter.rect_filled(screen_rect, rounding, COLOR_NODE_BG);

    // Header background
    let header_rect = Rect::from_min_size(
        screen_rect.min,
        Vec2::new(screen_rect.width(), header_h),
    );
    let header_color = node.header_color();
    let header_dark = node.header_dark_color();

    // Gradient effect for header
    painter.rect_filled(header_rect, egui::Rounding {
        nw: (NODE_CORNER_RADIUS * zoom) as u8,
        ne: (NODE_CORNER_RADIUS * zoom) as u8,
        sw: 0,
        se: 0,
    }, header_color);

    // Disabled overlay
    if !node.enabled {
        painter.rect_filled(screen_rect, rounding, Color32::from_rgba_premultiplied(0, 0, 0, 100));
    }

    // Status border
    let status_color = node.status.color();
    let border_width = if is_selected { 3.0 } else if is_hovered { 2.0 } else { 1.5 };
    let border_color = if is_selected {
        COLOR_SELECTED_BORDER
    } else if is_hovered {
        COLOR_HOVERED_BORDER
    } else {
        Color32::from_rgba_premultiplied(
            status_color.r(),
            status_color.g(),
            status_color.b(),
            180,
        )
    };
    painter.rect_stroke(screen_rect, rounding, Stroke::new(border_width * zoom, border_color), egui::StrokeKind::Outside);

    // Status glow when running
    if node.status == BtNodeStatus::Running {
        let pulse = ((frame as f32 * 0.1).sin() * 0.5 + 0.5) * 80.0;
        let glow_rect = screen_rect.expand(2.0 * zoom);
        painter.rect_stroke(
            glow_rect,
            egui::Rounding::same(((NODE_CORNER_RADIUS + 2.0) * zoom) as u8),
            Stroke::new(2.0 * zoom, Color32::from_rgba_premultiplied(
                COLOR_STATUS_RUNNING.r(),
                COLOR_STATUS_RUNNING.g(),
                COLOR_STATUS_RUNNING.b(),
                pulse as u8,
            )),
            egui::StrokeKind::Outside,
        );
    }

    // Icon + Label in header
    let icon_size = 14.0 * zoom;
    let icon_pos = Pos2::new(
        screen_rect.min.x + 8.0 * zoom,
        screen_rect.min.y + header_h / 2.0,
    );
    painter.text(
        icon_pos,
        Align2::LEFT_CENTER,
        node.node_type.icon(),
        FontId::proportional(icon_size),
        Color32::WHITE,
    );

    let label_x = icon_pos.x + 20.0 * zoom;
    let label_max_w = screen_rect.width() - 36.0 * zoom;
    let label = if node.label.len() > 20 {
        format!("{}…", &node.label[..17])
    } else {
        node.label.clone()
    };
    painter.text(
        Pos2::new(label_x, screen_rect.min.y + header_h / 2.0),
        Align2::LEFT_CENTER,
        &label,
        FontId::proportional(12.0 * zoom),
        Color32::WHITE,
    );

    // Breakpoint indicator
    if has_breakpoint {
        let bp_pos = Pos2::new(
            screen_rect.max.x - 8.0 * zoom,
            screen_rect.min.y + 8.0 * zoom,
        );
        painter.circle_filled(bp_pos, 6.0 * zoom, COLOR_BREAKPOINT);
        painter.circle_stroke(bp_pos, 6.0 * zoom, Stroke::new(1.0 * zoom, Color32::WHITE));
    }

    // Disabled badge
    if !node.enabled {
        let dis_pos = Pos2::new(
            screen_rect.min.x + 8.0 * zoom,
            screen_rect.min.y + 8.0 * zoom,
        );
        painter.text(
            dis_pos,
            Align2::LEFT_TOP,
            "disabled",
            FontId::proportional(9.0 * zoom),
            Color32::from_rgb(180, 100, 100),
        );
    }

    // Body parameters (if not collapsed)
    if !node.collapsed {
        let params = node.param_lines();
        let body_start_y = screen_rect.min.y + header_h + 4.0 * zoom;
        for (i, (key, val)) in params.iter().enumerate() {
            let line_y = body_start_y + i as f32 * (NODE_BODY_LINE_HEIGHT * zoom);
            let key_x = screen_rect.min.x + 8.0 * zoom;
            let val_x = screen_rect.min.x + screen_rect.width() * 0.5;

            painter.text(
                Pos2::new(key_x, line_y + NODE_BODY_LINE_HEIGHT * zoom * 0.5),
                Align2::LEFT_CENTER,
                key,
                FontId::proportional(10.0 * zoom),
                COLOR_NODE_SUBTEXT,
            );

            let val_str = if val.len() > 14 { format!("{}…", &val[..11]) } else { val.clone() };
            painter.text(
                Pos2::new(val_x, line_y + NODE_BODY_LINE_HEIGHT * zoom * 0.5),
                Align2::LEFT_CENTER,
                &val_str,
                FontId::proportional(10.0 * zoom),
                COLOR_NODE_TEXT,
            );
        }
    }

    // Collapse/expand toggle
    let toggle_pos = Pos2::new(
        screen_rect.max.x - 14.0 * zoom,
        screen_rect.min.y + header_h / 2.0,
    );
    let toggle_text = if node.collapsed { "+" } else { "-" };
    painter.text(
        toggle_pos,
        Align2::CENTER_CENTER,
        toggle_text,
        FontId::proportional(12.0 * zoom),
        Color32::from_rgb(180, 180, 200),
    );

    // Status dot in bottom-right of header
    let status_dot_pos = Pos2::new(
        screen_rect.max.x - (if has_breakpoint { 20.0 } else { 8.0 }) * zoom,
        screen_rect.min.y + header_h - 8.0 * zoom,
    );
    painter.circle_filled(status_dot_pos, 4.0 * zoom, status_color);

    // Comment indicator
    if !node.comment.is_empty() {
        let comment_pos = Pos2::new(
            screen_rect.min.x + 2.0 * zoom,
            screen_rect.max.y - 4.0 * zoom,
        );
        painter.text(
            comment_pos,
            Align2::LEFT_BOTTOM,
            "💬",
            FontId::proportional(9.0 * zoom),
            Color32::from_rgb(180, 200, 120),
        );
    }

    // Tags
    if !node.tags.is_empty() {
        let tag_x = screen_rect.min.x + 4.0 * zoom;
        let tag_y = screen_rect.max.y - 16.0 * zoom;
        for (i, tag) in node.tags.iter().take(3).enumerate() {
            let tag_rect = Rect::from_min_size(
                Pos2::new(tag_x + i as f32 * 40.0 * zoom, tag_y),
                Vec2::new(36.0 * zoom, 12.0 * zoom),
            );
            painter.rect_filled(tag_rect, egui::Rounding::same((3.0 * zoom) as u8),
                Color32::from_rgba_premultiplied(80, 80, 120, 200));
            let short = if tag.len() > 5 { &tag[..5] } else { tag.as_str() };
            painter.text(
                tag_rect.center(),
                Align2::CENTER_CENTER,
                short,
                FontId::proportional(8.0 * zoom),
                Color32::from_rgb(180, 180, 220),
            );
        }
    }

    // Connection ports (top center for input, bottom center for output)
    let top_port = Pos2::new(
        screen_rect.center().x,
        screen_rect.min.y,
    );
    let bottom_port = Pos2::new(
        screen_rect.center().x,
        screen_rect.max.y,
    );

    // Draw port indicators
    if node.parent.is_some() {
        painter.circle_filled(top_port, 4.0 * zoom, Color32::from_rgb(150, 150, 170));
        painter.circle_stroke(top_port, 4.0 * zoom, Stroke::new(1.0 * zoom, Color32::from_rgb(200, 200, 220)));
    }
    if node.node_type.can_have_children() {
        painter.circle_filled(bottom_port, 4.0 * zoom, Color32::from_rgb(150, 150, 170));
        painter.circle_stroke(bottom_port, 4.0 * zoom, Stroke::new(1.0 * zoom, Color32::from_rgb(200, 200, 220)));
    }
}

fn draw_grid(painter: &Painter, rect: Rect, offset: Vec2, zoom: f32) {
    let spacing = GRID_SPACING * zoom;
    let start_x = rect.min.x - ((rect.min.x - offset.x) % spacing);
    let start_y = rect.min.y - ((rect.min.y - offset.y) % spacing);

    let mut x = start_x;
    while x < rect.max.x {
        let mut y = start_y;
        while y < rect.max.y {
            painter.circle_filled(
                Pos2::new(x, y),
                GRID_DOT_RADIUS,
                COLOR_GRID_DOT,
            );
            y += spacing;
        }
        x += spacing;
    }
}

fn draw_minimap(
    painter: &Painter,
    minimap_rect: Rect,
    tree: &BehaviorTree,
    canvas_offset: Vec2,
    canvas_zoom: f32,
    canvas_rect: Rect,
    selected_nodes: &HashSet<BtNodeId>,
) {
    painter.rect_filled(minimap_rect, egui::Rounding::same(4), Color32::from_rgba_premultiplied(25, 25, 30, 220));
    painter.rect_stroke(minimap_rect, egui::Rounding::same(4), Stroke::new(1.0, Color32::from_rgb(60, 60, 80)), egui::StrokeKind::Outside);

    let bounds = match tree.bounds() {
        Some(b) => b,
        None => return,
    };
    let bounds_w = bounds.width().max(100.0);
    let bounds_h = bounds.height().max(100.0);
    let mm_scale = (minimap_rect.width() / bounds_w).min(minimap_rect.height() / bounds_h) * 0.9;
    let mm_offset_x = minimap_rect.min.x + (minimap_rect.width() - bounds_w * mm_scale) / 2.0;
    let mm_offset_y = minimap_rect.min.y + (minimap_rect.height() - bounds_h * mm_scale) / 2.0;

    let canvas_to_mm = |cx: f32, cy: f32| -> Pos2 {
        Pos2::new(
            mm_offset_x + (cx - bounds.min.x) * mm_scale,
            mm_offset_y + (cy - bounds.min.y) * mm_scale,
        )
    };

    // Draw connections
    for node in tree.nodes.values() {
        if node.children.is_empty() { continue; }
        let from = canvas_to_mm(
            node.position.0 + NODE_WIDTH / 2.0,
            node.position.1 + node.compute_height(),
        );
        for &child_id in &node.children {
            if let Some(child) = tree.nodes.get(&child_id) {
                let to = canvas_to_mm(child.position.0 + NODE_WIDTH / 2.0, child.position.1);
                painter.line_segment([from, to], Stroke::new(0.5, Color32::from_rgb(80, 80, 100)));
            }
        }
    }

    // Draw nodes
    for (&id, node) in &tree.nodes {
        let h = node.compute_height();
        let p1 = canvas_to_mm(node.position.0, node.position.1);
        let p2 = canvas_to_mm(node.position.0 + NODE_WIDTH, node.position.1 + h);
        let r = Rect::from_min_max(p1, p2);
        let color = if selected_nodes.contains(&id) {
            COLOR_SELECTED_BORDER
        } else {
            node.header_color()
        };
        painter.rect_filled(r, egui::Rounding::same(1), color);
    }

    // Viewport indicator
    let vp_canvas_min = Pos2::new(
        -canvas_offset.x / canvas_zoom,
        -canvas_offset.y / canvas_zoom,
    );
    let vp_canvas_max = Pos2::new(
        (canvas_rect.width() - canvas_offset.x) / canvas_zoom,
        (canvas_rect.height() - canvas_offset.y) / canvas_zoom,
    );
    let vp_mm_min = canvas_to_mm(vp_canvas_min.x, vp_canvas_min.y);
    let vp_mm_max = canvas_to_mm(vp_canvas_max.x, vp_canvas_max.y);
    let vp_rect = Rect::from_min_max(vp_mm_min, vp_mm_max);
    painter.rect_stroke(
        vp_rect,
        egui::Rounding::same(1),
        Stroke::new(1.0, Color32::from_rgb(200, 200, 100)),
        egui::StrokeKind::Outside,
    );
}

// ============================================================
// PANEL DRAWING
// ============================================================

fn show_node_palette_panel(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor, canvas_rect: Rect) {
    ui.set_min_width(NODE_PALETTE_WIDTH);
    ui.set_max_width(NODE_PALETTE_WIDTH);

    ui.vertical(|ui| {
        // Header
        let header_rect = ui.allocate_space(Vec2::new(NODE_PALETTE_WIDTH, 28.0)).1;
        ui.painter().rect_filled(header_rect, egui::Rounding::ZERO, COLOR_PANEL_HEADER);
        ui.painter().text(
            header_rect.center(),
            Align2::CENTER_CENTER,
            "Node Palette",
            FontId::proportional(13.0),
            Color32::WHITE,
        );

        ui.add_space(4.0);

        // Search box
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.text_edit_singleline(&mut editor.palette_search);
        });

        ui.add_space(4.0);
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("palette_scroll")
            .show(ui, |ui| {
                let filter = editor.palette_search.to_lowercase();

                // Composite nodes
                show_palette_category(
                    ui, editor, "Composites", NodeCategory::Composite, &filter,
                    &[
                        ("→  Sequence", BtNode::Sequence, "All children must succeed"),
                        ("?  Selector", BtNode::Selector, "First child to succeed wins"),
                        ("⇉  Parallel", BtNode::Parallel(ParallelPolicy::RequireAll), "Run all children at once"),
                        ("🎲 Random Selector", BtNode::RandomSelector, "Pick a random child"),
                        ("⚖  Weighted Selector", BtNode::WeightedSelector(vec![1.0, 1.0]), "Weighted random choice"),
                    ]
                );

                show_palette_category(
                    ui, editor, "Decorators", NodeCategory::Decorator, &filter,
                    &[
                        ("!  Inverter", BtNode::Inverter, "Flips child's result"),
                        ("✓  Succeeder", BtNode::Succeeder, "Always succeeds"),
                        ("✗  Failer", BtNode::Failer, "Always fails"),
                        ("↺  Repeat", BtNode::Repeat(3), "Repeat N times"),
                        ("↻  Repeat Until Fail", BtNode::RepeatUntilFail, "Loop until child fails"),
                        ("⏱  Cooldown", BtNode::Cooldown { duration_secs: 5.0, shared_key: None }, "Rate-limit child"),
                        ("⌛  Timeout", BtNode::Timeout(3.0), "Fail child after time limit"),
                        ("🔒 Limiter", BtNode::Limiter { max_per_interval: 3, interval_secs: 10.0 }, "Max executions per interval"),
                    ]
                );

                show_palette_category_actions(ui, editor, &filter);
                show_palette_category_conditions(ui, editor, &filter);

                show_palette_category(
                    ui, editor, "Leaves", NodeCategory::Leaf, &filter,
                    &[
                        ("⏳ Wait", BtNode::Wait(1.0), "Wait N seconds"),
                        ("📝 Log", BtNode::Log("message".into()), "Print debug message"),
                        ("✎  Set Blackboard", BtNode::SetBlackboard { key: "key".into(), value: BlackboardValue::Bool(true) }, "Write to blackboard"),
                        ("≡  Check Blackboard", BtNode::CheckBlackboard { key: "key".into(), op: CompareOp::Equal, value: BlackboardValue::Bool(true) }, "Compare blackboard value"),
                    ]
                );
            });
    });
}

fn show_palette_category(
    ui: &mut egui::Ui,
    editor: &mut BehaviorTreeEditor,
    category_name: &str,
    category: NodeCategory,
    filter: &str,
    items: &[(&str, BtNode, &str)],
) {
    let visible_items: Vec<_> = items.iter()
        .filter(|(name, _, _)| filter.is_empty() || name.to_lowercase().contains(filter))
        .collect();

    if visible_items.is_empty() { return; }

    let is_collapsed = editor.palette_collapsed_categories.contains(category_name);
    let cat_color = category.color();

    let header = ui.horizontal(|ui| {
        let arrow = if is_collapsed { "▶" } else { "▼" };
        let resp = ui.colored_label(cat_color, format!("{} {}", arrow, category_name));
        resp
    });

    if header.inner.clicked() {
        if is_collapsed {
            editor.palette_collapsed_categories.remove(category_name);
        } else {
            editor.palette_collapsed_categories.insert(category_name.to_string());
        }
    }

    if !is_collapsed {
        for (name, node_type, tooltip) in &visible_items {
            let label_resp = ui.selectable_label(false, *name);
            if label_resp.hovered() {
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(*name), |ui| {
                    ui.label(*tooltip);
                    ui.label(node_type.description());
                });
            }

            // Drag-to-add: detect drag start from palette item
            if label_resp.drag_started() {
                // We'll use a simple click-to-add approach since full dnd requires more infrastructure
            }

            if label_resp.clicked() {
                // Add to center of canvas
                let canvas_center = Pos2::new(400.0, 300.0);
                let selected_parent = editor.selected_node;
                let nt = node_type.clone();
                editor.add_node_to_tree(nt, canvas_center, selected_parent);
            }
        }
    }

    ui.add_space(2.0);
}

fn show_palette_category_actions(
    ui: &mut egui::Ui,
    editor: &mut BehaviorTreeEditor,
    filter: &str,
) {
    let is_collapsed = editor.palette_collapsed_categories.contains("Actions");
    let cat_color = NodeCategory::Action.color();

    let variants = ActionKind::all_variants();
    let visible: Vec<&ActionKind> = variants.iter()
        .filter(|k| filter.is_empty() || k.display_name().to_lowercase().contains(filter))
        .collect();

    if visible.is_empty() { return; }

    let header = ui.horizontal(|ui| {
        let arrow = if is_collapsed { "▶" } else { "▼" };
        ui.colored_label(cat_color, format!("{} Actions", arrow))
    });

    if header.inner.clicked() {
        if is_collapsed {
            editor.palette_collapsed_categories.remove("Actions");
        } else {
            editor.palette_collapsed_categories.insert("Actions".to_string());
        }
    }

    if !is_collapsed {
        for kind in &visible {
            let name = format!("▶  {}", kind.display_name());
            let resp = ui.selectable_label(false, &name);
            if resp.clicked() {
                let canvas_center = Pos2::new(400.0, 300.0);
                let parent = editor.selected_node;
                let node_type = BtNode::Action((*kind).clone());
                editor.add_node_to_tree(node_type, canvas_center, parent);
            }
        }
    }
    ui.add_space(2.0);
}

fn show_palette_category_conditions(
    ui: &mut egui::Ui,
    editor: &mut BehaviorTreeEditor,
    filter: &str,
) {
    let is_collapsed = editor.palette_collapsed_categories.contains("Conditions");
    let cat_color = NodeCategory::Condition.color();

    let variants = ConditionKind::all_variants();
    let visible: Vec<&ConditionKind> = variants.iter()
        .filter(|k| filter.is_empty() || k.display_name().to_lowercase().contains(filter))
        .collect();

    if visible.is_empty() { return; }

    let header = ui.horizontal(|ui| {
        let arrow = if is_collapsed { "▶" } else { "▼" };
        ui.colored_label(cat_color, format!("{} Conditions", arrow))
    });

    if header.inner.clicked() {
        if is_collapsed {
            editor.palette_collapsed_categories.remove("Conditions");
        } else {
            editor.palette_collapsed_categories.insert("Conditions".to_string());
        }
    }

    if !is_collapsed {
        for kind in &visible {
            let name = format!("◇  {}", kind.display_name());
            let resp = ui.selectable_label(false, &name);
            if resp.clicked() {
                let canvas_center = Pos2::new(400.0, 300.0);
                let parent = editor.selected_node;
                let node_type = BtNode::Condition((*kind).clone());
                editor.add_node_to_tree(node_type, canvas_center, parent);
            }
        }
    }
    ui.add_space(2.0);
}

fn show_properties_panel(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor) {
    let selected_id = match editor.selected_node {
        Some(id) => id,
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.colored_label(Color32::from_rgb(120, 120, 140), "No node selected");
                ui.add_space(4.0);
                ui.label("Click a node to edit its properties");
            });
            return;
        }
    };

    let node = match editor.trees.get(editor.active_tree)
        .and_then(|t| t.nodes.get(&selected_id))
        .cloned()
    {
        Some(n) => n,
        None => {
            editor.selected_node = None;
            return;
        }
    };

    egui::ScrollArea::vertical()
        .id_salt("props_scroll")
        .show(ui, |ui| {
            // Header
            let cat_color = node.node_type.category().color();
            ui.horizontal(|ui| {
                ui.colored_label(cat_color, node.node_type.icon());
                ui.colored_label(cat_color, node.node_type.type_name());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("ID: {}", node.id));
                });
            });
            ui.separator();

            // Label
            ui.label("Label:");
            let mut label = node.label.clone();
            if ui.text_edit_singleline(&mut label).changed() {
                if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                    if let Some(n) = tree.nodes.get_mut(&selected_id) {
                        n.label = label;
                    }
                }
            }

            // Enable toggle
            ui.horizontal(|ui| {
                let mut enabled = node.enabled;
                if ui.checkbox(&mut enabled, "Enabled").changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&selected_id) {
                            n.enabled = enabled;
                        }
                    }
                }
                ui.label(" ");
                let mut collapsed = node.collapsed;
                if ui.checkbox(&mut collapsed, "Collapsed").changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&selected_id) {
                            n.collapsed = collapsed;
                        }
                    }
                }
            });

            ui.separator();
            ui.label("Node Type Parameters:");

            // Type-specific properties
            show_node_type_properties(ui, editor, selected_id, &node.node_type.clone());

            ui.separator();

            // Comment
            ui.label("Comment:");
            let mut comment = node.comment.clone();
            if ui.text_edit_multiline(&mut comment).changed() {
                if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                    if let Some(n) = tree.nodes.get_mut(&selected_id) {
                        n.comment = comment;
                    }
                }
            }

            ui.separator();

            // Tags
            ui.label("Tags:");
            let tags_str: String = node.tags.join(", ");
            let mut tags_edit = tags_str.clone();
            if ui.text_edit_singleline(&mut tags_edit).changed() {
                if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                    if let Some(n) = tree.nodes.get_mut(&selected_id) {
                        n.tags = tags_edit.split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                }
            }

            ui.separator();

            // Custom color
            ui.label("Custom Color:");
            ui.horizontal(|ui| {
                let has_custom = node.custom_color.is_some();
                let mut use_custom = has_custom;
                if ui.checkbox(&mut use_custom, "Override").changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&selected_id) {
                            n.custom_color = if use_custom { Some([100, 100, 200]) } else { None };
                        }
                    }
                }
                if has_custom {
                    if let Some([r, g, b]) = node.custom_color {
                        let mut color = egui::Color32::from_rgb(r, g, b);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                                if let Some(n) = tree.nodes.get_mut(&selected_id) {
                                    n.custom_color = Some([color.r(), color.g(), color.b()]);
                                }
                            }
                        }
                    }
                }
            });

            ui.separator();

            // Children list
            if !node.children.is_empty() {
                ui.label(format!("Children ({}):", node.children.len()));
                let children = node.children.clone();
                for child_id in &children {
                    let child_label = editor.trees.get(editor.active_tree)
                        .and_then(|t| t.nodes.get(child_id))
                        .map(|n| n.label.clone())
                        .unwrap_or_else(|| format!("#{}", child_id));
                    ui.horizontal(|ui| {
                        if ui.small_button("↑").clicked() {
                            editor.reorder_child_up(*child_id);
                        }
                        if ui.small_button("↓").clicked() {
                            editor.reorder_child_down(*child_id);
                        }
                        if ui.selectable_label(false, &child_label).clicked() {
                            editor.selected_node = Some(*child_id);
                        }
                        if ui.small_button("✗").clicked() {
                            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                                tree.remove_child(selected_id, *child_id);
                            }
                        }
                    });
                }
            }

            ui.separator();

            // Execution stats
            ui.collapsing("Execution Stats", |ui| {
                ui.label(format!("Status: {}", node.status.label()));
                ui.label(format!("Exec count: {}", node.execution_count));
                ui.label(format!("Last tick: {}", node.last_execution_tick));
                ui.label(format!("Depth: {}",
                    editor.trees.get(editor.active_tree)
                        .map(|t| t.node_depth(selected_id))
                        .unwrap_or(0)
                ));
            });

            ui.separator();

            // Quick actions
            ui.horizontal(|ui| {
                if ui.button("Duplicate").clicked() {
                    if let Some(new_id) = editor.duplicate_node(selected_id) {
                        editor.selected_node = Some(new_id);
                    }
                }
                if ui.button("Delete").clicked() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        tree.remove_node(selected_id);
                    }
                    editor.selected_node = None;
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Set Root").clicked() {
                    editor.set_root(selected_id);
                }
                if ui.button("Copy Subtree").clicked() {
                    editor.copy_subtree(selected_id);
                }
                let has_bp = editor.breakpoints.contains(&selected_id);
                if ui.button(if has_bp { "Clear BP" } else { "Set BP" }).clicked() {
                    if has_bp {
                        editor.breakpoints.remove(&selected_id);
                    } else {
                        editor.breakpoints.insert(selected_id);
                    }
                }
            });
        });
}

fn show_node_type_properties(
    ui: &mut egui::Ui,
    editor: &mut BehaviorTreeEditor,
    id: BtNodeId,
    node_type: &BtNode,
) {
    match node_type.clone() {
        BtNode::Sequence | BtNode::Selector | BtNode::RandomSelector | BtNode::RepeatUntilFail
        | BtNode::Inverter | BtNode::Succeeder | BtNode::Failer => {
            ui.label("No configurable parameters.");
        },

        BtNode::Parallel(policy) => {
            ui.label("Policy:");
            let mut current = policy.clone();
            let variants = [
                ("Require All", ParallelPolicy::RequireAll),
                ("Require One", ParallelPolicy::RequireOne),
            ];
            for (name, var) in &variants {
                if ui.radio(current == *var, *name).clicked() {
                    current = var.clone();
                }
            }
            ui.horizontal(|ui| {
                ui.radio(matches!(current, ParallelPolicy::RequireN(_)), "Require N:");
                let mut n = if let ParallelPolicy::RequireN(n) = &current { *n } else { 2 };
                if ui.add(egui::DragValue::new(&mut n).range(1..=16)).changed() {
                    current = ParallelPolicy::RequireN(n);
                }
            });
            let new_type = BtNode::Parallel(current);
            if new_type != *node_type {
                if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                    if let Some(n) = tree.nodes.get_mut(&id) {
                        n.node_type = new_type;
                    }
                }
            }
        },

        BtNode::WeightedSelector(mut weights) => {
            ui.label("Weights (one per child):");
            let child_count = editor.trees.get(editor.active_tree)
                .and_then(|t| t.nodes.get(&id))
                .map(|n| n.children.len())
                .unwrap_or(0);
            while weights.len() < child_count { weights.push(1.0); }
            let mut changed = false;
            for (i, w) in weights.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("Child {}:", i));
                    if ui.add(egui::DragValue::new(w).speed(0.1).range(0.0..=100.0)).changed() {
                        changed = true;
                    }
                });
            }
            if changed {
                if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                    if let Some(n) = tree.nodes.get_mut(&id) {
                        n.node_type = BtNode::WeightedSelector(weights);
                    }
                }
            }
        },

        BtNode::Repeat(mut count) => {
            ui.horizontal(|ui| {
                ui.label("Repeat count:");
                if ui.add(egui::DragValue::new(&mut count).range(1..=10000)).changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::Repeat(count);
                        }
                    }
                }
            });
        },

        BtNode::Cooldown { mut duration_secs, mut shared_key } => {
            ui.horizontal(|ui| {
                ui.label("Duration (s):");
                if ui.add(egui::DragValue::new(&mut duration_secs).speed(0.1).range(0.01..=3600.0)).changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::Cooldown { duration_secs, shared_key: shared_key.clone() };
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Shared key:");
                let mut key_str = shared_key.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut key_str).changed() {
                    let new_key = if key_str.is_empty() { None } else { Some(key_str) };
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::Cooldown { duration_secs, shared_key: new_key };
                        }
                    }
                }
            });
        },

        BtNode::Timeout(mut secs) => {
            ui.horizontal(|ui| {
                ui.label("Timeout (s):");
                if ui.add(egui::DragValue::new(&mut secs).speed(0.1).range(0.01..=3600.0)).changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::Timeout(secs);
                        }
                    }
                }
            });
        },

        BtNode::Limiter { mut max_per_interval, mut interval_secs } => {
            ui.horizontal(|ui| {
                ui.label("Max per interval:");
                if ui.add(egui::DragValue::new(&mut max_per_interval).range(1..=1000)).changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::Limiter { max_per_interval, interval_secs };
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Interval (s):");
                if ui.add(egui::DragValue::new(&mut interval_secs).speed(0.1).range(0.1..=3600.0)).changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::Limiter { max_per_interval, interval_secs };
                        }
                    }
                }
            });
        },

        BtNode::Wait(mut secs) => {
            ui.horizontal(|ui| {
                ui.label("Wait (s):");
                if ui.add(egui::DragValue::new(&mut secs).speed(0.05).range(0.0..=3600.0)).changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::Wait(secs);
                            n.label = format!("Wait {}s", secs);
                        }
                    }
                }
            });
        },

        BtNode::Log(mut msg) => {
            ui.label("Message:");
            if ui.text_edit_multiline(&mut msg).changed() {
                if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                    if let Some(n) = tree.nodes.get_mut(&id) {
                        n.node_type = BtNode::Log(msg.clone());
                        n.label = if msg.is_empty() { "Log".to_string() } else {
                            format!("Log: {}", &msg[..msg.len().min(20)])
                        };
                    }
                }
            }
        },

        BtNode::SetBlackboard { mut key, mut value } => {
            ui.horizontal(|ui| {
                ui.label("Key:");
                if ui.text_edit_singleline(&mut key).changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::SetBlackboard { key: key.clone(), value: value.clone() };
                        }
                    }
                }
            });
            ui.label("Value type:");
            let type_names = BlackboardValue::all_type_names();
            let current_type = value.type_name();
            let mut selected_type = current_type.to_string();
            egui::ComboBox::from_id_salt("set_bb_type")
                .selected_text(&selected_type)
                .show_ui(ui, |ui| {
                    for t in type_names {
                        if ui.selectable_label(*t == current_type, *t).clicked() {
                            selected_type = t.to_string();
                        }
                    }
                });
            if selected_type != current_type {
                value = BlackboardValue::default_for_type(&selected_type);
            }
            ui.label("Value:");
            let mut val_changed = false;
            match &mut value {
                BlackboardValue::Bool(b) => { val_changed = ui.checkbox(b, "").changed(); },
                BlackboardValue::Int(i) => { val_changed = ui.add(egui::DragValue::new(i)).changed(); },
                BlackboardValue::Float(f) => {
                    let mut fv = *f as f32;
                    if ui.add(egui::DragValue::new(&mut fv).speed(0.1)).changed() {
                        *f = fv as f64;
                        val_changed = true;
                    }
                },
                BlackboardValue::Str(s) => { val_changed = ui.text_edit_singleline(s).changed(); },
                BlackboardValue::Vec2(x, y) => {
                    ui.horizontal(|ui| {
                        val_changed |= ui.add(egui::DragValue::new(x).prefix("x:").speed(0.1)).changed();
                        val_changed |= ui.add(egui::DragValue::new(y).prefix("y:").speed(0.1)).changed();
                    });
                },
                BlackboardValue::EntityRef(r) => { val_changed = ui.text_edit_singleline(r).changed(); },
                BlackboardValue::List(_) => { ui.label("(list editing not supported here)"); },
            }
            if val_changed || selected_type != current_type {
                if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                    if let Some(n) = tree.nodes.get_mut(&id) {
                        n.node_type = BtNode::SetBlackboard { key: key.clone(), value: value.clone() };
                    }
                }
            }
        },

        BtNode::CheckBlackboard { mut key, mut op, mut value } => {
            ui.horizontal(|ui| {
                ui.label("Key:");
                if ui.text_edit_singleline(&mut key).changed() {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(n) = tree.nodes.get_mut(&id) {
                            n.node_type = BtNode::CheckBlackboard { key: key.clone(), op: op.clone(), value: value.clone() };
                        }
                    }
                }
            });
            ui.label("Operator:");
            let all_ops = CompareOp::all();
            egui::ComboBox::from_id_salt("check_bb_op")
                .selected_text(op.symbol())
                .show_ui(ui, |ui| {
                    for o in &all_ops {
                        if ui.selectable_label(*o == op, o.symbol()).clicked() {
                            op = o.clone();
                        }
                    }
                });
            ui.label("Value:");
            let type_names = BlackboardValue::all_type_names();
            let current_type = value.type_name();
            let mut selected_type = current_type.to_string();
            egui::ComboBox::from_id_salt("check_bb_type")
                .selected_text(&selected_type)
                .show_ui(ui, |ui| {
                    for t in type_names {
                        if ui.selectable_label(*t == current_type, *t).clicked() {
                            selected_type = t.to_string();
                        }
                    }
                });
            if selected_type != current_type {
                value = BlackboardValue::default_for_type(&selected_type);
            }
            let mut val_changed = false;
            match &mut value {
                BlackboardValue::Bool(b) => { val_changed = ui.checkbox(b, "").changed(); },
                BlackboardValue::Int(i) => { val_changed = ui.add(egui::DragValue::new(i)).changed(); },
                BlackboardValue::Float(f) => {
                    let mut fv = *f as f32;
                    if ui.add(egui::DragValue::new(&mut fv).speed(0.1)).changed() {
                        *f = fv as f64;
                        val_changed = true;
                    }
                },
                BlackboardValue::Str(s) => { val_changed = ui.text_edit_singleline(s).changed(); },
                BlackboardValue::Vec2(x, y) => {
                    ui.horizontal(|ui| {
                        val_changed |= ui.add(egui::DragValue::new(x).prefix("x:").speed(0.1)).changed();
                        val_changed |= ui.add(egui::DragValue::new(y).prefix("y:").speed(0.1)).changed();
                    });
                },
                BlackboardValue::EntityRef(r) => { val_changed = ui.text_edit_singleline(r).changed(); },
                BlackboardValue::List(_) => { ui.label("(list editing not supported here)"); },
            }
            if val_changed || selected_type != current_type {
                if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                    if let Some(n) = tree.nodes.get_mut(&id) {
                        n.node_type = BtNode::CheckBlackboard { key: key.clone(), op: op.clone(), value: value.clone() };
                    }
                }
            }
        },

        BtNode::Action(kind) => {
            show_action_properties(ui, editor, id, kind);
        },

        BtNode::Condition(kind) => {
            show_condition_properties(ui, editor, id, kind);
        },
    }
}

fn show_action_properties(
    ui: &mut egui::Ui,
    editor: &mut BehaviorTreeEditor,
    id: BtNodeId,
    kind: ActionKind,
) {
    let mut new_kind = kind.clone();
    let mut changed = false;

    match &mut new_kind {
        ActionKind::MoveTo { target, speed, tolerance } => {
            ui.label("Target Position:");
            ui.horizontal(|ui| {
                changed |= ui.add(egui::DragValue::new(&mut target.0).prefix("X:").speed(0.5)).changed();
                changed |= ui.add(egui::DragValue::new(&mut target.1).prefix("Y:").speed(0.5)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Speed:");
                changed |= ui.add(egui::DragValue::new(speed).speed(0.1).range(0.01..=100.0)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Tolerance:");
                changed |= ui.add(egui::DragValue::new(tolerance).speed(0.05).range(0.0..=50.0)).changed();
            });
        },
        ActionKind::Attack { target_key, damage, range } => {
            ui.horizontal(|ui| {
                ui.label("Target key:");
                changed |= ui.text_edit_singleline(target_key).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Damage:");
                changed |= ui.add(egui::DragValue::new(damage).speed(0.5).range(0.0..=10000.0)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Range:");
                changed |= ui.add(egui::DragValue::new(range).speed(0.1).range(0.0..=100.0)).changed();
            });
        },
        ActionKind::Flee { from_key, speed, distance } => {
            ui.horizontal(|ui| {
                ui.label("From key:");
                changed |= ui.text_edit_singleline(from_key).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Speed:");
                changed |= ui.add(egui::DragValue::new(speed).speed(0.1).range(0.01..=100.0)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Distance:");
                changed |= ui.add(egui::DragValue::new(distance).speed(0.5).range(0.0..=500.0)).changed();
            });
        },
        ActionKind::Patrol { waypoints, loop_patrol } => {
            changed |= ui.checkbox(loop_patrol, "Loop patrol").changed();
            ui.label(format!("Waypoints ({}): ", waypoints.len()));
            let mut to_remove: Option<usize> = None;
            for (i, wp) in waypoints.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("[{}]", i));
                    changed |= ui.add(egui::DragValue::new(&mut wp.0).prefix("X:").speed(1.0)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut wp.1).prefix("Y:").speed(1.0)).changed();
                    if ui.small_button("✗").clicked() {
                        to_remove = Some(i);
                        changed = true;
                    }
                });
            }
            if let Some(idx) = to_remove {
                waypoints.remove(idx);
            }
            if ui.button("+ Add Waypoint").clicked() {
                waypoints.push((0.0, 0.0));
                changed = true;
            }
        },
        ActionKind::Idle { duration } => {
            ui.horizontal(|ui| {
                ui.label("Duration (s):");
                changed |= ui.add(egui::DragValue::new(duration).speed(0.1).range(0.0..=3600.0)).changed();
            });
        },
        ActionKind::PlayAnimation { clip_name, blend_time, looping } => {
            ui.horizontal(|ui| {
                ui.label("Clip name:");
                changed |= ui.text_edit_singleline(clip_name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Blend time:");
                changed |= ui.add(egui::DragValue::new(blend_time).speed(0.01).range(0.0..=5.0)).changed();
            });
            changed |= ui.checkbox(looping, "Looping").changed();
        },
        ActionKind::EmitParticles { emitter_name, count, duration } => {
            ui.horizontal(|ui| {
                ui.label("Emitter:");
                changed |= ui.text_edit_singleline(emitter_name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Count:");
                changed |= ui.add(egui::DragValue::new(count).range(1..=10000)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Duration (s):");
                changed |= ui.add(egui::DragValue::new(duration).speed(0.1).range(0.0..=60.0)).changed();
            });
        },
        ActionKind::TriggerEvent { event_name, payload } => {
            ui.horizontal(|ui| {
                ui.label("Event name:");
                changed |= ui.text_edit_singleline(event_name).changed();
            });
            ui.label("Payload (JSON):");
            changed |= ui.text_edit_multiline(payload).changed();
        },
        ActionKind::CallScript { script_path, function_name, args } => {
            ui.horizontal(|ui| {
                ui.label("Script path:");
                changed |= ui.text_edit_singleline(script_path).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Function:");
                changed |= ui.text_edit_singleline(function_name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Args:");
                changed |= ui.text_edit_singleline(args).changed();
            });
        },
        ActionKind::UseAbility { ability_name, target_key } => {
            ui.horizontal(|ui| {
                ui.label("Ability:");
                changed |= ui.text_edit_singleline(ability_name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Target key:");
                changed |= ui.text_edit_singleline(target_key).changed();
            });
        },
        ActionKind::PickupItem { item_name, search_radius } => {
            ui.horizontal(|ui| {
                ui.label("Item name:");
                changed |= ui.text_edit_singleline(item_name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Search radius:");
                changed |= ui.add(egui::DragValue::new(search_radius).speed(0.1).range(0.0..=100.0)).changed();
            });
        },
        ActionKind::DropItem { item_name } => {
            ui.horizontal(|ui| {
                ui.label("Item name:");
                changed |= ui.text_edit_singleline(item_name).changed();
            });
        },
        ActionKind::OpenDoor { door_key, force } => {
            ui.horizontal(|ui| {
                ui.label("Door key:");
                changed |= ui.text_edit_singleline(door_key).changed();
            });
            changed |= ui.checkbox(force, "Force open").changed();
        },
        ActionKind::AlertAllies { radius, alert_type } => {
            ui.horizontal(|ui| {
                ui.label("Radius:");
                changed |= ui.add(egui::DragValue::new(radius).speed(0.5).range(0.0..=500.0)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Alert type:");
                changed |= ui.text_edit_singleline(alert_type).changed();
            });
        },
        ActionKind::Surrender => {
            ui.label("No parameters.");
        },
        ActionKind::FaceTarget { target_key, turn_speed } => {
            ui.horizontal(|ui| {
                ui.label("Target key:");
                changed |= ui.text_edit_singleline(target_key).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Turn speed (°/s):");
                changed |= ui.add(egui::DragValue::new(turn_speed).speed(1.0).range(1.0..=720.0)).changed();
            });
        },
        ActionKind::SetNavTarget { target_key, priority } => {
            ui.horizontal(|ui| {
                ui.label("Target key:");
                changed |= ui.text_edit_singleline(target_key).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Priority:");
                changed |= ui.add(egui::DragValue::new(priority)).changed();
            });
        },
        ActionKind::StopMovement => {
            ui.label("No parameters.");
        },
        ActionKind::PlaySound { sound_name, volume, spatial } => {
            ui.horizontal(|ui| {
                ui.label("Sound file:");
                changed |= ui.text_edit_singleline(sound_name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Volume:");
                changed |= ui.add(egui::DragValue::new(volume).speed(0.01).range(0.0..=1.0)).changed();
            });
            changed |= ui.checkbox(spatial, "Spatial audio").changed();
        },
        ActionKind::SpawnEntity { entity_name, offset } => {
            ui.horizontal(|ui| {
                ui.label("Entity name:");
                changed |= ui.text_edit_singleline(entity_name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Offset:");
                changed |= ui.add(egui::DragValue::new(&mut offset.0).prefix("X:").speed(0.5)).changed();
                changed |= ui.add(egui::DragValue::new(&mut offset.1).prefix("Y:").speed(0.5)).changed();
            });
        },
    }

    if changed {
        if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
            if let Some(n) = tree.nodes.get_mut(&id) {
                n.node_type = BtNode::Action(new_kind.clone());
                n.label = new_kind.display_name().to_string();
            }
        }
    }
}

fn show_condition_properties(
    ui: &mut egui::Ui,
    editor: &mut BehaviorTreeEditor,
    id: BtNodeId,
    kind: ConditionKind,
) {
    let mut new_kind = kind.clone();
    let mut changed = false;

    match &mut new_kind {
        ConditionKind::IsHealthBelow { threshold, use_percentage } => {
            changed |= ui.checkbox(use_percentage, "Use percentage").changed();
            ui.horizontal(|ui| {
                let label = if *use_percentage { "Threshold (0-1):" } else { "Threshold:" };
                ui.label(label);
                changed |= ui.add(egui::DragValue::new(threshold).speed(0.01).range(0.0..=if *use_percentage { 1.0 } else { 10000.0 })).changed();
            });
        },
        ConditionKind::IsHealthAbove { threshold, use_percentage } => {
            changed |= ui.checkbox(use_percentage, "Use percentage").changed();
            ui.horizontal(|ui| {
                let label = if *use_percentage { "Threshold (0-1):" } else { "Threshold:" };
                ui.label(label);
                changed |= ui.add(egui::DragValue::new(threshold).speed(0.01).range(0.0..=if *use_percentage { 1.0 } else { 10000.0 })).changed();
            });
        },
        ConditionKind::IsTargetInRange { target_key, range } => {
            ui.horizontal(|ui| {
                ui.label("Target key:");
                changed |= ui.text_edit_singleline(target_key).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Range:");
                changed |= ui.add(egui::DragValue::new(range).speed(0.5).range(0.0..=1000.0)).changed();
            });
        },
        ConditionKind::IsTargetVisible { target_key, use_los } => {
            ui.horizontal(|ui| {
                ui.label("Target key:");
                changed |= ui.text_edit_singleline(target_key).changed();
            });
            changed |= ui.checkbox(use_los, "Line of sight check").changed();
        },
        ConditionKind::HasItem { item_name, min_count } => {
            ui.horizontal(|ui| {
                ui.label("Item name:");
                changed |= ui.text_edit_singleline(item_name).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Min count:");
                changed |= ui.add(egui::DragValue::new(min_count).range(1..=9999)).changed();
            });
        },
        ConditionKind::IsOnCooldown { ability_name } => {
            ui.horizontal(|ui| {
                ui.label("Ability name:");
                changed |= ui.text_edit_singleline(ability_name).changed();
            });
        },
        ConditionKind::IsAlerted | ConditionKind::IsStunned => {
            ui.label("No parameters — checks global agent state.");
        },
        ConditionKind::IsPathClear { direction, distance } => {
            ui.label("Direction:");
            ui.horizontal(|ui| {
                changed |= ui.add(egui::DragValue::new(&mut direction.0).prefix("X:").speed(0.05).range(-1.0..=1.0)).changed();
                changed |= ui.add(egui::DragValue::new(&mut direction.1).prefix("Y:").speed(0.05).range(-1.0..=1.0)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Distance:");
                changed |= ui.add(egui::DragValue::new(distance).speed(0.5).range(0.1..=500.0)).changed();
            });
        },
        ConditionKind::CanSeePlayer { fov_degrees, max_distance } => {
            ui.horizontal(|ui| {
                ui.label("FOV (degrees):");
                changed |= ui.add(egui::DragValue::new(fov_degrees).speed(1.0).range(1.0..=360.0)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Max distance:");
                changed |= ui.add(egui::DragValue::new(max_distance).speed(0.5).range(0.0..=1000.0)).changed();
            });
        },
        ConditionKind::HasBlackboardKey { key } => {
            ui.horizontal(|ui| {
                ui.label("Key:");
                changed |= ui.text_edit_singleline(key).changed();
            });
        },
        ConditionKind::CheckFlag { flag_name } => {
            ui.horizontal(|ui| {
                ui.label("Flag name:");
                changed |= ui.text_edit_singleline(flag_name).changed();
            });
        },
        ConditionKind::IsTargetDead { target_key } => {
            ui.horizontal(|ui| {
                ui.label("Target key:");
                changed |= ui.text_edit_singleline(target_key).changed();
            });
        },
        ConditionKind::IsNearWall { distance } => {
            ui.horizontal(|ui| {
                ui.label("Distance:");
                changed |= ui.add(egui::DragValue::new(distance).speed(0.05).range(0.0..=50.0)).changed();
            });
        },
        ConditionKind::IsFacingTarget { target_key, tolerance_degrees } => {
            ui.horizontal(|ui| {
                ui.label("Target key:");
                changed |= ui.text_edit_singleline(target_key).changed();
            });
            ui.horizontal(|ui| {
                ui.label("Tolerance (°):");
                changed |= ui.add(egui::DragValue::new(tolerance_degrees).speed(0.5).range(0.0..=180.0)).changed();
            });
        },
        ConditionKind::IsTimeOfDay { hour_min, hour_max } => {
            ui.horizontal(|ui| {
                ui.label("From hour:");
                changed |= ui.add(egui::DragValue::new(hour_min).speed(0.5).range(0.0..=23.0)).changed();
            });
            ui.horizontal(|ui| {
                ui.label("To hour:");
                changed |= ui.add(egui::DragValue::new(hour_max).speed(0.5).range(0.0..=23.0)).changed();
            });
        },
    }

    if changed {
        if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
            if let Some(n) = tree.nodes.get_mut(&id) {
                n.node_type = BtNode::Condition(new_kind.clone());
                n.label = new_kind.display_name().to_string();
            }
        }
    }
}

fn show_blackboard_panel(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor) {
    ui.horizontal(|ui| {
        ui.heading("Blackboard");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear All").clicked() {
                editor.blackboard.entries.clear();
                editor.blackboard.metadata.clear();
            }
            if ui.button("+ Add").clicked() {
                editor.bb_editor.show_add_row = true;
            }
        });
    });

    if editor.bb_editor.show_add_row {
        ui.horizontal(|ui| {
            ui.label("Key:");
            ui.text_edit_singleline(&mut editor.bb_editor.new_key);
            ui.label("Type:");
            egui::ComboBox::from_id_salt("bb_type_select")
                .selected_text(&editor.bb_editor.new_type)
                .show_ui(ui, |ui| {
                    for t in BlackboardValue::all_type_names() {
                        ui.selectable_value(&mut editor.bb_editor.new_type, t.to_string(), *t);
                    }
                });
            if ui.button("Add").clicked() && !editor.bb_editor.new_key.is_empty() {
                let val = BlackboardValue::default_for_type(&editor.bb_editor.new_type.clone());
                let key = editor.bb_editor.new_key.clone();
                editor.blackboard.set(key, val);
                editor.bb_editor.new_key.clear();
                editor.bb_editor.show_add_row = false;
            }
            if ui.button("Cancel").clicked() {
                editor.bb_editor.show_add_row = false;
            }
        });
    }

    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("bb_scroll")
        .max_height(200.0)
        .show(ui, |ui| {
            let keys: Vec<String> = editor.blackboard.entries.keys().cloned().collect();

            if keys.is_empty() {
                ui.colored_label(Color32::from_rgb(120, 120, 140), "(empty blackboard)");
                return;
            }

            egui::Grid::new("bb_grid")
                .num_columns(5)
                .striped(true)
                .spacing([4.0, 2.0])
                .show(ui, |ui| {
                    ui.colored_label(Color32::from_rgb(180, 180, 200), "Key");
                    ui.colored_label(Color32::from_rgb(180, 180, 200), "Type");
                    ui.colored_label(Color32::from_rgb(180, 180, 200), "Value");
                    ui.colored_label(Color32::from_rgb(180, 180, 200), "Deps");
                    ui.colored_label(Color32::from_rgb(180, 180, 200), "");
                    ui.end_row();

                    let mut to_delete: Option<String> = None;
                    let mut to_edit: Option<(String, BlackboardValue)> = None;

                    for key in &keys {
                        let val = match editor.blackboard.entries.get(key) {
                            Some(v) => v.clone(),
                            None => continue,
                        };
                        let meta = editor.blackboard.metadata.get(key);
                        let read_count = meta.map(|m| m.read_by.len()).unwrap_or(0);
                        let write_count = meta.map(|m| m.written_by.len()).unwrap_or(0);

                        ui.label(key.as_str());
                        ui.colored_label(
                            NodeCategory::Leaf.color(),
                            val.type_name(),
                        );

                        // Inline value editing
                        let editing = editor.bb_editor.editing_key.as_deref() == Some(key.as_str());
                        if editing {
                            if ui.text_edit_singleline(&mut editor.bb_editor.edit_buffer).lost_focus() {
                                // Parse and commit
                                let new_val = parse_blackboard_value(&editor.bb_editor.edit_buffer, &val);
                                to_edit = Some((key.clone(), new_val));
                                editor.bb_editor.editing_key = None;
                            }
                        } else {
                            let display = val.display_string();
                            if ui.selectable_label(false, &display).double_clicked() {
                                editor.bb_editor.editing_key = Some(key.clone());
                                editor.bb_editor.edit_buffer = display;
                            }
                        }

                        let dep_text = if read_count + write_count == 0 {
                            "none".to_string()
                        } else {
                            format!("R:{} W:{}", read_count, write_count)
                        };
                        ui.label(dep_text);

                        if ui.small_button("✗").clicked() {
                            to_delete = Some(key.clone());
                        }
                        ui.end_row();
                    }

                    if let Some(k) = to_delete {
                        editor.blackboard.remove(&k);
                    }
                    if let Some((k, v)) = to_edit {
                        editor.blackboard.set(k, v);
                    }
                });
        });
}

fn parse_blackboard_value(s: &str, existing: &BlackboardValue) -> BlackboardValue {
    match existing {
        BlackboardValue::Bool(_) => BlackboardValue::Bool(s.trim().to_lowercase() == "true" || s.trim() == "1"),
        BlackboardValue::Int(_) => BlackboardValue::Int(s.trim().parse().unwrap_or(0)),
        BlackboardValue::Float(_) => BlackboardValue::Float(s.trim().parse().unwrap_or(0.0)),
        BlackboardValue::Str(_) => BlackboardValue::Str(s.to_string()),
        BlackboardValue::Vec2(_, _) => {
            let parts: Vec<f32> = s.trim_matches(|c| c == '(' || c == ')')
                .split(',')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            BlackboardValue::Vec2(
                parts.get(0).copied().unwrap_or(0.0),
                parts.get(1).copied().unwrap_or(0.0),
            )
        },
        BlackboardValue::EntityRef(_) => BlackboardValue::EntityRef(s.trim_start_matches('@').to_string()),
        BlackboardValue::List(_) => existing.clone(),
    }
}

fn show_simulation_panel(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor) {
    ui.horizontal(|ui| {
        ui.heading("Simulation");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("Tick: {}", editor.simulation.tick));
        });
    });

    ui.horizontal(|ui| {
        // Play/Pause
        let play_label = if editor.simulation.running { "⏸ Pause" } else { "▶ Play" };
        if ui.button(play_label).clicked() {
            editor.simulation.running = !editor.simulation.running;
        }

        // Step
        if ui.button("⏭ Step").clicked() {
            editor.simulation_step();
        }

        // Reset
        if ui.button("⏹ Reset").clicked() {
            editor.simulation_reset();
        }

        // Speed
        ui.label("Speed:");
        let speeds = [1.0f32, 2.0, 5.0, 10.0];
        for s in &speeds {
            if ui.selectable_label(editor.simulation.speed == *s, format!("{}x", s)).clicked() {
                editor.simulation.speed = *s;
            }
        }
    });

    ui.horizontal(|ui| {
        // Breakpoints
        ui.label(format!("Breakpoints: {}", editor.breakpoints.len()));
        if ui.small_button("Clear All BPs").clicked() {
            editor.breakpoints.clear();
        }
    });

    ui.separator();
    ui.label("Execution Log:");

    let log_max_height = 120.0;
    egui::ScrollArea::vertical()
        .id_salt("exec_log_scroll")
        .max_height(log_max_height)
        .stick_to_bottom(editor.log_scroll_to_bottom)
        .show(ui, |ui| {
            if editor.execution_log.is_empty() {
                ui.colored_label(Color32::from_rgb(100, 100, 120), "(no executions yet)");
            } else {
                let tree = editor.trees.get(editor.active_tree);
                for (tick, node_id, status) in editor.execution_log.iter().rev().take(50) {
                    let node_label = tree
                        .and_then(|t| t.nodes.get(node_id))
                        .map(|n| n.label.as_str())
                        .unwrap_or("?");
                    let color = status.color();
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(100, 100, 120), format!("[{}]", tick));
                        ui.colored_label(Color32::WHITE, node_label);
                        ui.colored_label(color, format!("→ {}", status.label()));
                    });
                }
            }
        });
    editor.log_scroll_to_bottom = false;
}

fn show_tree_tab_bar(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor) {
    ui.horizontal(|ui| {
        let mut new_active: Option<usize> = None;
        let mut to_delete: Option<usize> = None;

        let tree_count = editor.trees.len();
        for i in 0..tree_count {
            let is_active = i == editor.active_tree;

            if editor.tree_rename.editing_idx == Some(i) {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut editor.tree_rename.buffer)
                        .desired_width(80.0)
                );
                if resp.lost_focus() || ui.input(|inp| inp.key_pressed(egui::Key::Enter)) {
                    let new_name = editor.tree_rename.buffer.clone();
                    if !new_name.is_empty() {
                        if let Some(t) = editor.trees.get_mut(i) {
                            t.name = new_name;
                        }
                    }
                    editor.tree_rename.editing_idx = None;
                }
            } else {
                let tree_name = editor.trees[i].name.clone();
                let tab_resp = ui.selectable_label(is_active, &tree_name);
                if tab_resp.clicked() {
                    new_active = Some(i);
                }
                if tab_resp.double_clicked() {
                    editor.tree_rename.editing_idx = Some(i);
                    editor.tree_rename.buffer = tree_name.clone();
                }
                if tab_resp.secondary_clicked() {
                    // Show small context menu
                    to_delete = Some(i);
                }
            }
        }

        if ui.button("+").clicked() {
            editor.add_new_tree();
        }

        if let Some(idx) = new_active {
            editor.active_tree = idx;
            editor.selected_node = None;
            editor.selected_nodes.clear();
        }
        if let Some(idx) = to_delete {
            editor.delete_tree(idx);
        }
    });
}

fn show_toolbar(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor, canvas_rect: Rect) {
    ui.horizontal(|ui| {
        // Layout buttons
        ui.label("Layout:");
        if ui.selectable_label(editor.layout_mode == LayoutMode::Manual, "Manual").clicked() {
            editor.layout_mode = LayoutMode::Manual;
        }
        if ui.selectable_label(editor.layout_mode == LayoutMode::AutoHorizontal, "Auto H").clicked() {
            editor.layout_mode = LayoutMode::AutoHorizontal;
            editor.apply_layout();
        }
        if ui.selectable_label(editor.layout_mode == LayoutMode::AutoVertical, "Auto V").clicked() {
            editor.layout_mode = LayoutMode::AutoVertical;
            editor.apply_layout();
        }

        ui.separator();

        if ui.button("Fit View").clicked() {
            editor.do_fit_to_view(canvas_rect);
        }

        if ui.button("Re-Layout").clicked() {
            editor.apply_layout();
        }

        ui.separator();

        ui.checkbox(&mut editor.show_grid, "Grid");
        ui.checkbox(&mut editor.snap_to_grid, "Snap");
        ui.checkbox(&mut editor.show_minimap, "Minimap");

        ui.separator();

        if ui.button("Validate").clicked() {
            editor.validate_active_tree();
            editor.validation.show_errors = true;
        }

        if ui.button("Export JSON").clicked() {
            if let Some(tree) = editor.trees.get(editor.active_tree) {
                editor.import_export.export_text = tree.to_json();
                editor.import_export.show_export = true;
            }
        }

        if ui.button("Import JSON").clicked() {
            editor.import_export.show_import = true;
        }

        ui.separator();

        // Zoom display
        ui.label(format!("Zoom: {:.0}%", editor.canvas_zoom * 100.0));
        if ui.small_button("-").clicked() {
            editor.canvas_zoom = (editor.canvas_zoom - ZOOM_STEP).max(MIN_ZOOM);
        }
        if ui.small_button("+").clicked() {
            editor.canvas_zoom = (editor.canvas_zoom + ZOOM_STEP).min(MAX_ZOOM);
        }
        if ui.small_button("1:1").clicked() {
            editor.canvas_zoom = 1.0;
        }

        // Panel toggles
        ui.separator();
        ui.toggle_value(&mut editor.show_node_palette, "Palette");
        ui.toggle_value(&mut editor.show_properties, "Properties");
        ui.toggle_value(&mut editor.show_blackboard, "Blackboard");
        ui.toggle_value(&mut editor.show_simulation, "Simulation");
    });
}

fn show_context_menu(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor, canvas_rect: Rect) {
    let ctx_state = match editor.context_menu.take() {
        Some(s) if s.open => s,
        _ => return,
    };

    let mut keep_open = true;
    let mut action: Option<ContextMenuAction> = None;

    egui::Area::new(egui::Id::new("bt_context_menu"))
        .fixed_pos(ctx_state.position)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(160.0);

                match &ctx_state.target {
                    ContextMenuTarget::Node(node_id) => {
                        let nid = *node_id;
                        ui.colored_label(Color32::from_rgb(180, 180, 200), "Node Actions");
                        ui.separator();
                        if ui.button("Select").clicked() { action = Some(ContextMenuAction::SelectNode(nid)); }
                        if ui.button("Edit Properties").clicked() { action = Some(ContextMenuAction::EditNode(nid)); }
                        ui.separator();
                        if ui.button("Duplicate").clicked() { action = Some(ContextMenuAction::DuplicateNode(nid)); }
                        if ui.button("Copy Subtree").clicked() { action = Some(ContextMenuAction::CopySubtree(nid)); }
                        if ui.button("Cut").clicked() { action = Some(ContextMenuAction::CutNode(nid)); }
                        ui.separator();
                        if ui.button("Add Child").clicked() { action = Some(ContextMenuAction::AddChildTo(nid)); }
                        if ui.button("Detach from Parent").clicked() { action = Some(ContextMenuAction::DetachFromParent(nid)); }
                        if ui.button("Set as Root").clicked() { action = Some(ContextMenuAction::SetRoot(nid)); }
                        ui.separator();
                        if ui.button("Reorder Child Up").clicked() { action = Some(ContextMenuAction::ReorderUp(nid)); }
                        if ui.button("Reorder Child Down").clicked() { action = Some(ContextMenuAction::ReorderDown(nid)); }
                        ui.separator();
                        let has_bp = editor.breakpoints.contains(&nid);
                        if ui.button(if has_bp { "Clear Breakpoint" } else { "Set Breakpoint" }).clicked() {
                            action = Some(ContextMenuAction::ToggleBreakpoint(nid));
                        }
                        if ui.button("Collapse All Children").clicked() {
                            action = Some(ContextMenuAction::CollapseChildren(nid));
                        }
                        if ui.button("Expand All Children").clicked() {
                            action = Some(ContextMenuAction::ExpandChildren(nid));
                        }
                        ui.separator();
                        if ui.button("Delete Node").clicked() { action = Some(ContextMenuAction::DeleteNode(nid)); }
                        if ui.button("Delete Subtree").clicked() { action = Some(ContextMenuAction::DeleteSubtree(nid)); }
                    },
                    ContextMenuTarget::Canvas(pos) => {
                        let canvas_pos = *pos;
                        ui.colored_label(Color32::from_rgb(180, 180, 200), "Canvas Actions");
                        ui.separator();
                        if ui.button("Select All").clicked() { action = Some(ContextMenuAction::SelectAll); }
                        if editor.clipboard_subtree.is_some() {
                            if ui.button("Paste").clicked() { action = Some(ContextMenuAction::Paste(canvas_pos)); }
                        }
                        ui.separator();
                        ui.menu_button("Add Node", |ui| {
                            for cat in NodeCategory::all() {
                                ui.menu_button(cat.label(), |ui| {
                                    match cat {
                                        NodeCategory::Composite => {
                                            let nodes = [
                                                ("Sequence", BtNode::Sequence),
                                                ("Selector", BtNode::Selector),
                                                ("Parallel", BtNode::Parallel(ParallelPolicy::RequireAll)),
                                                ("Random Selector", BtNode::RandomSelector),
                                            ];
                                            for (name, nt) in nodes {
                                                if ui.button(name).clicked() {
                                                    action = Some(ContextMenuAction::AddNode(canvas_pos, nt));
                                                }
                                            }
                                        },
                                        NodeCategory::Decorator => {
                                            let nodes = [
                                                ("Inverter", BtNode::Inverter),
                                                ("Succeeder", BtNode::Succeeder),
                                                ("Failer", BtNode::Failer),
                                                ("Repeat", BtNode::Repeat(3)),
                                                ("Repeat Until Fail", BtNode::RepeatUntilFail),
                                                ("Cooldown", BtNode::Cooldown { duration_secs: 5.0, shared_key: None }),
                                                ("Timeout", BtNode::Timeout(3.0)),
                                                ("Limiter", BtNode::Limiter { max_per_interval: 3, interval_secs: 10.0 }),
                                            ];
                                            for (name, nt) in nodes {
                                                if ui.button(name).clicked() {
                                                    action = Some(ContextMenuAction::AddNode(canvas_pos, nt));
                                                }
                                            }
                                        },
                                        NodeCategory::Action => {
                                            for k in ActionKind::all_variants() {
                                                let name = k.display_name().to_string();
                                                if ui.button(&name).clicked() {
                                                    action = Some(ContextMenuAction::AddNode(canvas_pos, BtNode::Action(k)));
                                                }
                                            }
                                        },
                                        NodeCategory::Condition => {
                                            for k in ConditionKind::all_variants() {
                                                let name = k.display_name().to_string();
                                                if ui.button(&name).clicked() {
                                                    action = Some(ContextMenuAction::AddNode(canvas_pos, BtNode::Condition(k)));
                                                }
                                            }
                                        },
                                        NodeCategory::Leaf => {
                                            let nodes = [
                                                ("Wait", BtNode::Wait(1.0)),
                                                ("Log", BtNode::Log("message".into())),
                                                ("Set Blackboard", BtNode::SetBlackboard { key: "key".into(), value: BlackboardValue::Bool(true) }),
                                                ("Check Blackboard", BtNode::CheckBlackboard { key: "key".into(), op: CompareOp::Equal, value: BlackboardValue::Bool(true) }),
                                            ];
                                            for (name, nt) in nodes {
                                                if ui.button(name).clicked() {
                                                    action = Some(ContextMenuAction::AddNode(canvas_pos, nt));
                                                }
                                            }
                                        },
                                    }
                                });
                            }
                        });
                        ui.separator();
                        if ui.button("Auto Layout (H)").clicked() {
                            action = Some(ContextMenuAction::AutoLayoutH);
                        }
                        if ui.button("Auto Layout (V)").clicked() {
                            action = Some(ContextMenuAction::AutoLayoutV);
                        }
                        if ui.button("Fit to View").clicked() {
                            action = Some(ContextMenuAction::FitView);
                        }
                    },
                    ContextMenuTarget::Connection(from, to) => {
                        ui.colored_label(Color32::from_rgb(180, 180, 200), "Connection Actions");
                        ui.separator();
                        if ui.button("Remove Connection").clicked() {
                            action = Some(ContextMenuAction::RemoveConnection(*from, *to));
                        }
                        if ui.button("Insert Node Between").clicked() {
                            action = Some(ContextMenuAction::InsertNodeBetween(*from, *to));
                        }
                    },
                }

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) || ui.input(|i| i.pointer.any_click()) {
                    keep_open = false;
                }
            });
        });

    if let Some(act) = action {
        execute_context_action(editor, act, canvas_rect);
        keep_open = false;
    }

    if keep_open {
        editor.context_menu = Some(ctx_state);
    }
}

#[derive(Debug)]
enum ContextMenuAction {
    SelectNode(BtNodeId),
    EditNode(BtNodeId),
    DuplicateNode(BtNodeId),
    CopySubtree(BtNodeId),
    CutNode(BtNodeId),
    AddChildTo(BtNodeId),
    DetachFromParent(BtNodeId),
    SetRoot(BtNodeId),
    ReorderUp(BtNodeId),
    ReorderDown(BtNodeId),
    ToggleBreakpoint(BtNodeId),
    CollapseChildren(BtNodeId),
    ExpandChildren(BtNodeId),
    DeleteNode(BtNodeId),
    DeleteSubtree(BtNodeId),
    SelectAll,
    Paste(Pos2),
    AddNode(Pos2, BtNode),
    AutoLayoutH,
    AutoLayoutV,
    FitView,
    RemoveConnection(BtNodeId, BtNodeId),
    InsertNodeBetween(BtNodeId, BtNodeId),
}

fn execute_context_action(editor: &mut BehaviorTreeEditor, action: ContextMenuAction, canvas_rect: Rect) {
    match action {
        ContextMenuAction::SelectNode(id) => {
            editor.selected_node = Some(id);
            editor.selected_nodes.clear();
            editor.selected_nodes.insert(id);
        },
        ContextMenuAction::EditNode(id) => {
            editor.selected_node = Some(id);
            editor.show_properties = true;
        },
        ContextMenuAction::DuplicateNode(id) => {
            if let Some(new_id) = editor.duplicate_subtree(id) {
                editor.selected_node = Some(new_id);
            }
        },
        ContextMenuAction::CopySubtree(id) => {
            editor.copy_subtree(id);
        },
        ContextMenuAction::CutNode(id) => {
            editor.copy_subtree(id);
            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                tree.remove_node(id);
            }
            editor.selected_node = None;
        },
        ContextMenuAction::AddChildTo(parent_id) => {
            let parent_pos = editor.trees.get(editor.active_tree)
                .and_then(|t| t.nodes.get(&parent_id))
                .map(|n| n.position)
                .unwrap_or((200.0, 200.0));
            let new_pos = Pos2::new(parent_pos.0 + 40.0, parent_pos.1 + 120.0);
            let new_id = editor.add_node_to_tree(BtNode::Sequence, new_pos, Some(parent_id));
            editor.selected_node = Some(new_id);
        },
        ContextMenuAction::DetachFromParent(id) => {
            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                if let Some(parent_id) = tree.nodes.get(&id).and_then(|n| n.parent) {
                    tree.remove_child(parent_id, id);
                }
            }
        },
        ContextMenuAction::SetRoot(id) => {
            editor.set_root(id);
        },
        ContextMenuAction::ReorderUp(id) => {
            editor.reorder_child_up(id);
        },
        ContextMenuAction::ReorderDown(id) => {
            editor.reorder_child_down(id);
        },
        ContextMenuAction::ToggleBreakpoint(id) => {
            if editor.breakpoints.contains(&id) {
                editor.breakpoints.remove(&id);
            } else {
                editor.breakpoints.insert(id);
            }
        },
        ContextMenuAction::CollapseChildren(id) => {
            editor.collapse_all_children(id);
        },
        ContextMenuAction::ExpandChildren(id) => {
            editor.expand_all_children(id);
        },
        ContextMenuAction::DeleteNode(id) => {
            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                // Only remove the node, not subtree
                if let Some(node) = tree.nodes.remove(&id) {
                    if let Some(parent_id) = node.parent {
                        if let Some(parent) = tree.nodes.get_mut(&parent_id) {
                            parent.children.retain(|&c| c != id);
                        }
                    }
                    // Re-attach children to the deleted node's parent
                    for child_id in &node.children {
                        if let Some(child) = tree.nodes.get_mut(child_id) {
                            child.parent = node.parent;
                        }
                    }
                }
            }
            if editor.selected_node == Some(id) {
                editor.selected_node = None;
            }
        },
        ContextMenuAction::DeleteSubtree(id) => {
            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                tree.remove_node(id);
            }
            if editor.selected_node == Some(id) {
                editor.selected_node = None;
            }
            editor.selected_nodes.remove(&id);
        },
        ContextMenuAction::SelectAll => {
            editor.select_all();
        },
        ContextMenuAction::Paste(pos) => {
            editor.paste_subtree(pos);
        },
        ContextMenuAction::AddNode(pos, nt) => {
            let id = editor.add_node_to_tree(nt, pos, None);
            editor.selected_node = Some(id);
        },
        ContextMenuAction::AutoLayoutH => {
            editor.layout_mode = LayoutMode::AutoHorizontal;
            editor.apply_layout();
        },
        ContextMenuAction::AutoLayoutV => {
            editor.layout_mode = LayoutMode::AutoVertical;
            editor.apply_layout();
        },
        ContextMenuAction::FitView => {
            editor.do_fit_to_view(canvas_rect);
        },
        ContextMenuAction::RemoveConnection(from, to) => {
            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                tree.remove_child(from, to);
            }
        },
        ContextMenuAction::InsertNodeBetween(parent_id, child_id) => {
            let parent_pos = editor.trees.get(editor.active_tree)
                .and_then(|t| t.nodes.get(&parent_id))
                .map(|n| n.position)
                .unwrap_or((200.0, 200.0));
            let child_pos = editor.trees.get(editor.active_tree)
                .and_then(|t| t.nodes.get(&child_id))
                .map(|n| n.position)
                .unwrap_or((200.0, 300.0));
            let mid = Pos2::new(
                (parent_pos.0 + child_pos.0) / 2.0,
                (parent_pos.1 + child_pos.1) / 2.0,
            );
            let new_id = editor.next_id();
            let new_node = BtNodeData::new(new_id, BtNode::Sequence, (mid.x, mid.y));
            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                tree.nodes.insert(new_id, new_node);
                // Reroute: parent -> new, new -> child
                if let Some(parent) = tree.nodes.get_mut(&parent_id) {
                    if let Some(idx) = parent.children.iter().position(|&c| c == child_id) {
                        parent.children[idx] = new_id;
                    }
                }
                if let Some(nn) = tree.nodes.get_mut(&new_id) {
                    nn.parent = Some(parent_id);
                    nn.children.push(child_id);
                }
                if let Some(child) = tree.nodes.get_mut(&child_id) {
                    child.parent = Some(new_id);
                }
            }
            editor.selected_node = Some(new_id);
        },
    }
}

fn show_validation_window(ctx: &egui::Context, editor: &mut BehaviorTreeEditor) {
    if !editor.validation.show_errors { return; }
    let mut open = true;
    egui::Window::new("Tree Validation")
        .open(&mut open)
        .resizable(true)
        .default_size([400.0, 300.0])
        .show(ctx, |ui| {
            if editor.validation.errors.is_empty() {
                ui.colored_label(COLOR_STATUS_SUCCESS, "✓ No errors found.");
            } else {
                ui.colored_label(COLOR_STATUS_FAILURE, format!("✗ {} error(s):", editor.validation.errors.len()));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for err in &editor.validation.errors {
                        ui.horizontal(|ui| {
                            ui.colored_label(COLOR_STATUS_FAILURE, "•");
                            ui.label(err);
                        });
                    }
                });
            }
            if ui.button("Re-validate").clicked() {
                editor.validate_active_tree();
            }
        });
    if !open {
        editor.validation.show_errors = false;
    }
}

fn show_export_window(ctx: &egui::Context, editor: &mut BehaviorTreeEditor) {
    if !editor.import_export.show_export { return; }
    let mut open = true;
    egui::Window::new("Export Tree as JSON")
        .open(&mut open)
        .resizable(true)
        .default_size([600.0, 400.0])
        .show(ctx, |ui| {
            ui.label("Copy the JSON below:");
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    let text = editor.import_export.export_text.clone();
                    let mut text_mut = text.clone();
                    ui.add(egui::TextEdit::multiline(&mut text_mut)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(15)
                        .desired_width(f32::INFINITY));
                });
            if ui.button("Close").clicked() {
                editor.import_export.show_export = false;
            }
        });
    if !open {
        editor.import_export.show_export = false;
    }
}

fn show_import_window(ctx: &egui::Context, editor: &mut BehaviorTreeEditor) {
    if !editor.import_export.show_import { return; }
    let mut open = true;
    egui::Window::new("Import Tree from JSON")
        .open(&mut open)
        .resizable(true)
        .default_size([600.0, 400.0])
        .show(ctx, |ui| {
            ui.label("Paste JSON below:");
            egui::ScrollArea::vertical()
                .max_height(250.0)
                .show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut editor.import_export.import_text)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(12)
                        .desired_width(f32::INFINITY));
                });

            if let Some(err) = &editor.import_export.import_error {
                ui.colored_label(COLOR_STATUS_FAILURE, format!("Error: {}", err));
            }

            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    let text = editor.import_export.import_text.clone();
                    match BehaviorTree::from_json(&text) {
                        Ok(tree) => {
                            editor.trees.push(tree);
                            editor.active_tree = editor.trees.len() - 1;
                            editor.import_export.import_error = None;
                            editor.import_export.show_import = false;
                        },
                        Err(e) => {
                            editor.import_export.import_error = Some(e);
                        },
                    }
                }
                if ui.button("Cancel").clicked() {
                    editor.import_export.show_import = false;
                    editor.import_export.import_error = None;
                }
            });
        });
    if !open {
        editor.import_export.show_import = false;
    }
}

// ============================================================
// CANVAS INTERACTION
// ============================================================

fn handle_canvas_interaction(
    ui: &mut egui::Ui,
    editor: &mut BehaviorTreeEditor,
    canvas_response: &egui::Response,
    canvas_rect: Rect,
) {
    let pointer_pos = ui.ctx().pointer_latest_pos();
    let Some(ptr) = pointer_pos else { return; };

    // Update hovered node
    editor.hovered_node = editor.node_at_screen_pos(ptr);

    // Scroll wheel zoom
    let scroll_delta = ui.ctx().input(|i| i.smooth_scroll_delta);
    if canvas_response.hovered() && scroll_delta.y.abs() > 0.1 {
        let zoom_delta = scroll_delta.y * 0.001;
        let old_zoom = editor.canvas_zoom;
        editor.canvas_zoom = (editor.canvas_zoom + zoom_delta).clamp(MIN_ZOOM, MAX_ZOOM);

        // Zoom toward pointer
        let canvas_ptr = (ptr - editor.canvas_offset) / old_zoom;
        let new_screen = canvas_ptr * editor.canvas_zoom;
        editor.canvas_offset = ptr - new_screen;
    }

    // Pinch zoom
    let zoom_delta_pinch = ui.ctx().input(|i| i.zoom_delta());
    if canvas_response.hovered() && (zoom_delta_pinch - 1.0).abs() > 0.001 {
        let old_zoom = editor.canvas_zoom;
        editor.canvas_zoom = (editor.canvas_zoom * zoom_delta_pinch).clamp(MIN_ZOOM, MAX_ZOOM);
        let canvas_ptr = (ptr - editor.canvas_offset) / old_zoom;
        let new_screen = canvas_ptr * editor.canvas_zoom;
        editor.canvas_offset = ptr - new_screen;
    }

    // Middle mouse pan
    let middle_down = ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Middle));
    let primary_down = ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Primary));
    let secondary_down = ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
    let primary_pressed = ui.ctx().input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
    let primary_released = ui.ctx().input(|i| i.pointer.button_released(egui::PointerButton::Primary));
    let secondary_pressed = ui.ctx().input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary));

    let drag_delta = canvas_response.drag_delta();

    match editor.drag_state.clone() {
        DragState::None => {
            if !canvas_rect.contains(ptr) { return; }

            if middle_down && drag_delta.length() > 0.0 {
                editor.drag_state = DragState::PanCanvas { last_pos: ptr };
                editor.canvas_offset += drag_delta;
            }

            if primary_pressed {
                if let Some(hovered_id) = editor.hovered_node {
                    // Check if clicked the toggle button
                    let toggle_clicked = {
                        let tree = editor.trees.get(editor.active_tree);
                        tree.and_then(|t| t.nodes.get(&hovered_id)).map(|n| {
                            let screen_rect = editor.node_screen_rect(n);
                            let header_h = NODE_HEADER_HEIGHT * editor.canvas_zoom;
                            let toggle_rect = Rect::from_center_size(
                                Pos2::new(screen_rect.max.x - 14.0 * editor.canvas_zoom, screen_rect.min.y + header_h / 2.0),
                                Vec2::splat(14.0 * editor.canvas_zoom),
                            );
                            toggle_rect.contains(ptr)
                        }).unwrap_or(false)
                    };

                    if toggle_clicked {
                        if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                            if let Some(node) = tree.nodes.get_mut(&hovered_id) {
                                node.collapsed = !node.collapsed;
                            }
                        }
                    } else {
                        // Check if ctrl held for multi-select
                        let ctrl = ui.ctx().input(|i| i.modifiers.ctrl);
                        if ctrl {
                            if editor.selected_nodes.contains(&hovered_id) {
                                editor.selected_nodes.remove(&hovered_id);
                                if editor.selected_node == Some(hovered_id) {
                                    editor.selected_node = editor.selected_nodes.iter().next().copied();
                                }
                            } else {
                                editor.selected_nodes.insert(hovered_id);
                                editor.selected_node = Some(hovered_id);
                            }
                        } else {
                            editor.selected_node = Some(hovered_id);
                            if !editor.selected_nodes.contains(&hovered_id) {
                                editor.selected_nodes.clear();
                                editor.selected_nodes.insert(hovered_id);
                            }
                        }

                        // Begin node drag
                        let offset = {
                            let tree = editor.trees.get(editor.active_tree);
                            tree.and_then(|t| t.nodes.get(&hovered_id)).map(|n| {
                                ptr - editor.canvas_to_screen(Pos2::new(n.position.0, n.position.1))
                            }).unwrap_or(Vec2::ZERO)
                        };
                        editor.drag_state = DragState::DraggingNode { id: hovered_id, offset };
                    }
                } else {
                    // Clicked empty canvas
                    editor.selected_node = None;
                    editor.selected_nodes.clear();
                    editor.drag_state = DragState::SelectionBox { start: ptr };
                    editor.selection_box = Some(SelectionBox { start: ptr, end: ptr });
                }
            }

            if secondary_pressed {
                let canvas_pos = editor.screen_to_canvas(ptr);
                let target = if let Some(hovered_id) = editor.hovered_node {
                    ContextMenuTarget::Node(hovered_id)
                } else {
                    ContextMenuTarget::Canvas(canvas_pos)
                };
                editor.context_menu = Some(ContextMenuState {
                    target,
                    position: ptr,
                    open: true,
                });
            }
        },

        DragState::DraggingNode { id, offset } => {
            if primary_down {
                let new_canvas_pos = editor.screen_to_canvas(ptr - offset);
                let mut nx = new_canvas_pos.x;
                let mut ny = new_canvas_pos.y;
                if editor.snap_to_grid {
                    nx = (nx / SNAP_GRID).round() * SNAP_GRID;
                    ny = (ny / SNAP_GRID).round() * SNAP_GRID;
                }

                // Move all selected nodes together if multi-selected
                if editor.selected_nodes.len() > 1 && editor.selected_nodes.contains(&id) {
                    // Get the delta from original position
                    let orig_pos = editor.trees.get(editor.active_tree)
                        .and_then(|t| t.nodes.get(&id))
                        .map(|n| n.position)
                        .unwrap_or((0.0, 0.0));
                    let delta_x = nx - orig_pos.0;
                    let delta_y = ny - orig_pos.1;

                    let selected: Vec<BtNodeId> = editor.selected_nodes.iter().copied().collect();
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        for sel_id in selected {
                            if let Some(node) = tree.nodes.get_mut(&sel_id) {
                                node.position.0 += delta_x;
                                node.position.1 += delta_y;
                            }
                        }
                    }
                } else {
                    if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                        if let Some(node) = tree.nodes.get_mut(&id) {
                            node.position = (nx, ny);
                        }
                    }
                }
            } else {
                editor.drag_state = DragState::None;

                // Check if dropped on another node for connection
                if let Some(hover_id) = editor.hovered_node {
                    if hover_id != id {
                        let shift = ui.ctx().input(|i| i.modifiers.shift);
                        if shift {
                            // Connect: make id a child of hover_id
                            if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                                if tree.nodes.get(&hover_id).map(|n| n.node_type.can_have_children()).unwrap_or(false) {
                                    tree.add_child(hover_id, id);
                                }
                            }
                        }
                    }
                }
            }
        },

        DragState::PanCanvas { .. } => {
            if middle_down {
                editor.canvas_offset += drag_delta;
            } else {
                editor.drag_state = DragState::None;
            }
        },

        DragState::SelectionBox { start } => {
            if primary_down {
                editor.selection_box = Some(SelectionBox { start, end: ptr });
            } else {
                // Finalize selection
                if let Some(sel_box) = &editor.selection_box {
                    let screen_box = sel_box.rect();
                    let ctrl = ui.ctx().input(|i| i.modifiers.ctrl);
                    if !ctrl {
                        editor.selected_nodes.clear();
                    }
                    if let Some(tree) = editor.trees.get(editor.active_tree) {
                        for (&nid, node) in &tree.nodes {
                            let nr = editor.node_screen_rect(node);
                            if screen_box.intersects(nr) {
                                editor.selected_nodes.insert(nid);
                            }
                        }
                    }
                    editor.selected_node = editor.selected_nodes.iter().next().copied();
                }
                editor.selection_box = None;
                editor.drag_state = DragState::None;
            }
        },

        DragState::ConnectingFrom { from_id, .. } => {
            if primary_down {
                editor.drag_state = DragState::ConnectingFrom { from_id, current_pos: ptr };
            } else {
                if let Some(target_id) = editor.hovered_node {
                    if target_id != from_id {
                        if let Some(tree) = editor.trees.get_mut(editor.active_tree) {
                            tree.add_child(from_id, target_id);
                        }
                    }
                }
                editor.drag_state = DragState::None;
            }
        },
    }

    // Also pan with space + drag
    let space_held = ui.ctx().input(|i| i.key_down(egui::Key::Space));
    if space_held && primary_down && drag_delta.length() > 0.0 {
        editor.canvas_offset += drag_delta;
    }

    // Keyboard shortcuts
    let kb = ui.ctx().input(|i| {
        let del = i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace);
        let copy = i.modifiers.ctrl && i.key_pressed(egui::Key::C);
        let paste = i.modifiers.ctrl && i.key_pressed(egui::Key::V);
        let select_all = i.modifiers.ctrl && i.key_pressed(egui::Key::A);
        let dup = i.modifiers.ctrl && i.key_pressed(egui::Key::D);
        let fit = i.key_pressed(egui::Key::F);
        (del, copy, paste, select_all, dup, fit)
    });

    if canvas_response.hovered() {
        if kb.0 { // Delete
            if !editor.selected_nodes.is_empty() {
                editor.delete_selected();
            }
        }
        if kb.1 { // Copy
            if let Some(id) = editor.selected_node {
                editor.copy_subtree(id);
            }
        }
        if kb.2 { // Paste
            let paste_pos = editor.screen_to_canvas(ptr);
            editor.paste_subtree(paste_pos);
        }
        if kb.3 { // Select all
            editor.select_all();
        }
        if kb.4 { // Duplicate
            if let Some(id) = editor.selected_node {
                if let Some(new_id) = editor.duplicate_subtree(id) {
                    editor.selected_node = Some(new_id);
                }
            }
        }
        if kb.5 { // Fit
            editor.do_fit_to_view(canvas_rect);
        }
    }
}

// ============================================================
// CANVAS DRAW
// ============================================================

fn draw_canvas(
    ui: &mut egui::Ui,
    editor: &mut BehaviorTreeEditor,
    canvas_rect: Rect,
) {
    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, egui::Rounding::ZERO, COLOR_CANVAS_BG);

    if editor.show_grid {
        draw_grid(&painter, canvas_rect, editor.canvas_offset, editor.canvas_zoom);
    }

    let tree = match editor.trees.get(editor.active_tree) {
        Some(t) => t,
        None => return,
    };

    // Draw connections
    {
        let node_ids: Vec<BtNodeId> = tree.nodes.keys().copied().collect();
        for parent_id in &node_ids {
            let (parent_pos, children, parent_h) = {
                let Some(parent) = tree.nodes.get(parent_id) else { continue; };
                (parent.position, parent.children.clone(), parent.compute_height())
            };

            let from_canvas = Pos2::new(
                parent_pos.0 + NODE_WIDTH / 2.0,
                parent_pos.1 + parent_h,
            );
            let from_screen = editor.canvas_to_screen(from_canvas);

            for child_id in &children {
                let Some(child) = tree.nodes.get(child_id) else { continue; };
                let to_canvas = Pos2::new(
                    child.position.0 + NODE_WIDTH / 2.0,
                    child.position.1,
                );
                let to_screen = editor.canvas_to_screen(to_canvas);

                let is_active = editor.simulation.running &&
                    (child.status == BtNodeStatus::Running || child.status == BtNodeStatus::Success);

                let conn_color = if is_active {
                    COLOR_CONNECTION_ACTIVE
                } else {
                    let hov = editor.hovered_node == Some(*parent_id) || editor.hovered_node == Some(*child_id);
                    if hov { Color32::from_rgb(180, 180, 200) } else { COLOR_CONNECTION }
                };

                let thickness = if editor.selected_nodes.contains(parent_id) || editor.selected_nodes.contains(child_id) {
                    CONNECTION_THICKNESS_HOVER
                } else {
                    CONNECTION_THICKNESS
                };

                draw_bezier_connection(
                    &painter, from_screen, to_screen, conn_color,
                    thickness * editor.canvas_zoom,
                    editor.canvas_zoom,
                );
            }
        }
    }

    // Draw in-progress connection
    if let DragState::ConnectingFrom { from_id, current_pos } = &editor.drag_state {
        let from_pos = tree.nodes.get(from_id).map(|n| {
            let h = n.compute_height();
            editor.canvas_to_screen(Pos2::new(n.position.0 + NODE_WIDTH / 2.0, n.position.1 + h))
        });
        if let Some(from_screen) = from_pos {
            draw_bezier_connection(
                &painter, from_screen, *current_pos,
                COLOR_CONNECTION_ACTIVE, 2.0 * editor.canvas_zoom, editor.canvas_zoom,
            );
        }
    }

    // Draw nodes
    let frame = editor.frame_counter;
    let node_data: Vec<(BtNodeId, BtNodeData)> = tree.nodes.iter()
        .map(|(&id, n)| (id, n.clone()))
        .collect();

    // Sort: root first, then by depth, then selected on top
    let mut sorted_nodes = node_data;
    sorted_nodes.sort_by_key(|(id, _)| {
        let is_selected = editor.selected_nodes.contains(id) as u8;
        let is_hovered = (editor.hovered_node == Some(*id)) as u8;
        (is_selected, is_hovered)
    });

    for (id, node) in &sorted_nodes {
        let screen_rect = editor.node_screen_rect(node);
        if !canvas_rect.intersects(screen_rect) { continue; } // cull off-screen

        draw_node(
            &painter,
            node,
            screen_rect,
            editor.selected_nodes.contains(id),
            editor.hovered_node == Some(*id),
            editor.breakpoints.contains(id),
            editor.canvas_zoom,
            frame,
        );

        // Root indicator
        if tree.root == Some(*id) {
            painter.text(
                Pos2::new(screen_rect.min.x, screen_rect.min.y - 4.0),
                Align2::LEFT_BOTTOM,
                "ROOT",
                FontId::proportional(9.0 * editor.canvas_zoom),
                Color32::from_rgb(220, 180, 60),
            );
        }
    }

    // Draw selection box
    if let Some(sel_box) = &editor.selection_box {
        let rect = sel_box.rect();
        painter.rect_filled(rect, egui::Rounding::ZERO, COLOR_SELECTION_BOX);
        painter.rect_stroke(rect, egui::Rounding::ZERO, Stroke::new(1.0, COLOR_SELECTION_BORDER), egui::StrokeKind::Outside);
    }

    // Draw minimap
    if editor.show_minimap && !tree.nodes.is_empty() {
        let mm_size = Vec2::new(160.0, 100.0);
        let mm_rect = Rect::from_min_size(
            Pos2::new(canvas_rect.max.x - mm_size.x - 8.0, canvas_rect.max.y - mm_size.y - 8.0),
            mm_size,
        );
        draw_minimap(
            &painter, mm_rect, tree,
            editor.canvas_offset, editor.canvas_zoom, canvas_rect,
            &editor.selected_nodes,
        );
    }

    // Draw status bar at bottom of canvas
    {
        let status_rect = Rect::from_min_size(
            Pos2::new(canvas_rect.min.x, canvas_rect.max.y - 20.0),
            Vec2::new(canvas_rect.width(), 20.0),
        );
        painter.rect_filled(status_rect, egui::Rounding::ZERO, Color32::from_rgba_premultiplied(20, 20, 28, 200));
        let tree = editor.trees.get(editor.active_tree);
        let node_count = tree.map(|t| t.nodes.len()).unwrap_or(0);
        let sel_count = editor.selected_nodes.len();
        let status = format!(
            "Nodes: {}  |  Selected: {}  |  Zoom: {:.0}%  |  Tick: {}  |  {}",
            node_count, sel_count,
            editor.canvas_zoom * 100.0,
            editor.simulation.tick,
            if editor.simulation.running { "RUNNING" } else { "PAUSED" },
        );
        painter.text(
            Pos2::new(canvas_rect.min.x + 8.0, canvas_rect.max.y - 10.0),
            Align2::LEFT_CENTER,
            &status,
            FontId::proportional(10.0),
            Color32::from_rgb(150, 150, 170),
        );
    }
}

// ============================================================
// SIMULATION UPDATE
// ============================================================

fn update_simulation(editor: &mut BehaviorTreeEditor, dt: f32) {
    if !editor.simulation.running { return; }

    editor.simulation.accumulated_time += dt;
    let interval = editor.simulation.tick_interval();

    while editor.simulation.accumulated_time >= interval {
        editor.simulation.accumulated_time -= interval;
        editor.simulation_step();

        // Stop after hit breakpoint (simulation_step already pauses it)
        if !editor.simulation.running { break; }
    }
}

// ============================================================
// TREE INFO PANEL
// ============================================================

fn show_tree_info_panel(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor) {
    let tree = match editor.trees.get_mut(editor.active_tree) {
        Some(t) => t,
        None => return,
    };

    ui.collapsing("Tree Info", |ui| {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut tree.name);
        });
        ui.horizontal(|ui| {
            ui.label("Description:");
        });
        ui.text_edit_multiline(&mut tree.description);
        ui.horizontal(|ui| {
            ui.label("Author:");
            ui.text_edit_singleline(&mut tree.author);
        });
        ui.label(format!("Nodes: {}", tree.nodes.len()));
        ui.label(format!("Version: {}", tree.version));
        if let Some(root_id) = tree.root {
            ui.label(format!("Root ID: {}", root_id));
        }
        let errors = tree.validate();
        if errors.is_empty() {
            ui.colored_label(COLOR_STATUS_SUCCESS, "✓ Valid");
        } else {
            ui.colored_label(COLOR_STATUS_FAILURE, format!("✗ {} error(s)", errors.len()));
        }
    });
}

// ============================================================
// MAIN PUBLIC INTERFACE
// ============================================================

pub fn new() -> BehaviorTreeEditor {
    BehaviorTreeEditor::new()
}

pub fn show(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor) {
    editor.frame_counter += 1;

    // Update simulation with real delta time
    let dt = ui.ctx().input(|i| i.unstable_dt).min(0.1);
    update_simulation(editor, dt);

    ui.ctx().request_repaint_after(std::time::Duration::from_millis(if editor.simulation.running { 16 } else { 100 }));

    let available = ui.available_rect_before_wrap();

    // Layout:
    // [Palette] [Canvas] [Properties]
    //           [Blackboard + Simulation]

    let palette_w = if editor.show_node_palette { NODE_PALETTE_WIDTH + 8.0 } else { 0.0 };
    let props_w = if editor.show_properties { PROPERTIES_PANEL_WIDTH + 8.0 } else { 0.0 };
    let bottom_h = {
        let mut h = 0.0;
        if editor.show_blackboard { h += BLACKBOARD_PANEL_HEIGHT + 8.0; }
        if editor.show_simulation { h += SIMULATION_PANEL_HEIGHT + 8.0; }
        h
    };
    let canvas_w = (available.width() - palette_w - props_w).max(200.0);
    let toolbar_h = 28.0;
    let tab_bar_h = 28.0;
    let canvas_h = (available.height() - toolbar_h - tab_bar_h - bottom_h).max(200.0);

    // Tree tab bar
    let tab_rect = Rect::from_min_size(available.min, Vec2::new(available.width(), tab_bar_h));
    let tab_ui_resp = ui.allocate_ui_at_rect(tab_rect, |ui| {
        ui.painter().rect_filled(tab_rect, egui::Rounding::ZERO, COLOR_PANEL_HEADER);
        show_tree_tab_bar(ui, editor);
    });

    // Toolbar
    let toolbar_y = available.min.y + tab_bar_h;
    let toolbar_rect = Rect::from_min_size(
        Pos2::new(available.min.x, toolbar_y),
        Vec2::new(available.width(), toolbar_h),
    );
    let canvas_rect_for_toolbar = Rect::from_min_size(
        Pos2::new(available.min.x + palette_w, toolbar_y + toolbar_h),
        Vec2::new(canvas_w, canvas_h),
    );
    ui.allocate_ui_at_rect(toolbar_rect, |ui| {
        ui.painter().rect_filled(toolbar_rect, egui::Rounding::ZERO, COLOR_PANEL_BG);
        show_toolbar(ui, editor, canvas_rect_for_toolbar);
    });

    let content_y = toolbar_y + toolbar_h;

    // Palette panel
    if editor.show_node_palette {
        let palette_rect = Rect::from_min_size(
            Pos2::new(available.min.x, content_y),
            Vec2::new(palette_w, canvas_h),
        );
        ui.allocate_ui_at_rect(palette_rect, |ui| {
            ui.painter().rect_filled(palette_rect, egui::Rounding::ZERO, COLOR_PANEL_BG);
            show_node_palette_panel(ui, editor, canvas_rect_for_toolbar);
        });
    }

    // Properties panel
    if editor.show_properties {
        let props_rect = Rect::from_min_size(
            Pos2::new(available.min.x + palette_w + canvas_w, content_y),
            Vec2::new(props_w, canvas_h),
        );
        ui.allocate_ui_at_rect(props_rect, |ui| {
            ui.painter().rect_filled(props_rect, egui::Rounding::ZERO, COLOR_PANEL_BG);
            ui.vertical(|ui| {
                let header_r = ui.allocate_space(Vec2::new(props_w, 24.0)).1;
                ui.painter().rect_filled(header_r, egui::Rounding::ZERO, COLOR_PANEL_HEADER);
                ui.painter().text(header_r.center(), Align2::CENTER_CENTER,
                    "Properties", FontId::proportional(13.0), Color32::WHITE);
                show_tree_info_panel(ui, editor);
                ui.separator();
                show_properties_panel(ui, editor);
            });
        });
    }

    // Canvas
    let canvas_rect = Rect::from_min_size(
        Pos2::new(available.min.x + palette_w, content_y),
        Vec2::new(canvas_w, canvas_h),
    );
    let canvas_resp = ui.allocate_rect(canvas_rect, egui::Sense::click_and_drag());
    draw_canvas(ui, editor, canvas_rect);
    handle_canvas_interaction(ui, editor, &canvas_resp, canvas_rect);

    // Bottom panels
    let bottom_y = content_y + canvas_h;
    let bottom_x = available.min.x;
    let mut cur_bottom_y = bottom_y;

    if editor.show_blackboard {
        let bb_rect = Rect::from_min_size(
            Pos2::new(bottom_x, cur_bottom_y),
            Vec2::new(available.width(), BLACKBOARD_PANEL_HEIGHT + 8.0),
        );
        ui.allocate_ui_at_rect(bb_rect, |ui| {
            ui.painter().rect_filled(bb_rect, egui::Rounding::ZERO, COLOR_PANEL_BG);
            ui.add_space(4.0);
            show_blackboard_panel(ui, editor);
        });
        cur_bottom_y += BLACKBOARD_PANEL_HEIGHT + 8.0;
    }

    if editor.show_simulation {
        let sim_rect = Rect::from_min_size(
            Pos2::new(bottom_x, cur_bottom_y),
            Vec2::new(available.width(), SIMULATION_PANEL_HEIGHT + 8.0),
        );
        ui.allocate_ui_at_rect(sim_rect, |ui| {
            ui.painter().rect_filled(sim_rect, egui::Rounding::ZERO, COLOR_PANEL_BG);
            ui.add_space(4.0);
            show_simulation_panel(ui, editor);
        });
    }

    // Context menu
    show_context_menu(ui, editor, canvas_rect);

    // Modal windows
    show_validation_window(ui.ctx(), editor);
    show_export_window(ui.ctx(), editor);
    show_import_window(ui.ctx(), editor);
}

pub fn show_panel(ctx: &egui::Context, editor: &mut BehaviorTreeEditor, open: &mut bool) {
    egui::Window::new("Behavior Tree Editor")
        .open(open)
        .resizable(true)
        .default_size([1200.0, 800.0])
        .min_size([600.0, 400.0])
        .show(ctx, |ui| {
            show(ui, editor);
        });
}

// ============================================================
// ADDITIONAL UTILITY FUNCTIONS
// ============================================================

/// Compute a stable hash for a BtNodeId chain, used for animation offsets etc.
fn node_hash(id: BtNodeId, seed: u32) -> f32 {
    let h = id.wrapping_mul(2654435761).wrapping_add(seed.wrapping_mul(40503));
    (h & 0xFFFF) as f32 / 65535.0
}

/// Find nodes by label search
pub fn search_nodes<'a>(tree: &'a BehaviorTree, query: &str) -> Vec<&'a BtNodeData> {
    let q = query.to_lowercase();
    tree.nodes.values()
        .filter(|n| {
            n.label.to_lowercase().contains(&q)
            || n.node_type.type_name().to_lowercase().contains(&q)
            || n.comment.to_lowercase().contains(&q)
            || n.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .collect()
}

/// Get all leaf nodes (no children)
pub fn get_leaf_nodes(tree: &BehaviorTree) -> Vec<BtNodeId> {
    tree.nodes.iter()
        .filter(|(_, n)| n.children.is_empty())
        .map(|(&id, _)| id)
        .collect()
}

/// Get maximum depth of the tree
pub fn get_tree_depth(tree: &BehaviorTree) -> u32 {
    tree.nodes.keys().map(|&id| tree.node_depth(id)).max().unwrap_or(0)
}

/// Count nodes by status
pub fn count_by_status(tree: &BehaviorTree) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for node in tree.nodes.values() {
        *counts.entry(node.status.label().to_string()).or_insert(0) += 1;
    }
    counts
}

/// Count nodes by category
pub fn count_by_category(tree: &BehaviorTree) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for node in tree.nodes.values() {
        *counts.entry(node.node_type.category().label().to_string()).or_insert(0) += 1;
    }
    counts
}

/// Collect all blackboard keys referenced by the tree
pub fn collect_blackboard_keys(tree: &BehaviorTree) -> HashMap<String, Vec<(BtNodeId, bool)>> {
    let mut keys: HashMap<String, Vec<(BtNodeId, bool)>> = HashMap::new();

    for (&id, node) in &tree.nodes {
        match &node.node_type {
            BtNode::SetBlackboard { key, .. } => {
                keys.entry(key.clone()).or_default().push((id, true));
            },
            BtNode::CheckBlackboard { key, .. } => {
                keys.entry(key.clone()).or_default().push((id, false));
            },
            BtNode::Condition(ConditionKind::HasBlackboardKey { key }) => {
                keys.entry(key.clone()).or_default().push((id, false));
            },
            BtNode::Cooldown { shared_key: Some(key), .. } => {
                keys.entry(key.clone()).or_default().push((id, true));
            },
            _ => {}
        }
    }
    keys
}

/// Topological sort of nodes (root first)
pub fn topological_sort(tree: &BehaviorTree) -> Vec<BtNodeId> {
    let Some(root_id) = tree.root else { return Vec::new(); };
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = vec![root_id];

    while let Some(id) = stack.pop() {
        if visited.contains(&id) { continue; }
        visited.insert(id);
        result.push(id);
        if let Some(node) = tree.nodes.get(&id) {
            for &child in node.children.iter().rev() {
                if !visited.contains(&child) {
                    stack.push(child);
                }
            }
        }
    }
    result
}

/// Check if node_b is a descendant of node_a
pub fn is_descendant(tree: &BehaviorTree, ancestor: BtNodeId, descendant: BtNodeId) -> bool {
    let subtree = tree.collect_subtree(ancestor);
    subtree.contains(&descendant)
}

/// Compute bounding box for a subtree
pub fn subtree_bounds(tree: &BehaviorTree, root_id: BtNodeId) -> Option<Rect> {
    let ids = tree.collect_subtree(root_id);
    if ids.is_empty() { return None; }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for id in &ids {
        if let Some(node) = tree.nodes.get(id) {
            let h = node.compute_height();
            min_x = min_x.min(node.position.0);
            min_y = min_y.min(node.position.1);
            max_x = max_x.max(node.position.0 + NODE_WIDTH);
            max_y = max_y.max(node.position.1 + h);
        }
    }
    Some(Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y)))
}

/// Generate a textual summary of the tree structure
pub fn tree_summary(tree: &BehaviorTree) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Tree: {} ({} nodes)", tree.name, tree.nodes.len()));

    fn visit(tree: &BehaviorTree, id: BtNodeId, depth: usize, lines: &mut Vec<String>) {
        let indent = "  ".repeat(depth);
        if let Some(node) = tree.nodes.get(&id) {
            lines.push(format!(
                "{}[{}] {} ({}) - {}",
                indent, id, node.label, node.node_type.type_name(), node.status.label()
            ));
            for &child in &node.children {
                visit(tree, child, depth + 1, lines);
            }
        }
    }

    if let Some(root_id) = tree.root {
        visit(tree, root_id, 0, &mut lines);
    }

    lines.join("\n")
}

/// Build a flat node list for a list-based view
#[derive(Debug, Clone)]
pub struct FlatNodeEntry {
    pub id: BtNodeId,
    pub depth: u32,
    pub label: String,
    pub type_name: String,
    pub status: BtNodeStatus,
    pub enabled: bool,
    pub has_children: bool,
    pub is_root: bool,
}

pub fn build_flat_node_list(tree: &BehaviorTree) -> Vec<FlatNodeEntry> {
    let mut result = Vec::new();
    let root_id = match tree.root {
        Some(id) => id,
        None => return result,
    };

    fn visit(tree: &BehaviorTree, id: BtNodeId, depth: u32, root_id: BtNodeId, result: &mut Vec<FlatNodeEntry>) {
        let Some(node) = tree.nodes.get(&id) else { return; };
        result.push(FlatNodeEntry {
            id,
            depth,
            label: node.label.clone(),
            type_name: node.node_type.type_name().to_string(),
            status: node.status.clone(),
            enabled: node.enabled,
            has_children: !node.children.is_empty(),
            is_root: id == root_id,
        });
        for &child in &node.children {
            visit(tree, child, depth + 1, root_id, result);
        }
    }

    visit(tree, root_id, 0, root_id, &mut result);
    result
}

/// Show a list view of the tree (useful for hierarchical browsing)
pub fn show_tree_list_view(ui: &mut egui::Ui, tree: &BehaviorTree, selected: &mut Option<BtNodeId>) {
    let entries = build_flat_node_list(tree);

    egui::ScrollArea::vertical().id_salt("tree_list").show(ui, |ui| {
        for entry in &entries {
            let indent = entry.depth as f32 * 14.0;
            ui.horizontal(|ui| {
                ui.add_space(indent);
                let is_sel = *selected == Some(entry.id);
                let color = match entry.status {
                    BtNodeStatus::Success => COLOR_STATUS_SUCCESS,
                    BtNodeStatus::Failure => COLOR_STATUS_FAILURE,
                    BtNodeStatus::Running => COLOR_STATUS_RUNNING,
                    BtNodeStatus::Idle => COLOR_NODE_SUBTEXT,
                };
                let label = if entry.is_root {
                    format!("★ {} [{}]", entry.label, entry.type_name)
                } else {
                    format!("{} [{}]", entry.label, entry.type_name)
                };
                let resp = ui.selectable_label(is_sel, &label);
                if resp.clicked() {
                    *selected = Some(entry.id);
                }
                ui.colored_label(color, entry.status.label());
            });
        }
    });
}

/// Snap all nodes to the grid
pub fn snap_all_to_grid(tree: &mut BehaviorTree) {
    for node in tree.nodes.values_mut() {
        node.position.0 = (node.position.0 / SNAP_GRID).round() * SNAP_GRID;
        node.position.1 = (node.position.1 / SNAP_GRID).round() * SNAP_GRID;
    }
}

/// Clone a tree with new IDs
pub fn clone_tree(tree: &BehaviorTree, id_counter: &mut BtNodeId) -> BehaviorTree {
    let mut new_tree = tree.clone();
    new_tree.name = format!("{} (copy)", tree.name);

    let mut id_map: HashMap<BtNodeId, BtNodeId> = HashMap::new();
    let old_ids: Vec<BtNodeId> = tree.nodes.keys().copied().collect();

    for &old_id in &old_ids {
        let new_id = *id_counter;
        *id_counter += 1;
        id_map.insert(old_id, new_id);
    }

    new_tree.nodes.clear();
    for (&old_id, node) in &tree.nodes {
        let new_id = *id_map.get(&old_id).unwrap();
        let mut new_node = node.clone();
        new_node.id = new_id;
        new_node.children = node.children.iter()
            .filter_map(|&c| id_map.get(&c).copied())
            .collect();
        new_node.parent = node.parent.and_then(|p| id_map.get(&p).copied());
        new_tree.nodes.insert(new_id, new_node);
    }

    new_tree.root = tree.root.and_then(|r| id_map.get(&r).copied());
    new_tree
}

/// Statistics struct for a tree
#[derive(Debug, Clone)]
pub struct TreeStats {
    pub total_nodes: usize,
    pub max_depth: u32,
    pub leaf_count: usize,
    pub composite_count: usize,
    pub decorator_count: usize,
    pub action_count: usize,
    pub condition_count: usize,
    pub disabled_count: usize,
    pub breakpoint_count: usize,
    pub tagged_count: usize,
}

pub fn compute_tree_stats(tree: &BehaviorTree, breakpoints: &HashSet<BtNodeId>) -> TreeStats {
    let mut stats = TreeStats {
        total_nodes: tree.nodes.len(),
        max_depth: get_tree_depth(tree),
        leaf_count: 0,
        composite_count: 0,
        decorator_count: 0,
        action_count: 0,
        condition_count: 0,
        disabled_count: 0,
        breakpoint_count: breakpoints.len(),
        tagged_count: 0,
    };

    for node in tree.nodes.values() {
        match node.node_type.category() {
            NodeCategory::Composite => stats.composite_count += 1,
            NodeCategory::Decorator => stats.decorator_count += 1,
            NodeCategory::Action => stats.action_count += 1,
            NodeCategory::Condition => stats.condition_count += 1,
            NodeCategory::Leaf => stats.leaf_count += 1,
        }
        if node.children.is_empty() {
            stats.leaf_count += if !matches!(node.node_type.category(), NodeCategory::Leaf) { 1 } else { 0 };
        }
        if !node.enabled { stats.disabled_count += 1; }
        if !node.tags.is_empty() { stats.tagged_count += 1; }
    }
    stats
}

pub fn show_stats_panel(ui: &mut egui::Ui, editor: &BehaviorTreeEditor) {
    let Some(tree) = editor.trees.get(editor.active_tree) else { return; };
    let stats = compute_tree_stats(tree, &editor.breakpoints);

    ui.collapsing("Tree Statistics", |ui| {
        egui::Grid::new("stats_grid").num_columns(2).striped(true).show(ui, |ui| {
            ui.label("Total nodes:");     ui.label(stats.total_nodes.to_string()); ui.end_row();
            ui.label("Max depth:");      ui.label(stats.max_depth.to_string()); ui.end_row();
            ui.label("Composites:");     ui.label(stats.composite_count.to_string()); ui.end_row();
            ui.label("Decorators:");     ui.label(stats.decorator_count.to_string()); ui.end_row();
            ui.label("Actions:");        ui.label(stats.action_count.to_string()); ui.end_row();
            ui.label("Conditions:");     ui.label(stats.condition_count.to_string()); ui.end_row();
            ui.label("Leaves:");         ui.label(stats.leaf_count.to_string()); ui.end_row();
            ui.label("Disabled:");       ui.colored_label(Color32::from_rgb(180, 100, 100), stats.disabled_count.to_string()); ui.end_row();
            ui.label("Breakpoints:");    ui.colored_label(COLOR_BREAKPOINT, stats.breakpoint_count.to_string()); ui.end_row();
            ui.label("Tagged:");         ui.label(stats.tagged_count.to_string()); ui.end_row();
        });
    });
}

// ============================================================
// SEARCH PANEL
// ============================================================

pub fn show_search_panel(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor) {
    ui.horizontal(|ui| {
        ui.label("🔍 Search:");
        ui.text_edit_singleline(&mut editor.search_filter);
        if ui.small_button("✗").clicked() {
            editor.search_filter.clear();
        }
    });

    if editor.search_filter.is_empty() { return; }

    let Some(tree) = editor.trees.get(editor.active_tree) else { return; };
    let results = search_nodes(tree, &editor.search_filter);

    if results.is_empty() {
        ui.colored_label(Color32::from_rgb(150, 100, 100), "No results.");
        return;
    }

    ui.label(format!("{} result(s):", results.len()));
    egui::ScrollArea::vertical().id_salt("search_results").max_height(160.0).show(ui, |ui| {
        let mut to_select: Option<BtNodeId> = None;
        for node in &results {
            let is_sel = editor.selected_node == Some(node.id);
            let cat_color = node.node_type.category().color();
            ui.horizontal(|ui| {
                ui.colored_label(cat_color, node.node_type.icon());
                if ui.selectable_label(is_sel, &node.label).clicked() {
                    to_select = Some(node.id);
                }
                ui.colored_label(Color32::from_rgb(120, 120, 140), node.node_type.type_name());
            });
        }
        if let Some(id) = to_select {
            editor.selected_node = Some(id);
            editor.selected_nodes.clear();
            editor.selected_nodes.insert(id);
            // Pan to the node
            if let Some(tree) = editor.trees.get(editor.active_tree) {
                if let Some(node) = tree.nodes.get(&id) {
                    let target_x = node.position.0 + NODE_WIDTH / 2.0;
                    let target_y = node.position.1 + node.compute_height() / 2.0;
                    // We'd need the canvas rect here; just adjust offset
                    editor.canvas_offset = Vec2::new(
                        400.0 - target_x * editor.canvas_zoom,
                        300.0 - target_y * editor.canvas_zoom,
                    );
                }
            }
        }
    });
}

// ============================================================
// TAGS MANAGER
// ============================================================

pub fn collect_all_tags(tree: &BehaviorTree) -> Vec<String> {
    let mut tag_set: HashSet<String> = HashSet::new();
    for node in tree.nodes.values() {
        for tag in &node.tags {
            tag_set.insert(tag.clone());
        }
    }
    let mut tags: Vec<String> = tag_set.into_iter().collect();
    tags.sort();
    tags
}

pub fn get_nodes_with_tag(tree: &BehaviorTree, tag: &str) -> Vec<BtNodeId> {
    tree.nodes.iter()
        .filter(|(_, n)| n.tags.contains(&tag.to_string()))
        .map(|(&id, _)| id)
        .collect()
}

// ============================================================
// UNDO / REDO STUBS (Structural — would need full impl)
// ============================================================

#[derive(Debug, Clone)]
pub enum UndoAction {
    AddNode { node: BtNodeData },
    RemoveNode { node: BtNodeData, parent_id: Option<BtNodeId> },
    MoveNode { id: BtNodeId, old_pos: (f32, f32), new_pos: (f32, f32) },
    ChangeNodeType { id: BtNodeId, old_type: BtNode, new_type: BtNode },
    AddConnection { parent: BtNodeId, child: BtNodeId },
    RemoveConnection { parent: BtNodeId, child: BtNodeId },
    RenameNode { id: BtNodeId, old_label: String, new_label: String },
    ToggleEnabled { id: BtNodeId, was_enabled: bool },
    ReorderChildren { parent_id: BtNodeId, old_order: Vec<BtNodeId>, new_order: Vec<BtNodeId> },
}

pub struct UndoHistory {
    pub undo_stack: VecDeque<UndoAction>,
    pub redo_stack: VecDeque<UndoAction>,
    pub max_size: usize,
}

impl UndoHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_size: 100,
        }
    }

    pub fn push(&mut self, action: UndoAction) {
        self.undo_stack.push_back(action);
        if self.undo_stack.len() > self.max_size {
            self.undo_stack.pop_front();
        }
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    pub fn pop_undo(&mut self) -> Option<UndoAction> {
        self.undo_stack.pop_back()
    }

    pub fn pop_redo(&mut self) -> Option<UndoAction> {
        self.redo_stack.pop_back()
    }

    pub fn push_redo(&mut self, action: UndoAction) {
        self.redo_stack.push_back(action);
    }
}

pub fn apply_undo(tree: &mut BehaviorTree, action: &UndoAction) {
    match action {
        UndoAction::AddNode { node } => {
            tree.remove_node(node.id);
        },
        UndoAction::RemoveNode { node, parent_id } => {
            tree.nodes.insert(node.id, node.clone());
            if let Some(pid) = parent_id {
                tree.add_child(*pid, node.id);
            }
        },
        UndoAction::MoveNode { id, old_pos, .. } => {
            if let Some(n) = tree.nodes.get_mut(id) {
                n.position = *old_pos;
            }
        },
        UndoAction::ChangeNodeType { id, old_type, .. } => {
            if let Some(n) = tree.nodes.get_mut(id) {
                n.node_type = old_type.clone();
            }
        },
        UndoAction::AddConnection { parent, child } => {
            tree.remove_child(*parent, *child);
        },
        UndoAction::RemoveConnection { parent, child } => {
            tree.add_child(*parent, *child);
        },
        UndoAction::RenameNode { id, old_label, .. } => {
            if let Some(n) = tree.nodes.get_mut(id) {
                n.label = old_label.clone();
            }
        },
        UndoAction::ToggleEnabled { id, was_enabled } => {
            if let Some(n) = tree.nodes.get_mut(id) {
                n.enabled = *was_enabled;
            }
        },
        UndoAction::ReorderChildren { parent_id, old_order, .. } => {
            if let Some(parent) = tree.nodes.get_mut(parent_id) {
                parent.children = old_order.clone();
            }
        },
    }
}

pub fn apply_redo(tree: &mut BehaviorTree, action: &UndoAction) {
    match action {
        UndoAction::AddNode { node } => {
            tree.nodes.insert(node.id, node.clone());
        },
        UndoAction::RemoveNode { node, .. } => {
            tree.remove_node(node.id);
        },
        UndoAction::MoveNode { id, new_pos, .. } => {
            if let Some(n) = tree.nodes.get_mut(id) {
                n.position = *new_pos;
            }
        },
        UndoAction::ChangeNodeType { id, new_type, .. } => {
            if let Some(n) = tree.nodes.get_mut(id) {
                n.node_type = new_type.clone();
            }
        },
        UndoAction::AddConnection { parent, child } => {
            tree.add_child(*parent, *child);
        },
        UndoAction::RemoveConnection { parent, child } => {
            tree.remove_child(*parent, *child);
        },
        UndoAction::RenameNode { id, new_label, .. } => {
            if let Some(n) = tree.nodes.get_mut(id) {
                n.label = new_label.clone();
            }
        },
        UndoAction::ToggleEnabled { id, was_enabled } => {
            if let Some(n) = tree.nodes.get_mut(id) {
                n.enabled = !was_enabled;
            }
        },
        UndoAction::ReorderChildren { parent_id, new_order, .. } => {
            if let Some(parent) = tree.nodes.get_mut(parent_id) {
                parent.children = new_order.clone();
            }
        },
    }
}

// ============================================================
// PREFAB TREES — EXAMPLE TEMPLATES
// ============================================================

pub fn create_guard_patrol_tree(id_counter: &mut BtNodeId) -> BehaviorTree {
    let mut tree = BehaviorTree::new("Guard Patrol");
    tree.description = "A guard that patrols, investigates sounds, and attacks enemies.".to_string();

    let root_id = *id_counter; *id_counter += 1;
    let mut root = BtNodeData::new(root_id, BtNode::Selector, (400.0, 40.0));
    root.label = "Guard Root".into();

    let combat_seq_id = *id_counter; *id_counter += 1;
    let mut combat_seq = BtNodeData::new(combat_seq_id, BtNode::Sequence, (200.0, 160.0));
    combat_seq.label = "Combat Sequence".into();

    let can_see_id = *id_counter; *id_counter += 1;
    let mut can_see = BtNodeData::new(can_see_id, BtNode::Condition(
        ConditionKind::CanSeePlayer { fov_degrees: 90.0, max_distance: 20.0 }
    ), (100.0, 280.0));
    can_see.label = "Can See Player".into();

    let alert_id = *id_counter; *id_counter += 1;
    let mut alert = BtNodeData::new(alert_id, BtNode::Action(
        ActionKind::AlertAllies { radius: 25.0, alert_type: "enemy_spotted".into() }
    ), (200.0, 280.0));
    alert.label = "Alert Allies".into();

    let attack_id = *id_counter; *id_counter += 1;
    let mut attack = BtNodeData::new(attack_id, BtNode::Action(
        ActionKind::Attack { target_key: "player".into(), damage: 15.0, range: 2.5 }
    ), (300.0, 280.0));
    attack.label = "Attack Player".into();

    let patrol_seq_id = *id_counter; *id_counter += 1;
    let mut patrol_seq = BtNodeData::new(patrol_seq_id, BtNode::Sequence, (600.0, 160.0));
    patrol_seq.label = "Patrol Sequence".into();

    let patrol_id = *id_counter; *id_counter += 1;
    let mut patrol = BtNodeData::new(patrol_id, BtNode::Action(
        ActionKind::Patrol { waypoints: vec![(100.0, 100.0), (200.0, 100.0), (200.0, 200.0), (100.0, 200.0)], loop_patrol: true }
    ), (550.0, 280.0));
    patrol.label = "Patrol Route".into();

    let idle_id = *id_counter; *id_counter += 1;
    let mut idle = BtNodeData::new(idle_id, BtNode::Action(
        ActionKind::Idle { duration: 2.0 }
    ), (680.0, 280.0));
    idle.label = "Idle".into();

    tree.root = Some(root_id);
    tree.nodes.insert(root_id, root);
    tree.nodes.insert(combat_seq_id, combat_seq);
    tree.nodes.insert(can_see_id, can_see);
    tree.nodes.insert(alert_id, alert);
    tree.nodes.insert(attack_id, attack);
    tree.nodes.insert(patrol_seq_id, patrol_seq);
    tree.nodes.insert(patrol_id, patrol);
    tree.nodes.insert(idle_id, idle);

    tree.add_child(root_id, combat_seq_id);
    tree.add_child(root_id, patrol_seq_id);
    tree.add_child(combat_seq_id, can_see_id);
    tree.add_child(combat_seq_id, alert_id);
    tree.add_child(combat_seq_id, attack_id);
    tree.add_child(patrol_seq_id, patrol_id);
    tree.add_child(patrol_seq_id, idle_id);

    tree
}

pub fn create_archer_tree(id_counter: &mut BtNodeId) -> BehaviorTree {
    let mut tree = BehaviorTree::new("Archer AI");
    tree.description = "An archer that tries to maintain distance while attacking.".to_string();

    let root_id = *id_counter; *id_counter += 1;
    let mut root = BtNodeData::new(root_id, BtNode::Selector, (400.0, 40.0));
    root.label = "Archer Root".into();

    let flee_seq_id = *id_counter; *id_counter += 1;
    let mut flee_seq = BtNodeData::new(flee_seq_id, BtNode::Sequence, (150.0, 160.0));
    flee_seq.label = "Flee if Too Close".into();

    let too_close_id = *id_counter; *id_counter += 1;
    let mut too_close = BtNodeData::new(too_close_id, BtNode::Condition(
        ConditionKind::IsTargetInRange { target_key: "player".into(), range: 4.0 }
    ), (80.0, 280.0));
    too_close.label = "Player Too Close".into();

    let flee_id = *id_counter; *id_counter += 1;
    let mut flee = BtNodeData::new(flee_id, BtNode::Action(
        ActionKind::Flee { from_key: "player".into(), speed: 8.0, distance: 12.0 }
    ), (200.0, 280.0));
    flee.label = "Flee from Player".into();

    let attack_seq_id = *id_counter; *id_counter += 1;
    let mut attack_seq = BtNodeData::new(attack_seq_id, BtNode::Sequence, (450.0, 160.0));
    attack_seq.label = "Attack Sequence".into();

    let visible_id = *id_counter; *id_counter += 1;
    let mut visible = BtNodeData::new(visible_id, BtNode::Condition(
        ConditionKind::IsTargetVisible { target_key: "player".into(), use_los: true }
    ), (380.0, 280.0));
    visible.label = "Player Visible".into();

    let cooldown_id = *id_counter; *id_counter += 1;
    let mut cooldown = BtNodeData::new(cooldown_id, BtNode::Cooldown { duration_secs: 1.5, shared_key: Some("arrow_cd".into()) }, (500.0, 280.0));
    cooldown.label = "Arrow Cooldown".into();

    let shoot_id = *id_counter; *id_counter += 1;
    let mut shoot = BtNodeData::new(shoot_id, BtNode::Action(
        ActionKind::UseAbility { ability_name: "shoot_arrow".into(), target_key: "player".into() }
    ), (500.0, 400.0));
    shoot.label = "Shoot Arrow".into();

    let idle_id = *id_counter; *id_counter += 1;
    let mut idle = BtNodeData::new(idle_id, BtNode::Action(
        ActionKind::Idle { duration: 1.0 }
    ), (680.0, 160.0));
    idle.label = "Idle".into();

    tree.root = Some(root_id);
    for (id, n) in [(root_id, root), (flee_seq_id, flee_seq), (too_close_id, too_close),
        (flee_id, flee), (attack_seq_id, attack_seq), (visible_id, visible),
        (cooldown_id, cooldown), (shoot_id, shoot), (idle_id, idle)] {
        tree.nodes.insert(id, n);
    }

    tree.add_child(root_id, flee_seq_id);
    tree.add_child(root_id, attack_seq_id);
    tree.add_child(root_id, idle_id);
    tree.add_child(flee_seq_id, too_close_id);
    tree.add_child(flee_seq_id, flee_id);
    tree.add_child(attack_seq_id, visible_id);
    tree.add_child(attack_seq_id, cooldown_id);
    tree.add_child(cooldown_id, shoot_id);

    tree
}

/// Show a template selector
pub fn show_templates_panel(ui: &mut egui::Ui, editor: &mut BehaviorTreeEditor) {
    ui.heading("Templates");
    ui.separator();
    ui.label("Create a new tree from a template:");

    if ui.button("Guard Patrol").clicked() {
        let tree = create_guard_patrol_tree(&mut editor.node_id_counter);
        editor.trees.push(tree);
        editor.active_tree = editor.trees.len() - 1;
    }
    ui.label("  A guard that patrols and attacks on sight.");

    if ui.button("Archer AI").clicked() {
        let tree = create_archer_tree(&mut editor.node_id_counter);
        editor.trees.push(tree);
        editor.active_tree = editor.trees.len() - 1;
    }
    ui.label("  An archer that maintains range and shoots.");

    if ui.button("Empty Tree").clicked() {
        editor.add_new_tree();
    }
    ui.label("  Start from scratch.");
}

// ============================================================
// NODE CONNECTION HIGHLIGHT
// ============================================================

pub fn get_connected_node_ids(tree: &BehaviorTree, id: BtNodeId) -> HashSet<BtNodeId> {
    let mut connected = HashSet::new();
    if let Some(node) = tree.nodes.get(&id) {
        if let Some(parent_id) = node.parent {
            connected.insert(parent_id);
        }
        for &child_id in &node.children {
            connected.insert(child_id);
        }
    }
    connected
}

// ============================================================
// EXPORT FORMATS
// ============================================================

pub fn export_to_dot(tree: &BehaviorTree) -> String {
    let mut lines = vec![
        "digraph BehaviorTree {".to_string(),
        "  rankdir=TB;".to_string(),
        "  node [shape=box, style=filled];".to_string(),
    ];

    for (id, node) in &tree.nodes {
        let color = match node.node_type.category() {
            NodeCategory::Composite => "#4682C8",
            NodeCategory::Decorator => "#AA50C8",
            NodeCategory::Action => "#3CB45A",
            NodeCategory::Condition => "#D2B432",
            NodeCategory::Leaf => "#969698",
        };
        let root_marker = if tree.root == Some(*id) { " (ROOT)" } else { "" };
        lines.push(format!(
            "  n{} [label=\"{}{}\", fillcolor=\"{}\", fontcolor=white];",
            id, node.label, root_marker, color
        ));
    }

    for (parent_id, node) in &tree.nodes {
        for child_id in &node.children {
            lines.push(format!("  n{} -> n{};", parent_id, child_id));
        }
    }

    lines.push("}".to_string());
    lines.join("\n")
}

pub fn export_to_mermaid(tree: &BehaviorTree) -> String {
    let mut lines = vec!["graph TD".to_string()];

    for (id, node) in &tree.nodes {
        let shape_open = match node.node_type.category() {
            NodeCategory::Composite => "[",
            NodeCategory::Decorator => "([",
            NodeCategory::Action => "[[",
            NodeCategory::Condition => "{",
            NodeCategory::Leaf => "(",
        };
        let shape_close = match node.node_type.category() {
            NodeCategory::Composite => "]",
            NodeCategory::Decorator => "])",
            NodeCategory::Action => "]]",
            NodeCategory::Condition => "}",
            NodeCategory::Leaf => ")",
        };
        lines.push(format!(
            "  n{}{}{}{}",
            id, shape_open, node.label, shape_close
        ));
    }

    for (parent_id, node) in &tree.nodes {
        for child_id in &node.children {
            lines.push(format!("  n{} --> n{}", parent_id, child_id));
        }
    }

    lines.join("\n")
}

// ============================================================
// TEST / DEBUG HELPERS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_creation() {
        let mut counter = 1u32;
        let tree = BehaviorTree::new_with_root("Test", &mut counter);
        assert!(tree.root.is_some());
        assert_eq!(tree.nodes.len(), 1);
    }

    #[test]
    fn test_add_remove_child() {
        let mut counter = 1u32;
        let mut tree = BehaviorTree::new_with_root("Test", &mut counter);
        let root_id = tree.root.unwrap();

        let child = BtNodeData::new(counter, BtNode::Action(ActionKind::Surrender), (100.0, 100.0));
        let child_id = counter;
        counter += 1;
        tree.add_node(child);
        assert!(tree.add_child(root_id, child_id));

        let root = tree.get_node(root_id).unwrap();
        assert!(root.children.contains(&child_id));

        tree.remove_child(root_id, child_id);
        let root = tree.get_node(root_id).unwrap();
        assert!(!root.children.contains(&child_id));
    }

    #[test]
    fn test_blackboard_compare() {
        let mut bb = Blackboard::new();
        bb.set("hp", BlackboardValue::Float(0.25));

        assert!(bb.compare("hp", &CompareOp::LessThan, &BlackboardValue::Float(0.5)));
        assert!(!bb.compare("hp", &CompareOp::GreaterThan, &BlackboardValue::Float(0.5)));
        assert!(bb.compare("hp", &CompareOp::GreaterThan, &BlackboardValue::Float(0.1)));
    }

    #[test]
    fn test_tree_validate_empty() {
        let tree = BehaviorTree::new("Empty");
        let errors = tree.validate();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_topological_sort() {
        let mut counter = 1u32;
        let mut tree = BehaviorTree::new_with_root("Test", &mut counter);
        let root_id = tree.root.unwrap();

        let child_id = counter;
        counter += 1;
        let child = BtNodeData::new(child_id, BtNode::Action(ActionKind::Surrender), (0.0, 0.0));
        tree.add_node(child);
        tree.add_child(root_id, child_id);

        let sorted = topological_sort(&tree);
        assert_eq!(sorted[0], root_id);
        assert_eq!(sorted[1], child_id);
    }

    #[test]
    fn test_node_depth() {
        let mut counter = 1u32;
        let mut tree = BehaviorTree::new_with_root("Test", &mut counter);
        let root_id = tree.root.unwrap();

        let child_id = counter;
        counter += 1;
        let grandchild_id = counter;
        counter += 1;

        tree.add_node(BtNodeData::new(child_id, BtNode::Selector, (0.0, 0.0)));
        tree.add_node(BtNodeData::new(grandchild_id, BtNode::Action(ActionKind::Surrender), (0.0, 0.0)));
        tree.add_child(root_id, child_id);
        tree.add_child(child_id, grandchild_id);

        assert_eq!(tree.node_depth(root_id), 0);
        assert_eq!(tree.node_depth(child_id), 1);
        assert_eq!(tree.node_depth(grandchild_id), 2);
    }

    #[test]
    fn test_blackboard_string_operations() {
        let mut bb = Blackboard::new();
        bb.set("name", BlackboardValue::Str("hello world".into()));
        assert!(bb.compare("name", &CompareOp::Contains, &BlackboardValue::Str("world".into())));
        assert!(!bb.compare("name", &CompareOp::NotContains, &BlackboardValue::Str("hello".into())));
    }

    #[test]
    fn test_json_roundtrip() {
        let mut counter = 1u32;
        let tree = BehaviorTree::new_with_root("RoundtripTest", &mut counter);
        let json = tree.to_json();
        let restored = BehaviorTree::from_json(&json).unwrap();
        assert_eq!(restored.name, tree.name);
        assert_eq!(restored.nodes.len(), tree.nodes.len());
    }

    #[test]
    fn test_clone_tree() {
        let mut counter = 1u32;
        let tree = BehaviorTree::new_with_root("Original", &mut counter);
        let cloned = clone_tree(&tree, &mut counter);
        assert_ne!(cloned.root, tree.root);
        assert_eq!(cloned.nodes.len(), tree.nodes.len());
    }

    #[test]
    fn test_node_param_lines() {
        let node = BtNodeData::new(1, BtNode::Action(
            ActionKind::MoveTo { target: (10.0, 20.0), speed: 5.0, tolerance: 0.5 }
        ), (0.0, 0.0));
        let params = node.param_lines();
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].0, "Target");
        assert_eq!(params[1].0, "Speed");
    }
}

// ============================================================
// ADVANCED NODE DRAWING HELPERS
// ============================================================

/// Draw a node type badge in the corner of a node rect
fn draw_category_badge(painter: &Painter, rect: Rect, category: &NodeCategory, zoom: f32) {
    let badge_size = 10.0 * zoom;
    let badge_rect = Rect::from_min_size(
        Pos2::new(rect.min.x, rect.max.y - badge_size),
        Vec2::new(badge_size * 3.0, badge_size),
    );
    painter.rect_filled(badge_rect, egui::Rounding::same((2.0 * zoom) as u8), category.dark_color());
    painter.text(
        badge_rect.center(),
        Align2::CENTER_CENTER,
        &category.label()[..3.min(category.label().len())],
        FontId::proportional(7.0 * zoom),
        Color32::from_rgba_premultiplied(255, 255, 255, 200),
    );
}

/// Draw execution count badge on the bottom-right of a node
fn draw_execution_badge(painter: &Painter, rect: Rect, count: u32, zoom: f32) {
    if count == 0 { return; }
    let badge_radius = 7.0 * zoom;
    let center = Pos2::new(rect.max.x - badge_radius, rect.max.y - badge_radius);
    painter.circle_filled(center, badge_radius, Color32::from_rgb(60, 60, 80));
    painter.circle_stroke(center, badge_radius, Stroke::new(1.0 * zoom, Color32::from_rgb(120, 120, 150)));
    let label = if count >= 100 { "99+".to_string() } else { count.to_string() };
    painter.text(
        center,
        Align2::CENTER_CENTER,
        &label,
        FontId::proportional(7.0 * zoom),
        Color32::from_rgb(200, 200, 220),
    );
}

/// Draw a "connecting wire" preview (dashed line) for when user hovers over a port
fn draw_dashed_line(painter: &Painter, from: Pos2, to: Pos2, color: Color32, dash_len: f32, gap_len: f32) {
    let dir = (to - from).normalized();
    let total = (to - from).length();
    let mut t = 0.0_f32;
    let mut draw = true;
    while t < total {
        let seg = if draw { dash_len } else { gap_len };
        let end_t = (t + seg).min(total);
        if draw {
            let p0 = from + dir * t;
            let p1 = from + dir * end_t;
            painter.line_segment([p0, p1], Stroke::new(1.5, color));
        }
        t = end_t;
        draw = !draw;
    }
}

/// Draw port hover indicator (glowing ring around a connection port)
fn draw_port_hover(painter: &Painter, pos: Pos2, zoom: f32) {
    let r1 = 5.0 * zoom;
    let r2 = 8.0 * zoom;
    painter.circle_filled(pos, r2, Color32::from_rgba_premultiplied(100, 180, 255, 60));
    painter.circle_stroke(pos, r1, Stroke::new(1.5 * zoom, Color32::from_rgb(100, 180, 255)));
}

/// Draw a selection highlight ring around a node
fn draw_selection_glow(painter: &Painter, rect: Rect, zoom: f32, frame: u64) {
    let pulse = ((frame as f32 * 0.05).sin() * 0.5 + 0.5);
    let alpha = (60.0 + pulse * 80.0) as u8;
    let expand = (2.0 + pulse * 2.0) * zoom;
    let glow_rect = rect.expand(expand);
    painter.rect_stroke(
        glow_rect,
        egui::Rounding::same(((NODE_CORNER_RADIUS + 2.0) * zoom) as u8),
        Stroke::new(2.0 * zoom, Color32::from_rgba_premultiplied(255, 230, 80, alpha)),
        egui::StrokeKind::Outside,
    );
}

/// Draw a tooltip-like label floating below a node
fn draw_node_tooltip(painter: &Painter, rect: Rect, text: &str, zoom: f32) {
    let tooltip_w = text.len() as f32 * 6.0 * zoom + 8.0 * zoom;
    let tooltip_h = 16.0 * zoom;
    let tooltip_rect = Rect::from_min_size(
        Pos2::new(rect.center().x - tooltip_w / 2.0, rect.max.y + 4.0 * zoom),
        Vec2::new(tooltip_w, tooltip_h),
    );
    painter.rect_filled(tooltip_rect, egui::Rounding::same((3.0 * zoom) as u8),
        Color32::from_rgba_premultiplied(30, 30, 40, 220));
    painter.rect_stroke(tooltip_rect, egui::Rounding::same((3.0 * zoom) as u8),
        Stroke::new(0.5 * zoom, Color32::from_rgb(80, 80, 100)),
        egui::StrokeKind::Outside);
    painter.text(
        tooltip_rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(9.0 * zoom),
        Color32::from_rgb(200, 200, 220),
    );
}

// ============================================================
// ADVANCED LAYOUT ALGORITHMS
// ============================================================

/// Walker's algorithm for aesthetic tree layout (simplified version)
pub fn walker_layout(tree: &mut BehaviorTree, horizontal: bool) {
    let Some(root_id) = tree.root else { return; };

    struct Subtree {
        x: f32,
        mod_: f32,
    }

    let mut mods: HashMap<BtNodeId, f32> = HashMap::new();
    let mut prelim: HashMap<BtNodeId, f32> = HashMap::new();

    fn first_walk(
        tree: &BehaviorTree,
        id: BtNodeId,
        mods: &mut HashMap<BtNodeId, f32>,
        prelim: &mut HashMap<BtNodeId, f32>,
        node_sep: f32,
    ) {
        let children: Vec<BtNodeId> = tree.nodes.get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        if children.is_empty() {
            prelim.insert(id, 0.0);
            mods.insert(id, 0.0);
            return;
        }

        for (i, &child) in children.iter().enumerate() {
            first_walk(tree, child, mods, prelim, node_sep);
        }

        let leftmost = *prelim.get(children.first().unwrap()).unwrap_or(&0.0);
        let rightmost = *prelim.get(children.last().unwrap()).unwrap_or(&0.0);
        let mid = (leftmost + rightmost) / 2.0;

        // Space children out
        let mut cur_x = 0.0_f32;
        for (i, &child) in children.iter().enumerate() {
            if i == 0 {
                prelim.insert(child, 0.0);
            } else {
                let prev = children[i - 1];
                let prev_x = *prelim.get(&prev).unwrap_or(&0.0);
                cur_x = prev_x + (NODE_WIDTH + node_sep);
                prelim.insert(child, cur_x);
            }
        }

        let new_mid = cur_x / 2.0;
        prelim.insert(id, new_mid);
        mods.insert(id, new_mid - mid);
    }

    fn second_walk(
        tree: &mut BehaviorTree,
        id: BtNodeId,
        mod_sum: f32,
        depth: u32,
        prelim: &HashMap<BtNodeId, f32>,
        mods: &HashMap<BtNodeId, f32>,
        level_sep: f32,
        horizontal: bool,
        start_x: f32,
        start_y: f32,
    ) {
        let prelim_x = *prelim.get(&id).unwrap_or(&0.0) + mod_sum;
        let level_pos = depth as f32 * level_sep;

        let (x, y) = if horizontal {
            (start_x + prelim_x, start_y + level_pos)
        } else {
            (start_x + level_pos, start_y + prelim_x)
        };

        if let Some(node) = tree.nodes.get_mut(&id) {
            node.position = (x, y);
        }

        let children: Vec<BtNodeId> = tree.nodes.get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();

        let child_mod = *mods.get(&id).unwrap_or(&0.0);
        for &child in &children {
            second_walk(tree, child, mod_sum + child_mod, depth + 1,
                prelim, mods, level_sep, horizontal, start_x, start_y);
        }
    }

    // Borrow tree immutably for first_walk
    let tree_ref: &BehaviorTree = unsafe { &*(tree as *const BehaviorTree) };
    first_walk(tree_ref, root_id, &mut mods, &mut prelim, 40.0);
    second_walk(tree, root_id, 0.0, 0, &prelim, &mods, 140.0, horizontal, 60.0, 60.0);
}

/// Compact layout: places nodes as tightly as possible while avoiding overlaps
pub fn compact_layout(tree: &mut BehaviorTree) {
    let Some(root_id) = tree.root else { return; };
    let node_h_padding = 20.0_f32;
    let node_v_padding = 100.0_f32;

    // Assign columns by depth
    let mut depth_map: HashMap<BtNodeId, u32> = HashMap::new();
    let mut queue = VecDeque::new();
    queue.push_back((root_id, 0u32));

    while let Some((id, depth)) = queue.pop_front() {
        depth_map.insert(id, depth);
        if let Some(node) = tree.nodes.get(&id) {
            for &child in &node.children {
                if !depth_map.contains_key(&child) {
                    queue.push_back((child, depth + 1));
                }
            }
        }
    }

    // Group by depth
    let max_depth = depth_map.values().copied().max().unwrap_or(0);
    for depth in 0..=max_depth {
        let mut nodes_at_depth: Vec<BtNodeId> = depth_map.iter()
            .filter(|(_, &d)| d == depth)
            .map(|(&id, _)| id)
            .collect();
        nodes_at_depth.sort();

        let y = 60.0 + depth as f32 * node_v_padding;
        let total_w = nodes_at_depth.len() as f32 * (NODE_WIDTH + node_h_padding);
        let start_x = 60.0;

        for (i, &id) in nodes_at_depth.iter().enumerate() {
            if let Some(node) = tree.nodes.get_mut(&id) {
                node.position = (start_x + i as f32 * (NODE_WIDTH + node_h_padding), y);
            }
        }
    }
}

// ============================================================
// ADVANCED SIMULATION FEATURES
// ============================================================

/// Simulate a single node in isolation (for testing)
pub fn simulate_node_isolated(
    node_type: &BtNode,
    blackboard: &mut Blackboard,
    tick: u32,
) -> BtNodeStatus {
    match node_type {
        BtNode::Condition(ConditionKind::IsAlerted) => {
            match blackboard.get("alerted") {
                Some(BlackboardValue::Bool(true)) => BtNodeStatus::Success,
                _ => BtNodeStatus::Failure,
            }
        },
        BtNode::Condition(ConditionKind::IsStunned) => {
            match blackboard.get("stunned") {
                Some(BlackboardValue::Bool(true)) => BtNodeStatus::Success,
                _ => BtNodeStatus::Failure,
            }
        },
        BtNode::Condition(ConditionKind::IsHealthBelow { threshold, use_percentage }) => {
            match blackboard.get("health") {
                Some(BlackboardValue::Float(hp)) => {
                    if (*hp as f32) < *threshold { BtNodeStatus::Success } else { BtNodeStatus::Failure }
                },
                _ => BtNodeStatus::Failure,
            }
        },
        BtNode::Condition(ConditionKind::HasBlackboardKey { key }) => {
            if blackboard.has_key(key) { BtNodeStatus::Success } else { BtNodeStatus::Failure }
        },
        BtNode::CheckBlackboard { key, op, value } => {
            if blackboard.compare(key, op, value) { BtNodeStatus::Success } else { BtNodeStatus::Failure }
        },
        BtNode::SetBlackboard { key, value } => {
            blackboard.set(key.clone(), value.clone());
            BtNodeStatus::Success
        },
        BtNode::Wait(_) => BtNodeStatus::Running,
        BtNode::Log(_) => BtNodeStatus::Success,
        BtNode::Action(_) => BtNodeStatus::Running,
        BtNode::Condition(_) => BtNodeStatus::Failure,
        _ => BtNodeStatus::Success,
    }
}

/// Replay a recorded execution log back into the tree (sets node statuses)
pub fn replay_tick(
    tree: &mut BehaviorTree,
    log: &[(u32, BtNodeId, BtNodeStatus)],
    tick: u32,
) {
    // Reset all to idle
    for node in tree.nodes.values_mut() {
        node.status = BtNodeStatus::Idle;
    }
    // Apply entries for this tick
    for (entry_tick, node_id, status) in log {
        if *entry_tick == tick {
            if let Some(node) = tree.nodes.get_mut(node_id) {
                node.status = status.clone();
            }
        }
    }
}

/// Build a tick-by-tick history from a full execution log
pub fn build_tick_history(
    log: &[(u32, BtNodeId, BtNodeStatus)],
) -> HashMap<u32, Vec<(BtNodeId, BtNodeStatus)>> {
    let mut history: HashMap<u32, Vec<(BtNodeId, BtNodeStatus)>> = HashMap::new();
    for &(tick, id, ref status) in log {
        history.entry(tick).or_default().push((id, status.clone()));
    }
    history
}

// ============================================================
// NODE PALETTE DRAG-AND-DROP SUPPORT
// ============================================================

#[derive(Debug, Clone)]
pub struct PaletteDragPayload {
    pub node_type: BtNode,
    pub label: String,
}

impl PaletteDragPayload {
    pub fn new(node_type: BtNode) -> Self {
        let label = node_type.default_label();
        Self { node_type, label }
    }
}

// ============================================================
// GRAPH ANALYSIS FUNCTIONS
// ============================================================

/// Find all paths from root to a given node
pub fn find_paths_to_node(tree: &BehaviorTree, target: BtNodeId) -> Vec<Vec<BtNodeId>> {
    let Some(root_id) = tree.root else { return Vec::new(); };
    let mut paths = Vec::new();
    let mut current_path = Vec::new();

    fn dfs(
        tree: &BehaviorTree,
        id: BtNodeId,
        target: BtNodeId,
        current_path: &mut Vec<BtNodeId>,
        paths: &mut Vec<Vec<BtNodeId>>,
    ) {
        current_path.push(id);
        if id == target {
            paths.push(current_path.clone());
        } else {
            let children: Vec<BtNodeId> = tree.nodes.get(&id)
                .map(|n| n.children.clone())
                .unwrap_or_default();
            for child in children {
                dfs(tree, child, target, current_path, paths);
            }
        }
        current_path.pop();
    }

    dfs(tree, root_id, target, &mut current_path, &mut paths);
    paths
}

/// Check if the tree has any cycles (shouldn't happen in a well-formed BT, but guard against it)
pub fn has_cycles(tree: &BehaviorTree) -> bool {
    let mut visited: HashSet<BtNodeId> = HashSet::new();
    let mut rec_stack: HashSet<BtNodeId> = HashSet::new();

    fn dfs_cycle(
        tree: &BehaviorTree,
        id: BtNodeId,
        visited: &mut HashSet<BtNodeId>,
        rec_stack: &mut HashSet<BtNodeId>,
    ) -> bool {
        if rec_stack.contains(&id) { return true; }
        if visited.contains(&id) { return false; }
        visited.insert(id);
        rec_stack.insert(id);
        let children: Vec<BtNodeId> = tree.nodes.get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child in children {
            if dfs_cycle(tree, child, visited, rec_stack) {
                return true;
            }
        }
        rec_stack.remove(&id);
        false
    }

    for &id in tree.nodes.keys() {
        if !visited.contains(&id) {
            if dfs_cycle(tree, id, &mut visited, &mut rec_stack) {
                return true;
            }
        }
    }
    false
}

/// Find orphaned nodes (not reachable from root)
pub fn find_orphaned_nodes(tree: &BehaviorTree) -> Vec<BtNodeId> {
    let Some(root_id) = tree.root else {
        return tree.nodes.keys().copied().collect();
    };
    let reachable: HashSet<BtNodeId> = tree.collect_subtree(root_id).into_iter().collect();
    tree.nodes.keys()
        .filter(|id| !reachable.contains(id))
        .copied()
        .collect()
}

/// Compute the width (number of leaf nodes) of a subtree
pub fn subtree_width(tree: &BehaviorTree, id: BtNodeId) -> usize {
    let children: Vec<BtNodeId> = tree.nodes.get(&id)
        .map(|n| n.children.clone())
        .unwrap_or_default();
    if children.is_empty() {
        1
    } else {
        children.iter().map(|&c| subtree_width(tree, c)).sum()
    }
}

// ============================================================
// ADVANCED EDITOR UI COMPONENTS
// ============================================================

/// Show a node type selector combo box
pub fn show_node_type_selector(ui: &mut egui::Ui, current: &BtNode, id_salt: &str) -> Option<BtNode> {
    let mut result = None;
    let current_name = current.type_name();

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(format!("{} {}", current.icon(), current_name))
        .width(200.0)
        .show_ui(ui, |ui| {
            ui.label("Composites");
            for nt in [BtNode::Sequence, BtNode::Selector, BtNode::RandomSelector,
                BtNode::Parallel(ParallelPolicy::RequireAll), BtNode::WeightedSelector(vec![])] {
                if ui.selectable_label(nt == *current, format!("{} {}", nt.icon(), nt.type_name())).clicked() {
                    result = Some(nt);
                }
            }
            ui.separator();
            ui.label("Decorators");
            for nt in [BtNode::Inverter, BtNode::Succeeder, BtNode::Failer,
                BtNode::Repeat(1), BtNode::RepeatUntilFail,
                BtNode::Cooldown { duration_secs: 1.0, shared_key: None },
                BtNode::Timeout(5.0),
                BtNode::Limiter { max_per_interval: 3, interval_secs: 10.0 }] {
                if ui.selectable_label(nt == *current, format!("{} {}", nt.icon(), nt.type_name())).clicked() {
                    result = Some(nt);
                }
            }
            ui.separator();
            ui.label("Leaves");
            for nt in [BtNode::Wait(1.0), BtNode::Log(String::new()),
                BtNode::SetBlackboard { key: String::new(), value: BlackboardValue::Bool(false) },
                BtNode::CheckBlackboard { key: String::new(), op: CompareOp::Equal, value: BlackboardValue::Bool(false) }] {
                if ui.selectable_label(nt == *current, format!("{} {}", nt.icon(), nt.type_name())).clicked() {
                    result = Some(nt);
                }
            }
        });
    result
}

/// Show a compact node card (used in list views or search results)
pub fn show_node_card(ui: &mut egui::Ui, node: &BtNodeData, selected: bool) -> egui::Response {
    let cat = node.node_type.category();
    let color = cat.color();

    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 36.0),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);

        let bg = if selected {
            Color32::from_rgb(50, 60, 80)
        } else if resp.hovered() {
            Color32::from_rgb(42, 42, 52)
        } else {
            Color32::from_rgb(35, 35, 44)
        };

        painter.rect_filled(rect, egui::Rounding::same(4), bg);

        // Left accent bar
        let accent_rect = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
        painter.rect_filled(accent_rect, egui::Rounding::ZERO, color);

        // Icon
        painter.text(
            Pos2::new(rect.min.x + 12.0, rect.center().y),
            Align2::CENTER_CENTER,
            node.node_type.icon(),
            FontId::proportional(13.0),
            color,
        );

        // Label
        painter.text(
            Pos2::new(rect.min.x + 24.0, rect.center().y - 5.0),
            Align2::LEFT_CENTER,
            &node.label,
            FontId::proportional(11.0),
            Color32::WHITE,
        );

        // Type name
        painter.text(
            Pos2::new(rect.min.x + 24.0, rect.center().y + 7.0),
            Align2::LEFT_CENTER,
            node.node_type.type_name(),
            FontId::proportional(9.0),
            Color32::from_rgb(140, 140, 160),
        );

        // Status dot
        painter.circle_filled(
            Pos2::new(rect.max.x - 10.0, rect.center().y),
            4.0,
            node.status.color(),
        );

        if selected {
            painter.rect_stroke(rect, egui::Rounding::same(4),
                Stroke::new(1.0, COLOR_SELECTED_BORDER), egui::StrokeKind::Outside);
        }
    }

    resp
}

/// Show a compact status badge
pub fn show_status_badge(ui: &mut egui::Ui, status: &BtNodeStatus) {
    let color = status.color();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 16.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::Rounding::same(8),
        Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), 60));
    painter.rect_stroke(rect, egui::Rounding::same(8), Stroke::new(1.0, color), egui::StrokeKind::Outside);
    painter.text(rect.center(), Align2::CENTER_CENTER, status.label(),
        FontId::proportional(9.0), color);
}

/// Show a connection info popup
pub fn show_connection_info(
    ui: &mut egui::Ui,
    tree: &BehaviorTree,
    parent_id: BtNodeId,
    child_id: BtNodeId,
) {
    let parent_label = tree.nodes.get(&parent_id).map(|n| n.label.as_str()).unwrap_or("?");
    let child_label = tree.nodes.get(&child_id).map(|n| n.label.as_str()).unwrap_or("?");
    ui.horizontal(|ui| {
        ui.label(parent_label);
        ui.colored_label(Color32::from_rgb(140, 140, 180), " → ");
        ui.label(child_label);
    });
    let child_idx = tree.nodes.get(&parent_id)
        .and_then(|p| p.children.iter().position(|&c| c == child_id))
        .map(|i| format!("Child index: {}", i))
        .unwrap_or_else(|| "Not a direct child".to_string());
    ui.small(&child_idx);
}

// ============================================================
// KEYBOARD SHORTCUT REFERENCE
// ============================================================

pub fn show_keyboard_shortcuts(ui: &mut egui::Ui) {
    ui.heading("Keyboard Shortcuts");
    ui.separator();

    egui::Grid::new("shortcuts_grid")
        .num_columns(2)
        .striped(true)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            let shortcuts: &[(&str, &str)] = &[
                ("Del / Backspace", "Delete selected nodes"),
                ("Ctrl+C", "Copy selected subtree"),
                ("Ctrl+V", "Paste subtree"),
                ("Ctrl+D", "Duplicate selected subtree"),
                ("Ctrl+A", "Select all nodes"),
                ("F", "Fit all nodes in view"),
                ("Space + Drag", "Pan canvas"),
                ("Middle Mouse", "Pan canvas"),
                ("Scroll Wheel", "Zoom in/out"),
                ("Left Click", "Select node"),
                ("Ctrl + Left Click", "Multi-select"),
                ("Right Click", "Open context menu"),
                ("Shift + Drag", "Connect nodes"),
                ("Double Click Tab", "Rename tree"),
                ("Enter (in search)", "Confirm search"),
                ("Escape", "Close context menu / Cancel"),
            ];

            for (key, desc) in shortcuts {
                ui.colored_label(Color32::from_rgb(200, 200, 100), *key);
                ui.label(*desc);
                ui.end_row();
            }
        });
}

// ============================================================
// BLACKBOARD VISUALIZATION
// ============================================================

/// Show a dependency graph for blackboard keys (which nodes read/write each key)
pub fn show_blackboard_dependency_graph(ui: &mut egui::Ui, tree: &BehaviorTree, blackboard: &Blackboard) {
    let keys_map = collect_blackboard_keys(tree);

    if keys_map.is_empty() {
        ui.colored_label(Color32::from_rgb(120, 120, 140), "No blackboard dependencies in this tree.");
        return;
    }

    egui::ScrollArea::vertical().id_salt("bb_deps").show(ui, |ui| {
        for (key, refs) in &keys_map {
            ui.collapsing(key, |ui| {
                let value = blackboard.get(key);
                if let Some(v) = value {
                    ui.horizontal(|ui| {
                        ui.colored_label(Color32::from_rgb(140, 180, 140), "Current:");
                        ui.label(v.display_string());
                        ui.label(format!("({})", v.type_name()));
                    });
                }

                let readers: Vec<BtNodeId> = refs.iter().filter(|(_, w)| !w).map(|(id, _)| *id).collect();
                let writers: Vec<BtNodeId> = refs.iter().filter(|(_, w)| *w).map(|(id, _)| *id).collect();

                if !writers.is_empty() {
                    ui.colored_label(Color32::from_rgb(200, 140, 80), "Writers:");
                    for &nid in &writers {
                        let label = tree.nodes.get(&nid).map(|n| n.label.as_str()).unwrap_or("?");
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.colored_label(Color32::from_rgb(180, 120, 60), "✎");
                            ui.label(label);
                            ui.colored_label(Color32::from_rgb(100, 100, 120), format!("(#{})", nid));
                        });
                    }
                }

                if !readers.is_empty() {
                    ui.colored_label(Color32::from_rgb(100, 160, 220), "Readers:");
                    for &nid in &readers {
                        let label = tree.nodes.get(&nid).map(|n| n.label.as_str()).unwrap_or("?");
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.colored_label(Color32::from_rgb(80, 140, 200), "◎");
                            ui.label(label);
                            ui.colored_label(Color32::from_rgb(100, 100, 120), format!("(#{})", nid));
                        });
                    }
                }
            });
        }
    });
}

// ============================================================
// ENHANCED PROPERTIES PANEL COMPONENTS
// ============================================================

/// Show a color picker with preset swatches for node custom color
pub fn show_color_picker_with_presets(ui: &mut egui::Ui, current: &mut Option<[u8; 3]>) -> bool {
    let presets: &[[u8; 3]] = &[
        [70, 130, 200], [60, 180, 90], [210, 180, 50], [170, 80, 200],
        [200, 80, 80], [80, 200, 200], [200, 140, 60], [140, 200, 80],
        [180, 60, 120], [60, 120, 180], [120, 80, 60], [160, 160, 60],
    ];

    let mut changed = false;
    let use_custom = current.is_some();

    ui.horizontal(|ui| {
        let mut checked = use_custom;
        if ui.checkbox(&mut checked, "Custom color").changed() {
            if checked {
                *current = Some([100, 150, 200]);
            } else {
                *current = None;
            }
            changed = true;
        }
    });

    if let Some(ref mut color) = current {
        ui.label("Presets:");
        ui.horizontal_wrapped(|ui| {
            for &preset in presets {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::click());
                let c = Color32::from_rgb(preset[0], preset[1], preset[2]);
                ui.painter_at(rect).rect_filled(rect, egui::Rounding::same(3), c);
                if *color == preset {
                    ui.painter_at(rect).rect_stroke(rect, egui::Rounding::same(3), Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
                }
                if resp.clicked() {
                    *color = preset;
                    changed = true;
                }
            }
        });

        let mut egui_color = Color32::from_rgb(color[0], color[1], color[2]);
        if ui.color_edit_button_srgba(&mut egui_color).changed() {
            *color = [egui_color.r(), egui_color.g(), egui_color.b()];
            changed = true;
        }
    }

    changed
}

/// Show an enhanced label editor with character count
pub fn show_label_editor(ui: &mut egui::Ui, label: &mut String, max_chars: usize) -> bool {
    ui.horizontal(|ui| {
        let resp = ui.text_edit_singleline(label);
        let over = label.len() > max_chars;
        let count_color = if over { Color32::from_rgb(220, 80, 80) } else { Color32::from_rgb(120, 120, 140) };
        ui.colored_label(count_color, format!("{}/{}", label.len(), max_chars));
        resp.changed()
    }).inner
}

/// Show compact parameter grid for a node type
pub fn show_params_grid(ui: &mut egui::Ui, node: &BtNodeData) {
    let params = node.param_lines();
    if params.is_empty() { return; }

    egui::Grid::new(format!("params_{}", node.id))
        .num_columns(2)
        .spacing([8.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for (key, val) in &params {
                ui.colored_label(Color32::from_rgb(160, 160, 180), key);
                ui.label(val);
                ui.end_row();
            }
        });
}

// ============================================================
// BREAKPOINT MANAGEMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct BreakpointInfo {
    pub node_id: BtNodeId,
    pub condition: BreakpointCondition,
    pub enabled: bool,
    pub hit_count: u32,
    pub label: String,
}

#[derive(Debug, Clone)]
pub enum BreakpointCondition {
    Always,
    OnStatus(BtNodeStatus),
    AfterNHits(u32),
    WhenBlackboardKey(String, CompareOp, BlackboardValue),
}

impl BreakpointCondition {
    pub fn description(&self) -> String {
        match self {
            BreakpointCondition::Always => "Always break".to_string(),
            BreakpointCondition::OnStatus(s) => format!("Break on {}", s.label()),
            BreakpointCondition::AfterNHits(n) => format!("Break after {} hits", n),
            BreakpointCondition::WhenBlackboardKey(k, op, v) => {
                format!("Break when {} {} {}", k, op.symbol(), v.display_string())
            },
        }
    }

    pub fn should_break(&self, status: &BtNodeStatus, hit_count: u32, blackboard: &Blackboard) -> bool {
        match self {
            BreakpointCondition::Always => true,
            BreakpointCondition::OnStatus(s) => s == status,
            BreakpointCondition::AfterNHits(n) => hit_count >= *n,
            BreakpointCondition::WhenBlackboardKey(k, op, v) => blackboard.compare(k, op, v),
        }
    }
}

pub struct BreakpointManager {
    pub breakpoints: Vec<BreakpointInfo>,
}

impl BreakpointManager {
    pub fn new() -> Self {
        Self { breakpoints: Vec::new() }
    }

    pub fn add(&mut self, node_id: BtNodeId, condition: BreakpointCondition, label: String) {
        self.breakpoints.push(BreakpointInfo {
            node_id,
            condition,
            enabled: true,
            hit_count: 0,
            label,
        });
    }

    pub fn remove(&mut self, node_id: BtNodeId) {
        self.breakpoints.retain(|b| b.node_id != node_id);
    }

    pub fn has_breakpoint(&self, node_id: BtNodeId) -> bool {
        self.breakpoints.iter().any(|b| b.node_id == node_id && b.enabled)
    }

    pub fn check_and_trigger(
        &mut self,
        node_id: BtNodeId,
        status: &BtNodeStatus,
        blackboard: &Blackboard,
    ) -> bool {
        for bp in &mut self.breakpoints {
            if bp.node_id == node_id && bp.enabled {
                bp.hit_count += 1;
                if bp.condition.should_break(status, bp.hit_count, blackboard) {
                    return true;
                }
            }
        }
        false
    }

    pub fn clear_all(&mut self) {
        self.breakpoints.clear();
    }

    pub fn enabled_ids(&self) -> HashSet<BtNodeId> {
        self.breakpoints.iter()
            .filter(|b| b.enabled)
            .map(|b| b.node_id)
            .collect()
    }
}

pub fn show_breakpoint_manager(ui: &mut egui::Ui, manager: &mut BreakpointManager, tree: &BehaviorTree) {
    ui.heading("Breakpoints");

    if manager.breakpoints.is_empty() {
        ui.colored_label(Color32::from_rgb(120, 120, 140), "No breakpoints set.");
        ui.label("Right-click a node and select 'Set Breakpoint' to add one.");
        return;
    }

    if ui.button("Clear All").clicked() {
        manager.clear_all();
    }

    ui.separator();

    egui::ScrollArea::vertical().id_salt("bp_manager").max_height(200.0).show(ui, |ui| {
        let mut to_remove: Option<usize> = None;

        for (i, bp) in manager.breakpoints.iter_mut().enumerate() {
            let node_label = tree.nodes.get(&bp.node_id)
                .map(|n| n.label.as_str())
                .unwrap_or("?");

            ui.horizontal(|ui| {
                ui.checkbox(&mut bp.enabled, "");
                ui.colored_label(COLOR_BREAKPOINT, "⊙");
                ui.label(node_label);
                ui.colored_label(Color32::from_rgb(140, 140, 160),
                    bp.condition.description());
                ui.colored_label(Color32::from_rgb(100, 120, 160),
                    format!("hits: {}", bp.hit_count));
                if ui.small_button("✗").clicked() {
                    to_remove = Some(i);
                }
            });
        }

        if let Some(idx) = to_remove {
            manager.breakpoints.remove(idx);
        }
    });
}

// ============================================================
// MULTI-TREE MANAGEMENT
// ============================================================

/// Check if a subtree in one tree can be transplanted to another tree
pub fn can_transplant(
    source_tree: &BehaviorTree,
    source_node_id: BtNodeId,
    target_tree: &BehaviorTree,
    target_parent_id: Option<BtNodeId>,
) -> Result<(), String> {
    let source_node = source_tree.nodes.get(&source_node_id)
        .ok_or_else(|| format!("Source node {} not found", source_node_id))?;

    if let Some(pid) = target_parent_id {
        let target_parent = target_tree.nodes.get(&pid)
            .ok_or_else(|| format!("Target parent {} not found", pid))?;

        if !target_parent.node_type.can_have_children() {
            return Err(format!("Target parent '{}' cannot have children", target_parent.label));
        }

        if let Some(max) = target_parent.node_type.max_children() {
            if target_parent.children.len() >= max {
                return Err(format!("Target parent '{}' already has max children ({})", target_parent.label, max));
            }
        }
    }

    Ok(())
}

/// Transplant a subtree from one tree to another
pub fn transplant_subtree(
    source_tree: &mut BehaviorTree,
    source_node_id: BtNodeId,
    target_tree: &mut BehaviorTree,
    target_parent_id: Option<BtNodeId>,
    id_counter: &mut BtNodeId,
) -> Result<BtNodeId, String> {
    can_transplant(source_tree, source_node_id, target_tree, target_parent_id)?;

    let subtree: Vec<BtNodeData> = source_tree.collect_subtree(source_node_id).iter()
        .filter_map(|&id| source_tree.nodes.get(&id).cloned())
        .collect();

    let mut id_map: HashMap<BtNodeId, BtNodeId> = HashMap::new();
    for node in &subtree {
        let new_id = *id_counter;
        *id_counter += 1;
        id_map.insert(node.id, new_id);
    }

    let new_root_id = *id_map.get(&source_node_id)
        .ok_or("Root ID mapping failed")?;

    for node in &subtree {
        let new_id = *id_map.get(&node.id).unwrap();
        let mut new_node = node.clone();
        new_node.id = new_id;
        new_node.children = node.children.iter()
            .filter_map(|&c| id_map.get(&c).copied())
            .collect();
        new_node.parent = None;
        new_node.status = BtNodeStatus::Idle;
        new_node.execution_count = 0;
        target_tree.nodes.insert(new_id, new_node);
    }

    if let Some(pid) = target_parent_id {
        target_tree.add_child(pid, new_root_id);
    } else if target_tree.root.is_none() {
        target_tree.root = Some(new_root_id);
    }

    // Remove from source
    source_tree.remove_node(source_node_id);

    Ok(new_root_id)
}

// ============================================================
// NODE INSPECTION / HOVER DETAIL
// ============================================================

pub fn show_node_hover_detail(
    ctx: &egui::Context,
    node: &BtNodeData,
    tree: &BehaviorTree,
    breakpoints: &HashSet<BtNodeId>,
) {
    egui::show_tooltip_at_pointer(ctx, egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("node_hover")), egui::Id::new("node_hover_tt"), |ui| {
        let cat = node.node_type.category();
        ui.colored_label(cat.color(), format!("{} {}", node.node_type.icon(), node.node_type.type_name()));
        ui.separator();
        ui.label(&node.label);

        if !node.comment.is_empty() {
            ui.separator();
            ui.colored_label(Color32::from_rgb(180, 200, 120), "Comment:");
            ui.label(&node.comment);
        }

        ui.separator();
        ui.label(node.node_type.description());

        let params = node.param_lines();
        if !params.is_empty() {
            ui.separator();
            ui.label("Parameters:");
            for (k, v) in &params {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(160, 160, 180), k);
                    ui.label(": ");
                    ui.label(v);
                });
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Status:");
            show_status_badge(ui, &node.status);
        });
        ui.label(format!("ID: {} | Depth: {}", node.id, tree.node_depth(node.id)));
        ui.label(format!("Children: {} | Exec: {}", node.children.len(), node.execution_count));

        if breakpoints.contains(&node.id) {
            ui.colored_label(COLOR_BREAKPOINT, "⊙ Breakpoint active");
        }

        if !node.tags.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Tags:");
                for tag in &node.tags {
                    ui.colored_label(Color32::from_rgb(140, 140, 200), format!("#{}", tag));
                }
            });
        }
    });
}

// ============================================================
// DOCUMENTATION / HELP SYSTEM
// ============================================================

pub fn get_node_help_text(node_type: &BtNode) -> String {
    let description = node_type.description();
    let usage = match node_type {
        BtNode::Sequence => "USE WHEN: You need all sub-behaviors to succeed in order. Like a checklist.\nEXAMPLE: 'See enemy' → 'Move to enemy' → 'Attack'",
        BtNode::Selector => "USE WHEN: You want to try alternatives until one succeeds.\nEXAMPLE: 'Attack if in range' OR 'Chase if visible' OR 'Patrol'",
        BtNode::Parallel(_) => "USE WHEN: Multiple behaviors should run simultaneously.\nEXAMPLE: 'Play anim' AND 'Move to target' at the same time.",
        BtNode::RandomSelector => "USE WHEN: You want non-deterministic behavior (variety).\nEXAMPLE: Random idle animations.",
        BtNode::Inverter => "USE WHEN: You need to negate a condition.\nEXAMPLE: 'NOT (enemy is dead)' = continue attacking.",
        BtNode::Succeeder => "USE WHEN: You want to run a subtree without caring if it fails.\nEXAMPLE: Optional bonus actions.",
        BtNode::Repeat(_) => "USE WHEN: A behavior needs to run a fixed number of times.\nEXAMPLE: Attack 3 times before retreating.",
        BtNode::RepeatUntilFail => "USE WHEN: A behavior should loop until it naturally ends.\nEXAMPLE: Keep walking waypoints until path is blocked.",
        BtNode::Cooldown { .. } => "USE WHEN: A behavior should be rate-limited.\nEXAMPLE: Only fire ability every 5 seconds.",
        BtNode::Action(_) => "LEAF NODE: Executes a concrete action. Returns Running while in progress, Success when done, Failure if unable.",
        BtNode::Condition(_) => "LEAF NODE: Tests a game condition. Returns Success if true, Failure if false. Never returns Running.",
        BtNode::Wait(_) => "USE WHEN: You need a simple time delay in a sequence.",
        _ => "No additional guidance available.",
    };
    format!("{}\n\n{}", description, usage)
}

pub fn show_node_help_window(ctx: &egui::Context, node_type: &BtNode, open: &mut bool) {
    egui::Window::new(format!("Help: {}", node_type.type_name()))
        .open(open)
        .resizable(true)
        .default_size([400.0, 300.0])
        .show(ctx, |ui| {
            let cat = node_type.category();
            ui.horizontal(|ui| {
                ui.colored_label(cat.color(), format!("{} {}", node_type.icon(), node_type.type_name()));
                ui.colored_label(cat.color(), format!("[{}]", cat.label()));
            });
            ui.separator();
            let help = get_node_help_text(node_type);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for paragraph in help.split("\n\n") {
                    ui.label(paragraph);
                    ui.add_space(6.0);
                }
            });
        });
}

// ============================================================
// EXPORT / IMPORT: ADDITIONAL FORMATS
// ============================================================

/// Export the tree as a flat CSV for spreadsheet analysis
pub fn export_to_csv(tree: &BehaviorTree) -> String {
    let mut lines = vec![
        "ID,Label,Type,Category,ParentID,ChildCount,Depth,Enabled,HasComment,Tags".to_string()
    ];

    let sorted = topological_sort(tree);
    for id in sorted {
        let Some(node) = tree.nodes.get(&id) else { continue; };
        let depth = tree.node_depth(id);
        lines.push(format!(
            "{},{},{},{},{},{},{},{},{},\"{}\"",
            id,
            node.label.replace(',', ";"),
            node.node_type.type_name(),
            node.node_type.category().label(),
            node.parent.map(|p| p.to_string()).unwrap_or_else(|| "none".to_string()),
            node.children.len(),
            depth,
            node.enabled,
            !node.comment.is_empty(),
            node.tags.join("|"),
        ));
    }

    lines.join("\n")
}

/// Parse a simplified text-based tree description into a BehaviorTree
/// Format: each line is "INDENT TYPE [label]"
/// Example:
/// Sequence [Root]
///   Condition:CanSeePlayer [See Player]
///   Action:Attack [Attack]
pub fn import_from_text(text: &str, id_counter: &mut BtNodeId) -> Result<BehaviorTree, String> {
    let mut tree = BehaviorTree::new("Imported");

    #[derive(Debug)]
    struct ParsedLine {
        depth: usize,
        node_type_str: String,
        label: String,
        id: BtNodeId,
    }

    let mut parsed: Vec<ParsedLine> = Vec::new();

    for line in text.lines() {
        if line.trim().is_empty() { continue; }
        let spaces = line.len() - line.trim_start().len();
        let depth = spaces / 2;
        let rest = line.trim();
        let (type_part, label) = if let Some(idx) = rest.find('[') {
            let lbl = rest[idx+1..].trim_end_matches(']').trim().to_string();
            (rest[..idx].trim(), lbl)
        } else {
            (rest, rest.to_string())
        };

        let id = *id_counter;
        *id_counter += 1;
        parsed.push(ParsedLine { depth, node_type_str: type_part.to_string(), label, id });
    }

    // Build tree from parsed lines
    let mut stack: Vec<(usize, BtNodeId)> = Vec::new();

    for line in &parsed {
        let nt = match line.node_type_str.as_str() {
            "Sequence" => BtNode::Sequence,
            "Selector" => BtNode::Selector,
            "Parallel" => BtNode::Parallel(ParallelPolicy::RequireAll),
            "RandomSelector" => BtNode::RandomSelector,
            "Inverter" => BtNode::Inverter,
            "Succeeder" => BtNode::Succeeder,
            "Failer" => BtNode::Failer,
            "RepeatUntilFail" => BtNode::RepeatUntilFail,
            "Wait" => BtNode::Wait(1.0),
            s if s.starts_with("Repeat:") => {
                let n: u32 = s[7..].parse().unwrap_or(1);
                BtNode::Repeat(n)
            },
            s if s.starts_with("Condition:") => {
                BtNode::Condition(ConditionKind::CheckFlag { flag_name: s[10..].to_string() })
            },
            s if s.starts_with("Action:") => {
                BtNode::Action(ActionKind::TriggerEvent { event_name: s[7..].to_string(), payload: "{}".into() })
            },
            _ => BtNode::Action(ActionKind::Idle { duration: 1.0 }),
        };

        let y_pos = 60.0 + line.depth as f32 * 120.0;
        let x_pos = 60.0 + parsed.iter().enumerate()
            .filter(|(i, p)| p.depth == line.depth && *i <= parsed.iter().position(|p2| p2.id == line.id).unwrap_or(0))
            .count() as f32 * 220.0;

        let mut node = BtNodeData::new(line.id, nt, (x_pos, y_pos));
        node.label = line.label.clone();
        tree.nodes.insert(line.id, node);

        // Pop stack to find parent
        while stack.last().map(|(d, _)| *d >= line.depth).unwrap_or(false) {
            stack.pop();
        }

        if let Some(&(_, parent_id)) = stack.last() {
            tree.add_child(parent_id, line.id);
        } else if tree.root.is_none() {
            tree.root = Some(line.id);
        }

        stack.push((line.depth, line.id));
    }

    if tree.root.is_none() {
        return Err("No root node found in text".into());
    }

    Ok(tree)
}

// ============================================================
// NODE COMMENTING AND ANNOTATION
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasAnnotation {
    pub id: u32,
    pub position: (f32, f32),
    pub size: (f32, f32),
    pub text: String,
    pub color: [u8; 4],
    pub style: AnnotationStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnnotationStyle {
    StickyNote,
    Box,
    Arrow { to: (f32, f32) },
    Highlight,
}

impl CanvasAnnotation {
    pub fn new_sticky(id: u32, pos: (f32, f32), text: impl Into<String>) -> Self {
        Self {
            id,
            position: pos,
            size: (200.0, 80.0),
            text: text.into(),
            color: [200, 190, 80, 200],
            style: AnnotationStyle::StickyNote,
        }
    }

    pub fn new_box(id: u32, pos: (f32, f32), size: (f32, f32)) -> Self {
        Self {
            id,
            position: pos,
            size,
            text: String::new(),
            color: [100, 120, 200, 80],
            style: AnnotationStyle::Box,
        }
    }
}

pub fn draw_annotation(painter: &Painter, annotation: &CanvasAnnotation, offset: Vec2, zoom: f32) {
    let x = annotation.position.0 * zoom + offset.x;
    let y = annotation.position.1 * zoom + offset.y;
    let w = annotation.size.0 * zoom;
    let h = annotation.size.1 * zoom;
    let [r, g, b, a] = annotation.color;
    let color = Color32::from_rgba_premultiplied(r, g, b, a);

    match &annotation.style {
        AnnotationStyle::StickyNote => {
            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
            painter.rect_filled(rect, egui::Rounding::same((3.0 * zoom) as u8), color);
            painter.rect_stroke(rect, egui::Rounding::same((3.0 * zoom) as u8),
                Stroke::new(1.0 * zoom, Color32::from_rgba_premultiplied(r/2, g/2, b/2, 200)),
                egui::StrokeKind::Outside);
            if !annotation.text.is_empty() {
                painter.text(
                    Pos2::new(x + 6.0 * zoom, y + 6.0 * zoom),
                    Align2::LEFT_TOP,
                    &annotation.text,
                    FontId::proportional(10.0 * zoom),
                    Color32::from_rgb(30, 30, 30),
                );
            }
        },
        AnnotationStyle::Box => {
            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
            painter.rect_filled(rect, egui::Rounding::same((4.0 * zoom) as u8), color);
            painter.rect_stroke(rect, egui::Rounding::same((4.0 * zoom) as u8),
                Stroke::new(1.5 * zoom, Color32::from_rgba_premultiplied(r, g, b, 220)),
                egui::StrokeKind::Outside);
        },
        AnnotationStyle::Arrow { to } => {
            let from = Pos2::new(x, y);
            let to_screen = Pos2::new(to.0 * zoom + offset.x, to.1 * zoom + offset.y);
            painter.line_segment([from, to_screen], Stroke::new(2.0 * zoom, color));
            let dir = (to_screen - from).normalized();
            let perp = Vec2::new(-dir.y, dir.x);
            let tip = to_screen;
            painter.add(Shape::convex_polygon(
                vec![tip, tip - dir * 10.0 * zoom + perp * 4.0 * zoom, tip - dir * 10.0 * zoom - perp * 4.0 * zoom],
                color, Stroke::NONE,
            ));
        },
        AnnotationStyle::Highlight => {
            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
            painter.rect_filled(rect, egui::Rounding::same((2.0 * zoom) as u8),
                Color32::from_rgba_premultiplied(r, g, b, (a as u32 / 2) as u8));
        },
    }
}

// ============================================================
// PERFORMANCE TRACKING
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct NodePerformanceData {
    pub total_ticks: u32,
    pub success_count: u32,
    pub failure_count: u32,
    pub running_count: u32,
    pub avg_running_duration: f32,
    pub last_running_start_tick: Option<u32>,
}

impl NodePerformanceData {
    pub fn success_rate(&self) -> f32 {
        let total = (self.success_count + self.failure_count) as f32;
        if total == 0.0 { 0.0 } else { self.success_count as f32 / total }
    }

    pub fn record(&mut self, status: &BtNodeStatus, tick: u32) {
        self.total_ticks += 1;
        match status {
            BtNodeStatus::Success => {
                self.success_count += 1;
                if let Some(start) = self.last_running_start_tick.take() {
                    let duration = (tick - start) as f32;
                    self.avg_running_duration =
                        (self.avg_running_duration * (self.running_count as f32) + duration)
                        / (self.running_count as f32 + 1.0);
                    self.running_count += 1;
                }
            },
            BtNodeStatus::Failure => {
                self.failure_count += 1;
                self.last_running_start_tick = None;
            },
            BtNodeStatus::Running => {
                if self.last_running_start_tick.is_none() {
                    self.last_running_start_tick = Some(tick);
                }
            },
            BtNodeStatus::Idle => {},
        }
    }
}

pub fn show_performance_panel(
    ui: &mut egui::Ui,
    perf_data: &HashMap<BtNodeId, NodePerformanceData>,
    tree: &BehaviorTree,
) {
    ui.heading("Node Performance");
    ui.separator();

    if perf_data.is_empty() {
        ui.colored_label(Color32::from_rgb(120, 120, 140), "No performance data yet. Run simulation first.");
        return;
    }

    let mut entries: Vec<(BtNodeId, &NodePerformanceData)> = perf_data.iter()
        .map(|(&id, d)| (id, d))
        .collect();
    entries.sort_by(|a, b| b.1.total_ticks.cmp(&a.1.total_ticks));

    egui::ScrollArea::vertical().id_salt("perf_panel").show(ui, |ui| {
        egui::Grid::new("perf_grid")
            .num_columns(5)
            .striped(true)
            .spacing([8.0, 2.0])
            .show(ui, |ui| {
                ui.colored_label(Color32::from_rgb(180, 180, 200), "Node");
                ui.colored_label(Color32::from_rgb(180, 180, 200), "Ticks");
                ui.colored_label(Color32::from_rgb(80, 220, 80), "Success");
                ui.colored_label(Color32::from_rgb(220, 80, 80), "Fail");
                ui.colored_label(Color32::from_rgb(220, 180, 80), "Rate");
                ui.end_row();

                for (id, data) in &entries {
                    let label = tree.nodes.get(id)
                        .map(|n| n.label.as_str())
                        .unwrap_or("?");
                    let rate = data.success_rate();
                    let rate_color = if rate > 0.7 {
                        Color32::from_rgb(80, 200, 80)
                    } else if rate > 0.4 {
                        Color32::from_rgb(200, 180, 60)
                    } else {
                        Color32::from_rgb(200, 80, 80)
                    };

                    ui.label(label);
                    ui.label(data.total_ticks.to_string());
                    ui.colored_label(COLOR_STATUS_SUCCESS, data.success_count.to_string());
                    ui.colored_label(COLOR_STATUS_FAILURE, data.failure_count.to_string());
                    ui.colored_label(rate_color, format!("{:.0}%", rate * 100.0));
                    ui.end_row();
                }
            });
    });
}

// ============================================================
// FINAL UTILITY IMPLS
// ============================================================

impl Default for BehaviorTreeEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Blackboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SimulationState {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for BreakpointManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for UndoHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// ADVANCED COMPOSITE NODE BEHAVIORS
// ============================================================

/// Memory Sequence — remembers the last running child and resumes from there
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySequenceState {
    pub last_running_index: usize,
}

/// Memory Selector — like selector but resumes from last non-failed child
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySelectorState {
    pub last_running_index: usize,
}

/// Priority Selector — dynamically reorders children by a priority value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrioritySelectorState {
    pub priorities: HashMap<BtNodeId, f32>,
}

/// Extended composite node with additional policy options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExtendedCompositePolicy {
    Sequence,
    MemorySequence,
    Selector,
    MemorySelector,
    PrioritySelector,
    DynamicGuardSelector,
    InterruptibleSelector,
}

impl ExtendedCompositePolicy {
    pub fn display_name(&self) -> &'static str {
        match self {
            ExtendedCompositePolicy::Sequence => "Sequence",
            ExtendedCompositePolicy::MemorySequence => "Memory Sequence",
            ExtendedCompositePolicy::Selector => "Selector",
            ExtendedCompositePolicy::MemorySelector => "Memory Selector",
            ExtendedCompositePolicy::PrioritySelector => "Priority Selector",
            ExtendedCompositePolicy::DynamicGuardSelector => "Dynamic Guard Selector",
            ExtendedCompositePolicy::InterruptibleSelector => "Interruptible Selector",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ExtendedCompositePolicy::Sequence => "Runs children in order; fails on first failure.",
            ExtendedCompositePolicy::MemorySequence => "Like Sequence but remembers the last running child.",
            ExtendedCompositePolicy::Selector => "Tries children in order; succeeds on first success.",
            ExtendedCompositePolicy::MemorySelector => "Like Selector but resumes from last position.",
            ExtendedCompositePolicy::PrioritySelector => "Runs the highest-priority child each tick.",
            ExtendedCompositePolicy::DynamicGuardSelector => "Children have guards; highest-priority satisfied guard wins.",
            ExtendedCompositePolicy::InterruptibleSelector => "Lower-priority running children can be interrupted.",
        }
    }
}

// ============================================================
// DECORATOR CHAIN HELPER
// ============================================================

/// Build a chain of decorators wrapping a leaf node
/// e.g. Cooldown → Repeat → Inverter → Action
pub fn build_decorator_chain(
    decorators: Vec<BtNode>,
    leaf: BtNode,
    base_pos: (f32, f32),
    id_counter: &mut BtNodeId,
) -> Vec<BtNodeData> {
    let mut nodes: Vec<BtNodeData> = Vec::new();
    let leaf_id = *id_counter;
    *id_counter += 1;
    let leaf_node = BtNodeData::new(leaf_id, leaf, base_pos);
    nodes.push(leaf_node);

    let mut current_child = leaf_id;
    for (i, dec) in decorators.into_iter().enumerate().rev() {
        let dec_id = *id_counter;
        *id_counter += 1;
        let y_offset = -(i as f32 + 1.0) * 120.0;
        let mut dec_node = BtNodeData::new(dec_id, dec, (base_pos.0, base_pos.1 + y_offset));
        dec_node.children.push(current_child);
        // Update child's parent
        if let Some(child_node) = nodes.iter_mut().find(|n| n.id == current_child) {
            child_node.parent = Some(dec_id);
        }
        nodes.push(dec_node);
        current_child = dec_id;
    }

    nodes
}

// ============================================================
// NODE HOTKEY BINDING
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub struct NodeHotkey {
    pub key: egui::Key,
    pub modifiers: egui::Modifiers,
    pub node_type: BtNode,
    pub label: String,
}

impl NodeHotkey {
    pub fn new(key: egui::Key, modifiers: egui::Modifiers, node_type: BtNode) -> Self {
        let label = node_type.default_label();
        Self { key, modifiers, node_type, label }
    }

    pub fn matches(&self, input: &egui::InputState) -> bool {
        input.key_pressed(self.key) && input.modifiers == self.modifiers
    }
}

pub fn default_hotkeys() -> Vec<NodeHotkey> {
    vec![
        NodeHotkey::new(
            egui::Key::Num1,
            egui::Modifiers::NONE,
            BtNode::Sequence,
        ),
        NodeHotkey::new(
            egui::Key::Num2,
            egui::Modifiers::NONE,
            BtNode::Selector,
        ),
        NodeHotkey::new(
            egui::Key::Num3,
            egui::Modifiers::NONE,
            BtNode::Parallel(ParallelPolicy::RequireAll),
        ),
        NodeHotkey::new(
            egui::Key::Num4,
            egui::Modifiers::NONE,
            BtNode::Inverter,
        ),
        NodeHotkey::new(
            egui::Key::Num5,
            egui::Modifiers::NONE,
            BtNode::Action(ActionKind::Idle { duration: 1.0 }),
        ),
        NodeHotkey::new(
            egui::Key::Num6,
            egui::Modifiers::NONE,
            BtNode::Wait(1.0),
        ),
    ]
}

// ============================================================
// SERIALIZATION HELPERS
// ============================================================

/// Compact serialization format (smaller than full JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactNodeRecord {
    pub i: BtNodeId,
    pub t: String,
    pub l: String,
    pub c: Vec<BtNodeId>,
    pub p: (f32, f32),
    pub e: bool,
}

impl From<&BtNodeData> for CompactNodeRecord {
    fn from(n: &BtNodeData) -> Self {
        Self {
            i: n.id,
            t: n.node_type.type_name().to_string(),
            l: n.label.clone(),
            c: n.children.clone(),
            p: n.position,
            e: n.enabled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactTree {
    pub name: String,
    pub root: Option<BtNodeId>,
    pub nodes: Vec<CompactNodeRecord>,
}

impl CompactTree {
    pub fn from_tree(tree: &BehaviorTree) -> Self {
        let nodes = tree.nodes.values().map(CompactNodeRecord::from).collect();
        Self {
            name: tree.name.clone(),
            root: tree.root,
            nodes,
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

// ============================================================
// VISUAL STYLE THEMES
// ============================================================

#[derive(Debug, Clone)]
pub struct BtEditorTheme {
    pub canvas_bg: Color32,
    pub grid_dot: Color32,
    pub node_bg: Color32,
    pub node_header_composite: Color32,
    pub node_header_action: Color32,
    pub node_header_condition: Color32,
    pub node_header_decorator: Color32,
    pub node_header_leaf: Color32,
    pub connection: Color32,
    pub connection_active: Color32,
    pub selection_border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub panel_bg: Color32,
    pub panel_header: Color32,
}

impl BtEditorTheme {
    pub fn dark() -> Self {
        Self {
            canvas_bg: COLOR_CANVAS_BG,
            grid_dot: COLOR_GRID_DOT,
            node_bg: COLOR_NODE_BG,
            node_header_composite: COLOR_COMPOSITE,
            node_header_action: COLOR_ACTION,
            node_header_condition: COLOR_CONDITION,
            node_header_decorator: COLOR_DECORATOR,
            node_header_leaf: COLOR_LEAF,
            connection: COLOR_CONNECTION,
            connection_active: COLOR_CONNECTION_ACTIVE,
            selection_border: COLOR_SELECTED_BORDER,
            text_primary: COLOR_NODE_TEXT,
            text_secondary: COLOR_NODE_SUBTEXT,
            panel_bg: COLOR_PANEL_BG,
            panel_header: COLOR_PANEL_HEADER,
        }
    }

    pub fn light() -> Self {
        Self {
            canvas_bg: Color32::from_rgb(240, 240, 245),
            grid_dot: Color32::from_rgb(200, 200, 210),
            node_bg: Color32::from_rgb(250, 250, 255),
            node_header_composite: Color32::from_rgb(50, 100, 180),
            node_header_action: Color32::from_rgb(30, 140, 60),
            node_header_condition: Color32::from_rgb(160, 130, 20),
            node_header_decorator: Color32::from_rgb(130, 50, 170),
            node_header_leaf: Color32::from_rgb(100, 100, 110),
            connection: Color32::from_rgb(120, 120, 140),
            connection_active: Color32::from_rgb(40, 140, 220),
            selection_border: Color32::from_rgb(200, 160, 0),
            text_primary: Color32::from_rgb(30, 30, 40),
            text_secondary: Color32::from_rgb(80, 80, 100),
            panel_bg: Color32::from_rgb(220, 220, 230),
            panel_header: Color32::from_rgb(180, 180, 200),
        }
    }

    pub fn high_contrast() -> Self {
        Self {
            canvas_bg: Color32::BLACK,
            grid_dot: Color32::from_rgb(50, 50, 60),
            node_bg: Color32::from_rgb(20, 20, 24),
            node_header_composite: Color32::from_rgb(0, 140, 255),
            node_header_action: Color32::from_rgb(0, 220, 80),
            node_header_condition: Color32::from_rgb(255, 220, 0),
            node_header_decorator: Color32::from_rgb(200, 0, 255),
            node_header_leaf: Color32::from_rgb(160, 160, 170),
            connection: Color32::from_rgb(160, 160, 180),
            connection_active: Color32::from_rgb(0, 220, 255),
            selection_border: Color32::from_rgb(255, 255, 0),
            text_primary: Color32::WHITE,
            text_secondary: Color32::from_rgb(180, 180, 200),
            panel_bg: Color32::from_rgb(15, 15, 20),
            panel_header: Color32::from_rgb(30, 30, 40),
        }
    }
}

// ============================================================
// TIMELINE / REPLAY VIEW
// ============================================================

pub struct TimelineView {
    pub tick_range: (u32, u32),
    pub selected_tick: u32,
    pub zoom: f32,
    pub scroll: f32,
}

impl TimelineView {
    pub fn new() -> Self {
        Self {
            tick_range: (0, 100),
            selected_tick: 0,
            zoom: 1.0,
            scroll: 0.0,
        }
    }
}

pub fn show_timeline(
    ui: &mut egui::Ui,
    timeline: &mut TimelineView,
    log: &[(u32, BtNodeId, BtNodeStatus)],
    tree: &BehaviorTree,
) {
    if log.is_empty() {
        ui.label("No execution history.");
        return;
    }

    let max_tick = log.iter().map(|(t, _, _)| *t).max().unwrap_or(0);
    timeline.tick_range.1 = max_tick;

    ui.heading(format!("Timeline (tick {}/{})", timeline.selected_tick, max_tick));

    // Timeline scrubber
    let scrubber_response = ui.add(
        egui::Slider::new(&mut timeline.selected_tick, 0..=max_tick)
            .text("Tick")
            .integer()
    );

    ui.separator();

    // Build per-node status at selected tick
    let tick_log: Vec<(BtNodeId, &BtNodeStatus)> = log.iter()
        .filter(|(t, _, _)| *t == timeline.selected_tick)
        .map(|(_, id, s)| (*id, s))
        .collect();

    egui::ScrollArea::vertical()
        .id_salt("timeline_log")
        .max_height(150.0)
        .show(ui, |ui| {
            for (node_id, status) in &tick_log {
                let label = tree.nodes.get(node_id)
                    .map(|n| n.label.as_str())
                    .unwrap_or("?");
                ui.horizontal(|ui| {
                    let cat_color = tree.nodes.get(node_id)
                        .map(|n| n.node_type.category().color())
                        .unwrap_or(Color32::GRAY);
                    ui.colored_label(cat_color, label);
                    ui.colored_label(status.color(), format!("→ {}", status.label()));
                });
            }

            if tick_log.is_empty() {
                ui.colored_label(Color32::from_rgb(120, 120, 140),
                    format!("No nodes executed at tick {}", timeline.selected_tick));
            }
        });

    // Mini heat-map by node
    ui.separator();
    ui.label("Node activity heat-map:");

    let node_ids: Vec<BtNodeId> = tree.nodes.keys().copied().collect();
    let ticks_per_node: HashMap<BtNodeId, usize> = {
        let mut map: HashMap<BtNodeId, usize> = HashMap::new();
        for (_, id, _) in log {
            *map.entry(*id).or_insert(0) += 1;
        }
        map
    };
    let max_count = ticks_per_node.values().copied().max().unwrap_or(1) as f32;

    ui.horizontal_wrapped(|ui| {
        let mut sorted_ids: Vec<BtNodeId> = tree.nodes.keys().copied().collect();
        sorted_ids.sort();

        for id in sorted_ids {
            let count = *ticks_per_node.get(&id).unwrap_or(&0) as f32;
            let intensity = (count / max_count).min(1.0);
            let label = tree.nodes.get(&id)
                .map(|n| if n.label.len() > 8 { format!("{}…", &n.label[..6]) } else { n.label.clone() })
                .unwrap_or_else(|| format!("#{}", id));
            let heat_color = Color32::from_rgb(
                (255.0 * intensity) as u8,
                (80.0 + 100.0 * (1.0 - intensity)) as u8,
                50,
            );
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(60.0, 28.0), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, egui::Rounding::same(3),
                    Color32::from_rgba_premultiplied(heat_color.r(), heat_color.g(), heat_color.b(), 160));
                painter.text(rect.center(), Align2::CENTER_CENTER, &label,
                    FontId::proportional(8.0), Color32::WHITE);
            }
            if resp.hovered() {
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(id), |ui| {
                    ui.label(format!("Node #{}: {}", id, count as u32));
                });
            }
        }
    });
}

// ============================================================
// ADVANCED BLACKBOARD FEATURES
// ============================================================

/// Compute the minimum and maximum values seen for a Float key in the execution log
pub fn blackboard_float_range(
    log_snapshots: &[(u32, String, BlackboardValue)],
    key: &str,
) -> Option<(f64, f64)> {
    let values: Vec<f64> = log_snapshots.iter()
        .filter(|(_, k, _)| k == key)
        .filter_map(|(_, _, v)| if let BlackboardValue::Float(f) = v { Some(*f) } else { None })
        .collect();

    if values.is_empty() { return None; }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some((min, max))
}

/// Blackboard snapshot: record current state for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardSnapshot {
    pub tick: u32,
    pub entries: HashMap<String, BlackboardValue>,
}

impl BlackboardSnapshot {
    pub fn capture(blackboard: &Blackboard, tick: u32) -> Self {
        Self {
            tick,
            entries: blackboard.entries.clone(),
        }
    }

    pub fn diff(&self, other: &BlackboardSnapshot) -> Vec<(String, Option<BlackboardValue>, Option<BlackboardValue>)> {
        let mut changes = Vec::new();
        let all_keys: HashSet<&String> = self.entries.keys().chain(other.entries.keys()).collect();

        for key in all_keys {
            let before = self.entries.get(key).cloned();
            let after = other.entries.get(key).cloned();
            if before != after {
                changes.push((key.clone(), before, after));
            }
        }
        changes
    }
}

pub struct BlackboardHistory {
    pub snapshots: VecDeque<BlackboardSnapshot>,
    pub max_snapshots: usize,
}

impl BlackboardHistory {
    pub fn new() -> Self {
        Self {
            snapshots: VecDeque::new(),
            max_snapshots: 500,
        }
    }

    pub fn capture(&mut self, blackboard: &Blackboard, tick: u32) {
        if self.snapshots.len() >= self.max_snapshots {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(BlackboardSnapshot::capture(blackboard, tick));
    }

    pub fn get_at_tick(&self, tick: u32) -> Option<&BlackboardSnapshot> {
        self.snapshots.iter().find(|s| s.tick == tick)
    }

    pub fn get_changes_since(&self, since_tick: u32) -> Vec<(u32, String, BlackboardValue)> {
        let mut changes = Vec::new();
        let mut prev: Option<&BlackboardSnapshot> = None;
        for snap in &self.snapshots {
            if snap.tick < since_tick { prev = Some(snap); continue; }
            if let Some(p) = prev {
                for (key, after) in &snap.entries {
                    if p.entries.get(key) != Some(after) {
                        changes.push((snap.tick, key.clone(), after.clone()));
                    }
                }
            }
            prev = Some(snap);
        }
        changes
    }
}

impl Default for BlackboardHistory {
    fn default() -> Self {
        Self::new()
    }
}

pub fn show_blackboard_history(
    ui: &mut egui::Ui,
    history: &BlackboardHistory,
    current_tick: u32,
) {
    ui.heading("Blackboard History");

    let changes = history.get_changes_since(current_tick.saturating_sub(50));

    if changes.is_empty() {
        ui.colored_label(Color32::from_rgb(120, 120, 140), "No recent blackboard changes.");
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("bb_history")
        .max_height(180.0)
        .show(ui, |ui| {
            for (tick, key, value) in changes.iter().rev().take(30) {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(120, 120, 160), format!("[{}]", tick));
                    ui.colored_label(Color32::from_rgb(180, 200, 120), key);
                    ui.label("←");
                    ui.colored_label(Color32::from_rgb(200, 200, 220), value.display_string());
                    ui.colored_label(Color32::from_rgb(120, 120, 140), format!("({})", value.type_name()));
                });
            }
        });
}

// ============================================================
// NODE SEARCH WITH ADVANCED FILTERS
// ============================================================

#[derive(Debug, Clone)]
pub struct NodeSearchFilter {
    pub text: String,
    pub category: Option<NodeCategory>,
    pub status: Option<BtNodeStatus>,
    pub only_enabled: bool,
    pub only_with_breakpoints: bool,
    pub has_comment: Option<bool>,
    pub min_execution_count: Option<u32>,
}

impl Default for NodeSearchFilter {
    fn default() -> Self {
        Self {
            text: String::new(),
            category: None,
            status: None,
            only_enabled: false,
            only_with_breakpoints: false,
            has_comment: None,
            min_execution_count: None,
        }
    }
}

impl NodeSearchFilter {
    pub fn matches(&self, node: &BtNodeData, breakpoints: &HashSet<BtNodeId>) -> bool {
        if !self.text.is_empty() {
            let q = self.text.to_lowercase();
            let label_match = node.label.to_lowercase().contains(&q);
            let type_match = node.node_type.type_name().to_lowercase().contains(&q);
            let comment_match = node.comment.to_lowercase().contains(&q);
            let tag_match = node.tags.iter().any(|t| t.to_lowercase().contains(&q));
            if !label_match && !type_match && !comment_match && !tag_match {
                return false;
            }
        }
        if let Some(ref cat) = self.category {
            if node.node_type.category() != *cat { return false; }
        }
        if let Some(ref status) = self.status {
            if node.status != *status { return false; }
        }
        if self.only_enabled && !node.enabled { return false; }
        if self.only_with_breakpoints && !breakpoints.contains(&node.id) { return false; }
        if let Some(has_comment) = self.has_comment {
            if has_comment != !node.comment.is_empty() { return false; }
        }
        if let Some(min_exec) = self.min_execution_count {
            if node.execution_count < min_exec { return false; }
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.category.is_none()
            && self.status.is_none()
            && !self.only_enabled
            && !self.only_with_breakpoints
            && self.has_comment.is_none()
            && self.min_execution_count.is_none()
    }
}

pub fn show_advanced_search(
    ui: &mut egui::Ui,
    filter: &mut NodeSearchFilter,
    tree: &BehaviorTree,
    breakpoints: &HashSet<BtNodeId>,
    selected: &mut Option<BtNodeId>,
) {
    ui.collapsing("Advanced Search", |ui| {
        ui.horizontal(|ui| {
            ui.label("Text:");
            ui.text_edit_singleline(&mut filter.text);
        });

        ui.horizontal(|ui| {
            ui.label("Category:");
            let cat_text = filter.category.as_ref().map(|c| c.label()).unwrap_or("Any");
            egui::ComboBox::from_id_salt("adv_search_cat")
                .selected_text(cat_text)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(filter.category.is_none(), "Any").clicked() {
                        filter.category = None;
                    }
                    for cat in NodeCategory::all() {
                        let selected_now = filter.category.as_ref() == Some(&cat);
                        if ui.selectable_label(selected_now, cat.label()).clicked() {
                            filter.category = Some(cat);
                        }
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Status:");
            let status_text = filter.status.as_ref().map(|s| s.label()).unwrap_or("Any");
            egui::ComboBox::from_id_salt("adv_search_status")
                .selected_text(status_text)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(filter.status.is_none(), "Any").clicked() {
                        filter.status = None;
                    }
                    for s in [BtNodeStatus::Idle, BtNodeStatus::Running, BtNodeStatus::Success, BtNodeStatus::Failure] {
                        let selected_now = filter.status.as_ref() == Some(&s);
                        if ui.selectable_label(selected_now, s.label()).clicked() {
                            filter.status = Some(s);
                        }
                    }
                });
        });

        ui.checkbox(&mut filter.only_enabled, "Enabled only");
        ui.checkbox(&mut filter.only_with_breakpoints, "With breakpoints only");

        ui.horizontal(|ui| {
            ui.label("Min exec count:");
            let mut v = filter.min_execution_count.unwrap_or(0);
            if ui.add(egui::DragValue::new(&mut v).range(0..=100000)).changed() {
                filter.min_execution_count = if v == 0 { None } else { Some(v) };
            }
        });

        if ui.button("Clear Filters").clicked() {
            *filter = NodeSearchFilter::default();
        }

        if !filter.is_empty() {
            ui.separator();
            let results: Vec<&BtNodeData> = tree.nodes.values()
                .filter(|n| filter.matches(n, breakpoints))
                .collect();

            ui.label(format!("{} result(s):", results.len()));
            egui::ScrollArea::vertical().id_salt("adv_search_results").max_height(150.0).show(ui, |ui| {
                let mut to_select = None;
                for node in results {
                    let resp = show_node_card(ui, node, *selected == Some(node.id));
                    if resp.clicked() {
                        to_select = Some(node.id);
                    }
                }
                if let Some(id) = to_select {
                    *selected = Some(id);
                }
            });
        }
    });
}

// ============================================================
// DRAG-AND-DROP BETWEEN TREES
// ============================================================

#[derive(Debug, Clone)]
pub struct CrossTreeDragState {
    pub dragging: bool,
    pub source_tree_idx: usize,
    pub source_node_id: BtNodeId,
    pub current_screen_pos: Pos2,
    pub snapshot: Option<BtNodeData>,
}

impl CrossTreeDragState {
    pub fn new(source_tree: usize, node_id: BtNodeId, pos: Pos2, node: BtNodeData) -> Self {
        Self {
            dragging: true,
            source_tree_idx: source_tree,
            source_node_id: node_id,
            current_screen_pos: pos,
            snapshot: Some(node),
        }
    }
}

pub fn draw_cross_tree_drag_preview(painter: &Painter, state: &CrossTreeDragState, zoom: f32) {
    if !state.dragging { return; }
    let Some(ref node) = state.snapshot else { return; };

    let rect = Rect::from_center_size(
        state.current_screen_pos,
        Vec2::new(NODE_WIDTH * zoom * 0.7, NODE_HEADER_HEIGHT * zoom * 0.7),
    );

    painter.rect_filled(rect, egui::Rounding::same((4.0 * zoom) as u8),
        Color32::from_rgba_premultiplied(60, 60, 80, 180));
    painter.rect_stroke(rect, egui::Rounding::same((4.0 * zoom) as u8),
        Stroke::new(1.5 * zoom, node.node_type.category().color()),
        egui::StrokeKind::Outside);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        &node.label,
        FontId::proportional(10.0 * zoom),
        Color32::WHITE,
    );
}

// ============================================================
// EXECUTION PATH VISUALIZATION
// ============================================================

/// Given the execution log for a specific tick, compute which nodes were "active"
pub fn compute_active_path(
    log: &[(u32, BtNodeId, BtNodeStatus)],
    tick: u32,
) -> Vec<(BtNodeId, BtNodeStatus)> {
    log.iter()
        .filter(|(t, _, _)| *t == tick)
        .map(|(_, id, s)| (*id, s.clone()))
        .collect()
}

/// Highlight the execution path on the canvas by drawing colored overlays
pub fn draw_execution_path_overlay(
    painter: &Painter,
    tree: &BehaviorTree,
    active_path: &[(BtNodeId, BtNodeStatus)],
    canvas_offset: Vec2,
    zoom: f32,
) {
    for (node_id, status) in active_path {
        let Some(node) = tree.nodes.get(node_id) else { continue; };
        let screen_x = node.position.0 * zoom + canvas_offset.x;
        let screen_y = node.position.1 * zoom + canvas_offset.y;
        let h = node.compute_height() * zoom;
        let rect = Rect::from_min_size(
            Pos2::new(screen_x, screen_y),
            Vec2::new(NODE_WIDTH * zoom, h),
        );
        let color = status.color();
        painter.rect_stroke(
            rect.expand(3.0 * zoom),
            egui::Rounding::same(((NODE_CORNER_RADIUS + 3.0) * zoom) as u8),
            Stroke::new(2.0 * zoom, Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), 180)),
            egui::StrokeKind::Outside,
        );
    }
}

// ============================================================
// NODE COMMENT BUBBLE DRAWING
// ============================================================

pub fn draw_comment_bubble(
    painter: &Painter,
    pos: Pos2,
    text: &str,
    max_width: f32,
    zoom: f32,
) {
    if text.is_empty() { return; }
    let line_height = 12.0 * zoom;
    let padding = 6.0 * zoom;
    let chars_per_line = ((max_width - padding * 2.0) / (6.0 * zoom)) as usize;
    let lines_owned: Vec<String> = if text.len() <= chars_per_line {
        vec![text.to_string()]
    } else {
        text.split_whitespace()
            .fold(vec![String::new()], |mut acc, word| {
                let last = acc.last_mut().unwrap();
                if last.is_empty() {
                    *last = word.to_string();
                } else if last.len() + 1 + word.len() <= chars_per_line {
                    last.push(' ');
                    last.push_str(word);
                } else {
                    acc.push(word.to_string());
                }
                acc
            })
    };
    let lines: Vec<&str> = lines_owned.iter().map(|s| s.as_str()).collect();

    let bubble_h = lines.len() as f32 * line_height + padding * 2.0;
    let bubble_rect = Rect::from_min_size(
        Pos2::new(pos.x, pos.y - bubble_h - 8.0 * zoom),
        Vec2::new(max_width, bubble_h),
    );

    painter.rect_filled(bubble_rect, egui::Rounding::same((4.0 * zoom) as u8),
        Color32::from_rgba_premultiplied(40, 45, 55, 230));
    painter.rect_stroke(bubble_rect, egui::Rounding::same((4.0 * zoom) as u8),
        Stroke::new(1.0 * zoom, Color32::from_rgb(100, 120, 80)),
        egui::StrokeKind::Outside);

    // Tail
    let tail_base_y = bubble_rect.max.y;
    let tail_tip_y = pos.y;
    painter.add(Shape::convex_polygon(
        vec![
            Pos2::new(pos.x + 6.0 * zoom, tail_base_y),
            Pos2::new(pos.x + 14.0 * zoom, tail_base_y),
            Pos2::new(pos.x + 10.0 * zoom, tail_tip_y),
        ],
        Color32::from_rgba_premultiplied(40, 45, 55, 230),
        Stroke::NONE,
    ));

    for (i, line) in lines.iter().enumerate() {
        painter.text(
            Pos2::new(bubble_rect.min.x + padding, bubble_rect.min.y + padding + i as f32 * line_height + line_height * 0.5),
            Align2::LEFT_CENTER,
            line,
            FontId::proportional(9.0 * zoom),
            Color32::from_rgb(180, 200, 150),
        );
    }
}
