//! Component storage primitives for the ECS.
//!
//! The primary storage is [`ComponentStorage<T>`], a **sparse set** implementation:
//! - A sparse array (`Vec<Option<usize>>`) maps entity slot indices to dense indices.
//! - A dense array (`Vec<T>`) holds the actual component values contiguously.
//! - A parallel `Vec<Entity>` records which entity owns each dense slot.
//!
//! This gives O(1) insert/remove/lookup and cache-friendly iteration.
//!
//! For type-erased storage needed by [`crate::ecs::world::World`], the
//! [`AnyComponentStorage`] trait and [`TypedStorage<T>`] wrapper provide
//! downcasting support.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::marker::PhantomData;

use super::entity::Entity;

// ---------------------------------------------------------------------------
// Component trait
// ---------------------------------------------------------------------------

/// Marker trait for all component types.
/// Automatically implemented for any `'static + Send + Sync` type.
pub trait Component: 'static + Send + Sync {}
impl<T: 'static + Send + Sync> Component for T {}

// ---------------------------------------------------------------------------
// ComponentStorage<T>
// ---------------------------------------------------------------------------

/// Sparse-set component storage for component type `T`.
///
/// # Complexity
/// | Operation | Complexity |
/// |-----------|-----------|
/// | `insert`  | O(1) amortised |
/// | `remove`  | O(1)      |
/// | `get`     | O(1)      |
/// | iteration | O(n) dense|
#[derive(Debug)]
pub struct ComponentStorage<T> {
    /// Maps entity slot id → index into the dense arrays. `None` = absent.
    sparse: Vec<Option<usize>>,
    /// Dense component values — tightly packed, cache-friendly.
    dense: Vec<T>,
    /// Dense entity list — parallel to `dense`, records which entity owns each slot.
    entities: Vec<Entity>,
    /// Change-detection tick — incremented when components are modified.
    change_tick: u64,
    /// Per-component added/changed ticks for fine-grained change detection.
    added_ticks: Vec<u64>,
    changed_ticks: Vec<u64>,
}

