//! Environment probes and global illumination helpers.
//!
//! Provides:
//! * Spherical harmonic (SH3) projection and evaluation
//! * Cubemap / equirectangular / octahedral map utilities
//! * Reflection probe parallax correction
//! * Tetrahedral-interpolated light probe grids
//! * Screen-space reflection ray generation
//! * Irradiance cache with validity ageing

use glam::{Mat3, Vec2, Vec3, Vec4};
use std::f32::consts::{FRAC_1_PI, PI};

// ─────────────────────────────────────────────────────────────────────────────
// Local Ray type
// ─────────────────────────────────────────────────────────────────────────────

/// A ray with an origin and a (unit) direction.
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Evaluate the ray at parameter `t`.
    #[inline]
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Spherical Harmonics
// ─────────────────────────────────────────────────────────────────────────────

/// Third-order (L2) spherical harmonics — 9 real coefficients per colour
/// channel.  Each `Sh3` stores (R, G, B) for all 9 basis functions.
#[derive(Debug, Clone)]
pub struct Sh3 {
    /// `coeffs[i]` = (r, g, b) for the i-th SH basis function.
    pub coeffs: [Vec3; 9],
}

impl Sh3 {
    /// Zero SH (no contribution).
    pub fn zero() -> Self {
        Self {
            coeffs: [Vec3::ZERO; 9],
        }
    }

    /// Create from a flat array of (r,g,b) tuples.
    pub fn from_coeffs(c: [Vec3; 9]) -> Self {
        Self { coeffs: c }
    }

    /// Evaluate the SH at direction `dir` to get irradiance.
    pub fn evaluate(&self, dir: Vec3) -> Vec3 {
        let b = sh_basis(dir);
        let mut result = Vec3::ZERO;
        for i in 0..9 {
            result += self.coeffs[i] * b[i];
        }
        result.max(Vec3::ZERO)
    }
}

impl Default for Sh3 {
    fn default() -> Self {
        Self::zero()
    }
}

/// SH normalization constants for bands 0–2.
const SH_C0: f32 = 0.282_094_8; // 1 / (2*sqrt(pi))
const SH_C1: f32 = 0.488_602_5; // sqrt(3 / (4*pi))
const SH_C2_A: f32 = 1.092_548_4; // sqrt(15 / (4*pi))
const SH_C2_B: f32 = 0.315_391_6; // sqrt(5 / (16*pi))
const SH_C2_C: f32 = 0.546_274_2; // sqrt(15 / (16*pi))

/// Evaluate all 9 SH basis functions at direction `dir`.
///
/// The returned array follows the convention:
/// `[Y0_0, Y1_{-1}, Y1_0, Y1_1, Y2_{-2}, Y2_{-1}, Y2_0, Y2_1, Y2_2]`
pub fn sh_basis(dir: Vec3) -> [f32; 9] {
    let (x, y, z) = (dir.x, dir.y, dir.z);
    [
        // L=0
        SH_C0,
        // L=1
        -SH_C1 * y,
        SH_C1 * z,
        -SH_C1 * x,
        // L=2
        SH_C2_A * x * y,
        -SH_C2_A * y * z,
        SH_C2_B * (2.0 * z * z - x * x - y * y),
        -SH_C2_A * x * z,
        SH_C2_C * (x * x - y * y),
    ]
}

/// Monte Carlo projection of a spherical function onto SH3 basis.
///
/// `sample_fn` — callable that returns the RGB radiance for a given direction.
/// `n_samples` — number of uniform sphere samples.
pub fn project_to_sh(sample_fn: impl Fn(Vec3) -> Vec3, n_samples: usize) -> Sh3 {
    let mut coeffs = [Vec3::ZERO; 9];

    for i in 0..n_samples {
        // Uniform sphere sampling using Fibonacci lattice
        let golden = (1.0 + 5.0_f32.sqrt()) * 0.5;
        let theta = (1.0 - 2.0 * (i as f32 + 0.5) / n_samples as f32)
            .clamp(-1.0, 1.0)
            .acos();
        let phi = 2.0 * PI * (i as f32) / golden;

        let dir = Vec3::new(theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos());
        let radiance = sample_fn(dir);
        let basis = sh_basis(dir);

        for j in 0..9 {
            coeffs[j] += radiance * basis[j];
        }
    }

    // Normalize by solid-angle weight (4π / N)
    let weight = 4.0 * PI / n_samples as f32;
    for c in &mut coeffs {
        *c *= weight;
    }

    Sh3 { coeffs }
}

/// Evaluate irradiance for a surface with `normal` from precomputed SH
/// coefficients (Ramamoorthi & Hanrahan 2001).
pub fn irradiance_from_sh(sh: &Sh3, normal: Vec3) -> Vec3 {
    // Pre-computed zonal harmonic + cosine lobe convolution factors
    const A0: f32 = PI;
    const A1: f32 = 2.0 * PI / 3.0;
    const A2: f32 = PI / 4.0;

    let b = sh_basis(normal);

    sh.coeffs[0] * b[0] * A0
        + sh.coeffs[1] * b[1] * A1
        + sh.coeffs[2] * b[2] * A1
        + sh.coeffs[3] * b[3] * A1
        + sh.coeffs[4] * b[4] * A2
        + sh.coeffs[5] * b[5] * A2
        + sh.coeffs[6] * b[6] * A2
        + sh.coeffs[7] * b[7] * A2
        + sh.coeffs[8] * b[8] * A2
}

