//! # Entity Component System (ECS)
//!
//! A standalone archetype-based ECS for the Proof Engine.
//! All types live in this single module — no external submodules required.
//!
//! ## Design
//!
//! Components are grouped by type signature into `Archetype` tables for
//! cache-friendly iteration. Fetch markers (`Read<T>`, `Write<T>`,
//! `OptionRead<T>`) carry no lifetimes so they satisfy `WorldQuery: 'static`.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use proof_engine::ecs::*;
//!
//! #[derive(Clone)] struct Pos(f32, f32);
//! impl Component for Pos {}
//!
//! let mut world = World::new();
//! let e = world.spawn(Pos(0.0, 0.0));
//! for pos in world.query::<Read<Pos>, ()>() { let _ = pos.0; }
//! ```

#![allow(dead_code)]

use std::{any::{Any, TypeId}, collections::HashMap};

// ---------------------------------------------------------------------------
// Component trait
// ---------------------------------------------------------------------------

/// Marker trait for ECS components. Implement this for any `'static + Send +
/// Sync + Clone` type to use it as a component.
pub trait Component: 'static + Send + Sync + Clone {}

// ---------------------------------------------------------------------------
// Entity (generational ID)
// ---------------------------------------------------------------------------

/// A lightweight generational handle identifying a live entity.
///
/// `index` is the slot in the allocator; `generation` differentiates reused
/// slots so stale handles can be detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    /// Slot index.
    pub index: u32,
    /// Generation counter (incremented on free).
    pub generation: u32,
}

impl Entity {
    /// Constructs an entity from raw parts.
    #[inline] pub fn from_raw(index: u32, generation: u32) -> Self { Self { index, generation } }
    /// Returns the null sentinel (never alive).
    #[inline] pub fn null() -> Self { Self { index: u32::MAX, generation: u32::MAX } }
    /// True if this is the null sentinel.
    #[inline] pub fn is_null(self) -> bool { self.index == u32::MAX }
}

impl std::fmt::Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entity({}v{})", self.index, self.generation)
    }
}

// ---------------------------------------------------------------------------
// Entity allocator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct EntityRecord { generation: u32, location: Option<EntityLocation> }

#[derive(Debug, Clone, Copy)]
struct EntityLocation { archetype_id: ArchetypeId, row: usize }

#[derive(Debug, Default)]
struct EntityAllocator { records: Vec<EntityRecord>, free: Vec<u32> }

impl EntityAllocator {
    fn alloc(&mut self) -> Entity {
        if let Some(idx) = self.free.pop() {
            let gen = self.records[idx as usize].generation;
            Entity { index: idx, generation: gen }
        } else {
            let idx = self.records.len() as u32;
            self.records.push(EntityRecord { generation: 0, location: None });
            Entity { index: idx, generation: 0 }
        }
    }
    fn free(&mut self, e: Entity) {
        let r = &mut self.records[e.index as usize];
        r.generation = r.generation.wrapping_add(1);
        r.location = None;
        self.free.push(e.index);
    }
    fn is_alive(&self, e: Entity) -> bool {
        self.records.get(e.index as usize)
            .map(|r| r.generation == e.generation && r.location.is_some())
            .unwrap_or(false)
    }
    fn location(&self, e: Entity) -> Option<EntityLocation> {
        self.records.get(e.index as usize)
            .filter(|r| r.generation == e.generation)
            .and_then(|r| r.location)
    }
    fn set_location(&mut self, e: Entity, loc: EntityLocation) {
        if let Some(r) = self.records.get_mut(e.index as usize) {
            if r.generation == e.generation { r.location = Some(loc); }
        }
    }
}

// ---------------------------------------------------------------------------
// AnyVec — type-erased dense vector
// ---------------------------------------------------------------------------

struct AnyVec {
    type_id: TypeId,
    len: usize,
    data: Vec<u8>,
    element_size: usize,
    drop_fn: unsafe fn(*mut u8),
    clone_fn: unsafe fn(*const u8, *mut u8),
}
unsafe impl Send for AnyVec {}
unsafe impl Sync for AnyVec {}

impl AnyVec {
    fn new<T: Component>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            len: 0,
            data: Vec::new(),
            element_size: std::mem::size_of::<T>(),
            drop_fn: |p| unsafe { std::ptr::drop_in_place(p as *mut T) },
            clone_fn: |s, d| unsafe { std::ptr::write(d as *mut T, (*(s as *const T)).clone()) },
        }
    }

    fn reserve_one(&mut self) {
        let sz = self.element_size;
        if sz == 0 { return; }
        let need = (self.len + 1) * sz;
        if self.data.len() < need {
            self.data.resize(need.max(self.data.len() * 2).max(sz * 4), 0);
        }
    }

    fn push<T: Component>(&mut self, v: T) {
        self.reserve_one();
        unsafe { std::ptr::write(self.data.as_mut_ptr().add(self.len * self.element_size) as *mut T, v); }
        self.len += 1;
    }

    fn get<T: Component>(&self, row: usize) -> &T {
        unsafe { &*(self.data.as_ptr().add(row * self.element_size) as *const T) }
    }

    fn get_mut<T: Component>(&mut self, row: usize) -> &mut T {
        unsafe { &mut *(self.data.as_mut_ptr().add(row * self.element_size) as *mut T) }
    }

    fn get_ptr(&self, row: usize) -> *const u8 {
        unsafe { self.data.as_ptr().add(row * self.element_size) }
    }

    fn swap_remove_raw(&mut self, row: usize) {
        let last = self.len - 1;
        let sz = self.element_size;
        unsafe {
            let dst = self.data.as_mut_ptr().add(row * sz);
            (self.drop_fn)(dst);
            if row != last {
                let src = self.data.as_ptr().add(last * sz);
                std::ptr::copy_nonoverlapping(src, dst, sz);
            }
        }
        self.len -= 1;
    }
}

impl Drop for AnyVec {
    fn drop(&mut self) {
        for i in 0..self.len {
            unsafe { (self.drop_fn)(self.data.as_mut_ptr().add(i * self.element_size)); }
        }
    }
}

// ---------------------------------------------------------------------------
// ComponentStorage — column + change-detection ticks
// ---------------------------------------------------------------------------

struct ComponentStorage {
    column: AnyVec,
    added_ticks: Vec<u32>,
    changed_ticks: Vec<u32>,
}

impl ComponentStorage {
    fn new<T: Component>() -> Self {
        Self { column: AnyVec::new::<T>(), added_ticks: Vec::new(), changed_ticks: Vec::new() }
    }

    fn push<T: Component>(&mut self, v: T, tick: u32) {
        self.column.push(v);
        self.added_ticks.push(tick);
        self.changed_ticks.push(tick);
    }

    fn swap_remove(&mut self, row: usize) {
        self.column.swap_remove_raw(row);
        let last = self.added_ticks.len() - 1;
        self.added_ticks.swap(row, last); self.added_ticks.pop();
        let last = self.changed_ticks.len() - 1;
        self.changed_ticks.swap(row, last); self.changed_ticks.pop();
    }

    /// Clone row `row` from this storage into `dst`, appending it.
    fn clone_row_into(&self, row: usize, dst: &mut ComponentStorage, tick: u32) {
        let sz = self.column.element_size;
        let need = (dst.column.len + 1) * sz;
        if dst.column.data.len() < need {
            dst.column.data.resize(need.max(dst.column.data.len() * 2).max(sz * 4), 0);
        }
        unsafe {
            let s = self.column.data.as_ptr().add(row * sz);
            let d = dst.column.data.as_mut_ptr().add(dst.column.len * sz);
            (self.column.clone_fn)(s, d);
        }
        dst.column.len += 1;
        dst.added_ticks.push(self.added_ticks[row]);
        dst.changed_ticks.push(tick);
    }
}

// ---------------------------------------------------------------------------
// Archetype
// ---------------------------------------------------------------------------

/// Unique index into [`World::archetypes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchetypeId(pub u32);

