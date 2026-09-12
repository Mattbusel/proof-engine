//! GPU density entities: a figure described by a handful of bones, rendered
//! as millions of particles that never touch the CPU.
//!
//! A [`GpuDensityEntityData`] is sixteen capsules with a radius, a density
//! weight and a colour, plus a few numbers about how the whole body behaves:
//! how much of it is still bound (`hp`), how it breathes, how tightly its
//! matter holds to the skeleton. That is all that crosses to the GPU each
//! frame: under a kilobyte.
//!
//! Every particle is then derived in the vertex shader from its own instance
//! index. A hash picks a bone in proportion to the bones' weights, a point in
//! that capsule, a phase for its jitter, and whether it is one of the ones
//! that has come loose. Nothing is stored per particle, nothing is uploaded,
//! and the count is whatever the GPU can draw: two million particles is one
//! instanced draw call of two million quads.
//!
//! Loose matter drifts away from the body and fades. Lower `hp` and more of
//! the body is loose, which is what damage looks like on something that is
//! made of matter rather than drawn.
//!
//! This needs nothing past OpenGL 3.3: no compute, no storage buffers.

use glam::Mat4;
use glow::HasContext;

/// The number of bones an entity carries. Fixed so the whole skeleton fits
/// in one uniform array.
pub const MAX_BONES: usize = 16;

/// The most particles one entity is ever drawn with, whatever was asked for.
/// Above this the draw is fill-bound on any GPU and the picture stops
/// improving, because there are more particles than pixels.
pub const MAX_PARTICLES_PER_ENTITY: u32 = 4_000_000;

/// One capsule of the skeleton, laid out for the shader.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuBone {
    /// `(start_x, start_y, end_x, end_y)` in the entity's own space, `y` down.
    pub start_end: [f32; 4],
    /// `(radius, density, weight, unused)`. Particles are shared out among
    /// the bones by `weight`; `density` is kept for the caller's own use.
    pub params: [f32; 4],
    /// RGBA. Near-white or saturated bright colours are drawn emissive.
    pub color: [f32; 4],
}

impl Default for GpuBone {
    fn default() -> Self {
        GpuBone { start_end: [0.0; 4], params: [0.0; 4], color: [0.0; 4] }
    }
}

/// Everything the GPU needs to draw one entity this frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuDensityEntityData {
    /// World position, and the scale from entity space to world units.
    pub position_scale: [f32; 4],
    /// A tint over every bone's colour. White leaves them alone.
    pub color: [f32; 4],
    /// `(hp_ratio, breath_phase, breath_amplitude, density_falloff)`.
    pub params: [f32; 4],
    /// `(bone_count, particle_count, jitter, binding_strength)`.
    pub params2: [f32; 4],
    pub bones: [GpuBone; MAX_BONES],
}

impl GpuDensityEntityData {
    /// How many bones are live.
    pub fn bone_count(&self) -> usize {
        (self.params2[0].max(0.0) as usize).min(MAX_BONES)
    }

    /// How many particles the entity asked to be drawn with.
    pub fn requested_particles(&self) -> u32 {
        self.params2[1].max(0.0).min(u32::MAX as f32) as u32
    }

    /// The particle count actually drawn, under a budget.
    pub fn particle_count(&self, budget: u32) -> u32 {
        self.requested_particles()
            .min(budget)
            .min(MAX_PARTICLES_PER_ENTITY)
    }

    /// The bone weights, and their sum. Bones with no weight get none of
    /// the particles.
    pub fn weights(&self) -> ([f32; MAX_BONES], f32) {
        let mut w = [0.0f32; MAX_BONES];
        let mut total = 0.0;
        for (i, b) in self.bones.iter().enumerate().take(self.bone_count()) {
            w[i] = b.params[2].max(0.0);
            total += w[i];
        }
        (w, total)
    }
}

/// The size of one particle on screen, in pixels, for a count. Fewer
/// particles are drawn bigger so the body still reads as solid.
pub fn particle_pixels(count: u32) -> f32 {
    let n = count.max(1) as f32;
    (1.35 * (2_000_000.0 / n).sqrt()).clamp(1.0, 6.0)
}

/// The alpha of one particle, so that a body of any count sums to about
/// the same brightness.
pub fn particle_alpha(count: u32) -> f32 {
    let n = count.max(1) as f32;
    (400_000.0 / n).clamp(0.03, 0.85)
}

const VERT: &str = r#"
#version 330 core
layout(location = 0) in vec2 v_pos;
layout(location = 1) in vec2 v_uv;

uniform mat4  u_view_proj;
uniform vec4  u_pos_scale;
uniform vec4  u_color;
uniform vec4  u_params;    // hp, breath_phase, breath_amp, falloff
uniform vec4  u_params2;   // bone_count, particle_count, jitter, binding
uniform vec4  u_bones[48]; // per bone: start_end, params, color
uniform float u_time;
uniform vec2  u_px;        // particle size in clip units per w
uniform float u_alpha;

