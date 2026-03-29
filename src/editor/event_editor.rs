
//! Event system editor — visual event graph, publisher/subscriber wiring,
//! event timeline, dispatch tracing, priority queues, and event replay.

use glam::{Vec2, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Event value types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventParamType {
    Void,
    Bool,
    Int,
    UInt,
    Float,
    Double,
    String,
    Vec2,
    Vec3,
    Vec4,
    Quaternion,
    EntityId,
    AssetRef,
    Color,
    Bytes,
    Json,
    Custom,
}

impl EventParamType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Bool => "bool",
            Self::Int => "int",
            Self::UInt => "uint",
            Self::Float => "float",
            Self::Double => "double",
            Self::String => "string",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Quaternion => "quat",
            Self::EntityId => "EntityId",
            Self::AssetRef => "AssetRef",
            Self::Color => "Color",
            Self::Bytes => "bytes",
            Self::Json => "JSON",
            Self::Custom => "Custom",
        }
    }

    pub fn size_bytes(&self) -> usize {
        match self {
            Self::Void => 0,
            Self::Bool => 1,
            Self::Int | Self::UInt | Self::Float => 4,
            Self::Double | Self::EntityId => 8,
            Self::Vec2 => 8,
            Self::Vec3 => 12,
            Self::Vec4 | Self::Quaternion | Self::Color => 16,
            Self::String | Self::Bytes | Self::Json | Self::Custom | Self::AssetRef => 32,
        }
    }

    pub fn port_color(&self) -> Vec4 {
        match self {
            Self::Void => Vec4::new(0.6, 0.6, 0.6, 1.0),
            Self::Bool => Vec4::new(0.87, 0.4, 0.4, 1.0),
            Self::Int | Self::UInt => Vec4::new(0.4, 0.8, 0.4, 1.0),
            Self::Float | Self::Double => Vec4::new(0.4, 0.7, 0.87, 1.0),
            Self::String | Self::Json => Vec4::new(0.87, 0.7, 0.4, 1.0),
            Self::Vec2 | Self::Vec3 | Self::Vec4 => Vec4::new(0.6, 0.4, 0.87, 1.0),
            Self::Quaternion => Vec4::new(0.7, 0.4, 0.87, 1.0),
            Self::EntityId => Vec4::new(0.87, 0.87, 0.4, 1.0),
            Self::Color => Vec4::new(0.87, 0.5, 0.3, 1.0),
            _ => Vec4::new(0.5, 0.5, 0.5, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Event definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EventParam {
    pub name: String,
    pub param_type: EventParamType,
    pub description: String,
    pub optional: bool,
    pub default_value: String,
}

impl EventParam {
    pub fn new(name: &str, param_type: EventParamType) -> Self {
        Self {
            name: name.to_string(),
            param_type,
            description: String::new(),
            optional: false,
            default_value: String::new(),
        }
    }
    pub fn optional(mut self) -> Self { self.optional = true; self }
    pub fn with_default(mut self, def: &str) -> Self { self.default_value = def.to_string(); self }
    pub fn with_description(mut self, desc: &str) -> Self { self.description = desc.to_string(); self }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventPriority { Lowest = 0, Low = 25, Normal = 50, High = 75, Critical = 100, Immediate = 255 }

impl EventPriority {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lowest => "Lowest",
            Self::Low => "Low",
            Self::Normal => "Normal",
            Self::High => "High",
            Self::Critical => "Critical",
            Self::Immediate => "Immediate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventDispatchMode {
    Immediate,
    Queued,
    Deferred,
    LateTick,
    FixedUpdate,
    ThreadSafe,
    Broadcast,
}

impl EventDispatchMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Immediate => "Immediate",
            Self::Queued => "Queued",
            Self::Deferred => "Deferred",
            Self::LateTick => "Late Tick",
            Self::FixedUpdate => "Fixed Update",
            Self::ThreadSafe => "Thread-Safe",
            Self::Broadcast => "Broadcast",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            Self::Immediate => "Dispatched and handled in the same call frame",
            Self::Queued => "Added to a queue, processed next tick",
            Self::Deferred => "Processed after current frame",
            Self::LateTick => "Processed at the end of the update loop",
            Self::FixedUpdate => "Processed during fixed-rate update",
            Self::ThreadSafe => "Can be raised from any thread",
            Self::Broadcast => "Sent to all listening systems globally",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventDefinition {
    pub id: u64,
    pub name: String,
    pub namespace: String,
    pub params: Vec<EventParam>,
    pub priority: EventPriority,
    pub dispatch_mode: EventDispatchMode,
    pub category: String,
    pub description: String,
    pub cancelable: bool,
    pub persistent: bool,
    pub one_shot: bool,
    pub max_listeners: Option<u32>,
    pub ttl_ms: Option<u32>,
    pub tags: Vec<String>,
    pub deprecated: bool,
    pub version: u32,
}

impl EventDefinition {
    pub fn new(id: u64, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            namespace: "game".to_string(),
            params: Vec::new(),
            priority: EventPriority::Normal,
            dispatch_mode: EventDispatchMode::Queued,
            category: "General".to_string(),
            description: String::new(),
            cancelable: false,
            persistent: false,
            one_shot: false,
            max_listeners: None,
            ttl_ms: None,
            tags: Vec::new(),
            deprecated: false,
            version: 1,
        }
    }
    pub fn with_param(mut self, param: EventParam) -> Self { self.params.push(param); self }
    pub fn with_priority(mut self, priority: EventPriority) -> Self { self.priority = priority; self }
    pub fn with_mode(mut self, mode: EventDispatchMode) -> Self { self.dispatch_mode = mode; self }
    pub fn cancelable(mut self) -> Self { self.cancelable = true; self }
    pub fn persistent(mut self) -> Self { self.persistent = true; self }
    pub fn with_category(mut self, cat: &str) -> Self { self.category = cat.to_string(); self }
    pub fn full_name(&self) -> String { format!("{}.{}", self.namespace, self.name) }
    pub fn payload_size_bytes(&self) -> usize { self.params.iter().map(|p| p.param_type.size_bytes()).sum() }
}

// ---------------------------------------------------------------------------
// Event listener
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListenerKind {
    System,
    Component,
    Script,
    UI,
    Animation,
    Audio,
    Network,
    AI,
    Physics,
    VFX,
    Custom,
}

impl ListenerKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Component => "Component",
            Self::Script => "Script",
            Self::UI => "UI",
            Self::Animation => "Animation",
            Self::Audio => "Audio",
            Self::Network => "Network",
            Self::AI => "AI",
            Self::Physics => "Physics",
            Self::VFX => "VFX",
            Self::Custom => "Custom",
        }
    }
    pub fn color(&self) -> Vec4 {
        match self {
            Self::System => Vec4::new(0.8, 0.8, 0.3, 1.0),
            Self::Component => Vec4::new(0.3, 0.7, 0.9, 1.0),
            Self::Script => Vec4::new(0.5, 0.9, 0.5, 1.0),
            Self::UI => Vec4::new(0.9, 0.5, 0.8, 1.0),
            Self::Animation => Vec4::new(0.9, 0.7, 0.3, 1.0),
            Self::Audio => Vec4::new(0.5, 0.8, 0.9, 1.0),
            Self::Network => Vec4::new(0.9, 0.3, 0.3, 1.0),
            Self::AI => Vec4::new(0.7, 0.4, 0.9, 1.0),
            Self::Physics => Vec4::new(0.4, 0.9, 0.7, 1.0),
            Self::VFX => Vec4::new(0.9, 0.6, 0.2, 1.0),
            Self::Custom => Vec4::new(0.6, 0.6, 0.6, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventListener {
    pub id: u64,
    pub name: String,
    pub kind: ListenerKind,
    pub event_id: u64,
    pub priority: i32,
    pub consume_event: bool,
    pub filter_expression: Option<String>,
    pub handler_code: String,
    pub enabled: bool,
    pub call_count: u64,
    pub total_time_us: f64,
    pub last_triggered_ms: Option<f64>,
    pub max_calls_per_frame: Option<u32>,
    pub cooldown_ms: Option<u32>,
}

impl EventListener {
    pub fn new(id: u64, name: &str, event_id: u64, kind: ListenerKind) -> Self {
        Self {
            id,
            name: name.to_string(),
            kind,
            event_id,
            priority: 0,
            consume_event: false,
            filter_expression: None,
            handler_code: String::new(),
            enabled: true,
            call_count: 0,
            total_time_us: 0.0,
            last_triggered_ms: None,
            max_calls_per_frame: None,
            cooldown_ms: None,
        }
    }
    pub fn avg_time_us(&self) -> f64 {
        if self.call_count == 0 { 0.0 } else { self.total_time_us / self.call_count as f64 }
    }
}

// ---------------------------------------------------------------------------
// Event publisher
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EventPublisher {
    pub id: u64,
    pub name: String,
    pub kind: ListenerKind,
    pub publishes: Vec<u64>,
    pub dispatch_count: u64,
    pub last_dispatch_ms: Option<f64>,
    pub enabled: bool,
}

impl EventPublisher {
    pub fn new(id: u64, name: &str, kind: ListenerKind) -> Self {
        Self {
            id,
            name: name.to_string(),
            kind,
            publishes: Vec::new(),
            dispatch_count: 0,
            last_dispatch_ms: None,
            enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Event channel (direct pub/sub wiring)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EventChannel {
    pub id: u64,
    pub publisher_id: u64,
    pub listener_id: u64,
    pub event_id: u64,
    pub transform_code: Option<String>,
    pub active: bool,
    pub total_dispatches: u64,
    pub dropped_count: u64,
}

// ---------------------------------------------------------------------------
// Visual event graph node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventNodeKind {
    EventDefinition,
    Publisher,
    Listener,
    Filter,
    Transform,
    Splitter,
    Merger,
    Throttle,
    Debounce,
    Buffer,
    Delay,
    Map,
    Reduce,
    Fork,
    Conditional,
    Logger,
    Counter,
    Timer,
    Gate,
    LatchSet,
    LatchReset,
    Comment,
}

impl EventNodeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::EventDefinition => "Event",
            Self::Publisher => "Publisher",
            Self::Listener => "Listener",
            Self::Filter => "Filter",
            Self::Transform => "Transform",
            Self::Splitter => "Splitter",
            Self::Merger => "Merger",
            Self::Throttle => "Throttle",
            Self::Debounce => "Debounce",
            Self::Buffer => "Buffer",
            Self::Delay => "Delay",
            Self::Map => "Map",
            Self::Reduce => "Reduce",
            Self::Fork => "Fork",
            Self::Conditional => "Conditional",
            Self::Logger => "Logger",
            Self::Counter => "Counter",
            Self::Timer => "Timer",
            Self::Gate => "Gate",
            Self::LatchSet => "Latch (Set)",
            Self::LatchReset => "Latch (Reset)",
            Self::Comment => "Comment",
        }
    }
    pub fn header_color(&self) -> Vec4 {
        match self {
            Self::EventDefinition => Vec4::new(0.2, 0.4, 0.7, 1.0),
            Self::Publisher => Vec4::new(0.7, 0.4, 0.2, 1.0),
            Self::Listener => Vec4::new(0.2, 0.7, 0.4, 1.0),
            Self::Filter | Self::Conditional | Self::Gate => Vec4::new(0.6, 0.2, 0.6, 1.0),
            Self::Throttle | Self::Debounce | Self::Delay => Vec4::new(0.6, 0.6, 0.2, 1.0),
            Self::Map | Self::Transform | Self::Reduce => Vec4::new(0.2, 0.6, 0.6, 1.0),
            Self::Logger | Self::Counter => Vec4::new(0.4, 0.4, 0.4, 1.0),
            _ => Vec4::new(0.3, 0.3, 0.3, 1.0),
        }
    }
}

static mut EVENT_NODE_ID: u64 = 1;
fn next_event_node_id() -> u64 {
    unsafe { let id = EVENT_NODE_ID; EVENT_NODE_ID += 1; id }
}

#[derive(Debug, Clone)]
pub struct EventGraphNode {
    pub id: u64,
    pub kind: EventNodeKind,
    pub position: Vec2,
    pub size: Vec2,
    pub label: String,
    pub linked_id: Option<u64>,
    pub params: HashMap<String, String>,
    pub enabled: bool,
    pub collapsed: bool,
    pub comment: String,
    pub dispatch_count: u64,
    pub error_message: Option<String>,
}

impl EventGraphNode {
    pub fn new(kind: EventNodeKind, position: Vec2) -> Self {
        Self {
            id: next_event_node_id(),
            kind,
            position,
            size: Vec2::new(180.0, 80.0),
            label: kind.label().to_string(),
            linked_id: None,
            params: HashMap::new(),
            enabled: true,
            collapsed: false,
            comment: String::new(),
            dispatch_count: 0,
            error_message: None,
        }
    }
    pub fn contains(&self, pt: Vec2) -> bool {
        pt.x >= self.position.x && pt.x <= self.position.x + self.size.x
            && pt.y >= self.position.y && pt.y <= self.position.y + self.size.y
    }
    pub fn set_param(&mut self, key: &str, value: &str) {
        self.params.insert(key.to_string(), value.to_string());
    }
}

#[derive(Debug, Clone)]
pub struct EventGraphEdge {
    pub id: u64,
    pub from_node: u64,
    pub from_port: u32,
    pub to_node: u64,
    pub to_port: u32,
    pub event_id: Option<u64>,
    pub active: bool,
    pub dispatch_count: u64,
}

// ---------------------------------------------------------------------------
// Event graph
// ---------------------------------------------------------------------------

static mut EVENT_GRAPH_ID: u64 = 1;
fn next_eg_id() -> u64 { unsafe { let id = EVENT_GRAPH_ID; EVENT_GRAPH_ID += 1; id } }

#[derive(Debug, Clone)]
pub struct EventGraph {
    pub name: String,
    pub nodes: Vec<EventGraphNode>,
    pub edges: Vec<EventGraphEdge>,
    pub next_edge_id: u64,
}

impl EventGraph {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), nodes: Vec::new(), edges: Vec::new(), next_edge_id: 1 }
    }

    pub fn add_node(&mut self, node: EventGraphNode) -> u64 {
        let id = node.id;
        self.nodes.push(node);
        id
    }

    pub fn connect(&mut self, from: u64, from_port: u32, to: u64, to_port: u32) -> u64 {
        let id = self.next_edge_id;
        self.next_edge_id += 1;
        self.edges.push(EventGraphEdge { id, from_node: from, from_port, to_node: to, to_port, event_id: None, active: true, dispatch_count: 0 });
        id
    }

    pub fn find_node(&self, id: u64) -> Option<&EventGraphNode> { self.nodes.iter().find(|n| n.id == id) }
    pub fn find_node_mut(&mut self, id: u64) -> Option<&mut EventGraphNode> { self.nodes.iter_mut().find(|n| n.id == id) }

    pub fn build_player_combat_graph() -> Self {
        let mut g = Self::new("PlayerCombat");
        let fire_event = g.add_node(EventGraphNode::new(EventNodeKind::EventDefinition, Vec2::new(20.0, 20.0)));
        if let Some(n) = g.find_node_mut(fire_event) { n.label = "OnPlayerFire".to_string(); }
        let hit_event = g.add_node(EventGraphNode::new(EventNodeKind::EventDefinition, Vec2::new(20.0, 140.0)));
        if let Some(n) = g.find_node_mut(hit_event) { n.label = "OnHitDetected".to_string(); }
        let death_event = g.add_node(EventGraphNode::new(EventNodeKind::EventDefinition, Vec2::new(20.0, 260.0)));
        if let Some(n) = g.find_node_mut(death_event) { n.label = "OnPlayerDeath".to_string(); }

        let fire_pub = g.add_node(EventGraphNode::new(EventNodeKind::Publisher, Vec2::new(260.0, 20.0)));
        if let Some(n) = g.find_node_mut(fire_pub) { n.label = "WeaponSystem".to_string(); }
        let hit_pub = g.add_node(EventGraphNode::new(EventNodeKind::Publisher, Vec2::new(260.0, 140.0)));
        if let Some(n) = g.find_node_mut(hit_pub) { n.label = "HitDetectionSystem".to_string(); }

        let throttle = g.add_node({ let mut n = EventGraphNode::new(EventNodeKind::Throttle, Vec2::new(480.0, 20.0)); n.set_param("rate_hz", "10"); n });
        let filter = g.add_node({ let mut n = EventGraphNode::new(EventNodeKind::Filter, Vec2::new(480.0, 140.0)); n.set_param("condition", "damage > 0"); n });
        let splitter = g.add_node(EventGraphNode::new(EventNodeKind::Splitter, Vec2::new(700.0, 140.0)));

        let audio_listener = g.add_node(EventGraphNode::new(EventNodeKind::Listener, Vec2::new(900.0, 80.0)));
        if let Some(n) = g.find_node_mut(audio_listener) { n.label = "AudioSystem".to_string(); }
        let vfx_listener = g.add_node(EventGraphNode::new(EventNodeKind::Listener, Vec2::new(900.0, 180.0)));
        if let Some(n) = g.find_node_mut(vfx_listener) { n.label = "VFXSystem".to_string(); }
        let ui_listener = g.add_node(EventGraphNode::new(EventNodeKind::Listener, Vec2::new(900.0, 280.0)));
        if let Some(n) = g.find_node_mut(ui_listener) { n.label = "UIHUDSystem".to_string(); }
        let logger = g.add_node(EventGraphNode::new(EventNodeKind::Logger, Vec2::new(900.0, 380.0)));

        g.connect(fire_pub, 0, throttle, 0);
        g.connect(throttle, 0, audio_listener, 0);
        g.connect(hit_pub, 0, filter, 0);
        g.connect(filter, 0, splitter, 0);
        g.connect(splitter, 0, audio_listener, 0);
        g.connect(splitter, 1, vfx_listener, 0);
        g.connect(splitter, 2, ui_listener, 0);
        g.connect(splitter, 3, logger, 0);
        g
    }
}

