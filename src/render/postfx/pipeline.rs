//! GPU post-processing pipeline: bloom, chromatic aberration, film grain, vignette, scanlines.
//!
//! Pipeline:
//!   1. Glyphs render into scene FBO (2 color attachments: color + emission)
//!   2. Emission texture → horizontal Gaussian blur → bloom_tex[0]
//!   3. bloom_tex[0]    → vertical   Gaussian blur → bloom_tex[1]
//!   4. (extra pass for softer bloom)
//!   5. Composite: scene_color + bloom + chromatic aberration + grain + vignette → screen

use glow::HasContext;
use crate::render::shaders::{FULLSCREEN_VERT, BLOOM_FRAG, COMPOSITE_FRAG, UPSAMPLE_FRAG};
use crate::config::RenderConfig;

pub struct PostFxPipeline {
    // Scene FBO — dual color attachments
    pub scene_fbo:          glow::Framebuffer,
    pub scene_color_tex:    glow::Texture,    // attachment 0: rendered scene
    pub scene_emission_tex: glow::Texture,    // attachment 1: emission → bloom input

    // Bloom ping-pong (half-res)
    bloom_fbo: [glow::Framebuffer; 2],
    bloom_tex: [glow::Texture; 2],

    /// A pyramid of successively halved targets, and their sizes.
    ///
    /// Two blur widths at half resolution is a small glow and a slightly
    /// larger one. Real bloom is scale-invariant: a bright point puts light
    /// into a tight core, a wider halo, and a very broad veil across the
    /// frame, and no amount of widening one Gaussian produces that — the
    /// kernel would need hundreds of taps and would still band.
    ///
    /// A pyramid gets it for almost nothing. Each level is half the last, so
    /// the whole chain costs about a third of one full-resolution pass, and
    /// blurring a sixteenth-size image by three pixels *is* a forty-eight
    /// pixel blur of the original.
    mip_fbo: Vec<glow::Framebuffer>,
    mip_tex: Vec<glow::Texture>,
    mip_size: Vec<(i32, i32)>,
    /// Second chain, for the upward pass that combines them.
    up_fbo: Vec<glow::Framebuffer>,
    up_tex: Vec<glow::Texture>,
    /// Combines a blurred level with the level above it.
    upsample_prog: glow::Program,

    // Programs
    bloom_prog:     glow::Program,
    composite_prog: glow::Program,

    // Empty VAO for fullscreen draws (GL 3.3 Core requires a VAO bound)
    fullscreen_vao: glow::VertexArray,
}

impl PostFxPipeline {
    pub unsafe fn new(gl: &glow::Context, width: u32, height: u32) -> Self {
        let bloom_prog     = compile_postfx_program(gl, FULLSCREEN_VERT, BLOOM_FRAG);
        let composite_prog = compile_postfx_program(gl, FULLSCREEN_VERT, COMPOSITE_FRAG);
        let fullscreen_vao = gl.create_vertex_array().expect("postfx fullscreen_vao");

        // Pre-bind sampler units (never changes)
        gl.use_program(Some(bloom_prog));
        set_u_i32(gl, bloom_prog, "u_texture", 0);

        gl.use_program(Some(composite_prog));
        set_u_i32(gl, composite_prog, "u_scene", 0);
        set_u_i32(gl, composite_prog, "u_bloom", 1);

        let (scene_fbo, scene_color_tex, scene_emission_tex) =
            create_scene_fbo(gl, width, height);
        let (bloom_fbo, bloom_tex) =
            create_bloom_fbos(gl, (width / 2).max(1), (height / 2).max(1));
        let (mip_fbo, mip_tex, mip_size) = create_pyramid(gl, width, height);
        let (up_fbo, up_tex, _) = create_pyramid(gl, width, height);

        let upsample_prog = compile_postfx_program(gl, FULLSCREEN_VERT, UPSAMPLE_FRAG);
        gl.use_program(Some(upsample_prog));
        set_u_i32(gl, upsample_prog, "u_lower", 0);
        set_u_i32(gl, upsample_prog, "u_higher", 1);

        Self {
            scene_fbo,
            scene_color_tex,
            scene_emission_tex,
            bloom_fbo,
            bloom_tex,
            mip_fbo,
            mip_tex,
            mip_size,
            up_fbo,
            up_tex,
            upsample_prog,
            bloom_prog,
            composite_prog,
            fullscreen_vao,
        }
    }

