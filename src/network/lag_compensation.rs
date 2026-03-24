//! Server-side lag compensation for Proof Engine multiplayer.
//!
//! Implements: entity history ring buffers, time-rewind, hit validation,
//! client movement prediction validation, and anti-cheat monitoring.

use std::collections::HashMap;
use glam::Vec3;

use crate::network::server::EntitySnapshot;

// ── Aabb ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Aabb { min, max }
    }

    /// Create an AABB centred at `centre` with half-extents `half`.
    pub fn from_centre(centre: [f32; 3], half: [f32; 3]) -> Self {
        Aabb {
            min: [centre[0] - half[0], centre[1] - half[1], centre[2] - half[2]],
            max: [centre[0] + half[0], centre[1] + half[1], centre[2] + half[2]],
        }
    }

    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.max[0] >= other.min[0] && self.min[0] <= other.max[0] &&
        self.max[1] >= other.min[1] && self.min[1] <= other.max[1] &&
        self.max[2] >= other.min[2] && self.min[2] <= other.max[2]
    }

    pub fn contains(&self, point: [f32; 3]) -> bool {
        point[0] >= self.min[0] && point[0] <= self.max[0] &&
        point[1] >= self.min[1] && point[1] <= self.max[1] &&
        point[2] >= self.min[2] && point[2] <= self.max[2]
    }

    pub fn centre(&self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    pub fn half_extents(&self) -> [f32; 3] {
        [
            (self.max[0] - self.min[0]) * 0.5,
            (self.max[1] - self.min[1]) * 0.5,
            (self.max[2] - self.min[2]) * 0.5,
        ]
    }

    /// Expand this AABB uniformly by `margin` on each side.
    pub fn expanded(&self, margin: f32) -> Aabb {
        Aabb {
            min: [self.min[0] - margin, self.min[1] - margin, self.min[2] - margin],
            max: [self.max[0] + margin, self.max[1] + margin, self.max[2] + margin],
        }
    }

    /// Translate the AABB to a new position.
    pub fn at_position(&self, pos: [f32; 3]) -> Aabb {
        let c = self.centre();
        let h = self.half_extents();
        Aabb::from_centre(
            [pos[0] + (c[0] - pos[0]), pos[1], pos[2]],
            h,
        )
    }

    /// Offset the AABB by a displacement.
    pub fn offset(&self, delta: [f32; 3]) -> Aabb {
        Aabb {
            min: [self.min[0] + delta[0], self.min[1] + delta[1], self.min[2] + delta[2]],
            max: [self.max[0] + delta[0], self.max[1] + delta[1], self.max[2] + delta[2]],
        }
    }

    pub fn volume(&self) -> f32 {
        let w = (self.max[0] - self.min[0]).max(0.0);
        let h = (self.max[1] - self.min[1]).max(0.0);
        let d = (self.max[2] - self.min[2]).max(0.0);
        w * h * d
    }
}

// ── HistoryRecord ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HistoryRecord {
    pub frame:     u64,
    pub timestamp: f64,
    pub entities:  Vec<EntitySnapshot>,
}

impl HistoryRecord {
    pub fn get_entity(&self, id: u64) -> Option<&EntitySnapshot> {
        self.entities.iter().find(|e| e.id == id)
    }
}

// ── EntityHistory ─────────────────────────────────────────────────────────────

const ENTITY_HISTORY_FRAMES: usize = 32;

/// Ring buffer of the last N snapshots for a single entity.
pub struct EntityHistory {
    frames:    [Option<EntitySnapshot>; ENTITY_HISTORY_FRAMES],
    times:     [f64; ENTITY_HISTORY_FRAMES],
    write_pos: usize,
    count:     usize,
}

impl EntityHistory {
    pub fn new() -> Self {
        EntityHistory {
            frames:    std::array::from_fn(|_| None),
            times:     [0.0; ENTITY_HISTORY_FRAMES],
            write_pos: 0,
            count:     0,
        }
    }

    pub fn push(&mut self, snap: EntitySnapshot, time: f64) {
        self.frames[self.write_pos] = Some(snap);
        self.times[self.write_pos]  = time;
        self.write_pos = (self.write_pos + 1) % ENTITY_HISTORY_FRAMES;
        if self.count < ENTITY_HISTORY_FRAMES { self.count += 1; }
    }

