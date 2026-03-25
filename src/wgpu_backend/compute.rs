//! Compute shader support: storage buffers, dispatch, CPU fallback, profiling.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::backend::{
    BackendCapabilities, BackendContext, BufferHandle, BufferUsage, ComputePipelineHandle,
    GpuBackend, GpuCommand, PipelineLayout, ShaderHandle, ShaderStage, SoftwareContext,
};

// ---------------------------------------------------------------------------
// Access mode
// ---------------------------------------------------------------------------

/// Access mode for a binding in a compute shader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

// ---------------------------------------------------------------------------
// Bind group layout
// ---------------------------------------------------------------------------

/// Describes one entry in a bind group.
#[derive(Debug, Clone)]
pub struct BindGroupEntry {
    pub binding: u32,
    pub buffer_or_texture: BindingResource,
    pub access: AccessMode,
}

/// What is bound at a given slot.
#[derive(Debug, Clone)]
pub enum BindingResource {
    Buffer(BufferHandle),
    Texture(super::backend::TextureHandle),
}

/// Layout of a bind group (the descriptor, not the actual bound resources).
#[derive(Debug, Clone)]
pub struct BindGroupLayout {
    pub entries: Vec<BindGroupLayoutEntry>,
}

impl BindGroupLayout {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn push(mut self, binding: u32, access: AccessMode) -> Self {
        self.entries.push(BindGroupLayoutEntry { binding, access });
        self
    }
}

impl Default for BindGroupLayout {
    fn default() -> Self { Self::new() }
}

/// One entry in a bind-group layout descriptor.
#[derive(Debug, Clone)]
pub struct BindGroupLayoutEntry {
    pub binding: u32,
    pub access: AccessMode,
}

// ---------------------------------------------------------------------------
// ComputePipeline
// ---------------------------------------------------------------------------

/// A compute pipeline ready for dispatch.
#[derive(Debug, Clone)]
pub struct ComputePipeline {
    pub shader: ShaderHandle,
    pub bind_group_layout: BindGroupLayout,
    pub workgroup_size: [u32; 3],
    pub handle: ComputePipelineHandle,
}

// ---------------------------------------------------------------------------
// ComputeBuffer
// ---------------------------------------------------------------------------

/// A buffer suitable for compute shader storage.
#[derive(Debug, Clone)]
pub struct ComputeBuffer {
    pub handle: BufferHandle,
    pub size: usize,
    pub element_size: usize,
}

impl ComputeBuffer {
    /// Number of elements this buffer can hold.
    pub fn element_count(&self) -> usize {
        if self.element_size == 0 { 0 } else { self.size / self.element_size }
    }
}

// ---------------------------------------------------------------------------
// ComputeProfiler
// ---------------------------------------------------------------------------

/// Tracks timing information for compute dispatches.
pub struct ComputeProfiler {
    records: Vec<DispatchRecord>,
    max_records: usize,
}

#[derive(Debug, Clone)]
pub struct DispatchRecord {
    pub label: String,
    pub workgroups: [u32; 3],
    pub duration: Duration,
}

impl ComputeProfiler {
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Vec::with_capacity(max_records.min(4096)),
            max_records,
        }
    }

    pub fn record(&mut self, label: &str, workgroups: [u32; 3], duration: Duration) {
        if self.records.len() >= self.max_records {
            self.records.remove(0);
        }
        self.records.push(DispatchRecord {
            label: label.to_string(),
            workgroups,
            duration,
        });
    }

    pub fn average_duration(&self) -> Duration {
        if self.records.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.records.iter().map(|r| r.duration).sum();
        total / self.records.len() as u32
    }

    pub fn total_dispatches(&self) -> usize {
        self.records.len()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

    pub fn last(&self) -> Option<&DispatchRecord> {
        self.records.last()
    }

    pub fn records(&self) -> &[DispatchRecord] {
        &self.records
    }
}

// ---------------------------------------------------------------------------
// CPU fallback kernel
// ---------------------------------------------------------------------------

/// A CPU compute kernel: runs `f(global_id)` for every invocation in the
/// workgroup grid, in parallel across a simple thread-per-row scheme.
pub struct CpuKernel {
    pub workgroup_size: [u32; 3],
}

impl CpuKernel {
    pub fn new(workgroup_size: [u32; 3]) -> Self {
        Self { workgroup_size }
    }

