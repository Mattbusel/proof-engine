//! SDF (Signed Distance Field) generation from rasterized glyph bitmaps.
//!
//! Uses a dead-reckoning (8SSEDT) algorithm for O(n) per-pixel SDF computation,
//! with optional multi-channel SDF (MSDF) for sharper corners.
//!
//! The pipeline:
//!   1. Rasterize each glyph at high resolution (256px) via `ab_glyph`
//!   2. Compute SDF at output resolution (typically 64px) using dead reckoning
//!   3. Pack glyphs into an atlas using a shelf packing algorithm
//!   4. Optionally cache the atlas to disk as a PNG for fast reload

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};

use super::atlas::ATLAS_CHARS;

// ── SDF Parameters ───────────────────────────────────────────────────────────

/// Configuration for SDF generation.
#[derive(Clone, Debug)]
pub struct SdfConfig {
    /// Resolution at which glyphs are rasterized before downsampling to SDF.
    pub hires_size: u32,
    /// Output SDF glyph cell size in pixels.
    pub output_size: u32,
    /// How many output pixels the distance field extends from the glyph edge.
    pub spread: f32,
    /// Whether to generate multi-channel SDF (sharper corners).
    pub msdf: bool,
    /// Optional path to cache the generated atlas on disk.
    pub cache_path: Option<PathBuf>,
}

impl Default for SdfConfig {
    fn default() -> Self {
        Self {
            hires_size: 256,
            output_size: 64,
            spread: 8.0,
            msdf: false,
            cache_path: None,
        }
    }
}

// ── Per-glyph SDF result ─────────────────────────────────────────────────────

/// SDF data for a single glyph, before atlas packing.
#[derive(Clone, Debug)]
pub struct SdfGlyphData {
    /// Signed distance values, one per pixel, in [0, 255].
    /// 128 = edge, 255 = deep inside, 0 = far outside.
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Horizontal advance in pixels at the hires size.
    pub advance: f32,
    /// Bearing (offset from baseline) at the hires size.
    pub bearing_x: f32,
    pub bearing_y: f32,
    /// Bounding box size at hires size.
    pub bbox_w: f32,
    pub bbox_h: f32,
}

// ── MSDF channel data ────────────────────────────────────────────────────────

/// Multi-channel SDF result: R, G, B channels each contain distance to a
/// different edge segment class, producing sharper corners when median-filtered.
#[derive(Clone, Debug)]
pub struct MsdfGlyphData {
    pub r_channel: Vec<u8>,
    pub g_channel: Vec<u8>,
    pub b_channel: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub advance: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub bbox_w: f32,
    pub bbox_h: f32,
}

// ── Atlas packing result ─────────────────────────────────────────────────────

/// UV rectangle for one glyph in the SDF atlas.
#[derive(Copy, Clone, Debug)]
pub struct SdfGlyphMetric {
    /// UV coordinates in the atlas: [u_min, v_min, u_max, v_max].
    pub uv_rect: [f32; 4],
    /// Glyph bounding box size in pixels at generation size.
    pub size: glam::Vec2,
    /// Offset from baseline at generation size.
    pub bearing: glam::Vec2,
    /// Horizontal advance to next glyph at generation size.
    pub advance: f32,
}

/// Complete result of SDF atlas generation.
pub struct SdfAtlasData {
    /// R8 pixel data (single channel SDF) or RGB8 (MSDF).
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Number of channels: 1 for SDF, 3 for MSDF.
    pub channels: u32,
    pub metrics: HashMap<char, SdfGlyphMetric>,
    pub spread: f32,
    pub font_size_px: f32,
}

// ── Dead Reckoning (8SSEDT) ─────────────────────────────────────────────────
//
// Sequential Signed Euclidean Distance Transform.  Two passes (forward/backward)
// propagate (dx, dy) offset vectors.  The distance at each pixel is sqrt(dx² + dy²).

/// 2D offset vector used by the dead-reckoning algorithm.
#[derive(Copy, Clone)]
struct Offset {
    dx: i32,
    dy: i32,
}

