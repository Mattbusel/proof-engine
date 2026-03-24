//! # Parametric Surface Rendering
//!
//! Mathematical surfaces evaluated over (u, v) parameter space, tessellated into
//! triangle meshes suitable for glyph-grid rendering.
//!
//! Every surface implements the [`Surface`] trait, which provides `evaluate(u, v) -> Vec3`
//! and `normal(u, v) -> Vec3`. The [`SurfaceMesh`] struct tessellates any surface into
//! a triangle mesh with computed normals, tangents, and UV coordinates.
//!
//! ## Included surfaces
//!
//! - Sphere, Torus, MobiusStrip, KleinBottle
//! - BoySurface, RomanSurface, CrossCap
//! - TrefoilKnot, FigureEight
//! - Catenoid, Helicoid, EnneperSurface, DiniSurface
//! - FunctionSurface (user-defined closure)

use glam::Vec3;
use std::f32::consts::{PI, TAU};

// ─────────────────────────────────────────────────────────────────────────────
// Surface trait
// ─────────────────────────────────────────────────────────────────────────────

/// A parametric surface defined over (u, v) in [0, 1] x [0, 1].
pub trait Surface {
    /// Evaluate the surface position at parameter (u, v).
    fn evaluate(&self, u: f32, v: f32) -> Vec3;

    /// Compute the surface normal at parameter (u, v).
    /// Default implementation uses central differences.
    fn normal(&self, u: f32, v: f32) -> Vec3 {
        let eps = 1e-4;
        let du = self.evaluate((u + eps).min(1.0), v) - self.evaluate((u - eps).max(0.0), v);
        let dv = self.evaluate(u, (v + eps).min(1.0)) - self.evaluate(u, (v - eps).max(0.0));
        du.cross(dv).normalize_or_zero()
    }

    /// Compute the partial derivative with respect to u at (u, v).
    fn tangent_u(&self, u: f32, v: f32) -> Vec3 {
        let eps = 1e-4;
        let forward = self.evaluate((u + eps).min(1.0), v);
        let backward = self.evaluate((u - eps).max(0.0), v);
        (forward - backward).normalize_or_zero()
    }

    /// Compute the partial derivative with respect to v at (u, v).
    fn tangent_v(&self, u: f32, v: f32) -> Vec3 {
        let eps = 1e-4;
        let forward = self.evaluate(u, (v + eps).min(1.0));
        let backward = self.evaluate(u, (v - eps).max(0.0));
        (forward - backward).normalize_or_zero()
    }

    /// Parameter domain for u: (min, max). Defaults to (0, 1).
    fn u_range(&self) -> (f32, f32) { (0.0, 1.0) }

    /// Parameter domain for v: (min, max). Defaults to (0, 1).
    fn v_range(&self) -> (f32, f32) { (0.0, 1.0) }

    /// Whether the surface wraps in u. Used for seamless tessellation.
    fn wraps_u(&self) -> bool { false }

    /// Whether the surface wraps in v. Used for seamless tessellation.
    fn wraps_v(&self) -> bool { false }

    /// Human-readable name for debugging.
    fn name(&self) -> &str { "Surface" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sphere
// ─────────────────────────────────────────────────────────────────────────────

/// A unit sphere centered at the origin.
#[derive(Debug, Clone)]
pub struct Sphere {
    pub radius: f32,
    pub center: Vec3,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self { radius, center: Vec3::ZERO }
    }

    pub fn with_center(radius: f32, center: Vec3) -> Self {
        Self { radius, center }
    }
}

impl Default for Sphere {
    fn default() -> Self { Self::new(1.0) }
}

impl Surface for Sphere {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let phi = v * PI;
        let sp = phi.sin();
        self.center + Vec3::new(
            self.radius * sp * theta.cos(),
            self.radius * phi.cos(),
            self.radius * sp * theta.sin(),
        )
    }

    fn normal(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let phi = v * PI;
        let sp = phi.sin();
        Vec3::new(sp * theta.cos(), phi.cos(), sp * theta.sin()).normalize_or_zero()
    }