    /// Interpolate position at `time`. Returns `None` if outside buffered range.
    pub fn interpolate_at(&self, time: f64) -> Option<EntitySnapshot> {
        if self.count < 2 { return self.latest().cloned(); }

        // Find the two frames bracketing `time`
        let mut before_idx: Option<usize> = None;
        let mut after_idx:  Option<usize>  = None;

        for i in 0..self.count {
            let idx = (self.write_pos + ENTITY_HISTORY_FRAMES - 1 - i) % ENTITY_HISTORY_FRAMES;
            let t   = self.times[idx];
            if t <= time {
                before_idx = Some(idx);
                break;
            }
            after_idx = Some(idx);
        }

        match (before_idx, after_idx) {
            (Some(bi), Some(ai)) => {
                let t0 = self.times[bi];
                let t1 = self.times[ai];
                let s0 = self.frames[bi].as_ref()?;
                let s1 = self.frames[ai].as_ref()?;
                let span = t1 - t0;
                if span < 1e-9 { return Some(s0.clone()); }
                let alpha = ((time - t0) / span) as f32;
                Some(s0.lerp(s1, alpha))
            }
            (Some(bi), None) => self.frames[bi].clone(),
            (None,  Some(ai)) => self.frames[ai].clone(),
            (None, None) => None,
        }
    }

    pub fn latest(&self) -> Option<&EntitySnapshot> {
        if self.count == 0 { return None; }
        let idx = if self.write_pos == 0 {
            ENTITY_HISTORY_FRAMES - 1
        } else {
            self.write_pos - 1
        };
        self.frames[idx].as_ref()
    }

    pub fn oldest_time(&self) -> Option<f64> {
        if self.count == 0 { return None; }
        // The oldest entry is at write_pos (for a full buffer) or index 0
        let oldest_idx = if self.count < ENTITY_HISTORY_FRAMES {
            0
        } else {
            self.write_pos
        };
        Some(self.times[oldest_idx])
    }

    pub fn newest_time(&self) -> Option<f64> {
        if self.count == 0 { return None; }
        let idx = if self.write_pos == 0 { ENTITY_HISTORY_FRAMES - 1 } else { self.write_pos - 1 };
        Some(self.times[idx])
    }

    pub fn len(&self) -> usize { self.count }
    pub fn is_empty(&self) -> bool { self.count == 0 }
}

impl Default for EntityHistory {
    fn default() -> Self { Self::new() }
}

// ── LagCompensator ────────────────────────────────────────────────────────────

/// Server-side lag compensation: rewinds entity positions to a past time for
/// hit detection, then advances back to present.
pub struct LagCompensator {
    /// entity_id → position history
    pub history:       HashMap<u64, EntityHistory>,
    /// Maximum time we'll rewind in milliseconds
    pub max_rewind_ms: f64,
}

impl LagCompensator {
    pub const DEFAULT_MAX_REWIND_MS: f64 = 200.0;

    pub fn new() -> Self {
        LagCompensator {
            history:       HashMap::new(),
            max_rewind_ms: Self::DEFAULT_MAX_REWIND_MS,
        }
    }

    pub fn with_max_rewind(max_ms: f64) -> Self {
        LagCompensator {
            history:       HashMap::new(),
            max_rewind_ms: max_ms,
        }
    }

    /// Record current-frame snapshots.
    pub fn record_frame(&mut self, frame: u64, time: f64, snapshots: &[EntitySnapshot]) {
        let _ = frame;
        for snap in snapshots {
            self.history
                .entry(snap.id)
                .or_insert_with(EntityHistory::new)
                .push(snap.clone(), time);
        }
    }

    /// Get interpolated positions for all known entities at `time`.
    pub fn rewind_to(&self, time: f64) -> Vec<EntitySnapshot> {
        self.history
            .values()
            .filter_map(|h| h.interpolate_at(time))
            .collect()
    }

