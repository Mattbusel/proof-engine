//! Compute-to-render synchronization: fences, memory barriers, async compute queue,
//! frame timeline, CPU fallback, resource state machine.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// GL constants
// ---------------------------------------------------------------------------

const GL_SYNC_GPU_COMMANDS_COMPLETE: u32 = 0x9117;
const GL_ALREADY_SIGNALED: u32 = 0x911A;
const GL_TIMEOUT_EXPIRED: u32 = 0x911B;
const GL_CONDITION_SATISFIED: u32 = 0x911C;
const GL_WAIT_FAILED: u32 = 0x911D;
const GL_SYNC_FLUSH_COMMANDS_BIT: u32 = 0x00000001;

// Barrier bits
const GL_VERTEX_ATTRIB_ARRAY_BARRIER_BIT: u32 = 0x00000001;
const GL_ELEMENT_ARRAY_BARRIER_BIT: u32 = 0x00000002;
const GL_UNIFORM_BARRIER_BIT: u32 = 0x00000004;
const GL_TEXTURE_FETCH_BARRIER_BIT: u32 = 0x00000008;
const GL_SHADER_IMAGE_ACCESS_BARRIER_BIT: u32 = 0x00000020;
const GL_COMMAND_BARRIER_BIT: u32 = 0x00000040;
const GL_PIXEL_BUFFER_BARRIER_BIT: u32 = 0x00000080;
const GL_TEXTURE_UPDATE_BARRIER_BIT: u32 = 0x00000100;
const GL_BUFFER_UPDATE_BARRIER_BIT: u32 = 0x00000200;
const GL_FRAMEBUFFER_BARRIER_BIT: u32 = 0x00000400;
const GL_TRANSFORM_FEEDBACK_BARRIER_BIT: u32 = 0x00000800;
const GL_ATOMIC_COUNTER_BARRIER_BIT: u32 = 0x00001000;
const GL_SHADER_STORAGE_BARRIER_BIT: u32 = 0x00002000;
const GL_ALL_BARRIER_BITS: u32 = 0xFFFFFFFF;

// ---------------------------------------------------------------------------
// FenceSync
// ---------------------------------------------------------------------------

/// Status of a GPU fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceStatus {
    /// Not yet signaled.
    Unsignaled,
    /// Already signaled — GPU work is complete.
    Signaled,
    /// Wait timed out.
    TimedOut,
    /// Wait failed (GL error).
    Failed,
    /// Fence has not been inserted yet.
    NotInserted,
}

/// A GPU fence for synchronizing compute and render work.
///
/// Insert a fence after issuing GPU commands; then poll or wait for it
/// to determine when the GPU has finished processing those commands.
pub struct FenceSync {
    sync: Option<glow::NativeFence>,
    status: FenceStatus,
    inserted_at: Option<Instant>,
}

impl FenceSync {
    /// Create a new fence (not yet inserted).
    pub fn new() -> Self {
        Self {
            sync: None,
            status: FenceStatus::NotInserted,
            inserted_at: None,
        }
    }

    /// Insert a fence into the GL command stream.
    pub fn insert(&mut self, gl: &glow::Context) {
        use glow::HasContext;
        // Delete old fence if any
        if let Some(old) = self.sync.take() {
            unsafe {
                gl.delete_sync(old);
            }
        }
        let sync = unsafe { gl.fence_sync(GL_SYNC_GPU_COMMANDS_COMPLETE, 0).unwrap() };
        self.sync = Some(sync);
        self.status = FenceStatus::Unsignaled;
        self.inserted_at = Some(Instant::now());
    }

    /// Poll the fence without blocking. Returns current status.
    pub fn poll(&mut self, gl: &glow::Context) -> FenceStatus {
        if let Some(sync) = self.sync {
            use glow::HasContext;
            let result = unsafe { gl.client_wait_sync(sync, 0, 0) };
            self.status = match result {
                GL_ALREADY_SIGNALED | GL_CONDITION_SATISFIED => FenceStatus::Signaled,
                GL_TIMEOUT_EXPIRED => FenceStatus::Unsignaled,
                GL_WAIT_FAILED => FenceStatus::Failed,
                _ => FenceStatus::Unsignaled,
            };
        }
        self.status
    }

    /// Wait for the fence with a timeout. Returns status after wait.
    pub fn wait(&mut self, gl: &glow::Context, timeout: Duration) -> FenceStatus {
        if let Some(sync) = self.sync {
            use glow::HasContext;
            let timeout_ns = timeout.as_nanos() as u64;
            let result = unsafe {
                gl.client_wait_sync(sync, GL_SYNC_FLUSH_COMMANDS_BIT, timeout_ns as i32)
            };
            self.status = match result {
                GL_ALREADY_SIGNALED | GL_CONDITION_SATISFIED => FenceStatus::Signaled,
                GL_TIMEOUT_EXPIRED => FenceStatus::TimedOut,
                GL_WAIT_FAILED => FenceStatus::Failed,
                _ => FenceStatus::Unsignaled,
            };
        }
        self.status
    }

    /// Block until the fence is signaled (infinite wait).
    pub fn wait_forever(&mut self, gl: &glow::Context) -> FenceStatus {
        self.wait(gl, Duration::from_secs(30)) // 30s practical "infinity"
    }

    /// Current status (may be stale — call poll() to refresh).
    pub fn status(&self) -> FenceStatus {
        self.status
    }

    /// Whether the fence has been signaled.
    pub fn is_signaled(&self) -> bool {
        self.status == FenceStatus::Signaled
    }

