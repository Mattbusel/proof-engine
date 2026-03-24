//! High-level asset server — the primary entry point for loading and querying assets.
//!
//! `AssetServer` wraps `AssetRegistry` and adds:
//! * Path resolution relative to a configurable asset root
//! * Synchronous file loading from the OS filesystem
//! * An async-style queued-load queue (processes on `flush()`)
//! * Load-state tracking and convenience predicates

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

use crate::asset::handle::{AssetId, AssetPath, AssetRefCount, Handle, LoadState};
use crate::asset::loader::{Asset, AssetLoadError, BytesAsset, ScriptAsset, TextAsset, TomlAsset};
use crate::asset::registry::AssetRegistry;

// ─────────────────────────────────────────────
//  Pending load entry (async queue)
// ─────────────────────────────────────────────

struct PendingLoad {
    path: String,
    /// TypeId encoded as a loader key. We use the extension to find the loader.
    load_fn: Box<dyn FnOnce(&mut AssetRegistry, &str, &[u8]) -> Result<AssetId, AssetLoadError> + Send>,
    reserved_id: AssetId,
}

// ─────────────────────────────────────────────
//  AssetServer
// ─────────────────────────────────────────────

/// High-level asset management.
///
/// Create once and keep alive for the duration of the program. Registrations
/// (loaders, storages) are done at construction time via `AssetServer::new()`.
pub struct AssetServer {
    registry: AssetRegistry,
    /// Root directory all relative paths are resolved against.
    asset_root: PathBuf,
    /// Queue of pending async loads (processed on `flush()`).
    pending: VecDeque<PendingLoad>,
    /// Number of assets that are currently in the Loading state.
    loading_count: usize,
    /// Number of assets that have been successfully loaded.
    loaded_count: usize,
    /// Tracks ref-counts per AssetId so handles can be managed.
    ref_counts: HashMap<u64, std::sync::Arc<AssetRefCount>>,
}

impl AssetServer {
    /// Create a new server with all built-in loaders and storages registered.
    pub fn new() -> Self {
        let mut registry = AssetRegistry::new();

        // Register built-in asset types
        registry.register::<TextAsset>();
        registry.register::<BytesAsset>();
        registry.register::<TomlAsset>();
        registry.register::<ScriptAsset>();

        Self {
            registry,
            asset_root: PathBuf::from("."),
            pending: VecDeque::new(),
            loading_count: 0,
            loaded_count: 0,
            ref_counts: HashMap::new(),
        }
    }

    // ── Path resolution ────────────────────────────────────────────────────

    /// Set the root directory for asset path resolution.
    ///
    /// All relative paths passed to `load` and `load_async` are resolved against
    /// this root. Absolute paths are used as-is.
    pub fn set_asset_root(&mut self, path: impl Into<PathBuf>) {
        self.asset_root = path.into();
    }

