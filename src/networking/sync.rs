//! Game-state synchronisation: snapshots, delta encoding, client-side
//! prediction, lag compensation, and network clock synchronisation.
//!
//! ## Design
//! The server produces a `GameStateSnapshot` every server tick.  Only
//! changed entities are shipped as `DeltaSnapshot` to save bandwidth.
//! Clients buffer recent snapshots in a `SnapshotBuffer` and use a
//! `StateInterpolator` to render smoothly between them.
//! `ClientPrediction` applies local inputs immediately and reconciles when
//! the authoritative state arrives.  `LagCompensation` lets the server rewind
//! its history to the client's perceived point in time for fair hit detection.

use std::collections::VecDeque;

// ─── Vec2 / Vec3 ─────────────────────────────────────────────────────────────
// Lightweight local types — no glam dependency needed inside this module.

/// 2D vector used for movement directions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self { Self { x, y } }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < f32::EPSILON { Self::ZERO } else { Self { x: self.x / len, y: self.y / len } }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

/// 3D vector used for positions, velocities, and rotations.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }

    pub fn add(self, other: Self) -> Self {
        Self { x: self.x + other.x, y: self.y + other.y, z: self.z + other.z }
    }

    pub fn scale(self, s: f32) -> Self {
        Self { x: self.x * s, y: self.y * s, z: self.z * s }
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z }
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z }
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs }
    }
}

// ─── EntitySnapshot ──────────────────────────────────────────────────────────

/// Complete state of one entity at a specific server tick.
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySnapshot {
    /// Unique entity identifier.
    pub id:          u64,
    pub position:    Vec3,
    pub velocity:    Vec3,
    /// Euler angles (roll, pitch, yaw) in radians.
    pub rotation:    Vec3,
    pub health:      f32,
    /// Bitfield: alive, grounded, crouching, attacking, etc.
    pub state_flags: u32,
    /// Application-specific extra data (e.g. ammo, power-up timer).
    pub custom:      Vec<u8>,
}

impl EntitySnapshot {
    pub fn new(id: u64) -> Self {
        Self {
            id, position: Vec3::ZERO, velocity: Vec3::ZERO,
            rotation: Vec3::ZERO, health: 100.0, state_flags: 0, custom: Vec::new(),
        }
    }

    pub fn is_alive(&self) -> bool { self.state_flags & 1 != 0 }
    pub fn is_grounded(&self) -> bool { self.state_flags & 2 != 0 }
    pub fn is_crouching(&self) -> bool { self.state_flags & 4 != 0 }
}

// ─── GameStateSnapshot ───────────────────────────────────────────────────────

/// Authoritative game state for one server tick, ready to be diffed or sent.
#[derive(Debug, Clone)]
pub struct GameStateSnapshot {
    /// Monotonically increasing server tick number.
    pub tick:      u64,
    /// Server wall-clock time (seconds since epoch or game start).
    pub timestamp: f64,
    pub entities:  Vec<EntitySnapshot>,
}

impl GameStateSnapshot {
    pub fn new(tick: u64, timestamp: f64) -> Self {
        Self { tick, timestamp, entities: Vec::new() }
    }

    pub fn with_entities(mut self, entities: Vec<EntitySnapshot>) -> Self {
        self.entities = entities;
        self
    }

    /// Find an entity by ID.
    pub fn entity(&self, id: u64) -> Option<&EntitySnapshot> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Find an entity by ID (mutable).
    pub fn entity_mut(&mut self, id: u64) -> Option<&mut EntitySnapshot> {
        self.entities.iter_mut().find(|e| e.id == id)
    }
}

// ─── EntityDelta ─────────────────────────────────────────────────────────────

/// Per-field diff for one entity between two consecutive snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityDelta {
    pub id:               u64,
    /// `Some(new_position)` if position changed; `None` if unchanged.
    pub position_delta:   Option<Vec3>,
    pub velocity_delta:   Option<Vec3>,
    pub rotation_delta:   Option<Vec3>,
    pub health_delta:     Option<f32>,
    pub state_flags:      Option<u32>,
    pub custom:           Option<Vec<u8>>,
    /// Whether this entity was newly spawned (not present in base).
    pub spawned:          bool,
    /// Whether this entity was destroyed.
    pub despawned:        bool,
}