    /// How long ago the fence was inserted (wall-clock, not GPU time).
    pub fn elapsed_since_insert(&self) -> Option<Duration> {
        self.inserted_at.map(|t| t.elapsed())
    }

    /// Destroy the fence.
    pub fn destroy(self, gl: &glow::Context) {
        if let Some(sync) = self.sync {
            use glow::HasContext;
            unsafe {
                gl.delete_sync(sync);
            }
        }
    }
}

impl Default for FenceSync {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MemoryBarrierFlags
// ---------------------------------------------------------------------------

/// Flags for glMemoryBarrier, wrapped for type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBarrierFlags(pub u32);

impl MemoryBarrierFlags {
    pub const VERTEX_ATTRIB: Self = Self(GL_VERTEX_ATTRIB_ARRAY_BARRIER_BIT);
    pub const ELEMENT_ARRAY: Self = Self(GL_ELEMENT_ARRAY_BARRIER_BIT);
    pub const UNIFORM: Self = Self(GL_UNIFORM_BARRIER_BIT);
    pub const TEXTURE_FETCH: Self = Self(GL_TEXTURE_FETCH_BARRIER_BIT);
    pub const SHADER_IMAGE: Self = Self(GL_SHADER_IMAGE_ACCESS_BARRIER_BIT);
    pub const COMMAND: Self = Self(GL_COMMAND_BARRIER_BIT);
    pub const PIXEL_BUFFER: Self = Self(GL_PIXEL_BUFFER_BARRIER_BIT);
    pub const TEXTURE_UPDATE: Self = Self(GL_TEXTURE_UPDATE_BARRIER_BIT);
    pub const BUFFER_UPDATE: Self = Self(GL_BUFFER_UPDATE_BARRIER_BIT);
    pub const FRAMEBUFFER: Self = Self(GL_FRAMEBUFFER_BARRIER_BIT);
    pub const TRANSFORM_FEEDBACK: Self = Self(GL_TRANSFORM_FEEDBACK_BARRIER_BIT);
    pub const ATOMIC_COUNTER: Self = Self(GL_ATOMIC_COUNTER_BARRIER_BIT);
    pub const SHADER_STORAGE: Self = Self(GL_SHADER_STORAGE_BARRIER_BIT);
    pub const ALL: Self = Self(GL_ALL_BARRIER_BITS);

    /// Combine two barrier flag sets.
    pub fn combine(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if a specific flag is set.
    pub fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }

    /// Issue this barrier on the GL context.
    pub fn issue(self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.memory_barrier(self.0);
        }
    }
}

impl std::ops::BitOr for MemoryBarrierFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for MemoryBarrierFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

// ---------------------------------------------------------------------------
// PipelineBarrier
// ---------------------------------------------------------------------------

/// A pipeline barrier specifying which stages must complete before which can start.
#[derive(Debug, Clone)]
pub struct PipelineBarrier {
    /// Memory barrier flags.
    pub memory_flags: MemoryBarrierFlags,
    /// Optional fence for GPU-CPU sync.
    pub fence: bool,
    /// Label for debugging.
    pub label: Option<String>,
}

impl PipelineBarrier {
    /// Create a barrier with just memory flags.
    pub fn memory(flags: MemoryBarrierFlags) -> Self {
        Self {
            memory_flags: flags,
            fence: false,
            label: None,
        }
    }

    /// Create a barrier with memory flags and a fence.
    pub fn memory_and_fence(flags: MemoryBarrierFlags) -> Self {
        Self {
            memory_flags: flags,
            fence: true,
            label: None,
        }
    }

    /// Create a full barrier (all bits + fence).
    pub fn full() -> Self {
        Self {
            memory_flags: MemoryBarrierFlags::ALL,
            fence: true,
            label: None,
        }
    }

    /// Set a debug label.
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    /// Shader storage read-after-write barrier.
    pub fn ssbo_raw() -> Self {
        Self::memory(MemoryBarrierFlags::SHADER_STORAGE)
    }

    /// Shader storage + vertex attrib barrier (compute writes, vertex shader reads).
    pub fn compute_to_vertex() -> Self {
        Self::memory(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::VERTEX_ATTRIB)
    }

    /// Compute writes, indirect draw reads.
    pub fn compute_to_indirect() -> Self {
        Self::memory(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::COMMAND)
    }

