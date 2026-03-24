//! Post-processing pipeline: TAA, SSAO, DoF, motion blur, chromatic aberration,
//! lens flares, film grain, vignette, and a compositing stack.

use glam::{Vec2, Vec3, Vec4, Mat4, Quat};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// RenderTarget (CPU-side representation; GPU handle managed externally)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RenderTarget {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Vec4>,
}

impl RenderTarget {
    pub fn new(width: u32, height: u32) -> Self {
        let count = (width * height) as usize;
        Self {
            width,
            height,
            pixels: vec![Vec4::ZERO; count],
        }
    }

    pub fn new_with_color(width: u32, height: u32, color: Vec4) -> Self {
        let count = (width * height) as usize;
        Self { width, height, pixels: vec![color; count] }
    }

    pub fn pixel(&self, x: u32, y: u32) -> Vec4 {
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        self.pixels[(y * self.width + x) as usize]
    }

    pub fn pixel_mut(&mut self, x: u32, y: u32) -> &mut Vec4 {
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        let idx = (y * self.width + x) as usize;
        &mut self.pixels[idx]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Vec4) {
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        let idx = (y * self.width + x) as usize;
        if idx < self.pixels.len() {
            self.pixels[idx] = color;
        }
    }

    /// Sample with bilinear interpolation.
    pub fn sample_bilinear(&self, uv: Vec2) -> Vec4 {
        let x = (uv.x * self.width as f32 - 0.5).max(0.0);
        let y = (uv.y * self.height as f32 - 0.5).max(0.0);
        let x0 = (x.floor() as u32).min(self.width - 1);
        let y0 = (y.floor() as u32).min(self.height - 1);
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let fx = x - x.floor();
        let fy = y - y.floor();
        let c00 = self.pixel(x0, y0);
        let c10 = self.pixel(x1, y0);
        let c01 = self.pixel(x0, y1);
        let c11 = self.pixel(x1, y1);
        let c0 = c00 + (c10 - c00) * fx;
        let c1 = c01 + (c11 - c01) * fx;
        c0 + (c1 - c0) * fy
    }

    /// Sample with UV clamping.
    pub fn sample(&self, uv: Vec2) -> Vec4 {
        let uv = Vec2::new(uv.x.clamp(0.0, 1.0), uv.y.clamp(0.0, 1.0));
        self.sample_bilinear(uv)
    }

    pub fn clear(&mut self, color: Vec4) {
        for p in &mut self.pixels {
            *p = color;
        }
    }

    pub fn is_same_size(&self, other: &RenderTarget) -> bool {
        self.width == other.width && self.height == other.height
    }
}

// ---------------------------------------------------------------------------
// PostParams
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PostParams {
    pub exposure: f32,
    pub gamma: f32,
    pub time: f32,
    pub view_pos: Vec3,
    pub resolution: [u32; 2],
    pub frame_index: u32,
    pub delta_time: f32,
}

impl PostParams {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            exposure: 1.0,
            gamma: 2.2,
            time: 0.0,
            view_pos: Vec3::ZERO,
            resolution: [width, height],
            frame_index: 0,
            delta_time: 0.016,
        }
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.resolution[0] as f32 / self.resolution[1] as f32
    }
}

// ---------------------------------------------------------------------------
// PostProcessEffect trait
// ---------------------------------------------------------------------------

