// composite.frag — scene + light + grade, in the order a film pipeline uses.
//
// The scene arrives in linear HDR: a sixteen-bit float buffer that a few
// hundred thousand overlapping emissive particles can push well past white
// without clipping. Everything here works on those real values, and the
// tonemap is the one place the range is brought down.
//
// Order: shockwave refraction, lens shape, chromatic sample, sharpen,
// exposure, indirect light, bloom, halation, light shafts, lens flare,
// flash, tonemap, grade, vignette, grain, dither, scanlines.
#version 330 core

in vec2 f_uv;
uniform sampler2D u_scene;
uniform sampler2D u_bloom;
uniform float u_bloom_intensity;
uniform float u_exposure;
uniform vec3  u_tint;
uniform float u_saturation;
uniform float u_contrast;
uniform float u_brightness;
uniform float u_vignette;
uniform float u_vignette_softness;
uniform float u_grain_intensity;
uniform float u_grain_seed;
uniform float u_chromatic;
uniform float u_scanline_intensity;
uniform bool  u_scanlines_enabled;
uniform float u_tonemap;
uniform vec3  u_lift;
uniform vec3  u_gain;
uniform float u_halation;
uniform float u_sharpen;
uniform float u_dither;
uniform float u_barrel;

// ── New: light and moments ──────────────────────────────────────────────────
// The finished frame's size in pixels. Shockwave geometry is in pixels so a
// blow looks the same at any resolution.
uniform vec2  u_screen;
// Screen-space indirect light: matter near something bright is lit by it.
uniform float u_indirect;
// Radial light shafts streaming from a point.
uniform float u_light_shafts;
uniform vec3  u_shaft;        // (u, v, strength)
uniform vec3  u_shaft_tint;
// Lens flare ghosts and halo off anything that blooms.
uniform float u_lens_flare;
// A colour added to the whole frame, already scaled. Decays on the CPU.
uniform vec3  u_flash;
// A glossy floor: (line_v, strength, fade_v, ripple_px), and its blur in px.
uniform vec4  u_reflection;
uniform float u_reflection_blur;
// Heat shimmer, 0 to about 1, and a clock for it.
uniform float u_haze;
uniform float u_time;
// The light map, and the emission buffer that says what lights itself.
uniform sampler2D u_light;
uniform sampler2D u_emission;
uniform bool  u_lighting;
// Expanding rings of refraction: (u, v, radius_px, width_px) and strength_px.
const int MAX_SHOCK = 12;
uniform vec4  u_shock[MAX_SHOCK];
uniform float u_shock_strength[MAX_SHOCK];
uniform int   u_shock_count;

out vec4 o_color;

float rand(vec2 co) {
    return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
}

// ACES filmic, the fitted curve. Rolls highlights off instead of clipping
// them, which is the single thing that separates a rendered image from a
// photographed one.
vec3 aces(vec3 x) {
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0);
}