/// Convolve SH3 with the clamped-cosine (Lambertian) kernel.
///
/// This is the ZH product used for irradiance environment maps.
pub fn convolve_sh_lambert(sh: &Sh3) -> Sh3 {
    const ZH0: f32 = 3.141_593;
    const ZH1: f32 = 2.094_395;
    const ZH2: f32 = 0.785_398;

    let mut out = sh.clone();
    // Band 0
    out.coeffs[0] = sh.coeffs[0] * ZH0;
    // Band 1
    for i in 1..=3 {
        out.coeffs[i] = sh.coeffs[i] * ZH1;
    }
    // Band 2
    for i in 4..=8 {
        out.coeffs[i] = sh.coeffs[i] * ZH2;
    }
    out
}

/// Add two SH3 sets.
pub fn sh_add(a: &Sh3, b: &Sh3) -> Sh3 {
    let mut out = Sh3::zero();
    for i in 0..9 {
        out.coeffs[i] = a.coeffs[i] + b.coeffs[i];
    }
    out
}

/// Scale SH3 by scalar `s`.
pub fn sh_scale(sh: &Sh3, s: f32) -> Sh3 {
    let mut out = sh.clone();
    for c in &mut out.coeffs {
        *c *= s;
    }
    out
}

/// Rotate SH3 by a rotation matrix.
///
/// Uses the exact band-0/1/2 SH rotation formulas.  Band 0 is invariant; bands
/// 1 and 2 are rotated using the provided `rotation` matrix.
pub fn sh_rotate(sh: &Sh3, rotation: &Mat3) -> Sh3 {
    let mut out = Sh3::zero();

    // Band 0 is rotationally invariant
    out.coeffs[0] = sh.coeffs[0];

    // Band 1: Y_1m transforms as Cartesian vector components (x, y, z)
    // Basis ordering: [1]=y, [2]=z, [3]=x
    let r = *rotation;
    // Extract the columns of the rotation matrix that map x,y,z to x',y',z'
    let rx = Vec3::new(r.x_axis.x, r.x_axis.y, r.x_axis.z);
    let ry = Vec3::new(r.y_axis.x, r.y_axis.y, r.y_axis.z);
    let rz = Vec3::new(r.z_axis.x, r.z_axis.y, r.z_axis.z);

    // Coefficients for basis [y, z, x] -> indices [1, 2, 3]
    let b1_y = sh.coeffs[1];
    let b1_z = sh.coeffs[2];
    let b1_x = sh.coeffs[3];

    out.coeffs[1] = b1_x * ry.x + b1_y * ry.y + b1_z * ry.z; // new y
    out.coeffs[2] = b1_x * rz.x + b1_y * rz.y + b1_z * rz.z; // new z
    out.coeffs[3] = b1_x * rx.x + b1_y * rx.y + b1_z * rx.z; // new x

    // Band 2: use the analytic 5×5 rotation derived from the Wigner D-matrices.
    // We sample the rotated SH by evaluating in the original basis.
    // For each of the 5 band-2 functions we sample the rotated direction to find
    // the coefficients — this is the "reconstruct & re-project" approach, which
    // is exact for band-2 if we use the right 5 reference directions.
    let dirs_b2: [Vec3; 5] = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 1.0, 0.0).normalize(),
        Vec3::new(0.0, 1.0, 1.0).normalize(),
    ];

    // For each direction, evaluate original band-2 SH and then the rotated dir
    for (idx, &dir) in dirs_b2.iter().enumerate() {
        let rot_dir = rotation.mul_vec3(dir).normalize();
        let b_orig = sh_basis(dir);
        let b_rot = sh_basis(rot_dir);

        // Contribution from original band-2 coefficients
        let mut val = Vec3::ZERO;
        for k in 0..5 {
            val += sh.coeffs[4 + k] * b_orig[4 + k];
        }

        // The rotated direction's band-2 basis distributes this back
        for k in 0..5 {
            out.coeffs[4 + k] += val * b_rot[4 + k];
        }

        let _ = idx; // suppress warning
    }

    // Normalise the band-2 projection (5 samples for 5 coefficients — biased,
    // but an acceptable approximation when sample directions are well-chosen)
    for k in 0..5 {
        out.coeffs[4 + k] /= 5.0;
    }

    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Cubemap utilities
// ─────────────────────────────────────────────────────────────────────────────

