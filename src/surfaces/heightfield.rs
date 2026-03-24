//! # Height Field Surfaces
//!
//! Height-map driven surfaces with configurable noise sources, real-time deformation,
//! collision detection, LOD, and chunk-based infinite scrolling.
//!
//! The [`HeightFieldSurface`] type generates a 2D grid of heights from a [`NoiseSource`]
//! and provides normal computation, ray intersection, and mesh export. The [`ChunkManager`]
//! implements infinite scrolling by generating chunks around a camera position and recycling
//! distant ones.

use glam::{Vec2, Vec3};
use std::f32::consts::{PI, TAU};

// ─────────────────────────────────────────────────────────────────────────────
// Noise sources
// ─────────────────────────────────────────────────────────────────────────────

/// Noise generation method for height fields.
#[derive(Debug, Clone)]
pub enum NoiseSource {
    /// Simple Perlin noise at a single frequency.
    Perlin {
        frequency: f32,
        amplitude: f32,
        seed: u32,
    },
    /// Fractional Brownian Motion — multiple octaves of Perlin noise.
    Fbm {
        frequency: f32,
        amplitude: f32,
        octaves: u32,
        lacunarity: f32,
        persistence: f32,
        seed: u32,
    },
    /// Ridged multifractal noise — sharp ridges.
    Ridged {
        frequency: f32,
        amplitude: f32,
        octaves: u32,
        lacunarity: f32,
        gain: f32,
        offset: f32,
        seed: u32,
    },
    /// Domain-warped noise — feeds noise through itself for organic distortion.
    DomainWarped {
        base: Box<NoiseSource>,
        warp_frequency: f32,
        warp_amplitude: f32,
        warp_seed: u32,
    },
    /// Flat surface (height = constant).
    Flat {
        height: f32,
    },
    /// Sinusoidal terrain (for testing).
    Sinusoidal {
        frequency_x: f32,
        frequency_z: f32,
        amplitude: f32,
    },
    /// Composite: sum of multiple noise sources.
    Composite {
        sources: Vec<NoiseSource>,
    },
}

impl Default for NoiseSource {
    fn default() -> Self {
        NoiseSource::Fbm {
            frequency: 0.02,
            amplitude: 30.0,
            octaves: 6,
            lacunarity: 2.0,
            persistence: 0.5,
            seed: 42,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Built-in noise evaluation (std-only, no external crate)
// ─────────────────────────────────────────────────────────────────────────────

/// Permutation table for noise evaluation.
const PERM: [u8; 256] = [
    151,160,137, 91, 90, 15,131, 13,201, 95, 96, 53,194,233,  7,225,
    140, 36,103, 30, 69,142,  8, 99, 37,240, 21, 10, 23,190,  6,148,
    247,120,234, 75,  0, 26,197, 62, 94,252,219,203,117, 35, 11, 32,
     57,177, 33, 88,237,149, 56, 87,174, 20,125,136,171,168, 68,175,
     74,165, 71,134,139, 48, 27,166, 77,146,158,231, 83,111,229,122,
     60,211,133,230,220,105, 92, 41, 55, 46,245, 40,244,102,143, 54,
     65, 25, 63,161,  1,216, 80, 73,209, 76,132,187,208, 89, 18,169,
    200,196,135,130,116,188,159, 86,164,100,109,198,173,186,  3, 64,
     52,217,226,250,124,123,  5,202, 38,147,118,126,255, 82, 85,212,
    207,206, 59,227, 47, 16, 58, 17,182,189, 28, 42,223,183,170,213,
    119,248,152,  2, 44,154,163, 70,221,153,101,155,167, 43,172,  9,
    129, 22, 39,253, 19, 98,108,110, 79,113,224,232,178,185,112,104,
    218,246, 97,228,251, 34,242,193,238,210,144, 12,191,179,162,241,
     81, 51,145,235,249, 14,239,107, 49,192,214, 31,181,199,106,157,
    184, 84,204,176,115,121, 50, 45,127,  4,150,254,138,236,205, 93,
    222,114, 67, 29, 24, 72,243,141,128,195, 78, 66,215, 61,156,180,
];

#[inline(always)]
fn perm(i: i32) -> usize {
    PERM[((i % 256 + 256) % 256) as usize] as usize
}

#[inline(always)]
fn perm_seeded(i: i32, seed: u32) -> usize {
    PERM[(((i.wrapping_add(seed as i32)) % 256 + 256) % 256) as usize] as usize
}

#[inline(always)]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline(always)]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

#[inline(always)]
fn grad2(hash: usize, x: f32, y: f32) -> f32 {
    match hash & 3 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        _ => -x - y,
    }
}

/// 2D Perlin noise with seed. Output in approximately [-1, 1].
fn perlin2_seeded(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);

    let aa = perm_seeded(perm_seeded(xi, seed) as i32 + yi, seed);
    let ba = perm_seeded(perm_seeded(xi + 1, seed) as i32 + yi, seed);
    let ab = perm_seeded(perm_seeded(xi, seed) as i32 + yi + 1, seed);
    let bb = perm_seeded(perm_seeded(xi + 1, seed) as i32 + yi + 1, seed);

    lerp(
        lerp(grad2(aa, xf, yf), grad2(ba, xf - 1.0, yf), u),
        lerp(grad2(ab, xf, yf - 1.0), grad2(bb, xf - 1.0, yf - 1.0), u),
        v,
    )
}

impl NoiseSource {
    /// Evaluate the noise source at world-space position (x, z).
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        match self {
            NoiseSource::Perlin { frequency, amplitude, seed } => {
                perlin2_seeded(x * frequency, z * frequency, *seed) * amplitude
            }
            NoiseSource::Fbm { frequency, amplitude, octaves, lacunarity, persistence, seed } => {
                let mut value = 0.0_f32;
                let mut freq = *frequency;
                let mut amp = *amplitude;
                for oct in 0..*octaves {
                    let s = seed.wrapping_add(oct * 31);
                    value += perlin2_seeded(x * freq, z * freq, s) * amp;
                    freq *= lacunarity;
                    amp *= persistence;
                }
                value
            }
            NoiseSource::Ridged { frequency, amplitude, octaves, lacunarity, gain, offset, seed } => {
                let mut value = 0.0_f32;
                let mut freq = *frequency;
                let mut amp = *amplitude;
                let mut weight = 1.0_f32;
                for oct in 0..*octaves {
                    let s = seed.wrapping_add(oct * 31);
                    let signal = perlin2_seeded(x * freq, z * freq, s).abs();
                    let signal = offset - signal;
                    let signal = signal * signal * weight;
                    weight = (signal * gain).clamp(0.0, 1.0);
                    value += signal * amp;
                    freq *= lacunarity;
                    amp *= 0.5;
                }
                value
            }
            NoiseSource::DomainWarped { base, warp_frequency, warp_amplitude, warp_seed } => {
                let wx = perlin2_seeded(x * warp_frequency, z * warp_frequency, *warp_seed)
                    * warp_amplitude;
                let wz = perlin2_seeded(
                    (x + 100.0) * warp_frequency,
                    (z + 100.0) * warp_frequency,
                    warp_seed.wrapping_add(7),
                ) * warp_amplitude;
                base.sample(x + wx, z + wz)
            }
            NoiseSource::Flat { height } => *height,
            NoiseSource::Sinusoidal { frequency_x, frequency_z, amplitude } => {
                (x * frequency_x * TAU).sin() * (z * frequency_z * TAU).sin() * amplitude
            }
            NoiseSource::Composite { sources } => {
                sources.iter().map(|s| s.sample(x, z)).sum()
            }
        }
    }

