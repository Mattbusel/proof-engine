//! Batched glyph rendering — layer-sorted, blend-mode-separated instanced draw calls.
//!
//! The renderer needs to:
//!  1. Sort all active glyphs by (RenderLayer, blend_mode) to minimize state changes
//!  2. Pack each batch into a `GlyphInstance` array for the GPU
//!  3. Issue one `draw_arrays_instanced` call per batch
//!
//! # Batch key
//!
//! `(RenderLayer, BlendMode)` — changes in either require a new draw call.
//! Within each batch, instances are further sorted by Z (back-to-front) for
//! transparent geometry.

use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;
use crate::glyph::RenderLayer;

// ── GlyphInstance ─────────────────────────────────────────────────────────────

/// Per-instance GPU data. Must match the vertex shader layout exactly.
///
/// Layout (84 bytes total, 21 × f32):
///   position   : vec3  (offset  0)
///   scale      : vec2  (offset 12)
///   rotation   : f32   (offset 20)
///   color      : vec4  (offset 24)
///   emission   : f32   (offset 40)
///   glow_color : vec3  (offset 44)
///   glow_radius: f32   (offset 56)
///   uv_offset  : vec2  (offset 60)
///   uv_size    : vec2  (offset 68)
///   _pad       : vec2  (offset 76)  — aligns to 84 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphInstance {
    pub position:    [f32; 3],
    pub scale:       [f32; 2],
    pub rotation:    f32,
    pub color:       [f32; 4],
    pub emission:    f32,
    pub glow_color:  [f32; 3],
    pub glow_radius: f32,
    pub uv_offset:   [f32; 2],
    pub uv_size:     [f32; 2],
    pub _pad:        [f32; 2],
}

// Note: GlyphInstance must be exactly 84 bytes — verified in tests below.

// ── Blend mode ────────────────────────────────────────────────────────────────

/// GPU blend mode for a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BlendMode {
    /// Standard alpha blending: src × src_alpha + dst × (1 - src_alpha).
    Alpha,
    /// Additive: src × src_alpha + dst × 1.  Great for glows and bloom.
    Additive,
    /// Multiplicative: dst × src. Darkens/tints.
    Multiply,
    /// Screen: 1 - (1 - src)(1 - dst). Brightens.
    Screen,
}

// ── Batch key ─────────────────────────────────────────────────────────────────

/// Determines which draw call a glyph belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub layer: RenderLayerOrd,
    pub blend: BlendMode,
    /// Texture atlas page (0 for single-atlas setups).
    pub atlas_page: u8,
}

/// Ordered wrapper for RenderLayer (to allow sorting by layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderLayerOrd(pub u8);

impl RenderLayerOrd {
    pub fn from_layer(layer: RenderLayer) -> Self {
        Self(match layer {
            RenderLayer::Background => 0,
            RenderLayer::World      => 1,
            RenderLayer::Entity     => 2,
            RenderLayer::Particle   => 3,
            RenderLayer::Overlay     => 4,
            RenderLayer::UI         => 5,
        })
    }
}

impl BatchKey {
    pub fn new(layer: RenderLayer, blend: BlendMode, atlas_page: u8) -> Self {
        Self { layer: RenderLayerOrd::from_layer(layer), blend, atlas_page }
    }

    pub fn default_for_layer(layer: RenderLayer) -> Self {
        let blend = match layer {
            RenderLayer::Overlay | RenderLayer::Particle => BlendMode::Additive,
            _ => BlendMode::Alpha,
        };
        Self::new(layer, blend, 0)
    }
}

impl PartialOrd for BatchKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BatchKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.layer.cmp(&other.layer)
            .then(self.blend.cmp(&other.blend))
            .then(self.atlas_page.cmp(&other.atlas_page))
    }
}

// ── CPU-side batch ────────────────────────────────────────────────────────────

/// A CPU-side instance buffer for one draw call.
#[derive(Debug)]
pub struct GlyphBatch {
    pub key:       BatchKey,
    pub instances: Vec<GlyphInstance>,
}

impl GlyphBatch {
    pub fn new(key: BatchKey) -> Self {
        Self { key, instances: Vec::with_capacity(64) }
    }

    pub fn clear(&mut self) { self.instances.clear(); }

    pub fn push(&mut self, inst: GlyphInstance) { self.instances.push(inst); }

    pub fn len(&self)      -> usize { self.instances.len() }
    pub fn is_empty(&self) -> bool  { self.instances.is_empty() }

