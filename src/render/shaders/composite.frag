// composite.frag — combine scene + bloom + postfx
#version 330 core

in vec2 f_uv;
uniform sampler2D u_scene;
uniform sampler2D u_bloom;
uniform float u_bloom_intensity;
uniform vec3  u_tint;
uniform float u_saturation;
uniform float u_contrast;
uniform float u_brightness;
uniform float u_vignette;
uniform float u_grain_intensity;
uniform float u_grain_seed;
uniform float u_chromatic;
uniform float u_scanline_intensity;
uniform bool  u_scanlines_enabled;

out vec4 o_color;

float rand(vec2 co) {
    return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
}

void main() {
    // Chromatic aberration
    float ca = u_chromatic;
    vec2 offset = (f_uv - 0.5) * ca;
    float r = texture(u_scene, f_uv + offset).r;
    float g = texture(u_scene, f_uv).g;
    float b = texture(u_scene, f_uv - offset).b;
    vec3 color = vec3(r, g, b);

    // Add bloom
    vec3 bloom = texture(u_bloom, f_uv).rgb;
    color += bloom * u_bloom_intensity;

    // Tint
    color *= u_tint;

    // Brightness
    color += u_brightness;

    // Contrast
    color = (color - 0.5) * u_contrast + 0.5;

    // Saturation
    float lum = dot(color, vec3(0.299, 0.587, 0.114));
    color = mix(vec3(lum), color, u_saturation);

    // Vignette
    vec2 vig_uv = f_uv * 2.0 - 1.0;
    float vig = 1.0 - dot(vig_uv, vig_uv) * u_vignette;
    color *= vig;

    // Film grain
    float grain = rand(f_uv + vec2(u_grain_seed)) * 2.0 - 1.0;
    color += grain * u_grain_intensity;

    // CRT scanlines
    if (u_scanlines_enabled) {
        float scanline = sin(f_uv.y * float(textureSize(u_scene, 0).y) * 3.14159) * 0.5 + 0.5;
        color *= 1.0 - u_scanline_intensity * (1.0 - scanline);
    }

    o_color = vec4(clamp(color, 0.0, 1.0), 1.0);
}
