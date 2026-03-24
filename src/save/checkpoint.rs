//! Checkpoint system — spatial save points and respawn management.
//!
//! A `Checkpoint` is a named point in the world that stores a full
//! `WorldSnapshot` and a 2D position. The `CheckpointManager` maintains an
//! ordered list of checkpoints, evicting the oldest ones when the cap is
//! exceeded.
//!
//! `RespawnSystem` sits on top of the manager and tracks the "last activated"
//! checkpoint so the game can quickly restore state on player death.

use std::collections::HashMap;

use glam::Vec2;

use crate::save::serializer::{DeserializeError, Serialize, Deserialize, SerializedValue};
use crate::save::snapshot::{SnapshotSerializer, WorldSnapshot};

// ─────────────────────────────────────────────
//  Checkpoint
// ─────────────────────────────────────────────

/// A single checkpoint — a world position plus a saved snapshot.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    /// Unique id assigned by `CheckpointManager`.
    pub id: u64,
    /// Human-readable label (e.g. "dungeon_entrance", "boss_arena_pre").
    pub name: String,
    /// The world-space position of this checkpoint.
    pub position: Vec2,
    /// The world state at the moment this checkpoint was created.
    pub snapshot: WorldSnapshot,
    /// When this checkpoint was created (game time in seconds).
    pub created_at: f64,
    /// Optional metadata tags.
    pub tags: HashMap<String, String>,
}

impl Checkpoint {
    pub fn new(
        id: u64,
        name: impl Into<String>,
        position: Vec2,
        snapshot: WorldSnapshot,
        created_at: f64,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            position,
            snapshot,
            created_at,
            tags: HashMap::new(),
        }
    }

    /// Set a metadata tag on this checkpoint.
    pub fn set_tag(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.tags.insert(key.into(), value.into());
    }

    /// Get a metadata tag.
    pub fn get_tag(&self, key: &str) -> Option<&str> {
        self.tags.get(key).map(String::as_str)
    }

    /// Euclidean distance from this checkpoint to `pos`.
    pub fn distance_to(&self, pos: Vec2) -> f32 {
        (self.position - pos).length()
    }

    // ── Serialization ──────────────────────────────────────────────────────

    pub fn to_serialized(&self) -> SerializedValue {
        let mut map = HashMap::new();
        map.insert("id".into(), SerializedValue::Int(self.id as i64));
        map.insert("name".into(), SerializedValue::Str(self.name.clone()));
        map.insert("position".into(), self.position.serialize());
        map.insert("created_at".into(), SerializedValue::Float(self.created_at));

        // Embed snapshot as a nested JSON string to avoid encoding issues
        let snap_bytes = SnapshotSerializer::to_bytes(&self.snapshot);
        let snap_str = String::from_utf8(snap_bytes).unwrap_or_default();
        map.insert("snapshot".into(), SerializedValue::Str(snap_str));

        let tags: HashMap<String, SerializedValue> = self.tags.iter()
            .map(|(k, v)| (k.clone(), SerializedValue::Str(v.clone())))
            .collect();
        map.insert("tags".into(), SerializedValue::Map(tags));

        SerializedValue::Map(map)
    }

    pub fn from_serialized(sv: &SerializedValue) -> Result<Self, DeserializeError> {
        let id = sv.get("id")
            .and_then(|v| v.as_int())
            .ok_or_else(|| DeserializeError::MissingKey("id".into()))? as u64;
        let name = sv.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed")
            .to_string();
        let position = sv.get("position")
            .map(Vec2::deserialize)
            .transpose()?
            .unwrap_or(Vec2::ZERO);
        let created_at = sv.get("created_at")
            .and_then(|v| v.as_float())
            .unwrap_or(0.0);

        let snapshot_str = sv.get("snapshot")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        let snapshot = SnapshotSerializer::from_bytes(snapshot_str.as_bytes())
            .unwrap_or_else(|_| WorldSnapshot::new());

        let tags: HashMap<String, String> = sv.get("tags")
            .and_then(|v| v.as_map())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Checkpoint { id, name, position, snapshot, created_at, tags })
    }
}

