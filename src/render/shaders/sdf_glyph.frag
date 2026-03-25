// sdf_glyph.frag — SDF glyph fragment shader
//
// Renders signed distance field glyphs with:
//   - Razor-sharp edges at any scale via smoothstep on the distance field
//   - Optional outline effect (second smoothstep at wider threshold)
//   - Optional drop shadow (UV-offset SDF sample)
//   - Optional glow (distance field as glow intensity)
//   - Bold mode (threshold shift)
//   - UV distortion effects (wave, shake, glitch)
//   - Dual output: color + emission for bloom pipeline

#version 330 core

in vec2  f_uv;
in vec4  f_color;
in float f_emission;
in vec3  f_glow_color;
in float f_glow_radius;
in vec2  f_screen_pos;
in float f_scale_factor;

uniform sampler2D u_sdf_atlas;

// SDF parameters
uniform float u_threshold;       // typically 0.5
uniform float u_smoothing;       // typically 0.05, auto-adjusted by scale
uniform float u_time;

// Outline
uniform float u_outline_enabled; // 0.0 or 1.0
uniform float u_outline_width;   // in SDF-space (0.0 to ~0.2)
uniform vec4  u_outline_color;

// Shadow
uniform float u_shadow_enabled;
uniform float u_shadow_softness;
uniform vec2  u_shadow_uv_offset;
uniform vec4  u_shadow_color;

// Glow
uniform float u_glow_enabled;
uniform float u_glow_sdf_radius; // in SDF-space
uniform vec4  u_glow_color_param;

// Distortion
uniform float u_wave_amplitude;
uniform float u_wave_frequency;
uniform float u_shake_amount;
uniform float u_glitch_intensity;

layout(location = 0) out vec4 o_color;
layout(location = 1) out vec4 o_emission;

// Pseudo-random hash for glitch/shake effects.
float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

void main() {
    vec2 uv = f_uv;

    // ── UV Distortion effects ───────────────────────────────────────────────
    if (u_wave_amplitude > 0.0) {
        uv.y += sin(uv.x * u_wave_frequency + u_time * 3.0) * u_wave_amplitude * 0.01;
        uv.x += cos(uv.y * u_wave_frequency * 0.7 + u_time * 2.5) * u_wave_amplitude * 0.005;
    }

    if (u_shake_amount > 0.0) {
        float shake = u_shake_amount * 0.01;
        uv.x += (hash(f_screen_pos + u_time) - 0.5) * shake;
        uv.y += (hash(f_screen_pos + u_time + 1.0) - 0.5) * shake;
    }

    if (u_glitch_intensity > 0.0) {
        float glitch_line = step(0.95, hash(vec2(floor(f_screen_pos.y * 0.1), u_time * 10.0)));
        uv.x += glitch_line * u_glitch_intensity * 0.05 * (hash(vec2(u_time * 100.0, f_screen_pos.y)) - 0.5);
    }

    // ── SDF sampling ────────────────────────────────────────────────────────
    float distance = texture(u_sdf_atlas, uv).r;

    // Adaptive smoothing: sharper at large sizes, softer at small sizes.
    float smoothing = u_smoothing;
    if (f_scale_factor > 0.0) {
        smoothing = clamp(0.25 / f_scale_factor, 0.001, 0.25);
    }

    float threshold = u_threshold;

    // ── Glyph alpha (the core SDF operation) ────────────────────────────────
    float alpha = smoothstep(threshold - smoothing, threshold + smoothing, distance);

    // ── Drop shadow ─────────────────────────────────────────────────────────
    vec4 final_color = vec4(0.0);

    if (u_shadow_enabled > 0.5) {
        float shadow_dist = texture(u_sdf_atlas, uv + u_shadow_uv_offset).r;
        float shadow_smooth = smoothing + u_shadow_softness;
        float shadow_alpha = smoothstep(threshold - shadow_smooth, threshold + shadow_smooth, shadow_dist);
        final_color = u_shadow_color * shadow_alpha;
    }

    // ── Glow (distance-based) ───────────────────────────────────────────────
    if (u_glow_enabled > 0.5) {
        float glow_outer = threshold - u_glow_sdf_radius;
        float glow_alpha = smoothstep(glow_outer - smoothing, threshold, distance);
        vec4 glow_contrib = u_glow_color_param * glow_alpha * (1.0 - alpha);
        final_color = mix(final_color, glow_contrib, glow_contrib.a);
    }

    // ── Outline ─────────────────────────────────────────────────────────────
    if (u_outline_enabled > 0.5) {
        float outline_outer = threshold - u_outline_width;
        float outline_alpha = smoothstep(outline_outer - smoothing, outline_outer + smoothing, distance);
        // Outline is visible where outline_alpha > 0 but alpha == 0 (between outline and glyph).
        float outline_mask = outline_alpha * (1.0 - alpha);
        vec4 outline_contrib = vec4(u_outline_color.rgb, u_outline_color.a * outline_mask);
        final_color = mix(final_color, outline_contrib, outline_mask);
    }

    // ── Main glyph color ────────────────────────────────────────────────────
    vec4 glyph_color = f_color * alpha;
    final_color = mix(final_color, glyph_color, alpha);

    if (final_color.a < 0.01) discard;

    o_color = final_color;

    // ── Emission for bloom ──────────────────────────────────────────────────
    float bloom_strength = clamp(f_emission - 0.3, 0.0, 1.0);
    float glow_boost = clamp(f_glow_radius * 0.15, 0.0, 0.8);
    o_emission = vec4(f_glow_color * (bloom_strength + glow_boost), final_color.a);
}
