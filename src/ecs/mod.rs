//! # Entity Component System (ECS)
//!
//! A standalone archetype-based ECS implementation for the Proof Engine.
//! Components are grouped by their type signature into `Archetype` tables,
//! providing cache-friendly iteration and efficient query dispatch.
//!
//! ## Core Concepts
//!
//! - [`Entity`]: A generational ID (index + generation) representing a live object.
//! - [`World`]: The central store for all entities, components, resources, and events.
//! - [`Archetype`]: A table of entities that all share the same set of component types.
//! - [`QueryIter`]: An iterator over entities matching a given component filter.
//! - [`Commands`]: Deferred mutations (spawn / despawn / insert / remove).
//! - [`Schedule`]: An ordered list of [`System`]s with labels and run conditions.
//! - [`Events<E>`]: A typed event queue with independent reader cursors.
//! - [`Resources`]: A type-map for global singleton data.
//!
//! ## Design notes
//!
//! Components are fetched using marker wrapper types:
//! - [`Read<T>`]  — yields `&T`
//! - [`Write<T>`] — yields `&mut T`
//! - [`OptionRead<T>`] — yields `Option<&T>` (entity need not have `T`)
//!
//! ## Example
//!
//! ```rust
//! use proof_engine::ecs::*;
//!
//! #[derive(Clone)]
//! struct Position { x: f32, y: f32 }
//! impl Component for Position {}
//!
//! #[derive(Clone)]
//! struct Velocity { dx: f32, dy: f32 }
//! impl Component for Velocity {}
//!
//! let mut world = World::new();
//! let e = world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { dx: 1.0, dy: 0.0 }));
//!
//! // iterate all entities with Position + Velocity
//! for (pos, vel) in world.query::<(Read<Position>, Read<Velocity>), ()>() {
//!     let _ = (pos.x + vel.dx, pos.y + vel.dy);
//! }
//! ```

use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

// ─────────────────────────────────────────────────────────────────────────────
// Component trait
// ─────────────────────────────────────────────────────────────────────────────

/// Marker trait for all ECS components.
///
/// Any type that is `'static + Send + Sync + Clone` can be a component.
/// Derive or manually implement this trait to opt a type into the ECS.
///
/// # Example
/// ```rust
/// use proof_engine::ecs::Component;
///
/// #[derive(Clone)]
/// struct Health(f32);
/// impl Component for Health {}
/// ```
pub trait Component: 'static + Send + Sync + Clone {}

// ─────────────────────────────────────────────────────────────────────────────
// Entity
// ─────────────────────────────────────────────────────────────────────────────

/// A lightweight generational handle to an entity.
///
/// The `index` field is the slot in the entity allocator. The `generation`
/// field is incremented each time that slot is reused after a despawn, so
/// holding an old `Entity` with a stale generation will correctly fail
/// liveness checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    /// Slot index in the entity table.
    pub index: u32,
    /// Generation counter — incremented each time the slot is recycled.
    pub generation: u32,
}

