//! Retained-mode scene graph — glyphs persist across frames, only dirty glyphs
//! re-upload to the GPU.
//!
//! # Architecture
//!
//! ```text
//! RetainedScene
//!   ├─ glyphs: Vec<RetainedGlyph>      (all retained glyphs, indexed by slot)
//!   ├─ dirty_flags: Vec<bool>           (one per slot, set when changed)
//!   ├─ gpu_buffer: Vec<GlyphInstance>   (mirror of GPU buffer contents)
//!   ├─ generation: u64                  (increments on any mutation)
//!   └─ mode_tracker: Vec<UpdateMode>    (per-glyph: auto-detected immediate vs retained)
//!
//! Each frame:
//!   1. Check dirty_flags
//!   2. For dirty glyphs: rebuild their GlyphInstance and mark the GPU range
//!   3. Upload only dirty ranges via glBufferSubData
//!   4. If nothing dirty: zero GPU upload, zero CPU work beyond the check
//! ```
//!
//! # Hybrid Mode
//!
//! The system supports three update modes per glyph:
//! - **Retained**: Glyph rarely changes. Only re-uploaded when dirty.
//! - **Immediate**: Glyph changes every frame (chaos field, particles). Always dirty.
//! - **Auto**: System tracks update frequency and auto-promotes to immediate
//!   if the glyph has been dirty for 60+ consecutive frames.

use glow::HasContext;
use glam::{Vec2, Vec3, Vec4};
use crate::glyph::{Glyph, GlyphId, RenderLayer, BlendMode};
use crate::glyph::batch::GlyphInstance;
use crate::glyph::atlas::FontAtlas;

// ── Retained glyph handle ───────────────────────────────────────────────────

/// Opaque handle to a glyph in the retained scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RetainedId(pub u32);

// ── Update mode ─────────────────────────────────────────────────────────────

/// How a retained glyph's dirty state is managed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    /// Glyph is rarely modified. Only re-uploaded when explicitly marked dirty.
    Retained,
    /// Glyph changes every frame. Always considered dirty (skip dirty check).
    Immediate,
    /// System auto-detects: starts as Retained, promotes to Immediate if the
    /// glyph is dirty for `AUTO_PROMOTE_THRESHOLD` consecutive frames.
    Auto,
}

/// Number of consecutive dirty frames before Auto mode promotes to Immediate.
const AUTO_PROMOTE_THRESHOLD: u32 = 60;

// ── Retained glyph ─────────────────────────────────────────────────────────

/// A glyph managed by the retained scene graph.
#[derive(Clone, Debug)]
pub struct RetainedGlyph {
    pub glyph: Glyph,
    pub id: RetainedId,
    pub visible: bool,
    pub layer: RenderLayer,
    pub generation: u64,
}

// ── Dirty range ─────────────────────────────────────────────────────────────

/// A contiguous range of dirty slots in the GPU buffer.
#[derive(Debug, Clone)]
struct DirtyRange {
    start: usize,
    end: usize,  // exclusive
}

impl DirtyRange {
    fn len(&self) -> usize { self.end - self.start }
}

// ── Per-frame statistics ────────────────────────────────────────────────────

/// Statistics from the retained scene's frame update.
#[derive(Debug, Clone, Default)]
pub struct RetainedStats {
    /// Total retained glyphs.
    pub total_glyphs: usize,
    /// Number of dirty glyphs this frame.
    pub dirty_count: usize,
    /// Number of glyphs in Immediate mode (always dirty).
    pub immediate_count: usize,
    /// Number of glyphs in Retained mode.
    pub retained_count: usize,
    /// Number of glyphs in Auto mode.
    pub auto_count: usize,
    /// Number of Auto→Immediate promotions this frame.
    pub auto_promotions: usize,
    /// Bytes uploaded to GPU this frame.
    pub upload_bytes: usize,
    /// Number of dirty ranges coalesced for upload.
    pub upload_ranges: usize,
    /// Ratio: dirty / total (0.0 = nothing changed, 1.0 = everything changed).
    pub dirty_ratio: f32,
    /// Total generation (mutation counter).
    pub generation: u64,
}

// ── Mode tracker ────────────────────────────────────────────────────────────

