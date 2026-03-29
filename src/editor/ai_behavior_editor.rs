#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

const MAX_BEHAVIOR_TREE_DEPTH: usize = 64;
const MAX_BLACKBOARD_ENTRIES: usize = 512;
const GOAP_MAX_PLAN_STEPS: usize = 32;
const GOAP_MAX_OPEN_NODES: usize = 4096;
const UTILITY_MAX_CONSIDERATIONS: usize = 16;
const PERCEPTION_MAX_ENTITIES: usize = 256;
const FORMATION_MAX_AGENTS: usize = 128;
const STEERING_MAX_NEIGHBORS: usize = 64;
const FSM_MAX_STATES: usize = 128;
const FSM_MAX_TRANSITIONS: usize = 512;
const EMOTION_DECAY_RATE: f32 = 0.02;
const EMOTION_INFLUENCE_SCALE: f32 = 0.15;
const WANDER_CIRCLE_RADIUS: f32 = 1.2;
const WANDER_CIRCLE_DISTANCE: f32 = 2.0;
const WANDER_ANGLE_CHANGE: f32 = 0.4;
const ARRIVE_DECELERATION_RADIUS: f32 = 3.0;
const SEPARATION_WEIGHT: f32 = 1.5;
const ALIGNMENT_WEIGHT: f32 = 1.0;
const COHESION_WEIGHT: f32 = 1.0;
const LEADER_FOLLOW_DISTANCE: f32 = 2.5;
const QUEUE_MIN_DIST: f32 = 1.5;
const PI: f32 = std::f32::consts::PI;
const TWO_PI: f32 = 2.0 * PI;
const HALF_PI: f32 = PI / 2.0;
const SQRT2: f32 = std::f32::consts::SQRT_2;
const EPSILON: f32 = 1e-6;
const VISION_NEAR_PLANE: f32 = 0.1;
const HEARING_MIN_ATTENUATION: f32 = 0.01;
const SMELL_DIFFUSION_RATE: f32 = 0.005;
const BT_TICK_RATE_HZ: f32 = 30.0;
const REINGOLD_NODE_WIDTH: f32 = 120.0;
const REINGOLD_NODE_HEIGHT: f32 = 60.0;
const REINGOLD_H_SEPARATION: f32 = 20.0;
const REINGOLD_V_SEPARATION: f32 = 80.0;
const PLUTCHIK_EMOTIONS: usize = 8;
const PLUTCHIK_SECONDARY: usize = 8;

// ============================================================
// BLACKBOARD
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum BlackboardValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    String(String),
    EntityId(u64),
    None,
}

impl BlackboardValue {
    pub fn as_bool(&self) -> bool {
        match self {
            BlackboardValue::Bool(b) => *b,
            BlackboardValue::Int(i) => *i != 0,
            BlackboardValue::Float(f) => *f != 0.0,
            _ => false,
        }
    }
    pub fn as_float(&self) -> f32 {
        match self {
            BlackboardValue::Float(f) => *f,
            BlackboardValue::Int(i) => *i as f32,
            BlackboardValue::Bool(b) => if *b { 1.0 } else { 0.0 },
            _ => 0.0,
        }
    }
    pub fn as_int(&self) -> i64 {
        match self {
            BlackboardValue::Int(i) => *i,
            BlackboardValue::Float(f) => *f as i64,
            BlackboardValue::Bool(b) => if *b { 1 } else { 0 },
            _ => 0,
        }
    }
    pub fn as_vec3(&self) -> Vec3 {
        match self {
            BlackboardValue::Vec3(v) => *v,
            BlackboardValue::Vec2(v) => Vec3::new(v.x, v.y, 0.0),
            _ => Vec3::ZERO,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Blackboard {
    pub entries: HashMap<String, BlackboardValue>,
    pub change_timestamps: HashMap<String, f64>,
    pub current_time: f64,
}

impl Blackboard {
    pub fn new() -> Self {
        Self {
            entries: HashMap::with_capacity(64),
            change_timestamps: HashMap::with_capacity(64),
            current_time: 0.0,
        }
    }

    pub fn set(&mut self, key: &str, value: BlackboardValue) {
        self.entries.insert(key.to_string(), value);
        self.change_timestamps.insert(key.to_string(), self.current_time);
    }

    pub fn get(&self, key: &str) -> &BlackboardValue {
        self.entries.get(key).unwrap_or(&BlackboardValue::None)
    }

    pub fn get_bool(&self, key: &str) -> bool {
        self.get(key).as_bool()
    }

    pub fn get_float(&self, key: &str) -> f32 {
        self.get(key).as_float()
    }

    pub fn get_int(&self, key: &str) -> i64 {
        self.get(key).as_int()
    }

    pub fn get_vec3(&self, key: &str) -> Vec3 {
        self.get(key).as_vec3()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<BlackboardValue> {
        self.change_timestamps.remove(key);
        self.entries.remove(key)
    }

    pub fn age_of(&self, key: &str) -> f64 {
        self.change_timestamps.get(key)
            .map(|t| self.current_time - t)
            .unwrap_or(f64::MAX)
    }

    pub fn advance_time(&mut self, dt: f64) {
        self.current_time += dt;
    }
}

// ============================================================
// BEHAVIOR TREE — NODE STATUS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BtStatus {
    Success,
    Failure,
    Running,
    Invalid,
}

impl BtStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, BtStatus::Success | BtStatus::Failure)
    }
}

// ============================================================
// BEHAVIOR TREE — NODE TYPES
// ============================================================

#[derive(Clone, Debug)]
pub enum BtNodeType {
    // Composite
    Sequence,
    Selector,
    ParallelAll,       // succeed when ALL children succeed
    ParallelAny,       // succeed when ANY child succeeds
    RandomSelector,
    RandomSequence,
    // Decorators
    Inverter,
    Repeater { times: u32 },
    RepeatForever,
    RetryUntilSuccess { max_retries: u32 },
    Timeout { duration: f32 },
    Cooldown { cooldown: f32 },
    Succeeder,
    Failer,
    UntilFail,
    UntilSuccess,
    BlackboardCheck { key: String, op: CompareOp, value: BlackboardValue },
    BlackboardGuard { key: String },
    // Leaf — action
    MoveTo { target_key: String, speed: f32, acceptance_radius: f32 },
    MoveToPosition { position: Vec3, speed: f32, acceptance_radius: f32 },
    Attack { target_key: String, damage: f32, range: f32 },
    PlayAnimation { clip: String, layer: u32, blend_time: f32 },
    SetBlackboard { key: String, value: BlackboardValue },
    IncrementBlackboard { key: String, amount: f32 },
    Wait { duration: f32 },
    WaitBlackboard { key: String },
    Log { message: String },
    Idle,
    FindTarget { radius: f32, faction_key: String, result_key: String },
    Flee { threat_key: String, speed: f32, distance: f32 },
    Patrol { waypoints_key: String, speed: f32 },
    TakeCover { threat_key: String, result_key: String },
    AlertAllies { radius: f32, message: String },
    UseItem { item_key: String },
    PickupItem { item_key: String },
    DropItem { item_key: String },
    Interact { target_key: String, interaction_id: String },
    PlaySound { sound: String, volume: f32 },
    SpawnEntity { prefab: String, position_key: String },
    DestroyEntity { target_key: String },
    SendEvent { event_name: String, payload_key: String },
    FailAlways,
    SucceedAlways,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompareOp {
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
    Exists,
    NotExists,
}

impl CompareOp {
    pub fn evaluate(&self, lhs: &BlackboardValue, rhs: &BlackboardValue) -> bool {
        match self {
            CompareOp::Exists => !matches!(lhs, BlackboardValue::None),
            CompareOp::NotExists => matches!(lhs, BlackboardValue::None),
            CompareOp::Equal => lhs == rhs,
            CompareOp::NotEqual => lhs != rhs,
            CompareOp::LessThan => lhs.as_float() < rhs.as_float(),
            CompareOp::LessOrEqual => lhs.as_float() <= rhs.as_float(),
            CompareOp::GreaterThan => lhs.as_float() > rhs.as_float(),
            CompareOp::GreaterOrEqual => lhs.as_float() >= rhs.as_float(),
        }
    }
}

// ============================================================
// BEHAVIOR TREE — NODE
// ============================================================

#[derive(Clone, Debug)]
pub struct BtNode {
    pub id: u32,
    pub node_type: BtNodeType,
    pub children: Vec<u32>,
    pub parent: Option<u32>,
    pub status: BtStatus,
    // Runtime state
    pub current_child_index: usize,
    pub repeat_count: u32,
    pub elapsed_time: f32,
    pub cooldown_remaining: f32,
    pub last_run_time: f32,
    // Layout
    pub position: Vec2,
    pub size: Vec2,
    pub is_selected: bool,
    pub is_collapsed: bool,
    // Reingold-Tilford
    pub prelim: f32,
    pub modifier: f32,
    pub thread: Option<u32>,
    pub ancestor: Option<u32>,
    pub number: usize,
    pub change: f32,
    pub shift: f32,
}

impl BtNode {
    pub fn new(id: u32, node_type: BtNodeType) -> Self {
        Self {
            id,
            node_type,
            children: Vec::new(),
            parent: None,
            status: BtStatus::Invalid,
            current_child_index: 0,
            repeat_count: 0,
            elapsed_time: 0.0,
            cooldown_remaining: 0.0,
            last_run_time: 0.0,
            position: Vec2::ZERO,
            size: Vec2::new(REINGOLD_NODE_WIDTH, REINGOLD_NODE_HEIGHT),
            is_selected: false,
            is_collapsed: false,
            prelim: 0.0,
            modifier: 0.0,
            thread: None,
            ancestor: None,
            number: 0,
            change: 0.0,
            shift: 0.0,
        }
    }

    pub fn display_name(&self) -> &str {
        match &self.node_type {
            BtNodeType::Sequence => "Sequence",
            BtNodeType::Selector => "Selector",
            BtNodeType::ParallelAll => "Parallel(All)",
            BtNodeType::ParallelAny => "Parallel(Any)",
            BtNodeType::RandomSelector => "Random Selector",
            BtNodeType::RandomSequence => "Random Sequence",
            BtNodeType::Inverter => "Inverter",
            BtNodeType::Repeater { .. } => "Repeater",
            BtNodeType::RepeatForever => "Repeat Forever",
            BtNodeType::RetryUntilSuccess { .. } => "Retry Until Success",
            BtNodeType::Timeout { .. } => "Timeout",
            BtNodeType::Cooldown { .. } => "Cooldown",
            BtNodeType::Succeeder => "Succeeder",
            BtNodeType::Failer => "Failer",
            BtNodeType::UntilFail => "Until Fail",
            BtNodeType::UntilSuccess => "Until Success",
            BtNodeType::BlackboardCheck { .. } => "BB Check",
            BtNodeType::BlackboardGuard { .. } => "BB Guard",
            BtNodeType::MoveTo { .. } => "Move To",
            BtNodeType::MoveToPosition { .. } => "Move To Pos",
            BtNodeType::Attack { .. } => "Attack",
            BtNodeType::PlayAnimation { .. } => "Play Anim",
            BtNodeType::SetBlackboard { .. } => "Set BB",
            BtNodeType::IncrementBlackboard { .. } => "Inc BB",
            BtNodeType::Wait { .. } => "Wait",
            BtNodeType::WaitBlackboard { .. } => "Wait BB",
            BtNodeType::Log { .. } => "Log",
            BtNodeType::Idle => "Idle",
            BtNodeType::FindTarget { .. } => "Find Target",
            BtNodeType::Flee { .. } => "Flee",
            BtNodeType::Patrol { .. } => "Patrol",
            BtNodeType::TakeCover { .. } => "Take Cover",
            BtNodeType::AlertAllies { .. } => "Alert Allies",
            BtNodeType::UseItem { .. } => "Use Item",
            BtNodeType::PickupItem { .. } => "Pickup Item",
            BtNodeType::DropItem { .. } => "Drop Item",
            BtNodeType::Interact { .. } => "Interact",
            BtNodeType::PlaySound { .. } => "Play Sound",
            BtNodeType::SpawnEntity { .. } => "Spawn Entity",
            BtNodeType::DestroyEntity { .. } => "Destroy Entity",
            BtNodeType::SendEvent { .. } => "Send Event",
            BtNodeType::FailAlways => "Fail",
            BtNodeType::SucceedAlways => "Succeed",
        }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(
            &self.node_type,
            BtNodeType::MoveTo { .. }
            | BtNodeType::MoveToPosition { .. }
            | BtNodeType::Attack { .. }
            | BtNodeType::PlayAnimation { .. }
            | BtNodeType::SetBlackboard { .. }
            | BtNodeType::IncrementBlackboard { .. }
            | BtNodeType::Wait { .. }
            | BtNodeType::WaitBlackboard { .. }
            | BtNodeType::Log { .. }
            | BtNodeType::Idle
            | BtNodeType::FindTarget { .. }
            | BtNodeType::Flee { .. }
            | BtNodeType::Patrol { .. }
            | BtNodeType::TakeCover { .. }
            | BtNodeType::AlertAllies { .. }
            | BtNodeType::UseItem { .. }
            | BtNodeType::PickupItem { .. }
            | BtNodeType::DropItem { .. }
            | BtNodeType::Interact { .. }
            | BtNodeType::PlaySound { .. }
            | BtNodeType::SpawnEntity { .. }
            | BtNodeType::DestroyEntity { .. }
            | BtNodeType::SendEvent { .. }
            | BtNodeType::FailAlways
            | BtNodeType::SucceedAlways
        )
    }

    pub fn is_composite(&self) -> bool {
        matches!(
            &self.node_type,
            BtNodeType::Sequence
            | BtNodeType::Selector
            | BtNodeType::ParallelAll
            | BtNodeType::ParallelAny
            | BtNodeType::RandomSelector
            | BtNodeType::RandomSequence
        )
    }

    pub fn is_decorator(&self) -> bool {
        !self.is_leaf() && !self.is_composite()
    }
}

// ============================================================
// BEHAVIOR TREE — TICK CONTEXT
// ============================================================

#[derive(Debug)]
pub struct BtTickContext<'a> {
    pub blackboard: &'a mut Blackboard,
    pub delta_time: f32,
    pub current_time: f32,
    pub agent_position: Vec3,
    pub agent_velocity: Vec3,
    pub agent_id: u64,
    pub rng_seed: u64,
    pub debug_log: Vec<String>,
    pub visited_nodes: Vec<u32>,
}

impl<'a> BtTickContext<'a> {
    pub fn new(blackboard: &'a mut Blackboard, dt: f32, current_time: f32, agent_pos: Vec3, agent_id: u64) -> Self {
        Self {
            blackboard,
            delta_time: dt,
            current_time,
            agent_position: agent_pos,
            agent_velocity: Vec3::ZERO,
            agent_id,
            rng_seed: 12345 ^ (agent_id * 6364136223846793005),
            debug_log: Vec::new(),
            visited_nodes: Vec::new(),
        }
    }

    pub fn next_rand_f32(&mut self) -> f32 {
        // LCG random
        self.rng_seed = self.rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let bits = ((self.rng_seed >> 33) as u32) | 0x3F800000;
        let f = f32::from_bits(bits) - 1.0;
        f
    }

    pub fn next_rand_usize(&mut self, n: usize) -> usize {
        self.rng_seed = self.rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.rng_seed >> 33) as usize) % n.max(1)
    }
}

// ============================================================
// BEHAVIOR TREE — EXECUTOR
// ============================================================

pub struct BehaviorTree {
    pub nodes: HashMap<u32, BtNode>,
    pub root_id: Option<u32>,
    pub next_id: u32,
    pub name: String,
    pub tick_count: u64,
    pub last_status: BtStatus,
}

impl BehaviorTree {
    pub fn new(name: &str) -> Self {
        Self {
            nodes: HashMap::with_capacity(64),
            root_id: None,
            next_id: 1,
            name: name.to_string(),
            tick_count: 0,
            last_status: BtStatus::Invalid,
        }
    }

    pub fn add_node(&mut self, node_type: BtNodeType) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let node = BtNode::new(id, node_type);
        self.nodes.insert(id, node);
        id
    }

    pub fn set_root(&mut self, id: u32) {
        self.root_id = Some(id);
    }

    pub fn add_child(&mut self, parent_id: u32, child_id: u32) {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(child_id);
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
    }

    pub fn tick(&mut self, ctx: &mut BtTickContext) -> BtStatus {
        self.tick_count += 1;
        let root = match self.root_id {
            Some(id) => id,
            None => return BtStatus::Failure,
        };
        let status = self.tick_node(root, ctx, 0);
        self.last_status = status;
        status
    }

    fn tick_node(&mut self, node_id: u32, ctx: &mut BtTickContext, depth: usize) -> BtStatus {
        if depth >= MAX_BEHAVIOR_TREE_DEPTH {
            return BtStatus::Failure;
        }
        ctx.visited_nodes.push(node_id);

        // Clone node data needed for dispatch
        let (node_type, children, mut current_child_index, mut repeat_count, mut elapsed_time, mut cooldown_remaining) = {
            let node = match self.nodes.get(&node_id) {
                Some(n) => n,
                None => return BtStatus::Failure,
            };
            (
                node.node_type.clone(),
                node.children.clone(),
                node.current_child_index,
                node.repeat_count,
                node.elapsed_time,
                node.cooldown_remaining,
            )
        };

        elapsed_time += ctx.delta_time;
        cooldown_remaining = (cooldown_remaining - ctx.delta_time).max(0.0);

        let status = match &node_type {
            // ---- COMPOSITES ----
            BtNodeType::Sequence => {
                let mut result = BtStatus::Success;
                let mut new_child_idx = current_child_index;
                for i in current_child_index..children.len() {
                    let child_id = children[i];
                    let child_status = self.tick_node(child_id, ctx, depth + 1);
                    match child_status {
                        BtStatus::Failure => {
                            result = BtStatus::Failure;
                            new_child_idx = 0;
                            break;
                        }
                        BtStatus::Running => {
                            result = BtStatus::Running;
                            new_child_idx = i;
                            break;
                        }
                        BtStatus::Success => {
                            new_child_idx = i + 1;
                        }
                        BtStatus::Invalid => {
                            result = BtStatus::Failure;
                            new_child_idx = 0;
                            break;
                        }
                    }
                }
                if result == BtStatus::Success { new_child_idx = 0; }
                if let Some(n) = self.nodes.get_mut(&node_id) {
                    n.current_child_index = new_child_idx;
                    n.elapsed_time = elapsed_time;
                }
                result
            }

            BtNodeType::Selector => {
                let mut result = BtStatus::Failure;
                let mut new_child_idx = 0usize;
                for i in 0..children.len() {
                    let child_id = children[i];
                    let child_status = self.tick_node(child_id, ctx, depth + 1);
                    match child_status {
                        BtStatus::Success => {
                            result = BtStatus::Success;
                            new_child_idx = 0;
                            break;
                        }
                        BtStatus::Running => {
                            result = BtStatus::Running;
                            new_child_idx = i;
                            break;
                        }
                        BtStatus::Failure => {}
                        BtStatus::Invalid => {}
                    }
                }
                if let Some(n) = self.nodes.get_mut(&node_id) {
                    n.current_child_index = new_child_idx;
                    n.elapsed_time = elapsed_time;
                }
                result
            }

            BtNodeType::ParallelAll => {
                let mut all_success = true;
                let mut any_running = false;
                for &child_id in &children {
                    let child_status = self.tick_node(child_id, ctx, depth + 1);
                    match child_status {
                        BtStatus::Failure => { all_success = false; }
                        BtStatus::Running => { any_running = true; }
                        BtStatus::Success => {}
                        BtStatus::Invalid => { all_success = false; }
                    }
                }
                if let Some(n) = self.nodes.get_mut(&node_id) {
                    n.elapsed_time = elapsed_time;
                }
                if !all_success { BtStatus::Failure }
                else if any_running { BtStatus::Running }
                else { BtStatus::Success }
            }

            BtNodeType::ParallelAny => {
                let mut any_success = false;
                let mut any_running = false;
                for &child_id in &children {
                    let child_status = self.tick_node(child_id, ctx, depth + 1);
                    match child_status {
                        BtStatus::Success => { any_success = true; }
                        BtStatus::Running => { any_running = true; }
                        _ => {}
                    }
                }
                if let Some(n) = self.nodes.get_mut(&node_id) {
                    n.elapsed_time = elapsed_time;
                }
                if any_success { BtStatus::Success }
                else if any_running { BtStatus::Running }
                else { BtStatus::Failure }
            }

            BtNodeType::RandomSelector => {
                if children.is_empty() { return BtStatus::Failure; }
                // Fisher-Yates shuffle index list using ctx rng
                let mut indices: Vec<usize> = (0..children.len()).collect();
                for i in (1..indices.len()).rev() {
                    let j = ctx.next_rand_usize(i + 1);
                    indices.swap(i, j);
                }
                let mut result = BtStatus::Failure;
                for idx in indices {
                    let child_id = children[idx];
                    let s = self.tick_node(child_id, ctx, depth + 1);
                    if s == BtStatus::Success || s == BtStatus::Running {
                        result = s;
                        break;
                    }
                }
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                result
            }

            BtNodeType::RandomSequence => {
                if children.is_empty() { return BtStatus::Success; }
                let mut indices: Vec<usize> = (0..children.len()).collect();
                for i in (1..indices.len()).rev() {
                    let j = ctx.next_rand_usize(i + 1);
                    indices.swap(i, j);
                }
                let mut result = BtStatus::Success;
                for idx in indices {
                    let child_id = children[idx];
                    let s = self.tick_node(child_id, ctx, depth + 1);
                    if s == BtStatus::Failure || s == BtStatus::Running {
                        result = s;
                        break;
                    }
                }
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                result
            }

            // ---- DECORATORS ----
            BtNodeType::Inverter => {
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Failure };
                let s = self.tick_node(child_id, ctx, depth + 1);
                let result = match s {
                    BtStatus::Success => BtStatus::Failure,
                    BtStatus::Failure => BtStatus::Success,
                    other => other,
                };
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                result
            }

            BtNodeType::Succeeder => {
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Success };
                self.tick_node(child_id, ctx, depth + 1);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                BtStatus::Success
            }

            BtNodeType::Failer => {
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Failure };
                self.tick_node(child_id, ctx, depth + 1);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                BtStatus::Failure
            }

            BtNodeType::Repeater { times } => {
                let times = *times;
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Success };
                if repeat_count >= times {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.repeat_count = 0; n.elapsed_time = elapsed_time; }
                    return BtStatus::Success;
                }
                let s = self.tick_node(child_id, ctx, depth + 1);
                if s.is_terminal() {
                    repeat_count += 1;
                    if repeat_count >= times {
                        if let Some(n) = self.nodes.get_mut(&node_id) { n.repeat_count = 0; n.elapsed_time = elapsed_time; }
                        BtStatus::Success
                    } else {
                        if let Some(n) = self.nodes.get_mut(&node_id) { n.repeat_count = repeat_count; n.elapsed_time = elapsed_time; }
                        BtStatus::Running
                    }
                } else {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.repeat_count = repeat_count; n.elapsed_time = elapsed_time; }
                    BtStatus::Running
                }
            }

            BtNodeType::RepeatForever => {
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Running };
                self.tick_node(child_id, ctx, depth + 1);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                BtStatus::Running
            }

            BtNodeType::RetryUntilSuccess { max_retries } => {
                let max = *max_retries;
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Failure };
                let s = self.tick_node(child_id, ctx, depth + 1);
                match s {
                    BtStatus::Success => {
                        if let Some(n) = self.nodes.get_mut(&node_id) { n.repeat_count = 0; n.elapsed_time = elapsed_time; }
                        BtStatus::Success
                    }
                    BtStatus::Failure => {
                        let new_count = repeat_count + 1;
                        if new_count >= max {
                            if let Some(n) = self.nodes.get_mut(&node_id) { n.repeat_count = 0; n.elapsed_time = elapsed_time; }
                            BtStatus::Failure
                        } else {
                            if let Some(n) = self.nodes.get_mut(&node_id) { n.repeat_count = new_count; n.elapsed_time = elapsed_time; }
                            BtStatus::Running
                        }
                    }
                    other => {
                        if let Some(n) = self.nodes.get_mut(&node_id) { n.repeat_count = repeat_count; n.elapsed_time = elapsed_time; }
                        other
                    }
                }
            }

            BtNodeType::Timeout { duration } => {
                let dur = *duration;
                if elapsed_time > dur {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    return BtStatus::Failure;
                }
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Failure };
                let s = self.tick_node(child_id, ctx, depth + 1);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                s
            }

            BtNodeType::Cooldown { cooldown } => {
                let cd = *cooldown;
                if cooldown_remaining > 0.0 {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.cooldown_remaining = cooldown_remaining; }
                    return BtStatus::Failure;
                }
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Failure };
                let s = self.tick_node(child_id, ctx, depth + 1);
                if s == BtStatus::Success {
                    if let Some(n) = self.nodes.get_mut(&node_id) {
                        n.cooldown_remaining = cd;
                        n.elapsed_time = elapsed_time;
                    }
                } else {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                }
                s
            }

            BtNodeType::UntilFail => {
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Success };
                let s = self.tick_node(child_id, ctx, depth + 1);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                if s == BtStatus::Failure { BtStatus::Success } else { BtStatus::Running }
            }

            BtNodeType::UntilSuccess => {
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Failure };
                let s = self.tick_node(child_id, ctx, depth + 1);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                if s == BtStatus::Success { BtStatus::Success } else { BtStatus::Running }
            }

            BtNodeType::BlackboardCheck { key, op, value } => {
                let key = key.clone();
                let op = op.clone();
                let value = value.clone();
                let bb_val = ctx.blackboard.get(&key).clone();
                let result = op.evaluate(&bb_val, &value);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                if result { BtStatus::Success } else { BtStatus::Failure }
            }

            BtNodeType::BlackboardGuard { key } => {
                let key = key.clone();
                let exists = ctx.blackboard.contains(&key);
                if !exists {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                    return BtStatus::Failure;
                }
                let child_id = match children.first() { Some(&c) => c, None => return BtStatus::Failure };
                let s = self.tick_node(child_id, ctx, depth + 1);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                s
            }

            // ---- LEAF NODES ----
            BtNodeType::Wait { duration } => {
                let dur = *duration;
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                if elapsed_time >= dur {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    BtStatus::Running
                }
            }

            BtNodeType::WaitBlackboard { key } => {
                let key = key.clone();
                let dur = ctx.blackboard.get_float(&key);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                if elapsed_time >= dur {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    BtStatus::Running
                }
            }

            BtNodeType::Idle => {
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                BtStatus::Running
            }

            BtNodeType::FailAlways => BtStatus::Failure,
            BtNodeType::SucceedAlways => BtStatus::Success,

            BtNodeType::Log { message } => {
                ctx.debug_log.push(format!("[BT] {}", message));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                BtStatus::Success
            }

            BtNodeType::SetBlackboard { key, value } => {
                let key = key.clone();
                let value = value.clone();
                ctx.blackboard.set(&key, value);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                BtStatus::Success
            }

            BtNodeType::IncrementBlackboard { key, amount } => {
                let key = key.clone();
                let amount = *amount;
                let current = ctx.blackboard.get_float(&key);
                ctx.blackboard.set(&key, BlackboardValue::Float(current + amount));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                BtStatus::Success
            }

            BtNodeType::MoveTo { target_key, speed, acceptance_radius } => {
                let target_key = target_key.clone();
                let speed = *speed;
                let acceptance_radius = *acceptance_radius;
                let target = ctx.blackboard.get_vec3(&target_key);
                let diff = target - ctx.agent_position;
                let dist = diff.length();
                if dist <= acceptance_radius {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    let move_dist = speed * ctx.delta_time;
                    let dir = diff / dist;
                    let new_pos = ctx.agent_position + dir * move_dist.min(dist);
                    ctx.blackboard.set("agent_position", BlackboardValue::Vec3(new_pos));
                    ctx.agent_position = new_pos;
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                    BtStatus::Running
                }
            }

            BtNodeType::MoveToPosition { position, speed, acceptance_radius } => {
                let target = *position;
                let speed = *speed;
                let acceptance_radius = *acceptance_radius;
                let diff = target - ctx.agent_position;
                let dist = diff.length();
                if dist <= acceptance_radius {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    let dir = diff / dist;
                    let move_dist = speed * ctx.delta_time;
                    ctx.agent_position = ctx.agent_position + dir * move_dist.min(dist);
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                    BtStatus::Running
                }
            }

            BtNodeType::Attack { target_key, damage, range } => {
                let target_key = target_key.clone();
                let damage = *damage;
                let range = *range;
                let target_pos = ctx.blackboard.get_vec3(&target_key);
                let dist = (target_pos - ctx.agent_position).length();
                if dist <= range {
                    // Apply damage in blackboard
                    let key = format!("{}_health", target_key);
                    let current_hp = ctx.blackboard.get_float(&key);
                    ctx.blackboard.set(&key, BlackboardValue::Float(current_hp - damage));
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                    BtStatus::Failure
                }
            }

            BtNodeType::PlayAnimation { clip, layer, blend_time } => {
                ctx.blackboard.set("anim_clip", BlackboardValue::String(clip.clone()));
                ctx.blackboard.set("anim_layer", BlackboardValue::Int(*layer as i64));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }

            BtNodeType::FindTarget { radius, faction_key, result_key } => {
                let radius = *radius;
                let result_key = result_key.clone();
                let faction_key = faction_key.clone();
                // In a real system this would query a spatial index; here we check blackboard
                let nearest_key = format!("nearest_enemy_{}", faction_key);
                let found = ctx.blackboard.contains(&nearest_key);
                if found {
                    let val = ctx.blackboard.get(&nearest_key).clone();
                    ctx.blackboard.set(&result_key, val);
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                    BtStatus::Failure
                }
            }

            BtNodeType::Flee { threat_key, speed, distance } => {
                let threat_key = threat_key.clone();
                let speed = *speed;
                let distance = *distance;
                let threat_pos = ctx.blackboard.get_vec3(&threat_key);
                let diff = ctx.agent_position - threat_pos;
                let dist = diff.length();
                if dist >= distance {
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    let dir = if dist > EPSILON { diff / dist } else { Vec3::X };
                    ctx.agent_position = ctx.agent_position + dir * speed * ctx.delta_time;
                    ctx.blackboard.set("agent_position", BlackboardValue::Vec3(ctx.agent_position));
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                    BtStatus::Running
                }
            }

            BtNodeType::Patrol { waypoints_key, speed } => {
                let waypoints_key = waypoints_key.clone();
                let speed = *speed;
                // Waypoints stored as concatenated Vec3 in blackboard as array index
                let wp_index_key = format!("{}_index", waypoints_key);
                let mut wp_idx = ctx.blackboard.get_int(&wp_index_key) as usize;
                let wp_pos_key = format!("{}_{}", waypoints_key, wp_idx);
                if !ctx.blackboard.contains(&wp_pos_key) {
                    ctx.blackboard.set(&wp_index_key, BlackboardValue::Int(0));
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                    return BtStatus::Running;
                }
                let target = ctx.blackboard.get_vec3(&wp_pos_key);
                let diff = target - ctx.agent_position;
                let dist = diff.length();
                if dist < 0.5 {
                    let next_key = format!("{}_{}", waypoints_key, wp_idx + 1);
                    if ctx.blackboard.contains(&next_key) {
                        ctx.blackboard.set(&wp_index_key, BlackboardValue::Int((wp_idx + 1) as i64));
                    } else {
                        ctx.blackboard.set(&wp_index_key, BlackboardValue::Int(0));
                    }
                } else {
                    let dir = diff / dist;
                    ctx.agent_position = ctx.agent_position + dir * speed * ctx.delta_time;
                    ctx.blackboard.set("agent_position", BlackboardValue::Vec3(ctx.agent_position));
                }
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = elapsed_time; }
                BtStatus::Running
            }

            BtNodeType::TakeCover { threat_key, result_key } => {
                // Simple: find a point perpendicular to threat direction
                let threat_key = threat_key.clone();
                let result_key = result_key.clone();
                let threat_pos = ctx.blackboard.get_vec3(&threat_key);
                let to_threat = (threat_pos - ctx.agent_position).normalize_or_zero();
                let perp = Vec3::new(-to_threat.z, 0.0, to_threat.x);
                let cover_pos = ctx.agent_position + perp * 5.0;
                ctx.blackboard.set(&result_key, BlackboardValue::Vec3(cover_pos));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }

            BtNodeType::AlertAllies { radius, message } => {
                ctx.blackboard.set("alert_issued", BlackboardValue::Bool(true));
                ctx.blackboard.set("alert_message", BlackboardValue::String(message.clone()));
                ctx.blackboard.set("alert_radius", BlackboardValue::Float(*radius));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }

            BtNodeType::UseItem { item_key } => {
                let item_key = item_key.clone();
                if ctx.blackboard.contains(&item_key) {
                    ctx.blackboard.remove(&item_key);
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    BtStatus::Failure
                }
            }

            BtNodeType::PickupItem { item_key } => {
                let item_key = item_key.clone();
                let pos_key = format!("{}_pos", item_key);
                if !ctx.blackboard.contains(&pos_key) {
                    return BtStatus::Failure;
                }
                let item_pos = ctx.blackboard.get_vec3(&pos_key);
                let dist = (item_pos - ctx.agent_position).length();
                if dist < 1.5 {
                    ctx.blackboard.set(&item_key, BlackboardValue::Bool(true));
                    ctx.blackboard.remove(&pos_key);
                    if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                    BtStatus::Success
                } else {
                    BtStatus::Failure
                }
            }

            BtNodeType::DropItem { item_key } => {
                let item_key = item_key.clone();
                let drop_pos_key = format!("{}_pos", item_key);
                ctx.blackboard.set(&drop_pos_key, BlackboardValue::Vec3(ctx.agent_position));
                ctx.blackboard.remove(&item_key);
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }

            BtNodeType::Interact { target_key, interaction_id } => {
                let result_key = format!("interact_result_{}", interaction_id);
                ctx.blackboard.set(&result_key, BlackboardValue::Bool(true));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }

            BtNodeType::PlaySound { sound, volume } => {
                ctx.blackboard.set("sound_playing", BlackboardValue::String(sound.clone()));
                ctx.blackboard.set("sound_volume", BlackboardValue::Float(*volume));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }

            BtNodeType::SpawnEntity { prefab, position_key } => {
                let pos_key = position_key.clone();
                let prefab = prefab.clone();
                let spawn_pos = ctx.blackboard.get_vec3(&pos_key);
                ctx.blackboard.set("last_spawned_prefab", BlackboardValue::String(prefab));
                ctx.blackboard.set("last_spawned_pos", BlackboardValue::Vec3(spawn_pos));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }

            BtNodeType::DestroyEntity { target_key } => {
                let key = target_key.clone();
                ctx.blackboard.set(&format!("{}_destroyed", key), BlackboardValue::Bool(true));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }

            BtNodeType::SendEvent { event_name, payload_key } => {
                ctx.blackboard.set("pending_event", BlackboardValue::String(event_name.clone()));
                if let Some(n) = self.nodes.get_mut(&node_id) { n.elapsed_time = 0.0; }
                BtStatus::Success
            }
        };

        if let Some(n) = self.nodes.get_mut(&node_id) {
            n.status = status;
        }
        status
    }

    pub fn reset(&mut self) {
        for node in self.nodes.values_mut() {
            node.status = BtStatus::Invalid;
            node.current_child_index = 0;
            node.repeat_count = 0;
            node.elapsed_time = 0.0;
        }
    }
}

// ============================================================
// REINGOLD-TILFORD LAYOUT ALGORITHM
// ============================================================

pub struct ReingoldTilford {
    pub contours: HashMap<u32, f32>,
}

impl ReingoldTilford {
    pub fn new() -> Self {
        Self { contours: HashMap::new() }
    }

    pub fn layout(&mut self, tree: &mut BehaviorTree) {
        if let Some(root_id) = tree.root_id {
            // First pass
            let depth = 0;
            let siblings_count = 1;
            self.first_walk(tree, root_id, 0, 0);
            // Second pass
            let root_prelim = tree.nodes.get(&root_id).map(|n| n.prelim).unwrap_or(0.0);
            self.second_walk(tree, root_id, -root_prelim, 0);
        }
    }

    fn first_walk(&mut self, tree: &mut BehaviorTree, node_id: u32, sibling_index: usize, depth: usize) {
        if depth >= MAX_BEHAVIOR_TREE_DEPTH { return; }

        let children = tree.nodes.get(&node_id).map(|n| n.children.clone()).unwrap_or_default();

        if children.is_empty() {
            // Leaf node
            let prelim = if sibling_index == 0 {
                0.0
            } else {
                // Find previous sibling
                let parent_id = tree.nodes.get(&node_id).and_then(|n| n.parent);
                if let Some(pid) = parent_id {
                    let siblings = tree.nodes.get(&pid).map(|n| n.children.clone()).unwrap_or_default();
                    if sibling_index > 0 {
                        let prev_id = siblings[sibling_index - 1];
                        let prev_prelim = tree.nodes.get(&prev_id).map(|n| n.prelim).unwrap_or(0.0);
                        prev_prelim + REINGOLD_NODE_WIDTH + REINGOLD_H_SEPARATION
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            };
            if let Some(n) = tree.nodes.get_mut(&node_id) {
                n.prelim = prelim;
                n.modifier = 0.0;
                n.number = sibling_index;
            }
        } else {
            // Internal node — recurse first
            for (i, &child_id) in children.iter().enumerate() {
                self.first_walk(tree, child_id, i, depth + 1);
            }

            // Apportion
            let children2 = tree.nodes.get(&node_id).map(|n| n.children.clone()).unwrap_or_default();
            self.apportion(tree, node_id);

            // Place node between first and last child
            let first_child = children2[0];
            let last_child = children2[children2.len() - 1];
            let fc_prelim = tree.nodes.get(&first_child).map(|n| n.prelim).unwrap_or(0.0);
            let lc_prelim = tree.nodes.get(&last_child).map(|n| n.prelim).unwrap_or(0.0);
            let mid_point = (fc_prelim + lc_prelim) / 2.0;

            let parent_id = tree.nodes.get(&node_id).and_then(|n| n.parent);

            if sibling_index == 0 {
                if let Some(n) = tree.nodes.get_mut(&node_id) {
                    n.prelim = mid_point;
                    n.modifier = 0.0;
                    n.number = sibling_index;
                }
            } else {
                if let Some(pid) = parent_id {
                    let siblings = tree.nodes.get(&pid).map(|n| n.children.clone()).unwrap_or_default();
                    if sibling_index > 0 {
                        let prev_id = siblings[sibling_index - 1];
                        let prev_prelim = tree.nodes.get(&prev_id).map(|n| n.prelim).unwrap_or(0.0);
                        let prelim = prev_prelim + REINGOLD_NODE_WIDTH + REINGOLD_H_SEPARATION;
                        let modifier = prelim - mid_point;
                        if let Some(n) = tree.nodes.get_mut(&node_id) {
                            n.prelim = prelim;
                            n.modifier = modifier;
                            n.number = sibling_index;
                        }
                    }
                }
            }
        }
    }

    fn apportion(&mut self, tree: &mut BehaviorTree, node_id: u32) {
        let children = tree.nodes.get(&node_id).map(|n| n.children.clone()).unwrap_or_default();
        if children.len() < 2 { return; }

        for i in 1..children.len() {
            let child_id = children[i];
            let prev_id = children[i - 1];
            let child_prelim = tree.nodes.get(&child_id).map(|n| n.prelim).unwrap_or(0.0);
            let prev_prelim = tree.nodes.get(&prev_id).map(|n| n.prelim).unwrap_or(0.0);
            let gap = child_prelim - prev_prelim - (REINGOLD_NODE_WIDTH + REINGOLD_H_SEPARATION);
            if gap < 0.0 {
                // Shift right subtree
                self.shift_subtree(tree, child_id, -gap);
            }
        }
    }

    fn shift_subtree(&mut self, tree: &mut BehaviorTree, node_id: u32, shift: f32) {
        if let Some(n) = tree.nodes.get_mut(&node_id) {
            n.prelim += shift;
            n.modifier += shift;
        }
        let children = tree.nodes.get(&node_id).map(|n| n.children.clone()).unwrap_or_default();
        for child_id in children {
            self.shift_subtree(tree, child_id, shift);
        }
    }

    fn second_walk(&mut self, tree: &mut BehaviorTree, node_id: u32, mod_sum: f32, depth: usize) {
        if depth >= MAX_BEHAVIOR_TREE_DEPTH { return; }
        let (prelim, modifier, children) = {
            let n = match tree.nodes.get(&node_id) { Some(n) => n, None => return };
            (n.prelim, n.modifier, n.children.clone())
        };
        let x = prelim + mod_sum;
        let y = depth as f32 * (REINGOLD_NODE_HEIGHT + REINGOLD_V_SEPARATION);
        if let Some(n) = tree.nodes.get_mut(&node_id) {
            n.position = Vec2::new(x, y);
        }
        for child_id in children {
            self.second_walk(tree, child_id, mod_sum + modifier, depth + 1);
        }
    }
}

// ============================================================
// GOAP — WORLD STATE
// ============================================================

pub type WorldState = u64; // bitmask of facts

#[derive(Clone, Debug)]
pub struct GoapAction {
    pub id: u32,
    pub name: String,
    pub preconditions: WorldState,   // required bits set
    pub preconditions_false: WorldState, // required bits clear
    pub effects_set: WorldState,     // bits to set
    pub effects_clear: WorldState,   // bits to clear
    pub cost: f32,
    pub duration: f32,
    pub cooldown: f32,
    pub last_used: f32,
}

impl GoapAction {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            preconditions: 0,
            preconditions_false: 0,
            effects_set: 0,
            effects_clear: 0,
            cost: 1.0,
            duration: 1.0,
            cooldown: 0.0,
            last_used: -999.0,
        }
    }

    pub fn can_execute(&self, world: WorldState, current_time: f32) -> bool {
        let prec_met = (world & self.preconditions) == self.preconditions;
        let false_prec_met = (world & self.preconditions_false) == 0;
        let cd_ok = (current_time - self.last_used) >= self.cooldown;
        prec_met && false_prec_met && cd_ok
    }

    pub fn apply(&self, world: WorldState) -> WorldState {
        (world | self.effects_set) & !self.effects_clear
    }
}

// ============================================================
// GOAP — A* PLANNER
// ============================================================

#[derive(Clone, Debug)]
struct GoapNode {
    pub world_state: WorldState,
    pub g: f32,
    pub h: f32,
    pub action_index: Option<usize>,
    pub parent_index: Option<usize>,
}

impl GoapNode {
    pub fn f(&self) -> f32 { self.g + self.h }
}

pub struct GoapPlanner {
    pub actions: Vec<GoapAction>,
    pub world_state_labels: HashMap<u8, String>,
}

impl GoapPlanner {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            world_state_labels: HashMap::new(),
        }
    }

    pub fn add_action(&mut self, action: GoapAction) {
        self.actions.push(action);
    }

    pub fn label_bit(&mut self, bit: u8, label: &str) {
        self.world_state_labels.insert(bit, label.to_string());
    }

    /// Heuristic: count number of goal bits not yet set
    fn heuristic(state: WorldState, goal: WorldState) -> f32 {
        let unsatisfied = goal & !state;
        unsatisfied.count_ones() as f32
    }

    pub fn plan(
        &self,
        start: WorldState,
        goal: WorldState,
        current_time: f32,
    ) -> Option<Vec<usize>> {
        // A* search over world states
        let mut open: Vec<GoapNode> = Vec::with_capacity(64);
        let mut closed: Vec<GoapNode> = Vec::with_capacity(64);

        let h0 = Self::heuristic(start, goal);
        open.push(GoapNode {
            world_state: start,
            g: 0.0,
            h: h0,
            action_index: None,
            parent_index: None,
        });

        let mut iterations = 0;
        while !open.is_empty() && iterations < GOAP_MAX_OPEN_NODES {
            iterations += 1;

            // Find lowest f
            let mut best_idx = 0;
            for i in 1..open.len() {
                if open[i].f() < open[best_idx].f() {
                    best_idx = i;
                }
            }
            let current = open.remove(best_idx);

            // Check if goal satisfied
            if (current.world_state & goal) == goal {
                // Reconstruct path
                let mut plan: Vec<usize> = Vec::new();
                let mut node = &closed[closed.len() - 1]; // will push current first
                // Push current to closed temporarily to allow backtrack
                closed.push(current.clone());
                let mut idx = closed.len() - 1;
                loop {
                    if let Some(action_idx) = closed[idx].action_index {
                        plan.push(action_idx);
                    }
                    if let Some(parent_idx) = closed[idx].parent_index {
                        idx = parent_idx;
                    } else {
                        break;
                    }
                }
                plan.reverse();
                return Some(plan);
            }

            let current_idx = closed.len();
            closed.push(current.clone());

            if closed.len() > GOAP_MAX_PLAN_STEPS * 10 { break; }

            // Expand
            for (action_idx, action) in self.actions.iter().enumerate() {
                if !action.can_execute(current.world_state, current_time) { continue; }
                let new_state = action.apply(current.world_state);
                // Check if already in closed
                let in_closed = closed.iter().any(|n| n.world_state == new_state);
                if in_closed { continue; }

                let new_g = current.g + action.cost;
                let new_h = Self::heuristic(new_state, goal);

                // Check if already in open with lower cost
                let existing = open.iter().enumerate().find(|(_, n)| n.world_state == new_state);
                if let Some((oi, existing_node)) = existing {
                    if new_g < existing_node.g {
                        open[oi].g = new_g;
                        open[oi].action_index = Some(action_idx);
                        open[oi].parent_index = Some(current_idx);
                    }
                } else {
                    open.push(GoapNode {
                        world_state: new_state,
                        g: new_g,
                        h: new_h,
                        action_index: Some(action_idx),
                        parent_index: Some(current_idx),
                    });
                }
            }
        }
        None
    }

    pub fn world_state_description(&self, state: WorldState) -> String {
        let mut parts = Vec::new();
        for bit in 0..64u8 {
            if (state >> bit) & 1 == 1 {
                if let Some(label) = self.world_state_labels.get(&bit) {
                    parts.push(label.clone());
                } else {
                    parts.push(format!("bit{}", bit));
                }
            }
        }
        parts.join(", ")
    }
}

// ============================================================
// UTILITY AI — RESPONSE CURVES
// ============================================================

#[derive(Clone, Debug)]
pub enum ResponseCurve {
    Linear { slope: f32, intercept: f32 },
    Exponential { base: f32, exponent: f32, scale: f32 },
    Logistic { steepness: f32, midpoint: f32 },
    Sine { frequency: f32, phase: f32, amplitude: f32, offset: f32 },
    Polynomial { coefficients: Vec<f32> },
    Inverse { scale: f32 },
    Step { threshold: f32, low: f32, high: f32 },
    Smoothstep { edge0: f32, edge1: f32 },
    Bell { center: f32, width: f32 },
    Constant { value: f32 },
}

impl ResponseCurve {
    pub fn evaluate(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            ResponseCurve::Linear { slope, intercept } => {
                (slope * x + intercept).clamp(0.0, 1.0)
            }
            ResponseCurve::Exponential { base, exponent, scale } => {
                let v = base.powf(x * exponent) * scale;
                v.clamp(0.0, 1.0)
            }
            ResponseCurve::Logistic { steepness, midpoint } => {
                let e = std::f32::consts::E;
                let v = 1.0 / (1.0 + e.powf(-steepness * (x - midpoint)));
                v.clamp(0.0, 1.0)
            }
            ResponseCurve::Sine { frequency, phase, amplitude, offset } => {
                let v = amplitude * (frequency * x * TWO_PI + phase).sin() + offset;
                v.clamp(0.0, 1.0)
            }
            ResponseCurve::Polynomial { coefficients } => {
                // Evaluate polynomial using Horner's method
                let mut result = 0.0f32;
                for &c in coefficients.iter().rev() {
                    result = result * x + c;
                }
                result.clamp(0.0, 1.0)
            }
            ResponseCurve::Inverse { scale } => {
                if x.abs() < EPSILON { 1.0 }
                else { (scale / x).clamp(0.0, 1.0) }
            }
            ResponseCurve::Step { threshold, low, high } => {
                if x >= *threshold { *high } else { *low }
            }
            ResponseCurve::Smoothstep { edge0, edge1 } => {
                let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
                (t * t * (3.0 - 2.0 * t)).clamp(0.0, 1.0)
            }
            ResponseCurve::Bell { center, width } => {
                let d = (x - center) / (width + EPSILON);
                let v = (-d * d * 2.0).exp();
                v.clamp(0.0, 1.0)
            }
            ResponseCurve::Constant { value } => value.clamp(0.0, 1.0),
        }
    }

    pub fn sample_points(&self, n: usize) -> Vec<Vec2> {
        (0..n).map(|i| {
            let x = i as f32 / (n - 1).max(1) as f32;
            Vec2::new(x, self.evaluate(x))
        }).collect()
    }
}

// ============================================================
// UTILITY AI — CONSIDERATION
// ============================================================

#[derive(Clone, Debug)]
pub struct Consideration {
    pub name: String,
    pub input_key: String,       // blackboard key
    pub input_min: f32,
    pub input_max: f32,
    pub curve: ResponseCurve,
    pub weight: f32,
}

impl Consideration {
    pub fn new(name: &str, input_key: &str, curve: ResponseCurve) -> Self {
        Self {
            name: name.to_string(),
            input_key: input_key.to_string(),
            input_min: 0.0,
            input_max: 1.0,
            curve,
            weight: 1.0,
        }
    }

    pub fn evaluate(&self, blackboard: &Blackboard) -> f32 {
        let raw = blackboard.get_float(&self.input_key);
        let range = self.input_max - self.input_min;
        let normalized = if range.abs() > EPSILON {
            ((raw - self.input_min) / range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.curve.evaluate(normalized) * self.weight
    }
}

// ============================================================
// UTILITY AI — UTILITY ACTION
// ============================================================

#[derive(Clone, Debug)]
pub struct UtilityAction {
    pub id: u32,
    pub name: String,
    pub considerations: Vec<Consideration>,
    pub bonus_score: f32,
    pub cooldown: f32,
    pub last_selected_time: f32,
    pub momentum: f32,     // inertia factor to prevent thrashing
    pub is_active: bool,
}

impl UtilityAction {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            considerations: Vec::new(),
            bonus_score: 0.0,
            cooldown: 0.0,
            last_selected_time: -999.0,
            momentum: 0.0,
            is_active: false,
        }
    }

    pub fn score(&self, blackboard: &Blackboard, current_time: f32) -> f32 {
        if (current_time - self.last_selected_time) < self.cooldown {
            return 0.0;
        }
        if self.considerations.is_empty() {
            return self.bonus_score;
        }
        // Geometric mean of all considerations (avoids all-or-nothing bias)
        let n = self.considerations.len() as f32;
        let mut product = 1.0f32;
        for c in &self.considerations {
            let v = c.evaluate(blackboard);
            product *= v;
        }
        // Compensation factor: geometric mean normalization
        let avg = product.powf(1.0 / n);
        // Modification factor makes the score approach arithmetic mean as considerations grow
        let modification_factor = 1.0 - (1.0 / n);
        let final_score = avg + (avg * modification_factor * (1.0 - avg));
        (final_score + self.bonus_score + if self.is_active { self.momentum } else { 0.0 }).clamp(0.0, 1.0)
    }
}

// ============================================================
// UTILITY AI — DECISION MAKER
// ============================================================

pub struct UtilityDecisionMaker {
    pub actions: Vec<UtilityAction>,
    pub selected_action_id: Option<u32>,
    pub selection_history: VecDeque<(u32, f32)>,   // (action_id, time)
    pub evaluation_frequency: f32,
    pub last_evaluation: f32,
    pub score_threshold: f32,
}

impl UtilityDecisionMaker {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
            selected_action_id: None,
            selection_history: VecDeque::with_capacity(32),
            evaluation_frequency: 0.1,
            last_evaluation: 0.0,
            score_threshold: 0.05,
        }
    }

    pub fn add_action(&mut self, action: UtilityAction) {
        self.actions.push(action);
    }

    pub fn evaluate(&mut self, blackboard: &Blackboard, current_time: f32) -> Option<u32> {
        if current_time - self.last_evaluation < self.evaluation_frequency {
            return self.selected_action_id;
        }
        self.last_evaluation = current_time;

        let mut best_id = None;
        let mut best_score = self.score_threshold;

        for action in &self.actions {
            let score = action.score(blackboard, current_time);
            if score > best_score {
                best_score = score;
                best_id = Some(action.id);
            }
        }

        // Update active states
        for action in &mut self.actions {
            action.is_active = Some(action.id) == best_id;
        }

        if let Some(id) = best_id {
            if Some(id) != self.selected_action_id {
                if let Some(a) = self.actions.iter_mut().find(|a| a.id == id) {
                    a.last_selected_time = current_time;
                }
                self.selection_history.push_back((id, current_time));
                if self.selection_history.len() > 32 {
                    self.selection_history.pop_front();
                }
                self.selected_action_id = Some(id);
            }
        }
        self.selected_action_id
    }
}

// ============================================================
// PERCEPTION SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct PerceivedEntity {
    pub entity_id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub last_seen_time: f32,
    pub last_known_position: Vec3,
    pub confidence: f32,    // 0..1
    pub threat_level: f32,
    pub is_visible: bool,
    pub is_heard: bool,
    pub is_smelled: bool,
}

impl PerceivedEntity {
    pub fn new(entity_id: u64, position: Vec3) -> Self {
        Self {
            entity_id,
            position,
            velocity: Vec3::ZERO,
            last_seen_time: 0.0,
            last_known_position: position,
            confidence: 1.0,
            threat_level: 0.0,
            is_visible: false,
            is_heard: false,
            is_smelled: false,
        }
    }

    pub fn update_position(&mut self, pos: Vec3, vel: Vec3, time: f32) {
        self.position = pos;
        self.velocity = vel;
        self.last_seen_time = time;
        self.last_known_position = pos;
        self.confidence = 1.0;
    }

    pub fn decay_confidence(&mut self, dt: f32, decay_rate: f32) {
        self.confidence = (self.confidence - decay_rate * dt).max(0.0);
        // Predict position based on last known velocity
        self.last_known_position = self.last_known_position + self.velocity * dt;
        // Slow velocity decay (entity might stop)
        self.velocity *= (1.0 - dt * 0.5).max(0.0);
    }
}

#[derive(Clone, Debug)]
pub struct VisionConfig {
    pub range: f32,
    pub half_angle: f32,         // radians
    pub near_range: f32,         // always sees within this range regardless of angle
    pub darkness_penalty: f32,   // 0 = full dark, 1 = full light
    pub moving_target_bonus: f32,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            range: 20.0,
            half_angle: PI / 3.0,    // 120-degree FOV
            near_range: 1.5,
            darkness_penalty: 1.0,
            moving_target_bonus: 0.2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HearingConfig {
    pub base_radius: f32,
    pub frequency_response: f32,   // Hz filtering analog
    pub noise_floor: f32,
}

impl Default for HearingConfig {
    fn default() -> Self {
        Self {
            base_radius: 15.0,
            frequency_response: 1.0,
            noise_floor: 0.1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SmellConfig {
    pub base_radius: f32,
    pub wind_direction: Vec3,
    pub wind_speed: f32,
    pub min_intensity: f32,
}

impl Default for SmellConfig {
    fn default() -> Self {
        Self {
            base_radius: 8.0,
            wind_direction: Vec3::new(1.0, 0.0, 0.0),
            wind_speed: 1.0,
            min_intensity: 0.05,
        }
    }
}

pub struct PerceptionSystem {
    pub vision: VisionConfig,
    pub hearing: HearingConfig,
    pub smell: SmellConfig,
    pub perceived: HashMap<u64, PerceivedEntity>,
    pub confidence_decay: f32,
    pub forget_threshold: f32,
    pub observer_id: u64,
}

impl PerceptionSystem {
    pub fn new(observer_id: u64) -> Self {
        Self {
            vision: VisionConfig::default(),
            hearing: HearingConfig::default(),
            smell: SmellConfig::default(),
            perceived: HashMap::new(),
            confidence_decay: 0.1,
            forget_threshold: 0.05,
            observer_id,
        }
    }

    /// Vision cone check with distance falloff
    pub fn can_see(
        &self,
        observer_pos: Vec3,
        observer_forward: Vec3,
        target_pos: Vec3,
        target_velocity: Vec3,
        obstacles: &[Aabb],
    ) -> (bool, f32) {
        let to_target = target_pos - observer_pos;
        let dist = to_target.length();

        if dist < VISION_NEAR_PLANE { return (true, 1.0); }

        // Near range always visible
        if dist <= self.vision.near_range { return (true, 1.0); }

        if dist > self.vision.range { return (false, 0.0); }

        // Angle check
        let to_target_norm = to_target / dist;
        let fwd = observer_forward.normalize_or_zero();
        let dot = fwd.dot(to_target_norm);
        let angle = dot.clamp(-1.0, 1.0).acos();

        if angle > self.vision.half_angle { return (false, 0.0); }

        // Distance falloff: linear from range_start to range
        let range_start = self.vision.range * 0.3;
        let dist_factor = if dist < range_start { 1.0 }
            else { 1.0 - (dist - range_start) / (self.vision.range - range_start) };

        // Angle falloff: cos falloff from center to edge of cone
        let angle_factor = 1.0 - (angle / self.vision.half_angle);

        // Moving target bonus
        let vel_factor = 1.0 + (target_velocity.length().min(5.0) / 5.0) * self.vision.moving_target_bonus;

        // Darkness penalty
        let light_factor = self.vision.darkness_penalty;

        // Occlusion: raycast against obstacles
        let occluded = self.raycast_occluded(observer_pos, target_pos, obstacles);
        if occluded { return (false, 0.0); }

        let confidence = (dist_factor * angle_factor * vel_factor * light_factor).clamp(0.0, 1.0);
        (confidence > 0.1, confidence)
    }

    /// Simple AABB ray intersection for occlusion
    fn raycast_occluded(&self, from: Vec3, to: Vec3, obstacles: &[Aabb]) -> bool {
        let dir = to - from;
        let len = dir.length();
        if len < EPSILON { return false; }
        let inv_dir = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        for obs in obstacles {
            if obs.ray_intersects(from, inv_dir, len) {
                return true;
            }
        }
        false
    }

    /// Hearing: sound attenuation model
    pub fn can_hear(&self, observer_pos: Vec3, source_pos: Vec3, sound_intensity: f32) -> (bool, f32) {
        let dist = (source_pos - observer_pos).length();
        if dist < EPSILON { return (true, 1.0); }

        // Inverse square law with min attenuation
        let attenuation = (sound_intensity / (1.0 + dist * dist * 0.1)).max(0.0);

        if attenuation < self.hearing.noise_floor { return (false, 0.0); }

        let max_dist = self.hearing.base_radius * (sound_intensity / 1.0).sqrt();
        if dist > max_dist { return (false, 0.0); }

        let confidence = (attenuation / sound_intensity).clamp(0.0, 1.0);
        (true, confidence)
    }

    /// Smell: wind-adjusted radius
    pub fn can_smell(&self, observer_pos: Vec3, source_pos: Vec3, smell_intensity: f32) -> (bool, f32) {
        let to_source = source_pos - observer_pos;
        let dist = to_source.length();
        if dist < EPSILON { return (true, 1.0); }

        // Wind direction shifts the detectable range
        let wind_dot = self.smell.wind_direction.normalize_or_zero().dot(to_source / dist);
        // Downwind multiplier: 1.5x range downwind, 0.5x range upwind
        let wind_factor = 1.0 + wind_dot * 0.5;
        let effective_radius = self.smell.base_radius * wind_factor * smell_intensity;

        if dist > effective_radius { return (false, 0.0); }

        // Smell intensity: exponential falloff
        let normalized = 1.0 - (dist / effective_radius);
        let confidence = normalized * normalized * smell_intensity;
        (confidence > self.smell.min_intensity, confidence)
    }

    pub fn update(
        &mut self,
        observer_pos: Vec3,
        observer_forward: Vec3,
        candidates: &[(u64, Vec3, Vec3, f32, f32, f32)],  // (id, pos, vel, sound_int, smell_int, threat)
        dt: f32,
        current_time: f32,
        obstacles: &[Aabb],
    ) {
        // Decay existing perceptions
        let mut to_forget: Vec<u64> = Vec::new();
        for (id, p) in self.perceived.iter_mut() {
            p.decay_confidence(dt, self.confidence_decay);
            if p.confidence < self.forget_threshold {
                to_forget.push(*id);
            }
        }
        for id in to_forget {
            self.perceived.remove(&id);
        }

        // Check new candidates
        for &(id, pos, vel, sound, smell, threat) in candidates {
            if id == self.observer_id { continue; }

            let (vis, vis_conf) = self.can_see(observer_pos, observer_forward, pos, vel, obstacles);
            let (hrd, hrd_conf) = self.can_hear(observer_pos, pos, sound);
            let (sml, sml_conf) = self.can_smell(observer_pos, pos, smell);

            if vis || hrd || sml {
                let max_conf = vis_conf.max(hrd_conf).max(sml_conf);
                let entry = self.perceived.entry(id).or_insert_with(|| PerceivedEntity::new(id, pos));
                entry.is_visible = vis;
                entry.is_heard = hrd;
                entry.is_smelled = sml;
                entry.threat_level = threat;
                if vis {
                    entry.update_position(pos, vel, current_time);
                } else {
                    entry.confidence = entry.confidence.max(max_conf);
                }
            }
        }
    }

    pub fn most_threatening(&self) -> Option<&PerceivedEntity> {
        self.perceived.values()
            .filter(|p| p.confidence > 0.2)
            .max_by(|a, b| (a.threat_level * a.confidence)
                .partial_cmp(&(b.threat_level * b.confidence)).unwrap())
    }

    pub fn nearest_visible(&self, observer_pos: Vec3) -> Option<&PerceivedEntity> {
        self.perceived.values()
            .filter(|p| p.is_visible)
            .min_by(|a, b| {
                let da = (a.position - observer_pos).length_squared();
                let db = (b.position - observer_pos).length_squared();
                da.partial_cmp(&db).unwrap()
            })
    }
}

// ============================================================
// AABB (used by perception and steering)
// ============================================================

#[derive(Clone, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(center: Vec3, half_extents: Vec3) -> Self {
        Self { min: center - half_extents, max: center + half_extents }
    }

    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x && p.x <= self.max.x &&
        p.y >= self.min.y && p.y <= self.max.y &&
        p.z >= self.min.z && p.z <= self.max.z
    }

    pub fn center(&self) -> Vec3 { (self.min + self.max) * 0.5 }
    pub fn half_extents(&self) -> Vec3 { (self.max - self.min) * 0.5 }

    pub fn ray_intersects(&self, origin: Vec3, inv_dir: Vec3, max_t: f32) -> bool {
        let t1 = (self.min - origin) * inv_dir;
        let t2 = (self.max - origin) * inv_dir;
        let t_min_v = Vec3::new(t1.x.min(t2.x), t1.y.min(t2.y), t1.z.min(t2.z));
        let t_max_v = Vec3::new(t1.x.max(t2.x), t1.y.max(t2.y), t1.z.max(t2.z));
        let t_enter = t_min_v.x.max(t_min_v.y).max(t_min_v.z);
        let t_exit = t_max_v.x.min(t_max_v.y).min(t_max_v.z);
        t_enter <= t_exit && t_exit >= 0.0 && t_enter <= max_t
    }
}

// ============================================================
// SQUAD FORMATIONS
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormationType {
    Line,
    Column,
    Wedge,
    InvertedWedge,
    Circle,
    Box,
    EchelonLeft,
    EchelonRight,
    Vee,
    Diamond,
}

pub struct FormationLayout;

impl FormationLayout {
    /// Returns world-space slot positions for N agents given leader pos/forward
    pub fn compute_slots(
        formation: FormationType,
        leader_pos: Vec3,
        leader_forward: Vec3,
        n_agents: usize,
        spacing: f32,
    ) -> Vec<Vec3> {
        let fwd = leader_forward.normalize_or_zero();
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        let mut slots = Vec::with_capacity(n_agents);

        match formation {
            FormationType::Line => {
                // Horizontal line perpendicular to forward
                let half = (n_agents as f32 - 1.0) * 0.5;
                for i in 0..n_agents {
                    let offset = (i as f32 - half) * spacing;
                    slots.push(leader_pos + right * offset);
                }
            }
            FormationType::Column => {
                // Single file behind leader
                for i in 0..n_agents {
                    slots.push(leader_pos - fwd * (i as f32 * spacing));
                }
            }
            FormationType::Wedge => {
                // V-shape with leader at front
                slots.push(leader_pos);
                let mut left = true;
                for i in 1..n_agents {
                    let row = (i + 1) / 2;
                    let side = if left { -1.0 } else { 1.0 };
                    let pos = leader_pos
                        - fwd * (row as f32 * spacing)
                        + right * side * (row as f32 * spacing * 0.7);
                    slots.push(pos);
                    left = !left;
                }
            }
            FormationType::InvertedWedge => {
                slots.push(leader_pos);
                let mut left = true;
                for i in 1..n_agents {
                    let row = (i + 1) / 2;
                    let side = if left { -1.0 } else { 1.0 };
                    let pos = leader_pos
                        + fwd * (row as f32 * spacing)
                        + right * side * (row as f32 * spacing * 0.7);
                    slots.push(pos);
                    left = !left;
                }
            }
            FormationType::Circle => {
                let radius = (n_agents as f32 * spacing) / TWO_PI;
                for i in 0..n_agents {
                    let angle = (i as f32 / n_agents as f32) * TWO_PI;
                    let x = angle.cos();
                    let z = angle.sin();
                    let local = right * x + Vec3::new(0.0, 0.0, 1.0).cross(right) * z;
                    slots.push(leader_pos + local * radius);
                }
            }
            FormationType::Box => {
                // Rectangular grid, roughly square
                let side = (n_agents as f32).sqrt().ceil() as usize;
                for i in 0..n_agents {
                    let row = i / side;
                    let col = i % side;
                    let half_side = (side as f32 - 1.0) * 0.5;
                    let pos = leader_pos
                        - fwd * (row as f32 * spacing)
                        + right * ((col as f32 - half_side) * spacing);
                    slots.push(pos);
                }
            }
            FormationType::EchelonLeft => {
                for i in 0..n_agents {
                    let pos = leader_pos
                        - fwd * (i as f32 * spacing)
                        - right * (i as f32 * spacing * 0.5);
                    slots.push(pos);
                }
            }
            FormationType::EchelonRight => {
                for i in 0..n_agents {
                    let pos = leader_pos
                        - fwd * (i as f32 * spacing)
                        + right * (i as f32 * spacing * 0.5);
                    slots.push(pos);
                }
            }
            FormationType::Vee => {
                slots.push(leader_pos);
                for i in 1..n_agents {
                    let side = if i % 2 == 0 { 1.0f32 } else { -1.0f32 };
                    let rank = ((i + 1) / 2) as f32;
                    let pos = leader_pos
                        - fwd * rank * spacing
                        + right * side * rank * spacing;
                    slots.push(pos);
                }
            }
            FormationType::Diamond => {
                if n_agents == 0 { return slots; }
                // Leader front
                slots.push(leader_pos + fwd * spacing);
                // Left & right
                if n_agents > 1 { slots.push(leader_pos - right * spacing); }
                if n_agents > 2 { slots.push(leader_pos + right * spacing); }
                // Rear
                if n_agents > 3 { slots.push(leader_pos - fwd * spacing); }
                // Fill remaining
                let half = (n_agents.saturating_sub(4) as f32) * 0.5;
                for i in 4..n_agents {
                    let k = (i - 4) as f32;
                    let side = if k % 2.0 < 1.0 { -1.0f32 } else { 1.0f32 };
                    let row = (k * 0.5).floor() + 1.0;
                    slots.push(leader_pos + right * side * row * spacing * 0.5);
                }
            }
        }

        // Ensure we have exactly n_agents slots
        while slots.len() < n_agents {
            let last = slots.last().copied().unwrap_or(leader_pos);
            slots.push(last - fwd * spacing);
        }
        slots.truncate(n_agents);
        slots
    }

    /// Optimal slot assignment using min-cost bipartite matching (Hungarian greedy approximation)
    pub fn assign_slots(agent_positions: &[Vec3], slots: &[Vec3]) -> Vec<usize> {
        let n = agent_positions.len().min(slots.len());
        let mut assignment = vec![usize::MAX; n];
        let mut used_slots: HashSet<usize> = HashSet::new();

        for agent_idx in 0..n {
            let ap = agent_positions[agent_idx];
            let mut best_slot = 0;
            let mut best_dist = f32::MAX;
            for slot_idx in 0..slots.len() {
                if used_slots.contains(&slot_idx) { continue; }
                let d = (slots[slot_idx] - ap).length_squared();
                if d < best_dist {
                    best_dist = d;
                    best_slot = slot_idx;
                }
            }
            assignment[agent_idx] = best_slot;
            used_slots.insert(best_slot);
        }
        assignment
    }
}

// ============================================================
// STEERING BEHAVIORS
// ============================================================

#[derive(Clone, Debug)]
pub struct SteeringAgent {
    pub id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub heading: Vec3,
    pub max_speed: f32,
    pub max_force: f32,
    pub mass: f32,
    pub radius: f32,
    pub wander_angle: f32,
    pub path_index: usize,
}

impl SteeringAgent {
    pub fn new(id: u64, pos: Vec3, max_speed: f32, max_force: f32) -> Self {
        Self {
            id,
            position: pos,
            velocity: Vec3::ZERO,
            heading: Vec3::Z,
            max_speed,
            max_force,
            mass: 1.0,
            radius: 0.5,
            wander_angle: 0.0,
            path_index: 0,
        }
    }

    pub fn apply_force(&mut self, force: Vec3, dt: f32) {
        let clamped = if force.length() > self.max_force {
            force.normalize() * self.max_force
        } else { force };
        let accel = clamped / self.mass;
        self.velocity += accel * dt;
        if self.velocity.length() > self.max_speed {
            self.velocity = self.velocity.normalize() * self.max_speed;
        }
        self.position += self.velocity * dt;
        if self.velocity.length() > EPSILON {
            self.heading = self.velocity.normalize();
        }
    }

    pub fn speed(&self) -> f32 { self.velocity.length() }
}

pub struct SteeringBehaviors;

impl SteeringBehaviors {
    // ---- 1. SEEK ----
    pub fn seek(agent: &SteeringAgent, target: Vec3) -> Vec3 {
        let desired = (target - agent.position).normalize_or_zero() * agent.max_speed;
        desired - agent.velocity
    }

    // ---- 2. FLEE ----
    pub fn flee(agent: &SteeringAgent, threat: Vec3) -> Vec3 {
        let desired = (agent.position - threat).normalize_or_zero() * agent.max_speed;
        desired - agent.velocity
    }

    // ---- 3. ARRIVE ----
    pub fn arrive(agent: &SteeringAgent, target: Vec3, deceleration: f32) -> Vec3 {
        let to_target = target - agent.position;
        let dist = to_target.length();
        if dist < EPSILON { return Vec3::ZERO; }
        // Slow down as we approach
        let speed = (dist / deceleration).min(agent.max_speed);
        let desired = (to_target / dist) * speed;
        desired - agent.velocity
    }

    // ---- 4. PURSUE ----
    pub fn pursue(agent: &SteeringAgent, target_pos: Vec3, target_vel: Vec3) -> Vec3 {
        let to_target = target_pos - agent.position;
        let dist = to_target.length();
        let speed = agent.speed();
        // Prediction time: distance / (own_speed + target_speed) approximately
        let target_speed = target_vel.length();
        let look_ahead = if speed + target_speed > EPSILON {
            dist / (speed + target_speed)
        } else { 0.0 };
        let future_pos = target_pos + target_vel * look_ahead;
        Self::seek(agent, future_pos)
    }

    // ---- 5. EVADE ----
    pub fn evade(agent: &SteeringAgent, threat_pos: Vec3, threat_vel: Vec3) -> Vec3 {
        let to_threat = threat_pos - agent.position;
        let dist = to_threat.length();
        let look_ahead = dist / (agent.max_speed + threat_vel.length() + EPSILON);
        let future_pos = threat_pos + threat_vel * look_ahead;
        Self::flee(agent, future_pos)
    }

    // ---- 6. WANDER ----
    pub fn wander(agent: &mut SteeringAgent, rng_seed: &mut u64, dt: f32) -> Vec3 {
        // Move wander angle randomly
        *rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rand_val = ((*rng_seed >> 33) as i32 as f32) / (i32::MAX as f32);
        agent.wander_angle += rand_val * WANDER_ANGLE_CHANGE;

        // Wander circle center is ahead of agent
        let circle_center = agent.position + agent.heading * WANDER_CIRCLE_DISTANCE;
        // Displacement on the circle
        let displacement = Vec3::new(
            agent.wander_angle.cos() * WANDER_CIRCLE_RADIUS,
            0.0,
            agent.wander_angle.sin() * WANDER_CIRCLE_RADIUS,
        );
        let wander_target = circle_center + displacement;
        Self::seek(agent, wander_target)
    }

    // ---- 7. OBSTACLE AVOIDANCE ----
    pub fn obstacle_avoidance(agent: &SteeringAgent, obstacles: &[Aabb]) -> Vec3 {
        let look_ahead = agent.max_speed * 1.5;
        let ahead = agent.position + agent.heading * look_ahead;
        let ahead_half = agent.position + agent.heading * look_ahead * 0.5;

        let mut most_threat: Option<(&Aabb, Vec3)> = None;
        let mut most_threat_dist = f32::MAX;

        for obs in obstacles {
            let center = obs.center();
            let he = obs.half_extents();
            let r = he.x.max(he.z);  // approximate radius

            // Check if ray from position to ahead intersects obstacle sphere
            let to_center = center - agent.position;
            let proj = to_center.dot(agent.heading);
            if proj < 0.0 { continue; }  // behind agent

            let closest_on_ray = agent.position + agent.heading * proj.min(look_ahead);
            let dist_to_center = (center - closest_on_ray).length();

            if dist_to_center < r + agent.radius {
                let d = (center - agent.position).length();
                if d < most_threat_dist {
                    most_threat_dist = d;
                    most_threat = Some((obs, center));
                }
            }
        }

        if let Some((obs, center)) = most_threat {
            // Steer away
            let avoid_dir = (ahead - center).normalize_or_zero();
            avoid_dir * agent.max_force
        } else {
            Vec3::ZERO
        }
    }

    // ---- 8. WALL FOLLOWING ----
    pub fn wall_following(agent: &SteeringAgent, walls: &[(Vec3, Vec3)]) -> Vec3 {
        // walls: list of (point_on_wall, wall_normal)
        let feeler_len = 2.0;
        let feeler = agent.position + agent.heading * feeler_len;

        let mut force = Vec3::ZERO;
        for &(wall_point, wall_normal) in walls {
            let dist = (agent.position - wall_point).dot(wall_normal);
            if dist > 0.0 && dist < feeler_len + agent.radius {
                // Push agent along the wall (perpendicular to normal)
                let along_wall = Vec3::new(-wall_normal.z, 0.0, wall_normal.x);
                // Desired velocity: along wall direction + slight push away
                let desired = along_wall * agent.max_speed + wall_normal * agent.max_speed * 0.5;
                force = desired - agent.velocity;
                break;
            }
        }
        force
    }

    // ---- 9. PATH FOLLOWING ----
    pub fn path_following(agent: &SteeringAgent, waypoints: &[Vec3], path_index: &mut usize) -> Vec3 {
        if waypoints.is_empty() { return Vec3::ZERO; }
        let current_wp = waypoints[*path_index];
        let dist = (current_wp - agent.position).length();
        let waypoint_radius = 1.0;
        if dist < waypoint_radius && *path_index + 1 < waypoints.len() {
            *path_index += 1;
        }
        Self::arrive(agent, waypoints[*path_index], ARRIVE_DECELERATION_RADIUS)
    }

    // ---- 10. FLOW FIELD FOLLOWING ----
    pub fn flow_field_following(
        agent: &SteeringAgent,
        flow_field: &HashMap<(i32, i32), Vec3>,
        cell_size: f32,
    ) -> Vec3 {
        let cell_x = (agent.position.x / cell_size).floor() as i32;
        let cell_z = (agent.position.z / cell_size).floor() as i32;
        if let Some(&field_dir) = flow_field.get(&(cell_x, cell_z)) {
            let desired = field_dir.normalize_or_zero() * agent.max_speed;
            desired - agent.velocity
        } else {
            Vec3::ZERO
        }
    }

    // ---- 11. ALIGNMENT ----
    pub fn alignment(agent: &SteeringAgent, neighbors: &[&SteeringAgent]) -> Vec3 {
        if neighbors.is_empty() { return Vec3::ZERO; }
        let mut avg_heading = Vec3::ZERO;
        let mut count = 0;
        for n in neighbors {
            if n.id == agent.id { continue; }
            avg_heading += n.heading;
            count += 1;
        }
        if count == 0 { return Vec3::ZERO; }
        avg_heading /= count as f32;
        (avg_heading.normalize_or_zero() * agent.max_speed) - agent.velocity
    }

    // ---- 12. COHESION ----
    pub fn cohesion(agent: &SteeringAgent, neighbors: &[&SteeringAgent]) -> Vec3 {
        if neighbors.is_empty() { return Vec3::ZERO; }
        let mut center = Vec3::ZERO;
        let mut count = 0;
        for n in neighbors {
            if n.id == agent.id { continue; }
            center += n.position;
            count += 1;
        }
        if count == 0 { return Vec3::ZERO; }
        center /= count as f32;
        Self::seek(agent, center)
    }

    // ---- 13. SEPARATION ----
    pub fn separation(agent: &SteeringAgent, neighbors: &[&SteeringAgent], desired_separation: f32) -> Vec3 {
        let mut force = Vec3::ZERO;
        let mut count = 0;
        for n in neighbors {
            if n.id == agent.id { continue; }
            let diff = agent.position - n.position;
            let dist = diff.length();
            if dist < desired_separation && dist > EPSILON {
                // Weighted by inverse distance
                force += (diff / dist) * (desired_separation - dist) / desired_separation;
                count += 1;
            }
        }
        if count > 0 {
            force /= count as f32;
            force.normalize_or_zero() * agent.max_force
        } else {
            Vec3::ZERO
        }
    }

    // ---- 14. LEADER FOLLOWING ----
    pub fn leader_following(
        agent: &SteeringAgent,
        leader: &SteeringAgent,
        slot_offset: Vec3,
    ) -> Vec3 {
        let behind_leader = leader.position
            - leader.heading * LEADER_FOLLOW_DISTANCE
            + leader.heading.cross(Vec3::Y).normalize_or_zero() * slot_offset.x
            - leader.heading * slot_offset.z;

        let dist_to_slot = (behind_leader - agent.position).length();
        let is_on_path = dist_to_slot < 2.0;

        // Evade if ahead of leader (in way)
        let to_agent = agent.position - leader.position;
        let dot = to_agent.dot(leader.heading);
        if dot > 0.0 && to_agent.length() < LEADER_FOLLOW_DISTANCE {
            // Get out of the way
            Self::flee(agent, leader.position + leader.heading * 3.0)
        } else {
            Self::arrive(agent, behind_leader, ARRIVE_DECELERATION_RADIUS * 0.5)
        }
    }

    // ---- 15. QUEUE BEHAVIOR ----
    pub fn queue_behavior(
        agent: &SteeringAgent,
        neighbors: &[&SteeringAgent],
        target: Vec3,
    ) -> Vec3 {
        // Move toward target but slow down if neighbor ahead is too close
        let ahead_in_queue = neighbors.iter()
            .filter(|n| n.id != agent.id)
            .filter(|n| {
                let to_n = n.position - agent.position;
                let dist = to_n.length();
                dist < QUEUE_MIN_DIST * 3.0 && to_n.dot(agent.heading) > 0.0
            })
            .min_by(|a, b| {
                let da = (a.position - agent.position).length_squared();
                let db = (b.position - agent.position).length_squared();
                da.partial_cmp(&db).unwrap()
            });

        if let Some(ahead) = ahead_in_queue {
            let dist = (ahead.position - agent.position).length();
            if dist < QUEUE_MIN_DIST {
                // Too close — brake
                return -agent.velocity;
            }
        }
        Self::arrive(agent, target, ARRIVE_DECELERATION_RADIUS)
    }

    // ---- 16. COLLISION AVOIDANCE ----
    pub fn collision_avoidance(agent: &SteeringAgent, others: &[&SteeringAgent]) -> Vec3 {
        let mut first_threat: Option<(&SteeringAgent, f32)> = None;
        let min_time_to_collision = f32::MAX;
        let mut min_time = min_time_to_collision;

        for other in others {
            if other.id == agent.id { continue; }
            let rel_pos = other.position - agent.position;
            let rel_vel = other.velocity - agent.velocity;
            // Time to closest approach
            let rel_speed_sq = rel_vel.length_squared();
            if rel_speed_sq < EPSILON { continue; }
            let t = -rel_pos.dot(rel_vel) / rel_speed_sq;
            if t < 0.0 || t > 5.0 { continue; }
            let closest_dist = (rel_pos + rel_vel * t).length();
            let combined_radius = agent.radius + other.radius;
            if closest_dist < combined_radius && t < min_time {
                min_time = t;
                first_threat = Some((other, t));
            }
        }

        if let Some((threat, t)) = first_threat {
            let future_rel_pos = (threat.position + threat.velocity * t) - (agent.position + agent.velocity * t);
            let push = (agent.position - threat.position).normalize_or_zero();
            push * agent.max_force * (1.0 - (t / 5.0).clamp(0.0, 1.0))
        } else {
            Vec3::ZERO
        }
    }

    // ---- 17. HIDE ----
    pub fn hide(
        agent: &SteeringAgent,
        threat: Vec3,
        obstacles: &[Aabb],
    ) -> Vec3 {
        let mut best_hiding_spot = agent.position;
        let mut best_dist = f32::MAX;

        for obs in obstacles {
            let center = obs.center();
            // Hiding spot is on the far side of obstacle from threat
            let to_center = (center - threat).normalize_or_zero();
            let he = obs.half_extents();
            let r = he.x.max(he.z);
            let hiding_spot = center + to_center * (r + agent.radius + 1.0);
            let dist = (hiding_spot - agent.position).length_squared();
            if dist < best_dist {
                best_dist = dist;
                best_hiding_spot = hiding_spot;
            }
        }
        Self::arrive(agent, best_hiding_spot, ARRIVE_DECELERATION_RADIUS)
    }

    // ---- 18. INTERPOSE ----
    pub fn interpose(
        agent: &SteeringAgent,
        agent_a: &SteeringAgent,
        agent_b: &SteeringAgent,
    ) -> Vec3 {
        // Get future midpoint between A and B
        let midpoint = (agent_a.position + agent_b.position) * 0.5;
        let time_to_reach = (midpoint - agent.position).length() / (agent.max_speed + EPSILON);
        let future_a = agent_a.position + agent_a.velocity * time_to_reach;
        let future_b = agent_b.position + agent_b.velocity * time_to_reach;
        let future_mid = (future_a + future_b) * 0.5;
        Self::arrive(agent, future_mid, ARRIVE_DECELERATION_RADIUS)
    }

    /// Weighted sum of all applicable behaviors
    pub fn compute_weighted(
        agent: &mut SteeringAgent,
        seek_target: Option<Vec3>,
        flee_target: Option<Vec3>,
        arrive_target: Option<Vec3>,
        pursue_target: Option<(Vec3, Vec3)>,
        evade_threat: Option<(Vec3, Vec3)>,
        do_wander: bool,
        rng_seed: &mut u64,
        dt: f32,
        obstacles: &[Aabb],
        walls: &[(Vec3, Vec3)],
        neighbors: &[&SteeringAgent],
        waypoints: Option<&[Vec3]>,
        flow_field: Option<&HashMap<(i32, i32), Vec3>>,
        leader: Option<&SteeringAgent>,
        hide_from: Option<Vec3>,
        interpose_ab: Option<(&SteeringAgent, &SteeringAgent)>,
    ) -> Vec3 {
        let mut total = Vec3::ZERO;

        macro_rules! add_force {
            ($force:expr, $weight:expr, $budget:expr) => {{
                let f = $force * $weight;
                let len = f.length();
                if len > EPSILON {
                    total += f;
                }
            }};
        }

        if let Some(t) = seek_target { add_force!(Self::seek(agent, t), 1.0, agent.max_force); }
        if let Some(t) = flee_target { add_force!(Self::flee(agent, t), 1.0, agent.max_force); }
        if let Some(t) = arrive_target { add_force!(Self::arrive(agent, t, ARRIVE_DECELERATION_RADIUS), 1.0, agent.max_force); }
        if let Some((p, v)) = pursue_target { add_force!(Self::pursue(agent, p, v), 1.0, agent.max_force); }
        if let Some((p, v)) = evade_threat { add_force!(Self::evade(agent, p, v), 1.0, agent.max_force); }
        if do_wander { add_force!(Self::wander(agent, rng_seed, dt), 0.5, agent.max_force); }
        if !obstacles.is_empty() { add_force!(Self::obstacle_avoidance(agent, obstacles), 2.0, agent.max_force); }
        if !walls.is_empty() { add_force!(Self::wall_following(agent, walls), 1.0, agent.max_force); }
        if !neighbors.is_empty() {
            add_force!(Self::alignment(agent, neighbors), ALIGNMENT_WEIGHT, agent.max_force);
            add_force!(Self::cohesion(agent, neighbors), COHESION_WEIGHT, agent.max_force);
            add_force!(Self::separation(agent, neighbors, agent.radius * 2.5), SEPARATION_WEIGHT, agent.max_force);
            add_force!(Self::collision_avoidance(agent, neighbors), 2.0, agent.max_force);
        }
        if let Some(wps) = waypoints {
            let pi = &mut { agent.path_index };
            add_force!(Self::path_following(agent, wps, pi), 1.0, agent.max_force);
        }
        if let Some(ff) = flow_field {
            add_force!(Self::flow_field_following(agent, ff, 1.0), 1.0, agent.max_force);
        }
        if let Some(ldr) = leader {
            add_force!(Self::leader_following(agent, ldr, Vec3::ZERO), 1.0, agent.max_force);
        }
        if let Some(threat) = hide_from {
            add_force!(Self::hide(agent, threat, obstacles), 1.0, agent.max_force);
        }
        if let Some((a, b)) = interpose_ab {
            add_force!(Self::interpose(agent, a, b), 1.0, agent.max_force);
        }

        // Clamp total force
        if total.length() > agent.max_force {
            total = total.normalize() * agent.max_force;
        }
        total
    }
}

// ============================================================
// FSM — FINITE STATE MACHINE EDITOR
// ============================================================

#[derive(Clone, Debug)]
pub enum FsmConditionOp {
    BlackboardBool { key: String, expected: bool },
    BlackboardCompare { key: String, op: CompareOp, value: BlackboardValue },
    TimeElapsed { duration: f32 },
    Always,
    Never,
    And(Box<FsmConditionOp>, Box<FsmConditionOp>),
    Or(Box<FsmConditionOp>, Box<FsmConditionOp>),
    Not(Box<FsmConditionOp>),
}

impl FsmConditionOp {
    pub fn evaluate(&self, blackboard: &Blackboard, time_in_state: f32) -> bool {
        match self {
            FsmConditionOp::Always => true,
            FsmConditionOp::Never => false,
            FsmConditionOp::BlackboardBool { key, expected } => {
                blackboard.get_bool(key) == *expected
            }
            FsmConditionOp::BlackboardCompare { key, op, value } => {
                op.evaluate(blackboard.get(key), value)
            }
            FsmConditionOp::TimeElapsed { duration } => time_in_state >= *duration,
            FsmConditionOp::And(a, b) => {
                a.evaluate(blackboard, time_in_state) && b.evaluate(blackboard, time_in_state)
            }
            FsmConditionOp::Or(a, b) => {
                a.evaluate(blackboard, time_in_state) || b.evaluate(blackboard, time_in_state)
            }
            FsmConditionOp::Not(inner) => !inner.evaluate(blackboard, time_in_state),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FsmTransition {
    pub id: u32,
    pub from_state: u32,
    pub to_state: u32,
    pub condition: FsmConditionOp,
    pub priority: i32,
    pub actions: Vec<FsmAction>,
}

#[derive(Clone, Debug)]
pub enum FsmAction {
    SetBlackboard { key: String, value: BlackboardValue },
    IncrementBlackboard { key: String, amount: f32 },
    Log { message: String },
    PlayAnimation { clip: String },
    PlaySound { sound: String },
}

impl FsmAction {
    pub fn execute(&self, blackboard: &mut Blackboard) {
        match self {
            FsmAction::SetBlackboard { key, value } => {
                blackboard.set(key, value.clone());
            }
            FsmAction::IncrementBlackboard { key, amount } => {
                let v = blackboard.get_float(key);
                blackboard.set(key, BlackboardValue::Float(v + amount));
            }
            FsmAction::Log { message } => {
                // In real engine: log to console
                let _ = message;
            }
            FsmAction::PlayAnimation { clip } => {
                blackboard.set("fsm_anim", BlackboardValue::String(clip.clone()));
            }
            FsmAction::PlaySound { sound } => {
                blackboard.set("fsm_sound", BlackboardValue::String(sound.clone()));
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct FsmState {
    pub id: u32,
    pub name: String,
    pub entry_actions: Vec<FsmAction>,
    pub exit_actions: Vec<FsmAction>,
    pub tick_actions: Vec<FsmAction>,
    pub position: Vec2,  // editor layout
    pub is_initial: bool,
    pub is_final: bool,
    pub color: Vec4,
    pub sub_fsm: Option<u32>,  // sub FSM id for hierarchical FSMs
}

impl FsmState {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            entry_actions: Vec::new(),
            exit_actions: Vec::new(),
            tick_actions: Vec::new(),
            position: Vec2::ZERO,
            is_initial: false,
            is_final: false,
            color: Vec4::new(0.3, 0.4, 0.7, 1.0),
            sub_fsm: None,
        }
    }
}

pub struct FsmInstance {
    pub id: u32,
    pub name: String,
    pub states: HashMap<u32, FsmState>,
    pub transitions: Vec<FsmTransition>,
    pub initial_state: Option<u32>,
    pub current_state: Option<u32>,
    pub time_in_state: f32,
    pub transition_history: VecDeque<(u32, u32, f32)>,   // (from, to, time)
    pub next_state_id: u32,
    pub next_transition_id: u32,
}

impl FsmInstance {
    pub fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            states: HashMap::new(),
            transitions: Vec::new(),
            initial_state: None,
            current_state: None,
            time_in_state: 0.0,
            transition_history: VecDeque::with_capacity(32),
            next_state_id: 1,
            next_transition_id: 1,
        }
    }

    pub fn add_state(&mut self, name: &str) -> u32 {
        let id = self.next_state_id;
        self.next_state_id += 1;
        self.states.insert(id, FsmState::new(id, name));
        id
    }

    pub fn set_initial(&mut self, state_id: u32) {
        if let Some(s) = self.states.get_mut(&state_id) { s.is_initial = true; }
        self.initial_state = Some(state_id);
    }

    pub fn add_transition(&mut self, from: u32, to: u32, condition: FsmConditionOp, priority: i32) -> u32 {
        let id = self.next_transition_id;
        self.next_transition_id += 1;
        self.transitions.push(FsmTransition { id, from_state: from, to_state: to, condition, priority, actions: Vec::new() });
        id
    }

    pub fn start(&mut self, blackboard: &mut Blackboard) {
        if let Some(init) = self.initial_state {
            self.enter_state(init, blackboard, 0.0);
        }
    }

    fn enter_state(&mut self, state_id: u32, blackboard: &mut Blackboard, time: f32) {
        if let Some(prev) = self.current_state {
            if let Some(state) = self.states.get(&prev) {
                let exit_actions: Vec<FsmAction> = state.exit_actions.clone();
                for action in &exit_actions { action.execute(blackboard); }
            }
        }
        if let Some(prev) = self.current_state {
            self.transition_history.push_back((prev, state_id, time));
            if self.transition_history.len() > 32 { self.transition_history.pop_front(); }
        }
        self.current_state = Some(state_id);
        self.time_in_state = 0.0;
        if let Some(state) = self.states.get(&state_id) {
            let entry_actions: Vec<FsmAction> = state.entry_actions.clone();
            for action in &entry_actions { action.execute(blackboard); }
        }
    }

    pub fn tick(&mut self, blackboard: &mut Blackboard, dt: f32, current_time: f32) {
        self.time_in_state += dt;
        let current = match self.current_state { Some(c) => c, None => return };

        // Execute tick actions
        if let Some(state) = self.states.get(&current) {
            let tick_actions: Vec<FsmAction> = state.tick_actions.clone();
            for action in &tick_actions { action.execute(blackboard); }
        }

        // Check transitions sorted by priority
        let mut sorted_transitions: Vec<&FsmTransition> = self.transitions.iter()
            .filter(|t| t.from_state == current)
            .collect();
        sorted_transitions.sort_by(|a, b| b.priority.cmp(&a.priority));

        for t in sorted_transitions {
            if t.condition.evaluate(blackboard, self.time_in_state) {
                let to = t.to_state;
                let t_actions: Vec<FsmAction> = t.actions.clone();
                for action in &t_actions { action.execute(blackboard); }
                self.enter_state(to, blackboard, current_time);
                break;
            }
        }
    }

    /// Auto-layout states using circular layout
    pub fn auto_layout(&mut self) {
        let n = self.states.len();
        if n == 0 { return; }
        let radius = (n as f32 * 80.0) / TWO_PI;
        let ids: Vec<u32> = self.states.keys().copied().collect();
        for (i, id) in ids.iter().enumerate() {
            let angle = (i as f32 / n as f32) * TWO_PI;
            let pos = Vec2::new(angle.cos() * radius, angle.sin() * radius);
            if let Some(s) = self.states.get_mut(id) { s.position = pos; }
        }
    }
}

// ============================================================
// EMOTION SYSTEM — PLUTCHIK WHEEL
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrimaryEmotion {
    Joy,
    Trust,
    Fear,
    Surprise,
    Sadness,
    Disgust,
    Anger,
    Anticipation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SecondaryEmotion {
    Love,       // Joy + Trust
    Submission, // Trust + Fear
    Awe,        // Fear + Surprise
    Disapproval,// Surprise + Sadness
    Remorse,    // Sadness + Disgust
    Contempt,   // Disgust + Anger
    Aggressiveness, // Anger + Anticipation
    Optimism,   // Anticipation + Joy
}

impl PrimaryEmotion {
    pub const ALL: [PrimaryEmotion; 8] = [
        PrimaryEmotion::Joy,
        PrimaryEmotion::Trust,
        PrimaryEmotion::Fear,
        PrimaryEmotion::Surprise,
        PrimaryEmotion::Sadness,
        PrimaryEmotion::Disgust,
        PrimaryEmotion::Anger,
        PrimaryEmotion::Anticipation,
    ];

    pub fn index(&self) -> usize {
        match self {
            PrimaryEmotion::Joy => 0,
            PrimaryEmotion::Trust => 1,
            PrimaryEmotion::Fear => 2,
            PrimaryEmotion::Surprise => 3,
            PrimaryEmotion::Sadness => 4,
            PrimaryEmotion::Disgust => 5,
            PrimaryEmotion::Anger => 6,
            PrimaryEmotion::Anticipation => 7,
        }
    }

    /// Opposite emotion on the wheel (180 degrees)
    pub fn opposite(&self) -> PrimaryEmotion {
        PrimaryEmotion::ALL[(self.index() + 4) % 8]
    }

    /// Wheel position as Vec2 (unit circle, 8 segments)
    pub fn wheel_position(&self) -> Vec2 {
        let angle = (self.index() as f32 / 8.0) * TWO_PI;
        Vec2::new(angle.cos(), angle.sin())
    }

    /// Get the secondary emotion formed with the next emotion
    pub fn blend_with_next(&self) -> SecondaryEmotion {
        match self {
            PrimaryEmotion::Joy => SecondaryEmotion::Love,
            PrimaryEmotion::Trust => SecondaryEmotion::Submission,
            PrimaryEmotion::Fear => SecondaryEmotion::Awe,
            PrimaryEmotion::Surprise => SecondaryEmotion::Disapproval,
            PrimaryEmotion::Sadness => SecondaryEmotion::Remorse,
            PrimaryEmotion::Disgust => SecondaryEmotion::Contempt,
            PrimaryEmotion::Anger => SecondaryEmotion::Aggressiveness,
            PrimaryEmotion::Anticipation => SecondaryEmotion::Optimism,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EmotionState {
    pub intensities: [f32; 8],   // One per primary emotion
    pub secondary_intensities: [f32; 8],
    pub mood_valence: f32,       // -1 negative .. +1 positive
    pub mood_arousal: f32,       // 0 calm .. 1 excited
    pub decay_rates: [f32; 8],
    pub threshold: f32,          // Below this: negligible
}

impl EmotionState {
    pub fn new() -> Self {
        Self {
            intensities: [0.0; 8],
            secondary_intensities: [0.0; 8],
            mood_valence: 0.0,
            mood_arousal: 0.0,
            decay_rates: [EMOTION_DECAY_RATE; 8],
            threshold: 0.02,
        }
    }

    pub fn add_emotion(&mut self, emotion: PrimaryEmotion, amount: f32) {
        let idx = emotion.index();
        self.intensities[idx] = (self.intensities[idx] + amount).clamp(0.0, 1.0);
        // Suppress opposite emotion
        let opp_idx = emotion.opposite().index();
        self.intensities[opp_idx] = (self.intensities[opp_idx] - amount * 0.3).max(0.0);
    }

    pub fn get_intensity(&self, emotion: PrimaryEmotion) -> f32 {
        self.intensities[emotion.index()]
    }

    pub fn dominant(&self) -> Option<PrimaryEmotion> {
        let max_idx = self.intensities.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)?;
        if self.intensities[max_idx] < self.threshold { return None; }
        Some(PrimaryEmotion::ALL[max_idx])
    }

    pub fn update(&mut self, dt: f32) {
        // Decay emotions
        for i in 0..8 {
            self.intensities[i] = (self.intensities[i] - self.decay_rates[i] * dt).max(0.0);
        }

        // Compute secondary emotions
        for i in 0..8 {
            let next = (i + 1) % 8;
            self.secondary_intensities[i] = (self.intensities[i] + self.intensities[next]) * 0.5;
        }

        // Compute mood valence: positive = joy, trust, anticipation; negative = fear, sadness, disgust, anger
        let positive = self.intensities[0] + self.intensities[1] + self.intensities[7]; // joy, trust, anticipation
        let negative = self.intensities[2] + self.intensities[4] + self.intensities[5] + self.intensities[6]; // fear, sadness, disgust, anger
        let total = positive + negative;
        if total > EPSILON {
            self.mood_valence = (positive - negative) / total;
        }

        // Arousal: surprise and fear drive high arousal; sadness drives low
        let high_arousal = self.intensities[2] + self.intensities[3] + self.intensities[6]; // fear, surprise, anger
        let low_arousal = self.intensities[4];  // sadness
        self.mood_arousal = ((high_arousal - low_arousal * 0.5) / (8.0f32.sqrt())).clamp(0.0, 1.0);
    }

    /// Map emotion state to behavior parameter modifiers
    pub fn behavior_modifiers(&self) -> EmotionBehaviorModifiers {
        EmotionBehaviorModifiers {
            speed_multiplier: 1.0 + self.intensities[6] * 0.3      // anger increases speed
                - self.intensities[4] * 0.2                         // sadness decreases
                + self.intensities[7] * 0.15,                       // anticipation
            aggression_bias: self.intensities[6] * 0.5 + self.intensities[3] * 0.2, // anger + surprise
            flee_threshold_modifier: self.intensities[2] * 0.4,    // fear: flee sooner
            search_radius_multiplier: 1.0 + self.intensities[7] * 0.3, // anticipation: search wider
            reaction_time_modifier: -self.intensities[2] * 0.2     // fear: faster reaction
                + self.intensities[4] * 0.3,                        // sadness: slower
            accuracy_modifier: 1.0 - self.intensities[2] * 0.15    // fear reduces accuracy
                - self.intensities[3] * 0.1,                        // surprise
            cooperation_bias: self.intensities[1] * 0.4            // trust improves cooperation
                - self.intensities[5] * 0.3,                        // disgust reduces
            curiosity_bias: self.intensities[3] * 0.3 + self.intensities[7] * 0.2,
        }
    }

    pub fn serialize_to_blackboard(&self, blackboard: &mut Blackboard, prefix: &str) {
        for (i, &intensity) in self.intensities.iter().enumerate() {
            let emotion_name = match i {
                0 => "joy", 1 => "trust", 2 => "fear", 3 => "surprise",
                4 => "sadness", 5 => "disgust", 6 => "anger", 7 => "anticipation",
                _ => "unknown",
            };
            blackboard.set(
                &format!("{}_{}", prefix, emotion_name),
                BlackboardValue::Float(intensity),
            );
        }
        blackboard.set(&format!("{}_valence", prefix), BlackboardValue::Float(self.mood_valence));
        blackboard.set(&format!("{}_arousal", prefix), BlackboardValue::Float(self.mood_arousal));
    }
}

#[derive(Clone, Debug)]
pub struct EmotionBehaviorModifiers {
    pub speed_multiplier: f32,
    pub aggression_bias: f32,
    pub flee_threshold_modifier: f32,
    pub search_radius_multiplier: f32,
    pub reaction_time_modifier: f32,
    pub accuracy_modifier: f32,
    pub cooperation_bias: f32,
    pub curiosity_bias: f32,
}

impl Default for EmotionBehaviorModifiers {
    fn default() -> Self {
        Self {
            speed_multiplier: 1.0,
            aggression_bias: 0.0,
            flee_threshold_modifier: 0.0,
            search_radius_multiplier: 1.0,
            reaction_time_modifier: 0.0,
            accuracy_modifier: 1.0,
            cooperation_bias: 0.0,
            curiosity_bias: 0.0,
        }
    }
}

// Emotional stimulus
#[derive(Clone, Debug)]
pub struct EmotionalStimulus {
    pub emotion: PrimaryEmotion,
    pub intensity: f32,
    pub source_id: u64,
    pub decay_rate_override: Option<f32>,
}

pub struct EmotionEngine {
    pub state: EmotionState,
    pub stimuli_queue: VecDeque<EmotionalStimulus>,
    pub history: VecDeque<(f32, [f32; 8])>,  // (time, intensities)
    pub history_capacity: usize,
}

impl EmotionEngine {
    pub fn new() -> Self {
        Self {
            state: EmotionState::new(),
            stimuli_queue: VecDeque::new(),
            history: VecDeque::with_capacity(64),
            history_capacity: 64,
        }
    }

    pub fn submit_stimulus(&mut self, stimulus: EmotionalStimulus) {
        self.stimuli_queue.push_back(stimulus);
    }

    pub fn update(&mut self, dt: f32, current_time: f32) {
        // Process stimuli
        while let Some(stimulus) = self.stimuli_queue.pop_front() {
            self.state.add_emotion(stimulus.emotion, stimulus.intensity);
            if let Some(rate) = stimulus.decay_rate_override {
                let idx = stimulus.emotion.index();
                self.state.decay_rates[idx] = rate;
            }
        }
        self.state.update(dt);

        // Record history
        self.history.push_back((current_time as f32, self.state.intensities));
        if self.history.len() > self.history_capacity {
            self.history.pop_front();
        }
    }

    pub fn get_modifier(&self) -> EmotionBehaviorModifiers {
        self.state.behavior_modifiers()
    }

    /// Compute emotional contagion: spread emotion from nearby agents
    pub fn apply_contagion(
        &mut self,
        neighbor_emotions: &[EmotionState],
        contagion_rate: f32,
    ) {
        for neighbor in neighbor_emotions {
            for emotion in &PrimaryEmotion::ALL {
                let n_intensity = neighbor.get_intensity(*emotion);
                if n_intensity > 0.1 {
                    self.state.add_emotion(*emotion, n_intensity * contagion_rate);
                }
            }
        }
    }
}

// ============================================================
// NODE GRAPH EDITOR — UI STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct NodeGraphCamera {
    pub pan: Vec2,
    pub zoom: f32,
    pub target_pan: Vec2,
    pub target_zoom: f32,
}

impl NodeGraphCamera {
    pub fn new() -> Self {
        Self { pan: Vec2::ZERO, zoom: 1.0, target_pan: Vec2::ZERO, target_zoom: 1.0 }
    }

    pub fn world_to_screen(&self, world_pos: Vec2, viewport_size: Vec2) -> Vec2 {
        let centered = world_pos * self.zoom + viewport_size * 0.5 + self.pan;
        centered
    }

    pub fn screen_to_world(&self, screen_pos: Vec2, viewport_size: Vec2) -> Vec2 {
        (screen_pos - viewport_size * 0.5 - self.pan) / self.zoom
    }

    pub fn smooth_update(&mut self, dt: f32) {
        let speed = 10.0 * dt;
        self.pan = self.pan.lerp(self.target_pan, speed.min(1.0));
        self.zoom = self.zoom + (self.target_zoom - self.zoom) * speed.min(1.0);
        self.zoom = self.zoom.clamp(0.05, 5.0);
    }

    pub fn zoom_toward(&mut self, screen_point: Vec2, viewport_size: Vec2, delta: f32) {
        let world_before = self.screen_to_world(screen_point, viewport_size);
        self.target_zoom = (self.target_zoom * (1.0 + delta * 0.1)).clamp(0.05, 5.0);
        // Adjust pan so zoom centers on cursor
        let world_after = self.screen_to_world(screen_point, viewport_size);
        let diff = world_after - world_before;
        self.target_pan = self.target_pan + diff * self.target_zoom;
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionDraft {
    pub from_node: u32,
    pub from_port: usize,
    pub current_pos: Vec2,
    pub is_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTool {
    Select,
    Pan,
    AddNode,
    Connect,
    Delete,
    Comment,
}

#[derive(Clone, Debug)]
pub struct NodeComment {
    pub id: u32,
    pub text: String,
    pub rect: (Vec2, Vec2),   // top-left, bottom-right
    pub color: Vec4,
}

#[derive(Clone, Debug)]
pub struct GraphSelection {
    pub selected_nodes: HashSet<u32>,
    pub selection_rect: Option<(Vec2, Vec2)>,
    pub is_dragging: bool,
    pub drag_start: Vec2,
    pub drag_offset: HashMap<u32, Vec2>,
}

impl GraphSelection {
    pub fn new() -> Self {
        Self {
            selected_nodes: HashSet::new(),
            selection_rect: None,
            is_dragging: false,
            drag_start: Vec2::ZERO,
            drag_offset: HashMap::new(),
        }
    }

    pub fn select_single(&mut self, id: u32) {
        self.selected_nodes.clear();
        self.selected_nodes.insert(id);
    }

    pub fn toggle(&mut self, id: u32) {
        if self.selected_nodes.contains(&id) {
            self.selected_nodes.remove(&id);
        } else {
            self.selected_nodes.insert(id);
        }
    }

    pub fn clear(&mut self) {
        self.selected_nodes.clear();
        self.selection_rect = None;
    }

    pub fn apply_rect_selection(&mut self, nodes: &HashMap<u32, BtNode>) {
        if let Some((min, max)) = self.selection_rect {
            let rect_min = Vec2::new(min.x.min(max.x), min.y.min(max.y));
            let rect_max = Vec2::new(min.x.max(max.x), min.y.max(max.y));
            for (id, node) in nodes {
                let center = node.position + node.size * 0.5;
                if center.x >= rect_min.x && center.x <= rect_max.x
                && center.y >= rect_min.y && center.y <= rect_max.y {
                    self.selected_nodes.insert(*id);
                }
            }
        }
    }
}

// ============================================================
// BLACKBOARD INSPECTOR STATE
// ============================================================

#[derive(Clone, Debug)]
pub struct BlackboardInspector {
    pub filter_text: String,
    pub show_only_changed: bool,
    pub sort_by_name: bool,
    pub sort_by_time: bool,
    pub sort_ascending: bool,
    pub highlighted_keys: HashSet<String>,
    pub pinned_keys: Vec<String>,
    pub edit_key: Option<String>,
    pub edit_value: String,
    pub history: VecDeque<(String, BlackboardValue, f64)>,
    pub history_capacity: usize,
}

impl BlackboardInspector {
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            show_only_changed: false,
            sort_by_name: true,
            sort_by_time: false,
            sort_ascending: true,
            highlighted_keys: HashSet::new(),
            pinned_keys: Vec::new(),
            edit_key: None,
            edit_value: String::new(),
            history: VecDeque::with_capacity(256),
            history_capacity: 256,
        }
    }

    pub fn get_filtered_keys<'a>(&'a self, blackboard: &'a Blackboard) -> Vec<&'a str> {
        let mut keys: Vec<&str> = blackboard.entries.keys().map(|s| s.as_str()).collect();

        if !self.filter_text.is_empty() {
            let filter = self.filter_text.to_lowercase();
            keys.retain(|k| k.to_lowercase().contains(&filter));
        }

        if self.sort_by_name {
            keys.sort_by(|a, b| {
                if self.sort_ascending { a.cmp(b) } else { b.cmp(a) }
            });
        } else if self.sort_by_time {
            keys.sort_by(|a, b| {
                let ta = blackboard.change_timestamps.get(*a).copied().unwrap_or(0.0);
                let tb = blackboard.change_timestamps.get(*b).copied().unwrap_or(0.0);
                if self.sort_ascending {
                    ta.partial_cmp(&tb).unwrap()
                } else {
                    tb.partial_cmp(&ta).unwrap()
                }
            });
        }

        // Pinned keys first
        let pinned: Vec<&str> = self.pinned_keys.iter().map(|s| s.as_str()).collect();
        let mut result: Vec<&str> = pinned.iter().filter(|&&k| keys.contains(&k)).copied().collect();
        result.extend(keys.iter().filter(|&&k| !self.pinned_keys.iter().any(|p| p == k)));
        result
    }

    pub fn record_change(&mut self, key: &str, value: BlackboardValue, time: f64) {
        self.history.push_back((key.to_string(), value, time));
        if self.history.len() > self.history_capacity {
            self.history.pop_front();
        }
        self.highlighted_keys.insert(key.to_string());
    }

    pub fn value_to_string(value: &BlackboardValue) -> String {
        match value {
            BlackboardValue::Bool(b) => format!("{}", b),
            BlackboardValue::Int(i) => format!("{}", i),
            BlackboardValue::Float(f) => format!("{:.4}", f),
            BlackboardValue::Vec2(v) => format!("({:.2}, {:.2})", v.x, v.y),
            BlackboardValue::Vec3(v) => format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z),
            BlackboardValue::String(s) => s.clone(),
            BlackboardValue::EntityId(id) => format!("Entity#{}", id),
            BlackboardValue::None => "<none>".to_string(),
        }
    }

    pub fn try_parse_value(raw: &str, hint: &BlackboardValue) -> Option<BlackboardValue> {
        match hint {
            BlackboardValue::Bool(_) => raw.parse::<bool>().ok().map(BlackboardValue::Bool),
            BlackboardValue::Int(_) => raw.parse::<i64>().ok().map(BlackboardValue::Int),
            BlackboardValue::Float(_) => raw.parse::<f32>().ok().map(BlackboardValue::Float),
            BlackboardValue::String(_) => Some(BlackboardValue::String(raw.to_string())),
            _ => None,
        }
    }
}

// ============================================================
// DEBUG VISUALIZATION DATA
// ============================================================

#[derive(Clone, Debug)]
pub struct DebugVizShape {
    pub shape_type: DebugShapeType,
    pub color: Vec4,
    pub duration: f32,       // seconds; 0 = one frame
    pub elapsed: f32,
}

#[derive(Clone, Debug)]
pub enum DebugShapeType {
    Line { from: Vec3, to: Vec3 },
    Circle { center: Vec3, radius: f32, normal: Vec3 },
    Sphere { center: Vec3, radius: f32 },
    Aabb(Aabb),
    Arrow { from: Vec3, to: Vec3, head_size: f32 },
    Text { position: Vec3, text: String, size: f32 },
    Cross { center: Vec3, size: f32 },
    Arc { center: Vec3, from_angle: f32, to_angle: f32, radius: f32, normal: Vec3 },
}

pub struct DebugVisualizationBuffer {
    pub shapes: Vec<DebugVizShape>,
    pub max_shapes: usize,
}

impl DebugVisualizationBuffer {
    pub fn new(max_shapes: usize) -> Self {
        Self { shapes: Vec::with_capacity(max_shapes), max_shapes }
    }

    pub fn add(&mut self, shape: DebugShapeType, color: Vec4, duration: f32) {
        if self.shapes.len() >= self.max_shapes { return; }
        self.shapes.push(DebugVizShape { shape_type: shape, color, duration, elapsed: 0.0 });
    }

    pub fn tick(&mut self, dt: f32) {
        self.shapes.retain_mut(|s| {
            s.elapsed += dt;
            s.duration == 0.0 || s.elapsed < s.duration
        });
    }

    pub fn draw_vision_cone(&mut self, pos: Vec3, forward: Vec3, half_angle: f32, range: f32, color: Vec4) {
        // Draw arc at range
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let steps = 16;
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let a0 = -half_angle + t0 * half_angle * 2.0;
            let a1 = -half_angle + t1 * half_angle * 2.0;
            let d0 = forward * a0.cos() + right * a0.sin();
            let d1 = forward * a1.cos() + right * a1.sin();
            self.add(DebugShapeType::Line {
                from: pos + d0 * range,
                to: pos + d1 * range,
            }, color, 0.0);
        }
        // Left edge
        let left_edge = forward * half_angle.cos() - right * half_angle.sin();
        self.add(DebugShapeType::Line { from: pos, to: pos + left_edge * range }, color, 0.0);
        // Right edge
        let right_edge = forward * half_angle.cos() + right * half_angle.sin();
        self.add(DebugShapeType::Line { from: pos, to: pos + right_edge * range }, color, 0.0);
    }

    pub fn draw_hearing_radius(&mut self, pos: Vec3, radius: f32, color: Vec4) {
        self.add(DebugShapeType::Circle { center: pos, radius, normal: Vec3::Y }, color, 0.0);
    }

    pub fn draw_bt_status(&mut self, pos: Vec3, status: BtStatus) {
        let color = match status {
            BtStatus::Success => Vec4::new(0.0, 1.0, 0.0, 0.8),
            BtStatus::Failure => Vec4::new(1.0, 0.0, 0.0, 0.8),
            BtStatus::Running => Vec4::new(1.0, 1.0, 0.0, 0.8),
            BtStatus::Invalid => Vec4::new(0.5, 0.5, 0.5, 0.5),
        };
        self.add(DebugShapeType::Sphere { center: pos, radius: 0.3 }, color, 0.0);
    }

    pub fn draw_formation_slots(&mut self, slots: &[Vec3], assignments: &[usize], color: Vec4) {
        for &slot_pos in slots {
            self.add(DebugShapeType::Cross { center: slot_pos, size: 0.5 }, color, 0.0);
        }
    }

    pub fn draw_velocity_arrow(&mut self, pos: Vec3, vel: Vec3, color: Vec4) {
        if vel.length() > EPSILON {
            self.add(DebugShapeType::Arrow {
                from: pos,
                to: pos + vel,
                head_size: vel.length() * 0.2,
            }, color, 0.0);
        }
    }

    pub fn draw_emotion_wheel(&mut self, center: Vec3, emotions: &EmotionState, scale: f32) {
        for (i, &intensity) in emotions.intensities.iter().enumerate() {
            if intensity < 0.01 { continue; }
            let angle = (i as f32 / 8.0) * TWO_PI;
            let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
            let end = center + dir * (intensity * scale);
            let hue = i as f32 / 8.0;
            let color = hsv_to_rgba(hue, 0.8, 0.9, 0.9);
            self.add(DebugShapeType::Arrow { from: center, to: end, head_size: 0.1 }, color, 0.0);
        }
    }
}

// Color utility
fn hsv_to_rgba(h: f32, s: f32, v: f32, a: f32) -> Vec4 {
    let h6 = h * 6.0;
    let hi = h6.floor() as u32 % 6;
    let f = h6 - h6.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match hi {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Vec4::new(r, g, b, a)
}

// ============================================================
// BEHAVIOR TREE LIBRARY — PRE-BUILT TEMPLATES
// ============================================================

pub struct BtTemplates;

impl BtTemplates {
    /// Simple patrol + attack tree
    pub fn combat_patrol_tree() -> BehaviorTree {
        let mut tree = BehaviorTree::new("CombatPatrol");

        let root = tree.add_node(BtNodeType::Selector);
        tree.set_root(root);

        // Branch 1: Combat
        let combat_seq = tree.add_node(BtNodeType::Sequence);
        let has_target = tree.add_node(BtNodeType::BlackboardCheck {
            key: "target".to_string(),
            op: CompareOp::Exists,
            value: BlackboardValue::None,
        });
        let in_range_check = tree.add_node(BtNodeType::BlackboardCheck {
            key: "target_dist".to_string(),
            op: CompareOp::LessThan,
            value: BlackboardValue::Float(15.0),
        });
        let attack_cd = tree.add_node(BtNodeType::Cooldown { cooldown: 1.0 });
        let attack = tree.add_node(BtNodeType::Attack {
            target_key: "target_pos".to_string(),
            damage: 10.0,
            range: 2.0,
        });
        let move_to_target = tree.add_node(BtNodeType::MoveTo {
            target_key: "target_pos".to_string(),
            speed: 4.0,
            acceptance_radius: 2.0,
        });

        tree.add_child(root, combat_seq);
        tree.add_child(combat_seq, has_target);
        tree.add_child(combat_seq, in_range_check);

        let attack_or_move = tree.add_node(BtNodeType::Selector);
        tree.add_child(combat_seq, attack_or_move);
        tree.add_child(attack_or_move, attack_cd);
        tree.add_child(attack_cd, attack);
        tree.add_child(attack_or_move, move_to_target);

        // Branch 2: Investigate sound
        let investigate_seq = tree.add_node(BtNodeType::Sequence);
        let heard_sound = tree.add_node(BtNodeType::BlackboardCheck {
            key: "heard_position".to_string(),
            op: CompareOp::Exists,
            value: BlackboardValue::None,
        });
        let move_to_sound = tree.add_node(BtNodeType::MoveTo {
            target_key: "heard_position".to_string(),
            speed: 3.0,
            acceptance_radius: 1.5,
        });
        let look_around = tree.add_node(BtNodeType::Wait { duration: 2.0 });
        let clear_heard = tree.add_node(BtNodeType::SetBlackboard {
            key: "heard_position".to_string(),
            value: BlackboardValue::None,
        });
        tree.add_child(root, investigate_seq);
        tree.add_child(investigate_seq, heard_sound);
        tree.add_child(investigate_seq, move_to_sound);
        tree.add_child(investigate_seq, look_around);
        tree.add_child(investigate_seq, clear_heard);

        // Branch 3: Patrol
        let patrol = tree.add_node(BtNodeType::Patrol {
            waypoints_key: "patrol_waypoints".to_string(),
            speed: 2.0,
        });
        tree.add_child(root, patrol);

        tree
    }

    /// Flee and take cover tree
    pub fn flee_tree() -> BehaviorTree {
        let mut tree = BehaviorTree::new("FleeTakeCover");

        let root = tree.add_node(BtNodeType::Sequence);
        tree.set_root(root);

        let threat_check = tree.add_node(BtNodeType::BlackboardCheck {
            key: "threat".to_string(),
            op: CompareOp::Exists,
            value: BlackboardValue::None,
        });
        let find_cover = tree.add_node(BtNodeType::TakeCover {
            threat_key: "threat_pos".to_string(),
            result_key: "cover_pos".to_string(),
        });
        let move_to_cover = tree.add_node(BtNodeType::MoveTo {
            target_key: "cover_pos".to_string(),
            speed: 6.0,
            acceptance_radius: 1.0,
        });
        let wait_at_cover = tree.add_node(BtNodeType::Wait { duration: 3.0 });
        let alert = tree.add_node(BtNodeType::AlertAllies {
            radius: 20.0,
            message: "Enemy spotted!".to_string(),
        });

        tree.add_child(root, threat_check);
        tree.add_child(root, find_cover);
        tree.add_child(root, move_to_cover);
        tree.add_child(root, wait_at_cover);
        tree.add_child(root, alert);

        tree
    }

    /// Gather resources tree
    pub fn gather_tree() -> BehaviorTree {
        let mut tree = BehaviorTree::new("GatherResources");
        let root = tree.add_node(BtNodeType::Selector);
        tree.set_root(root);

        // Have inventory full?
        let check_full = tree.add_node(BtNodeType::BlackboardCheck {
            key: "inventory_count".to_string(),
            op: CompareOp::GreaterOrEqual,
            value: BlackboardValue::Int(10),
        });
        let return_to_base = tree.add_node(BtNodeType::Sequence);
        let move_base = tree.add_node(BtNodeType::MoveTo {
            target_key: "base_pos".to_string(),
            speed: 3.5,
            acceptance_radius: 2.0,
        });
        let deposit = tree.add_node(BtNodeType::SetBlackboard {
            key: "inventory_count".to_string(),
            value: BlackboardValue::Int(0),
        });
        tree.add_child(root, return_to_base);
        tree.add_child(return_to_base, check_full);
        tree.add_child(return_to_base, move_base);
        tree.add_child(return_to_base, deposit);

        // Gather resource
        let gather_seq = tree.add_node(BtNodeType::Sequence);
        let find_resource = tree.add_node(BtNodeType::FindTarget {
            radius: 20.0,
            faction_key: "resource".to_string(),
            result_key: "resource_pos".to_string(),
        });
        let move_to_res = tree.add_node(BtNodeType::MoveTo {
            target_key: "resource_pos".to_string(),
            speed: 3.5,
            acceptance_radius: 1.0,
        });
        let pickup = tree.add_node(BtNodeType::PickupItem {
            item_key: "resource".to_string(),
        });
        let inc_inv = tree.add_node(BtNodeType::IncrementBlackboard {
            key: "inventory_count".to_string(),
            amount: 1.0,
        });
        tree.add_child(root, gather_seq);
        tree.add_child(gather_seq, find_resource);
        tree.add_child(gather_seq, move_to_res);
        tree.add_child(gather_seq, pickup);
        tree.add_child(gather_seq, inc_inv);

        // Wander
        let idle = tree.add_node(BtNodeType::Wait { duration: 1.0 });
        tree.add_child(root, idle);

        tree
    }
}

// ============================================================
// GOAP ACTION LIBRARY
// ============================================================

pub struct GoapLibrary;

impl GoapLibrary {
    /// Combat agent GOAP
    pub fn build_combat_planner() -> GoapPlanner {
        let mut planner = GoapPlanner::new();
        // World state bits
        // 0: has_ammo
        // 1: enemy_visible
        // 2: enemy_dead
        // 3: in_cover
        // 4: health_low
        // 5: has_medpack
        // 6: enemy_alerted

        planner.label_bit(0, "has_ammo");
        planner.label_bit(1, "enemy_visible");
        planner.label_bit(2, "enemy_dead");
        planner.label_bit(3, "in_cover");
        planner.label_bit(4, "health_low");
        planner.label_bit(5, "has_medpack");
        planner.label_bit(6, "enemy_alerted");

        // Action: Shoot enemy
        let mut shoot = GoapAction::new(1, "Shoot");
        shoot.preconditions = (1 << 0) | (1 << 1); // has_ammo + enemy_visible
        shoot.effects_clear = 1 << 1;               // clear enemy_visible (they die or flee)
        shoot.effects_set = 1 << 2;                 // enemy_dead (optimistic)
        shoot.cost = 1.0;
        planner.add_action(shoot);

        // Action: Find cover
        let mut find_cover = GoapAction::new(2, "FindCover");
        find_cover.preconditions = 1 << 1;           // enemy_visible
        find_cover.effects_set = 1 << 3;             // in_cover
        find_cover.cost = 2.0;
        planner.add_action(find_cover);

        // Action: Use medpack
        let mut heal = GoapAction::new(3, "Heal");
        heal.preconditions = (1 << 4) | (1 << 5);   // health_low + has_medpack
        heal.effects_clear = (1 << 4) | (1 << 5);   // clear both
        heal.cost = 1.0;
        planner.add_action(heal);

        // Action: Reload
        let mut reload = GoapAction::new(4, "Reload");
        reload.preconditions_false = 1 << 0;         // doesn't have ammo
        reload.effects_set = 1 << 0;                 // has ammo
        reload.cost = 1.5;
        planner.add_action(reload);

        // Action: Patrol
        let mut patrol = GoapAction::new(5, "Patrol");
        patrol.preconditions = 0;
        patrol.effects_set = 1 << 1;                 // might spot enemy
        patrol.cost = 3.0;
        planner.add_action(patrol);

        // Action: Alert allies
        let mut alert = GoapAction::new(6, "AlertAllies");
        alert.preconditions = 1 << 1;               // enemy_visible
        alert.effects_set = 1 << 6;                  // enemy_alerted
        alert.cost = 0.5;
        planner.add_action(alert);

        // Action: Melee attack
        let mut melee = GoapAction::new(7, "Melee");
        melee.preconditions = 1 << 1;               // enemy visible
        melee.preconditions_false = 1 << 0;         // no ammo
        melee.effects_set = 1 << 2;                  // enemy dead
        melee.effects_clear = 1 << 1;
        melee.cost = 1.5;
        planner.add_action(melee);

        planner
    }
}

// ============================================================
// UTILITY AI LIBRARY
// ============================================================

pub struct UtilityLibrary;

impl UtilityLibrary {
    pub fn build_combat_decision_maker() -> UtilityDecisionMaker {
        let mut dm = UtilityDecisionMaker::new();

        // Attack action
        let mut attack = UtilityAction::new(1, "Attack");
        attack.considerations.push(Consideration {
            name: "health".to_string(),
            input_key: "self_health".to_string(),
            input_min: 0.0,
            input_max: 100.0,
            curve: ResponseCurve::Linear { slope: 1.0, intercept: 0.0 },
            weight: 1.0,
        });
        attack.considerations.push(Consideration {
            name: "enemy_visible".to_string(),
            input_key: "enemy_visible".to_string(),
            input_min: 0.0,
            input_max: 1.0,
            curve: ResponseCurve::Step { threshold: 0.5, low: 0.0, high: 1.0 },
            weight: 2.0,
        });
        attack.considerations.push(Consideration {
            name: "ammo".to_string(),
            input_key: "ammo_count".to_string(),
            input_min: 0.0,
            input_max: 30.0,
            curve: ResponseCurve::Smoothstep { edge0: 0.0, edge1: 0.5 },
            weight: 1.5,
        });
        dm.add_action(attack);

        // Flee action
        let mut flee = UtilityAction::new(2, "Flee");
        flee.considerations.push(Consideration {
            name: "health_low".to_string(),
            input_key: "self_health".to_string(),
            input_min: 0.0,
            input_max: 100.0,
            curve: ResponseCurve::Logistic { steepness: -10.0, midpoint: 0.3 },
            weight: 2.0,
        });
        flee.considerations.push(Consideration {
            name: "threat_distance".to_string(),
            input_key: "threat_dist".to_string(),
            input_min: 0.0,
            input_max: 20.0,
            curve: ResponseCurve::Inverse { scale: 0.3 },
            weight: 1.0,
        });
        dm.add_action(flee);

        // Heal action
        let mut heal = UtilityAction::new(3, "Heal");
        heal.considerations.push(Consideration {
            name: "need_heal".to_string(),
            input_key: "self_health".to_string(),
            input_min: 0.0,
            input_max: 100.0,
            curve: ResponseCurve::Logistic { steepness: -8.0, midpoint: 0.4 },
            weight: 2.0,
        });
        heal.considerations.push(Consideration {
            name: "has_medpack".to_string(),
            input_key: "medpack_count".to_string(),
            input_min: 0.0,
            input_max: 5.0,
            curve: ResponseCurve::Step { threshold: 0.15, low: 0.0, high: 1.0 },
            weight: 1.5,
        });
        dm.add_action(heal);

        // Patrol action
        let mut patrol = UtilityAction::new(4, "Patrol");
        patrol.considerations.push(Consideration {
            name: "boredom".to_string(),
            input_key: "idle_time".to_string(),
            input_min: 0.0,
            input_max: 30.0,
            curve: ResponseCurve::Exponential { base: 2.0, exponent: 1.5, scale: 0.5 },
            weight: 1.0,
        });
        patrol.bonus_score = 0.1;
        dm.add_action(patrol);

        // Reload
        let mut reload = UtilityAction::new(5, "Reload");
        reload.considerations.push(Consideration {
            name: "ammo_low".to_string(),
            input_key: "ammo_count".to_string(),
            input_min: 0.0,
            input_max: 30.0,
            curve: ResponseCurve::Logistic { steepness: -8.0, midpoint: 0.2 },
            weight: 2.0,
        });
        reload.considerations.push(Consideration {
            name: "not_in_danger".to_string(),
            input_key: "threat_dist".to_string(),
            input_min: 0.0,
            input_max: 20.0,
            curve: ResponseCurve::Smoothstep { edge0: 0.3, edge1: 0.8 },
            weight: 1.0,
        });
        dm.add_action(reload);

        dm
    }
}

// ============================================================
// AI AGENT — TOP-LEVEL INTEGRATION
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiAgentMode {
    BehaviorTree,
    UtilityAi,
    Goap,
    Fsm,
    Hybrid,  // BT orchestrates, Utility selects leaves
}

pub struct AiAgent {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
    pub velocity: Vec3,
    pub heading: Vec3,
    pub blackboard: Blackboard,
    pub mode: AiAgentMode,
    pub behavior_tree: Option<BehaviorTree>,
    pub utility_dm: Option<UtilityDecisionMaker>,
    pub goap_planner: Option<GoapPlanner>,
    pub goap_world_state: WorldState,
    pub goap_goal_state: WorldState,
    pub goap_current_plan: Option<Vec<usize>>,
    pub goap_plan_step: usize,
    pub fsm: Option<FsmInstance>,
    pub perception: PerceptionSystem,
    pub emotion_engine: EmotionEngine,
    pub steering_agent: SteeringAgent,
    pub formation_slot: Option<Vec3>,
    pub formation_type: FormationType,
    pub current_time: f32,
    pub debug_enabled: bool,
}

impl AiAgent {
    pub fn new(id: u64, name: &str, position: Vec3, mode: AiAgentMode) -> Self {
        Self {
            id,
            name: name.to_string(),
            position,
            velocity: Vec3::ZERO,
            heading: Vec3::Z,
            blackboard: Blackboard::new(),
            mode,
            behavior_tree: None,
            utility_dm: None,
            goap_planner: None,
            goap_world_state: 0,
            goap_goal_state: 0,
            goap_current_plan: None,
            goap_plan_step: 0,
            fsm: None,
            perception: PerceptionSystem::new(id),
            emotion_engine: EmotionEngine::new(),
            steering_agent: SteeringAgent::new(id, position, 5.0, 10.0),
            formation_slot: None,
            formation_type: FormationType::Line,
            current_time: 0.0,
            debug_enabled: false,
        }
    }

    pub fn update(&mut self, dt: f32, obstacles: &[Aabb]) {
        self.current_time += dt;
        self.blackboard.advance_time(dt as f64);
        self.blackboard.set("agent_position", BlackboardValue::Vec3(self.position));
        self.blackboard.set("current_time", BlackboardValue::Float(self.current_time));

        // Update emotion engine
        self.emotion_engine.update(dt, self.current_time);
        let modifiers = self.emotion_engine.get_modifier();
        self.emotion_engine.state.serialize_to_blackboard(&mut self.blackboard, "emotion");
        self.blackboard.set("speed_mult", BlackboardValue::Float(modifiers.speed_multiplier));

        match self.mode {
            AiAgentMode::BehaviorTree => {
                if let Some(ref mut tree) = self.behavior_tree {
                    let mut ctx = BtTickContext::new(
                        &mut self.blackboard, dt, self.current_time, self.position, self.id
                    );
                    tree.tick(&mut ctx);
                    self.position = ctx.agent_position;
                }
            }
            AiAgentMode::UtilityAi => {
                if let Some(ref mut dm) = self.utility_dm {
                    let selected = dm.evaluate(&self.blackboard, self.current_time);
                    if let Some(action_id) = selected {
                        self.blackboard.set("utility_selected_action", BlackboardValue::Int(action_id as i64));
                    }
                }
            }
            AiAgentMode::Goap => {
                self.tick_goap(dt);
            }
            AiAgentMode::Fsm => {
                if let Some(ref mut fsm) = self.fsm {
                    fsm.tick(&mut self.blackboard, dt, self.current_time);
                }
            }
            AiAgentMode::Hybrid => {
                // Run utility AI to determine high-level goal, then BT executes
                if let Some(ref mut dm) = self.utility_dm {
                    dm.evaluate(&self.blackboard, self.current_time);
                }
                if let Some(ref mut tree) = self.behavior_tree {
                    let mut ctx = BtTickContext::new(
                        &mut self.blackboard, dt, self.current_time, self.position, self.id
                    );
                    tree.tick(&mut ctx);
                    self.position = ctx.agent_position;
                }
            }
        }

        // Update steering
        self.steering_agent.position = self.position;
        self.steering_agent.velocity = self.velocity;
    }

    fn tick_goap(&mut self, dt: f32) {
        let planner = match &self.goap_planner { Some(p) => p, None => return };

        // Re-plan if needed
        if self.goap_current_plan.is_none() || self.goap_plan_step >= self.goap_current_plan.as_ref().map(|p| p.len()).unwrap_or(0) {
            let plan = planner.plan(self.goap_world_state, self.goap_goal_state, self.current_time);
            self.goap_current_plan = plan;
            self.goap_plan_step = 0;
        }

        if let Some(ref plan) = self.goap_current_plan {
            if self.goap_plan_step < plan.len() {
                let action_idx = plan[self.goap_plan_step];
                if action_idx < planner.actions.len() {
                    let action = &planner.actions[action_idx];
                    // Execute action (tick for dt — simplified: complete in one tick)
                    self.goap_world_state = action.apply(self.goap_world_state);
                    self.blackboard.set("goap_action", BlackboardValue::String(action.name.clone()));
                    self.goap_plan_step += 1;
                }
            }
        }
    }
}

// ============================================================
// FULL AI BEHAVIOR EDITOR STRUCT
// ============================================================

pub struct AiBehaviorEditor {
    // Behavior Tree editor
    pub active_tree_index: usize,
    pub behavior_trees: Vec<BehaviorTree>,
    pub bt_layout: ReingoldTilford,
    pub bt_selection: GraphSelection,
    pub bt_camera: NodeGraphCamera,
    pub bt_connection_draft: Option<ConnectionDraft>,
    pub bt_tool: EditorTool,
    pub bt_node_palette: Vec<(String, BtNodeType)>,
    pub bt_comments: Vec<NodeComment>,
    pub bt_undo_stack: Vec<BtUndoEntry>,
    pub bt_redo_stack: Vec<BtUndoEntry>,

    // FSM editor
    pub active_fsm_index: usize,
    pub fsm_instances: Vec<FsmInstance>,
    pub fsm_camera: NodeGraphCamera,
    pub fsm_selection: HashSet<u32>,
    pub fsm_tool: EditorTool,
    pub fsm_transition_draft: Option<(u32, Vec2)>,

    // GOAP editor
    pub goap_planner: GoapPlanner,
    pub goap_world_state: WorldState,
    pub goap_goal_state: WorldState,
    pub goap_last_plan: Option<Vec<usize>>,
    pub goap_action_editor_open: bool,
    pub goap_selected_action: Option<u32>,

    // Utility AI editor
    pub utility_dm: UtilityDecisionMaker,
    pub utility_selected_action: Option<u32>,
    pub utility_curve_editor_open: bool,
    pub utility_selected_consideration: Option<(u32, usize)>,  // (action_id, consideration_idx)
    pub utility_curve_preview_points: Vec<Vec2>,

    // Perception inspector
    pub perception_systems: Vec<PerceptionSystem>,
    pub selected_perception_agent: Option<u64>,
    pub perception_debug_draw: bool,

    // Formation editor
    pub formation_preview: FormationType,
    pub formation_n_agents: usize,
    pub formation_spacing: f32,
    pub formation_preview_slots: Vec<Vec3>,

    // Steering editor
    pub steering_agents: Vec<SteeringAgent>,
    pub steering_debug_draw: bool,
    pub steering_selected_agent: Option<u64>,

    // Emotion editor
    pub emotion_engines: Vec<EmotionEngine>,
    pub selected_emotion_agent: usize,
    pub emotion_debug_draw: bool,

    // Blackboard inspector
    pub blackboard_inspector: BlackboardInspector,
    pub shared_blackboard: Blackboard,

    // Debug visualization
    pub debug_buffer: DebugVisualizationBuffer,
    pub show_debug_panel: bool,

    // Agents (live sim)
    pub agents: Vec<AiAgent>,
    pub selected_agent_id: Option<u64>,
    pub simulation_running: bool,
    pub simulation_speed: f32,
    pub obstacles: Vec<Aabb>,
    pub flow_field: HashMap<(i32, i32), Vec3>,

    // Editor global state
    pub current_time: f32,
    pub frame_dt: f32,
    pub panel_sizes: HashMap<String, Vec2>,
    pub theme_color: Vec4,
    pub font_size: f32,
    pub grid_visible: bool,
    pub grid_size: f32,
    pub snap_to_grid: bool,
    pub status_message: String,
    pub status_timer: f32,
}

// Undo / redo for BT editor
#[derive(Clone, Debug)]
pub enum BtUndoEntry {
    AddNode { tree_idx: usize, node_id: u32, node: BtNode },
    RemoveNode { tree_idx: usize, node_id: u32, node: BtNode },
    AddChild { tree_idx: usize, parent_id: u32, child_id: u32, index: usize },
    RemoveChild { tree_idx: usize, parent_id: u32, child_id: u32 },
    MoveNode { tree_idx: usize, node_id: u32, old_pos: Vec2, new_pos: Vec2 },
    ChangeNodeType { tree_idx: usize, node_id: u32, old_type: BtNodeType, new_type: BtNodeType },
}

impl AiBehaviorEditor {
    pub fn new() -> Self {
        let mut editor = Self {
            active_tree_index: 0,
            behavior_trees: Vec::new(),
            bt_layout: ReingoldTilford::new(),
            bt_selection: GraphSelection::new(),
            bt_camera: NodeGraphCamera::new(),
            bt_connection_draft: None,
            bt_tool: EditorTool::Select,
            bt_node_palette: Self::build_node_palette(),
            bt_comments: Vec::new(),
            bt_undo_stack: Vec::with_capacity(64),
            bt_redo_stack: Vec::with_capacity(64),

            active_fsm_index: 0,
            fsm_instances: Vec::new(),
            fsm_camera: NodeGraphCamera::new(),
            fsm_selection: HashSet::new(),
            fsm_tool: EditorTool::Select,
            fsm_transition_draft: None,

            goap_planner: GoapLibrary::build_combat_planner(),
            goap_world_state: 0b0000_0001,   // has_ammo
            goap_goal_state:  0b0000_0100,   // enemy_dead
            goap_last_plan: None,
            goap_action_editor_open: false,
            goap_selected_action: None,

            utility_dm: UtilityLibrary::build_combat_decision_maker(),
            utility_selected_action: None,
            utility_curve_editor_open: false,
            utility_selected_consideration: None,
            utility_curve_preview_points: Vec::new(),

            perception_systems: Vec::new(),
            selected_perception_agent: None,
            perception_debug_draw: true,

            formation_preview: FormationType::Wedge,
            formation_n_agents: 8,
            formation_spacing: 2.0,
            formation_preview_slots: Vec::new(),

            steering_agents: Vec::new(),
            steering_debug_draw: true,
            steering_selected_agent: None,

            emotion_engines: Vec::new(),
            selected_emotion_agent: 0,
            emotion_debug_draw: false,

            blackboard_inspector: BlackboardInspector::new(),
            shared_blackboard: Blackboard::new(),

            debug_buffer: DebugVisualizationBuffer::new(4096),
            show_debug_panel: true,

            agents: Vec::new(),
            selected_agent_id: None,
            simulation_running: false,
            simulation_speed: 1.0,
            obstacles: Vec::new(),
            flow_field: HashMap::new(),

            current_time: 0.0,
            frame_dt: 0.0,
            panel_sizes: HashMap::new(),
            theme_color: Vec4::new(0.18, 0.2, 0.25, 1.0),
            font_size: 14.0,
            grid_visible: true,
            grid_size: 20.0,
            snap_to_grid: false,
            status_message: String::new(),
            status_timer: 0.0,
        };

        // Add default trees
        editor.behavior_trees.push(BtTemplates::combat_patrol_tree());
        editor.behavior_trees.push(BtTemplates::flee_tree());
        editor.behavior_trees.push(BtTemplates::gather_tree());

        // Add default FSM
        let mut fsm = FsmInstance::new(1, "CombatFsm");
        let patrol_s = fsm.add_state("Patrol");
        let engage_s = fsm.add_state("Engage");
        let cover_s = fsm.add_state("TakeCover");
        let dead_s = fsm.add_state("Dead");
        fsm.set_initial(patrol_s);
        if let Some(s) = fsm.states.get_mut(&dead_s) { s.is_final = true; }
        fsm.add_transition(patrol_s, engage_s, FsmConditionOp::BlackboardBool {
            key: "enemy_visible".to_string(), expected: true
        }, 10);
        fsm.add_transition(engage_s, cover_s, FsmConditionOp::BlackboardCompare {
            key: "self_health".to_string(),
            op: CompareOp::LessThan,
            value: BlackboardValue::Float(0.3),
        }, 20);
        fsm.add_transition(cover_s, engage_s, FsmConditionOp::TimeElapsed { duration: 5.0 }, 5);
        fsm.add_transition(engage_s, patrol_s, FsmConditionOp::BlackboardBool {
            key: "enemy_visible".to_string(), expected: false
        }, 5);
        fsm.add_transition(engage_s, dead_s, FsmConditionOp::BlackboardCompare {
            key: "self_health".to_string(),
            op: CompareOp::LessOrEqual,
            value: BlackboardValue::Float(0.0),
        }, 100);
        fsm.auto_layout();
        editor.fsm_instances.push(fsm);

        // Layout trees
        for tree in &mut editor.behavior_trees {
            editor.bt_layout.layout(tree);
        }

        // Spawn a few demo agents
        for i in 0..4 {
            let pos = Vec3::new(i as f32 * 5.0, 0.0, 0.0);
            let mut agent = AiAgent::new(i as u64 + 1, &format!("Agent_{}", i), pos, AiAgentMode::BehaviorTree);
            agent.behavior_tree = Some(BtTemplates::combat_patrol_tree());
            agent.blackboard.set("self_health", BlackboardValue::Float(1.0));
            agent.blackboard.set("ammo_count", BlackboardValue::Float(30.0));
            // Set patrol waypoints
            for wp_i in 0..4 {
                let wp_pos = Vec3::new(
                    pos.x + (wp_i as f32 * 4.0 - 8.0),
                    0.0,
                    ((wp_i as f32 + 0.5) * PI * 0.5).sin() * 5.0,
                );
                agent.blackboard.set(
                    &format!("patrol_waypoints_{}", wp_i),
                    BlackboardValue::Vec3(wp_pos),
                );
            }
            editor.agents.push(agent);
        }

        // Add some obstacles
        editor.obstacles.push(Aabb::new(Vec3::new(10.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 2.0)));
        editor.obstacles.push(Aabb::new(Vec3::new(-5.0, 0.0, 8.0), Vec3::new(1.5, 1.0, 1.5)));

        // Compute initial formation preview
        editor.recompute_formation_preview();

        // Run initial GOAP plan
        editor.goap_last_plan = editor.goap_planner.plan(
            editor.goap_world_state,
            editor.goap_goal_state,
            0.0,
        );

        editor
    }

    fn build_node_palette() -> Vec<(String, BtNodeType)> {
        vec![
            ("Sequence".to_string(), BtNodeType::Sequence),
            ("Selector".to_string(), BtNodeType::Selector),
            ("Parallel (All)".to_string(), BtNodeType::ParallelAll),
            ("Parallel (Any)".to_string(), BtNodeType::ParallelAny),
            ("Random Selector".to_string(), BtNodeType::RandomSelector),
            ("Random Sequence".to_string(), BtNodeType::RandomSequence),
            ("Inverter".to_string(), BtNodeType::Inverter),
            ("Repeater x3".to_string(), BtNodeType::Repeater { times: 3 }),
            ("Repeat Forever".to_string(), BtNodeType::RepeatForever),
            ("Retry Until Success".to_string(), BtNodeType::RetryUntilSuccess { max_retries: 5 }),
            ("Timeout 5s".to_string(), BtNodeType::Timeout { duration: 5.0 }),
            ("Cooldown 2s".to_string(), BtNodeType::Cooldown { cooldown: 2.0 }),
            ("Succeeder".to_string(), BtNodeType::Succeeder),
            ("Failer".to_string(), BtNodeType::Failer),
            ("Until Fail".to_string(), BtNodeType::UntilFail),
            ("Until Success".to_string(), BtNodeType::UntilSuccess),
            ("BB Check".to_string(), BtNodeType::BlackboardCheck {
                key: "var".to_string(), op: CompareOp::GreaterThan, value: BlackboardValue::Float(0.0)
            }),
            ("BB Guard".to_string(), BtNodeType::BlackboardGuard { key: "var".to_string() }),
            ("Move To".to_string(), BtNodeType::MoveTo { target_key: "target_pos".to_string(), speed: 3.5, acceptance_radius: 1.0 }),
            ("Attack".to_string(), BtNodeType::Attack { target_key: "target_pos".to_string(), damage: 10.0, range: 2.0 }),
            ("Play Animation".to_string(), BtNodeType::PlayAnimation { clip: "idle".to_string(), layer: 0, blend_time: 0.2 }),
            ("Set Blackboard".to_string(), BtNodeType::SetBlackboard { key: "var".to_string(), value: BlackboardValue::Bool(true) }),
            ("Increment BB".to_string(), BtNodeType::IncrementBlackboard { key: "counter".to_string(), amount: 1.0 }),
            ("Wait 1s".to_string(), BtNodeType::Wait { duration: 1.0 }),
            ("Log".to_string(), BtNodeType::Log { message: "Hello".to_string() }),
            ("Idle".to_string(), BtNodeType::Idle),
            ("Find Target".to_string(), BtNodeType::FindTarget { radius: 15.0, faction_key: "enemy".to_string(), result_key: "target".to_string() }),
            ("Flee".to_string(), BtNodeType::Flee { threat_key: "threat_pos".to_string(), speed: 6.0, distance: 10.0 }),
            ("Patrol".to_string(), BtNodeType::Patrol { waypoints_key: "waypoints".to_string(), speed: 2.5 }),
            ("Take Cover".to_string(), BtNodeType::TakeCover { threat_key: "threat_pos".to_string(), result_key: "cover_pos".to_string() }),
            ("Alert Allies".to_string(), BtNodeType::AlertAllies { radius: 20.0, message: "Alert!".to_string() }),
            ("Play Sound".to_string(), BtNodeType::PlaySound { sound: "alert.wav".to_string(), volume: 1.0 }),
            ("Send Event".to_string(), BtNodeType::SendEvent { event_name: "on_spotted".to_string(), payload_key: "target_id".to_string() }),
            ("Succeed".to_string(), BtNodeType::SucceedAlways),
            ("Fail".to_string(), BtNodeType::FailAlways),
        ]
    }

    // ---- BT EDITOR OPERATIONS ----

    pub fn bt_add_node(&mut self, tree_idx: usize, node_type: BtNodeType) -> Option<u32> {
        let tree = self.behavior_trees.get_mut(tree_idx)?;
        let id = tree.add_node(node_type.clone());
        self.bt_undo_stack.push(BtUndoEntry::AddNode {
            tree_idx,
            node_id: id,
            node: tree.nodes[&id].clone(),
        });
        self.bt_redo_stack.clear();
        Some(id)
    }

    pub fn bt_remove_node(&mut self, tree_idx: usize, node_id: u32) {
        let tree = match self.behavior_trees.get_mut(tree_idx) { Some(t) => t, None => return };
        if let Some(node) = tree.nodes.remove(&node_id) {
            // Remove from parent
            if let Some(parent_id) = node.parent {
                if let Some(parent) = tree.nodes.get_mut(&parent_id) {
                    parent.children.retain(|&c| c != node_id);
                }
            }
            // Orphan children
            for child_id in &node.children {
                if let Some(child) = tree.nodes.get_mut(child_id) {
                    child.parent = None;
                }
            }
            // Reset root if needed
            if tree.root_id == Some(node_id) { tree.root_id = None; }
            self.bt_undo_stack.push(BtUndoEntry::RemoveNode { tree_idx, node_id, node });
        }
    }

    pub fn bt_connect_nodes(&mut self, tree_idx: usize, parent_id: u32, child_id: u32) {
        let tree = match self.behavior_trees.get_mut(tree_idx) { Some(t) => t, None => return };
        // Remove from old parent if any
        let old_parent = tree.nodes.get(&child_id).and_then(|n| n.parent);
        if let Some(op) = old_parent {
            if let Some(op_node) = tree.nodes.get_mut(&op) {
                op_node.children.retain(|&c| c != child_id);
            }
        }
        let child_idx = tree.nodes.get(&parent_id).map(|n| n.children.len()).unwrap_or(0);
        tree.add_child(parent_id, child_id);
        self.bt_undo_stack.push(BtUndoEntry::AddChild { tree_idx, parent_id, child_id, index: child_idx });
        self.bt_redo_stack.clear();
        // Re-run layout
        if let Some(tree) = self.behavior_trees.get_mut(tree_idx) {
            self.bt_layout.layout(tree);
        }
    }

    pub fn bt_move_node(&mut self, tree_idx: usize, node_id: u32, new_pos: Vec2) {
        let tree = match self.behavior_trees.get_mut(tree_idx) { Some(t) => t, None => return };
        let old_pos = tree.nodes.get(&node_id).map(|n| n.position).unwrap_or(Vec2::ZERO);
        if let Some(node) = tree.nodes.get_mut(&node_id) {
            node.position = if self.snap_to_grid {
                let g = self.grid_size;
                Vec2::new((new_pos.x / g).round() * g, (new_pos.y / g).round() * g)
            } else { new_pos };
        }
        self.bt_undo_stack.push(BtUndoEntry::MoveNode { tree_idx, node_id, old_pos, new_pos });
        self.bt_redo_stack.clear();
    }

    pub fn bt_auto_layout(&mut self, tree_idx: usize) {
        if let Some(tree) = self.behavior_trees.get_mut(tree_idx) {
            self.bt_layout.layout(tree);
        }
    }

    pub fn bt_undo(&mut self) {
        if let Some(entry) = self.bt_undo_stack.pop() {
            match &entry {
                BtUndoEntry::AddNode { tree_idx, node_id, .. } => {
                    let t = *tree_idx;
                    let nid = *node_id;
                    self.bt_remove_node(t, nid);
                }
                BtUndoEntry::RemoveNode { tree_idx, node_id, node } => {
                    let t = *tree_idx;
                    let nid = *node_id;
                    let n = node.clone();
                    if let Some(tree) = self.behavior_trees.get_mut(t) {
                        tree.nodes.insert(nid, n);
                    }
                }
                BtUndoEntry::MoveNode { tree_idx, node_id, old_pos, .. } => {
                    let t = *tree_idx;
                    let nid = *node_id;
                    let op = *old_pos;
                    if let Some(tree) = self.behavior_trees.get_mut(t) {
                        if let Some(node) = tree.nodes.get_mut(&nid) {
                            node.position = op;
                        }
                    }
                }
                _ => {}
            }
            self.bt_redo_stack.push(entry);
        }
    }

    pub fn bt_redo(&mut self) {
        if let Some(entry) = self.bt_redo_stack.pop() {
            match &entry {
                BtUndoEntry::MoveNode { tree_idx, node_id, new_pos, .. } => {
                    let t = *tree_idx;
                    let nid = *node_id;
                    let np = *new_pos;
                    self.bt_move_node(t, nid, np);
                }
                _ => {}
            }
        }
    }

    pub fn bt_duplicate_node(&mut self, tree_idx: usize, node_id: u32) -> Option<u32> {
        let tree = self.behavior_trees.get(tree_idx)?;
        let original = tree.nodes.get(&node_id)?.clone();
        let new_node_type = original.node_type.clone();
        let new_id = self.bt_add_node(tree_idx, new_node_type)?;
        let tree = self.behavior_trees.get_mut(tree_idx)?;
        if let Some(new_node) = tree.nodes.get_mut(&new_id) {
            new_node.position = original.position + Vec2::new(REINGOLD_NODE_WIDTH + 10.0, 0.0);
        }
        Some(new_id)
    }

    pub fn bt_select_all(&mut self, tree_idx: usize) {
        if let Some(tree) = self.behavior_trees.get(tree_idx) {
            self.bt_selection.selected_nodes = tree.nodes.keys().copied().collect();
        }
    }

    pub fn bt_delete_selected(&mut self, tree_idx: usize) {
        let selected: Vec<u32> = self.bt_selection.selected_nodes.iter().copied().collect();
        for id in selected {
            self.bt_remove_node(tree_idx, id);
        }
        self.bt_selection.clear();
        if let Some(tree) = self.behavior_trees.get_mut(tree_idx) {
            self.bt_layout.layout(tree);
        }
    }

    pub fn bt_hit_test(&self, tree_idx: usize, world_pos: Vec2) -> Option<u32> {
        let tree = self.behavior_trees.get(tree_idx)?;
        for (id, node) in &tree.nodes {
            let min = node.position;
            let max = node.position + node.size;
            if world_pos.x >= min.x && world_pos.x <= max.x
            && world_pos.y >= min.y && world_pos.y <= max.y {
                return Some(*id);
            }
        }
        None
    }

    pub fn bt_get_node_color(&self, node: &BtNode) -> Vec4 {
        if node.is_selected {
            return Vec4::new(1.0, 0.9, 0.3, 1.0);
        }
        match node.status {
            BtStatus::Success => Vec4::new(0.2, 0.7, 0.2, 1.0),
            BtStatus::Failure => Vec4::new(0.7, 0.2, 0.2, 1.0),
            BtStatus::Running => Vec4::new(0.7, 0.7, 0.1, 1.0),
            BtStatus::Invalid => {
                if node.is_composite() { Vec4::new(0.3, 0.4, 0.7, 1.0) }
                else if node.is_decorator() { Vec4::new(0.5, 0.3, 0.7, 1.0) }
                else { Vec4::new(0.2, 0.5, 0.3, 1.0) }
            }
        }
    }

    pub fn bt_get_bezier_control_points(from: Vec2, to: Vec2) -> (Vec2, Vec2, Vec2, Vec2) {
        let mid_y = (from.y + to.y) * 0.5;
        let p0 = from;
        let p1 = Vec2::new(from.x, mid_y);
        let p2 = Vec2::new(to.x, mid_y);
        let p3 = to;
        (p0, p1, p2, p3)
    }

    pub fn bt_bezier_point(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
        let u = 1.0 - t;
        p0 * (u * u * u)
        + p1 * (3.0 * u * u * t)
        + p2 * (3.0 * u * t * t)
        + p3 * (t * t * t)
    }

    pub fn bt_get_edge_polyline(from: Vec2, to: Vec2, num_points: usize) -> Vec<Vec2> {
        let (p0, p1, p2, p3) = Self::bt_get_bezier_control_points(from, to);
        (0..num_points).map(|i| {
            let t = i as f32 / (num_points - 1).max(1) as f32;
            Self::bt_bezier_point(p0, p1, p2, p3, t)
        }).collect()
    }

    // ---- FSM EDITOR OPERATIONS ----

    pub fn fsm_add_state(&mut self, fsm_idx: usize, name: &str, pos: Vec2) -> Option<u32> {
        let fsm = self.fsm_instances.get_mut(fsm_idx)?;
        let id = fsm.add_state(name);
        if let Some(s) = fsm.states.get_mut(&id) { s.position = pos; }
        Some(id)
    }

    pub fn fsm_add_transition(&mut self, fsm_idx: usize, from: u32, to: u32, condition: FsmConditionOp, priority: i32) -> Option<u32> {
        let fsm = self.fsm_instances.get_mut(fsm_idx)?;
        let id = fsm.add_transition(from, to, condition, priority);
        Some(id)
    }

    pub fn fsm_remove_state(&mut self, fsm_idx: usize, state_id: u32) {
        let fsm = match self.fsm_instances.get_mut(fsm_idx) { Some(f) => f, None => return };
        fsm.states.remove(&state_id);
        fsm.transitions.retain(|t| t.from_state != state_id && t.to_state != state_id);
    }

    pub fn fsm_remove_transition(&mut self, fsm_idx: usize, transition_id: u32) {
        let fsm = match self.fsm_instances.get_mut(fsm_idx) { Some(f) => f, None => return };
        fsm.transitions.retain(|t| t.id != transition_id);
    }

    pub fn fsm_transition_midpoint(&self, fsm_idx: usize, transition: &FsmTransition) -> Vec2 {
        let fsm = match self.fsm_instances.get(fsm_idx) { Some(f) => f, None => return Vec2::ZERO };
        let from_pos = fsm.states.get(&transition.from_state).map(|s| s.position).unwrap_or(Vec2::ZERO);
        let to_pos = fsm.states.get(&transition.to_state).map(|s| s.position).unwrap_or(Vec2::ZERO);
        (from_pos + to_pos) * 0.5
    }

    pub fn fsm_hit_test_state(&self, fsm_idx: usize, world_pos: Vec2, state_radius: f32) -> Option<u32> {
        let fsm = self.fsm_instances.get(fsm_idx)?;
        for (id, state) in &fsm.states {
            if (state.position - world_pos).length() <= state_radius {
                return Some(*id);
            }
        }
        None
    }

    // ---- GOAP EDITOR OPERATIONS ----

    pub fn goap_add_action(&mut self, action: GoapAction) {
        self.goap_planner.add_action(action);
        self.goap_replan();
    }

    pub fn goap_remove_action(&mut self, action_id: u32) {
        self.goap_planner.actions.retain(|a| a.id != action_id);
        self.goap_replan();
    }

    pub fn goap_replan(&mut self) {
        self.goap_last_plan = self.goap_planner.plan(
            self.goap_world_state,
            self.goap_goal_state,
            self.current_time,
        );
    }

    pub fn goap_toggle_world_state_bit(&mut self, bit: u8) {
        self.goap_world_state ^= 1 << bit;
        self.goap_replan();
    }

    pub fn goap_toggle_goal_bit(&mut self, bit: u8) {
        self.goap_goal_state ^= 1 << bit;
        self.goap_replan();
    }

    pub fn goap_plan_to_names(&self) -> Vec<String> {
        if let Some(ref plan) = self.goap_last_plan {
            plan.iter().filter_map(|&idx| {
                self.goap_planner.actions.get(idx).map(|a| a.name.clone())
            }).collect()
        } else {
            vec!["[No plan found]".to_string()]
        }
    }

    pub fn goap_plan_total_cost(&self) -> f32 {
        if let Some(ref plan) = self.goap_last_plan {
            plan.iter().filter_map(|&idx| {
                self.goap_planner.actions.get(idx).map(|a| a.cost)
            }).sum()
        } else {
            0.0
        }
    }

    pub fn goap_simulate_plan_states(&self) -> Vec<(String, WorldState)> {
        let plan = match &self.goap_last_plan { Some(p) => p, None => return vec![] };
        let mut state = self.goap_world_state;
        let mut result = vec![("Start".to_string(), state)];
        for &idx in plan {
            if let Some(action) = self.goap_planner.actions.get(idx) {
                state = action.apply(state);
                result.push((action.name.clone(), state));
            }
        }
        result
    }

    // ---- UTILITY AI EDITOR ----

    pub fn utility_update_curve_preview(&mut self) {
        if let Some((action_id, consideration_idx)) = self.utility_selected_consideration {
            if let Some(action) = self.utility_dm.actions.iter().find(|a| a.id == action_id) {
                if let Some(consideration) = action.considerations.get(consideration_idx) {
                    self.utility_curve_preview_points = consideration.curve.sample_points(64);
                }
            }
        }
    }

    pub fn utility_score_all(&self) -> Vec<(u32, String, f32)> {
        self.utility_dm.actions.iter().map(|a| {
            let score = a.score(&self.shared_blackboard, self.current_time);
            (a.id, a.name.clone(), score)
        }).collect()
    }

    pub fn utility_set_consideration_curve(
        &mut self,
        action_id: u32,
        consideration_idx: usize,
        curve: ResponseCurve,
    ) {
        if let Some(action) = self.utility_dm.actions.iter_mut().find(|a| a.id == action_id) {
            if let Some(c) = action.considerations.get_mut(consideration_idx) {
                c.curve = curve;
            }
        }
        self.utility_update_curve_preview();
    }

    // ---- FORMATION EDITOR ----

    pub fn recompute_formation_preview(&mut self) {
        self.formation_preview_slots = FormationLayout::compute_slots(
            self.formation_preview,
            Vec3::ZERO,
            Vec3::Z,
            self.formation_n_agents,
            self.formation_spacing,
        );
    }

    pub fn set_formation(&mut self, formation: FormationType) {
        self.formation_preview = formation;
        self.recompute_formation_preview();
    }

    pub fn get_formation_debug_lines(&self) -> Vec<(Vec3, Vec3)> {
        let mut lines = Vec::new();
        let leader = Vec3::ZERO;
        for slot in &self.formation_preview_slots {
            lines.push((leader, *slot));
        }
        lines
    }

    // ---- PERCEPTION EDITOR ----

    pub fn add_perception_system(&mut self, agent_id: u64) {
        self.perception_systems.push(PerceptionSystem::new(agent_id));
    }

    pub fn perception_debug_draw(&mut self, agent_idx: usize, observer_pos: Vec3, observer_fwd: Vec3) {
        if agent_idx >= self.perception_systems.len() { return; }
        let ps = &self.perception_systems[agent_idx];
        let color_vision = Vec4::new(0.3, 0.8, 0.3, 0.7);
        let color_hearing = Vec4::new(0.3, 0.3, 0.8, 0.5);
        self.debug_buffer.draw_vision_cone(
            observer_pos, observer_fwd,
            ps.vision.half_angle, ps.vision.range, color_vision
        );
        self.debug_buffer.draw_hearing_radius(observer_pos, ps.hearing.base_radius, color_hearing);
    }

    // ---- STEERING EDITOR ----

    pub fn add_steering_agent(&mut self, id: u64, pos: Vec3) {
        self.steering_agents.push(SteeringAgent::new(id, pos, 5.0, 10.0));
    }

    pub fn tick_steering_agents(&mut self, dt: f32) {
        let n = self.steering_agents.len();
        if n == 0 { return; }

        let mut forces: Vec<Vec3> = vec![Vec3::ZERO; n];
        let agents_clone: Vec<SteeringAgent> = self.steering_agents.clone();
        let obstacles_clone = self.obstacles.clone();

        for i in 0..n {
            let neighbors: Vec<&SteeringAgent> = agents_clone.iter().enumerate()
                .filter(|(j, _)| *j != i)
                .filter(|(_, a)| (a.position - agents_clone[i].position).length() < 8.0)
                .map(|(_, a)| a)
                .collect();

            let mut rng_seed = agents_clone[i].id.wrapping_mul(0x9e3779b97f4a7c15);
            let mut agent_copy = agents_clone[i].clone();
            let force = SteeringBehaviors::compute_weighted(
                &mut agent_copy,
                None,        // no seek target for demo
                None,
                None,
                None,
                None,
                true,        // wander
                &mut rng_seed,
                dt,
                &obstacles_clone,
                &[],
                &neighbors,
                None,
                None,
                None,
                None,
                None,
            );
            // Also avoid obstacles
            let avoid = SteeringBehaviors::obstacle_avoidance(&agents_clone[i], &obstacles_clone);
            let sep = SteeringBehaviors::separation(&agents_clone[i], &neighbors, 2.0);
            forces[i] = force + avoid * 2.0 + sep * 1.5;
        }

        for (i, agent) in self.steering_agents.iter_mut().enumerate() {
            agent.apply_force(forces[i], dt);
        }
    }

    // ---- EMOTION EDITOR ----

    pub fn add_emotion_engine(&mut self) {
        self.emotion_engines.push(EmotionEngine::new());
    }

    pub fn trigger_emotion(&mut self, engine_idx: usize, emotion: PrimaryEmotion, intensity: f32) {
        if let Some(engine) = self.emotion_engines.get_mut(engine_idx) {
            engine.submit_stimulus(EmotionalStimulus {
                emotion,
                intensity,
                source_id: 0,
                decay_rate_override: None,
            });
        }
    }

    pub fn get_emotion_wheel_points(&self, engine_idx: usize, scale: f32) -> Vec<Vec2> {
        if let Some(engine) = self.emotion_engines.get(engine_idx) {
            PrimaryEmotion::ALL.iter().map(|e| {
                let intensity = engine.state.get_intensity(*e);
                let wheel_pos = e.wheel_position();
                wheel_pos * intensity * scale
            }).collect()
        } else {
            vec![]
        }
    }

    // ---- SIMULATION ----

    pub fn simulation_tick(&mut self, dt: f32) {
        if !self.simulation_running { return; }
        let effective_dt = dt * self.simulation_speed;
        self.current_time += effective_dt;
        self.frame_dt = effective_dt;

        let obstacles_clone = self.obstacles.clone();
        for agent in &mut self.agents {
            agent.update(effective_dt, &obstacles_clone);
        }

        // Update steering agents
        self.tick_steering_agents(effective_dt);

        // Update emotion engines
        for engine in &mut self.emotion_engines {
            engine.update(effective_dt, self.current_time);
        }

        // Update debug buffer
        self.debug_buffer.tick(effective_dt);

        // Draw debug for agents
        if self.show_debug_panel {
            for agent in &self.agents {
                self.debug_buffer.draw_velocity_arrow(
                    agent.position, agent.velocity,
                    Vec4::new(0.8, 0.8, 0.0, 0.9)
                );
                if let Some(ref tree) = agent.behavior_tree {
                    self.debug_buffer.draw_bt_status(agent.position, tree.last_status);
                }
            }
        }

        // Decay status message
        if self.status_timer > 0.0 {
            self.status_timer -= effective_dt;
            if self.status_timer <= 0.0 {
                self.status_message.clear();
            }
        }
    }

    pub fn show_status(&mut self, message: &str, duration: f32) {
        self.status_message = message.to_string();
        self.status_timer = duration;
    }

    pub fn spawn_agent(&mut self, position: Vec3, mode: AiAgentMode) -> u64 {
        let id = (self.agents.len() as u64) + 100;
        let mut agent = AiAgent::new(id, &format!("Agent_{}", id), position, mode);
        agent.behavior_tree = Some(BtTemplates::combat_patrol_tree());
        agent.blackboard.set("self_health", BlackboardValue::Float(1.0));
        agent.blackboard.set("ammo_count", BlackboardValue::Float(30.0));
        agent.utility_dm = Some(UtilityLibrary::build_combat_decision_maker());
        self.agents.push(agent);
        id
    }

    pub fn remove_agent(&mut self, id: u64) {
        self.agents.retain(|a| a.id != id);
        if self.selected_agent_id == Some(id) { self.selected_agent_id = None; }
    }

    pub fn get_agent(&self, id: u64) -> Option<&AiAgent> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn get_agent_mut(&mut self, id: u64) -> Option<&mut AiAgent> {
        self.agents.iter_mut().find(|a| a.id == id)
    }

    // ---- BLACKBOARD INSPECTOR ----

    pub fn inspect_blackboard(&mut self, agent_id: Option<u64>) {
        if let Some(id) = agent_id {
            if let Some(agent) = self.agents.iter().find(|a| a.id == id) {
                // Diff blackboard for changes
                for (key, value) in &agent.blackboard.entries {
                    if !self.shared_blackboard.entries.contains_key(key)
                        || self.shared_blackboard.entries[key] != *value
                    {
                        let v = value.clone();
                        let t = agent.blackboard.current_time;
                        self.blackboard_inspector.record_change(key, v, t);
                    }
                }
                self.shared_blackboard = agent.blackboard.clone();
            }
        }
    }

    pub fn blackboard_search_keys(&self, prefix: &str) -> Vec<String> {
        self.shared_blackboard.entries.keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    // ---- FLOW FIELD GENERATION ----

    pub fn generate_flow_field_toward(
        &mut self,
        target: Vec3,
        bounds_min: Vec2,
        bounds_max: Vec2,
        cell_size: f32,
    ) {
        self.flow_field.clear();
        let cols = ((bounds_max.x - bounds_min.x) / cell_size).ceil() as i32;
        let rows = ((bounds_max.y - bounds_min.y) / cell_size).ceil() as i32;
        for row in 0..rows {
            for col in 0..cols {
                let cx = bounds_min.x + col as f32 * cell_size + cell_size * 0.5;
                let cy = bounds_min.y + row as f32 * cell_size + cell_size * 0.5;
                let pos = Vec3::new(cx, 0.0, cy);
                let dir = (target - pos).normalize_or_zero();
                self.flow_field.insert((col, row), dir);
            }
        }
    }

    pub fn generate_flow_field_rotational(
        &mut self,
        center: Vec3,
        bounds_min: Vec2,
        bounds_max: Vec2,
        cell_size: f32,
        clockwise: bool,
    ) {
        self.flow_field.clear();
        let cols = ((bounds_max.x - bounds_min.x) / cell_size).ceil() as i32;
        let rows = ((bounds_max.y - bounds_min.y) / cell_size).ceil() as i32;
        for row in 0..rows {
            for col in 0..cols {
                let cx = bounds_min.x + col as f32 * cell_size + cell_size * 0.5;
                let cy = bounds_min.y + row as f32 * cell_size + cell_size * 0.5;
                let pos = Vec3::new(cx, 0.0, cy);
                let to_center = (center - pos).normalize_or_zero();
                let tangent = if clockwise {
                    Vec3::new(to_center.z, 0.0, -to_center.x)
                } else {
                    Vec3::new(-to_center.z, 0.0, to_center.x)
                };
                self.flow_field.insert((col, row), tangent);
            }
        }
    }

    // ---- EDITOR THEMING ----

    pub fn set_dark_theme(&mut self) {
        self.theme_color = Vec4::new(0.15, 0.17, 0.2, 1.0);
    }

    pub fn set_light_theme(&mut self) {
        self.theme_color = Vec4::new(0.85, 0.87, 0.9, 1.0);
    }

    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size.clamp(8.0, 32.0);
    }

    // ---- SERIALIZATION HELPERS ----

    pub fn serialize_behavior_tree(&self, tree_idx: usize) -> Option<String> {
        let tree = self.behavior_trees.get(tree_idx)?;
        let mut out = String::new();
        out.push_str(&format!("BehaviorTree: {}\n", tree.name));
        out.push_str(&format!("  Nodes: {}\n", tree.nodes.len()));
        if let Some(root) = tree.root_id {
            self.serialize_bt_node_recursive(tree, root, &mut out, 0);
        }
        Some(out)
    }

    fn serialize_bt_node_recursive(&self, tree: &BehaviorTree, node_id: u32, out: &mut String, depth: usize) {
        let indent = "  ".repeat(depth + 1);
        if let Some(node) = tree.nodes.get(&node_id) {
            out.push_str(&format!("{}[{}] {}\n", indent, node.id, node.display_name()));
            for &child_id in &node.children {
                self.serialize_bt_node_recursive(tree, child_id, out, depth + 1);
            }
        }
    }

    pub fn serialize_goap_actions(&self) -> String {
        let mut out = String::new();
        out.push_str("GOAP Actions:\n");
        for action in &self.goap_planner.actions {
            out.push_str(&format!(
                "  [{}] {} | cost={:.1} | pre={:b} | eff_set={:b}\n",
                action.id, action.name, action.cost, action.preconditions, action.effects_set
            ));
        }
        out
    }

    // ---- GRID UTILITIES ----

    pub fn grid_snap(&self, pos: Vec2) -> Vec2 {
        let g = self.grid_size;
        Vec2::new((pos.x / g).round() * g, (pos.y / g).round() * g)
    }

    pub fn grid_lines_in_view(&self, viewport_min: Vec2, viewport_max: Vec2, camera: &NodeGraphCamera, viewport_size: Vec2) -> (Vec<(Vec2, Vec2)>, Vec<(Vec2, Vec2)>) {
        let world_min = camera.screen_to_world(viewport_min, viewport_size);
        let world_max = camera.screen_to_world(viewport_max, viewport_size);
        let g = self.grid_size;
        let mut minor_lines = Vec::new();
        let mut major_lines = Vec::new();

        let x_start = (world_min.x / g).floor() as i32;
        let x_end = (world_max.x / g).ceil() as i32;
        let y_start = (world_min.y / g).floor() as i32;
        let y_end = (world_max.y / g).ceil() as i32;

        for i in x_start..=x_end {
            let x = i as f32 * g;
            let line = (Vec2::new(x, world_min.y), Vec2::new(x, world_max.y));
            if i % 5 == 0 { major_lines.push(line); } else { minor_lines.push(line); }
        }
        for j in y_start..=y_end {
            let y = j as f32 * g;
            let line = (Vec2::new(world_min.x, y), Vec2::new(world_max.x, y));
            if j % 5 == 0 { major_lines.push(line); } else { minor_lines.push(line); }
        }
        (minor_lines, major_lines)
    }

    // ---- STATISTICS / METRICS ----

    pub fn bt_tree_depth(&self, tree_idx: usize) -> usize {
        if let Some(tree) = self.behavior_trees.get(tree_idx) {
            if let Some(root) = tree.root_id {
                self.bt_node_depth(tree, root)
            } else { 0 }
        } else { 0 }
    }

    fn bt_node_depth(&self, tree: &BehaviorTree, node_id: u32) -> usize {
        if let Some(node) = tree.nodes.get(&node_id) {
            if node.children.is_empty() { 1 }
            else {
                1 + node.children.iter()
                    .map(|&c| self.bt_node_depth(tree, c))
                    .max()
                    .unwrap_or(0)
            }
        } else { 0 }
    }

    pub fn bt_leaf_count(&self, tree_idx: usize) -> usize {
        if let Some(tree) = self.behavior_trees.get(tree_idx) {
            tree.nodes.values().filter(|n| n.is_leaf()).count()
        } else { 0 }
    }

    pub fn goap_action_count(&self) -> usize { self.goap_planner.actions.len() }
    pub fn agent_count(&self) -> usize { self.agents.len() }

    pub fn selected_agent_debug_info(&self) -> Option<String> {
        let id = self.selected_agent_id?;
        let agent = self.get_agent(id)?;
        let mut info = String::new();
        info.push_str(&format!("Agent: {} (id={})\n", agent.name, agent.id));
        info.push_str(&format!("  Position: ({:.2}, {:.2}, {:.2})\n", agent.position.x, agent.position.y, agent.position.z));
        info.push_str(&format!("  Mode: {:?}\n", agent.mode));
        if let Some(ref tree) = agent.behavior_tree {
            info.push_str(&format!("  BT: {} | status={:?} | ticks={}\n", tree.name, tree.last_status, tree.tick_count));
        }
        let dominant = agent.emotion_engine.state.dominant();
        info.push_str(&format!("  Dominant emotion: {:?}\n", dominant));
        info.push_str(&format!("  Blackboard entries: {}\n", agent.blackboard.entries.len()));
        Some(info)
    }

    // ---- KEYBOARD SHORTCUTS ----

    pub fn handle_key(&mut self, key: EditorKey, shift: bool, ctrl: bool) {
        match key {
            EditorKey::Delete => {
                self.bt_delete_selected(self.active_tree_index);
            }
            EditorKey::Z if ctrl && !shift => {
                self.bt_undo();
                self.show_status("Undo", 2.0);
            }
            EditorKey::Z if ctrl && shift => {
                self.bt_redo();
                self.show_status("Redo", 2.0);
            }
            EditorKey::A if ctrl => {
                self.bt_select_all(self.active_tree_index);
            }
            EditorKey::L if ctrl => {
                self.bt_auto_layout(self.active_tree_index);
                self.show_status("Layout computed", 2.0);
            }
            EditorKey::Space => {
                self.simulation_running = !self.simulation_running;
                let msg = if self.simulation_running { "Simulation started" } else { "Simulation paused" };
                self.show_status(msg, 2.0);
            }
            EditorKey::F5 => {
                for tree in &mut self.behavior_trees {
                    tree.reset();
                }
                self.show_status("Trees reset", 2.0);
            }
            EditorKey::G if ctrl => {
                self.snap_to_grid = !self.snap_to_grid;
                let msg = if self.snap_to_grid { "Snap to grid ON" } else { "Snap to grid OFF" };
                self.show_status(msg, 2.0);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorKey {
    Delete, Z, A, L, Space, F5, G, Other,
}

// ============================================================
// PATHFINDING — A* on a grid (used by agents for navigation)
// ============================================================

#[derive(Clone, Debug)]
pub struct GridPathfinder {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub origin: Vec2,
    pub passable: Vec<bool>,
    pub cost_map: Vec<f32>,
}

impl GridPathfinder {
    pub fn new(width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        let n = width * height;
        Self {
            width,
            height,
            cell_size,
            origin,
            passable: vec![true; n],
            cost_map: vec![1.0; n],
        }
    }

    pub fn world_to_cell(&self, pos: Vec2) -> (i32, i32) {
        let rel = pos - self.origin;
        let x = (rel.x / self.cell_size).floor() as i32;
        let y = (rel.y / self.cell_size).floor() as i32;
        (x, y)
    }

    pub fn cell_to_world(&self, x: i32, y: i32) -> Vec2 {
        Vec2::new(
            self.origin.x + x as f32 * self.cell_size + self.cell_size * 0.5,
            self.origin.y + y as f32 * self.cell_size + self.cell_size * 0.5,
        )
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { return None; }
        Some(y as usize * self.width + x as usize)
    }

    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        self.idx(x, y).map(|i| self.passable[i]).unwrap_or(false)
    }

    pub fn set_obstacle(&mut self, x: i32, y: i32, obstacle: bool) {
        if let Some(i) = self.idx(x, y) { self.passable[i] = !obstacle; }
    }

    /// A* search; returns world-space waypoints
    pub fn find_path(&self, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
        let (sx, sy) = self.world_to_cell(from);
        let (ex, ey) = self.world_to_cell(to);
        if !self.is_passable(sx, sy) || !self.is_passable(ex, ey) { return None; }
        if sx == ex && sy == ey { return Some(vec![to]); }

        #[derive(Clone, Debug)]
        struct Node { x: i32, y: i32, g: f32, h: f32, parent: Option<(i32, i32)> }
        impl Node { fn f(&self) -> f32 { self.g + self.h } }

        let heuristic = |x: i32, y: i32| -> f32 {
            let dx = (x - ex).abs() as f32;
            let dy = (y - ey).abs() as f32;
            (dx + dy) * 1.001  // tie-breaking
        };

        let mut open: BTreeMap<(i32, i32), Node> = BTreeMap::new();
        let mut closed: HashMap<(i32, i32), Node> = HashMap::new();

        open.insert((sx, sy), Node { x: sx, y: sy, g: 0.0, h: heuristic(sx, sy), parent: None });

        let neighbors_offsets: [(i32, i32, f32); 8] = [
            (1, 0, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0),
            (1, 1, SQRT2), (-1, 1, SQRT2), (1, -1, SQRT2), (-1, -1, SQRT2),
        ];

        while !open.is_empty() {
            // Find lowest f in open
            let current_key = open.iter()
                .min_by(|a, b| a.1.f().partial_cmp(&b.1.f()).unwrap())
                .map(|(k, _)| *k)?;
            let current = open.remove(&current_key)?;

            if current.x == ex && current.y == ey {
                // Reconstruct
                let mut path: Vec<Vec2> = Vec::new();
                let mut cur = (current.x, current.y);
                path.push(self.cell_to_world(cur.0, cur.1));
                closed.insert(cur, current);
                while let Some(parent) = closed.get(&cur).and_then(|n| n.parent) {
                    path.push(self.cell_to_world(parent.0, parent.1));
                    cur = parent;
                }
                path.reverse();
                return Some(path);
            }

            closed.insert((current.x, current.y), current.clone());

            for &(dx, dy, move_cost) in &neighbors_offsets {
                let nx = current.x + dx;
                let ny = current.y + dy;
                if !self.is_passable(nx, ny) { continue; }
                if closed.contains_key(&(nx, ny)) { continue; }
                let cell_cost = self.idx(nx, ny).map(|i| self.cost_map[i]).unwrap_or(1.0);
                let new_g = current.g + move_cost * cell_cost;
                let new_h = heuristic(nx, ny);
                let new_f = new_g + new_h;
                if let Some(existing) = open.get(&(nx, ny)) {
                    if existing.f() <= new_f { continue; }
                }
                open.insert((nx, ny), Node { x: nx, y: ny, g: new_g, h: new_h, parent: Some((current.x, current.y)) });
            }

            if closed.len() > 8192 { break; }
        }
        None
    }

    pub fn smooth_path(path: &[Vec2], obstacles: &[Aabb]) -> Vec<Vec2> {
        if path.len() <= 2 { return path.to_vec(); }
        let mut smoothed = vec![path[0]];
        let mut current_idx = 0;
        while current_idx < path.len() - 1 {
            let mut furthest = current_idx + 1;
            for i in (current_idx + 1)..path.len() {
                let from = path[current_idx];
                let to = path[i];
                let from3 = Vec3::new(from.x, 0.0, from.y);
                let to3 = Vec3::new(to.x, 0.0, to.y);
                let dir3 = to3 - from3;
                let len3 = dir3.length();
                let inv = Vec3::new(1.0 / dir3.x, 1.0 / dir3.y, 1.0 / dir3.z);
                let clear = !obstacles.iter().any(|obs| obs.ray_intersects(from3, inv, len3));
                if clear { furthest = i; }
            }
            smoothed.push(path[furthest]);
            current_idx = furthest;
        }
        smoothed
    }
}

// ============================================================
// ADDITIONAL MATH UTILITIES
// ============================================================

pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t.clamp(0.0, 1.0) }

pub fn smooth_damp(current: f32, target: f32, velocity: &mut f32, smooth_time: f32, max_speed: f32, dt: f32) -> f32 {
    let smooth_time = smooth_time.max(0.0001);
    let omega = 2.0 / smooth_time;
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let original_to = target;
    let max_change = max_speed * smooth_time;
    let change = change.clamp(-max_change, max_change);
    let target2 = current - change;
    let temp = (*velocity + omega * change) * dt;
    *velocity = (*velocity - omega * temp) * exp;
    let output = target2 + (change + temp) * exp;
    if original_to - current > 0.0 && output > original_to {
        *velocity = 0.0;
        return original_to;
    }
    if original_to - current < 0.0 && output < original_to {
        *velocity = 0.0;
        return original_to;
    }
    output
}

pub fn smooth_damp_vec3(
    current: Vec3, target: Vec3,
    velocity: &mut Vec3,
    smooth_time: f32,
    max_speed: f32,
    dt: f32,
) -> Vec3 {
    Vec3::new(
        smooth_damp(current.x, target.x, &mut velocity.x, smooth_time, max_speed, dt),
        smooth_damp(current.y, target.y, &mut velocity.y, smooth_time, max_speed, dt),
        smooth_damp(current.z, target.z, &mut velocity.z, smooth_time, max_speed, dt),
    )
}

pub fn angle_between_vectors(a: Vec3, b: Vec3) -> f32 {
    let dot = a.normalize_or_zero().dot(b.normalize_or_zero());
    dot.clamp(-1.0, 1.0).acos()
}

pub fn signed_angle_2d(from: Vec2, to: Vec2) -> f32 {
    let cross = from.x * to.y - from.y * to.x;
    let dot = from.x * to.x + from.y * to.y;
    cross.atan2(dot)
}

pub fn rotate_vec2(v: Vec2, angle: f32) -> Vec2 {
    let cos = angle.cos();
    let sin = angle.sin();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

pub fn closest_point_on_segment(point: Vec3, seg_a: Vec3, seg_b: Vec3) -> Vec3 {
    let ab = seg_b - seg_a;
    let ap = point - seg_a;
    let len_sq = ab.length_squared();
    if len_sq < EPSILON { return seg_a; }
    let t = ap.dot(ab) / len_sq;
    seg_a + ab * t.clamp(0.0, 1.0)
}

pub fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = sign_2d(p, a, b);
    let d2 = sign_2d(p, b, c);
    let d3 = sign_2d(p, c, a);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn sign_2d(p1: Vec2, p2: Vec2, p3: Vec2) -> f32 {
    (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
}

pub fn catmull_rom_point(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    p0 * (-t3 + 2.0 * t2 - t) * 0.5
        + p1 * (3.0 * t3 - 5.0 * t2 + 2.0) * 0.5
        + p2 * (-3.0 * t3 + 4.0 * t2 + t) * 0.5
        + p3 * (t3 - t2) * 0.5
}

pub fn catmull_rom_velocity(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    p0 * (-3.0 * t2 + 4.0 * t - 1.0) * 0.5
        + p1 * (9.0 * t2 - 10.0 * t) * 0.5
        + p2 * (-9.0 * t2 + 8.0 * t + 1.0) * 0.5
        + p3 * (3.0 * t2 - 2.0 * t) * 0.5
}

// ============================================================
// SQUAD INTELLIGENCE — TACTICAL AI
// ============================================================

pub struct SquadAi {
    pub agents: Vec<u64>,
    pub leader_id: u64,
    pub formation: FormationType,
    pub formation_spacing: f32,
    pub objective: SquadObjective,
    pub threat_map: HashMap<u64, f32>,
    pub suppression_targets: Vec<u64>,
    pub current_time: f32,
}

#[derive(Clone, Debug)]
pub enum SquadObjective {
    Patrol { waypoints: Vec<Vec3>, current_wp: usize },
    Attack { target_id: u64, target_pos: Vec3 },
    Defend { position: Vec3, radius: f32 },
    Retreat { rally_point: Vec3 },
    Scout { area_center: Vec3, radius: f32 },
    Flank { target_pos: Vec3, flank_direction: Vec3 },
    Ambush { ambush_pos: Vec3, trigger_radius: f32 },
}

impl SquadAi {
    pub fn new(leader_id: u64) -> Self {
        Self {
            agents: Vec::new(),
            leader_id,
            formation: FormationType::Wedge,
            formation_spacing: 2.5,
            objective: SquadObjective::Patrol { waypoints: Vec::new(), current_wp: 0 },
            threat_map: HashMap::new(),
            suppression_targets: Vec::new(),
            current_time: 0.0,
        }
    }

    pub fn add_agent(&mut self, id: u64) {
        if !self.agents.contains(&id) { self.agents.push(id); }
    }

    pub fn remove_agent(&mut self, id: u64) {
        self.agents.retain(|&a| a != id);
        self.threat_map.remove(&id);
        self.suppression_targets.retain(|&a| a != id);
    }

    pub fn assign_formation_slots(&self, agent_positions: &HashMap<u64, Vec3>, leader_forward: Vec3) -> HashMap<u64, Vec3> {
        let leader_pos = agent_positions.get(&self.leader_id).copied().unwrap_or(Vec3::ZERO);
        let slots = FormationLayout::compute_slots(
            self.formation, leader_pos, leader_forward, self.agents.len(), self.formation_spacing
        );
        let positions: Vec<Vec3> = self.agents.iter().map(|id| agent_positions.get(id).copied().unwrap_or(Vec3::ZERO)).collect();
        let assignment = FormationLayout::assign_slots(&positions, &slots);
        self.agents.iter().zip(assignment.iter()).map(|(&id, &slot_idx)| {
            (id, slots.get(slot_idx).copied().unwrap_or(leader_pos))
        }).collect()
    }

    pub fn assess_threat_level(&mut self, perceived_entities: &[PerceivedEntity]) -> f32 {
        self.threat_map.clear();
        let mut total_threat = 0.0f32;
        for entity in perceived_entities {
            let threat = entity.threat_level * entity.confidence;
            self.threat_map.insert(entity.entity_id, threat);
            total_threat += threat;
        }
        total_threat
    }

    pub fn decide_objective(&mut self, threat_level: f32, perceived: &[PerceivedEntity]) {
        if threat_level > 3.0 {
            // High threat: attack highest priority target
            if let Some(highest) = perceived.iter()
                .max_by(|a, b| (a.threat_level * a.confidence).partial_cmp(&(b.threat_level * b.confidence)).unwrap())
            {
                self.objective = SquadObjective::Attack {
                    target_id: highest.entity_id,
                    target_pos: highest.last_known_position,
                };
            }
        } else if threat_level > 1.0 {
            // Moderate: defend current position or flank
            // Keep current objective
        } else {
            // Low threat: patrol
            if !matches!(self.objective, SquadObjective::Patrol { .. }) {
                // Transition to patrol — keep existing waypoints
            }
        }
    }

    pub fn tick(&mut self, dt: f32, perceived: &[PerceivedEntity], agent_positions: &HashMap<u64, Vec3>) {
        self.current_time += dt;
        let threat = self.assess_threat_level(perceived);
        self.decide_objective(threat, perceived);

        // Update patrol waypoint
        if let SquadObjective::Patrol { ref waypoints, ref mut current_wp } = &mut self.objective {
            if let Some(leader_pos) = agent_positions.get(&self.leader_id) {
                if let Some(wp) = waypoints.get(*current_wp) {
                    if (*wp - *leader_pos).length() < 2.0 {
                        *current_wp = (*current_wp + 1) % waypoints.len().max(1);
                    }
                }
            }
        }
    }

    pub fn get_leader_target(&self) -> Option<Vec3> {
        match &self.objective {
            SquadObjective::Patrol { waypoints, current_wp } => waypoints.get(*current_wp).copied(),
            SquadObjective::Attack { target_pos, .. } => Some(*target_pos),
            SquadObjective::Defend { position, .. } => Some(*position),
            SquadObjective::Retreat { rally_point } => Some(*rally_point),
            SquadObjective::Scout { area_center, .. } => Some(*area_center),
            SquadObjective::Flank { target_pos, flank_direction } => {
                Some(*target_pos + *flank_direction * 10.0)
            }
            SquadObjective::Ambush { ambush_pos, .. } => Some(*ambush_pos),
        }
    }
}

// ============================================================
// BEHAVIOR TREE DEBUGGER
// ============================================================

pub struct BtDebugger {
    pub is_attached: bool,
    pub agent_id: Option<u64>,
    pub breakpoints: HashSet<u32>,
    pub step_mode: bool,
    pub last_tick_nodes: Vec<u32>,
    pub node_exec_counts: HashMap<u32, u64>,
    pub node_status_history: HashMap<u32, VecDeque<BtStatus>>,
    pub status_history_len: usize,
    pub paused_at_node: Option<u32>,
    pub play_speed: f32,
}

impl BtDebugger {
    pub fn new() -> Self {
        Self {
            is_attached: false,
            agent_id: None,
            breakpoints: HashSet::new(),
            step_mode: false,
            last_tick_nodes: Vec::new(),
            node_exec_counts: HashMap::new(),
            node_status_history: HashMap::new(),
            status_history_len: 32,
            paused_at_node: None,
            play_speed: 1.0,
        }
    }

    pub fn attach(&mut self, agent_id: u64) {
        self.agent_id = Some(agent_id);
        self.is_attached = true;
    }

    pub fn detach(&mut self) {
        self.agent_id = None;
        self.is_attached = false;
        self.paused_at_node = None;
    }

    pub fn toggle_breakpoint(&mut self, node_id: u32) {
        if self.breakpoints.contains(&node_id) {
            self.breakpoints.remove(&node_id);
        } else {
            self.breakpoints.insert(node_id);
        }
    }

    pub fn record_tick(&mut self, visited: &[u32], tree: &BehaviorTree) {
        self.last_tick_nodes = visited.to_vec();
        for &node_id in visited {
            *self.node_exec_counts.entry(node_id).or_insert(0) += 1;
            if let Some(node) = tree.nodes.get(&node_id) {
                let history = self.node_status_history.entry(node_id).or_insert_with(|| VecDeque::with_capacity(self.status_history_len));
                history.push_back(node.status);
                if history.len() > self.status_history_len { history.pop_front(); }
            }
        }
    }

    pub fn check_breakpoints(&mut self, visited: &[u32]) -> bool {
        for &node_id in visited {
            if self.breakpoints.contains(&node_id) {
                self.paused_at_node = Some(node_id);
                return true;
            }
        }
        false
    }

    pub fn get_node_coverage(&self, tree: &BehaviorTree) -> f32 {
        let total = tree.nodes.len();
        if total == 0 { return 0.0; }
        let executed = self.node_exec_counts.len();
        executed as f32 / total as f32
    }

    pub fn hot_nodes(&self, top_n: usize) -> Vec<(u32, u64)> {
        let mut vec: Vec<(u32, u64)> = self.node_exec_counts.iter().map(|(&k, &v)| (k, v)).collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        vec.truncate(top_n);
        vec
    }

    pub fn node_success_rate(&self, node_id: u32) -> f32 {
        if let Some(history) = self.node_status_history.get(&node_id) {
            let successes = history.iter().filter(|&&s| s == BtStatus::Success).count();
            if history.is_empty() { 0.0 }
            else { successes as f32 / history.len() as f32 }
        } else { 0.0 }
    }
}

// ============================================================
// NOISE UTILS (for procedural behavior variation)
// ============================================================

pub struct ValueNoise {
    pub perm: [u8; 512],
}

impl ValueNoise {
    pub fn new(seed: u64) -> Self {
        let mut perm = [0u8; 512];
        let mut rng = seed;
        let mut table: Vec<u8> = (0..=255u8).collect();
        for i in (1..256).rev() {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let j = (rng >> 33) as usize % (i + 1);
            table.swap(i, j);
        }
        for i in 0..256 { perm[i] = table[i]; perm[i + 256] = table[i]; }
        Self { perm }
    }

    fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }
    fn lerp_n(a: f32, b: f32, t: f32) -> f32 { a + t * (b - a) }
    fn grad(hash: u8, x: f32, y: f32, z: f32) -> f32 {
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 { y } else if h == 12 || h == 14 { x } else { z };
        (if (h & 1) == 0 { u } else { -u }) + (if (h & 2) == 0 { v } else { -v })
    }

    pub fn sample_3d(&self, x: f32, y: f32, z: f32) -> f32 {
        let xi = x.floor() as i32 & 255;
        let yi = y.floor() as i32 & 255;
        let zi = z.floor() as i32 & 255;
        let xf = x - x.floor();
        let yf = y - y.floor();
        let zf = z - z.floor();
        let u = Self::fade(xf);
        let v = Self::fade(yf);
        let w = Self::fade(zf);
        let a  = self.perm[xi as usize] as i32 + yi;
        let aa = self.perm[a  as usize] as i32 + zi;
        let ab = self.perm[(a+1) as usize] as i32 + zi;
        let b  = self.perm[(xi+1) as usize] as i32 + yi;
        let ba = self.perm[b as usize] as i32 + zi;
        let bb = self.perm[(b+1) as usize] as i32 + zi;

        let r = Self::lerp_n(
            Self::lerp_n(
                Self::lerp_n(Self::grad(self.perm[aa as usize], xf,   yf,   zf   ), Self::grad(self.perm[ba as usize], xf-1.0, yf,   zf   ), u),
                Self::lerp_n(Self::grad(self.perm[ab as usize], xf,   yf-1.0, zf), Self::grad(self.perm[bb as usize], xf-1.0, yf-1.0, zf  ), u), v),
            Self::lerp_n(
                Self::lerp_n(Self::grad(self.perm[(aa+1) as usize], xf, yf,   zf-1.0), Self::grad(self.perm[(ba+1) as usize], xf-1.0, yf,   zf-1.0), u),
                Self::lerp_n(Self::grad(self.perm[(ab+1) as usize], xf, yf-1.0, zf-1.0), Self::grad(self.perm[(bb+1) as usize], xf-1.0, yf-1.0, zf-1.0), u), v), w);
        (r + 1.0) * 0.5
    }

    pub fn octave_3d(&self, x: f32, y: f32, z: f32, octaves: usize, persistence: f32, lacunarity: f32) -> f32 {
        let mut value = 0.0f32;
        let mut amplitude = 1.0f32;
        let mut frequency = 1.0f32;
        let mut max_value = 0.0f32;
        for _ in 0..octaves {
            value += self.sample_3d(x * frequency, y * frequency, z * frequency) * amplitude;
            max_value += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }
        value / max_value
    }
}

// ============================================================
// BEHAVIOR MODULATION SYSTEM
// ============================================================

pub struct BehaviorModulator {
    pub noise: ValueNoise,
    pub time_offset: f32,
    pub parameters: HashMap<String, ModulatedParam>,
}

#[derive(Clone, Debug)]
pub struct ModulatedParam {
    pub base_value: f32,
    pub noise_scale: f32,
    pub noise_speed: f32,
    pub noise_seed: f32,
    pub clamp_min: f32,
    pub clamp_max: f32,
    pub current_value: f32,
}

impl ModulatedParam {
    pub fn new(base: f32, noise_scale: f32, noise_speed: f32, seed: f32) -> Self {
        Self {
            base_value: base,
            noise_scale,
            noise_speed,
            noise_seed: seed,
            clamp_min: f32::NEG_INFINITY,
            clamp_max: f32::INFINITY,
            current_value: base,
        }
    }

    pub fn with_clamp(mut self, min: f32, max: f32) -> Self {
        self.clamp_min = min;
        self.clamp_max = max;
        self
    }
}

impl BehaviorModulator {
    pub fn new(seed: u64) -> Self {
        Self {
            noise: ValueNoise::new(seed),
            time_offset: 0.0,
            parameters: HashMap::new(),
        }
    }

    pub fn add_param(&mut self, name: &str, param: ModulatedParam) {
        self.parameters.insert(name.to_string(), param);
    }

    pub fn update(&mut self, dt: f32) {
        self.time_offset += dt;
        for param in self.parameters.values_mut() {
            let noise_val = self.noise.sample_3d(
                param.noise_seed + self.time_offset * param.noise_speed,
                param.noise_seed * 1.37,
                0.0,
            );
            let modulated = param.base_value + (noise_val * 2.0 - 1.0) * param.noise_scale;
            param.current_value = modulated.clamp(param.clamp_min, param.clamp_max);
        }
    }

    pub fn get(&self, name: &str) -> f32 {
        self.parameters.get(name).map(|p| p.current_value).unwrap_or(0.0)
    }
}

// ============================================================
// MEMORY SYSTEM (longer-term agent memory beyond blackboard)
// ============================================================

#[derive(Clone, Debug)]
pub struct MemoryRecord {
    pub key: String,
    pub value: BlackboardValue,
    pub created_at: f32,
    pub last_accessed: f32,
    pub importance: f32,
    pub decay_rate: f32,
    pub source_entity: Option<u64>,
    pub tags: HashSet<String>,
}

impl MemoryRecord {
    pub fn new(key: &str, value: BlackboardValue, time: f32, importance: f32) -> Self {
        Self {
            key: key.to_string(),
            value,
            created_at: time,
            last_accessed: time,
            importance,
            decay_rate: 0.05,
            source_entity: None,
            tags: HashSet::new(),
        }
    }

    pub fn decay_importance(&mut self, dt: f32) {
        self.importance = (self.importance - self.decay_rate * dt).max(0.0);
    }

    pub fn is_forgotten(&self) -> bool { self.importance < 0.01 }
}

pub struct AgentMemory {
    pub records: HashMap<String, MemoryRecord>,
    pub forget_threshold: f32,
    pub max_records: usize,
    pub current_time: f32,
}

impl AgentMemory {
    pub fn new(max_records: usize) -> Self {
        Self { records: HashMap::new(), forget_threshold: 0.01, max_records, current_time: 0.0 }
    }

    pub fn remember(&mut self, key: &str, value: BlackboardValue, importance: f32) {
        if self.records.len() >= self.max_records {
            // Forget least important
            let min_key = self.records.iter()
                .min_by(|a, b| a.1.importance.partial_cmp(&b.1.importance).unwrap())
                .map(|(k, _)| k.clone());
            if let Some(mk) = min_key { self.records.remove(&mk); }
        }
        let record = MemoryRecord::new(key, value, self.current_time, importance);
        self.records.insert(key.to_string(), record);
    }

    pub fn recall(&mut self, key: &str) -> Option<&BlackboardValue> {
        if let Some(record) = self.records.get_mut(key) {
            record.last_accessed = self.current_time;
            // Accessing refreshes importance slightly
            record.importance = (record.importance + 0.1).min(1.0);
            Some(&record.value)
        } else { None }
    }

    pub fn update(&mut self, dt: f32) {
        self.current_time += dt;
        let mut to_forget = Vec::new();
        for (key, record) in &mut self.records {
            record.decay_importance(dt);
            if record.is_forgotten() { to_forget.push(key.clone()); }
        }
        for key in to_forget { self.records.remove(&key); }
    }

    pub fn most_important(&self, n: usize) -> Vec<&MemoryRecord> {
        let mut records: Vec<&MemoryRecord> = self.records.values().collect();
        records.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
        records.truncate(n);
        records
    }

    pub fn with_tag(&self, tag: &str) -> Vec<&MemoryRecord> {
        self.records.values().filter(|r| r.tags.contains(tag)).collect()
    }
}

// ============================================================
// WORLD STATE TRACKER (global shared state for GOAP + agents)
// ============================================================

pub struct WorldStateTracker {
    pub facts: HashMap<String, BlackboardValue>,
    pub last_changed: HashMap<String, f32>,
    pub listeners: Vec<WorldStateFact>,
    pub current_time: f32,
}

#[derive(Clone, Debug)]
pub struct WorldStateFact {
    pub key: String,
    pub condition: CompareOp,
    pub value: BlackboardValue,
    pub triggered: bool,
    pub callback_label: String,
}

impl WorldStateTracker {
    pub fn new() -> Self {
        Self {
            facts: HashMap::new(),
            last_changed: HashMap::new(),
            listeners: Vec::new(),
            current_time: 0.0,
        }
    }

    pub fn set(&mut self, key: &str, value: BlackboardValue) {
        self.facts.insert(key.to_string(), value);
        self.last_changed.insert(key.to_string(), self.current_time);
        // Check listeners
        for listener in &mut self.listeners {
            if listener.key == key {
                let val = self.facts.get(key).unwrap_or(&BlackboardValue::None);
                listener.triggered = listener.condition.evaluate(val, &listener.value);
            }
        }
    }

    pub fn get(&self, key: &str) -> &BlackboardValue {
        self.facts.get(key).unwrap_or(&BlackboardValue::None)
    }

    pub fn tick(&mut self, dt: f32) { self.current_time += dt; }

    pub fn add_listener(&mut self, key: &str, condition: CompareOp, value: BlackboardValue, callback: &str) {
        self.listeners.push(WorldStateFact {
            key: key.to_string(),
            condition,
            value,
            triggered: false,
            callback_label: callback.to_string(),
        });
    }

    pub fn triggered_callbacks(&self) -> Vec<String> {
        self.listeners.iter().filter(|l| l.triggered).map(|l| l.callback_label.clone()).collect()
    }
}

// ============================================================
// SOCIAL BEHAVIOR GRAPH
// ============================================================

#[derive(Clone, Debug)]
pub struct SocialRelationship {
    pub other_id: u64,
    pub affinity: f32,      // -1..1
    pub trust: f32,         // 0..1
    pub fear: f32,          // 0..1
    pub last_interaction: f32,
    pub interaction_count: u32,
}

impl SocialRelationship {
    pub fn new(other_id: u64) -> Self {
        Self { other_id, affinity: 0.0, trust: 0.5, fear: 0.0, last_interaction: 0.0, interaction_count: 0 }
    }

    pub fn update_after_interaction(&mut self, positive: bool, intensity: f32, time: f32) {
        self.last_interaction = time;
        self.interaction_count += 1;
        let delta = if positive { intensity } else { -intensity };
        self.affinity = (self.affinity + delta * 0.2).clamp(-1.0, 1.0);
        if positive {
            self.trust = (self.trust + intensity * 0.1).min(1.0);
        } else {
            self.trust = (self.trust - intensity * 0.15).max(0.0);
            self.fear = (self.fear + intensity * 0.1).min(1.0);
        }
    }

    pub fn decay(&mut self, dt: f32, current_time: f32) {
        let age = current_time - self.last_interaction;
        let decay_factor = (-age * 0.001 * dt).exp();
        self.affinity *= decay_factor;
        self.fear = (self.fear - dt * 0.01).max(0.0);
    }
}

pub struct SocialGraph {
    pub agent_id: u64,
    pub relationships: HashMap<u64, SocialRelationship>,
    pub faction_id: u32,
    pub faction_relations: HashMap<u32, f32>,  // faction_id -> affinity
}

impl SocialGraph {
    pub fn new(agent_id: u64, faction_id: u32) -> Self {
        Self { agent_id, relationships: HashMap::new(), faction_id, faction_relations: HashMap::new() }
    }

    pub fn get_or_create_relationship(&mut self, other_id: u64) -> &mut SocialRelationship {
        self.relationships.entry(other_id).or_insert_with(|| SocialRelationship::new(other_id))
    }

    pub fn affinity_toward(&self, other_id: u64) -> f32 {
        self.relationships.get(&other_id).map(|r| r.affinity).unwrap_or(0.0)
    }

    pub fn is_ally(&self, other_id: u64, other_faction: u32) -> bool {
        let personal = self.relationships.get(&other_id).map(|r| r.affinity).unwrap_or(0.0);
        let faction_aff = self.faction_relations.get(&other_faction).copied().unwrap_or(0.0);
        (personal + faction_aff) > 0.2
    }

    pub fn is_enemy(&self, other_id: u64, other_faction: u32) -> bool {
        let personal = self.relationships.get(&other_id).map(|r| r.affinity).unwrap_or(0.0);
        let faction_aff = self.faction_relations.get(&other_faction).copied().unwrap_or(0.0);
        (personal + faction_aff) < -0.2
    }

    pub fn update(&mut self, dt: f32, current_time: f32) {
        for rel in self.relationships.values_mut() {
            rel.decay(dt, current_time);
        }
    }
}

// ============================================================
// ANIMATION STATE MACHINE (simple, for AI locomotion)
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum LocoState {
    Idle,
    Walk,
    Run,
    Crouch,
    CrouchWalk,
    Jump,
    Fall,
    Land,
    Strafe(f32),  // angle
    Dead,
}

pub struct LocomotionAnimController {
    pub state: LocoState,
    pub blend_weights: HashMap<String, f32>,
    pub transition_time: f32,
    pub transition_remaining: f32,
    pub prev_state: LocoState,
    pub speed: f32,
    pub turn_rate: f32,
    pub is_grounded: bool,
}

impl LocomotionAnimController {
    pub fn new() -> Self {
        Self {
            state: LocoState::Idle,
            blend_weights: HashMap::new(),
            transition_time: 0.2,
            transition_remaining: 0.0,
            prev_state: LocoState::Idle,
            speed: 0.0,
            turn_rate: 0.0,
            is_grounded: true,
        }
    }

    pub fn update(&mut self, velocity: Vec3, is_grounded: bool, is_crouching: bool, dt: f32) {
        self.speed = velocity.length();
        self.is_grounded = is_grounded;
        self.transition_remaining = (self.transition_remaining - dt).max(0.0);

        let new_state = if !is_grounded {
            if velocity.y > 0.1 { LocoState::Jump }
            else { LocoState::Fall }
        } else if is_crouching {
            if self.speed > 0.5 { LocoState::CrouchWalk } else { LocoState::Crouch }
        } else if self.speed < 0.1 {
            LocoState::Idle
        } else if self.speed < 2.5 {
            LocoState::Walk
        } else {
            LocoState::Run
        };

        if new_state != self.state {
            self.prev_state = self.state.clone();
            self.state = new_state;
            self.transition_remaining = self.transition_time;
        }

        // Update blend weights
        let t = if self.transition_time > 0.0 {
            1.0 - (self.transition_remaining / self.transition_time)
        } else { 1.0 };

        self.blend_weights.insert("walk".to_string(), if matches!(self.state, LocoState::Walk) { t } else { 0.0 });
        self.blend_weights.insert("run".to_string(), if matches!(self.state, LocoState::Run) { t } else { 0.0 });
        self.blend_weights.insert("idle".to_string(), if matches!(self.state, LocoState::Idle) { t } else { 0.0 });
        self.blend_weights.insert("crouch".to_string(), if matches!(self.state, LocoState::Crouch | LocoState::CrouchWalk) { t } else { 0.0 });
    }

    pub fn get_blend_weight(&self, anim: &str) -> f32 {
        self.blend_weights.get(anim).copied().unwrap_or(0.0)
    }
}

// ============================================================
// AI EDITOR TEST SUITE
// ============================================================

pub struct AiEditorTests;

impl AiEditorTests {
    pub fn run_all() -> Vec<(String, bool)> {
        let mut results = Vec::new();
        results.push(("blackboard_basic".to_string(), Self::test_blackboard_basic()));
        results.push(("bt_sequence_success".to_string(), Self::test_bt_sequence_success()));
        results.push(("bt_selector_fallthrough".to_string(), Self::test_bt_selector_fallthrough()));
        results.push(("bt_inverter".to_string(), Self::test_bt_inverter()));
        results.push(("bt_cooldown".to_string(), Self::test_bt_cooldown()));
        results.push(("bt_repeater".to_string(), Self::test_bt_repeater()));
        results.push(("bt_wait".to_string(), Self::test_bt_wait()));
        results.push(("goap_basic_plan".to_string(), Self::test_goap_basic_plan()));
        results.push(("utility_scoring".to_string(), Self::test_utility_scoring()));
        results.push(("perception_vision".to_string(), Self::test_perception_vision()));
        results.push(("formation_line".to_string(), Self::test_formation_line()));
        results.push(("steering_seek".to_string(), Self::test_steering_seek()));
        results.push(("emotion_decay".to_string(), Self::test_emotion_decay()));
        results.push(("response_curve_logistic".to_string(), Self::test_response_curve_logistic()));
        results.push(("astar_pathfinding".to_string(), Self::test_astar_pathfinding()));
        results
    }

    fn test_blackboard_basic() -> bool {
        let mut bb = Blackboard::new();
        bb.set("health", BlackboardValue::Float(100.0));
        let v = bb.get_float("health");
        (v - 100.0).abs() < EPSILON
    }

    fn test_bt_sequence_success() -> bool {
        let mut tree = BehaviorTree::new("test");
        let root = tree.add_node(BtNodeType::Sequence);
        tree.set_root(root);
        let s1 = tree.add_node(BtNodeType::SucceedAlways);
        let s2 = tree.add_node(BtNodeType::SucceedAlways);
        tree.add_child(root, s1);
        tree.add_child(root, s2);
        let mut bb = Blackboard::new();
        let mut ctx = BtTickContext::new(&mut bb, 0.016, 0.0, Vec3::ZERO, 1);
        let status = tree.tick(&mut ctx);
        status == BtStatus::Success
    }

    fn test_bt_selector_fallthrough() -> bool {
        let mut tree = BehaviorTree::new("test");
        let root = tree.add_node(BtNodeType::Selector);
        tree.set_root(root);
        let f1 = tree.add_node(BtNodeType::FailAlways);
        let s1 = tree.add_node(BtNodeType::SucceedAlways);
        tree.add_child(root, f1);
        tree.add_child(root, s1);
        let mut bb = Blackboard::new();
        let mut ctx = BtTickContext::new(&mut bb, 0.016, 0.0, Vec3::ZERO, 1);
        let status = tree.tick(&mut ctx);
        status == BtStatus::Success
    }

    fn test_bt_inverter() -> bool {
        let mut tree = BehaviorTree::new("test");
        let root = tree.add_node(BtNodeType::Inverter);
        tree.set_root(root);
        let child = tree.add_node(BtNodeType::SucceedAlways);
        tree.add_child(root, child);
        let mut bb = Blackboard::new();
        let mut ctx = BtTickContext::new(&mut bb, 0.016, 0.0, Vec3::ZERO, 1);
        let status = tree.tick(&mut ctx);
        status == BtStatus::Failure
    }

    fn test_bt_cooldown() -> bool {
        let mut tree = BehaviorTree::new("test");
        let root = tree.add_node(BtNodeType::Cooldown { cooldown: 2.0 });
        tree.set_root(root);
        let child = tree.add_node(BtNodeType::SucceedAlways);
        tree.add_child(root, child);
        let mut bb = Blackboard::new();
        let mut ctx = BtTickContext::new(&mut bb, 0.016, 0.0, Vec3::ZERO, 1);
        let s1 = tree.tick(&mut ctx);  // Should succeed (first call, cooldown 0)
        let s2 = tree.tick(&mut ctx);  // Should fail (on cooldown)
        s1 == BtStatus::Success && s2 == BtStatus::Failure
    }

    fn test_bt_repeater() -> bool {
        let mut tree = BehaviorTree::new("test");
        let root = tree.add_node(BtNodeType::Repeater { times: 3 });
        tree.set_root(root);
        let child = tree.add_node(BtNodeType::SucceedAlways);
        tree.add_child(root, child);
        let mut bb = Blackboard::new();
        let mut ctx = BtTickContext::new(&mut bb, 0.016, 0.0, Vec3::ZERO, 1);
        let s1 = tree.tick(&mut ctx);
        let s2 = tree.tick(&mut ctx);
        let s3 = tree.tick(&mut ctx);
        // After 3 completions returns Success
        s3 == BtStatus::Success
    }

    fn test_bt_wait() -> bool {
        let mut tree = BehaviorTree::new("test");
        let root = tree.add_node(BtNodeType::Wait { duration: 0.5 });
        tree.set_root(root);
        let mut bb = Blackboard::new();
        let mut ctx1 = BtTickContext::new(&mut bb, 0.1, 0.0, Vec3::ZERO, 1);
        let s1 = tree.tick(&mut ctx1);
        let mut bb2 = Blackboard::new();
        let mut ctx2 = BtTickContext::new(&mut bb2, 0.5, 0.5, Vec3::ZERO, 1);
        let s2 = tree.tick(&mut ctx2);
        s1 == BtStatus::Running
    }

    fn test_goap_basic_plan() -> bool {
        let planner = GoapLibrary::build_combat_planner();
        // Start: has_ammo=1, enemy_visible=1
        // Goal: enemy_dead=1
        let start: WorldState = 0b0000_0011;
        let goal: WorldState = 0b0000_0100;
        let plan = planner.plan(start, goal, 0.0);
        plan.is_some()
    }

    fn test_utility_scoring() -> bool {
        let dm = UtilityLibrary::build_combat_decision_maker();
        let mut bb = Blackboard::new();
        bb.set("self_health", BlackboardValue::Float(80.0));
        bb.set("enemy_visible", BlackboardValue::Float(1.0));
        bb.set("ammo_count", BlackboardValue::Float(20.0));
        bb.set("threat_dist", BlackboardValue::Float(10.0));
        let scores: Vec<f32> = dm.actions.iter().map(|a| a.score(&bb, 0.0)).collect();
        scores.iter().any(|&s| s > 0.0)
    }

    fn test_perception_vision() -> bool {
        let ps = PerceptionSystem::new(1);
        let observer_pos = Vec3::ZERO;
        let observer_fwd = Vec3::Z;
        let target_pos = Vec3::new(0.0, 0.0, 10.0);  // directly ahead
        let (vis, conf) = ps.can_see(observer_pos, observer_fwd, target_pos, Vec3::ZERO, &[]);
        vis && conf > 0.0
    }

    fn test_formation_line() -> bool {
        let slots = FormationLayout::compute_slots(FormationType::Line, Vec3::ZERO, Vec3::Z, 5, 2.0);
        slots.len() == 5
    }

    fn test_steering_seek() -> bool {
        let agent = SteeringAgent::new(1, Vec3::ZERO, 5.0, 10.0);
        let target = Vec3::new(0.0, 0.0, 10.0);
        let force = SteeringBehaviors::seek(&agent, target);
        force.length() > 0.0
    }

    fn test_emotion_decay() -> bool {
        let mut state = EmotionState::new();
        state.add_emotion(PrimaryEmotion::Fear, 1.0);
        let initial = state.get_intensity(PrimaryEmotion::Fear);
        state.update(1.0);
        let after = state.get_intensity(PrimaryEmotion::Fear);
        after < initial
    }

    fn test_response_curve_logistic() -> bool {
        let curve = ResponseCurve::Logistic { steepness: 5.0, midpoint: 0.5 };
        let low = curve.evaluate(0.0);
        let mid = curve.evaluate(0.5);
        let high = curve.evaluate(1.0);
        low < mid && mid < high
    }

    fn test_astar_pathfinding() -> bool {
        let pf = GridPathfinder::new(20, 20, 1.0, Vec2::ZERO);
        let path = pf.find_path(Vec2::new(0.5, 0.5), Vec2::new(18.5, 18.5));
        path.is_some()
    }
}

// ============================================================
// MODULE REGISTRATION / ENTRY POINT
// ============================================================

pub fn create_default_ai_editor() -> AiBehaviorEditor {
    AiBehaviorEditor::new()
}

pub fn run_editor_tests() -> usize {
    let results = AiEditorTests::run_all();
    let passed = results.iter().filter(|(_, ok)| *ok).count();
    passed
}

// ============================================================
// COVER SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct CoverPoint {
    pub id: u32,
    pub position: Vec3,
    pub normal: Vec3,
    pub height: f32,
    pub is_occupied: Option<u64>,
    pub quality: f32,
    pub flanked_by: Vec<Vec3>,
}

impl CoverPoint {
    pub fn new(id: u32, position: Vec3, normal: Vec3, height: f32) -> Self {
        Self { id, position, normal, height, is_occupied: None, quality: 1.0, flanked_by: Vec::new() }
    }

    pub fn is_good_cover_from(&self, threat_pos: Vec3) -> bool {
        let to_threat = (threat_pos - self.position).normalize_or_zero();
        self.normal.dot(to_threat) > 0.5
    }

    pub fn cover_quality_from(&self, threat_pos: Vec3) -> f32 {
        let to_threat = (threat_pos - self.position).normalize_or_zero();
        let dot = self.normal.dot(to_threat).max(0.0);
        let dist_factor = {
            let d = (threat_pos - self.position).length();
            (d / 20.0).clamp(0.1, 1.0)
        };
        let flank_penalty = self.flanked_by.iter()
            .map(|&fdir| (fdir - self.position).normalize_or_zero().dot(to_threat).max(0.0))
            .fold(0.0f32, |a, b| a.max(b));
        (dot * dist_factor * self.quality * (1.0 - flank_penalty * 0.5)).clamp(0.0, 1.0)
    }

    pub fn peek_position(&self, peek_amount: f32) -> Vec3 {
        self.position + self.normal * peek_amount
    }
}

pub struct CoverSystem {
    pub cover_points: Vec<CoverPoint>,
    pub next_id: u32,
    pub occupation_radius: f32,
}

impl CoverSystem {
    pub fn new() -> Self {
        Self { cover_points: Vec::new(), next_id: 1, occupation_radius: 1.5 }
    }

    pub fn add_cover(&mut self, position: Vec3, normal: Vec3, height: f32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.cover_points.push(CoverPoint::new(id, position, normal, height));
        id
    }

    pub fn find_best_cover(&self, seeker_pos: Vec3, threats: &[Vec3], occupied_by: u64, max_distance: f32) -> Option<&CoverPoint> {
        if threats.is_empty() { return None; }
        self.cover_points.iter()
            .filter(|c| {
                let dist = (c.position - seeker_pos).length();
                dist <= max_distance && (c.is_occupied.is_none() || c.is_occupied == Some(occupied_by))
            })
            .filter(|c| threats.iter().any(|&t| c.is_good_cover_from(t)))
            .max_by(|a, b| {
                let qa: f32 = threats.iter().map(|&t| a.cover_quality_from(t)).sum::<f32>() / (1.0 + (a.position - seeker_pos).length() * 0.1);
                let qb: f32 = threats.iter().map(|&t| b.cover_quality_from(t)).sum::<f32>() / (1.0 + (b.position - seeker_pos).length() * 0.1);
                qa.partial_cmp(&qb).unwrap()
            })
    }

    pub fn occupy(&mut self, cover_id: u32, agent_id: u64) {
        if let Some(c) = self.cover_points.iter_mut().find(|c| c.id == cover_id) {
            c.is_occupied = Some(agent_id);
        }
    }

    pub fn vacate(&mut self, agent_id: u64) {
        for c in &mut self.cover_points { if c.is_occupied == Some(agent_id) { c.is_occupied = None; } }
    }

    pub fn generate_cover_from_obstacles(&mut self, obstacles: &[Aabb], normal_directions: &[Vec3]) {
        for obs in obstacles {
            for &normal in normal_directions {
                let position = obs.center() + normal * (obs.half_extents().length() + 0.5);
                self.add_cover(position, -normal, 1.0);
            }
        }
    }

    pub fn debug_draw(&self, buf: &mut DebugVisualizationBuffer) {
        for cover in &self.cover_points {
            let color = if cover.is_occupied.is_some() { Vec4::new(1.0, 0.5, 0.0, 0.8) } else { Vec4::new(0.0, 0.8, 0.8, 0.8) };
            buf.add(DebugShapeType::Arrow { from: cover.position, to: cover.position + cover.normal * 1.0, head_size: 0.2 }, color, 0.0);
            buf.add(DebugShapeType::Cross { center: cover.position, size: 0.4 }, color, 0.0);
        }
    }
}

// ============================================================
// THREAT ASSESSMENT
// ============================================================

#[derive(Clone, Debug)]
pub struct ThreatEntry {
    pub entity_id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub threat_score: f32,
    pub last_damage_dealt: f32,
    pub can_see_me: bool,
    pub is_flanking: bool,
    pub last_updated: f32,
}

pub struct ThreatAssessor {
    pub threats: Vec<ThreatEntry>,
    pub current_time: f32,
    pub stale_threshold: f32,
    pub damage_weight: f32,
    pub distance_weight: f32,
    pub flanking_weight: f32,
    pub facing_weight: f32,
}

impl ThreatAssessor {
    pub fn new() -> Self {
        Self {
            threats: Vec::new(),
            current_time: 0.0,
            stale_threshold: 5.0,
            damage_weight: 2.5,
            distance_weight: 2.0,
            flanking_weight: 2.0,
            facing_weight: 1.5,
        }
    }

    pub fn compute_threat_score(&self, perceiver_pos: Vec3, target: &PerceivedEntity, target_facing: Vec3, damage_dealt: f32) -> f32 {
        let dist = (target.position - perceiver_pos).length();
        let dist_score = 1.0 / (1.0 + dist * 0.1);
        let to_target = (target.position - perceiver_pos).normalize_or_zero();
        let facing_dot = target_facing.dot(to_target).max(0.0);
        let behind_dot = (-to_target).dot((perceiver_pos - target.position).normalize_or_zero()).max(0.0);
        let flanking = behind_dot > 0.7;
        let speed = target.velocity.length();
        (dist_score * self.distance_weight
            + facing_dot * self.facing_weight
            + (damage_dealt / 100.0) * self.damage_weight
            + if flanking { self.flanking_weight } else { 0.0 }
            + (speed / 10.0) * 0.5) * target.confidence
    }

    pub fn update_threat(&mut self, entity_id: u64, position: Vec3, velocity: Vec3, score: f32, damage_dealt: f32, can_see_me: bool, is_flanking: bool) {
        self.threats.retain(|t| t.entity_id != entity_id);
        self.threats.push(ThreatEntry { entity_id, position, velocity, threat_score: score, last_damage_dealt: damage_dealt, can_see_me, is_flanking, last_updated: self.current_time });
        self.threats.sort_by(|a, b| b.threat_score.partial_cmp(&a.threat_score).unwrap());
    }

    pub fn remove_stale(&mut self) {
        let stale_time = self.current_time - self.stale_threshold;
        self.threats.retain(|t| t.last_updated >= stale_time);
    }

    pub fn primary_threat(&self) -> Option<&ThreatEntry> { self.threats.first() }

    pub fn tick(&mut self, dt: f32) {
        self.current_time += dt;
        self.remove_stale();
    }
}

// ============================================================
// DECISION TREE
// ============================================================

#[derive(Clone, Debug)]
pub enum DecisionTreeNode {
    Decision {
        attribute_key: String,
        threshold: f32,
        left_branch: Box<DecisionTreeNode>,
        right_branch: Box<DecisionTreeNode>,
    },
    Leaf {
        action_label: String,
        action_id: u32,
        confidence: f32,
    },
}

impl DecisionTreeNode {
    pub fn evaluate(&self, blackboard: &Blackboard) -> (u32, String, f32) {
        match self {
            DecisionTreeNode::Leaf { action_id, action_label, confidence } => (*action_id, action_label.clone(), *confidence),
            DecisionTreeNode::Decision { attribute_key, threshold, left_branch, right_branch } => {
                if blackboard.get_float(attribute_key) < *threshold { left_branch.evaluate(blackboard) }
                else { right_branch.evaluate(blackboard) }
            }
        }
    }

    pub fn depth(&self) -> usize {
        match self {
            DecisionTreeNode::Leaf { .. } => 1,
            DecisionTreeNode::Decision { left_branch, right_branch, .. } => 1 + left_branch.depth().max(right_branch.depth()),
        }
    }
}

pub struct DecisionTreeBuilder;
impl DecisionTreeBuilder {
    pub fn build_combat_tree() -> DecisionTreeNode {
        DecisionTreeNode::Decision {
            attribute_key: "self_health".to_string(),
            threshold: 0.3,
            left_branch: Box::new(DecisionTreeNode::Decision {
                attribute_key: "medpack_count".to_string(),
                threshold: 1.0,
                left_branch: Box::new(DecisionTreeNode::Leaf { action_label: "Heal".to_string(), action_id: 10, confidence: 0.95 }),
                right_branch: Box::new(DecisionTreeNode::Decision {
                    attribute_key: "threat_dist".to_string(),
                    threshold: 8.0,
                    left_branch: Box::new(DecisionTreeNode::Leaf { action_label: "Flee".to_string(), action_id: 11, confidence: 0.9 }),
                    right_branch: Box::new(DecisionTreeNode::Leaf { action_label: "TakeCover".to_string(), action_id: 12, confidence: 0.8 }),
                }),
            }),
            right_branch: Box::new(DecisionTreeNode::Decision {
                attribute_key: "enemy_visible".to_string(),
                threshold: 0.5,
                left_branch: Box::new(DecisionTreeNode::Leaf { action_label: "Patrol".to_string(), action_id: 13, confidence: 0.7 }),
                right_branch: Box::new(DecisionTreeNode::Decision {
                    attribute_key: "ammo_count".to_string(),
                    threshold: 5.0,
                    left_branch: Box::new(DecisionTreeNode::Leaf { action_label: "Reload".to_string(), action_id: 15, confidence: 0.85 }),
                    right_branch: Box::new(DecisionTreeNode::Leaf { action_label: "Attack".to_string(), action_id: 16, confidence: 0.9 }),
                }),
            }),
        }
    }
}

// ============================================================
// FUZZY LOGIC
// ============================================================

#[derive(Clone, Debug)]
pub enum FuzzyMembershipType {
    Triangular { left: f32, center: f32, right: f32 },
    Trapezoidal { left_edge: f32, left_plateau: f32, right_plateau: f32, right_edge: f32 },
    Gaussian { center: f32, sigma: f32 },
    Singleton { value: f32 },
}

#[derive(Clone, Debug)]
pub struct FuzzySet {
    pub name: String,
    pub membership_type: FuzzyMembershipType,
}

impl FuzzySet {
    pub fn membership(&self, x: f32) -> f32 {
        match &self.membership_type {
            FuzzyMembershipType::Triangular { left, center, right } => {
                if x <= *left || x >= *right { 0.0 }
                else if x <= *center { (x - left) / (center - left + EPSILON) }
                else { (right - x) / (right - center + EPSILON) }
            }
            FuzzyMembershipType::Trapezoidal { left_edge, left_plateau, right_plateau, right_edge } => {
                if x <= *left_edge || x >= *right_edge { 0.0 }
                else if x <= *left_plateau { (x - left_edge) / (left_plateau - left_edge + EPSILON) }
                else if x <= *right_plateau { 1.0 }
                else { (right_edge - x) / (right_edge - right_plateau + EPSILON) }
            }
            FuzzyMembershipType::Gaussian { center, sigma } => {
                let d = (x - center) / (sigma + EPSILON);
                (-0.5 * d * d).exp()
            }
            FuzzyMembershipType::Singleton { value } => {
                if (x - value).abs() < EPSILON { 1.0 } else { 0.0 }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct FuzzyRule {
    pub input_set_indices: Vec<usize>,
    pub output_set_index: usize,
    pub weight: f32,
}

pub struct FuzzyInferenceSystem {
    pub input_sets: Vec<Vec<FuzzySet>>,
    pub output_sets: Vec<FuzzySet>,
    pub rules: Vec<FuzzyRule>,
    pub input_variables: Vec<String>,
    pub output_variable: String,
}

impl FuzzyInferenceSystem {
    pub fn new(output_var: &str) -> Self {
        Self { input_sets: Vec::new(), output_sets: Vec::new(), rules: Vec::new(), input_variables: Vec::new(), output_variable: output_var.to_string() }
    }

    pub fn add_input(&mut self, name: &str, sets: Vec<FuzzySet>) -> usize {
        let idx = self.input_sets.len();
        self.input_variables.push(name.to_string());
        self.input_sets.push(sets);
        idx
    }

    pub fn add_output_sets(&mut self, sets: Vec<FuzzySet>) { self.output_sets = sets; }

    pub fn add_rule(&mut self, input_set_indices: Vec<usize>, output_set_index: usize, weight: f32) {
        self.rules.push(FuzzyRule { input_set_indices, output_set_index, weight });
    }

    pub fn infer(&self, inputs: &[f32], output_range: (f32, f32), resolution: usize) -> f32 {
        let mut output_activations: Vec<f32> = vec![0.0; self.output_sets.len()];
        for rule in &self.rules {
            let mut activation = rule.weight;
            for (input_idx, &set_idx) in rule.input_set_indices.iter().enumerate() {
                if input_idx >= inputs.len() || input_idx >= self.input_sets.len() { break; }
                let m = if set_idx < self.input_sets[input_idx].len() { self.input_sets[input_idx][set_idx].membership(inputs[input_idx]) } else { 0.0 };
                activation = activation.min(m);
            }
            if rule.output_set_index < output_activations.len() {
                output_activations[rule.output_set_index] = output_activations[rule.output_set_index].max(activation);
            }
        }
        let (lo, hi) = output_range;
        let step = (hi - lo) / resolution.max(1) as f32;
        let mut num = 0.0f32;
        let mut den = 0.0f32;
        for i in 0..resolution {
            let x = lo + i as f32 * step + step * 0.5;
            let mut max_mem = 0.0f32;
            for (j, set) in self.output_sets.iter().enumerate() {
                let clipped = set.membership(x).min(output_activations.get(j).copied().unwrap_or(0.0));
                max_mem = max_mem.max(clipped);
            }
            num += x * max_mem;
            den += max_mem;
        }
        if den < EPSILON { (lo + hi) * 0.5 } else { num / den }
    }
}

pub struct FuzzyBehaviorController;
impl FuzzyBehaviorController {
    pub fn build_aggressiveness_fis() -> FuzzyInferenceSystem {
        let mut fis = FuzzyInferenceSystem::new("aggressiveness");
        fis.add_input("health", vec![
            FuzzySet { name: "low".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 0.0, center: 0.0, right: 0.4 } },
            FuzzySet { name: "medium".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 0.2, center: 0.5, right: 0.8 } },
            FuzzySet { name: "high".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 0.6, center: 1.0, right: 1.0 } },
        ]);
        fis.add_input("threat_count", vec![
            FuzzySet { name: "few".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 0.0, center: 0.0, right: 3.0 } },
            FuzzySet { name: "moderate".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 1.0, center: 4.0, right: 7.0 } },
            FuzzySet { name: "many".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 5.0, center: 10.0, right: 10.0 } },
        ]);
        fis.add_output_sets(vec![
            FuzzySet { name: "cowardly".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 0.0, center: 0.0, right: 0.3 } },
            FuzzySet { name: "cautious".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 0.1, center: 0.4, right: 0.7 } },
            FuzzySet { name: "aggressive".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 0.5, center: 0.8, right: 1.0 } },
            FuzzySet { name: "berserker".to_string(), membership_type: FuzzyMembershipType::Triangular { left: 0.8, center: 1.0, right: 1.0 } },
        ]);
        fis.add_rule(vec![2, 0], 2, 1.0);
        fis.add_rule(vec![2, 2], 1, 1.0);
        fis.add_rule(vec![1, 0], 2, 0.8);
        fis.add_rule(vec![1, 1], 1, 0.9);
        fis.add_rule(vec![0, 0], 1, 0.7);
        fis.add_rule(vec![0, 1], 0, 1.0);
        fis.add_rule(vec![0, 2], 0, 1.0);
        fis.add_rule(vec![2, 0], 3, 0.5);
        fis
    }
}

// ============================================================
// HTN PLANNER
// ============================================================

#[derive(Clone, Debug)]
pub enum HtnTask {
    Primitive { name: String, action_id: u32, preconditions: WorldState, effects_set: WorldState, effects_clear: WorldState, cost: f32 },
    Compound { name: String, methods: Vec<HtnMethod> },
}

#[derive(Clone, Debug)]
pub struct HtnMethod {
    pub name: String,
    pub preconditions: WorldState,
    pub subtasks: Vec<String>,
    pub priority: i32,
}

pub struct HtnPlanner {
    pub tasks: HashMap<String, HtnTask>,
    pub root_task: String,
}

impl HtnPlanner {
    pub fn new(root_task: &str) -> Self { Self { tasks: HashMap::new(), root_task: root_task.to_string() } }

    pub fn add_task(&mut self, name: &str, task: HtnTask) { self.tasks.insert(name.to_string(), task); }

    pub fn plan(&self, world_state: WorldState) -> Vec<u32> {
        let mut plan = Vec::new();
        let mut tasks_to_process: VecDeque<String> = VecDeque::new();
        tasks_to_process.push_back(self.root_task.clone());
        let mut current_state = world_state;
        let mut depth = 0;

        while let Some(task_name) = tasks_to_process.pop_front() {
            if depth > 50 { break; }
            depth += 1;
            if let Some(task) = self.tasks.get(&task_name) {
                match task {
                    HtnTask::Primitive { action_id, preconditions, effects_set, effects_clear, .. } => {
                        if (current_state & preconditions) == *preconditions {
                            plan.push(*action_id);
                            current_state = (current_state | effects_set) & !effects_clear;
                        }
                    }
                    HtnTask::Compound { methods, .. } => {
                        let mut sorted_methods: Vec<&HtnMethod> = methods.iter().collect();
                        sorted_methods.sort_by(|a, b| b.priority.cmp(&a.priority));
                        for method in sorted_methods {
                            if (current_state & method.preconditions) == method.preconditions {
                                let existing: Vec<String> = tasks_to_process.iter().cloned().collect();
                                tasks_to_process.clear();
                                for st in &method.subtasks { tasks_to_process.push_back(st.clone()); }
                                for et in existing { tasks_to_process.push_back(et); }
                                break;
                            }
                        }
                    }
                }
            }
        }
        plan
    }

    pub fn build_combat_network() -> HtnPlanner {
        let mut planner = HtnPlanner::new("BeSoldier");
        planner.add_task("BeSoldier", HtnTask::Compound {
            name: "BeSoldier".to_string(),
            methods: vec![
                HtnMethod { name: "Fight".to_string(), preconditions: 0b0000_0011, subtasks: vec!["EngageEnemy".to_string()], priority: 10 },
                HtnMethod { name: "GetAmmo".to_string(), preconditions: 0, subtasks: vec!["FindAmmo".to_string(), "Reload".to_string()], priority: 5 },
                HtnMethod { name: "Patrol".to_string(), preconditions: 0, subtasks: vec!["PatrolArea".to_string()], priority: 1 },
            ],
        });
        planner.add_task("EngageEnemy", HtnTask::Compound {
            name: "EngageEnemy".to_string(),
            methods: vec![
                HtnMethod { name: "ShootEnemy".to_string(), preconditions: 0b0000_0001, subtasks: vec!["MoveToAttackPos".to_string(), "Shoot".to_string()], priority: 10 },
                HtnMethod { name: "MeleeEnemy".to_string(), preconditions: 0, subtasks: vec!["MoveToMeleePos".to_string(), "MeleeAttack".to_string()], priority: 5 },
            ],
        });
        planner.add_task("Shoot", HtnTask::Primitive { name: "Shoot".to_string(), action_id: 101, preconditions: 0b11, effects_set: 0b100, effects_clear: 0b10, cost: 1.0 });
        planner.add_task("MeleeAttack", HtnTask::Primitive { name: "MeleeAttack".to_string(), action_id: 102, preconditions: 0b10, effects_set: 0b100, effects_clear: 0b10, cost: 1.5 });
        planner.add_task("MoveToAttackPos", HtnTask::Primitive { name: "MoveToAttackPos".to_string(), action_id: 103, preconditions: 0b10, effects_set: 0b1_0000, effects_clear: 0, cost: 2.0 });
        planner.add_task("MoveToMeleePos", HtnTask::Primitive { name: "MoveToMeleePos".to_string(), action_id: 104, preconditions: 0b10, effects_set: 0b10_0000, effects_clear: 0, cost: 3.0 });
        planner.add_task("Reload", HtnTask::Primitive { name: "Reload".to_string(), action_id: 105, preconditions: 0, effects_set: 1, effects_clear: 0, cost: 1.5 });
        planner.add_task("FindAmmo", HtnTask::Primitive { name: "FindAmmo".to_string(), action_id: 106, preconditions: 0, effects_set: 0b100_0000, effects_clear: 0, cost: 2.5 });
        planner.add_task("PatrolArea", HtnTask::Primitive { name: "PatrolArea".to_string(), action_id: 107, preconditions: 0, effects_set: 0b10, effects_clear: 0, cost: 1.0 });
        planner
    }
}

// ============================================================
// BEHAVIOR TREE SERIALIZER
// ============================================================

pub struct BtSerializer;
impl BtSerializer {
    pub fn serialize(tree: &BehaviorTree) -> String {
        let mut out = String::new();
        out.push_str(&format!("tree \"{}\" {{\n", tree.name));
        if let Some(root) = tree.root_id { Self::serialize_node(tree, root, &mut out, 1); }
        out.push_str("}\n");
        out
    }

    fn serialize_node(tree: &BehaviorTree, node_id: u32, out: &mut String, depth: usize) {
        if depth > 30 { return; }
        let indent = "  ".repeat(depth);
        if let Some(node) = tree.nodes.get(&node_id) {
            out.push_str(&format!("{}node {} [id={}] {{\n", indent, node.display_name(), node.id));
            for &child_id in &node.children { Self::serialize_node(tree, child_id, out, depth + 1); }
            out.push_str(&format!("{}}}\n", indent));
        }
    }
}

// ============================================================
// AI EVENT BUS
// ============================================================

#[derive(Clone, Debug)]
pub enum SoundType { Footstep, Gunshot, Explosion, Voice, Ambient }

#[derive(Clone, Debug)]
pub enum AiSignal {
    EnemySpotted { spotter_id: u64, enemy_id: u64, position: Vec3 },
    AllyKilled { ally_id: u64, position: Vec3, killer_id: u64 },
    SoundHeard { listener_id: u64, source_pos: Vec3, sound_type: SoundType, intensity: f32 },
    ItemPickedUp { agent_id: u64, item_id: String },
    ObjectiveReached { agent_id: u64, objective_id: u32 },
    FormationBreak { squad_id: u32, reason: String },
    EmotionalEvent { agent_id: u64, emotion: PrimaryEmotion, intensity: f32 },
    DamageTaken { agent_id: u64, damage: f32, source_id: u64, source_pos: Vec3 },
    AgentDied { agent_id: u64, position: Vec3 },
    TargetLost { agent_id: u64, last_known_pos: Vec3 },
    CoverReached { agent_id: u64, cover_id: u32 },
    BehaviorChanged { agent_id: u64, from_mode: String, to_mode: String },
}

pub struct AiEventBus {
    pub events: VecDeque<(f32, AiSignal)>,
    pub history: VecDeque<(f32, AiSignal)>,
    pub max_history: usize,
    pub current_time: f32,
}

impl AiEventBus {
    pub fn new() -> Self {
        Self { events: VecDeque::with_capacity(256), history: VecDeque::with_capacity(512), max_history: 512, current_time: 0.0 }
    }

    pub fn publish(&mut self, signal: AiSignal) {
        self.events.push_back((self.current_time, signal.clone()));
        self.history.push_back((self.current_time, signal));
        if self.history.len() > self.max_history { self.history.pop_front(); }
    }

    pub fn drain(&mut self) -> Vec<(f32, AiSignal)> { self.events.drain(..).collect() }
    pub fn tick(&mut self, dt: f32) { self.current_time += dt; }
}

// ============================================================
// SPATIAL GRID
// ============================================================

pub struct SpatialGrid {
    pub cell_size: f32,
    pub cells: HashMap<(i32, i32), Vec<u64>>,
    pub agent_cells: HashMap<u64, (i32, i32)>,
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self { Self { cell_size, cells: HashMap::new(), agent_cells: HashMap::new() } }

    pub fn cell_of(&self, pos: Vec3) -> (i32, i32) {
        ((pos.x / self.cell_size).floor() as i32, (pos.z / self.cell_size).floor() as i32)
    }

    pub fn insert(&mut self, id: u64, pos: Vec3) {
        let cell = self.cell_of(pos);
        self.cells.entry(cell).or_default().push(id);
        self.agent_cells.insert(id, cell);
    }

    pub fn remove(&mut self, id: u64) {
        if let Some(cell) = self.agent_cells.remove(&id) {
            if let Some(v) = self.cells.get_mut(&cell) { v.retain(|&x| x != id); }
        }
    }

    pub fn update(&mut self, id: u64, pos: Vec3) {
        let new_cell = self.cell_of(pos);
        if let Some(&old_cell) = self.agent_cells.get(&id) {
            if old_cell != new_cell {
                if let Some(v) = self.cells.get_mut(&old_cell) { v.retain(|&x| x != id); }
                self.cells.entry(new_cell).or_default().push(id);
                self.agent_cells.insert(id, new_cell);
            }
        }
    }

    pub fn query_radius(&self, pos: Vec3, radius: f32) -> Vec<u64> {
        let cell_radius = (radius / self.cell_size).ceil() as i32 + 1;
        let center_cell = self.cell_of(pos);
        let mut results = Vec::new();
        for dx in -cell_radius..=cell_radius {
            for dz in -cell_radius..=cell_radius {
                if let Some(agents) = self.cells.get(&(center_cell.0 + dx, center_cell.1 + dz)) {
                    results.extend_from_slice(agents);
                }
            }
        }
        results
    }

    pub fn clear(&mut self) { self.cells.clear(); self.agent_cells.clear(); }

    pub fn rebuild(&mut self, agents: &[(u64, Vec3)]) {
        self.clear();
        for &(id, pos) in agents { self.insert(id, pos); }
    }
}

// ============================================================
// NAV MESH (simplified)
// ============================================================

#[derive(Clone, Debug)]
pub struct NavRegion {
    pub id: u32,
    pub vertices: Vec<Vec2>,
    pub center: Vec2,
    pub connections: Vec<NavConnection>,
    pub cost_modifier: f32,
}

#[derive(Clone, Debug)]
pub struct NavConnection {
    pub to_region: u32,
    pub portal_start: Vec2,
    pub portal_end: Vec2,
    pub traversal_cost: f32,
}

impl NavRegion {
    pub fn new(id: u32, vertices: Vec<Vec2>) -> Self {
        let center = if vertices.is_empty() { Vec2::ZERO }
        else { vertices.iter().copied().fold(Vec2::ZERO, |a, b| a + b) / vertices.len() as f32 };
        Self { id, vertices, center, connections: Vec::new(), cost_modifier: 1.0 }
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        let n = self.vertices.len();
        if n < 3 { return false; }
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let vi = self.vertices[i];
            let vj = self.vertices[j];
            if ((vi.y > p.y) != (vj.y > p.y)) && (p.x < (vj.x - vi.x) * (p.y - vi.y) / (vj.y - vi.y + EPSILON) + vi.x) {
                inside = !inside;
            }
            j = i;
        }
        inside
    }
}

pub struct NavMesh {
    pub regions: HashMap<u32, NavRegion>,
    pub next_id: u32,
}

impl NavMesh {
    pub fn new() -> Self { Self { regions: HashMap::new(), next_id: 1 } }

    pub fn add_region(&mut self, vertices: Vec<Vec2>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.regions.insert(id, NavRegion::new(id, vertices));
        id
    }

    pub fn connect_regions(&mut self, a: u32, b: u32, portal_start: Vec2, portal_end: Vec2, cost: f32) {
        if let Some(ra) = self.regions.get_mut(&a) {
            ra.connections.push(NavConnection { to_region: b, portal_start, portal_end, traversal_cost: cost });
        }
        if let Some(rb) = self.regions.get_mut(&b) {
            rb.connections.push(NavConnection { to_region: a, portal_start: portal_end, portal_end: portal_start, traversal_cost: cost });
        }
    }

    pub fn find_region(&self, pos: Vec2) -> Option<u32> {
        self.regions.iter().find(|(_, r)| r.contains_point(pos)).map(|(&id, _)| id)
    }

    pub fn find_path_regions(&self, from_region: u32, to_region: u32) -> Option<Vec<u32>> {
        if from_region == to_region { return Some(vec![from_region]); }
        let goal_center = self.regions.get(&to_region)?.center;
        let h = |rid: u32| self.regions.get(&rid).map(|r| (r.center - goal_center).length()).unwrap_or(f32::MAX);

        let mut open: HashMap<u32, (f32, f32, Option<u32>)> = HashMap::new(); // id -> (g, h, parent)
        let mut closed: HashMap<u32, (f32, Option<u32>)> = HashMap::new();
        open.insert(from_region, (0.0, h(from_region), None));

        while !open.is_empty() {
            let (&cur_id, _) = open.iter().min_by(|a, b| {
                let fa = a.1.0 + a.1.1;
                let fb = b.1.0 + b.1.1;
                fa.partial_cmp(&fb).unwrap()
            })?;
            let (g, _, parent) = open.remove(&cur_id)?;
            closed.insert(cur_id, (g, parent));

            if cur_id == to_region {
                let mut path = vec![cur_id];
                let mut c = cur_id;
                while let Some((_, Some(p))) = closed.get(&c) { path.push(*p); c = *p; }
                path.reverse();
                return Some(path);
            }
            if let Some(region) = self.regions.get(&cur_id) {
                for conn in &region.connections {
                    let nid = conn.to_region;
                    if closed.contains_key(&nid) { continue; }
                    let new_g = g + conn.traversal_cost;
                    let new_h = h(nid);
                    if let Some((og, _, _)) = open.get(&nid) { if *og <= new_g { continue; } }
                    open.insert(nid, (new_g, new_h, Some(cur_id)));
                }
            }
            if closed.len() > 2048 { break; }
        }
        None
    }
}


// ============================================================
// AI LOD MANAGER
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiLodLevel { Full, Medium, Low, Dormant }

pub struct AiLodManager {
    pub agent_lods: HashMap<u64, AiLodLevel>,
    pub camera_pos: Vec3,
    pub full_radius: f32,
    pub medium_radius: f32,
    pub low_radius: f32,
    pub force_full: HashSet<u64>,
}

impl AiLodManager {
    pub fn new(full_radius: f32, medium_radius: f32, low_radius: f32) -> Self {
        Self { agent_lods: HashMap::new(), camera_pos: Vec3::ZERO, full_radius, medium_radius, low_radius, force_full: HashSet::new() }
    }

    pub fn update(&mut self, agent_positions: &HashMap<u64, Vec3>) {
        for (&id, &pos) in agent_positions {
            let lod = if self.force_full.contains(&id) { AiLodLevel::Full }
            else {
                let d = (pos - self.camera_pos).length();
                if d < self.full_radius { AiLodLevel::Full }
                else if d < self.medium_radius { AiLodLevel::Medium }
                else if d < self.low_radius { AiLodLevel::Low }
                else { AiLodLevel::Dormant }
            };
            self.agent_lods.insert(id, lod);
        }
    }

    pub fn get_lod(&self, id: u64) -> AiLodLevel { self.agent_lods.get(&id).copied().unwrap_or(AiLodLevel::Dormant) }

    pub fn lod_update_freq(&self, lod: AiLodLevel) -> f32 {
        match lod {
            AiLodLevel::Full => 1.0 / BT_TICK_RATE_HZ,
            AiLodLevel::Medium => 0.1,
            AiLodLevel::Low => 0.5,
            AiLodLevel::Dormant => f32::MAX,
        }
    }

    pub fn should_update(&self, id: u64, last_update: f32, current_time: f32) -> bool {
        (current_time - last_update) >= self.lod_update_freq(self.get_lod(id))
    }

    pub fn counts_by_lod(&self) -> (usize, usize, usize, usize) {
        let f = self.agent_lods.values().filter(|&&l| l == AiLodLevel::Full).count();
        let m = self.agent_lods.values().filter(|&&l| l == AiLodLevel::Medium).count();
        let l = self.agent_lods.values().filter(|&&l| l == AiLodLevel::Low).count();
        let d = self.agent_lods.values().filter(|&&l| l == AiLodLevel::Dormant).count();
        (f, m, l, d)
    }
}

// ============================================================
// PERFORMANCE MONITOR
// ============================================================

pub struct PerformanceMonitor {
    pub bt_times: VecDeque<f32>,
    pub perception_times: VecDeque<f32>,
    pub steering_times: VecDeque<f32>,
    pub total_times: VecDeque<f32>,
    pub history_len: usize,
    pub frame: u64,
}

impl PerformanceMonitor {
    pub fn new(history_len: usize) -> Self {
        Self { bt_times: VecDeque::with_capacity(history_len), perception_times: VecDeque::with_capacity(history_len), steering_times: VecDeque::with_capacity(history_len), total_times: VecDeque::with_capacity(history_len), history_len, frame: 0 }
    }

    pub fn record(&mut self, bt: f32, percept: f32, steering: f32, _goap: f32) {
        self.frame += 1;
        macro_rules! push_b { ($q:expr, $v:expr) => { $q.push_back($v); if $q.len() > self.history_len { $q.pop_front(); } } }
        push_b!(self.bt_times, bt);
        push_b!(self.perception_times, percept);
        push_b!(self.steering_times, steering);
        push_b!(self.total_times, bt + percept + steering);
    }

    pub fn avg_total(&self) -> f32 { if self.total_times.is_empty() { 0.0 } else { self.total_times.iter().sum::<f32>() / self.total_times.len() as f32 } }
    pub fn peak_total(&self) -> f32 { self.total_times.iter().copied().fold(0.0f32, f32::max) }
    pub fn avg_bt(&self) -> f32 { if self.bt_times.is_empty() { 0.0 } else { self.bt_times.iter().sum::<f32>() / self.bt_times.len() as f32 } }
}

// ============================================================
// CAMERA DIRECTOR AI
// ============================================================

#[derive(Clone, Debug)]
pub enum FramingRule {
    ThirdPerson { angle_yaw: f32, angle_pitch: f32 },
    OverShoulder { shoulder_offset: Vec3 },
    TopDown { height: f32 },
    FreeOrbit { orbit_angle: f32, orbit_pitch: f32 },
}

pub struct CameraDirectorAi {
    pub camera_pos: Vec3,
    pub camera_velocity: Vec3,
    pub camera_target: Vec3,
    pub smoothing: f32,
    pub look_ahead_factor: f32,
    pub distance: f32,
    pub height_offset: f32,
    pub max_speed: f32,
    pub framing_rule: FramingRule,
    pub cut_threshold: f32,
}

impl CameraDirectorAi {
    pub fn new() -> Self {
        Self { camera_pos: Vec3::new(0.0, 5.0, -10.0), camera_velocity: Vec3::ZERO, camera_target: Vec3::ZERO, smoothing: 5.0, look_ahead_factor: 2.0, distance: 8.0, height_offset: 3.0, max_speed: 20.0, framing_rule: FramingRule::ThirdPerson { angle_yaw: 0.0, angle_pitch: 0.3 }, cut_threshold: 30.0 }
    }

    pub fn update(&mut self, target_pos: Vec3, target_velocity: Vec3, dt: f32) {
        let predicted = target_pos + target_velocity * self.look_ahead_factor * 0.5;
        let desired = match &self.framing_rule {
            FramingRule::ThirdPerson { angle_yaw, angle_pitch } => {
                let yaw = *angle_yaw; let pitch = *angle_pitch;
                let offset = Vec3::new(yaw.sin() * self.distance * pitch.cos(), self.height_offset + self.distance * pitch.sin(), yaw.cos() * self.distance * pitch.cos());
                predicted + offset
            }
            FramingRule::TopDown { height } => Vec3::new(predicted.x, *height, predicted.z),
            FramingRule::OverShoulder { shoulder_offset } => predicted + *shoulder_offset,
            FramingRule::FreeOrbit { orbit_angle, orbit_pitch } => {
                let ang = *orbit_angle; let pitch = *orbit_pitch;
                let offset = Vec3::new(ang.cos() * self.distance * pitch.cos(), self.distance * pitch.sin() + self.height_offset, ang.sin() * self.distance * pitch.cos());
                predicted + offset
            }
        };
        let dist = (desired - self.camera_pos).length();
        if dist > self.cut_threshold {
            self.camera_pos = desired;
            self.camera_velocity = Vec3::ZERO;
        } else {
            self.camera_pos = smooth_damp_vec3(self.camera_pos, desired, &mut self.camera_velocity, 1.0 / self.smoothing, self.max_speed, dt);
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.camera_pos, self.camera_target, Vec3::Y)
    }
}

// ============================================================
// SENSOR FUSION
// ============================================================

#[derive(Clone, Debug)]
pub struct FusedBelief {
    pub entity_id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub confidence: f32,
    pub sensor_contributions: [f32; 4],
    pub last_fused: f32,
    pub threat: f32,
}

pub struct SensorFusion {
    pub vision_weight: f32,
    pub hearing_weight: f32,
    pub smell_weight: f32,
    pub memory_weight: f32,
    pub fused_beliefs: HashMap<u64, FusedBelief>,
    pub decay_rate: f32,
    pub current_time: f32,
}

impl SensorFusion {
    pub fn new() -> Self {
        Self { vision_weight: 1.0, hearing_weight: 0.6, smell_weight: 0.3, memory_weight: 0.4, fused_beliefs: HashMap::new(), decay_rate: 0.1, current_time: 0.0 }
    }

    pub fn fuse(&mut self, entity_id: u64, vision_pos: Option<(Vec3, f32)>, hearing_pos: Option<(Vec3, f32)>, smell_pos: Option<(Vec3, f32)>, memory_pos: Option<(Vec3, f32)>, threat: f32) {
        let mut total_weight = 0.0f32;
        let mut fused_pos = Vec3::ZERO;
        let mut contributions = [0.0f32; 4];
        macro_rules! add_s { ($sensor:expr, $weight:expr, $idx:expr) => { if let Some((pos, conf)) = $sensor { let w = $weight * conf; fused_pos += pos * w; total_weight += w; contributions[$idx] = w; } } }
        add_s!(vision_pos, self.vision_weight, 0);
        add_s!(hearing_pos, self.hearing_weight, 1);
        add_s!(smell_pos, self.smell_weight, 2);
        add_s!(memory_pos, self.memory_weight, 3);
        if total_weight > EPSILON {
            fused_pos /= total_weight;
            let confidence = (total_weight / (self.vision_weight + self.hearing_weight + self.smell_weight + self.memory_weight)).min(1.0);
            let belief = self.fused_beliefs.entry(entity_id).or_insert_with(|| FusedBelief { entity_id, position: fused_pos, velocity: Vec3::ZERO, confidence: 0.0, sensor_contributions: [0.0; 4], last_fused: self.current_time, threat: 0.0 });
            let dt = (self.current_time - belief.last_fused).max(EPSILON as f32);
            belief.velocity = (fused_pos - belief.position) / dt;
            belief.position = fused_pos;
            belief.confidence = confidence;
            belief.sensor_contributions = contributions;
            belief.last_fused = self.current_time;
            belief.threat = threat;
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.current_time += dt;
        let to_remove: Vec<u64> = self.fused_beliefs.iter_mut().filter_map(|(id, b)| { b.confidence = (b.confidence - self.decay_rate * dt).max(0.0); b.position += b.velocity * dt; if b.confidence < 0.02 { Some(*id) } else { None } }).collect();
        for id in to_remove { self.fused_beliefs.remove(&id); }
    }

    pub fn most_confident(&self) -> Option<&FusedBelief> {
        self.fused_beliefs.values().max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
    }
}




// ============================================================
// EDITOR WINDOW LAYOUT
// ============================================================

#[derive(Clone, Debug)]
pub struct EditorWindowLayout {
    pub viewport_size: Vec2,
}

impl EditorWindowLayout {
    pub fn default_layout(viewport_size: Vec2) -> Self {
        Self { viewport_size }
    }
}

// ============================================================
// FULL AI SYSTEM INTEGRATOR
// ============================================================

pub struct AiSystemIntegrator {
    pub editor: AiBehaviorEditor,
    pub spatial_grid: SpatialGrid,
    pub cover_system: CoverSystem,
    pub event_bus: AiEventBus,
    pub lod_manager: AiLodManager,
    pub perf_monitor: PerformanceMonitor,
    pub nav_mesh: NavMesh,
    pub world_tracker: WorldStateTracker,
    pub htn_planner: HtnPlanner,
    pub pathfinder: GridPathfinder,
    pub camera_director: CameraDirectorAi,
    pub agent_memories: HashMap<u64, AgentMemory>,
    pub agent_social_graphs: HashMap<u64, SocialGraph>,
    pub agent_fusion: HashMap<u64, SensorFusion>,
    pub squad_ais: Vec<SquadAi>,
    pub decision_trees: HashMap<String, DecisionTreeNode>,
    pub fuzzy_systems: HashMap<String, FuzzyInferenceSystem>,
    pub behavior_modulators: HashMap<u64, BehaviorModulator>,
    pub noise: ValueNoise,
    pub window_layout: EditorWindowLayout,
}

impl AiSystemIntegrator {
    pub fn new() -> Self {
        let editor = AiBehaviorEditor::new();
        let pathfinder = GridPathfinder::new(100, 100, 1.0, Vec2::new(-50.0, -50.0));
        let htn_planner = HtnPlanner::build_combat_network();
        let nav_mesh = NavMesh::new();
        let mut cover_system = CoverSystem::new();
        let obstacles: Vec<Aabb> = vec![
            Aabb::new(Vec3::new(10.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 2.0)),
            Aabb::new(Vec3::new(-5.0, 0.0, 8.0), Vec3::new(1.5, 1.0, 1.5)),
        ];
        cover_system.generate_cover_from_obstacles(&obstacles, &[Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]);
        let mut decision_trees = HashMap::new();
        decision_trees.insert("combat".to_string(), DecisionTreeBuilder::build_combat_tree());
        let mut fuzzy_systems = HashMap::new();
        fuzzy_systems.insert("aggressiveness".to_string(), FuzzyBehaviorController::build_aggressiveness_fis());
        let window_layout = EditorWindowLayout::default_layout(Vec2::new(1920.0, 1080.0));

        Self {
            spatial_grid: SpatialGrid::new(5.0),
            cover_system,
            event_bus: AiEventBus::new(),
            lod_manager: AiLodManager::new(15.0, 40.0, 80.0),
            perf_monitor: PerformanceMonitor::new(128),
            nav_mesh,
            world_tracker: WorldStateTracker::new(),
            htn_planner,
            pathfinder,
            camera_director: CameraDirectorAi::new(),
            agent_memories: HashMap::new(),
            agent_social_graphs: HashMap::new(),
            agent_fusion: HashMap::new(),
            squad_ais: Vec::new(),
            decision_trees,
            fuzzy_systems,
            behavior_modulators: HashMap::new(),
            noise: ValueNoise::new(42),
            window_layout,
            editor,
        }
    }

    pub fn full_update(&mut self, dt: f32) {
        let positions: Vec<(u64, Vec3)> = self.editor.agents.iter().map(|a| (a.id, a.position)).collect();
        self.spatial_grid.rebuild(&positions);
        let pos_map: HashMap<u64, Vec3> = positions.iter().copied().collect();
        self.lod_manager.camera_pos = Vec3::new(0.0, 5.0, 0.0);
        self.lod_manager.update(&pos_map);
        self.event_bus.tick(dt);
        self.world_tracker.tick(dt);
        let perceived_empty: Vec<PerceivedEntity> = Vec::new();
        for squad in &mut self.squad_ais { squad.tick(dt, &perceived_empty, &pos_map); }
        self.editor.simulation_tick(dt);
        for memory in self.agent_memories.values_mut() { memory.update(dt); }
        for graph in self.agent_social_graphs.values_mut() { graph.update(dt, self.editor.current_time); }
        for fusion in self.agent_fusion.values_mut() { fusion.update(dt); }
        for modulator in self.behavior_modulators.values_mut() { modulator.update(dt); }
        if let Some(id) = self.editor.selected_agent_id {
            if let Some(agent) = self.editor.agents.iter().find(|a| a.id == id) {
                self.camera_director.update(agent.position, agent.velocity, dt);
            }
        }
        self.perf_monitor.record(0.1, 0.05, 0.03, 0.02);
    }

    pub fn spawn_squad(&mut self, leader_id: u64, formation: FormationType) {
        let mut squad = SquadAi::new(leader_id);
        squad.formation = formation;
        if let Some(leader) = self.editor.agents.iter().find(|a| a.id == leader_id) {
            let leader_pos = leader.position;
            let nearby = self.spatial_grid.query_radius(leader_pos, 10.0);
            for id in nearby { squad.add_agent(id); }
        }
        self.squad_ais.push(squad);
    }

    pub fn query_decision_tree(&self, tree_name: &str, blackboard: &Blackboard) -> Option<(u32, String)> {
        let tree = self.decision_trees.get(tree_name)?;
        let (id, label, _) = tree.evaluate(blackboard);
        Some((id, label))
    }

    pub fn query_fuzzy(&self, system_name: &str, inputs: &[f32]) -> Option<f32> {
        let fis = self.fuzzy_systems.get(system_name)?;
        Some(fis.infer(inputs, (0.0, 1.0), 100))
    }

    pub fn broadcast_event(&mut self, signal: AiSignal) { self.event_bus.publish(signal); }

    pub fn stats_summary(&self) -> String {
        let mut out = String::new();
        out.push_str("=== AI System Status ===\n");
        out.push_str(&format!("Agents: {}\n", self.editor.agents.len()));
        out.push_str(&format!("BTs: {}  FSMs: {}  Squads: {}\n", self.editor.behavior_trees.len(), self.editor.fsm_instances.len(), self.squad_ais.len()));
        out.push_str(&format!("Cover Points: {}\n", self.cover_system.cover_points.len()));
        let (f, m, l, d) = self.lod_manager.counts_by_lod();
        out.push_str(&format!("LOD: Full={} Med={} Low={} Dormant={}\n", f, m, l, d));
        out.push_str(&format!("Avg AI: {:.3}ms  Peak: {:.3}ms\n", self.perf_monitor.avg_total(), self.perf_monitor.peak_total()));
        out.push_str(&format!("Sim time: {:.2}s\n", self.editor.current_time));
        out
    }
}

pub fn compute_intercept_point(shooter_pos: Vec3, projectile_speed: f32, target_pos: Vec3, target_vel: Vec3) -> Option<Vec3> {
    let to_target = target_pos - shooter_pos;
    let a = target_vel.length_squared() - projectile_speed * projectile_speed;
    let b = 2.0 * to_target.dot(target_vel);
    let c = to_target.length_squared();
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 { return None; }
    let sqrt_disc = discriminant.sqrt();
    let t1 = (-b + sqrt_disc) / (2.0 * a + EPSILON);
    let t2 = (-b - sqrt_disc) / (2.0 * a + EPSILON);
    let t = [t1, t2].iter().filter(|&&t| t > 0.0).copied().fold(f32::MAX, f32::min);
    if t == f32::MAX { None } else { Some(target_pos + target_vel * t) }
}

pub fn effective_range_modifier(distance: f32, weapon_range: f32, falloff_start: f32) -> f32 {
    if distance > weapon_range { return 0.0; }
    if distance <= falloff_start { return 1.0; }
    let t = (distance - falloff_start) / (weapon_range - falloff_start + EPSILON);
    1.0 - t * t
}

pub fn compute_flanking_score(attacker_pos: Vec3, defender_pos: Vec3, defender_forward: Vec3) -> f32 {
    let to_attacker = (attacker_pos - defender_pos).normalize_or_zero();
    (1.0 - defender_forward.dot(to_attacker)) * 0.5
}

pub fn clamp_angle(angle: f32) -> f32 {
    let mut a = angle % TWO_PI;
    if a > PI { a -= TWO_PI; }
    if a < -PI { a += TWO_PI; }
    a
}

pub fn project_onto_plane(v: Vec3, plane_normal: Vec3) -> Vec3 {
    v - plane_normal * v.dot(plane_normal)
}

pub fn reflect_vector(v: Vec3, normal: Vec3) -> Vec3 {
    v - normal * (2.0 * v.dot(normal))
}

pub fn frustum_cull_sphere(center: Vec3, radius: f32, frustum_planes: &[(Vec3, f32)]) -> bool {
    for &(normal, d) in frustum_planes {
        if normal.dot(center) + d < -radius { return false; }
    }
    true
}

pub fn pack_behavior_config(agent: &AiAgent) -> HashMap<String, f32> {
    let mut cfg = HashMap::new();
    cfg.insert("health".to_string(), agent.blackboard.get_float("self_health"));
    cfg.insert("ammo".to_string(), agent.blackboard.get_float("ammo_count"));
    cfg.insert("emotion_valence".to_string(), agent.emotion_engine.state.mood_valence);
    cfg.insert("emotion_arousal".to_string(), agent.emotion_engine.state.mood_arousal);
    cfg.insert("speed_mult".to_string(), agent.emotion_engine.get_modifier().speed_multiplier);
    cfg
}

pub fn global_editor_init() -> AiSystemIntegrator {
    let mut integrator = AiSystemIntegrator::new();
    integrator.editor.simulation_running = true;
    for i in 0..8 {
        let angle = (i as f32 / 8.0) * TWO_PI;
        let pos = Vec3::new(angle.cos() * 8.0, 0.0, angle.sin() * 8.0);
        let mode = if i % 2 == 0 { AiAgentMode::BehaviorTree } else { AiAgentMode::UtilityAi };
        integrator.editor.spawn_agent(pos, mode);
    }
    if let Some(first) = integrator.editor.agents.first() {
        let leader_id = first.id;
        integrator.spawn_squad(leader_id, FormationType::Wedge);
    }
    integrator
}


// ============================================================
// ANIMATION BLEND TREE
// ============================================================

#[derive(Clone, Debug)]
pub struct AnimLayer {
    pub clip_name: String,
    pub weight: f32,
    pub time: f32,
    pub speed: f32,
    pub looping: bool,
    pub duration: f32,
    pub blend_in_time: f32,
}

impl AnimLayer {
    pub fn new(clip_name: &str, duration: f32, looping: bool) -> Self {
        Self { clip_name: clip_name.to_string(), weight: 0.0, time: 0.0, speed: 1.0, looping, duration, blend_in_time: 0.2 }
    }

    pub fn normalized_time(&self) -> f32 { if self.duration < EPSILON { 0.0 } else { self.time / self.duration } }
    pub fn is_finished(&self) -> bool { !self.looping && self.time >= self.duration }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt * self.speed;
        if self.looping && self.duration > EPSILON { self.time %= self.duration; }
    }
}

pub struct AnimBlendTree {
    pub layers: Vec<AnimLayer>,
    pub layer_weights: Vec<f32>,
    pub active_layer: usize,
    pub transition_time: f32,
}

impl AnimBlendTree {
    pub fn new() -> Self { Self { layers: Vec::new(), layer_weights: Vec::new(), active_layer: 0, transition_time: 0.0 } }

    pub fn add_layer(&mut self, layer: AnimLayer) { self.layer_weights.push(0.0); self.layers.push(layer); }

    pub fn play(&mut self, layer_idx: usize, blend_time: f32) {
        if layer_idx >= self.layers.len() { return; }
        self.active_layer = layer_idx;
        self.transition_time = blend_time;
        self.layers[layer_idx].time = 0.0;
    }

    pub fn tick(&mut self, dt: f32) {
        let n = self.layers.len();
        if n == 0 { return; }
        for i in 0..n {
            let target = if i == self.active_layer { 1.0 } else { 0.0 };
            let speed = if self.transition_time > EPSILON { dt / self.transition_time } else { 1.0 };
            self.layer_weights[i] += (target - self.layer_weights[i]) * speed.min(1.0);
        }
        let total: f32 = self.layer_weights.iter().sum();
        if total > EPSILON { for w in &mut self.layer_weights { *w /= total; } }
        for layer in &mut self.layers { layer.tick(dt); }
    }

    pub fn root_motion_velocity(&self, velocities: &[Vec3]) -> Vec3 {
        velocities.iter().enumerate().map(|(i, &v)| v * self.layer_weights.get(i).copied().unwrap_or(0.0)).fold(Vec3::ZERO, |a, b| a + b)
    }
}

// ============================================================
// DIALOG GRAPH
// ============================================================

#[derive(Clone, Debug)]
pub struct DialogOption {
    pub id: u32,
    pub text: String,
    pub next_node_id: Option<u32>,
    pub condition_key: Option<String>,
    pub condition_op: Option<CompareOp>,
    pub condition_value: Option<BlackboardValue>,
    pub effects: Vec<FsmAction>,
    pub ai_weight: f32,
}

#[derive(Clone, Debug)]
pub struct DialogNode {
    pub id: u32,
    pub speaker: String,
    pub text: String,
    pub options: Vec<DialogOption>,
    pub auto_advance: bool,
    pub advance_time: f32,
    pub entry_effects: Vec<FsmAction>,
}

pub struct DialogGraph {
    pub nodes: HashMap<u32, DialogNode>,
    pub start_node: Option<u32>,
    pub current_node: Option<u32>,
    pub next_id: u32,
    pub blackboard: Blackboard,
    pub history: Vec<u32>,
}

impl DialogGraph {
    pub fn new() -> Self {
        Self { nodes: HashMap::new(), start_node: None, current_node: None, next_id: 1, blackboard: Blackboard::new(), history: Vec::new() }
    }

    pub fn add_node(&mut self, speaker: &str, text: &str) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.nodes.insert(id, DialogNode { id, speaker: speaker.to_string(), text: text.to_string(), options: Vec::new(), auto_advance: false, advance_time: 3.0, entry_effects: Vec::new() });
        id
    }

    pub fn add_option(&mut self, node_id: u32, text: &str, next_node: Option<u32>, weight: f32) {
        let id = self.next_id; self.next_id += 1;
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.options.push(DialogOption { id, text: text.to_string(), next_node_id: next_node, condition_key: None, condition_op: None, condition_value: None, effects: Vec::new(), ai_weight: weight });
        }
    }

    pub fn start(&mut self) {
        if let Some(node_id) = self.start_node {
            self.current_node = Some(node_id);
            self.history.push(node_id);
            if let Some(node) = self.nodes.get(&node_id) {
                for effect in &node.entry_effects.clone() { effect.execute(&mut self.blackboard); }
            }
        }
    }

    fn option_available(&self, opt: &DialogOption) -> bool {
        if let (Some(key), Some(op), Some(val)) = (&opt.condition_key, &opt.condition_op, &opt.condition_value) {
            op.evaluate(self.blackboard.get(key), val)
        } else { true }
    }

    pub fn choose_option(&mut self, option_idx: usize) -> bool {
        let current = match self.current_node { Some(c) => c, None => return false };
        let (next_node, effects) = if let Some(node) = self.nodes.get(&current) {
            let available: Vec<&DialogOption> = node.options.iter().filter(|o| self.option_available(o)).collect();
            if option_idx >= available.len() { return false; }
            let opt = available[option_idx];
            (opt.next_node_id, opt.effects.clone())
        } else { return false; };

        for effect in &effects { effect.execute(&mut self.blackboard); }
        self.current_node = next_node;
        if let Some(nn) = next_node {
            self.history.push(nn);
            if let Some(node) = self.nodes.get(&nn) {
                for effect in &node.entry_effects.clone() { effect.execute(&mut self.blackboard); }
            }
        }
        true
    }

    pub fn ai_choose_response(&self) -> Option<usize> {
        let current = self.current_node?;
        let node = self.nodes.get(&current)?;
        let available: Vec<(usize, f32)> = node.options.iter().enumerate()
            .filter(|(_, o)| self.option_available(o))
            .map(|(i, o)| (i, o.ai_weight))
            .collect();
        available.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|&(i, _)| i)
    }

    pub fn current_text(&self) -> Option<(&str, &str)> {
        let node = self.nodes.get(&self.current_node?)?;
        Some((&node.speaker, &node.text))
    }

    pub fn available_options(&self) -> Vec<(usize, &str)> {
        let current = match self.current_node { Some(c) => c, None => return vec![] };
        if let Some(node) = self.nodes.get(&current) {
            node.options.iter().enumerate().filter(|(_, o)| self.option_available(o)).map(|(i, o)| (i, o.text.as_str())).collect()
        } else { vec![] }
    }
}

// ============================================================
// INTENT RECOGNIZER (companion AI)
// ============================================================

#[derive(Clone, Debug)]
pub enum PlayerIntent {
    Attack { target_pos: Vec3 },
    Defend { position: Vec3, radius: f32 },
    Follow { leader_id: u64 },
    Retreat { direction: Vec3 },
    UseAbility { ability_id: u32, target_pos: Vec3 },
    Idle,
}

pub struct IntentRecognizer {
    pub window: VecDeque<(f32, PlayerIntent)>,
    pub window_duration: f32,
    pub current_intent: PlayerIntent,
    pub confidence: f32,
}

impl IntentRecognizer {
    pub fn new(window_duration: f32) -> Self {
        Self { window: VecDeque::new(), window_duration, current_intent: PlayerIntent::Idle, confidence: 0.0 }
    }

    pub fn observe(&mut self, time: f32, intent: PlayerIntent) {
        self.window.push_back((time, intent));
        while self.window.front().map(|&(t, _)| time - t > self.window_duration).unwrap_or(false) { self.window.pop_front(); }
    }

    pub fn infer_intent(&mut self) -> &PlayerIntent {
        let mut attack_c = 0usize;
        let mut defend_c = 0usize;
        let mut follow_c = 0usize;
        let mut retreat_c = 0usize;
        let mut idle_c = 0usize;

        for (_, intent) in &self.window {
            match intent {
                PlayerIntent::Attack { .. } => attack_c += 1,
                PlayerIntent::Defend { .. } => defend_c += 1,
                PlayerIntent::Follow { .. } => follow_c += 1,
                PlayerIntent::Retreat { .. } => retreat_c += 1,
                PlayerIntent::Idle => idle_c += 1,
                _ => {}
            }
        }

        let total = self.window.len().max(1) as f32;
        let counts = [attack_c, defend_c, follow_c, retreat_c, idle_c];
        let best = counts.iter().enumerate().max_by_key(|(_, &c)| c).map(|(i, _)| i).unwrap_or(4);
        self.confidence = counts[best] as f32 / total;

        for (_, intent) in self.window.iter().rev() {
            let matched = match (best, intent) {
                (0, PlayerIntent::Attack { .. }) | (1, PlayerIntent::Defend { .. })
                | (2, PlayerIntent::Follow { .. }) | (3, PlayerIntent::Retreat { .. }) => true,
                _ => false,
            };
            if matched { self.current_intent = intent.clone(); return &self.current_intent; }
        }
        self.current_intent = PlayerIntent::Idle;
        &self.current_intent
    }
}

// ============================================================
// ADDITIONAL BT ANALYSIS
// ============================================================

pub struct BtAnalyzer;

impl BtAnalyzer {
    pub fn find_unreachable_nodes(tree: &BehaviorTree) -> Vec<u32> {
        let root = match tree.root_id { Some(r) => r, None => return tree.nodes.keys().copied().collect() };
        let mut reachable = HashSet::new();
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if reachable.contains(&id) { continue; }
            reachable.insert(id);
            if let Some(node) = tree.nodes.get(&id) {
                for &child in &node.children { stack.push(child); }
            }
        }
        tree.nodes.keys().filter(|&&id| !reachable.contains(&id)).copied().collect()
    }

    pub fn find_cycles(tree: &BehaviorTree) -> Vec<Vec<u32>> {
        // BTs should be DAGs; detect any cycles
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let mut path = Vec::new();
        if let Some(root) = tree.root_id {
            Self::dfs_cycle(tree, root, &mut visited, &mut stack, &mut path, &mut cycles);
        }
        cycles
    }

    fn dfs_cycle(tree: &BehaviorTree, node_id: u32, visited: &mut HashSet<u32>, stack: &mut HashSet<u32>, path: &mut Vec<u32>, cycles: &mut Vec<Vec<u32>>) {
        if stack.contains(&node_id) {
            if let Some(pos) = path.iter().position(|&x| x == node_id) {
                cycles.push(path[pos..].to_vec());
            }
            return;
        }
        if visited.contains(&node_id) { return; }
        visited.insert(node_id);
        stack.insert(node_id);
        path.push(node_id);
        if let Some(node) = tree.nodes.get(&node_id) {
            for &child in &node.children { Self::dfs_cycle(tree, child, visited, stack, path, cycles); }
        }
        stack.remove(&node_id);
        path.pop();
    }

    pub fn get_subtree_size(tree: &BehaviorTree, node_id: u32) -> usize {
        if let Some(node) = tree.nodes.get(&node_id) {
            1 + node.children.iter().map(|&c| Self::get_subtree_size(tree, c)).sum::<usize>()
        } else { 0 }
    }

    pub fn max_branching_factor(tree: &BehaviorTree) -> usize {
        tree.nodes.values().map(|n| n.children.len()).max().unwrap_or(0)
    }

    pub fn count_by_type(tree: &BehaviorTree) -> (usize, usize, usize) {
        let composites = tree.nodes.values().filter(|n| n.is_composite()).count();
        let decorators = tree.nodes.values().filter(|n| n.is_decorator()).count();
        let leaves = tree.nodes.values().filter(|n| n.is_leaf()).count();
        (composites, decorators, leaves)
    }

    pub fn validate(tree: &BehaviorTree) -> Vec<String> {
        let mut errors = Vec::new();
        // Check root exists
        if tree.root_id.is_none() { errors.push("No root node".to_string()); }
        // Check no orphans
        let unreachable = Self::find_unreachable_nodes(tree);
        if !unreachable.is_empty() { errors.push(format!("{} unreachable nodes: {:?}", unreachable.len(), unreachable)); }
        // Check decorators have exactly one child
        for node in tree.nodes.values() {
            if node.is_decorator() && node.children.len() > 1 {
                errors.push(format!("Decorator node {} [{}] has {} children (should have 1)", node.id, node.display_name(), node.children.len()));
            }
        }
        // Check cycles
        let cycles = Self::find_cycles(tree);
        for cycle in cycles { errors.push(format!("Cycle detected: {:?}", cycle)); }
        errors
    }
}

// ============================================================
// EDITOR UNDO/REDO SYSTEM (Extended)
// ============================================================

pub struct CommandHistory {
    pub undo_stack: VecDeque<EditorCommand>,
    pub redo_stack: VecDeque<EditorCommand>,
    pub max_history: usize,
}

#[derive(Clone, Debug)]
pub enum EditorCommand {
    AddBtNode { tree_idx: usize, node_id: u32, node_type: BtNodeType, position: Vec2 },
    RemoveBtNode { tree_idx: usize, node_id: u32 },
    MoveBtNode { tree_idx: usize, node_id: u32, from: Vec2, to: Vec2 },
    ConnectBtNodes { tree_idx: usize, parent_id: u32, child_id: u32 },
    DisconnectBtNodes { tree_idx: usize, parent_id: u32, child_id: u32 },
    AddFsmState { fsm_idx: usize, state_id: u32, name: String, pos: Vec2 },
    RemoveFsmState { fsm_idx: usize, state_id: u32 },
    AddFsmTransition { fsm_idx: usize, transition_id: u32, from: u32, to: u32 },
    RemoveFsmTransition { fsm_idx: usize, transition_id: u32 },
    SetBlackboardValue { key: String, old_value: BlackboardValue, new_value: BlackboardValue },
    AddGoapAction { action_id: u32 },
    RemoveGoapAction { action_id: u32 },
    ChangeFsmStateColor { fsm_idx: usize, state_id: u32, old_color: Vec4, new_color: Vec4 },
    Composite(Vec<EditorCommand>),
}

impl CommandHistory {
    pub fn new(max_history: usize) -> Self {
        Self { undo_stack: VecDeque::with_capacity(max_history), redo_stack: VecDeque::with_capacity(max_history), max_history }
    }

    pub fn push(&mut self, cmd: EditorCommand) {
        self.undo_stack.push_back(cmd);
        if self.undo_stack.len() > self.max_history { self.undo_stack.pop_front(); }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Option<EditorCommand> {
        let cmd = self.undo_stack.pop_back()?;
        self.redo_stack.push_back(cmd.clone());
        Some(cmd)
    }

    pub fn redo(&mut self) -> Option<EditorCommand> {
        let cmd = self.redo_stack.pop_back()?;
        self.undo_stack.push_back(cmd.clone());
        Some(cmd)
    }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    pub fn history_summary(&self) -> String {
        format!("Undo: {} commands  Redo: {} commands", self.undo_stack.len(), self.redo_stack.len())
    }
}

// ============================================================
// BEHAVIOR TREE COPY/PASTE BUFFER
// ============================================================

pub struct BtClipboard {
    pub copied_nodes: HashMap<u32, BtNode>,
    pub root_of_copy: Option<u32>,
    pub offset: Vec2,
}

impl BtClipboard {
    pub fn new() -> Self { Self { copied_nodes: HashMap::new(), root_of_copy: None, offset: Vec2::ZERO } }

    pub fn copy_subtree(&mut self, tree: &BehaviorTree, root_node_id: u32) {
        self.copied_nodes.clear();
        self.root_of_copy = None;
        let mut stack = vec![root_node_id];
        while let Some(id) = stack.pop() {
            if let Some(node) = tree.nodes.get(&id) {
                self.copied_nodes.insert(id, node.clone());
                for &child_id in &node.children { stack.push(child_id); }
            }
        }
        self.root_of_copy = Some(root_node_id);
        if let Some(root) = tree.nodes.get(&root_node_id) { self.offset = root.position; }
    }

    pub fn paste_into(&self, tree: &mut BehaviorTree, paste_pos: Vec2) -> Option<u32> {
        if self.copied_nodes.is_empty() { return None; }
        let old_root = self.root_of_copy?;
        let pos_delta = paste_pos - self.offset;

        // Remap old ids to new ids
        let mut id_map: HashMap<u32, u32> = HashMap::new();
        for &old_id in self.copied_nodes.keys() {
            let new_id = tree.next_id;
            tree.next_id += 1;
            id_map.insert(old_id, new_id);
        }

        // Insert nodes with new ids
        for (&old_id, old_node) in &self.copied_nodes {
            let new_id = id_map[&old_id];
            let mut new_node = old_node.clone();
            new_node.id = new_id;
            new_node.position = old_node.position + pos_delta;
            new_node.status = BtStatus::Invalid;
            new_node.elapsed_time = 0.0;
            new_node.repeat_count = 0;
            new_node.parent = old_node.parent.and_then(|p| id_map.get(&p).copied());
            new_node.children = old_node.children.iter().filter_map(|c| id_map.get(c).copied()).collect();
            tree.nodes.insert(new_id, new_node);
        }

        id_map.get(&old_root).copied()
    }

    pub fn is_empty(&self) -> bool { self.copied_nodes.is_empty() }
}

// ============================================================
// TERRAIN QUERY (for AI navigation decisions)
// ============================================================

pub struct TerrainQuery {
    pub height_map: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub origin: Vec2,
    pub slope_threshold: f32,
}

impl TerrainQuery {
    pub fn new(width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        Self { height_map: vec![0.0; width * height], width, height, cell_size, origin, slope_threshold: 0.5 }
    }

    fn sample(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { return 0.0; }
        self.height_map[y as usize * self.width + x as usize]
    }

    pub fn get_height(&self, pos: Vec2) -> f32 {
        let rel = pos - self.origin;
        let xi = (rel.x / self.cell_size) as i32;
        let yi = (rel.y / self.cell_size) as i32;
        let tx = (rel.x / self.cell_size) - xi as f32;
        let ty = (rel.y / self.cell_size) - yi as f32;
        let h00 = self.sample(xi, yi);
        let h10 = self.sample(xi + 1, yi);
        let h01 = self.sample(xi, yi + 1);
        let h11 = self.sample(xi + 1, yi + 1);
        h00 * (1.0 - tx) * (1.0 - ty) + h10 * tx * (1.0 - ty) + h01 * (1.0 - tx) * ty + h11 * tx * ty
    }

    pub fn get_normal(&self, pos: Vec2) -> Vec3 {
        let step = self.cell_size;
        let hx0 = self.get_height(pos - Vec2::new(step, 0.0));
        let hx1 = self.get_height(pos + Vec2::new(step, 0.0));
        let hy0 = self.get_height(pos - Vec2::new(0.0, step));
        let hy1 = self.get_height(pos + Vec2::new(0.0, step));
        let dx = (hx1 - hx0) / (2.0 * step);
        let dy = (hy1 - hy0) / (2.0 * step);
        Vec3::new(-dx, 1.0, -dy).normalize_or_zero()
    }

    pub fn is_traversable(&self, pos: Vec2) -> bool {
        let normal = self.get_normal(pos);
        normal.y >= (1.0 - self.slope_threshold * self.slope_threshold).sqrt()
    }

    pub fn get_slope_angle(&self, pos: Vec2) -> f32 {
        let normal = self.get_normal(pos);
        normal.y.clamp(-1.0, 1.0).acos()
    }

    pub fn find_high_ground_near(&self, center: Vec2, search_radius: f32) -> Option<Vec2> {
        let cells = (search_radius / self.cell_size) as i32;
        let center_cell_x = ((center.x - self.origin.x) / self.cell_size) as i32;
        let center_cell_y = ((center.y - self.origin.y) / self.cell_size) as i32;
        let mut best_h = f32::NEG_INFINITY;
        let mut best_pos = None;
        for dy in -cells..=cells {
            for dx in -cells..=cells {
                let cx = center_cell_x + dx;
                let cy = center_cell_y + dy;
                let h = self.sample(cx, cy);
                if h > best_h {
                    best_h = h;
                    let world_pos = Vec2::new(self.origin.x + cx as f32 * self.cell_size, self.origin.y + cy as f32 * self.cell_size);
                    if (world_pos - center).length() <= search_radius { best_pos = Some(world_pos); }
                }
            }
        }
        best_pos
    }
}

// ============================================================
// DYNAMIC DIFFICULTY ADJUSTMENT
// ============================================================

pub struct DdaSystem {
    pub player_skill_estimate: f32,  // 0..1
    pub kill_death_ratio: f32,
    pub time_to_die_avg: f32,
    pub time_to_kill_avg: f32,
    pub current_difficulty: f32,    // 0..1
    pub target_difficulty: f32,
    pub adjustment_rate: f32,
    pub history_window: VecDeque<DdaEvent>,
    pub window_size: usize,
}

#[derive(Clone, Debug)]
pub enum DdaEvent {
    PlayerKilled { time: f32 },
    EnemyKilled { time: f32, time_to_kill: f32 },
    PlayerDamaged { amount: f32, time: f32 },
    PlayerHealed { amount: f32, time: f32 },
    ObjectiveCompleted { time: f32 },
    ObjectiveFailed { time: f32 },
}

impl DdaSystem {
    pub fn new() -> Self {
        Self {
            player_skill_estimate: 0.5,
            kill_death_ratio: 1.0,
            time_to_die_avg: 30.0,
            time_to_kill_avg: 5.0,
            current_difficulty: 0.5,
            target_difficulty: 0.5,
            adjustment_rate: 0.05,
            history_window: VecDeque::with_capacity(50),
            window_size: 50,
        }
    }

    pub fn record_event(&mut self, event: DdaEvent) {
        self.history_window.push_back(event);
        if self.history_window.len() > self.window_size { self.history_window.pop_front(); }
        self.recompute_skill();
    }

    fn recompute_skill(&mut self) {
        let kills: Vec<f32> = self.history_window.iter().filter_map(|e| if let DdaEvent::EnemyKilled { time_to_kill, .. } = e { Some(*time_to_kill) } else { None }).collect();
        let deaths = self.history_window.iter().filter(|e| matches!(e, DdaEvent::PlayerKilled { .. })).count() as f32;
        let n_kills = kills.len() as f32;

        if n_kills > 0.0 {
            let avg_ttk = kills.iter().sum::<f32>() / n_kills;
            self.time_to_kill_avg = avg_ttk;
            let kdr = n_kills / (deaths + 1.0);
            self.kill_death_ratio = kdr;
            // Skill: fast kills + high KDR = high skill
            let ttk_score = (1.0 - (avg_ttk / 30.0).min(1.0));
            let kdr_score = (kdr / (kdr + 1.0)).min(1.0);
            self.player_skill_estimate = (ttk_score * 0.4 + kdr_score * 0.6).clamp(0.0, 1.0);
        }

        // Adjust target difficulty toward challenging but not frustrating
        // Target: player wins ~60% of encounters
        let target = 0.4 + self.player_skill_estimate * 0.4;
        self.target_difficulty = target.clamp(0.1, 0.9);
    }

    pub fn update(&mut self, dt: f32) {
        // Smoothly approach target difficulty
        let diff = self.target_difficulty - self.current_difficulty;
        self.current_difficulty += diff * self.adjustment_rate * dt;
        self.current_difficulty = self.current_difficulty.clamp(0.0, 1.0);
    }

    pub fn get_enemy_health_multiplier(&self) -> f32 { 0.5 + self.current_difficulty * 1.0 }
    pub fn get_enemy_damage_multiplier(&self) -> f32 { 0.6 + self.current_difficulty * 0.8 }
    pub fn get_enemy_accuracy(&self) -> f32 { 0.3 + self.current_difficulty * 0.5 }
    pub fn get_enemy_reaction_time(&self) -> f32 { 0.8 - self.current_difficulty * 0.5 }
    pub fn get_enemy_aggression(&self) -> f32 { 0.2 + self.current_difficulty * 0.6 }

    pub fn apply_to_blackboard(&self, bb: &mut Blackboard) {
        bb.set("dda_difficulty", BlackboardValue::Float(self.current_difficulty));
        bb.set("dda_health_mult", BlackboardValue::Float(self.get_enemy_health_multiplier()));
        bb.set("dda_damage_mult", BlackboardValue::Float(self.get_enemy_damage_multiplier()));
        bb.set("dda_accuracy", BlackboardValue::Float(self.get_enemy_accuracy()));
        bb.set("dda_reaction_time", BlackboardValue::Float(self.get_enemy_reaction_time()));
        bb.set("dda_aggression", BlackboardValue::Float(self.get_enemy_aggression()));
    }
}

// ============================================================
// ADDITIONAL RESPONSE CURVE TESTS
// ============================================================

pub struct ResponseCurveTests;

impl ResponseCurveTests {
    pub fn test_all() -> Vec<(String, bool)> {
        let mut results = Vec::new();
        let curves = [
            ("linear", ResponseCurve::Linear { slope: 1.0, intercept: 0.0 }),
            ("exponential", ResponseCurve::Exponential { base: 2.0, exponent: 1.0, scale: 0.5 }),
            ("logistic", ResponseCurve::Logistic { steepness: 5.0, midpoint: 0.5 }),
            ("sine", ResponseCurve::Sine { frequency: 1.0, phase: 0.0, amplitude: 0.5, offset: 0.5 }),
            ("polynomial", ResponseCurve::Polynomial { coefficients: vec![0.0, 0.0, 1.0] }),
            ("inverse", ResponseCurve::Inverse { scale: 0.5 }),
            ("step", ResponseCurve::Step { threshold: 0.5, low: 0.0, high: 1.0 }),
            ("smoothstep", ResponseCurve::Smoothstep { edge0: 0.2, edge1: 0.8 }),
            ("bell", ResponseCurve::Bell { center: 0.5, width: 0.3 }),
            ("constant", ResponseCurve::Constant { value: 0.7 }),
        ];
        for (name, curve) in &curves {
            // Verify output in [0,1] for inputs 0..1
            let valid = (0..=10).map(|i| i as f32 / 10.0).all(|x| {
                let v = curve.evaluate(x);
                v >= 0.0 && v <= 1.0
            });
            results.push((name.to_string(), valid));
        }
        results
    }
}

// ============================================================
// GOAP VALIDATOR
// ============================================================

pub struct GoapValidator;

impl GoapValidator {
    /// Checks that every action's effects can satisfy at least one other action's preconditions or goal
    pub fn validate_action_chain(planner: &GoapPlanner, goal: WorldState) -> Vec<String> {
        let mut warnings = Vec::new();
        for action in &planner.actions {
            let effective_state = action.effects_set;
            if effective_state == 0 { warnings.push(format!("Action '{}' has no effects", action.name)); continue; }
            // Check if any effect bit satisfies goal or another action's precondition
            let satisfies_goal = (effective_state & goal) != 0;
            let satisfies_precond = planner.actions.iter().any(|other| {
                other.id != action.id && (effective_state & other.preconditions) != 0
            });
            if !satisfies_goal && !satisfies_precond {
                warnings.push(format!("Action '{}' effects don't satisfy any goal or precondition", action.name));
            }
        }
        warnings
    }

    pub fn check_dead_ends(planner: &GoapPlanner, start: WorldState, goal: WorldState) -> Vec<WorldState> {
        // Find world states reachable from start but from which goal is not reachable
        let mut dead_ends = Vec::new();
        let mut to_check = vec![start];
        let mut seen = HashSet::new();
        seen.insert(start);

        while let Some(state) = to_check.pop() {
            if (state & goal) == goal { continue; }
            let applicable: Vec<&GoapAction> = planner.actions.iter().filter(|a| a.can_execute(state, 0.0)).collect();
            if applicable.is_empty() {
                dead_ends.push(state);
            } else {
                for action in applicable {
                    let new_state = action.apply(state);
                    if !seen.contains(&new_state) {
                        seen.insert(new_state);
                        to_check.push(new_state);
                    }
                }
            }
        }
        dead_ends
    }
}

// ============================================================
// FINAL: COMPLETE INTEGRATION CREATION
// ============================================================

pub fn create_full_ai_system() -> AiSystemIntegrator {
    global_editor_init()
}

pub fn validate_editor(editor: &AiBehaviorEditor) -> Vec<String> {
    let mut issues = Vec::new();
    for (i, tree) in editor.behavior_trees.iter().enumerate() {
        let tree_issues = BtAnalyzer::validate(tree);
        for issue in tree_issues {
            issues.push(format!("Tree[{}] '{}': {}", i, tree.name, issue));
        }
    }
    if editor.goap_planner.actions.is_empty() {
        issues.push("GOAP planner has no actions".to_string());
    }
    let goap_warnings = GoapValidator::validate_action_chain(&editor.goap_planner, editor.goap_goal_state);
    issues.extend(goap_warnings);
    issues
}

pub fn run_all_validations() -> (usize, usize) {
    let test_results = AiEditorTests::run_all();
    let curve_tests = ResponseCurveTests::test_all();
    let passed = test_results.iter().filter(|(_, ok)| *ok).count() + curve_tests.iter().filter(|(_, ok)| *ok).count();
    let total = test_results.len() + curve_tests.len();
    (passed, total)
}

// ============================================================
// INFLUENCE MAP SYSTEM
// ============================================================

pub struct InfluenceMap {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub origin: Vec2,
    pub friendly_influence: Vec<f32>,
    pub enemy_influence: Vec<f32>,
    pub danger_map: Vec<f32>,
    pub opportunity_map: Vec<f32>,
    pub decay: f32,
    pub propagation_iterations: usize,
}

impl InfluenceMap {
    pub fn new(width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        let n = width * height;
        Self {
            width, height, cell_size, origin,
            friendly_influence: vec![0.0; n],
            enemy_influence: vec![0.0; n],
            danger_map: vec![0.0; n],
            opportunity_map: vec![0.0; n],
            decay: 0.9,
            propagation_iterations: 3,
        }
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { return None; }
        Some(y as usize * self.width + x as usize)
    }

    pub fn cell_of(&self, pos: Vec2) -> (i32, i32) {
        let rel = pos - self.origin;
        ((rel.x / self.cell_size).floor() as i32, (rel.y / self.cell_size).floor() as i32)
    }

    pub fn stamp_influence(&mut self, pos: Vec2, value: f32, radius: f32, friendly: bool) {
        let (cx, cy) = self.cell_of(pos);
        let cell_radius = (radius / self.cell_size) as i32 + 1;
        let width = self.width;
        let height = self.height;
        let cell_size = self.cell_size;
        let map = if friendly { &mut self.friendly_influence } else { &mut self.enemy_influence };
        for dy in -cell_radius..=cell_radius {
            for dx in -cell_radius..=cell_radius {
                let nx = cx + dx;
                let ny = cy + dy;
                let idx_opt = if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                    None
                } else {
                    Some(ny as usize * width + nx as usize)
                };
                if let Some(idx) = idx_opt {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt() * cell_size;
                    if dist <= radius {
                        let falloff = 1.0 - (dist / radius);
                        map[idx] = (map[idx] + value * falloff).clamp(-1.0, 1.0);
                    }
                }
            }
        }
    }

    pub fn propagate(&mut self) {
        let w = self.width;
        let h = self.height;
        for _ in 0..self.propagation_iterations {
            let mut new_friendly = self.friendly_influence.clone();
            let mut new_enemy = self.enemy_influence.clone();
            for y in 0..(h as i32) {
                for x in 0..(w as i32) {
                    if let Some(idx) = self.idx(x, y) {
                        let neighbors = [(x-1, y), (x+1, y), (x, y-1), (x, y+1)];
                        let mut sum_f = 0.0f32;
                        let mut sum_e = 0.0f32;
                        let mut count = 0;
                        for &(nx, ny) in &neighbors {
                            if let Some(ni) = self.idx(nx, ny) {
                                sum_f += self.friendly_influence[ni];
                                sum_e += self.enemy_influence[ni];
                                count += 1;
                            }
                        }
                        if count > 0 {
                            let avg_f = sum_f / count as f32;
                            let avg_e = sum_e / count as f32;
                            new_friendly[idx] = (new_friendly[idx] + avg_f * self.decay * 0.25).clamp(-1.0, 1.0);
                            new_enemy[idx] = (new_enemy[idx] + avg_e * self.decay * 0.25).clamp(-1.0, 1.0);
                        }
                    }
                }
            }
            self.friendly_influence = new_friendly;
            self.enemy_influence = new_enemy;
        }
        // Compute derived maps
        for i in 0..(self.width * self.height) {
            self.danger_map[i] = (self.enemy_influence[i] - self.friendly_influence[i]).max(0.0);
            self.opportunity_map[i] = (self.friendly_influence[i] - self.enemy_influence[i]).max(0.0);
        }
    }

    pub fn decay_all(&mut self, dt: f32) {
        let decay = (1.0 - dt * 0.5).max(0.0);
        for v in &mut self.friendly_influence { *v *= decay; }
        for v in &mut self.enemy_influence { *v *= decay; }
    }

    pub fn get_tension(&self, pos: Vec2) -> f32 {
        let (cx, cy) = self.cell_of(pos);
        if let Some(idx) = self.idx(cx, cy) {
            (self.friendly_influence[idx] + self.enemy_influence[idx]).abs()
        } else { 0.0 }
    }

    pub fn get_vulnerability(&self, pos: Vec2) -> f32 {
        let (cx, cy) = self.cell_of(pos);
        if let Some(idx) = self.idx(cx, cy) { self.danger_map[idx] } else { 0.0 }
    }

    pub fn find_safest_direction(&self, pos: Vec2) -> Vec2 {
        let (cx, cy) = self.cell_of(pos);
        let directions = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)];
        let mut safest_dir = Vec2::ZERO;
        let mut min_danger = f32::MAX;
        for &(dx, dy) in &directions {
            if let Some(idx) = self.idx(cx + dx, cy + dy) {
                let danger = self.danger_map[idx];
                if danger < min_danger {
                    min_danger = danger;
                    safest_dir = Vec2::new(dx as f32, dy as f32).normalize_or_zero();
                }
            }
        }
        safest_dir
    }

    pub fn find_most_opportune_position(&self, center: Vec2, search_radius: f32) -> Option<Vec2> {
        let (cx, cy) = self.cell_of(center);
        let cell_r = (search_radius / self.cell_size) as i32;
        let mut best = f32::NEG_INFINITY;
        let mut best_pos = None;
        for dy in -cell_r..=cell_r {
            for dx in -cell_r..=cell_r {
                if let Some(idx) = self.idx(cx + dx, cy + dy) {
                    let opp = self.opportunity_map[idx];
                    if opp > best {
                        best = opp;
                        best_pos = Some(Vec2::new(
                            self.origin.x + (cx + dx) as f32 * self.cell_size,
                            self.origin.y + (cy + dy) as f32 * self.cell_size,
                        ));
                    }
                }
            }
        }
        best_pos
    }
}

// ============================================================
// ADVANCED STEERING — CONTEXT STEERING
// ============================================================

pub struct ContextSteering {
    pub resolution: usize,          // Number of directions to sample (e.g., 8 or 16)
    pub interest: Vec<f32>,          // How much we want to move in each direction
    pub danger: Vec<f32>,            // Obstacles/dangers in each direction
    pub result_dir: Vec2,
    pub result_speed: f32,
}

impl ContextSteering {
    pub fn new(resolution: usize) -> Self {
        Self {
            resolution,
            interest: vec![0.0; resolution],
            danger: vec![0.0; resolution],
            result_dir: Vec2::ZERO,
            result_speed: 0.0,
        }
    }

    pub fn direction_for_slot(&self, slot: usize) -> Vec2 {
        let angle = (slot as f32 / self.resolution as f32) * TWO_PI;
        Vec2::new(angle.cos(), angle.sin())
    }

    pub fn add_interest(&mut self, desired_direction: Vec2, weight: f32) {
        let desired_norm = desired_direction.normalize_or_zero();
        for i in 0..self.resolution {
            let slot_dir = self.direction_for_slot(i);
            let dot = slot_dir.dot(desired_norm).max(0.0);
            self.interest[i] += dot * weight;
        }
    }

    pub fn add_danger(&mut self, danger_direction: Vec2, weight: f32) {
        let danger_norm = danger_direction.normalize_or_zero();
        for i in 0..self.resolution {
            let slot_dir = self.direction_for_slot(i);
            let dot = slot_dir.dot(danger_norm).max(0.0);
            self.danger[i] = (self.danger[i] + dot * weight).min(1.0);
        }
    }

    pub fn solve(&mut self) -> Vec2 {
        // Mask interest with danger
        let masked: Vec<f32> = self.interest.iter().zip(self.danger.iter())
            .map(|(&i, &d)| if d > 0.7 { 0.0 } else { i * (1.0 - d) })
            .collect();

        // Find best slot
        let best_slot = masked.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let best_weight = masked[best_slot];
        if best_weight < EPSILON {
            self.result_dir = Vec2::ZERO;
            self.result_speed = 0.0;
            return Vec2::ZERO;
        }

        // Weighted average of top directions
        let mut dir_sum = Vec2::ZERO;
        let mut weight_sum = 0.0f32;
        for i in 0..self.resolution {
            if masked[i] > best_weight * 0.5 {
                dir_sum += self.direction_for_slot(i) * masked[i];
                weight_sum += masked[i];
            }
        }

        self.result_dir = if weight_sum > EPSILON { (dir_sum / weight_sum).normalize_or_zero() } else { Vec2::ZERO };
        self.result_speed = best_weight.min(1.0);
        self.result_dir
    }

    pub fn reset(&mut self) {
        for v in &mut self.interest { *v = 0.0; }
        for v in &mut self.danger { *v = 0.0; }
    }

    pub fn debug_draw(&self, center: Vec3, scale: f32, buf: &mut DebugVisualizationBuffer) {
        for i in 0..self.resolution {
            let dir_2d = self.direction_for_slot(i);
            let dir_3d = Vec3::new(dir_2d.x, 0.0, dir_2d.y);
            let interest_color = Vec4::new(0.0, self.interest[i], 0.0, 0.8);
            let danger_color = Vec4::new(self.danger[i], 0.0, 0.0, 0.8);
            buf.add(DebugShapeType::Arrow { from: center, to: center + dir_3d * self.interest[i] * scale, head_size: 0.1 }, interest_color, 0.0);
            buf.add(DebugShapeType::Arrow { from: center, to: center + dir_3d * self.danger[i] * scale * 0.5, head_size: 0.08 }, danger_color, 0.0);
        }
        // Result
        let result_3d = Vec3::new(self.result_dir.x, 0.0, self.result_dir.y);
        buf.add(DebugShapeType::Arrow { from: center, to: center + result_3d * self.result_speed * scale * 1.2, head_size: 0.15 }, Vec4::new(1.0, 1.0, 0.0, 1.0), 0.0);
    }
}

// ============================================================
// ABILITY SYSTEM (for AI skill activation)
// ============================================================

#[derive(Clone, Debug)]
pub struct AiAbility {
    pub id: u32,
    pub name: String,
    pub cooldown: f32,
    pub cooldown_remaining: f32,
    pub cast_time: f32,
    pub range: f32,
    pub area_radius: f32,
    pub damage: f32,
    pub healing: f32,
    pub energy_cost: f32,
    pub is_casting: bool,
    pub cast_elapsed: f32,
    pub target_pos: Vec3,
    pub target_entity: Option<u64>,
    pub tags: HashSet<String>,
}

impl AiAbility {
    pub fn new(id: u32, name: &str, cooldown: f32, range: f32) -> Self {
        Self {
            id, name: name.to_string(), cooldown, cooldown_remaining: 0.0,
            cast_time: 0.5, range, area_radius: 0.0, damage: 0.0, healing: 0.0,
            energy_cost: 10.0, is_casting: false, cast_elapsed: 0.0,
            target_pos: Vec3::ZERO, target_entity: None, tags: HashSet::new(),
        }
    }

    pub fn is_ready(&self) -> bool { self.cooldown_remaining <= 0.0 && !self.is_casting }

    pub fn can_reach(&self, user_pos: Vec3, target_pos: Vec3) -> bool {
        (target_pos - user_pos).length() <= self.range
    }

    pub fn start_cast(&mut self, target_pos: Vec3, target_entity: Option<u64>) {
        if !self.is_ready() { return; }
        self.is_casting = true;
        self.cast_elapsed = 0.0;
        self.target_pos = target_pos;
        self.target_entity = target_entity;
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        // Returns true when ability fires
        self.cooldown_remaining = (self.cooldown_remaining - dt).max(0.0);
        if self.is_casting {
            self.cast_elapsed += dt;
            if self.cast_elapsed >= self.cast_time {
                self.is_casting = false;
                self.cooldown_remaining = self.cooldown;
                return true;
            }
        }
        false
    }

    pub fn interrupt(&mut self) {
        self.is_casting = false;
        self.cast_elapsed = 0.0;
    }

    pub fn cast_progress(&self) -> f32 {
        if self.cast_time < EPSILON { 1.0 } else { self.cast_elapsed / self.cast_time }
    }
}

pub struct AbilityManager {
    pub abilities: Vec<AiAbility>,
    pub energy: f32,
    pub max_energy: f32,
    pub energy_regen: f32,
}

impl AbilityManager {
    pub fn new(max_energy: f32) -> Self {
        Self { abilities: Vec::new(), energy: max_energy, max_energy, energy_regen: 5.0 }
    }

    pub fn add_ability(&mut self, ability: AiAbility) { self.abilities.push(ability); }

    pub fn tick(&mut self, dt: f32) -> Vec<u32> {
        // Energy regen
        self.energy = (self.energy + self.energy_regen * dt).min(self.max_energy);
        // Tick each ability, collect fired IDs
        self.abilities.iter_mut().filter_map(|a| if a.tick(dt) { Some(a.id) } else { None }).collect()
    }

    pub fn try_use(&mut self, ability_id: u32, target_pos: Vec3, user_pos: Vec3) -> bool {
        if let Some(ability) = self.abilities.iter_mut().find(|a| a.id == ability_id) {
            if ability.is_ready() && ability.can_reach(user_pos, target_pos) && self.energy >= ability.energy_cost {
                self.energy -= ability.energy_cost;
                ability.start_cast(target_pos, None);
                return true;
            }
        }
        false
    }

    pub fn best_offensive_ability(&self, user_pos: Vec3, target_pos: Vec3) -> Option<u32> {
        self.abilities.iter()
            .filter(|a| a.is_ready() && a.damage > 0.0 && a.can_reach(user_pos, target_pos))
            .max_by(|a, b| a.damage.partial_cmp(&b.damage).unwrap())
            .map(|a| a.id)
    }

    pub fn best_healing_ability(&self) -> Option<u32> {
        self.abilities.iter()
            .filter(|a| a.is_ready() && a.healing > 0.0 && self.energy >= a.energy_cost)
            .max_by(|a, b| a.healing.partial_cmp(&b.healing).unwrap())
            .map(|a| a.id)
    }

    pub fn interrupt_all(&mut self) {
        for ability in &mut self.abilities { ability.interrupt(); }
    }
}

// ============================================================
// FINAL AI EDITOR EXTENSIONS
// ============================================================

impl AiBehaviorEditor {
    pub fn add_influence_map_panel(&mut self) {
        // Register panel
        self.panel_sizes.insert("influence_map".to_string(), Vec2::new(300.0, 300.0));
    }

    pub fn get_bt_node_tooltip(&self, tree_idx: usize, node_id: u32) -> String {
        let tree = match self.behavior_trees.get(tree_idx) { Some(t) => t, None => return String::new() };
        let node = match tree.nodes.get(&node_id) { Some(n) => n, None => return String::new() };
        let mut tip = format!("[{}] {}\n", node.id, node.display_name());
        tip.push_str(&format!("  Status: {:?}\n", node.status));
        tip.push_str(&format!("  Children: {}\n", node.children.len()));
        tip.push_str(&format!("  Elapsed: {:.2}s\n", node.elapsed_time));
        if node.repeat_count > 0 { tip.push_str(&format!("  Repeat count: {}\n", node.repeat_count)); }
        tip
    }

    pub fn center_camera_on_tree(&mut self, tree_idx: usize) {
        if let Some(tree) = self.behavior_trees.get(tree_idx) {
            if tree.nodes.is_empty() { return; }
            let mut min_x = f32::MAX; let mut max_x = f32::MIN;
            let mut min_y = f32::MAX; let mut max_y = f32::MIN;
            for node in tree.nodes.values() {
                min_x = min_x.min(node.position.x);
                max_x = max_x.max(node.position.x + node.size.x);
                min_y = min_y.min(node.position.y);
                max_y = max_y.max(node.position.y + node.size.y);
            }
            let center = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
            self.bt_camera.target_pan = -center;
        }
    }

    pub fn align_nodes_horizontal(&mut self, tree_idx: usize) {
        let selected: Vec<u32> = self.bt_selection.selected_nodes.iter().copied().collect();
        if selected.is_empty() { return; }
        let tree = match self.behavior_trees.get(tree_idx) { Some(t) => t, None => return };
        let avg_y: f32 = selected.iter().filter_map(|id| tree.nodes.get(id)).map(|n| n.position.y).sum::<f32>() / selected.len() as f32;
        let tree = match self.behavior_trees.get_mut(tree_idx) { Some(t) => t, None => return };
        for id in &selected {
            if let Some(node) = tree.nodes.get_mut(id) { node.position.y = avg_y; }
        }
    }

    pub fn align_nodes_vertical(&mut self, tree_idx: usize) {
        let selected: Vec<u32> = self.bt_selection.selected_nodes.iter().copied().collect();
        if selected.is_empty() { return; }
        let tree = match self.behavior_trees.get(tree_idx) { Some(t) => t, None => return };
        let avg_x: f32 = selected.iter().filter_map(|id| tree.nodes.get(id)).map(|n| n.position.x).sum::<f32>() / selected.len() as f32;
        let tree = match self.behavior_trees.get_mut(tree_idx) { Some(t) => t, None => return };
        for id in &selected {
            if let Some(node) = tree.nodes.get_mut(id) { node.position.x = avg_x; }
        }
    }

    pub fn distribute_nodes_horizontally(&mut self, tree_idx: usize) {
        let mut selected: Vec<u32> = self.bt_selection.selected_nodes.iter().copied().collect();
        if selected.len() < 2 { return; }
        let tree = match self.behavior_trees.get(tree_idx) { Some(t) => t, None => return };
        selected.sort_by(|&a, &b| {
            let xa = tree.nodes.get(&a).map(|n| n.position.x).unwrap_or(0.0);
            let xb = tree.nodes.get(&b).map(|n| n.position.x).unwrap_or(0.0);
            xa.partial_cmp(&xb).unwrap()
        });
        let first_x = tree.nodes.get(&selected[0]).map(|n| n.position.x).unwrap_or(0.0);
        let last_x = tree.nodes.get(selected.last().unwrap()).map(|n| n.position.x + n.size.x).unwrap_or(0.0);
        let total_width: f32 = selected.iter().filter_map(|id| tree.nodes.get(id)).map(|n| n.size.x).sum();
        let gap = (last_x - first_x - total_width) / (selected.len() as f32 - 1.0).max(1.0);
        let tree = match self.behavior_trees.get_mut(tree_idx) { Some(t) => t, None => return };
        let mut cursor = first_x;
        for id in &selected {
            if let Some(node) = tree.nodes.get_mut(id) {
                node.position.x = cursor;
                cursor += node.size.x + gap;
            }
        }
    }

    pub fn set_node_color_by_status(&self) -> HashMap<u32, Vec4> {
        let tree = match self.behavior_trees.get(self.active_tree_index) { Some(t) => t, None => return HashMap::new() };
        tree.nodes.iter().map(|(&id, node)| (id, self.bt_get_node_color(node))).collect()
    }

    pub fn export_tree_as_dot(&self, tree_idx: usize) -> String {
        let tree = match self.behavior_trees.get(tree_idx) { Some(t) => t, None => return String::new() };
        let mut out = String::from("digraph BehaviorTree {\n  rankdir=TB;\n");
        for (id, node) in &tree.nodes {
            let color = match node.status {
                BtStatus::Success => "green",
                BtStatus::Failure => "red",
                BtStatus::Running => "yellow",
                BtStatus::Invalid => "gray",
            };
            let shape = if node.is_composite() { "diamond" } else if node.is_decorator() { "hexagon" } else { "box" };
            out.push_str(&format!("  {} [label=\"{}\" style=filled fillcolor={} shape={}];\n", id, node.display_name(), color, shape));
        }
        for (parent_id, node) in &tree.nodes {
            for child_id in &node.children {
                out.push_str(&format!("  {} -> {};\n", parent_id, child_id));
            }
        }
        out.push_str("}\n");
        out
    }

    pub fn compute_heatmap_positions(&self, tree_idx: usize, debugger: &BtDebugger) -> Vec<(Vec2, f32)> {
        let tree = match self.behavior_trees.get(tree_idx) { Some(t) => t, None => return Vec::new() };
        let max_count = debugger.node_exec_counts.values().copied().max().unwrap_or(1) as f32;
        tree.nodes.iter().map(|(&id, node)| {
            let count = debugger.node_exec_counts.get(&id).copied().unwrap_or(0) as f32;
            let heat = count / max_count;
            (node.position + node.size * 0.5, heat)
        }).collect()
    }

    pub fn fsm_get_transition_arrow(&self, fsm_idx: usize, transition_id: u32) -> Option<(Vec2, Vec2)> {
        let fsm = self.fsm_instances.get(fsm_idx)?;
        let t = fsm.transitions.iter().find(|t| t.id == transition_id)?;
        let from_pos = fsm.states.get(&t.from_state)?.position;
        let to_pos = fsm.states.get(&t.to_state)?.position;
        // Offset for self-loops
        if t.from_state == t.to_state {
            let offset = Vec2::new(60.0, -40.0);
            Some((from_pos + offset, from_pos + offset * 2.0))
        } else {
            Some((from_pos, to_pos))
        }
    }

    pub fn blackboard_diff(&self, other: &Blackboard) -> Vec<(String, BlackboardValue, BlackboardValue)> {
        let mut diffs = Vec::new();
        for (key, value) in &self.shared_blackboard.entries {
            if let Some(other_val) = other.entries.get(key) {
                if other_val != value {
                    diffs.push((key.clone(), value.clone(), other_val.clone()));
                }
            } else {
                diffs.push((key.clone(), value.clone(), BlackboardValue::None));
            }
        }
        diffs
    }

    pub fn get_formation_agent_positions(&self, leader_pos: Vec3, leader_fwd: Vec3) -> Vec<Vec3> {
        FormationLayout::compute_slots(self.formation_preview, leader_pos, leader_fwd, self.formation_n_agents, self.formation_spacing)
    }

    pub fn compute_all_utility_scores(&self) -> Vec<(String, f32)> {
        self.utility_dm.actions.iter().map(|a| {
            (a.name.clone(), a.score(&self.shared_blackboard, self.current_time))
        }).collect()
    }

    pub fn tick_all_emotion_engines(&mut self, dt: f32) {
        for engine in &mut self.emotion_engines {
            engine.update(dt, self.current_time);
        }
    }

    pub fn get_global_threat_level(&self) -> f32 {
        self.agents.iter().map(|a| {
            a.perception.perceived.values().map(|p| p.threat_level * p.confidence).sum::<f32>()
        }).sum::<f32>() / self.agents.len().max(1) as f32
    }

    pub fn snapshot_agent_states(&self) -> Vec<HashMap<String, f32>> {
        self.agents.iter().map(|agent| {
            let mut snap = HashMap::new();
            snap.insert("health".to_string(), agent.blackboard.get_float("self_health"));
            snap.insert("ammo".to_string(), agent.blackboard.get_float("ammo_count"));
            snap.insert("pos_x".to_string(), agent.position.x);
            snap.insert("pos_y".to_string(), agent.position.y);
            snap.insert("pos_z".to_string(), agent.position.z);
            snap.insert("speed".to_string(), agent.steering_agent.speed());
            snap.insert("emotion_valence".to_string(), agent.emotion_engine.state.mood_valence);
            snap.insert("emotion_arousal".to_string(), agent.emotion_engine.state.mood_arousal);
            snap
        }).collect()
    }
}

// ============================================================
// COMBAT SIMULATION HELPERS
// ============================================================

pub fn simulate_combat_round(
    attacker_pos: Vec3, attacker_damage: f32, attacker_accuracy: f32,
    defender_pos: Vec3, defender_health: f32, defender_cover: f32,
    rng: &mut u64,
) -> (f32, bool) {
    // RNG roll
    *rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let roll = ((*rng >> 33) as f32) / (u32::MAX as f32);

    let hit_chance = (attacker_accuracy * (1.0 - defender_cover * 0.5)).clamp(0.0, 1.0);
    let dist = (defender_pos - attacker_pos).length();
    let range_penalty = effective_range_modifier(dist, 20.0, 5.0);
    let effective_hit_chance = hit_chance * range_penalty;

    if roll < effective_hit_chance {
        let damage = attacker_damage * (0.8 + roll * 0.4); // slight variance
        let new_health = (defender_health - damage).max(0.0);
        let killed = new_health <= 0.0;
        (new_health, killed)
    } else {
        (defender_health, false)
    }
}

pub fn estimate_time_to_kill(attacker_damage: f32, attacker_fire_rate: f32, attacker_accuracy: f32, defender_health: f32, defender_cover: f32) -> f32 {
    if attacker_fire_rate <= 0.0 || attacker_damage <= 0.0 { return f32::MAX; }
    let shots_needed = (defender_health / attacker_damage).ceil();
    let effective_accuracy = attacker_accuracy * (1.0 - defender_cover * 0.3);
    let shots_to_fire = shots_needed / effective_accuracy.max(0.01);
    shots_to_fire / attacker_fire_rate
}

pub fn check_line_of_sight_multi(from: Vec3, to: Vec3, obstacles: &[Aabb]) -> (bool, Option<Vec3>) {
    let dir = to - from;
    let len = dir.length();
    if len < EPSILON { return (true, None); }
    let inv_dir = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

    let mut nearest_hit: Option<Vec3> = None;
    let mut nearest_t = f32::MAX;

    for obs in obstacles {
        let t1 = (obs.min - from) * inv_dir;
        let t2 = (obs.max - from) * inv_dir;
        let t_enter = Vec3::new(t1.x.min(t2.x), t1.y.min(t2.y), t1.z.min(t2.z));
        let t_exit = Vec3::new(t1.x.max(t2.x), t1.y.max(t2.y), t1.z.max(t2.z));
        let t_in = t_enter.x.max(t_enter.y).max(t_enter.z);
        let t_out = t_exit.x.min(t_exit.y).min(t_exit.z);
        if t_in <= t_out && t_out >= 0.0 && t_in <= len {
            let t = t_in.max(0.0);
            if t < nearest_t {
                nearest_t = t;
                nearest_hit = Some(from + dir.normalize() * t);
            }
        }
    }

    if nearest_hit.is_some() { (false, nearest_hit) } else { (true, None) }
}

// ============================================================
// EDITOR QUICK-START PRESETS
// ============================================================

pub struct AiPresets;

impl AiPresets {
    pub fn apply_sniper_config(agent: &mut AiAgent) {
        agent.perception.vision.range = 50.0;
        agent.perception.vision.half_angle = PI / 6.0;   // narrow but far
        agent.perception.hearing.base_radius = 20.0;
        agent.steering_agent.max_speed = 2.5;
        agent.blackboard.set("preferred_range", BlackboardValue::Float(25.0));
        agent.blackboard.set("aggression", BlackboardValue::Float(0.3));
        agent.blackboard.set("cover_preference", BlackboardValue::Float(0.9));
    }

    pub fn apply_berserker_config(agent: &mut AiAgent) {
        agent.perception.vision.range = 15.0;
        agent.perception.vision.half_angle = PI * 0.6;   // wide peripheral
        agent.steering_agent.max_speed = 8.0;
        agent.blackboard.set("preferred_range", BlackboardValue::Float(2.0));
        agent.blackboard.set("aggression", BlackboardValue::Float(0.95));
        agent.blackboard.set("cover_preference", BlackboardValue::Float(0.1));
        agent.emotion_engine.submit_stimulus(EmotionalStimulus { emotion: PrimaryEmotion::Anger, intensity: 0.8, source_id: 0, decay_rate_override: Some(0.005) });
    }

    pub fn apply_medic_config(agent: &mut AiAgent) {
        agent.perception.vision.range = 25.0;
        agent.steering_agent.max_speed = 4.0;
        agent.blackboard.set("preferred_range", BlackboardValue::Float(10.0));
        agent.blackboard.set("aggression", BlackboardValue::Float(0.1));
        agent.blackboard.set("heal_priority", BlackboardValue::Float(0.9));
        agent.blackboard.set("medpack_count", BlackboardValue::Int(5));
    }

    pub fn apply_scout_config(agent: &mut AiAgent) {
        agent.perception.vision.range = 35.0;
        agent.perception.vision.half_angle = PI * 0.4;
        agent.perception.hearing.base_radius = 30.0;
        agent.steering_agent.max_speed = 7.0;
        agent.blackboard.set("preferred_range", BlackboardValue::Float(15.0));
        agent.blackboard.set("aggression", BlackboardValue::Float(0.4));
        agent.blackboard.set("report_sightings", BlackboardValue::Bool(true));
        agent.emotion_engine.submit_stimulus(EmotionalStimulus { emotion: PrimaryEmotion::Anticipation, intensity: 0.6, source_id: 0, decay_rate_override: None });
    }

    pub fn apply_guardian_config(agent: &mut AiAgent) {
        agent.perception.vision.range = 20.0;
        agent.perception.vision.near_range = 3.0;
        agent.steering_agent.max_speed = 3.5;
        agent.blackboard.set("preferred_range", BlackboardValue::Float(5.0));
        agent.blackboard.set("aggression", BlackboardValue::Float(0.6));
        agent.blackboard.set("defend_position", BlackboardValue::Vec3(agent.position));
        agent.blackboard.set("defend_radius", BlackboardValue::Float(8.0));
        agent.emotion_engine.submit_stimulus(EmotionalStimulus { emotion: PrimaryEmotion::Trust, intensity: 0.7, source_id: 0, decay_rate_override: None });
    }
}

// ============================================================
// TIMELINE / REPLAY SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct AgentSnapshot {
    pub time: f32,
    pub agent_id: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub heading: Vec3,
    pub bt_status: BtStatus,
    pub active_node_id: Option<u32>,
    pub emotion_intensities: [f32; 8],
    pub blackboard_floats: HashMap<String, f32>,
    pub goap_world_state: WorldState,
}

pub struct ReplayBuffer {
    pub snapshots: VecDeque<AgentSnapshot>,
    pub max_duration: f32,
    pub snapshot_interval: f32,
    pub last_snapshot_time: f32,
    pub is_recording: bool,
    pub is_replaying: bool,
    pub replay_time: f32,
    pub replay_speed: f32,
}

impl ReplayBuffer {
    pub fn new(max_duration: f32, snapshot_interval: f32) -> Self {
        let capacity = (max_duration / snapshot_interval) as usize * 8;
        Self { snapshots: VecDeque::with_capacity(capacity), max_duration, snapshot_interval, last_snapshot_time: 0.0, is_recording: false, is_replaying: false, replay_time: 0.0, replay_speed: 1.0 }
    }

    pub fn record_agent(&mut self, agent: &AiAgent, current_time: f32) {
        if !self.is_recording { return; }
        if current_time - self.last_snapshot_time < self.snapshot_interval { return; }

        let active_node = agent.behavior_tree.as_ref().and_then(|bt| bt.root_id);
        let mut bb_floats = HashMap::new();
        for (key, value) in &agent.blackboard.entries {
            if let BlackboardValue::Float(f) = value { bb_floats.insert(key.clone(), *f); }
        }

        self.snapshots.push_back(AgentSnapshot {
            time: current_time,
            agent_id: agent.id,
            position: agent.position,
            velocity: agent.velocity,
            heading: agent.heading,
            bt_status: agent.behavior_tree.as_ref().map(|bt| bt.last_status).unwrap_or(BtStatus::Invalid),
            active_node_id: active_node,
            emotion_intensities: agent.emotion_engine.state.intensities,
            blackboard_floats: bb_floats,
            goap_world_state: agent.goap_world_state,
        });

        // Prune old snapshots
        while let Some(s) = self.snapshots.front() {
            if current_time - s.time > self.max_duration { self.snapshots.pop_front(); } else { break; }
        }
        self.last_snapshot_time = current_time;
    }

    pub fn get_snapshot_at(&self, time: f32, agent_id: u64) -> Option<&AgentSnapshot> {
        let mut best: Option<&AgentSnapshot> = None;
        for snap in &self.snapshots {
            if snap.agent_id == agent_id && snap.time <= time {
                best = Some(snap);
            }
        }
        best
    }

    pub fn interpolate_position(&self, time: f32, agent_id: u64) -> Option<Vec3> {
        let mut before: Option<&AgentSnapshot> = None;
        let mut after: Option<&AgentSnapshot> = None;
        for snap in &self.snapshots {
            if snap.agent_id != agent_id { continue; }
            if snap.time <= time { before = Some(snap); }
            if snap.time >= time && after.is_none() { after = Some(snap); }
        }
        match (before, after) {
            (Some(b), Some(a)) if b.time != a.time => {
                let t = (time - b.time) / (a.time - b.time);
                Some(b.position.lerp(a.position, t))
            }
            (Some(b), _) => Some(b.position),
            (_, Some(a)) => Some(a.position),
            _ => None,
        }
    }

    pub fn start_recording(&mut self) { self.is_recording = true; self.is_replaying = false; }
    pub fn stop_recording(&mut self) { self.is_recording = false; }
    pub fn start_replay(&mut self) { self.is_replaying = true; self.is_recording = false; if let Some(s) = self.snapshots.front() { self.replay_time = s.time; } }
    pub fn stop_replay(&mut self) { self.is_replaying = false; }

    pub fn tick_replay(&mut self, dt: f32) {
        if self.is_replaying { self.replay_time += dt * self.replay_speed; }
    }

    pub fn snapshot_count(&self) -> usize { self.snapshots.len() }
    pub fn duration_recorded(&self) -> f32 {
        match (self.snapshots.front(), self.snapshots.back()) {
            (Some(f), Some(b)) => b.time - f.time,
            _ => 0.0,
        }
    }
}

// ============================================================
// STEERING AGENT GROUP — FLOCKING SIMULATION
// ============================================================

pub struct FlockSimulation {
    pub agents: Vec<SteeringAgent>,
    pub obstacles: Vec<Aabb>,
    pub neighbor_radius: f32,
    pub separation_radius: f32,
    pub rng_seeds: Vec<u64>,
    pub seek_target: Option<Vec3>,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

impl FlockSimulation {
    pub fn new(n: usize, bounds_min: Vec3, bounds_max: Vec3) -> Self {
        let agents: Vec<SteeringAgent> = (0..n).map(|i| {
            let x = bounds_min.x + (i as f32 / n as f32) * (bounds_max.x - bounds_min.x);
            let z = bounds_min.z + ((i * 7 % n) as f32 / n as f32) * (bounds_max.z - bounds_min.z);
            SteeringAgent::new(i as u64, Vec3::new(x, 0.0, z), 4.0, 8.0)
        }).collect();
        let rng_seeds: Vec<u64> = (0..n).map(|i| (i as u64 + 1) * 6364136223846793005).collect();
        Self { agents, obstacles: Vec::new(), neighbor_radius: 5.0, separation_radius: 1.5, rng_seeds, seek_target: None, bounds_min, bounds_max }
    }

    pub fn tick(&mut self, dt: f32) {
        let n = self.agents.len();
        let agents_clone = self.agents.clone();
        let obs_clone = self.obstacles.clone();

        for i in 0..n {
            let neighbors: Vec<&SteeringAgent> = agents_clone.iter().enumerate()
                .filter(|(j, _)| *j != i)
                .filter(|(_, a)| (a.position - agents_clone[i].position).length() < self.neighbor_radius)
                .map(|(_, a)| a)
                .collect();

            let mut force = Vec3::ZERO;

            // Flocking
            force += SteeringBehaviors::alignment(&agents_clone[i], &neighbors) * ALIGNMENT_WEIGHT;
            force += SteeringBehaviors::cohesion(&agents_clone[i], &neighbors) * COHESION_WEIGHT;
            force += SteeringBehaviors::separation(&agents_clone[i], &neighbors, self.separation_radius) * SEPARATION_WEIGHT;

            // Seek center target
            if let Some(target) = self.seek_target {
                force += SteeringBehaviors::arrive(&agents_clone[i], target, ARRIVE_DECELERATION_RADIUS * 2.0) * 0.5;
            }

            // Wander if no target
            if self.seek_target.is_none() {
                force += SteeringBehaviors::wander(&mut self.agents[i], &mut self.rng_seeds[i], dt) * 0.3;
            }

            // Obstacle avoidance
            force += SteeringBehaviors::obstacle_avoidance(&agents_clone[i], &obs_clone) * 2.0;

            // Boundary avoidance
            let bmin = self.bounds_min;
            let bmax = self.bounds_max;
            let pos = agents_clone[i].position;
            let margin = 3.0;
            if pos.x < bmin.x + margin { force += Vec3::X * (bmin.x + margin - pos.x) * 2.0; }
            if pos.x > bmax.x - margin { force -= Vec3::X * (pos.x - (bmax.x - margin)) * 2.0; }
            if pos.z < bmin.z + margin { force += Vec3::Z * (bmin.z + margin - pos.z) * 2.0; }
            if pos.z > bmax.z - margin { force -= Vec3::Z * (pos.z - (bmax.z - margin)) * 2.0; }

            self.agents[i].apply_force(force, dt);
        }
    }

    pub fn average_velocity(&self) -> Vec3 {
        if self.agents.is_empty() { return Vec3::ZERO; }
        let sum = self.agents.iter().map(|a| a.velocity).fold(Vec3::ZERO, |a, b| a + b);
        sum / self.agents.len() as f32
    }

    pub fn centroid(&self) -> Vec3 {
        if self.agents.is_empty() { return Vec3::ZERO; }
        let sum = self.agents.iter().map(|a| a.position).fold(Vec3::ZERO, |a, b| a + b);
        sum / self.agents.len() as f32
    }

    pub fn spread(&self) -> f32 {
        let center = self.centroid();
        if self.agents.is_empty() { return 0.0; }
        self.agents.iter().map(|a| (a.position - center).length()).sum::<f32>() / self.agents.len() as f32
    }
}

// ============================================================
// AI STATE PROFILER
// ============================================================

pub struct AiStateProfiler {
    pub mode_time: HashMap<String, f32>,
    pub bt_node_time: HashMap<u32, f32>,
    pub current_mode: String,
    pub mode_entry_time: f32,
    pub current_time: f32,
    pub sample_count: u64,
}

impl AiStateProfiler {
    pub fn new() -> Self {
        Self { mode_time: HashMap::new(), bt_node_time: HashMap::new(), current_mode: "idle".to_string(), mode_entry_time: 0.0, current_time: 0.0, sample_count: 0 }
    }

    pub fn enter_mode(&mut self, mode: &str) {
        let elapsed = self.current_time - self.mode_entry_time;
        if elapsed > 0.0 {
            *self.mode_time.entry(self.current_mode.clone()).or_insert(0.0) += elapsed;
        }
        self.current_mode = mode.to_string();
        self.mode_entry_time = self.current_time;
    }

    pub fn tick(&mut self, dt: f32) {
        self.current_time += dt;
        self.sample_count += 1;
    }

    pub fn mode_percentage(&self, mode: &str) -> f32 {
        let total: f32 = self.mode_time.values().sum();
        if total < EPSILON { return 0.0; }
        self.mode_time.get(mode).copied().unwrap_or(0.0) / total
    }

    pub fn most_common_mode(&self) -> Option<(&str, f32)> {
        let total: f32 = self.mode_time.values().sum();
        if total < EPSILON { return None; }
        self.mode_time.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, &v)| (k.as_str(), v / total))
    }

    pub fn report(&self) -> String {
        let mut out = String::from("AI State Profile:\n");
        let total: f32 = self.mode_time.values().sum();
        let mut sorted: Vec<(&String, &f32)> = self.mode_time.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        for (mode, time) in sorted {
            let pct = if total > 0.0 { time / total * 100.0 } else { 0.0 };
            out.push_str(&format!("  {}: {:.2}s ({:.1}%)\n", mode, time, pct));
        }
        out.push_str(&format!("  Total: {:.2}s  Samples: {}\n", total, self.sample_count));
        out
    }
}

// ============================================================
// HEAT MAP RENDERER DATA
// ============================================================

pub struct HeatMapData {
    pub width: usize,
    pub height: usize,
    pub values: Vec<f32>,
    pub cell_size: f32,
    pub origin: Vec2,
    pub max_value: f32,
    pub label: String,
}

impl HeatMapData {
    pub fn new(width: usize, height: usize, cell_size: f32, origin: Vec2, label: &str) -> Self {
        Self { width, height, values: vec![0.0; width * height], cell_size, origin, max_value: 1.0, label: label.to_string() }
    }

    pub fn add_point(&mut self, pos: Vec2, value: f32, radius: f32) {
        let cx = ((pos.x - self.origin.x) / self.cell_size).floor() as i32;
        let cy = ((pos.y - self.origin.y) / self.cell_size).floor() as i32;
        let cell_r = (radius / self.cell_size).ceil() as i32;
        for dy in -cell_r..=cell_r {
            for dx in -cell_r..=cell_r {
                let nx = cx + dx;
                let ny = cy + dy;
                if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 { continue; }
                let dist = ((dx * dx + dy * dy) as f32).sqrt() * self.cell_size;
                if dist <= radius {
                    let falloff = 1.0 - dist / (radius + EPSILON);
                    let idx = ny as usize * self.width + nx as usize;
                    self.values[idx] += value * falloff * falloff;
                    if self.values[idx] > self.max_value { self.max_value = self.values[idx]; }
                }
            }
        }
    }

    pub fn normalize(&mut self) {
        if self.max_value > EPSILON {
            for v in &mut self.values { *v /= self.max_value; }
            self.max_value = 1.0;
        }
    }

    pub fn get_normalized(&self, x: usize, y: usize) -> f32 {
        if x >= self.width || y >= self.height { return 0.0; }
        let v = self.values[y * self.width + x];
        if self.max_value > EPSILON { v / self.max_value } else { 0.0 }
    }

    pub fn to_rgba_gradient(&self, low: Vec4, high: Vec4) -> Vec<Vec4> {
        self.values.iter().map(|&v| {
            let t = (v / self.max_value.max(EPSILON)).clamp(0.0, 1.0);
            Vec4::new(
                low.x + (high.x - low.x) * t,
                low.y + (high.y - low.y) * t,
                low.z + (high.z - low.z) * t,
                low.w + (high.w - low.w) * t,
            )
        }).collect()
    }

    pub fn from_agent_positions(agents: &[AiAgent], width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        let mut hmap = HeatMapData::new(width, height, cell_size, origin, "Agent Positions");
        for agent in agents {
            let pos_2d = Vec2::new(agent.position.x, agent.position.z);
            hmap.add_point(pos_2d, 1.0, cell_size * 2.0);
        }
        hmap.normalize();
        hmap
    }

    pub fn from_threat_data(threat_assessors: &[(Vec3, f32)], width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        let mut hmap = HeatMapData::new(width, height, cell_size, origin, "Threat");
        for &(pos, threat) in threat_assessors {
            hmap.add_point(Vec2::new(pos.x, pos.z), threat, cell_size * 3.0);
        }
        hmap.normalize();
        hmap
    }
}

// ============================================================
// EDITOR SEARCH SYSTEM
// ============================================================

pub struct EditorSearch {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub selected_result: Option<usize>,
    pub search_bt_nodes: bool,
    pub search_blackboard: bool,
    pub search_goap_actions: bool,
    pub search_fsm_states: bool,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub label: String,
    pub category: String,
    pub location: SearchLocation,
    pub relevance: f32,
}

#[derive(Clone, Debug)]
pub enum SearchLocation {
    BtNode { tree_idx: usize, node_id: u32 },
    FsmState { fsm_idx: usize, state_id: u32 },
    GoapAction { action_id: u32 },
    BlackboardKey { key: String },
    UtilityAction { action_id: u32 },
}

impl EditorSearch {
    pub fn new() -> Self {
        Self { query: String::new(), results: Vec::new(), selected_result: None, search_bt_nodes: true, search_blackboard: true, search_goap_actions: true, search_fsm_states: true }
    }

    pub fn search(&mut self, editor: &AiBehaviorEditor) {
        self.results.clear();
        if self.query.is_empty() { return; }
        let q = self.query.to_lowercase();

        if self.search_bt_nodes {
            for (tree_idx, tree) in editor.behavior_trees.iter().enumerate() {
                for (node_id, node) in &tree.nodes {
                    let name = node.display_name().to_lowercase();
                    if name.contains(&q) {
                        let relevance = if name == q { 1.0 } else if name.starts_with(&q) { 0.8 } else { 0.5 };
                        self.results.push(SearchResult {
                            label: format!("{} [{}]", node.display_name(), node_id),
                            category: format!("BT: {}", tree.name),
                            location: SearchLocation::BtNode { tree_idx, node_id: *node_id },
                            relevance,
                        });
                    }
                }
            }
        }

        if self.search_fsm_states {
            for (fsm_idx, fsm) in editor.fsm_instances.iter().enumerate() {
                for (state_id, state) in &fsm.states {
                    if state.name.to_lowercase().contains(&q) {
                        self.results.push(SearchResult {
                            label: state.name.clone(),
                            category: format!("FSM: {}", fsm.name),
                            location: SearchLocation::FsmState { fsm_idx, state_id: *state_id },
                            relevance: 0.7,
                        });
                    }
                }
            }
        }

        if self.search_goap_actions {
            for action in &editor.goap_planner.actions {
                if action.name.to_lowercase().contains(&q) {
                    self.results.push(SearchResult {
                        label: action.name.clone(),
                        category: "GOAP Action".to_string(),
                        location: SearchLocation::GoapAction { action_id: action.id },
                        relevance: 0.6,
                    });
                }
            }
        }

        if self.search_blackboard {
            for key in editor.shared_blackboard.entries.keys() {
                if key.to_lowercase().contains(&q) {
                    self.results.push(SearchResult {
                        label: key.clone(),
                        category: "Blackboard".to_string(),
                        location: SearchLocation::BlackboardKey { key: key.clone() },
                        relevance: 0.5,
                    });
                }
            }
        }

        self.results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap());
        self.results.truncate(50);
        self.selected_result = if self.results.is_empty() { None } else { Some(0) };
    }

    pub fn select_next(&mut self) {
        if let Some(idx) = self.selected_result {
            self.selected_result = Some((idx + 1) % self.results.len().max(1));
        }
    }

    pub fn select_prev(&mut self) {
        if let Some(idx) = self.selected_result {
            self.selected_result = Some(if idx == 0 { self.results.len().saturating_sub(1) } else { idx - 1 });
        }
    }
}

// ============================================================
// FINAL MODULE EXPORTS AND CONSTANTS SUMMARY
// ============================================================

pub const AI_EDITOR_VERSION: &str = "1.0.0";
pub const AI_EDITOR_MAX_AGENTS: usize = 1024;
pub const AI_EDITOR_MAX_TREES: usize = 256;
pub const AI_EDITOR_MAX_FSMS: usize = 128;
pub const AI_EDITOR_MAX_GOAP_ACTIONS: usize = 64;
pub const AI_EDITOR_MAX_UTILITY_ACTIONS: usize = 32;

pub struct AiEditorCapabilities {
    pub supports_bt: bool,
    pub supports_goap: bool,
    pub supports_utility: bool,
    pub supports_fsm: bool,
    pub supports_htn: bool,
    pub supports_fuzzy: bool,
    pub supports_perception: bool,
    pub supports_steering: bool,
    pub supports_emotions: bool,
    pub supports_formations: bool,
    pub supports_cover: bool,
    pub supports_influence_maps: bool,
    pub supports_navmesh: bool,
    pub supports_replay: bool,
    pub supports_dda: bool,
    pub supports_dialog: bool,
    pub max_agents: usize,
    pub max_bt_nodes_per_tree: usize,
    pub max_fsm_states: usize,
}

impl Default for AiEditorCapabilities {
    fn default() -> Self {
        Self {
            supports_bt: true,
            supports_goap: true,
            supports_utility: true,
            supports_fsm: true,
            supports_htn: true,
            supports_fuzzy: true,
            supports_perception: true,
            supports_steering: true,
            supports_emotions: true,
            supports_formations: true,
            supports_cover: true,
            supports_influence_maps: true,
            supports_navmesh: true,
            supports_replay: true,
            supports_dda: true,
            supports_dialog: true,
            max_agents: AI_EDITOR_MAX_AGENTS,
            max_bt_nodes_per_tree: 512,
            max_fsm_states: AI_EDITOR_MAX_FSMS,
        }
    }
}

pub fn get_capabilities() -> AiEditorCapabilities { AiEditorCapabilities::default() }

pub fn ai_editor_info() -> String {
    format!(
        "AI Behavior Editor v{}\nCapabilities: BT={}, GOAP={}, Utility={}, FSM={}, HTN={}, Fuzzy={}\nPerception, Steering(18 types), Emotions(Plutchik), Formations(10), Cover, InfluenceMaps, NavMesh, Replay, DDA, Dialog\nMax Agents: {}",
        AI_EDITOR_VERSION, true, true, true, true, true, true, AI_EDITOR_MAX_AGENTS
    )
}
