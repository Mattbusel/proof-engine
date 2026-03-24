//! Entity identity and allocation for the ECS.
//!
//! An [`Entity`] is a lightweight identifier — two `u32` fields (id and generation)
//! packed into a single `u64`. The generation counter catches use-after-free bugs:
//! when an entity is despawned and its slot is reused, the new entity has a higher
//! generation number, so old handles compare unequal.
//!
//! [`EntityAllocator`] manages a free list for O(1) allocation and deallocation.

use std::fmt;
use std::num::NonZeroU32;

// ---------------------------------------------------------------------------
// EntityId type alias
// ---------------------------------------------------------------------------

/// A raw packed representation of an entity: high 32 bits = generation, low 32 bits = id.
pub type EntityId = u64;

// ---------------------------------------------------------------------------
// Entity
// ---------------------------------------------------------------------------

/// A lightweight, copyable entity handle.
///
/// Internally stores `id` (slot index) and `generation` (reuse counter) packed
/// into a single `u64` for cheap hashing and comparison.
///
/// # Null entity
/// [`Entity::NULL`] is a sentinel value (`id=u32::MAX, gen=0`) that is never
/// returned by the allocator. Use [`Entity::is_null`] to test for it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    /// Slot index inside the allocator's array.
    pub(crate) id: u32,
    /// Monotonically increasing counter that changes every time the slot is recycled.
    pub(crate) generation: u32,
}

impl Entity {
    /// The null / sentinel entity. Never valid; returned in error paths.
    pub const NULL: Entity = Entity {
        id: u32::MAX,
        generation: 0,
    };

    /// Construct an entity from raw parts. Prefer using [`EntityAllocator::allocate`].
    #[inline]
    pub fn new(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }

    /// Return the slot index (low half of the packed representation).
    #[inline]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Return the generation counter (high half of the packed representation).
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Pack the entity into a single `u64`: high 32 bits = generation, low 32 = id.
    #[inline]
    pub fn to_bits(self) -> u64 {
        ((self.generation as u64) << 32) | (self.id as u64)
    }

    /// Reconstruct an entity from its packed `u64` representation.
    #[inline]
    pub fn from_bits(bits: u64) -> Self {
        Self {
            id: bits as u32,
            generation: (bits >> 32) as u32,
        }
    }

    /// Returns `true` if this is the null sentinel.
    #[inline]
    pub fn is_null(self) -> bool {
        self == Self::NULL
    }

    /// Returns a compact display string like `Entity(42:3)`.
    pub fn display_string(self) -> String {
        if self.is_null() {
            "Entity(NULL)".to_owned()
        } else {
            format!("Entity({}:{})", self.id, self.generation)
        }
    }
}

impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_null() {
            write!(f, "Entity::NULL")
        } else {
            write!(f, "Entity(id={}, gen={})", self.id, self.generation)
        }
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_string())
    }
}

impl Default for Entity {
    fn default() -> Self {
        Self::NULL
    }
}

impl From<Entity> for u64 {
    fn from(e: Entity) -> u64 {
        e.to_bits()
    }
}

impl From<u64> for Entity {
    fn from(bits: u64) -> Entity {
        Entity::from_bits(bits)
    }
}

impl PartialOrd for Entity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}

// ---------------------------------------------------------------------------
// AllocatorEntry — one slot in the allocator's backing array
// ---------------------------------------------------------------------------

/// Internal state for a single slot in the entity allocator.
#[derive(Debug, Clone)]
enum SlotState {
    /// Slot is alive; value is the current generation.
    Alive { generation: u32 },
    /// Slot is free; value is the next free slot index, or `u32::MAX` for end-of-list.
    Free { generation: u32, next_free: u32 },
}

impl SlotState {
    fn generation(&self) -> u32 {
        match self {
            SlotState::Alive { generation } => *generation,
            SlotState::Free { generation, .. } => *generation,
        }
    }

    fn is_alive(&self) -> bool {
        matches!(self, SlotState::Alive { .. })
    }
}

// ---------------------------------------------------------------------------
// EntityAllocator
// ---------------------------------------------------------------------------