impl<T: Component> ComponentStorage<T> {
    /// Create an empty storage.
    pub fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense: Vec::new(),
            entities: Vec::new(),
            change_tick: 0,
            added_ticks: Vec::new(),
            changed_ticks: Vec::new(),
        }
    }

    /// Create storage with pre-allocated sparse capacity.
    pub fn with_capacity(sparse_cap: usize, dense_cap: usize) -> Self {
        Self {
            sparse: Vec::with_capacity(sparse_cap),
            dense: Vec::with_capacity(dense_cap),
            entities: Vec::with_capacity(dense_cap),
            change_tick: 0,
            added_ticks: Vec::with_capacity(dense_cap),
            changed_ticks: Vec::with_capacity(dense_cap),
        }
    }

    fn ensure_sparse(&mut self, id: usize) {
        if id >= self.sparse.len() {
            self.sparse.resize(id + 1, None);
        }
    }

    /// Insert or replace the component for `entity`.
    /// Returns the old component if one existed.
    pub fn insert(&mut self, entity: Entity, component: T) -> Option<T> {
        let id = entity.id() as usize;
        self.ensure_sparse(id);
        self.change_tick += 1;
        let tick = self.change_tick;

        if let Some(dense_idx) = self.sparse[id] {
            // Replace existing.
            let old = std::mem::replace(&mut self.dense[dense_idx], component);
            self.changed_ticks[dense_idx] = tick;
            Some(old)
        } else {
            // New entry.
            let dense_idx = self.dense.len();
            self.sparse[id] = Some(dense_idx);
            self.dense.push(component);
            self.entities.push(entity);
            self.added_ticks.push(tick);
            self.changed_ticks.push(tick);
            None
        }
    }

    /// Remove the component for `entity`. Returns it if present.
    pub fn remove(&mut self, entity: Entity) -> Option<T> {
        self.remove_and_return(entity)
    }

    /// Remove the component for `entity`. Returns it if present.
    pub fn remove_and_return(&mut self, entity: Entity) -> Option<T> {
        let id = entity.id() as usize;
        if id >= self.sparse.len() {
            return None;
        }
        let dense_idx = match self.sparse[id] {
            Some(i) => i,
            None => return None,
        };

        self.sparse[id] = None;
        let last_idx = self.dense.len() - 1;

        // If not last, move last element into the gap.
        if dense_idx != last_idx {
            let moved_entity = self.entities[last_idx];
            self.sparse[moved_entity.id() as usize] = Some(dense_idx);
        }

        let value = self.dense.swap_remove(dense_idx);
        self.entities.swap_remove(dense_idx);
        self.added_ticks.swap_remove(dense_idx);
        self.changed_ticks.swap_remove(dense_idx);

        Some(value)
    }

    /// Returns `true` if `entity` has this component.
    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        let id = entity.id() as usize;
        id < self.sparse.len() && self.sparse[id].is_some()
    }

    /// Get an immutable reference to the component for `entity`.
    #[inline]
    pub fn get(&self, entity: Entity) -> Option<&T> {
        let id = entity.id() as usize;
        if id >= self.sparse.len() {
            return None;
        }
        let dense_idx = self.sparse[id]?;
        Some(&self.dense[dense_idx])
    }

    /// Get a mutable reference to the component for `entity`.
    #[inline]
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let id = entity.id() as usize;
        if id >= self.sparse.len() {
            return None;
        }
        let dense_idx = self.sparse[id]?;
        self.change_tick += 1;
        self.changed_ticks[dense_idx] = self.change_tick;
        Some(&mut self.dense[dense_idx])
    }

    /// Number of components stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.dense.len()
    }

    /// Returns `true` if no components are stored.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    /// Clear all components.
    pub fn clear(&mut self) {
        self.sparse.iter_mut().for_each(|s| *s = None);
        self.dense.clear();
        self.entities.clear();
        self.added_ticks.clear();
        self.changed_ticks.clear();
    }

    /// Iterate over `(entity, component)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&Entity, &T)> {
        self.entities.iter().zip(self.dense.iter())
    }

    /// Iterate over `(entity, &mut component)` pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Entity, &mut T)> {
        self.entities.iter().zip(self.dense.iter_mut())
    }

    /// Iterate over component values only (dense, cache-friendly).
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.dense.iter()
    }

    /// Iterate over mutable component values only.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.dense.iter_mut()
    }

    /// Iterate over entities that have this component.
    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter()
    }

    /// Get the dense index for an entity (internal helper).
    #[inline]
    pub fn dense_index(&self, entity: Entity) -> Option<usize> {
        let id = entity.id() as usize;
        if id >= self.sparse.len() { None } else { self.sparse[id] }
    }

    /// Sort the dense arrays by entity (useful for deterministic iteration).
    pub fn sort_by_entity(&mut self) {
        // Build indices sorted by entity
        let mut indices: Vec<usize> = (0..self.dense.len()).collect();
        indices.sort_by_key(|&i| self.entities[i]);

        // Apply permutation in-place using a temporary.
        let new_entities: Vec<Entity> = indices.iter().map(|&i| self.entities[i]).collect();
        let new_added: Vec<u64> = indices.iter().map(|&i| self.added_ticks[i]).collect();
        let new_changed: Vec<u64> = indices.iter().map(|&i| self.changed_ticks[i]).collect();

        // Apply the permutation to self.dense using a swap-cycle approach.
        // This works in-place without requiring T: Default or Clone.
        let mut placed = vec![false; self.dense.len()];
        for start in 0..self.dense.len() {
            if placed[start] { continue; }
            let mut current = start;
            loop {
                let target = indices[current];
                if target == start || placed[target] {
                    placed[current] = true;
                    break;
                }
                self.dense.swap(current, target);
                placed[current] = true;
                current = target;
            }
        }

        self.entities = new_entities;
        self.added_ticks = new_added;
        self.changed_ticks = new_changed;

        // Rebuild sparse from new entity positions.
        for (dense_idx, entity) in self.entities.iter().enumerate() {
            let id = entity.id() as usize;
            self.sparse[id] = Some(dense_idx);
        }
    }

    /// Returns components added since `tick`.
    pub fn iter_added_since(&self, tick: u64) -> impl Iterator<Item = (&Entity, &T)> {
        self.entities
            .iter()
            .zip(self.dense.iter())
            .zip(self.added_ticks.iter())
            .filter_map(move |((e, c), &t)| if t > tick { Some((e, c)) } else { None })
    }

    /// Returns components changed since `tick`.
    pub fn iter_changed_since(&self, tick: u64) -> impl Iterator<Item = (&Entity, &T)> {
        self.entities
            .iter()
            .zip(self.dense.iter())
            .zip(self.changed_ticks.iter())
            .filter_map(move |((e, c), &t)| if t > tick { Some((e, c)) } else { None })
    }

    /// Current change tick.
    pub fn change_tick(&self) -> u64 {
        self.change_tick
    }

    /// Bump the change tick externally (e.g., when the world ticks).
    pub fn advance_tick(&mut self) {
        self.change_tick += 1;
    }

    /// Get a component by its dense index (unchecked — panics on OOB).
    #[inline]
    pub fn get_by_dense_index(&self, idx: usize) -> &T {
        &self.dense[idx]
    }

    /// Get a mutable component by dense index.
    #[inline]
    pub fn get_mut_by_dense_index(&mut self, idx: usize) -> &mut T {
        &mut self.dense[idx]
    }

    /// Drain all components, returning an iterator of `(Entity, T)` pairs.
    pub fn drain(&mut self) -> impl Iterator<Item = (Entity, T)> + '_ {
        self.sparse.iter_mut().for_each(|s| *s = None);
        let entities = std::mem::take(&mut self.entities);
        let dense = std::mem::take(&mut self.dense);
        self.added_ticks.clear();
        self.changed_ticks.clear();
        entities.into_iter().zip(dense.into_iter())
    }

    /// Returns true if ALL entities in `entities` have this component.
    pub fn contains_all(&self, entities: &[Entity]) -> bool {
        entities.iter().all(|e| self.contains(*e))
    }

    /// Returns true if ANY entity in `entities` has this component.
    pub fn contains_any(&self, entities: &[Entity]) -> bool {
        entities.iter().any(|e| self.contains(*e))
    }
}

