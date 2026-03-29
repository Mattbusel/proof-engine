
//! Network editor — visual multiplayer graph, RPC system, replication settings,
//! lag simulation, bandwidth monitor, session browser, and lobby management.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Network topology
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkTopology {
    ClientServer,
    PeerToPeer,
    Dedicated,
    ListenServer,
    Relay,
    Hybrid,
}

impl NetworkTopology {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ClientServer => "Client-Server",
            Self::PeerToPeer => "Peer-to-Peer",
            Self::Dedicated => "Dedicated Server",
            Self::ListenServer => "Listen Server",
            Self::Relay => "Relay",
            Self::Hybrid => "Hybrid",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            Self::ClientServer => "Authoritative server with multiple clients",
            Self::PeerToPeer => "Direct peer connections, no central server",
            Self::Dedicated => "Headless dedicated server process",
            Self::ListenServer => "One client acts as host",
            Self::Relay => "All traffic routed through relay server",
            Self::Hybrid => "Mix of server-authority and P2P for low-latency data",
        }
    }
}

// ---------------------------------------------------------------------------
// Replication
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplicationMode {
    Unreliable,
    Reliable,
    ReliableOrdered,
    ReliableSequenced,
    Multicast,
    OwnerOnly,
    ServerOnly,
    Skip,
}

impl ReplicationMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Unreliable => "Unreliable",
            Self::Reliable => "Reliable",
            Self::ReliableOrdered => "Reliable Ordered",
            Self::ReliableSequenced => "Reliable Sequenced",
            Self::Multicast => "Multicast",
            Self::OwnerOnly => "Owner Only",
            Self::ServerOnly => "Server Only",
            Self::Skip => "No Replication",
        }
    }

    pub fn uses_sequence_numbers(&self) -> bool {
        matches!(self, Self::ReliableOrdered | Self::ReliableSequenced)
    }

    pub fn is_reliable(&self) -> bool {
        matches!(self, Self::Reliable | Self::ReliableOrdered | Self::ReliableSequenced)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplicatedPropertyType {
    Bool,
    Byte,
    Int32,
    Int64,
    Float,
    Double,
    Vector3,
    Quaternion,
    String,
    Transform,
    Color,
    ObjectRef,
    Custom,
}

impl ReplicatedPropertyType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Byte => "byte",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::Float => "float",
            Self::Double => "double",
            Self::Vector3 => "Vector3",
            Self::Quaternion => "Quaternion",
            Self::String => "string",
            Self::Transform => "Transform",
            Self::Color => "Color",
            Self::ObjectRef => "Object Reference",
            Self::Custom => "Custom",
        }
    }

    pub fn size_bytes(&self) -> usize {
        match self {
            Self::Bool | Self::Byte => 1,
            Self::Int32 | Self::Float => 4,
            Self::Int64 | Self::Double => 8,
            Self::Vector3 => 12,
            Self::Quaternion | Self::Color | Self::Transform => 16,
            Self::String => 64, // estimate
            Self::ObjectRef => 8,
            Self::Custom => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReplicatedProperty {
    pub name: String,
    pub prop_type: ReplicatedPropertyType,
    pub replication_mode: ReplicationMode,
    pub update_rate_hz: f32,
    pub send_on_change_only: bool,
    pub priority: f32,
    pub relevancy_distance: f32,
    pub use_delta_compression: bool,
    pub custom_serializer: bool,
    pub size_bytes: usize,
    pub dirty: bool,
    pub last_sent_frame: u64,
}

impl ReplicatedProperty {
    pub fn new(name: &str, prop_type: ReplicatedPropertyType) -> Self {
        let size_bytes = prop_type.size_bytes();
        Self {
            name: name.to_string(),
            prop_type,
            replication_mode: ReplicationMode::Unreliable,
            update_rate_hz: 30.0,
            send_on_change_only: true,
            priority: 1.0,
            relevancy_distance: 0.0,
            use_delta_compression: false,
            custom_serializer: false,
            size_bytes,
            dirty: false,
            last_sent_frame: 0,
        }
    }

    pub fn bits_per_second(&self) -> f32 {
        self.size_bytes as f32 * 8.0 * self.update_rate_hz
    }
}

// ---------------------------------------------------------------------------
// RPC system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RpcTarget {
    Server,
    AllClients,
    AllClientsIncludingSelf,
    OwnerClient,
    MulticastReliable,
    MulticastUnreliable,
}

impl RpcTarget {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Server => "Server",
            Self::AllClients => "All Clients",
            Self::AllClientsIncludingSelf => "All Clients (incl. Self)",
            Self::OwnerClient => "Owner Client",
            Self::MulticastReliable => "Multicast Reliable",
            Self::MulticastUnreliable => "Multicast Unreliable",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RpcParam {
    pub name: String,
    pub param_type: ReplicatedPropertyType,
}

#[derive(Debug, Clone)]
pub struct RpcDefinition {
    pub name: String,
    pub target: RpcTarget,
    pub params: Vec<RpcParam>,
    pub reliable: bool,
    pub requires_authority: bool,
    pub rate_limit_hz: f32,
    pub estimated_bytes: usize,
}

impl RpcDefinition {
    pub fn new(name: &str, target: RpcTarget) -> Self {
        Self {
            name: name.to_string(),
            target,
            params: Vec::new(),
            reliable: true,
            requires_authority: false,
            rate_limit_hz: 0.0,
            estimated_bytes: 8, // name + overhead
        }
    }

    pub fn add_param(mut self, name: &str, param_type: ReplicatedPropertyType) -> Self {
        self.estimated_bytes += param_type.size_bytes();
        self.params.push(RpcParam { name: name.to_string(), param_type });
        self
    }

    pub fn call_signature(&self) -> String {
        let params: Vec<String> = self.params.iter().map(|p| format!("{}: {}", p.name, p.param_type.label())).collect();
        format!("{}({}) -> {}", self.name, params.join(", "), self.target.label())
    }
}

// ---------------------------------------------------------------------------
// Replicated actor definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReplicatedActor {
    pub class_name: String,
    pub properties: Vec<ReplicatedProperty>,
    pub rpcs: Vec<RpcDefinition>,
    pub net_update_frequency: f32,
    pub net_cull_distance_squared: f32,
    pub bnet_use_owner_relevancy: bool,
    pub always_relevant: bool,
    pub relevancy_method: RelevancyMethod,
    pub bandwidth_bytes_per_sec: f32,
    pub priority: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RelevancyMethod {
    Distance,
    AlwaysRelevant,
    OwnerAndViewer,
    TeammatesOnly,
    Custom,
}

impl RelevancyMethod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Distance => "Distance",
            Self::AlwaysRelevant => "Always Relevant",
            Self::OwnerAndViewer => "Owner & Viewer",
            Self::TeammatesOnly => "Teammates Only",
            Self::Custom => "Custom",
        }
    }
}

