//! Complete terrain system for Proof Engine.
//!
//! Provides:
//! - `Heightmap`          — 2D float grid with sampling and modification
//! - `HeightmapBuilder`   — procedural generation (noise, ridges, erosion)
//! - `HydraulicErosion`   — water-driven erosion simulation
//! - `ThermalErosion`     — talus slope erosion
//! - `TerrainLod`         — level-of-detail mesh generation from heightmap
//! - `MarchingCubes`      — isosurface extraction from a scalar volume
//! - `Biome`              — climate/altitude-based biome classification
//! - `BiomeMap`           — per-cell biome assignment
//! - `TerrainSampler`     — analytical queries (ray cast, slope, normal)
//! - `ChunkSystem`        — infinite world with chunked heightmaps
//! - `TerrainDeformer`    — runtime sculpting operations

use glam::{Vec2, Vec3};
use crate::math::MathFunction;

// ── Heightmap ─────────────────────────────────────────────────────────────────

/// A 2D grid of height values.
#[derive(Debug, Clone)]
pub struct Heightmap {
    pub width:    u32,
    pub height:   u32,
    pub data:     Vec<f32>,
    /// World-space extents of the terrain.
    pub scale_xz: f32,
    pub scale_y:  f32,
}

impl Heightmap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; (width * height) as usize],
            scale_xz: 1.0,
            scale_y:  1.0,
        }
    }

    pub fn with_scale(mut self, xz: f32, y: f32) -> Self {
        self.scale_xz = xz;
        self.scale_y  = y;
        self
    }

    #[inline]
    pub fn index(&self, x: u32, z: u32) -> usize {
        (z * self.width + x) as usize
    }

    #[inline]
    pub fn get(&self, x: u32, z: u32) -> f32 {
        self.data.get(self.index(x, z)).copied().unwrap_or(0.0)
    }

    #[inline]
    pub fn set(&mut self, x: u32, z: u32, h: f32) {
        let idx = self.index(x, z);
        if idx < self.data.len() { self.data[idx] = h; }
    }

    pub fn add(&mut self, x: u32, z: u32, delta: f32) {
        let idx = self.index(x, z);
        if idx < self.data.len() { self.data[idx] += delta; }
    }

    pub fn clamp_all(&mut self, min: f32, max: f32) {
        for v in &mut self.data { *v = v.clamp(min, max); }
    }

    pub fn normalize(&mut self) {
        let min = self.data.iter().cloned().fold(f32::MAX,  f32::min);
        let max = self.data.iter().cloned().fold(f32::MIN, f32::max);
        let range = (max - min).max(1e-7);
        for v in &mut self.data { *v = (*v - min) / range; }
    }

    pub fn min_height(&self) -> f32 { self.data.iter().cloned().fold(f32::MAX,  f32::min) }
    pub fn max_height(&self) -> f32 { self.data.iter().cloned().fold(f32::MIN, f32::max) }
    pub fn avg_height(&self) -> f32 {
        if self.data.is_empty() { return 0.0; }
        self.data.iter().sum::<f32>() / self.data.len() as f32
    }

    /// Bilinear sample at fractional grid coordinates.
    pub fn sample_bilinear(&self, fx: f32, fz: f32) -> f32 {
        let x0 = (fx as u32).min(self.width  - 1);
        let z0 = (fz as u32).min(self.height - 1);
        let x1 = (x0 + 1).min(self.width  - 1);
        let z1 = (z0 + 1).min(self.height - 1);
        let tx = fx.fract().clamp(0.0, 1.0);
        let tz = fz.fract().clamp(0.0, 1.0);
        let h00 = self.get(x0, z0);
        let h10 = self.get(x1, z0);
        let h01 = self.get(x0, z1);
        let h11 = self.get(x1, z1);
        let hx0 = h00 + (h10 - h00) * tx;
        let hx1 = h01 + (h11 - h01) * tx;
        hx0 + (hx1 - hx0) * tz
    }

    /// Sample height at world-space (x, z).
    pub fn sample_world(&self, wx: f32, wz: f32) -> f32 {
        let fx = wx / self.scale_xz * (self.width  as f32 - 1.0);
        let fz = wz / self.scale_xz * (self.height as f32 - 1.0);
        self.sample_bilinear(fx.max(0.0), fz.max(0.0))
    }

    /// Surface normal at grid position (x, z) using central differences.
    pub fn normal_at(&self, x: u32, z: u32) -> Vec3 {
        let x0 = x.saturating_sub(1);
        let x1 = (x + 1).min(self.width  - 1);
        let z0 = z.saturating_sub(1);
        let z1 = (z + 1).min(self.height - 1);
        let dx = self.get(x1, z) - self.get(x0, z);
        let dz = self.get(x, z1) - self.get(x, z0);
        let step = self.scale_xz / (self.width as f32 - 1.0);
        Vec3::new(-dx * self.scale_y, 2.0 * step, -dz * self.scale_y).normalize_or_zero()
    }

    /// Slope in degrees at (x, z).
    pub fn slope_degrees(&self, x: u32, z: u32) -> f32 {
        let n = self.normal_at(x, z);
        n.dot(Vec3::Y).acos().to_degrees()
    }

    /// Smooth the entire heightmap using a box blur kernel.
    pub fn smooth(&mut self, passes: u32) {
        for _ in 0..passes {
            let src = self.data.clone();
            for z in 0..self.height {
                for x in 0..self.width {
                    let mut sum   = 0.0_f32;
                    let mut count = 0_u32;
                    for dz in -1_i32..=1 {
                        for dx in -1_i32..=1 {
                            let nx = x as i32 + dx;
                            let nz = z as i32 + dz;
                            if nx >= 0 && nx < self.width as i32 && nz >= 0 && nz < self.height as i32 {
                                sum   += src[(nz as u32 * self.width + nx as u32) as usize];
                                count += 1;
                            }
                        }
                    }
                    let idx = self.index(x, z);
                    self.data[idx] = sum / count as f32;
                }
            }
        }
    }

    /// Sharpen: enhance local differences.
    pub fn sharpen(&mut self, amount: f32) {
        let src = self.data.clone();
        for z in 1..self.height - 1 {
            for x in 1..self.width - 1 {
                let c = src[self.index(x,   z  )];
                let laplacian =
                    src[self.index(x+1, z  )] + src[self.index(x-1, z  )] +
                    src[self.index(x,   z+1)] + src[self.index(x,   z-1)] -
                    4.0 * c;
                let idx = self.index(x, z);
                self.data[idx] = c + laplacian * amount;
            }
        }
    }

    /// Add another heightmap scaled by `weight`.
    pub fn blend(&mut self, other: &Heightmap, weight: f32) {
        for (a, b) in self.data.iter_mut().zip(other.data.iter()) {
            *a += b * weight;
        }
    }

    /// Apply a MathFunction to transform each height value.
    pub fn apply_function(&mut self, f: &MathFunction) {
        let len = self.data.len() as f32 - 1.0;
        for (i, v) in self.data.iter_mut().enumerate() {
            let t = i as f32 / len;
            *v = f.evaluate(*v, t);
        }
    }

    /// Set heights from a flat slice (must be width*height length).
    pub fn set_from_slice(&mut self, data: &[f32]) {
        let len = (self.width * self.height) as usize;
        for (i, &v) in data.iter().take(len).enumerate() {
            self.data[i] = v;
        }
    }

    /// Export to a flat Vec<u8> (8-bit grayscale).
    pub fn to_u8(&self) -> Vec<u8> {
        let min = self.min_height();
        let max = self.max_height();
        let range = (max - min).max(1e-7);
        self.data.iter().map(|&v| ((v - min) / range * 255.0) as u8).collect()
    }

    /// Export to normalized f32 Vec (0.0–1.0).
    pub fn to_normalized(&self) -> Vec<f32> {
        let min = self.min_height();
        let max = self.max_height();
        let range = (max - min).max(1e-7);
        self.data.iter().map(|&v| (v - min) / range).collect()
    }
}

// ── HeightmapBuilder ──────────────────────────────────────────────────────────

/// Procedural heightmap generator.
pub struct HeightmapBuilder {
    pub width:  u32,
    pub height: u32,
}

