//! SSBO management: typed buffers, double-buffering, atomic counters,
//! buffer pools, barrier types, memory tracking, mapped ranges, copy engine.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Buffer handle & usage
// ---------------------------------------------------------------------------

/// Opaque handle to a GPU buffer (wraps a raw OpenGL buffer name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle {
    /// Raw OpenGL buffer object name.
    pub raw: u32,
    /// Size in bytes currently allocated on the GPU.
    pub size_bytes: usize,
}

impl BufferHandle {
    /// Create a new handle from a raw GL name and size.
    pub fn new(raw: u32, size_bytes: usize) -> Self {
        Self { raw, size_bytes }
    }

    /// Null / invalid handle.
    pub fn null() -> Self {
        Self {
            raw: 0,
            size_bytes: 0,
        }
    }

    /// Returns true if this handle refers to no buffer.
    pub fn is_null(&self) -> bool {
        self.raw == 0
    }
}

/// Intended usage hint when creating a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferUsage {
    /// Data is set once and used many times (GL_STATIC_DRAW equivalent).
    StaticDraw,
    /// Data changes frequently and is used many times (GL_DYNAMIC_DRAW equivalent).
    DynamicDraw,
    /// Data changes every frame (GL_STREAM_DRAW equivalent).
    StreamDraw,
    /// GPU writes, CPU reads (GL_STREAM_READ equivalent).
    StreamRead,
    /// GPU writes, GPU reads (GL_DYNAMIC_COPY equivalent).
    DynamicCopy,
}

impl BufferUsage {
    /// Map to a GL enum value (integer form used by glow).
    pub fn to_gl(self) -> u32 {
        match self {
            BufferUsage::StaticDraw => 0x88E4,  // GL_STATIC_DRAW
            BufferUsage::DynamicDraw => 0x88E8, // GL_DYNAMIC_DRAW
            BufferUsage::StreamDraw => 0x88E0,  // GL_STREAM_DRAW
            BufferUsage::StreamRead => 0x88E1,  // GL_STREAM_READ
            BufferUsage::DynamicCopy => 0x88EA, // GL_DYNAMIC_COPY
        }
    }
}

// ---------------------------------------------------------------------------
// GL constants used throughout this module
// ---------------------------------------------------------------------------

/// GL_SHADER_STORAGE_BUFFER
const GL_SHADER_STORAGE_BUFFER: u32 = 0x90D2;
/// GL_ATOMIC_COUNTER_BUFFER
const GL_ATOMIC_COUNTER_BUFFER: u32 = 0x92C0;
/// GL_COPY_READ_BUFFER
const GL_COPY_READ_BUFFER: u32 = 0x8F36;
/// GL_COPY_WRITE_BUFFER
const GL_COPY_WRITE_BUFFER: u32 = 0x8F37;
/// GL_MAP_READ_BIT
const GL_MAP_READ_BIT: u32 = 0x0001;
/// GL_MAP_WRITE_BIT
const GL_MAP_WRITE_BIT: u32 = 0x0002;
/// GL_BUFFER_UPDATE_BARRIER_BIT
const _GL_BUFFER_UPDATE_BARRIER_BIT: u32 = 0x00000200;
/// GL_SHADER_STORAGE_BARRIER_BIT
const GL_SHADER_STORAGE_BARRIER_BIT: u32 = 0x00002000;
/// GL_VERTEX_ATTRIB_ARRAY_BARRIER_BIT
const GL_VERTEX_ATTRIB_ARRAY_BARRIER_BIT: u32 = 0x00000001;
/// GL_COMMAND_BARRIER_BIT
const GL_COMMAND_BARRIER_BIT: u32 = 0x00000040;

// ---------------------------------------------------------------------------
// Buffer barrier types
// ---------------------------------------------------------------------------

/// Types of memory barriers that can be issued after compute writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferBarrierType {
    /// Barrier for shader storage buffer reads/writes.
    ShaderStorage,
    /// Barrier so that vertex attribute fetches see the writes.
    VertexAttrib,
    /// Barrier so that indirect draw commands see the writes.
    IndirectDraw,
    /// Combined: shader storage + vertex attrib.
    ShaderStorageAndVertex,
    /// Combined: all three.
    All,
}

