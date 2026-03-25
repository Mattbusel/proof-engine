//! Graph execution engine: walks sorted passes, manages barriers, GPU timing,
//! triple-buffered frame-in-flight management, async compute overlap detection,
//! execution statistics, and hot-reload from config.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use crate::rendergraph::graph::{
    DependencyKind, GraphConfig, PassType, QueueAffinity, RenderGraph,
};
use crate::rendergraph::resources::{
    MemoryBudget, ResourceDescriptor, ResourceHandle, ResourceLifetime, ResourcePool, TextureFormat,
};

// ---------------------------------------------------------------------------
// Barrier types
// ---------------------------------------------------------------------------

/// The kind of GPU barrier inserted between passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierKind {
    /// Render target finished writing, will be read as texture.
    RenderToShaderRead,
    /// Compute shader finished writing, will be read by render pass.
    ComputeToRender,
    /// Render pass finished, another render pass will write the same target.
    RenderToRender,
    /// Compute finished writing, another compute will read.
    ComputeToCompute,
    /// Transfer finished, resource will be read.
    TransferToRead,
    /// Generic full pipeline barrier.
    FullPipeline,
}

/// A barrier that must be issued between two passes.
#[derive(Debug, Clone)]
pub struct PassBarrier {
    pub before_pass: String,
    pub after_pass: String,
    pub resource_name: String,
    pub kind: BarrierKind,
}

impl fmt::Display for PassBarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Barrier({:?}): {} -> {} [{}]",
            self.kind, self.before_pass, self.after_pass, self.resource_name
        )
    }
}

// ---------------------------------------------------------------------------
// GPU timing queries
// ---------------------------------------------------------------------------

/// Simulated GPU timing query result for a single pass.
#[derive(Debug, Clone)]
pub struct PassTimingQuery {
    pub pass_name: String,
    pub cpu_time: Duration,
    pub gpu_time_estimate: Duration,
    pub start_offset: Duration,
}

impl PassTimingQuery {
    pub fn cpu_ms(&self) -> f64 {
        self.cpu_time.as_secs_f64() * 1000.0
    }

    pub fn gpu_ms(&self) -> f64 {
        self.gpu_time_estimate.as_secs_f64() * 1000.0
    }
}

// ---------------------------------------------------------------------------
// Pass context
// ---------------------------------------------------------------------------

/// Context provided to each pass during execution. Gives access to input and
/// output resources, resolution info, and pass metadata.
#[derive(Debug)]
pub struct PassContext {
    pub pass_name: String,
    pub pass_index: usize,
    pub frame_index: u64,
    pub backbuffer_width: u32,
    pub backbuffer_height: u32,
    pub delta_time: f32,
    /// Input resource handles and names.
    pub inputs: Vec<(ResourceHandle, String)>,
    /// Output resource handles and names.
    pub outputs: Vec<(ResourceHandle, String)>,
    /// Effective resolution for this pass (after applying resolution scale).
    pub render_width: u32,
    pub render_height: u32,
}

impl PassContext {
    /// Find an input resource by name.
    pub fn input(&self, name: &str) -> Option<ResourceHandle> {
        self.inputs
            .iter()
            .find(|(_, n)| n == name)
            .map(|(h, _)| *h)
    }

    /// Find an output resource by name.
    pub fn output(&self, name: &str) -> Option<ResourceHandle> {
        self.outputs
            .iter()
            .find(|(_, n)| n == name)
            .map(|(h, _)| *h)
    }
}

// ---------------------------------------------------------------------------
// Frame timeline (triple-buffered)
// ---------------------------------------------------------------------------

/// Manages triple-buffered frame-in-flight state.
pub struct FrameTimeline {
    /// Maximum frames in flight.
    max_frames_in_flight: usize,
    /// Ring of frame states.
    frames: Vec<FrameState>,
    /// Index of the current frame being recorded.
    current_index: usize,
    /// Global frame counter.
    frame_counter: u64,
}

/// State of a single frame in the pipeline.
#[derive(Debug, Clone)]
pub struct FrameState {
    pub frame_index: u64,
    pub status: FrameStatus,
    pub submit_time: Option<Instant>,
    pub complete_time: Option<Instant>,
    pub pass_timings: Vec<PassTimingQuery>,
    pub barriers: Vec<PassBarrier>,
    pub resource_allocations: usize,
    pub total_cpu_time: Duration,
}