pub trait PostProcessEffect {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, params: &PostParams);
    fn name(&self) -> &str;
    fn is_enabled(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Utility — Halton sequence
// ---------------------------------------------------------------------------

pub fn halton(index: u32, base: u32) -> f32 {
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

// ---------------------------------------------------------------------------
// TemporalAA
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TemporalAA {
    pub jitter_pattern: Vec<[f32; 2]>,
    pub history_blend: f32,
    pub ghosting_threshold: f32,
    pub sharpness: f32,
    pub enabled: bool,
}

impl TemporalAA {
    pub fn new() -> Self {
        let mut pattern = Vec::with_capacity(16);
        for i in 0..16u32 {
            let x = halton(i + 1, 2) - 0.5;
            let y = halton(i + 1, 3) - 0.5;
            pattern.push([x, y]);
        }
        Self {
            jitter_pattern: pattern,
            history_blend: 0.1,
            ghosting_threshold: 0.1,
            sharpness: 0.25,
            enabled: true,
        }
    }

    pub fn sample_jitter(&self, frame: u32) -> [f32; 2] {
        let idx = (frame as usize) % self.jitter_pattern.len();
        self.jitter_pattern[idx]
    }

    /// Clamp color to neighborhood AABB to reduce ghosting.
    fn clip_to_aabb(history: Vec4, min_c: Vec4, max_c: Vec4) -> Vec4 {
        let center = (min_c + max_c) * 0.5;
        let extent = (max_c - min_c) * 0.5;
        let d = history - center;
        // Scale to box boundary
        let scale_x = if d.x.abs() > 0.0001 { (extent.x / d.x.abs()).min(1.0) } else { 1.0 };
        let scale_y = if d.y.abs() > 0.0001 { (extent.y / d.y.abs()).min(1.0) } else { 1.0 };
        let scale_z = if d.z.abs() > 0.0001 { (extent.z / d.z.abs()).min(1.0) } else { 1.0 };
        let scale = scale_x.min(scale_y).min(scale_z).min(1.0);
        center + d * scale
    }

    /// Compute neighborhood min/max (3x3 kernel).
    fn neighborhood_aabb(rt: &RenderTarget, x: u32, y: u32) -> (Vec4, Vec4) {
        let mut min_c = Vec4::splat(f32::MAX);
        let mut max_c = Vec4::splat(-f32::MAX);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let sx = (x as i32 + dx).clamp(0, rt.width as i32 - 1) as u32;
                let sy = (y as i32 + dy).clamp(0, rt.height as i32 - 1) as u32;
                let c = rt.pixel(sx, sy);
                min_c = Vec4::new(min_c.x.min(c.x), min_c.y.min(c.y), min_c.z.min(c.z), min_c.w.min(c.w));
                max_c = Vec4::new(max_c.x.max(c.x), max_c.y.max(c.y), max_c.z.max(c.z), max_c.w.max(c.w));
            }
        }
        (min_c, max_c)
    }

    /// Resolve TAA by blending current frame with history, clamping to neighborhood.
    pub fn resolve(&self, current: &RenderTarget, history: &RenderTarget, motion: &RenderTarget) -> RenderTarget {
        let w = current.width;
        let h = current.height;
        let mut output = RenderTarget::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let cur = current.pixel(x, y);
                // Use motion vectors to reproject history
                let mv = motion.pixel(x, y);
                let hist_uv = Vec2::new(
                    x as f32 / w as f32 - mv.x,
                    y as f32 / h as f32 - mv.y,
                );
                let hist = if hist_uv.x < 0.0 || hist_uv.x > 1.0 || hist_uv.y < 0.0 || hist_uv.y > 1.0 {
                    cur
                } else {
                    history.sample(hist_uv)
                };
                let (min_c, max_c) = Self::neighborhood_aabb(current, x, y);
                let hist_clamped = Self::clip_to_aabb(hist, min_c, max_c);
                // Compute blend factor based on disocclusion
                let lum_diff = (cur.x + cur.y + cur.z - hist_clamped.x - hist_clamped.y - hist_clamped.z).abs();
                let blend = if lum_diff > self.ghosting_threshold {
                    // More weight on current frame when ghosting detected
                    self.history_blend + (1.0 - self.history_blend) * (lum_diff / (lum_diff + 1.0))
                } else {
                    self.history_blend
                };
                let result = hist_clamped + (cur - hist_clamped) * blend;
                output.set_pixel(x, y, result);
            }
        }
        output
    }
}

impl PostProcessEffect for TemporalAA {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, _params: &PostParams) {
        if !self.enabled || !output.is_same_size(input) {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        // Without history available here, just copy (history-aware version uses resolve())
        for (i, p) in output.pixels.iter_mut().enumerate() {
            *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
        }
    }

    fn name(&self) -> &str { "TemporalAA" }
    fn is_enabled(&self) -> bool { self.enabled }
}

impl Default for TemporalAA {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// ScreenSpaceAmbientOcclusion
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ScreenSpaceAmbientOcclusion {
    pub radius: f32,
    pub bias: f32,
    pub num_samples: u32,
    pub kernel: Vec<Vec3>,
    pub noise_tex: Vec<Vec3>,
    pub noise_size: u32,
    pub intensity: f32,
    pub enabled: bool,
}

impl ScreenSpaceAmbientOcclusion {
    fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
        a + t * (b - a)
    }

    pub fn new(num_samples: u32) -> Self {
        let kernel = Self::generate_kernel(num_samples);
        let noise_tex = Self::generate_noise(4);
        Self {
            radius: 0.5,
            bias: 0.025,
            num_samples,
            kernel,
            noise_tex,
            noise_size: 4,
            intensity: 1.0,
            enabled: true,
        }
    }

    /// Generate cosine-weighted hemisphere sample kernel.
    fn generate_kernel(count: u32) -> Vec<Vec3> {
        let mut kernel = Vec::with_capacity(count as usize);
        for i in 0..count {
            let t = i as f32 / count as f32;
            // Spherical coordinates
            let phi = halton(i, 2) * 2.0 * std::f32::consts::PI;
            let cos_theta = halton(i, 3).sqrt();
            let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
            let x = sin_theta * phi.cos();
            let y = sin_theta * phi.sin();
            let z = cos_theta;
            // Scale to push samples toward origin (importance sampling)
            let scale = Self::lerp_f32(0.1, 1.0, t * t);
            kernel.push(Vec3::new(x * scale, y * scale, z * scale));
        }
        kernel
    }

    /// Generate 4×4 noise texture for kernel rotation.
    fn generate_noise(size: u32) -> Vec<Vec3> {
        let count = (size * size) as usize;
        let mut noise = Vec::with_capacity(count);
        for i in 0..count {
            let angle = (i as f32 / count as f32) * 2.0 * std::f32::consts::PI;
            noise.push(Vec3::new(angle.cos(), angle.sin(), 0.0));
        }
        noise
    }

    /// Get noise rotation vector at screen position.
    fn noise_at(&self, x: u32, y: u32) -> Vec3 {
        let nx = (x % self.noise_size) as usize;
        let ny = (y % self.noise_size) as usize;
        let idx = ny * self.noise_size as usize + nx;
        self.noise_tex.get(idx).copied().unwrap_or(Vec3::X)
    }

