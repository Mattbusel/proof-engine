//! SDF-aware instanced glyph batch rendering.
//!
//! Adapts the existing `GlyphInstance` / `GlyphBatcher` approach for SDF rendering:
//!   - Same VBO/instance layout, but rendered with `sdf_glyph.frag`
//!   - Automatic smoothing calculation based on glyph screen-space size
//!   - Scale-dependent threshold for consistent edge quality at all sizes
//!   - Per-glyph SDF effects (outline, shadow, glow, bold, wave, glitch)

use glam::{Vec2, Vec3, Vec4, Mat4};
use std::collections::HashMap;

use super::batch::{GlyphInstance, GlyphBatch, BatchKey, BlendMode, RenderLayerOrd};
use super::sdf_atlas::SdfAtlas;
use super::sdf_generator::SdfGlyphMetric;
use super::RenderLayer;

// ── SDF Instance (extended) ─────────────────────────────────────────────────

/// Per-instance SDF-specific data that supplements the base GlyphInstance.
///
/// This is kept CPU-side and used to set shader uniforms per-batch or to
/// pack into an extended instance buffer for per-glyph effects.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SdfInstanceExtra {
    /// SDF threshold: 0.5 = normal, 0.45 = bold, etc.
    pub threshold: f32,
    /// Smoothing factor (computed from screen-space size).
    pub smoothing: f32,
    /// Outline parameters: [enabled, width, 0, 0].
    pub outline_params: [f32; 4],
    /// Outline color RGBA.
    pub outline_color: [f32; 4],
    /// Shadow parameters: [enabled, softness, uv_offset_x, uv_offset_y].
    pub shadow_params: [f32; 4],
    /// Shadow color RGBA.
    pub shadow_color: [f32; 4],
    /// Glow parameters: [enabled, radius, 0, 0].
    pub glow_params: [f32; 4],
    /// Glow color RGBA.
    pub glow_color: [f32; 4],
    /// UV distortion: [wave_amp, wave_freq, shake, glitch].
    pub distortion: [f32; 4],
}

impl Default for SdfInstanceExtra {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            smoothing: 0.05,
            outline_params: [0.0; 4],
            outline_color: [0.0, 0.0, 0.0, 1.0],
            shadow_params: [0.0; 4],
            shadow_color: [0.0, 0.0, 0.0, 0.6],
            glow_params: [0.0; 4],
            glow_color: [1.0, 1.0, 1.0, 0.5],
            distortion: [0.0; 4],
        }
    }
}

// ── SDF Batch ───────────────────────────────────────────────────────────────

/// A batch of SDF glyph instances ready for rendering.
pub struct SdfGlyphBatch {
    pub key: BatchKey,
    pub base_instances: Vec<GlyphInstance>,
    pub sdf_extras: Vec<SdfInstanceExtra>,
}

impl SdfGlyphBatch {
    pub fn new(key: BatchKey) -> Self {
        Self {
            key,
            base_instances: Vec::with_capacity(64),
            sdf_extras: Vec::with_capacity(64),
        }
    }

    pub fn clear(&mut self) {
        self.base_instances.clear();
        self.sdf_extras.clear();
    }

    pub fn push(&mut self, base: GlyphInstance, extra: SdfInstanceExtra) {
        self.base_instances.push(base);
        self.sdf_extras.push(extra);
    }

    pub fn len(&self) -> usize {
        self.base_instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.base_instances.is_empty()
    }

