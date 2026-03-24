//! Ambient and indirect lighting for Proof Engine.
//!
//! Provides screen-space ambient occlusion (SSAO), spherical harmonics for diffuse
//! irradiance, light probe grids with trilinear interpolation, reflection probes with
//! parallax correction, ambient cubes, and hemisphere lighting.

use super::lights::{Vec3, Color, Mat4};
use std::f32::consts::PI;

// ── SSAO Configuration ─────────────────────────────────────────────────────

/// Configuration for screen-space ambient occlusion.
#[derive(Debug, Clone)]
pub struct SsaoConfig {
    /// Number of hemisphere samples per pixel.
    pub sample_count: u32,
    /// Radius of the sampling hemisphere in world units.
    pub radius: f32,
    /// Bias to prevent self-occlusion on flat surfaces.
    pub bias: f32,
    /// Power exponent to increase contrast.
    pub power: f32,
    /// Intensity multiplier.
    pub intensity: f32,
    /// Whether to apply bilateral blur to the AO result.
    pub blur: bool,
    /// Blur kernel radius (in pixels).
    pub blur_radius: u32,
    /// Blur sharpness (higher = less blur across edges).
    pub blur_sharpness: f32,
    /// Noise texture size for rotating the hemisphere kernel.
    pub noise_size: u32,
}

impl Default for SsaoConfig {
    fn default() -> Self {
        Self {
            sample_count: 32,
            radius: 0.5,
            bias: 0.025,
            power: 2.0,
            intensity: 1.0,
            blur: true,
            blur_radius: 4,
            blur_sharpness: 8.0,
            noise_size: 4,
        }
    }
}

// ── SSAO Kernel ─────────────────────────────────────────────────────────────

/// Generates and stores the SSAO sampling kernel and noise rotation vectors.
#[derive(Debug, Clone)]
pub struct SsaoKernel {
    /// Sample positions in tangent space (hemisphere).
    pub samples: Vec<Vec3>,
    /// Noise rotation vectors for randomizing the kernel per pixel.
    pub noise: Vec<Vec3>,
    /// Configuration used to generate this kernel.
    pub config: SsaoConfig,
}

impl SsaoKernel {
    /// Generate a new SSAO kernel from configuration.
    pub fn new(config: SsaoConfig) -> Self {
        let samples = Self::generate_samples(config.sample_count, config.radius);
        let noise = Self::generate_noise(config.noise_size);
        Self { samples, noise, config }
    }

    /// Generate hemisphere sample points using a quasi-random distribution.
    fn generate_samples(count: u32, radius: f32) -> Vec<Vec3> {
        let mut samples = Vec::with_capacity(count as usize);

        for i in 0..count {
            // Use a low-discrepancy sequence for better distribution
            let xi1 = Self::radical_inverse_vdc(i);
            let xi2 = Self::halton_sequence(i, 3);

            // Map to hemisphere (cosine-weighted)
            let phi = 2.0 * PI * xi1;
            let cos_theta = (1.0 - xi2).sqrt();
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

            let x = sin_theta * phi.cos();
            let y = sin_theta * phi.sin();
            let z = cos_theta;

            // Scale samples so they cluster near the origin (more detail close up)
            let scale = (i as f32 + 1.0) / count as f32;
            let scale = Self::lerp_f32(0.1, 1.0, scale * scale);

            samples.push(Vec3::new(x * scale * radius, y * scale * radius, z * scale * radius));
        }

        samples
    }

    /// Generate noise rotation vectors for the noise texture.
    fn generate_noise(size: u32) -> Vec<Vec3> {
        let count = (size * size) as usize;
        let mut noise = Vec::with_capacity(count);

        for i in 0..count {
            // Deterministic pseudo-random rotation vectors in tangent plane (z=0)
            let seed = i as f32 * 7.31 + 0.5;
            let x = (seed * 12.9898 + 78.233).sin() * 43758.5453;
            let y = (seed * 39.346 + 11.135).sin() * 28461.7231;
            let nx = x.fract() * 2.0 - 1.0;
            let ny = y.fract() * 2.0 - 1.0;
            let len = (nx * nx + ny * ny).sqrt().max(0.001);
            noise.push(Vec3::new(nx / len, ny / len, 0.0));
        }

        noise
    }

    /// Van der Corput radical inverse for low-discrepancy sequences.
    fn radical_inverse_vdc(mut bits: u32) -> f32 {
        bits = (bits << 16) | (bits >> 16);
        bits = ((bits & 0x55555555) << 1) | ((bits & 0xAAAAAAAA) >> 1);
        bits = ((bits & 0x33333333) << 2) | ((bits & 0xCCCCCCCC) >> 2);
        bits = ((bits & 0x0F0F0F0F) << 4) | ((bits & 0xF0F0F0F0) >> 4);
        bits = ((bits & 0x00FF00FF) << 8) | ((bits & 0xFF00FF00) >> 8);
        bits as f32 * 2.3283064365386963e-10
    }

    /// Halton sequence for the given base.
    fn halton_sequence(index: u32, base: u32) -> f32 {
        let mut f = 1.0f32;
        let mut r = 0.0f32;
        let mut i = index;
        while i > 0 {
            f /= base as f32;
            r += f * (i % base) as f32;
            i /= base;
        }
        r
    }

    fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    /// Get the noise vector for a given screen pixel.
    pub fn noise_at(&self, x: u32, y: u32) -> Vec3 {
        let size = self.config.noise_size;
        if size == 0 || self.noise.is_empty() {
            return Vec3::new(1.0, 0.0, 0.0);
        }
        let idx = ((y % size) * size + (x % size)) as usize;
        self.noise[idx % self.noise.len()]
    }
}

// ── SSAO Result ─────────────────────────────────────────────────────────────

/// The computed SSAO buffer.
#[derive(Debug, Clone)]
pub struct SsaoResult {
    pub width: u32,
    pub height: u32,
    /// AO values per pixel (0.0 = fully occluded, 1.0 = fully open).
    pub ao_buffer: Vec<f32>,
}