    /// Execute this barrier, issuing the memory barrier and optionally a fence.
    pub fn execute(&self, gl: &glow::Context) -> Option<FenceSync> {
        self.memory_flags.issue(gl);
        if self.fence {
            let mut fence = FenceSync::new();
            fence.insert(gl);
            Some(fence)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// ResourceState & ResourceTransition
// ---------------------------------------------------------------------------

/// Possible states a GPU resource can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceState {
    /// Undefined / initial.
    Undefined,
    /// Being written by a compute shader.
    ComputeWrite,
    /// Being read by a compute shader.
    ComputeRead,
    /// Being read as vertex attribute data.
    VertexRead,
    /// Being read as an index buffer.
    IndexRead,
    /// Being read as an indirect command buffer.
    IndirectRead,
    /// Being read as a uniform buffer.
    UniformRead,
    /// Being read/written by the CPU (mapped).
    CpuAccess,
    /// Transfer source (copy read).
    TransferSrc,
    /// Transfer destination (copy write).
    TransferDst,
}

/// Tracks and validates resource state transitions, issuing barriers as needed.
pub struct ResourceTransition {
    states: HashMap<u32, ResourceState>,
}

impl ResourceTransition {
    /// Create a new transition tracker.
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Register a resource with an initial state.
    pub fn register(&mut self, resource_id: u32, initial: ResourceState) {
        self.states.insert(resource_id, initial);
    }

    /// Get current state.
    pub fn current_state(&self, resource_id: u32) -> Option<ResourceState> {
        self.states.get(&resource_id).copied()
    }

    /// Transition a resource to a new state. Returns the barrier flags needed.
    pub fn transition(
        &mut self,
        resource_id: u32,
        new_state: ResourceState,
    ) -> Option<MemoryBarrierFlags> {
        let old_state = self.states.get(&resource_id).copied().unwrap_or(ResourceState::Undefined);
        if old_state == new_state {
            return None;
        }
        let flags = Self::barrier_for_transition(old_state, new_state);
        self.states.insert(resource_id, new_state);
        flags
    }

    /// Transition and immediately issue the barrier.
    pub fn transition_and_barrier(
        &mut self,
        gl: &glow::Context,
        resource_id: u32,
        new_state: ResourceState,
    ) {
        if let Some(flags) = self.transition(resource_id, new_state) {
            flags.issue(gl);
        }
    }

    /// Determine which barrier flags are needed for a given transition.
    fn barrier_for_transition(
        from: ResourceState,
        to: ResourceState,
    ) -> Option<MemoryBarrierFlags> {
        // Only need barriers when transitioning from a write state
        match (from, to) {
            (ResourceState::ComputeWrite, ResourceState::ComputeRead) => {
                Some(MemoryBarrierFlags::SHADER_STORAGE)
            }
            (ResourceState::ComputeWrite, ResourceState::VertexRead) => {
                Some(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::VERTEX_ATTRIB)
            }
            (ResourceState::ComputeWrite, ResourceState::IndexRead) => {
                Some(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::ELEMENT_ARRAY)
            }
            (ResourceState::ComputeWrite, ResourceState::IndirectRead) => {
                Some(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::COMMAND)
            }
            (ResourceState::ComputeWrite, ResourceState::UniformRead) => {
                Some(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::UNIFORM)
            }
            (ResourceState::ComputeWrite, ResourceState::CpuAccess) => {
                Some(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::BUFFER_UPDATE)
            }
            (ResourceState::ComputeWrite, ResourceState::TransferSrc) => {
                Some(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::BUFFER_UPDATE)
            }
            (ResourceState::ComputeWrite, ResourceState::TransferDst) => {
                Some(MemoryBarrierFlags::SHADER_STORAGE | MemoryBarrierFlags::BUFFER_UPDATE)
            }
            (ResourceState::TransferDst, ResourceState::ComputeRead) => {
                Some(MemoryBarrierFlags::BUFFER_UPDATE | MemoryBarrierFlags::SHADER_STORAGE)
            }
            (ResourceState::TransferDst, ResourceState::VertexRead) => {
                Some(MemoryBarrierFlags::BUFFER_UPDATE | MemoryBarrierFlags::VERTEX_ATTRIB)
            }
            (ResourceState::CpuAccess, ResourceState::ComputeRead) => {
                Some(MemoryBarrierFlags::BUFFER_UPDATE | MemoryBarrierFlags::SHADER_STORAGE)
            }
            (ResourceState::CpuAccess, ResourceState::ComputeWrite) => {
                Some(MemoryBarrierFlags::BUFFER_UPDATE | MemoryBarrierFlags::SHADER_STORAGE)
            }
            // No barrier needed for same-state or read-to-read transitions
            _ if from == to => None,
            // Default: if transitioning from any write to any read, barrier everything
            (ResourceState::ComputeWrite, _) => Some(MemoryBarrierFlags::ALL),
            _ => None,
        }
    }

    /// Remove a resource from tracking.
    pub fn unregister(&mut self, resource_id: u32) {
        self.states.remove(&resource_id);
    }

    /// Number of tracked resources.
    pub fn tracked_count(&self) -> usize {
        self.states.len()
    }

    /// Reset all resources to undefined state.
    pub fn reset_all(&mut self) {
        for state in self.states.values_mut() {
            *state = ResourceState::Undefined;
        }
    }
}

impl Default for ResourceTransition {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AsyncComputeQueue
// ---------------------------------------------------------------------------

/// A queued compute job for async execution.
struct ComputeJob {
    /// Unique job ID.
    id: u64,
    /// Program cache key.
    program_key: u64,
    /// Dispatch dimensions.
    dimension: super::dispatch::DispatchDimension,
    /// Uniforms to set.
    uniforms: Vec<super::dispatch::UniformValue>,
    /// Barrier flags after dispatch.
    barrier: MemoryBarrierFlags,
    /// Fence inserted after dispatch.
    fence: Option<FenceSync>,
    /// Whether the job has been dispatched.
    dispatched: bool,
    /// Whether the job has completed.
    completed: bool,
}

/// Queue that manages overlapping compute and render work.
///
/// Submit compute jobs which are dispatched in order. Fences are inserted
/// after each job so that subsequent render work can wait for completion.
pub struct AsyncComputeQueue {
    jobs: VecDeque<ComputeJob>,
    next_id: u64,
    max_in_flight: usize,
}

impl AsyncComputeQueue {
    /// Create a new async queue.
    pub fn new(max_in_flight: usize) -> Self {
        Self {
            jobs: VecDeque::new(),
            next_id: 1,
            max_in_flight,
        }
    }

    /// Submit a compute job. Returns a job ID for tracking.
    pub fn submit(
        &mut self,
        program_key: u64,
        dimension: super::dispatch::DispatchDimension,
        uniforms: Vec<super::dispatch::UniformValue>,
        barrier: MemoryBarrierFlags,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.jobs.push_back(ComputeJob {
            id,
            program_key,
            dimension,
            uniforms,
            barrier,
            fence: None,
            dispatched: false,
            completed: false,
        });
        id
    }

    /// Flush: dispatch all pending jobs up to max_in_flight.
    pub fn flush(
        &mut self,
        gl: &glow::Context,
        cache: &super::dispatch::PipelineCache,
    ) {
        use glow::HasContext;
        let in_flight = self.jobs.iter().filter(|j| j.dispatched && !j.completed).count();
        let can_dispatch = self.max_in_flight.saturating_sub(in_flight);

        let mut dispatched_count = 0;
        for job in self.jobs.iter_mut() {
            if dispatched_count >= can_dispatch {
                break;
            }
            if job.dispatched {
                continue;
            }
            // Find program
            if let Some(program) = cache.cache.get(&job.program_key) {
                program.bind(gl);
                // Set uniforms
                for u in &job.uniforms {
                    match u {
                        super::dispatch::UniformValue::Int(name, v) => {
                            program.set_uniform_int(gl, name, *v)
                        }
                        super::dispatch::UniformValue::Uint(name, v) => {
                            program.set_uniform_uint(gl, name, *v)
                        }
                        super::dispatch::UniformValue::Float(name, v) => {
                            program.set_uniform_float(gl, name, *v)
                        }
                        super::dispatch::UniformValue::Vec2(name, x, y) => {
                            program.set_uniform_vec2(gl, name, *x, *y)
                        }
                        super::dispatch::UniformValue::Vec3(name, x, y, z) => {
                            program.set_uniform_vec3(gl, name, *x, *y, *z)
                        }
                        super::dispatch::UniformValue::Vec4(name, x, y, z, w) => {
                            program.set_uniform_vec4(gl, name, *x, *y, *z, *w)
                        }
                    }
                }
                let (gx, gy, gz) = job.dimension.as_tuple();
                unsafe {
                    gl.dispatch_compute(gx, gy, gz);
                    gl.memory_barrier(job.barrier.0);
                }
                let mut fence = FenceSync::new();
                fence.insert(gl);
                job.fence = Some(fence);
                job.dispatched = true;
                dispatched_count += 1;
            }
        }
    }

    /// Poll all in-flight jobs and mark completed ones.
    pub fn poll(&mut self, gl: &glow::Context) {
        for job in self.jobs.iter_mut() {
            if job.dispatched && !job.completed {
                if let Some(ref mut fence) = job.fence {
                    if fence.poll(gl) == FenceStatus::Signaled {
                        job.completed = true;
                    }
                }
            }
        }
    }

    /// Remove and return all completed job IDs.
    pub fn drain_completed(&mut self) -> Vec<u64> {
        let mut completed = Vec::new();
        while let Some(front) = self.jobs.front() {
            if front.completed {
                let job = self.jobs.pop_front().unwrap();
                completed.push(job.id);
            } else {
                break;
            }
        }
        completed
    }

    /// Check if a specific job is complete.
    pub fn is_complete(&self, job_id: u64) -> bool {
        self.jobs.iter().find(|j| j.id == job_id).map_or(true, |j| j.completed)
    }

    /// Wait for a specific job to complete.
    pub fn wait_for(
        &mut self,
        gl: &glow::Context,
        job_id: u64,
        timeout: Duration,
    ) -> bool {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            if job.completed {
                return true;
            }
            if let Some(ref mut fence) = job.fence {
                let status = fence.wait(gl, timeout);
                if status == FenceStatus::Signaled {
                    job.completed = true;
                    return true;
                }
            }
            false
        } else {
            true // Job not found = already completed and drained
        }
    }

    /// Number of pending (not yet dispatched) jobs.
    pub fn pending_count(&self) -> usize {
        self.jobs.iter().filter(|j| !j.dispatched).count()
    }

    /// Number of in-flight (dispatched but not completed) jobs.
    pub fn in_flight_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.dispatched && !j.completed).count()
    }