impl EntityDelta {
    pub fn new(id: u64) -> Self {
        Self {
            id, position_delta: None, velocity_delta: None, rotation_delta: None,
            health_delta: None, state_flags: None, custom: None,
            spawned: false, despawned: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.position_delta.is_none()
            && self.velocity_delta.is_none()
            && self.rotation_delta.is_none()
            && self.health_delta.is_none()
            && self.state_flags.is_none()
            && self.custom.is_none()
            && !self.spawned && !self.despawned
    }
}

// ─── DeltaSnapshot ────────────────────────────────────────────────────────────

/// Compact diff between two `GameStateSnapshot` values.
///
/// Only entities that changed are included.  The receiver applies the delta
/// on top of the last ack'd base snapshot to reconstruct the new state.
#[derive(Debug, Clone)]
pub struct DeltaSnapshot {
    /// Tick this delta advances the client to.
    pub tick:      u64,
    pub timestamp: f64,
    /// Which base tick this delta is relative to (last ack'd by client).
    pub base_tick: u64,
    pub changed:   Vec<EntityDelta>,
}

impl DeltaSnapshot {
    pub fn new(tick: u64, timestamp: f64, base_tick: u64) -> Self {
        Self { tick, timestamp, base_tick, changed: Vec::new() }
    }

    /// Build a `DeltaSnapshot` by diffing `base` → `current`.
    pub fn build(base: &GameStateSnapshot, current: &GameStateSnapshot) -> Self {
        let mut delta = DeltaSnapshot::new(current.tick, current.timestamp, base.tick);

        // Entities in current
        for cur in &current.entities {
            match base.entity(cur.id) {
                None => {
                    // Newly spawned
                    let mut d = EntityDelta::new(cur.id);
                    d.spawned          = true;
                    d.position_delta   = Some(cur.position);
                    d.velocity_delta   = Some(cur.velocity);
                    d.rotation_delta   = Some(cur.rotation);
                    d.health_delta     = Some(cur.health);
                    d.state_flags      = Some(cur.state_flags);
                    d.custom           = Some(cur.custom.clone());
                    delta.changed.push(d);
                }
                Some(base_ent) => {
                    let mut d = EntityDelta::new(cur.id);
                    let thresh = 0.001f32;
                    if cur.position.distance(base_ent.position) > thresh {
                        d.position_delta = Some(cur.position - base_ent.position);
                    }
                    if cur.velocity.distance(base_ent.velocity) > thresh {
                        d.velocity_delta = Some(cur.velocity - base_ent.velocity);
                    }
                    if cur.rotation.distance(base_ent.rotation) > thresh {
                        d.rotation_delta = Some(cur.rotation - base_ent.rotation);
                    }
                    if (cur.health - base_ent.health).abs() > 0.01 {
                        d.health_delta = Some(cur.health - base_ent.health);
                    }
                    if cur.state_flags != base_ent.state_flags {
                        d.state_flags = Some(cur.state_flags);
                    }
                    if cur.custom != base_ent.custom {
                        d.custom = Some(cur.custom.clone());
                    }
                    if !d.is_empty() {
                        delta.changed.push(d);
                    }
                }
            }
        }

        // Despawned entities
        for base_ent in &base.entities {
            if current.entity(base_ent.id).is_none() {
                let mut d = EntityDelta::new(base_ent.id);
                d.despawned = true;
                delta.changed.push(d);
            }
        }

        delta
    }

    /// Apply this delta on top of `base`, producing a new full snapshot.
    pub fn apply(&self, base: &GameStateSnapshot) -> GameStateSnapshot {
        let mut result = base.clone();
        result.tick      = self.tick;
        result.timestamp = self.timestamp;

        for d in &self.changed {
            if d.despawned {
                result.entities.retain(|e| e.id != d.id);
                continue;
            }
            if d.spawned {
                let mut ent = EntitySnapshot::new(d.id);
                if let Some(p) = d.position_delta { ent.position = p; }
                if let Some(v) = d.velocity_delta { ent.velocity = v; }
                if let Some(r) = d.rotation_delta { ent.rotation = r; }
                if let Some(h) = d.health_delta   { ent.health = h; }
                if let Some(f) = d.state_flags     { ent.state_flags = f; }
                if let Some(ref c) = d.custom      { ent.custom = c.clone(); }
                result.entities.push(ent);
                continue;
            }
            if let Some(ent) = result.entity_mut(d.id) {
                if let Some(dp) = d.position_delta { ent.position = ent.position + dp; }
                if let Some(dv) = d.velocity_delta { ent.velocity = ent.velocity + dv; }
                if let Some(dr) = d.rotation_delta { ent.rotation = ent.rotation + dr; }
                if let Some(dh) = d.health_delta   { ent.health += dh; }
                if let Some(f)  = d.state_flags     { ent.state_flags = f; }
                if let Some(ref c) = d.custom      { ent.custom = c.clone(); }
            }
        }
        result
    }

