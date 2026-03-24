//! GPU Compute Pipeline for Proof Engine.
//!
//! Abstracts over GPU compute shaders for:
//! - 100K+ particle simulation (position/velocity integration on GPU)
//! - Compute shader dispatch with SSBO (Shader Storage Buffer Objects)
//! - Double-buffered state for ping-pong GPU updates
//! - Indirect draw command generation from compute results
//! - GPU particle sorting (bitonic sort compute shader)
//! - Force field evaluation on the GPU
//! - Fluid simulation compute passes

// Note: This module provides the CPU-side orchestration and data structures
// for GPU compute. Actual GLSL shader source strings are included for completeness.

use std::collections::HashMap;

// ── GpuBufferId ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComputePassId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineId(pub u32);

// ── GpuBufferDesc ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferUsage {
    /// Shader Storage Buffer Object — read/write by compute.
    Ssbo,
    /// Uniform Buffer Object — read-only small data.
    Ubo,
    /// Indirect draw command buffer.
    IndirectDraw,
    /// Atomic counter.
    Atomic,
    /// Vertex buffer populated by compute.
    VertexOut,
}

#[derive(Debug, Clone)]
pub struct GpuBufferDesc {
    pub id:     BufferId,
    pub name:   String,
    pub size:   usize, // bytes
    pub usage:  BufferUsage,
    /// Optional initial data.
    pub data:   Option<Vec<u8>>,
    pub dynamic: bool,
}

impl GpuBufferDesc {
    pub fn ssbo(name: &str, size: usize) -> Self {
        Self { id: BufferId(0), name: name.to_string(), size, usage: BufferUsage::Ssbo, data: None, dynamic: true }
    }

    pub fn ubo(name: &str, size: usize) -> Self {
        Self { id: BufferId(0), name: name.to_string(), size, usage: BufferUsage::Ubo, data: None, dynamic: false }
    }

    pub fn indirect(name: &str, max_draws: usize) -> Self {
        // IndirectDrawArraysCommand = 4 × u32 = 16 bytes each
        Self::ssbo(name, max_draws * 16).with_usage(BufferUsage::IndirectDraw)
    }

    fn with_usage(mut self, usage: BufferUsage) -> Self { self.usage = usage; self }

    pub fn with_data(mut self, data: Vec<u8>) -> Self { self.data = Some(data); self }
}

// ── ComputePassDesc ───────────────────────────────────────────────────────────

/// Descriptor for a single compute dispatch pass.
#[derive(Debug, Clone)]
pub struct ComputePassDesc {
    pub id:          ComputePassId,
    pub name:        String,
    pub shader_src:  String,
    pub work_groups: [u32; 3],
    /// (binding_point, buffer_id)
    pub ssbo_bindings: Vec<(u32, BufferId)>,
    pub ubo_bindings:  Vec<(u32, BufferId)>,
    pub uniforms:      HashMap<String, ComputeUniform>,
    /// Barrier required before next pass.
    pub barrier:       MemoryBarrier,
}

#[derive(Debug, Clone)]
pub enum ComputeUniform {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    UInt(u32),
    Mat4([f32; 16]),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryBarrier {
    None,
    ShaderStorage,
    Buffer,
    All,
}

impl ComputePassDesc {
    pub fn new(name: &str, shader_src: &str) -> Self {
        Self {
            id: ComputePassId(0),
            name: name.to_string(),
            shader_src: shader_src.to_string(),
            work_groups: [1, 1, 1],
            ssbo_bindings: Vec::new(),
            ubo_bindings: Vec::new(),
            uniforms: HashMap::new(),
            barrier: MemoryBarrier::ShaderStorage,
        }
    }

    pub fn dispatch(mut self, x: u32, y: u32, z: u32) -> Self {
        self.work_groups = [x, y, z];
        self
    }

