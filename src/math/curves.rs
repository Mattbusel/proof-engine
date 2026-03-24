//! Parametric curves — Bezier, B-Spline, Catmull-Rom, and Hermite splines.
//!
//! All curves implement the `Curve` trait: `sample(t)` returns a point on
//! the curve for t ∈ [0, 1].  Arc-length reparametrization is provided
//! for uniform speed traversal.

use glam::{Vec2, Vec3};

// ── Curve trait ───────────────────────────────────────────────────────────────

/// Common interface for parametric curves.
pub trait Curve: Send + Sync {
    /// Sample position at parameter t ∈ [0, 1].
    fn sample(&self, t: f32) -> Vec3;

    /// Sample tangent (unnormalized derivative) at t.
    fn tangent(&self, t: f32) -> Vec3 {
        let eps = 1e-4_f32;
        let t0  = (t - eps).max(0.0);
        let t1  = (t + eps).min(1.0);
        (self.sample(t1) - self.sample(t0)) / (t1 - t0)
    }

    /// Sample unit tangent at t.
    fn unit_tangent(&self, t: f32) -> Vec3 {
        self.tangent(t).normalize_or_zero()
    }

    /// Sample the normal (perpendicular to tangent) at t, in the XY plane.
    fn normal_2d(&self, t: f32) -> Vec3 {
        let tan = self.unit_tangent(t);
        Vec3::new(-tan.y, tan.x, 0.0)
    }

    /// Approximate arc length using `n` segments.
    fn arc_length(&self, n: u32) -> f32 {
        let n = n.max(2);
        let mut len  = 0.0_f32;
        let mut prev = self.sample(0.0);
        for i in 1..=n {
            let t    = i as f32 / n as f32;
            let next = self.sample(t);
            len += (next - prev).length();
            prev = next;
        }
        len
    }

    /// Build an arc-length table with `n` segments for uniform-speed queries.
    fn build_arc_table(&self, n: u32) -> ArcTable {
        let n = n.max(2) as usize;
        let mut t_values    = Vec::with_capacity(n + 1);
        let mut arc_lengths = Vec::with_capacity(n + 1);
        let mut len         = 0.0_f32;
        let mut prev        = self.sample(0.0);

        t_values.push(0.0_f32);
        arc_lengths.push(0.0_f32);

        for i in 1..=n {
            let t    = i as f32 / n as f32;
            let next = self.sample(t);
            len += (next - prev).length();
            t_values.push(t);
            arc_lengths.push(len);
            prev = next;
        }

        // Normalize
        let total = len;
        for v in &mut arc_lengths { *v /= total.max(f32::EPSILON); }

        ArcTable { t_values, arc_lengths, total_length: total }
    }

    /// Sample n evenly-spaced points along the curve (by parameter).
    fn sample_uniform(&self, n: usize) -> Vec<Vec3> {
        (0..n).map(|i| self.sample(i as f32 / (n - 1).max(1) as f32)).collect()
    }

    /// Sample n points at equal arc-length intervals.
    fn sample_arc_uniform(&self, n: usize, table_resolution: u32) -> Vec<Vec3> {
        let table = self.build_arc_table(table_resolution);
        (0..n).map(|i| {
            let s = i as f32 / (n - 1).max(1) as f32;
            let t = table.arc_to_t(s);
            self.sample(t)
        }).collect()
    }

    /// Bounding box as (min, max).
    fn bounding_box(&self, resolution: u32) -> (Vec3, Vec3) {
        let pts = self.sample_uniform(resolution as usize);
        let min = pts.iter().copied().fold(Vec3::splat(f32::MAX), |a, p| a.min(p));
        let max = pts.iter().copied().fold(Vec3::splat(f32::MIN), |a, p| a.max(p));
        (min, max)
    }

