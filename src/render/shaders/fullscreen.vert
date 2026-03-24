// fullscreen.vert — single-triangle fullscreen pass
//
// Covers the entire clip-space viewport using 3 vertices, no VBO needed.
// Call with glDrawArrays(GL_TRIANGLES, 0, 3) with any VAO bound.
//
//   gl_VertexID  uv        clip-space pos
//   0            (0, 0)    (-1, -1)
//   1            (2, 0)    ( 3, -1)
//   2            (0, 2)    (-1,  3)
//
// The giant triangle is clipped to the viewport, so f_uv stays in [0,1]
// across all visible fragments.

#version 330 core

out vec2 f_uv;

void main() {
    float u = float(gl_VertexID & 1) * 2.0;
    float v = float(gl_VertexID >> 1) * 2.0;
    f_uv        = vec2(u, v);
    gl_Position = vec4(u * 2.0 - 1.0, v * 2.0 - 1.0, 0.0, 1.0);
}