    /// Total number of jobs in the queue.
    pub fn total_count(&self) -> usize {
        self.jobs.len()
    }

    /// Destroy the queue, cleaning up fences.
    pub fn destroy(self, gl: &glow::Context) {
        for job in self.jobs {
            if let Some(fence) = job.fence {
                fence.destroy(gl);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FrameTimeline
// ---------------------------------------------------------------------------

/// Per-frame resource versioning for safe concurrent GPU/CPU access.
///
/// Maintains a ring of frame contexts. Each frame has its own fence so we
/// know when it's safe to reuse that frame's resources.
pub struct FrameTimeline {
    /// Frame contexts in a ring.
    frames: Vec<FrameContext>,
    /// Current frame index into the ring.
    current: usize,
    /// Total frames processed.
    total_frames: u64,
}

/// A single frame's synchronization context.
struct FrameContext {
    /// Fence signaling GPU completion of this frame.
    fence: FenceSync,
    /// Frame number.
    frame_number: u64,
    /// Resources allocated this frame (buffer IDs for cleanup).
    transient_resources: Vec<u32>,
    /// CPU-side timestamp for when this frame began.
    begin_time: Option<Instant>,
    /// CPU-side timestamp for when this frame's GPU work completed.
    complete_time: Option<Instant>,
}

impl FrameTimeline {
    /// Create a timeline with `ring_size` frames in flight.
    pub fn new(ring_size: usize) -> Self {
        let frames = (0..ring_size)
            .map(|_| FrameContext {
                fence: FenceSync::new(),
                frame_number: 0,
                transient_resources: Vec::new(),
                begin_time: None,
                complete_time: None,
            })
            .collect();
        Self {
            frames,
            current: 0,
            total_frames: 0,
        }
    }

    /// Begin a new frame. Waits for the oldest frame to complete if necessary.
    pub fn begin_frame(&mut self, gl: &glow::Context) {
        let ctx = &mut self.frames[self.current];

        // If this slot has an active fence, wait for it
        if ctx.fence.status() == FenceStatus::Unsignaled {
            ctx.fence.wait_forever(gl);
        }
        ctx.complete_time = Some(Instant::now());

        // Clean up transient resources
        ctx.transient_resources.clear();

        // Set up for new frame
        ctx.frame_number = self.total_frames;
        ctx.begin_time = Some(Instant::now());
    }

    /// End the current frame: insert a fence and advance to next slot.
    pub fn end_frame(&mut self, gl: &glow::Context) {
        self.frames[self.current].fence.insert(gl);
        self.current = (self.current + 1) % self.frames.len();
        self.total_frames += 1;
    }

    /// Register a transient resource for the current frame.
    pub fn register_transient(&mut self, resource_id: u32) {
        self.frames[self.current]
            .transient_resources
            .push(resource_id);
    }

    /// Current frame number (total frames started).
    pub fn current_frame_number(&self) -> u64 {
        self.total_frames
    }

    /// Ring size.
    pub fn ring_size(&self) -> usize {
        self.frames.len()
    }

    /// Current slot index in the ring.
    pub fn current_slot(&self) -> usize {
        self.current
    }

    /// Check if a specific frame has completed.
    pub fn is_frame_complete(&mut self, gl: &glow::Context, frame_number: u64) -> bool {
        for ctx in self.frames.iter_mut() {
            if ctx.frame_number == frame_number {
                if ctx.fence.is_signaled() {
                    return true;
                }
                return ctx.fence.poll(gl) == FenceStatus::Signaled;
            }
        }
        // Frame not in ring = already completed long ago
        true
    }

    /// Wait for all in-flight frames to complete.
    pub fn wait_all(&mut self, gl: &glow::Context) {
        for ctx in self.frames.iter_mut() {
            if ctx.fence.status() == FenceStatus::Unsignaled {
                ctx.fence.wait_forever(gl);
            }
        }
    }

    /// Get the average frame latency (time between begin and GPU completion).
    pub fn average_latency(&self) -> Option<Duration> {
        let mut total = Duration::ZERO;
        let mut count = 0u32;
        for ctx in &self.frames {
            if let (Some(begin), Some(complete)) = (ctx.begin_time, ctx.complete_time) {
                if complete > begin {
                    total += complete - begin;
                    count += 1;
                }
            }
        }
        if count > 0 {
            Some(total / count)
        } else {
            None
        }
    }

    /// Destroy the timeline, cleaning up all fences.
    pub fn destroy(self, gl: &glow::Context) {
        for ctx in self.frames {
            ctx.fence.destroy(gl);
        }
    }
}

// ---------------------------------------------------------------------------
// CpuFallback — software implementation for hardware without compute
// ---------------------------------------------------------------------------

/// CPU fallback that implements the same interface as GPU compute for
/// hardware without compute shader support.
///
/// Each "kernel" is implemented as a Rust function operating on CPU-side
/// arrays. This allows the engine to function on older hardware, albeit
/// at lower performance.
pub struct CpuFallback {
    /// Whether the fallback is active.
    active: bool,
    /// Performance tracking: last kernel execution time.
    last_execution_us: HashMap<String, u64>,
}

impl CpuFallback {
    /// Create a new CPU fallback.
    pub fn new() -> Self {
        Self {
            active: false,
            last_execution_us: HashMap::new(),
        }
    }

    /// Activate the fallback (use when compute shaders are unavailable).
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate (revert to GPU).
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Whether the fallback is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// CPU particle integration (matches particle_integrate kernel).
    pub fn particle_integrate(
        &mut self,
        positions: &mut [[f32; 4]],
        velocities: &mut [[f32; 4]],
        params: &super::kernels::ParticleIntegrateParams,
    ) {
        let start = Instant::now();
        let dt = params.dt;
        let gravity = params.gravity;
        let damping = params.damping;
        let max_age = params.max_age;
        let wind = params.wind;

        for i in 0..positions.len() {
            let age = positions[i][3] + dt;
            let lifetime = velocities[i][3];

            if age >= lifetime || age >= max_age {
                positions[i][3] = lifetime + 1.0;
                velocities[i][0] = 0.0;
                velocities[i][1] = 0.0;
                velocities[i][2] = 0.0;
                continue;
            }

            // Apply gravity
            velocities[i][0] += gravity[0] * dt;
            velocities[i][1] += gravity[1] * dt;
            velocities[i][2] += gravity[2] * dt;

            // Apply wind
            velocities[i][0] += wind[0] * dt;
            velocities[i][1] += wind[1] * dt;
            velocities[i][2] += wind[2] * dt;

            // Damping
            let d = damping.powf(dt);
            velocities[i][0] *= d;
            velocities[i][1] *= d;
            velocities[i][2] *= d;

            // Integrate position
            positions[i][0] += velocities[i][0] * dt;
            positions[i][1] += velocities[i][1] * dt;
            positions[i][2] += velocities[i][2] * dt;
            positions[i][3] = age;
        }

        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("particle_integrate".to_string(), elapsed);
    }

    /// CPU Lorenz attractor integration.
    pub fn lorenz_step(
        &mut self,
        points: &mut [[f32; 3]],
        sigma: f32,
        rho: f32,
        beta: f32,
        dt: f32,
    ) {
        let start = Instant::now();
        for p in points.iter_mut() {
            let dx = sigma * (p[1] - p[0]);
            let dy = p[0] * (rho - p[2]) - p[1];
            let dz = p[0] * p[1] - beta * p[2];

            // RK4
            let k1 = [dx, dy, dz];
            let p2 = [
                p[0] + 0.5 * dt * k1[0],
                p[1] + 0.5 * dt * k1[1],
                p[2] + 0.5 * dt * k1[2],
            ];
            let k2 = [
                sigma * (p2[1] - p2[0]),
                p2[0] * (rho - p2[2]) - p2[1],
                p2[0] * p2[1] - beta * p2[2],
            ];
            let p3 = [
                p[0] + 0.5 * dt * k2[0],
                p[1] + 0.5 * dt * k2[1],
                p[2] + 0.5 * dt * k2[2],
            ];
            let k3 = [
                sigma * (p3[1] - p3[0]),
                p3[0] * (rho - p3[2]) - p3[1],
                p3[0] * p3[1] - beta * p3[2],
            ];
            let p4 = [
                p[0] + dt * k3[0],
                p[1] + dt * k3[1],
                p[2] + dt * k3[2],
            ];
            let k4 = [
                sigma * (p4[1] - p4[0]),
                p4[0] * (rho - p4[2]) - p4[1],
                p4[0] * p4[1] - beta * p4[2],
            ];

            p[0] += (dt / 6.0) * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]);
            p[1] += (dt / 6.0) * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]);
            p[2] += (dt / 6.0) * (k1[2] + 2.0 * k2[2] + 2.0 * k3[2] + k4[2]);
        }
        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("lorenz_step".to_string(), elapsed);
    }

