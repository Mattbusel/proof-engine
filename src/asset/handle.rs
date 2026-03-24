//! Asset handles and identifiers.
//!
//! `AssetId` is a 64-bit packed value holding a 32-bit slot index and a 32-bit
//! generation counter, making stale handles detectable. `Handle<T>` wraps an id
//! with a `PhantomData` marker so the type system tracks what kind of asset the
//! handle refers to. Both `Handle<T>` and `HandleUntyped` are `Send + Sync`
//! regardless of `T`.

use std::any::TypeId;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

// ─────────────────────────────────────────────
//  AssetId
// ─────────────────────────────────────────────

/// Packed 64-bit asset identifier: high 32 bits = slot index, low 32 bits = generation.
///
/// The generation is incremented each time a slot is reused, allowing callers to
/// detect handles that point to a since-removed asset.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct AssetId(u64);

impl AssetId {
    /// Construct from separate index and generation components.
    #[inline]
    pub fn new(index: u32, generation: u32) -> Self {
        AssetId(((index as u64) << 32) | (generation as u64))
    }

    /// Construct from a raw packed `u64`. Useful for serialization round-trips.
    #[inline]
    pub fn from_raw(raw: u64) -> Self {
        AssetId(raw)
    }

    /// The slot index — unique within a storage until slots are recycled.
    #[inline]
    pub fn index(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// The generation counter.
    #[inline]
    pub fn generation(self) -> u32 {
        self.0 as u32
    }

    /// The raw packed `u64`.
    #[inline]
    pub fn raw(self) -> u64 {
        self.0
    }

    /// A sentinel "null" id that is never assigned to a real asset.
    pub const NULL: AssetId = AssetId(u64::MAX);

    /// Returns `true` if this is the null sentinel.
    #[inline]
    pub fn is_null(self) -> bool {
        self == Self::NULL
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetId({}:{})", self.index(), self.generation())
    }
}

// ─────────────────────────────────────────────
//  Reference counting
// ─────────────────────────────────────────────

/// Per-asset reference count shared between strong handles.
///
/// When `strong_count` reaches zero the asset server may choose to evict the
/// asset from memory (policy is up to the `AssetServer`).
pub struct AssetRefCount {
    pub strong_count: AtomicU32,
    pub weak_count: AtomicU32,
}

impl AssetRefCount {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            strong_count: AtomicU32::new(1),
            weak_count: AtomicU32::new(0),
        })
    }

    pub fn strong(&self) -> u32 {
        self.strong_count.load(Ordering::SeqCst)
    }

    pub fn weak(&self) -> u32 {
        self.weak_count.load(Ordering::SeqCst)
    }

    pub fn inc_strong(&self) {
        self.strong_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn dec_strong(&self) -> u32 {
        self.strong_count.fetch_sub(1, Ordering::SeqCst).saturating_sub(1)
    }

    pub fn inc_weak(&self) {
        self.weak_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn dec_weak(&self) -> u32 {
        self.weak_count.fetch_sub(1, Ordering::SeqCst).saturating_sub(1)
    }
}

impl Default for AssetRefCount {
    fn default() -> Self {
        Self {
            strong_count: AtomicU32::new(1),
            weak_count: AtomicU32::new(0),
        }
    }
}

// ─────────────────────────────────────────────
//  Handle<T>
// ─────────────────────────────────────────────

/// A typed, cheaply cloneable handle to a loaded asset of type `T`.
///
/// A handle does **not** keep the asset alive by itself — lifetime management is
/// handled by `AssetServer`. Handles that are "strong" merely signal to the server
/// that the caller still cares about the asset; weak handles can be used for
/// look-ups without preventing eviction.
pub struct Handle<T> {
    pub id: AssetId,
    /// Whether this handle contributes to the strong reference count.
    strong: bool,
    /// Optional reference count — `None` for weak handles created without a counter.
    ref_count: Option<Arc<AssetRefCount>>,
    marker: PhantomData<fn() -> T>,
}

// Safety: Handle<T> only stores AssetId (Copy) and an Arc — both are Send+Sync
// regardless of T. The PhantomData<fn() -> T> is covariant and doesn't add Send
// restrictions.
unsafe impl<T> Send for Handle<T> {}
unsafe impl<T> Sync for Handle<T> {}

impl<T> Handle<T> {
    /// Create a new strong handle.
    pub fn strong(id: AssetId, ref_count: Arc<AssetRefCount>) -> Self {
        ref_count.inc_strong();
        Self {
            id,
            strong: true,
            ref_count: Some(ref_count),
            marker: PhantomData,
        }
    }

    /// Create a new weak handle (does not affect strong ref count).
    pub fn weak(id: AssetId) -> Self {
        Self {
            id,
            strong: false,
            ref_count: None,
            marker: PhantomData,
        }
    }

    /// Create a weak handle from an existing strong handle without cloning the count.
    pub fn weak_from(other: &Handle<T>) -> Self {
        if let Some(rc) = &other.ref_count {
            rc.inc_weak();
            Self {
                id: other.id,
                strong: false,
                ref_count: Some(Arc::clone(rc)),
                marker: PhantomData,
            }
        } else {
            Self::weak(other.id)
        }
    }

    /// The underlying `AssetId`.
    #[inline]
    pub fn id(&self) -> AssetId {
        self.id
    }

    /// Returns `true` if this is a weak handle.
    #[inline]
    pub fn is_weak(&self) -> bool {
        !self.strong
    }

    /// Returns `true` if this is a strong handle.
    #[inline]
    pub fn is_strong(&self) -> bool {
        self.strong
    }

    /// Clone as a weak handle (no ref-count bump).
    pub fn clone_weak(&self) -> Handle<T> {
        Handle::weak(self.id)
    }

    /// Clone as a strong handle (increments strong ref count if one exists).
    pub fn clone_strong(&self) -> Handle<T> {
        if let Some(rc) = &self.ref_count {
            Handle::strong(self.id, Arc::clone(rc))
        } else {
            // If no ref count tracking, just clone the id
            Handle {
                id: self.id,
                strong: self.strong,
                ref_count: None,
                marker: PhantomData,
            }
        }
    }

    /// Reinterpret this handle as a handle to a different asset type.
    ///
    /// # Safety
    /// The caller must ensure `U` is actually the correct type stored at this id.
    pub fn typed<U>(self) -> Handle<U> {
        Handle {
            id: self.id,
            strong: self.strong,
            ref_count: self.ref_count,
            marker: PhantomData,
        }
    }

    /// Erase the type information, producing a `HandleUntyped`.
    pub fn untyped(self) -> HandleUntyped
    where
        T: 'static,
    {
        HandleUntyped {
            id: self.id,
            type_id: TypeId::of::<T>(),
            strong: self.strong,
            ref_count: self.ref_count,
        }
    }

    /// Number of strong references (0 if no ref count is attached).
    pub fn strong_count(&self) -> u32 {
        self.ref_count.as_ref().map_or(0, |rc| rc.strong())
    }
}

impl<T> Drop for Handle<T> {
    fn drop(&mut self) {
        if self.strong {
            if let Some(rc) = &self.ref_count {
                rc.dec_strong();
            }
        } else if let Some(rc) = &self.ref_count {
            rc.dec_weak();
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        if self.strong {
            self.clone_strong()
        } else {
            self.clone_weak()
        }
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Handle<{}>({}:{})",
            std::any::type_name::<T>(),
            self.id.index(),
            self.id.generation()
        )
    }
}

impl<T> fmt::Display for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

// ─────────────────────────────────────────────
//  HandleUntyped
// ─────────────────────────────────────────────

/// Type-erased asset handle. Useful when storing handles of mixed asset types.
pub struct HandleUntyped {
    pub id: AssetId,
    pub type_id: TypeId,
    strong: bool,
    ref_count: Option<Arc<AssetRefCount>>,
}

unsafe impl Send for HandleUntyped {}
unsafe impl Sync for HandleUntyped {}

impl HandleUntyped {
    /// Create a new untyped weak handle.
    pub fn weak(id: AssetId, type_id: TypeId) -> Self {
        Self {
            id,
            type_id,
            strong: false,
            ref_count: None,
        }
    }

    /// Attempt to recover the typed handle. Returns `None` if the type doesn't match.
    pub fn typed<T: 'static>(self) -> Option<Handle<T>> {
        if self.type_id == TypeId::of::<T>() {
            Some(Handle {
                id: self.id,
                strong: self.strong,
                ref_count: self.ref_count,
                marker: PhantomData,
            })
        } else {
            None
        }
    }

    pub fn is_weak(&self) -> bool {
        !self.strong
    }

    pub fn is_strong(&self) -> bool {
        self.strong
    }

    pub fn clone_weak(&self) -> HandleUntyped {
        HandleUntyped {
            id: self.id,
            type_id: self.type_id,
            strong: false,
            ref_count: None,
        }
    }
}