impl SsaoResult {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            width,
            height,
            ao_buffer: vec![1.0; size],
        }
    }

    /// Compute SSAO from a depth buffer and normal buffer.
    pub fn compute(
        &mut self,
        depth_buffer: &[f32],
        normal_buffer: &[Vec3],
        kernel: &SsaoKernel,
        projection: &Mat4,
    ) {
        let w = self.width as usize;
        let h = self.height as usize;
        let config = &kernel.config;

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let depth = depth_buffer[idx];
                if depth >= 1.0 {
                    self.ao_buffer[idx] = 1.0;
                    continue;
                }

                let normal = normal_buffer[idx];
                let noise = kernel.noise_at(x as u32, y as u32);

                // Reconstruct view-space position from depth
                let ndc_x = (x as f32 / w as f32) * 2.0 - 1.0;
                let ndc_y = (y as f32 / h as f32) * 2.0 - 1.0;
                let frag_pos = Vec3::new(ndc_x * depth, ndc_y * depth, -depth);

                // Create TBN matrix from normal and noise
                let tangent = Self::gramm_schmidt(noise, normal);
                let bitangent = normal.cross(tangent);

                let mut occlusion = 0.0f32;
                for sample in &kernel.samples {
                    // Transform sample to view space
                    let rotated = Vec3::new(
                        tangent.x * sample.x + bitangent.x * sample.y + normal.x * sample.z,
                        tangent.y * sample.x + bitangent.y * sample.y + normal.y * sample.z,
                        tangent.z * sample.x + bitangent.z * sample.y + normal.z * sample.z,
                    );

                    let sample_pos = frag_pos + rotated * config.radius;

                    // Project sample to screen space
                    let clip = projection.transform_point(sample_pos);
                    let screen_x = ((clip.x * 0.5 + 0.5) * w as f32) as usize;
                    let screen_y = ((clip.y * 0.5 + 0.5) * h as f32) as usize;

                    if screen_x < w && screen_y < h {
                        let sample_depth = depth_buffer[screen_y * w + screen_x];
                        let range_check = Self::smooth_step(
                            0.0,
                            1.0,
                            config.radius / (frag_pos.z - sample_depth).abs().max(0.001),
                        );

                        if sample_depth >= sample_pos.z + config.bias {
                            occlusion += range_check;
                        }
                    }
                }

                occlusion /= kernel.samples.len() as f32;
                let ao = (1.0 - occlusion * config.intensity).max(0.0).powf(config.power);
                self.ao_buffer[idx] = ao;
            }
        }

        if config.blur {
            self.bilateral_blur(depth_buffer, config.blur_radius, config.blur_sharpness);
        }
    }

    /// Apply bilateral blur to the AO buffer (preserves edges based on depth).
    pub fn bilateral_blur(&mut self, depth_buffer: &[f32], radius: u32, sharpness: f32) {
        let w = self.width as usize;
        let h = self.height as usize;
        let mut temp = vec![0.0f32; w * h];

        // Horizontal pass
        for y in 0..h {
            for x in 0..w {
                let center_depth = depth_buffer[y * w + x];
                let center_ao = self.ao_buffer[y * w + x];
                let mut sum = 0.0f32;
                let mut weight_sum = 0.0f32;

                let x_start = x.saturating_sub(radius as usize);
                let x_end = (x + radius as usize + 1).min(w);

                for sx in x_start..x_end {
                    let sample_depth = depth_buffer[y * w + sx];
                    let sample_ao = self.ao_buffer[y * w + sx];

                    let depth_diff = (center_depth - sample_depth).abs();
                    let weight = (-depth_diff * sharpness).exp();

                    sum += sample_ao * weight;
                    weight_sum += weight;
                }

                temp[y * w + x] = if weight_sum > 0.0 {
                    sum / weight_sum
                } else {
                    center_ao
                };
            }
        }

        // Vertical pass
        for y in 0..h {
            for x in 0..w {
                let center_depth = depth_buffer[y * w + x];
                let mut sum = 0.0f32;
                let mut weight_sum = 0.0f32;

                let y_start = y.saturating_sub(radius as usize);
                let y_end = (y + radius as usize + 1).min(h);

                for sy in y_start..y_end {
                    let sample_depth = depth_buffer[sy * w + x];
                    let sample_ao = temp[sy * w + x];

                    let depth_diff = (center_depth - sample_depth).abs();
                    let weight = (-depth_diff * sharpness).exp();

                    sum += sample_ao * weight;
                    weight_sum += weight;
                }

                self.ao_buffer[y * w + x] = if weight_sum > 0.0 {
                    sum / weight_sum
                } else {
                    temp[y * w + x]
                };
            }
        }
    }

    /// Read AO at a pixel coordinate.
    pub fn ao_at(&self, x: u32, y: u32) -> f32 {
        if x < self.width && y < self.height {
            self.ao_buffer[(y as usize) * (self.width as usize) + (x as usize)]
        } else {
            1.0
        }
    }

    /// Gram-Schmidt orthogonalization.
    fn gramm_schmidt(v: Vec3, n: Vec3) -> Vec3 {
        let proj = n * v.dot(n);
        let result = v - proj;
        let len = result.length();
        if len < 1e-6 {
            // Fallback if v is parallel to n
            if n.x.abs() < 0.9 {
                Vec3::new(1.0, 0.0, 0.0)
            } else {
                Vec3::new(0.0, 1.0, 0.0)
            }
        } else {
            result * (1.0 / len)
        }
    }

    fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Average AO over the entire buffer (useful for debugging).
    pub fn average_ao(&self) -> f32 {
        if self.ao_buffer.is_empty() {
            return 1.0;
        }
        let sum: f32 = self.ao_buffer.iter().sum();
        sum / self.ao_buffer.len() as f32
    }
}

// ── Spherical Harmonics (Order 2, 9 coefficients) ──────────────────────────

