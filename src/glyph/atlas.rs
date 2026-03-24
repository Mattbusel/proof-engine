//! Font texture atlas generation.
//!
//! Renders all ASCII characters (32-126) plus mathematical symbols into a
//! single RGBA texture. Each character occupies a fixed-size cell.
//! UV coordinates are pre-computed and cached for fast lookup.

use std::collections::HashMap;

/// UV coordinates of a single glyph in the atlas texture.
#[derive(Copy, Clone, Debug)]
pub struct GlyphUv {
    /// Top-left UV (normalized 0.0-1.0).
    pub offset: [f32; 2],
    /// Size in UV space.
    pub size: [f32; 2],
}

/// The font atlas: a texture + UV lookup table.
pub struct FontAtlas {
    /// Width of the atlas texture in pixels.
    pub width: u32,
    /// Height of the atlas texture in pixels.
    pub height: u32,
    /// Width of each character cell in pixels.
    pub cell_w: u32,
    /// Height of each character cell in pixels.
    pub cell_h: u32,
    /// Raw RGBA pixel data (width * height * 4 bytes).
    pub pixels: Vec<u8>,
    /// UV lookup per character.
    pub uvs: HashMap<char, GlyphUv>,
}

impl FontAtlas {
    /// Characters included in the atlas.
    pub const CHARS: &'static str =
        " !\"#$%&'()*+,-./0123456789:;<=>?@\
         ABCDEFGHIJKLMNOPQRSTUVWXYZ[\]^_`\
         abcdefghijklmnopqrstuvwxyz{|}~\
         ░▒▓█▄▀▌▐■□▪▫▬▭▮▯\
         ←→↑↓↔↕↖↗↘↙\
         ★✦✧☆⊕⊗⊙⊚⊛⊜\
         ∞∑∫∂∇∆∏∐\
         αβγδεζηθλμπρσφψω\
         ΑΒΓΔΕΖΗΘΛΜΠΡΣΦΨΩ\
         ℕℤℚℝℂ\
         ╔╗╚╝║═╠╣╦╩╬╟╢╤╧\
         ☠☢☣☯☮✓✗";

    /// Build a stub atlas (no actual font rasterization yet — Phase 1 uses a placeholder).
    pub fn build_stub(cell_w: u32, cell_h: u32) -> Self {
        let chars: Vec<char> = Self::CHARS.chars().collect();
        let cols = 32u32;
        let rows = (chars.len() as u32 + cols - 1) / cols;
        let width = cols * cell_w;
        let height = rows * cell_h;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let mut uvs = HashMap::new();

        for (i, ch) in chars.iter().enumerate() {
            let col = i as u32 % cols;
            let row = i as u32 / cols;
            // Fill cell with a simple brightness pattern
            let base_x = col * cell_w;
            let base_y = row * cell_h;
            for py in 0..cell_h {
                for px in 0..cell_w {
                    let idx = ((base_y + py) * width + (base_x + px)) as usize * 4;
                    // Checkerboard stub so glyphs are visible
                    let on = ((px + py) % 2 == 0) as u8;
                    pixels[idx]     = 255 * on;
                    pixels[idx + 1] = 255 * on;
                    pixels[idx + 2] = 255 * on;
                    pixels[idx + 3] = 255;
                }
            }
            uvs.insert(*ch, GlyphUv {
                offset: [base_x as f32 / width as f32, base_y as f32 / height as f32],
                size: [cell_w as f32 / width as f32, cell_h as f32 / height as f32],
            });
        }

        Self { width, height, cell_w, cell_h, pixels, uvs }
    }

    pub fn uv_for(&self, ch: char) -> GlyphUv {
        self.uvs.get(&ch).copied().unwrap_or(GlyphUv {
            offset: [0.0, 0.0],
            size: [self.cell_w as f32 / self.width as f32,
                   self.cell_h as f32 / self.height as f32],
        })
    }
}
