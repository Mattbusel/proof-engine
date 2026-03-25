//! Instanced 3D glyph rendering with PBR G-buffer output.
//!
//! Groups glyphs by character, issues one instanced draw call per unique character.
//! Outputs to a G-buffer: albedo, normal, metallic+roughness, emission.
//! The existing deferred pipeline resolves lighting including SVOGI.

use glam::{Vec2, Vec3, Vec4, Mat4};
use std::collections::HashMap;

use crate::glyph::glyph_mesh::GlyphMeshCache;
use crate::glyph::glyph_materials::GlyphMaterial;

// ── Instance data ───────────────────────────────────────────────────────────

/// Per-instance data for 3D glyph rendering. 80 bytes, GPU-friendly.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Glyph3DInstance {
    /// Column-major 4×4 model matrix.
    pub model_matrix: [f32; 16],
    /// RGBA base color.
    pub base_color: [f32; 4],
    /// Emission intensity.
    pub emission: f32,
    /// Metallic factor [0,1].
    pub metallic: f32,
    /// Roughness factor [0,1].
    pub roughness: f32,
    /// Animation phase (for per-glyph time offset).
    pub animation_phase: f32,
}

const _: () = assert!(std::mem::size_of::<Glyph3DInstance>() == 96);

impl Glyph3DInstance {
    pub fn new(transform: Mat4, material: &GlyphMaterial, phase: f32) -> Self {
        Self {
            model_matrix: transform.to_cols_array(),
            base_color: material.base_color,
            emission: material.emission,
            metallic: material.metallic,
            roughness: material.roughness,
            animation_phase: phase,
        }
    }
}

// ── Batch ───────────────────────────────────────────────────────────────────

/// A batch of instances sharing the same character mesh.
pub struct Glyph3DBatch {
    pub character: char,
    pub instances: Vec<Glyph3DInstance>,
}

impl Glyph3DBatch {
    pub fn instance_count(&self) -> u32 { self.instances.len() as u32 }

    pub fn instance_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.instances)
    }
}

// ── Render config ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Render3DConfig {
    pub enable_3d: bool,
    pub extrusion_depth: f32,
    pub rotation_variation: f32,
    pub scale_pulse_amount: f32,
    pub bevel: bool,
}

impl Default for Render3DConfig {
    fn default() -> Self {
        Self {
            enable_3d: true,
            extrusion_depth: 0.3,
            rotation_variation: 0.05,
            scale_pulse_amount: 0.02,
            bevel: false,
        }
    }
}

// ── Renderer ────────────────────────────────────────────────────────────────

pub struct Glyph3DRenderer {
    per_char_instances: HashMap<char, Vec<Glyph3DInstance>>,
    batches: Vec<Glyph3DBatch>,
    pub config: Render3DConfig,
    frame_count: u64,
}

impl Glyph3DRenderer {
    pub fn new(config: Render3DConfig) -> Self {
        Self {
            per_char_instances: HashMap::new(),
            batches: Vec::new(),
            config,
            frame_count: 0,
        }
    }

    /// Clear instance lists. Call at the start of each frame.
    pub fn begin_frame(&mut self) {
        self.per_char_instances.clear();
        self.batches.clear();
        self.frame_count += 1;
    }

    /// Queue a 3D glyph for rendering.
    pub fn submit_glyph(&mut self, ch: char, transform: Mat4, material: &GlyphMaterial, phase: f32) {
        let final_transform = per_glyph_transform(transform, phase, &self.config);
        let instance = Glyph3DInstance::new(final_transform, material, phase);
        self.per_char_instances.entry(ch).or_default().push(instance);
    }

    /// Build sorted batches from submitted instances.
    pub fn build_batches(&mut self) {
        self.batches.clear();
        let mut chars: Vec<char> = self.per_char_instances.keys().copied().collect();
        chars.sort();

        for ch in chars {
            if let Some(instances) = self.per_char_instances.get(&ch) {
                if !instances.is_empty() {
                    self.batches.push(Glyph3DBatch {
                        character: ch,
                        instances: instances.clone(),
                    });
                }
            }
        }
    }

