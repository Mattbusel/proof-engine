//! Query system for the ECS.
//!
//! Queries allow systems to iterate over entities that match a set of component
//! requirements, with optional filters.
//!
//! # Example
//! ```rust,ignore
//! // Iterate all entities with Position and Velocity.
//! let results = QueryBuilder::<(&Position, &Velocity)>::new(&world).run();
//! for (entity, (pos, vel)) in results {
//!     // ...
//! }
//! ```
//!
//! # Design
//! - [`WorldQuery`] is the trait that types must implement to be query items.
//! - [`QueryFilter`] is the trait for optional filters like `With<T>` / `Without<T>`.
//! - [`QueryState`] caches which TypeIds are required (enabling future archetype caching).
//! - [`QueryBuilder`] is the main entry point, combining a query and optional filter.

use std::any::TypeId;
use std::marker::PhantomData;

use super::entity::Entity;
use super::storage::Component;
use super::world::World;

// ---------------------------------------------------------------------------
// WorldQuery trait
// ---------------------------------------------------------------------------

/// A type that can be used as a query item.
///
/// Implementing types describe what component data a query fetches and how to
/// fetch it from the world.
pub trait WorldQuery: 'static {
    /// The item type yielded per entity on each iteration.
    type Item<'w>;

    /// The TypeIds of all components that must be present for a match.
    fn required_type_ids() -> Vec<TypeId>;

    /// Fetch the item for `entity` from the world. Returns `None` if missing.
    fn fetch<'w>(world: &'w World, entity: Entity) -> Option<Self::Item<'w>>;
}

// ---------------------------------------------------------------------------
// Ref<T> — alternative named wrapper for immutable access
// ---------------------------------------------------------------------------

/// Named marker for immutable component access. Equivalent to using `&T` directly.
pub struct Ref<T>(PhantomData<T>);

impl<T: Component> WorldQuery for Ref<T> {
    type Item<'w> = &'w T;

    fn required_type_ids() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn fetch<'w>(world: &'w World, entity: Entity) -> Option<&'w T> {
        world.get::<T>(entity)
    }
}

// ---------------------------------------------------------------------------
// WorldQuery for &mut T  (NOTE: requires &mut World)
// ---------------------------------------------------------------------------

/// Marker for mutable component access in queries.
/// Because Rust's borrow checker can't statically verify that mutable query
/// results don't alias, mutable queries work by collecting into owned `Vec`
/// results (via `QueryBuilderMut`) or by using for_each_mut patterns.
pub struct Mut<T>(PhantomData<T>);

impl<T: Component> WorldQuery for Mut<T> {
    type Item<'w> = &'w mut T;

    fn required_type_ids() -> Vec<TypeId> {
        vec![TypeId::of::<T>()]
    }

    fn fetch<'w>(world: &'w World, _entity: Entity) -> Option<&'w mut T> {
        // Cannot fetch &mut T from &World — this is only meaningful through
        // QueryBuilderMut which takes &mut World.
        None
    }
}

// ---------------------------------------------------------------------------
// WorldQuery for Option<&T>
// ---------------------------------------------------------------------------

/// Optional component access — the query still matches even if `T` is absent.
pub struct OptionQuery<T>(PhantomData<T>);

impl<T: Component> WorldQuery for OptionQuery<T> {
    type Item<'w> = Option<&'w T>;

    fn required_type_ids() -> Vec<TypeId> {
        Vec::new() // optional — does not gate entity matching
    }

    fn fetch<'w>(world: &'w World, entity: Entity) -> Option<Option<&'w T>> {
        Some(world.get::<T>(entity))
    }
}

// ---------------------------------------------------------------------------
// WorldQuery for Entity itself
// ---------------------------------------------------------------------------

impl WorldQuery for Entity {
    type Item<'w> = Entity;

    fn required_type_ids() -> Vec<TypeId> {
        Vec::new()
    }

    fn fetch<'w>(_world: &'w World, entity: Entity) -> Option<Entity> {
        Some(entity)
    }
}

// ---------------------------------------------------------------------------
// WorldQuery for tuples
// ---------------------------------------------------------------------------