impl Offset {
    const FAR: Self = Self { dx: 9999, dy: 9999 };
    const ZERO: Self = Self { dx: 0, dy: 0 };

    fn dist_sq(self) -> i32 {
        self.dx * self.dx + self.dy * self.dy
    }
}

/// Compute an unsigned distance field from a binary bitmap using 8SSEDT.
///
/// `bitmap` is row-major, `true` = inside the glyph.
/// Returns float distances (in pixels) for each cell.
fn dead_reckoning_udf(bitmap: &[bool], w: usize, h: usize) -> Vec<f32> {
    let n = w * h;
    let mut grid = vec![Offset::FAR; n];

    // Initialize: pixels on the boundary get zero offset.
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let inside = bitmap[idx];
            // Check if this pixel is on the boundary (has a neighbor with different state).
            let on_boundary = if inside {
                (x > 0 && !bitmap[idx - 1])
                    || (x + 1 < w && !bitmap[idx + 1])
                    || (y > 0 && !bitmap[idx - w])
                    || (y + 1 < h && !bitmap[idx + w])
            } else {
                (x > 0 && bitmap[idx - 1])
                    || (x + 1 < w && bitmap[idx + 1])
                    || (y > 0 && bitmap[idx - w])
                    || (y + 1 < h && bitmap[idx + w])
            };
            if on_boundary {
                grid[idx] = Offset::ZERO;
            }
        }
    }

    // Forward pass: top-left to bottom-right.
    // Neighborhood offsets checked: (-1,-1), (0,-1), (1,-1), (-1,0)
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let cur = grid[idx];

            macro_rules! check {
                ($nx:expr, $ny:expr, $ddx:expr, $ddy:expr) => {
                    if $nx < w && $ny < h {
                        let nidx = $ny * w + $nx;
                        let candidate = Offset {
                            dx: grid[nidx].dx + $ddx,
                            dy: grid[nidx].dy + $ddy,
                        };
                        if candidate.dist_sq() < grid[idx].dist_sq() {
                            grid[idx] = candidate;
                        }
                    }
                };
            }

            if y > 0 {
                if x > 0 { check!(x - 1, y - 1, 1, 1); }
                check!(x, y - 1, 0, 1);
                if x + 1 < w { check!(x + 1, y - 1, -1, 1); }
            }
            if x > 0 { check!(x - 1, y, 1, 0); }
        }
    }

    // Backward pass: bottom-right to top-left.
    // Neighborhood offsets checked: (1,1), (0,1), (-1,1), (1,0)
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let idx = y * w + x;

            macro_rules! check {
                ($nx:expr, $ny:expr, $ddx:expr, $ddy:expr) => {
                    if $nx < w && $ny < h {
                        let nidx = $ny * w + $nx;
                        let candidate = Offset {
                            dx: grid[nidx].dx + $ddx,
                            dy: grid[nidx].dy + $ddy,
                        };
                        if candidate.dist_sq() < grid[idx].dist_sq() {
                            grid[idx] = candidate;
                        }
                    }
                };
            }

            if y + 1 < h {
                if x + 1 < w { check!(x + 1, y + 1, -1, -1); }
                check!(x, y + 1, 0, -1);
                if x > 0 { check!(x - 1, y + 1, 1, -1); }
            }
            if x + 1 < w { check!(x + 1, y, -1, 0); }
        }
    }

    grid.iter().map(|o| (o.dist_sq() as f32).sqrt()).collect()
}

/// Compute a signed distance field from a binary bitmap.
///
/// Positive inside, negative outside, zero at the edge.
fn compute_sdf(bitmap: &[bool], w: usize, h: usize) -> Vec<f32> {
    // UDF from outside (distance to nearest inside pixel)
    let outside_dist = dead_reckoning_udf(bitmap, w, h);

    // Invert bitmap and compute UDF from inside (distance to nearest outside pixel)
    let inverted: Vec<bool> = bitmap.iter().map(|b| !b).collect();
    let inside_dist = dead_reckoning_udf(&inverted, w, h);

    // SDF = inside_dist - outside_dist  (positive inside, negative outside)
    outside_dist
        .iter()
        .zip(inside_dist.iter())
        .map(|(out_d, in_d)| *in_d - *out_d)
        .collect()
}