impl Entity {
    /// Creates an entity handle from raw parts. Prefer using [`World::spawn`].
    #[inline]
    pub fn from_raw(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Returns a sentinel "null" entity that is never considered alive.
    #[inline]
    pub fn null() -> Self {
        Self { index: u32::MAX, generation: u32::MAX }
    }

    /// Returns `true` if this is the null sentinel value.
    #[inline]
    pub fn is_null(self) -> bool {
        self.index == u32::MAX
    }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({}v{})", self.index, self.generation)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entity allocator (generational free-list)
// ─────────────────────────────────────────────────────────────────────────────

/// Internal record kept per entity slot.
#[derive(Debug, Clone)]
struct EntityRecord {
    generation: u32,
    /// Where this entity's component data lives, if it is alive.
    location: Option<EntityLocation>,
}

/// Identifies exactly where an entity's row of component data is stored.
#[derive(Debug, Clone, Copy)]
struct EntityLocation {
    /// Index into [`World::archetypes`].
    archetype_id: ArchetypeId,
    /// Row within that archetype's column storage.
    row: usize,
}

/// Dense generational free-list entity allocator.
#[derive(Debug, Default)]
struct EntityAllocator {
    records: Vec<EntityRecord>,
    free: Vec<u32>,
}

impl EntityAllocator {
    /// Allocates a new entity, reusing a freed slot when possible.
    fn alloc(&mut self) -> Entity {
        if let Some(index) = self.free.pop() {
            let rec = &mut self.records[index as usize];
            Entity { index, generation: rec.generation }
        } else {
            let index = self.records.len() as u32;
            self.records.push(EntityRecord { generation: 0, location: None });
            Entity { index, generation: 0 }
        }
    }

    /// Frees an entity slot, bumping its generation so old handles become stale.
    fn free(&mut self, entity: Entity) {
        let rec = &mut self.records[entity.index as usize];
        rec.generation = rec.generation.wrapping_add(1);
        rec.location = None;
        self.free.push(entity.index);
    }

    /// Returns `true` if the entity handle matches a live slot.
    fn is_alive(&self, entity: Entity) -> bool {
        self.records
            .get(entity.index as usize)
            .map(|r| r.generation == entity.generation && r.location.is_some())
            .unwrap_or(false)
    }

    /// Returns the current storage location for a live entity.
    fn location(&self, entity: Entity) -> Option<EntityLocation> {
        self.records
            .get(entity.index as usize)
            .filter(|r| r.generation == entity.generation)
            .and_then(|r| r.location)
    }

    /// Updates the storage location for a live entity.
    fn set_location(&mut self, entity: Entity, loc: EntityLocation) {
        if let Some(rec) = self.records.get_mut(entity.index as usize) {
            if rec.generation == entity.generation {
                rec.location = Some(loc);
            }
        }
    }

    /// Clears the storage location without bumping the generation.
    fn clear_location(&mut self, entity: Entity) {
        if let Some(rec) = self.records.get_mut(entity.index as usize) {
            if rec.generation == entity.generation {
                rec.location = None;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AnyVec — type-erased heap vector
// ─────────────────────────────────────────────────────────────────────────────

/// A type-erased, heap-allocated `Vec` that stores values of a single concrete
/// type. All operations are performed through function pointers captured at
/// construction time, so the concrete type does not need to be known at call
/// sites that manipulate the vector generically.
struct AnyVec {
    /// The concrete component `TypeId`.
    type_id: TypeId,
    /// Number of live elements.
    len: usize,
    /// Raw byte storage (size = element_size * capacity).
    data: Vec<u8>,
    /// Size in bytes of one element.
    element_size: usize,
    /// Runs the destructor for the element at the given pointer.
    drop_fn: unsafe fn(*mut u8),
    /// Clones the element at `src` and writes the clone to `dst`.
    clone_fn: unsafe fn(*const u8, *mut u8),
}

unsafe impl Send for AnyVec {}
unsafe impl Sync for AnyVec {}

impl AnyVec {
    /// Constructs a new empty `AnyVec` for component type `T`.
    fn new<T: Component>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            len: 0,
            data: Vec::new(),
            element_size: std::mem::size_of::<T>(),
            drop_fn: |ptr| unsafe { std::ptr::drop_in_place(ptr as *mut T) },
            clone_fn: |src, dst| unsafe {
                let cloned = (&*(src as *const T)).clone();
                std::ptr::write(dst as *mut T, cloned);
            },
        }
    }

    /// Ensures the backing buffer can hold at least `min_cap` elements.
    fn reserve_one(&mut self) {
        let size = self.element_size;
        if size == 0 {
            return;
        }
        let needed = (self.len + 1) * size;
        if self.data.len() < needed {
            let new_cap = needed.max(self.data.len() * 2).max(size * 4);
            self.data.resize(new_cap, 0);
        }
    }

    /// Appends `value` of type `T` to the end.
    fn push<T: Component>(&mut self, value: T) {
        debug_assert_eq!(TypeId::of::<T>(), self.type_id);
        self.reserve_one();
        unsafe {
            let dst = self.data.as_mut_ptr().add(self.len * self.element_size);
            std::ptr::write(dst as *mut T, value);
        }
        self.len += 1;
    }

    /// Returns a shared reference to the element at `row`.
    fn get<T: Component>(&self, row: usize) -> &T {
        debug_assert!(row < self.len);
        unsafe { &*(self.data.as_ptr().add(row * self.element_size) as *const T) }
    }

    /// Returns a mutable reference to the element at `row`.
    fn get_mut<T: Component>(&mut self, row: usize) -> &mut T {
        debug_assert!(row < self.len);
        unsafe { &mut *(self.data.as_mut_ptr().add(row * self.element_size) as *mut T) }
    }

    /// Returns a raw const pointer to the element at `row`.
    fn get_ptr(&self, row: usize) -> *const u8 {
        debug_assert!(row < self.len);
        unsafe { self.data.as_ptr().add(row * self.element_size) }
    }

    /// Swap-removes the element at `row`, dropping the evicted value.
    ///
    /// The last element is moved into `row`. Call sites are responsible for
    /// updating entity-location bookkeeping.
    fn swap_remove_raw(&mut self, row: usize) {
        assert!(row < self.len);
        let last = self.len - 1;
        let size = self.element_size;
        unsafe {
            let dst = self.data.as_mut_ptr().add(row * size);
            // Drop the element being removed.
            (self.drop_fn)(dst);
            if row != last {
                // Bit-copy the last element into the vacated slot.
                let src = self.data.as_ptr().add(last * size);
                std::ptr::copy_nonoverlapping(src, dst, size);
            }
        }
        self.len -= 1;
    }
}

impl Drop for AnyVec {
    fn drop(&mut self) {
        let size = self.element_size;
        for i in 0..self.len {
            unsafe {
                let ptr = self.data.as_mut_ptr().add(i * size);
                (self.drop_fn)(ptr);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ComponentStorage — column with change-detection ticks
// ─────────────────────────────────────────────────────────────────────────────

/// A single component column within an [`Archetype`].
///
/// Wraps an [`AnyVec`] data column and tracks per-row change-detection ticks:
/// - `added_ticks[row]` — the world tick when the component was first inserted.
/// - `changed_ticks[row]` — the world tick of the most recent mutable access.
struct ComponentStorage {
    column: AnyVec,
    /// Tick at which each row was added (for `Added<T>` detection).
    added_ticks: Vec<u32>,
    /// Tick at which each row was last mutably touched (for `Changed<T>`).
    changed_ticks: Vec<u32>,
}

impl ComponentStorage {
    /// Creates a new empty column for component type `T`.
    fn new<T: Component>() -> Self {
        Self {
            column: AnyVec::new::<T>(),
            added_ticks: Vec::new(),
            changed_ticks: Vec::new(),
        }
    }

    /// Appends a value, recording `tick` as both the added and changed tick.
    fn push<T: Component>(&mut self, value: T, tick: u32) {
        self.column.push(value);
        self.added_ticks.push(tick);
        self.changed_ticks.push(tick);
    }

    /// Swap-removes row `row`, keeping tick vectors in sync.
    fn swap_remove(&mut self, row: usize) {
        self.column.swap_remove_raw(row);
        let last = self.added_ticks.len() - 1;
        self.added_ticks.swap(row, last);
        self.added_ticks.pop();
        let last = self.changed_ticks.len() - 1;
        self.changed_ticks.swap(row, last);
        self.changed_ticks.pop();
    }

    /// Clones the element at `row` into `dst`, pushing it as a new row.
    fn clone_row_into(&self, row: usize, dst: &mut ComponentStorage, tick: u32) {
        // Grow dst backing buffer if needed.
        let size = self.column.element_size;
        let needed = (dst.column.len + 1) * size;
        if dst.column.data.len() < needed {
            let new_cap = needed.max(dst.column.data.len() * 2).max(size * 4);
            dst.column.data.resize(new_cap, 0);
        }
        unsafe {
            let src_ptr = self.column.data.as_ptr().add(row * size);
            let dst_ptr = dst.column.data.as_mut_ptr().add(dst.column.len * size);
            (self.column.clone_fn)(src_ptr, dst_ptr);
        }
        dst.column.len += 1;
        dst.added_ticks.push(self.added_ticks[row]);
        dst.changed_ticks.push(tick);
    }

    fn len(&self) -> usize {
        self.column.len
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Archetype
// ─────────────────────────────────────────────────────────────────────────────

/// A unique index into [`World::archetypes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchetypeId(pub u32);

/// An archetype stores all entities that share *exactly* the same set of
/// component types.
///
/// Components are stored in dense, parallel columns (`HashMap<TypeId,
/// ComponentStorage>`), one per component type. A parallel `entities` vec
/// records which [`Entity`] lives at each row, enabling O(1) row → entity
/// lookup.
pub struct Archetype {
    /// Unique id for this archetype.
    pub id: ArchetypeId,
    /// The sorted, deduplicated set of `TypeId`s that define this archetype.
    pub component_types: Vec<TypeId>,
    /// One column per component type.
    columns: HashMap<TypeId, ComponentStorage>,
    /// Entity at each row.
    pub entities: Vec<Entity>,
}

impl Archetype {
    fn new(id: ArchetypeId, component_types: Vec<TypeId>) -> Self {
        Self {
            id,
            component_types,
            columns: HashMap::new(),
            entities: Vec::new(),
        }
    }

    /// Registers a component column for `T`. No-op if already registered.
    fn register_column<T: Component>(&mut self) {
        self.columns
            .entry(TypeId::of::<T>())
            .or_insert_with(ComponentStorage::new::<T>);
    }

    /// Returns `true` if this archetype has a column for `type_id`.
    pub fn contains(&self, type_id: TypeId) -> bool {
        self.columns.contains_key(&type_id)
    }

    fn column(&self, type_id: TypeId) -> Option<&ComponentStorage> {
        self.columns.get(&type_id)
    }

    fn column_mut(&mut self, type_id: TypeId) -> Option<&mut ComponentStorage> {
        self.columns.get_mut(&type_id)
    }

    /// Number of entities stored in this archetype.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Returns `true` if no entities are stored.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Swap-removes the entity at `row`.
    ///
    /// Returns the entity that was swapped *into* `row` (i.e. the previous
    /// last entity), or `None` if `row` was already the last row.
    fn swap_remove(&mut self, row: usize) -> Option<Entity> {
        let last = self.entities.len() - 1;
        for col in self.columns.values_mut() {
            col.swap_remove(row);
        }
        self.entities.swap(row, last);
        self.entities.pop();
        if row < self.entities.len() {
            Some(self.entities[row])
        } else {
            None
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bundle
// ─────────────────────────────────────────────────────────────────────────────

/// A heterogeneous collection of components that can be inserted together.
///
/// The ECS provides blanket implementations for tuples up to 8 elements.
/// You can also implement `Bundle` manually for named structs.
pub trait Bundle: 'static + Send + Sync {
    /// Returns the sorted `TypeId`s of every component in this bundle.
    fn type_ids() -> Vec<TypeId>;

    /// Registers all columns required by this bundle on `arch`.
    fn register_columns(arch: &mut Archetype);

    /// Pushes all components into `arch`, consuming `self`.
    fn insert_into(self, arch: &mut Archetype, tick: u32);
}

// Single-component bundle
impl<C: Component> Bundle for C {
    fn type_ids() -> Vec<TypeId> {
        vec![TypeId::of::<C>()]
    }
    fn register_columns(arch: &mut Archetype) {
        arch.register_column::<C>();
    }
    fn insert_into(self, arch: &mut Archetype, tick: u32) {
        arch.columns
            .get_mut(&TypeId::of::<C>())
            .expect("column not registered")
            .push(self, tick);
    }
}

/// Macro generating `Bundle` implementations for N-tuples.
macro_rules! impl_bundle_tuple {
    ($($C:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($C: Component),*> Bundle for ($($C,)*) {
            fn type_ids() -> Vec<TypeId> {
                let mut ids = vec![$(TypeId::of::<$C>()),*];
                ids.sort();
                ids.dedup();
                ids
            }
            fn register_columns(arch: &mut Archetype) {
                $(arch.register_column::<$C>();)*
            }
            fn insert_into(self, arch: &mut Archetype, tick: u32) {
                let ($($C,)*) = self;
                $(
                    arch.columns
                        .get_mut(&TypeId::of::<$C>())
                        .expect("column not registered")
                        .push($C, tick);
                )*
            }
        }
    };
}

impl_bundle_tuple!(A, B);
impl_bundle_tuple!(A, B, C);
impl_bundle_tuple!(A, B, C, D);
impl_bundle_tuple!(A, B, C, D, E);
impl_bundle_tuple!(A, B, C, D, E, F);
impl_bundle_tuple!(A, B, C, D, E, F, G);
impl_bundle_tuple!(A, B, C, D, E, F, G, H);

// ─────────────────────────────────────────────────────────────────────────────
// Resources
// ─────────────────────────────────────────────────────────────────────────────

/// A type-map holding global singleton resources.
///
/// Resources are retrieved by type and are independent of any entity.
///
/// # Example
/// ```rust
/// use proof_engine::ecs::Resources;
///
/// struct DeltaTime(f32);
///
/// let mut res = Resources::new();
/// res.insert(DeltaTime(0.016));
/// assert!((res.get::<DeltaTime>().unwrap().0 - 0.016).abs() < 1e-6);
/// ```
#[derive(Default)]
pub struct Resources {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Resources {
    /// Creates an empty resource map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a resource, replacing any existing value of the same type.
    pub fn insert<T: 'static + Send + Sync>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Returns a shared reference to the resource of type `T`.
    pub fn get<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref::<T>()
    }

    /// Returns a mutable reference to the resource of type `T`.
    pub fn get_mut<T: 'static + Send + Sync>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>())?.downcast_mut::<T>()
    }

    /// Removes and returns the resource of type `T`.
    pub fn remove<T: 'static + Send + Sync>(&mut self) -> Option<T> {
        self.map
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok())
            .map(|b| *b)
    }

    /// Returns `true` if a resource of type `T` is present.
    pub fn contains<T: 'static + Send + Sync>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Res<T> / ResMut<T>
// ─────────────────────────────────────────────────────────────────────────────

/// An immutable borrow of a resource `T`.
///
/// Returned by [`World::resource`] and usable as a [`SystemParam`].
pub struct Res<'a, T: 'static + Send + Sync> {
    inner: &'a T,
}
impl<'a, T: 'static + Send + Sync> Res<'a, T> {
    /// Wraps a shared reference as a `Res`.
    pub fn new(inner: &'a T) -> Self {
        Self { inner }
    }
}
impl<'a, T: 'static + Send + Sync> std::ops::Deref for Res<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.inner
    }
}

/// A mutable borrow of a resource `T`.
///
/// Returned by [`World::resource_mut`] and usable as a [`SystemParam`].
pub struct ResMut<'a, T: 'static + Send + Sync> {
    inner: &'a mut T,
}
impl<'a, T: 'static + Send + Sync> ResMut<'a, T> {
    /// Wraps a mutable reference as a `ResMut`.
    pub fn new(inner: &'a mut T) -> Self {
        Self { inner }
    }
}
impl<'a, T: 'static + Send + Sync> std::ops::Deref for ResMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.inner
    }
}
impl<'a, T: 'static + Send + Sync> std::ops::DerefMut for ResMut<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Local<T>
// ─────────────────────────────────────────────────────────────────────────────

/// System-local state that persists across invocations of the same system
/// instance but is invisible outside the system.
///
/// Unlike a resource, each system instance has its own `Local<T>`. The
/// value is initialised with `T::default()` on first use.
///
/// # Example
/// ```rust
/// use proof_engine::ecs::Local;
///
/// let mut counter: Local<u32> = Local::default_value();
/// *counter += 1;
/// assert_eq!(*counter, 1);
/// ```
pub struct Local<T: Default + 'static> {
    value: T,
}
impl<T: Default + 'static> Local<T> {
    /// Creates a `Local` wrapping `value`.
    pub fn new(value: T) -> Self {
        Self { value }
    }
    /// Creates a `Local` using `T::default()`.
    pub fn default_value() -> Self {
        Self { value: T::default() }
    }
}
impl<T: Default + 'static> std::ops::Deref for Local<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T: Default + 'static> std::ops::DerefMut for Local<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Events<E>
// ─────────────────────────────────────────────────────────────────────────────

/// A typed double-buffered event queue.
///
/// Events are written via [`EventWriter`] and consumed via [`EventReader`] or
/// manual [`EventCursor`] iteration. The queue is double-buffered so that
/// events survive for at least one full frame (one call to [`Events::update`]).
///
/// # Example
/// ```rust
/// use proof_engine::ecs::{Events, EventWriter, EventReader};
///
/// struct Explosion { pos: (f32, f32) }
///
/// let mut events: Events<Explosion> = Events::new();
/// events.send(Explosion { pos: (1.0, 2.0) });
/// ```
pub struct Events<E: 'static + Send + Sync> {
    /// Two alternating buffers: buffers[current] receives new events.
    buffers: [Vec<E>; 2],
    /// Index of the buffer currently being written (0 or 1).
    current: usize,
    /// Monotonically increasing count of total events ever sent.
    event_count: usize,
    /// `event_count` value at the start of the current double-buffer epoch.
    start_event_count: usize,
}

impl<E: 'static + Send + Sync> Default for Events<E> {
    fn default() -> Self {
        Self {
            buffers: [Vec::new(), Vec::new()],
            current: 0,
            event_count: 0,
            start_event_count: 0,
        }
    }
}

impl<E: 'static + Send + Sync> Events<E> {
    /// Creates an empty event queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a single event.
    pub fn send(&mut self, event: E) {
        self.buffers[self.current].push(event);
        self.event_count += 1;
    }

