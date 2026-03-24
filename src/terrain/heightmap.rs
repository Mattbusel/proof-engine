//! Heightmap generation, erosion, post-processing, and analysis.
//!
//! This module provides multiple terrain generation algorithms, erosion
//! simulations, post-processing filters, and analytical tools for working
//! with 2D height fields used as terrain data.

use glam::{Vec2, Vec3};

// ── Seeded RNG (xoshiro256** — no external deps) ──────────────────────────────

#[derive(Clone, Debug)]
struct Rng {
    state: [u64; 4],
}

impl Rng {
    fn new(seed: u64) -> Self {
        let mut s = seed;
        let mut next = || {
            s = s.wrapping_add(0x9e3779b97f4a7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
            z ^ (z >> 31)
        };
        Self { state: [next(), next(), next(), next()] }
    }

    fn rol64(x: u64, k: u32) -> u64 { (x << k) | (x >> (64 - k)) }

    fn next_u64(&mut self) -> u64 {
        let result = Self::rol64(self.state[1].wrapping_mul(5), 7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = Self::rol64(self.state[3], 45);
        result
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 11) as f32 / (1u64 << 53) as f32
    }

    fn next_f32_range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    fn next_usize(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

// ── Perlin-style gradient noise (internal, no external dep) ──────────────────

fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }
fn lerp(a: f32, b: f32, t: f32) -> f32 { a + t * (b - a) }

fn grad2(hash: u32, x: f32, y: f32) -> f32 {
    match hash & 7 {
        0 =>  x + y, 1 => -x + y, 2 =>  x - y, 3 => -x - y,
        4 =>  x,     5 => -x,     6 =>  y,      _ => -y,
    }
}

/// Minimal self-contained 2D value/gradient noise.
struct GradientNoise {
    perm: [u8; 512],
}

impl GradientNoise {
    fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut p: [u8; 256] = [0u8; 256];
        for (i, v) in p.iter_mut().enumerate() { *v = i as u8; }
        // Fisher-Yates shuffle
        for i in (1..256).rev() {
            let j = rng.next_usize(i + 1);
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 { perm[i] = p[i & 255]; }
        Self { perm }
    }

    fn noise2d(&self, x: f32, y: f32) -> f32 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let xf = x - x.floor();
        let yf = y - y.floor();
        let u = fade(xf);
        let v = fade(yf);
        let aa = self.perm[(self.perm[(xi & 255) as usize] as i32 + (yi & 255)) as usize & 511] as u32;
        let ab = self.perm[(self.perm[(xi & 255) as usize] as i32 + (yi & 255) + 1) as usize & 511] as u32;
        let ba = self.perm[(self.perm[((xi + 1) & 255) as usize] as i32 + (yi & 255)) as usize & 511] as u32;
        let bb = self.perm[(self.perm[((xi + 1) & 255) as usize] as i32 + (yi & 255) + 1) as usize & 511] as u32;
        let x1 = lerp(grad2(aa, xf, yf), grad2(ba, xf - 1.0, yf), u);
        let x2 = lerp(grad2(ab, xf, yf - 1.0), grad2(bb, xf - 1.0, yf - 1.0), u);
        (lerp(x1, x2, v) + 1.0) * 0.5
    }
}

// ── HeightMap ─────────────────────────────────────────────────────────────────

/// A 2D grid of f32 height values.
///
/// Heights are stored in row-major order: `data[y * width + x]`.
/// Values are typically in [0, 1] but the API does not enforce this.
#[derive(Clone, Debug)]
pub struct HeightMap {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl HeightMap {
    /// Create a new zero-filled heightmap.
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, data: vec![0.0; width * height] }
    }

    /// Create from existing data. Panics if `data.len() != width * height`.
    pub fn from_data(width: usize, height: usize, data: Vec<f32>) -> Self {
        assert_eq!(data.len(), width * height, "data length mismatch");
        Self { width, height, data }
    }