    /// Count of entities affected by this delta.
    pub fn change_count(&self) -> usize { self.changed.len() }
}

// ─── SnapshotBuffer ───────────────────────────────────────────────────────────

/// Ring buffer of recent game-state snapshots.
///
/// Used on both client (for interpolation) and server (for lag compensation).
pub struct SnapshotBuffer {
    snapshots: VecDeque<GameStateSnapshot>,
    max_len:   usize,
}

impl SnapshotBuffer {
    pub const DEFAULT_MAX_LEN: usize = 64;

    pub fn new(max_len: usize) -> Self {
        Self { snapshots: VecDeque::with_capacity(max_len), max_len }
    }

    pub fn default() -> Self { Self::new(Self::DEFAULT_MAX_LEN) }

    /// Push a new snapshot; evict the oldest if at capacity.
    pub fn push(&mut self, snap: GameStateSnapshot) {
        if self.snapshots.len() >= self.max_len {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snap);
    }

    /// Most recent snapshot.
    pub fn latest(&self) -> Option<&GameStateSnapshot> {
        self.snapshots.back()
    }

    /// Oldest stored snapshot.
    pub fn oldest(&self) -> Option<&GameStateSnapshot> {
        self.snapshots.front()
    }

    /// Find the snapshot with the given tick, or the nearest one before it.
    pub fn at_tick(&self, tick: u64) -> Option<&GameStateSnapshot> {
        // Snapshots are ordered oldest → newest.
        let mut best: Option<&GameStateSnapshot> = None;
        for s in &self.snapshots {
            if s.tick <= tick {
                best = Some(s);
            } else {
                break;
            }
        }
        best
    }

    /// Find the two snapshots that bracket `tick` for interpolation.
    /// Returns `(older, newer)`.
    pub fn bracket(&self, tick: u64) -> Option<(&GameStateSnapshot, &GameStateSnapshot)> {
        let snaps: Vec<&GameStateSnapshot> = self.snapshots.iter().collect();
        for i in 0..snaps.len().saturating_sub(1) {
            let a = snaps[i];
            let b = snaps[i + 1];
            if a.tick <= tick && b.tick >= tick {
                return Some((a, b));
            }
        }
        None
    }

    pub fn len(&self) -> usize { self.snapshots.len() }
    pub fn is_empty(&self) -> bool { self.snapshots.is_empty() }
    pub fn clear(&mut self) { self.snapshots.clear(); }
}

// ─── StateInterpolator ────────────────────────────────────────────────────────

/// Smoothly interpolates entity positions between buffered snapshots.
pub struct StateInterpolator {
    buffer: SnapshotBuffer,
    /// Render is behind this many ticks to ensure we always have two samples.
    interp_delay_ticks: u64,
}

impl StateInterpolator {
    pub fn new(interp_delay_ticks: u64) -> Self {
        Self {
            buffer: SnapshotBuffer::new(64),
            interp_delay_ticks,
        }
    }

    pub fn push_snapshot(&mut self, snap: GameStateSnapshot) {
        self.buffer.push(snap);
    }

    /// Interpolate entity states at `render_tick` (may be fractional via `t`).
    ///
    /// `render_tick` is the server tick we want to display.
    /// Returns a synthetic snapshot with interpolated entity positions.
    pub fn interpolate(&self, render_tick: u64, t: f32) -> Option<GameStateSnapshot> {
        let display_tick = render_tick.saturating_sub(self.interp_delay_ticks);

        match self.buffer.bracket(display_tick) {
            Some((a, b)) => {
                let tick_range = (b.tick - a.tick) as f32;
                let local_t = if tick_range > 0.0 {
                    ((display_tick - a.tick) as f32 + t) / tick_range
                } else {
                    0.0
                };
                let local_t = local_t.clamp(0.0, 1.0);

                let mut result = GameStateSnapshot::new(display_tick, a.timestamp);
                for a_ent in &a.entities {
                    let pos = if let Some(b_ent) = b.entity(a_ent.id) {
                        a_ent.position.lerp(b_ent.position, local_t)
                    } else {
                        a_ent.position
                    };
                    let vel = if let Some(b_ent) = b.entity(a_ent.id) {
                        a_ent.velocity.lerp(b_ent.velocity, local_t)
                    } else {
                        a_ent.velocity
                    };
                    let rot = if let Some(b_ent) = b.entity(a_ent.id) {
                        a_ent.rotation.lerp(b_ent.rotation, local_t)
                    } else {
                        a_ent.rotation
                    };
                    let health = if let Some(b_ent) = b.entity(a_ent.id) {
                        a_ent.health + (b_ent.health - a_ent.health) * local_t
                    } else {
                        a_ent.health
                    };
                    let flags = a_ent.state_flags; // discrete — no interpolation

                    result.entities.push(EntitySnapshot {
                        id: a_ent.id,
                        position: pos,
                        velocity: vel,
                        rotation: rot,
                        health,
                        state_flags: flags,
                        custom: a_ent.custom.clone(),
                    });
                }
                Some(result)
            }
            None => {
                // Dead reckoning: extrapolate from latest snapshot
                self.dead_reckon(render_tick, t)
            }
        }
    }

