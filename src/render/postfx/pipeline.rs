//! GPU post-processing pipeline.
//!
//! ```text
//! scene FBO (RGBA16F ×2: colour, emission) at render scale
//!   ├─ emission ─► bloom pyramid: threshold, blur down, tent-filter up
//!   └─ colour ───┐
//!                ├─ composite: refraction, lens, exposure, indirect light,
//!                │             bloom, halation, shafts, flare, flash,
//!                │             tonemap, grade, vignette, grain, dither
//!                │      ├─ (fxaa off) ─► screen
//!                │      └─ (fxaa on)  ─► LDR FBO ─► FXAA ─► screen
//! ```
//!
//! The scene targets are half-float. Before this they were eight bits per
//! channel, which meant a scene lit by a few hundred thousand overlapping
//! emissive particles was clipped to white before the composite ever saw it,
//! and the exposure, tonemap and halation were working on a picture that had
//! already lost its highlights. Now the composite is the first place the
//! range is brought down.

use std::cell::Cell;
use glow::HasContext;
use crate::render::shaders::{FULLSCREEN_VERT, BLOOM_FRAG, COMPOSITE_FRAG, UPSAMPLE_FRAG, FXAA_FRAG, LIGHT_FRAG, PERSIST_FRAG};
use crate::render::screen_fx::{ScreenFx, MAX_SHOCKWAVES};
use crate::config::RenderConfig;

pub struct PostFxPipeline {
    // Scene FBO — dual HDR colour attachments, at render scale
    pub scene_fbo:          glow::Framebuffer,
    pub scene_color_tex:    glow::Texture,    // attachment 0: rendered scene
    pub scene_emission_tex: glow::Texture,    // attachment 1: emission → bloom input
    pub scene_occluder_tex: glow::Texture,    // attachment 2: matter coverage → shadows
    /// The scene targets' size. `render_scale` times the window, so a 4K
    /// display can render at 1440p and be upsampled by the composite.
    scene_w: u32,
    scene_h: u32,

    // Bloom scratch (half-res)
    /// Horizontal-pass scratch, one per pyramid level. A single shared
    /// scratch sized for level 0 was wrong: each smaller level wrote its
    /// bottom-left corner and the vertical pass then read the whole
    /// texture, so three quarters of every level was the clear colour.
    bloom_fbo: Vec<glow::Framebuffer>,
    bloom_tex: Vec<glow::Texture>,

    /// A pyramid of successively halved targets, and their sizes.
    ///
    /// Real bloom is scale-invariant: a bright point puts light into a
    /// tight core, a wider halo, and a very broad veil across the frame. A
    /// pyramid gets all three for about a third of one full-resolution pass.
    mip_fbo: Vec<glow::Framebuffer>,
    mip_tex: Vec<glow::Texture>,
    mip_size: Vec<(i32, i32)>,
    /// Second chain, for the upward pass that combines them.
    up_fbo: Vec<glow::Framebuffer>,
    up_tex: Vec<glow::Texture>,
    upsample_prog: glow::Program,

    /// The light map, at half the scene size.
    light_fbo: glow::Framebuffer,
    light_tex: glow::Texture,
    light_size: (i32, i32),
    light_prog: glow::Program,

    /// Motion trails: last frame's scene, two of them so one can be read
    /// while the other is written.
    history_fbo: [glow::Framebuffer; 2],
    history_tex: [glow::Texture; 2],
    history_idx: usize,
    history_valid: bool,
    persist_prog: glow::Program,

    /// Where the composite lands when anti-aliasing runs after it.
    ldr_fbo: glow::Framebuffer,
    ldr_tex: glow::Texture,
    fxaa_prog: glow::Program,

    bloom_prog:     glow::Program,
    composite_prog: glow::Program,

    /// The brightest texel of the smallest bloom level last frame, as
    /// `(u, v, strength, unused)` plus its colour: the automatic light-shaft
    /// source. Read back from the GPU, so it is a frame late.
    auto_shaft: Cell<([f32; 4], [f32; 3])>,
    /// Scratch for that read-back, sized to the smallest level.
    readback: std::cell::RefCell<Vec<f32>>,

