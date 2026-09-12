// light.frag — the screen-space light map, with shadows.
//
// Every pixel of the scene is lit by an ambient level plus each light in
// the frame, and each light is shadowed by the matter between it and the
// pixel. The matter is the occluder buffer the glyph pass wrote: coverage
// for particles, nothing for floors and panel fills. Twenty-four samples
// are taken along the line from the pixel to the light, and each one that
// lands on matter takes a share of the light away.
//
// So a figure standing beside a brazier is lit on the side that faces it
// and dark on the side that does not, and casts its shape across the floor
// behind it, because the pixels back there have to look through the figure
// to see the fire. Nobody wrote a shadow; the geometry did.
#version 330 core

in vec2 f_uv;
uniform sampler2D u_occluder;
uniform vec2  u_screen;
uniform vec3  u_ambient;
uniform int   u_count;
const int MAX_LIGHTS = 32;
// (u, v, radius_px, casts_shadow)
uniform vec4  u_light_pos[MAX_LIGHTS];
// (r, g, b, intensity)
uniform vec4  u_light_color[MAX_LIGHTS];
// How much light one fully covered sample removes.
uniform float u_shadow_density;

out vec4 o_light;

const int STEPS = 24;

void main() {
    vec3 light = u_ambient;
    for (int i = 0; i < MAX_LIGHTS; ++i) {
        if (i >= u_count) break;
        vec4 lp = u_light_pos[i];
        vec4 lc = u_light_color[i];
        vec2 d_px = (lp.xy - f_uv) * u_screen;
        float dist = length(d_px);
        float radius = max(lp.z, 1.0);
        if (dist >= radius) continue;
        // Inverse-square-ish with a smooth end at the radius.
        float x = dist / radius;
        float atten = (1.0 - x) * (1.0 - x) / (0.15 + x * x * 2.0);

        float transmit = 1.0;
        if (lp.w > 0.5 && u_shadow_density > 0.0) {
            // Start a little way off the pixel itself, so a surface is not
            // shadowed by its own coverage; stop a little short of the
            // light, which sits inside its own emitter.
            vec2 step_uv = (lp.xy - f_uv) / float(STEPS);
            vec2 p = f_uv + step_uv * 1.5;
            float density = u_shadow_density / float(STEPS);
            for (int s = 1; s < STEPS - 1; ++s) {
                float occ = texture(u_occluder, p).r;
                transmit *= 1.0 - clamp(occ * density, 0.0, 1.0);
                p += step_uv;
                if (transmit < 0.02) break;
            }
        }
        light += lc.rgb * lc.a * atten * transmit;
    }
    o_light = vec4(light, 1.0);
}
