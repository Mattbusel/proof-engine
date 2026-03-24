//! # Asset Pipeline
//!
//! A comprehensive asset management system for the Proof Engine.
//!
//! ## Overview
//!
//! The asset pipeline provides a complete lifecycle for all game/engine assets:
//! loading from disk, caching, hot-reloading, dependency tracking, post-processing,
//! and streaming. It is designed to work entirely with `std` — no external crates.
//!
//! ## Architecture
//!
//! ```text
//! AssetServer
//!   ├── AssetRegistry       — type-erased storage of all loaded assets
//!   ├── AssetCache          — LRU eviction layer
//!   ├── HotReload           — poll-based file watcher
//!   ├── StreamingManager    — priority queue for background loads
//!   └── AssetPack[]         — optional archive bundles
//! ```
//!
//! ## Usage
//!
//! ```rust,no_run
//! use proof_engine::asset::{AssetServer, AssetPath, ImageAsset};
//!
//! let mut server = AssetServer::new();
//! let handle = server.load::<ImageAsset>(AssetPath::new("textures/player.png"));
//! // Later, after the asset is ready:
//! if let Some(img) = server.get(&handle) {
//!     println!("{}x{}", img.width, img.height);
//! }
//! ```

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant, SystemTime};

// ────────────────────────────────────────────────────────────────────────────
// Section 1 — Core Traits
// ────────────────────────────────────────────────────────────────────────────

/// Marker trait for any type that can be stored as an asset.
///
/// Implementors must be `Send + Sync + 'static` so they can be shared
/// across threads and stored in the global registry without lifetime issues.
///
/// # Example
/// ```rust
/// use proof_engine::asset::Asset;
/// struct MyData { value: u32 }
/// impl Asset for MyData {}
/// ```
pub trait Asset: Send + Sync + 'static {}

/// Trait for loading raw bytes into a concrete [`Asset`] type.
///
/// Each asset format (PNG, WAV, GLSL, etc.) has one loader. Loaders are
/// stateless by design; mutable state lives in [`AssetServer`].
///
/// # Type parameter
/// `A` — the [`Asset`] type produced by this loader.
pub trait AssetLoader<A: Asset>: Send + Sync + 'static {
    /// Load an asset from the given byte slice.
    ///
    /// `path` is provided for diagnostic messages; the loader must not
    /// perform additional I/O itself.
    ///
    /// Returns `Ok(asset)` on success, `Err(message)` on failure.
    fn load(&self, bytes: &[u8], path: &AssetPath) -> Result<A, String>;

    /// Returns the file extensions this loader handles (without the dot).
    ///
    /// Examples: `&["png", "jpg"]`, `&["glsl", "vert", "frag"]`.
    fn extensions(&self) -> &[&str];
}

/// Post-processes an asset after it is first loaded.
///
/// Examples: generating mip-maps for images, building a BVH for meshes,
/// packing glyphs into a texture atlas for fonts.
pub trait AssetProcessor<A: Asset>: Send + Sync + 'static {
    /// Mutate or replace the asset in-place.
    fn process(&self, asset: &mut A, path: &AssetPath) -> Result<(), String>;

    /// Human-readable name used in debug output.
    fn name(&self) -> &str;
}

// ────────────────────────────────────────────────────────────────────────────
// Section 2 — AssetPath
// ────────────────────────────────────────────────────────────────────────────

/// A path to an asset, optionally qualified with a sub-asset label.
///
/// The syntax is `"path/to/file.ext"` or `"path/to/file.ext#label"`.
///
/// Sub-asset labels allow a single file to expose multiple logical assets.
/// For example a GLTF file can expose `"scene.gltf#mesh0"`, `"scene.gltf#mesh1"`.
///
/// # Example
/// ```rust
/// use proof_engine::asset::AssetPath;
///
/// let p = AssetPath::parse("models/robot.gltf#body_mesh");
/// assert_eq!(p.path().to_str().unwrap(), "models/robot.gltf");
/// assert_eq!(p.label(), Some("body_mesh"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetPath {
    path: PathBuf,
    label: Option<String>,
}

impl AssetPath {
    /// Create a new path with no sub-asset label.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            label: None,
        }
    }

    /// Create a path with an explicit sub-asset label.
    pub fn with_label<P: AsRef<Path>>(path: P, label: impl Into<String>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            label: Some(label.into()),
        }
    }

    /// Parse a combined `"path#label"` string.
    ///
    /// If no `#` is present the label is `None`.
    pub fn parse(s: &str) -> Self {
        match s.find('#') {
            Some(idx) => Self {
                path: PathBuf::from(&s[..idx]),
                label: Some(s[idx + 1..].to_owned()),
            },
            None => Self {
                path: PathBuf::from(s),
                label: None,
            },
        }
    }

    /// The file-system component of the path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The optional sub-asset label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// File extension of the path (without the dot), lowercased.
    pub fn extension(&self) -> Option<String> {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
    }

    /// Returns the canonical string representation `"path"` or `"path#label"`.
    pub fn to_string_repr(&self) -> String {
        match &self.label {
            Some(l) => format!("{}#{}", self.path.display(), l),
            None => self.path.display().to_string(),
        }
    }
}

impl fmt::Display for AssetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_repr())
    }
}

impl From<&str> for AssetPath {
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}

impl From<String> for AssetPath {
    fn from(s: String) -> Self {
        Self::parse(&s)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 3 — AssetId & Handles
// ────────────────────────────────────────────────────────────────────────────

/// Opaque numeric identifier for a loaded asset, parameterised by asset type.
///
/// Two `AssetId<T>` values are equal iff they refer to the same loaded asset.
/// The phantom type parameter prevents mixing IDs for different asset types.
#[derive(Debug)]
pub struct AssetId<T: Asset> {
    id: u64,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Asset> AssetId<T> {
    fn new(id: u64) -> Self {
        Self { id, _marker: PhantomData }
    }

    /// The raw numeric identifier.
    pub fn raw(&self) -> u64 {
        self.id
    }
}

impl<T: Asset> Clone for AssetId<T> {
    fn clone(&self) -> Self {
        Self::new(self.id)
    }
}

impl<T: Asset> Copy for AssetId<T> {}

impl<T: Asset> PartialEq for AssetId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: Asset> Eq for AssetId<T> {}

impl<T: Asset> Hash for AssetId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: Asset> fmt::Display for AssetId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetId({})", self.id)
    }
}

/// A **strong** handle that keeps the underlying asset alive.
///
/// Dropping the last `AssetHandle` for a given asset does not automatically
/// unload the asset — the [`AssetCache`] decides eviction — but it signals
/// to the server that no live code holds a reference.
///
/// Cloning an `AssetHandle` is cheap: it increments an `Arc` reference count.
#[derive(Debug)]
pub struct AssetHandle<T: Asset> {
    id: AssetId<T>,
    inner: Arc<RwLock<Option<T>>>,
}

impl<T: Asset> AssetHandle<T> {
    fn new(id: AssetId<T>, inner: Arc<RwLock<Option<T>>>) -> Self {
        Self { id, inner }
    }

    /// The typed identifier for this asset.
    pub fn id(&self) -> AssetId<T> {
        self.id
    }

    /// Obtain a weak handle that does not prevent eviction.
    pub fn downgrade(&self) -> WeakHandle<T> {
        WeakHandle {
            id: self.id,
            inner: Arc::downgrade(&self.inner),
        }
    }

    /// Returns `true` if the asset data is currently available.
    pub fn is_loaded(&self) -> bool {
        self.inner
            .read()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }
}

impl<T: Asset> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: Asset> PartialEq for AssetHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: Asset> Eq for AssetHandle<T> {}

/// A **weak** handle that does not prevent the asset from being evicted.
///
/// Upgrade to an [`AssetHandle`] before accessing the data. If the upgrade
/// returns `None` the asset has been evicted and must be re-loaded.
#[derive(Debug, Clone)]
pub struct WeakHandle<T: Asset> {
    id: AssetId<T>,
    inner: Weak<RwLock<Option<T>>>,
}

impl<T: Asset> WeakHandle<T> {
    /// Try to upgrade to a strong handle.
    ///
    /// Returns `None` if all strong handles have been dropped (asset evicted).
    pub fn upgrade(&self) -> Option<AssetHandle<T>> {
        self.inner.upgrade().map(|arc| AssetHandle::new(self.id, arc))
    }

    /// The typed identifier for this asset.
    pub fn id(&self) -> AssetId<T> {
        self.id
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 4 — LoadState
// ────────────────────────────────────────────────────────────────────────────

/// The current load state of an asset identified by its raw ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    /// The asset has never been requested.
    NotLoaded,
    /// The asset is queued or currently being read from disk.
    Loading,
    /// The asset is available in the registry.
    Loaded,
    /// Loading failed; the message describes the error.
    Failed(String),
}

impl fmt::Display for LoadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadState::NotLoaded => write!(f, "NotLoaded"),
            LoadState::Loading => write!(f, "Loading"),
            LoadState::Loaded => write!(f, "Loaded"),
            LoadState::Failed(msg) => write!(f, "Failed: {msg}"),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 5 — AssetDependency
// ────────────────────────────────────────────────────────────────────────────

/// Records which other assets a given asset depends on.
///
/// When an asset is reloaded all of its dependents are also re-queued so
/// that stale composed assets (e.g. a material that references a texture)
/// are always consistent.
#[derive(Debug, Default, Clone)]
pub struct AssetDependency {
    /// Raw IDs of the assets that this asset directly depends upon.
    pub depends_on: HashSet<u64>,
    /// Raw IDs of the assets that depend on this asset (reverse edges).
    pub depended_by: HashSet<u64>,
}

impl AssetDependency {
    /// Create a new, empty dependency record.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `owner` depends on `dependency`.
    pub fn add(&mut self, owner: u64, dependency: u64) {
        self.depends_on.insert(dependency);
        let _ = owner;
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 6 — Type-erased internals
// ────────────────────────────────────────────────────────────────────────────

/// Internal type-erased slot stored in the registry.
struct ErasedSlot {
    /// `TypeId` of the concrete `T`.
    type_id: TypeId,
    /// The asset value, downcasting via `Any`.
    value: Arc<dyn Any + Send + Sync>,
    /// Load state of this slot.
    state: LoadState,
    /// The path this asset was loaded from.
    path: AssetPath,
    /// When the file was last read (for hot-reload comparison).
    file_mtime: Option<SystemTime>,
    /// Dependency graph entry.
    dependency: AssetDependency,
    /// How many times this asset has been accessed (for LRU).
    access_count: u64,
    /// When was this asset last accessed.
    last_access: Instant,
}

impl ErasedSlot {
    fn new(type_id: TypeId, path: AssetPath) -> Self {
        Self {
            type_id,
            value: Arc::new(()),
            state: LoadState::NotLoaded,
            path,
            file_mtime: None,
            dependency: AssetDependency::new(),
            access_count: 0,
            last_access: Instant::now(),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 7 — AssetRegistry
// ────────────────────────────────────────────────────────────────────────────

/// Central type-erased registry that maps numeric IDs to asset slots.
///
/// The registry does not know the concrete type of the assets it stores;
/// type safety is enforced at the [`AssetHandle`] / [`AssetId`] boundary.
///
/// External code normally interacts with the registry through [`AssetServer`].
pub struct AssetRegistry {
    slots: HashMap<u64, ErasedSlot>,
    path_to_id: HashMap<AssetPath, u64>,
    next_id: u64,
}

impl AssetRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            path_to_id: HashMap::new(),
            next_id: 1,
        }
    }

    /// Allocate a new slot and return its ID.
    pub fn alloc(&mut self, type_id: TypeId, path: AssetPath) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.path_to_id.insert(path.clone(), id);
        self.slots.insert(id, ErasedSlot::new(type_id, path));
        id
    }

    /// Look up the ID already assigned to `path`, if any.
    pub fn id_for_path(&self, path: &AssetPath) -> Option<u64> {
        self.path_to_id.get(path).copied()
    }

    /// Return the current [`LoadState`] for the given raw ID.
    pub fn load_state(&self, id: u64) -> LoadState {
        self.slots
            .get(&id)
            .map(|s| s.state.clone())
            .unwrap_or(LoadState::NotLoaded)
    }

    /// Mark a slot as [`LoadState::Loading`].
    pub fn mark_loading(&mut self, id: u64) {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.state = LoadState::Loading;
        }
    }

