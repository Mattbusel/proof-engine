//! GLSL shader sources, embedded at compile time.

pub const FULLSCREEN_VERT: &str = include_str!("fullscreen.vert");
pub const GLYPH_VERT: &str     = include_str!("glyph.vert");
pub const GLYPH_FRAG: &str     = include_str!("glyph.frag");
pub const BLOOM_FRAG: &str     = include_str!("bloom.frag");
pub const COMPOSITE_FRAG: &str = include_str!("composite.frag");
pub const SDF_GLYPH_VERT: &str = include_str!("sdf_glyph.vert");
pub const SDF_GLYPH_FRAG: &str = include_str!("sdf_glyph.frag");
pub const PARTICLE_UPDATE_COMP: &str = include_str!("particle_update.comp");