    /// Get height at integer coordinates. Returns 0.0 if out-of-bounds.
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            0.0
        }
    }

    /// Set height at integer coordinates. No-op if out-of-bounds.
    pub fn set(&mut self, x: usize, y: usize, v: f32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = v;
        }
    }

    /// Sample with bilinear interpolation at floating-point coordinates.
    /// Coordinates are clamped to valid range.
    pub fn sample_bilinear(&self, x: f32, y: f32) -> f32 {
        let cx = x.clamp(0.0, (self.width  - 1) as f32);
        let cy = y.clamp(0.0, (self.height - 1) as f32);
        let x0 = cx.floor() as usize;
        let y0 = cy.floor() as usize;
        let x1 = (x0 + 1).min(self.width  - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = cx - x0 as f32;
        let ty = cy - y0 as f32;
        let h00 = self.get(x0, y0);
        let h10 = self.get(x1, y0);
        let h01 = self.get(x0, y1);
        let h11 = self.get(x1, y1);
        lerp(lerp(h00, h10, tx), lerp(h01, h11, tx), ty)
    }

    /// Sample with cubic (Catmull-Rom) interpolation.
    pub fn sample_cubic(&self, x: f32, y: f32) -> f32 {
        let cx = x.clamp(1.0, (self.width  - 2) as f32);
        let cy = y.clamp(1.0, (self.height - 2) as f32);
        let x1 = cx.floor() as usize;
        let y1 = cy.floor() as usize;
        let tx = cx - x1 as f32;
        let ty = cy - y1 as f32;

        let catmull = |p0: f32, p1: f32, p2: f32, p3: f32, t: f32| -> f32 {
            let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
            let b =        p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
            let c = -0.5 * p0              + 0.5 * p2;
            let d = p1;
            a * t * t * t + b * t * t + c * t + d
        };

        let row = |yr: usize| {
            let x0 = if x1 > 0 { x1 - 1 } else { 0 };
            let x2 = (x1 + 1).min(self.width - 1);
            let x3 = (x1 + 2).min(self.width - 1);
            catmull(self.get(x0, yr), self.get(x1, yr), self.get(x2, yr), self.get(x3, yr), tx)
        };

        let y0 = if y1 > 0 { y1 - 1 } else { 0 };
        let y2 = (y1 + 1).min(self.height - 1);
        let y3 = (y1 + 2).min(self.height - 1);
        catmull(row(y0), row(y1), row(y2), row(y3), ty)
    }

    /// Compute surface normal at (x, y) from height differences.
    /// Returns a normalized Vec3 pointing upward.
    pub fn normal_at(&self, x: usize, y: usize) -> Vec3 {
        let x0 = if x > 0 { x - 1 } else { 0 };
        let x2 = (x + 1).min(self.width  - 1);
        let y0 = if y > 0 { y - 1 } else { 0 };
        let y2 = (y + 1).min(self.height - 1);
        let dzdx = (self.get(x2, y) - self.get(x0, y)) / 2.0;
        let dzdy = (self.get(x, y2) - self.get(x, y0)) / 2.0;
        Vec3::new(-dzdx, 1.0, -dzdy).normalize()
    }

    /// Minimum value in the map.
    pub fn min_value(&self) -> f32 {
        self.data.iter().cloned().fold(f32::INFINITY, f32::min)
    }

    /// Maximum value in the map.
    pub fn max_value(&self) -> f32 {
        self.data.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
    }

    /// Normalize values so they span [0, 1].
    pub fn normalize(&mut self) {
        let mn = self.min_value();
        let mx = self.max_value();
        let range = mx - mn;
        if range < 1e-9 { return; }
        for v in self.data.iter_mut() { *v = (*v - mn) / range; }
    }

    /// Clamp all values to [min, max].
    pub fn clamp_range(&mut self, min: f32, max: f32) {
        for v in self.data.iter_mut() { *v = v.clamp(min, max); }
    }

    /// Box-blur with the given radius (integer cells).
    pub fn blur(&mut self, radius: usize) {
        if radius == 0 { return; }
        let w = self.width;
        let h = self.height;
        let mut tmp = vec![0.0f32; w * h];
        let r = radius as i32;
        // Horizontal pass
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0f32;
                let mut count = 0;
                for dx in -r..=r {
                    let nx = x as i32 + dx;
                    if nx >= 0 && (nx as usize) < w {
                        sum += self.data[y * w + nx as usize];
                        count += 1;
                    }
                }
                tmp[y * w + x] = sum / count as f32;
            }
        }
        // Vertical pass
        let mut out = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0f32;
                let mut count = 0;
                for dy in -r..=r {
                    let ny = y as i32 + dy;
                    if ny >= 0 && (ny as usize) < h {
                        sum += tmp[ny as usize * w + x];
                        count += 1;
                    }
                }
                out[y * w + x] = sum / count as f32;
            }
        }
        self.data = out;
    }

    /// Sharpen using unsharp mask: `original + amount * (original - blurred)`.
    pub fn sharpen(&mut self, amount: f32) {
        let mut blurred = self.clone();
        blurred.blur(2);
        for (v, b) in self.data.iter_mut().zip(blurred.data.iter()) {
            *v = (*v + amount * (*v - b)).clamp(0.0, 1.0);
        }
    }

    /// Terrace the heightmap into `levels` discrete steps.
    pub fn terrace(&mut self, levels: usize) {
        if levels < 2 { return; }
        let levels_f = levels as f32;
        for v in self.data.iter_mut() {
            *v = (*v * levels_f).floor() / (levels_f - 1.0);
            *v = v.clamp(0.0, 1.0);
        }
    }

    /// Apply a radial island mask, fading edges to zero.
    /// `falloff` controls how quickly edges fade (larger = sharper).
    pub fn island_mask(&mut self, falloff: f32) {
        let cx = (self.width  as f32 - 1.0) * 0.5;
        let cy = (self.height as f32 - 1.0) * 0.5;
        let max_dist = cx.min(cy);
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = (x as f32 - cx) / max_dist;
                let dy = (y as f32 - cy) / max_dist;
                let dist = (dx * dx + dy * dy).sqrt().clamp(0.0, 1.0);
                let mask = (1.0 - dist.powf(falloff)).clamp(0.0, 1.0);
                self.data[y * self.width + x] *= mask;
            }
        }
    }

    /// Compute ridge noise by folding the noise into ridges.
    /// Modifies in place using the existing data as input.
    pub fn ridge_noise(&mut self, octaves: usize) {
        let noise = GradientNoise::new(42);
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let mut amplitude = 1.0f32;
                let mut frequency = 1.0f32;
                let mut value = 0.0f32;
                let mut max_amplitude = 0.0f32;
                for _oct in 0..octaves {
                    let nx = x as f32 / w as f32 * frequency;
                    let ny = y as f32 / h as f32 * frequency;
                    let n = noise.noise2d(nx * 8.0, ny * 8.0);
                    // Fold: ridge = 1 - |2n - 1|
                    let ridge = 1.0 - (2.0 * n - 1.0).abs();
                    value += ridge * ridge * amplitude;
                    max_amplitude += amplitude;
                    amplitude *= 0.5;
                    frequency *= 2.0;
                }
                let existing = self.data[y * w + x];
                self.data[y * w + x] = (existing + value / max_amplitude).clamp(0.0, 1.0) * 0.5;
            }
        }
    }

    // ── Analysis ─────────────────────────────────────────────────────────────

    /// Compute gradient magnitude (slope) at each cell.
    pub fn slope_map(&self) -> HeightMap {
        let mut out = HeightMap::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let n = self.normal_at(x, y);
                // Slope is angle from vertical: 0 = flat, 1 = vertical
                let slope = 1.0 - n.y.clamp(0.0, 1.0);
                out.set(x, y, slope);
            }
        }
        out
    }

    /// Compute curvature (Laplacian) at each cell.
    /// Positive = convex (hill top), negative = concave (valley).
    pub fn curvature_map(&self) -> HeightMap {
        let mut out = HeightMap::new(self.width, self.height);
        for y in 1..(self.height - 1) {
            for x in 1..(self.width - 1) {
                let center = self.get(x, y);
                let laplacian = self.get(x - 1, y) + self.get(x + 1, y)
                    + self.get(x, y - 1) + self.get(x, y + 1)
                    - 4.0 * center;
                // Normalize to [0,1] (laplacian typically in [-1, 1] range for [0,1] heights)
                out.set(x, y, (laplacian * 0.5 + 0.5).clamp(0.0, 1.0));
            }
        }
        out
    }

    /// Compute water flow direction map using D8 steepest descent.
    /// Returns a map where value encodes direction (0-7 for 8 neighbors, 8 = flat).
    pub fn flow_map(&self) -> HeightMap {
        let mut out = HeightMap::new(self.width, self.height);
        let dirs: [(i32, i32); 8] = [
            (-1, -1), (0, -1), (1, -1),
            (-1,  0),          (1,  0),
            (-1,  1), (0,  1), (1,  1),
        ];
        for y in 0..self.height {
            for x in 0..self.width {
                let h = self.get(x, y);
                let mut min_h = h;
                let mut best_dir = 8usize;
                for (d, (dx, dy)) in dirs.iter().enumerate() {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < self.width as i32 && ny >= 0 && ny < self.height as i32 {
                        let nh = self.get(nx as usize, ny as usize);
                        if nh < min_h {
                            min_h = nh;
                            best_dir = d;
                        }
                    }
                }
                out.set(x, y, best_dir as f32 / 8.0);
            }
        }
        out
    }

    /// Compute a static shadow map given a sun direction (normalized Vec3).
    /// Returns 1.0 = lit, 0.0 = shadowed.
    pub fn shadow_map(&self, sun_dir: Vec3) -> HeightMap {
        let mut out = HeightMap::new(self.width, self.height);
        // Initialize all as lit
        for v in out.data.iter_mut() { *v = 1.0; }

        // Sun direction must go from terrain to sun: invert for ray march direction
        let step_x = -sun_dir.x / sun_dir.y.abs().max(0.001);
        let step_z = -sun_dir.z / sun_dir.y.abs().max(0.001);
        let step_h = 1.0f32; // height units per step

        for y in 0..self.height {
            for x in 0..self.width {
                let h0 = self.get(x, y);
                let mut cx = x as f32;
                let mut cy = y as f32;
                let mut horizon_h = h0;
                for _step in 0..256 {
                    cx += step_x;
                    cy += step_z;
                    horizon_h += step_h * 0.01;
                    if cx < 0.0 || cx >= self.width as f32 || cy < 0.0 || cy >= self.height as f32 {
                        break;
                    }
                    let terrain_h = self.sample_bilinear(cx, cy);
                    if terrain_h > horizon_h {
                        out.set(x, y, 0.0);
                        break;
                    }
                }
            }
        }
        out
    }

    /// Compute viewshed: for each cell, how many other cells can see it from `observer_height`.
    /// Returns a normalized visibility count map.
    pub fn visibility_map(&self, observer_height: f32) -> HeightMap {
        let mut out = HeightMap::new(self.width, self.height);
        let w = self.width;
        let h = self.height;
        // For each source cell, cast rays to sample points
        for y in 0..h {
            for x in 0..w {
                let eye_h = self.get(x, y) + observer_height;
                let mut visible_count = 0usize;
                let total = 64usize; // 64 sample directions
                for i in 0..total {
                    let angle = i as f32 * std::f32::consts::TAU / total as f32;
                    let dir_x = angle.cos();
                    let dir_y = angle.sin();
                    let max_dist = (w.min(h) as f32) * 0.5;
                    let steps = max_dist as usize;
                    let mut visible = true;
                    let mut max_slope = f32::NEG_INFINITY;
                    for s in 1..steps {
                        let sx = x as f32 + dir_x * s as f32;
                        let sy = y as f32 + dir_y * s as f32;
                        if sx < 0.0 || sx >= w as f32 || sy < 0.0 || sy >= h as f32 { break; }
                        let th = self.sample_bilinear(sx, sy);
                        let dist = (s as f32).max(1.0);
                        let slope = (th - eye_h) / dist;
                        if slope > max_slope {
                            max_slope = slope;
                        } else if slope < max_slope - 0.05 {
                            visible = false;
                            break;
                        }
                    }
                    if visible { visible_count += 1; }
                }
                out.set(x, y, visible_count as f32 / total as f32);
            }
        }
        out
    }

    // ── Import / Export ───────────────────────────────────────────────────────

    /// Serialize to raw f32 binary (little-endian).
    /// Format: [width: u32][height: u32][data: f32 * width * height]
    pub fn to_raw_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.data.len() * 4);
        out.extend_from_slice(&(self.width  as u32).to_le_bytes());
        out.extend_from_slice(&(self.height as u32).to_le_bytes());
        for &v in &self.data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Deserialize from raw f32 binary.
    pub fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 { return None; }
        let width  = u32::from_le_bytes(bytes[0..4].try_into().ok()?) as usize;
        let height = u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize;
        if bytes.len() < 8 + width * height * 4 { return None; }
        let mut data = Vec::with_capacity(width * height);
        for i in 0..(width * height) {
            let off = 8 + i * 4;
            let v = f32::from_le_bytes(bytes[off..off + 4].try_into().ok()?);
            data.push(v);
        }
        Some(Self { width, height, data })
    }

    /// Export as 8-bit grayscale PNG bytes (raw PNG, no external dep).
    /// Uses a minimal PNG encoder.
    pub fn to_png_8bit(&self) -> Vec<u8> {
        let pixels: Vec<u8> = self.data.iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();
        encode_png_grayscale(self.width as u32, self.height as u32, &pixels, 8)
    }

    /// Export as 16-bit grayscale PNG bytes.
    pub fn to_png_16bit(&self) -> Vec<u8> {
        let pixels: Vec<u8> = self.data.iter().flat_map(|&v| {
            let val = (v.clamp(0.0, 1.0) * 65535.0) as u16;
            val.to_be_bytes()
        }).collect();
        encode_png_grayscale(self.width as u32, self.height as u32, &pixels, 16)
    }

    /// Import from 8-bit grayscale raw pixel data.
    pub fn from_png_8bit(width: usize, height: usize, pixels: &[u8]) -> Self {
        let data = pixels.iter().map(|&p| p as f32 / 255.0).collect();
        Self { width, height, data }
    }

    /// Import from 16-bit big-endian grayscale pixel data.
    pub fn from_png_16bit(width: usize, height: usize, pixels: &[u8]) -> Self {
        let mut data = Vec::with_capacity(width * height);
        for i in 0..(width * height) {
            let hi = pixels[i * 2] as u16;
            let lo = pixels[i * 2 + 1] as u16;
            data.push(((hi << 8) | lo) as f32 / 65535.0);
        }
        Self { width, height, data }
    }
}

