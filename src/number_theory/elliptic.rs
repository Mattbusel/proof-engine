//! Elliptic curves over the reals: point arithmetic, rendering, and group law.

use glam::{Vec2, Vec3, Vec4};

/// An elliptic curve in short Weierstrass form: y^2 = x^3 + ax + b.
#[derive(Debug, Clone, Copy)]
pub struct EllipticCurve {
    pub a: f64,
    pub b: f64,
}

/// A point on an elliptic curve (or the point at infinity).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurvePoint {
    Infinity,
    Point(f64, f64),
}

impl EllipticCurve {
    pub fn new(a: f64, b: f64) -> Self {
        Self { a, b }
    }

    /// Discriminant: -16(4a^3 + 27b^2). Non-zero means the curve is non-singular.
    pub fn discriminant(&self) -> f64 {
        -16.0 * (4.0 * self.a.powi(3) + 27.0 * self.b.powi(2))
    }

    /// Check whether the curve is non-singular.
    pub fn is_non_singular(&self) -> bool {
        self.discriminant().abs() > 1e-12
    }

    /// Evaluate the right-hand side: x^3 + ax + b.
    pub fn rhs(&self, x: f64) -> f64 {
        x * x * x + self.a * x + self.b
    }

    /// Check if a point lies on the curve (within tolerance).
    pub fn is_on_curve(&self, point: CurvePoint) -> bool {
        match point {
            CurvePoint::Infinity => true,
            CurvePoint::Point(x, y) => {
                let lhs = y * y;
                let rhs = self.rhs(x);
                (lhs - rhs).abs() < 1e-8
            }
        }
    }

    /// Add two points on the curve (elliptic curve group law).
    pub fn point_add(&self, p: CurvePoint, q: CurvePoint) -> CurvePoint {
        match (p, q) {
            (CurvePoint::Infinity, _) => q,
            (_, CurvePoint::Infinity) => p,
            (CurvePoint::Point(x1, y1), CurvePoint::Point(x2, y2)) => {
                // Check if P = -Q (same x, opposite y)
                if (x1 - x2).abs() < 1e-12 && (y1 + y2).abs() < 1e-12 {
                    return CurvePoint::Infinity;
                }

                let m = if (x1 - x2).abs() < 1e-12 {
                    // Point doubling: m = (3x1^2 + a) / (2y1)
                    if y1.abs() < 1e-12 {
                        return CurvePoint::Infinity;
                    }
                    (3.0 * x1 * x1 + self.a) / (2.0 * y1)
                } else {
                    // General addition: m = (y2 - y1) / (x2 - x1)
                    (y2 - y1) / (x2 - x1)
                };

                let x3 = m * m - x1 - x2;
                let y3 = m * (x1 - x3) - y1;
                CurvePoint::Point(x3, y3)
            }
        }
    }

    /// Scalar multiplication: compute n * P using double-and-add.
    pub fn scalar_multiply(&self, n: i64, p: CurvePoint) -> CurvePoint {
        if n == 0 {
            return CurvePoint::Infinity;
        }
        let (mut k, point) = if n < 0 {
            (-n as u64, self.negate(p))
        } else {
            (n as u64, p)
        };

        let mut result = CurvePoint::Infinity;
        let mut base = point;
        while k > 0 {
            if k & 1 == 1 {
                result = self.point_add(result, base);
            }
            base = self.point_add(base, base);
            k >>= 1;
        }
        result
    }

    /// Negate a point: (x, y) -> (x, -y).
    pub fn negate(&self, p: CurvePoint) -> CurvePoint {
        match p {
            CurvePoint::Infinity => CurvePoint::Infinity,
            CurvePoint::Point(x, y) => CurvePoint::Point(x, -y),
        }
    }

    /// Sample points on the curve for rendering: returns both upper and lower branches.
    pub fn sample_curve(&self, x_min: f64, x_max: f64, steps: usize) -> Vec<Vec2> {
        let mut points = Vec::new();
        let dx = (x_max - x_min) / steps as f64;
        for i in 0..=steps {
            let x = x_min + i as f64 * dx;
            let rhs = self.rhs(x);
            if rhs >= 0.0 {
                let y = rhs.sqrt();
                points.push(Vec2::new(x as f32, y as f32));
                points.push(Vec2::new(x as f32, -y as f32));
            }
        }
        points
    }

    /// Find a point on the curve near x by solving y^2 = rhs.
    pub fn point_at_x(&self, x: f64) -> Option<CurvePoint> {
        let rhs = self.rhs(x);
        if rhs < 0.0 {
            None
        } else {
            Some(CurvePoint::Point(x, rhs.sqrt()))
        }
    }
}