    pub fn bind_ssbo(mut self, binding: u32, buf: BufferId) -> Self {
        self.ssbo_bindings.push((binding, buf));
        self
    }

    pub fn bind_ubo(mut self, binding: u32, buf: BufferId) -> Self {
        self.ubo_bindings.push((binding, buf));
        self
    }

    pub fn set_uniform(mut self, name: &str, v: ComputeUniform) -> Self {
        self.uniforms.insert(name.to_string(), v);
        self
    }
}

// ── Particle layout (matches GPU struct) ──────────────────────────────────────

/// CPU-side mirror of the GPU particle struct.
/// `#[repr(C)]` to match GLSL std430 layout.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GpuParticle {
    pub position:  [f32; 4], // xyz + lifetime
    pub velocity:  [f32; 4], // xyz + age
    pub color:     [f32; 4], // rgba
    pub size:      f32,
    pub mass:      f32,
    pub flags:     u32,      // bit 0: alive, bit 1: emitting, bit 2: collides
    pub attractor: u32,      // index of attractor affecting this particle
}

impl GpuParticle {
    pub const SIZE: usize = std::mem::size_of::<GpuParticle>();

    pub fn alive(pos: [f32; 3], vel: [f32; 3], color: [f32; 4], lifetime: f32, size: f32) -> Self {
        Self {
            position: [pos[0], pos[1], pos[2], lifetime],
            velocity: [vel[0], vel[1], vel[2], 0.0],
            color,
            size,
            mass: 1.0,
            flags: 1,
            attractor: 0,
        }
    }

    pub fn is_alive(&self) -> bool { self.flags & 1 != 0 }
    pub fn lifetime(&self) -> f32 { self.position[3] }
    pub fn age(&self) -> f32 { self.velocity[3] }
}

// ── GPU Attractor (force field element) ──────────────────────────────────────

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GpuAttractor {
    pub position:  [f32; 4], // xyz + strength
    pub params:    [f32; 4], // type, falloff_start, falloff_end, rotation
    pub color:     [f32; 4],
    pub attractor_type: u32, // 0=point, 1=vortex, 2=lorenz, 3=repulse
    _pad: [u32; 3],
}

impl GpuAttractor {
    pub fn point(pos: [f32; 3], strength: f32) -> Self {
        Self {
            position: [pos[0], pos[1], pos[2], strength],
            params: [0.0, 0.5, 5.0, 0.0],
            color: [1.0; 4],
            attractor_type: 0,
            _pad: [0; 3],
        }
    }

    pub fn vortex(pos: [f32; 3], strength: f32, rotation: f32) -> Self {
        let mut a = Self::point(pos, strength);
        a.attractor_type = 1;
        a.params[3] = rotation;
        a
    }
}

// ── IndirectDrawCommand ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct IndirectDrawCommand {
    pub count:      u32, // vertices to draw
    pub prim_count: u32, // instances
    pub first:      u32, // first vertex
    pub base_inst:  u32, // base instance
}

impl IndirectDrawCommand {
    pub fn new(count: u32) -> Self {
        Self { count, prim_count: 1, first: 0, base_inst: 0 }
    }
}

// ── GLSL Shader Sources ───────────────────────────────────────────────────────

/// Particle integration compute shader (GLSL 4.30+).
pub const PARTICLE_INTEGRATE_GLSL: &str = r#"
#version 430 core

layout(local_size_x = 256) in;

struct Particle {
    vec4 position;  // xyz + lifetime
    vec4 velocity;  // xyz + age
    vec4 color;
    float size;
    float mass;
    uint flags;
    uint attractor;
};

struct Attractor {
    vec4 position;   // xyz + strength
    vec4 params;     // type, falloff_start, falloff_end, rotation
    vec4 color;
    uint atype;
    uint _pad[3];
};

layout(std430, binding = 0) buffer ParticleBuffer {
    Particle particles[];
};

layout(std430, binding = 1) readonly buffer AttractorBuffer {
    Attractor attractors[];
};