/// Minimal PNG encoder for grayscale images (8 or 16 bit depth).
/// Implements enough of PNG spec for valid output without external deps.
fn encode_png_grayscale(width: u32, height: u32, pixels: &[u8], bit_depth: u8) -> Vec<u8> {
    use std::io::Write;

    // PNG signature
    let mut out: Vec<u8> = vec![137, 80, 78, 71, 13, 10, 26, 10];

    let write_chunk = |out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]| {
        let len = data.len() as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(tag);
        out.extend_from_slice(data);
        let crc = png_crc(tag, data);
        out.extend_from_slice(&crc.to_be_bytes());
    };

    // IHDR
    let mut ihdr = Vec::new();
    let _ = ihdr.write_all(&width.to_be_bytes());
    let _ = ihdr.write_all(&height.to_be_bytes());
    ihdr.push(bit_depth); // bit depth
    ihdr.push(0);  // color type: grayscale
    ihdr.push(0);  // compression
    ihdr.push(0);  // filter
    ihdr.push(0);  // interlace
    write_chunk(&mut out, b"IHDR", &ihdr);

    // IDAT: filter type 0 (None) for each row
    let bytes_per_pixel = if bit_depth == 16 { 2 } else { 1 };
    let row_bytes = width as usize * bytes_per_pixel;
    let mut raw = Vec::with_capacity((row_bytes + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0); // filter type None
        raw.extend_from_slice(&pixels[row * row_bytes..(row + 1) * row_bytes]);
    }
    let compressed = deflate_no_compress(&raw);
    write_chunk(&mut out, b"IDAT", &compressed);

    // IEND
    write_chunk(&mut out, b"IEND", &[]);

    out
}

