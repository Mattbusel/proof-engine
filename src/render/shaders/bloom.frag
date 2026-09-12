// bloom.frag — separable Gaussian bloom with a soft-knee threshold and an
// anamorphic option.
//
// Three things were missing.
//
// There was no threshold: the whole emission buffer was blurred and added
// back, so dim matter glowed exactly as readily as bright matter and the
// result was a general haze rather than light coming off the things that are
// actually lit. A soft knee is what makes bloom look like an optical effect
// instead of a blur layer.
//
// The kernel was thirteen taps at a fixed radius with no use of bilinear
// filtering, which is the standard trick: sampling *between* two texels gets
// two taps for one fetch, so the same nine fetches cover a much wider kernel.
//
// And it was isotropic. Real anamorphic glass streaks light horizontally, and
// a horizontal pass with a stretched radius costs exactly what the ordinary
// horizontal pass costs.
#version 330 core

in vec2 f_uv;
uniform sampler2D u_texture;
uniform bool  u_horizontal;
uniform float u_radius;
uniform float u_threshold;
uniform float u_knee;
uniform bool  u_prefilter;
uniform float u_stretch;

out vec4 o_color;

// Linear-sampled 9-tap: offsets fall between texels so the hardware's
// bilinear filter does half the work.
const float weight[3] = float[](0.2270270270, 0.3162162162, 0.0702702703);
const float offset[3] = float[](0.0, 1.3846153846, 3.2307692308);

// Soft knee: nothing under the threshold blooms, everything well over it
// blooms fully, and the region between is a quadratic rather than a cliff.
// A hard cut-off makes bright edges crawl as the frame changes.
vec3 prefilter(vec3 c) {
    float lum = max(c.r, max(c.g, c.b));
    float knee = max(u_knee, 1e-4);
    float soft = clamp(lum - u_threshold + knee, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee);
    float contribution = max(soft, lum - u_threshold) / max(lum, 1e-4);
    return c * contribution;
}

void main() {
    vec2 texel = 1.0 / vec2(textureSize(u_texture, 0));
    vec3 result = texture(u_texture, f_uv).rgb * weight[0];

    if (u_horizontal) {
        float r = u_radius * u_stretch;
        for (int i = 1; i < 3; ++i) {
            vec2 o = vec2(texel.x * offset[i] * r, 0.0);
            result += texture(u_texture, f_uv + o).rgb * weight[i];
            result += texture(u_texture, f_uv - o).rgb * weight[i];
        }
    } else {
        for (int i = 1; i < 3; ++i) {
            vec2 o = vec2(0.0, texel.y * offset[i] * u_radius);
            result += texture(u_texture, f_uv + o).rgb * weight[i];
            result += texture(u_texture, f_uv - o).rgb * weight[i];
        }
    }

    // Only the first pass thresholds. Doing it on every pass would eat the
    // blur it just produced.
    if (u_prefilter) {
        result = prefilter(result);
    }
    o_color = vec4(result, 1.0);
}