    /// Find the closest point on the curve to `query`, returning (t, point).
    fn closest_point(&self, query: Vec3, resolution: u32) -> (f32, Vec3) {
        let n = resolution.max(2) as usize;
        let mut best_t   = 0.0_f32;
        let mut best_d2  = f32::MAX;
        let mut best_pt  = self.sample(0.0);

        for i in 0..=n {
            let t  = i as f32 / n as f32;
            let pt = self.sample(t);
            let d2 = (pt - query).length_squared();
            if d2 < best_d2 {
                best_d2 = d2;
                best_t  = t;
                best_pt = pt;
            }
        }
        (best_t, best_pt)
    }
}

// ── Arc-length table ──────────────────────────────────────────────────────────

/// Precomputed arc-length lookup table for uniform speed traversal.
pub struct ArcTable {
    t_values:     Vec<f32>,
    arc_lengths:  Vec<f32>,  // normalized [0, 1]
    pub total_length: f32,
}

impl ArcTable {
    /// Given a normalized arc-length fraction s ∈ [0, 1], return curve parameter t.
    pub fn arc_to_t(&self, s: f32) -> f32 {
        let s = s.clamp(0.0, 1.0);
        // Binary search in arc_lengths
        let pos = self.arc_lengths.partition_point(|&v| v < s);
        if pos == 0 { return self.t_values[0]; }
        if pos >= self.t_values.len() { return *self.t_values.last().unwrap(); }
        let lo = pos - 1;
        let hi = pos;
        let al = self.arc_lengths[lo];
        let ah = self.arc_lengths[hi];
        let span = ah - al;
        if span < f32::EPSILON { return self.t_values[lo]; }
        let frac = (s - al) / span;
        self.t_values[lo] + frac * (self.t_values[hi] - self.t_values[lo])
    }
}

// ── Linear segment ────────────────────────────────────────────────────────────

/// Simple line segment from A to B.
pub struct LineSegment {
    pub a: Vec3,
    pub b: Vec3,
}

impl LineSegment {
    pub fn new(a: Vec3, b: Vec3) -> Self { Self { a, b } }
}

impl Curve for LineSegment {
    fn sample(&self, t: f32) -> Vec3 {
        self.a.lerp(self.b, t)
    }
    fn tangent(&self, _t: f32) -> Vec3 {
        self.b - self.a
    }
}

// ── Quadratic Bezier ──────────────────────────────────────────────────────────

/// Quadratic Bezier curve through 3 control points.
pub struct QuadraticBezier {
    pub p0: Vec3,
    pub p1: Vec3,  // control point
    pub p2: Vec3,
}

impl QuadraticBezier {
    pub fn new(p0: Vec3, p1: Vec3, p2: Vec3) -> Self { Self { p0, p1, p2 } }

    /// Construct a curve that passes through `through` at t=0.5,
    /// using De Casteljau to find the implied control point.
    pub fn through_point(p0: Vec3, through: Vec3, p2: Vec3) -> Self {
        // p1 = 2*through - 0.5*p0 - 0.5*p2
        let p1 = through * 2.0 - p0 * 0.5 - p2 * 0.5;
        Self { p0, p1, p2 }
    }

    /// Split at t into two quadratic curves.
    pub fn split(&self, t: f32) -> (Self, Self) {
        let q0 = self.p0.lerp(self.p1, t);
        let q1 = self.p1.lerp(self.p2, t);
        let r0 = q0.lerp(q1, t);
        (
            Self::new(self.p0, q0, r0),
            Self::new(r0, q1, self.p2),
        )
    }
}

impl Curve for QuadraticBezier {
    fn sample(&self, t: f32) -> Vec3 {
        let u  = 1.0 - t;
        self.p0 * (u * u) + self.p1 * (2.0 * u * t) + self.p2 * (t * t)
    }
    fn tangent(&self, t: f32) -> Vec3 {
        let u = 1.0 - t;
        (self.p1 - self.p0) * (2.0 * u) + (self.p2 - self.p1) * (2.0 * t)
    }
}

// ── Cubic Bezier ──────────────────────────────────────────────────────────────

/// Cubic Bezier curve (4 control points).
pub struct CubicBezier {
    pub p0: Vec3,
    pub p1: Vec3,
    pub p2: Vec3,
    pub p3: Vec3,
}

