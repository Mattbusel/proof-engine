//! Parametric surface evaluation — any f(u,v) → Vec3 mapped to glyph grids.

use glam::{Vec2, Vec3, Vec4};
use super::GeoMesh;

/// A parametric surface defined by a function f(u,v) → position.
pub trait ParametricSurface: Send + Sync {
    /// Evaluate the surface position at parameters (u, v) ∈ [0, 1]².
    fn evaluate(&self, u: f32, v: f32) -> Vec3;

    /// Surface normal at (u, v), computed via finite differences if not overridden.
    fn normal(&self, u: f32, v: f32) -> Vec3 {
        let eps = 1e-4;
        let du = self.evaluate(u + eps, v) - self.evaluate(u - eps, v);
        let dv = self.evaluate(u, v + eps) - self.evaluate(u, v - eps);
        du.cross(dv).normalize_or_zero()
    }

    /// Optional color at (u, v).
    fn color(&self, _u: f32, _v: f32) -> Vec4 { Vec4::ONE }
}

/// Sample point on a parametric surface.
#[derive(Debug, Clone)]
pub struct SurfaceSample {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
}

/// A grid of samples from a parametric surface.
#[derive(Debug, Clone)]
pub struct SurfaceGrid {
    pub samples: Vec<SurfaceSample>,
    pub u_count: usize,
    pub v_count: usize,
}

impl SurfaceGrid {
    /// Sample a parametric surface on a regular u×v grid.
    pub fn sample(surface: &dyn ParametricSurface, u_count: usize, v_count: usize) -> Self {
        let mut samples = Vec::with_capacity(u_count * v_count);
        for vi in 0..v_count {
            let v = vi as f32 / (v_count - 1).max(1) as f32;
            for ui in 0..u_count {
                let u = ui as f32 / (u_count - 1).max(1) as f32;
                samples.push(SurfaceSample {
                    position: surface.evaluate(u, v),
                    normal: surface.normal(u, v),
                    uv: Vec2::new(u, v),
                    color: surface.color(u, v),
                });
            }
        }
        Self { samples, u_count, v_count }
    }

    /// Convert to a triangle mesh.
    pub fn to_mesh(&self) -> GeoMesh {
        let mut mesh = GeoMesh::new();
        for s in &self.samples {
            mesh.add_vertex(s.position, s.normal, s.uv);
            mesh.colors.pop(); // remove default white
            mesh.colors.push(s.color);
        }
        for vi in 0..self.v_count - 1 {
            for ui in 0..self.u_count - 1 {
                let i00 = (vi * self.u_count + ui) as u32;
                let i10 = i00 + 1;
                let i01 = i00 + self.u_count as u32;
                let i11 = i01 + 1;
                mesh.add_triangle(i00, i10, i11);
                mesh.add_triangle(i00, i11, i01);
            }
        }
        mesh
    }

    pub fn get(&self, u_idx: usize, v_idx: usize) -> &SurfaceSample {
        &self.samples[v_idx * self.u_count + u_idx]
    }
}

// ── Built-in parametric surfaces ────────────────────────────────────────────

/// Sphere of given radius.
pub struct Sphere { pub radius: f32 }

impl ParametricSurface for Sphere {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * std::f32::consts::TAU;
        let phi = v * std::f32::consts::PI;
        Vec3::new(
            self.radius * phi.sin() * theta.cos(),
            self.radius * phi.cos(),
            self.radius * phi.sin() * theta.sin(),
        )
    }
}

/// Torus with major radius R and minor radius r.
pub struct Torus { pub major: f32, pub minor: f32 }

impl ParametricSurface for Torus {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * std::f32::consts::TAU;
        let phi = v * std::f32::consts::TAU;
        Vec3::new(
            (self.major + self.minor * phi.cos()) * theta.cos(),
            self.minor * phi.sin(),
            (self.major + self.minor * phi.cos()) * theta.sin(),
        )
    }
}

/// Klein bottle (immersed in 3D).
pub struct KleinBottle { pub scale: f32 }