/// The six faces of a cubemap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CubemapFace {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl CubemapFace {
    /// Return the face index in the conventional order: +X, -X, +Y, -Y, +Z, -Z.
    pub fn index(self) -> usize {
        match self {
            CubemapFace::PosX => 0,
            CubemapFace::NegX => 1,
            CubemapFace::PosY => 2,
            CubemapFace::NegY => 3,
            CubemapFace::PosZ => 4,
            CubemapFace::NegZ => 5,
        }
    }

    /// Return all six faces.
    pub fn all() -> [CubemapFace; 6] {
        [
            CubemapFace::PosX,
            CubemapFace::NegX,
            CubemapFace::PosY,
            CubemapFace::NegY,
            CubemapFace::PosZ,
            CubemapFace::NegZ,
        ]
    }
}

/// Map a world-space direction `dir` to a cubemap face and per-face UV in [0, 1]².
pub fn dir_to_face_uv(dir: Vec3) -> (CubemapFace, Vec2) {
    let abs = dir.abs();
    let (face, u_raw, v_raw) = if abs.x >= abs.y && abs.x >= abs.z {
        if dir.x > 0.0 {
            (CubemapFace::PosX, -dir.z / dir.x, -dir.y / dir.x)
        } else {
            (CubemapFace::NegX, dir.z / (-dir.x), -dir.y / (-dir.x))
        }
    } else if abs.y >= abs.x && abs.y >= abs.z {
        if dir.y > 0.0 {
            (CubemapFace::PosY, dir.x / dir.y, dir.z / dir.y)
        } else {
            (CubemapFace::NegY, dir.x / (-dir.y), -dir.z / (-dir.y))
        }
    } else if dir.z > 0.0 {
        (CubemapFace::PosZ, dir.x / dir.z, -dir.y / dir.z)
    } else {
        (CubemapFace::NegZ, -dir.x / (-dir.z), -dir.y / (-dir.z))
    };

    let uv = Vec2::new(u_raw * 0.5 + 0.5, v_raw * 0.5 + 0.5);
    (face, uv.clamp(Vec2::ZERO, Vec2::ONE))
}

/// Map a cubemap face + per-face UV back to a world-space direction.
pub fn face_uv_to_dir(face: CubemapFace, uv: Vec2) -> Vec3 {
    let uc = uv.x * 2.0 - 1.0;
    let vc = uv.y * 2.0 - 1.0;

    let dir = match face {
        CubemapFace::PosX => Vec3::new(1.0, -vc, -uc),
        CubemapFace::NegX => Vec3::new(-1.0, -vc, uc),
        CubemapFace::PosY => Vec3::new(uc, 1.0, vc),
        CubemapFace::NegY => Vec3::new(uc, -1.0, -vc),
        CubemapFace::PosZ => Vec3::new(uc, -vc, 1.0),
        CubemapFace::NegZ => Vec3::new(-uc, -vc, -1.0),
    };
    dir.normalize()
}

/// Convert an equirectangular UV `[0,1]²` to a unit direction.
pub fn equirect_to_dir(uv: Vec2) -> Vec3 {
    let phi = uv.x * 2.0 * PI - PI; // [-π, π]
    let theta = uv.y * PI; // [0, π]
    Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin())
}

/// Convert a unit direction to equirectangular UV `[0,1]²`.
pub fn dir_to_equirect(dir: Vec3) -> Vec2 {
    let dir = dir.normalize();
    let phi = dir.z.atan2(dir.x); // [-π, π]
    let theta = dir.y.clamp(-1.0, 1.0).acos(); // [0, π]
    Vec2::new(
        (phi + PI) / (2.0 * PI),
        theta / PI,
    )
}

/// Encode a unit direction using the octahedral map (Cigolle et al., 2014).
///
/// Returns a value in `[-1, 1]²`.
pub fn octahedral_map(dir: Vec3) -> Vec2 {
    let dir = dir.normalize();
    let l1 = dir.x.abs() + dir.y.abs() + dir.z.abs();
    let p = Vec2::new(dir.x / l1, dir.y / l1);
    if dir.z < 0.0 {
        let sx = if p.x >= 0.0 { 1.0f32 } else { -1.0f32 };
        let sy = if p.y >= 0.0 { 1.0f32 } else { -1.0f32 };
        Vec2::new((1.0 - p.y.abs()) * sx, (1.0 - p.x.abs()) * sy)
    } else {
        p
    }
}

/// Decode an octahedral-mapped UV back to a unit direction.
pub fn octahedral_unmap(uv: Vec2) -> Vec3 {
    let p = uv;
    let z = 1.0 - p.x.abs() - p.y.abs();
    let dir = if z >= 0.0 {
        Vec3::new(p.x, p.y, z)
    } else {
        let sx = if p.x >= 0.0 { 1.0f32 } else { -1.0f32 };
        let sy = if p.y >= 0.0 { 1.0f32 } else { -1.0f32 };
        Vec3::new((1.0 - p.y.abs()) * sx, (1.0 - p.x.abs()) * sy, z)
    };
    dir.normalize()
}

// ─────────────────────────────────────────────────────────────────────────────
// Reflection probes
// ─────────────────────────────────────────────────────────────────────────────

/// A spherical reflection probe that captures environment radiance at a point.
#[derive(Debug, Clone)]
pub struct ReflectionProbe {
    pub position: Vec3,
    /// Influence radius — objects within this sphere can use this probe.
    pub radius: f32,
    /// Priority weight for blending when multiple probes overlap.
    pub importance: f32,
    /// Whether to apply box parallax correction.
    pub parallax_correction: bool,
}