    /// Recreate FBO textures after a window resize. Call from Pipeline::poll_events resize handler.
    pub unsafe fn resize(&mut self, gl: &glow::Context, width: u32, height: u32) {
        // Delete old resources
        gl.delete_framebuffer(self.scene_fbo);
        gl.delete_texture(self.scene_color_tex);
        gl.delete_texture(self.scene_emission_tex);
        for i in 0..2 {
            gl.delete_framebuffer(self.bloom_fbo[i]);
            gl.delete_texture(self.bloom_tex[i]);
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
        let (mip_fbo, mip_tex, mip_size) = create_pyramid(gl, width, height);
        let (up_fbo, up_tex, _) = create_pyramid(gl, width, height);
        self.mip_fbo = mip_fbo;
        self.mip_tex = mip_tex;
        self.mip_size = mip_size;
        self.up_fbo = up_fbo;
        self.up_tex = up_tex;

        let (scene_fbo, scene_color_tex, scene_emission_tex) =
            create_scene_fbo(gl, width, height);
        let (bloom_fbo, bloom_tex) =
            create_bloom_fbos(gl, (width / 2).max(1), (height / 2).max(1));

        self.scene_fbo          = scene_fbo;
        self.scene_color_tex    = scene_color_tex;
        self.scene_emission_tex = scene_emission_tex;
        self.bloom_fbo          = bloom_fbo;
        self.bloom_tex          = bloom_tex;
    }

    /// Run bloom passes then composite to the default (screen) framebuffer.
    ///
    /// Must be called after all glyphs have been drawn to scene_fbo.
    pub unsafe fn run(
        &self,
        gl:     &glow::Context,
        config: &RenderConfig,
        full_w: u32,
        full_h: u32,
        time:   f32,
    ) {
        let bw = (full_w / 2).max(1) as i32;
        let bh = (full_h / 2).max(1) as i32;

        gl.bind_vertex_array(Some(self.fullscreen_vao));

        // ── Bloom ────────────────────────────────────────────────────────────
        //
        // Down the pyramid blurring as it goes, then back up adding each level
        // into the one above it. The result contains a tight core, a wide
        // halo and a broad veil at once, which is what light actually does and
        // what no single Gaussian can produce.
        if config.bloom_enabled && !self.mip_fbo.is_empty() {
            let radius = config.bloom_radius.max(0.5);
            gl.use_program(Some(self.bloom_prog));
            gl.active_texture(glow::TEXTURE0);
            set_u_f32(gl, self.bloom_prog, "u_threshold", config.bloom_threshold);
            set_u_f32(gl, self.bloom_prog, "u_knee", config.bloom_knee.max(1e-3));
            set_u_f32(gl, self.bloom_prog, "u_stretch", 1.0);

            // ── Down ─────────────────────────────────────────────────────────
            //
            // Each level takes the one above it, blurs horizontally into the
            // scratch pair and vertically into itself. Only the first level
            // thresholds; doing it again at every level would eat the blur
            // just produced.
            for level in 0..self.mip_fbo.len() {
                let (w, h) = self.mip_size[level];
                let source = if level == 0 {
                    self.scene_emission_tex
                } else {
                    self.mip_tex[level - 1]
                };

                // Horizontal, into the half-res scratch buffer.
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.bloom_fbo[0]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_texture(glow::TEXTURE_2D, Some(source));
                set_u_bool(gl, self.bloom_prog, "u_prefilter", level == 0);
                set_u_bool(gl, self.bloom_prog, "u_horizontal", true);
                set_u_f32(gl,  self.bloom_prog, "u_radius", radius);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);

                // Vertical, into this level.
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.mip_fbo[level]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.bloom_tex[0]));
                set_u_bool(gl, self.bloom_prog, "u_prefilter", false);
                set_u_bool(gl, self.bloom_prog, "u_horizontal", false);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
            }