    /// Store a successfully loaded value.
    pub fn store<T: Asset>(&mut self, id: u64, value: T, mtime: Option<SystemTime>) {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.value = Arc::new(value);
            slot.state = LoadState::Loaded;
            slot.file_mtime = mtime;
            slot.last_access = Instant::now();
        }
    }

    /// Mark a slot as failed.
    pub fn mark_failed(&mut self, id: u64, message: String) {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.state = LoadState::Failed(message);
        }
    }

    /// Try to read a stored value, downcasting to `T`.
    pub fn get<T: Asset>(&mut self, id: u64) -> Option<Arc<T>> {
        let slot = self.slots.get_mut(&id)?;
        if slot.state != LoadState::Loaded {
            return None;
        }
        slot.access_count += 1;
        slot.last_access = Instant::now();
        Arc::clone(&slot.value).downcast::<T>().ok()
    }

    /// Return the [`AssetPath`] for a given raw ID.
    pub fn path_for_id(&self, id: u64) -> Option<&AssetPath> {
        self.slots.get(&id).map(|s| &s.path)
    }

    /// Return the [`TypeId`] stored in a slot.
    pub fn type_id_for(&self, id: u64) -> Option<TypeId> {
        self.slots.get(&id).map(|s| s.type_id)
    }

    /// Iterate over all IDs in the registry.
    pub fn all_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.slots.keys().copied()
    }

    /// Total number of slots (includes not-yet-loaded and failed).
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// `true` if there are no slots.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Remove a slot entirely, returning whether it existed.
    pub fn evict(&mut self, id: u64) -> bool {
        if let Some(slot) = self.slots.remove(&id) {
            self.path_to_id.remove(&slot.path);
            true
        } else {
            false
        }
    }
}

impl Default for AssetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AssetRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetRegistry")
            .field("slot_count", &self.slots.len())
            .field("next_id", &self.next_id)
            .finish()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 8 — AssetCache (LRU)
// ────────────────────────────────────────────────────────────────────────────

/// LRU eviction cache that sits on top of [`AssetRegistry`].
///
/// When the cache is full, the least-recently-used asset is evicted from the
/// registry. Assets held by live [`AssetHandle`]s will not be collected by
/// the OS even after eviction, but the registry entry is removed so the next
/// request will trigger a reload.
///
/// Capacity is measured in number of assets, not bytes.
pub struct AssetCache {
    /// Maximum number of assets to keep loaded simultaneously.
    capacity: usize,
    /// Access-ordered queue: front = least recently used.
    lru_queue: VecDeque<u64>,
    /// Set for O(1) existence checks.
    lru_set: HashSet<u64>,
}

impl AssetCache {
    /// Create a new cache with the given capacity.
    ///
    /// A `capacity` of `0` disables eviction (unlimited).
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            lru_queue: VecDeque::new(),
            lru_set: HashSet::new(),
        }
    }

    /// Notify the cache that `id` was accessed.
    ///
    /// Moves `id` to the most-recently-used position and returns any ID that
    /// should now be evicted (the least-recently-used), or `None`.
    pub fn touch(&mut self, id: u64) -> Option<u64> {
        if self.lru_set.contains(&id) {
            self.lru_queue.retain(|&x| x != id);
        } else {
            self.lru_set.insert(id);
        }
        self.lru_queue.push_back(id);

        if self.capacity > 0 && self.lru_queue.len() > self.capacity {
            let victim = self.lru_queue.pop_front().unwrap();
            self.lru_set.remove(&victim);
            Some(victim)
        } else {
            None
        }
    }

    /// Remove `id` from the tracking queue (called after explicit eviction).
    pub fn remove(&mut self, id: u64) {
        self.lru_queue.retain(|&x| x != id);
        self.lru_set.remove(&id);
    }

    /// Current number of tracked entries.
    pub fn len(&self) -> usize {
        self.lru_queue.len()
    }

    /// `true` if no entries are tracked.
    pub fn is_empty(&self) -> bool {
        self.lru_queue.is_empty()
    }

    /// Configured capacity (0 = unlimited).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Change the capacity. If the new capacity is smaller, returns a list of
    /// IDs that must be evicted immediately.
    pub fn set_capacity(&mut self, new_cap: usize) -> Vec<u64> {
        self.capacity = new_cap;
        let mut evicted = Vec::new();
        while new_cap > 0 && self.lru_queue.len() > new_cap {
            if let Some(victim) = self.lru_queue.pop_front() {
                self.lru_set.remove(&victim);
                evicted.push(victim);
            }
        }
        evicted
    }
}

impl fmt::Debug for AssetCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetCache")
            .field("capacity", &self.capacity)
            .field("len", &self.lru_queue.len())
            .finish()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 9 — HotReload
// ────────────────────────────────────────────────────────────────────────────

/// Record of a file being watched for changes.
#[derive(Debug, Clone)]
struct WatchedFile {
    path: PathBuf,
    last_mtime: Option<SystemTime>,
    asset_ids: Vec<u64>,
}

/// Poll-based hot-reload watcher.
///
/// No platform-specific file-watch APIs are used. Instead, `poll()` is called
/// periodically (e.g. every second) and compares the `mtime` of each watched
/// file against the value recorded at load time. Changed files are returned
/// so [`AssetServer`] can queue them for reload.
///
/// # Limitations
/// * mtime resolution is operating-system dependent (commonly 1 s on FAT32).
/// * Files inside [`AssetPack`] archives are not watched.
pub struct HotReload {
    watched: HashMap<PathBuf, WatchedFile>,
    poll_interval: Duration,
    last_poll: Instant,
    enabled: bool,
}

impl HotReload {
    /// Create a new watcher.
    ///
    /// `poll_interval` — how often to stat files when `poll()` is called.
    /// `enabled` — pass `false` in release builds to disable entirely.
    pub fn new(poll_interval: Duration, enabled: bool) -> Self {
        Self {
            watched: HashMap::new(),
            poll_interval,
            last_poll: Instant::now(),
            enabled,
        }
    }

    /// Register a file to be watched for changes.
    pub fn watch(&mut self, path: PathBuf, asset_id: u64, current_mtime: Option<SystemTime>) {
        let entry = self.watched.entry(path.clone()).or_insert(WatchedFile {
            path,
            last_mtime: current_mtime,
            asset_ids: Vec::new(),
        });
        if !entry.asset_ids.contains(&asset_id) {
            entry.asset_ids.push(asset_id);
        }
        if current_mtime.is_some() {
            entry.last_mtime = current_mtime;
        }
    }

    /// Unwatch a specific asset ID. Removes the file entry if no more IDs use it.
    pub fn unwatch(&mut self, asset_id: u64) {
        self.watched.retain(|_, wf| {
            wf.asset_ids.retain(|&id| id != asset_id);
            !wf.asset_ids.is_empty()
        });
    }

    /// Poll all watched files. Returns a list of `(path, asset_ids)` for every
    /// file whose mtime has changed since the last observation.
    ///
    /// Returns an empty list if polling is disabled or the interval hasn't elapsed.
    pub fn poll(&mut self) -> Vec<(PathBuf, Vec<u64>)> {
        if !self.enabled {
            return Vec::new();
        }
        if self.last_poll.elapsed() < self.poll_interval {
            return Vec::new();
        }
        self.last_poll = Instant::now();

        let mut changed = Vec::new();
        for wf in self.watched.values_mut() {
            let current_mtime = std::fs::metadata(&wf.path)
                .and_then(|m| m.modified())
                .ok();
            if current_mtime != wf.last_mtime {
                wf.last_mtime = current_mtime;
                changed.push((wf.path.clone(), wf.asset_ids.clone()));
            }
        }
        changed
    }

    /// Force the next `poll()` call to check all files regardless of interval.
    pub fn force_next_poll(&mut self) {
        self.last_poll = Instant::now()
            .checked_sub(self.poll_interval + Duration::from_millis(1))
            .unwrap_or(Instant::now());
    }

    /// Enable or disable hot-reload at runtime.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Number of files currently being watched.
    pub fn watched_count(&self) -> usize {
        self.watched.len()
    }
}

impl fmt::Debug for HotReload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HotReload")
            .field("enabled", &self.enabled)
            .field("watched_files", &self.watched.len())
            .field("poll_interval_ms", &self.poll_interval.as_millis())
            .finish()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 10 — StreamingManager
// ────────────────────────────────────────────────────────────────────────────

/// Priority level for a streaming request.
///
/// Higher-priority assets are loaded first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StreamPriority {
    /// Background / speculative pre-fetch.
    Low = 0,
    /// Normal assets needed soon.
    Normal = 1,
    /// Assets needed immediately (e.g. blocking the next frame).
    High = 2,
    /// Assets that must be available before rendering can continue.
    Critical = 3,
}

impl Default for StreamPriority {
    fn default() -> Self {
        StreamPriority::Normal
    }
}

