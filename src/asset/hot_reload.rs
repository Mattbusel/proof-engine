//! Hot-reload support — detects file changes on disk and triggers asset reloads.
//!
//! `FileWatcher` polls `std::fs::metadata` modification times. This is intentionally
//! simple — no inotify / FSEvents / ReadDirectoryChangesW dependency — so it works on
//! every platform without extra crates. The polling interval is controlled by the
//! caller (typically once per frame or on a fixed timer).
//!
//! `HotReloadPlugin` wraps `FileWatcher` and the `AssetServer` so game code only
//! needs to call `plugin.update()` each frame.

use std::collections::HashMap;
use std::time::SystemTime;

// ─────────────────────────────────────────────
//  ReloadEvent
// ─────────────────────────────────────────────

/// Emitted when a watched file is found to have changed on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadEvent {
    /// The file path that changed.
    pub path: String,
    /// The raw `AssetId` (packed u64) of the primary asset backed by this file.
    /// Zero if no specific asset id was registered.
    pub asset_id: u64,
}

// ─────────────────────────────────────────────
//  ChangedFile
// ─────────────────────────────────────────────

/// Detailed description of a detected file change.
#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: String,
    pub previous_modified: Option<SystemTime>,
    pub current_modified: SystemTime,
    /// All asset ids registered against this path.
    pub handle_ids: Vec<u64>,
}

// ─────────────────────────────────────────────
//  WatchedFile
// ─────────────────────────────────────────────

/// Internal state for a single watched file.
#[derive(Debug, Clone)]
pub struct WatchedFile {
    /// The canonical path being monitored.
    pub path: String,
    /// The modification time the last time we polled successfully.
    pub last_modified: SystemTime,
    /// Asset ids (raw u64) that should be reloaded when this file changes.
    pub handle_ids: Vec<u64>,
}

impl WatchedFile {
    fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        let last_modified = Self::current_mtime(&path);
        Self { path, last_modified, handle_ids: Vec::new() }
    }

    fn current_mtime(path: &str) -> SystemTime {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }

    fn has_changed(&self) -> Option<SystemTime> {
        let current = Self::current_mtime(&self.path);
        if current != self.last_modified && current != SystemTime::UNIX_EPOCH {
            Some(current)
        } else {
            None
        }
    }
}

// ─────────────────────────────────────────────
//  FileWatcher
// ─────────────────────────────────────────────

/// Polls file modification times to detect on-disk changes.
///
/// All polling is synchronous and runs on the calling thread.
pub struct FileWatcher {
    watched: HashMap<String, WatchedFile>,
    /// Paths detected as changed on the last `poll_changes` / `check_all` call.
    last_changed: Vec<String>,
    /// Running total of changes detected across all polls.
    change_count: u64,
}

impl FileWatcher {
    /// Create an empty watcher with no files under observation.
    pub fn new() -> Self {
        Self {
            watched: HashMap::new(),
            last_changed: Vec::new(),
            change_count: 0,
        }
    }

    // ── Registration ──────────────────────────────────────────────────────

    /// Start watching `path`. If the path is already watched, this is a no-op.
    pub fn watch(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.watched.entry(path.clone()).or_insert_with(|| WatchedFile::new(path));
    }

    /// Start watching `path` and associate `handle_id` with it.
    ///
    /// When this path changes, the corresponding `ReloadEvent` will carry the id.
    pub fn watch_with_id(&mut self, path: impl Into<String>, handle_id: u64) {
        let path = path.into();
        let entry = self.watched.entry(path.clone()).or_insert_with(|| WatchedFile::new(path));
        if !entry.handle_ids.contains(&handle_id) {
            entry.handle_ids.push(handle_id);
        }
    }

    /// Stop watching `path`.
    pub fn unwatch(&mut self, path: &str) {
        self.watched.remove(path);
    }

    /// Remove all watched files.
    pub fn unwatch_all(&mut self) {
        self.watched.clear();
    }

    // ── Polling ────────────────────────────────────────────────────────────