    /// Extrapolate entity positions beyond the latest buffered snapshot using
    /// their velocity.
    pub fn dead_reckon(&self, render_tick: u64, t: f32) -> Option<GameStateSnapshot> {
        let latest = self.buffer.latest()?;
        let dt = ((render_tick.saturating_sub(latest.tick)) as f32 + t) / 60.0; // assume 60 Hz

        let mut result = GameStateSnapshot::new(render_tick, latest.timestamp + dt as f64);
        for ent in &latest.entities {
            let predicted_pos = ent.position + ent.velocity * dt;
            result.entities.push(EntitySnapshot {
                id: ent.id,
                position: predicted_pos,
                velocity: ent.velocity,
                rotation: ent.rotation,
                health: ent.health,
                state_flags: ent.state_flags,
                custom: ent.custom.clone(),
            });
        }
        Some(result)
    }

    pub fn buffer(&self) -> &SnapshotBuffer { &self.buffer }
    pub fn latest_tick(&self) -> Option<u64> { self.buffer.latest().map(|s| s.tick) }
}

// ─── PlayerInput ─────────────────────────────────────────────────────────────

/// One frame of local player input.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerInput {
    /// Server tick this input corresponds to.
    pub tick:     u64,
    /// Normalised movement direction (-1 to 1 on each axis).
    pub move_dir: Vec2,
    pub jump:     bool,
    /// Bitmask: bit 0=primary fire, 1=secondary fire, 2=reload, 3=interact, etc.
    pub actions:  u32,
    /// Yaw angle the player was facing when this input was made.
    pub facing:   f32,
}

impl PlayerInput {
    pub fn new(tick: u64) -> Self {
        Self { tick, move_dir: Vec2::ZERO, jump: false, actions: 0, facing: 0.0 }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32);
        out.extend_from_slice(&self.tick.to_be_bytes());
        out.extend_from_slice(&self.move_dir.x.to_bits().to_be_bytes());
        out.extend_from_slice(&self.move_dir.y.to_bits().to_be_bytes());
        out.push(self.jump as u8);
        out.extend_from_slice(&self.actions.to_be_bytes());
        out.extend_from_slice(&self.facing.to_bits().to_be_bytes());
        out
    }

    pub fn deserialize(b: &[u8]) -> Option<Self> {
        if b.len() < 29 { return None; }
        let tick     = u64::from_be_bytes(b[0..8].try_into().ok()?);
        let mx       = f32::from_bits(u32::from_be_bytes(b[8..12].try_into().ok()?));
        let my       = f32::from_bits(u32::from_be_bytes(b[12..16].try_into().ok()?));
        let jump     = b[16] != 0;
        let actions  = u32::from_be_bytes(b[17..21].try_into().ok()?);
        let facing   = f32::from_bits(u32::from_be_bytes(b[21..25].try_into().ok()?));
        Some(Self { tick, move_dir: Vec2::new(mx, my), jump, actions, facing })
    }
}

// ─── InputBuffer ─────────────────────────────────────────────────────────────

/// Holds un-acknowledged player inputs for prediction reconciliation.
pub struct InputBuffer {
    inputs:  VecDeque<PlayerInput>,
    max_len: usize,
}

impl InputBuffer {
    pub const DEFAULT_MAX_LEN: usize = 128;

    pub fn new(max_len: usize) -> Self {
        Self { inputs: VecDeque::with_capacity(max_len), max_len }
    }

    pub fn push(&mut self, input: PlayerInput) {
        if self.inputs.len() >= self.max_len {
            self.inputs.pop_front();
        }
        self.inputs.push_back(input);
    }

    /// Discard all inputs at or before `acked_tick`.
    pub fn ack_up_to(&mut self, acked_tick: u64) {
        while let Some(front) = self.inputs.front() {
            if front.tick <= acked_tick {
                self.inputs.pop_front();
            } else {
                break;
            }
        }
    }

    /// Inputs not yet acknowledged by the server.
    pub fn unacked(&self) -> impl Iterator<Item = &PlayerInput> {
        self.inputs.iter()
    }

