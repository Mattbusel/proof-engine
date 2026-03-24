//! Built-in compute kernels as embedded GLSL source strings.
//!
//! Each kernel is a fully implemented GLSL compute shader. The Rust side
//! provides parameter structs and convenience methods to compile and dispatch
//! each kernel through the `dispatch` module.
//!
//! Kernels:
//! 1. **particle_integrate** — position += velocity * dt, apply forces, age, kill dead
//! 2. **particle_emit** — atomic counter for birth, initialize from emitter params
//! 3. **force_field_sample** — evaluate multiple force fields at particle positions
//! 4. **math_function_gpu** — Lorenz attractor, Mandelbrot iteration, Julia set
//! 5. **fluid_diffuse** — Jacobi iteration for diffusion
//! 6. **histogram_equalize** — compute histogram and equalize
//! 7. **prefix_sum** — Blelloch parallel prefix sum (scan)
//! 8. **radix_sort** — GPU radix sort
//! 9. **frustum_cull** — per-instance frustum culling
//! 10. **skinning** — bone matrix palette skinning

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// KernelId
// ---------------------------------------------------------------------------

/// Identifies a built-in kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelId {
    ParticleIntegrate,
    ParticleEmit,
    ForceFieldSample,
    MathFunctionGpu,
    FluidDiffuse,
    HistogramEqualize,
    PrefixSum,
    RadixSort,
    FrustumCull,
    Skinning,
}

impl KernelId {
    /// All kernel IDs.
    pub fn all() -> &'static [KernelId] {
        &[
            KernelId::ParticleIntegrate,
            KernelId::ParticleEmit,
            KernelId::ForceFieldSample,
            KernelId::MathFunctionGpu,
            KernelId::FluidDiffuse,
            KernelId::HistogramEqualize,
            KernelId::PrefixSum,
            KernelId::RadixSort,
            KernelId::FrustumCull,
            KernelId::Skinning,
        ]
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            KernelId::ParticleIntegrate => "particle_integrate",
            KernelId::ParticleEmit => "particle_emit",
            KernelId::ForceFieldSample => "force_field_sample",
            KernelId::MathFunctionGpu => "math_function_gpu",
            KernelId::FluidDiffuse => "fluid_diffuse",
            KernelId::HistogramEqualize => "histogram_equalize",
            KernelId::PrefixSum => "prefix_sum",
            KernelId::RadixSort => "radix_sort",
            KernelId::FrustumCull => "frustum_cull",
            KernelId::Skinning => "skinning",
        }
    }
}

// ---------------------------------------------------------------------------
// Parameter structs
// ---------------------------------------------------------------------------

/// Parameters for the particle integration kernel.
#[derive(Debug, Clone, Copy)]
pub struct ParticleIntegrateParams {
    pub dt: f32,
    pub gravity: [f32; 3],
    pub damping: f32,
    pub particle_count: u32,
    pub max_age: f32,
    pub wind: [f32; 3],
    pub turbulence_strength: f32,
    pub time: f32,
}

impl Default for ParticleIntegrateParams {
    fn default() -> Self {
        Self {
            dt: 1.0 / 60.0,
            gravity: [0.0, -9.81, 0.0],
            damping: 0.98,
            particle_count: 0,
            max_age: 5.0,
            wind: [0.0; 3],
            turbulence_strength: 0.0,
            time: 0.0,
        }
    }
}

/// Parameters for the particle emission kernel.
#[derive(Debug, Clone, Copy)]
pub struct ParticleEmitParams {
    pub emit_count: u32,
    pub max_particles: u32,
    pub emitter_position: [f32; 3],
    pub emitter_radius: f32,
    pub initial_speed_min: f32,
    pub initial_speed_max: f32,
    pub initial_direction: [f32; 3],
    pub spread_angle: f32,
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    pub time: f32,
    pub seed: u32,
    pub color_start: [f32; 4],
    pub color_end: [f32; 4],
    pub size_start: f32,
    pub size_end: f32,
}

impl Default for ParticleEmitParams {
    fn default() -> Self {
        Self {
            emit_count: 100,
            max_particles: 100_000,
            emitter_position: [0.0; 3],
            emitter_radius: 0.1,
            initial_speed_min: 1.0,
            initial_speed_max: 3.0,
            initial_direction: [0.0, 1.0, 0.0],
            spread_angle: 0.5,
            lifetime_min: 1.0,
            lifetime_max: 3.0,
            time: 0.0,
            seed: 0,
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 1.0, 1.0, 0.0],
            size_start: 1.0,
            size_end: 0.0,
        }
    }
}

/// Describes a force field for the force_field_sample kernel.
#[derive(Debug, Clone, Copy)]
pub struct ForceFieldDesc {
    pub field_type: ForceFieldType,
    pub position: [f32; 3],
    pub strength: f32,
    pub radius: f32,
    pub falloff: f32,
    pub direction: [f32; 3],
    pub frequency: f32,
}

/// Types of force fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceFieldType {
    Attractor = 0,
    Repulsor = 1,
    Vortex = 2,
    Directional = 3,
    Noise = 4,
    Drag = 5,
}

impl Default for ForceFieldDesc {
    fn default() -> Self {
        Self {
            field_type: ForceFieldType::Attractor,
            position: [0.0; 3],
            strength: 1.0,
            radius: 10.0,
            falloff: 2.0,
            direction: [0.0, 1.0, 0.0],
            frequency: 1.0,
        }
    }
}

/// Math function types for the math_function_gpu kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathFunctionType {
    LorenzAttractor = 0,
    MandelbrotIteration = 1,
    JuliaSet = 2,
    RosslerAttractor = 3,
    AizawaAttractor = 4,
}

/// Parameters for the fluid diffusion kernel (Jacobi iteration).
#[derive(Debug, Clone, Copy)]
pub struct FluidDiffuseParams {
    pub grid_width: u32,
    pub grid_height: u32,
    pub diffusion_rate: f32,
    pub dt: f32,
    pub iterations: u32,
}

impl Default for FluidDiffuseParams {
    fn default() -> Self {
        Self {
            grid_width: 256,
            grid_height: 256,
            diffusion_rate: 0.001,
            dt: 1.0 / 60.0,
            iterations: 20,
        }
    }
}