    // Empty VAO for fullscreen draws (GL 3.3 Core requires a VAO bound)
    fullscreen_vao: glow::VertexArray,
}

/// Scene target size for a window size and render scale, never below one
/// pixel.
fn scaled(width: u32, height: u32, scale: f32) -> (u32, u32) {
    let s = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    (
        ((width as f32 * s).round() as u32).max(1),
        ((height as f32 * s).round() as u32).max(1),
    )
}

impl PostFxPipeline {
    pub unsafe fn new(gl: &glow::Context, width: u32, height: u32, render_scale: f32) -> Self {
        let bloom_prog     = compile_postfx_program(gl, FULLSCREEN_VERT, BLOOM_FRAG);
        let composite_prog = compile_postfx_program(gl, FULLSCREEN_VERT, COMPOSITE_FRAG);
        let upsample_prog  = compile_postfx_program(gl, FULLSCREEN_VERT, UPSAMPLE_FRAG);
        let fxaa_prog      = compile_postfx_program(gl, FULLSCREEN_VERT, FXAA_FRAG);
        let light_prog     = compile_postfx_program(gl, FULLSCREEN_VERT, LIGHT_FRAG);
        let persist_prog   = compile_postfx_program(gl, FULLSCREEN_VERT, PERSIST_FRAG);
        let fullscreen_vao = gl.create_vertex_array().expect("postfx fullscreen_vao");

        // Pre-bind sampler units (never changes)
        gl.use_program(Some(bloom_prog));
        set_u_i32(gl, bloom_prog, "u_texture", 0);

        gl.use_program(Some(composite_prog));
        set_u_i32(gl, composite_prog, "u_scene", 0);
        set_u_i32(gl, composite_prog, "u_bloom", 1);
        set_u_i32(gl, composite_prog, "u_light", 2);
        set_u_i32(gl, composite_prog, "u_emission", 3);

        gl.use_program(Some(light_prog));
        set_u_i32(gl, light_prog, "u_occluder", 0);

        gl.use_program(Some(persist_prog));
        set_u_i32(gl, persist_prog, "u_scene", 0);
        set_u_i32(gl, persist_prog, "u_history", 1);

        gl.use_program(Some(upsample_prog));
        set_u_i32(gl, upsample_prog, "u_lower", 0);
        set_u_i32(gl, upsample_prog, "u_higher", 1);

        gl.use_program(Some(fxaa_prog));
        set_u_i32(gl, fxaa_prog, "u_image", 0);

        let (scene_w, scene_h) = scaled(width, height, render_scale);
        let (scene_fbo, scene_color_tex, scene_emission_tex, scene_occluder_tex) =
            create_scene_fbo(gl, scene_w, scene_h);
        let (bloom_fbo, bloom_tex, _) = create_pyramid(gl, scene_w, scene_h);
        let (mip_fbo, mip_tex, mip_size) = create_pyramid(gl, scene_w, scene_h);
        let (up_fbo, up_tex, _) = create_pyramid(gl, scene_w, scene_h);
        let (ldr_fbo, ldr_tex) = create_ldr_fbo(gl, width, height);
        let (lw, lh) = ((scene_w / 2).max(1), (scene_h / 2).max(1));
        let (light_fbo, light_tex) = create_hdr_fbo(gl, lw, lh, "light");
        let (h0, t0) = create_hdr_fbo(gl, scene_w, scene_h, "history 0");
        let (h1, t1) = create_hdr_fbo(gl, scene_w, scene_h, "history 1");

        Self {
            scene_fbo,
            scene_color_tex,
            scene_emission_tex,
            scene_occluder_tex,
            scene_w,
            scene_h,
            light_fbo,
            light_tex,
            light_size: (lw as i32, lh as i32),
            light_prog,
            history_fbo: [h0, h1],
            history_tex: [t0, t1],
            history_idx: 0,
            history_valid: false,
            persist_prog,
            bloom_fbo,
            bloom_tex,
            mip_fbo,
            mip_tex,
            mip_size,
            up_fbo,
            up_tex,
            upsample_prog,
            ldr_fbo,
            ldr_tex,
            fxaa_prog,
            bloom_prog,
            composite_prog,
            auto_shaft: Cell::new(([0.5, 0.5, 0.0, 0.0], [1.0, 1.0, 1.0])),
            readback: std::cell::RefCell::new(Vec::new()),
            fullscreen_vao,
        }
    }