// ---------------------------------------------------------------------------
// Event trace (runtime recording)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EventTraceEntry {
    pub timestamp_ms: f64,
    pub frame: u64,
    pub event_name: String,
    pub event_id: u64,
    pub dispatch_mode: EventDispatchMode,
    pub priority: EventPriority,
    pub publisher: String,
    pub listener_count: u32,
    pub cancelled: bool,
    pub consumed: bool,
    pub dispatch_time_us: f32,
    pub payload_bytes: usize,
}

#[derive(Debug, Clone, Default)]
pub struct EventTrace {
    pub entries: Vec<EventTraceEntry>,
    pub recording: bool,
    pub max_entries: usize,
    pub filter_event: Option<String>,
    pub filter_publisher: Option<String>,
    pub paused: bool,
}

impl EventTrace {
    pub fn new(max: usize) -> Self { Self { max_entries: max, ..Default::default() } }
    pub fn push(&mut self, entry: EventTraceEntry) {
        if !self.recording || self.paused { return; }
        if let Some(ref f) = self.filter_event {
            if !entry.event_name.contains(f.as_str()) { return; }
        }
        self.entries.push(entry);
        if self.entries.len() > self.max_entries { self.entries.remove(0); }
    }
    pub fn clear(&mut self) { self.entries.clear(); }
    pub fn total_events(&self) -> usize { self.entries.len() }
    pub fn events_per_second(&self) -> f64 {
        if self.entries.len() < 2 { return 0.0; }
        let dt = self.entries.last().unwrap().timestamp_ms - self.entries.first().unwrap().timestamp_ms;
        if dt <= 0.0 { return 0.0; }
        self.entries.len() as f64 / (dt / 1000.0)
    }
    pub fn most_frequent(&self, n: usize) -> Vec<(&str, usize)> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for e in &self.entries { *counts.entry(e.event_name.as_str()).or_insert(0) += 1; }
        let mut sorted: Vec<(&str, usize)> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().take(n).collect()
    }
    pub fn avg_dispatch_time_us(&self) -> f32 {
        if self.entries.is_empty() { return 0.0; }
        self.entries.iter().map(|e| e.dispatch_time_us).sum::<f32>() / self.entries.len() as f32
    }
    pub fn cancelled_count(&self) -> usize { self.entries.iter().filter(|e| e.cancelled).count() }
}

