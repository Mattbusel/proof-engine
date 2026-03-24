// glyph.frag — glyph fragment shader
// Phase 1 stub. Full shader in Phase 1 implementation.

#version 330 core

in vec2 f_uv;
in vec4 f_color;
in float f_emission;
in vec3 f_glow_color;

uniform sampler2D u_atlas;

layout(location = 0) out vec4 o_color;
layout(location = 1) out vec4 o_emission;  // for bloom pass

void main() {
    float alpha = texture(u_atlas, f_uv).r;  // atlas is R8 (greyscale glyph mask)
    if (alpha < 0.05) discard;

    o_color   = vec4(f_color.rgb * f_color.a, alpha * f_color.a);
    o_emission = vec4(f_glow_color * f_emission * alpha, 1.0);
}
