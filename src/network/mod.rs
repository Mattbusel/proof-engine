//! Networking for Proof Engine: HTTP + WebSocket client, leaderboards, cloud saves.
//!
//! Provides async-compatible (non-blocking) networking primitives:
//! - HTTP request builder (GET/POST/PUT/DELETE)
//! - WebSocket message protocol
//! - Leaderboard submission and retrieval
//! - Cloud save serialization
//! - Lobby system messages
//! - Rollback netcode data structures

use std::collections::{HashMap, VecDeque};

// ── HttpMethod ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod { Get, Post, Put, Delete, Patch, Head }

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get    => "GET",
            HttpMethod::Post   => "POST",
            HttpMethod::Put    => "PUT",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Patch  => "PATCH",
            HttpMethod::Head   => "HEAD",
        }
    }
}

// ── HttpRequest ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method:  HttpMethod,
    pub url:     String,
    pub headers: HashMap<String, String>,
    pub body:    Option<Vec<u8>>,
    pub timeout_ms: u32,
}

impl HttpRequest {
    pub fn get(url: &str) -> Self {
        Self { method: HttpMethod::Get, url: url.to_string(), headers: HashMap::new(), body: None, timeout_ms: 5000 }
    }

    pub fn post(url: &str, body: Vec<u8>) -> Self {
        let mut req = Self::get(url);
        req.method = HttpMethod::Post;
        req.body = Some(body);
        req
    }

    pub fn post_json(url: &str, json: &str) -> Self {
        let mut req = Self::post(url, json.as_bytes().to_vec());
        req.headers.insert("Content-Type".to_string(), "application/json".to_string());
        req
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_bearer(mut self, token: &str) -> Self {
        self.headers.insert("Authorization".to_string(), format!("Bearer {}", token));
        self
    }

    pub fn with_timeout(mut self, ms: u32) -> Self { self.timeout_ms = ms; self }

    /// Encode as a simple textual representation (for debugging/logging).
    pub fn to_string(&self) -> String {
        format!("{} {} (body: {} bytes)",
            self.method.as_str(), self.url,
            self.body.as_ref().map(|b| b.len()).unwrap_or(0))
    }
}

// ── HttpResponse ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status:  u16,
    pub headers: HashMap<String, String>,
    pub body:    Vec<u8>,
    pub latency_ms: u32,
}

impl HttpResponse {
    pub fn ok(body: Vec<u8>) -> Self {
        Self { status: 200, headers: HashMap::new(), body, latency_ms: 0 }
    }

    pub fn error(status: u16, message: &str) -> Self {
        Self { status, headers: HashMap::new(), body: message.as_bytes().to_vec(), latency_ms: 0 }
    }

    pub fn body_str(&self) -> &str {
        std::str::from_utf8(&self.body).unwrap_or("")
    }

    pub fn is_success(&self) -> bool { self.status >= 200 && self.status < 300 }
    pub fn is_client_error(&self) -> bool { self.status >= 400 && self.status < 500 }
    pub fn is_server_error(&self) -> bool { self.status >= 500 }
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum WsMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<(u16, String)>),
}

impl WsMessage {
    pub fn text(s: &str) -> Self { Self::Text(s.to_string()) }
    pub fn json(s: &str) -> Self { Self::Text(s.to_string()) }
    pub fn binary(data: Vec<u8>) -> Self { Self::Binary(data) }

    pub fn is_text(&self) -> bool { matches!(self, WsMessage::Text(_)) }
    pub fn is_binary(&self) -> bool { matches!(self, WsMessage::Binary(_)) }