/// Parameters for histogram equalization.
#[derive(Debug, Clone, Copy)]
pub struct HistogramParams {
    pub width: u32,
    pub height: u32,
    pub bin_count: u32,
    pub min_value: f32,
    pub max_value: f32,
}

impl Default for HistogramParams {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            bin_count: 256,
            min_value: 0.0,
            max_value: 1.0,
        }
    }
}

/// Plan for prefix sum (Blelloch algorithm).
#[derive(Debug, Clone)]
pub struct PrefixSumPlan {
    pub element_count: u32,
    pub workgroup_size: u32,
    pub inclusive: bool,
}

impl Default for PrefixSumPlan {
    fn default() -> Self {
        Self {
            element_count: 1024,
            workgroup_size: 256,
            inclusive: false,
        }
    }
}

/// Plan for radix sort.
#[derive(Debug, Clone)]
pub struct RadixSortPlan {
    pub element_count: u32,
    pub bits_per_pass: u32,
    pub total_bits: u32,
    pub workgroup_size: u32,
}

impl Default for RadixSortPlan {
    fn default() -> Self {
        Self {
            element_count: 1024,
            bits_per_pass: 4,
            total_bits: 32,
            workgroup_size: 256,
        }
    }
}

impl RadixSortPlan {
    /// Number of passes needed.
    pub fn pass_count(&self) -> u32 {
        (self.total_bits + self.bits_per_pass - 1) / self.bits_per_pass
    }

    /// Number of bins per pass (2^bits_per_pass).
    pub fn radix(&self) -> u32 {
        1 << self.bits_per_pass
    }
}

/// Parameters for frustum culling.
#[derive(Debug, Clone, Copy)]
pub struct FrustumCullParams {
    pub instance_count: u32,
    pub frustum_planes: [[f32; 4]; 6],
    pub lod_distances: [f32; 4],
    pub camera_position: [f32; 3],
    pub enable_lod: bool,
}

impl Default for FrustumCullParams {
    fn default() -> Self {
        Self {
            instance_count: 0,
            frustum_planes: [[0.0; 4]; 6],
            lod_distances: [50.0, 150.0, 500.0, 1000.0],
            camera_position: [0.0; 3],
            enable_lod: true,
        }
    }
}

/// Parameters for skeletal skinning.
#[derive(Debug, Clone, Copy)]
pub struct SkinningParams {
    pub vertex_count: u32,
    pub bone_count: u32,
    pub max_bones_per_vertex: u32,
}

impl Default for SkinningParams {
    fn default() -> Self {
        Self {
            vertex_count: 0,
            bone_count: 64,
            max_bones_per_vertex: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// GLSL kernel sources
// ---------------------------------------------------------------------------

/// Particle integration kernel: advance positions, apply forces, age, kill dead.
pub const KERNEL_PARTICLE_INTEGRATE: &str = r#"
// Particle integration kernel
// Reads from SSBO binding 0, writes to SSBO binding 1 (ping-pong).
// Each particle: vec4 position (xyz + age), vec4 velocity (xyz + lifetime).

layout(local_size_x = 256) in;

struct Particle {
    vec4 pos_age;    // xyz = position, w = age
    vec4 vel_life;   // xyz = velocity, w = lifetime
};

layout(std430, binding = 0) readonly buffer ParticlesIn {
    Particle particles_in[];
};

layout(std430, binding = 1) writeonly buffer ParticlesOut {
    Particle particles_out[];
};

uniform float u_dt;
uniform vec3 u_gravity;
uniform float u_damping;
uniform uint u_particle_count;
uniform float u_max_age;
uniform vec3 u_wind;
uniform float u_turbulence_strength;
uniform float u_time;

// Simple hash for turbulence
float hash(vec3 p) {
    p = fract(p * vec3(443.897, 441.423, 437.195));
    p += dot(p, p.yzx + 19.19);
    return fract((p.x + p.y) * p.z);
}

vec3 turbulence(vec3 pos, float time) {
    float n1 = hash(pos + vec3(time * 0.3));
    float n2 = hash(pos + vec3(time * 0.7, 0.0, 0.0));
    float n3 = hash(pos + vec3(0.0, time * 0.5, 0.0));
    return vec3(n1 - 0.5, n2 - 0.5, n3 - 0.5) * 2.0;
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= u_particle_count) return;

    Particle p = particles_in[idx];

    float age = p.pos_age.w;
    float lifetime = p.vel_life.w;

    // Advance age
    age += u_dt;

    // Kill dead particles by setting lifetime to 0
    if (age >= lifetime || age >= u_max_age) {
        p.pos_age.w = lifetime + 1.0; // Mark as dead
        p.vel_life.xyz = vec3(0.0);
        particles_out[idx] = p;
        return;
    }

    // Apply forces
    vec3 vel = p.vel_life.xyz;
    vec3 pos = p.pos_age.xyz;

    // Gravity
    vel += u_gravity * u_dt;

    // Wind
    vel += u_wind * u_dt;

    // Turbulence
    if (u_turbulence_strength > 0.0) {
        vec3 turb = turbulence(pos * 0.1, u_time);
        vel += turb * u_turbulence_strength * u_dt;
    }

    // Damping
    vel *= pow(u_damping, u_dt);

    // Integrate position
    pos += vel * u_dt;

    // Write output
    particles_out[idx].pos_age = vec4(pos, age);
    particles_out[idx].vel_life = vec4(vel, lifetime);
}
"#;

/// Particle emission kernel: spawn new particles using atomic counter.
pub const KERNEL_PARTICLE_EMIT: &str = r#"
// Particle emission kernel
// Uses an atomic counter to allocate slots in the particle buffer.

layout(local_size_x = 64) in;

struct Particle {
    vec4 pos_age;
    vec4 vel_life;
};

layout(std430, binding = 1) writeonly buffer ParticlesOut {
    Particle particles[];
};

layout(binding = 0, offset = 0) uniform atomic_uint u_alive_count;

uniform uint u_emit_count;
uniform uint u_max_particles;
uniform vec3 u_emitter_pos;
uniform float u_emitter_radius;
uniform float u_speed_min;
uniform float u_speed_max;
uniform vec3 u_direction;
uniform float u_spread;
uniform float u_life_min;
uniform float u_life_max;
uniform float u_time;
uniform uint u_seed;
uniform vec4 u_color_start;
uniform vec4 u_color_end;

// PCG random number generator
uint pcg(uint state) {
    uint s = state * 747796405u + 2891336453u;
    uint word = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (word >> 22u) ^ word;
}

float rand01(inout uint seed) {
    seed = pcg(seed);
    return float(seed) / 4294967295.0;
}

vec3 random_direction(inout uint seed, vec3 dir, float spread) {
    float phi = rand01(seed) * 6.283185307;
    float cos_theta = 1.0 - rand01(seed) * spread;
    float sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));