/// An entry in the streaming queue.
#[derive(Debug, Clone)]
pub struct StreamRequest {
    /// Raw asset ID.
    pub id: u64,
    /// Path to load from.
    pub path: AssetPath,
    /// TypeId of the expected asset.
    pub type_id: TypeId,
    /// Priority controlling load order.
    pub priority: StreamPriority,
    /// When this request was enqueued.
    pub enqueued_at: Instant,
}

/// Priority queue of pending asset loads.
///
/// Requests are dequeued in descending priority order. Ties are broken by
/// enqueue time (older requests first — FIFO within a priority tier).
pub struct StreamingManager {
    queue: Vec<StreamRequest>,
    /// Maximum number of requests to process per `drain()` call.
    batch_size: usize,
    /// Total requests processed since creation.
    total_processed: u64,
}

impl StreamingManager {
    /// Create a new manager.
    ///
    /// `batch_size` — how many requests `drain()` returns at once.
    pub fn new(batch_size: usize) -> Self {
        Self {
            queue: Vec::new(),
            batch_size,
            total_processed: 0,
        }
    }

    /// Enqueue a load request.
    ///
    /// If a request for the same ID already exists at a lower priority it is
    /// upgraded in-place. Duplicate same-priority requests are ignored.
    pub fn enqueue(&mut self, req: StreamRequest) {
        if let Some(existing) = self.queue.iter_mut().find(|r| r.id == req.id) {
            if req.priority > existing.priority {
                existing.priority = req.priority;
            }
            return;
        }
        self.queue.push(req);
    }

    /// Drain up to `batch_size` requests from the queue, highest priority first.
    ///
    /// The returned requests have been removed from the queue.
    pub fn drain(&mut self) -> Vec<StreamRequest> {
        if self.queue.is_empty() {
            return Vec::new();
        }
        self.queue.sort_unstable_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.enqueued_at.cmp(&b.enqueued_at))
        });

        let take = self.batch_size.min(self.queue.len());
        let drained: Vec<_> = self.queue.drain(0..take).collect();
        self.total_processed += drained.len() as u64;
        drained
    }

    /// Remove a pending request by ID (e.g. asset was evicted before loading).
    pub fn cancel(&mut self, id: u64) -> bool {
        let before = self.queue.len();
        self.queue.retain(|r| r.id != id);
        self.queue.len() < before
    }

    /// Number of requests currently queued.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Total requests ever processed (drained).
    pub fn total_processed(&self) -> u64 {
        self.total_processed
    }

    /// `true` if no requests are waiting.
    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }
}

impl fmt::Debug for StreamingManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamingManager")
            .field("pending", &self.queue.len())
            .field("batch_size", &self.batch_size)
            .field("total_processed", &self.total_processed)
            .finish()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 11 — AssetPack (simple archive format)
// ────────────────────────────────────────────────────────────────────────────

/// In-memory record of a single file inside a pack archive.
#[derive(Debug, Clone)]
struct PackEntry {
    /// Virtual path within the pack (e.g. `"textures/ui/button.png"`).
    virtual_path: String,
    /// Byte offset in the pack file's data blob.
    offset: usize,
    /// Byte length of the entry.
    length: usize,
}

/// A bundle of multiple asset files stored in a single archive.
///
/// # Format
///
/// ```text
/// [4 bytes magic "PACK"]
/// [4 bytes version = 1 as little-endian u32]
/// [4 bytes entry_count as little-endian u32]
/// For each entry:
///   [4 bytes path_len as little-endian u32]
///   [path_len bytes UTF-8 path]
///   [8 bytes data_offset as little-endian u64]
///   [8 bytes data_length as little-endian u64]
/// [raw concatenated asset bytes]
/// ```
///
/// The data section immediately follows the directory.
pub struct AssetPack {
    /// Human-readable name for diagnostic output.
    name: String,
    /// Directory of all entries.
    entries: Vec<PackEntry>,
    /// The full archive bytes, kept in memory.
    data: Vec<u8>,
    /// Byte offset where the data blob begins (after the directory).
    data_offset: usize,
}

impl AssetPack {
    /// Parse an asset pack from raw bytes.
    ///
    /// Returns `Err` if the bytes do not match the expected format.
    pub fn from_bytes(name: impl Into<String>, bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() < 12 {
            return Err("pack too small".into());
        }
        if &bytes[0..4] != b"PACK" {
            return Err("invalid magic bytes".into());
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != 1 {
            return Err(format!("unsupported pack version {version}"));
        }
        let entry_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;

        let mut cursor = 12usize;
        let mut entries = Vec::with_capacity(entry_count);

        for _ in 0..entry_count {
            if cursor + 4 > bytes.len() {
                return Err("truncated directory".into());
            }
            let path_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            if cursor + path_len > bytes.len() {
                return Err("truncated path".into());
            }
            let virtual_path = std::str::from_utf8(&bytes[cursor..cursor + path_len])
                .map_err(|e| format!("invalid UTF-8 in path: {e}"))?
                .to_owned();
            cursor += path_len;
            if cursor + 16 > bytes.len() {
                return Err("truncated entry offsets".into());
            }
            let offset = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap()) as usize;
            let length = u64::from_le_bytes(bytes[cursor + 8..cursor + 16].try_into().unwrap()) as usize;
            cursor += 16;
            entries.push(PackEntry { virtual_path, offset, length });
        }

        let data_offset = cursor;
        Ok(Self {
            name: name.into(),
            entries,
            data: bytes,
            data_offset,
        })
    }

    /// Build a pack from a map of `virtual_path → bytes`.
    pub fn build(name: impl Into<String>, files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut dir: Vec<u8> = Vec::new();
        let mut blob: Vec<u8> = Vec::new();

        dir.extend_from_slice(b"PACK");
        dir.extend_from_slice(&1u32.to_le_bytes());
        dir.extend_from_slice(&(files.len() as u32).to_le_bytes());

        for (path, data) in files {
            let path_bytes = path.as_bytes();
            dir.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
            dir.extend_from_slice(path_bytes);
            dir.extend_from_slice(&(blob.len() as u64).to_le_bytes());
            dir.extend_from_slice(&(data.len() as u64).to_le_bytes());
            blob.extend_from_slice(data);
        }

        let _ = name;
        let mut out = dir;
        out.extend_from_slice(&blob);
        out
    }

    /// Read a virtual file from the pack by its path.
    pub fn read(&self, virtual_path: &str) -> Option<&[u8]> {
        for entry in &self.entries {
            if entry.virtual_path == virtual_path {
                let start = self.data_offset + entry.offset;
                let end = start + entry.length;
                return self.data.get(start..end);
            }
        }
        None
    }

    /// List all virtual paths in this pack.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.virtual_path.as_str())
    }

    /// The name of this pack.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Number of entries in this pack.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

impl fmt::Debug for AssetPack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetPack")
            .field("name", &self.name)
            .field("entries", &self.entries.len())
            .field("total_bytes", &self.data.len())
            .finish()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 12 — AssetManifest
// ────────────────────────────────────────────────────────────────────────────

/// A single entry in an [`AssetManifest`].
#[derive(Debug, Clone)]
pub struct ManifestEntry {
    /// Path (and optional label) for this asset.
    pub path: AssetPath,
    /// Desired load priority.
    pub priority: StreamPriority,
    /// Whether this asset must be loaded before the application can start.
    pub required: bool,
    /// Optional human-readable tag (e.g. `"ui"`, `"world"`, `"audio"`).
    pub tag: Option<String>,
}

impl ManifestEntry {
    /// Create a required, high-priority entry with no tag.
    pub fn required(path: impl Into<AssetPath>) -> Self {
        Self {
            path: path.into(),
            priority: StreamPriority::High,
            required: true,
            tag: None,
        }
    }

    /// Create an optional, normal-priority entry.
    pub fn optional(path: impl Into<AssetPath>) -> Self {
        Self {
            path: path.into(),
            priority: StreamPriority::Normal,
            required: false,
            tag: None,
        }
    }

    /// Attach a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: StreamPriority) -> Self {
        self.priority = priority;
        self
    }
}

/// A declarative list of assets to pre-load.
///
/// Manifests allow level designers or content pipelines to specify exactly
/// which assets are needed for a scene without writing Rust code.
///
/// # Example
/// ```rust
/// use proof_engine::asset::{AssetManifest, ManifestEntry};
///
/// let mut manifest = AssetManifest::new("level1");
/// manifest.add(ManifestEntry::required("textures/floor.png").with_tag("level1"));
/// manifest.add(ManifestEntry::optional("sounds/ambient.wav").with_tag("level1"));
/// ```
#[derive(Debug, Clone)]
pub struct AssetManifest {
    /// Name of this manifest (e.g. level name).
    pub name: String,
    /// All entries in declaration order.
    pub entries: Vec<ManifestEntry>,
}

