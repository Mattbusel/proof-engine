//! World state snapshots with delta compression and relevancy filtering.

use std::collections::HashMap;

/// Unique identifier for a snapshot in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SnapshotId(pub u64);

impl SnapshotId {
    pub fn next(self) -> Self {
        SnapshotId(self.0.wrapping_add(1))
    }

    pub fn distance(self, other: SnapshotId) -> u64 {
        if self.0 >= other.0 {
            self.0 - other.0
        } else {
            other.0 - self.0
        }
    }

    pub fn zero() -> Self {
        SnapshotId(0)
    }
}

/// Strongly-typed entity identifier within the netcode layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NetEntityId(pub u32);

/// Individual component data stored as a type-tagged byte buffer.
/// Components are identified by a numeric type ID for serialization.
#[derive(Debug, Clone)]
pub struct ComponentData {
    pub type_id: u16,
    pub data: Vec<u8>,
    pub version: u32,
}

impl ComponentData {
    pub fn new(type_id: u16, data: Vec<u8>) -> Self {
        Self {
            type_id,
            data,
            version: 1,
        }
    }

    pub fn with_version(type_id: u16, data: Vec<u8>, version: u32) -> Self {
        Self {
            type_id,
            data,
            version,
        }
    }

    pub fn size(&self) -> usize {
        self.data.len() + 6 // type_id(2) + version(4)
    }

    /// Compute a simple FNV-1a hash of the data for change detection.
    pub fn content_hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &byte in &self.data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Produce a delta between this component and another of the same type.
    /// Returns None if data is identical.
    pub fn delta_against(&self, baseline: &ComponentData) -> Option<ComponentDelta> {
        if self.type_id != baseline.type_id {
            return Some(ComponentDelta {
                type_id: self.type_id,
                kind: DeltaKind::Replaced(self.data.clone()),
                new_version: self.version,
            });
        }

        if self.data == baseline.data {
            return None;
        }

        // XOR delta for same-length data
        if self.data.len() == baseline.data.len() {
            let mut xor_data = Vec::with_capacity(self.data.len());
            let mut has_diff = false;
            for i in 0..self.data.len() {
                let x = self.data[i] ^ baseline.data[i];
                if x != 0 {
                    has_diff = true;
                }
                xor_data.push(x);
            }

            if !has_diff {
                return None;
            }

            // Run-length encode the XOR data for compression
            let rle = rle_encode(&xor_data);
            if rle.len() < xor_data.len() {
                return Some(ComponentDelta {
                    type_id: self.type_id,
                    kind: DeltaKind::XorRle(rle),
                    new_version: self.version,
                });
            }

            return Some(ComponentDelta {
                type_id: self.type_id,
                kind: DeltaKind::XorRaw(xor_data),
                new_version: self.version,
            });
        }

        // Different lengths: full replacement
        Some(ComponentDelta {
            type_id: self.type_id,
            kind: DeltaKind::Replaced(self.data.clone()),
            new_version: self.version,
        })
    }

    /// Apply a delta to produce a new ComponentData.
    pub fn apply_delta(&self, delta: &ComponentDelta) -> ComponentData {
        let new_data = match &delta.kind {
            DeltaKind::Replaced(data) => data.clone(),
            DeltaKind::XorRaw(xor) => {
                let mut result = self.data.clone();
                for i in 0..result.len().min(xor.len()) {
                    result[i] ^= xor[i];
                }
                result
            }
            DeltaKind::XorRle(rle) => {
                let xor = rle_decode(rle, self.data.len());
                let mut result = self.data.clone();
                for i in 0..result.len().min(xor.len()) {
                    result[i] ^= xor[i];
                }
                result
            }
        };
        ComponentData {
            type_id: delta.type_id,
            data: new_data,
            version: delta.new_version,
        }
    }
}

/// Run-length encode: pairs of (count, value).
fn rle_encode(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    if data.is_empty() {
        return result;
    }
    let mut current = data[0];
    let mut count: u8 = 1;
    for &byte in &data[1..] {
        if byte == current && count < 255 {
            count += 1;
        } else {
            result.push(count);
            result.push(current);
            current = byte;
            count = 1;
        }
    }
    result.push(count);
    result.push(current);
    result
}

/// Decode RLE data back to original size.
fn rle_decode(rle: &[u8], expected_len: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(expected_len);
    let mut i = 0;
    while i + 1 < rle.len() && result.len() < expected_len {
        let count = rle[i] as usize;
        let value = rle[i + 1];
        for _ in 0..count {
            if result.len() >= expected_len {
                break;
            }
            result.push(value);
        }
        i += 2;
    }
    while result.len() < expected_len {
        result.push(0);
    }
    result
}