out vec2  f_uv;
out vec4  f_color;
out float f_emission;

// Same hash the glyph pass uses, so the two kinds of matter share a grain.
float hf(uint seed, uint v) {
    uint n = seed * 374761393u + v * 668265263u;
    n ^= (n >> 13u);
    n *= 0x5851F42Du;
    n ^= (n >> 16u);
    return float(n & 0x00FFFFFFu) / float(0x01000000u);
}

void main() {
    uint id = uint(gl_InstanceID);
    int nb = int(clamp(u_params2.x, 1.0, 16.0));

    // Which bone: in proportion to the weights.
    float total = 0.0;
    for (int i = 0; i < nb; ++i) total += max(u_bones[i * 3 + 1].z, 0.0);
    float r = hf(id, 1u) * max(total, 1e-6);
    int b = nb - 1;
    float acc = 0.0;
    for (int i = 0; i < nb; ++i) {
        acc += max(u_bones[i * 3 + 1].z, 0.0);
        if (r <= acc) { b = i; break; }
    }
    vec4 se = u_bones[b * 3];
    vec4 bp = u_bones[b * 3 + 1];
    vec4 bc = u_bones[b * 3 + 2];

    // Where in the capsule. Uniform along the axis, a disc across it, the
    // disc pulled toward the core by the falloff so the surface is a
    // gradient rather than a hard edge.
    float t = hf(id, 2u);
    vec2 axis = se.zw - se.xy;
    vec2 c = se.xy + axis * t;
    float falloff = max(u_params.w, 0.5);
    float rr = bp.x * pow(hf(id, 3u), 0.5 + 0.12 * (falloff - 2.0));
    rr *= 1.0 + (hf(id, 7u) - 0.5) * 0.12;
    float ang = hf(id, 4u) * 6.28318530;
    vec3 local = vec3(c.x + cos(ang) * rr, c.y, sin(ang) * rr * 0.7);

    // Breathing: the chest swells and the rest follows less.
    local.xz *= 1.0 + u_params.z * sin(u_params.y + local.y * 2.0);

    // Damage: the share of matter past hp has come loose. It drifts out
    // from the body's middle and fades, then is reseeded.
    float hp = clamp(u_params.x, 0.0, 1.0);
    float loose = step(hp, hf(id, 5u));
    float age = fract(u_time * 0.12 + hf(id, 6u));
    vec3 away = normalize(vec3(local.x, local.y + 0.4, local.z) + vec3(1e-4));
    local += away * loose * age * 1.6;
    float alpha = u_alpha * mix(1.0, (1.0 - age) * (1.0 - age), loose);

    // Jitter: matter held by springs is never still. The stronger the
    // binding, the smaller the wander.
    float jit = u_params2.z / max(u_params2.w, 1.0) * 6.0;
    float ph = u_time * 2.6 + hf(id, 11u) * 6.28318530;
    local += (vec3(hf(id, 8u), hf(id, 9u), hf(id, 10u)) - 0.5) * jit * (0.6 + 0.4 * sin(ph));

    // Entity space is y-down like the screen; world is y-up.
    vec3 world = u_pos_scale.xyz + vec3(local.x, -local.y, local.z) * u_pos_scale.w;
    vec4 clip = u_view_proj * vec4(world, 1.0);
    clip.xy += v_pos * u_px * clip.w;
    clip.y = -clip.y;
    gl_Position = clip;

    vec3 rgb = mix(bc.rgb, bc.rgb * u_color.rgb, 0.35);
    float bright = max(rgb.r, max(rgb.g, rgb.b));
    f_uv = v_uv;
    f_color = vec4(rgb, alpha * bc.a);
    f_emission = smoothstep(0.72, 1.0, bright) * 0.9 + 0.04;
}
"#;

const FRAG: &str = r#"
#version 330 core
in vec2  f_uv;
in vec4  f_color;
in float f_emission;
layout(location = 0) out vec4 o_color;
layout(location = 1) out vec4 o_emission;
void main() {
    float d = length(f_uv - 0.5) * 2.0;
    float a = (1.0 - smoothstep(0.55, 1.0, d)) * f_color.a;
    if (a < 0.002) discard;
    // Premultiplied, added: a million faint discs sum to a solid body.
    o_color    = vec4(f_color.rgb * a, a);
    o_emission = vec4(f_color.rgb * f_emission * a, a);
}
"#;

#[rustfmt::skip]
const QUAD: [f32; 24] = [
    -0.5,  0.5,  0.0, 1.0,
    -0.5, -0.5,  0.0, 0.0,
     0.5,  0.5,  1.0, 1.0,
    -0.5, -0.5,  0.0, 0.0,
     0.5, -0.5,  1.0, 0.0,
     0.5,  0.5,  1.0, 1.0,
];

