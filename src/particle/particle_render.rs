//! GPU particle rendering bridge — renders particles directly from compute SSBO
//! using indirect draw, with zero CPU readback.
//!
//! The render pipeline reads from the SSBO that was written by `particle_update.comp`
//! and draws instanced quads (one per alive particle) using the standard glyph
//! vertex format.  The indirect draw buffer's instance count is written by the
//! compute shader, so the CPU never needs to know how many particles are alive.

use glam::{Vec2, Vec3, Vec4, Mat4};

use super::gpu_particles::{GpuParticle, GpuParticleSystem, GpuIndirectDrawParams};

// ── Particle render instance (GPU-side) ─────────────────────────────────────

/// Per-particle data extracted from the SSBO for rendering.
///
/// In the fully GPU path, this conversion happens in a second compute pass
/// or in the vertex shader itself.  For the hybrid path, this struct is used
/// for CPU-side extraction.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleRenderInstance {
    pub position: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
    pub age_frac: f32,
    pub engine_type: u32,
    pub _pad: [f32; 2],
}

// Verify 48 bytes.
const _: () = assert!(std::mem::size_of::<ParticleRenderInstance>() == 48);

// ── Particle vertex shader (embedded) ───────────────────────────────────────

/// Vertex shader that reads particle data from SSBO and renders instanced quads.
///
/// Each particle is a camera-facing (billboard) quad.  The vertex shader
/// reads position/size/color from the SSBO via gl_InstanceID.
pub const PARTICLE_VERT_SRC: &str = r#"
#version 430 core

// Per-vertex (unit quad)
layout(location = 0) in vec2 v_pos;   // [-0.5, 0.5]
layout(location = 1) in vec2 v_uv;    // [0, 1]

// Particle SSBO (read-only in vertex shader)
struct Particle {
    vec3  position;
    float _pad0;
    vec3  velocity;
    float _pad1;
    vec4  color;
    float life;
    float max_life;
    float size;
    uint  engine_type;
    float seed;
    uint  flags;
    float _reserved0;
    float _reserved1;
};

layout(std430, binding = 0) readonly buffer ParticleBuffer {
    Particle particles[];
};

uniform mat4 u_view_proj;
uniform vec3 u_camera_right;
uniform vec3 u_camera_up;
uniform float u_time;

out vec2 f_uv;
out vec4 f_color;
out float f_age_frac;
out float f_emission;

void main() {
    Particle p = particles[gl_InstanceID];

    // Skip dead particles (alpha will be zero, but also move off-screen).
    if (p.life <= 0.0) {
        gl_Position = vec4(0.0, 0.0, -999.0, 1.0);
        f_color = vec4(0.0);
        f_uv = vec2(0.0);
        f_age_frac = 1.0;
        f_emission = 0.0;
        return;
    }

    float age_frac = 1.0 - clamp(p.life / p.max_life, 0.0, 1.0);

    // Size modulation over lifetime: starts at full, shrinks in last 20%.
    float size_mod = 1.0;
    if (age_frac > 0.8) {
        size_mod = 1.0 - (age_frac - 0.8) * 5.0;
    }
    float particle_size = p.size * size_mod;

    // Billboard: orient quad to face camera.
    vec3 world_pos = p.position
        + u_camera_right * v_pos.x * particle_size
        + u_camera_up    * v_pos.y * particle_size;

    gl_Position = u_view_proj * vec4(world_pos, 1.0);
    gl_Position.y = -gl_Position.y; // FBO Y inversion

    f_uv = v_uv;
    f_color = p.color;
    f_age_frac = age_frac;

    // Emission based on engine type: some engines glow more.
    float base_emission = 0.0;
    if (p.engine_type == 1u) base_emission = 0.5; // Lorenz
    if (p.engine_type == 4u) base_emission = 0.4; // Rossler
    if (p.engine_type == 7u) base_emission = 0.3; // Halvorsen
    f_emission = base_emission * (1.0 - age_frac);
}
"#;

/// Fragment shader for GPU particles.
///
/// Renders a soft circular particle with color from the SSBO.
/// Outputs to dual attachments (color + emission) for the bloom pipeline.
pub const PARTICLE_FRAG_SRC: &str = r#"
#version 430 core

in vec2 f_uv;
in vec4 f_color;
in float f_age_frac;
in float f_emission;

layout(location = 0) out vec4 o_color;
layout(location = 1) out vec4 o_emission;

void main() {
    // Soft circle: distance from center of UV quad.
    vec2 center = f_uv - 0.5;
    float dist = length(center) * 2.0;
    float alpha = smoothstep(1.0, 0.6, dist);

    if (alpha < 0.01) discard;

    vec4 color = f_color;
    color.a *= alpha;

    o_color = color;

    // Emission for bloom.
    float bloom = max(f_emission, 0.0);
    o_emission = vec4(color.rgb * bloom, color.a * bloom);
}
"#;

// ── Render configuration ────────────────────────────────────────────────────

