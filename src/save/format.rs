//! Save file format, slot management, and the `SaveManager`.
//!
//! ## File layout
//!
//! ```text
//! [magic: 4 bytes "SAVE"]
//! [header length: u32 little-endian]
//! [header JSON bytes]
//! [snapshot length: u32 little-endian]
//! [snapshot JSON bytes]
//! [checksum: u32 little-endian] — CRC-32-style over header+snapshot bytes
//! ```
//!
//! The format is intentionally simple and human-inspectable (the JSON sections
//! are UTF-8 text). No compression or encryption is applied by default.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::save::serializer::{DeserializeError, SerializedValue};
use crate::save::snapshot::{SnapshotSerializer, WorldSnapshot};

// ─────────────────────────────────────────────
//  SaveError
// ─────────────────────────────────────────────

/// Errors that can occur during save file operations.
#[derive(Debug, Clone)]
pub enum SaveError {
    /// A filesystem error.
    Io(String),
    /// The file is not a valid save file (bad magic, truncated, etc.).
    Corrupt,
    /// The save file was written by an incompatible engine version.
    VersionMismatch { expected: u32, found: u32 },
    /// The stored checksum does not match the computed checksum.
    ChecksumMismatch,
    /// No save file exists at the requested slot.
    SlotEmpty(u8),
    /// A serialization/deserialization problem.
    Serialize(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(s) => write!(f, "I/O error: {s}"),
            SaveError::Corrupt => write!(f, "save file is corrupt"),
            SaveError::VersionMismatch { expected, found } => {
                write!(f, "version mismatch: expected {expected}, found {found}")
            }
            SaveError::ChecksumMismatch => write!(f, "checksum mismatch — file may be corrupt"),
            SaveError::SlotEmpty(n) => write!(f, "save slot {n} is empty"),
            SaveError::Serialize(s) => write!(f, "serialization error: {s}"),
        }
    }
}

impl std::error::Error for SaveError {}

impl From<DeserializeError> for SaveError {
    fn from(e: DeserializeError) -> Self {
        SaveError::Serialize(e.to_string())
    }
}

// ─────────────────────────────────────────────
//  Magic / version constants
// ─────────────────────────────────────────────

/// The four magic bytes at the start of every save file.
pub const SAVE_MAGIC: [u8; 4] = *b"SAVE";

/// The current save file format version.
pub const CURRENT_FORMAT_VERSION: u32 = 1;

// ─────────────────────────────────────────────
//  Simple CRC-32 (no external crate)
// ─────────────────────────────────────────────