    /// CPU Mandelbrot iteration.
    pub fn mandelbrot_iterate(
        &mut self,
        z_re: &mut [f32],
        z_im: &mut [f32],
        c_re: &[f32],
        c_im: &[f32],
        iterations: &mut [u32],
        max_iter: u32,
    ) {
        let start = Instant::now();
        assert_eq!(z_re.len(), z_im.len());
        assert_eq!(z_re.len(), c_re.len());
        assert_eq!(z_re.len(), c_im.len());
        assert_eq!(z_re.len(), iterations.len());

        for i in 0..z_re.len() {
            if iterations[i] >= max_iter {
                continue;
            }
            let zr = z_re[i];
            let zi = z_im[i];
            if zr * zr + zi * zi >= 4.0 {
                continue;
            }
            z_re[i] = zr * zr - zi * zi + c_re[i];
            z_im[i] = 2.0 * zr * zi + c_im[i];
            iterations[i] += 1;
        }
        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("mandelbrot_iterate".to_string(), elapsed);
    }

    /// CPU Julia set iteration.
    pub fn julia_iterate(
        &mut self,
        z_re: &mut [f32],
        z_im: &mut [f32],
        c_re: f32,
        c_im: f32,
        iterations: &mut [u32],
        max_iter: u32,
    ) {
        let start = Instant::now();
        for i in 0..z_re.len() {
            if iterations[i] >= max_iter {
                continue;
            }
            let zr = z_re[i];
            let zi = z_im[i];
            if zr * zr + zi * zi >= 4.0 {
                continue;
            }
            z_re[i] = zr * zr - zi * zi + c_re;
            z_im[i] = 2.0 * zr * zi + c_im;
            iterations[i] += 1;
        }
        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("julia_iterate".to_string(), elapsed);
    }

