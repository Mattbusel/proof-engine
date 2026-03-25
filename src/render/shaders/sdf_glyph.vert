// sdf_glyph.vert — instanced SDF glyph vertex shader
//
// Same instance layout as glyph.vert but outputs additional data
// for SDF-specific fragment processing (threshold, smoothing, effects).

#version 330 core

// Per-vertex (quad)
layout(location = 0) in vec2 v_pos;       // [-0.5, 0.5] unit quad
layout(location = 1) in vec2 v_uv;        // [0, 1] UV

// Per-instance (base — same layout as standard glyph)
layout(location = 2)  in vec3  i_position;
layout(location = 3)  in vec2  i_scale;
layout(location = 4)  in float i_rotation;
layout(location = 5)  in vec4  i_color;
layout(location = 6)  in float i_emission;
layout(location = 7)  in vec3  i_glow_color;
layout(location = 8)  in float i_glow_radius;
layout(location = 9)  in vec2  i_uv_offset;
layout(location = 10) in vec2  i_uv_size;

uniform mat4  u_view_proj;
uniform float u_time;
uniform vec2  u_screen_size;

out vec2  f_uv;
out vec4  f_color;
out float f_emission;
out vec3  f_glow_color;
out float f_glow_radius;
out vec2  f_screen_pos;    // screen-space position for effects
out float f_scale_factor;  // screen-space scale for smoothing calc

void main() {
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    vec2 rotated = vec2(
        v_pos.x * c - v_pos.y * s,
        v_pos.x * s + v_pos.y * c
    ) * i_scale;

    vec4 world_pos = vec4(i_position + vec3(rotated, 0.0), 1.0);
    gl_Position = u_view_proj * world_pos;
    gl_Position.y = -gl_Position.y;  // FBO Y inversion

    f_uv          = i_uv_offset + v_uv * i_uv_size;
    f_color       = i_color;
    f_emission    = i_emission;
    f_glow_color  = i_glow_color;
    f_glow_radius = i_glow_radius;

    // Compute screen-space position and scale for adaptive smoothing.
    f_screen_pos   = (gl_Position.xy / gl_Position.w) * 0.5 + 0.5;
    f_screen_pos  *= u_screen_size;
    f_scale_factor = length(i_scale) * length(vec2(u_view_proj[0][0], u_view_proj[1][1]));
}
