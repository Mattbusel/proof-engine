//! Render curves as thick glowing lines via the glyph pipeline.
//!
//! Since we're using an OpenGL 3.3 glyph instancing pipeline (no geometry shader),
//! we render curves by tessellating them into line segments and placing a small
//! glyph (a dot or dash) at each sample point. Dense sampling + additive blend
//! + bloom creates the appearance of smooth glowing lines.
//!
//! For a full GPU line renderer (geometry shader expanding to quads), the GLSL
//! shaders are included at the bottom for future use.

use glam::{Vec2, Vec3, Vec4};
use crate::glyph::{Glyph, GlyphId, RenderLayer, BlendMode};
use crate::math::MathFunction;
use super::entity_curves::{CurveEntity, EntityCurve};
use super::tessellate::tessellate_curve;

/// Render a CurveEntity into the scene by spawning short-lived glyphs along each curve.
/// Returns the number of glyphs spawned.
pub fn render_curve_entity(
    entity: &CurveEntity,
    spawn_fn: &mut dyn FnMut(Glyph) -> GlyphId,
    dt: f32,
) -> usize {
    let mut count = 0;
    let pos = entity.position;
    let em_mult = entity.emission_mult;

    for curve in &entity.curves {
        if !curve.alive && curve.kinetic_energy() < 0.001 { continue; }

        let polyline = tessellate_curve(curve);
        if polyline.len() < 2 { continue; }

        let base_color = curve.color;
        let thickness = curve.thickness;
        let emission = curve.emission * em_mult;

        // Dash pattern state
        let mut dash_accumulated = 0.0f32;
        let dash = curve.dash_pattern;

        for i in 0..polyline.len() {
            let pt = polyline[i];

            // Dash pattern: skip points during the "off" phase
            if let Some((on, off)) = dash {
                if i > 0 {
                    dash_accumulated += (polyline[i] - polyline[i - 1]).length();
                }
                let cycle = on + off;
                let phase = dash_accumulated % cycle;
                if phase > on { continue; }
            }

            // Color gradient along curve length
            let t = i as f32 / (polyline.len() - 1).max(1) as f32;
            let gradient_color = Vec4::new(
                base_color.x * (1.0 - t * 0.1),
                base_color.y * (1.0 + t * 0.05),
                base_color.z,
                base_color.w * (0.7 + t * 0.3),
            );

            // Tangent direction for oriented rendering
            let tangent = if i < polyline.len() - 1 {
                (polyline[i + 1] - polyline[i]).normalize_or_zero()
            } else if i > 0 {
                (polyline[i] - polyline[i - 1]).normalize_or_zero()
            } else {
                Vec2::X
            };
            let rotation = tangent.y.atan2(tangent.x);

            // Glyph character based on thickness and style
            let ch = if thickness > 0.05 { '#' }
                else if thickness > 0.03 { '*' }
                else if thickness > 0.015 { '+' }
                else { '.' };

            let glyph = Glyph {
                character: ch,
                position: Vec3::new(pos.x + pt.x, pos.y + pt.y, pos.z),
                scale: Vec2::splat(thickness * 5.0), // scale proportional to thickness
                rotation,
                color: gradient_color,
                emission,
                glow_color: Vec3::new(gradient_color.x, gradient_color.y, gradient_color.z),
                glow_radius: emission * 0.5,
                mass: 0.0,
                layer: RenderLayer::Entity,
                blend_mode: BlendMode::Additive,
                lifetime: dt * 1.5, // one-frame glyph
                ..Default::default()
            };

            spawn_fn(glyph);
            count += 1;
        }
    }

    count
}