macro_rules! impl_world_query_tuple {
    ($($Q:ident),+) => {
        impl<$($Q: WorldQuery),+> WorldQuery for ($($Q,)+) {
            type Item<'w> = ($($Q::Item<'w>,)+);

            fn required_type_ids() -> Vec<TypeId> {
                let mut ids = Vec::new();
                $(ids.extend($Q::required_type_ids());)+
                ids.sort();
                ids.dedup();
                ids
            }

            fn fetch<'w>(world: &'w World, entity: Entity) -> Option<Self::Item<'w>> {
                Some(($($Q::fetch(world, entity)?,)+))
            }
        }
    };
}

impl_world_query_tuple!(Q1, Q2);
impl_world_query_tuple!(Q1, Q2, Q3);
impl_world_query_tuple!(Q1, Q2, Q3, Q4);
impl_world_query_tuple!(Q1, Q2, Q3, Q4, Q5);
impl_world_query_tuple!(Q1, Q2, Q3, Q4, Q5, Q6);

// ---------------------------------------------------------------------------
// QueryFilter trait
// ---------------------------------------------------------------------------

/// Additional predicate applied after component matching.
pub trait QueryFilter: 'static {
    /// TypeIds that must be PRESENT (in addition to query requirements).
    fn with_type_ids() -> Vec<TypeId> { Vec::new() }
    /// TypeIds that must be ABSENT.
    fn without_type_ids() -> Vec<TypeId> { Vec::new() }
    /// Runtime check on the world + entity (for Changed, Added filters).
    fn matches(world: &World, entity: Entity) -> bool;
}

// ---------------------------------------------------------------------------
// Filter types
// ---------------------------------------------------------------------------

/// Filter: entity must have component `T`.
pub struct With<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for With<T> {
    fn with_type_ids() -> Vec<TypeId> { vec![TypeId::of::<T>()] }
    fn matches(_world: &World, _entity: Entity) -> bool { true }
}

/// Filter: entity must NOT have component `T`.
pub struct Without<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for Without<T> {
    fn without_type_ids() -> Vec<TypeId> { vec![TypeId::of::<T>()] }
    fn matches(world: &World, entity: Entity) -> bool {
        !world.has::<T>(entity)
    }
}

/// Filter: component `T` was added since last system run.
pub struct Added<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for Added<T> {
    fn with_type_ids() -> Vec<TypeId> { vec![TypeId::of::<T>()] }
    fn matches(world: &World, entity: Entity) -> bool {
        if let Some(storage) = world.get_storage::<T>() {
            let borrow = storage.borrow();
            if let Some(idx) = borrow.dense_index(entity) {
                // Check added tick against world tick
                return borrow.iter_added_since(world.tick().saturating_sub(1))
                    .any(|(e, _)| *e == entity);
            }
        }
        false
    }
}

/// Filter: component `T` was changed since last system run.
pub struct Changed<T: Component>(PhantomData<T>);

impl<T: Component> QueryFilter for Changed<T> {
    fn with_type_ids() -> Vec<TypeId> { vec![TypeId::of::<T>()] }
    fn matches(world: &World, entity: Entity) -> bool {
        if let Some(storage) = world.get_storage::<T>() {
            let borrow = storage.borrow();
            return borrow.iter_changed_since(world.tick().saturating_sub(1))
                .any(|(e, _)| *e == entity);
        }
        false
    }
}

/// No filter (pass-through).
pub struct NoFilter;

impl QueryFilter for NoFilter {
    fn matches(_world: &World, _entity: Entity) -> bool { true }
}

/// AND combination of two filters.
pub struct And<F1: QueryFilter, F2: QueryFilter>(PhantomData<(F1, F2)>);

impl<F1: QueryFilter, F2: QueryFilter> QueryFilter for And<F1, F2> {
    fn with_type_ids() -> Vec<TypeId> {
        let mut ids = F1::with_type_ids();
        ids.extend(F2::with_type_ids());
        ids
    }
    fn without_type_ids() -> Vec<TypeId> {
        let mut ids = F1::without_type_ids();
        ids.extend(F2::without_type_ids());
        ids
    }
    fn matches(world: &World, entity: Entity) -> bool {
        F1::matches(world, entity) && F2::matches(world, entity)
    }
}