// ---------------------------------------------------------------------------
// Event queue visualization
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QueuedEvent {
    pub event_id: u64,
    pub event_name: String,
    pub queued_at_ms: f64,
    pub scheduled_frame: u64,
    pub priority: EventPriority,
    pub publisher: String,
    pub payload_preview: String,
    pub age_ms: f32,
}

#[derive(Debug, Clone, Default)]
pub struct EventQueueVisualizer {
    pub queued: Vec<QueuedEvent>,
    pub processed_this_frame: u32,
    pub dropped_this_frame: u32,
    pub max_queue_depth_seen: usize,
    pub avg_queue_depth: f32,
    pub frame: u64,
}

impl EventQueueVisualizer {
    pub fn simulate_frame(&mut self, current_time_ms: f64) {
        self.frame += 1;
        // simulate random events being added and processed
        let seed: u64 = self.frame.wrapping_mul(6364136223846793005);
        let rng = ((seed >> 33) as f32) / (u32::MAX as f32);

        if rng > 0.3 {
            self.queued.push(QueuedEvent {
                event_id: self.frame,
                event_name: ["OnPlayerMove", "OnHit", "OnFire", "OnCollision", "OnTick"][((rng * 5.0) as usize).min(4)].to_string(),
                queued_at_ms: current_time_ms,
                scheduled_frame: self.frame,
                priority: EventPriority::Normal,
                publisher: "GameSystem".to_string(),
                payload_preview: "{ ... }".to_string(),
                age_ms: 0.0,
            });
        }

        let to_process = (self.queued.len() as f32 * 0.8) as usize;
        self.processed_this_frame = to_process as u32;
        self.queued = self.queued.split_off(to_process.min(self.queued.len()));

        if self.queued.len() > self.max_queue_depth_seen {
            self.max_queue_depth_seen = self.queued.len();
        }
        self.avg_queue_depth = self.avg_queue_depth * 0.95 + self.queued.len() as f32 * 0.05;
    }
}