/// A table of entities that share exactly the same component type set.
pub struct Archetype {
    /// Unique id for this archetype.
    pub id: ArchetypeId,
    /// Sorted, deduplicated component `TypeId`s.
    pub component_types: Vec<TypeId>,
    columns: HashMap<TypeId, ComponentStorage>,
    /// Entity handle at each row.
    pub entities: Vec<Entity>,
}

impl Archetype {
    fn new(id: ArchetypeId, types: Vec<TypeId>) -> Self {
        Self { id, component_types: types, columns: HashMap::new(), entities: Vec::new() }
    }

    fn register_column<T: Component>(&mut self) {
        self.columns.entry(TypeId::of::<T>()).or_insert_with(ComponentStorage::new::<T>);
    }

    /// Returns `true` if this archetype has a column for `tid`.
    pub fn contains(&self, tid: TypeId) -> bool { self.columns.contains_key(&tid) }

    /// Number of entities in this archetype.
    pub fn len(&self) -> usize { self.entities.len() }

    /// `true` if empty.
    pub fn is_empty(&self) -> bool { self.entities.is_empty() }

    /// Swap-removes row `row`. Returns the entity swapped into that row, if any.
    fn swap_remove(&mut self, row: usize) -> Option<Entity> {
        let last = self.entities.len() - 1;
        for col in self.columns.values_mut() { col.swap_remove(row); }
        self.entities.swap(row, last);
        self.entities.pop();
        if row < self.entities.len() { Some(self.entities[row]) } else { None }
    }
}

// ---------------------------------------------------------------------------
// Bundle
// ---------------------------------------------------------------------------

/// A heterogeneous set of components spawned together.
pub trait Bundle: 'static + Send + Sync {
    /// Sorted `TypeId`s of all components.
    fn type_ids() -> Vec<TypeId>;
    /// Ensure all columns exist on `arch`.
    fn register_columns(arch: &mut Archetype);
    /// Push all components into `arch`.
    fn insert_into(self, arch: &mut Archetype, tick: u32);
}

impl<C: Component> Bundle for C {
    fn type_ids() -> Vec<TypeId> { vec![TypeId::of::<C>()] }
    fn register_columns(a: &mut Archetype) { a.register_column::<C>(); }
    fn insert_into(self, a: &mut Archetype, tick: u32) {
        a.columns.get_mut(&TypeId::of::<C>()).expect("col missing").push(self, tick);
    }
}

macro_rules! impl_bundle {
    ($($C:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($C: Component),+> Bundle for ($($C,)+) {
            fn type_ids() -> Vec<TypeId> {
                let mut v = vec![$(TypeId::of::<$C>()),+]; v.sort(); v.dedup(); v
            }
            fn register_columns(a: &mut Archetype) { $(a.register_column::<$C>();)+ }
            fn insert_into(self, a: &mut Archetype, tick: u32) {
                let ($($C,)+) = self;
                $(a.columns.get_mut(&TypeId::of::<$C>()).expect("col missing").push($C, tick);)+
            }
        }
    };
}
impl_bundle!(A, B);
impl_bundle!(A, B, C);
impl_bundle!(A, B, C, D);
impl_bundle!(A, B, C, D, E);
impl_bundle!(A, B, C, D, E, F);
impl_bundle!(A, B, C, D, E, F, G);
impl_bundle!(A, B, C, D, E, F, G, H);

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Type-map of global singleton resources.
#[derive(Default)]
pub struct Resources {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Resources {
    /// Creates an empty resource map.
    pub fn new() -> Self { Self::default() }
    /// Inserts (or replaces) a resource.
    pub fn insert<T: 'static + Send + Sync>(&mut self, v: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(v));
    }
    /// Returns `&T` if present.
    pub fn get<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref()
    }
    /// Returns `&mut T` if present.
    pub fn get_mut<T: 'static + Send + Sync>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }
    /// Removes and returns `T`.
    pub fn remove<T: 'static + Send + Sync>(&mut self) -> Option<T> {
        self.map.remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok()).map(|b| *b)
    }
    /// `true` if resource `T` is present.
    pub fn contains<T: 'static + Send + Sync>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }
}

// ---------------------------------------------------------------------------
// Res<T> / ResMut<T>
// ---------------------------------------------------------------------------

/// Immutable resource borrow.
pub struct Res<'a, T: 'static + Send + Sync> { inner: &'a T }
impl<'a, T: 'static + Send + Sync> Res<'a, T> {
    /// Wraps a reference.
    pub fn new(inner: &'a T) -> Self { Self { inner } }
}
impl<'a, T: 'static + Send + Sync> std::ops::Deref for Res<'a, T> {
    type Target = T; fn deref(&self) -> &T { self.inner }
}

/// Mutable resource borrow.
pub struct ResMut<'a, T: 'static + Send + Sync> { inner: &'a mut T }
impl<'a, T: 'static + Send + Sync> ResMut<'a, T> {
    /// Wraps a mutable reference.
    pub fn new(inner: &'a mut T) -> Self { Self { inner } }
}
impl<'a, T: 'static + Send + Sync> std::ops::Deref for ResMut<'a, T> {
    type Target = T; fn deref(&self) -> &T { self.inner }
}
impl<'a, T: 'static + Send + Sync> std::ops::DerefMut for ResMut<'a, T> {
    fn deref_mut(&mut self) -> &mut T { self.inner }
}

// ---------------------------------------------------------------------------
// Local<T>
// ---------------------------------------------------------------------------

/// System-local persistent state.
pub struct Local<T: Default + 'static> { value: T }
impl<T: Default + 'static> Local<T> {
    /// Initialises with `value`.
    pub fn new(value: T) -> Self { Self { value } }
    /// Initialises with `T::default()`.
    pub fn default_value() -> Self { Self { value: T::default() } }
}
impl<T: Default + 'static> std::ops::Deref for Local<T> {
    type Target = T; fn deref(&self) -> &T { &self.value }
}
impl<T: Default + 'static> std::ops::DerefMut for Local<T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.value }
}

// ---------------------------------------------------------------------------
// Events<E>
// ---------------------------------------------------------------------------

/// Double-buffered typed event queue.
pub struct Events<E: 'static + Send + Sync> {
    buffers: [Vec<E>; 2],
    current: usize,
    event_count: usize,
    start_event_count: usize,
}

impl<E: 'static + Send + Sync> Default for Events<E> {
    fn default() -> Self {
        Self { buffers: [Vec::new(), Vec::new()], current: 0, event_count: 0, start_event_count: 0 }
    }
}

impl<E: 'static + Send + Sync> Events<E> {
    /// Creates a new empty queue.
    pub fn new() -> Self { Self::default() }
    /// Appends an event.
    pub fn send(&mut self, e: E) { self.buffers[self.current].push(e); self.event_count += 1; }
    /// Swaps buffers and clears the stale one (call once per frame).
    pub fn update(&mut self) {
        let next = 1 - self.current;
        self.buffers[next].clear();
        self.start_event_count = self.event_count - self.buffers[self.current].len();
        self.current = next;
    }
    /// Returns a cursor at the current end (reads only future events).
    pub fn get_reader(&self) -> EventCursor { EventCursor { last: self.event_count } }
    /// Returns a cursor at the oldest buffered event.
    pub fn get_reader_current(&self) -> EventCursor { EventCursor { last: self.start_event_count } }
    /// Reads all events since `cursor`, advancing it.
    pub fn read<'a>(&'a self, cursor: &mut EventCursor) -> impl Iterator<Item = &'a E> {
        let start = cursor.last;
        cursor.last = self.event_count;
        let old = &self.buffers[1 - self.current];
        let new = &self.buffers[self.current];
        let base = self.start_event_count;
        let sk0 = start.saturating_sub(base);
        let tk0 = old.len().saturating_sub(sk0);
        let sk1 = start.saturating_sub(base + old.len());
        let tk1 = new.len().saturating_sub(sk1);
        old.iter().skip(sk0).take(tk0).chain(new.iter().skip(sk1).take(tk1))
    }
    /// Total events sent since creation.
    pub fn len(&self) -> usize { self.event_count }
    /// `true` if both buffers are empty.
    pub fn is_empty(&self) -> bool { self.buffers[0].is_empty() && self.buffers[1].is_empty() }
    /// Clears all buffered events.
    pub fn clear(&mut self) { self.buffers[0].clear(); self.buffers[1].clear(); }
}

