//! Compute shader compilation, dispatch, pipeline caching, and profiling.
//!
//! Provides the core compute dispatch pipeline:
//! - `ShaderSource` with `#define` injection
//! - `ComputeProgram` with compile/link/validate
//! - `WorkgroupSize` calculation with hardware-limit awareness
//! - `ComputeDispatch` for 1D/2D/3D and indirect dispatch
//! - `PipelineCache` for shader reuse
//! - `SpecializationConstant` for compile-time constants
//! - `ComputeProfiler` with GPU timer queries

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// GL constants
// ---------------------------------------------------------------------------

const GL_COMPUTE_SHADER: u32 = 0x91B9;
const GL_SHADER_STORAGE_BARRIER_BIT: u32 = 0x00002000;
const GL_DISPATCH_INDIRECT_BUFFER: u32 = 0x90EE;
const GL_TIME_ELAPSED: u32 = 0x88BF;
const GL_QUERY_RESULT: u32 = 0x8866;
const GL_QUERY_RESULT_AVAILABLE: u32 = 0x8867;

// ---------------------------------------------------------------------------
// ShaderSource
// ---------------------------------------------------------------------------

/// A compute shader source with support for `#define` injection and includes.
#[derive(Debug, Clone)]
pub struct ShaderSource {
    /// Base GLSL source code.
    source: String,
    /// Defines to inject after the #version line.
    defines: Vec<(String, String)>,
    /// Version string (e.g., "430").
    version: String,
    /// Optional label for debugging.
    label: Option<String>,
}

impl ShaderSource {
    /// Create a new shader source from GLSL code.
    pub fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
            defines: Vec::new(),
            version: "430".to_string(),
            label: None,
        }
    }

    /// Create with explicit version.
    pub fn with_version(source: &str, version: &str) -> Self {
        Self {
            source: source.to_string(),
            defines: Vec::new(),
            version: version.to_string(),
            label: None,
        }
    }

    /// Add a `#define NAME VALUE` to be injected.
    pub fn define(&mut self, name: &str, value: &str) -> &mut Self {
        self.defines.push((name.to_string(), value.to_string()));
        self
    }

    /// Add a `#define NAME` (flag, no value).
    pub fn define_flag(&mut self, name: &str) -> &mut Self {
        self.defines.push((name.to_string(), String::new()));
        self
    }

    /// Set the debug label.
    pub fn set_label(&mut self, label: &str) -> &mut Self {
        self.label = Some(label.to_string());
        self
    }

    /// Get the label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Produce the final GLSL source with version and defines injected.
    pub fn assemble(&self) -> String {
        let mut result = String::with_capacity(self.source.len() + 256);
        result.push_str(&format!("#version {} core\n", self.version));

        for (name, value) in &self.defines {
            if value.is_empty() {
                result.push_str(&format!("#define {}\n", name));
            } else {
                result.push_str(&format!("#define {} {}\n", name, value));
            }
        }
        result.push('\n');

        // Strip any existing #version line from the source
        for line in self.source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#version") {
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }
        result
    }

    /// Generate a cache key based on source + defines (for PipelineCache).
    pub fn cache_key(&self) -> u64 {
        // Simple FNV-1a hash
        let assembled = self.assemble();
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in assembled.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

// ---------------------------------------------------------------------------
// SpecializationConstant
// ---------------------------------------------------------------------------

/// A specialization constant that can be set at compile time.
/// In OpenGL this is simulated via `#define` injection.
#[derive(Debug, Clone)]
pub struct SpecializationConstant {
    /// Constant name (becomes a #define).
    pub name: String,
    /// Value as string (will be injected as-is).
    pub value: String,
    /// Constant ID (for Vulkan compatibility tracking).
    pub id: u32,
}

impl SpecializationConstant {
    /// Create a new integer specialization constant.
    pub fn int(name: &str, id: u32, value: i32) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
            id,
        }
    }

    /// Create a new unsigned integer specialization constant.
    pub fn uint(name: &str, id: u32, value: u32) -> Self {
        Self {
            name: name.to_string(),
            value: format!("{}u", value),
            id,
        }
    }

    /// Create a new float specialization constant.
    pub fn float(name: &str, id: u32, value: f32) -> Self {
        Self {
            name: name.to_string(),
            value: format!("{:.8}", value),
            id,
        }
    }

    /// Create a boolean specialization constant (0 or 1).
    pub fn boolean(name: &str, id: u32, value: bool) -> Self {
        Self {
            name: name.to_string(),
            value: if value { "1".to_string() } else { "0".to_string() },
            id,
        }
    }

    /// Apply this constant to a shader source as a define.
    pub fn apply(&self, source: &mut ShaderSource) {
        source.define(&self.name, &self.value);
    }
}