/// CRC32 for PNG chunks.
fn png_crc(tag: &[u8], data: &[u8]) -> u32 {
    let table = crc32_table();
    let mut crc = 0xFFFF_FFFFu32;
    for &b in tag.iter().chain(data.iter()) {
        crc = (crc >> 8) ^ table[((crc ^ b as u32) & 0xFF) as usize];
    }
    !crc
}

fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for n in 0..256u32 {
        let mut c = n;
        for _ in 0..8 { c = if c & 1 != 0 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 }; }
        table[n as usize] = c;
    }
    table
}

/// Minimal deflate "store" (no compression) implementation for PNG IDAT.
fn deflate_no_compress(data: &[u8]) -> Vec<u8> {
    // zlib header: CMF=0x78, FLG=0x01 (no dict, check bits)
    let mut out: Vec<u8> = vec![0x78, 0x01];
    const BLOCK_SIZE: usize = 65535;
    let mut pos = 0;
    while pos < data.len() {
        let end = (pos + BLOCK_SIZE).min(data.len());
        let is_last = end == data.len();
        out.push(if is_last { 1 } else { 0 }); // BFINAL | (BTYPE=0 << 1)
        let len = (end - pos) as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&data[pos..end]);
        pos = end;
    }
    if data.is_empty() {
        out.push(1); // final block
        out.extend_from_slice(&[0, 0, 0xFF, 0xFF]); // len=0, nlen=~0
    }
    // Adler-32 checksum
    let (s1, s2) = data.iter().fold((1u32, 0u32), |(s1, s2), &b| {
        let s1 = (s1 + b as u32) % 65521;
        ((s1 + s2) % 65521, (s1 + s2) % 65521)
    });
    let adler = (s2 << 16) | s1;
    out.extend_from_slice(&adler.to_be_bytes());
    out
}

// ── Diamond-Square Algorithm ──────────────────────────────────────────────────

/// Generates terrain using the Diamond-Square (Midpoint Displacement) algorithm.
pub struct DiamondSquare;

