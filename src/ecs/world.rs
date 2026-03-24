//! The [`World`] is the central container of the ECS.
//!
//! It owns:
//! - An [`EntityAllocator`] for entity identity management.
//! - A `HashMap<TypeId, Box<dyn AnyComponentStorage>>` for all component data.
//! - A `HashMap<TypeId, Box<dyn Any + Send + Sync>>` for singleton resources.
//!
//! # Usage
//!
//! ```rust,ignore
//! let mut world = World::new();
//! let entity = world.spawn()
//!     .insert(Position { x: 0.0, y: 0.0 })
//!     .insert(Velocity { dx: 1.0, dy: 0.0 })
//!     .id();
//!
//! world.insert_resource(Gravity(9.81));
//! let gravity = world.resource::<Gravity>();
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;

use super::entity::{Entity, EntityAllocator};
use super::storage::{AnyComponentStorage, ComponentStorage, StorageSet, TypedStorage};

// ---------------------------------------------------------------------------
// Component / Resource marker traits
// ---------------------------------------------------------------------------

/// Marker trait for component types. Blanket-implemented for all eligible types.
pub trait Component: 'static + Send + Sync {}
impl<T: 'static + Send + Sync> Component for T {}

/// Marker trait for resource types. Blanket-implemented for all eligible types.
pub trait Resource: 'static + Send + Sync {}
impl<T: 'static + Send + Sync> Resource for T {}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

/// The central ECS world.
///
/// All entity/component/resource data lives here. Systems receive `&mut World`
/// (or access it through a [`crate::ecs::schedule::Schedule`]).
pub struct World {
    /// Entity identity management.
    pub(crate) entities: EntityAllocator,
    /// Component storages, keyed by component TypeId.
    pub(crate) components: HashMap<TypeId, Box<dyn AnyComponentStorage>>,
    /// Singleton resources, keyed by resource TypeId.
    pub(crate) resources: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    /// Monotonically increasing world tick, advanced once per schedule run.
    pub(crate) tick: u64,
    /// Running entity count (mirrors `entities.len()`).
    entity_count: usize,
}

impl World {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create an empty world.
    pub fn new() -> Self {
        Self {
            entities: EntityAllocator::new(),
            components: HashMap::new(),
            resources: HashMap::new(),
            tick: 0,
            entity_count: 0,
        }
    }

    /// Create a world with pre-allocated capacity for `entity_cap` entities.
    pub fn with_capacity(entity_cap: usize) -> Self {
        Self {
            entities: EntityAllocator::with_capacity(entity_cap),
            components: HashMap::new(),
            resources: HashMap::new(),
            tick: 0,
            entity_count: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Entity lifecycle
    // -----------------------------------------------------------------------

    /// Begin building a new entity. Returns an [`EntityBuilder`] for fluent insertion.
    ///
    /// # Example
    /// ```rust,ignore
    /// let e = world.spawn().insert(Health(100)).insert(Name("Bob")).id();
    /// ```
    pub fn spawn(&mut self) -> EntityBuilder<'_> {
        let entity = self.entities.allocate();
        self.entity_count += 1;
        EntityBuilder { world: self, entity }
    }

    /// Spawn an entity with no components. Returns the [`Entity`] handle directly.
    pub fn spawn_empty(&mut self) -> Entity {
        let entity = self.entities.allocate();
        self.entity_count += 1;
        entity
    }

    /// Despawn `entity`, removing all its components. Returns `true` if it was alive.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        // Remove the entity from every component storage.
        for storage in self.components.values_mut() {
            storage.remove_erased(entity);
        }
        let freed = self.entities.free(entity);
        if freed {
            self.entity_count = self.entity_count.saturating_sub(1);
        }
        freed
    }

    /// Returns `true` if `entity` is currently alive.
    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    /// Number of currently live entities.
    #[inline]
    pub fn entity_count(&self) -> usize {
        self.entity_count
    }

    /// Current world tick.
    #[inline]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Advance the world tick.
    pub fn advance_tick(&mut self) {
        self.tick += 1;
    }

