//! Render pipeline — glutin 0.32/winit 0.30 window, OpenGL context,
//! instanced glyph batch rendering, and multi-pass post-processing
//! (bright-pass bloom + chromatic aberration).

use std::num::NonZeroU32;
use std::ffi::CString;
use std::time::Duration;

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext,
                      PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use glow::HasContext;
use raw_window_handle::HasWindowHandle;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::Window;
use glam::{Mat4, Vec3};
use bytemuck::cast_slice;

use crate::config::EngineConfig;
use crate::scene::Scene;
use crate::render::camera::ProofCamera;
use crate::input::{InputState, Key};
use crate::glyph::atlas::FontAtlas;
use crate::glyph::batch::{GlyphBatch, GlyphInstance};

// ── Glyph shaders ──────────────────────────────────────────────────────────────

const VERT_SRC: &str = r#"
#version 330 core

layout(location = 0) in vec2  v_pos;
layout(location = 1) in vec2  v_uv;

layout(location = 2)  in vec3  i_position;
layout(location = 3)  in vec2  i_scale;
layout(location = 4)  in float i_rotation;
layout(location = 5)  in vec4  i_color;
layout(location = 6)  in float i_emission;
layout(location = 7)  in vec3  i_glow_color;
layout(location = 8)  in float i_glow_radius;
layout(location = 9)  in vec2  i_uv_offset;
layout(location = 10) in vec2  i_uv_size;

uniform mat4 u_view_proj;

out vec2  f_uv;
out vec4  f_color;
out float f_emission;
out vec3  f_glow_color;

void main() {
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    vec2 rotated = vec2(
        v_pos.x * c - v_pos.y * s,
        v_pos.x * s + v_pos.y * c
    ) * i_scale;

    gl_Position = u_view_proj * vec4(i_position + vec3(rotated, 0.0), 1.0);

    f_uv         = i_uv_offset + v_uv * i_uv_size;
    f_color      = i_color;
    f_emission   = i_emission;
    f_glow_color = i_glow_color;
}
"#;

const FRAG_SRC: &str = r#"
#version 330 core

in vec2  f_uv;
in vec4  f_color;
in float f_emission;
in vec3  f_glow_color;

uniform sampler2D u_atlas;

out vec4 o_color;

void main() {
    float alpha = texture(u_atlas, f_uv).r;
    if (alpha < 0.05) discard;
    float em  = clamp(f_emission * 0.5, 0.0, 1.0);
    vec3  col = mix(f_color.rgb, f_glow_color, em);
    o_color = vec4(col, alpha * f_color.a);
}
"#;

// Unit quad: 6 vertices (2 CCW triangles), each: [pos_x, pos_y, uv_x, uv_y]
#[rustfmt::skip]
const QUAD_VERTS: [f32; 24] = [
    -0.5,  0.5,  0.0, 1.0,
    -0.5, -0.5,  0.0, 0.0,
     0.5,  0.5,  1.0, 1.0,
    -0.5, -0.5,  0.0, 0.0,
     0.5, -0.5,  1.0, 0.0,
     0.5,  0.5,  1.0, 1.0,
];

// ── Post-processing shaders ────────────────────────────────────────────────────

/// Full-screen triangle vertex shader (no VBO needed — uses gl_VertexID).
const POSTFX_VERT: &str = r#"
#version 330 core
out vec2 f_uv;
void main() {
    // 3-vertex full-screen triangle trick
    float x = float((gl_VertexID & 1) << 2) - 1.0;
    float y = float((gl_VertexID & 2) << 1) - 1.0;
    f_uv = vec2(x * 0.5 + 0.5, y * 0.5 + 0.5);
    gl_Position = vec4(x, y, 0.0, 1.0);
}
"#;

/// Extract pixels above a luminance threshold (bright-pass).
const BRIGHT_EXTRACT_FRAG: &str = r#"
#version 330 core
uniform sampler2D u_scene;
uniform float u_threshold;
in vec2 f_uv;
out vec4 o_color;
void main() {
    vec4 c = texture(u_scene, f_uv);
    float lum = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    float t = max(lum - u_threshold, 0.0) / max(1.0 - u_threshold, 0.001);
    o_color = c * t;
}
"#;