            // ── The streak ───────────────────────────────────────────────────
            //
            // Applied to one middle level rather than to all of them, so it is
            // a flare off bright things rather than a horizontal smear over
            // the whole frame.
            if config.anamorphic > 0.0 && self.mip_fbo.len() > 2 {
                let level = self.mip_fbo.len() / 2;
                let (w, h) = self.mip_size[level];
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.bloom_fbo[0]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.mip_tex[level]));
                set_u_bool(gl, self.bloom_prog, "u_horizontal", true);
                set_u_f32(gl,  self.bloom_prog, "u_stretch", 9.0 * config.anamorphic);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);

                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.mip_fbo[level]));
                gl.viewport(0, 0, w, h);
                gl.clear(glow::COLOR_BUFFER_BIT);
                gl.bind_texture(glow::TEXTURE_2D, Some(self.bloom_tex[0]));
                set_u_bool(gl, self.bloom_prog, "u_horizontal", true);
                set_u_f32(gl,  self.bloom_prog, "u_stretch", 16.0 * config.anamorphic);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
                set_u_f32(gl,  self.bloom_prog, "u_stretch", 1.0);
            }

            // ── Up ───────────────────────────────────────────────────────────
            //
            // From the smallest level back to the largest, each one added into
            // the level above through a tent filter. The strength falls off as
            // it climbs, so the broadest scales are a veil rather than a wash.
            gl.use_program(Some(self.upsample_prog));
            let top = self.mip_fbo.len() - 1;
            // The smallest level starts the chain unchanged.
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
                // Falls off as the chain climbs, which the comment above
                // claimed and a constant 0.82 did not do: the broadest levels
                // cover the whole frame, and at full strength they lift the
                // black of the picture into a general wash.
                let up = (level + 1) as f32 / top as f32;
                set_u_f32(gl, self.upsample_prog, "u_strength", 0.55 + up * 0.30);
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
            }
            gl.active_texture(glow::TEXTURE0);
        }

        // ── Composite to screen ───────────────────────────────────────────────
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.viewport(0, 0, full_w as i32, full_h as i32);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.use_program(Some(self.composite_prog));

        // Texture unit 0: scene color
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(self.scene_color_tex));

        // Texture unit 1: bloom result (or scene color at zero intensity if disabled)
        gl.active_texture(glow::TEXTURE1);
        if config.bloom_enabled && !self.up_tex.is_empty() {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.up_tex[0]));
        } else {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.scene_color_tex));
        }

        let bloom_intensity = if config.bloom_enabled { config.bloom_intensity } else { 0.0 };
        set_u_f32(gl,  self.composite_prog, "u_bloom_intensity",    bloom_intensity);
        // The grade, from the config rather than from four numbers written
        // here where nothing could reach them.
        set_u_f32(gl,  self.composite_prog, "u_exposure",           config.exposure);
        set_u_f32(gl,  self.composite_prog, "u_tonemap",            config.tonemap);
        set_u_f32(gl,  self.composite_prog, "u_halation",           config.halation);
        set_u_vec3(gl, self.composite_prog, "u_tint",               config.tint);
        set_u_vec3(gl, self.composite_prog, "u_lift",               config.lift);
        set_u_vec3(gl, self.composite_prog, "u_gain",               config.gain);
        set_u_f32(gl,  self.composite_prog, "u_saturation",         config.saturation);
        set_u_f32(gl,  self.composite_prog, "u_contrast",           config.contrast);
        set_u_f32(gl,  self.composite_prog, "u_brightness",         config.brightness);
        set_u_f32(gl,  self.composite_prog, "u_vignette",           config.vignette);
        set_u_f32(gl,  self.composite_prog, "u_vignette_softness",  config.vignette_softness);
        set_u_f32(gl,  self.composite_prog, "u_sharpen",            config.sharpen);
        set_u_f32(gl,  self.composite_prog, "u_dither",             config.dither);
        set_u_f32(gl,  self.composite_prog, "u_barrel",             config.barrel);
        set_u_f32(gl,  self.composite_prog, "u_grain_intensity",    config.film_grain);
        set_u_f32(gl,  self.composite_prog, "u_grain_seed",         time);
        set_u_f32(gl,  self.composite_prog, "u_chromatic",          config.chromatic_aberration);
        set_u_f32(gl,  self.composite_prog, "u_scanline_intensity",
            if config.scanlines_enabled { config.scanline_intensity } else { 0.0 });
        set_u_bool(gl, self.composite_prog, "u_scanlines_enabled",  config.scanlines_enabled);

        gl.draw_arrays(glow::TRIANGLES, 0, 3);

        gl.bind_vertex_array(None);
    }
}