    pub fn as_text(&self) -> Option<&str> {
        if let WsMessage::Text(s) = self { Some(s.as_str()) } else { None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WsState { Connecting, Open, Closing, Closed }

/// Mock WebSocket client with a pending message queue.
pub struct WebSocket {
    pub url:     String,
    pub state:   WsState,
    outgoing:    VecDeque<WsMessage>,
    incoming:    VecDeque<WsMessage>,
    on_error:    Option<String>,
    pub protocol: String,
}

impl WebSocket {
    pub fn new(url: &str) -> Self {
        Self { url: url.to_string(), state: WsState::Connecting,
               outgoing: VecDeque::new(), incoming: VecDeque::new(),
               on_error: None, protocol: String::new() }
    }

    pub fn open(&mut self) { self.state = WsState::Open; }
    pub fn close(&mut self) { self.state = WsState::Closing; }

    pub fn send(&mut self, msg: WsMessage) -> bool {
        if self.state != WsState::Open { return false; }
        self.outgoing.push_back(msg);
        true
    }

    pub fn send_text(&mut self, text: &str) -> bool { self.send(WsMessage::text(text)) }
    pub fn send_json(&mut self, json: &str) -> bool { self.send(WsMessage::json(json)) }

    pub fn recv(&mut self) -> Option<WsMessage> { self.incoming.pop_front() }

    /// Inject a received message (for testing / server push).
    pub fn inject_message(&mut self, msg: WsMessage) { self.incoming.push_back(msg); }

    pub fn has_pending_send(&self) -> bool { !self.outgoing.is_empty() }
    pub fn drain_outgoing(&mut self) -> Vec<WsMessage> { self.outgoing.drain(..).collect() }
    pub fn is_open(&self) -> bool { self.state == WsState::Open }
}

// ── Leaderboard ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LeaderboardEntry {
    pub rank:       u32,
    pub player_id:  String,
    pub display_name: String,
    pub score:      i64,
    pub metadata:   HashMap<String, String>,
    pub timestamp:  u64,
}

#[derive(Debug, Clone)]
pub struct Leaderboard {
    pub id:      String,
    pub name:    String,
    pub entries: Vec<LeaderboardEntry>,
    pub page:    u32,
    pub total:   u32,
}

impl Leaderboard {
    pub fn new(id: &str, name: &str) -> Self {
        Self { id: id.to_string(), name: name.to_string(), entries: Vec::new(), page: 0, total: 0 }
    }

    pub fn add_entry(&mut self, entry: LeaderboardEntry) {
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.score.cmp(&a.score));
        // Re-rank
        for (i, e) in self.entries.iter_mut().enumerate() { e.rank = (i + 1) as u32; }
    }

    pub fn top_n(&self, n: usize) -> &[LeaderboardEntry] {
        &self.entries[..n.min(self.entries.len())]
    }

    pub fn rank_of(&self, player_id: &str) -> Option<u32> {
        self.entries.iter().find(|e| e.player_id == player_id).map(|e| e.rank)
    }

    /// Serialize to JSON-like string for API submission.
    pub fn submission_json(&self, player_id: &str, score: i64, metadata: &HashMap<String, String>) -> String {
        let meta_str: Vec<String> = metadata.iter().map(|(k, v)| format!("\"{}\":\"{}\"", k, v)).collect();
        format!(
            "{{\"leaderboard\":\"{}\",\"player_id\":\"{}\",\"score\":{},\"metadata\":{{{}}}}}",
            self.id, player_id, score, meta_str.join(",")
        )
    }
}

// ── CloudSave ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CloudSave {
    pub player_id:   String,
    pub slot:        u8,
    pub version:     u32,
    pub created_at:  u64,
    pub updated_at:  u64,
    pub data:        Vec<u8>,
    pub checksum:    u32,
    pub tags:        Vec<String>,
    pub metadata:    HashMap<String, String>,
}

impl CloudSave {
    pub fn new(player_id: &str, slot: u8, data: Vec<u8>) -> Self {
        let checksum = simple_checksum(&data);
        Self {
            player_id: player_id.to_string(), slot, version: 1,
            created_at: 0, updated_at: 0,
            data, checksum, tags: Vec::new(), metadata: HashMap::new(),
        }
    }

    pub fn validate(&self) -> bool {
        simple_checksum(&self.data) == self.checksum
    }

    pub fn size_bytes(&self) -> usize { self.data.len() }

    /// Encode to bytes for upload.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"CSAVE1\x00\x00");
        out.push(self.slot);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&self.checksum.to_le_bytes());
        out.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.data);
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 20 || &bytes[0..6] != b"CSAVE1" { return None; }
        let slot = bytes[8];
        let version = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
        let checksum = u32::from_le_bytes(bytes[13..17].try_into().ok()?);
        let data_len = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
        if bytes.len() < 21 + data_len { return None; }
        let data = bytes[21..21 + data_len].to_vec();
        Some(Self { player_id: String::new(), slot, version, created_at: 0, updated_at: 0,
                    data, checksum, tags: Vec::new(), metadata: HashMap::new() })
    }
}