impl FrameState {
    fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            status: FrameStatus::Available,
            submit_time: None,
            complete_time: None,
            pass_timings: Vec::new(),
            barriers: Vec::new(),
            resource_allocations: 0,
            total_cpu_time: Duration::ZERO,
        }
    }

    fn reset(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.status = FrameStatus::Recording;
        self.submit_time = None;
        self.complete_time = None;
        self.pass_timings.clear();
        self.barriers.clear();
        self.resource_allocations = 0;
        self.total_cpu_time = Duration::ZERO;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameStatus {
    Available,
    Recording,
    Submitted,
    Complete,
}

impl FrameTimeline {
    pub fn new(max_frames_in_flight: usize) -> Self {
        let frames = (0..max_frames_in_flight)
            .map(|_| FrameState::new(0))
            .collect();
        Self {
            max_frames_in_flight,
            frames,
            current_index: 0,
            frame_counter: 0,
        }
    }

    pub fn triple_buffered() -> Self {
        Self::new(3)
    }

    /// Begin recording a new frame. Returns the frame index.
    pub fn begin_frame(&mut self) -> u64 {
        self.frame_counter += 1;
        let idx = self.current_index;
        self.frames[idx].reset(self.frame_counter);
        self.frame_counter
    }

    /// Submit the current frame.
    pub fn submit_frame(&mut self) {
        let idx = self.current_index;
        self.frames[idx].status = FrameStatus::Submitted;
        self.frames[idx].submit_time = Some(Instant::now());
        self.current_index = (self.current_index + 1) % self.max_frames_in_flight;
    }

    /// Mark a frame as complete (GPU finished).
    pub fn complete_frame(&mut self, frame_index: u64) {
        for f in &mut self.frames {
            if f.frame_index == frame_index && f.status == FrameStatus::Submitted {
                f.status = FrameStatus::Complete;
                f.complete_time = Some(Instant::now());
                break;
            }
        }
    }

    /// Get the current recording frame (mutable).
    pub fn current_frame_mut(&mut self) -> &mut FrameState {
        &mut self.frames[self.current_index]
    }

    /// Get the current recording frame.
    pub fn current_frame(&self) -> &FrameState {
        &self.frames[self.current_index]
    }

    /// Get a completed frame by index.
    pub fn completed_frame(&self, frame_index: u64) -> Option<&FrameState> {
        self.frames
            .iter()
            .find(|f| f.frame_index == frame_index && f.status == FrameStatus::Complete)
    }

    /// Number of frames currently in flight (submitted but not completed).
    pub fn frames_in_flight(&self) -> usize {
        self.frames
            .iter()
            .filter(|f| f.status == FrameStatus::Submitted)
            .count()
    }

    /// Wait until a frame slot is available.
    pub fn wait_for_available(&self) -> bool {
        self.frames
            .iter()
            .any(|f| f.status == FrameStatus::Available || f.status == FrameStatus::Complete)
    }

    pub fn max_frames_in_flight(&self) -> usize {
        self.max_frames_in_flight
    }

    pub fn frame_counter(&self) -> u64 {
        self.frame_counter
    }
}

// ---------------------------------------------------------------------------
// Async compute scheduling
// ---------------------------------------------------------------------------

/// Identifies passes that can run on the async compute queue, overlapping
/// with graphics work.
#[derive(Debug, Clone)]
pub struct AsyncComputeSchedule {
    /// Passes that run on the graphics queue (in order).
    pub graphics_passes: Vec<String>,
    /// Passes that run on the async compute queue (in order).
    pub compute_passes: Vec<String>,
    /// Sync points: (graphics_pass, compute_pass) where compute must finish
    /// before graphics can continue.
    pub sync_points: Vec<(String, String)>,
}

impl AsyncComputeSchedule {
    /// Analyze a graph and partition passes into graphics and async compute queues.
    pub fn from_graph(graph: &mut RenderGraph) -> Result<Self, Vec<String>> {
        let sorted = graph.topological_sort()?;
        let mut graphics = Vec::new();
        let mut compute = Vec::new();
        let mut sync_points = Vec::new();

        for name in &sorted {
            let pass = graph.get_pass(name).unwrap();
            if pass.is_async_compute_candidate() {
                compute.push(name.clone());
            } else {
                graphics.push(name.clone());
            }
        }

        // Determine sync points: if a graphics pass reads a resource written
        // by a compute pass, we need a sync point.
        let edges = graph.edges().to_vec();
        for edge in &edges {
            let from_is_compute = compute.contains(&edge.from_pass);
            let to_is_graphics = graphics.contains(&edge.to_pass);
            if from_is_compute && to_is_graphics {
                sync_points.push((edge.to_pass.clone(), edge.from_pass.clone()));
            }
        }

        Ok(Self {
            graphics_passes: graphics,
            compute_passes: compute,
            sync_points,
        })
    }

    /// Returns the percentage of passes that can run asynchronously.
    pub fn async_ratio(&self) -> f32 {
        let total = self.graphics_passes.len() + self.compute_passes.len();
        if total == 0 {
            return 0.0;
        }
        self.compute_passes.len() as f32 / total as f32
    }
}

// ---------------------------------------------------------------------------
// Execution statistics
// ---------------------------------------------------------------------------

/// Per-frame execution statistics.
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub frame_index: u64,
    pub total_cpu_time: Duration,
    pub pass_times: Vec<(String, Duration)>,
    pub barrier_count: usize,
    pub resource_allocation_count: usize,
    pub resource_reuse_count: usize,
    pub async_compute_passes: usize,
    pub skipped_passes: usize,
    pub active_passes: usize,
    pub memory_budget: Option<MemoryBudget>,
}