/// Compute a simple 32-bit checksum over `data`.
///
/// This is a polynomial-based CRC-32 (IEEE) computed without lookup tables so
/// no crate dependency is required.
pub fn compute_checksum(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ─────────────────────────────────────────────
//  SaveHeader
// ─────────────────────────────────────────────

/// Metadata stored at the top of a save file.
///
/// The header is always present and can be read without loading the full snapshot,
/// which is useful for displaying slot previews in a save/load menu.
#[derive(Debug, Clone)]
pub struct SaveHeader {
    /// Magic bytes — always `SAVE_MAGIC`.
    pub magic: [u8; 4],
    /// Format version (not the game version).
    pub version: u32,
    /// Human-readable game version string, e.g. `"1.2.3"`.
    pub game_version: String,
    /// Unix timestamp (seconds) when this save was written.
    pub timestamp: u64,
    /// Player name at save time.
    pub player_name: String,
    /// Cumulative play time in seconds.
    pub play_time_seconds: f64,
    /// Arbitrary key-value metadata (level name, difficulty, etc.).
    pub metadata: HashMap<String, String>,
}

impl SaveHeader {
    pub fn new() -> Self {
        Self {
            magic: SAVE_MAGIC,
            version: CURRENT_FORMAT_VERSION,
            game_version: "0.1.0".to_string(),
            timestamp: current_unix_ts(),
            player_name: "Player".to_string(),
            play_time_seconds: 0.0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_player(mut self, name: impl Into<String>) -> Self {
        self.player_name = name.into();
        self
    }

    pub fn with_play_time(mut self, secs: f64) -> Self {
        self.play_time_seconds = secs;
        self
    }

    pub fn with_game_version(mut self, ver: impl Into<String>) -> Self {
        self.game_version = ver.into();
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(String::as_str)
    }

    fn to_serialized(&self) -> SerializedValue {
        let mut map = HashMap::new();
        map.insert("magic".into(), SerializedValue::Str(
            std::str::from_utf8(&self.magic).unwrap_or("SAVE").to_string()
        ));
        map.insert("version".into(), SerializedValue::Int(self.version as i64));
        map.insert("game_version".into(), SerializedValue::Str(self.game_version.clone()));
        map.insert("timestamp".into(), SerializedValue::Int(self.timestamp as i64));
        map.insert("player_name".into(), SerializedValue::Str(self.player_name.clone()));
        map.insert("play_time_seconds".into(), SerializedValue::Float(self.play_time_seconds));
        let meta: HashMap<String, SerializedValue> = self.metadata.iter()
            .map(|(k, v)| (k.clone(), SerializedValue::Str(v.clone())))
            .collect();
        map.insert("metadata".into(), SerializedValue::Map(meta));
        SerializedValue::Map(map)
    }

    fn from_serialized(sv: &SerializedValue) -> Result<Self, SaveError> {
        let version = sv.get("version")
            .and_then(|v| v.as_int())
            .unwrap_or(CURRENT_FORMAT_VERSION as i64) as u32;
        let game_version = sv.get("game_version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let timestamp = sv.get("timestamp")
            .and_then(|v| v.as_int())
            .unwrap_or(0) as u64;
        let player_name = sv.get("player_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Player")
            .to_string();
        let play_time_seconds = sv.get("play_time_seconds")
            .and_then(|v| v.as_float())
            .unwrap_or(0.0);
        let metadata: HashMap<String, String> = sv.get("metadata")
            .and_then(|v| v.as_map())
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        Ok(SaveHeader {
            magic: SAVE_MAGIC,
            version,
            game_version,
            timestamp,
            player_name,
            play_time_seconds,
            metadata,
        })
    }
}

impl Default for SaveHeader {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
//  SaveFile
// ─────────────────────────────────────────────

/// A complete in-memory representation of a save file.
///
/// Call `write_to_bytes()` / `write_to_file()` to persist it.
pub struct SaveFile {
    pub header: SaveHeader,
    pub snapshot: WorldSnapshot,
    /// CRC-32 checksum of header + snapshot bytes. Computed lazily on write.
    pub checksum: u32,
}

impl SaveFile {
    // ── Construction ───────────────────────────────────────────────────────

    pub fn new(snapshot: WorldSnapshot) -> Self {
        Self {
            header: SaveHeader::new(),
            snapshot,
            checksum: 0,
        }
    }

    pub fn with_header(mut self, header: SaveHeader) -> Self {
        self.header = header;
        self
    }

    // ── Serialization ──────────────────────────────────────────────────────

    /// Serialize to a byte vector using the binary format described in the module doc.
    pub fn write_to_bytes(&mut self) -> Vec<u8> {
        let header_bytes = self.header.to_serialized().to_json_string().into_bytes();
        let snapshot_bytes = SnapshotSerializer::to_bytes(&self.snapshot);

        // Compute checksum over both sections
        let mut checksum_input = Vec::with_capacity(header_bytes.len() + snapshot_bytes.len());
        checksum_input.extend_from_slice(&header_bytes);
        checksum_input.extend_from_slice(&snapshot_bytes);
        self.checksum = compute_checksum(&checksum_input);

        let mut out = Vec::with_capacity(
            4 + 4 + header_bytes.len() + 4 + snapshot_bytes.len() + 4,
        );

        // Magic
        out.extend_from_slice(&SAVE_MAGIC);
        // Header length + header
        out.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&header_bytes);
        // Snapshot length + snapshot
        out.extend_from_slice(&(snapshot_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&snapshot_bytes);
        // Checksum
        out.extend_from_slice(&self.checksum.to_le_bytes());

        out
    }

    /// Write the save file to disk at `path`.
    pub fn write_to_file(&mut self, path: &str) -> Result<(), SaveError> {
        let bytes = self.write_to_bytes();
        // Create parent directories
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| SaveError::Io(format!("create_dir_all: {e}")))?;
            }
        }
        std::fs::write(path, &bytes)
            .map_err(|e| SaveError::Io(format!("write '{path}': {e}")))?;
        Ok(())
    }

    // ── Deserialization ────────────────────────────────────────────────────

    /// Parse a `SaveFile` from raw bytes.
    pub fn read_from_bytes(bytes: &[u8]) -> Result<SaveFile, SaveError> {
        if bytes.len() < 4 {
            return Err(SaveError::Corrupt);
        }

        // Magic
        if &bytes[0..4] != b"SAVE" {
            return Err(SaveError::Corrupt);
        }
        let mut cursor = 4usize;

        // Header
        let header_len = read_u32(bytes, cursor)? as usize;
        cursor += 4;
        if cursor + header_len > bytes.len() {
            return Err(SaveError::Corrupt);
        }
        let header_bytes = &bytes[cursor..cursor + header_len];
        cursor += header_len;

        // Snapshot
        let snap_len = read_u32(bytes, cursor)? as usize;
        cursor += 4;
        if cursor + snap_len > bytes.len() {
            return Err(SaveError::Corrupt);
        }
        let snap_bytes = &bytes[cursor..cursor + snap_len];
        cursor += snap_len;

        // Checksum
        if cursor + 4 > bytes.len() {
            return Err(SaveError::Corrupt);
        }
        let stored_checksum = read_u32(bytes, cursor)?;

        // Verify checksum
        let mut checksum_input = Vec::with_capacity(header_len + snap_len);
        checksum_input.extend_from_slice(header_bytes);
        checksum_input.extend_from_slice(snap_bytes);
        let computed = compute_checksum(&checksum_input);
        if computed != stored_checksum {
            return Err(SaveError::ChecksumMismatch);
        }

        // Parse header
        let header_str = std::str::from_utf8(header_bytes).map_err(|_| SaveError::Corrupt)?;
        let header_sv = SerializedValue::from_json_str(header_str)
            .map_err(|e| SaveError::Serialize(e.to_string()))?;
        let header = SaveHeader::from_serialized(&header_sv)?;

        // Version check
        if header.version != CURRENT_FORMAT_VERSION {
            return Err(SaveError::VersionMismatch {
                expected: CURRENT_FORMAT_VERSION,
                found: header.version,
            });
        }

        // Parse snapshot
        let snapshot = SnapshotSerializer::from_bytes(snap_bytes)
            .map_err(|e| SaveError::Serialize(e.to_string()))?;

        Ok(SaveFile { header, snapshot, checksum: stored_checksum })
    }

    /// Read a `SaveFile` from a file on disk.
    pub fn read_from_file(path: &str) -> Result<SaveFile, SaveError> {
        let bytes = std::fs::read(path)
            .map_err(|e| SaveError::Io(format!("read '{path}': {e}")))?;
        SaveFile::read_from_bytes(&bytes)
    }

    // ── Validation ─────────────────────────────────────────────────────────

    /// Re-compute the checksum and compare it with the stored value.
    pub fn verify_checksum(&self) -> bool {
        let header_bytes = self.header.to_serialized().to_json_string().into_bytes();
        let snapshot_bytes = SnapshotSerializer::to_bytes(&self.snapshot);
        let mut input = Vec::with_capacity(header_bytes.len() + snapshot_bytes.len());
        input.extend_from_slice(&header_bytes);
        input.extend_from_slice(&snapshot_bytes);
        compute_checksum(&input) == self.checksum
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, SaveError> {
    if offset + 4 > bytes.len() {
        return Err(SaveError::Corrupt);
    }
    Ok(u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}

// ─────────────────────────────────────────────
//  SaveSlot
// ─────────────────────────────────────────────

/// A single numbered save slot. `header` is `None` if the slot is empty.
#[derive(Debug, Clone)]
pub struct SaveSlot {
    /// The slot number (0-based).
    pub slot_number: u8,
    /// Absolute path to the save file on disk (may not exist if slot is empty).
    pub path: String,
    /// The header parsed from the file, or `None` if the slot is empty.
    pub header: Option<SaveHeader>,
}

impl SaveSlot {
    pub fn new(slot_number: u8, path: String) -> Self {
        Self { slot_number, path, header: None }
    }

    pub fn is_empty(&self) -> bool {
        self.header.is_none()
    }

    pub fn player_name(&self) -> Option<&str> {
        self.header.as_ref().map(|h| h.player_name.as_str())
    }

    pub fn play_time(&self) -> Option<f64> {
        self.header.as_ref().map(|h| h.play_time_seconds)
    }

    pub fn timestamp(&self) -> Option<u64> {
        self.header.as_ref().map(|h| h.timestamp)
    }
}

// ─────────────────────────────────────────────
//  SaveManager
// ─────────────────────────────────────────────

/// Manages multiple save slots under a single directory.
///
/// Slots are numbered from 0. Slot 255 is reserved for the auto-save.
pub struct SaveManager {
    /// Directory where save files are stored.
    pub save_dir: String,
    /// Cached slot metadata.
    slots: Vec<SaveSlot>,
}

const AUTO_SAVE_SLOT: u8 = 255;
const AUTO_SAVE_FILENAME: &str = "autosave.sav";

impl SaveManager {
    /// Create a new `SaveManager` pointing at `save_dir`.
    ///
    /// The directory is created lazily when a save is written.
    pub fn new(save_dir: impl Into<String>) -> Self {
        Self {
            save_dir: save_dir.into(),
            slots: Vec::new(),
        }
    }

    // ── Slot path helpers ──────────────────────────────────────────────────

    fn slot_path(&self, slot: u8) -> String {
        if slot == AUTO_SAVE_SLOT {
            format!("{}/{}", self.save_dir, AUTO_SAVE_FILENAME)
        } else {
            format!("{}/save_{:02}.sav", self.save_dir, slot)
        }
    }

    // ── List ───────────────────────────────────────────────────────────────

    /// Return up to `max` slot descriptors (0 through `max-1`), reading headers
    /// from disk where save files exist.
    pub fn list_slots(&self, max: usize) -> Vec<SaveSlot> {
        let mut result = Vec::with_capacity(max);
        for i in 0..max.min(255) {
            let slot_num = i as u8;
            let path = self.slot_path(slot_num);
            let header = Self::try_read_header(&path);
            result.push(SaveSlot { slot_number: slot_num, path, header });
        }
        result
    }

    fn try_read_header(path: &str) -> Option<SaveHeader> {
        let bytes = std::fs::read(path).ok()?;
        let file = SaveFile::read_from_bytes(&bytes).ok()?;
        Some(file.header)
    }

    // ── Save ───────────────────────────────────────────────────────────────

    /// Write a save to slot `slot`.
    pub fn save_to_slot(
        &self,
        slot: u8,
        snapshot: WorldSnapshot,
        header: SaveHeader,
    ) -> Result<(), SaveError> {
        let path = self.slot_path(slot);
        let mut file = SaveFile::new(snapshot).with_header(header);
        file.write_to_file(&path)
    }

    // ── Load ───────────────────────────────────────────────────────────────

    /// Load the full save file from slot `slot`.
    pub fn load_slot(&self, slot: u8) -> Result<SaveFile, SaveError> {
        let path = self.slot_path(slot);
        if !Path::new(&path).exists() {
            return Err(SaveError::SlotEmpty(slot));
        }
        SaveFile::read_from_file(&path)
    }

    // ── Delete ─────────────────────────────────────────────────────────────

    /// Delete the save file in slot `slot`.
    pub fn delete_slot(&self, slot: u8) -> Result<(), SaveError> {
        let path = self.slot_path(slot);
        if !Path::new(&path).exists() {
            return Ok(()); // idempotent
        }
        std::fs::remove_file(&path)
            .map_err(|e| SaveError::Io(format!("delete slot {slot}: {e}")))
    }

    // ── Auto-save ──────────────────────────────────────────────────────────

    /// Write the auto-save slot.
    pub fn auto_save(&self, snapshot: WorldSnapshot) -> Result<(), SaveError> {
        let mut header = SaveHeader::new();
        header.set_meta("slot_type", "autosave");
        self.save_to_slot(AUTO_SAVE_SLOT, snapshot, header)
    }

    /// Returns `true` if an auto-save file exists.
    pub fn has_auto_save(&self) -> bool {
        Path::new(&self.slot_path(AUTO_SAVE_SLOT)).exists()
    }

    /// Load the auto-save.
    pub fn load_auto_save(&self) -> Result<SaveFile, SaveError> {
        self.load_slot(AUTO_SAVE_SLOT)
    }

    // ── Most recent ────────────────────────────────────────────────────────

    /// Return the slot number of the most recently written save (by Unix timestamp).
    ///
    /// Scans up to the first 32 slots plus the auto-save slot.
    pub fn most_recent_slot(&self) -> Option<u8> {
        let mut best: Option<(u8, u64)> = None;

        for slot_num in 0u8..32 {
            let path = self.slot_path(slot_num);
            if let Some(header) = Self::try_read_header(&path) {
                if best.map_or(true, |(_, ts)| header.timestamp > ts) {
                    best = Some((slot_num, header.timestamp));
                }
            }
        }

        // Also check auto-save
        let auto_path = self.slot_path(AUTO_SAVE_SLOT);
        if let Some(header) = Self::try_read_header(&auto_path) {
            if best.map_or(true, |(_, ts)| header.timestamp > ts) {
                best = Some((AUTO_SAVE_SLOT, header.timestamp));
            }
        }

        best.map(|(slot, _)| slot)
    }

    // ── Utilities ──────────────────────────────────────────────────────────

    /// Ensure the save directory exists.
    pub fn ensure_dir(&self) -> Result<(), SaveError> {
        std::fs::create_dir_all(&self.save_dir)
            .map_err(|e| SaveError::Io(format!("create save dir '{}': {e}", self.save_dir)))
    }

    /// Count non-empty slots (0..32).
    pub fn used_slot_count(&self) -> usize {
        (0u8..32)
            .filter(|&slot| Path::new(&self.slot_path(slot)).exists())
            .count()
    }
}

// ─────────────────────────────────────────────
//  Time helper
// ─────────────────────────────────────────────

fn current_unix_ts() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ─────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::serializer::SerializedValue;

    fn tmp_dir() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let p = std::env::temp_dir().join(format!("proof_save_{t}"));
        std::fs::create_dir_all(&p).unwrap();
        p.to_string_lossy().to_string()
    }

    fn cleanup(dir: &str) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn checksum_deterministic() {
        let data = b"hello world";
        assert_eq!(compute_checksum(data), compute_checksum(data));
        assert_ne!(compute_checksum(data), compute_checksum(b"hello worlD"));
    }

    #[test]
    fn save_header_defaults() {
        let h = SaveHeader::new();
        assert_eq!(h.magic, SAVE_MAGIC);
        assert_eq!(h.version, CURRENT_FORMAT_VERSION);
    }

    #[test]
    fn save_file_roundtrip_bytes() {
        let mut snap = WorldSnapshot::new();
        snap.timestamp = 77.0;
        snap.add_entity(1, {
            let mut m = std::collections::HashMap::new();
            m.insert("hp".into(), SerializedValue::Int(100));
            m
        });

        let mut file = SaveFile::new(snap);
        file.header.player_name = "Tester".into();

        let bytes = file.write_to_bytes();
        let restored = SaveFile::read_from_bytes(&bytes).unwrap();
        assert_eq!(restored.header.player_name, "Tester");
        assert_eq!(restored.snapshot.timestamp, 77.0);
        assert_eq!(restored.snapshot.entity_count(), 1);
    }

    #[test]
    fn save_file_verify_checksum() {
        let mut file = SaveFile::new(WorldSnapshot::new());
        file.write_to_bytes();
        assert!(file.verify_checksum());
    }

    #[test]
    fn save_file_checksum_mismatch() {
        let mut file = SaveFile::new(WorldSnapshot::new());
        let mut bytes = file.write_to_bytes();
        // Flip the last byte of the snapshot section (before checksum)
        let len = bytes.len();
        bytes[len - 5] ^= 0xFF;
        let result = SaveFile::read_from_bytes(&bytes);
        assert!(matches!(result, Err(SaveError::ChecksumMismatch)));
    }

    #[test]
    fn save_file_bad_magic() {
        let bytes = b"NOPE...";
        let result = SaveFile::read_from_bytes(bytes);
        assert!(matches!(result, Err(SaveError::Corrupt)));
    }

    #[test]
    fn save_manager_write_and_load() {
        let dir = tmp_dir();
        let mgr = SaveManager::new(&dir);

        let mut snap = WorldSnapshot::new();
        snap.set_meta("level", "forest");
        let header = SaveHeader::new().with_player("Alice");
        mgr.save_to_slot(0, snap, header).unwrap();

        let file = mgr.load_slot(0).unwrap();
        assert_eq!(file.header.player_name, "Alice");
        assert_eq!(file.snapshot.get_meta("level"), Some("forest"));

        cleanup(&dir);
    }

    #[test]
    fn save_manager_empty_slot_error() {
        let dir = tmp_dir();
        let mgr = SaveManager::new(&dir);
        let result = mgr.load_slot(7);
        assert!(matches!(result, Err(SaveError::SlotEmpty(7))));
        cleanup(&dir);
    }

    #[test]
    fn save_manager_auto_save() {
        let dir = tmp_dir();
        let mgr = SaveManager::new(&dir);
        assert!(!mgr.has_auto_save());
        mgr.auto_save(WorldSnapshot::new()).unwrap();
        assert!(mgr.has_auto_save());
        cleanup(&dir);
    }

    #[test]
    fn save_manager_delete_slot() {
        let dir = tmp_dir();
        let mgr = SaveManager::new(&dir);
        mgr.save_to_slot(1, WorldSnapshot::new(), SaveHeader::new()).unwrap();
        assert!(mgr.load_slot(1).is_ok());
        mgr.delete_slot(1).unwrap();
        assert!(matches!(mgr.load_slot(1), Err(SaveError::SlotEmpty(1))));
        cleanup(&dir);
    }

    #[test]
    fn save_manager_most_recent() {
        let dir = tmp_dir();
        let mgr = SaveManager::new(&dir);
        mgr.save_to_slot(0, WorldSnapshot::new(), SaveHeader::new()).unwrap();
        // Sleep a tiny bit to get a different timestamp
        std::thread::sleep(std::time::Duration::from_millis(1100));
        mgr.save_to_slot(1, WorldSnapshot::new(), SaveHeader::new()).unwrap();
        let recent = mgr.most_recent_slot();
        // Slot 1 was written last — should be the most recent
        assert!(recent.is_some());
        cleanup(&dir);
    }

    #[test]
    fn save_slot_is_empty() {
        let slot = SaveSlot::new(0, "path/save_00.sav".into());
        assert!(slot.is_empty());
    }

    #[test]
    fn save_header_metadata() {
        let mut h = SaveHeader::new();
        h.set_meta("difficulty", "hard");
        assert_eq!(h.get_meta("difficulty"), Some("hard"));
    }
}
