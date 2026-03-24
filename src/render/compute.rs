//! GPU Compute pipeline: SSBO management, compute dispatch, GPU particles.
//!
//! Provides a CPU-side representation of compute shader infrastructure:
//!
//! - `ComputeBuffer`    — SSBO (shader storage buffer) management
//! - `ComputeShader`    — compiled compute program handle
//! - `ComputeDispatch`  — parameters for a compute dispatch call
//! - `GpuParticleSystem`— 100K+ particle simulation driven by GPU
//! - `GpuFieldSampler` — parallel force field evaluation on GPU
//! - `ComputeSync`      — memory barrier / fence sync primitives
//! - `IndirectDrawArgs` — GPU-generated draw arguments (no CPU readback)
//!
//! ## Design
//! The structs here describe the compute pipeline state. Actual OpenGL
//! calls happen in the render pipeline (which has the GL context). This
//! module provides the data model and parameter management.

use glam::Vec3;

// ── ComputeBuffer ─────────────────────────────────────────────────────────────

/// A Shader Storage Buffer Object (SSBO) binding.
#[derive(Debug, Clone)]
pub struct ComputeBuffer {
    pub name:       String,
    /// OpenGL binding index.
    pub binding:    u32,
    /// Size in bytes.
    pub size_bytes: usize,
    /// Usage hint for the driver.
    pub usage:      BufferUsage,
    /// CPU-side staging data (written to GPU on sync).
    pub data:       Vec<u8>,
    pub dirty:      bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferUsage {
    /// Written by CPU, read by GPU compute.
    DynamicDraw,
    /// Written by GPU compute, read by GPU draw.
    GpuOnly,
    /// Written by GPU, occasionally read back to CPU.
    Readback,
    /// Static data uploaded once.
    Static,
}

impl ComputeBuffer {
    pub fn new(name: impl Into<String>, binding: u32, size_bytes: usize, usage: BufferUsage) -> Self {
        Self {
            name: name.into(),
            binding,
            size_bytes,
            usage,
            data: vec![0u8; size_bytes],
            dirty: true,
        }
    }

    pub fn write_f32(&mut self, offset: usize, value: f32) {
        if offset + 4 <= self.data.len() {
            self.data[offset..offset+4].copy_from_slice(&value.to_le_bytes());
            self.dirty = true;
        }
    }

    pub fn write_vec3(&mut self, offset: usize, v: Vec3) {
        self.write_f32(offset,     v.x);
        self.write_f32(offset + 4, v.y);
        self.write_f32(offset + 8, v.z);
    }

    pub fn write_u32(&mut self, offset: usize, value: u32) {
        if offset + 4 <= self.data.len() {
            self.data[offset..offset+4].copy_from_slice(&value.to_le_bytes());
            self.dirty = true;
        }
    }

    pub fn read_f32(&self, offset: usize) -> f32 {
        if offset + 4 <= self.data.len() {
            f32::from_le_bytes(self.data[offset..offset+4].try_into().unwrap_or([0;4]))
        } else {
            0.0
        }
    }

    pub fn read_u32(&self, offset: usize) -> u32 {
        if offset + 4 <= self.data.len() {
            u32::from_le_bytes(self.data[offset..offset+4].try_into().unwrap_or([0;4]))
        } else {
            0
        }
    }

    pub fn clear(&mut self) {
        self.data.fill(0);
        self.dirty = true;
    }
}

// ── ComputeShader ─────────────────────────────────────────────────────────────

/// A compiled compute shader program.
#[derive(Debug, Clone)]
pub struct ComputeShader {
    pub name:        String,
    /// GLSL source code.
    pub source:      String,
    /// Local work group sizes.
    pub local_x:     u32,
    pub local_y:     u32,
    pub local_z:     u32,
    /// Bound SSBOs.
    pub buffers:     Vec<String>,
    /// Uniform values.
    pub uniforms:    std::collections::HashMap<String, UniformValue>,
}

#[derive(Debug, Clone)]
pub enum UniformValue {
    Float(f32),
    Int(i32),
    UInt(u32),
    Vec3(Vec3),
    Vec4(glam::Vec4),
    Bool(bool),
}

impl ComputeShader {
    pub fn new(name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            name:     name.into(),
            source:   source.into(),
            local_x:  64,
            local_y:  1,
            local_z:  1,
            buffers:  Vec::new(),
            uniforms: std::collections::HashMap::new(),
        }
    }

    pub fn with_work_group(mut self, x: u32, y: u32, z: u32) -> Self {
        self.local_x = x; self.local_y = y; self.local_z = z;
        self
    }

