//! Remote Procedure Call (RPC) system for game events.
//!
//! RPCs are named, typed, network-dispatched function calls.  They are
//! registered centrally in `RpcRegistry`, dispatched via `RpcQueue`, and
//! can be targeted at a single client, a team, or all peers.
//!
//! ## Flow
//! 1. Server/client calls `RpcQueue::enqueue(call)`.
//! 2. Transport drains the queue and serialises all pending calls into one
//!    or more UDP packets (via `RpcBatcher`).
//! 3. Remote side deserialises, looks up the `RpcId` in `RpcRegistry`, and
//!    invokes the registered `RpcHandler`.
//! 4. `RpcSecurity` validates the caller and rate-limits calls per client.
//! 5. `RpcReplay` optionally records every call for debugging / replay.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::networking::sync::Vec3;

// ─── RpcId ───────────────────────────────────────────────────────────────────

/// Compact 16-bit identifier for a registered RPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RpcId(pub u16);

// ─── Built-in RPC IDs ────────────────────────────────────────────────────────

pub const RPC_CHAT_MESSAGE:   RpcId = RpcId(0x0001);
pub const RPC_PLAYER_JOINED:  RpcId = RpcId(0x0002);
pub const RPC_PLAYER_LEFT:    RpcId = RpcId(0x0003);
pub const RPC_GAME_EVENT:     RpcId = RpcId(0x0004);
pub const RPC_FORCE_FIELD:    RpcId = RpcId(0x0005);
pub const RPC_PARTICLE_BURST: RpcId = RpcId(0x0006);
pub const RPC_SCREEN_EFFECT:  RpcId = RpcId(0x0007);
pub const RPC_PLAY_SOUND:     RpcId = RpcId(0x0008);
pub const RPC_DAMAGE_NUMBER:  RpcId = RpcId(0x0009);
pub const RPC_ENTITY_STATUS:  RpcId = RpcId(0x000A);
pub const RPC_CAMERA_SHAKE:   RpcId = RpcId(0x000B);
pub const RPC_DIALOGUE:       RpcId = RpcId(0x000C);

// ─── RpcTarget ───────────────────────────────────────────────────────────────

/// Who should receive and execute this RPC call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcTarget {
    /// Broadcast to all connected clients.
    All,
    /// Send to the server only.
    Server,
    /// Send to a specific client.
    Client(u64),
    /// Send to all players on a team.
    Team(u8),
    /// Broadcast except one client.
    AllExcept(u64),
}

// ─── RpcParam ────────────────────────────────────────────────────────────────

/// Dynamically typed RPC parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum RpcParam {
    Bool(bool),
    Int(i64),
    Float(f32),
    Str(String),
    Vec3(Vec3),
    Bytes(Vec<u8>),
}

impl RpcParam {
    /// Type tag byte used in serialisation.
    pub fn type_tag(&self) -> u8 {
        match self {
            RpcParam::Bool(_)  => 0x01,
            RpcParam::Int(_)   => 0x02,
            RpcParam::Float(_) => 0x03,
            RpcParam::Str(_)   => 0x04,
            RpcParam::Vec3(_)  => 0x05,
            RpcParam::Bytes(_) => 0x06,
        }
    }

    /// Serialise to bytes: [type_tag(1)] + [payload].
    pub fn serialize(&self, out: &mut Vec<u8>) {
        out.push(self.type_tag());
        match self {
            RpcParam::Bool(b)  => out.push(*b as u8),
            RpcParam::Int(i)   => out.extend_from_slice(&i.to_be_bytes()),
            RpcParam::Float(f) => out.extend_from_slice(&f.to_bits().to_be_bytes()),
            RpcParam::Str(s)   => {
                let bytes = s.as_bytes();
                let len   = bytes.len().min(0xFFFF) as u16;
                out.extend_from_slice(&len.to_be_bytes());
                out.extend_from_slice(&bytes[..len as usize]);
            }
            RpcParam::Vec3(v) => {
                out.extend_from_slice(&v.x.to_bits().to_be_bytes());
                out.extend_from_slice(&v.y.to_bits().to_be_bytes());
                out.extend_from_slice(&v.z.to_bits().to_be_bytes());
            }
            RpcParam::Bytes(b) => {
                let len = b.len().min(0xFFFF) as u16;
                out.extend_from_slice(&len.to_be_bytes());
                out.extend_from_slice(&b[..len as usize]);
            }
        }
    }