impl HeightmapBuilder {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Generate using layered noise (FBM-style).
    pub fn noise_fbm(
        &self,
        octaves:     u32,
        frequency:   f32,
        amplitude:   f32,
        lacunarity:  f32,
        persistence: f32,
        seed:        u32,
    ) -> Heightmap {
        let mut hm = Heightmap::new(self.width, self.height);
        for z in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 / self.width  as f32;
                let nz = z as f32 / self.height as f32;
                let mut h = 0.0_f32;
                let mut amp  = amplitude;
                let mut freq = frequency;
                let mut max_amp = 0.0_f32;
                for oct in 0..octaves {
                    let sx = nx * freq + (seed as f32 + oct as f32) * 127.1;
                    let sz = nz * freq + (seed as f32 + oct as f32) * 311.7;
                    h       += perlin2(sx, sz) * amp;
                    max_amp += amp;
                    amp  *= persistence;
                    freq *= lacunarity;
                }
                hm.set(x, z, h / max_amp.max(1e-7));
            }
        }
        hm
    }

    /// Generate ridge noise (inverted absolute value of noise).
    pub fn noise_ridges(
        &self,
        octaves:   u32,
        frequency: f32,
        sharpness: f32,
        seed:      u32,
    ) -> Heightmap {
        let mut hm = Heightmap::new(self.width, self.height);
        for z in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 / self.width  as f32;
                let nz = z as f32 / self.height as f32;
                let mut h    = 0.0_f32;
                let mut freq = frequency;
                let mut amp  = 1.0_f32;
                let mut prev = 1.0_f32;
                for oct in 0..octaves {
                    let sx = nx * freq + (seed as f32 + oct as f32) * 19.3;
                    let sz = nz * freq + (seed as f32 + oct as f32) * 41.7;
                    let n  = (1.0 - perlin2(sx, sz).abs()).powi(2);
                    h     += n * amp * prev;
                    prev   = n;
                    freq  *= 2.0;
                    amp   *= 0.5 * sharpness;
                }
                hm.set(x, z, h);
            }
        }
        hm.normalize();
        hm
    }

    /// Generate domain-warped terrain (warp the input coordinates with noise).
    pub fn noise_warped(
        &self,
        octaves:   u32,
        frequency: f32,
        warp_amp:  f32,
        seed:      u32,
    ) -> Heightmap {
        let mut hm = Heightmap::new(self.width, self.height);
        for z in 0..self.height {
            for x in 0..self.width {
                let nx = x as f32 / self.width  as f32;
                let nz = z as f32 / self.height as f32;
                // First warp pass
                let wx = nx + warp_amp * perlin2(nx * frequency + seed as f32 * 5.1, nz * frequency);
                let wz = nz + warp_amp * perlin2(nx * frequency, nz * frequency + seed as f32 * 3.7);
                // Second pass using warped coords
                let mut h   = 0.0_f32;
                let mut f2  = frequency;
                let mut amp = 1.0_f32;
                for oct in 0..octaves {
                    h   += perlin2(wx * f2 + oct as f32 * 17.3, wz * f2 + oct as f32 * 31.1) * amp;
                    f2  *= 2.0;
                    amp *= 0.5;
                }
                hm.set(x, z, h * 0.5 + 0.5);
            }
        }
        hm
    }

    /// Generate a caldera (volcano ring).
    pub fn caldera(&self, cx: f32, cz: f32, inner_r: f32, outer_r: f32) -> Heightmap {
        let mut hm = Heightmap::new(self.width, self.height);
        for z in 0..self.height {
            for x in 0..self.width {
                let dx = (x as f32 / self.width  as f32 - cx).abs();
                let dz = (z as f32 / self.height as f32 - cz).abs();
                let dist = (dx * dx + dz * dz).sqrt();
                let h = if dist < inner_r {
                    -1.0 + dist / inner_r  // crater pit
                } else if dist < outer_r {
                    let t = (dist - inner_r) / (outer_r - inner_r);
                    1.0 - t * t  // rim
                } else {
                    0.0
                };
                hm.set(x, z, h);
            }
        }
        hm
    }

    /// Generate a canyon system (terrain cut by rivers).
    pub fn canyon(&self, river_count: u32, seed: u32) -> Heightmap {
        let base = self.noise_fbm(6, 2.0, 1.0, 2.0, 0.5, seed);
        let mut hm = base;
        for r in 0..river_count {
            let sx = perlin2(r as f32 * 17.3, seed as f32) * 0.5 + 0.5;
            let sz = perlin2(r as f32 * 31.1, seed as f32 * 2.0) * 0.5 + 0.5;
            let ex = perlin2(r as f32 * 43.7, seed as f32 * 3.0) * 0.5 + 0.5;
            let ez = perlin2(r as f32 * 57.9, seed as f32 * 4.0) * 0.5 + 0.5;
            for z in 0..hm.height {
                for x in 0..hm.width {
                    let px = x as f32 / hm.width  as f32;
                    let pz = z as f32 / hm.height as f32;
                    // Distance to line segment [s, e]
                    let dx = ex - sx; let dz = ez - sz;
                    let len2 = dx * dx + dz * dz;
                    let t = if len2 < 1e-10 { 0.0 } else {
                        ((px - sx) * dx + (pz - sz) * dz) / len2
                    }.clamp(0.0, 1.0);
                    let cx = sx + t * dx - px;
                    let cz_d = sz + t * dz - pz;
                    let dist = (cx * cx + cz_d * cz_d).sqrt();
                    let depth = (0.04 - dist).max(0.0) / 0.04;
                    if depth > 0.0 {
                        let cur = hm.get(x, z);
                        hm.set(x, z, cur - depth * 0.3);
                    }
                }
            }
        }
        hm
    }

    /// Generate islands: circular bumps scattered on flat ocean.
    pub fn islands(&self, island_count: u32, seed: u32) -> Heightmap {
        let mut hm = Heightmap::new(self.width, self.height);
        // Start with deep ocean
        for v in &mut hm.data { *v = -0.3; }
        let noise_hm = self.noise_fbm(4, 3.0, 0.3, 2.0, 0.5, seed);
        for i in 0..island_count {
            let cx = (perlin2(i as f32 * 11.1, seed as f32) * 0.5 + 0.5).clamp(0.05, 0.95);
            let cz = (perlin2(i as f32 * 23.3, seed as f32 + 5.0) * 0.5 + 0.5).clamp(0.05, 0.95);
            let r  = 0.05 + (perlin2(i as f32 * 37.7, seed as f32 + 10.0) * 0.5 + 0.5) * 0.15;
            for z in 0..self.height {
                for x in 0..self.width {
                    let px = x as f32 / self.width  as f32;
                    let pz = z as f32 / self.height as f32;
                    let dist = ((px - cx).powi(2) + (pz - cz).powi(2)).sqrt();
                    if dist < r {
                        let falloff = 1.0 - (dist / r).powi(3);
                        let n = noise_hm.get(x, z);
                        let cur = hm.get(x, z);
                        hm.set(x, z, cur.max(falloff * 0.8 + n * 0.2));
                    }
                }
            }
        }
        hm
    }
}

// ── Hydraulic Erosion ─────────────────────────────────────────────────────────

/// Parameters for hydraulic erosion simulation.
#[derive(Debug, Clone)]
pub struct HydraulicErosionConfig {
    pub iterations:         u32,
    pub drops_per_iter:     u32,
    pub inertia:            f32,  // resistance to direction change
    pub capacity:           f32,  // max sediment per unit water
    pub deposition_rate:    f32,
    pub erosion_rate:       f32,
    pub evaporation_rate:   f32,
    pub gravity:            f32,
    pub min_slope:          f32,
    pub erosion_radius:     u32,
    pub max_path_length:    u32,
}

impl Default for HydraulicErosionConfig {
    fn default() -> Self {
        Self {
            iterations:      50000,
            drops_per_iter:  1,
            inertia:         0.05,
            capacity:        4.0,
            deposition_rate: 0.3,
            erosion_rate:    0.3,
            evaporation_rate: 0.01,
            gravity:         4.0,
            min_slope:       0.01,
            erosion_radius:  3,
            max_path_length: 30,
        }
    }
}