layout(std430, binding = 2) buffer DeadList {
    uint dead_count;
    uint dead_indices[];
};

layout(std140, binding = 0) uniform Params {
    float dt;
    float time;
    vec3 gravity;
    float drag;
    uint num_particles;
    uint num_attractors;
    float emit_rate;
    float _pad;
};

// Lorenz attractor vector field
vec3 lorenz(vec3 p, float sigma, float rho, float beta) {
    return vec3(
        sigma * (p.y - p.x),
        p.x * (rho - p.z) - p.y,
        p.x * p.y - beta * p.z
    );
}

// Vortex force
vec3 vortex_force(vec3 particle_pos, vec3 center, float strength, float rotation) {
    vec3 r = particle_pos - center;
    float d = length(r) + 0.001;
    vec3 tangent = cross(r, vec3(0.0, 1.0, 0.0)) / d;
    return tangent * strength * rotation / (d * d + 1.0);
}

vec3 compute_attractor_force(Particle p, Attractor a) {
    vec3 pos = p.position.xyz;
    vec3 apos = a.position.xyz;
    float strength = a.position.w;
    float falloff_start = a.params.y;
    float falloff_end   = a.params.z;

    vec3 delta = apos - pos;
    float dist = length(delta) + 0.001;

    // Falloff
    float t = clamp((dist - falloff_start) / (falloff_end - falloff_start + 0.001), 0.0, 1.0);
    float attenuation = 1.0 - t;

    switch (a.atype) {
        case 0: // Point attractor
            return normalize(delta) * strength * attenuation / (dist * dist + 1.0);
        case 1: // Vortex
            return vortex_force(pos, apos, strength * attenuation, a.params.w);
        case 2: // Lorenz field
            return lorenz(pos * 0.1, 10.0, 28.0, 2.667) * strength * 0.01 * attenuation;
        case 3: // Repulse
            return -normalize(delta) * strength * attenuation / (dist * dist + 0.5);
        default:
            return vec3(0.0);
    }
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= num_particles) return;

    Particle p = particles[idx];
    if ((p.flags & 1u) == 0u) return; // Skip dead particles

    // Accumulate forces
    vec3 force = gravity * p.mass;

    for (uint i = 0; i < num_attractors; i++) {
        force += compute_attractor_force(p, attractors[i]);
    }

    // Drag
    force -= p.velocity.xyz * drag;

    // Semi-implicit Euler integration
    vec3 new_vel = p.velocity.xyz + (force / p.mass) * dt;
    vec3 new_pos = p.position.xyz + new_vel * dt;

    // Age
    float new_age      = p.velocity.w + dt;
    float lifetime     = p.position.w;

    // Kill if expired
    if (new_age >= lifetime) {
        p.flags &= ~1u; // clear alive bit
        uint dead_idx = atomicAdd(dead_count, 1u);
        dead_indices[dead_idx] = idx;
    } else {
        p.position.xyz = new_pos;
        p.velocity.xyz = new_vel;
        p.velocity.w   = new_age;
    }

    particles[idx] = p;
}
"#;

/// Particle emit compute shader — spawns new particles from dead list.
pub const PARTICLE_EMIT_GLSL: &str = r#"
#version 430 core

layout(local_size_x = 64) in;

struct Particle {
    vec4 position;
    vec4 velocity;
    vec4 color;
    float size;
    float mass;
    uint flags;
    uint attractor;
};

layout(std430, binding = 0) buffer ParticleBuffer { Particle particles[]; };
layout(std430, binding = 1) buffer DeadList       { uint dead_count; uint dead_indices[]; };
layout(std430, binding = 2) readonly buffer EmitBuffer { uint emit_count; uvec4 emit_data[]; };

layout(std140, binding = 0) uniform EmitParams {
    vec3 origin;
    float spread;
    vec4 color_a;
    vec4 color_b;
    float lifetime_min;
    float lifetime_max;
    float speed_min;
    float speed_max;
    float size_min;
    float size_max;
    float time;
    uint seed;
};

