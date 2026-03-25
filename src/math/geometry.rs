//! Computational geometry: primitives, intersection, distance, convex hull,
//! triangulation, polygon operations, GJK/EPA collision detection.

use std::f64;

// ============================================================
// PRIMITIVE TYPES
// ============================================================

/// 2D point / vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn new(x: f64, y: f64) -> Self { Point2 { x, y } }
    pub fn zero() -> Self { Point2 { x: 0.0, y: 0.0 } }
    pub fn dot(self, other: Self) -> f64 { self.x * other.x + self.y * other.y }
    pub fn cross(self, other: Self) -> f64 { self.x * other.y - self.y * other.x }
    pub fn len_sq(self) -> f64 { self.x * self.x + self.y * self.y }
    pub fn len(self) -> f64 { self.len_sq().sqrt() }
    pub fn normalize(self) -> Self {
        let l = self.len();
        if l < 1e-300 { return Self::zero(); }
        Point2 { x: self.x / l, y: self.y / l }
    }
    pub fn dist(self, other: Self) -> f64 { (self - other).len() }
    pub fn perp(self) -> Self { Point2 { x: -self.y, y: self.x } }
}

impl std::ops::Add for Point2 {
    type Output = Self;
    fn add(self, o: Self) -> Self { Point2 { x: self.x + o.x, y: self.y + o.y } }
}
impl std::ops::Sub for Point2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self { Point2 { x: self.x - o.x, y: self.y - o.y } }
}
impl std::ops::Mul<f64> for Point2 {
    type Output = Self;
    fn mul(self, s: f64) -> Self { Point2 { x: self.x * s, y: self.y * s } }
}
impl std::ops::Neg for Point2 {
    type Output = Self;
    fn neg(self) -> Self { Point2 { x: -self.x, y: -self.y } }
}

/// 3D point / vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self { Point3 { x, y, z } }
    pub fn zero() -> Self { Point3 { x: 0.0, y: 0.0, z: 0.0 } }
    pub fn dot(self, other: Self) -> f64 { self.x * other.x + self.y * other.y + self.z * other.z }
    pub fn cross(self, other: Self) -> Self {
        Point3 {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }
    pub fn len_sq(self) -> f64 { self.x * self.x + self.y * self.y + self.z * self.z }
    pub fn len(self) -> f64 { self.len_sq().sqrt() }
    pub fn normalize(self) -> Self {
        let l = self.len();
        if l < 1e-300 { return Self::zero(); }
        Point3 { x: self.x / l, y: self.y / l, z: self.z / l }
    }
    pub fn dist(self, other: Self) -> f64 { (self - other).len() }
    pub fn lerp(self, other: Self, t: f64) -> Self {
        self + (other - self) * t
    }
    pub fn abs(self) -> Self { Point3 { x: self.x.abs(), y: self.y.abs(), z: self.z.abs() } }
    pub fn component_min(self, other: Self) -> Self {
        Point3 { x: self.x.min(other.x), y: self.y.min(other.y), z: self.z.min(other.z) }
    }
    pub fn component_max(self, other: Self) -> Self {
        Point3 { x: self.x.max(other.x), y: self.y.max(other.y), z: self.z.max(other.z) }
    }
}

impl std::ops::Add for Point3 {
    type Output = Self;
    fn add(self, o: Self) -> Self { Point3 { x: self.x + o.x, y: self.y + o.y, z: self.z + o.z } }
}
impl std::ops::Sub for Point3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self { Point3 { x: self.x - o.x, y: self.y - o.y, z: self.z - o.z } }
}
impl std::ops::Mul<f64> for Point3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self { Point3 { x: self.x * s, y: self.y * s, z: self.z * s } }
}
impl std::ops::Div<f64> for Point3 {
    type Output = Self;
    fn div(self, s: f64) -> Self { Point3 { x: self.x / s, y: self.y / s, z: self.z / s } }
}
impl std::ops::Neg for Point3 {
    type Output = Self;
    fn neg(self) -> Self { Point3 { x: -self.x, y: -self.y, z: -self.z } }
}
impl std::ops::AddAssign for Point3 {
    fn add_assign(&mut self, o: Self) { self.x += o.x; self.y += o.y; self.z += o.z; }
}

/// 2D line segment.
#[derive(Clone, Copy, Debug)]
pub struct Segment2 {
    pub a: Point2,
    pub b: Point2,
}

/// 2D triangle.
#[derive(Clone, Copy, Debug)]
pub struct Triangle2 {
    pub a: Point2,
    pub b: Point2,
    pub c: Point2,
}

/// 3D triangle.
#[derive(Clone, Copy, Debug)]
pub struct Triangle3 {
    pub a: Point3,
    pub b: Point3,
    pub c: Point3,
}

/// 2D axis-aligned bounding box.
#[derive(Clone, Copy, Debug)]
pub struct Aabb2 {
    pub min: Point2,
    pub max: Point2,
}

impl Aabb2 {
    pub fn new(min: Point2, max: Point2) -> Self { Aabb2 { min, max } }
    pub fn contains(&self, p: Point2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
    pub fn center(&self) -> Point2 {
        Point2 { x: (self.min.x + self.max.x) * 0.5, y: (self.min.y + self.max.y) * 0.5 }
    }
}

/// 3D axis-aligned bounding box.
#[derive(Clone, Copy, Debug)]
pub struct Aabb3 {
    pub min: Point3,
    pub max: Point3,
}

impl Aabb3 {
    pub fn new(min: Point3, max: Point3) -> Self { Aabb3 { min, max } }
    pub fn center(&self) -> Point3 { (self.min + self.max) * 0.5 }
    pub fn half_extents(&self) -> Point3 { (self.max - self.min) * 0.5 }
    pub fn expand(&self, amount: f64) -> Self {
        let a = Point3::new(amount, amount, amount);
        Aabb3 { min: self.min - a, max: self.max + a }
    }
}

/// Circle in 2D.
#[derive(Clone, Copy, Debug)]
pub struct Circle {
    pub center: Point2,
    pub radius: f64,
}

/// Sphere in 3D.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub center: Point3,
    pub radius: f64,
}

/// Plane: normal·x + d = 0. Normal should be unit length.
#[derive(Clone, Copy, Debug)]
pub struct Plane {
    pub normal: Point3,
    pub d: f64,
}

impl Plane {
    pub fn from_point_normal(p: Point3, n: Point3) -> Self {
        let n = n.normalize();
        Plane { normal: n, d: -n.dot(p) }
    }
    pub fn from_triangle(a: Point3, b: Point3, c: Point3) -> Self {
        let n = (b - a).cross(c - a).normalize();
        Self::from_point_normal(a, n)
    }
    pub fn signed_dist(&self, p: Point3) -> f64 {
        self.normal.dot(p) + self.d
    }
}