/// Apply a set of specialization constants to a shader source.
pub fn apply_specializations(source: &mut ShaderSource, constants: &[SpecializationConstant]) {
    for c in constants {
        c.apply(source);
    }
}

// ---------------------------------------------------------------------------
// ComputeProgram
// ---------------------------------------------------------------------------

/// A compiled and linked compute shader program.
pub struct ComputeProgram {
    /// GL program object.
    program: glow::NativeProgram,
    /// Cache key for lookup.
    cache_key: u64,
    /// Local workgroup size declared in the shader.
    local_size: [u32; 3],
    /// Debug label.
    label: Option<String>,
}

impl ComputeProgram {
    /// Compile and link a compute shader from source.
    pub fn compile(
        gl: &glow::Context,
        source: &ShaderSource,
    ) -> Result<Self, String> {
        use glow::HasContext;
        let assembled = source.assemble();
        let cache_key = source.cache_key();

        unsafe {
            let shader = gl
                .create_shader(GL_COMPUTE_SHADER)
                .map_err(|e| format!("Failed to create compute shader: {}", e))?;

            gl.shader_source(shader, &assembled);
            gl.compile_shader(shader);

            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                return Err(format!("Compute shader compilation failed:\n{}", log));
            }

            let program = gl
                .create_program()
                .map_err(|e| format!("Failed to create program: {}", e))?;

            gl.attach_shader(program, shader);
            gl.link_program(program);

            if !gl.get_program_link_status(program) {
                let log = gl.get_program_info_log(program);
                gl.delete_program(program);
                gl.delete_shader(shader);
                return Err(format!("Compute program link failed:\n{}", log));
            }

            gl.detach_shader(program, shader);
            gl.delete_shader(shader);

            // Query local workgroup size
            let local_size = Self::query_work_group_size(gl, program);

            Ok(Self {
                program,
                cache_key,
                local_size,
                label: source.label().map(|s| s.to_string()),
            })
        }
    }

    /// Compile with specialization constants applied.
    pub fn compile_specialized(
        gl: &glow::Context,
        source: &ShaderSource,
        constants: &[SpecializationConstant],
    ) -> Result<Self, String> {
        let mut src = source.clone();
        apply_specializations(&mut src, constants);
        Self::compile(gl, &src)
    }

    /// Validate the program by checking the link status and info log.
    pub fn validate(&self, gl: &glow::Context) -> Result<(), String> {
        use glow::HasContext;
        unsafe {
            if !gl.get_program_link_status(self.program) {
                let log = gl.get_program_info_log(self.program);
                return Err(format!("Program validation failed:\n{}", log));
            }
        }
        Ok(())
    }

    /// Use (bind) this program.
    pub fn bind(&self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.use_program(Some(self.program));
        }
    }

    /// Unbind the current program.
    pub fn unbind(&self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.use_program(None);
        }
    }

    /// Set a uniform int value.
    pub fn set_uniform_int(&self, gl: &glow::Context, name: &str, value: i32) {
        use glow::HasContext;
        unsafe {
            let loc = gl.get_uniform_location(self.program, name);
            if let Some(loc) = loc {
                gl.uniform_1_i32(Some(&loc), value);
            }
        }
    }

    /// Set a uniform uint value.
    pub fn set_uniform_uint(&self, gl: &glow::Context, name: &str, value: u32) {
        use glow::HasContext;
        unsafe {
            let loc = gl.get_uniform_location(self.program, name);
            if let Some(loc) = loc {
                gl.uniform_1_u32(Some(&loc), value);
            }
        }
    }

    /// Set a uniform float value.
    pub fn set_uniform_float(&self, gl: &glow::Context, name: &str, value: f32) {
        use glow::HasContext;
        unsafe {
            let loc = gl.get_uniform_location(self.program, name);
            if let Some(loc) = loc {
                gl.uniform_1_f32(Some(&loc), value);
            }
        }
    }

    /// Set a uniform vec2.
    pub fn set_uniform_vec2(&self, gl: &glow::Context, name: &str, x: f32, y: f32) {
        use glow::HasContext;
        unsafe {
            let loc = gl.get_uniform_location(self.program, name);
            if let Some(loc) = loc {
                gl.uniform_2_f32(Some(&loc), x, y);
            }
        }
    }

    /// Set a uniform vec3.
    pub fn set_uniform_vec3(&self, gl: &glow::Context, name: &str, x: f32, y: f32, z: f32) {
        use glow::HasContext;
        unsafe {
            let loc = gl.get_uniform_location(self.program, name);
            if let Some(loc) = loc {
                gl.uniform_3_f32(Some(&loc), x, y, z);
            }
        }
    }

    /// Set a uniform vec4.
    pub fn set_uniform_vec4(
        &self,
        gl: &glow::Context,
        name: &str,
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    ) {
        use glow::HasContext;
        unsafe {
            let loc = gl.get_uniform_location(self.program, name);
            if let Some(loc) = loc {
                gl.uniform_4_f32(Some(&loc), x, y, z, w);
            }
        }
    }

    /// Set a uniform mat4 (column-major).
    pub fn set_uniform_mat4(&self, gl: &glow::Context, name: &str, data: &[f32; 16]) {
        use glow::HasContext;
        unsafe {
            let loc = gl.get_uniform_location(self.program, name);
            if let Some(loc) = loc {
                gl.uniform_matrix_4_f32_slice(Some(&loc), false, data);
            }
        }
    }

    /// Get the local workgroup size declared in the shader.
    pub fn local_size(&self) -> [u32; 3] {
        self.local_size
    }

    /// Get the cache key.
    pub fn cache_key(&self) -> u64 {
        self.cache_key
    }

    /// Get the label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Get the raw GL program.
    pub fn raw_program(&self) -> glow::NativeProgram {
        self.program
    }

    /// Destroy the program.
    pub fn destroy(self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.delete_program(self.program);
        }
    }

    /// Query the work group size from a linked compute program.
    /// Falls back to [64, 1, 1] since glow does not directly expose
    /// glGetProgramiv(GL_COMPUTE_WORK_GROUP_SIZE).
    fn query_work_group_size(_gl: &glow::Context, _program: glow::NativeProgram) -> [u32; 3] {
        // glow's abstraction does not expose glGetProgramiv for
        // GL_COMPUTE_WORK_GROUP_SIZE directly. We use a sensible default
        // and let callers override via WorkgroupSize.
        [64, 1, 1]
    }
}

