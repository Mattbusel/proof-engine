//! Schema migration system for save data.
//!
//! `MigrationRegistry` holds an ordered chain of `MigrationFn` values keyed by
//! source version.  `migrate(data, from, to)` runs the chain to bring save data
//! from any older version up to the current version without data loss.

use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
//  SchemaVersion
// ─────────────────────────────────────────────────────────────────────────────

/// A monotonically increasing version number for the save schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct SchemaVersion(pub u32);

impl SchemaVersion {
    pub const CURRENT: SchemaVersion = SchemaVersion(10);

    pub fn value(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  SaveValue
// ─────────────────────────────────────────────────────────────────────────────

/// A flexible value type for use inside `SaveData`.
#[derive(Debug, Clone, PartialEq)]
pub enum SaveValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<SaveValue>),
    Map(HashMap<String, SaveValue>),
    Bytes(Vec<u8>),
}

impl SaveValue {
    pub fn as_bool(&self) -> Option<bool> {
        if let SaveValue::Bool(b) = self { Some(*b) } else { None }
    }
    pub fn as_int(&self) -> Option<i64> {
        if let SaveValue::Int(i) = self { Some(*i) } else { None }
    }
    pub fn as_float(&self) -> Option<f64> {
        match self {
            SaveValue::Float(f) => Some(*f),
            SaveValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let SaveValue::Str(s) = self { Some(s.as_str()) } else { None }
    }
    pub fn as_list(&self) -> Option<&[SaveValue]> {
        if let SaveValue::List(v) = self { Some(v.as_slice()) } else { None }
    }
    pub fn as_map(&self) -> Option<&HashMap<String, SaveValue>> {
        if let SaveValue::Map(m) = self { Some(m) } else { None }
    }
    pub fn as_map_mut(&mut self) -> Option<&mut HashMap<String, SaveValue>> {
        if let SaveValue::Map(m) = self { Some(m) } else { None }
    }
    pub fn as_bytes(&self) -> Option<&[u8]> {
        if let SaveValue::Bytes(b) = self { Some(b.as_slice()) } else { None }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            SaveValue::Bool(_)  => "Bool",
            SaveValue::Int(_)   => "Int",
            SaveValue::Float(_) => "Float",
            SaveValue::Str(_)   => "Str",
            SaveValue::List(_)  => "List",
            SaveValue::Map(_)   => "Map",
            SaveValue::Bytes(_) => "Bytes",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  SaveData
// ─────────────────────────────────────────────────────────────────────────────

/// Flexible key→value store that migrations read and modify.
#[derive(Debug, Clone, Default)]
pub struct SaveData {
    pub fields: HashMap<String, SaveValue>,
    pub version: SchemaVersion,
}

impl SaveData {
    pub fn new(version: SchemaVersion) -> Self {
        Self { fields: HashMap::new(), version }
    }

    pub fn get(&self, key: &str) -> Option<&SaveValue> {
        self.fields.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut SaveValue> {
        self.fields.get_mut(key)
    }

    pub fn set(&mut self, key: impl Into<String>, value: SaveValue) {
        self.fields.insert(key.into(), value);
    }

    pub fn remove(&mut self, key: &str) -> Option<SaveValue> {
        self.fields.remove(key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    /// Sum all integer and float values for checksum computation.
    pub fn sum_numeric(&self) -> f64 {
        fn recurse(v: &SaveValue) -> f64 {
            match v {
                SaveValue::Int(i)   => *i as f64,
                SaveValue::Float(f) => *f,
                SaveValue::List(l)  => l.iter().map(recurse).sum(),
                SaveValue::Map(m)   => m.values().map(recurse).sum(),
                _ => 0.0,
            }
        }
        self.fields.values().map(recurse).sum()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  MigrationFn
// ─────────────────────────────────────────────────────────────────────────────

/// A function that transforms `SaveData` from version N to N+1.
pub type MigrationFn = fn(data: &mut SaveData) -> Result<(), String>;

// ─────────────────────────────────────────────────────────────────────────────
//  MigrationRegistry
// ─────────────────────────────────────────────────────────────────────────────

/// Holds an ordered chain of migrations keyed by source version.
pub struct MigrationRegistry {
    /// from_version → migration function
    migrations: Vec<(SchemaVersion, MigrationFn)>,
}

impl MigrationRegistry {
    pub fn new() -> Self {
        Self { migrations: Vec::new() }
    }

    /// Register a migration from `from_version` to `from_version + 1`.
    pub fn register(&mut self, from_version: u32, f: MigrationFn) {
        self.migrations.push((SchemaVersion(from_version), f));
        self.migrations.sort_by_key(|(v, _)| *v);
    }

    /// Run the chain of migrations to bring `data` from `from` to `to`.
    pub fn migrate(
        &self,
        data: &mut SaveData,
        from: SchemaVersion,
        to: SchemaVersion,
    ) -> Result<(), String> {
        if from >= to {
            return Ok(());
        }
        let mut current = from;
        for (version, f) in &self.migrations {
            if *version < from || *version >= to {
                continue;
            }
            if *version != current {
                return Err(format!(
                    "missing migration from {current} (next available is {version})"
                ));
            }
            f(data).map_err(|e| format!("migration {version}: {e}"))?;
            current = SchemaVersion(current.0 + 1);
            data.version = current;
        }
        if current != to {
            return Err(format!("migration chain incomplete: reached {current}, needed {to}"));
        }
        Ok(())
    }

    /// Build a registry pre-populated with all 10 built-in migrations (v0→v10).
    pub fn with_builtin_migrations() -> Self {
        let mut reg = Self::new();
        reg.register(0, migrate_v0_to_v1);
        reg.register(1, migrate_v1_to_v2);
        reg.register(2, migrate_v2_to_v3);
        reg.register(3, migrate_v3_to_v4);
        reg.register(4, migrate_v4_to_v5);
        reg.register(5, migrate_v5_to_v6);
        reg.register(6, migrate_v6_to_v7);
        reg.register(7, migrate_v7_to_v8);
        reg.register(8, migrate_v8_to_v9);
        reg.register(9, migrate_v9_to_v10);
        reg
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Concrete migrations
// ─────────────────────────────────────────────────────────────────────────────

/// v0 → v1: Add `created_at` timestamp field (defaults to 0).
fn migrate_v0_to_v1(data: &mut SaveData) -> Result<(), String> {
    if !data.contains("created_at") {
        data.set("created_at", SaveValue::Int(0));
    }
    Ok(())
}

/// v1 → v2: Rename `hp` to `health_points` throughout the top-level fields.
fn migrate_v1_to_v2(data: &mut SaveData) -> Result<(), String> {
    if let Some(val) = data.remove("hp") {
        data.set("health_points", val);
    }
    // Also rename inside any nested maps
    let keys: Vec<String> = data.fields.keys().cloned().collect();
    for key in keys {
        if let Some(SaveValue::Map(ref mut m)) = data.fields.get_mut(&key) {
            if let Some(val) = m.remove("hp") {
                m.insert("health_points".into(), val);
            }
        }
    }
    Ok(())
}

/// v2 → v3: Flatten nested `stats` map — `stats.strength` becomes `stat_strength`, etc.
fn migrate_v2_to_v3(data: &mut SaveData) -> Result<(), String> {
    if let Some(SaveValue::Map(stats_map)) = data.remove("stats") {
        for (k, v) in stats_map {
            data.set(format!("stat_{k}"), v);
        }
    }
    Ok(())
}

/// v3 → v4: Convert `inventory` from a list-of-strings to a list-of-objects
/// `{name: String, quantity: 1, durability: 100}`.
fn migrate_v3_to_v4(data: &mut SaveData) -> Result<(), String> {
    if let Some(SaveValue::List(inv)) = data.remove("inventory") {
        let new_inv: Vec<SaveValue> = inv
            .into_iter()
            .map(|item| {
                let name = match &item {
                    SaveValue::Str(s) => s.clone(),
                    _ => "unknown".into(),
                };
                let mut m = HashMap::new();
                m.insert("name".into(),       SaveValue::Str(name));
                m.insert("quantity".into(),   SaveValue::Int(1));
                m.insert("durability".into(), SaveValue::Int(100));
                SaveValue::Map(m)
            })
            .collect();
        data.set("inventory", SaveValue::List(new_inv));
    }
    Ok(())
}

/// v4 → v5: Add `player_level` defaulting to 1.
fn migrate_v4_to_v5(data: &mut SaveData) -> Result<(), String> {
    if !data.contains("player_level") {
        data.set("player_level", SaveValue::Int(1));
    }
    Ok(())
}

/// v5 → v6: Convert `position` from a `[x, y]` list to `{x, y, z: 0}` map.
fn migrate_v5_to_v6(data: &mut SaveData) -> Result<(), String> {
    if let Some(SaveValue::List(pos)) = data.remove("position") {
        let x = pos.get(0).and_then(|v| v.as_float()).unwrap_or(0.0);
        let y = pos.get(1).and_then(|v| v.as_float()).unwrap_or(0.0);
        let mut m = HashMap::new();
        m.insert("x".into(), SaveValue::Float(x));
        m.insert("y".into(), SaveValue::Float(y));
        m.insert("z".into(), SaveValue::Float(0.0));
        data.set("position", SaveValue::Map(m));
    }
    Ok(())
}

/// v6 → v7: Add `difficulty` defaulting to "normal".
fn migrate_v6_to_v7(data: &mut SaveData) -> Result<(), String> {
    if !data.contains("difficulty") {
        data.set("difficulty", SaveValue::Str("normal".into()));
    }
    Ok(())
}

/// v7 → v8: Encode `player_name` as UTF-8 bytes (simulates an encoding migration).
fn migrate_v7_to_v8(data: &mut SaveData) -> Result<(), String> {
    if let Some(SaveValue::Str(name)) = data.remove("player_name") {
        data.set("player_name", SaveValue::Bytes(name.into_bytes()));
    }
    Ok(())
}

/// v8 → v9: Split `audio_volume` (0.0–1.0) into `music_volume` and `sfx_volume`.
fn migrate_v8_to_v9(data: &mut SaveData) -> Result<(), String> {
    let vol = if let Some(v) = data.remove("audio_volume") {
        v.as_float().unwrap_or(1.0)
    } else {
        1.0
    };
    if !data.contains("music_volume") {
        data.set("music_volume", SaveValue::Float(vol));
    }
    if !data.contains("sfx_volume") {
        data.set("sfx_volume", SaveValue::Float(vol));
    }
    Ok(())
}

/// v9 → v10: Compute a `checksum` field as the integer sum of all numeric values.
fn migrate_v9_to_v10(data: &mut SaveData) -> Result<(), String> {
    let sum = data.sum_numeric();
    data.set("checksum", SaveValue::Int(sum as i64));
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> MigrationRegistry {
        MigrationRegistry::with_builtin_migrations()
    }

    fn v0_data() -> SaveData {
        let mut d = SaveData::new(SchemaVersion(0));
        d.set("hp", SaveValue::Int(100));
        d.set("audio_volume", SaveValue::Float(0.8));
        d.set("player_name", SaveValue::Str("Hero".into()));
        let inv = SaveValue::List(vec![
            SaveValue::Str("sword".into()),
            SaveValue::Str("shield".into()),
        ]);
        d.set("inventory", inv);
        let mut stats = HashMap::new();
        stats.insert("strength".into(), SaveValue::Int(10));
        stats.insert("agility".into(),  SaveValue::Int(8));
        d.set("stats", SaveValue::Map(stats));
        d.set("position", SaveValue::List(vec![SaveValue::Float(1.5), SaveValue::Float(2.5)]));
        d
    }

    #[test]
    fn test_v0_to_v1_adds_created_at() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(1)).unwrap();
        assert!(data.contains("created_at"));
    }

    #[test]
    fn test_v1_to_v2_renames_hp() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(2)).unwrap();
        assert!(!data.contains("hp"));
        assert!(data.contains("health_points"));
    }

    #[test]
    fn test_v2_to_v3_flattens_stats() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(3)).unwrap();
        assert!(!data.contains("stats"));
        assert!(data.contains("stat_strength"));
        assert!(data.contains("stat_agility"));
    }

    #[test]
    fn test_v3_to_v4_converts_inventory() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(4)).unwrap();
        let inv = data.get("inventory").unwrap().as_list().unwrap();
        assert_eq!(inv.len(), 2);
        let item = inv[0].as_map().unwrap();
        assert!(item.contains_key("name"));
        assert!(item.contains_key("quantity"));
        assert!(item.contains_key("durability"));
    }

    #[test]
    fn test_v4_to_v5_adds_player_level() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(5)).unwrap();
        assert_eq!(data.get("player_level").unwrap().as_int(), Some(1));
    }