// Simple hash function for pseudo-randomness
float hash(uint n) {
    n = (n ^ 61u) ^ (n >> 16u);
    n *= 9u; n ^= n >> 4u;
    n *= 0x27d4eb2du; n ^= n >> 15u;
    return float(n) / float(0xFFFFFFFFu);
}

vec3 random_dir(uint seed) {
    float theta = hash(seed)       * 6.2831853;
    float phi   = hash(seed + 1u)  * 3.1415927;
    return vec3(sin(phi)*cos(theta), cos(phi), sin(phi)*sin(theta));
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= emit_count) return;

    // Claim a dead particle slot
    uint dead_idx_pos = atomicAdd(dead_count, uint(-1));
    if (dead_idx_pos == 0u) return; // No dead particles available
    uint slot = dead_indices[dead_idx_pos - 1u];

    uint s = seed + idx * 7u;
    float lifetime = mix(lifetime_min, lifetime_max, hash(s));
    float speed    = mix(speed_min,    speed_max,    hash(s + 2u));
    float psize    = mix(size_min,     size_max,     hash(s + 3u));
    vec3 dir = random_dir(s + 4u);
    vec3 pos = origin + dir * spread * hash(s + 5u);

    particles[slot].position = vec4(pos, lifetime);
    particles[slot].velocity = vec4(dir * speed, 0.0);
    particles[slot].color    = mix(color_a, color_b, hash(s + 6u));
    particles[slot].size     = psize;
    particles[slot].mass     = 1.0;
    particles[slot].flags    = 1u;
    particles[slot].attractor = 0u;
}
"#;

/// Indirect draw generation — count alive particles and build draw command.
pub const PARTICLE_COUNT_GLSL: &str = r#"
#version 430 core

layout(local_size_x = 256) in;

struct Particle { vec4 position; vec4 velocity; vec4 color; float size; float mass; uint flags; uint attractor; };

layout(std430, binding = 0) readonly buffer ParticleBuffer { Particle particles[]; };
layout(std430, binding = 1) buffer IndirectBuffer {
    uint vertex_count;
    uint instance_count;
    uint first_vertex;
    uint base_instance;
};

uniform uint num_particles;

shared uint local_count;

void main() {
    if (gl_LocalInvocationID.x == 0) local_count = 0;
    barrier();

    uint idx = gl_GlobalInvocationID.x;
    if (idx < num_particles && (particles[idx].flags & 1u) != 0u) {
        atomicAdd(local_count, 1u);
    }
    barrier();

    if (gl_LocalInvocationID.x == 0) {
        atomicAdd(instance_count, local_count);
    }

    if (idx == 0) vertex_count = 4u; // Billboard quad = 4 verts
}
"#;

/// Fluid simulation velocity advection compute pass.
pub const FLUID_ADVECT_GLSL: &str = r#"
#version 430 core

layout(local_size_x = 16, local_size_y = 16) in;

layout(std430, binding = 0) buffer VelocityX  { float vel_x[]; };
layout(std430, binding = 1) buffer VelocityY  { float vel_y[]; };
layout(std430, binding = 2) buffer VelocityXn { float vel_xn[]; };
layout(std430, binding = 3) buffer VelocityYn { float vel_yn[]; };
layout(std430, binding = 4) readonly buffer Density { float density[]; };

uniform int grid_w;
uniform int grid_h;
uniform float dt;
uniform float dissipation;

int idx(int x, int y) { return clamp(x, 0, grid_w-1) + clamp(y, 0, grid_h-1) * grid_w; }