impl Clone for HandleUntyped {
    fn clone(&self) -> Self {
        if self.strong {
            if let Some(rc) = &self.ref_count {
                rc.inc_strong();
            }
        }
        Self {
            id: self.id,
            type_id: self.type_id,
            strong: self.strong,
            ref_count: self.ref_count.clone(),
        }
    }
}

impl Drop for HandleUntyped {
    fn drop(&mut self) {
        if self.strong {
            if let Some(rc) = &self.ref_count {
                rc.dec_strong();
            }
        }
    }
}

impl PartialEq for HandleUntyped {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.type_id == other.type_id
    }
}

impl Eq for HandleUntyped {}

impl Hash for HandleUntyped {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.type_id.hash(state);
    }
}

impl fmt::Debug for HandleUntyped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HandleUntyped({:?}, {})", self.type_id, self.id)
    }
}

// ─────────────────────────────────────────────
//  Strong / Weak newtype aliases
// ─────────────────────────────────────────────

/// A guaranteed-strong handle. Created by `AssetServer::load`.
pub struct HandleStrong<T>(pub Handle<T>);

/// A guaranteed-weak handle. Never keeps an asset alive.
pub struct HandleWeak<T>(pub Handle<T>);

impl<T> HandleStrong<T> {
    pub fn id(&self) -> AssetId {
        self.0.id()
    }