void main() {
    vec2 uv0 = f_uv;

    // ── Shockwaves ─────────────────────────────────────────────────────────
    //
    // Each one is a Gaussian ring in pixel space. Inside the ring the image
    // is pulled toward the origin, outside it is pushed away, which is what
    // a pressure front does to the air in front of a lens. The ring also
    // brightens a little, because compressed air refracts more light into
    // the eye than the calm air either side of it.
    float shock_light = 0.0;
    for (int i = 0; i < MAX_SHOCK; ++i) {
        if (i >= u_shock_count) break;
        vec4 s = u_shock[i];
        vec2 d_px = (uv0 - s.xy) * u_screen;
        float dist = length(d_px);
        float ring = exp(-pow((dist - s.z) / s.w, 2.0));
        // Signed: the leading edge pushes out, the trailing edge pulls in.
        float side = clamp((dist - s.z) / s.w, -1.0, 1.0);
        vec2 dir = dist > 0.5 ? d_px / dist : vec2(0.0);
        uv0 -= dir * ring * side * u_shock_strength[i] / u_screen;
        shock_light += ring * u_shock_strength[i] * 0.012;
    }

    // ── Heat haze ──────────────────────────────────────────────────────────
    //
    // Two sine fields at different scales, rising, strongest at the bottom
    // of the frame where the hot air is. A fraction of a pixel is enough:
    // real shimmer is seen as things wavering, not as things moving.
    if (u_haze > 0.0) {
        float rise = u_time * 0.9;
        float band = 1.0 - smoothstep(0.0, 0.9, uv0.y);
        vec2 wobble = vec2(
            sin(uv0.y * 61.0 + rise * 7.0 + sin(uv0.x * 23.0 + rise * 3.0)),
            sin(uv0.x * 47.0 - rise * 5.0 + cos(uv0.y * 31.0 + rise * 2.0)) * 0.5
        );
        uv0 += wobble * (0.9 / u_screen) * u_haze * 2.2 * band;
    }

    // ── Lens shape ─────────────────────────────────────────────────────────
    // A very slight barrel, applied before anything is sampled so
    // everything downstream inherits it.
    vec2 centred = uv0 - 0.5;
    if (u_barrel != 0.0) {
        centred *= 1.0 + u_barrel * dot(centred, centred);
    }
    vec2 uv = centred + 0.5;

    // Chromatic aberration, scaled by distance from the centre rather than
    // applied flat, because a real lens is sharp in the middle.
    float radial = dot(centred, centred);
    vec2 offset = centred * u_chromatic * (0.35 + radial * 2.4);
    float r = texture(u_scene, uv + offset).r;
    float g = texture(u_scene, uv).g;
    float b = texture(u_scene, uv - offset).b;
    vec3 color = vec3(r, g, b);

    // An unsharp mask on the scene before anything is added to it. Bloom and
    // a tonemap both soften an image, and this picture is built out of
    // overlapping particles whose detail is all in the edges.
    if (u_sharpen > 0.0) {
        vec2 t = 1.0 / vec2(textureSize(u_scene, 0));
        vec3 blur = texture(u_scene, uv + vec2( t.x, 0.0)).rgb
                  + texture(u_scene, uv + vec2(-t.x, 0.0)).rgb
                  + texture(u_scene, uv + vec2(0.0,  t.y)).rgb
                  + texture(u_scene, uv + vec2(0.0, -t.y)).rgb;
        color += (color - blur * 0.25) * u_sharpen;
    }

    // ── Floor reflection ───────────────────────────────────────────────────
    //
    // Below the line, the picture above it, mirrored, blurred a little and
    // laid over the floor, fading out with distance from the line. What is
    // standing on the floor is what shows in it, because the whole scene is
    // in the buffer by now. Sampled after the lens so the mirror inherits
    // the same glass.
    if (u_reflection.y > 0.0 && uv.y < u_reflection.x) {
        float below = (u_reflection.x - uv.y) / max(u_reflection.z, 1e-4);
        float weight = u_reflection.y * (1.0 - smoothstep(0.0, 1.0, below));
        if (weight > 0.001) {
            vec2 t = 1.0 / vec2(textureSize(u_scene, 0));
            float ripple = sin(uv.y * 380.0 + u_time * 2.4) * u_reflection.w * t.x;
            vec2 m = vec2(uv.x + ripple, 2.0 * u_reflection.x - uv.y);
            float b = u_reflection_blur * (1.0 + below * 2.0);
            vec3 refl = texture(u_scene, m).rgb * 2.0
                      + texture(u_scene, m + vec2( t.x * b, 0.0)).rgb
                      + texture(u_scene, m + vec2(-t.x * b, 0.0)).rgb
                      + texture(u_scene, m + vec2(0.0,  t.y * b)).rgb
                      + texture(u_scene, m + vec2(0.0, -t.y * b)).rgb;
            refl /= 6.0;
            // Screen rather than add: a mirror never makes a floor darker,
            // and adding on top of a lit floor blows it out.
            color = color + refl * weight * (1.0 - clamp(color, 0.0, 1.0) * 0.5);
        }
    }

    // ── The light map ──────────────────────────────────────────────────────
    //
    // Ambient plus every light, shadowed. Emissive matter lights itself:
    // a brazier is not dimmed by the dark it is standing in.
    if (u_lighting) {
        vec3 light = texture(u_light, uv).rgb;
        vec3 em = texture(u_emission, uv).rgb;
        float self_lit = clamp(max(em.r, max(em.g, em.b)) * 1.6, 0.0, 1.0);
        color *= max(light, vec3(self_lit));
    }

    // ── Exposure, then light ───────────────────────────────────────────────
    color *= u_exposure;
    vec3 bloom = texture(u_bloom, uv).rgb;

    // Indirect light. The bloom pyramid is, among other things, a blurred
    // map of where the light in the frame is. Multiplying the scene by it
    // lights the matter standing near a lit thing in that thing's colour,
    // by that matter's own albedo, which is what bounce light does and what
    // adding bloom on top never does: bloom brightens the air, this
    // brightens the surfaces.
    color += color * bloom * u_indirect;

    color += bloom * u_bloom_intensity;

    // Halation: the warm bleed real film gets around a bright edge.
    color += vec3(bloom.r, bloom.r * 0.42, bloom.r * 0.22) * u_halation;

    // ── Light shafts ───────────────────────────────────────────────────────
    //
    // A radial blur of the light toward its source. Twenty-four samples
    // stepping from this pixel toward the origin, each weighted a little
    // less than the last, so the light streams out from the source and
    // fades along its length. Sampled from the bloom, which is half the
    // resolution and already thresholded, so only the lit things cast.
    if (u_light_shafts > 0.0 && u_shaft.z > 0.0) {
        const int STEPS = 24;
        vec2 to_src = (u_shaft.xy - uv) / float(STEPS) * 0.85;
        float decay = 0.94;
        float weight = 1.0;
        vec3 shaft = vec3(0.0);
        vec2 p = uv;
        for (int i = 0; i < STEPS; ++i) {
            p += to_src;
            shaft += texture(u_bloom, p).rgb * weight;
            weight *= decay;
        }
        shaft /= float(STEPS) * 0.55;
        // Fade with distance from the source, so the far side of the frame
        // is not lit by a brazier on the near side.
        float reach = 1.0 - smoothstep(0.0, 1.1, length((uv - u_shaft.xy) * vec2(1.0, u_screen.y / u_screen.x)));
        color += shaft * u_shaft_tint * u_light_shafts * u_shaft.z * reach;
    }

    // ── Lens flare ─────────────────────────────────────────────────────────
    //
    // Ghosts are the bloom mirrored through the centre of the lens, at a few
    // spacings, each with a slight spectral tint; the halo is a ring at a
    // fixed radius from the centre. Both are weighted toward the centre so
    // a light at the edge of frame puts its ghosts across the middle, the
    // way real glass does, and nothing flares out of a dark frame because
    // the bloom is already thresholded.
    if (u_lens_flare > 0.0) {
        vec2 ghost_vec = (0.5 - uv) * 0.44;
        vec3 flare = vec3(0.0);
        for (int i = 1; i <= 3; ++i) {
            vec2 g = uv + ghost_vec * float(i);
            float w = 1.0 - smoothstep(0.0, 0.75, length(g - 0.5));
            w = w * w;
            vec3 tint = (i == 1) ? vec3(1.0, 0.85, 0.7)
                      : (i == 2) ? vec3(0.7, 0.9, 1.0)
                                 : vec3(0.9, 0.7, 1.0);
            flare += texture(u_bloom, g).rgb * w * tint * (0.5 / float(i));
        }
        vec2 halo_dir = normalize(ghost_vec + vec2(1e-5));
        vec2 halo_uv = uv + halo_dir * 0.38;
        float halo_w = 1.0 - smoothstep(0.0, 0.6, length(halo_uv - 0.5));
        flare += texture(u_bloom, halo_uv).rgb * halo_w * halo_w * vec3(0.8, 0.9, 1.0) * 0.25;
        color += flare * u_lens_flare;
    }

    // Shockwave brightening and any flash, before the roll-off so a strong
    // one goes to white through the highlights rather than by clipping.
    color += vec3(shock_light);
    color += u_flash;

    // Tonemap, blended rather than switched so a caller can dial it back
    // without a hard change of look at some threshold.
    color = mix(clamp(color, 0.0, 1.0), aces(color), u_tonemap);

    // Grade. Lift and gain move shadows and highlights independently, which is
    // what a colour grade actually is.
    color = color * u_gain + u_lift * (1.0 - color);
    color *= u_tint;
    color += u_brightness;
    color = (color - 0.5) * u_contrast + 0.5;

    float lum = dot(color, vec3(0.2126, 0.7152, 0.0722));
    color = mix(vec3(lum), color, u_saturation);

    // Vignette, smoothstepped from an adjustable edge rather than a raw
    // quadratic, so it frames the picture rather than dimming the middle.
    float d = length(centred) * 1.41421356;
    color *= 1.0 - u_vignette * smoothstep(u_vignette_softness, 1.0, d);

    // Grain, weighted toward the shadows where film grain actually lives.
    float grain = rand(uv + vec2(u_grain_seed)) * 2.0 - 1.0;
    float shadow_weight = 1.0 - smoothstep(0.0, 0.7, lum);
    color += grain * u_grain_intensity * (0.35 + shadow_weight);

    // A 4x4 ordered dither, under one quantisation step, against banding in
    // the dark gradients this game is made of.
    if (u_dither > 0.0) {
        const float bayer[16] = float[](
             0.0,  8.0,  2.0, 10.0,
            12.0,  4.0, 14.0,  6.0,
             3.0, 11.0,  1.0,  9.0,
            15.0,  7.0, 13.0,  5.0
        );
        ivec2 px = ivec2(gl_FragCoord.xy) % 4;
        color += ((bayer[px.y * 4 + px.x] + 0.5) / 16.0 - 0.5) * u_dither / 255.0;
    }

    if (u_scanlines_enabled) {
        float scanline = sin(uv.y * u_screen.y * 3.14159) * 0.5 + 0.5;
        color *= 1.0 - u_scanline_intensity * (1.0 - scanline);
    }

    o_color = vec4(clamp(color, 0.0, 1.0), 1.0);
}