impl BufferBarrierType {
    /// Convert to the raw GL bitfield.
    pub fn to_gl_bits(self) -> u32 {
        match self {
            BufferBarrierType::ShaderStorage => GL_SHADER_STORAGE_BARRIER_BIT,
            BufferBarrierType::VertexAttrib => GL_VERTEX_ATTRIB_ARRAY_BARRIER_BIT,
            BufferBarrierType::IndirectDraw => GL_COMMAND_BARRIER_BIT,
            BufferBarrierType::ShaderStorageAndVertex => {
                GL_SHADER_STORAGE_BARRIER_BIT | GL_VERTEX_ATTRIB_ARRAY_BARRIER_BIT
            }
            BufferBarrierType::All => {
                GL_SHADER_STORAGE_BARRIER_BIT
                    | GL_VERTEX_ATTRIB_ARRAY_BARRIER_BIT
                    | GL_COMMAND_BARRIER_BIT
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Memory tracker
// ---------------------------------------------------------------------------

/// Tracks total GPU memory allocated through this subsystem.
#[derive(Debug)]
pub struct MemoryTracker {
    /// Total bytes currently allocated.
    allocated_bytes: AtomicU64,
    /// High-water mark.
    peak_bytes: AtomicU64,
    /// Number of allocations made (lifetime).
    allocation_count: AtomicU64,
    /// Number of frees made (lifetime).
    free_count: AtomicU64,
}

impl MemoryTracker {
    /// Create a new tracker starting at zero.
    pub fn new() -> Self {
        Self {
            allocated_bytes: AtomicU64::new(0),
            peak_bytes: AtomicU64::new(0),
            allocation_count: AtomicU64::new(0),
            free_count: AtomicU64::new(0),
        }
    }

    /// Record an allocation of `bytes`.
    pub fn record_alloc(&self, bytes: usize) {
        let prev = self.allocated_bytes.fetch_add(bytes as u64, Ordering::Relaxed);
        let new_total = prev + bytes as u64;
        // Update peak — CAS loop
        let mut current_peak = self.peak_bytes.load(Ordering::Relaxed);
        while new_total > current_peak {
            match self.peak_bytes.compare_exchange_weak(
                current_peak,
                new_total,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_peak = actual,
            }
        }
        self.allocation_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a deallocation of `bytes`.
    pub fn record_free(&self, bytes: usize) {
        self.allocated_bytes.fetch_sub(bytes as u64, Ordering::Relaxed);
        self.free_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Current total allocated bytes.
    pub fn current_bytes(&self) -> u64 {
        self.allocated_bytes.load(Ordering::Relaxed)
    }

    /// Peak allocated bytes ever.
    pub fn peak_bytes(&self) -> u64 {
        self.peak_bytes.load(Ordering::Relaxed)
    }

    /// Lifetime allocation count.
    pub fn allocation_count(&self) -> u64 {
        self.allocation_count.load(Ordering::Relaxed)
    }

    /// Lifetime free count.
    pub fn free_count(&self) -> u64 {
        self.free_count.load(Ordering::Relaxed)
    }

    /// Returns a formatted summary string.
    pub fn summary(&self) -> String {
        let curr = self.current_bytes();
        let peak = self.peak_bytes();
        let allocs = self.allocation_count();
        let frees = self.free_count();
        format!(
            "GPU Memory: {:.2} MB current, {:.2} MB peak, {} allocs, {} frees",
            curr as f64 / (1024.0 * 1024.0),
            peak as f64 / (1024.0 * 1024.0),
            allocs,
            frees,
        )
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TypedBuffer<T>
// ---------------------------------------------------------------------------

/// A strongly-typed GPU Shader Storage Buffer Object.
///
/// `T` must be `Copy` so that we can safely transmute slices to byte slices
/// for upload/download. In a real engine, `T` would also be `bytemuck::Pod`.
///
/// This buffer holds the GL name, current element count, capacity, usage hint,
/// and a reference to the shared memory tracker.
pub struct TypedBuffer<T: Copy> {
    handle: BufferHandle,
    len: usize,
    capacity: usize,
    usage: BufferUsage,
    binding_index: u32,
    tracker: Arc<MemoryTracker>,
    _marker: PhantomData<T>,
}

impl<T: Copy> TypedBuffer<T> {
    /// Byte size of a single element.
    const ELEM_SIZE: usize = std::mem::size_of::<T>();

    /// Create a new typed buffer with given capacity (number of T elements).
    ///
    /// The `gl` parameter is the glow context. `binding_index` is the SSBO
    /// binding point. The buffer is allocated but not filled.
    pub fn create(
        gl: &glow::Context,
        capacity: usize,
        usage: BufferUsage,
        binding_index: u32,
        tracker: Arc<MemoryTracker>,
    ) -> Self {
        use glow::HasContext;
        let byte_size = capacity * Self::ELEM_SIZE;
        let raw = unsafe {
            let buf = gl.create_buffer().expect("Failed to create GL buffer");
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.buffer_data_size(GL_SHADER_STORAGE_BUFFER, byte_size as i32, usage.to_gl());
            gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, binding_index, Some(buf));
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
            buf
        };
        tracker.record_alloc(byte_size);
        Self {
            handle: BufferHandle::new(raw.0.get(), byte_size),
            len: 0,
            capacity,
            usage,
            binding_index,
            tracker,
            _marker: PhantomData,
        }
    }

    /// Upload a slice of data into the buffer starting at element offset 0.
    /// The buffer is resized if necessary.
    pub fn upload(&mut self, gl: &glow::Context, data: &[T]) {
        if data.len() > self.capacity {
            self.resize(gl, data.len());
        }
        self.len = data.len();
        let byte_slice = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * Self::ELEM_SIZE)
        };
        use glow::HasContext;
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(self.gl_buffer()));
            gl.buffer_sub_data_u8_slice(GL_SHADER_STORAGE_BUFFER, 0, byte_slice);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
    }

    /// Upload data at a specific element offset (partial update).
    pub fn upload_range(&mut self, gl: &glow::Context, offset: usize, data: &[T]) {
        assert!(
            offset + data.len() <= self.capacity,
            "upload_range out of bounds"
        );
        let byte_offset = (offset * Self::ELEM_SIZE) as i32;
        let byte_slice = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * Self::ELEM_SIZE)
        };
        use glow::HasContext;
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(self.gl_buffer()));
            gl.buffer_sub_data_u8_slice(GL_SHADER_STORAGE_BUFFER, byte_offset, byte_slice);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        // Update len to cover this range if it extends past current
        let new_end = offset + data.len();
        if new_end > self.len {
            self.len = new_end;
        }
    }

    /// Download the entire buffer contents back to the CPU.
    pub fn download(&self, gl: &glow::Context) -> Vec<T> {
        if self.len == 0 {
            return Vec::new();
        }
        let byte_count = self.len * Self::ELEM_SIZE;
        let mut bytes = vec![0u8; byte_count];
        use glow::HasContext;
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(self.gl_buffer()));
            gl.get_buffer_sub_data(GL_SHADER_STORAGE_BUFFER, 0, &mut bytes);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        // Reinterpret bytes as Vec<T>
        let mut result = Vec::with_capacity(self.len);
        let src_ptr = bytes.as_ptr() as *const T;
        for i in 0..self.len {
            result.push(unsafe { std::ptr::read(src_ptr.add(i)) });
        }
        result
    }

