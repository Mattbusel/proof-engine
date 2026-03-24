//! Deferred command system for the ECS.
//!
//! Systems that receive `&World` (immutable) can queue mutations via [`Commands`].
//! All queued commands are applied to the [`World`] after the system finishes,
//! when [`Commands::apply`] is called by the scheduler.
//!
//! # Design
//! - [`Command`] is an object-safe trait with a single `apply` method.
//! - [`Commands`] is a `Vec<Box<dyn Command>>` accumulator.
//! - [`EntityCommands`] is a fluent builder for entity-scoped commands.
//! - [`ComponentInserter`] is a type-erased trait for inserting arbitrary components.

use std::marker::PhantomData;

use super::entity::Entity;
use super::storage::Component;
use super::world::{Resource, World};

// ---------------------------------------------------------------------------
// ComponentInserter — type-erased component insertion
// ---------------------------------------------------------------------------

/// Trait for type-erased component insertion.
/// Used by `SpawnCommand` to insert a list of heterogeneous components.
pub trait ComponentInserter: Send + Sync {
    fn insert_into(self: Box<Self>, world: &mut World, entity: Entity);
    fn insert_into_ref(&mut self, world: &mut World, entity: Entity);
}

/// Concrete inserter for a specific component type.
struct TypedInserter<T: Component> {
    component: Option<T>,
}

impl<T: Component> ComponentInserter for TypedInserter<T> {
    fn insert_into(mut self: Box<Self>, world: &mut World, entity: Entity) {
        if let Some(c) = self.component.take() {
            world.insert::<T>(entity, c);
        }
    }
    fn insert_into_ref(&mut self, world: &mut World, entity: Entity) {
        if let Some(c) = self.component.take() {
            world.insert::<T>(entity, c);
        }
    }
}

// ---------------------------------------------------------------------------
// Command trait
// ---------------------------------------------------------------------------

/// A deferred mutation to the [`World`].
pub trait Command: Send + Sync + 'static {
    fn apply(self: Box<Self>, world: &mut World);
    fn debug_name(&self) -> &'static str { "Command" }
}

// ---------------------------------------------------------------------------
// Concrete command types
// ---------------------------------------------------------------------------

/// Spawn a new entity with a list of components.
pub struct SpawnCommand {
    /// Components to insert. Collected via `EntityCommandsBuilder`.
    pub inserters: Vec<Box<dyn ComponentInserter>>,
    /// If the caller captured the entity handle, it's placed here after apply.
    pub entity_out: Option<std::sync::Arc<std::sync::Mutex<Option<Entity>>>>,
}

impl SpawnCommand {
    pub fn new() -> Self {
        Self { inserters: Vec::new(), entity_out: None }
    }

    pub fn with_inserter(mut self, inserter: Box<dyn ComponentInserter>) -> Self {
        self.inserters.push(inserter);
        self
    }
}

impl Command for SpawnCommand {
    fn apply(mut self: Box<Self>, world: &mut World) {
        let entity = world.spawn_empty();
        for mut ins in self.inserters.drain(..) {
            ins.insert_into_ref(world, entity);
        }
        if let Some(out) = &self.entity_out {
            *out.lock().expect("SpawnCommand entity_out lock poisoned") = Some(entity);
        }
    }
    fn debug_name(&self) -> &'static str { "SpawnCommand" }
}

/// Despawn an entity and all its components.
pub struct DespawnCommand {
    pub entity: Entity,
}

impl Command for DespawnCommand {
    fn apply(self: Box<Self>, world: &mut World) {
        world.despawn(self.entity);
    }
    fn debug_name(&self) -> &'static str { "DespawnCommand" }
}

/// Insert (or replace) a component on an existing entity.
pub struct InsertCommand<T: Component> {
    pub entity: Entity,
    pub component: Option<T>,
}

impl<T: Component> InsertCommand<T> {
    pub fn new(entity: Entity, component: T) -> Self {
        Self { entity, component: Some(component) }
    }
}

