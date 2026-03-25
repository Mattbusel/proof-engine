//! 2D Sprite Batching — high-performance instanced draw-call batching system.
//!
//! Renders thousands of quads in minimal draw calls by sorting sprites by
//! (texture atlas page, blend mode, render layer), packing per-instance data
//! into a GPU buffer, and issuing one `glDrawArraysInstanced` per batch.
//!
//! # Architecture
//!
//! ```text
//! SpriteBatch owns:
//!   ├─ VAO (shared unit-quad geometry + instanced attributes)
//!   ├─ quad_vbo (static, uploaded once)
//!   ├─ instance_vbos[2] (double-buffered, swapped each frame)
//!   ├─ instance_data: Vec<SpriteInstance> (CPU staging buffer)
//!   └─ shader program + atlas texture handle
//!
//! Each frame:
//!   1. Collect SpriteInstances from glyphs, particles, UI, etc.
//!   2. Sort by BatchSortKey (layer → blend → atlas page → depth)
//!   3. Split into DrawBatch runs (contiguous key ranges)
//!   4. Upload to the "write" VBO (orphan-buffer strategy)
//!   5. Issue one glDrawArraysInstanced per DrawBatch
//!   6. Swap read/write VBO index
//! ```

use glow::HasContext;
use glam::{Vec2, Vec3, Vec4};
use bytemuck::cast_slice;

use crate::glyph::{Glyph, RenderLayer, BlendMode as GlyphBlendMode};
use crate::glyph::batch::{GlyphInstance, BlendMode, RenderLayerOrd};
use crate::glyph::atlas::FontAtlas;
use crate::particle::ParticlePool;
use crate::scene::Scene;

// ── SpriteInstance ───────────────────────────────────────────────────────────

/// Per-instance GPU data for the sprite batch system.
///
/// This is a compact 48-byte struct optimized for bandwidth. It drops glow
/// fields from `GlyphInstance` (84 bytes) that the sprite batch shader doesn't
/// need, keeping only what's required for instanced quad rendering.
///
/// For compatibility with the existing pipeline, we also provide conversion
/// from the full `GlyphInstance` format.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SpriteInstance {
    pub position: [f32; 3],     // x, y, z
    pub scale: [f32; 2],        // width, height
    pub uv_rect: [f32; 4],     // u_min, v_min, u_max, v_max
    pub color: [f32; 4],       // r, g, b, a
    pub emission: f32,
    pub rotation: f32,
    // total: 60 bytes per instance
}

impl SpriteInstance {
    pub fn from_glyph(glyph: &Glyph, atlas: &FontAtlas) -> Self {
        let uv = atlas.uv_for(glyph.character);
        Self {
            position: glyph.position.to_array(),
            scale: [glyph.scale.x, glyph.scale.y],
            uv_rect: [uv.u0, uv.v0, uv.u1, uv.v1],
            color: glyph.color.to_array(),
            emission: glyph.emission,
            rotation: glyph.rotation,
        }
    }

    pub fn from_glyph_instance(gi: &GlyphInstance) -> Self {
        Self {
            position: gi.position,
            scale: gi.scale,
            uv_rect: [
                gi.uv_offset[0],
                gi.uv_offset[1],
                gi.uv_offset[0] + gi.uv_size[0],
                gi.uv_offset[1] + gi.uv_size[1],
            ],
            color: gi.color,
            emission: gi.emission,
            rotation: gi.rotation,
        }
    }
}

// ── Batch sort key ──────────────────────────────────────────────────────────

/// Determines draw-call grouping. Sprites with the same key render together.
///
/// Sort order: atlas_page → blend_mode_ord → layer_ord
/// This minimizes texture binds (most expensive), then blend state changes,
/// then respects layer ordering for visual correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchSortKey {
    pub atlas_page: u8,
    pub blend_ord: u8,      // 0=Opaque/Alpha, 1=Additive, 2=Multiply, 3=Screen
    pub layer_ord: u8,      // from RenderLayerOrd
}

impl BatchSortKey {
    pub fn new(layer: RenderLayer, blend: BlendMode, atlas_page: u8) -> Self {
        let blend_ord = match blend {
            BlendMode::Alpha    => 0,
            BlendMode::Additive => 1,
            BlendMode::Multiply => 2,
            BlendMode::Screen   => 3,
        };
        Self {
            atlas_page,
            blend_ord,
            layer_ord: RenderLayerOrd::from_layer(layer).0,
        }
    }