    /// Raw byte slice of base instances for GPU upload.
    pub fn base_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.base_instances)
    }

    /// Raw byte slice of SDF extras for GPU upload.
    pub fn extra_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.sdf_extras)
    }

    /// Sort instances back-to-front by Z.
    pub fn sort_back_to_front(&mut self) {
        // Create index array, sort by Z, then reorder both arrays.
        let mut indices: Vec<usize> = (0..self.base_instances.len()).collect();
        indices.sort_by(|&a, &b| {
            self.base_instances[b].position[2]
                .partial_cmp(&self.base_instances[a].position[2])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let old_base = self.base_instances.clone();
        let old_extra = self.sdf_extras.clone();
        for (new_idx, &old_idx) in indices.iter().enumerate() {
            self.base_instances[new_idx] = old_base[old_idx];
            self.sdf_extras[new_idx] = old_extra[old_idx];
        }
    }
}

// ── SDF Batcher ─────────────────────────────────────────────────────────────

/// Pending SDF glyph submission.
pub struct PendingSdfGlyph {
    pub key: BatchKey,
    pub base: GlyphInstance,
    pub extra: SdfInstanceExtra,
    pub depth: f32,
}

/// Builds and sorts SDF glyph batches each frame.
pub struct SdfGlyphBatcher {
    pending: Vec<PendingSdfGlyph>,
    batches: Vec<SdfGlyphBatch>,
    stats: SdfBatchStats,
}

#[derive(Default, Debug, Clone)]
pub struct SdfBatchStats {
    pub total_glyphs: usize,
    pub batch_count: usize,
    pub outlined_glyphs: usize,
    pub shadowed_glyphs: usize,
}

impl SdfGlyphBatcher {
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(4096),
            batches: Vec::with_capacity(16),
            stats: SdfBatchStats::default(),
        }
    }

    pub fn begin(&mut self) {
        self.pending.clear();
        self.stats = SdfBatchStats::default();
    }

    pub fn push(&mut self, key: BatchKey, base: GlyphInstance, extra: SdfInstanceExtra, depth: f32) {
        self.pending.push(PendingSdfGlyph { key, base, extra, depth });
    }

    pub fn push_simple(
        &mut self,
        layer: RenderLayer,
        base: GlyphInstance,
        smoothing: f32,
        depth: f32,
    ) {
        let key = BatchKey::default_for_layer(layer);
        let extra = SdfInstanceExtra {
            smoothing,
            ..SdfInstanceExtra::default()
        };
        self.push(key, base, extra, depth);
    }

    pub fn finish(&mut self) {
        self.stats.total_glyphs = self.pending.len();

        // Sort by batch key then depth.
        self.pending.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then(b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal))
        });

        self.batches.clear();
        let mut current_key: Option<BatchKey> = None;

        for item in &self.pending {
            if item.extra.outline_params[0] > 0.0 {
                self.stats.outlined_glyphs += 1;
            }
            if item.extra.shadow_params[0] > 0.0 {
                self.stats.shadowed_glyphs += 1;
            }

            if current_key != Some(item.key) {
                self.batches.push(SdfGlyphBatch::new(item.key));
                current_key = Some(item.key);
            }

            let batch = self.batches.last_mut().unwrap();
            batch.base_instances.push(item.base);
            batch.sdf_extras.push(item.extra);
        }

        self.stats.batch_count = self.batches.len();
    }

    pub fn batches(&self) -> &[SdfGlyphBatch] {
        &self.batches
    }

    pub fn stats(&self) -> &SdfBatchStats {
        &self.stats
    }

    pub fn instance_count(&self) -> usize {
        self.batches.iter().map(|b| b.len()).sum()
    }
}

impl Default for SdfGlyphBatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ── SDF Text Layout Helper ──────────────────────────────────────────────────

/// Lay out a string of text using the SDF atlas metrics and return GlyphInstance
/// + SdfInstanceExtra pairs ready for batching.
///
/// `position` is the top-left of the text block in world space.
/// `scale` is the desired font size in world units.
pub fn layout_sdf_text(
    atlas: &SdfAtlas,
    text: &str,
    position: Vec3,
    scale: f32,
    color: Vec4,
    emission: f32,
    screen_px_per_unit: f32,
) -> Vec<(GlyphInstance, SdfInstanceExtra)> {
    let scale_factor = scale / atlas.font_size_px;
    let smoothing = atlas.compute_smoothing(screen_px_per_unit, scale);
    let mut result = Vec::with_capacity(text.len());
    let mut cursor_x = position.x;

    for ch in text.chars() {
        if ch == ' ' {
            let metric = atlas.metric_for(ch);
            cursor_x += metric.advance * scale_factor;
            continue;
        }

        let metric = atlas.metric_for(ch);
        let uv = metric.uv_rect;

        let glyph_pos = Vec3::new(
            cursor_x + metric.bearing.x * scale_factor,
            position.y - metric.bearing.y * scale_factor,
            position.z,
        );

        let glyph_scale = Vec2::new(
            metric.size.x * scale_factor,
            metric.size.y * scale_factor,
        );

        let base = GlyphInstance {
            position: glyph_pos.to_array(),
            scale: [glyph_scale.x, glyph_scale.y],
            rotation: 0.0,
            color: color.to_array(),
            emission,
            glow_color: [color.x, color.y, color.z],
            glow_radius: 0.0,
            uv_offset: [uv[0], uv[1]],
            uv_size: [uv[2] - uv[0], uv[3] - uv[1]],
            _pad: [0.0; 2],
        };

        let extra = SdfInstanceExtra {
            threshold: 0.5,
            smoothing,
            ..SdfInstanceExtra::default()
        };

        result.push((base, extra));
        cursor_x += metric.advance * scale_factor;
    }

    result
}

