//! Render pipeline — glutin 0.32 / winit 0.30 window + OpenGL 3.3 Core context,
//! instanced glyph batch rendering, and the full multi-pass post-processing pipeline
//! (bloom, chromatic aberration, film grain, vignette, scanlines) wired through
//! `PostFxPipeline` so that `RenderConfig` actually controls runtime behaviour.
//!
//! # Post-processing flow
//!
//! ```text
//! GlyphPass (to scene FBO, dual attachments)
//!   └─ color    ──┐
//!   └─ emission ──┤
//!                 ├─ PostFxPipeline::run(RenderConfig)
//!                 │   ├─ Bloom H-blur
//!                 │   ├─ Bloom V-blur   (×2 for softness)
//!                 │   └─ Composite: scene + bloom + CA + grain + vignette → screen
//!                 └─► Default framebuffer
//! ```

use std::num::NonZeroU32;
use std::ffi::CString;
use std::time::{Duration, Instant};

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext,
                      PossiblyCurrentContext, Version};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use glow::HasContext;
use raw_window_handle::HasWindowHandle;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::Window;
use glam::{Mat4, Vec2, Vec3};
use bytemuck::cast_slice;

use crate::config::{EngineConfig, RenderConfig};
use crate::scene::Scene;
use crate::render::camera::ProofCamera;
use crate::render::postfx::PostFxPipeline;
use crate::input::{InputState, Key};
use crate::glyph::atlas::FontAtlas;
use crate::glyph::batch::GlyphInstance;

// ── Glyph vertex shader ────────────────────────────────────────────────────────

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
out float f_glow_radius;

void main() {
    float c = cos(i_rotation);
    float s = sin(i_rotation);
    vec2 rotated = vec2(
        v_pos.x * c - v_pos.y * s,
        v_pos.x * s + v_pos.y * c
    ) * i_scale;

    vec4 pos = u_view_proj * vec4(i_position + vec3(rotated, 0.0), 1.0);
    pos.x = -pos.x;  // flip X: look_at_rh from +Z has right=-X, this corrects it
    gl_Position = pos;

    f_uv         = i_uv_offset + v_uv * i_uv_size;
    f_color      = i_color;
    f_emission   = i_emission;
    f_glow_color = i_glow_color;
    f_glow_radius = i_glow_radius;
}
"#;

/// Glyph fragment shader with dual output: color + emission.
///
/// `o_color`    → COLOR_ATTACHMENT0 — blended scene color
/// `o_emission` → COLOR_ATTACHMENT1 — bloom input (high-intensity glowing pixels)
const FRAG_SRC: &str = r#"
#version 330 core

in vec2  f_uv;
in vec4  f_color;
in float f_emission;
in vec3  f_glow_color;
in float f_glow_radius;

uniform sampler2D u_atlas;

layout(location = 0) out vec4 o_color;
layout(location = 1) out vec4 o_emission;

void main() {
    float alpha = texture(u_atlas, f_uv).r;
    if (alpha < 0.05) discard;

    // Base color with emission tint
    float em  = clamp(f_emission * 0.5, 0.0, 1.0);
    vec3  col = mix(f_color.rgb, f_glow_color, em);
    o_color   = vec4(col, alpha * f_color.a);

    // Emission output: only bright, glowing pixels go to the bloom input.
    // The bloom amount is proportional to (emission - 0.3), clamped.
    float bloom_strength = clamp(f_emission - 0.3, 0.0, 1.0);
    // Add glow radius influence — higher glow_radius means more bloom spread
    float glow_boost     = clamp(f_glow_radius * 0.15, 0.0, 0.8);
    o_emission = vec4(f_glow_color * (bloom_strength + glow_boost), alpha * f_color.a);
}
"#;

// ── Unit quad geometry ─────────────────────────────────────────────────────────