    /// Find the brightest texel of the smallest bloom level. The level is
    /// about twenty texels across, so the read-back is a few kilobytes.
    unsafe fn read_auto_shaft(&self, gl: &glow::Context) {
        let Some(top) = self.mip_fbo.len().checked_sub(1) else { return };
        let (w, h) = self.mip_size[top];
        let n = (w * h * 4) as usize;
        let mut buf = self.readback.borrow_mut();
        if buf.len() != n {
            buf.resize(n, 0.0);
        }
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.mip_fbo[top]));
        gl.read_buffer(glow::COLOR_ATTACHMENT0);
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
        gl.read_pixels(
            0, 0, w, h,
            glow::RGBA, glow::FLOAT,
            glow::PixelPackData::Slice(Some(bytemuck::cast_slice_mut(&mut buf[..]))),
        );
        let mut best = 0.0f32;
        let mut best_i = 0usize;
        for i in 0..(w * h) as usize {
            let r = buf[i * 4];
            let g = buf[i * 4 + 1];
            let b = buf[i * 4 + 2];
            let lum = r * 0.2126 + g * 0.7152 + b * 0.0722;
            if lum > best {
                best = lum;
                best_i = i;
            }
        }
        if best <= 1e-4 {
            self.auto_shaft.set(([0.5, 0.5, 0.0, 0.0], [1.0, 1.0, 1.0]));
            return;
        }
        let x = (best_i as i32 % w) as f32 + 0.5;
        let y = (best_i as i32 / w) as f32 + 0.5;
        let r = buf[best_i * 4];
        let g = buf[best_i * 4 + 1];
        let b = buf[best_i * 4 + 2];
        let m = r.max(g).max(b).max(1e-4);
        // The smallest level is the whole frame's light averaged over a few
        // texels, so even a bright brazier reads as a fraction there.
        let strength = (best * 6.0).clamp(0.0, 1.0);
        self.auto_shaft.set(([x / w as f32, y / h as f32, strength, 0.0], [r / m, g / m, b / m]));
    }

    /// The size of the scene targets, which is what the scene passes must
    /// set their viewport to.
    pub fn scene_size(&self) -> (u32, u32) {
        (self.scene_w, self.scene_h)
    }

    /// Recreate every target for a new window size or render scale.
    pub unsafe fn resize(&mut self, gl: &glow::Context, width: u32, height: u32, render_scale: f32) {
        gl.delete_framebuffer(self.scene_fbo);
        gl.delete_texture(self.scene_color_tex);
        gl.delete_texture(self.scene_emission_tex);
        gl.delete_texture(self.scene_occluder_tex);
        gl.delete_framebuffer(self.light_fbo);
        gl.delete_texture(self.light_tex);
        for i in 0..2 {
            gl.delete_framebuffer(self.history_fbo[i]);
            gl.delete_texture(self.history_tex[i]);
        }
        for f in self.bloom_fbo.drain(..) {
            gl.delete_framebuffer(f);
        }
        for t in self.bloom_tex.drain(..) {
            gl.delete_texture(t);
        }
        for f in self.mip_fbo.drain(..) {
            gl.delete_framebuffer(f);
        }
        for t in self.mip_tex.drain(..) {
            gl.delete_texture(t);
        }
        for f in self.up_fbo.drain(..) {
            gl.delete_framebuffer(f);
        }
        for t in self.up_tex.drain(..) {
            gl.delete_texture(t);
        }
        gl.delete_framebuffer(self.ldr_fbo);
        gl.delete_texture(self.ldr_tex);

        let (scene_w, scene_h) = scaled(width, height, render_scale);
        self.scene_w = scene_w;
        self.scene_h = scene_h;

        let (mip_fbo, mip_tex, mip_size) = create_pyramid(gl, scene_w, scene_h);
        let (up_fbo, up_tex, _) = create_pyramid(gl, scene_w, scene_h);
        self.mip_fbo = mip_fbo;
        self.mip_tex = mip_tex;
        self.mip_size = mip_size;
        self.up_fbo = up_fbo;
        self.up_tex = up_tex;

        let (scene_fbo, scene_color_tex, scene_emission_tex, scene_occluder_tex) =
            create_scene_fbo(gl, scene_w, scene_h);
        let (bloom_fbo, bloom_tex, _) = create_pyramid(gl, scene_w, scene_h);
        let (ldr_fbo, ldr_tex) = create_ldr_fbo(gl, width, height);
        let (lw, lh) = ((scene_w / 2).max(1), (scene_h / 2).max(1));
        let (light_fbo, light_tex) = create_hdr_fbo(gl, lw, lh, "light");
        let (h0, t0) = create_hdr_fbo(gl, scene_w, scene_h, "history 0");
        let (h1, t1) = create_hdr_fbo(gl, scene_w, scene_h, "history 1");
        self.light_fbo = light_fbo;
        self.light_tex = light_tex;
        self.light_size = (lw as i32, lh as i32);
        self.history_fbo = [h0, h1];
        self.history_tex = [t0, t1];
        self.history_valid = false;

        self.scene_fbo          = scene_fbo;
        self.scene_color_tex    = scene_color_tex;
        self.scene_emission_tex = scene_emission_tex;
        self.scene_occluder_tex = scene_occluder_tex;
        self.bloom_fbo          = bloom_fbo;
        self.bloom_tex          = bloom_tex;
        self.ldr_fbo            = ldr_fbo;
        self.ldr_tex            = ldr_tex;
    }

    /// Run bloom, composite, and anti-aliasing, ending on the default
    /// (screen) framebuffer. Returns how many draws it made.
    pub unsafe fn run(
        &mut self,
        gl:     &glow::Context,
        config: &RenderConfig,
        fx:     &ScreenFx,
        full_w: u32,
        full_h: u32,
        time:   f32,
    ) -> u32 {
        let mut draws = 0u32;
        gl.bind_vertex_array(Some(self.fullscreen_vao));
        gl.disable(glow::BLEND);

        // ── Persistence ──────────────────────────────────────────────────────
        //
        // The scene the composite reads is either the raw one or the one
        // with last frame's trails folded in.
        let mut scene_tex = self.scene_color_tex;
        if config.persistence > 0.0 {
            let next = self.history_idx ^ 1;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.history_fbo[next]));
            gl.viewport(0, 0, self.scene_w as i32, self.scene_h as i32);
            gl.use_program(Some(self.persist_prog));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.scene_color_tex));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.history_tex[self.history_idx]));
            let keep = if self.history_valid { config.persistence } else { 0.0 };
            set_u_f32(gl, self.persist_prog, "u_persistence", keep);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.active_texture(glow::TEXTURE0);
            self.history_idx = next;
            self.history_valid = true;
            scene_tex = self.history_tex[next];
            draws += 1;
        } else {
            self.history_valid = false;
        }

        // ── The light map ────────────────────────────────────────────────────
        let lighting = fx.lighting_active();
        if lighting {
            let (lw, lh) = self.light_size;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.light_fbo));
            gl.viewport(0, 0, lw, lh);
            gl.use_program(Some(self.light_prog));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.scene_occluder_tex));
            let p = self.light_prog;
            set_u_vec2(gl, p, "u_screen", [full_w as f32, full_h as f32]);
            set_u_vec3(gl, p, "u_ambient", fx.ambient.to_array());
            set_u_f32(gl, p, "u_shadow_density", fx.shadow_density.max(0.0));
            let (pos, col, n) = fx.pack_lights(full_w as f32, full_h as f32);
            set_u_i32(gl, p, "u_count", n as i32);
            if n > 0 {
                if let Some(loc) = gl.get_uniform_location(p, "u_light_pos[0]") {
                    gl.uniform_4_f32_slice(Some(&loc), &pos);
                }
                if let Some(loc) = gl.get_uniform_location(p, "u_light_color[0]") {
                    gl.uniform_4_f32_slice(Some(&loc), &col);
                }
            }
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            draws += 1;
        }

        // ── Bloom ────────────────────────────────────────────────────────────
        //
        // Down the pyramid blurring as it goes, then back up adding each level
        // into the one above it. The result contains a tight core, a wide
        // halo and a broad veil at once.
        if config.bloom_enabled && !self.mip_fbo.is_empty() {
            let radius = config.bloom_radius.max(0.5);
            // Nothing in the bloom chain may inherit the scene ground colour.
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.use_program(Some(self.bloom_prog));
            gl.active_texture(glow::TEXTURE0);
            set_u_f32(gl, self.bloom_prog, "u_threshold", config.bloom_threshold);
            set_u_f32(gl, self.bloom_prog, "u_knee", config.bloom_knee.max(1e-3));
            set_u_f32(gl, self.bloom_prog, "u_stretch", 1.0);

            // ── Down ─────────────────────────────────────────────────────────
            for level in 0..self.mip_fbo.len() {
                let (w, h) = self.mip_size[level];
                let source = if level == 0 {
                    self.scene_emission_tex
                } else {
                    self.mip_tex[level - 1]
                };

                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.bloom_fbo[level]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_texture(glow::TEXTURE_2D, Some(source));
                set_u_bool(gl, self.bloom_prog, "u_prefilter", level == 0);
                set_u_bool(gl, self.bloom_prog, "u_horizontal", true);
                set_u_f32(gl,  self.bloom_prog, "u_radius", radius);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);

                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.mip_fbo[level]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.bloom_tex[level]));
                set_u_bool(gl, self.bloom_prog, "u_prefilter", false);
                set_u_bool(gl, self.bloom_prog, "u_horizontal", false);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
                draws += 2;
            }

            // ── The streak ───────────────────────────────────────────────────
            if config.anamorphic > 0.0 && self.mip_fbo.len() > 2 {
                let level = self.mip_fbo.len() / 2;
                let (w, h) = self.mip_size[level];
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.bloom_fbo[level]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.mip_tex[level]));
                set_u_bool(gl, self.bloom_prog, "u_horizontal", true);
                set_u_f32(gl,  self.bloom_prog, "u_stretch", 9.0 * config.anamorphic);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);

                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.mip_fbo[level]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.bloom_tex[level]));
                set_u_bool(gl, self.bloom_prog, "u_horizontal", true);
                set_u_f32(gl,  self.bloom_prog, "u_stretch", 16.0 * config.anamorphic);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
                set_u_f32(gl,  self.bloom_prog, "u_stretch", 1.0);
                draws += 2;
            }

            // ── Up ───────────────────────────────────────────────────────────
            gl.use_program(Some(self.upsample_prog));
            let top = self.mip_fbo.len() - 1;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.up_fbo[top]));
            let (w, h) = self.mip_size[top];
            gl.viewport(0, 0, w, h);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.mip_tex[top]));
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.mip_tex[top]));
            set_u_f32(gl, self.upsample_prog, "u_radius", 1.0);
            set_u_f32(gl, self.upsample_prog, "u_strength", 0.0);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            draws += 1;

            if fx.auto_shafts && config.light_shafts > 0.0 {
                self.read_auto_shaft(gl);
            }

            for level in (0..top).rev() {
                let (w, h) = self.mip_size[level];
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.up_fbo[level]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.up_tex[level + 1]));
                gl.active_texture(glow::TEXTURE1);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.mip_tex[level]));
                set_u_f32(gl, self.upsample_prog, "u_radius", radius);
                let up = (level + 1) as f32 / top.max(1) as f32;
                set_u_f32(gl, self.upsample_prog, "u_strength", 0.55 + up * 0.30);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
                draws += 1;
            }
            gl.active_texture(glow::TEXTURE0);
        }

        // ── Composite ────────────────────────────────────────────────────────
        let aa = config.fxaa;
        if aa {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.ldr_fbo));
        } else {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
        gl.viewport(0, 0, full_w as i32, full_h as i32);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.use_program(Some(self.composite_prog));

        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(scene_tex));
        gl.active_texture(glow::TEXTURE2);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.light_tex));
        gl.active_texture(glow::TEXTURE3);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.scene_emission_tex));

        // Unit 1: the bloom result. With bloom off the shader still samples
        // it for indirect light, shafts and flare, so hand it the emission
        // buffer rather than the scene: still only the lit things.
        gl.active_texture(glow::TEXTURE1);
        if config.bloom_enabled && !self.up_tex.is_empty() {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.up_tex[0]));
        } else {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.scene_emission_tex));
        }

        let bloom_intensity = if config.bloom_enabled { config.bloom_intensity } else { 0.0 };
        let p = self.composite_prog;
        set_u_f32(gl,  p, "u_bloom_intensity",    bloom_intensity);
        set_u_f32(gl,  p, "u_exposure",           config.exposure);
        set_u_f32(gl,  p, "u_tonemap",            config.tonemap);
        set_u_f32(gl,  p, "u_halation",           config.halation);
        set_u_vec3(gl, p, "u_tint",               config.tint);
        set_u_vec3(gl, p, "u_lift",               config.lift);
        set_u_vec3(gl, p, "u_gain",               config.gain);
        set_u_f32(gl,  p, "u_saturation",         config.saturation);
        set_u_f32(gl,  p, "u_contrast",           config.contrast);
        set_u_f32(gl,  p, "u_brightness",         config.brightness);
        set_u_f32(gl,  p, "u_vignette",           config.vignette);
        set_u_f32(gl,  p, "u_vignette_softness",  config.vignette_softness);
        set_u_f32(gl,  p, "u_sharpen",            config.sharpen);
        set_u_f32(gl,  p, "u_dither",             config.dither);
        set_u_f32(gl,  p, "u_barrel",             config.barrel);
        set_u_f32(gl,  p, "u_grain_intensity",    config.film_grain);
        set_u_f32(gl,  p, "u_grain_seed",         time);
        set_u_f32(gl,  p, "u_chromatic",          config.chromatic_aberration);
        set_u_f32(gl,  p, "u_scanline_intensity",
            if config.scanlines_enabled { config.scanline_intensity } else { 0.0 });
        set_u_bool(gl, p, "u_scanlines_enabled",  config.scanlines_enabled);

        // Light and moments.
        let (fw, fh) = (full_w as f32, full_h as f32);
        set_u_vec2(gl, p, "u_screen",             [fw, fh]);
        set_u_f32(gl,  p, "u_indirect",           config.indirect_light);
        set_u_f32(gl,  p, "u_light_shafts",       config.light_shafts);
        // An explicit source wins; otherwise the brightest thing on screen.
        let explicit = fx.pack_shaft(fw, fh);
        if explicit[2] > 0.0 || !fx.auto_shafts || !config.bloom_enabled {
            set_u_vec3(gl, p, "u_shaft",          explicit);
            set_u_vec3(gl, p, "u_shaft_tint",     fx.shaft_tint.to_array());
        } else {
            let (src, tint) = self.auto_shaft.get();
            set_u_vec3(gl, p, "u_shaft",          [src[0], src[1], src[2]]);
            set_u_vec3(gl, p, "u_shaft_tint",     tint);
        }
        let (refl, blur) = fx.pack_reflection(fw, fh);
        if let Some(loc) = gl.get_uniform_location(p, "u_reflection") {
            gl.uniform_4_f32(Some(&loc), refl[0], refl[1], refl[2], refl[3]);
        }
        set_u_f32(gl,  p, "u_reflection_blur",    blur);
        set_u_f32(gl,  p, "u_haze",               fx.haze.max(0.0));
        set_u_f32(gl,  p, "u_time",               fx.time);
        set_u_bool(gl, p, "u_lighting",           lighting);
        set_u_f32(gl,  p, "u_lens_flare",         config.lens_flare);
        set_u_vec3(gl, p, "u_flash",              fx.flash.to_array());
        let (shock, strength, count) = fx.pack_shockwaves(fw, fh);
        set_u_i32(gl, p, "u_shock_count", count as i32);
        if count > 0 {
            let flat: Vec<f32> = shock.iter().flat_map(|v| v.iter().copied()).collect();
            if let Some(loc) = gl.get_uniform_location(p, "u_shock[0]") {
                gl.uniform_4_f32_slice(Some(&loc), &flat);
            }
            if let Some(loc) = gl.get_uniform_location(p, "u_shock_strength[0]") {
                gl.uniform_1_f32_slice(Some(&loc), &strength[..MAX_SHOCKWAVES]);
            }
        }

        gl.draw_arrays(glow::TRIANGLES, 0, 3);
        draws += 1;

        // ── Anti-aliasing ────────────────────────────────────────────────────
        if aa {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.viewport(0, 0, full_w as i32, full_h as i32);
            gl.use_program(Some(self.fxaa_prog));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.ldr_tex));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            draws += 1;
        }

        gl.active_texture(glow::TEXTURE0);
        gl.enable(glow::BLEND);
        gl.bind_vertex_array(None);
        draws
    }
}