impl ExecutionStats {
    fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            total_cpu_time: Duration::ZERO,
            pass_times: Vec::new(),
            barrier_count: 0,
            resource_allocation_count: 0,
            resource_reuse_count: 0,
            async_compute_passes: 0,
            skipped_passes: 0,
            active_passes: 0,
            memory_budget: None,
        }
    }

    pub fn total_ms(&self) -> f64 {
        self.total_cpu_time.as_secs_f64() * 1000.0
    }

    /// Slowest pass name and its time.
    pub fn slowest_pass(&self) -> Option<(&str, Duration)> {
        self.pass_times
            .iter()
            .max_by_key(|(_, d)| *d)
            .map(|(n, d)| (n.as_str(), *d))
    }
}

impl fmt::Display for ExecutionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Frame {}: {:.2}ms, {} passes ({} skipped), {} barriers, {} allocs",
            self.frame_index,
            self.total_ms(),
            self.active_passes,
            self.skipped_passes,
            self.barrier_count,
            self.resource_allocation_count,
        )
    }
}

// ---------------------------------------------------------------------------
// Pass executor callback
// ---------------------------------------------------------------------------

/// Trait for pass execution callbacks. Each built-in pass implements this.
pub trait PassExecutor {
    /// Execute the pass, given its context.
    fn execute(&self, ctx: &PassContext);

    /// Name of the pass (for debugging).
    fn name(&self) -> &str;
}

/// A boxed pass executor.
pub type BoxedPassExecutor = Box<dyn PassExecutor>;

/// A simple closure-based pass executor.
pub struct FnPassExecutor {
    name: String,
    func: Box<dyn Fn(&PassContext)>,
}

impl FnPassExecutor {
    pub fn new(name: &str, func: impl Fn(&PassContext) + 'static) -> Self {
        Self {
            name: name.to_string(),
            func: Box::new(func),
        }
    }
}