    #[test]
    fn test_v5_to_v6_converts_position() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(6)).unwrap();
        let pos = data.get("position").unwrap().as_map().unwrap();
        assert!(pos.contains_key("x"));
        assert!(pos.contains_key("y"));
        assert!(pos.contains_key("z"));
        assert_eq!(pos["z"].as_float(), Some(0.0));
    }

    #[test]
    fn test_v6_to_v7_adds_difficulty() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(7)).unwrap();
        assert_eq!(data.get("difficulty").unwrap().as_str(), Some("normal"));
    }

    #[test]
    fn test_v7_to_v8_encodes_name_as_bytes() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(8)).unwrap();
        let name_val = data.get("player_name").unwrap();
        assert!(matches!(name_val, SaveValue::Bytes(_)));
        assert_eq!(name_val.as_bytes(), Some(b"Hero" as &[u8]));
    }

    #[test]
    fn test_v8_to_v9_splits_audio_volume() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(9)).unwrap();
        assert!(!data.contains("audio_volume"));
        assert!(data.contains("music_volume"));
        assert!(data.contains("sfx_volume"));
    }

    #[test]
    fn test_v9_to_v10_adds_checksum() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(10)).unwrap();
        assert!(data.contains("checksum"));
    }

    #[test]
    fn test_full_migration_chain() {
        let reg = make_registry();
        let mut data = v0_data();
        reg.migrate(&mut data, SchemaVersion(0), SchemaVersion(10)).unwrap();
        assert_eq!(data.version, SchemaVersion(10));
    }

    #[test]
    fn test_migration_already_at_version() {
        let reg = make_registry();
        let mut data = SaveData::new(SchemaVersion(5));
        let result = reg.migrate(&mut data, SchemaVersion(5), SchemaVersion(5));
        assert!(result.is_ok());
    }

    #[test]
    fn test_schema_version_ordering() {
        assert!(SchemaVersion(0) < SchemaVersion(1));
        assert!(SchemaVersion(10) == SchemaVersion::CURRENT);
    }

    #[test]
    fn test_save_value_type_accessors() {
        let v = SaveValue::Int(42);
        assert_eq!(v.as_int(), Some(42));
        assert_eq!(v.as_bool(), None);
        assert_eq!(v.as_float(), Some(42.0));

        let s = SaveValue::Str("hello".into());
        assert_eq!(s.as_str(), Some("hello"));

        let b = SaveValue::Bytes(vec![1, 2, 3]);
        assert_eq!(b.as_bytes(), Some(&[1u8, 2, 3] as &[u8]));
    }
}
