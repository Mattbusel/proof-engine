//! # Netcode Subsystem
//!
//! Game engine networking: world state snapshots with delta compression,
//! reliable/unreliable transport with fragmentation and bandwidth control,
//! and state synchronization with client-side prediction and server reconciliation.

pub mod snapshot;
pub mod transport;
pub mod sync;

pub use snapshot::{
    SnapshotId, EntitySnapshot, ComponentData, WorldSnapshot,
    SnapshotDelta, EntityDelta, ComponentDelta, SnapshotRingBuffer,
    RelevancyFilter, RelevancyEntry, RelevancyRegion,
};

pub use transport::{
    PacketHeader, PacketType, ReliableChannel, UnreliableChannel,
    PacketFragmenter, FragmentHeader, ReassemblyBuffer,
    BandwidthThrottle, ConnectionState, ConnectionStateMachine,
    TransportConfig, TransportStats,
};

pub use sync::{
    InterpolationBuffer, InterpolationSample, PredictionEntry,
    ClientPrediction, AuthorityModel, AuthorityMode,
    ReplicatedProperty, PropertyFlags, DirtyTracker,
    SpawnEvent, DespawnEvent, SpawnDespawnReplicator,
    ClockSync, ClockSyncSample, SyncState,
};