// ── FBO helpers ────────────────────────────────────────────────────────────────

/// A colour-renderable texture. `hdr` makes it half-float so it can hold
/// values past 1.0; otherwise eight bits per channel.
unsafe fn make_tex(gl: &glow::Context, w: u32, h: u32, hdr: bool) -> glow::Texture {
    let tex = gl.create_texture().expect("postfx texture");
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    if hdr {
        gl.tex_image_2d(
            glow::TEXTURE_2D, 0, glow::RGBA16F as i32,
            w as i32, h as i32, 0,
            glow::RGBA, glow::HALF_FLOAT, glow::PixelUnpackData::Slice(None),
        );
    } else {
        gl.tex_image_2d(
            glow::TEXTURE_2D, 0, glow::RGBA8 as i32,
            w as i32, h as i32, 0,
            glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None),
        );
    }
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S,     glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T,     glow::CLAMP_TO_EDGE as i32);
    tex
}

unsafe fn check_complete(gl: &glow::Context, what: &str) {
    let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
    if status != glow::FRAMEBUFFER_COMPLETE {
        log::error!("{what} FBO incomplete: 0x{status:X}");
    }
}

unsafe fn create_scene_fbo(
    gl: &glow::Context, w: u32, h: u32,
) -> (glow::Framebuffer, glow::Texture, glow::Texture, glow::Texture) {
    let color_tex    = make_tex(gl, w, h, true);
    let emission_tex = make_tex(gl, w, h, true);
    let occluder_tex = make_tex(gl, w, h, true);

    let fbo = gl.create_framebuffer().expect("scene fbo");
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(color_tex), 0,
    );
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT1, glow::TEXTURE_2D, Some(emission_tex), 0,
    );
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT2, glow::TEXTURE_2D, Some(occluder_tex), 0,
    );
    gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1, glow::COLOR_ATTACHMENT2]);
    check_complete(gl, "scene");
    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    (fbo, color_tex, emission_tex, occluder_tex)
}