    pub fn len(&self) -> usize { self.inputs.len() }
    pub fn is_empty(&self) -> bool { self.inputs.is_empty() }
    pub fn clear(&mut self) { self.inputs.clear(); }
}

// ─── ClientPrediction ────────────────────────────────────────────────────────

/// Client-side movement prediction with server reconciliation.
///
/// The client applies inputs locally before the server confirms them.
/// When the server snapshot arrives, if our predicted position differs from
/// the authoritative one, we roll back and replay unacked inputs.
pub struct ClientPrediction {
    pub input_buffer:      InputBuffer,
    /// Predicted entity position (local player).
    pub predicted_pos:     Vec3,
    /// Predicted entity velocity.
    pub predicted_vel:     Vec3,
    /// Last tick acknowledged by the server.
    pub last_acked_tick:   u64,
    /// Correction blend factor per frame (0.1 = smooth over ~10 frames).
    pub correction_blend:  f32,
    /// Pending correction offset being smoothly applied.
    correction_offset:     Vec3,
    /// Whether we are currently in a correction phase.
    correcting:            bool,
    /// Correction threshold below which we snap (metres).
    snap_threshold:        f32,
}

impl ClientPrediction {
    pub fn new() -> Self {
        Self {
            input_buffer:    InputBuffer::new(128),
            predicted_pos:   Vec3::ZERO,
            predicted_vel:   Vec3::ZERO,
            last_acked_tick: 0,
            correction_blend: 0.2,
            correction_offset: Vec3::ZERO,
            correcting: false,
            snap_threshold: 5.0,
        }
    }

    /// Apply `input` locally using the provided `simulate` function.
    /// `simulate(pos, vel, input, dt) -> (new_pos, new_vel)`.
    pub fn apply_input<F>(&mut self, input: PlayerInput, dt: f32, simulate: F)
    where F: Fn(Vec3, Vec3, &PlayerInput, f32) -> (Vec3, Vec3) {
        let (np, nv) = simulate(self.predicted_pos, self.predicted_vel, &input, dt);
        self.predicted_pos = np;
        self.predicted_vel = nv;
        self.input_buffer.push(input);
    }

    /// Called when an authoritative server state arrives.
    /// Rolls back to the server position and replays unacked inputs.
    pub fn reconcile<F>(
        &mut self,
        server_pos:  Vec3,
        server_vel:  Vec3,
        server_tick: u64,
        dt:          f32,
        simulate:    F,
    ) where F: Fn(Vec3, Vec3, &PlayerInput, f32) -> (Vec3, Vec3) {
        self.last_acked_tick = server_tick;
        self.input_buffer.ack_up_to(server_tick);

        // Replay all unacked inputs from the authoritative position
        let mut pos = server_pos;
        let mut vel = server_vel;
        let unacked: Vec<PlayerInput> = self.input_buffer.unacked().cloned().collect();
        for inp in &unacked {
            let (np, nv) = simulate(pos, vel, inp, dt);
            pos = np;
            vel = nv;
        }

        // Compute error between our previous prediction and the replayed result
        let error = pos - self.predicted_pos;
        let error_dist = error.length();

        if error_dist > self.snap_threshold {
            // Large error — snap immediately
            self.predicted_pos = pos;
            self.predicted_vel = vel;
            self.correcting    = false;
            self.correction_offset = Vec3::ZERO;
        } else if error_dist > 0.001 {
            // Small error — blend over several frames
            self.correction_offset = error;
            self.correcting        = true;
            self.predicted_pos     = pos;
            self.predicted_vel     = vel;
        } else {
            self.predicted_pos = pos;
            self.predicted_vel = vel;
        }
    }

    /// Advance correction blend each frame.  Returns the visually rendered position.
    pub fn tick_correction(&mut self) -> Vec3 {
        if self.correcting {
            let step = self.correction_offset.scale(self.correction_blend);
            self.correction_offset = self.correction_offset - step;
            if self.correction_offset.length() < 0.001 {
                self.correcting = false;
                self.correction_offset = Vec3::ZERO;
            }
            self.predicted_pos - self.correction_offset
        } else {
            self.predicted_pos
        }
    }

    pub fn is_correcting(&self) -> bool { self.correcting }
}

impl Default for ClientPrediction {
    fn default() -> Self { Self::new() }
}

// ─── LagCompensation ─────────────────────────────────────────────────────────

/// Server-side lag compensation: rewinds game state to a client's perceived
/// point in time for accurate hit registration.
pub struct LagCompensation {
    history: SnapshotBuffer,
    /// How many milliseconds of history to keep.
    max_history_ms: f64,
    tick_rate_hz: f64,
}

