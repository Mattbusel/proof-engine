//! Color Grading LUT — 3D look-up table generation, blending, and GPU upload.
//!
//! Generates a 3D LUT from color grade parameters and applies it as a post-processing
//! pass. The LUT transforms input RGB → output RGB in a single texture lookup,
//! enabling complex color transformations at zero per-pixel math cost.
//!
//! # Sizes
//!
//! - 16³ = 4,096 entries × 3 channels = 12,288 bytes (fast, low quality)
//! - 32³ = 32,768 entries × 3 channels = 98,304 bytes (high quality)
//!
//! # Game state LUTs
//!
//! Each game state has a pre-baked LUT. When state changes, we smoothly blend
//! between the old and new LUT over a configurable duration.

use glam::{Vec3, Vec4};
use super::color_grade::ColorGradeParams;

// ── LUT data ────────────────────────────────────────────────────────────────

/// A 3D color look-up table.
#[derive(Clone, Debug)]
pub struct Lut3D {
    /// LUT dimension (e.g. 16 or 32). Total entries = size³.
    pub size: u32,
    /// RGB data, row-major: data[r + g*size + b*size*size] = (out_r, out_g, out_b).
    pub data: Vec<[f32; 3]>,
}

impl Lut3D {
    /// Create an identity LUT (output = input) of the given size.
    pub fn identity(size: u32) -> Self {
        let total = (size * size * size) as usize;
        let mut data = Vec::with_capacity(total);
        let scale = 1.0 / (size - 1).max(1) as f32;

        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.push([r as f32 * scale, g as f32 * scale, b as f32 * scale]);
                }
            }
        }

        Self { size, data }
    }

    /// Generate a LUT from color grade parameters.
    pub fn from_params(size: u32, params: &ColorGradeParams) -> Self {
        let mut lut = Self::identity(size);
        let scale = 1.0 / (size - 1).max(1) as f32;

        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    let idx = (r + g * size + b * size * size) as usize;
                    let input = Vec3::new(r as f32 * scale, g as f32 * scale, b as f32 * scale);
                    let output = apply_grade(input, params);
                    lut.data[idx] = [output.x, output.y, output.z];
                }
            }
        }

        lut
    }

    /// Sample the LUT with trilinear interpolation.
    pub fn sample(&self, input: Vec3) -> Vec3 {
        let s = (self.size - 1) as f32;
        let r = (input.x * s).clamp(0.0, s);
        let g = (input.y * s).clamp(0.0, s);
        let b = (input.z * s).clamp(0.0, s);

        let r0 = r.floor() as u32;
        let g0 = g.floor() as u32;
        let b0 = b.floor() as u32;
        let r1 = (r0 + 1).min(self.size - 1);
        let g1 = (g0 + 1).min(self.size - 1);
        let b1 = (b0 + 1).min(self.size - 1);

        let fr = r.fract();
        let fg = g.fract();
        let fb = b.fract();

        // Trilinear interpolation
        let fetch = |ri: u32, gi: u32, bi: u32| -> Vec3 {
            let idx = (ri + gi * self.size + bi * self.size * self.size) as usize;
            let d = self.data[idx];
            Vec3::new(d[0], d[1], d[2])
        };

        let c000 = fetch(r0, g0, b0);
        let c100 = fetch(r1, g0, b0);
        let c010 = fetch(r0, g1, b0);
        let c110 = fetch(r1, g1, b0);
        let c001 = fetch(r0, g0, b1);
        let c101 = fetch(r1, g0, b1);
        let c011 = fetch(r0, g1, b1);
        let c111 = fetch(r1, g1, b1);

        let c00 = c000.lerp(c100, fr);
        let c10 = c010.lerp(c110, fr);
        let c01 = c001.lerp(c101, fr);
        let c11 = c011.lerp(c111, fr);

        let c0 = c00.lerp(c10, fg);
        let c1 = c01.lerp(c11, fg);

        c0.lerp(c1, fb)
    }

    /// Linearly blend two LUTs together. `t` = 0.0 returns `self`, 1.0 returns `other`.
    pub fn blend(&self, other: &Lut3D, t: f32) -> Lut3D {
        assert_eq!(self.size, other.size, "LUT sizes must match for blending");
        let t = t.clamp(0.0, 1.0);
        let data: Vec<[f32; 3]> = self.data.iter().zip(other.data.iter()).map(|(a, b)| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        }).collect();

        Lut3D { size: self.size, data }
    }

    /// Convert to flat f32 RGB buffer for GPU upload.
    pub fn to_rgb_f32(&self) -> Vec<f32> {
        let mut buf = Vec::with_capacity(self.data.len() * 3);
        for entry in &self.data {
            buf.push(entry[0]);
            buf.push(entry[1]);
            buf.push(entry[2]);
        }
        buf
    }

    /// Convert to u8 RGB buffer for GPU upload (RGBA8 3D texture).
    pub fn to_rgb_u8(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.data.len() * 3);
        for entry in &self.data {
            buf.push((entry[0].clamp(0.0, 1.0) * 255.0) as u8);
            buf.push((entry[1].clamp(0.0, 1.0) * 255.0) as u8);
            buf.push((entry[2].clamp(0.0, 1.0) * 255.0) as u8);
        }
        buf
    }

    /// Total number of entries.
    pub fn entry_count(&self) -> usize { (self.size * self.size * self.size) as usize }

    /// Memory size in bytes (f32 RGB).
    pub fn memory_bytes(&self) -> usize { self.entry_count() * 3 * 4 }
}