/// 2D ray.
#[derive(Clone, Copy, Debug)]
pub struct Ray2 {
    pub origin: Point2,
    pub dir: Point2,
}

/// 3D ray.
#[derive(Clone, Copy, Debug)]
pub struct Ray3 {
    pub origin: Point3,
    pub dir: Point3,
}

impl Ray3 {
    pub fn at(&self, t: f64) -> Point3 { self.origin + self.dir * t }
}

/// Capsule: cylinder with hemispherical caps.
#[derive(Clone, Copy, Debug)]
pub struct Capsule {
    pub a: Point3,
    pub b: Point3,
    pub radius: f64,
}

/// Oriented bounding box.
#[derive(Clone, Copy, Debug)]
pub struct Obb3 {
    pub center: Point3,
    pub axes: [Point3; 3],       // orthonormal local axes
    pub half_extents: Point3,    // half-widths along each axis
}

/// Result of a halfspace test for AABB vs plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Halfspace {
    Front,
    Back,
    Intersect,
}

// ============================================================
// INTERSECTION TESTS
// ============================================================

/// Ray vs AABB — returns t of first positive intersection or None.
pub fn ray_aabb(ray: &Ray3, aabb: &Aabb3) -> Option<f64> {
    let inv_dir = Point3::new(1.0 / ray.dir.x, 1.0 / ray.dir.y, 1.0 / ray.dir.z);
    let t1 = (aabb.min.x - ray.origin.x) * inv_dir.x;
    let t2 = (aabb.max.x - ray.origin.x) * inv_dir.x;
    let t3 = (aabb.min.y - ray.origin.y) * inv_dir.y;
    let t4 = (aabb.max.y - ray.origin.y) * inv_dir.y;
    let t5 = (aabb.min.z - ray.origin.z) * inv_dir.z;
    let t6 = (aabb.max.z - ray.origin.z) * inv_dir.z;
    let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
    let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));
    if tmax < 0.0 || tmin > tmax { None } else { Some(tmin.max(0.0)) }
}

/// Ray vs sphere.
pub fn ray_sphere(ray: &Ray3, sphere: &Sphere) -> Option<f64> {
    let oc = ray.origin - sphere.center;
    let a = ray.dir.dot(ray.dir);
    let b = 2.0 * oc.dot(ray.dir);
    let c = oc.dot(oc) - sphere.radius * sphere.radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 { return None; }
    let sqrt_d = disc.sqrt();
    let t1 = (-b - sqrt_d) / (2.0 * a);
    let t2 = (-b + sqrt_d) / (2.0 * a);
    if t1 >= 0.0 { Some(t1) } else if t2 >= 0.0 { Some(t2) } else { None }
}

/// Ray vs triangle using Möller–Trumbore algorithm.
/// Returns (t, u, v) — barycentric coords (w = 1 - u - v).
pub fn ray_triangle(ray: &Ray3, tri: &Triangle3) -> Option<(f64, f64, f64)> {
    const EPS: f64 = 1e-10;
    let edge1 = tri.b - tri.a;
    let edge2 = tri.c - tri.a;
    let h = ray.dir.cross(edge2);
    let det = edge1.dot(h);
    if det.abs() < EPS { return None; }
    let inv_det = 1.0 / det;
    let s = ray.origin - tri.a;
    let u = inv_det * s.dot(h);
    if !(0.0..=1.0).contains(&u) { return None; }
    let q = s.cross(edge1);
    let v = inv_det * ray.dir.dot(q);
    if v < 0.0 || u + v > 1.0 { return None; }
    let t = inv_det * edge2.dot(q);
    if t < EPS { return None; }
    Some((t, u, v))
}

/// Ray vs plane.
pub fn ray_plane(ray: &Ray3, plane: &Plane) -> Option<f64> {
    let denom = plane.normal.dot(ray.dir);
    if denom.abs() < 1e-12 { return None; }
    let t = -(plane.normal.dot(ray.origin) + plane.d) / denom;
    if t < 0.0 { None } else { Some(t) }
}

/// Ray vs capsule.
pub fn ray_capsule(ray: &Ray3, cap: &Capsule) -> Option<f64> {
    let ab = cap.b - cap.a;
    let ao = ray.origin - cap.a;
    let ab_len_sq = ab.dot(ab);
    let ab_dot_d = ab.dot(ray.dir);
    let ab_dot_ao = ab.dot(ao);
    let a = ab_len_sq * ray.dir.dot(ray.dir) - ab_dot_d * ab_dot_d;
    let bv = ab_len_sq * ray.dir.dot(ao) - ab_dot_d * ab_dot_ao;
    let c = ab_len_sq * (ao.dot(ao) - cap.radius * cap.radius) - ab_dot_ao * ab_dot_ao;
    let mut t_hit = f64::MAX;
    if a.abs() > 1e-10 {
        let disc = bv * bv - a * c;
        if disc >= 0.0 {
            let t = (-bv - disc.sqrt()) / a;
            let m = ab_dot_d * t + ab_dot_ao;
            if m >= 0.0 && m <= ab_len_sq {
                t_hit = t_hit.min(t);
            }
        }
    }
    // Check sphere caps
    for cap_center in [cap.a, cap.b] {
        let sphere = Sphere { center: cap_center, radius: cap.radius };
        if let Some(t) = ray_sphere(ray, &sphere) {
            t_hit = t_hit.min(t);
        }
    }
    if t_hit < f64::MAX && t_hit >= 0.0 { Some(t_hit) } else { None }
}

/// Ray vs oriented bounding box.
pub fn ray_obb(ray: &Ray3, obb: &Obb3) -> Option<f64> {
    // Transform ray into OBB local space
    let d = ray.origin - obb.center;
    let mut t_min = f64::NEG_INFINITY;
    let mut t_max = f64::INFINITY;
    let half = [obb.half_extents.x, obb.half_extents.y, obb.half_extents.z];
    for (i, axis) in obb.axes.iter().enumerate() {
        let e = axis.dot(d);
        let f = axis.dot(ray.dir);
        if f.abs() > 1e-12 {
            let t1 = (-e - half[i]) / f;
            let t2 = (-e + half[i]) / f;
            let (t1, t2) = if t1 > t2 { (t2, t1) } else { (t1, t2) };
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max { return None; }
        } else if e.abs() > half[i] {
            return None;
        }
    }
    if t_max < 0.0 { return None; }
    let t = if t_min >= 0.0 { t_min } else { t_max };
    Some(t)
}

/// AABB vs AABB overlap test.
pub fn aabb_aabb(a: &Aabb3, b: &Aabb3) -> bool {
    a.min.x <= b.max.x && a.max.x >= b.min.x &&
    a.min.y <= b.max.y && a.max.y >= b.min.y &&
    a.min.z <= b.max.z && a.max.z >= b.min.z
}

/// Sphere vs sphere overlap.
pub fn sphere_sphere(a: &Sphere, b: &Sphere) -> bool {
    let d = a.center.dist(b.center);
    d <= a.radius + b.radius
}