    pub fn downgrade(self) -> HandleWeak<T> {
        HandleWeak(self.0.clone_weak())
    }
}

impl<T> HandleWeak<T> {
    pub fn id(&self) -> AssetId {
        self.0.id()
    }
}

impl<T> Clone for HandleStrong<T> {
    fn clone(&self) -> Self {
        HandleStrong(self.0.clone_strong())
    }
}

impl<T> Clone for HandleWeak<T> {
    fn clone(&self) -> Self {
        HandleWeak(self.0.clone_weak())
    }
}

unsafe impl<T> Send for HandleStrong<T> {}
unsafe impl<T> Sync for HandleStrong<T> {}
unsafe impl<T> Send for HandleWeak<T> {}
unsafe impl<T> Sync for HandleWeak<T> {}

// ─────────────────────────────────────────────
//  AssetPath
// ─────────────────────────────────────────────

/// A path to an asset on disk, with an optional label for sub-assets.
///
/// The `path#label` syntax is used to reference a specific sub-asset within a file.
/// For example `"atlas.png#player_idle"` refers to a frame called `player_idle`
/// packed inside `atlas.png`.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct AssetPath {
    path: String,
    label: Option<String>,
}

impl AssetPath {
    /// Parse a path, splitting on `#` to extract an optional label.
    pub fn new(raw: impl Into<String>) -> Self {
        let s: String = raw.into();
        if let Some(pos) = s.find('#') {
            let (path, rest) = s.split_at(pos);
            let label = &rest[1..]; // skip '#'
            Self {
                path: path.to_string(),
                label: if label.is_empty() { None } else { Some(label.to_string()) },
            }
        } else {
            Self { path: s, label: None }
        }
    }

    /// Construct explicitly from path and optional label.
    pub fn with_parts(path: impl Into<String>, label: Option<String>) -> Self {
        Self { path: path.into(), label }
    }

    /// The file path portion (without label).
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The optional label portion.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Return a new `AssetPath` with the given label attached.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Return a new `AssetPath` with the label stripped.
    pub fn without_label(mut self) -> Self {
        self.label = None;
        self
    }

