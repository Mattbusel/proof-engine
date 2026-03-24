//! Declarative render graph subsystem for the Proof Engine.
//!
//! This module provides a frame-graph architecture where render passes are
//! declared as nodes in a directed acyclic graph. Resource lifetimes are
//! managed automatically, barriers are inserted between passes, and execution
//! order is derived via topological sort.
//!
//! # Architecture
//!
//! - [`graph`] — RenderGraph, RenderPass, ResourceNode, topological sort,
//!   cycle detection, conditional passes, multi-resolution, validation,
//!   graph merging, DOT export.
//! - [`resources`] — ResourceDescriptor, TransientResource, ResourcePool,
//!   aliasing, ImportedResource, version tracking, memory budgets.
//! - [`executor`] — GraphExecutor, PassContext, barrier insertion, GPU
//!   timing queries, triple-buffered frame timeline, async compute,
//!   execution statistics, hot-reload.
//! - [`passes`] — 12 built-in pass implementations for a full deferred
//!   rendering pipeline.

pub mod graph;
pub mod resources;
pub mod executor;
pub mod passes;

// Re-export the most commonly used types.
pub use graph::{
    DependencyKind, PassCondition, PassDependency, PassType, QueueAffinity, RenderGraph,
    RenderGraphBuilder, RenderPass, ResolutionScale, ResourceNode, ValidationResult,
    GraphConfig, PassConfig, ResourceConfig,
};
pub use resources::{
    DanglingKind, DanglingResource, ImportedResource, MemoryBudget, PoolFrameStats,
    ResourceDescriptor, ResourceHandle, ResourceLifetime, ResourcePool, ResourceTable,
    ResourceVersionChain, SizePolicy, TextureFormat, TransientResource, UsageFlags,
};
pub use executor::{
    AsyncComputeSchedule, BarrierKind, BoxedPassExecutor, ExecutionStats, FnPassExecutor,
    FramePacer, FrameState, FrameStatus, FrameTimeline, GraphExecutor, MultiGraphExecutor,
    PassBarrier, PassContext, PassExecutor, PassTimingQuery,
};
pub use passes::{
    BloomPass, BuiltinPass, DebugOverlayPass, DebugVisualization, DepthPrePass, DrawCall,
    DrawType, FXAAPass, FinalCompositePass, GBufferPass, LightingPass, SSAOPass, ShadowPass,
    SkyboxPass, ToneMapOperator, ToneMappingPass, TransparencyPass, WeightFunction,
    build_deferred_pipeline,
};
