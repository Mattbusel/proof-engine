//! Screen-space distortion pass — gravitational lensing, heat shimmer, entropy warping.
//!
//! Force fields write distortion vectors into a CPU-side distortion map each frame.
//! The distortion pass samples the scene at UV + distortion_offset, creating:
//!   - Gravity wells: circular lensing toward field centers
//!   - Heat shimmer: Perlin-noise-driven wavy distortion
//!   - Chaos Rift: high-entropy chaotic UV scrambling
//!   - Vortex: rotating UV distortion following Vortex force fields

use glam::Vec2;

/// Distortion pass parameters.
#[derive(Clone, Debug)]
pub struct DistortionParams {
    pub enabled: bool,
    /// Global distortion scale multiplier (1.0 = normal, 0.0 = disabled).
    pub scale: f32,
    /// Chromatic splitting of distorted UV offsets (0.0 = mono, 1.0 = full RGB split).
    pub chromatic_split: f32,
    /// Maximum distortion offset in UV units (clamp for safety).
    pub max_offset: f32,
    /// Time scale for animated distortions (1.0 = realtime, 0.5 = slow motion).
    pub time_scale: f32,
    /// Whether to apply a subtle edge-fade to prevent border artifacts.
    pub edge_fade: bool,
    /// Edge fade width in UV units (0.05 = 5% of screen from each edge).
    pub edge_fade_width: f32,
}

impl Default for DistortionParams {
    fn default() -> Self {
        Self {
            enabled:         true,
            scale:           1.0,
            chromatic_split: 0.2,
            max_offset:      0.05,
            time_scale:      1.0,
            edge_fade:       true,
            edge_fade_width: 0.05,
        }
    }
}

impl DistortionParams {
    /// Disabled distortion.
    pub fn none() -> Self { Self { enabled: false, ..Default::default() } }

    /// Subtle heat shimmer (great for desert levels or forges).
    pub fn heat_shimmer() -> Self {
        Self { enabled: true, scale: 0.3, chromatic_split: 0.1,
               max_offset: 0.008, time_scale: 1.5, edge_fade: true,
               edge_fade_width: 0.1 }
    }

    /// Strong gravitational lensing (black hole or Gravity Nexus).
    pub fn gravity_lens() -> Self {
        Self { enabled: true, scale: 2.0, chromatic_split: 0.5,
               max_offset: 0.08, time_scale: 0.3, edge_fade: true,
               edge_fade_width: 0.05 }
    }

    /// Chaos Rift distortion (high entropy, chaotic, colorful).
    pub fn chaos_rift(entropy: f32) -> Self {
        let e = entropy.clamp(0.0, 1.0);
        Self { enabled: e > 0.01, scale: e * 3.0, chromatic_split: e * 0.8,
               max_offset: e * 0.12, time_scale: 1.0 + e * 4.0, edge_fade: false,
               edge_fade_width: 0.0 }
    }

    /// Lerp between two distortion configs.
    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            enabled:         if t < 0.5 { a.enabled } else { b.enabled },
            scale:           lerp_f32(a.scale,           b.scale,           t),
            chromatic_split: lerp_f32(a.chromatic_split, b.chromatic_split, t),
            max_offset:      lerp_f32(a.max_offset,      b.max_offset,      t),
            time_scale:      lerp_f32(a.time_scale,      b.time_scale,      t),
            edge_fade:       a.edge_fade,
            edge_fade_width: lerp_f32(a.edge_fade_width, b.edge_fade_width, t),
        }
    }
}

// ── Distortion map ────────────────────────────────────────────────────────────

/// A CPU-side distortion map — an array of 2D UV offsets, one per pixel.
///
/// The GPU reads this as a 2D texture (RG32F) and uses it to offset UV lookups
/// when sampling the scene texture.
pub struct DistortionMap {
    pub width:  u32,
    pub height: u32,
    /// Flat array of (u_offset, v_offset) pairs, row-major.
    offsets: Vec<Vec2>,
}