// ─── Renderer ───────────────────────────────────────────────────────────────

/// Renders the elliptic curve and point operations as glyph paths.
pub struct EllipticCurveRenderer {
    pub curve: EllipticCurve,
    pub origin: Vec3,
    pub scale: f32,
    pub x_range: (f64, f64),
}

pub struct CurveGlyph {
    pub position: Vec3,
    pub color: Vec4,
    pub character: char,
}

impl EllipticCurveRenderer {
    pub fn new(curve: EllipticCurve, origin: Vec3, scale: f32, x_range: (f64, f64)) -> Self {
        Self { curve, origin, scale, x_range }
    }

    /// Render the curve itself as a series of glyphs.
    pub fn render_curve(&self, steps: usize) -> Vec<CurveGlyph> {
        let points = self.curve.sample_curve(self.x_range.0, self.x_range.1, steps);
        points
            .iter()
            .map(|p| {
                let pos = self.origin + Vec3::new(p.x * self.scale, p.y * self.scale, 0.0);
                let t = ((p.x - self.x_range.0 as f32)
                    / (self.x_range.1 - self.x_range.0) as f32)
                    .clamp(0.0, 1.0);
                CurveGlyph {
                    position: pos,
                    color: Vec4::new(0.2, 0.8, t, 1.0),
                    character: '.',
                }
            })
            .collect()
    }

    /// Render a point addition: P, Q, and P+Q with connecting lines.
    pub fn render_addition(
        &self,
        p: CurvePoint,
        q: CurvePoint,
    ) -> Vec<CurveGlyph> {
        let mut glyphs = Vec::new();

        let pq = self.curve.point_add(p, q);

        let point_data = [
            (p, Vec4::new(1.0, 0.3, 0.3, 1.0), 'P'),
            (q, Vec4::new(0.3, 1.0, 0.3, 1.0), 'Q'),
            (pq, Vec4::new(0.3, 0.3, 1.0, 1.0), 'R'),
        ];

        for &(pt, color, ch) in &point_data {
            if let CurvePoint::Point(x, y) = pt {
                glyphs.push(CurveGlyph {
                    position: self.origin
                        + Vec3::new(x as f32 * self.scale, y as f32 * self.scale, 0.0),
                    color,
                    character: ch,
                });
            }
        }

        // Add line from P to Q
        if let (CurvePoint::Point(x1, y1), CurvePoint::Point(x2, y2)) = (p, q) {
            let line_steps = 20;
            for i in 0..=line_steps {
                let t = i as f64 / line_steps as f64;
                let x = x1 + t * (x2 - x1);
                let y = y1 + t * (y2 - y1);
                glyphs.push(CurveGlyph {
                    position: self.origin
                        + Vec3::new(x as f32 * self.scale, y as f32 * self.scale, 0.0),
                    color: Vec4::new(0.5, 0.5, 0.5, 0.5),
                    character: '-',
                });
            }
        }

        glyphs
    }

    /// Render scalar multiples: P, 2P, 3P, ..., nP.
    pub fn render_multiples(&self, p: CurvePoint, n: i64) -> Vec<CurveGlyph> {
        let mut glyphs = Vec::new();
        for k in 1..=n {
            let kp = self.curve.scalar_multiply(k, p);
            if let CurvePoint::Point(x, y) = kp {
                let t = k as f32 / n as f32;
                glyphs.push(CurveGlyph {
                    position: self.origin
                        + Vec3::new(x as f32 * self.scale, y as f32 * self.scale, 0.0),
                    color: Vec4::new(t, 0.5, 1.0 - t, 1.0),
                    character: std::char::from_digit(k as u32 % 10, 10).unwrap_or('*'),
                });
            }
        }
        glyphs
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    fn point_approx_eq(p: CurvePoint, q: CurvePoint, eps: f64) -> bool {
        match (p, q) {
            (CurvePoint::Infinity, CurvePoint::Infinity) => true,
            (CurvePoint::Point(x1, y1), CurvePoint::Point(x2, y2)) => {
                approx(x1, x2, eps) && approx(y1, y2, eps)
            }
            _ => false,
        }
    }

    #[test]
    fn discriminant() {
        // y^2 = x^3 - x (a=-1, b=0): disc = -16(4*(-1)^3 + 0) = -16*(-4) = 64
        let e = EllipticCurve::new(-1.0, 0.0);
        assert!(approx(e.discriminant(), 64.0, 1e-10));
        assert!(e.is_non_singular());
    }

    #[test]
    fn on_curve() {
        let e = EllipticCurve::new(-1.0, 1.0); // y^2 = x^3 - x + 1
        // x=0: y^2 = 1, y = 1
        assert!(e.is_on_curve(CurvePoint::Point(0.0, 1.0)));
        assert!(e.is_on_curve(CurvePoint::Point(0.0, -1.0)));
        assert!(e.is_on_curve(CurvePoint::Infinity));
        assert!(!e.is_on_curve(CurvePoint::Point(0.0, 0.5)));
    }

    #[test]
    fn identity_element() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let p = CurvePoint::Point(0.0, 1.0);
        // P + O = P
        assert!(point_approx_eq(e.point_add(p, CurvePoint::Infinity), p, 1e-10));
        // O + P = P
        assert!(point_approx_eq(e.point_add(CurvePoint::Infinity, p), p, 1e-10));
    }

