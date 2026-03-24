//! # Save / Load System
//!
//! Provides serialization, world snapshots, file-level save formats, save slot
//! management, and checkpoint-based respawn for the Proof Engine game loop.
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |---|---|
//! | `serializer` | `SerializedValue` enum (JSON-like), `Serialize`/`Deserialize` traits, built-in impls |
//! | `snapshot` | `WorldSnapshot`, `EntitySnapshot`, `SnapshotDiff` |
//! | `format` | `SaveFile`, `SaveHeader`, `SaveManager`, slot management |
//! | `checkpoint` | `Checkpoint`, `CheckpointManager`, `RespawnSystem` |
//!
//! ## Quick save example
//!
//! ```rust,no_run
//! use proof_engine::save::{
//!     snapshot::WorldSnapshot,
//!     format::{SaveFile, SaveManager},
//! };
//!
//! let mut snapshot = WorldSnapshot::new();
//! let mut save_manager = SaveManager::new("saves/");
//! save_manager.save_to_slot(0, snapshot, Default::default()).unwrap();
//! let file = save_manager.load_slot(0).unwrap();
//! ```

pub mod serializer;
pub mod snapshot;
pub mod format;
pub mod checkpoint;
pub mod compression;
pub mod migrations;
pub mod cloud;
pub mod profile;

// Key re-exports
pub use serializer::{
    DeserializeError, Deserialize as SaveDeserialize, Serialize as SaveSerialize, SerializedValue,
};
pub use snapshot::{EntitySnapshot, ResourceSnapshot, SnapshotDiff, WorldSnapshot};
pub use format::{SaveError, SaveFile, SaveHeader, SaveManager, SaveSlot};
pub use checkpoint::{Checkpoint, CheckpointManager, RespawnSystem};
