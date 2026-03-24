//! Velocity-based motion blur pass.
//!
//! High-velocity glyphs are sampled multiple times along their screen-space velocity
//! vector with decreasing opacity, creating realistic motion streaks. The blur is:
//!   1. Proportional to screen-space velocity magnitude
//!   2. Directional (samples along the velocity vector)
//!   3. Weighted by a falloff curve (more opacity near the actual position)
//!   4. Optional: camera motion blur (blur entire frame by camera delta)
//!
//! The velocity buffer is a CPU-side 2-channel texture (screen-space velocity per pixel).

use glam::Vec2;

/// Motion blur pass parameters.
#[derive(Clone, Debug)]
pub struct MotionBlurParams {
    pub enabled: bool,
    /// Maximum number of samples along the velocity vector per pixel. Higher = smoother.
    pub samples: u8,
    /// How strongly velocity translates to blur offset (in UV units per m/s equivalent).
    pub scale: f32,
    /// Clamp maximum blur length in UV units (prevents extreme blurring).
    pub max_length: f32,
    /// Sample weight falloff: 0.0 = uniform, 1.0 = exponential decay toward tail.
    pub falloff: f32,
    /// Whether to include camera motion blur (blur entire frame by camera velocity).
    pub camera_blur: bool,
    /// Camera blur strength multiplier (independent of per-glyph blur).
    pub camera_blur_scale: f32,
    /// Blur quality: sharper = fewer artifacts but less smoothness.
    pub quality: BlurQuality,
    /// Temporal accumulation factor (0.0 = per-frame, 0.9 = heavy ghosting).
    pub temporal: f32,
}

/// Quality setting for blur sampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlurQuality {
    /// Fast: 3-4 samples, visible stepping artifacts.
    Low,
    /// Medium: 6-8 samples, acceptable quality.
    Medium,
    /// High: 12-16 samples, smooth result.
    High,
    /// Ultra: 24-32 samples, production quality.
    Ultra,
}

impl BlurQuality {
    pub fn sample_count(&self) -> u8 {
        match self {
            BlurQuality::Low    => 4,
            BlurQuality::Medium => 8,
            BlurQuality::High   => 14,
            BlurQuality::Ultra  => 28,
        }
    }
}

impl Default for MotionBlurParams {
    fn default() -> Self {
        Self {
            enabled:           true,
            samples:           8,
            scale:             0.3,
            max_length:        0.05,
            falloff:           0.7,
            camera_blur:       false,
            camera_blur_scale: 0.5,
            quality:           BlurQuality::Medium,
            temporal:          0.0,
        }
    }
}

impl MotionBlurParams {
    /// Disabled motion blur.
    pub fn none() -> Self { Self { enabled: false, ..Default::default() } }

    /// Cinematic motion blur (smooth, strong, camera blur enabled).
    pub fn cinematic() -> Self {
        Self {
            enabled:           true,
            samples:           16,
            scale:             0.5,
            max_length:        0.08,
            falloff:           0.8,
            camera_blur:       true,
            camera_blur_scale: 0.8,
            quality:           BlurQuality::High,
            temporal:          0.1,
        }
    }

    /// Fast game blur (minimal overhead, good enough for action).
    pub fn game() -> Self {
        Self {
            enabled:           true,
            samples:           6,
            scale:             0.25,
            max_length:        0.04,
            falloff:           0.6,
            camera_blur:       false,
            camera_blur_scale: 0.0,
            quality:           BlurQuality::Low,
            temporal:          0.0,
        }
    }

    /// Chaos Rift hyper-blur (extreme blur for dimensional distortion).
    pub fn chaos_warp(intensity: f32) -> Self {
        let i = intensity.clamp(0.0, 1.0);
        Self {
            enabled:           i > 0.05,
            samples:           (4.0 + i * 20.0) as u8,
            scale:             i * 1.5,
            max_length:        i * 0.2,
            falloff:           0.3,
            camera_blur:       true,
            camera_blur_scale: i * 2.0,
            quality:           BlurQuality::Medium,
            temporal:          i * 0.5,
        }
    }

    /// Lerp between two motion blur configs.
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            enabled:           if t < 0.5 { a.enabled } else { b.enabled },
            samples:           if t < 0.5 { a.samples } else { b.samples },
            scale:             lerp_f32(a.scale,             b.scale,             t),
            max_length:        lerp_f32(a.max_length,        b.max_length,        t),
            falloff:           lerp_f32(a.falloff,           b.falloff,           t),
            camera_blur:       if t < 0.5 { a.camera_blur } else { b.camera_blur },
            camera_blur_scale: lerp_f32(a.camera_blur_scale, b.camera_blur_scale, t),
            quality:           if t < 0.5 { a.quality } else { b.quality },
            temporal:          lerp_f32(a.temporal,          b.temporal,          t),
        }
    }
}

// ── Velocity buffer ───────────────────────────────────────────────────────────

/// Per-pixel screen-space velocity (in UV units/frame).
///
/// This is populated each frame by the scene renderer before the blur pass.
/// Glyphs with high velocity write their screen-space motion vector here.
pub struct VelocityBuffer {
    pub width:  u32,
    pub height: u32,
    velocities: Vec<Vec2>,
}