/// An independent read cursor into [`Events<E>`].
#[derive(Debug, Clone, Default)]
pub struct EventCursor { last: usize }

/// Write handle for [`Events<E>`].
pub struct EventWriter<'a, E: 'static + Send + Sync> { events: &'a mut Events<E> }
impl<'a, E: 'static + Send + Sync> EventWriter<'a, E> {
    /// Creates a writer.
    pub fn new(events: &'a mut Events<E>) -> Self { Self { events } }
    /// Sends an event.
    pub fn send(&mut self, e: E) { self.events.send(e); }
    /// Sends events from an iterator.
    pub fn send_batch(&mut self, it: impl IntoIterator<Item = E>) { for e in it { self.events.send(e); } }
}

/// Read handle for [`Events<E>`] with its own cursor.
pub struct EventReader<'a, E: 'static + Send + Sync> { events: &'a Events<E>, cursor: EventCursor }
impl<'a, E: 'static + Send + Sync> EventReader<'a, E> {
    /// Reads only future events.
    pub fn new(events: &'a Events<E>) -> Self { Self { cursor: events.get_reader(), events } }
    /// Reads all buffered events.
    pub fn new_current(events: &'a Events<E>) -> Self { Self { cursor: events.get_reader_current(), events } }
    /// Returns an iterator over unread events.
    pub fn read(&mut self) -> impl Iterator<Item = &E> { self.events.read(&mut self.cursor) }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

enum Cmd {
    Spawn(Box<dyn FnOnce(&mut World) + Send + Sync>),
    Despawn(Entity),
    Insert(Box<dyn FnOnce(&mut World) + Send + Sync>),
    Remove(Entity, TypeId),
    InsertRes(Box<dyn FnOnce(&mut World) + Send + Sync>),
    RemoveRes(TypeId),
}

/// Deferred world mutations applied at end-of-frame.
pub struct Commands { queue: Vec<Cmd> }
impl Commands {
    /// Creates an empty command queue.
    pub fn new() -> Self { Self { queue: Vec::new() } }
    /// Queues a bundle spawn.
    pub fn spawn<B: Bundle>(&mut self, b: B) {
        self.queue.push(Cmd::Spawn(Box::new(move |w| { w.spawn(b); })));
    }
    /// Queues a despawn.
    pub fn despawn(&mut self, e: Entity) { self.queue.push(Cmd::Despawn(e)); }
    /// Queues a component insertion.
    pub fn insert<C: Component>(&mut self, e: Entity, c: C) {
        self.queue.push(Cmd::Insert(Box::new(move |w| w.insert(e, c))));
    }
    /// Queues a component removal.
    pub fn remove<C: Component>(&mut self, e: Entity) {
        self.queue.push(Cmd::Remove(e, TypeId::of::<C>()));
    }
    /// Queues a resource insertion.
    pub fn insert_resource<T: 'static + Send + Sync>(&mut self, v: T) {
        self.queue.push(Cmd::InsertRes(Box::new(move |w| w.resources.insert(v))));
    }
    /// Queues a resource removal.
    pub fn remove_resource<T: 'static + Send + Sync>(&mut self) {
        self.queue.push(Cmd::RemoveRes(TypeId::of::<T>()));
    }
    /// Applies all queued commands and clears the queue.
    pub fn apply(&mut self, world: &mut World) {
        for cmd in self.queue.drain(..) {
            match cmd {
                Cmd::Spawn(f) => f(world),
                Cmd::Despawn(e) => { world.despawn(e); }
                Cmd::Insert(f) => f(world),
                Cmd::Remove(e, tid) => world.remove_by_type_id(e, tid),
                Cmd::InsertRes(f) => f(world),
                Cmd::RemoveRes(tid) => { world.resources.map.remove(&tid); }
            }
        }
    }
    /// `true` if no commands are queued.
    pub fn is_empty(&self) -> bool { self.queue.is_empty() }
}
impl Default for Commands { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// Query filters
// ---------------------------------------------------------------------------

/// Requires component `T` to be present (not fetched).
pub struct With<T: Component>(std::marker::PhantomData<T>);
/// Requires component `T` to be absent.
pub struct Without<T: Component>(std::marker::PhantomData<T>);
/// Change-detection filter: component `T` was added.
pub struct Added<T: Component>(std::marker::PhantomData<T>);
/// Change-detection filter: component `T` was mutably accessed.
pub struct Changed<T: Component>(std::marker::PhantomData<T>);

/// Archetype-level filter for queries.
pub trait QueryFilter {
    /// `true` if the archetype passes this filter.
    fn matches_archetype(arch: &Archetype) -> bool;
}
impl QueryFilter for () { fn matches_archetype(_: &Archetype) -> bool { true } }
impl<T: Component> QueryFilter for With<T> {
    fn matches_archetype(a: &Archetype) -> bool { a.contains(TypeId::of::<T>()) }
}
impl<T: Component> QueryFilter for Without<T> {
    fn matches_archetype(a: &Archetype) -> bool { !a.contains(TypeId::of::<T>()) }
}
impl<A: QueryFilter, B: QueryFilter> QueryFilter for (A, B) {
    fn matches_archetype(a: &Archetype) -> bool { A::matches_archetype(a) && B::matches_archetype(a) }
}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter> QueryFilter for (A, B, C) {
    fn matches_archetype(a: &Archetype) -> bool {
        A::matches_archetype(a) && B::matches_archetype(a) && C::matches_archetype(a)
    }
}

// ---------------------------------------------------------------------------
// WorldQuery fetch markers and trait
// ---------------------------------------------------------------------------

/// Fetch marker: yields `&'w T`.
pub struct Read<T: Component>(std::marker::PhantomData<T>);
/// Fetch marker: yields `&'w mut T`.
pub struct Write<T: Component>(std::marker::PhantomData<T>);
/// Fetch marker: yields `Option<&'w T>` (entity need not have T).
pub struct OptionRead<T: Component>(std::marker::PhantomData<T>);

/// Trait for types that describe how to fetch data from an archetype row.
pub trait WorldQuery: 'static {
    /// The type yielded per entity.
    type Item<'w>;
    /// Required component `TypeId`s (must all be present in the archetype).
    fn required_types() -> Vec<TypeId>;
    /// `true` if `arch` satisfies this query.
    fn matches(arch: &Archetype) -> bool;
    /// Fetches the item for `row` in `arch`.
    ///
    /// # Safety
    /// `row` must be valid; aliasing rules must be upheld by the caller.
    unsafe fn fetch<'w>(arch: &'w Archetype, row: usize) -> Self::Item<'w>;
}

impl<T: Component> WorldQuery for Read<T> {
    type Item<'w> = &'w T;
    fn required_types() -> Vec<TypeId> { vec![TypeId::of::<T>()] }
    fn matches(a: &Archetype) -> bool { a.contains(TypeId::of::<T>()) }
    unsafe fn fetch<'w>(a: &'w Archetype, row: usize) -> &'w T {
        a.columns[&TypeId::of::<T>()].column.get::<T>(row)
    }
}

impl<T: Component> WorldQuery for Write<T> {
    type Item<'w> = &'w mut T;
    fn required_types() -> Vec<TypeId> { vec![TypeId::of::<T>()] }
    fn matches(a: &Archetype) -> bool { a.contains(TypeId::of::<T>()) }
    unsafe fn fetch<'w>(a: &'w Archetype, row: usize) -> &'w mut T {
        let ptr = a.columns.get(&TypeId::of::<T>()).unwrap().column.get_ptr(row) as *mut T;
        &mut *ptr
    }
}

impl<T: Component> WorldQuery for OptionRead<T> {
    type Item<'w> = Option<&'w T>;
    fn required_types() -> Vec<TypeId> { vec![] }
    fn matches(_: &Archetype) -> bool { true }
    unsafe fn fetch<'w>(a: &'w Archetype, row: usize) -> Option<&'w T> {
        a.columns.get(&TypeId::of::<T>()).map(|c| c.column.get::<T>(row))
    }
}

