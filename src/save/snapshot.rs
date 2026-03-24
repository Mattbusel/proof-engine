//! World snapshots — serializable representations of the entire game state.
//!
//! A `WorldSnapshot` captures all entity components and named resources at a
//! single point in time. Snapshots are used by the save/load system, the
//! checkpoint system, and the network replication layer.
//!
//! ## Diff / incremental saves
//!
//! `WorldSnapshot::diff` produces a `SnapshotDiff` describing what changed
//! between two snapshots. This is used for incremental save files and for
//! network state synchronisation.

use std::collections::HashMap;

use crate::save::serializer::{DeserializeError, Serialize, SerializedValue};

// ─────────────────────────────────────────────
//  EntitySnapshot
// ─────────────────────────────────────────────

/// Serialized state of a single entity (all its components).
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySnapshot {
    /// Stable entity id (assigned by the ECS / entity manager).
    pub entity_id: u64,
    /// Component type name → serialized value.
    pub components: HashMap<String, SerializedValue>,
}

impl EntitySnapshot {
    /// Create an empty snapshot for `entity_id`.
    pub fn new(entity_id: u64) -> Self {
        Self { entity_id, components: HashMap::new() }
    }

    /// Add or replace a component value.
    pub fn insert_component(&mut self, name: impl Into<String>, value: SerializedValue) {
        self.components.insert(name.into(), value);
    }

    /// Remove a component. Returns the old value if present.
    pub fn remove_component(&mut self, name: &str) -> Option<SerializedValue> {
        self.components.remove(name)
    }

    /// Get a component value by name.
    pub fn get_component(&self, name: &str) -> Option<&SerializedValue> {
        self.components.get(name)
    }

    /// Returns `true` if the entity has a component with this name.
    pub fn has_component(&self, name: &str) -> bool {
        self.components.contains_key(name)
    }

    /// Number of components stored.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Merge components from `other` into `self` (other wins on conflict).
    pub fn merge_from(&mut self, other: &EntitySnapshot) {
        for (k, v) in &other.components {
            self.components.insert(k.clone(), v.clone());
        }
    }

    /// Serialize to a `SerializedValue::Map`.
    pub fn to_serialized(&self) -> SerializedValue {
        let mut map = HashMap::new();
        map.insert("entity_id".into(), SerializedValue::Int(self.entity_id as i64));
        let comp_map: HashMap<String, SerializedValue> = self.components.clone();
        map.insert("components".into(), SerializedValue::Map(comp_map));
        SerializedValue::Map(map)
    }

    /// Deserialize from a `SerializedValue::Map`.
    pub fn from_serialized(v: &SerializedValue) -> Result<Self, DeserializeError> {
        let id = v.get("entity_id")
            .and_then(|v| v.as_int())
            .ok_or_else(|| DeserializeError::MissingKey("entity_id".into()))? as u64;
        let components = v.get("components")
            .and_then(|v| v.as_map())
            .cloned()
            .unwrap_or_default();
        Ok(Self { entity_id: id, components })
    }
}

// ─────────────────────────────────────────────
//  ResourceSnapshot
// ─────────────────────────────────────────────

/// Serialized state of a named global resource (e.g. "PlayerStats", "GameClock").
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceSnapshot {
    /// The resource's type name (used as a key during deserialization).
    pub type_name: String,
    /// The serialized value.
    pub value: SerializedValue,
}

impl ResourceSnapshot {
    pub fn new(type_name: impl Into<String>, value: SerializedValue) -> Self {
        Self { type_name: type_name.into(), value }
    }

    pub fn to_serialized(&self) -> SerializedValue {
        let mut map = HashMap::new();
        map.insert("type_name".into(), SerializedValue::Str(self.type_name.clone()));
        map.insert("value".into(), self.value.clone());
        SerializedValue::Map(map)
    }

    pub fn from_serialized(v: &SerializedValue) -> Result<Self, DeserializeError> {
        let type_name = v.get("type_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DeserializeError::MissingKey("type_name".into()))?
            .to_string();
        let value = v.get("value").cloned().unwrap_or(SerializedValue::Null);
        Ok(Self { type_name, value })
    }
}

// ─────────────────────────────────────────────
//  SnapshotDiff
// ─────────────────────────────────────────────

/// The delta between two `WorldSnapshot`s. Used for incremental saves and
/// for detecting what changed between frames.
#[derive(Debug, Clone, Default)]
pub struct SnapshotDiff {
    /// Entities present in `new` but not in `old`.
    pub added_entities: Vec<EntitySnapshot>,
    /// Entity ids present in `old` but removed in `new`.
    pub removed_entities: Vec<u64>,
    /// Entities present in both but with different component data.
    pub changed_entities: Vec<(u64, EntitySnapshot)>,
    /// Resources added or changed.
    pub changed_resources: Vec<ResourceSnapshot>,
    /// Resource type names that were removed.
    pub removed_resources: Vec<String>,
}

impl SnapshotDiff {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if no changes were detected.
    pub fn is_empty(&self) -> bool {
        self.added_entities.is_empty()
            && self.removed_entities.is_empty()
            && self.changed_entities.is_empty()
            && self.changed_resources.is_empty()
            && self.removed_resources.is_empty()
    }