    /// Validate a hit: given the attacker's client-side time and aim direction,
    /// check if `hit_box` (defined in world space at the attacker's perceived time)
    /// actually intersected with the rewound target position.
    pub fn validate_hit(
        &self,
        attacker_time: f64,
        attacker_pos:  Vec3,
        target_id:     u64,
        hit_box:       &Aabb,
    ) -> bool {
        let _ = attacker_pos;
        let history = match self.history.get(&target_id) {
            Some(h) => h,
            None    => return false,
        };

        // Ensure we have history old enough
        let oldest = history.oldest_time().unwrap_or(f64::MAX);
        let newest = history.newest_time().unwrap_or(0.0);

        if attacker_time < oldest || attacker_time > newest + (self.max_rewind_ms / 1000.0) {
            return false;
        }

        // Clamp rewind target time
        let rewind_time = attacker_time.max(oldest);
        let rewound = match history.interpolate_at(rewind_time) {
            Some(s) => s,
            None    => return false,
        };

        // Build entity AABB at rewound position (assume unit hitbox if no size data)
        let entity_aabb = Aabb::from_centre(rewound.pos, [0.4, 0.9, 0.4]);
        entity_aabb.overlaps(hit_box)
    }

    pub fn remove_entity(&mut self, entity_id: u64) {
        self.history.remove(&entity_id);
    }

    pub fn entity_count(&self) -> usize { self.history.len() }
}

impl Default for LagCompensator {
    fn default() -> Self { Self::new() }
}

// ── PredictionValidator ───────────────────────────────────────────────────────

/// Validates client-submitted movement predictions on the server.
///
/// Catches impossible movement, teleportation, and applies rubber-banding
/// to smoothly correct diverged client positions.
pub struct PredictionValidator {
    pub max_speed:         f32, // world units per second
    pub max_teleport_dist: f32, // world units per tick — larger means disconnect
    pub correction_blend:  f32, // 0..1 — how quickly to snap back (1 = instant)
}

impl PredictionValidator {
    pub fn new(max_speed: f32) -> Self {
        PredictionValidator {
            max_speed,
            max_teleport_dist: max_speed * 0.5,
            correction_blend:  0.2,
        }
    }

    /// Returns `true` if moving from `old_pos` to `new_pos` in `dt` seconds is valid.
    pub fn check_speed(&self, old_pos: [f32; 3], new_pos: [f32; 3], dt: f32) -> bool {
        if dt <= 0.0 { return false; }
        let dx = new_pos[0] - old_pos[0];
        let dy = new_pos[1] - old_pos[1];
        let dz = new_pos[2] - old_pos[2];
        let dist_sq = dx * dx + dy * dy + dz * dz;
        let max_dist = self.max_speed * dt;
        dist_sq <= max_dist * max_dist * 1.44 // 20% tolerance (1.2^2 = 1.44)
    }

    /// Returns `true` if movement is within allowed single-tick distance.
    pub fn check_teleport(&self, old_pos: [f32; 3], new_pos: [f32; 3]) -> bool {
        let dx = new_pos[0] - old_pos[0];
        let dy = new_pos[1] - old_pos[1];
        let dz = new_pos[2] - old_pos[2];
        let dist_sq = dx * dx + dy * dy + dz * dz;
        dist_sq <= self.max_teleport_dist * self.max_teleport_dist
    }

    /// Rubber-band correction: blend `client_pos` toward `server_pos`.
    /// Returns the corrected position.
    pub fn correct_position(&self, client_pos: [f32; 3], server_pos: [f32; 3]) -> [f32; 3] {
        let t = self.correction_blend;
        [
            client_pos[0] + t * (server_pos[0] - client_pos[0]),
            client_pos[1] + t * (server_pos[1] - client_pos[1]),
            client_pos[2] + t * (server_pos[2] - client_pos[2]),
        ]
    }

