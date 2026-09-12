// composite.frag — scene + bloom + grade, in the order a film pipeline uses.
//
// Everything used to happen in low dynamic range on a clamped buffer: bloom
// added, then contrast and saturation applied to an already-clipped image,
// then clamped again. A scene made almost entirely of overlapping emissive
// particles came out looking untouched, because the highlights had nowhere to
// go and there was no roll-off for a grade to work on.
//
// Now: distort, sample, sharpen, expose, add light, tonemap, grade, vignette,
// grain, dither.
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
    // Lens shape. A very slight barrel, applied before anything is sampled so
    // everything downstream inherits it. Enough that the frame is not a
    // perfect rectangle; not enough that anyone would call it a fisheye.
    vec2 centred = f_uv - 0.5;
    if (u_barrel != 0.0) {
        centred *= 1.0 + u_barrel * dot(centred, centred);
    }
    vec2 uv = centred + 0.5;

    // Chromatic aberration, scaled by distance from the centre rather than
    // applied flat, because a real lens is sharp in the middle. A uniform
    // split reads as a broken screen; this reads as glass.
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

    // Exposure, then light.
    color *= u_exposure;
    vec3 bloom = texture(u_bloom, uv).rgb;
    color += bloom * u_bloom_intensity;

    // Halation: the warm bleed real film gets around a bright edge. The red
    // channel of the bloom, added back wide and warm. It costs one multiply
    // and it is most of why a lit thing looks lit rather than bright.
    color += vec3(bloom.r, bloom.r * 0.42, bloom.r * 0.22) * u_halation;

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
    // quadratic. The old one darkened from the centre outward, so it dimmed
    // the middle of the picture as well as the corners.
    float d = length(centred) * 1.41421356;
    color *= 1.0 - u_vignette * smoothstep(u_vignette_softness, 1.0, d);

    // Grain, weighted toward the shadows where film grain actually lives.
    // Flat grain over a whole frame reads as video noise.
    float grain = rand(uv + vec2(u_grain_seed)) * 2.0 - 1.0;
    float shadow_weight = 1.0 - smoothstep(0.0, 0.7, lum);
    color += grain * u_grain_intensity * (0.35 + shadow_weight);

    // A 4x4 ordered dither, under one quantisation step. This game is very
    // dark, and a dark gradient in eight bits per channel bands visibly: every
    // soft shadow and every vignette edge had rings in it. A fraction of a
    // step of structured noise breaks them up and is invisible on its own.
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
        float scanline = sin(uv.y * float(textureSize(u_scene, 0).y) * 3.14159) * 0.5 + 0.5;
        color *= 1.0 - u_scanline_intensity * (1.0 - scanline);
    }

    o_color = vec4(clamp(color, 0.0, 1.0), 1.0);
}