impl WorldQuery for Entity {
    type Item<'w> = Entity;
    fn required_types() -> Vec<TypeId> { vec![] }
    fn matches(_: &Archetype) -> bool { true }
    unsafe fn fetch<'w>(a: &'w Archetype, row: usize) -> Entity { a.entities[row] }
}

macro_rules! impl_wq_tuple {
    ($($Q:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($Q: WorldQuery),+> WorldQuery for ($($Q,)+) {
            type Item<'w> = ($($Q::Item<'w>,)+);
            fn required_types() -> Vec<TypeId> {
                let mut v = Vec::new(); $(v.extend($Q::required_types());)+ v.sort(); v.dedup(); v
            }
            fn matches(a: &Archetype) -> bool { $($Q::matches(a))&&+ }
            unsafe fn fetch<'w>(a: &'w Archetype, row: usize) -> ($($Q::Item<'w>,)+) {
                ($($Q::fetch(a, row),)+)
            }
        }
    };
}
impl_wq_tuple!(A);
impl_wq_tuple!(A, B);
impl_wq_tuple!(A, B, C);
impl_wq_tuple!(A, B, C, D);
impl_wq_tuple!(A, B, C, D, E);
impl_wq_tuple!(A, B, C, D, E, F);
impl_wq_tuple!(A, B, C, D, E, F, G);
impl_wq_tuple!(A, B, C, D, E, F, G, H);

// ---------------------------------------------------------------------------
// QueryIter
// ---------------------------------------------------------------------------

/// Iterator over archetypes matching query `Q` and filter `F`.
pub struct QueryIter<'w, Q: WorldQuery, F: QueryFilter> {
    archetypes: &'w [Archetype],
    arch_index: usize,
    row: usize,
    _q: std::marker::PhantomData<Q>,
    _f: std::marker::PhantomData<F>,
}
impl<'w, Q: WorldQuery, F: QueryFilter> QueryIter<'w, Q, F> {
    fn new(archetypes: &'w [Archetype]) -> Self {
        Self { archetypes, arch_index: 0, row: 0, _q: std::marker::PhantomData, _f: std::marker::PhantomData }
    }
}
impl<'w, Q: WorldQuery, F: QueryFilter> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let arch = self.archetypes.get(self.arch_index)?;
            if !Q::matches(arch) || !F::matches_archetype(arch) {
                self.arch_index += 1; self.row = 0; continue;
            }
            if self.row >= arch.len() {
                self.arch_index += 1; self.row = 0; continue;
            }
            let row = self.row; self.row += 1;
            return Some(unsafe { Q::fetch(arch, row) });
        }
    }
}

// ---------------------------------------------------------------------------
// EntityRef / EntityMut
// ---------------------------------------------------------------------------

/// Read-only view into a single entity's components.
pub struct EntityRef<'w> { arch: &'w Archetype, row: usize }
impl<'w> EntityRef<'w> {
    fn new(arch: &'w Archetype, row: usize) -> Self { Self { arch, row } }
    /// The entity's handle.
    pub fn entity(&self) -> Entity { self.arch.entities[self.row] }
    /// Returns `&T` if the entity has component `T`.
    pub fn get<T: Component>(&self) -> Option<&T> {
        self.arch.columns.get(&TypeId::of::<T>()).map(|c| c.column.get::<T>(self.row))
    }
    /// `true` if the entity has component `T`.
    pub fn has<T: Component>(&self) -> bool { self.arch.contains(TypeId::of::<T>()) }
    /// All component types on this entity.
    pub fn component_types(&self) -> &[TypeId] { &self.arch.component_types }
}

/// Read-write view into a single entity's components.
pub struct EntityMut<'w> { arch: &'w mut Archetype, row: usize }
impl<'w> EntityMut<'w> {
    fn new(arch: &'w mut Archetype, row: usize) -> Self { Self { arch, row } }
    /// The entity's handle.
    pub fn entity(&self) -> Entity { self.arch.entities[self.row] }
    /// Returns `&T` if present.
    pub fn get<T: Component>(&self) -> Option<&T> {
        self.arch.columns.get(&TypeId::of::<T>()).map(|c| c.column.get::<T>(self.row))
    }
    /// Returns `&mut T` if present.
    pub fn get_mut<T: Component>(&mut self) -> Option<&mut T> {
        self.arch.columns.get_mut(&TypeId::of::<T>()).map(|c| c.column.get_mut::<T>(self.row))
    }
    /// `true` if the entity has component `T`.
    pub fn has<T: Component>(&self) -> bool { self.arch.contains(TypeId::of::<T>()) }
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

/// The central ECS container: archetypes, entity allocator, resources, events.
pub struct World {
    archetypes: Vec<Archetype>,
    archetype_index: HashMap<Vec<TypeId>, usize>,
    entities: EntityAllocator,
    /// Global singleton resources.
    pub resources: Resources,
    events: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    tick: u32,
}

impl World {
    /// Creates an empty world.
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
    pub fn tick(&self) -> u32 { self.tick }
    /// Advances the tick counter by 1.
    pub fn increment_tick(&mut self) { self.tick = self.tick.wrapping_add(1); }

    // -- Archetype helpers --------------------------------------------------

    fn get_or_create_archetype(&mut self, mut types: Vec<TypeId>) -> usize {
        types.sort(); types.dedup();
        if let Some(&i) = self.archetype_index.get(&types) { return i; }
        let id = ArchetypeId(self.archetypes.len() as u32);
        let arch = Archetype::new(id, types.clone());
        let i = self.archetypes.len();
        self.archetypes.push(arch);
        self.archetype_index.insert(types, i);
        i
    }

    // -- Spawn / despawn ----------------------------------------------------

    /// Spawns an entity with the given bundle and returns its handle.
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> Entity {
        let types = B::type_ids();
        let ai = self.get_or_create_archetype(types);
        let entity = self.entities.alloc();
        let tick = self.tick;
        let arch = &mut self.archetypes[ai];
        B::register_columns(arch);
        let row = arch.entities.len();
        arch.entities.push(entity);
        bundle.insert_into(arch, tick);
        self.entities.set_location(entity, EntityLocation { archetype_id: ArchetypeId(ai as u32), row });
        entity
    }

    /// Despawns an entity, returning `true` if it was alive.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        let loc = match self.entities.location(entity) { Some(l) => l, None => return false };
        let ai = loc.archetype_id.0 as usize;
        let swapped = self.archetypes[ai].swap_remove(loc.row);
        self.entities.free(entity);
        if let Some(se) = swapped {
            self.entities.set_location(se, EntityLocation { archetype_id: loc.archetype_id, row: loc.row });
        }
        true
    }

    /// `true` if entity is currently alive.
    pub fn is_alive(&self, entity: Entity) -> bool { self.entities.is_alive(entity) }

    // -- Component insert / remove -----------------------------------------

    /// Inserts component `C` onto an entity, migrating its archetype if needed.
    pub fn insert<C: Component>(&mut self, entity: Entity, component: C) {
        let loc = match self.entities.location(entity) { Some(l) => l, None => return };
        let ai = loc.archetype_id.0 as usize;
        // Fast path: replace in-place.
        if self.archetypes[ai].contains(TypeId::of::<C>()) {
            let tick = self.tick;
            let col = self.archetypes[ai].columns.get_mut(&TypeId::of::<C>()).unwrap();
            *col.column.get_mut::<C>(loc.row) = component;
            col.changed_ticks[loc.row] = tick;
            return;
        }
        // Slow path: migrate to larger archetype.
        let mut new_types = self.archetypes[ai].component_types.clone();
        new_types.push(TypeId::of::<C>());
        let new_ai = self.get_or_create_archetype(new_types);
        let tick = self.tick;
        self.migrate_add::<C>(entity, loc, new_ai, component, tick);
    }

    fn migrate_add<C: Component>(
        &mut self, entity: Entity, loc: EntityLocation, new_ai: usize, extra: C, tick: u32,
    ) {
        let old_ai = loc.archetype_id.0 as usize;
        let row = loc.row;
        let old_types: Vec<TypeId> = self.archetypes[old_ai].component_types.clone();
        // Ensure destination columns exist.
        self.ensure_cols(old_ai, new_ai, &old_types);
        if !self.archetypes[new_ai].columns.contains_key(&TypeId::of::<C>()) {
            self.archetypes[new_ai].register_column::<C>();
        }
        let new_row = self.archetypes[new_ai].entities.len();
        for &tid in &old_types {
            Self::copy_row(&mut self.archetypes, old_ai, new_ai, tid, row, tick);
        }
        self.archetypes[new_ai].columns.get_mut(&TypeId::of::<C>()).unwrap().push(extra, tick);
        self.archetypes[new_ai].entities.push(entity);
        let swapped = self.archetypes[old_ai].swap_remove(row);
        self.entities.set_location(entity, EntityLocation { archetype_id: ArchetypeId(new_ai as u32), row: new_row });
        if let Some(se) = swapped {
            self.entities.set_location(se, EntityLocation { archetype_id: loc.archetype_id, row });
        }
    }

    fn ensure_cols(&mut self, src: usize, dst: usize, types: &[TypeId]) {
        for &tid in types {
            if self.archetypes[dst].columns.contains_key(&tid) { continue; }
            let s = &self.archetypes[src].columns[&tid];
            let nc = ComponentStorage {
                column: AnyVec {
                    type_id: s.column.type_id, len: 0, data: Vec::new(),
                    element_size: s.column.element_size,
                    drop_fn: s.column.drop_fn, clone_fn: s.column.clone_fn,
                },
                added_ticks: Vec::new(), changed_ticks: Vec::new(),
            };
            self.archetypes[dst].columns.insert(tid, nc);
        }
    }

    fn copy_row(archetypes: &mut Vec<Archetype>, src: usize, dst: usize, tid: TypeId, row: usize, tick: u32) {
        let (sa, da) = if src < dst {
            let (l, r) = archetypes.split_at_mut(dst); (&l[src], &mut r[0])
        } else {
            let (l, r) = archetypes.split_at_mut(src); (&r[0], &mut l[dst])
        };
        if let (Some(sc), Some(dc)) = (sa.columns.get(&tid), da.columns.get_mut(&tid)) {
            sc.clone_row_into(row, dc, tick);
        }
    }

    /// Removes component `C` from an entity.
    pub fn remove<C: Component>(&mut self, entity: Entity) {
        self.remove_by_type_id(entity, TypeId::of::<C>());
    }

    /// Removes component by raw `TypeId`.
    pub(crate) fn remove_by_type_id(&mut self, entity: Entity, tid: TypeId) {
        let loc = match self.entities.location(entity) { Some(l) => l, None => return };
        let old_ai = loc.archetype_id.0 as usize;
        if !self.archetypes[old_ai].contains(tid) { return; }
        let row = loc.row;
        let new_types: Vec<TypeId> = self.archetypes[old_ai].component_types.iter()
            .copied().filter(|&t| t != tid).collect();
        let new_ai = self.get_or_create_archetype(new_types.clone());
        let tick = self.tick;
        let old_types: Vec<TypeId> = self.archetypes[old_ai].component_types.clone();
        self.ensure_cols(old_ai, new_ai, &old_types);
        let new_row = self.archetypes[new_ai].entities.len();
        for &t in &old_types {
            if t == tid { continue; }
            Self::copy_row(&mut self.archetypes, old_ai, new_ai, t, row, tick);
        }
        self.archetypes[new_ai].entities.push(entity);
        let swapped = self.archetypes[old_ai].swap_remove(row);
        self.entities.set_location(entity, EntityLocation { archetype_id: ArchetypeId(new_ai as u32), row: new_row });
        if let Some(se) = swapped {
            self.entities.set_location(se, EntityLocation { archetype_id: loc.archetype_id, row });
        }
    }

    // -- Component access --------------------------------------------------

    /// Returns `&C` for `entity`, or `None`.
    pub fn get<C: Component>(&self, entity: Entity) -> Option<&C> {
        let loc = self.entities.location(entity)?;
        self.archetypes[loc.archetype_id.0 as usize]
            .columns.get(&TypeId::of::<C>()).map(|c| c.column.get::<C>(loc.row))
    }

    /// Returns `&mut C` for `entity`, updating changed tick.
    pub fn get_mut<C: Component>(&mut self, entity: Entity) -> Option<&mut C> {
        let loc = self.entities.location(entity)?;
        let tick = self.tick;
        let arch = &mut self.archetypes[loc.archetype_id.0 as usize];
        arch.columns.get_mut(&TypeId::of::<C>()).map(|c| {
            c.changed_ticks[loc.row] = tick;
            c.column.get_mut::<C>(loc.row)
        })
    }

    /// Read-only view of an entity's components.
    pub fn entity_ref(&self, entity: Entity) -> Option<EntityRef<'_>> {
        let loc = self.entities.location(entity)?;
        Some(EntityRef::new(&self.archetypes[loc.archetype_id.0 as usize], loc.row))
    }