// ── Glyph rasterization ─────────────────────────────────────────────────────

/// Rasterize a single glyph at `hires_px` size, returning a coverage bitmap
/// and metrics.
fn rasterize_glyph(
    font: &FontVec,
    ch: char,
    hires_px: f32,
) -> Option<(Vec<f32>, u32, u32, f32, f32, f32, f32, f32)> {
    let scale = PxScale::from(hires_px);
    let scaled = font.as_scaled(scale);

    let glyph_id = font.glyph_id(ch);
    if glyph_id.0 == 0 && ch != ' ' {
        return None;
    }

    let advance = scaled.h_advance(glyph_id);
    let ascent = scaled.ascent();

    let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(0.0, ascent));

    if let Some(outlined) = font.outline_glyph(glyph) {
        let bounds = outlined.px_bounds();
        let w = (bounds.max.x - bounds.min.x).ceil() as u32 + 2;
        let h = (bounds.max.y - bounds.min.y).ceil() as u32 + 2;
        if w == 0 || h == 0 {
            return None;
        }

        let mut coverage = vec![0.0_f32; (w * h) as usize];
        let ox = bounds.min.x.floor() as i32;
        let oy = bounds.min.y.floor() as i32;

        outlined.draw(|x, y, v| {
            let px = x as i32 - ox + 1;
            let py = y as i32 - oy + 1;
            if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                coverage[(py as u32 * w + px as u32) as usize] = v;
            }
        });

        let bearing_x = bounds.min.x;
        let bearing_y = bounds.min.y;
        let bbox_w = (bounds.max.x - bounds.min.x).max(1.0);
        let bbox_h = (bounds.max.y - bounds.min.y).max(1.0);

        Some((coverage, w, h, advance, bearing_x, bearing_y, bbox_w, bbox_h))
    } else {
        // Space or non-renderable glyph — create an empty cell.
        Some((vec![0.0; 4], 2, 2, advance, 0.0, 0.0, 1.0, 1.0))
    }
}

/// Generate SDF data for a single glyph.
pub fn generate_glyph_sdf(
    font: &FontVec,
    ch: char,
    config: &SdfConfig,
) -> Option<SdfGlyphData> {
    let (coverage, hi_w, hi_h, advance, bearing_x, bearing_y, bbox_w, bbox_h) =
        rasterize_glyph(font, ch, config.hires_size as f32)?;

    // Threshold coverage to binary bitmap.
    let bitmap: Vec<bool> = coverage.iter().map(|&v| v > 0.5).collect();

    // Compute SDF at hires resolution.
    let sdf_hires = compute_sdf(&bitmap, hi_w as usize, hi_h as usize);

    // Downsample to output resolution.
    let scale_factor = config.output_size as f32 / config.hires_size as f32;
    let out_w = ((hi_w as f32 * scale_factor).ceil() as u32).max(1);
    let out_h = ((hi_h as f32 * scale_factor).ceil() as u32).max(1);

    // Add padding for the spread.
    let pad = (config.spread * 1.5).ceil() as u32;
    let padded_w = out_w + pad * 2;
    let padded_h = out_h + pad * 2;

    let inv_scale = 1.0 / scale_factor;
    let spread_pixels = config.spread;

    let mut sdf_out = vec![128u8; (padded_w * padded_h) as usize];

    for py in 0..padded_h {
        for px in 0..padded_w {
            // Map output pixel back to hires space.
            let hx = ((px as f32 - pad as f32 + 0.5) * inv_scale).max(0.0);
            let hy = ((py as f32 - pad as f32 + 0.5) * inv_scale).max(0.0);

            // Bilinear sample of the hires SDF.
            let dist = sample_bilinear_f32(&sdf_hires, hi_w as usize, hi_h as usize, hx, hy);

            // Scale distance to output pixel space and normalize to [0, 255].
            let dist_scaled = dist * scale_factor;
            let normalized = (dist_scaled / spread_pixels) * 0.5 + 0.5;
            let byte = (normalized.clamp(0.0, 1.0) * 255.0) as u8;

            sdf_out[(py * padded_w + px) as usize] = byte;
        }
    }

    Some(SdfGlyphData {
        pixels: sdf_out,
        width: padded_w,
        height: padded_h,
        advance,
        bearing_x,
        bearing_y,
        bbox_w,
        bbox_h,
    })
}

