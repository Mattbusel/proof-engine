// upsample.frag — combine one level of the bloom pyramid with the next up.
//
// The tent filter from the standard dual-filtering blur. A plain bilinear
// upsample of a sixteenth-size image leaves visible square structure once it
// is added back at full size; nine taps in a 3x3 tent removes it for the cost
// of eight extra samples on a very small texture.
#version 330 core

in vec2 f_uv;
uniform sampler2D u_lower;   // the smaller, blurrier level
uniform sampler2D u_higher;  // the level being written back into
uniform float u_radius;
uniform float u_strength;

out vec4 o_color;

void main() {
    vec2 t = (1.0 / vec2(textureSize(u_lower, 0))) * u_radius;

    vec3 c = texture(u_lower, f_uv + vec2(-t.x,  t.y)).rgb
           + texture(u_lower, f_uv + vec2( 0.0,  t.y)).rgb * 2.0
           + texture(u_lower, f_uv + vec2( t.x,  t.y)).rgb
           + texture(u_lower, f_uv + vec2(-t.x,  0.0)).rgb * 2.0
           + texture(u_lower, f_uv).rgb * 4.0
           + texture(u_lower, f_uv + vec2( t.x,  0.0)).rgb * 2.0
           + texture(u_lower, f_uv + vec2(-t.x, -t.y)).rgb
           + texture(u_lower, f_uv + vec2( 0.0, -t.y)).rgb * 2.0
           + texture(u_lower, f_uv + vec2( t.x, -t.y)).rgb;
    c /= 16.0;

    // Added rather than mixed: each level of the pyramid is a different scale
    // of the same light, and light adds.
    o_color = vec4(texture(u_higher, f_uv).rgb + c * u_strength, 1.0);
}