/// OR combination of two filters.
pub struct Or<F1: QueryFilter, F2: QueryFilter>(PhantomData<(F1, F2)>);

impl<F1: QueryFilter, F2: QueryFilter> QueryFilter for Or<F1, F2> {
    fn matches(world: &World, entity: Entity) -> bool {
        F1::matches(world, entity) || F2::matches(world, entity)
    }
}

// ---------------------------------------------------------------------------
// QueryState — cached metadata about a query
// ---------------------------------------------------------------------------

/// Caches the required TypeIds for a query, enabling fast entity filtering.
///
/// In a full archetype ECS this would also cache archetype matches.
#[derive(Debug, Clone)]
pub struct QueryState<Q: WorldQuery, F: QueryFilter = NoFilter> {
    /// Component TypeIds that must be present.
    pub required: Vec<TypeId>,
    /// Additional TypeIds required by the filter.
    pub filter_with: Vec<TypeId>,
    /// TypeIds that must be absent.
    pub filter_without: Vec<TypeId>,
    _marker: PhantomData<(Q, F)>,
}

impl<Q: WorldQuery, F: QueryFilter> QueryState<Q, F> {
    pub fn new() -> Self {
        Self {
            required: Q::required_type_ids(),
            filter_with: F::with_type_ids(),
            filter_without: F::without_type_ids(),
            _marker: PhantomData,
        }
    }

    /// All TypeIds that must be PRESENT for a match.
    pub fn all_required(&self) -> Vec<TypeId> {
        let mut ids = self.required.clone();
        ids.extend(&self.filter_with);
        ids.sort();
        ids.dedup();
        ids
    }

    /// Check whether `entity` matches this query state.
    pub fn matches(&self, world: &World, entity: Entity) -> bool {
        let required = self.all_required();
        for tid in &required {
            if !world.components.get(tid).map_or(false, |s| s.contains_erased(entity)) {
                return false;
            }
        }
        for tid in &self.filter_without {
            if world.components.get(tid).map_or(false, |s| s.contains_erased(entity)) {
                return false;
            }
        }
        F::matches(world, entity)
    }
}

impl<Q: WorldQuery, F: QueryFilter> Default for QueryState<Q, F> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// QueryBuilder
// ---------------------------------------------------------------------------

/// Main entry point for constructing and executing queries.
///
/// # Example
/// ```rust,ignore
/// let results: Vec<(Entity, (&Position, &Velocity))> =
///     QueryBuilder::<(&Position, &Velocity)>::new(&world)
///         .filter::<Without<Dead>>()
///         .run();
/// ```
pub struct QueryBuilder<'w, Q: WorldQuery> {
    world: &'w World,
    state: QueryState<Q, NoFilter>,
    extra_required: Vec<TypeId>,
    extra_excluded: Vec<TypeId>,
}

impl<'w, Q: WorldQuery> QueryBuilder<'w, Q> {
    pub fn new(world: &'w World) -> Self {
        Self {
            world,
            state: QueryState::new(),
            extra_required: Vec::new(),
            extra_excluded: Vec::new(),
        }
    }

    /// Add a `With<T>` constraint.
    pub fn with<T: Component>(mut self) -> Self {
        self.extra_required.push(TypeId::of::<T>());
        self
    }

    /// Add a `Without<T>` constraint.
    pub fn without<T: Component>(mut self) -> Self {
        self.extra_excluded.push(TypeId::of::<T>());
        self
    }

    /// Execute the query, collecting results into a `Vec`.
    pub fn run(self) -> Vec<(Entity, Q::Item<'w>)> {
        let mut required = self.state.all_required();
        required.extend(&self.extra_required);
        required.sort();
        required.dedup();

        let candidates = self.world.entities_with_all(&required);
        let mut results = Vec::new();

        for entity in candidates {
            // Check exclusions.
            let excluded = self.extra_excluded.iter().any(|tid| {
                self.world.components.get(tid).map_or(false, |s| s.contains_erased(entity))
            });
            if excluded { continue; }

            if let Some(item) = Q::fetch(self.world, entity) {
                results.push((entity, item));
            }
        }
        results
    }