/// Single-axis 9-tap Gaussian blur.
const BLUR_FRAG: &str = r#"
#version 330 core
uniform sampler2D u_src;
uniform vec2 u_direction;   // (1,0) horizontal, (0,1) vertical
in vec2 f_uv;
out vec4 o_color;
const float w[5] = float[](0.227027, 0.194595, 0.121622, 0.054054, 0.016216);
void main() {
    vec2 step = u_direction / vec2(textureSize(u_src, 0));
    vec4 result = texture(u_src, f_uv) * w[0];
    for (int i = 1; i < 5; ++i) {
        result += texture(u_src, f_uv + step * float(i)) * w[i];
        result += texture(u_src, f_uv - step * float(i)) * w[i];
    }
    o_color = result;
}
"#;

/// Composite: scene + bloom with optional chromatic aberration.
const COMPOSITE_FRAG: &str = r#"
#version 330 core
uniform sampler2D u_scene;
uniform sampler2D u_bloom;
uniform float u_bloom_intensity;
uniform float u_chromatic;    // 0 = off
in vec2 f_uv;
out vec4 o_color;
void main() {
    vec4 scene;
    if (u_chromatic > 0.001) {
        vec2 dir = (f_uv - 0.5) * u_chromatic;
        float r = texture(u_scene, f_uv + dir).r;
        float g = texture(u_scene, f_uv).g;
        float b = texture(u_scene, f_uv - dir).b;
        scene = vec4(r, g, b, texture(u_scene, f_uv).a);
    } else {
        scene = texture(u_scene, f_uv);
    }
    vec4 bloom = texture(u_bloom, f_uv) * u_bloom_intensity;
    o_color = vec4(scene.rgb + bloom.rgb, scene.a);
}
"#;

// ── Pipeline ───────────────────────────────────────────────────────────────────

#[allow(dead_code)] // Some fields held for GL resource lifetime or future use
pub struct Pipeline {
    pub width:  u32,
    pub height: u32,
    running:    bool,

    // Windowing
    event_loop: EventLoop<()>,
    window:     Window,
    surface:    Surface<WindowSurface>,
    context:    PossiblyCurrentContext,

    // OpenGL — glyph pass
    gl:            glow::Context,
    program:       glow::Program,
    vao:           glow::VertexArray,
    quad_vbo:      glow::Buffer,
    instance_vbo:  glow::Buffer,
    atlas_tex:     glow::Texture,
    loc_view_proj: glow::UniformLocation,

    // Phase 7 — scene FBO
    scene_fbo: glow::Framebuffer,
    scene_tex: glow::Texture,

    // Phase 7 — bright-pass FBO
    bright_fbo: glow::Framebuffer,
    bright_tex: glow::Texture,

    // Phase 7 — blur ping-pong FBOs
    blur_fbo_a: glow::Framebuffer,
    blur_tex_a: glow::Texture,
    blur_fbo_b: glow::Framebuffer,
    blur_tex_b: glow::Texture,

    // Phase 7 — post-process programs
    extract_prog:   glow::Program,
    blur_prog:      glow::Program,
    composite_prog: glow::Program,

    // Uniform locations
    extract_loc_scene:     glow::UniformLocation,
    extract_loc_threshold: glow::UniformLocation,
    blur_loc_src:          glow::UniformLocation,
    blur_loc_direction:    glow::UniformLocation,
    comp_loc_scene:        glow::UniformLocation,
    comp_loc_bloom:        glow::UniformLocation,
    comp_loc_intensity:    glow::UniformLocation,
    comp_loc_chromatic:    glow::UniformLocation,

    // Font atlas (kept for UV lookups)
    atlas: FontAtlas,

    // CPU-side batch rebuilt every frame
    batch: GlyphBatch,
}

impl Pipeline {
    /// Initialize the window, OpenGL context, shaders, font atlas, and post-process FBOs.
    pub fn init(config: &EngineConfig) -> Self {
        // ── 1. Event loop ────────────────────────────────────────────────────
        let event_loop = EventLoop::new().expect("EventLoop::new");

        // ── 2. Window + GL config via glutin-winit DisplayBuilder ────────────
        // winit 0.30: WindowAttributes instead of WindowBuilder
        let window_attrs = Window::default_attributes()
            .with_title(&config.window_title)
            .with_inner_size(LogicalSize::new(config.window_width, config.window_height))
            .with_resizable(true);

        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_depth_size(0);

        // glutin-winit 0.5: with_window_attributes instead of with_window_builder
        let display_builder = DisplayBuilder::new().with_window_attributes(Some(window_attrs));

        let (window, gl_config) = display_builder
            .build(&event_loop, template, |mut configs| {
                configs.next().expect("no GL config found")
            })
            .expect("DisplayBuilder::build");

        let window = window.expect("window not created");

        let display = gl_config.display();

        // ── 3. GL context (OpenGL 3.3 Core) ──────────────────────────────────
        // rwh 0.6: use HasWindowHandle instead of HasRawWindowHandle
        let raw_window_handle = window.window_handle().unwrap().as_raw();
        let ctx_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(raw_window_handle));