/// One half-float colour target of any size.
unsafe fn create_hdr_fbo(gl: &glow::Context, w: u32, h: u32, what: &str) -> (glow::Framebuffer, glow::Texture) {
    let tex = make_tex(gl, w.max(1), h.max(1), true);
    let fbo = gl.create_framebuffer().expect("hdr fbo");
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(tex), 0,
    );
    gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
    check_complete(gl, what);
    // Trails and light start from nothing, not from uninitialised memory.
    gl.clear_color(0.0, 0.0, 0.0, 0.0);
    gl.clear(glow::COLOR_BUFFER_BIT);
    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    (fbo, tex)
}

unsafe fn create_ldr_fbo(gl: &glow::Context, w: u32, h: u32) -> (glow::Framebuffer, glow::Texture) {
    let tex = make_tex(gl, w.max(1), h.max(1), false);
    let fbo = gl.create_framebuffer().expect("ldr fbo");
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(tex), 0,
    );
    gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
    check_complete(gl, "ldr");
    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    (fbo, tex)
}

/// Build a chain of halving render targets, largest first.
///
/// Stops at eight pixels: below that a level is a colour rather than an image,
/// and blurring it only spreads the frame's average brightness over the frame.
unsafe fn create_pyramid(
    gl: &glow::Context,
    width: u32,
    height: u32,
) -> (Vec<glow::Framebuffer>, Vec<glow::Texture>, Vec<(i32, i32)>) {
    let mut fbos = Vec::new();
    let mut texs = Vec::new();
    let mut sizes = Vec::new();
    let (mut w, mut h) = ((width / 2).max(1), (height / 2).max(1));
    for _ in 0..6 {
        if w < 8 || h < 8 {
            break;
        }
        let tex = make_tex(gl, w, h, true);
        let fbo = gl.create_framebuffer().expect("bloom mip fbo");
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(tex),
            0,
        );
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
        fbos.push(fbo);
        texs.push(tex);
        sizes.push((w as i32, h as i32));
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    (fbos, texs, sizes)
}