float sample_x(float px, float py) {
    int x0 = int(floor(px)); int x1 = x0 + 1;
    int y0 = int(floor(py)); int y1 = y0 + 1;
    float tx = fract(px); float ty = fract(py);
    return mix(mix(vel_x[idx(x0,y0)], vel_x[idx(x1,y0)], tx),
               mix(vel_x[idx(x0,y1)], vel_x[idx(x1,y1)], tx), ty);
}

float sample_y(float px, float py) {
    int x0 = int(floor(px)); int x1 = x0 + 1;
    int y0 = int(floor(py)); int y1 = y0 + 1;
    float tx = fract(px); float ty = fract(py);
    return mix(mix(vel_y[idx(x0,y0)], vel_y[idx(x1,y0)], tx),
               mix(vel_y[idx(x0,y1)], vel_y[idx(x1,y1)], tx), ty);
}

void main() {
    int x = int(gl_GlobalInvocationID.x);
    int y = int(gl_GlobalInvocationID.y);
    if (x >= grid_w || y >= grid_h) return;

    int i = idx(x, y);
    float vx = vel_x[i];
    float vy = vel_y[i];

    // Backtrace
    float px = float(x) - vx * dt;
    float py = float(y) - vy * dt;

    vel_xn[i] = sample_x(px, py) * dissipation;
    vel_yn[i] = sample_y(px, py) * dissipation;
}
"#;

/// Bitonic sort for GPU particle depth ordering.
pub const BITONIC_SORT_GLSL: &str = r#"
#version 430 core

layout(local_size_x = 512) in;

layout(std430, binding = 0) buffer Keys   { float keys[]; };   // depth values
layout(std430, binding = 1) buffer Values { uint  values[]; }; // particle indices

uniform uint num_elements;
uniform uint block_size;
uniform uint sub_block_size;
uniform bool ascending;

shared float shared_keys[512];
shared uint  shared_vals[512];

void main() {
    uint gid = gl_GlobalInvocationID.x;
    uint lid = gl_LocalInvocationID.x;

    if (gid < num_elements) {
        shared_keys[lid] = keys[gid];
        shared_vals[lid] = values[gid];
    } else {
        shared_keys[lid] = ascending ? 1e38 : -1e38;
        shared_vals[lid] = gid;
    }
    barrier();

    for (uint stride = sub_block_size; stride > 0; stride >>= 1) {
        uint idx_a = (gid / stride) * stride * 2 + (gid % stride);
        uint idx_b = idx_a + stride;

        if (idx_a < num_elements && idx_b < num_elements) {
            bool swap_cond = ascending
                ? (shared_keys[idx_a % 512] > shared_keys[idx_b % 512])
                : (shared_keys[idx_a % 512] < shared_keys[idx_b % 512]);

            if (swap_cond) {
                float tmp_k = shared_keys[idx_a % 512];
                shared_keys[idx_a % 512] = shared_keys[idx_b % 512];
                shared_keys[idx_b % 512] = tmp_k;

                uint tmp_v = shared_vals[idx_a % 512];
                shared_vals[idx_a % 512] = shared_vals[idx_b % 512];
                shared_vals[idx_b % 512] = tmp_v;
            }
        }
        barrier();
    }

    if (gid < num_elements) {
        keys[gid]   = shared_keys[lid];
        values[gid] = shared_vals[lid];
    }
}
"#;

// ── ComputePipeline ───────────────────────────────────────────────────────────

/// Manages a set of compute passes as a pipeline.
pub struct ComputePipeline {
    pub name:    String,
    passes:      Vec<ComputePassDesc>,
    buffers:     Vec<GpuBufferDesc>,
    next_buf_id: u32,
    next_pass_id: u32,
    pub enabled: bool,
    /// Execution order (pass indices).
    pub order:   Vec<usize>,
}