impl PassExecutor for FnPassExecutor {
    fn execute(&self, ctx: &PassContext) {
        (self.func)(ctx);
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// Graph executor
// ---------------------------------------------------------------------------

/// The main execution engine. Walks sorted passes, inserts barriers, tracks
/// timing, and manages resources.
pub struct GraphExecutor {
    /// Resource pool for automatic allocation.
    pub resource_pool: ResourcePool,
    /// Frame timeline for triple-buffered frame management.
    pub timeline: FrameTimeline,
    /// Registered pass executors.
    executors: HashMap<String, BoxedPassExecutor>,
    /// Backbuffer dimensions.
    backbuffer_width: u32,
    backbuffer_height: u32,
    /// Delta time for current frame.
    delta_time: f32,
    /// Accumulated statistics for the last N frames.
    stats_history: Vec<ExecutionStats>,
    /// Maximum number of stats frames to keep.
    max_stats_history: usize,
    /// Current graph config (for hot-reload).
    current_config: Option<GraphConfig>,
    /// Config file path (for hot-reload watching).
    config_path: Option<String>,
    /// Last modification timestamp for config file.
    last_config_modified: Option<Instant>,
    /// Whether to collect detailed per-pass timing.
    enable_timing: bool,
}

impl GraphExecutor {
    pub fn new(backbuffer_width: u32, backbuffer_height: u32) -> Self {
        Self {
            resource_pool: ResourcePool::new(),
            timeline: FrameTimeline::triple_buffered(),
            executors: HashMap::new(),
            backbuffer_width,
            backbuffer_height,
            delta_time: 0.016,
            stats_history: Vec::new(),
            max_stats_history: 120,
            current_config: None,
            config_path: None,
            last_config_modified: None,
            enable_timing: true,
        }
    }

    pub fn with_timing(mut self, enable: bool) -> Self {
        self.enable_timing = enable;
        self
    }

    pub fn with_max_stats_history(mut self, n: usize) -> Self {
        self.max_stats_history = n;
        self
    }

    /// Register a pass executor.
    pub fn register_executor(&mut self, name: &str, executor: BoxedPassExecutor) {
        self.executors.insert(name.to_string(), executor);
    }

    /// Register a closure-based executor.
    pub fn register_fn(
        &mut self,
        name: &str,
        func: impl Fn(&PassContext) + 'static,
    ) {
        self.executors.insert(
            name.to_string(),
            Box::new(FnPassExecutor::new(name, func)),
        );
    }

    /// Set backbuffer dimensions (e.g., on window resize).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.backbuffer_width = width;
        self.backbuffer_height = height;
    }

    /// Set delta time for the current frame.
    pub fn set_delta_time(&mut self, dt: f32) {
        self.delta_time = dt;
    }

    // -- Barrier insertion ------------------------------------------------

    /// Compute barriers needed between passes based on their resource
    /// dependencies and queue types.
    fn compute_barriers(&self, graph: &RenderGraph, sorted: &[String]) -> Vec<PassBarrier> {
        let mut barriers = Vec::new();
        let edges = graph.edges();

        for edge in edges {
            // Find the indices of from/to in sorted order
            let from_idx = sorted.iter().position(|n| n == &edge.from_pass);
            let to_idx = sorted.iter().position(|n| n == &edge.to_pass);
            if from_idx.is_none() || to_idx.is_none() {
                continue;
            }

            let from_pass = graph.get_pass(&edge.from_pass);
            let to_pass = graph.get_pass(&edge.to_pass);
            if from_pass.is_none() || to_pass.is_none() {
                continue;
            }
            let from_pass = from_pass.unwrap();
            let to_pass = to_pass.unwrap();

            let kind = match (from_pass.pass_type, to_pass.pass_type, edge.kind) {
                (PassType::Compute, PassType::Graphics, _) => BarrierKind::ComputeToRender,
                (PassType::Compute, PassType::Compute, _) => BarrierKind::ComputeToCompute,
                (PassType::Graphics, PassType::Graphics, DependencyKind::ReadAfterWrite) => {
                    BarrierKind::RenderToShaderRead
                }
                (PassType::Graphics, PassType::Graphics, _) => BarrierKind::RenderToRender,
                (PassType::Transfer, _, _) => BarrierKind::TransferToRead,
                _ => BarrierKind::FullPipeline,
            };

            barriers.push(PassBarrier {
                before_pass: edge.from_pass.clone(),
                after_pass: edge.to_pass.clone(),
                resource_name: edge.resource.clone(),
                kind,
            });
        }

        barriers
    }

    // -- Main execution ---------------------------------------------------

    /// Execute one frame of the render graph.
    pub fn execute_frame(&mut self, graph: &mut RenderGraph) -> Result<ExecutionStats, String> {
        let frame_start = Instant::now();

        // Begin frame
        let frame_index = self.timeline.begin_frame();
        self.resource_pool.begin_frame();

        let mut stats = ExecutionStats::new(frame_index);

        // Topological sort
        let sorted = graph
            .topological_sort()
            .map_err(|cycle| format!("Cycle detected: {:?}", cycle))?;

        // Filter to active passes
        let active_passes = graph.active_passes().unwrap_or_default();
        let skipped = sorted.len() - active_passes.len();
        stats.skipped_passes = skipped;
        stats.active_passes = active_passes.len();

        // Compute barriers
        let barriers = self.compute_barriers(graph, &active_passes);
        stats.barrier_count = barriers.len();

        // Store barriers in frame state
        self.timeline.current_frame_mut().barriers = barriers.clone();

        // Acquire resources
        let mut allocated = 0usize;
        for entry in graph.resource_table.entries() {
            let _handle = self.resource_pool.acquire(
                entry.descriptor.clone(),
                entry.lifetime,
                self.backbuffer_width,
                self.backbuffer_height,
            );
            allocated += 1;
        }
        stats.resource_allocation_count = allocated;

        // Record resource read/write for lifetime tracking
        for (pass_idx, pass_name) in active_passes.iter().enumerate() {
            if let Some(pass) = graph.get_pass(pass_name) {
                for &h in &pass.outputs {
                    self.resource_pool.record_write(h, pass_idx, pass_name);
                }
                for &h in &pass.inputs {
                    self.resource_pool.record_read(h, pass_idx, pass_name);
                }
            }
        }

        // Execute each pass
        let mut barrier_idx = 0;
        for (pass_idx, pass_name) in active_passes.iter().enumerate() {
            // Issue barriers that precede this pass
            while barrier_idx < barriers.len() && barriers[barrier_idx].after_pass == *pass_name {
                // In a real implementation, this would call the GPU API
                barrier_idx += 1;
            }

            let pass_start = Instant::now();

            let pass = graph.get_pass(pass_name).unwrap();

            // Build pass context
            let (rw, rh) = {
                let w = (self.backbuffer_width as f32 * pass.resolution.width_scale) as u32;
                let h = (self.backbuffer_height as f32 * pass.resolution.height_scale) as u32;
                (w.max(1), h.max(1))
            };

            let ctx = PassContext {
                pass_name: pass_name.clone(),
                pass_index: pass_idx,
                frame_index,
                backbuffer_width: self.backbuffer_width,
                backbuffer_height: self.backbuffer_height,
                delta_time: self.delta_time,
                inputs: pass
                    .inputs
                    .iter()
                    .zip(pass.input_names.iter())
                    .map(|(&h, n)| (h, n.clone()))
                    .collect(),
                outputs: pass
                    .outputs
                    .iter()
                    .zip(pass.output_names.iter())
                    .map(|(&h, n)| (h, n.clone()))
                    .collect(),
                render_width: rw,
                render_height: rh,
            };

            // Execute
            if let Some(executor) = self.executors.get(pass_name) {
                executor.execute(&ctx);
            }

            let pass_elapsed = pass_start.elapsed();
            if self.enable_timing {
                stats.pass_times.push((pass_name.clone(), pass_elapsed));

                self.timeline
                    .current_frame_mut()
                    .pass_timings
                    .push(PassTimingQuery {
                        pass_name: pass_name.clone(),
                        cpu_time: pass_elapsed,
                        gpu_time_estimate: pass_elapsed, // simulated
                        start_offset: pass_start.duration_since(frame_start),
                    });
            }

            // Count async compute passes
            if pass.is_async_compute_candidate() {
                stats.async_compute_passes += 1;
            }
        }

        // Compute aliasing
        self.resource_pool
            .compute_aliasing(active_passes.len());

        // Memory budget
        let budget = self
            .resource_pool
            .estimate_memory_budget(self.backbuffer_width, self.backbuffer_height);
        stats.memory_budget = Some(budget);

        // End frame
        let pool_stats = self.resource_pool.end_frame();
        stats.resource_allocation_count = pool_stats.active_resources;

        let total_elapsed = frame_start.elapsed();
        stats.total_cpu_time = total_elapsed;
        self.timeline.current_frame_mut().total_cpu_time = total_elapsed;
        self.timeline.current_frame_mut().resource_allocations = pool_stats.active_resources;

        // Submit frame
        self.timeline.submit_frame();

        // Store stats
        self.stats_history.push(stats.clone());
        if self.stats_history.len() > self.max_stats_history {
            self.stats_history.remove(0);
        }

        Ok(stats)
    }

    // -- Hot-reload -------------------------------------------------------

    /// Set a graph config for hot-reload support.
    pub fn set_config(&mut self, config: GraphConfig) {
        self.current_config = Some(config);
        self.last_config_modified = Some(Instant::now());
    }

    /// Set the path to watch for config changes.
    pub fn set_config_path(&mut self, path: &str) {
        self.config_path = Some(path.to_string());
    }

    /// Rebuild the graph from the current config (hot-reload).
    pub fn rebuild_from_config(&mut self) -> Option<RenderGraph> {
        self.current_config.as_ref().map(|config| {
            self.last_config_modified = Some(Instant::now());
            config.build()
        })
    }

    /// Check if config has been modified and rebuild if needed.
    /// In a real implementation, this would watch the filesystem.
    pub fn check_hot_reload(&mut self) -> Option<RenderGraph> {
        if let Some(ref _path) = self.config_path {
            // In production: check file modification time against last_config_modified.
            // For now, this is a no-op; call rebuild_from_config() explicitly.
        }
        None
    }

    // -- Statistics access -------------------------------------------------

    pub fn stats_history(&self) -> &[ExecutionStats] {
        &self.stats_history
    }

    pub fn last_stats(&self) -> Option<&ExecutionStats> {
        self.stats_history.last()
    }

    /// Average frame time over the last N frames.
    pub fn average_frame_time(&self, n: usize) -> Duration {
        let count = self.stats_history.len().min(n);
        if count == 0 {
            return Duration::ZERO;
        }
        let total: Duration = self.stats_history[self.stats_history.len() - count..]
            .iter()
            .map(|s| s.total_cpu_time)
            .sum();
        total / count as u32
    }

    /// Average barrier count over the last N frames.
    pub fn average_barrier_count(&self, n: usize) -> f32 {
        let count = self.stats_history.len().min(n);
        if count == 0 {
            return 0.0;
        }
        let total: usize = self.stats_history[self.stats_history.len() - count..]
            .iter()
            .map(|s| s.barrier_count)
            .sum();
        total as f32 / count as f32
    }

    pub fn backbuffer_size(&self) -> (u32, u32) {
        (self.backbuffer_width, self.backbuffer_height)
    }

    /// Generate a text report of the last frame's execution.
    pub fn frame_report(&self) -> String {
        let mut report = String::new();
        if let Some(stats) = self.last_stats() {
            report.push_str(&format!("=== Frame {} Report ===\n", stats.frame_index));
            report.push_str(&format!(
                "Total CPU time: {:.3}ms\n",
                stats.total_ms()
            ));
            report.push_str(&format!(
                "Active passes: {} ({} skipped)\n",
                stats.active_passes, stats.skipped_passes
            ));
            report.push_str(&format!("Barriers: {}\n", stats.barrier_count));
            report.push_str(&format!(
                "Resource allocations: {}\n",
                stats.resource_allocation_count
            ));
            report.push_str(&format!(
                "Async compute passes: {}\n",
                stats.async_compute_passes
            ));
            if let Some(ref budget) = stats.memory_budget {
                report.push_str(&format!("Memory: {}\n", budget));
            }
            report.push_str("\nPer-pass timing:\n");
            for (name, dur) in &stats.pass_times {
                report.push_str(&format!(
                    "  {}: {:.3}ms\n",
                    name,
                    dur.as_secs_f64() * 1000.0
                ));
            }
            if let Some((name, dur)) = stats.slowest_pass() {
                report.push_str(&format!(
                    "\nSlowest pass: {} ({:.3}ms)\n",
                    name,
                    dur.as_secs_f64() * 1000.0
                ));
            }
        } else {
            report.push_str("No frame data available.\n");
        }
        report
    }
}

// ---------------------------------------------------------------------------
// Multi-graph executor
// ---------------------------------------------------------------------------

/// Executes multiple render graphs in sequence (e.g., main scene + UI overlay).
pub struct MultiGraphExecutor {
    executor: GraphExecutor,
    graphs: Vec<(String, RenderGraph)>,
}

impl MultiGraphExecutor {
    pub fn new(executor: GraphExecutor) -> Self {
        Self {
            executor,
            graphs: Vec::new(),
        }
    }