    /// Advances the double-buffer: swaps to the other buffer and clears the
    /// stale one. Call once per frame.
    pub fn update(&mut self) {
        let next = 1 - self.current;
        self.buffers[next].clear();
        self.start_event_count =
            self.event_count - self.buffers[self.current].len();
        self.current = next;
    }

    /// Returns a cursor that starts at the current end (reads only future events).
    pub fn get_reader(&self) -> EventCursor {
        EventCursor { last_event_count: self.event_count }
    }

    /// Returns a cursor that starts at the oldest buffered event.
    pub fn get_reader_current(&self) -> EventCursor {
        EventCursor { last_event_count: self.start_event_count }
    }

    /// Reads all events since `cursor` and advances `cursor`.
    pub fn read<'a>(&'a self, cursor: &mut EventCursor) -> impl Iterator<Item = &'a E> {
        let start = cursor.last_event_count;
        cursor.last_event_count = self.event_count;

        let buf_old = &self.buffers[1 - self.current];
        let buf_new = &self.buffers[self.current];
        let base = self.start_event_count;

        let skip0 = start.saturating_sub(base);
        let take0 = buf_old.len().saturating_sub(skip0);
        let skip1 = start.saturating_sub(base + buf_old.len());
        let take1 = buf_new.len().saturating_sub(skip1);

        buf_old
            .iter()
            .skip(skip0)
            .take(take0)
            .chain(buf_new.iter().skip(skip1).take(take1))
    }

    /// Total events sent since creation.
    pub fn len(&self) -> usize {
        self.event_count
    }

    /// `true` if both buffers are empty.
    pub fn is_empty(&self) -> bool {
        self.buffers[0].is_empty() && self.buffers[1].is_empty()
    }

    /// Clears all buffered events.
    pub fn clear(&mut self) {
        self.buffers[0].clear();
        self.buffers[1].clear();
    }
}

/// An independent read cursor into an [`Events<E>`] queue.
///
/// Each system that reads events should maintain its own `EventCursor` so that
/// multiple systems can each read the same events independently.
#[derive(Debug, Clone, Default)]
pub struct EventCursor {
    last_event_count: usize,
}

/// A write handle for an [`Events<E>`] queue.
pub struct EventWriter<'a, E: 'static + Send + Sync> {
    events: &'a mut Events<E>,
}
impl<'a, E: 'static + Send + Sync> EventWriter<'a, E> {
    /// Creates a new writer.
    pub fn new(events: &'a mut Events<E>) -> Self {
        Self { events }
    }
    /// Sends a single event.
    pub fn send(&mut self, event: E) {
        self.events.send(event);
    }
    /// Sends all events from an iterator.
    pub fn send_batch(&mut self, events: impl IntoIterator<Item = E>) {
        for e in events {
            self.events.send(e);
        }
    }
}

/// A read handle for an [`Events<E>`] queue with its own [`EventCursor`].
pub struct EventReader<'a, E: 'static + Send + Sync> {
    events: &'a Events<E>,
    cursor: EventCursor,
}
impl<'a, E: 'static + Send + Sync> EventReader<'a, E> {
    /// Creates a reader that sees only events sent *after* this point.
    pub fn new(events: &'a Events<E>) -> Self {
        Self { cursor: events.get_reader(), events }
    }
    /// Creates a reader that sees all currently buffered events.
    pub fn new_current(events: &'a Events<E>) -> Self {
        Self { cursor: events.get_reader_current(), events }
    }
    /// Returns an iterator over all unread events, advancing the cursor.
    pub fn read(&mut self) -> impl Iterator<Item = &E> {
        self.events.read(&mut self.cursor)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

/// The kind of a deferred world command.
enum CommandKind {
    Spawn(Box<dyn FnOnce(&mut World) + Send + Sync>),
    Despawn(Entity),
    InsertComponent(Box<dyn FnOnce(&mut World) + Send + Sync>),
    RemoveComponent(Entity, TypeId),
    InsertResource(Box<dyn FnOnce(&mut World) + Send + Sync>),
    RemoveResource(TypeId),
}

/// A queue of deferred world mutations.
///
/// Systems accumulate structural changes (spawn, despawn, insert, remove) into
/// a `Commands` queue and apply them in bulk after the system finishes. This
/// avoids holding exclusive borrows on the world during iteration.
///
/// # Example
/// ```rust
/// use proof_engine::ecs::{World, Commands, Component};
///
/// #[derive(Clone)] struct Marker;
/// impl Component for Marker {}
///
/// let mut world = World::new();
/// let mut commands = Commands::new();
/// commands.spawn(Marker);
/// commands.apply(&mut world);
/// assert_eq!(world.entity_count(), 1);
/// ```
pub struct Commands {
    queue: Vec<CommandKind>,
}

impl Commands {
    /// Creates an empty command queue.
    pub fn new() -> Self {
        Self { queue: Vec::new() }
    }

    /// Queues a bundle spawn.
    pub fn spawn<B: Bundle>(&mut self, bundle: B) {
        self.queue.push(CommandKind::Spawn(Box::new(move |w| {
            w.spawn(bundle);
        })));
    }

    /// Queues a despawn.
    pub fn despawn(&mut self, entity: Entity) {
        self.queue.push(CommandKind::Despawn(entity));
    }

    /// Queues insertion of component `C` onto `entity`.
    pub fn insert<C: Component>(&mut self, entity: Entity, component: C) {
        self.queue.push(CommandKind::InsertComponent(Box::new(move |w| {
            w.insert(entity, component);
        })));
    }

    /// Queues removal of component `C` from `entity`.
    pub fn remove<C: Component>(&mut self, entity: Entity) {
        self.queue.push(CommandKind::RemoveComponent(entity, TypeId::of::<C>()));
    }

    /// Queues insertion of a resource.
    pub fn insert_resource<T: 'static + Send + Sync>(&mut self, value: T) {
        self.queue.push(CommandKind::InsertResource(Box::new(move |w| {
            w.resources.insert(value);
        })));
    }

    /// Queues removal of a resource.
    pub fn remove_resource<T: 'static + Send + Sync>(&mut self) {
        self.queue.push(CommandKind::RemoveResource(TypeId::of::<T>()));
    }

    /// Applies all queued commands to `world` in order and clears the queue.
    pub fn apply(&mut self, world: &mut World) {
        for cmd in self.queue.drain(..) {
            match cmd {
                CommandKind::Spawn(f) => f(world),
                CommandKind::Despawn(e) => { world.despawn(e); }
                CommandKind::InsertComponent(f) => f(world),
                CommandKind::RemoveComponent(e, tid) => world.remove_by_type_id(e, tid),
                CommandKind::InsertResource(f) => f(world),
                CommandKind::RemoveResource(tid) => { world.resources.map.remove(&tid); }
            }
        }
    }

    /// Returns `true` if no commands are queued.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl Default for Commands {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Query filters
// ─────────────────────────────────────────────────────────────────────────────

/// A filter requiring an entity to have component `T` (without reading it).
///
/// # Example
/// ```rust,ignore
/// world.query::<Read<Position>, With<Player>>()
/// ```
pub struct With<T: Component>(std::marker::PhantomData<T>);

/// A filter requiring an entity to *not* have component `T`.
///
/// # Example
/// ```rust,ignore
/// world.query::<Read<Health>, Without<Dead>>()
/// ```
pub struct Without<T: Component>(std::marker::PhantomData<T>);

/// A structural change-detection filter — passes only entities where `T` was
/// added since `since_tick`. Used with [`query_added`].
pub struct Added<T: Component>(std::marker::PhantomData<T>);

/// A structural change-detection filter — passes only entities where `T` was
/// mutably accessed since `since_tick`. Used with [`query_changed`].
pub struct Changed<T: Component>(std::marker::PhantomData<T>);

/// Trait implemented by query filter types to test whether an archetype matches.
pub trait QueryFilter {
    /// Returns `true` if the given archetype passes this filter.
    fn matches_archetype(arch: &Archetype) -> bool;
}

impl QueryFilter for () {
    fn matches_archetype(_: &Archetype) -> bool { true }
}

impl<T: Component> QueryFilter for With<T> {
    fn matches_archetype(arch: &Archetype) -> bool {
        arch.contains(TypeId::of::<T>())
    }
}

impl<T: Component> QueryFilter for Without<T> {
    fn matches_archetype(arch: &Archetype) -> bool {
        !arch.contains(TypeId::of::<T>())
    }
}

impl<A: QueryFilter, B: QueryFilter> QueryFilter for (A, B) {
    fn matches_archetype(arch: &Archetype) -> bool {
        A::matches_archetype(arch) && B::matches_archetype(arch)
    }
}