    /// CPU prefix sum (exclusive scan).
    pub fn prefix_sum_exclusive(&mut self, data: &mut [u32]) {
        let start = Instant::now();
        if data.is_empty() {
            return;
        }
        let mut sum = 0u32;
        for val in data.iter_mut() {
            let old = *val;
            *val = sum;
            sum += old;
        }
        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("prefix_sum".to_string(), elapsed);
    }

    /// CPU prefix sum (inclusive scan).
    pub fn prefix_sum_inclusive(&mut self, data: &mut [u32]) {
        let start = Instant::now();
        if data.is_empty() {
            return;
        }
        for i in 1..data.len() {
            data[i] += data[i - 1];
        }
        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("prefix_sum_inclusive".to_string(), elapsed);
    }

    /// CPU radix sort (key-value pairs, ascending).
    pub fn radix_sort(&mut self, keys: &mut [u32], values: &mut [u32]) {
        let start = Instant::now();
        assert_eq!(keys.len(), values.len());
        let n = keys.len();
        if n == 0 {
            return;
        }

        let mut keys_tmp = vec![0u32; n];
        let mut vals_tmp = vec![0u32; n];

        let radix = 256usize;
        let mut counts = vec![0usize; radix];

        for bit_offset in (0..32).step_by(8) {
            // Count
            for c in counts.iter_mut() {
                *c = 0;
            }
            for &k in keys.iter() {
                let digit = ((k >> bit_offset) & 0xFF) as usize;
                counts[digit] += 1;
            }
            // Prefix sum on counts
            let mut total = 0;
            for c in counts.iter_mut() {
                let old = *c;
                *c = total;
                total += old;
            }
            // Scatter
            for i in 0..n {
                let digit = ((keys[i] >> bit_offset) & 0xFF) as usize;
                let dest = counts[digit];
                keys_tmp[dest] = keys[i];
                vals_tmp[dest] = values[i];
                counts[digit] += 1;
            }
            // Swap
            keys.copy_from_slice(&keys_tmp);
            values.copy_from_slice(&vals_tmp);
        }

        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("radix_sort".to_string(), elapsed);
    }