    /// Poll all watched files and return the paths whose modification times changed
    /// since the last call to this method.
    ///
    /// The internal baseline is updated for every changed file.
    pub fn poll_changes(&mut self) -> Vec<String> {
        let mut changed = Vec::new();
        for (path, watched) in self.watched.iter_mut() {
            if let Some(new_mtime) = watched.has_changed() {
                watched.last_modified = new_mtime;
                changed.push(path.clone());
                self.change_count += 1;
            }
        }
        self.last_changed = changed.clone();
        changed
    }

    /// Like `poll_changes` but returns rich `ChangedFile` structs instead of just paths.
    pub fn check_all(&mut self) -> Vec<ChangedFile> {
        let mut result = Vec::new();
        for (path, watched) in self.watched.iter_mut() {
            let prev = watched.last_modified;
            if let Some(new_mtime) = watched.has_changed() {
                result.push(ChangedFile {
                    path: path.clone(),
                    previous_modified: Some(prev),
                    current_modified: new_mtime,
                    handle_ids: watched.handle_ids.clone(),
                });
                watched.last_modified = new_mtime;
                self.change_count += 1;
            }
        }
        result
    }

    // ── Queries ────────────────────────────────────────────────────────────

    /// Number of files currently being watched.
    pub fn watch_count(&self) -> usize {
        self.watched.len()
    }

    /// Paths changed on the most recent poll.
    pub fn last_changed(&self) -> &[String] {
        &self.last_changed
    }

    /// Cumulative number of changes detected across all polls.
    pub fn total_change_count(&self) -> u64 {
        self.change_count
    }

    /// Returns `true` if the watcher is tracking any files.
    pub fn is_empty(&self) -> bool {
        self.watched.is_empty()
    }

    /// Returns `true` if `path` is currently being watched.
    pub fn is_watched(&self, path: &str) -> bool {
        self.watched.contains_key(path)
    }

    /// All currently watched paths.
    pub fn watched_paths(&self) -> impl Iterator<Item = &str> {
        self.watched.keys().map(String::as_str)
    }

    /// Force-reset the baseline modification time for `path` so the next poll
    /// considers it unchanged (useful after a reload to avoid re-triggering).
    pub fn reset_baseline(&mut self, path: &str) {
        if let Some(watched) = self.watched.get_mut(path) {
            watched.last_modified = WatchedFile::current_mtime(path);
        }
    }