// ── Shader compilation ─────────────────────────────────────────────────────────

unsafe fn compile_postfx_program(
    gl: &glow::Context, vert_src: &str, frag_src: &str,
) -> glow::Program {
    let vs = gl.create_shader(glow::VERTEX_SHADER).expect("postfx vs");
    gl.shader_source(vs, vert_src);
    gl.compile_shader(vs);
    if !gl.get_shader_compile_status(vs) {
        panic!("PostFx vertex shader error:\n{}", gl.get_shader_info_log(vs));
    }

    let fs = gl.create_shader(glow::FRAGMENT_SHADER).expect("postfx fs");
    gl.shader_source(fs, frag_src);
    gl.compile_shader(fs);
    if !gl.get_shader_compile_status(fs) {
        panic!("PostFx fragment shader error:\n{}", gl.get_shader_info_log(fs));
    }

    let prog = gl.create_program().expect("postfx program");
    gl.attach_shader(prog, vs);
    gl.attach_shader(prog, fs);
    gl.link_program(prog);
    if !gl.get_program_link_status(prog) {
        panic!("PostFx link error:\n{}", gl.get_program_info_log(prog));
    }
    gl.detach_shader(prog, vs);
    gl.detach_shader(prog, fs);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    prog
}

// ── Uniform helpers ─────────────────────────────────────────────────────────────