    /// Compute the gradient of the noise at (x, z) using central differences.
    pub fn gradient(&self, x: f32, z: f32) -> Vec2 {
        let eps = 0.1;
        let dx = (self.sample(x + eps, z) - self.sample(x - eps, z)) / (2.0 * eps);
        let dz = (self.sample(x, z + eps) - self.sample(x, z - eps)) / (2.0 * eps);
        Vec2::new(dx, dz)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HeightFieldSurface
// ─────────────────────────────────────────────────────────────────────────────

/// A height field surface driven by a noise source.
///
/// The surface occupies a rectangular region in XZ-space. Each grid cell stores
/// a height value (Y coordinate). Heights are lazily generated from the noise source.
#[derive(Clone)]
pub struct HeightFieldSurface {
    /// The noise source generating heights.
    pub noise: NoiseSource,
    /// World-space origin (bottom-left corner of the heightfield in XZ).
    pub origin: Vec2,
    /// World-space size of the heightfield in XZ.
    pub size: Vec2,
    /// Number of samples along X.
    pub resolution_x: usize,
    /// Number of samples along Z.
    pub resolution_z: usize,
    /// Computed height values. Row-major: heights[z * resolution_x + x].
    pub heights: Vec<f32>,
    /// Time parameter for animated deformations.
    pub time: f32,
    /// Animation modes applied to the surface.
    pub animations: Vec<HeightFieldAnimation>,
}

/// Real-time animation modes for height fields.
#[derive(Debug, Clone)]
pub enum HeightFieldAnimation {
    /// Terrain breathes up and down.
    Breathe {
        amplitude: f32,
        frequency: f32,
    },
    /// Ripple from a point.
    Ripple {
        center: Vec2,
        speed: f32,
        amplitude: f32,
        wavelength: f32,
        decay: f32,
    },
    /// Terrain warps like a cloth in wind.
    Warp {
        direction: Vec2,
        speed: f32,
        amplitude: f32,
        frequency: f32,
    },
    /// Terrain collapses toward a point.
    Collapse {
        center: Vec2,
        speed: f32,
        radius: f32,
        depth: f32,
    },
    /// Sinusoidal wave propagation.
    Wave {
        direction: Vec2,
        speed: f32,
        amplitude: f32,
        wavelength: f32,
    },
}

impl HeightFieldSurface {
    /// Create a new height field surface.
    pub fn new(
        noise: NoiseSource,
        origin: Vec2,
        size: Vec2,
        resolution_x: usize,
        resolution_z: usize,
    ) -> Self {
        let rx = resolution_x.max(2);
        let rz = resolution_z.max(2);
        let mut surface = Self {
            noise,
            origin,
            size,
            resolution_x: rx,
            resolution_z: rz,
            heights: vec![0.0; rx * rz],
            time: 0.0,
            animations: Vec::new(),
        };
        surface.regenerate();
        surface
    }

    /// Regenerate all height values from the noise source.
    pub fn regenerate(&mut self) {
        for iz in 0..self.resolution_z {
            for ix in 0..self.resolution_x {
                let wx = self.origin.x + (ix as f32 / (self.resolution_x - 1) as f32) * self.size.x;
                let wz = self.origin.y + (iz as f32 / (self.resolution_z - 1) as f32) * self.size.y;
                self.heights[iz * self.resolution_x + ix] = self.noise.sample(wx, wz);
            }
        }
    }

    /// Get the height at grid indices (ix, iz).
    #[inline]
    pub fn height_at_index(&self, ix: usize, iz: usize) -> f32 {
        let ix = ix.min(self.resolution_x - 1);
        let iz = iz.min(self.resolution_z - 1);
        self.heights[iz * self.resolution_x + ix]
    }

    /// Set the height at grid indices (ix, iz).
    #[inline]
    pub fn set_height(&mut self, ix: usize, iz: usize, h: f32) {
        if ix < self.resolution_x && iz < self.resolution_z {
            self.heights[iz * self.resolution_x + ix] = h;
        }
    }

    /// Sample height at world-space (x, z) using bilinear interpolation.
    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        let fx = ((x - self.origin.x) / self.size.x) * (self.resolution_x - 1) as f32;
        let fz = ((z - self.origin.y) / self.size.y) * (self.resolution_z - 1) as f32;

        let ix = fx.floor() as i32;
        let iz = fz.floor() as i32;

        if ix < 0 || iz < 0 || ix >= (self.resolution_x - 1) as i32 || iz >= (self.resolution_z - 1) as i32 {
            // Out of bounds: sample directly from noise
            return self.noise.sample(x, z);
        }

        let ix = ix as usize;
        let iz = iz as usize;
        let sx = fx - ix as f32;
        let sz = fz - iz as f32;

        let h00 = self.height_at_index(ix, iz);
        let h10 = self.height_at_index(ix + 1, iz);
        let h01 = self.height_at_index(ix, iz + 1);
        let h11 = self.height_at_index(ix + 1, iz + 1);

        let top = h00 * (1.0 - sx) + h10 * sx;
        let bottom = h01 * (1.0 - sx) + h11 * sx;
        top * (1.0 - sz) + bottom * sz
    }

    /// Compute the surface normal at world-space (x, z) using central differences.
    pub fn normal_at(&self, x: f32, z: f32) -> Vec3 {
        let cell_x = self.size.x / (self.resolution_x - 1) as f32;
        let cell_z = self.size.y / (self.resolution_z - 1) as f32;
        let eps_x = cell_x * 0.5;
        let eps_z = cell_z * 0.5;

        let hx_pos = self.sample_height(x + eps_x, z);
        let hx_neg = self.sample_height(x - eps_x, z);
        let hz_pos = self.sample_height(x, z + eps_z);
        let hz_neg = self.sample_height(x, z - eps_z);

        let dx = (hx_pos - hx_neg) / (2.0 * eps_x);
        let dz = (hz_pos - hz_neg) / (2.0 * eps_z);

        Vec3::new(-dx, 1.0, -dz).normalize()
    }

    /// Compute the normal at grid index using central differences.
    pub fn normal_at_index(&self, ix: usize, iz: usize) -> Vec3 {
        let h_left = if ix > 0 { self.height_at_index(ix - 1, iz) } else { self.height_at_index(ix, iz) };
        let h_right = if ix + 1 < self.resolution_x { self.height_at_index(ix + 1, iz) } else { self.height_at_index(ix, iz) };
        let h_down = if iz > 0 { self.height_at_index(ix, iz - 1) } else { self.height_at_index(ix, iz) };
        let h_up = if iz + 1 < self.resolution_z { self.height_at_index(ix, iz + 1) } else { self.height_at_index(ix, iz) };

        let cell_x = self.size.x / (self.resolution_x - 1) as f32;
        let cell_z = self.size.y / (self.resolution_z - 1) as f32;

        let dx = (h_right - h_left) / (2.0 * cell_x);
        let dz = (h_up - h_down) / (2.0 * cell_z);

        Vec3::new(-dx, 1.0, -dz).normalize()
    }

    /// Update time and apply real-time animations.
    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        if self.animations.is_empty() {
            return;
        }

        // Re-generate base heights
        self.regenerate();

        // Apply each animation on top
        let t = self.time;
        let animations = self.animations.clone();

        for iz in 0..self.resolution_z {
            for ix in 0..self.resolution_x {
                let wx = self.origin.x + (ix as f32 / (self.resolution_x - 1) as f32) * self.size.x;
                let wz = self.origin.y + (iz as f32 / (self.resolution_z - 1) as f32) * self.size.y;
                let idx = iz * self.resolution_x + ix;
                let pos = Vec2::new(wx, wz);

                for anim in &animations {
                    match anim {
                        HeightFieldAnimation::Breathe { amplitude, frequency } => {
                            self.heights[idx] += (t * frequency * TAU).sin() * amplitude;
                        }
                        HeightFieldAnimation::Ripple { center, speed, amplitude, wavelength, decay } => {
                            let dist = pos.distance(*center);
                            let wave = ((dist / wavelength - t * speed) * TAU).sin();
                            let falloff = (-dist * decay).exp();
                            self.heights[idx] += wave * amplitude * falloff;
                        }
                        HeightFieldAnimation::Warp { direction, speed, amplitude, frequency } => {
                            let phase = pos.dot(*direction) * frequency - t * speed;
                            self.heights[idx] += (phase * TAU).sin() * amplitude;
                        }
                        HeightFieldAnimation::Collapse { center, speed, radius, depth } => {
                            let dist = pos.distance(*center);
                            let progress = (t * speed).min(1.0);
                            let factor = (1.0 - (dist / radius).min(1.0)).max(0.0);
                            let factor = factor * factor; // smooth falloff
                            self.heights[idx] -= factor * depth * progress;
                        }
                        HeightFieldAnimation::Wave { direction, speed, amplitude, wavelength } => {
                            let phase = pos.dot(*direction) / wavelength - t * speed;
                            self.heights[idx] += (phase * TAU).sin() * amplitude;
                        }
                    }
                }
            }
        }
    }