    /// Build a TBN matrix from normal and a tangent hint.
    fn build_tbn(normal: Vec3, tangent: Vec3) -> Mat4 {
        let t = (tangent - normal * normal.dot(tangent)).normalize();
        let b = normal.cross(t);
        Mat4::from_cols(
            Vec4::new(t.x, t.y, t.z, 0.0),
            Vec4::new(b.x, b.y, b.z, 0.0),
            Vec4::new(normal.x, normal.y, normal.z, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        )
    }

    /// Compute per-sample occlusion. Depth and samples in view space.
    pub fn compute_occlusion(&self, depth: f32, normal: Vec3, samples: &[Vec3]) -> f32 {
        let view_pos = Vec3::new(0.0, 0.0, depth);
        let mut occlusion = 0.0f32;
        for &sample in samples {
            let sample_pos = view_pos + sample * self.radius;
            // Simplified: check if sample is above surface
            let sample_depth = sample_pos.z;
            let range_check = (1.0 - ((depth - sample_depth).abs() / self.radius).clamp(0.0, 1.0)).powi(2);
            let occ = if sample_depth >= depth + self.bias { 1.0f32 } else { 0.0f32 };
            occlusion += occ * range_check;
        }
        occlusion / samples.len().max(1) as f32
    }

    /// Full SSAO pass using depth and normal buffers.
    pub fn compute(&self, depth_buf: &[f32], normal_buf: &[Vec3], width: u32, height: u32) -> Vec<f32> {
        let count = (width * height) as usize;
        let mut ao_buf = vec![1.0f32; count];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let depth = depth_buf.get(idx).copied().unwrap_or(1.0);
                if depth >= 0.9999 {
                    ao_buf[idx] = 1.0;
                    continue;
                }
                let normal = normal_buf.get(idx).copied().unwrap_or(Vec3::Z);
                let noise = self.noise_at(x, y);
                let tangent = if noise.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
                let tbn = Self::build_tbn(normal, tangent + noise * 0.1);
                // Transform kernel to view space
                let mut view_samples: Vec<Vec3> = Vec::with_capacity(self.num_samples as usize);
                for k in &self.kernel {
                    let vs = tbn.transform_vector3(*k);
                    view_samples.push(vs);
                }
                let occ = self.compute_occlusion(depth, normal, &view_samples);
                ao_buf[idx] = (1.0 - occ * self.intensity).clamp(0.0, 1.0);
            }
        }
        ao_buf
    }
}

impl PostProcessEffect for ScreenSpaceAmbientOcclusion {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, _params: &PostParams) {
        if !self.enabled {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        let w = input.width;
        let h = input.height;
        // Extract depth from alpha channel, normal from RGB (simplified)
        let depth_buf: Vec<f32> = input.pixels.iter().map(|p| p.w).collect();
        let normal_buf: Vec<Vec3> = input.pixels.iter().map(|p| Vec3::new(p.x, p.y, p.z).normalize()).collect();
        let ao_buf = self.compute(&depth_buf, &normal_buf, w, h);
        for (i, p) in output.pixels.iter_mut().enumerate() {
            let src = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            let ao = ao_buf.get(i).copied().unwrap_or(1.0);
            *p = Vec4::new(src.x * ao, src.y * ao, src.z * ao, src.w);
        }
    }

    fn name(&self) -> &str { "SSAO" }
    fn is_enabled(&self) -> bool { self.enabled }
}

// ---------------------------------------------------------------------------
// DepthOfField
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DepthOfField {
    pub focal_distance: f32,
    pub focal_range: f32,
    pub bokeh_radius: f32,
    pub aperture: f32,
    pub focal_length: f32,
    pub max_coc: f32,
    pub near_blur: bool,
    pub enabled: bool,
    pub quality: u32,
}

impl DepthOfField {
    pub fn new(focal_distance: f32, aperture: f32) -> Self {
        Self {
            focal_distance,
            focal_range: 2.0,
            bokeh_radius: 15.0,
            aperture,
            focal_length: 50.0,
            max_coc: 20.0,
            near_blur: true,
            enabled: true,
            quality: 16,
        }
    }

    /// Compute circle of confusion radius in pixels.
    pub fn coc_radius(&self, depth: f32, focus: f32, aperture: f32, focal_len: f32) -> f32 {
        if depth < 1e-5 {
            return 0.0;
        }
        let coc = aperture * focal_len * (depth - focus).abs() / (depth * (focus - focal_len).abs().max(1e-5));
        (coc * self.bokeh_radius).min(self.max_coc)
    }

    /// Accumulate bokeh by gathering nearby pixels weighted by CoC.
    pub fn gather_bokeh(&self, center_x: u32, center_y: u32, radius: f32, rt: &RenderTarget, depth_buf: &[f32]) -> Vec4 {
        let r = radius.ceil() as i32;
        let mut color_sum = Vec4::ZERO;
        let mut weight_sum = 0.0f32;
        let w = rt.width as i32;
        let h = rt.height as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let dist2 = (dx * dx + dy * dy) as f32;
                if dist2 > radius * radius {
                    continue;
                }
                let sx = (center_x as i32 + dx).clamp(0, w - 1) as u32;
                let sy = (center_y as i32 + dy).clamp(0, h - 1) as u32;
                let idx = (sy * rt.width + sx) as usize;
                let sample_depth = depth_buf.get(idx).copied().unwrap_or(1.0);
                let sample_coc = self.coc_radius(sample_depth, self.focal_distance, self.aperture, self.focal_length);
                // Only accept sample if its CoC is large enough to reach center
                let sample_r = dist2.sqrt();
                if sample_r <= sample_coc.max(radius) {
                    let w = 1.0 - dist2.sqrt() / (radius + 1.0);
                    color_sum += rt.pixel(sx, sy) * w;
                    weight_sum += w;
                }
            }
        }
        if weight_sum > 0.0 { color_sum / weight_sum } else { rt.pixel(center_x, center_y) }
    }
}