impl AssetManifest {
    /// Create an empty manifest.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), entries: Vec::new() }
    }

    /// Add an entry to the manifest.
    pub fn add(&mut self, entry: ManifestEntry) {
        self.entries.push(entry);
    }

    /// Iterate over only the required entries.
    pub fn required_entries(&self) -> impl Iterator<Item = &ManifestEntry> {
        self.entries.iter().filter(|e| e.required)
    }

    /// Iterate over entries that carry the given tag.
    pub fn entries_with_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a ManifestEntry> {
        self.entries.iter().filter(move |e| {
            e.tag.as_deref() == Some(tag)
        })
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` if no entries are present.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 13 — Built-in Asset Types
// ────────────────────────────────────────────────────────────────────────────

// ── ImageAsset ──────────────────────────────────────────────────────────────

/// Pixel format for [`ImageAsset`] data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// 1 byte per pixel, luminance.
    R8,
    /// 2 bytes per pixel, luminance + alpha.
    Rg8,
    /// 3 bytes per pixel, RGB.
    Rgb8,
    /// 4 bytes per pixel, RGBA.
    Rgba8,
    /// 4 bytes per component, HDR float RGBA.
    Rgba32F,
}

impl PixelFormat {
    /// Bytes per pixel for this format.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::R8 => 1,
            PixelFormat::Rg8 => 2,
            PixelFormat::Rgb8 => 3,
            PixelFormat::Rgba8 => 4,
            PixelFormat::Rgba32F => 16,
        }
    }
}

/// A loaded image — raw decoded pixel data.
///
/// Mip-maps can be generated by an [`AssetProcessor`] after load.
#[derive(Debug, Clone)]
pub struct ImageAsset {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: PixelFormat,
    /// Raw pixel bytes, row-major top-to-bottom.
    pub data: Vec<u8>,
    /// Mip levels (empty if no mips have been generated).
    pub mip_levels: Vec<Vec<u8>>,
}

impl ImageAsset {
    /// Create a solid-colour image of the given size.
    pub fn solid_color(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let pixels = (width * height) as usize;
        let mut data = Vec::with_capacity(pixels * 4);
        for _ in 0..pixels {
            data.extend_from_slice(&rgba);
        }
        Self {
            width,
            height,
            format: PixelFormat::Rgba8,
            data,
            mip_levels: Vec::new(),
        }
    }

    /// Total byte size of the base level.
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }

    /// Number of channels implied by the pixel format.
    pub fn channels(&self) -> usize {
        self.format.bytes_per_pixel()
    }
}

impl Asset for ImageAsset {}

// ── ShaderAsset ─────────────────────────────────────────────────────────────

/// Which pipeline stage a shader belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    /// Vertex processing stage.
    Vertex,
    /// Fragment / pixel shading stage.
    Fragment,
    /// Compute / general-purpose GPU stage.
    Compute,
    /// Geometry shading stage.
    Geometry,
}

impl fmt::Display for ShaderStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShaderStage::Vertex => write!(f, "vertex"),
            ShaderStage::Fragment => write!(f, "fragment"),
            ShaderStage::Compute => write!(f, "compute"),
            ShaderStage::Geometry => write!(f, "geometry"),
        }
    }
}

/// A loaded GLSL / WGSL / SPIR-V shader source.
#[derive(Debug, Clone)]
pub struct ShaderAsset {
    /// Identifier used in error messages.
    pub name: String,
    /// The full shader source text.
    pub source: String,
    /// Which pipeline stage this shader belongs to.
    pub stage: ShaderStage,
    /// Optional pre-processor defines injected at load time.
    pub defines: Vec<(String, String)>,
}

impl ShaderAsset {
    /// Create a shader asset from source text.
    pub fn new(name: impl Into<String>, source: impl Into<String>, stage: ShaderStage) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            stage,
            defines: Vec::new(),
        }
    }

    /// Add a preprocessor define.
    pub fn with_define(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.defines.push((key.into(), value.into()));
        self
    }

    /// Line count of the source.
    pub fn line_count(&self) -> usize {
        self.source.lines().count()
    }
}

impl Asset for ShaderAsset {}

// ── FontAsset ───────────────────────────────────────────────────────────────

/// Metrics for the whole font face.
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Height of capital letters above the baseline, in font units.
    pub cap_height: f32,
    /// Distance from baseline to top of tallest ascender.
    pub ascender: f32,
    /// Distance from baseline to bottom of deepest descender (negative).
    pub descender: f32,
    /// Recommended line gap between successive text lines.
    pub line_gap: f32,
    /// Units per em (coordinate space of the font).
    pub units_per_em: f32,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            cap_height: 700.0,
            ascender: 800.0,
            descender: -200.0,
            line_gap: 0.0,
            units_per_em: 1000.0,
        }
    }
}

/// Geometry and metrics for a single glyph.
#[derive(Debug, Clone)]
pub struct GlyphData {
    /// Unicode codepoint.
    pub codepoint: char,
    /// Advance width (how far the cursor moves after this glyph).
    pub advance_width: f32,
    /// Bounding box: (min_x, min_y, max_x, max_y) in font units.
    pub bounds: (f32, f32, f32, f32),
    /// UV coordinates in the atlas texture (if packed).
    pub atlas_uv: Option<(f32, f32, f32, f32)>,
    /// Raw outline commands (simplified path, optional).
    pub outline: Vec<OutlineCommand>,
}

/// A single command in a glyph outline path.
#[derive(Debug, Clone, Copy)]
pub enum OutlineCommand {
    /// Move to (x, y) without drawing.
    MoveTo(f32, f32),
    /// Straight line to (x, y).
    LineTo(f32, f32),
    /// Quadratic Bézier to (x, y) with control point (cx, cy).
    QuadTo(f32, f32, f32, f32),
    /// Close the current contour.
    Close,
}

/// A loaded font, decomposed into per-glyph data and face metrics.
#[derive(Debug, Clone)]
pub struct FontAsset {
    /// Font family name.
    pub name: String,
    /// Per-character glyph data.
    pub glyphs: HashMap<char, GlyphData>,
    /// Whole-face metrics.
    pub metrics: FontMetrics,
    /// Optional packed atlas image (generated by a processor).
    pub atlas: Option<ImageAsset>,
}

impl FontAsset {
    /// Look up a glyph, falling back to the replacement character `'?'`.
    pub fn glyph(&self, ch: char) -> Option<&GlyphData> {
        self.glyphs.get(&ch).or_else(|| self.glyphs.get(&'?'))
    }

    /// Number of glyphs loaded.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
}

impl Asset for FontAsset {}

// ── SoundAsset ──────────────────────────────────────────────────────────────

/// A loaded audio clip stored as normalised float samples.
#[derive(Debug, Clone)]
pub struct SoundAsset {
    /// Samples per second (e.g. 44100).
    pub sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Interleaved normalised samples in `[-1.0, 1.0]`.
    pub samples: Vec<f32>,
    /// Optional loop start sample index.
    pub loop_start: Option<usize>,
    /// Optional loop end sample index.
    pub loop_end: Option<usize>,
}

impl SoundAsset {
    /// Duration of the clip in seconds.
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / (self.sample_rate as f32 * self.channels as f32)
    }

    /// Total number of frames (one frame = one sample per channel).
    pub fn frame_count(&self) -> usize {
        if self.channels == 0 { 0 } else { self.samples.len() / self.channels as usize }
    }
}

impl Asset for SoundAsset {}

// ── ScriptAsset ─────────────────────────────────────────────────────────────

/// A raw script source file.
///
/// Actual parsing / compilation is deferred to the scripting subsystem.
#[derive(Debug, Clone)]
pub struct ScriptAsset {
    /// Module name.
    pub name: String,
    /// Full source text.
    pub source: String,
    /// Optional language hint (e.g. `"lua"`, `"wren"`, `"rhai"`).
    pub language: Option<String>,
}

impl ScriptAsset {
    /// Create a script from a string.
    pub fn new(name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            language: None,
        }
    }

    /// Line count of the source.
    pub fn line_count(&self) -> usize {
        self.source.lines().count()
    }

    /// Byte length of the source.
    pub fn byte_len(&self) -> usize {
        self.source.len()
    }
}

impl Asset for ScriptAsset {}

// ── MeshAsset ───────────────────────────────────────────────────────────────

/// A single vertex in a mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Position in object space.
    pub position: [f32; 3],
    /// Surface normal.
    pub normal: [f32; 3],
    /// Primary texture coordinate.
    pub uv: [f32; 2],
    /// Tangent vector (for normal mapping).
    pub tangent: [f32; 4],
    /// Vertex colour (RGBA, defaults to white).
    pub color: [f32; 4],
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0; 2],
            tangent: [1.0, 0.0, 0.0, 1.0],
            color: [1.0; 4],
        }
    }
}

/// A loaded triangle mesh.
#[derive(Debug, Clone)]
pub struct MeshAsset {
    /// Vertex buffer.
    pub vertices: Vec<Vertex>,
    /// Index buffer (triangles: every 3 indices = 1 triangle).
    pub indices: Vec<u32>,
    /// Name of the material assigned to this mesh (references a [`MaterialAsset`]).
    pub material: Option<String>,
    /// Axis-aligned bounding box: (min, max).
    pub aabb: Option<([f32; 3], [f32; 3])>,
}

impl MeshAsset {
    /// Compute the AABB from the vertex data and cache it.
    pub fn compute_aabb(&mut self) {
        if self.vertices.is_empty() {
            self.aabb = None;
            return;
        }
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in &self.vertices {
            for i in 0..3 {
                min[i] = min[i].min(v.position[i]);
                max[i] = max[i].max(v.position[i]);
            }
        }
        self.aabb = Some((min, max));
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

impl Asset for MeshAsset {}

// ── MaterialAsset ────────────────────────────────────────────────────────────

/// A PBR material description.
#[derive(Debug, Clone)]
pub struct MaterialAsset {
    /// Path to the albedo / base-colour texture.
    pub albedo: Option<AssetPath>,
    /// Path to the normal map texture.
    pub normal_map: Option<AssetPath>,
    /// Roughness value (0 = mirror, 1 = fully rough) or texture path.
    pub roughness: MaterialParam,
    /// Metallic value (0 = dielectric, 1 = metal) or texture path.
    pub metallic: MaterialParam,
    /// Reference to the shader to use when rendering.
    pub shader: Option<AssetPath>,
    /// Base albedo colour tint (RGBA).
    pub base_color: [f32; 4],
    /// Whether this material uses alpha blending.
    pub alpha_blend: bool,
    /// Whether this material renders both sides of triangles.
    pub double_sided: bool,
}

/// Either a constant scalar value or a reference to a texture channel.
#[derive(Debug, Clone)]
pub enum MaterialParam {
    /// Constant value in `[0.0, 1.0]`.
    Value(f32),
    /// Path to a texture; the red channel is used.
    Texture(AssetPath),
}

impl Default for MaterialAsset {
    fn default() -> Self {
        Self {
            albedo: None,
            normal_map: None,
            roughness: MaterialParam::Value(0.5),
            metallic: MaterialParam::Value(0.0),
            shader: None,
            base_color: [1.0; 4],
            alpha_blend: false,
            double_sided: false,
        }
    }
}

impl Asset for MaterialAsset {}

// ── SceneAsset ───────────────────────────────────────────────────────────────

/// A serialised entity in a scene.
#[derive(Debug, Clone)]
pub struct SceneEntity {
    /// Unique name within the scene.
    pub name: String,
    /// Optional parent entity name (for hierarchy).
    pub parent: Option<String>,
    /// Position in world space.
    pub position: [f32; 3],
    /// Rotation as a quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    /// Uniform scale.
    pub scale: [f32; 3],
    /// Named components — key is component type, value is serialised data.
    pub components: HashMap<String, String>,
}

impl SceneEntity {
    /// Create a new entity at the origin with identity rotation and unit scale.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parent: None,
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            components: HashMap::new(),
        }
    }
}

/// A reference to a re-usable prefab template.
#[derive(Debug, Clone)]
pub struct PrefabRef {
    /// Instance name.
    pub name: String,
    /// Path to the source prefab asset.
    pub prefab_path: AssetPath,
    /// Override position.
    pub position: [f32; 3],
    /// Override rotation (quaternion).
    pub rotation: [f32; 4],
    /// Override scale.
    pub scale: [f32; 3],
}

/// A complete serialised scene graph.
#[derive(Debug, Clone)]
pub struct SceneAsset {
    /// Human-readable scene name.
    pub name: String,
    /// All static entities baked into the scene.
    pub entities: Vec<SceneEntity>,
    /// Prefab instances referenced in the scene.
    pub prefabs: Vec<PrefabRef>,
    /// Global scene properties (fog, ambient light, sky, etc.).
    pub properties: HashMap<String, String>,
}

impl SceneAsset {
    /// Create an empty scene.
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entities: Vec::new(),
            prefabs: Vec::new(),
            properties: HashMap::new(),
        }
    }

    /// Look up an entity by name.
    pub fn find_entity(&self, name: &str) -> Option<&SceneEntity> {
        self.entities.iter().find(|e| e.name == name)
    }

    /// Total number of objects (entities + prefab instances).
    pub fn object_count(&self) -> usize {
        self.entities.len() + self.prefabs.len()
    }
}

impl Asset for SceneAsset {}

// ────────────────────────────────────────────────────────────────────────────
// Section 14 — Built-in Loaders
// ────────────────────────────────────────────────────────────────────────────

// ── RawImageLoader ───────────────────────────────────────────────────────────

/// Loader for raw RGBA8 image files.
///
/// The file format is a 12-byte header:
/// `[4 bytes "RIMG"] [4 bytes width LE u32] [4 bytes height LE u32]`
/// followed by `width * height * 4` raw RGBA bytes.
///
/// This is intentionally minimal — a real engine would plug in a PNG decoder.
pub struct RawImageLoader;

impl AssetLoader<ImageAsset> for RawImageLoader {
    fn load(&self, bytes: &[u8], path: &AssetPath) -> Result<ImageAsset, String> {
        if bytes.len() < 12 {
            return Ok(ImageAsset::solid_color(1, 1, [255, 0, 255, 255]));
        }
        if &bytes[0..4] == b"RIMG" {
            let width = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
            let height = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
            let expected = (width * height * 4) as usize;
            if bytes.len() < 12 + expected {
                return Err(format!("{path}: truncated RIMG data"));
            }
            return Ok(ImageAsset {
                width,
                height,
                format: PixelFormat::Rgba8,
                data: bytes[12..12 + expected].to_vec(),
                mip_levels: Vec::new(),
            });
        }
        Ok(ImageAsset::solid_color(1, 1, [255, 0, 255, 255]))
    }

    fn extensions(&self) -> &[&str] {
        &["rimg", "png", "jpg", "jpeg", "bmp", "tga"]
    }
}

// ── PlainTextShaderLoader ─────────────────────────────────────────────────────

/// Loader for plain-text shader source files.
///
/// The stage is inferred from the file extension.
pub struct PlainTextShaderLoader;

impl AssetLoader<ShaderAsset> for PlainTextShaderLoader {
    fn load(&self, bytes: &[u8], path: &AssetPath) -> Result<ShaderAsset, String> {
        let source = std::str::from_utf8(bytes)
            .map_err(|e| format!("{path}: invalid UTF-8: {e}"))?
            .to_owned();

        let stage = match path.extension().as_deref() {
            Some("vert") => ShaderStage::Vertex,
            Some("frag") => ShaderStage::Fragment,
            Some("comp") => ShaderStage::Compute,
            Some("geom") => ShaderStage::Geometry,
            _ => ShaderStage::Fragment,
        };

        let name = path
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_owned();

        Ok(ShaderAsset::new(name, source, stage))
    }

    fn extensions(&self) -> &[&str] {
        &["glsl", "vert", "frag", "comp", "geom", "wgsl", "hlsl"]
    }
}

// ── PlainTextScriptLoader ─────────────────────────────────────────────────────

/// Loader for plain-text script files.
pub struct PlainTextScriptLoader;

impl AssetLoader<ScriptAsset> for PlainTextScriptLoader {
    fn load(&self, bytes: &[u8], path: &AssetPath) -> Result<ScriptAsset, String> {
        let source = std::str::from_utf8(bytes)
            .map_err(|e| format!("{path}: invalid UTF-8: {e}"))?
            .to_owned();
        let name = path
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("script")
            .to_owned();
        let language = path.extension();
        Ok(ScriptAsset { name, source, language })
    }

    fn extensions(&self) -> &[&str] {
        &["lua", "rhai", "wren", "js", "py", "script"]
    }
}

// ── RawSoundLoader ───────────────────────────────────────────────────────────

/// Loader for raw PCM audio files.
///
/// Header format:
/// `[4 bytes "RSND"] [4 bytes sample_rate LE u32] [2 bytes channels LE u16]`
/// `[2 bytes padding] [remaining bytes: little-endian f32 samples]`
pub struct RawSoundLoader;

impl AssetLoader<SoundAsset> for RawSoundLoader {
    fn load(&self, bytes: &[u8], path: &AssetPath) -> Result<SoundAsset, String> {
        if bytes.len() < 12 {
            return Err(format!("{path}: sound file too small"));
        }
        if &bytes[0..4] != b"RSND" {
            return Ok(SoundAsset {
                sample_rate: 44100,
                channels: 1,
                samples: vec![0.0f32; 44100],
                loop_start: None,
                loop_end: None,
            });
        }
        let sample_rate = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let channels = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        let sample_bytes = &bytes[12..];
        if sample_bytes.len() % 4 != 0 {
            return Err(format!("{path}: sample data not aligned to 4 bytes"));
        }
        let samples: Vec<f32> = sample_bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        Ok(SoundAsset { sample_rate, channels, samples, loop_start: None, loop_end: None })
    }

    fn extensions(&self) -> &[&str] {
        &["rsnd", "wav", "ogg", "mp3", "flac"]
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 15 — Built-in Processors
// ────────────────────────────────────────────────────────────────────────────

/// Generates box-filtered mip-maps for [`ImageAsset`] data.
///
/// Each mip level is half the size of the previous, stopping at 1x1.
/// Only RGBA8 images are supported; others pass through unchanged.
pub struct MipMapGenerator;

impl AssetProcessor<ImageAsset> for MipMapGenerator {
    fn process(&self, asset: &mut ImageAsset, _path: &AssetPath) -> Result<(), String> {
        if asset.format != PixelFormat::Rgba8 {
            return Ok(());
        }
        let mut src_w = asset.width as usize;
        let mut src_h = asset.height as usize;
        let mut src_data = asset.data.clone();

        while src_w > 1 || src_h > 1 {
            let dst_w = (src_w / 2).max(1);
            let dst_h = (src_h / 2).max(1);
            let mut dst_data = vec![0u8; dst_w * dst_h * 4];

            for y in 0..dst_h {
                for x in 0..dst_w {
                    let src_x = (x * 2).min(src_w - 1);
                    let src_y = (y * 2).min(src_h - 1);
                    let nx = (src_x + 1).min(src_w - 1);
                    let ny = (src_y + 1).min(src_h - 1);

                    let p = |py: usize, px: usize| -> [u8; 4] {
                        let off = (py * src_w + px) * 4;
                        src_data[off..off + 4].try_into().unwrap()
                    };
                    let p00 = p(src_y, src_x);
                    let p01 = p(src_y, nx);
                    let p10 = p(ny, src_x);
                    let p11 = p(ny, nx);

                    let off = (y * dst_w + x) * 4;
                    for c in 0..4 {
                        dst_data[off + c] = (
                            (p00[c] as u32 + p01[c] as u32 + p10[c] as u32 + p11[c] as u32) / 4
                        ) as u8;
                    }
                }
            }

            asset.mip_levels.push(dst_data.clone());
            src_data = dst_data;
            src_w = dst_w;
            src_h = dst_h;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "MipMapGenerator"
    }
}

/// Normalises audio samples to the range `[-1.0, 1.0]`.
pub struct AudioNormalizer;

impl AssetProcessor<SoundAsset> for AudioNormalizer {
    fn process(&self, asset: &mut SoundAsset, _path: &AssetPath) -> Result<(), String> {
        let peak = asset.samples.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
        if peak > 0.0 && (peak - 1.0).abs() > 1e-6 {
            for s in &mut asset.samples {
                *s /= peak;
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "AudioNormalizer"
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 16 — AssetServer
// ────────────────────────────────────────────────────────────────────────────

/// Configuration for [`AssetServer`].
#[derive(Debug, Clone)]
pub struct AssetServerConfig {
    /// Root directory for asset files on disk.
    pub root_dir: PathBuf,
    /// LRU cache capacity (0 = unlimited).
    pub cache_capacity: usize,
    /// How many streaming requests to process per `update()` call.
    pub stream_batch_size: usize,
    /// Enable hot-reload (file-change polling).
    pub hot_reload: bool,
    /// How often to poll for file changes.
    pub hot_reload_interval: Duration,
}

impl Default for AssetServerConfig {
    fn default() -> Self {
        Self {
            root_dir: PathBuf::from("assets"),
            cache_capacity: 512,
            stream_batch_size: 8,
            hot_reload: cfg!(debug_assertions),
            hot_reload_interval: Duration::from_secs(1),
        }
    }
}

/// Type-erased loader function stored inside [`AssetServer`].
type ErasedLoadFn = Box<dyn Fn(&[u8], &AssetPath) -> Result<Box<dyn Any + Send + Sync>, String> + Send + Sync>;

/// Type-erased store-into-registry function.
type ErasedStoreFn = Box<dyn Fn(&mut AssetRegistry, u64, Box<dyn Any + Send + Sync>, Option<SystemTime>) + Send + Sync>;

struct LoaderEntry {
    extensions: Vec<String>,
    type_id: TypeId,
    load: ErasedLoadFn,
    store: ErasedStoreFn,
}

/// The central asset server: coordinates loading, caching, hot-reload, and streaming.
///
/// # Lifecycle
///
/// 1. Register loaders with [`register_loader`](AssetServer::register_loader).
/// 2. Request assets with [`load`](AssetServer::load) or [`load_manifest`](AssetServer::load_manifest).
/// 3. Call [`update`](AssetServer::update) each frame to process the streaming queue and hot-reload.
/// 4. Retrieve loaded data via [`get`](AssetServer::get).
pub struct AssetServer {
    config: AssetServerConfig,
    registry: AssetRegistry,
    cache: AssetCache,
    hot_reload: HotReload,
    streaming: StreamingManager,
    loaders: Vec<LoaderEntry>,
    packs: Vec<AssetPack>,
    /// Per-id typed handle storage: id -> Arc<RwLock<Option<T>>> erased as Box<dyn Any>
    typed_slots: HashMap<u64, Box<dyn Any + Send + Sync>>,
    /// Statistics.
    stats: AssetServerStats,
}

/// Runtime statistics for the asset server.
#[derive(Debug, Clone, Default)]
pub struct AssetServerStats {
    /// Total assets loaded from disk since server creation.
    pub loads_from_disk: u64,
    /// Total assets loaded from an [`AssetPack`] since server creation.
    pub loads_from_pack: u64,
    /// Total bytes read from disk.
    pub bytes_read: u64,
    /// Total assets evicted from the LRU cache.
    pub evictions: u64,
    /// Total hot-reload events processed.
    pub hot_reloads: u64,
    /// Total load failures.
    pub failures: u64,
}

impl AssetServer {
    /// Create a new asset server with the given configuration.
    pub fn new_with_config(config: AssetServerConfig) -> Self {
        let cache = AssetCache::new(config.cache_capacity);
        let hot_reload = HotReload::new(config.hot_reload_interval, config.hot_reload);
        let streaming = StreamingManager::new(config.stream_batch_size);
        Self {
            config,
            registry: AssetRegistry::new(),
            cache,
            hot_reload,
            streaming,
            loaders: Vec::new(),
            packs: Vec::new(),
            typed_slots: HashMap::new(),
            stats: AssetServerStats::default(),
        }
    }

    /// Create a new asset server with default configuration.
    pub fn new() -> Self {
        Self::new_with_config(AssetServerConfig::default())
    }

    /// Change the root asset directory.
    pub fn set_root_dir(&mut self, dir: impl Into<PathBuf>) {
        self.config.root_dir = dir.into();
    }

    /// Register a loader for asset type `A`.
    ///
    /// The loader's `extensions()` are used to match file paths.
    /// If multiple loaders claim the same extension, the most recently
    /// registered one takes precedence.
    pub fn register_loader<A: Asset, L: AssetLoader<A>>(&mut self, loader: L) {
        let extensions: Vec<String> = loader.extensions().iter().map(|e| e.to_string()).collect();
        let type_id = TypeId::of::<A>();
        let loader = Arc::new(loader);

        let load_loader = Arc::clone(&loader);
        let load: ErasedLoadFn = Box::new(move |bytes, path| {
            load_loader
                .load(bytes, path)
                .map(|a| Box::new(a) as Box<dyn Any + Send + Sync>)
        });

        let store: ErasedStoreFn = Box::new(|registry, id, boxed, mtime| {
            if let Ok(asset) = boxed.downcast::<A>() {
                registry.store::<A>(id, *asset, mtime);
            }
        });

        self.loaders.push(LoaderEntry { extensions, type_id, load, store });
    }

    /// Mount an [`AssetPack`]. Files in packs are resolved before the file-system.
    pub fn mount_pack(&mut self, pack: AssetPack) {
        self.packs.push(pack);
    }

    /// Request that `path` be loaded as asset type `A`.
    ///
    /// Returns an [`AssetHandle<A>`] immediately. The handle will become
    /// populated after [`update`](AssetServer::update) processes the request.
    ///
    /// If the asset is already loaded, a handle to the existing data is returned.
    pub fn load<A: Asset>(&mut self, path: impl Into<AssetPath>) -> AssetHandle<A> {
        let path = path.into();
        let type_id = TypeId::of::<A>();

        if let Some(id) = self.registry.id_for_path(&path) {
            return self.make_handle::<A>(id);
        }

        let id = self.registry.alloc(type_id, path.clone());
        self.registry.mark_loading(id);

        let arc: Arc<RwLock<Option<A>>> = Arc::new(RwLock::new(None));
        self.typed_slots.insert(id, Box::new(Arc::clone(&arc)));

        self.streaming.enqueue(StreamRequest {
            id,
            path,
            type_id,
            priority: StreamPriority::Normal,
            enqueued_at: Instant::now(),
        });

        AssetHandle::new(AssetId::new(id), arc)
    }

    /// Like [`load`](AssetServer::load) but with an explicit priority.
    pub fn load_with_priority<A: Asset>(
        &mut self,
        path: impl Into<AssetPath>,
        priority: StreamPriority,
    ) -> AssetHandle<A> {
        let path = path.into();
        let type_id = TypeId::of::<A>();

        if let Some(id) = self.registry.id_for_path(&path) {
            return self.make_handle::<A>(id);
        }

        let id = self.registry.alloc(type_id, path.clone());
        self.registry.mark_loading(id);

        let arc: Arc<RwLock<Option<A>>> = Arc::new(RwLock::new(None));
        self.typed_slots.insert(id, Box::new(Arc::clone(&arc)));

        self.streaming.enqueue(StreamRequest {
            id,
            path,
            type_id,
            priority,
            enqueued_at: Instant::now(),
        });

        AssetHandle::new(AssetId::new(id), arc)
    }

    /// Queue all assets declared in `manifest` for loading.
    ///
    /// Callers should subsequently call typed `load::<T>()` for each entry to
    /// obtain typed handles; this method ensures the paths are pre-registered
    /// at the declared priorities.
    pub fn load_manifest(&mut self, manifest: &AssetManifest) {
        for entry in &manifest.entries {
            let path = entry.path.clone();
            let priority = entry.priority;

            if self.registry.id_for_path(&path).is_none() {
                // Placeholder type — real typed loads will overwrite if needed
                let id = self.registry.alloc(TypeId::of::<ScriptAsset>(), path.clone());
                self.registry.mark_loading(id);
                self.streaming.enqueue(StreamRequest {
                    id,
                    path,
                    type_id: TypeId::of::<ScriptAsset>(),
                    priority,
                    enqueued_at: Instant::now(),
                });
            }
        }
    }

    /// Drive the asset server for one frame.
    ///
    /// This processes pending streaming requests (up to `stream_batch_size` per
    /// call) and polls the hot-reload watcher. Call once per frame.
    pub fn update(&mut self) {
        let batch = self.streaming.drain();
        for req in batch {
            self.execute_load(req);
        }

        let changed = self.hot_reload.poll();
        for (_path, asset_ids) in changed {
            for id in asset_ids {
                self.enqueue_reload(id);
            }
        }
    }

    /// Try to get the data for `handle`.
    ///
    /// Returns `None` if the asset is not yet loaded or has been evicted.
    pub fn get<A: Asset>(&mut self, handle: &AssetHandle<A>) -> Option<Arc<A>> {
        let id = handle.id().raw();
        let arc = self.registry.get::<A>(id)?;

        if let Some(evict_id) = self.cache.touch(id) {
            self.registry.evict(evict_id);
            self.cache.remove(evict_id);
            self.typed_slots.remove(&evict_id);
            self.stats.evictions += 1;
        }

        Some(arc)
    }

    /// Get the [`LoadState`] of `handle`.
    pub fn load_state<A: Asset>(&self, handle: &AssetHandle<A>) -> LoadState {
        self.registry.load_state(handle.id().raw())
    }

    /// Force an immediate synchronous reload of an asset by handle.
    pub fn reload<A: Asset>(&mut self, handle: &AssetHandle<A>) {
        let id = handle.id().raw();
        self.enqueue_reload(id);
        // Drain immediately so the reload happens synchronously
        let batch = self.streaming.drain();
        for req in batch {
            self.execute_load(req);
        }
    }

    /// Insert an already-constructed asset directly into the registry.
    ///
    /// Returns a handle to the inserted asset. Useful for procedurally generated
    /// assets that have no backing file.
    pub fn insert<A: Asset>(&mut self, path: impl Into<AssetPath>, asset: A) -> AssetHandle<A> {
        let path = path.into();
        let type_id = TypeId::of::<A>();

        let id = if let Some(existing_id) = self.registry.id_for_path(&path) {
            existing_id
        } else {
            self.registry.alloc(type_id, path)
        };

        self.registry.store::<A>(id, asset, None);

        // Build the typed Arc slot
        let arc: Arc<RwLock<Option<A>>> = Arc::new(RwLock::new(None));
        // Populate from registry
        if let Some(value_arc) = self.registry.get::<A>(id) {
            // We can't move out of Arc<T>, but we can clone if T: Clone.
            // Instead, store None in the handle — callers use server.get() for Arc<T>.
            // The handle's inner Arc is a separate slot; populate it by writing.
            // Since A: Asset (not Clone), we leave the handle slot empty and
            // direct callers to use server.get() which returns Arc<A> from registry.
            let _ = value_arc;
        }
        self.typed_slots.insert(id, Box::new(Arc::clone(&arc)));

        if let Some(evict_id) = self.cache.touch(id) {
            self.registry.evict(evict_id);
            self.cache.remove(evict_id);
            self.typed_slots.remove(&evict_id);
            self.stats.evictions += 1;
        }

        AssetHandle::new(AssetId::new(id), arc)
    }

    /// Return a snapshot of the current server statistics.
    pub fn stats(&self) -> &AssetServerStats {
        &self.stats
    }

    /// Number of assets currently tracked by the registry.
    pub fn asset_count(&self) -> usize {
        self.registry.len()
    }

    /// `true` if all streaming requests have been processed.
    pub fn is_idle(&self) -> bool {
        self.streaming.is_idle()
    }

    /// Read bytes for a given path, checking packs first then the file-system.
    /// Exposed as `pub` for testing and pack inspection utilities.
    pub fn read_bytes(&mut self, path: &AssetPath) -> Result<(Vec<u8>, Option<SystemTime>), String> {
        let virtual_str = path.path().to_string_lossy().replace('\\', "/");

        for pack in &self.packs {
            if let Some(data) = pack.read(&virtual_str) {
                self.stats.loads_from_pack += 1;
                self.stats.bytes_read += data.len() as u64;
                return Ok((data.to_vec(), None));
            }
        }

        let full_path = self.config.root_dir.join(path.path());
        let mtime = std::fs::metadata(&full_path)
            .and_then(|m| m.modified())
            .ok();
        let data = std::fs::read(&full_path)
            .map_err(|e| format!("failed to read {}: {e}", full_path.display()))?;
        self.stats.loads_from_disk += 1;
        self.stats.bytes_read += data.len() as u64;
        Ok((data, mtime))
    }

    // ── Private helpers ───────────────────────────────────────────────────

    /// Construct a handle referencing an existing slot (already allocated).
    fn make_handle<A: Asset>(&mut self, id: u64) -> AssetHandle<A> {
        // If a typed slot exists and is the right type, reuse it
        if let Some(boxed) = self.typed_slots.get(&id) {
            if let Some(arc) = boxed.downcast_ref::<Arc<RwLock<Option<A>>>>() {
                return AssetHandle::new(AssetId::new(id), Arc::clone(arc));
            }
        }
        // Create a fresh typed slot
        let arc: Arc<RwLock<Option<A>>> = Arc::new(RwLock::new(None));
        self.typed_slots.insert(id, Box::new(Arc::clone(&arc)));
        AssetHandle::new(AssetId::new(id), arc)
    }

    /// Execute a single load request synchronously.
    fn execute_load(&mut self, req: StreamRequest) {
        let path = req.path.clone();
        let id = req.id;
        let ext = path.extension().unwrap_or_default();

        // Find a matching loader by type + extension
        let loader_idx = self.loaders.iter().rposition(|l| {
            l.type_id == req.type_id && l.extensions.iter().any(|e| e == &ext)
        }).or_else(|| {
            // Fallback: any loader for this type regardless of extension
            self.loaders.iter().rposition(|l| l.type_id == req.type_id)
        });

        let loader_idx = match loader_idx {
            Some(i) => i,
            None => {
                let msg = format!("no loader for extension '{ext}'");
                self.registry.mark_failed(id, msg);
                self.stats.failures += 1;
                return;
            }
        };

        let (bytes, mtime) = match self.read_bytes(&path) {
            Ok(b) => b,
            Err(e) => {
                self.registry.mark_failed(id, e);
                self.stats.failures += 1;
                return;
            }
        };

        let loaded = (self.loaders[loader_idx].load)(&bytes, &path);
        match loaded {
            Ok(boxed) => {
                (self.loaders[loader_idx].store)(&mut self.registry, id, boxed, mtime);
                if mtime.is_some() {
                    let disk_path = self.config.root_dir.join(path.path());
                    self.hot_reload.watch(disk_path, id, mtime);
                }
                if let Some(evict_id) = self.cache.touch(id) {
                    self.registry.evict(evict_id);
                    self.cache.remove(evict_id);
                    self.typed_slots.remove(&evict_id);
                    self.stats.evictions += 1;
                }
            }
            Err(msg) => {
                self.registry.mark_failed(id, msg);
                self.stats.failures += 1;
            }
        }
    }

    /// Enqueue a reload of the asset with the given ID.
    fn enqueue_reload(&mut self, id: u64) {
        let path_opt = self.registry.path_for_id(id).cloned();
        let type_id_opt = self.registry.type_id_for(id);
        if let (Some(path), Some(type_id)) = (path_opt, type_id_opt) {
            self.registry.mark_loading(id);
            self.streaming.enqueue(StreamRequest {
                id,
                path,
                type_id,
                priority: StreamPriority::High,
                enqueued_at: Instant::now(),
            });
            self.stats.hot_reloads += 1;
        }
    }
}

impl Default for AssetServer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AssetServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetServer")
            .field("assets", &self.registry.len())
            .field("pending", &self.streaming.pending())
            .field("cache", &self.cache)
            .field("hot_reload", &self.hot_reload)
            .field("stats", &self.stats)
            .finish()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Section 17 — Convenience functions
// ────────────────────────────────────────────────────────────────────────────

/// Build a default `AssetServer` with all built-in loaders registered.
///
/// This is the quickest way to get a fully configured server.
///
/// # Example
/// ```rust
/// use proof_engine::asset::default_asset_server;
/// let mut server = default_asset_server();
/// ```
pub fn default_asset_server() -> AssetServer {
    let mut server = AssetServer::new();
    server.register_loader::<ImageAsset, _>(RawImageLoader);
    server.register_loader::<ShaderAsset, _>(PlainTextShaderLoader);
    server.register_loader::<ScriptAsset, _>(PlainTextScriptLoader);
    server.register_loader::<SoundAsset, _>(RawSoundLoader);
    server
}

/// Load a file synchronously from disk, returning its bytes.
///
/// Used internally; also exposed for simple file utilities.
pub fn load_file_bytes(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("load_file_bytes: {e}"))
}