impl ComputePipeline {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), passes: Vec::new(), buffers: Vec::new(),
               next_buf_id: 1, next_pass_id: 1, enabled: true, order: Vec::new() }
    }

    pub fn add_buffer(&mut self, mut desc: GpuBufferDesc) -> BufferId {
        let id = BufferId(self.next_buf_id);
        self.next_buf_id += 1;
        desc.id = id;
        self.buffers.push(desc);
        id
    }

    pub fn add_pass(&mut self, mut desc: ComputePassDesc) -> ComputePassId {
        let id = ComputePassId(self.next_pass_id);
        self.next_pass_id += 1;
        desc.id = id;
        let idx = self.passes.len();
        self.passes.push(desc);
        self.order.push(idx);
        id
    }

    pub fn pass(&self, id: ComputePassId) -> Option<&ComputePassDesc> {
        self.passes.iter().find(|p| p.id == id)
    }

    pub fn buffer(&self, id: BufferId) -> Option<&GpuBufferDesc> {
        self.buffers.iter().find(|b| b.id == id)
    }

    pub fn buffer_by_name(&self, name: &str) -> Option<&GpuBufferDesc> {
        self.buffers.iter().find(|b| b.name == name)
    }

    pub fn total_buffer_size(&self) -> usize {
        self.buffers.iter().map(|b| b.size).sum()
    }
}

// ── GpuParticleSystem ─────────────────────────────────────────────────────────

/// Complete GPU particle system configuration and state.
pub struct GpuParticleSystem {
    pub pipeline: ComputePipeline,
    pub max_particles: usize,
    pub particle_buf_a: BufferId,
    pub particle_buf_b: BufferId,
    pub attractor_buf:  BufferId,
    pub dead_list_buf:  BufferId,
    pub indirect_buf:   BufferId,
    pub params_ubo:     BufferId,
    pub integrate_pass: ComputePassId,
    pub emit_pass:      ComputePassId,
    pub count_pass:     ComputePassId,
    pub sort_pass:      ComputePassId,
    /// Which buffer is "current" (ping/pong).
    pub frame:          u64,
    // CPU-side particle state for initial upload
    pub initial_particles: Vec<GpuParticle>,
    pub attractors:        Vec<GpuAttractor>,
    pub gravity:           [f32; 3],
    pub drag:              f32,
    pub emit_rate:         f32,
    pub do_sort:           bool,
}

impl GpuParticleSystem {
    /// Build a complete 100K particle system pipeline.
    pub fn new(max_particles: usize) -> Self {
        let mut pipeline = ComputePipeline::new("gpu_particles");

        // Buffers
        let particle_size = GpuParticle::SIZE * max_particles;
        let attractor_size = std::mem::size_of::<GpuAttractor>() * 64;
        let dead_size = 4 + 4 * max_particles; // count + indices
        let indirect_size = std::mem::size_of::<IndirectDrawCommand>();
        let params_size = 64; // Params UBO

        let particle_buf_a = pipeline.add_buffer(GpuBufferDesc::ssbo("particles_a", particle_size));
        let particle_buf_b = pipeline.add_buffer(GpuBufferDesc::ssbo("particles_b", particle_size));
        let attractor_buf  = pipeline.add_buffer(GpuBufferDesc::ssbo("attractors", attractor_size));
        let dead_list_buf  = pipeline.add_buffer(GpuBufferDesc::ssbo("dead_list", dead_size));
        let indirect_buf   = pipeline.add_buffer(GpuBufferDesc::indirect("indirect", 1));
        let params_ubo     = pipeline.add_buffer(GpuBufferDesc::ubo("params", params_size));

        // Work groups: 256 threads per group, ceil(N/256) groups
        let integrate_groups = ((max_particles + 255) / 256) as u32;

        let integrate_pass = pipeline.add_pass(
            ComputePassDesc::new("integrate", PARTICLE_INTEGRATE_GLSL)
                .dispatch(integrate_groups, 1, 1)
                .bind_ssbo(0, particle_buf_a)
                .bind_ssbo(1, attractor_buf)
                .bind_ssbo(2, dead_list_buf)
                .bind_ubo(0, params_ubo)
        );

        let emit_pass = pipeline.add_pass(
            ComputePassDesc::new("emit", PARTICLE_EMIT_GLSL)
                .dispatch(4, 1, 1)
                .bind_ssbo(0, particle_buf_a)
                .bind_ssbo(1, dead_list_buf)
                .bind_ubo(0, params_ubo)
        );

        let count_pass = pipeline.add_pass(
            ComputePassDesc::new("count", PARTICLE_COUNT_GLSL)
                .dispatch(integrate_groups, 1, 1)
                .bind_ssbo(0, particle_buf_a)
                .bind_ssbo(1, indirect_buf)
                .set_uniform("num_particles", ComputeUniform::UInt(max_particles as u32))
        );

        let sort_pass = pipeline.add_pass(
            ComputePassDesc::new("sort", BITONIC_SORT_GLSL)
                .dispatch((max_particles / 512 + 1) as u32, 1, 1)
                .bind_ssbo(0, particle_buf_a)
        );

        Self {
            pipeline,
            max_particles,
            particle_buf_a, particle_buf_b,
            attractor_buf, dead_list_buf, indirect_buf, params_ubo,
            integrate_pass, emit_pass, count_pass, sort_pass,
            frame: 0,
            initial_particles: Vec::new(),
            attractors: Vec::new(),
            gravity: [0.0, -9.81, 0.0],
            drag: 0.02,
            emit_rate: 1000.0,
            do_sort: false,
        }
    }