    /// CPU frustum culling.
    pub fn frustum_cull(
        &mut self,
        positions: &[[f32; 3]],
        radii: &[f32],
        planes: &[[f32; 4]; 6],
    ) -> Vec<usize> {
        let start = Instant::now();
        let mut visible = Vec::new();

        for (i, (pos, &radius)) in positions.iter().zip(radii).enumerate() {
            let mut inside = true;
            for plane in planes {
                let dist =
                    plane[0] * pos[0] + plane[1] * pos[1] + plane[2] * pos[2] + plane[3];
                if dist < -radius {
                    inside = false;
                    break;
                }
            }
            if inside {
                visible.push(i);
            }
        }

        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("frustum_cull".to_string(), elapsed);
        visible
    }

    /// CPU skinning: transform vertices by bone matrices.
    pub fn skin_vertices(
        &mut self,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        bone_indices: &[[u32; 4]],
        bone_weights: &[[f32; 4]],
        bone_matrices: &[[f32; 16]],
        inv_bind_matrices: &[[f32; 16]],
        out_positions: &mut [[f32; 3]],
        out_normals: &mut [[f32; 3]],
    ) {
        let start = Instant::now();

        for i in 0..positions.len() {
            let pos = positions[i];
            let norm = normals[i];
            let indices = bone_indices[i];
            let weights = bone_weights[i];

            let mut skinned_pos = [0.0f32; 3];
            let mut skinned_norm = [0.0f32; 3];

            for j in 0..4 {
                let w = weights[j];
                if w <= 0.0 {
                    continue;
                }
                let bi = indices[j] as usize;
                if bi >= bone_matrices.len() {
                    continue;
                }
                // Compute final_matrix = bone * inv_bind
                let bone = &bone_matrices[bi];
                let inv = &inv_bind_matrices[bi];
                let mat = mat4_mul(bone, inv);

                // Transform position
                let tp = mat4_transform_point(&mat, &pos);
                skinned_pos[0] += tp[0] * w;
                skinned_pos[1] += tp[1] * w;
                skinned_pos[2] += tp[2] * w;

                // Transform normal (upper-left 3x3)
                let tn = mat4_transform_normal(&mat, &norm);
                skinned_norm[0] += tn[0] * w;
                skinned_norm[1] += tn[1] * w;
                skinned_norm[2] += tn[2] * w;
            }

            // Normalize the normal
            let len = (skinned_norm[0] * skinned_norm[0]
                + skinned_norm[1] * skinned_norm[1]
                + skinned_norm[2] * skinned_norm[2])
                .sqrt();
            if len > 1e-6 {
                skinned_norm[0] /= len;
                skinned_norm[1] /= len;
                skinned_norm[2] /= len;
            }

            out_positions[i] = skinned_pos;
            out_normals[i] = skinned_norm;
        }

        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("skinning".to_string(), elapsed);
    }

    /// CPU fluid diffusion (Jacobi iteration).
    pub fn fluid_diffuse(
        &mut self,
        grid: &mut [f32],
        scratch: &mut [f32],
        width: usize,
        height: usize,
        diffusion_rate: f32,
        dt: f32,
        iterations: usize,
    ) {
        let start = Instant::now();
        let dx = 1.0f32;
        let alpha = diffusion_rate * dt / (dx * dx);
        let r_beta = 1.0 / (1.0 + 4.0 * alpha);

        for _ in 0..iterations {
            for y in 0..height {
                for x in 0..width {
                    let idx = y * width + x;
                    let left = if x > 0 { grid[idx - 1] } else { grid[idx] };
                    let right = if x + 1 < width { grid[idx + 1] } else { grid[idx] };
                    let down = if y > 0 { grid[idx - width] } else { grid[idx] };
                    let up = if y + 1 < height { grid[idx + width] } else { grid[idx] };
                    scratch[idx] = (grid[idx] + alpha * (left + right + down + up)) * r_beta;
                }
            }
            grid.copy_from_slice(scratch);
        }

        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("fluid_diffuse".to_string(), elapsed);
    }

    /// CPU histogram equalization.
    pub fn histogram_equalize(
        &mut self,
        data: &mut [f32],
        bin_count: usize,
        min_val: f32,
        max_val: f32,
    ) {
        let start = Instant::now();
        let range = max_val - min_val;
        if range <= 0.0 || data.is_empty() {
            return;
        }

        // Build histogram
        let mut histogram = vec![0u32; bin_count];
        for &v in data.iter() {
            let norm = ((v - min_val) / range).clamp(0.0, 1.0);
            let bin = ((norm * (bin_count - 1) as f32) as usize).min(bin_count - 1);
            histogram[bin] += 1;
        }

        // Build CDF
        let mut cdf = vec![0.0f32; bin_count];
        let mut running = 0u32;
        for i in 0..bin_count {
            running += histogram[i];
            cdf[i] = running as f32 / data.len() as f32;
        }

        // Apply equalization
        for v in data.iter_mut() {
            let norm = ((*v - min_val) / range).clamp(0.0, 1.0);
            let bin = ((norm * (bin_count - 1) as f32) as usize).min(bin_count - 1);
            *v = cdf[bin] * range + min_val;
        }

        let elapsed = start.elapsed().as_micros() as u64;
        self.last_execution_us
            .insert("histogram_equalize".to_string(), elapsed);
    }