// ── Color grade application (CPU-side) ──────────────────────────────────────

fn apply_grade(color: Vec3, params: &ColorGradeParams) -> Vec3 {
    let mut c = color;

    // Tint
    c *= params.tint;

    // Brightness
    c += Vec3::splat(params.brightness);

    // Contrast (around 0.5 midpoint)
    c = (c - Vec3::splat(0.5)) * params.contrast + Vec3::splat(0.5);

    // Lift/Gamma/Gain
    c = c * params.gain + params.lift;
    if params.gamma != Vec3::ONE {
        c = Vec3::new(
            c.x.max(0.0).powf(1.0 / params.gamma.x.max(0.01)),
            c.y.max(0.0).powf(1.0 / params.gamma.y.max(0.01)),
            c.z.max(0.0).powf(1.0 / params.gamma.z.max(0.01)),
        );
    }

    // Saturation
    let lum = 0.2126 * c.x + 0.7152 * c.y + 0.0722 * c.z;
    c = Vec3::splat(lum).lerp(c, params.saturation);

    // Split toning
    if params.shadow_tint_strength > 0.0 || params.highlight_tint_strength > 0.0 {
        let shadow_w = (1.0 - lum / params.split_midpoint.max(0.01)).clamp(0.0, 1.0);
        let highlight_w = (lum / params.split_midpoint.max(0.01) - 1.0).clamp(0.0, 1.0);
        c += params.shadow_tint * shadow_w * params.shadow_tint_strength;
        c += params.highlight_tint * highlight_w * params.highlight_tint_strength;
    }

    // Clamp
    Vec3::new(c.x.clamp(0.0, 1.0), c.y.clamp(0.0, 1.0), c.z.clamp(0.0, 1.0))
}

// ── Game State LUT Presets ──────────────────────────────────────────────────

/// Pre-built LUT presets for each game state.
pub struct GameStateLuts;

impl GameStateLuts {
    const SIZE: u32 = 16;

    /// Normal gameplay: neutral with slight theme tint.
    pub fn normal() -> Lut3D {
        let params = ColorGradeParams::default();
        Lut3D::from_params(Self::SIZE, &params)
    }

    /// Low HP: desaturated with red push.
    pub fn low_hp(severity: f32) -> Lut3D {
        let params = ColorGradeParams::danger(severity);
        Lut3D::from_params(Self::SIZE, &params)
    }

    /// High corruption: purple shift, reduced contrast.
    pub fn corruption(level: f32) -> Lut3D {
        let t = (level / 1000.0).clamp(0.0, 1.0);
        let params = ColorGradeParams {
            tint: Vec3::new(0.9 + t * 0.1, 0.8 - t * 0.2, 0.95 + t * 0.15),
            saturation: 1.0 + t * 0.3,
            contrast: 1.0 - t * 0.15,
            shadow_tint: Vec3::new(0.15, 0.0, 0.3),
            shadow_tint_strength: t * 0.5,
            brightness: t * 0.05,
            ..Default::default()
        };
        Lut3D::from_params(Self::SIZE, &params)
    }

    /// Death: full desaturation over progress (0→1).
    pub fn death(progress: f32) -> Lut3D {
        let params = ColorGradeParams::death(progress);
        Lut3D::from_params(Self::SIZE, &params)
    }