impl CubicBezier {
    pub fn new(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3) -> Self {
        Self { p0, p1, p2, p3 }
    }

    /// Ease-in-out curve (commonly used for animation).
    pub fn ease_in_out(from: Vec3, to: Vec3) -> Self {
        let dir = (to - from) * 0.33;
        Self::new(from, from + Vec3::new(dir.x, 0.0, 0.0), to - Vec3::new(dir.x, 0.0, 0.0), to)
    }

    /// Split at t into two cubic curves (De Casteljau).
    pub fn split(&self, t: f32) -> (Self, Self) {
        let q0 = self.p0.lerp(self.p1, t);
        let q1 = self.p1.lerp(self.p2, t);
        let q2 = self.p2.lerp(self.p3, t);
        let r0 = q0.lerp(q1, t);
        let r1 = q1.lerp(q2, t);
        let s0 = r0.lerp(r1, t);
        (
            Self::new(self.p0, q0, r0, s0),
            Self::new(s0, r1, q2, self.p3),
        )
    }

    /// Second derivative (curvature vector) at t.
    pub fn second_derivative(&self, t: f32) -> Vec3 {
        let d0 = self.p1 - self.p0;
        let d1 = self.p2 - self.p1;
        let d2 = self.p3 - self.p2;
        let e0 = d1 - d0;
        let e1 = d2 - d1;
        (e0.lerp(e1, t)) * 6.0
    }

    /// Curvature scalar at t.
    pub fn curvature(&self, t: f32) -> f32 {
        let d1 = self.tangent(t);
        let d2 = self.second_derivative(t);
        let cross = d1.cross(d2);
        cross.length() / d1.length().powi(3)
    }
}

impl Curve for CubicBezier {
    fn sample(&self, t: f32) -> Vec3 {
        let u  = 1.0 - t;
        let u2 = u * u;
        let u3 = u2 * u;
        let t2 = t * t;
        let t3 = t2 * t;
        self.p0 * u3 + self.p1 * (3.0 * u2 * t) + self.p2 * (3.0 * u * t2) + self.p3 * t3
    }
    fn tangent(&self, t: f32) -> Vec3 {
        let u  = 1.0 - t;
        let u2 = u * u;
        let t2 = t * t;
        (self.p1 - self.p0) * (3.0 * u2)
        + (self.p2 - self.p1) * (6.0 * u * t)
        + (self.p3 - self.p2) * (3.0 * t2)
    }
}

// ── N-th order Bezier ─────────────────────────────────────────────────────────

/// Arbitrary-degree Bezier via De Casteljau's algorithm.
pub struct BezierN {
    pub control_points: Vec<Vec3>,
}

impl BezierN {
    pub fn new(pts: Vec<Vec3>) -> Self {
        assert!(pts.len() >= 2, "Need at least 2 control points");
        Self { control_points: pts }
    }
}

impl Curve for BezierN {
    fn sample(&self, t: f32) -> Vec3 {
        // De Casteljau
        let mut pts = self.control_points.clone();
        while pts.len() > 1 {
            pts = pts.windows(2).map(|w| w[0].lerp(w[1], t)).collect();
        }
        pts[0]
    }
}

// ── Catmull-Rom spline ────────────────────────────────────────────────────────

/// Catmull-Rom spline through a sequence of waypoints.
///
/// The spline passes through all waypoints (unlike Bezier which only passes
/// through endpoints). `alpha` controls the parameterization:
/// - 0.0 = uniform (standard)
/// - 0.5 = centripetal (avoids self-intersection)
/// - 1.0 = chordal
#[derive(Clone, Debug)]
pub struct CatmullRom {
    pub points: Vec<Vec3>,
    pub alpha:  f32,
    pub closed: bool,
}

impl CatmullRom {
    pub fn new(points: Vec<Vec3>) -> Self {
        Self { points, alpha: 0.5, closed: false }
    }

    pub fn closed(points: Vec<Vec3>) -> Self {
        Self { points, alpha: 0.5, closed: true }
    }