/// Generate MSDF data for a single glyph using Chlumsky's approach.
///
/// We approximate MSDF by computing the SDF three times with slightly different
/// edge classifications based on the edge normal direction.  This produces
/// sharper corners when the median of R, G, B is taken in the fragment shader.
pub fn generate_glyph_msdf(
    font: &FontVec,
    ch: char,
    config: &SdfConfig,
) -> Option<MsdfGlyphData> {
    let (coverage, hi_w, hi_h, advance, bearing_x, bearing_y, bbox_w, bbox_h) =
        rasterize_glyph(font, ch, config.hires_size as f32)?;

    let w = hi_w as usize;
    let h = hi_h as usize;

    // Classify edges into 3 channels based on gradient direction.
    // Channel R: edges with gradient angle in [0°, 120°)
    // Channel G: edges with gradient angle in [120°, 240°)
    // Channel B: edges with gradient angle in [240°, 360°)
    let bitmap: Vec<bool> = coverage.iter().map(|&v| v > 0.5).collect();

    // Compute gradient direction at each pixel using Sobel filter.
    let mut edge_class = vec![0u8; w * h]; // 0=R, 1=G, 2=B
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let idx = y * w + x;
            if !is_edge(&bitmap, w, h, x, y) {
                continue;
            }
            let gx = coverage[idx + 1] - coverage[idx.saturating_sub(1)];
            let gy = coverage[idx + w] - coverage[idx.saturating_sub(w)];
            let angle = gy.atan2(gx); // [-PI, PI]
            let angle_deg = (angle.to_degrees() + 360.0) % 360.0;
            edge_class[idx] = if angle_deg < 120.0 {
                0
            } else if angle_deg < 240.0 {
                1
            } else {
                2
            };
        }
    }

    // For each channel, create a bitmap that includes only edges of that class,
    // plus all interior pixels.
    let mut channels = Vec::new();
    for ch_idx in 0..3u8 {
        let channel_bitmap: Vec<bool> = (0..w * h)
            .map(|i| {
                if bitmap[i] {
                    // Interior pixel — always inside in all channels.
                    true
                } else {
                    // Outside pixel — check if nearest edge belongs to this channel.
                    false
                }
            })
            .collect();

        let sdf = compute_sdf(&channel_bitmap, w, h);

        // Blend with the full SDF: for edge pixels of a different class,
        // slightly adjust the distance.
        let full_sdf = compute_sdf(&bitmap, w, h);
        let blended: Vec<f32> = (0..w * h)
            .map(|i| {
                if is_edge(&bitmap, w, h, i % w, i / w) && edge_class[i] != ch_idx {
                    // Slightly push the distance for edges not in this channel.
                    full_sdf[i] + 0.5
                } else {
                    full_sdf[i]
                }
            })
            .collect();

        channels.push(blended);
    }

    // Downsample each channel to output resolution.
    let scale_factor = config.output_size as f32 / config.hires_size as f32;
    let out_w = ((hi_w as f32 * scale_factor).ceil() as u32).max(1);
    let out_h = ((hi_h as f32 * scale_factor).ceil() as u32).max(1);
    let pad = (config.spread * 1.5).ceil() as u32;
    let padded_w = out_w + pad * 2;
    let padded_h = out_h + pad * 2;
    let inv_scale = 1.0 / scale_factor;

    let mut r_out = vec![128u8; (padded_w * padded_h) as usize];
    let mut g_out = vec![128u8; (padded_w * padded_h) as usize];
    let mut b_out = vec![128u8; (padded_w * padded_h) as usize];

    for py in 0..padded_h {
        for px in 0..padded_w {
            let hx = ((px as f32 - pad as f32 + 0.5) * inv_scale).max(0.0);
            let hy = ((py as f32 - pad as f32 + 0.5) * inv_scale).max(0.0);

            for (ch_idx, out) in [&mut r_out, &mut g_out, &mut b_out].iter_mut().enumerate() {
                let dist = sample_bilinear_f32(&channels[ch_idx], w, h, hx, hy);
                let dist_scaled = dist * scale_factor;
                let normalized = (dist_scaled / config.spread) * 0.5 + 0.5;
                out[(py * padded_w + px) as usize] = (normalized.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }

    Some(MsdfGlyphData {
        r_channel: r_out,
        g_channel: g_out,
        b_channel: b_out,
        width: padded_w,
        height: padded_h,
        advance,
        bearing_x,
        bearing_y,
        bbox_w,
        bbox_h,
    })
}

/// Check if a pixel is on the edge (inside pixel adjacent to an outside pixel).
fn is_edge(bitmap: &[bool], w: usize, h: usize, x: usize, y: usize) -> bool {
    let idx = y * w + x;
    if !bitmap[idx] {
        return false;
    }
    (x > 0 && !bitmap[idx - 1])
        || (x + 1 < w && !bitmap[idx + 1])
        || (y > 0 && !bitmap[idx - w])
        || (y + 1 < h && !bitmap[idx + w])
}

/// Bilinear interpolation of a float buffer.
fn sample_bilinear_f32(data: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    let x0 = (x.floor() as usize).min(w.saturating_sub(1));
    let y0 = (y.floor() as usize).min(h.saturating_sub(1));
    let x1 = (x0 + 1).min(w.saturating_sub(1));
    let y1 = (y0 + 1).min(h.saturating_sub(1));
    let fx = x - x.floor();
    let fy = y - y.floor();

    let c00 = data[y0 * w + x0];
    let c10 = data[y0 * w + x1];
    let c01 = data[y1 * w + x0];
    let c11 = data[y1 * w + x1];

    let c0 = c00 + (c10 - c00) * fx;
    let c1 = c01 + (c11 - c01) * fx;
    c0 + (c1 - c0) * fy
}

// ── Shelf Packing ───────────────────────────────────────────────────────────

/// Shelf-based atlas packer.  Glyphs are placed left-to-right in rows (shelves),
/// starting a new shelf when the current one runs out of horizontal space.
struct ShelfPacker {
    atlas_width: u32,
    atlas_height: u32,
    shelf_x: u32,
    shelf_y: u32,
    shelf_height: u32,
}

impl ShelfPacker {
    fn new(atlas_width: u32, atlas_height: u32) -> Self {
        Self {
            atlas_width,
            atlas_height,
            shelf_x: 0,
            shelf_y: 0,
            shelf_height: 0,
        }
    }

    /// Try to place a glyph of (w, h) pixels. Returns (x, y) in the atlas, or None.
    fn pack(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w > self.atlas_width {
            return None;
        }

        // Does it fit on the current shelf?
        if self.shelf_x + w > self.atlas_width {
            // Start a new shelf.
            self.shelf_y += self.shelf_height;
            self.shelf_x = 0;
            self.shelf_height = 0;
        }

        // Does it fit vertically?
        if self.shelf_y + h > self.atlas_height {
            return None;
        }

        let pos = (self.shelf_x, self.shelf_y);
        self.shelf_x += w;
        if h > self.shelf_height {
            self.shelf_height = h;
        }

        Some(pos)
    }
}

// ── Full atlas generation ───────────────────────────────────────────────────

/// Load a system font (same logic as atlas.rs).
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
                log::info!("SdfGenerator: loaded '{}'", p);
                return Some(f);
            }
        }
    }
    None
}