    /// Read-write view of an entity's components.
    pub fn entity_mut(&mut self, entity: Entity) -> Option<EntityMut<'_>> {
        let loc = self.entities.location(entity)?;
        let ai = loc.archetype_id.0 as usize;
        Some(EntityMut::new(&mut self.archetypes[ai], loc.row))
    }

    // -- Queries -----------------------------------------------------------

    /// Returns an iterator over entities matching `Q` and filter `F`.
    pub fn query<Q: WorldQuery, F: QueryFilter>(&self) -> QueryIter<'_, Q, F> {
        QueryIter::new(&self.archetypes)
    }

    /// Query with no filter.
    pub fn query_all<Q: WorldQuery>(&self) -> QueryIter<'_, Q, ()> {
        QueryIter::new(&self.archetypes)
    }

    /// Returns the single matching entity, or `None` if zero or multiple.
    pub fn query_single<Q: WorldQuery>(&self) -> Option<Q::Item<'_>> {
        let mut it = self.query::<Q, ()>();
        let first = it.next()?;
        if it.next().is_some() { return None; }
        Some(first)
    }

    // -- Resources ---------------------------------------------------------

    /// Inserts a resource.
    pub fn insert_resource<T: 'static + Send + Sync>(&mut self, v: T) { self.resources.insert(v); }
    /// Returns `Res<T>`.
    pub fn resource<T: 'static + Send + Sync>(&self) -> Option<Res<'_, T>> {
        self.resources.get::<T>().map(Res::new)
    }
    /// Returns `ResMut<T>`.
    pub fn resource_mut<T: 'static + Send + Sync>(&mut self) -> Option<ResMut<'_, T>> {
        self.resources.get_mut::<T>().map(ResMut::new)
    }
    /// Removes and returns `T`.
    pub fn remove_resource<T: 'static + Send + Sync>(&mut self) -> Option<T> { self.resources.remove::<T>() }

    // -- Events ------------------------------------------------------------

    /// Registers event type `E`.
    pub fn add_event<E: 'static + Send + Sync>(&mut self) {
        self.events.entry(TypeId::of::<Events<E>>())
            .or_insert_with(|| Box::new(Events::<E>::new()));
    }
    /// Returns `&Events<E>`.
    pub fn events<E: 'static + Send + Sync>(&self) -> Option<&Events<E>> {
        self.events.get(&TypeId::of::<Events<E>>())?.downcast_ref()
    }
    /// Returns `&mut Events<E>`.
    pub fn events_mut<E: 'static + Send + Sync>(&mut self) -> Option<&mut Events<E>> {
        self.events.get_mut(&TypeId::of::<Events<E>>())?.downcast_mut()
    }
    /// Sends event `E` (auto-creates queue).
    pub fn send_event<E: 'static + Send + Sync>(&mut self, event: E) {
        self.events.entry(TypeId::of::<Events<E>>())
            .or_insert_with(|| Box::new(Events::<E>::new()))
            .downcast_mut::<Events<E>>().unwrap().send(event);
    }

    // -- Entity iteration --------------------------------------------------

    /// Iterator over all live entities.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.archetypes.iter().flat_map(|a| a.entities.iter().copied())
    }
    /// Total number of live entities.
    pub fn entity_count(&self) -> usize { self.archetypes.iter().map(|a| a.len()).sum() }
    /// Number of archetypes.
    pub fn archetype_count(&self) -> usize { self.archetypes.len() }
}