impl ReplicatedActor {
    pub fn new(class_name: &str) -> Self {
        Self {
            class_name: class_name.to_string(),
            properties: Vec::new(),
            rpcs: Vec::new(),
            net_update_frequency: 30.0,
            net_cull_distance_squared: 150.0 * 150.0,
            bnet_use_owner_relevancy: false,
            always_relevant: false,
            relevancy_method: RelevancyMethod::Distance,
            bandwidth_bytes_per_sec: 0.0,
            priority: 1.0,
        }
    }

    pub fn with_property(mut self, prop: ReplicatedProperty) -> Self {
        self.bandwidth_bytes_per_sec += prop.bits_per_second() / 8.0;
        self.properties.push(prop);
        self
    }

    pub fn with_rpc(mut self, rpc: RpcDefinition) -> Self {
        self.rpcs.push(rpc);
        self
    }

    pub fn total_bandwidth_bps(&self) -> f32 {
        self.properties.iter().map(|p| p.bits_per_second()).sum()
    }

    pub fn generate_replication_code(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("// Replicated properties for {}", self.class_name));
        for prop in &self.properties {
            lines.push(format!("// [Replicated({:?})] pub {}: {},", prop.replication_mode, prop.name, prop.prop_type.label()));
        }
        lines.push(String::new());
        lines.push("// RPCs:".to_string());
        for rpc in &self.rpcs {
            lines.push(format!("// [RPC({:?})] fn {};", rpc.target, rpc.call_signature()));
        }
        lines.join("\n")
    }

    pub fn player_actor() -> Self {
        Self::new("PlayerController")
            .with_property({
                let mut p = ReplicatedProperty::new("position", ReplicatedPropertyType::Vector3);
                p.update_rate_hz = 60.0;
                p.use_delta_compression = true;
                p
            })
            .with_property({
                let mut p = ReplicatedProperty::new("rotation", ReplicatedPropertyType::Quaternion);
                p.update_rate_hz = 60.0;
                p
            })
            .with_property({
                let mut p = ReplicatedProperty::new("health", ReplicatedPropertyType::Float);
                p.replication_mode = ReplicationMode::Reliable;
                p.send_on_change_only = true;
                p
            })
            .with_property({
                let mut p = ReplicatedProperty::new("ammo", ReplicatedPropertyType::Int32);
                p.replication_mode = ReplicationMode::ReliableOrdered;
                p.send_on_change_only = true;
                p
            })
            .with_rpc(
                RpcDefinition::new("ServerFire", RpcTarget::Server)
                    .add_param("origin", ReplicatedPropertyType::Vector3)
                    .add_param("direction", ReplicatedPropertyType::Vector3)
            )
            .with_rpc(
                RpcDefinition::new("ClientPlayHitEffect", RpcTarget::OwnerClient)
                    .add_param("hit_position", ReplicatedPropertyType::Vector3)
            )
            .with_rpc(
                RpcDefinition::new("MulticastExplosion", RpcTarget::MulticastUnreliable)
                    .add_param("position", ReplicatedPropertyType::Vector3)
                    .add_param("radius", ReplicatedPropertyType::Float)
            )
    }
}