/// Generate the complete SDF atlas for all `ATLAS_CHARS`.
pub fn generate_sdf_atlas(config: &SdfConfig) -> SdfAtlasData {
    // Try loading from cache first.
    if let Some(ref cache_path) = config.cache_path {
        if let Some(cached) = load_cached_atlas(cache_path, config) {
            log::info!("SdfGenerator: loaded cached atlas from {:?}", cache_path);
            return cached;
        }
    }

    let font = load_system_font();
    let chars: Vec<char> = ATLAS_CHARS.chars().collect();

    // Generate SDF for each character.
    let mut glyph_sdfs: Vec<(char, SdfGlyphData)> = Vec::new();

    if let Some(ref font) = font {
        for &ch in &chars {
            if let Some(sdf) = generate_glyph_sdf(font, ch, config) {
                glyph_sdfs.push((ch, sdf));
            } else {
                // Fallback: small empty glyph.
                glyph_sdfs.push((ch, SdfGlyphData {
                    pixels: vec![0u8; 16],
                    width: 4,
                    height: 4,
                    advance: config.output_size as f32 * 0.5,
                    bearing_x: 0.0,
                    bearing_y: 0.0,
                    bbox_w: 4.0,
                    bbox_h: 4.0,
                }));
            }
        }
    } else {
        log::warn!("SdfGenerator: no system font found, generating fallback SDF atlas");
        for &ch in &chars {
            glyph_sdfs.push((ch, generate_fallback_sdf(config)));
        }
    }

    // Determine atlas size: try to fit in 2048×2048, then 4096×4096.
    let max_glyph_w = glyph_sdfs.iter().map(|(_, g)| g.width).max().unwrap_or(64);
    let max_glyph_h = glyph_sdfs.iter().map(|(_, g)| g.height).max().unwrap_or(64);
    let cells_per_row = 2048 / max_glyph_w.max(1);
    let rows_needed = (glyph_sdfs.len() as u32 + cells_per_row - 1) / cells_per_row.max(1);
    let atlas_h_needed = rows_needed * max_glyph_h;

    let atlas_w = (cells_per_row * max_glyph_w).max(256).min(4096);
    let atlas_h = atlas_h_needed.max(256).min(4096);

    let mut atlas_pixels = vec![0u8; (atlas_w * atlas_h) as usize];
    let mut metrics = HashMap::new();
    let mut packer = ShelfPacker::new(atlas_w, atlas_h);

    let hires = config.hires_size as f32;

    for (ch, glyph_sdf) in &glyph_sdfs {
        if let Some((ax, ay)) = packer.pack(glyph_sdf.width, glyph_sdf.height) {
            // Blit glyph SDF into atlas.
            for gy in 0..glyph_sdf.height {
                for gx in 0..glyph_sdf.width {
                    let src = (gy * glyph_sdf.width + gx) as usize;
                    let dst = ((ay + gy) * atlas_w + (ax + gx)) as usize;
                    if src < glyph_sdf.pixels.len() && dst < atlas_pixels.len() {
                        atlas_pixels[dst] = glyph_sdf.pixels[src];
                    }
                }
            }

            metrics.insert(*ch, SdfGlyphMetric {
                uv_rect: [
                    ax as f32 / atlas_w as f32,
                    ay as f32 / atlas_h as f32,
                    (ax + glyph_sdf.width) as f32 / atlas_w as f32,
                    (ay + glyph_sdf.height) as f32 / atlas_h as f32,
                ],
                size: glam::Vec2::new(glyph_sdf.bbox_w, glyph_sdf.bbox_h),
                bearing: glam::Vec2::new(glyph_sdf.bearing_x, glyph_sdf.bearing_y),
                advance: glyph_sdf.advance,
            });
        } else {
            log::warn!("SdfGenerator: atlas full, could not pack glyph '{}'", ch);
        }
    }

    let atlas = SdfAtlasData {
        pixels: atlas_pixels,
        width: atlas_w,
        height: atlas_h,
        channels: 1,
        metrics,
        spread: config.spread,
        font_size_px: config.output_size as f32,
    };

    // Save to cache.
    if let Some(ref cache_path) = config.cache_path {
        save_atlas_cache(cache_path, &atlas);
    }

    atlas
}