impl DiamondSquare {
    /// Generate a heightmap of size `(size+1) x (size+1)` where `size` must be a power of 2.
    /// `roughness` controls fractal dimension (0.0 = smooth, 1.0 = rough).
    pub fn generate(size: usize, roughness: f32, seed: u64) -> HeightMap {
        let n = size + 1;
        let mut rng = Rng::new(seed);
        let mut map = HeightMap::new(n, n);

        // Seed corners
        map.set(0,    0,    rng.next_f32());
        map.set(size, 0,    rng.next_f32());
        map.set(0,    size, rng.next_f32());
        map.set(size, size, rng.next_f32());

        let mut step = size;
        let mut scale = roughness;

        while step > 1 {
            let half = step / 2;

            // Diamond step
            let mut y = 0;
            while y < size {
                let mut x = 0;
                while x < size {
                    let avg = (map.get(x, y)
                        + map.get(x + step, y)
                        + map.get(x, y + step)
                        + map.get(x + step, y + step)) * 0.25;
                    let rand = rng.next_f32_range(-scale, scale);
                    map.set(x + half, y + half, (avg + rand).clamp(0.0, 1.0));
                    x += step;
                }
                y += step;
            }

            // Square step
            let mut y = 0;
            while y <= size {
                let mut x = (y / half % 2) * half;
                while x <= size {
                    let mut sum = 0.0f32;
                    let mut count = 0;
                    if x >= half {
                        sum += map.get(x - half, y);
                        count += 1;
                    }
                    if x + half <= size {
                        sum += map.get(x + half, y);
                        count += 1;
                    }
                    if y >= half {
                        sum += map.get(x, y - half);
                        count += 1;
                    }
                    if y + half <= size {
                        sum += map.get(x, y + half);
                        count += 1;
                    }
                    let avg = if count > 0 { sum / count as f32 } else { 0.5 };
                    let rand = rng.next_f32_range(-scale, scale);
                    map.set(x, y, (avg + rand).clamp(0.0, 1.0));
                    x += step;
                }
                y += half;
            }

            step = half;
            scale *= 2.0f32.powf(-roughness);
        }
        map
    }
}

// ── Fractal Noise ─────────────────────────────────────────────────────────────

/// Generates terrain using layered (fractal) gradient noise.
pub struct FractalNoise;

impl FractalNoise {
    /// Generate a heightmap.
    ///
    /// - `octaves`: number of noise layers (4–8 typical)
    /// - `lacunarity`: frequency multiplier per octave (2.0 typical)
    /// - `persistence`: amplitude multiplier per octave (0.5 typical)
    /// - `scale`: overall frequency scale
    pub fn generate(
        width: usize,
        height: usize,
        octaves: usize,
        lacunarity: f32,
        persistence: f32,
        scale: f32,
        seed: u64,
    ) -> HeightMap {
        let noise = GradientNoise::new(seed);
        let mut map = HeightMap::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let mut value = 0.0f32;
                let mut amplitude = 1.0f32;
                let mut frequency = scale;
                let mut max_val = 0.0f32;
                for _oct in 0..octaves {
                    let nx = x as f32 / width  as f32 * frequency;
                    let ny = y as f32 / height as f32 * frequency;
                    value += noise.noise2d(nx, ny) * amplitude;
                    max_val += amplitude;
                    amplitude *= persistence;
                    frequency *= lacunarity;
                }
                map.set(x, y, (value / max_val).clamp(0.0, 1.0));
            }
        }
        map
    }

    /// Generate fractal Brownian motion terrain with domain warping.
    pub fn generate_warped(
        width: usize,
        height: usize,
        octaves: usize,
        seed: u64,
    ) -> HeightMap {
        let noise1 = GradientNoise::new(seed);
        let noise2 = GradientNoise::new(seed.wrapping_add(137));
        let noise3 = GradientNoise::new(seed.wrapping_add(271));
        let mut map = HeightMap::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let px = x as f32 / width  as f32 * 4.0;
                let py = y as f32 / height as f32 * 4.0;
                // First warp layer
                let wx = noise1.noise2d(px, py) * 2.0 - 1.0;
                let wy = noise2.noise2d(px + 5.2, py + 1.3) * 2.0 - 1.0;
                // Second warp layer
                let wx2 = noise2.noise2d(px + wx, py + wy);
                let wy2 = noise3.noise2d(px + wx + 1.7, py + wy + 9.2);
                // Final sample
                let mut value = 0.0f32;
                let mut amp = 1.0f32;
                let mut freq = 1.0f32;
                let mut max_amp = 0.0f32;
                for _oct in 0..octaves {
                    let sx = (px + wx2) * freq;
                    let sy = (py + wy2) * freq;
                    value += noise3.noise2d(sx, sy) * amp;
                    max_amp += amp;
                    amp *= 0.5;
                    freq *= 2.0;
                }
                map.set(x, y, (value / max_amp).clamp(0.0, 1.0));
            }
        }
        map
    }
}

// ── Voronoi Plates ────────────────────────────────────────────────────────────

/// Simulates tectonic plates using Voronoi diagrams.
pub struct VoronoiPlates;

impl VoronoiPlates {
    /// Generate tectonic plate terrain.
    ///
    /// `num_plates`: number of tectonic plates (8–32 typical).
    /// Plate boundaries create mountain ranges, interiors become plains/oceans.
    pub fn generate(width: usize, height: usize, num_plates: usize, seed: u64) -> HeightMap {
        let mut rng = Rng::new(seed);

        // Generate plate centers with random positions
        let mut centers: Vec<(f32, f32)> = (0..num_plates)
            .map(|_| (rng.next_f32() * width as f32, rng.next_f32() * height as f32))
            .collect();

        // Assign each plate a base elevation (ocean vs continent)
        let plate_elevations: Vec<f32> = (0..num_plates)
            .map(|_| if rng.next_f32() < 0.4 { rng.next_f32_range(0.0, 0.35) } else { rng.next_f32_range(0.4, 0.7) })
            .collect();

        // Assign plate movement vectors
        let plate_velocities: Vec<(f32, f32)> = (0..num_plates)
            .map(|_| {
                let angle = rng.next_f32() * std::f32::consts::TAU;
                (angle.cos() * 0.5, angle.sin() * 0.5)
            })
            .collect();

        // For each cell, find nearest plate and second-nearest for boundaries
        let mut map = HeightMap::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let px = x as f32;
                let py = y as f32;
                let mut dists: Vec<(f32, usize)> = centers.iter().enumerate()
                    .map(|(i, &(cx, cy))| {
                        let dx = px - cx;
                        let dy = py - cy;
                        (dx * dx + dy * dy, i)
                    })
                    .collect();
                dists.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                let (d0, p0) = dists[0];
                let (d1, _p1) = dists[1];
                let base_h = plate_elevations[p0];
                // Boundary = close to Voronoi edge
                let boundary_t = 1.0 - ((d1.sqrt() - d0.sqrt()) / (width.min(height) as f32 * 0.05)).clamp(0.0, 1.0);
                // Convergent boundary → mountains, divergent → rifts
                let v0 = plate_velocities[p0];
                let dir = ((px - centers[p0].0).signum(), (py - centers[p0].1).signum());
                let convergence = -(v0.0 * dir.0 + v0.1 * dir.1).clamp(-1.0, 1.0);
                let mountain_bonus = boundary_t * convergence.max(0.0) * 0.4;
                let rift_penalty   = boundary_t * (-convergence).max(0.0) * 0.15;
                let h = (base_h + mountain_bonus - rift_penalty).clamp(0.0, 1.0);
                map.set(x, y, h);
            }
        }

        // Slight blur for natural-looking plates
        map.blur(3);
        map.normalize();
        map
    }
}