// ---------------------------------------------------------------------------
// Event library
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct EventLibrary {
    pub events: Vec<EventDefinition>,
    pub next_id: u64,
}

impl EventLibrary {
    pub fn with_defaults() -> Self {
        let mut lib = Self { events: Vec::new(), next_id: 1 };
        lib.add(EventDefinition::new(lib.next_id, "OnGameStart").with_category("Game"));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnGameEnd").with_category("Game")
            .with_param(EventParam::new("reason", EventParamType::String)));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnLevelLoaded").with_category("Level")
            .with_param(EventParam::new("scene_name", EventParamType::String)));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnPlayerSpawned").with_category("Player")
            .with_param(EventParam::new("entity_id", EventParamType::EntityId))
            .with_param(EventParam::new("position", EventParamType::Vec3)));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnPlayerDied").with_category("Player")
            .with_param(EventParam::new("entity_id", EventParamType::EntityId))
            .with_param(EventParam::new("killer_id", EventParamType::EntityId).optional())
            .with_priority(EventPriority::High).cancelable());
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnDamageDealt").with_category("Combat")
            .with_param(EventParam::new("target", EventParamType::EntityId))
            .with_param(EventParam::new("attacker", EventParamType::EntityId))
            .with_param(EventParam::new("amount", EventParamType::Float))
            .with_param(EventParam::new("type", EventParamType::String))
            .cancelable());
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnItemPickedUp").with_category("Inventory")
            .with_param(EventParam::new("entity_id", EventParamType::EntityId))
            .with_param(EventParam::new("item_id", EventParamType::AssetRef))
            .with_param(EventParam::new("count", EventParamType::Int)));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnQuestCompleted").with_category("Quest")
            .with_param(EventParam::new("quest_id", EventParamType::String))
            .with_priority(EventPriority::High));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnDialogueStarted").with_category("Dialogue")
            .with_param(EventParam::new("npc_id", EventParamType::EntityId))
            .with_param(EventParam::new("dialogue_id", EventParamType::String)));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnAchievementUnlocked").with_category("Progression")
            .with_param(EventParam::new("achievement_id", EventParamType::String))
            .with_priority(EventPriority::High).persistent());
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnUIButtonClicked").with_category("UI")
            .with_param(EventParam::new("button_id", EventParamType::String))
            .with_mode(EventDispatchMode::Immediate));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnSettingsChanged").with_category("System")
            .with_param(EventParam::new("category", EventParamType::String))
            .with_mode(EventDispatchMode::Broadcast));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnNetworkDisconnect").with_category("Network")
            .with_param(EventParam::new("player_id", EventParamType::EntityId))
            .with_param(EventParam::new("reason", EventParamType::String))
            .with_priority(EventPriority::Critical));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnPhysicsCollision").with_category("Physics")
            .with_param(EventParam::new("entity_a", EventParamType::EntityId))
            .with_param(EventParam::new("entity_b", EventParamType::EntityId))
            .with_param(EventParam::new("contact_point", EventParamType::Vec3))
            .with_param(EventParam::new("impulse", EventParamType::Float))
            .with_mode(EventDispatchMode::FixedUpdate));
        lib.next_id += 1;
        lib.add(EventDefinition::new(lib.next_id, "OnAnimationEvent").with_category("Animation")
            .with_param(EventParam::new("entity_id", EventParamType::EntityId))
            .with_param(EventParam::new("event_name", EventParamType::String))
            .with_param(EventParam::new("frame", EventParamType::Int)));
        lib.next_id += 1;
        lib
    }

    pub fn add(&mut self, def: EventDefinition) { self.events.push(def); }
    pub fn find_by_name(&self, name: &str) -> Option<&EventDefinition> { self.events.iter().find(|e| e.name == name) }
    pub fn find_by_id(&self, id: u64) -> Option<&EventDefinition> { self.events.iter().find(|e| e.id == id) }
    pub fn by_category(&self) -> HashMap<&str, Vec<&EventDefinition>> {
        let mut map: HashMap<&str, Vec<&EventDefinition>> = HashMap::new();
        for e in &self.events { map.entry(e.category.as_str()).or_default().push(e); }
        map
    }
    pub fn search(&self, query: &str) -> Vec<&EventDefinition> {
        let q = query.to_lowercase();
        self.events.iter().filter(|e| e.name.to_lowercase().contains(&q) || e.category.to_lowercase().contains(&q)).collect()
    }
    pub fn total_payload_size_bytes(&self) -> usize { self.events.iter().map(|e| e.payload_size_bytes()).sum() }
}