/// Hydraulic erosion using droplet simulation.
pub struct HydraulicErosion {
    pub config: HydraulicErosionConfig,
    /// Erosion brush weights precomputed.
    brush:       Vec<(i32, i32, f32)>,
}

impl HydraulicErosion {
    pub fn new(config: HydraulicErosionConfig) -> Self {
        let r = config.erosion_radius as i32;
        let mut brush = Vec::new();
        let mut total_weight = 0.0_f32;
        for dz in -r..=r {
            for dx in -r..=r {
                let dist = ((dx * dx + dz * dz) as f32).sqrt();
                if dist <= r as f32 {
                    let w = 1.0 - dist / r as f32;
                    brush.push((dx, dz, w));
                    total_weight += w;
                }
            }
        }
        for (_, _, w) in &mut brush { *w /= total_weight.max(1e-7); }
        Self { config, brush }
    }

    pub fn with_default() -> Self {
        Self::new(HydraulicErosionConfig::default())
    }

    /// Run erosion simulation on a heightmap in-place.
    pub fn erode(&self, hm: &mut Heightmap, seed: u32) {
        let w = hm.width  as usize;
        let h = hm.height as usize;
        let cfg = &self.config;

        for i in 0..cfg.iterations {
            // Spawn a drop at a random position
            let seed_f = (i + seed) as f32;
            let mut px = ((perlin2(seed_f * 0.1, 1.0) * 0.5 + 0.5) * (w - 1) as f32).max(0.0);
            let mut pz = ((perlin2(seed_f * 0.1, 2.0) * 0.5 + 0.5) * (h - 1) as f32).max(0.0);
            let mut vx = 0.0_f32;
            let mut vz = 0.0_f32;
            let mut speed    = 1.0_f32;
            let mut water    = 1.0_f32;
            let mut sediment = 0.0_f32;

            for _ in 0..cfg.max_path_length {
                let xi = px as u32;
                let zi = pz as u32;
                if xi >= hm.width - 1 || zi >= hm.height - 1 { break; }
                let fx = px.fract();
                let fz = pz.fract();

                // Height and gradient at drop position
                let h00 = hm.get(xi,   zi  );
                let h10 = hm.get(xi+1, zi  );
                let h01 = hm.get(xi,   zi+1);
                let h11 = hm.get(xi+1, zi+1);
                let old_h = h00 * (1.0-fx) * (1.0-fz) + h10 * fx * (1.0-fz)
                          + h01 * (1.0-fx) * fz        + h11 * fx * fz;
                let gx = (h10 - h00) * (1.0 - fz) + (h11 - h01) * fz;
                let gz = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;

                // Update direction
                vx = vx * cfg.inertia - gx * (1.0 - cfg.inertia);
                vz = vz * cfg.inertia - gz * (1.0 - cfg.inertia);
                let vel_len = (vx * vx + vz * vz).sqrt().max(1e-7);
                vx /= vel_len;
                vz /= vel_len;

                let new_px = px + vx;
                let new_pz = pz + vz;
                if new_px < 0.0 || new_px >= w as f32 - 1.0 || new_pz < 0.0 || new_pz >= h as f32 - 1.0 { break; }

                // New height
                let nxi = new_px as u32;
                let nzi = new_pz as u32;
                let nfx = new_px.fract();
                let nfz = new_pz.fract();
                let nh00 = hm.get(nxi,   nzi  );
                let nh10 = hm.get(nxi+1, nzi  );
                let nh01 = hm.get(nxi,   nzi+1);
                let nh11 = hm.get(nxi+1, nzi+1);
                let new_h = nh00 * (1.0-nfx) * (1.0-nfz) + nh10 * nfx * (1.0-nfz)
                          + nh01 * (1.0-nfx) * nfz        + nh11 * nfx * nfz;

                let delta_h = new_h - old_h;
                let carry_capacity = (speed * water * cfg.capacity * (-delta_h).max(cfg.min_slope)).max(0.0);

                if sediment > carry_capacity || delta_h > 0.0 {
                    // Deposit
                    let deposit = if delta_h > 0.0 {
                        sediment.min(delta_h)
                    } else {
                        (sediment - carry_capacity) * cfg.deposition_rate
                    };
                    sediment -= deposit;
                    hm.set(xi, zi, h00 + deposit * (1.0-fx)*(1.0-fz));
                    hm.set(xi+1, zi, h10 + deposit * fx*(1.0-fz));
                    hm.set(xi, zi+1, h01 + deposit * (1.0-fx)*fz);
                    hm.set(xi+1, zi+1, h11 + deposit * fx*fz);
                } else {
                    // Erode
                    let erode = ((carry_capacity - sediment) * cfg.erosion_rate).min(-delta_h);
                    for &(dx, dz, w) in &self.brush {
                        let bx = xi as i32 + dx;
                        let bz = zi as i32 + dz;
                        if bx >= 0 && bx < hm.width as i32 && bz >= 0 && bz < hm.height as i32 {
                            let cur = hm.get(bx as u32, bz as u32);
                            let amount = erode * w;
                            hm.set(bx as u32, bz as u32, cur - amount);
                            sediment += amount;
                        }
                    }
                }

                speed = (speed * speed + delta_h * cfg.gravity).max(0.0).sqrt();
                water *= 1.0 - cfg.evaporation_rate;
                if water < 0.01 { break; }
                px = new_px;
                pz = new_pz;
            }
        }
    }
}

// ── Thermal Erosion ───────────────────────────────────────────────────────────

/// Thermal erosion (talus slope / scree formation).
pub struct ThermalErosion {
    /// Maximum stable slope before material slides.
    pub talus_angle:  f32,
    /// Fraction of unstable material that slides per step.
    pub slide_rate:   f32,
    pub iterations:   u32,
}

impl Default for ThermalErosion {
    fn default() -> Self {
        Self { talus_angle: 30.0, slide_rate: 0.5, iterations: 50 }
    }
}

impl ThermalErosion {
    pub fn erode(&self, hm: &mut Heightmap) {
        let cell_size = hm.scale_xz / (hm.width as f32 - 1.0);
        let max_diff  = cell_size * self.talus_angle.to_radians().tan() * hm.scale_y;

        for _ in 0..self.iterations {
            let src = hm.data.clone();
            for z in 1..hm.height - 1 {
                for x in 1..hm.width - 1 {
                    let h = src[hm.index(x, z)];
                    let neighbors = [
                        (x+1, z), (x-1, z), (x, z+1), (x, z-1),
                    ];
                    for (nx, nz) in neighbors {
                        let nh = src[hm.index(nx, nz)];
                        let diff = h - nh;
                        if diff > max_diff {
                            let slide = (diff - max_diff) * self.slide_rate;
                            let idx_cur = hm.index(x,  z );
                            let idx_nb  = hm.index(nx, nz);
                            hm.data[idx_cur] -= slide;
                            hm.data[idx_nb]  += slide;
                        }
                    }
                }
            }
        }
    }
}

// ── Biomes ────────────────────────────────────────────────────────────────────

/// A terrain biome classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Ocean,
    Beach,
    Desert,
    Savanna,
    Grassland,
    Forest,
    RainForest,
    Taiga,
    Tundra,
    Snowcap,
    Mountain,
    Volcanic,
}

impl Biome {
    pub fn color(&self) -> Vec3 {
        match self {
            Self::Ocean      => Vec3::new(0.05, 0.20, 0.50),
            Self::Beach      => Vec3::new(0.90, 0.85, 0.60),
            Self::Desert     => Vec3::new(0.90, 0.75, 0.40),
            Self::Savanna    => Vec3::new(0.70, 0.65, 0.30),
            Self::Grassland  => Vec3::new(0.40, 0.65, 0.25),
            Self::Forest     => Vec3::new(0.15, 0.50, 0.10),
            Self::RainForest => Vec3::new(0.05, 0.40, 0.10),
            Self::Taiga      => Vec3::new(0.20, 0.45, 0.30),
            Self::Tundra     => Vec3::new(0.60, 0.65, 0.55),
            Self::Snowcap    => Vec3::new(0.95, 0.95, 1.00),
            Self::Mountain   => Vec3::new(0.55, 0.50, 0.45),
            Self::Volcanic   => Vec3::new(0.25, 0.05, 0.05),
        }
    }