    #[test]
    fn inverse_element() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let p = CurvePoint::Point(0.0, 1.0);
        let neg_p = e.negate(p);
        // P + (-P) = O
        assert_eq!(e.point_add(p, neg_p), CurvePoint::Infinity);
    }

    #[test]
    fn point_addition() {
        let e = EllipticCurve::new(-1.0, 1.0); // y^2 = x^3 - x + 1
        let p = CurvePoint::Point(0.0, 1.0);
        let q = CurvePoint::Point(1.0, 1.0);
        let r = e.point_add(p, q);
        // Verify result is on curve
        assert!(e.is_on_curve(r), "P+Q should be on the curve");
    }

    #[test]
    fn point_doubling() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let p = CurvePoint::Point(0.0, 1.0);
        let twop = e.point_add(p, p);
        assert!(e.is_on_curve(twop), "2P should be on the curve");
    }

    #[test]
    fn associativity() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let p = CurvePoint::Point(0.0, 1.0);
        let q = CurvePoint::Point(1.0, 1.0);
        // (P+Q)+P vs P+(Q+P)
        let lhs = e.point_add(e.point_add(p, q), p);
        let rhs = e.point_add(p, e.point_add(q, p));
        assert!(
            point_approx_eq(lhs, rhs, 1e-8),
            "associativity failed: {:?} vs {:?}",
            lhs,
            rhs
        );
    }

    #[test]
    fn commutativity() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let p = CurvePoint::Point(0.0, 1.0);
        let q = CurvePoint::Point(1.0, 1.0);
        let pq = e.point_add(p, q);
        let qp = e.point_add(q, p);
        assert!(point_approx_eq(pq, qp, 1e-10));
    }

    #[test]
    fn scalar_multiply_test() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let p = CurvePoint::Point(0.0, 1.0);
        // 0*P = O
        assert_eq!(e.scalar_multiply(0, p), CurvePoint::Infinity);
        // 1*P = P
        assert!(point_approx_eq(e.scalar_multiply(1, p), p, 1e-10));
        // 2*P = P+P
        assert!(point_approx_eq(
            e.scalar_multiply(2, p),
            e.point_add(p, p),
            1e-10
        ));
        // 3*P = P+P+P
        assert!(point_approx_eq(
            e.scalar_multiply(3, p),
            e.point_add(e.point_add(p, p), p),
            1e-8
        ));
    }

    #[test]
    fn scalar_multiply_negative() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let p = CurvePoint::Point(0.0, 1.0);
        let neg_p = e.negate(p);
        assert!(point_approx_eq(e.scalar_multiply(-1, p), neg_p, 1e-10));
        // P + (-P) = O
        let sum = e.point_add(e.scalar_multiply(3, p), e.scalar_multiply(-3, p));
        assert_eq!(sum, CurvePoint::Infinity);
    }

    #[test]
    fn sample_curve_nonempty() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let pts = e.sample_curve(-2.0, 3.0, 100);
        assert!(!pts.is_empty());
    }

    #[test]
    fn renderer_curve() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let r = EllipticCurveRenderer::new(e, Vec3::ZERO, 1.0, (-2.0, 3.0));
        let glyphs = r.render_curve(50);
        assert!(!glyphs.is_empty());
    }

    #[test]
    fn renderer_addition() {
        let e = EllipticCurve::new(-1.0, 1.0);
        let r = EllipticCurveRenderer::new(e, Vec3::ZERO, 1.0, (-2.0, 3.0));
        let p = CurvePoint::Point(0.0, 1.0);
        let q = CurvePoint::Point(1.0, 1.0);
        let glyphs = r.render_addition(p, q);
        assert!(glyphs.len() >= 3); // at least P, Q, R
    }

    #[test]
    fn singular_curve() {
        // y^2 = x^3 (a=0, b=0), disc = 0
        let e = EllipticCurve::new(0.0, 0.0);
        assert!(!e.is_non_singular());
    }
}