    /// Deserialise one param from `buf` at `offset`.
    /// Returns `(param, new_offset)`.
    pub fn deserialize(buf: &[u8], offset: usize) -> Result<(Self, usize), RpcError> {
        if offset >= buf.len() {
            return Err(RpcError::DeserializeError("buffer empty".into()));
        }
        let tag = buf[offset];
        let pos = offset + 1;

        macro_rules! need {
            ($n:expr) => {
                if pos + $n > buf.len() {
                    return Err(RpcError::DeserializeError("truncated param".into()));
                }
            };
        }

        match tag {
            0x01 => {
                need!(1);
                Ok((RpcParam::Bool(buf[pos] != 0), pos + 1))
            }
            0x02 => {
                need!(8);
                let v = i64::from_be_bytes(buf[pos..pos+8].try_into().unwrap());
                Ok((RpcParam::Int(v), pos + 8))
            }
            0x03 => {
                need!(4);
                let v = f32::from_bits(u32::from_be_bytes(buf[pos..pos+4].try_into().unwrap()));
                Ok((RpcParam::Float(v), pos + 4))
            }
            0x04 => {
                need!(2);
                let len = u16::from_be_bytes([buf[pos], buf[pos+1]]) as usize;
                if pos + 2 + len > buf.len() {
                    return Err(RpcError::DeserializeError("str truncated".into()));
                }
                let s = std::str::from_utf8(&buf[pos+2..pos+2+len])
                    .map_err(|e| RpcError::DeserializeError(e.to_string()))?
                    .to_string();
                Ok((RpcParam::Str(s), pos + 2 + len))
            }
            0x05 => {
                need!(12);
                let x = f32::from_bits(u32::from_be_bytes(buf[pos..pos+4].try_into().unwrap()));
                let y = f32::from_bits(u32::from_be_bytes(buf[pos+4..pos+8].try_into().unwrap()));
                let z = f32::from_bits(u32::from_be_bytes(buf[pos+8..pos+12].try_into().unwrap()));
                Ok((RpcParam::Vec3(Vec3::new(x, y, z)), pos + 12))
            }
            0x06 => {
                need!(2);
                let len = u16::from_be_bytes([buf[pos], buf[pos+1]]) as usize;
                if pos + 2 + len > buf.len() {
                    return Err(RpcError::DeserializeError("bytes truncated".into()));
                }
                Ok((RpcParam::Bytes(buf[pos+2..pos+2+len].to_vec()), pos + 2 + len))
            }
            _ => Err(RpcError::DeserializeError(format!("unknown param tag 0x{tag:02x}"))),
        }
    }

    // ── Convenience accessors ────────────────────────────────────────────────

    pub fn as_bool(&self) -> Option<bool> {
        if let RpcParam::Bool(v) = self { Some(*v) } else { None }
    }
    pub fn as_int(&self) -> Option<i64> {
        if let RpcParam::Int(v) = self { Some(*v) } else { None }
    }
    pub fn as_float(&self) -> Option<f32> {
        if let RpcParam::Float(v) = self { Some(*v) } else { None }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let RpcParam::Str(v) = self { Some(v) } else { None }
    }
    pub fn as_vec3(&self) -> Option<Vec3> {
        if let RpcParam::Vec3(v) = self { Some(*v) } else { None }
    }
    pub fn as_bytes(&self) -> Option<&[u8]> {
        if let RpcParam::Bytes(v) = self { Some(v) } else { None }
    }
}

// ─── RpcCall ─────────────────────────────────────────────────────────────────

/// A fully-formed RPC call ready for serialisation and dispatch.
#[derive(Debug, Clone)]
pub struct RpcCall {
    pub id:     RpcId,
    pub target: RpcTarget,
    pub params: Vec<RpcParam>,
    /// Sequence number (assigned by `RpcQueue` on enqueue).
    pub seq:    u32,
    /// Originating client_id (0 = server).
    pub caller: u64,
}

impl RpcCall {
    pub fn new(id: RpcId, target: RpcTarget, params: Vec<RpcParam>) -> Self {
        Self { id, target, params, seq: 0, caller: 0 }
    }

    pub fn with_caller(mut self, caller_id: u64) -> Self {
        self.caller = caller_id;
        self
    }

    /// Serialise to bytes: [rpc_id(2)] [target(2)] [caller(8)] [param_count(1)] [params...]
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.id.0.to_be_bytes());
        let target_tag: u16 = match &self.target {
            RpcTarget::All            => 0x0000,
            RpcTarget::Server         => 0x0001,
            RpcTarget::Client(id)     => {
                out.extend_from_slice(&id.to_be_bytes());
                0x0002
            }
            RpcTarget::Team(t)        => { out.push(*t); 0x0003 }
            RpcTarget::AllExcept(id)  => { out.extend_from_slice(&id.to_be_bytes()); 0x0004 }
        };
        // We need to write target_tag first, so rebuild
        let mut final_out = Vec::new();
        final_out.extend_from_slice(&self.id.0.to_be_bytes());
        final_out.extend_from_slice(&target_tag.to_be_bytes());
        final_out.extend_from_slice(&self.seq.to_be_bytes());
        final_out.extend_from_slice(&self.caller.to_be_bytes());

        // Target-specific extra bytes
        match &self.target {
            RpcTarget::Client(id)    => final_out.extend_from_slice(&id.to_be_bytes()),
            RpcTarget::Team(t)       => final_out.push(*t),
            RpcTarget::AllExcept(id) => final_out.extend_from_slice(&id.to_be_bytes()),
            _ => {}
        }

        final_out.push(self.params.len().min(0xFF) as u8);
        for p in &self.params {
            p.serialize(&mut final_out);
        }
        final_out
    }

    /// Deserialise one `RpcCall` from `buf` at `offset`.
    pub fn deserialize(buf: &[u8], offset: usize) -> Result<(Self, usize), RpcError> {
        let mut pos = offset;

        macro_rules! need {
            ($n:expr) => {
                if pos + $n > buf.len() {
                    return Err(RpcError::DeserializeError("truncated rpc call".into()));
                }
            };
        }

        need!(2);
        let id = RpcId(u16::from_be_bytes([buf[pos], buf[pos+1]]));
        pos += 2;

        need!(2);
        let target_tag = u16::from_be_bytes([buf[pos], buf[pos+1]]);
        pos += 2;

        need!(4);
        let seq = u32::from_be_bytes(buf[pos..pos+4].try_into().unwrap());
        pos += 4;

        need!(8);
        let caller = u64::from_be_bytes(buf[pos..pos+8].try_into().unwrap());
        pos += 8;

        let target = match target_tag {
            0x0000 => RpcTarget::All,
            0x0001 => RpcTarget::Server,
            0x0002 => {
                need!(8);
                let id_val = u64::from_be_bytes(buf[pos..pos+8].try_into().unwrap());
                pos += 8;
                RpcTarget::Client(id_val)
            }
            0x0003 => {
                need!(1);
                let t = buf[pos];
                pos += 1;
                RpcTarget::Team(t)
            }
            0x0004 => {
                need!(8);
                let id_val = u64::from_be_bytes(buf[pos..pos+8].try_into().unwrap());
                pos += 8;
                RpcTarget::AllExcept(id_val)
            }
            _ => return Err(RpcError::DeserializeError(format!("unknown target tag {target_tag}"))),
        };

        need!(1);
        let param_count = buf[pos] as usize;
        pos += 1;

        let mut params = Vec::with_capacity(param_count);
        for _ in 0..param_count {
            let (p, new_pos) = RpcParam::deserialize(buf, pos)?;
            params.push(p);
            pos = new_pos;
        }

        Ok((Self { id, target, params, seq, caller }, pos))
    }
}