    // -----------------------------------------------------------------------
    // Component access
    // -----------------------------------------------------------------------

    /// Insert (or replace) a component on `entity`.
    ///
    /// Returns the old component value if one existed.
    ///
    /// # Panics
    /// Panics in debug mode if `entity` is not alive.
    pub fn insert<T: Component>(&mut self, entity: Entity, component: T) -> Option<T> {
        debug_assert!(self.entities.is_alive(entity), "insert: entity {:?} is not alive", entity);
        self.get_or_create_storage_mut::<T>()
            .borrow_mut()
            .remove_and_return(entity); // discard old, we'll re-insert below

        let old = self.get_or_create_storage_mut::<T>()
            .borrow_mut()
            .insert(entity, component);
        old
    }

    /// Insert a component, returning `&mut Self` for chaining.
    pub fn insert_component<T: Component>(&mut self, entity: Entity, component: T) -> &mut Self {
        self.insert(entity, component);
        self
    }

    /// Remove a component from `entity`. Returns the component if present.
    pub fn remove<T: Component>(&mut self, entity: Entity) -> Option<T> {
        let storage = self.components.get_mut(&TypeId::of::<T>())?;
        let typed = storage
            .as_any_mut()
            .downcast_mut::<TypedStorage<T>>()
            .expect("storage downcast failed");
        typed.borrow_mut().remove_and_return(entity)
    }