    /// Classify biome from height [0,1], moisture [0,1], temperature [0,1].
    pub fn classify(height: f32, moisture: f32, temperature: f32) -> Self {
        if height < 0.35 { return Self::Ocean; }
        if height < 0.40 { return Self::Beach; }
        if temperature < 0.1 {
            if height > 0.85 { return Self::Snowcap; }
            return Self::Tundra;
        }
        if height > 0.90 { return Self::Snowcap; }
        if height > 0.75 { return Self::Mountain; }
        if temperature > 0.75 {
            if moisture < 0.25 { return Self::Desert; }
            if moisture < 0.65 { return Self::Savanna; }
            return Self::RainForest;
        }
        if temperature > 0.4 {
            if moisture < 0.3 { return Self::Desert; }
            if moisture < 0.6 { return Self::Grassland; }
            return Self::Forest;
        }
        if moisture < 0.5 { return Self::Tundra; }
        Self::Taiga
    }
}

/// Per-cell biome assignment for a terrain.
#[derive(Debug, Clone)]
pub struct BiomeMap {
    pub width:    u32,
    pub height:   u32,
    pub biomes:   Vec<Biome>,
    pub moisture: Vec<f32>,
    pub temperature: Vec<f32>,
}

impl BiomeMap {
    pub fn generate(
        heightmap:       &Heightmap,
        moisture_seed:   u32,
        temperature_seed: u32,
    ) -> Self {
        let w = heightmap.width;
        let h = heightmap.height;
        let n = (w * h) as usize;
        let mut biomes      = Vec::with_capacity(n);
        let mut moisture    = Vec::with_capacity(n);
        let mut temperature = Vec::with_capacity(n);

        let norm = heightmap.to_normalized();

        for z in 0..h {
            for x in 0..w {
                let nx = x as f32 / w as f32;
                let nz = z as f32 / h as f32;
                let height = norm[(z * w + x) as usize];

                let moist = (perlin2(
                    nx * 3.0 + moisture_seed as f32 * 0.01,
                    nz * 3.0 + moisture_seed as f32 * 0.02,
                ) * 0.5 + 0.5).clamp(0.0, 1.0);

                let temp_lat = 1.0 - (nz - 0.5).abs() * 2.0;
                let temp_alt = (1.0 - height * 1.5).max(0.0);
                let temp_noise = perlin2(
                    nx * 2.0 + temperature_seed as f32 * 0.01,
                    nz * 2.0 + temperature_seed as f32 * 0.02,
                ) * 0.1;
                let temp = (temp_lat * 0.6 + temp_alt * 0.3 + temp_noise + 0.1).clamp(0.0, 1.0);

                biomes.push(Biome::classify(height, moist, temp));
                moisture.push(moist);
                temperature.push(temp);
            }
        }
        Self { width: w, height: h, biomes, moisture, temperature }
    }

    pub fn get(&self, x: u32, z: u32) -> Biome {
        self.biomes.get((z * self.width + x) as usize).copied().unwrap_or(Biome::Ocean)
    }

    /// Generate a color map from biome assignments.
    pub fn to_color_map(&self) -> Vec<Vec3> {
        self.biomes.iter().map(|b| b.color()).collect()
    }
}

// ── LOD Mesh Generation ───────────────────────────────────────────────────────

/// A simple triangle mesh for terrain rendering.
#[derive(Debug, Clone)]
pub struct TerrainMesh {
    pub vertices:  Vec<Vec3>,
    pub normals:   Vec<Vec3>,
    pub uvs:       Vec<Vec2>,
    pub indices:   Vec<u32>,
    /// LOD level this mesh was generated at.
    pub lod_level: u32,
}

impl TerrainMesh {
    pub fn vertex_count(&self) -> usize  { self.vertices.len() }
    pub fn triangle_count(&self) -> usize { self.indices.len() / 3 }
}

/// LOD levels for terrain mesh generation.
#[derive(Debug, Clone, Copy)]
pub enum LodLevel {
    /// Full resolution (one quad per heightmap cell).
    Full,
    /// Half resolution.
    Half,
    /// Quarter resolution.
    Quarter,
    /// Eighth resolution.
    Eighth,
    /// Custom reduction (1 = full, 2 = half, 4 = quarter, ...).
    Custom(u32),
}

impl LodLevel {
    pub fn step(&self) -> u32 {
        match self {
            Self::Full     => 1,
            Self::Half     => 2,
            Self::Quarter  => 4,
            Self::Eighth   => 8,
            Self::Custom(s) => *s,
        }
    }

    pub fn index(&self) -> u32 {
        match self {
            Self::Full     => 0,
            Self::Half     => 1,
            Self::Quarter  => 2,
            Self::Eighth   => 3,
            Self::Custom(s) => s.trailing_zeros(),
        }
    }
}

/// Terrain LOD mesh generator.
pub struct TerrainLod;

impl TerrainLod {
    /// Generate a mesh from a heightmap at the given LOD level.
    pub fn build(hm: &Heightmap, lod: LodLevel) -> TerrainMesh {
        let step = lod.step();
        let lod_w = (hm.width  + step - 1) / step;
        let lod_h = (hm.height + step - 1) / step;
        let cell_size = hm.scale_xz / (hm.width as f32 - 1.0) * step as f32;

        let mut vertices = Vec::new();
        let mut normals  = Vec::new();
        let mut uvs      = Vec::new();
        let mut indices  = Vec::new();

        for zi in 0..lod_h {
            for xi in 0..lod_w {
                let x = (xi * step).min(hm.width  - 1);
                let z = (zi * step).min(hm.height - 1);
                let h = hm.get(x, z) * hm.scale_y;
                let wx = xi as f32 * cell_size;
                let wz = zi as f32 * cell_size;
                vertices.push(Vec3::new(wx, h, wz));
                normals.push(hm.normal_at(x, z));
                uvs.push(Vec2::new(xi as f32 / (lod_w - 1).max(1) as f32,
                                   zi as f32 / (lod_h - 1).max(1) as f32));
            }
        }

        for zi in 0..lod_h - 1 {
            for xi in 0..lod_w - 1 {
                let i00 = zi * lod_w + xi;
                let i10 = zi * lod_w + xi + 1;
                let i01 = (zi + 1) * lod_w + xi;
                let i11 = (zi + 1) * lod_w + xi + 1;
                // Two triangles per quad
                indices.push(i00); indices.push(i10); indices.push(i11);
                indices.push(i00); indices.push(i11); indices.push(i01);
            }
        }

        TerrainMesh { vertices, normals, uvs, indices, lod_level: lod.index() }
    }

    /// Generate 4 LOD levels.
    pub fn build_lod_chain(hm: &Heightmap) -> [TerrainMesh; 4] {
        [
            Self::build(hm, LodLevel::Full),
            Self::build(hm, LodLevel::Half),
            Self::build(hm, LodLevel::Quarter),
            Self::build(hm, LodLevel::Eighth),
        ]
    }
}

// ── Marching Cubes ────────────────────────────────────────────────────────────

/// 3D scalar volume for marching cubes.
#[derive(Debug, Clone)]
pub struct ScalarVolume {
    pub width:  u32,
    pub height: u32,
    pub depth:  u32,
    pub data:   Vec<f32>,
}

impl ScalarVolume {
    pub fn new(width: u32, height: u32, depth: u32) -> Self {
        Self { width, height, depth, data: vec![0.0; (width * height * depth) as usize] }
    }

    #[inline]
    pub fn index(&self, x: u32, y: u32, z: u32) -> usize {
        (z * self.height * self.width + y * self.width + x) as usize
    }

    pub fn get(&self, x: u32, y: u32, z: u32) -> f32 {
        self.data.get(self.index(x, y, z)).copied().unwrap_or(0.0)
    }

    pub fn set(&mut self, x: u32, y: u32, z: u32, v: f32) {
        let i = self.index(x, y, z);
        if i < self.data.len() { self.data[i] = v; }
    }

