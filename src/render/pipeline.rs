//! Render pipeline — glutin/winit window, OpenGL context, instanced glyph batch rendering.
//!
//! Phase 1 implementation:
//!   - glutin-winit DisplayBuilder creates the window + GL config together
//!   - GL 3.3 Core context via glow
//!   - Font atlas uploaded as R8 texture (ab_glyph or fallback)
//!   - All glyphs rendered as instanced textured quads
//!   - winit pump_events drives the input loop without blocking

use std::num::NonZeroU32;
use std::ffi::CString;
use std::time::Duration;

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext,
                      PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SurfaceAttributesBuilder, WindowSurface};
use glutin_winit::DisplayBuilder;
use glow::HasContext;
use raw_window_handle::HasRawWindowHandle;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::{Window, WindowBuilder};
use glam::{Mat4, Vec3};
use bytemuck::cast_slice;

use crate::config::EngineConfig;
use crate::scene::Scene;
use crate::render::camera::ProofCamera;
use crate::input::{InputState, Key};
use crate::glyph::atlas::FontAtlas;
use crate::glyph::batch::{GlyphBatch, GlyphInstance};

// ── Shaders (Phase 1 — single render target, emission blended inline) ──────────

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

// ── Pipeline ───────────────────────────────────────────────────────────────────

pub struct Pipeline {
    pub width:  u32,
    pub height: u32,
    running:    bool,

    // Windowing
    event_loop: EventLoop<()>,
    window:     Window,
    surface:    Surface<WindowSurface>,
    context:    PossiblyCurrentContext,

    // OpenGL
    gl:            glow::Context,
    program:       glow::Program,
    vao:           glow::VertexArray,
    quad_vbo:      glow::Buffer,
    instance_vbo:  glow::Buffer,
    atlas_tex:     glow::Texture,
    loc_view_proj: glow::UniformLocation,

    // Font atlas (kept for UV lookups)
    atlas: FontAtlas,

    // CPU-side batch rebuilt every frame
    batch: GlyphBatch,
}

impl Pipeline {
    /// Initialize the window, OpenGL context, shaders, and font atlas.
    pub fn init(config: &EngineConfig) -> Self {
        // ── 1. Event loop ────────────────────────────────────────────────────
        let event_loop = EventLoop::new().expect("EventLoop::new");

        // ── 2. Window + GL config via glutin-winit DisplayBuilder ────────────
        // Note: pass ConfigTemplateBuilder directly (not built ConfigTemplate)
        let window_builder = WindowBuilder::new()
            .with_title(&config.window_title)
            .with_inner_size(LogicalSize::new(config.window_width, config.window_height))
            .with_resizable(true);

        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_depth_size(0);

        let display_builder = DisplayBuilder::new().with_window_builder(Some(window_builder));

        let (window, gl_config) = display_builder
            .build(&event_loop, template, |mut configs| {
                configs.next().expect("no GL config found")
            })
            .expect("DisplayBuilder::build");

        let window = window.expect("window not created");

        let display = gl_config.display();

        // ── 3. GL context (OpenGL 3.3 Core) ──────────────────────────────────
        let ctx_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(window.raw_window_handle()));

        let not_current = unsafe {
            display
                .create_context(&gl_config, &ctx_attrs)
                .expect("create_context")
        };