// ---------------------------------------------------------------------------
// Event editor panels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventEditorPanel {
    EventLibrary,
    Graph,
    Listeners,
    Publishers,
    Trace,
    Queue,
    Statistics,
    CodeGen,
}

impl EventEditorPanel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::EventLibrary => "Events",
            Self::Graph => "Graph",
            Self::Listeners => "Listeners",
            Self::Publishers => "Publishers",
            Self::Trace => "Trace",
            Self::Queue => "Queue",
            Self::Statistics => "Statistics",
            Self::CodeGen => "Code Gen",
        }
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct EventStatistics {
    pub total_dispatched: u64,
    pub total_cancelled: u64,
    pub total_consumed: u64,
    pub total_dropped: u64,
    pub per_event_counts: HashMap<String, u64>,
    pub per_publisher_counts: HashMap<String, u64>,
    pub per_listener_counts: HashMap<String, u64>,
    pub peak_events_per_frame: u32,
    pub avg_events_per_frame: f32,
    pub avg_dispatch_time_us: f32,
    pub peak_queue_depth: usize,
    pub hottest_event: Option<String>,
}

impl EventStatistics {
    pub fn record_dispatch(&mut self, event: &str, publisher: &str, cancelled: bool, consumed: bool) {
        self.total_dispatched += 1;
        if cancelled { self.total_cancelled += 1; }
        if consumed { self.total_consumed += 1; }
        *self.per_event_counts.entry(event.to_string()).or_insert(0) += 1;
        *self.per_publisher_counts.entry(publisher.to_string()).or_insert(0) += 1;
        self.hottest_event = self.per_event_counts.iter()
            .max_by_key(|(_, &v)| v)
            .map(|(k, _)| k.clone());
    }
    pub fn top_events(&self, n: usize) -> Vec<(&String, &u64)> {
        let mut sorted: Vec<(&String, &u64)> = self.per_event_counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        sorted.into_iter().take(n).collect()
    }
    pub fn cancel_rate(&self) -> f32 {
        if self.total_dispatched == 0 { return 0.0; }
        self.total_cancelled as f32 / self.total_dispatched as f32
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodeGenLanguage { Rust, Cpp, CSharp, TypeScript, Python, Lua }

impl CodeGenLanguage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Rust => "Rust", Self::Cpp => "C++", Self::CSharp => "C#",
            Self::TypeScript => "TypeScript", Self::Python => "Python", Self::Lua => "Lua",
        }
    }
}