/// Manages entity identity with a generational free list.
///
/// Allocation is O(1) amortised. Deallocation is O(1). Aliveness checks are O(1).
///
/// # Generation overflow
/// Generations are `u32`. If a single slot is recycled 2^32 times the generation
/// wraps to 0. In practice this is not a concern for game workloads.
#[derive(Debug)]
pub struct EntityAllocator {
    /// Backing array — one entry per slot ever allocated.
    slots: Vec<SlotState>,
    /// Head of the singly-linked free list. `u32::MAX` = empty.
    free_head: u32,
    /// Number of currently live entities.
    alive: usize,
    /// Total number of entities ever allocated (including freed).
    total_allocated: u64,
}

impl EntityAllocator {
    /// Create an empty allocator.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: u32::MAX,
            alive: 0,
            total_allocated: 0,
        }
    }

    /// Create an allocator pre-allocated for `capacity` entities.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            free_head: u32::MAX,
            alive: 0,
            total_allocated: 0,
        }
    }

    /// Allocate a new entity. Returns it with a fresh generation.
    ///
    /// If there are free slots they are reused; otherwise the backing array grows.
    pub fn allocate(&mut self) -> Entity {
        self.total_allocated += 1;
        self.alive += 1;

        if self.free_head != u32::MAX {
            let idx = self.free_head;
            let slot = &self.slots[idx as usize];
            let (generation, next_free) = match slot {
                SlotState::Free { generation, next_free } => (*generation, *next_free),
                SlotState::Alive { .. } => panic!("EntityAllocator: free list pointed to alive slot"),
            };
            self.free_head = next_free;
            self.slots[idx as usize] = SlotState::Alive { generation };
            Entity::new(idx, generation)
        } else {
            let idx = self.slots.len() as u32;
            self.slots.push(SlotState::Alive { generation: 0 });
            Entity::new(idx, 0)
        }
    }

    /// Allocate `count` entities, returning them in a `Vec`.
    pub fn allocate_many(&mut self, count: usize) -> Vec<Entity> {
        (0..count).map(|_| self.allocate()).collect()
    }

    /// Free an entity. Returns `true` if the entity was alive and has been freed.
    ///
    /// Returns `false` if the entity is already dead or its generation is stale.
    pub fn free(&mut self, entity: Entity) -> bool {
        if entity.is_null() {
            return false;
        }
        let idx = entity.id as usize;
        if idx >= self.slots.len() {
            return false;
        }
        match self.slots[idx] {
            SlotState::Alive { generation } if generation == entity.generation => {
                // Bump generation so old handles become stale.
                let new_gen = generation.wrapping_add(1);
                self.slots[idx] = SlotState::Free {
                    generation: new_gen,
                    next_free: self.free_head,
                };
                self.free_head = idx as u32;
                self.alive -= 1;
                true
            }
            _ => false,
        }
    }

    /// Returns `true` if `entity` is currently alive.
    #[inline]
    pub fn is_alive(&self, entity: Entity) -> bool {
        if entity.is_null() {
            return false;
        }
        let idx = entity.id as usize;
        if idx >= self.slots.len() {
            return false;
        }
        match self.slots[idx] {
            SlotState::Alive { generation } => generation == entity.generation,
            SlotState::Free { .. } => false,
        }
    }

    /// Number of currently live entities.
    #[inline]
    pub fn len(&self) -> usize {
        self.alive
    }

    /// Returns `true` if no entities are currently alive.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.alive == 0
    }

    /// Total number of entity slots ever allocated (useful for sizing sparse arrays).
    #[inline]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Total lifetime allocations (monotonically increasing).
    #[inline]
    pub fn total_allocated(&self) -> u64 {
        self.total_allocated
    }

    /// Free all entities. Resets the free list and alive count.
    /// All previously issued [`Entity`] handles become stale.
    pub fn clear(&mut self) {
        // Bump all alive slot generations before clearing.
        for slot in &mut self.slots {
            if let SlotState::Alive { generation } = slot {
                *generation = generation.wrapping_add(1);
            }
        }
        self.slots.clear();
        self.free_head = u32::MAX;
        self.alive = 0;
    }

    /// Iterate over all currently live entities.
    pub fn iter_alive(&self) -> impl Iterator<Item = Entity> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(idx, slot)| match slot {
                SlotState::Alive { generation } => Some(Entity::new(idx as u32, *generation)),
                SlotState::Free { .. } => None,
            })
    }

    /// Return a snapshot of all currently live entities as a `Vec`.
    pub fn alive_entities(&self) -> Vec<Entity> {
        self.iter_alive().collect()
    }

    /// Return the generation currently stored for a slot index.
    /// Returns `None` if the index is out of range.
    pub fn generation_of(&self, id: u32) -> Option<u32> {
        self.slots.get(id as usize).map(|s| s.generation())
    }

    /// Verify the allocator's internal invariants (debug helper).
    #[cfg(debug_assertions)]
    pub fn verify_invariants(&self) {
        let mut free_count = 0usize;
        let mut cursor = self.free_head;
        let mut visited = std::collections::HashSet::new();
        while cursor != u32::MAX {
            assert!(visited.insert(cursor), "free list cycle detected");
            match &self.slots[cursor as usize] {
                SlotState::Free { next_free, .. } => {
                    free_count += 1;
                    cursor = *next_free;
                }
                SlotState::Alive { .. } => panic!("free list contained alive slot"),
            }
        }
        let alive_count = self.slots.iter().filter(|s| s.is_alive()).count();
        assert_eq!(alive_count, self.alive, "alive count mismatch");
        assert_eq!(alive_count + free_count, self.slots.len(), "slot count mismatch");
    }
}