// ---------------------------------------------------------------------------
// Network simulation settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LagSimulation {
    pub enabled: bool,
    pub latency_ms: f32,
    pub jitter_ms: f32,
    pub packet_loss_pct: f32,
    pub duplication_pct: f32,
    pub reorder_pct: f32,
    pub bandwidth_kbps: u32,
    pub profile: LagProfile,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LagProfile {
    None,
    Good,
    Average,
    Poor,
    Terrible,
    Mobile3G,
    Mobile4G,
    Wifi,
    Custom,
}

impl LagProfile {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "No Lag",
            Self::Good => "Good (20ms)",
            Self::Average => "Average (60ms)",
            Self::Poor => "Poor (150ms)",
            Self::Terrible => "Terrible (300ms+)",
            Self::Mobile3G => "Mobile 3G",
            Self::Mobile4G => "Mobile 4G",
            Self::Wifi => "WiFi",
            Self::Custom => "Custom",
        }
    }

    pub fn apply_to_sim(&self, sim: &mut LagSimulation) {
        match self {
            Self::None => { sim.latency_ms = 0.0; sim.jitter_ms = 0.0; sim.packet_loss_pct = 0.0; sim.bandwidth_kbps = 100_000; }
            Self::Good => { sim.latency_ms = 20.0; sim.jitter_ms = 2.0; sim.packet_loss_pct = 0.0; sim.bandwidth_kbps = 10_000; }
            Self::Average => { sim.latency_ms = 60.0; sim.jitter_ms = 10.0; sim.packet_loss_pct = 0.5; sim.bandwidth_kbps = 5_000; }
            Self::Poor => { sim.latency_ms = 150.0; sim.jitter_ms = 30.0; sim.packet_loss_pct = 2.0; sim.bandwidth_kbps = 1_000; }
            Self::Terrible => { sim.latency_ms = 300.0; sim.jitter_ms = 80.0; sim.packet_loss_pct = 5.0; sim.bandwidth_kbps = 256; }
            Self::Mobile3G => { sim.latency_ms = 100.0; sim.jitter_ms = 20.0; sim.packet_loss_pct = 1.0; sim.bandwidth_kbps = 384; }
            Self::Mobile4G => { sim.latency_ms = 40.0; sim.jitter_ms = 8.0; sim.packet_loss_pct = 0.2; sim.bandwidth_kbps = 5_000; }
            Self::Wifi => { sim.latency_ms = 5.0; sim.jitter_ms = 5.0; sim.packet_loss_pct = 0.1; sim.bandwidth_kbps = 20_000; }
            Self::Custom => {}
        }
        sim.profile = *self;
    }
}

impl Default for LagSimulation {
    fn default() -> Self {
        Self {
            enabled: false,
            latency_ms: 0.0,
            jitter_ms: 0.0,
            packet_loss_pct: 0.0,
            duplication_pct: 0.0,
            reorder_pct: 0.0,
            bandwidth_kbps: 100_000,
            profile: LagProfile::None,
        }
    }
}

impl LagSimulation {
    pub fn rtt_ms(&self) -> f32 { self.latency_ms * 2.0 }
    pub fn bandwidth_bytes_per_sec(&self) -> f32 { self.bandwidth_kbps as f32 * 128.0 }
}