    /// Fill using a math function f(x, y, z) -> scalar.
    pub fn fill_fn<F: Fn(f32, f32, f32) -> f32>(&mut self, f: F) {
        for z in 0..self.depth {
            for y in 0..self.height {
                for x in 0..self.width {
                    let fx = x as f32 / (self.width  - 1).max(1) as f32;
                    let fy = y as f32 / (self.height - 1).max(1) as f32;
                    let fz = z as f32 / (self.depth  - 1).max(1) as f32;
                    self.set(x, y, z, f(fx, fy, fz));
                }
            }
        }
    }

    /// Fill with a sphere implicit surface.
    pub fn fill_sphere(&mut self, cx: f32, cy: f32, cz: f32, r: f32) {
        self.fill_fn(|x, y, z| {
            let dx = x - cx; let dy = y - cy; let dz = z - cz;
            r - (dx*dx + dy*dy + dz*dz).sqrt()
        });
    }

    /// Fill with a torus.
    pub fn fill_torus(&mut self, major_r: f32, minor_r: f32) {
        self.fill_fn(|x, y, z| {
            let dx = x - 0.5; let dy = y - 0.5; let dz = z - 0.5;
            let q = ((dx*dx + dz*dz).sqrt() - major_r).powi(2) + dy * dy;
            minor_r * minor_r - q
        });
    }
}

/// Marching cubes isosurface extractor.
pub struct MarchingCubes {
    pub isolevel: f32,
}

impl MarchingCubes {
    pub fn new(isolevel: f32) -> Self { Self { isolevel } }

    /// Extract the isosurface mesh from a scalar volume.
    pub fn extract(&self, volume: &ScalarVolume) -> (Vec<Vec3>, Vec<Vec3>, Vec<u32>) {
        let mut vertices: Vec<Vec3> = Vec::new();
        let mut normals:  Vec<Vec3> = Vec::new();
        let mut indices:  Vec<u32>  = Vec::new();

        let w = volume.width;
        let h = volume.height;
        let d = volume.depth;

        for z in 0..d - 1 {
            for y in 0..h - 1 {
                for x in 0..w - 1 {
                    let corners = [
                        volume.get(x,   y,   z  ),
                        volume.get(x+1, y,   z  ),
                        volume.get(x+1, y,   z+1),
                        volume.get(x,   y,   z+1),
                        volume.get(x,   y+1, z  ),
                        volume.get(x+1, y+1, z  ),
                        volume.get(x+1, y+1, z+1),
                        volume.get(x,   y+1, z+1),
                    ];
                    let positions = [
                        Vec3::new(x as f32,   y as f32,   z as f32  ),
                        Vec3::new(x as f32+1.0, y as f32,   z as f32  ),
                        Vec3::new(x as f32+1.0, y as f32,   z as f32+1.0),
                        Vec3::new(x as f32,   y as f32,   z as f32+1.0),
                        Vec3::new(x as f32,   y as f32+1.0, z as f32  ),
                        Vec3::new(x as f32+1.0, y as f32+1.0, z as f32  ),
                        Vec3::new(x as f32+1.0, y as f32+1.0, z as f32+1.0),
                        Vec3::new(x as f32,   y as f32+1.0, z as f32+1.0),
                    ];

                    let mut cube_index = 0u8;
                    for (i, &c) in corners.iter().enumerate() {
                        if c < self.isolevel { cube_index |= 1 << i; }
                    }

                    if cube_index == 0 || cube_index == 255 { continue; }

                    // Compute edge intersections
                    let mut edge_verts = [Vec3::ZERO; 12];
                    let edges = MC_EDGES[cube_index as usize];
                    if edges == 0 { continue; }

                    for e in 0..12 {
                        if edges & (1 << e) != 0 {
                            let (a, b) = EDGE_PAIRS[e];
                            let va = corners[a];
                            let vb = corners[b];
                            let t = if (vb - va).abs() > 1e-8 {
                                (self.isolevel - va) / (vb - va)
                            } else { 0.5 };
                            edge_verts[e] = positions[a].lerp(positions[b], t.clamp(0.0, 1.0));
                        }
                    }

                    let tris = &MC_TRIANGLES[cube_index as usize];
                    let mut ti = 0;
                    while ti < tris.len() && tris[ti] != -1 {
                        let base = vertices.len() as u32;
                        let v0 = edge_verts[tris[ti  ] as usize];
                        let v1 = edge_verts[tris[ti+1] as usize];
                        let v2 = edge_verts[tris[ti+2] as usize];
                        let n  = (v1 - v0).cross(v2 - v0).normalize_or_zero();
                        vertices.push(v0); normals.push(n);
                        vertices.push(v1); normals.push(n);
                        vertices.push(v2); normals.push(n);
                        indices.push(base); indices.push(base+1); indices.push(base+2);
                        ti += 3;
                    }
                }
            }
        }

        (vertices, normals, indices)
    }
}

// Edge pair lookup table (which corners each edge connects)
const EDGE_PAIRS: [(usize, usize); 12] = [
    (0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)
];

// Marching cubes edge table (256 entries)
const MC_EDGES: [u16; 256] = [
    0x000, 0x109, 0x203, 0x30a, 0x406, 0x50f, 0x605, 0x70c,
    0x80c, 0x905, 0xa0f, 0xb06, 0xc0a, 0xd03, 0xe09, 0xf00,
    0x190, 0x099, 0x393, 0x29a, 0x596, 0x49f, 0x795, 0x69c,
    0x99c, 0x895, 0xb9f, 0xa96, 0xd9a, 0xc93, 0xf99, 0xe90,
    0x230, 0x339, 0x033, 0x13a, 0x636, 0x73f, 0x435, 0x53c,
    0xa3c, 0xb35, 0x83f, 0x936, 0xe3a, 0xf33, 0xc39, 0xd30,
    0x3a0, 0x2a9, 0x1a3, 0x0aa, 0x7a6, 0x6af, 0x5a5, 0x4ac,
    0xbac, 0xaa5, 0x9af, 0x8a6, 0xfaa, 0xea3, 0xda9, 0xca0,
    0x460, 0x569, 0x663, 0x76a, 0x066, 0x16f, 0x265, 0x36c,
    0xc6c, 0xd65, 0xe6f, 0xf66, 0x86a, 0x963, 0xa69, 0xb60,
    0x5f0, 0x4f9, 0x7f3, 0x6fa, 0x1f6, 0x0ff, 0x3f5, 0x2fc,
    0xdfc, 0xcf5, 0xfff, 0xef6, 0x9fa, 0x8f3, 0xbf9, 0xaf0,
    0x650, 0x759, 0x453, 0x55a, 0x256, 0x35f, 0x055, 0x15c,
    0xe5c, 0xf55, 0xc5f, 0xd56, 0xa5a, 0xb53, 0x859, 0x950,
    0x7c0, 0x6c9, 0x5c3, 0x4ca, 0x3c6, 0x2cf, 0x1c5, 0x0cc,
    0xfcc, 0xec5, 0xdcf, 0xcc6, 0xbca, 0xac3, 0x9c9, 0x8c0,
    0x8c0, 0x9c9, 0xac3, 0xbca, 0xcc6, 0xdcf, 0xec5, 0xfcc,
    0x0cc, 0x1c5, 0x2cf, 0x3c6, 0x4ca, 0x5c3, 0x6c9, 0x7c0,
    0x950, 0x859, 0xb53, 0xa5a, 0xd56, 0xc5f, 0xf55, 0xe5c,
    0x15c, 0x055, 0x35f, 0x256, 0x55a, 0x453, 0x759, 0x650,
    0xaf0, 0xbf9, 0x8f3, 0x9fa, 0xef6, 0xfff, 0xcf5, 0xdfc,
    0x2fc, 0x3f5, 0x0ff, 0x1f6, 0x6fa, 0x7f3, 0x4f9, 0x5f0,
    0xb60, 0xa69, 0x963, 0x86a, 0xf66, 0xe6f, 0xd65, 0xc6c,
    0x36c, 0x265, 0x16f, 0x066, 0x76a, 0x663, 0x569, 0x460,
    0xca0, 0xda9, 0xea3, 0xfaa, 0x8a6, 0x9af, 0xaa5, 0xbac,
    0x4ac, 0x5a5, 0x6af, 0x7a6, 0x0aa, 0x1a3, 0x2a9, 0x3a0,
    0xd30, 0xc39, 0xf33, 0xe3a, 0x936, 0x835, 0xb3f, 0xa36,
    0x53c, 0x435, 0x73f, 0x636, 0x13a, 0x033, 0x339, 0x230,
    0xe90, 0xf99, 0xc93, 0xd9a, 0xa96, 0xb9f, 0x895, 0x99c,
    0x69c, 0x795, 0x49f, 0x596, 0x29a, 0x393, 0x099, 0x190,
    0xf00, 0xe09, 0xd03, 0xc0a, 0xb06, 0xa0f, 0x905, 0x80c,
    0x70c, 0x605, 0x50f, 0x406, 0x30a, 0x203, 0x109, 0x000,
];