/// Sphere vs AABB overlap.
pub fn sphere_aabb(s: &Sphere, b: &Aabb3) -> bool {
    let closest = Point3::new(
        s.center.x.clamp(b.min.x, b.max.x),
        s.center.y.clamp(b.min.y, b.max.y),
        s.center.z.clamp(b.min.z, b.max.z),
    );
    s.center.dist(closest) <= s.radius
}

/// AABB vs plane halfspace classification.
pub fn aabb_plane(b: &Aabb3, p: &Plane) -> Halfspace {
    let center = b.center();
    let half = b.half_extents();
    let r = half.x * p.normal.x.abs()
           + half.y * p.normal.y.abs()
           + half.z * p.normal.z.abs();
    let dist = p.normal.dot(center) + p.d;
    if dist > r { Halfspace::Front }
    else if dist < -r { Halfspace::Back }
    else { Halfspace::Intersect }
}

/// OBB vs OBB — Separating Axis Theorem.
pub fn obb_obb(a: &Obb3, b: &Obb3) -> bool {
    let t = b.center - a.center;
    let half_a = [a.half_extents.x, a.half_extents.y, a.half_extents.z];
    let half_b = [b.half_extents.x, b.half_extents.y, b.half_extents.z];

    // Test 15 axes: 3 from A, 3 from B, 9 cross products
    let axes_a = a.axes;
    let axes_b = b.axes;

    // Helper: project OBB half-extent onto axis
    let project_half = |axes: &[Point3; 3], halves: &[f64; 3], axis: Point3| -> f64 {
        halves[0] * axes[0].dot(axis).abs()
        + halves[1] * axes[1].dot(axis).abs()
        + halves[2] * axes[2].dot(axis).abs()
    };

    let test_axis = |axis: Point3| -> bool {
        let len = axis.len_sq();
        if len < 1e-10 { return true; } // degenerate
        let axis = axis * (1.0 / len.sqrt());
        let dist = t.dot(axis).abs();
        let ra = project_half(&axes_a, &half_a, axis);
        let rb = project_half(&axes_b, &half_b, axis);
        dist <= ra + rb
    };

    // A's axes
    for i in 0..3 {
        if !test_axis(axes_a[i]) { return false; }
    }
    // B's axes
    for i in 0..3 {
        if !test_axis(axes_b[i]) { return false; }
    }
    // Cross products
    for i in 0..3 {
        for j in 0..3 {
            let axis = axes_a[i].cross(axes_b[j]);
            if !test_axis(axis) { return false; }
        }
    }
    true
}

/// Capsule vs capsule overlap.
pub fn capsule_capsule(a: &Capsule, b: &Capsule) -> bool {
    let dist = segment_segment_dist_3d(a.a, a.b, b.a, b.b);
    dist <= a.radius + b.radius
}

/// Point in 2D triangle test.
pub fn point_in_triangle(p: Point2, tri: &Triangle2) -> bool {
    let d1 = (p - tri.a).cross(tri.b - tri.a);
    let d2 = (p - tri.b).cross(tri.c - tri.b);
    let d3 = (p - tri.c).cross(tri.a - tri.c);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

/// Point inside AABB.
pub fn point_in_aabb(p: Point3, b: &Aabb3) -> bool {
    p.x >= b.min.x && p.x <= b.max.x &&
    p.y >= b.min.y && p.y <= b.max.y &&
    p.z >= b.min.z && p.z <= b.max.z
}

/// Point inside OBB.
pub fn point_in_obb(p: Point3, obb: &Obb3) -> bool {
    let d = p - obb.center;
    let hx = d.dot(obb.axes[0]).abs();
    let hy = d.dot(obb.axes[1]).abs();
    let hz = d.dot(obb.axes[2]).abs();
    hx <= obb.half_extents.x && hy <= obb.half_extents.y && hz <= obb.half_extents.z
}

/// 2D segment-segment intersection. Returns intersection point or None.
pub fn segment_segment_2d(a: &Segment2, b: &Segment2) -> Option<Point2> {
    let r = a.b - a.a;
    let s = b.b - b.a;
    let denom = r.cross(s);
    if denom.abs() < 1e-12 { return None; }
    let diff = b.a - a.a;
    let t = diff.cross(s) / denom;
    let u = diff.cross(r) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(a.a + r * t)
    } else {
        None
    }
}

// ============================================================
// DISTANCE QUERIES
// ============================================================

fn clamp01(x: f64) -> f64 { x.clamp(0.0, 1.0) }

/// Closest point on a 3D segment to point p (treating segment as 3D using z=0 for Point2).
pub fn point_to_segment(p: Point3, seg_a: Point3, seg_b: Point3) -> f64 {
    let ab = seg_b - seg_a;
    let t = clamp01((p - seg_a).dot(ab) / ab.dot(ab).max(1e-300));
    (seg_a + ab * t).dist(p)
}

/// 3D segment-to-segment minimum distance.
fn segment_segment_dist_3d(a0: Point3, a1: Point3, b0: Point3, b1: Point3) -> f64 {
    let d1 = a1 - a0;
    let d2 = b1 - b0;
    let r = a0 - b0;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let (s, t) = if a < 1e-10 {
        (0.0, clamp01(f / e.max(1e-10)))
    } else {
        let c = d1.dot(r);
        if e < 1e-10 {
            (clamp01(-c / a), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let s = if denom.abs() > 1e-10 { clamp01((b * f - c * e) / denom) } else { 0.0 };
            let t = (b * s + f) / e;
            if t < 0.0 {
                (clamp01(-c / a), 0.0)
            } else if t > 1.0 {
                (clamp01((b - c) / a), 1.0)
            } else {
                (s, t)
            }
        }
    };
    let p1 = a0 + d1 * s;
    let p2 = b0 + d2 * t;
    p1.dist(p2)
}

/// 2D segment-to-segment distance (embedding in 3D).
pub fn segment_to_segment(a: &Segment2, b: &Segment2) -> f64 {
    let a0 = Point3::new(a.a.x, a.a.y, 0.0);
    let a1 = Point3::new(a.b.x, a.b.y, 0.0);
    let b0 = Point3::new(b.a.x, b.a.y, 0.0);
    let b1 = Point3::new(b.b.x, b.b.y, 0.0);
    segment_segment_dist_3d(a0, a1, b0, b1)
}

/// Signed distance from point to plane.
pub fn point_to_plane(p: Point3, plane: &Plane) -> f64 {
    plane.normal.dot(p) + plane.d
}

/// Distance from point to AABB (0 if inside).
pub fn point_to_aabb(p: Point3, b: &Aabb3) -> f64 {
    let dx = (b.min.x - p.x).max(0.0).max(p.x - b.max.x);
    let dy = (b.min.y - p.y).max(0.0).max(p.y - b.max.y);
    let dz = (b.min.z - p.z).max(0.0).max(p.z - b.max.z);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Distance from point to triangle in 3D.
pub fn point_to_triangle(p: Point3, tri: &Triangle3) -> f64 {
    let ab = tri.b - tri.a;
    let ac = tri.c - tri.a;
    let ap = p - tri.a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 { return p.dist(tri.a); }
    let bp = p - tri.b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 { return p.dist(tri.b); }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return p.dist(tri.a + ab * v);
    }
    let cp = p - tri.c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 { return p.dist(tri.c); }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return p.dist(tri.a + ac * w);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return p.dist(tri.b + (tri.c - tri.b) * w);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let closest = tri.a + ab * v + ac * w;
    p.dist(closest)
}

/// Distance from sphere to AABB surface. Negative means overlap.
pub fn sphere_to_aabb(s: &Sphere, b: &Aabb3) -> f64 {
    point_to_aabb(s.center, b) - s.radius
}

// ============================================================
// CONVEX HULL
// ============================================================

/// 2D convex hull via Jarvis march (gift wrapping).
pub fn convex_hull_2d(points: &[Point2]) -> Vec<Point2> {
    let n = points.len();
    if n < 3 { return points.to_vec(); }
    // Find leftmost point
    let start = points
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.x.partial_cmp(&b.x).unwrap().then(a.y.partial_cmp(&b.y).unwrap()))
        .unwrap()
        .0;
    let mut hull = Vec::new();
    let mut current = start;
    loop {
        hull.push(points[current]);
        let mut next = (current + 1) % n;
        for i in 0..n {
            let cross = (points[next] - points[current]).cross(points[i] - points[current]);
            if cross < 0.0 { next = i; }
        }
        current = next;
        if current == start { break; }
        if hull.len() > n { break; } // safety
    }
    hull
}