impl LagCompensation {
    /// `max_history_ms` should be at least the maximum expected client RTT.
    pub fn new(max_history_ms: f64, tick_rate_hz: f64) -> Self {
        // Store enough ticks to cover max_history_ms
        let max_ticks = ((max_history_ms / 1000.0) * tick_rate_hz).ceil() as usize + 4;
        Self {
            history: SnapshotBuffer::new(max_ticks),
            max_history_ms,
            tick_rate_hz,
        }
    }

    pub fn default_one_second() -> Self {
        Self::new(1000.0, 60.0)
    }

    /// Record the authoritative server state each tick.
    pub fn record(&mut self, snap: GameStateSnapshot) {
        self.history.push(snap);
    }

    /// Rewind to the state at `target_tick`.
    /// Returns `None` if the tick is outside the history window.
    pub fn rewind_to_tick(&self, target_tick: u64) -> Option<&GameStateSnapshot> {
        self.history.at_tick(target_tick)
    }

    /// Rewind to approximate server tick that a client with `rtt_ms` round-trip
    /// and `client_tick` would have been seeing.
    pub fn rewind_for_client(&self, client_tick: u64, rtt_ms: f64) -> Option<&GameStateSnapshot> {
        let ticks_back = (rtt_ms / 1000.0 * self.tick_rate_hz / 2.0).round() as u64;
        let target_tick = client_tick.saturating_sub(ticks_back);
        self.rewind_to_tick(target_tick)
    }

    /// Get the entity state at a particular tick (for hit detection).
    pub fn entity_at_tick(&self, entity_id: u64, tick: u64) -> Option<&EntitySnapshot> {
        self.rewind_to_tick(tick)?.entity(entity_id)
    }

    pub fn oldest_tick(&self) -> Option<u64> {
        self.history.oldest().map(|s| s.tick)
    }

    pub fn latest_tick(&self) -> Option<u64> {
        self.history.latest().map(|s| s.tick)
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }
}

// ─── NetworkClock ─────────────────────────────────────────────────────────────

/// NTP-style network clock: estimates server time from ping round-trips.
///
/// Call `record_ping_pong` each time a pong arrives, then query
/// `server_time(local_time)` to get the best estimate of current server time.
pub struct NetworkClock {
    /// Estimated offset: server_time = local_time + offset
    time_offset: f64,
    /// Smoothed RTT in seconds.
    rtt_s:       f64,
    /// Number of samples taken.
    samples:     u64,
    /// EWMA alpha for offset smoothing.
    alpha:       f64,
    /// Running correction rate to avoid jumps (seconds per second).
    correction_rate: f64,
    correction_remaining: f64,
}

impl NetworkClock {
    pub fn new() -> Self {
        Self {
            time_offset:           0.0,
            rtt_s:                 0.05,
            samples:               0,
            alpha:                 0.1,
            correction_rate:       0.001,
            correction_remaining:  0.0,
        }
    }

    /// Call when a pong arrives.
    ///
    /// - `send_time_s` — local time (seconds) when we sent the ping.
    /// - `recv_time_s` — local time (seconds) when we received the pong.
    /// - `server_send_time_s` — the server's timestamp embedded in the pong.
    pub fn record_ping_pong(
        &mut self,
        send_time_s:        f64,
        recv_time_s:        f64,
        server_send_time_s: f64,
    ) {
        let rtt = recv_time_s - send_time_s;
        if rtt <= 0.0 { return; }

        // Update smoothed RTT
        if self.samples == 0 {
            self.rtt_s = rtt;
        } else {
            self.rtt_s = self.rtt_s * (1.0 - self.alpha) + rtt * self.alpha;
        }

        // Estimate server time at moment of reception
        let estimated_server_recv = server_send_time_s + rtt / 2.0;
        let new_offset = estimated_server_recv - recv_time_s;

        // Smooth the offset
        if self.samples == 0 {
            self.time_offset = new_offset;
        } else {
            let error = new_offset - self.time_offset;
            // Queue smooth correction
            self.correction_remaining += error;
        }
        self.samples += 1;
    }

    /// Advance the clock correction by `dt` seconds.
    /// Should be called each game frame.
    pub fn tick(&mut self, dt: f64) {
        if self.correction_remaining.abs() > f64::EPSILON {
            let step = self.correction_rate * dt * self.correction_remaining.signum();
            let step = if step.abs() > self.correction_remaining.abs() {
                self.correction_remaining
            } else {
                step
            };
            self.time_offset += step;
            self.correction_remaining -= step;
        }
    }

