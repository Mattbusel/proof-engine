//! Dedicated game server framework for Proof Engine multiplayer.
//!
//! Provides tick-based authoritative server logic: client registry,
//! input processing, world snapshots, delta updates, chat, and kick/ban.

use std::collections::{HashMap, VecDeque};

// ── Newtype IDs ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);

impl ServerId {
    pub fn new(id: u64) -> Self { ServerId(id) }
    pub fn inner(self) -> u64 { self.0 }
}

impl ClientId {
    pub fn new(id: u64) -> Self { ClientId(id) }
    pub fn inner(self) -> u64 { self.0 }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client({})", self.0)
    }
}

// ── ServerConfig ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub max_clients:      u32,
    pub tick_rate:        u32,  // ticks per second
    pub timeout_ms:       u64,
    pub max_message_size: u32,
    pub compression:      bool,
    pub snapshot_interval: u32, // full snapshot every N ticks; deltas in between
    pub max_rewind_frames: u32,
    pub heartbeat_interval_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            max_clients:          64,
            tick_rate:            20,
            timeout_ms:           10_000,
            max_message_size:     65_536,
            compression:          true,
            snapshot_interval:    10,
            max_rewind_frames:    32,
            heartbeat_interval_ms: 2_000,
        }
    }
}

// ── ClientState ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientState {
    Connecting,
    Connected,
    Authenticating,
    Playing,
    Spectating,
    Disconnecting,
}

impl ClientState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClientState::Connecting     => "Connecting",
            ClientState::Connected      => "Connected",
            ClientState::Authenticating => "Authenticating",
            ClientState::Playing        => "Playing",
            ClientState::Spectating     => "Spectating",
            ClientState::Disconnecting  => "Disconnecting",
        }
    }

    pub fn can_receive_game_data(&self) -> bool {
        matches!(self, ClientState::Playing | ClientState::Spectating)
    }
}

// ── ChatChannel ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatChannel {
    Global,
    Team,
    Whisper(String),
    System,
}

// ── DisconnectReason ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DisconnectReason {
    Timeout,
    Kicked,
    Banned,
    ServerShutdown,
    ClientRequest,
    Error(String),
}

impl DisconnectReason {
    pub fn as_str(&self) -> &str {
        match self {
            DisconnectReason::Timeout        => "Timeout",
            DisconnectReason::Kicked         => "Kicked",
            DisconnectReason::Banned         => "Banned",
            DisconnectReason::ServerShutdown => "ServerShutdown",
            DisconnectReason::ClientRequest  => "ClientRequest",
            DisconnectReason::Error(s)       => s.as_str(),
        }
    }
}

// ── EntitySnapshot ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EntitySnapshot {
    pub id:    u64,
    pub pos:   [f32; 3],
    pub rot:   [f32; 4],
    pub vel:   [f32; 3],
    pub flags: u32,
}

impl EntitySnapshot {
    pub fn new(id: u64) -> Self {
        EntitySnapshot {
            id,
            pos:   [0.0; 3],
            rot:   [0.0, 0.0, 0.0, 1.0],
            vel:   [0.0; 3],
            flags: 0,
        }
    }

    /// Linear interpolation between two snapshots.
    pub fn lerp(&self, other: &EntitySnapshot, t: f32) -> EntitySnapshot {
        let lerp_f32 = |a: f32, b: f32| a + t * (b - a);
        EntitySnapshot {
            id: self.id,
            pos: [
                lerp_f32(self.pos[0], other.pos[0]),
                lerp_f32(self.pos[1], other.pos[1]),
                lerp_f32(self.pos[2], other.pos[2]),
            ],
            rot: [
                lerp_f32(self.rot[0], other.rot[0]),
                lerp_f32(self.rot[1], other.rot[1]),
                lerp_f32(self.rot[2], other.rot[2]),
                lerp_f32(self.rot[3], other.rot[3]),
            ],
            vel: [
                lerp_f32(self.vel[0], other.vel[0]),
                lerp_f32(self.vel[1], other.vel[1]),
                lerp_f32(self.vel[2], other.vel[2]),
            ],
            flags: self.flags,
        }
    }