impl Default for EntityAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EntityAllocator {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            free_head: self.free_head,
            alive: self.alive,
            total_allocated: self.total_allocated,
        }
    }
}

// ---------------------------------------------------------------------------
// EntitySet — a lightweight set of entities backed by a sorted Vec
// ---------------------------------------------------------------------------

/// A compact, sorted set of entity handles. Useful for group membership.
#[derive(Debug, Clone, Default)]
pub struct EntitySet {
    entities: Vec<Entity>,
}

impl EntitySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self { entities: Vec::with_capacity(cap) }
    }

    /// Insert an entity. Returns `true` if it was not already present.
    pub fn insert(&mut self, entity: Entity) -> bool {
        match self.entities.binary_search(&entity) {
            Ok(_) => false,
            Err(pos) => {
                self.entities.insert(pos, entity);
                true
            }
        }
    }

    /// Remove an entity. Returns `true` if it was present.
    pub fn remove(&mut self, entity: Entity) -> bool {
        match self.entities.binary_search(&entity) {
            Ok(pos) => {
                self.entities.remove(pos);
                true
            }
            Err(_) => false,
        }
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.binary_search(&entity).is_ok()
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.entities.iter()
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }

    /// Retain only entities that are alive according to the allocator.
    pub fn retain_alive(&mut self, alloc: &EntityAllocator) {
        self.entities.retain(|e| alloc.is_alive(*e));
    }

    /// Intersection with another set.
    pub fn intersection<'a>(&'a self, other: &'a EntitySet) -> impl Iterator<Item = &'a Entity> {
        self.entities.iter().filter(move |e| other.contains(**e))
    }

    /// Union of two sets (returns a new set).
    pub fn union(&self, other: &EntitySet) -> EntitySet {
        let mut result = self.clone();
        for &e in &other.entities {
            result.insert(e);
        }
        result
    }
}

impl FromIterator<Entity> for EntitySet {
    fn from_iter<I: IntoIterator<Item = Entity>>(iter: I) -> Self {
        let mut set = EntitySet::new();
        for e in iter {
            set.insert(e);
        }
        set
    }
}

// ---------------------------------------------------------------------------
// EntityMap — map from Entity to arbitrary value
// ---------------------------------------------------------------------------

/// A map keyed by [`Entity`], backed by a `HashMap` for general use.
#[derive(Debug, Clone)]
pub struct EntityMap<V> {
    inner: std::collections::HashMap<Entity, V>,
}

impl<V> EntityMap<V> {
    pub fn new() -> Self {
        Self { inner: std::collections::HashMap::new() }
    }

    pub fn insert(&mut self, entity: Entity, value: V) -> Option<V> {
        self.inner.insert(entity, value)
    }

    pub fn remove(&mut self, entity: Entity) -> Option<V> {
        self.inner.remove(&entity)
    }