    /// Get an immutable reference to the component `T` on `entity`.
    ///
    /// Returns `None` if the entity doesn't have this component.
    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        let storage = self.components.get(&TypeId::of::<T>())?;
        let typed = storage
            .as_any()
            .downcast_ref::<TypedStorage<T>>()
            .expect("storage downcast failed");
        // SAFETY: We borrow the RefCell immutably. The returned reference is
        // tied to the lifetime of `self`, not the `Ref` guard, which is why
        // we use an unsafe pointer cast here. The World owns the storage and
        // ensures no aliased mutable borrows while immutable borrows exist
        // (caller must uphold this through the normal Rust borrow rules on `&self`).
        let guard = typed.borrow();
        let ptr = guard.get(entity)? as *const T;
        // SAFETY: The storage outlives `self`, and no `&mut World` can coexist
        // with this `&World` borrow.
        Some(unsafe { &*ptr })
    }

    /// Get a mutable reference to the component `T` on `entity`.
    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        let storage = self.components.get_mut(&TypeId::of::<T>())?;
        let typed = storage
            .as_any_mut()
            .downcast_mut::<TypedStorage<T>>()
            .expect("storage downcast failed");
        let mut guard = typed.borrow_mut();
        let ptr = guard.get_mut(entity)? as *mut T;
        // SAFETY: exclusive access via &mut self.
        Some(unsafe { &mut *ptr })
    }

    /// Returns `true` if `entity` has component `T`.
    pub fn has<T: Component>(&self, entity: Entity) -> bool {
        self.components
            .get(&TypeId::of::<T>())
            .map(|s| s.contains_erased(entity))
            .unwrap_or(false)
    }

    /// Count how many entities have component `T`.
    pub fn component_count<T: Component>(&self) -> usize {
        self.components
            .get(&TypeId::of::<T>())
            .map(|s| s.len_erased())
            .unwrap_or(0)
    }

    /// Iterate over all entities that have component `T`, yielding `(Entity, &T)`.
    pub fn iter_component<T: Component>(&self) -> impl Iterator<Item = (Entity, &T)> {
        match self.components.get(&TypeId::of::<T>()) {
            None => ComponentIter::empty(),
            Some(storage) => {
                let typed = storage
                    .as_any()
                    .downcast_ref::<TypedStorage<T>>()
                    .expect("storage downcast failed");
                ComponentIter::new(typed)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Resource access
    // -----------------------------------------------------------------------

    /// Insert a singleton resource into the world.
    /// Replaces any existing resource of the same type.
    pub fn insert_resource<T: Resource>(&mut self, resource: T) {
        self.resources.insert(TypeId::of::<T>(), Box::new(resource));
    }

    /// Remove a resource, returning it.
    pub fn remove_resource<T: Resource>(&mut self) -> Option<T> {
        let boxed = self.resources.remove(&TypeId::of::<T>())?;
        Some(*boxed.downcast::<T>().expect("resource downcast failed"))
    }

    /// Get an immutable reference to a resource.
    ///
    /// # Panics
    /// Panics if the resource does not exist. Use [`World::try_resource`] for a fallible version.
    pub fn resource<T: Resource>(&self) -> &T {
        self.try_resource::<T>()
            .unwrap_or_else(|| panic!("resource {} not found", std::any::type_name::<T>()))
    }

    /// Get a mutable reference to a resource.
    ///
    /// # Panics
    /// Panics if the resource does not exist.
    pub fn resource_mut<T: Resource>(&mut self) -> &mut T {
        self.try_resource_mut::<T>()
            .unwrap_or_else(|| panic!("resource {} not found", std::any::type_name::<T>()))
    }

    /// Try to get a resource; returns `None` if absent.
    pub fn try_resource<T: Resource>(&self) -> Option<&T> {
        self.resources
            .get(&TypeId::of::<T>())?
            .downcast_ref::<T>()
    }

    /// Try to get a mutable resource; returns `None` if absent.
    pub fn try_resource_mut<T: Resource>(&mut self) -> Option<&mut T> {
        self.resources
            .get_mut(&TypeId::of::<T>())?
            .downcast_mut::<T>()
    }

    /// Returns `true` if the resource `T` exists.
    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }

    /// Get or insert a resource with a default value.
    pub fn get_resource_or_insert<T: Resource + Default>(&mut self) -> &mut T {
        self.resources
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut::<T>()
            .expect("resource downcast failed")
    }

    // -----------------------------------------------------------------------
    // Querying
    // -----------------------------------------------------------------------

    /// Returns an iterator over all entities and their `T` components.
    /// Equivalent to a single-component query.
    pub fn query_component<T: Component>(&self) -> Vec<(Entity, &T)> {
        self.iter_component::<T>().collect()
    }

    /// Returns entities matching a predicate on component `T`.
    pub fn query_with<T: Component, F: Fn(&T) -> bool>(&self, pred: F) -> Vec<Entity> {
        self.iter_component::<T>()
            .filter(|(_, c)| pred(c))
            .map(|(e, _)| e)
            .collect()
    }

    /// Returns all entities that have all of the given component TypeIds.
    /// Used internally by the query system.
    pub fn entities_with_all(&self, type_ids: &[TypeId]) -> Vec<Entity> {
        if type_ids.is_empty() {
            return self.entities.alive_entities();
        }

        // Start with the smallest storage.
        let smallest = type_ids
            .iter()
            .filter_map(|tid| self.components.get(tid))
            .min_by_key(|s| s.len_erased());

        let Some(first_storage) = smallest else {
            return Vec::new();
        };

        // We can't iterate entities from the erased storage trait directly
        // without adding a method — so we iterate all alive entities and filter.
        // For large worlds this could be optimised with archetype acceleration.
        self.entities
            .iter_alive()
            .filter(|e| {
                type_ids
                    .iter()
                    .all(|tid| self.components.get(tid).map_or(false, |s| s.contains_erased(*e)))
            })
            .collect()
    }

    /// Returns all entities that have component `T`.
    pub fn entities_with<T: Component>(&self) -> Vec<Entity> {
        self.iter_component::<T>().map(|(e, _)| e).collect()
    }

    // -----------------------------------------------------------------------
    // World maintenance
    // -----------------------------------------------------------------------

    /// Despawn all entities. Resources are preserved.
    pub fn clear(&mut self) {
        for storage in self.components.values_mut() {
            storage.clear_erased();
        }
        self.entities.clear();
        self.entity_count = 0;
    }

    /// Despawn all entities and remove all resources.
    pub fn clear_all(&mut self) {
        self.clear();
        self.resources.clear();
    }

    /// Number of component types currently registered.
    pub fn component_type_count(&self) -> usize {
        self.components.len()
    }

    /// Number of resources currently stored.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Get or create the `TypedStorage<T>` for component type `T`.
    pub(crate) fn get_or_create_storage_mut<T: Component>(&mut self) -> &mut TypedStorage<T> {
        let entry = self
            .components
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(TypedStorage::<T>::new()));
        entry
            .as_any_mut()
            .downcast_mut::<TypedStorage<T>>()
            .expect("storage downcast failed")
    }

    /// Get the `TypedStorage<T>` immutably.
    pub(crate) fn get_storage<T: Component>(&self) -> Option<&TypedStorage<T>> {
        self.components
            .get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref::<TypedStorage<T>>()
    }

    /// Get the `TypedStorage<T>` mutably.
    pub(crate) fn get_storage_mut<T: Component>(&mut self) -> Option<&mut TypedStorage<T>> {
        self.components
            .get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut::<TypedStorage<T>>()
    }

    /// Register a component type without inserting any data.
    pub fn register<T: Component>(&mut self) {
        self.components
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(TypedStorage::<T>::new()));
    }

    /// Perform a bulk operation on all components `T`.
    pub fn for_each<T: Component>(&self, mut f: impl FnMut(Entity, &T)) {
        if let Some(storage) = self.get_storage::<T>() {
            let borrowed = storage.borrow();
            for (&e, c) in borrowed.iter() {
                f(e, c);
            }
        }
    }

    /// Perform a bulk mutable operation on all components `T`.
    pub fn for_each_mut<T: Component>(&mut self, mut f: impl FnMut(Entity, &mut T)) {
        if let Some(storage) = self.get_storage_mut::<T>() {
            let mut borrowed = storage.borrow_mut();
            for (&e, c) in borrowed.iter_mut() {
                f(e, c);
            }
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("World")
            .field("entity_count", &self.entity_count)
            .field("component_types", &self.components.len())
            .field("resources", &self.resources.len())
            .field("tick", &self.tick)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// ComponentIter — helper for iter_component
// ---------------------------------------------------------------------------

/// Iterator over `(Entity, &T)` pairs from a component storage.
/// Exists to avoid lifetime issues with the `RefCell` guard.
pub struct ComponentIter<'w, T: Component> {
    // We store a raw pointer to the storage to decouple lifetimes.
    // Safety invariant: the World (and its RefCell<ComponentStorage<T>>) outlives 'w.
    ptr: *const ComponentStorage<T>,
    index: usize,
    _marker: PhantomData<&'w T>,
}

impl<'w, T: Component> ComponentIter<'w, T> {
    fn empty() -> Self {
        Self {
            ptr: std::ptr::null(),
            index: 0,
            _marker: PhantomData,
        }
    }

    fn new(typed: &'w TypedStorage<T>) -> Self {
        let guard = typed.borrow();
        let ptr = &*guard as *const ComponentStorage<T>;
        std::mem::forget(guard); // leak the borrow guard — we'll reborrow as raw pointer
        Self {
            ptr,
            index: 0,
            _marker: PhantomData,
        }
    }
}

impl<'w, T: Component> Iterator for ComponentIter<'w, T> {
    type Item = (Entity, &'w T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr.is_null() {
            return None;
        }
        // SAFETY: ptr is valid for 'w (World is borrowed for 'w), and we only
        // read through it immutably. The RefCell's dynamic borrow is satisfied
        // because we hold `&World` which prevents any mutable borrows.
        let storage = unsafe { &*self.ptr };
        if self.index >= storage.len() {
            return None;
        }
        let entity = *storage.entities().nth(self.index)?;
        let component = storage.get_by_dense_index(self.index);
        self.index += 1;
        Some((entity, component))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.ptr.is_null() {
            return (0, Some(0));
        }
        let storage = unsafe { &*self.ptr };
        let remaining = storage.len() - self.index;
        (remaining, Some(remaining))
    }
}