impl PostProcessEffect for DepthOfField {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, _params: &PostParams) {
        if !self.enabled {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        let w = input.width;
        let h = input.height;
        // Use alpha as depth
        let depth_buf: Vec<f32> = input.pixels.iter().map(|p| p.w).collect();
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                let depth = depth_buf.get(idx).copied().unwrap_or(1.0);
                let coc = self.coc_radius(depth, self.focal_distance, self.aperture, self.focal_length);
                let result = if coc < 0.5 {
                    input.pixel(x, y)
                } else {
                    self.gather_bokeh(x, y, coc.min(self.max_coc), input, &depth_buf)
                };
                output.set_pixel(x, y, result);
            }
        }
    }

    fn name(&self) -> &str { "DepthOfField" }
    fn is_enabled(&self) -> bool { self.enabled }
}

// ---------------------------------------------------------------------------
// MotionBlur
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MotionBlur {
    pub shutter_speed: f32,
    pub max_samples: u32,
    pub tile_size: u32,
    pub enabled: bool,
}

impl MotionBlur {
    pub fn new() -> Self {
        Self {
            shutter_speed: 0.5,
            max_samples: 16,
            tile_size: 32,
            enabled: true,
        }
    }

    /// Build tile-max velocity buffer.
    fn build_tile_velocity(&self, motion: &RenderTarget) -> Vec<Vec2> {
        let tw = (motion.width + self.tile_size - 1) / self.tile_size;
        let th = (motion.height + self.tile_size - 1) / self.tile_size;
        let mut tile_vel = vec![Vec2::ZERO; (tw * th) as usize];
        for ty in 0..th {
            for tx in 0..tw {
                let x0 = tx * self.tile_size;
                let y0 = ty * self.tile_size;
                let x1 = (x0 + self.tile_size).min(motion.width);
                let y1 = (y0 + self.tile_size).min(motion.height);
                let mut max_vel = Vec2::ZERO;
                let mut max_len2 = 0.0f32;
                for py in y0..y1 {
                    for px in x0..x1 {
                        let mv = motion.pixel(px, py);
                        let vel = Vec2::new(mv.x, mv.y);
                        let len2 = vel.x * vel.x + vel.y * vel.y;
                        if len2 > max_len2 {
                            max_len2 = len2;
                            max_vel = vel;
                        }
                    }
                }
                tile_vel[(ty * tw + tx) as usize] = max_vel;
            }
        }
        tile_vel
    }

    fn get_tile_vel(&self, tile_buf: &[Vec2], tw: u32, x: u32, y: u32) -> Vec2 {
        let tx = x / self.tile_size;
        let ty = y / self.tile_size;
        tile_buf.get((ty * tw + tx) as usize).copied().unwrap_or(Vec2::ZERO)
    }

    pub fn apply_blur(&self, input: &RenderTarget, motion: &RenderTarget) -> RenderTarget {
        let w = input.width;
        let h = input.height;
        let mut output = RenderTarget::new(w, h);
        let tile_vel = self.build_tile_velocity(motion);
        let tw = (w + self.tile_size - 1) / self.tile_size;
        for y in 0..h {
            for x in 0..w {
                let tile_v = self.get_tile_vel(&tile_vel, tw, x, y);
                let tile_len = (tile_v.x * tile_v.x + tile_v.y * tile_v.y).sqrt();
                if tile_len < 0.5 / w as f32 {
                    output.set_pixel(x, y, input.pixel(x, y));
                    continue;
                }
                let mv = motion.pixel(x, y);
                let vel = Vec2::new(mv.x, mv.y) * self.shutter_speed;
                let n = self.max_samples;
                let mut color_sum = Vec4::ZERO;
                for s in 0..n {
                    let t = (s as f32 / (n - 1).max(1) as f32) - 0.5;
                    let su = (x as f32 / w as f32 + vel.x * t).clamp(0.0, 1.0);
                    let sv = (y as f32 / h as f32 + vel.y * t).clamp(0.0, 1.0);
                    color_sum += input.sample(Vec2::new(su, sv));
                }
                output.set_pixel(x, y, color_sum / n as f32);
            }
        }
        output
    }
}

impl PostProcessEffect for MotionBlur {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, _params: &PostParams) {
        if !self.enabled {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        // Without motion buffer in params, apply a simple per-pixel copy.
        // Caller should use apply_blur() directly when motion buffer is available.
        for (i, p) in output.pixels.iter_mut().enumerate() {
            *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
        }
    }

    fn name(&self) -> &str { "MotionBlur" }
    fn is_enabled(&self) -> bool { self.enabled }
}

impl Default for MotionBlur {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// ChromaticAberration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ChromaticAberration {
    pub strength: f32,
    pub radial_falloff: f32,
    pub enabled: bool,
}

impl ChromaticAberration {
    pub fn new(strength: f32) -> Self {
        Self { strength, radial_falloff: 2.0, enabled: true }
    }

    /// Compute channel-specific UV offset at a screen position.
    fn channel_offset(&self, uv: Vec2, channel_scale: f32) -> Vec2 {
        let center = Vec2::new(0.5, 0.5);
        let dir = uv - center;
        let dist = (dir.x * dir.x + dir.y * dir.y).sqrt();
        let falloff = dist.powf(self.radial_falloff);
        dir * self.strength * channel_scale * falloff
    }
}