    /// Get the sorted batches for draw call submission.
    pub fn batches(&self) -> &[Glyph3DBatch] {
        &self.batches
    }

    /// Total instance count across all batches.
    pub fn total_instances(&self) -> usize {
        self.batches.iter().map(|b| b.instances.len()).sum()
    }

    /// Number of draw calls (one per unique character).
    pub fn draw_call_count(&self) -> usize {
        self.batches.len()
    }
}

impl Default for Glyph3DRenderer {
    fn default() -> Self { Self::new(Render3DConfig::default()) }
}

/// Apply per-glyph rotation variation and scale pulsing.
pub fn per_glyph_transform(base: Mat4, phase: f32, config: &Render3DConfig) -> Mat4 {
    // Hash phase to get a deterministic but varied rotation offset
    let hash = (phase * 12345.6789).sin() * 43758.5453;
    let rot_offset = (hash.fract() - 0.5) * config.rotation_variation;

    // Scale pulse
    let pulse = 1.0 + (phase * 3.0).sin() * config.scale_pulse_amount;

    let rotation = Mat4::from_rotation_z(rot_offset)
        * Mat4::from_rotation_y(rot_offset * 0.5);
    let scale = Mat4::from_scale(Vec3::splat(pulse));

    base * rotation * scale
}

// ── GLSL Shaders ────────────────────────────────────────────────────────────

/// Vertex shader for 3D PBR glyph rendering.
pub const GLYPH_3D_VERT: &str = r#"
#version 330 core

// Per-vertex
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;

// Per-instance (model matrix uses 4 attribute slots)
layout(location = 3) in vec4 i_model_col0;
layout(location = 4) in vec4 i_model_col1;
layout(location = 5) in vec4 i_model_col2;
layout(location = 6) in vec4 i_model_col3;
layout(location = 7) in vec4 i_base_color;
layout(location = 8) in float i_emission;
layout(location = 9) in float i_metallic;
layout(location = 10) in float i_roughness;
layout(location = 11) in float i_anim_phase;

uniform mat4 u_view_proj;
uniform float u_time;

out vec3 v_world_pos;
out vec3 v_world_normal;
out vec2 v_uv;
out vec4 v_base_color;
out float v_emission;
out float v_metallic;
out float v_roughness;
out float v_anim_phase;

void main() {
    mat4 model = mat4(i_model_col0, i_model_col1, i_model_col2, i_model_col3);
    vec4 world_pos = model * vec4(a_position, 1.0);
    mat3 normal_mat = transpose(inverse(mat3(model)));

    gl_Position = u_view_proj * world_pos;

    v_world_pos = world_pos.xyz;
    v_world_normal = normalize(normal_mat * a_normal);
    v_uv = a_uv;
    v_base_color = i_base_color;
    v_emission = i_emission;
    v_metallic = i_metallic;
    v_roughness = i_roughness;
    v_anim_phase = i_anim_phase;
}
"#;

/// Fragment shader: G-buffer output for deferred PBR.
pub const GLYPH_3D_FRAG: &str = r#"
#version 330 core

in vec3 v_world_pos;
in vec3 v_world_normal;
in vec2 v_uv;
in vec4 v_base_color;
in float v_emission;
in float v_metallic;
in float v_roughness;
in float v_anim_phase;

uniform float u_time;

// G-buffer outputs
layout(location = 0) out vec4 o_albedo;      // RGB albedo + alpha
layout(location = 1) out vec4 o_normal;       // world-space normal (RGB) + unused
layout(location = 2) out vec4 o_material;     // R=metallic, G=roughness, B=subsurface, A=unused
layout(location = 3) out vec4 o_emission;     // RGB emission color + intensity

void main() {
    // Emission pulsing based on animation phase
    float pulse = 1.0 + sin(v_anim_phase * 3.14159 + u_time * 2.0) * 0.1;
    float final_emission = v_emission * pulse;

    o_albedo = v_base_color;
    o_normal = vec4(normalize(v_world_normal) * 0.5 + 0.5, 1.0);
    o_material = vec4(v_metallic, v_roughness, 0.0, 1.0);
    o_emission = vec4(v_base_color.rgb * final_emission, final_emission);
}
"#;