impl<T: Component> Default for ComponentStorage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Component + Clone> ComponentStorage<T> {
    /// Clone the component for `entity` without borrowing the whole storage.
    pub fn clone_component(&self, entity: Entity) -> Option<T> {
        self.get(entity).cloned()
    }
}

// ---------------------------------------------------------------------------
// AnyComponentStorage — type-erased trait
// ---------------------------------------------------------------------------

/// Type-erased interface for component storage, allowing [`World`] to hold
/// heterogeneous storages in a `HashMap<TypeId, Box<dyn AnyComponentStorage>>`.
pub trait AnyComponentStorage: Any + Send + Sync {
    /// The `TypeId` of the component type this storage holds.
    fn component_type_id(&self) -> TypeId;

    /// Remove the component for `entity` (value is dropped).
    fn remove_erased(&mut self, entity: Entity);

    /// Returns `true` if `entity` has this component.
    fn contains_erased(&self, entity: Entity) -> bool;

    /// Number of components stored.
    fn len_erased(&self) -> usize;

    /// Clear all components.
    fn clear_erased(&mut self);

    /// Expose as `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Expose as `&mut dyn Any` for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Remove all components for entities that are no longer alive.
    fn remove_stale(&mut self, alive: &[Entity]);
}

// ---------------------------------------------------------------------------
// TypedStorage<T> — AnyComponentStorage wrapper
// ---------------------------------------------------------------------------

/// Wraps `ComponentStorage<T>` inside a `RefCell` and implements
/// [`AnyComponentStorage`] for type-erased use in [`World`].
pub struct TypedStorage<T: Component> {
    pub(crate) inner: RefCell<ComponentStorage<T>>,
}

impl<T: Component> TypedStorage<T> {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(ComponentStorage::new()),
        }
    }

    /// Borrow the inner storage immutably.
    pub fn borrow(&self) -> std::cell::Ref<'_, ComponentStorage<T>> {
        self.inner.borrow()
    }

    /// Borrow the inner storage mutably.
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, ComponentStorage<T>> {
        self.inner.borrow_mut()
    }
}

impl<T: Component> Default for TypedStorage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Component> std::fmt::Debug for TypedStorage<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypedStorage<{}>(len={})", std::any::type_name::<T>(), self.inner.borrow().len())
    }
}