/// Lay out text with SDF effects applied.
pub fn layout_sdf_text_with_effects(
    atlas: &SdfAtlas,
    text: &str,
    position: Vec3,
    scale: f32,
    color: Vec4,
    emission: f32,
    screen_px_per_unit: f32,
    effects: &super::sdf_atlas::SdfEffects,
) -> Vec<(GlyphInstance, SdfInstanceExtra)> {
    let mut result = layout_sdf_text(atlas, text, position, scale, color, emission, screen_px_per_unit);

    for (_, extra) in &mut result {
        if effects.bold {
            extra.threshold = 0.45;
        }
        if effects.outline {
            extra.outline_params = [1.0, effects.outline_width, 0.0, 0.0];
            extra.outline_color = effects.outline_color.to_array();
        }
        if effects.shadow {
            let uv_off = atlas.shadow_uv_offset(effects.shadow_offset, scale);
            extra.shadow_params = [1.0, effects.shadow_softness, uv_off.x, uv_off.y];
            extra.shadow_color = effects.shadow_color.to_array();
        }
        if effects.glow {
            extra.glow_params = [1.0, effects.glow_radius, 0.0, 0.0];
            extra.glow_color = effects.glow_color.to_array();
        }
        extra.distortion = [
            effects.wave_amplitude,
            effects.wave_frequency,
            effects.shake_amount,
            effects.glitch_intensity,
        ];
    }

    result
}

// ── Uniform block for batch-level SDF params ────────────────────────────────

/// Uniforms that are set once per SDF draw call (not per-instance).
#[derive(Clone, Debug)]
pub struct SdfBatchUniforms {
    /// View-projection matrix.
    pub view_proj: Mat4,
    /// Default threshold (can be overridden per-instance).
    pub threshold: f32,
    /// Default smoothing (can be overridden per-instance).
    pub smoothing: f32,
    /// Current time for animated effects.
    pub time: f32,
    /// Screen dimensions for pixel-space calculations.
    pub screen_size: Vec2,
}

impl Default for SdfBatchUniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY,
            threshold: 0.5,
            smoothing: 0.05,
            time: 0.0,
            screen_size: Vec2::new(1280.0, 800.0),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sdf_instance(z: f32) -> (GlyphInstance, SdfInstanceExtra) {
        let base = GlyphInstance::simple(Vec3::new(0.0, 0.0, z), Vec2::ZERO, Vec2::new(0.1, 0.1));
        let extra = SdfInstanceExtra::default();
        (base, extra)
    }

    #[test]
    fn sdf_batcher_groups_by_key() {
        let mut batcher = SdfGlyphBatcher::new();
        batcher.begin();

        let world_key = BatchKey::new(RenderLayer::World, BlendMode::Alpha, 0);
        let ui_key = BatchKey::new(RenderLayer::UI, BlendMode::Alpha, 0);

        let (b1, e1) = make_sdf_instance(0.0);
        let (b2, e2) = make_sdf_instance(1.0);
        let (b3, e3) = make_sdf_instance(0.0);

        batcher.push(world_key, b1, e1, 0.0);
        batcher.push(world_key, b2, e2, 1.0);
        batcher.push(ui_key, b3, e3, 0.0);

        batcher.finish();

        assert_eq!(batcher.batches().len(), 2);
        assert_eq!(batcher.instance_count(), 3);
    }

    #[test]
    fn sdf_batch_sort_back_to_front() {
        let key = BatchKey::new(RenderLayer::World, BlendMode::Alpha, 0);
        let mut batch = SdfGlyphBatch::new(key);
        let (b1, e1) = make_sdf_instance(0.0);
        let (b2, e2) = make_sdf_instance(5.0);
        let (b3, e3) = make_sdf_instance(2.0);
        batch.push(b1, e1);
        batch.push(b2, e2);
        batch.push(b3, e3);
        batch.sort_back_to_front();

        let zs: Vec<f32> = batch.base_instances.iter().map(|i| i.position[2]).collect();
        assert!(zs[0] >= zs[1] && zs[1] >= zs[2]);
    }

    #[test]
    fn sdf_instance_extra_size() {
        // Verify the struct is Pod-safe and has expected layout.
        let extra = SdfInstanceExtra::default();
        let bytes: &[u8] = bytemuck::bytes_of(&extra);
        assert_eq!(bytes.len(), std::mem::size_of::<SdfInstanceExtra>());
    }
}