impl PostProcessEffect for ChromaticAberration {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, _params: &PostParams) {
        if !self.enabled {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        let w = input.width;
        let h = input.height;
        for y in 0..h {
            for x in 0..w {
                let uv = Vec2::new(x as f32 / w as f32, y as f32 / h as f32);
                let r_off = self.channel_offset(uv, -1.0);
                let g_off = self.channel_offset(uv, 0.0);
                let b_off = self.channel_offset(uv, 1.0);
                let r_uv = Vec2::new((uv.x + r_off.x).clamp(0.0, 1.0), (uv.y + r_off.y).clamp(0.0, 1.0));
                let g_uv = Vec2::new((uv.x + g_off.x).clamp(0.0, 1.0), (uv.y + g_off.y).clamp(0.0, 1.0));
                let b_uv = Vec2::new((uv.x + b_off.x).clamp(0.0, 1.0), (uv.y + b_off.y).clamp(0.0, 1.0));
                let r = input.sample(r_uv).x;
                let g = input.sample(g_uv).y;
                let b = input.sample(b_uv).z;
                let a = input.pixel(x, y).w;
                output.set_pixel(x, y, Vec4::new(r, g, b, a));
            }
        }
    }

    fn name(&self) -> &str { "ChromaticAberration" }
    fn is_enabled(&self) -> bool { self.enabled }
}

// ---------------------------------------------------------------------------
// LensFlare
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FlareGhost {
    pub offset: f32,
    pub size: f32,
    pub color: Vec4,
    pub shape: FlareShape,
}

#[derive(Debug, Clone)]
pub enum FlareShape {
    Circle,
    Hexagon,
    Heptagon,
    Streak { angle: f32, length: f32 },
}

impl FlareGhost {
    pub fn new(offset: f32, size: f32, color: Vec4) -> Self {
        Self { offset, size, color, shape: FlareShape::Circle }
    }
}

#[derive(Debug, Clone)]
pub struct LensFlare {
    pub position: Vec3,
    pub color: Vec4,
    pub ghosts: Vec<FlareGhost>,
    pub enabled: bool,
    pub threshold: f32,
    pub intensity: f32,
    pub star_burst_strength: f32,
}

impl LensFlare {
    pub fn new(position: Vec3, color: Vec4) -> Self {
        let ghosts = vec![
            FlareGhost::new(0.4, 0.1, Vec4::new(1.0, 0.8, 0.6, 0.3)),
            FlareGhost::new(0.7, 0.06, Vec4::new(0.8, 0.6, 1.0, 0.2)),
            FlareGhost::new(-0.3, 0.15, Vec4::new(0.6, 1.0, 0.8, 0.15)),
            FlareGhost::new(1.2, 0.04, Vec4::new(1.0, 1.0, 0.6, 0.25)),
            FlareGhost::new(-0.8, 0.08, Vec4::new(0.6, 0.8, 1.0, 0.2)),
        ];
        Self {
            position,
            color,
            ghosts,
            enabled: true,
            threshold: 1.0,
            intensity: 1.0,
            star_burst_strength: 0.5,
        }
    }

    /// Project 3D position to screen UV (assumes simple perspective).
    pub fn screen_uv(&self, view_proj: Mat4) -> Option<Vec2> {
        let clip = view_proj.project_point3(self.position);
        if clip.z < 0.0 || clip.z > 1.0 {
            return None;
        }
        Some(Vec2::new(clip.x * 0.5 + 0.5, clip.y * 0.5 + 0.5))
    }

    /// Compute flare contribution at a screen UV position.
    pub fn compute_flare(&self, screen_uv: Vec2, light_screen_uv: Vec2, screen_center: Vec2) -> Vec4 {
        if !self.enabled {
            return Vec4::ZERO;
        }
        let mut result = Vec4::ZERO;
        // Ghost positions are along the line from light to screen center
        let axis = screen_center - light_screen_uv;
        for ghost in &self.ghosts {
            let ghost_pos = light_screen_uv + axis * ghost.offset;
            let dist = (screen_uv - ghost_pos).length();
            if dist < ghost.size {
                let falloff = match &ghost.shape {
                    FlareShape::Circle => {
                        let t = dist / ghost.size;
                        (1.0 - t * t).powi(2)
                    }
                    FlareShape::Hexagon => {
                        let d = screen_uv - ghost_pos;
                        // Simplified hexagon approximation
                        let hex_dist = d.x.abs().max(d.y.abs() * 1.1547 + d.x.abs() * 0.5).max(d.y.abs() * 1.1547 - d.x.abs() * 0.5);
                        (1.0 - hex_dist / ghost.size).clamp(0.0, 1.0)
                    }
                    FlareShape::Heptagon => {
                        let t = dist / ghost.size;
                        1.0 - t
                    }
                    FlareShape::Streak { angle, length } => {
                        let d = screen_uv - ghost_pos;
                        let rotated_x = d.x * angle.cos() + d.y * angle.sin();
                        let rotated_y = -d.x * angle.sin() + d.y * angle.cos();
                        let streak_dist = rotated_x.abs() / length + rotated_y.abs() / ghost.size;
                        (1.0 - streak_dist).clamp(0.0, 1.0)
                    }
                };
                let gc = ghost.color * falloff * self.intensity;
                result += gc;
            }
        }
        // Starburst around light position
        let to_light = screen_uv - light_screen_uv;
        let light_dist = (to_light.x * to_light.x + to_light.y * to_light.y).sqrt();
        if light_dist < 0.1 {
            let spokes = 8.0f32;
            let angle = to_light.y.atan2(to_light.x);
            let spoke_factor = (angle * spokes).cos().abs().powf(20.0);
            let radial_falloff = (1.0 - light_dist / 0.1).powi(2);
            let streak_contrib = Vec4::new(
                self.color.x * spoke_factor * radial_falloff * self.star_burst_strength,
                self.color.y * spoke_factor * radial_falloff * self.star_burst_strength,
                self.color.z * spoke_factor * radial_falloff * self.star_burst_strength,
                0.0,
            );
            result += streak_contrib;
        }
        result
    }
}