// ── Perlin Terrain ────────────────────────────────────────────────────────────

/// Multi-octave Perlin terrain with additional continental shaping.
pub struct PerlinTerrain;

impl PerlinTerrain {
    /// Generate terrain by combining multiple Perlin noise octaves.
    pub fn generate(
        width: usize,
        height: usize,
        octaves: usize,
        scale: f32,
        seed: u64,
    ) -> HeightMap {
        // Continental base: low-frequency shape
        let base = FractalNoise::generate(width, height, 2, 2.0, 0.5, scale * 0.2, seed);
        // Detail: higher frequency features
        let detail = FractalNoise::generate(width, height, octaves, 2.1, 0.48, scale, seed.wrapping_add(9999));
        let mut map = HeightMap::new(width, height);
        for i in 0..(width * height) {
            // Continental shape dominates, detail adds local variation
            map.data[i] = (base.data[i] * 0.6 + detail.data[i] * 0.4).clamp(0.0, 1.0);
        }
        map.normalize();
        map
    }

    /// Generate terrain with mountain ridges using warped noise + ridge fold.
    pub fn generate_with_ridges(width: usize, height: usize, seed: u64) -> HeightMap {
        let mut base = FractalNoise::generate_warped(width, height, 6, seed);
        base.ridge_noise(4);
        base.normalize();
        base
    }
}

// ── Hydraulic Erosion ─────────────────────────────────────────────────────────

/// Simulates water-driven erosion.
pub struct HydraulicErosion;

impl HydraulicErosion {
    /// Erode a heightmap using particle-based hydraulic erosion.
    ///
    /// Each iteration simulates a water droplet that flows downhill,
    /// eroding and depositing sediment.
    pub fn erode(
        map: &mut HeightMap,
        iterations: usize,
        rain_amount: f32,
        sediment_capacity: f32,
        evaporation: f32,
        seed: u64,
    ) {
        let mut rng = Rng::new(seed);
        let w = map.width;
        let h = map.height;

        for _ in 0..iterations {
            // Spawn droplet at random position
            let mut px = rng.next_f32() * (w - 2) as f32 + 1.0;
            let mut py = rng.next_f32() * (h - 2) as f32 + 1.0;
            let mut water = rain_amount;
            let mut sediment = 0.0f32;
            let mut vel_x = 0.0f32;
            let mut vel_y = 0.0f32;
            let inertia = 0.3f32;
            let gravity = 4.0f32;
            let erosion_speed = 0.3f32;
            let deposit_speed = 0.3f32;

            for _step in 0..128 {
                let xi = px as usize;
                let yi = py as usize;
                if xi + 1 >= w || yi + 1 >= h { break; }

                // Compute gradient
                let tx = px - xi as f32;
                let ty = py - yi as f32;
                let h00 = map.get(xi, yi);
                let h10 = map.get(xi + 1, yi);
                let h01 = map.get(xi, yi + 1);
                let h11 = map.get(xi + 1, yi + 1);
                let gx = (h10 - h00) * (1.0 - ty) + (h11 - h01) * ty;
                let gy = (h01 - h00) * (1.0 - tx) + (h11 - h10) * tx;
                let height_at = h00 * (1.0 - tx) * (1.0 - ty)
                    + h10 * tx * (1.0 - ty)
                    + h01 * (1.0 - tx) * ty
                    + h11 * tx * ty;

                // Update velocity
                vel_x = vel_x * inertia - gx * (1.0 - inertia) * gravity;
                vel_y = vel_y * inertia - gy * (1.0 - inertia) * gravity;
                let speed = (vel_x * vel_x + vel_y * vel_y).sqrt().max(0.001);
                vel_x /= speed;
                vel_y /= speed;

                let new_px = px + vel_x;
                let new_py = py + vel_y;
                if new_px < 1.0 || new_px >= (w - 1) as f32 || new_py < 1.0 || new_py >= (h - 1) as f32 { break; }

                let new_h = map.sample_bilinear(new_px, new_py);
                let delta_h = new_h - height_at;

                let capacity = sediment_capacity * speed * water * (-delta_h).max(0.001);
                if sediment > capacity || delta_h > 0.0 {
                    // Deposit sediment
                    let deposit = if delta_h > 0.0 {
                        delta_h.min(sediment)
                    } else {
                        (sediment - capacity) * deposit_speed
                    };
                    sediment -= deposit;
                    // Spread deposit around current cell
                    let w0 = (1.0 - tx) * (1.0 - ty);
                    let w1 = tx * (1.0 - ty);
                    let w2 = (1.0 - tx) * ty;
                    let w3 = tx * ty;
                    let cur00 = map.get(xi, yi);
                    let cur10 = map.get(xi + 1, yi);
                    let cur01 = map.get(xi, yi + 1);
                    let cur11 = map.get(xi + 1, yi + 1);
                    map.set(xi,     yi,     (cur00 + deposit * w0).clamp(0.0, 1.0));
                    map.set(xi + 1, yi,     (cur10 + deposit * w1).clamp(0.0, 1.0));
                    map.set(xi,     yi + 1, (cur01 + deposit * w2).clamp(0.0, 1.0));
                    map.set(xi + 1, yi + 1, (cur11 + deposit * w3).clamp(0.0, 1.0));
                } else {
                    // Erode
                    let erode_amount = (capacity - sediment) * erosion_speed;
                    let erode_radius = 2usize;
                    let mut total_weight = 0.0f32;
                    let mut weights = [[0.0f32; 5]; 5];
                    for dy in 0..=erode_radius * 2 {
                        for dx in 0..=erode_radius * 2 {
                            let ddx = dx as i32 - erode_radius as i32;
                            let ddy = dy as i32 - erode_radius as i32;
                            let w = (1.0 - (ddx * ddx + ddy * ddy) as f32 / (erode_radius * erode_radius + 1) as f32).max(0.0);
                            weights[dy][dx] = w;
                            total_weight += w;
                        }
                    }
                    for dy in 0..=erode_radius * 2 {
                        for dx in 0..=erode_radius * 2 {
                            let ddx = dx as i32 - erode_radius as i32;
                            let ddy = dy as i32 - erode_radius as i32;
                            let nx = (xi as i32 + ddx) as usize;
                            let ny = (yi as i32 + ddy) as usize;
                            if nx < w && ny < h {
                                let w = weights[dy][dx] / total_weight;
                                let cur = map.get(nx, ny);
                                map.set(nx, ny, (cur - erode_amount * w).clamp(0.0, 1.0));
                            }
                        }
                    }
                    sediment += erode_amount;
                }

                water *= 1.0 - evaporation;
                if water < 0.001 { break; }
                px = new_px;
                py = new_py;
            }
        }
    }
}