// ─────────────────────────────────────────────
//  CheckpointManager
// ─────────────────────────────────────────────

/// Manages a bounded list of checkpoints.
///
/// When the checkpoint count exceeds `max_checkpoints`, the oldest (by
/// `created_at`) is evicted automatically.
pub struct CheckpointManager {
    checkpoints: Vec<Checkpoint>,
    pub max_checkpoints: usize,
    id_counter: u64,
}

impl CheckpointManager {
    /// Create a new manager with a capacity cap.
    pub fn new(max: usize) -> Self {
        Self {
            checkpoints: Vec::new(),
            max_checkpoints: max.max(1),
            id_counter: 0,
        }
    }

    // ── Creation ───────────────────────────────────────────────────────────

    /// Create a new checkpoint and return its id.
    ///
    /// If the checkpoint list is at capacity, the oldest checkpoint is removed.
    pub fn create(
        &mut self,
        name: impl Into<String>,
        pos: Vec2,
        snapshot: WorldSnapshot,
    ) -> u64 {
        self.create_with_time(name, pos, snapshot, 0.0)
    }

    /// Like `create` but with an explicit game-time timestamp.
    pub fn create_with_time(
        &mut self,
        name: impl Into<String>,
        pos: Vec2,
        snapshot: WorldSnapshot,
        created_at: f64,
    ) -> u64 {
        let id = self.next_id();
        let checkpoint = Checkpoint::new(id, name, pos, snapshot, created_at);
        self.checkpoints.push(checkpoint);
        self.evict_if_over_cap();
        id
    }

    fn next_id(&mut self) -> u64 {
        let id = self.id_counter;
        self.id_counter += 1;
        id
    }

    fn evict_if_over_cap(&mut self) {
        while self.checkpoints.len() > self.max_checkpoints {
            // Remove the checkpoint with the smallest created_at
            let oldest_idx = self
                .checkpoints
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.created_at.partial_cmp(&b.created_at).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.checkpoints.remove(oldest_idx);
        }
    }

    // ── Retrieval ──────────────────────────────────────────────────────────