    /// Apply a deformation brush: raise or lower terrain in a radius around (cx, cz).
    pub fn deform_brush(&mut self, cx: f32, cz: f32, radius: f32, strength: f32) {
        for iz in 0..self.resolution_z {
            for ix in 0..self.resolution_x {
                let wx = self.origin.x + (ix as f32 / (self.resolution_x - 1) as f32) * self.size.x;
                let wz = self.origin.y + (iz as f32 / (self.resolution_z - 1) as f32) * self.size.y;
                let dist = ((wx - cx).powi(2) + (wz - cz).powi(2)).sqrt();
                if dist < radius {
                    let falloff = 1.0 - dist / radius;
                    let falloff = falloff * falloff * (3.0 - 2.0 * falloff); // smoothstep
                    self.heights[iz * self.resolution_x + ix] += strength * falloff;
                }
            }
        }
    }

    /// Flatten terrain in a radius around (cx, cz) toward a target height.
    pub fn flatten_brush(&mut self, cx: f32, cz: f32, radius: f32, target: f32, strength: f32) {
        for iz in 0..self.resolution_z {
            for ix in 0..self.resolution_x {
                let wx = self.origin.x + (ix as f32 / (self.resolution_x - 1) as f32) * self.size.x;
                let wz = self.origin.y + (iz as f32 / (self.resolution_z - 1) as f32) * self.size.y;
                let dist = ((wx - cx).powi(2) + (wz - cz).powi(2)).sqrt();
                if dist < radius {
                    let falloff = 1.0 - dist / radius;
                    let falloff = falloff * falloff * (3.0 - 2.0 * falloff);
                    let idx = iz * self.resolution_x + ix;
                    self.heights[idx] += (target - self.heights[idx]) * strength * falloff;
                }
            }
        }
    }