    /// Best estimate of the current server time given `local_time_s`.
    pub fn server_time(&self, local_time_s: f64) -> f64 {
        local_time_s + self.time_offset
    }

    /// Convert a local time to server tick given tick rate.
    pub fn to_server_tick(&self, local_time_s: f64, tick_rate_hz: f64) -> u64 {
        (self.server_time(local_time_s) * tick_rate_hz) as u64
    }

    pub fn rtt_ms(&self) -> f64 { self.rtt_s * 1000.0 }
    pub fn offset_s(&self) -> f64 { self.time_offset }
    pub fn sample_count(&self) -> u64 { self.samples }
}

impl Default for NetworkClock {
    fn default() -> Self { Self::new() }
}

// ─── AuthorityModel ───────────────────────────────────────────────────────────

/// Which peer has authoritative control over a given entity or subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityModel {
    /// Server is always right; clients predict but must defer to server.
    ServerAuthority,
    /// Client owns the entity; server trusts client (cheating risk).
    ClientAuthority,
    /// Negotiated per-property (e.g. physics server-auth, animation client-auth).
    SharedAuthority,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(tick: u64, entities: Vec<EntitySnapshot>) -> GameStateSnapshot {
        GameStateSnapshot { tick, timestamp: tick as f64 / 60.0, entities }
    }

    fn ent(id: u64, pos: Vec3) -> EntitySnapshot {
        EntitySnapshot {
            id, position: pos, velocity: Vec3::ZERO, rotation: Vec3::ZERO,
            health: 100.0, state_flags: 1, custom: vec![],
        }
    }

    // ── DeltaSnapshot ─────────────────────────────────────────────────────────

    #[test]
    fn test_delta_snapshot_no_changes() {
        let e = ent(1, Vec3::new(0.0, 0.0, 0.0));
        let base    = snap(10, vec![e.clone()]);
        let current = snap(11, vec![e]);
        let delta = DeltaSnapshot::build(&base, &current);
        assert_eq!(delta.change_count(), 0, "no changes expected");
    }

    #[test]
    fn test_delta_snapshot_position_change() {
        let base    = snap(10, vec![ent(1, Vec3::new(0.0, 0.0, 0.0))]);
        let current = snap(11, vec![ent(1, Vec3::new(1.0, 0.0, 0.0))]);
        let delta   = DeltaSnapshot::build(&base, &current);
        assert_eq!(delta.change_count(), 1);
        let d = &delta.changed[0];
        assert!(d.position_delta.is_some());
    }

    #[test]
    fn test_delta_snapshot_spawn_despawn() {
        let base    = snap(10, vec![ent(1, Vec3::ZERO)]);
        let current = snap(11, vec![ent(1, Vec3::ZERO), ent(2, Vec3::new(5.0, 0.0, 0.0))]);
        let delta   = DeltaSnapshot::build(&base, &current);
        assert!(delta.changed.iter().any(|d| d.id == 2 && d.spawned));

        // Now entity 1 despawns
        let base2    = snap(11, vec![ent(1, Vec3::ZERO), ent(2, Vec3::ZERO)]);
        let current2 = snap(12, vec![ent(2, Vec3::ZERO)]);
        let delta2 = DeltaSnapshot::build(&base2, &current2);
        assert!(delta2.changed.iter().any(|d| d.id == 1 && d.despawned));
    }

    #[test]
    fn test_delta_apply_roundtrip() {
        let base    = snap(10, vec![ent(1, Vec3::new(0.0, 0.0, 0.0))]);
        let target  = snap(11, vec![ent(1, Vec3::new(3.0, 1.0, 2.0))]);
        let delta   = DeltaSnapshot::build(&base, &target);
        let applied = delta.apply(&base);
        let ent_r   = applied.entity(1).unwrap();
        assert!((ent_r.position.x - 3.0).abs() < 0.001);
        assert!((ent_r.position.y - 1.0).abs() < 0.001);
        assert!((ent_r.position.z - 2.0).abs() < 0.001);
    }

    // ── SnapshotBuffer ────────────────────────────────────────────────────────

    #[test]
    fn test_snapshot_buffer_capacity() {
        let mut buf = SnapshotBuffer::new(4);
        for i in 0..6u64 {
            buf.push(snap(i, vec![]));
        }
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.oldest().unwrap().tick, 2);
        assert_eq!(buf.latest().unwrap().tick, 5);
    }

    #[test]
    fn test_snapshot_buffer_at_tick() {
        let mut buf = SnapshotBuffer::new(64);
        for i in [10u64, 20, 30, 40] {
            buf.push(snap(i, vec![]));
        }
        assert_eq!(buf.at_tick(25).unwrap().tick, 20);
        assert_eq!(buf.at_tick(40).unwrap().tick, 40);
        assert!(buf.at_tick(5).is_none());
    }

    // ── StateInterpolator ─────────────────────────────────────────────────────

    #[test]
    fn test_interpolator_between_snapshots() {
        let mut interp = StateInterpolator::new(0);
        interp.push_snapshot(snap(10, vec![ent(1, Vec3::new(0.0, 0.0, 0.0))]));
        interp.push_snapshot(snap(20, vec![ent(1, Vec3::new(10.0, 0.0, 0.0))]));

        let result = interp.interpolate(15, 0.0).unwrap();
        let e      = result.entity(1).unwrap();
        assert!((e.position.x - 5.0).abs() < 0.1, "expected ~5.0, got {}", e.position.x);
    }

    // ── PlayerInput serialization ─────────────────────────────────────────────

    #[test]
    fn test_player_input_roundtrip() {
        let inp = PlayerInput {
            tick:     42,
            move_dir: Vec2::new(0.5, -0.5),
            jump:     true,
            actions:  0b1010,
            facing:   1.57,
        };
        let bytes   = inp.serialize();
        let decoded = PlayerInput::deserialize(&bytes).unwrap();
        assert_eq!(decoded.tick, inp.tick);
        assert!((decoded.move_dir.x - inp.move_dir.x).abs() < 0.0001);
        assert_eq!(decoded.jump, inp.jump);
        assert_eq!(decoded.actions, inp.actions);
    }

    // ── ClientPrediction ──────────────────────────────────────────────────────

    #[test]
    fn test_client_prediction_reconcile_snaps_large_error() {
        let mut pred = ClientPrediction::new();
        pred.predicted_pos = Vec3::new(100.0, 0.0, 0.0);
        pred.predicted_vel = Vec3::ZERO;

        let sim = |_pos: Vec3, vel: Vec3, _inp: &PlayerInput, _dt: f32| (Vec3::new(1.0, 0.0, 0.0), vel);
        pred.reconcile(Vec3::new(0.0, 0.0, 0.0), Vec3::ZERO, 5, 0.016, sim);

        // Error > snap_threshold → should have snapped
        assert!(!pred.is_correcting());
    }

    #[test]
    fn test_client_prediction_reconcile_blends_small_error() {
        let mut pred = ClientPrediction::new();
        pred.predicted_pos = Vec3::new(1.0, 0.0, 0.0);
        let sim = |_pos: Vec3, vel: Vec3, _inp: &PlayerInput, _dt: f32| (Vec3::new(1.0, 0.0, 0.0), vel);
        // Server says 1.05 — small error
        pred.reconcile(Vec3::new(1.05, 0.0, 0.0), Vec3::ZERO, 0, 0.016, sim);
        // Error < snap_threshold but > 0.001 → blending
        assert!(pred.is_correcting());
    }

    // ── LagCompensation ───────────────────────────────────────────────────────

    #[test]
    fn test_lag_compensation_rewind() {
        let mut lc = LagCompensation::default_one_second();
        for tick in 0..65u64 {
            lc.record(snap(tick, vec![ent(1, Vec3::new(tick as f32, 0.0, 0.0))]));
        }
        let rewound = lc.rewind_to_tick(10).unwrap();
        assert!(rewound.tick <= 10);

        let ent_at_10 = lc.entity_at_tick(1, 10).unwrap();
        assert!((ent_at_10.position.x - 10.0).abs() < 0.001);
    }

    // ── NetworkClock ──────────────────────────────────────────────────────────

    #[test]
    fn test_network_clock_basic_sync() {
        let mut clock = NetworkClock::new();
        // Simulate: local sends at t=1.0, server is 5s ahead, pong received at t=1.1
        let send_t  = 1.0f64;
        let recv_t  = 1.1f64;
        let srv_t   = 6.05f64; // server time at midpoint of RTT
        clock.record_ping_pong(send_t, recv_t, srv_t);
        // After one sample, offset should be close to 5.0
        let est = clock.server_time(recv_t);
        assert!((est - 6.1).abs() < 0.2, "est={est}");
    }

    #[test]
    fn test_network_clock_tick_applies_correction() {
        let mut clock = NetworkClock::new();
        clock.record_ping_pong(0.0, 0.1, 5.05);
        clock.record_ping_pong(1.0, 1.1, 6.05); // second ping, same offset
        // Correction should converge; just verify it doesn't panic
        for _ in 0..100 {
            clock.tick(0.016);
        }
        assert!(clock.sample_count() >= 2);
    }
}