/// Unit quad: 6 vertices (2 CCW triangles), each: [pos_x, pos_y, uv_x, uv_y]
#[rustfmt::skip]
const QUAD_VERTS: [f32; 24] = [
    -0.5,  0.5,  0.0, 1.0,
    -0.5, -0.5,  0.0, 0.0,
     0.5,  0.5,  1.0, 1.0,
    -0.5, -0.5,  0.0, 0.0,
     0.5, -0.5,  1.0, 0.0,
     0.5,  0.5,  1.0, 1.0,
];

// ── FrameStats ─────────────────────────────────────────────────────────────────

/// Per-frame rendering statistics.
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    /// Frames per second (rolling average over 60 frames).
    pub fps:              f32,
    /// Time of last frame in seconds.
    pub dt:               f32,
    /// Number of glyphs drawn this frame.
    pub glyph_count:      usize,
    /// Number of particles drawn this frame.
    pub particle_count:   usize,
    /// Number of draw calls this frame.
    pub draw_calls:       u32,
    /// Total frame number since engine start.
    pub frame_number:     u64,
}

/// Rolling FPS calculator over N frames.
struct FpsCounter {
    samples:   [f32; 60],
    head:      usize,
    filled:    bool,
}

impl FpsCounter {
    fn new() -> Self { Self { samples: [0.016; 60], head: 0, filled: false } }

    fn push(&mut self, dt: f32) {
        self.samples[self.head] = dt.max(f32::EPSILON);
        self.head = (self.head + 1) % 60;
        if self.head == 0 { self.filled = true; }
    }

    fn fps(&self) -> f32 {
        let count = if self.filled { 60 } else { self.head.max(1) };
        let avg_dt: f32 = self.samples[..count].iter().sum::<f32>() / count as f32;
        1.0 / avg_dt
    }
}

// ── Pipeline ───────────────────────────────────────────────────────────────────

/// The main render pipeline.
///
/// Created once by `ProofEngine::new()` and kept alive for the duration of the game.
/// Owns the window, OpenGL context, shader programs, font atlas, glyph VAO, and
/// the post-processing pipeline.
#[allow(dead_code)]
pub struct Pipeline {
    // ── Runtime info ──────────────────────────────────────────────────────────
    pub width:   u32,
    pub height:  u32,
    pub stats:   FrameStats,
    running:     bool,

    // ── Config snapshot (not a reference — the engine owns EngineConfig) ──────
    render_config: RenderConfig,

    // ── Windowing ────────────────────────────────────────────────────────────
    event_loop: EventLoop<()>,
    window:     Window,
    surface:    Surface<WindowSurface>,
    context:    PossiblyCurrentContext,

    // ── OpenGL glyph pass ─────────────────────────────────────────────────────
    gl:            glow::Context,
    program:       glow::Program,
    vao:           glow::VertexArray,
    quad_vbo:      glow::Buffer,
    instance_vbo:  glow::Buffer,
    atlas_tex:     glow::Texture,
    loc_view_proj: glow::UniformLocation,

    // ── Post-processing pipeline (the real deal — reads RenderConfig) ─────────
    postfx: PostFxPipeline,

    // ── Font atlas ────────────────────────────────────────────────────────────
    atlas: FontAtlas,

    // ── CPU-side glyph batch ──────────────────────────────────────────────────
    instances: Vec<GlyphInstance>,

    // ── Timing ────────────────────────────────────────────────────────────────
    fps_counter:  FpsCounter,
    frame_start:  Instant,
    scene_time:   f32,

    // ── Mouse state ───────────────────────────────────────────────────────────
    mouse_pos:      Vec2,
    mouse_pos_prev: Vec2,
    /// Normalized device coordinates (NDC) of the mouse cursor.
    mouse_ndc:      Vec2,
}

