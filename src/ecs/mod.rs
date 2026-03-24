//! # ECS — Entity Component System
//!
//! A complete, production-quality ECS implementation for the Proof Engine.
//!
//! ## Architecture
//!
//! ```text
//! World
//! ├── EntityAllocator      (entity.rs)   — generational free list
//! ├── ComponentStorage<T>  (storage.rs)  — sparse-set per component type
//! ├── Resources            (world.rs)    — singleton Any values
//! ├── Query system         (query.rs)    — WorldQuery + filters
//! ├── Events               (events.rs)  — double-buffered event queues
//! ├── Commands             (commands.rs)— deferred world mutations
//! └── Schedule             (schedule.rs)— ordered system execution
//! ```
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use proof_engine::ecs::*;
//!
//! let mut world = World::new();
//! let entity = world.spawn()
//!     .insert(Position { x: 0.0, y: 0.0 })
//!     .id();
//! ```
//!
//! ## Module overview
//!
//! | Module      | Responsibility |
//! |-------------|----------------|
//! | `entity`    | `Entity`, `EntityAllocator`, `EntitySet`, `EntityMap` |
//! | `storage`   | `ComponentStorage<T>` (sparse set), `AnyComponentStorage`, `TypedStorage` |
//! | `world`     | `World`, `EntityBuilder`, `Component`/`Resource` traits |
//! | `query`     | `WorldQuery`, `QueryBuilder`, `QueryIter`, filter types |
//! | `events`    | `Events<E>`, `EventWriter`, `EventReader`, `ManualEventReader` |
//! | `commands`  | `Commands`, `EntityCommands`, `Command` trait |
//! | `schedule`  | `Schedule`, `SystemStage`, `FixedTimestep`, `SystemSet` |

pub mod entity;
pub mod storage;
pub mod world;
pub mod query;
pub mod events;
pub mod commands;
pub mod schedule;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use entity::{
    Entity,
    EntityId,
    EntityAllocator,
    EntitySet,
    EntityMap,
};

pub use storage::{
    Component,
    ComponentStorage,
    AnyComponentStorage,
    TypedStorage,
    ComponentVec,
    StorageSet,
    ComponentMask,
};

pub use world::{
    World,
    Resource,
    EntityBuilder,
    WorldCell,
    ComponentIter,
};

pub use query::{
    WorldQuery,
    QueryBuilder,
    QueryBuilderMut,
    QueryIter,
    QueryState,
    QueryResult,
    FilteredQuery,
    QueryFilter,
    NoFilter,
    With,
    Without,
    Added,
    Changed,
    And,
    Or,
    Mut,
    OptionQuery,
};

pub use events::{
    Events,
    EventId,
    EventInstance,
    EventWriter,
    EventReader,
    ManualEventReader,
    AnyEvents,
    EventQueues,
    EventBus,
};

pub use commands::{
    Command,
    Commands,
    CommandBuffer,
    EntityCommands,
    EntityCommandsBuilder,
    DeferredEntity,
    ComponentInserter,
    SpawnCommand,
    DespawnCommand,
    InsertCommand,
    RemoveCommand,
    InsertResourceCommand,
    RemoveResourceCommand,
    FnCommand,
    DespawnBatchCommand,
    WorldCommandExt,
};

pub use schedule::{
    SystemStage,
    SystemLabel,
    System,
    Schedule,
    ScheduleBuilder,
    FixedTimestep,
    SystemSet,
    RunCriteria,
    Always,
    Never,
    RunOnce,
    EveryNFrames,
    WhenResource,
};