    pub fn get(&self, entity: Entity) -> Option<&V> {
        self.inner.get(&entity)
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut V> {
        self.inner.get_mut(&entity)
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.inner.contains_key(&entity)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Entity, &V)> {
        self.inner.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Entity, &mut V)> {
        self.inner.iter_mut()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Remove entries whose entity is no longer alive.
    pub fn retain_alive(&mut self, alloc: &EntityAllocator) {
        self.inner.retain(|e, _| alloc.is_alive(*e));
    }
}

impl<V> Default for EntityMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_packing() {
        let e = Entity::new(42, 7);
        assert_eq!(e.id(), 42);
        assert_eq!(e.generation(), 7);
        let bits = e.to_bits();
        let e2 = Entity::from_bits(bits);
        assert_eq!(e, e2);
    }

    #[test]
    fn test_null_entity() {
        assert!(Entity::NULL.is_null());
        assert!(!Entity::new(0, 0).is_null());
    }

    #[test]
    fn test_allocate_and_free() {
        let mut alloc = EntityAllocator::new();
        let e1 = alloc.allocate();
        let e2 = alloc.allocate();
        assert_ne!(e1, e2);
        assert!(alloc.is_alive(e1));
        assert!(alloc.is_alive(e2));
        assert_eq!(alloc.len(), 2);

        assert!(alloc.free(e1));
        assert!(!alloc.is_alive(e1));
        assert_eq!(alloc.len(), 1);

        // Double free should return false
        assert!(!alloc.free(e1));
    }

    #[test]
    fn test_generation_reuse() {
        let mut alloc = EntityAllocator::new();
        let e1 = alloc.allocate();
        alloc.free(e1);
        let e2 = alloc.allocate(); // reuses slot 0
        assert_eq!(e1.id(), e2.id());
        assert_ne!(e1.generation(), e2.generation());
        assert!(!alloc.is_alive(e1)); // stale handle
        assert!(alloc.is_alive(e2));  // fresh handle
    }

    #[test]
    fn test_allocate_many() {
        let mut alloc = EntityAllocator::new();
        let entities = alloc.allocate_many(100);
        assert_eq!(entities.len(), 100);
        assert_eq!(alloc.len(), 100);
        for e in &entities {
            assert!(alloc.is_alive(*e));
        }
    }

    #[test]
    fn test_iter_alive() {
        let mut alloc = EntityAllocator::new();
        let e1 = alloc.allocate();
        let e2 = alloc.allocate();
        let e3 = alloc.allocate();
        alloc.free(e2);

        let alive: Vec<_> = alloc.iter_alive().collect();
        assert_eq!(alive.len(), 2);
        assert!(alive.contains(&e1));
        assert!(!alive.contains(&e2));
        assert!(alive.contains(&e3));
    }

    #[test]
    fn test_entity_set() {
        let mut set = EntitySet::new();
        let e1 = Entity::new(1, 0);
        let e2 = Entity::new(2, 0);
        assert!(set.insert(e1));
        assert!(!set.insert(e1)); // duplicate
        assert!(set.insert(e2));
        assert_eq!(set.len(), 2);
        assert!(set.contains(e1));
        assert!(set.remove(e1));
        assert!(!set.contains(e1));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_entity_map() {
        let mut map: EntityMap<i32> = EntityMap::new();
        let e1 = Entity::new(1, 0);
        map.insert(e1, 42);
        assert_eq!(map.get(e1), Some(&42));
        *map.get_mut(e1).unwrap() = 99;
        assert_eq!(map.get(e1), Some(&99));
        assert_eq!(map.remove(e1), Some(99));
        assert_eq!(map.get(e1), None);
    }

    #[test]
    fn test_clear() {
        let mut alloc = EntityAllocator::new();
        let entities = alloc.allocate_many(10);
        alloc.clear();
        assert_eq!(alloc.len(), 0);
        for e in entities {
            assert!(!alloc.is_alive(e));
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_invariants() {
        let mut alloc = EntityAllocator::new();
        let _e1 = alloc.allocate();
        let e2 = alloc.allocate();
        let _e3 = alloc.allocate();
        alloc.free(e2);
        alloc.verify_invariants();
    }

    #[test]
    fn test_entity_ordering() {
        let e1 = Entity::new(0, 0);
        let e2 = Entity::new(1, 0);
        let e3 = Entity::new(0, 1);
        assert!(e1 < e2);
        assert!(e1 < e3); // generation is in high bits
    }

    #[test]
    fn test_display() {
        let e = Entity::new(5, 3);
        assert_eq!(format!("{}", e), "Entity(5:3)");
        assert_eq!(format!("{}", Entity::NULL), "Entity(NULL)");
    }
}