    /// Download a sub-range [offset..offset+count) of elements.
    pub fn download_range(&self, gl: &glow::Context, offset: usize, count: usize) -> Vec<T> {
        assert!(
            offset + count <= self.len,
            "download_range out of bounds"
        );
        let byte_offset = (offset * Self::ELEM_SIZE) as i32;
        let byte_count = count * Self::ELEM_SIZE;
        let mut bytes = vec![0u8; byte_count];
        use glow::HasContext;
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(self.gl_buffer()));
            gl.get_buffer_sub_data(GL_SHADER_STORAGE_BUFFER, byte_offset, &mut bytes);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        let mut result = Vec::with_capacity(count);
        let src_ptr = bytes.as_ptr() as *const T;
        for i in 0..count {
            result.push(unsafe { std::ptr::read(src_ptr.add(i)) });
        }
        result
    }

    /// Resize the buffer to a new capacity. Existing data is lost.
    pub fn resize(&mut self, gl: &glow::Context, new_capacity: usize) {
        let old_byte_size = self.capacity * Self::ELEM_SIZE;
        let new_byte_size = new_capacity * Self::ELEM_SIZE;
        self.tracker.record_free(old_byte_size);
        use glow::HasContext;
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(self.gl_buffer()));
            gl.buffer_data_size(
                GL_SHADER_STORAGE_BUFFER,
                new_byte_size as i32,
                self.usage.to_gl(),
            );
            gl.bind_buffer_base(
                GL_SHADER_STORAGE_BUFFER,
                self.binding_index,
                Some(self.gl_buffer()),
            );
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        self.tracker.record_alloc(new_byte_size);
        self.handle.size_bytes = new_byte_size;
        self.capacity = new_capacity;
        self.len = 0; // data is invalidated
    }

    /// Resize the buffer, preserving existing data up to min(old_len, new_capacity).
    pub fn resize_preserve(&mut self, gl: &glow::Context, new_capacity: usize) {
        if new_capacity == self.capacity {
            return;
        }
        let old_byte_size = self.capacity * Self::ELEM_SIZE;
        let new_byte_size = new_capacity * Self::ELEM_SIZE;
        let copy_bytes = std::cmp::min(self.len * Self::ELEM_SIZE, new_byte_size);

        use glow::HasContext;
        unsafe {
            // Create a temp buffer, copy old data into it
            let tmp = gl.create_buffer().expect("Failed to create temp buffer");
            gl.bind_buffer(GL_COPY_READ_BUFFER, Some(self.gl_buffer()));
            gl.bind_buffer(GL_COPY_WRITE_BUFFER, Some(tmp));
            gl.buffer_data_size(GL_COPY_WRITE_BUFFER, copy_bytes as i32, BufferUsage::StreamDraw.to_gl());
            gl.copy_buffer_sub_data(
                GL_COPY_READ_BUFFER,
                GL_COPY_WRITE_BUFFER,
                0,
                0,
                copy_bytes as i32,
            );
            // Reallocate original buffer
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(self.gl_buffer()));
            gl.buffer_data_size(
                GL_SHADER_STORAGE_BUFFER,
                new_byte_size as i32,
                self.usage.to_gl(),
            );
            // Copy back from tmp
            gl.bind_buffer(GL_COPY_READ_BUFFER, Some(tmp));
            gl.bind_buffer(GL_COPY_WRITE_BUFFER, Some(self.gl_buffer()));
            gl.copy_buffer_sub_data(
                GL_COPY_READ_BUFFER,
                GL_COPY_WRITE_BUFFER,
                0,
                0,
                copy_bytes as i32,
            );
            gl.delete_buffer(tmp);
            gl.bind_buffer_base(
                GL_SHADER_STORAGE_BUFFER,
                self.binding_index,
                Some(self.gl_buffer()),
            );
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        self.tracker.record_free(old_byte_size);
        self.tracker.record_alloc(new_byte_size);
        self.handle.size_bytes = new_byte_size;
        self.capacity = new_capacity;
        self.len = std::cmp::min(self.len, new_capacity);
    }

    /// Bind this buffer to its SSBO binding index.
    pub fn bind(&self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.bind_buffer_base(
                GL_SHADER_STORAGE_BUFFER,
                self.binding_index,
                Some(self.gl_buffer()),
            );
        }
    }

    /// Unbind SSBO at this buffer's binding index.
    pub fn unbind(&self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, self.binding_index, None);
        }
    }

    /// Issue a memory barrier for this buffer.
    pub fn barrier(&self, gl: &glow::Context, barrier_type: BufferBarrierType) {
        use glow::HasContext;
        unsafe {
            gl.memory_barrier(barrier_type.to_gl_bits());
        }
    }

    /// Clear the buffer contents to zero.
    pub fn clear(&mut self, gl: &glow::Context) {
        let byte_size = self.capacity * Self::ELEM_SIZE;
        let zeros = vec![0u8; byte_size];
        use glow::HasContext;
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(self.gl_buffer()));
            gl.buffer_sub_data_u8_slice(GL_SHADER_STORAGE_BUFFER, 0, &zeros);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        self.len = 0;
    }

    /// Delete the underlying GL buffer.
    pub fn destroy(self, gl: &glow::Context) {
        let byte_size = self.capacity * Self::ELEM_SIZE;
        self.tracker.record_free(byte_size);
        use glow::HasContext;
        unsafe {
            gl.delete_buffer(self.gl_buffer());
        }
    }

    /// Number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Current capacity in number of elements.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Size in bytes currently allocated.
    pub fn byte_size(&self) -> usize {
        self.handle.size_bytes
    }

    /// Raw buffer handle.
    pub fn handle(&self) -> BufferHandle {
        self.handle
    }

    /// SSBO binding index.
    pub fn binding(&self) -> u32 {
        self.binding_index
    }

    /// Reconstruct a glow NativeBuffer from the raw u32.
    fn gl_buffer(&self) -> glow::NativeBuffer {
        // glow::NativeBuffer is a NonZeroU32 wrapper
        glow::NativeBuffer(std::num::NonZeroU32::new(self.handle.raw).unwrap())
    }
}

// ---------------------------------------------------------------------------
// ParticleBuffer (double-buffered)
// ---------------------------------------------------------------------------

/// A particle position/velocity datum, tightly packed for GPU.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ParticleGpuData {
    pub position: [f32; 4], // xyz + age
    pub velocity: [f32; 4], // xyz + lifetime
}

impl Default for ParticleGpuData {
    fn default() -> Self {
        Self {
            position: [0.0; 4],
            velocity: [0.0; 4],
        }
    }
}

/// Double-buffered particle storage for ping-pong compute dispatch.
///
/// One buffer is the "read" (source) and the other is the "write" (target).
/// After a compute dispatch, call `swap()` to flip them.
pub struct ParticleBuffer {
    buffers: [BufferHandle; 2],
    capacity: usize,
    active_count: usize,
    read_index: usize,
    binding_read: u32,
    binding_write: u32,
    tracker: Arc<MemoryTracker>,
}

impl ParticleBuffer {
    /// Element size for particle data.
    const PARTICLE_SIZE: usize = std::mem::size_of::<ParticleGpuData>();

    /// Create a double-buffered particle buffer.
    pub fn create(
        gl: &glow::Context,
        capacity: usize,
        binding_read: u32,
        binding_write: u32,
        tracker: Arc<MemoryTracker>,
    ) -> Self {
        let byte_size = capacity * Self::PARTICLE_SIZE;
        use glow::HasContext;
        let mut handles = [BufferHandle::null(); 2];
        for handle in handles.iter_mut() {
            let raw = unsafe {
                let buf = gl.create_buffer().expect("Failed to create particle buffer");
                gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
                gl.buffer_data_size(
                    GL_SHADER_STORAGE_BUFFER,
                    byte_size as i32,
                    BufferUsage::DynamicCopy.to_gl(),
                );
                gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
                buf
            };
            *handle = BufferHandle::new(raw.0.get(), byte_size);
            tracker.record_alloc(byte_size);
        }
        Self {
            buffers: handles,
            capacity,
            active_count: 0,
            read_index: 0,
            binding_read,
            binding_write,
            tracker,
        }
    }

    /// Swap read and write buffers (ping-pong).
    pub fn swap(&mut self) {
        self.read_index = 1 - self.read_index;
    }