// ---------------------------------------------------------------------------
// WorkgroupSize
// ---------------------------------------------------------------------------

/// Computes optimal workgroup sizes for dispatch, respecting hardware limits.
#[derive(Debug, Clone, Copy)]
pub struct WorkgroupSize {
    /// Local size X.
    pub x: u32,
    /// Local size Y.
    pub y: u32,
    /// Local size Z.
    pub z: u32,
}

impl WorkgroupSize {
    /// Create a 1D workgroup size.
    pub fn new_1d(x: u32) -> Self {
        Self { x, y: 1, z: 1 }
    }

    /// Create a 2D workgroup size.
    pub fn new_2d(x: u32, y: u32) -> Self {
        Self { x, y, z: 1 }
    }

    /// Create a 3D workgroup size.
    pub fn new_3d(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    /// Total number of invocations per workgroup.
    pub fn total_invocations(&self) -> u32 {
        self.x * self.y * self.z
    }

    /// Auto-fit a 1D workgroup to a total element count, clamped to hardware max.
    pub fn auto_fit_1d(total_elements: u32, max_invocations: u32) -> Self {
        let size = total_elements.min(max_invocations).max(1);
        // Round down to nearest power of 2 for efficiency
        let size = Self::round_down_pow2(size);
        Self::new_1d(size)
    }

    /// Auto-fit a 2D workgroup to a width x height, clamped to limits.
    pub fn auto_fit_2d(width: u32, height: u32, max_invocations: u32) -> Self {
        let mut sx = 8u32;
        let mut sy = 8u32;
        while sx * sy > max_invocations {
            if sx > sy {
                sx /= 2;
            } else {
                sy /= 2;
            }
        }
        sx = sx.min(width).max(1);
        sy = sy.min(height).max(1);
        Self::new_2d(sx, sy)
    }

    /// Compute the number of workgroups needed to cover `total` elements in 1D.
    pub fn dispatch_count_1d(&self, total: u32) -> u32 {
        (total + self.x - 1) / self.x
    }

    /// Compute the number of workgroups needed to cover (width, height) in 2D.
    pub fn dispatch_count_2d(&self, width: u32, height: u32) -> (u32, u32) {
        ((width + self.x - 1) / self.x, (height + self.y - 1) / self.y)
    }

    /// Compute dispatch counts for 3D.
    pub fn dispatch_count_3d(&self, w: u32, h: u32, d: u32) -> (u32, u32, u32) {
        (
            (w + self.x - 1) / self.x,
            (h + self.y - 1) / self.y,
            (d + self.z - 1) / self.z,
        )
    }

    /// Round down to nearest power of 2.
    fn round_down_pow2(v: u32) -> u32 {
        if v == 0 {
            return 1;
        }
        let mut r = v;
        r |= r >> 1;
        r |= r >> 2;
        r |= r >> 4;
        r |= r >> 8;
        r |= r >> 16;
        (r >> 1) + 1
    }

    /// Query hardware limits from GL context.
    pub fn query_limits(gl: &glow::Context) -> WorkgroupLimits {
        use glow::HasContext;
        unsafe {
            let max_invocations = gl.get_parameter_i32(0x90EB) as u32; // GL_MAX_COMPUTE_WORK_GROUP_INVOCATIONS
            let max_x = gl.get_parameter_indexed_i32(0x91BE, 0) as u32; // GL_MAX_COMPUTE_WORK_GROUP_SIZE
            let max_y = gl.get_parameter_indexed_i32(0x91BE, 1) as u32;
            let max_z = gl.get_parameter_indexed_i32(0x91BE, 2) as u32;
            let max_count_x = gl.get_parameter_indexed_i32(0x91BF, 0) as u32; // GL_MAX_COMPUTE_WORK_GROUP_COUNT
            let max_count_y = gl.get_parameter_indexed_i32(0x91BF, 1) as u32;
            let max_count_z = gl.get_parameter_indexed_i32(0x91BF, 2) as u32;
            let max_shared = gl.get_parameter_i32(0x8262) as u32; // GL_MAX_COMPUTE_SHARED_MEMORY_SIZE
            WorkgroupLimits {
                max_invocations,
                max_size: [max_x, max_y, max_z],
                max_count: [max_count_x, max_count_y, max_count_z],
                max_shared_memory: max_shared,
            }
        }
    }
}

/// Hardware workgroup limits queried from the GL context.
#[derive(Debug, Clone, Copy)]
pub struct WorkgroupLimits {
    /// Maximum total invocations per workgroup.
    pub max_invocations: u32,
    /// Maximum local_size in each dimension.
    pub max_size: [u32; 3],
    /// Maximum dispatch count in each dimension.
    pub max_count: [u32; 3],
    /// Maximum shared memory in bytes.
    pub max_shared_memory: u32,
}

impl Default for WorkgroupLimits {
    fn default() -> Self {
        Self {
            max_invocations: 1024,
            max_size: [1024, 1024, 64],
            max_count: [65535, 65535, 65535],
            max_shared_memory: 49152,
        }
    }
}

// ---------------------------------------------------------------------------
// DispatchDimension
// ---------------------------------------------------------------------------

/// Describes the dispatch dimensionality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchDimension {
    /// 1D dispatch: (groups_x, 1, 1).
    D1(u32),
    /// 2D dispatch: (groups_x, groups_y, 1).
    D2(u32, u32),
    /// 3D dispatch: (groups_x, groups_y, groups_z).
    D3(u32, u32, u32),
}