    /// Returns true if position differs by more than epsilon.
    pub fn pos_differs(&self, other: &EntitySnapshot, eps: f32) -> bool {
        let dx = self.pos[0] - other.pos[0];
        let dy = self.pos[1] - other.pos[1];
        let dz = self.pos[2] - other.pos[2];
        dx * dx + dy * dy + dz * dz > eps * eps
    }
}

// ── EntityDelta ───────────────────────────────────────────────────────────────

/// Bitmask constants for EntityDelta.changed_fields
pub struct EntityField;
impl EntityField {
    pub const POS:   u32 = 1 << 0;
    pub const ROT:   u32 = 1 << 1;
    pub const VEL:   u32 = 1 << 2;
    pub const FLAGS: u32 = 1 << 3;
}

#[derive(Debug, Clone)]
pub struct EntityDelta {
    pub id:             u64,
    pub changed_fields: u32,
    pub data:           Vec<u8>,
}

impl EntityDelta {
    pub fn from_snapshots(prev: &EntitySnapshot, curr: &EntitySnapshot, eps: f32) -> EntityDelta {
        let mut changed = 0u32;
        let mut data = Vec::new();

        let pos_diff = {
            let dx = curr.pos[0] - prev.pos[0];
            let dy = curr.pos[1] - prev.pos[1];
            let dz = curr.pos[2] - prev.pos[2];
            dx * dx + dy * dy + dz * dz
        };
        if pos_diff > eps * eps {
            changed |= EntityField::POS;
            for &v in &curr.pos { data.extend_from_slice(&v.to_le_bytes()); }
        }

        let rot_diff = {
            let d: f32 = curr.rot.iter().zip(prev.rot.iter()).map(|(a, b)| (a - b).abs()).sum();
            d
        };
        if rot_diff > eps {
            changed |= EntityField::ROT;
            for &v in &curr.rot { data.extend_from_slice(&v.to_le_bytes()); }
        }

        let vel_diff = {
            let dx = curr.vel[0] - prev.vel[0];
            let dy = curr.vel[1] - prev.vel[1];
            let dz = curr.vel[2] - prev.vel[2];
            dx * dx + dy * dy + dz * dz
        };
        if vel_diff > eps * eps {
            changed |= EntityField::VEL;
            for &v in &curr.vel { data.extend_from_slice(&v.to_le_bytes()); }
        }

        if curr.flags != prev.flags {
            changed |= EntityField::FLAGS;
            data.extend_from_slice(&curr.flags.to_le_bytes());
        }

        EntityDelta { id: curr.id, changed_fields: changed, data }
    }

    pub fn has_changes(&self) -> bool {
        self.changed_fields != 0
    }
}

// ── PlayerInput ───────────────────────────────────────────────────────────────

/// Input buttons bitmask constants
pub struct InputButton;
impl InputButton {
    pub const JUMP:    u32 = 1 << 0;
    pub const CROUCH:  u32 = 1 << 1;
    pub const FIRE:    u32 = 1 << 2;
    pub const ALT_FIRE:u32 = 1 << 3;
    pub const INTERACT:u32 = 1 << 4;
    pub const SPRINT:  u32 = 1 << 5;
    pub const RELOAD:  u32 = 1 << 6;
    pub const USE:     u32 = 1 << 7;
}

#[derive(Debug, Clone)]
pub struct PlayerInput {
    pub frame:   u64,
    pub buttons: u32,
    pub axis_x:  f32,
    pub axis_y:  f32,
    pub yaw:     f32,
    pub pitch:   f32,
}

impl PlayerInput {
    pub fn button_held(&self, mask: u32) -> bool {
        self.buttons & mask != 0
    }
    pub fn is_jumping(&self)  -> bool { self.button_held(InputButton::JUMP) }
    pub fn is_crouching(&self)-> bool { self.button_held(InputButton::CROUCH) }
    pub fn is_firing(&self)   -> bool { self.button_held(InputButton::FIRE) }
    pub fn is_sprinting(&self)-> bool { self.button_held(InputButton::SPRINT) }
}