    /// Bind the read buffer to `binding_read` and write buffer to `binding_write`.
    pub fn bind(&self, gl: &glow::Context) {
        use glow::HasContext;
        let read_buf = self.gl_buffer(self.read_index);
        let write_buf = self.gl_buffer(1 - self.read_index);
        unsafe {
            gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, self.binding_read, Some(read_buf));
            gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, self.binding_write, Some(write_buf));
        }
    }

    /// Upload initial particle data into the read buffer.
    pub fn upload_initial(&mut self, gl: &glow::Context, data: &[ParticleGpuData]) {
        assert!(data.len() <= self.capacity);
        self.active_count = data.len();
        let byte_slice = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * Self::PARTICLE_SIZE,
            )
        };
        use glow::HasContext;
        let buf = self.gl_buffer(self.read_index);
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.buffer_sub_data_u8_slice(GL_SHADER_STORAGE_BUFFER, 0, byte_slice);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
    }

    /// Download current read-buffer contents.
    pub fn download(&self, gl: &glow::Context) -> Vec<ParticleGpuData> {
        if self.active_count == 0 {
            return Vec::new();
        }
        let byte_count = self.active_count * Self::PARTICLE_SIZE;
        let mut bytes = vec![0u8; byte_count];
        use glow::HasContext;
        let buf = self.gl_buffer(self.read_index);
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.get_buffer_sub_data(GL_SHADER_STORAGE_BUFFER, 0, &mut bytes);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        let mut result = Vec::with_capacity(self.active_count);
        let src_ptr = bytes.as_ptr() as *const ParticleGpuData;
        for i in 0..self.active_count {
            result.push(unsafe { std::ptr::read(src_ptr.add(i)) });
        }
        result
    }

    /// Set the active particle count (usually read back from atomic counter).
    pub fn set_active_count(&mut self, count: usize) {
        self.active_count = count.min(self.capacity);
    }

    /// Get the current active count.
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Max particle capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Resize both buffers. Existing data is lost.
    pub fn resize(&mut self, gl: &glow::Context, new_capacity: usize) {
        let old_byte = self.capacity * Self::PARTICLE_SIZE;
        let new_byte = new_capacity * Self::PARTICLE_SIZE;
        use glow::HasContext;
        for i in 0..2 {
            let buf = self.gl_buffer(i);
            unsafe {
                gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
                gl.buffer_data_size(
                    GL_SHADER_STORAGE_BUFFER,
                    new_byte as i32,
                    BufferUsage::DynamicCopy.to_gl(),
                );
                gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
            }
            self.tracker.record_free(old_byte);
            self.tracker.record_alloc(new_byte);
            self.buffers[i].size_bytes = new_byte;
        }
        self.capacity = new_capacity;
        self.active_count = 0;
    }

    /// Read buffer handle (current source).
    pub fn read_handle(&self) -> BufferHandle {
        self.buffers[self.read_index]
    }

    /// Write buffer handle (current target).
    pub fn write_handle(&self) -> BufferHandle {
        self.buffers[1 - self.read_index]
    }

    /// Destroy both underlying GL buffers.
    pub fn destroy(self, gl: &glow::Context) {
        use glow::HasContext;
        for i in 0..2 {
            let byte_size = self.buffers[i].size_bytes;
            self.tracker.record_free(byte_size);
            let buf = self.gl_buffer(i);
            unsafe {
                gl.delete_buffer(buf);
            }
        }
    }

    /// Reconstruct glow NativeBuffer from handle index.
    fn gl_buffer(&self, idx: usize) -> glow::NativeBuffer {
        glow::NativeBuffer(std::num::NonZeroU32::new(self.buffers[idx].raw).unwrap())
    }
}

// ---------------------------------------------------------------------------
// AtomicCounter
// ---------------------------------------------------------------------------

/// GPU atomic counter buffer. Used for particle birth/death counting,
/// indirect dispatch argument generation, etc.
pub struct AtomicCounter {
    handle: BufferHandle,
    binding_index: u32,
    tracker: Arc<MemoryTracker>,
}

impl AtomicCounter {
    /// Size of a single u32 counter.
    const COUNTER_SIZE: usize = std::mem::size_of::<u32>();

    /// Create a new atomic counter bound to a specific binding index.
    pub fn create(
        gl: &glow::Context,
        binding_index: u32,
        tracker: Arc<MemoryTracker>,
    ) -> Self {
        use glow::HasContext;
        let raw = unsafe {
            let buf = gl
                .create_buffer()
                .expect("Failed to create atomic counter buffer");
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(buf));
            let zero = 0u32.to_le_bytes();
            gl.buffer_data_u8_slice(
                GL_ATOMIC_COUNTER_BUFFER,
                &zero,
                BufferUsage::DynamicDraw.to_gl(),
            );
            gl.bind_buffer_base(GL_ATOMIC_COUNTER_BUFFER, binding_index, Some(buf));
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
            buf
        };
        tracker.record_alloc(Self::COUNTER_SIZE);
        Self {
            handle: BufferHandle::new(raw.0.get(), Self::COUNTER_SIZE),
            binding_index,
            tracker,
        }
    }

    /// Reset the counter to zero.
    pub fn reset(&self, gl: &glow::Context) {
        use glow::HasContext;
        let zero = 0u32.to_le_bytes();
        unsafe {
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(self.gl_buffer()));
            gl.buffer_sub_data_u8_slice(GL_ATOMIC_COUNTER_BUFFER, 0, &zero);
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
        }
    }

    /// Set the counter to a specific value.
    pub fn set(&self, gl: &glow::Context, value: u32) {
        use glow::HasContext;
        let bytes = value.to_le_bytes();
        unsafe {
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(self.gl_buffer()));
            gl.buffer_sub_data_u8_slice(GL_ATOMIC_COUNTER_BUFFER, 0, &bytes);
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
        }
    }

    /// Read back the current counter value from the GPU.
    pub fn read(&self, gl: &glow::Context) -> u32 {
        use glow::HasContext;
        let mut bytes = [0u8; 4];
        unsafe {
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(self.gl_buffer()));
            gl.get_buffer_sub_data(GL_ATOMIC_COUNTER_BUFFER, 0, &mut bytes);
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
        }
        u32::from_le_bytes(bytes)
    }

    /// Bind the counter to its binding index.
    pub fn bind(&self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.bind_buffer_base(
                GL_ATOMIC_COUNTER_BUFFER,
                self.binding_index,
                Some(self.gl_buffer()),
            );
        }
    }

    /// Handle.
    pub fn handle(&self) -> BufferHandle {
        self.handle
    }

    /// Destroy the counter buffer.
    pub fn destroy(self, gl: &glow::Context) {
        self.tracker.record_free(Self::COUNTER_SIZE);
        use glow::HasContext;
        unsafe {
            gl.delete_buffer(self.gl_buffer());
        }
    }

    fn gl_buffer(&self) -> glow::NativeBuffer {
        glow::NativeBuffer(std::num::NonZeroU32::new(self.handle.raw).unwrap())
    }
}

