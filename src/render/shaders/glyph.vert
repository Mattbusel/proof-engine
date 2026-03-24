// glyph.vert — instanced glyph vertex shader
// Phase 1 stub. Full shader in Phase 1 implementation.

#version 330 core

// Per-vertex (quad)
layout(location = 0) in vec2 v_pos;       // [-0.5, 0.5] unit quad
layout(location = 1) in vec2 v_uv;        // [0, 1] UV

// Per-instance
layout(location = 2) in vec3  i_position;
layout(location = 3) in vec2  i_scale;
layout(location = 4) in float i_rotation;
layout(location = 5) in vec4  i_color;
layout(location = 6) in float i_emission;
layout(location = 7) in vec3  i_glow_color;
layout(location = 8) in float i_glow_radius;
layout(location = 9) in vec2  i_uv_offset;
layout(location = 10) in vec2 i_uv_size;

uniform mat4 u_view_proj;

out vec2 f_uv;
out vec4 f_color;
out float f_emission;
out vec3 f_glow_color;

void main() {
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    vec2 rotated = vec2(
        v_pos.x * c - v_pos.y * s,
        v_pos.x * s + v_pos.y * c
    ) * i_scale;

    vec4 world_pos = vec4(i_position + vec3(rotated, 0.0), 1.0);
    gl_Position = u_view_proj * world_pos;

    f_uv = i_uv_offset + v_uv * i_uv_size;
    f_color = i_color;
    f_emission = i_emission;
    f_glow_color = i_glow_color;
}