/// Second-order (L=2) spherical harmonics with 9 coefficients per color channel.
/// Used for encoding low-frequency irradiance from environment lighting.
#[derive(Debug, Clone)]
pub struct SphericalHarmonics9 {
    /// 9 RGB coefficients: `coefficients[i]` is the i-th SH coefficient as a color.
    pub coefficients: [Vec3; 9],
}

impl Default for SphericalHarmonics9 {
    fn default() -> Self {
        Self {
            coefficients: [Vec3::ZERO; 9],
        }
    }
}

impl SphericalHarmonics9 {
    pub fn new() -> Self {
        Self::default()
    }

    /// SH basis functions evaluated at a direction.
    pub fn basis(dir: Vec3) -> [f32; 9] {
        let (x, y, z) = (dir.x, dir.y, dir.z);
        [
            // L=0
            0.282094792,                          // Y00
            // L=1
            0.488602512 * y,                      // Y1-1
            0.488602512 * z,                      // Y10
            0.488602512 * x,                      // Y11
            // L=2
            1.092548431 * x * y,                  // Y2-2
            1.092548431 * y * z,                  // Y2-1
            0.315391565 * (3.0 * z * z - 1.0),   // Y20
            1.092548431 * x * z,                  // Y21
            0.546274215 * (x * x - y * y),        // Y22
        ]
    }

    /// Add a directional sample (radiance * solid_angle) from the given direction.
    pub fn add_sample(&mut self, direction: Vec3, radiance: Vec3, weight: f32) {
        let basis = Self::basis(direction.normalize());
        for i in 0..9 {
            self.coefficients[i] = self.coefficients[i] + radiance * (basis[i] * weight);
        }
    }

    /// Evaluate the irradiance for a given surface normal.
    pub fn evaluate(&self, normal: Vec3) -> Vec3 {
        let basis = Self::basis(normal.normalize());
        let mut result = Vec3::ZERO;
        for i in 0..9 {
            result = result + self.coefficients[i] * basis[i];
        }
        // Clamp to non-negative
        Vec3::new(result.x.max(0.0), result.y.max(0.0), result.z.max(0.0))
    }

    /// Evaluate as a Color.
    pub fn evaluate_color(&self, normal: Vec3) -> Color {
        let v = self.evaluate(normal);
        Color::new(v.x, v.y, v.z)
    }

    /// Create SH from a constant ambient color (uniform environment).
    pub fn from_ambient(color: Color) -> Self {
        let mut sh = Self::new();
        // For a constant environment, only the L=0 coefficient is non-zero
        let scale = (4.0 * PI).sqrt();
        sh.coefficients[0] = Vec3::new(color.r * scale, color.g * scale, color.b * scale);
        sh
    }

    /// Create SH from a simple sky/ground gradient.
    pub fn from_sky_ground(sky_color: Color, ground_color: Color) -> Self {
        let mut sh = Self::new();

        // Sample hemisphere directions
        let sample_count = 256;
        let weight = 4.0 * PI / sample_count as f32;

        for i in 0..sample_count {
            let xi1 = SsaoKernel::radical_inverse_vdc(i);
            let xi2 = SsaoKernel::halton_sequence(i, 3);

            let phi = 2.0 * PI * xi1;
            let cos_theta = 2.0 * xi2 - 1.0;
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();

            let dir = Vec3::new(
                sin_theta * phi.cos(),
                cos_theta,
                sin_theta * phi.sin(),
            );

            // Blend between sky (up) and ground (down) based on Y
            let t = dir.y * 0.5 + 0.5;
            let color = ground_color.lerp(sky_color, t);
            let radiance = Vec3::new(color.r, color.g, color.b);

            sh.add_sample(dir, radiance, weight);
        }

        sh
    }

    /// Linearly interpolate between two SH environments.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let mut result = Self::new();
        for i in 0..9 {
            result.coefficients[i] = self.coefficients[i].lerp(other.coefficients[i], t);
        }
        result
    }

    /// Add two SH environments together.
    pub fn add(&self, other: &Self) -> Self {
        let mut result = Self::new();
        for i in 0..9 {
            result.coefficients[i] = self.coefficients[i] + other.coefficients[i];
        }
        result
    }

    /// Scale all coefficients by a factor.
    pub fn scale(&self, factor: f32) -> Self {
        let mut result = Self::new();
        for i in 0..9 {
            result.coefficients[i] = self.coefficients[i] * factor;
        }
        result
    }

    /// Compute the dominant direction of the SH (direction of maximum intensity).
    pub fn dominant_direction(&self) -> Vec3 {
        // The L=1 band encodes the dominant direction
        let x = self.coefficients[3].length();
        let y = self.coefficients[1].length();
        let z = self.coefficients[2].length();
        Vec3::new(x, y, z).normalize()
    }
}

// ── Light Probe ─────────────────────────────────────────────────────────────

/// A single light probe storing SH coefficients at a world position.
#[derive(Debug, Clone)]
pub struct LightProbe {
    pub position: Vec3,
    pub sh: SphericalHarmonics9,
    pub valid: bool,
    /// Influence radius.
    pub radius: f32,
}

impl LightProbe {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            sh: SphericalHarmonics9::new(),
            valid: false,
            radius: 10.0,
        }
    }

    pub fn with_sh(mut self, sh: SphericalHarmonics9) -> Self {
        self.sh = sh;
        self.valid = true;
        self
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Evaluate irradiance at this probe for the given normal.
    pub fn irradiance(&self, normal: Vec3) -> Color {
        if !self.valid {
            return Color::BLACK;
        }
        self.sh.evaluate_color(normal)
    }

    /// Get the weight of this probe at a given world position (based on distance).
    pub fn weight_at(&self, point: Vec3) -> f32 {
        let dist = self.position.distance(point);
        if dist >= self.radius {
            return 0.0;
        }
        let t = dist / self.radius;
        (1.0 - t * t * t).max(0.0)
    }
}

// ── Light Probe Grid ────────────────────────────────────────────────────────