    /// Raw byte slice for GPU upload.
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.instances)
    }

    /// Sort instances back-to-front by Z position (for alpha blending correctness).
    pub fn sort_back_to_front(&mut self) {
        self.instances.sort_by(|a, b| {
            b.position[2].partial_cmp(&a.position[2]).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Sort instances front-to-back by Z (for depth occlusion passes).
    pub fn sort_front_to_back(&mut self) {
        self.instances.sort_by(|a, b| {
            a.position[2].partial_cmp(&b.position[2]).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

// ── Batch sorter / builder ────────────────────────────────────────────────────

/// Pending item — a glyph to be assigned to a batch.
pub struct PendingGlyph {
    pub key:      BatchKey,
    pub instance: GlyphInstance,
    pub depth:    f32,  // for Z sorting within a layer
}

/// Builds and sorts batches from a flat list of glyphs each frame.
///
/// Usage:
/// ```text
/// batcher.begin();
/// for glyph in glyphs { batcher.push(glyph, key, instance); }
/// batcher.finish();
/// for batch in batcher.batches() { gpu.draw(batch); }
/// ```
pub struct GlyphBatcher {
    pending:  Vec<PendingGlyph>,
    batches:  Vec<GlyphBatch>,
    stats:    BatchStats,
}

/// Statistics from the last `finish()` call.
#[derive(Default, Debug, Clone)]
pub struct BatchStats {
    pub total_glyphs:  usize,
    pub batch_count:   usize,
    pub alpha_glyphs:  usize,
    pub additive_glyphs: usize,
}

impl GlyphBatcher {
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(4096),
            batches: Vec::with_capacity(16),
            stats:   BatchStats::default(),
        }
    }

    /// Clear pending list and stats. Call at the start of each frame.
    pub fn begin(&mut self) {
        self.pending.clear();
        self.stats = BatchStats::default();
    }

    /// Submit a glyph for batching.
    pub fn push(&mut self, key: BatchKey, instance: GlyphInstance, depth: f32) {
        self.pending.push(PendingGlyph { key, instance, depth });
    }

    /// Submit a glyph using default batch key for its layer.
    pub fn push_default(&mut self, layer: RenderLayer, instance: GlyphInstance, depth: f32) {
        let key = BatchKey::default_for_layer(layer);
        self.push(key, instance, depth);
    }

    /// Sort and group all pending glyphs into batches. Call after all pushes.
    pub fn finish(&mut self) {
        self.stats.total_glyphs = self.pending.len();

        // Sort pending by batch key (layer, blend, atlas) then depth
        self.pending.sort_by(|a, b| {
            a.key.cmp(&b.key)
                .then(b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Group into batches
        self.batches.clear();
        let mut current_key: Option<BatchKey> = None;

        for item in &self.pending {
            match &item.blend_type() {
                BlendMode::Alpha | BlendMode::Multiply | BlendMode::Screen => {
                    self.stats.alpha_glyphs += 1;
                }
                BlendMode::Additive => {
                    self.stats.additive_glyphs += 1;
                }
            }

            if current_key != Some(item.key) {
                self.batches.push(GlyphBatch::new(item.key));
                current_key = Some(item.key);
            }

            self.batches.last_mut().unwrap().instances.push(item.instance);
        }

        self.stats.batch_count = self.batches.len();
    }

    /// Iterate over completed batches in draw order.
    pub fn batches(&self) -> &[GlyphBatch] {
        &self.batches
    }

    /// Statistics from the last `finish()` call.
    pub fn stats(&self) -> &BatchStats { &self.stats }

    /// Total instance count across all batches.
    pub fn instance_count(&self) -> usize {
        self.batches.iter().map(|b| b.len()).sum()
    }
}

impl PendingGlyph {
    fn blend_type(&self) -> BlendMode { self.key.blend }
}

impl Default for GlyphBatcher {
    fn default() -> Self { Self::new() }
}

// ── Convenience constructors for GlyphInstance ────────────────────────────────

impl GlyphInstance {
    /// Build a GlyphInstance from common parameters.
    pub fn build(
        position:    Vec3,
        scale:       Vec2,
        rotation:    f32,
        color:       Vec4,
        emission:    f32,
        glow_color:  Vec3,
        glow_radius: f32,
        uv_offset:   Vec2,
        uv_size:     Vec2,
    ) -> Self {
        Self {
            position:    position.into(),
            scale:       scale.into(),
            rotation,
            color:       color.into(),
            emission,
            glow_color:  glow_color.into(),
            glow_radius,
            uv_offset:   uv_offset.into(),
            uv_size:     uv_size.into(),
            _pad:        [0.0; 2],
        }
    }

    /// A simple opaque white glyph at a position (useful for testing).
    pub fn simple(position: Vec3, uv_offset: Vec2, uv_size: Vec2) -> Self {
        Self::build(
            position,
            Vec2::ONE,
            0.0,
            Vec4::ONE,
            0.0,
            Vec3::ZERO,
            0.0,
            uv_offset,
            uv_size,
        )
    }

    /// A glowing glyph.
    pub fn glowing(position: Vec3, color: Vec4, emission: f32, glow_radius: f32,
                   uv_offset: Vec2, uv_size: Vec2) -> Self {
        Self::build(
            position,
            Vec2::ONE,
            0.0,
            color,
            emission,
            Vec3::new(color.x, color.y, color.z),
            glow_radius,
            uv_offset,
            uv_size,
        )
    }
}

// ── Multi-frame atlas upload tracking ─────────────────────────────────────────

/// Tracks which atlas pages have been uploaded this frame (avoids redundant uploads).
#[derive(Default)]
pub struct AtlasUploadTracker {
    dirty_pages: std::collections::HashSet<u8>,
}

impl AtlasUploadTracker {
    pub fn mark_dirty(&mut self, page: u8) { self.dirty_pages.insert(page); }
    pub fn is_dirty(&self, page: u8) -> bool { self.dirty_pages.contains(&page) }
    pub fn clear(&mut self) { self.dirty_pages.clear(); }
    pub fn dirty_pages(&self) -> impl Iterator<Item = u8> + '_ {
        self.dirty_pages.iter().copied()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance(z: f32) -> GlyphInstance {
        GlyphInstance::simple(Vec3::new(0.0, 0.0, z), Vec2::ZERO, Vec2::new(0.1, 0.1))
    }

    #[test]
    fn batcher_groups_by_key() {
        let mut batcher = GlyphBatcher::new();
        batcher.begin();

        let world_key = BatchKey::new(RenderLayer::World, BlendMode::Alpha, 0);
        let ui_key    = BatchKey::new(RenderLayer::UI,    BlendMode::Alpha, 0);

        batcher.push(world_key, make_instance(0.0), 0.0);
        batcher.push(world_key, make_instance(1.0), 1.0);
        batcher.push(ui_key,    make_instance(0.0), 0.0);

        batcher.finish();

        assert_eq!(batcher.batches().len(), 2, "Expected 2 batches");
        assert_eq!(batcher.instance_count(), 3);
    }

    #[test]
    fn batches_sorted_by_layer() {
        let mut batcher = GlyphBatcher::new();
        batcher.begin();

        batcher.push_default(RenderLayer::UI,    make_instance(0.0), 0.0);
        batcher.push_default(RenderLayer::World, make_instance(0.0), 0.0);

        batcher.finish();

        let batches = batcher.batches();
        // World (layer=1) should come before UI (layer=5)
        assert!(batches[0].key.layer < batches[1].key.layer);
    }

    #[test]
    fn stats_correct() {
        let mut batcher = GlyphBatcher::new();
        batcher.begin();
        batcher.push_default(RenderLayer::World,    make_instance(0.0), 0.0);
        batcher.push_default(RenderLayer::World,    make_instance(1.0), 1.0);
        batcher.push_default(RenderLayer::Particle, make_instance(0.0), 0.0);
        batcher.finish();

        assert_eq!(batcher.stats().total_glyphs, 3);
    }

    #[test]
    fn glyph_instance_size_is_84() {
        assert_eq!(std::mem::size_of::<GlyphInstance>(), 84);
    }

    #[test]
    fn sort_back_to_front_orders_by_descending_z() {
        let key = BatchKey::new(RenderLayer::World, BlendMode::Alpha, 0);
        let mut batch = GlyphBatch::new(key);
        batch.push(make_instance(0.0));
        batch.push(make_instance(5.0));
        batch.push(make_instance(2.0));
        batch.sort_back_to_front();

        let zs: Vec<f32> = batch.instances.iter().map(|i| i.position[2]).collect();
        assert!(zs[0] >= zs[1] && zs[1] >= zs[2]);
    }
}