/// Snapshot of a single entity's state at a point in time.
#[derive(Debug, Clone)]
pub struct EntitySnapshot {
    pub entity_id: NetEntityId,
    pub components: Vec<ComponentData>,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub flags: u32,
}

impl EntitySnapshot {
    pub fn new(entity_id: NetEntityId) -> Self {
        Self {
            entity_id,
            components: Vec::new(),
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0; 3],
            flags: 0,
        }
    }

    pub fn with_position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = [x, y, z];
        self
    }

    pub fn with_rotation(mut self, x: f32, y: f32, z: f32, w: f32) -> Self {
        self.rotation = [x, y, z, w];
        self
    }

    pub fn with_velocity(mut self, x: f32, y: f32, z: f32) -> Self {
        self.velocity = [x, y, z];
        self
    }

    pub fn add_component(&mut self, component: ComponentData) {
        // Replace if same type_id exists
        if let Some(existing) = self.components.iter_mut().find(|c| c.type_id == component.type_id) {
            *existing = component;
        } else {
            self.components.push(component);
        }
    }

    pub fn get_component(&self, type_id: u16) -> Option<&ComponentData> {
        self.components.iter().find(|c| c.type_id == type_id)
    }

    pub fn remove_component(&mut self, type_id: u16) -> bool {
        let len_before = self.components.len();
        self.components.retain(|c| c.type_id != type_id);
        self.components.len() < len_before
    }

    pub fn total_size(&self) -> usize {
        let base = 4 + 12 + 16 + 12 + 4; // id + pos + rot + vel + flags
        let comp_size: usize = self.components.iter().map(|c| c.size()).sum();
        base + comp_size
    }

    /// Compute delta between this snapshot and a baseline of the same entity.
    pub fn delta_against(&self, baseline: &EntitySnapshot) -> EntityDelta {
        let mut changed_components = Vec::new();
        let mut added_components = Vec::new();
        let mut removed_component_types = Vec::new();

        // Check for changed/added components
        for comp in &self.components {
            if let Some(base_comp) = baseline.get_component(comp.type_id) {
                if let Some(delta) = comp.delta_against(base_comp) {
                    changed_components.push(delta);
                }
            } else {
                added_components.push(comp.clone());
            }
        }

        // Check for removed components
        for base_comp in &baseline.components {
            if self.get_component(base_comp.type_id).is_none() {
                removed_component_types.push(base_comp.type_id);
            }
        }

        // Position delta
        let pos_changed = (self.position[0] - baseline.position[0]).abs() > f32::EPSILON
            || (self.position[1] - baseline.position[1]).abs() > f32::EPSILON
            || (self.position[2] - baseline.position[2]).abs() > f32::EPSILON;

        let rot_changed = (self.rotation[0] - baseline.rotation[0]).abs() > f32::EPSILON
            || (self.rotation[1] - baseline.rotation[1]).abs() > f32::EPSILON
            || (self.rotation[2] - baseline.rotation[2]).abs() > f32::EPSILON
            || (self.rotation[3] - baseline.rotation[3]).abs() > f32::EPSILON;

        let vel_changed = (self.velocity[0] - baseline.velocity[0]).abs() > f32::EPSILON
            || (self.velocity[1] - baseline.velocity[1]).abs() > f32::EPSILON
            || (self.velocity[2] - baseline.velocity[2]).abs() > f32::EPSILON;

        EntityDelta {
            entity_id: self.entity_id,
            position: if pos_changed { Some(self.position) } else { None },
            rotation: if rot_changed { Some(self.rotation) } else { None },
            velocity: if vel_changed { Some(self.velocity) } else { None },
            flags_changed: self.flags != baseline.flags,
            new_flags: self.flags,
            changed_components,
            added_components,
            removed_component_types,
        }
    }

    /// Apply a delta to produce an updated snapshot.
    pub fn apply_delta(&self, delta: &EntityDelta) -> EntitySnapshot {
        let mut result = self.clone();

        if let Some(pos) = delta.position {
            result.position = pos;
        }
        if let Some(rot) = delta.rotation {
            result.rotation = rot;
        }
        if let Some(vel) = delta.velocity {
            result.velocity = vel;
        }
        if delta.flags_changed {
            result.flags = delta.new_flags;
        }

        // Apply component changes
        for comp_delta in &delta.changed_components {
            if let Some(existing) = result.components.iter_mut().find(|c| c.type_id == comp_delta.type_id) {
                let original = existing.clone();
                *existing = original.apply_delta(comp_delta);
            }
        }

        // Add new components
        for added in &delta.added_components {
            result.add_component(added.clone());
        }

        // Remove components
        for &type_id in &delta.removed_component_types {
            result.remove_component(type_id);
        }

        result
    }
}