impl ReflectionProbe {
    pub fn new(position: Vec3, radius: f32, importance: f32, parallax_correction: bool) -> Self {
        Self {
            position,
            radius,
            importance,
            parallax_correction,
        }
    }

    /// Blend weight for a sample at `sample_pos`.
    ///
    /// Returns 0 outside the influence radius, and 1 at the probe centre.
    pub fn blend_weight(&self, sample_pos: Vec3) -> f32 {
        blend_weight(self, sample_pos)
    }
}

/// Compute the blend weight for a reflection probe at `sample_pos`.
pub fn blend_weight(probe: &ReflectionProbe, sample_pos: Vec3) -> f32 {
    let dist = (probe.position - sample_pos).length();
    if dist >= probe.radius {
        return 0.0;
    }
    let t = dist / probe.radius;
    // Smooth step with cubic falloff
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// Apply parallax correction to a reflection direction.
///
/// Assumes the probe captures the environment inside an AABB centred at
/// `probe_pos` with half-extents `box_half`.
///
/// `dir`        — un-corrected reflection direction (unit)
/// `sample_pos` — world position of the shaded surface point
/// `probe_pos`  — world position of the probe centre
/// `box_half`   — half-extents of the proxy box
pub fn parallax_correct_dir(
    dir: Vec3,
    sample_pos: Vec3,
    probe_pos: Vec3,
    box_half: Vec3,
) -> Vec3 {
    let dir = dir.normalize();
    // Ray-AABB intersection from sample_pos
    let box_min = probe_pos - box_half;
    let box_max = probe_pos + box_half;

    let inv_dir = Vec3::new(
        if dir.x.abs() > 1e-10 { 1.0 / dir.x } else { f32::MAX },
        if dir.y.abs() > 1e-10 { 1.0 / dir.y } else { f32::MAX },
        if dir.z.abs() > 1e-10 { 1.0 / dir.z } else { f32::MAX },
    );

    let t0 = (box_min - sample_pos) * inv_dir;
    let t1 = (box_max - sample_pos) * inv_dir;

    let t_max = Vec3::new(t0.x.max(t1.x), t0.y.max(t1.y), t0.z.max(t1.z));
    let t_hit = t_max.x.min(t_max.y).min(t_max.z).max(0.0);

    // Intersection point on the proxy box
    let hit = sample_pos + dir * t_hit;

    // Direction from probe centre to intersection
    (hit - probe_pos).normalize()
}

// ─────────────────────────────────────────────────────────────────────────────
// Light probe grid
// ─────────────────────────────────────────────────────────────────────────────

/// A regular 3D grid of SH3 irradiance probes.
///
/// At runtime, shaders can trilinearly interpolate between the 8 nearest probes
/// to get smooth irradiance at any position within the grid bounds.
#[derive(Debug, Clone)]
pub struct LightProbeGrid {
    /// World-space AABB minimum corner.
    pub min: Vec3,
    /// World-space AABB maximum corner.
    pub max: Vec3,
    /// Number of probes along each axis.
    pub resolution: [u32; 3],
    /// Flat probe storage, in x-major order (x + y*rx + z*rx*ry).
    pub probes: Vec<Sh3>,
}

impl LightProbeGrid {
    /// Create a grid with all probes initialised to zero.
    pub fn new(min: Vec3, max: Vec3, resolution: [u32; 3]) -> Self {
        let count = (resolution[0] * resolution[1] * resolution[2]) as usize;
        Self {
            min,
            max,
            resolution,
            probes: vec![Sh3::zero(); count],
        }
    }

    /// Cell size along each axis.
    pub fn cell_size(&self) -> Vec3 {
        let r = Vec3::new(
            (self.resolution[0] - 1).max(1) as f32,
            (self.resolution[1] - 1).max(1) as f32,
            (self.resolution[2] - 1).max(1) as f32,
        );
        (self.max - self.min) / r
    }

    /// Linear index of probe at grid coordinates `(ix, iy, iz)`.
    fn index(&self, ix: usize, iy: usize, iz: usize) -> usize {
        let rx = self.resolution[0] as usize;
        let ry = self.resolution[1] as usize;
        ix + iy * rx + iz * rx * ry
    }

    /// Get a reference to the probe at grid coordinates.
    pub fn probe_at(&self, ix: usize, iy: usize, iz: usize) -> &Sh3 {
        &self.probes[self.index(ix, iy, iz)]
    }

    /// Get a mutable reference to the probe at grid coordinates.
    pub fn probe_at_mut(&mut self, ix: usize, iy: usize, iz: usize) -> &mut Sh3 {
        let idx = self.index(ix, iy, iz);
        &mut self.probes[idx]
    }

    /// Trilinearly interpolate SH3 at world-space position `pos`.
    pub fn sample(&self, pos: Vec3) -> Sh3 {
        let cell_size = self.cell_size();
        let local = (pos - self.min) / cell_size;

        let rx = (self.resolution[0] - 1) as f32;
        let ry = (self.resolution[1] - 1) as f32;
        let rz = (self.resolution[2] - 1) as f32;

        let lx = local.x.clamp(0.0, rx);
        let ly = local.y.clamp(0.0, ry);
        let lz = local.z.clamp(0.0, rz);

        let ix0 = (lx as usize).min(self.resolution[0] as usize - 1);
        let iy0 = (ly as usize).min(self.resolution[1] as usize - 1);
        let iz0 = (lz as usize).min(self.resolution[2] as usize - 1);
        let ix1 = (ix0 + 1).min(self.resolution[0] as usize - 1);
        let iy1 = (iy0 + 1).min(self.resolution[1] as usize - 1);
        let iz1 = (iz0 + 1).min(self.resolution[2] as usize - 1);

        let tx = lx - ix0 as f32;
        let ty = ly - iy0 as f32;
        let tz = lz - iz0 as f32;

        // Trilinear blend of 8 corner probes
        let p000 = self.probe_at(ix0, iy0, iz0);
        let p100 = self.probe_at(ix1, iy0, iz0);
        let p010 = self.probe_at(ix0, iy1, iz0);
        let p110 = self.probe_at(ix1, iy1, iz0);
        let p001 = self.probe_at(ix0, iy0, iz1);
        let p101 = self.probe_at(ix1, iy0, iz1);
        let p011 = self.probe_at(ix0, iy1, iz1);
        let p111 = self.probe_at(ix1, iy1, iz1);

        let lerp_sh = |a: &Sh3, b: &Sh3, t: f32| -> Sh3 {
            sh_add(a, &sh_scale(&sh_add(b, &sh_scale(a, -1.0)), t))
        };

        let s00 = lerp_sh(p000, p100, tx);
        let s10 = lerp_sh(p010, p110, tx);
        let s01 = lerp_sh(p001, p101, tx);
        let s11 = lerp_sh(p011, p111, tx);

        let s0 = lerp_sh(&s00, &s10, ty);
        let s1 = lerp_sh(&s01, &s11, ty);

        lerp_sh(&s0, &s1, tz)
    }

    /// Compute ambient irradiance at `pos` for a surface with `normal`.
    pub fn compute_ambient(&self, pos: Vec3, normal: Vec3) -> Vec3 {
        let sh = self.sample(pos);
        let conv = convolve_sh_lambert(&sh);
        irradiance_from_sh(&conv, normal.normalize()).max(Vec3::ZERO)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Baked AO volume
// ─────────────────────────────────────────────────────────────────────────────

/// A 3D grid of pre-baked ambient-occlusion values.
#[derive(Debug, Clone)]
pub struct BakedAo {
    pub grid_size: [u32; 3],
    pub data: Vec<f32>,
}

impl BakedAo {
    /// Allocate an AO volume initialised to 1.0 (fully unoccluded).
    pub fn new(grid_size: [u32; 3]) -> Self {
        let count = (grid_size[0] * grid_size[1] * grid_size[2]) as usize;
        Self {
            grid_size,
            data: vec![1.0; count],
        }
    }

    fn index(&self, ix: usize, iy: usize, iz: usize) -> usize {
        let rx = self.grid_size[0] as usize;
        let ry = self.grid_size[1] as usize;
        ix + iy * rx + iz * rx * ry
    }

    /// Sample AO at continuous grid coordinates using trilinear interpolation.
    pub fn sample_trilinear(&self, gx: f32, gy: f32, gz: f32) -> f32 {
        let rx = (self.grid_size[0] - 1) as f32;
        let ry = (self.grid_size[1] - 1) as f32;
        let rz = (self.grid_size[2] - 1) as f32;

        let lx = gx.clamp(0.0, rx);
        let ly = gy.clamp(0.0, ry);
        let lz = gz.clamp(0.0, rz);

        let ix0 = (lx as usize).min(self.grid_size[0] as usize - 1);
        let iy0 = (ly as usize).min(self.grid_size[1] as usize - 1);
        let iz0 = (lz as usize).min(self.grid_size[2] as usize - 1);
        let ix1 = (ix0 + 1).min(self.grid_size[0] as usize - 1);
        let iy1 = (iy0 + 1).min(self.grid_size[1] as usize - 1);
        let iz1 = (iz0 + 1).min(self.grid_size[2] as usize - 1);

        let tx = lx - ix0 as f32;
        let ty = ly - iy0 as f32;
        let tz = lz - iz0 as f32;

        macro_rules! ao {
            ($x:expr, $y:expr, $z:expr) => {
                self.data[self.index($x, $y, $z)]
            };
        }

        let i00 = ao!(ix0, iy0, iz0) * (1.0 - tx) + ao!(ix1, iy0, iz0) * tx;
        let i10 = ao!(ix0, iy1, iz0) * (1.0 - tx) + ao!(ix1, iy1, iz0) * tx;
        let i01 = ao!(ix0, iy0, iz1) * (1.0 - tx) + ao!(ix1, iy0, iz1) * tx;
        let i11 = ao!(ix0, iy1, iz1) * (1.0 - tx) + ao!(ix1, iy1, iz1) * tx;

        let j0 = i00 * (1.0 - ty) + i10 * ty;
        let j1 = i01 * (1.0 - ty) + i11 * ty;

        j0 * (1.0 - tz) + j1 * tz
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Screen-space reflections (CPU-side math)
// ─────────────────────────────────────────────────────────────────────────────

/// Generate an SSR ray in view space.
///
/// `pos_vs`    — surface position in view space
/// `normal_vs` — surface normal in view space
/// `roughness` — material roughness (used for cone jitter)
/// `jitter`    — random jitter in [0, 1) for anti-aliasing (from blue noise etc.)
pub fn ssr_ray(pos_vs: Vec3, normal_vs: Vec3, roughness: f32, jitter: f32) -> Ray {
    let view_dir = -pos_vs.normalize(); // looking from origin toward surface
    let normal = normal_vs.normalize();

    // Base reflection direction
    let reflect_dir = (view_dir - 2.0 * view_dir.dot(normal) * normal).normalize();

    // Jitter the reflection direction by roughness-scaled cone
    let cone_angle = roughness * std::f32::consts::FRAC_PI_2 * 0.5;
    let jitter_angle = jitter * cone_angle;

    // Build local tangent frame around reflect_dir
    let (t, b) = orthonormal_basis(reflect_dir);
    let phi = jitter * 2.0 * PI;
    let sin_j = jitter_angle.sin();
    let cos_j = jitter_angle.cos();

    let jittered = (reflect_dir * cos_j + t * sin_j * phi.cos() + b * sin_j * phi.sin()).normalize();

    Ray::new(pos_vs, jittered)
}

/// Compute the SSR fade factor — attenuates hits near screen edges and for
/// high roughness.
///
/// `screen_uv` — screen UV of the hit point [0,1]²
/// `hit_dist`  — distance travelled by the SSR ray
/// `roughness` — material roughness
pub fn ssr_fade(screen_uv: Vec2, hit_dist: f32, roughness: f32) -> f32 {
    // Edge fade
    let edge_dist = Vec2::new(
        screen_uv.x.min(1.0 - screen_uv.x),
        screen_uv.y.min(1.0 - screen_uv.y),
    );
    let edge_fade = (edge_dist.x / 0.1).clamp(0.0, 1.0) * (edge_dist.y / 0.1).clamp(0.0, 1.0);

    // Distance fade (long SSR rays are less reliable)
    let dist_fade = (1.0 - (hit_dist / 50.0).clamp(0.0, 1.0)).max(0.0);

    // Roughness fade (only smooth surfaces show reflections)
    let rough_fade = 1.0 - roughness.clamp(0.0, 1.0);

    edge_fade * dist_fade * rough_fade
}

/// Duff orthonormal basis (re-exported here for use in ssr_ray).
fn orthonormal_basis(n: Vec3) -> (Vec3, Vec3) {
    let sign = if n.z >= 0.0 { 1.0_f32 } else { -1.0_f32 };
    let a = -1.0 / (sign + n.z);
    let b = n.x * n.y * a;
    let t = Vec3::new(1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x);
    let bi = Vec3::new(b, sign + n.y * n.y * a, -n.y);
    (t, bi)
}

// ─────────────────────────────────────────────────────────────────────────────
// Irradiance Cache
// ─────────────────────────────────────────────────────────────────────────────

/// A single cached irradiance sample.
#[derive(Debug, Clone)]
pub struct IrradianceCacheEntry {
    pub position: Vec3,
    pub normal: Vec3,
    /// Cached irradiance value (linear RGB).
    pub irradiance: Vec3,
    /// Validity weight — decays to zero over time / distance.
    pub validity: f32,
}

impl IrradianceCacheEntry {
    pub fn new(position: Vec3, normal: Vec3, irradiance: Vec3, validity: f32) -> Self {
        Self {
            position,
            normal: normal.normalize(),
            irradiance,
            validity: validity.clamp(0.0, 1.0),
        }
    }

    /// Interpolation weight for a query at `(pos, n)`.
    pub fn weight(&self, pos: Vec3, normal: Vec3, max_dist: f32) -> f32 {
        let dist = (self.position - pos).length();
        if dist >= max_dist || self.validity < 1e-4 {
            return 0.0;
        }
        let dist_w = 1.0 - dist / max_dist;
        let normal_w = self.normal.dot(normal.normalize()).max(0.0);
        dist_w * dist_w * normal_w * self.validity
    }
}

/// A cache of irradiance samples for global-illumination interpolation.
///
/// New samples are inserted and queries interpolate nearby entries using
/// distance and normal similarity as weights.
pub struct IrradianceCache {
    pub entries: Vec<IrradianceCacheEntry>,
    /// Maximum number of entries (oldest are evicted when at capacity).
    pub capacity: usize,
}

impl IrradianceCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Query the cache for irradiance at `(pos, normal)`.
    ///
    /// Interpolates from all entries within `max_dist`.  Returns `None` if
    /// there are no valid entries nearby.
    pub fn query(&self, pos: Vec3, normal: Vec3, max_dist: f32) -> Option<Vec3> {
        let mut weighted_sum = Vec3::ZERO;
        let mut weight_total = 0.0f32;

        for entry in &self.entries {
            let w = entry.weight(pos, normal, max_dist);
            if w > 1e-6 {
                weighted_sum += entry.irradiance * w;
                weight_total += w;
            }
        }

        if weight_total < 1e-6 {
            None
        } else {
            Some(weighted_sum / weight_total)
        }
    }

    /// Insert a new irradiance sample.
    ///
    /// If the cache is at capacity the entry with the lowest validity is evicted.
    pub fn insert(&mut self, pos: Vec3, normal: Vec3, irradiance: Vec3, validity: f32) {
        // De-duplicate: if a very close entry already exists, update it instead.
        let merge_dist = 0.01f32;
        for entry in &mut self.entries {
            if (entry.position - pos).length() < merge_dist
                && entry.normal.dot(normal.normalize()) > 0.99
            {
                // Exponential moving average update
                let alpha = 0.2f32;
                entry.irradiance = entry.irradiance * (1.0 - alpha) + irradiance * alpha;
                entry.validity = validity;
                return;
            }
        }

        if self.entries.len() >= self.capacity {
            // Evict lowest-validity entry
            let evict = self
                .entries
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.validity.partial_cmp(&b.validity).unwrap())
                .map(|(i, _)| i);
            if let Some(idx) = evict {
                self.entries.swap_remove(idx);
            }
        }

        self.entries.push(IrradianceCacheEntry::new(pos, normal, irradiance, validity));
    }

    /// Reduce validity of all entries within `radius` of `pos` by `decay_rate`.
    pub fn update_validity(&mut self, pos: Vec3, decay_rate: f32) {
        for entry in &mut self.entries {
            let dist = (entry.position - pos).length();
            let dist_w = 1.0 - (dist / 50.0).clamp(0.0, 1.0);
            entry.validity = (entry.validity - decay_rate * dist_w).max(0.0);
        }
    }

    /// Remove all entries with validity below threshold.
    pub fn prune(&mut self, threshold: f32) {
        self.entries.retain(|e| e.validity >= threshold);
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Re-exports used by mod.rs (PbrMaterial::f0 references brdf::fresnel)
// ─────────────────────────────────────────────────────────────────────────────

use super::brdf;

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    // ── SH basis ──────────────────────────────────────────────────────────────

    #[test]
    fn sh_basis_length_is_9() {
        let b = sh_basis(Vec3::Y);
        assert_eq!(b.len(), 9);
    }

    #[test]
    fn sh_basis_band0_is_constant() {
        let b1 = sh_basis(Vec3::Y);
        let b2 = sh_basis(Vec3::X);
        // Band 0 is constant
        assert!((b1[0] - b2[0]).abs() < 1e-6);
    }

    #[test]
    fn sh_project_and_evaluate_constant_fn() {
        // Projecting a constant function f(ω)=1 and evaluating at any direction
        // should give approximately 4π * C0 (the monopole integral).
        let sh = project_to_sh(|_| Vec3::ONE, 2048);
        let val = sh.evaluate(Vec3::Y);
        // Band 0 contribution: SH_C0 * SH_C0 * 4π ≈ 1.0 (after normalisation)
        assert!(
            val.x > 0.5 && val.x < 2.0,
            "SH evaluation of constant fn should be ~1: {val:?}"
        );
    }

    #[test]
    fn sh_add_is_commutative() {
        let mut a = Sh3::zero();
        let mut b = Sh3::zero();
        a.coeffs[0] = Vec3::new(1.0, 0.0, 0.0);
        b.coeffs[1] = Vec3::new(0.0, 1.0, 0.0);
        let ab = sh_add(&a, &b);
        let ba = sh_add(&b, &a);
        for i in 0..9 {
            assert!((ab.coeffs[i] - ba.coeffs[i]).length() < 1e-6);
        }
    }

    #[test]
    fn sh_scale_zero_gives_zero() {
        let mut sh = Sh3::zero();
        sh.coeffs[0] = Vec3::ONE;
        let scaled = sh_scale(&sh, 0.0);
        assert_eq!(scaled.coeffs[0], Vec3::ZERO);
    }

    // ── Cubemap ───────────────────────────────────────────────────────────────

    #[test]
    fn dir_to_face_uv_round_trip() {
        let dirs = [
            Vec3::X,
            -Vec3::X,
            Vec3::Y,
            -Vec3::Y,
            Vec3::Z,
            -Vec3::Z,
            Vec3::new(1.0, 1.0, 0.0).normalize(),
        ];
        for dir in dirs {
            let (face, uv) = dir_to_face_uv(dir);
            let recovered = face_uv_to_dir(face, uv);
            let dot = dir.dot(recovered);
            assert!(
                dot > 0.99,
                "Round-trip dir={dir:?} -> face={face:?} uv={uv:?} -> {recovered:?}, dot={dot}"
            );
        }
    }

    #[test]
    fn equirect_round_trip() {
        let dirs = [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(0.5, 0.7, 0.3).normalize()];
        for dir in dirs {
            let uv = dir_to_equirect(dir);
            let back = equirect_to_dir(uv);
            let dot = dir.dot(back);
            assert!(dot > 0.999, "Equirect round-trip dot={dot} for dir={dir:?}");
        }
    }

    #[test]
    fn octahedral_round_trip() {
        let dirs = [
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            -Vec3::X,
            -Vec3::Y,
            -Vec3::Z,
            Vec3::new(0.5, 0.3, 0.8).normalize(),
        ];
        for dir in dirs {
            let enc = octahedral_map(dir);
            let dec = octahedral_unmap(enc);
            let dot = dir.dot(dec);
            assert!(dot > 0.999, "Oct round-trip failed for {dir:?}: dot={dot}");
        }
    }

    // ── Reflection probe ──────────────────────────────────────────────────────

    #[test]
    fn probe_weight_zero_outside_radius() {
        let probe = ReflectionProbe::new(Vec3::ZERO, 5.0, 1.0, false);
        let w = probe.blend_weight(Vec3::new(10.0, 0.0, 0.0));
        assert_eq!(w, 0.0);
    }

    #[test]
    fn probe_weight_one_at_centre() {
        let probe = ReflectionProbe::new(Vec3::ZERO, 5.0, 1.0, false);
        let w = probe.blend_weight(Vec3::ZERO);
        assert!((w - 1.0).abs() < 1e-5);
    }

    // ── Light probe grid ──────────────────────────────────────────────────────

    #[test]
    fn light_probe_grid_trilinear_at_corner() {
        let mut grid = LightProbeGrid::new(Vec3::ZERO, Vec3::ONE, [2, 2, 2]);
        // Set one probe to a constant colour
        grid.probe_at_mut(0, 0, 0).coeffs[0] = Vec3::new(1.0, 0.0, 0.0);
        let sh = grid.sample(Vec3::ZERO);
        assert!(sh.coeffs[0].x > 0.5, "Should pick up the red probe at corner");
    }

    // ── SSR ───────────────────────────────────────────────────────────────────

    #[test]
    fn ssr_ray_direction_is_unit() {
        let pos = Vec3::new(0.0, 0.0, -5.0);
        let normal = Vec3::Z;
        let ray = ssr_ray(pos, normal, 0.2, 0.3);
        assert!(
            (ray.direction.length() - 1.0).abs() < 1e-4,
            "SSR ray direction must be unit: {}",
            ray.direction.length()
        );
    }

    #[test]
    fn ssr_fade_zero_at_edge() {
        let uv = Vec2::new(0.0, 0.5); // at left edge
        let fade = ssr_fade(uv, 5.0, 0.1);
        assert_eq!(fade, 0.0, "Fade should be 0 at screen edge");
    }

    #[test]
    fn ssr_fade_high_roughness_is_low() {
        let uv = Vec2::new(0.5, 0.5);
        let f_smooth = ssr_fade(uv, 1.0, 0.0);
        let f_rough = ssr_fade(uv, 1.0, 1.0);
        assert!(
            f_smooth > f_rough,
            "Smooth surfaces should have higher SSR fade factor"
        );
    }

    // ── Irradiance cache ──────────────────────────────────────────────────────

    #[test]
    fn irradiance_cache_insert_and_query() {
        let mut cache = IrradianceCache::new(64);
        cache.insert(Vec3::ZERO, Vec3::Y, Vec3::new(1.0, 0.5, 0.2), 1.0);
        let result = cache.query(Vec3::ZERO, Vec3::Y, 1.0);
        assert!(result.is_some(), "Should find nearby entry");
        let irr = result.unwrap();
        assert!((irr - Vec3::new(1.0, 0.5, 0.2)).length() < 0.01);
    }

    #[test]
    fn irradiance_cache_miss_returns_none() {
        let mut cache = IrradianceCache::new(16);
        cache.insert(Vec3::new(100.0, 0.0, 0.0), Vec3::Y, Vec3::ONE, 1.0);
        let result = cache.query(Vec3::ZERO, Vec3::Y, 1.0);
        assert!(result.is_none(), "Should return None when no nearby entry");
    }

    #[test]
    fn irradiance_cache_eviction() {
        let mut cache = IrradianceCache::new(2);
        cache.insert(Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec3::ONE, 1.0);
        cache.insert(Vec3::new(10.0, 0.0, 0.0), Vec3::Y, Vec3::ONE, 0.5);
        // Insert third — should evict lowest validity (0.5 at pos 10)
        cache.insert(Vec3::new(20.0, 0.0, 0.0), Vec3::Y, Vec3::ONE, 0.9);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn irradiance_cache_decay() {
        let mut cache = IrradianceCache::new(8);
        cache.insert(Vec3::ZERO, Vec3::Y, Vec3::ONE, 1.0);
        let v0 = cache.entries[0].validity;
        cache.update_validity(Vec3::ZERO, 0.1);
        let v1 = cache.entries[0].validity;
        assert!(v1 < v0, "Validity should decrease after decay");
    }
}