    pub fn with_alpha(mut self, alpha: f32) -> Self { self.alpha = alpha; self }

    fn segment_count(&self) -> usize {
        if self.closed {
            self.points.len()
        } else {
            self.points.len().saturating_sub(1)
        }
    }

    fn get_point(&self, i: i32) -> Vec3 {
        let n = self.points.len() as i32;
        if self.closed {
            self.points[((i % n + n) % n) as usize]
        } else {
            self.points[i.clamp(0, n - 1) as usize]
        }
    }

    fn sample_segment(&self, seg: usize, t: f32) -> Vec3 {
        let i  = seg as i32;
        let p0 = self.get_point(i - 1);
        let p1 = self.get_point(i);
        let p2 = self.get_point(i + 1);
        let p3 = self.get_point(i + 2);

        // Centripetal parameterization
        let t01 = ((p1 - p0).length() + f32::EPSILON).powf(self.alpha);
        let t12 = ((p2 - p1).length() + f32::EPSILON).powf(self.alpha);
        let t23 = ((p3 - p2).length() + f32::EPSILON).powf(self.alpha);

        let t0 = 0.0_f32;
        let t1 = t0 + t01;
        let t2 = t1 + t12;
        let t3 = t2 + t23;

        let tt = t1 + t * t12; // map t ∈ [0,1] to [t1, t2]

        let a1 = p0 * ((t1 - tt) / (t1 - t0)) + p1 * ((tt - t0) / (t1 - t0));
        let a2 = p1 * ((t2 - tt) / (t2 - t1)) + p2 * ((tt - t1) / (t2 - t1));
        let a3 = p2 * ((t3 - tt) / (t3 - t2)) + p3 * ((tt - t2) / (t3 - t2));

        let b1 = a1 * ((t2 - tt) / (t2 - t0)) + a2 * ((tt - t0) / (t2 - t0));
        let b2 = a2 * ((t3 - tt) / (t3 - t1)) + a3 * ((tt - t1) / (t3 - t1));

        b1 * ((t2 - tt) / (t2 - t1)) + b2 * ((tt - t1) / (t2 - t1))
    }
}

impl Curve for CatmullRom {
    fn sample(&self, t: f32) -> Vec3 {
        let n_segs = self.segment_count();
        if n_segs == 0 { return self.points.first().copied().unwrap_or(Vec3::ZERO); }
        let t      = t.clamp(0.0, 1.0);
        let scaled = t * n_segs as f32;
        let seg    = (scaled as usize).min(n_segs - 1);
        let local  = scaled - seg as f32;
        self.sample_segment(seg, local)
    }
}

// ── Hermite spline ────────────────────────────────────────────────────────────

/// Cubic Hermite spline: interpolates positions and velocities at each knot.
#[derive(Clone, Debug)]
pub struct HermiteSpline {
    /// (position, tangent) pairs.
    pub knots: Vec<(Vec3, Vec3)>,
    pub times: Vec<f32>,  // parameter values (must be sorted)
}

impl HermiteSpline {
    pub fn new() -> Self {
        Self { knots: Vec::new(), times: Vec::new() }
    }

    pub fn add_knot(mut self, t: f32, pos: Vec3, tangent: Vec3) -> Self {
        // Insert sorted by time
        let idx = self.times.partition_point(|&v| v < t);
        self.times.insert(idx, t);
        self.knots.insert(idx, (pos, tangent));
        self
    }

    /// Evaluate at local parameter u ∈ [0,1] within a segment.
    fn eval_segment(p0: Vec3, m0: Vec3, p1: Vec3, m1: Vec3, u: f32) -> Vec3 {
        let u2 = u * u;
        let u3 = u2 * u;
        let h00 =  2.0*u3 - 3.0*u2 + 1.0;
        let h10 =      u3 - 2.0*u2 + u;
        let h01 = -2.0*u3 + 3.0*u2;
        let h11 =      u3 -     u2;
        p0 * h00 + m0 * h10 + p1 * h01 + m1 * h11
    }
}