fn simple_checksum(data: &[u8]) -> u32 {
    data.iter().enumerate().fold(0u32, |acc, (i, &b)| {
        acc.wrapping_add((b as u32).wrapping_mul((i as u32).wrapping_add(1)))
    })
}

// ── Lobby ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LobbyState { Open, InProgress, Closed }

#[derive(Debug, Clone)]
pub struct LobbyPlayer {
    pub player_id:    String,
    pub display_name: String,
    pub ready:        bool,
    pub latency_ms:   u32,
    pub team:         u8,
    pub metadata:     HashMap<String, String>,
}

impl LobbyPlayer {
    pub fn new(id: &str, name: &str) -> Self {
        Self { player_id: id.to_string(), display_name: name.to_string(),
               ready: false, latency_ms: 0, team: 0, metadata: HashMap::new() }
    }
}

#[derive(Debug, Clone)]
pub struct Lobby {
    pub id:      String,
    pub name:    String,
    pub state:   LobbyState,
    pub players: Vec<LobbyPlayer>,
    pub max_players: u8,
    pub host_id: String,
    pub settings: HashMap<String, String>,
}

impl Lobby {
    pub fn new(id: &str, name: &str, max_players: u8) -> Self {
        Self { id: id.to_string(), name: name.to_string(), state: LobbyState::Open,
               players: Vec::new(), max_players, host_id: String::new(), settings: HashMap::new() }
    }

    pub fn join(&mut self, player: LobbyPlayer) -> bool {
        if self.players.len() >= self.max_players as usize { return false; }
        if self.state != LobbyState::Open { return false; }
        if self.players.iter().any(|p| p.player_id == player.player_id) { return false; }
        if self.players.is_empty() {
            self.host_id = player.player_id.clone();
        }
        self.players.push(player);
        true
    }

    pub fn leave(&mut self, player_id: &str) {
        self.players.retain(|p| p.player_id != player_id);
        // Transfer host if needed
        if self.host_id == player_id && !self.players.is_empty() {
            self.host_id = self.players[0].player_id.clone();
        }
    }

    pub fn set_ready(&mut self, player_id: &str, ready: bool) {
        if let Some(p) = self.players.iter_mut().find(|p| p.player_id == player_id) {
            p.ready = ready;
        }
    }

    pub fn all_ready(&self) -> bool {
        !self.players.is_empty() && self.players.iter().all(|p| p.ready)
    }

    pub fn start(&mut self) -> bool {
        if !self.all_ready() { return false; }
        self.state = LobbyState::InProgress;
        true
    }

    pub fn player_count(&self) -> usize { self.players.len() }
    pub fn is_full(&self) -> bool { self.players.len() >= self.max_players as usize }
}

// ── Rollback Netcode ──────────────────────────────────────────────────────────

/// Input for one player at a given frame.
#[derive(Debug, Clone, Default)]
pub struct NetInput {
    pub frame:   u64,
    pub player:  u8,
    pub buttons: u32, // bitfield
    pub axes:    [i16; 4], // fixed-point axes
    pub checksum: u16,
}

impl NetInput {
    pub fn new(frame: u64, player: u8) -> Self {
        Self { frame, player, ..Default::default() }
    }

    pub fn press_button(&mut self, btn: u8) { self.buttons |= 1 << btn; }
    pub fn release_button(&mut self, btn: u8) { self.buttons &= !(1 << btn); }
    pub fn is_pressed(&self, btn: u8) -> bool { (self.buttons >> btn) & 1 != 0 }
    pub fn set_axis(&mut self, idx: usize, value: f32) {
        if idx < 4 { self.axes[idx] = (value.clamp(-1.0, 1.0) * 32767.0) as i16; }
    }
    pub fn get_axis(&self, idx: usize) -> f32 {
        if idx < 4 { self.axes[idx] as f32 / 32767.0 } else { 0.0 }
    }