    /// Get the last execution time for a named kernel, in microseconds.
    pub fn last_execution_us(&self, name: &str) -> Option<u64> {
        self.last_execution_us.get(name).copied()
    }

    /// Summary of all kernel execution times.
    pub fn summary(&self) -> String {
        let mut s = String::from("=== CPU Fallback Timings ===\n");
        let mut names: Vec<&str> = self.last_execution_us.keys().map(|s| s.as_str()).collect();
        names.sort();
        for name in names {
            if let Some(us) = self.last_execution_us.get(name) {
                s.push_str(&format!("  {}: {} us\n", name, us));
            }
        }
        s
    }
}

impl Default for CpuFallback {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Matrix math helpers for CPU fallback
// ---------------------------------------------------------------------------

/// Multiply two column-major 4x4 matrices.
fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut result = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            result[col * 4 + row] = sum;
        }
    }
    result
}

/// Transform a point by a column-major 4x4 matrix (w=1).
fn mat4_transform_point(m: &[f32; 16], p: &[f32; 3]) -> [f32; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

/// Transform a normal by the upper-left 3x3 of a column-major 4x4 matrix.
fn mat4_transform_normal(m: &[f32; 16], n: &[f32; 3]) -> [f32; 3] {
    [
        m[0] * n[0] + m[4] * n[1] + m[8] * n[2],
        m[1] * n[0] + m[5] * n[1] + m[9] * n[2],
        m[2] * n[0] + m[6] * n[1] + m[10] * n[2],
    ]
}

// ---------------------------------------------------------------------------
// ComputeCapabilities — detect hardware support
// ---------------------------------------------------------------------------

/// Detected GPU compute capabilities.
#[derive(Debug, Clone)]
pub struct ComputeCapabilities {
    pub has_compute: bool,
    pub max_work_group_invocations: u32,
    pub max_work_group_size: [u32; 3],
    pub max_work_group_count: [u32; 3],
    pub max_shared_memory: u32,
    pub max_ssbo_bindings: u32,
    pub max_atomic_counter_bindings: u32,
    pub gl_version_major: u32,
    pub gl_version_minor: u32,
}

impl ComputeCapabilities {
    /// Query capabilities from the GL context.
    pub fn query(gl: &glow::Context) -> Self {
        use glow::HasContext;
        unsafe {
            let major = gl.get_parameter_i32(glow::MAJOR_VERSION) as u32;
            let minor = gl.get_parameter_i32(glow::MINOR_VERSION) as u32;
            let has_compute = major > 4 || (major == 4 && minor >= 3);

            if !has_compute {
                return Self {
                    has_compute: false,
                    max_work_group_invocations: 0,
                    max_work_group_size: [0; 3],
                    max_work_group_count: [0; 3],
                    max_shared_memory: 0,
                    max_ssbo_bindings: 0,
                    max_atomic_counter_bindings: 0,
                    gl_version_major: major,
                    gl_version_minor: minor,
                };
            }

            let max_invocations = gl.get_parameter_i32(0x90EB) as u32;
            let max_size = [
                gl.get_parameter_indexed_i32(0x91BE, 0) as u32,
                gl.get_parameter_indexed_i32(0x91BE, 1) as u32,
                gl.get_parameter_indexed_i32(0x91BE, 2) as u32,
            ];
            let max_count = [
                gl.get_parameter_indexed_i32(0x91BF, 0) as u32,
                gl.get_parameter_indexed_i32(0x91BF, 1) as u32,
                gl.get_parameter_indexed_i32(0x91BF, 2) as u32,
            ];
            let max_shared = gl.get_parameter_i32(0x8262) as u32;
            let max_ssbo = gl.get_parameter_i32(0x90DC) as u32; // GL_MAX_SHADER_STORAGE_BUFFER_BINDINGS
            let max_atomic = gl.get_parameter_i32(0x92D1) as u32; // GL_MAX_ATOMIC_COUNTER_BUFFER_BINDINGS

            Self {
                has_compute,
                max_work_group_invocations: max_invocations,
                max_work_group_size: max_size,
                max_work_group_count: max_count,
                max_shared_memory: max_shared,
                max_ssbo_bindings: max_ssbo,
                max_atomic_counter_bindings: max_atomic,
                gl_version_major: major,
                gl_version_minor: minor,
            }
        }
    }

    /// Check if a specific workgroup size fits within limits.
    pub fn validate_workgroup(&self, size: &super::dispatch::WorkgroupSize) -> bool {
        size.x <= self.max_work_group_size[0]
            && size.y <= self.max_work_group_size[1]
            && size.z <= self.max_work_group_size[2]
            && size.total_invocations() <= self.max_work_group_invocations
    }

    /// Summary string.
    pub fn summary(&self) -> String {
        if !self.has_compute {
            return format!(
                "GL {}.{}: NO compute support (requires 4.3+)",
                self.gl_version_major, self.gl_version_minor
            );
        }
        format!(
            "GL {}.{}: compute OK, max_invocations={}, max_size=[{},{},{}], max_shared={}KB, ssbo_bindings={}, atomic_bindings={}",
            self.gl_version_major, self.gl_version_minor,
            self.max_work_group_invocations,
            self.max_work_group_size[0], self.max_work_group_size[1], self.max_work_group_size[2],
            self.max_shared_memory / 1024,
            self.max_ssbo_bindings,
            self.max_atomic_counter_bindings,
        )
    }
}