    pub fn add_graph(&mut self, name: &str, graph: RenderGraph) {
        self.graphs.push((name.to_string(), graph));
    }

    pub fn remove_graph(&mut self, name: &str) {
        self.graphs.retain(|(n, _)| n != name);
    }

    pub fn execute_all(&mut self) -> Vec<Result<ExecutionStats, String>> {
        let mut results = Vec::new();
        // We need to iterate mutably, which requires indexing
        for i in 0..self.graphs.len() {
            let result = self.executor.execute_frame(&mut self.graphs[i].1);
            results.push(result);
        }
        results
    }

    pub fn executor(&self) -> &GraphExecutor {
        &self.executor
    }

    pub fn executor_mut(&mut self) -> &mut GraphExecutor {
        &mut self.executor
    }

    pub fn graph(&self, name: &str) -> Option<&RenderGraph> {
        self.graphs.iter().find(|(n, _)| n == name).map(|(_, g)| g)
    }

    pub fn graph_mut(&mut self, name: &str) -> Option<&mut RenderGraph> {
        self.graphs
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, g)| g)
    }
}

// ---------------------------------------------------------------------------
// Frame pacing
// ---------------------------------------------------------------------------

/// Simple frame pacing utility to target a specific framerate.
pub struct FramePacer {
    target_frame_time: Duration,
    last_frame_start: Instant,
    frame_times: Vec<Duration>,
    max_samples: usize,
}