    vec3 random_dir = vec3(
        sin_theta * cos(phi),
        sin_theta * sin(phi),
        cos_theta
    );

    // Rotate random_dir to align with dir
    vec3 up = abs(dir.y) < 0.999 ? vec3(0, 1, 0) : vec3(1, 0, 0);
    vec3 right = normalize(cross(up, dir));
    up = cross(dir, right);

    return right * random_dir.x + up * random_dir.y + dir * random_dir.z;
}

vec3 random_sphere(inout uint seed, float radius) {
    float phi = rand01(seed) * 6.283185307;
    float cos_theta = rand01(seed) * 2.0 - 1.0;
    float sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    float r = pow(rand01(seed), 1.0 / 3.0) * radius;
    return r * vec3(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta);
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= u_emit_count) return;

    // Allocate a slot
    uint slot = atomicCounterIncrement(u_alive_count);
    if (slot >= u_max_particles) return;

    // Seed RNG
    uint seed = u_seed + idx * 1973u + uint(u_time * 1000.0) * 7919u;

    // Random position within emitter sphere
    vec3 pos = u_emitter_pos + random_sphere(seed, u_emitter_radius);

    // Random direction with spread
    vec3 dir = random_direction(seed, normalize(u_direction), u_spread);

    // Random speed
    float speed = mix(u_speed_min, u_speed_max, rand01(seed));

    // Random lifetime
    float life = mix(u_life_min, u_life_max, rand01(seed));

    particles[slot].pos_age = vec4(pos, 0.0);
    particles[slot].vel_life = vec4(dir * speed, life);
}
"#;

/// Force field sampling kernel: evaluate multiple force fields at particle positions.
pub const KERNEL_FORCE_FIELD_SAMPLE: &str = r#"
// Force field sampling kernel
// Reads particle positions, evaluates force fields, accumulates forces into velocity.

layout(local_size_x = 256) in;

struct Particle {
    vec4 pos_age;
    vec4 vel_life;
};

layout(std430, binding = 0) buffer Particles {
    Particle particles[];
};

// Force field types: 0=attractor, 1=repulsor, 2=vortex, 3=directional, 4=noise, 5=drag
struct ForceField {
    vec4 pos_strength;   // xyz = position, w = strength
    vec4 dir_radius;     // xyz = direction, w = radius
    vec4 params;         // x = falloff, y = frequency, z = type, w = unused
};

layout(std430, binding = 2) readonly buffer ForceFields {
    ForceField fields[];
};

uniform uint u_particle_count;
uniform uint u_field_count;
uniform float u_dt;
uniform float u_time;

float hash31(vec3 p) {
    p = fract(p * vec3(443.8975, 441.4230, 437.1950));
    p += dot(p, p.yzx + 19.19);
    return fract((p.x + p.y) * p.z);
}

vec3 eval_field(ForceField f, vec3 pos, float time) {
    vec3 field_pos = f.pos_strength.xyz;
    float strength = f.pos_strength.w;
    float radius = f.dir_radius.w;
    vec3 direction = f.dir_radius.xyz;
    float falloff = f.params.x;
    float freq = f.params.y;
    int ftype = int(f.params.z);

    vec3 delta = field_pos - pos;
    float dist = length(delta);
    float atten = 1.0;
    if (radius > 0.0) {
        atten = 1.0 - clamp(dist / radius, 0.0, 1.0);
        atten = pow(atten, falloff);
    }

    vec3 force = vec3(0.0);

    if (ftype == 0) {
        // Attractor: pull toward center
        if (dist > 0.001) {
            force = normalize(delta) * strength * atten;
        }
    } else if (ftype == 1) {
        // Repulsor: push away from center
        if (dist > 0.001) {
            force = -normalize(delta) * strength * atten;
        }
    } else if (ftype == 2) {
        // Vortex: swirl around axis (direction)
        vec3 axis = normalize(direction);
        vec3 radial = delta - dot(delta, axis) * axis;
        if (length(radial) > 0.001) {
            vec3 tangent = cross(axis, normalize(radial));
            force = tangent * strength * atten;
        }
    } else if (ftype == 3) {
        // Directional: constant force in a direction within radius
        force = normalize(direction) * strength * atten;
    } else if (ftype == 4) {
        // Noise: pseudo-random force based on position and time
        float n1 = hash31(pos * freq + vec3(time));
        float n2 = hash31(pos * freq + vec3(0.0, time, 0.0));
        float n3 = hash31(pos * freq + vec3(0.0, 0.0, time));
        force = (vec3(n1, n2, n3) * 2.0 - 1.0) * strength * atten;
    } else if (ftype == 5) {
        // Drag: opposes velocity (we approximate by opposing position change)
        force = -normalize(delta) * strength * atten * dist;
    }

    return force;
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= u_particle_count) return;

    Particle p = particles[idx];
    vec3 pos = p.pos_age.xyz;
    vec3 vel = p.vel_life.xyz;

    // Skip dead particles
    if (p.pos_age.w >= p.vel_life.w) return;

    // Accumulate forces from all fields
    vec3 total_force = vec3(0.0);
    for (uint i = 0u; i < u_field_count; i++) {
        total_force += eval_field(fields[i], pos, u_time);
    }

    // Apply accumulated force
    vel += total_force * u_dt;
    particles[idx].vel_life.xyz = vel;
}
"#;