// ── InputHistory ──────────────────────────────────────────────────────────────

const INPUT_HISTORY_SIZE: usize = 64;

/// Per-client ring buffer of the last 64 frames of input.
pub struct InputHistory {
    buffer:    [Option<PlayerInput>; INPUT_HISTORY_SIZE],
    write_pos: usize,
    count:     usize,
}

impl InputHistory {
    pub fn new() -> Self {
        InputHistory {
            buffer:    std::array::from_fn(|_| None),
            write_pos: 0,
            count:     0,
        }
    }

    pub fn push(&mut self, input: PlayerInput) {
        self.buffer[self.write_pos] = Some(input);
        self.write_pos = (self.write_pos + 1) % INPUT_HISTORY_SIZE;
        if self.count < INPUT_HISTORY_SIZE { self.count += 1; }
    }

    pub fn get_by_frame(&self, frame: u64) -> Option<&PlayerInput> {
        for slot in &self.buffer {
            if let Some(inp) = slot {
                if inp.frame == frame { return Some(inp); }
            }
        }
        None
    }

    pub fn latest(&self) -> Option<&PlayerInput> {
        if self.count == 0 { return None; }
        let idx = if self.write_pos == 0 {
            INPUT_HISTORY_SIZE - 1
        } else {
            self.write_pos - 1
        };
        self.buffer[idx].as_ref()
    }

    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
}

impl Default for InputHistory {
    fn default() -> Self { Self::new() }
}

// ── ClientInfo ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub id:              ClientId,
    pub address:         String,
    pub connected_at:    f32,
    pub last_heartbeat:  f32,
    pub ping_ms:         u32,
    pub state:           ClientState,
    pub player_id:       Option<String>,
}

impl ClientInfo {
    pub fn new(id: ClientId, address: String, time: f32) -> Self {
        ClientInfo {
            id,
            address,
            connected_at:   time,
            last_heartbeat:  time,
            ping_ms:         0,
            state:           ClientState::Connecting,
            player_id:       None,
        }
    }

    pub fn is_timed_out(&self, current_time: f32, timeout_ms: u64) -> bool {
        let timeout_secs = timeout_ms as f32 / 1000.0;
        current_time - self.last_heartbeat > timeout_secs
    }

    pub fn update_heartbeat(&mut self, time: f32, rtt_ms: u32) {
        self.last_heartbeat = time;
        self.ping_ms = rtt_ms / 2;
    }
}

// ── ServerMessage ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ServerMessage {
    WorldSnapshot {
        frame:    u64,
        entities: Vec<EntitySnapshot>,
    },
    DeltaUpdate {
        frame:   u64,
        changes: Vec<EntityDelta>,
    },
    PlayerJoined {
        client_id: u64,
        player_id: String,
    },
    PlayerLeft {
        client_id: u64,
        reason:    DisconnectReason,
    },
    ChatMessage {
        sender:  String,
        text:    String,
        channel: ChatChannel,
    },
    GameEvent {
        event_type: String,
        payload:    Vec<u8>,
    },
    Heartbeat {
        server_time: f64,
    },
    ConnectionAccepted {
        client_id:   u64,
        server_time: f64,
    },
    ConnectionRejected {
        reason: String,
    },
    KickNotice {
        reason: DisconnectReason,
    },
}

// ── ClientMessage ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ClientMessage {
    PlayerInput {
        frame:   u64,
        buttons: u32,
        axis_x:  f32,
        axis_y:  f32,
        yaw:     f32,
        pitch:   f32,
    },
    ChatMessage {
        text:    String,
        channel: ChatChannel,
    },
    RequestRespawn,
    Acknowledge {
        frame: u64,
    },
    Connect {
        player_id: String,
        auth_token: String,
        version:    u32,
    },
    Disconnect,
    HeartbeatReply {
        client_time: f64,
    },
}

// ── BanRecord ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BanRecord {
    pub address:      String,
    pub player_id:    Option<String>,
    pub banned_at:    f32,
    pub duration_secs: f64,
    pub reason:       String,
}