    fn wraps_u(&self) -> bool { true }
    fn name(&self) -> &str { "Sphere" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Torus
// ─────────────────────────────────────────────────────────────────────────────

/// A torus with major radius R and minor radius r.
#[derive(Debug, Clone)]
pub struct Torus {
    pub major_radius: f32,
    pub minor_radius: f32,
    pub center: Vec3,
}

impl Torus {
    pub fn new(major_radius: f32, minor_radius: f32) -> Self {
        Self { major_radius, minor_radius, center: Vec3::ZERO }
    }
}

impl Default for Torus {
    fn default() -> Self { Self::new(1.0, 0.3) }
}

impl Surface for Torus {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let phi = v * TAU;
        let r = self.major_radius + self.minor_radius * phi.cos();
        self.center + Vec3::new(
            r * theta.cos(),
            self.minor_radius * phi.sin(),
            r * theta.sin(),
        )
    }

    fn normal(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let phi = v * TAU;
        Vec3::new(
            phi.cos() * theta.cos(),
            phi.sin(),
            phi.cos() * theta.sin(),
        ).normalize_or_zero()
    }

    fn wraps_u(&self) -> bool { true }
    fn wraps_v(&self) -> bool { true }
    fn name(&self) -> &str { "Torus" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mobius Strip
// ─────────────────────────────────────────────────────────────────────────────

/// A Mobius strip — a surface with only one side.
#[derive(Debug, Clone)]
pub struct MobiusStrip {
    pub radius: f32,
    pub width: f32,
}

impl MobiusStrip {
    pub fn new(radius: f32, width: f32) -> Self {
        Self { radius, width }
    }
}

impl Default for MobiusStrip {
    fn default() -> Self { Self::new(1.0, 0.4) }
}

impl Surface for MobiusStrip {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let s = (v - 0.5) * self.width;
        let half_theta = theta * 0.5;
        let r = self.radius + s * half_theta.cos();
        Vec3::new(
            r * theta.cos(),
            s * half_theta.sin(),
            r * theta.sin(),
        )
    }

    fn wraps_u(&self) -> bool { true }
    fn name(&self) -> &str { "MobiusStrip" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Klein Bottle
// ─────────────────────────────────────────────────────────────────────────────

/// The Klein bottle — a non-orientable closed surface.
/// Uses the "figure-8" immersion in R^3.
#[derive(Debug, Clone)]
pub struct KleinBottle {
    pub scale: f32,
}

impl KleinBottle {
    pub fn new(scale: f32) -> Self { Self { scale } }
}

impl Default for KleinBottle {
    fn default() -> Self { Self::new(1.0) }
}

impl Surface for KleinBottle {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let phi = v * TAU;
        let s = self.scale;
        // Figure-8 Klein bottle immersion
        let r = 4.0 * (1.0 - 0.5 * theta.sin());
        let x = s * (6.0 * theta.cos() * (1.0 + theta.sin())
                     + r * theta.cos() * phi.cos());
        let y = s * (16.0 * theta.sin()
                     + r * theta.sin() * phi.cos());
        let z = s * r * phi.sin();
        Vec3::new(x * 0.05, y * 0.05, z * 0.05)
    }

    fn wraps_u(&self) -> bool { true }
    fn wraps_v(&self) -> bool { true }
    fn name(&self) -> &str { "KleinBottle" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Boy's Surface
// ─────────────────────────────────────────────────────────────────────────────

/// Boy's surface — an immersion of the real projective plane in R^3.
#[derive(Debug, Clone)]
pub struct BoySurface {
    pub scale: f32,
}

impl BoySurface {
    pub fn new(scale: f32) -> Self { Self { scale } }
}

impl Default for BoySurface {
    fn default() -> Self { Self::new(1.0) }
}

impl Surface for BoySurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        // Bryant-Kusner parametrization via spherical coords
        let theta = u * PI;
        let phi = v * TAU;
        let st = theta.sin();
        let ct = theta.cos();
        let sp = phi.sin();
        let cp = phi.cos();

        let denom = 2.0_f32.sqrt();
        let x1 = st * cp;
        let x2 = st * sp;
        let x3 = ct;

        // Boy surface parametrization (Apery)
        let g0 = x1 * x1 - x2 * x2;
        let g1 = x1 * x2;
        let g2 = x2 * x3;
        let g3 = x3 * x1;

        let px = denom * (2.0 * g3 + g0) / (x1 * x1 + x2 * x2 + x3 * x3 + 1.0);
        let py = denom * (2.0 * g2 + g1) / (x1 * x1 + x2 * x2 + x3 * x3 + 1.0);
        let pz = (3.0 * x3 * x3 - 1.0) / (2.0 * (x1 * x1 + x2 * x2 + x3 * x3 + 1.0));

        Vec3::new(px, py, pz) * self.scale
    }

    fn name(&self) -> &str { "BoySurface" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Roman Surface (Steiner surface)
// ─────────────────────────────────────────────────────────────────────────────

/// Steiner's Roman surface — a self-intersecting immersion of the real projective plane.
#[derive(Debug, Clone)]
pub struct RomanSurface {
    pub scale: f32,
}

impl RomanSurface {
    pub fn new(scale: f32) -> Self { Self { scale } }
}

impl Default for RomanSurface {
    fn default() -> Self { Self::new(1.0) }
}

impl Surface for RomanSurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * PI;
        let phi = v * TAU;
        let st = theta.sin();
        let ct = theta.cos();
        let sp = phi.sin();
        let cp = phi.cos();
        let s = self.scale;
        Vec3::new(
            s * st * st * sp * cp,
            s * st * cp * ct,
            s * st * sp * ct,
        )
    }

    fn name(&self) -> &str { "RomanSurface" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-Cap
// ─────────────────────────────────────────────────────────────────────────────

/// Cross-cap — another immersion of the real projective plane in R^3.
#[derive(Debug, Clone)]
pub struct CrossCap {
    pub scale: f32,
}

impl CrossCap {
    pub fn new(scale: f32) -> Self { Self { scale } }
}

impl Default for CrossCap {
    fn default() -> Self { Self::new(1.0) }
}

impl Surface for CrossCap {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * PI;
        let phi = v * TAU;
        let st = theta.sin();
        let ct = theta.cos();
        let sp = phi.sin();
        let cp = phi.cos();
        let s = self.scale;
        Vec3::new(
            s * 0.5 * st * (2.0 * phi).sin(),
            s * st * cp,
            s * ct,
        )
    }

    fn name(&self) -> &str { "CrossCap" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Trefoil Knot (tube surface around a trefoil curve)
// ─────────────────────────────────────────────────────────────────────────────

/// Trefoil knot — a tube surface of given radius wrapped around the trefoil curve.
#[derive(Debug, Clone)]
pub struct TrefoilKnot {
    pub tube_radius: f32,
    pub scale: f32,
}

impl TrefoilKnot {
    pub fn new(tube_radius: f32, scale: f32) -> Self {
        Self { tube_radius, scale }
    }
}

impl Default for TrefoilKnot {
    fn default() -> Self { Self::new(0.15, 1.0) }
}

impl TrefoilKnot {
    /// Evaluate the trefoil centerline at parameter t in [0, TAU].
    fn curve(&self, t: f32) -> Vec3 {
        let s = self.scale;
        Vec3::new(
            s * (t.sin() + 2.0 * (2.0 * t).sin()),
            s * (t.cos() - 2.0 * (2.0 * t).cos()),
            s * (-(3.0 * t).sin()),
        )
    }

    /// Approximate tangent by central differences.
    fn curve_tangent(&self, t: f32) -> Vec3 {
        let eps = 1e-4;
        (self.curve(t + eps) - self.curve(t - eps)).normalize_or_zero()
    }
}

impl Surface for TrefoilKnot {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let t = u * TAU;
        let phi = v * TAU;
        let center = self.curve(t);
        let tangent = self.curve_tangent(t);

        // Build a Frenet frame
        let up = if tangent.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
        let normal = tangent.cross(up).normalize_or_zero();
        let binormal = tangent.cross(normal).normalize_or_zero();

        center + self.tube_radius * (phi.cos() * normal + phi.sin() * binormal)
    }

    fn wraps_u(&self) -> bool { true }
    fn wraps_v(&self) -> bool { true }
    fn name(&self) -> &str { "TrefoilKnot" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Figure-Eight Knot (tube surface)
// ─────────────────────────────────────────────────────────────────────────────

/// Figure-eight knot — a tube surface around the figure-eight curve.
#[derive(Debug, Clone)]
pub struct FigureEight {
    pub tube_radius: f32,
    pub scale: f32,
}

impl FigureEight {
    pub fn new(tube_radius: f32, scale: f32) -> Self {
        Self { tube_radius, scale }
    }
}

impl Default for FigureEight {
    fn default() -> Self { Self::new(0.12, 1.0) }
}

impl FigureEight {
    fn curve(&self, t: f32) -> Vec3 {
        let s = self.scale;
        let ct = t.cos();
        let st = t.sin();
        let s2t = (2.0 * t).sin();
        Vec3::new(
            s * (2.0 + ct) * (3.0 * t).cos(),
            s * (2.0 + ct) * (3.0 * t).sin(),
            s * st * 3.0,
        )
    }

    fn curve_tangent(&self, t: f32) -> Vec3 {
        let eps = 1e-4;
        (self.curve(t + eps) - self.curve(t - eps)).normalize_or_zero()
    }
}

impl Surface for FigureEight {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let t = u * TAU;
        let phi = v * TAU;
        let center = self.curve(t);
        let tangent = self.curve_tangent(t);

        let up = if tangent.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
        let normal = tangent.cross(up).normalize_or_zero();
        let binormal = tangent.cross(normal).normalize_or_zero();

        center + self.tube_radius * (phi.cos() * normal + phi.sin() * binormal)
    }

    fn wraps_u(&self) -> bool { true }
    fn wraps_v(&self) -> bool { true }
    fn name(&self) -> &str { "FigureEight" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Catenoid
// ─────────────────────────────────────────────────────────────────────────────

/// A catenoid — a minimal surface of revolution formed by a catenary.
#[derive(Debug, Clone)]
pub struct Catenoid {
    pub c: f32,
    pub height_range: f32,
}

impl Catenoid {
    pub fn new(c: f32, height_range: f32) -> Self {
        Self { c, height_range }
    }
}

impl Default for Catenoid {
    fn default() -> Self { Self::new(1.0, 2.0) }
}

impl Surface for Catenoid {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let t = (v - 0.5) * self.height_range;
        let ch = (t / self.c).cosh();
        Vec3::new(
            self.c * ch * theta.cos(),
            t,
            self.c * ch * theta.sin(),
        )
    }

    fn normal(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let t = (v - 0.5) * self.height_range;
        let sh = (t / self.c).sinh();
        let ch = (t / self.c).cosh();
        Vec3::new(
            -theta.cos() / ch,
            sh / ch,
            -theta.sin() / ch,
        ).normalize_or_zero()
    }

    fn wraps_u(&self) -> bool { true }
    fn name(&self) -> &str { "Catenoid" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helicoid
// ─────────────────────────────────────────────────────────────────────────────

/// A helicoid — a minimal surface swept by a line rotating about and translating along an axis.
#[derive(Debug, Clone)]
pub struct Helicoid {
    pub pitch: f32,
    pub radius: f32,
    pub height_range: f32,
}

impl Helicoid {
    pub fn new(pitch: f32, radius: f32, height_range: f32) -> Self {
        Self { pitch, radius, height_range }
    }
}

impl Default for Helicoid {
    fn default() -> Self { Self::new(1.0, 1.0, 4.0) }
}

impl Surface for Helicoid {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU * 2.0; // two full turns
        let r = (v - 0.5) * 2.0 * self.radius;
        Vec3::new(
            r * theta.cos(),
            self.pitch * theta / TAU * self.height_range * 0.5,
            r * theta.sin(),
        )
    }

    fn normal(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU * 2.0;
        let denom = (self.pitch * self.pitch + ((v - 0.5) * 2.0 * self.radius).powi(2)).sqrt();
        if denom < 1e-8 {
            return Vec3::Y;
        }
        Vec3::new(
            self.pitch * theta.sin() / denom,
            -((v - 0.5) * 2.0 * self.radius) / denom,
            -self.pitch * theta.cos() / denom,
        ).normalize_or_zero()
    }

    fn name(&self) -> &str { "Helicoid" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Enneper Surface
// ─────────────────────────────────────────────────────────────────────────────

/// Enneper's minimal surface — self-intersects at higher ranges.
#[derive(Debug, Clone)]
pub struct EnneperSurface {
    pub scale: f32,
    pub range: f32,
}

impl EnneperSurface {
    pub fn new(scale: f32, range: f32) -> Self {
        Self { scale, range }
    }
}

impl Default for EnneperSurface {
    fn default() -> Self { Self::new(0.3, 2.0) }
}

impl Surface for EnneperSurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let uu = (u - 0.5) * 2.0 * self.range;
        let vv = (v - 0.5) * 2.0 * self.range;
        let s = self.scale;
        Vec3::new(
            s * (uu - uu.powi(3) / 3.0 + uu * vv * vv),
            s * (vv - vv.powi(3) / 3.0 + vv * uu * uu),
            s * (uu * uu - vv * vv),
        )
    }

    fn name(&self) -> &str { "EnneperSurface" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dini's Surface
// ─────────────────────────────────────────────────────────────────────────────

/// Dini's surface — a twisted pseudosphere with constant negative curvature.
#[derive(Debug, Clone)]
pub struct DiniSurface {
    pub a: f32,
    pub b: f32,
    pub turns: f32,
}

impl DiniSurface {
    pub fn new(a: f32, b: f32, turns: f32) -> Self {
        Self { a, b, turns }
    }
}

impl Default for DiniSurface {
    fn default() -> Self { Self::new(1.0, 0.2, 4.0) }
}

impl Surface for DiniSurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU * self.turns;
        // v in (0, PI) but avoid singularity at 0
        let phi = v * (PI - 0.02) + 0.01;
        let a = self.a;
        let b = self.b;
        Vec3::new(
            a * theta.cos() * phi.sin(),
            a * theta.sin() * phi.sin(),
            a * (phi.cos() + (phi / 2.0).tan().max(-100.0).min(100.0).ln()) + b * theta,
        )
    }

    fn name(&self) -> &str { "DiniSurface" }
}

// ─────────────────────────────────────────────────────────────────────────────
// FunctionSurface (user-defined)
// ─────────────────────────────────────────────────────────────────────────────

/// A surface defined by a user-provided closure.
pub struct FunctionSurface {
    pub func: Box<dyn Fn(f32, f32) -> Vec3 + Send + Sync>,
    label: String,
    wrap_u: bool,
    wrap_v: bool,
}

impl FunctionSurface {
    pub fn new<F>(func: F) -> Self
    where
        F: Fn(f32, f32) -> Vec3 + Send + Sync + 'static,
    {
        Self {
            func: Box::new(func),
            label: "FunctionSurface".to_string(),
            wrap_u: false,
            wrap_v: false,
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.label = name.to_string();
        self
    }

    pub fn with_wrapping(mut self, wrap_u: bool, wrap_v: bool) -> Self {
        self.wrap_u = wrap_u;
        self.wrap_v = wrap_v;
        self
    }
}

impl Surface for FunctionSurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        (self.func)(u, v)
    }

    fn wraps_u(&self) -> bool { self.wrap_u }
    fn wraps_v(&self) -> bool { self.wrap_v }
    fn name(&self) -> &str { &self.label }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vertex
// ─────────────────────────────────────────────────────────────────────────────

/// A vertex in a surface mesh.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub bitangent: Vec3,
    pub uv: [f32; 2],
    /// Parameter-space coordinates that produced this vertex.
    pub param_u: f32,
    pub param_v: f32,
}

impl Default for SurfaceVertex {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::Y,
            tangent: Vec3::X,
            bitangent: Vec3::Z,
            uv: [0.0, 0.0],
            param_u: 0.0,
            param_v: 0.0,
        }
    }
}

/// A triangle index triple.
#[derive(Debug, Clone, Copy)]
pub struct TriangleIndex {
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

impl TriangleIndex {
    pub fn new(a: u32, b: u32, c: u32) -> Self { Self { a, b, c } }
}

// ─────────────────────────────────────────────────────────────────────────────
// SurfaceMesh
// ─────────────────────────────────────────────────────────────────────────────

/// A tessellated mesh of a parametric surface.
#[derive(Debug, Clone)]
pub struct SurfaceMesh {
    pub vertices: Vec<SurfaceVertex>,
    pub indices: Vec<TriangleIndex>,
    pub resolution_u: usize,
    pub resolution_v: usize,
    /// Axis-aligned bounding box: (min, max).
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
}

impl SurfaceMesh {
    /// Tessellate a surface at the given resolution.
    ///
    /// `resolution_u` and `resolution_v` define the number of subdivisions along
    /// each parameter axis. The resulting mesh has `(res_u + 1) * (res_v + 1)` vertices
    /// and `2 * res_u * res_v` triangles.
    pub fn tessellate(surface: &dyn Surface, resolution_u: usize, resolution_v: usize) -> Self {
        let res_u = resolution_u.max(2);
        let res_v = resolution_v.max(2);
        let num_verts = (res_u + 1) * (res_v + 1);
        let mut vertices = Vec::with_capacity(num_verts);
        let mut aabb_min = Vec3::splat(f32::MAX);
        let mut aabb_max = Vec3::splat(f32::MIN);

        // Generate vertices
        for j in 0..=res_v {
            let v = j as f32 / res_v as f32;
            for i in 0..=res_u {
                let u = i as f32 / res_u as f32;
                let pos = surface.evaluate(u, v);
                let norm = surface.normal(u, v);
                let tan_u = surface.tangent_u(u, v);
                let bitan = norm.cross(tan_u).normalize_or_zero();

                aabb_min = aabb_min.min(pos);
                aabb_max = aabb_max.max(pos);

                vertices.push(SurfaceVertex {
                    position: pos,
                    normal: norm,
                    tangent: tan_u,
                    bitangent: bitan,
                    uv: [u, v],
                    param_u: u,
                    param_v: v,
                });
            }
        }

        // Generate triangle indices
        let num_triangles = res_u * res_v * 2;
        let mut indices = Vec::with_capacity(num_triangles);
        for j in 0..res_v {
            for i in 0..res_u {
                let tl = (j * (res_u + 1) + i) as u32;
                let tr = tl + 1;
                let bl = tl + (res_u + 1) as u32;
                let br = bl + 1;
                indices.push(TriangleIndex::new(tl, bl, tr));
                indices.push(TriangleIndex::new(tr, bl, br));
            }
        }

        let mut mesh = Self {
            vertices,
            indices,
            resolution_u: res_u,
            resolution_v: res_v,
            aabb_min,
            aabb_max,
        };

        // Recompute smooth normals by averaging face normals at shared vertices
        mesh.recompute_smooth_normals();
        mesh
    }

    /// Tessellate with adaptive resolution that places more samples where curvature is higher.
    pub fn tessellate_adaptive(
        surface: &dyn Surface,
        base_resolution_u: usize,
        base_resolution_v: usize,
        curvature_threshold: f32,
    ) -> Self {
        // Sample curvature to find high-curvature regions
        let sample_res = 16.min(base_resolution_u).max(4);
        let mut max_curvature = 0.0_f32;
        let mut curvature_map: Vec<f32> = Vec::with_capacity(sample_res * sample_res);

        for j in 0..sample_res {
            let v = j as f32 / sample_res as f32;
            for i in 0..sample_res {
                let u = i as f32 / sample_res as f32;
                let c = Self::estimate_curvature(surface, u, v);
                max_curvature = max_curvature.max(c);
                curvature_map.push(c);
            }
        }

        // For now, if curvature is uniformly low, use base resolution.
        // Otherwise, double it.
        let factor = if max_curvature > curvature_threshold { 2 } else { 1 };
        Self::tessellate(surface, base_resolution_u * factor, base_resolution_v * factor)
    }

    /// Estimate Gaussian curvature at a point using finite differences.
    fn estimate_curvature(surface: &dyn Surface, u: f32, v: f32) -> f32 {
        let eps = 1e-3;
        let n0 = surface.normal(u, v);
        let nu = surface.normal((u + eps).min(1.0), v);
        let nv = surface.normal(u, (v + eps).min(1.0));
        let du = (nu - n0).length() / eps;
        let dv = (nv - n0).length() / eps;
        (du * du + dv * dv).sqrt()
    }

    /// Recompute vertex normals by averaging adjacent face normals.
    pub fn recompute_smooth_normals(&mut self) {
        // Zero all normals
        for v in &mut self.vertices {
            v.normal = Vec3::ZERO;
        }

        // Accumulate face normals
        for tri in &self.indices {
            let a = tri.a as usize;
            let b = tri.b as usize;
            let c = tri.c as usize;
            let pa = self.vertices[a].position;
            let pb = self.vertices[b].position;
            let pc = self.vertices[c].position;
            let face_normal = (pb - pa).cross(pc - pa);
            // Weight by face area (non-normalized cross product = 2 * area)
            self.vertices[a].normal += face_normal;
            self.vertices[b].normal += face_normal;
            self.vertices[c].normal += face_normal;
        }

        // Normalize and recompute tangent frames
        for v in &mut self.vertices {
            v.normal = v.normal.normalize_or_zero();
            // Recompute tangent frame from normal
            let up = if v.normal.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
            v.tangent = v.normal.cross(up).normalize_or_zero();
            v.bitangent = v.normal.cross(v.tangent).normalize_or_zero();
        }
    }

    /// Recompute tangent vectors using UV-aligned tangent calculation (MikkTSpace-style).
    pub fn recompute_tangents(&mut self) {
        // Zero tangents and bitangents
        for v in &mut self.vertices {
            v.tangent = Vec3::ZERO;
            v.bitangent = Vec3::ZERO;
        }

        for tri in &self.indices {
            let i0 = tri.a as usize;
            let i1 = tri.b as usize;
            let i2 = tri.c as usize;

            let p0 = self.vertices[i0].position;
            let p1 = self.vertices[i1].position;
            let p2 = self.vertices[i2].position;

            let uv0 = self.vertices[i0].uv;
            let uv1 = self.vertices[i1].uv;
            let uv2 = self.vertices[i2].uv;

            let dp1 = p1 - p0;
            let dp2 = p2 - p0;
            let duv1 = [uv1[0] - uv0[0], uv1[1] - uv0[1]];
            let duv2 = [uv2[0] - uv0[0], uv2[1] - uv0[1]];

            let det = duv1[0] * duv2[1] - duv1[1] * duv2[0];
            if det.abs() < 1e-8 {
                continue;
            }
            let inv_det = 1.0 / det;

            let tangent = (dp1 * duv2[1] - dp2 * duv1[1]) * inv_det;
            let bitangent = (dp2 * duv1[0] - dp1 * duv2[0]) * inv_det;

            self.vertices[i0].tangent += tangent;
            self.vertices[i1].tangent += tangent;
            self.vertices[i2].tangent += tangent;

            self.vertices[i0].bitangent += bitangent;
            self.vertices[i1].bitangent += bitangent;
            self.vertices[i2].bitangent += bitangent;
        }

        // Gram-Schmidt orthonormalize
        for v in &mut self.vertices {
            let n = v.normal;
            let t = v.tangent;
            let b = v.bitangent;

            // Orthonormalize tangent
            let t_orth = (t - n * n.dot(t)).normalize_or_zero();
            // Compute bitangent with correct handedness
            let b_sign = if n.cross(t).dot(b) < 0.0 { -1.0 } else { 1.0 };
            let b_orth = n.cross(t_orth) * b_sign;

            v.tangent = t_orth;
            v.bitangent = b_orth;
        }
    }

    /// Transform all vertices by a 4x4-like operation (scale + translate).
    pub fn transform(&mut self, scale: f32, offset: Vec3) {
        self.aabb_min = Vec3::splat(f32::MAX);
        self.aabb_max = Vec3::splat(f32::MIN);
        for v in &mut self.vertices {
            v.position = v.position * scale + offset;
            self.aabb_min = self.aabb_min.min(v.position);
            self.aabb_max = self.aabb_max.max(v.position);
        }
    }

    /// Return the center of the bounding box.
    pub fn center(&self) -> Vec3 {
        (self.aabb_min + self.aabb_max) * 0.5
    }

    /// Return the extent (half-size) of the bounding box.
    pub fn extent(&self) -> Vec3 {
        (self.aabb_max - self.aabb_min) * 0.5
    }

    /// Total number of triangles in the mesh.
    pub fn triangle_count(&self) -> usize {
        self.indices.len()
    }

    /// Total number of vertices in the mesh.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Compute the surface area of the mesh by summing triangle areas.
    pub fn surface_area(&self) -> f32 {
        let mut area = 0.0_f32;
        for tri in &self.indices {
            let a = self.vertices[tri.a as usize].position;
            let b = self.vertices[tri.b as usize].position;
            let c = self.vertices[tri.c as usize].position;
            area += (b - a).cross(c - a).length() * 0.5;
        }
        area
    }

    /// Generate flat normals (each face gets its own normal, vertices are duplicated).
    pub fn make_flat_shaded(&mut self) {
        let mut new_verts = Vec::with_capacity(self.indices.len() * 3);
        let mut new_indices = Vec::with_capacity(self.indices.len());

        for tri in &self.indices {
            let va = self.vertices[tri.a as usize];
            let vb = self.vertices[tri.b as usize];
            let vc = self.vertices[tri.c as usize];

            let face_normal = (vb.position - va.position)
                .cross(vc.position - va.position)
                .normalize_or_zero();

            let base = new_verts.len() as u32;

            let mut a = va;
            let mut b = vb;
            let mut c = vc;
            a.normal = face_normal;
            b.normal = face_normal;
            c.normal = face_normal;

            new_verts.push(a);
            new_verts.push(b);
            new_verts.push(c);
            new_indices.push(TriangleIndex::new(base, base + 1, base + 2));
        }

        self.vertices = new_verts;
        self.indices = new_indices;
    }

    /// Subdivide each triangle into 4 triangles (Loop-style without smoothing).
    pub fn subdivide(&mut self) {
        let mut edge_midpoints: std::collections::HashMap<(u32, u32), u32> =
            std::collections::HashMap::new();

        let mut new_indices = Vec::with_capacity(self.indices.len() * 4);

        for tri in &self.indices {
            let mid_ab = Self::get_or_create_midpoint(
                &mut self.vertices, &mut edge_midpoints, tri.a, tri.b,
            );
            let mid_bc = Self::get_or_create_midpoint(
                &mut self.vertices, &mut edge_midpoints, tri.b, tri.c,
            );
            let mid_ca = Self::get_or_create_midpoint(
                &mut self.vertices, &mut edge_midpoints, tri.c, tri.a,
            );

            new_indices.push(TriangleIndex::new(tri.a, mid_ab, mid_ca));
            new_indices.push(TriangleIndex::new(mid_ab, tri.b, mid_bc));
            new_indices.push(TriangleIndex::new(mid_ca, mid_bc, tri.c));
            new_indices.push(TriangleIndex::new(mid_ab, mid_bc, mid_ca));
        }

        self.indices = new_indices;
        self.recompute_smooth_normals();
    }

    fn get_or_create_midpoint(
        vertices: &mut Vec<SurfaceVertex>,
        edge_map: &mut std::collections::HashMap<(u32, u32), u32>,
        a: u32,
        b: u32,
    ) -> u32 {
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(&idx) = edge_map.get(&key) {
            return idx;
        }
        let va = vertices[a as usize];
        let vb = vertices[b as usize];
        let mid = SurfaceVertex {
            position: (va.position + vb.position) * 0.5,
            normal: (va.normal + vb.normal).normalize_or_zero(),
            tangent: (va.tangent + vb.tangent).normalize_or_zero(),
            bitangent: (va.bitangent + vb.bitangent).normalize_or_zero(),
            uv: [(va.uv[0] + vb.uv[0]) * 0.5, (va.uv[1] + vb.uv[1]) * 0.5],
            param_u: (va.param_u + vb.param_u) * 0.5,
            param_v: (va.param_v + vb.param_v) * 0.5,
        };
        let idx = vertices.len() as u32;
        vertices.push(mid);
        edge_map.insert(key, idx);
        idx
    }

    /// Extract positions as a flat array of f32 triples (for GPU upload).
    pub fn positions_flat(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.vertices.len() * 3);
        for v in &self.vertices {
            out.push(v.position.x);
            out.push(v.position.y);
            out.push(v.position.z);
        }
        out
    }

    /// Extract normals as a flat array of f32 triples.
    pub fn normals_flat(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.vertices.len() * 3);
        for v in &self.vertices {
            out.push(v.normal.x);
            out.push(v.normal.y);
            out.push(v.normal.z);
        }
        out
    }

    /// Extract UVs as a flat array of f32 pairs.
    pub fn uvs_flat(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.vertices.len() * 2);
        for v in &self.vertices {
            out.push(v.uv[0]);
            out.push(v.uv[1]);
        }
        out
    }

    /// Extract indices as a flat array of u32.
    pub fn indices_flat(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.indices.len() * 3);
        for tri in &self.indices {
            out.push(tri.a);
            out.push(tri.b);
            out.push(tri.c);
        }
        out
    }

    /// Generate wireframe edges (deduplicated edge list).
    pub fn wireframe_edges(&self) -> Vec<(u32, u32)> {
        let mut edges = std::collections::HashSet::new();
        for tri in &self.indices {
            let mut add_edge = |a: u32, b: u32| {
                let key = if a < b { (a, b) } else { (b, a) };
                edges.insert(key);
            };
            add_edge(tri.a, tri.b);
            add_edge(tri.b, tri.c);
            add_edge(tri.c, tri.a);
        }
        edges.into_iter().collect()
    }

    /// Sample the mesh at a given (u, v) by bilinear interpolation of grid vertices.
    pub fn sample(&self, u: f32, v: f32) -> Vec3 {
        let fu = u.clamp(0.0, 1.0) * self.resolution_u as f32;
        let fv = v.clamp(0.0, 1.0) * self.resolution_v as f32;
        let iu = (fu as usize).min(self.resolution_u - 1);
        let iv = (fv as usize).min(self.resolution_v - 1);
        let su = fu - iu as f32;
        let sv = fv - iv as f32;

        let stride = self.resolution_u + 1;
        let i00 = iv * stride + iu;
        let i10 = i00 + 1;
        let i01 = i00 + stride;
        let i11 = i01 + 1;

        let p00 = self.vertices[i00].position;
        let p10 = self.vertices[i10.min(self.vertices.len() - 1)].position;
        let p01 = self.vertices[i01.min(self.vertices.len() - 1)].position;
        let p11 = self.vertices[i11.min(self.vertices.len() - 1)].position;

        let top = p00 * (1.0 - su) + p10 * su;
        let bottom = p01 * (1.0 - su) + p11 * su;
        top * (1.0 - sv) + bottom * sv
    }

    /// Convert the mesh into a grid of characters for text-mode rendering.
    /// Maps the surface onto a width x height character grid using depth buffering.
    pub fn to_glyph_grid(&self, width: usize, height: usize) -> Vec<Vec<char>> {
        let mut grid = vec![vec![' '; width]; height];
        let mut depth = vec![vec![f32::MAX; width]; height];

        let shading_chars = ['.', ':', '-', '=', '+', '*', '#', '%', '@'];

        let center = self.center();
        let ext = self.extent();
        let max_ext = ext.x.max(ext.y).max(ext.z).max(0.001);

        for tri in &self.indices {
            let va = &self.vertices[tri.a as usize];
            let vb = &self.vertices[tri.b as usize];
            let vc = &self.vertices[tri.c as usize];

            // Project each vertex to screen space
            for vert in [va, vb, vc] {
                let rel = vert.position - center;
                let sx = ((rel.x / max_ext + 1.0) * 0.5 * (width - 1) as f32) as i32;
                let sy = ((1.0 - (rel.y / max_ext + 1.0) * 0.5) * (height - 1) as f32) as i32;
                let sz = rel.z / max_ext;

                if sx >= 0 && sx < width as i32 && sy >= 0 && sy < height as i32 {
                    let ux = sx as usize;
                    let uy = sy as usize;
                    if sz < depth[uy][ux] {
                        depth[uy][ux] = sz;
                        // Use normal dot product with light direction for shading
                        let light = Vec3::new(0.577, 0.577, 0.577);
                        let intensity = vert.normal.dot(light).abs();
                        let idx = (intensity * (shading_chars.len() - 1) as f32) as usize;
                        let idx = idx.min(shading_chars.len() - 1);
                        grid[uy][ux] = shading_chars[idx];
                    }
                }
            }
        }

        grid
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Surface builder utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Create a surface of revolution from a 2D profile curve.
///
/// The profile is a function `f(v) -> (radius, height)` where v is in [0, 1].
/// The surface revolves around the Y axis.
pub struct RevolutionSurface {
    pub profile: Box<dyn Fn(f32) -> (f32, f32) + Send + Sync>,
    label: String,
}

impl RevolutionSurface {
    pub fn new<F>(profile: F) -> Self
    where
        F: Fn(f32) -> (f32, f32) + Send + Sync + 'static,
    {
        Self {
            profile: Box::new(profile),
            label: "RevolutionSurface".to_string(),
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.label = name.to_string();
        self
    }
}

impl Surface for RevolutionSurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * TAU;
        let (r, h) = (self.profile)(v);
        Vec3::new(r * theta.cos(), h, r * theta.sin())
    }

    fn wraps_u(&self) -> bool { true }
    fn name(&self) -> &str { &self.label }
}

/// Create a ruled surface between two space curves.
///
/// Given curves `a(t)` and `b(t)` for t in [0,1], the ruled surface is
/// `S(u, v) = (1-v) * a(u) + v * b(u)`.
pub struct RuledSurface {
    pub curve_a: Box<dyn Fn(f32) -> Vec3 + Send + Sync>,
    pub curve_b: Box<dyn Fn(f32) -> Vec3 + Send + Sync>,
    label: String,
}

impl RuledSurface {
    pub fn new<A, B>(curve_a: A, curve_b: B) -> Self
    where
        A: Fn(f32) -> Vec3 + Send + Sync + 'static,
        B: Fn(f32) -> Vec3 + Send + Sync + 'static,
    {
        Self {
            curve_a: Box::new(curve_a),
            curve_b: Box::new(curve_b),
            label: "RuledSurface".to_string(),
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.label = name.to_string();
        self
    }
}

impl Surface for RuledSurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let a = (self.curve_a)(u);
        let b = (self.curve_b)(u);
        a * (1.0 - v) + b * v
    }

    fn name(&self) -> &str { &self.label }
}

/// Compose two surfaces by blending them with a weight function.
pub struct BlendedSurface<A: Surface, B: Surface> {
    pub surface_a: A,
    pub surface_b: B,
    pub blend_fn: Box<dyn Fn(f32, f32) -> f32 + Send + Sync>,
}

impl<A: Surface, B: Surface> BlendedSurface<A, B> {
    pub fn new<F>(a: A, b: B, blend: F) -> Self
    where
        F: Fn(f32, f32) -> f32 + Send + Sync + 'static,
    {
        Self {
            surface_a: a,
            surface_b: b,
            blend_fn: Box::new(blend),
        }
    }

    /// Uniform blend (constant t).
    pub fn uniform(a: A, b: B, t: f32) -> Self {
        Self::new(a, b, move |_u, _v| t)
    }
}

impl<A: Surface, B: Surface> Surface for BlendedSurface<A, B> {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let t = (self.blend_fn)(u, v).clamp(0.0, 1.0);
        let pa = self.surface_a.evaluate(u, v);
        let pb = self.surface_b.evaluate(u, v);
        pa * (1.0 - t) + pb * t
    }

    fn name(&self) -> &str { "BlendedSurface" }
}

/// Displace a surface along its normals by a scalar field.
pub struct DisplacedSurface<S: Surface> {
    pub base: S,
    pub displacement: Box<dyn Fn(f32, f32) -> f32 + Send + Sync>,
}

impl<S: Surface> DisplacedSurface<S> {
    pub fn new<F>(base: S, displacement: F) -> Self
    where
        F: Fn(f32, f32) -> f32 + Send + Sync + 'static,
    {
        Self {
            base,
            displacement: Box::new(displacement),
        }
    }
}

impl<S: Surface> Surface for DisplacedSurface<S> {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let pos = self.base.evaluate(u, v);
        let n = self.base.normal(u, v);
        let d = (self.displacement)(u, v);
        pos + n * d
    }