    /// Get a checkpoint by id.
    pub fn get(&self, id: u64) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| c.id == id)
    }

    /// Get a mutable checkpoint by id.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Checkpoint> {
        self.checkpoints.iter_mut().find(|c| c.id == id)
    }

    /// Get the checkpoint closest to `pos` (Euclidean distance).
    pub fn get_nearest(&self, pos: Vec2) -> Option<&Checkpoint> {
        self.checkpoints
            .iter()
            .min_by(|a, b| {
                a.distance_to(pos)
                    .partial_cmp(&b.distance_to(pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get the most recently created checkpoint (highest `created_at`).
    pub fn get_most_recent(&self) -> Option<&Checkpoint> {
        self.checkpoints
            .iter()
            .max_by(|a, b| a.created_at.partial_cmp(&b.created_at).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get all checkpoints within `radius` of `pos`, sorted by distance.
    pub fn get_within_radius(&self, pos: Vec2, radius: f32) -> Vec<&Checkpoint> {
        let mut nearby: Vec<&Checkpoint> = self
            .checkpoints
            .iter()
            .filter(|c| c.distance_to(pos) <= radius)
            .collect();
        nearby.sort_by(|a, b| {
            a.distance_to(pos)
                .partial_cmp(&b.distance_to(pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        nearby
    }

    /// Iterate over all checkpoints.
    pub fn list(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    // ── Removal ────────────────────────────────────────────────────────────

    /// Remove the checkpoint with the given id. Returns `true` if found.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.checkpoints.len();
        self.checkpoints.retain(|c| c.id != id);
        self.checkpoints.len() < before
    }

    /// Remove all checkpoints.
    pub fn clear(&mut self) {
        self.checkpoints.clear();
    }

    // ── Info ───────────────────────────────────────────────────────────────

    /// Number of checkpoints.
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }

    pub fn is_at_cap(&self) -> bool {
        self.checkpoints.len() >= self.max_checkpoints
    }

    // ── Serialization ──────────────────────────────────────────────────────

    /// Serialize all checkpoints to a JSON byte vector.
    pub fn serialize_all(&self) -> Vec<u8> {
        let list: Vec<SerializedValue> = self.checkpoints.iter()
            .map(|c| c.to_serialized())
            .collect();
        let sv = SerializedValue::List(list);
        sv.to_json_string().into_bytes()
    }

    /// Deserialize checkpoints from a JSON byte vector (as produced by `serialize_all`).
    pub fn deserialize_all(bytes: &[u8]) -> Result<Vec<Checkpoint>, DeserializeError> {
        let s = std::str::from_utf8(bytes)
            .map_err(|e| DeserializeError::ParseError(e.to_string()))?;
        let sv = SerializedValue::from_json_str(s)?;
        sv.as_list()
            .ok_or(DeserializeError::Custom("expected list of checkpoints".into()))?
            .iter()
            .map(Checkpoint::from_serialized)
            .collect()
    }

    /// Restore checkpoints from bytes, replacing any existing ones.
    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<(), DeserializeError> {
        let checkpoints = Self::deserialize_all(bytes)?;
        // Update id counter to avoid collisions
        if let Some(max_id) = checkpoints.iter().map(|c| c.id).max() {
            self.id_counter = max_id + 1;
        }
        self.checkpoints = checkpoints;
        Ok(())
    }
}

// ─────────────────────────────────────────────
//  RespawnSystem
// ─────────────────────────────────────────────

/// Tracks the active checkpoint and handles player respawning.
///
/// Call `update_checkpoint` periodically (e.g. when the player enters a
/// checkpoint trigger zone) to register the nearest checkpoint as active.
/// Call `respawn` on player death to retrieve the saved state.
pub struct RespawnSystem {
    /// The id of the last activated checkpoint, if any.
    pub last_checkpoint: Option<u64>,
    /// How many times the player has respawned.
    pub respawn_count: u32,
    /// The minimum distance from a checkpoint to auto-activate it.
    pub activation_radius: f32,
    /// History of respawn events (checkpoint id + game time).
    respawn_history: Vec<RespawnEvent>,
}

/// A single respawn event recorded by `RespawnSystem`.
#[derive(Debug, Clone)]
pub struct RespawnEvent {
    pub checkpoint_id: u64,
    pub game_time: f64,
    pub respawn_index: u32,
}

impl RespawnSystem {
    /// Create a new respawn system with no active checkpoint.
    pub fn new() -> Self {
        Self {
            last_checkpoint: None,
            respawn_count: 0,
            activation_radius: 2.0,
            respawn_history: Vec::new(),
        }
    }

    pub fn with_activation_radius(mut self, r: f32) -> Self {
        self.activation_radius = r;
        self
    }

    // ── Activation ─────────────────────────────────────────────────────────

    /// Update the active checkpoint based on the player's current position.
    ///
    /// If the player is within `activation_radius` of any checkpoint, the nearest
    /// one becomes the active checkpoint. Returns the id of the newly activated
    /// checkpoint (if one was activated this call) or `None`.
    pub fn update_checkpoint(
        &mut self,
        manager: &CheckpointManager,
        player_pos: Vec2,
    ) -> Option<u64> {
        let nearest = manager.get_nearest(player_pos)?;
        if nearest.distance_to(player_pos) <= self.activation_radius {
            let activated = self.last_checkpoint != Some(nearest.id);
            self.last_checkpoint = Some(nearest.id);
            if activated { Some(nearest.id) } else { None }
        } else {
            None
        }
    }

    /// Manually activate a checkpoint by id.
    pub fn activate(&mut self, checkpoint_id: u64) {
        self.last_checkpoint = Some(checkpoint_id);
    }

    /// Clear the active checkpoint (e.g. at the start of a level).
    pub fn deactivate(&mut self) {
        self.last_checkpoint = None;
    }

    // ── Respawn ────────────────────────────────────────────────────────────

    /// Get the snapshot to restore on respawn.
    ///
    /// Returns `None` if no checkpoint has been activated.
    pub fn respawn<'a>(
        &mut self,
        manager: &'a CheckpointManager,
        game_time: f64,
    ) -> Option<&'a WorldSnapshot> {
        let id = self.last_checkpoint?;
        let checkpoint = manager.get(id)?;
        self.respawn_count += 1;
        self.respawn_history.push(RespawnEvent {
            checkpoint_id: id,
            game_time,
            respawn_index: self.respawn_count,
        });
        Some(&checkpoint.snapshot)
    }

    /// Get the snapshot without incrementing the respawn counter.
    pub fn peek_snapshot<'a>(&self, manager: &'a CheckpointManager) -> Option<&'a WorldSnapshot> {
        let id = self.last_checkpoint?;
        manager.get(id).map(|c| &c.snapshot)
    }

    // ── Info ───────────────────────────────────────────────────────────────

    pub fn has_checkpoint(&self) -> bool {
        self.last_checkpoint.is_some()
    }

    pub fn respawn_history(&self) -> &[RespawnEvent] {
        &self.respawn_history
    }

    pub fn clear_history(&mut self) {
        self.respawn_history.clear();
    }

    /// The position of the active checkpoint, if any.
    pub fn checkpoint_position(&self, manager: &CheckpointManager) -> Option<Vec2> {
        let id = self.last_checkpoint?;
        manager.get(id).map(|c| c.position)
    }
}

impl Default for RespawnSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::serializer::SerializedValue;

    fn make_snap(id: u64) -> WorldSnapshot {
        let mut s = WorldSnapshot::new();
        s.set_meta("source_entity", &id.to_string());
        s
    }

    #[test]
    fn create_and_get() {
        let mut mgr = CheckpointManager::new(10);
        let id = mgr.create("start", Vec2::ZERO, make_snap(1));
        assert!(mgr.get(id).is_some());
        assert_eq!(mgr.get(id).unwrap().name, "start");
    }

    #[test]
    fn eviction_at_cap() {
        let mut mgr = CheckpointManager::new(3);
        let mut ids = vec![];
        for i in 0..5u64 {
            ids.push(mgr.create_with_time(format!("cp{i}"), Vec2::ZERO, make_snap(i), i as f64));
        }
        assert_eq!(mgr.len(), 3);
        // Oldest (id 0, 1) should be gone; newest 3 remain
        assert!(mgr.get(ids[0]).is_none());
        assert!(mgr.get(ids[1]).is_none());
        assert!(mgr.get(ids[4]).is_some());
    }

    #[test]
    fn nearest_checkpoint() {
        let mut mgr = CheckpointManager::new(10);
        mgr.create("a", Vec2::new(0.0, 0.0), make_snap(1));
        mgr.create("b", Vec2::new(100.0, 0.0), make_snap(2));
        let near = mgr.get_nearest(Vec2::new(5.0, 0.0)).unwrap();
        assert_eq!(near.name, "a");
    }

    #[test]
    fn most_recent_checkpoint() {
        let mut mgr = CheckpointManager::new(10);
        mgr.create_with_time("first", Vec2::ZERO, make_snap(1), 1.0);
        mgr.create_with_time("second", Vec2::ZERO, make_snap(2), 5.0);
        mgr.create_with_time("third", Vec2::ZERO, make_snap(3), 3.0);
        assert_eq!(mgr.get_most_recent().unwrap().name, "second");
    }

    #[test]
    fn remove_checkpoint() {
        let mut mgr = CheckpointManager::new(10);
        let id = mgr.create("cp", Vec2::ZERO, make_snap(1));
        assert!(mgr.remove(id));
        assert!(!mgr.remove(id));
        assert!(mgr.is_empty());
    }

    #[test]
    fn serialize_deserialize_all() {
        let mut mgr = CheckpointManager::new(10);
        let mut s = make_snap(1);
        s.timestamp = 42.0;
        mgr.create("alpha", Vec2::new(1.0, 2.0), s);
        mgr.create("beta", Vec2::new(3.0, 4.0), make_snap(2));

        let bytes = mgr.serialize_all();
        let restored = CheckpointManager::deserialize_all(&bytes).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].name, "alpha");
        assert!((restored[0].position.x - 1.0_f32).abs() < 1e-5);
    }

    #[test]
    fn checkpoint_manager_load_from_bytes() {
        let mut mgr = CheckpointManager::new(10);
        mgr.create("cp1", Vec2::new(10.0, 20.0), make_snap(1));
        let bytes = mgr.serialize_all();

        let mut mgr2 = CheckpointManager::new(10);
        mgr2.load_from_bytes(&bytes).unwrap();
        assert_eq!(mgr2.len(), 1);
        assert_eq!(mgr2.list()[0].name, "cp1");
    }

    #[test]
    fn respawn_system_activate_and_respawn() {
        let mut mgr = CheckpointManager::new(10);
        let id = mgr.create("spawn", Vec2::ZERO, make_snap(99));

        let mut respawn = RespawnSystem::new();
        respawn.activate(id);
        assert!(respawn.has_checkpoint());

        let snap = respawn.respawn(&mgr, 0.0).unwrap();
        assert_eq!(snap.get_meta("source_entity"), Some("99"));
        assert_eq!(respawn.respawn_count, 1);
    }

    #[test]
    fn respawn_system_update_by_proximity() {
        let mut mgr = CheckpointManager::new(10);
        mgr.create("near", Vec2::new(1.0, 0.0), make_snap(1));

        let mut respawn = RespawnSystem::new().with_activation_radius(5.0);
        let activated = respawn.update_checkpoint(&mgr, Vec2::new(0.5, 0.0));
        assert!(activated.is_some());
        assert!(respawn.has_checkpoint());
    }

    #[test]
    fn respawn_system_no_checkpoint() {
        let mgr = CheckpointManager::new(10);
        let mut respawn = RespawnSystem::new();
        assert!(respawn.respawn(&mgr, 0.0).is_none());
    }

    #[test]
    fn respawn_history_tracks_events() {
        let mut mgr = CheckpointManager::new(10);
        let id = mgr.create("cp", Vec2::ZERO, make_snap(1));
        let mut respawn = RespawnSystem::new();
        respawn.activate(id);
        respawn.respawn(&mgr, 10.0);
        respawn.respawn(&mgr, 20.0);
        assert_eq!(respawn.respawn_history().len(), 2);
        assert_eq!(respawn.respawn_history()[1].game_time, 20.0);
    }

    #[test]
    fn within_radius() {
        let mut mgr = CheckpointManager::new(10);
        mgr.create("close", Vec2::new(1.0, 0.0), make_snap(1));
        mgr.create("far", Vec2::new(100.0, 0.0), make_snap(2));
        let nearby = mgr.get_within_radius(Vec2::ZERO, 5.0);
        assert_eq!(nearby.len(), 1);
        assert_eq!(nearby[0].name, "close");
    }

    #[test]
    fn checkpoint_tags() {
        let mut mgr = CheckpointManager::new(10);
        let id = mgr.create("tagged", Vec2::ZERO, make_snap(1));
        mgr.get_mut(id).unwrap().set_tag("type", "boss_entrance");
        assert_eq!(mgr.get(id).unwrap().get_tag("type"), Some("boss_entrance"));
    }
}