impl BanRecord {
    pub fn is_expired(&self, current_time: f32) -> bool {
        if self.duration_secs < 0.0 { return false; } // permanent
        let end = self.banned_at as f64 + self.duration_secs;
        current_time as f64 > end
    }
}

// ── OutboundMessage ───────────────────────────────────────────────────────────

#[derive(Debug)]
struct OutboundMessage {
    target:  MessageTarget,
    message: ServerMessage,
}

#[derive(Debug)]
enum MessageTarget {
    All,
    Single(ClientId),
    Except(ClientId),
    Group(Vec<ClientId>),
}

// ── ServerStats ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ServerStats {
    pub total_clients_ever:  u64,
    pub messages_sent:       u64,
    pub messages_received:   u64,
    pub snapshots_sent:       u64,
    pub deltas_sent:          u64,
    pub bytes_out:            u64,
    pub bytes_in:             u64,
    pub ticks_processed:      u64,
    pub average_tick_ms:      f32,
    pub peak_clients:         u32,
}

// ── GameServer ────────────────────────────────────────────────────────────────

/// Authoritative multiplayer game server.
///
/// Call [`GameServer::tick`] every frame from the engine's main loop.
/// Messages are queued and dispatched synchronously — no async runtime needed.
pub struct GameServer {
    pub id:      ServerId,
    pub config:  ServerConfig,

    clients:       HashMap<ClientId, ClientInfo>,
    input_history: HashMap<ClientId, InputHistory>,

    /// Outbound message queue flushed each tick
    outbound:      Vec<OutboundMessage>,

    /// Inbound messages delivered by transport layer before tick
    inbound:       VecDeque<(ClientId, ClientMessage)>,

    current_frame: u64,
    server_time:   f64,

    /// Previous frame entity snapshots for delta computation
    prev_snapshot: HashMap<u64, EntitySnapshot>,

    /// Current frame entity snapshots (set by the simulation layer)
    curr_snapshot: HashMap<u64, EntitySnapshot>,

    ban_list:    Vec<BanRecord>,
    next_client_id: u64,

    pub stats: ServerStats,

    /// Pending disconnects processed at end of tick
    pending_disconnects: Vec<(ClientId, DisconnectReason)>,

    /// Per-client acknowledged frame — used to determine delta base
    client_acked_frame: HashMap<ClientId, u64>,
}

impl GameServer {
    pub fn new(id: ServerId, config: ServerConfig) -> Self {
        GameServer {
            id,
            config,
            clients:              HashMap::new(),
            input_history:        HashMap::new(),
            outbound:             Vec::new(),
            inbound:              VecDeque::new(),
            current_frame:        0,
            server_time:          0.0,
            prev_snapshot:        HashMap::new(),
            curr_snapshot:        HashMap::new(),
            ban_list:             Vec::new(),
            next_client_id:       1,
            stats:                ServerStats::default(),
            pending_disconnects:  Vec::new(),
            client_acked_frame:   HashMap::new(),
        }
    }

    // ── Client management ────────────────────────────────────────────────────

    /// Accept an incoming connection. Returns `None` if the server is full or the address is banned.
    pub fn accept_connection(&mut self, address: String) -> Option<ClientId> {
        if self.clients.len() >= self.config.max_clients as usize {
            return None;
        }
        if self.is_address_banned(&address) {
            return None;
        }
        let id = ClientId::new(self.next_client_id);
        self.next_client_id += 1;

        let info = ClientInfo::new(id, address, self.server_time as f32);
        self.clients.insert(id, info);
        self.input_history.insert(id, InputHistory::new());
        self.client_acked_frame.insert(id, 0);

        self.stats.total_clients_ever += 1;
        if self.clients.len() as u32 > self.stats.peak_clients {
            self.stats.peak_clients = self.clients.len() as u32;
        }

        Some(id)
    }

    /// Enqueue a message received from a client (call from transport layer).
    pub fn receive(&mut self, from: ClientId, msg: ClientMessage) {
        self.stats.messages_received += 1;
        self.inbound.push_back((from, msg));
    }

    /// Set state on a client (e.g., after auth succeeds).
    pub fn set_client_state(&mut self, id: ClientId, state: ClientState) {
        if let Some(c) = self.clients.get_mut(&id) {
            c.state = state;
        }
    }

