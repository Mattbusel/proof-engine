//! Batched glyph rendering via instanced draw calls.
//!
//! All glyphs in the same layer and blend mode are batched into a single
//! instanced draw call for GPU efficiency.

/// Per-instance data uploaded to the GPU for each glyph.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphInstance {
    pub position: [f32; 3],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
    pub emission: f32,
    pub glow_color: [f32; 3],
    pub glow_radius: f32,
    pub uv_offset: [f32; 2],  // top-left UV of this glyph in the atlas
    pub uv_size: [f32; 2],    // size in UV space
    pub _pad: [f32; 2],
}

/// A CPU-side batch ready for upload.
pub struct GlyphBatch {
    pub instances: Vec<GlyphInstance>,
}

impl GlyphBatch {
    pub fn new() -> Self { Self { instances: Vec::new() } }
    pub fn clear(&mut self) { self.instances.clear(); }
    pub fn push(&mut self, inst: GlyphInstance) { self.instances.push(inst); }
    pub fn len(&self) -> usize { self.instances.len() }
    pub fn is_empty(&self) -> bool { self.instances.is_empty() }
}