impl<A: QueryFilter, B: QueryFilter, C: QueryFilter> QueryFilter for (A, B, C) {
    fn matches_archetype(arch: &Archetype) -> bool {
        A::matches_archetype(arch) && B::matches_archetype(arch) && C::matches_archetype(arch)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WorldQuery — fetch marker types and trait
// ─────────────────────────────────────────────────────────────────────────────

/// Marker type: fetch component `T` immutably → yields `&T`.
///
/// Use as a type parameter in [`World::query`]:
/// ```rust,ignore
/// for pos in world.query::<Read<Position>, ()>() { ... }
/// ```
pub struct Read<T: Component>(std::marker::PhantomData<T>);

/// Marker type: fetch component `T` mutably → yields `&mut T`.
///
/// Use as a type parameter in [`World::query`]:
/// ```rust,ignore
/// for vel in world.query::<Write<Velocity>, ()>() { vel.dx *= 0.9; }
/// ```
pub struct Write<T: Component>(std::marker::PhantomData<T>);

/// Marker type: optionally fetch component `T` → yields `Option<&T>`.
///
/// Entities that do not have `T` yield `None` rather than being skipped.
pub struct OptionRead<T: Component>(std::marker::PhantomData<T>);

/// Describes how to fetch data from a single archetype row.
///
/// The `'static` bound on the trait (but not the `Item` GAT) is satisfied by
/// the marker types [`Read<T>`], [`Write<T>`], [`OptionRead<T>`], and
/// [`Entity`] — none of which carry lifetimes.
pub trait WorldQuery: 'static {
    /// The type yielded per entity (may borrow from the archetype).
    type Item<'w>;

    /// Returns the component `TypeId`s that *must* be present in an archetype.
    fn required_types() -> Vec<TypeId>;

    /// Returns `true` if `arch` provides all necessary columns.
    fn matches(arch: &Archetype) -> bool;

    /// Fetches the item for the entity at `row` in `arch`.
    ///
    /// # Safety
    /// `row` must be a valid row index. The caller must uphold aliasing rules
    /// (no other mutable references to the same row must exist for `Write<T>`).
    unsafe fn fetch<'w>(arch: &'w Archetype, row: usize) -> Self::Item<'w>;
}

// --- Read<T> → &'w T ----------------------------------------------------------
impl<T: Component> WorldQuery for Read<T> {
    type Item<'w> = &'w T;

    fn required_types() -> Vec<TypeId> { vec![TypeId::of::<T>()] }

    fn matches(arch: &Archetype) -> bool { arch.contains(TypeId::of::<T>()) }

    unsafe fn fetch<'w>(arch: &'w Archetype, row: usize) -> &'w T {
        arch.columns[&TypeId::of::<T>()].column.get::<T>(row)
    }
}

// --- Write<T> → &'w mut T ----------------------------------------------------
impl<T: Component> WorldQuery for Write<T> {
    type Item<'w> = &'w mut T;

    fn required_types() -> Vec<TypeId> { vec![TypeId::of::<T>()] }

    fn matches(arch: &Archetype) -> bool { arch.contains(TypeId::of::<T>()) }

    unsafe fn fetch<'w>(arch: &'w Archetype, row: usize) -> &'w mut T {
        // SAFETY: exclusive access guaranteed by caller.
        let col = arch.columns.get(&TypeId::of::<T>()).unwrap();
        let ptr = col.column.get_ptr(row) as *mut T;
        &mut *ptr
    }
}

// --- OptionRead<T> → Option<&'w T> -------------------------------------------
impl<T: Component> WorldQuery for OptionRead<T> {
    type Item<'w> = Option<&'w T>;

    fn required_types() -> Vec<TypeId> { vec![] }

    fn matches(_: &Archetype) -> bool { true }

    unsafe fn fetch<'w>(arch: &'w Archetype, row: usize) -> Option<&'w T> {
        arch.columns
            .get(&TypeId::of::<T>())
            .map(|col| col.column.get::<T>(row))
    }
}

// --- Entity ------------------------------------------------------------------
impl WorldQuery for Entity {
    type Item<'w> = Entity;

    fn required_types() -> Vec<TypeId> { vec![] }

    fn matches(_: &Archetype) -> bool { true }

    unsafe fn fetch<'w>(arch: &'w Archetype, row: usize) -> Entity {
        arch.entities[row]
    }
}

// --- Tuple impls -------------------------------------------------------------
macro_rules! impl_world_query_tuple {
    ($($Q:ident),*) => {
        #[allow(non_snake_case)]
        impl<$($Q: WorldQuery),*> WorldQuery for ($($Q,)*) {
            type Item<'w> = ($($Q::Item<'w>,)*);

            fn required_types() -> Vec<TypeId> {
                let mut ids = Vec::new();
                $(ids.extend($Q::required_types());)*
                ids.sort();
                ids.dedup();
                ids
            }

            fn matches(arch: &Archetype) -> bool {
                $($Q::matches(arch))&&*
            }

            unsafe fn fetch<'w>(arch: &'w Archetype, row: usize) -> ($($Q::Item<'w>,)*) {
                ($($Q::fetch(arch, row),)*)
            }
        }
    };
}

impl_world_query_tuple!(A);
impl_world_query_tuple!(A, B);
impl_world_query_tuple!(A, B, C);
impl_world_query_tuple!(A, B, C, D);
impl_world_query_tuple!(A, B, C, D, E);
impl_world_query_tuple!(A, B, C, D, E, F);
impl_world_query_tuple!(A, B, C, D, E, F, G);
impl_world_query_tuple!(A, B, C, D, E, F, G, H);

// ─────────────────────────────────────────────────────────────────────────────
// QueryIter
// ─────────────────────────────────────────────────────────────────────────────

/// An iterator over all entities in a world matching query `Q` and filter `F`.
///
/// Obtained via [`World::query`]. Walks through all archetypes in order,
/// skipping those that do not match, and yields one item per entity row.
pub struct QueryIter<'w, Q: WorldQuery, F: QueryFilter> {
    archetypes: &'w [Archetype],
    arch_index: usize,
    row: usize,
    _q: std::marker::PhantomData<Q>,
    _f: std::marker::PhantomData<F>,
}

impl<'w, Q: WorldQuery, F: QueryFilter> QueryIter<'w, Q, F> {
    fn new(archetypes: &'w [Archetype]) -> Self {
        Self {
            archetypes,
            arch_index: 0,
            row: 0,
            _q: std::marker::PhantomData,
            _f: std::marker::PhantomData,
        }
    }
}

impl<'w, Q: WorldQuery, F: QueryFilter> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let arch = self.archetypes.get(self.arch_index)?;
            if !Q::matches(arch) || !F::matches_archetype(arch) {
                self.arch_index += 1;
                self.row = 0;
                continue;
            }
            if self.row >= arch.len() {
                self.arch_index += 1;
                self.row = 0;
                continue;
            }
            let row = self.row;
            self.row += 1;
            // SAFETY: row < arch.len(); shared reference upholds read aliasing rules.
            return Some(unsafe { Q::fetch(arch, row) });
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EntityRef / EntityMut
// ─────────────────────────────────────────────────────────────────────────────

/// An immutable view into a single entity's components.
pub struct EntityRef<'w> {
    archetype: &'w Archetype,
    row: usize,
}

impl<'w> EntityRef<'w> {
    fn new(archetype: &'w Archetype, row: usize) -> Self {
        Self { archetype, row }
    }

    /// Returns the `Entity` handle for this view.
    pub fn entity(&self) -> Entity {
        self.archetype.entities[self.row]
    }

    /// Returns `&T` if the entity has component `T`.
    pub fn get<T: Component>(&self) -> Option<&T> {
        self.archetype
            .columns
            .get(&TypeId::of::<T>())
            .map(|col| col.column.get::<T>(self.row))
    }

    /// Returns `true` if the entity has component `T`.
    pub fn has<T: Component>(&self) -> bool {
        self.archetype.contains(TypeId::of::<T>())
    }

    /// Returns the full list of component type IDs for this entity.
    pub fn component_types(&self) -> &[TypeId] {
        &self.archetype.component_types
    }
}

/// A mutable view into a single entity's components.
pub struct EntityMut<'w> {
    archetype: &'w mut Archetype,
    row: usize,
}

impl<'w> EntityMut<'w> {
    fn new(archetype: &'w mut Archetype, row: usize) -> Self {
        Self { archetype, row }
    }

    /// Returns the `Entity` handle for this view.
    pub fn entity(&self) -> Entity {
        self.archetype.entities[self.row]
    }

    /// Returns `&T` if the entity has component `T`.
    pub fn get<T: Component>(&self) -> Option<&T> {
        self.archetype
            .columns
            .get(&TypeId::of::<T>())
            .map(|col| col.column.get::<T>(self.row))
    }

    /// Returns `&mut T` if the entity has component `T`.
    pub fn get_mut<T: Component>(&mut self) -> Option<&mut T> {
        self.archetype
            .columns
            .get_mut(&TypeId::of::<T>())
            .map(|col| col.column.get_mut::<T>(self.row))
    }

    /// Returns `true` if the entity has component `T`.
    pub fn has<T: Component>(&self) -> bool {
        self.archetype.contains(TypeId::of::<T>())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// World
// ─────────────────────────────────────────────────────────────────────────────

/// The central ECS container.
///
/// [`World`] owns:
/// - All [`Archetype`]s and their component data columns.
/// - The [`EntityAllocator`] (generational index → location mapping).
/// - The [`Resources`] type-map.
/// - The global change-detection tick counter.
/// - Registered [`Events<E>`] queues.
///
/// # Spawning entities
/// ```rust
/// use proof_engine::ecs::{World, Component};
///
/// #[derive(Clone)] struct Pos(f32, f32);
/// impl Component for Pos {}
///
/// let mut world = World::new();
/// let entity = world.spawn(Pos(1.0, 2.0));
/// assert!(world.is_alive(entity));
/// ```
pub struct World {
    /// All archetypes, indexed by position.
    archetypes: Vec<Archetype>,
    /// Maps sorted component-type-set → archetype index.
    archetype_index: HashMap<Vec<TypeId>, usize>,
    /// Entity allocator and location tracking.
    entities: EntityAllocator,
    /// Global singleton resources.
    pub resources: Resources,
    /// Registered event queues, keyed by `TypeId::of::<Events<E>>()`.
    events: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    /// Monotonically-increasing tick used for change detection.
    tick: u32,
}

impl World {
    /// Creates a new, empty world.
    pub fn new() -> Self {
        Self {
            archetypes: Vec::new(),
            archetype_index: HashMap::new(),
            entities: EntityAllocator::default(),
            resources: Resources::new(),
            events: HashMap::new(),
            tick: 0,
        }
    }

    /// Returns the current change-detection tick.
    pub fn tick(&self) -> u32 {
        self.tick
    }

    /// Advances the tick counter by 1 (call once per frame / schedule run).
    pub fn increment_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    // ── Archetype management ────────────────────────────────────────────

    /// Returns the index of the archetype for `types` (sorted/deduped),
    /// creating it if it does not yet exist.
    fn get_or_create_archetype(&mut self, mut types: Vec<TypeId>) -> usize {
        types.sort();
        types.dedup();
        if let Some(&idx) = self.archetype_index.get(&types) {
            return idx;
        }
        let id = ArchetypeId(self.archetypes.len() as u32);
        let arch = Archetype::new(id, types.clone());
        let idx = self.archetypes.len();
        self.archetypes.push(arch);
        self.archetype_index.insert(types, idx);
        idx
    }

    // ── Spawn / despawn ─────────────────────────────────────────────────

    /// Spawns a new entity with the given [`Bundle`] of components.
    ///
    /// Returns the new [`Entity`] handle.
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> Entity {
        let types = B::type_ids();
        let arch_idx = self.get_or_create_archetype(types);

        let entity = self.entities.alloc();
        let tick = self.tick;
        let arch = &mut self.archetypes[arch_idx];

        B::register_columns(arch);
        let row = arch.entities.len();
        arch.entities.push(entity);
        bundle.insert_into(arch, tick);

        self.entities.set_location(
            entity,
            EntityLocation { archetype_id: ArchetypeId(arch_idx as u32), row },
        );
        entity
    }

    /// Despawns the entity, removing all its components.
    ///
    /// Returns `true` if the entity was alive and has been removed.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        let loc = match self.entities.location(entity) {
            Some(l) => l,
            None => return false,
        };
        let arch_idx = loc.archetype_id.0 as usize;
        let swapped = self.archetypes[arch_idx].swap_remove(loc.row);
        self.entities.free(entity);
        if let Some(swapped_entity) = swapped {
            self.entities.set_location(
                swapped_entity,
                EntityLocation { archetype_id: loc.archetype_id, row: loc.row },
            );
        }
        true
    }