        let not_current = unsafe {
            display
                .create_context(&gl_config, &ctx_attrs)
                .expect("create_context")
        };

        // ── 4. Window surface ─────────────────────────────────────────────────
        let size = window.inner_size();
        let w = size.width.max(1);
        let h = size.height.max(1);

        let surface_attrs = window
            .build_surface_attributes(Default::default())
            .expect("build_surface_attributes");
        let surface = unsafe {
            display
                .create_window_surface(&gl_config, &surface_attrs)
                .expect("create_window_surface")
        };

        // ── 5. Make current ───────────────────────────────────────────────────
        let context = not_current
            .make_current(&surface)
            .expect("make_current");

        // ── 6. glow context ───────────────────────────────────────────────────
        let gl = unsafe {
            glow::Context::from_loader_function(|sym| {
                let sym_c = CString::new(sym).unwrap();
                display.get_proc_address(sym_c.as_c_str()) as *const _
            })
        };

        // ── 7. Compile shaders ────────────────────────────────────────────────
        let program = unsafe { compile_program(&gl, VERT_SRC, FRAG_SRC) };
        let loc_view_proj = unsafe {
            gl.get_uniform_location(program, "u_view_proj")
                .expect("uniform u_view_proj not found")
        };
        unsafe {
            gl.use_program(Some(program));
            if let Some(loc) = gl.get_uniform_location(program, "u_atlas") {
                gl.uniform_1_i32(Some(&loc), 0);
            }
        }

        let extract_prog   = unsafe { compile_program(&gl, POSTFX_VERT, BRIGHT_EXTRACT_FRAG) };
        let blur_prog      = unsafe { compile_program(&gl, POSTFX_VERT, BLUR_FRAG) };
        let composite_prog = unsafe { compile_program(&gl, POSTFX_VERT, COMPOSITE_FRAG) };

        let (extract_loc_scene, extract_loc_threshold) = unsafe {
            gl.use_program(Some(extract_prog));
            let s = gl.get_uniform_location(extract_prog, "u_scene").expect("extract u_scene");
            let t = gl.get_uniform_location(extract_prog, "u_threshold").expect("extract u_threshold");
            gl.uniform_1_i32(Some(&s), 0);
            gl.uniform_1_f32(Some(&t), 0.6);
            (s, t)
        };

        let (blur_loc_src, blur_loc_direction) = unsafe {
            gl.use_program(Some(blur_prog));
            let s = gl.get_uniform_location(blur_prog, "u_src").expect("blur u_src");
            let d = gl.get_uniform_location(blur_prog, "u_direction").expect("blur u_direction");
            gl.uniform_1_i32(Some(&s), 0);
            (s, d)
        };

        let (comp_loc_scene, comp_loc_bloom, comp_loc_intensity, comp_loc_chromatic) = unsafe {
            gl.use_program(Some(composite_prog));
            let sc = gl.get_uniform_location(composite_prog, "u_scene").expect("comp u_scene");
            let bl = gl.get_uniform_location(composite_prog, "u_bloom").expect("comp u_bloom");
            let bi = gl.get_uniform_location(composite_prog, "u_bloom_intensity").expect("comp u_bloom_intensity");
            let ch = gl.get_uniform_location(composite_prog, "u_chromatic").expect("comp u_chromatic");
            gl.uniform_1_i32(Some(&sc), 0);
            gl.uniform_1_i32(Some(&bl), 1);
            gl.uniform_1_f32(Some(&bi), 0.8);
            gl.uniform_1_f32(Some(&ch), 0.003);
            (sc, bl, bi, ch)
        };

        // ── 8. VAO + VBOs ─────────────────────────────────────────────────────
        let (vao, quad_vbo, instance_vbo) = unsafe { setup_vao(&gl) };