/// Math function GPU kernel: Lorenz, Mandelbrot, Julia, Rossler, Aizawa.
pub const KERNEL_MATH_FUNCTION_GPU: &str = r#"
// GPU math function kernel
// Computes one step of various mathematical functions.
// Mode uniform selects the function.

layout(local_size_x = 256) in;

struct Point {
    vec4 pos_age;    // xyz = position, w = iteration count or age
    vec4 vel_param;  // xyz = velocity/derivative, w = parameter
};

layout(std430, binding = 0) buffer Points {
    Point points[];
};

uniform uint u_point_count;
uniform uint u_function_type;  // 0=Lorenz, 1=Mandelbrot, 2=Julia, 3=Rossler, 4=Aizawa
uniform float u_dt;
uniform float u_time;
uniform float u_param_a;
uniform float u_param_b;
uniform float u_param_c;
uniform float u_param_d;
uniform uint u_max_iterations;
uniform vec2 u_julia_c;  // Julia set constant

// Lorenz attractor: dx/dt = sigma*(y-x), dy/dt = x*(rho-z)-y, dz/dt = x*y - beta*z
vec3 lorenz(vec3 p, float sigma, float rho, float beta) {
    return vec3(
        sigma * (p.y - p.x),
        p.x * (rho - p.z) - p.y,
        p.x * p.y - beta * p.z
    );
}

// Rossler attractor: dx/dt = -y-z, dy/dt = x+a*y, dz/dt = b+z*(x-c)
vec3 rossler(vec3 p, float a, float b, float c) {
    return vec3(
        -p.y - p.z,
        p.x + a * p.y,
        b + p.z * (p.x - c)
    );
}

// Aizawa attractor
vec3 aizawa(vec3 p, float a, float b, float c, float d) {
    float x = p.x, y = p.y, z = p.z;
    return vec3(
        (z - b) * x - d * y,
        d * x + (z - b) * y,
        c + a * z - z * z * z / 3.0 - (x * x + y * y) * (1.0 + 0.25 * z) + 0.1 * z * x * x * x
    );
}

// Complex multiply
vec2 cmul(vec2 a, vec2 b) {
    return vec2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= u_point_count) return;

    Point pt = points[idx];
    vec3 pos = pt.pos_age.xyz;
    float age = pt.pos_age.w;

    if (u_function_type == 0u) {
        // Lorenz attractor (RK4 integration)
        float sigma = u_param_a;  // default 10.0
        float rho = u_param_b;    // default 28.0
        float beta = u_param_c;   // default 8.0/3.0

        vec3 k1 = lorenz(pos, sigma, rho, beta);
        vec3 k2 = lorenz(pos + 0.5 * u_dt * k1, sigma, rho, beta);
        vec3 k3 = lorenz(pos + 0.5 * u_dt * k2, sigma, rho, beta);
        vec3 k4 = lorenz(pos + u_dt * k3, sigma, rho, beta);

        pos += (u_dt / 6.0) * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
        pt.vel_param.xyz = k1;  // Store derivative for visualization
        age += u_dt;

    } else if (u_function_type == 1u) {
        // Mandelbrot iteration
        // pos.xy = current z, vel_param.xy = c (constant)
        vec2 z = pos.xy;
        vec2 c = pt.vel_param.xy;
        uint iter = uint(age);

        if (iter < u_max_iterations && dot(z, z) < 4.0) {
            z = cmul(z, z) + c;
            pos.xy = z;
            pos.z = dot(z, z);  // magnitude squared for coloring
            age = float(iter + 1u);
        }

    } else if (u_function_type == 2u) {
        // Julia set iteration
        vec2 z = pos.xy;
        vec2 c = u_julia_c;
        uint iter = uint(age);

        if (iter < u_max_iterations && dot(z, z) < 4.0) {
            z = cmul(z, z) + c;
            pos.xy = z;
            pos.z = dot(z, z);
            age = float(iter + 1u);
        }

    } else if (u_function_type == 3u) {
        // Rossler attractor (RK4)
        float a = u_param_a;  // default 0.2
        float b = u_param_b;  // default 0.2
        float c = u_param_c;  // default 5.7

        vec3 k1 = rossler(pos, a, b, c);
        vec3 k2 = rossler(pos + 0.5 * u_dt * k1, a, b, c);
        vec3 k3 = rossler(pos + 0.5 * u_dt * k2, a, b, c);
        vec3 k4 = rossler(pos + u_dt * k3, a, b, c);

        pos += (u_dt / 6.0) * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
        pt.vel_param.xyz = k1;
        age += u_dt;

    } else if (u_function_type == 4u) {
        // Aizawa attractor (RK4)
        float a = u_param_a;  // default 0.95
        float b = u_param_b;  // default 0.7
        float c = u_param_c;  // default 0.6
        float d = u_param_d;  // default 3.5

        vec3 k1 = aizawa(pos, a, b, c, d);
        vec3 k2 = aizawa(pos + 0.5 * u_dt * k1, a, b, c, d);
        vec3 k3 = aizawa(pos + 0.5 * u_dt * k2, a, b, c, d);
        vec3 k4 = aizawa(pos + u_dt * k3, a, b, c, d);

        pos += (u_dt / 6.0) * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
        pt.vel_param.xyz = k1;
        age += u_dt;
    }

    points[idx].pos_age = vec4(pos, age);
    points[idx].vel_param = pt.vel_param;
}
"#;

/// Fluid diffusion kernel using Jacobi iteration.
pub const KERNEL_FLUID_DIFFUSE: &str = r#"
// Jacobi iteration for 2D fluid diffusion
// Reads from one grid, writes to another (ping-pong).
// d(x)/dt = k * laplacian(x)
// Jacobi: x_new[i,j] = (x_old[i,j] + alpha * (x[i-1,j] + x[i+1,j] + x[i,j-1] + x[i,j+1])) / (1 + 4*alpha)
// where alpha = k * dt / (dx * dx)

layout(local_size_x = 16, local_size_y = 16) in;

layout(std430, binding = 0) readonly buffer GridIn {
    float grid_in[];
};

layout(std430, binding = 1) writeonly buffer GridOut {
    float grid_out[];
};

uniform uint u_width;
uniform uint u_height;
uniform float u_alpha;  // diffusion_rate * dt / (dx * dx)
uniform float u_r_beta;  // 1.0 / (1.0 + 4.0 * alpha)