    /// Returns how far `client_pos` diverges from `server_pos`.
    pub fn divergence(&self, client_pos: [f32; 3], server_pos: [f32; 3]) -> f32 {
        let dx = client_pos[0] - server_pos[0];
        let dy = client_pos[1] - server_pos[1];
        let dz = client_pos[2] - server_pos[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Returns `true` if correction should be applied (divergence > threshold).
    pub fn needs_correction(&self, client_pos: [f32; 3], server_pos: [f32; 3], threshold: f32) -> bool {
        self.divergence(client_pos, server_pos) > threshold
    }
}

impl Default for PredictionValidator {
    fn default() -> Self { Self::new(10.0) }
}

// ── CheatViolationType ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheatViolationType {
    SpeedHack,
    Noclip,
    AimbotSuspect,
    ExcessiveHeadshots,
    InvalidPosition,
    PacketManipulation,
}

impl CheatViolationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheatViolationType::SpeedHack             => "SpeedHack",
            CheatViolationType::Noclip                => "Noclip",
            CheatViolationType::AimbotSuspect         => "AimbotSuspect",
            CheatViolationType::ExcessiveHeadshots    => "ExcessiveHeadshots",
            CheatViolationType::InvalidPosition       => "InvalidPosition",
            CheatViolationType::PacketManipulation    => "PacketManipulation",
        }
    }

    pub fn base_confidence(&self) -> f32 {
        match self {
            CheatViolationType::SpeedHack          => 0.7,
            CheatViolationType::Noclip             => 0.9,
            CheatViolationType::AimbotSuspect      => 0.4,
            CheatViolationType::ExcessiveHeadshots => 0.3,
            CheatViolationType::InvalidPosition    => 0.8,
            CheatViolationType::PacketManipulation => 0.95,
        }
    }
}

// ── CheatFlag ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CheatFlag {
    pub player_id:      String,
    pub violation_type: CheatViolationType,
    pub confidence:     f32,
    pub timestamp:      f64,
    pub details:        String,
}

impl CheatFlag {
    pub fn new(
        player_id:      String,
        violation_type: CheatViolationType,
        timestamp:      f64,
        details:        String,
    ) -> Self {
        let confidence = violation_type.base_confidence();
        CheatFlag { player_id, violation_type, confidence, timestamp, details }
    }
}

// ── PlayerCheatState ──────────────────────────────────────────────────────────

/// Per-player accumulated cheat detection state.
struct PlayerCheatState {
    /// Recent positions for velocity analysis
    positions:     Vec<([f32; 3], f64)>, // (pos, time)
    /// Recent aim angles for aimbot detection
    aim_angles:    Vec<([f32; 2], f64)>, // (yaw, pitch), time
    /// Consecutive speed violations
    speed_violations: u32,
    /// Total flags accumulated
    total_flags:   u32,
    /// Combined confidence score (0..1)
    confidence:    f32,
    /// Kill streak for headshot ratio tracking
    headshot_kills:   u32,
    total_kills:      u32,
}

impl PlayerCheatState {
    fn new() -> Self {
        PlayerCheatState {
            positions:       Vec::new(),
            aim_angles:      Vec::new(),
            speed_violations: 0,
            total_flags:     0,
            confidence:      0.0,
            headshot_kills:  0,
            total_kills:     0,
        }
    }

    fn record_position(&mut self, pos: [f32; 3], time: f64) {
        self.positions.push((pos, time));
        if self.positions.len() > 32 { self.positions.remove(0); }
    }

    fn record_aim(&mut self, yaw: f32, pitch: f32, time: f64) {
        self.aim_angles.push(([yaw, pitch], time));
        if self.aim_angles.len() > 64 { self.aim_angles.remove(0); }
    }

    fn measured_speed(&self) -> f32 {
        if self.positions.len() < 2 { return 0.0; }
        let n = self.positions.len();
        let (p1, t1) = &self.positions[n - 2];
        let (p2, t2) = &self.positions[n - 1];
        let dt = (t2 - t1) as f32;
        if dt < 1e-6 { return 0.0; }
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let dz = p2[2] - p1[2];
        (dx*dx + dy*dy + dz*dz).sqrt() / dt
    }

    /// Returns the maximum angular velocity (degrees/sec) over recent frames.
    fn max_turn_rate(&self) -> f32 {
        if self.aim_angles.len() < 2 { return 0.0; }
        let mut max_rate = 0.0f32;
        for i in 1..self.aim_angles.len() {
            let ([y0, p0], t0) = &self.aim_angles[i - 1];
            let ([y1, p1], t1) = &self.aim_angles[i];
            let dt = (t1 - t0) as f32;
            if dt < 1e-6 { continue; }
            let dy = (y1 - y0).abs();
            let dp = (p1 - p0).abs();
            // Wrap yaw around 360
            let dy = if dy > 180.0 { 360.0 - dy } else { dy };
            let rate = (dy * dy + dp * dp).sqrt() / dt;
            if rate > max_rate { max_rate = rate; }
        }
        max_rate
    }