        // ── 9. Font atlas ─────────────────────────────────────────────────────
        let atlas = FontAtlas::build(config.render.font_size as f32);
        let atlas_tex = unsafe { upload_atlas(&gl, &atlas) };

        // ── 10. Post-process FBOs ─────────────────────────────────────────────
        let (scene_fbo, scene_tex,
             bright_fbo, bright_tex,
             blur_fbo_a, blur_tex_a,
             blur_fbo_b, blur_tex_b) = unsafe { create_fbos(&gl, w, h) };

        // ── 11. Global GL state ───────────────────────────────────────────────
        unsafe {
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.clear_color(0.02, 0.02, 0.05, 1.0);
            gl.viewport(0, 0, w as i32, h as i32);
        }

        log::info!(
            "Pipeline ready — {}×{} — font atlas {}×{} ({} chars)",
            w, h, atlas.width, atlas.height, atlas.uvs.len()
        );

        Self {
            width: w, height: h,
            running: true,
            event_loop, window, surface, context,
            gl, program, vao, quad_vbo, instance_vbo, atlas_tex, loc_view_proj,
            scene_fbo, scene_tex,
            bright_fbo, bright_tex,
            blur_fbo_a, blur_tex_a,
            blur_fbo_b, blur_tex_b,
            extract_prog, blur_prog, composite_prog,
            extract_loc_scene, extract_loc_threshold,
            blur_loc_src, blur_loc_direction,
            comp_loc_scene, comp_loc_bloom, comp_loc_intensity, comp_loc_chromatic,
            atlas,
            batch: GlyphBatch::new(),
        }
    }

    /// Process pending window events; returns false if the window was closed.
    pub fn poll_events(&mut self, input: &mut InputState) -> bool {
        input.clear_frame();

        let mut should_exit = false;
        let mut resize: Option<(u32, u32)> = None;
        let mut key_events: Vec<(KeyCode, bool)> = Vec::new();

        #[allow(deprecated)]
        let status = self.event_loop.pump_events(Some(Duration::ZERO), |event, elwt| {
            if let Event::WindowEvent { event: we, .. } = event {
                match we {
                    WindowEvent::CloseRequested => {
                        should_exit = true;
                        elwt.exit();
                    }
                    WindowEvent::Resized(s) => {
                        resize = Some((s.width, s.height));
                    }
                    WindowEvent::KeyboardInput { event: key_ev, .. } => {
                        if let PhysicalKey::Code(kc) = key_ev.physical_key {
                            let pressed = key_ev.state == ElementState::Pressed;
                            key_events.push((kc, pressed));
                        }
                    }
                    _ => {}
                }
            }
        });

        if let Some((w, h)) = resize {
            if w > 0 && h > 0 {
                self.surface.resize(
                    &self.context,
                    NonZeroU32::new(w).unwrap(),
                    NonZeroU32::new(h).unwrap(),
                );
                unsafe { self.gl.viewport(0, 0, w as i32, h as i32); }
                self.width  = w;
                self.height = h;
                input.window_resized = Some((w, h));
                unsafe { self.resize_fbos(w, h); }
            }
        }

        for (kc, pressed) in key_events {
            if let Some(key) = keycode_to_engine(kc) {
                if pressed {
                    input.keys_pressed.insert(key);
                    input.keys_just_pressed.insert(key);
                } else {
                    input.keys_pressed.remove(&key);
                    input.keys_just_released.insert(key);
                }
            }
        }

        if should_exit || matches!(status, PumpStatus::Exit(_)) {
            self.running = false;
        }
        self.running
    }

    /// Collect all visible glyphs + particles into the batch and draw with post-processing.
    pub fn render(&mut self, scene: &Scene, camera: &ProofCamera) {
        let pos    = camera.position.position();
        let tgt    = camera.target.position();
        let fov    = camera.fov.position;
        let aspect = if self.height > 0 { self.width as f32 / self.height as f32 } else { 1.0 };
        let view      = Mat4::look_at_rh(pos, tgt, Vec3::Y);
        let proj      = Mat4::perspective_rh_gl(fov.to_radians(), aspect, camera.near, camera.far);
        let view_proj = proj * view;

        self.batch.clear();

        for (_, glyph) in scene.glyphs.iter() {
            if !glyph.visible { continue; }
            let life_scale = if let Some(ref f) = glyph.life_function {
                f.evaluate(scene.time, 0.0)
            } else {
                1.0
            };
            let uv = self.atlas.uv_for(glyph.character);
            self.batch.push(GlyphInstance {
                position:    glyph.position.to_array(),
                scale:       [glyph.scale.x * life_scale, glyph.scale.y * life_scale],
                rotation:    glyph.rotation,
                color:       glyph.color.to_array(),
                emission:    glyph.emission,
                glow_color:  glyph.glow_color.to_array(),
                glow_radius: glyph.glow_radius,
                uv_offset:   uv.offset(),
                uv_size:     uv.size(),
                _pad:        [0.0; 2],
            });
        }

        for particle in scene.particles.iter() {
            let g = &particle.glyph;
            if !g.visible { continue; }
            let uv = self.atlas.uv_for(g.character);
            self.batch.push(GlyphInstance {
                position:    g.position.to_array(),
                scale:       [g.scale.x, g.scale.y],
                rotation:    g.rotation,
                color:       g.color.to_array(),
                emission:    g.emission,
                glow_color:  g.glow_color.to_array(),
                glow_radius: g.glow_radius,
                uv_offset:   uv.offset(),
                uv_size:     uv.size(),
                _pad:        [0.0; 2],
            });
        }

        unsafe { self.draw_postfx(view_proj); }
    }

    /// Swap the back buffer to screen. Returns false if the window was closed.
    pub fn swap(&mut self) -> bool {
        if let Err(e) = self.surface.swap_buffers(&self.context) {
            log::error!("swap_buffers: {e}");
            self.running = false;
        }
        self.running
    }

    // ── Private GL multi-pass draw ─────────────────────────────────────────────

    unsafe fn draw_postfx(&mut self, view_proj: Mat4) {
        let gl = &self.gl;

        // ── Pass 1: Render glyphs → scene FBO ────────────────────────────────
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.scene_fbo));
        gl.clear(glow::COLOR_BUFFER_BIT);

        if !self.batch.is_empty() {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instance_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                cast_slice(self.batch.instances.as_slice()),
                glow::DYNAMIC_DRAW,
            );
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(
                Some(&self.loc_view_proj), false, &view_proj.to_cols_array(),
            );
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_tex));
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.batch.len() as i32);
        }

        // ── Pass 2: Bright-pass extract (scene_tex → bright_fbo) ─────────────
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.bright_fbo));
        gl.disable(glow::BLEND);
        gl.use_program(Some(self.extract_prog));
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.scene_tex));
        gl.draw_arrays(glow::TRIANGLES, 0, 3); // full-screen triangle

        // ── Pass 3: Horizontal blur (bright_tex → blur_fbo_a) ────────────────
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.blur_fbo_a));
        gl.use_program(Some(self.blur_prog));
        gl.uniform_2_f32(Some(&self.blur_loc_direction), 1.0, 0.0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.bright_tex));
        gl.draw_arrays(glow::TRIANGLES, 0, 3);

        // ── Pass 4: Vertical blur (blur_tex_a → blur_fbo_b) ──────────────────
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.blur_fbo_b));
        gl.uniform_2_f32(Some(&self.blur_loc_direction), 0.0, 1.0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.blur_tex_a));
        gl.draw_arrays(glow::TRIANGLES, 0, 3);

        // ── Pass 5: Composite → default framebuffer ───────────────────────────
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.enable(glow::BLEND);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.use_program(Some(self.composite_prog));
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.scene_tex));
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.blur_tex_b));
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
    }

    /// Recreate all FBO textures after a window resize.
    unsafe fn resize_fbos(&mut self, w: u32, h: u32) {
        let gl = &self.gl;

        gl.delete_framebuffer(self.scene_fbo);
        gl.delete_texture(self.scene_tex);
        gl.delete_framebuffer(self.bright_fbo);
        gl.delete_texture(self.bright_tex);
        gl.delete_framebuffer(self.blur_fbo_a);
        gl.delete_texture(self.blur_tex_a);
        gl.delete_framebuffer(self.blur_fbo_b);
        gl.delete_texture(self.blur_tex_b);

        let (scene_fbo, scene_tex,
             bright_fbo, bright_tex,
             blur_fbo_a, blur_tex_a,
             blur_fbo_b, blur_tex_b) = create_fbos(gl, w, h);

        self.scene_fbo  = scene_fbo;
        self.scene_tex  = scene_tex;
        self.bright_fbo = bright_fbo;
        self.bright_tex = bright_tex;
        self.blur_fbo_a = blur_fbo_a;
        self.blur_tex_a = blur_tex_a;
        self.blur_fbo_b = blur_fbo_b;
        self.blur_tex_b = blur_tex_b;
    }
}

