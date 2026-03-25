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
    pub fn offset(&self) -> [f32; 2] { [self.u0, self.v0] }
    pub fn size(&self)   -> [f32; 2] { [self.u1 - self.u0, self.v1 - self.v0] }
}

/// The characters the engine can render.
///
/// Covers: ASCII, box drawing (single + double), block elements, arrows,
/// math operators, Greek alphabet, symbols, card suits, musical notes,
/// geometric shapes, dingbats/stars, bullets, currency, braille-bar
/// characters, and common game/UI glyphs.
pub const ATLAS_CHARS: &str = concat!(
    // ASCII printable (32-126)
    " !\"#$%&'()*+,-./0123456789:;<=>?",
    "@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_",
    "`abcdefghijklmnopqrstuvwxyz{|}~",

    // Block elements
    "░▒▓█▄▀▌▐■□▪▫▬▮▯▰▱",

    // Box drawing — double lines
    "╔╗╚╝║═╠╣╦╩╬╟╢╤╧╪╫",
    // Box drawing — single lines
    "┌┐└┘│─├┤┬┴┼┃━┏┓┗┛┣┫┳┻╋",
    // Box drawing — mixed
    "╭╮╯╰╱╲╳",

    // Arrows
    "←→↑↓↔↕↖↗↘↙⇐⇒⇑⇓⇔▲▼◀▶▷▸▹►◁◂◃",

    // Math operators and symbols
    "∞∑∫∂∇∆∏√±∓×÷≈≠≡≤≥«»∈∉⊂⊃∪∩∧∨¬∀∃∅∝∠",
    "⌈⌉⌊⌋",

    // Greek alphabet (upper + lower)
    "ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩαβγδεζηθικλμνξοπρστυφχψω",

    // Geometric shapes
    "●○◉◎◌◍◐◑◒◓◔◕",
    "◆◇◈□■▢▣▤▥▦▧▨▩",
    "△▽▲▼◁▷◀▶",
    "◯⬠⬡⬢⬣",

    // Stars, dingbats, decorative
    "★☆✦✧✩✪✫✬✭✮✯✰✱✲✳✴✵✶✷✸✹",
    "⁂❖❘❙❚",
    "⊕⊗⊙⊛⊜⊝",

    // Bullets and dots
    "·•‣⁃∙◦°※†‡§¶",

    // Card suits
    "♠♣♥♦♤♧♡♢",

    // Musical
    "♩♪♫♬♭♮♯",

    // Misc symbols
    "☠☢☣☯☮☸✓✗✘✔✕❌⚙⚡⚔⚒⚑⚐⚠⚰⚱⛏",
    "☀☁☂☃☄☾☽❄❆❇",
    "☹☺☻♻♲♳⚕⚖⚗⚛⚜",

    // Currency
    "¢£¥€₹₽₿",

    // Bar chart characters (for spectrum visualizer, HP bars, etc.)
    "▁▂▃▄▅▆▇█",

    // Braces, brackets, misc punctuation
    "‹›「」『』【】〈〉《》",
    "…—–\u{2018}\u{2019}\u{201C}\u{201D}\u{201E}\u{201A}",
);

/// Complete font atlas ready to upload to the GPU.
/// Uses Signed Distance Field (SDF) encoding for resolution-independent rendering.
/// Each pixel stores the distance to the nearest edge:
///   128 = on the edge
///   >128 = inside the glyph
///   <128 = outside the glyph
pub struct FontAtlas {
    pub width:  u32,
    pub height: u32,
    /// R8 pixel data -- SDF encoded (128 = edge, >128 = inside, <128 = outside).
    pub pixels: Vec<u8>,
    pub uvs:    HashMap<char, GlyphUv>,
    pub cell_w: u32,
    pub cell_h: u32,
    /// Whether this atlas uses SDF encoding (vs raw coverage).
    pub is_sdf: bool,
}

/// Convert a coverage bitmap to a signed distance field.
/// `spread` is the maximum distance in pixels that the SDF encodes.
fn bitmap_to_sdf(bitmap: &[u8], w: u32, h: u32, spread: f32) -> Vec<u8> {
    let mut sdf = vec![0u8; (w * h) as usize];
    let threshold = 128u8; // coverage > this = inside

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let idx = (y as u32 * w + x as u32) as usize;
            let is_inside = bitmap[idx] > threshold;

            // Find minimum distance to an edge (brute force within spread radius)
            let search = spread.ceil() as i32 + 1;
            let mut min_dist_sq = (spread * spread) as f32 + 1.0;

            'search: for dy in -search..=search {
                for dx in -search..=search {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
                    let ni = (ny as u32 * w + nx as u32) as usize;
                    let neighbor_inside = bitmap[ni] > threshold;
                    if neighbor_inside != is_inside {
                        let d = (dx * dx + dy * dy) as f32;
                        if d < min_dist_sq { min_dist_sq = d; }
                    }
                }
            }

            let dist = min_dist_sq.sqrt();
            let signed_dist = if is_inside { dist } else { -dist };
            // Map to 0-255: 128 = edge, 255 = deep inside, 0 = far outside
            let normalized = (signed_dist / spread * 127.0 + 128.0).clamp(0.0, 255.0);
            sdf[idx] = normalized as u8;
        }
    }
    sdf
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
        // Convert to SDF for resolution-independent rendering
        let sdf_spread = 6.0; // pixels of distance field spread
        log::info!("FontAtlas: computing SDF ({}x{}, spread={})...", w, h, sdf_spread);
        let sdf_pixels = bitmap_to_sdf(&pixels, w, h, sdf_spread);
        log::info!("FontAtlas: SDF complete");

        Self { width: w, height: h, pixels: sdf_pixels, uvs, cell_w, cell_h, is_sdf: true }
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
        Self { width: w, height: h, pixels, uvs, cell_w: cw, cell_h: ch, is_sdf: false }
    }

    pub fn uv_for(&self, ch: char) -> GlyphUv {
        self.uvs.get(&ch)
            .or_else(|| self.uvs.get(&'?'))
            .copied()
            .unwrap_or(GlyphUv { u0: 0.0, v0: 0.0, u1: 0.01, v1: 0.01 })
    }
}
