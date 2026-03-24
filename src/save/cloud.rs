//! Cloud save synchronisation: upload/download queuing, conflict resolution,
//! offline buffering, encryption, and rotating local backups.
//!
//! No real network I/O is performed — all "remote" state is simulated in
//! memory so the module compiles and tests without external dependencies.

use std::collections::{HashMap, VecDeque};

// ─────────────────────────────────────────────────────────────────────────────
//  CloudProvider
// ─────────────────────────────────────────────────────────────────────────────

/// Which cloud backend to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudProvider {
    None,
    SteamCloud,
    EpicOnline,
    CustomServer(String),
}

impl CloudProvider {
    pub fn is_enabled(&self) -> bool {
        !matches!(self, CloudProvider::None)
    }

    pub fn display_name(&self) -> String {
        match self {
            CloudProvider::None              => "None".into(),
            CloudProvider::SteamCloud        => "Steam Cloud".into(),
            CloudProvider::EpicOnline        => "Epic Online".into(),
            CloudProvider::CustomServer(url) => format!("Custom ({})", url),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  CloudSaveEntry
// ─────────────────────────────────────────────────────────────────────────────

/// A single save slot entry stored in the cloud.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudSaveEntry {
    pub slot_id: u32,
    pub user_id: String,
    pub save_data: Vec<u8>,
    pub timestamp: u64,
    pub checksum: u32,
    pub size_bytes: usize,
    pub conflict_resolved: bool,
}

impl CloudSaveEntry {
    pub fn new(slot_id: u32, user_id: impl Into<String>, save_data: Vec<u8>, timestamp: u64) -> Self {
        let checksum = simple_checksum(&save_data);
        let size_bytes = save_data.len();
        Self { slot_id, user_id: user_id.into(), save_data, timestamp, checksum, size_bytes, conflict_resolved: false }
    }

    pub fn verify_checksum(&self) -> bool {
        simple_checksum(&self.save_data) == self.checksum
    }
}

fn simple_checksum(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &b in data {
        crc = crc.wrapping_add(b as u32).rotate_left(3);
    }
    crc
}

// ─────────────────────────────────────────────────────────────────────────────
//  ConflictResolution
// ─────────────────────────────────────────────────────────────────────────────

/// How to resolve a conflict between a local and remote save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    LocalWins,
    RemoteWins,
    MergeByTimestamp,
    MergeByVersion,
    UserChoice,
}

// ─────────────────────────────────────────────────────────────────────────────
//  CloudSyncState
// ─────────────────────────────────────────────────────────────────────────────

/// Current synchronisation state of the cloud client.
#[derive(Debug, Clone)]
pub enum CloudSyncState {
    Idle,
    Uploading(f32),
    Downloading(f32),
    Conflict { local: CloudSaveEntry, remote: CloudSaveEntry },
    SyncComplete,
    Error(String),
}

// ─────────────────────────────────────────────────────────────────────────────
//  CloudMetadata / CloudSaveStats
// ─────────────────────────────────────────────────────────────────────────────

/// Aggregate metadata about the cloud storage account.
#[derive(Debug, Clone, Default)]
pub struct CloudMetadata {
    pub last_sync_time: u64,
    pub total_slots: u32,
    pub used_bytes: u64,
    pub quota_bytes: u64,
}

impl CloudMetadata {
    pub fn usage_fraction(&self) -> f32 {
        if self.quota_bytes == 0 { return 0.0; }
        (self.used_bytes as f32) / (self.quota_bytes as f32)
    }