// ── GL helpers ─────────────────────────────────────────────────────────────────

unsafe fn compile_program(gl: &glow::Context, vert_src: &str, frag_src: &str) -> glow::Program {
    let vs = gl.create_shader(glow::VERTEX_SHADER).expect("create vertex shader");
    gl.shader_source(vs, vert_src);
    gl.compile_shader(vs);
    if !gl.get_shader_compile_status(vs) {
        panic!("Vertex shader compile error:\n{}", gl.get_shader_info_log(vs));
    }

    let fs = gl.create_shader(glow::FRAGMENT_SHADER).expect("create fragment shader");
    gl.shader_source(fs, frag_src);
    gl.compile_shader(fs);
    if !gl.get_shader_compile_status(fs) {
        panic!("Fragment shader compile error:\n{}", gl.get_shader_info_log(fs));
    }

    let prog = gl.create_program().expect("create program");
    gl.attach_shader(prog, vs);
    gl.attach_shader(prog, fs);
    gl.link_program(prog);
    if !gl.get_program_link_status(prog) {
        panic!("Shader link error:\n{}", gl.get_program_info_log(prog));
    }

    gl.detach_shader(prog, vs);
    gl.detach_shader(prog, fs);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    prog
}

/// Create VAO with per-vertex quad data (locations 0-1) and per-instance data (locations 2-10).
unsafe fn setup_vao(
    gl: &glow::Context,
) -> (glow::VertexArray, glow::Buffer, glow::Buffer) {
    let vao = gl.create_vertex_array().expect("create vao");
    gl.bind_vertex_array(Some(vao));

    let quad_vbo = gl.create_buffer().expect("create quad_vbo");
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad_vbo));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, cast_slice(&QUAD_VERTS), glow::STATIC_DRAW);
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
    gl.enable_vertex_attrib_array(1);

    let instance_vbo = gl.create_buffer().expect("create instance_vbo");
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_vbo));

    let stride = std::mem::size_of::<GlyphInstance>() as i32; // 84

    macro_rules! inst_attr {
        ($loc:expr, $count:expr, $off:expr) => {
            gl.vertex_attrib_pointer_f32($loc, $count, glow::FLOAT, false, stride, $off);
            gl.enable_vertex_attrib_array($loc);
            gl.vertex_attrib_divisor($loc, 1);
        };
    }

    inst_attr!(2,  3,  0);  // i_position
    inst_attr!(3,  2, 12);  // i_scale
    inst_attr!(4,  1, 20);  // i_rotation
    inst_attr!(5,  4, 24);  // i_color
    inst_attr!(6,  1, 40);  // i_emission
    inst_attr!(7,  3, 44);  // i_glow_color
    inst_attr!(8,  1, 56);  // i_glow_radius
    inst_attr!(9,  2, 60);  // i_uv_offset
    inst_attr!(10, 2, 68);  // i_uv_size

    (vao, quad_vbo, instance_vbo)
}