impl<T: Component> Command for InsertCommand<T> {
    fn apply(mut self: Box<Self>, world: &mut World) {
        if let Some(c) = self.component.take() {
            world.insert::<T>(self.entity, c);
        }
    }
    fn debug_name(&self) -> &'static str { "InsertCommand" }
}

/// Remove a component from an entity.
pub struct RemoveCommand<T: Component> {
    pub entity: Entity,
    _marker: PhantomData<T>,
}

impl<T: Component> RemoveCommand<T> {
    pub fn new(entity: Entity) -> Self {
        Self { entity, _marker: PhantomData }
    }
}

impl<T: Component> Command for RemoveCommand<T> {
    fn apply(self: Box<Self>, world: &mut World) {
        world.remove::<T>(self.entity);
    }
    fn debug_name(&self) -> &'static str { "RemoveCommand" }
}

/// Insert a resource into the world.
pub struct InsertResourceCommand<T: Resource> {
    pub resource: Option<T>,
}

impl<T: Resource> InsertResourceCommand<T> {
    pub fn new(resource: T) -> Self {
        Self { resource: Some(resource) }
    }
}

impl<T: Resource> Command for InsertResourceCommand<T> {
    fn apply(mut self: Box<Self>, world: &mut World) {
        if let Some(r) = self.resource.take() {
            world.insert_resource(r);
        }
    }
    fn debug_name(&self) -> &'static str { "InsertResourceCommand" }
}

/// Remove a resource from the world.
pub struct RemoveResourceCommand<T: Resource> {
    _marker: PhantomData<T>,
}

impl<T: Resource> RemoveResourceCommand<T> {
    pub fn new() -> Self {
        Self { _marker: PhantomData }
    }
}

impl<T: Resource> Command for RemoveResourceCommand<T> {
    fn apply(self: Box<Self>, world: &mut World) {
        world.remove_resource::<T>();
    }
    fn debug_name(&self) -> &'static str { "RemoveResourceCommand" }
}

/// A custom closure command.
pub struct FnCommand {
    func: Box<dyn FnOnce(&mut World) + Send + Sync>,
    name: &'static str,
}

impl FnCommand {
    pub fn new(name: &'static str, func: impl FnOnce(&mut World) + Send + Sync + 'static) -> Self {
        Self { func: Box::new(func), name }
    }
}

impl Command for FnCommand {
    fn apply(self: Box<Self>, world: &mut World) {
        (self.func)(world);
    }
    fn debug_name(&self) -> &'static str { self.name }
}

/// Batch despawn — removes multiple entities.
pub struct DespawnBatchCommand {
    entities: Vec<Entity>,
}

impl DespawnBatchCommand {
    pub fn new(entities: Vec<Entity>) -> Self {
        Self { entities }
    }
}

impl Command for DespawnBatchCommand {
    fn apply(self: Box<Self>, world: &mut World) {
        for e in self.entities {
            world.despawn(e);
        }
    }
    fn debug_name(&self) -> &'static str { "DespawnBatchCommand" }
}

// ---------------------------------------------------------------------------
// Commands — the command buffer
// ---------------------------------------------------------------------------

/// A deferred command buffer. Systems accumulate commands here; the scheduler
/// calls [`Commands::apply`] to flush them into the world.
///
/// Commands are applied in the order they were added.
pub struct Commands {
    queue: Vec<Box<dyn Command>>,
    /// Pre-allocated entity handles for commands that need to know the entity id.
    pending_entities: std::collections::VecDeque<Entity>,
}