    /// Returns `true` if `entity` is currently alive in the world.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    // ── Component insert / remove ───────────────────────────────────────

    /// Inserts component `C` onto `entity`.
    ///
    /// If the entity already has `C`, the value is replaced in-place.
    /// If not, the entity is migrated to a new archetype that includes `C`.
    pub fn insert<C: Component>(&mut self, entity: Entity, component: C) {
        let loc = match self.entities.location(entity) {
            Some(l) => l,
            None => return,
        };
        let old_arch_idx = loc.archetype_id.0 as usize;

        // Fast path: component already present — update in-place.
        if self.archetypes[old_arch_idx].contains(TypeId::of::<C>()) {
            let tick = self.tick;
            let arch = &mut self.archetypes[old_arch_idx];
            let col = arch.columns.get_mut(&TypeId::of::<C>()).unwrap();
            *col.column.get_mut::<C>(loc.row) = component;
            col.changed_ticks[loc.row] = tick;
            return;
        }

        // Slow path: migrate entity to a larger archetype.
        let mut new_types = self.archetypes[old_arch_idx].component_types.clone();
        new_types.push(TypeId::of::<C>());
        let new_arch_idx = self.get_or_create_archetype(new_types);
        let tick = self.tick;
        self.migrate_entity_add_component::<C>(entity, loc, new_arch_idx, component, tick);
    }

    /// Migrates `entity` from its current archetype to `new_arch_idx`,
    /// also adding component `C` with value `extra`.
    fn migrate_entity_add_component<C: Component>(
        &mut self,
        entity: Entity,
        loc: EntityLocation,
        new_arch_idx: usize,
        extra: C,
        tick: u32,
    ) {
        let old_arch_idx = loc.archetype_id.0 as usize;
        let row = loc.row;

        // Ensure destination has columns for all old types.
        let old_types: Vec<TypeId> =
            self.archetypes[old_arch_idx].component_types.clone();
        self.ensure_columns_from_old(old_arch_idx, new_arch_idx, &old_types);

        // Ensure destination has column for new type C.
        if !self.archetypes[new_arch_idx].columns.contains_key(&TypeId::of::<C>()) {
            self.archetypes[new_arch_idx].register_column::<C>();
        }

        // Copy all old component rows into new arch.
        let new_row = self.archetypes[new_arch_idx].entities.len();
        for &tid in &old_types {
            Self::copy_component_row(
                &mut self.archetypes,
                old_arch_idx,
                new_arch_idx,
                tid,
                row,
                tick,
            );
        }

        // Push the new component.
        self.archetypes[new_arch_idx]
            .columns
            .get_mut(&TypeId::of::<C>())
            .unwrap()
            .push(extra, tick);

        // Register entity in new arch, remove from old.
        self.archetypes[new_arch_idx].entities.push(entity);
        let swapped = self.archetypes[old_arch_idx].swap_remove(row);

        // Update locations.
        self.entities.set_location(
            entity,
            EntityLocation { archetype_id: ArchetypeId(new_arch_idx as u32), row: new_row },
        );
        if let Some(swapped_entity) = swapped {
            self.entities.set_location(
                swapped_entity,
                EntityLocation { archetype_id: loc.archetype_id, row },
            );
        }
    }

    /// Ensures that `new_arch` has columns for every TypeId in `old_types`,
    /// copying column metadata (element_size, drop_fn, clone_fn) from `old_arch`.
    fn ensure_columns_from_old(
        &mut self,
        old_arch_idx: usize,
        new_arch_idx: usize,
        old_types: &[TypeId],
    ) {
        for &tid in old_types {
            if self.archetypes[new_arch_idx].columns.contains_key(&tid) {
                continue;
            }
            // Copy column factory metadata from old arch.
            let src = &self.archetypes[old_arch_idx].columns[&tid];
            let new_col = ComponentStorage {
                column: AnyVec {
                    type_id: src.column.type_id,
                    len: 0,
                    data: Vec::new(),
                    element_size: src.column.element_size,
                    drop_fn: src.column.drop_fn,
                    clone_fn: src.column.clone_fn,
                },
                added_ticks: Vec::new(),
                changed_ticks: Vec::new(),
            };
            self.archetypes[new_arch_idx].columns.insert(tid, new_col);
        }
    }

    /// Clones the component at `row` in `archetypes[src_idx]` into `archetypes[dst_idx]`.
    fn copy_component_row(
        archetypes: &mut Vec<Archetype>,
        src_idx: usize,
        dst_idx: usize,
        tid: TypeId,
        row: usize,
        tick: u32,
    ) {
        // Split borrow so we can access two archetypes mutably.
        let (src_arch, dst_arch) = if src_idx < dst_idx {
            let (left, right) = archetypes.split_at_mut(dst_idx);
            (&left[src_idx], &mut right[0])
        } else {
            let (left, right) = archetypes.split_at_mut(src_idx);
            (&right[0], &mut left[dst_idx])
        };

        if let (Some(src_col), Some(dst_col)) = (
            src_arch.columns.get(&tid),
            dst_arch.columns.get_mut(&tid),
        ) {
            src_col.clone_row_into(row, dst_col, tick);
        }
    }

    /// Removes component `C` from `entity`, migrating it to a smaller archetype.
    ///
    /// Does nothing if the entity does not have `C`.
    pub fn remove<C: Component>(&mut self, entity: Entity) {
        self.remove_by_type_id(entity, TypeId::of::<C>());
    }

    /// Removes a component identified by raw `TypeId` from `entity`.
    pub(crate) fn remove_by_type_id(&mut self, entity: Entity, type_id: TypeId) {
        let loc = match self.entities.location(entity) {
            Some(l) => l,
            None => return,
        };
        let old_arch_idx = loc.archetype_id.0 as usize;
        if !self.archetypes[old_arch_idx].contains(type_id) {
            return;
        }

        let row = loc.row;
        let new_types: Vec<TypeId> = self.archetypes[old_arch_idx]
            .component_types
            .iter()
            .copied()
            .filter(|&t| t != type_id)
            .collect();
        let new_arch_idx = self.get_or_create_archetype(new_types.clone());
        let tick = self.tick;

        // Ensure columns exist in destination.
        let old_types: Vec<TypeId> =
            self.archetypes[old_arch_idx].component_types.clone();
        self.ensure_columns_from_old(old_arch_idx, new_arch_idx, &old_types);

        let new_row = self.archetypes[new_arch_idx].entities.len();

        // Copy all rows except the one being removed.
        for &tid in &old_types {
            if tid == type_id { continue; }
            Self::copy_component_row(
                &mut self.archetypes,
                old_arch_idx,
                new_arch_idx,
                tid,
                row,
                tick,
            );
        }

        self.archetypes[new_arch_idx].entities.push(entity);
        let swapped = self.archetypes[old_arch_idx].swap_remove(row);

        self.entities.set_location(
            entity,
            EntityLocation { archetype_id: ArchetypeId(new_arch_idx as u32), row: new_row },
        );
        if let Some(swapped_entity) = swapped {
            self.entities.set_location(
                swapped_entity,
                EntityLocation { archetype_id: loc.archetype_id, row },
            );
        }
    }

    // ── Component access ────────────────────────────────────────────────

    /// Returns `&C` for the given entity's component, or `None`.
    pub fn get<C: Component>(&self, entity: Entity) -> Option<&C> {
        let loc = self.entities.location(entity)?;
        self.archetypes[loc.archetype_id.0 as usize]
            .columns
            .get(&TypeId::of::<C>())
            .map(|col| col.column.get::<C>(loc.row))
    }