impl DistortionMap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            offsets: vec![Vec2::ZERO; (width * height) as usize],
        }
    }

    pub fn clear(&mut self) {
        self.offsets.iter_mut().for_each(|o| *o = Vec2::ZERO);
    }

    pub fn set(&mut self, x: u32, y: u32, offset: Vec2) {
        if x < self.width && y < self.height {
            self.offsets[(y * self.width + x) as usize] = offset;
        }
    }

    pub fn get(&self, x: u32, y: u32) -> Vec2 {
        if x < self.width && y < self.height {
            self.offsets[(y * self.width + x) as usize]
        } else {
            Vec2::ZERO
        }
    }

    pub fn add(&mut self, x: u32, y: u32, offset: Vec2) {
        if x < self.width && y < self.height {
            self.offsets[(y * self.width + x) as usize] += offset;
        }
    }

    /// Raw f32 slice for GPU upload (RGBA32F, packed as [r_offset, g_offset, 0, 0] per pixel).
    pub fn as_f32_slice(&self) -> Vec<f32> {
        self.offsets.iter().flat_map(|o| [o.x, o.y, 0.0, 0.0]).collect()
    }

    /// Write a radial gravity lens distortion around a screen-space center.
    ///
    /// `center_uv` in [0, 1], `strength` is UV units of max displacement,
    /// `radius_uv` is the falloff radius in UV units.
    pub fn add_gravity_lens(&mut self, center_uv: Vec2, strength: f32, radius_uv: f32) {
        for y in 0..self.height {
            for x in 0..self.width {
                let uv = Vec2::new(
                    x as f32 / self.width  as f32,
                    y as f32 / self.height as f32,
                );
                let delta = center_uv - uv;
                let dist = delta.length();
                if dist < radius_uv && dist > 0.0001 {
                    let factor = (1.0 - dist / radius_uv).powi(2);
                    let pull = delta / dist * strength * factor;
                    self.add(x, y, pull);
                }
            }
        }
    }

    /// Write a heat shimmer distortion (Perlin noise based, animated by time).
    pub fn add_heat_shimmer(
        &mut self,
        region_min: Vec2,
        region_max: Vec2,
        strength: f32,
        time: f32,
    ) {
        for y in 0..self.height {
            for x in 0..self.width {
                let uv = Vec2::new(
                    x as f32 / self.width  as f32,
                    y as f32 / self.height as f32,
                );
                if uv.x < region_min.x || uv.x > region_max.x
                    || uv.y < region_min.y || uv.y > region_max.y {
                    continue;
                }
                // Simple sine-wave shimmer (no Perlin dependency here for self-containment)
                let phase_x = (uv.y * 20.0 + time * 3.0).sin();
                let phase_y = (uv.x * 15.0 + time * 2.3).cos();
                let offset = Vec2::new(phase_x, phase_y) * strength;
                self.add(x, y, offset);
            }
        }
    }

    /// Clamp all offsets to `max_magnitude` UV units.
    pub fn clamp_offsets(&mut self, max_magnitude: f32) {
        for o in &mut self.offsets {
            let len = o.length();
            if len > max_magnitude {
                *o = *o / len * max_magnitude;
            }
        }
    }

    pub fn pixel_count(&self) -> usize { self.offsets.len() }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_map_is_all_zero() {
        let map = DistortionMap::new(16, 16);
        assert_eq!(map.get(0, 0), Vec2::ZERO);
        assert_eq!(map.get(8, 8), Vec2::ZERO);
    }

    #[test]
    fn set_and_get() {
        let mut map = DistortionMap::new(16, 16);
        map.set(3, 5, Vec2::new(0.01, -0.02));
        let v = map.get(3, 5);
        assert!((v.x - 0.01).abs() < 1e-6);
        assert!((v.y + 0.02).abs() < 1e-6);
    }

    #[test]
    fn clear_resets_all() {
        let mut map = DistortionMap::new(8, 8);
        map.set(2, 2, Vec2::new(1.0, 1.0));
        map.clear();
        assert_eq!(map.get(2, 2), Vec2::ZERO);
    }

    #[test]
    fn gravity_lens_creates_offsets() {
        let mut map = DistortionMap::new(64, 64);
        map.add_gravity_lens(Vec2::new(0.5, 0.5), 0.05, 0.3);
        // Near the center the offset should be near zero (distance = 0)
        // At the edge it should be non-zero
        let edge = map.get(0, 0);
        let near_center = map.get(32, 32);
        assert!(edge.length() > near_center.length());
    }

    #[test]
    fn clamp_limits_magnitude() {
        let mut map = DistortionMap::new(4, 4);
        map.set(0, 0, Vec2::new(10.0, 10.0));
        map.clamp_offsets(0.1);
        assert!(map.get(0, 0).length() <= 0.11);
    }

    #[test]
    fn as_f32_slice_length() {
        let map = DistortionMap::new(8, 8);
        let v = map.as_f32_slice();
        assert_eq!(v.len(), 8 * 8 * 4);  // 4 floats per pixel (RGBA)
    }

    #[test]
    fn chaos_rift_scales() {
        let low  = DistortionParams::chaos_rift(0.1);
        let high = DistortionParams::chaos_rift(0.9);
        assert!(high.scale > low.scale);
    }
}