unsafe fn set_u_i32(gl: &glow::Context, prog: glow::Program, name: &str, v: i32) {
    if let Some(loc) = gl.get_uniform_location(prog, name) {
        gl.uniform_1_i32(Some(&loc), v);
    }
}

unsafe fn set_u_bool(gl: &glow::Context, prog: glow::Program, name: &str, v: bool) {
    set_u_i32(gl, prog, name, if v { 1 } else { 0 });
}

unsafe fn set_u_f32(gl: &glow::Context, prog: glow::Program, name: &str, v: f32) {
    if let Some(loc) = gl.get_uniform_location(prog, name) {
        gl.uniform_1_f32(Some(&loc), v);
    }
}

unsafe fn set_u_vec2(gl: &glow::Context, prog: glow::Program, name: &str, v: [f32; 2]) {
    if let Some(loc) = gl.get_uniform_location(prog, name) {
        gl.uniform_2_f32(Some(&loc), v[0], v[1]);
    }
}

unsafe fn set_u_vec3(gl: &glow::Context, prog: glow::Program, name: &str, v: [f32; 3]) {
    if let Some(loc) = gl.get_uniform_location(prog, name) {
        gl.uniform_3_f32(Some(&loc), v[0], v[1], v[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::scaled;

    #[test]
    fn scaled_targets_never_collapse() {
        assert_eq!(scaled(1280, 800, 1.0), (1280, 800));
        assert_eq!(scaled(1280, 800, 0.5), (640, 400));
        assert_eq!(scaled(3, 3, 0.1), (1, 1));
        assert_eq!(scaled(100, 100, 0.0), (100, 100));
        assert_eq!(scaled(100, 100, f32::NAN), (100, 100));
    }
}
