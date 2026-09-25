// persist.frag — motion trails by remembering the last frame.
//
// The new frame is laid over what the previous one left behind, decayed.
// Taking the brighter of the two rather than blending them keeps a moving
// thing sharp and leaves a fading comet of where it was, which is what an
// eye does with something bright moving fast in the dark. Static things
// are the same in both frames and are unaffected.
#version 330 core

in vec2 f_uv;
uniform sampler2D u_scene;
uniform sampler2D u_history;
uniform float u_persistence;

out vec4 o_color;

void main() {
    vec4 now  = texture(u_scene, f_uv);
    vec4 then = texture(u_history, f_uv) * u_persistence;
    o_color = max(now, then);
}