    /// File extension (after the last `.`), if any.
    pub fn extension(&self) -> Option<&str> {
        self.path.rsplit('.').next().filter(|e| !e.contains('/'))
    }
}

impl fmt::Display for AssetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref label) = self.label {
            write!(f, "{}#{}", self.path, label)
        } else {
            write!(f, "{}", self.path)
        }
    }
}

impl From<&str> for AssetPath {
    fn from(s: &str) -> Self {
        AssetPath::new(s)
    }
}

impl From<String> for AssetPath {
    fn from(s: String) -> Self {
        AssetPath::new(s)
    }
}

// ─────────────────────────────────────────────
//  LoadState
// ─────────────────────────────────────────────

/// The current loading state of an asset slot.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum LoadState {
    /// The asset has never been requested or has been unloaded.
    #[default]
    NotLoaded,
    /// A load request has been issued and is in progress.
    Loading,
    /// The asset loaded successfully and is ready to use.
    Loaded,
    /// The load failed. The string describes the error.
    Failed(String),
}

impl LoadState {
    pub fn is_loaded(&self) -> bool {
        matches!(self, LoadState::Loaded)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, LoadState::Loading)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, LoadState::Failed(_))
    }

    pub fn error_message(&self) -> Option<&str> {
        if let LoadState::Failed(msg) = self {
            Some(msg.as_str())
        } else {
            None
        }
    }
}

impl fmt::Display for LoadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadState::NotLoaded => write!(f, "NotLoaded"),
            LoadState::Loading => write!(f, "Loading"),
            LoadState::Loaded => write!(f, "Loaded"),
            LoadState::Failed(e) => write!(f, "Failed({e})"),
        }
    }
}

// ─────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_id_round_trip() {
        let id = AssetId::new(42, 7);
        assert_eq!(id.index(), 42);
        assert_eq!(id.generation(), 7);
        let id2 = AssetId::from_raw(id.raw());
        assert_eq!(id, id2);
    }

    #[test]
    fn asset_id_null() {
        assert!(AssetId::NULL.is_null());
        assert!(!AssetId::new(0, 0).is_null());
    }

    #[test]
    fn asset_path_parse_label() {
        let p = AssetPath::new("sprites/hero.png#idle");
        assert_eq!(p.path(), "sprites/hero.png");
        assert_eq!(p.label(), Some("idle"));
        assert_eq!(p.extension(), Some("png"));
    }

    #[test]
    fn asset_path_no_label() {
        let p = AssetPath::new("data/config.toml");
        assert_eq!(p.path(), "data/config.toml");
        assert_eq!(p.label(), None);
        assert_eq!(p.extension(), Some("toml"));
    }

    #[test]
    fn asset_path_display() {
        let p = AssetPath::new("music/theme.ogg#loop");
        assert_eq!(p.to_string(), "music/theme.ogg#loop");
    }

    #[test]
    fn handle_weak_strong() {
        let rc = AssetRefCount::new();
        let id = AssetId::new(1, 0);
        let h: Handle<String> = Handle::strong(id, rc.clone());
        assert!(h.is_strong());
        let w = h.clone_weak();
        assert!(w.is_weak());
        assert_eq!(h.id(), w.id());
    }

    #[test]
    fn load_state_helpers() {
        assert!(LoadState::Loaded.is_loaded());
        assert!(LoadState::Loading.is_loading());
        assert!(LoadState::Failed("oops".into()).is_failed());
        assert_eq!(LoadState::Failed("oops".into()).error_message(), Some("oops"));
    }

    #[test]
    fn handle_untyped_typed_roundtrip() {
        let rc = AssetRefCount::new();
        let id = AssetId::new(5, 2);
        let h: Handle<String> = Handle::strong(id, rc);
        let untyped = h.untyped();
        assert!(untyped.typed::<String>().is_some());
        // Wrong type returns None
        let rc2 = AssetRefCount::new();
        let h2: Handle<String> = Handle::strong(id, rc2);
        let untyped2 = h2.untyped();
        assert!(untyped2.typed::<u32>().is_none());
    }

    #[test]
    fn handle_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Handle<Vec<u8>>>();
        assert_send_sync::<HandleUntyped>();
    }
}