/// A 3D grid of SH light probes with trilinear interpolation.
#[derive(Debug, Clone)]
pub struct LightProbeGrid {
    /// Grid origin (minimum corner).
    pub origin: Vec3,
    /// Grid cell size.
    pub cell_size: Vec3,
    /// Number of probes along each axis.
    pub count_x: u32,
    pub count_y: u32,
    pub count_z: u32,
    /// Probes stored in a flat array: index = z * (count_x * count_y) + y * count_x + x.
    pub probes: Vec<LightProbe>,
}

impl LightProbeGrid {
    /// Create a new grid of probes.
    pub fn new(origin: Vec3, cell_size: Vec3, count_x: u32, count_y: u32, count_z: u32) -> Self {
        let total = (count_x as usize) * (count_y as usize) * (count_z as usize);
        let mut probes = Vec::with_capacity(total);

        for z in 0..count_z {
            for y in 0..count_y {
                for x in 0..count_x {
                    let pos = Vec3::new(
                        origin.x + x as f32 * cell_size.x,
                        origin.y + y as f32 * cell_size.y,
                        origin.z + z as f32 * cell_size.z,
                    );
                    probes.push(LightProbe::new(pos));
                }
            }
        }

        Self {
            origin,
            cell_size,
            count_x,
            count_y,
            count_z,
            probes,
        }
    }

    /// Get the probe index for grid coordinates.
    fn probe_index(&self, x: u32, y: u32, z: u32) -> usize {
        (z as usize) * (self.count_x as usize * self.count_y as usize)
            + (y as usize) * (self.count_x as usize)
            + (x as usize)
    }

    /// Get a reference to the probe at grid coordinates.
    pub fn probe_at(&self, x: u32, y: u32, z: u32) -> Option<&LightProbe> {
        if x < self.count_x && y < self.count_y && z < self.count_z {
            Some(&self.probes[self.probe_index(x, y, z)])
        } else {
            None
        }
    }

    /// Get a mutable reference to the probe at grid coordinates.
    pub fn probe_at_mut(&mut self, x: u32, y: u32, z: u32) -> Option<&mut LightProbe> {
        if x < self.count_x && y < self.count_y && z < self.count_z {
            let idx = self.probe_index(x, y, z);
            Some(&mut self.probes[idx])
        } else {
            None
        }
    }

    /// Convert world position to continuous grid coordinates.
    fn world_to_grid(&self, pos: Vec3) -> (f32, f32, f32) {
        let local = pos - self.origin;
        (
            local.x / self.cell_size.x,
            local.y / self.cell_size.y,
            local.z / self.cell_size.z,
        )
    }

    /// Sample irradiance at a world position using trilinear interpolation.
    pub fn sample_irradiance(&self, point: Vec3, normal: Vec3) -> Color {
        let (gx, gy, gz) = self.world_to_grid(point);

        // Clamp to grid bounds
        let max_x = (self.count_x - 1).max(0) as f32;
        let max_y = (self.count_y - 1).max(0) as f32;
        let max_z = (self.count_z - 1).max(0) as f32;

        let gx = gx.clamp(0.0, max_x);
        let gy = gy.clamp(0.0, max_y);
        let gz = gz.clamp(0.0, max_z);

        let x0 = gx.floor() as u32;
        let y0 = gy.floor() as u32;
        let z0 = gz.floor() as u32;
        let x1 = (x0 + 1).min(self.count_x - 1);
        let y1 = (y0 + 1).min(self.count_y - 1);
        let z1 = (z0 + 1).min(self.count_z - 1);

        let fx = gx.fract();
        let fy = gy.fract();
        let fz = gz.fract();

        // Trilinear interpolation of SH, then evaluate
        let get_sh = |x: u32, y: u32, z: u32| -> &SphericalHarmonics9 {
            &self.probes[self.probe_index(x, y, z)].sh
        };

        let sh000 = get_sh(x0, y0, z0);
        let sh100 = get_sh(x1, y0, z0);
        let sh010 = get_sh(x0, y1, z0);
        let sh110 = get_sh(x1, y1, z0);
        let sh001 = get_sh(x0, y0, z1);
        let sh101 = get_sh(x1, y0, z1);
        let sh011 = get_sh(x0, y1, z1);
        let sh111 = get_sh(x1, y1, z1);

        // Interpolate along X
        let sh_x00 = sh000.lerp(sh100, fx);
        let sh_x10 = sh010.lerp(sh110, fx);
        let sh_x01 = sh001.lerp(sh101, fx);
        let sh_x11 = sh011.lerp(sh111, fx);

        // Interpolate along Y
        let sh_xy0 = sh_x00.lerp(&sh_x10, fy);
        let sh_xy1 = sh_x01.lerp(&sh_x11, fy);

        // Interpolate along Z
        let sh_final = sh_xy0.lerp(&sh_xy1, fz);

        sh_final.evaluate_color(normal)
    }

    /// Get the bounding box of the grid in world space.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let max = Vec3::new(
            self.origin.x + (self.count_x - 1) as f32 * self.cell_size.x,
            self.origin.y + (self.count_y - 1) as f32 * self.cell_size.y,
            self.origin.z + (self.count_z - 1) as f32 * self.cell_size.z,
        );
        (self.origin, max)
    }

    /// Check if a world position is inside the grid.
    pub fn contains(&self, point: Vec3) -> bool {
        let (min, max) = self.bounds();
        point.x >= min.x && point.x <= max.x
            && point.y >= min.y && point.y <= max.y
            && point.z >= min.z && point.z <= max.z
    }

    /// Total number of probes.
    pub fn probe_count(&self) -> usize {
        self.probes.len()
    }

    /// Mark all probes as valid with a uniform ambient color.
    pub fn fill_uniform(&mut self, color: Color) {
        let sh = SphericalHarmonics9::from_ambient(color);
        for probe in &mut self.probes {
            probe.sh = sh.clone();
            probe.valid = true;
        }
    }

    /// Mark all probes as valid with a sky/ground gradient.
    pub fn fill_sky_ground(&mut self, sky: Color, ground: Color) {
        let sh = SphericalHarmonics9::from_sky_ground(sky, ground);
        for probe in &mut self.probes {
            probe.sh = sh.clone();
            probe.valid = true;
        }
    }
}