/// Delta for a single component.
#[derive(Debug, Clone)]
pub struct ComponentDelta {
    pub type_id: u16,
    pub kind: DeltaKind,
    pub new_version: u32,
}

impl ComponentDelta {
    pub fn size(&self) -> usize {
        2 + 4 + match &self.kind {
            DeltaKind::Replaced(data) => data.len() + 1,
            DeltaKind::XorRaw(data) => data.len() + 1,
            DeltaKind::XorRle(data) => data.len() + 1,
        }
    }
}

/// How a component changed between snapshots.
#[derive(Debug, Clone)]
pub enum DeltaKind {
    Replaced(Vec<u8>),
    XorRaw(Vec<u8>),
    XorRle(Vec<u8>),
}

/// Delta for a single entity between two snapshots.
#[derive(Debug, Clone)]
pub struct EntityDelta {
    pub entity_id: NetEntityId,
    pub position: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub velocity: Option<[f32; 3]>,
    pub flags_changed: bool,
    pub new_flags: u32,
    pub changed_components: Vec<ComponentDelta>,
    pub added_components: Vec<ComponentData>,
    pub removed_component_types: Vec<u16>,
}

impl EntityDelta {
    pub fn is_empty(&self) -> bool {
        self.position.is_none()
            && self.rotation.is_none()
            && self.velocity.is_none()
            && !self.flags_changed
            && self.changed_components.is_empty()
            && self.added_components.is_empty()
            && self.removed_component_types.is_empty()
    }

    pub fn estimated_size(&self) -> usize {
        let mut size = 4; // entity_id
        if self.position.is_some() { size += 12; }
        if self.rotation.is_some() { size += 16; }
        if self.velocity.is_some() { size += 12; }
        if self.flags_changed { size += 4; }
        for cd in &self.changed_components { size += cd.size(); }
        for ac in &self.added_components { size += ac.size(); }
        size += self.removed_component_types.len() * 2;
        size
    }
}

/// A complete snapshot of the world at a given tick.
#[derive(Debug, Clone)]
pub struct WorldSnapshot {
    pub id: SnapshotId,
    pub tick: u64,
    pub timestamp_ms: u64,
    pub entities: HashMap<NetEntityId, EntitySnapshot>,
}

impl WorldSnapshot {
    pub fn new(id: SnapshotId, tick: u64, timestamp_ms: u64) -> Self {
        Self {
            id,
            tick,
            timestamp_ms,
            entities: HashMap::new(),
        }
    }

    pub fn add_entity(&mut self, snapshot: EntitySnapshot) {
        self.entities.insert(snapshot.entity_id, snapshot);
    }

    pub fn remove_entity(&mut self, id: NetEntityId) -> Option<EntitySnapshot> {
        self.entities.remove(&id)
    }