/// 3D convex hull via incremental Quickhull. Returns triangle face indices.
pub fn convex_hull_3d(points: &[Point3]) -> Vec<[usize; 3]> {
    let n = points.len();
    if n < 4 { return vec![]; }

    // Find initial tetrahedron
    let mut extreme = [0usize; 6];
    for (i, p) in points.iter().enumerate() {
        if p.x < points[extreme[0]].x { extreme[0] = i; }
        if p.x > points[extreme[1]].x { extreme[1] = i; }
        if p.y < points[extreme[2]].y { extreme[2] = i; }
        if p.y > points[extreme[3]].y { extreme[3] = i; }
        if p.z < points[extreme[4]].z { extreme[4] = i; }
        if p.z > points[extreme[5]].z { extreme[5] = i; }
    }

    // Pick 4 non-coplanar points
    let mut simplex = vec![extreme[0], extreme[1]];
    for &e in &extreme[2..] {
        if !simplex.contains(&e) { simplex.push(e); }
        if simplex.len() == 4 { break; }
    }
    // Fill with any remaining if needed
    for i in 0..n {
        if simplex.len() >= 4 { break; }
        if !simplex.contains(&i) { simplex.push(i); }
    }
    if simplex.len() < 4 { return vec![]; }

    // Initial faces of tetrahedron
    let (i0, i1, i2, i3) = (simplex[0], simplex[1], simplex[2], simplex[3]);
    let mut faces: Vec<[usize; 3]> = vec![
        [i0, i1, i2], [i0, i2, i3], [i0, i3, i1], [i1, i3, i2],
    ];

    // Make all faces outward-facing
    let centroid = points.iter().fold(Point3::zero(), |acc, p| acc + *p) * (1.0 / n as f64);
    for face in &mut faces {
        let n = (points[face[1]] - points[face[0]]).cross(points[face[2]] - points[face[0]]);
        if n.dot(centroid - points[face[0]]) > 0.0 {
            face.swap(0, 1);
        }
    }

    // Iteratively add remaining points
    for p_idx in 0..n {
        if simplex.contains(&p_idx) { continue; }
        let p = points[p_idx];

        // Find visible faces
        let visible: Vec<bool> = faces.iter().map(|face| {
            let normal = (points[face[1]] - points[face[0]]).cross(points[face[2]] - points[face[0]]);
            normal.dot(p - points[face[0]]) > 1e-10
        }).collect();

        if !visible.iter().any(|&v| v) { continue; }

        // Find horizon edges
        let mut horizon: Vec<(usize, usize)> = Vec::new();
        for (fi, face) in faces.iter().enumerate() {
            if !visible[fi] { continue; }
            let edges = [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])];
            for &(ea, eb) in &edges {
                // Check if this edge is shared with an invisible face
                let shared = faces.iter().enumerate().any(|(fj, f2)| {
                    !visible[fj] && (
                        (f2[0] == eb && f2[1] == ea) ||
                        (f2[1] == eb && f2[2] == ea) ||
                        (f2[2] == eb && f2[0] == ea)
                    )
                });
                if shared { horizon.push((ea, eb)); }
            }
        }

        // Remove visible faces and add new faces from horizon
        let kept: Vec<[usize; 3]> = faces.iter().enumerate()
            .filter(|(i, _)| !visible[*i])
            .map(|(_, f)| *f)
            .collect();
        faces = kept;
        for (ea, eb) in horizon {
            let normal = (points[eb] - points[ea]).cross(points[p_idx] - points[ea]);
            let centroid_side = normal.dot(centroid - points[ea]);
            if centroid_side > 0.0 {
                faces.push([ea, p_idx, eb]);
            } else {
                faces.push([ea, eb, p_idx]);
            }
        }
    }
    faces
}

/// Point in convex polygon (hull assumed CCW).
pub fn point_in_convex_polygon(p: Point2, hull: &[Point2]) -> bool {
    let n = hull.len();
    if n < 3 { return false; }
    for i in 0..n {
        let a = hull[i];
        let b = hull[(i + 1) % n];
        if (b - a).cross(p - a) < 0.0 { return false; }
    }
    true
}

/// Area of a convex polygon.
pub fn convex_polygon_area(hull: &[Point2]) -> f64 {
    polygon_area(hull).abs()
}

// ============================================================
// TRIANGULATION
// ============================================================

/// Ear clipping triangulation for simple (non-self-intersecting) polygons.
/// Returns triangle indices.
pub fn ear_clipping(polygon: &[Point2]) -> Vec<[usize; 3]> {
    let n = polygon.len();
    if n < 3 { return vec![]; }
    let mut result = Vec::new();
    let mut indices: Vec<usize> = (0..n).collect();

    while indices.len() > 3 {
        let len = indices.len();
        let mut found_ear = false;
        for i in 0..len {
            let prev = (i + len - 1) % len;
            let next = (i + 1) % len;
            let a = polygon[indices[prev]];
            let b = polygon[indices[i]];
            let c = polygon[indices[next]];
            // Check if angle at b is convex (CCW polygon)
            let cross = (b - a).cross(c - b);
            if cross <= 0.0 { continue; }
            // Check no other vertex is inside triangle ABC
            let tri = Triangle2 { a, b, c };
            let ear_valid = !indices.iter().enumerate().any(|(j, &vi)| {
                j != prev && j != i && j != next
                    && point_in_triangle(polygon[vi], &tri)
            });
            if ear_valid {
                result.push([indices[prev], indices[i], indices[next]]);
                indices.remove(i);
                found_ear = true;
                break;
            }
        }
        if !found_ear { break; }
    }
    if indices.len() == 3 {
        result.push([indices[0], indices[1], indices[2]]);
    }
    result
}

