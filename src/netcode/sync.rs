//! State synchronization: interpolation, prediction, authority, replication, and clock sync.

use std::collections::{HashMap, VecDeque};

/// A timestamped sample for interpolation.
#[derive(Debug, Clone)]
pub struct InterpolationSample {
    pub timestamp_ms: u64,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub extra: Vec<(u16, Vec<u8>)>,
}

impl InterpolationSample {
    pub fn new(timestamp_ms: u64, position: [f32; 3], rotation: [f32; 4]) -> Self {
        Self {
            timestamp_ms,
            position,
            rotation,
            velocity: [0.0; 3],
            extra: Vec::new(),
        }
    }

    pub fn with_velocity(mut self, velocity: [f32; 3]) -> Self {
        self.velocity = velocity;
        self
    }

    pub fn with_extra(mut self, component_type: u16, data: Vec<u8>) -> Self {
        self.extra.push((component_type, data));
        self
    }

    /// Linearly interpolate between two samples.
    pub fn lerp(&self, other: &InterpolationSample, t: f32) -> InterpolationSample {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;

        let position = [
            self.position[0] * inv + other.position[0] * t,
            self.position[1] * inv + other.position[1] * t,
            self.position[2] * inv + other.position[2] * t,
        ];

        let velocity = [
            self.velocity[0] * inv + other.velocity[0] * t,
            self.velocity[1] * inv + other.velocity[1] * t,
            self.velocity[2] * inv + other.velocity[2] * t,
        ];

        // Slerp for quaternion
        let rotation = quat_slerp(self.rotation, other.rotation, t);

        InterpolationSample {
            timestamp_ms: self.timestamp_ms + ((other.timestamp_ms as f64 - self.timestamp_ms as f64) * t as f64) as u64,
            position,
            rotation,
            velocity,
            extra: if t < 0.5 { self.extra.clone() } else { other.extra.clone() },
        }
    }

    /// Extrapolate forward from this sample using velocity.
    pub fn extrapolate(&self, dt_ms: u64) -> InterpolationSample {
        let dt_sec = dt_ms as f32 / 1000.0;
        InterpolationSample {
            timestamp_ms: self.timestamp_ms + dt_ms,
            position: [
                self.position[0] + self.velocity[0] * dt_sec,
                self.position[1] + self.velocity[1] * dt_sec,
                self.position[2] + self.velocity[2] * dt_sec,
            ],
            rotation: self.rotation,
            velocity: self.velocity,
            extra: self.extra.clone(),
        }
    }
}

/// Quaternion spherical linear interpolation.
fn quat_slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let mut b = b;

    // Ensure shortest path
    if dot < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        dot = -dot;
    }

    // If very close, use linear interpolation
    if dot > 0.9999 {
        let result = [
            a[0] + t * (b[0] - a[0]),
            a[1] + t * (b[1] - a[1]),
            a[2] + t * (b[2] - a[2]),
            a[3] + t * (b[3] - a[3]),
        ];
        return quat_normalize(result);
    }

    let theta = dot.clamp(-1.0, 1.0).acos();
    let sin_theta = theta.sin();

    if sin_theta.abs() < 1e-6 {
        return a;
    }

    let s0 = ((1.0 - t) * theta).sin() / sin_theta;
    let s1 = (t * theta).sin() / sin_theta;

    [
        a[0] * s0 + b[0] * s1,
        a[1] * s0 + b[1] * s1,
        a[2] * s0 + b[2] * s1,
        a[3] * s0 + b[3] * s1,
    ]
}

fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len < 1e-10 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
}

/// Entity identifier used in the sync layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyncEntityId(pub u32);

/// Interpolation buffer for a single remote entity.
/// Stores recent state samples and provides smooth interpolation.
pub struct InterpolationBuffer {
    entity_id: SyncEntityId,
    samples: VecDeque<InterpolationSample>,
    max_samples: usize,
    interp_delay_ms: u64,
    max_extrapolation_ms: u64,
    last_interpolated: Option<InterpolationSample>,
    hermite_enabled: bool,
}

impl InterpolationBuffer {
    pub fn new(entity_id: SyncEntityId, interp_delay_ms: u64) -> Self {
        Self {
            entity_id,
            samples: VecDeque::new(),
            max_samples: 32,
            interp_delay_ms,
            max_extrapolation_ms: 250,
            last_interpolated: None,
            hermite_enabled: false,
        }
    }

    pub fn entity_id(&self) -> SyncEntityId {
        self.entity_id
    }

    pub fn set_interp_delay(&mut self, delay_ms: u64) {
        self.interp_delay_ms = delay_ms;
    }

    pub fn set_max_extrapolation(&mut self, max_ms: u64) {
        self.max_extrapolation_ms = max_ms;
    }

