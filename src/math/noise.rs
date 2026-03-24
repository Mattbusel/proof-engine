//! Noise functions for the Proof Engine mathematical renderer.
//!
//! Implementations: Perlin 1-D / 2-D / 3-D, Simplex 2-D / 3-D, Cellular / Worley,
//! Curl (divergence-free), Domain Warping, Ridged Multifractal, Billow, and
//! Fractional Brownian Motion over each.  All functions are stateless, deterministic,
//! and seeded by a compile-time permutation table.

use glam::{Vec2, Vec3};

// ─────────────────────────────────────────────────────────────────────────────
// Permutation table — Ken Perlin's canonical p256, doubled to avoid modular
// indexing in hot paths.
// ─────────────────────────────────────────────────────────────────────────────

const PERM_BASE: [u8; 256] = [
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

const P: [u8; 512] = {
    let mut out = [0u8; 512];
    let mut i = 0usize;
    while i < 256 { out[i] = PERM_BASE[i]; out[i + 256] = PERM_BASE[i]; i += 1; }
    out
};

#[inline(always)] fn p(i: usize) -> usize { P[i & 511] as usize }
#[inline(always)] fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }
#[inline(always)] fn lerp(a: f32, b: f32, t: f32) -> f32 { a + t * (b - a) }

// ─────────────────────────────────────────────────────────────────────────────
// 1-D Perlin noise
// ─────────────────────────────────────────────────────────────────────────────

#[inline(always)]
fn grad1(h: usize, x: f32) -> f32 { if h & 1 == 0 { x } else { -x } }

/// 1-D Perlin noise.  Output ≈ [-1, 1].
pub fn perlin1(x: f32) -> f32 {
    let xi = x.floor() as i32 as usize & 255;
    let xf = x - x.floor();
    let u = fade(xf);
    lerp(grad1(p(xi), xf), grad1(p(xi + 1), xf - 1.0), u)
}

// ─────────────────────────────────────────────────────────────────────────────
// 2-D Perlin noise
// ─────────────────────────────────────────────────────────────────────────────

#[inline(always)]
fn grad2(h: usize, x: f32, y: f32) -> f32 {
    match h & 3 { 0 => x + y, 1 => -x + y, 2 => x - y, _ => -x - y }
}

/// 2-D Perlin noise.  Output ≈ [-1, 1].
pub fn perlin2(x: f32, y: f32) -> f32 {
    let xi = x.floor() as i32 as usize & 255;
    let yi = y.floor() as i32 as usize & 255;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf); let v = fade(yf);

    let aa = p(p(xi) + yi);       let ba = p(p(xi + 1) + yi);
    let ab = p(p(xi) + yi + 1);   let bb = p(p(xi + 1) + yi + 1);

    lerp(
        lerp(grad2(aa, xf, yf),       grad2(ba, xf - 1.0, yf),       u),
        lerp(grad2(ab, xf, yf - 1.0), grad2(bb, xf - 1.0, yf - 1.0), u),
        v,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// 3-D Perlin noise
// ─────────────────────────────────────────────────────────────────────────────

#[inline(always)]
fn grad3(h: usize, x: f32, y: f32, z: f32) -> f32 {
    match h & 15 {
        0  =>  x + y,  1 => -x + y,  2 =>  x - y,  3 => -x - y,
        4  =>  x + z,  5 => -x + z,  6 =>  x - z,  7 => -x - z,
        8  =>  y + z,  9 => -y + z, 10 =>  y - z, 11 => -y - z,
        12 =>  y + x, 13 => -y + z, 14 =>  y - x,  _ => -y - z,
    }
}

/// 3-D Perlin noise.  Output ≈ [-1, 1].
pub fn perlin3(x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor() as i32 as usize & 255;
    let yi = y.floor() as i32 as usize & 255;
    let zi = z.floor() as i32 as usize & 255;
    let xf = x - x.floor(); let yf = y - y.floor(); let zf = z - z.floor();
    let u = fade(xf); let v = fade(yf); let w = fade(zf);

    let aaa = p(p(p(xi)+yi)+zi);        let baa = p(p(p(xi+1)+yi)+zi);
    let aba = p(p(p(xi)+yi+1)+zi);      let bba = p(p(p(xi+1)+yi+1)+zi);
    let aab = p(p(p(xi)+yi)+zi+1);      let bab = p(p(p(xi+1)+yi)+zi+1);
    let abb = p(p(p(xi)+yi+1)+zi+1);    let bbb = p(p(p(xi+1)+yi+1)+zi+1);

    let x1 = lerp(grad3(aaa,xf,yf,zf),   grad3(baa,xf-1.0,yf,zf),   u);
    let x2 = lerp(grad3(aba,xf,yf-1.0,zf),grad3(bba,xf-1.0,yf-1.0,zf),u);
    let x3 = lerp(grad3(aab,xf,yf,zf-1.0),grad3(bab,xf-1.0,yf,zf-1.0),u);
    let x4 = lerp(grad3(abb,xf,yf-1.0,zf-1.0),grad3(bbb,xf-1.0,yf-1.0,zf-1.0),u);

    lerp(lerp(x1, x2, v), lerp(x3, x4, v), w)
}

// ─────────────────────────────────────────────────────────────────────────────
// 2-D Simplex noise
// ─────────────────────────────────────────────────────────────────────────────

const F2: f32 = 0.366025403784439;  // (sqrt(3) - 1) / 2
const G2: f32 = 0.211324865405187;  // (3 - sqrt(3)) / 6

fn grad2s(h: usize, x: f32, y: f32) -> f32 {
    let h4 = h & 7;
    let (u, v) = if h4 < 4 { (x, y) } else { (y, x) };
    let su = if h4 & 1 != 0 { -u } else { u };
    let sv = if h4 & 2 != 0 { -2.0 * v } else { 2.0 * v };
    su + sv
}

/// 2-D Simplex noise.  Output ≈ [-1, 1].  Sharper features than Perlin.
pub fn simplex2(x: f32, y: f32) -> f32 {
    let s = (x + y) * F2;
    let i = (x + s).floor() as i32;
    let j = (y + s).floor() as i32;
    let t = (i + j) as f32 * G2;
    let x0 = x - (i as f32 - t);
    let y0 = y - (j as f32 - t);

    let (i1, j1) = if x0 > y0 { (1i32, 0i32) } else { (0i32, 1i32) };
    let x1 = x0 - i1 as f32 + G2;
    let y1 = y0 - j1 as f32 + G2;
    let x2 = x0 - 1.0 + 2.0 * G2;
    let y2 = y0 - 1.0 + 2.0 * G2;

    let ii = (i & 255) as usize;
    let jj = (j & 255) as usize;
    let gi0 = p(ii          + p(jj));
    let gi1 = p(ii + i1 as usize + p(jj + j1 as usize));
    let gi2 = p(ii + 1      + p(jj + 1));

    let contrib = |gi: usize, dx: f32, dy: f32| -> f32 {
        let t = 0.5 - dx * dx - dy * dy;
        if t < 0.0 { 0.0 } else { let t2 = t * t; t2 * t2 * grad2s(gi, dx, dy) }
    };
    70.0 * (contrib(gi0, x0, y0) + contrib(gi1, x1, y1) + contrib(gi2, x2, y2))
}

// ─────────────────────────────────────────────────────────────────────────────
// 3-D Simplex noise
// ─────────────────────────────────────────────────────────────────────────────

const F3: f32 = 1.0 / 3.0;
const G3: f32 = 1.0 / 6.0;

/// 3-D Simplex noise.  Output ≈ [-1, 1].
pub fn simplex3(x: f32, y: f32, z: f32) -> f32 {
    let s = (x + y + z) * F3;
    let i = (x + s).floor() as i32;
    let j = (y + s).floor() as i32;
    let k = (z + s).floor() as i32;
    let t = (i + j + k) as f32 * G3;
    let x0 = x - (i as f32 - t);
    let y0 = y - (j as f32 - t);
    let z0 = z - (k as f32 - t);

    let (i1,j1,k1,i2,j2,k2) = if x0 >= y0 {
        if y0 >= z0 { (1,0,0, 1,1,0) } else if x0 >= z0 { (1,0,0, 1,0,1) } else { (0,0,1, 1,0,1) }
    } else {
        if y0 < z0 { (0,0,1, 0,1,1) } else if x0 < z0 { (0,1,0, 0,1,1) } else { (0,1,0, 1,1,0) }
    };

    let g3 = G3;
    let (x1,y1,z1) = (x0-i1 as f32+g3,   y0-j1 as f32+g3,   z0-k1 as f32+g3);
    let (x2,y2,z2) = (x0-i2 as f32+2.0*g3, y0-j2 as f32+2.0*g3, z0-k2 as f32+2.0*g3);
    let (x3,y3,z3) = (x0-1.0+3.0*g3,     y0-1.0+3.0*g3,     z0-1.0+3.0*g3);

    let ii = (i & 255) as usize; let jj = (j & 255) as usize; let kk = (k & 255) as usize;
    let gi0 = p(ii          + p(jj          + p(kk)));
    let gi1 = p(ii+i1 as usize + p(jj+j1 as usize + p(kk+k1 as usize)));
    let gi2 = p(ii+i2 as usize + p(jj+j2 as usize + p(kk+k2 as usize)));
    let gi3 = p(ii+1        + p(jj+1        + p(kk+1)));

    let contrib = |gi: usize, dx: f32, dy: f32, dz: f32| -> f32 {
        let t = 0.6 - dx*dx - dy*dy - dz*dz;
        if t < 0.0 { 0.0 } else { let t2 = t*t; t2*t2*grad3(gi, dx, dy, dz) }
    };
    32.0 * (contrib(gi0,x0,y0,z0) + contrib(gi1,x1,y1,z1) + contrib(gi2,x2,y2,z2) + contrib(gi3,x3,y3,z3))
}

// ─────────────────────────────────────────────────────────────────────────────
// Fractional Brownian Motion (fBm)
// ─────────────────────────────────────────────────────────────────────────────

/// Classic 2-D fBm over Perlin noise.
///
/// # Arguments
/// * `persistence` – amplitude multiplier per octave (0.5 = halved each time)
/// * `lacunarity`  – frequency multiplier per octave (2.0 = doubled each time)
pub fn fbm(x: f32, y: f32, octaves: u8, persistence: f32, lacunarity: f32) -> f32 {
    let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; let mut norm = 0.0f32;
    for _ in 0..octaves {
        v += perlin2(x * freq, y * freq) * amp; norm += amp; amp *= persistence; freq *= lacunarity;
    }
    v / norm
}

/// 3-D fBm over Perlin noise.
pub fn fbm3(x: f32, y: f32, z: f32, octaves: u8, persistence: f32, lacunarity: f32) -> f32 {
    let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; let mut norm = 0.0f32;
    for _ in 0..octaves {
        v += perlin3(x*freq,y*freq,z*freq) * amp; norm += amp; amp *= persistence; freq *= lacunarity;
    }
    v / norm
}

/// 2-D fBm over Simplex noise — tighter, more crystalline features.
pub fn fbm_simplex(x: f32, y: f32, octaves: u8, persistence: f32, lacunarity: f32) -> f32 {
    let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; let mut norm = 0.0f32;
    for _ in 0..octaves {
        v += simplex2(x*freq, y*freq) * amp; norm += amp; amp *= persistence; freq *= lacunarity;
    }
    v / norm
}

/// 1-D fBm over Perlin noise — useful for audio modulation and time curves.
pub fn fbm1(t: f32, octaves: u8, persistence: f32, lacunarity: f32) -> f32 {
    let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; let mut norm = 0.0f32;
    for _ in 0..octaves {
        v += perlin1(t * freq) * amp; norm += amp; amp *= persistence; freq *= lacunarity;
    }
    v / norm
}

// ─────────────────────────────────────────────────────────────────────────────
// Ridged Multifractal noise
// ─────────────────────────────────────────────────────────────────────────────

/// Ridged multifractal — sharp ridges, excellent for lightning, cracks, and mountain terrain.
/// Output ≈ [0, 1].
pub fn ridged(x: f32, y: f32, octaves: u8, lacunarity: f32, gain: f32, offset: f32) -> f32 {
    let mut value = 0.0f32;
    let mut freq = 1.0f32;
    let mut amp = 0.5f32;
    let mut weight = 1.0f32;

    for _ in 0..octaves {
        let mut signal = perlin2(x * freq, y * freq);
        signal = (offset - signal.abs()).powi(2);
        signal *= weight;
        weight = (signal * gain).clamp(0.0, 1.0);
        value += signal * amp;
        freq  *= lacunarity;
        amp   *= 0.5;
    }
    value
}

/// Ridged 3-D variant — useful for volumetric density fields.
pub fn ridged3(x: f32, y: f32, z: f32, octaves: u8, lacunarity: f32, gain: f32) -> f32 {
    let mut value = 0.0f32;
    let mut freq  = 1.0f32;
    let mut amp   = 0.5f32;
    let mut weight = 1.0f32;

    for _ in 0..octaves {
        let mut signal = perlin3(x*freq, y*freq, z*freq);
        signal = (1.0 - signal.abs()).powi(2);
        signal *= weight;
        weight = (signal * gain).clamp(0.0, 1.0);
        value += signal * amp;
        freq  *= lacunarity;
        amp   *= 0.5;
    }
    value
}

// ─────────────────────────────────────────────────────────────────────────────
// Billow noise
// ─────────────────────────────────────────────────────────────────────────────

/// Billow noise — absolute-value fBm, gives cloud and pillow shapes.
/// Output ≈ [-1, 1].
pub fn billow(x: f32, y: f32, octaves: u8, persistence: f32, lacunarity: f32) -> f32 {
    let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; let mut norm = 0.0f32;
    for _ in 0..octaves {
        v += (perlin2(x*freq, y*freq).abs() * 2.0 - 1.0) * amp;
        norm += amp; amp *= persistence; freq *= lacunarity;
    }
    v / norm
}

// ─────────────────────────────────────────────────────────────────────────────
// Turbulence
// ─────────────────────────────────────────────────────────────────────────────

/// 1-D turbulence (abs fBm) — good for entropy ripple and heat haze.
pub fn turbulence1(t: f32, octaves: u8) -> f32 {
    let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; let mut norm = 0.0f32;
    for _ in 0..octaves {
        v += perlin1(t * freq).abs() * amp; norm += amp; amp *= 0.5; freq *= 2.0;
    }
    v / norm
}

/// 2-D turbulence — classic signed-to-abs fBm, used for marble veining.
pub fn turbulence(x: f32, y: f32, octaves: u8) -> f32 {
    let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; let mut norm = 0.0f32;
    for _ in 0..octaves {
        v += perlin2(x * freq, y * freq).abs() * amp; norm += amp; amp *= 0.5; freq *= 2.0;
    }
    v / norm
}

// ─────────────────────────────────────────────────────────────────────────────
// Cellular / Worley noise
// ─────────────────────────────────────────────────────────────────────────────

/// Pseudo-random point within integer grid cell (ix, iy).
fn cell_point_2d(ix: i32, iy: i32) -> (f32, f32) {
    let h = ((ix as i64).wrapping_mul(1031) ^ (iy as i64).wrapping_mul(1099)) as u64;
    let h = h.wrapping_mul(0x9e3779b97f4a7c15_u64);
    let px = ((h >> 32) as f32) / u32::MAX as f32;
    let py = ((h & 0xFFFF_FFFF) as f32) / u32::MAX as f32;
    (ix as f32 + px, iy as f32 + py)
}

/// Pseudo-random point in 3-D integer grid cell.
fn cell_point_3d(ix: i32, iy: i32, iz: i32) -> (f32, f32, f32) {
    let h = ((ix as i64).wrapping_mul(1031)
        ^ (iy as i64).wrapping_mul(1099)
        ^ (iz as i64).wrapping_mul(853)) as u64;
    let h1 = h.wrapping_mul(0x9e3779b97f4a7c15_u64);
    let h2 = h1.wrapping_mul(0x6c62272e07bb0142_u64);
    let h3 = h2.wrapping_mul(0x62b821756295c58d_u64);
    (
        ix as f32 + (h1 >> 32) as f32 / u32::MAX as f32,
        iy as f32 + (h2 >> 32) as f32 / u32::MAX as f32,
        iz as f32 + (h3 >> 32) as f32 / u32::MAX as f32,
    )
}

/// Cellular / Worley noise.  Returns `(f1, f2)` — distances to nearest and
/// second-nearest cell point.
pub fn worley2(x: f32, y: f32) -> (f32, f32) {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let mut f1 = f32::MAX;
    let mut f2 = f32::MAX;
    for dy in -2..=2i32 {
        for dx in -2..=2i32 {
            let (px, py) = cell_point_2d(ix + dx, iy + dy);
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if d < f1 { f2 = f1; f1 = d; } else if d < f2 { f2 = d; }
        }
    }
    (f1, f2)
}

/// 3-D Worley noise.
pub fn worley3(x: f32, y: f32, z: f32) -> (f32, f32) {
    let ix = x.floor() as i32; let iy = y.floor() as i32; let iz = z.floor() as i32;
    let mut f1 = f32::MAX; let mut f2 = f32::MAX;
    for dz in -2..=2i32 { for dy in -2..=2i32 { for dx in -2..=2i32 {
        let (px,py,pz) = cell_point_3d(ix+dx, iy+dy, iz+dz);
        let d = ((x-px).powi(2)+(y-py).powi(2)+(z-pz).powi(2)).sqrt();
        if d < f1 { f2 = f1; f1 = d; } else if d < f2 { f2 = d; }
    }}}
    (f1, f2)
}

/// Nearest-cell Worley, clamped to [0, 1].
pub fn worley_f1(x: f32, y: f32) -> f32 { worley2(x, y).0.clamp(0.0, 1.0) }

/// `f2 - f1` — highlights cell borders, useful for vein / crack patterns.
pub fn worley_crackle(x: f32, y: f32) -> f32 { let (f1,f2) = worley2(x,y); (f2-f1).clamp(0.0,1.0) }

/// Worley fBm — octave-stacked cellular noise for complex surface patterns.
pub fn worley_fbm(x: f32, y: f32, octaves: u8, persistence: f32, lacunarity: f32) -> f32 {
    let mut v = 0.0f32; let mut amp = 1.0f32; let mut freq = 1.0f32; let mut norm = 0.0f32;
    for _ in 0..octaves {
        let (f1, _) = worley2(x * freq, y * freq);
        v += f1 * amp; norm += amp; amp *= persistence; freq *= lacunarity;
    }
    v / norm
}

// ─────────────────────────────────────────────────────────────────────────────
// Curl / Divergence-free noise  (perfect for fluid, smoke, and particle trails)
// ─────────────────────────────────────────────────────────────────────────────

/// 2-D curl noise.  Returns a velocity vector tangent to the gradient of the
/// underlying Perlin field — zero divergence means no sources/sinks.
pub fn curl2(x: f32, y: f32, frequency: f32) -> Vec2 {
    let eps = 0.001f32;
    let f = frequency;
    let dndx = (perlin2((x+eps)*f, y*f) - perlin2((x-eps)*f, y*f)) / (2.0*eps);
    let dndy = (perlin2(x*f, (y+eps)*f) - perlin2(x*f, (y-eps)*f)) / (2.0*eps);
    // curl in 2-D = rotate gradient 90°: (∂n/∂y, -∂n/∂x)
    Vec2::new(dndy, -dndx)
}

/// 2-D curl noise with fBm underneath — richer turbulent flows.
pub fn curl2_fbm(x: f32, y: f32, frequency: f32, octaves: u8) -> Vec2 {
    let eps = 0.001f32;
    let f = frequency;
    let noise = |nx: f32, ny: f32| fbm(nx, ny, octaves, 0.5, 2.0);
    let dndx = (noise((x+eps)*f, y*f) - noise((x-eps)*f, y*f)) / (2.0*eps);
    let dndy = (noise(x*f, (y+eps)*f) - noise(x*f, (y-eps)*f)) / (2.0*eps);
    Vec2::new(dndy, -dndx)
}

/// 3-D curl noise — divergence-free velocity field in 3-D.
/// Uses three separate Perlin potentials for the three curl components.
pub fn curl3(pos: Vec3, frequency: f32) -> Vec3 {
    let eps = 0.001f32;
    let f = frequency;
    let (x,y,z) = (pos.x, pos.y, pos.z);

    // Potential fields Px, Py, Pz (using different offsets to decorrelate)
    let px = |a: f32, b: f32, c: f32| perlin3(a*f + 7.3, b*f + 1.7, c*f + 4.1);
    let py = |a: f32, b: f32, c: f32| perlin3(a*f + 2.9, b*f + 8.5, c*f + 3.2);
    let pz = |a: f32, b: f32, c: f32| perlin3(a*f + 5.1, b*f + 6.3, c*f + 9.7);

    // curl = (∂Pz/∂y - ∂Py/∂z, ∂Px/∂z - ∂Pz/∂x, ∂Py/∂x - ∂Px/∂y)
    let dpz_dy = (pz(x,y+eps,z) - pz(x,y-eps,z)) / (2.0*eps);
    let dpy_dz = (py(x,y,z+eps) - py(x,y,z-eps)) / (2.0*eps);
    let dpx_dz = (px(x,y,z+eps) - px(x,y,z-eps)) / (2.0*eps);
    let dpz_dx = (pz(x+eps,y,z) - pz(x-eps,y,z)) / (2.0*eps);
    let dpy_dx = (py(x+eps,y,z) - py(x-eps,y,z)) / (2.0*eps);
    let dpx_dy = (px(x,y+eps,z) - px(x,y-eps,z)) / (2.0*eps);

    Vec3::new(dpz_dy - dpy_dz, dpx_dz - dpz_dx, dpy_dx - dpx_dy)
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain Warping
// ─────────────────────────────────────────────────────────────────────────────

/// Domain-warped fBm — feeds noise output back into coordinates.
/// Produces the swirling, self-similar patterns seen in marble, flame, and
/// procedural nebulae.  `warp_strength` controls how much to offset coordinates.
pub fn domain_warp(x: f32, y: f32, octaves: u8, warp_strength: f32) -> f32 {
    let qx = fbm(x,         y,         octaves, 0.5, 2.0);
    let qy = fbm(x + 5.2,   y + 1.3,   octaves, 0.5, 2.0);
    let rx = fbm(x + warp_strength * qx + 1.7,  y + warp_strength * qy + 9.2, octaves, 0.5, 2.0);
    let ry = fbm(x + warp_strength * qx + 8.3,  y + warp_strength * qy + 2.8, octaves, 0.5, 2.0);
    fbm(x + warp_strength * rx, y + warp_strength * ry, octaves, 0.5, 2.0)
}

/// Two-level domain warp — deeper self-similarity, heavier compute cost.
pub fn domain_warp2(x: f32, y: f32, octaves: u8, strength: f32) -> f32 {
    let w1 = domain_warp(x, y, octaves, strength * 0.5);
    domain_warp(x + w1 * strength, y + w1 * strength * 0.7, octaves, strength * 0.5)
}

/// 3-D domain warp.
pub fn domain_warp3(pos: Vec3, octaves: u8, strength: f32) -> f32 {
    let qx = perlin3(pos.x, pos.y, pos.z);
    let qy = perlin3(pos.x + 3.7, pos.y + 1.9, pos.z + 7.1);
    let qz = perlin3(pos.x + 8.2, pos.y + 5.4, pos.z + 2.3);
    fbm3(pos.x + strength * qx, pos.y + strength * qy, pos.z + strength * qz,
         octaves, 0.5, 2.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// 1-D helpers
// ─────────────────────────────────────────────────────────────────────────────

/// 1-D value noise (smooth random walk), good for audio modulation.
pub fn noise1(t: f32) -> f32 {
    let i = t.floor() as i64;
    let f = t - t.floor();
    let a = hash_1d(i);
    let b = hash_1d(i + 1);
    lerp(a, b, fade(f))
}

fn hash_1d(i: i64) -> f32 {
    let x = (i ^ (i >> 13)) as u64;
    let x = x.wrapping_mul(0x9e3779b97f4a7c15);
    ((x >> 32) as f32) / u32::MAX as f32
}

// ─────────────────────────────────────────────────────────────────────────────
// Tiled noise
// ─────────────────────────────────────────────────────────────────────────────

/// Tiled 2-D Perlin noise — wraps seamlessly at `(tile_w, tile_h)`.
/// Useful for seamless texture generation.
pub fn tiled_perlin2(x: f32, y: f32, tile_w: u32, tile_h: u32) -> f32 {
    let tw = tile_w as usize;
    let th = tile_h as usize;
    let xi  = x.floor() as i32 as usize;
    let yi  = y.floor() as i32 as usize;
    let xf  = x - x.floor();
    let yf  = y - y.floor();
    let u = fade(xf); let v = fade(yf);
    let aa = p(p(xi     % tw) + yi     % th);
    let ba = p(p((xi+1) % tw) + yi     % th);
    let ab = p(p(xi     % tw) + (yi+1) % th);
    let bb = p(p((xi+1) % tw) + (yi+1) % th);
    lerp(
        lerp(grad2(aa, xf, yf),       grad2(ba, xf-1.0, yf),       u),
        lerp(grad2(ab, xf, yf-1.0),   grad2(bb, xf-1.0, yf-1.0),   u),
        v,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Convenience wrappers
// ─────────────────────────────────────────────────────────────────────────────

/// Remap from Perlin's [-1, 1] to [0, 1].
#[inline] pub fn to_01(n: f32)   -> f32 { n * 0.5 + 0.5 }
/// Remap from [0, 1] to [-1, 1].
#[inline] pub fn from_01(n: f32) -> f32 { n * 2.0 - 1.0 }

/// Sample 3-D Perlin noise at a world position.
pub fn sample3(pos: Vec3, frequency: f32) -> f32 {
    perlin3(pos.x * frequency, pos.y * frequency, pos.z * frequency)
}

/// Sample 3-D fBm at a world position.
pub fn sample_fbm3(pos: Vec3, frequency: f32, octaves: u8) -> f32 {
    fbm3(pos.x*frequency, pos.y*frequency, pos.z*frequency, octaves, 0.5, 2.0)
}

/// Sample curl noise at a 3-D world position.
pub fn sample_curl3(pos: Vec3, frequency: f32) -> Vec3 { curl3(pos, frequency) }

/// Smooth noise at a position with no configuration — sensible defaults.
/// Output in [0, 1].
pub fn quick_noise(x: f32, y: f32) -> f32 {
    to_01(fbm(x, y, 4, 0.5, 2.0))
}

/// Animated noise — samples a moving "slice" through 3-D space.
/// `time` advances the z coordinate, giving smooth noise animation.
pub fn animated_noise(x: f32, y: f32, time: f32, frequency: f32, octaves: u8) -> f32 {
    fbm3(x * frequency, y * frequency, time * 0.1, octaves, 0.5, 2.0)
}

/// Generate a gradient field direction from noise — useful for steering particles.
pub fn gradient_2d(x: f32, y: f32, frequency: f32) -> Vec2 {
    let eps = 0.001f32;
    let f = frequency;
    let dndx = (perlin2((x+eps)*f, y*f) - perlin2((x-eps)*f, y*f)) / (2.0*eps);
    let dndy = (perlin2(x*f, (y+eps)*f) - perlin2(x*f, (y-eps)*f)) / (2.0*eps);
    Vec2::new(dndx, dndy)
}

/// Generate a 3-D gradient vector from noise.
pub fn gradient_3d(pos: Vec3, frequency: f32) -> Vec3 {
    let eps = 0.001f32;
    let f = frequency;
    let (x,y,z) = (pos.x, pos.y, pos.z);
    let gx = (perlin3((x+eps)*f,y*f,z*f) - perlin3((x-eps)*f,y*f,z*f)) / (2.0*eps);
    let gy = (perlin3(x*f,(y+eps)*f,z*f) - perlin3(x*f,(y-eps)*f,z*f)) / (2.0*eps);
    let gz = (perlin3(x*f,y*f,(z+eps)*f) - perlin3(x*f,y*f,(z-eps)*f)) / (2.0*eps);
    Vec3::new(gx, gy, gz)
}