// ---------------------------------------------------------------------------
// Bandwidth monitor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct BandwidthSample {
    pub timestamp_ms: f64,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub packets_sent: u32,
    pub packets_recv: u32,
    pub packets_lost: u32,
    pub rtt_ms: f32,
}

#[derive(Debug, Clone)]
pub struct BandwidthMonitor {
    pub samples: Vec<BandwidthSample>,
    pub max_samples: usize,
    pub total_bytes_sent: u64,
    pub total_bytes_recv: u64,
    pub total_packets_lost: u64,
    pub session_start_ms: f64,
}

impl Default for BandwidthMonitor {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            max_samples: 512,
            total_bytes_sent: 0,
            total_bytes_recv: 0,
            total_packets_lost: 0,
            session_start_ms: 0.0,
        }
    }
}

impl BandwidthMonitor {
    pub fn push_sample(&mut self, sample: BandwidthSample) {
        self.total_bytes_sent += sample.bytes_sent;
        self.total_bytes_recv += sample.bytes_recv;
        self.total_packets_lost += sample.packets_lost as u64;
        self.samples.push(sample);
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    pub fn current_send_kbps(&self) -> f32 {
        if self.samples.len() < 2 { return 0.0; }
        let n = self.samples.len();
        let bytes = self.samples[n-1].bytes_sent;
        let dt_s = (self.samples[n-1].timestamp_ms - self.samples[n-2].timestamp_ms) / 1000.0;
        if dt_s <= 0.0 { return 0.0; }
        bytes as f32 / dt_s as f32 / 1024.0
    }

    pub fn current_recv_kbps(&self) -> f32 {
        if self.samples.len() < 2 { return 0.0; }
        let n = self.samples.len();
        let bytes = self.samples[n-1].bytes_recv;
        let dt_s = (self.samples[n-1].timestamp_ms - self.samples[n-2].timestamp_ms) / 1000.0;
        if dt_s <= 0.0 { return 0.0; }
        bytes as f32 / dt_s as f32 / 1024.0
    }

    pub fn avg_rtt_ms(&self) -> f32 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().map(|s| s.rtt_ms).sum::<f32>() / self.samples.len() as f32
    }

    pub fn packet_loss_pct(&self) -> f32 {
        let total_sent: u64 = self.samples.iter().map(|s| s.packets_sent as u64).sum();
        if total_sent == 0 { return 0.0; }
        self.total_packets_lost as f32 / total_sent as f32 * 100.0
    }