impl Default for World { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// SystemParam trait
// ---------------------------------------------------------------------------

/// Types extractable from a [`World`] as system parameters.
pub trait SystemParam: Sized {
    /// Per-system persistent state.
    type State: Default + Send + Sync + 'static;
    /// Initialises state from the world.
    fn init_state(world: &mut World) -> Self::State;
}

// ---------------------------------------------------------------------------
// System trait
// ---------------------------------------------------------------------------

/// A unit of logic operating on the [`World`].
pub trait System: Send + Sync + 'static {
    /// Executes the system.
    fn run(&mut self, world: &mut World);
    /// Human-readable name.
    fn name(&self) -> &str;
}

/// A system wrapping a plain closure.
pub struct FunctionSystem<F: FnMut(&mut World) + Send + Sync + 'static> { f: F, name: String }
impl<F: FnMut(&mut World) + Send + Sync + 'static> FunctionSystem<F> {
    /// Creates a named function system.
    pub fn new(name: impl Into<String>, f: F) -> Self { Self { f, name: name.into() } }
}
impl<F: FnMut(&mut World) + Send + Sync + 'static> System for FunctionSystem<F> {
    fn run(&mut self, w: &mut World) { (self.f)(w); }
    fn name(&self) -> &str { &self.name }
}

/// Wraps a closure as a boxed `System`.
pub fn into_system(name: impl Into<String>, f: impl FnMut(&mut World) + Send + Sync + 'static) -> Box<dyn System> {
    Box::new(FunctionSystem::new(name, f))
}

// ---------------------------------------------------------------------------
// Schedule
// ---------------------------------------------------------------------------

/// A human-readable system identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemLabel(pub String);
impl SystemLabel {
    /// Creates a new label.
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
}
impl<S: Into<String>> From<S> for SystemLabel { fn from(s: S) -> Self { Self(s.into()) } }

/// Boxed run-condition closure.
pub type RunCondition = Box<dyn Fn(&World) -> bool + Send + Sync>;

struct SystemEntry { system: Box<dyn System>, label: Option<SystemLabel>, run_if: Option<RunCondition> }

/// Ordered list of systems with optional labels and run conditions.
pub struct Schedule { systems: Vec<SystemEntry> }
impl Schedule {
    /// Creates an empty schedule.
    pub fn new() -> Self { Self { systems: Vec::new() } }
    /// Adds a system.
    pub fn add_system(&mut self, s: Box<dyn System>) -> &mut Self {
        self.systems.push(SystemEntry { system: s, label: None, run_if: None }); self
    }
    /// Adds a labelled system.
    pub fn add_system_with_label(&mut self, s: Box<dyn System>, label: impl Into<SystemLabel>) -> &mut Self {
        self.systems.push(SystemEntry { system: s, label: Some(label.into()), run_if: None }); self
    }
    /// Adds a system with a run condition.
    pub fn add_system_with_condition(
        &mut self, s: Box<dyn System>,
        cond: impl Fn(&World) -> bool + Send + Sync + 'static,
    ) -> &mut Self {
        self.systems.push(SystemEntry { system: s, label: None, run_if: Some(Box::new(cond)) }); self
    }
    /// Adds a labelled system with a run condition.
    pub fn add_system_full(
        &mut self, s: Box<dyn System>, label: impl Into<SystemLabel>,
        cond: impl Fn(&World) -> bool + Send + Sync + 'static,
    ) -> &mut Self {
        self.systems.push(SystemEntry { system: s, label: Some(label.into()), run_if: Some(Box::new(cond)) }); self
    }
    /// Removes all systems with `label`.
    pub fn remove_system(&mut self, label: &SystemLabel) {
        self.systems.retain(|e| e.label.as_ref() != Some(label));
    }
    /// Number of system slots.
    pub fn system_count(&self) -> usize { self.systems.len() }
    /// Runs all systems, incrementing the world tick first.
    pub fn run(&mut self, world: &mut World) {
        world.increment_tick();
        for e in &mut self.systems {
            if e.run_if.as_ref().map(|c| c(world)).unwrap_or(true) { e.system.run(world); }
        }
    }
    /// Labels of all systems in order.
    pub fn labels(&self) -> Vec<Option<&SystemLabel>> { self.systems.iter().map(|e| e.label.as_ref()).collect() }
}
impl Default for Schedule { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// Change-detection helpers
// ---------------------------------------------------------------------------

/// `true` if component `C` on `entity` was added after tick `since`.
pub fn was_added<C: Component>(w: &World, entity: Entity, since: u32) -> bool {
    let loc = match w.entities.location(entity) { Some(l) => l, None => return false };
    w.archetypes[loc.archetype_id.0 as usize].columns
        .get(&TypeId::of::<C>()).map(|c| c.added_ticks[loc.row] > since).unwrap_or(false)
}

/// `true` if component `C` on `entity` was changed after tick `since`.
pub fn was_changed<C: Component>(w: &World, entity: Entity, since: u32) -> bool {
    let loc = match w.entities.location(entity) { Some(l) => l, None => return false };
    w.archetypes[loc.archetype_id.0 as usize].columns
        .get(&TypeId::of::<C>()).map(|c| c.changed_ticks[loc.row] > since).unwrap_or(false)
}

/// Entities with `C` added after tick `since`.
pub fn query_added<C: Component>(w: &World, since: u32) -> impl Iterator<Item = Entity> + '_ {
    w.archetypes.iter().flat_map(move |arch| {
        if !arch.contains(TypeId::of::<C>()) { return vec![]; }
        let col = &arch.columns[&TypeId::of::<C>()];
        arch.entities.iter().enumerate()
            .filter(move |(r, _)| col.added_ticks[*r] > since)
            .map(|(_, &e)| e).collect::<Vec<_>>()
    })
}

/// Entities with `C` changed after tick `since`.
pub fn query_changed<C: Component>(w: &World, since: u32) -> impl Iterator<Item = Entity> + '_ {
    w.archetypes.iter().flat_map(move |arch| {
        if !arch.contains(TypeId::of::<C>()) { return vec![]; }
        let col = &arch.columns[&TypeId::of::<C>()];
        arch.entities.iter().enumerate()
            .filter(move |(r, _)| col.changed_ticks[*r] > since)
            .map(|(_, &e)| e).collect::<Vec<_>>()
    })
}

// ---------------------------------------------------------------------------
// Prelude
// ---------------------------------------------------------------------------