    pub fn get_entity(&self, id: NetEntityId) -> Option<&EntitySnapshot> {
        self.entities.get(&id)
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn total_size(&self) -> usize {
        let header = 8 + 8 + 8; // id + tick + timestamp
        let entity_size: usize = self.entities.values().map(|e| e.total_size()).sum();
        header + entity_size
    }

    /// Compute a delta from a baseline snapshot.
    pub fn delta_from(&self, baseline: &WorldSnapshot) -> SnapshotDelta {
        let mut entity_deltas = Vec::new();
        let mut spawned_entities = Vec::new();
        let mut despawned_entities = Vec::new();

        // Find changed and new entities
        for (id, entity) in &self.entities {
            if let Some(base_entity) = baseline.entities.get(id) {
                let delta = entity.delta_against(base_entity);
                if !delta.is_empty() {
                    entity_deltas.push(delta);
                }
            } else {
                spawned_entities.push(entity.clone());
            }
        }

        // Find despawned entities
        for id in baseline.entities.keys() {
            if !self.entities.contains_key(id) {
                despawned_entities.push(*id);
            }
        }

        SnapshotDelta {
            baseline_id: baseline.id,
            target_id: self.id,
            baseline_tick: baseline.tick,
            target_tick: self.tick,
            timestamp_ms: self.timestamp_ms,
            entity_deltas,
            spawned_entities,
            despawned_entities,
        }
    }

    /// Apply a delta to produce a new WorldSnapshot.
    pub fn apply_delta(&self, delta: &SnapshotDelta) -> WorldSnapshot {
        let mut result = WorldSnapshot::new(delta.target_id, delta.target_tick, delta.timestamp_ms);

        // Copy all existing entities
        for (id, entity) in &self.entities {
            result.entities.insert(*id, entity.clone());
        }

        // Apply entity deltas
        for ed in &delta.entity_deltas {
            if let Some(existing) = self.entities.get(&ed.entity_id) {
                let updated = existing.apply_delta(ed);
                result.entities.insert(ed.entity_id, updated);
            }
        }

        // Add spawned entities
        for spawned in &delta.spawned_entities {
            result.entities.insert(spawned.entity_id, spawned.clone());
        }

        // Remove despawned entities
        for &id in &delta.despawned_entities {
            result.entities.remove(&id);
        }

        result
    }

    /// Merge another snapshot into this one (union of entities, preferring other on conflict).
    pub fn merge_from(&mut self, other: &WorldSnapshot) {
        for (id, entity) in &other.entities {
            self.entities.insert(*id, entity.clone());
        }
        if other.tick > self.tick {
            self.tick = other.tick;
            self.id = other.id;
            self.timestamp_ms = other.timestamp_ms;
        }
    }
}

/// Delta between two world snapshots.
#[derive(Debug, Clone)]
pub struct SnapshotDelta {
    pub baseline_id: SnapshotId,
    pub target_id: SnapshotId,
    pub baseline_tick: u64,
    pub target_tick: u64,
    pub timestamp_ms: u64,
    pub entity_deltas: Vec<EntityDelta>,
    pub spawned_entities: Vec<EntitySnapshot>,
    pub despawned_entities: Vec<NetEntityId>,
}

impl SnapshotDelta {
    pub fn is_empty(&self) -> bool {
        self.entity_deltas.is_empty()
            && self.spawned_entities.is_empty()
            && self.despawned_entities.is_empty()
    }

    pub fn estimated_size(&self) -> usize {
        let header = 8 + 8 + 8 + 8 + 8;
        let deltas: usize = self.entity_deltas.iter().map(|d| d.estimated_size()).sum();
        let spawns: usize = self.spawned_entities.iter().map(|e| e.total_size()).sum();
        let despawns = self.despawned_entities.len() * 4;
        header + deltas + spawns + despawns
    }