    /// Add an attractor.
    pub fn add_attractor(&mut self, a: GpuAttractor) {
        self.attractors.push(a);
    }

    /// Spawn initial particles (CPU-side setup for upload).
    pub fn spawn_burst(&mut self, origin: [f32; 3], count: usize, speed: f32, lifetime: f32) {
        for i in 0..count {
            let theta = i as f32 * 2.399963; // golden angle
            let phi   = (i as f32 / count as f32).acos();
            let vel = [phi.sin() * theta.cos() * speed,
                       phi.cos() * speed,
                       phi.sin() * theta.sin() * speed];
            self.initial_particles.push(GpuParticle::alive(
                origin, vel, [1.0, 0.8, 0.2, 1.0], lifetime, 2.0,
            ));
        }
    }

    pub fn advance_frame(&mut self) { self.frame += 1; }

    /// Current particle buffer (ping-pong).
    pub fn current_buffer(&self) -> BufferId {
        if self.frame % 2 == 0 { self.particle_buf_a } else { self.particle_buf_b }
    }

    /// Build dispatch parameters for this frame.
    pub fn frame_params(&self, dt: f32) -> HashMap<String, f32> {
        let mut p = HashMap::new();
        p.insert("dt".to_string(), dt);
        p.insert("time".to_string(), self.frame as f32 * dt);
        p.insert("gravity_x".to_string(), self.gravity[0]);
        p.insert("gravity_y".to_string(), self.gravity[1]);
        p.insert("gravity_z".to_string(), self.gravity[2]);
        p.insert("drag".to_string(), self.drag);
        p.insert("num_particles".to_string(), self.max_particles as f32);
        p.insert("emit_rate".to_string(), self.emit_rate);
        p
    }
}

// ── Presets ───────────────────────────────────────────────────────────────────

pub struct ComputePresets;

impl ComputePresets {
    /// 100K chaos field: mix of point attractors + Lorenz
    pub fn chaos_field() -> GpuParticleSystem {
        let mut sys = GpuParticleSystem::new(100_000);
        sys.add_attractor(GpuAttractor::point([0.0, 0.0, 0.0], 5.0));
        sys.add_attractor(GpuAttractor::vortex([10.0, 0.0, 0.0], 3.0, 2.0));
        sys.add_attractor(GpuAttractor::vortex([-10.0, 0.0, 0.0], 3.0, -2.0));
        sys.spawn_burst([0.0; 3], 50_000, 0.5, 5.0);
        sys.gravity = [0.0; 3];
        sys.drag = 0.01;
        sys
    }