// ── VAO layout description ──────────────────────────────────────────────────

/// Describes how to set up the vertex attribute layout for 3D glyph rendering.
/// This is a reference for the OpenGL setup code, not executable.
pub struct Glyph3DVaoLayout;

impl Glyph3DVaoLayout {
    /// Vertex stride (Vertex3D: 3+3+2 floats = 32 bytes)
    pub const VERTEX_STRIDE: i32 = 32;
    /// Instance stride (Glyph3DInstance: 16+4+1+1+1+1 floats = 96 bytes)
    pub const INSTANCE_STRIDE: i32 = 96;

    /// Per-vertex attributes (from vertex buffer, divisor = 0)
    pub const VERTEX_ATTRIBS: [(u32, i32, i32); 3] = [
        (0, 3, 0),   // position: vec3 @ offset 0
        (1, 3, 12),  // normal: vec3 @ offset 12
        (2, 2, 24),  // uv: vec2 @ offset 24
    ];

    /// Per-instance attributes (from instance buffer, divisor = 1)
    pub const INSTANCE_ATTRIBS: [(u32, i32, i32); 9] = [
        (3, 4, 0),   // model_col0: vec4 @ offset 0
        (4, 4, 16),  // model_col1: vec4 @ offset 16
        (5, 4, 32),  // model_col2: vec4 @ offset 32
        (6, 4, 48),  // model_col3: vec4 @ offset 48
        (7, 4, 64),  // base_color: vec4 @ offset 64
        (8, 1, 80),  // emission: float @ offset 80
        (9, 1, 84),  // metallic: float @ offset 84
        (10, 1, 88), // roughness: float @ offset 88
        (11, 1, 92), // anim_phase: float @ offset 92
    ];
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_size() {
        assert_eq!(std::mem::size_of::<Glyph3DInstance>(), 96);
    }

    #[test]
    fn submit_and_batch() {
        let mut renderer = Glyph3DRenderer::default();
        renderer.begin_frame();

        let mat = GlyphMaterial::player();
        renderer.submit_glyph('A', Mat4::IDENTITY, &mat, 0.0);
        renderer.submit_glyph('A', Mat4::IDENTITY, &mat, 0.5);
        renderer.submit_glyph('B', Mat4::IDENTITY, &mat, 0.0);

        renderer.build_batches();
        assert_eq!(renderer.draw_call_count(), 2); // A and B
        assert_eq!(renderer.total_instances(), 3);
    }

    #[test]
    fn batches_sorted_by_character() {
        let mut renderer = Glyph3DRenderer::default();
        renderer.begin_frame();

        let mat = GlyphMaterial::player();
        renderer.submit_glyph('Z', Mat4::IDENTITY, &mat, 0.0);
        renderer.submit_glyph('A', Mat4::IDENTITY, &mat, 0.0);

        renderer.build_batches();
        assert_eq!(renderer.batches()[0].character, 'A');
        assert_eq!(renderer.batches()[1].character, 'Z');
    }

    #[test]
    fn per_glyph_transform_applies_variation() {
        let config = Render3DConfig {
            rotation_variation: 0.1,
            scale_pulse_amount: 0.05,
            ..Render3DConfig::default()
        };
        let t1 = per_glyph_transform(Mat4::IDENTITY, 0.0, &config);
        let t2 = per_glyph_transform(Mat4::IDENTITY, 1.0, &config);
        // Different phases should produce different transforms
        assert_ne!(t1.to_cols_array(), t2.to_cols_array());
    }

    #[test]
    fn begin_frame_clears() {
        let mut renderer = Glyph3DRenderer::default();
        renderer.submit_glyph('X', Mat4::IDENTITY, &GlyphMaterial::player(), 0.0);
        renderer.build_batches();
        assert_eq!(renderer.total_instances(), 1);

        renderer.begin_frame();
        renderer.build_batches();
        assert_eq!(renderer.total_instances(), 0);
    }
}