    pub fn delta_count(&self) -> usize {
        self.entity_deltas.len() + self.spawned_entities.len() + self.despawned_entities.len()
    }
}

/// Ring buffer for storing recent world snapshots.
/// Supports efficient lookup by snapshot ID and tick number.
pub struct SnapshotRingBuffer {
    buffer: Vec<Option<WorldSnapshot>>,
    capacity: usize,
    head: usize,
    count: usize,
    latest_id: SnapshotId,
    id_to_index: HashMap<SnapshotId, usize>,
    tick_to_id: HashMap<u64, SnapshotId>,
}

impl SnapshotRingBuffer {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(4);
        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(None);
        }
        Self {
            buffer,
            capacity,
            head: 0,
            count: 0,
            latest_id: SnapshotId(0),
            id_to_index: HashMap::new(),
            tick_to_id: HashMap::new(),
        }
    }

    pub fn push(&mut self, snapshot: WorldSnapshot) {
        // Evict the old entry at this position
        if let Some(ref old) = self.buffer[self.head] {
            self.id_to_index.remove(&old.id);
            self.tick_to_id.remove(&old.tick);
        }

        let id = snapshot.id;
        let tick = snapshot.tick;
        self.id_to_index.insert(id, self.head);
        self.tick_to_id.insert(tick, id);
        self.buffer[self.head] = Some(snapshot);

        if id.0 > self.latest_id.0 {
            self.latest_id = id;
        }

        self.head = (self.head + 1) % self.capacity;
        if self.count < self.capacity {
            self.count += 1;
        }
    }

    pub fn get(&self, id: SnapshotId) -> Option<&WorldSnapshot> {
        self.id_to_index.get(&id).and_then(|&idx| self.buffer[idx].as_ref())
    }

    pub fn get_by_tick(&self, tick: u64) -> Option<&WorldSnapshot> {
        self.tick_to_id.get(&tick).and_then(|id| self.get(*id))
    }

    pub fn latest(&self) -> Option<&WorldSnapshot> {
        if self.count == 0 {
            return None;
        }
        self.get(self.latest_id)
    }

    pub fn latest_id(&self) -> SnapshotId {
        self.latest_id
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn is_full(&self) -> bool {
        self.count >= self.capacity
    }

    /// Get the N most recent snapshots, ordered oldest to newest.
    pub fn recent(&self, n: usize) -> Vec<&WorldSnapshot> {
        let n = n.min(self.count);
        let mut result = Vec::with_capacity(n);
        let start = if self.head >= n {
            self.head - n
        } else {
            self.capacity - (n - self.head)
        };
        for i in 0..n {
            let idx = (start + i) % self.capacity;
            if let Some(ref snap) = self.buffer[idx] {
                result.push(snap);
            }
        }
        result
    }

    /// Find the closest snapshot to a given tick.
    pub fn closest_to_tick(&self, tick: u64) -> Option<&WorldSnapshot> {
        let mut best: Option<&WorldSnapshot> = None;
        let mut best_dist = u64::MAX;
        for entry in &self.buffer {
            if let Some(ref snap) = entry {
                let dist = if snap.tick > tick {
                    snap.tick - tick
                } else {
                    tick - snap.tick
                };
                if dist < best_dist {
                    best_dist = dist;
                    best = Some(snap);
                }
            }
        }
        best
    }

    /// Compute delta between two stored snapshots.
    pub fn compute_delta(&self, baseline_id: SnapshotId, target_id: SnapshotId) -> Option<SnapshotDelta> {
        let baseline = self.get(baseline_id)?;
        let target = self.get(target_id)?;
        Some(target.delta_from(baseline))
    }

    /// Remove all snapshots older than the given tick.
    pub fn prune_before(&mut self, tick: u64) {
        for i in 0..self.capacity {
            if let Some(ref snap) = self.buffer[i] {
                if snap.tick < tick {
                    let id = snap.id;
                    let t = snap.tick;
                    self.id_to_index.remove(&id);
                    self.tick_to_id.remove(&t);
                    self.buffer[i] = None;
                    if self.count > 0 {
                        self.count -= 1;
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        for i in 0..self.capacity {
            self.buffer[i] = None;
        }
        self.id_to_index.clear();
        self.tick_to_id.clear();
        self.head = 0;
        self.count = 0;
    }

    /// Collect all snapshot IDs currently stored, in no particular order.
    pub fn stored_ids(&self) -> Vec<SnapshotId> {
        self.id_to_index.keys().copied().collect()
    }
}

/// Priority and relevancy information for a single entity relative to a client.
#[derive(Debug, Clone)]
pub struct RelevancyEntry {
    pub entity_id: NetEntityId,
    pub priority: f32,
    pub distance_sq: f32,
    pub is_relevant: bool,
    pub last_sent_tick: u64,
    pub update_frequency: f32,
    pub accumulated_priority: f32,
}

impl RelevancyEntry {
    pub fn new(entity_id: NetEntityId) -> Self {
        Self {
            entity_id,
            priority: 1.0,
            distance_sq: 0.0,
            is_relevant: true,
            last_sent_tick: 0,
            update_frequency: 1.0,
            accumulated_priority: 0.0,
        }
    }
}

/// A spatial region used for area-of-interest filtering.
#[derive(Debug, Clone)]
pub struct RelevancyRegion {
    pub center: [f32; 3],
    pub radius: f32,
    pub priority_falloff: f32,
    pub min_priority: f32,
}

impl RelevancyRegion {
    pub fn new(center: [f32; 3], radius: f32) -> Self {
        Self {
            center,
            radius,
            priority_falloff: 1.0,
            min_priority: 0.0,
        }
    }

    pub fn contains(&self, pos: [f32; 3]) -> bool {
        let dx = pos[0] - self.center[0];
        let dy = pos[1] - self.center[1];
        let dz = pos[2] - self.center[2];
        dx * dx + dy * dy + dz * dz <= self.radius * self.radius
    }

    pub fn distance_sq(&self, pos: [f32; 3]) -> f32 {
        let dx = pos[0] - self.center[0];
        let dy = pos[1] - self.center[1];
        let dz = pos[2] - self.center[2];
        dx * dx + dy * dy + dz * dz
    }

    pub fn priority_at(&self, pos: [f32; 3]) -> f32 {
        let dist_sq = self.distance_sq(pos);
        let radius_sq = self.radius * self.radius;
        if dist_sq >= radius_sq {
            return self.min_priority;
        }
        let t = 1.0 - (dist_sq / radius_sq).sqrt();
        let p = t.powf(self.priority_falloff);
        self.min_priority + (1.0 - self.min_priority) * p
    }
}

/// Client identifier for relevancy tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u32);

/// Per-client relevancy filter that determines which entities
/// should be sent and at what priority/frequency.
pub struct RelevancyFilter {
    client_id: ClientId,
    region: RelevancyRegion,
    entries: HashMap<NetEntityId, RelevancyEntry>,
    max_entities_per_update: usize,
    bandwidth_budget_bytes: usize,
    always_relevant: Vec<NetEntityId>,
    never_relevant: Vec<NetEntityId>,
    priority_bias: HashMap<u16, f32>,
    current_tick: u64,
}

impl RelevancyFilter {
    pub fn new(client_id: ClientId, region: RelevancyRegion) -> Self {
        Self {
            client_id,
            region,
            entries: HashMap::new(),
            max_entities_per_update: 64,
            bandwidth_budget_bytes: 16384,
            always_relevant: Vec::new(),
            never_relevant: Vec::new(),
            priority_bias: HashMap::new(),
            current_tick: 0,
        }
    }

    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub fn set_region(&mut self, region: RelevancyRegion) {
        self.region = region;
    }

    pub fn region(&self) -> &RelevancyRegion {
        &self.region
    }

    pub fn set_max_entities(&mut self, max: usize) {
        self.max_entities_per_update = max;
    }

    pub fn set_bandwidth_budget(&mut self, bytes: usize) {
        self.bandwidth_budget_bytes = bytes;
    }

    pub fn add_always_relevant(&mut self, id: NetEntityId) {
        if !self.always_relevant.contains(&id) {
            self.always_relevant.push(id);
        }
    }

    pub fn remove_always_relevant(&mut self, id: NetEntityId) {
        self.always_relevant.retain(|&x| x != id);
    }

    pub fn add_never_relevant(&mut self, id: NetEntityId) {
        if !self.never_relevant.contains(&id) {
            self.never_relevant.push(id);
        }
    }

    pub fn remove_never_relevant(&mut self, id: NetEntityId) {
        self.never_relevant.retain(|&x| x != id);
    }

    pub fn set_component_priority_bias(&mut self, component_type: u16, bias: f32) {
        self.priority_bias.insert(component_type, bias);
    }

    /// Update the filter with a new world snapshot. Recomputes relevancy for all entities.
    pub fn update(&mut self, snapshot: &WorldSnapshot, tick: u64) {
        self.current_tick = tick;

        // Remove entries for entities no longer in the world
        self.entries.retain(|id, _| snapshot.entities.contains_key(id));

        for (id, entity) in &snapshot.entities {
            // Skip never-relevant entities
            if self.never_relevant.contains(id) {
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.is_relevant = false;
                    entry.priority = 0.0;
                }
                continue;
            }

            let entry = self.entries.entry(*id).or_insert_with(|| RelevancyEntry::new(*id));
            let dist_sq = self.region.distance_sq(entity.position);
            entry.distance_sq = dist_sq;

            if self.always_relevant.contains(id) {
                entry.is_relevant = true;
                entry.priority = 10.0;
            } else {
                entry.is_relevant = self.region.contains(entity.position);
                entry.priority = self.region.priority_at(entity.position);
            }

            // Apply component-based priority bias
            for comp in &entity.components {
                if let Some(&bias) = self.priority_bias.get(&comp.type_id) {
                    entry.priority *= bias;
                }
            }

            // Accumulate priority based on time since last sent
            let ticks_since_sent = if tick > entry.last_sent_tick {
                tick - entry.last_sent_tick
            } else {
                0
            };
            entry.accumulated_priority = entry.priority * (1.0 + ticks_since_sent as f32 * 0.1);
        }
    }

    /// Get entities that should be included in the next update,
    /// sorted by accumulated priority (highest first), limited by bandwidth and count.
    pub fn select_entities(&mut self, snapshot: &WorldSnapshot) -> Vec<NetEntityId> {
        let mut candidates: Vec<(NetEntityId, f32)> = self.entries.iter()
            .filter(|(_, entry)| entry.is_relevant)
            .map(|(&id, entry)| (id, entry.accumulated_priority))
            .collect();

        // Sort by accumulated priority descending
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut selected = Vec::new();
        let mut budget_remaining = self.bandwidth_budget_bytes;

        for (id, _priority) in candidates {
            if selected.len() >= self.max_entities_per_update {
                break;
            }

            // Estimate the size of this entity's data
            if let Some(entity) = snapshot.get_entity(id) {
                let est_size = entity.total_size();
                if est_size > budget_remaining && !selected.is_empty() {
                    continue;
                }
                budget_remaining = budget_remaining.saturating_sub(est_size);
                selected.push(id);

                // Mark as sent
                if let Some(entry) = self.entries.get_mut(&id) {
                    entry.last_sent_tick = self.current_tick;
                    entry.accumulated_priority = 0.0;
                }
            }
        }

        selected
    }

    /// Create a filtered world snapshot containing only relevant entities for this client.
    pub fn filter_snapshot(&mut self, snapshot: &WorldSnapshot) -> WorldSnapshot {
        let selected = self.select_entities(snapshot);
        let mut filtered = WorldSnapshot::new(snapshot.id, snapshot.tick, snapshot.timestamp_ms);
        for id in selected {
            if let Some(entity) = snapshot.get_entity(id) {
                filtered.add_entity(entity.clone());
            }
        }
        filtered
    }

    /// Get the current relevancy entry for an entity.
    pub fn get_entry(&self, id: NetEntityId) -> Option<&RelevancyEntry> {
        self.entries.get(&id)
    }

    /// Number of entities currently tracked.
    pub fn tracked_count(&self) -> usize {
        self.entries.len()
    }

    /// Number of entities currently considered relevant.
    pub fn relevant_count(&self) -> usize {
        self.entries.values().filter(|e| e.is_relevant).count()
    }

    /// Reset accumulated priorities for all entries.
    pub fn reset_priorities(&mut self) {
        for entry in self.entries.values_mut() {
            entry.accumulated_priority = 0.0;
        }
    }
}

/// Quantize a float to a fixed number of bits for bandwidth reduction.
pub fn quantize_f32(value: f32, min: f32, max: f32, bits: u32) -> u32 {
    let range = max - min;
    if range <= f32::EPSILON {
        return 0;
    }
    let normalized = ((value - min) / range).clamp(0.0, 1.0);
    let max_val = (1u32 << bits) - 1;
    (normalized * max_val as f32) as u32
}

/// Dequantize back to float.
pub fn dequantize_f32(quantized: u32, min: f32, max: f32, bits: u32) -> f32 {
    let max_val = (1u32 << bits) - 1;
    if max_val == 0 {
        return min;
    }
    let normalized = quantized as f32 / max_val as f32;
    min + normalized * (max - min)
}

/// Pack three quantized position components into bytes.
pub fn pack_position(pos: [f32; 3], bounds_min: [f32; 3], bounds_max: [f32; 3], bits_per_axis: u32) -> Vec<u8> {
    let qx = quantize_f32(pos[0], bounds_min[0], bounds_max[0], bits_per_axis);
    let qy = quantize_f32(pos[1], bounds_min[1], bounds_max[1], bits_per_axis);
    let qz = quantize_f32(pos[2], bounds_min[2], bounds_max[2], bits_per_axis);

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&qx.to_le_bytes());
    bytes.extend_from_slice(&qy.to_le_bytes());
    bytes.extend_from_slice(&qz.to_le_bytes());
    bytes
}

/// Unpack position from bytes.
pub fn unpack_position(data: &[u8], bounds_min: [f32; 3], bounds_max: [f32; 3], bits_per_axis: u32) -> [f32; 3] {
    if data.len() < 12 {
        return [0.0; 3];
    }
    let qx = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let qy = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let qz = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

    [
        dequantize_f32(qx, bounds_min[0], bounds_max[0], bits_per_axis),
        dequantize_f32(qy, bounds_min[1], bounds_max[1], bits_per_axis),
        dequantize_f32(qz, bounds_min[2], bounds_max[2], bits_per_axis),
    ]
}

/// Smallest-three quaternion compression: store only the 3 smallest components
/// and the index of the largest.
pub fn compress_quaternion(q: [f32; 4]) -> [u16; 3] {
    let mut largest_idx = 0;
    let mut largest_val = q[0].abs();
    for i in 1..4 {
        if q[i].abs() > largest_val {
            largest_val = q[i].abs();
            largest_idx = i;
        }
    }

    // Ensure the largest component is positive (negate all if needed)
    let sign = if q[largest_idx] < 0.0 { -1.0 } else { 1.0 };

    let mut small = Vec::with_capacity(3);
    for i in 0..4 {
        if i != largest_idx {
            let val = q[i] * sign;
            // Quaternion components are in [-1/sqrt(2), 1/sqrt(2)] range
            let normalized = (val * std::f32::consts::FRAC_1_SQRT_2.recip() + 1.0) * 0.5;
            let quantized = (normalized.clamp(0.0, 1.0) * 65535.0) as u16;
            small.push(quantized);
        }
    }

    // Pack largest index into top 2 bits of first value
    let idx_bits = (largest_idx as u16) << 14;
    small[0] = (small[0] & 0x3FFF) | idx_bits;

    [small[0], small[1], small[2]]
}

/// Decompress a quaternion from smallest-three representation.
pub fn decompress_quaternion(packed: [u16; 3]) -> [f32; 4] {
    let largest_idx = ((packed[0] >> 14) & 0x03) as usize;
    let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;

    let a_norm = (packed[0] & 0x3FFF) as f32 / 16383.0;
    let b_norm = packed[1] as f32 / 65535.0;
    let c_norm = packed[2] as f32 / 65535.0;

    let a = (a_norm * 2.0 - 1.0) * inv_sqrt2;
    let b = (b_norm * 2.0 - 1.0) * inv_sqrt2;
    let c = (c_norm * 2.0 - 1.0) * inv_sqrt2;

    let sum_sq = a * a + b * b + c * c;
    let largest = (1.0 - sum_sq).max(0.0).sqrt();

    let mut result = [0.0f32; 4];
    let mut small_idx = 0;
    let smalls = [a, b, c];
    for i in 0..4 {
        if i == largest_idx {
            result[i] = largest;
        } else {
            result[i] = smalls[small_idx];
            small_idx += 1;
        }
    }

    // Normalize
    let len = (result[0] * result[0] + result[1] * result[1]
        + result[2] * result[2] + result[3] * result[3]).sqrt();
    if len > f32::EPSILON {
        for v in &mut result {
            *v /= len;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_id() {
        let id = SnapshotId(5);
        assert_eq!(id.next(), SnapshotId(6));
        assert_eq!(id.distance(SnapshotId(10)), 5);
    }

    #[test]
    fn test_component_delta_identical() {
        let c1 = ComponentData::new(1, vec![1, 2, 3, 4]);
        let c2 = ComponentData::new(1, vec![1, 2, 3, 4]);
        assert!(c1.delta_against(&c2).is_none());
    }

    #[test]
    fn test_component_delta_changed() {
        let c1 = ComponentData::new(1, vec![1, 2, 3, 4]);
        let c2 = ComponentData::new(1, vec![1, 2, 5, 4]);
        let delta = c1.delta_against(&c2);
        assert!(delta.is_some());
        let reconstructed = c2.apply_delta(&delta.unwrap());
        assert_eq!(reconstructed.data, c1.data);
    }

    #[test]
    fn test_world_snapshot_delta() {
        let mut ws1 = WorldSnapshot::new(SnapshotId(1), 1, 100);
        let mut e1 = EntitySnapshot::new(NetEntityId(1));
        e1.position = [1.0, 2.0, 3.0];
        e1.add_component(ComponentData::new(1, vec![10, 20]));
        ws1.add_entity(e1);

        let mut ws2 = WorldSnapshot::new(SnapshotId(2), 2, 200);
        let mut e1b = EntitySnapshot::new(NetEntityId(1));
        e1b.position = [4.0, 5.0, 6.0];
        e1b.add_component(ComponentData::new(1, vec![10, 20]));
        ws2.add_entity(e1b);

        let delta = ws2.delta_from(&ws1);
        assert_eq!(delta.entity_deltas.len(), 1);
        assert!(delta.entity_deltas[0].position.is_some());
    }

    #[test]
    fn test_ring_buffer() {
        let mut rb = SnapshotRingBuffer::new(4);
        for i in 0..6 {
            let snap = WorldSnapshot::new(SnapshotId(i), i, i * 100);
            rb.push(snap);
        }
        assert_eq!(rb.len(), 4);
        assert!(rb.get(SnapshotId(0)).is_none());
        assert!(rb.get(SnapshotId(1)).is_none());
        assert!(rb.get(SnapshotId(2)).is_some());
        assert!(rb.get(SnapshotId(5)).is_some());
    }

    #[test]
    fn test_quantize_roundtrip() {
        let val = 3.14;
        let q = quantize_f32(val, 0.0, 10.0, 16);
        let dq = dequantize_f32(q, 0.0, 10.0, 16);
        assert!((dq - val).abs() < 0.001);
    }

    #[test]
    fn test_relevancy_region() {
        let region = RelevancyRegion::new([0.0, 0.0, 0.0], 100.0);
        assert!(region.contains([50.0, 0.0, 0.0]));
        assert!(!region.contains([150.0, 0.0, 0.0]));
        let p = region.priority_at([0.0, 0.0, 0.0]);
        assert!((p - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rle_roundtrip() {
        let data = vec![0, 0, 0, 5, 5, 0, 0, 7];
        let encoded = rle_encode(&data);
        let decoded = rle_decode(&encoded, data.len());
        assert_eq!(decoded, data);
    }
}