    /// Total number of entity-level changes.
    pub fn entity_change_count(&self) -> usize {
        self.added_entities.len() + self.removed_entities.len() + self.changed_entities.len()
    }

    /// Total number of resource-level changes.
    pub fn resource_change_count(&self) -> usize {
        self.changed_resources.len() + self.removed_resources.len()
    }
}

// ─────────────────────────────────────────────
//  WorldSnapshot
// ─────────────────────────────────────────────

/// A complete serialized snapshot of the world state at a point in time.
///
/// This is the primary unit passed to the save/load pipeline and the checkpoint
/// manager.
#[derive(Debug, Clone)]
pub struct WorldSnapshot {
    /// All entity snapshots.
    pub entities: Vec<EntitySnapshot>,
    /// All resource snapshots.
    pub resources: Vec<ResourceSnapshot>,
    /// When this snapshot was taken, in seconds since some epoch (game time).
    pub timestamp: f64,
    /// Schema version — used to detect incompatible save files.
    pub version: u32,
    /// Free-form metadata (level name, player name, etc.).
    pub metadata: HashMap<String, String>,
}

impl WorldSnapshot {
    /// Create an empty snapshot.
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            resources: Vec::new(),
            timestamp: 0.0,
            version: 1,
            metadata: HashMap::new(),
        }
    }

    /// Create a snapshot with a specific timestamp.
    pub fn with_timestamp(mut self, ts: f64) -> Self {
        self.timestamp = ts;
        self
    }

    // ── Building ───────────────────────────────────────────────────────────

    /// Add or replace an entity snapshot.
    pub fn add_entity(&mut self, entity_id: u64, components: HashMap<String, SerializedValue>) {
        if let Some(existing) = self.entities.iter_mut().find(|e| e.entity_id == entity_id) {
            existing.components = components;
        } else {
            self.entities.push(EntitySnapshot { entity_id, components });
        }
    }

    /// Add an entity snapshot directly.
    pub fn add_entity_snapshot(&mut self, snapshot: EntitySnapshot) {
        self.add_entity(snapshot.entity_id, snapshot.components);
    }

    /// Add or replace a resource snapshot.
    pub fn add_resource(&mut self, type_name: impl Into<String>, value: SerializedValue) {
        let type_name = type_name.into();
        if let Some(existing) = self.resources.iter_mut().find(|r| r.type_name == type_name) {
            existing.value = value;
        } else {
            self.resources.push(ResourceSnapshot::new(type_name, value));
        }
    }

    /// Set a metadata key.
    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get a metadata value.
    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(String::as_str)
    }

    // ── Queries ────────────────────────────────────────────────────────────

    /// Number of entity snapshots.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Number of resource snapshots.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Find an entity snapshot by id.
    pub fn get_entity(&self, id: u64) -> Option<&EntitySnapshot> {
        self.entities.iter().find(|e| e.entity_id == id)
    }

    /// Find a mutable entity snapshot by id.
    pub fn get_entity_mut(&mut self, id: u64) -> Option<&mut EntitySnapshot> {
        self.entities.iter_mut().find(|e| e.entity_id == id)
    }

    /// Find a resource snapshot by type name.
    pub fn get_resource(&self, type_name: &str) -> Option<&ResourceSnapshot> {
        self.resources.iter().find(|r| r.type_name == type_name)
    }

    /// Remove an entity by id. Returns `true` if it was found.
    pub fn remove_entity(&mut self, id: u64) -> bool {
        let before = self.entities.len();
        self.entities.retain(|e| e.entity_id != id);
        self.entities.len() < before
    }

    // ── Merge ──────────────────────────────────────────────────────────────

    /// Merge `other` into `self`. Entities and resources in `other` override `self`.
    pub fn merge(&mut self, other: &WorldSnapshot) {
        for entity in &other.entities {
            self.add_entity(entity.entity_id, entity.components.clone());
        }
        for resource in &other.resources {
            self.add_resource(resource.type_name.clone(), resource.value.clone());
        }
        for (k, v) in &other.metadata {
            self.metadata.insert(k.clone(), v.clone());
        }
        // Use the newer timestamp
        if other.timestamp > self.timestamp {
            self.timestamp = other.timestamp;
        }
    }

    // ── Diff ───────────────────────────────────────────────────────────────

    /// Compute the changes from `self` (old) to `other` (new).
    pub fn diff(&self, other: &WorldSnapshot) -> SnapshotDiff {
        let mut diff = SnapshotDiff::new();

        // Build entity lookup
        let old_map: HashMap<u64, &EntitySnapshot> =
            self.entities.iter().map(|e| (e.entity_id, e)).collect();
        let new_map: HashMap<u64, &EntitySnapshot> =
            other.entities.iter().map(|e| (e.entity_id, e)).collect();

        // Added and changed
        for (&id, &new_e) in &new_map {
            match old_map.get(&id) {
                None => diff.added_entities.push(new_e.clone()),
                Some(&old_e) => {
                    if old_e != new_e {
                        diff.changed_entities.push((id, new_e.clone()));
                    }
                }
            }
        }

        // Removed entities
        for &id in old_map.keys() {
            if !new_map.contains_key(&id) {
                diff.removed_entities.push(id);
            }
        }

        // Resources
        let old_res: HashMap<&str, &ResourceSnapshot> =
            self.resources.iter().map(|r| (r.type_name.as_str(), r)).collect();
        let new_res: HashMap<&str, &ResourceSnapshot> =
            other.resources.iter().map(|r| (r.type_name.as_str(), r)).collect();

        for (&name, &new_r) in &new_res {
            match old_res.get(name) {
                None => diff.changed_resources.push(new_r.clone()),
                Some(&old_r) => {
                    if old_r != new_r {
                        diff.changed_resources.push(new_r.clone());
                    }
                }
            }
        }

        for &name in old_res.keys() {
            if !new_res.contains_key(name) {
                diff.removed_resources.push(name.to_string());
            }
        }

        diff
    }

    // ── Apply diff ─────────────────────────────────────────────────────────

    /// Apply a `SnapshotDiff` to produce an updated snapshot.
    pub fn apply_diff(&mut self, diff: &SnapshotDiff) {
        // Remove entities
        for &id in &diff.removed_entities {
            self.remove_entity(id);
        }
        // Add new entities
        for entity in &diff.added_entities {
            self.add_entity_snapshot(entity.clone());
        }
        // Apply changes
        for (_, entity) in &diff.changed_entities {
            self.add_entity_snapshot(entity.clone());
        }
        // Resources
        for &ref name in &diff.removed_resources {
            self.resources.retain(|r| &r.type_name != name);
        }
        for resource in &diff.changed_resources {
            self.add_resource(resource.type_name.clone(), resource.value.clone());
        }
    }
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
//  SnapshotSerializer
// ─────────────────────────────────────────────