impl Commands {
    /// Create an empty command buffer.
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            pending_entities: std::collections::VecDeque::new(),
        }
    }

    /// Queue a spawn command. Returns an [`EntityCommandsBuilder`] for chaining.
    pub fn spawn(&mut self) -> EntityCommandsBuilder<'_> {
        EntityCommandsBuilder::spawn(self)
    }

    /// Queue spawning multiple empty entities.
    pub fn spawn_batch(&mut self, count: usize) {
        for _ in 0..count {
            self.push(SpawnCommand::new());
        }
    }

    /// Queue despawning `entity`.
    pub fn despawn(&mut self, entity: Entity) {
        self.push(DespawnCommand { entity });
    }

    /// Queue despawning multiple entities.
    pub fn despawn_batch(&mut self, entities: impl IntoIterator<Item = Entity>) {
        self.push(DespawnBatchCommand::new(entities.into_iter().collect()));
    }

    /// Get an [`EntityCommands`] builder for an existing `entity`.
    pub fn entity(&mut self, entity: Entity) -> EntityCommands<'_> {
        EntityCommands { commands: self, entity }
    }

    /// Queue inserting a resource.
    pub fn insert_resource<T: Resource>(&mut self, resource: T) {
        self.push(InsertResourceCommand::new(resource));
    }

    /// Queue removing a resource.
    pub fn remove_resource<T: Resource>(&mut self) {
        self.push(RemoveResourceCommand::<T>::new());
    }

    /// Queue a custom closure command.
    pub fn add<F: FnOnce(&mut World) + Send + Sync + 'static>(&mut self, f: F) {
        self.push(FnCommand::new("closure", f));
    }

    /// Queue a named closure command.
    pub fn add_named<F: FnOnce(&mut World) + Send + Sync + 'static>(&mut self, name: &'static str, f: F) {
        self.push(FnCommand::new(name, f));
    }

    /// Push a boxed command.
    pub fn push(&mut self, command: impl Command) {
        self.queue.push(Box::new(command));
    }

    /// Push a pre-boxed command.
    pub fn push_boxed(&mut self, command: Box<dyn Command>) {
        self.queue.push(command);
    }

    /// Apply all queued commands to `world`, consuming the queue.
    pub fn apply(&mut self, world: &mut World) {
        for command in self.queue.drain(..) {
            command.apply(world);
        }
    }

    /// Number of pending commands.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns `true` if no commands are queued.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Discard all queued commands without applying them.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Iterate command debug names.
    pub fn debug_names(&self) -> Vec<&'static str> {
        self.queue.iter().map(|c| c.debug_name()).collect()
    }
}

impl Default for Commands {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Commands(pending={})", self.queue.len())
    }
}

// ---------------------------------------------------------------------------
// EntityCommands — fluent builder for an existing entity
// ---------------------------------------------------------------------------

/// Fluent builder for issuing commands on a specific existing entity.
pub struct EntityCommands<'a> {
    commands: &'a mut Commands,
    entity: Entity,
}

impl<'a> EntityCommands<'a> {
    /// Insert (or replace) a component.
    pub fn insert<T: Component>(self, component: T) -> Self {
        self.commands.push(InsertCommand::new(self.entity, component));
        self
    }

    /// Remove a component.
    pub fn remove<T: Component>(self) -> Self {
        self.commands.push(RemoveCommand::<T>::new(self.entity));
        self
    }

    /// Queue despawning this entity.
    pub fn despawn(self) {
        self.commands.despawn(self.entity);
    }

    /// Get the entity handle.
    pub fn id(&self) -> Entity {
        self.entity
    }

    /// Add a closure that receives the world and this entity.
    pub fn add<F: FnOnce(&mut World) + Send + Sync + 'static>(self, f: F) -> Self {
        self.commands.push(FnCommand::new("entity_closure", f));
        self
    }
}

// ---------------------------------------------------------------------------
// EntityCommandsBuilder — fluent builder for a new entity
// ---------------------------------------------------------------------------

/// Fluent builder created by [`Commands::spawn`].
/// Accumulates components to insert when the spawn command is applied.
pub struct EntityCommandsBuilder<'a> {
    commands: &'a mut Commands,
    inserters: Vec<Box<dyn ComponentInserter>>,
    /// An Arc-Mutex that will hold the spawned entity once applied.
    entity_out: std::sync::Arc<std::sync::Mutex<Option<Entity>>>,
}