impl DispatchDimension {
    /// Total number of workgroups.
    pub fn total_groups(&self) -> u64 {
        match *self {
            DispatchDimension::D1(x) => x as u64,
            DispatchDimension::D2(x, y) => x as u64 * y as u64,
            DispatchDimension::D3(x, y, z) => x as u64 * y as u64 * z as u64,
        }
    }

    /// Unpack to (x, y, z).
    pub fn as_tuple(&self) -> (u32, u32, u32) {
        match *self {
            DispatchDimension::D1(x) => (x, 1, 1),
            DispatchDimension::D2(x, y) => (x, y, 1),
            DispatchDimension::D3(x, y, z) => (x, y, z),
        }
    }
}

// ---------------------------------------------------------------------------
// IndirectDispatchArgs
// ---------------------------------------------------------------------------

/// Arguments for an indirect dispatch (read from a buffer on the GPU).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IndirectDispatchArgs {
    pub num_groups_x: u32,
    pub num_groups_y: u32,
    pub num_groups_z: u32,
}

impl IndirectDispatchArgs {
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        Self {
            num_groups_x: x,
            num_groups_y: y,
            num_groups_z: z,
        }
    }
}

// ---------------------------------------------------------------------------
// ComputeDispatch
// ---------------------------------------------------------------------------

/// Executes compute shader dispatches.
pub struct ComputeDispatch {
    /// Default barrier bits to issue after each dispatch.
    default_barrier: u32,
    /// Whether to automatically issue a barrier after dispatch.
    auto_barrier: bool,
}

impl ComputeDispatch {
    /// Create a new dispatcher.
    pub fn new() -> Self {
        Self {
            default_barrier: GL_SHADER_STORAGE_BARRIER_BIT,
            auto_barrier: true,
        }
    }

    /// Set whether barriers are automatically issued after dispatch.
    pub fn set_auto_barrier(&mut self, enabled: bool) {
        self.auto_barrier = enabled;
    }

    /// Set the default barrier bits.
    pub fn set_default_barrier(&mut self, bits: u32) {
        self.default_barrier = bits;
    }

    /// Dispatch a compute shader with the given program and dimensions.
    pub fn dispatch(
        &self,
        gl: &glow::Context,
        program: &ComputeProgram,
        dim: DispatchDimension,
    ) {
        use glow::HasContext;
        program.bind(gl);
        let (x, y, z) = dim.as_tuple();
        unsafe {
            gl.dispatch_compute(x, y, z);
        }
        if self.auto_barrier {
            unsafe {
                gl.memory_barrier(self.default_barrier);
            }
        }
    }