/// Render just the control points of a curve entity (for editor/debug).
pub fn render_control_points(
    entity: &CurveEntity,
    spawn_fn: &mut dyn FnMut(Glyph) -> GlyphId,
    dt: f32,
) -> usize {
    let mut count = 0;
    let pos = entity.position;

    for curve in &entity.curves {
        for (i, pt) in curve.control_points.iter().enumerate() {
            let glyph = Glyph {
                character: 'o',
                position: Vec3::new(pos.x + pt.x, pos.y + pt.y, pos.z + 0.1),
                scale: Vec2::splat(0.15),
                color: Vec4::new(1.0, 0.8, 0.2, 0.5),
                emission: 0.5,
                mass: 0.0,
                layer: RenderLayer::Overlay,
                lifetime: dt * 1.5,
                ..Default::default()
            };
            spawn_fn(glyph);
            count += 1;
        }
    }
    count
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// GLSL shaders for GPU line rendering (future use with geometry shader)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Vertex shader for thick line rendering.
pub const LINE_VERT_GLSL: &str = r#"
#version 330 core

layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_next_position;
layout(location = 2) in float a_thickness;
layout(location = 3) in vec4 a_color;
layout(location = 4) in float a_emission;
layout(location = 5) in float a_t; // parameter along curve [0, 1]

uniform mat4 u_view_proj;

out vec4 v_color;
out float v_emission;
out float v_t;
out vec2 v_position;
out vec2 v_next;
out float v_thickness;

void main() {
    v_color = a_color;
    v_emission = a_emission;
    v_t = a_t;
    v_position = a_position;
    v_next = a_next_position;
    v_thickness = a_thickness;
    gl_Position = u_view_proj * vec4(a_position, 0.0, 1.0);
}
"#;

/// Geometry shader: expands line segments into screen-aligned quads.
pub const LINE_GEOM_GLSL: &str = r#"
#version 330 core

layout(lines) in;
layout(triangle_strip, max_vertices = 4) out;

in vec4 v_color[];
in float v_emission[];
in float v_t[];
in vec2 v_position[];
in vec2 v_next[];
in float v_thickness[];

uniform mat4 u_view_proj;
uniform vec2 u_resolution;

out vec4 f_color;
out float f_emission;
out float f_t;
out float f_dist_from_center; // -1 to 1 across the line width

void main() {
    vec2 p0 = v_position[0];
    vec2 p1 = v_position[1];
    vec2 dir = normalize(p1 - p0);
    vec2 normal = vec2(-dir.y, dir.x);

    float half_w0 = v_thickness[0] * 0.5;
    float half_w1 = v_thickness[1] * 0.5;

    // Emit 4 vertices forming a quad
    f_color = v_color[0]; f_emission = v_emission[0]; f_t = v_t[0]; f_dist_from_center = -1.0;
    gl_Position = u_view_proj * vec4(p0 + normal * half_w0, 0.0, 1.0); EmitVertex();

    f_color = v_color[0]; f_emission = v_emission[0]; f_t = v_t[0]; f_dist_from_center = 1.0;
    gl_Position = u_view_proj * vec4(p0 - normal * half_w0, 0.0, 1.0); EmitVertex();

    f_color = v_color[1]; f_emission = v_emission[1]; f_t = v_t[1]; f_dist_from_center = -1.0;
    gl_Position = u_view_proj * vec4(p1 + normal * half_w1, 0.0, 1.0); EmitVertex();

    f_color = v_color[1]; f_emission = v_emission[1]; f_t = v_t[1]; f_dist_from_center = 1.0;
    gl_Position = u_view_proj * vec4(p1 - normal * half_w1, 0.0, 1.0); EmitVertex();

    EndPrimitive();
}
"#;

/// Fragment shader: soft glowing line with distance-based falloff.
pub const LINE_FRAG_GLSL: &str = r#"
#version 330 core

in vec4 f_color;
in float f_emission;
in float f_t;
in float f_dist_from_center;

layout(location = 0) out vec4 o_color;
layout(location = 1) out vec4 o_emission_out;

void main() {
    // Gaussian falloff from centerline (soft glowing edges)
    float dist = abs(f_dist_from_center);
    float alpha = exp(-dist * dist * 3.0); // gaussian: sharp center, soft edges
    if (alpha < 0.01) discard;

    vec3 col = f_color.rgb;
    float a = alpha * f_color.a;

    o_color = vec4(col, a);

    // Emission for bloom
    float bloom = clamp(f_emission - 0.3, 0.0, 1.0);
    o_emission_out = vec4(col * bloom, a);
}
"#;
