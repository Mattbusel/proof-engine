//! GPU Compute Pipeline
//!
//! This module provides a complete GPU compute subsystem built on top of OpenGL 4.3+
//! compute shaders via the `glow` crate. It includes:
//!
//! - **buffer**: SSBO management, typed buffers, double-buffered particle buffers,
//!   atomic counters, buffer pools, memory tracking, mapped ranges, copy engines.
//! - **dispatch**: Compute shader compilation with `#define` injection, workgroup
//!   sizing, 1D/2D/3D dispatch, indirect dispatch, pipeline caching, profiling.
//! - **kernels**: 10 built-in compute kernels as embedded GLSL source strings.
//! - **sync**: Fence synchronization, memory barriers, async compute queue,
//!   frame timeline, CPU fallback path, resource transition state machine.

pub mod buffer;
pub mod dispatch;
pub mod kernels;
pub mod sync;

pub use buffer::{
    TypedBuffer, ParticleBuffer, AtomicCounter, BufferPool, BufferBarrierType,
    MemoryTracker, MappedRange, BufferCopyEngine, BufferHandle, BufferUsage,
};
pub use dispatch::{
    ShaderSource, ComputeProgram, WorkgroupSize, ComputeDispatch, DispatchDimension,
    IndirectDispatchArgs, PipelineCache, SpecializationConstant, ComputeProfiler,
    TimingQuery,
};
pub use kernels::{
    KernelLibrary, KernelId, ParticleIntegrateParams, ParticleEmitParams,
    ForceFieldDesc, MathFunctionType, FluidDiffuseParams, HistogramParams,
    PrefixSumPlan, RadixSortPlan, FrustumCullParams, SkinningParams,
};
pub use sync::{
    FenceSync, FenceStatus, MemoryBarrierFlags, PipelineBarrier, AsyncComputeQueue,
    FrameTimeline, CpuFallback, ResourceTransition, ResourceState,
};