    /// Returns `&mut C` for the given entity's component, or `None`.
    ///
    /// Also updates the `changed_tick` for that row.
    pub fn get_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        let loc = self.entities.location(entity)?;
        let tick = self.tick;
        let arch = &mut self.archetypes[loc.archetype_id.0 as usize];
        arch.columns.get_mut(&TypeId::of::<C>()).map(|col| {
            col.changed_ticks[loc.row] = tick;
            col.column.get_mut::<C>(loc.row)
        })
    }

    /// Returns an [`EntityRef`] for read-only access to an entity's components.
    pub fn entity_ref(&self, entity: Entity) -> Option<EntityRef<'_>> {
        let loc = self.entities.location(entity)?;
        Some(EntityRef::new(
            &self.archetypes[loc.archetype_id.0 as usize],
            loc.row,
        ))
    }

    /// Returns an [`EntityMut`] for read-write access to an entity's components.
    pub fn entity_mut(&mut self, entity: Entity) -> Option<EntityMut<'_>> {
        let loc = self.entities.location(entity)?;
        let arch_idx = loc.archetype_id.0 as usize;
        Some(EntityMut::new(&mut self.archetypes[arch_idx], loc.row))
    }

    // ── Queries ─────────────────────────────────────────────────────────

    /// Returns an iterator over all entities matching query `Q` and filter `F`.
    ///
    /// # Type parameters
    /// - `Q`: a [`WorldQuery`] (e.g. `Read<Position>`, `(Read<Pos>, Write<Vel>)`).
    /// - `F`: a [`QueryFilter`] (e.g. `With<Player>`, `Without<Dead>`, `()`).
    ///
    /// # Example
    /// ```rust,ignore
    /// for (entity, pos) in world.query::<(Entity, Read<Position>), ()>() {
    ///     println!("{entity}: ({}, {})", pos.x, pos.y);
    /// }
    /// ```
    pub fn query<Q: WorldQuery, F: QueryFilter>(&self) -> QueryIter<'_, Q, F> {
        QueryIter::new(&self.archetypes)
    }

    /// Returns an iterator over all entities matching `Q` with no filter.
    pub fn query_all<Q: WorldQuery>(&self) -> QueryIter<'_, Q, ()> {
        QueryIter::new(&self.archetypes)
    }

    /// Returns the single entity matching `Q`, or `None` if zero or more than one match.
    pub fn query_single<Q: WorldQuery>(&self) -> Option<Q::Item<'_>> {
        let mut iter = self.query::<Q, ()>();
        let first = iter.next()?;
        if iter.next().is_some() { return None; }
        Some(first)
    }

    // ── Resources ───────────────────────────────────────────────────────

    /// Inserts a resource into the world.
    pub fn insert_resource<T: 'static + Send + Sync>(&mut self, value: T) {
        self.resources.insert(value);
    }

    /// Returns an immutable [`Res`] wrapper for resource `T`.
    pub fn resource<T: 'static + Send + Sync>(&self) -> Option<Res<'_, T>> {
        self.resources.get::<T>().map(Res::new)
    }

    /// Returns a mutable [`ResMut`] wrapper for resource `T`.
    pub fn resource_mut<T: 'static + Send + Sync>(&mut self) -> Option<ResMut<'_, T>> {
        self.resources.get_mut::<T>().map(ResMut::new)
    }

    /// Removes and returns the resource of type `T`.
    pub fn remove_resource<T: 'static + Send + Sync>(&mut self) -> Option<T> {
        self.resources.remove::<T>()
    }

    // ── Events ──────────────────────────────────────────────────────────

    /// Registers event type `E`, creating its [`Events<E>`] storage.
    pub fn add_event<E: 'static + Send + Sync>(&mut self) {
        self.events
            .entry(TypeId::of::<Events<E>>())
            .or_insert_with(|| Box::new(Events::<E>::new()));
    }

    /// Returns a reference to the [`Events<E>`] queue.
    pub fn events<E: 'static + Send + Sync>(&self) -> Option<&Events<E>> {
        self.events
            .get(&TypeId::of::<Events<E>>())
            .and_then(|b| b.downcast_ref::<Events<E>>())
    }

    /// Returns a mutable reference to the [`Events<E>`] queue.
    pub fn events_mut<E: 'static + Send + Sync>(&mut self) -> Option<&mut Events<E>> {
        self.events
            .get_mut(&TypeId::of::<Events<E>>())
            .and_then(|b| b.downcast_mut::<Events<E>>())
    }

    /// Sends a single event, auto-creating the queue if not yet registered.
    pub fn send_event<E: 'static + Send + Sync>(&mut self, event: E) {
        self.events
            .entry(TypeId::of::<Events<E>>())
            .or_insert_with(|| Box::new(Events::<E>::new()))
            .downcast_mut::<Events<E>>()
            .unwrap()
            .send(event);
    }

    // ── Entity iteration ────────────────────────────────────────────────

    /// Returns an iterator over all live entities.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.archetypes.iter().flat_map(|a| a.entities.iter().copied())
    }

    /// Returns the total number of live entities.
    pub fn entity_count(&self) -> usize {
        self.archetypes.iter().map(|a| a.len()).sum()
    }

    /// Returns the number of archetypes currently in the world.
    pub fn archetype_count(&self) -> usize {
        self.archetypes.len()
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SystemParam trait
// ─────────────────────────────────────────────────────────────────────────────

/// A type that can be extracted from a [`World`] as a parameter to a system.
///
/// Implementations are provided for [`Res<T>`], [`ResMut<T>`], [`Commands`],
/// and [`Local<T>`]. Custom system parameter types can implement this trait
/// to participate in the parameter-injection protocol.
pub trait SystemParam: Sized {
    /// Per-system persistent state (e.g. a cursor for an event reader).
    type State: Default + Send + Sync + 'static;

    /// Called once to initialise the state when the system is registered.
    fn init_state(world: &mut World) -> Self::State;
}

// ─────────────────────────────────────────────────────────────────────────────
// System trait
// ─────────────────────────────────────────────────────────────────────────────

/// A unit of logic that reads and/or writes the [`World`].
///
/// The simplest way to create a system is via [`into_system`], wrapping a
/// plain closure. More complex systems can implement this trait directly.
pub trait System: Send + Sync + 'static {
    /// Executes the system against `world`.
    fn run(&mut self, world: &mut World);

    /// Returns the human-readable name of this system.
    fn name(&self) -> &str;
}

/// A [`System`] wrapping a plain closure `FnMut(&mut World)`.
pub struct FunctionSystem<F>
where
    F: FnMut(&mut World) + Send + Sync + 'static,
{
    func: F,
    name: String,
}

impl<F: FnMut(&mut World) + Send + Sync + 'static> FunctionSystem<F> {
    /// Creates a named function system.
    pub fn new(name: impl Into<String>, func: F) -> Self {
        Self { func, name: name.into() }
    }
}

impl<F: FnMut(&mut World) + Send + Sync + 'static> System for FunctionSystem<F> {
    fn run(&mut self, world: &mut World) { (self.func)(world); }
    fn name(&self) -> &str { &self.name }
}

/// Wraps a named closure as a boxed [`System`].
///
/// # Example
/// ```rust
/// use proof_engine::ecs::into_system;
/// let sys = into_system("noop", |_w| {});
/// ```
pub fn into_system(
    name: impl Into<String>,
    f: impl FnMut(&mut World) + Send + Sync + 'static,
) -> Box<dyn System> {
    Box::new(FunctionSystem::new(name, f))
}

// ─────────────────────────────────────────────────────────────────────────────
// Schedule
// ─────────────────────────────────────────────────────────────────────────────

/// A human-readable identifier for a system within a [`Schedule`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemLabel(pub String);

impl SystemLabel {
    /// Creates a new `SystemLabel` from any string-like value.
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
}

impl<S: Into<String>> From<S> for SystemLabel {
    fn from(s: S) -> Self { Self(s.into()) }
}

/// A boxed run condition closure.
pub type RunCondition = Box<dyn Fn(&World) -> bool + Send + Sync>;

/// A single system slot in a [`Schedule`].
struct SystemEntry {
    system: Box<dyn System>,
    label: Option<SystemLabel>,
    run_if: Option<RunCondition>,
}

/// An ordered list of systems with optional labels and run conditions.
///
/// Each call to [`Schedule::run`] increments the world tick and then
/// executes every enabled system in insertion order.
///
/// # Example
/// ```rust
/// use proof_engine::ecs::{Schedule, World, into_system};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// schedule.add_system(into_system("tick", |_w| {}));
/// schedule.run(&mut world);
/// ```
pub struct Schedule {
    systems: Vec<SystemEntry>,
}

impl Schedule {
    /// Creates an empty schedule.
    pub fn new() -> Self { Self { systems: Vec::new() } }

    /// Appends a system (no label, always runs).
    pub fn add_system(&mut self, system: Box<dyn System>) -> &mut Self {
        self.systems.push(SystemEntry { system, label: None, run_if: None });
        self
    }

    /// Appends a labelled system.
    pub fn add_system_with_label(
        &mut self,
        system: Box<dyn System>,
        label: impl Into<SystemLabel>,
    ) -> &mut Self {
        self.systems.push(SystemEntry {
            system, label: Some(label.into()), run_if: None,
        });
        self
    }

    /// Appends a system with a run condition.
    pub fn add_system_with_condition(
        &mut self,
        system: Box<dyn System>,
        condition: impl Fn(&World) -> bool + Send + Sync + 'static,
    ) -> &mut Self {
        self.systems.push(SystemEntry {
            system, label: None, run_if: Some(Box::new(condition)),
        });
        self
    }

    /// Appends a labelled system with a run condition.
    pub fn add_system_full(
        &mut self,
        system: Box<dyn System>,
        label: impl Into<SystemLabel>,
        condition: impl Fn(&World) -> bool + Send + Sync + 'static,
    ) -> &mut Self {
        self.systems.push(SystemEntry {
            system,
            label: Some(label.into()),
            run_if: Some(Box::new(condition)),
        });
        self
    }

    /// Removes all systems with the given label.
    pub fn remove_system(&mut self, label: &SystemLabel) {
        self.systems.retain(|e| e.label.as_ref() != Some(label));
    }

    /// Returns the number of system slots in this schedule.
    pub fn system_count(&self) -> usize { self.systems.len() }

    /// Runs all systems (incrementing the world tick first).
    pub fn run(&mut self, world: &mut World) {
        world.increment_tick();
        for entry in &mut self.systems {
            let should_run = entry.run_if.as_ref()
                .map(|cond| cond(world))
                .unwrap_or(true);
            if should_run {
                entry.system.run(world);
            }
        }
    }

    /// Returns the labels of all systems in order (`None` for unlabelled).
    pub fn labels(&self) -> Vec<Option<&SystemLabel>> {
        self.systems.iter().map(|e| e.label.as_ref()).collect()
    }
}

impl Default for Schedule {
    fn default() -> Self { Self::new() }
}

// ─────────────────────────────────────────────────────────────────────────────
// Change detection helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if component `C` on `entity` was added after tick `since`.
///
/// Returns `false` if the entity is dead or does not have `C`.
pub fn was_added<C: Component>(world: &World, entity: Entity, since: u32) -> bool {
    let loc = match world.entities.location(entity) {
        Some(l) => l,
        None => return false,
    };
    world.archetypes[loc.archetype_id.0 as usize]
        .columns
        .get(&TypeId::of::<C>())
        .map(|col| col.added_ticks[loc.row] > since)
        .unwrap_or(false)
}

/// Returns `true` if component `C` on `entity` was mutably accessed after tick `since`.
pub fn was_changed<C: Component>(world: &World, entity: Entity, since: u32) -> bool {
    let loc = match world.entities.location(entity) {
        Some(l) => l,
        None => return false,
    };
    world.archetypes[loc.archetype_id.0 as usize]
        .columns
        .get(&TypeId::of::<C>())
        .map(|col| col.changed_ticks[loc.row] > since)
        .unwrap_or(false)
}