/// Bowyer-Watson Delaunay triangulation in 2D.
/// Returns triangle index triples.
pub fn delaunay_2d(points: &[Point2]) -> Vec<[usize; 3]> {
    let n = points.len();
    if n < 3 { return vec![]; }

    // Add super-triangle containing all points
    let mut pts = points.to_vec();
    let min_x = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_x = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = pts.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = pts.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    let delta_max = dx.max(dy);
    let mid_x = (min_x + max_x) * 0.5;
    let mid_y = (min_y + max_y) * 0.5;

    let st0 = Point2::new(mid_x - 20.0 * delta_max, mid_y - delta_max);
    let st1 = Point2::new(mid_x, mid_y + 20.0 * delta_max);
    let st2 = Point2::new(mid_x + 20.0 * delta_max, mid_y - delta_max);
    let si0 = n;
    let si1 = n + 1;
    let si2 = n + 2;
    pts.push(st0); pts.push(st1); pts.push(st2);

    let mut triangles: Vec<[usize; 3]> = vec![[si0, si1, si2]];

    for i in 0..n {
        let p = pts[i];
        let mut bad_triangles: Vec<[usize; 3]> = Vec::new();
        let mut good_triangles: Vec<[usize; 3]> = Vec::new();

        for &tri in &triangles {
            if circumcircle_contains(&pts[tri[0]], &pts[tri[1]], &pts[tri[2]], &p) {
                bad_triangles.push(tri);
            } else {
                good_triangles.push(tri);
            }
        }

        // Find the boundary polygon of the bad triangle cavity
        let mut boundary: Vec<(usize, usize)> = Vec::new();
        for &tri in &bad_triangles {
            let edges = [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])];
            for &(ea, eb) in &edges {
                let shared = bad_triangles.iter().any(|&t2| {
                    t2 != tri && (
                        (t2[0] == eb && t2[1] == ea)
                        || (t2[1] == eb && t2[2] == ea)
                        || (t2[2] == eb && t2[0] == ea)
                        || (t2[0] == ea && t2[1] == eb)
                        || (t2[1] == ea && t2[2] == eb)
                        || (t2[2] == ea && t2[0] == eb)
                    )
                });
                if !shared { boundary.push((ea, eb)); }
            }
        }

        triangles = good_triangles;
        for (ea, eb) in boundary {
            triangles.push([ea, eb, i]);
        }
    }

    // Remove triangles sharing super-triangle vertices
    triangles.retain(|t| t[0] < n && t[1] < n && t[2] < n);
    triangles
}

fn circumcircle_contains(a: &Point2, b: &Point2, c: &Point2, p: &Point2) -> bool {
    let ax = a.x - p.x; let ay = a.y - p.y;
    let bx = b.x - p.x; let by = b.y - p.y;
    let cx = c.x - p.x; let cy = c.y - p.y;
    let det = ax * (by * (cx * cx + cy * cy) - cy * (bx * bx + by * by))
            - ay * (bx * (cx * cx + cy * cy) - cx * (bx * bx + by * by))
            + (ax * ax + ay * ay) * (bx * cy - by * cx);
    // The sign of det depends on the orientation of the triangle (CCW vs CW).
    // Check orientation and flip sign if clockwise.
    let orient = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if orient > 0.0 { det > 0.0 } else { det < 0.0 }
}

/// Compute approximate Voronoi diagram from Delaunay triangulation dual.
/// Returns one polygon (as point list) per input point.
pub fn voronoi_2d(points: &[Point2], bounds: Aabb2) -> Vec<Vec<Point2>> {
    let tris = delaunay_2d(points);
    let n = points.len();
    let mut cells: Vec<Vec<Point2>> = vec![Vec::new(); n];

    // For each Delaunay triangle, compute circumcenter
    let circumcenter = |a: &Point2, b: &Point2, c: &Point2| -> Point2 {
        let ax = a.x; let ay = a.y;
        let bx = b.x; let by = b.y;
        let cx = c.x; let cy = c.y;
        let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
        if d.abs() < 1e-12 { return Point2::new((ax + bx + cx) / 3.0, (ay + by + cy) / 3.0); }
        let ux = ((ax * ax + ay * ay) * (by - cy)
                + (bx * bx + by * by) * (cy - ay)
                + (cx * cx + cy * cy) * (ay - by)) / d;
        let uy = ((ax * ax + ay * ay) * (cx - bx)
                + (bx * bx + by * by) * (ax - cx)
                + (cx * cx + cy * cy) * (bx - ax)) / d;
        Point2::new(ux, uy)
    };

    // For each point, collect circumcenters of adjacent triangles
    for tri in &tris {
        let cc = circumcenter(&points[tri[0]], &points[tri[1]], &points[tri[2]]);
        // Clamp to bounds
        let cc = Point2::new(
            cc.x.clamp(bounds.min.x, bounds.max.x),
            cc.y.clamp(bounds.min.y, bounds.max.y),
        );
        for &vi in tri {
            cells[vi].push(cc);
        }
    }

    // Sort each cell's points angularly around the site
    for (i, cell) in cells.iter_mut().enumerate() {
        let center = points[i];
        cell.sort_by(|a, b| {
            let angle_a = (a.y - center.y).atan2(a.x - center.x);
            let angle_b = (b.y - center.y).atan2(b.x - center.x);
            angle_a.partial_cmp(&angle_b).unwrap()
        });
        cell.dedup_by(|a, b| (a.x - b.x).abs() < 1e-10 && (a.y - b.y).abs() < 1e-10);
    }
    cells
}

// ============================================================
// POLYGON OPERATIONS
// ============================================================

/// Signed polygon area via Shoelace formula.
pub fn polygon_area(pts: &[Point2]) -> f64 {
    let n = pts.len();
    if n < 3 { return 0.0; }
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i].x * pts[j].y;
        area -= pts[j].x * pts[i].y;
    }
    area * 0.5
}