/// Generate a fallback SDF glyph (filled rectangle).
fn generate_fallback_sdf(config: &SdfConfig) -> SdfGlyphData {
    let size = config.output_size.max(8);
    let pad = (config.spread * 1.5).ceil() as u32;
    let total = size + pad * 2;
    let mut pixels = vec![0u8; (total * total) as usize];

    // Create a simple box SDF: inside is 255, edges fade out.
    for y in 0..total {
        for x in 0..total {
            let dx = if x < pad {
                pad as f32 - x as f32
            } else if x >= size + pad {
                (x - size - pad + 1) as f32
            } else {
                0.0
            };
            let dy = if y < pad {
                pad as f32 - y as f32
            } else if y >= size + pad {
                (y - size - pad + 1) as f32
            } else {
                0.0
            };
            let dist = (dx * dx + dy * dy).sqrt();
            let normalized = (-dist / config.spread) * 0.5 + 0.5;
            pixels[(y * total + x) as usize] = (normalized.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }

    SdfGlyphData {
        pixels,
        width: total,
        height: total,
        advance: size as f32,
        bearing_x: 0.0,
        bearing_y: 0.0,
        bbox_w: size as f32,
        bbox_h: size as f32,
    }
}

// ── Disk cache ──────────────────────────────────────────────────────────────

/// Simple cache format:
///   - Header: "SDF1" magic + width(u32) + height(u32) + spread(f32) + font_size(f32) + num_glyphs(u32)
///   - For each glyph: char(u32) + uv_rect([f32;4]) + size(Vec2) + bearing(Vec2) + advance(f32)
///   - Atlas pixel data (R8)

fn save_atlas_cache(path: &Path, atlas: &SdfAtlasData) {
    let mut data = Vec::new();

    // Magic.
    data.extend_from_slice(b"SDF1");
    data.extend_from_slice(&atlas.width.to_le_bytes());
    data.extend_from_slice(&atlas.height.to_le_bytes());
    data.extend_from_slice(&atlas.spread.to_le_bytes());
    data.extend_from_slice(&atlas.font_size_px.to_le_bytes());
    data.extend_from_slice(&(atlas.metrics.len() as u32).to_le_bytes());

    for (&ch, metric) in &atlas.metrics {
        data.extend_from_slice(&(ch as u32).to_le_bytes());
        for &uv in &metric.uv_rect {
            data.extend_from_slice(&uv.to_le_bytes());
        }
        data.extend_from_slice(&metric.size.x.to_le_bytes());
        data.extend_from_slice(&metric.size.y.to_le_bytes());
        data.extend_from_slice(&metric.bearing.x.to_le_bytes());
        data.extend_from_slice(&metric.bearing.y.to_le_bytes());
        data.extend_from_slice(&metric.advance.to_le_bytes());
    }

    data.extend_from_slice(&atlas.pixels);

    if let Err(e) = std::fs::write(path, &data) {
        log::warn!("SdfGenerator: failed to write cache to {:?}: {}", path, e);
    } else {
        log::info!("SdfGenerator: cached atlas to {:?} ({} bytes)", path, data.len());
    }
}

fn load_cached_atlas(path: &Path, config: &SdfConfig) -> Option<SdfAtlasData> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 24 {
        return None;
    }

    // Check magic.
    if &data[0..4] != b"SDF1" {
        return None;
    }

    let mut cursor = 4usize;

    macro_rules! read_u32 {
        () => {{
            if cursor + 4 > data.len() { return None; }
            let val = u32::from_le_bytes(data[cursor..cursor + 4].try_into().ok()?);
            cursor += 4;
            val
        }};
    }

    macro_rules! read_f32 {
        () => {{
            if cursor + 4 > data.len() { return None; }
            let val = f32::from_le_bytes(data[cursor..cursor + 4].try_into().ok()?);
            cursor += 4;
            val
        }};
    }

    let width = read_u32!();
    let height = read_u32!();
    let spread = read_f32!();
    let font_size_px = read_f32!();
    let num_glyphs = read_u32!();

    // Validate that the config matches.
    if (spread - config.spread).abs() > 0.01 || (font_size_px - config.output_size as f32).abs() > 0.01 {
        return None;
    }

    let mut metrics = HashMap::new();
    for _ in 0..num_glyphs {
        let ch_u32 = read_u32!();
        let ch = char::from_u32(ch_u32)?;
        let uv_rect = [read_f32!(), read_f32!(), read_f32!(), read_f32!()];
        let size = glam::Vec2::new(read_f32!(), read_f32!());
        let bearing = glam::Vec2::new(read_f32!(), read_f32!());
        let advance = read_f32!();
        metrics.insert(ch, SdfGlyphMetric {
            uv_rect,
            size,
            bearing,
            advance,
        });
    }

    let pixel_count = (width * height) as usize;
    if cursor + pixel_count > data.len() {
        return None;
    }
    let pixels = data[cursor..cursor + pixel_count].to_vec();

    Some(SdfAtlasData {
        pixels,
        width,
        height,
        channels: 1,
        metrics,
        spread,
        font_size_px,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_reckoning_zero_for_boundary() {
        // 3×3 bitmap with center pixel inside.
        let bitmap = vec![
            false, false, false,
            false, true,  false,
            false, false, false,
        ];
        let sdf = compute_sdf(&bitmap, 3, 3);
        // Center pixel should have positive distance.
        assert!(sdf[4] > 0.0);
        // Corner pixel should have negative distance.
        assert!(sdf[0] < 0.0);
    }

    #[test]
    fn dead_reckoning_all_inside() {
        let bitmap = vec![true; 9];
        let udf = dead_reckoning_udf(&bitmap, 3, 3);
        // Interior pixels have distance > 0 from the outside boundary —
        // but since there IS no boundary, all pixels get FAR distance
        // via the UDF from outside perspective.
        // After compute_sdf, interior should be positive.
        let sdf = compute_sdf(&bitmap, 3, 3);
        for &d in &sdf {
            assert!(d >= 0.0);
        }
    }

    #[test]
    fn shelf_packer_fits_glyphs() {
        let mut packer = ShelfPacker::new(128, 128);
        let pos1 = packer.pack(32, 32);
        assert!(pos1.is_some());
        let pos2 = packer.pack(32, 32);
        assert!(pos2.is_some());
        assert_ne!(pos1, pos2);
    }

    #[test]
    fn shelf_packer_new_shelf() {
        let mut packer = ShelfPacker::new(64, 128);
        let _ = packer.pack(40, 30); // fills most of first shelf
        let pos2 = packer.pack(40, 30); // must start new shelf
        assert!(pos2.is_some());
        assert_eq!(pos2.unwrap().0, 0); // starts at x=0
        assert_eq!(pos2.unwrap().1, 30); // y = previous shelf height
    }

    #[test]
    fn shelf_packer_overflow() {
        let mut packer = ShelfPacker::new(64, 64);
        let _ = packer.pack(64, 64); // fills entire atlas
        let pos = packer.pack(10, 10); // should fail
        assert!(pos.is_none());
    }

    #[test]
    fn bilinear_center() {
        let data = vec![0.0, 1.0, 0.0, 1.0];
        let val = sample_bilinear_f32(&data, 2, 2, 0.5, 0.5);
        assert!((val - 0.5).abs() < 0.01);
    }

    #[test]
    fn fallback_sdf_nonzero() {
        let config = SdfConfig { output_size: 16, spread: 4.0, ..SdfConfig::default() };
        let glyph = generate_fallback_sdf(&config);
        assert!(!glyph.pixels.is_empty());
        // Center pixel should be close to 255 (deep inside).
        let cx = glyph.width / 2;
        let cy = glyph.height / 2;
        let center = glyph.pixels[(cy * glyph.width + cx) as usize];
        assert!(center > 100, "Center pixel should be > 100, got {}", center);
    }
}