    /// Execute and call `f` for each match.
    pub fn for_each(self, mut f: impl FnMut(Entity, Q::Item<'w>)) {
        for (e, item) in self.run() {
            f(e, item);
        }
    }

    /// Execute and return only entities that match.
    pub fn entities(self) -> Vec<Entity> {
        self.run().into_iter().map(|(e, _)| e).collect()
    }

    /// Count matching entities.
    pub fn count(self) -> usize {
        self.run().len()
    }

    /// Return the first match.
    pub fn first(self) -> Option<(Entity, Q::Item<'w>)> {
        self.run().into_iter().next()
    }
}

// ---------------------------------------------------------------------------
// QueryBuilderMut — mutable component access
// ---------------------------------------------------------------------------

/// A query builder that takes `&mut World`, allowing mutable component access.
pub struct QueryBuilderMut<'w, Q: WorldQuery> {
    world: &'w mut World,
    _marker: PhantomData<Q>,
}

impl<'w, Q: WorldQuery> QueryBuilderMut<'w, Q> {
    pub fn new(world: &'w mut World) -> Self {
        Self { world, _marker: PhantomData }
    }

    /// Execute, calling `f(entity, item)` for each matching entity.
    /// This is the safe API for mutable queries — we process one entity at a time.
    pub fn for_each<F>(self, mut f: F)
    where
        F: FnMut(&mut World, Entity),
    {
        let required = Q::required_type_ids();
        let entities = self.world.entities_with_all(&required);
        for entity in entities {
            f(self.world, entity);
        }
    }
}

// ---------------------------------------------------------------------------
// QueryIter — an iterator adaptor
// ---------------------------------------------------------------------------

/// An iterator that yields `(Entity, Q::Item<'w>)` for all matching entities.
pub struct QueryIter<'w, Q: WorldQuery> {
    world: &'w World,
    candidates: std::vec::IntoIter<Entity>,
    _marker: PhantomData<Q>,
}

impl<'w, Q: WorldQuery> QueryIter<'w, Q> {
    pub fn new(world: &'w World) -> Self {
        let required = Q::required_type_ids();
        let candidates = world.entities_with_all(&required);
        Self {
            world,
            candidates: candidates.into_iter(),
            _marker: PhantomData,
        }
    }

    pub fn new_filtered(world: &'w World, extra_required: &[TypeId], extra_excluded: &[TypeId]) -> Self {
        let mut required = Q::required_type_ids();
        required.extend_from_slice(extra_required);
        required.sort();
        required.dedup();

        let all = world.entities_with_all(&required);
        let candidates: Vec<Entity> = all.into_iter().filter(|e| {
            !extra_excluded.iter().any(|tid| {
                world.components.get(tid).map_or(false, |s| s.contains_erased(*e))
            })
        }).collect();

        Self {
            world,
            candidates: candidates.into_iter(),
            _marker: PhantomData,
        }
    }
}