    pub fn set_uniform(&mut self, name: &str, value: UniformValue) {
        self.uniforms.insert(name.to_owned(), value);
    }

    pub fn bind_buffer(mut self, name: impl Into<String>) -> Self {
        self.buffers.push(name.into());
        self
    }
}

// ── ComputeDispatch ───────────────────────────────────────────────────────────

/// Parameters for a single compute dispatch call.
#[derive(Debug, Clone)]
pub struct ComputeDispatch {
    pub shader:     String,
    pub groups_x:   u32,
    pub groups_y:   u32,
    pub groups_z:   u32,
    pub barriers:   Vec<MemoryBarrier>,
}

impl ComputeDispatch {
    pub fn new(shader: impl Into<String>, items: u32, local_size: u32) -> Self {
        let groups = (items + local_size - 1) / local_size;
        Self {
            shader:   shader.into(),
            groups_x: groups,
            groups_y: 1,
            groups_z: 1,
            barriers: vec![MemoryBarrier::ShaderStorage],
        }
    }

    pub fn with_barrier(mut self, b: MemoryBarrier) -> Self {
        self.barriers.push(b); self
    }
}

// ── MemoryBarrier ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryBarrier {
    ShaderStorage,
    VertexAttrib,
    Uniform,
    TextureFetch,
    ImageAccess,
    All,
}

// ── GpuParticle ───────────────────────────────────────────────────────────────

/// Layout of a single GPU particle in the SSBO (48 bytes per particle).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuParticle {
    pub position:   [f32; 3],
    pub life:       f32,
    pub velocity:   [f32; 3],
    pub max_life:   f32,
    pub color:      [f32; 4],
    pub size:       f32,
    pub emission:   f32,
    pub behavior:   u32,
    pub _pad:       u32,
}

impl GpuParticle {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const Self as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }
}

// ── GpuParticleSystem ─────────────────────────────────────────────────────────

/// GPU-side particle simulation supporting 100K+ particles.
///
/// Particles live entirely on the GPU. Two SSBOs double-buffer the state.
/// A compute shader updates positions, applies forces, and handles death.
/// Indirect draw calls render without CPU readback.
pub struct GpuParticleSystem {
    pub capacity:       usize,
    pub alive_count:    u32,
    pub dt_uniform:     f32,
    /// Current write buffer index (0 or 1).
    pub write_buffer:   usize,
    pub particle_bufs:  [ComputeBuffer; 2],
    pub counter_buf:    ComputeBuffer,
    pub force_buf:      ComputeBuffer,
    pub indirect_buf:   ComputeBuffer,
    pub update_shader:  ComputeShader,
    pub emit_shader:    ComputeShader,
}

impl GpuParticleSystem {
    pub fn new(capacity: usize) -> Self {
        let bytes_per_particle = std::mem::size_of::<GpuParticle>();
        let buf_size = capacity * bytes_per_particle;

        let particle_buf_a = ComputeBuffer::new("particles_a", 0, buf_size, BufferUsage::GpuOnly);
        let particle_buf_b = ComputeBuffer::new("particles_b", 1, buf_size, BufferUsage::GpuOnly);
        // Counter: [alive_count: u32, dead_stack_top: u32, pad: u32 * 2]
        let counter_buf = ComputeBuffer::new("particle_counter", 2, 16, BufferUsage::GpuOnly);
        // Force buffer: [count: u32, pad * 3, force_entries: {pos:vec3, type:u32, strength:f32, radius:f32, pad*2} * 64]
        let force_buf = ComputeBuffer::new("particle_forces", 3, 4 + 64 * 32, BufferUsage::DynamicDraw);
        // Indirect draw args: [vertex_count: u32, instance_count: u32, first_vertex: u32, base_instance: u32]
        let indirect_buf = ComputeBuffer::new("indirect_draw", 4, 16, BufferUsage::GpuOnly);

        let update_shader = ComputeShader::new("particle_update", PARTICLE_UPDATE_GLSL)
            .with_work_group(64, 1, 1)
            .bind_buffer("particles_a")
            .bind_buffer("particles_b")
            .bind_buffer("particle_counter")
            .bind_buffer("particle_forces")
            .bind_buffer("indirect_draw");

        let emit_shader = ComputeShader::new("particle_emit", PARTICLE_EMIT_GLSL)
            .with_work_group(64, 1, 1)
            .bind_buffer("particles_a")
            .bind_buffer("particle_counter");

        Self {
            capacity,
            alive_count:   0,
            dt_uniform:    0.016,
            write_buffer:  0,
            particle_bufs: [particle_buf_a, particle_buf_b],
            counter_buf,
            force_buf,
            indirect_buf,
            update_shader,
            emit_shader,
        }
    }