    /// Dispatch the kernel on CPU.  Calls `f(global_x, global_y, global_z)`
    /// for every invocation.
    pub fn dispatch<F>(&self, groups: [u32; 3], mut f: F)
    where
        F: FnMut(u32, u32, u32),
    {
        let [sx, sy, sz] = self.workgroup_size;
        let [gx, gy, gz] = groups;
        for gz_i in 0..gz {
            for gy_i in 0..gy {
                for gx_i in 0..gx {
                    for lz in 0..sz {
                        for ly in 0..sy {
                            for lx in 0..sx {
                                let x = gx_i * sx + lx;
                                let y = gy_i * sy + ly;
                                let z = gz_i * sz + lz;
                                f(x, y, z);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Total number of invocations for the given dispatch size.
    pub fn total_invocations(&self, groups: [u32; 3]) -> u64 {
        let [sx, sy, sz] = self.workgroup_size;
        let [gx, gy, gz] = groups;
        (sx as u64) * (sy as u64) * (sz as u64)
            * (gx as u64) * (gy as u64) * (gz as u64)
    }
}

// ---------------------------------------------------------------------------
// ComputeContext
// ---------------------------------------------------------------------------

/// High-level compute context wrapping a backend.
pub struct ComputeContext {
    pub backend_type: GpuBackend,
    backend: Box<dyn BackendContext>,
    capabilities: BackendCapabilities,
    profiler: ComputeProfiler,
    pipelines: HashMap<u64, ComputePipeline>,
}

impl ComputeContext {
    pub fn new(backend: Box<dyn BackendContext>, backend_type: GpuBackend) -> Self {
        let capabilities = BackendCapabilities::for_backend(backend_type);
        Self {
            backend_type,
            backend,
            capabilities,
            profiler: ComputeProfiler::new(1024),
            pipelines: HashMap::new(),
        }
    }

    /// Create a compute context with a software backend.
    pub fn software() -> Self {
        Self::new(Box::new(SoftwareContext::new()), GpuBackend::Software)
    }

    /// Create a typed storage buffer from a slice, copying element data.
    pub fn create_storage_buffer<T: Copy>(&mut self, data: &[T]) -> ComputeBuffer {
        let element_size = std::mem::size_of::<T>();
        let byte_size = element_size * data.len();
        let handle = self.backend.create_buffer(byte_size, BufferUsage::STORAGE);

        // Copy data bytes
        let byte_slice = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, byte_size)
        };
        self.backend.write_buffer(handle, byte_slice);

        ComputeBuffer {
            handle,
            size: byte_size,
            element_size,
        }
    }

    /// Create an empty storage buffer with room for `count` elements of type T.
    pub fn create_empty_buffer<T>(&mut self, count: usize) -> ComputeBuffer {
        let element_size = std::mem::size_of::<T>();
        let byte_size = element_size * count;
        let handle = self.backend.create_buffer(byte_size, BufferUsage::STORAGE);
        ComputeBuffer {
            handle,
            size: byte_size,
            element_size,
        }
    }

    /// Create a compute pipeline.
    pub fn create_pipeline(
        &mut self,
        source: &str,
        layout: BindGroupLayout,
        workgroup_size: [u32; 3],
    ) -> ComputePipeline {
        let shader = self.backend.create_shader(source, ShaderStage::Compute);
        let pl = PipelineLayout::default();
        let handle = self.backend.create_compute_pipeline(shader, &pl);
        let pipeline = ComputePipeline {
            shader,
            bind_group_layout: layout,
            workgroup_size,
            handle,
        };
        self.pipelines.insert(handle.0, pipeline.clone());
        pipeline
    }

    /// Dispatch a compute pipeline.
    pub fn dispatch(&mut self, pipeline: &ComputePipeline, x: u32, y: u32, z: u32) {
        let start = Instant::now();

        if self.backend_type == GpuBackend::Software {
            // CPU fallback: nothing to actually execute — the software backend
            // records the command but doesn't run shader code.
        }

        self.backend.submit(&[GpuCommand::Dispatch {
            pipeline: pipeline.handle,
            x,
            y,
            z,
        }]);

        let elapsed = start.elapsed();
        self.profiler.record("dispatch", [x, y, z], elapsed);
    }

    /// Indirect dispatch: the workgroup counts come from a GPU buffer.
    pub fn indirect_dispatch(&mut self, pipeline: &ComputePipeline, args_buffer: &ComputeBuffer) {
        // Read the indirect args from the buffer (3 x u32).
        let data = self.backend.read_buffer(args_buffer.handle);
        let mut groups = [1u32, 1, 1];
        if data.len() >= 12 {
            for i in 0..3 {
                let bytes = [data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]];
                groups[i] = u32::from_le_bytes(bytes);
            }
        }
        self.dispatch(pipeline, groups[0], groups[1], groups[2]);
    }

    /// Insert a memory barrier.
    pub fn memory_barrier(&mut self) {
        self.backend.submit(&[GpuCommand::Barrier]);
    }

    /// Read back buffer contents as a typed slice.
    pub fn read_back<T: Copy + Default>(&self, buffer: &ComputeBuffer) -> Vec<T> {
        let data = self.backend.read_buffer(buffer.handle);
        let elem_size = std::mem::size_of::<T>();
        if elem_size == 0 {
            return Vec::new();
        }
        let count = data.len() / elem_size;
        let mut result = vec![T::default(); count];
        unsafe {
            let dst = std::slice::from_raw_parts_mut(
                result.as_mut_ptr() as *mut u8,
                count * elem_size,
            );
            dst.copy_from_slice(&data[..count * elem_size]);
        }
        result
    }

    /// Write typed data into an existing buffer.
    pub fn write_buffer<T: Copy>(&mut self, buffer: &ComputeBuffer, data: &[T]) {
        let byte_size = std::mem::size_of::<T>() * data.len();
        let byte_slice = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, byte_size)
        };
        self.backend.write_buffer(buffer.handle, byte_slice);
    }

    /// Get the profiler.
    pub fn profiler(&self) -> &ComputeProfiler {
        &self.profiler
    }

    /// Mutable access to the profiler.
    pub fn profiler_mut(&mut self) -> &mut ComputeProfiler {
        &mut self.profiler
    }

    /// Whether the backend supports compute shaders natively.
    pub fn supports_compute(&self) -> bool {
        self.capabilities.compute_shaders
    }

    /// Destroy a compute buffer.
    pub fn destroy_buffer(&mut self, buffer: &ComputeBuffer) {
        self.backend.destroy_buffer(buffer.handle);
    }
}

// ---------------------------------------------------------------------------
// CPU fallback dispatch (parallel-ish, no rayon dep — uses std threads)
// ---------------------------------------------------------------------------

/// Run a CPU compute kernel across multiple threads.
/// `f` receives `(thread_id, global_x, global_y, global_z)`.
pub fn cpu_parallel_dispatch<F>(
    workgroup_size: [u32; 3],
    groups: [u32; 3],
    num_threads: usize,
    f: F,
) where
    F: Fn(usize, u32, u32, u32) + Send + Sync,
{
    let [sx, sy, sz] = workgroup_size;
    let [gx, gy, gz] = groups;
    let total_groups = (gx as usize) * (gy as usize) * (gz as usize);
    let num_threads = num_threads.max(1).min(total_groups);

    if num_threads <= 1 {
        // Single-threaded fast path.
        let kernel = CpuKernel::new(workgroup_size);
        kernel.dispatch(groups, |x, y, z| f(0, x, y, z));
        return;
    }

    // Build a flat list of group indices and split among threads.
    let groups_per_thread = (total_groups + num_threads - 1) / num_threads;
    let f_ref = &f;

    std::thread::scope(|scope| {
        for tid in 0..num_threads {
            let start = tid * groups_per_thread;
            let end = ((tid + 1) * groups_per_thread).min(total_groups);
            scope.spawn(move || {
                for flat in start..end {
                    let gz_i = (flat / ((gx as usize) * (gy as usize))) as u32;
                    let rem = flat % ((gx as usize) * (gy as usize));
                    let gy_i = (rem / (gx as usize)) as u32;
                    let gx_i = (rem % (gx as usize)) as u32;
                    for lz in 0..sz {
                        for ly in 0..sy {
                            for lx in 0..sx {
                                let x = gx_i * sx + lx;
                                let y = gy_i * sy + ly;
                                let z = gz_i * sz + lz;
                                f_ref(tid, x, y, z);
                            }
                        }
                    }
                }
            });
        }
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_buffer_element_count() {
        let buf = ComputeBuffer {
            handle: BufferHandle(1),
            size: 40,
            element_size: 4,
        };
        assert_eq!(buf.element_count(), 10);
    }

    #[test]
    fn create_storage_buffer_f32() {
        let mut ctx = ComputeContext::software();
        let data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let buf = ctx.create_storage_buffer(&data);
        assert_eq!(buf.size, 16);
        assert_eq!(buf.element_size, 4);
        assert_eq!(buf.element_count(), 4);

        let readback: Vec<f32> = ctx.read_back(&buf);
        assert_eq!(readback, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn create_storage_buffer_u32() {
        let mut ctx = ComputeContext::software();
        let data: Vec<u32> = vec![10, 20, 30];
        let buf = ctx.create_storage_buffer(&data);
        let readback: Vec<u32> = ctx.read_back(&buf);
        assert_eq!(readback, vec![10, 20, 30]);
    }

    #[test]
    fn create_empty_buffer() {
        let mut ctx = ComputeContext::software();
        let buf = ctx.create_empty_buffer::<f32>(8);
        assert_eq!(buf.size, 32);
        assert_eq!(buf.element_count(), 8);
    }

    #[test]
    fn dispatch_pipeline() {
        let mut ctx = ComputeContext::software();
        let layout = BindGroupLayout::new().push(0, AccessMode::ReadWrite);
        let pipeline = ctx.create_pipeline("void main(){}", layout, [64, 1, 1]);
        ctx.dispatch(&pipeline, 4, 1, 1);
        assert_eq!(ctx.profiler().total_dispatches(), 1);
    }

    #[test]
    fn indirect_dispatch() {
        let mut ctx = ComputeContext::software();
        let layout = BindGroupLayout::new();
        let pipeline = ctx.create_pipeline("void main(){}", layout, [1, 1, 1]);

        // Create an indirect args buffer with [2, 1, 1]
        let args: Vec<u32> = vec![2, 1, 1];
        let args_buf = ctx.create_storage_buffer(&args);
        ctx.indirect_dispatch(&pipeline, &args_buf);
        assert_eq!(ctx.profiler().total_dispatches(), 1);
    }

    #[test]
    fn memory_barrier() {
        let mut ctx = ComputeContext::software();
        ctx.memory_barrier();
    }

    #[test]
    fn write_and_read_back() {
        let mut ctx = ComputeContext::software();
        let buf = ctx.create_empty_buffer::<u32>(4);
        ctx.write_buffer(&buf, &[100u32, 200, 300, 400]);
        let result: Vec<u32> = ctx.read_back(&buf);
        assert_eq!(result, vec![100, 200, 300, 400]);
    }

    #[test]
    fn profiler_average() {
        let mut profiler = ComputeProfiler::new(10);
        profiler.record("a", [1, 1, 1], Duration::from_millis(10));
        profiler.record("b", [1, 1, 1], Duration::from_millis(20));
        assert_eq!(profiler.total_dispatches(), 2);
        let avg = profiler.average_duration();
        assert_eq!(avg, Duration::from_millis(15));
    }

    #[test]
    fn profiler_rolling() {
        let mut profiler = ComputeProfiler::new(3);
        for i in 0..5 {
            profiler.record(&format!("d{}", i), [1, 1, 1], Duration::from_millis(i as u64));
        }
        assert_eq!(profiler.total_dispatches(), 3);
        assert_eq!(profiler.last().unwrap().label, "d4");
    }

    #[test]
    fn profiler_clear() {
        let mut profiler = ComputeProfiler::new(10);
        profiler.record("x", [1, 1, 1], Duration::from_millis(5));
        profiler.clear();
        assert_eq!(profiler.total_dispatches(), 0);
        assert_eq!(profiler.average_duration(), Duration::ZERO);
    }

    #[test]
    fn cpu_kernel_dispatch() {
        let kernel = CpuKernel::new([2, 2, 1]);
        let mut invocations = Vec::new();
        kernel.dispatch([2, 1, 1], |x, y, z| {
            invocations.push((x, y, z));
        });
        // 2 groups * (2*2*1) local = 8 invocations
        assert_eq!(invocations.len(), 8);
        assert!(invocations.contains(&(0, 0, 0)));
        assert!(invocations.contains(&(3, 1, 0)));
    }

    #[test]
    fn cpu_kernel_total_invocations() {
        let kernel = CpuKernel::new([8, 8, 1]);
        assert_eq!(kernel.total_invocations([4, 4, 1]), 8 * 8 * 4 * 4);
    }

    #[test]
    fn cpu_parallel_dispatch_runs() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let counter = AtomicU32::new(0);
        cpu_parallel_dispatch([2, 1, 1], [4, 1, 1], 2, |_tid, _x, _y, _z| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        assert_eq!(counter.load(Ordering::Relaxed), 8); // 4 groups * 2 local
    }

    #[test]
    fn cpu_parallel_dispatch_single_thread() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let counter = AtomicU32::new(0);
        cpu_parallel_dispatch([1, 1, 1], [3, 2, 1], 1, |_tid, _x, _y, _z| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
        assert_eq!(counter.load(Ordering::Relaxed), 6);
    }

    #[test]
    fn bind_group_layout_builder() {
        let layout = BindGroupLayout::new()
            .push(0, AccessMode::ReadOnly)
            .push(1, AccessMode::WriteOnly)
            .push(2, AccessMode::ReadWrite);
        assert_eq!(layout.entries.len(), 3);
        assert_eq!(layout.entries[1].access, AccessMode::WriteOnly);
    }

    #[test]
    fn supports_compute_software() {
        let ctx = ComputeContext::software();
        assert!(ctx.supports_compute());
    }

    #[test]
    fn destroy_buffer() {
        let mut ctx = ComputeContext::software();
        let buf = ctx.create_storage_buffer(&[1u32, 2, 3]);
        ctx.destroy_buffer(&buf);
        let readback: Vec<u32> = ctx.read_back(&buf);
        assert!(readback.is_empty());
    }
}