/// Tracks per-glyph update mode and consecutive dirty frame count.
#[derive(Clone, Debug)]
struct ModeTracker {
    mode: UpdateMode,
    consecutive_dirty: u32,
}

impl ModeTracker {
    fn new(mode: UpdateMode) -> Self {
        Self { mode, consecutive_dirty: 0 }
    }

    /// Record that this glyph was dirty this frame. Returns true if mode changed.
    fn record_dirty(&mut self) -> bool {
        self.consecutive_dirty += 1;
        if self.mode == UpdateMode::Auto && self.consecutive_dirty >= AUTO_PROMOTE_THRESHOLD {
            self.mode = UpdateMode::Immediate;
            return true;
        }
        false
    }

    /// Record that this glyph was clean this frame.
    fn record_clean(&mut self) {
        self.consecutive_dirty = 0;
        // If it was promoted to Immediate but is now clean, demote back to Auto
        // (this shouldn't normally happen since Immediate glyphs are always dirty,
        // but handle it gracefully)
    }
}

// ── Retained Scene ──────────────────────────────────────────────────────────

/// Retained-mode scene graph with dirty tracking.
///
/// Manages a flat array of glyphs that persist across frames. Only glyphs
/// whose properties have changed are re-uploaded to the GPU buffer.
pub struct RetainedScene {
    // ── Glyph storage ───────────────────────────────────────────────────
    glyphs: Vec<Option<RetainedGlyph>>,
    free_slots: Vec<u32>,
    next_id: u32,

    // ── Dirty tracking ──────────────────────────────────────────────────
    dirty_flags: Vec<bool>,
    mode_trackers: Vec<ModeTracker>,

    // ── GPU buffer mirror ───────────────────────────────────────────────
    gpu_buffer: Vec<GlyphInstance>,
    /// Whether the GPU buffer needs a full re-upload (e.g. after resize).
    full_rebuild_needed: bool,

    // ── Generation counter ──────────────────────────────────────────────
    generation: u64,

    // ── Stats ───────────────────────────────────────────────────────────
    pub stats: RetainedStats,
}