impl Pipeline {
    /// Initialize window, OpenGL 3.3 Core context, shader programs, font atlas, and PostFxPipeline.
    pub fn init(config: &EngineConfig) -> Self {
        // ── 1. winit EventLoop ────────────────────────────────────────────────
        let event_loop = EventLoop::new().expect("EventLoop::new");

        // ── 2. Window attributes (winit 0.30 API) ─────────────────────────────
        let window_attrs = Window::default_attributes()
            .with_title(&config.window_title)
            .with_inner_size(LogicalSize::new(config.window_width, config.window_height))
            .with_resizable(true);

        // ── 3. GL config via DisplayBuilder (glutin-winit 0.5) ────────────────
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_depth_size(0);

        let display_builder = DisplayBuilder::new()
            .with_window_attributes(Some(window_attrs));

        let (window, gl_config) = display_builder
            .build(&event_loop, template, |mut configs| {
                configs.next().expect("no suitable GL config found")
            })
            .expect("DisplayBuilder::build failed");

        let window = window.expect("window was not created");
        let display = gl_config.display();

        // ── 4. OpenGL 3.3 Core context ────────────────────────────────────────
        let raw_handle = window.window_handle().unwrap().as_raw();
        let ctx_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(Some(raw_handle));

        let not_current = unsafe {
            display.create_context(&gl_config, &ctx_attrs)
                   .expect("create_context failed")
        };

        // ── 5. Window surface ─────────────────────────────────────────────────
        let size = window.inner_size();
        let w = size.width.max(1);
        let h = size.height.max(1);

        let surface_attrs = window
            .build_surface_attributes(Default::default())
            .expect("build_surface_attributes failed");

        let surface = unsafe {
            display.create_window_surface(&gl_config, &surface_attrs)
                   .expect("create_window_surface failed")
        };

        // ── 6. Make current ───────────────────────────────────────────────────
        let context = not_current.make_current(&surface)
                                 .expect("make_current failed");

        // ── 7. glow context from proc address ─────────────────────────────────
        let gl = unsafe {
            glow::Context::from_loader_function(|sym| {
                let sym_c = CString::new(sym).unwrap();
                display.get_proc_address(sym_c.as_c_str()) as *const _
            })
        };

        // ── 8. Compile glyph program ──────────────────────────────────────────
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

        // ── 9. Geometry: VAO + VBOs ───────────────────────────────────────────
        let (vao, quad_vbo, instance_vbo) = unsafe { setup_vao(&gl) };

        // ── 10. Font atlas ────────────────────────────────────────────────────
        let atlas     = FontAtlas::build(config.render.font_size as f32);
        let atlas_tex = unsafe { upload_atlas(&gl, &atlas) };

        // ── 11. PostFxPipeline — dual-attachment FBOs + bloom shaders ────────
        let postfx = unsafe { PostFxPipeline::new(&gl, w, h) };

        // ── 12. Global GL state ───────────────────────────────────────────────
        unsafe {
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.clear_color(0.02, 0.02, 0.05, 1.0);
            gl.viewport(0, 0, w as i32, h as i32);
        }

        log::info!(
            "Pipeline ready — {}×{} — font atlas {}×{} ({} chars) — PostFxPipeline wired",
            w, h, atlas.width, atlas.height, atlas.uvs.len()
        );

        Self {
            width: w, height: h,
            stats: FrameStats::default(),
            running: true,
            render_config: config.render.clone(),
            event_loop, window, surface, context,
            gl, program, vao, quad_vbo, instance_vbo, atlas_tex, loc_view_proj,
            postfx,
            atlas,
            instances: Vec::with_capacity(8192),
            fps_counter: FpsCounter::new(),
            frame_start: Instant::now(),
            scene_time: 0.0,
            mouse_pos: Vec2::ZERO,
            mouse_pos_prev: Vec2::ZERO,
            mouse_ndc: Vec2::ZERO,
        }
    }

    /// Update the render config used by the PostFx pipeline this frame.
    /// Call from `ProofEngine::run()` whenever the config changes.
    pub fn update_render_config(&mut self, config: &RenderConfig) {
        self.render_config = config.clone();
    }