    fn name(&self) -> &str { "DisplacedSurface" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Surface catalog / registry
// ─────────────────────────────────────────────────────────────────────────────

/// Enumeration of built-in surface types for catalog purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    Sphere,
    Torus,
    MobiusStrip,
    KleinBottle,
    BoySurface,
    RomanSurface,
    CrossCap,
    TrefoilKnot,
    FigureEight,
    Catenoid,
    Helicoid,
    Enneper,
    Dini,
}

impl SurfaceKind {
    /// All built-in surface types.
    pub fn all() -> &'static [SurfaceKind] {
        &[
            SurfaceKind::Sphere,
            SurfaceKind::Torus,
            SurfaceKind::MobiusStrip,
            SurfaceKind::KleinBottle,
            SurfaceKind::BoySurface,
            SurfaceKind::RomanSurface,
            SurfaceKind::CrossCap,
            SurfaceKind::TrefoilKnot,
            SurfaceKind::FigureEight,
            SurfaceKind::Catenoid,
            SurfaceKind::Helicoid,
            SurfaceKind::Enneper,
            SurfaceKind::Dini,
        ]
    }

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            SurfaceKind::Sphere => "Sphere",
            SurfaceKind::Torus => "Torus",
            SurfaceKind::MobiusStrip => "Mobius Strip",
            SurfaceKind::KleinBottle => "Klein Bottle",
            SurfaceKind::BoySurface => "Boy's Surface",
            SurfaceKind::RomanSurface => "Roman Surface",
            SurfaceKind::CrossCap => "Cross-Cap",
            SurfaceKind::TrefoilKnot => "Trefoil Knot",
            SurfaceKind::FigureEight => "Figure-Eight",
            SurfaceKind::Catenoid => "Catenoid",
            SurfaceKind::Helicoid => "Helicoid",
            SurfaceKind::Enneper => "Enneper Surface",
            SurfaceKind::Dini => "Dini's Surface",
        }
    }

    /// Create a default-parameterized mesh for this surface at given resolution.
    pub fn tessellate(self, res_u: usize, res_v: usize) -> SurfaceMesh {
        match self {
            SurfaceKind::Sphere => SurfaceMesh::tessellate(&Sphere::default(), res_u, res_v),
            SurfaceKind::Torus => SurfaceMesh::tessellate(&Torus::default(), res_u, res_v),
            SurfaceKind::MobiusStrip => SurfaceMesh::tessellate(&MobiusStrip::default(), res_u, res_v),
            SurfaceKind::KleinBottle => SurfaceMesh::tessellate(&KleinBottle::default(), res_u, res_v),
            SurfaceKind::BoySurface => SurfaceMesh::tessellate(&BoySurface::default(), res_u, res_v),
            SurfaceKind::RomanSurface => SurfaceMesh::tessellate(&RomanSurface::default(), res_u, res_v),
            SurfaceKind::CrossCap => SurfaceMesh::tessellate(&CrossCap::default(), res_u, res_v),
            SurfaceKind::TrefoilKnot => SurfaceMesh::tessellate(&TrefoilKnot::default(), res_u, res_v),
            SurfaceKind::FigureEight => SurfaceMesh::tessellate(&FigureEight::default(), res_u, res_v),
            SurfaceKind::Catenoid => SurfaceMesh::tessellate(&Catenoid::default(), res_u, res_v),
            SurfaceKind::Helicoid => SurfaceMesh::tessellate(&Helicoid::default(), res_u, res_v),
            SurfaceKind::Enneper => SurfaceMesh::tessellate(&EnneperSurface::default(), res_u, res_v),
            SurfaceKind::Dini => SurfaceMesh::tessellate(&DiniSurface::default(), res_u, res_v),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Iso-curve extraction
// ─────────────────────────────────────────────────────────────────────────────

/// Extract iso-parameter curves from a surface.
pub struct IsoCurves;

impl IsoCurves {
    /// Extract a curve at constant u, varying v from 0 to 1.
    pub fn at_constant_u(surface: &dyn Surface, u: f32, samples: usize) -> Vec<Vec3> {
        let n = samples.max(2);
        (0..n).map(|i| {
            let v = i as f32 / (n - 1) as f32;
            surface.evaluate(u, v)
        }).collect()
    }

    /// Extract a curve at constant v, varying u from 0 to 1.
    pub fn at_constant_v(surface: &dyn Surface, v: f32, samples: usize) -> Vec<Vec3> {
        let n = samples.max(2);
        (0..n).map(|i| {
            let u = i as f32 / (n - 1) as f32;
            surface.evaluate(u, v)
        }).collect()
    }

    /// Extract a grid of iso-curves in both directions.
    pub fn grid(
        surface: &dyn Surface,
        u_curves: usize,
        v_curves: usize,
        samples: usize,
    ) -> (Vec<Vec<Vec3>>, Vec<Vec<Vec3>>) {
        let u_lines: Vec<Vec<Vec3>> = (0..u_curves)
            .map(|i| {
                let u = i as f32 / (u_curves - 1).max(1) as f32;
                Self::at_constant_u(surface, u, samples)
            })
            .collect();

        let v_lines: Vec<Vec<Vec3>> = (0..v_curves)
            .map(|i| {
                let v = i as f32 / (v_curves - 1).max(1) as f32;
                Self::at_constant_v(surface, v, samples)
            })
            .collect();

        (u_lines, v_lines)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Ray-surface intersection
// ─────────────────────────────────────────────────────────────────────────────

/// Result of a ray-mesh intersection test.
#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub distance: f32,
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: [f32; 2],
    pub triangle_index: usize,
}

impl SurfaceMesh {
    /// Ray-mesh intersection using Moller-Trumbore algorithm.
    /// Returns the closest hit, if any.
    pub fn ray_intersect(&self, ray_origin: Vec3, ray_dir: Vec3) -> Option<RayHit> {
        let mut closest: Option<RayHit> = None;
        let eps = 1e-7;

        for (tri_idx, tri) in self.indices.iter().enumerate() {
            let v0 = self.vertices[tri.a as usize].position;
            let v1 = self.vertices[tri.b as usize].position;
            let v2 = self.vertices[tri.c as usize].position;

            let e1 = v1 - v0;
            let e2 = v2 - v0;
            let h = ray_dir.cross(e2);
            let a = e1.dot(h);

            if a.abs() < eps {
                continue;
            }

            let f = 1.0 / a;
            let s = ray_origin - v0;
            let u_bary = f * s.dot(h);
            if !(0.0..=1.0).contains(&u_bary) {
                continue;
            }

            let q = s.cross(e1);
            let v_bary = f * ray_dir.dot(q);
            if v_bary < 0.0 || u_bary + v_bary > 1.0 {
                continue;
            }

            let t = f * e2.dot(q);
            if t < eps {
                continue;
            }

            if closest.as_ref().map_or(true, |c| t < c.distance) {
                let w = 1.0 - u_bary - v_bary;
                let va = &self.vertices[tri.a as usize];
                let vb = &self.vertices[tri.b as usize];
                let vc = &self.vertices[tri.c as usize];

                let normal = (va.normal * w + vb.normal * u_bary + vc.normal * v_bary)
                    .normalize_or_zero();
                let uv = [
                    va.uv[0] * w + vb.uv[0] * u_bary + vc.uv[0] * v_bary,
                    va.uv[1] * w + vb.uv[1] * u_bary + vc.uv[1] * v_bary,
                ];

                closest = Some(RayHit {
                    distance: t,
                    position: ray_origin + ray_dir * t,
                    normal,
                    uv,
                    triangle_index: tri_idx,
                });
            }
        }

        closest
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_poles() {
        let s = Sphere::default();
        let north = s.evaluate(0.0, 0.0);
        let south = s.evaluate(0.0, 1.0);
        assert!((north.y - 1.0).abs() < 1e-5);
        assert!((south.y + 1.0).abs() < 1e-5);
    }

    #[test]
    fn torus_evaluate() {
        let t = Torus::default();
        let p = t.evaluate(0.0, 0.0);
        assert!((p.x - 1.3).abs() < 0.01); // major + minor
    }

    #[test]
    fn tessellate_sphere() {
        let s = Sphere::default();
        let mesh = SurfaceMesh::tessellate(&s, 16, 8);
        assert_eq!(mesh.vertex_count(), 17 * 9);
        assert_eq!(mesh.triangle_count(), 16 * 8 * 2);
    }

    #[test]
    fn surface_mesh_area() {
        let s = Sphere::new(1.0);
        let mesh = SurfaceMesh::tessellate(&s, 64, 32);
        let area = mesh.surface_area();
        // 4 * pi * r^2 ~ 12.566
        assert!((area - 4.0 * PI).abs() < 0.5);
    }

    #[test]
    fn ray_intersect_sphere() {
        let s = Sphere::new(1.0);
        let mesh = SurfaceMesh::tessellate(&s, 32, 16);
        let hit = mesh.ray_intersect(Vec3::new(0.0, 0.0, -5.0), Vec3::Z);
        assert!(hit.is_some());
        let h = hit.unwrap();
        assert!((h.distance - 4.0).abs() < 0.2);
    }

    #[test]
    fn surface_kind_catalog() {
        assert_eq!(SurfaceKind::all().len(), 13);
        for kind in SurfaceKind::all() {
            let mesh = kind.tessellate(8, 8);
            assert!(mesh.vertex_count() > 0);
            assert!(mesh.triangle_count() > 0);
        }
    }

    #[test]
    fn function_surface() {
        let fs = FunctionSurface::new(|u, v| Vec3::new(u, v, 0.0));
        let p = fs.evaluate(0.5, 0.5);
        assert!((p.x - 0.5).abs() < 1e-5);
    }

    #[test]
    fn wireframe_edges() {
        let s = Sphere::default();
        let mesh = SurfaceMesh::tessellate(&s, 4, 4);
        let edges = mesh.wireframe_edges();
        assert!(!edges.is_empty());
    }

    #[test]
    fn subdivide_mesh() {
        let s = Sphere::default();
        let mut mesh = SurfaceMesh::tessellate(&s, 4, 4);
        let original_tris = mesh.triangle_count();
        mesh.subdivide();
        assert_eq!(mesh.triangle_count(), original_tris * 4);
    }

    #[test]
    fn flat_shaded_mesh() {
        let s = Torus::default();
        let mut mesh = SurfaceMesh::tessellate(&s, 8, 8);
        let tri_count = mesh.triangle_count();
        mesh.make_flat_shaded();
        assert_eq!(mesh.vertex_count(), tri_count * 3);
    }

    #[test]
    fn iso_curves_extraction() {
        let s = Sphere::default();
        let curve = IsoCurves::at_constant_u(&s, 0.0, 32);
        assert_eq!(curve.len(), 32);
    }
}