impl<'a> EntityCommandsBuilder<'a> {
    fn spawn(commands: &'a mut Commands) -> Self {
        let entity_out = std::sync::Arc::new(std::sync::Mutex::new(None));
        Self {
            commands,
            inserters: Vec::new(),
            entity_out,
        }
    }

    /// Add a component to insert.
    pub fn insert<T: Component>(mut self, component: T) -> Self {
        self.inserters.push(Box::new(TypedInserter { component: Some(component) }));
        self
    }

    /// Conditionally add a component.
    pub fn insert_if<T: Component>(self, condition: bool, component: impl FnOnce() -> T) -> Self {
        if condition {
            self.insert(component())
        } else {
            self
        }
    }

    /// Finalize the builder, pushing the spawn command.
    /// Returns a handle that resolves to the entity after [`Commands::apply`] is called.
    pub fn finish(self) -> DeferredEntity {
        let out = self.entity_out.clone();
        let mut cmd = SpawnCommand::new();
        for ins in self.inserters {
            cmd.inserters.push(ins);
        }
        cmd.entity_out = Some(out.clone());
        self.commands.push_boxed(Box::new(cmd));
        DeferredEntity { inner: out }
    }

    /// Finalize and discard the handle (convenience).
    pub fn done(self) {
        self.finish();
    }
}

// ---------------------------------------------------------------------------
// DeferredEntity — resolves after Commands::apply
// ---------------------------------------------------------------------------

/// A handle to an entity that hasn't been spawned yet.
/// Resolves after [`Commands::apply`] is called.
#[derive(Debug, Clone)]
pub struct DeferredEntity {
    inner: std::sync::Arc<std::sync::Mutex<Option<Entity>>>,
}

impl DeferredEntity {
    /// Returns the entity if it has been spawned, or `None` if `apply` hasn't run yet.
    pub fn get(&self) -> Option<Entity> {
        *self.inner.lock().expect("DeferredEntity lock poisoned")
    }

    /// Waits for the entity to be resolved (spin-loop, single-threaded).
    /// In production code you'd call this after `commands.apply(world)`.
    pub fn resolve(&self) -> Entity {
        self.get().expect("DeferredEntity: Commands::apply has not been called yet")
    }
}

// ---------------------------------------------------------------------------
// CommandBuffer — alias for Commands in parallel-friendly code
// ---------------------------------------------------------------------------

/// Alias for [`Commands`] — used when multiple threads each have their own
/// command buffer that is merged into the main one at frame end.
pub type CommandBuffer = Commands;

impl CommandBuffer {
    /// Merge another command buffer into this one.
    pub fn merge(&mut self, other: CommandBuffer) {
        self.queue.extend(other.queue);
    }
}

// ---------------------------------------------------------------------------
// WorldCommandExt — convenience methods on World
// ---------------------------------------------------------------------------

/// Extension methods on [`World`] that combine command creation and immediate application.
pub trait WorldCommandExt {
    /// Immediately apply a command.
    fn apply_command(&mut self, command: impl Command);
    /// Immediately apply a closure.
    fn apply_fn(&mut self, f: impl FnOnce(&mut World));
}

impl WorldCommandExt for World {
    fn apply_command(&mut self, command: impl Command) {
        Box::new(command).apply(self);
    }