    fn headshot_ratio(&self) -> f32 {
        if self.total_kills == 0 { return 0.0; }
        self.headshot_kills as f32 / self.total_kills as f32
    }
}

// ── AntiCheatMonitor ──────────────────────────────────────────────────────────

/// Monitors all players for cheat patterns. Non-blocking, accumulates evidence.
pub struct AntiCheatMonitor {
    player_states: HashMap<String, PlayerCheatState>,
    flags:         Vec<CheatFlag>,
    /// Speed above which a violation is recorded (max_speed × 1.2)
    speed_threshold: f32,
    /// Max angular velocity in deg/sec before aimbot suspicion
    max_turn_rate_deg: f32,
    /// Headshot ratio above which to flag
    headshot_threshold: f32,
    current_time:    f64,
}

impl AntiCheatMonitor {
    pub fn new(player_max_speed: f32) -> Self {
        AntiCheatMonitor {
            player_states:      HashMap::new(),
            flags:              Vec::new(),
            speed_threshold:    player_max_speed * 1.2,
            max_turn_rate_deg:  3600.0, // 10 full rotations/sec
            headshot_threshold: 0.85,
            current_time:       0.0,
        }
    }

    pub fn set_time(&mut self, t: f64) { self.current_time = t; }

    // ── Per-tick updates ─────────────────────────────────────────────────────