/// Common ECS re-exports.
pub mod prelude {
    pub use super::{
        Component, Entity, World, Bundle,
        Archetype, ArchetypeId, EntityRef, EntityMut,
        Resources, Res, ResMut, Local,
        Events, EventCursor, EventWriter, EventReader,
        Commands, With, Without, Added, Changed,
        Read, Write, OptionRead,
        WorldQuery, QueryFilter, QueryIter,
        System, SystemParam, FunctionSystem, into_system,
        Schedule, SystemLabel,
        was_added, was_changed, query_added, query_changed,
    };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)] struct Position { x: f32, y: f32 }
    impl Component for Position {}

    #[derive(Debug, Clone, PartialEq)] struct Velocity { dx: f32, dy: f32 }
    impl Component for Velocity {}

    #[derive(Debug, Clone, PartialEq)] struct Health(f32);
    impl Component for Health {}

    #[derive(Debug, Clone)] struct Tag; impl Component for Tag {}
    #[derive(Debug, Clone)] struct Enemy; impl Component for Enemy {}
    #[derive(Debug, Clone)] struct Player; impl Component for Player {}

    // Entity lifecycle
    #[test] fn test_spawn_alive() {
        let mut w = World::new();
        let e = w.spawn(Position { x: 1.0, y: 2.0 });
        assert!(w.is_alive(e)); assert_eq!(w.entity_count(), 1);
    }
    #[test] fn test_despawn() {
        let mut w = World::new();
        let e = w.spawn(Position { x: 0.0, y: 0.0 });
        assert!(w.despawn(e)); assert!(!w.is_alive(e)); assert_eq!(w.entity_count(), 0);
    }
    #[test] fn test_despawn_nonexistent() {
        let mut w = World::new(); assert!(!w.despawn(Entity::from_raw(99, 99)));
    }
    #[test] fn test_null_entity() {
        assert!(Entity::null().is_null()); assert!(!Entity::from_raw(0,0).is_null());
    }
    #[test] fn test_generation_reuse() {
        let mut w = World::new();
        let e1 = w.spawn(Position { x: 0.0, y: 0.0 }); w.despawn(e1);
        let e2 = w.spawn(Position { x: 1.0, y: 0.0 });
        assert_eq!(e1.index, e2.index); assert_ne!(e1.generation, e2.generation);
        assert!(!w.is_alive(e1)); assert!(w.is_alive(e2));
    }
    #[test] fn test_entity_display() { assert_eq!(format!("{}", Entity::from_raw(5,2)), "Entity(5v2)"); }

    // Component access
    #[test] fn test_get_component() {
        let mut w = World::new(); let e = w.spawn(Position { x: 3.0, y: 4.0 });
        assert_eq!(w.get::<Position>(e).unwrap().x, 3.0);
    }
    #[test] fn test_get_mut_component() {
        let mut w = World::new(); let e = w.spawn(Position { x: 0.0, y: 0.0 });
        w.get_mut::<Position>(e).unwrap().x = 10.0;
        assert_eq!(w.get::<Position>(e).unwrap().x, 10.0);
    }
    #[test] fn test_missing_component_none() {
        let mut w = World::new(); let e = w.spawn(Position { x: 0.0, y: 0.0 });
        assert!(w.get::<Velocity>(e).is_none());
    }
    #[test] fn test_entity_ref() {
        let mut w = World::new(); let e = w.spawn((Position { x: 1.0, y: 2.0 }, Health(100.0)));
        let er = w.entity_ref(e).unwrap();
        assert_eq!(er.get::<Position>().unwrap(), &Position { x: 1.0, y: 2.0 });
        assert!(er.has::<Position>()); assert!(!er.has::<Velocity>());
    }
    #[test] fn test_entity_mut() {
        let mut w = World::new(); let e = w.spawn(Health(50.0));
        w.entity_mut(e).unwrap().get_mut::<Health>().unwrap().0 = 75.0;
        assert_eq!(w.get::<Health>(e).unwrap().0, 75.0);
    }

    // Bundle
    #[test] fn test_spawn_tuple_bundle() {
        let mut w = World::new();
        let e = w.spawn((Position { x: 1.0, y: 0.0 }, Velocity { dx: 0.5, dy: 0.0 }, Health(100.0)));
        assert!(w.get::<Position>(e).is_some()); assert!(w.get::<Velocity>(e).is_some()); assert!(w.get::<Health>(e).is_some());
    }
    #[test] fn test_same_archetype() {
        let mut w = World::new();
        let e1 = w.spawn((Position { x: 0.0, y: 0.0 }, Velocity { dx: 1.0, dy: 0.0 }));
        let e2 = w.spawn((Position { x: 5.0, y: 5.0 }, Velocity { dx: -1.0, dy: 0.0 }));
        assert_eq!(w.archetype_count(), 1);
        assert_eq!(w.get::<Position>(e1).unwrap().x, 0.0);
        assert_eq!(w.get::<Position>(e2).unwrap().x, 5.0);
    }

    // Insert / remove
    #[test] fn test_insert_new_component() {
        let mut w = World::new(); let e = w.spawn(Position { x: 0.0, y: 0.0 });
        w.insert(e, Velocity { dx: 1.0, dy: 0.0 });
        assert!(w.get::<Velocity>(e).is_some()); assert!(w.get::<Position>(e).is_some());
    }
    #[test] fn test_insert_replaces() {
        let mut w = World::new(); let e = w.spawn(Health(100.0));
        w.insert(e, Health(50.0)); assert_eq!(w.get::<Health>(e).unwrap().0, 50.0);
    }
    #[test] fn test_remove_component() {
        let mut w = World::new();
        let e = w.spawn((Position { x: 0.0, y: 0.0 }, Velocity { dx: 1.0, dy: 0.0 }));
        w.remove::<Velocity>(e);
        assert!(w.get::<Velocity>(e).is_none()); assert!(w.get::<Position>(e).is_some());
    }
    #[test] fn test_remove_missing_noop() {
        let mut w = World::new(); let e = w.spawn(Position { x: 0.0, y: 0.0 });
        w.remove::<Velocity>(e); assert!(w.is_alive(e));
    }
    #[test] fn test_insert_remove_roundtrip() {
        let mut w = World::new(); let e = w.spawn(Position { x: 1.0, y: 2.0 });
        w.insert(e, Health(99.0)); assert_eq!(w.get::<Health>(e).unwrap().0, 99.0);
        w.remove::<Health>(e); assert!(w.get::<Health>(e).is_none());
        assert_eq!(w.get::<Position>(e).unwrap().x, 1.0);
    }

    // Queries
    #[test] fn test_query_single_component() {
        let mut w = World::new();
        w.spawn(Position { x: 1.0, y: 0.0 }); w.spawn(Position { x: 2.0, y: 0.0 }); w.spawn(Position { x: 3.0, y: 0.0 });
        assert_eq!(w.query::<Read<Position>, ()>().count(), 3);
    }
    #[test] fn test_query_tuple() {
        let mut w = World::new();
        w.spawn((Position { x: 0.0, y: 0.0 }, Velocity { dx: 1.0, dy: 0.0 }));
        w.spawn(Position { x: 5.0, y: 0.0 });
        assert_eq!(w.query::<(Read<Position>, Read<Velocity>), ()>().count(), 1);
    }
    #[test] fn test_query_with_filter() {
        let mut w = World::new();
        w.spawn((Position { x: 0.0, y: 0.0 }, Tag)); w.spawn(Position { x: 1.0, y: 0.0 });
        assert_eq!(w.query::<Read<Position>, With<Tag>>().count(), 1);
    }
    #[test] fn test_query_without_filter() {
        let mut w = World::new();
        w.spawn((Position { x: 0.0, y: 0.0 }, Enemy));
        w.spawn((Position { x: 1.0, y: 0.0 }, Player));
        w.spawn(Position { x: 2.0, y: 0.0 });
        assert_eq!(w.query::<Read<Position>, Without<Enemy>>().count(), 2);
    }
    #[test] fn test_query_option_read() {
        let mut w = World::new();
        w.spawn((Position { x: 0.0, y: 0.0 }, Health(100.0))); w.spawn(Position { x: 1.0, y: 0.0 });
        let r: Vec<_> = w.query::<(Read<Position>, OptionRead<Health>), ()>().collect();
        assert_eq!(r.len(), 2); assert_eq!(r.iter().filter(|(_, h)| h.is_some()).count(), 1);
    }
    #[test] fn test_query_entity() {
        let mut w = World::new();
        let e1 = w.spawn(Position { x: 0.0, y: 0.0 }); let e2 = w.spawn(Position { x: 1.0, y: 0.0 });
        let es: Vec<Entity> = w.query::<Entity, ()>().collect();
        assert!(es.contains(&e1)); assert!(es.contains(&e2));
    }
    #[test] fn test_query_mutable() {
        let mut w = World::new();
        w.spawn(Position { x: 0.0, y: 0.0 }); w.spawn(Position { x: 1.0, y: 0.0 });
        for p in w.query::<Write<Position>, ()>() { p.x += 10.0; }
        assert!(w.query::<Read<Position>, ()>().all(|p| p.x >= 10.0));
    }
    #[test] fn test_query_single() {
        let mut w = World::new(); w.spawn(Player);
        assert!(w.query_single::<Read<Player>>().is_some());
    }
    #[test] fn test_query_single_none_multiple() {
        let mut w = World::new(); w.spawn(Player); w.spawn(Player);
        assert!(w.query_single::<Read<Player>>().is_none());
    }

    // Resources
    #[test] fn test_resource_insert_get() {
        let mut w = World::new(); w.insert_resource(42u32);
        assert_eq!(*w.resource::<u32>().unwrap(), 42);
    }
    #[test] fn test_resource_mut() {
        let mut w = World::new(); w.insert_resource(0u32);
        *w.resource_mut::<u32>().unwrap() = 99;
        assert_eq!(*w.resource::<u32>().unwrap(), 99);
    }
    #[test] fn test_resource_remove() {
        let mut w = World::new(); w.insert_resource(42u32);
        assert_eq!(w.remove_resource::<u32>(), Some(42));
        assert!(w.resource::<u32>().is_none());
    }
    #[test] fn test_resources_standalone() {
        let mut r = Resources::new(); r.insert(42u32);
        assert!(r.contains::<u32>()); assert!(!r.contains::<i32>());
        r.remove::<u32>(); assert!(!r.contains::<u32>());
    }

    // Commands
    #[test] fn test_commands_spawn() {
        let mut w = World::new(); let mut c = Commands::new();
        c.spawn(Position { x: 7.0, y: 8.0 }); c.apply(&mut w);
        assert_eq!(w.query_single::<Read<Position>>().unwrap().x, 7.0);
    }
    #[test] fn test_commands_despawn() {
        let mut w = World::new(); let e = w.spawn(Position { x: 0.0, y: 0.0 });
        let mut c = Commands::new(); c.despawn(e); c.apply(&mut w); assert!(!w.is_alive(e));
    }
    #[test] fn test_commands_insert() {
        let mut w = World::new(); let e = w.spawn(Position { x: 0.0, y: 0.0 });
        let mut c = Commands::new(); c.insert(e, Health(50.0)); c.apply(&mut w);
        assert_eq!(w.get::<Health>(e).unwrap().0, 50.0);
    }
    #[test] fn test_commands_remove() {
        let mut w = World::new(); let e = w.spawn((Position { x: 0.0, y: 0.0 }, Health(100.0)));
        let mut c = Commands::new(); c.remove::<Health>(e); c.apply(&mut w);
        assert!(w.get::<Health>(e).is_none());
    }
    #[test] fn test_commands_insert_resource() {
        let mut w = World::new(); let mut c = Commands::new();
        c.insert_resource(100i32); c.apply(&mut w);
        assert_eq!(*w.resource::<i32>().unwrap(), 100);
    }

    // Events
    #[derive(Debug, Clone, PartialEq)] struct Dmg { amount: f32 }
    #[test] fn test_events_send_read() {
        let mut ev: Events<Dmg> = Events::new(); let mut cur = ev.get_reader_current();
        ev.send(Dmg { amount: 10.0 }); ev.send(Dmg { amount: 20.0 });
        let r: Vec<_> = ev.read(&mut cur).collect();
        assert_eq!(r.len(), 2); assert_eq!(r[0].amount, 10.0);
    }
    #[test] fn test_events_update_clears() {
        let mut ev: Events<Dmg> = Events::new(); ev.send(Dmg { amount: 5.0 });
        ev.update(); ev.update(); assert!(ev.is_empty());
    }
    #[test] fn test_event_writer_reader() {
        let mut ev: Events<Dmg> = Events::new();
        EventWriter::new(&mut ev).send(Dmg { amount: 42.0 });
        let r: Vec<_> = EventReader::new_current(&ev).read().collect();
        assert_eq!(r.len(), 1); assert_eq!(r[0].amount, 42.0);
    }
    #[test] fn test_world_events() {
        let mut w = World::new(); w.add_event::<Dmg>();
        w.send_event(Dmg { amount: 99.0 }); assert!(!w.events::<Dmg>().unwrap().is_empty());
    }

    // Schedule
    #[test] fn test_schedule_runs_systems() {
        let mut w = World::new(); w.insert_resource(0u32);
        let mut s = Schedule::new();
        s.add_system(into_system("inc", |w| { *w.resource_mut::<u32>().unwrap() += 1; }));
        s.run(&mut w); s.run(&mut w); assert_eq!(*w.resource::<u32>().unwrap(), 2);
    }
    #[test] fn test_schedule_run_condition() {
        let mut w = World::new(); w.insert_resource(0u32); w.insert_resource(false);
        let mut s = Schedule::new();
        s.add_system_with_condition(
            into_system("g", |w| { *w.resource_mut::<u32>().unwrap() += 1; }),
            |w| *w.resource::<bool>().unwrap(),
        );
        s.run(&mut w); assert_eq!(*w.resource::<u32>().unwrap(), 0);
        *w.resource_mut::<bool>().unwrap() = true;
        s.run(&mut w); assert_eq!(*w.resource::<u32>().unwrap(), 1);
    }
    #[test] fn test_schedule_remove_label() {
        let mut s = Schedule::new();
        s.add_system_with_label(into_system("n", |_|{}), "lbl");
        assert_eq!(s.system_count(), 1);
        s.remove_system(&SystemLabel::new("lbl"));
        assert_eq!(s.system_count(), 0);
    }
    #[test] fn test_schedule_tick_increment() {
        let mut w = World::new(); let mut s = Schedule::new();
        s.run(&mut w); assert_eq!(w.tick(), 1); s.run(&mut w); assert_eq!(w.tick(), 2);
    }

    // Change detection
    #[test] fn test_was_added() {
        let mut w = World::new(); w.increment_tick();
        let e = w.spawn(Health(100.0));
        assert!(was_added::<Health>(&w, e, 0)); assert!(!was_added::<Health>(&w, e, 1));
    }
    #[test] fn test_was_changed() {
        let mut w = World::new(); w.increment_tick();
        let e = w.spawn(Health(100.0)); w.increment_tick();
        w.get_mut::<Health>(e).unwrap().0 = 50.0;
        assert!(was_changed::<Health>(&w, e, 1)); assert!(!was_changed::<Health>(&w, e, 2));
    }
    #[test] fn test_query_added() {
        let mut w = World::new(); w.increment_tick();
        let e1 = w.spawn(Health(100.0)); w.increment_tick(); let _e2 = w.spawn(Health(50.0));
        let added: Vec<_> = query_added::<Health>(&w, 1).collect();
        assert_eq!(added.len(), 1); assert!(!added.contains(&e1));
    }

    // Local
    #[test] fn test_local() {
        let mut l: Local<u32> = Local::default_value();
        assert_eq!(*l, 0); *l += 5; assert_eq!(*l, 5);
    }

    // Structural
    #[test] fn test_swap_remove_updates_location() {
        let mut w = World::new();
        let e1 = w.spawn(Position { x: 1.0, y: 0.0 });
        let _e2 = w.spawn(Position { x: 2.0, y: 0.0 });
        let e3 = w.spawn(Position { x: 3.0, y: 0.0 });
        w.despawn(e1);
        assert!(w.is_alive(e3)); assert_eq!(w.get::<Position>(e3).unwrap().x, 3.0);
    }
    #[test] fn test_spawn_many() {
        let mut w = World::new();
        for i in 0..1000u32 { w.spawn(Position { x: i as f32, y: 0.0 }); }
        assert_eq!(w.entity_count(), 1000);
        assert_eq!(w.query::<Read<Position>, ()>().count(), 1000);
    }
    #[test] fn test_world_default() { let w = World::default(); assert_eq!(w.entity_count(), 0); }
    #[test] fn test_multiple_archetypes() {
        let mut w = World::new();
        w.spawn(Position { x: 0.0, y: 0.0 }); w.spawn(Health(100.0));
        w.spawn((Position { x: 1.0, y: 0.0 }, Health(50.0)));
        assert_eq!(w.archetype_count(), 3);
        assert_eq!(w.query::<Read<Position>, ()>().count(), 2);
        assert_eq!(w.query::<Read<Health>, ()>().count(), 2);
        assert_eq!(w.query::<(Read<Position>, Read<Health>), ()>().count(), 1);
    }
    #[test] fn test_entity_count_after_despawn() {
        let mut w = World::new();
        let es: Vec<Entity> = (0..10).map(|i| w.spawn(Health(i as f32))).collect();
        for &e in &es[..5] { w.despawn(e); }
        assert_eq!(w.entity_count(), 5);
    }
}