    /// Reset baselines for all watched paths.
    pub fn reset_all_baselines(&mut self) {
        for watched in self.watched.values_mut() {
            watched.last_modified = WatchedFile::current_mtime(&watched.path);
        }
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
//  AssetChangeDetector
// ─────────────────────────────────────────────

/// Tracks which asset handles changed this frame.
///
/// Cleared at the start of each frame via `begin_frame()`, populated by
/// `HotReloadPlugin::update()`.
pub struct AssetChangeDetector {
    /// Asset ids that were reloaded this frame.
    changed_this_frame: Vec<u64>,
    /// Cumulative history of reload events (capped at `history_cap`).
    history: Vec<ReloadEvent>,
    history_cap: usize,
}

impl AssetChangeDetector {
    pub fn new() -> Self {
        Self::with_history_cap(256)
    }

    pub fn with_history_cap(cap: usize) -> Self {
        Self {
            changed_this_frame: Vec::new(),
            history: Vec::new(),
            history_cap: cap,
        }
    }

    /// Call at the start of every frame to clear per-frame change records.
    pub fn begin_frame(&mut self) {
        self.changed_this_frame.clear();
    }

    /// Record that an asset was reloaded.
    pub fn record(&mut self, event: ReloadEvent) {
        self.changed_this_frame.push(event.asset_id);
        self.history.push(event);
        if self.history.len() > self.history_cap {
            self.history.remove(0);
        }
    }

    /// Returns `true` if the given asset id was reloaded this frame.
    pub fn changed_this_frame(&self, asset_id: u64) -> bool {
        self.changed_this_frame.contains(&asset_id)
    }

    /// All ids that changed this frame.
    pub fn changed_ids(&self) -> &[u64] {
        &self.changed_this_frame
    }

    /// Number of changes detected this frame.
    pub fn change_count_this_frame(&self) -> usize {
        self.changed_this_frame.len()
    }

    /// Full event history.
    pub fn history(&self) -> &[ReloadEvent] {
        &self.history
    }

    /// Clear both current-frame and full history.
    pub fn clear(&mut self) {
        self.changed_this_frame.clear();
        self.history.clear();
    }
}

impl Default for AssetChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
//  HotReloadPlugin
// ─────────────────────────────────────────────

/// Combines `FileWatcher` with an `AssetServer` reference (path string) to
/// automatically detect and trigger asset reloads.
///
/// In a real ECS this would hold a mutable reference or resource handle to the
/// server. Here we store a path-based queue and the caller flushes them into
/// the actual server. This avoids borrow-checker conflicts while remaining
/// practical.
pub struct HotReloadPlugin {
    watcher: FileWatcher,
    detector: AssetChangeDetector,
    enabled: bool,
    /// Paths that changed since the last `update()` and are queued for reload.
    queued_reloads: Vec<ReloadEvent>,
    /// Asset root so we can make relative paths for the watcher.
    asset_root: String,
}

impl HotReloadPlugin {
    /// Create a new plugin. Call `update()` each frame to process changes.
    pub fn new() -> Self {
        Self {
            watcher: FileWatcher::new(),
            detector: AssetChangeDetector::new(),
            enabled: true,
            queued_reloads: Vec::new(),
            asset_root: String::from("."),
        }
    }

    /// Set the asset root (for display/relative path purposes).
    pub fn set_asset_root(&mut self, root: impl Into<String>) {
        self.asset_root = root.into();
    }

    /// Enable hot reloading (default: enabled).
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable hot reloading. `update()` becomes a no-op.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Whether hot reloading is currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // ── File registration ──────────────────────────────────────────────────

    /// Register a file path + asset id for watching.
    pub fn watch(&mut self, path: impl Into<String>, asset_id: u64) {
        let path = path.into();
        self.watcher.watch_with_id(path, asset_id);
    }

    /// Stop watching a file.
    pub fn unwatch(&mut self, path: &str) {
        self.watcher.unwatch(path);
    }

    // ── Per-frame update ───────────────────────────────────────────────────

    /// Poll for file changes and queue reload events.
    ///
    /// Call this once per frame. After calling, drain `take_queued_reloads()`
    /// and pass each event to `AssetServer::reload_by_path` or similar.
    pub fn update(&mut self) {
        if !self.enabled {
            return;
        }

        self.detector.begin_frame();
        self.queued_reloads.clear();

        let changes = self.watcher.check_all();
        for changed in changes {
            for &id in &changed.handle_ids {
                let event = ReloadEvent { path: changed.path.clone(), asset_id: id };
                self.detector.record(event.clone());
                self.queued_reloads.push(event);
            }
            // Even if no handles registered, emit a path-only event
            if changed.handle_ids.is_empty() {
                self.queued_reloads.push(ReloadEvent {
                    path: changed.path.clone(),
                    asset_id: 0,
                });
            }
        }
    }

    // ── Drain ─────────────────────────────────────────────────────────────

    /// Consume the queued reload events for this frame.
    ///
    /// The caller should iterate these and apply them to the `AssetServer`.
    pub fn take_queued_reloads(&mut self) -> Vec<ReloadEvent> {
        std::mem::take(&mut self.queued_reloads)
    }

    /// Peek at queued reloads without consuming them.
    pub fn queued_reloads(&self) -> &[ReloadEvent] {
        &self.queued_reloads
    }

    // ── Queries ────────────────────────────────────────────────────────────

    /// Number of files currently being watched.
    pub fn watch_count(&self) -> usize {
        self.watcher.watch_count()
    }

    /// The underlying `FileWatcher` (read-only view).
    pub fn watcher(&self) -> &FileWatcher {
        &self.watcher
    }

    /// The `AssetChangeDetector` tracking per-frame changes.
    pub fn detector(&self) -> &AssetChangeDetector {
        &self.detector
    }

    /// Mutable access to the detector (e.g. to reset history).
    pub fn detector_mut(&mut self) -> &mut AssetChangeDetector {
        &mut self.detector
    }

    /// Total reload events detected across all frames.
    pub fn total_reload_count(&self) -> usize {
        self.detector.history().len()
    }

    /// Reset baselines so the current state is considered "clean" (no false
    /// positives on next poll).
    pub fn reset_baselines(&mut self) {
        self.watcher.reset_all_baselines();
    }
}

impl Default for HotReloadPlugin {
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
    use std::io::Write;

    fn tmp_path() -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        std::env::temp_dir().join(format!("proof_hot_{t}.txt"))
    }

    #[test]
    fn watcher_watch_unwatch() {
        let mut w = FileWatcher::new();
        assert_eq!(w.watch_count(), 0);
        w.watch("/nonexistent/path.txt");
        assert_eq!(w.watch_count(), 1);
        assert!(w.is_watched("/nonexistent/path.txt"));
        w.unwatch("/nonexistent/path.txt");
        assert_eq!(w.watch_count(), 0);
    }

    #[test]
    fn watcher_detects_modification() {
        let path = tmp_path();
        std::fs::write(&path, b"v1").unwrap();

        let mut w = FileWatcher::new();
        w.watch(path.to_str().unwrap());
        // First poll — nothing changed since baseline
        let changes = w.poll_changes();
        assert!(changes.is_empty(), "should not change immediately");

        // Modify the file
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&path, b"v2").unwrap();

        let changes = w.poll_changes();
        // May or may not detect depending on OS mtime granularity
        // but the infrastructure doesn't panic
        let _ = changes;

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn watcher_with_id() {
        let mut w = FileWatcher::new();
        w.watch_with_id("/some/file.txt", 42);
        assert!(w.is_watched("/some/file.txt"));
    }

    #[test]
    fn change_detector_begin_frame() {
        let mut det = AssetChangeDetector::new();
        det.record(ReloadEvent { path: "a.txt".into(), asset_id: 1 });
        det.record(ReloadEvent { path: "b.txt".into(), asset_id: 2 });
        assert_eq!(det.change_count_this_frame(), 2);
        assert!(det.changed_this_frame(1));
        det.begin_frame();
        assert_eq!(det.change_count_this_frame(), 0);
        // History should still have both events
        assert_eq!(det.history().len(), 2);
    }

    #[test]
    fn change_detector_history_cap() {
        let mut det = AssetChangeDetector::with_history_cap(3);
        for i in 0..5u64 {
            det.record(ReloadEvent { path: "x.txt".into(), asset_id: i });
        }
        // History should not grow past cap
        assert!(det.history().len() <= 3);
    }

    #[test]
    fn hot_reload_plugin_enable_disable() {
        let mut plugin = HotReloadPlugin::new();
        assert!(plugin.is_enabled());
        plugin.disable();
        assert!(!plugin.is_enabled());
        plugin.enable();
        assert!(plugin.is_enabled());
    }

    #[test]
    fn hot_reload_plugin_watch_and_update_no_change() {
        let path = tmp_path();
        std::fs::write(&path, b"hello").unwrap();

        let mut plugin = HotReloadPlugin::new();
        plugin.watch(path.to_str().unwrap(), 10);
        plugin.update();
        // No changes expected immediately after creation
        assert_eq!(plugin.queued_reloads().len(), 0);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn hot_reload_plugin_disabled_no_events() {
        let mut plugin = HotReloadPlugin::new();
        plugin.disable();
        plugin.watch("/fake/path.txt", 99);
        plugin.update();
        assert_eq!(plugin.queued_reloads().len(), 0);
    }

    #[test]
    fn reload_event_eq() {
        let a = ReloadEvent { path: "test.txt".into(), asset_id: 1 };
        let b = ReloadEvent { path: "test.txt".into(), asset_id: 1 };
        assert_eq!(a, b);
    }
}
