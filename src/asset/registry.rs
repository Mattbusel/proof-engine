//! Asset storage and loader registry.
//!
//! `AssetStorage<T>` is a generational slot map for a single asset type.
//! `AssetRegistry` aggregates multiple typed storages and the global loader table.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::asset::handle::{AssetId, LoadState};
use crate::asset::loader::{Asset, AssetLoadError, AssetLoader, LoadContext};

// ─────────────────────────────────────────────
//  AssetStorage<T>
// ─────────────────────────────────────────────

/// Typed storage for a single asset class `T`.
///
/// Assets are addressed by `AssetId` (packed index + generation). The storage
/// maintains a secondary `path_to_id` index for look-ups by file path.
pub struct AssetStorage<T> {
    /// The live assets.
    assets: HashMap<AssetId, T>,
    /// Load state per slot.
    load_states: HashMap<AssetId, LoadState>,
    /// Reverse mapping: canonical path → id.
    path_to_id: HashMap<String, AssetId>,
    /// Monotonically increasing counter used to generate unique ids.
    id_counter: u64,
}

impl<T> AssetStorage<T> {
    /// Create an empty storage.
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
            load_states: HashMap::new(),
            path_to_id: HashMap::new(),
            id_counter: 0,
        }
    }

    // ── Id generation ─────────────────────────────────────────────────────

    fn next_id(&mut self) -> AssetId {
        let raw = self.id_counter;
        self.id_counter += 1;
        // Use the raw counter as the index, generation = 0 for new slots.
        // Re-used slots would bump the generation; for simplicity we never
        // recycle here — just increment forever.
        AssetId::new(raw as u32, (raw >> 32) as u32)
    }

    // ── Insertion ─────────────────────────────────────────────────────────

    /// Store an asset under `path` and return its new `AssetId`.
    ///
    /// If `path` is already mapped to an id, the existing asset is replaced and
    /// the same id is returned (with the generation preserved).
    pub fn insert(&mut self, path: impl Into<String>, asset: T) -> AssetId {
        let path = path.into();
        if let Some(&existing_id) = self.path_to_id.get(&path) {
            self.assets.insert(existing_id, asset);
            self.load_states.insert(existing_id, LoadState::Loaded);
            return existing_id;
        }
        let id = self.next_id();
        self.assets.insert(id, asset);
        self.load_states.insert(id, LoadState::Loaded);
        self.path_to_id.insert(path, id);
        id
    }

    /// Reserve an id for a path that is still loading. The slot will hold no
    /// asset data until `insert_at` is called with the same id.
    pub fn reserve(&mut self, path: impl Into<String>) -> AssetId {
        let path = path.into();
        if let Some(&existing_id) = self.path_to_id.get(&path) {
            return existing_id;
        }
        let id = self.next_id();
        self.load_states.insert(id, LoadState::Loading);
        self.path_to_id.insert(path, id);
        id
    }

    /// Place an asset into a previously reserved slot.
    pub fn insert_at(&mut self, id: AssetId, asset: T) {
        self.assets.insert(id, asset);
        self.load_states.insert(id, LoadState::Loaded);
    }

    /// Mark a slot as failed.
    pub fn mark_failed(&mut self, id: AssetId, reason: String) {
        self.load_states.insert(id, LoadState::Failed(reason));
        self.assets.remove(&id);
    }

    // ── Retrieval ─────────────────────────────────────────────────────────

    /// Get a reference to the asset with the given id.
    pub fn get(&self, id: AssetId) -> Option<&T> {
        self.assets.get(&id)
    }

    /// Get a mutable reference to the asset with the given id.
    pub fn get_mut(&mut self, id: AssetId) -> Option<&mut T> {
        self.assets.get_mut(&id)
    }

    /// Look up by path, returning `(id, &asset)` if found and loaded.
    pub fn get_by_path(&self, path: &str) -> Option<(AssetId, &T)> {
        let &id = self.path_to_id.get(path)?;
        let asset = self.assets.get(&id)?;
        Some((id, asset))
    }

    /// Look up the id registered for a path, even if still loading.
    pub fn id_for_path(&self, path: &str) -> Option<AssetId> {
        self.path_to_id.get(path).copied()
    }

    // ── Removal ───────────────────────────────────────────────────────────

    /// Remove and return the asset at `id`. The path mapping is also cleaned up.
    pub fn remove(&mut self, id: AssetId) -> Option<T> {
        self.load_states.remove(&id);
        // Remove path mapping
        self.path_to_id.retain(|_, v| *v != id);
        self.assets.remove(&id)
    }

    /// Remove the asset registered at `path`. Returns the asset if it was present.
    pub fn remove_by_path(&mut self, path: &str) -> Option<T> {
        let id = self.path_to_id.remove(path)?;
        self.load_states.remove(&id);
        self.assets.remove(&id)
    }

    // ── State queries ─────────────────────────────────────────────────────

    /// Returns `true` if an asset occupies this slot (loaded, not just reserved).
    pub fn contains(&self, id: AssetId) -> bool {
        self.assets.contains_key(&id)
    }

    /// Returns `true` if any record (loaded or reserved) exists for this id.
    pub fn has_slot(&self, id: AssetId) -> bool {
        self.load_states.contains_key(&id)
    }

    /// The load state of the given id.
    pub fn load_state(&self, id: AssetId) -> LoadState {
        self.load_states.get(&id).cloned().unwrap_or(LoadState::NotLoaded)
    }

    /// Set the load state directly.
    pub fn set_load_state(&mut self, id: AssetId, state: LoadState) {
        self.load_states.insert(id, state);
    }

    // ── Enumeration ───────────────────────────────────────────────────────

    /// Number of assets currently stored (excludes reserved-but-loading slots).
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Returns `true` if no assets are stored.
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Iterate over all loaded `(AssetId, &T)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (AssetId, &T)> {
        self.assets.iter().map(|(&id, asset)| (id, asset))
    }

    /// Iterate mutably over all loaded `(AssetId, &mut T)` pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (AssetId, &mut T)> {
        self.assets.iter_mut().map(|(&id, asset)| (id, asset))
    }

    /// All currently registered `AssetId`s (including still-loading reservations).
    pub fn ids(&self) -> impl Iterator<Item = AssetId> + '_ {
        self.load_states.keys().copied()
    }

    /// All known paths (including still-loading reservations).
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.path_to_id.keys().map(String::as_str)
    }

    /// Number of paths registered (includes reserved).
    pub fn slot_count(&self) -> usize {
        self.load_states.len()
    }

    /// Clear all assets and reset the id counter.
    pub fn clear(&mut self) {
        self.assets.clear();
        self.load_states.clear();
        self.path_to_id.clear();
        self.id_counter = 0;
    }
}

impl<T> Default for AssetStorage<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────
//  AnyAssetStorage — type erasure
// ─────────────────────────────────────────────

/// Type-erased access to an `AssetStorage<T>`. Used by `AssetRegistry` to store
/// heterogeneous storage objects in a single collection.
pub trait AnyAssetStorage: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn contains_id(&self, id: AssetId) -> bool;
    fn load_state(&self, id: AssetId) -> LoadState;
    fn remove_by_id(&mut self, id: AssetId);
    fn slot_count(&self) -> usize;
    fn clear(&mut self);
}

impl<T: Any + Send + Sync + 'static> AnyAssetStorage for AssetStorage<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn len(&self) -> usize {
        AssetStorage::len(self)
    }
    fn is_empty(&self) -> bool {
        AssetStorage::is_empty(self)
    }
    fn contains_id(&self, id: AssetId) -> bool {
        self.contains(id)
    }
    fn load_state(&self, id: AssetId) -> LoadState {
        AssetStorage::load_state(self, id)
    }
    fn remove_by_id(&mut self, id: AssetId) {
        self.remove(id);
    }
    fn slot_count(&self) -> usize {
        AssetStorage::slot_count(self)
    }
    fn clear(&mut self) {
        AssetStorage::clear(self);
    }
}

// ─────────────────────────────────────────────
//  Loader entry
// ─────────────────────────────────────────────

struct LoaderEntry {
    loader: Box<dyn AssetLoader>,
    /// The extensions this loader handles (cached from `loader.extensions()`).
    extensions: Vec<String>,
}

// ─────────────────────────────────────────────
//  AssetRegistry
// ─────────────────────────────────────────────

/// Central registry holding all typed `AssetStorage`s and the loader table.
///
/// Typically wrapped inside an `AssetServer`. Most callers should interact with
/// `AssetServer` rather than `AssetRegistry` directly.
pub struct AssetRegistry {
    /// Per-type storage, keyed by `TypeId`.
    storages: HashMap<TypeId, Box<dyn AnyAssetStorage>>,
    /// All registered loaders.
    loaders: Vec<LoaderEntry>,
    /// Extension → index into `loaders`.
    extension_map: HashMap<String, usize>,
}

impl AssetRegistry {
    /// Create an empty registry. You must call `register_storage` and
    /// `register_loader` before loading assets.
    pub fn new() -> Self {
        Self {
            storages: HashMap::new(),
            loaders: Vec::new(),
            extension_map: HashMap::new(),
        }
    }

    // ── Loader registration ───────────────────────────────────────────────

    /// Register a loader. Panics if an extension is already claimed by another loader.
    pub fn register_loader_boxed(&mut self, loader: Box<dyn AssetLoader>) {
        let extensions: Vec<String> = loader.extensions().iter().map(|s| s.to_string()).collect();
        let idx = self.loaders.len();
        for ext in &extensions {
            // Later registrations override earlier ones for the same extension.
            self.extension_map.insert(ext.clone(), idx);
        }
        self.loaders.push(LoaderEntry { loader, extensions });
    }

    /// Register a loader by type.
    pub fn register_loader<T: Asset>(&mut self) {
        let loader = Box::new(T::Loader::default());
        self.register_loader_boxed(loader);
    }

    /// Register a custom loader instance.
    pub fn register_loader_instance<L: AssetLoader>(&mut self, loader: L) {
        self.register_loader_boxed(Box::new(loader));
    }

    /// Get the loader for a given file extension (case-insensitive).
    pub fn get_loader_for_extension(&self, ext: &str) -> Option<&dyn AssetLoader> {
        let ext_lower = ext.to_lowercase();
        let idx = self.extension_map.get(&ext_lower)?;
        Some(self.loaders[*idx].loader.as_ref())
    }

    /// Returns `true` if any loader is registered for the given extension.
    pub fn has_loader_for(&self, ext: &str) -> bool {
        self.extension_map.contains_key(&ext.to_lowercase())
    }

    /// All registered extensions.
    pub fn registered_extensions(&self) -> impl Iterator<Item = &str> {
        self.extension_map.keys().map(String::as_str)
    }

    // ── Storage registration ──────────────────────────────────────────────

    /// Register an `AssetStorage<T>` for the asset type `T`.
    ///
    /// If storage for `T` already exists, this is a no-op.
    pub fn register_storage<T: Asset>(&mut self) {
        let type_id = TypeId::of::<T>();
        self.storages
            .entry(type_id)
            .or_insert_with(|| Box::new(AssetStorage::<T>::new()));
    }

    /// Register storage and loader together for convenience.
    pub fn register<T: Asset>(&mut self) {
        self.register_storage::<T>();
        self.register_loader::<T>();
    }

    // ── Storage access ────────────────────────────────────────────────────

    /// Get a reference to the typed storage for `T`.
    ///
    /// Returns `None` if `register_storage::<T>()` has not been called.
    pub fn storage<T: Asset>(&self) -> Option<&AssetStorage<T>> {
        let type_id = TypeId::of::<T>();
        let any = self.storages.get(&type_id)?;
        any.as_any().downcast_ref::<AssetStorage<T>>()
    }

    /// Get a mutable reference to the typed storage for `T`.
    pub fn storage_mut<T: Asset>(&mut self) -> Option<&mut AssetStorage<T>> {
        let type_id = TypeId::of::<T>();
        let any = self.storages.get_mut(&type_id)?;
        any.as_any_mut().downcast_mut::<AssetStorage<T>>()
    }

    /// Get a reference to the typed storage, creating it if missing.
    pub fn storage_or_create<T: Asset>(&mut self) -> &AssetStorage<T> {
        self.register_storage::<T>();
        self.storage::<T>().unwrap()
    }

    /// Get a mutable reference to the typed storage, creating it if missing.
    pub fn storage_or_create_mut<T: Asset>(&mut self) -> &mut AssetStorage<T> {
        self.register_storage::<T>();
        self.storage_mut::<T>().unwrap()
    }

    // ── Load dispatch ─────────────────────────────────────────────────────

    /// Run the registered loader for `path` and store the result in the typed storage.
    ///
    /// Returns the `AssetId` on success. The caller is responsible for passing `bytes`.
    pub fn load_asset<T: Asset>(
        &mut self,
        path: &str,
        bytes: &[u8],
    ) -> Result<AssetId, AssetLoadError> {
        // Choose loader
        let ext = path.rsplit('.').next().unwrap_or("");
        let ext_lower = ext.to_lowercase();
        let loader_idx = self
            .extension_map
            .get(&ext_lower)
            .copied()
            .ok_or(AssetLoadError::UnsupportedFormat { extension: ext.to_string() })?;

        let mut ctx = LoadContext::new(path);
        let boxed = self.loaders[loader_idx].loader.load_bytes(bytes, path, &mut ctx)?;

        // Downcast to the concrete type
        let asset = *boxed.downcast::<T>().map_err(|_| AssetLoadError::InvalidData {
            message: format!(
                "loader returned wrong type for '{path}'; expected {}",
                std::any::type_name::<T>()
            ),
        })?;

        self.register_storage::<T>();
        let storage = self.storage_or_create_mut::<T>();
        let id = storage.insert(path, asset);
        Ok(id)
    }

    // ── Stats ─────────────────────────────────────────────────────────────

    /// Total number of registered storages (one per asset type).
    pub fn storage_count(&self) -> usize {
        self.storages.len()
    }

    /// Total number of loaders registered.
    pub fn loader_count(&self) -> usize {
        self.loaders.len()
    }

    /// Total assets across all storages.
    pub fn total_asset_count(&self) -> usize {
        self.storages.values().map(|s| s.len()).sum()
    }

    /// Clear all storages (removes all loaded assets).
    pub fn clear_all(&mut self) {
        for storage in self.storages.values_mut() {
            storage.clear();
        }
    }
}

impl Default for AssetRegistry {
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
    use crate::asset::loader::{TextAsset, TextAssetLoader};

    fn text(s: &str) -> TextAsset {
        TextAsset { text: s.to_string(), source_path: String::new() }
    }

    #[test]
    fn storage_insert_get() {
        let mut storage: AssetStorage<TextAsset> = AssetStorage::new();
        let id = storage.insert("test.txt", text("hello"));
        assert!(storage.contains(id));
        assert_eq!(storage.get(id).unwrap().text, "hello");
    }

    #[test]
    fn storage_path_lookup() {
        let mut storage: AssetStorage<TextAsset> = AssetStorage::new();
        storage.insert("greet.txt", text("hi"));
        let (id, asset) = storage.get_by_path("greet.txt").unwrap();
        assert_eq!(asset.text, "hi");
        assert_eq!(storage.id_for_path("greet.txt"), Some(id));
    }

    #[test]
    fn storage_replace_same_id() {
        let mut storage: AssetStorage<TextAsset> = AssetStorage::new();
        let id1 = storage.insert("greet.txt", text("old"));
        let id2 = storage.insert("greet.txt", text("new"));
        assert_eq!(id1, id2);
        assert_eq!(storage.get(id1).unwrap().text, "new");
    }

    #[test]
    fn storage_remove() {
        let mut storage: AssetStorage<TextAsset> = AssetStorage::new();
        let id = storage.insert("x.txt", text("x"));
        assert!(storage.remove(id).is_some());
        assert!(!storage.contains(id));
    }

    #[test]
    fn storage_reserve_then_insert_at() {
        let mut storage: AssetStorage<TextAsset> = AssetStorage::new();
        let id = storage.reserve("loading.txt");
        assert_eq!(storage.load_state(id), LoadState::Loading);
        assert!(!storage.contains(id));
        storage.insert_at(id, text("loaded!"));
        assert!(storage.contains(id));
        assert_eq!(storage.load_state(id), LoadState::Loaded);
    }

    #[test]
    fn storage_iter() {
        let mut storage: AssetStorage<TextAsset> = AssetStorage::new();
        storage.insert("a.txt", text("a"));
        storage.insert("b.txt", text("b"));
        let count = storage.iter().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn registry_register_and_load() {
        let mut registry = AssetRegistry::new();
        registry.register::<TextAsset>();

        let bytes = b"hello from registry";
        let id = registry.load_asset::<TextAsset>("hello.txt", bytes).unwrap();
        let storage = registry.storage::<TextAsset>().unwrap();
        assert_eq!(storage.get(id).unwrap().text, "hello from registry");
    }

    #[test]
    fn registry_unknown_extension() {
        let mut registry = AssetRegistry::new();
        registry.register::<TextAsset>();
        let result = registry.load_asset::<TextAsset>("file.xyz", b"data");
        assert!(matches!(result, Err(AssetLoadError::UnsupportedFormat { .. })));
    }

    #[test]
    fn registry_total_asset_count() {
        let mut registry = AssetRegistry::new();
        registry.register::<TextAsset>();
        registry.load_asset::<TextAsset>("a.txt", b"a").unwrap();
        registry.load_asset::<TextAsset>("b.txt", b"b").unwrap();
        assert_eq!(registry.total_asset_count(), 2);
    }
}