impl ParametricSurface for KleinBottle {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let u = u * std::f32::consts::TAU;
        let v = v * std::f32::consts::TAU;
        let s = self.scale;
        let r = 4.0 * (1.0 - (u / 2.0).cos());
        if u < std::f32::consts::PI {
            Vec3::new(
                s * (6.0 * (1.0 + u.sin()) + r * (u / 2.0).cos() * v.cos()),
                s * 16.0 * u.sin(),
                s * r * v.sin(),
            )
        } else {
            Vec3::new(
                s * (6.0 * (1.0 + u.sin()) - r * (u / 2.0).cos() * v.cos()),
                s * 16.0 * u.sin(),
                s * r * v.sin(),
            )
        }
    }
}

/// Möbius strip.
pub struct MobiusStrip { pub radius: f32, pub width: f32 }

impl ParametricSurface for MobiusStrip {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let theta = u * std::f32::consts::TAU;
        let s = (v - 0.5) * self.width;
        let half_twist = theta / 2.0;
        Vec3::new(
            (self.radius + s * half_twist.cos()) * theta.cos(),
            (self.radius + s * half_twist.cos()) * theta.sin(),
            s * half_twist.sin(),
        )
    }
}

/// Trefoil knot tube.
pub struct TrefoilKnot { pub radius: f32, pub tube: f32 }

impl ParametricSurface for TrefoilKnot {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let t = u * std::f32::consts::TAU;
        let phi = v * std::f32::consts::TAU;
        let r = self.radius;

        // Trefoil curve
        let cx = r * ((2.0 + (3.0 * t).cos()) * t.cos());
        let cy = r * ((2.0 + (3.0 * t).cos()) * t.sin());
        let cz = r * (3.0 * t).sin();

        // Tangent and normal frame (Frenet)
        let eps = 0.001;
        let t2 = t + eps;
        let cx2 = r * ((2.0 + (3.0 * t2).cos()) * t2.cos());
        let cy2 = r * ((2.0 + (3.0 * t2).cos()) * t2.sin());
        let cz2 = r * (3.0 * t2).sin();
        let tangent = Vec3::new(cx2 - cx, cy2 - cy, cz2 - cz).normalize_or_zero();
        let up = Vec3::Y;
        let binormal = tangent.cross(up).normalize_or_zero();
        let normal = binormal.cross(tangent).normalize_or_zero();

        let offset = binormal * (self.tube * phi.cos()) + normal * (self.tube * phi.sin());
        Vec3::new(cx, cy, cz) + offset
    }
}

/// Custom surface from a closure.
pub struct CustomSurface<F: Fn(f32, f32) -> Vec3 + Send + Sync> {
    pub func: F,
}

impl<F: Fn(f32, f32) -> Vec3 + Send + Sync> ParametricSurface for CustomSurface<F> {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 { (self.func)(u, v) }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_radius() {
        let s = Sphere { radius: 2.0 };
        let p = s.evaluate(0.0, 0.5); // equator
        assert!((p.length() - 2.0).abs() < 0.01);
    }

    #[test]
    fn torus_topology() {
        let t = Torus { major: 3.0, minor: 1.0 };
        let grid = SurfaceGrid::sample(&t, 16, 16);
        let mesh = grid.to_mesh();
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn klein_bottle_generates() {
        let k = KleinBottle { scale: 1.0 };
        let grid = SurfaceGrid::sample(&k, 10, 10);
        assert_eq!(grid.samples.len(), 100);
    }

    #[test]
    fn mobius_strip_generates() {
        let m = MobiusStrip { radius: 2.0, width: 1.0 };
        let p = m.evaluate(0.0, 0.5);
        assert!((p.length() - 2.0).abs() < 0.1);
    }

    #[test]
    fn surface_grid_mesh_has_correct_counts() {
        let s = Sphere { radius: 1.0 };
        let grid = SurfaceGrid::sample(&s, 8, 8);
        let mesh = grid.to_mesh();
        assert_eq!(mesh.vertex_count(), 64);
        assert_eq!(mesh.triangle_count(), 7 * 7 * 2);
    }
}