// SAFETY: T: Send + Sync, RefCell<ComponentStorage<T>> is not Send by default,
// but we uphold exclusive-access invariants through the ECS world borrow rules.
unsafe impl<T: Component> Send for TypedStorage<T> {}
unsafe impl<T: Component> Sync for TypedStorage<T> {}

impl<T: Component> AnyComponentStorage for TypedStorage<T> {
    fn component_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn remove_erased(&mut self, entity: Entity) {
        self.inner.borrow_mut().remove_and_return(entity);
    }

    fn contains_erased(&self, entity: Entity) -> bool {
        self.inner.borrow().contains(entity)
    }

    fn len_erased(&self) -> usize {
        self.inner.borrow().len()
    }

    fn clear_erased(&mut self) {
        self.inner.borrow_mut().clear();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn remove_stale(&mut self, alive: &[Entity]) {
        let mut storage = self.inner.borrow_mut();
        let stale: Vec<Entity> = storage
            .entities()
            .copied()
            .filter(|e| !alive.contains(e))
            .collect();
        for e in stale {
            storage.remove_and_return(e);
        }
    }
}

// ---------------------------------------------------------------------------
// ComponentVec — helper newtype for boxed storage
// ---------------------------------------------------------------------------

/// A `Box<dyn AnyComponentStorage>` with a convenient downcasting API.
pub struct ComponentVec(pub Box<dyn AnyComponentStorage>);

impl ComponentVec {
    pub fn new<T: Component>() -> Self {
        Self(Box::new(TypedStorage::<T>::new()))
    }

    /// Downcast to `&TypedStorage<T>`, or `None` if the type doesn't match.
    pub fn downcast_ref<T: Component>(&self) -> Option<&TypedStorage<T>> {
        self.0.as_any().downcast_ref::<TypedStorage<T>>()
    }

    /// Downcast to `&mut TypedStorage<T>`.
    pub fn downcast_mut<T: Component>(&mut self) -> Option<&mut TypedStorage<T>> {
        self.0.as_any_mut().downcast_mut::<TypedStorage<T>>()
    }

    /// Convenience: get a component from the storage.
    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<std::cell::Ref<'_, ComponentStorage<T>>> {
        let typed = self.downcast_ref::<T>()?;
        let guard = typed.borrow();
        // We can't return a Ref to a sub-field, so just return the guard.
        // Caller can call .get(entity) on it.
        Some(guard)
    }

    /// Delegate to inner storage.
    pub fn contains(&self, entity: Entity) -> bool {
        self.0.contains_erased(entity)
    }

    pub fn remove(&mut self, entity: Entity) {
        self.0.remove_erased(entity);
    }

    pub fn len(&self) -> usize {
        self.0.len_erased()
    }

    pub fn is_empty(&self) -> bool {
        self.0.len_erased() == 0
    }

    pub fn component_type_id(&self) -> TypeId {
        self.0.component_type_id()
    }
}

impl std::fmt::Debug for ComponentVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ComponentVec(TypeId={:?}, len={})", self.component_type_id(), self.len())
    }
}

// ---------------------------------------------------------------------------
// StorageSet — a collection of typed storages identified by TypeId
// ---------------------------------------------------------------------------

/// A map of `TypeId -> ComponentVec`, used internally by [`World`].
#[derive(Debug, Default)]
pub struct StorageSet {
    storages: std::collections::HashMap<TypeId, ComponentVec>,
}