fn generate_rust_event_code(lib: &EventLibrary) -> String {
    let mut lines = vec![
        "// Auto-generated event types".to_string(),
        "use std::any::Any;".to_string(),
        "".to_string(),
        "pub trait Event: Any { fn name(&self) -> &'static str; }".to_string(),
        "".to_string(),
    ];
    for def in &lib.events {
        lines.push(format!("/// {}", def.description));
        lines.push(format!("pub struct {} {{", def.name));
        for param in &def.params {
            let rust_type = match param.param_type {
                EventParamType::Bool => "bool",
                EventParamType::Int => "i32",
                EventParamType::UInt => "u32",
                EventParamType::Float => "f32",
                EventParamType::Double => "f64",
                EventParamType::String => "String",
                EventParamType::Vec2 => "glam::Vec2",
                EventParamType::Vec3 => "glam::Vec3",
                EventParamType::Vec4 => "glam::Vec4",
                EventParamType::Quaternion => "glam::Quat",
                EventParamType::EntityId => "u64",
                EventParamType::AssetRef => "String",
                EventParamType::Color => "glam::Vec4",
                _ => "Vec<u8>",
            };
            let opt = if param.optional { format!("Option<{}>", rust_type) } else { rust_type.to_string() };
            lines.push(format!("    pub {}: {},", param.name, opt));
        }
        lines.push("}".to_string());
        lines.push(format!("impl Event for {} {{ fn name(&self) -> &'static str {{ \"{}\" }} }}", def.name, def.full_name()));
        lines.push("".to_string());
    }
    lines.join("\n")
}