impl VelocityBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width, height,
            velocities: vec![Vec2::ZERO; (width * height) as usize],
        }
    }

    pub fn clear(&mut self) {
        self.velocities.iter_mut().for_each(|v| *v = Vec2::ZERO);
    }

    pub fn set(&mut self, x: u32, y: u32, velocity: Vec2) {
        if x < self.width && y < self.height {
            self.velocities[(y * self.width + x) as usize] = velocity;
        }
    }

    pub fn get(&self, x: u32, y: u32) -> Vec2 {
        if x < self.width && y < self.height {
            self.velocities[(y * self.width + x) as usize]
        } else {
            Vec2::ZERO
        }
    }

    /// Add velocity to a pixel (accumulate from multiple sources).
    pub fn add(&mut self, x: u32, y: u32, velocity: Vec2) {
        if x < self.width && y < self.height {
            self.velocities[(y * self.width + x) as usize] += velocity;
        }
    }

    /// Write a glyph's screen-space velocity to the buffer.
    /// `pos_px` is pixel position, `vel_uv` is screen-space velocity in UV units/frame.
    /// Writes to a small neighborhood for fill coverage.
    pub fn splat(&mut self, pos_px: Vec2, vel_uv: Vec2, radius_px: f32) {
        let r = radius_px.ceil() as i32;
        let cx = pos_px.x as i32;
        let cy = pos_px.y as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0 && y >= 0 {
                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    if dist <= radius_px {
                        let weight = 1.0 - dist / (radius_px + 1.0);
                        self.add(x as u32, y as u32, vel_uv * weight);
                    }
                }
            }
        }
    }

    /// Clamp all velocities to max_length UV units per frame.
    pub fn clamp(&mut self, max_length: f32) {
        for v in &mut self.velocities {
            let len = v.length();
            if len > max_length {
                *v = *v / len * max_length;
            }
        }
    }

    /// Raw f32 slice for GPU upload (RG32F: 2 floats per pixel).
    pub fn as_f32_slice(&self) -> &[f32] {
        unsafe {
            std::slice::from_raw_parts(
                self.velocities.as_ptr() as *const f32,
                self.velocities.len() * 2,
            )
        }
    }

    pub fn pixel_count(&self) -> usize { self.velocities.len() }
}

// ── Sample weight curve ───────────────────────────────────────────────────────

/// Compute the opacity weight for the i-th sample in a blur of N samples.
///
/// `i` is 0 = closest to actual position, N-1 = farthest (the tail).
/// `falloff` in [0, 1] controls how quickly weight drops off.
pub fn sample_weight(i: usize, n: usize, falloff: f32) -> f32 {
    if n <= 1 { return 1.0; }
    let t = i as f32 / (n - 1) as f32;
    let linear = 1.0 - t;
    // Mix linear and exponential falloff
    let exp = (-t * 3.0 * falloff).exp();
    linear * (1.0 - falloff) + exp * falloff
}

/// Compute sample UV offsets along a velocity vector.
///
/// Returns `n` UV offsets, going from 0 (no offset) to `vel * scale` (full offset).
/// Clamped to `max_length`.
pub fn blur_sample_uvs(velocity_uv: Vec2, n: usize, scale: f32, max_length: f32) -> Vec<Vec2> {
    let vel = velocity_uv * scale;
    let vel_len = vel.length();
    let clamped_vel = if vel_len > max_length && vel_len > 0.0001 {
        vel / vel_len * max_length
    } else {
        vel
    };

    (0..n).map(|i| {
        let t = if n <= 1 { 0.0 } else { i as f32 / (n - 1) as f32 };
        clamped_vel * t
    }).collect()
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_buffer_set_get() {
        let mut buf = VelocityBuffer::new(64, 64);
        buf.set(10, 20, Vec2::new(0.01, -0.02));
        let v = buf.get(10, 20);
        assert!((v.x - 0.01).abs() < 1e-6);
        assert!((v.y + 0.02).abs() < 1e-6);
    }

    #[test]
    fn velocity_buffer_clear() {
        let mut buf = VelocityBuffer::new(8, 8);
        buf.set(3, 3, Vec2::new(1.0, 1.0));
        buf.clear();
        assert_eq!(buf.get(3, 3), Vec2::ZERO);
    }

    #[test]
    fn sample_weight_first_is_highest() {
        let w0 = sample_weight(0, 8, 0.7);
        let w7 = sample_weight(7, 8, 0.7);
        assert!(w0 > w7, "Closest sample should have highest weight");
    }

    #[test]
    fn blur_uvs_starts_at_zero() {
        let vel = Vec2::new(0.1, 0.0);
        let uvs = blur_sample_uvs(vel, 4, 1.0, 0.5);
        assert_eq!(uvs.len(), 4);
        assert!((uvs[0].x).abs() < 1e-6, "First sample should be at origin");
    }

    #[test]
    fn blur_uvs_clamped() {
        let vel = Vec2::new(10.0, 0.0);
        let uvs = blur_sample_uvs(vel, 4, 1.0, 0.05);
        for uv in &uvs {
            assert!(uv.length() <= 0.051, "UV should be clamped: {:?}", uv);
        }
    }

    #[test]
    fn velocity_buffer_f32_slice_length() {
        let buf = VelocityBuffer::new(16, 16);
        assert_eq!(buf.as_f32_slice().len(), 16 * 16 * 2);
    }

    #[test]
    fn splat_writes_neighborhood() {
        let mut buf = VelocityBuffer::new(64, 64);
        buf.splat(Vec2::new(32.0, 32.0), Vec2::new(0.01, 0.0), 3.0);
        // Center should have velocity
        let center = buf.get(32, 32);
        assert!(center.x > 0.0);
        // Far away should not
        let far = buf.get(0, 0);
        assert_eq!(far, Vec2::ZERO);
    }

    #[test]
    fn quality_sample_counts() {
        assert!(BlurQuality::Ultra.sample_count() > BlurQuality::High.sample_count());
        assert!(BlurQuality::High.sample_count()  > BlurQuality::Low.sample_count());
    }
}