impl StorageSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or insert storage for component type `T`.
    pub fn get_or_insert<T: Component>(&mut self) -> &ComponentVec {
        self.storages
            .entry(TypeId::of::<T>())
            .or_insert_with(ComponentVec::new::<T>)
    }

    /// Get or insert storage for component type `T` (mutable).
    pub fn get_or_insert_mut<T: Component>(&mut self) -> &mut ComponentVec {
        self.storages
            .entry(TypeId::of::<T>())
            .or_insert_with(ComponentVec::new::<T>)
    }

    /// Get immutable storage for `T`, returns `None` if not yet registered.
    pub fn get<T: Component>(&self) -> Option<&ComponentVec> {
        self.storages.get(&TypeId::of::<T>())
    }

    /// Get mutable storage for `T`.
    pub fn get_mut<T: Component>(&mut self) -> Option<&mut ComponentVec> {
        self.storages.get_mut(&TypeId::of::<T>())
    }

    /// Remove all components for `entity` across all storages.
    pub fn remove_entity(&mut self, entity: Entity) {
        for cv in self.storages.values_mut() {
            cv.remove(entity);
        }
    }

    /// Remove stale entries (entities no longer alive).
    pub fn remove_stale(&mut self, alive: &[Entity]) {
        for cv in self.storages.values_mut() {
            cv.0.remove_stale(alive);
        }
    }

    /// Number of component types registered.
    pub fn type_count(&self) -> usize {
        self.storages.len()
    }

    /// Total components across all storages.
    pub fn total_component_count(&self) -> usize {
        self.storages.values().map(|cv| cv.len()).sum()
    }

    /// Clear all storages.
    pub fn clear_all(&mut self) {
        for cv in self.storages.values_mut() {
            cv.0.clear_erased();
        }
    }

    /// Iterate over all storage entries.
    pub fn iter(&self) -> impl Iterator<Item = (&TypeId, &ComponentVec)> {
        self.storages.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&TypeId, &mut ComponentVec)> {
        self.storages.iter_mut()
    }

    /// Check if a component type is registered.
    pub fn has_type<T: Component>(&self) -> bool {
        self.storages.contains_key(&TypeId::of::<T>())
    }

    /// Check entity has component T.
    pub fn contains<T: Component>(&self, entity: Entity) -> bool {
        self.get::<T>().map_or(false, |cv| cv.contains(entity))
    }

    /// Insert component T for entity.
    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) {
        let cv = self.get_or_insert_mut::<T>();
        cv.downcast_mut::<T>()
            .expect("storage type mismatch")
            .borrow_mut()
            .insert(entity, component);
    }

    /// Remove component T for entity, returning it.
    pub fn remove_component<T: Component>(&mut self, entity: Entity) -> Option<T> {
        let cv = self.get_mut::<T>()?;
        cv.downcast_mut::<T>()
            .expect("storage type mismatch")
            .borrow_mut()
            .remove_and_return(entity)
    }
}

// ---------------------------------------------------------------------------
// Archetype — groups entities with the same component composition
// ---------------------------------------------------------------------------

/// Tracks which component types an entity has.
/// Used for accelerated query matching.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentMask {
    /// Sorted list of TypeIds present.
    types: Vec<TypeId>,
}

impl ComponentMask {
    pub fn empty() -> Self {
        Self { types: Vec::new() }
    }

    pub fn with<T: Component>(mut self) -> Self {
        let tid = TypeId::of::<T>();
        if let Err(pos) = self.types.binary_search(&tid) {
            self.types.insert(pos, tid);
        }
        self
    }

    pub fn add<T: Component>(&mut self) {
        let tid = TypeId::of::<T>();
        if let Err(pos) = self.types.binary_search(&tid) {
            self.types.insert(pos, tid);
        }
    }

    pub fn remove<T: Component>(&mut self) {
        let tid = TypeId::of::<T>();
        if let Ok(pos) = self.types.binary_search(&tid) {
            self.types.remove(pos);
        }
    }

    pub fn has<T: Component>(&self) -> bool {
        self.types.binary_search(&TypeId::of::<T>()).is_ok()
    }

    pub fn has_type(&self, tid: &TypeId) -> bool {
        self.types.binary_search(tid).is_ok()
    }

    pub fn contains_all(&self, required: &[TypeId]) -> bool {
        required.iter().all(|t| self.has_type(t))
    }