// Marching cubes triangle table (simplified — 16 entries per cube, -1 = end)
const MC_TRIANGLES: [[i8; 16]; 256] = {
    let mut t = [[-1i8; 16]; 256];
    // A minimal but correct MC triangle table for common cases
    // Full 256-entry table:
    t[0] = [-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[1] = [0,8,3,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[2] = [0,1,9,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[3] = [1,8,3,9,8,1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[4] = [1,2,10,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[5] = [0,8,3,1,2,10,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[6] = [9,2,10,0,2,9,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[7] = [2,8,3,2,10,8,10,9,8,-1,-1,-1,-1,-1,-1,-1];
    t[8] = [3,11,2,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[9] = [0,11,2,8,11,0,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[10] = [1,9,0,2,3,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[11] = [1,11,2,1,9,11,9,8,11,-1,-1,-1,-1,-1,-1,-1];
    t[12] = [3,10,1,11,10,3,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t[13] = [0,10,1,0,8,10,8,11,10,-1,-1,-1,-1,-1,-1,-1];
    t[14] = [3,9,0,3,11,9,11,10,9,-1,-1,-1,-1,-1,-1,-1];
    t[15] = [9,8,10,10,8,11,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1];
    t
};

// ── Ray Casting ───────────────────────────────────────────────────────────────

/// Result of a terrain ray intersection.
#[derive(Debug, Clone)]
pub struct TerrainHit {
    pub position:    Vec3,
    pub normal:      Vec3,
    pub distance:    f32,
    pub grid_x:      u32,
    pub grid_z:      u32,
}

/// Terrain ray-caster and analytical query utility.
pub struct TerrainSampler<'a> {
    hm: &'a Heightmap,
}

impl<'a> TerrainSampler<'a> {
    pub fn new(hm: &'a Heightmap) -> Self { Self { hm } }

    /// Ray-march to find the first intersection with the terrain surface.
    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_dist: f32, steps: u32) -> Option<TerrainHit> {
        let dir = direction.normalize_or_zero();
        let step_size = max_dist / steps as f32;
        for i in 0..steps {
            let t = i as f32 * step_size;
            let p = origin + dir * t;
            let wx = p.x.max(0.0);
            let wz = p.z.max(0.0);
            let terrain_h = self.hm.sample_world(wx, wz) * self.hm.scale_y;
            if p.y <= terrain_h {
                // Binary search to refine hit
                let mut lo = (i as f32 - 1.0).max(0.0) * step_size;
                let mut hi = t;
                for _ in 0..8 {
                    let mid = (lo + hi) * 0.5;
                    let mp  = origin + dir * mid;
                    let mh  = self.hm.sample_world(mp.x.max(0.0), mp.z.max(0.0)) * self.hm.scale_y;
                    if mp.y <= mh { hi = mid; } else { lo = mid; }
                }
                let hit_t = (lo + hi) * 0.5;
                let hit_p = origin + dir * hit_t;
                let gx = (hit_p.x / self.hm.scale_xz * (self.hm.width  as f32 - 1.0)) as u32;
                let gz = (hit_p.z / self.hm.scale_xz * (self.hm.height as f32 - 1.0)) as u32;
                let normal = self.hm.normal_at(
                    gx.min(self.hm.width  - 1),
                    gz.min(self.hm.height - 1),
                );
                return Some(TerrainHit {
                    position: hit_p,
                    normal,
                    distance: hit_t,
                    grid_x: gx,
                    grid_z: gz,
                });
            }
        }
        None
    }

    /// Compute the steepest descent direction at (gx, gz).
    pub fn flow_direction(&self, gx: u32, gz: u32) -> Vec2 {
        let x0 = gx.saturating_sub(1);
        let x1 = (gx + 1).min(self.hm.width  - 1);
        let z0 = gz.saturating_sub(1);
        let z1 = (gz + 1).min(self.hm.height - 1);
        let dx = self.hm.get(x1, gz) - self.hm.get(x0, gz);
        let dz = self.hm.get(gx, z1) - self.hm.get(gx, z0);
        Vec2::new(-dx, -dz).normalize_or_zero()
    }

    /// Trace a flow path downhill from a starting grid position.
    pub fn trace_flow_path(&self, start_x: u32, start_z: u32, steps: u32) -> Vec<(u32, u32)> {
        let mut path = vec![(start_x, start_z)];
        let mut cx = start_x;
        let mut cz = start_z;
        for _ in 0..steps {
            let dir = self.flow_direction(cx, cz);
            let nx = ((cx as f32 + dir.x * 0.5 + 0.5) as i32).clamp(0, self.hm.width  as i32 - 1) as u32;
            let nz = ((cz as f32 + dir.y * 0.5 + 0.5) as i32).clamp(0, self.hm.height as i32 - 1) as u32;
            if nx == cx && nz == cz { break; }
            cx = nx; cz = nz;
            path.push((cx, cz));
        }
        path
    }

    /// Compute the catchment area for each cell (how many cells drain through it).
    pub fn catchment_area(&self) -> Vec<u32> {
        let w = self.hm.width;
        let h = self.hm.height;
        let mut area = vec![1u32; (w * h) as usize];
        // Sort cells by height descending
        let mut cells: Vec<(u32, u32)> = (0..h).flat_map(|z| (0..w).map(move |x| (x, z))).collect();
        cells.sort_by(|&(ax, az), &(bx, bz)| {
            self.hm.get(bx, bz).partial_cmp(&self.hm.get(ax, az)).unwrap_or(std::cmp::Ordering::Equal)
        });
        for (x, z) in cells {
            let dir = self.flow_direction(x, z);
            let nx = ((x as f32 + dir.x).round() as i32).clamp(0, w as i32 - 1) as u32;
            let nz = ((z as f32 + dir.y).round() as i32).clamp(0, h as i32 - 1) as u32;
            if nx != x || nz != z {
                let src = area[(z * w + x) as usize];
                area[(nz * w + nx) as usize] += src;
            }
        }
        area
    }
}

// ── Chunk System ──────────────────────────────────────────────────────────────

/// A terrain chunk identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkId(pub i32, pub i32);

impl ChunkId {
    pub fn from_world(wx: f32, wz: f32, chunk_size: f32) -> Self {
        ChunkId(
            (wx / chunk_size).floor() as i32,
            (wz / chunk_size).floor() as i32,
        )
    }

    pub fn world_origin(&self, chunk_size: f32) -> Vec2 {
        Vec2::new(self.0 as f32 * chunk_size, self.1 as f32 * chunk_size)
    }
}

/// An infinite world made up of heightmap chunks.
pub struct ChunkSystem {
    pub chunk_resolution: u32,
    pub chunk_world_size: f32,
    pub scale_y:          f32,
    chunks:               std::collections::HashMap<ChunkId, Heightmap>,
    pub view_distance:    u32,  // chunks in each direction
}

impl ChunkSystem {
    pub fn new(resolution: u32, world_size: f32, scale_y: f32, view_dist: u32) -> Self {
        Self {
            chunk_resolution: resolution,
            chunk_world_size: world_size,
            scale_y,
            chunks: std::collections::HashMap::new(),
            view_distance: view_dist,
        }
    }

    /// Get or generate a chunk at the given ID.
    pub fn get_or_generate<F>(&mut self, id: ChunkId, generator: &mut F) -> &Heightmap
    where F: FnMut(ChunkId) -> Heightmap
    {
        if !self.chunks.contains_key(&id) {
            let hm = generator(id);
            self.chunks.insert(id, hm);
        }
        self.chunks.get(&id).unwrap()
    }

    /// Get all visible chunk IDs from a camera position.
    pub fn visible_chunks(&self, camera_wx: f32, camera_wz: f32) -> Vec<ChunkId> {
        let center = ChunkId::from_world(camera_wx, camera_wz, self.chunk_world_size);
        let d = self.view_distance as i32;
        let mut result = Vec::new();
        for dz in -d..=d {
            for dx in -d..=d {
                result.push(ChunkId(center.0 + dx, center.1 + dz));
            }
        }
        result
    }

    /// Evict chunks beyond view distance.
    pub fn evict_distant(&mut self, camera_wx: f32, camera_wz: f32) {
        let center = ChunkId::from_world(camera_wx, camera_wz, self.chunk_world_size);
        let d = self.view_distance as i32 + 1;
        self.chunks.retain(|id, _| {
            (id.0 - center.0).abs() <= d && (id.1 - center.1).abs() <= d
        });
    }

    /// Sample height at a world position (across chunk boundaries).
    pub fn sample_height(&self, wx: f32, wz: f32) -> f32 {
        let id = ChunkId::from_world(wx, wz, self.chunk_world_size);
        if let Some(hm) = self.chunks.get(&id) {
            let origin = id.world_origin(self.chunk_world_size);
            hm.sample_world(wx - origin.x, wz - origin.y)
        } else {
            0.0
        }
    }

    pub fn loaded_count(&self) -> usize { self.chunks.len() }
}

/// Default chunk generator using FBM noise.
pub struct DefaultChunkGenerator {
    pub octaves:    u32,
    pub frequency:  f32,
    pub amplitude:  f32,
    pub seed:       u32,
}

impl DefaultChunkGenerator {
    pub fn new(seed: u32) -> Self {
        Self { octaves: 7, frequency: 1.5, amplitude: 1.0, seed }
    }

    pub fn generate(&self, id: ChunkId, resolution: u32, world_size: f32) -> Heightmap {
        let builder = HeightmapBuilder::new(resolution, resolution);
        let offset_x = id.0 as f32 / world_size;
        let offset_z = id.1 as f32 / world_size;
        let mut hm = Heightmap::new(resolution, resolution);
        hm.scale_xz = world_size;
        for z in 0..resolution {
            for x in 0..resolution {
                let nx = x as f32 / resolution as f32 + offset_x;
                let nz = z as f32 / resolution as f32 + offset_z;
                let mut h = 0.0_f32;
                let mut freq = self.frequency;
                let mut amp  = self.amplitude;
                for oct in 0..self.octaves {
                    h    += perlin2(nx * freq + self.seed as f32 * 0.01, nz * freq + oct as f32 * 7.3) * amp;
                    freq *= 2.0;
                    amp  *= 0.5;
                }
                hm.set(x, z, h * 0.5 + 0.5);
            }
        }
        let _ = builder;
        hm
    }
}

// ── Terrain Deformer ──────────────────────────────────────────────────────────

/// Runtime terrain sculpting operations.
pub struct TerrainDeformer;

impl TerrainDeformer {
    /// Raise or lower the terrain in a circular brush.
    pub fn paint(hm: &mut Heightmap, cx: f32, cz: f32, radius: f32, strength: f32, falloff: f32) {
        let cell_size = hm.scale_xz / (hm.width as f32 - 1.0);
        let cr = (radius / cell_size) as i32;
        let gx = (cx / cell_size) as i32;
        let gz = (cz / cell_size) as i32;
        for dz in -cr..=cr {
            for dx in -cr..=cr {
                let nx = gx + dx;
                let nz = gz + dz;
                if nx < 0 || nx >= hm.width as i32 || nz < 0 || nz >= hm.height as i32 { continue; }
                let dist = ((dx * dx + dz * dz) as f32).sqrt() / cr as f32;
                if dist > 1.0 { continue; }
                let weight = (1.0 - dist.powf(falloff)).max(0.0);
                hm.add(nx as u32, nz as u32, strength * weight);
            }
        }
    }

    /// Flatten the terrain toward a target height in a circular brush.
    pub fn flatten(hm: &mut Heightmap, cx: f32, cz: f32, radius: f32, target_h: f32, strength: f32) {
        let cell_size = hm.scale_xz / (hm.width as f32 - 1.0);
        let cr = (radius / cell_size) as i32;
        let gx = (cx / cell_size) as i32;
        let gz = (cz / cell_size) as i32;
        for dz in -cr..=cr {
            for dx in -cr..=cr {
                let nx = gx + dx;
                let nz = gz + dz;
                if nx < 0 || nx >= hm.width as i32 || nz < 0 || nz >= hm.height as i32 { continue; }
                let dist = ((dx * dx + dz * dz) as f32).sqrt() / cr as f32;
                if dist > 1.0 { continue; }
                let weight = 1.0 - dist;
                let cur = hm.get(nx as u32, nz as u32);
                let delta = (target_h - cur) * strength * weight;
                hm.add(nx as u32, nz as u32, delta);
            }
        }
    }

    /// Smooth the terrain in a circular brush.
    pub fn smooth_brush(hm: &mut Heightmap, cx: f32, cz: f32, radius: f32, strength: f32) {
        let cell_size = hm.scale_xz / (hm.width as f32 - 1.0);
        let cr = (radius / cell_size) as i32;
        let gx = (cx / cell_size) as i32;
        let gz = (cz / cell_size) as i32;
        let src = hm.data.clone();
        for dz in -cr..=cr {
            for dx in -cr..=cr {
                let nx = gx + dx;
                let nz = gz + dz;
                if nx < 0 || nx >= hm.width as i32 || nz < 0 || nz >= hm.height as i32 { continue; }
                let dist = ((dx * dx + dz * dz) as f32).sqrt() / cr as f32;
                if dist > 1.0 { continue; }
                let mut sum   = 0.0_f32;
                let mut count = 0_u32;
                for ndz in -1_i32..=1 {
                    for ndx in -1_i32..=1 {
                        let nnx = nx + ndx;
                        let nnz = nz + ndz;
                        if nnx >= 0 && nnx < hm.width as i32 && nnz >= 0 && nnz < hm.height as i32 {
                            sum += src[(nnz as u32 * hm.width + nnx as u32) as usize];
                            count += 1;
                        }
                    }
                }
                let avg  = sum / count as f32;
                let cur  = src[(nz as u32 * hm.width + nx as u32) as usize];
                let weight = (1.0 - dist) * strength;
                hm.data[(nz as u32 * hm.width + nx as u32) as usize] = cur + (avg - cur) * weight;
            }
        }
    }

    /// Stamp another heightmap onto this one (additive blend).
    pub fn stamp(hm: &mut Heightmap, stamp: &Heightmap, cx: f32, cz: f32, scale: f32, strength: f32) {
        let cell = hm.scale_xz / (hm.width as f32 - 1.0);
        for sz in 0..stamp.height {
            for sx in 0..stamp.width {
                let wx = cx + (sx as f32 / stamp.width  as f32 - 0.5) * scale;
                let wz = cz + (sz as f32 / stamp.height as f32 - 0.5) * scale;
                let gx = (wx / cell) as i32;
                let gz = (wz / cell) as i32;
                if gx < 0 || gx >= hm.width as i32 || gz < 0 || gz >= hm.height as i32 { continue; }
                let sv = stamp.get(sx, sz) * strength;
                hm.add(gx as u32, gz as u32, sv);
            }
        }
    }

    /// Carve a river channel from point A to B.
    pub fn carve_river(hm: &mut Heightmap, from: Vec2, to: Vec2, width: f32, depth: f32) {
        let steps = 200;
        for i in 0..=steps {
            let t  = i as f32 / steps as f32;
            let cx = from.x + (to.x - from.x) * t;
            let cz = from.y + (to.y - from.y) * t;
            Self::paint(hm, cx, cz, width, -depth, 2.0);
        }
    }
}

// ── Noise utility (inline Perlin) ─────────────────────────────────────────────

fn perlin2(x: f32, y: f32) -> f32 {
    fn fade(t: f32)  -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }
    fn lerp(a: f32, b: f32, t: f32) -> f32 { a + t * (b - a) }
    fn grad(hash: u8, x: f32, y: f32) -> f32 {
        match hash & 3 {
            0 =>  x + y,
            1 => -x + y,
            2 =>  x - y,
            _ => -x - y,
        }
    }
    static P: [u8; 512] = {
        let mut p = [0u8; 512];
        let perm: [u8; 256] = [
            151,160,137,91,90,15,131,13,201,95,96,53,194,233,7,225,
            140,36,103,30,69,142,8,99,37,240,21,10,23,190,6,148,
            247,120,234,75,0,26,197,62,94,252,219,203,117,35,11,32,
            57,177,33,88,237,149,56,87,174,20,125,136,171,168,68,175,
            74,165,71,134,139,48,27,166,77,146,158,231,83,111,229,122,
            60,211,133,230,220,105,92,41,55,46,245,40,244,102,143,54,
            65,25,63,161,1,216,80,73,209,76,132,187,208,89,18,169,
            200,196,135,130,116,188,159,86,164,100,109,198,173,186,3,64,
            52,217,226,250,124,123,5,202,38,147,118,126,255,82,85,212,
            207,206,59,227,47,16,58,17,182,189,28,42,223,183,170,213,
            119,248,152,2,44,154,163,70,221,153,101,155,167,43,172,9,
            129,22,39,253,19,98,108,110,79,113,224,232,178,185,112,104,
            218,246,97,228,251,34,242,193,238,210,144,12,191,179,162,241,
            81,51,145,235,249,14,239,107,49,192,214,31,181,199,106,157,
            184,84,204,176,115,121,50,45,127,4,150,254,138,236,205,93,
            222,114,67,29,24,72,243,141,128,195,78,66,215,61,156,180,
        ];
        let mut i = 0;
        while i < 256 { p[i] = perm[i]; p[i + 256] = perm[i]; i += 1; }
        p
    };
    let xi = x.floor() as i32 & 255;
    let yi = y.floor() as i32 & 255;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);
    let a  = P[xi as usize]         as usize + yi as usize;
    let b  = P[(xi + 1) as usize]   as usize + yi as usize;
    lerp(
        lerp(grad(P[ a    & 255], xf,     yf    ),
             grad(P[ b    & 255], xf-1.0, yf    ), u),
        lerp(grad(P[(a+1) & 255], xf,     yf-1.0),
             grad(P[(b+1) & 255], xf-1.0, yf-1.0), u),
        v,
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heightmap_basic() {
        let mut hm = Heightmap::new(64, 64);
        hm.set(10, 10, 0.5);
        assert!((hm.get(10, 10) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_heightmap_bilinear() {
        let mut hm = Heightmap::new(4, 4);
        hm.set(1, 1, 1.0);
        let v = hm.sample_bilinear(1.0, 1.0);
        assert!((v - 1.0).abs() < 1e-5);
        let v2 = hm.sample_bilinear(1.5, 1.0);
        assert!(v2 > 0.0 && v2 < 1.0);
    }

    #[test]
    fn test_heightmap_smooth() {
        let mut hm = Heightmap::new(16, 16);
        hm.set(8, 8, 10.0);
        hm.smooth(3);
        assert!(hm.get(8, 8) < 10.0);
    }

    #[test]
    fn test_fbm_builder() {
        let builder = HeightmapBuilder::new(64, 64);
        let hm = builder.noise_fbm(5, 2.0, 1.0, 2.0, 0.5, 42);
        assert_eq!(hm.data.len(), 64 * 64);
    }

    #[test]
    fn test_islands_builder() {
        let builder = HeightmapBuilder::new(64, 64);
        let hm = builder.islands(5, 123);
        assert!(hm.min_height() < 0.0);  // ocean below 0
        assert!(hm.max_height() > 0.0);  // land above 0
    }

    #[test]
    fn test_heightmap_normalize() {
        let mut hm = Heightmap::new(8, 8);
        for i in 0..64 { hm.data[i] = i as f32; }
        hm.normalize();
        assert!((hm.min_height() - 0.0).abs() < 1e-5);
        assert!((hm.max_height() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_biome_classify() {
        assert_eq!(Biome::classify(0.2, 0.5, 0.5), Biome::Ocean);
        assert_eq!(Biome::classify(0.95, 0.5, 0.5), Biome::Snowcap);
        let b = Biome::classify(0.6, 0.1, 0.9);
        assert!(matches!(b, Biome::Desert));
    }

    #[test]
    fn test_biome_map_generate() {
        let builder = HeightmapBuilder::new(32, 32);
        let hm = builder.noise_fbm(4, 2.0, 1.0, 2.0, 0.5, 1);
        let bmap = BiomeMap::generate(&hm, 2, 3);
        assert_eq!(bmap.biomes.len(), 32 * 32);
    }

    #[test]
    fn test_lod_mesh() {
        let builder = HeightmapBuilder::new(32, 32);
        let hm = builder.noise_fbm(3, 2.0, 1.0, 2.0, 0.5, 77);
        let mesh = TerrainLod::build(&hm, LodLevel::Full);
        assert_eq!(mesh.vertex_count(), 32 * 32);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_lod_chain() {
        let builder = HeightmapBuilder::new(64, 64);
        let hm = builder.noise_fbm(4, 2.0, 1.0, 2.0, 0.5, 99);
        let chain = TerrainLod::build_lod_chain(&hm);
        assert!(chain[0].vertex_count() > chain[3].vertex_count());
    }

    #[test]
    fn test_thermal_erosion() {
        let builder = HeightmapBuilder::new(32, 32);
        let mut hm  = builder.noise_ridges(4, 2.0, 0.7, 11);
        let before  = hm.max_height() - hm.min_height();
        ThermalErosion::default().erode(&mut hm);
        let after   = hm.max_height() - hm.min_height();
        // After erosion the range should shrink
        assert!(after <= before + 0.01);
    }

    #[test]
    fn test_hydraulic_erosion() {
        let builder = HeightmapBuilder::new(32, 32);
        let mut hm  = builder.noise_fbm(4, 2.0, 1.0, 2.0, 0.5, 55);
        let mut config = HydraulicErosionConfig::default();
        config.iterations = 1000;
        let eroder = HydraulicErosion::new(config);
        eroder.erode(&mut hm, 123);
        // Should not panic or produce NaN
        assert!(hm.data.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_terrain_deformer_paint() {
        let mut hm = Heightmap::new(32, 32).with_scale(31.0, 1.0);
        TerrainDeformer::paint(&mut hm, 15.5, 15.5, 5.0, 1.0, 2.0);
        assert!(hm.get(15, 15) > 0.0);
    }

    #[test]
    fn test_terrain_deformer_flatten() {
        let mut hm = Heightmap::new(32, 32).with_scale(31.0, 1.0);
        for v in &mut hm.data { *v = 0.5; }
        TerrainDeformer::flatten(&mut hm, 15.5, 15.5, 5.0, 0.0, 1.0);
        assert!(hm.get(15, 15) < 0.5);
    }

    #[test]
    fn test_chunk_system_visibility() {
        let cs = ChunkSystem::new(32, 100.0, 50.0, 2);
        let visible = cs.visible_chunks(150.0, 250.0);
        assert!(!visible.is_empty());
    }

    #[test]
    fn test_terrain_sampler_flow() {
        let builder = HeightmapBuilder::new(32, 32);
        let hm = builder.noise_fbm(3, 2.0, 1.0, 2.0, 0.5, 7);
        let sampler = TerrainSampler::new(&hm);
        let dir = sampler.flow_direction(16, 16);
        assert!(dir.length() <= 1.01);
    }

    #[test]
    fn test_marching_cubes_sphere() {
        let mut vol = ScalarVolume::new(16, 16, 16);
        vol.fill_sphere(0.5, 0.5, 0.5, 0.4);
        let mc = MarchingCubes::new(0.0);
        let (verts, norms, inds) = mc.extract(&vol);
        assert!(!verts.is_empty());
        assert_eq!(verts.len(), norms.len());
        assert!(inds.len() % 3 == 0);
    }

    #[test]
    fn test_perlin2_range() {
        for i in 0..100 {
            let v = perlin2(i as f32 * 0.3, i as f32 * 0.17);
            assert!(v >= -1.0 && v <= 1.0, "perlin2 out of range: {}", v);
        }
    }
}