    pub fn encode(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..8].copy_from_slice(&self.frame.to_le_bytes());
        buf[8]  = self.player;
        buf[9..13].copy_from_slice(&self.buttons.to_le_bytes());
        buf[13..15].copy_from_slice(&self.axes[0].to_le_bytes());
        buf[15] = (self.checksum & 0xFF) as u8;
        buf
    }

    pub fn decode(buf: &[u8; 16]) -> Self {
        let frame   = u64::from_le_bytes(buf[0..8].try_into().unwrap());
        let player  = buf[8];
        let buttons = u32::from_le_bytes(buf[9..13].try_into().unwrap());
        Self { frame, player, buttons, axes: [0; 4], checksum: buf[15] as u16 }
    }
}

/// Rollback netcode state: stores a history of inputs for re-simulation.
pub struct RollbackState {
    pub max_rollback:   usize,
    pub current_frame:  u64,
    /// Confirmed inputs per frame per player.
    confirmed:          VecDeque<Vec<NetInput>>,
    /// Predicted inputs (used when real inputs not yet received).
    predicted:          HashMap<(u64, u8), NetInput>,
    /// Pending remote inputs waiting for confirmation.
    pending:            Vec<NetInput>,
    /// Frame numbers where rollback was needed.
    pub rollback_log:   Vec<u64>,
    pub player_count:   usize,
}

impl RollbackState {
    pub fn new(player_count: usize, max_rollback: usize) -> Self {
        Self {
            max_rollback, current_frame: 0,
            confirmed: VecDeque::new(),
            predicted: HashMap::new(),
            pending: Vec::new(),
            rollback_log: Vec::new(),
            player_count,
        }
    }

    /// Add a confirmed input from a remote player.
    pub fn add_remote_input(&mut self, input: NetInput) {
        // If this input's frame is in the past, we need to rollback
        if input.frame < self.current_frame {
            let rollback_to = input.frame;
            self.rollback_log.push(rollback_to);
        }
        // Store confirmed input
        let frame = input.frame;
        while self.confirmed.len() <= frame as usize {
            self.confirmed.push_back(Vec::new());
        }
        if (frame as usize) < self.confirmed.len() {
            self.confirmed[frame as usize].push(input);
        }
        // Remove matching prediction
        self.predicted.remove(&(frame, input.player));
    }

    /// Get input for (frame, player): confirmed if available, else predicted.
    pub fn get_input(&self, frame: u64, player: u8) -> NetInput {
        // Check confirmed
        if let Some(inputs) = self.confirmed.get(frame as usize) {
            if let Some(inp) = inputs.iter().find(|i| i.player == player) {
                return inp.clone();
            }
        }
        // Return prediction
        self.predicted.get(&(frame, player))
            .cloned()
            .unwrap_or_else(|| NetInput::new(frame, player))
    }

    /// Predict next frame's input by repeating current frame's input.
    pub fn predict_input(&mut self, frame: u64, player: u8) {
        let prev = self.get_input(frame.saturating_sub(1), player);
        let mut pred = prev;
        pred.frame = frame;
        self.predicted.insert((frame, player), pred);
    }

    pub fn advance_frame(&mut self) {
        for p in 0..self.player_count as u8 {
            self.predict_input(self.current_frame + 1, p);
        }
        self.current_frame += 1;
        // Trim old confirmed data
        while self.confirmed.len() > self.max_rollback {
            self.confirmed.pop_front();
        }
    }

    pub fn needs_rollback(&self) -> bool { !self.rollback_log.is_empty() }

    pub fn consume_rollback(&mut self) -> Option<u64> { self.rollback_log.pop() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_builder() {
        let req = HttpRequest::get("https://api.example.com/scores")
            .with_header("Accept", "application/json")
            .with_bearer("token123")
            .with_timeout(3000);
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.timeout_ms, 3000);
        assert!(req.headers.contains_key("Authorization"));
    }

    #[test]
    fn test_http_response_status() {
        let ok = HttpResponse::ok(b"{}".to_vec());
        assert!(ok.is_success());
        let err = HttpResponse::error(404, "Not Found");
        assert!(err.is_client_error());
        let srv = HttpResponse::error(500, "Internal Server Error");
        assert!(srv.is_server_error());
    }