// ---------------------------------------------------------------------------
// EntityBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for constructing an entity with initial components.
///
/// Created by [`World::spawn`]. Commit by calling [`EntityBuilder::id`].
///
/// ```rust,ignore
/// let entity = world.spawn()
///     .insert(Position::default())
///     .insert(Velocity::default())
///     .id();
/// ```
pub struct EntityBuilder<'w> {
    world: &'w mut World,
    entity: Entity,
}

impl<'w> EntityBuilder<'w> {
    /// Add a component to the entity being built.
    pub fn insert<T: Component>(self, component: T) -> Self {
        // We need to split the borrow: `self.world` and `self.entity`.
        let entity = self.entity;
        self.world.insert(entity, component);
        Self { world: self.world, entity }
    }

    /// Add a component only if a condition is true.
    pub fn insert_if<T: Component>(self, condition: bool, component: impl FnOnce() -> T) -> Self {
        if condition {
            self.insert(component())
        } else {
            self
        }
    }

    /// Finalize the builder and return the entity handle.
    #[inline]
    pub fn id(self) -> Entity {
        self.entity
    }

    /// Get the entity handle without consuming the builder.
    #[inline]
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Despawn this entity before finishing (for conditional logic).
    pub fn despawn(self) {
        let entity = self.entity;
        self.world.despawn(entity);
    }
}