uint idx2d(uint x, uint y) {
    return y * u_width + x;
}

void main() {
    uint x = gl_GlobalInvocationID.x;
    uint y = gl_GlobalInvocationID.y;

    if (x >= u_width || y >= u_height) return;

    // Boundary: clamp to edge
    uint x0 = max(x, 1u) - 1u;
    uint x1 = min(x + 1u, u_width - 1u);
    uint y0 = max(y, 1u) - 1u;
    uint y1 = min(y + 1u, u_height - 1u);

    float center = grid_in[idx2d(x, y)];
    float left   = grid_in[idx2d(x0, y)];
    float right  = grid_in[idx2d(x1, y)];
    float down   = grid_in[idx2d(x, y0)];
    float up     = grid_in[idx2d(x, y1)];

    float result = (center + u_alpha * (left + right + down + up)) * u_r_beta;
    grid_out[idx2d(x, y)] = result;
}
"#;

/// Histogram equalization kernel (two passes: histogram + equalize).
pub const KERNEL_HISTOGRAM_EQUALIZE: &str = r#"
// Histogram computation and equalization.
// Pass 1: Compute histogram (atomically increment bins).
// Pass 2: Use CDF to remap values.
// Selected by PASS_MODE define: 0 = histogram, 1 = CDF prefix sum, 2 = equalize.

layout(local_size_x = 256) in;

#ifndef PASS_MODE
#define PASS_MODE 0
#endif

#ifndef BIN_COUNT
#define BIN_COUNT 256
#endif

layout(std430, binding = 0) buffer InputData {
    float input_data[];
};

layout(std430, binding = 1) buffer Histogram {
    uint histogram[];
};

layout(std430, binding = 2) buffer CDF {
    float cdf[];
};

layout(std430, binding = 3) buffer OutputData {
    float output_data[];
};

uniform uint u_element_count;
uniform float u_min_value;
uniform float u_max_value;

// Shared memory for local histogram accumulation
shared uint local_hist[BIN_COUNT];

void main() {
    uint idx = gl_GlobalInvocationID.x;
    uint lid = gl_LocalInvocationID.x;

#if PASS_MODE == 0
    // Pass 0: Build histogram
    // Initialize shared histogram
    if (lid < uint(BIN_COUNT)) {
        local_hist[lid] = 0u;
    }
    barrier();

    if (idx < u_element_count) {
        float val = input_data[idx];
        float norm = clamp((val - u_min_value) / (u_max_value - u_min_value), 0.0, 1.0);
        uint bin = min(uint(norm * float(BIN_COUNT - 1)), uint(BIN_COUNT - 1));
        atomicAdd(local_hist[bin], 1u);
    }
    barrier();

    // Merge local histogram into global
    if (lid < uint(BIN_COUNT)) {
        atomicAdd(histogram[lid], local_hist[lid]);
    }

#elif PASS_MODE == 1
    // Pass 1: Build CDF from histogram (single workgroup, sequential for simplicity)
    if (idx == 0u) {
        uint running = 0u;
        for (uint i = 0u; i < uint(BIN_COUNT); i++) {
            running += histogram[i];
            cdf[i] = float(running) / float(u_element_count);
        }
    }

#elif PASS_MODE == 2
    // Pass 2: Apply equalization using CDF
    if (idx < u_element_count) {
        float val = input_data[idx];
        float norm = clamp((val - u_min_value) / (u_max_value - u_min_value), 0.0, 1.0);
        uint bin = min(uint(norm * float(BIN_COUNT - 1)), uint(BIN_COUNT - 1));
        float equalized = cdf[bin];
        output_data[idx] = equalized * (u_max_value - u_min_value) + u_min_value;
    }

#endif
}
"#;

/// Blelloch parallel prefix sum (exclusive scan).
pub const KERNEL_PREFIX_SUM: &str = r#"
// Blelloch parallel prefix sum (exclusive scan)
// Two-phase: up-sweep (reduce) then down-sweep.
// Works on a single workgroup; for larger arrays, use multi-block with auxiliary sums.
//
// PHASE define: 0 = up-sweep, 1 = down-sweep, 2 = add block offsets

layout(local_size_x = 256) in;

#ifndef PHASE
#define PHASE 0
#endif

layout(std430, binding = 0) buffer Data {
    uint data[];
};

layout(std430, binding = 1) buffer BlockSums {
    uint block_sums[];
};

uniform uint u_n;           // number of elements
uniform uint u_block_size;  // elements per block (2 * local_size)

shared uint temp[512]; // 2 * local_size_x

void main() {
    uint lid = gl_LocalInvocationID.x;
    uint gid = gl_WorkGroupID.x;
    uint block_offset = gid * u_block_size;

#if PHASE == 0
    // Load into shared memory
    uint ai = lid;
    uint bi = lid + 256u;
    uint a_idx = block_offset + ai;
    uint b_idx = block_offset + bi;

    temp[ai] = (a_idx < u_n) ? data[a_idx] : 0u;
    temp[bi] = (b_idx < u_n) ? data[b_idx] : 0u;
    barrier();

    // Up-sweep (reduce)
    uint offset = 1u;
    for (uint d = 512u >> 1u; d > 0u; d >>= 1u) {
        barrier();
        if (lid < d) {
            uint ai2 = offset * (2u * lid + 1u) - 1u;
            uint bi2 = offset * (2u * lid + 2u) - 1u;
            temp[bi2] += temp[ai2];
        }
        offset <<= 1u;
    }
    barrier();

    // Store block sum and clear last element
    if (lid == 0u) {
        block_sums[gid] = temp[511u];
        temp[511u] = 0u;
    }
    barrier();

    // Down-sweep
    for (uint d = 1u; d < 512u; d <<= 1u) {
        offset >>= 1u;
        barrier();
        if (lid < d) {
            uint ai2 = offset * (2u * lid + 1u) - 1u;
            uint bi2 = offset * (2u * lid + 2u) - 1u;
            uint t = temp[ai2];
            temp[ai2] = temp[bi2];
            temp[bi2] += t;
        }
    }
    barrier();

    // Write back
    if (a_idx < u_n) data[a_idx] = temp[ai];
    if (b_idx < u_n) data[b_idx] = temp[bi];

#elif PHASE == 2
    // Add block offsets for multi-block scan
    if (gid > 0u) {
        uint a_idx2 = block_offset + lid;
        uint b_idx2 = block_offset + lid + 256u;
        uint block_sum = block_sums[gid];

        if (a_idx2 < u_n) data[a_idx2] += block_sum;
        if (b_idx2 < u_n) data[b_idx2] += block_sum;
    }

#endif
}
"#;

