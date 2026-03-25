//! Bezier and NURBS surface rendering.

use glam::{Vec2, Vec3, Vec4};
use super::{GeoMesh, parametric::ParametricSurface};

/// A 3D control point with optional weight (for NURBS).
#[derive(Debug, Clone, Copy)]
pub struct ControlPoint {
    pub position: Vec3,
    pub weight: f32,
}

impl ControlPoint {
    pub fn new(pos: Vec3) -> Self { Self { position: pos, weight: 1.0 } }
    pub fn weighted(pos: Vec3, w: f32) -> Self { Self { position: pos, weight: w } }
}

/// Bicubic Bezier surface patch (4×4 control points).
#[derive(Debug, Clone)]
pub struct BezierSurface {
    pub control_points: [[ControlPoint; 4]; 4],
}

impl BezierSurface {
    pub fn new(points: [[Vec3; 4]; 4]) -> Self {
        let mut cp = [[ControlPoint::new(Vec3::ZERO); 4]; 4];
        for i in 0..4 { for j in 0..4 { cp[i][j] = ControlPoint::new(points[i][j]); } }
        Self { control_points: cp }
    }

    fn bernstein(t: f32) -> [f32; 4] {
        let s = 1.0 - t;
        [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t]
    }
}

impl ParametricSurface for BezierSurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let bu = Self::bernstein(u);
        let bv = Self::bernstein(v);
        let mut pos = Vec3::ZERO;
        for i in 0..4 {
            for j in 0..4 {
                pos += self.control_points[i][j].position * bu[i] * bv[j];
            }
        }
        pos
    }
}

/// NURBS surface of arbitrary degree.
#[derive(Debug, Clone)]
pub struct NurbsSurface {
    pub control_points: Vec<Vec<ControlPoint>>,
    pub knots_u: Vec<f32>,
    pub knots_v: Vec<f32>,
    pub degree_u: usize,
    pub degree_v: usize,
}

impl NurbsSurface {
    pub fn new(
        control_points: Vec<Vec<ControlPoint>>,
        knots_u: Vec<f32>, knots_v: Vec<f32>,
        degree_u: usize, degree_v: usize,
    ) -> Self {
        Self { control_points, knots_u, knots_v, degree_u, degree_v }
    }

    fn basis(knots: &[f32], i: usize, degree: usize, t: f32) -> f32 {
        if degree == 0 {
            return if t >= knots[i] && t < knots[i + 1] { 1.0 } else { 0.0 };
        }
        let mut result = 0.0;
        let d1 = knots[i + degree] - knots[i];
        if d1.abs() > 1e-10 {
            result += (t - knots[i]) / d1 * Self::basis(knots, i, degree - 1, t);
        }
        let d2 = knots[i + degree + 1] - knots[i + 1];
        if d2.abs() > 1e-10 {
            result += (knots[i + degree + 1] - t) / d2 * Self::basis(knots, i + 1, degree - 1, t);
        }
        result
    }
}

impl ParametricSurface for NurbsSurface {
    fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let rows = self.control_points.len();
        let cols = if rows > 0 { self.control_points[0].len() } else { 0 };

        let mut numerator = Vec3::ZERO;
        let mut denominator = 0.0;

        for i in 0..rows {
            let bu = Self::basis(&self.knots_u, i, self.degree_u, u.clamp(0.001, 0.999));
            for j in 0..cols {
                let bv = Self::basis(&self.knots_v, j, self.degree_v, v.clamp(0.001, 0.999));
                let cp = &self.control_points[i][j];
                let w = bu * bv * cp.weight;
                numerator += cp.position * w;
                denominator += w;
            }
        }

        if denominator.abs() > 1e-10 { numerator / denominator } else { Vec3::ZERO }
    }
}

/// Bezier curve (arbitrary degree).
#[derive(Debug, Clone)]
pub struct BezierCurve {
    pub control_points: Vec<Vec3>,
}

impl BezierCurve {
    pub fn new(points: Vec<Vec3>) -> Self { Self { control_points: points } }

    /// Evaluate at parameter t ∈ [0, 1] using De Casteljau's algorithm.
    pub fn evaluate(&self, t: f32) -> Vec3 {
        let n = self.control_points.len();
        if n == 0 { return Vec3::ZERO; }
        let mut temp = self.control_points.clone();
        for k in 1..n {
            for i in 0..n - k {
                temp[i] = temp[i] * (1.0 - t) + temp[i + 1] * t;
            }
        }
        temp[0]
    }

    /// Tangent at parameter t.
    pub fn tangent(&self, t: f32) -> Vec3 {
        let eps = 1e-4;
        (self.evaluate((t + eps).min(1.0)) - self.evaluate((t - eps).max(0.0))).normalize_or_zero()
    }

    /// Split curve at parameter t into two curves.
    pub fn split(&self, t: f32) -> (BezierCurve, BezierCurve) {
        let n = self.control_points.len();
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        let mut temp = self.control_points.clone();

        left.push(temp[0]);
        right.push(temp[n - 1]);

        for k in 1..n {
            for i in 0..n - k {
                temp[i] = temp[i] * (1.0 - t) + temp[i + 1] * t;
            }
            left.push(temp[0]);
            right.push(temp[n - 1 - k]);
        }

        right.reverse();
        (BezierCurve::new(left), BezierCurve::new(right))
    }

    /// Arc length approximation.
    pub fn arc_length(&self, segments: usize) -> f32 {
        let mut length = 0.0;
        let mut prev = self.evaluate(0.0);
        for i in 1..=segments {
            let t = i as f32 / segments as f32;
            let p = self.evaluate(t);
            length += (p - prev).length();
            prev = p;
        }
        length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_curve_endpoints() {
        let curve = BezierCurve::new(vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0)]);
        assert!((curve.evaluate(0.0) - Vec3::ZERO).length() < 1e-5);
        assert!((curve.evaluate(1.0) - Vec3::new(1.0, 1.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn bezier_surface_evaluates() {
        let points = [[Vec3::ZERO; 4]; 4];
        let surface = BezierSurface::new(points);
        let p = surface.evaluate(0.5, 0.5);
        assert!((p - Vec3::ZERO).length() < 1e-5);
    }

    #[test]
    fn bezier_split_preserves_curve() {
        let curve = BezierCurve::new(vec![Vec3::ZERO, Vec3::new(1.0, 2.0, 0.0), Vec3::X * 2.0]);
        let (left, right) = curve.split(0.5);
        let mid_orig = curve.evaluate(0.5);
        let mid_left = left.evaluate(1.0);
        assert!((mid_orig - mid_left).length() < 1e-4);
    }
}