// ---------------------------------------------------------------------------
// BufferPool
// ---------------------------------------------------------------------------

/// Key for pooled buffers: (byte_size, usage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PoolKey {
    size_bytes: usize,
    usage: BufferUsage,
}

/// A pool that recycles GPU buffers to avoid frequent allocation/deallocation.
///
/// When a buffer is "released" back to the pool it is not deleted, but stored
/// for reuse. When a buffer of the same size and usage is requested, it is
/// returned from the pool instead of allocating a new one.
pub struct BufferPool {
    available: HashMap<PoolKey, Vec<BufferHandle>>,
    in_use: HashMap<u32, PoolKey>,
    tracker: Arc<MemoryTracker>,
    max_pool_size: usize,
}

impl BufferPool {
    /// Create a new buffer pool.
    pub fn new(tracker: Arc<MemoryTracker>, max_pool_size: usize) -> Self {
        Self {
            available: HashMap::new(),
            in_use: HashMap::new(),
            tracker,
            max_pool_size,
        }
    }

    /// Acquire a buffer of at least `size_bytes` with the given usage.
    /// Returns an existing pooled buffer if one matches, otherwise allocates new.
    pub fn acquire(
        &mut self,
        gl: &glow::Context,
        size_bytes: usize,
        usage: BufferUsage,
    ) -> BufferHandle {
        let key = PoolKey { size_bytes, usage };
        // Try to find an available buffer of the right size
        if let Some(list) = self.available.get_mut(&key) {
            if let Some(handle) = list.pop() {
                self.in_use.insert(handle.raw, key);
                return handle;
            }
        }
        // Allocate new
        use glow::HasContext;
        let raw = unsafe {
            let buf = gl.create_buffer().expect("Failed to create pooled buffer");
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.buffer_data_size(GL_SHADER_STORAGE_BUFFER, size_bytes as i32, usage.to_gl());
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
            buf
        };
        self.tracker.record_alloc(size_bytes);
        let handle = BufferHandle::new(raw.0.get(), size_bytes);
        self.in_use.insert(handle.raw, key);
        handle
    }

    /// Release a buffer back to the pool for reuse.
    pub fn release(&mut self, gl: &glow::Context, handle: BufferHandle) {
        if let Some(key) = self.in_use.remove(&handle.raw) {
            let list = self.available.entry(key).or_insert_with(Vec::new);
            if list.len() < self.max_pool_size {
                list.push(handle);
            } else {
                // Pool is full for this key — actually delete
                self.tracker.record_free(handle.size_bytes);
                use glow::HasContext;
                unsafe {
                    let buf = glow::NativeBuffer(
                        std::num::NonZeroU32::new(handle.raw).unwrap(),
                    );
                    gl.delete_buffer(buf);
                }
            }
        }
    }

    /// Delete all pooled (available) buffers.
    pub fn drain(&mut self, gl: &glow::Context) {
        use glow::HasContext;
        for (_key, list) in self.available.drain() {
            for handle in list {
                self.tracker.record_free(handle.size_bytes);
                unsafe {
                    let buf = glow::NativeBuffer(
                        std::num::NonZeroU32::new(handle.raw).unwrap(),
                    );
                    gl.delete_buffer(buf);
                }
            }
        }
    }

    /// Destroy the entire pool including in-use buffers.
    /// Caller must ensure no in-use buffers are still referenced.
    pub fn destroy(mut self, gl: &glow::Context) {
        self.drain(gl);
        use glow::HasContext;
        for (raw, key) in self.in_use.drain() {
            self.tracker.record_free(key.size_bytes);
            unsafe {
                let buf = glow::NativeBuffer(std::num::NonZeroU32::new(raw).unwrap());
                gl.delete_buffer(buf);
            }
        }
    }

    /// Number of available (pooled) buffers.
    pub fn available_count(&self) -> usize {
        self.available.values().map(|v| v.len()).sum()
    }

    /// Number of buffers currently in use.
    pub fn in_use_count(&self) -> usize {
        self.in_use.len()
    }

    /// Total pooled memory in bytes (available buffers only).
    pub fn pooled_bytes(&self) -> usize {
        self.available
            .values()
            .flat_map(|v| v.iter())
            .map(|h| h.size_bytes)
            .sum()
    }
}

// ---------------------------------------------------------------------------
// MappedRange — for persistent / partial buffer updates
// ---------------------------------------------------------------------------

/// Represents a mapped sub-range of a buffer for CPU write access.
/// The mapping must be flushed and unmapped before the GPU can read.
pub struct MappedRange {
    /// Buffer that is mapped.
    pub buffer: BufferHandle,
    /// Byte offset into the buffer.
    pub offset: usize,
    /// Length in bytes of the mapped region.
    pub length: usize,
    /// Whether the range has been flushed.
    flushed: bool,
}

impl MappedRange {
    /// Map a range of a buffer for writing.
    pub fn map_write(
        gl: &glow::Context,
        buffer: BufferHandle,
        offset: usize,
        length: usize,
    ) -> Self {
        use glow::HasContext;
        let buf = glow::NativeBuffer(std::num::NonZeroU32::new(buffer.raw).unwrap());
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            // We use MapBufferRange for partial updates
            let _ptr = gl.map_buffer_range(
                GL_SHADER_STORAGE_BUFFER,
                offset as i32,
                length as i32,
                GL_MAP_WRITE_BIT,
            );
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        Self {
            buffer,
            offset,
            length,
            flushed: false,
        }
    }

    /// Map a range for reading.
    pub fn map_read(
        gl: &glow::Context,
        buffer: BufferHandle,
        offset: usize,
        length: usize,
    ) -> Self {
        use glow::HasContext;
        let buf = glow::NativeBuffer(std::num::NonZeroU32::new(buffer.raw).unwrap());
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            let _ptr = gl.map_buffer_range(
                GL_SHADER_STORAGE_BUFFER,
                offset as i32,
                length as i32,
                GL_MAP_READ_BIT,
            );
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        Self {
            buffer,
            offset,
            length,
            flushed: false,
        }
    }