    /// Dispatch 1D: convenience for dispatching over N elements.
    pub fn dispatch_1d(
        &self,
        gl: &glow::Context,
        program: &ComputeProgram,
        total_elements: u32,
        local_size_x: u32,
    ) {
        let groups = (total_elements + local_size_x - 1) / local_size_x;
        self.dispatch(gl, program, DispatchDimension::D1(groups));
    }

    /// Dispatch 2D: convenience for dispatching over a width x height grid.
    pub fn dispatch_2d(
        &self,
        gl: &glow::Context,
        program: &ComputeProgram,
        width: u32,
        height: u32,
        local_size: WorkgroupSize,
    ) {
        let gx = (width + local_size.x - 1) / local_size.x;
        let gy = (height + local_size.y - 1) / local_size.y;
        self.dispatch(gl, program, DispatchDimension::D2(gx, gy));
    }

    /// Dispatch 3D.
    pub fn dispatch_3d(
        &self,
        gl: &glow::Context,
        program: &ComputeProgram,
        w: u32,
        h: u32,
        d: u32,
        local_size: WorkgroupSize,
    ) {
        let gx = (w + local_size.x - 1) / local_size.x;
        let gy = (h + local_size.y - 1) / local_size.y;
        let gz = (d + local_size.z - 1) / local_size.z;
        self.dispatch(gl, program, DispatchDimension::D3(gx, gy, gz));
    }

    /// Indirect dispatch: read dispatch arguments from a buffer on the GPU.
    pub fn dispatch_indirect(
        &self,
        gl: &glow::Context,
        program: &ComputeProgram,
        buffer: super::buffer::BufferHandle,
        offset: usize,
    ) {
        use glow::HasContext;
        program.bind(gl);
        let buf = glow::NativeBuffer(std::num::NonZeroU32::new(buffer.raw).unwrap());
        unsafe {
            gl.bind_buffer(GL_DISPATCH_INDIRECT_BUFFER, Some(buf));
            gl.dispatch_compute_indirect(offset as i32);
            gl.bind_buffer(GL_DISPATCH_INDIRECT_BUFFER, None);
        }
        if self.auto_barrier {
            unsafe {
                gl.memory_barrier(self.default_barrier);
            }
        }
    }

    /// Dispatch with an explicit barrier type (overrides auto).
    pub fn dispatch_with_barrier(
        &self,
        gl: &glow::Context,
        program: &ComputeProgram,
        dim: DispatchDimension,
        barrier: super::buffer::BufferBarrierType,
    ) {
        use glow::HasContext;
        program.bind(gl);
        let (x, y, z) = dim.as_tuple();
        unsafe {
            gl.dispatch_compute(x, y, z);
            gl.memory_barrier(barrier.to_gl_bits());
        }
    }

    /// Dispatch multiple passes of the same program with different dimensions.
    pub fn dispatch_multi(
        &self,
        gl: &glow::Context,
        program: &ComputeProgram,
        dims: &[DispatchDimension],
    ) {
        use glow::HasContext;
        program.bind(gl);
        for dim in dims {
            let (x, y, z) = dim.as_tuple();
            unsafe {
                gl.dispatch_compute(x, y, z);
                if self.auto_barrier {
                    gl.memory_barrier(self.default_barrier);
                }
            }
        }
    }
}

impl Default for ComputeDispatch {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PipelineCache
// ---------------------------------------------------------------------------

/// Caches compiled compute programs by their source hash.
pub struct PipelineCache {
    /// Internal cache map, keyed by source hash.
    pub(crate) cache: HashMap<u64, ComputeProgram>,
}

impl PipelineCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Get or compile a program. If already cached, returns a reference.
    pub fn get_or_compile(
        &mut self,
        gl: &glow::Context,
        source: &ShaderSource,
    ) -> Result<&ComputeProgram, String> {
        let key = source.cache_key();
        if !self.cache.contains_key(&key) {
            let program = ComputeProgram::compile(gl, source)?;
            self.cache.insert(key, program);
        }
        Ok(self.cache.get(&key).unwrap())
    }

    /// Get or compile with specialization constants.
    pub fn get_or_compile_specialized(
        &mut self,
        gl: &glow::Context,
        source: &ShaderSource,
        constants: &[SpecializationConstant],
    ) -> Result<&ComputeProgram, String> {
        let mut src = source.clone();
        apply_specializations(&mut src, constants);
        let key = src.cache_key();
        if !self.cache.contains_key(&key) {
            let program = ComputeProgram::compile(gl, &src)?;
            self.cache.insert(key, program);
        }
        Ok(self.cache.get(&key).unwrap())
    }