impl PostProcessEffect for LensFlare {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, _params: &PostParams) {
        for (i, p) in output.pixels.iter_mut().enumerate() {
            *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
        }
        if !self.enabled {
            return;
        }
        // Without a projection matrix here, skip — callers should use compute_flare() directly.
    }

    fn name(&self) -> &str { "LensFlare" }
    fn is_enabled(&self) -> bool { self.enabled }
}

// ---------------------------------------------------------------------------
// FilmGrain
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FilmGrain {
    pub strength: f32,
    pub size: f32,
    pub luminance_only: bool,
    pub enabled: bool,
}

impl FilmGrain {
    pub fn new(strength: f32) -> Self {
        Self { strength, size: 1.0, luminance_only: false, enabled: true }
    }

    fn hash(n: f32) -> f32 {
        let x = n.sin() * 43758.5453;
        x - x.floor()
    }

    fn grain_at(&self, x: u32, y: u32, time: f32) -> f32 {
        let n = x as f32 * 1.7 + y as f32 * 31.3 + time * 7919.0;
        Self::hash(n) * 2.0 - 1.0
    }
}

impl PostProcessEffect for FilmGrain {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, params: &PostParams) {
        if !self.enabled {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        let w = input.width;
        let h = input.height;
        for y in 0..h {
            for x in 0..w {
                let src = input.pixel(x, y);
                let grain = self.grain_at(x / self.size.max(1.0) as u32, y / self.size.max(1.0) as u32, params.time) * self.strength;
                let result = if self.luminance_only {
                    let lum = src.x * 0.2126 + src.y * 0.7152 + src.z * 0.0722;
                    let factor = if lum > 0.0 { 1.0 + grain / (lum.sqrt() + 0.001) } else { 1.0 };
                    Vec4::new(src.x * factor, src.y * factor, src.z * factor, src.w)
                } else {
                    Vec4::new(
                        (src.x + grain).clamp(0.0, 1.0),
                        (src.y + grain).clamp(0.0, 1.0),
                        (src.z + grain).clamp(0.0, 1.0),
                        src.w,
                    )
                };
                output.set_pixel(x, y, result);
            }
        }
    }

    fn name(&self) -> &str { "FilmGrain" }
    fn is_enabled(&self) -> bool { self.enabled }
}

// ---------------------------------------------------------------------------
// Vignette
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Vignette {
    pub intensity: f32,
    pub smoothness: f32,
    pub inner_radius: f32,
    pub color: Vec3,
    pub enabled: bool,
}

impl Vignette {
    pub fn new(intensity: f32) -> Self {
        Self {
            intensity,
            smoothness: 0.4,
            inner_radius: 0.5,
            color: Vec3::ZERO,
            enabled: true,
        }
    }

    pub fn compute_factor(&self, uv: Vec2) -> f32 {
        let center = Vec2::new(0.5, 0.5);
        let dist = (uv - center).length();
        let outer = self.inner_radius + self.smoothness;
        let t = ((dist - self.inner_radius) / (outer - self.inner_radius + 1e-5)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t) * self.intensity
    }
}

impl PostProcessEffect for Vignette {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, _params: &PostParams) {
        if !self.enabled {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        let w = input.width;
        let h = input.height;
        for y in 0..h {
            for x in 0..w {
                let src = input.pixel(x, y);
                let uv = Vec2::new(x as f32 / w as f32, y as f32 / h as f32);
                let vignette = self.compute_factor(uv);
                let vig_r = self.color.x + (src.x - self.color.x) * (1.0 - vignette);
                let vig_g = self.color.y + (src.y - self.color.y) * (1.0 - vignette);
                let vig_b = self.color.z + (src.z - self.color.z) * (1.0 - vignette);
                output.set_pixel(x, y, Vec4::new(vig_r, vig_g, vig_b, src.w));
            }
        }
    }

    fn name(&self) -> &str { "Vignette" }
    fn is_enabled(&self) -> bool { self.enabled }
}

// ---------------------------------------------------------------------------
// ToneMapping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ToneMapping {
    pub mode: ToneMappingMode,
    pub exposure: f32,
    pub gamma: f32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToneMappingMode {
    Reinhard,
    ReinhardExtended { max_white: f32 },
    ACES,
    Uncharted2,
    Lottes,
    Linear,
}

impl ToneMapping {
    pub fn new(mode: ToneMappingMode) -> Self {
        Self { mode, exposure: 1.0, gamma: 2.2, enabled: true }
    }

    fn reinhard(x: f32) -> f32 { x / (1.0 + x) }

    fn reinhard_extended(x: f32, max_white: f32) -> f32 {
        let numerator = x * (1.0 + x / (max_white * max_white));
        numerator / (1.0 + x)
    }

    fn aces_film(x: f32) -> f32 {
        let a = 2.51f32;
        let b = 0.03f32;
        let c = 2.43f32;
        let d = 0.59f32;
        let e = 0.14f32;
        ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
    }

    fn uncharted2_partial(x: f32) -> f32 {
        let a = 0.15f32;
        let b = 0.50f32;
        let c = 0.10f32;
        let d = 0.20f32;
        let e = 0.02f32;
        let f = 0.30f32;
        ((x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f)) - e / f
    }