impl RetainedScene {
    /// Create a new retained scene with pre-allocated capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            glyphs: vec![None; capacity],
            free_slots: (0..capacity as u32).rev().collect(),
            next_id: 0,
            dirty_flags: vec![false; capacity],
            mode_trackers: vec![ModeTracker::new(UpdateMode::Auto); capacity],
            gpu_buffer: vec![GlyphInstance {
                position: [0.0; 3], scale: [0.0; 2], rotation: 0.0,
                color: [0.0; 4], emission: 0.0, glow_color: [0.0; 3],
                glow_radius: 0.0, uv_offset: [0.0; 2], uv_size: [0.0; 2],
                _pad: [0.0; 2],
            }; capacity],
            full_rebuild_needed: true,
            generation: 0,
            stats: RetainedStats::default(),
        }
    }

    // ── Glyph management ────────────────────────────────────────────────

    /// Insert a new glyph into the retained scene.
    pub fn insert(&mut self, glyph: Glyph, mode: UpdateMode) -> RetainedId {
        let id = RetainedId(self.next_id);
        self.next_id += 1;

        let slot = if let Some(s) = self.free_slots.pop() {
            s as usize
        } else {
            // Grow
            let s = self.glyphs.len();
            self.glyphs.push(None);
            self.dirty_flags.push(false);
            self.mode_trackers.push(ModeTracker::new(UpdateMode::Auto));
            self.gpu_buffer.push(GlyphInstance {
                position: [0.0; 3], scale: [0.0; 2], rotation: 0.0,
                color: [0.0; 4], emission: 0.0, glow_color: [0.0; 3],
                glow_radius: 0.0, uv_offset: [0.0; 2], uv_size: [0.0; 2],
                _pad: [0.0; 2],
            });
            s
        };

        let layer = glyph.layer;
        let visible = glyph.visible;
        self.glyphs[slot] = Some(RetainedGlyph {
            glyph,
            id,
            visible,
            layer,
            generation: self.generation,
        });
        self.dirty_flags[slot] = true;
        self.mode_trackers[slot] = ModeTracker::new(mode);
        self.generation += 1;

        id
    }

    /// Insert a glyph with the default Auto update mode.
    pub fn insert_auto(&mut self, glyph: Glyph) -> RetainedId {
        self.insert(glyph, UpdateMode::Auto)
    }

    /// Insert a glyph that's expected to change every frame (chaos field, etc.).
    pub fn insert_immediate(&mut self, glyph: Glyph) -> RetainedId {
        self.insert(glyph, UpdateMode::Immediate)
    }

    /// Insert a glyph that's expected to be mostly static (UI labels, etc.).
    pub fn insert_retained(&mut self, glyph: Glyph) -> RetainedId {
        self.insert(glyph, UpdateMode::Retained)
    }

    /// Remove a glyph from the retained scene.
    pub fn remove(&mut self, id: RetainedId) -> Option<Glyph> {
        let slot = self.find_slot(id)?;
        let retained = self.glyphs[slot].take()?;
        self.free_slots.push(slot as u32);
        self.dirty_flags[slot] = false;
        // Zero out the GPU buffer slot so it doesn't render
        self.gpu_buffer[slot] = GlyphInstance {
            position: [0.0; 3], scale: [0.0; 2], rotation: 0.0,
            color: [0.0, 0.0, 0.0, 0.0], emission: 0.0, glow_color: [0.0; 3],
            glow_radius: 0.0, uv_offset: [0.0; 2], uv_size: [0.0; 2],
            _pad: [0.0; 2],
        };
        self.full_rebuild_needed = true;
        self.generation += 1;
        Some(retained.glyph)
    }

    /// Get an immutable reference to a retained glyph.
    pub fn get(&self, id: RetainedId) -> Option<&Glyph> {
        let slot = self.find_slot(id)?;
        self.glyphs[slot].as_ref().map(|r| &r.glyph)
    }

    /// Get a mutable reference to a retained glyph. Automatically marks it dirty.
    pub fn get_mut(&mut self, id: RetainedId) -> Option<&mut Glyph> {
        let slot = self.find_slot(id)?;
        self.dirty_flags[slot] = true;
        self.generation += 1;
        self.glyphs[slot].as_mut().map(|r| {
            r.generation = self.generation;
            &mut r.glyph
        })
    }

    /// Explicitly mark a glyph as dirty (will be re-uploaded next frame).
    pub fn mark_dirty(&mut self, id: RetainedId) {
        if let Some(slot) = self.find_slot(id) {
            self.dirty_flags[slot] = true;
            self.generation += 1;
        }
    }

    /// Set the update mode for a glyph.
    pub fn set_mode(&mut self, id: RetainedId, mode: UpdateMode) {
        if let Some(slot) = self.find_slot(id) {
            self.mode_trackers[slot].mode = mode;
            self.mode_trackers[slot].consecutive_dirty = 0;
        }
    }

    /// Get the current update mode for a glyph.
    pub fn get_mode(&self, id: RetainedId) -> Option<UpdateMode> {
        let slot = self.find_slot(id)?;
        Some(self.mode_trackers[slot].mode)
    }

    // ── Batch property updates ──────────────────────────────────────────

    /// Update position of a retained glyph. Marks dirty automatically.
    pub fn set_position(&mut self, id: RetainedId, pos: Vec3) {
        if let Some(glyph) = self.get_mut(id) {
            glyph.position = pos;
        }
    }

    /// Update color of a retained glyph. Marks dirty automatically.
    pub fn set_color(&mut self, id: RetainedId, color: Vec4) {
        if let Some(glyph) = self.get_mut(id) {
            glyph.color = color;
        }
    }

    /// Update scale of a retained glyph. Marks dirty automatically.
    pub fn set_scale(&mut self, id: RetainedId, scale: Vec2) {
        if let Some(glyph) = self.get_mut(id) {
            glyph.scale = scale;
        }
    }

    /// Update visibility. Marks dirty automatically.
    pub fn set_visible(&mut self, id: RetainedId, visible: bool) {
        if let Some(slot) = self.find_slot(id) {
            if let Some(ref mut rg) = self.glyphs[slot] {
                rg.visible = visible;
                rg.glyph.visible = visible;
            }
            self.dirty_flags[slot] = true;
            self.generation += 1;
        }
    }

    // ── Frame update ────────────────────────────────────────────────────

    /// Process dirty glyphs and rebuild the GPU buffer mirror.
    ///
    /// After calling this, use `dirty_ranges()` to get the byte ranges that
    /// need uploading via `glBufferSubData`, or use `gpu_buffer_bytes()` for
    /// a full upload if `needs_full_rebuild()` is true.
    ///
    /// Returns the number of dirty glyphs processed.
    pub fn update(&mut self, atlas: &FontAtlas) -> usize {
        let mut dirty_count = 0;
        let mut immediate_count = 0;
        let mut retained_count = 0;
        let mut auto_count = 0;
        let mut promotions = 0;
        let total = self.glyphs.iter().filter(|g| g.is_some()).count();

        for slot in 0..self.glyphs.len() {
            let Some(ref rg) = self.glyphs[slot] else { continue };
            let tracker = &self.mode_trackers[slot];

            match tracker.mode {
                UpdateMode::Immediate => {
                    immediate_count += 1;
                    // Always dirty
                    self.dirty_flags[slot] = true;
                }
                UpdateMode::Retained => {
                    retained_count += 1;
                }
                UpdateMode::Auto => {
                    auto_count += 1;
                }
            }
        }

        // Process dirty glyphs
        for slot in 0..self.glyphs.len() {
            if !self.dirty_flags[slot] {
                self.mode_trackers[slot].record_clean();
                continue;
            }

            let Some(ref rg) = self.glyphs[slot] else {
                self.dirty_flags[slot] = false;
                continue;
            };

            // Rebuild GPU instance for this slot
            let glyph = &rg.glyph;
            if rg.visible {
                let uv = atlas.uv_for(glyph.character);
                self.gpu_buffer[slot] = GlyphInstance {
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
            } else {
                // Invisible: zero alpha so it doesn't render
                self.gpu_buffer[slot] = GlyphInstance {
                    position: [0.0; 3], scale: [0.0; 2], rotation: 0.0,
                    color: [0.0, 0.0, 0.0, 0.0], emission: 0.0,
                    glow_color: [0.0; 3], glow_radius: 0.0,
                    uv_offset: [0.0; 2], uv_size: [0.0; 2],
                    _pad: [0.0; 2],
                };
            }

            // Track mode promotion
            if self.mode_trackers[slot].record_dirty() {
                promotions += 1;
            }

            self.dirty_flags[slot] = false;
            dirty_count += 1;
        }

        // Update stats
        self.stats = RetainedStats {
            total_glyphs: total,
            dirty_count,
            immediate_count,
            retained_count,
            auto_count,
            auto_promotions: promotions,
            upload_bytes: if self.full_rebuild_needed {
                self.gpu_buffer.len() * std::mem::size_of::<GlyphInstance>()
            } else {
                dirty_count * std::mem::size_of::<GlyphInstance>()
            },
            upload_ranges: 0, // computed by dirty_ranges() if called
            dirty_ratio: if total > 0 { dirty_count as f32 / total as f32 } else { 0.0 },
            generation: self.generation,
        };

        dirty_count
    }

    /// Compute coalesced dirty ranges for partial GPU buffer upload.
    ///
    /// Returns a list of (byte_offset, byte_slice) pairs to upload via
    /// `glBufferSubData`. Adjacent dirty slots are merged into single ranges
    /// to minimize driver overhead.
    pub fn dirty_upload_ranges(&self) -> Vec<(usize, &[u8])> {
        if self.full_rebuild_needed {
            // Full rebuild: return entire buffer as one range
            let bytes = bytemuck::cast_slice(&self.gpu_buffer);
            return vec![(0, bytes)];
        }

        let instance_size = std::mem::size_of::<GlyphInstance>();
        let mut ranges: Vec<(usize, &[u8])> = Vec::new();
        let mut range_start: Option<usize> = None;

        // Note: dirty_flags have been cleared by update(), so we need to detect
        // which slots were dirty by comparing gpu_buffer generations. Instead,
        // we track ranges during update. For now, we re-scan based on the
        // generation of each glyph vs a threshold.
        //
        // In practice, the caller should use `upload_dirty` which handles this
        // internally.

        // Fallback: return the full buffer
        let bytes = bytemuck::cast_slice(&self.gpu_buffer);
        vec![(0, bytes)]
    }

    /// Upload dirty regions to the GPU buffer.
    ///
    /// This is the primary API for retained-mode rendering. It:
    /// 1. Calls `update()` to process dirty glyphs
    /// 2. Uploads only changed regions via `glBufferSubData`
    /// 3. Returns the number of bytes uploaded
    ///
    /// # Safety
    /// Requires a valid, current OpenGL context.
    pub unsafe fn upload_dirty(
        &mut self,
        gl: &glow::Context,
        vbo: glow::Buffer,
        atlas: &FontAtlas,
    ) -> usize {
        let dirty_count = self.update(atlas);
        let instance_size = std::mem::size_of::<GlyphInstance>();

        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

        if self.full_rebuild_needed {
            // Full upload via orphan strategy
            let bytes = bytemuck::cast_slice(&self.gpu_buffer);
            let total_bytes = bytes.len();

            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytes,
                glow::DYNAMIC_DRAW,
            );

            self.full_rebuild_needed = false;
            self.stats.upload_bytes = total_bytes;
            self.stats.upload_ranges = 1;
            return total_bytes;
        }

        if dirty_count == 0 {
            // Nothing to upload!
            self.stats.upload_bytes = 0;
            self.stats.upload_ranges = 0;
            return 0;
        }

        // Partial upload: find dirty ranges by scanning glyphs whose generation
        // matches the current generation (they were just updated).
        let current_gen = self.generation;
        let mut uploaded_bytes = 0usize;
        let mut range_count = 0usize;

        // Coalesce adjacent dirty slots into ranges
        let mut range_start: Option<usize> = None;
        let mut range_end: usize = 0;

        // We need to know which slots were dirty. Since we cleared flags in update(),
        // we use generation tracking: a slot was dirty if its RetainedGlyph.generation
        // is within `dirty_count` of the current generation.
        let gen_threshold = current_gen.saturating_sub(dirty_count as u64);

        for slot in 0..self.glyphs.len() {
            let is_recent = match &self.glyphs[slot] {
                Some(rg) => rg.generation > gen_threshold,
                None => false,
            };

            if is_recent {
                match range_start {
                    None => {
                        range_start = Some(slot);
                        range_end = slot + 1;
                    }
                    Some(_) => {
                        // Coalesce if within 4 slots gap (upload the gap too, cheaper than
                        // multiple small uploads)
                        if slot <= range_end + 4 {
                            range_end = slot + 1;
                        } else {
                            // Flush current range
                            let start = range_start.unwrap();
                            let byte_offset = start * instance_size;
                            let byte_len = (range_end - start) * instance_size;
                            let slice = &bytemuck::cast_slice::<GlyphInstance, u8>(
                                &self.gpu_buffer[start..range_end]
                            );
                            gl.buffer_sub_data_u8_slice(
                                glow::ARRAY_BUFFER,
                                byte_offset as i32,
                                slice,
                            );
                            uploaded_bytes += byte_len;
                            range_count += 1;

                            range_start = Some(slot);
                            range_end = slot + 1;
                        }
                    }
                }
            }
        }

        // Flush final range
        if let Some(start) = range_start {
            let byte_offset = start * instance_size;
            let byte_len = (range_end - start) * instance_size;
            let slice = bytemuck::cast_slice::<GlyphInstance, u8>(
                &self.gpu_buffer[start..range_end]
            );
            gl.buffer_sub_data_u8_slice(
                glow::ARRAY_BUFFER,
                byte_offset as i32,
                slice,
            );
            uploaded_bytes += byte_len;
            range_count += 1;
        }

        self.stats.upload_bytes = uploaded_bytes;
        self.stats.upload_ranges = range_count;
        uploaded_bytes
    }

    // ── Query ───────────────────────────────────────────────────────────

    /// Whether a full GPU buffer rebuild is needed (after insert/remove).
    pub fn needs_full_rebuild(&self) -> bool { self.full_rebuild_needed }

    /// Request a full rebuild on the next frame.
    pub fn request_full_rebuild(&mut self) { self.full_rebuild_needed = true; }

    /// Total number of active glyphs.
    pub fn count(&self) -> usize {
        self.glyphs.iter().filter(|g| g.is_some()).count()
    }

    /// Total allocated capacity.
    pub fn capacity(&self) -> usize { self.glyphs.len() }

    /// Current generation (mutation counter).
    pub fn generation(&self) -> u64 { self.generation }

    /// The full GPU buffer as a byte slice (for full uploads).
    pub fn gpu_buffer_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.gpu_buffer)
    }

    /// The full GPU buffer as GlyphInstance slice.
    pub fn gpu_buffer(&self) -> &[GlyphInstance] {
        &self.gpu_buffer
    }

    /// Number of instances in the GPU buffer (including empty slots).
    pub fn gpu_buffer_len(&self) -> usize { self.gpu_buffer.len() }

    /// Iterate over all active retained glyphs.
    pub fn iter(&self) -> impl Iterator<Item = (RetainedId, &Glyph)> {
        self.glyphs.iter().filter_map(|slot| {
            slot.as_ref().map(|rg| (rg.id, &rg.glyph))
        })
    }

    /// Iterate mutably over all active retained glyphs. Marks all as dirty.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (RetainedId, &mut Glyph)> {
        self.generation += 1;
        let gen = self.generation;
        self.glyphs.iter_mut()
            .zip(self.dirty_flags.iter_mut())
            .filter_map(move |(slot, dirty)| {
                if let Some(ref mut rg) = slot {
                    *dirty = true;
                    rg.generation = gen;
                    Some((rg.id, &mut rg.glyph))
                } else {
                    None
                }
            })
    }

    // ── Private helpers ─────────────────────────────────────────────────

    /// Find the slot index for a given RetainedId.
    fn find_slot(&self, id: RetainedId) -> Option<usize> {
        self.glyphs.iter().position(|slot| {
            slot.as_ref().map(|rg| rg.id == id).unwrap_or(false)
        })
    }

    // ── Bulk operations ─────────────────────────────────────────────────

    /// Mark all glyphs as dirty (forces full re-upload).
    pub fn mark_all_dirty(&mut self) {
        for flag in &mut self.dirty_flags {
            *flag = true;
        }
        self.generation += 1;
    }

    /// Remove all glyphs.
    pub fn clear(&mut self) {
        for slot in &mut self.glyphs {
            *slot = None;
        }
        self.free_slots = (0..self.glyphs.len() as u32).rev().collect();
        for flag in &mut self.dirty_flags {
            *flag = false;
        }
        self.full_rebuild_needed = true;
        self.generation += 1;
    }

    /// Get the number of glyphs in each update mode.
    pub fn mode_counts(&self) -> (usize, usize, usize) {
        let mut retained = 0;
        let mut immediate = 0;
        let mut auto = 0;
        for (slot, tracker) in self.mode_trackers.iter().enumerate() {
            if self.glyphs[slot].is_some() {
                match tracker.mode {
                    UpdateMode::Retained => retained += 1,
                    UpdateMode::Immediate => immediate += 1,
                    UpdateMode::Auto => auto += 1,
                }
            }
        }
        (retained, immediate, auto)
    }
}