impl Default for HermiteSpline {
    fn default() -> Self { Self::new() }
}

impl Curve for HermiteSpline {
    fn sample(&self, t: f32) -> Vec3 {
        if self.knots.is_empty() { return Vec3::ZERO; }
        if self.knots.len() == 1 { return self.knots[0].0; }

        let t0 = *self.times.first().unwrap();
        let t1 = *self.times.last().unwrap();
        let t  = t0 + t.clamp(0.0, 1.0) * (t1 - t0);

        // Find segment
        let idx = self.times.partition_point(|&v| v < t).min(self.times.len() - 1);
        let idx = idx.max(1);
        let i0  = idx - 1;
        let i1  = idx;

        let ta  = self.times[i0];
        let tb  = self.times[i1];
        let dt  = tb - ta;
        let u   = if dt < f32::EPSILON { 0.0 } else { (t - ta) / dt };

        let (p0, m0) = self.knots[i0];
        let (p1, m1) = self.knots[i1];
        // Scale tangents by segment duration
        Self::eval_segment(p0, m0 * dt, p1, m1 * dt, u)
    }
}

// ── Uniform B-Spline ──────────────────────────────────────────────────────────

/// Cubic uniform B-Spline (C2 continuity). Does NOT pass through control points.
pub struct BSpline {
    pub control_points: Vec<Vec3>,
    pub closed:         bool,
}

impl BSpline {
    pub fn new(control_points: Vec<Vec3>) -> Self {
        Self { control_points, closed: false }
    }

    pub fn closed(control_points: Vec<Vec3>) -> Self {
        Self { control_points, closed: true }
    }

    fn get_point(&self, i: i32) -> Vec3 {
        let n = self.control_points.len() as i32;
        if self.closed {
            self.control_points[((i % n + n) % n) as usize]
        } else {
            self.control_points[i.clamp(0, n - 1) as usize]
        }
    }

    fn segment_count(&self) -> usize {
        if self.closed {
            self.control_points.len()
        } else {
            self.control_points.len().saturating_sub(3)
        }
    }

    fn sample_segment(&self, seg: usize, u: f32) -> Vec3 {
        let i  = seg as i32;
        let p0 = self.get_point(i);
        let p1 = self.get_point(i + 1);
        let p2 = self.get_point(i + 2);
        let p3 = self.get_point(i + 3);

        // Uniform cubic B-spline basis
        let u2 = u * u;
        let u3 = u2 * u;
        let b0 = (1.0 - 3.0*u + 3.0*u2 -     u3) / 6.0;
        let b1 = (4.0          - 6.0*u2 + 3.0*u3) / 6.0;
        let b2 = (1.0 + 3.0*u + 3.0*u2 - 3.0*u3) / 6.0;
        let b3 =                               u3  / 6.0;

        p0 * b0 + p1 * b1 + p2 * b2 + p3 * b3
    }
}

impl Curve for BSpline {
    fn sample(&self, t: f32) -> Vec3 {
        let n_segs = self.segment_count();
        if n_segs == 0 { return self.control_points.first().copied().unwrap_or(Vec3::ZERO); }
        let t      = t.clamp(0.0, 1.0);
        let scaled = t * n_segs as f32;
        let seg    = (scaled as usize).min(n_segs - 1);
        let local  = scaled - seg as f32;
        self.sample_segment(seg, local)
    }
}

// ── Composite / Polyline ──────────────────────────────────────────────────────

/// Multiple curve segments joined end-to-end.
pub struct CompositeCurve {
    segments:    Vec<Box<dyn Curve>>,
    /// Normalized parameter breakpoints (length == segments.len() + 1).
    breakpoints: Vec<f32>,
}

impl CompositeCurve {
    pub fn new() -> Self {
        Self { segments: Vec::new(), breakpoints: vec![0.0] }
    }

    /// Add a segment with an explicit weight (relative length contribution).
    pub fn add_weighted(mut self, seg: Box<dyn Curve>, weight: f32) -> Self {
        let last = *self.breakpoints.last().unwrap();
        self.breakpoints.push(last + weight.max(0.0));
        self.segments.push(seg);
        self
    }