/// Converts `WorldSnapshot` to/from raw bytes using JSON encoding.
pub struct SnapshotSerializer;

impl SnapshotSerializer {
    /// Serialize a `WorldSnapshot` to a JSON byte vector.
    pub fn to_bytes(snapshot: &WorldSnapshot) -> Vec<u8> {
        let sv = Self::snapshot_to_sv(snapshot);
        sv.to_json_string().into_bytes()
    }

    /// Deserialize a `WorldSnapshot` from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<WorldSnapshot, DeserializeError> {
        let s = std::str::from_utf8(bytes)
            .map_err(|e| DeserializeError::ParseError(e.to_string()))?;
        let sv = SerializedValue::from_json_str(s)?;
        Self::snapshot_from_sv(&sv)
    }

    fn snapshot_to_sv(snapshot: &WorldSnapshot) -> SerializedValue {
        let mut map = HashMap::new();
        map.insert("version".into(), SerializedValue::Int(snapshot.version as i64));
        map.insert("timestamp".into(), SerializedValue::Float(snapshot.timestamp));

        // Entities
        let entities: Vec<SerializedValue> =
            snapshot.entities.iter().map(|e| e.to_serialized()).collect();
        map.insert("entities".into(), SerializedValue::List(entities));

        // Resources
        let resources: Vec<SerializedValue> =
            snapshot.resources.iter().map(|r| r.to_serialized()).collect();
        map.insert("resources".into(), SerializedValue::List(resources));

        // Metadata
        let meta: HashMap<String, SerializedValue> = snapshot
            .metadata
            .iter()
            .map(|(k, v)| (k.clone(), SerializedValue::Str(v.clone())))
            .collect();
        map.insert("metadata".into(), SerializedValue::Map(meta));

        SerializedValue::Map(map)
    }

    fn snapshot_from_sv(sv: &SerializedValue) -> Result<WorldSnapshot, DeserializeError> {
        let version = sv.get("version")
            .and_then(|v| v.as_int())
            .unwrap_or(1) as u32;
        let timestamp = sv.get("timestamp")
            .and_then(|v| v.as_float())
            .unwrap_or(0.0);

        let entities = sv.get("entities")
            .and_then(|v| v.as_list())
            .unwrap_or(&[])
            .iter()
            .map(EntitySnapshot::from_serialized)
            .collect::<Result<Vec<_>, _>>()?;

        let resources = sv.get("resources")
            .and_then(|v| v.as_list())
            .unwrap_or(&[])
            .iter()
            .map(ResourceSnapshot::from_serialized)
            .collect::<Result<Vec<_>, _>>()?;

        let metadata = sv.get("metadata")
            .and_then(|v| v.as_map())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        Ok(WorldSnapshot { entities, resources, timestamp, version, metadata })
    }
}