// ---------------------------------------------------------------------------
// WorldCell — shared-reference access pattern (for parallel system stubs)
// ---------------------------------------------------------------------------

/// A wrapper around `*mut World` for use in system parameters.
/// Not actually thread-safe — this is a single-threaded proof-of-concept.
pub struct WorldCell {
    world: *mut World,
}

impl WorldCell {
    /// SAFETY: caller must ensure exclusive access for the duration.
    pub unsafe fn new(world: &mut World) -> Self {
        Self { world: world as *mut World }
    }

    pub fn get(&self) -> &World {
        unsafe { &*self.world }
    }

    pub fn get_mut(&self) -> &mut World {
        unsafe { &mut *self.world }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Component types for testing
    #[derive(Debug, Clone, PartialEq)]
    struct Position { x: f32, y: f32 }

    #[derive(Debug, Clone, PartialEq)]
    struct Velocity { dx: f32, dy: f32 }

    #[derive(Debug, Clone, PartialEq)]
    struct Health(i32);

    #[derive(Debug, Clone, PartialEq)]
    struct Name(String);

    #[derive(Debug, Clone, PartialEq)]
    struct Gravity(f32);

    #[test]
    fn test_spawn_and_get() {
        let mut world = World::new();
        let e = world.spawn()
            .insert(Position { x: 1.0, y: 2.0 })
            .insert(Health(100))
            .id();

        assert!(world.is_alive(e));
        assert_eq!(world.get::<Position>(e), Some(&Position { x: 1.0, y: 2.0 }));
        assert_eq!(world.get::<Health>(e), Some(&Health(100)));
        assert_eq!(world.get::<Velocity>(e), None);
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn test_despawn() {
        let mut world = World::new();
        let e = world.spawn().insert(Position { x: 0.0, y: 0.0 }).id();
        assert!(world.is_alive(e));
        assert!(world.despawn(e));
        assert!(!world.is_alive(e));
        assert_eq!(world.entity_count(), 0);
        // Double-despawn is safe.
        assert!(!world.despawn(e));
    }

    #[test]
    fn test_insert_and_remove_component() {
        let mut world = World::new();
        let e = world.spawn_empty();
        world.insert(e, Health(50));
        assert_eq!(world.get::<Health>(e), Some(&Health(50)));

        let old = world.remove::<Health>(e);
        assert_eq!(old, Some(Health(50)));
        assert!(!world.has::<Health>(e));
    }

    #[test]
    fn test_get_mut() {
        let mut world = World::new();
        let e = world.spawn().insert(Health(10)).id();
        if let Some(h) = world.get_mut::<Health>(e) {
            h.0 += 5;
        }
        assert_eq!(world.get::<Health>(e), Some(&Health(15)));
    }

    #[test]
    fn test_resources() {
        let mut world = World::new();
        world.insert_resource(Gravity(9.81));
        assert!(world.has_resource::<Gravity>());
        assert_eq!(world.resource::<Gravity>(), &Gravity(9.81));
        world.resource_mut::<Gravity>().0 = 1.62;
        assert_eq!(world.resource::<Gravity>().0, 1.62);
        let removed = world.remove_resource::<Gravity>();
        assert_eq!(removed, Some(Gravity(1.62)));
        assert!(!world.has_resource::<Gravity>());
    }

    #[test]
    fn test_for_each() {
        let mut world = World::new();
        for i in 0..5i32 {
            world.spawn().insert(Health(i * 10)).insert(Position { x: i as f32, y: 0.0 });
        }
        let mut sum = 0;
        world.for_each::<Health>(|_e, h| { sum += h.0; });
        assert_eq!(sum, 0 + 10 + 20 + 30 + 40);
    }

    #[test]
    fn test_for_each_mut() {
        let mut world = World::new();
        for i in 0..3i32 {
            world.spawn().insert(Health(i));
        }
        world.for_each_mut::<Health>(|_e, h| { h.0 *= 2; });
        let values: Vec<i32> = world
            .entities_with::<Health>()
            .iter()
            .map(|&e| world.get::<Health>(e).unwrap().0)
            .collect();
        let mut sorted = values.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 2, 4]);
    }