impl<'w, Q: WorldQuery> Iterator for QueryIter<'w, Q> {
    type Item = (Entity, Q::Item<'w>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let entity = self.candidates.next()?;
            if let Some(item) = Q::fetch(self.world, entity) {
                return Some((entity, item));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FilteredQuery — combines a query with a compile-time filter
// ---------------------------------------------------------------------------

/// A query with a compile-time filter type parameter.
pub struct FilteredQuery<'w, Q: WorldQuery, F: QueryFilter = NoFilter> {
    world: &'w World,
    state: QueryState<Q, F>,
}

impl<'w, Q: WorldQuery, F: QueryFilter> FilteredQuery<'w, Q, F> {
    pub fn new(world: &'w World) -> Self {
        Self {
            world,
            state: QueryState::new(),
        }
    }

    pub fn run(self) -> Vec<(Entity, Q::Item<'w>)> {
        let required = self.state.all_required();
        let candidates = self.world.entities_with_all(&required);
        let mut results = Vec::new();

        for entity in candidates {
            // Check without filters
            let excluded = self.state.filter_without.iter().any(|tid| {
                self.world.components.get(tid).map_or(false, |s| s.contains_erased(entity))
            });
            if excluded { continue; }

            // Runtime filter
            if !F::matches(self.world, entity) { continue; }

            if let Some(item) = Q::fetch(self.world, entity) {
                results.push((entity, item));
            }
        }
        results
    }

    pub fn for_each(self, mut f: impl FnMut(Entity, Q::Item<'w>)) {
        for (e, item) in self.run() {
            f(e, item);
        }
    }

    pub fn count(self) -> usize {
        self.run().len()
    }
}

// ---------------------------------------------------------------------------
// Convenience query functions on World
// ---------------------------------------------------------------------------

impl World {
    /// Create a `QueryBuilder` for query type `Q`.
    pub fn query<Q: WorldQuery>(&self) -> QueryBuilder<'_, Q> {
        QueryBuilder::new(self)
    }

    /// Create a `QueryIter` for query type `Q`.
    pub fn query_iter<Q: WorldQuery>(&self) -> QueryIter<'_, Q> {
        QueryIter::new(self)
    }

    /// Create a filtered query.
    pub fn query_filtered<Q: WorldQuery, F: QueryFilter>(&self) -> FilteredQuery<'_, Q, F> {
        FilteredQuery::new(self)
    }
}

// ---------------------------------------------------------------------------
// QueryResult — collected query results for deferred processing
// ---------------------------------------------------------------------------

/// A snapshot of query results, useful when you need to process results
/// after taking a mutable reference to the world.
#[derive(Debug, Clone)]
pub struct QueryResult<T> {
    pub items: Vec<(Entity, T)>,
}

impl<T> QueryResult<T> {
    pub fn new(items: Vec<(Entity, T)>) -> Self {
        Self { items }
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.items.iter().map(|(e, _)| *e)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Entity, T)> {
        self.items.iter()
    }

    pub fn first(&self) -> Option<&(Entity, T)> {
        self.items.first()
    }

    pub fn filter<F: Fn(&T) -> bool>(self, pred: F) -> Self {
        QueryResult {
            items: self.items.into_iter().filter(|(_, v)| pred(v)).collect(),
        }
    }
}

impl<T> IntoIterator for QueryResult<T> {
    type Item = (Entity, T);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Position { x: f32, y: f32 }

    #[derive(Debug, Clone, PartialEq)]
    struct Velocity { dx: f32, dy: f32 }

    #[derive(Debug, Clone, PartialEq)]
    struct Health(i32);

    #[derive(Debug, Clone, PartialEq)]
    struct Tag;

    #[test]
    fn test_single_component_query() {
        let mut world = World::new();
        world.spawn().insert(Position { x: 1.0, y: 2.0 });
        world.spawn().insert(Position { x: 3.0, y: 4.0 });
        world.spawn().insert(Health(10)); // no Position

        let results = world.query::<&Position>().run();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_two_component_query() {
        let mut world = World::new();
        let e1 = world.spawn().insert(Position { x: 0.0, y: 0.0 }).insert(Velocity { dx: 1.0, dy: 0.0 }).id();
        let _e2 = world.spawn().insert(Position { x: 1.0, y: 1.0 }).id(); // no velocity
        let _e3 = world.spawn().insert(Velocity { dx: 2.0, dy: 0.0 }).id(); // no position

        let results = world.query::<(&Position, &Velocity)>().run();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e1);
    }