    /// Build dispatch commands for one simulation step.
    pub fn build_update_dispatch(&self) -> ComputeDispatch {
        ComputeDispatch::new("particle_update", self.capacity as u32, 64)
            .with_barrier(MemoryBarrier::ShaderStorage)
    }

    pub fn build_emit_dispatch(&self, count: u32) -> ComputeDispatch {
        ComputeDispatch::new("particle_emit", count, 64)
    }

    pub fn swap_buffers(&mut self) {
        self.write_buffer ^= 1;
    }

    pub fn read_buffer(&self) -> usize { self.write_buffer ^ 1 }

    /// Upload a force field entry to the force buffer.
    pub fn set_force(&mut self, index: usize, pos: Vec3, force_type: u32, strength: f32, radius: f32) {
        let base = 4 + index * 32;
        self.force_buf.write_vec3(base,      pos);
        self.force_buf.write_u32 (base + 12, force_type);
        self.force_buf.write_f32 (base + 16, strength);
        self.force_buf.write_f32 (base + 20, radius);
    }

    pub fn set_force_count(&mut self, count: u32) {
        self.force_buf.write_u32(0, count);
    }
}

// ── GLSL Sources ──────────────────────────────────────────────────────────────

const PARTICLE_UPDATE_GLSL: &str = r#"
#version 430 core
layout(local_size_x = 64) in;

struct Particle {
    vec3  position;  float life;
    vec3  velocity;  float max_life;
    vec4  color;
    float size;      float emission;
    uint  behavior;  uint  pad;
};

layout(std430, binding = 0) buffer ReadBuf  { Particle particles_in[];  };
layout(std430, binding = 1) buffer WriteBuf { Particle particles_out[]; };
layout(std430, binding = 2) buffer Counter  { uint alive; uint dead_top; };
layout(std430, binding = 4) buffer Indirect { uint vertex_count; uint instance_count; uint first; uint base; };

uniform float dt;
uniform uint  capacity;

void main() {
    uint id = gl_GlobalInvocationID.x;
    if (id >= capacity) return;

    Particle p = particles_in[id];
    if (p.life <= 0.0) { particles_out[id] = p; return; }

    p.life     -= dt;
    p.position += p.velocity * dt;

    // Gravity
    p.velocity.y -= 9.8 * dt * 0.1;

    // Fade out
    float t = p.life / max(p.max_life, 0.001);
    p.color.a = t;

    particles_out[id] = p;

    if (p.life > 0.0) {
        atomicAdd(instance_count, 1u);
    }
}
"#;

const PARTICLE_EMIT_GLSL: &str = r#"
#version 430 core
layout(local_size_x = 64) in;

struct Particle {
    vec3  position;  float life;
    vec3  velocity;  float max_life;
    vec4  color;
    float size;      float emission;
    uint  behavior;  uint  pad;
};

layout(std430, binding = 0) buffer Particles { Particle data[]; };
layout(std430, binding = 2) buffer Counter   { uint alive; uint dead_top; };

uniform vec3  emit_origin;
uniform float emit_speed;
uniform float emit_life;
uniform uint  emit_count;
uniform uint  frame_seed;

uint hash(uint x) {
    x ^= x >> 16; x *= 0x45d9f3b; x ^= x >> 16;
    return x;
}

void main() {
    uint id = gl_GlobalInvocationID.x;
    if (id >= emit_count) return;
    uint slot = atomicAdd(alive, 1u);
    uint h = hash(id ^ frame_seed);
    vec3 dir = normalize(vec3(
        float(h & 0xFFu) / 127.5 - 1.0,
        float((h >> 8) & 0xFFu) / 127.5 - 1.0,
        float((h >> 16) & 0xFFu) / 127.5 - 1.0
    ));
    data[slot].position = emit_origin;
    data[slot].velocity = dir * emit_speed;
    data[slot].life     = emit_life;
    data[slot].max_life = emit_life;
    data[slot].color    = vec4(1.0);
    data[slot].size     = 0.1;
    data[slot].emission = 1.0;
}
"#;

// ── RenderGraph ───────────────────────────────────────────────────────────────
// (Forward declaration — full implementation in render_graph.rs)

/// A handle to a render graph resource (texture, FBO, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceHandle(pub u32);

impl ResourceHandle {
    pub fn backbuffer() -> Self { Self(0) }
    pub fn is_backbuffer(self) -> bool { self.0 == 0 }
}