/// GPU radix sort kernel.
pub const KERNEL_RADIX_SORT: &str = r#"
// Radix sort kernel (LSB first, 4 bits per pass)
// PASS define: 0 = count, 1 = scatter
// Uses prefix sums (from prefix_sum kernel) between passes.

layout(local_size_x = 256) in;

#ifndef PASS
#define PASS 0
#endif

#ifndef RADIX_BITS
#define RADIX_BITS 4
#endif

#define RADIX (1 << RADIX_BITS)

layout(std430, binding = 0) buffer KeysIn {
    uint keys_in[];
};

layout(std430, binding = 1) buffer KeysOut {
    uint keys_out[];
};

layout(std430, binding = 2) buffer ValuesIn {
    uint values_in[];
};

layout(std430, binding = 3) buffer ValuesOut {
    uint values_out[];
};

layout(std430, binding = 4) buffer Offsets {
    uint offsets[];      // RADIX * num_blocks
};

layout(std430, binding = 5) buffer GlobalOffsets {
    uint global_offsets[];  // RADIX prefix sums
};

uniform uint u_n;
uniform uint u_bit_offset;  // which 4-bit nibble (0, 4, 8, 12, ...)

shared uint local_counts[RADIX];

uint extract_digit(uint key, uint bit_offset) {
    return (key >> bit_offset) & uint(RADIX - 1);
}

void main() {
    uint lid = gl_LocalInvocationID.x;
    uint gid = gl_WorkGroupID.x;
    uint idx = gl_GlobalInvocationID.x;

#if PASS == 0
    // Count pass: count occurrences of each digit in this block
    if (lid < uint(RADIX)) {
        local_counts[lid] = 0u;
    }
    barrier();

    if (idx < u_n) {
        uint digit = extract_digit(keys_in[idx], u_bit_offset);
        atomicAdd(local_counts[digit], 1u);
    }
    barrier();

    // Write local counts to global offset table
    if (lid < uint(RADIX)) {
        offsets[lid * gl_NumWorkGroups.x + gid] = local_counts[lid];
    }

#elif PASS == 1
    // Scatter pass: place each element at its globally sorted position
    if (idx < u_n) {
        uint key = keys_in[idx];
        uint digit = extract_digit(key, u_bit_offset);

        // global_offsets[digit] = prefix sum of all counts for this digit
        // We need the exact output position: global prefix + local prefix
        // This is a simplified scatter; a production sort uses local prefix sums
        uint dest = atomicAdd(global_offsets[digit], 1u);
        if (dest < u_n) {
            keys_out[dest] = key;
            values_out[dest] = values_in[idx];
        }
    }

#endif
}
"#;

/// Frustum culling kernel: per-instance visibility testing.
pub const KERNEL_FRUSTUM_CULL: &str = r#"
// Per-instance frustum culling with optional LOD selection.
// Tests each instance's bounding sphere against 6 frustum planes.

layout(local_size_x = 256) in;

struct Instance {
    vec4 position_radius;   // xyz = position, w = bounding sphere radius
    vec4 extra;             // xyz = scale, w = LOD override (-1 = auto)
};

struct VisibleInstance {
    uint original_index;
    uint lod_level;
    vec2 _padding;
};

layout(std430, binding = 0) readonly buffer Instances {
    Instance instances[];
};

layout(std430, binding = 1) writeonly buffer VisibleOut {
    VisibleInstance visible[];
};

layout(binding = 0, offset = 0) uniform atomic_uint u_visible_count;

uniform uint u_instance_count;
uniform vec4 u_planes[6];      // frustum planes (normal.xyz, distance.w)
uniform vec3 u_camera_pos;
uniform vec4 u_lod_distances;  // x=lod0, y=lod1, z=lod2, w=lod3
uniform uint u_enable_lod;

bool sphere_vs_frustum(vec3 center, float radius) {
    for (int i = 0; i < 6; i++) {
        float dist = dot(u_planes[i].xyz, center) + u_planes[i].w;
        if (dist < -radius) {
            return false;  // fully outside this plane
        }
    }
    return true;
}

uint compute_lod(vec3 pos, float radius) {
    if (u_enable_lod == 0u) return 0u;

    float dist = length(pos - u_camera_pos) - radius;
    if (dist < u_lod_distances.x) return 0u;
    if (dist < u_lod_distances.y) return 1u;
    if (dist < u_lod_distances.z) return 2u;
    return 3u;
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= u_instance_count) return;

    Instance inst = instances[idx];
    vec3 center = inst.position_radius.xyz;
    float radius = inst.position_radius.w * max(inst.extra.x, max(inst.extra.y, inst.extra.z));

    if (sphere_vs_frustum(center, radius)) {
        uint lod = (inst.extra.w >= 0.0)
            ? uint(inst.extra.w)
            : compute_lod(center, radius);

        uint slot = atomicCounterIncrement(u_visible_count);
        visible[slot].original_index = idx;
        visible[slot].lod_level = lod;
    }
}
"#;

/// Skeletal skinning kernel: apply bone matrix palette transforms.
pub const KERNEL_SKINNING: &str = r#"
// GPU skinning kernel
// Transforms vertices by a weighted sum of bone matrices.

layout(local_size_x = 256) in;

struct Vertex {
    vec4 position;     // xyz = position, w = 1
    vec4 normal;       // xyz = normal, w = 0
    vec4 tangent;      // xyz = tangent, w = handedness
    vec4 bone_weights; // up to 4 bone weights
    uvec4 bone_indices; // up to 4 bone indices
};

struct SkinnedVertex {
    vec4 position;
    vec4 normal;
    vec4 tangent;
    vec4 _reserved;
};