    fn apply_fn(&mut self, f: impl FnOnce(&mut World)) {
        f(self);
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Health(i32);

    #[derive(Debug, Clone, PartialEq)]
    struct Mana(i32);

    #[derive(Debug, Clone, PartialEq)]
    struct Name(String);

    #[derive(Debug, Clone, PartialEq)]
    struct Score(u32);

    #[test]
    fn test_despawn_command() {
        let mut world = World::new();
        let e = world.spawn().insert(Health(10)).id();
        assert!(world.is_alive(e));

        let mut commands = Commands::new();
        commands.despawn(e);
        commands.apply(&mut world);

        assert!(!world.is_alive(e));
    }

    #[test]
    fn test_insert_command() {
        let mut world = World::new();
        let e = world.spawn_empty();

        let mut commands = Commands::new();
        commands.entity(e).insert(Health(50)).insert(Mana(30));
        commands.apply(&mut world);

        assert_eq!(world.get::<Health>(e), Some(&Health(50)));
        assert_eq!(world.get::<Mana>(e), Some(&Mana(30)));
    }

    #[test]
    fn test_remove_command() {
        let mut world = World::new();
        let e = world.spawn().insert(Health(100)).insert(Mana(50)).id();

        let mut commands = Commands::new();
        commands.entity(e).remove::<Mana>();
        commands.apply(&mut world);

        assert!(world.has::<Health>(e));
        assert!(!world.has::<Mana>(e));
    }

    #[test]
    fn test_insert_resource_command() {
        let mut world = World::new();
        let mut commands = Commands::new();
        commands.insert_resource(Score(999));
        commands.apply(&mut world);
        assert_eq!(world.resource::<Score>(), &Score(999));
    }

    #[test]
    fn test_remove_resource_command() {
        let mut world = World::new();
        world.insert_resource(Score(1));

        let mut commands = Commands::new();
        commands.remove_resource::<Score>();
        commands.apply(&mut world);

        assert!(!world.has_resource::<Score>());
    }

    #[test]
    fn test_fn_command() {
        let mut world = World::new();
        let e = world.spawn_empty();

        let mut commands = Commands::new();
        commands.add(move |w: &mut World| {
            w.insert(e, Name("custom".to_string()));
        });
        commands.apply(&mut world);

        assert_eq!(world.get::<Name>(e).map(|n| n.0.as_str()), Some("custom"));
    }

    #[test]
    fn test_spawn_command_with_deferred_entity() {
        let mut world = World::new();
        let mut commands = Commands::new();

        let deferred = commands.spawn()
            .insert(Health(77))
            .insert(Name("deferred".to_string()))
            .finish();

        assert!(deferred.get().is_none()); // not yet applied

        commands.apply(&mut world);

        let entity = deferred.resolve();
        assert!(world.is_alive(entity));
        assert_eq!(world.get::<Health>(entity), Some(&Health(77)));
    }

    #[test]
    fn test_commands_ordering() {
        let mut world = World::new();
        let mut commands = Commands::new();

        // Spawn, then insert, then remove — in order.
        let deferred = commands.spawn().insert(Health(10)).finish();
        commands.apply(&mut world);
        let e = deferred.resolve();

        let mut commands2 = Commands::new();
        commands2.entity(e).insert(Mana(20));
        commands2.entity(e).remove::<Health>();
        commands2.apply(&mut world);

        assert!(world.has::<Mana>(e));
        assert!(!world.has::<Health>(e));
    }

    #[test]
    fn test_despawn_batch() {
        let mut world = World::new();
        let entities: Vec<Entity> = (0..5).map(|i| world.spawn().insert(Health(i)).id()).collect();

        let mut commands = Commands::new();
        commands.despawn_batch(entities.clone());
        commands.apply(&mut world);

        for e in &entities {
            assert!(!world.is_alive(*e));
        }
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn test_command_buffer_merge() {
        let mut world = World::new();
        let e = world.spawn_empty();

        let mut buf1 = CommandBuffer::new();
        buf1.entity(e).insert(Health(1));

        let mut buf2 = CommandBuffer::new();
        buf2.entity(e).insert(Mana(2));

        buf1.merge(buf2);
        assert_eq!(buf1.len(), 2);
        buf1.apply(&mut world);

        assert!(world.has::<Health>(e));
        assert!(world.has::<Mana>(e));
    }

    #[test]
    fn test_commands_clear() {
        let mut commands = Commands::new();
        commands.insert_resource(Score(1));
        commands.insert_resource(Score(2));
        assert_eq!(commands.len(), 2);
        commands.clear();
        assert!(commands.is_empty());
    }

    #[test]
    fn test_world_command_ext() {
        let mut world = World::new();
        world.apply_command(InsertResourceCommand::new(Score(42)));
        assert_eq!(world.resource::<Score>(), &Score(42));
    }
}