    /// Record a player's latest position. Call each tick.
    pub fn record_position(&mut self, player_id: &str, pos: [f32; 3]) {
        let state = self.player_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerCheatState::new);
        state.record_position(pos, self.current_time);
    }

    /// Record player aim angles (yaw in degrees, pitch in degrees).
    pub fn record_aim(&mut self, player_id: &str, yaw: f32, pitch: f32) {
        let state = self.player_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerCheatState::new);
        state.record_aim(yaw, pitch, self.current_time);
    }

    /// Notify the monitor of a kill (headshot: true/false).
    pub fn record_kill(&mut self, player_id: &str, headshot: bool) {
        let state = self.player_states
            .entry(player_id.to_string())
            .or_insert_with(PlayerCheatState::new);
        state.total_kills += 1;
        if headshot { state.headshot_kills += 1; }
    }

    // ── Detection ────────────────────────────────────────────────────────────

    /// Check a player for speed hacking. Returns `true` if a flag was raised.
    pub fn check_speed_hack(&mut self, player_id: &str) -> bool {
        let state = match self.player_states.get_mut(player_id) {
            Some(s) => s,
            None    => return false,
        };
        let speed = state.measured_speed();
        if speed > self.speed_threshold {
            state.speed_violations += 1;
            let confidence = (speed / self.speed_threshold - 1.0).min(1.0);
            let flag = CheatFlag {
                player_id:      player_id.to_string(),
                violation_type: CheatViolationType::SpeedHack,
                confidence,
                timestamp:      self.current_time,
                details:        format!("speed={:.2} threshold={:.2}", speed, self.speed_threshold),
            };
            self.flags.push(flag);
            true
        } else {
            if state.speed_violations > 0 { state.speed_violations -= 1; }
            false
        }
    }

    /// Check for impossible position (outside world bounds or inside geometry hint).
    pub fn check_invalid_position(&mut self, player_id: &str, pos: [f32; 3], world_bounds: &Aabb) -> bool {
        if !world_bounds.contains(pos) {
            let flag = CheatFlag::new(
                player_id.to_string(),
                CheatViolationType::InvalidPosition,
                self.current_time,
                format!("pos=[{:.2},{:.2},{:.2}] outside bounds", pos[0], pos[1], pos[2]),
            );
            self.flags.push(flag);
            return true;
        }
        false
    }

    /// Check for noclip by testing if the straight-line path between last two positions
    /// passes through a set of solid AABBs.
    pub fn check_noclip(
        &mut self,
        player_id:  &str,
        solid_boxes: &[Aabb],
    ) -> bool {
        let positions = match self.player_states.get(player_id) {
            Some(s) => s.positions.clone(),
            None    => return false,
        };
        if positions.len() < 2 { return false; }
        let n = positions.len();
        let (prev, _) = &positions[n - 2];
        let (curr, _) = &positions[n - 1];

        // Sample the path in 5 steps
        let mut flagged = false;
        for step in 0..=5 {
            let t = step as f32 / 5.0;
            let sample = [
                prev[0] + t * (curr[0] - prev[0]),
                prev[1] + t * (curr[1] - prev[1]),
                prev[2] + t * (curr[2] - prev[2]),
            ];
            for solid in solid_boxes {
                if solid.contains(sample) {
                    let flag = CheatFlag::new(
                        player_id.to_string(),
                        CheatViolationType::Noclip,
                        self.current_time,
                        format!("path passes through solid geometry at [{:.2},{:.2},{:.2}]",
                            sample[0], sample[1], sample[2]),
                    );
                    self.flags.push(flag);
                    flagged = true;
                    break;
                }
            }
            if flagged { break; }
        }
        flagged
    }

    /// Check for aimbot: impossibly high turn rates or perfect headshot ratios.
    pub fn check_aimbot(&mut self, player_id: &str) -> bool {
        let (turn_rate, hs_ratio) = {
            let state = match self.player_states.get(player_id) {
                Some(s) => s,
                None    => return false,
            };
            (state.max_turn_rate(), state.headshot_ratio())
        };

        let mut flagged = false;

        if turn_rate > self.max_turn_rate_deg {
            let confidence = (turn_rate / self.max_turn_rate_deg - 1.0).min(1.0) * 0.6;
            self.flags.push(CheatFlag {
                player_id:      player_id.to_string(),
                violation_type: CheatViolationType::AimbotSuspect,
                confidence,
                timestamp:      self.current_time,
                details:        format!("turn_rate={:.1} deg/s", turn_rate),
            });
            flagged = true;
        }

        if let Some(state) = self.player_states.get(player_id) {
            if state.total_kills >= 10 && hs_ratio > self.headshot_threshold {
                self.flags.push(CheatFlag {
                    player_id:      player_id.to_string(),
                    violation_type: CheatViolationType::ExcessiveHeadshots,
                    confidence:     (hs_ratio - self.headshot_threshold) * 2.0,
                    timestamp:      self.current_time,
                    details:        format!("headshot_ratio={:.2}", hs_ratio),
                });
                flagged = true;
            }
        }

        flagged
    }

    // ── Flag management ───────────────────────────────────────────────────────

    pub fn flags(&self) -> &[CheatFlag] { &self.flags }

    pub fn flags_for(&self, player_id: &str) -> Vec<&CheatFlag> {
        self.flags.iter().filter(|f| f.player_id == player_id).collect()
    }

    pub fn total_confidence(&self, player_id: &str) -> f32 {
        let flags = self.flags_for(player_id);
        if flags.is_empty() { return 0.0; }
        // Accumulate using: combined = 1 - product(1 - c_i)
        let combined = flags.iter().fold(0.0f32, |acc, f| {
            acc + f.confidence * (1.0 - acc)
        });
        combined
    }

    pub fn should_kick(&self, player_id: &str) -> bool {
        self.total_confidence(player_id) > 0.9
    }

    pub fn clear_flags(&mut self, player_id: &str) {
        self.flags.retain(|f| f.player_id != player_id);
    }

    pub fn remove_player(&mut self, player_id: &str) {
        self.player_states.remove(player_id);
        self.clear_flags(player_id);
    }

    /// Drain all current flags (e.g., for logging/action pipeline).
    pub fn drain_flags(&mut self) -> Vec<CheatFlag> {
        std::mem::take(&mut self.flags)
    }

    pub fn flag_count(&self) -> usize { self.flags.len() }
    pub fn player_count(&self) -> usize { self.player_states.len() }
}