        // ── 4. Window surface ─────────────────────────────────────────────────
        let size = window.inner_size();
        let surface_attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            window.raw_window_handle(),
            NonZeroU32::new(size.width.max(1)).unwrap(),
            NonZeroU32::new(size.height.max(1)).unwrap(),
        );
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

        // Bind atlas sampler to texture unit 0 (set once, never changes)
        unsafe {
            gl.use_program(Some(program));
            if let Some(loc) = gl.get_uniform_location(program, "u_atlas") {
                gl.uniform_1_i32(Some(&loc), 0);
            }
        }

        // ── 8. VAO + VBOs ─────────────────────────────────────────────────────
        let (vao, quad_vbo, instance_vbo) = unsafe { setup_vao(&gl) };

        // ── 9. Font atlas ─────────────────────────────────────────────────────
        let atlas = FontAtlas::build(config.render.font_size as f32);
        let atlas_tex = unsafe { upload_atlas(&gl, &atlas) };

        // ── 10. Global GL state ───────────────────────────────────────────────
        unsafe {
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.clear_color(0.02, 0.02, 0.05, 1.0);
            gl.viewport(0, 0, size.width as i32, size.height as i32);
        }

        log::info!(
            "Pipeline ready — {}×{} — font atlas {}×{} ({} chars)",
            size.width, size.height,
            atlas.width, atlas.height,
            atlas.uvs.len()
        );

        Self {
            width:  size.width,
            height: size.height,
            running: true,
            event_loop,
            window,
            surface,
            context,
            gl,
            program,
            vao,
            quad_vbo,
            instance_vbo,
            atlas_tex,
            loc_view_proj,
            atlas,
            batch: GlyphBatch::new(),
        }
    }

    /// Process pending window events; returns false if the window was closed.
    pub fn poll_events(&mut self, input: &mut InputState) -> bool {
        input.clear_frame();

        // Collect raw events into local Vecs to avoid borrow-checker conflicts
        let mut should_exit = false;
        let mut resize: Option<(u32, u32)> = None;
        // (KeyCode, pressed)
        let mut key_events: Vec<(KeyCode, bool)> = Vec::new();

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

        // Apply resize
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
            }
        }

        // Apply key events
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

    /// Collect all visible glyphs + particles into the batch and draw.
    pub fn render(&mut self, scene: &Scene, camera: &ProofCamera) {
        // Compute view-projection from camera's current spring positions
        let pos    = camera.position.position();
        let tgt    = camera.target.position();
        let fov    = camera.fov.position;
        let aspect = if self.height > 0 { self.width as f32 / self.height as f32 } else { 1.0 };
        let view      = Mat4::look_at_rh(pos, tgt, Vec3::Y);
        let proj      = Mat4::perspective_rh_gl(fov.to_radians(), aspect, camera.near, camera.far);
        let view_proj = proj * view;

        // ── Build instance batch ───────────────────────────────────────────────
        self.batch.clear();

        for (_, glyph) in scene.glyphs.iter() {
            if !glyph.visible { continue; }

            // life_function modulates scale (e.g. Breathing oscillates scale)
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

        // ── Draw ──────────────────────────────────────────────────────────────
        unsafe {
            self.gl.clear(glow::COLOR_BUFFER_BIT);

            if self.batch.is_empty() { return; }

            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instance_vbo));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                cast_slice(self.batch.instances.as_slice()),
                glow::DYNAMIC_DRAW,
            );

            self.gl.use_program(Some(self.program));
            self.gl.uniform_matrix_4_f32_slice(
                Some(&self.loc_view_proj),
                false,
                &view_proj.to_cols_array(),
            );

            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_tex));

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.batch.len() as i32);
        }
    }

    /// Swap the back buffer to screen. Returns false if the window was closed.
    pub fn swap(&mut self) -> bool {
        if let Err(e) = self.surface.swap_buffers(&self.context) {
            log::error!("swap_buffers: {e}");
            self.running = false;
        }
        self.running
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

    // ── Quad VBO (static, per-vertex) ────────────────────────────────────────
    let quad_vbo = gl.create_buffer().expect("create quad_vbo");
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad_vbo));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, cast_slice(&QUAD_VERTS), glow::STATIC_DRAW);
    // loc 0: vec2 v_pos  (stride 16, offset 0)
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
    gl.enable_vertex_attrib_array(0);
    // loc 1: vec2 v_uv   (stride 16, offset 8)
    gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
    gl.enable_vertex_attrib_array(1);

    // ── Instance VBO (dynamic, per-instance) ─────────────────────────────────
    // GlyphInstance layout (repr(C), 84 bytes):
    //   [f32;3] position   offset  0
    //   [f32;2] scale      offset 12
    //   f32     rotation   offset 20
    //   [f32;4] color      offset 24
    //   f32     emission   offset 40
    //   [f32;3] glow_color offset 44
    //   f32     glow_radius offset 56
    //   [f32;2] uv_offset  offset 60
    //   [f32;2] uv_size    offset 68
    //   [f32;2] _pad       offset 76
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