    pub fn contains_none(&self, excluded: &[TypeId]) -> bool {
        excluded.iter().all(|t| !self.has_type(t))
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    pub fn types(&self) -> &[TypeId] {
        &self.types
    }
}

impl Default for ComponentMask {
    fn default() -> Self {
        Self::empty()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::entity::EntityAllocator;

    fn make_entity(id: u32, gen: u32) -> Entity {
        Entity::new(id, gen)
    }

    #[test]
    fn test_insert_get() {
        let mut storage: ComponentStorage<i32> = ComponentStorage::new();
        let e = make_entity(0, 0);
        storage.insert(e, 42);
        assert_eq!(storage.get(e), Some(&42));
        assert!(storage.contains(e));
        assert_eq!(storage.len(), 1);
    }

    #[test]
    fn test_replace() {
        let mut storage: ComponentStorage<i32> = ComponentStorage::new();
        let e = make_entity(0, 0);
        storage.insert(e, 10);
        let old = storage.remove_and_return(e);
        assert_eq!(old, Some(10));
        storage.insert(e, 20);
        assert_eq!(storage.get(e), Some(&20));
    }

    #[test]
    fn test_remove_swap() {
        let mut storage: ComponentStorage<i32> = ComponentStorage::new();
        let e0 = make_entity(0, 0);
        let e1 = make_entity(1, 0);
        let e2 = make_entity(2, 0);
        storage.insert(e0, 100);
        storage.insert(e1, 200);
        storage.insert(e2, 300);

        let removed = storage.remove_and_return(e1);
        assert_eq!(removed, Some(200));
        assert!(!storage.contains(e1));
        assert!(storage.contains(e0));
        assert!(storage.contains(e2));
        assert_eq!(storage.len(), 2);
        assert_eq!(storage.get(e0), Some(&100));
        assert_eq!(storage.get(e2), Some(&300));
    }

    #[test]
    fn test_iter() {
        let mut storage: ComponentStorage<u32> = ComponentStorage::new();
        let e0 = make_entity(0, 0);
        let e1 = make_entity(1, 0);
        storage.insert(e0, 1);
        storage.insert(e1, 2);

        let mut pairs: Vec<(Entity, u32)> = storage.iter().map(|(&e, &v)| (e, v)).collect();
        pairs.sort_by_key(|(e, _)| e.id());
        assert_eq!(pairs, vec![(e0, 1), (e1, 2)]);
    }

    #[test]
    fn test_get_mut() {
        let mut storage: ComponentStorage<String> = ComponentStorage::new();
        let e = make_entity(5, 0);
        storage.insert(e, "hello".to_string());
        if let Some(val) = storage.get_mut(e) {
            val.push_str(" world");
        }
        assert_eq!(storage.get(e).map(|s| s.as_str()), Some("hello world"));
    }

    #[test]
    fn test_clear() {
        let mut storage: ComponentStorage<i32> = ComponentStorage::new();
        for i in 0..10u32 {
            storage.insert(make_entity(i, 0), i as i32);
        }
        assert_eq!(storage.len(), 10);
        storage.clear();
        assert_eq!(storage.len(), 0);
        for i in 0..10u32 {
            assert!(!storage.contains(make_entity(i, 0)));
        }
    }

    #[test]
    fn test_typed_storage_any_downcast() {
        let mut ts = TypedStorage::<f32>::new();
        let e = make_entity(0, 0);
        ts.borrow_mut().insert(e, 3.14f32);

        let boxed: Box<dyn AnyComponentStorage> = Box::new(ts);
        let typed = boxed.as_any().downcast_ref::<TypedStorage<f32>>().unwrap();
        assert_eq!(typed.borrow().get(e), Some(&3.14f32));
    }

    #[test]
    fn test_storage_set_insert_remove() {
        let mut set = StorageSet::new();
        let e = make_entity(0, 0);
        set.insert_component::<i32>(e, 99);
        assert!(set.contains::<i32>(e));
        let val = set.remove_component::<i32>(e);
        assert_eq!(val, Some(99));
        assert!(!set.contains::<i32>(e));
    }

    #[test]
    fn test_component_mask() {
        let mask = ComponentMask::empty()
            .with::<i32>()
            .with::<f32>()
            .with::<String>();
        assert!(mask.has::<i32>());
        assert!(mask.has::<f32>());
        assert!(mask.has::<String>());
        assert!(!mask.has::<u8>());
        assert_eq!(mask.len(), 3);
    }

    #[test]
    fn test_change_ticks() {
        let mut storage: ComponentStorage<i32> = ComponentStorage::new();
        let e = make_entity(0, 0);
        storage.insert(e, 10);
        let tick_after_insert = storage.change_tick();
        storage.get_mut(e).map(|v| *v = 20);
        let tick_after_mut = storage.change_tick();
        assert!(tick_after_mut > tick_after_insert);

        let changed: Vec<_> = storage.iter_changed_since(tick_after_insert).collect();
        assert_eq!(changed.len(), 1);
    }
}