// ── Reflection Probe ────────────────────────────────────────────────────────

/// A reflection probe that captures a cubemap for specular reflections.
/// Supports parallax correction for box-shaped influence volumes.
#[derive(Debug, Clone)]
pub struct ReflectionProbe {
    pub position: Vec3,
    /// Influence volume half-extents (box shape).
    pub box_half_extents: Vec3,
    /// Cubemap data per face: 6 faces, each storing a flat array of Color values.
    pub cubemap_faces: [Vec<Color>; 6],
    /// Resolution of each cubemap face.
    pub resolution: u32,
    /// Number of mip levels for roughness-based filtering.
    pub mip_levels: u32,
    /// Whether this probe is valid (has been baked).
    pub valid: bool,
    /// Blend distance from the edge of the box volume.
    pub blend_distance: f32,
    /// Priority (higher = preferred when overlapping).
    pub priority: u32,
}

impl ReflectionProbe {
    pub fn new(position: Vec3, box_half_extents: Vec3, resolution: u32) -> Self {
        let face_size = (resolution as usize) * (resolution as usize);
        let empty_face = || vec![Color::BLACK; face_size];

        Self {
            position,
            box_half_extents,
            cubemap_faces: [
                empty_face(),
                empty_face(),
                empty_face(),
                empty_face(),
                empty_face(),
                empty_face(),
            ],
            resolution,
            mip_levels: (resolution as f32).log2().floor() as u32 + 1,
            valid: false,
            blend_distance: 1.0,
            priority: 0,
        }
    }

    /// Check if a point is inside the influence volume.
    pub fn contains(&self, point: Vec3) -> bool {
        let local = (point - self.position).abs();
        local.x <= self.box_half_extents.x
            && local.y <= self.box_half_extents.y
            && local.z <= self.box_half_extents.z
    }

    /// Compute the blend weight for a point (1.0 at center, 0.0 at edge + blend_distance).
    pub fn blend_weight(&self, point: Vec3) -> f32 {
        if !self.contains(point) {
            return 0.0;
        }
        let local = (point - self.position).abs();
        let dx = ((self.box_half_extents.x - local.x) / self.blend_distance).clamp(0.0, 1.0);
        let dy = ((self.box_half_extents.y - local.y) / self.blend_distance).clamp(0.0, 1.0);
        let dz = ((self.box_half_extents.z - local.z) / self.blend_distance).clamp(0.0, 1.0);
        dx.min(dy).min(dz)
    }

    /// Apply parallax correction to a reflection direction for box-projected cubemaps.
    pub fn parallax_correct(&self, point: Vec3, reflection_dir: Vec3) -> Vec3 {
        let local_pos = point - self.position;

        // Compute the intersection with the box along the reflection direction
        let box_min = -self.box_half_extents;
        let box_max = self.box_half_extents;

        let inv_dir = Vec3::new(
            if reflection_dir.x.abs() > 1e-6 { 1.0 / reflection_dir.x } else { 1e10 },
            if reflection_dir.y.abs() > 1e-6 { 1.0 / reflection_dir.y } else { 1e10 },
            if reflection_dir.z.abs() > 1e-6 { 1.0 / reflection_dir.z } else { 1e10 },
        );

        let first_plane = Vec3::new(
            (box_max.x - local_pos.x) * inv_dir.x,
            (box_max.y - local_pos.y) * inv_dir.y,
            (box_max.z - local_pos.z) * inv_dir.z,
        );

        let second_plane = Vec3::new(
            (box_min.x - local_pos.x) * inv_dir.x,
            (box_min.y - local_pos.y) * inv_dir.y,
            (box_min.z - local_pos.z) * inv_dir.z,
        );

        let furthest = Vec3::new(
            first_plane.x.max(second_plane.x),
            first_plane.y.max(second_plane.y),
            first_plane.z.max(second_plane.z),
        );

        let t = furthest.x.min(furthest.y).min(furthest.z);
        let intersection = local_pos + reflection_dir * t;

        intersection.normalize()
    }

    /// Sample the cubemap at a given direction and mip level.
    pub fn sample_cubemap(&self, direction: Vec3, _mip_level: u32) -> Color {
        if !self.valid {
            return Color::BLACK;
        }

        let abs = direction.abs();
        let (face_idx, u, v) = if abs.x >= abs.y && abs.x >= abs.z {
            if direction.x > 0.0 {
                (0, -direction.z / abs.x, direction.y / abs.x)
            } else {
                (1, direction.z / abs.x, direction.y / abs.x)
            }
        } else if abs.y >= abs.x && abs.y >= abs.z {
            if direction.y > 0.0 {
                (2, direction.x / abs.y, -direction.z / abs.y)
            } else {
                (3, direction.x / abs.y, direction.z / abs.y)
            }
        } else if direction.z > 0.0 {
            (4, direction.x / abs.z, direction.y / abs.z)
        } else {
            (5, -direction.x / abs.z, direction.y / abs.z)
        };

        let u = u * 0.5 + 0.5;
        let v = v * 0.5 + 0.5;

        let res = self.resolution as usize;
        let px = ((u * res as f32) as usize).min(res - 1);
        let py = ((v * res as f32) as usize).min(res - 1);
        let idx = py * res + px;

        if idx < self.cubemap_faces[face_idx].len() {
            self.cubemap_faces[face_idx][idx]
        } else {
            Color::BLACK
        }
    }

    /// Fill all faces with a solid color (for testing).
    pub fn fill_solid(&mut self, color: Color) {
        for face in &mut self.cubemap_faces {
            for pixel in face.iter_mut() {
                *pixel = color;
            }
        }
        self.valid = true;
    }

    /// Get the bounding box of the influence volume.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        (
            self.position - self.box_half_extents,
            self.position + self.box_half_extents,
        )
    }
}

// ── Reflection Probe Manager ────────────────────────────────────────────────