    #[test]
    fn test_websocket_send_recv() {
        let mut ws = WebSocket::new("wss://echo.example.com");
        ws.open();
        assert!(ws.is_open());

        ws.send_text("hello");
        assert!(ws.has_pending_send());
        let out = ws.drain_outgoing();
        assert_eq!(out.len(), 1);

        ws.inject_message(WsMessage::text("world"));
        let msg = ws.recv().unwrap();
        assert_eq!(msg.as_text(), Some("world"));
    }

    #[test]
    fn test_leaderboard_ranking() {
        let mut lb = Leaderboard::new("speed_run", "Speed Run");
        lb.add_entry(LeaderboardEntry { rank: 0, player_id: "a".to_string(), display_name: "Alice".to_string(), score: 500, metadata: HashMap::new(), timestamp: 0 });
        lb.add_entry(LeaderboardEntry { rank: 0, player_id: "b".to_string(), display_name: "Bob".to_string(), score: 800, metadata: HashMap::new(), timestamp: 0 });
        lb.add_entry(LeaderboardEntry { rank: 0, player_id: "c".to_string(), display_name: "Carol".to_string(), score: 650, metadata: HashMap::new(), timestamp: 0 });

        assert_eq!(lb.entries[0].display_name, "Bob");
        assert_eq!(lb.rank_of("a"), Some(3));
        assert_eq!(lb.rank_of("b"), Some(1));
    }

    #[test]
    fn test_cloud_save_encode_decode() {
        let save = CloudSave::new("player1", 0, vec![1, 2, 3, 4, 5]);
        assert!(save.validate());
        let encoded = save.encode();
        let decoded = CloudSave::decode(&encoded).unwrap();
        assert_eq!(decoded.data, vec![1, 2, 3, 4, 5]);
        assert!(decoded.validate());
    }

    #[test]
    fn test_lobby_join_leave() {
        let mut lobby = Lobby::new("room1", "Test Room", 4);
        assert!(lobby.join(LobbyPlayer::new("p1", "Alice")));
        assert!(lobby.join(LobbyPlayer::new("p2", "Bob")));
        assert_eq!(lobby.player_count(), 2);
        assert_eq!(lobby.host_id, "p1");

        lobby.leave("p1");
        assert_eq!(lobby.host_id, "p2"); // host transferred
        assert_eq!(lobby.player_count(), 1);
    }

    #[test]
    fn test_lobby_ready_and_start() {
        let mut lobby = Lobby::new("r2", "Room", 2);
        lobby.join(LobbyPlayer::new("p1", "A"));
        lobby.join(LobbyPlayer::new("p2", "B"));
        assert!(!lobby.start()); // not all ready
        lobby.set_ready("p1", true);
        lobby.set_ready("p2", true);
        assert!(lobby.start());
        assert_eq!(lobby.state, LobbyState::InProgress);
    }

    #[test]
    fn test_lobby_max_players() {
        let mut lobby = Lobby::new("r3", "Room", 2);
        assert!(lobby.join(LobbyPlayer::new("p1", "A")));
        assert!(lobby.join(LobbyPlayer::new("p2", "B")));
        assert!(!lobby.join(LobbyPlayer::new("p3", "C"))); // full
        assert!(lobby.is_full());
    }

    #[test]
    fn test_net_input_encode_decode() {
        let mut input = NetInput::new(42, 1);
        input.press_button(3);
        input.set_axis(0, 0.75);
        let encoded = input.encode();
        let decoded = NetInput::decode(&encoded);
        assert_eq!(decoded.frame, 42);
        assert_eq!(decoded.player, 1);
        assert!(decoded.is_pressed(3));
    }

    #[test]
    fn test_rollback_state_predict() {
        let mut rb = RollbackState::new(2, 8);
        rb.advance_frame();
        let inp = rb.get_input(1, 0);
        assert_eq!(inp.frame, 1);
    }

    #[test]
    fn test_rollback_detects_mismatch() {
        let mut rb = RollbackState::new(2, 8);
        rb.advance_frame();
        rb.advance_frame();
        // Remote player sends input for frame 1 (in the past)
        let remote = NetInput::new(1, 1);
        rb.add_remote_input(remote);
        assert!(rb.needs_rollback());
        let frame = rb.consume_rollback();
        assert_eq!(frame, Some(1));
    }
}