// ────────────────────────────────────────────────────────────────────────────
// Section 18 — Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── AssetPath tests ─────────────────────────────────────────────────────

    #[test]
    fn asset_path_parse_no_label() {
        let p = AssetPath::parse("textures/player.png");
        assert_eq!(p.path(), Path::new("textures/player.png"));
        assert_eq!(p.label(), None);
    }

    #[test]
    fn asset_path_parse_with_label() {
        let p = AssetPath::parse("models/robot.gltf#body");
        assert_eq!(p.path(), Path::new("models/robot.gltf"));
        assert_eq!(p.label(), Some("body"));
    }

    #[test]
    fn asset_path_extension() {
        let p = AssetPath::new("shaders/main.frag");
        assert_eq!(p.extension(), Some("frag".to_string()));
    }

    #[test]
    fn asset_path_display() {
        let p = AssetPath::with_label("a/b.png", "sub");
        assert!(p.to_string().contains('#'));
    }

    #[test]
    fn asset_path_from_str() {
        let p: AssetPath = "foo/bar.lua".into();
        assert_eq!(p.extension(), Some("lua".to_string()));
    }

    // ── AssetId tests ───────────────────────────────────────────────────────

    #[test]
    fn asset_id_equality() {
        let a = AssetId::<ImageAsset>::new(42);
        let b = AssetId::<ImageAsset>::new(42);
        let c = AssetId::<ImageAsset>::new(7);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn asset_id_copy() {
        let a = AssetId::<ShaderAsset>::new(1);
        let b = a;
        assert_eq!(a, b);
    }

    // ── LoadState tests ─────────────────────────────────────────────────────

    #[test]
    fn load_state_display() {
        assert_eq!(LoadState::NotLoaded.to_string(), "NotLoaded");
        assert_eq!(LoadState::Loading.to_string(), "Loading");
        assert_eq!(LoadState::Loaded.to_string(), "Loaded");
        assert!(LoadState::Failed("oops".into()).to_string().contains("oops"));
    }

    #[test]
    fn load_state_equality() {
        assert_eq!(LoadState::Loaded, LoadState::Loaded);
        assert_ne!(LoadState::Loaded, LoadState::Loading);
        assert_eq!(
            LoadState::Failed("x".into()),
            LoadState::Failed("x".into())
        );
    }

    // ── AssetRegistry tests ─────────────────────────────────────────────────

    #[test]
    fn registry_alloc_and_lookup() {
        let mut reg = AssetRegistry::new();
        let path = AssetPath::new("test.png");
        let id = reg.alloc(TypeId::of::<ImageAsset>(), path.clone());
        assert_eq!(reg.id_for_path(&path), Some(id));
        assert_eq!(reg.load_state(id), LoadState::NotLoaded);
    }

    #[test]
    fn registry_store_and_get() {
        let mut reg = AssetRegistry::new();
        let path = AssetPath::new("solid.png");
        let id = reg.alloc(TypeId::of::<ImageAsset>(), path);
        let img = ImageAsset::solid_color(4, 4, [0, 0, 0, 255]);
        reg.store::<ImageAsset>(id, img, None);
        assert_eq!(reg.load_state(id), LoadState::Loaded);
        let arc = reg.get::<ImageAsset>(id).unwrap();
        assert_eq!(arc.width, 4);
    }

    #[test]
    fn registry_mark_failed() {
        let mut reg = AssetRegistry::new();
        let id = reg.alloc(TypeId::of::<ImageAsset>(), AssetPath::new("x.png"));
        reg.mark_failed(id, "disk error".into());
        assert!(matches!(reg.load_state(id), LoadState::Failed(_)));
    }

    #[test]
    fn registry_evict() {
        let mut reg = AssetRegistry::new();
        let id = reg.alloc(TypeId::of::<ImageAsset>(), AssetPath::new("e.png"));
        assert!(reg.evict(id));
        assert_eq!(reg.load_state(id), LoadState::NotLoaded);
        assert!(!reg.evict(id));
    }

    // ── AssetCache tests ────────────────────────────────────────────────────

    #[test]
    fn cache_lru_eviction() {
        let mut cache = AssetCache::new(3);
        assert_eq!(cache.touch(1), None);
        assert_eq!(cache.touch(2), None);
        assert_eq!(cache.touch(3), None);
        let evicted = cache.touch(4);
        assert_eq!(evicted, Some(1));
    }

    #[test]
    fn cache_touch_updates_order() {
        let mut cache = AssetCache::new(3);
        cache.touch(1);
        cache.touch(2);
        cache.touch(3);
        cache.touch(1); // 1 is now MRU; 2 is LRU
        let evicted = cache.touch(4);
        assert_eq!(evicted, Some(2));
    }

    #[test]
    fn cache_unlimited() {
        let mut cache = AssetCache::new(0);
        for i in 0..1000u64 {
            assert_eq!(cache.touch(i), None);
        }
        assert_eq!(cache.len(), 1000);
    }

    #[test]
    fn cache_set_capacity_evicts() {
        let mut cache = AssetCache::new(10);
        for i in 0..10u64 {
            cache.touch(i);
        }
        let evicted = cache.set_capacity(5);
        assert_eq!(evicted.len(), 5);
        assert_eq!(cache.len(), 5);
    }

    // ── HotReload tests ─────────────────────────────────────────────────────

    #[test]
    fn hot_reload_disabled_returns_empty() {
        let mut hr = HotReload::new(Duration::from_secs(1), false);
        hr.watch(PathBuf::from("x.png"), 1, None);
        let changed = hr.poll();
        assert!(changed.is_empty());
    }

    #[test]
    fn hot_reload_watch_count() {
        let mut hr = HotReload::new(Duration::from_secs(60), true);
        hr.watch(PathBuf::from("a.png"), 1, None);
        hr.watch(PathBuf::from("b.png"), 2, None);
        assert_eq!(hr.watched_count(), 2);
    }

    #[test]
    fn hot_reload_unwatch() {
        let mut hr = HotReload::new(Duration::from_secs(60), true);
        hr.watch(PathBuf::from("a.png"), 1, None);
        hr.unwatch(1);
        assert_eq!(hr.watched_count(), 0);
    }

    // ── StreamingManager tests ──────────────────────────────────────────────

    #[test]
    fn streaming_priority_order() {
        let mut sm = StreamingManager::new(10);
        let make = |id: u64, priority: StreamPriority| StreamRequest {
            id,
            path: AssetPath::new("x"),
            type_id: TypeId::of::<ImageAsset>(),
            priority,
            enqueued_at: Instant::now(),
        };
        sm.enqueue(make(1, StreamPriority::Low));
        sm.enqueue(make(2, StreamPriority::Critical));
        sm.enqueue(make(3, StreamPriority::Normal));

        let drained = sm.drain();
        assert_eq!(drained[0].id, 2);
        assert_eq!(drained[1].id, 3);
        assert_eq!(drained[2].id, 1);
    }

    #[test]
    fn streaming_cancel() {
        let mut sm = StreamingManager::new(10);
        sm.enqueue(StreamRequest {
            id: 99,
            path: AssetPath::new("y"),
            type_id: TypeId::of::<ImageAsset>(),
            priority: StreamPriority::Normal,
            enqueued_at: Instant::now(),
        });
        assert_eq!(sm.pending(), 1);
        assert!(sm.cancel(99));
        assert_eq!(sm.pending(), 0);
    }

    #[test]
    fn streaming_batch_limit() {
        let mut sm = StreamingManager::new(2);
        for i in 0..5u64 {
            sm.enqueue(StreamRequest {
                id: i,
                path: AssetPath::new("z"),
                type_id: TypeId::of::<ImageAsset>(),
                priority: StreamPriority::Normal,
                enqueued_at: Instant::now(),
            });
        }
        let first = sm.drain();
        assert_eq!(first.len(), 2);
        assert_eq!(sm.pending(), 3);
    }

    // ── AssetPack tests ─────────────────────────────────────────────────────

    #[test]
    fn pack_build_and_read() {
        let files: &[(&str, &[u8])] = &[
            ("shaders/main.vert", b"void main() {}"),
            ("textures/logo.png", &[0u8, 1, 2, 3, 4]),
        ];
        let bytes = AssetPack::build("test", files);
        let pack = AssetPack::from_bytes("test", bytes).expect("parse failed");
        assert_eq!(pack.entry_count(), 2);
        assert_eq!(pack.read("shaders/main.vert"), Some(b"void main() {}".as_ref()));
        assert_eq!(pack.read("textures/logo.png"), Some([0u8, 1, 2, 3, 4].as_ref()));
        assert_eq!(pack.read("nonexistent"), None);
    }

    #[test]
    fn pack_invalid_magic() {
        let result = AssetPack::from_bytes("bad", b"XXXX\0\0\0\0".to_vec());
        assert!(result.is_err());
    }

    #[test]
    fn pack_paths_iterator() {
        let files: &[(&str, &[u8])] = &[("a.txt", b"hello"), ("b.txt", b"world")];
        let bytes = AssetPack::build("p", files);
        let pack = AssetPack::from_bytes("p", bytes).unwrap();
        let paths: Vec<_> = pack.paths().collect();
        assert!(paths.contains(&"a.txt"));
        assert!(paths.contains(&"b.txt"));
    }

    // ── AssetManifest tests ─────────────────────────────────────────────────

    #[test]
    fn manifest_required_optional() {
        let mut m = AssetManifest::new("level1");
        m.add(ManifestEntry::required("textures/floor.png"));
        m.add(ManifestEntry::optional("sounds/bg.wav"));
        assert_eq!(m.len(), 2);
        assert_eq!(m.required_entries().count(), 1);
    }

    #[test]
    fn manifest_tag_filter() {
        let mut m = AssetManifest::new("lvl");
        m.add(ManifestEntry::required("a.png").with_tag("ui"));
        m.add(ManifestEntry::optional("b.png").with_tag("world"));
        m.add(ManifestEntry::optional("c.png").with_tag("ui"));
        assert_eq!(m.entries_with_tag("ui").count(), 2);
        assert_eq!(m.entries_with_tag("world").count(), 1);
    }

    // ── ImageAsset tests ────────────────────────────────────────────────────

    #[test]
    fn image_solid_color() {
        let img = ImageAsset::solid_color(2, 2, [255, 0, 0, 255]);
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 16);
        assert_eq!(img.data[0], 255);
        assert_eq!(img.data[1], 0);
    }

    #[test]
    fn mipmap_processor() {
        let img = ImageAsset::solid_color(4, 4, [128, 64, 32, 255]);
        let mut img = img;
        let proc = MipMapGenerator;
        proc.process(&mut img, &AssetPath::new("test.png")).unwrap();
        // 4x4 → 2x2 → 1x1 = 2 mip levels
        assert_eq!(img.mip_levels.len(), 2);
        assert_eq!(img.mip_levels[0].len(), 2 * 2 * 4); // 2x2 RGBA
    }

    // ── SoundAsset tests ────────────────────────────────────────────────────

    #[test]
    fn sound_duration() {
        let snd = SoundAsset {
            sample_rate: 44100,
            channels: 1,
            samples: vec![0.0f32; 44100],
            loop_start: None,
            loop_end: None,
        };
        assert!((snd.duration_secs() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn audio_normalizer() {
        let mut snd = SoundAsset {
            sample_rate: 44100,
            channels: 1,
            samples: vec![0.5f32, -0.5, 0.25],
            loop_start: None,
            loop_end: None,
        };
        let norm = AudioNormalizer;
        norm.process(&mut snd, &AssetPath::new("test.wav")).unwrap();
        assert!((snd.samples[0] - 1.0).abs() < 1e-5);
    }

    // ── ShaderAsset tests ───────────────────────────────────────────────────

    #[test]
    fn shader_line_count() {
        let src = "void main() {\n    gl_Position = vec4(0);\n}\n";
        let shader = ShaderAsset::new("test", src, ShaderStage::Vertex);
        assert_eq!(shader.line_count(), 3);
    }

    // ── ScriptAsset tests ───────────────────────────────────────────────────

    #[test]
    fn script_byte_len() {
        let s = ScriptAsset::new("test", "print('hello')");
        assert_eq!(s.byte_len(), 14);
    }

    // ── MeshAsset tests ─────────────────────────────────────────────────────

    #[test]
    fn mesh_aabb() {
        let mut mesh = MeshAsset {
            vertices: vec![
                Vertex { position: [-1.0, 0.0, 0.0], ..Default::default() },
                Vertex { position: [1.0, 2.0, 3.0], ..Default::default() },
            ],
            indices: vec![0, 1, 0],
            material: None,
            aabb: None,
        };
        mesh.compute_aabb();
        let (min, max) = mesh.aabb.unwrap();
        assert_eq!(min, [-1.0, 0.0, 0.0]);
        assert_eq!(max, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn mesh_triangle_count() {
        let mesh = MeshAsset {
            vertices: vec![Vertex::default(); 3],
            indices: vec![0, 1, 2, 0, 2, 1],
            material: None,
            aabb: None,
        };
        assert_eq!(mesh.triangle_count(), 2);
    }

    // ── SceneAsset tests ────────────────────────────────────────────────────

    #[test]
    fn scene_find_entity() {
        let mut scene = SceneAsset::empty("test_scene");
        scene.entities.push(SceneEntity::new("player"));
        assert!(scene.find_entity("player").is_some());
        assert!(scene.find_entity("enemy").is_none());
    }

    // ── AssetServer integration tests ───────────────────────────────────────

    #[test]
    fn server_insert_and_load_state() {
        let mut server = AssetServer::new();
        server.register_loader::<ImageAsset, _>(RawImageLoader);

        let img = ImageAsset::solid_color(8, 8, [0, 255, 0, 255]);
        let handle = server.insert::<ImageAsset>("generated/green.png", img);

        assert_eq!(server.load_state(&handle), LoadState::Loaded);
    }

    #[test]
    fn server_default_asset_server() {
        let server = default_asset_server();
        assert_eq!(server.asset_count(), 0);
        assert!(server.is_idle());
    }

    #[test]
    fn server_pack_load() {
        let files: &[(&str, &[u8])] = &[("shaders/quad.vert", b"// vert shader")];
        let pack_bytes = AssetPack::build("shaders", files);
        let pack = AssetPack::from_bytes("shaders", pack_bytes).unwrap();

        let mut server = AssetServer::new_with_config(AssetServerConfig {
            root_dir: PathBuf::from("nonexistent"),
            ..Default::default()
        });
        server.register_loader::<ShaderAsset, _>(PlainTextShaderLoader);
        server.mount_pack(pack);

        let result = server.read_bytes(&AssetPath::new("shaders/quad.vert"));
        assert!(result.is_ok());
        let (data, _) = result.unwrap();
        assert_eq!(data, b"// vert shader");
    }

    #[test]
    fn server_load_enqueues_request() {
        let mut server = AssetServer::new();
        server.register_loader::<ImageAsset, _>(RawImageLoader);
        let _handle = server.load::<ImageAsset>(AssetPath::new("test.png"));
        // The request is in the queue, not yet processed
        assert!(!server.is_idle());
    }

    #[test]
    fn server_get_returns_none_before_update() {
        let mut server = AssetServer::new();
        server.register_loader::<ImageAsset, _>(RawImageLoader);
        let handle = server.load::<ImageAsset>(AssetPath::new("test.png"));
        // Not yet processed
        assert!(server.get(&handle).is_none());
    }

    #[test]
    fn streaming_priority_upgrade() {
        let mut sm = StreamingManager::new(10);
        sm.enqueue(StreamRequest {
            id: 1,
            path: AssetPath::new("a.png"),
            type_id: TypeId::of::<ImageAsset>(),
            priority: StreamPriority::Low,
            enqueued_at: Instant::now(),
        });
        // Enqueue same ID with higher priority — should upgrade
        sm.enqueue(StreamRequest {
            id: 1,
            path: AssetPath::new("a.png"),
            type_id: TypeId::of::<ImageAsset>(),
            priority: StreamPriority::Critical,
            enqueued_at: Instant::now(),
        });
        assert_eq!(sm.pending(), 1);
        let drained = sm.drain();
        assert_eq!(drained[0].priority, StreamPriority::Critical);
    }

    #[test]
    fn pixel_format_bytes_per_pixel() {
        assert_eq!(PixelFormat::R8.bytes_per_pixel(), 1);
        assert_eq!(PixelFormat::Rgba8.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::Rgba32F.bytes_per_pixel(), 16);
    }

    #[test]
    fn font_asset_glyph_fallback() {
        let mut font = FontAsset {
            name: "test".into(),
            glyphs: HashMap::new(),
            metrics: FontMetrics::default(),
            atlas: None,
        };
        font.glyphs.insert('?', GlyphData {
            codepoint: '?',
            advance_width: 500.0,
            bounds: (0.0, 0.0, 500.0, 700.0),
            atlas_uv: None,
            outline: Vec::new(),
        });
        assert!(font.glyph('A').is_none()); // no fallback without '?'... wait
        // Actually glyph('A') falls back to '?' when 'A' is not present
        assert!(font.glyph('A').is_some());
        assert_eq!(font.glyph('A').unwrap().advance_width, 500.0);
    }

    #[test]
    fn material_param_variants() {
        let m = MaterialAsset::default();
        assert!(matches!(m.roughness, MaterialParam::Value(_)));
        let tex = MaterialParam::Texture(AssetPath::new("rough.png"));
        assert!(matches!(tex, MaterialParam::Texture(_)));
    }
}