// ── FBO helpers ────────────────────────────────────────────────────────────────

unsafe fn make_rgba_tex(gl: &glow::Context, w: u32, h: u32) -> glow::Texture {
    let tex = gl.create_texture().expect("postfx texture");
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    gl.tex_image_2d(
        glow::TEXTURE_2D, 0, glow::RGBA as i32,
        w as i32, h as i32, 0,
        glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None),
    );
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S,     glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T,     glow::CLAMP_TO_EDGE as i32);
    tex
}

unsafe fn create_scene_fbo(
    gl: &glow::Context, w: u32, h: u32,
) -> (glow::Framebuffer, glow::Texture, glow::Texture) {
    let color_tex    = make_rgba_tex(gl, w, h);
    let emission_tex = make_rgba_tex(gl, w, h);

    let fbo = gl.create_framebuffer().expect("scene fbo");
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(color_tex), 0,
    );
    gl.framebuffer_texture_2d(
        glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT1, glow::TEXTURE_2D, Some(emission_tex), 0,
    );
    gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1]);

    let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
    if status != glow::FRAMEBUFFER_COMPLETE {
        log::error!("Scene FBO incomplete: 0x{status:X}");
    }
    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    (fbo, color_tex, emission_tex)
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
        let tex = make_rgba_tex(gl, w, h);
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

unsafe fn create_bloom_fbos(
    gl: &glow::Context, w: u32, h: u32,
) -> ([glow::Framebuffer; 2], [glow::Texture; 2]) {
    let textures = [make_rgba_tex(gl, w, h), make_rgba_tex(gl, w, h)];
    let fbos = [
        gl.create_framebuffer().expect("bloom fbo 0"),
        gl.create_framebuffer().expect("bloom fbo 1"),
    ];
    for i in 0..2 {
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbos[i]));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(textures[i]), 0,
        );
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
    }
    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    (fbos, textures)
}

// ── Shader compilation ─────────────────────────────────────────────────────────

unsafe fn compile_postfx_program(
    gl: &glow::Context, vert_src: &str, frag_src: &str,
) -> glow::Program {
    let vs = gl.create_shader(glow::VERTEX_SHADER).expect("postfx vert shader");
    gl.shader_source(vs, vert_src);
    gl.compile_shader(vs);
    if !gl.get_shader_compile_status(vs) {
        panic!("PostFx vert compile error:\n{}", gl.get_shader_info_log(vs));
    }

    let fs = gl.create_shader(glow::FRAGMENT_SHADER).expect("postfx frag shader");
    gl.shader_source(fs, frag_src);
    gl.compile_shader(fs);
    if !gl.get_shader_compile_status(fs) {
        panic!("PostFx frag compile error:\n{}", gl.get_shader_info_log(fs));
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

// ── Uniform helpers ────────────────────────────────────────────────────────────

unsafe fn set_u_i32(gl: &glow::Context, prog: glow::Program, name: &str, v: i32) {
    if let Some(loc) = gl.get_uniform_location(prog, name) {
        gl.uniform_1_i32(Some(&loc), v);
    }
}

unsafe fn set_u_bool(gl: &glow::Context, prog: glow::Program, name: &str, v: bool) {
    if let Some(loc) = gl.get_uniform_location(prog, name) {
        gl.uniform_1_i32(Some(&loc), v as i32);
    }
}

unsafe fn set_u_f32(gl: &glow::Context, prog: glow::Program, name: &str, v: f32) {
    if let Some(loc) = gl.get_uniform_location(prog, name) {
        gl.uniform_1_f32(Some(&loc), v);
    }
}

unsafe fn set_u_vec3(gl: &glow::Context, prog: glow::Program, name: &str, v: [f32; 3]) {
    if let Some(loc) = gl.get_uniform_location(prog, name) {
        gl.uniform_3_f32(Some(&loc), v[0], v[1], v[2]);
    }
}