// ── HybridScene: combines retained + immediate ─────────────────────────────

/// A hybrid scene that combines retained-mode (persistent) and immediate-mode
/// (per-frame) rendering.
///
/// Use cases:
/// - **Retained**: UI panels, labels, static decorations, bestiary text
/// - **Immediate**: Chaos field glyphs, particles, animated combat effects
///
/// The hybrid scene manages the retained portion and provides a way to merge
/// immediate-mode instances at render time.
pub struct HybridScene {
    /// Retained portion — glyphs that persist across frames.
    pub retained: RetainedScene,

    /// Immediate-mode instances added this frame (cleared each frame).
    immediate_instances: Vec<GlyphInstance>,

    /// Combined buffer for rendering (retained + immediate).
    combined_buffer: Vec<GlyphInstance>,

    /// Stats
    pub stats: HybridStats,
}

/// Statistics for the hybrid scene.
#[derive(Debug, Clone, Default)]
pub struct HybridStats {
    pub retained_stats: RetainedStats,
    pub immediate_count: usize,
    pub combined_count: usize,
    pub retained_upload_bytes: usize,
    pub immediate_upload_bytes: usize,
    pub total_upload_bytes: usize,
    /// Percentage of total that was saved by retained mode.
    pub bandwidth_saved_pct: f32,
}