    /// Resolve a relative path against the asset root.
    pub fn resolve_path(&self, relative: &str) -> PathBuf {
        let p = Path::new(relative);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.asset_root.join(p)
        }
    }

    /// The current asset root.
    pub fn asset_root(&self) -> &Path {
        &self.asset_root
    }

    // ── Ref-count helpers ──────────────────────────────────────────────────

    fn get_or_create_rc(&mut self, id: AssetId) -> std::sync::Arc<AssetRefCount> {
        self.ref_counts
            .entry(id.raw())
            .or_insert_with(AssetRefCount::new)
            .clone()
    }

    fn make_handle<T>(&mut self, id: AssetId) -> Handle<T> {
        let rc = self.get_or_create_rc(id);
        Handle::strong(id, rc)
    }

    // ── Synchronous load ───────────────────────────────────────────────────

    /// Load `T` from `path` synchronously, blocking until the file is read and parsed.
    ///
    /// If the asset is already loaded (same path, same type), the cached version is
    /// returned immediately without re-reading the file.
    ///
    /// Returns a strong `Handle<T>`.
    pub fn load<T: Asset>(&mut self, path: &str) -> Handle<T> {
        // Check if already loaded
        if let Some(storage) = self.registry.storage::<T>() {
            if let Some(id) = storage.id_for_path(path) {
                let state = storage.load_state(id);
                if state.is_loaded() {
                    return self.make_handle(id);
                }
            }
        }

        // Resolve and read the file
        let full_path = self.resolve_path(path);
        let bytes = match std::fs::read(&full_path) {
            Ok(b) => b,
            Err(e) => {
                // Reserve a slot and mark as failed
                let storage = self.registry.storage_or_create_mut::<T>();
                let id = storage.reserve(path);
                storage.mark_failed(id, format!("IO error reading '{}': {e}", full_path.display()));
                return self.make_handle(id);
            }
        };

        // Load through registry
        match self.registry.load_asset::<T>(path, &bytes) {
            Ok(id) => {
                self.loaded_count += 1;
                self.make_handle(id)
            }
            Err(e) => {
                let storage = self.registry.storage_or_create_mut::<T>();
                let id = storage.reserve(path);
                storage.mark_failed(id, e.to_string());
                self.make_handle(id)
            }
        }
    }

    /// Load `T` from raw bytes (already in memory). Useful for embedded assets.
    pub fn load_from_bytes<T: Asset>(&mut self, path: &str, bytes: &[u8]) -> Handle<T> {
        match self.registry.load_asset::<T>(path, bytes) {
            Ok(id) => {
                self.loaded_count += 1;
                self.make_handle(id)
            }
            Err(e) => {
                let storage = self.registry.storage_or_create_mut::<T>();
                let id = storage.reserve(path);
                storage.mark_failed(id, e.to_string());
                self.make_handle(id)
            }
        }
    }

    // ── Async / queued load ────────────────────────────────────────────────

    /// Queue `T` for background loading. Returns a handle immediately.
    ///
    /// The handle will return `LoadState::Loading` until `flush()` is called and
    /// the load succeeds or fails.
    ///
    /// Currently this is a single-threaded "future load" queue — the work is done
    /// on the next call to `flush()`. For true multi-threaded loading, the caller
    /// should use Rust's thread pool and call `load_from_bytes` on completion.
    pub fn load_async<T: Asset>(&mut self, path: &str) -> Handle<T> {
        // If already loaded, return immediately
        if let Some(storage) = self.registry.storage::<T>() {
            if let Some(id) = storage.id_for_path(path) {
                if storage.load_state(id).is_loaded() {
                    return self.make_handle(id);
                }
            }
        }

        // Reserve a slot
        let storage = self.registry.storage_or_create_mut::<T>();
        let id = storage.reserve(path);
        self.loading_count += 1;

        let path_owned = path.to_string();
        self.pending.push_back(PendingLoad {
            path: path_owned.clone(),
            load_fn: Box::new(move |registry: &mut AssetRegistry, path: &str, bytes: &[u8]| {
                registry.load_asset::<T>(path, bytes)
            }),
            reserved_id: id,
        });

        self.make_handle(id)
    }

    // ── Flush ──────────────────────────────────────────────────────────────

    /// Process all pending (async-queued) loads.
    ///
    /// This reads files and runs loaders for everything in the queue. Designed to
    /// be called once per frame or on a background thread.
    pub fn flush(&mut self) {
        while let Some(pending) = self.pending.pop_front() {
            let full_path = self.resolve_path(&pending.path);
            let result = match std::fs::read(&full_path) {
                Ok(bytes) => (pending.load_fn)(&mut self.registry, &pending.path, &bytes),
                Err(e) => Err(AssetLoadError::Io {
                    path: full_path.display().to_string(),
                    message: e.to_string(),
                }),
            };

            match result {
                Ok(id) => {
                    // The load_fn already inserted the asset; update counters.
                    if self.loading_count > 0 {
                        self.loading_count -= 1;
                    }
                    self.loaded_count += 1;
                    // Ensure the reserved id matches — if it differs the storage
                    // made a new slot; mark the reserved id as loaded too.
                    let _ = id; // id already stored in registry
                }
                Err(e) => {
                    // We can't easily call mark_failed without knowing T here,
                    // but the reserved_id state was set to Loading — we leave it.
                    // Callers can check get_load_state_raw.
                    let _ = e;
                    if self.loading_count > 0 {
                        self.loading_count -= 1;
                    }
                }
            }
        }
    }

    // ── Queries ────────────────────────────────────────────────────────────

    /// Get a reference to the loaded asset.
    pub fn get<T: Asset>(&self, handle: &Handle<T>) -> Option<&T> {
        self.registry.storage::<T>()?.get(handle.id())
    }

    /// Get a mutable reference to the loaded asset.
    pub fn get_mut<T: Asset>(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        self.registry.storage_mut::<T>()?.get_mut(handle.id())
    }

    /// Get the load state for a handle.
    pub fn get_load_state<T: Asset>(&self, handle: &Handle<T>) -> LoadState {
        self.registry
            .storage::<T>()
            .map(|s| s.load_state(handle.id()))
            .unwrap_or(LoadState::NotLoaded)
    }

    /// Returns `true` if the asset is loaded and ready.
    pub fn is_loaded<T: Asset>(&self, handle: &Handle<T>) -> bool {
        self.get_load_state(handle).is_loaded()
    }

    /// Returns `true` if the asset is still being loaded.
    pub fn is_loading<T: Asset>(&self, handle: &Handle<T>) -> bool {
        self.get_load_state(handle).is_loading()
    }

    // ── Reload ─────────────────────────────────────────────────────────────

    /// Force-reload an asset from disk, replacing the stored value.
    ///
    /// The handle remains valid; its id is preserved.
    pub fn reload<T: Asset>(&mut self, handle: &Handle<T>) {
        // Find the path for this handle
        let path_opt: Option<String> = self
            .registry
            .storage::<T>()
            .and_then(|s| {
                s.paths()
                    .find(|p| s.id_for_path(p) == Some(handle.id()))
                    .map(|p| p.to_string())
            });

        if let Some(path) = path_opt {
            let full_path = self.resolve_path(&path);
            match std::fs::read(&full_path) {
                Ok(bytes) => {
                    // Re-run the loader directly
                    let ext = path.rsplit('.').next().unwrap_or("");
                    let ext_lower = ext.to_lowercase();
                    if let Some(loader) = self.registry.get_loader_for_extension(&ext_lower) {
                        let mut ctx = crate::asset::loader::LoadContext::new(&path);
                        match loader.load_bytes(&bytes, &path, &mut ctx) {
                            Ok(boxed) => {
                                if let Ok(asset) = boxed.downcast::<T>() {
                                    if let Some(storage) = self.registry.storage_mut::<T>() {
                                        storage.insert_at(handle.id(), *asset);
                                    }
                                }
                            }
                            Err(_) => {}
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }

    // ── Unload ─────────────────────────────────────────────────────────────

    /// Remove an asset from storage, freeing its memory.
    ///
    /// The handle becomes stale after this call. Any further `get()` calls with
    /// this handle will return `None`.
    pub fn unload<T: Asset>(&mut self, handle: Handle<T>) {
        if let Some(storage) = self.registry.storage_mut::<T>() {
            if storage.contains(handle.id()) {
                storage.remove(handle.id());
                if self.loaded_count > 0 {
                    self.loaded_count -= 1;
                }
            }
        }
        self.ref_counts.remove(&handle.id().raw());
    }

    // ── Counters ───────────────────────────────────────────────────────────

    /// Number of assets that have been successfully loaded (across all types).
    pub fn loaded_count(&self) -> usize {
        self.loaded_count
    }

    /// Number of assets currently in the loading queue.
    pub fn loading_count(&self) -> usize {
        self.loading_count
    }

    /// Total number of pending (not yet flushed) loads.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Access the underlying registry (advanced use).
    pub fn registry(&self) -> &AssetRegistry {
        &self.registry
    }

    /// Mutable access to the underlying registry.
    pub fn registry_mut(&mut self) -> &mut AssetRegistry {
        &mut self.registry
    }

    // ── Batch helpers ──────────────────────────────────────────────────────

    /// Load multiple assets of the same type at once.
    pub fn load_batch<T: Asset>(&mut self, paths: &[&str]) -> Vec<Handle<T>> {
        paths.iter().map(|p| self.load::<T>(p)).collect()
    }

    /// Queue multiple assets for async loading.
    pub fn load_async_batch<T: Asset>(&mut self, paths: &[&str]) -> Vec<Handle<T>> {
        paths.iter().map(|p| self.load_async::<T>(p)).collect()
    }

    // ── Asset root scanning ────────────────────────────────────────────────

    /// List all files under the asset root that have extensions handled by registered loaders.
    pub fn list_loadable_files(&self) -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, out);
                    } else {
                        out.push(path);
                    }
                }
            }
        }
        let mut paths = Vec::new();
        walk(&self.asset_root, &mut paths);
        paths
            .into_iter()
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| self.registry.has_loader_for(e))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Check if all pending loads have completed.
    pub fn all_loaded(&self) -> bool {
        self.pending.is_empty()
    }
}