    /// Get the world-space position of a grid vertex.
    pub fn world_position(&self, ix: usize, iz: usize) -> Vec3 {
        let wx = self.origin.x + (ix as f32 / (self.resolution_x - 1) as f32) * self.size.x;
        let wz = self.origin.y + (iz as f32 / (self.resolution_z - 1) as f32) * self.size.y;
        let h = self.height_at_index(ix, iz);
        Vec3::new(wx, h, wz)
    }

    /// Compute the bounding box: (min, max).
    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        let mut min_h = f32::MAX;
        let mut max_h = f32::MIN;
        for &h in &self.heights {
            min_h = min_h.min(h);
            max_h = max_h.max(h);
        }
        (
            Vec3::new(self.origin.x, min_h, self.origin.y),
            Vec3::new(self.origin.x + self.size.x, max_h, self.origin.y + self.size.y),
        )
    }

    /// Export to a list of positions and normals (interleaved grid).
    pub fn to_vertex_data(&self) -> (Vec<Vec3>, Vec<Vec3>) {
        let mut positions = Vec::with_capacity(self.resolution_x * self.resolution_z);
        let mut normals = Vec::with_capacity(self.resolution_x * self.resolution_z);

        for iz in 0..self.resolution_z {
            for ix in 0..self.resolution_x {
                positions.push(self.world_position(ix, iz));
                normals.push(self.normal_at_index(ix, iz));
            }
        }

        (positions, normals)
    }

    /// Generate triangle indices for the height field grid.
    pub fn generate_indices(&self) -> Vec<[u32; 3]> {
        let mut indices = Vec::with_capacity((self.resolution_x - 1) * (self.resolution_z - 1) * 2);
        for iz in 0..self.resolution_z - 1 {
            for ix in 0..self.resolution_x - 1 {
                let tl = (iz * self.resolution_x + ix) as u32;
                let tr = tl + 1;
                let bl = tl + self.resolution_x as u32;
                let br = bl + 1;
                indices.push([tl, bl, tr]);
                indices.push([tr, bl, br]);
            }
        }
        indices
    }

    /// Sample height directly from noise (without grid interpolation).
    pub fn sample_noise(&self, x: f32, z: f32) -> f32 {
        self.noise.sample(x, z)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Height field collision detection
// ─────────────────────────────────────────────────────────────────────────────

/// Collision detector for ray-heightfield intersection.
pub struct HeightFieldCollider;

/// Result of a height field ray intersection.
#[derive(Debug, Clone, Copy)]
pub struct HeightFieldHit {
    pub position: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}

impl HeightFieldCollider {
    /// Cast a ray against a height field surface using ray marching + binary search refinement.
    ///
    /// The ray starts at `origin` and moves in `direction` (should be normalized).
    /// `max_distance` limits the search. `step_size` controls the initial march resolution.
    pub fn ray_cast(
        surface: &HeightFieldSurface,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        step_size: f32,
    ) -> Option<HeightFieldHit> {
        let dir = direction.normalize();
        let mut t = 0.0_f32;
        let mut prev_above = true;
        let mut prev_pos = origin;

        // Phase 1: Ray march to find bracket
        while t < max_distance {
            let pos = origin + dir * t;
            let terrain_h = surface.sample_height(pos.x, pos.z);
            let above = pos.y > terrain_h;

            if !above && prev_above && t > 0.0 {
                // Crossed the surface between prev_pos and pos
                // Phase 2: Binary search refinement
                let mut lo = t - step_size;
                let mut hi = t;

                for _ in 0..16 {
                    let mid = (lo + hi) * 0.5;
                    let mid_pos = origin + dir * mid;
                    let mid_h = surface.sample_height(mid_pos.x, mid_pos.z);
                    if mid_pos.y > mid_h {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }

                let final_t = (lo + hi) * 0.5;
                let hit_pos = origin + dir * final_t;
                let normal = surface.normal_at(hit_pos.x, hit_pos.z);

                return Some(HeightFieldHit {
                    position: hit_pos,
                    normal,
                    distance: final_t,
                });
            }

            prev_above = above;
            prev_pos = pos;
            t += step_size;
        }

        None
    }

    /// Test if a point is above or below the terrain.
    pub fn is_above(surface: &HeightFieldSurface, point: Vec3) -> bool {
        point.y > surface.sample_height(point.x, point.z)
    }

    /// Get the vertical distance from a point to the terrain surface.
    pub fn distance_to_surface(surface: &HeightFieldSurface, point: Vec3) -> f32 {
        point.y - surface.sample_height(point.x, point.z)
    }

    /// Project a point onto the terrain (snap Y to terrain height).
    pub fn project_onto(surface: &HeightFieldSurface, point: Vec3) -> Vec3 {
        Vec3::new(point.x, surface.sample_height(point.x, point.z), point.z)
    }

    /// Sphere-terrain intersection test.
    /// Returns true if a sphere at `center` with given `radius` intersects the terrain.
    pub fn sphere_intersects(
        surface: &HeightFieldSurface,
        center: Vec3,
        radius: f32,
    ) -> bool {
        // Sample terrain at the sphere center and nearby points
        let h = surface.sample_height(center.x, center.z);
        if center.y - radius < h {
            return true;
        }

        // Check 4 surrounding sample points
        for &(dx, dz) in &[(radius, 0.0), (-radius, 0.0), (0.0, radius), (0.0, -radius)] {
            let px = center.x + dx;
            let pz = center.z + dz;
            let ph = surface.sample_height(px, pz);
            let dist_sq = (center.x - px).powi(2) + (center.y - ph).powi(2) + (center.z - pz).powi(2);
            if dist_sq < radius * radius {
                return true;
            }
        }

        false
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LOD system
// ─────────────────────────────────────────────────────────────────────────────

/// Level of detail for height field rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LodLevel {
    /// Full resolution.
    Full,
    /// Half resolution.
    Half,
    /// Quarter resolution.
    Quarter,
    /// Eighth resolution.
    Eighth,
}

impl LodLevel {
    /// Get the stride (skip) factor for this LOD level.
    pub fn stride(self) -> usize {
        match self {
            LodLevel::Full => 1,
            LodLevel::Half => 2,
            LodLevel::Quarter => 4,
            LodLevel::Eighth => 8,
        }
    }

    /// Select a LOD level based on distance from camera.
    pub fn from_distance(distance: f32, thresholds: &LodThresholds) -> Self {
        if distance < thresholds.full_distance {
            LodLevel::Full
        } else if distance < thresholds.half_distance {
            LodLevel::Half
        } else if distance < thresholds.quarter_distance {
            LodLevel::Quarter
        } else {
            LodLevel::Eighth
        }
    }

    /// All LOD levels from finest to coarsest.
    pub fn all() -> &'static [LodLevel] {
        &[LodLevel::Full, LodLevel::Half, LodLevel::Quarter, LodLevel::Eighth]
    }
}

/// Distance thresholds for LOD level selection.
#[derive(Debug, Clone, Copy)]
pub struct LodThresholds {
    pub full_distance: f32,
    pub half_distance: f32,
    pub quarter_distance: f32,
}

impl Default for LodThresholds {
    fn default() -> Self {
        Self {
            full_distance: 100.0,
            half_distance: 200.0,
            quarter_distance: 400.0,
        }
    }
}

/// Generate indices for a height field at a given LOD level.
pub fn generate_lod_indices(resolution_x: usize, resolution_z: usize, lod: LodLevel) -> Vec<[u32; 3]> {
    let stride = lod.stride();
    let mut indices = Vec::new();

    let mut iz = 0;
    while iz + stride < resolution_z {
        let mut ix = 0;
        while ix + stride < resolution_x {
            let tl = (iz * resolution_x + ix) as u32;
            let tr = (iz * resolution_x + ix + stride) as u32;
            let bl = ((iz + stride) * resolution_x + ix) as u32;
            let br = ((iz + stride) * resolution_x + ix + stride) as u32;
            indices.push([tl, bl, tr]);
            indices.push([tr, bl, br]);
            ix += stride;
        }
        iz += stride;
    }

    indices
}

// ─────────────────────────────────────────────────────────────────────────────
// Chunk-based infinite scrolling
// ─────────────────────────────────────────────────────────────────────────────

/// A coordinate in chunk space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub fn new(x: i32, z: i32) -> Self { Self { x, z } }

    /// Get the world-space origin of this chunk.
    pub fn world_origin(self, chunk_size: f32) -> Vec2 {
        Vec2::new(self.x as f32 * chunk_size, self.z as f32 * chunk_size)
    }

    /// Compute the distance from this chunk's center to a point.
    pub fn distance_to(self, point: Vec2, chunk_size: f32) -> f32 {
        let center = Vec2::new(
            (self.x as f32 + 0.5) * chunk_size,
            (self.z as f32 + 0.5) * chunk_size,
        );
        center.distance(point)
    }

    /// Get the chunk coordinate that contains a world-space point.
    pub fn from_world(x: f32, z: f32, chunk_size: f32) -> Self {
        Self {
            x: (x / chunk_size).floor() as i32,
            z: (z / chunk_size).floor() as i32,
        }
    }
}

/// A single terrain chunk.
pub struct HeightFieldChunk {
    pub coord: ChunkCoord,
    pub surface: HeightFieldSurface,
    pub lod: LodLevel,
    /// Cached indices at the current LOD level.
    pub cached_indices: Vec<[u32; 3]>,
}

impl HeightFieldChunk {
    /// Create a new chunk at the given coordinate.
    pub fn new(
        coord: ChunkCoord,
        noise: &NoiseSource,
        chunk_size: f32,
        resolution: usize,
    ) -> Self {
        let origin = coord.world_origin(chunk_size);
        let surface = HeightFieldSurface::new(
            noise.clone(),
            origin,
            Vec2::splat(chunk_size),
            resolution,
            resolution,
        );
        let cached_indices = surface.generate_indices();
        Self {
            coord,
            surface,
            lod: LodLevel::Full,
            cached_indices,
        }
    }

    /// Update the LOD level and regenerate indices if changed.
    pub fn update_lod(&mut self, new_lod: LodLevel) {
        if self.lod != new_lod {
            self.lod = new_lod;
            self.cached_indices = generate_lod_indices(
                self.surface.resolution_x,
                self.surface.resolution_z,
                new_lod,
            );
        }
    }

    /// Tick animation on this chunk.
    pub fn tick(&mut self, dt: f32) {
        self.surface.tick(dt);
    }
}

/// Manages a dynamic set of terrain chunks around a camera position.
pub struct ChunkManager {
    /// The noise source shared by all chunks.
    pub noise: NoiseSource,
    /// Size of each chunk in world units.
    pub chunk_size: f32,
    /// Resolution of each chunk (vertices per side).
    pub chunk_resolution: usize,
    /// How many chunks to keep around the camera in each direction.
    pub view_radius: i32,
    /// LOD distance thresholds.
    pub lod_thresholds: LodThresholds,
    /// Currently loaded chunks.
    pub chunks: std::collections::HashMap<ChunkCoord, HeightFieldChunk>,
    /// Last known camera position (for determining which chunks to load/unload).
    pub last_camera_pos: Vec2,
    /// Animations applied to all chunks.
    pub animations: Vec<HeightFieldAnimation>,
}

impl ChunkManager {
    /// Create a new chunk manager.
    pub fn new(
        noise: NoiseSource,
        chunk_size: f32,
        chunk_resolution: usize,
        view_radius: i32,
    ) -> Self {
        Self {
            noise,
            chunk_size,
            chunk_resolution,
            view_radius,
            lod_thresholds: LodThresholds::default(),
            chunks: std::collections::HashMap::new(),
            last_camera_pos: Vec2::ZERO,
            animations: Vec::new(),
        }
    }

    /// Update the chunk manager with a new camera position.
    /// Loads new chunks that are now in range, unloads chunks that are too far.
    pub fn update(&mut self, camera_x: f32, camera_z: f32) {
        let cam_pos = Vec2::new(camera_x, camera_z);
        self.last_camera_pos = cam_pos;

        let center_coord = ChunkCoord::from_world(camera_x, camera_z, self.chunk_size);

        // Determine which chunks should be loaded
        let mut needed: std::collections::HashSet<ChunkCoord> = std::collections::HashSet::new();
        for dz in -self.view_radius..=self.view_radius {
            for dx in -self.view_radius..=self.view_radius {
                needed.insert(ChunkCoord::new(center_coord.x + dx, center_coord.z + dz));
            }
        }

        // Remove chunks that are no longer needed
        let to_remove: Vec<ChunkCoord> = self.chunks.keys()
            .filter(|c| !needed.contains(c))
            .copied()
            .collect();
        for coord in to_remove {
            self.chunks.remove(&coord);
        }

        // Add chunks that are newly needed
        for coord in &needed {
            if !self.chunks.contains_key(coord) {
                let mut chunk = HeightFieldChunk::new(
                    *coord,
                    &self.noise,
                    self.chunk_size,
                    self.chunk_resolution,
                );
                chunk.surface.animations = self.animations.clone();
                self.chunks.insert(*coord, chunk);
            }
        }

        // Update LOD levels based on distance
        for (coord, chunk) in &mut self.chunks {
            let dist = coord.distance_to(cam_pos, self.chunk_size);
            let lod = LodLevel::from_distance(dist, &self.lod_thresholds);
            chunk.update_lod(lod);
        }
    }

    /// Tick all chunks (for animation).
    pub fn tick(&mut self, dt: f32) {
        for chunk in self.chunks.values_mut() {
            chunk.tick(dt);
        }
    }

    /// Sample height at an arbitrary world-space position.
    pub fn sample_height(&self, x: f32, z: f32) -> f32 {
        let coord = ChunkCoord::from_world(x, z, self.chunk_size);
        if let Some(chunk) = self.chunks.get(&coord) {
            chunk.surface.sample_height(x, z)
        } else {
            // Chunk not loaded; sample directly from noise
            self.noise.sample(x, z)
        }
    }

    /// Sample normal at an arbitrary world-space position.
    pub fn sample_normal(&self, x: f32, z: f32) -> Vec3 {
        let coord = ChunkCoord::from_world(x, z, self.chunk_size);
        if let Some(chunk) = self.chunks.get(&coord) {
            chunk.surface.normal_at(x, z)
        } else {
            Vec3::Y
        }
    }

    /// Get the number of currently loaded chunks.
    pub fn loaded_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Get an iterator over all loaded chunks.
    pub fn iter_chunks(&self) -> impl Iterator<Item = &HeightFieldChunk> {
        self.chunks.values()
    }

    /// Get a mutable iterator over all loaded chunks.
    pub fn iter_chunks_mut(&mut self) -> impl Iterator<Item = &mut HeightFieldChunk> {
        self.chunks.values_mut()
    }

    /// Cast a ray against all loaded chunks.
    pub fn ray_cast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> Option<HeightFieldHit> {
        let mut closest: Option<HeightFieldHit> = None;

        for chunk in self.chunks.values() {
            if let Some(hit) = HeightFieldCollider::ray_cast(
                &chunk.surface, origin, direction, max_distance, 0.5,
            ) {
                if closest.as_ref().map_or(true, |c| hit.distance < c.distance) {
                    closest = Some(hit);
                }
            }
        }

        closest
    }

    /// Apply a deformation brush at world position.
    pub fn deform(&mut self, x: f32, z: f32, radius: f32, strength: f32) {
        // Determine which chunks might be affected
        let chunk_radius = (radius / self.chunk_size).ceil() as i32 + 1;
        let center = ChunkCoord::from_world(x, z, self.chunk_size);

        for dz in -chunk_radius..=chunk_radius {
            for dx in -chunk_radius..=chunk_radius {
                let coord = ChunkCoord::new(center.x + dx, center.z + dz);
                if let Some(chunk) = self.chunks.get_mut(&coord) {
                    chunk.surface.deform_brush(x, z, radius, strength);
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Erosion simulation
// ─────────────────────────────────────────────────────────────────────────────

/// Simple hydraulic erosion parameters.
#[derive(Debug, Clone, Copy)]
pub struct ErosionParams {
    pub iterations: usize,
    pub inertia: f32,
    pub sediment_capacity: f32,
    pub min_sediment_capacity: f32,
    pub deposit_speed: f32,
    pub erode_speed: f32,
    pub evaporate_speed: f32,
    pub gravity: f32,
    pub max_droplet_lifetime: usize,
    pub initial_water: f32,
    pub initial_speed: f32,
}

impl Default for ErosionParams {
    fn default() -> Self {
        Self {
            iterations: 5000,
            inertia: 0.05,
            sediment_capacity: 4.0,
            min_sediment_capacity: 0.01,
            deposit_speed: 0.3,
            erode_speed: 0.3,
            evaporate_speed: 0.01,
            gravity: 4.0,
            max_droplet_lifetime: 30,
            initial_water: 1.0,
            initial_speed: 1.0,
        }
    }
}

/// Simple pseudo-random number generator (xorshift32) for erosion.
struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    fn new(seed: u32) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }
}

/// Run hydraulic erosion simulation on a height field.
pub fn erode(surface: &mut HeightFieldSurface, params: &ErosionParams, seed: u32) {
    let mut rng = SimpleRng::new(seed);
    let rx = surface.resolution_x;
    let rz = surface.resolution_z;

    for _ in 0..params.iterations {
        // Random starting position
        let mut pos_x = rng.next_f32() * (rx - 2) as f32 + 0.5;
        let mut pos_z = rng.next_f32() * (rz - 2) as f32 + 0.5;
        let mut dir_x = 0.0_f32;
        let mut dir_z = 0.0_f32;
        let mut speed = params.initial_speed;
        let mut water = params.initial_water;
        let mut sediment = 0.0_f32;

        for _ in 0..params.max_droplet_lifetime {
            let ix = pos_x as usize;
            let iz = pos_z as usize;

            if ix < 1 || ix >= rx - 1 || iz < 1 || iz >= rz - 1 {
                break;
            }

            // Compute gradient using central differences on the grid
            let h_l = surface.heights[iz * rx + ix - 1];
            let h_r = surface.heights[iz * rx + ix + 1];
            let h_d = surface.heights[(iz - 1) * rx + ix];
            let h_u = surface.heights[(iz + 1) * rx + ix];

            let grad_x = (h_r - h_l) * 0.5;
            let grad_z = (h_u - h_d) * 0.5;

            // Update direction with inertia
            dir_x = dir_x * params.inertia - grad_x * (1.0 - params.inertia);
            dir_z = dir_z * params.inertia - grad_z * (1.0 - params.inertia);

            let len = (dir_x * dir_x + dir_z * dir_z).sqrt();
            if len < 1e-6 {
                // Random direction if stuck
                let angle = rng.next_f32() * TAU;
                dir_x = angle.cos();
                dir_z = angle.sin();
            } else {
                dir_x /= len;
                dir_z /= len;
            }

            // Move droplet
            let new_x = pos_x + dir_x;
            let new_z = pos_z + dir_z;

            let new_ix = new_x as usize;
            let new_iz = new_z as usize;
            if new_ix < 1 || new_ix >= rx - 1 || new_iz < 1 || new_iz >= rz - 1 {
                break;
            }

            let old_h = surface.heights[iz * rx + ix];
            let new_h = surface.heights[new_iz * rx + new_ix];
            let delta_h = new_h - old_h;

            // Compute sediment capacity
            let capacity = (-delta_h * speed * water * params.sediment_capacity)
                .max(params.min_sediment_capacity);

            if sediment > capacity || delta_h > 0.0 {
                // Deposit
                let amount = if delta_h > 0.0 {
                    delta_h.min(sediment)
                } else {
                    (sediment - capacity) * params.deposit_speed
                };
                sediment -= amount;
                surface.heights[iz * rx + ix] += amount;
            } else {
                // Erode
                let amount = ((capacity - sediment) * params.erode_speed).min(-delta_h);
                sediment += amount;
                surface.heights[iz * rx + ix] -= amount;
            }

            // Update physics
            speed = (speed * speed + delta_h * params.gravity).abs().sqrt();
            water *= 1.0 - params.evaporate_speed;

            pos_x = new_x;
            pos_z = new_z;

            if water < 0.001 {
                break;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_noise_source() {
        let ns = NoiseSource::Flat { height: 5.0 };
        assert!((ns.sample(0.0, 0.0) - 5.0).abs() < 1e-5);
        assert!((ns.sample(100.0, 200.0) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn sinusoidal_terrain() {
        let ns = NoiseSource::Sinusoidal {
            frequency_x: 1.0,
            frequency_z: 1.0,
            amplitude: 10.0,
        };
        let h = ns.sample(0.25, 0.25);
        assert!(h.abs() <= 10.0);
    }

    #[test]
    fn heightfield_create() {
        let hf = HeightFieldSurface::new(
            NoiseSource::Flat { height: 3.0 },
            Vec2::ZERO,
            Vec2::splat(100.0),
            16,
            16,
        );
        assert!((hf.sample_height(50.0, 50.0) - 3.0).abs() < 1e-3);
    }

    #[test]
    fn heightfield_normal_flat() {
        let hf = HeightFieldSurface::new(
            NoiseSource::Flat { height: 0.0 },
            Vec2::ZERO,
            Vec2::splat(100.0),
            16,
            16,
        );
        let n = hf.normal_at(50.0, 50.0);
        assert!((n.y - 1.0).abs() < 0.01);
    }

    #[test]
    fn ray_cast_flat() {
        let hf = HeightFieldSurface::new(
            NoiseSource::Flat { height: 0.0 },
            Vec2::new(-50.0, -50.0),
            Vec2::splat(100.0),
            32,
            32,
        );
        let hit = HeightFieldCollider::ray_cast(
            &hf,
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            100.0,
            0.5,
        );
        assert!(hit.is_some());
        let h = hit.unwrap();
        assert!((h.position.y).abs() < 1.0);
    }

    #[test]
    fn chunk_manager_loading() {
        let mut mgr = ChunkManager::new(
            NoiseSource::Flat { height: 0.0 },
            64.0,
            16,
            1,
        );
        mgr.update(0.0, 0.0);
        // 3x3 grid of chunks (radius 1)
        assert_eq!(mgr.loaded_chunk_count(), 9);
    }

    #[test]
    fn chunk_manager_move_camera() {
        let mut mgr = ChunkManager::new(
            NoiseSource::Flat { height: 0.0 },
            64.0,
            8,
            1,
        );
        mgr.update(0.0, 0.0);
        let initial = mgr.loaded_chunk_count();
        mgr.update(1000.0, 0.0);
        assert_eq!(mgr.loaded_chunk_count(), initial);
    }

    #[test]
    fn deform_brush() {
        let mut hf = HeightFieldSurface::new(
            NoiseSource::Flat { height: 0.0 },
            Vec2::ZERO,
            Vec2::splat(100.0),
            32,
            32,
        );
        hf.deform_brush(50.0, 50.0, 10.0, 5.0);
        let h = hf.sample_height(50.0, 50.0);
        assert!(h > 4.0);
    }

    #[test]
    fn lod_levels() {
        let thresholds = LodThresholds::default();
        assert_eq!(LodLevel::from_distance(50.0, &thresholds), LodLevel::Full);
        assert_eq!(LodLevel::from_distance(150.0, &thresholds), LodLevel::Half);
        assert_eq!(LodLevel::from_distance(300.0, &thresholds), LodLevel::Quarter);
        assert_eq!(LodLevel::from_distance(500.0, &thresholds), LodLevel::Eighth);
    }

    #[test]
    fn erosion_runs() {
        let mut hf = HeightFieldSurface::new(
            NoiseSource::Sinusoidal {
                frequency_x: 0.1,
                frequency_z: 0.1,
                amplitude: 10.0,
            },
            Vec2::ZERO,
            Vec2::splat(100.0),
            32,
            32,
        );
        let params = ErosionParams { iterations: 100, ..Default::default() };
        erode(&mut hf, &params, 12345);
        // Just verify it doesn't panic and modifies heights
    }

    #[test]
    fn composite_noise() {
        let ns = NoiseSource::Composite {
            sources: vec![
                NoiseSource::Flat { height: 5.0 },
                NoiseSource::Flat { height: 3.0 },
            ],
        };
        assert!((ns.sample(0.0, 0.0) - 8.0).abs() < 1e-5);
    }
}