    #[test]
    fn test_with_filter() {
        let mut world = World::new();
        let e1 = world.spawn().insert(Position { x: 0.0, y: 0.0 }).insert(Tag).id();
        let _e2 = world.spawn().insert(Position { x: 1.0, y: 1.0 }).id(); // no Tag

        let results = world.query::<&Position>().with::<Tag>().run();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e1);
    }

    #[test]
    fn test_without_filter() {
        let mut world = World::new();
        let _e1 = world.spawn().insert(Position { x: 0.0, y: 0.0 }).insert(Tag).id();
        let e2 = world.spawn().insert(Position { x: 1.0, y: 1.0 }).id(); // no Tag

        let results = world.query::<&Position>().without::<Tag>().run();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e2);
    }

    #[test]
    fn test_query_iter() {
        let mut world = World::new();
        for i in 0..5 {
            world.spawn().insert(Health(i));
        }
        let count = world.query_iter::<&Health>().count();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_query_for_each() {
        let mut world = World::new();
        for i in 0..4i32 {
            world.spawn().insert(Health(i * 10));
        }
        let mut sum = 0i32;
        world.query::<&Health>().for_each(|_e, h| { sum += h.0; });
        assert_eq!(sum, 0 + 10 + 20 + 30);
    }

    #[test]
    fn test_query_count() {
        let mut world = World::new();
        for i in 0..7 {
            let mut b = world.spawn().insert(Position { x: i as f32, y: 0.0 });
            if i % 2 == 0 {
                let entity = b.entity();
                drop(b);
                world.insert(entity, Velocity { dx: 0.0, dy: 0.0 });
            }
        }
        let pos_vel_count = world.query::<(&Position, &Velocity)>().count();
        assert_eq!(pos_vel_count, 4); // 0, 2, 4, 6
    }

    #[test]
    fn test_entity_query() {
        let mut world = World::new();
        let e1 = world.spawn().insert(Health(1)).id();
        let e2 = world.spawn().insert(Health(2)).id();

        let entities = world.query::<Entity>().entities();
        // Entity query returns all alive entities
        assert!(entities.contains(&e1));
        assert!(entities.contains(&e2));
    }

    #[test]
    fn test_four_component_tuple_query() {
        #[derive(Debug, PartialEq)]
        struct A(i32);
        #[derive(Debug, PartialEq)]
        struct B(i32);
        #[derive(Debug, PartialEq)]
        struct C(i32);
        #[derive(Debug, PartialEq)]
        struct D(i32);

        let mut world = World::new();
        let e = world.spawn()
            .insert(A(1)).insert(B(2)).insert(C(3)).insert(D(4))
            .id();
        world.spawn().insert(A(0)).insert(B(0)); // only 2 components

        let results = world.query::<(&A, &B, &C, &D)>().run();
        assert_eq!(results.len(), 1);
        let (entity, (a, b, c, d)) = &results[0];
        assert_eq!(*entity, e);
        assert_eq!(a.0, 1);
        assert_eq!(d.0, 4);
    }

    #[test]
    fn test_filtered_query_with_type() {
        let mut world = World::new();
        let e1 = world.spawn().insert(Position { x: 0.0, y: 0.0 }).insert(Tag).id();
        let _e2 = world.spawn().insert(Position { x: 1.0, y: 0.0 }).id();

        let results = world.query_filtered::<&Position, With<Tag>>().run();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e1);
    }

    #[test]
    fn test_filtered_query_without_type() {
        let mut world = World::new();
        let _e1 = world.spawn().insert(Position { x: 0.0, y: 0.0 }).insert(Tag).id();
        let e2 = world.spawn().insert(Position { x: 1.0, y: 0.0 }).id();

        let results = world.query_filtered::<&Position, Without<Tag>>().run();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e2);
    }

    #[test]
    fn test_query_first() {
        let mut world = World::new();
        world.spawn().insert(Health(99));

        let first = world.query::<&Health>().first();
        assert!(first.is_some());
        assert_eq!(first.unwrap().1.0, 99);
    }

    #[test]
    fn test_query_result_filter() {
        let mut world = World::new();
        for i in 0..10i32 {
            world.spawn().insert(Health(i));
        }
        let raw = world.query::<&Health>().run();
        let result = QueryResult::new(raw.into_iter().map(|(e, h)| (e, h.0)).collect());
        let filtered = result.filter(|&v| v > 5);
        assert_eq!(filtered.len(), 4); // 6, 7, 8, 9
    }

    #[test]
    fn test_query_state_matches() {
        let mut world = World::new();
        let e1 = world.spawn().insert(Position { x: 0.0, y: 0.0 }).insert(Velocity { dx: 0.0, dy: 0.0 }).id();
        let e2 = world.spawn().insert(Position { x: 0.0, y: 0.0 }).id();

        let state = QueryState::<(&Position, &Velocity), NoFilter>::new();
        assert!(state.matches(&world, e1));
        assert!(!state.matches(&world, e2));
    }
}