/// Upload a FontAtlas as an R8 GL texture; returns the texture handle.
unsafe fn upload_atlas(gl: &glow::Context, atlas: &FontAtlas) -> glow::Texture {
    let tex = gl.create_texture().expect("create atlas texture");
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
    gl.tex_image_2d(
        glow::TEXTURE_2D, 0,
        glow::R8 as i32,
        atlas.width as i32, atlas.height as i32,
        0, glow::RED, glow::UNSIGNED_BYTE,
        Some(&atlas.pixels),
    );
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    tex
}

/// Create a color attachment texture for an FBO.
unsafe fn make_color_tex(gl: &glow::Context, w: u32, h: u32) -> glow::Texture {
    let tex = gl.create_texture().expect("create fbo tex");
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, w as i32, h as i32,
                    0, glow::RGBA, glow::UNSIGNED_BYTE, None);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    tex
}

/// Create a single-attachment FBO backed by a color texture.
unsafe fn make_fbo(gl: &glow::Context, tex: glow::Texture) -> glow::Framebuffer {
    let fbo = gl.create_framebuffer().expect("create framebuffer");
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0,
                              glow::TEXTURE_2D, Some(tex), 0);
    fbo
}

/// Create all post-processing FBOs for the given resolution.
#[allow(clippy::type_complexity)]
unsafe fn create_fbos(gl: &glow::Context, w: u32, h: u32) -> (
    glow::Framebuffer, glow::Texture,
    glow::Framebuffer, glow::Texture,
    glow::Framebuffer, glow::Texture,
    glow::Framebuffer, glow::Texture,
) {
    let scene_tex  = make_color_tex(gl, w, h);
    let bright_tex = make_color_tex(gl, w, h);
    let blur_tex_a = make_color_tex(gl, w, h);
    let blur_tex_b = make_color_tex(gl, w, h);

    let scene_fbo  = make_fbo(gl, scene_tex);
    let bright_fbo = make_fbo(gl, bright_tex);
    let blur_fbo_a = make_fbo(gl, blur_tex_a);
    let blur_fbo_b = make_fbo(gl, blur_tex_b);

    gl.bind_framebuffer(glow::FRAMEBUFFER, None);

    (scene_fbo, scene_tex, bright_fbo, bright_tex, blur_fbo_a, blur_tex_a, blur_fbo_b, blur_tex_b)
}