    #[test]
    fn test_iter_component() {
        let mut world = World::new();
        let e1 = world.spawn().insert(Name("Alice".to_string())).id();
        let e2 = world.spawn().insert(Name("Bob".to_string())).id();

        let names: Vec<_> = world.iter_component::<Name>()
            .map(|(e, n)| (e, n.0.clone()))
            .collect();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_entities_with_all() {
        let mut world = World::new();
        let e1 = world.spawn().insert(Position { x: 0.0, y: 0.0 }).insert(Velocity { dx: 1.0, dy: 0.0 }).id();
        let e2 = world.spawn().insert(Position { x: 1.0, y: 1.0 }).id(); // no Velocity
        let e3 = world.spawn().insert(Velocity { dx: 2.0, dy: 0.0 }).id(); // no Position

        let matches = world.entities_with_all(&[TypeId::of::<Position>(), TypeId::of::<Velocity>()]);
        assert_eq!(matches.len(), 1);
        assert!(matches.contains(&e1));
        assert!(!matches.contains(&e2));
        assert!(!matches.contains(&e3));
    }

    #[test]
    fn test_clear() {
        let mut world = World::new();
        for _ in 0..10 {
            world.spawn().insert(Health(1));
        }
        assert_eq!(world.entity_count(), 10);
        world.clear();
        assert_eq!(world.entity_count(), 0);
        assert_eq!(world.component_count::<Health>(), 0);
    }

    #[test]
    fn test_component_replace() {
        let mut world = World::new();
        let e = world.spawn().insert(Health(10)).id();
        world.insert(e, Health(99));
        assert_eq!(world.get::<Health>(e), Some(&Health(99)));
        assert_eq!(world.component_count::<Health>(), 1);
    }

    #[test]
    fn test_get_resource_or_insert_default() {
        #[derive(Default, PartialEq, Debug)]
        struct Counter(u32);

        let mut world = World::new();
        {
            let c = world.get_resource_or_insert::<Counter>();
            assert_eq!(c, &Counter(0));
            c.0 = 5;
        }
        assert_eq!(world.resource::<Counter>(), &Counter(5));
    }

    #[test]
    fn test_spawn_many() {
        let mut world = World::new();
        let mut entities = Vec::new();
        for i in 0..100i32 {
            let e = world.spawn().insert(Health(i)).id();
            entities.push(e);
        }
        assert_eq!(world.entity_count(), 100);
        // Despawn half.
        for &e in entities.iter().step_by(2) {
            world.despawn(e);
        }
        assert_eq!(world.entity_count(), 50);
    }

    #[test]
    fn test_entity_builder_insert_if() {
        let mut world = World::new();
        let e = world.spawn()
            .insert(Health(10))
            .insert_if(false, || Velocity { dx: 1.0, dy: 0.0 })
            .insert_if(true, || Position { x: 5.0, y: 5.0 })
            .id();

        assert!(world.has::<Health>(e));
        assert!(!world.has::<Velocity>(e));
        assert!(world.has::<Position>(e));
    }
}