    /// Poll window events and update `InputState`. Returns false on quit.
    pub fn poll_events(&mut self, input: &mut InputState) -> bool {
        input.clear_frame();
        self.mouse_pos_prev = self.mouse_pos;

        let mut should_exit = false;
        let mut resize:     Option<(u32, u32)>  = None;
        let mut key_events: Vec<(KeyCode, bool)> = Vec::new();
        let mut mouse_moved:     Option<(f64, f64)> = None;
        let mut mouse_buttons:   Vec<(MouseButton, bool)> = Vec::new();
        let mut scroll_delta:    f32 = 0.0;

        #[allow(deprecated)]
        let status = self.event_loop.pump_events(Some(Duration::ZERO), |event, elwt| {
            match event {
                Event::WindowEvent { event: we, .. } => match we {
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
                    WindowEvent::CursorMoved { position, .. } => {
                        mouse_moved = Some((position.x, position.y));
                    }
                    WindowEvent::MouseInput { button, state, .. } => {
                        let pressed = state == ElementState::Pressed;
                        mouse_buttons.push((button, pressed));
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        scroll_delta += match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(d)   => d.y as f32 / 40.0,
                        };
                    }
                    _ => {}
                }
                _ => {}
            }
        });

        // ── Apply resize ───────────────────────────────────────────────────────
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
                unsafe { self.postfx.resize(&self.gl, w, h); }
            }
        }

        // ── Apply key events ───────────────────────────────────────────────────
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

        // ── Apply mouse events ─────────────────────────────────────────────────
        if let Some((x, y)) = mouse_moved {
            self.mouse_pos = Vec2::new(x as f32, y as f32);
            input.mouse_x = x as f32;
            input.mouse_y = y as f32;
            // Compute NDC: x/y ∈ [0, width/height] → [-1, 1]
            let w = self.width.max(1) as f32;
            let h = self.height.max(1) as f32;
            self.mouse_ndc = Vec2::new(
                (x as f32 / w) * 2.0 - 1.0,
                1.0 - (y as f32 / h) * 2.0,
            );
            input.mouse_ndc = self.mouse_ndc;
            input.mouse_delta = self.mouse_pos - self.mouse_pos_prev;
        }

        for (button, pressed) in mouse_buttons {
            match button {
                MouseButton::Left   => {
                    if pressed { input.mouse_left_just_pressed  = true; }
                    else       { input.mouse_left_just_released = true; }
                    input.mouse_left = pressed;
                }
                MouseButton::Right  => {
                    if pressed { input.mouse_right_just_pressed  = true; }
                    else       { input.mouse_right_just_released = true; }
                    input.mouse_right = pressed;
                }
                MouseButton::Middle => {
                    if pressed { input.mouse_middle_just_pressed = true; }
                    input.mouse_middle = pressed;
                }
                _ => {}
            }
        }

        input.scroll_delta = scroll_delta;

        // ── Exit check ─────────────────────────────────────────────────────────
        if should_exit || matches!(status, PumpStatus::Exit(_)) {
            self.running = false;
        }
        self.running
    }

    /// Collect all visible glyphs + particles from the scene, upload to the GPU,
    /// and execute the full multi-pass rendering pipeline.
    pub fn render(&mut self, scene: &Scene, camera: &ProofCamera) {
        // ── Frame timing ───────────────────────────────────────────────────────
        let now = Instant::now();
        let dt  = now.duration_since(self.frame_start).as_secs_f32().min(0.1);
        self.frame_start = now;
        self.scene_time  = scene.time;

        self.fps_counter.push(dt);
        self.stats.fps          = self.fps_counter.fps();
        self.stats.dt           = dt;
        self.stats.frame_number += 1;

        // ── Build camera matrices ──────────────────────────────────────────────
        let pos    = camera.position.position();
        let tgt    = camera.target.position();
        let fov    = camera.fov.position;
        let aspect = if self.height > 0 { self.width as f32 / self.height as f32 } else { 1.0 };
        let view      = Mat4::look_at_rh(pos, tgt, Vec3::Y);
        let proj      = Mat4::perspective_rh_gl(fov.to_radians(), aspect, camera.near, camera.far);
        let view_proj = proj * view;

        // ── Build glyph batch ──────────────────────────────────────────────────
        self.instances.clear();
        let mut glyph_count    = 0;
        let mut particle_count = 0;

        // Glyphs sorted by render layer (entity < particle < UI)
        for (_, glyph) in scene.glyphs.iter() {
            if !glyph.visible { continue; }
            let life_scale = if let Some(ref f) = glyph.life_function {
                f.evaluate(scene.time, 0.0)
            } else {
                1.0
            };
            let uv = self.atlas.uv_for(glyph.character);
            self.instances.push(GlyphInstance {
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
            glyph_count += 1;
        }

        for particle in scene.particles.iter() {
            let g = &particle.glyph;
            if !g.visible { continue; }
            let uv = self.atlas.uv_for(g.character);
            self.instances.push(GlyphInstance {
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
            particle_count += 1;
        }

        self.stats.glyph_count    = glyph_count;
        self.stats.particle_count = particle_count;
        self.stats.draw_calls     = 0;

        // ── Execute render passes ──────────────────────────────────────────────
        unsafe { self.execute_render_passes(view_proj); }
    }

    /// Swap back buffer to screen. Returns false on window close.
    pub fn swap(&mut self) -> bool {
        if let Err(e) = self.surface.swap_buffers(&self.context) {
            log::error!("swap_buffers failed: {e}");
            self.running = false;
        }
        self.running
    }

    // ── Private render pass execution ─────────────────────────────────────────

    unsafe fn execute_render_passes(&mut self, view_proj: Mat4) {
        let gl = &self.gl;

        // ── Pass 1: Render glyphs into PostFxPipeline's dual-attachment scene FBO ──
        //
        // Attachment 0 → scene_color_tex  (regular glyph colors)
        // Attachment 1 → scene_emission_tex (bloom-input: high-emission pixels only)
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.postfx.scene_fbo));
        gl.viewport(0, 0, self.width as i32, self.height as i32);
        gl.clear(glow::COLOR_BUFFER_BIT);

        if !self.instances.is_empty() {
            // Upload instance data
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instance_vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                cast_slice(self.instances.as_slice()),
                glow::DYNAMIC_DRAW,
            );

            // Draw all glyphs in one instanced call
            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(
                Some(&self.loc_view_proj),
                false,
                &view_proj.to_cols_array(),
            );
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas_tex));
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.instances.len() as i32);
            self.stats.draw_calls += 1;
        }

        // ── Passes 2-5: PostFxPipeline handles bloom + compositing ────────────
        //
        // PostFxPipeline reads render_config.bloom_enabled, bloom_intensity,
        // chromatic_aberration, film_grain, scanlines_enabled, etc.
        self.postfx.run(gl, &self.render_config, self.width, self.height, self.scene_time);
        self.stats.draw_calls += 4; // bloom H, bloom V, bloom H2, bloom V2, composite
    }
}