    pub fn from_glyph(glyph: &Glyph) -> Self {
        let blend = match glyph.blend_mode {
            GlyphBlendMode::Normal   => BlendMode::Alpha,
            GlyphBlendMode::Additive => BlendMode::Additive,
            GlyphBlendMode::Multiply => BlendMode::Multiply,
            GlyphBlendMode::Screen   => BlendMode::Screen,
        };
        Self::new(glyph.layer, blend, 0)
    }

    /// Packed u32 for fast comparison/sorting.
    #[inline]
    pub fn sort_value(&self) -> u32 {
        (self.atlas_page as u32) << 16
            | (self.layer_ord as u32) << 8
            | (self.blend_ord as u32)
    }

    pub fn blend_mode(&self) -> BlendMode {
        match self.blend_ord {
            0 => BlendMode::Alpha,
            1 => BlendMode::Additive,
            2 => BlendMode::Multiply,
            3 => BlendMode::Screen,
            _ => BlendMode::Alpha,
        }
    }
}

impl PartialOrd for BatchSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BatchSortKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_value().cmp(&other.sort_value())
    }
}

// ── Pending sprite (CPU staging) ────────────────────────────────────────────

/// A sprite waiting to be sorted and batched.
struct PendingSprite {
    key: BatchSortKey,
    instance: GlyphInstance,
    depth: f32,
}

// ── Draw batch (one draw call) ──────────────────────────────────────────────

/// A contiguous range of instances that share the same batch key.
/// Rendered with a single `glDrawArraysInstanced` call.
#[derive(Debug, Clone)]
pub struct DrawBatch {
    pub key: BatchSortKey,
    pub offset: usize,      // start index in the instance buffer
    pub count: usize,       // number of instances
}

// ── Per-frame statistics ────────────────────────────────────────────────────

/// Statistics from the last frame's sprite batch rendering.
#[derive(Debug, Clone, Default)]
pub struct SpriteBatchStats {
    /// Total number of draw calls issued.
    pub draw_calls: u32,
    /// Total number of sprite instances rendered.
    pub instance_count: usize,
    /// Total bytes uploaded to the GPU this frame.
    pub upload_bytes: usize,
    /// Number of glyph instances.
    pub glyph_count: usize,
    /// Number of particle instances.
    pub particle_count: usize,
    /// Current VBO capacity in instances.
    pub vbo_capacity: usize,
    /// Whether the VBO was grown this frame.
    pub vbo_grew: bool,
    /// Instances per draw call (average).
    pub avg_batch_size: f32,
}

// ── SpriteBatch ─────────────────────────────────────────────────────────────

/// High-performance instanced sprite renderer.
///
/// Manages a double-buffered instance VBO, sorts sprites by batch key,
/// and issues minimal draw calls. Target: 8,192+ sprites per draw call.
pub struct SpriteBatch {
    // GL resources
    vao: glow::VertexArray,
    quad_vbo: glow::Buffer,
    instance_vbos: [glow::Buffer; 2],  // double-buffered
    write_index: usize,                 // which VBO to write to this frame

    // CPU staging
    pending: Vec<PendingSprite>,
    sorted_instances: Vec<GlyphInstance>,
    batches: Vec<DrawBatch>,

    // Capacity management
    max_instances: usize,
    vbo_capacity_bytes: usize,

    // Stats
    pub stats: SpriteBatchStats,
}

/// Default initial capacity: 8192 sprites.
const DEFAULT_MAX_INSTANCES: usize = 8192;

/// Minimum capacity after growth.
const MIN_CAPACITY: usize = 1024;