    pub fn set_hermite(&mut self, enabled: bool) {
        self.hermite_enabled = enabled;
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Add a new state sample.
    pub fn push(&mut self, sample: InterpolationSample) {
        // Insert in timestamp order
        let ts = sample.timestamp_ms;
        let pos = self.samples.iter().position(|s| s.timestamp_ms > ts);
        match pos {
            Some(idx) => self.samples.insert(idx, sample),
            None => self.samples.push_back(sample),
        }

        // Trim old samples
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    /// Get the interpolated state at the given render time.
    pub fn sample_at(&mut self, render_time_ms: u64) -> Option<InterpolationSample> {
        if self.samples.is_empty() {
            return self.last_interpolated.clone();
        }

        let target_time = render_time_ms.saturating_sub(self.interp_delay_ms);

        // Find the two samples surrounding target_time
        let mut before_idx = None;
        let mut after_idx = None;

        for (i, sample) in self.samples.iter().enumerate() {
            if sample.timestamp_ms <= target_time {
                before_idx = Some(i);
            } else {
                after_idx = Some(i);
                break;
            }
        }

        let result = match (before_idx, after_idx) {
            (Some(bi), Some(ai)) => {
                let before = &self.samples[bi];
                let after = &self.samples[ai];
                let range = after.timestamp_ms.saturating_sub(before.timestamp_ms);
                if range == 0 {
                    before.clone()
                } else {
                    let t = (target_time.saturating_sub(before.timestamp_ms)) as f32 / range as f32;
                    if self.hermite_enabled {
                        self.hermite_interpolate(bi, ai, t)
                    } else {
                        before.lerp(after, t)
                    }
                }
            }
            (Some(bi), None) => {
                // Extrapolate from the latest sample
                let latest = &self.samples[bi];
                let dt = target_time.saturating_sub(latest.timestamp_ms);
                if dt <= self.max_extrapolation_ms {
                    latest.extrapolate(dt)
                } else {
                    latest.extrapolate(self.max_extrapolation_ms)
                }
            }
            (None, Some(ai)) => {
                // Before all samples, just use the earliest
                self.samples[ai].clone()
            }
            (None, None) => {
                return self.last_interpolated.clone();
            }
        };

        self.last_interpolated = Some(result.clone());
        Some(result)
    }

    /// Cubic Hermite interpolation using velocity as tangents.
    fn hermite_interpolate(&self, idx_a: usize, idx_b: usize, t: f32) -> InterpolationSample {
        let a = &self.samples[idx_a];
        let b = &self.samples[idx_b];
        let dt_sec = (b.timestamp_ms.saturating_sub(a.timestamp_ms)) as f32 / 1000.0;

        let t2 = t * t;
        let t3 = t2 * t;

        // Hermite basis functions
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;

        let mut position = [0.0f32; 3];
        for i in 0..3 {
            position[i] = h00 * a.position[i]
                + h10 * a.velocity[i] * dt_sec
                + h01 * b.position[i]
                + h11 * b.velocity[i] * dt_sec;
        }

        // Velocity: derivative of Hermite
        let dh00 = 6.0 * t2 - 6.0 * t;
        let dh10 = 3.0 * t2 - 4.0 * t + 1.0;
        let dh01 = -6.0 * t2 + 6.0 * t;
        let dh11 = 3.0 * t2 - 2.0 * t;

        let mut velocity = [0.0f32; 3];
        if dt_sec > 0.0 {
            for i in 0..3 {
                velocity[i] = (dh00 * a.position[i]
                    + dh10 * a.velocity[i] * dt_sec
                    + dh01 * b.position[i]
                    + dh11 * b.velocity[i] * dt_sec) / dt_sec;
            }
        }

        let rotation = quat_slerp(a.rotation, b.rotation, t);

        let timestamp_ms = a.timestamp_ms + (dt_sec * t * 1000.0) as u64;

        InterpolationSample {
            timestamp_ms,
            position,
            rotation,
            velocity,
            extra: if t < 0.5 { a.extra.clone() } else { b.extra.clone() },
        }
    }

    /// Clear all samples.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.last_interpolated = None;
    }

    /// Remove samples older than the given timestamp.
    pub fn prune_before(&mut self, timestamp_ms: u64) {
        // Keep at least 2 samples for interpolation
        while self.samples.len() > 2 {
            if let Some(front) = self.samples.front() {
                if front.timestamp_ms < timestamp_ms {
                    self.samples.pop_front();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Get the time span covered by buffered samples.
    pub fn buffered_time_ms(&self) -> u64 {
        if self.samples.len() < 2 {
            return 0;
        }
        let first = self.samples.front().unwrap().timestamp_ms;
        let last = self.samples.back().unwrap().timestamp_ms;
        last.saturating_sub(first)
    }
}

/// A prediction entry representing one input frame.
#[derive(Debug, Clone)]
pub struct PredictionEntry {
    pub sequence: u32,
    pub timestamp_ms: u64,
    pub input_data: Vec<u8>,
    pub predicted_position: [f32; 3],
    pub predicted_rotation: [f32; 4],
    pub predicted_velocity: [f32; 3],
    pub acknowledged: bool,
}

impl PredictionEntry {
    pub fn new(sequence: u32, timestamp_ms: u64, input_data: Vec<u8>) -> Self {
        Self {
            sequence,
            timestamp_ms,
            input_data,
            predicted_position: [0.0; 3],
            predicted_rotation: [0.0, 0.0, 0.0, 1.0],
            predicted_velocity: [0.0; 3],
            acknowledged: false,
        }
    }
}

/// Client-side prediction with server reconciliation.
pub struct ClientPrediction {
    entity_id: SyncEntityId,
    pending_inputs: VecDeque<PredictionEntry>,
    next_sequence: u32,
    max_pending: usize,
    last_server_position: [f32; 3],
    last_server_rotation: [f32; 4],
    last_server_velocity: [f32; 3],
    last_server_sequence: u32,
    correction_threshold: f32,
    correction_smoothing: f32,
    correction_offset: [f32; 3],
    correction_timer: f32,
    correction_duration: f32,
    misprediction_count: u64,
    total_predictions: u64,
}

impl ClientPrediction {
    pub fn new(entity_id: SyncEntityId) -> Self {
        Self {
            entity_id,
            pending_inputs: VecDeque::new(),
            next_sequence: 0,
            max_pending: 128,
            last_server_position: [0.0; 3],
            last_server_rotation: [0.0, 0.0, 0.0, 1.0],
            last_server_velocity: [0.0; 3],
            last_server_sequence: 0,
            correction_threshold: 0.1,
            correction_smoothing: 0.1,
            correction_offset: [0.0; 3],
            correction_timer: 0.0,
            correction_duration: 0.2,
            misprediction_count: 0,
            total_predictions: 0,
        }
    }

    pub fn entity_id(&self) -> SyncEntityId {
        self.entity_id
    }

    pub fn set_correction_threshold(&mut self, threshold: f32) {
        self.correction_threshold = threshold;
    }

    pub fn set_correction_duration(&mut self, duration: f32) {
        self.correction_duration = duration;
    }

    pub fn misprediction_rate(&self) -> f64 {
        if self.total_predictions == 0 {
            return 0.0;
        }
        self.misprediction_count as f64 / self.total_predictions as f64
    }

    pub fn pending_count(&self) -> usize {
        self.pending_inputs.len()
    }

    /// Record a new predicted input. Returns the sequence number.
    pub fn record_input(
        &mut self,
        timestamp_ms: u64,
        input_data: Vec<u8>,
        predicted_position: [f32; 3],
        predicted_rotation: [f32; 4],
        predicted_velocity: [f32; 3],
    ) -> u32 {
        let seq = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);

        let mut entry = PredictionEntry::new(seq, timestamp_ms, input_data);
        entry.predicted_position = predicted_position;
        entry.predicted_rotation = predicted_rotation;
        entry.predicted_velocity = predicted_velocity;

        self.pending_inputs.push_back(entry);
        self.total_predictions += 1;

        // Trim old entries
        while self.pending_inputs.len() > self.max_pending {
            self.pending_inputs.pop_front();
        }

        seq
    }

    /// Process a server authoritative state update.
    /// Returns the corrected position after reconciliation.
    pub fn reconcile(
        &mut self,
        server_sequence: u32,
        server_position: [f32; 3],
        server_rotation: [f32; 4],
        server_velocity: [f32; 3],
        apply_input: &dyn Fn(&[u8], [f32; 3], [f32; 4], [f32; 3]) -> ([f32; 3], [f32; 4], [f32; 3]),
    ) -> [f32; 3] {
        self.last_server_position = server_position;
        self.last_server_rotation = server_rotation;
        self.last_server_velocity = server_velocity;
        self.last_server_sequence = server_sequence;

        // Remove all acknowledged inputs
        while let Some(front) = self.pending_inputs.front() {
            if front.sequence <= server_sequence {
                self.pending_inputs.pop_front();
            } else {
                break;
            }
        }

        // Re-simulate remaining inputs from server state
        let mut pos = server_position;
        let mut rot = server_rotation;
        let mut vel = server_velocity;

        for entry in self.pending_inputs.iter_mut() {
            let (new_pos, new_rot, new_vel) = apply_input(&entry.input_data, pos, rot, vel);

            // Check for misprediction
            let dx = new_pos[0] - entry.predicted_position[0];
            let dy = new_pos[1] - entry.predicted_position[1];
            let dz = new_pos[2] - entry.predicted_position[2];
            let error = (dx * dx + dy * dy + dz * dz).sqrt();

            if error > self.correction_threshold {
                self.misprediction_count += 1;
                // Start smooth correction
                self.correction_offset = [
                    entry.predicted_position[0] - new_pos[0],
                    entry.predicted_position[1] - new_pos[1],
                    entry.predicted_position[2] - new_pos[2],
                ];
                self.correction_timer = self.correction_duration;
            }

            entry.predicted_position = new_pos;
            entry.predicted_rotation = new_rot;
            entry.predicted_velocity = new_vel;

            pos = new_pos;
            rot = new_rot;
            vel = new_vel;
        }

        pos
    }

    /// Get the visual position with smoothed correction applied.
    pub fn visual_position(&self, predicted_position: [f32; 3], dt: f32) -> [f32; 3] {
        if self.correction_timer <= 0.0 {
            return predicted_position;
        }
        let t = (self.correction_timer / self.correction_duration).clamp(0.0, 1.0);
        // Smooth interpolation of the error offset
        let smooth_t = t * t * (3.0 - 2.0 * t); // smoothstep
        [
            predicted_position[0] + self.correction_offset[0] * smooth_t,
            predicted_position[1] + self.correction_offset[1] * smooth_t,
            predicted_position[2] + self.correction_offset[2] * smooth_t,
        ]
    }

    /// Tick the correction timer.
    pub fn update_correction(&mut self, dt: f32) {
        if self.correction_timer > 0.0 {
            self.correction_timer = (self.correction_timer - dt).max(0.0);
        }
    }

    /// Get inputs that haven't been acknowledged yet, for retransmission.
    pub fn unacknowledged_inputs(&self) -> Vec<&PredictionEntry> {
        self.pending_inputs.iter().filter(|e| !e.acknowledged).collect()
    }

    /// Mark inputs as acknowledged up to and including the given sequence.
    pub fn acknowledge_up_to(&mut self, sequence: u32) {
        for entry in self.pending_inputs.iter_mut() {
            if entry.sequence <= sequence {
                entry.acknowledged = true;
            }
        }
    }

    pub fn clear(&mut self) {
        self.pending_inputs.clear();
        self.correction_offset = [0.0; 3];
        self.correction_timer = 0.0;
    }

    pub fn server_position(&self) -> [f32; 3] {
        self.last_server_position
    }

    pub fn server_rotation(&self) -> [f32; 4] {
        self.last_server_rotation
    }
}

/// Authority models for networked entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityMode {
    /// Server has full authority. Client sends inputs, server resolves.
    ServerAuth,
    /// Client has authority over this entity. Server accepts client state.
    ClientAuth,
    /// Shared authority: client predicts, server validates and can override.
    SharedAuth,
}

/// Authority tracking for a set of entities.
pub struct AuthorityModel {
    authorities: HashMap<SyncEntityId, AuthorityEntry>,
    default_mode: AuthorityMode,
}

#[derive(Debug, Clone)]
struct AuthorityEntry {
    mode: AuthorityMode,
    owner_client: u32,
    transfer_pending: bool,
    transfer_target: Option<u32>,
    lock_timestamp_ms: u64,
}

impl AuthorityModel {
    pub fn new(default_mode: AuthorityMode) -> Self {
        Self {
            authorities: HashMap::new(),
            default_mode,
        }
    }

    pub fn set_default_mode(&mut self, mode: AuthorityMode) {
        self.default_mode = mode;
    }

    pub fn register(&mut self, entity_id: SyncEntityId, mode: AuthorityMode, owner: u32) {
        self.authorities.insert(entity_id, AuthorityEntry {
            mode,
            owner_client: owner,
            transfer_pending: false,
            transfer_target: None,
            lock_timestamp_ms: 0,
        });
    }

    pub fn unregister(&mut self, entity_id: SyncEntityId) {
        self.authorities.remove(&entity_id);
    }

    pub fn get_mode(&self, entity_id: SyncEntityId) -> AuthorityMode {
        self.authorities.get(&entity_id).map(|e| e.mode).unwrap_or(self.default_mode)
    }

    pub fn get_owner(&self, entity_id: SyncEntityId) -> Option<u32> {
        self.authorities.get(&entity_id).map(|e| e.owner_client)
    }

    pub fn set_mode(&mut self, entity_id: SyncEntityId, mode: AuthorityMode) {
        if let Some(entry) = self.authorities.get_mut(&entity_id) {
            entry.mode = mode;
        }
    }

    pub fn set_owner(&mut self, entity_id: SyncEntityId, owner: u32) {
        if let Some(entry) = self.authorities.get_mut(&entity_id) {
            entry.owner_client = owner;
        }
    }

    /// Request an authority transfer.
    pub fn request_transfer(&mut self, entity_id: SyncEntityId, target_client: u32, timestamp_ms: u64) -> bool {
        if let Some(entry) = self.authorities.get_mut(&entity_id) {
            if entry.transfer_pending {
                return false;
            }
            entry.transfer_pending = true;
            entry.transfer_target = Some(target_client);
            entry.lock_timestamp_ms = timestamp_ms;
            true
        } else {
            false
        }
    }

    /// Complete a pending transfer.
    pub fn complete_transfer(&mut self, entity_id: SyncEntityId) -> bool {
        if let Some(entry) = self.authorities.get_mut(&entity_id) {
            if entry.transfer_pending {
                if let Some(target) = entry.transfer_target.take() {
                    entry.owner_client = target;
                    entry.transfer_pending = false;
                    return true;
                }
            }
        }
        false
    }

    /// Cancel a pending transfer.
    pub fn cancel_transfer(&mut self, entity_id: SyncEntityId) {
        if let Some(entry) = self.authorities.get_mut(&entity_id) {
            entry.transfer_pending = false;
            entry.transfer_target = None;
        }
    }

    /// Check whether a given client has authority over an entity.
    pub fn has_authority(&self, entity_id: SyncEntityId, client_id: u32) -> bool {
        match self.authorities.get(&entity_id) {
            Some(entry) => {
                match entry.mode {
                    AuthorityMode::ServerAuth => client_id == 0, // 0 = server
                    AuthorityMode::ClientAuth => entry.owner_client == client_id,
                    AuthorityMode::SharedAuth => true,
                }
            }
            None => {
                match self.default_mode {
                    AuthorityMode::ServerAuth => client_id == 0,
                    _ => true,
                }
            }
        }
    }

    /// Whether a transfer is in progress for the given entity.
    pub fn is_transfer_pending(&self, entity_id: SyncEntityId) -> bool {
        self.authorities.get(&entity_id).map(|e| e.transfer_pending).unwrap_or(false)
    }

    pub fn entity_count(&self) -> usize {
        self.authorities.len()
    }
}

/// Flags for replicated property change tracking.
#[derive(Debug, Clone, Copy)]
pub struct PropertyFlags(pub u64);

impl PropertyFlags {
    pub fn empty() -> Self {
        PropertyFlags(0)
    }

    pub fn all() -> Self {
        PropertyFlags(u64::MAX)
    }

    pub fn set(&mut self, bit: u32) {
        self.0 |= 1u64 << bit;
    }

    pub fn clear(&mut self, bit: u32) {
        self.0 &= !(1u64 << bit);
    }

    pub fn is_set(&self, bit: u32) -> bool {
        (self.0 & (1u64 << bit)) != 0
    }

    pub fn any_set(&self) -> bool {
        self.0 != 0
    }

    pub fn clear_all(&mut self) {
        self.0 = 0;
    }

    pub fn count_set(&self) -> u32 {
        self.0.count_ones()
    }

    pub fn union(&self, other: PropertyFlags) -> PropertyFlags {
        PropertyFlags(self.0 | other.0)
    }

    pub fn intersection(&self, other: PropertyFlags) -> PropertyFlags {
        PropertyFlags(self.0 & other.0)
    }
}

/// A single replicated property with dirty tracking.
#[derive(Debug, Clone)]
pub struct ReplicatedProperty {
    pub name: String,
    pub property_index: u32,
    pub data: Vec<u8>,
    pub previous_data: Vec<u8>,
    pub reliable: bool,
    pub interpolate: bool,
    pub priority: f32,
    generation: u32,
}

impl ReplicatedProperty {
    pub fn new(name: String, property_index: u32, initial_data: Vec<u8>) -> Self {
        let prev = initial_data.clone();
        Self {
            name,
            property_index,
            data: initial_data,
            previous_data: prev,
            reliable: false,
            interpolate: true,
            priority: 1.0,
            generation: 0,
        }
    }

    pub fn set_reliable(mut self, reliable: bool) -> Self {
        self.reliable = reliable;
        self
    }

    pub fn set_interpolate(mut self, interpolate: bool) -> Self {
        self.interpolate = interpolate;
        self
    }

    pub fn set_priority(mut self, priority: f32) -> Self {
        self.priority = priority;
        self
    }

    pub fn update(&mut self, new_data: Vec<u8>) {
        if self.data != new_data {
            self.previous_data = std::mem::replace(&mut self.data, new_data);
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.data != self.previous_data
    }

    pub fn clear_dirty(&mut self) {
        self.previous_data = self.data.clone();
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// Tracks dirty state for a set of replicated properties on an entity.
pub struct DirtyTracker {
    entity_id: SyncEntityId,
    properties: Vec<ReplicatedProperty>,
    dirty_flags: PropertyFlags,
    force_full_update: bool,
    last_replicated_generation: HashMap<u32, u32>,
}

impl DirtyTracker {
    pub fn new(entity_id: SyncEntityId) -> Self {
        Self {
            entity_id,
            properties: Vec::new(),
            dirty_flags: PropertyFlags::empty(),
            force_full_update: false,
            last_replicated_generation: HashMap::new(),
        }
    }

    pub fn entity_id(&self) -> SyncEntityId {
        self.entity_id
    }

    pub fn add_property(&mut self, property: ReplicatedProperty) {
        let idx = property.property_index;
        self.properties.push(property);
        self.last_replicated_generation.insert(idx, 0);
    }

    pub fn update_property(&mut self, index: u32, data: Vec<u8>) {
        if let Some(prop) = self.properties.iter_mut().find(|p| p.property_index == index) {
            prop.update(data);
            if prop.is_dirty() {
                self.dirty_flags.set(index);
            }
        }
    }

    pub fn get_property(&self, index: u32) -> Option<&ReplicatedProperty> {
        self.properties.iter().find(|p| p.property_index == index)
    }

    pub fn dirty_flags(&self) -> PropertyFlags {
        self.dirty_flags
    }

    pub fn has_dirty(&self) -> bool {
        self.dirty_flags.any_set() || self.force_full_update
    }

    pub fn force_full_update(&mut self) {
        self.force_full_update = true;
    }

    /// Collect dirty properties for replication. Returns (property_index, data) pairs.
    pub fn collect_dirty(&self) -> Vec<(u32, Vec<u8>)> {
        let mut result = Vec::new();
        for prop in &self.properties {
            if self.force_full_update || self.dirty_flags.is_set(prop.property_index) {
                result.push((prop.property_index, prop.data.clone()));
            }
        }
        result
    }

    /// Collect only reliable dirty properties.
    pub fn collect_reliable_dirty(&self) -> Vec<(u32, Vec<u8>)> {
        let mut result = Vec::new();
        for prop in &self.properties {
            if prop.reliable && (self.force_full_update || self.dirty_flags.is_set(prop.property_index)) {
                result.push((prop.property_index, prop.data.clone()));
            }
        }
        result
    }

    /// Collect only unreliable dirty properties.
    pub fn collect_unreliable_dirty(&self) -> Vec<(u32, Vec<u8>)> {
        let mut result = Vec::new();
        for prop in &self.properties {
            if !prop.reliable && (self.force_full_update || self.dirty_flags.is_set(prop.property_index)) {
                result.push((prop.property_index, prop.data.clone()));
            }
        }
        result
    }

    /// Mark all dirty properties as clean after replication.
    pub fn clear_dirty(&mut self) {
        for prop in &mut self.properties {
            if self.dirty_flags.is_set(prop.property_index) {
                self.last_replicated_generation.insert(prop.property_index, prop.generation());
                prop.clear_dirty();
            }
        }
        self.dirty_flags.clear_all();
        self.force_full_update = false;
    }

    pub fn property_count(&self) -> usize {
        self.properties.len()
    }

    /// Total estimated replication size for dirty properties.
    pub fn dirty_size(&self) -> usize {
        let mut size = 0;
        for prop in &self.properties {
            if self.force_full_update || self.dirty_flags.is_set(prop.property_index) {
                size += 4 + prop.size(); // index + data
            }
        }
        size
    }
}

/// An event representing an entity spawn on the network.
#[derive(Debug, Clone)]
pub struct SpawnEvent {
    pub entity_id: SyncEntityId,
    pub entity_type: u16,
    pub owner_client: u32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub initial_properties: Vec<(u32, Vec<u8>)>,
    pub timestamp_ms: u64,
    pub authority_mode: AuthorityMode,
}

impl SpawnEvent {
    pub fn new(entity_id: SyncEntityId, entity_type: u16, position: [f32; 3]) -> Self {
        Self {
            entity_id,
            entity_type,
            owner_client: 0,
            position,
            rotation: [0.0, 0.0, 0.0, 1.0],
            initial_properties: Vec::new(),
            timestamp_ms: 0,
            authority_mode: AuthorityMode::ServerAuth,
        }
    }

    pub fn with_owner(mut self, owner: u32) -> Self {
        self.owner_client = owner;
        self
    }

    pub fn with_rotation(mut self, rotation: [f32; 4]) -> Self {
        self.rotation = rotation;
        self
    }

    pub fn with_property(mut self, index: u32, data: Vec<u8>) -> Self {
        self.initial_properties.push((index, data));
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp_ms = ts;
        self
    }

    pub fn with_authority(mut self, mode: AuthorityMode) -> Self {
        self.authority_mode = mode;
        self
    }

    pub fn estimated_size(&self) -> usize {
        let base = 4 + 2 + 4 + 12 + 16 + 8 + 1;
        let props: usize = self.initial_properties.iter().map(|(_, d)| 4 + d.len()).sum();
        base + props
    }
}

/// An event representing an entity despawn.
#[derive(Debug, Clone)]
pub struct DespawnEvent {
    pub entity_id: SyncEntityId,
    pub reason: DespawnReason,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DespawnReason {
    Destroyed,
    OutOfRelevancy,
    OwnerDisconnected,
    ServerCommand,
}

impl DespawnEvent {
    pub fn new(entity_id: SyncEntityId, reason: DespawnReason) -> Self {
        Self {
            entity_id,
            reason,
            timestamp_ms: 0,
        }
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp_ms = ts;
        self
    }
}

/// Manages spawn/despawn replication across the network.
pub struct SpawnDespawnReplicator {
    pending_spawns: VecDeque<SpawnEvent>,
    pending_despawns: VecDeque<DespawnEvent>,
    spawned_entities: HashMap<SyncEntityId, SpawnEvent>,
    acknowledged_spawns: HashMap<SyncEntityId, Vec<u32>>,
    max_pending: usize,
}

impl SpawnDespawnReplicator {
    pub fn new() -> Self {
        Self {
            pending_spawns: VecDeque::new(),
            pending_despawns: VecDeque::new(),
            spawned_entities: HashMap::new(),
            acknowledged_spawns: HashMap::new(),
            max_pending: 256,
        }
    }

    /// Queue an entity spawn for replication.
    pub fn queue_spawn(&mut self, event: SpawnEvent) {
        let id = event.entity_id;
        self.spawned_entities.insert(id, event.clone());
        self.pending_spawns.push_back(event);

        while self.pending_spawns.len() > self.max_pending {
            self.pending_spawns.pop_front();
        }
    }

    /// Queue an entity despawn for replication.
    pub fn queue_despawn(&mut self, event: DespawnEvent) {
        let id = event.entity_id;
        self.spawned_entities.remove(&id);
        self.acknowledged_spawns.remove(&id);
        self.pending_despawns.push_back(event);

        while self.pending_despawns.len() > self.max_pending {
            self.pending_despawns.pop_front();
        }
    }

    /// Acknowledge that a client has received a spawn event.
    pub fn acknowledge_spawn(&mut self, entity_id: SyncEntityId, client_id: u32) {
        let clients = self.acknowledged_spawns.entry(entity_id).or_insert_with(Vec::new);
        if !clients.contains(&client_id) {
            clients.push(client_id);
        }
    }

    /// Check if a client knows about an entity.
    pub fn client_knows_entity(&self, entity_id: SyncEntityId, client_id: u32) -> bool {
        self.acknowledged_spawns.get(&entity_id)
            .map(|clients| clients.contains(&client_id))
            .unwrap_or(false)
    }

    /// Get pending spawns for a specific client (entities it doesn't know about yet).
    pub fn pending_spawns_for_client(&self, client_id: u32) -> Vec<&SpawnEvent> {
        self.spawned_entities.values()
            .filter(|e| !self.client_knows_entity(e.entity_id, client_id))
            .collect()
    }

    /// Drain all pending spawns.
    pub fn drain_spawns(&mut self) -> Vec<SpawnEvent> {
        self.pending_spawns.drain(..).collect()
    }

    /// Drain all pending despawns.
    pub fn drain_despawns(&mut self) -> Vec<DespawnEvent> {
        self.pending_despawns.drain(..).collect()
    }

    pub fn spawned_count(&self) -> usize {
        self.spawned_entities.len()
    }

    pub fn pending_spawn_count(&self) -> usize {
        self.pending_spawns.len()
    }

    pub fn pending_despawn_count(&self) -> usize {
        self.pending_despawns.len()
    }

    pub fn clear(&mut self) {
        self.pending_spawns.clear();
        self.pending_despawns.clear();
        self.spawned_entities.clear();
        self.acknowledged_spawns.clear();
    }

    /// Get the spawn event for a specific entity.
    pub fn get_spawn_event(&self, entity_id: SyncEntityId) -> Option<&SpawnEvent> {
        self.spawned_entities.get(&entity_id)
    }

    /// Remove client tracking when they disconnect.
    pub fn remove_client(&mut self, client_id: u32) {
        for clients in self.acknowledged_spawns.values_mut() {
            clients.retain(|&c| c != client_id);
        }
    }
}

/// A single clock synchronization sample.
#[derive(Debug, Clone, Copy)]
pub struct ClockSyncSample {
    pub local_send_time_ms: u64,
    pub remote_time_ms: u64,
    pub local_recv_time_ms: u64,
    pub rtt_ms: u64,
    pub offset_ms: i64,
}

impl ClockSyncSample {
    pub fn new(
        local_send_time_ms: u64,
        remote_time_ms: u64,
        local_recv_time_ms: u64,
    ) -> Self {
        let rtt_ms = local_recv_time_ms.saturating_sub(local_send_time_ms);
        let half_rtt = (rtt_ms / 2) as i64;
        let offset_ms = remote_time_ms as i64 - local_send_time_ms as i64 - half_rtt;
        Self {
            local_send_time_ms,
            remote_time_ms,
            local_recv_time_ms,
            rtt_ms,
            offset_ms,
        }
    }
}

/// State of the clock sync algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    Unsynchronized,
    Synchronizing,
    Synchronized,
}

/// NTP-like clock synchronization between client and server.
pub struct ClockSync {
    samples: VecDeque<ClockSyncSample>,
    max_samples: usize,
    estimated_offset_ms: i64,
    estimated_rtt_ms: f64,
    rtt_variance_ms: f64,
    state: SyncState,
    min_samples_for_sync: usize,
    outlier_threshold: f64,
    last_sync_time_ms: u64,
    sync_interval_ms: u64,
    drift_rate: f64,
    last_drift_update_ms: u64,
    accumulated_drift_ms: f64,
    convergence_rate: f64,
}

impl ClockSync {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            max_samples: 16,
            estimated_offset_ms: 0,
            estimated_rtt_ms: 0.0,
            rtt_variance_ms: 0.0,
            state: SyncState::Unsynchronized,
            min_samples_for_sync: 5,
            outlier_threshold: 2.0,
            last_sync_time_ms: 0,
            sync_interval_ms: 2000,
            drift_rate: 0.0,
            last_drift_update_ms: 0,
            accumulated_drift_ms: 0.0,
            convergence_rate: 0.1,
        }
    }

    pub fn state(&self) -> SyncState {
        self.state
    }

    pub fn estimated_offset_ms(&self) -> i64 {
        self.estimated_offset_ms
    }

    pub fn estimated_rtt_ms(&self) -> f64 {
        self.estimated_rtt_ms
    }

    pub fn is_synchronized(&self) -> bool {
        self.state == SyncState::Synchronized
    }

    pub fn set_sync_interval(&mut self, interval_ms: u64) {
        self.sync_interval_ms = interval_ms;
    }

    pub fn set_convergence_rate(&mut self, rate: f64) {
        self.convergence_rate = rate.clamp(0.01, 1.0);
    }

    /// Add a new synchronization sample.
    pub fn add_sample(&mut self, sample: ClockSyncSample) {
        if self.state == SyncState::Unsynchronized {
            self.state = SyncState::Synchronizing;
        }

        self.samples.push_back(sample);
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }

        self.recompute_estimate();

        if self.samples.len() >= self.min_samples_for_sync {
            self.state = SyncState::Synchronized;
        }
    }

    /// Recompute the clock offset estimate by filtering outliers and averaging.
    fn recompute_estimate(&mut self) {
        if self.samples.is_empty() {
            return;
        }

        // Compute median RTT to detect outliers
        let mut rtts: Vec<u64> = self.samples.iter().map(|s| s.rtt_ms).collect();
        rtts.sort();
        let median_rtt = rtts[rtts.len() / 2] as f64;

        // Compute RTT standard deviation
        let mean_rtt: f64 = rtts.iter().map(|&r| r as f64).sum::<f64>() / rtts.len() as f64;
        let variance: f64 = rtts.iter()
            .map(|&r| { let d = r as f64 - mean_rtt; d * d })
            .sum::<f64>() / rtts.len() as f64;
        let std_dev = variance.sqrt();
        self.rtt_variance_ms = std_dev;

        // Filter outliers: keep samples within threshold * std_dev of median
        let threshold = self.outlier_threshold;
        let valid_samples: Vec<&ClockSyncSample> = self.samples.iter()
            .filter(|s| {
                let diff = (s.rtt_ms as f64 - median_rtt).abs();
                diff <= threshold * std_dev.max(1.0)
            })
            .collect();

        if valid_samples.is_empty() {
            // Fall back to all samples
            let sum: i64 = self.samples.iter().map(|s| s.offset_ms).sum();
            let new_offset = sum / self.samples.len() as i64;
            self.estimated_offset_ms = self.smooth_offset(new_offset);
            self.estimated_rtt_ms = mean_rtt;
            return;
        }

        // Weight by inverse RTT (lower RTT = more accurate)
        let mut weighted_offset: f64 = 0.0;
        let mut total_weight: f64 = 0.0;
        let mut rtt_sum: f64 = 0.0;

        for sample in &valid_samples {
            let weight = 1.0 / (sample.rtt_ms as f64 + 1.0);
            weighted_offset += sample.offset_ms as f64 * weight;
            total_weight += weight;
            rtt_sum += sample.rtt_ms as f64;
        }

        if total_weight > 0.0 {
            let new_offset = (weighted_offset / total_weight) as i64;
            self.estimated_offset_ms = self.smooth_offset(new_offset);
            self.estimated_rtt_ms = rtt_sum / valid_samples.len() as f64;
        }
    }

    /// Smoothly converge toward a new offset to avoid time jumps.
    fn smooth_offset(&self, new_offset: i64) -> i64 {
        if self.state == SyncState::Unsynchronized || self.state == SyncState::Synchronizing {
            return new_offset;
        }
        let diff = new_offset - self.estimated_offset_ms;
        let adjustment = (diff as f64 * self.convergence_rate) as i64;
        self.estimated_offset_ms + adjustment
    }

    /// Convert a local timestamp to estimated remote time.
    pub fn local_to_remote(&self, local_ms: u64) -> u64 {
        let adjusted = local_ms as i64 + self.estimated_offset_ms + self.accumulated_drift_ms as i64;
        adjusted.max(0) as u64
    }

    /// Convert a remote timestamp to estimated local time.
    pub fn remote_to_local(&self, remote_ms: u64) -> u64 {
        let adjusted = remote_ms as i64 - self.estimated_offset_ms - self.accumulated_drift_ms as i64;
        adjusted.max(0) as u64
    }

    /// Whether it's time to send another sync request.
    pub fn needs_sync(&self, current_time_ms: u64) -> bool {
        if self.state == SyncState::Unsynchronized {
            return true;
        }
        current_time_ms.saturating_sub(self.last_sync_time_ms) >= self.sync_interval_ms
    }

    /// Mark that we sent a sync request.
    pub fn on_sync_sent(&mut self, time_ms: u64) {
        self.last_sync_time_ms = time_ms;
    }

    /// Update drift estimation. Call periodically.
    pub fn update_drift(&mut self, current_time_ms: u64) {
        if self.last_drift_update_ms == 0 {
            self.last_drift_update_ms = current_time_ms;
            return;
        }

        let dt_ms = current_time_ms.saturating_sub(self.last_drift_update_ms);
        self.last_drift_update_ms = current_time_ms;

        // Estimate drift from recent offset changes
        if self.samples.len() >= 4 {
            let recent_half = &self.samples.as_slices().0;
            let first_half_len = self.samples.len() / 2;
            if first_half_len > 0 && self.samples.len() > first_half_len {
                let first_avg: f64 = self.samples.iter().take(first_half_len)
                    .map(|s| s.offset_ms as f64).sum::<f64>() / first_half_len as f64;
                let second_avg: f64 = self.samples.iter().skip(first_half_len)
                    .map(|s| s.offset_ms as f64).sum::<f64>() / (self.samples.len() - first_half_len) as f64;

                let first_time = self.samples.iter().take(first_half_len)
                    .map(|s| s.local_recv_time_ms as f64).sum::<f64>() / first_half_len as f64;
                let second_time = self.samples.iter().skip(first_half_len)
                    .map(|s| s.local_recv_time_ms as f64).sum::<f64>() / (self.samples.len() - first_half_len) as f64;

                let time_diff = second_time - first_time;
                if time_diff > 100.0 {
                    self.drift_rate = (second_avg - first_avg) / time_diff;
                }

                let _ = recent_half; // suppress unused warning
            }
        }

        self.accumulated_drift_ms += self.drift_rate * dt_ms as f64;
        // Clamp accumulated drift
        self.accumulated_drift_ms = self.accumulated_drift_ms.clamp(-5000.0, 5000.0);
    }

    pub fn drift_rate(&self) -> f64 {
        self.drift_rate
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn reset(&mut self) {
        self.samples.clear();
        self.estimated_offset_ms = 0;
        self.estimated_rtt_ms = 0.0;
        self.rtt_variance_ms = 0.0;
        self.state = SyncState::Unsynchronized;
        self.drift_rate = 0.0;
        self.accumulated_drift_ms = 0.0;
        self.last_drift_update_ms = 0;
    }

    /// Get the confidence of the current estimate (0.0 to 1.0).
    pub fn confidence(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let count_factor = (self.samples.len() as f64 / self.min_samples_for_sync as f64).min(1.0);
        let variance_factor = 1.0 / (1.0 + self.rtt_variance_ms / 50.0);
        count_factor * variance_factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_sample_lerp() {
        let a = InterpolationSample::new(0, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
        let b = InterpolationSample::new(100, [10.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]);
        let mid = a.lerp(&b, 0.5);
        assert!((mid.position[0] - 5.0).abs() < 0.01);
        assert_eq!(mid.timestamp_ms, 50);
    }

    #[test]
    fn test_interpolation_sample_extrapolate() {
        let sample = InterpolationSample::new(0, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0])
            .with_velocity([10.0, 0.0, 0.0]);
        let ext = sample.extrapolate(1000);
        assert!((ext.position[0] - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolation_buffer() {
        let mut buf = InterpolationBuffer::new(SyncEntityId(1), 50);
        buf.push(InterpolationSample::new(100, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]));
        buf.push(InterpolationSample::new(200, [10.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]));

        // Render time 200 with 50ms delay => target 150 => interpolate between 100 and 200
        let result = buf.sample_at(200).unwrap();
        assert!((result.position[0] - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_client_prediction() {
        let mut pred = ClientPrediction::new(SyncEntityId(1));
        let seq = pred.record_input(
            100, vec![1, 0, 0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0],
        );
        assert_eq!(seq, 0);
        assert_eq!(pred.pending_count(), 1);
    }

    #[test]
    fn test_authority_model() {
        let mut auth = AuthorityModel::new(AuthorityMode::ServerAuth);
        auth.register(SyncEntityId(1), AuthorityMode::ClientAuth, 5);
        assert!(auth.has_authority(SyncEntityId(1), 5));
        assert!(!auth.has_authority(SyncEntityId(1), 3));
        assert_eq!(auth.get_mode(SyncEntityId(1)), AuthorityMode::ClientAuth);
    }

    #[test]
    fn test_authority_transfer() {
        let mut auth = AuthorityModel::new(AuthorityMode::ServerAuth);
        auth.register(SyncEntityId(1), AuthorityMode::ClientAuth, 1);
        assert!(auth.request_transfer(SyncEntityId(1), 2, 100));
        assert!(auth.is_transfer_pending(SyncEntityId(1)));
        assert!(auth.complete_transfer(SyncEntityId(1)));
        assert_eq!(auth.get_owner(SyncEntityId(1)), Some(2));
    }

    #[test]
    fn test_property_flags() {
        let mut flags = PropertyFlags::empty();
        assert!(!flags.any_set());
        flags.set(0);
        flags.set(5);
        assert!(flags.is_set(0));
        assert!(flags.is_set(5));
        assert!(!flags.is_set(1));
        assert_eq!(flags.count_set(), 2);
        flags.clear(0);
        assert!(!flags.is_set(0));
    }

    #[test]
    fn test_dirty_tracker() {
        let mut tracker = DirtyTracker::new(SyncEntityId(1));
        tracker.add_property(ReplicatedProperty::new("health".into(), 0, vec![100]));
        tracker.add_property(ReplicatedProperty::new("pos_x".into(), 1, vec![0, 0, 0, 0]));

        assert!(!tracker.has_dirty());

        tracker.update_property(0, vec![50]);
        assert!(tracker.has_dirty());
        assert!(tracker.dirty_flags().is_set(0));

        let dirty = tracker.collect_dirty();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0].0, 0);
        assert_eq!(dirty[0].1, vec![50]);

        tracker.clear_dirty();
        assert!(!tracker.has_dirty());
    }

    #[test]
    fn test_spawn_despawn_replicator() {
        let mut rep = SpawnDespawnReplicator::new();
        let spawn = SpawnEvent::new(SyncEntityId(1), 42, [1.0, 2.0, 3.0]);
        rep.queue_spawn(spawn);
        assert_eq!(rep.spawned_count(), 1);

        // Client 1 doesn't know about it yet
        let pending = rep.pending_spawns_for_client(1);
        assert_eq!(pending.len(), 1);

        // Acknowledge it
        rep.acknowledge_spawn(SyncEntityId(1), 1);
        assert!(rep.client_knows_entity(SyncEntityId(1), 1));
        let pending2 = rep.pending_spawns_for_client(1);
        assert_eq!(pending2.len(), 0);

        // Despawn
        rep.queue_despawn(DespawnEvent::new(SyncEntityId(1), DespawnReason::Destroyed));
        assert_eq!(rep.spawned_count(), 0);
    }

    #[test]
    fn test_clock_sync() {
        let mut clock = ClockSync::new();
        assert_eq!(clock.state(), SyncState::Unsynchronized);

        // Simulate 5 samples with ~50ms RTT and ~100ms offset
        for i in 0..5 {
            let send = 1000 + i * 2000;
            let remote = send + 100 + 25; // offset=100, half_rtt=25
            let recv = send + 50;
            clock.add_sample(ClockSyncSample::new(send, remote, recv));
        }

        assert_eq!(clock.state(), SyncState::Synchronized);
        // Offset should be approximately 100
        assert!((clock.estimated_offset_ms() - 100).abs() < 10);
    }

    #[test]
    fn test_clock_sync_conversion() {
        let mut clock = ClockSync::new();
        for i in 0..5 {
            let send = i * 1000;
            let remote = send + 200 + 25;
            let recv = send + 50;
            clock.add_sample(ClockSyncSample::new(send, remote, recv));
        }

        let local = 5000u64;
        let remote = clock.local_to_remote(local);
        let back = clock.remote_to_local(remote);
        assert!((back as i64 - local as i64).abs() <= 1);
    }

    #[test]
    fn test_quat_slerp_identity() {
        let q = [0.0, 0.0, 0.0, 1.0];
        let result = quat_slerp(q, q, 0.5);
        assert!((result[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_replicated_property() {
        let mut prop = ReplicatedProperty::new("test".into(), 0, vec![1, 2, 3]);
        assert!(!prop.is_dirty());
        prop.update(vec![4, 5, 6]);
        assert!(prop.is_dirty());
        assert_eq!(prop.generation(), 1);
        prop.clear_dirty();
        assert!(!prop.is_dirty());
    }
}