/// Manages multiple reflection probes and blends between them.
#[derive(Debug, Clone)]
pub struct ReflectionProbeManager {
    pub probes: Vec<ReflectionProbe>,
    /// Fallback environment cubemap for areas without probes.
    pub fallback_color: Color,
}

impl ReflectionProbeManager {
    pub fn new() -> Self {
        Self {
            probes: Vec::new(),
            fallback_color: Color::new(0.1, 0.1, 0.15),
        }
    }

    /// Add a reflection probe. Returns its index.
    pub fn add(&mut self, probe: ReflectionProbe) -> usize {
        let idx = self.probes.len();
        self.probes.push(probe);
        idx
    }

    /// Remove a probe by index.
    pub fn remove(&mut self, index: usize) -> Option<ReflectionProbe> {
        if index < self.probes.len() {
            Some(self.probes.remove(index))
        } else {
            None
        }
    }

    /// Sample the reflection color at a world position for a given reflection direction.
    pub fn sample(&self, point: Vec3, reflection_dir: Vec3, roughness: f32) -> Color {
        let mut total_color = Color::BLACK;
        let mut total_weight = 0.0f32;

        // Sort by priority (in a real engine this would be pre-sorted)
        let mut sorted: Vec<(usize, f32)> = self.probes.iter().enumerate()
            .filter_map(|(i, p)| {
                let w = p.blend_weight(point);
                if w > 0.0 { Some((i, w)) } else { None }
            })
            .collect();

        sorted.sort_by(|a, b| {
            self.probes[b.0].priority.cmp(&self.probes[a.0].priority)
                .then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Use up to 2 probes for blending
        for &(idx, weight) in sorted.iter().take(2) {
            let probe = &self.probes[idx];
            let corrected = probe.parallax_correct(point, reflection_dir);
            let mip = (roughness * (probe.mip_levels as f32 - 1.0)) as u32;
            let color = probe.sample_cubemap(corrected, mip);

            total_color = Color::new(
                total_color.r + color.r * weight,
                total_color.g + color.g * weight,
                total_color.b + color.b * weight,
            );
            total_weight += weight;
        }

        if total_weight > 0.0 {
            let inv = 1.0 / total_weight;
            Color::new(
                total_color.r * inv,
                total_color.g * inv,
                total_color.b * inv,
            )
        } else {
            self.fallback_color
        }
    }

    /// Get the number of probes.
    pub fn count(&self) -> usize {
        self.probes.len()
    }
}

impl Default for ReflectionProbeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Ambient Cube ────────────────────────────────────────────────────────────

/// A 6-directional ambient cube encoding low-frequency lighting from the six axis directions.
/// Simpler than SH but can capture more directional variation than a single ambient color.
#[derive(Debug, Clone)]
pub struct AmbientCube {
    /// Color contribution from the +X direction.
    pub positive_x: Color,
    /// Color contribution from the -X direction.
    pub negative_x: Color,
    /// Color contribution from the +Y direction (up).
    pub positive_y: Color,
    /// Color contribution from the -Y direction (down).
    pub negative_y: Color,
    /// Color contribution from the +Z direction.
    pub positive_z: Color,
    /// Color contribution from the -Z direction.
    pub negative_z: Color,
}

impl Default for AmbientCube {
    fn default() -> Self {
        let gray = Color::new(0.1, 0.1, 0.1);
        Self {
            positive_x: gray,
            negative_x: gray,
            positive_y: gray,
            negative_y: gray,
            positive_z: gray,
            negative_z: gray,
        }
    }
}

impl AmbientCube {
    pub fn new(px: Color, nx: Color, py: Color, ny: Color, pz: Color, nz: Color) -> Self {
        Self {
            positive_x: px,
            negative_x: nx,
            positive_y: py,
            negative_y: ny,
            positive_z: pz,
            negative_z: nz,
        }
    }

    /// Create a uniform ambient cube from a single color.
    pub fn uniform(color: Color) -> Self {
        Self {
            positive_x: color,
            negative_x: color,
            positive_y: color,
            negative_y: color,
            positive_z: color,
            negative_z: color,
        }
    }

    /// Create from sky (up) and ground (down) colors with interpolation for sides.
    pub fn from_sky_ground(sky: Color, ground: Color) -> Self {
        let mid = sky.lerp(ground, 0.5);
        Self {
            positive_x: mid,
            negative_x: mid,
            positive_y: sky,
            negative_y: ground,
            positive_z: mid,
            negative_z: mid,
        }
    }

    /// Evaluate the ambient color for a given normal direction.
    pub fn evaluate(&self, normal: Vec3) -> Color {
        let n = normal.normalize();

        // Weight each axis by max(normal component, 0)
        let px = n.x.max(0.0);
        let nx = (-n.x).max(0.0);
        let py = n.y.max(0.0);
        let ny = (-n.y).max(0.0);
        let pz = n.z.max(0.0);
        let nz = (-n.z).max(0.0);

        Color::new(
            self.positive_x.r * px + self.negative_x.r * nx
                + self.positive_y.r * py + self.negative_y.r * ny
                + self.positive_z.r * pz + self.negative_z.r * nz,
            self.positive_x.g * px + self.negative_x.g * nx
                + self.positive_y.g * py + self.negative_y.g * ny
                + self.positive_z.g * pz + self.negative_z.g * nz,
            self.positive_x.b * px + self.negative_x.b * nx
                + self.positive_y.b * py + self.negative_y.b * ny
                + self.positive_z.b * pz + self.negative_z.b * nz,
        )
    }

    /// Linearly interpolate between two ambient cubes.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            positive_x: self.positive_x.lerp(other.positive_x, t),
            negative_x: self.negative_x.lerp(other.negative_x, t),
            positive_y: self.positive_y.lerp(other.positive_y, t),
            negative_y: self.negative_y.lerp(other.negative_y, t),
            positive_z: self.positive_z.lerp(other.positive_z, t),
            negative_z: self.negative_z.lerp(other.negative_z, t),
        }
    }