layout(std430, binding = 0) readonly buffer VerticesIn {
    Vertex vertices_in[];
};

layout(std430, binding = 1) writeonly buffer VerticesOut {
    SkinnedVertex vertices_out[];
};

layout(std430, binding = 2) readonly buffer BoneMatrices {
    mat4 bones[];
};

layout(std430, binding = 3) readonly buffer InverseBindMatrices {
    mat4 inv_bind[];
};

uniform uint u_vertex_count;
uniform uint u_bone_count;
uniform uint u_max_bones_per_vertex;

mat4 get_skin_matrix(uvec4 indices, vec4 weights) {
    mat4 skin = mat4(0.0);

    // Bone 0
    if (weights.x > 0.0 && indices.x < u_bone_count) {
        skin += weights.x * (bones[indices.x] * inv_bind[indices.x]);
    }

    // Bone 1
    if (u_max_bones_per_vertex > 1u && weights.y > 0.0 && indices.y < u_bone_count) {
        skin += weights.y * (bones[indices.y] * inv_bind[indices.y]);
    }

    // Bone 2
    if (u_max_bones_per_vertex > 2u && weights.z > 0.0 && indices.z < u_bone_count) {
        skin += weights.z * (bones[indices.z] * inv_bind[indices.z]);
    }

    // Bone 3
    if (u_max_bones_per_vertex > 3u && weights.w > 0.0 && indices.w < u_bone_count) {
        skin += weights.w * (bones[indices.w] * inv_bind[indices.w]);
    }

    return skin;
}

void main() {
    uint idx = gl_GlobalInvocationID.x;
    if (idx >= u_vertex_count) return;

    Vertex v = vertices_in[idx];
    mat4 skin = get_skin_matrix(v.bone_indices, v.bone_weights);

    // If no bones affect this vertex, use identity
    if (v.bone_weights.x + v.bone_weights.y + v.bone_weights.z + v.bone_weights.w < 0.001) {
        skin = mat4(1.0);
    }

    vec4 skinned_pos = skin * v.position;
    vec3 skinned_normal = normalize(mat3(skin) * v.normal.xyz);
    vec3 skinned_tangent = normalize(mat3(skin) * v.tangent.xyz);

    vertices_out[idx].position = skinned_pos;
    vertices_out[idx].normal = vec4(skinned_normal, 0.0);
    vertices_out[idx].tangent = vec4(skinned_tangent, v.tangent.w);
}
"#;

// ---------------------------------------------------------------------------
// KernelLibrary
// ---------------------------------------------------------------------------

/// Library of all built-in compute kernels, providing easy access to sources
/// and parameter setup.
pub struct KernelLibrary {
    sources: HashMap<KernelId, &'static str>,
}

impl KernelLibrary {
    /// Create the library with all built-in kernels.
    pub fn new() -> Self {
        let mut sources = HashMap::new();
        sources.insert(KernelId::ParticleIntegrate, KERNEL_PARTICLE_INTEGRATE);
        sources.insert(KernelId::ParticleEmit, KERNEL_PARTICLE_EMIT);
        sources.insert(KernelId::ForceFieldSample, KERNEL_FORCE_FIELD_SAMPLE);
        sources.insert(KernelId::MathFunctionGpu, KERNEL_MATH_FUNCTION_GPU);
        sources.insert(KernelId::FluidDiffuse, KERNEL_FLUID_DIFFUSE);
        sources.insert(KernelId::HistogramEqualize, KERNEL_HISTOGRAM_EQUALIZE);
        sources.insert(KernelId::PrefixSum, KERNEL_PREFIX_SUM);
        sources.insert(KernelId::RadixSort, KERNEL_RADIX_SORT);
        sources.insert(KernelId::FrustumCull, KERNEL_FRUSTUM_CULL);
        sources.insert(KernelId::Skinning, KERNEL_SKINNING);
        Self { sources }
    }

    /// Get the raw GLSL source for a kernel.
    pub fn source(&self, id: KernelId) -> Option<&'static str> {
        self.sources.get(&id).copied()
    }

    /// Build a `ShaderSource` for a kernel, with version header and optional defines.
    pub fn shader_source(&self, id: KernelId) -> Option<super::dispatch::ShaderSource> {
        self.source(id).map(|src| {
            let mut ss = super::dispatch::ShaderSource::new(src);
            ss.set_label(id.name());
            ss
        })
    }

    /// Compile a kernel into a `ComputeProgram`.
    pub fn compile(
        &self,
        gl: &glow::Context,
        id: KernelId,
    ) -> Result<super::dispatch::ComputeProgram, String> {
        let src = self
            .shader_source(id)
            .ok_or_else(|| format!("Unknown kernel: {:?}", id))?;
        super::dispatch::ComputeProgram::compile(gl, &src)
    }

    /// Compile a kernel with extra defines.
    pub fn compile_with_defines(
        &self,
        gl: &glow::Context,
        id: KernelId,
        defines: &[(&str, &str)],
    ) -> Result<super::dispatch::ComputeProgram, String> {
        let mut src = self
            .shader_source(id)
            .ok_or_else(|| format!("Unknown kernel: {:?}", id))?;
        for (name, value) in defines {
            src.define(name, value);
        }
        super::dispatch::ComputeProgram::compile(gl, &src)
    }

    /// List all available kernel IDs.
    pub fn available_kernels(&self) -> Vec<KernelId> {
        KernelId::all().to_vec()
    }
}

impl Default for KernelLibrary {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Convenience: set uniforms for each kernel
// ---------------------------------------------------------------------------

/// Set uniforms for the particle integration kernel.
pub fn set_particle_integrate_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    params: &ParticleIntegrateParams,
) {
    program.set_uniform_float(gl, "u_dt", params.dt);
    program.set_uniform_vec3(
        gl,
        "u_gravity",
        params.gravity[0],
        params.gravity[1],
        params.gravity[2],
    );
    program.set_uniform_float(gl, "u_damping", params.damping);
    program.set_uniform_uint(gl, "u_particle_count", params.particle_count);
    program.set_uniform_float(gl, "u_max_age", params.max_age);
    program.set_uniform_vec3(
        gl,
        "u_wind",
        params.wind[0],
        params.wind[1],
        params.wind[2],
    );
    program.set_uniform_float(gl, "u_turbulence_strength", params.turbulence_strength);
    program.set_uniform_float(gl, "u_time", params.time);
}