    /// Flush the mapped range (call before unmap to ensure GPU sees writes).
    pub fn flush(&mut self, gl: &glow::Context) {
        use glow::HasContext;
        let buf = glow::NativeBuffer(std::num::NonZeroU32::new(self.buffer.raw).unwrap());
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.flush_mapped_buffer_range(GL_SHADER_STORAGE_BUFFER, self.offset as i32, self.length as i32);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
        self.flushed = true;
    }

    /// Unmap the buffer. Must be called after writing is complete.
    pub fn unmap(self, gl: &glow::Context) {
        use glow::HasContext;
        let buf = glow::NativeBuffer(std::num::NonZeroU32::new(self.buffer.raw).unwrap());
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.unmap_buffer(GL_SHADER_STORAGE_BUFFER);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
    }

    /// Whether flush has been called.
    pub fn is_flushed(&self) -> bool {
        self.flushed
    }
}

// ---------------------------------------------------------------------------
// BufferCopyEngine — GPU-to-GPU transfers
// ---------------------------------------------------------------------------

/// Performs GPU-side buffer-to-buffer copies without CPU readback.
pub struct BufferCopyEngine {
    /// Stats: total bytes copied in the lifetime of this engine.
    total_bytes_copied: u64,
    /// Stats: number of copy operations performed.
    copy_count: u64,
}

impl BufferCopyEngine {
    /// Create a new copy engine.
    pub fn new() -> Self {
        Self {
            total_bytes_copied: 0,
            copy_count: 0,
        }
    }

    /// Copy `length` bytes from `src` at `src_offset` to `dst` at `dst_offset`.
    /// All offsets and lengths are in bytes.
    pub fn copy(
        &mut self,
        gl: &glow::Context,
        src: BufferHandle,
        src_offset: usize,
        dst: BufferHandle,
        dst_offset: usize,
        length: usize,
    ) {
        assert!(
            src_offset + length <= src.size_bytes,
            "copy: source range out of bounds"
        );
        assert!(
            dst_offset + length <= dst.size_bytes,
            "copy: dest range out of bounds"
        );
        use glow::HasContext;
        let src_buf = glow::NativeBuffer(std::num::NonZeroU32::new(src.raw).unwrap());
        let dst_buf = glow::NativeBuffer(std::num::NonZeroU32::new(dst.raw).unwrap());
        unsafe {
            gl.bind_buffer(GL_COPY_READ_BUFFER, Some(src_buf));
            gl.bind_buffer(GL_COPY_WRITE_BUFFER, Some(dst_buf));
            gl.copy_buffer_sub_data(
                GL_COPY_READ_BUFFER,
                GL_COPY_WRITE_BUFFER,
                src_offset as i32,
                dst_offset as i32,
                length as i32,
            );
            gl.bind_buffer(GL_COPY_READ_BUFFER, None);
            gl.bind_buffer(GL_COPY_WRITE_BUFFER, None);
        }
        self.total_bytes_copied += length as u64;
        self.copy_count += 1;
    }

    /// Copy the entire source buffer into destination at offset 0.
    pub fn copy_full(
        &mut self,
        gl: &glow::Context,
        src: BufferHandle,
        dst: BufferHandle,
    ) {
        let length = src.size_bytes.min(dst.size_bytes);
        self.copy(gl, src, 0, dst, 0, length);
    }

    /// Scatter-copy: copy multiple disjoint regions in a single call.
    pub fn copy_regions(
        &mut self,
        gl: &glow::Context,
        src: BufferHandle,
        dst: BufferHandle,
        regions: &[CopyRegion],
    ) {
        for region in regions {
            self.copy(
                gl,
                src,
                region.src_offset,
                dst,
                region.dst_offset,
                region.length,
            );
        }
    }

    /// Total bytes copied across all operations.
    pub fn total_bytes_copied(&self) -> u64 {
        self.total_bytes_copied
    }

    /// Total number of copy operations.
    pub fn copy_count(&self) -> u64 {
        self.copy_count
    }

    /// Reset statistics.
    pub fn reset_stats(&mut self) {
        self.total_bytes_copied = 0;
        self.copy_count = 0;
    }
}

impl Default for BufferCopyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Describes a region to copy between two buffers.
#[derive(Debug, Clone, Copy)]
pub struct CopyRegion {
    pub src_offset: usize,
    pub dst_offset: usize,
    pub length: usize,
}

// ---------------------------------------------------------------------------
// BufferRingAllocator — frame-based ring allocation
// ---------------------------------------------------------------------------

/// A ring allocator for streaming per-frame data into a large buffer.
/// Each frame gets a slice of the ring; after N frames the oldest slice is reused.
pub struct BufferRingAllocator {
    handle: BufferHandle,
    total_size: usize,
    frame_count: usize,
    current_frame: usize,
    frame_size: usize,
    tracker: Arc<MemoryTracker>,
}

impl BufferRingAllocator {
    /// Create a ring allocator with `frame_count` slots, each of `frame_size` bytes.
    pub fn create(
        gl: &glow::Context,
        frame_count: usize,
        frame_size: usize,
        tracker: Arc<MemoryTracker>,
    ) -> Self {
        let total_size = frame_count * frame_size;
        use glow::HasContext;
        let raw = unsafe {
            let buf = gl.create_buffer().expect("Failed to create ring buffer");
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.buffer_data_size(
                GL_SHADER_STORAGE_BUFFER,
                total_size as i32,
                BufferUsage::StreamDraw.to_gl(),
            );
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
            buf
        };
        tracker.record_alloc(total_size);
        Self {
            handle: BufferHandle::new(raw.0.get(), total_size),
            total_size,
            frame_count,
            current_frame: 0,
            frame_size,
            tracker,
        }
    }

    /// Advance to the next frame slot. Returns the byte offset for this frame's data.
    pub fn advance(&mut self) -> usize {
        let offset = self.current_frame * self.frame_size;
        self.current_frame = (self.current_frame + 1) % self.frame_count;
        offset
    }

    /// Write data to the current frame slot.
    pub fn write_current(&self, gl: &glow::Context, data: &[u8]) {
        assert!(data.len() <= self.frame_size, "data exceeds frame size");
        let offset = self.current_frame * self.frame_size;
        use glow::HasContext;
        let buf = glow::NativeBuffer(std::num::NonZeroU32::new(self.handle.raw).unwrap());
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.buffer_sub_data_u8_slice(GL_SHADER_STORAGE_BUFFER, offset as i32, data);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
    }

    /// Current frame index.
    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    /// Byte offset for a given frame index.
    pub fn frame_offset(&self, frame: usize) -> usize {
        (frame % self.frame_count) * self.frame_size
    }

    /// Handle to the underlying buffer.
    pub fn handle(&self) -> BufferHandle {
        self.handle
    }