/// Map winit KeyCode → engine Key.  Returns None for unrecognised keys.
fn keycode_to_engine(kc: KeyCode) -> Option<Key> {
    Some(match kc {
        KeyCode::KeyA => Key::A, KeyCode::KeyB => Key::B, KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D, KeyCode::KeyE => Key::E, KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G, KeyCode::KeyH => Key::H, KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J, KeyCode::KeyK => Key::K, KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M, KeyCode::KeyN => Key::N, KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P, KeyCode::KeyQ => Key::Q, KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S, KeyCode::KeyT => Key::T, KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V, KeyCode::KeyW => Key::W, KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y, KeyCode::KeyZ => Key::Z,
        KeyCode::Digit1 => Key::Num1, KeyCode::Digit2 => Key::Num2,
        KeyCode::Digit3 => Key::Num3, KeyCode::Digit4 => Key::Num4,
        KeyCode::Digit5 => Key::Num5, KeyCode::Digit6 => Key::Num6,
        KeyCode::Digit7 => Key::Num7, KeyCode::Digit8 => Key::Num8,
        KeyCode::Digit9 => Key::Num9, KeyCode::Digit0 => Key::Num0,
        KeyCode::ArrowUp    => Key::Up,    KeyCode::ArrowDown  => Key::Down,
        KeyCode::ArrowLeft  => Key::Left,  KeyCode::ArrowRight => Key::Right,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Escape    => Key::Escape,
        KeyCode::Space     => Key::Space,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Tab       => Key::Tab,
        KeyCode::ShiftLeft   => Key::LShift,  KeyCode::ShiftRight   => Key::RShift,
        KeyCode::ControlLeft => Key::LCtrl,   KeyCode::ControlRight => Key::RCtrl,
        KeyCode::AltLeft     => Key::LAlt,    KeyCode::AltRight     => Key::RAlt,
        KeyCode::F1  => Key::F1,  KeyCode::F2  => Key::F2,  KeyCode::F3  => Key::F3,
        KeyCode::F4  => Key::F4,  KeyCode::F5  => Key::F5,  KeyCode::F6  => Key::F6,
        KeyCode::F7  => Key::F7,  KeyCode::F8  => Key::F8,  KeyCode::F9  => Key::F9,
        KeyCode::F10 => Key::F10, KeyCode::F11 => Key::F11, KeyCode::F12 => Key::F12,
        KeyCode::Slash        => Key::Slash,
        KeyCode::Backslash    => Key::Backslash,
        KeyCode::Period       => Key::Period,
        KeyCode::Comma        => Key::Comma,
        KeyCode::Semicolon    => Key::Semicolon,
        KeyCode::Quote        => Key::Quote,
        KeyCode::BracketLeft  => Key::LBracket,
        KeyCode::BracketRight => Key::RBracket,
        KeyCode::Minus        => Key::Minus,
        KeyCode::Equal        => Key::Equals,
        KeyCode::Backquote    => Key::Backtick,
        _ => return None,
    })
}