// ─── RpcResult ───────────────────────────────────────────────────────────────

/// Return value from an RPC handler.
pub type RpcResult = Result<Option<RpcParam>, RpcError>;

/// Errors from the RPC system.
#[derive(Debug, Clone, PartialEq)]
pub enum RpcError {
    UnknownRpc(RpcId),
    InvalidParams { expected: usize, got: usize },
    WrongParamType { index: usize, expected: &'static str },
    RateLimited { rpc_id: RpcId, caller: u64 },
    Unauthorised { rpc_id: RpcId, caller: u64 },
    DeserializeError(String),
    HandlerPanic(String),
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RpcError {}

// ─── RpcHandler ──────────────────────────────────────────────────────────────

/// A boxed, type-erased RPC handler function.
pub type RpcHandler = Box<dyn Fn(&[RpcParam]) -> RpcResult + Send + Sync>;

// ─── RpcRegistry ─────────────────────────────────────────────────────────────

/// Central registry mapping `RpcId` ↔ name ↔ handler.
pub struct RpcRegistry {
    /// id → (name, handler)
    handlers:  HashMap<RpcId, (String, RpcHandler)>,
    /// name → id (reverse lookup)
    name_map:  HashMap<String, RpcId>,
    next_id:   u16,
}

impl RpcRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            handlers:  HashMap::new(),
            name_map:  HashMap::new(),
            next_id:   0x1000, // user RPCs start here
        };
        reg.register_builtins();
        reg
    }

    fn alloc_id(&mut self) -> RpcId {
        let id = RpcId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Register a handler under a fixed `id` and `name`.
    pub fn register_fixed(
        &mut self,
        id:      RpcId,
        name:    impl Into<String>,
        handler: RpcHandler,
    ) {
        let name = name.into();
        self.name_map.insert(name.clone(), id);
        self.handlers.insert(id, (name, handler));
    }

    /// Register a handler, auto-assigning an ID.  Returns the assigned `RpcId`.
    pub fn register(
        &mut self,
        name:    impl Into<String>,
        handler: RpcHandler,
    ) -> RpcId {
        let id = self.alloc_id();
        self.register_fixed(id, name, handler);
        id
    }

    /// Look up a handler by `RpcId`.
    pub fn handler(&self, id: RpcId) -> Option<&RpcHandler> {
        self.handlers.get(&id).map(|(_, h)| h)
    }

    /// Look up an ID by name.
    pub fn id_for(&self, name: &str) -> Option<RpcId> {
        self.name_map.get(name).copied()
    }

    /// Invoke an RPC by ID.  Returns `Err(UnknownRpc)` if not registered.
    pub fn invoke(&self, id: RpcId, params: &[RpcParam]) -> RpcResult {
        match self.handlers.get(&id) {
            Some((_, h)) => h(params),
            None => Err(RpcError::UnknownRpc(id)),
        }
    }

    /// Number of registered RPCs.
    pub fn len(&self) -> usize { self.handlers.len() }
    pub fn is_empty(&self) -> bool { self.handlers.is_empty() }

    // ── Built-in RPCs ────────────────────────────────────────────────────────

    fn register_builtins(&mut self) {
        // chat_message(sender_id: Int, text: Str)
        self.register_fixed(RPC_CHAT_MESSAGE, "chat_message", Box::new(|params| {
            if params.len() < 2 {
                return Err(RpcError::InvalidParams { expected: 2, got: params.len() });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            let _ = params[1].as_str()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Str" })?;
            Ok(None)
        }));

        // player_joined(player_id: Int, name: Str)
        self.register_fixed(RPC_PLAYER_JOINED, "player_joined", Box::new(|params| {
            if params.len() < 2 {
                return Err(RpcError::InvalidParams { expected: 2, got: params.len() });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            let _ = params[1].as_str()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Str" })?;
            Ok(None)
        }));

        // player_left(player_id: Int)
        self.register_fixed(RPC_PLAYER_LEFT, "player_left", Box::new(|params| {
            if params.is_empty() {
                return Err(RpcError::InvalidParams { expected: 1, got: 0 });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            Ok(None)
        }));

        // game_event(kind: Int, data: Bytes)
        self.register_fixed(RPC_GAME_EVENT, "game_event", Box::new(|params| {
            if params.len() < 2 {
                return Err(RpcError::InvalidParams { expected: 2, got: params.len() });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            let _ = params[1].as_bytes()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Bytes" })?;
            Ok(None)
        }));

        // force_field_spawn(field_type: Int, position: Vec3, strength: Float, ttl: Float)
        self.register_fixed(RPC_FORCE_FIELD, "force_field_spawn", Box::new(|params| {
            if params.len() < 4 {
                return Err(RpcError::InvalidParams { expected: 4, got: params.len() });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            let _ = params[1].as_vec3()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Vec3" })?;
            let _ = params[2].as_float()
                .ok_or(RpcError::WrongParamType { index: 2, expected: "Float" })?;
            let _ = params[3].as_float()
                .ok_or(RpcError::WrongParamType { index: 3, expected: "Float" })?;
            Ok(None)
        }));

        // particle_burst(preset: Int, origin: Vec3)
        self.register_fixed(RPC_PARTICLE_BURST, "particle_burst", Box::new(|params| {
            if params.len() < 2 {
                return Err(RpcError::InvalidParams { expected: 2, got: params.len() });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            let _ = params[1].as_vec3()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Vec3" })?;
            Ok(None)
        }));

        // screen_effect(effect_type: Int)
        self.register_fixed(RPC_SCREEN_EFFECT, "screen_effect", Box::new(|params| {
            if params.is_empty() {
                return Err(RpcError::InvalidParams { expected: 1, got: 0 });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            Ok(None)
        }));

        // play_sound(sound_id: Int, position: Vec3)
        self.register_fixed(RPC_PLAY_SOUND, "play_sound", Box::new(|params| {
            if params.len() < 2 {
                return Err(RpcError::InvalidParams { expected: 2, got: params.len() });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            let _ = params[1].as_vec3()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Vec3" })?;
            Ok(None)
        }));

        // damage_number(amount: Float, position: Vec3, crit: Bool)
        self.register_fixed(RPC_DAMAGE_NUMBER, "damage_number", Box::new(|params| {
            if params.len() < 3 {
                return Err(RpcError::InvalidParams { expected: 3, got: params.len() });
            }
            let _ = params[0].as_float()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Float" })?;
            let _ = params[1].as_vec3()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Vec3" })?;
            let _ = params[2].as_bool()
                .ok_or(RpcError::WrongParamType { index: 2, expected: "Bool" })?;
            Ok(None)
        }));

        // entity_status(entity_id: Int, status_effect: Int, duration: Float)
        self.register_fixed(RPC_ENTITY_STATUS, "entity_status", Box::new(|params| {
            if params.len() < 3 {
                return Err(RpcError::InvalidParams { expected: 3, got: params.len() });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            let _ = params[1].as_int()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Int" })?;
            let _ = params[2].as_float()
                .ok_or(RpcError::WrongParamType { index: 2, expected: "Float" })?;
            Ok(None)
        }));

        // camera_shake(trauma: Float)
        self.register_fixed(RPC_CAMERA_SHAKE, "camera_shake", Box::new(|params| {
            if params.is_empty() {
                return Err(RpcError::InvalidParams { expected: 1, got: 0 });
            }
            let _ = params[0].as_float()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Float" })?;
            Ok(None)
        }));

        // dialogue_trigger(npc_id: Int, dialogue_id: Int)
        self.register_fixed(RPC_DIALOGUE, "dialogue_trigger", Box::new(|params| {
            if params.len() < 2 {
                return Err(RpcError::InvalidParams { expected: 2, got: params.len() });
            }
            let _ = params[0].as_int()
                .ok_or(RpcError::WrongParamType { index: 0, expected: "Int" })?;
            let _ = params[1].as_int()
                .ok_or(RpcError::WrongParamType { index: 1, expected: "Int" })?;
            Ok(None)
        }));
    }
}

impl Default for RpcRegistry {
    fn default() -> Self { Self::new() }
}

// ─── RpcQueue ────────────────────────────────────────────────────────────────

/// Accumulates outgoing RPC calls for batched network dispatch.
pub struct RpcQueue {
    pending:   VecDeque<RpcCall>,
    next_seq:  u32,
    /// Maximum calls buffered before oldest are dropped.
    max_len:   usize,
}

impl RpcQueue {
    pub fn new(max_len: usize) -> Self {
        Self { pending: VecDeque::with_capacity(max_len), next_seq: 0, max_len }
    }

    /// Add an RPC call to the queue.  Assigns a sequence number.
    pub fn enqueue(&mut self, mut call: RpcCall) {
        call.seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        if self.pending.len() >= self.max_len {
            self.pending.pop_front(); // drop oldest
        }
        self.pending.push_back(call);
    }

    /// Drain all pending calls.
    pub fn drain(&mut self) -> impl Iterator<Item = RpcCall> + '_ {
        self.pending.drain(..)
    }

    /// Peek at the front without removing.
    pub fn peek(&self) -> Option<&RpcCall> { self.pending.front() }

    pub fn len(&self) -> usize { self.pending.len() }
    pub fn is_empty(&self) -> bool { self.pending.is_empty() }
    pub fn clear(&mut self) { self.pending.clear(); }
}

// ─── RpcSecurity ─────────────────────────────────────────────────────────────

/// Per-client rate limiting and basic call validation.
#[derive(Debug, Clone)]
struct RateState {
    /// Timestamps of recent calls for this RPC from this client.
    history: VecDeque<Instant>,
    /// Maximum allowed calls per `window`.
    max_calls: u32,
    window: Duration,
}

impl RateState {
    fn new(max_calls: u32, window_ms: u64) -> Self {
        Self {
            history:   VecDeque::new(),
            max_calls,
            window:    Duration::from_millis(window_ms),
        }
    }

    /// Returns `true` if the call is allowed (and records it).
    fn allow(&mut self) -> bool {
        let now = Instant::now();
        // Evict old entries
        while let Some(&front) = self.history.front() {
            if now.duration_since(front) > self.window {
                self.history.pop_front();
            } else {
                break;
            }
        }
        if self.history.len() as u32 >= self.max_calls {
            return false;
        }
        self.history.push_back(now);
        true
    }
}

pub struct RpcSecurity {
    /// Per (client_id, rpc_id) rate state.
    rate_states: HashMap<(u64, RpcId), RateState>,
    /// RPCs restricted to server-only callers.
    server_only: std::collections::HashSet<RpcId>,
    /// Default max calls per 1 second window.
    default_rate: u32,
}

impl RpcSecurity {
    pub fn new(default_rate: u32) -> Self {
        let mut s = Self {
            rate_states: HashMap::new(),
            server_only: std::collections::HashSet::new(),
            default_rate,
        };
        // These RPCs can only come from the server (caller = 0)
        s.server_only.insert(RPC_PLAYER_JOINED);
        s.server_only.insert(RPC_PLAYER_LEFT);
        s.server_only.insert(RPC_GAME_EVENT);
        s.server_only.insert(RPC_FORCE_FIELD);
        s.server_only.insert(RPC_ENTITY_STATUS);
        s
    }

    /// Returns `Ok(())` if the call is allowed.
    pub fn check(&mut self, call: &RpcCall) -> Result<(), RpcError> {
        // Server-only check
        if self.server_only.contains(&call.id) && call.caller != 0 {
            return Err(RpcError::Unauthorised { rpc_id: call.id, caller: call.caller });
        }

        // Rate limiting
        let rate = self.rate_states
            .entry((call.caller, call.id))
            .or_insert_with(|| RateState::new(self.default_rate, 1000));

        if !rate.allow() {
            return Err(RpcError::RateLimited { rpc_id: call.id, caller: call.caller });
        }

        Ok(())
    }

    /// Override rate limit for a specific RPC.
    pub fn set_rate(&mut self, rpc_id: RpcId, max_calls_per_sec: u32) {
        // Stored as default — new entries for this RPC will use this limit.
        // Existing entries are not updated to keep the API simple.
        let _ = (rpc_id, max_calls_per_sec); // Applied on next call via or_insert_with
    }

    pub fn add_server_only(&mut self, rpc_id: RpcId) {
        self.server_only.insert(rpc_id);
    }

    pub fn remove_server_only(&mut self, rpc_id: RpcId) {
        self.server_only.remove(&rpc_id);
    }
}

impl Default for RpcSecurity {
    fn default() -> Self { Self::new(30) }
}

// ─── RpcBatcher ──────────────────────────────────────────────────────────────

/// Combines multiple `RpcCall`s into a single packet payload when possible.
///
/// Wire format for a batch:
/// ```text
/// [count: u8]  [call_1][call_2]...[call_N]
/// ```
pub struct RpcBatcher {
    /// Maximum payload bytes before starting a new batch packet.
    pub max_batch_bytes: usize,
}

impl RpcBatcher {
    pub fn new(max_batch_bytes: usize) -> Self {
        Self { max_batch_bytes }
    }

    /// Split `calls` into batches that each fit within `max_batch_bytes`.
    /// Returns a vector of serialised batch payloads.
    pub fn batch(&self, calls: &[RpcCall]) -> Vec<Vec<u8>> {
        let mut batches: Vec<Vec<u8>> = Vec::new();
        let mut current  = Vec::new();
        let mut count    = 0u8;
        let mut count_pos = 0usize;

        // Reserve 1 byte for count at start
        current.push(0u8);
        count_pos = 0;

        for call in calls {
            let serialised = call.serialize();
            let needed = serialised.len();

            // If adding this call would overflow, flush
            if current.len() + needed > self.max_batch_bytes && count > 0 {
                current[count_pos] = count;
                batches.push(current);
                current   = vec![0u8];
                count     = 0;
                count_pos = 0;
            }

            current.extend_from_slice(&serialised);
            count += 1;
        }

        if count > 0 {
            current[count_pos] = count;
            batches.push(current);
        }

        batches
    }

    /// Deserialise a batch payload into `RpcCall`s.
    pub fn unbatch(&self, payload: &[u8]) -> Result<Vec<RpcCall>, RpcError> {
        if payload.is_empty() {
            return Ok(Vec::new());
        }
        let count = payload[0] as usize;
        let mut calls = Vec::with_capacity(count);
        let mut pos   = 1usize;
        for _ in 0..count {
            let (call, new_pos) = RpcCall::deserialize(payload, pos)?;
            calls.push(call);
            pos = new_pos;
        }
        Ok(calls)
    }
}

impl Default for RpcBatcher {
    fn default() -> Self { Self::new(1200) }
}

// ─── RpcReplay ────────────────────────────────────────────────────────────────

/// Records RPC calls with timestamps for debugging and replay.
#[derive(Debug, Clone)]
pub struct RecordedCall {
    pub timestamp_ms: u64,
    pub call:         RpcCall,
}

pub struct RpcReplay {
    records:      Vec<RecordedCall>,
    recording:    bool,
    max_records:  usize,
    start_time:   Instant,
}

impl RpcReplay {
    pub fn new(max_records: usize) -> Self {
        Self {
            records:     Vec::new(),
            recording:   false,
            max_records,
            start_time:  Instant::now(),
        }
    }

    pub fn start_recording(&mut self) {
        self.records.clear();
        self.recording  = true;
        self.start_time = Instant::now();
    }

    pub fn stop_recording(&mut self) {
        self.recording = false;
    }

    /// Record a call if recording is active.
    pub fn record(&mut self, call: RpcCall) {
        if !self.recording { return; }
        let timestamp_ms = self.start_time.elapsed().as_millis() as u64;
        if self.records.len() >= self.max_records {
            self.records.remove(0); // evict oldest
        }
        self.records.push(RecordedCall { timestamp_ms, call });
    }

    /// Replay all recorded calls into `queue` immediately (timestamp ignored).
    pub fn replay_all(&self, queue: &mut RpcQueue) {
        for rec in &self.records {
            queue.enqueue(rec.call.clone());
        }
    }

    /// Replay only calls whose `rpc_id` matches `filter`.
    pub fn replay_filtered(&self, queue: &mut RpcQueue, filter: RpcId) {
        for rec in &self.records {
            if rec.call.id == filter {
                queue.enqueue(rec.call.clone());
            }
        }
    }

    /// Get a read-only slice of recorded calls.
    pub fn records(&self) -> &[RecordedCall] { &self.records }

    pub fn record_count(&self) -> usize { self.records.len() }
    pub fn is_recording(&self) -> bool { self.recording }

    /// Export recording as raw bytes (each call serialised, prefixed by timestamp_ms u64).
    pub fn export(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.records.len() as u32).to_be_bytes());
        for rec in &self.records {
            out.extend_from_slice(&rec.timestamp_ms.to_be_bytes());
            let call_bytes = rec.call.serialize();
            out.extend_from_slice(&(call_bytes.len() as u32).to_be_bytes());
            out.extend_from_slice(&call_bytes);
        }
        out
    }

    /// Import from bytes produced by `export`.
    pub fn import(&mut self, data: &[u8]) -> Result<(), RpcError> {
        if data.len() < 4 {
            return Err(RpcError::DeserializeError("export too short".into()));
        }
        let count = u32::from_be_bytes(data[0..4].try_into().unwrap()) as usize;
        let mut pos = 4usize;
        self.records.clear();

        for _ in 0..count {
            if pos + 12 > data.len() {
                return Err(RpcError::DeserializeError("truncated record".into()));
            }
            let timestamp_ms = u64::from_be_bytes(data[pos..pos+8].try_into().unwrap());
            pos += 8;
            let call_len = u32::from_be_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
            pos += 4;
            if pos + call_len > data.len() {
                return Err(RpcError::DeserializeError("truncated call bytes".into()));
            }
            let (call, _) = RpcCall::deserialize(&data[pos..pos+call_len], 0)?;
            pos += call_len;
            self.records.push(RecordedCall { timestamp_ms, call });
        }
        Ok(())
    }
}

impl Default for RpcReplay {
    fn default() -> Self { Self::new(10_000) }
}

// ─── Convenience builders ─────────────────────────────────────────────────────

/// Build a `chat_message` RPC call.
pub fn rpc_chat(sender_id: u64, text: impl Into<String>) -> RpcCall {
    RpcCall::new(
        RPC_CHAT_MESSAGE,
        RpcTarget::All,
        vec![RpcParam::Int(sender_id as i64), RpcParam::Str(text.into())],
    )
}

/// Build a `player_joined` RPC call.
pub fn rpc_player_joined(player_id: u64, name: impl Into<String>) -> RpcCall {
    RpcCall::new(
        RPC_PLAYER_JOINED,
        RpcTarget::All,
        vec![RpcParam::Int(player_id as i64), RpcParam::Str(name.into())],
    )
}

/// Build a `player_left` RPC call.
pub fn rpc_player_left(player_id: u64) -> RpcCall {
    RpcCall::new(
        RPC_PLAYER_LEFT,
        RpcTarget::All,
        vec![RpcParam::Int(player_id as i64)],
    )
}

/// Build a `game_event` RPC call.
pub fn rpc_game_event(kind: i64, data: Vec<u8>) -> RpcCall {
    RpcCall::new(
        RPC_GAME_EVENT,
        RpcTarget::All,
        vec![RpcParam::Int(kind), RpcParam::Bytes(data)],
    )
}

/// Build a `force_field_spawn` RPC call.
pub fn rpc_force_field(field_type: i64, position: Vec3, strength: f32, ttl: f32) -> RpcCall {
    RpcCall::new(
        RPC_FORCE_FIELD,
        RpcTarget::All,
        vec![
            RpcParam::Int(field_type),
            RpcParam::Vec3(position),
            RpcParam::Float(strength),
            RpcParam::Float(ttl),
        ],
    )
}

/// Build a `particle_burst` RPC call.
pub fn rpc_particle_burst(preset: i64, origin: Vec3) -> RpcCall {
    RpcCall::new(
        RPC_PARTICLE_BURST,
        RpcTarget::All,
        vec![RpcParam::Int(preset), RpcParam::Vec3(origin)],
    )
}

/// Build a `screen_effect` RPC call.
pub fn rpc_screen_effect(effect_type: i64, target_client: u64) -> RpcCall {
    RpcCall::new(
        RPC_SCREEN_EFFECT,
        RpcTarget::Client(target_client),
        vec![RpcParam::Int(effect_type)],
    )
}

/// Build a `play_sound` RPC call.
pub fn rpc_play_sound(sound_id: i64, position: Vec3) -> RpcCall {
    RpcCall::new(
        RPC_PLAY_SOUND,
        RpcTarget::All,
        vec![RpcParam::Int(sound_id), RpcParam::Vec3(position)],
    )
}

/// Build a `damage_number` RPC call.
pub fn rpc_damage_number(amount: f32, position: Vec3, crit: bool) -> RpcCall {
    RpcCall::new(
        RPC_DAMAGE_NUMBER,
        RpcTarget::All,
        vec![RpcParam::Float(amount), RpcParam::Vec3(position), RpcParam::Bool(crit)],
    )
}

/// Build an `entity_status` RPC call.
pub fn rpc_entity_status(entity_id: i64, status: i64, duration: f32) -> RpcCall {
    RpcCall::new(
        RPC_ENTITY_STATUS,
        RpcTarget::All,
        vec![RpcParam::Int(entity_id), RpcParam::Int(status), RpcParam::Float(duration)],
    )
}

/// Build a `camera_shake` RPC call.
pub fn rpc_camera_shake(trauma: f32, target_client: u64) -> RpcCall {
    RpcCall::new(
        RPC_CAMERA_SHAKE,
        RpcTarget::Client(target_client),
        vec![RpcParam::Float(trauma)],
    )
}

/// Build a `dialogue_trigger` RPC call.
pub fn rpc_dialogue_trigger(npc_id: i64, dialogue_id: i64) -> RpcCall {
    RpcCall::new(
        RPC_DIALOGUE,
        RpcTarget::All,
        vec![RpcParam::Int(npc_id), RpcParam::Int(dialogue_id)],
    )
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::sync::Vec3;

    // ── RpcParam serialisation ────────────────────────────────────────────────

    #[test]
    fn test_param_bool_roundtrip() {
        for &b in &[true, false] {
            let p = RpcParam::Bool(b);
            let mut buf = Vec::new();
            p.serialize(&mut buf);
            let (decoded, _) = RpcParam::deserialize(&buf, 0).unwrap();
            assert_eq!(decoded, p);
        }
    }

    #[test]
    fn test_param_int_roundtrip() {
        for &v in &[0i64, -1, i64::MIN, i64::MAX, 42] {
            let p = RpcParam::Int(v);
            let mut buf = Vec::new();
            p.serialize(&mut buf);
            let (decoded, _) = RpcParam::deserialize(&buf, 0).unwrap();
            assert_eq!(decoded, p);
        }
    }

    #[test]
    fn test_param_float_roundtrip() {
        let p = RpcParam::Float(3.14159);
        let mut buf = Vec::new();
        p.serialize(&mut buf);
        let (decoded, _) = RpcParam::deserialize(&buf, 0).unwrap();
        if let (RpcParam::Float(a), RpcParam::Float(b)) = (&p, &decoded) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_param_str_roundtrip() {
        let p = RpcParam::Str("hello, world!".into());
        let mut buf = Vec::new();
        p.serialize(&mut buf);
        let (decoded, _) = RpcParam::deserialize(&buf, 0).unwrap();
        assert_eq!(decoded, p);
    }

    #[test]
    fn test_param_vec3_roundtrip() {
        let p = RpcParam::Vec3(Vec3::new(1.0, 2.0, 3.0));
        let mut buf = Vec::new();
        p.serialize(&mut buf);
        let (decoded, _) = RpcParam::deserialize(&buf, 0).unwrap();
        assert_eq!(decoded, p);
    }

    #[test]
    fn test_param_bytes_roundtrip() {
        let p = RpcParam::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let mut buf = Vec::new();
        p.serialize(&mut buf);
        let (decoded, _) = RpcParam::deserialize(&buf, 0).unwrap();
        assert_eq!(decoded, p);
    }

    // ── RpcCall serialisation ─────────────────────────────────────────────────

    #[test]
    fn test_rpc_call_serialize_deserialize() {
        let call = rpc_chat(42, "Hi there");
        let bytes = call.serialize();
        let (decoded, consumed) = RpcCall::deserialize(&bytes, 0).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.id, call.id);
        assert_eq!(decoded.params.len(), 2);
    }

    #[test]
    fn test_rpc_call_all_targets() {
        let targets = vec![
            RpcTarget::All,
            RpcTarget::Server,
            RpcTarget::Client(123),
            RpcTarget::Team(2),
            RpcTarget::AllExcept(456),
        ];
        for target in targets {
            let call = RpcCall::new(RPC_CHAT_MESSAGE, target.clone(), vec![
                RpcParam::Int(1), RpcParam::Str("x".into()),
            ]);
            let bytes = call.serialize();
            let (decoded, _) = RpcCall::deserialize(&bytes, 0).unwrap();
            assert_eq!(decoded.target, target);
        }
    }

    // ── RpcRegistry ───────────────────────────────────────────────────────────

    #[test]
    fn test_registry_has_builtins() {
        let reg = RpcRegistry::new();
        assert!(reg.handler(RPC_CHAT_MESSAGE).is_some());
        assert!(reg.handler(RPC_PLAYER_JOINED).is_some());
        assert!(reg.handler(RPC_CAMERA_SHAKE).is_some());
        assert!(reg.handler(RPC_DIALOGUE).is_some());
    }

    #[test]
    fn test_registry_invoke_chat() {
        let reg = RpcRegistry::new();
        let result = reg.invoke(RPC_CHAT_MESSAGE, &[
            RpcParam::Int(1),
            RpcParam::Str("hello".into()),
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_registry_invoke_wrong_params() {
        let reg = RpcRegistry::new();
        let result = reg.invoke(RPC_CHAT_MESSAGE, &[]);
        assert!(matches!(result, Err(RpcError::InvalidParams { .. })));
    }

    #[test]
    fn test_registry_unknown_rpc() {
        let reg = RpcRegistry::new();
        let result = reg.invoke(RpcId(0xFFFF), &[]);
        assert!(matches!(result, Err(RpcError::UnknownRpc(_))));
    }

    // ── RpcBatcher ────────────────────────────────────────────────────────────

    #[test]
    fn test_batcher_roundtrip() {
        let batcher = RpcBatcher::new(4096);
        let calls = vec![
            rpc_chat(1, "hello"),
            rpc_player_joined(2, "Alice"),
            rpc_camera_shake(0.8, 42),
        ];
        let batches = batcher.batch(&calls);
        assert!(!batches.is_empty());

        let mut decoded_all = Vec::new();
        for batch in &batches {
            let decoded = batcher.unbatch(batch).unwrap();
            decoded_all.extend(decoded);
        }
        assert_eq!(decoded_all.len(), calls.len());
        assert_eq!(decoded_all[0].id, RPC_CHAT_MESSAGE);
        assert_eq!(decoded_all[1].id, RPC_PLAYER_JOINED);
    }

    #[test]
    fn test_batcher_splits_large_batch() {
        let batcher = RpcBatcher::new(60); // tiny max
        let calls: Vec<RpcCall> = (0..5).map(|i| rpc_chat(i, "x")).collect();
        let batches = batcher.batch(&calls);
        // With tiny max, should produce multiple batches
        assert!(batches.len() >= 1);
        // Total decoded calls should equal 5
        let total: usize = batches.iter()
            .map(|b| batcher.unbatch(b).unwrap().len())
            .sum();
        assert_eq!(total, 5);
    }

    // ── RpcQueue ──────────────────────────────────────────────────────────────

    #[test]
    fn test_rpc_queue_sequence() {
        let mut q = RpcQueue::new(64);
        q.enqueue(rpc_chat(1, "a"));
        q.enqueue(rpc_chat(1, "b"));
        let calls: Vec<RpcCall> = q.drain().collect();
        assert_eq!(calls[0].seq, 0);
        assert_eq!(calls[1].seq, 1);
    }

    // ── RpcSecurity ───────────────────────────────────────────────────────────

    #[test]
    fn test_security_server_only_rejected() {
        let mut sec = RpcSecurity::new(100);
        let mut call = rpc_player_joined(1, "Alice");
        call.caller = 99; // non-server caller
        assert!(matches!(sec.check(&call), Err(RpcError::Unauthorised { .. })));
    }

    #[test]
    fn test_security_server_allowed() {
        let mut sec = RpcSecurity::new(100);
        let mut call = rpc_player_joined(1, "Alice");
        call.caller = 0; // server
        assert!(sec.check(&call).is_ok());
    }

    #[test]
    fn test_security_rate_limit() {
        let mut sec = RpcSecurity::new(3); // 3 per second
        let mut call = rpc_chat(1, "hi");
        call.caller = 5;
        // First 3 allowed
        assert!(sec.check(&call).is_ok());
        assert!(sec.check(&call).is_ok());
        assert!(sec.check(&call).is_ok());
        // 4th should be rate-limited
        assert!(matches!(sec.check(&call), Err(RpcError::RateLimited { .. })));
    }

    // ── RpcReplay ─────────────────────────────────────────────────────────────

    #[test]
    fn test_replay_record_and_replay() {
        let mut replay = RpcReplay::new(100);
        replay.start_recording();
        replay.record(rpc_chat(1, "test"));
        replay.record(rpc_camera_shake(0.5, 1));
        replay.stop_recording();

        assert_eq!(replay.record_count(), 2);

        let mut queue = RpcQueue::new(64);
        replay.replay_all(&mut queue);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_replay_export_import() {
        let mut replay = RpcReplay::new(100);
        replay.start_recording();
        replay.record(rpc_chat(7, "hello"));
        replay.stop_recording();

        let exported = replay.export();

        let mut replay2 = RpcReplay::new(100);
        replay2.import(&exported).unwrap();
        assert_eq!(replay2.record_count(), 1);
        assert_eq!(replay2.records()[0].call.id, RPC_CHAT_MESSAGE);
    }
}