impl Default for AntiCheatMonitor {
    fn default() -> Self { Self::new(10.0) }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: u64, x: f32, y: f32, z: f32) -> EntitySnapshot {
        EntitySnapshot {
            id,
            pos:   [x, y, z],
            rot:   [0.0, 0.0, 0.0, 1.0],
            vel:   [0.0; 3],
            flags: 0,
        }
    }

    #[test]
    fn aabb_overlap() {
        let a = Aabb::new([0.0; 3], [1.0; 3]);
        let b = Aabb::new([0.5, 0.5, 0.5], [1.5, 1.5, 1.5]);
        assert!(a.overlaps(&b));
    }

    #[test]
    fn aabb_no_overlap() {
        let a = Aabb::new([0.0; 3], [1.0; 3]);
        let b = Aabb::new([2.0; 3], [3.0; 3]);
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn aabb_contains() {
        let a = Aabb::new([0.0; 3], [2.0; 3]);
        assert!(a.contains([1.0, 1.0, 1.0]));
        assert!(!a.contains([3.0, 1.0, 1.0]));
    }

    #[test]
    fn entity_history_interpolation() {
        let mut h = EntityHistory::new();
        h.push(snap(1, 0.0, 0.0, 0.0), 0.0);
        h.push(snap(1, 10.0, 0.0, 0.0), 1.0);
        let mid = h.interpolate_at(0.5).unwrap();
        assert!((mid.pos[0] - 5.0).abs() < 0.01, "Expected ~5.0, got {}", mid.pos[0]);
    }

    #[test]
    fn lag_compensator_record_and_rewind() {
        let mut lc = LagCompensator::new();
        lc.record_frame(0, 0.0, &[snap(1, 0.0, 0.0, 0.0)]);
        lc.record_frame(1, 0.1, &[snap(1, 1.0, 0.0, 0.0)]);
        lc.record_frame(2, 0.2, &[snap(1, 2.0, 0.0, 0.0)]);

        let rewound = lc.rewind_to(0.1);
        assert_eq!(rewound.len(), 1);
        assert!((rewound[0].pos[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn prediction_validator_speed_check() {
        let v = PredictionValidator::new(10.0);
        // Move 1 unit in 0.1s = 10 u/s — right at limit
        assert!(v.check_speed([0.0; 3], [1.0, 0.0, 0.0], 0.1));
        // Move 5 units in 0.1s = 50 u/s — too fast
        assert!(!v.check_speed([0.0; 3], [5.0, 0.0, 0.0], 0.1));
    }

    #[test]
    fn prediction_validator_correction() {
        let v = PredictionValidator { correction_blend: 0.5, ..PredictionValidator::new(10.0) };
        let corrected = v.correct_position([2.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert!((corrected[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn anti_cheat_speed_hack_detection() {
        let mut monitor = AntiCheatMonitor::new(10.0);
        monitor.set_time(0.0);
        monitor.record_position("p1", [0.0, 0.0, 0.0]);
        monitor.set_time(0.1);
        // Move 50 units in 0.1s = 500 u/s — well above threshold
        monitor.record_position("p1", [50.0, 0.0, 0.0]);
        let flagged = monitor.check_speed_hack("p1");
        assert!(flagged);
        assert!(!monitor.flags.is_empty());
    }

    #[test]
    fn anti_cheat_aimbot_turn_rate() {
        let mut monitor = AntiCheatMonitor::new(10.0);
        monitor.set_time(0.0);
        monitor.record_aim("p1", 0.0, 0.0);
        monitor.set_time(0.01);
        // 720° in 0.01s = 72000 deg/s — absurd
        monitor.record_aim("p1", 720.0, 0.0);
        let flagged = monitor.check_aimbot("p1");
        assert!(flagged);
    }

    #[test]
    fn validate_hit_within_box() {
        let mut lc = LagCompensator::new();
        lc.record_frame(0, 0.0, &[snap(42, 5.0, 0.0, 0.0)]);
        lc.record_frame(1, 0.05, &[snap(42, 5.0, 0.0, 0.0)]);

        let hit_box = Aabb::from_centre([5.0, 0.0, 0.0], [0.5, 1.0, 0.5]);
        let attacker_pos = Vec3::new(0.0, 0.0, 0.0);
        assert!(lc.validate_hit(0.025, attacker_pos, 42, &hit_box));
    }

    #[test]
    fn validate_hit_miss() {
        let mut lc = LagCompensator::new();
        lc.record_frame(0, 0.0, &[snap(42, 5.0, 0.0, 0.0)]);
        lc.record_frame(1, 0.05, &[snap(42, 5.0, 0.0, 0.0)]);

        // Hit box far away from entity
        let hit_box = Aabb::from_centre([100.0, 0.0, 0.0], [0.5, 1.0, 0.5]);
        let attacker_pos = Vec3::new(0.0, 0.0, 0.0);
        assert!(!lc.validate_hit(0.025, attacker_pos, 42, &hit_box));
    }
}