    pub fn has_space(&self, needed: u64) -> bool {
        self.used_bytes + needed <= self.quota_bytes
    }
}

/// Cumulative statistics for a session.
#[derive(Debug, Clone, Default)]
pub struct CloudSaveStats {
    pub uploads: u64,
    pub downloads: u64,
    pub conflicts_resolved: u64,
    pub bytes_transferred: u64,
}

impl CloudSaveStats {
    pub fn record_upload(&mut self, bytes: usize) {
        self.uploads += 1;
        self.bytes_transferred += bytes as u64;
    }
    pub fn record_download(&mut self, bytes: usize) {
        self.downloads += 1;
        self.bytes_transferred += bytes as u64;
    }
    pub fn record_conflict_resolved(&mut self) {
        self.conflicts_resolved += 1;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  OfflineQueue
// ─────────────────────────────────────────────────────────────────────────────

/// Pending cloud operation recorded while offline.
#[derive(Debug, Clone)]
pub enum PendingOp {
    Upload { slot_id: u32, data: Vec<u8>, timestamp: u64 },
    Download { slot_id: u32 },
}

/// Stores pending cloud operations when offline.  Max 100 entries; oldest is
/// evicted when full.
#[derive(Debug, Default)]
pub struct OfflineQueue {
    ops: VecDeque<PendingOp>,
}

impl OfflineQueue {
    const MAX_OPS: usize = 100;

    pub fn new() -> Self {
        Self { ops: VecDeque::new() }
    }

    pub fn push(&mut self, op: PendingOp) {
        if self.ops.len() >= Self::MAX_OPS {
            self.ops.pop_front(); // evict oldest
        }
        self.ops.push_back(op);
    }

    pub fn drain(&mut self) -> Vec<PendingOp> {
        self.ops.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.ops.len() >= Self::MAX_OPS
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  SaveEncryption
// ─────────────────────────────────────────────────────────────────────────────

/// XOR-cipher with key derived from the user ID.
///
/// NOT cryptographically secure — this is a structural demonstration only.
pub struct SaveEncryption;

impl SaveEncryption {
    /// Derive a repeating key from the user ID.
    fn derive_key(user_id: &str) -> Vec<u8> {
        let base = user_id.as_bytes();
        if base.is_empty() {
            return vec![0xAB];
        }
        // Mix bytes together deterministically
        let mut key = base.to_vec();
        for i in 1..32 {
            let prev = key[i - 1];
            let next = base[i % base.len()];
            key.push(prev.wrapping_add(next).wrapping_mul(0x45).wrapping_add(0x12));
        }
        key
    }

    /// Encrypt `data` using a key derived from `user_id`.
    pub fn encrypt(data: &[u8], user_id: &str) -> Vec<u8> {
        let key = Self::derive_key(user_id);
        data.iter()
            .enumerate()
            .map(|(i, &b)| b ^ key[i % key.len()])
            .collect()
    }

    /// Decrypt `data` using a key derived from `user_id`.
    pub fn decrypt(data: &[u8], user_id: &str) -> Result<Vec<u8>, String> {
        // XOR is self-inverse
        Ok(Self::encrypt(data, user_id))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  BackupManager
// ─────────────────────────────────────────────────────────────────────────────

/// Entry in the backup list for a slot.
#[derive(Debug, Clone)]
pub struct BackupEntry {
    pub backup_idx: usize,
    pub timestamp: u64,
    pub size_bytes: usize,
    pub checksum: u32,
}

/// Rotating local backups — up to 5 per slot.
#[derive(Debug, Default)]
pub struct BackupManager {
    /// slot_id → ordered list of backups (oldest first)
    backups: HashMap<u32, Vec<(u64, Vec<u8>)>>,
}

impl BackupManager {
    const MAX_BACKUPS: usize = 5;

    pub fn new() -> Self {
        Self { backups: HashMap::new() }
    }

    /// Create a backup of `data` for the given slot.
    pub fn backup(&mut self, slot_id: u32, data: Vec<u8>, timestamp: u64) {
        let slot_backups = self.backups.entry(slot_id).or_default();
        if slot_backups.len() >= Self::MAX_BACKUPS {
            slot_backups.remove(0); // evict oldest
        }
        slot_backups.push((timestamp, data));
    }

    /// Restore a backup for a slot by index (0 = oldest).
    pub fn restore_backup(&self, slot_id: u32, backup_idx: usize) -> Option<Vec<u8>> {
        let slot_backups = self.backups.get(&slot_id)?;
        slot_backups.get(backup_idx).map(|(_, data)| data.clone())
    }

    /// List all backups for a slot.
    pub fn list_backups(&self, slot_id: u32) -> Vec<BackupEntry> {
        match self.backups.get(&slot_id) {
            None => Vec::new(),
            Some(backups) => backups
                .iter()
                .enumerate()
                .map(|(i, (ts, data))| BackupEntry {
                    backup_idx: i,
                    timestamp: *ts,
                    size_bytes: data.len(),
                    checksum: simple_checksum(data),
                })
                .collect(),
        }
    }

    /// Number of backups for a slot.
    pub fn backup_count(&self, slot_id: u32) -> usize {
        self.backups.get(&slot_id).map_or(0, |v| v.len())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  CloudSaveClient
// ─────────────────────────────────────────────────────────────────────────────

/// Manages upload/download queue, conflict detection, retry with exponential
/// backoff (3 attempts), and a concurrent upload limit of 2.
pub struct CloudSaveClient {
    pub provider: CloudProvider,
    pub user_id: String,
    pub state: CloudSyncState,
    pub stats: CloudSaveStats,
    pub metadata: CloudMetadata,
    pub conflict_resolution: ConflictResolution,

    /// Simulated remote storage: slot_id → entry
    remote_store: HashMap<u32, CloudSaveEntry>,
    /// Local working copies: slot_id → data
    local_store: HashMap<u32, Vec<u8>>,

    offline_queue: OfflineQueue,
    backup_manager: BackupManager,
    is_online: bool,
}

impl CloudSaveClient {
    const MAX_RETRIES: u32 = 3;
    const MAX_CONCURRENT_UPLOADS: usize = 2;

    pub fn new(provider: CloudProvider, user_id: impl Into<String>) -> Self {
        Self {
            provider,
            user_id: user_id.into(),
            state: CloudSyncState::Idle,
            stats: CloudSaveStats::default(),
            metadata: CloudMetadata { quota_bytes: 100 * 1024 * 1024, ..Default::default() },
            conflict_resolution: ConflictResolution::MergeByTimestamp,
            remote_store: HashMap::new(),
            local_store: HashMap::new(),
            offline_queue: OfflineQueue::new(),
            backup_manager: BackupManager::new(),
            is_online: true,
        }
    }

    pub fn set_online(&mut self, online: bool) {
        self.is_online = online;
        if online {
            self.drain_offline_queue();
        }
    }

    /// Upload a slot to the (simulated) remote store.
    pub fn upload_slot(&mut self, slot_id: u32, data: Vec<u8>, timestamp: u64) -> Result<(), String> {
        if !self.is_online {
            self.offline_queue.push(PendingOp::Upload { slot_id, data, timestamp });
            return Err("offline — operation queued".into());
        }

        self.backup_manager.backup(slot_id, data.clone(), timestamp);

        let mut last_err = String::new();
        for attempt in 0..Self::MAX_RETRIES {
            match self.do_upload(slot_id, data.clone(), timestamp) {
                Ok(()) => {
                    self.stats.record_upload(data.len());
                    self.state = CloudSyncState::Uploading(1.0);
                    return Ok(());
                }
                Err(e) => {
                    last_err = e;
                    let _backoff_ms = 100u64 * (1 << attempt);
                    // In real code we would sleep here; in tests we skip
                }
            }
        }
        self.state = CloudSyncState::Error(last_err.clone());
        Err(last_err)
    }

    fn do_upload(&mut self, slot_id: u32, data: Vec<u8>, timestamp: u64) -> Result<(), String> {
        let entry = CloudSaveEntry::new(slot_id, &self.user_id, data, timestamp);
        self.metadata.used_bytes = self.metadata.used_bytes
            .saturating_add(entry.size_bytes as u64);
        self.remote_store.insert(slot_id, entry);
        Ok(())
    }

    /// Download a slot from the (simulated) remote store.
    pub fn download_slot(&mut self, slot_id: u32) -> Result<Vec<u8>, String> {
        if !self.is_online {
            self.offline_queue.push(PendingOp::Download { slot_id });
            return Err("offline — operation queued".into());
        }

        let entry = self.remote_store.get(&slot_id)
            .ok_or_else(|| format!("slot {slot_id} not found in remote store"))?
            .clone();

        if !entry.verify_checksum() {
            return Err(format!("slot {slot_id} checksum mismatch on download"));
        }

        self.stats.record_download(entry.size_bytes);
        self.state = CloudSyncState::Downloading(1.0);
        self.local_store.insert(slot_id, entry.save_data.clone());
        Ok(entry.save_data)
    }

    /// Detect and resolve conflicts for a slot.
    ///
    /// A conflict exists when local and remote checksums differ.
    pub fn resolve_conflict(
        &mut self,
        slot_id: u32,
        local_data: Vec<u8>,
        local_timestamp: u64,
        resolution: ConflictResolution,
    ) -> Result<Vec<u8>, String> {
        let remote = self.remote_store.get(&slot_id).cloned();
        let local_entry = CloudSaveEntry::new(slot_id, &self.user_id, local_data, local_timestamp);

        match remote {
            None => {
                // No remote — local wins by default
                Ok(local_entry.save_data)
            }
            Some(remote_entry) => {
                if local_entry.checksum == remote_entry.checksum {
                    return Ok(local_entry.save_data); // no conflict
                }
                self.stats.record_conflict_resolved();
                let winner = match resolution {
                    ConflictResolution::LocalWins => local_entry.save_data,
                    ConflictResolution::RemoteWins => remote_entry.save_data,
                    ConflictResolution::MergeByTimestamp => {
                        if local_entry.timestamp >= remote_entry.timestamp {
                            local_entry.save_data
                        } else {
                            remote_entry.save_data
                        }
                    }
                    ConflictResolution::MergeByVersion => {
                        // Pick larger data as proxy for "more data = later version"
                        if local_entry.size_bytes >= remote_entry.size_bytes {
                            local_entry.save_data
                        } else {
                            remote_entry.save_data
                        }
                    }
                    ConflictResolution::UserChoice => {
                        // Default to local when no UI callback provided
                        local_entry.save_data
                    }
                };
                Ok(winner)
            }
        }
    }

    /// Sync all locally known slots to the remote.
    pub fn sync_all(&mut self, timestamp: u64) -> Result<(), String> {
        let slots: Vec<(u32, Vec<u8>)> = self.local_store
            .iter()
            .map(|(&id, data)| (id, data.clone()))
            .collect();

        let mut errors = Vec::new();
        let mut in_flight = 0usize;

        for (slot_id, data) in slots {
            if in_flight >= Self::MAX_CONCURRENT_UPLOADS {
                in_flight = 0; // simulate completing a batch
            }
            if let Err(e) = self.upload_slot(slot_id, data, timestamp) {
                errors.push(e);
            } else {
                in_flight += 1;
            }
        }

        if errors.is_empty() {
            self.metadata.last_sync_time = timestamp;
            self.state = CloudSyncState::SyncComplete;
            Ok(())
        } else {
            let msg = errors.join("; ");
            self.state = CloudSyncState::Error(msg.clone());
            Err(msg)
        }
    }

    /// Store a local copy without uploading.
    pub fn set_local(&mut self, slot_id: u32, data: Vec<u8>) {
        self.local_store.insert(slot_id, data);
    }

    pub fn offline_queue(&self) -> &OfflineQueue {
        &self.offline_queue
    }

    pub fn backup_manager(&self) -> &BackupManager {
        &self.backup_manager
    }

    pub fn backup_manager_mut(&mut self) -> &mut BackupManager {
        &mut self.backup_manager
    }

    fn drain_offline_queue(&mut self) {
        let ops = self.offline_queue.drain();
        for op in ops {
            match op {
                PendingOp::Upload { slot_id, data, timestamp } => {
                    let _ = self.do_upload(slot_id, data, timestamp);
                }
                PendingOp::Download { slot_id } => {
                    // Re-queue if remote has it; ignore errors during drain
                    let _ = self.remote_store.get(&slot_id).cloned();
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client() -> CloudSaveClient {
        CloudSaveClient::new(CloudProvider::SteamCloud, "user_123")
    }

    #[test]
    fn test_upload_and_download_roundtrip() {
        let mut client = make_client();
        let data = b"save data content".to_vec();
        client.upload_slot(0, data.clone(), 1000).unwrap();
        let downloaded = client.download_slot(0).unwrap();
        assert_eq!(downloaded, data);
    }

    #[test]
    fn test_upload_creates_backup() {
        let mut client = make_client();
        let data = b"backup test".to_vec();
        client.upload_slot(1, data.clone(), 2000).unwrap();
        assert_eq!(client.backup_manager().backup_count(1), 1);
    }

    #[test]
    fn test_offline_queue_enqueue() {
        let mut client = make_client();
        client.set_online(false);
        let result = client.upload_slot(0, b"data".to_vec(), 100);
        assert!(result.is_err());
        assert_eq!(client.offline_queue().len(), 1);
    }

    #[test]
    fn test_offline_queue_max_100() {
        let mut queue = OfflineQueue::new();
        for i in 0..105u64 {
            queue.push(PendingOp::Upload { slot_id: i as u32, data: vec![], timestamp: i });
        }
        assert_eq!(queue.len(), 100);
    }

    #[test]
    fn test_offline_queue_drains_on_reconnect() {
        let mut client = make_client();
        client.set_online(false);
        let _ = client.upload_slot(5, b"queued".to_vec(), 500);
        assert_eq!(client.offline_queue().len(), 1);
        client.set_online(true);
        assert_eq!(client.offline_queue().len(), 0);
    }

    #[test]
    fn test_conflict_resolution_local_wins() {
        let mut client = make_client();
        let remote_data = b"remote version".to_vec();
        client.upload_slot(0, remote_data.clone(), 100).unwrap();
        let local_data = b"local version".to_vec();
        let result = client.resolve_conflict(0, local_data.clone(), 200, ConflictResolution::LocalWins).unwrap();
        assert_eq!(result, local_data);
    }

    #[test]
    fn test_conflict_resolution_remote_wins() {
        let mut client = make_client();
        let remote_data = b"remote version".to_vec();
        client.upload_slot(0, remote_data.clone(), 100).unwrap();
        let local_data = b"local version".to_vec();
        let result = client.resolve_conflict(0, local_data, 200, ConflictResolution::RemoteWins).unwrap();
        assert_eq!(result, remote_data);
    }

    #[test]
    fn test_conflict_resolution_merge_by_timestamp() {
        let mut client = make_client();
        client.upload_slot(0, b"old remote".to_vec(), 100).unwrap();
        let local_data = b"newer local".to_vec();
        let result = client.resolve_conflict(0, local_data.clone(), 999, ConflictResolution::MergeByTimestamp).unwrap();
        assert_eq!(result, local_data);
    }

    #[test]
    fn test_encryption_roundtrip() {
        let data = b"sensitive save data 123".to_vec();
        let encrypted = SaveEncryption::encrypt(&data, "user_abc");
        assert_ne!(encrypted, data);
        let decrypted = SaveEncryption::decrypt(&encrypted, "user_abc").unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encryption_different_users_differ() {
        let data = b"hello world".to_vec();
        let enc1 = SaveEncryption::encrypt(&data, "user_a");
        let enc2 = SaveEncryption::encrypt(&data, "user_b");
        assert_ne!(enc1, enc2);
    }

    #[test]
    fn test_backup_manager_max_5() {
        let mut bm = BackupManager::new();
        for i in 0..7u64 {
            bm.backup(0, vec![i as u8], i);
        }
        assert_eq!(bm.backup_count(0), 5);
    }

    #[test]
    fn test_backup_manager_restore() {
        let mut bm = BackupManager::new();
        bm.backup(0, b"first".to_vec(), 1);
        bm.backup(0, b"second".to_vec(), 2);
        let restored = bm.restore_backup(0, 0).unwrap();
        assert_eq!(restored, b"first");
    }

    #[test]
    fn test_backup_manager_list() {
        let mut bm = BackupManager::new();
        bm.backup(0, b"v1".to_vec(), 100);
        bm.backup(0, b"v2".to_vec(), 200);
        let list = bm.list_backups(0);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].backup_idx, 0);
        assert_eq!(list[1].backup_idx, 1);
    }

    #[test]
    fn test_cloud_metadata_usage() {
        let meta = CloudMetadata {
            last_sync_time: 0,
            total_slots: 10,
            used_bytes: 50,
            quota_bytes: 100,
        };
        assert!((meta.usage_fraction() - 0.5).abs() < 1e-6);
        assert!(meta.has_space(40));
        assert!(!meta.has_space(60));
    }

    #[test]
    fn test_sync_all() {
        let mut client = make_client();
        client.set_local(0, b"slot0".to_vec());
        client.set_local(1, b"slot1".to_vec());
        client.sync_all(9999).unwrap();
        assert!(matches!(client.state, CloudSyncState::SyncComplete));
    }
}
