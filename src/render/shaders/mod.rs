//\! GLSL shader sources, embedded at compile time.

pub const GLYPH_VERT: &str = include_str\!("glyph.vert");
pub const GLYPH_FRAG: &str = include_str\!("glyph.frag");
pub const BLOOM_FRAG: &str = include_str\!("bloom.frag");
pub const COMPOSITE_FRAG: &str = include_str\!("composite.frag");