    fn uncharted2(x: f32) -> f32 {
        let exposure_bias = 2.0f32;
        let curr = Self::uncharted2_partial(x * exposure_bias);
        let w = 11.2f32;
        let white_scale = 1.0 / Self::uncharted2_partial(w);
        curr * white_scale
    }

    fn lottes(x: f32) -> f32 {
        let a = 1.6f32;
        let d = 0.977f32;
        let hdr_max = 8.0f32;
        let mid_in = 0.18f32;
        let mid_out = 0.267f32;
        let b = (-mid_in.powf(a) + hdr_max.powf(a) * mid_out)
            / ((hdr_max.powf(a * d) - mid_in.powf(a * d)) * mid_out);
        let c_val = (hdr_max.powf(a * d) * mid_in.powf(a) - hdr_max.powf(a) * mid_in.powf(a * d) * mid_out)
            / ((hdr_max.powf(a * d) - mid_in.powf(a * d)) * mid_out);
        x.powf(a) / (x.powf(a * d) * b + c_val)
    }

    pub fn map_color(&self, color: Vec3) -> Vec3 {
        let exposed = color * self.exposure;
        let mapped = match self.mode {
            ToneMappingMode::Linear => exposed,
            ToneMappingMode::Reinhard => Vec3::new(
                Self::reinhard(exposed.x),
                Self::reinhard(exposed.y),
                Self::reinhard(exposed.z),
            ),
            ToneMappingMode::ReinhardExtended { max_white } => Vec3::new(
                Self::reinhard_extended(exposed.x, max_white),
                Self::reinhard_extended(exposed.y, max_white),
                Self::reinhard_extended(exposed.z, max_white),
            ),
            ToneMappingMode::ACES => Vec3::new(
                Self::aces_film(exposed.x),
                Self::aces_film(exposed.y),
                Self::aces_film(exposed.z),
            ),
            ToneMappingMode::Uncharted2 => Vec3::new(
                Self::uncharted2(exposed.x),
                Self::uncharted2(exposed.y),
                Self::uncharted2(exposed.z),
            ),
            ToneMappingMode::Lottes => Vec3::new(
                Self::lottes(exposed.x),
                Self::lottes(exposed.y),
                Self::lottes(exposed.z),
            ),
        };
        // Gamma correction
        let inv_gamma = 1.0 / self.gamma;
        Vec3::new(
            mapped.x.max(0.0).powf(inv_gamma),
            mapped.y.max(0.0).powf(inv_gamma),
            mapped.z.max(0.0).powf(inv_gamma),
        )
    }
}

impl PostProcessEffect for ToneMapping {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, params: &PostParams) {
        if !self.enabled {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        let mut tone = self.clone();
        tone.exposure = params.exposure;
        tone.gamma = params.gamma;
        for (i, out_p) in output.pixels.iter_mut().enumerate() {
            let src = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            let mapped = tone.map_color(Vec3::new(src.x, src.y, src.z));
            *out_p = Vec4::new(mapped.x, mapped.y, mapped.z, src.w);
        }
    }

    fn name(&self) -> &str { "ToneMapping" }
    fn is_enabled(&self) -> bool { self.enabled }
}

// ---------------------------------------------------------------------------
// Bloom
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Bloom {
    pub threshold: f32,
    pub intensity: f32,
    pub scatter: f32,
    pub num_passes: u32,
    pub dirt_mask_strength: f32,
    pub enabled: bool,
}

impl Bloom {
    pub fn new() -> Self {
        Self {
            threshold: 1.0,
            intensity: 0.5,
            scatter: 0.7,
            num_passes: 5,
            dirt_mask_strength: 0.0,
            enabled: true,
        }
    }

    fn luminance(c: Vec4) -> f32 {
        c.x * 0.2126 + c.y * 0.7152 + c.z * 0.0722
    }

    fn extract_bright(input: &RenderTarget, threshold: f32) -> RenderTarget {
        let mut bright = RenderTarget::new(input.width, input.height);
        for (i, &p) in input.pixels.iter().enumerate() {
            let lum = Self::luminance(p);
            let knee = threshold * 0.5;
            let factor = if lum > threshold {
                1.0
            } else if lum > threshold - knee {
                (lum - (threshold - knee)) / knee
            } else {
                0.0
            };
            bright.pixels[i] = p * factor;
        }
        bright
    }

    fn downsample(input: &RenderTarget) -> RenderTarget {
        let w = (input.width / 2).max(1);
        let h = (input.height / 2).max(1);
        let mut out = RenderTarget::new(w, h);
        for y in 0..h {
            for x in 0..w {
                // 4-tap box filter
                let c = input.pixel(x * 2, y * 2)
                    + input.pixel(x * 2 + 1, y * 2)
                    + input.pixel(x * 2, y * 2 + 1)
                    + input.pixel(x * 2 + 1, y * 2 + 1);
                out.set_pixel(x, y, c * 0.25);
            }
        }
        out
    }