fn generate_cpp_event_code(lib: &EventLibrary) -> String {
    let mut lines = vec![
        "// Auto-generated event types".to_string(),
        "#pragma once".to_string(),
        "#include <string>".to_string(),
        "#include <glm/glm.hpp>".to_string(),
        "".to_string(),
    ];
    for def in &lib.events {
        lines.push(format!("struct {} {{", def.name));
        for param in &def.params {
            let cpp_type = match param.param_type {
                EventParamType::Bool => "bool",
                EventParamType::Int => "int32_t",
                EventParamType::UInt => "uint32_t",
                EventParamType::Float => "float",
                EventParamType::Double => "double",
                EventParamType::String => "std::string",
                EventParamType::Vec2 => "glm::vec2",
                EventParamType::Vec3 => "glm::vec3",
                EventParamType::Vec4 => "glm::vec4",
                EventParamType::Quaternion => "glm::quat",
                EventParamType::EntityId => "uint64_t",
                _ => "std::vector<uint8_t>",
            };
            lines.push(format!("    {} {};", cpp_type, param.name));
        }
        lines.push(format!("    static constexpr const char* NAME = \"{}\";", def.full_name()));
        lines.push("};".to_string());
        lines.push("".to_string());
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Main event editor
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct EventSystemEditor {
    pub library: EventLibrary,
    pub graphs: Vec<EventGraph>,
    pub active_graph: usize,
    pub listeners: Vec<EventListener>,
    pub publishers: Vec<EventPublisher>,
    pub channels: Vec<EventChannel>,
    pub trace: EventTrace,
    pub queue_vis: EventQueueVisualizer,
    pub statistics: EventStatistics,
    pub active_panel: EventEditorPanel,
    pub selected_event: Option<u64>,
    pub selected_node: Option<u64>,
    pub selected_listener: Option<usize>,
    pub selected_publisher: Option<usize>,
    pub search_query: String,
    pub category_filter: Option<String>,
    pub codegen_language: CodeGenLanguage,
    pub generated_code: String,
    pub zoom: f32,
    pub pan: Vec2,
    pub simulation_time_ms: f64,
    pub auto_trace: bool,
    pub trace_max_entries: usize,
    pub show_cancelled: bool,
    pub show_consumed: bool,
    pub history: Vec<Vec<EventGraphNode>>,
    pub history_pos: usize,
}

impl Default for EventSystemEditor {
    fn default() -> Self {
        let library = EventLibrary::with_defaults();
        let graphs = vec![
            EventGraph::build_player_combat_graph(),
            EventGraph::new("UIEvents"),
            EventGraph::new("NetworkEvents"),
        ];
        let mut trace = EventTrace::new(2048);
        trace.recording = true;

        // Synthetic listeners
        let listeners = vec![
            EventListener::new(1, "AudioSystem.OnHit", 6, ListenerKind::Audio),
            EventListener::new(2, "VFXSystem.OnHit", 6, ListenerKind::VFX),
            EventListener::new(3, "UIHud.OnDamage", 6, ListenerKind::UI),
            EventListener::new(4, "QuestTracker.OnKill", 5, ListenerKind::System),
            EventListener::new(5, "AchievementSystem.OnDeath", 5, ListenerKind::System),
        ];

        let publishers = vec![
            EventPublisher::new(1, "WeaponSystem", ListenerKind::System),
            EventPublisher::new(2, "HealthSystem", ListenerKind::System),
            EventPublisher::new(3, "PlayerController", ListenerKind::Component),
            EventPublisher::new(4, "PhysicsSystem", ListenerKind::Physics),
        ];

        Self {
            library,
            graphs,
            active_graph: 0,
            listeners,
            publishers,
            channels: Vec::new(),
            trace,
            queue_vis: EventQueueVisualizer::default(),
            statistics: EventStatistics::default(),
            active_panel: EventEditorPanel::EventLibrary,
            selected_event: None,
            selected_node: None,
            selected_listener: None,
            selected_publisher: None,
            search_query: String::new(),
            category_filter: None,
            codegen_language: CodeGenLanguage::Rust,
            generated_code: String::new(),
            zoom: 1.0,
            pan: Vec2::ZERO,
            simulation_time_ms: 0.0,
            auto_trace: true,
            trace_max_entries: 2048,
            show_cancelled: true,
            show_consumed: true,
            history: Vec::new(),
            history_pos: 0,
        }
    }
}

impl EventSystemEditor {
    pub fn active_graph(&self) -> &EventGraph { &self.graphs[self.active_graph] }
    pub fn active_graph_mut(&mut self) -> &mut EventGraph { &mut self.graphs[self.active_graph] }

    pub fn simulate_tick(&mut self, dt_ms: f64) {
        self.simulation_time_ms += dt_ms;
        self.queue_vis.simulate_frame(self.simulation_time_ms);

        // Inject synthetic trace events
        if self.auto_trace && (self.simulation_time_ms as u64 % 16) == 0 {
            let events = ["OnHit", "OnPlayerMove", "OnFire", "OnTick", "OnCollision"];
            let idx = (self.simulation_time_ms as usize / 16) % events.len();
            self.trace.push(EventTraceEntry {
                timestamp_ms: self.simulation_time_ms,
                frame: self.queue_vis.frame,
                event_name: events[idx].to_string(),
                event_id: idx as u64 + 1,
                dispatch_mode: EventDispatchMode::Queued,
                priority: EventPriority::Normal,
                publisher: "GameSystem".to_string(),
                listener_count: 2,
                cancelled: false,
                consumed: false,
                dispatch_time_us: 0.5,
                payload_bytes: 16,
            });
            self.statistics.record_dispatch(events[idx], "GameSystem", false, false);
        }
    }

    pub fn generate_code(&mut self) {
        self.generated_code = match self.codegen_language {
            CodeGenLanguage::Rust => generate_rust_event_code(&self.library),
            CodeGenLanguage::Cpp => generate_cpp_event_code(&self.library),
            _ => "// Code generation not yet supported for this language".to_string(),
        };
    }

    pub fn filtered_events(&self) -> Vec<&EventDefinition> {
        let q = self.search_query.to_lowercase();
        self.library.events.iter().filter(|e| {
            let cat_ok = self.category_filter.as_ref().map_or(true, |f| &e.category == f);
            let text_ok = q.is_empty() || e.name.to_lowercase().contains(&q) || e.category.to_lowercase().contains(&q);
            cat_ok && text_ok
        }).collect()
    }

    pub fn listeners_for_event(&self, event_id: u64) -> Vec<&EventListener> {
        self.listeners.iter().filter(|l| l.event_id == event_id).collect()
    }

    pub fn snapshot(&mut self) {
        let nodes = self.active_graph().nodes.clone();
        self.history.truncate(self.history_pos);
        self.history.push(nodes);
        self.history_pos = self.history.len();
    }

    pub fn undo(&mut self) {
        if self.history_pos > 1 {
            self.history_pos -= 1;
            let nodes = self.history[self.history_pos - 1].clone();
            self.active_graph_mut().nodes = nodes;
        }
    }

    pub fn redo(&mut self) {
        if self.history_pos < self.history.len() {
            let nodes = self.history[self.history_pos].clone();
            self.active_graph_mut().nodes = nodes;
            self.history_pos += 1;
        }
    }
}
