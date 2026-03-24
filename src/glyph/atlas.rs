//! Font texture atlas.
//!
//! Uses ab_glyph to rasterize a system TTF into a single R8 texture.
//! Falls back to a placeholder if no font is found.

use std::collections::HashMap;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};

/// UV rectangle for one glyph in the atlas.
#[derive(Copy, Clone, Debug)]
pub struct GlyphUv {
    pub u0: f32, pub v0: f32,
    pub u1: f32, pub v1: f32,
}
impl GlyphUv {
    /// Returns UV offset. U is flipped (starts at u1) so that the glyph reads
    /// correctly when rendered with look_at_rh from +Z (which mirrors world X).
    pub fn offset(&self) -> [f32; 2] { [self.u1, self.v0] }
    pub fn size(&self)   -> [f32; 2] { [self.u0 - self.u1, self.v1 - self.v0] }
}

/// The characters the engine can render.
pub const ATLAS_CHARS: &str = concat!(
    " !\"#$%&'()*+,-./0123456789:;<=>?",
    "@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_",
    "`abcdefghijklmnopqrstuvwxyz{|}~",
    "░▒▓█▄▀▌▐■□▪▫",
    "╔╗╚╝║═╠╣╦╩╬",
    "←→↑↓★✦✧☆",
    "∞∑∫∂∇∆∏λαβγδεζηθπσφψω",
    "☠☢☣☯✓✗⊕⊗⊙◆◇◈◉",
);

/// Complete font atlas ready to upload to the GPU.
pub struct FontAtlas {
    pub width:  u32,
    pub height: u32,
    /// R8 pixel data (one byte per pixel, 0-255 coverage).
    pub pixels: Vec<u8>,
    pub uvs:    HashMap<char, GlyphUv>,
    pub cell_w: u32,
    pub cell_h: u32,
}

fn load_system_font() -> Option<FontVec> {
    let paths: &[&str] = &[
        r"C:\Windows\Fonts\consola.ttf",
        r"C:\Windows\Fonts\cour.ttf",
        r"C:\Windows\Fonts\lucon.ttf",
        "/System/Library/Fonts/Menlo.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    ];
    for p in paths {
        if let Ok(data) = std::fs::read(p) {
            if let Ok(f) = FontVec::try_from_vec(data) {
                log::info!("FontAtlas: loaded '{}'", p);
                return Some(f);
            }
        }
    }
    None
}

impl FontAtlas {
    pub fn build(px_size: f32) -> Self {
        let chars: Vec<char> = ATLAS_CHARS.chars().collect();
        if let Some(font) = load_system_font() {
            return Self::from_ttf(&font, px_size, &chars);
        }
        log::warn!("FontAtlas: no system font found, using fallback");
        Self::fallback(&chars, px_size as u32)
    }

    fn from_ttf(font: &FontVec, px_size: f32, chars: &[char]) -> Self {
        let scale   = PxScale::from(px_size);
        let scaled  = font.as_scaled(scale);
        let ascent  = scaled.ascent();
        let descent = scaled.descent();
        let cell_h  = (ascent - descent).ceil() as u32 + 4;
        let cell_w  = chars.iter().filter_map(|ch| {
            let id = font.glyph_id(*ch);
            if id.0 == 0 { return None; }
            Some(scaled.h_advance(id).ceil() as u32)
        }).max().unwrap_or(px_size as u32) + 4;

        let cols = 32u32;
        let rows = (chars.len() as u32 + cols - 1) / cols;
        let w = cols * cell_w;
        let h = rows * cell_h;
        let mut pixels = vec![0u8; (w * h) as usize];
        let mut uvs    = HashMap::new();
        let baseline_offset = ascent.ceil() as i32 + 1;

        for (i, ch) in chars.iter().enumerate() {
            let col = (i as u32 % cols) as i32;
            let row = (i as u32 / cols) as i32;
            let cx  = col * cell_w as i32;
            let cy  = row * cell_h as i32;

            let glyph = font.glyph_id(*ch).with_scale_and_position(
                scale,
                ab_glyph::point(cx as f32 + 1.0, cy as f32 + baseline_offset as f32),
            );
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|x, y, v| {
                    let px = bounds.min.x as i32 + x as i32;
                    let py = bounds.min.y as i32 + y as i32;
                    if px >= 0 && py >= 0 {
                        let idx = (py as u32 * w + px as u32) as usize;
                        if idx < pixels.len() {
                            pixels[idx] = (v * 255.0).min(255.0) as u8;
                        }
                    }
                });
            }
            uvs.insert(*ch, GlyphUv {
                u0: cx as f32 / w as f32,
                v0: cy as f32 / h as f32,
                u1: (cx + cell_w as i32) as f32 / w as f32,
                v1: (cy + cell_h as i32) as f32 / h as f32,
            });
        }
        Self { width: w, height: h, pixels, uvs, cell_w, cell_h }
    }

    fn fallback(chars: &[char], px: u32) -> Self {
        let cw = (px / 2).max(8);
        let ch = px.max(12);
        let cols = 32u32;
        let rows = (chars.len() as u32 + cols - 1) / cols;
        let w = cols * cw;
        let h = rows * ch;
        let mut pixels = vec![0u8; (w * h) as usize];
        let mut uvs = HashMap::new();
        for (i, c) in chars.iter().enumerate() {
            let col = (i as u32 % cols) as i32;
            let row = (i as u32 / cols) as i32;
            let cx = col * cw as i32;
            let cy = row * ch as i32;
            for py in 1..ch as i32 - 1 {
                for px in 1..cw as i32 - 1 {
                    let idx = ((cy + py) as u32 * w + (cx + px) as u32) as usize;
                    if idx < pixels.len() { pixels[idx] = 180; }
                }
            }
            uvs.insert(*c, GlyphUv {
                u0: cx as f32 / w as f32,
                v0: cy as f32 / h as f32,
                u1: (cx + cw as i32) as f32 / w as f32,
                v1: (cy + ch as i32) as f32 / h as f32,
            });
        }
        Self { width: w, height: h, pixels, uvs, cell_w: cw, cell_h: ch }
    }

    pub fn uv_for(&self, ch: char) -> GlyphUv {
        self.uvs.get(&ch)
            .or_else(|| self.uvs.get(&'?'))
            .copied()
            .unwrap_or(GlyphUv { u0: 0.0, v0: 0.0, u1: 0.01, v1: 0.01 })
    }
}