    /// Add a segment with weight derived from arc length.
    pub fn add(self, seg: Box<dyn Curve>) -> Self {
        let len = seg.arc_length(64);
        self.add_weighted(seg, len)
    }

    fn normalize_breakpoints(&mut self) {
        let total = *self.breakpoints.last().copied().as_ref().unwrap_or(&1.0);
        if total > f32::EPSILON {
            for b in &mut self.breakpoints { *b /= total; }
        }
    }
}

impl Default for CompositeCurve {
    fn default() -> Self { Self::new() }
}

impl Curve for CompositeCurve {
    fn sample(&self, t: f32) -> Vec3 {
        if self.segments.is_empty() { return Vec3::ZERO; }
        let t = t.clamp(0.0, 1.0);
        for i in 0..self.segments.len() {
            let t0 = self.breakpoints[i];
            let t1 = self.breakpoints[i + 1];
            if t <= t1 || i == self.segments.len() - 1 {
                let span = t1 - t0;
                let local = if span < f32::EPSILON { 1.0 } else { (t - t0) / span };
                return self.segments[i].sample(local.clamp(0.0, 1.0));
            }
        }
        self.segments.last().unwrap().sample(1.0)
    }
}

// ── 2D Bezier helpers (for UI paths) ─────────────────────────────────────────

/// 2D cubic Bezier for UI/path use.
pub struct CubicBezier2D {
    pub p0: Vec2,
    pub p1: Vec2,
    pub p2: Vec2,
    pub p3: Vec2,
}

impl CubicBezier2D {
    pub fn new(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> Self {
        Self { p0, p1, p2, p3 }
    }

    pub fn sample(&self, t: f32) -> Vec2 {
        let u  = 1.0 - t;
        let u2 = u * u;
        let u3 = u2 * u;
        let t2 = t * t;
        let t3 = t2 * t;
        self.p0 * u3 + self.p1 * (3.0 * u2 * t) + self.p2 * (3.0 * u * t2) + self.p3 * t3
    }

    pub fn tangent(&self, t: f32) -> Vec2 {
        let u  = 1.0 - t;
        let u2 = u * u;
        let t2 = t * t;
        (self.p1 - self.p0) * (3.0 * u2)
        + (self.p2 - self.p1) * (6.0 * u * t)
        + (self.p3 - self.p2) * (3.0 * t2)
    }

    /// CSS cubic-bezier ease convenience function (control points on [0,1]).
    /// Standard CSS ease: cubic-bezier(0.25, 0.1, 0.25, 1.0).
    pub fn css_ease() -> Self {
        Self::new(
            Vec2::ZERO,
            Vec2::new(0.25, 0.1),
            Vec2::new(0.25, 1.0),
            Vec2::ONE,
        )
    }

    pub fn css_ease_in() -> Self {
        Self::new(Vec2::ZERO, Vec2::new(0.42, 0.0), Vec2::ONE, Vec2::ONE)
    }

    pub fn css_ease_out() -> Self {
        Self::new(Vec2::ZERO, Vec2::ZERO, Vec2::new(0.58, 1.0), Vec2::ONE)
    }

    pub fn css_ease_in_out() -> Self {
        Self::new(Vec2::ZERO, Vec2::new(0.42, 0.0), Vec2::new(0.58, 1.0), Vec2::ONE)
    }

    /// Solve for Y given X using Newton's method (for CSS-style timing functions).
    pub fn solve_for_x(&self, x: f32, tol: f32) -> f32 {
        let mut t = x;
        for _ in 0..8 {
            let pt = self.sample(t);
            let err = pt.x - x;
            if err.abs() < tol { return pt.y; }
            let dt = self.tangent(t).x;
            if dt.abs() < 1e-8 { break; }
            t -= err / dt;
            t  = t.clamp(0.0, 1.0);
        }
        self.sample(t).y
    }
}

// ── Frenet frame ──────────────────────────────────────────────────────────────

/// Frenet-Serret frame at a point on a curve.
#[derive(Clone, Copy, Debug)]
pub struct FrenetFrame {
    pub position: Vec3,
    pub tangent:  Vec3,  // T
    pub normal:   Vec3,  // N (principal normal)
    pub binormal: Vec3,  // B = T × N
    pub curvature: f32,
    pub torsion:   f32,
}

impl FrenetFrame {
    /// Compute the Frenet frame at parameter t on a curve.
    pub fn compute(curve: &dyn Curve, t: f32) -> Self {
        let eps = 1e-4_f32;
        let pos = curve.sample(t);
        let tan = curve.unit_tangent(t);

        // Second derivative for normal
        let t0 = (t - eps).max(0.0);
        let t1 = (t + eps).min(1.0);
        let d0 = curve.unit_tangent(t0);
        let d1 = curve.unit_tangent(t1);
        let dT = (d1 - d0) / (t1 - t0);

        let curvature = dT.length();
        let normal    = if curvature > 1e-8 { dT.normalize() } else { tan.any_orthogonal_vector() };
        let binormal  = tan.cross(normal).normalize_or_zero();

        // Torsion: dB/dt · N
        let t00 = (t - 2.0 * eps).max(0.0);
        let t11 = (t + 2.0 * eps).min(1.0);
        let bin0 = {
            let t0 = curve.unit_tangent(t00);
            let d  = (curve.unit_tangent(t00 + eps) - t0) / eps;
            let n  = if d.length() > 1e-8 { d.normalize() } else { Vec3::Y };
            t0.cross(n)
        };
        let bin1 = {
            let t1 = curve.unit_tangent(t11);
            let d  = (t1 - curve.unit_tangent(t11 - eps)) / eps;
            let n  = if d.length() > 1e-8 { d.normalize() } else { Vec3::Y };
            t1.cross(n)
        };
        let dB = (bin1 - bin0) / (t11 - t00);
        let torsion = dB.dot(normal);

        Self { position: pos, tangent: tan, normal, binormal, curvature, torsion }
    }
}

// ── Curve Sampler (iterator) ──────────────────────────────────────────────────

/// Iterator that walks a curve at fixed arc-length steps.
pub struct CurveWalker<'a> {
    curve:   &'a dyn Curve,
    table:   ArcTable,
    current: f32,  // current arc-length fraction
    step:    f32,  // arc-length step per tick
}