    /// Per-frame size in bytes.
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    /// Total size in bytes.
    pub fn total_size(&self) -> usize {
        self.total_size
    }

    /// Destroy the ring buffer.
    pub fn destroy(self, gl: &glow::Context) {
        self.tracker.record_free(self.total_size);
        use glow::HasContext;
        let buf = glow::NativeBuffer(std::num::NonZeroU32::new(self.handle.raw).unwrap());
        unsafe {
            gl.delete_buffer(buf);
        }
    }
}

// ---------------------------------------------------------------------------
// Utility: issue_barrier helper
// ---------------------------------------------------------------------------

/// Issue a glMemoryBarrier with the given barrier type.
pub fn issue_barrier(gl: &glow::Context, barrier: BufferBarrierType) {
    use glow::HasContext;
    unsafe {
        gl.memory_barrier(barrier.to_gl_bits());
    }
}

/// Issue a glMemoryBarrier with raw bits.
pub fn issue_barrier_raw(gl: &glow::Context, bits: u32) {
    use glow::HasContext;
    unsafe {
        gl.memory_barrier(bits);
    }
}

// ---------------------------------------------------------------------------
// BufferDebug — optional debug labeling
// ---------------------------------------------------------------------------

/// Debug utilities for labeling buffers (requires GL_KHR_debug).
pub struct BufferDebug;

impl BufferDebug {
    /// Label a buffer for graphics debugger visibility.
    pub fn label(gl: &glow::Context, handle: BufferHandle, name: &str) {
        use glow::HasContext;
        let buf = glow::NativeBuffer(std::num::NonZeroU32::new(handle.raw).unwrap());
        unsafe {
            gl.object_label(glow::BUFFER, buf.0.get(), Some(name));
        }
    }
}

// ---------------------------------------------------------------------------
// Global convenience: create a shared tracker
// ---------------------------------------------------------------------------

/// Create a new shared memory tracker (convenience wrapper).
pub fn shared_tracker() -> Arc<MemoryTracker> {
    Arc::new(MemoryTracker::new())
}

// ---------------------------------------------------------------------------
// MultiCounter — multiple atomic counters in one buffer
// ---------------------------------------------------------------------------

/// Holds multiple atomic counters in a single buffer.
pub struct MultiCounter {
    handle: BufferHandle,
    count: usize,
    binding_index: u32,
    tracker: Arc<MemoryTracker>,
}

impl MultiCounter {
    /// Create N counters in a single atomic counter buffer.
    pub fn create(
        gl: &glow::Context,
        count: usize,
        binding_index: u32,
        tracker: Arc<MemoryTracker>,
    ) -> Self {
        let byte_size = count * std::mem::size_of::<u32>();
        use glow::HasContext;
        let raw = unsafe {
            let buf = gl
                .create_buffer()
                .expect("Failed to create multi-counter buffer");
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(buf));
            let zeros = vec![0u8; byte_size];
            gl.buffer_data_u8_slice(
                GL_ATOMIC_COUNTER_BUFFER,
                &zeros,
                BufferUsage::DynamicDraw.to_gl(),
            );
            gl.bind_buffer_base(GL_ATOMIC_COUNTER_BUFFER, binding_index, Some(buf));
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
            buf
        };
        tracker.record_alloc(byte_size);
        Self {
            handle: BufferHandle::new(raw.0.get(), byte_size),
            count,
            binding_index,
            tracker,
        }
    }

    /// Reset all counters to zero.
    pub fn reset_all(&self, gl: &glow::Context) {
        let byte_size = self.count * 4;
        let zeros = vec![0u8; byte_size];
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(buf));
            gl.buffer_sub_data_u8_slice(GL_ATOMIC_COUNTER_BUFFER, 0, &zeros);
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
        }
    }

    /// Reset a single counter by index.
    pub fn reset_one(&self, gl: &glow::Context, index: usize) {
        assert!(index < self.count);
        let zero = 0u32.to_le_bytes();
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(buf));
            gl.buffer_sub_data_u8_slice(
                GL_ATOMIC_COUNTER_BUFFER,
                (index * 4) as i32,
                &zero,
            );
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
        }
    }

    /// Read all counter values.
    pub fn read_all(&self, gl: &glow::Context) -> Vec<u32> {
        let byte_size = self.count * 4;
        let mut bytes = vec![0u8; byte_size];
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(buf));
            gl.get_buffer_sub_data(GL_ATOMIC_COUNTER_BUFFER, 0, &mut bytes);
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
        }
        bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// Read a single counter value.
    pub fn read_one(&self, gl: &glow::Context, index: usize) -> u32 {
        assert!(index < self.count);
        let mut bytes = [0u8; 4];
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, Some(buf));
            gl.get_buffer_sub_data(GL_ATOMIC_COUNTER_BUFFER, (index * 4) as i32, &mut bytes);
            gl.bind_buffer(GL_ATOMIC_COUNTER_BUFFER, None);
        }
        u32::from_le_bytes(bytes)
    }

    /// Bind the multi-counter buffer.
    pub fn bind(&self, gl: &glow::Context) {
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer_base(GL_ATOMIC_COUNTER_BUFFER, self.binding_index, Some(buf));
        }
    }

    /// Number of counters.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Handle.
    pub fn handle(&self) -> BufferHandle {
        self.handle
    }

    /// Destroy.
    pub fn destroy(self, gl: &glow::Context) {
        self.tracker.record_free(self.handle.size_bytes);
        use glow::HasContext;
        unsafe {
            gl.delete_buffer(self.gl_buffer());
        }
    }

    fn gl_buffer(&self) -> glow::NativeBuffer {
        glow::NativeBuffer(std::num::NonZeroU32::new(self.handle.raw).unwrap())
    }
}

// ---------------------------------------------------------------------------
// IndirectBuffer — for indirect dispatch / draw commands
// ---------------------------------------------------------------------------

/// Indirect dispatch arguments as laid out in GPU memory.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IndirectDispatchCommand {
    pub num_groups_x: u32,
    pub num_groups_y: u32,
    pub num_groups_z: u32,
}

/// GL_DISPATCH_INDIRECT_BUFFER
const GL_DISPATCH_INDIRECT_BUFFER: u32 = 0x90EE;

/// Buffer that stores indirect dispatch commands.
pub struct IndirectBuffer {
    handle: BufferHandle,
    command_count: usize,
    tracker: Arc<MemoryTracker>,
}

impl IndirectBuffer {
    const CMD_SIZE: usize = std::mem::size_of::<IndirectDispatchCommand>();