    /// Victory: warm golden tint.
    pub fn victory() -> Lut3D {
        let params = ColorGradeParams::victory();
        Lut3D::from_params(Self::SIZE, &params)
    }

    /// Boss fight: high contrast, deep shadows.
    pub fn boss_fight() -> Lut3D {
        let params = ColorGradeParams {
            contrast: 1.3,
            saturation: 1.1,
            shadow_tint: Vec3::new(0.05, 0.0, 0.1),
            shadow_tint_strength: 0.4,
            vignette: 0.35,
            lift: Vec3::new(-0.02, -0.02, -0.01),
            ..Default::default()
        };
        Lut3D::from_params(Self::SIZE, &params)
    }

    /// Shrine: blue-shifted, soft, low contrast.
    pub fn shrine() -> Lut3D {
        let params = ColorGradeParams {
            tint: Vec3::new(0.85, 0.9, 1.15),
            contrast: 0.85,
            saturation: 0.9,
            brightness: 0.05,
            highlight_tint: Vec3::new(0.7, 0.8, 1.0),
            highlight_tint_strength: 0.3,
            ..Default::default()
        };
        Lut3D::from_params(Self::SIZE, &params)
    }

    /// Chaos Rift: oversaturated, high contrast, green-purple split tone.
    pub fn chaos_rift() -> Lut3D {
        let params = ColorGradeParams {
            saturation: 1.6,
            contrast: 1.4,
            shadow_tint: Vec3::new(0.0, 0.2, 0.0),
            shadow_tint_strength: 0.5,
            highlight_tint: Vec3::new(0.5, 0.0, 0.5),
            highlight_tint_strength: 0.4,
            ..Default::default()
        };
        Lut3D::from_params(Self::SIZE, &params)
    }
}

// ── LUT Blender ─────────────────────────────────────────────────────────────

/// Smoothly interpolates between LUTs when game state changes.
pub struct LutBlender {
    /// Current active LUT.
    current: Lut3D,
    /// Target LUT (what we're blending toward).
    target: Option<Lut3D>,
    /// Blend progress: 0.0 = current, 1.0 = target.
    progress: f32,
    /// Blend duration in seconds.
    duration: f32,
    /// Blended result (updated each tick).
    blended: Lut3D,
    /// Whether the blended LUT needs re-upload to GPU.
    pub dirty: bool,
}

impl LutBlender {
    pub fn new(initial: Lut3D) -> Self {
        let blended = initial.clone();
        Self {
            current: initial,
            target: None,
            progress: 0.0,
            duration: 0.0,
            blended,
            dirty: true,
        }
    }

    /// Start blending toward a new LUT over `duration` seconds.
    pub fn blend_to(&mut self, target: Lut3D, duration: f32) {
        // If already at this target, skip
        self.current = self.blended.clone();
        self.target = Some(target);
        self.progress = 0.0;
        self.duration = duration.max(0.01);
        self.dirty = true;
    }

    /// Instant cut to a new LUT (no blend).
    pub fn set(&mut self, lut: Lut3D) {
        self.current = lut.clone();
        self.target = None;
        self.progress = 0.0;
        self.blended = lut;
        self.dirty = true;
    }

    /// Advance the blend by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if let Some(ref target) = self.target {
            self.progress = (self.progress + dt / self.duration).min(1.0);

            // Smooth-step easing
            let t = self.progress;
            let eased = t * t * (3.0 - 2.0 * t);

            self.blended = self.current.blend(target, eased);
            self.dirty = true;

            if self.progress >= 1.0 {
                self.current = self.blended.clone();
                self.target = None;
            }
        }
    }

    /// Whether a blend is currently in progress.
    pub fn is_blending(&self) -> bool { self.target.is_some() }

    /// Current blend progress (0.0 to 1.0).
    pub fn blend_progress(&self) -> f32 { self.progress }

    /// Get the current (possibly blended) LUT.
    pub fn current_lut(&self) -> &Lut3D { &self.blended }

    /// Consume the dirty flag. Returns true if the LUT needs re-upload.
    pub fn take_dirty(&mut self) -> bool {
        let d = self.dirty;
        self.dirty = false;
        d
    }
}

// ── Blend durations for game state transitions ──────────────────────────────

/// Standard blend durations for LUT transitions between game states.
pub struct LutBlendDurations;