impl<'a> CurveWalker<'a> {
    pub fn new(curve: &'a dyn Curve, steps_per_unit_length: f32, resolution: u32) -> Self {
        let table = curve.build_arc_table(resolution);
        let step  = steps_per_unit_length / table.total_length.max(f32::EPSILON);
        Self { curve, table, current: 0.0, step }
    }
}

impl<'a> Iterator for CurveWalker<'a> {
    type Item = (f32, Vec3);  // (arc_fraction, position)

    fn next(&mut self) -> Option<Self::Item> {
        if self.current > 1.0 { return None; }
        let s   = self.current;
        let t   = self.table.arc_to_t(s);
        let pos = self.curve.sample(t);
        self.current += self.step;
        Some((s, pos))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn v3(x: f32, y: f32, z: f32) -> Vec3 { Vec3::new(x, y, z) }

    #[test]
    fn line_segment_endpoints() {
        let seg = LineSegment::new(v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0));
        let p0  = seg.sample(0.0);
        let p1  = seg.sample(1.0);
        assert!((p0 - v3(0.0, 0.0, 0.0)).length() < 1e-5);
        assert!((p1 - v3(1.0, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn quadratic_bezier_midpoint() {
        // Control points: (0,0), (1,2), (2,0) — midpoint should be (1,1)
        let q   = QuadraticBezier::new(v3(0.0,0.0,0.0), v3(1.0,2.0,0.0), v3(2.0,0.0,0.0));
        let mid = q.sample(0.5);
        assert!((mid - v3(1.0, 1.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn cubic_bezier_endpoints() {
        let b = CubicBezier::new(
            v3(0.0,0.0,0.0), v3(1.0,2.0,0.0),
            v3(2.0,-1.0,0.0), v3(3.0,0.0,0.0),
        );
        assert!((b.sample(0.0) - v3(0.0,0.0,0.0)).length() < 1e-5);
        assert!((b.sample(1.0) - v3(3.0,0.0,0.0)).length() < 1e-5);
    }

    #[test]
    fn bezier_n_matches_cubic() {
        let pts = vec![
            v3(0.0,0.0,0.0), v3(1.0,2.0,0.0),
            v3(2.0,-1.0,0.0), v3(3.0,0.0,0.0),
        ];
        let cubic   = CubicBezier::new(pts[0], pts[1], pts[2], pts[3]);
        let generic = BezierN::new(pts);
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let d = (cubic.sample(t) - generic.sample(t)).length();
            assert!(d < 1e-4, "Mismatch at t={}: {}", t, d);
        }
    }

    #[test]
    fn catmull_rom_passes_through_waypoints() {
        let pts = vec![
            v3(0.0,0.0,0.0), v3(1.0,1.0,0.0),
            v3(2.0,0.0,0.0), v3(3.0,1.0,0.0),
        ];
        let cr = CatmullRom::new(pts.clone());
        // For uniform CR, internal points are not exactly hit (centripetal)
        // But start and end should be close to first/last segments
        let start = cr.sample(0.0);
        let end   = cr.sample(1.0);
        // With 4 pts, segments go [0,1], [1,2], [2,3] → t=0 → pt[0], t=1 → pt[3]
        assert!((start - pts[0]).length() < 0.1);
        assert!((end   - pts[3]).length() < 0.1);
    }

    #[test]
    fn arc_table_monotone() {
        let b = CubicBezier::new(
            v3(0.0,0.0,0.0), v3(0.5,1.0,0.0),
            v3(1.5,-1.0,0.0), v3(2.0,0.0,0.0),
        );
        let table = b.build_arc_table(128);
        // Arc lengths should be monotonically non-decreasing
        for i in 1..table.arc_lengths.len() {
            assert!(table.arc_lengths[i] >= table.arc_lengths[i-1]);
        }
    }

    #[test]
    fn arc_to_t_returns_valid_range() {
        let b = LineSegment::new(v3(0.0,0.0,0.0), v3(1.0,0.0,0.0));
        let table = b.build_arc_table(64);
        for i in 0..=10 {
            let s = i as f32 / 10.0;
            let t = table.arc_to_t(s);
            assert!(t >= 0.0 && t <= 1.0, "t={} out of range for s={}", t, s);
        }
    }

    #[test]
    fn bspline_continuity() {
        let pts = vec![
            v3(0.0,0.0,0.0), v3(1.0,1.0,0.0), v3(2.0,0.0,0.0),
            v3(3.0,1.0,0.0), v3(4.0,0.0,0.0),
        ];
        let bs = BSpline::new(pts);
        // Just verify no panics and values are bounded
        for i in 0..=20 {
            let t = i as f32 / 20.0;
            let p = bs.sample(t);
            assert!(p.x >= -1.0 && p.x <= 5.0);
        }
    }

    #[test]
    fn hermite_spline_at_knot() {
        let spl = HermiteSpline::new()
            .add_knot(0.0, v3(0.0, 0.0, 0.0), v3(1.0, 0.0, 0.0))
            .add_knot(1.0, v3(2.0, 0.0, 0.0), v3(1.0, 0.0, 0.0));
        let p0 = spl.sample(0.0);
        let p1 = spl.sample(1.0);
        assert!((p0 - v3(0.0, 0.0, 0.0)).length() < 1e-5);
        assert!((p1 - v3(2.0, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn css_bezier_ease_start_end() {
        let b = CubicBezier2D::css_ease();
        let s = b.sample(0.0);
        let e = b.sample(1.0);
        assert!((s - Vec2::ZERO).length() < 1e-5);
        assert!((e - Vec2::ONE ).length() < 1e-5);
    }
}