    /// Create an indirect buffer that can hold `count` dispatch commands.
    pub fn create(
        gl: &glow::Context,
        count: usize,
        tracker: Arc<MemoryTracker>,
    ) -> Self {
        let byte_size = count * Self::CMD_SIZE;
        use glow::HasContext;
        let raw = unsafe {
            let buf = gl
                .create_buffer()
                .expect("Failed to create indirect buffer");
            gl.bind_buffer(GL_DISPATCH_INDIRECT_BUFFER, Some(buf));
            gl.buffer_data_size(
                GL_DISPATCH_INDIRECT_BUFFER,
                byte_size as i32,
                BufferUsage::DynamicDraw.to_gl(),
            );
            gl.bind_buffer(GL_DISPATCH_INDIRECT_BUFFER, None);
            buf
        };
        tracker.record_alloc(byte_size);
        Self {
            handle: BufferHandle::new(raw.0.get(), byte_size),
            command_count: count,
            tracker,
        }
    }

    /// Upload dispatch commands.
    pub fn upload(&self, gl: &glow::Context, commands: &[IndirectDispatchCommand]) {
        assert!(commands.len() <= self.command_count);
        let byte_slice = unsafe {
            std::slice::from_raw_parts(
                commands.as_ptr() as *const u8,
                commands.len() * Self::CMD_SIZE,
            )
        };
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer(GL_DISPATCH_INDIRECT_BUFFER, Some(buf));
            gl.buffer_sub_data_u8_slice(GL_DISPATCH_INDIRECT_BUFFER, 0, byte_slice);
            gl.bind_buffer(GL_DISPATCH_INDIRECT_BUFFER, None);
        }
    }

    /// Bind for indirect dispatch.
    pub fn bind(&self, gl: &glow::Context) {
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer(GL_DISPATCH_INDIRECT_BUFFER, Some(buf));
        }
    }

    /// Handle.
    pub fn handle(&self) -> BufferHandle {
        self.handle
    }

    /// Destroy.
    pub fn destroy(self, gl: &glow::Context) {
        self.tracker.record_free(self.handle.size_bytes);
        use glow::HasContext;
        unsafe {
            gl.delete_buffer(self.gl_buffer());
        }
    }

    fn gl_buffer(&self) -> glow::NativeBuffer {
        glow::NativeBuffer(std::num::NonZeroU32::new(self.handle.raw).unwrap())
    }
}

// ---------------------------------------------------------------------------
// StructuredBuffer — convenience for structured data with named fields
// ---------------------------------------------------------------------------

/// Metadata about a field within a structured buffer.
#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    /// Name for debugging / shader matching.
    pub name: String,
    /// Offset in bytes from the start of the struct.
    pub offset: usize,
    /// Size in bytes of this field.
    pub size: usize,
}

/// A buffer that tracks its internal field layout for shader interop.
pub struct StructuredBuffer {
    handle: BufferHandle,
    stride: usize,
    element_count: usize,
    fields: Vec<FieldDescriptor>,
    binding_index: u32,
    tracker: Arc<MemoryTracker>,
}

impl StructuredBuffer {
    /// Create a new structured buffer.
    pub fn create(
        gl: &glow::Context,
        stride: usize,
        capacity: usize,
        fields: Vec<FieldDescriptor>,
        binding_index: u32,
        tracker: Arc<MemoryTracker>,
    ) -> Self {
        let byte_size = stride * capacity;
        use glow::HasContext;
        let raw = unsafe {
            let buf = gl.create_buffer().expect("Failed to create structured buffer");
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.buffer_data_size(
                GL_SHADER_STORAGE_BUFFER,
                byte_size as i32,
                BufferUsage::DynamicDraw.to_gl(),
            );
            gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, binding_index, Some(buf));
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
            buf
        };
        tracker.record_alloc(byte_size);
        Self {
            handle: BufferHandle::new(raw.0.get(), byte_size),
            stride,
            element_count: capacity,
            fields,
            binding_index,
            tracker,
        }
    }

    /// Upload raw bytes into the structured buffer.
    pub fn upload_raw(&self, gl: &glow::Context, data: &[u8]) {
        assert!(data.len() <= self.handle.size_bytes);
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, Some(buf));
            gl.buffer_sub_data_u8_slice(GL_SHADER_STORAGE_BUFFER, 0, data);
            gl.bind_buffer(GL_SHADER_STORAGE_BUFFER, None);
        }
    }

    /// Bind to SSBO.
    pub fn bind(&self, gl: &glow::Context) {
        use glow::HasContext;
        let buf = self.gl_buffer();
        unsafe {
            gl.bind_buffer_base(GL_SHADER_STORAGE_BUFFER, self.binding_index, Some(buf));
        }
    }

    /// Get field descriptors.
    pub fn fields(&self) -> &[FieldDescriptor] {
        &self.fields
    }

    /// Stride per element.
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Element count (capacity).
    pub fn element_count(&self) -> usize {
        self.element_count
    }

    /// Handle.
    pub fn handle(&self) -> BufferHandle {
        self.handle
    }

    /// Destroy.
    pub fn destroy(self, gl: &glow::Context) {
        self.tracker.record_free(self.handle.size_bytes);
        use glow::HasContext;
        unsafe {
            gl.delete_buffer(self.gl_buffer());
        }
    }

    fn gl_buffer(&self) -> glow::NativeBuffer {
        glow::NativeBuffer(std::num::NonZeroU32::new(self.handle.raw).unwrap())
    }
}

// Mutex wrapper for thread safety on the pool
/// Thread-safe buffer pool.
pub struct SharedBufferPool {
    inner: Mutex<BufferPool>,
}

impl SharedBufferPool {
    /// Create a new shared pool.
    pub fn new(tracker: Arc<MemoryTracker>, max_pool_size: usize) -> Self {
        Self {
            inner: Mutex::new(BufferPool::new(tracker, max_pool_size)),
        }
    }

    /// Acquire a buffer.
    pub fn acquire(
        &self,
        gl: &glow::Context,
        size_bytes: usize,
        usage: BufferUsage,
    ) -> BufferHandle {
        self.inner.lock().unwrap().acquire(gl, size_bytes, usage)
    }

    /// Release a buffer.
    pub fn release(&self, gl: &glow::Context, handle: BufferHandle) {
        self.inner.lock().unwrap().release(gl, handle);
    }

    /// Drain the pool.
    pub fn drain(&self, gl: &glow::Context) {
        self.inner.lock().unwrap().drain(gl);
    }

    /// Get available count.
    pub fn available_count(&self) -> usize {
        self.inner.lock().unwrap().available_count()
    }

    /// Get in-use count.
    pub fn in_use_count(&self) -> usize {
        self.inner.lock().unwrap().in_use_count()
    }
}