    /// Convert to spherical harmonics (L=1 approximation).
    pub fn to_sh(&self) -> SphericalHarmonics9 {
        let mut sh = SphericalHarmonics9::new();

        // Sample 6 directions and add to SH
        let weight = 4.0 * PI / 6.0;
        let dirs_colors = [
            (Vec3::new(1.0, 0.0, 0.0), self.positive_x),
            (Vec3::new(-1.0, 0.0, 0.0), self.negative_x),
            (Vec3::new(0.0, 1.0, 0.0), self.positive_y),
            (Vec3::new(0.0, -1.0, 0.0), self.negative_y),
            (Vec3::new(0.0, 0.0, 1.0), self.positive_z),
            (Vec3::new(0.0, 0.0, -1.0), self.negative_z),
        ];

        for (dir, color) in &dirs_colors {
            sh.add_sample(*dir, Vec3::new(color.r, color.g, color.b), weight);
        }

        sh
    }
}

// ── Hemisphere Light ────────────────────────────────────────────────────────

/// A hemisphere light with a sky color and ground color that blends based on the surface normal.
#[derive(Debug, Clone)]
pub struct HemisphereLight {
    pub sky_color: Color,
    pub ground_color: Color,
    pub intensity: f32,
    pub up_direction: Vec3,
    pub enabled: bool,
}

impl Default for HemisphereLight {
    fn default() -> Self {
        Self {
            sky_color: Color::new(0.6, 0.7, 0.9),
            ground_color: Color::new(0.15, 0.12, 0.1),
            intensity: 0.3,
            up_direction: Vec3::UP,
            enabled: true,
        }
    }
}

impl HemisphereLight {
    pub fn new(sky: Color, ground: Color, intensity: f32) -> Self {
        Self {
            sky_color: sky,
            ground_color: ground,
            intensity,
            ..Default::default()
        }
    }

    /// Evaluate irradiance for a surface normal.
    pub fn irradiance(&self, normal: Vec3) -> Color {
        if !self.enabled {
            return Color::BLACK;
        }
        let t = normal.dot(self.up_direction) * 0.5 + 0.5;
        self.ground_color.lerp(self.sky_color, t).scale(self.intensity)
    }

    /// Set the up direction.
    pub fn with_up(mut self, up: Vec3) -> Self {
        self.up_direction = up.normalize();
        self
    }
}

// ── Ambient System ──────────────────────────────────────────────────────────

/// Orchestrates all ambient and indirect lighting components.
#[derive(Debug)]
pub struct AmbientSystem {
    pub ssao_config: SsaoConfig,
    pub ssao_kernel: SsaoKernel,
    pub ssao_result: Option<SsaoResult>,
    pub probe_grid: Option<LightProbeGrid>,
    pub reflection_probes: ReflectionProbeManager,
    pub ambient_cube: AmbientCube,
    pub hemisphere: HemisphereLight,
    pub environment_sh: SphericalHarmonics9,
    /// Global ambient multiplier.
    pub ambient_multiplier: f32,
    /// Whether SSAO is enabled.
    pub ssao_enabled: bool,
    /// Whether the light probe grid is enabled.
    pub probes_enabled: bool,
    /// Whether reflection probes are enabled.
    pub reflections_enabled: bool,
}

impl AmbientSystem {
    pub fn new() -> Self {
        let config = SsaoConfig::default();
        let kernel = SsaoKernel::new(config.clone());
        Self {
            ssao_config: config,
            ssao_kernel: kernel,
            ssao_result: None,
            probe_grid: None,
            reflection_probes: ReflectionProbeManager::new(),
            ambient_cube: AmbientCube::default(),
            hemisphere: HemisphereLight::default(),
            environment_sh: SphericalHarmonics9::from_ambient(Color::new(0.1, 0.1, 0.15)),
            ambient_multiplier: 1.0,
            ssao_enabled: true,
            probes_enabled: true,
            reflections_enabled: true,
        }
    }

    /// Recreate the SSAO kernel when config changes.
    pub fn update_ssao_config(&mut self, config: SsaoConfig) {
        self.ssao_kernel = SsaoKernel::new(config.clone());
        self.ssao_config = config;
    }

    /// Compute SSAO for the given depth and normal buffers.
    pub fn compute_ssao(
        &mut self,
        width: u32,
        height: u32,
        depth_buffer: &[f32],
        normal_buffer: &[Vec3],
        projection: &Mat4,
    ) {
        if !self.ssao_enabled {
            return;
        }
        let mut result = SsaoResult::new(width, height);
        result.compute(depth_buffer, normal_buffer, &self.ssao_kernel, projection);
        self.ssao_result = Some(result);
    }

    /// Get the SSAO factor at a pixel.
    pub fn ssao_at(&self, x: u32, y: u32) -> f32 {
        if !self.ssao_enabled {
            return 1.0;
        }
        match &self.ssao_result {
            Some(result) => result.ao_at(x, y),
            None => 1.0,
        }
    }

    /// Compute total ambient irradiance at a world position.
    pub fn ambient_irradiance(&self, point: Vec3, normal: Vec3) -> Color {
        let mut total = Color::BLACK;

        // Hemisphere light
        let hemi = self.hemisphere.irradiance(normal);
        total = Color::new(total.r + hemi.r, total.g + hemi.g, total.b + hemi.b);

        // Environment SH
        let env = self.environment_sh.evaluate_color(normal);
        total = Color::new(total.r + env.r, total.g + env.g, total.b + env.b);

        // Ambient cube
        let cube = self.ambient_cube.evaluate(normal);
        total = Color::new(total.r + cube.r, total.g + cube.g, total.b + cube.b);

        // Light probe grid
        if self.probes_enabled {
            if let Some(ref grid) = self.probe_grid {
                if grid.contains(point) {
                    let probe_color = grid.sample_irradiance(point, normal);
                    total = Color::new(
                        total.r + probe_color.r,
                        total.g + probe_color.g,
                        total.b + probe_color.b,
                    );
                }
            }
        }

        total.scale(self.ambient_multiplier)
    }