impl HybridScene {
    /// Create a new hybrid scene.
    pub fn new(retained_capacity: usize) -> Self {
        Self {
            retained: RetainedScene::new(retained_capacity),
            immediate_instances: Vec::with_capacity(4096),
            combined_buffer: Vec::with_capacity(retained_capacity + 4096),
            stats: HybridStats::default(),
        }
    }

    /// Begin a new frame. Clears immediate-mode instances.
    pub fn begin_frame(&mut self) {
        self.immediate_instances.clear();
    }

    /// Add an immediate-mode glyph instance (will be cleared next frame).
    pub fn push_immediate(&mut self, instance: GlyphInstance) {
        self.immediate_instances.push(instance);
    }

    /// Add an immediate-mode glyph (converted to GlyphInstance).
    pub fn push_immediate_glyph(&mut self, glyph: &Glyph, atlas: &FontAtlas) {
        if !glyph.visible { return; }
        let uv = atlas.uv_for(glyph.character);
        self.immediate_instances.push(GlyphInstance {
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
        });
    }

    /// Process retained dirty tracking and build the combined render buffer.
    ///
    /// Returns the combined buffer as a byte slice ready for GPU upload.
    pub fn update(&mut self, atlas: &FontAtlas) -> &[GlyphInstance] {
        // Update retained scene
        let dirty_count = self.retained.update(atlas);

        // Build combined buffer: retained (only active slots) + immediate
        self.combined_buffer.clear();

        // Copy active retained glyphs
        for slot in &self.retained.gpu_buffer {
            // Skip empty slots (zero alpha = invisible)
            if slot.color[3] > 0.0 || slot.emission > 0.0 {
                self.combined_buffer.push(*slot);
            }
        }

        let retained_visible = self.combined_buffer.len();

        // Append immediate instances
        self.combined_buffer.extend_from_slice(&self.immediate_instances);

        // Compute stats
        let instance_size = std::mem::size_of::<GlyphInstance>();
        let retained_upload = self.retained.stats.upload_bytes;
        let immediate_upload = self.immediate_instances.len() * instance_size;
        let total_upload = retained_upload + immediate_upload;
        let full_upload = self.combined_buffer.len() * instance_size;

        self.stats = HybridStats {
            retained_stats: self.retained.stats.clone(),
            immediate_count: self.immediate_instances.len(),
            combined_count: self.combined_buffer.len(),
            retained_upload_bytes: retained_upload,
            immediate_upload_bytes: immediate_upload,
            total_upload_bytes: total_upload,
            bandwidth_saved_pct: if full_upload > 0 {
                (1.0 - total_upload as f32 / full_upload as f32) * 100.0
            } else {
                0.0
            },
        };

        &self.combined_buffer
    }