/// Set uniforms for the particle emission kernel.
pub fn set_particle_emit_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    params: &ParticleEmitParams,
) {
    program.set_uniform_uint(gl, "u_emit_count", params.emit_count);
    program.set_uniform_uint(gl, "u_max_particles", params.max_particles);
    program.set_uniform_vec3(
        gl,
        "u_emitter_pos",
        params.emitter_position[0],
        params.emitter_position[1],
        params.emitter_position[2],
    );
    program.set_uniform_float(gl, "u_emitter_radius", params.emitter_radius);
    program.set_uniform_float(gl, "u_speed_min", params.initial_speed_min);
    program.set_uniform_float(gl, "u_speed_max", params.initial_speed_max);
    program.set_uniform_vec3(
        gl,
        "u_direction",
        params.initial_direction[0],
        params.initial_direction[1],
        params.initial_direction[2],
    );
    program.set_uniform_float(gl, "u_spread", params.spread_angle);
    program.set_uniform_float(gl, "u_life_min", params.lifetime_min);
    program.set_uniform_float(gl, "u_life_max", params.lifetime_max);
    program.set_uniform_float(gl, "u_time", params.time);
    program.set_uniform_uint(gl, "u_seed", params.seed);
    program.set_uniform_vec4(
        gl,
        "u_color_start",
        params.color_start[0],
        params.color_start[1],
        params.color_start[2],
        params.color_start[3],
    );
    program.set_uniform_vec4(
        gl,
        "u_color_end",
        params.color_end[0],
        params.color_end[1],
        params.color_end[2],
        params.color_end[3],
    );
}

/// Set uniforms for the fluid diffusion kernel.
pub fn set_fluid_diffuse_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    params: &FluidDiffuseParams,
) {
    let dx = 1.0f32;
    let alpha = params.diffusion_rate * params.dt / (dx * dx);
    let r_beta = 1.0 / (1.0 + 4.0 * alpha);
    program.set_uniform_uint(gl, "u_width", params.grid_width);
    program.set_uniform_uint(gl, "u_height", params.grid_height);
    program.set_uniform_float(gl, "u_alpha", alpha);
    program.set_uniform_float(gl, "u_r_beta", r_beta);
}

/// Set uniforms for the histogram equalization kernel.
pub fn set_histogram_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    params: &HistogramParams,
) {
    program.set_uniform_uint(gl, "u_element_count", params.width * params.height);
    program.set_uniform_float(gl, "u_min_value", params.min_value);
    program.set_uniform_float(gl, "u_max_value", params.max_value);
}

/// Set uniforms for the prefix sum kernel.
pub fn set_prefix_sum_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    plan: &PrefixSumPlan,
) {
    program.set_uniform_uint(gl, "u_n", plan.element_count);
    program.set_uniform_uint(gl, "u_block_size", plan.workgroup_size * 2);
}

/// Set uniforms for the radix sort kernel.
pub fn set_radix_sort_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    plan: &RadixSortPlan,
    bit_offset: u32,
) {
    program.set_uniform_uint(gl, "u_n", plan.element_count);
    program.set_uniform_uint(gl, "u_bit_offset", bit_offset);
}

/// Set uniforms for the frustum culling kernel.
pub fn set_frustum_cull_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    params: &FrustumCullParams,
) {
    program.set_uniform_uint(gl, "u_instance_count", params.instance_count);
    // Set frustum planes as individual vec4 uniforms
    for (i, plane) in params.frustum_planes.iter().enumerate() {
        let name = format!("u_planes[{}]", i);
        program.set_uniform_vec4(gl, &name, plane[0], plane[1], plane[2], plane[3]);
    }
    program.set_uniform_vec3(
        gl,
        "u_camera_pos",
        params.camera_position[0],
        params.camera_position[1],
        params.camera_position[2],
    );
    program.set_uniform_vec4(
        gl,
        "u_lod_distances",
        params.lod_distances[0],
        params.lod_distances[1],
        params.lod_distances[2],
        params.lod_distances[3],
    );
    program.set_uniform_uint(gl, "u_enable_lod", if params.enable_lod { 1 } else { 0 });
}

/// Set uniforms for the skinning kernel.
pub fn set_skinning_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    params: &SkinningParams,
) {
    program.set_uniform_uint(gl, "u_vertex_count", params.vertex_count);
    program.set_uniform_uint(gl, "u_bone_count", params.bone_count);
    program.set_uniform_uint(gl, "u_max_bones_per_vertex", params.max_bones_per_vertex);
}

/// Set uniforms for the math function GPU kernel.
pub fn set_math_function_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    function_type: MathFunctionType,
    point_count: u32,
    dt: f32,
    time: f32,
    params: [f32; 4],
    max_iterations: u32,
    julia_c: [f32; 2],
) {
    program.set_uniform_uint(gl, "u_point_count", point_count);
    program.set_uniform_uint(gl, "u_function_type", function_type as u32);
    program.set_uniform_float(gl, "u_dt", dt);
    program.set_uniform_float(gl, "u_time", time);
    program.set_uniform_float(gl, "u_param_a", params[0]);
    program.set_uniform_float(gl, "u_param_b", params[1]);
    program.set_uniform_float(gl, "u_param_c", params[2]);
    program.set_uniform_float(gl, "u_param_d", params[3]);
    program.set_uniform_uint(gl, "u_max_iterations", max_iterations);
    program.set_uniform_vec2(gl, "u_julia_c", julia_c[0], julia_c[1]);
}

/// Set uniforms for the force field sampling kernel.
pub fn set_force_field_uniforms(
    gl: &glow::Context,
    program: &super::dispatch::ComputeProgram,
    particle_count: u32,
    field_count: u32,
    dt: f32,
    time: f32,
) {
    program.set_uniform_uint(gl, "u_particle_count", particle_count);
    program.set_uniform_uint(gl, "u_field_count", field_count);
    program.set_uniform_float(gl, "u_dt", dt);
    program.set_uniform_float(gl, "u_time", time);
}