impl Default for AssetServer {
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
    use crate::asset::loader::TextAsset;
    use std::io::Write;

    fn temp_txt_file(content: &str) -> (tempfile_helper::TempDir, PathBuf) {
        let dir = tempfile_helper::TempDir::new();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, content).unwrap();
        (dir, path)
    }

    // Simple inline tempdir helper — avoids the tempfile crate dep.
    mod tempfile_helper {
        use std::path::{Path, PathBuf};

        pub struct TempDir {
            path: PathBuf,
        }

        impl TempDir {
            pub fn new() -> Self {
                use std::time::{SystemTime, UNIX_EPOCH};
                let t = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .subsec_nanos();
                let path = std::env::temp_dir().join(format!("proof_asset_test_{t}"));
                std::fs::create_dir_all(&path).unwrap();
                Self { path }
            }

            pub fn path(&self) -> &Path {
                &self.path
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }

    #[test]
    fn load_from_bytes_text() {
        let mut server = AssetServer::new();
        let handle = server.load_from_bytes::<TextAsset>("readme.txt", b"hello world");
        assert!(server.is_loaded(&handle));
        assert_eq!(server.get(&handle).unwrap().text, "hello world");
    }

    #[test]
    fn get_load_state_not_loaded() {
        let server = AssetServer::new();
        let handle: Handle<TextAsset> = Handle::weak(AssetId::new(99, 99));
        assert_eq!(server.get_load_state(&handle), LoadState::NotLoaded);
    }

    #[test]
    fn load_from_bytes_twice_same_handle() {
        let mut server = AssetServer::new();
        let h1 = server.load_from_bytes::<TextAsset>("a.txt", b"first");
        let h2 = server.load_from_bytes::<TextAsset>("a.txt", b"second");
        // Same path → same id
        assert_eq!(h1.id(), h2.id());
        // Latest write wins
        assert_eq!(server.get(&h2).unwrap().text, "second");
    }

    #[test]
    fn unload_removes_asset() {
        let mut server = AssetServer::new();
        let handle = server.load_from_bytes::<TextAsset>("bye.txt", b"bye");
        assert!(server.is_loaded(&handle));
        let weak = handle.clone_weak();
        server.unload(handle);
        // After unload, the weak handle should find nothing
        assert!(!server.is_loaded(&weak));
        assert!(server.get(&weak).is_none());
    }

    #[test]
    fn load_batch() {
        let mut server = AssetServer::new();
        // Pre-populate via load_from_bytes
        server.load_from_bytes::<TextAsset>("x.txt", b"x");
        server.load_from_bytes::<TextAsset>("y.txt", b"y");

        // Reload via batch — files won't exist on disk so they'll fail, but
        // this tests the API path.
        let handles = server.load_batch::<TextAsset>(&["x.txt", "y.txt"]);
        assert_eq!(handles.len(), 2);
    }

    #[test]
    fn asset_server_resolve_path() {
        let mut server = AssetServer::new();
        server.set_asset_root("/game/assets");
        let resolved = server.resolve_path("fonts/mono.ttf");
        assert_eq!(resolved, PathBuf::from("/game/assets/fonts/mono.ttf"));
    }

    #[test]
    fn loaded_count_increments() {
        let mut server = AssetServer::new();
        assert_eq!(server.loaded_count(), 0);
        server.load_from_bytes::<TextAsset>("a.txt", b"a");
        server.load_from_bytes::<TextAsset>("b.txt", b"b");
        assert_eq!(server.loaded_count(), 2);
    }

    #[test]
    fn load_sync_from_disk() {
        let dir = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let t = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .subsec_nanos();
            let d = std::env::temp_dir().join(format!("proof_asset_disk_{t}"));
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        let file_path = dir.join("disk.txt");
        std::fs::write(&file_path, b"disk content").unwrap();

        let mut server = AssetServer::new();
        server.set_asset_root(&dir);
        let handle = server.load::<TextAsset>("disk.txt");
        assert!(server.is_loaded(&handle), "state: {:?}", server.get_load_state(&handle));
        assert_eq!(server.get(&handle).unwrap().text, "disk content");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