    /// Fireworks burst: particles explode from center with gravity
    pub fn fireworks() -> GpuParticleSystem {
        let mut sys = GpuParticleSystem::new(50_000);
        sys.spawn_burst([0.0, 0.0, 0.0], 50_000, 5.0, 3.0);
        sys.gravity = [0.0, -9.81, 0.0];
        sys.drag = 0.05;
        sys
    }

    /// Fluid simulation particles
    pub fn fluid_particles() -> GpuParticleSystem {
        let mut sys = GpuParticleSystem::new(200_000);
        sys.do_sort = true;
        sys.gravity = [0.0, -2.0, 0.0];
        sys.drag = 0.1;
        sys
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_particle_size() {
        assert_eq!(GpuParticle::SIZE, 48, "Particle must be exactly 48 bytes for std430");
    }

    #[test]
    fn test_pipeline_builds() {
        let sys = GpuParticleSystem::new(1024);
        assert!(sys.pipeline.total_buffer_size() > 0);
        assert_eq!(sys.pipeline.passes.len(), 4);
    }

    #[test]
    fn test_particle_alive() {
        let p = GpuParticle::alive([1.0, 2.0, 3.0], [0.1, 0.2, 0.3], [1.0; 4], 5.0, 2.0);
        assert!(p.is_alive());
        assert!((p.lifetime() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_attractor_types() {
        let a = GpuAttractor::point([0.0; 3], 10.0);
        assert_eq!(a.attractor_type, 0);
        let v = GpuAttractor::vortex([1.0, 0.0, 0.0], 5.0, 1.5);
        assert_eq!(v.attractor_type, 1);
    }

    #[test]
    fn test_chaos_field_preset() {
        let sys = ComputePresets::chaos_field();
        assert_eq!(sys.max_particles, 100_000);
        assert_eq!(sys.attractors.len(), 3);
        assert!(!sys.initial_particles.is_empty());
    }

    #[test]
    fn test_frame_params() {
        let sys = GpuParticleSystem::new(1024);
        let params = sys.frame_params(0.016);
        assert!((params["dt"] - 0.016).abs() < 0.0001);
        assert_eq!(params["num_particles"] as usize, 1024);
    }

    #[test]
    fn test_spawn_burst() {
        let mut sys = GpuParticleSystem::new(10_000);
        sys.spawn_burst([0.0; 3], 100, 1.0, 3.0);
        assert_eq!(sys.initial_particles.len(), 100);
        for p in &sys.initial_particles {
            assert!(p.is_alive());
        }
    }

    #[test]
    fn test_pipeline_buffers() {
        let sys = GpuParticleSystem::new(1000);
        assert!(sys.pipeline.buffer(sys.particle_buf_a).is_some());
        assert!(sys.pipeline.buffer(sys.attractor_buf).is_some());
        assert!(sys.pipeline.buffer(sys.indirect_buf).is_some());
    }

    #[test]
    fn test_shader_sources_not_empty() {
        assert!(!PARTICLE_INTEGRATE_GLSL.is_empty());
        assert!(!PARTICLE_EMIT_GLSL.is_empty());
        assert!(!FLUID_ADVECT_GLSL.is_empty());
        assert!(!BITONIC_SORT_GLSL.is_empty());
    }

    #[test]
    fn test_compute_pass_desc_builder() {
        let buf_id = BufferId(1);
        let pass = ComputePassDesc::new("test", "#version 430")
            .dispatch(32, 1, 1)
            .bind_ssbo(0, buf_id)
            .set_uniform("num_particles", ComputeUniform::UInt(1000));
        assert_eq!(pass.work_groups, [32, 1, 1]);
        assert_eq!(pass.ssbo_bindings.len(), 1);
        assert!(pass.uniforms.contains_key("num_particles"));
    }
}