/// Returns an iterator over entities that have `C` and where `C` was added
/// strictly after tick `since`.
pub fn query_added<C: Component>(world: &World, since: u32) -> impl Iterator<Item = Entity> + '_ {
    world.archetypes.iter().flat_map(move |arch| {
        if !arch.contains(TypeId::of::<C>()) { return vec![]; }
        let col = &arch.columns[&TypeId::of::<C>()];
        arch.entities
            .iter()
            .enumerate()
            .filter(move |(row, _)| col.added_ticks[*row] > since)
            .map(|(_, &e)| e)
            .collect::<Vec<_>>()
    })
}

/// Returns an iterator over entities that have `C` and where `C` was changed
/// strictly after tick `since`.
pub fn query_changed<C: Component>(world: &World, since: u32) -> impl Iterator<Item = Entity> + '_ {
    world.archetypes.iter().flat_map(move |arch| {
        if !arch.contains(TypeId::of::<C>()) { return vec![]; }
        let col = &arch.columns[&TypeId::of::<C>()];
        arch.entities
            .iter()
            .enumerate()
            .filter(move |(row, _)| col.changed_ticks[*row] > since)
            .map(|(_, &e)| e)
            .collect::<Vec<_>>()
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Prelude
// ─────────────────────────────────────────────────────────────────────────────

/// Common re-exports for working with the ECS.
pub mod prelude {
    pub use super::{
        Component, Entity, World, Bundle,
        Archetype, ArchetypeId,
        EntityRef, EntityMut,
        Resources, Res, ResMut,
        Local,
        Events, EventCursor, EventWriter, EventReader,
        Commands,
        With, Without, Added, Changed,
        Read, Write, OptionRead,
        WorldQuery, QueryFilter, QueryIter,
        System, SystemParam, FunctionSystem, into_system,
        Schedule, SystemLabel,
        was_added, was_changed, query_added, query_changed,
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Component definitions ─────────────────────────────────────────────

    #[derive(Debug, Clone, PartialEq)]
    struct Position { x: f32, y: f32 }
    impl Component for Position {}

    #[derive(Debug, Clone, PartialEq)]
    struct Velocity { dx: f32, dy: f32 }
    impl Component for Velocity {}

    #[derive(Debug, Clone, PartialEq)]
    struct Health(f32);
    impl Component for Health {}

    #[derive(Debug, Clone, PartialEq)]
    struct Name(String);
    impl Component for Name {}

    #[derive(Debug, Clone)]
    struct Tag;
    impl Component for Tag {}

    #[derive(Debug, Clone)]
    struct Enemy;
    impl Component for Enemy {}

    #[derive(Debug, Clone)]
    struct Player;
    impl Component for Player {}

    #[derive(Debug, Clone)]
    struct Dead;
    impl Component for Dead {}

    // ── Entity lifecycle ──────────────────────────────────────────────────

    #[test]
    fn test_spawn_and_alive() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 1.0, y: 2.0 });
        assert!(world.is_alive(e));
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn test_despawn() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 0.0, y: 0.0 });
        assert!(world.despawn(e));
        assert!(!world.is_alive(e));
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn test_despawn_nonexistent_returns_false() {
        let mut world = World::new();
        assert!(!world.despawn(Entity::from_raw(99, 99)));
    }

    #[test]
    fn test_entity_null_sentinel() {
        let null = Entity::null();
        assert!(null.is_null());
        assert!(!Entity::from_raw(0, 0).is_null());
    }

    #[test]
    fn test_generational_slot_reuse() {
        let mut world = World::new();
        let e1 = world.spawn(Position { x: 0.0, y: 0.0 });
        world.despawn(e1);
        let e2 = world.spawn(Position { x: 1.0, y: 0.0 });
        assert_eq!(e1.index, e2.index);
        assert_ne!(e1.generation, e2.generation);
        assert!(!world.is_alive(e1));
        assert!(world.is_alive(e2));
    }

    #[test]
    fn test_entity_display() {
        let e = Entity::from_raw(5, 2);
        assert_eq!(format!("{e}"), "Entity(5v2)");
    }

    // ── Component access ──────────────────────────────────────────────────

    #[test]
    fn test_get_component() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 3.0, y: 4.0 });
        let pos = world.get::<Position>(e).unwrap();
        assert_eq!(pos.x, 3.0);
        assert_eq!(pos.y, 4.0);
    }

    #[test]
    fn test_get_mut_component() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 0.0, y: 0.0 });
        world.get_mut::<Position>(e).unwrap().x = 10.0;
        assert_eq!(world.get::<Position>(e).unwrap().x, 10.0);
    }

    #[test]
    fn test_missing_component_returns_none() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 0.0, y: 0.0 });
        assert!(world.get::<Velocity>(e).is_none());
    }

    #[test]
    fn test_entity_ref() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 1.0, y: 2.0 }, Health(100.0)));
        let eref = world.entity_ref(e).unwrap();
        assert_eq!(eref.get::<Position>().unwrap(), &Position { x: 1.0, y: 2.0 });
        assert_eq!(eref.get::<Health>().unwrap(), &Health(100.0));
        assert!(eref.has::<Position>());
        assert!(!eref.has::<Velocity>());
    }

    #[test]
    fn test_entity_mut() {
        let mut world = World::new();
        let e = world.spawn(Health(50.0));
        world.entity_mut(e).unwrap().get_mut::<Health>().unwrap().0 = 75.0;
        assert_eq!(world.get::<Health>(e).unwrap().0, 75.0);
    }

    // ── Bundle spawning ───────────────────────────────────────────────────

    #[test]
    fn test_spawn_tuple_bundle() {
        let mut world = World::new();
        let e = world.spawn((
            Position { x: 1.0, y: 0.0 },
            Velocity { dx: 0.5, dy: 0.0 },
            Health(100.0),
        ));
        assert!(world.get::<Position>(e).is_some());
        assert!(world.get::<Velocity>(e).is_some());
        assert!(world.get::<Health>(e).is_some());
    }

    #[test]
    fn test_spawn_multiple_same_archetype() {
        let mut world = World::new();
        let e1 = world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { dx: 1.0, dy: 0.0 }));
        let e2 = world.spawn((Position { x: 5.0, y: 5.0 }, Velocity { dx: -1.0, dy: 0.0 }));
        assert_eq!(world.archetype_count(), 1);
        assert_eq!(world.get::<Position>(e1).unwrap().x, 0.0);
        assert_eq!(world.get::<Position>(e2).unwrap().x, 5.0);
    }

    // ── Component insert / remove ─────────────────────────────────────────

    #[test]
    fn test_insert_new_component_migrates_archetype() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 0.0, y: 0.0 });
        world.insert(e, Velocity { dx: 1.0, dy: 0.0 });
        assert!(world.get::<Velocity>(e).is_some());
        assert!(world.get::<Position>(e).is_some());
    }

    #[test]
    fn test_insert_replaces_existing_component() {
        let mut world = World::new();
        let e = world.spawn(Health(100.0));
        world.insert(e, Health(50.0));
        assert_eq!(world.get::<Health>(e).unwrap().0, 50.0);
    }

    #[test]
    fn test_remove_component_migrates_archetype() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { dx: 1.0, dy: 0.0 }));
        world.remove::<Velocity>(e);
        assert!(world.get::<Velocity>(e).is_none());
        assert!(world.get::<Position>(e).is_some());
        assert!(world.is_alive(e));
    }

    #[test]
    fn test_remove_nonexistent_component_is_noop() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 0.0, y: 0.0 });
        world.remove::<Velocity>(e); // should not panic
        assert!(world.is_alive(e));
    }

    #[test]
    fn test_insert_then_remove_roundtrip() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 1.0, y: 2.0 });
        world.insert(e, Health(99.0));
        assert_eq!(world.get::<Health>(e).unwrap().0, 99.0);
        world.remove::<Health>(e);
        assert!(world.get::<Health>(e).is_none());
        assert_eq!(world.get::<Position>(e).unwrap().x, 1.0);
    }

    // ── Queries ───────────────────────────────────────────────────────────

    #[test]
    fn test_query_single_component() {
        let mut world = World::new();
        world.spawn(Position { x: 1.0, y: 0.0 });
        world.spawn(Position { x: 2.0, y: 0.0 });
        world.spawn(Position { x: 3.0, y: 0.0 });
        let xs: Vec<f32> = world.query::<Read<Position>, ()>().map(|p| p.x).collect();
        assert_eq!(xs.len(), 3);
    }

    #[test]
    fn test_query_tuple() {
        let mut world = World::new();
        world.spawn((Position { x: 0.0, y: 0.0 }, Velocity { dx: 1.0, dy: 0.0 }));
        world.spawn(Position { x: 5.0, y: 0.0 }); // no velocity
        let count = world.query::<(Read<Position>, Read<Velocity>), ()>().count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_query_with_filter() {
        let mut world = World::new();
        world.spawn((Position { x: 0.0, y: 0.0 }, Tag));
        world.spawn(Position { x: 1.0, y: 0.0 });
        let count = world.query::<Read<Position>, With<Tag>>().count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_query_without_filter() {
        let mut world = World::new();
        world.spawn((Position { x: 0.0, y: 0.0 }, Enemy));
        world.spawn((Position { x: 1.0, y: 0.0 }, Player));
        world.spawn(Position { x: 2.0, y: 0.0 });
        let count = world.query::<Read<Position>, Without<Enemy>>().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_query_option_read() {
        let mut world = World::new();
        world.spawn((Position { x: 0.0, y: 0.0 }, Health(100.0)));
        world.spawn(Position { x: 1.0, y: 0.0 });
        let results: Vec<_> = world
            .query::<(Read<Position>, OptionRead<Health>), ()>()
            .collect();
        assert_eq!(results.len(), 2);
        assert_eq!(results.iter().filter(|(_, h)| h.is_some()).count(), 1);
    }

    #[test]
    fn test_query_entity() {
        let mut world = World::new();
        let e1 = world.spawn(Position { x: 0.0, y: 0.0 });
        let e2 = world.spawn(Position { x: 1.0, y: 0.0 });
        let entities: Vec<Entity> = world.query::<Entity, ()>().collect();
        assert!(entities.contains(&e1));
        assert!(entities.contains(&e2));
    }

    #[test]
    fn test_query_mutable() {
        let mut world = World::new();
        world.spawn(Position { x: 0.0, y: 0.0 });
        world.spawn(Position { x: 1.0, y: 0.0 });
        for pos in world.query::<Write<Position>, ()>() {
            pos.x += 10.0;
        }
        let xs: Vec<f32> = world.query::<Read<Position>, ()>().map(|p| p.x).collect();
        assert!(xs.iter().all(|&x| x >= 10.0));
    }

    #[test]
    fn test_query_single() {
        let mut world = World::new();
        world.spawn(Player);
        assert!(world.query_single::<Read<Player>>().is_some());
    }

    #[test]
    fn test_query_single_none_when_multiple() {
        let mut world = World::new();
        world.spawn(Player);
        world.spawn(Player);
        assert!(world.query_single::<Read<Player>>().is_none());
    }

    // ── Resources ─────────────────────────────────────────────────────────

    #[test]
    fn test_resource_insert_get() {
        let mut world = World::new();
        world.insert_resource(42u32);
        assert_eq!(*world.resource::<u32>().unwrap(), 42);
    }

    #[test]
    fn test_resource_get_mut() {
        let mut world = World::new();
        world.insert_resource(0u32);
        *world.resource_mut::<u32>().unwrap() = 99;
        assert_eq!(*world.resource::<u32>().unwrap(), 99);
    }

    #[test]
    fn test_resource_remove() {
        let mut world = World::new();
        world.insert_resource(42u32);
        assert_eq!(world.remove_resource::<u32>(), Some(42));
        assert!(world.resource::<u32>().is_none());
    }

    #[test]
    fn test_resource_missing_returns_none() {
        let world = World::new();
        assert!(world.resource::<u32>().is_none());
    }

    #[test]
    fn test_resources_standalone() {
        let mut res = Resources::new();
        res.insert(42u32);
        assert!(res.contains::<u32>());
        assert!(!res.contains::<i32>());
        assert_eq!(*res.get::<u32>().unwrap(), 42);
        res.remove::<u32>();
        assert!(!res.contains::<u32>());
    }

    // ── Commands ──────────────────────────────────────────────────────────

    #[test]
    fn test_commands_spawn() {
        let mut world = World::new();
        let mut cmds = Commands::new();
        cmds.spawn(Position { x: 7.0, y: 8.0 });
        cmds.apply(&mut world);
        assert_eq!(world.entity_count(), 1);
        let pos = world.query_single::<Read<Position>>().unwrap();
        assert_eq!(pos.x, 7.0);
    }

    #[test]
    fn test_commands_despawn() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 0.0, y: 0.0 });
        let mut cmds = Commands::new();
        cmds.despawn(e);
        cmds.apply(&mut world);
        assert!(!world.is_alive(e));
    }

    #[test]
    fn test_commands_insert() {
        let mut world = World::new();
        let e = world.spawn(Position { x: 0.0, y: 0.0 });
        let mut cmds = Commands::new();
        cmds.insert(e, Health(50.0));
        cmds.apply(&mut world);
        assert_eq!(world.get::<Health>(e).unwrap().0, 50.0);
    }

    #[test]
    fn test_commands_remove() {
        let mut world = World::new();
        let e = world.spawn((Position { x: 0.0, y: 0.0 }, Health(100.0)));
        let mut cmds = Commands::new();
        cmds.remove::<Health>(e);
        cmds.apply(&mut world);
        assert!(world.get::<Health>(e).is_none());
    }

    #[test]
    fn test_commands_insert_resource() {
        let mut world = World::new();
        let mut cmds = Commands::new();
        cmds.insert_resource(100i32);
        cmds.apply(&mut world);
        assert_eq!(*world.resource::<i32>().unwrap(), 100);
    }

    // ── Events ────────────────────────────────────────────────────────────

    #[derive(Debug, Clone, PartialEq)]
    struct DamageEvent { amount: f32 }

    #[test]
    fn test_events_send_and_read() {
        let mut events: Events<DamageEvent> = Events::new();
        let mut cursor = events.get_reader_current();
        events.send(DamageEvent { amount: 10.0 });
        events.send(DamageEvent { amount: 20.0 });
        let read: Vec<_> = events.read(&mut cursor).collect();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].amount, 10.0);
        assert_eq!(read[1].amount, 20.0);
    }

    #[test]
    fn test_events_update_clears_stale() {
        let mut events: Events<DamageEvent> = Events::new();
        events.send(DamageEvent { amount: 5.0 });
        events.update();
        events.update();
        assert!(events.is_empty());
    }

    #[test]
    fn test_event_writer_and_reader() {
        let mut events: Events<DamageEvent> = Events::new();
        EventWriter::new(&mut events).send(DamageEvent { amount: 42.0 });
        let mut reader = EventReader::new_current(&events);
        let v: Vec<_> = reader.read().collect();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].amount, 42.0);
    }

    #[test]
    fn test_world_events() {
        let mut world = World::new();
        world.add_event::<DamageEvent>();
        world.send_event(DamageEvent { amount: 99.0 });
        assert!(!world.events::<DamageEvent>().unwrap().is_empty());
    }

    // ── Schedule ──────────────────────────────────────────────────────────

    #[test]
    fn test_schedule_runs_systems_in_order() {
        let mut world = World::new();
        world.insert_resource(0u32);
        let mut schedule = Schedule::new();
        schedule.add_system(into_system("inc", |w| {
            *w.resource_mut::<u32>().unwrap() += 1;
        }));
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert_eq!(*world.resource::<u32>().unwrap(), 2);
    }

    #[test]
    fn test_schedule_run_condition() {
        let mut world = World::new();
        world.insert_resource(0u32);
        world.insert_resource(false);
        let mut schedule = Schedule::new();
        schedule.add_system_with_condition(
            into_system("gated", |w| { *w.resource_mut::<u32>().unwrap() += 1; }),
            |w| *w.resource::<bool>().unwrap(),
        );
        schedule.run(&mut world);
        assert_eq!(*world.resource::<u32>().unwrap(), 0);
        *world.resource_mut::<bool>().unwrap() = true;
        schedule.run(&mut world);
        assert_eq!(*world.resource::<u32>().unwrap(), 1);
    }

    #[test]
    fn test_schedule_remove_by_label() {
        let mut schedule = Schedule::new();
        schedule.add_system_with_label(into_system("noop", |_| {}), "my_label");
        assert_eq!(schedule.system_count(), 1);
        schedule.remove_system(&SystemLabel::new("my_label"));
        assert_eq!(schedule.system_count(), 0);
    }

    #[test]
    fn test_schedule_increments_tick() {
        let mut world = World::new();
        let mut schedule = Schedule::new();
        schedule.run(&mut world);
        assert_eq!(world.tick(), 1);
        schedule.run(&mut world);
        assert_eq!(world.tick(), 2);
    }

    // ── Change detection ──────────────────────────────────────────────────

    #[test]
    fn test_was_added() {
        let mut world = World::new();
        world.increment_tick(); // tick = 1
        let e = world.spawn(Health(100.0));
        assert!(was_added::<Health>(&world, e, 0));
        assert!(!was_added::<Health>(&world, e, 1));
    }

    #[test]
    fn test_was_changed() {
        let mut world = World::new();
        world.increment_tick(); // tick = 1
        let e = world.spawn(Health(100.0));
        world.increment_tick(); // tick = 2
        world.get_mut::<Health>(e).unwrap().0 = 50.0;
        assert!(was_changed::<Health>(&world, e, 1));
        assert!(!was_changed::<Health>(&world, e, 2));
    }

    #[test]
    fn test_query_added_filter() {
        let mut world = World::new();
        world.increment_tick(); // tick = 1
        let e1 = world.spawn(Health(100.0));
        world.increment_tick(); // tick = 2
        let _e2 = world.spawn(Health(50.0));
        let added: Vec<_> = query_added::<Health>(&world, 1).collect();
        assert_eq!(added.len(), 1);
        assert!(!added.contains(&e1));
    }

    #[test]
    fn test_query_changed_filter() {
        let mut world = World::new();
        world.increment_tick();
        let e1 = world.spawn(Health(100.0));
        let e2 = world.spawn(Health(50.0));
        world.increment_tick();
        world.get_mut::<Health>(e1).unwrap().0 = 90.0;
        let changed: Vec<_> = query_changed::<Health>(&world, 1).collect();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&e1));
        assert!(!changed.contains(&e2));
    }

    // ── Local<T> ──────────────────────────────────────────────────────────

    #[test]
    fn test_local_default_and_mutate() {
        let mut local: Local<u32> = Local::default_value();
        assert_eq!(*local, 0);
        *local += 5;
        assert_eq!(*local, 5);
    }

    #[test]
    fn test_local_new() {
        let local: Local<String> = Local::new("hello".to_string());
        assert_eq!(*local, "hello");
    }

    // ── Structural integrity ──────────────────────────────────────────────

    #[test]
    fn test_swap_remove_updates_location() {
        let mut world = World::new();
        let e1 = world.spawn(Position { x: 1.0, y: 0.0 });
        let e2 = world.spawn(Position { x: 2.0, y: 0.0 });
        let e3 = world.spawn(Position { x: 3.0, y: 0.0 });
        world.despawn(e1); // e3 moves to row 0
        assert!(!world.is_alive(e1));
        assert!(world.is_alive(e2));
        assert!(world.is_alive(e3));
        assert_eq!(world.get::<Position>(e3).unwrap().x, 3.0);
    }

    #[test]
    fn test_spawn_many_entities() {
        let mut world = World::new();
        for i in 0..1000u32 {
            world.spawn(Position { x: i as f32, y: 0.0 });
        }
        assert_eq!(world.entity_count(), 1000);
        assert_eq!(world.query::<Read<Position>, ()>().count(), 1000);
    }

    #[test]
    fn test_world_default_is_empty() {
        let world = World::default();
        assert_eq!(world.entity_count(), 0);
        assert_eq!(world.archetype_count(), 0);
    }

    #[test]
    fn test_entity_count_after_despawns() {
        let mut world = World::new();
        let entities: Vec<Entity> = (0..10).map(|i| world.spawn(Health(i as f32))).collect();
        for &e in &entities[0..5] { world.despawn(e); }
        assert_eq!(world.entity_count(), 5);
    }

    #[test]
    fn test_multiple_component_types_different_archetypes() {
        let mut world = World::new();
        world.spawn(Position { x: 0.0, y: 0.0 });
        world.spawn(Health(100.0));
        world.spawn((Position { x: 1.0, y: 0.0 }, Health(50.0)));
        // Three distinct archetypes
        assert_eq!(world.archetype_count(), 3);
        assert_eq!(world.query::<Read<Position>, ()>().count(), 2);
        assert_eq!(world.query::<Read<Health>, ()>().count(), 2);
        assert_eq!(world.query::<(Read<Position>, Read<Health>), ()>().count(), 1);
    }
}