/// Configuration for GPU particle rendering.
#[derive(Clone, Debug)]
pub struct ParticleRenderConfig {
    /// Whether to use indirect draw (GPU-driven instance count).
    pub indirect_draw: bool,
    /// Whether to render particles as billboards (camera-facing quads).
    pub billboard: bool,
    /// Whether to use additive blending (true) or alpha blending (false).
    pub additive_blend: bool,
    /// Maximum render distance (particles beyond this are culled in the vertex shader).
    pub max_render_distance: f32,
    /// Particle character for atlas-based rendering (when not using soft circles).
    pub atlas_char: Option<char>,
    /// Whether to sort particles back-to-front (expensive, only for alpha blend).
    pub depth_sort: bool,
}

impl Default for ParticleRenderConfig {
    fn default() -> Self {
        Self {
            indirect_draw: true,
            billboard: true,
            additive_blend: true,
            max_render_distance: 100.0,
            atlas_char: None,
            depth_sort: false,
        }
    }
}

// ── CPU fallback: extract render instances from particle buffer ──────────────

/// Extract render instances from a CPU-side particle buffer.
///
/// Used when compute shaders are unavailable.  Filters dead particles and
/// produces a compact list of render instances.
pub fn extract_render_instances(particles: &[GpuParticle]) -> Vec<ParticleRenderInstance> {
    let mut instances = Vec::with_capacity(particles.len());
    for p in particles {
        if p.life <= 0.0 {
            continue;
        }
        let age_frac = 1.0 - (p.life / p.max_life).clamp(0.0, 1.0);
        instances.push(ParticleRenderInstance {
            position: p.position,
            size: p.size,
            color: p.color,
            age_frac,
            engine_type: p.engine_type,
            _pad: [0.0; 2],
        });
    }
    instances
}

/// Sort render instances back-to-front relative to the camera position.
pub fn sort_instances_back_to_front(instances: &mut [ParticleRenderInstance], camera_pos: Vec3) {
    instances.sort_by(|a, b| {
        let da = Vec3::from(a.position).distance_squared(camera_pos);
        let db = Vec3::from(b.position).distance_squared(camera_pos);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });
}

// ── LOD system ──────────────────────────────────────────────────────────────

/// LOD tier for particle rendering based on camera distance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleLodTier {
    /// Full quality: all particles rendered.
    Full,
    /// Medium: skip every 2nd particle.
    Medium,
    /// Low: skip every 4th particle.
    Low,
    /// Minimal: skip every 8th particle.
    Minimal,
}

impl ParticleLodTier {
    /// Get the skip stride for this LOD tier.
    pub fn stride(self) -> u32 {
        match self {
            ParticleLodTier::Full => 1,
            ParticleLodTier::Medium => 2,
            ParticleLodTier::Low => 4,
            ParticleLodTier::Minimal => 8,
        }
    }

    /// Determine LOD tier based on camera distance to the particle system's center.
    pub fn from_distance(distance: f32) -> Self {
        if distance < 20.0 {
            ParticleLodTier::Full
        } else if distance < 50.0 {
            ParticleLodTier::Medium
        } else if distance < 100.0 {
            ParticleLodTier::Low
        } else {
            ParticleLodTier::Minimal
        }
    }
}

// ── Depth layer rendering ───────────────────────────────────────────────────

/// Configuration for rendering particles across multiple depth layers.
///
/// Each layer has its own Z range and can have different opacity/blend settings.
#[derive(Clone, Debug)]
pub struct DepthLayerConfig {
    /// Z offset for this layer.
    pub z_offset: f32,
    /// Opacity multiplier for this layer (back layers dimmer).
    pub opacity: f32,
    /// Size multiplier (back layers can be larger for parallax).
    pub size_scale: f32,
}

impl DepthLayerConfig {
    /// Generate layer configs from Z offsets with automatic opacity/size scaling.
    pub fn from_z_offsets(offsets: &[f32]) -> Vec<Self> {
        let count = offsets.len();
        offsets.iter().enumerate().map(|(i, &z)| {
            let depth_frac = i as f32 / (count.max(1) - 1).max(1) as f32;
            Self {
                z_offset: z,
                opacity: 0.4 + 0.6 * (1.0 - depth_frac), // back layers dimmer
                size_scale: 0.8 + 0.4 * depth_frac,       // back layers slightly larger
            }
        }).collect()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_instance_size() {
        assert_eq!(std::mem::size_of::<ParticleRenderInstance>(), 48);
    }

    #[test]
    fn extract_filters_dead() {
        let particles = vec![
            GpuParticle { life: 1.0, max_life: 2.0, size: 1.0, ..GpuParticle::dead() },
            GpuParticle::dead(),
            GpuParticle { life: 0.5, max_life: 1.0, size: 0.5, ..GpuParticle::dead() },
        ];
        let instances = extract_render_instances(&particles);
        assert_eq!(instances.len(), 2);
    }

    #[test]
    fn lod_tiers() {
        assert_eq!(ParticleLodTier::from_distance(5.0), ParticleLodTier::Full);
        assert_eq!(ParticleLodTier::from_distance(30.0), ParticleLodTier::Medium);
        assert_eq!(ParticleLodTier::from_distance(75.0), ParticleLodTier::Low);
        assert_eq!(ParticleLodTier::from_distance(150.0), ParticleLodTier::Minimal);
    }

    #[test]
    fn depth_layers_from_offsets() {
        let layers = DepthLayerConfig::from_z_offsets(&[-5.0, 0.0, 5.0]);
        assert_eq!(layers.len(), 3);
        assert!(layers[0].opacity > layers[2].opacity, "Front should be brighter");
    }
}