    /// Check if a program is cached.
    pub fn contains(&self, source: &ShaderSource) -> bool {
        self.cache.contains_key(&source.cache_key())
    }

    /// Number of cached programs.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Evict a specific entry.
    pub fn evict(&mut self, gl: &glow::Context, source: &ShaderSource) {
        let key = source.cache_key();
        if let Some(prog) = self.cache.remove(&key) {
            prog.destroy(gl);
        }
    }

    /// Clear the entire cache, deleting all programs.
    pub fn clear(&mut self, gl: &glow::Context) {
        for (_key, prog) in self.cache.drain() {
            prog.destroy(gl);
        }
    }

    /// Destroy the cache.
    pub fn destroy(mut self, gl: &glow::Context) {
        self.clear(gl);
    }
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TimingQuery
// ---------------------------------------------------------------------------

/// A GPU timer query for measuring dispatch duration.
pub struct TimingQuery {
    query: glow::NativeQuery,
    active: bool,
    last_result_ns: u64,
}

impl TimingQuery {
    /// Create a new timing query.
    pub fn create(gl: &glow::Context) -> Self {
        use glow::HasContext;
        let query = unsafe {
            gl.create_query().expect("Failed to create timer query")
        };
        Self {
            query,
            active: false,
            last_result_ns: 0,
        }
    }

    /// Begin the timer query.
    pub fn begin(&mut self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.begin_query(GL_TIME_ELAPSED, self.query);
        }
        self.active = true;
    }

    /// End the timer query.
    pub fn end(&mut self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.end_query(GL_TIME_ELAPSED);
        }
        self.active = false;
    }

    /// Check if the result is available (non-blocking).
    pub fn is_available(&self, gl: &glow::Context) -> bool {
        use glow::HasContext;
        unsafe {
            let available = gl.get_query_parameter_u32(self.query, GL_QUERY_RESULT_AVAILABLE);
            available != 0
        }
    }

    /// Retrieve the elapsed time in nanoseconds (blocks until available).
    pub fn result_ns(&mut self, gl: &glow::Context) -> u64 {
        use glow::HasContext;
        let ns = unsafe { gl.get_query_parameter_u32(self.query, GL_QUERY_RESULT) };
        self.last_result_ns = ns as u64;
        self.last_result_ns
    }

    /// Get the last retrieved result without re-querying.
    pub fn last_result_ns(&self) -> u64 {
        self.last_result_ns
    }

    /// Last result in milliseconds.
    pub fn last_result_ms(&self) -> f64 {
        self.last_result_ns as f64 / 1_000_000.0
    }

    /// Whether a query is currently active (between begin/end).
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Destroy the query.
    pub fn destroy(self, gl: &glow::Context) {
        use glow::HasContext;
        unsafe {
            gl.delete_query(self.query);
        }
    }
}

// ---------------------------------------------------------------------------
// ComputeProfiler
// ---------------------------------------------------------------------------

/// Profiles compute dispatches with per-dispatch GPU timing.
pub struct ComputeProfiler {
    /// Named timing queries.
    queries: HashMap<String, TimingQuery>,
    /// Whether profiling is enabled.
    enabled: bool,
    /// History of frame timings (dispatch_name -> Vec of ms values).
    history: HashMap<String, Vec<f64>>,
    /// Maximum history length per dispatch.
    max_history: usize,
}

impl ComputeProfiler {
    /// Create a new profiler.
    pub fn new(max_history: usize) -> Self {
        Self {
            queries: HashMap::new(),
            enabled: true,
            history: HashMap::new(),
            max_history,
        }
    }

    /// Enable or disable profiling.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether profiling is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Begin timing a named dispatch.
    pub fn begin(&mut self, gl: &glow::Context, name: &str) {
        if !self.enabled {
            return;
        }
        if !self.queries.contains_key(name) {
            self.queries
                .insert(name.to_string(), TimingQuery::create(gl));
        }
        if let Some(q) = self.queries.get_mut(name) {
            q.begin(gl);
        }
    }

    /// End timing a named dispatch.
    pub fn end(&mut self, gl: &glow::Context, name: &str) {
        if !self.enabled {
            return;
        }
        if let Some(q) = self.queries.get_mut(name) {
            q.end(gl);
        }
    }

    /// Collect results for all completed queries.
    pub fn collect_results(&mut self, gl: &glow::Context) {
        if !self.enabled {
            return;
        }
        let names: Vec<String> = self.queries.keys().cloned().collect();
        for name in names {
            if let Some(q) = self.queries.get_mut(&name) {
                if !q.is_active() && q.is_available(gl) {
                    let ns = q.result_ns(gl);
                    let ms = ns as f64 / 1_000_000.0;
                    let hist = self.history.entry(name).or_insert_with(Vec::new);
                    hist.push(ms);
                    if hist.len() > self.max_history {
                        hist.remove(0);
                    }
                }
            }
        }
    }

