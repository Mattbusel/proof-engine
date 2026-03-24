//! Perlin and Simplex noise implementations.


/// Hash a 2D integer coordinate to a float gradient.
fn hash2(ix: i32, iy: i32) -> f32 {
    let n = (ix.wrapping_mul(127) ^ iy.wrapping_mul(311)) as u64;
    let n = n.wrapping_mul(0x9e3779b97f4a7c15);
    ((n >> 32) as f32) / u32::MAX as f32 * 2.0 - 1.0
}

fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }
fn lerp(a: f32, b: f32, t: f32) -> f32 { a + t * (b - a) }

/// 2D Perlin noise. Output is roughly in [-1, 1].
pub fn perlin2(x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i32; let x1 = x0 + 1;
    let y0 = y.floor() as i32; let y1 = y0 + 1;
    let fx = x - x0 as f32;  let fy = y - y0 as f32;
    let u = fade(fx); let v = fade(fy);
    let g00 = hash2(x0, y0); let g10 = hash2(x1, y0);
    let g01 = hash2(x0, y1); let g11 = hash2(x1, y1);
    let n00 = g00 * fx         + g00 * fy;
    let n10 = g10 * (fx - 1.0) + g10 * fy;
    let n01 = g01 * fx         + g01 * (fy - 1.0);
    let n11 = g11 * (fx - 1.0) + g11 * (fy - 1.0);
    lerp(lerp(n00, n10, u), lerp(n01, n11, u), v)
}

/// Octave Perlin noise (fBm).
pub fn fbm(x: f32, y: f32, octaves: u8, persistence: f32, lacunarity: f32) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max = 0.0f32;
    for _ in 0..octaves {
        value += perlin2(x * frequency, y * frequency) * amplitude;
        max += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    value / max
}

/// 1D value noise.
pub fn noise1(t: f32) -> f32 {
    let i = t.floor() as i64;
    let f = t.fract();
    let a = hash_1d(i);
    let b = hash_1d(i + 1);
    let u = fade(f);
    a + (b - a) * u
}

fn hash_1d(i: i64) -> f32 {
    let x = (i ^ (i >> 13)) as u64;
    let x = x.wrapping_mul(0x9e3779b97f4a7c15);
    ((x >> 32) as f32) / u32::MAX as f32
}

/// Turbulence (absolute value fBm).
pub fn turbulence(x: f32, y: f32, octaves: u8) -> f32 {
    let mut value = 0.0f32;
    let mut amplitude = 1.0f32;
    let mut frequency = 1.0f32;
    let mut max = 0.0f32;
    for _ in 0..octaves {
        value += perlin2(x * frequency, y * frequency).abs() * amplitude;
        max += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    value / max
}