// ── GL helper functions ────────────────────────────────────────────────────────

/// Compile a vertex + fragment shader pair into a linked GL program.
unsafe fn compile_program(gl: &glow::Context, vert_src: &str, frag_src: &str) -> glow::Program {
    let vs = gl.create_shader(glow::VERTEX_SHADER).expect("create vertex shader");
    gl.shader_source(vs, vert_src);
    gl.compile_shader(vs);
    if !gl.get_shader_compile_status(vs) {
        let log = gl.get_shader_info_log(vs);
        panic!("Vertex shader compile error:\n{log}");
    }

    let fs = gl.create_shader(glow::FRAGMENT_SHADER).expect("create fragment shader");
    gl.shader_source(fs, frag_src);
    gl.compile_shader(fs);
    if !gl.get_shader_compile_status(fs) {
        let log = gl.get_shader_info_log(fs);
        panic!("Fragment shader compile error:\n{log}");
    }

    let prog = gl.create_program().expect("create shader program");
    gl.attach_shader(prog, vs);
    gl.attach_shader(prog, fs);
    gl.link_program(prog);
    if !gl.get_program_link_status(prog) {
        let log = gl.get_program_info_log(prog);
        panic!("Shader link error:\n{log}");
    }

    gl.detach_shader(prog, vs);
    gl.detach_shader(prog, fs);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    prog
}