/// The GL side: one program, one quad, no per-particle data at all.
pub struct GpuDensityRenderer {
    program: glow::Program,
    vao: glow::VertexArray,
    quad_vbo: glow::Buffer,
    loc_view_proj: Option<glow::UniformLocation>,
    loc_pos_scale: Option<glow::UniformLocation>,
    loc_color: Option<glow::UniformLocation>,
    loc_params: Option<glow::UniformLocation>,
    loc_params2: Option<glow::UniformLocation>,
    loc_bones: Option<glow::UniformLocation>,
    loc_time: Option<glow::UniformLocation>,
    loc_px: Option<glow::UniformLocation>,
    loc_alpha: Option<glow::UniformLocation>,
    /// Particles drawn last frame, for the stats.
    pub drawn: u32,
}

impl GpuDensityRenderer {
    pub unsafe fn new(gl: &glow::Context) -> Self {
        let program = compile(gl, VERT, FRAG);
        let vao = gl.create_vertex_array().expect("density vao");
        gl.bind_vertex_array(Some(vao));
        let quad_vbo = gl.create_buffer().expect("density quad");
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad_vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&QUAD), glow::STATIC_DRAW);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
        gl.enable_vertex_attrib_array(1);
        gl.bind_vertex_array(None);
        let loc = |n: &str| gl.get_uniform_location(program, n);
        Self {
            loc_view_proj: loc("u_view_proj"),
            loc_pos_scale: loc("u_pos_scale"),
            loc_color: loc("u_color"),
            loc_params: loc("u_params"),
            loc_params2: loc("u_params2"),
            loc_bones: loc("u_bones[0]"),
            loc_time: loc("u_time"),
            loc_px: loc("u_px"),
            loc_alpha: loc("u_alpha"),
            program,
            vao,
            quad_vbo,
            drawn: 0,
        }
    }

    /// Draw every entity into whatever framebuffer is bound. The caller has
    /// set the viewport; this sets and restores the blend mode.
    pub unsafe fn draw(
        &mut self,
        gl: &glow::Context,
        entities: &[GpuDensityEntityData],
        budget: u32,
        view_proj: &Mat4,
        viewport: (u32, u32),
        time: f32,
    ) -> u32 {
        self.drawn = 0;
        if entities.is_empty() || budget == 0 {
            return 0;
        }
        let mut draws = 0;
        gl.use_program(Some(self.program));
        gl.bind_vertex_array(Some(self.vao));
        gl.enable(glow::BLEND);
        gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
        gl.uniform_matrix_4_f32_slice(self.loc_view_proj.as_ref(), false, &view_proj.to_cols_array());
        gl.uniform_1_f32(self.loc_time.as_ref(), time);
        let (vw, vh) = (viewport.0.max(1) as f32, viewport.1.max(1) as f32);

        for e in entities {
            let n = e.particle_count(budget);
            if n == 0 || e.bone_count() == 0 {
                continue;
            }
            let px = particle_pixels(n);
            gl.uniform_4_f32_slice(self.loc_pos_scale.as_ref(), &e.position_scale);
            gl.uniform_4_f32_slice(self.loc_color.as_ref(), &e.color);
            gl.uniform_4_f32_slice(self.loc_params.as_ref(), &e.params);
            gl.uniform_4_f32_slice(self.loc_params2.as_ref(), &e.params2);
            let flat: &[f32] = bytemuck::cast_slice(&e.bones);
            gl.uniform_4_f32_slice(self.loc_bones.as_ref(), flat);
            gl.uniform_2_f32(self.loc_px.as_ref(), px * 2.0 / vw, px * 2.0 / vh);
            gl.uniform_1_f32(self.loc_alpha.as_ref(), particle_alpha(n));
            gl.draw_arrays_instanced(glow::TRIANGLES, 0, 6, n as i32);
            self.drawn += n;
            draws += 1;
        }

        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        gl.bind_vertex_array(None);
        draws
    }

    pub unsafe fn destroy(&self, gl: &glow::Context) {
        gl.delete_program(self.program);
        gl.delete_vertex_array(self.vao);
        gl.delete_buffer(self.quad_vbo);
    }
}

unsafe fn compile(gl: &glow::Context, vs_src: &str, fs_src: &str) -> glow::Program {
    let vs = gl.create_shader(glow::VERTEX_SHADER).expect("density vs");
    gl.shader_source(vs, vs_src);
    gl.compile_shader(vs);
    if !gl.get_shader_compile_status(vs) {
        panic!("density vertex shader:\n{}", gl.get_shader_info_log(vs));
    }
    let fs = gl.create_shader(glow::FRAGMENT_SHADER).expect("density fs");
    gl.shader_source(fs, fs_src);
    gl.compile_shader(fs);
    if !gl.get_shader_compile_status(fs) {
        panic!("density fragment shader:\n{}", gl.get_shader_info_log(fs));
    }
    let p = gl.create_program().expect("density program");
    gl.attach_shader(p, vs);
    gl.attach_shader(p, fs);
    gl.link_program(p);
    if !gl.get_program_link_status(p) {
        panic!("density link:\n{}", gl.get_program_info_log(p));
    }
    gl.detach_shader(p, vs);
    gl.detach_shader(p, fs);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    p
}