/// Polygon centroid.
pub fn polygon_centroid(pts: &[Point2]) -> Point2 {
    let n = pts.len();
    if n == 0 { return Point2::zero(); }
    let area = polygon_area(pts);
    if area.abs() < 1e-12 {
        let x = pts.iter().map(|p| p.x).sum::<f64>() / n as f64;
        let y = pts.iter().map(|p| p.y).sum::<f64>() / n as f64;
        return Point2::new(x, y);
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        let cross = pts[i].x * pts[j].y - pts[j].x * pts[i].y;
        cx += (pts[i].x + pts[j].x) * cross;
        cy += (pts[i].y + pts[j].y) * cross;
    }
    let factor = 1.0 / (6.0 * area);
    Point2::new(cx * factor, cy * factor)
}

/// Test if polygon is convex.
pub fn polygon_is_convex(pts: &[Point2]) -> bool {
    let n = pts.len();
    if n < 3 { return true; }
    let mut sign = 0.0f64;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let c = pts[(i + 2) % n];
        let cross = (b - a).cross(c - b);
        if cross != 0.0 {
            if sign == 0.0 { sign = cross.signum(); }
            else if cross.signum() != sign { return false; }
        }
    }
    true
}

/// Winding number of polygon. Positive = CCW.
pub fn polygon_winding(pts: &[Point2]) -> f64 {
    polygon_area(pts).signum()
}

/// Sutherland-Hodgman polygon clipping.
pub fn sutherland_hodgman(subject: &[Point2], clip: &[Point2]) -> Vec<Point2> {
    if subject.is_empty() || clip.is_empty() { return vec![]; }
    let mut output = subject.to_vec();
    let m = clip.len();
    for i in 0..m {
        if output.is_empty() { return vec![]; }
        let input = output.clone();
        output.clear();
        let edge_start = clip[i];
        let edge_end = clip[(i + 1) % m];
        let inside = |p: Point2| -> bool {
            (edge_end - edge_start).cross(p - edge_start) >= 0.0
        };
        let intersect = |a: Point2, b: Point2| -> Point2 {
            let ab = b - a;
            let es = edge_end - edge_start;
            let num = (edge_start - a).cross(es);
            let den = ab.cross(es);
            if den.abs() < 1e-12 { return a; }
            a + ab * (num / den)
        };
        for j in 0..input.len() {
            let current = input[j];
            let previous = input[(j + input.len() - 1) % input.len()];
            if inside(current) {
                if !inside(previous) { output.push(intersect(previous, current)); }
                output.push(current);
            } else if inside(previous) {
                output.push(intersect(previous, current));
            }
        }
    }
    output
}

/// Minkowski sum of two convex polygons (CCW winding assumed).
pub fn minkowski_sum(a: &[Point2], b: &[Point2]) -> Vec<Point2> {
    if a.is_empty() || b.is_empty() { return vec![]; }
    // Find bottom-most (then left-most) point for each
    let start_of = |pts: &[Point2]| -> usize {
        pts.iter().enumerate()
           .min_by(|(_, p), (_, q)| {
               p.y.partial_cmp(&q.y).unwrap().then(p.x.partial_cmp(&q.x).unwrap())
           })
           .unwrap()
           .0
    };
    let ia = start_of(a);
    let ib = start_of(b);
    let na = a.len();
    let nb = b.len();
    let mut result = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < na || j < nb {
        result.push(a[(ia + i) % na] + b[(ib + j) % nb]);
        let ea = a[(ia + i + 1) % na] - a[(ia + i) % na];
        let eb = b[(ib + j + 1) % nb] - b[(ib + j) % nb];
        let cross = ea.cross(eb);
        if i >= na { j += 1; }
        else if j >= nb { i += 1; }
        else if cross > 0.0 { i += 1; }
        else if cross < 0.0 { j += 1; }
        else { i += 1; j += 1; }
    }
    result
}

// ============================================================
// GJK / EPA COLLISION DETECTION
// ============================================================

fn support_3d(shape: &[Point3], dir: Point3) -> Point3 {
    shape.iter().copied()
        .max_by(|a, b| a.dot(dir).partial_cmp(&b.dot(dir)).unwrap())
        .unwrap_or(Point3::zero())
}

fn support_minkowski_diff(a: &[Point3], b: &[Point3], dir: Point3) -> Point3 {
    support_3d(a, dir) - support_3d(b, -dir)
}

fn triple_product(a: Point3, b: Point3, c: Point3) -> Point3 {
    b * a.dot(c) - c * a.dot(b)
}

/// GJK distance test — returns true if shapes overlap.
pub fn gjk(shape_a: &[Point3], shape_b: &[Point3]) -> bool {
    let mut dir = support_minkowski_diff(shape_a, shape_b, Point3::new(1.0, 0.0, 0.0));
    if dir.len_sq() < 1e-20 {
        dir = Point3::new(0.0, 1.0, 0.0);
    }
    let mut simplex: Vec<Point3> = Vec::new();
    simplex.push(dir);
    let mut search_dir = -dir;
    for _ in 0..64 {
        let a = support_minkowski_diff(shape_a, shape_b, search_dir);
        if a.dot(search_dir) < 0.0 { return false; }
        simplex.push(a);
        if do_simplex(&mut simplex, &mut search_dir) { return true; }
    }
    false
}

fn do_simplex(simplex: &mut Vec<Point3>, dir: &mut Point3) -> bool {
    match simplex.len() {
        2 => {
            let b = simplex[0];
            let a = simplex[1];
            let ab = b - a;
            let ao = -a;
            if ab.dot(ao) > 0.0 {
                *dir = triple_product(ab, ao, ab);
            } else {
                *simplex = vec![a];
                *dir = ao;
            }
            false
        }
        3 => {
            let c = simplex[0]; let b = simplex[1]; let a = simplex[2];
            let ab = b - a; let ac = c - a; let ao = -a;
            let abc = ab.cross(ac);
            if abc.cross(ac).dot(ao) > 0.0 {
                if ac.dot(ao) > 0.0 {
                    *simplex = vec![c, a]; *dir = triple_product(ac, ao, ac);
                } else {
                    *simplex = vec![b, a]; return do_simplex(simplex, dir);
                }
            } else if ab.cross(abc).dot(ao) > 0.0 {
                *simplex = vec![b, a]; return do_simplex(simplex, dir);
            } else if abc.dot(ao) > 0.0 {
                *dir = abc;
            } else {
                simplex.swap(0, 1); *dir = -abc;
            }
            false
        }
        4 => {
            let d = simplex[0]; let c = simplex[1];
            let b = simplex[2]; let a = simplex[3];
            let ab = b - a; let ac = c - a;
            let ad = d - a; let ao = -a;
            let abc = ab.cross(ac);
            let acd = ac.cross(ad);
            let adb = ad.cross(ab);
            if abc.dot(ao) > 0.0 {
                *simplex = vec![c, b, a];
                return do_simplex(simplex, dir);
            }
            if acd.dot(ao) > 0.0 {
                *simplex = vec![d, c, a];
                return do_simplex(simplex, dir);
            }
            if adb.dot(ao) > 0.0 {
                *simplex = vec![b, d, a];
                return do_simplex(simplex, dir);
            }
            true // origin inside tetrahedron
        }
        _ => { *dir = Point3::new(1.0, 0.0, 0.0); false }
    }
}