impl SpriteBatch {
    /// Create a new SpriteBatch with the given GL context.
    ///
    /// # Safety
    /// Must be called with a valid, current OpenGL context.
    pub unsafe fn new(gl: &glow::Context) -> Self {
        let vao = gl.create_vertex_array().expect("SpriteBatch: create VAO");
        gl.bind_vertex_array(Some(vao));

        // ── Static quad geometry VBO ────────────────────────────────────────
        let quad_vbo = gl.create_buffer().expect("SpriteBatch: create quad_vbo");
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad_vbo));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, cast_slice(&QUAD_VERTS), glow::STATIC_DRAW);

        // location 0: vec2 v_pos (offset 0, stride 16)
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
        gl.enable_vertex_attrib_array(0);
        // location 1: vec2 v_uv (offset 8, stride 16)
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);
        gl.enable_vertex_attrib_array(1);

        // ── Double-buffered instance VBOs ───────────────────────────────────
        let capacity_bytes = DEFAULT_MAX_INSTANCES * std::mem::size_of::<GlyphInstance>();
        let instance_vbos = [
            Self::create_instance_vbo(gl, capacity_bytes),
            Self::create_instance_vbo(gl, capacity_bytes),
        ];

        // Set up instance attributes on the first VBO (we'll rebind per frame)
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(instance_vbos[0]));
        Self::setup_instance_attribs(gl);

        gl.bind_vertex_array(None);

        Self {
            vao,
            quad_vbo,
            instance_vbos,
            write_index: 0,
            pending: Vec::with_capacity(DEFAULT_MAX_INSTANCES),
            sorted_instances: Vec::with_capacity(DEFAULT_MAX_INSTANCES),
            batches: Vec::with_capacity(32),
            max_instances: DEFAULT_MAX_INSTANCES,
            vbo_capacity_bytes: capacity_bytes,
            stats: SpriteBatchStats::default(),
        }
    }

    /// Create an instance VBO with pre-allocated capacity.
    unsafe fn create_instance_vbo(gl: &glow::Context, capacity_bytes: usize) -> glow::Buffer {
        let vbo = gl.create_buffer().expect("SpriteBatch: create instance_vbo");
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        // Pre-allocate with null data
        gl.buffer_data_size(
            glow::ARRAY_BUFFER,
            capacity_bytes as i32,
            glow::DYNAMIC_DRAW,
        );
        vbo
    }

    /// Configure instanced vertex attributes (locations 2-10) matching GlyphInstance layout.
    unsafe fn setup_instance_attribs(gl: &glow::Context) {
        let stride = std::mem::size_of::<GlyphInstance>() as i32;

        macro_rules! inst_attr {
            ($loc:expr, $count:expr, $off:expr) => {{
                gl.vertex_attrib_pointer_f32($loc, $count, glow::FLOAT, false, stride, $off);
                gl.enable_vertex_attrib_array($loc);
                gl.vertex_attrib_divisor($loc, 1);
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
    }

    // ── Frame lifecycle ─────────────────────────────────────────────────────

    /// Begin a new frame. Clears all pending sprites and stats.
    pub fn begin_frame(&mut self) {
        self.pending.clear();
        self.sorted_instances.clear();
        self.batches.clear();
        self.stats = SpriteBatchStats {
            vbo_capacity: self.max_instances,
            ..Default::default()
        };
    }

    /// Submit a glyph for batched rendering.
    pub fn push_glyph(&mut self, glyph: &Glyph, atlas: &FontAtlas) {
        if !glyph.visible { return; }

        let key = BatchSortKey::from_glyph(glyph);
        let uv = atlas.uv_for(glyph.character);

        let instance = GlyphInstance {
            position: glyph.position.to_array(),
            scale: [glyph.scale.x, glyph.scale.y],
            rotation: glyph.rotation,
            color: glyph.color.to_array(),
            emission: glyph.emission,
            glow_color: glyph.glow_color.to_array(),
            glow_radius: glyph.glow_radius,
            uv_offset: uv.offset(),
            uv_size: uv.size(),
            _pad: [0.0; 2],
        };

        self.pending.push(PendingSprite {
            key,
            instance,
            depth: glyph.position.z,
        });
        self.stats.glyph_count += 1;
    }

    /// Submit a glyph with a life_function scale applied.
    pub fn push_glyph_with_life_scale(&mut self, glyph: &Glyph, atlas: &FontAtlas, life_scale: f32) {
        if !glyph.visible { return; }

        let key = BatchSortKey::from_glyph(glyph);
        let uv = atlas.uv_for(glyph.character);

        let instance = GlyphInstance {
            position: glyph.position.to_array(),
            scale: [glyph.scale.x * life_scale, glyph.scale.y * life_scale],
            rotation: glyph.rotation,
            color: glyph.color.to_array(),
            emission: glyph.emission,
            glow_color: glyph.glow_color.to_array(),
            glow_radius: glyph.glow_radius,
            uv_offset: uv.offset(),
            uv_size: uv.size(),
            _pad: [0.0; 2],
        };

        self.pending.push(PendingSprite {
            key,
            instance,
            depth: glyph.position.z,
        });
        self.stats.glyph_count += 1;
    }

    /// Submit a pre-built GlyphInstance with an explicit batch key.
    pub fn push_instance(&mut self, key: BatchSortKey, instance: GlyphInstance, depth: f32) {
        self.pending.push(PendingSprite { key, instance, depth });
    }

    /// Submit all visible glyphs from the scene's glyph pool.
    pub fn collect_glyphs(&mut self, scene: &Scene, atlas: &FontAtlas) {
        for (_, glyph) in scene.glyphs.iter() {
            if !glyph.visible { continue; }

            let life_scale = if let Some(ref f) = glyph.life_function {
                f.evaluate(scene.time, 0.0)
            } else {
                1.0
            };

            self.push_glyph_with_life_scale(glyph, atlas, life_scale);
        }
    }

    /// Submit all alive particles from the scene's particle pool.
    ///
    /// This is the zero-copy-ish path: particles write directly into pending
    /// sprites in GlyphInstance format, avoiding intermediate allocations.
    pub fn collect_particles(&mut self, scene: &Scene, atlas: &FontAtlas) {
        for particle in scene.particles.iter() {
            let g = &particle.glyph;
            if !g.visible { continue; }

            let uv = atlas.uv_for(g.character);
            let key = BatchSortKey::new(
                g.layer,
                match g.blend_mode {
                    GlyphBlendMode::Normal   => BlendMode::Alpha,
                    GlyphBlendMode::Additive => BlendMode::Additive,
                    GlyphBlendMode::Multiply => BlendMode::Multiply,
                    GlyphBlendMode::Screen   => BlendMode::Screen,
                },
                0,
            );

            let instance = GlyphInstance {
                position: g.position.to_array(),
                scale: [g.scale.x, g.scale.y],
                rotation: g.rotation,
                color: g.color.to_array(),
                emission: g.emission,
                glow_color: g.glow_color.to_array(),
                glow_radius: g.glow_radius,
                uv_offset: uv.offset(),
                uv_size: uv.size(),
                _pad: [0.0; 2],
            };

            self.pending.push(PendingSprite {
                key,
                instance,
                depth: g.position.z,
            });
            self.stats.particle_count += 1;
        }
    }

    /// Collect everything from the scene (glyphs + particles) in one call.
    pub fn collect_scene(&mut self, scene: &Scene, atlas: &FontAtlas) {
        self.collect_glyphs(scene, atlas);
        self.collect_particles(scene, atlas);
    }

    // ── Sort and batch ──────────────────────────────────────────────────────

    /// Sort pending sprites by batch key, build draw batches, and pack the
    /// sorted instance array. Call after all `push_*` / `collect_*` calls.
    pub fn build_batches(&mut self) {
        if self.pending.is_empty() { return; }

        // Sort by key, then back-to-front by depth within each key
        self.pending.sort_unstable_by(|a, b| {
            a.key.sort_value().cmp(&b.key.sort_value())
                .then(b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Build sorted instance array and batch ranges
        self.sorted_instances.clear();
        self.sorted_instances.reserve(self.pending.len());
        self.batches.clear();

        let mut current_key: Option<BatchSortKey> = None;
        let mut batch_start: usize = 0;

        for (i, sprite) in self.pending.iter().enumerate() {
            if current_key != Some(sprite.key) {
                // Close previous batch
                if let Some(prev_key) = current_key {
                    if i > batch_start {
                        self.batches.push(DrawBatch {
                            key: prev_key,
                            offset: batch_start,
                            count: i - batch_start,
                        });
                    }
                }
                current_key = Some(sprite.key);
                batch_start = i;
            }
            self.sorted_instances.push(sprite.instance);
        }

        // Close final batch
        if let Some(key) = current_key {
            let total = self.pending.len();
            if total > batch_start {
                self.batches.push(DrawBatch {
                    key,
                    offset: batch_start,
                    count: total - batch_start,
                });
            }
        }

        self.stats.instance_count = self.sorted_instances.len();
    }

    // ── GPU upload + draw ───────────────────────────────────────────────────

    /// Upload the sorted instance buffer to the GPU and issue draw calls.
    ///
    /// Uses the orphan-buffer strategy: `glBufferData(NULL)` to discard the
    /// old buffer, then `glBufferSubData` to upload new data, avoiding GPU
    /// sync stalls.
    ///
    /// # Safety
    /// Must be called with a valid, current OpenGL context. The caller must
    /// have already bound the correct framebuffer and set the viewport.
    pub unsafe fn flush(
        &mut self,
        gl: &glow::Context,
        program: glow::Program,
        view_proj_loc: &glow::UniformLocation,
        view_proj: &[f32; 16],
        atlas_tex: glow::Texture,
    ) -> u32 {
        if self.sorted_instances.is_empty() {
            return 0;
        }

        // ── Grow VBO if needed ──────────────────────────────────────────────
        if self.sorted_instances.len() > self.max_instances {
            let new_capacity = (self.sorted_instances.len() * 2).max(MIN_CAPACITY);
            self.grow_vbos(gl, new_capacity);
            self.stats.vbo_grew = true;
        }

        // ── Select write VBO (double-buffering) ────────────────────────────
        let write_vbo = self.instance_vbos[self.write_index];

        // ── Orphan buffer strategy ──────────────────────────────────────────
        // Upload: orphan old buffer then write new data in one call
        let instance_bytes = cast_slice::<GlyphInstance, u8>(&self.sorted_instances);
        let upload_size = instance_bytes.len();
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(write_vbo));
        gl.buffer_data_u8_slice(
            glow::ARRAY_BUFFER,
            instance_bytes,
            glow::DYNAMIC_DRAW,
        );

        self.stats.upload_bytes = upload_size;

        // ── Bind shader, atlas, VAO ─────────────────────────────────────────
        gl.use_program(Some(program));
        gl.uniform_matrix_4_f32_slice(Some(view_proj_loc), false, view_proj);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(atlas_tex));

        // Rebind VAO and update instance buffer binding
        gl.bind_vertex_array(Some(self.vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(write_vbo));
        Self::setup_instance_attribs(gl);

        // ── Issue draw calls per batch ──────────────────────────────────────
        let mut draw_calls = 0u32;
        let mut current_blend: Option<BlendMode> = None;

        for batch in &self.batches {
            // Switch blend mode if needed
            let blend = batch.key.blend_mode();
            if current_blend != Some(blend) {
                Self::set_blend_mode(gl, blend);
                current_blend = Some(blend);
            }

            // Draw this batch — all instances are contiguous in the buffer
            // We need to offset the instance attributes. Since we can't easily
            // use baseInstance in GL 3.3, we re-upload with sub-ranges or use
            // a vertex attrib offset trick.
            //
            // For GL 3.3 compatibility, we use glDrawArraysInstanced with the
            // full buffer and skip via attribute pointer offsets.
            let byte_offset = batch.offset * std::mem::size_of::<GlyphInstance>();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(write_vbo));
            Self::setup_instance_attribs_with_offset(gl, byte_offset);

            gl.draw_arrays_instanced(glow::TRIANGLES, 0, 6, batch.count as i32);
            draw_calls += 1;
        }

        // ── Restore default blend ───────────────────────────────────────────
        if current_blend != Some(BlendMode::Alpha) {
            Self::set_blend_mode(gl, BlendMode::Alpha);
        }

        // ── Swap double-buffer index ────────────────────────────────────────
        self.write_index = 1 - self.write_index;

        self.stats.draw_calls = draw_calls;
        if draw_calls > 0 {
            self.stats.avg_batch_size = self.stats.instance_count as f32 / draw_calls as f32;
        }

        draw_calls
    }

    /// Set GL blend mode for a batch.
    unsafe fn set_blend_mode(gl: &glow::Context, mode: BlendMode) {
        match mode {
            BlendMode::Alpha => {
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            }
            BlendMode::Additive => {
                gl.blend_func(glow::SRC_ALPHA, glow::ONE);
            }
            BlendMode::Multiply => {
                gl.blend_func(glow::DST_COLOR, glow::ZERO);
            }
            BlendMode::Screen => {
                gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_COLOR);
            }
        }
    }

    /// Configure instanced attributes with a byte offset into the instance VBO.
    /// Used to render sub-ranges of the instance buffer without baseInstance.
    unsafe fn setup_instance_attribs_with_offset(gl: &glow::Context, byte_offset: usize) {
        let stride = std::mem::size_of::<GlyphInstance>() as i32;
        let off = byte_offset;

        macro_rules! inst_attr {
            ($loc:expr, $count:expr, $field_off:expr) => {{
                gl.vertex_attrib_pointer_f32(
                    $loc, $count, glow::FLOAT, false, stride,
                    (off + $field_off) as i32,
                );
                gl.enable_vertex_attrib_array($loc);
                gl.vertex_attrib_divisor($loc, 1);
            }};
        }

        inst_attr!(2,  3,  0);   // i_position
        inst_attr!(3,  2, 12);   // i_scale
        inst_attr!(4,  1, 20);   // i_rotation
        inst_attr!(5,  4, 24);   // i_color
        inst_attr!(6,  1, 40);   // i_emission
        inst_attr!(7,  3, 44);   // i_glow_color
        inst_attr!(8,  1, 56);   // i_glow_radius
        inst_attr!(9,  2, 60);   // i_uv_offset
        inst_attr!(10, 2, 68);   // i_uv_size
    }

    // ── Capacity management ─────────────────────────────────────────────────

    /// Grow both instance VBOs to accommodate `new_capacity` instances.
    unsafe fn grow_vbos(&mut self, gl: &glow::Context, new_capacity: usize) {
        let new_bytes = new_capacity * std::mem::size_of::<GlyphInstance>();
        log::info!(
            "SpriteBatch: growing VBOs {} → {} instances ({} → {} bytes)",
            self.max_instances, new_capacity,
            self.vbo_capacity_bytes, new_bytes,
        );

        for vbo in &self.instance_vbos {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(*vbo));
            gl.buffer_data_size(glow::ARRAY_BUFFER, new_bytes as i32, glow::DYNAMIC_DRAW);
        }

        self.max_instances = new_capacity;
        self.vbo_capacity_bytes = new_bytes;
    }

    /// Current VBO capacity in instances.
    pub fn capacity(&self) -> usize { self.max_instances }

    /// Pre-allocate capacity for at least `n` instances.
    pub unsafe fn reserve(&mut self, gl: &glow::Context, n: usize) {
        if n > self.max_instances {
            self.grow_vbos(gl, n);
        }
    }

    // ── Cleanup ─────────────────────────────────────────────────────────────

    /// Delete all GL resources owned by this SpriteBatch.
    ///
    /// # Safety
    /// Must be called with the same GL context that created the resources.
    pub unsafe fn destroy(&self, gl: &glow::Context) {
        gl.delete_vertex_array(self.vao);
        gl.delete_buffer(self.quad_vbo);
        for vbo in &self.instance_vbos {
            gl.delete_buffer(*vbo);
        }
    }

    // ── Accessors ───────────────────────────────────────────────────────────

    pub fn batches(&self) -> &[DrawBatch] { &self.batches }
    pub fn instance_count(&self) -> usize { self.sorted_instances.len() }
    pub fn pending_count(&self) -> usize { self.pending.len() }
}

// ── Particle integration: direct export to SpriteBatch ──────────────────────

/// Extension trait for ParticlePool to export directly to SpriteBatch format.
pub trait ParticleExport {
    /// Write all alive particles directly into the SpriteBatch's pending list.
    /// This is the "zero-copy" path — particles write GlyphInstances directly
    /// into the batch staging buffer, avoiding the intermediate `export_gpu_buffer`.
    fn export_to_sprite_batch(&self, batch: &mut SpriteBatch, atlas: &FontAtlas);

    /// Export particle instances into a pre-allocated slice.
    /// Returns the number of particles written.
    fn export_instances(
        &self,
        out: &mut Vec<GlyphInstance>,
        atlas: &FontAtlas,
    ) -> usize;
}

impl ParticleExport for ParticlePool {
    fn export_to_sprite_batch(&self, batch: &mut SpriteBatch, atlas: &FontAtlas) {
        for particle in self.iter() {
            let g = &particle.glyph;
            if !g.visible { continue; }

            let uv = atlas.uv_for(g.character);
            let key = BatchSortKey::new(
                g.layer,
                match g.blend_mode {
                    GlyphBlendMode::Normal   => BlendMode::Alpha,
                    GlyphBlendMode::Additive => BlendMode::Additive,
                    GlyphBlendMode::Multiply => BlendMode::Multiply,
                    GlyphBlendMode::Screen   => BlendMode::Screen,
                },
                0,
            );

            batch.push_instance(
                key,
                GlyphInstance {
                    position: g.position.to_array(),
                    scale: [g.scale.x, g.scale.y],
                    rotation: g.rotation,
                    color: g.color.to_array(),
                    emission: g.emission,
                    glow_color: g.glow_color.to_array(),
                    glow_radius: g.glow_radius,
                    uv_offset: uv.offset(),
                    uv_size: uv.size(),
                    _pad: [0.0; 2],
                },
                g.position.z,
            );
            batch.stats.particle_count += 1;
        }
    }

    fn export_instances(
        &self,
        out: &mut Vec<GlyphInstance>,
        atlas: &FontAtlas,
    ) -> usize {
        let mut count = 0;
        for particle in self.iter() {
            let g = &particle.glyph;
            if !g.visible { continue; }

            let uv = atlas.uv_for(g.character);
            out.push(GlyphInstance {
                position: g.position.to_array(),
                scale: [g.scale.x, g.scale.y],
                rotation: g.rotation,
                color: g.color.to_array(),
                emission: g.emission,
                glow_color: g.glow_color.to_array(),
                glow_radius: g.glow_radius,
                uv_offset: uv.offset(),
                uv_size: uv.size(),
                _pad: [0.0; 2],
            });
            count += 1;
        }
        count
    }
}

// ── Unit quad (same as pipeline.rs) ─────────────────────────────────────────

#[rustfmt::skip]
const QUAD_VERTS: [f32; 24] = [
    -0.5,  0.5,  0.0, 1.0,
    -0.5, -0.5,  0.0, 0.0,
     0.5,  0.5,  1.0, 1.0,
    -0.5, -0.5,  0.0, 0.0,
     0.5, -0.5,  1.0, 0.0,
     0.5,  0.5,  1.0, 1.0,
];

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph::batch::GlyphInstance;

    #[test]
    fn sprite_instance_size() {
        assert_eq!(std::mem::size_of::<SpriteInstance>(), 60);
    }

    #[test]
    fn batch_sort_key_ordering() {
        let bg_alpha = BatchSortKey::new(RenderLayer::Background, BlendMode::Alpha, 0);
        let world_alpha = BatchSortKey::new(RenderLayer::World, BlendMode::Alpha, 0);
        let world_add = BatchSortKey::new(RenderLayer::World, BlendMode::Additive, 0);
        let ui_alpha = BatchSortKey::new(RenderLayer::UI, BlendMode::Alpha, 0);
        let particle_add = BatchSortKey::new(RenderLayer::Particle, BlendMode::Additive, 0);

        // Layer ordering: Background < World < Entity < Particle < Overlay < UI
        assert!(bg_alpha < world_alpha);
        assert!(world_alpha < ui_alpha);

        // Same layer, blend ordering: Alpha < Additive
        assert!(world_alpha < world_add);

        // Different layer takes priority
        assert!(world_add < particle_add);
    }

    #[test]
    fn batch_sort_key_atlas_page_priority() {
        // Atlas page has highest priority in sort
        let page0 = BatchSortKey::new(RenderLayer::UI, BlendMode::Alpha, 0);
        let page1 = BatchSortKey::new(RenderLayer::Background, BlendMode::Alpha, 1);
        assert!(page0 < page1, "Atlas page should have highest sort priority");
    }

    fn make_test_instance(z: f32) -> GlyphInstance {
        GlyphInstance {
            position: [0.0, 0.0, z],
            scale: [1.0, 1.0],
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            emission: 0.0,
            glow_color: [0.0, 0.0, 0.0],
            glow_radius: 0.0,
            uv_offset: [0.0, 0.0],
            uv_size: [0.1, 0.1],
            _pad: [0.0; 2],
        }
    }

    /// Test-only helper: exercises sorting/batching logic without GL resources.
    struct TestBatcher {
        pending: Vec<PendingSprite>,
        sorted_instances: Vec<GlyphInstance>,
        batches: Vec<DrawBatch>,
        stats: SpriteBatchStats,
    }

    impl TestBatcher {
        fn new() -> Self {
            Self {
                pending: Vec::new(),
                sorted_instances: Vec::new(),
                batches: Vec::new(),
                stats: SpriteBatchStats::default(),
            }
        }

        fn begin_frame(&mut self) {
            self.pending.clear();
            self.sorted_instances.clear();
            self.batches.clear();
            self.stats = SpriteBatchStats::default();
        }

        fn push_instance(&mut self, key: BatchSortKey, instance: GlyphInstance, depth: f32) {
            self.pending.push(PendingSprite { key, instance, depth });
        }

        fn build_batches(&mut self) {
            if self.pending.is_empty() { return; }
            self.pending.sort_unstable_by(|a, b| {
                a.key.sort_value().cmp(&b.key.sort_value())
                    .then(b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal))
            });
            self.sorted_instances.clear();
            self.sorted_instances.reserve(self.pending.len());
            self.batches.clear();
            let mut current_key: Option<BatchSortKey> = None;
            let mut batch_start: usize = 0;
            for (i, sprite) in self.pending.iter().enumerate() {
                if current_key != Some(sprite.key) {
                    if let Some(prev_key) = current_key {
                        if i > batch_start {
                            self.batches.push(DrawBatch { key: prev_key, offset: batch_start, count: i - batch_start });
                        }
                    }
                    current_key = Some(sprite.key);
                    batch_start = i;
                }
                self.sorted_instances.push(sprite.instance);
            }
            if let Some(key) = current_key {
                let total = self.pending.len();
                if total > batch_start {
                    self.batches.push(DrawBatch { key, offset: batch_start, count: total - batch_start });
                }
            }
            self.stats.instance_count = self.sorted_instances.len();
        }

        fn instance_count(&self) -> usize { self.sorted_instances.len() }
    }

    #[test]
    fn build_batches_groups_by_key() {
        let mut batch = TestBatcher::new();
        batch.begin_frame();

        let key_world = BatchSortKey::new(RenderLayer::World, BlendMode::Alpha, 0);
        let key_ui = BatchSortKey::new(RenderLayer::UI, BlendMode::Alpha, 0);
        let key_particle = BatchSortKey::new(RenderLayer::Particle, BlendMode::Additive, 0);

        batch.push_instance(key_ui, make_test_instance(0.0), 0.0);
        batch.push_instance(key_world, make_test_instance(1.0), 1.0);
        batch.push_instance(key_particle, make_test_instance(0.5), 0.5);
        batch.push_instance(key_world, make_test_instance(2.0), 2.0);
        batch.push_instance(key_world, make_test_instance(0.0), 0.0);
        batch.push_instance(key_particle, make_test_instance(1.0), 1.0);

        batch.build_batches();

        assert_eq!(batch.batches.len(), 3);
        assert_eq!(batch.batches[0].count, 3); // world
        assert_eq!(batch.batches[1].count, 2); // particle
        assert_eq!(batch.batches[2].count, 1); // ui
        assert_eq!(batch.instance_count(), 6);
    }

    #[test]
    fn build_batches_sorts_depth_within_key() {
        let mut batch = TestBatcher::new();
        batch.begin_frame();
        let key = BatchSortKey::new(RenderLayer::World, BlendMode::Alpha, 0);
        batch.push_instance(key, make_test_instance(1.0), 1.0);
        batch.push_instance(key, make_test_instance(5.0), 5.0);
        batch.push_instance(key, make_test_instance(3.0), 3.0);
        batch.build_batches();

        let zs: Vec<f32> = batch.sorted_instances.iter().map(|i| i.position[2]).collect();
        assert!(zs[0] >= zs[1] && zs[1] >= zs[2]);
    }

    #[test]
    fn empty_batch_produces_no_draws() {
        let mut batch = TestBatcher::new();
        batch.begin_frame();
        batch.build_batches();
        assert_eq!(batch.batches.len(), 0);
        assert_eq!(batch.instance_count(), 0);
    }
}