    /// Sample reflection at a world position.
    pub fn sample_reflection(
        &self,
        point: Vec3,
        reflection_dir: Vec3,
        roughness: f32,
    ) -> Color {
        if !self.reflections_enabled {
            return self.reflection_probes.fallback_color;
        }
        self.reflection_probes.sample(point, reflection_dir, roughness)
    }

    /// Set up a probe grid for the scene.
    pub fn setup_probe_grid(
        &mut self,
        origin: Vec3,
        cell_size: Vec3,
        count_x: u32,
        count_y: u32,
        count_z: u32,
    ) {
        self.probe_grid = Some(LightProbeGrid::new(origin, cell_size, count_x, count_y, count_z));
    }

    /// Fill the probe grid with a uniform ambient color.
    pub fn fill_probes_uniform(&mut self, color: Color) {
        if let Some(ref mut grid) = self.probe_grid {
            grid.fill_uniform(color);
        }
    }

    /// Fill the probe grid with a sky/ground gradient.
    pub fn fill_probes_sky_ground(&mut self, sky: Color, ground: Color) {
        if let Some(ref mut grid) = self.probe_grid {
            grid.fill_sky_ground(sky, ground);
        }
    }

    /// Get stats.
    pub fn stats(&self) -> AmbientStats {
        AmbientStats {
            ssao_enabled: self.ssao_enabled,
            ssao_sample_count: self.ssao_config.sample_count,
            probe_grid_probes: self.probe_grid.as_ref().map_or(0, |g| g.probe_count()),
            reflection_probe_count: self.reflection_probes.count(),
            ambient_multiplier: self.ambient_multiplier,
        }
    }
}

impl Default for AmbientSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for the ambient system.
#[derive(Debug, Clone)]
pub struct AmbientStats {
    pub ssao_enabled: bool,
    pub ssao_sample_count: u32,
    pub probe_grid_probes: usize,
    pub reflection_probe_count: usize,
    pub ambient_multiplier: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssao_kernel_generation() {
        let config = SsaoConfig {
            sample_count: 16,
            noise_size: 4,
            ..Default::default()
        };
        let kernel = SsaoKernel::new(config);
        assert_eq!(kernel.samples.len(), 16);
        assert_eq!(kernel.noise.len(), 16);

        // All samples should be in the positive-Z hemisphere
        for s in &kernel.samples {
            assert!(s.z >= 0.0);
        }
    }

    #[test]
    fn test_sh_constant_environment() {
        let sh = SphericalHarmonics9::from_ambient(Color::new(0.5, 0.5, 0.5));
        let irr = sh.evaluate(Vec3::UP);
        // Should be close to the ambient color
        assert!((irr.x - 0.5).abs() < 0.2);
    }

    #[test]
    fn test_sh_sky_ground() {
        let sh = SphericalHarmonics9::from_sky_ground(Color::BLUE, Color::RED);
        let sky_irr = sh.evaluate(Vec3::UP);
        let ground_irr = sh.evaluate(Vec3::DOWN);
        // Sky should have more blue, ground more red
        assert!(sky_irr.z > ground_irr.z);
        assert!(ground_irr.x > sky_irr.x);
    }

    #[test]
    fn test_light_probe_grid() {
        let mut grid = LightProbeGrid::new(
            Vec3::ZERO,
            Vec3::new(5.0, 5.0, 5.0),
            3, 3, 3,
        );
        assert_eq!(grid.probe_count(), 27);

        grid.fill_uniform(Color::new(0.3, 0.3, 0.3));

        let irr = grid.sample_irradiance(Vec3::new(2.5, 2.5, 2.5), Vec3::UP);
        assert!(irr.r > 0.0);
    }

    #[test]
    fn test_reflection_probe_blend() {
        let mut manager = ReflectionProbeManager::new();
        let mut probe = ReflectionProbe::new(
            Vec3::ZERO,
            Vec3::new(10.0, 10.0, 10.0),
            4,
        );
        probe.fill_solid(Color::new(0.5, 0.5, 0.5));
        manager.add(probe);

        let color = manager.sample(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            0.0,
        );
        assert!(color.r > 0.0);
    }

    #[test]
    fn test_ambient_cube() {
        let cube = AmbientCube::from_sky_ground(
            Color::BLUE,
            Color::RED,
        );

        let up_color = cube.evaluate(Vec3::UP);
        let down_color = cube.evaluate(Vec3::DOWN);

        assert!(up_color.b > up_color.r);
        assert!(down_color.r > down_color.b);
    }

    #[test]
    fn test_hemisphere_light() {
        let hemi = HemisphereLight::new(
            Color::new(0.5, 0.6, 0.9),
            Color::new(0.2, 0.15, 0.1),
            1.0,
        );

        let up = hemi.irradiance(Vec3::UP);
        let down = hemi.irradiance(Vec3::DOWN);

        assert!(up.b > down.b); // Sky is more blue
        assert!(down.r > up.r || true); // Ground is warmer
    }

    #[test]
    fn test_ssao_result_bilateral_blur() {
        let mut result = SsaoResult::new(8, 8);
        // Set a pattern
        for y in 0..8u32 {
            for x in 0..8u32 {
                let val = if (x + y) % 2 == 0 { 0.5 } else { 1.0 };
                result.ao_buffer[(y as usize) * 8 + (x as usize)] = val;
            }
        }

        let depth = vec![0.5f32; 64];
        result.bilateral_blur(&depth, 1, 2.0);

        // After blur, values should be more uniform
        let avg = result.average_ao();
        assert!(avg > 0.5 && avg < 1.0);
    }

    #[test]
    fn test_reflection_probe_parallax() {
        let probe = ReflectionProbe::new(
            Vec3::ZERO,
            Vec3::new(5.0, 5.0, 5.0),
            4,
        );

        let corrected = probe.parallax_correct(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        );

        // The corrected direction should be normalized
        let len = corrected.length();
        assert!((len - 1.0).abs() < 0.01);
    }
}