/// GJK closest points between two shapes. Returns (point_on_a, point_on_b, distance) or None if overlapping.
pub fn gjk_closest(shape_a: &[Point3], shape_b: &[Point3]) -> Option<(Point3, Point3, f64)> {
    if gjk(shape_a, shape_b) { return None; }
    // Simple iterative closest point using support functions
    let mut dir = shape_a[0] - shape_b[0];
    if dir.len_sq() < 1e-20 { return None; }
    let mut best_dist = f64::INFINITY;
    let mut best_pa = Point3::zero();
    let mut best_pb = Point3::zero();
    for _ in 0..64 {
        dir = dir.normalize();
        let pa = support_3d(shape_a, dir);
        let pb = support_3d(shape_b, -dir);
        let d = (pa - pb).len();
        if d < best_dist {
            best_dist = d;
            best_pa = pa;
            best_pb = pb;
        }
        dir = pb - pa;
        if dir.len_sq() < 1e-20 { break; }
    }
    Some((best_pa, best_pb, best_dist))
}

/// EPA (Expanding Polytope Algorithm) — finds penetration depth and normal.
pub fn epa(shape_a: &[Point3], shape_b: &[Point3]) -> Option<(Point3, f64)> {
    if !gjk(shape_a, shape_b) { return None; }
    // Start with a simplex from GJK — build initial tetrahedron
    let dirs = [
        Point3::new(1.0, 0.0, 0.0), Point3::new(-1.0, 0.0, 0.0),
        Point3::new(0.0, 1.0, 0.0), Point3::new(0.0, -1.0, 0.0),
        Point3::new(0.0, 0.0, 1.0), Point3::new(0.0, 0.0, -1.0),
    ];
    let mut polytope: Vec<Point3> = dirs.iter()
        .map(|&d| support_minkowski_diff(shape_a, shape_b, d))
        .collect();
    let mut faces: Vec<[usize; 3]> = vec![
        [0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1],
        [5, 2, 1], [5, 3, 2], [5, 4, 3], [5, 1, 4],
    ];

    for _ in 0..64 {
        // Find closest face to origin
        let (min_dist, min_face_idx, min_normal) = faces.iter().enumerate()
            .map(|(fi, face)| {
                let a = polytope[face[0]]; let b = polytope[face[1]]; let c = polytope[face[2]];
                let n = (b - a).cross(c - a);
                let len = n.len();
                if len < 1e-12 { return (f64::INFINITY, fi, Point3::new(0.0, 0.0, 1.0)); }
                let n = n * (1.0 / len);
                let d = n.dot(a);
                (d.abs(), fi, if d >= 0.0 { n } else { -n })
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .unwrap();

        let _ = min_face_idx;
        let support = support_minkowski_diff(shape_a, shape_b, min_normal);
        let new_dist = min_normal.dot(support);

        if (new_dist - min_dist).abs() < 1e-6 {
            return Some((min_normal, min_dist));
        }

        // Expand polytope
        let si = polytope.len();
        polytope.push(support);
        let mut edge_set: Vec<(usize, usize)> = Vec::new();
        faces.retain(|face| {
            let a = polytope[face[0]]; let b = polytope[face[1]]; let c = polytope[face[2]];
            let n = (b - a).cross(c - a);
            if n.dot(support - a) > 0.0 {
                for &(ea, eb) in &[(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
                    let rev_pos = edge_set.iter().position(|&(x, y)| x == eb && y == ea);
                    if let Some(pos) = rev_pos { edge_set.remove(pos); }
                    else { edge_set.push((ea, eb)); }
                }
                false
            } else { true }
        });
        for (ea, eb) in edge_set {
            faces.push([ea, eb, si]);
        }
    }
    None
}

// ============================================================
// POLYGON OPERATIONS (continued)
// ============================================================

// segment_to_segment is already defined, segment_to_segment (Segment2 version) delegates to 3D

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_aabb_hit() {
        let ray = Ray3 { origin: Point3::new(0.0, 0.0, 0.0), dir: Point3::new(1.0, 0.0, 0.0) };
        let aabb = Aabb3 { min: Point3::new(2.0, -1.0, -1.0), max: Point3::new(4.0, 1.0, 1.0) };
        let t = ray_aabb(&ray, &aabb).unwrap();
        assert!((t - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_ray_aabb_miss() {
        let ray = Ray3 { origin: Point3::new(0.0, 0.0, 0.0), dir: Point3::new(0.0, 0.0, 1.0) };
        let aabb = Aabb3 { min: Point3::new(2.0, -1.0, -1.0), max: Point3::new(4.0, 1.0, 1.0) };
        assert!(ray_aabb(&ray, &aabb).is_none());
    }

    #[test]
    fn test_ray_sphere() {
        let ray = Ray3 { origin: Point3::new(-5.0, 0.0, 0.0), dir: Point3::new(1.0, 0.0, 0.0) };
        let sphere = Sphere { center: Point3::new(0.0, 0.0, 0.0), radius: 1.0 };
        let t = ray_sphere(&ray, &sphere).unwrap();
        assert!((t - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_ray_triangle_moller_trumbore() {
        let ray = Ray3 { origin: Point3::new(0.0, 0.0, -1.0), dir: Point3::new(0.0, 0.0, 1.0) };
        let tri = Triangle3 {
            a: Point3::new(-1.0, -1.0, 0.0),
            b: Point3::new(1.0, -1.0, 0.0),
            c: Point3::new(0.0, 1.0, 0.0),
        };
        let (t, u, v) = ray_triangle(&ray, &tri).unwrap();
        assert!((t - 1.0).abs() < 1e-10);
        let _ = (u, v);
    }

    #[test]
    fn test_aabb_aabb_overlap() {
        let a = Aabb3 { min: Point3::new(0.0, 0.0, 0.0), max: Point3::new(2.0, 2.0, 2.0) };
        let b = Aabb3 { min: Point3::new(1.0, 1.0, 1.0), max: Point3::new(3.0, 3.0, 3.0) };
        assert!(aabb_aabb(&a, &b));
    }

    #[test]
    fn test_aabb_aabb_no_overlap() {
        let a = Aabb3 { min: Point3::new(0.0, 0.0, 0.0), max: Point3::new(1.0, 1.0, 1.0) };
        let b = Aabb3 { min: Point3::new(2.0, 2.0, 2.0), max: Point3::new(3.0, 3.0, 3.0) };
        assert!(!aabb_aabb(&a, &b));
    }

    #[test]
    fn test_sphere_sphere() {
        let a = Sphere { center: Point3::new(0.0, 0.0, 0.0), radius: 1.0 };
        let b = Sphere { center: Point3::new(1.5, 0.0, 0.0), radius: 1.0 };
        assert!(sphere_sphere(&a, &b));
    }

    #[test]
    fn test_point_in_triangle() {
        let tri = Triangle2 {
            a: Point2::new(0.0, 0.0),
            b: Point2::new(1.0, 0.0),
            c: Point2::new(0.5, 1.0),
        };
        assert!(point_in_triangle(Point2::new(0.5, 0.4), &tri));
        assert!(!point_in_triangle(Point2::new(2.0, 0.0), &tri));
    }

    #[test]
    fn test_segment_segment_2d() {
        let a = Segment2 { a: Point2::new(0.0, 0.0), b: Point2::new(2.0, 2.0) };
        let b = Segment2 { a: Point2::new(0.0, 2.0), b: Point2::new(2.0, 0.0) };
        let p = segment_segment_2d(&a, &b).unwrap();
        assert!((p.x - 1.0).abs() < 1e-10);
        assert!((p.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_polygon_area() {
        let square = vec![
            Point2::new(0.0, 0.0), Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0), Point2::new(0.0, 1.0),
        ];
        assert!((polygon_area(&square) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_polygon_centroid() {
        let square = vec![
            Point2::new(0.0, 0.0), Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0), Point2::new(0.0, 2.0),
        ];
        let c = polygon_centroid(&square);
        assert!((c.x - 1.0).abs() < 1e-10);
        assert!((c.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_convex_hull_2d() {
        let pts = vec![
            Point2::new(0.0, 0.0), Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0), Point2::new(0.0, 1.0),
            Point2::new(0.5, 0.5), // interior
        ];
        let hull = convex_hull_2d(&pts);
        assert_eq!(hull.len(), 4);
    }

    #[test]
    fn test_point_in_convex_polygon() {
        let square = vec![
            Point2::new(0.0, 0.0), Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0), Point2::new(0.0, 1.0),
        ];
        assert!(point_in_convex_polygon(Point2::new(0.5, 0.5), &square));
        assert!(!point_in_convex_polygon(Point2::new(2.0, 0.5), &square));
    }

    #[test]
    fn test_ear_clipping() {
        let quad = vec![
            Point2::new(0.0, 0.0), Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0), Point2::new(0.0, 1.0),
        ];
        let tris = ear_clipping(&quad);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn test_delaunay_2d() {
        let pts = vec![
            Point2::new(0.0, 0.0), Point2::new(1.0, 0.0),
            Point2::new(0.5, 1.0), Point2::new(0.5, 0.3),
        ];
        let tris = delaunay_2d(&pts);
        assert!(!tris.is_empty());
    }

    #[test]
    fn test_sutherland_hodgman() {
        let subject = vec![
            Point2::new(0.0, 0.0), Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0), Point2::new(0.0, 2.0),
        ];
        let clip = vec![
            Point2::new(1.0, 1.0), Point2::new(3.0, 1.0),
            Point2::new(3.0, 3.0), Point2::new(1.0, 3.0),
        ];
        let result = sutherland_hodgman(&subject, &clip);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_point_to_plane() {
        let plane = Plane { normal: Point3::new(0.0, 1.0, 0.0), d: -2.0 };
        let p = Point3::new(0.0, 5.0, 0.0);
        let d = point_to_plane(p, &plane);
        assert!((d - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_to_aabb() {
        let aabb = Aabb3 { min: Point3::new(0.0, 0.0, 0.0), max: Point3::new(1.0, 1.0, 1.0) };
        assert!(point_to_aabb(Point3::new(0.5, 0.5, 0.5), &aabb).abs() < 1e-10);
        assert!((point_to_aabb(Point3::new(3.0, 0.5, 0.5), &aabb) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_obb_obb_overlap() {
        let center = Point3::new(0.0, 0.0, 0.0);
        let axes = [
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
        ];
        let a = Obb3 { center, axes, half_extents: Point3::new(1.0, 1.0, 1.0) };
        let b = Obb3 {
            center: Point3::new(1.5, 0.0, 0.0),
            axes,
            half_extents: Point3::new(1.0, 1.0, 1.0),
        };
        assert!(obb_obb(&a, &b));
    }

    #[test]
    fn test_gjk_overlapping() {
        let a: Vec<Point3> = vec![
            Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, -1.0, -1.0),
            Point3::new(0.0, 1.0, -1.0), Point3::new(0.0, 0.0, 1.0),
        ];
        let b: Vec<Point3> = vec![
            Point3::new(-0.5, -0.5, -0.5), Point3::new(0.5, -0.5, -0.5),
            Point3::new(0.0, 0.5, -0.5), Point3::new(0.0, 0.0, 0.5),
        ];
        assert!(gjk(&a, &b));
    }

    #[test]
    fn test_polygon_is_convex() {
        let convex = vec![
            Point2::new(0.0, 0.0), Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0), Point2::new(0.0, 1.0),
        ];
        assert!(polygon_is_convex(&convex));
    }

    #[test]
    fn test_ray_plane() {
        let plane = Plane { normal: Point3::new(0.0, 1.0, 0.0), d: -5.0 };
        let ray = Ray3 { origin: Point3::new(0.0, 0.0, 0.0), dir: Point3::new(0.0, 1.0, 0.0) };
        let t = ray_plane(&ray, &plane).unwrap();
        assert!((t - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_in_obb() {
        let obb = Obb3 {
            center: Point3::new(0.0, 0.0, 0.0),
            axes: [
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
            half_extents: Point3::new(2.0, 2.0, 2.0),
        };
        assert!(point_in_obb(Point3::new(1.0, 1.0, 1.0), &obb));
        assert!(!point_in_obb(Point3::new(3.0, 0.0, 0.0), &obb));
    }

    #[test]
    fn test_sphere_aabb() {
        let s = Sphere { center: Point3::new(2.0, 0.0, 0.0), radius: 1.5 };
        let b = Aabb3 { min: Point3::new(0.0, 0.0, 0.0), max: Point3::new(1.0, 1.0, 1.0) };
        assert!(sphere_aabb(&s, &b));
    }

    #[test]
    fn test_aabb_plane() {
        let aabb = Aabb3 { min: Point3::new(0.0, 0.0, 0.0), max: Point3::new(1.0, 1.0, 1.0) };
        let plane = Plane { normal: Point3::new(0.0, 1.0, 0.0), d: -10.0 };
        assert_eq!(aabb_plane(&aabb, &plane), Halfspace::Back);
    }

    #[test]
    fn test_point3_ops() {
        let a = Point3::new(1.0, 2.0, 3.0);
        let b = Point3::new(4.0, 5.0, 6.0);
        let c = a + b;
        assert_eq!(c, Point3::new(5.0, 7.0, 9.0));
        let d = b - a;
        assert_eq!(d, Point3::new(3.0, 3.0, 3.0));
        let e = a.cross(b);
        let dot = a.dot(b);
        assert!((dot - 32.0).abs() < 1e-10);
        let _ = e;
    }
}