/// Create VAO with per-vertex quad data (locations 0–1) and per-instance data (locations 2–10).
unsafe fn setup_vao(gl: &glow::Context) -> (glow::VertexArray, glow::Buffer, glow::Buffer) {
    let vao = gl.create_vertex_array().expect("create vao");
    gl.bind_vertex_array(Some(vao));

    // ── Quad geometry VBO ─────────────────────────────────────────────────────
    let quad_vbo = gl.create_buffer().expect("create quad_vbo");
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad_vbo));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, cast_slice(&QUAD_VERTS), glow::STATIC_DRAW);
    // location 0: vec2 v_pos  (offset 0, stride 16)
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
    gl.enable_vertex_attrib_array(0);
    // location 1: vec2 v_uv   (offset 8, stride 16)
    gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
    gl.enable_vertex_attrib_array(1);

    // ── Instance VBO (per-glyph data) ─────────────────────────────────────────
    let instance_vbo = gl.create_buffer().expect("create instance_vbo");
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_vbo));

    let stride = std::mem::size_of::<GlyphInstance>() as i32;

    // Macro: set up an instanced float attribute.
    macro_rules! inst_attr {
        ($loc:expr, $count:expr, $off:expr) => {{
            gl.vertex_attrib_pointer_f32($loc, $count, glow::FLOAT, false, stride, $off);
            gl.enable_vertex_attrib_array($loc);
            gl.vertex_attrib_divisor($loc, 1); // advance once per instance
        }};
    }

    inst_attr!(2,  3,  0);  // i_position   vec3   @ byte 0
    inst_attr!(3,  2, 12);  // i_scale      vec2   @ byte 12
    inst_attr!(4,  1, 20);  // i_rotation   float  @ byte 20
    inst_attr!(5,  4, 24);  // i_color      vec4   @ byte 24
    inst_attr!(6,  1, 40);  // i_emission   float  @ byte 40
    inst_attr!(7,  3, 44);  // i_glow_color vec3   @ byte 44
    inst_attr!(8,  1, 56);  // i_glow_radius float @ byte 56
    inst_attr!(9,  2, 60);  // i_uv_offset  vec2   @ byte 60
    inst_attr!(10, 2, 68);  // i_uv_size    vec2   @ byte 68
    // bytes 76-83: _pad (2× f32, needed to keep GlyphInstance 84-byte aligned)

    (vao, quad_vbo, instance_vbo)
}

/// Upload a FontAtlas as an R8 GL texture and return the handle.
unsafe fn upload_atlas(gl: &glow::Context, atlas: &FontAtlas) -> glow::Texture {
    let tex = gl.create_texture().expect("create atlas texture");
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
    gl.tex_image_2d(
        glow::TEXTURE_2D, 0, glow::R8 as i32,
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

// ── KeyCode → engine Key mapping ──────────────────────────────────────────────

/// Map a winit `KeyCode` to the engine's `Key` enum. Returns `None` for unknown keys.
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
        KeyCode::Escape     => Key::Escape,
        KeyCode::Space      => Key::Space,
        KeyCode::Backspace  => Key::Backspace,
        KeyCode::Tab        => Key::Tab,
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
        KeyCode::PageUp       => Key::PageUp,
        KeyCode::PageDown     => Key::PageDown,
        KeyCode::Home         => Key::Home,
        KeyCode::End          => Key::End,
        KeyCode::Insert       => Key::Insert,
        KeyCode::Delete       => Key::Delete,
        _ => return None,
    })
}