impl LutBlendDurations {
    /// Combat state changes (entering/exiting combat).
    pub const COMBAT: f32 = 0.3;
    /// Boss encounter entry.
    pub const BOSS_ENTRY: f32 = 0.5;
    /// HP threshold changes.
    pub const HP_CHANGE: f32 = 0.4;
    /// Death sequence.
    pub const DEATH: f32 = 2.0;
    /// Victory celebration.
    pub const VICTORY: f32 = 1.5;
    /// Shrine entry.
    pub const SHRINE: f32 = 0.8;
    /// Corruption level changes.
    pub const CORRUPTION: f32 = 0.6;
    /// Floor/room transition.
    pub const FLOOR_CHANGE: f32 = 0.5;
    /// Chaos rift entry.
    pub const CHAOS_RIFT: f32 = 0.4;
}

// ── GLSL shader for LUT application ────────────────────────────────────────

/// Fragment shader that applies a 3D LUT to the scene color.
/// The LUT is stored as a 3D texture (GL_TEXTURE_3D).
pub const LUT_APPLY_FRAG: &str = r#"
#version 330 core

in  vec2 f_uv;
out vec4 frag_color;

uniform sampler2D u_scene;
uniform sampler3D u_lut;
uniform float     u_lut_strength;

void main() {
    vec3 color = texture(u_scene, f_uv).rgb;

    // Clamp to [0,1] before LUT lookup
    vec3 clamped = clamp(color, 0.0, 1.0);

    // 3D LUT lookup
    vec3 graded = texture(u_lut, clamped).rgb;

    // Blend between original and graded
    frag_color = vec4(mix(color, graded, u_lut_strength), 1.0);
}
"#;

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_lut_is_passthrough() {
        let lut = Lut3D::identity(16);
        let input = Vec3::new(0.5, 0.3, 0.8);
        let output = lut.sample(input);
        assert!((output - input).length() < 0.02, "Identity LUT should be passthrough, got {:?}", output);
    }

    #[test]
    fn lut_blend_at_zero_is_first() {
        let a = Lut3D::identity(8);
        let mut b = Lut3D::identity(8);
        // Make b different
        for entry in &mut b.data { entry[0] = 0.0; }
        let blended = a.blend(&b, 0.0);
        // Should be identical to a
        for (ae, be) in a.data.iter().zip(blended.data.iter()) {
            assert!((ae[0] - be[0]).abs() < 1e-6);
        }
    }

    #[test]
    fn lut_blend_at_one_is_second() {
        let a = Lut3D::identity(8);
        let mut b = Lut3D::identity(8);
        for entry in &mut b.data { entry[0] = 0.0; }
        let blended = a.blend(&b, 1.0);
        for (be, re) in b.data.iter().zip(blended.data.iter()) {
            assert!((be[0] - re[0]).abs() < 1e-6);
        }
    }

    #[test]
    fn game_state_luts_differ() {
        let normal = GameStateLuts::normal();
        let boss = GameStateLuts::boss_fight();
        // They shouldn't be identical
        let diffs: usize = normal.data.iter().zip(boss.data.iter())
            .filter(|(a, b)| (a[0] - b[0]).abs() > 0.01)
            .count();
        assert!(diffs > 0, "Boss LUT should differ from normal");
    }

    #[test]
    fn lut_blender_completes() {
        let a = GameStateLuts::normal();
        let b = GameStateLuts::boss_fight();
        let mut blender = LutBlender::new(a);
        blender.blend_to(b, 1.0);

        assert!(blender.is_blending());
        for _ in 0..100 {
            blender.tick(0.02);
        }
        assert!(!blender.is_blending());
        assert!(blender.blend_progress() >= 1.0);
    }

    #[test]
    fn lut_to_u8_correct_range() {
        let lut = Lut3D::identity(4);
        let bytes = lut.to_rgb_u8();
        assert_eq!(bytes.len(), 4 * 4 * 4 * 3);
        assert!(*bytes.iter().max().unwrap() <= 255);
    }

    #[test]
    fn death_lut_desaturates() {
        let lut = GameStateLuts::death(1.0);
        // A bright red should be nearly grey when fully desaturated
        let output = lut.sample(Vec3::new(1.0, 0.0, 0.0));
        // R and G should be closer together than in the input
        assert!((output.x - output.y).abs() < 0.5, "Death LUT should desaturate, got {:?}", output);
    }
}