    pub fn set_player_id(&mut self, client_id: ClientId, player_id: String) {
        if let Some(c) = self.clients.get_mut(&client_id) {
            c.player_id = Some(player_id);
        }
    }

    pub fn get_client(&self, id: ClientId) -> Option<&ClientInfo> {
        self.clients.get(&id)
    }

    pub fn connected_clients(&self) -> Vec<&ClientInfo> {
        self.clients.values().collect()
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    // ── Snapshot management ──────────────────────────────────────────────────

    /// Replace the current-frame entity snapshot (called by the sim layer after physics).
    pub fn set_entity_snapshot(&mut self, snapshot: Vec<EntitySnapshot>) {
        self.curr_snapshot.clear();
        for s in snapshot {
            self.curr_snapshot.insert(s.id, s);
        }
    }

    /// Push one entity into the current snapshot.
    pub fn upsert_entity(&mut self, snap: EntitySnapshot) {
        self.curr_snapshot.insert(snap.id, snap);
    }

    pub fn remove_entity(&mut self, entity_id: u64) {
        self.curr_snapshot.remove(&entity_id);
        self.prev_snapshot.remove(&entity_id);
    }

    // ── Input access ─────────────────────────────────────────────────────────

    /// Returns the most recent input for a client, if any.
    pub fn latest_input(&self, client_id: ClientId) -> Option<&PlayerInput> {
        self.input_history.get(&client_id)?.latest()
    }

    /// Returns input at a specific frame for a client.
    pub fn input_at_frame(&self, client_id: ClientId, frame: u64) -> Option<&PlayerInput> {
        self.input_history.get(&client_id)?.get_by_frame(frame)
    }

    // ── Messaging ─────────────────────────────────────────────────────────────

    /// Broadcast a message to all connected clients.
    pub fn broadcast(&mut self, msg: ServerMessage) {
        self.outbound.push(OutboundMessage {
            target: MessageTarget::All,
            message: msg,
        });
    }

    /// Broadcast to all except one client.
    pub fn broadcast_except(&mut self, exclude: ClientId, msg: ServerMessage) {
        self.outbound.push(OutboundMessage {
            target: MessageTarget::Except(exclude),
            message: msg,
        });
    }

    /// Send to a specific client only.
    pub fn send_to(&mut self, client_id: ClientId, msg: ServerMessage) {
        self.outbound.push(OutboundMessage {
            target: MessageTarget::Single(client_id),
            message: msg,
        });
    }

    /// Send to a list of clients.
    pub fn send_to_group(&mut self, clients: Vec<ClientId>, msg: ServerMessage) {
        self.outbound.push(OutboundMessage {
            target: MessageTarget::Group(clients),
            message: msg,
        });
    }

    // ── Kick / Ban ────────────────────────────────────────────────────────────

    /// Kick a client with a reason. The disconnect is processed at end of tick.
    pub fn kick(&mut self, client_id: ClientId, reason: DisconnectReason) {
        self.send_to(client_id, ServerMessage::KickNotice { reason: reason.clone() });
        self.pending_disconnects.push((client_id, reason));
    }

    /// Ban a client for `duration_secs`. Negative = permanent.
    pub fn ban(&mut self, client_id: ClientId, duration_secs: f64) {
        if let Some(client) = self.clients.get(&client_id).cloned() {
            let record = BanRecord {
                address:      client.address.clone(),
                player_id:    client.player_id.clone(),
                banned_at:    self.server_time as f32,
                duration_secs,
                reason:       "Banned by server".to_string(),
            };
            self.ban_list.push(record);
            self.kick(client_id, DisconnectReason::Banned);
        }
    }

    /// Ban with an explicit reason string.
    pub fn ban_with_reason(&mut self, client_id: ClientId, duration_secs: f64, reason: &str) {
        if let Some(client) = self.clients.get(&client_id).cloned() {
            let record = BanRecord {
                address:      client.address.clone(),
                player_id:    client.player_id.clone(),
                banned_at:    self.server_time as f32,
                duration_secs,
                reason:       reason.to_string(),
            };
            self.ban_list.push(record);
            self.kick(client_id, DisconnectReason::Banned);
        }
    }

    pub fn unban_address(&mut self, address: &str) {
        self.ban_list.retain(|b| b.address != address);
    }

    pub fn is_address_banned(&self, address: &str) -> bool {
        self.ban_list.iter().any(|b| {
            b.address == address && !b.is_expired(self.server_time as f32)
        })
    }

    // ── Tick ─────────────────────────────────────────────────────────────────

    /// Advance server by one tick. `dt` is seconds since last tick.
    ///
    /// This:
    /// 1. Advances server time
    /// 2. Processes inbound messages
    /// 3. Checks for timed-out clients
    /// 4. Sends world snapshots or delta updates
    /// 5. Sends heartbeats
    /// 6. Flushes pending disconnects
    /// 7. Cleans expired bans
    ///
    /// Returns the list of outbound messages to be serialised by the transport layer.
    pub fn tick(&mut self, dt: f32) -> Vec<(MessageTarget, ServerMessage)> {
        let tick_start = self.server_time;
        self.server_time += dt as f64;
        self.current_frame += 1;
        self.stats.ticks_processed += 1;

        // Process inbound messages
        self.process_inbound();

        // Timeout check
        self.check_timeouts();

        // World snapshot / delta broadcast
        self.broadcast_world_state();

        // Heartbeats
        self.send_heartbeats();

        // Flush pending disconnects
        self.flush_disconnects();

        // Expire bans
        let now = self.server_time as f32;
        self.ban_list.retain(|b| !b.is_expired(now));

        // Update tick timing stats
        let tick_duration_ms = (self.server_time - tick_start) as f32 * 1000.0;
        let alpha = 0.1f32;
        self.stats.average_tick_ms = self.stats.average_tick_ms + alpha * (tick_duration_ms - self.stats.average_tick_ms);

        // Drain outbound queue
        let out: Vec<(MessageTarget, ServerMessage)> = self
            .outbound
            .drain(..)
            .map(|m| (m.target, m.message))
            .collect();

        self.stats.messages_sent += out.len() as u64;
        out
    }

    /// Graceful shutdown: kick all clients with ServerShutdown reason.
    pub fn shutdown(&mut self) -> Vec<(MessageTarget, ServerMessage)> {
        let ids: Vec<ClientId> = self.clients.keys().cloned().collect();
        for id in ids {
            self.kick(id, DisconnectReason::ServerShutdown);
        }
        self.flush_disconnects();
        self.outbound.drain(..).map(|m| (m.target, m.message)).collect()
    }

    pub fn current_frame(&self) -> u64 { self.current_frame }
    pub fn server_time(&self)  -> f64  { self.server_time }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn process_inbound(&mut self) {
        let messages: Vec<(ClientId, ClientMessage)> = self.inbound.drain(..).collect();
        for (from, msg) in messages {
            self.handle_client_message(from, msg);
        }
    }

    fn handle_client_message(&mut self, from: ClientId, msg: ClientMessage) {
        // Check client still exists
        if !self.clients.contains_key(&from) { return; }

        match msg {
            ClientMessage::Connect { player_id, auth_token: _, version: _ } => {
                if let Some(c) = self.clients.get_mut(&from) {
                    c.state = ClientState::Authenticating;
                    c.player_id = Some(player_id.clone());
                }
                let msg = ServerMessage::ConnectionAccepted {
                    client_id: from.inner(),
                    server_time: self.server_time,
                };
                self.outbound.push(OutboundMessage {
                    target: MessageTarget::Single(from),
                    message: msg,
                });
                // Notify others
                let join_msg = ServerMessage::PlayerJoined {
                    client_id: from.inner(),
                    player_id,
                };
                self.outbound.push(OutboundMessage {
                    target: MessageTarget::Except(from),
                    message: join_msg,
                });
            }

            ClientMessage::PlayerInput { frame, buttons, axis_x, axis_y, yaw, pitch } => {
                let input = PlayerInput { frame, buttons, axis_x, axis_y, yaw, pitch };
                if let Some(hist) = self.input_history.get_mut(&from) {
                    hist.push(input);
                }
            }

            ClientMessage::ChatMessage { text, channel } => {
                let sender = self.clients
                    .get(&from)
                    .and_then(|c| c.player_id.clone())
                    .unwrap_or_else(|| format!("Client_{}", from.inner()));

                let chat = ServerMessage::ChatMessage { sender, text, channel };
                self.outbound.push(OutboundMessage {
                    target: MessageTarget::All,
                    message: chat,
                });
            }

            ClientMessage::RequestRespawn => {
                // Signal respawn intent via a GameEvent
                let payload = from.inner().to_le_bytes().to_vec();
                let ev = ServerMessage::GameEvent {
                    event_type: "respawn_request".to_string(),
                    payload,
                };
                self.outbound.push(OutboundMessage {
                    target: MessageTarget::Single(from),
                    message: ev,
                });
            }

            ClientMessage::Acknowledge { frame } => {
                self.client_acked_frame.insert(from, frame);
            }

            ClientMessage::HeartbeatReply { client_time } => {
                let rtt_ms = ((self.server_time - client_time) * 1000.0) as u32;
                let now = self.server_time as f32;
                if let Some(c) = self.clients.get_mut(&from) {
                    c.update_heartbeat(now, rtt_ms);
                }
            }

            ClientMessage::Disconnect => {
                self.pending_disconnects.push((from, DisconnectReason::ClientRequest));
            }
        }
    }

    fn check_timeouts(&mut self) {
        let now = self.server_time as f32;
        let timeout_ms = self.config.timeout_ms;
        let timed_out: Vec<ClientId> = self.clients
            .iter()
            .filter(|(_, c)| c.is_timed_out(now, timeout_ms))
            .map(|(id, _)| *id)
            .collect();

        for id in timed_out {
            self.pending_disconnects.push((id, DisconnectReason::Timeout));
        }
    }

    fn broadcast_world_state(&mut self) {
        let is_snapshot_frame = self.current_frame % self.config.snapshot_interval as u64 == 0;

        let playing_clients: Vec<ClientId> = self.clients
            .iter()
            .filter(|(_, c)| c.state.can_receive_game_data())
            .map(|(id, _)| *id)
            .collect();

        if playing_clients.is_empty() { return; }

        if is_snapshot_frame {
            let entities: Vec<EntitySnapshot> = self.curr_snapshot.values().cloned().collect();
            let snap = ServerMessage::WorldSnapshot {
                frame: self.current_frame,
                entities,
            };
            self.outbound.push(OutboundMessage {
                target: MessageTarget::Group(playing_clients),
                message: snap,
            });
            self.stats.snapshots_sent += 1;
        } else {
            // Compute deltas vs prev snapshot
            let mut changes: Vec<EntityDelta> = Vec::new();
            for (id, curr) in &self.curr_snapshot {
                if let Some(prev) = self.prev_snapshot.get(id) {
                    let delta = EntityDelta::from_snapshots(prev, curr, 0.001);
                    if delta.has_changes() {
                        changes.push(delta);
                    }
                } else {
                    // New entity — send all fields as delta
                    changes.push(EntityDelta {
                        id: *id,
                        changed_fields: EntityField::POS | EntityField::ROT | EntityField::VEL | EntityField::FLAGS,
                        data: {
                            let mut d = Vec::new();
                            for &v in &curr.pos { d.extend_from_slice(&v.to_le_bytes()); }
                            for &v in &curr.rot { d.extend_from_slice(&v.to_le_bytes()); }
                            for &v in &curr.vel { d.extend_from_slice(&v.to_le_bytes()); }
                            d.extend_from_slice(&curr.flags.to_le_bytes());
                            d
                        },
                    });
                }
            }

            if !changes.is_empty() {
                let delta = ServerMessage::DeltaUpdate {
                    frame: self.current_frame,
                    changes,
                };
                self.outbound.push(OutboundMessage {
                    target: MessageTarget::Group(playing_clients),
                    message: delta,
                });
                self.stats.deltas_sent += 1;
            }
        }

        // Rotate snapshots
        self.prev_snapshot.clear();
        for (id, snap) in &self.curr_snapshot {
            self.prev_snapshot.insert(*id, snap.clone());
        }
    }

    fn send_heartbeats(&mut self) {
        let interval = self.config.heartbeat_interval_ms as f64 / 1000.0;
        let ticks_per_hb = (interval * self.config.tick_rate as f64).max(1.0) as u64;
        if self.current_frame % ticks_per_hb == 0 {
            let hb = ServerMessage::Heartbeat { server_time: self.server_time };
            self.outbound.push(OutboundMessage {
                target:  MessageTarget::All,
                message: hb,
            });
        }
    }

    fn flush_disconnects(&mut self) {
        let disconnects: Vec<(ClientId, DisconnectReason)> = self.pending_disconnects.drain(..).collect();
        for (id, reason) in disconnects {
            if self.clients.remove(&id).is_some() {
                self.input_history.remove(&id);
                self.client_acked_frame.remove(&id);

                let player_id = self.clients.get(&id)
                    .and_then(|c| c.player_id.clone())
                    .unwrap_or_default();
                let _ = player_id; // already removed above

                let leave_msg = ServerMessage::PlayerLeft {
                    client_id: id.inner(),
                    reason,
                };
                self.outbound.push(OutboundMessage {
                    target: MessageTarget::All,
                    message: leave_msg,
                });
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server() -> GameServer {
        GameServer::new(ServerId::new(1), ServerConfig::default())
    }

    #[test]
    fn accept_connection_increments_count() {
        let mut s = make_server();
        let id = s.accept_connection("127.0.0.1:1234".into());
        assert!(id.is_some());
        assert_eq!(s.client_count(), 1);
    }

    #[test]
    fn full_server_rejects() {
        let mut s = GameServer::new(ServerId::new(1), ServerConfig { max_clients: 2, ..Default::default() });
        s.accept_connection("1.1.1.1:1".into());
        s.accept_connection("1.1.1.2:1".into());
        let r = s.accept_connection("1.1.1.3:1".into());
        assert!(r.is_none());
    }

    #[test]
    fn entity_delta_detects_pos_change() {
        let prev = EntitySnapshot { id: 1, pos: [0.0, 0.0, 0.0], rot: [0.0,0.0,0.0,1.0], vel: [0.0;3], flags: 0 };
        let curr = EntitySnapshot { id: 1, pos: [1.0, 2.0, 3.0], rot: [0.0,0.0,0.0,1.0], vel: [0.0;3], flags: 0 };
        let delta = EntityDelta::from_snapshots(&prev, &curr, 0.001);
        assert!(delta.changed_fields & EntityField::POS != 0);
        assert!(delta.changed_fields & EntityField::ROT == 0);
    }

    #[test]
    fn input_history_ring_buffer() {
        let mut hist = InputHistory::new();
        for i in 0..70u64 {
            hist.push(PlayerInput { frame: i, buttons: 0, axis_x: 0.0, axis_y: 0.0, yaw: 0.0, pitch: 0.0 });
        }
        assert_eq!(hist.len(), 64);
        // Old frames evicted
        assert!(hist.get_by_frame(0).is_none());
        assert!(hist.get_by_frame(69).is_some());
    }

    #[test]
    fn ban_blocks_connection() {
        let mut s = make_server();
        let id = s.accept_connection("10.0.0.1:9000".into()).unwrap();
        s.ban(id, 3600.0);
        let r = s.accept_connection("10.0.0.1:9000".into());
        assert!(r.is_none());
    }

    #[test]
    fn tick_produces_heartbeat() {
        let mut s = make_server();
        s.accept_connection("127.0.0.1:1".into());
        // Advance enough ticks for heartbeat (every 2 s at 20 tps = 40 ticks)
        let mut found_hb = false;
        for _ in 0..50 {
            let out = s.tick(1.0 / 20.0);
            for (_, msg) in &out {
                if let ServerMessage::Heartbeat { .. } = msg { found_hb = true; }
            }
        }
        assert!(found_hb);
    }
}