    fn upsample_blur(input: &RenderTarget, target_w: u32, target_h: u32, scatter: f32) -> RenderTarget {
        let mut out = RenderTarget::new(target_w, target_h);
        for y in 0..target_h {
            for x in 0..target_w {
                let uv = Vec2::new(x as f32 / target_w as f32, y as f32 / target_h as f32);
                let texel = Vec2::new(1.0 / input.width as f32, 1.0 / input.height as f32);
                // 3x3 tent filter
                let mut sum = Vec4::ZERO;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let w = if dx == 0 && dy == 0 { 4.0 } else if dx == 0 || dy == 0 { 2.0 } else { 1.0 };
                        let su = Vec2::new(
                            uv.x + dx as f32 * texel.x,
                            uv.y + dy as f32 * texel.y,
                        );
                        sum += input.sample(su) * w;
                    }
                }
                out.set_pixel(x, y, sum * (scatter / 16.0));
            }
        }
        out
    }

    pub fn apply_bloom(&self, input: &RenderTarget) -> RenderTarget {
        if !self.enabled {
            return input.clone();
        }
        // Extract bright regions
        let bright = Self::extract_bright(input, self.threshold);
        // Build mip chain
        let mut mips = vec![bright];
        for _ in 1..self.num_passes {
            let last = mips.last().unwrap();
            if last.width <= 1 || last.height <= 1 {
                break;
            }
            mips.push(Self::downsample(last));
        }
        // Accumulate upsampled blur passes
        let mut accum = RenderTarget::new(input.width, input.height);
        for mip in mips.iter().rev() {
            let up = Self::upsample_blur(mip, input.width, input.height, self.scatter);
            for (i, p) in accum.pixels.iter_mut().enumerate() {
                *p += up.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
        }
        // Composite bloom onto input
        let mut output = input.clone();
        for (i, p) in output.pixels.iter_mut().enumerate() {
            let bloom = accum.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            *p += bloom * self.intensity;
        }
        output
    }
}

impl PostProcessEffect for Bloom {
    fn apply(&self, input: &RenderTarget, output: &mut RenderTarget, _params: &PostParams) {
        if !self.enabled {
            for (i, p) in output.pixels.iter_mut().enumerate() {
                *p = input.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        let result = self.apply_bloom(input);
        for (i, p) in output.pixels.iter_mut().enumerate() {
            *p = result.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
        }
    }

    fn name(&self) -> &str { "Bloom" }
    fn is_enabled(&self) -> bool { self.enabled }
}

impl Default for Bloom {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// PostProcessStack — ordered chain of effects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EffectEntry {
    pub name: String,
    pub enabled: bool,
    pub blend_weight: f32,
    pub order: u32,
}

pub struct PostProcessStack {
    pub effects: Vec<Box<dyn PostProcessEffect>>,
    pub entries: Vec<EffectEntry>,
    pub enabled: bool,
}

impl PostProcessStack {
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
            entries: Vec::new(),
            enabled: true,
        }
    }

    pub fn add_effect(&mut self, effect: Box<dyn PostProcessEffect>, blend_weight: f32) {
        let name = effect.name().to_string();
        let enabled = effect.is_enabled();
        let order = self.effects.len() as u32;
        self.entries.push(EffectEntry { name, enabled, blend_weight, order });
        self.effects.push(effect);
    }

    pub fn set_effect_enabled(&mut self, name: &str, enabled: bool) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.name == name) {
            entry.enabled = enabled;
        }
    }

    pub fn set_blend_weight(&mut self, name: &str, weight: f32) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.name == name) {
            entry.blend_weight = weight.clamp(0.0, 1.0);
        }
    }

    /// Execute the full post-process chain.
    pub fn execute(&self, input: &RenderTarget, params: &PostParams) -> RenderTarget {
        if !self.enabled || self.effects.is_empty() {
            return input.clone();
        }
        let w = input.width;
        let h = input.height;
        let mut ping = input.clone();
        let mut pong = RenderTarget::new(w, h);

        for (effect, entry) in self.effects.iter().zip(self.entries.iter()) {
            if !entry.enabled {
                continue;
            }
            effect.apply(&ping, &mut pong, params);
            if (entry.blend_weight - 1.0).abs() > 0.001 {
                // Blend between original and effect output
                let bw = entry.blend_weight;
                for i in 0..pong.pixels.len() {
                    let orig = ping.pixels.get(i).copied().unwrap_or(Vec4::ZERO);
                    let eff = pong.pixels[i];
                    pong.pixels[i] = orig + (eff - orig) * bw;
                }
            }
            std::mem::swap(&mut ping, &mut pong);
        }
        ping
    }

    /// Build a standard game post-process stack.
    pub fn build_standard(width: u32, height: u32) -> Self {
        let mut stack = Self::new();
        stack.add_effect(Box::new(ScreenSpaceAmbientOcclusion::new(32)), 1.0);
        stack.add_effect(Box::new(TemporalAA::new()), 1.0);
        stack.add_effect(Box::new(Bloom::new()), 1.0);
        stack.add_effect(Box::new(DepthOfField::new(10.0, 2.8)), 0.0);
        stack.add_effect(Box::new(MotionBlur::new()), 0.0);
        stack.add_effect(Box::new(ToneMapping::new(ToneMappingMode::ACES)), 1.0);
        stack.add_effect(Box::new(ChromaticAberration::new(0.003)), 0.8);
        stack.add_effect(Box::new(Vignette::new(0.4)), 1.0);
        stack.add_effect(Box::new(FilmGrain::new(0.03)), 1.0);
        // Disable expensive effects by default
        stack.set_effect_enabled("DepthOfField", false);
        stack.set_effect_enabled("MotionBlur", false);
        let _ = (width, height);
        stack
    }

    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }

    pub fn enabled_count(&self) -> usize {
        self.entries.iter().filter(|e| e.enabled).count()
    }
}

impl Default for PostProcessStack {
    fn default() -> Self {
        Self::new()
    }
}
