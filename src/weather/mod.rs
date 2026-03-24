pub mod atmosphere;
pub mod precipitation;
pub mod climate;

pub use atmosphere::*;
pub use precipitation::*;
pub use climate::*;

// Shared utilities expected by submodules.

/// Simple 3D vector used throughout weather simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
    pub fn length(&self) -> f32 { (self.x * self.x + self.y * self.y + self.z * self.z).sqrt() }
    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len < 1e-12 { Self::ZERO } else { Self { x: self.x / len, y: self.y / len, z: self.z / len } }
    }
    pub fn dot(&self, other: &Self) -> f32 { self.x * other.x + self.y * other.y + self.z * other.z }
    pub fn scale(&self, s: f32) -> Self { Self { x: self.x * s, y: self.y * s, z: self.z * s } }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self { Self { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z } }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self { Self { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z } }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self { Self { x: self.x * rhs, y: self.y * rhs, z: self.z * rhs } }
}

impl std::ops::AddAssign for Vec3 {
    fn add_assign(&mut self, rhs: Self) { self.x += rhs.x; self.y += rhs.y; self.z += rhs.z; }
}

impl std::ops::MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, rhs: f32) { self.x *= rhs; self.y *= rhs; self.z *= rhs; }
}

/// Linear interpolation.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Hermite smooth-step.
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// Simple hash-based value noise for weather simulation (no external deps).
fn hash_u32(mut x: u32) -> u32 {
    x = x.wrapping_mul(0x85ebca6b);
    x ^= x >> 13;
    x = x.wrapping_mul(0xc2b2ae35);
    x ^= x >> 16;
    x
}

fn hash_2d(ix: i32, iy: i32) -> f32 {
    let h = hash_u32((ix as u32).wrapping_mul(73856093) ^ (iy as u32).wrapping_mul(19349663));
    (h & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

/// 2D value noise in [0, 1].
pub fn value_noise_2d(x: f32, y: f32) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let u = fx * fx * (3.0 - 2.0 * fx);
    let v = fy * fy * (3.0 - 2.0 * fy);

    let c00 = hash_2d(ix, iy);
    let c10 = hash_2d(ix + 1, iy);
    let c01 = hash_2d(ix, iy + 1);
    let c11 = hash_2d(ix + 1, iy + 1);

    lerp(lerp(c00, c10, u), lerp(c01, c11, u), v)
}

/// FBM (fractional Brownian motion) from value noise.
pub fn fbm_2d(x: f32, y: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
    let mut sum = 0.0f32;
    let mut amp = 1.0f32;
    let mut freq = 1.0f32;
    let mut max_amp = 0.0f32;
    for _ in 0..octaves {
        sum += value_noise_2d(x * freq, y * freq) * amp;
        max_amp += amp;
        amp *= gain;
        freq *= lacunarity;
    }
    sum / max_amp
}