// ── Thermal Erosion ───────────────────────────────────────────────────────────

/// Simulates thermal weathering (slope-driven material movement).
pub struct ThermalErosion;

impl ThermalErosion {
    /// Erode by moving material downhill when slope exceeds `talus_angle` (in height units).
    pub fn erode(map: &mut HeightMap, iterations: usize, talus_angle: f32) {
        let w = map.width;
        let h = map.height;
        let dirs: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for _ in 0..iterations {
            for y in 0..h {
                for x in 0..w {
                    let h0 = map.get(x, y);
                    let mut total_diff = 0.0f32;
                    let mut max_diff = 0.0f32;
                    for (dx, dy) in &dirs {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                            let nh = map.get(nx as usize, ny as usize);
                            let diff = h0 - nh;
                            if diff > talus_angle {
                                total_diff += diff - talus_angle;
                                if diff > max_diff { max_diff = diff; }
                            }
                        }
                    }
                    if total_diff <= 0.0 { continue; }
                    for (dx, dy) in &dirs {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                            let nh = map.get(nx as usize, ny as usize);
                            let diff = h0 - nh;
                            if diff > talus_angle {
                                let frac = (diff - talus_angle) / total_diff;
                                let transfer = frac * (diff - talus_angle) * 0.5;
                                let cur0 = map.get(x, y);
                                let cur_n = map.get(nx as usize, ny as usize);
                                map.set(x, y, (cur0 - transfer).clamp(0.0, 1.0));
                                map.set(nx as usize, ny as usize, (cur_n + transfer).clamp(0.0, 1.0));
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Wind Erosion ──────────────────────────────────────────────────────────────

/// Simulates aeolian (wind-driven) erosion and deposition.
pub struct WindErosion;

impl WindErosion {
    /// Erode map with wind coming from `wind_dir` direction.
    /// Wind picks up material from windward slopes and deposits on lee slopes.
    pub fn erode(map: &mut HeightMap, wind_dir: Vec2, iterations: usize) {
        let w = map.width;
        let h = map.height;
        let wind = wind_dir.normalize_or_zero();
        let step_x = wind.x;
        let step_y = wind.y;
        let saltation_dist = 3usize;
        let erosion_rate = 0.002f32;
        let deposition_rate = 0.003f32;

        for _ in 0..iterations {
            let mut delta = vec![0.0f32; w * h];
            for y in 0..h {
                for x in 0..w {
                    let h0 = map.get(x, y);
                    // Check upwind cell
                    let ux = x as f32 - step_x * saltation_dist as f32;
                    let uy = y as f32 - step_y * saltation_dist as f32;
                    if ux >= 0.0 && ux < w as f32 && uy >= 0.0 && uy < h as f32 {
                        let upwind_h = map.sample_bilinear(ux, uy);
                        if upwind_h > h0 {
                            // Pick up material from upwind
                            let source_xi = ux as usize;
                            let source_yi = uy as usize;
                            let erode = erosion_rate * (upwind_h - h0);
                            if source_xi < w && source_yi < h {
                                delta[source_yi * w + source_xi] -= erode;
                                delta[y * w + x] += erode * (1.0 - deposition_rate);
                            }
                        }
                    }
                }
            }
            for i in 0..(w * h) {
                map.data[i] = (map.data[i] + delta[i]).clamp(0.0, 1.0);
            }
        }
        map.blur(1);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heightmap_new() {
        let m = HeightMap::new(64, 64);
        assert_eq!(m.data.len(), 64 * 64);
        assert!(m.data.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn test_heightmap_get_set() {
        let mut m = HeightMap::new(16, 16);
        m.set(3, 7, 0.75);
        assert_eq!(m.get(3, 7), 0.75);
        assert_eq!(m.get(100, 100), 0.0); // out of bounds
    }

    #[test]
    fn test_bilinear_sampling() {
        let mut m = HeightMap::new(4, 4);
        m.set(1, 1, 1.0);
        let v = m.sample_bilinear(1.5, 1.5);
        assert!(v > 0.0 && v <= 1.0);
    }

    #[test]
    fn test_cubic_sampling() {
        let mut m = HeightMap::new(8, 8);
        m.set(3, 3, 0.8);
        let v = m.sample_cubic(3.2, 3.2);
        // Just check it doesn't panic and is in reasonable range
        assert!(v >= -0.5 && v <= 1.5);
    }

    #[test]
    fn test_normalize() {
        let mut m = HeightMap::new(4, 4);
        for (i, v) in m.data.iter_mut().enumerate() { *v = i as f32; }
        m.normalize();
        let mn = m.min_value();
        let mx = m.max_value();
        assert!((mn - 0.0).abs() < 1e-5);
        assert!((mx - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_blur() {
        let mut m = HeightMap::new(32, 32);
        m.data[16 * 32 + 16] = 1.0;
        m.blur(3);
        // After blur, center should be reduced and neighbors increased
        assert!(m.get(16, 16) < 1.0);
        assert!(m.get(15, 16) > 0.0);
    }

    #[test]
    fn test_terrace() {
        let mut m = HeightMap::new(16, 16);
        for (i, v) in m.data.iter_mut().enumerate() { *v = i as f32 / (16 * 16) as f32; }
        m.terrace(4);
        let unique: Vec<f32> = {
            let mut vals: Vec<f32> = m.data.clone();
            vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
            vals.dedup();
            vals
        };
        assert!(unique.len() <= 5, "terrace should produce at most levels+1 unique values");
    }

    #[test]
    fn test_island_mask() {
        let mut m = HeightMap::new(64, 64);
        for v in m.data.iter_mut() { *v = 1.0; }
        m.island_mask(2.0);
        assert!(m.get(0, 0) < 0.1);
        // Center should remain high
        assert!(m.get(32, 32) > 0.5);
    }

    #[test]
    fn test_diamond_square() {
        let m = DiamondSquare::generate(64, 0.7, 42);
        assert_eq!(m.width, 65);
        assert_eq!(m.height, 65);
        let mn = m.min_value();
        let mx = m.max_value();
        assert!(mn >= 0.0 && mx <= 1.0);
    }

    #[test]
    fn test_fractal_noise() {
        let m = FractalNoise::generate(64, 64, 6, 2.0, 0.5, 4.0, 12345);
        assert_eq!(m.data.len(), 64 * 64);
        assert!(m.min_value() >= 0.0);
        assert!(m.max_value() <= 1.0);
    }

    #[test]
    fn test_voronoi_plates() {
        let m = VoronoiPlates::generate(64, 64, 8, 99);
        assert_eq!(m.data.len(), 64 * 64);
        let mn = m.min_value();
        let mx = m.max_value();
        assert!(mn >= 0.0 && mx <= 1.0);
    }

    #[test]
    fn test_perlin_terrain() {
        let m = PerlinTerrain::generate(64, 64, 6, 4.0, 7);
        assert_eq!(m.data.len(), 64 * 64);
        let mn = m.min_value();
        let mx = m.max_value();
        assert!((mn - 0.0).abs() < 1e-4);
        assert!((mx - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_hydraulic_erosion() {
        let mut m = DiamondSquare::generate(32, 0.8, 1);
        let before: f32 = m.data.iter().sum();
        HydraulicErosion::erode(&mut m, 500, 1.0, 8.0, 0.05, 77);
        let after: f32 = m.data.iter().sum();
        // Erosion should generally reduce total height (more deposition than formation)
        // Just check it doesn't panic and values remain valid
        assert!(m.min_value() >= 0.0);
        assert!(m.max_value() <= 1.0);
        let _ = (before, after);
    }

    #[test]
    fn test_thermal_erosion() {
        let mut m = DiamondSquare::generate(32, 0.9, 5);
        ThermalErosion::erode(&mut m, 20, 0.05);
        assert!(m.min_value() >= 0.0);
        assert!(m.max_value() <= 1.0);
    }

    #[test]
    fn test_wind_erosion() {
        let mut m = DiamondSquare::generate(32, 0.7, 3);
        WindErosion::erode(&mut m, Vec2::new(1.0, 0.3), 10);
        assert!(m.min_value() >= 0.0);
        assert!(m.max_value() <= 1.0);
    }

    #[test]
    fn test_slope_map() {
        let m = DiamondSquare::generate(32, 0.7, 42);
        let s = m.slope_map();
        assert_eq!(s.data.len(), m.data.len());
        assert!(s.min_value() >= 0.0);
        assert!(s.max_value() <= 1.0);
    }

    #[test]
    fn test_raw_bytes_roundtrip() {
        let m = FractalNoise::generate(16, 16, 4, 2.0, 0.5, 3.0, 42);
        let bytes = m.to_raw_bytes();
        let m2 = HeightMap::from_raw_bytes(&bytes).unwrap();
        assert_eq!(m.width, m2.width);
        assert_eq!(m.height, m2.height);
        for (a, b) in m.data.iter().zip(m2.data.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_png_8bit() {
        let m = FractalNoise::generate(16, 16, 4, 2.0, 0.5, 3.0, 42);
        let png = m.to_png_8bit();
        // PNG signature is 8 bytes
        assert!(png.len() > 8);
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_normal_at() {
        let m = DiamondSquare::generate(32, 0.7, 42);
        let n = m.normal_at(16, 16);
        assert!((n.length() - 1.0).abs() < 1e-4);
    }
}