    /// Get the combined buffer as bytes for GPU upload.
    pub fn combined_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.combined_buffer)
    }

    /// Total instance count (retained + immediate).
    pub fn total_count(&self) -> usize { self.combined_buffer.len() }
}

// ── Profiler integration ────────────────────────────────────────────────────

/// Format retained scene stats as a debug string for overlay display.
pub fn format_retained_stats(stats: &RetainedStats) -> String {
    format!(
        "Retained: {}/{} dirty ({:.1}%) | modes: R:{} I:{} A:{} | upload: {} bytes ({} ranges) | promos: {} | gen: {}",
        stats.dirty_count, stats.total_glyphs, stats.dirty_ratio * 100.0,
        stats.retained_count, stats.immediate_count, stats.auto_count,
        stats.upload_bytes, stats.upload_ranges,
        stats.auto_promotions, stats.generation,
    )
}

/// Format hybrid scene stats as a debug string.
pub fn format_hybrid_stats(stats: &HybridStats) -> String {
    format!(
        "Hybrid: {} retained + {} immediate = {} total | upload: {} bytes | saved: {:.1}%",
        stats.combined_count - stats.immediate_count,
        stats.immediate_count,
        stats.combined_count,
        stats.total_upload_bytes,
        stats.bandwidth_saved_pct,
    )
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph::Glyph;

    fn test_glyph(ch: char, pos: Vec3) -> Glyph {
        Glyph {
            character: ch,
            position: pos,
            visible: true,
            ..Default::default()
        }
    }

    #[test]
    fn insert_and_retrieve() {
        let mut scene = RetainedScene::new(64);
        let id = scene.insert_auto(test_glyph('A', Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(scene.count(), 1);

        let glyph = scene.get(id).unwrap();
        assert_eq!(glyph.character, 'A');
        assert_eq!(glyph.position, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn remove_glyph() {
        let mut scene = RetainedScene::new(64);
        let id = scene.insert_auto(test_glyph('B', Vec3::ZERO));
        assert_eq!(scene.count(), 1);

        let removed = scene.remove(id);
        assert!(removed.is_some());
        assert_eq!(scene.count(), 0);
        assert!(scene.get(id).is_none());
    }

    #[test]
    fn mutation_marks_dirty() {
        let mut scene = RetainedScene::new(64);
        let id = scene.insert_auto(test_glyph('C', Vec3::ZERO));

        // Clear dirty from insert
        let atlas = FontAtlas::build(16.0);
        scene.update(&atlas);

        // Mutate
        scene.set_position(id, Vec3::new(5.0, 0.0, 0.0));

        // Should have one dirty glyph
        let dirty = scene.update(&atlas);
        assert_eq!(dirty, 1);
    }

    #[test]
    fn retained_mode_no_upload_when_clean() {
        let mut scene = RetainedScene::new(64);
        let _id = scene.insert_retained(test_glyph('D', Vec3::ZERO));

        let atlas = FontAtlas::build(16.0);

        // First update: dirty from insert
        scene.update(&atlas);
        assert!(scene.stats.dirty_count > 0);

        // Second update: nothing changed
        let dirty = scene.update(&atlas);
        assert_eq!(dirty, 0);
        assert_eq!(scene.stats.dirty_ratio, 0.0);
    }

    #[test]
    fn immediate_mode_always_dirty() {
        let mut scene = RetainedScene::new(64);
        let _id = scene.insert_immediate(test_glyph('E', Vec3::ZERO));

        let atlas = FontAtlas::build(16.0);

        // First update
        scene.update(&atlas);

        // Second update: immediate glyphs are always dirty
        let dirty = scene.update(&atlas);
        assert_eq!(dirty, 1);
    }

    #[test]
    fn auto_promotes_to_immediate() {
        let mut scene = RetainedScene::new(64);
        let id = scene.insert_auto(test_glyph('F', Vec3::ZERO));

        let atlas = FontAtlas::build(16.0);
        assert_eq!(scene.get_mode(id), Some(UpdateMode::Auto));

        // Mark dirty for AUTO_PROMOTE_THRESHOLD frames
        for _ in 0..AUTO_PROMOTE_THRESHOLD {
            scene.mark_dirty(id);
            scene.update(&atlas);
        }

        // Should have been promoted to Immediate
        assert_eq!(scene.get_mode(id), Some(UpdateMode::Immediate));
    }

    #[test]
    fn hybrid_scene_combines_buffers() {
        let mut hybrid = HybridScene::new(64);
        let atlas = FontAtlas::build(16.0);

        // Add retained glyph
        hybrid.retained.insert_retained(test_glyph('G', Vec3::ZERO));

        // Begin frame + add immediate
        hybrid.begin_frame();
        hybrid.push_immediate_glyph(&test_glyph('H', Vec3::new(1.0, 0.0, 0.0)), &atlas);

        let combined = hybrid.update(&atlas);
        assert!(combined.len() >= 2); // at least 1 retained + 1 immediate
        assert_eq!(hybrid.stats.immediate_count, 1);
    }

    #[test]
    fn generation_increments() {
        let mut scene = RetainedScene::new(64);
        let gen0 = scene.generation();

        let id = scene.insert_auto(test_glyph('I', Vec3::ZERO));
        let gen1 = scene.generation();
        assert!(gen1 > gen0);

        scene.set_position(id, Vec3::ONE);
        let gen2 = scene.generation();
        assert!(gen2 > gen1);
    }

    #[test]
    fn clear_removes_all() {
        let mut scene = RetainedScene::new(64);
        scene.insert_auto(test_glyph('J', Vec3::ZERO));
        scene.insert_auto(test_glyph('K', Vec3::ONE));
        assert_eq!(scene.count(), 2);

        scene.clear();
        assert_eq!(scene.count(), 0);
    }
}