    pub fn generate_synthetic_samples(&mut self, count: usize) {
        let mut rng_seed: u64 = 42;
        for i in 0..count {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let noise = ((rng_seed >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
            self.push_sample(BandwidthSample {
                timestamp_ms: i as f64 * 16.666,
                bytes_sent: (1024 + (noise * 200.0) as i64).max(0) as u64,
                bytes_recv: (512 + (noise * 100.0) as i64).max(0) as u64,
                packets_sent: 8,
                packets_recv: 8,
                packets_lost: if noise > 0.9 { 1 } else { 0 },
                rtt_ms: 45.0 + noise * 10.0,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Session and lobby
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    Creating,
    Searching,
    Joining,
    InLobby,
    InGame,
    Ending,
    Disconnected,
}

impl SessionState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Creating => "Creating",
            Self::Searching => "Searching",
            Self::Joining => "Joining",
            Self::InLobby => "In Lobby",
            Self::InGame => "In Game",
            Self::Ending => "Ending",
            Self::Disconnected => "Disconnected",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkPlayer {
    pub player_id: u64,
    pub display_name: String,
    pub ping_ms: u32,
    pub is_host: bool,
    pub is_local: bool,
    pub is_ready: bool,
    pub team: Option<u32>,
    pub connection_quality: ConnectionQuality,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub packet_loss_pct: f32,
    pub load_pct: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
    Disconnecting,
}

impl ConnectionQuality {
    pub fn from_ping(ping_ms: u32) -> Self {
        match ping_ms {
            0..=40 => Self::Excellent,
            41..=80 => Self::Good,
            81..=150 => Self::Fair,
            151..=300 => Self::Poor,
            _ => Self::Disconnecting,
        }
    }

    pub fn color(&self) -> Vec4 {
        match self {
            Self::Excellent => Vec4::new(0.2, 0.9, 0.2, 1.0),
            Self::Good => Vec4::new(0.6, 0.9, 0.2, 1.0),
            Self::Fair => Vec4::new(0.9, 0.8, 0.1, 1.0),
            Self::Poor => Vec4::new(0.9, 0.4, 0.1, 1.0),
            Self::Disconnecting => Vec4::new(0.9, 0.1, 0.1, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkSession {
    pub session_id: String,
    pub session_name: String,
    pub topology: NetworkTopology,
    pub state: SessionState,
    pub max_players: u32,
    pub players: Vec<NetworkPlayer>,
    pub map_name: String,
    pub game_mode: String,
    pub is_private: bool,
    pub server_region: String,
    pub server_ip: String,
    pub server_port: u16,
    pub tick_rate: u32,
    pub uptime_secs: f64,
}

impl NetworkSession {
    pub fn new_lobby(name: &str) -> Self {
        Self {
            session_id: format!("session_{}", name.to_lowercase().replace(' ', "_")),
            session_name: name.to_string(),
            topology: NetworkTopology::Dedicated,
            state: SessionState::InLobby,
            max_players: 16,
            players: Vec::new(),
            map_name: "MainMap".to_string(),
            game_mode: "Team Deathmatch".to_string(),
            is_private: false,
            server_region: "us-east-1".to_string(),
            server_ip: "192.168.1.100".to_string(),
            server_port: 7777,
            tick_rate: 60,
            uptime_secs: 0.0,
        }
    }

    pub fn player_count(&self) -> u32 { self.players.len() as u32 }
    pub fn is_full(&self) -> bool { self.player_count() >= self.max_players }

    pub fn average_ping(&self) -> f32 {
        if self.players.is_empty() { return 0.0; }
        self.players.iter().map(|p| p.ping_ms as f32).sum::<f32>() / self.players.len() as f32
    }

    pub fn host_player(&self) -> Option<&NetworkPlayer> {
        self.players.iter().find(|p| p.is_host)
    }

    pub fn all_ready(&self) -> bool {
        self.players.iter().filter(|p| !p.is_host).all(|p| p.is_ready)
    }

    pub fn total_bandwidth_kbps(&self) -> f32 {
        self.players.iter().map(|p| (p.bytes_sent + p.bytes_recv) as f32 / 1024.0).sum()
    }
}

// ---------------------------------------------------------------------------
// Network node graph (visual routing)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetNodeKind {
    Client,
    Server,
    Relay,
    LoadBalancer,
    Database,
    Matchmaker,
    AuthServer,
    VoiceServer,
    Metrics,
    Gateway,
}

impl NetNodeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Client => "Client",
            Self::Server => "Server",
            Self::Relay => "Relay",
            Self::LoadBalancer => "Load Balancer",
            Self::Database => "Database",
            Self::Matchmaker => "Matchmaker",
            Self::AuthServer => "Auth Server",
            Self::VoiceServer => "Voice Server",
            Self::Metrics => "Metrics",
            Self::Gateway => "Gateway",
        }
    }

    pub fn color(&self) -> Vec4 {
        match self {
            Self::Client => Vec4::new(0.3, 0.6, 0.9, 1.0),
            Self::Server => Vec4::new(0.9, 0.6, 0.2, 1.0),
            Self::Relay => Vec4::new(0.5, 0.8, 0.5, 1.0),
            Self::LoadBalancer => Vec4::new(0.8, 0.5, 0.8, 1.0),
            Self::Database => Vec4::new(0.8, 0.4, 0.3, 1.0),
            Self::Matchmaker => Vec4::new(0.4, 0.8, 0.8, 1.0),
            Self::AuthServer => Vec4::new(0.8, 0.8, 0.3, 1.0),
            Self::VoiceServer => Vec4::new(0.6, 0.6, 0.9, 1.0),
            Self::Metrics => Vec4::new(0.6, 0.9, 0.6, 1.0),
            Self::Gateway => Vec4::new(0.9, 0.7, 0.5, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetNode {
    pub id: u64,
    pub kind: NetNodeKind,
    pub position: Vec2,
    pub size: Vec2,
    pub name: String,
    pub region: String,
    pub instance_count: u32,
    pub player_count: u32,
    pub cpu_usage: f32,
    pub mem_usage_mb: f32,
    pub bandwidth_kbps: f32,
    pub status_ok: bool,
}

impl NetNode {
    pub fn new(id: u64, kind: NetNodeKind, position: Vec2) -> Self {
        Self {
            id,
            kind,
            position,
            size: Vec2::new(140.0, 60.0),
            name: kind.label().to_string(),
            region: "us-east-1".to_string(),
            instance_count: 1,
            player_count: 0,
            cpu_usage: 0.0,
            mem_usage_mb: 0.0,
            bandwidth_kbps: 0.0,
            status_ok: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetEdge {
    pub id: u64,
    pub from_id: u64,
    pub to_id: u64,
    pub protocol: NetworkProtocol,
    pub latency_ms: f32,
    pub bandwidth_kbps: f32,
    pub encrypted: bool,
    pub bidirectional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkProtocol {
    Udp,
    Tcp,
    WebSocket,
    WebRtc,
    Quic,
    Rudp,
}

impl NetworkProtocol {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Udp => "UDP",
            Self::Tcp => "TCP",
            Self::WebSocket => "WebSocket",
            Self::WebRtc => "WebRTC",
            Self::Quic => "QUIC",
            Self::Rudp => "RUDP",
        }
    }
    pub fn is_reliable(&self) -> bool {
        matches!(self, Self::Tcp | Self::WebSocket | Self::Quic)
    }
}

#[derive(Debug, Clone)]
pub struct NetworkTopologyGraph {
    pub nodes: Vec<NetNode>,
    pub edges: Vec<NetEdge>,
    pub next_id: u64,
}

impl Default for NetworkTopologyGraph {
    fn default() -> Self {
        let mut g = Self { nodes: Vec::new(), edges: Vec::new(), next_id: 1 };
        g.build_typical_dedicated_topology();
        g
    }
}

impl NetworkTopologyGraph {
    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_node(&mut self, kind: NetNodeKind, pos: Vec2, name: &str) -> u64 {
        let id = self.next_id();
        let mut node = NetNode::new(id, kind, pos);
        node.name = name.to_string();
        self.nodes.push(node);
        id
    }

    pub fn add_edge(&mut self, from: u64, to: u64, protocol: NetworkProtocol) -> u64 {
        let id = self.next_id();
        self.edges.push(NetEdge {
            id,
            from_id: from,
            to_id: to,
            protocol,
            latency_ms: 20.0,
            bandwidth_kbps: 10_000.0,
            encrypted: true,
            bidirectional: true,
        });
        id
    }

    pub fn build_typical_dedicated_topology(&mut self) {
        let gw = self.add_node(NetNodeKind::Gateway, Vec2::new(400.0, 20.0), "CDN Gateway");
        let auth = self.add_node(NetNodeKind::AuthServer, Vec2::new(100.0, 120.0), "Auth Service");
        let match_ = self.add_node(NetNodeKind::Matchmaker, Vec2::new(400.0, 120.0), "Matchmaker");
        let lb = self.add_node(NetNodeKind::LoadBalancer, Vec2::new(700.0, 120.0), "Load Balancer");
        let db = self.add_node(NetNodeKind::Database, Vec2::new(100.0, 260.0), "Game DB");
        let srv1 = self.add_node(NetNodeKind::Server, Vec2::new(550.0, 260.0), "Game Server 1");
        let srv2 = self.add_node(NetNodeKind::Server, Vec2::new(700.0, 260.0), "Game Server 2");
        let srv3 = self.add_node(NetNodeKind::Server, Vec2::new(850.0, 260.0), "Game Server 3");
        let voice = self.add_node(NetNodeKind::VoiceServer, Vec2::new(250.0, 260.0), "Voice Chat");
        let metrics = self.add_node(NetNodeKind::Metrics, Vec2::new(400.0, 260.0), "Telemetry");
        let cli1 = self.add_node(NetNodeKind::Client, Vec2::new(400.0, 400.0), "Client A");
        let cli2 = self.add_node(NetNodeKind::Client, Vec2::new(550.0, 400.0), "Client B");
        let cli3 = self.add_node(NetNodeKind::Client, Vec2::new(700.0, 400.0), "Client C");

        self.add_edge(cli1, gw, NetworkProtocol::Quic);
        self.add_edge(cli2, gw, NetworkProtocol::Quic);
        self.add_edge(cli3, gw, NetworkProtocol::Quic);
        self.add_edge(gw, auth, NetworkProtocol::Tcp);
        self.add_edge(gw, match_, NetworkProtocol::Tcp);
        self.add_edge(match_, lb, NetworkProtocol::Tcp);
        self.add_edge(lb, srv1, NetworkProtocol::Udp);
        self.add_edge(lb, srv2, NetworkProtocol::Udp);
        self.add_edge(lb, srv3, NetworkProtocol::Udp);
        self.add_edge(auth, db, NetworkProtocol::Tcp);
        self.add_edge(srv1, db, NetworkProtocol::Tcp);
        self.add_edge(srv1, metrics, NetworkProtocol::Udp);
        self.add_edge(cli1, voice, NetworkProtocol::WebRtc);
        self.add_edge(cli2, voice, NetworkProtocol::WebRtc);
    }
}

// ---------------------------------------------------------------------------
// Network editor panels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkEditorPanel {
    Topology,
    Replication,
    BandwidthMonitor,
    LagSimulation,
    Lobby,
    RpcBrowser,
    Diagnostics,
}

impl NetworkEditorPanel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Topology => "Topology",
            Self::Replication => "Replication",
            Self::BandwidthMonitor => "Bandwidth",
            Self::LagSimulation => "Lag Sim",
            Self::Lobby => "Lobby",
            Self::RpcBrowser => "RPCs",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

// ---------------------------------------------------------------------------
// Diagnostic events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkEventKind {
    PlayerConnected,
    PlayerDisconnected,
    PacketLost,
    PacketReordered,
    PacketDuplicated,
    BandwidthThrottled,
    HighLatency,
    TimeOut,
    Reconnected,
    RpcDropped,
    AuthFailed,
    SessionCreated,
    SessionDestroyed,
    LevelLoad,
}

impl NetworkEventKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PlayerConnected => "Player Connected",
            Self::PlayerDisconnected => "Player Disconnected",
            Self::PacketLost => "Packet Lost",
            Self::PacketReordered => "Packet Reordered",
            Self::PacketDuplicated => "Packet Duplicated",
            Self::BandwidthThrottled => "Bandwidth Throttled",
            Self::HighLatency => "High Latency",
            Self::TimeOut => "Time Out",
            Self::Reconnected => "Reconnected",
            Self::RpcDropped => "RPC Dropped",
            Self::AuthFailed => "Auth Failed",
            Self::SessionCreated => "Session Created",
            Self::SessionDestroyed => "Session Destroyed",
            Self::LevelLoad => "Level Load",
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::PacketLost | Self::TimeOut | Self::RpcDropped | Self::AuthFailed)
    }

    pub fn color(&self) -> Vec4 {
        if self.is_error() {
            Vec4::new(0.9, 0.3, 0.3, 1.0)
        } else if matches!(self, Self::PlayerConnected | Self::SessionCreated | Self::Reconnected) {
            Vec4::new(0.3, 0.9, 0.3, 1.0)
        } else {
            Vec4::new(0.7, 0.7, 0.7, 1.0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkDiagnosticEvent {
    pub timestamp_ms: f64,
    pub kind: NetworkEventKind,
    pub player_id: Option<u64>,
    pub description: String,
    pub data: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Network editor state
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct NetworkEditor {
    pub topology_graph: NetworkTopologyGraph,
    pub session: NetworkSession,
    pub actors: Vec<ReplicatedActor>,
    pub bandwidth_monitor: BandwidthMonitor,
    pub lag_sim: LagSimulation,
    pub active_panel: NetworkEditorPanel,
    pub selected_node: Option<u64>,
    pub selected_actor: Option<usize>,
    pub diagnostic_events: Vec<NetworkDiagnosticEvent>,
    pub simulation_time_ms: f64,
    pub network_paused: bool,
    pub zoom: f32,
    pub pan: Vec2,
    pub search_query: String,
    pub show_bandwidth_overlay: bool,
    pub show_player_list: bool,
}

impl Default for NetworkEditor {
    fn default() -> Self {
        let mut bw_monitor = BandwidthMonitor::default();
        bw_monitor.generate_synthetic_samples(256);

        let session = {
            let mut s = NetworkSession::new_lobby("Dev Session");
            s.players = vec![
                NetworkPlayer {
                    player_id: 1,
                    display_name: "HostPlayer".to_string(),
                    ping_ms: 5,
                    is_host: true,
                    is_local: true,
                    is_ready: true,
                    team: Some(0),
                    connection_quality: ConnectionQuality::Excellent,
                    bytes_sent: 12_000,
                    bytes_recv: 8_000,
                    packet_loss_pct: 0.0,
                    load_pct: 0.0,
                },
                NetworkPlayer {
                    player_id: 2,
                    display_name: "Player2".to_string(),
                    ping_ms: 48,
                    is_host: false,
                    is_local: false,
                    is_ready: true,
                    team: Some(0),
                    connection_quality: ConnectionQuality::Good,
                    bytes_sent: 11_000,
                    bytes_recv: 9_000,
                    packet_loss_pct: 0.1,
                    load_pct: 0.0,
                },
                NetworkPlayer {
                    player_id: 3,
                    display_name: "Player3".to_string(),
                    ping_ms: 142,
                    is_host: false,
                    is_local: false,
                    is_ready: false,
                    team: Some(1),
                    connection_quality: ConnectionQuality::Fair,
                    bytes_sent: 9_000,
                    bytes_recv: 7_500,
                    packet_loss_pct: 0.8,
                    load_pct: 0.0,
                },
            ];
            s
        };

        let actors = vec![
            ReplicatedActor::player_actor(),
            {
                let mut a = ReplicatedActor::new("Projectile");
                a.properties.push({
                    let mut p = ReplicatedProperty::new("position", ReplicatedPropertyType::Vector3);
                    p.update_rate_hz = 60.0;
                    p
                });
                a.properties.push({
                    let mut p = ReplicatedProperty::new("velocity", ReplicatedPropertyType::Vector3);
                    p.update_rate_hz = 60.0;
                    p
                });
                a.always_relevant = true;
                a
            },
        ];

        Self {
            topology_graph: NetworkTopologyGraph::default(),
            session,
            actors,
            bandwidth_monitor: bw_monitor,
            lag_sim: LagSimulation::default(),
            active_panel: NetworkEditorPanel::Topology,
            selected_node: None,
            selected_actor: None,
            diagnostic_events: Vec::new(),
            simulation_time_ms: 0.0,
            network_paused: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
            search_query: String::new(),
            show_bandwidth_overlay: true,
            show_player_list: true,
        }
    }
}

impl NetworkEditor {
    pub fn simulate_tick(&mut self, dt_ms: f64) {
        if self.network_paused { return; }
        self.simulation_time_ms += dt_ms;
        // push synthetic bandwidth sample
        let mut rng: u64 = self.simulation_time_ms as u64 * 2654435761;
        rng ^= rng >> 33;
        rng = rng.wrapping_mul(0xFF51AFD7ED558CCD);
        let noise = ((rng >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
        self.bandwidth_monitor.push_sample(BandwidthSample {
            timestamp_ms: self.simulation_time_ms,
            bytes_sent: (1024 + (noise * 256.0) as i64).max(0) as u64,
            bytes_recv: (512 + (noise * 128.0) as i64).max(0) as u64,
            packets_sent: 8,
            packets_recv: 8,
            packets_lost: if noise > 0.95 { 1 } else { 0 },
            rtt_ms: self.lag_sim.rtt_ms() + noise * self.lag_sim.jitter_ms,
        });
    }

    pub fn apply_lag_profile(&mut self, profile: LagProfile) {
        profile.apply_to_sim(&mut self.lag_sim);
    }

    pub fn total_replicated_bandwidth_bps(&self) -> f32 {
        self.actors.iter().map(|a| a.total_bandwidth_bps()).sum()
    }

    pub fn generate_network_report(&self) -> String {
        let mut lines = Vec::new();
        lines.push("=== Network Report ===".to_string());
        lines.push(format!("Topology: {}", self.session.topology.label()));
        lines.push(format!("State: {}", self.session.state.label()));
        lines.push(format!("Players: {}/{}", self.session.player_count(), self.session.max_players));
        lines.push(format!("Avg Ping: {:.1} ms", self.session.average_ping()));
        lines.push(format!("Bandwidth Send: {:.1} kB/s", self.bandwidth_monitor.current_send_kbps()));
        lines.push(format!("Bandwidth Recv: {:.1} kB/s", self.bandwidth_monitor.current_recv_kbps()));
        lines.push(format!("Packet Loss: {:.2}%", self.bandwidth_monitor.packet_loss_pct()));
        lines.push(format!("Lag Sim: {} (latency: {:.0} ms, loss: {:.1}%)",
            if self.lag_sim.enabled { "ON" } else { "OFF" },
            self.lag_sim.latency_ms,
            self.lag_sim.packet_loss_pct,
        ));
        lines.push(format!("Replicated Actors: {}", self.actors.len()));
        lines.push(format!("Total Replication Bandwidth: {:.0} b/s", self.total_replicated_bandwidth_bps()));
        lines.join("\n")
    }
}