    /// Get the last timing for a named dispatch in milliseconds.
    pub fn last_ms(&self, name: &str) -> Option<f64> {
        self.queries.get(name).map(|q| q.last_result_ms())
    }

    /// Get the average timing for a named dispatch over the history window.
    pub fn average_ms(&self, name: &str) -> Option<f64> {
        self.history.get(name).and_then(|h| {
            if h.is_empty() {
                None
            } else {
                Some(h.iter().sum::<f64>() / h.len() as f64)
            }
        })
    }

    /// Get the min/max timing for a named dispatch.
    pub fn min_max_ms(&self, name: &str) -> Option<(f64, f64)> {
        self.history.get(name).and_then(|h| {
            if h.is_empty() {
                None
            } else {
                let min = h.iter().cloned().fold(f64::MAX, f64::min);
                let max = h.iter().cloned().fold(f64::MIN, f64::max);
                Some((min, max))
            }
        })
    }

    /// Get all dispatch names that have been profiled.
    pub fn dispatch_names(&self) -> Vec<&str> {
        self.queries.keys().map(|s| s.as_str()).collect()
    }

    /// Print a summary of all profiled dispatches.
    pub fn summary(&self) -> String {
        let mut s = String::from("=== Compute Profiler Summary ===\n");
        let mut names: Vec<&str> = self.dispatch_names();
        names.sort();
        for name in names {
            let avg = self.average_ms(name).unwrap_or(0.0);
            let (min, max) = self.min_max_ms(name).unwrap_or((0.0, 0.0));
            let last = self.last_ms(name).unwrap_or(0.0);
            s.push_str(&format!(
                "  {}: last={:.3}ms avg={:.3}ms min={:.3}ms max={:.3}ms\n",
                name, last, avg, min, max
            ));
        }
        s
    }

    /// Reset all history.
    pub fn reset_history(&mut self) {
        self.history.clear();
    }

    /// Destroy all queries.
    pub fn destroy(self, gl: &glow::Context) {
        for (_name, query) in self.queries {
            query.destroy(gl);
        }
    }
}

// ---------------------------------------------------------------------------
// PipelineState — immutable snapshot for caching dispatch configurations
// ---------------------------------------------------------------------------

/// Snapshot of the state needed for a compute dispatch.
#[derive(Debug, Clone)]
pub struct PipelineState {
    /// Program cache key.
    pub program_key: u64,
    /// Dispatch dimension.
    pub dimension: DispatchDimension,
    /// Barrier bits to issue after dispatch.
    pub barrier_bits: u32,
    /// SSBO bindings: (binding_index, buffer_raw_id).
    pub ssbo_bindings: Vec<(u32, u32)>,
    /// Uniform values.
    pub uniforms: Vec<UniformValue>,
}

/// A uniform value to set before dispatch.
#[derive(Debug, Clone)]
pub enum UniformValue {
    Int(String, i32),
    Uint(String, u32),
    Float(String, f32),
    Vec2(String, f32, f32),
    Vec3(String, f32, f32, f32),
    Vec4(String, f32, f32, f32, f32),
}

impl PipelineState {
    /// Create a new pipeline state.
    pub fn new(program_key: u64, dimension: DispatchDimension) -> Self {
        Self {
            program_key,
            dimension,
            barrier_bits: GL_SHADER_STORAGE_BARRIER_BIT,
            ssbo_bindings: Vec::new(),
            uniforms: Vec::new(),
        }
    }

    /// Add an SSBO binding.
    pub fn bind_ssbo(&mut self, binding: u32, buffer_raw: u32) -> &mut Self {
        self.ssbo_bindings.push((binding, buffer_raw));
        self
    }

    /// Add a uniform.
    pub fn set_uniform(&mut self, value: UniformValue) -> &mut Self {
        self.uniforms.push(value);
        self
    }

    /// Set barrier bits.
    pub fn set_barrier(&mut self, bits: u32) -> &mut Self {
        self.barrier_bits = bits;
        self
    }