impl FramePacer {
    pub fn new(target_fps: f64) -> Self {
        Self {
            target_frame_time: Duration::from_secs_f64(1.0 / target_fps),
            last_frame_start: Instant::now(),
            frame_times: Vec::new(),
            max_samples: 120,
        }
    }

    /// Call at the start of each frame. Returns delta time.
    pub fn begin_frame(&mut self) -> f32 {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_start);
        self.last_frame_start = now;
        self.frame_times.push(dt);
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }
        dt.as_secs_f32()
    }

    /// Call at the end of each frame. Sleeps if needed to hit target FPS.
    pub fn end_frame(&self) {
        let elapsed = self.last_frame_start.elapsed();
        if elapsed < self.target_frame_time {
            let remaining = self.target_frame_time - elapsed;
            std::thread::sleep(remaining);
        }
    }

    /// Average FPS over recent frames.
    pub fn average_fps(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let total: Duration = self.frame_times.iter().sum();
        let avg = total / self.frame_times.len() as u32;
        if avg.as_secs_f64() > 0.0 {
            1.0 / avg.as_secs_f64()
        } else {
            0.0
        }
    }

    /// 1% low frame time.
    pub fn percentile_1_low(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let mut sorted = self.frame_times.clone();
        sorted.sort();
        let idx = (sorted.len() as f64 * 0.99) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn set_target_fps(&mut self, fps: f64) {
        self.target_frame_time = Duration::from_secs_f64(1.0 / fps);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendergraph::graph::{PassCondition, RenderGraphBuilder, ResolutionScale};

    fn test_graph() -> RenderGraph {
        let mut b = RenderGraphBuilder::new("test_exec", 1920, 1080);
        let depth = b.texture("depth", TextureFormat::Depth32Float);
        let color = b.texture("color", TextureFormat::Rgba16Float);
        let final_rt = b.texture("final", TextureFormat::Rgba8Unorm);

        b.graphics_pass("depth_pre")
            .writes(depth, "depth")
            .finish();

        b.graphics_pass("lighting")
            .reads(depth, "depth")
            .writes(color, "color")
            .finish();

        b.graphics_pass("tonemap")
            .reads(color, "color")
            .writes(final_rt, "final")
            .side_effects()
            .finish();

        b.build()
    }

    #[test]
    fn test_execute_frame() {
        let mut graph = test_graph();
        let mut executor = GraphExecutor::new(1920, 1080);
        let stats = executor.execute_frame(&mut graph).unwrap();
        assert_eq!(stats.active_passes, 3);
        assert!(stats.barrier_count > 0);
    }

    #[test]
    fn test_frame_timeline() {
        let mut tl = FrameTimeline::triple_buffered();
        assert_eq!(tl.max_frames_in_flight(), 3);

        let f1 = tl.begin_frame();
        assert_eq!(f1, 1);
        tl.submit_frame();
        assert_eq!(tl.frames_in_flight(), 1);

        let f2 = tl.begin_frame();
        assert_eq!(f2, 2);
        tl.submit_frame();
        assert_eq!(tl.frames_in_flight(), 2);

        tl.complete_frame(1);
        assert_eq!(tl.frames_in_flight(), 1);
    }

    #[test]
    fn test_barrier_computation() {
        let mut graph = test_graph();
        let _ = graph.topological_sort().unwrap();
        let executor = GraphExecutor::new(1920, 1080);
        let sorted = vec![
            "depth_pre".to_string(),
            "lighting".to_string(),
            "tonemap".to_string(),
        ];
        let barriers = executor.compute_barriers(&graph, &sorted);
        assert!(barriers.len() >= 2); // depth->lighting, color->tonemap
    }

    #[test]
    fn test_async_compute_schedule() {
        let mut b = RenderGraphBuilder::new("async_test", 1920, 1080);
        let depth = b.texture("depth", TextureFormat::Depth32Float);
        let ssao = b.texture("ssao", TextureFormat::R16Float);
        let color = b.texture("color", TextureFormat::Rgba16Float);

        b.graphics_pass("depth_pre")
            .writes(depth, "depth")
            .finish();

        b.compute_pass("ssao")
            .reads(depth, "depth")
            .writes(ssao, "ssao")
            .queue(QueueAffinity::Compute)
            .finish();

        b.graphics_pass("lighting")
            .reads(depth, "depth")
            .reads(ssao, "ssao")
            .writes(color, "color")
            .finish();

        let mut graph = b.build();
        let schedule = AsyncComputeSchedule::from_graph(&mut graph).unwrap();
        assert_eq!(schedule.compute_passes.len(), 1);
        assert!(schedule.compute_passes.contains(&"ssao".to_string()));
    }

    #[test]
    fn test_frame_pacer() {
        let mut pacer = FramePacer::new(60.0);
        let dt = pacer.begin_frame();
        assert!(dt >= 0.0);
    }

    #[test]
    fn test_executor_with_custom_fn() {
        let mut graph = test_graph();
        let mut executor = GraphExecutor::new(1920, 1080);

        executor.register_fn("depth_pre", |ctx| {
            assert_eq!(ctx.pass_name, "depth_pre");
        });

        let stats = executor.execute_frame(&mut graph).unwrap();
        assert_eq!(stats.active_passes, 3);
    }

    #[test]
    fn test_hot_reload() {
        use crate::rendergraph::graph::{GraphConfig, PassConfig, ResourceConfig};
        use crate::rendergraph::resources::SizePolicy;
        let config = GraphConfig {
            label: "hot_reload".to_string(),
            resources: vec![ResourceConfig {
                name: "color".to_string(),
                format: TextureFormat::Rgba16Float,
                size: SizePolicy::Relative {
                    width_scale: 1.0,
                    height_scale: 1.0,
                },
                imported: false,
            }],
            passes: vec![PassConfig {
                name: "lighting".to_string(),
                pass_type: PassType::Graphics,
                inputs: vec![],
                outputs: vec!["color".to_string()],
                condition: None,
                resolution_scale: None,
                queue: QueueAffinity::Graphics,
                explicit_deps: vec![],
            }],
            features: vec![],
        };

        let mut executor = GraphExecutor::new(1920, 1080);
        executor.set_config(config);
        let graph = executor.rebuild_from_config();
        assert!(graph.is_some());
        let mut g = graph.unwrap();
        let sorted = g.topological_sort().unwrap();
        assert_eq!(sorted, vec!["lighting"]);
    }

    #[test]
    fn test_multi_graph_executor() {
        let executor = GraphExecutor::new(1920, 1080);
        let mut multi = MultiGraphExecutor::new(executor);
        multi.add_graph("main", test_graph());
        let results = multi.execute_all();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn test_frame_report() {
        let mut graph = test_graph();
        let mut executor = GraphExecutor::new(1920, 1080);
        let _stats = executor.execute_frame(&mut graph).unwrap();
        let report = executor.frame_report();
        assert!(report.contains("Frame"));
        assert!(report.contains("Total CPU time"));
    }
}