// ─────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity(id: u64, hp: i64) -> EntitySnapshot {
        let mut e = EntitySnapshot::new(id);
        e.insert_component("health", SerializedValue::Int(hp));
        e
    }

    #[test]
    fn snapshot_add_entity() {
        let mut snap = WorldSnapshot::new();
        snap.add_entity(1, {
            let mut m = HashMap::new();
            m.insert("x".into(), SerializedValue::Float(10.0));
            m
        });
        assert_eq!(snap.entity_count(), 1);
        assert!(snap.get_entity(1).is_some());
    }

    #[test]
    fn snapshot_replace_entity() {
        let mut snap = WorldSnapshot::new();
        snap.add_entity(1, {
            let mut m = HashMap::new(); m.insert("hp".into(), SerializedValue::Int(100)); m
        });
        snap.add_entity(1, {
            let mut m = HashMap::new(); m.insert("hp".into(), SerializedValue::Int(50)); m
        });
        assert_eq!(snap.entity_count(), 1);
        assert_eq!(
            snap.get_entity(1).unwrap().get_component("hp"),
            Some(&SerializedValue::Int(50))
        );
    }

    #[test]
    fn snapshot_remove_entity() {
        let mut snap = WorldSnapshot::new();
        snap.add_entity(42, HashMap::new());
        assert!(snap.remove_entity(42));
        assert!(!snap.remove_entity(42));
    }

    #[test]
    fn snapshot_diff_added_removed() {
        let mut old = WorldSnapshot::new();
        old.add_entity_snapshot(make_entity(1, 100));
        old.add_entity_snapshot(make_entity(2, 50));

        let mut new = WorldSnapshot::new();
        new.add_entity_snapshot(make_entity(1, 80)); // changed
        new.add_entity_snapshot(make_entity(3, 60)); // added

        let diff = old.diff(&new);
        assert_eq!(diff.added_entities.len(), 1);
        assert_eq!(diff.added_entities[0].entity_id, 3);
        assert_eq!(diff.removed_entities, vec![2]);
        assert_eq!(diff.changed_entities.len(), 1);
        assert_eq!(diff.changed_entities[0].0, 1);
    }

    #[test]
    fn snapshot_diff_no_changes() {
        let mut snap = WorldSnapshot::new();
        snap.add_entity_snapshot(make_entity(1, 100));
        let diff = snap.diff(&snap.clone());
        assert!(diff.is_empty());
    }

    #[test]
    fn snapshot_merge() {
        let mut base = WorldSnapshot::new();
        base.add_entity_snapshot(make_entity(1, 100));

        let mut patch = WorldSnapshot::new();
        patch.add_entity_snapshot(make_entity(2, 200));
        patch.timestamp = 99.0;

        base.merge(&patch);
        assert_eq!(base.entity_count(), 2);
        assert_eq!(base.timestamp, 99.0);
    }

    #[test]
    fn snapshot_serializer_roundtrip() {
        let mut snap = WorldSnapshot::new();
        snap.timestamp = 42.0;
        snap.add_entity_snapshot(make_entity(7, 123));
        snap.add_resource("score", SerializedValue::Int(9999));
        snap.set_meta("level", "dungeon_1");

        let bytes = SnapshotSerializer::to_bytes(&snap);
        let restored = SnapshotSerializer::from_bytes(&bytes).unwrap();

        assert_eq!(restored.timestamp, 42.0);
        assert_eq!(restored.entity_count(), 1);
        assert_eq!(restored.resource_count(), 1);
        assert_eq!(restored.get_meta("level"), Some("dungeon_1"));
        assert_eq!(
            restored.get_entity(7).unwrap().get_component("health"),
            Some(&SerializedValue::Int(123))
        );
    }

    #[test]
    fn resource_snapshot_roundtrip() {
        let r = ResourceSnapshot::new("timer", SerializedValue::Float(3.14));
        let sv = r.to_serialized();
        let r2 = ResourceSnapshot::from_serialized(&sv).unwrap();
        assert_eq!(r.type_name, r2.type_name);
    }

    #[test]
    fn entity_snapshot_merge_from() {
        let mut a = make_entity(1, 100);
        let mut b = EntitySnapshot::new(1);
        b.insert_component("mana", SerializedValue::Int(50));
        a.merge_from(&b);
        assert!(a.has_component("health"));
        assert!(a.has_component("mana"));
        assert_eq!(a.component_count(), 2);
    }
}