    /// Execute this pipeline state: bind SSBOs, set uniforms, dispatch.
    pub fn execute(
        &self,
        gl: &glow::Context,
        cache: &PipelineCache,
    ) {
        use glow::HasContext;
        // Find program in cache
        let program = match cache.cache.get(&self.program_key) {
            Some(p) => p,
            None => return, // Program not found
        };

        program.bind(gl);

        // Bind SSBOs
        for &(binding, raw) in &self.ssbo_bindings {
            if let Some(nz) = std::num::NonZeroU32::new(raw) {
                let buf = glow::NativeBuffer(nz);
                unsafe {
                    gl.bind_buffer_base(0x90D2, binding, Some(buf)); // GL_SHADER_STORAGE_BUFFER
                }
            }
        }

        // Set uniforms
        for u in &self.uniforms {
            match u {
                UniformValue::Int(name, v) => program.set_uniform_int(gl, name, *v),
                UniformValue::Uint(name, v) => program.set_uniform_uint(gl, name, *v),
                UniformValue::Float(name, v) => program.set_uniform_float(gl, name, *v),
                UniformValue::Vec2(name, x, y) => program.set_uniform_vec2(gl, name, *x, *y),
                UniformValue::Vec3(name, x, y, z) => {
                    program.set_uniform_vec3(gl, name, *x, *y, *z)
                }
                UniformValue::Vec4(name, x, y, z, w) => {
                    program.set_uniform_vec4(gl, name, *x, *y, *z, *w)
                }
            }
        }

        // Dispatch
        let (gx, gy, gz) = self.dimension.as_tuple();
        unsafe {
            gl.dispatch_compute(gx, gy, gz);
            gl.memory_barrier(self.barrier_bits);
        }
    }
}

// ---------------------------------------------------------------------------
// ComputeChain — chain multiple dispatches
// ---------------------------------------------------------------------------

/// A chain of compute dispatches to be executed in sequence with barriers between them.
pub struct ComputeChain {
    steps: Vec<ChainStep>,
}

/// A single step in a compute chain.
pub struct ChainStep {
    /// Program to dispatch.
    pub program_key: u64,
    /// Dispatch dimensions.
    pub dimension: DispatchDimension,
    /// Uniforms to set for this step.
    pub uniforms: Vec<UniformValue>,
    /// Barrier bits after this step (0 = no barrier).
    pub barrier_bits: u32,
}

impl ComputeChain {
    /// Create a new empty chain.
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Add a step.
    pub fn add_step(&mut self, step: ChainStep) -> &mut Self {
        self.steps.push(step);
        self
    }

    /// Number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Execute the entire chain.
    pub fn execute(&self, gl: &glow::Context, cache: &PipelineCache) {
        use glow::HasContext;
        for step in &self.steps {
            if let Some(program) = cache.cache.get(&step.program_key) {
                program.bind(gl);
                for u in &step.uniforms {
                    match u {
                        UniformValue::Int(name, v) => program.set_uniform_int(gl, name, *v),
                        UniformValue::Uint(name, v) => program.set_uniform_uint(gl, name, *v),
                        UniformValue::Float(name, v) => program.set_uniform_float(gl, name, *v),
                        UniformValue::Vec2(name, x, y) => {
                            program.set_uniform_vec2(gl, name, *x, *y)
                        }
                        UniformValue::Vec3(name, x, y, z) => {
                            program.set_uniform_vec3(gl, name, *x, *y, *z)
                        }
                        UniformValue::Vec4(name, x, y, z, w) => {
                            program.set_uniform_vec4(gl, name, *x, *y, *z, *w)
                        }
                    }
                }
                let (gx, gy, gz) = step.dimension.as_tuple();
                unsafe {
                    gl.dispatch_compute(gx, gy, gz);
                    if step.barrier_bits != 0 {
                        gl.memory_barrier(step.barrier_bits);
                    }
                }
            }
        }
    }
}

impl Default for ComputeChain {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ShaderPreprocessor — handle #include directives
// ---------------------------------------------------------------------------

/// Simple shader preprocessor that resolves `#include "name"` directives
/// from a registered library of snippets.
pub struct ShaderPreprocessor {
    snippets: HashMap<String, String>,
}

impl ShaderPreprocessor {
    /// Create a new preprocessor.
    pub fn new() -> Self {
        Self {
            snippets: HashMap::new(),
        }
    }

    /// Register a named snippet.
    pub fn register(&mut self, name: &str, source: &str) {
        self.snippets.insert(name.to_string(), source.to_string());
    }

    /// Process a shader source, resolving #include directives.
    pub fn process(&self, source: &str) -> String {
        let mut result = String::with_capacity(source.len());
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#include") {
                // Extract the name between quotes
                if let Some(start) = trimmed.find('"') {
                    if let Some(end) = trimmed[start + 1..].find('"') {
                        let name = &trimmed[start + 1..start + 1 + end];
                        if let Some(snippet) = self.snippets.get(name) {
                            result.push_str(snippet);
                            result.push('\n');
                            continue;
                        }
                    }
                }
                // Include not resolved — keep the line as a comment
                result.push_str("// UNRESOLVED: ");
                result.push_str(line);
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }
        result
    }
}

impl Default for ShaderPreprocessor {
    fn default() -> Self {
        Self::new()
    }
}
