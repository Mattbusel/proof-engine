#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

const EPSILON: f32 = 1e-6;
const ADAPTIVE_SIMPSON_MAX_DEPTH: u32 = 12;
const ARC_LENGTH_SAMPLE_COUNT: usize = 512;
const NEWTON_MAX_ITER: u32 = 64;
const NEWTON_TOL: f32 = 1e-7;
const BINARY_SEARCH_ITER: u32 = 48;
const CURVATURE_COMB_SCALE: f32 = 0.1;
const DEFAULT_RAIL_GAUGE: f32 = 1.435; // standard gauge in meters
const PARALLEL_TRANSPORT_STEPS: usize = 256;

// ============================================================
// UTILITY MATH
// ============================================================

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn smooth_damp(current: f32, target: f32, velocity: &mut f32, smooth_time: f32, dt: f32) -> f32 {
    let omega = 2.0 / smooth_time.max(EPSILON);
    let x = omega * dt;
    let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = current - target;
    let temp = (*velocity + omega * change) * dt;
    *velocity = (*velocity - omega * temp) * exp;
    target + (change + temp) * exp
}

fn smooth_step(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

fn hash_f32_noise(n: i32) -> f32 {
    let n = (n << 13) ^ n;
    let n = n.wrapping_mul(n.wrapping_mul(n.wrapping_mul(15731) + 789221) + 1376312589);
    1.0 - (n & 0x7fffffff) as f32 / 1073741824.0
}

fn value_noise_1d(x: f32) -> f32 {
    let xi = x.floor() as i32;
    let xf = x - x.floor();
    let h0 = hash_f32_noise(xi);
    let h1 = hash_f32_noise(xi + 1);
    lerp(h0, h1, smooth_step(xf))
}

fn quintic_ease(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn quintic_ease_derivative(t: f32) -> f32 {
    let t = clamp01(t);
    30.0 * t * t * (t - 1.0) * (t - 1.0)
}

fn cubic_ease_in_out(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn safe_normalize(v: Vec3) -> Vec3 {
    let len = v.length();
    if len < EPSILON { Vec3::Z } else { v / len }
}

fn cross_safe(a: Vec3, b: Vec3) -> Vec3 {
    let c = a.cross(b);
    if c.length_squared() < EPSILON * EPSILON {
        // find a perpendicular vector
        let perp = if a.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        a.cross(perp).normalize_or_zero()
    } else {
        c.normalize()
    }
}

/// Adaptive Simpson's rule for 1D integration
fn adaptive_simpson(f: &dyn Fn(f32) -> f32, a: f32, b: f32, tol: f32, depth: u32) -> f32 {
    let c = (a + b) * 0.5;
    let fa = f(a);
    let fb = f(b);
    let fc = f(c);
    let s = (b - a) / 6.0 * (fa + 4.0 * fc + fb);
    adaptive_simpson_inner(f, a, b, fa, fb, fc, s, tol, depth)
}

fn adaptive_simpson_inner(
    f: &dyn Fn(f32) -> f32,
    a: f32, b: f32,
    fa: f32, fb: f32, fc: f32,
    s: f32, tol: f32, depth: u32
) -> f32 {
    let c = (a + b) * 0.5;
    let d = (a + c) * 0.5;
    let e = (c + b) * 0.5;
    let fd = f(d);
    let fe = f(e);
    let left  = (c - a) / 6.0 * (fa + 4.0 * fd + fc);
    let right = (b - c) / 6.0 * (fc + 4.0 * fe + fb);
    let delta = left + right - s;
    if depth == 0 || delta.abs() <= 15.0 * tol {
        left + right + delta / 15.0
    } else {
        adaptive_simpson_inner(f, a, c, fa, fc, fd, left,  tol * 0.5, depth - 1)
      + adaptive_simpson_inner(f, c, b, fc, fb, fe, right, tol * 0.5, depth - 1)
    }
}

fn integrate_arc_length(deriv: &dyn Fn(f32) -> f32, a: f32, b: f32) -> f32 {
    let speed = |t: f32| deriv(t);
    adaptive_simpson(&speed, a, b, 1e-5, ADAPTIVE_SIMPSON_MAX_DEPTH)
}

/// Build arc-length table: maps parameter t -> arc length
fn build_arc_length_table(
    sample_count: usize,
    position_fn: &dyn Fn(f32) -> Vec3,
) -> Vec<(f32, f32)> {
    let mut table = Vec::with_capacity(sample_count + 1);
    let mut cumulative = 0.0_f32;
    let mut prev = position_fn(0.0);
    table.push((0.0_f32, 0.0_f32));
    for i in 1..=sample_count {
        let t = i as f32 / sample_count as f32;
        let cur = position_fn(t);
        cumulative += (cur - prev).length();
        table.push((t, cumulative));
        prev = cur;
    }
    table
}

/// Invert arc-length table: given arc length s, return parameter t
fn arc_length_to_t(table: &[(f32, f32)], s: f32) -> f32 {
    if table.is_empty() { return 0.0; }
    let total = table.last().unwrap().1;
    let s = s.clamp(0.0, total);
    let idx = table.partition_point(|entry| entry.1 <= s);
    if idx == 0 { return table[0].0; }
    if idx >= table.len() { return table.last().unwrap().0; }
    let (t0, s0) = table[idx - 1];
    let (t1, s1) = table[idx];
    let frac = if (s1 - s0).abs() < EPSILON { 0.0 } else { (s - s0) / (s1 - s0) };
    lerp(t0, t1, frac)
}

// ============================================================
// FRENET-SERRET FRAME
// ============================================================

#[derive(Clone, Debug)]
pub struct FrenetFrame {
    pub position: Vec3,
    pub tangent:  Vec3,  // T
    pub normal:   Vec3,  // N
    pub binormal: Vec3,  // B
    pub curvature: f32,  // κ
    pub torsion:   f32,  // τ
}

impl FrenetFrame {
    pub fn identity() -> Self {
        FrenetFrame {
            position: Vec3::ZERO,
            tangent:  Vec3::X,
            normal:   Vec3::Y,
            binormal: Vec3::Z,
            curvature: 0.0,
            torsion:   0.0,
        }
    }

    pub fn compute(pos: Vec3, d1: Vec3, d2: Vec3, d3: Vec3) -> Self {
        // d1 = first derivative, d2 = second, d3 = third
        let speed = d1.length();
        let tangent = if speed > EPSILON { d1 / speed } else { Vec3::X };
        let d1_cross_d2 = d1.cross(d2);
        let kappa_vec_len = d1_cross_d2.length();
        let curvature = if speed > EPSILON {
            kappa_vec_len / speed.powi(3)
        } else {
            0.0
        };
        let binormal = if kappa_vec_len > EPSILON {
            d1_cross_d2 / kappa_vec_len
        } else {
            Vec3::Z
        };
        let normal = binormal.cross(tangent);

        // Torsion: τ = (d1 × d2) · d3 / |d1 × d2|²
        let torsion = if kappa_vec_len > EPSILON {
            d1_cross_d2.dot(d3) / kappa_vec_len.powi(2)
        } else {
            0.0
        };

        FrenetFrame { position: pos, tangent, normal, binormal, curvature, torsion }
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_cols(
            Vec4::new(self.tangent.x,  self.tangent.y,  self.tangent.z,  0.0),
            Vec4::new(self.normal.x,   self.normal.y,   self.normal.z,   0.0),
            Vec4::new(self.binormal.x, self.binormal.y, self.binormal.z, 0.0),
            Vec4::new(self.position.x, self.position.y, self.position.z, 1.0),
        )
    }
}

// ============================================================
// PARALLEL TRANSPORT FRAME
// ============================================================

#[derive(Clone, Debug)]
pub struct ParallelTransportFrame {
    pub position: Vec3,
    pub tangent:  Vec3,
    pub normal:   Vec3,
    pub binormal: Vec3,
}

impl ParallelTransportFrame {
    /// Double-reflection parallel transport
    pub fn transport(prev: &ParallelTransportFrame, new_pos: Vec3, new_tangent: Vec3) -> Self {
        let t_prev = prev.tangent;
        let t_next = safe_normalize(new_tangent);
        let v1 = new_pos - prev.position;
        let c1 = v1.dot(v1);
        let r_l = if c1 > EPSILON { prev.normal  - (2.0 / c1) * v1.dot(prev.normal)  * v1 } else { prev.normal };
        let t_l = if c1 > EPSILON { t_prev - (2.0 / c1) * v1.dot(t_prev) * v1 } else { t_prev };
        let v2 = t_next - t_l;
        let c2 = v2.dot(v2);
        let normal = if c2 > EPSILON { r_l - (2.0 / c2) * v2.dot(r_l) * v2 } else { r_l };
        let normal = safe_normalize(normal);
        let binormal = safe_normalize(t_next.cross(normal));
        ParallelTransportFrame { position: new_pos, tangent: t_next, normal, binormal }
    }

    pub fn initial(position: Vec3, tangent: Vec3) -> Self {
        let t = safe_normalize(tangent);
        let perp = if t.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        let normal = safe_normalize(t.cross(perp).cross(t));
        let binormal = safe_normalize(t.cross(normal));
        ParallelTransportFrame { position, tangent: t, normal, binormal }
    }
}

// ============================================================
// CONTROL POINT
// ============================================================

#[derive(Clone, Debug)]
pub struct ControlPoint {
    pub position:  Vec3,
    pub tangent_in:  Vec3,
    pub tangent_out: Vec3,
    pub weight: f32,          // for NURBS
    pub knot_value: f32,      // for B-Spline/NURBS
    pub id: u64,
    pub tension: f32,         // for Hermite/Kochanek-Bartels
}

impl ControlPoint {
    pub fn new(position: Vec3) -> Self {
        ControlPoint {
            position,
            tangent_in:  Vec3::ZERO,
            tangent_out: Vec3::ZERO,
            weight: 1.0,
            knot_value: 0.0,
            id: rand_id(),
            tension: 0.0,
        }
    }

    pub fn with_tangents(position: Vec3, t_in: Vec3, t_out: Vec3) -> Self {
        let mut cp = Self::new(position);
        cp.tangent_in  = t_in;
        cp.tangent_out = t_out;
        cp
    }
}

static CONTROL_POINT_ID_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

fn rand_id() -> u64 {
    CONTROL_POINT_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// ============================================================
// SPLINE TYPES ENUM
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum SplineType {
    CatmullRom,
    CubicBezier,
    BSpline { degree: usize },
    Nurbs    { degree: usize },
    Hermite,
}

// ============================================================
// CATMULL-ROM SPLINE (centripetal parameterization)
// ============================================================

#[derive(Clone, Debug)]
pub struct CatmullRomSpline {
    pub control_points: Vec<ControlPoint>,
    pub closed: bool,
    pub alpha: f32,  // 0=uniform, 0.5=centripetal, 1=chordal
    arc_length_table: Vec<(f32, f32)>,
    total_length: f32,
}

impl CatmullRomSpline {
    pub fn new(points: Vec<Vec3>, alpha: f32, closed: bool) -> Self {
        let control_points = points.into_iter().map(ControlPoint::new).collect();
        let mut s = CatmullRomSpline {
            control_points,
            closed,
            alpha,
            arc_length_table: Vec::new(),
            total_length: 0.0,
        };
        s.rebuild_arc_length_table();
        s
    }

    fn num_segments(&self) -> usize {
        let n = self.control_points.len();
        if n < 2 { return 0; }
        if self.closed { n } else { n - 1 }
    }

    fn get_point(&self, i: usize) -> Vec3 {
        let n = self.control_points.len();
        self.control_points[i % n].position
    }

    fn segment_t_values(&self, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3) -> [f32; 4] {
        let t0 = 0.0_f32;
        let t1 = t0 + (p1 - p0).length().powf(self.alpha);
        let t2 = t1 + (p2 - p1).length().powf(self.alpha);
        let t3 = t2 + (p3 - p2).length().powf(self.alpha);
        [t0, t1, t2, t3]
    }

    /// Evaluate position at local segment parameter u in [0,1]
    pub fn eval_segment(&self, seg: usize, u: f32) -> Vec3 {
        let n = self.control_points.len();
        if n < 2 { return Vec3::ZERO; }
        let (i0, i1, i2, i3) = self.segment_indices(seg);
        let p0 = self.get_point(i0);
        let p1 = self.get_point(i1);
        let p2 = self.get_point(i2);
        let p3 = self.get_point(i3);
        let [t0, t1, t2, t3] = self.segment_t_values(p0, p1, p2, p3);
        let t = lerp(t1, t2, u);
        self.barry_phase(p0, p1, p2, p3, t0, t1, t2, t3, t)
    }

    fn barry_phase(&self, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3,
                   t0: f32, t1: f32, t2: f32, t3: f32, t: f32) -> Vec3 {
        let safe_div = |n: Vec3, d: f32| if d.abs() < EPSILON { Vec3::ZERO } else { n / d };
        let a1 = safe_div(p0 * (t1 - t) + p1 * (t - t0), t1 - t0);
        let a2 = safe_div(p1 * (t2 - t) + p2 * (t - t1), t2 - t1);
        let a3 = safe_div(p2 * (t3 - t) + p3 * (t - t2), t3 - t2);
        let b1 = safe_div(a1 * (t2 - t) + a2 * (t - t0), t2 - t0);
        let b2 = safe_div(a2 * (t3 - t) + a3 * (t - t1), t3 - t1);
        safe_div(b1 * (t2 - t) + b2 * (t - t1), t2 - t1)
    }

    fn segment_indices(&self, seg: usize) -> (usize, usize, usize, usize) {
        let n = self.control_points.len();
        if self.closed {
            let i1 = seg % n;
            let i2 = (seg + 1) % n;
            let i0 = (seg + n - 1) % n;
            let i3 = (seg + 2) % n;
            (i0, i1, i2, i3)
        } else {
            let i1 = seg.min(n - 1);
            let i2 = (seg + 1).min(n - 1);
            let i0 = if seg == 0 { 0 } else { seg - 1 };
            let i3 = (seg + 2).min(n - 1);
            (i0, i1, i2, i3)
        }
    }

    /// Global parameter t in [0,1] -> position
    pub fn evaluate(&self, t: f32) -> Vec3 {
        let nseg = self.num_segments();
        if nseg == 0 { return Vec3::ZERO; }
        let t = if self.closed { t.fract() } else { clamp01(t) };
        let scaled = t * nseg as f32;
        let seg = (scaled as usize).min(nseg - 1);
        let u   = scaled - seg as f32;
        self.eval_segment(seg, u)
    }

    pub fn evaluate_derivative(&self, t: f32) -> Vec3 {
        let dt = 1e-4;
        let t = clamp01(t);
        let fwd  = self.evaluate((t + dt).min(1.0));
        let back = self.evaluate((t - dt).max(0.0));
        (fwd - back) / (2.0 * dt)
    }

    pub fn evaluate_second_derivative(&self, t: f32) -> Vec3 {
        let dt = 1e-4;
        let t = clamp01(t);
        let fwd   = self.evaluate((t + dt).min(1.0));
        let cur   = self.evaluate(t);
        let back  = self.evaluate((t - dt).max(0.0));
        (fwd - 2.0 * cur + back) / (dt * dt)
    }

    pub fn evaluate_third_derivative(&self, t: f32) -> Vec3 {
        let dt = 1e-4;
        let t = clamp01(t);
        let p3 = self.evaluate((t + 2.0 * dt).min(1.0));
        let p1 = self.evaluate((t + dt).min(1.0));
        let m1 = self.evaluate((t - dt).max(0.0));
        let m3 = self.evaluate((t - 2.0 * dt).max(0.0));
        (-p3 + 2.0 * p1 - 2.0 * m1 + m3) / (2.0 * dt.powi(3))
    }

    pub fn frenet_frame_at(&self, t: f32) -> FrenetFrame {
        let pos = self.evaluate(t);
        let d1  = self.evaluate_derivative(t);
        let d2  = self.evaluate_second_derivative(t);
        let d3  = self.evaluate_third_derivative(t);
        FrenetFrame::compute(pos, d1, d2, d3)
    }

    pub fn rebuild_arc_length_table(&mut self) {
        let table = build_arc_length_table(ARC_LENGTH_SAMPLE_COUNT, &|t| self.evaluate(t));
        self.total_length = table.last().map(|e| e.1).unwrap_or(0.0);
        self.arc_length_table = table;
    }

    pub fn total_arc_length(&self) -> f32 { self.total_length }

    pub fn t_at_arc_length(&self, s: f32) -> f32 {
        arc_length_to_t(&self.arc_length_table, s)
    }

    pub fn evaluate_at_arc_length(&self, s: f32) -> Vec3 {
        self.evaluate(self.t_at_arc_length(s))
    }

    pub fn curvature_at(&self, t: f32) -> f32 {
        self.frenet_frame_at(t).curvature
    }

    pub fn torsion_at(&self, t: f32) -> f32 {
        self.frenet_frame_at(t).torsion
    }

    /// Nearest point on spline via binary search + Newton refinement
    pub fn nearest_point(&self, query: Vec3) -> (f32, Vec3) {
        let mut best_t   = 0.0_f32;
        let mut best_d2  = f32::MAX;
        let steps = 128usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let p = self.evaluate(t);
            let d2 = (p - query).length_squared();
            if d2 < best_d2 {
                best_d2 = d2;
                best_t  = t;
            }
        }
        // Newton refinement
        let t = newton_nearest_on_spline(best_t, query, &|t| self.evaluate(t),
                                         &|t| self.evaluate_derivative(t));
        (t, self.evaluate(t))
    }

    /// Insert a knot at parameter t, splitting a segment
    pub fn insert_knot(&mut self, t: f32) {
        let pos = self.evaluate(t);
        let idx = {
            let nseg = self.num_segments();
            let scaled = clamp01(t) * nseg as f32;
            (scaled as usize).min(nseg.saturating_sub(1))
        };
        let new_cp = ControlPoint::new(pos);
        self.control_points.insert(idx + 1, new_cp);
        self.rebuild_arc_length_table();
    }

    pub fn split_at(&self, t: f32) -> (CatmullRomSpline, CatmullRomSpline) {
        let n = self.control_points.len();
        let nseg = self.num_segments();
        let scaled = clamp01(t) * nseg as f32;
        let seg = (scaled as usize).min(nseg.saturating_sub(1));
        let split_idx = seg + 1;
        let pts_a: Vec<Vec3> = self.control_points[..split_idx.min(n)].iter()
            .map(|cp| cp.position).collect();
        let pts_b: Vec<Vec3> = self.control_points[split_idx.min(n)..].iter()
            .map(|cp| cp.position).collect();
        let mut a = CatmullRomSpline::new(pts_a, self.alpha, false);
        let mut b = CatmullRomSpline::new(pts_b, self.alpha, false);
        // add split point to both
        let split_pos = self.evaluate(t);
        a.control_points.push(ControlPoint::new(split_pos));
        if !b.control_points.is_empty() {
            b.control_points.insert(0, ControlPoint::new(split_pos));
        } else {
            b.control_points.push(ControlPoint::new(split_pos));
        }
        a.rebuild_arc_length_table();
        b.rebuild_arc_length_table();
        (a, b)
    }

    pub fn join(mut a: CatmullRomSpline, b: CatmullRomSpline) -> CatmullRomSpline {
        for cp in b.control_points {
            a.control_points.push(cp);
        }
        a.rebuild_arc_length_table();
        a
    }

    pub fn toggle_closed(&mut self) {
        self.closed = !self.closed;
        self.rebuild_arc_length_table();
    }

    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let steps = 200;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let p = self.evaluate(t);
            min = min.min(p);
            max = max.max(p);
        }
        (min, max)
    }
}

// ============================================================
// CUBIC BEZIER SPLINE (De Casteljau)
// ============================================================

#[derive(Clone, Debug)]
pub struct CubicBezierSpline {
    /// Control points in groups of 4: P0, P1, P2, P3 per segment
    /// Adjacent segments share endpoints: P3 of seg i == P0 of seg i+1
    pub segments: Vec<[Vec3; 4]>,
    pub closed: bool,
    arc_length_table: Vec<(f32, f32)>,
    total_length: f32,
}

impl CubicBezierSpline {
    pub fn new(segments: Vec<[Vec3; 4]>) -> Self {
        let mut s = CubicBezierSpline {
            segments,
            closed: false,
            arc_length_table: Vec::new(),
            total_length: 0.0,
        };
        s.rebuild_arc_length_table();
        s
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        // Auto-generate smooth cubic bezier from polyline
        let n = points.len();
        if n < 2 {
            return CubicBezierSpline::new(Vec::new());
        }
        let mut segs = Vec::new();
        for i in 0..n.saturating_sub(1) {
            let p0 = points[i];
            let p3 = points[i + 1];
            let prev = if i > 0 { points[i - 1] } else { p0 };
            let next = if i + 2 < n { points[i + 2] } else { p3 };
            let p1 = p0 + (p3 - prev) * (1.0 / 6.0);
            let p2 = p3 - (next - p0) * (1.0 / 6.0);
            segs.push([p0, p1, p2, p3]);
        }
        CubicBezierSpline::new(segs)
    }

    /// De Casteljau evaluation at t in [0,1] for a single segment
    pub fn de_casteljau(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
        let q0 = lerp_vec3(p0, p1, t);
        let q1 = lerp_vec3(p1, p2, t);
        let q2 = lerp_vec3(p2, p3, t);
        let r0 = lerp_vec3(q0, q1, t);
        let r1 = lerp_vec3(q1, q2, t);
        lerp_vec3(r0, r1, t)
    }

    /// De Casteljau split: returns left and right halves
    pub fn de_casteljau_split(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32)
        -> ([Vec3; 4], [Vec3; 4])
    {
        let q0 = lerp_vec3(p0, p1, t);
        let q1 = lerp_vec3(p1, p2, t);
        let q2 = lerp_vec3(p2, p3, t);
        let r0 = lerp_vec3(q0, q1, t);
        let r1 = lerp_vec3(q1, q2, t);
        let s  = lerp_vec3(r0, r1, t);
        ([p0, q0, r0, s], [s, r1, q2, p3])
    }

    pub fn num_segments(&self) -> usize { self.segments.len() }

    pub fn evaluate(&self, t: f32) -> Vec3 {
        let n = self.segments.len();
        if n == 0 { return Vec3::ZERO; }
        let t = clamp01(t);
        let scaled = t * n as f32;
        let seg = (scaled as usize).min(n - 1);
        let u   = scaled - seg as f32;
        let [p0, p1, p2, p3] = self.segments[seg];
        Self::de_casteljau(p0, p1, p2, p3, u)
    }

    pub fn evaluate_derivative(&self, t: f32) -> Vec3 {
        let n = self.segments.len();
        if n == 0 { return Vec3::ZERO; }
        let t = clamp01(t);
        let scaled = t * n as f32;
        let seg = (scaled as usize).min(n - 1);
        let u   = scaled - seg as f32;
        let [p0, p1, p2, p3] = self.segments[seg];
        // Derivative of cubic bezier: 3*(B(u) where control pts are differences)
        let d0 = 3.0 * (p1 - p0);
        let d1 = 3.0 * (p2 - p1);
        let d2 = 3.0 * (p3 - p2);
        Self::de_casteljau(d0, d1, d2, Vec3::ZERO, u) // quadratic bezier
        // Actually: derivative is a quadratic bezier with 3 control pts d0,d1,d2
        // evaluated at u:
        // = (1-u)^2 d0 + 2u(1-u) d1 + u^2 d2
    }

    pub fn evaluate_derivative_correct(&self, t: f32) -> Vec3 {
        let n = self.segments.len();
        if n == 0 { return Vec3::ZERO; }
        let t = clamp01(t);
        let scaled = t * n as f32;
        let seg = (scaled as usize).min(n - 1);
        let u   = scaled - seg as f32;
        let [p0, p1, p2, p3] = self.segments[seg];
        let u2 = u * u;
        let t1 = 1.0 - u;
        let t12 = t1 * t1;
        // B'(u) = 3[(p1-p0)(1-u)^2 + 2(p2-p1)u(1-u) + (p3-p2)u^2]
        3.0 * ((p1 - p0) * t12 + 2.0 * (p2 - p1) * u * t1 + (p3 - p2) * u2)
    }

    pub fn evaluate_second_derivative_correct(&self, t: f32) -> Vec3 {
        let n = self.segments.len();
        if n == 0 { return Vec3::ZERO; }
        let t = clamp01(t);
        let scaled = t * n as f32;
        let seg = (scaled as usize).min(n - 1);
        let u   = scaled - seg as f32;
        let [p0, p1, p2, p3] = self.segments[seg];
        // B''(u) = 6[(p2-2p1+p0)(1-u) + (p3-2p2+p1)u]
        6.0 * ((p2 - 2.0 * p1 + p0) * (1.0 - u) + (p3 - 2.0 * p2 + p1) * u)
    }

    pub fn curvature_at(&self, t: f32) -> f32 {
        let d1 = self.evaluate_derivative_correct(t);
        let d2 = self.evaluate_second_derivative_correct(t);
        let cross = d1.cross(d2).length();
        let speed = d1.length();
        if speed < EPSILON { 0.0 } else { cross / speed.powi(3) }
    }

    pub fn rebuild_arc_length_table(&mut self) {
        let table = build_arc_length_table(ARC_LENGTH_SAMPLE_COUNT, &|t| self.evaluate(t));
        self.total_length = table.last().map(|e| e.1).unwrap_or(0.0);
        self.arc_length_table = table;
    }

    pub fn total_arc_length(&self) -> f32 { self.total_length }

    pub fn t_at_arc_length(&self, s: f32) -> f32 {
        arc_length_to_t(&self.arc_length_table, s)
    }

    pub fn evaluate_at_arc_length(&self, s: f32) -> Vec3 {
        self.evaluate(self.t_at_arc_length(s))
    }

    pub fn split_segment(&mut self, seg: usize, u: f32) {
        if seg >= self.segments.len() { return; }
        let [p0, p1, p2, p3] = self.segments[seg];
        let (left, right) = Self::de_casteljau_split(p0, p1, p2, p3, u);
        self.segments.remove(seg);
        self.segments.insert(seg, right);
        self.segments.insert(seg, left);
        self.rebuild_arc_length_table();
    }

    pub fn nearest_point(&self, query: Vec3) -> (f32, Vec3) {
        let mut best_t = 0.0_f32;
        let mut best_d2 = f32::MAX;
        let steps = 200usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let p = self.evaluate(t);
            let d2 = (p - query).length_squared();
            if d2 < best_d2 {
                best_d2 = d2;
                best_t = t;
            }
        }
        let t = newton_nearest_on_spline(best_t, query,
            &|t| self.evaluate(t),
            &|t| self.evaluate_derivative_correct(t));
        (t, self.evaluate(t))
    }

    pub fn frenet_frame_at(&self, t: f32) -> FrenetFrame {
        let pos = self.evaluate(t);
        let d1  = self.evaluate_derivative_correct(t);
        let d2  = self.evaluate_second_derivative_correct(t);
        let dt = 1e-4;
        let d2a = self.evaluate_second_derivative_correct((t + dt).min(1.0));
        let d2b = self.evaluate_second_derivative_correct((t - dt).max(0.0));
        let d3  = (d2a - d2b) / (2.0 * dt);
        FrenetFrame::compute(pos, d1, d2, d3)
    }

    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for seg in &self.segments {
            for &p in seg.iter() {
                min = min.min(p);
                max = max.max(p);
            }
        }
        (min, max)
    }
}

// ============================================================
// B-SPLINE (Cox-de Boor recursion)
// ============================================================

#[derive(Clone, Debug)]
pub struct BSpline {
    pub control_points: Vec<Vec3>,
    pub knots: Vec<f32>,
    pub degree: usize,
    pub closed: bool,
    arc_length_table: Vec<(f32, f32)>,
    total_length: f32,
}

impl BSpline {
    pub fn new(control_points: Vec<Vec3>, degree: usize, closed: bool) -> Self {
        let mut s = BSpline {
            knots: Vec::new(),
            control_points,
            degree,
            closed,
            arc_length_table: Vec::new(),
            total_length: 0.0,
        };
        s.generate_uniform_knots();
        s.rebuild_arc_length_table();
        s
    }

    pub fn generate_uniform_knots(&mut self) {
        let n = self.control_points.len();
        let k = self.degree;
        // Clamped uniform knot vector: m = n + k + 1 knots
        let m = n + k + 1;
        let mut knots = Vec::with_capacity(m);
        for i in 0..m {
            if i < k + 1 {
                knots.push(0.0);
            } else if i > n {
                knots.push(1.0);
            } else {
                knots.push((i - k) as f32 / (n - k) as f32);
            }
        }
        self.knots = knots;
    }

    /// Cox-de Boor basis function N_{i,k}(t)
    fn basis(&self, i: usize, k: usize, t: f32) -> f32 {
        if k == 0 {
            let a = self.knots.get(i).cloned().unwrap_or(0.0);
            let b = self.knots.get(i + 1).cloned().unwrap_or(0.0);
            if t >= a && t < b { 1.0 } else { 0.0 }
        } else {
            let ti   = self.knots.get(i).cloned().unwrap_or(0.0);
            let tik  = self.knots.get(i + k).cloned().unwrap_or(0.0);
            let ti1  = self.knots.get(i + 1).cloned().unwrap_or(0.0);
            let tik1 = self.knots.get(i + k + 1).cloned().unwrap_or(0.0);
            let left = if (tik - ti).abs() < EPSILON { 0.0 }
                       else { (t - ti) / (tik - ti) * self.basis(i, k - 1, t) };
            let right = if (tik1 - ti1).abs() < EPSILON { 0.0 }
                        else { (tik1 - t) / (tik1 - ti1) * self.basis(i + 1, k - 1, t) };
            left + right
        }
    }

    pub fn evaluate(&self, t: f32) -> Vec3 {
        let n = self.control_points.len();
        if n == 0 { return Vec3::ZERO; }
        let t_min = self.knots.first().cloned().unwrap_or(0.0);
        let t_max = self.knots.last().cloned().unwrap_or(1.0);
        // Clamp t slightly to avoid endpoint issues
        let t = t.clamp(t_min, t_max - EPSILON);
        let mut result = Vec3::ZERO;
        for i in 0..n {
            let b = self.basis(i, self.degree, t);
            result += self.control_points[i] * b;
        }
        result
    }

    pub fn evaluate_derivative(&self, t: f32) -> Vec3 {
        let dt = 1e-4;
        let a = self.evaluate((t + dt).min(1.0 - EPSILON));
        let b = self.evaluate((t - dt).max(EPSILON));
        (a - b) / (2.0 * dt)
    }

    pub fn evaluate_second_derivative(&self, t: f32) -> Vec3 {
        let dt = 1e-4;
        let a = self.evaluate((t + dt).min(1.0 - EPSILON));
        let c = self.evaluate(t);
        let b = self.evaluate((t - dt).max(EPSILON));
        (a - 2.0 * c + b) / (dt * dt)
    }

    pub fn rebuild_arc_length_table(&mut self) {
        let table = build_arc_length_table(ARC_LENGTH_SAMPLE_COUNT, &|t| self.evaluate(t));
        self.total_length = table.last().map(|e| e.1).unwrap_or(0.0);
        self.arc_length_table = table;
    }

    pub fn total_arc_length(&self) -> f32 { self.total_length }

    pub fn t_at_arc_length(&self, s: f32) -> f32 {
        arc_length_to_t(&self.arc_length_table, s)
    }

    pub fn evaluate_at_arc_length(&self, s: f32) -> Vec3 {
        self.evaluate(self.t_at_arc_length(s))
    }

    pub fn frenet_frame_at(&self, t: f32) -> FrenetFrame {
        let pos = self.evaluate(t);
        let d1  = self.evaluate_derivative(t);
        let d2  = self.evaluate_second_derivative(t);
        let dt = 1e-4;
        let d2a = self.evaluate_second_derivative((t + dt).min(1.0 - EPSILON));
        let d2b = self.evaluate_second_derivative((t - dt).max(EPSILON));
        let d3  = (d2a - d2b) / (2.0 * dt);
        FrenetFrame::compute(pos, d1, d2, d3)
    }

    pub fn curvature_at(&self, t: f32) -> f32 {
        self.frenet_frame_at(t).curvature
    }

    /// Knot insertion using Boehm's algorithm
    pub fn insert_knot(&mut self, t_new: f32) {
        // find span
        let n  = self.control_points.len();
        let k  = self.degree;
        let mut r = 0usize;
        for i in 0..self.knots.len().saturating_sub(1) {
            if self.knots[i] <= t_new && t_new < self.knots[i + 1] {
                r = i;
            }
        }
        // new control points
        let mut new_pts = Vec::with_capacity(n + 1);
        for i in 0..=n {
            if i <= r.saturating_sub(k) {
                new_pts.push(self.control_points.get(i).cloned().unwrap_or(Vec3::ZERO));
            } else if i > r {
                new_pts.push(self.control_points.get(i.saturating_sub(1)).cloned().unwrap_or(Vec3::ZERO));
            } else {
                let ti   = self.knots.get(i).cloned().unwrap_or(0.0);
                let tik1 = self.knots.get(i + k).cloned().unwrap_or(1.0);
                let alpha = if (tik1 - ti).abs() < EPSILON { 0.5 }
                            else { (t_new - ti) / (tik1 - ti) };
                let prev = self.control_points.get(i.saturating_sub(1)).cloned().unwrap_or(Vec3::ZERO);
                let curr = self.control_points.get(i).cloned().unwrap_or(Vec3::ZERO);
                new_pts.push(lerp_vec3(prev, curr, alpha));
            }
        }
        self.control_points = new_pts;
        self.knots.insert(r + 1, t_new);
        self.rebuild_arc_length_table();
    }

    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for &p in &self.control_points {
            min = min.min(p);
            max = max.max(p);
        }
        (min, max)
    }
}

// ============================================================
// NURBS (Rational B-Spline)
// ============================================================

#[derive(Clone, Debug)]
pub struct NurbsSpline {
    pub control_points: Vec<Vec3>,
    pub weights: Vec<f32>,
    pub knots: Vec<f32>,
    pub degree: usize,
    pub closed: bool,
    arc_length_table: Vec<(f32, f32)>,
    total_length: f32,
}

impl NurbsSpline {
    pub fn new(control_points: Vec<Vec3>, weights: Vec<f32>, degree: usize) -> Self {
        let n = control_points.len();
        assert_eq!(weights.len(), n, "NURBS: weights and control points must match");
        let mut s = NurbsSpline {
            control_points,
            weights,
            knots: Vec::new(),
            degree,
            closed: false,
            arc_length_table: Vec::new(),
            total_length: 0.0,
        };
        s.generate_uniform_knots();
        s.rebuild_arc_length_table();
        s
    }

    fn generate_uniform_knots(&mut self) {
        let n = self.control_points.len();
        let k = self.degree;
        let m = n + k + 1;
        let mut knots = Vec::with_capacity(m);
        for i in 0..m {
            if i < k + 1 { knots.push(0.0); }
            else if i > n { knots.push(1.0); }
            else { knots.push((i - k) as f32 / (n - k) as f32); }
        }
        self.knots = knots;
    }

    fn basis(&self, i: usize, k: usize, t: f32) -> f32 {
        if k == 0 {
            let a = self.knots.get(i).cloned().unwrap_or(0.0);
            let b = self.knots.get(i + 1).cloned().unwrap_or(0.0);
            if t >= a && t < b { 1.0 } else { 0.0 }
        } else {
            let ti   = self.knots.get(i).cloned().unwrap_or(0.0);
            let tik  = self.knots.get(i + k).cloned().unwrap_or(0.0);
            let ti1  = self.knots.get(i + 1).cloned().unwrap_or(0.0);
            let tik1 = self.knots.get(i + k + 1).cloned().unwrap_or(0.0);
            let left  = if (tik - ti).abs() < EPSILON { 0.0 }
                        else { (t - ti) / (tik - ti) * self.basis(i, k - 1, t) };
            let right = if (tik1 - ti1).abs() < EPSILON { 0.0 }
                        else { (tik1 - t) / (tik1 - ti1) * self.basis(i + 1, k - 1, t) };
            left + right
        }
    }

    pub fn evaluate(&self, t: f32) -> Vec3 {
        let n = self.control_points.len();
        if n == 0 { return Vec3::ZERO; }
        let t_max = self.knots.last().cloned().unwrap_or(1.0);
        let t = t.clamp(0.0, t_max - EPSILON);
        let mut numerator   = Vec3::ZERO;
        let mut denominator = 0.0_f32;
        for i in 0..n {
            let b = self.basis(i, self.degree, t);
            let w = self.weights[i];
            numerator   += self.control_points[i] * (b * w);
            denominator += b * w;
        }
        if denominator.abs() < EPSILON { Vec3::ZERO } else { numerator / denominator }
    }

    pub fn evaluate_derivative(&self, t: f32) -> Vec3 {
        let dt = 1e-4;
        let a = self.evaluate((t + dt).min(1.0 - EPSILON));
        let b = self.evaluate((t - dt).max(EPSILON));
        (a - b) / (2.0 * dt)
    }

    pub fn evaluate_second_derivative(&self, t: f32) -> Vec3 {
        let dt = 1e-4;
        let a = self.evaluate((t + dt).min(1.0 - EPSILON));
        let c = self.evaluate(t);
        let b = self.evaluate((t - dt).max(EPSILON));
        (a - 2.0 * c + b) / (dt * dt)
    }

    pub fn rebuild_arc_length_table(&mut self) {
        let table = build_arc_length_table(ARC_LENGTH_SAMPLE_COUNT, &|t| self.evaluate(t));
        self.total_length = table.last().map(|e| e.1).unwrap_or(0.0);
        self.arc_length_table = table;
    }

    pub fn total_arc_length(&self) -> f32 { self.total_length }

    pub fn t_at_arc_length(&self, s: f32) -> f32 {
        arc_length_to_t(&self.arc_length_table, s)
    }

    pub fn evaluate_at_arc_length(&self, s: f32) -> Vec3 {
        self.evaluate(self.t_at_arc_length(s))
    }

    pub fn frenet_frame_at(&self, t: f32) -> FrenetFrame {
        let pos = self.evaluate(t);
        let d1  = self.evaluate_derivative(t);
        let d2  = self.evaluate_second_derivative(t);
        let dt  = 1e-4;
        let d2a = self.evaluate_second_derivative((t + dt).min(1.0 - EPSILON));
        let d2b = self.evaluate_second_derivative((t - dt).max(EPSILON));
        let d3  = (d2a - d2b) / (2.0 * dt);
        FrenetFrame::compute(pos, d1, d2, d3)
    }

    pub fn curvature_at(&self, t: f32) -> f32 {
        self.frenet_frame_at(t).curvature
    }

    pub fn circle_nurbs(center: Vec3, radius: f32, normal: Vec3) -> NurbsSpline {
        // Quarter-arc NURBS circle (9 control points, degree 2)
        let up = safe_normalize(normal.cross(Vec3::X));
        let right = safe_normalize(normal.cross(up));
        let r = radius;
        let w = std::f32::consts::FRAC_1_SQRT_2; // cos(45°)
        let mut pts = Vec::new();
        let mut wts = Vec::new();
        // 9-point NURBS circle
        let angles = [0.0_f32, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0, 360.0];
        for (i, &a) in angles.iter().enumerate() {
            let rad = a.to_radians();
            let pt = center + right * (rad.cos() * r) + up * (rad.sin() * r);
            pts.push(pt);
            if i % 2 == 0 { wts.push(1.0); } else { wts.push(w); }
        }
        let knots = vec![0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0];
        NurbsSpline {
            control_points: pts,
            weights: wts,
            knots,
            degree: 2,
            closed: true,
            arc_length_table: Vec::new(),
            total_length: 0.0,
        }
    }
}

// ============================================================
// HERMITE SPLINE
// ============================================================

#[derive(Clone, Debug)]
pub struct HermiteSpline {
    /// Each entry: (position, tangent)
    pub control_points: Vec<(Vec3, Vec3)>,
    pub closed: bool,
    arc_length_table: Vec<(f32, f32)>,
    total_length: f32,
}

impl HermiteSpline {
    pub fn new(points: Vec<(Vec3, Vec3)>) -> Self {
        let mut s = HermiteSpline {
            control_points: points,
            closed: false,
            arc_length_table: Vec::new(),
            total_length: 0.0,
        };
        s.rebuild_arc_length_table();
        s
    }

    pub fn num_segments(&self) -> usize {
        let n = self.control_points.len();
        if n < 2 { 0 }
        else if self.closed { n }
        else { n - 1 }
    }

    pub fn eval_segment(&self, seg: usize, u: f32) -> Vec3 {
        let n = self.control_points.len();
        let i0 = seg % n;
        let i1 = (seg + 1) % n;
        let (p0, m0) = self.control_points[i0];
        let (p1, m1) = self.control_points[i1];
        // Cubic Hermite basis functions
        let u2 = u * u;
        let u3 = u2 * u;
        let h00 =  2.0 * u3 - 3.0 * u2 + 1.0;
        let h10 =        u3 - 2.0 * u2 + u;
        let h01 = -2.0 * u3 + 3.0 * u2;
        let h11 =        u3 -       u2;
        p0 * h00 + m0 * h10 + p1 * h01 + m1 * h11
    }

    pub fn eval_segment_derivative(&self, seg: usize, u: f32) -> Vec3 {
        let n = self.control_points.len();
        let i0 = seg % n;
        let i1 = (seg + 1) % n;
        let (p0, m0) = self.control_points[i0];
        let (p1, m1) = self.control_points[i1];
        let u2 = u * u;
        let dh00 =  6.0 * u2 - 6.0 * u;
        let dh10 =  3.0 * u2 - 4.0 * u + 1.0;
        let dh01 = -6.0 * u2 + 6.0 * u;
        let dh11 =  3.0 * u2 - 2.0 * u;
        p0 * dh00 + m0 * dh10 + p1 * dh01 + m1 * dh11
    }

    pub fn eval_segment_second_derivative(&self, seg: usize, u: f32) -> Vec3 {
        let n = self.control_points.len();
        let i0 = seg % n;
        let i1 = (seg + 1) % n;
        let (p0, m0) = self.control_points[i0];
        let (p1, m1) = self.control_points[i1];
        let ddh00 =  12.0 * u - 6.0;
        let ddh10 =   6.0 * u - 4.0;
        let ddh01 = -12.0 * u + 6.0;
        let ddh11 =   6.0 * u - 2.0;
        p0 * ddh00 + m0 * ddh10 + p1 * ddh01 + m1 * ddh11
    }

    pub fn evaluate(&self, t: f32) -> Vec3 {
        let nseg = self.num_segments();
        if nseg == 0 { return Vec3::ZERO; }
        let t = clamp01(t);
        let scaled = t * nseg as f32;
        let seg = (scaled as usize).min(nseg - 1);
        let u   = scaled - seg as f32;
        self.eval_segment(seg, u)
    }

    pub fn evaluate_derivative(&self, t: f32) -> Vec3 {
        let nseg = self.num_segments();
        if nseg == 0 { return Vec3::ZERO; }
        let t = clamp01(t);
        let scaled = t * nseg as f32;
        let seg = (scaled as usize).min(nseg - 1);
        let u   = scaled - seg as f32;
        self.eval_segment_derivative(seg, u) * nseg as f32
    }

    pub fn evaluate_second_derivative(&self, t: f32) -> Vec3 {
        let nseg = self.num_segments();
        if nseg == 0 { return Vec3::ZERO; }
        let t = clamp01(t);
        let scaled = t * nseg as f32;
        let seg = (scaled as usize).min(nseg - 1);
        let u   = scaled - seg as f32;
        self.eval_segment_second_derivative(seg, u) * (nseg * nseg) as f32
    }

    pub fn rebuild_arc_length_table(&mut self) {
        let table = build_arc_length_table(ARC_LENGTH_SAMPLE_COUNT, &|t| self.evaluate(t));
        self.total_length = table.last().map(|e| e.1).unwrap_or(0.0);
        self.arc_length_table = table;
    }

    pub fn total_arc_length(&self) -> f32 { self.total_length }

    pub fn t_at_arc_length(&self, s: f32) -> f32 {
        arc_length_to_t(&self.arc_length_table, s)
    }

    pub fn frenet_frame_at(&self, t: f32) -> FrenetFrame {
        let pos = self.evaluate(t);
        let d1  = self.evaluate_derivative(t);
        let d2  = self.evaluate_second_derivative(t);
        let dt  = 1e-4;
        let d2a = self.evaluate_second_derivative((t + dt).min(1.0));
        let d2b = self.evaluate_second_derivative((t - dt).max(0.0));
        let d3  = (d2a - d2b) / (2.0 * dt);
        FrenetFrame::compute(pos, d1, d2, d3)
    }

    pub fn auto_tangents(&mut self) {
        let n = self.control_points.len();
        if n < 2 { return; }
        for i in 0..n {
            let prev = if i > 0 { self.control_points[i - 1].0 } else { self.control_points[0].0 };
            let next = if i + 1 < n { self.control_points[i + 1].0 } else { self.control_points[n - 1].0 };
            self.control_points[i].1 = (next - prev) * 0.5;
        }
        self.rebuild_arc_length_table();
    }
}

// ============================================================
// NEWTON'S METHOD NEAREST POINT
// ============================================================

fn newton_nearest_on_spline(
    t0: f32,
    query: Vec3,
    pos_fn: &dyn Fn(f32) -> Vec3,
    der_fn: &dyn Fn(f32) -> Vec3,
) -> f32 {
    let mut t = t0;
    for _ in 0..NEWTON_MAX_ITER {
        let p  = pos_fn(t);
        let d1 = der_fn(t);
        let err = (p - query).dot(d1);
        let denom = d1.dot(d1) + (p - query).dot(Vec3::ZERO); // simplified, omit d2 term
        if denom.abs() < EPSILON { break; }
        let delta = err / denom;
        t -= delta;
        t = clamp01(t);
        if delta.abs() < NEWTON_TOL { break; }
    }
    t
}

// ============================================================
// SPLINE-PLANE INTERSECTION
// ============================================================

pub struct SplinePlaneIntersection {
    pub t: f32,
    pub point: Vec3,
}

pub fn intersect_spline_plane(
    pos_fn: &dyn Fn(f32) -> Vec3,
    plane_normal: Vec3,
    plane_d: f32,
    steps: usize,
) -> Vec<SplinePlaneIntersection> {
    let mut results = Vec::new();
    let sdf = |t: f32| {
        let p = pos_fn(t);
        plane_normal.dot(p) - plane_d
    };
    let mut prev_val = sdf(0.0);
    for i in 1..=steps {
        let t1 = i as f32 / steps as f32;
        let val = sdf(t1);
        if prev_val * val <= 0.0 {
            let t0 = (i - 1) as f32 / steps as f32;
            // Bisect
            let mut lo = t0;
            let mut hi = t1;
            for _ in 0..32 {
                let mid = (lo + hi) * 0.5;
                let v = sdf(mid);
                if v * sdf(lo) <= 0.0 { hi = mid; } else { lo = mid; }
            }
            let t_hit = (lo + hi) * 0.5;
            results.push(SplinePlaneIntersection {
                t: t_hit,
                point: pos_fn(t_hit),
            });
        }
        prev_val = val;
    }
    results
}

// ============================================================
// SPLINE-SPLINE INTERSECTION (Newton-Raphson on distance)
// ============================================================

pub struct SplineSplineIntersection {
    pub t_a: f32,
    pub t_b: f32,
    pub point_a: Vec3,
    pub point_b: Vec3,
    pub distance: f32,
}

pub fn intersect_spline_spline(
    pos_a: &dyn Fn(f32) -> Vec3,
    pos_b: &dyn Fn(f32) -> Vec3,
    grid_steps: usize,
    tol: f32,
) -> Vec<SplineSplineIntersection> {
    let mut results = Vec::new();
    let mut checked: HashSet<(u32, u32)> = HashSet::new();
    // Coarse grid search
    for ia in 0..=grid_steps {
        for ib in 0..=grid_steps {
            let ta = ia as f32 / grid_steps as f32;
            let tb = ib as f32 / grid_steps as f32;
            let d = (pos_a(ta) - pos_b(tb)).length();
            if d < tol * 10.0 {
                // Newton-Raphson refinement on 2D residual
                let mut ta2 = ta;
                let mut tb2 = tb;
                for _ in 0..32 {
                    let pa = pos_a(ta2);
                    let pb = pos_b(tb2);
                    let diff = pa - pb;
                    let da = (pos_a(ta2 + 1e-4) - pos_a(ta2 - 1e-4)) / 2e-4;
                    let db = (pos_b(tb2 + 1e-4) - pos_b(tb2 - 1e-4)) / 2e-4;
                    // Jacobian J = [da, -db], solve J*[dta, dtb]^T = -diff
                    let j00 = da.dot(da);
                    let j01 = -da.dot(db);
                    let j10 = -db.dot(da);
                    let j11 = db.dot(db);
                    let det = j00 * j11 - j01 * j10;
                    if det.abs() < EPSILON { break; }
                    let r0 = diff.dot(da);
                    let r1 = -diff.dot(db);
                    let dta = (j11 * r0 - j01 * r1) / det;
                    let dtb = (j00 * r1 - j10 * r0) / det;
                    ta2 = (ta2 - dta).clamp(0.0, 1.0);
                    tb2 = (tb2 - dtb).clamp(0.0, 1.0);
                    if dta.abs() < tol && dtb.abs() < tol { break; }
                }
                let dist = (pos_a(ta2) - pos_b(tb2)).length();
                if dist < tol {
                    let key = ((ta2 * 1000.0) as u32, (tb2 * 1000.0) as u32);
                    if checked.insert(key) {
                        results.push(SplineSplineIntersection {
                            t_a: ta2, t_b: tb2,
                            point_a: pos_a(ta2), point_b: pos_b(tb2),
                            distance: dist,
                        });
                    }
                }
            }
        }
    }
    results
}

// ============================================================
// RAIL SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub struct RailTrack {
    pub id: u64,
    pub spline: CatmullRomSpline,
    pub gauge: f32,              // distance between rails in meters
    pub max_speed: f32,          // km/h
    pub super_elevation_max: f32, // radians, typically ~0.15 rad
    pub cant_deficiency: f32,    // excess cant in mm
    pub name: String,
}

impl RailTrack {
    pub fn new(spline: CatmullRomSpline, gauge: f32) -> Self {
        RailTrack {
            id: rand_id(),
            spline,
            gauge,
            max_speed: 120.0,
            super_elevation_max: 0.15,
            cant_deficiency: 75.0,
            name: String::from("Track"),
        }
    }

    /// Banking angle from curvature: θ = atan(v²κ/g) where κ = curvature
    pub fn banking_angle_at(&self, t: f32, speed_ms: f32) -> f32 {
        let kappa = self.spline.curvature_at(t);
        let g = 9.81_f32;
        let centripetal = speed_ms * speed_ms * kappa;
        (centripetal / g).atan()
    }

    /// Superelevation (cant) in mm: e = v²·g/(R·g) * gauge
    /// where R = 1/κ
    pub fn superelevation_at(&self, t: f32, speed_ms: f32) -> f32 {
        let kappa = self.spline.curvature_at(t);
        if kappa < EPSILON { return 0.0; }
        let r = 1.0 / kappa;
        let g = 9.81_f32;
        let cant = (speed_ms * speed_ms / (r * g)) * self.gauge * 1000.0; // in mm
        cant.min(self.super_elevation_max * 1000.0)
    }

    /// Left and right rail positions at parameter t
    pub fn rail_positions(&self, t: f32, speed_ms: f32) -> (Vec3, Vec3) {
        let frame = self.spline.frenet_frame_at(t);
        let bank = self.banking_angle_at(t, speed_ms);
        let half_gauge = self.gauge * 0.5;
        let bank_rot = Quat::from_axis_angle(frame.tangent, bank);
        let lateral = bank_rot * frame.normal;
        let left  = frame.position + lateral * half_gauge;
        let right = frame.position - lateral * half_gauge;
        (left, right)
    }

    pub fn rail_mesh_data(&self, resolution: usize, speed_ms: f32) -> RailMeshData {
        let mut left_pts  = Vec::with_capacity(resolution + 1);
        let mut right_pts = Vec::with_capacity(resolution + 1);
        for i in 0..=resolution {
            let t = i as f32 / resolution as f32;
            let (l, r) = self.rail_positions(t, speed_ms);
            left_pts.push(l);
            right_pts.push(r);
        }
        RailMeshData { left_rail: left_pts, right_rail: right_pts, sleepers: Vec::new() }
    }

    pub fn add_sleepers(&self, rail_data: &mut RailMeshData, spacing: f32) {
        let total = self.spline.total_arc_length();
        let mut s = 0.0_f32;
        while s < total {
            let t = self.spline.t_at_arc_length(s);
            let (l, r) = self.rail_positions(t, 0.0);
            rail_data.sleepers.push(Sleeper { left: l, right: r, t });
            s += spacing;
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sleeper {
    pub left: Vec3,
    pub right: Vec3,
    pub t: f32,
}

#[derive(Clone, Debug)]
pub struct RailMeshData {
    pub left_rail:  Vec<Vec3>,
    pub right_rail: Vec<Vec3>,
    pub sleepers:   Vec<Sleeper>,
}

// ============================================================
// CAMERA RAIL
// ============================================================

#[derive(Clone, Debug)]
pub struct SpeedProfile {
    pub keyframes: Vec<(f32, f32)>, // (t, speed) pairs
}

impl SpeedProfile {
    pub fn constant(speed: f32) -> Self {
        SpeedProfile { keyframes: vec![(0.0, speed), (1.0, speed)] }
    }

    pub fn ease_in_out(start_speed: f32, cruise_speed: f32, end_speed: f32) -> Self {
        SpeedProfile {
            keyframes: vec![
                (0.0, start_speed),
                (0.2, cruise_speed),
                (0.8, cruise_speed),
                (1.0, end_speed),
            ]
        }
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keyframes.is_empty() { return 0.0; }
        if self.keyframes.len() == 1 { return self.keyframes[0].1; }
        let t = clamp01(t);
        let idx = self.keyframes.partition_point(|kf| kf.0 <= t);
        if idx == 0 { return self.keyframes[0].1; }
        if idx >= self.keyframes.len() { return self.keyframes.last().unwrap().1; }
        let (t0, v0) = self.keyframes[idx - 1];
        let (t1, v1) = self.keyframes[idx];
        let frac = if (t1 - t0).abs() < EPSILON { 0.0 } else { (t - t0) / (t1 - t0) };
        lerp(v0, v1, quintic_ease(frac))
    }

    /// Compute the arc-length parameter corresponding to time elapsed
    pub fn time_to_t(&self, total_length: f32, time: f32, dt: f32) -> f32 {
        let mut t = 0.0_f32;
        let mut elapsed = 0.0_f32;
        while elapsed < time && t < 1.0 {
            let speed = self.evaluate(t);
            let ds = speed * dt;
            // advance arc-length
            elapsed += dt;
            t += ds / total_length.max(EPSILON);
            t = t.min(1.0);
        }
        t
    }
}

#[derive(Clone, Debug)]
pub struct CameraRail {
    pub spline: CatmullRomSpline,
    pub speed_profile: SpeedProfile,
    pub look_ahead_distance: f32,  // meters ahead to look at
    pub roll_correction: bool,
    pub fov_profile: SpeedProfile, // repurpose SpeedProfile for FOV over t
    pub up_axis: Vec3,
}

impl CameraRail {
    pub fn new(spline: CatmullRomSpline) -> Self {
        CameraRail {
            spline,
            speed_profile: SpeedProfile::ease_in_out(0.0, 10.0, 0.0),
            look_ahead_distance: 5.0,
            roll_correction: true,
            fov_profile: SpeedProfile::constant(60.0),
            up_axis: Vec3::Y,
        }
    }

    pub fn camera_transform_at(&self, t: f32) -> Mat4 {
        let pos = self.spline.evaluate(t);
        let total = self.spline.total_arc_length();
        let s_current = t * total;
        let s_ahead = (s_current + self.look_ahead_distance).min(total);
        let t_ahead = self.spline.t_at_arc_length(s_ahead);
        let target = self.spline.evaluate(t_ahead);
        let forward = safe_normalize(target - pos);
        let right   = safe_normalize(forward.cross(self.up_axis));
        let up      = if self.roll_correction {
            safe_normalize(right.cross(forward))
        } else {
            self.up_axis
        };
        Mat4::look_at_rh(pos, target, up).inverse()
    }

    pub fn fov_at(&self, t: f32) -> f32 {
        self.fov_profile.evaluate(t)
    }

    /// Compute camera path as sequence of (transform, fov) pairs
    pub fn bake_camera_path(&self, steps: usize) -> Vec<(Mat4, f32)> {
        (0..=steps).map(|i| {
            let t = i as f32 / steps as f32;
            (self.camera_transform_at(t), self.fov_at(t))
        }).collect()
    }
}

// ============================================================
// SPLINE MESH GENERATION (sweep cross-section along spline)
// ============================================================

#[derive(Clone, Debug)]
pub struct CrossSection {
    /// 2D points in local space (Y up, X right)
    pub points: Vec<Vec2>,
    pub closed: bool,
}

impl CrossSection {
    pub fn circle(radius: f32, segments: usize) -> Self {
        let pts = (0..segments).map(|i| {
            let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
            Vec2::new(angle.cos() * radius, angle.sin() * radius)
        }).collect();
        CrossSection { points: pts, closed: true }
    }

    pub fn rectangle(width: f32, height: f32) -> Self {
        let hw = width * 0.5;
        let hh = height * 0.5;
        CrossSection {
            points: vec![
                Vec2::new(-hw, -hh),
                Vec2::new( hw, -hh),
                Vec2::new( hw,  hh),
                Vec2::new(-hw,  hh),
            ],
            closed: true,
        }
    }

    pub fn i_beam(width: f32, height: f32, flange: f32, web: f32) -> Self {
        let hw = width * 0.5;
        let hh = height * 0.5;
        let hw_web = web * 0.5;
        CrossSection {
            points: vec![
                Vec2::new(-hw, -hh),
                Vec2::new( hw, -hh),
                Vec2::new( hw, -hh + flange),
                Vec2::new( hw_web, -hh + flange),
                Vec2::new( hw_web,  hh - flange),
                Vec2::new( hw,  hh - flange),
                Vec2::new( hw,  hh),
                Vec2::new(-hw,  hh),
                Vec2::new(-hw,  hh - flange),
                Vec2::new(-hw_web,  hh - flange),
                Vec2::new(-hw_web, -hh + flange),
                Vec2::new(-hw,  -hh + flange),
            ],
            closed: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SplineMesh {
    pub vertices:  Vec<Vec3>,
    pub normals:   Vec<Vec3>,
    pub uvs:       Vec<Vec2>,
    pub indices:   Vec<u32>,
    pub tangents:  Vec<Vec3>,
}

impl SplineMesh {
    pub fn new() -> Self {
        SplineMesh {
            vertices: Vec::new(),
            normals:  Vec::new(),
            uvs:      Vec::new(),
            indices:  Vec::new(),
            tangents: Vec::new(),
        }
    }

    pub fn vertex_count(&self) -> usize { self.vertices.len() }
    pub fn triangle_count(&self) -> usize { self.indices.len() / 3 }

    pub fn generate_from_spline(
        spline_pos: &dyn Fn(f32) -> Vec3,
        spline_tangent: &dyn Fn(f32) -> Vec3,
        section: &CrossSection,
        spline_steps: usize,
        total_arc_length: f32,
    ) -> SplineMesh {
        let mut mesh = SplineMesh::new();
        let n_section = section.points.len();
        if n_section == 0 || spline_steps == 0 { return mesh; }

        // Build parallel transport frames along the spline
        let mut frames: Vec<ParallelTransportFrame> = Vec::with_capacity(spline_steps + 1);
        {
            let p0 = spline_pos(0.0);
            let t0 = spline_tangent(0.0);
            frames.push(ParallelTransportFrame::initial(p0, t0));
        }
        for i in 1..=spline_steps {
            let t = i as f32 / spline_steps as f32;
            let p = spline_pos(t);
            let tang = safe_normalize(spline_tangent(t));
            let prev = frames.last().unwrap().clone();
            frames.push(ParallelTransportFrame::transport(&prev, p, tang));
        }

        // Build vertex rings
        let mut arc_s = 0.0_f32;
        let mut prev_pos = spline_pos(0.0);
        for (ring_idx, frame) in frames.iter().enumerate() {
            let t = ring_idx as f32 / spline_steps as f32;
            if ring_idx > 0 {
                let cur_pos = spline_pos(t);
                arc_s += (cur_pos - prev_pos).length();
                prev_pos = cur_pos;
            }
            let u_coord = arc_s / total_arc_length.max(EPSILON);
            for (j, &sec_pt) in section.points.iter().enumerate() {
                let v_coord = j as f32 / n_section as f32;
                let world = frame.position
                    + frame.normal   * sec_pt.x
                    + frame.binormal * sec_pt.y;
                let normal_2d = sec_pt.normalize_or_zero();
                let world_normal = safe_normalize(
                    frame.normal   * normal_2d.x +
                    frame.binormal * normal_2d.y
                );
                mesh.vertices.push(world);
                mesh.normals.push(world_normal);
                mesh.uvs.push(Vec2::new(u_coord, v_coord));
                mesh.tangents.push(frame.tangent);
            }
        }

        // Build indices
        let rings = spline_steps + 1;
        for r in 0..rings - 1 {
            for j in 0..n_section {
                let j_next = (j + 1) % n_section;
                let a = (r * n_section + j) as u32;
                let b = (r * n_section + j_next) as u32;
                let c = ((r + 1) * n_section + j) as u32;
                let d = ((r + 1) * n_section + j_next) as u32;
                mesh.indices.push(a);
                mesh.indices.push(b);
                mesh.indices.push(c);
                mesh.indices.push(b);
                mesh.indices.push(d);
                mesh.indices.push(c);
            }
        }

        mesh
    }

    /// LOD by curvature: denser sampling where curvature is high
    pub fn generate_lod(
        spline_pos: &dyn Fn(f32) -> Vec3,
        spline_tangent: &dyn Fn(f32) -> Vec3,
        spline_curvature: &dyn Fn(f32) -> f32,
        section: &CrossSection,
        min_steps: usize,
        max_steps: usize,
        total_arc_length: f32,
    ) -> SplineMesh {
        // Build adaptive t samples based on curvature
        let mut t_samples = vec![0.0_f32];
        let coarse = min_steps * 4;
        for i in 1..coarse {
            let t = i as f32 / coarse as f32;
            let kappa = spline_curvature(t);
            let step_factor = (1.0 + kappa * 10.0).recip();
            let prev = *t_samples.last().unwrap();
            let step = (1.0 / min_steps as f32) * step_factor.max(1.0 / max_steps as f32);
            if t - prev >= step { t_samples.push(t); }
        }
        t_samples.push(1.0);
        let spline_steps = t_samples.len() - 1;

        let mut mesh = SplineMesh::new();
        let n_section = section.points.len();
        if n_section == 0 { return mesh; }

        let mut frames: Vec<ParallelTransportFrame> = Vec::new();
        {
            let p0 = spline_pos(0.0);
            let t0 = spline_tangent(0.0);
            frames.push(ParallelTransportFrame::initial(p0, t0));
        }
        for i in 1..t_samples.len() {
            let t = t_samples[i];
            let p = spline_pos(t);
            let tang = safe_normalize(spline_tangent(t));
            let prev = frames.last().unwrap().clone();
            frames.push(ParallelTransportFrame::transport(&prev, p, tang));
        }

        let mut arc_s = 0.0_f32;
        let mut prev_pos = spline_pos(0.0);
        for (ring_idx, frame) in frames.iter().enumerate() {
            let t = t_samples[ring_idx];
            if ring_idx > 0 {
                let cur_pos = spline_pos(t);
                arc_s += (cur_pos - prev_pos).length();
                prev_pos = cur_pos;
            }
            let u_coord = arc_s / total_arc_length.max(EPSILON);
            for (j, &sec_pt) in section.points.iter().enumerate() {
                let v_coord = j as f32 / n_section as f32;
                let world = frame.position
                    + frame.normal   * sec_pt.x
                    + frame.binormal * sec_pt.y;
                let normal_2d = sec_pt.normalize_or_zero();
                let world_normal = safe_normalize(
                    frame.normal   * normal_2d.x +
                    frame.binormal * normal_2d.y
                );
                mesh.vertices.push(world);
                mesh.normals.push(world_normal);
                mesh.uvs.push(Vec2::new(u_coord, v_coord));
                mesh.tangents.push(frame.tangent);
            }
        }

        let rings = frames.len();
        for r in 0..rings.saturating_sub(1) {
            for j in 0..n_section {
                let j_next = (j + 1) % n_section;
                let a = (r * n_section + j) as u32;
                let b = (r * n_section + j_next) as u32;
                let c = ((r + 1) * n_section + j) as u32;
                let d = ((r + 1) * n_section + j_next) as u32;
                mesh.indices.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }

        mesh
    }
}

// ============================================================
// PATH NETWORK (Directed Graph + Dijkstra)
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineNode {
    pub id: u64,
    pub position: Vec3,
    pub connected_splines: Vec<u64>, // spline IDs
}

#[derive(Clone, Debug)]
pub struct SplineEdge {
    pub id: u64,
    pub from_node: u64,
    pub to_node: u64,
    pub spline_id: u64,
    pub weight: f32, // arc length or custom cost
    pub one_way: bool,
}

#[derive(Clone, Debug)]
pub struct PathNetwork {
    pub nodes: HashMap<u64, SplineNode>,
    pub edges: HashMap<u64, SplineEdge>,
    pub splines: HashMap<u64, CatmullRomSpline>,
    // adjacency list: node_id -> [(edge_id, neighbor_node_id)]
    adjacency: HashMap<u64, Vec<(u64, u64)>>,
}

impl PathNetwork {
    pub fn new() -> Self {
        PathNetwork {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            splines: HashMap::new(),
            adjacency: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, position: Vec3) -> u64 {
        let id = rand_id();
        self.nodes.insert(id, SplineNode {
            id, position, connected_splines: Vec::new(),
        });
        self.adjacency.insert(id, Vec::new());
        id
    }

    pub fn add_spline(&mut self, spline: CatmullRomSpline) -> u64 {
        let id = rand_id();
        self.splines.insert(id, spline);
        id
    }

    pub fn connect_nodes(&mut self, from: u64, to: u64, spline_id: u64, one_way: bool) {
        let weight = self.splines.get(&spline_id)
            .map(|s| s.total_arc_length())
            .unwrap_or(1.0);
        let edge_id = rand_id();
        let edge = SplineEdge { id: edge_id, from_node: from, to_node: to, spline_id, weight, one_way };
        self.edges.insert(edge_id, edge.clone());
        self.adjacency.entry(from).or_default().push((edge_id, to));
        if !one_way {
            let rev_edge_id = rand_id();
            let rev_edge = SplineEdge { id: rev_edge_id, from_node: to, to_node: from, spline_id, weight, one_way: false };
            self.edges.insert(rev_edge_id, rev_edge);
            self.adjacency.entry(to).or_default().push((rev_edge_id, from));
        }
    }

    /// Dijkstra shortest path from start to goal
    pub fn dijkstra(&self, start: u64, goal: u64) -> Option<Vec<u64>> {
        use std::collections::BinaryHeap;
        use std::cmp::Reverse;

        // dist: node_id -> (cost, prev_node_id)
        let mut dist: HashMap<u64, f32> = HashMap::new();
        let mut prev: HashMap<u64, u64> = HashMap::new();
        let mut heap: BinaryHeap<Reverse<(u32, u64)>> = BinaryHeap::new();

        dist.insert(start, 0.0);
        heap.push(Reverse((0, start)));

        while let Some(Reverse((cost_bits, node))) = heap.pop() {
            let cost = f32::from_bits(cost_bits);
            if node == goal {
                // Reconstruct path
                let mut path = vec![goal];
                let mut cur = goal;
                while let Some(&p) = prev.get(&cur) {
                    path.push(p);
                    cur = p;
                    if cur == start { break; }
                }
                path.reverse();
                return Some(path);
            }
            let best = dist.get(&node).cloned().unwrap_or(f32::MAX);
            if cost > best + EPSILON { continue; }
            if let Some(neighbors) = self.adjacency.get(&node) {
                for &(edge_id, neighbor) in neighbors {
                    if let Some(edge) = self.edges.get(&edge_id) {
                        let new_cost = cost + edge.weight;
                        let cur_best = dist.get(&neighbor).cloned().unwrap_or(f32::MAX);
                        if new_cost < cur_best {
                            dist.insert(neighbor, new_cost);
                            prev.insert(neighbor, node);
                            heap.push(Reverse((new_cost.to_bits(), neighbor)));
                        }
                    }
                }
            }
        }
        None
    }

    /// A* path planning with Euclidean heuristic
    pub fn astar(&self, start: u64, goal: u64) -> Option<Vec<u64>> {
        use std::collections::BinaryHeap;
        use std::cmp::Reverse;

        let goal_pos = self.nodes.get(&goal)?.position;
        let heuristic = |node_id: u64| -> f32 {
            self.nodes.get(&node_id)
                .map(|n| (n.position - goal_pos).length())
                .unwrap_or(0.0)
        };

        let mut g_score: HashMap<u64, f32> = HashMap::new();
        let mut prev: HashMap<u64, u64> = HashMap::new();
        let mut open: BinaryHeap<Reverse<(u32, u64)>> = BinaryHeap::new();

        g_score.insert(start, 0.0);
        let f0 = heuristic(start);
        open.push(Reverse((f0.to_bits(), start)));

        while let Some(Reverse((_, node))) = open.pop() {
            if node == goal {
                let mut path = vec![goal];
                let mut cur = goal;
                while let Some(&p) = prev.get(&cur) {
                    path.push(p);
                    cur = p;
                    if cur == start { break; }
                }
                path.reverse();
                return Some(path);
            }
            let g = g_score.get(&node).cloned().unwrap_or(f32::MAX);
            if let Some(neighbors) = self.adjacency.get(&node) {
                for &(edge_id, neighbor) in neighbors {
                    if let Some(edge) = self.edges.get(&edge_id) {
                        let new_g = g + edge.weight;
                        let cur_g = g_score.get(&neighbor).cloned().unwrap_or(f32::MAX);
                        if new_g < cur_g {
                            g_score.insert(neighbor, new_g);
                            prev.insert(neighbor, node);
                            let f = new_g + heuristic(neighbor);
                            open.push(Reverse((f.to_bits(), neighbor)));
                        }
                    }
                }
            }
        }
        None
    }

    pub fn nearest_node(&self, pos: Vec3) -> Option<u64> {
        self.nodes.values()
            .min_by(|a, b| {
                let da = (a.position - pos).length_squared();
                let db = (b.position - pos).length_squared();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| n.id)
    }
}

// ============================================================
// TRAFFIC SYSTEM PARAMETERS
// ============================================================

#[derive(Clone, Debug)]
pub struct TrafficAgent {
    pub id: u64,
    pub current_spline_id: u64,
    pub t: f32,
    pub speed: f32,
    pub max_speed: f32,
    pub path: Vec<u64>, // sequence of node IDs
    pub path_index: usize,
    pub braking_distance: f32,
    pub acceleration: f32,
}

impl TrafficAgent {
    pub fn new(spline_id: u64, max_speed: f32) -> Self {
        TrafficAgent {
            id: rand_id(),
            current_spline_id: spline_id,
            t: 0.0,
            speed: 0.0,
            max_speed,
            path: Vec::new(),
            path_index: 0,
            braking_distance: 20.0,
            acceleration: 2.0,
        }
    }

    pub fn update(&mut self, dt: f32, spline: &CatmullRomSpline) {
        // Simple kinematic update
        let target_speed = self.max_speed;
        if self.speed < target_speed {
            self.speed = (self.speed + self.acceleration * dt).min(target_speed);
        }
        let total_length = spline.total_arc_length();
        if total_length < EPSILON { return; }
        let ds = self.speed * dt;
        let current_s = self.t * total_length;
        let new_s = (current_s + ds).min(total_length);
        self.t = new_s / total_length;
    }

    pub fn position(&self, spline: &CatmullRomSpline) -> Vec3 {
        spline.evaluate(self.t)
    }
}

#[derive(Clone, Debug)]
pub struct TrafficSystem {
    pub agents: Vec<TrafficAgent>,
    pub network: PathNetwork,
    pub spawn_rate: f32,
    pub max_agents: usize,
}

impl TrafficSystem {
    pub fn new(network: PathNetwork) -> Self {
        TrafficSystem {
            agents: Vec::new(),
            network,
            spawn_rate: 0.1,
            max_agents: 64,
        }
    }

    pub fn spawn_agent(&mut self, spline_id: u64) {
        if self.agents.len() >= self.max_agents { return; }
        let agent = TrafficAgent::new(spline_id, 10.0 + (self.agents.len() as f32 % 5.0) * 2.0);
        self.agents.push(agent);
    }

    pub fn update(&mut self, dt: f32) {
        for agent in &mut self.agents {
            if let Some(spline) = self.network.splines.get(&agent.current_spline_id) {
                // Clone to avoid borrow conflict
                let spline_clone = spline.clone();
                agent.update(dt, &spline_clone);
            }
        }
        // Remove agents that have reached end
        self.agents.retain(|a| a.t < 1.0);
    }

    pub fn agent_separation_force(&self, agent_idx: usize) -> Vec3 {
        let agent = &self.agents[agent_idx];
        let spline = match self.network.splines.get(&agent.current_spline_id) {
            Some(s) => s,
            None => return Vec3::ZERO,
        };
        let my_pos = spline.evaluate(agent.t);
        let mut force = Vec3::ZERO;
        for (i, other) in self.agents.iter().enumerate() {
            if i == agent_idx { continue; }
            if other.current_spline_id != agent.current_spline_id { continue; }
            let other_pos = spline.evaluate(other.t);
            let diff = my_pos - other_pos;
            let dist = diff.length();
            if dist < 5.0 && dist > EPSILON {
                force += diff / (dist * dist);
            }
        }
        force
    }
}

// ============================================================
// SPLINE PHYSICS
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineConstrainedObject {
    pub id: u64,
    pub spline_id: u64,
    pub t: f32,
    pub speed: f32,            // m/s along spline
    pub mass: f32,
    pub gravity: Vec3,
    pub friction: f32,         // coefficient
    pub normal_force: f32,     // computed per frame
}

impl SplineConstrainedObject {
    pub fn new(spline_id: u64, t: f32, mass: f32) -> Self {
        SplineConstrainedObject {
            id: rand_id(),
            spline_id,
            t,
            speed: 0.0,
            mass,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            friction: 0.1,
            normal_force: 0.0,
        }
    }

    pub fn update(&mut self, dt: f32, spline: &CatmullRomSpline) {
        let frame = spline.frenet_frame_at(self.t);
        // Gravitational component along tangent
        let g_tangent = self.gravity.dot(frame.tangent);
        // Normal force = m * (g_normal + centripetal)
        let g_normal  = self.gravity.dot(frame.normal);
        let centripetal = self.speed * self.speed * frame.curvature;
        self.normal_force = self.mass * (g_normal + centripetal).abs();
        // Friction force (opposes motion)
        let friction_force = -self.speed.signum() * self.friction * self.normal_force;
        // Net tangential force
        let net_tangential = self.mass * g_tangent + friction_force;
        let tangential_accel = net_tangential / self.mass;
        self.speed += tangential_accel * dt;
        // Advance along spline
        let total_length = spline.total_arc_length();
        if total_length > EPSILON {
            let ds = self.speed * dt;
            let current_s = self.t * total_length;
            let new_s = (current_s + ds).clamp(0.0, total_length);
            self.t = new_s / total_length;
        }
    }

    pub fn position(&self, spline: &CatmullRomSpline) -> Vec3 {
        spline.evaluate(self.t)
    }

    pub fn centripetal_acceleration(&self, spline: &CatmullRomSpline) -> Vec3 {
        let frame = spline.frenet_frame_at(self.t);
        frame.normal * (self.speed * self.speed * frame.curvature)
    }
}

// ============================================================
// CHAIN LINKS ALONG SPLINE
// ============================================================

#[derive(Clone, Debug)]
pub struct ChainLink {
    pub t: f32,
    pub angle_twist: f32, // twist around tangent
    pub size: f32,
}

#[derive(Clone, Debug)]
pub struct SplineChain {
    pub spline_id: u64,
    pub links: Vec<ChainLink>,
    pub link_length: f32,
    pub link_width: f32,
    pub link_height: f32,
    pub offset: f32, // animation offset in [0,1]
}

impl SplineChain {
    pub fn new(spline: &CatmullRomSpline, spline_id: u64, link_length: f32) -> Self {
        let total = spline.total_arc_length();
        let n_links = (total / link_length.max(EPSILON)) as usize;
        let links = (0..n_links).map(|i| {
            let s = i as f32 * link_length;
            let t = spline.t_at_arc_length(s);
            ChainLink {
                t,
                angle_twist: if i % 2 == 0 { 0.0 } else { std::f32::consts::FRAC_PI_2 },
                size: link_length,
            }
        }).collect();
        SplineChain {
            spline_id,
            links,
            link_length,
            link_width:  link_length * 0.6,
            link_height: link_length * 0.15,
            offset: 0.0,
        }
    }

    pub fn update_offset(&mut self, delta: f32) {
        self.offset = (self.offset + delta).fract();
    }

    pub fn link_transform(&self, link_idx: usize, spline: &CatmullRomSpline) -> Mat4 {
        let link = &self.links[link_idx];
        let frame = spline.frenet_frame_at(link.t);
        let twist = Quat::from_axis_angle(frame.tangent, link.angle_twist);
        let normal   = twist * frame.normal;
        let binormal = twist * frame.binormal;
        Mat4::from_cols(
            Vec4::new(frame.tangent.x, frame.tangent.y, frame.tangent.z, 0.0),
            Vec4::new(normal.x, normal.y, normal.z, 0.0),
            Vec4::new(binormal.x, binormal.y, binormal.z, 0.0),
            Vec4::new(frame.position.x, frame.position.y, frame.position.z, 1.0),
        )
    }
}

// ============================================================
// DEBUG VISUALIZATION DATA
// ============================================================

#[derive(Clone, Debug)]
pub struct DebugLine {
    pub start: Vec3,
    pub end:   Vec3,
    pub color: Vec4,
}

#[derive(Clone, Debug)]
pub struct DebugPoint {
    pub position: Vec3,
    pub color:    Vec4,
    pub size:     f32,
}

#[derive(Clone, Debug)]
pub struct SplineDebugViz {
    pub lines:  Vec<DebugLine>,
    pub points: Vec<DebugPoint>,
    pub curvature_comb: Vec<(Vec3, Vec3)>, // base, tip
}

impl SplineDebugViz {
    pub fn new() -> Self {
        SplineDebugViz {
            lines:  Vec::new(),
            points: Vec::new(),
            curvature_comb: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.points.clear();
        self.curvature_comb.clear();
    }

    pub fn draw_frenet_frames(
        &mut self,
        pos_fn:  &dyn Fn(f32) -> Vec3,
        d1_fn:   &dyn Fn(f32) -> Vec3,
        d2_fn:   &dyn Fn(f32) -> Vec3,
        d3_fn:   &dyn Fn(f32) -> Vec3,
        steps:   usize,
        scale:   f32,
    ) {
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let pos = pos_fn(t);
            let d1  = d1_fn(t);
            let d2  = d2_fn(t);
            let d3  = d3_fn(t);
            let frame = FrenetFrame::compute(pos, d1, d2, d3);
            self.lines.push(DebugLine {
                start: pos,
                end:   pos + frame.tangent  * scale,
                color: Vec4::new(1.0, 0.0, 0.0, 1.0), // red = tangent
            });
            self.lines.push(DebugLine {
                start: pos,
                end:   pos + frame.normal   * scale,
                color: Vec4::new(0.0, 1.0, 0.0, 1.0), // green = normal
            });
            self.lines.push(DebugLine {
                start: pos,
                end:   pos + frame.binormal * scale,
                color: Vec4::new(0.0, 0.0, 1.0, 1.0), // blue = binormal
            });
        }
    }

    pub fn draw_curvature_comb(
        &mut self,
        pos_fn:      &dyn Fn(f32) -> Vec3,
        curvature_fn: &dyn Fn(f32) -> f32,
        normal_fn:   &dyn Fn(f32) -> Vec3,
        steps:       usize,
        scale:       f32,
    ) {
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let base = pos_fn(t);
            let kappa = curvature_fn(t);
            let normal = normal_fn(t);
            let tip = base + normal * kappa * scale;
            self.curvature_comb.push((base, tip));
            self.lines.push(DebugLine {
                start: base,
                end:   tip,
                color: Vec4::new(1.0, 1.0, 0.0, 0.8),
            });
        }
    }

    pub fn draw_arc_length_marks(
        &mut self,
        pos_fn:        &dyn Fn(f32) -> Vec3,
        t_at_length_fn: &dyn Fn(f32) -> f32,
        total_length:  f32,
        interval:      f32,
        up:            Vec3,
        size:          f32,
    ) {
        let mut s = 0.0_f32;
        while s <= total_length {
            let t = t_at_length_fn(s);
            let pos = pos_fn(t);
            self.points.push(DebugPoint {
                position: pos,
                color:    Vec4::new(1.0, 0.5, 0.0, 1.0),
                size,
            });
            self.lines.push(DebugLine {
                start: pos - up * size,
                end:   pos + up * size,
                color: Vec4::new(1.0, 0.5, 0.0, 1.0),
            });
            s += interval;
        }
    }

    pub fn draw_bounding_box(&mut self, min: Vec3, max: Vec3, color: Vec4) {
        let corners = [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ];
        let edges = [
            (0,1),(1,2),(2,3),(3,0), // bottom
            (4,5),(5,6),(6,7),(7,4), // top
            (0,4),(1,5),(2,6),(3,7), // sides
        ];
        for (a, b) in edges {
            self.lines.push(DebugLine { start: corners[a], end: corners[b], color });
        }
    }

    pub fn draw_spline_curve(
        &mut self,
        pos_fn: &dyn Fn(f32) -> Vec3,
        steps: usize,
        color: Vec4,
    ) {
        let mut prev = pos_fn(0.0);
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let cur = pos_fn(t);
            self.lines.push(DebugLine { start: prev, end: cur, color });
            prev = cur;
        }
    }

    pub fn draw_control_polygon(&mut self, points: &[Vec3], color: Vec4) {
        for i in 0..points.len().saturating_sub(1) {
            self.lines.push(DebugLine {
                start: points[i],
                end:   points[i + 1],
                color,
            });
        }
        for &p in points {
            self.points.push(DebugPoint {
                position: p,
                color,
                size: 6.0,
            });
        }
    }
}

// ============================================================
// UNDO/REDO SYSTEM
// ============================================================

#[derive(Clone, Debug)]
pub enum SplineEditorCommand {
    AddControlPoint  { spline_id: u64, index: usize, point: ControlPoint },
    RemoveControlPoint { spline_id: u64, index: usize, point: ControlPoint },
    MoveControlPoint { spline_id: u64, index: usize, old_pos: Vec3, new_pos: Vec3 },
    MoveTangent      { spline_id: u64, index: usize, which: TangentHandle, old_val: Vec3, new_val: Vec3 },
    InsertKnot       { spline_id: u64, t: f32 },
    SplitSpline      { spline_id: u64, t: f32 },
    JoinSplines      { spline_a: u64, spline_b: u64 },
    ToggleClosed     { spline_id: u64 },
    AddSpline        { spline_id: u64 },
    RemoveSpline     { spline_id: u64 },
    SetSplineType    { spline_id: u64, old_type: SplineType, new_type: SplineType },
}

#[derive(Clone, Debug, PartialEq)]
pub enum TangentHandle {
    In,
    Out,
}

#[derive(Debug)]
pub struct UndoHistory {
    past:   VecDeque<SplineEditorCommand>,
    future: VecDeque<SplineEditorCommand>,
    max_size: usize,
}

impl UndoHistory {
    pub fn new(max_size: usize) -> Self {
        UndoHistory { past: VecDeque::new(), future: VecDeque::new(), max_size }
    }

    pub fn push(&mut self, cmd: SplineEditorCommand) {
        self.future.clear();
        self.past.push_back(cmd);
        if self.past.len() > self.max_size {
            self.past.pop_front();
        }
    }

    pub fn can_undo(&self) -> bool { !self.past.is_empty() }
    pub fn can_redo(&self) -> bool { !self.future.is_empty() }

    pub fn undo(&mut self) -> Option<SplineEditorCommand> {
        let cmd = self.past.pop_back()?;
        self.future.push_back(cmd.clone());
        Some(cmd)
    }

    pub fn redo(&mut self) -> Option<SplineEditorCommand> {
        let cmd = self.future.pop_back()?;
        self.past.push_back(cmd.clone());
        Some(cmd)
    }
}

// ============================================================
// SELECTION STATE
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum SelectionTarget {
    SplineId(u64),
    ControlPointIndex(u64, usize), // (spline_id, cp_index)
    TangentIn(u64, usize),
    TangentOut(u64, usize),
    NodeId(u64),
    EdgeId(u64),
}

#[derive(Clone, Debug)]
pub struct SelectionState {
    pub selected: HashSet<u64>,        // spline IDs
    pub selected_cp: Vec<(u64, usize)>, // (spline_id, cp_index)
    pub hovered: Option<SelectionTarget>,
    pub active: Option<SelectionTarget>,
}

impl SelectionState {
    pub fn new() -> Self {
        SelectionState {
            selected: HashSet::new(),
            selected_cp: Vec::new(),
            hovered: None,
            active: None,
        }
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.selected_cp.clear();
        self.hovered = None;
        self.active  = None;
    }

    pub fn select_spline(&mut self, id: u64, add: bool) {
        if !add { self.selected.clear(); }
        self.selected.insert(id);
    }

    pub fn select_cp(&mut self, spline_id: u64, index: usize, add: bool) {
        if !add { self.selected_cp.clear(); }
        self.selected_cp.push((spline_id, index));
    }

    pub fn deselect_cp(&mut self, spline_id: u64, index: usize) {
        self.selected_cp.retain(|&(sid, ci)| !(sid == spline_id && ci == index));
    }

    pub fn is_cp_selected(&self, spline_id: u64, index: usize) -> bool {
        self.selected_cp.iter().any(|&(sid, ci)| sid == spline_id && ci == index)
    }
}

// ============================================================
// SERIALIZATION (simple text-based)
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineSerializedData {
    pub spline_id: u64,
    pub spline_type: SplineType,
    pub control_points: Vec<(Vec3, Vec3, Vec3, f32)>, // pos, t_in, t_out, weight
    pub closed: bool,
    pub name: String,
    pub metadata: HashMap<String, String>,
}

impl SplineSerializedData {
    pub fn serialize_catmull_rom(spline: &CatmullRomSpline, id: u64, name: &str) -> Self {
        SplineSerializedData {
            spline_id: id,
            spline_type: SplineType::CatmullRom,
            control_points: spline.control_points.iter().map(|cp| {
                (cp.position, cp.tangent_in, cp.tangent_out, cp.weight)
            }).collect(),
            closed: spline.closed,
            name: name.to_string(),
            metadata: HashMap::new(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        // Simple binary serialization: just f32 values packed
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.spline_id.to_le_bytes());
        bytes.extend_from_slice(&(self.control_points.len() as u32).to_le_bytes());
        for (pos, t_in, t_out, w) in &self.control_points {
            for &v in &[pos.x, pos.y, pos.z, t_in.x, t_in.y, t_in.z,
                        t_out.x, t_out.y, t_out.z, *w] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        bytes.push(if self.closed { 1 } else { 0 });
        bytes
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 12 { return None; }
        let mut cursor = 0usize;
        let spline_id = u64::from_le_bytes(data[cursor..cursor+8].try_into().ok()?);
        cursor += 8;
        let n = u32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?) as usize;
        cursor += 4;
        let floats_per_cp = 10usize;
        let mut control_points = Vec::with_capacity(n);
        for _ in 0..n {
            if cursor + floats_per_cp * 4 > data.len() { return None; }
            let mut vals = [0.0_f32; 10];
            for v in &mut vals {
                *v = f32::from_le_bytes(data[cursor..cursor+4].try_into().ok()?);
                cursor += 4;
            }
            control_points.push((
                Vec3::new(vals[0], vals[1], vals[2]),
                Vec3::new(vals[3], vals[4], vals[5]),
                Vec3::new(vals[6], vals[7], vals[8]),
                vals[9],
            ));
        }
        let closed = if cursor < data.len() { data[cursor] != 0 } else { false };
        Some(SplineSerializedData {
            spline_id,
            spline_type: SplineType::CatmullRom,
            control_points,
            closed,
            name: String::new(),
            metadata: HashMap::new(),
        })
    }
}

// ============================================================
// SPLINE EDITOR (main struct)
// ============================================================

#[derive(Debug)]
pub struct SplineEditor {
    // Spline storage
    pub catmull_splines: HashMap<u64, CatmullRomSpline>,
    pub bezier_splines:  HashMap<u64, CubicBezierSpline>,
    pub bsplines:        HashMap<u64, BSpline>,
    pub nurbs_splines:   HashMap<u64, NurbsSpline>,
    pub hermite_splines: HashMap<u64, HermiteSpline>,
    pub spline_names:    HashMap<u64, String>,
    pub spline_types:    HashMap<u64, SplineType>,

    // Rail system
    pub rail_tracks: HashMap<u64, RailTrack>,
    pub camera_rails: HashMap<u64, CameraRail>,

    // Path network
    pub path_network: PathNetwork,
    pub traffic_system: Option<TrafficSystem>,

    // Constrained objects
    pub constrained_objects: Vec<SplineConstrainedObject>,
    pub chains: Vec<SplineChain>,

    // Editor state
    pub selection: SelectionState,
    pub undo_history: UndoHistory,
    pub debug_viz: SplineDebugViz,

    // Settings
    pub default_alpha: f32,   // centripetal alpha for new Catmull-Rom splines
    pub snap_to_grid: bool,
    pub grid_size:    f32,
    pub show_debug:   bool,
    pub show_curvature_comb: bool,
    pub show_arc_length_marks: bool,
    pub curvature_comb_scale: f32,
    pub arc_length_mark_interval: f32,
    pub show_frenet_frames: bool,
    pub frenet_frame_scale: f32,

    // Mesh generation settings
    pub mesh_section: CrossSection,
    pub mesh_resolution: usize,
    pub generated_meshes: HashMap<u64, SplineMesh>,
}

impl SplineEditor {
    pub fn new() -> Self {
        SplineEditor {
            catmull_splines: HashMap::new(),
            bezier_splines:  HashMap::new(),
            bsplines:        HashMap::new(),
            nurbs_splines:   HashMap::new(),
            hermite_splines: HashMap::new(),
            spline_names:    HashMap::new(),
            spline_types:    HashMap::new(),
            rail_tracks:     HashMap::new(),
            camera_rails:    HashMap::new(),
            path_network:    PathNetwork::new(),
            traffic_system:  None,
            constrained_objects: Vec::new(),
            chains:          Vec::new(),
            selection:       SelectionState::new(),
            undo_history:    UndoHistory::new(128),
            debug_viz:       SplineDebugViz::new(),
            default_alpha:   0.5,
            snap_to_grid:    false,
            grid_size:       1.0,
            show_debug:      false,
            show_curvature_comb: false,
            show_arc_length_marks: false,
            curvature_comb_scale: CURVATURE_COMB_SCALE,
            arc_length_mark_interval: 1.0,
            show_frenet_frames: false,
            frenet_frame_scale: 0.3,
            mesh_section: CrossSection::circle(0.5, 12),
            mesh_resolution: 64,
            generated_meshes: HashMap::new(),
        }
    }

    fn snap(&self, pos: Vec3) -> Vec3 {
        if self.snap_to_grid {
            let g = self.grid_size;
            Vec3::new(
                (pos.x / g).round() * g,
                (pos.y / g).round() * g,
                (pos.z / g).round() * g,
            )
        } else {
            pos
        }
    }

    // ---- CATMULL-ROM OPERATIONS ----

    pub fn create_catmull_spline(&mut self, points: Vec<Vec3>, name: &str) -> u64 {
        let id = rand_id();
        let points: Vec<Vec3> = points.into_iter().map(|p| self.snap(p)).collect();
        let spline = CatmullRomSpline::new(points, self.default_alpha, false);
        self.catmull_splines.insert(id, spline);
        self.spline_names.insert(id, name.to_string());
        self.spline_types.insert(id, SplineType::CatmullRom);
        self.undo_history.push(SplineEditorCommand::AddSpline { spline_id: id });
        id
    }

    pub fn remove_catmull_spline(&mut self, id: u64) {
        if let Some(_) = self.catmull_splines.remove(&id) {
            self.spline_names.remove(&id);
            self.spline_types.remove(&id);
            self.undo_history.push(SplineEditorCommand::RemoveSpline { spline_id: id });
        }
    }

    pub fn add_control_point(&mut self, spline_id: u64, position: Vec3) {
        let position = self.snap(position);
        if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
            let index = spline.control_points.len();
            let cp = ControlPoint::new(position);
            self.undo_history.push(SplineEditorCommand::AddControlPoint {
                spline_id, index, point: cp.clone(),
            });
            spline.control_points.push(cp);
            spline.rebuild_arc_length_table();
        }
    }

    pub fn remove_control_point(&mut self, spline_id: u64, index: usize) {
        if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
            if index < spline.control_points.len() {
                let point = spline.control_points.remove(index);
                self.undo_history.push(SplineEditorCommand::RemoveControlPoint {
                    spline_id, index, point,
                });
                spline.rebuild_arc_length_table();
            }
        }
    }

    pub fn move_control_point(&mut self, spline_id: u64, index: usize, new_pos: Vec3) {
        let new_pos = self.snap(new_pos);
        if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
            if index < spline.control_points.len() {
                let old_pos = spline.control_points[index].position;
                spline.control_points[index].position = new_pos;
                self.undo_history.push(SplineEditorCommand::MoveControlPoint {
                    spline_id, index, old_pos, new_pos,
                });
                spline.rebuild_arc_length_table();
            }
        }
    }

    pub fn insert_knot_at(&mut self, spline_id: u64, t: f32) {
        if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
            self.undo_history.push(SplineEditorCommand::InsertKnot { spline_id, t });
            spline.insert_knot(t);
        }
    }

    pub fn toggle_closed_spline(&mut self, spline_id: u64) {
        if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
            spline.toggle_closed();
            self.undo_history.push(SplineEditorCommand::ToggleClosed { spline_id });
        }
    }

    pub fn split_spline(&mut self, spline_id: u64, t: f32) -> Option<(u64, u64)> {
        let spline = self.catmull_splines.remove(&spline_id)?;
        let (a, b) = spline.split_at(t);
        let id_a = rand_id();
        let id_b = rand_id();
        let name_a = format!("{}_A", self.spline_names.get(&spline_id).cloned().unwrap_or_default());
        let name_b = format!("{}_B", self.spline_names.get(&spline_id).cloned().unwrap_or_default());
        self.catmull_splines.insert(id_a, a);
        self.catmull_splines.insert(id_b, b);
        self.spline_names.insert(id_a, name_a);
        self.spline_names.insert(id_b, name_b);
        self.spline_types.insert(id_a, SplineType::CatmullRom);
        self.spline_types.insert(id_b, SplineType::CatmullRom);
        self.undo_history.push(SplineEditorCommand::SplitSpline { spline_id, t });
        Some((id_a, id_b))
    }

    pub fn join_splines(&mut self, id_a: u64, id_b: u64) -> Option<u64> {
        let a = self.catmull_splines.remove(&id_a)?;
        let b = self.catmull_splines.remove(&id_b)?;
        let joined = CatmullRomSpline::join(a, b);
        let new_id = rand_id();
        let name = format!("{}_{}",
            self.spline_names.get(&id_a).cloned().unwrap_or_default(),
            self.spline_names.get(&id_b).cloned().unwrap_or_default(),
        );
        self.catmull_splines.insert(new_id, joined);
        self.spline_names.insert(new_id, name);
        self.spline_types.insert(new_id, SplineType::CatmullRom);
        self.undo_history.push(SplineEditorCommand::JoinSplines { spline_a: id_a, spline_b: id_b });
        Some(new_id)
    }

    // ---- BEZIER OPERATIONS ----

    pub fn create_bezier_spline(&mut self, points: &[Vec3], name: &str) -> u64 {
        let id = rand_id();
        let spline = CubicBezierSpline::from_points(points);
        self.bezier_splines.insert(id, spline);
        self.spline_names.insert(id, name.to_string());
        self.spline_types.insert(id, SplineType::CubicBezier);
        self.undo_history.push(SplineEditorCommand::AddSpline { spline_id: id });
        id
    }

    pub fn split_bezier_segment(&mut self, spline_id: u64, seg: usize, u: f32) {
        if let Some(spline) = self.bezier_splines.get_mut(&spline_id) {
            spline.split_segment(seg, u);
        }
    }

    // ---- B-SPLINE OPERATIONS ----

    pub fn create_bspline(&mut self, points: Vec<Vec3>, degree: usize, name: &str) -> u64 {
        let id = rand_id();
        let spline = BSpline::new(points, degree, false);
        self.bsplines.insert(id, spline);
        self.spline_names.insert(id, name.to_string());
        self.spline_types.insert(id, SplineType::BSpline { degree });
        self.undo_history.push(SplineEditorCommand::AddSpline { spline_id: id });
        id
    }

    pub fn insert_bspline_knot(&mut self, spline_id: u64, t: f32) {
        if let Some(spline) = self.bsplines.get_mut(&spline_id) {
            spline.insert_knot(t);
        }
    }

    // ---- NURBS OPERATIONS ----

    pub fn create_nurbs(&mut self, points: Vec<Vec3>, weights: Vec<f32>, degree: usize, name: &str) -> u64 {
        let id = rand_id();
        let spline = NurbsSpline::new(points, weights, degree);
        self.nurbs_splines.insert(id, spline);
        self.spline_names.insert(id, name.to_string());
        self.spline_types.insert(id, SplineType::Nurbs { degree });
        self.undo_history.push(SplineEditorCommand::AddSpline { spline_id: id });
        id
    }

    // ---- HERMITE OPERATIONS ----

    pub fn create_hermite_spline(&mut self, points: Vec<(Vec3, Vec3)>, name: &str) -> u64 {
        let id = rand_id();
        let mut spline = HermiteSpline::new(points);
        spline.auto_tangents();
        self.hermite_splines.insert(id, spline);
        self.spline_names.insert(id, name.to_string());
        self.spline_types.insert(id, SplineType::Hermite);
        self.undo_history.push(SplineEditorCommand::AddSpline { spline_id: id });
        id
    }

    // ---- RAIL TRACK OPERATIONS ----

    pub fn create_rail_track(&mut self, spline_id: u64, gauge: f32) -> Option<u64> {
        let spline = self.catmull_splines.get(&spline_id)?.clone();
        let track = RailTrack::new(spline, gauge);
        let id = track.id;
        self.rail_tracks.insert(id, track);
        Some(id)
    }

    pub fn rail_banking_at(&self, track_id: u64, t: f32, speed_ms: f32) -> Option<f32> {
        let track = self.rail_tracks.get(&track_id)?;
        Some(track.banking_angle_at(t, speed_ms))
    }

    pub fn get_rail_mesh(&self, track_id: u64, resolution: usize, speed_ms: f32) -> Option<RailMeshData> {
        let track = self.rail_tracks.get(&track_id)?;
        let mut mesh = track.rail_mesh_data(resolution, speed_ms);
        track.add_sleepers(&mut mesh, 0.6);
        Some(mesh)
    }

    // ---- CAMERA RAIL OPERATIONS ----

    pub fn create_camera_rail(&mut self, spline_id: u64) -> Option<u64> {
        let spline = self.catmull_splines.get(&spline_id)?.clone();
        let rail = CameraRail::new(spline);
        let id = rand_id();
        self.camera_rails.insert(id, rail);
        Some(id)
    }

    pub fn camera_transform_at(&self, rail_id: u64, t: f32) -> Option<Mat4> {
        let rail = self.camera_rails.get(&rail_id)?;
        Some(rail.camera_transform_at(t))
    }

    pub fn bake_camera_path(&self, rail_id: u64, steps: usize) -> Vec<(Mat4, f32)> {
        self.camera_rails.get(&rail_id)
            .map(|r| r.bake_camera_path(steps))
            .unwrap_or_default()
    }

    // ---- MESH GENERATION ----

    pub fn generate_mesh_for_spline(&mut self, spline_id: u64) -> bool {
        let spline = match self.catmull_splines.get(&spline_id) {
            Some(s) => s.clone(),
            None => return false,
        };
        let total_length = spline.total_arc_length();
        let section = self.mesh_section.clone();
        let resolution = self.mesh_resolution;
        let mesh = SplineMesh::generate_from_spline(
            &|t| spline.evaluate(t),
            &|t| spline.evaluate_derivative(t),
            &section,
            resolution,
            total_length,
        );
        self.generated_meshes.insert(spline_id, mesh);
        true
    }

    pub fn generate_lod_mesh(&mut self, spline_id: u64, min_steps: usize, max_steps: usize) -> bool {
        let spline = match self.catmull_splines.get(&spline_id) {
            Some(s) => s.clone(),
            None => return false,
        };
        let total_length = spline.total_arc_length();
        let section = self.mesh_section.clone();
        let mesh = SplineMesh::generate_lod(
            &|t| spline.evaluate(t),
            &|t| spline.evaluate_derivative(t),
            &|t| spline.curvature_at(t),
            &section,
            min_steps,
            max_steps,
            total_length,
        );
        self.generated_meshes.insert(spline_id, mesh);
        true
    }

    // ---- SPLINE PHYSICS ----

    pub fn add_constrained_object(&mut self, spline_id: u64, t: f32, mass: f32) -> u64 {
        let obj = SplineConstrainedObject::new(spline_id, t, mass);
        let id = obj.id;
        self.constrained_objects.push(obj);
        id
    }

    pub fn update_physics(&mut self, dt: f32) {
        for obj in &mut self.constrained_objects {
            if let Some(spline) = self.catmull_splines.get(&obj.spline_id) {
                let spline_clone = spline.clone();
                obj.update(dt, &spline_clone);
            }
        }
        if let Some(ts) = &mut self.traffic_system {
            ts.update(dt);
        }
    }

    pub fn add_chain(&mut self, spline_id: u64, link_length: f32) {
        if let Some(spline) = self.catmull_splines.get(&spline_id) {
            let chain = SplineChain::new(spline, spline_id, link_length);
            self.chains.push(chain);
        }
    }

    // ---- PATH NETWORK ----

    pub fn setup_traffic_system(&mut self) {
        let network = self.path_network.clone();
        self.traffic_system = Some(TrafficSystem::new(network));
    }

    pub fn plan_path(&self, start_node: u64, end_node: u64) -> Option<Vec<u64>> {
        self.path_network.astar(start_node, end_node)
    }

    // ---- NEAREST POINT QUERIES ----

    pub fn nearest_point_on_any_spline(&self, query: Vec3) -> Option<(u64, f32, Vec3)> {
        let mut best_id = 0u64;
        let mut best_t  = 0.0_f32;
        let mut best_p  = Vec3::ZERO;
        let mut best_d  = f32::MAX;

        for (&id, spline) in &self.catmull_splines {
            let (t, p) = spline.nearest_point(query);
            let d = (p - query).length_squared();
            if d < best_d {
                best_d = d;
                best_id = id;
                best_t  = t;
                best_p  = p;
            }
        }
        for (&id, spline) in &self.bezier_splines {
            let (t, p) = spline.nearest_point(query);
            let d = (p - query).length_squared();
            if d < best_d {
                best_d = d;
                best_id = id;
                best_t  = t;
                best_p  = p;
            }
        }
        if best_id == 0 { None } else { Some((best_id, best_t, best_p)) }
    }

    // ---- INTERSECTION QUERIES ----

    pub fn find_spline_plane_intersections(
        &self, spline_id: u64, plane_normal: Vec3, plane_d: f32,
    ) -> Vec<SplinePlaneIntersection> {
        if let Some(spline) = self.catmull_splines.get(&spline_id) {
            intersect_spline_plane(&|t| spline.evaluate(t), plane_normal, plane_d, 200)
        } else { Vec::new() }
    }

    pub fn find_spline_spline_intersections(
        &self, id_a: u64, id_b: u64, tol: f32,
    ) -> Vec<SplineSplineIntersection> {
        let a = self.catmull_splines.get(&id_a);
        let b = self.catmull_splines.get(&id_b);
        if let (Some(sa), Some(sb)) = (a, b) {
            intersect_spline_spline(
                &|t| sa.evaluate(t),
                &|t| sb.evaluate(t),
                32, tol,
            )
        } else { Vec::new() }
    }

    // ---- DEBUG VIZ UPDATE ----

    pub fn update_debug_viz(&mut self) {
        self.debug_viz.clear();
        if !self.show_debug { return; }

        let ids: Vec<u64> = self.catmull_splines.keys().cloned().collect();
        for id in ids {
            let spline = match self.catmull_splines.get(&id) { Some(s) => s.clone(), None => continue };
            // Draw the spline curve
            let color = if self.selection.selected.contains(&id) {
                Vec4::new(1.0, 0.8, 0.0, 1.0)
            } else {
                Vec4::new(0.4, 0.9, 0.4, 1.0)
            };
            self.debug_viz.draw_spline_curve(&|t| spline.evaluate(t), 128, color);
            // Control polygon
            let pts: Vec<Vec3> = spline.control_points.iter().map(|cp| cp.position).collect();
            self.debug_viz.draw_control_polygon(&pts, Vec4::new(0.6, 0.6, 0.6, 0.5));
            // Bounding box
            let (bb_min, bb_max) = spline.bounding_box();
            self.debug_viz.draw_bounding_box(bb_min, bb_max, Vec4::new(0.3, 0.3, 1.0, 0.4));
            // Frenet frames
            if self.show_frenet_frames {
                let scale = self.frenet_frame_scale;
                self.debug_viz.draw_frenet_frames(
                    &|t| spline.evaluate(t),
                    &|t| spline.evaluate_derivative(t),
                    &|t| spline.evaluate_second_derivative(t),
                    &|t| {
                        let dt = 1e-4;
                        let a = spline.evaluate_second_derivative((t + dt).min(1.0));
                        let b = spline.evaluate_second_derivative((t - dt).max(0.0));
                        (a - b) / (2.0 * dt)
                    },
                    16,
                    scale,
                );
            }
            // Curvature comb
            if self.show_curvature_comb {
                let scale = self.curvature_comb_scale;
                self.debug_viz.draw_curvature_comb(
                    &|t| spline.evaluate(t),
                    &|t| spline.curvature_at(t),
                    &|t| spline.frenet_frame_at(t).normal,
                    64,
                    scale,
                );
            }
            // Arc length marks
            if self.show_arc_length_marks {
                let interval = self.arc_length_mark_interval;
                let total = spline.total_arc_length();
                self.debug_viz.draw_arc_length_marks(
                    &|t| spline.evaluate(t),
                    &|s| spline.t_at_arc_length(s),
                    total,
                    interval,
                    Vec3::Y,
                    0.15,
                );
            }
        }
    }

    // ---- UNDO / REDO ----

    pub fn undo(&mut self) {
        if let Some(cmd) = self.undo_history.undo() {
            self.apply_undo(cmd);
        }
    }

    pub fn redo(&mut self) {
        if let Some(cmd) = self.undo_history.redo() {
            self.apply_redo(cmd);
        }
    }

    fn apply_undo(&mut self, cmd: SplineEditorCommand) {
        match cmd {
            SplineEditorCommand::MoveControlPoint { spline_id, index, old_pos, .. } => {
                if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
                    if index < spline.control_points.len() {
                        spline.control_points[index].position = old_pos;
                        spline.rebuild_arc_length_table();
                    }
                }
            }
            SplineEditorCommand::AddControlPoint { spline_id, index, .. } => {
                if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
                    if index < spline.control_points.len() {
                        spline.control_points.remove(index);
                        spline.rebuild_arc_length_table();
                    }
                }
            }
            SplineEditorCommand::RemoveControlPoint { spline_id, index, point } => {
                if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
                    spline.control_points.insert(index.min(spline.control_points.len()), point);
                    spline.rebuild_arc_length_table();
                }
            }
            SplineEditorCommand::ToggleClosed { spline_id } => {
                if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
                    spline.toggle_closed();
                }
            }
            _ => { /* other commands handled separately */ }
        }
    }

    fn apply_redo(&mut self, cmd: SplineEditorCommand) {
        match cmd {
            SplineEditorCommand::MoveControlPoint { spline_id, index, new_pos, .. } => {
                if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
                    if index < spline.control_points.len() {
                        spline.control_points[index].position = new_pos;
                        spline.rebuild_arc_length_table();
                    }
                }
            }
            SplineEditorCommand::AddControlPoint { spline_id, index, point } => {
                if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
                    spline.control_points.insert(index.min(spline.control_points.len()), point);
                    spline.rebuild_arc_length_table();
                }
            }
            SplineEditorCommand::RemoveControlPoint { spline_id, index, .. } => {
                if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
                    if index < spline.control_points.len() {
                        spline.control_points.remove(index);
                        spline.rebuild_arc_length_table();
                    }
                }
            }
            SplineEditorCommand::ToggleClosed { spline_id } => {
                if let Some(spline) = self.catmull_splines.get_mut(&spline_id) {
                    spline.toggle_closed();
                }
            }
            _ => {}
        }
    }

    // ---- SERIALIZATION ----

    pub fn serialize_spline(&self, spline_id: u64) -> Option<SplineSerializedData> {
        let spline = self.catmull_splines.get(&spline_id)?;
        let name = self.spline_names.get(&spline_id).cloned().unwrap_or_default();
        Some(SplineSerializedData::serialize_catmull_rom(spline, spline_id, &name))
    }

    pub fn serialize_all(&self) -> Vec<SplineSerializedData> {
        self.catmull_splines.iter().map(|(&id, spline)| {
            let name = self.spline_names.get(&id).cloned().unwrap_or_default();
            SplineSerializedData::serialize_catmull_rom(spline, id, &name)
        }).collect()
    }

    pub fn deserialize_and_add(&mut self, data: SplineSerializedData) {
        let points: Vec<Vec3> = data.control_points.iter().map(|(p, _, _, _)| *p).collect();
        let id = data.spline_id;
        let mut spline = CatmullRomSpline::new(points, self.default_alpha, data.closed);
        // Restore weights / tangents
        for (i, (_, t_in, t_out, w)) in data.control_points.iter().enumerate() {
            if i < spline.control_points.len() {
                spline.control_points[i].tangent_in  = *t_in;
                spline.control_points[i].tangent_out = *t_out;
                spline.control_points[i].weight      = *w;
            }
        }
        spline.rebuild_arc_length_table();
        self.catmull_splines.insert(id, spline);
        self.spline_names.insert(id, data.name);
        self.spline_types.insert(id, data.spline_type);
    }

    // ---- QUERY HELPERS ----

    pub fn spline_ids(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self.catmull_splines.keys().cloned().collect();
        ids.extend(self.bezier_splines.keys().cloned());
        ids.extend(self.bsplines.keys().cloned());
        ids.extend(self.nurbs_splines.keys().cloned());
        ids.extend(self.hermite_splines.keys().cloned());
        ids
    }

    pub fn spline_count(&self) -> usize {
        self.catmull_splines.len()
            + self.bezier_splines.len()
            + self.bsplines.len()
            + self.nurbs_splines.len()
            + self.hermite_splines.len()
    }

    pub fn evaluate_spline(&self, id: u64, t: f32) -> Option<Vec3> {
        if let Some(s) = self.catmull_splines.get(&id) { return Some(s.evaluate(t)); }
        if let Some(s) = self.bezier_splines.get(&id)  { return Some(s.evaluate(t)); }
        if let Some(s) = self.bsplines.get(&id)        { return Some(s.evaluate(t)); }
        if let Some(s) = self.nurbs_splines.get(&id)   { return Some(s.evaluate(t)); }
        if let Some(s) = self.hermite_splines.get(&id) { return Some(s.evaluate(t)); }
        None
    }

    pub fn spline_arc_length(&self, id: u64) -> f32 {
        if let Some(s) = self.catmull_splines.get(&id) { return s.total_arc_length(); }
        if let Some(s) = self.bezier_splines.get(&id)  { return s.total_arc_length(); }
        if let Some(s) = self.bsplines.get(&id)        { return s.total_arc_length(); }
        if let Some(s) = self.nurbs_splines.get(&id)   { return s.total_arc_length(); }
        if let Some(s) = self.hermite_splines.get(&id) { return s.total_arc_length(); }
        0.0
    }

    pub fn curvature_at(&self, id: u64, t: f32) -> f32 {
        if let Some(s) = self.catmull_splines.get(&id) { return s.curvature_at(t); }
        if let Some(s) = self.bezier_splines.get(&id)  { return s.curvature_at(t); }
        if let Some(s) = self.bsplines.get(&id)        { return s.curvature_at(t); }
        if let Some(s) = self.nurbs_splines.get(&id)   { return s.curvature_at(t); }
        0.0
    }
}

// ============================================================
// ADDITIONAL SPLINE MATH UTILITIES
// ============================================================

/// Reparameterize a polyline by arc length to produce evenly spaced points
pub fn resample_polyline(pts: &[Vec3], n_out: usize) -> Vec<Vec3> {
    if pts.len() < 2 || n_out < 2 { return pts.to_vec(); }
    // Compute cumulative arc lengths
    let mut lengths = Vec::with_capacity(pts.len());
    lengths.push(0.0_f32);
    for i in 1..pts.len() {
        lengths.push(lengths[i - 1] + (pts[i] - pts[i - 1]).length());
    }
    let total = *lengths.last().unwrap();
    let mut out = Vec::with_capacity(n_out);
    for i in 0..n_out {
        let target_s = i as f32 / (n_out - 1) as f32 * total;
        let idx = lengths.partition_point(|&l| l <= target_s);
        let p = if idx == 0 {
            pts[0]
        } else if idx >= pts.len() {
            *pts.last().unwrap()
        } else {
            let s0 = lengths[idx - 1];
            let s1 = lengths[idx];
            let frac = if (s1 - s0).abs() < EPSILON { 0.0 } else { (target_s - s0) / (s1 - s0) };
            lerp_vec3(pts[idx - 1], pts[idx], frac)
        };
        out.push(p);
    }
    out
}

/// Compute signed curvature of a 2D polyline
pub fn polyline_signed_curvature_2d(pts: &[Vec2]) -> Vec<f32> {
    let n = pts.len();
    if n < 3 { return vec![0.0; n]; }
    let mut kappas = vec![0.0_f32; n];
    for i in 1..n - 1 {
        let a = pts[i - 1];
        let b = pts[i];
        let c = pts[i + 1];
        let ab = b - a;
        let bc = c - b;
        let cross = ab.x * bc.y - ab.y * bc.x; // 2D cross product
        let dot   = ab.dot(bc);
        let angle = cross.atan2(dot);
        let seg_len = (ab.length() + bc.length()) * 0.5;
        kappas[i] = if seg_len > EPSILON { angle / seg_len } else { 0.0 };
    }
    kappas[0]       = kappas[1];
    kappas[n - 1]   = kappas[n - 2];
    kappas
}

/// Smooth a polyline with a Gaussian kernel
pub fn smooth_polyline(pts: &[Vec3], iterations: usize, strength: f32) -> Vec<Vec3> {
    let n = pts.len();
    if n < 3 { return pts.to_vec(); }
    let mut result = pts.to_vec();
    for _ in 0..iterations {
        let prev = result.clone();
        for i in 1..n - 1 {
            let avg = (prev[i - 1] + prev[i + 1]) * 0.5;
            result[i] = lerp_vec3(prev[i], avg, strength);
        }
    }
    result
}

/// Douglas-Peucker polyline simplification
pub fn douglas_peucker(pts: &[Vec3], epsilon: f32) -> Vec<Vec3> {
    if pts.len() < 3 { return pts.to_vec(); }
    // Find point with max distance from line first-last
    let mut max_dist = 0.0_f32;
    let mut max_idx  = 0usize;
    let start = pts[0];
    let end   = *pts.last().unwrap();
    let seg = end - start;
    let seg_len_sq = seg.length_squared();
    for i in 1..pts.len() - 1 {
        let dist = if seg_len_sq < EPSILON {
            (pts[i] - start).length()
        } else {
            let t = ((pts[i] - start).dot(seg) / seg_len_sq).clamp(0.0, 1.0);
            let proj = start + seg * t;
            (pts[i] - proj).length()
        };
        if dist > max_dist {
            max_dist = dist;
            max_idx  = i;
        }
    }
    if max_dist > epsilon {
        let mut left  = douglas_peucker(&pts[..=max_idx], epsilon);
        let right = douglas_peucker(&pts[max_idx..], epsilon);
        left.pop(); // remove duplicate
        left.extend(right);
        left
    } else {
        vec![pts[0], *pts.last().unwrap()]
    }
}

/// Catmull-Clark subdivision of a 3D polyline (single iteration)
pub fn catmull_clark_subdivide_1d(pts: &[Vec3], closed: bool) -> Vec<Vec3> {
    let n = pts.len();
    if n < 2 { return pts.to_vec(); }
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n - 1 {
        out.push(pts[i]);
        out.push((pts[i] + pts[i + 1]) * 0.5);
    }
    out.push(*pts.last().unwrap());
    // Smooth
    let raw = out.clone();
    let m = raw.len();
    let mut smoothed = vec![Vec3::ZERO; m];
    smoothed[0]     = raw[0];
    smoothed[m - 1] = raw[m - 1];
    for i in 1..m - 1 {
        smoothed[i] = raw[i - 1] * 0.25 + raw[i] * 0.5 + raw[i + 1] * 0.25;
    }
    smoothed
}

/// Compute the osculating circle at a point: returns (center, radius)
pub fn osculating_circle(pos: Vec3, tangent: Vec3, normal: Vec3, curvature: f32) -> (Vec3, f32) {
    if curvature < EPSILON {
        return (pos + normal * 1e9, 1e9);
    }
    let r = 1.0 / curvature;
    let center = pos + normal * r;
    (center, r)
}

/// Evolute of a curve: locus of centers of osculating circles
pub fn compute_evolute(
    pos_fn: &dyn Fn(f32) -> Vec3,
    normal_fn: &dyn Fn(f32) -> Vec3,
    curvature_fn: &dyn Fn(f32) -> f32,
    steps: usize,
) -> Vec<Vec3> {
    (0..=steps).map(|i| {
        let t = i as f32 / steps as f32;
        let (center, _) = osculating_circle(pos_fn(t), Vec3::ZERO, normal_fn(t), curvature_fn(t));
        center
    }).collect()
}

/// Involute of a spline: unrolling a string from the curve
pub fn compute_involute(
    pos_fn: &dyn Fn(f32) -> Vec3,
    tangent_fn: &dyn Fn(f32) -> Vec3,
    t_at_len_fn: &dyn Fn(f32) -> f32,
    total_length: f32,
    start_s: f32,
    steps: usize,
) -> Vec<Vec3> {
    (0..=steps).map(|i| {
        let t = i as f32 / steps as f32;
        let s = t * total_length;
        let p = pos_fn(t);
        let tang = safe_normalize(tangent_fn(t));
        let arc_remaining = (s - start_s).max(0.0);
        p - tang * arc_remaining
    }).collect()
}

/// Compute the writhe of a closed curve (Gauss integral)
pub fn compute_writhe(pts: &[Vec3]) -> f32 {
    let n = pts.len();
    if n < 3 { return 0.0; }
    let mut writhe = 0.0_f32;
    for i in 0..n {
        let r1 = pts[i];
        let r1n = pts[(i + 1) % n];
        let dr1 = r1n - r1;
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 { continue; }
            let r2 = pts[j];
            let r2n = pts[(j + 1) % n];
            let dr2 = r2n - r2;
            let r = r2 - r1;
            let r_len = r.length();
            if r_len < EPSILON { continue; }
            let cross = dr1.cross(dr2);
            writhe += cross.dot(r) / (r_len * r_len * r_len);
        }
    }
    writhe / (4.0 * std::f32::consts::PI)
}

// ============================================================
// EXTRA SPLINE EDITOR UI HELPERS
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineEditorUIState {
    pub active_tool: SplineTool,
    pub drag_start: Option<Vec3>,
    pub drag_current: Option<Vec3>,
    pub hover_t: f32,
    pub hover_position: Vec3,
    pub show_tangent_handles: bool,
    pub tangent_handle_scale: f32,
    pub tangent_mirror: bool,         // mirror in/out tangents
    pub show_weights:   bool,
    pub edit_mode: SplineEditMode,
    pub snap_angle: f32,              // degrees, for tangent snapping
    pub snap_angle_enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SplineTool {
    Select,
    AddPoint,
    RemovePoint,
    MoveTangent,
    SliceAtCursor,
    MeasureLength,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SplineEditMode {
    Points,
    Tangents,
    Knots,
    Weights,
}

impl SplineEditorUIState {
    pub fn new() -> Self {
        SplineEditorUIState {
            active_tool: SplineTool::Select,
            drag_start: None,
            drag_current: None,
            hover_t: 0.0,
            hover_position: Vec3::ZERO,
            show_tangent_handles: true,
            tangent_handle_scale: 1.0,
            tangent_mirror: true,
            show_weights: false,
            edit_mode: SplineEditMode::Points,
            snap_angle: 15.0,
            snap_angle_enabled: false,
        }
    }

    pub fn snap_tangent_to_angle(&self, tangent: Vec3) -> Vec3 {
        if !self.snap_angle_enabled { return tangent; }
        let snap_rad = self.snap_angle.to_radians();
        let len = tangent.length();
        if len < EPSILON { return tangent; }
        let dir = tangent / len;
        // Snap to nearest multiple of snap_angle around Y axis
        let angle = dir.x.atan2(dir.z);
        let snapped = (angle / snap_rad).round() * snap_rad;
        Vec3::new(snapped.sin() * len, tangent.y, snapped.cos() * len)
    }

    pub fn mirror_tangent(&self, tangent: Vec3) -> Vec3 {
        if self.tangent_mirror { -tangent } else { tangent }
    }

    pub fn drag_delta(&self) -> Vec3 {
        match (self.drag_start, self.drag_current) {
            (Some(s), Some(c)) => c - s,
            _ => Vec3::ZERO,
        }
    }
}

// ============================================================
// SPLINE SAMPLING UTILITIES
// ============================================================

/// Sample evenly in arc length
pub fn sample_arc_length_uniform(
    pos_fn: &dyn Fn(f32) -> Vec3,
    t_at_len_fn: &dyn Fn(f32) -> f32,
    total_length: f32,
    n: usize,
) -> Vec<Vec3> {
    if n == 0 { return Vec::new(); }
    (0..n).map(|i| {
        let s = i as f32 / (n - 1).max(1) as f32 * total_length;
        pos_fn(t_at_len_fn(s))
    }).collect()
}

/// Sample using chord-length parameterization
pub fn sample_chord_length(pts: &[Vec3], n: usize) -> Vec<Vec3> {
    if pts.len() < 2 { return pts.to_vec(); }
    let total: f32 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    let mut cum = vec![0.0_f32];
    for w in pts.windows(2) {
        cum.push(*cum.last().unwrap() + (w[1] - w[0]).length());
    }
    (0..n).map(|i| {
        let target = i as f32 / (n - 1).max(1) as f32 * total;
        let idx = cum.partition_point(|&c| c <= target).min(cum.len() - 1);
        let idx = idx.max(1);
        let s0 = cum[idx - 1];
        let s1 = cum[idx];
        let f = if (s1 - s0).abs() < EPSILON { 0.0 } else { (target - s0) / (s1 - s0) };
        lerp_vec3(pts[idx - 1], pts[idx.min(pts.len() - 1)], f)
    }).collect()
}

// ============================================================
// BEZIER FITTING (least-squares fitting to point cloud)
// ============================================================

pub struct BezierFitter {
    pub max_error: f32,
    pub max_iterations: usize,
}

impl BezierFitter {
    pub fn new(max_error: f32) -> Self {
        BezierFitter { max_error, max_iterations: 32 }
    }

    /// Fit a single cubic bezier to a sequence of points
    pub fn fit_cubic(&self, pts: &[Vec3]) -> Option<[Vec3; 4]> {
        let n = pts.len();
        if n < 2 { return None; }
        if n == 2 {
            let t1 = (pts[1] - pts[0]) / 3.0;
            return Some([pts[0], pts[0] + t1, pts[1] - t1, pts[1]]);
        }
        // Chord-length parameterization
        let params = chord_length_params(pts);
        let d1 = safe_normalize(pts[1] - pts[0]);
        let dn = safe_normalize(pts[n - 1] - pts[n - 2]);
        // Least-squares
        self.fit_cubic_with_tangents(pts, &params, d1, dn)
    }

    fn fit_cubic_with_tangents(
        &self, pts: &[Vec3], params: &[f32], t0: Vec3, t1: Vec3
    ) -> Option<[Vec3; 4]> {
        let n = pts.len();
        let p0 = pts[0];
        let p3 = pts[n - 1];
        // Solve 2x2 linear system for alpha0, alpha1
        let mut a00 = 0.0_f32;
        let mut a01 = 0.0_f32;
        let mut a11 = 0.0_f32;
        let mut b0  = Vec3::ZERO;
        let mut b1  = Vec3::ZERO;
        for (i, &t) in params.iter().enumerate() {
            let b0_t = bernstein(0, 3, t);
            let b1_t = bernstein(1, 3, t);
            let b2_t = bernstein(2, 3, t);
            let b3_t = bernstein(3, 3, t);
            let a0i = t0 * b1_t;
            let a1i = t1 * b2_t;
            a00 += a0i.dot(a0i);
            a01 += a0i.dot(a1i);
            a11 += a1i.dot(a1i);
            let tmp = pts[i] - (p0 * (b0_t + b1_t) + p3 * (b2_t + b3_t));
            b0 += a0i * tmp.dot(a0i) / a0i.dot(a0i).max(EPSILON);
            b1 += a1i * tmp.dot(a1i) / a1i.dot(a1i).max(EPSILON);
        }
        let det = a00 * a11 - a01 * a01;
        let (alpha0, alpha1) = if det.abs() > EPSILON {
            let b0s = b0.length();
            let b1s = b1.length();
            let al0 = (a11 * b0s - a01 * b1s) / det;
            let al1 = (a00 * b1s - a01 * b0s) / det;
            (al0.max(EPSILON), al1.max(EPSILON))
        } else {
            let chord = (p3 - p0).length() / 3.0;
            (chord, chord)
        };
        Some([p0, p0 + t0 * alpha0, p3 - t1 * alpha1, p3])
    }
}

fn bernstein(i: usize, n: usize, t: f32) -> f32 {
    fn binom(n: usize, k: usize) -> f32 {
        if k > n { return 0.0; }
        let mut result = 1.0_f32;
        for j in 0..k {
            result *= (n - j) as f32 / (j + 1) as f32;
        }
        result
    }
    binom(n, i) * t.powi(i as i32) * (1.0 - t).powi((n - i) as i32)
}

fn chord_length_params(pts: &[Vec3]) -> Vec<f32> {
    let n = pts.len();
    let mut lengths = vec![0.0_f32; n];
    for i in 1..n {
        lengths[i] = lengths[i - 1] + (pts[i] - pts[i - 1]).length();
    }
    let total = lengths[n - 1];
    if total < EPSILON {
        return (0..n).map(|i| i as f32 / (n - 1).max(1) as f32).collect();
    }
    lengths.iter().map(|&l| l / total).collect()
}

// ============================================================
// SPLINE OFFSET (parallel curve)
// ============================================================

pub fn offset_spline(
    pos_fn:    &dyn Fn(f32) -> Vec3,
    normal_fn: &dyn Fn(f32) -> Vec3,
    offset:    f32,
    steps:     usize,
) -> Vec<Vec3> {
    (0..=steps).map(|i| {
        let t = i as f32 / steps as f32;
        pos_fn(t) + normal_fn(t) * offset
    }).collect()
}

/// Tube mesh: a circle cross-section swept along a spline
pub fn build_tube_mesh(
    pos_fn:    &dyn Fn(f32) -> Vec3,
    tang_fn:   &dyn Fn(f32) -> Vec3,
    radius:    f32,
    seg_count: usize,
    ring_count: usize,
) -> SplineMesh {
    let section = CrossSection::circle(radius, seg_count);
    let total_length = {
        let mut s = 0.0_f32;
        let mut prev = pos_fn(0.0);
        for i in 1..=256 {
            let t = i as f32 / 256.0;
            let cur = pos_fn(t);
            s += (cur - prev).length();
            prev = cur;
        }
        s
    };
    SplineMesh::generate_from_spline(pos_fn, tang_fn, &section, ring_count, total_length)
}

// ============================================================
// SPLINE LOFTING
// ============================================================

pub struct LoftedSurface {
    pub vertices: Vec<Vec3>,
    pub normals:  Vec<Vec3>,
    pub uvs:      Vec<Vec2>,
    pub indices:  Vec<u32>,
}

impl LoftedSurface {
    /// Loft between two splines
    pub fn loft(
        spline_a: &dyn Fn(f32) -> Vec3,
        spline_b: &dyn Fn(f32) -> Vec3,
        u_steps: usize,
        v_steps: usize,
    ) -> Self {
        let mut verts   = Vec::new();
        let mut normals = Vec::new();
        let mut uvs     = Vec::new();
        let mut indices = Vec::new();

        for j in 0..=v_steps {
            let v = j as f32 / v_steps as f32;
            for i in 0..=u_steps {
                let u = i as f32 / u_steps as f32;
                let pa = spline_a(u);
                let pb = spline_b(u);
                let p  = lerp_vec3(pa, pb, v);
                // Approximate normal via finite differences
                let pa_u = spline_a((u + 1e-3).min(1.0));
                let pb_u = spline_b((u + 1e-3).min(1.0));
                let pu = lerp_vec3(pa_u, pb_u, v) - p;
                let pv = pb - pa;
                let n = safe_normalize(pu.cross(pv));
                verts.push(p);
                normals.push(n);
                uvs.push(Vec2::new(u, v));
            }
        }

        for j in 0..v_steps {
            for i in 0..u_steps {
                let a = (j * (u_steps + 1) + i) as u32;
                let b = a + 1;
                let c = ((j + 1) * (u_steps + 1) + i) as u32;
                let d = c + 1;
                indices.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }

        LoftedSurface { vertices: verts, normals, uvs, indices }
    }
}

// ============================================================
// EXTENDED CATMULL-ROM: CHORD-LENGTH VS CENTRIPETAL COMPARISON
// ============================================================

pub fn catmull_rom_compare_parameterizations(
    p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3,
    num_samples: usize,
) -> (Vec<Vec3>, Vec<Vec3>, Vec<Vec3>) {
    // Uniform
    let uniform_pts: Vec<Vec3> = (0..=num_samples).map(|i| {
        let t = i as f32 / num_samples as f32;
        CatmullRomSpline::new(vec![p0, p1, p2, p3], 0.0, false).evaluate(t)
    }).collect();
    // Centripetal
    let centripetal_pts: Vec<Vec3> = (0..=num_samples).map(|i| {
        let t = i as f32 / num_samples as f32;
        CatmullRomSpline::new(vec![p0, p1, p2, p3], 0.5, false).evaluate(t)
    }).collect();
    // Chordal
    let chordal_pts: Vec<Vec3> = (0..=num_samples).map(|i| {
        let t = i as f32 / num_samples as f32;
        CatmullRomSpline::new(vec![p0, p1, p2, p3], 1.0, false).evaluate(t)
    }).collect();
    (uniform_pts, centripetal_pts, chordal_pts)
}

// ============================================================
// SPLINE DEFORMATION (FFD-style along spline)
// ============================================================

pub struct SplineDeformer {
    pub spline_id: u64,
    pub falloff_radius: f32,
    pub strength: f32,
    pub deform_axis: Vec3,
}

impl SplineDeformer {
    pub fn new(spline_id: u64, falloff_radius: f32, strength: f32) -> Self {
        SplineDeformer {
            spline_id,
            falloff_radius,
            strength,
            deform_axis: Vec3::Y,
        }
    }

    /// Deform a mesh vertex based on proximity to spline
    pub fn deform_point(&self, point: Vec3, spline: &CatmullRomSpline) -> Vec3 {
        let (t, closest) = spline.nearest_point(point);
        let dist = (point - closest).length();
        if dist > self.falloff_radius { return point; }
        let frame = spline.frenet_frame_at(t);
        let falloff = 1.0 - (dist / self.falloff_radius).powi(2);
        let displacement = frame.normal * self.strength * falloff;
        point + displacement
    }

    pub fn deform_mesh(&self, vertices: &mut [Vec3], spline: &CatmullRomSpline) {
        for v in vertices.iter_mut() {
            *v = self.deform_point(*v, spline);
        }
    }
}

// ============================================================
// SPEED CURVES (for cinematic/rail use)
// ============================================================

#[derive(Clone, Debug)]
pub struct SpeedCurve {
    /// Control points (t, speed) with tangents for smooth interpolation
    pub keyframes: Vec<SpeedKey>,
}

#[derive(Clone, Debug)]
pub struct SpeedKey {
    pub t:     f32,
    pub speed: f32,
    pub tan_in:  f32,
    pub tan_out: f32,
}

impl SpeedCurve {
    pub fn new() -> Self { SpeedCurve { keyframes: Vec::new() } }

    pub fn add_key(&mut self, t: f32, speed: f32) {
        let idx = self.keyframes.partition_point(|k| k.t < t);
        self.keyframes.insert(idx, SpeedKey { t, speed, tan_in: 0.0, tan_out: 0.0 });
        self.auto_tangents();
    }

    pub fn auto_tangents(&mut self) {
        let n = self.keyframes.len();
        for i in 0..n {
            let prev_speed = if i > 0 { self.keyframes[i-1].speed } else { self.keyframes[i].speed };
            let next_speed = if i+1 < n { self.keyframes[i+1].speed } else { self.keyframes[i].speed };
            let tan = (next_speed - prev_speed) * 0.5;
            self.keyframes[i].tan_in  = tan;
            self.keyframes[i].tan_out = tan;
        }
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        let n = self.keyframes.len();
        if n == 0 { return 0.0; }
        if n == 1 { return self.keyframes[0].speed; }
        let idx = self.keyframes.partition_point(|k| k.t <= t);
        if idx == 0 { return self.keyframes[0].speed; }
        if idx >= n { return self.keyframes[n-1].speed; }
        let k0 = &self.keyframes[idx-1];
        let k1 = &self.keyframes[idx];
        let dt = k1.t - k0.t;
        if dt.abs() < EPSILON { return k0.speed; }
        let u = (t - k0.t) / dt;
        // Cubic Hermite
        let u2 = u * u;
        let u3 = u2 * u;
        let h00 =  2.0*u3 - 3.0*u2 + 1.0;
        let h10 =     u3  - 2.0*u2 + u;
        let h01 = -2.0*u3 + 3.0*u2;
        let h11 =     u3  -     u2;
        k0.speed * h00 + k0.tan_out * h10 * dt
            + k1.speed * h01 + k1.tan_in * h11 * dt
    }

    /// Integrate speed over [0, t] to get arc-length
    pub fn integrate_to(&self, t: f32, steps: usize) -> f32 {
        let dt = t / steps.max(1) as f32;
        let mut s = 0.0_f32;
        for i in 0..steps {
            let t0 = i as f32 * dt;
            let t1 = (i + 1) as f32 * dt;
            s += (self.evaluate(t0) + self.evaluate(t1)) * 0.5 * dt;
        }
        s
    }
}

// ============================================================
// SIGNAL TRACK (for game events along spline)
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineSignal {
    pub t: f32,       // position along spline [0,1]
    pub kind: String,
    pub data: HashMap<String, f32>,
    pub triggered: bool,
}

impl SplineSignal {
    pub fn new(t: f32, kind: &str) -> Self {
        SplineSignal { t, kind: kind.to_string(), data: HashMap::new(), triggered: false }
    }

    pub fn with_data(mut self, key: &str, val: f32) -> Self {
        self.data.insert(key.to_string(), val);
        self
    }
}

#[derive(Clone, Debug)]
pub struct SplineSignalTrack {
    pub spline_id: u64,
    pub signals: Vec<SplineSignal>,
    pub loop_signals: bool,
}

impl SplineSignalTrack {
    pub fn new(spline_id: u64) -> Self {
        SplineSignalTrack { spline_id, signals: Vec::new(), loop_signals: false }
    }

    pub fn add_signal(&mut self, t: f32, kind: &str) {
        let sig = SplineSignal::new(t, kind);
        let idx = self.signals.partition_point(|s| s.t < t);
        self.signals.insert(idx, sig);
    }

    /// Poll for signals triggered as t passes from prev_t to cur_t
    pub fn poll(&mut self, prev_t: f32, cur_t: f32) -> Vec<SplineSignal> {
        let mut triggered = Vec::new();
        for sig in &mut self.signals {
            if sig.t > prev_t && sig.t <= cur_t && !sig.triggered {
                sig.triggered = true;
                triggered.push(sig.clone());
            }
        }
        if self.loop_signals && cur_t >= 1.0 {
            for sig in &mut self.signals {
                sig.triggered = false;
            }
        }
        triggered
    }

    pub fn reset(&mut self) {
        for sig in &mut self.signals {
            sig.triggered = false;
        }
    }
}

// ============================================================
// SPLINE LOD MANAGER
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineLodLevel {
    pub max_camera_distance: f32,
    pub resolution: usize,       // number of segments for mesh gen
    pub show_debug: bool,
}

#[derive(Clone, Debug)]
pub struct SplineLodManager {
    pub levels: Vec<SplineLodLevel>,
}

impl SplineLodManager {
    pub fn new() -> Self {
        SplineLodManager {
            levels: vec![
                SplineLodLevel { max_camera_distance: 20.0,  resolution: 128, show_debug: true  },
                SplineLodLevel { max_camera_distance: 50.0,  resolution: 64,  show_debug: false },
                SplineLodLevel { max_camera_distance: 150.0, resolution: 32,  show_debug: false },
                SplineLodLevel { max_camera_distance: f32::MAX, resolution: 16, show_debug: false },
            ],
        }
    }

    pub fn select_level(&self, camera_dist: f32) -> &SplineLodLevel {
        self.levels.iter()
            .find(|l| camera_dist <= l.max_camera_distance)
            .unwrap_or(self.levels.last().unwrap())
    }

    pub fn resolution_at_distance(&self, dist: f32) -> usize {
        self.select_level(dist).resolution
    }
}

// ============================================================
// TESTING / EXAMPLE USAGE
// ============================================================

pub fn example_build_roller_coaster() -> SplineEditor {
    let mut editor = SplineEditor::new();

    // Create a looping roller coaster track
    let loop_pts = vec![
        Vec3::new(  0.0, 0.0,   0.0),
        Vec3::new( 20.0, 5.0,   0.0),
        Vec3::new( 40.0,15.0,   0.0),
        Vec3::new( 50.0,15.0,  20.0),
        Vec3::new( 40.0,25.0,  40.0),
        Vec3::new( 20.0,30.0,  40.0),
        Vec3::new(  0.0,30.0,  20.0),
        Vec3::new(-10.0,15.0,   0.0),
        Vec3::new(  0.0, 0.0,   0.0), // back to start
    ];
    let spline_id = editor.create_catmull_spline(loop_pts, "RollerCoaster");
    editor.toggle_closed_spline(spline_id);

    // Add a rail track
    editor.create_rail_track(spline_id, DEFAULT_RAIL_GAUGE);

    // Set mesh section to a rail profile
    editor.mesh_section = CrossSection::i_beam(0.15, 0.2, 0.03, 0.02);
    editor.mesh_resolution = 128;
    editor.generate_lod_mesh(spline_id, 32, 256);

    // Add a physics ball
    editor.add_constrained_object(spline_id, 0.0, 5.0);

    editor
}

pub fn example_camera_path() -> (SplineEditor, u64) {
    let mut editor = SplineEditor::new();

    let cam_pts = vec![
        Vec3::new( 0.0, 3.0,  10.0),
        Vec3::new( 5.0, 4.0,   5.0),
        Vec3::new(10.0, 3.5,   0.0),
        Vec3::new(10.0, 3.0,  -5.0),
        Vec3::new( 5.0, 2.5, -10.0),
        Vec3::new( 0.0, 2.0,  -8.0),
    ];
    let spline_id = editor.create_catmull_spline(cam_pts, "CameraPath");
    let rail_id = editor.create_camera_rail(spline_id).unwrap();
    (editor, rail_id)
}

// ============================================================
// ADDITIONAL MATH: SPLINE TORSION INTEGRAL (total torsion)
// ============================================================

pub fn total_torsion(
    frenet_fn: &dyn Fn(f32) -> FrenetFrame,
    steps: usize,
) -> f32 {
    let dt = 1.0 / steps as f32;
    let mut total = 0.0_f32;
    for i in 0..steps {
        let t = i as f32 * dt;
        let frame = frenet_fn(t + dt * 0.5);
        total += frame.torsion.abs() * dt;
    }
    total
}

/// Total absolute curvature (integral of |κ| ds)
pub fn total_absolute_curvature(
    curvature_fn: &dyn Fn(f32) -> f32,
    deriv_fn: &dyn Fn(f32) -> Vec3,
    steps: usize,
) -> f32 {
    let dt = 1.0 / steps as f32;
    let mut total = 0.0_f32;
    for i in 0..steps {
        let t = (i as f32 + 0.5) * dt;
        let kappa = curvature_fn(t);
        let speed = deriv_fn(t).length();
        total += kappa * speed * dt;
    }
    total
}

/// Turning number of a closed planar curve
pub fn turning_number(pts: &[Vec2]) -> i32 {
    let n = pts.len();
    if n < 3 { return 0; }
    let mut angle_sum = 0.0_f32;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let c = pts[(i + 2) % n];
        let ab = b - a;
        let bc = c - b;
        angle_sum += (ab.x * bc.y - ab.y * bc.x).atan2(ab.dot(bc));
    }
    (angle_sum / std::f32::consts::TAU).round() as i32
}

// ============================================================
// CURVATURE FLOW (curve shortening flow)
// ============================================================

pub fn curvature_flow_step(pts: &[Vec3], dt: f32) -> Vec<Vec3> {
    let n = pts.len();
    if n < 3 { return pts.to_vec(); }
    let mut out = pts.to_vec();
    for i in 1..n - 1 {
        let prev = pts[i - 1];
        let cur  = pts[i];
        let next = pts[i + 1];
        // Discrete Laplacian
        let laplacian = prev + next - 2.0 * cur;
        out[i] = cur + laplacian * dt;
    }
    out
}

pub fn run_curvature_flow(pts: &[Vec3], iterations: usize, dt: f32) -> Vec<Vec3> {
    let mut result = pts.to_vec();
    for _ in 0..iterations {
        result = curvature_flow_step(&result, dt);
    }
    result
}

// ============================================================
// SPLINE FRAME EXPORT (for use with animation systems)
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineFrameExport {
    pub time: f32,
    pub position: Vec3,
    pub rotation: Quat,
    pub tangent: Vec3,
    pub curvature: f32,
    pub arc_length: f32,
}

pub fn export_spline_frames(
    spline: &CatmullRomSpline,
    duration: f32,
    fps: f32,
    speed: f32,
) -> Vec<SplineFrameExport> {
    let total_length = spline.total_arc_length();
    let n_frames = (duration * fps) as usize + 1;
    let mut frames = Vec::with_capacity(n_frames);
    for i in 0..n_frames {
        let time = i as f32 / fps;
        let arc_s = (time * speed).min(total_length);
        let t = spline.t_at_arc_length(arc_s);
        let pos = spline.evaluate(t);
        let tan = safe_normalize(spline.evaluate_derivative(t));
        let frame = spline.frenet_frame_at(t);
        let rot = Quat::from_mat4(&frame.to_matrix());
        frames.push(SplineFrameExport {
            time,
            position: pos,
            rotation: rot,
            tangent: tan,
            curvature: frame.curvature,
            arc_length: arc_s,
        });
    }
    frames
}

// ============================================================
// SPLINE WINDING / HELIX GENERATOR
// ============================================================

pub fn generate_helix(
    center: Vec3,
    radius: f32,
    pitch: f32,       // height per revolution
    turns: f32,
    n_pts: usize,
) -> Vec<Vec3> {
    (0..n_pts).map(|i| {
        let t = i as f32 / (n_pts - 1).max(1) as f32;
        let angle = t * turns * std::f32::consts::TAU;
        Vec3::new(
            center.x + angle.cos() * radius,
            center.y + t * turns * pitch,
            center.z + angle.sin() * radius,
        )
    }).collect()
}

pub fn generate_toroidal_helix(
    big_radius: f32,
    small_radius: f32,
    p: u32, // wraps p times around big axis
    q: u32, // wraps q times around small axis
    n_pts: usize,
) -> Vec<Vec3> {
    (0..n_pts).map(|i| {
        let t = i as f32 / (n_pts - 1).max(1) as f32 * std::f32::consts::TAU;
        let phi = t * p as f32;
        let theta = t * q as f32;
        let r = big_radius + small_radius * theta.cos();
        Vec3::new(
            r * phi.cos(),
            small_radius * theta.sin(),
            r * phi.sin(),
        )
    }).collect()
}

// ============================================================
// ARC LENGTH INTEGRATION TESTS (internal verification)
// ============================================================

fn verify_arc_length_integration() -> bool {
    // A circle of radius r should have arc length 2πr
    let r = 5.0_f32;
    let circle_pos = |t: f32| Vec3::new(
        r * (t * std::f32::consts::TAU).cos(),
        0.0,
        r * (t * std::f32::consts::TAU).sin(),
    );
    let table = build_arc_length_table(1024, &circle_pos);
    let measured = table.last().map(|e| e.1).unwrap_or(0.0);
    let expected = std::f32::consts::TAU * r;
    (measured - expected).abs() < 0.01 * expected // within 1%
}

// ============================================================
// PARALLEL TRANSPORT CHAIN: whole-spline PT frame sequence
// ============================================================

pub fn build_parallel_transport_frames(
    pos_fn:  &dyn Fn(f32) -> Vec3,
    tang_fn: &dyn Fn(f32) -> Vec3,
    steps:   usize,
) -> Vec<ParallelTransportFrame> {
    let mut frames = Vec::with_capacity(steps + 1);
    let p0 = pos_fn(0.0);
    let t0 = tang_fn(0.0);
    frames.push(ParallelTransportFrame::initial(p0, t0));
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let p = pos_fn(t);
        let tang = safe_normalize(tang_fn(t));
        let prev = frames.last().unwrap().clone();
        frames.push(ParallelTransportFrame::transport(&prev, p, tang));
    }
    frames
}

// ============================================================
// KNOT VECTOR UTILITIES
// ============================================================

pub fn knot_vector_uniform(n: usize, k: usize) -> Vec<f32> {
    let m = n + k + 1;
    (0..m).map(|i| i as f32 / (m - 1) as f32).collect()
}

pub fn knot_vector_clamped(n: usize, k: usize) -> Vec<f32> {
    let m = n + k + 1;
    let mut v = Vec::with_capacity(m);
    for i in 0..m {
        if i < k + 1 { v.push(0.0); }
        else if i > n { v.push(1.0); }
        else { v.push((i - k) as f32 / (n - k) as f32); }
    }
    v
}

pub fn knot_vector_periodic(n: usize, k: usize) -> Vec<f32> {
    let m = n + k + 1;
    (0..m).map(|i| (i as f32 - k as f32) / (n - k + 1) as f32).collect()
}

// ============================================================
// WHOLE-EDITOR UPDATE TICK
// ============================================================

impl SplineEditor {
    pub fn update(&mut self, dt: f32) {
        self.update_physics(dt);
        for chain in &mut self.chains {
            chain.update_offset(dt * 0.5);
        }
        if self.show_debug {
            self.update_debug_viz();
        }
    }

    pub fn stats(&self) -> SplineEditorStats {
        let total_verts: usize = self.generated_meshes.values()
            .map(|m| m.vertex_count()).sum();
        let total_tris: usize = self.generated_meshes.values()
            .map(|m| m.triangle_count()).sum();
        SplineEditorStats {
            spline_count:  self.spline_count(),
            rail_count:    self.rail_tracks.len(),
            camera_rail_count: self.camera_rails.len(),
            constrained_objects: self.constrained_objects.len(),
            chain_count:   self.chains.len(),
            mesh_count:    self.generated_meshes.len(),
            total_vertices: total_verts,
            total_triangles: total_tris,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SplineEditorStats {
    pub spline_count: usize,
    pub rail_count: usize,
    pub camera_rail_count: usize,
    pub constrained_objects: usize,
    pub chain_count: usize,
    pub mesh_count: usize,
    pub total_vertices: usize,
    pub total_triangles: usize,
}

// ============================================================
// MULTI-SEGMENT BSPLINE FITTING
// ============================================================

pub struct BSplineFitter {
    pub degree: usize,
    pub max_control_points: usize,
    pub tolerance: f32,
}

impl BSplineFitter {
    pub fn new(degree: usize, tolerance: f32) -> Self {
        BSplineFitter { degree, max_control_points: 32, tolerance }
    }

    pub fn fit(&self, pts: &[Vec3]) -> BSpline {
        let n = pts.len().min(self.max_control_points);
        // Use Greville abscissae for chord-length parameterized fit
        let params = chord_length_params(pts);
        let mut cps = Vec::with_capacity(n);
        // Simple approach: select n control points spaced evenly in parameter
        for i in 0..n {
            let t = i as f32 / (n - 1).max(1) as f32;
            let idx = (t * (pts.len() - 1) as f32) as usize;
            cps.push(pts[idx.min(pts.len() - 1)]);
        }
        let mut spline = BSpline::new(cps, self.degree, false);
        // Iterative refinement: move control points to minimize least-squares error
        for _iter in 0..self.max_control_points {
            let mut error = 0.0_f32;
            for (&t, &p) in params.iter().zip(pts.iter()) {
                let q = spline.evaluate(t);
                error += (q - p).length_squared();
            }
            if error.sqrt() < self.tolerance { break; }
            // Gradient descent step
            let n_cps = spline.control_points.len();
            for (idx, cp) in spline.control_points.iter_mut().enumerate() {
                let cp_t = idx as f32 / (n_cps - 1).max(1) as f32;
                let nearby: Vec3 = params.iter().zip(pts.iter())
                    .filter(|(&t, _)| (t - cp_t).abs() < 0.1)
                    .map(|(_, &p)| p)
                    .fold(Vec3::ZERO, |a, b| a + b);
                let count = params.iter()
                    .filter(|&&t| (t - cp_t).abs() < 0.1)
                    .count();
                if count > 0 {
                    let target = nearby / count as f32;
                    *cp = lerp_vec3(*cp, target, 0.1);
                }
            }
            spline.rebuild_arc_length_table();
        }
        spline
    }
}

// ============================================================
// SURFACE OF REVOLUTION ALONG SPLINE
// ============================================================

pub fn surface_of_revolution(
    profile_pts: &[Vec2],  // 2D profile in (r, z) space
    axis: Vec3,
    n_revolutions: usize,
) -> SplineMesh {
    let n_profile = profile_pts.len();
    let n_angular  = n_revolutions;
    let mut mesh   = SplineMesh::new();

    for j in 0..=n_angular {
        let angle = j as f32 / n_angular as f32 * std::f32::consts::TAU;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        for (i, &pt) in profile_pts.iter().enumerate() {
            let r = pt.x;
            let z = pt.y;
            // Rotate r around axis
            let right = safe_normalize(axis.cross(Vec3::Y));
            let up    = safe_normalize(axis.cross(right));
            let world = axis * z + right * (r * cos_a) + up * (r * sin_a);
            let normal = safe_normalize(right * cos_a + up * sin_a);
            let u = j as f32 / n_angular as f32;
            let v = i as f32 / (n_profile - 1).max(1) as f32;
            mesh.vertices.push(world);
            mesh.normals.push(normal);
            mesh.uvs.push(Vec2::new(u, v));
            mesh.tangents.push(axis);
        }
    }

    for j in 0..n_angular {
        for i in 0..n_profile.saturating_sub(1) {
            let a = (j * n_profile + i) as u32;
            let b = (j * n_profile + i + 1) as u32;
            let c = ((j + 1) * n_profile + i) as u32;
            let d = ((j + 1) * n_profile + i + 1) as u32;
            mesh.indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }

    mesh
}

// ============================================================
// SPLINE ANIMATION SAMPLING (baking to keyframes)
// ============================================================

#[derive(Clone, Debug)]
pub struct BakedSplineAnimation {
    pub positions: Vec<Vec3>,
    pub rotations: Vec<Quat>,
    pub times:     Vec<f32>,
    pub fps:       f32,
}

impl BakedSplineAnimation {
    pub fn bake(spline: &CatmullRomSpline, fps: f32, duration: f32, speed: f32) -> Self {
        let frames = export_spline_frames(spline, duration, fps, speed);
        BakedSplineAnimation {
            positions: frames.iter().map(|f| f.position).collect(),
            rotations: frames.iter().map(|f| f.rotation).collect(),
            times:     frames.iter().map(|f| f.time).collect(),
            fps,
        }
    }

    pub fn sample_position(&self, time: f32) -> Vec3 {
        if self.times.is_empty() { return Vec3::ZERO; }
        let idx = self.times.partition_point(|&t| t <= time);
        if idx == 0 { return self.positions[0]; }
        if idx >= self.positions.len() { return *self.positions.last().unwrap(); }
        let t0 = self.times[idx - 1];
        let t1 = self.times[idx];
        let frac = if (t1 - t0).abs() < EPSILON { 0.0 } else { (time - t0) / (t1 - t0) };
        lerp_vec3(self.positions[idx - 1], self.positions[idx], frac)
    }

    pub fn sample_rotation(&self, time: f32) -> Quat {
        if self.times.is_empty() { return Quat::IDENTITY; }
        let idx = self.times.partition_point(|&t| t <= time);
        if idx == 0 { return self.rotations[0]; }
        if idx >= self.rotations.len() { return *self.rotations.last().unwrap(); }
        let t0 = self.times[idx - 1];
        let t1 = self.times[idx];
        let frac = if (t1 - t0).abs() < EPSILON { 0.0 } else { (time - t0) / (t1 - t0) };
        self.rotations[idx - 1].slerp(self.rotations[idx], frac)
    }
}

// ============================================================
// SIGNED DISTANCE FIELD ALONG SPLINE (capsule approximation)
// ============================================================

pub fn sdf_spline_capsule(
    point: Vec3,
    pos_fn: &dyn Fn(f32) -> Vec3,
    total_length: f32,
    radius: f32,
    steps: usize,
) -> f32 {
    let mut min_dist = f32::MAX;
    let mut prev = pos_fn(0.0);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let cur = pos_fn(t);
        // SDF to segment
        let seg = cur - prev;
        let seg_len_sq = seg.length_squared();
        let t_seg = if seg_len_sq < EPSILON { 0.0 }
                    else { ((point - prev).dot(seg) / seg_len_sq).clamp(0.0, 1.0) };
        let closest = prev + seg * t_seg;
        let dist = (point - closest).length() - radius;
        if dist < min_dist { min_dist = dist; }
        prev = cur;
    }
    min_dist
}

// ============================================================
// SPLINE GROUNDING (project spline onto terrain heightmap)
// ============================================================

pub fn ground_spline_to_terrain(
    pts: &mut [Vec3],
    height_fn: &dyn Fn(f32, f32) -> f32,
    offset: f32,
) {
    for p in pts.iter_mut() {
        let ground = height_fn(p.x, p.z);
        p.y = p.y.max(ground + offset);
    }
}

// ============================================================
// UNIT TEST STUBS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catmull_rom_endpoints() {
        let pts = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
        ];
        let s = CatmullRomSpline::new(pts, 0.5, false);
        let start = s.evaluate(0.0);
        let end   = s.evaluate(1.0);
        assert!((start.x - 0.0).abs() < 0.1, "Start x should be near 0");
        assert!((end.x   - 3.0).abs() < 0.1, "End x should be near 3");
    }

    #[test]
    fn test_bezier_de_casteljau_endpoints() {
        let p0 = Vec3::new(0.0, 0.0, 0.0);
        let p1 = Vec3::new(1.0, 2.0, 0.0);
        let p2 = Vec3::new(2.0, 2.0, 0.0);
        let p3 = Vec3::new(3.0, 0.0, 0.0);
        let at0 = CubicBezierSpline::de_casteljau(p0, p1, p2, p3, 0.0);
        let at1 = CubicBezierSpline::de_casteljau(p0, p1, p2, p3, 1.0);
        assert!((at0 - p0).length() < EPSILON);
        assert!((at1 - p3).length() < EPSILON);
    }

    #[test]
    fn test_arc_length_circle() {
        assert!(verify_arc_length_integration(), "Circle arc length should be within 1%");
    }

    #[test]
    fn test_arc_length_inverse() {
        let pts = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(3.0, 4.0, 0.0), // length 5
        ];
        let s = CatmullRomSpline::new(pts, 0.5, false);
        let total = s.total_arc_length();
        let t_half = s.t_at_arc_length(total * 0.5);
        assert!((t_half - 0.5).abs() < 0.05, "Midpoint should be near t=0.5");
    }

    #[test]
    fn test_bspline_partition_of_unity() {
        let pts: Vec<Vec3> = (0..6).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect();
        let s = BSpline::new(pts, 3, false);
        // Sum of basis functions should be ~1 everywhere
        for j in 0..10 {
            let t = 0.05 + j as f32 * 0.09;
            let sum: f32 = (0..s.control_points.len())
                .map(|i| s.basis(i, s.degree, t))
                .sum();
            assert!((sum - 1.0).abs() < 0.01, "B-spline partition of unity failed at t={}", t);
        }
    }

    #[test]
    fn test_frenet_frame_orthonormality() {
        let pts = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.5, 0.0),
            Vec3::new(2.0, 0.0, 0.5),
            Vec3::new(3.0, 0.0, 0.0),
        ];
        let s = CatmullRomSpline::new(pts, 0.5, false);
        for i in 1..9 {
            let t = i as f32 / 9.0;
            let frame = s.frenet_frame_at(t);
            let tt = frame.tangent.dot(frame.tangent);
            let nn = frame.normal.dot(frame.normal);
            let tn = frame.tangent.dot(frame.normal);
            assert!((tt - 1.0).abs() < 0.01, "Tangent not unit");
            assert!((nn - 1.0).abs() < 0.01, "Normal not unit");
            assert!(tn.abs() < 0.01, "T·N not zero");
        }
    }

    #[test]
    fn test_undo_redo() {
        let mut editor = SplineEditor::new();
        let id = editor.create_catmull_spline(
            vec![Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y],
            "Test"
        );
        editor.move_control_point(id, 0, Vec3::new(1.0, 0.0, 0.0));
        let pos_after = editor.catmull_splines[&id].control_points[0].position;
        assert!((pos_after.x - 1.0).abs() < EPSILON);
        editor.undo();
        let pos_undone = editor.catmull_splines[&id].control_points[0].position;
        assert!(pos_undone.x.abs() < EPSILON, "Undo should restore position");
    }

    #[test]
    fn test_hermite_tangent_continuity() {
        let pts = vec![
            (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
            (Vec3::new(2.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
            (Vec3::new(4.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
        ];
        let s = HermiteSpline::new(pts);
        // Check derivative at segment boundary matches specified tangent
        let d = s.eval_segment_derivative(0, 1.0);
        // Should be proportional to the end tangent
        assert!(d.length() > EPSILON, "Derivative at boundary should be non-zero");
    }
}

// ============================================================
// SPLINE WARP DEFORMER
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineWarpDeformer {
    pub axis_spline_id:  u64,
    pub falloff_curve:   Vec<(f32, f32)>,
    pub world_up:        Vec3,
}

impl SplineWarpDeformer {
    pub fn new(axis_spline_id: u64) -> Self {
        SplineWarpDeformer {
            axis_spline_id,
            falloff_curve: vec![(0.0, 1.0), (1.0, 0.0)],
            world_up: Vec3::Y,
        }
    }

    pub fn falloff_at(&self, dist: f32) -> f32 {
        let n = self.falloff_curve.len();
        if n == 0 { return 1.0; }
        if n == 1 { return self.falloff_curve[0].1; }
        let idx = self.falloff_curve.partition_point(|&(d, _)| d <= dist);
        if idx == 0 { return self.falloff_curve[0].1; }
        if idx >= n { return self.falloff_curve[n-1].1; }
        let (d0, w0) = self.falloff_curve[idx-1];
        let (d1, w1) = self.falloff_curve[idx];
        let frac = if (d1 - d0).abs() < EPSILON { 0.0 } else { (dist - d0) / (d1 - d0) };
        lerp(w0, w1, frac)
    }

    pub fn warp_point(&self, point: Vec3, spline: &CatmullRomSpline, radius: f32) -> Vec3 {
        let (t, closest) = spline.nearest_point(point);
        let dist = (point - closest).length();
        if dist > radius { return point; }
        let weight = self.falloff_at(dist / radius.max(EPSILON));
        let frame  = spline.frenet_frame_at(t);
        let local   = point - closest;
        let local_n = local.dot(frame.normal);
        let local_b = local.dot(frame.binormal);
        let twist_angle = frame.torsion * weight * 0.1;
        let cos_t = twist_angle.cos();
        let sin_t = twist_angle.sin();
        let new_n = local_n * cos_t - local_b * sin_t;
        let new_b = local_n * sin_t + local_b * cos_t;
        let warped_local = frame.normal * new_n + frame.binormal * new_b;
        lerp_vec3(point, closest + warped_local, weight)
    }

    pub fn warp_mesh(&self, verts: &mut [Vec3], spline: &CatmullRomSpline, radius: f32) {
        for v in verts.iter_mut() {
            *v = self.warp_point(*v, spline, radius);
        }
    }
}

// ============================================================
// ROAD PROFILE
// ============================================================

#[derive(Clone, Debug)]
pub struct RoadProfile {
    pub lane_width:      f32,
    pub lane_count:      u32,
    pub shoulder_width:  f32,
    pub curb_height:     f32,
    pub median_width:    f32,
    pub has_sidewalk:    bool,
    pub sidewalk_width:  f32,
    pub sidewalk_height: f32,
}

impl RoadProfile {
    pub fn two_lane_road() -> Self {
        RoadProfile {
            lane_width: 3.7, lane_count: 2, shoulder_width: 1.2,
            curb_height: 0.15, median_width: 0.0,
            has_sidewalk: true, sidewalk_width: 2.0, sidewalk_height: 0.15,
        }
    }

    pub fn highway() -> Self {
        RoadProfile {
            lane_width: 3.7, lane_count: 6, shoulder_width: 3.0,
            curb_height: 0.0, median_width: 4.0,
            has_sidewalk: false, sidewalk_width: 0.0, sidewalk_height: 0.0,
        }
    }

    pub fn total_width(&self) -> f32 {
        self.lane_width * self.lane_count as f32
            + self.shoulder_width * 2.0
            + self.median_width
            + if self.has_sidewalk { self.sidewalk_width * 2.0 } else { 0.0 }
    }

    pub fn generate_cross_section(&self) -> CrossSection {
        let hw = self.total_width() * 0.5;
        let road_hw = (self.lane_width * self.lane_count as f32 * 0.5) + self.shoulder_width;
        let mut pts = Vec::new();
        pts.push(Vec2::new(-hw, 0.0));
        if self.has_sidewalk {
            pts.push(Vec2::new(-hw, self.sidewalk_height));
            pts.push(Vec2::new(-road_hw - self.sidewalk_width, self.sidewalk_height));
        }
        pts.push(Vec2::new(-road_hw, self.curb_height));
        pts.push(Vec2::new(-road_hw, 0.0));
        pts.push(Vec2::new( road_hw, 0.0));
        pts.push(Vec2::new( road_hw, self.curb_height));
        if self.has_sidewalk {
            pts.push(Vec2::new(road_hw + self.sidewalk_width, self.sidewalk_height));
            pts.push(Vec2::new(hw, self.sidewalk_height));
        }
        pts.push(Vec2::new(hw, 0.0));
        CrossSection { points: pts, closed: false }
    }
}

// ============================================================
// ROAD MARKING
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum RoadMarkingKind {
    Solid, Dashed, DoubleSolid, StopLine, Crosswalk,
}

#[derive(Clone, Debug)]
pub struct RoadMarking {
    pub kind:     RoadMarkingKind,
    pub offset:   f32,
    pub t_start:  f32,
    pub t_end:    f32,
    pub dash_len: f32,
    pub dash_gap: f32,
    pub color:    Vec4,
}

#[derive(Clone, Debug)]
pub struct RoadSegment {
    pub spline_id: u64,
    pub profile:   RoadProfile,
    pub mesh_id:   Option<u64>,
    pub markings:  Vec<RoadMarking>,
}

impl RoadSegment {
    pub fn new(spline_id: u64, profile: RoadProfile) -> Self {
        RoadSegment { spline_id, profile, mesh_id: None, markings: Vec::new() }
    }

    pub fn add_center_line(&mut self) {
        self.markings.push(RoadMarking {
            kind: RoadMarkingKind::Dashed, offset: 0.0,
            t_start: 0.0, t_end: 1.0, dash_len: 3.0, dash_gap: 9.0,
            color: Vec4::new(1.0, 1.0, 0.0, 1.0),
        });
    }

    pub fn add_edge_lines(&mut self) {
        let hw = (self.profile.lane_width * self.profile.lane_count as f32 * 0.5) + self.profile.shoulder_width;
        for &side in &[-hw, hw] {
            self.markings.push(RoadMarking {
                kind: RoadMarkingKind::Solid, offset: side,
                t_start: 0.0, t_end: 1.0, dash_len: 0.0, dash_gap: 0.0,
                color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            });
        }
    }

    pub fn marking_line_segments(&self, marking_idx: usize, spline: &CatmullRomSpline) -> Vec<(Vec3, Vec3)> {
        let m = &self.markings[marking_idx];
        let total = spline.total_arc_length();
        let mut result = Vec::new();
        match m.kind {
            RoadMarkingKind::Solid | RoadMarkingKind::DoubleSolid => {
                let steps = 64usize;
                for i in 0..steps {
                    let t0 = lerp(m.t_start, m.t_end, i as f32 / steps as f32);
                    let t1 = lerp(m.t_start, m.t_end, (i+1) as f32 / steps as f32);
                    let f0 = spline.frenet_frame_at(t0);
                    let f1 = spline.frenet_frame_at(t1);
                    result.push((f0.position + f0.normal * m.offset, f1.position + f1.normal * m.offset));
                }
            }
            RoadMarkingKind::Dashed => {
                let cycle = m.dash_len + m.dash_gap;
                let mut s = m.t_start * total;
                let s_end = m.t_end * total;
                while s < s_end {
                    let s_end_dash = (s + m.dash_len).min(s_end);
                    let t0 = spline.t_at_arc_length(s);
                    let t1 = spline.t_at_arc_length(s_end_dash);
                    let steps = 8usize;
                    for i in 0..steps {
                        let ta = lerp(t0, t1, i as f32 / steps as f32);
                        let tb = lerp(t0, t1, (i+1) as f32 / steps as f32);
                        let fa = spline.frenet_frame_at(ta);
                        let fb = spline.frenet_frame_at(tb);
                        result.push((fa.position + fa.normal * m.offset, fb.position + fb.normal * m.offset));
                    }
                    s += cycle;
                }
            }
            _ => {}
        }
        result
    }
}

// ============================================================
// SPLINE INTERSECTION GRAPH
// ============================================================

#[derive(Clone, Debug)]
pub struct IntersectionPoint {
    pub position:    Vec3,
    pub spline_ids:  Vec<u64>,
    pub t_values:    Vec<f32>,
    pub is_junction: bool,
}

#[derive(Clone, Debug)]
pub struct SplineIntersectionGraph {
    pub intersections: Vec<IntersectionPoint>,
}

impl SplineIntersectionGraph {
    pub fn new() -> Self { SplineIntersectionGraph { intersections: Vec::new() } }

    pub fn compute_all(splines: &HashMap<u64, CatmullRomSpline>, tol: f32) -> Self {
        let mut graph = Self::new();
        let ids: Vec<u64> = splines.keys().cloned().collect();
        for i in 0..ids.len() {
            for j in i+1..ids.len() {
                let sa = &splines[&ids[i]];
                let sb = &splines[&ids[j]];
                let hits = intersect_spline_spline(
                    &|t| sa.evaluate(t),
                    &|t| sb.evaluate(t),
                    24, tol,
                );
                for hit in hits {
                    graph.intersections.push(IntersectionPoint {
                        position:   hit.point_a,
                        spline_ids: vec![ids[i], ids[j]],
                        t_values:   vec![hit.t_a, hit.t_b],
                        is_junction: true,
                    });
                }
            }
        }
        graph
    }

    pub fn junctions_near(&self, pos: Vec3, radius: f32) -> Vec<&IntersectionPoint> {
        self.intersections.iter()
            .filter(|p| (p.position - pos).length() <= radius)
            .collect()
    }
}

// ============================================================
// VOLUMETRIC SPLINE REGION
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineVolume {
    pub spline_id:   u64,
    pub radius:      f32,
    pub taper_start: f32,
    pub taper_end:   f32,
}

impl SplineVolume {
    pub fn new(spline_id: u64, radius: f32) -> Self {
        SplineVolume { spline_id, radius, taper_start: 1.0, taper_end: 1.0 }
    }

    pub fn radius_at(&self, t: f32) -> f32 {
        self.radius * lerp(self.taper_start, self.taper_end, t)
    }

    pub fn contains(&self, point: Vec3, spline: &CatmullRomSpline) -> bool {
        let (t, closest) = spline.nearest_point(point);
        (point - closest).length() <= self.radius_at(t)
    }

    pub fn density_at(&self, point: Vec3, spline: &CatmullRomSpline) -> f32 {
        let (t, closest) = spline.nearest_point(point);
        let r = self.radius_at(t);
        let dist = (point - closest).length();
        if dist >= r { 0.0 } else { 1.0 - dist / r }
    }

    pub fn surface_sdf(&self, point: Vec3, spline: &CatmullRomSpline) -> f32 {
        let (t, closest) = spline.nearest_point(point);
        let r = self.radius_at(t);
        (point - closest).length() - r
    }
}

// ============================================================
// SPLINE GRADIENT (color along spline)
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineGradient {
    pub stops: Vec<(f32, Vec4)>,
}

impl SplineGradient {
    pub fn new() -> Self { SplineGradient { stops: Vec::new() } }

    pub fn add_stop(mut self, t: f32, color: Vec4) -> Self {
        let idx = self.stops.partition_point(|s| s.0 < t);
        self.stops.insert(idx, (t, color));
        self
    }

    pub fn evaluate(&self, t: f32) -> Vec4 {
        let n = self.stops.len();
        if n == 0 { return Vec4::ONE; }
        if n == 1 { return self.stops[0].1; }
        let idx = self.stops.partition_point(|s| s.0 <= t);
        if idx == 0 { return self.stops[0].1; }
        if idx >= n { return self.stops[n-1].1; }
        let (t0, c0) = self.stops[idx-1];
        let (t1, c1) = self.stops[idx];
        let frac = if (t1 - t0).abs() < EPSILON { 0.0 } else { (t - t0) / (t1 - t0) };
        Self::lerp_vec4(c0, c1, frac)
    }

    fn lerp_vec4(a: Vec4, b: Vec4, t: f32) -> Vec4 { a + (b - a) * t }

    pub fn rainbow() -> Self {
        SplineGradient::new()
            .add_stop(0.0,   Vec4::new(1.0, 0.0, 0.0, 1.0))
            .add_stop(0.166, Vec4::new(1.0, 0.5, 0.0, 1.0))
            .add_stop(0.333, Vec4::new(1.0, 1.0, 0.0, 1.0))
            .add_stop(0.5,   Vec4::new(0.0, 1.0, 0.0, 1.0))
            .add_stop(0.666, Vec4::new(0.0, 0.0, 1.0, 1.0))
            .add_stop(0.833, Vec4::new(0.5, 0.0, 1.0, 1.0))
            .add_stop(1.0,   Vec4::new(1.0, 0.0, 1.0, 1.0))
    }
}

// ============================================================
// ANIMATED SPLINE
// ============================================================

#[derive(Clone, Debug)]
pub struct AnimatedSplineKeyframe {
    pub time:           f32,
    pub control_points: Vec<Vec3>,
}

#[derive(Clone, Debug)]
pub struct AnimatedSpline {
    pub spline_id: u64,
    pub keyframes: Vec<AnimatedSplineKeyframe>,
    pub loop_anim: bool,
    pub duration:  f32,
}

impl AnimatedSpline {
    pub fn new(spline_id: u64, duration: f32) -> Self {
        AnimatedSpline { spline_id, keyframes: Vec::new(), loop_anim: true, duration }
    }

    pub fn add_keyframe(&mut self, time: f32, points: Vec<Vec3>) {
        let idx = self.keyframes.partition_point(|k| k.time < time);
        self.keyframes.insert(idx, AnimatedSplineKeyframe { time, control_points: points });
    }

    pub fn evaluate_points(&self, time: f32) -> Option<Vec<Vec3>> {
        let t = if self.loop_anim { time % self.duration.max(EPSILON) } else { time.min(self.duration) };
        let n = self.keyframes.len();
        if n == 0 { return None; }
        if n == 1 { return Some(self.keyframes[0].control_points.clone()); }
        let idx = self.keyframes.partition_point(|k| k.time <= t);
        let k0 = &self.keyframes[(idx.saturating_sub(1)).min(n-1)];
        let k1 = &self.keyframes[idx.min(n-1)];
        let dt = k1.time - k0.time;
        let frac = if dt.abs() < EPSILON { 0.0 } else { (t - k0.time) / dt };
        let n_pts = k0.control_points.len().min(k1.control_points.len());
        Some((0..n_pts).map(|i| lerp_vec3(k0.control_points[i], k1.control_points[i], frac)).collect())
    }

    pub fn apply(&self, time: f32, spline: &mut CatmullRomSpline) {
        if let Some(pts) = self.evaluate_points(time) {
            for (i, pt) in pts.iter().enumerate() {
                if i < spline.control_points.len() {
                    spline.control_points[i].position = *pt;
                }
            }
            spline.rebuild_arc_length_table();
        }
    }
}

// ============================================================
// CATENARY CURVE
// ============================================================

pub struct Catenary {
    pub anchor_a: Vec3,
    pub anchor_b: Vec3,
    pub slack:    f32,
}

impl Catenary {
    pub fn new(a: Vec3, b: Vec3, slack: f32) -> Self {
        Catenary { anchor_a: a, anchor_b: b, slack }
    }

    pub fn evaluate(&self, t: f32) -> Vec3 {
        let dir = self.anchor_b - self.anchor_a;
        let horiz = Vec2::new(dir.x, dir.z).length();
        let vert  = dir.y;
        let chain_len = horiz + self.slack;
        let a = Self::solve_a(horiz, vert, chain_len);
        let x_offset = -horiz * 0.5;
        let x = x_offset + t * horiz;
        let y0 = a * (x_offset / a).cosh();
        let y  = a * (x / a).cosh() - y0;
        let horiz_dir = if horiz > EPSILON {
            Vec3::new(dir.x, 0.0, dir.z) / horiz
        } else { Vec3::X };
        self.anchor_a + horiz_dir * (t * horiz) + Vec3::Y * (y + vert * t - self.slack * 0.3)
    }

    fn solve_a(h: f32, v: f32, l: f32) -> f32 {
        let target = (l * l - v * v).max(0.0);
        let mut a = h.max(EPSILON);
        for _ in 0..64 {
            let s  = 2.0 * a * (h / (2.0 * a)).sinh();
            let err = s * s - target;
            let ds  = 2.0 * (h / (2.0 * a)).sinh() - (h / a) * (h / (2.0 * a)).cosh();
            let d   = 2.0 * s * ds;
            if d.abs() < EPSILON { break; }
            a -= err / d;
            a = a.max(EPSILON);
        }
        a
    }

    pub fn to_polyline(&self, steps: usize) -> Vec<Vec3> {
        (0..=steps).map(|i| self.evaluate(i as f32 / steps as f32)).collect()
    }
}

// ============================================================
// SPLINE ELEVATOR PROFILE
// ============================================================

#[derive(Clone, Debug)]
pub struct ElevationProfile {
    pub samples:       Vec<(f32, f32)>,
    pub max_grade:     f32,
    pub avg_grade:     f32,
    pub total_ascent:  f32,
    pub total_descent: f32,
}

impl ElevationProfile {
    pub fn compute(spline: &CatmullRomSpline, n: usize) -> Self {
        let total = spline.total_arc_length();
        let samples: Vec<(f32, f32)> = (0..=n).map(|i| {
            let s = i as f32 / n as f32 * total;
            let t = spline.t_at_arc_length(s);
            (s, spline.evaluate(t).y)
        }).collect();
        let mut max_grade = 0.0_f32;
        let mut ascent = 0.0_f32;
        let mut descent = 0.0_f32;
        for i in 1..samples.len() {
            let ds = samples[i].0 - samples[i-1].0;
            let dy = samples[i].1 - samples[i-1].1;
            if ds > EPSILON { let g = (dy / ds).abs() * 100.0; if g > max_grade { max_grade = g; } }
            if dy > 0.0 { ascent += dy; } else { descent += dy.abs(); }
        }
        let avg_grade = if total > EPSILON { (ascent + descent) / total * 100.0 } else { 0.0 };
        ElevationProfile { samples, max_grade, avg_grade, total_ascent: ascent, total_descent: descent }
    }

    pub fn elevation_at(&self, s: f32) -> f32 {
        let n = self.samples.len();
        if n == 0 { return 0.0; }
        let idx = self.samples.partition_point(|&(sa, _)| sa <= s);
        if idx == 0 { return self.samples[0].1; }
        if idx >= n { return self.samples[n-1].1; }
        let (s0, e0) = self.samples[idx-1];
        let (s1, e1) = self.samples[idx];
        let f = if (s1-s0).abs() < EPSILON { 0.0 } else { (s-s0)/(s1-s0) };
        lerp(e0, e1, f)
    }
}

// ============================================================
// TRAFFIC LIGHT
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum TrafficLightPhase { Green, Yellow, Red, FlashingRed }

#[derive(Clone, Debug)]
pub struct TrafficLight {
    pub id:              u64,
    pub position:        Vec3,
    pub phase:           TrafficLightPhase,
    pub phase_timer:     f32,
    pub green_time:      f32,
    pub yellow_time:     f32,
    pub red_time:        f32,
    pub controlled_edges: Vec<u64>,
}

impl TrafficLight {
    pub fn new(position: Vec3) -> Self {
        TrafficLight {
            id: rand_id(), position,
            phase: TrafficLightPhase::Green, phase_timer: 0.0,
            green_time: 30.0, yellow_time: 5.0, red_time: 30.0,
            controlled_edges: Vec::new(),
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.phase_timer += dt;
        let (next, dur) = match self.phase {
            TrafficLightPhase::Green       => (TrafficLightPhase::Yellow, self.green_time),
            TrafficLightPhase::Yellow      => (TrafficLightPhase::Red,    self.yellow_time),
            TrafficLightPhase::Red         => (TrafficLightPhase::Green,  self.red_time),
            TrafficLightPhase::FlashingRed => (TrafficLightPhase::Red,    2.0),
        };
        if self.phase_timer >= dur { self.phase = next; self.phase_timer -= dur; }
    }

    pub fn can_pass(&self) -> bool { self.phase == TrafficLightPhase::Green }

    pub fn color_rgba(&self) -> Vec4 {
        match self.phase {
            TrafficLightPhase::Green       => Vec4::new(0.0, 1.0, 0.0, 1.0),
            TrafficLightPhase::Yellow      => Vec4::new(1.0, 1.0, 0.0, 1.0),
            TrafficLightPhase::Red         => Vec4::new(1.0, 0.0, 0.0, 1.0),
            TrafficLightPhase::FlashingRed => {
                if (self.phase_timer * 2.0) as u32 % 2 == 0 {
                    Vec4::new(1.0, 0.0, 0.0, 1.0)
                } else { Vec4::new(0.2, 0.0, 0.0, 1.0) }
            }
        }
    }
}

// ============================================================
// SPLINE STATISTICS
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineStatistics {
    pub id:                     u64,
    pub name:                   String,
    pub total_length:           f32,
    pub num_segments:           usize,
    pub num_control_pts:        usize,
    pub min_curvature:          f32,
    pub max_curvature:          f32,
    pub avg_curvature:          f32,
    pub total_torsion_integral: f32,
    pub bounding_box_volume:    f32,
    pub is_closed:              bool,
}

impl SplineStatistics {
    pub fn compute(spline: &CatmullRomSpline, id: u64, name: &str) -> Self {
        let n = 128usize;
        let mut curvatures = Vec::with_capacity(n+1);
        let mut torsion_int = 0.0_f32;
        let dt = 1.0 / n as f32;
        for i in 0..=n {
            let t = i as f32 * dt;
            let frame = spline.frenet_frame_at(t);
            curvatures.push(frame.curvature);
            torsion_int += frame.torsion.abs() * dt;
        }
        let min_k = curvatures.iter().cloned().fold(f32::MAX,  f32::min);
        let max_k = curvatures.iter().cloned().fold(f32::MIN,  f32::max);
        let avg_k = curvatures.iter().sum::<f32>() / curvatures.len() as f32;
        let (bmin, bmax) = spline.bounding_box();
        let sz = bmax - bmin;
        SplineStatistics {
            id, name: name.to_string(),
            total_length: spline.total_arc_length(),
            num_segments: spline.num_segments(),
            num_control_pts: spline.control_points.len(),
            min_curvature: min_k, max_curvature: max_k, avg_curvature: avg_k,
            total_torsion_integral: torsion_int,
            bounding_box_volume: sz.x * sz.y * sz.z,
            is_closed: spline.closed,
        }
    }
}

impl SplineEditor {
    pub fn compute_statistics(&self, id: u64) -> Option<SplineStatistics> {
        let s = self.catmull_splines.get(&id)?;
        let n = self.spline_names.get(&id).cloned().unwrap_or_default();
        Some(SplineStatistics::compute(s, id, &n))
    }

    pub fn all_statistics(&self) -> Vec<SplineStatistics> {
        self.catmull_splines.iter().map(|(&id, s)| {
            let n = self.spline_names.get(&id).cloned().unwrap_or_default();
            SplineStatistics::compute(s, id, &n)
        }).collect()
    }

    pub fn find_by_name(&self, name: &str) -> Option<u64> {
        self.spline_names.iter().find(|(_, n)| n.as_str() == name).map(|(&id, _)| id)
    }

    pub fn rename_spline(&mut self, id: u64, new_name: &str) {
        if let Some(n) = self.spline_names.get_mut(&id) { *n = new_name.to_string(); }
    }

    pub fn duplicate_spline(&mut self, id: u64) -> Option<u64> {
        let s = self.catmull_splines.get(&id)?.clone();
        let name = self.spline_names.get(&id).cloned().unwrap_or_default();
        let new_id = rand_id();
        self.catmull_splines.insert(new_id, s);
        self.spline_names.insert(new_id, format!("{}_copy", name));
        self.spline_types.insert(new_id, SplineType::CatmullRom);
        Some(new_id)
    }

    pub fn translate_spline(&mut self, id: u64, delta: Vec3) {
        if let Some(s) = self.catmull_splines.get_mut(&id) {
            for cp in &mut s.control_points { cp.position += delta; }
            s.rebuild_arc_length_table();
        }
    }

    pub fn scale_spline(&mut self, id: u64, origin: Vec3, scale: Vec3) {
        if let Some(s) = self.catmull_splines.get_mut(&id) {
            for cp in &mut s.control_points {
                cp.position = origin + (cp.position - origin) * scale;
            }
            s.rebuild_arc_length_table();
        }
    }

    pub fn rotate_spline(&mut self, id: u64, origin: Vec3, rotation: Quat) {
        if let Some(s) = self.catmull_splines.get_mut(&id) {
            for cp in &mut s.control_points {
                cp.position = origin + rotation * (cp.position - origin);
            }
            s.rebuild_arc_length_table();
        }
    }

    pub fn mirror_spline(&mut self, id: u64, plane_normal: Vec3, plane_d: f32) -> Option<u64> {
        let new_id = self.duplicate_spline(id)?;
        if let Some(s) = self.catmull_splines.get_mut(&new_id) {
            for cp in &mut s.control_points {
                let d = plane_normal.dot(cp.position) - plane_d;
                cp.position -= plane_normal * 2.0 * d;
            }
            s.rebuild_arc_length_table();
        }
        Some(new_id)
    }
}

// ============================================================
// ACCELERATION PROFILE
// ============================================================

#[derive(Clone, Debug)]
pub struct AccelerationProfile {
    pub max_speed:        f32,
    pub acceleration:     f32,
    pub deceleration:     f32,
    pub approach_radius:  f32,
}

impl AccelerationProfile {
    pub fn new(max_speed: f32, accel: f32, decel: f32) -> Self {
        AccelerationProfile { max_speed, acceleration: accel, deceleration: decel, approach_radius: 5.0 }
    }

    pub fn speed_at(&self, current: f32, dist_to_end: f32, dt: f32) -> f32 {
        let target = if dist_to_end < self.approach_radius {
            self.max_speed * (dist_to_end / self.approach_radius.max(EPSILON))
        } else { self.max_speed };
        if current < target { (current + self.acceleration * dt).min(target) }
        else { (current - self.deceleration * dt).max(target).max(0.0) }
    }

    pub fn stopping_distance(&self, speed: f32) -> f32 {
        speed * speed / (2.0 * self.deceleration.max(EPSILON))
    }

    pub fn travel_time(&self, arc_length: f32) -> f32 {
        let ad = self.max_speed * self.max_speed / (2.0 * self.acceleration.max(EPSILON));
        let dd = self.stopping_distance(self.max_speed);
        let ramp = ad + dd;
        if arc_length < ramp {
            let pv = (arc_length * self.acceleration * self.deceleration
                / (self.acceleration + self.deceleration)).sqrt();
            pv / self.acceleration + pv / self.deceleration
        } else {
            self.max_speed / self.acceleration
                + (arc_length - ramp) / self.max_speed
                + self.max_speed / self.deceleration
        }
    }
}

// ============================================================
// SPLINE GROWTH (vine / tendril)
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineGrowthParams {
    pub direction:   Vec3,
    pub gravity:     f32,
    pub seed:        u32,
    pub step_length: f32,
    pub max_steps:   usize,
    pub turn_rate:   f32,
}

impl SplineGrowthParams {
    pub fn vine() -> Self {
        SplineGrowthParams {
            direction: Vec3::Y, gravity: -0.05, seed: 42,
            step_length: 0.3, max_steps: 64, turn_rate: 0.2,
        }
    }

    pub fn grow(&self) -> Vec<Vec3> {
        let mut pts = vec![Vec3::ZERO];
        let mut dir = safe_normalize(self.direction);
        let mut rng = self.seed as f32;
        for _ in 0..self.max_steps {
            rng = (rng * 1664525.0 + 1013904223.0) % 4294967296.0;
            let r  = rng / 4294967296.0;
            rng = (rng * 1664525.0 + 1013904223.0) % 4294967296.0;
            let r2 = rng / 4294967296.0;
            let turn = Vec3::new((r - 0.5) * 2.0 * self.turn_rate, self.gravity, (r2 - 0.5) * 2.0 * self.turn_rate);
            dir = safe_normalize(dir + turn);
            let last = *pts.last().unwrap();
            pts.push(last + dir * self.step_length);
        }
        pts
    }
}

// ============================================================
// FENCE GENERATOR
// ============================================================

#[derive(Clone, Debug)]
pub struct FenceProfile {
    pub post_height:    f32,
    pub post_width:     f32,
    pub post_spacing:   f32,
    pub rail_count:     u32,
}

impl FenceProfile {
    pub fn wooden_rail() -> Self {
        FenceProfile { post_height: 1.2, post_width: 0.1, post_spacing: 2.5, rail_count: 3 }
    }
}

#[derive(Clone, Debug)]
pub struct FenceGeometry {
    pub posts:    Vec<FrenetFrame>,
    pub profile:  FenceProfile,
}

impl FenceGeometry {
    pub fn build(spline: &CatmullRomSpline, profile: FenceProfile) -> Self {
        let total = spline.total_arc_length();
        let n_posts = (total / profile.post_spacing).floor() as usize + 1;
        let posts = (0..n_posts).map(|i| {
            let t = spline.t_at_arc_length(i as f32 * profile.post_spacing);
            spline.frenet_frame_at(t)
        }).collect();
        FenceGeometry { posts, profile }
    }

    pub fn post_matrix(&self, idx: usize) -> Mat4 {
        let f = &self.posts[idx];
        Mat4::from_cols(
            Vec4::new(f.normal.x,   f.normal.y,   f.normal.z,   0.0),
            Vec4::new(0.0,          1.0,           0.0,          0.0),
            Vec4::new(f.tangent.x,  f.tangent.y,  f.tangent.z,  0.0),
            Vec4::new(f.position.x, f.position.y, f.position.z, 1.0),
        )
    }

    pub fn rail_endpoints(&self, rail_idx: u32) -> Vec<(Vec3, Vec3)> {
        let y = (rail_idx + 1) as f32 * (self.profile.post_height / (self.profile.rail_count + 1) as f32);
        self.posts.windows(2).map(|w| {
            (w[0].position + Vec3::Y * y, w[1].position + Vec3::Y * y)
        }).collect()
    }
}

// ============================================================
// FIGURE-EIGHT / HELIX TEST HELPERS
// ============================================================

pub fn test_create_figure_eight() -> CatmullRomSpline {
    let pts = vec![
        Vec3::new( 0.0, 0.0,  0.0), Vec3::new( 5.0, 0.0,  5.0),
        Vec3::new(10.0, 0.0,  0.0), Vec3::new( 5.0, 0.0, -5.0),
        Vec3::new( 0.0, 0.0,  0.0), Vec3::new(-5.0, 0.0,  5.0),
        Vec3::new(-10.0,0.0,  0.0), Vec3::new(-5.0, 0.0, -5.0),
        Vec3::new( 0.0, 0.0,  0.0),
    ];
    CatmullRomSpline::new(pts, 0.5, false)
}

pub fn test_create_spiral() -> CatmullRomSpline {
    let pts = generate_helix(Vec3::ZERO, 5.0, 2.0, 3.0, 64);
    CatmullRomSpline::new(pts, 0.5, false)
}

pub fn test_create_sine_wave() -> CatmullRomSpline {
    let pts: Vec<Vec3> = (0..32).map(|i| {
        let x = i as f32 * 0.5;
        Vec3::new(x, (x * 0.5).sin() * 2.0, 0.0)
    }).collect();
    CatmullRomSpline::new(pts, 0.5, false)
}

// ============================================================
// BEZIER TIGHT BOUNDING BOX
// ============================================================

pub fn bezier_tight_bounding_box(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3) -> (Vec3, Vec3) {
    let mut mn = p0.min(p3);
    let mut mx = p0.max(p3);
    for dim in 0..3usize {
        let v = [p0, p1, p2, p3].map(|p| [p.x, p.y, p.z][dim]);
        let a = -3.0*v[0] + 9.0*v[1] - 9.0*v[2] + 3.0*v[3];
        let b =  6.0*v[0] - 12.0*v[1] + 6.0*v[2];
        let c = -3.0*v[0] + 3.0*v[1];
        let mut test_t = |t: f32| {
            if t > 0.0 && t < 1.0 {
                let pt = CubicBezierSpline::de_casteljau(p0, p1, p2, p3, t);
                mn = mn.min(pt);
                mx = mx.max(pt);
            }
        };
        if a.abs() < EPSILON {
            if b.abs() > EPSILON { test_t(-c / b); }
        } else {
            let disc = b*b - 4.0*a*c;
            if disc >= 0.0 {
                let sq = disc.sqrt();
                test_t((-b + sq) / (2.0*a));
                test_t((-b - sq) / (2.0*a));
            }
        }
    }
    (mn, mx)
}

// ============================================================

impl PathNetwork {
    pub fn total_length(&self) -> f32 {
        self.edges.values().map(|e| e.weight).sum()
    }

    pub fn node_degree(&self, id: u64) -> usize {
        self.adjacency.get(&id).map(|v| v.len()).unwrap_or(0)
    }

    pub fn junction_nodes(&self) -> Vec<u64> {
        self.nodes.keys().filter(|&&id| self.node_degree(id) > 2).cloned().collect()
    }

    pub fn dead_end_nodes(&self) -> Vec<u64> {
        self.nodes.keys().filter(|&&id| self.node_degree(id) == 1).cloned().collect()
    }

    pub fn edge_passable(&self, edge_id: u64) -> bool {
        self.edges.contains_key(&edge_id)
    }
}

// ============================================================
// SPLINE ATTACHMENT
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineAttachment {
    pub spline_id:      u64,
    pub t:              f32,
    pub local_offset:   Vec3,
    pub local_rotation: Quat,
}

impl SplineAttachment {
    pub fn new(spline_id: u64, t: f32) -> Self {
        SplineAttachment { spline_id, t, local_offset: Vec3::ZERO, local_rotation: Quat::IDENTITY }
    }

    pub fn world_transform(&self, spline: &CatmullRomSpline) -> Mat4 {
        let frame = spline.frenet_frame_at(self.t);
        let rot   = Quat::from_mat4(&frame.to_matrix()) * self.local_rotation;
        let pos   = frame.position
            + frame.tangent  * self.local_offset.x
            + frame.normal   * self.local_offset.y
            + frame.binormal * self.local_offset.z;
        Mat4::from_rotation_translation(rot, pos)
    }
}

// ============================================================
// DOUGLAS-PEUCKER POLYLINE SIMPLIFICATION (public API)
// ============================================================

pub fn simplify_polyline(pts: &[Vec3], epsilon: f32) -> Vec<Vec3> {
    douglas_peucker(pts, epsilon)
}

// ============================================================
// SPLINE EDITOR FINALIZATION
// ============================================================

impl SplineEditor {
    /// Clear all data
    pub fn clear(&mut self) {
        self.catmull_splines.clear();
        self.bezier_splines.clear();
        self.bsplines.clear();
        self.nurbs_splines.clear();
        self.hermite_splines.clear();
        self.spline_names.clear();
        self.spline_types.clear();
        self.rail_tracks.clear();
        self.camera_rails.clear();
        self.constrained_objects.clear();
        self.chains.clear();
        self.generated_meshes.clear();
        self.selection.clear();
        self.debug_viz.clear();
        self.undo_history = UndoHistory::new(128);
    }

    /// Set mesh section preset
    pub fn use_circle_section(&mut self, radius: f32, segments: usize) {
        self.mesh_section = CrossSection::circle(radius, segments);
    }

    pub fn use_rectangle_section(&mut self, w: f32, h: f32) {
        self.mesh_section = CrossSection::rectangle(w, h);
    }

    pub fn use_ibeam_section(&mut self, w: f32, h: f32, flange: f32, web: f32) {
        self.mesh_section = CrossSection::i_beam(w, h, flange, web);
    }

    /// Select all splines
    pub fn select_all(&mut self) {
        for &id in self.catmull_splines.keys() {
            self.selection.selected.insert(id);
        }
    }

    /// Deselect all
    pub fn deselect_all(&mut self) {
        self.selection.clear();
    }

    /// Delete selected splines
    pub fn delete_selected(&mut self) {
        let to_delete: Vec<u64> = self.selection.selected.iter().cloned().collect();
        for id in to_delete {
            self.catmull_splines.remove(&id);
            self.spline_names.remove(&id);
            self.spline_types.remove(&id);
            self.generated_meshes.remove(&id);
        }
        self.selection.clear();
    }

    /// Regenerate all meshes
    pub fn regenerate_all_meshes(&mut self) {
        let ids: Vec<u64> = self.catmull_splines.keys().cloned().collect();
        for id in ids {
            self.generate_mesh_for_spline(id);
        }
    }
}

// ============================================================
// SPLINE SMOOTHING & FAIRING
// ============================================================

/// Laplacian smoothing: move each interior control point toward
/// the average of its two neighbours by factor `lambda`.
pub fn laplacian_smooth_catmull(spline: &mut CatmullRomSpline, lambda: f32, iterations: u32) {
    for _ in 0..iterations {
        let n = spline.control_points.len();
        if n < 3 { break; }
        let old: Vec<Vec3> = spline.control_points.iter().map(|cp| cp.position).collect();
        for i in 1..n - 1 {
            let avg = (old[i - 1] + old[i + 1]) * 0.5;
            spline.control_points[i].position = old[i] + (avg - old[i]) * lambda;
        }
    }
}

/// Taubin smoothing: two-pass (lambda / -mu) to avoid shrinkage.
pub fn taubin_smooth_catmull(spline: &mut CatmullRomSpline, lambda: f32, mu: f32, iterations: u32) {
    for _ in 0..iterations {
        laplacian_smooth_catmull(spline, lambda, 1);
        laplacian_smooth_catmull(spline, mu, 1);
    }
}

/// Compute the total variation (sum of |dp_i| changes) of a spline.
pub fn spline_total_variation(spline: &CatmullRomSpline) -> f32 {
    let pts: Vec<Vec3> = spline.control_points.iter().map(|cp| cp.position).collect();
    pts.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

/// Redistribute control points to be uniformly spaced along the arc.
pub fn equidistribute_catmull(spline: &mut CatmullRomSpline, new_count: usize) {
    if spline.control_points.len() < 2 || new_count < 2 { return; }
    let table = build_arc_length_table(512, &|t| spline.evaluate(t));
    let total  = table.last().map(|&(_, s)| s).unwrap_or(1.0);
    let step   = total / (new_count - 1) as f32;
    let new_pts: Vec<Vec3> = (0..new_count).map(|i| {
        let s = (i as f32 * step).min(total);
        let t = arc_length_to_t(&table, s);
        spline.evaluate(t)
    }).collect();
    spline.control_points = new_pts.into_iter().map(|p| ControlPoint {
        position: p, weight: 1.0, ..ControlPoint::new(p)
    }).collect();
}

// ============================================================
// CURVE FITTING (least-squares cubic Bezier)
// ============================================================

/// Fit a single cubic Bezier to a set of ordered sample points using chord-length
/// parameterization and the Moore-Penrose pseudoinverse.  Returns [P0, P1, P2, P3].
pub fn fit_cubic_bezier(points: &[Vec3]) -> [Vec3; 4] {
    if points.len() < 2 {
        let p = points.first().copied().unwrap_or(Vec3::ZERO);
        return [p, p, p, p];
    }
    let mut params: Vec<f32> = vec![0.0];
    for i in 1..points.len() {
        let d = (points[i] - points[i-1]).length();
        params.push(params[i-1] + d);
    }
    let total = *params.last().unwrap();
    if total < 1e-10 { let p = points[0]; return [p, p, p, p]; }
    for p in &mut params { *p /= total; }

    let p0 = points[0];
    let p3 = *points.last().unwrap();

    fn b0(t: f32) -> f32 { let u=1.0-t; u*u*u }
    fn b1(t: f32) -> f32 { let u=1.0-t; 3.0*u*u*t }
    fn b2(t: f32) -> f32 { let u=1.0-t; 3.0*u*t*t }
    fn b3(t: f32) -> f32 { t*t*t }

    let n = points.len();
    let mut ata = [[0.0f32; 2]; 2];
    let mut atr = [[0.0f32; 3]; 2];

    for i in 0..n {
        let t  = params[i];
        let a  = [b1(t), b2(t)];
        let rhs = points[i] - p0 * b0(t) - p3 * b3(t);
        for r in 0..2 {
            for c in 0..2 { ata[r][c] += a[r] * a[c]; }
            atr[r][0] += a[r] * rhs.x;
            atr[r][1] += a[r] * rhs.y;
            atr[r][2] += a[r] * rhs.z;
        }
    }
    let det = ata[0][0]*ata[1][1] - ata[0][1]*ata[1][0];
    if det.abs() < 1e-12 { return [p0, p0, p3, p3]; }
    let inv = [[ ata[1][1]/det, -ata[0][1]/det],
               [-ata[1][0]/det,  ata[0][0]/det]];
    let mut p1 = Vec3::ZERO;
    let mut p2 = Vec3::ZERO;
    for r in 0..2 {
        let vx = inv[r][0]*atr[0][0] + inv[r][1]*atr[1][0];
        let vy = inv[r][0]*atr[0][1] + inv[r][1]*atr[1][1];
        let vz = inv[r][0]*atr[0][2] + inv[r][1]*atr[1][2];
        if r == 0 { p1 = Vec3::new(vx, vy, vz); }
        else      { p2 = Vec3::new(vx, vy, vz); }
    }
    [p0, p1, p2, p3]
}

/// Fit a piecewise cubic Bezier (G1 continuous) to points.
pub fn fit_piecewise_cubic_bezier(points: &[Vec3], max_error: f32) -> CubicBezierSpline {
    let mut spline = CubicBezierSpline { segments: Vec::new(), closed: false, arc_length_table: Vec::new(), total_length: 0.0 };
    if points.len() < 2 { return spline; }

    fn fit_and_check(pts: &[Vec3], tol: f32, out: &mut Vec<[Vec3; 4]>) {
        if pts.len() < 2 { return; }
        let seg = fit_cubic_bezier(pts);
        let mut max_err = 0.0f32;
        let mut worst  = pts.len() / 2;
        for (i, &pt) in pts.iter().enumerate() {
            let t = i as f32 / (pts.len() - 1).max(1) as f32;
            let fitted = CubicBezierSpline::de_casteljau(seg[0], seg[1], seg[2], seg[3], t);
            let err = (fitted - pt).length();
            if err > max_err { max_err = err; worst = i; }
        }
        if max_err <= tol || pts.len() <= 3 { out.push(seg); }
        else {
            fit_and_check(&pts[..=worst], tol, out);
            fit_and_check(&pts[worst..], tol, out);
        }
    }

    fit_and_check(points, max_error, &mut spline.segments);
    spline
}

// ============================================================
// SPLINE OFFSET (2-D parallel offset)
// ============================================================

/// Offset a CatmullRom spline in the XZ plane by `distance` (positive = left).
pub fn offset_spline_xz(spline: &CatmullRomSpline, distance: f32, samples: usize) -> CatmullRomSpline {
    let table = build_arc_length_table(samples * 4, &|t| spline.evaluate(t));
    let total  = table.last().map(|&(_, s)| s).unwrap_or(1.0);
    let step   = total / (samples - 1).max(1) as f32;
    let pts: Vec<Vec3> = (0..samples).map(|i| {
        let s = (i as f32 * step).min(total);
        let t = arc_length_to_t(&table, s);
        let pos  = spline.evaluate(t);
        let tang = spline.evaluate_derivative(t);
        let n = Vec3::new(-tang.z, 0.0, tang.x).normalize_or_zero();
        pos + n * distance
    }).collect();
    CatmullRomSpline::new(pts.into_iter().map(|p| p).collect(), spline.alpha, spline.closed)
}

// ============================================================
// RIBBON MESH
// ============================================================

/// A flat ribbon along the spline with configurable width taper.
pub struct RibbonMesh {
    pub vertices: Vec<Vec3>,
    pub normals:  Vec<Vec3>,
    pub uvs:      Vec<Vec2>,
    pub indices:  Vec<u32>,
    pub width_at: Vec<f32>,
}

impl RibbonMesh {
    pub fn generate(spline: &CatmullRomSpline, steps: usize, width_fn: &dyn Fn(f32) -> f32) -> Self {
        let table  = build_arc_length_table(steps * 8, &|t| spline.evaluate(t));
        let total  = table.last().map(|&(_, s)| s).unwrap_or(1.0);
        let mut verts   = Vec::new();
        let mut normals = Vec::new();
        let mut uvs     = Vec::new();
        let mut indices = Vec::new();
        let mut width_at = Vec::new();

        let mut frames: Vec<ParallelTransportFrame> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t_param = i as f32 / steps as f32;
            let s  = t_param * total;
            let t  = arc_length_to_t(&table, s);
            let p  = spline.evaluate(t);
            let tn = spline.evaluate_derivative(t);
            if i == 0 { frames.push(ParallelTransportFrame::initial(p, tn)); }
            else {
                let prev = &frames[i - 1];
                frames.push(ParallelTransportFrame::transport(prev, p, tn));
            }
        }

        for (i, frame) in frames.iter().enumerate() {
            let t_param = i as f32 / steps as f32;
            let hw = width_fn(t_param);
            width_at.push(hw);
            let u = i as f32 / steps as f32;
            let left  = frame.position - frame.normal * hw;
            let right = frame.position + frame.normal * hw;
            verts.push(left);
            verts.push(right);
            normals.push(frame.binormal);
            normals.push(frame.binormal);
            uvs.push(Vec2::new(u, 0.0));
            uvs.push(Vec2::new(u, 1.0));
        }

        for i in 0..steps {
            let bl = (i * 2) as u32;
            let br = bl + 1;
            let tl = bl + 2;
            let tr = bl + 3;
            indices.extend_from_slice(&[bl, br, tl, br, tr, tl]);
        }

        RibbonMesh { vertices: verts, normals, uvs, indices, width_at }
    }

    pub fn surface_area(&self) -> f32 {
        let mut area = 0.0f32;
        for tri in self.indices.chunks(3) {
            if tri.len() < 3 { continue; }
            let a = self.vertices[tri[0] as usize];
            let b = self.vertices[tri[1] as usize];
            let c = self.vertices[tri[2] as usize];
            area += (b - a).cross(c - a).length() * 0.5;
        }
        area
    }
}

// ============================================================
// EXTRUDED PROFILE MESH
// ============================================================

pub struct ExtrudedProfile {
    pub vertices: Vec<Vec3>,
    pub normals:  Vec<Vec3>,
    pub uvs:      Vec<Vec2>,
    pub indices:  Vec<u32>,
}

impl ExtrudedProfile {
    pub fn generate(
        spline:   &CatmullRomSpline,
        profile:  &[Vec2],
        steps:    usize,
        scale_fn: &dyn Fn(f32) -> f32,
    ) -> Self {
        let table  = build_arc_length_table(steps * 8, &|t| spline.evaluate(t));
        let total  = table.last().map(|&(_, s)| s).unwrap_or(1.0);
        let np     = profile.len();
        let mut verts   = Vec::new();
        let mut normals = Vec::new();
        let mut uvs_out = Vec::new();
        let mut indices = Vec::new();

        let mut frames: Vec<ParallelTransportFrame> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t_p = i as f32 / steps as f32;
            let s   = t_p * total;
            let t   = arc_length_to_t(&table, s);
            let p   = spline.evaluate(t);
            let tn  = spline.evaluate_derivative(t);
            if i == 0 { frames.push(ParallelTransportFrame::initial(p, tn)); }
            else {
                let prev = &frames[i - 1];
                frames.push(ParallelTransportFrame::transport(prev, p, tn));
            }
        }

        for (i, frame) in frames.iter().enumerate() {
            let t_p   = i as f32 / steps as f32;
            let scale = scale_fn(t_p);
            let u_val = t_p;
            for (j, &pv) in profile.iter().enumerate() {
                let world = frame.position
                    + frame.normal   * pv.x * scale
                    + frame.binormal * pv.y * scale;
                let pn = Vec2::new(pv.y, -pv.x).normalize_or_zero();
                let wn = (frame.normal * pn.x + frame.binormal * pn.y).normalize_or_zero();
                verts.push(world);
                normals.push(wn);
                uvs_out.push(Vec2::new(u_val, j as f32 / np as f32));
            }
        }

        for i in 0..steps {
            for j in 0..np {
                let jn = (j + 1) % np;
                let a = (i * np + j)  as u32;
                let b = (i * np + jn) as u32;
                let c = ((i + 1) * np + j)  as u32;
                let d = ((i + 1) * np + jn) as u32;
                indices.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }

        ExtrudedProfile { vertices: verts, normals, uvs: uvs_out, indices }
    }
}

// ============================================================
// SPLINE CAGE DEFORMER
// ============================================================

pub struct SplineCage {
    pub source_spline: CatmullRomSpline,
    pub target_spline: CatmullRomSpline,
}

impl SplineCage {
    pub fn deform(&self, point: Vec3) -> Vec3 {
        let (t, _) = self.source_spline.nearest_point(point);
        let src_pos  = self.source_spline.evaluate(t);
        let src_tang = self.source_spline.evaluate_derivative(t);
        let src_norm = {
            let up = if src_tang.y.abs() < 0.99 { Vec3::Y } else { Vec3::Z };
            src_tang.cross(up).normalize_or_zero()
        };
        let src_bi = src_tang.cross(src_norm).normalize_or_zero();
        let offset = point - src_pos;
        let local_t = offset.dot(src_tang);
        let local_n = offset.dot(src_norm);
        let local_b = offset.dot(src_bi);

        let tgt_pos  = self.target_spline.evaluate(t);
        let tgt_tang = self.target_spline.evaluate_derivative(t);
        let tgt_up   = if tgt_tang.y.abs() < 0.99 { Vec3::Y } else { Vec3::Z };
        let tgt_norm = tgt_tang.cross(tgt_up).normalize_or_zero();
        let tgt_bi   = tgt_tang.cross(tgt_norm).normalize_or_zero();

        tgt_pos + tgt_tang * local_t + tgt_norm * local_n + tgt_bi * local_b
    }

    pub fn deform_mesh(&self, points: &mut [Vec3]) {
        for p in points.iter_mut() { *p = self.deform(*p); }
    }
}

// ============================================================
// SPLINE LATTICE DEFORMER (two-rail)
// ============================================================

pub struct SplineLattice {
    pub rail_a: CatmullRomSpline,
    pub rail_b: CatmullRomSpline,
}

impl SplineLattice {
    pub fn evaluate(&self, u: f32, v: f32) -> Vec3 {
        let pa = self.rail_a.evaluate(u.clamp(0.0, 1.0));
        let pb = self.rail_b.evaluate(u.clamp(0.0, 1.0));
        pa.lerp(pb, v.clamp(0.0, 1.0))
    }

    pub fn deform(&self, point: Vec3) -> Vec3 {
        let (u, _) = self.rail_a.nearest_point(point);
        let a = self.rail_a.evaluate(u);
        let b = self.rail_b.evaluate(u);
        let ab   = b - a;
        let len2 = ab.length_squared();
        let v = if len2 < 1e-10 { 0.0 } else { (point - a).dot(ab) / len2 };
        self.evaluate(u, v)
    }
}

// ============================================================
// SPLINE DEFORMATION HISTORY
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineDeformHistory {
    pub snapshots: VecDeque<Vec<Vec3>>,
    pub max_size:  usize,
}

impl SplineDeformHistory {
    pub fn new(max_size: usize) -> Self {
        SplineDeformHistory { snapshots: VecDeque::new(), max_size }
    }

    pub fn push(&mut self, positions: Vec<Vec3>) {
        if self.snapshots.len() >= self.max_size { self.snapshots.pop_front(); }
        self.snapshots.push_back(positions);
    }

    pub fn undo(&mut self) -> Option<Vec<Vec3>> { self.snapshots.pop_back() }

    pub fn blend(&self, t: f32) -> Option<Vec<Vec3>> {
        let n = self.snapshots.len();
        if n < 2 { return self.snapshots.back().cloned(); }
        let fi = (t.clamp(0.0, 1.0) * (n - 1) as f32).floor() as usize;
        let fi = fi.min(n - 2);
        let alpha = t * (n - 1) as f32 - fi as f32;
        let a = &self.snapshots[fi];
        let b = &self.snapshots[fi + 1];
        if a.len() != b.len() { return Some(a.clone()); }
        Some(a.iter().zip(b.iter()).map(|(&pa, &pb)| pa.lerp(pb, alpha)).collect())
    }
}

// ============================================================
// SPLINE MASS-SPRING DYNAMICS
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub mass:     f32,
    pub pinned:   bool,
}

pub struct SplineDynamics {
    pub particles:    Vec<SplineParticle>,
    pub rest_lengths: Vec<f32>,
    pub stiffness:    f32,
    pub damping:      f32,
    pub gravity:      Vec3,
}

impl SplineDynamics {
    pub fn from_catmull(spline: &CatmullRomSpline, stiffness: f32, damping: f32) -> Self {
        let particles: Vec<SplineParticle> = spline.control_points.iter()
            .map(|cp| SplineParticle { position: cp.position, velocity: Vec3::ZERO, mass: 1.0, pinned: false })
            .collect();
        let rest_lengths: Vec<f32> = particles.windows(2)
            .map(|w| (w[1].position - w[0].position).length())
            .collect();
        SplineDynamics { particles, rest_lengths, stiffness, damping, gravity: Vec3::new(0.0, -9.81, 0.0) }
    }

    pub fn step(&mut self, dt: f32) {
        let n = self.particles.len();
        let mut forces: Vec<Vec3> = vec![Vec3::ZERO; n];

        for i in 0..n.saturating_sub(1) {
            let rest  = self.rest_lengths[i];
            let pa    = self.particles[i].position;
            let pb    = self.particles[i + 1].position;
            let delta = pb - pa;
            let dist  = delta.length();
            if dist < 1e-10 { continue; }
            let f = delta.normalize() * self.stiffness * (dist - rest);
            forces[i]     += f;
            forces[i + 1] -= f;
        }

        for i in 0..n {
            if self.particles[i].pinned { continue; }
            let m     = self.particles[i].mass;
            let accel = (forces[i] + self.gravity * m) / m - self.particles[i].velocity * self.damping;
            self.particles[i].velocity += accel * dt;
            let vel = self.particles[i].velocity;
            self.particles[i].position += vel * dt;
        }
    }

    pub fn apply_to_catmull(&self, spline: &mut CatmullRomSpline) {
        for (i, p) in self.particles.iter().enumerate() {
            if let Some(cp) = spline.control_points.get_mut(i) { cp.position = p.position; }
        }
        spline.rebuild_arc_length_table();
    }
}

// ============================================================
// CLOTHOID (EULER SPIRAL) APPROXIMATION
// ============================================================

fn fresnel_s_approx(t: f32) -> f32 {
    let t2 = t * t;
    let mut s    = t * t2 / 3.0;
    let mut sign = -1.0f32;
    let mut term = t * t2 * t2 * t2 / (3.0 * 14.0);
    for k in 1u32..12 {
        s += sign * term;
        sign = -sign;
        let f = (2 * k + 1) as f32;
        term *= t2 * t2 / (f * (f + 2.0) * 2.0 * (k + 1) as f32);
        if term.abs() < 1e-10 { break; }
    }
    s
}

fn fresnel_c_approx(t: f32) -> f32 {
    let t2 = t * t;
    let mut c    = t;
    let mut sign = -1.0f32;
    let mut term = t * t2 * t2 / (2.0 * 5.0);
    for k in 1u32..12 {
        c += sign * term;
        sign = -sign;
        let f = (2 * k) as f32;
        term *= t2 * t2 / (f * (f + 1.0) * 2.0 * (k + 1) as f32);
        if term.abs() < 1e-10 { break; }
    }
    c
}

/// Sample a Clothoid (Euler spiral) with scale `a`, returning XZ positions.
pub fn sample_clothoid(a: f32, n: usize, flip_z: bool) -> Vec<Vec3> {
    let max_t = std::f32::consts::PI.sqrt();
    (0..n).map(|i| {
        let t = i as f32 / n.max(1) as f32 * max_t;
        let x = a * fresnel_c_approx(t);
        let z = a * fresnel_s_approx(t) * if flip_z { -1.0 } else { 1.0 };
        Vec3::new(x, 0.0, z)
    }).collect()
}

/// Insert a clothoid transition between two tangent directions.
pub fn clothoid_transition(length: f32, n: usize) -> (Vec<Vec3>, Vec<Vec3>) {
    let a = length.sqrt();
    let left  = sample_clothoid(a, n, false);
    let right = sample_clothoid(a, n, true);
    (left, right)
}

// ============================================================
// BIARC FIT
// ============================================================

#[derive(Clone, Debug)]
pub struct CircularArc {
    pub centre:    Vec3,
    pub radius:    f32,
    pub start_pt:  Vec3,
    pub end_pt:    Vec3,
    pub start_ang: f32,
    pub end_ang:   f32,
    pub axis:      Vec3,
}

impl CircularArc {
    pub fn evaluate(&self, t: f32) -> Vec3 {
        let angle = self.start_ang + (self.end_ang - self.start_ang) * t;
        let fwd   = (self.start_pt - self.centre).normalize_or_zero();
        let right = self.axis.cross(fwd).normalize_or_zero();
        self.centre + fwd * (angle.cos() * self.radius) + right * (angle.sin() * self.radius)
    }

    pub fn arc_length(&self) -> f32 {
        (self.end_ang - self.start_ang).abs() * self.radius
    }
}

/// Approximate a G1 segment with two circular arcs (biarc).
pub fn biarc_fit(p0: Vec3, t0: Vec3, p1: Vec3, t1: Vec3) -> (CircularArc, CircularArc) {
    let t0 = t0.normalize_or_zero();
    let chord  = p1 - p0;
    let chord_len = chord.length();
    let j = p0 + chord * 0.5; // simple midpoint join

    fn make_arc(a: Vec3, ta: Vec3, b: Vec3) -> CircularArc {
        let perp_ta = Vec3::new(-ta.z, 0.0, ta.x).normalize_or_zero();
        let d    = b - a;
        let proj = d.dot(perp_ta);
        let r    = if proj.abs() < 1e-8 { 1e6 } else { d.length_squared() / (2.0 * proj) };
        let centre = a + perp_ta * r;
        let axis = ta.cross(d).normalize_or_zero();
        CircularArc { centre, radius: r.abs(), start_pt: a, end_pt: b, start_ang: 0.0, end_ang: 1.0, axis }
    }

    let arc0 = make_arc(p0, t0, j);
    let d1   = (j - p0).normalize_or_zero();
    let arc1 = make_arc(j, d1, p1);
    let _ = (chord_len, t1);
    (arc0, arc1)
}

// ============================================================
// SPLINE IK CHAIN (FABRIK)
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineIKChain {
    pub joints:       Vec<Vec3>,
    pub bone_lengths: Vec<f32>,
    pub root_fixed:   bool,
}

impl SplineIKChain {
    pub fn new(joints: Vec<Vec3>) -> Self {
        let bone_lengths = joints.windows(2).map(|w| (w[1] - w[0]).length()).collect();
        SplineIKChain { joints, bone_lengths, root_fixed: true }
    }

    pub fn solve_fabrik(&mut self, target: Vec3, max_iter: u32, tolerance: f32) {
        let n    = self.joints.len();
        if n < 2 { return; }
        let root = self.joints[0];
        let total_len: f32 = self.bone_lengths.iter().sum();
        if (target - root).length() >= total_len {
            let dir = (target - root).normalize_or_zero();
            for i in 1..n {
                let len: f32 = self.bone_lengths[..i].iter().sum();
                self.joints[i] = root + dir * len;
            }
            return;
        }
        for _ in 0..max_iter {
            self.joints[n - 1] = target;
            for i in (0..n - 1).rev() {
                let d = (self.joints[i] - self.joints[i + 1]).normalize_or_zero();
                self.joints[i] = self.joints[i + 1] + d * self.bone_lengths[i];
            }
            if self.root_fixed { self.joints[0] = root; }
            for i in 0..n - 1 {
                let d = (self.joints[i + 1] - self.joints[i]).normalize_or_zero();
                self.joints[i + 1] = self.joints[i] + d * self.bone_lengths[i];
            }
            if (self.joints[n - 1] - target).length() < tolerance { break; }
        }
    }

    pub fn to_spline(&self) -> CatmullRomSpline {
        CatmullRomSpline::new(self.joints.iter().copied().collect(), 0.5, false)
    }
}

// ============================================================
// TRANSFORM UTILITIES
// ============================================================

pub fn snap_to_grid(spline: &mut CatmullRomSpline, cell_size: f32) {
    if cell_size < 1e-10 { return; }
    for cp in &mut spline.control_points {
        cp.position.x = (cp.position.x / cell_size).round() * cell_size;
        cp.position.y = (cp.position.y / cell_size).round() * cell_size;
        cp.position.z = (cp.position.z / cell_size).round() * cell_size;
    }
    spline.rebuild_arc_length_table();
}

pub fn snap_to_grid_xz(spline: &mut CatmullRomSpline, cell_size: f32) {
    if cell_size < 1e-10 { return; }
    for cp in &mut spline.control_points {
        cp.position.x = (cp.position.x / cell_size).round() * cell_size;
        cp.position.z = (cp.position.z / cell_size).round() * cell_size;
    }
    spline.rebuild_arc_length_table();
}

pub fn mirror_spline_x(spline: &mut CatmullRomSpline) {
    for cp in &mut spline.control_points { cp.position.x = -cp.position.x; }
    spline.control_points.reverse();
    spline.rebuild_arc_length_table();
}

pub fn mirror_spline_y(spline: &mut CatmullRomSpline) {
    for cp in &mut spline.control_points { cp.position.y = -cp.position.y; }
    spline.control_points.reverse();
    spline.rebuild_arc_length_table();
}

pub fn mirror_spline_z(spline: &mut CatmullRomSpline) {
    for cp in &mut spline.control_points { cp.position.z = -cp.position.z; }
    spline.control_points.reverse();
    spline.rebuild_arc_length_table();
}

pub fn translate_spline(spline: &mut CatmullRomSpline, delta: Vec3) {
    for cp in &mut spline.control_points { cp.position += delta; }
    spline.rebuild_arc_length_table();
}

pub fn rotate_spline(spline: &mut CatmullRomSpline, rot: Quat) {
    for cp in &mut spline.control_points { cp.position = rot * cp.position; }
    spline.rebuild_arc_length_table();
}

pub fn scale_spline_uniform(spline: &mut CatmullRomSpline, scale: f32) {
    for cp in &mut spline.control_points { cp.position *= scale; }
    spline.rebuild_arc_length_table();
}

pub fn scale_spline(spline: &mut CatmullRomSpline, sx: f32, sy: f32, sz: f32) {
    for cp in &mut spline.control_points {
        cp.position.x *= sx;
        cp.position.y *= sy;
        cp.position.z *= sz;
    }
    spline.rebuild_arc_length_table();
}

// ============================================================
// SAMPLING STRATEGIES
// ============================================================

pub fn sample_uniform_arc_length(spline: &CatmullRomSpline, n: usize) -> Vec<Vec3> {
    let table = build_arc_length_table(n * 8, &|t| spline.evaluate(t));
    let total  = table.last().map(|&(_, s)| s).unwrap_or(0.0);
    (0..n).map(|i| {
        let s = total * i as f32 / (n - 1).max(1) as f32;
        let t = arc_length_to_t(&table, s);
        spline.evaluate(t)
    }).collect()
}

pub fn sample_by_world_step(spline: &CatmullRomSpline, step: f32) -> Vec<(Vec3, f32)> {
    let table = build_arc_length_table(2048, &|t| spline.evaluate(t));
    let total  = table.last().map(|&(_, s)| s).unwrap_or(0.0);
    if step <= 0.0 || total <= 0.0 { return Vec::new(); }
    let count = (total / step).ceil() as usize + 1;
    (0..count).map(|i| {
        let s = (i as f32 * step).min(total);
        let t = arc_length_to_t(&table, s);
        (spline.evaluate(t), t)
    }).collect()
}

pub fn sample_adaptive_curvature(spline: &CatmullRomSpline, min_samples: usize, max_samples: usize, threshold: f32) -> Vec<Vec3> {
    let base: Vec<(f32, f32)> = (0..=max_samples).map(|i| {
        let t = i as f32 / max_samples as f32;
        let frenet = spline.frenet_frame_at(t);
        (t, frenet.curvature)
    }).collect();
    let mut selected: Vec<f32> = vec![0.0, 1.0];
    for &(t, kappa) in &base {
        if kappa > threshold { selected.push(t); }
    }
    selected.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    selected.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    while selected.len() < min_samples {
        let mut best_gap = 0.0f32;
        let mut best_idx = 0;
        for i in 0..selected.len().saturating_sub(1) {
            let g = selected[i + 1] - selected[i];
            if g > best_gap { best_gap = g; best_idx = i; }
        }
        let mid = (selected[best_idx] + selected[best_idx + 1]) * 0.5;
        selected.insert(best_idx + 1, mid);
    }
    selected.into_iter().map(|t| spline.evaluate(t)).collect()
}

// ============================================================
// ANALYSIS REPORT
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineAnalysisReport {
    pub total_arc_length:    f32,
    pub min_curvature:       f32,
    pub max_curvature:       f32,
    pub mean_curvature:      f32,
    pub min_torsion:         f32,
    pub max_torsion:         f32,
    pub inflection_count:    usize,
    pub control_point_count: usize,
    pub self_intersection:   bool,
    pub bounding_box_min:    Vec3,
    pub bounding_box_max:    Vec3,
}

impl SplineAnalysisReport {
    pub fn compute(spline: &CatmullRomSpline, samples: usize) -> Self {
        let mut min_k   = f32::MAX;
        let mut max_k   = f32::MIN;
        let mut sum_k   = 0.0f32;
        let mut min_tau = f32::MAX;
        let mut max_tau = f32::MIN;
        let mut inflections = 0usize;
        let mut prev_sign   = 0i32;
        let mut bb_min = Vec3::splat(f32::MAX);
        let mut bb_max = Vec3::splat(f32::MIN);

        let table = build_arc_length_table(samples * 4, &|t| spline.evaluate(t));
        let total  = table.last().map(|&(_, s)| s).unwrap_or(0.0);

        for i in 0..=samples {
            let t     = i as f32 / samples as f32;
            let frame = spline.frenet_frame_at(t);
            min_k = min_k.min(frame.curvature);
            max_k = max_k.max(frame.curvature);
            sum_k += frame.curvature;
            min_tau = min_tau.min(frame.torsion);
            max_tau = max_tau.max(frame.torsion);
            let sign = if frame.torsion > 0.0 { 1i32 } else if frame.torsion < 0.0 { -1 } else { 0 };
            if prev_sign != 0 && sign != 0 && sign != prev_sign { inflections += 1; }
            if sign != 0 { prev_sign = sign; }
            let p = frame.position;
            bb_min = bb_min.min(p);
            bb_max = bb_max.max(p);
        }

        let pts: Vec<Vec3> = (0..=samples).map(|i| spline.evaluate(i as f32 / samples as f32)).collect();
        let si = detect_self_intersection_coarse(&pts, 0.1);

        SplineAnalysisReport {
            total_arc_length:    total,
            min_curvature:       if min_k  == f32::MAX { 0.0 } else { min_k },
            max_curvature:       if max_k  == f32::MIN { 0.0 } else { max_k },
            mean_curvature:      sum_k / (samples + 1) as f32,
            min_torsion:         if min_tau == f32::MAX { 0.0 } else { min_tau },
            max_torsion:         if max_tau == f32::MIN { 0.0 } else { max_tau },
            inflection_count:    inflections,
            control_point_count: spline.control_points.len(),
            self_intersection:   si,
            bounding_box_min:    if bb_min == Vec3::splat(f32::MAX) { Vec3::ZERO } else { bb_min },
            bounding_box_max:    if bb_max == Vec3::splat(f32::MIN) { Vec3::ZERO } else { bb_max },
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "Arc length: {:.3}  CPs: {}  k[{:.4},{:.4}] tau[{:.4},{:.4}]  inflections: {}  si: {}",
            self.total_arc_length, self.control_point_count,
            self.min_curvature, self.max_curvature,
            self.min_torsion, self.max_torsion,
            self.inflection_count, self.self_intersection
        )
    }
}

fn detect_self_intersection_coarse(pts: &[Vec3], cell: f32) -> bool {
    let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
    for (i, p) in pts.iter().enumerate() {
        let key = ((p.x / cell) as i32, (p.y / cell) as i32, (p.z / cell) as i32);
        grid.entry(key).or_default().push(i);
    }
    for indices in grid.values() {
        for &a in indices {
            for &b in indices {
                if b > a + 2 { return true; }
            }
        }
    }
    false
}

// ============================================================
// FBM NOISE PERTURBATION
// ============================================================

pub fn perturb_spline_fbm(spline: &mut CatmullRomSpline, amplitude: f32, frequency: f32, octaves: u32, seed: u32) {
    for (i, cp) in spline.control_points.iter_mut().enumerate() {
        let fi = i as f32 * frequency + seed as f32 * 1.618;
        let mut disp = Vec3::ZERO;
        let mut amp  = amplitude;
        let mut freq = 1.0f32;
        for _ in 0..octaves {
            disp.x += value_noise_1d(fi * freq + 0.0)  * amp;
            disp.y += value_noise_1d(fi * freq + 13.7) * amp;
            disp.z += value_noise_1d(fi * freq + 27.3) * amp;
            amp  *= 0.5;
            freq *= 2.0;
        }
        cp.position += disp;
    }
    spline.rebuild_arc_length_table();
}

// ============================================================
// SPLINE MORPH TARGETS
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineMorphTarget {
    pub name:    String,
    pub offsets: Vec<Vec3>,
    pub weight:  f32,
}

impl SplineMorphTarget {
    pub fn new(name: &str, base: &CatmullRomSpline) -> Self {
        let offsets = vec![Vec3::ZERO; base.control_points.len()];
        SplineMorphTarget { name: name.to_string(), offsets, weight: 0.0 }
    }

    pub fn set_offset(&mut self, idx: usize, offset: Vec3) {
        if idx < self.offsets.len() { self.offsets[idx] = offset; }
    }
}

pub fn apply_morph_targets(base: &CatmullRomSpline, morphs: &[SplineMorphTarget]) -> CatmullRomSpline {
    let mut result = base.clone();
    for m in morphs {
        for (i, cp) in result.control_points.iter_mut().enumerate() {
            if let Some(&off) = m.offsets.get(i) { cp.position += off * m.weight; }
        }
    }
    result.rebuild_arc_length_table();
    result
}

// ============================================================
// SPLINE EVENT TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineEvent {
    pub id:        u64,
    pub name:      String,
    pub t:         f32,
    pub arc_s:     f32,
    pub payload:   String,
    pub triggered: bool,
}

pub struct SplineEventTrack {
    pub events: Vec<SplineEvent>,
    next_id:    u64,
}

impl SplineEventTrack {
    pub fn new() -> Self { SplineEventTrack { events: Vec::new(), next_id: 1 } }

    pub fn add_event(&mut self, name: &str, t: f32, arc_s: f32, payload: &str) -> u64 {
        let id = self.next_id; self.next_id += 1;
        self.events.push(SplineEvent {
            id, name: name.to_string(), t, arc_s, payload: payload.to_string(), triggered: false
        });
        id
    }

    pub fn reset(&mut self) { for e in &mut self.events { e.triggered = false; } }

    pub fn poll(&mut self, prev_s: f32, cur_s: f32) -> Vec<SplineEvent> {
        let mut fired = Vec::new();
        for e in &mut self.events {
            if !e.triggered && e.arc_s >= prev_s && e.arc_s < cur_s {
                e.triggered = true;
                fired.push(e.clone());
            }
        }
        fired
    }

    pub fn sort_by_t(&mut self) {
        self.events.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    }
}

// ============================================================
// WIND FORCE & COLLISION
// ============================================================

pub fn apply_wind(dyn_chain: &mut SplineDynamics, wind_dir: Vec3, wind_speed: f32, drag_coeff: f32, dt: f32) {
    let wind_vel = wind_dir.normalize_or_zero() * wind_speed;
    for p in &mut dyn_chain.particles {
        if p.pinned { continue; }
        let rel  = wind_vel - p.velocity;
        let drag = rel * drag_coeff;
        p.velocity += drag * dt;
    }
}

pub fn collide_with_sphere(dyn_chain: &mut SplineDynamics, centre: Vec3, radius: f32) {
    for p in &mut dyn_chain.particles {
        if p.pinned { continue; }
        let d   = p.position - centre;
        let len = d.length();
        if len < radius {
            p.position = centre + d.normalize_or_zero() * radius;
            let n  = d.normalize_or_zero();
            let vn = p.velocity.dot(n);
            if vn < 0.0 { p.velocity -= n * vn; }
        }
    }
}

pub fn collide_with_plane_y(dyn_chain: &mut SplineDynamics, y: f32, restitution: f32) {
    for p in &mut dyn_chain.particles {
        if p.pinned { continue; }
        if p.position.y < y {
            p.position.y = y;
            if p.velocity.y < 0.0 { p.velocity.y = -p.velocity.y * restitution; }
        }
    }
}

// ============================================================
// CSV SERIALISATION
// ============================================================

pub fn spline_to_csv(spline: &CatmullRomSpline) -> String {
    let mut out = String::new();
    out.push_str(&format!("#catmull,alpha={},closed={}\n", spline.alpha, spline.closed));
    for cp in &spline.control_points {
        out.push_str(&format!("{},{},{},{}\n", cp.position.x, cp.position.y, cp.position.z, cp.weight));
    }
    out
}

pub fn spline_from_csv(csv: &str) -> Result<CatmullRomSpline, String> {
    let mut alpha  = 0.5f32;
    let mut closed = false;
    let mut cps    = Vec::new();
    for line in csv.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if line.starts_with('#') {
            if let Some(a) = line.find("alpha=") {
                let rest = &line[a + 6..];
                let end  = rest.find(',').unwrap_or(rest.len());
                alpha    = rest[..end].parse().unwrap_or(0.5);
            }
            if line.contains("closed=true") { closed = true; }
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 { return Err(format!("Bad line: {}", line)); }
        let x: f32 = parts[0].parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
        let y: f32 = parts[1].parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
        let z: f32 = parts[2].parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
        let w: f32 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        cps.push(ControlPoint { position: Vec3::new(x, y, z), weight: w, ..ControlPoint::new(Vec3::new(x, y, z)) });
    }
    let pts: Vec<Vec3> = cps.iter().map(|c| c.position).collect();
    Ok(CatmullRomSpline::new(pts, alpha, closed))
}

// ============================================================
// SPLINE EDITOR EXTENDED COMMANDS
// ============================================================

impl SplineEditor {
    pub fn cmd_taubin_smooth(&mut self, id: u64, lambda: f32, mu: f32, iterations: u32) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) {
            let _ = spline.control_points.iter().map(|c| c.position).collect::<Vec<_>>();
            taubin_smooth_catmull(spline, lambda, mu, iterations);
        }
    }

    pub fn cmd_equidistribute(&mut self, id: u64, new_count: usize) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) {
            let _ = spline.control_points.iter().map(|c| c.position).collect::<Vec<_>>();
            equidistribute_catmull(spline, new_count);
        }
    }

    pub fn cmd_snap_grid(&mut self, id: u64, cell_size: f32) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) {
            let _ = spline.control_points.iter().map(|c| c.position).collect::<Vec<_>>();
            snap_to_grid(spline, cell_size);
        }
    }

    pub fn cmd_mirror(&mut self, id: u64, axis: u8) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) {
            let _ = spline.control_points.iter().map(|c| c.position).collect::<Vec<_>>();
            match axis {
                0 => mirror_spline_x(spline),
                1 => mirror_spline_y(spline),
                _ => mirror_spline_z(spline),
            }
        }
    }

    pub fn cmd_perturb_fbm(&mut self, id: u64, amplitude: f32, freq: f32, octaves: u32, seed: u32) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) {
            let _ = spline.control_points.iter().map(|c| c.position).collect::<Vec<_>>();
            perturb_spline_fbm(spline, amplitude, freq, octaves, seed);
        }
    }

    pub fn cmd_translate(&mut self, id: u64, delta: Vec3) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) { translate_spline(spline, delta); }
    }

    pub fn cmd_rotate(&mut self, id: u64, rot: Quat) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) { rotate_spline(spline, rot); }
    }

    pub fn cmd_scale(&mut self, id: u64, scale: f32) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) { scale_spline_uniform(spline, scale); }
    }

    pub fn cmd_analyze(&self, id: u64, samples: usize) -> Option<SplineAnalysisReport> {
        self.catmull_splines.get(&id).map(|s| SplineAnalysisReport::compute(s, samples))
    }

    pub fn cmd_fit_bezier(&mut self, id: u64, samples: usize, max_error: f32) -> Option<u64> {
        let dense = {
            let s = self.catmull_splines.get(&id)?;
            sample_uniform_arc_length(s, samples)
        };
        let bezier = fit_piecewise_cubic_bezier(&dense, max_error);
        let new_id = rand_id();
        self.bezier_splines.insert(new_id, bezier);
        Some(new_id)
    }

    pub fn cmd_export_csv(&self) -> String {
        let mut out = String::new();
        for (id, spline) in &self.catmull_splines {
            out.push_str(&format!("## spline_id={}\n", id));
            out.push_str(&spline_to_csv(spline));
        }
        out
    }

    pub fn cmd_import_csv(&mut self, csv: &str) {
        let mut current = String::new();
        for line in csv.lines() {
            if line.starts_with("## spline_id=") {
                if !current.is_empty() {
                    if let Ok(s) = spline_from_csv(&current) {
                        let id = rand_id();
                        self.catmull_splines.insert(id, s);
                    }
                    current.clear();
                }
            } else {
                current.push_str(line);
                current.push('\n');
            }
        }
        if !current.is_empty() {
            if let Ok(s) = spline_from_csv(&current) {
                let id = rand_id();
                self.catmull_splines.insert(id, s);
            }
        }
    }

    pub fn spline_bounding_box(&self, id: u64) -> Option<(Vec3, Vec3)> {
        let s = self.catmull_splines.get(&id)?;
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::splat(f32::MIN);
        for cp in &s.control_points { mn = mn.min(cp.position); mx = mx.max(cp.position); }
        if mn == Vec3::splat(f32::MAX) { None } else { Some((mn, mx)) }
    }

    pub fn clear_all(&mut self) {
        self.catmull_splines.clear();
        self.bezier_splines.clear();
        self.bsplines.clear();
        self.nurbs_splines.clear();
        self.hermite_splines.clear();
        self.generated_meshes.clear();
    }

    pub fn total_control_points(&self) -> usize {
        self.catmull_splines.values().map(|s| s.control_points.len()).sum()
    }

    pub fn cmd_reverse(&mut self, id: u64) {
        if let Some(spline) = self.catmull_splines.get_mut(&id) {
            spline.control_points.reverse();
            spline.rebuild_arc_length_table();
        }
    }

    pub fn cmd_duplicate(&mut self, id: u64, offset: Vec3) -> Option<u64> {
        let mut s = self.catmull_splines.get(&id)?.clone();
        translate_spline(&mut s, offset);
        let new_id = rand_id();
        self.catmull_splines.insert(new_id, s);
        Some(new_id)
    }

    pub fn cmd_weld(&mut self, id_a: u64, id_b: u64, threshold: f32) -> Option<u64> {
        let a = self.catmull_splines.get(&id_a)?.clone();
        let b = self.catmull_splines.get(&id_b)?.clone();
        let end_a   = a.control_points.last()?.position;
        let start_b = b.control_points.first()?.position;
        if (end_a - start_b).length() > threshold { return None; }
        let joined = CatmullRomSpline::join(a, b);
        let new_id = rand_id();
        self.catmull_splines.insert(new_id, joined);
        Some(new_id)
    }

    pub fn arc_length(&self, id: u64) -> f32 {
        self.catmull_splines.get(&id).map(|s| {
            let table = build_arc_length_table(512, &|t| s.evaluate(t));
            table.last().map(|&(_, l)| l).unwrap_or(0.0)
        }).unwrap_or(0.0)
    }
}

// ============================================================
// ADVANCED UNIT TESTS
// ============================================================

#[cfg(test)]
mod tests_spline_advanced {
    use super::*;

    fn simple_line(n: usize) -> CatmullRomSpline {
        CatmullRomSpline {
            control_points: (0..n).map(|i| ControlPoint {
                position: Vec3::new(i as f32, 0.0, 0.0), weight: 1.0, tension: 0.0
            }).collect(),
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        }
    }

    #[test]
    fn test_laplacian_smooth_middle_moves() {
        let mut s = CatmullRomSpline {
            control_points: vec![
                ControlPoint { position: Vec3::new(0.0, 0.0, 0.0), weight: 1.0, tension: 0.0 },
                ControlPoint { position: Vec3::new(1.0, 2.0, 0.0), weight: 1.0, tension: 0.0 },
                ControlPoint { position: Vec3::new(2.0, 0.0, 0.0), weight: 1.0, tension: 0.0 },
            ],
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        };
        laplacian_smooth_catmull(&mut s, 0.5, 1);
        assert!((s.control_points[1].position.y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_equidistribute_correct_count() {
        let mut s = simple_line(10);
        equidistribute_catmull(&mut s, 5);
        assert_eq!(s.control_points.len(), 5);
    }

    #[test]
    fn test_fit_cubic_bezier_endpoints() {
        let pts = vec![Vec3::new(0.0,0.0,0.0), Vec3::new(0.5,1.0,0.0), Vec3::new(1.0,0.0,0.0)];
        let seg = fit_cubic_bezier(&pts);
        assert!((seg[0] - Vec3::new(0.0,0.0,0.0)).length() < 1e-5);
        assert!((seg[3] - Vec3::new(1.0,0.0,0.0)).length() < 1e-5);
    }

    #[test]
    fn test_spline_csv_round_trip() {
        let s = CatmullRomSpline {
            control_points: vec![
                ControlPoint { position: Vec3::new(1.0,2.0,3.0), weight: 1.0, tension: 0.0 },
                ControlPoint { position: Vec3::new(4.0,5.0,6.0), weight: 1.0, tension: 0.0 },
            ],
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        };
        let csv = spline_to_csv(&s);
        let s2  = spline_from_csv(&csv).unwrap();
        assert_eq!(s2.control_points.len(), 2);
        assert!((s2.control_points[0].position - Vec3::new(1.0,2.0,3.0)).length() < 1e-4);
    }

    #[test]
    fn test_spline_dynamics_gravity_falls() {
        let s = CatmullRomSpline {
            control_points: vec![
                ControlPoint { position: Vec3::new(0.0,10.0,0.0), weight: 1.0, tension: 0.0 },
                ControlPoint { position: Vec3::new(1.0,10.0,0.0), weight: 1.0, tension: 0.0 },
            ],
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        };
        let mut chain = SplineDynamics::from_catmull(&s, 100.0, 0.1);
        chain.particles[0].pinned = true;
        chain.step(0.016);
        assert!(chain.particles[1].position.y < 10.0);
    }

    #[test]
    fn test_ribbon_mesh_vertex_count() {
        let s = simple_line(3);
        let ribbon = RibbonMesh::generate(&s, 8, &|_| 0.1);
        assert_eq!(ribbon.vertices.len(), (8 + 1) * 2);
    }

    #[test]
    fn test_analysis_report_positive_arc_length() {
        let s = simple_line(5);
        let rep = SplineAnalysisReport::compute(&s, 64);
        assert!(rep.total_arc_length > 0.0);
    }

    #[test]
    fn test_snap_to_grid_rounds() {
        let mut s = CatmullRomSpline {
            control_points: vec![ControlPoint { position: Vec3::new(0.3,1.7,-0.1), weight: 1.0, tension: 0.0 }],
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        };
        snap_to_grid(&mut s, 1.0);
        assert!((s.control_points[0].position.x).abs() < 1e-5);
        assert!((s.control_points[0].position.y - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_clothoid_sample_count() {
        let pts = sample_clothoid(1.0, 50, false);
        assert_eq!(pts.len(), 50);
        assert!(pts[0].length() < 0.01);
    }

    #[test]
    fn test_offset_spline_xz_point_count() {
        let s   = simple_line(5);
        let off = offset_spline_xz(&s, 0.5, 20);
        assert_eq!(off.control_points.len(), 20);
    }

    #[test]
    fn test_fabrik_reaches_target() {
        let joints = vec![Vec3::ZERO, Vec3::new(1.0,0.0,0.0), Vec3::new(2.0,0.0,0.0)];
        let mut chain = SplineIKChain::new(joints);
        let target = Vec3::new(1.5, 1.0, 0.0);
        chain.solve_fabrik(target, 20, 1e-3);
        let end = *chain.joints.last().unwrap();
        assert!((end - target).length() < 0.05);
    }

    #[test]
    fn test_spline_lattice_midpoint() {
        let rail_a = CatmullRomSpline {
            control_points: vec![
                ControlPoint { position: Vec3::new(0.0,0.0,0.0), weight: 1.0, tension: 0.0 },
                ControlPoint { position: Vec3::new(1.0,0.0,0.0), weight: 1.0, tension: 0.0 },
            ],
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        };
        let rail_b = CatmullRomSpline {
            control_points: vec![
                ControlPoint { position: Vec3::new(0.0,1.0,0.0), weight: 1.0, tension: 0.0 },
                ControlPoint { position: Vec3::new(1.0,1.0,0.0), weight: 1.0, tension: 0.0 },
            ],
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        };
        let lattice = SplineLattice { rail_a, rail_b };
        let mid = lattice.evaluate(0.0, 0.5);
        assert!((mid.y - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_mirror_spline_x_reverses() {
        let mut s = simple_line(3);
        let orig_last = s.control_points.last().unwrap().position;
        mirror_spline_x(&mut s);
        let new_first = s.control_points.first().unwrap().position;
        assert!((new_first.x + orig_last.x).abs() < 1e-5);
    }

    #[test]
    fn test_spline_total_variation_positive() {
        let s = simple_line(4);
        let tv = spline_total_variation(&s);
        assert!(tv > 0.0);
    }
}

// ============================================================
// SPLINE SEGMENT ANNOTATOR (metadata per segment)
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct SegmentAnnotation {
    pub label:       String,
    pub speed_limit: f32,
    pub terrain_tag: String,
    pub danger:      bool,
}

pub struct SplineAnnotator {
    pub annotations: Vec<(f32, f32, SegmentAnnotation)>,  // (t_start, t_end, data)
}

impl SplineAnnotator {
    pub fn new() -> Self { SplineAnnotator { annotations: Vec::new() } }

    pub fn add(&mut self, t_start: f32, t_end: f32, ann: SegmentAnnotation) {
        self.annotations.push((t_start.min(t_end), t_start.max(t_end), ann));
    }

    pub fn query(&self, t: f32) -> Vec<&SegmentAnnotation> {
        self.annotations.iter()
            .filter(|(s, e, _)| t >= *s && t <= *e)
            .map(|(_, _, ann)| ann)
            .collect()
    }

    pub fn speed_limit_at(&self, t: f32) -> f32 {
        self.query(t).iter()
            .map(|a| a.speed_limit)
            .fold(f32::MAX, f32::min)
    }
}

// ============================================================
// SPLINE BUNDLE (set of parallel splines)
// ============================================================

pub struct SplineBundle {
    pub splines:   Vec<CatmullRomSpline>,
    pub offsets:   Vec<f32>,  // lateral offset per lane
    pub separator: f32,       // world-space lane width
}

impl SplineBundle {
    /// Build a bundle from a centre-line and evenly-spaced offsets.
    pub fn from_centre(centre: &CatmullRomSpline, lane_count: usize, lane_width: f32, samples: usize) -> Self {
        let half = (lane_count as f32 - 1.0) * 0.5 * lane_width;
        let mut splines = Vec::new();
        let mut offsets = Vec::new();
        for i in 0..lane_count {
            let off = i as f32 * lane_width - half;
            offsets.push(off);
            splines.push(offset_spline_xz(centre, off, samples));
        }
        SplineBundle { splines, offsets, separator: lane_width }
    }

    pub fn lane_count(&self) -> usize { self.splines.len() }

    pub fn evaluate(&self, lane: usize, t: f32) -> Option<Vec3> {
        self.splines.get(lane).map(|s| s.evaluate(t))
    }

    pub fn nearest_lane(&self, point: Vec3) -> usize {
        let mut best = 0;
        let mut best_dist = f32::MAX;
        for (i, s) in self.splines.iter().enumerate() {
            let (t, _) = s.nearest_point(point);
            let d = (s.evaluate(t) - point).length();
            if d < best_dist { best_dist = d; best = i; }
        }
        best
    }
}

// ============================================================
// SPLINE PREVIEW WIDGET DATA
// ============================================================

/// A compact view of a spline for UI display: discretised polyline + tangent arrows.
pub struct SplinePreviewData {
    pub polyline:   Vec<Vec3>,
    pub tangents:   Vec<Vec3>,
    pub normals:    Vec<Vec3>,
    pub curvatures: Vec<f32>,
}

impl SplinePreviewData {
    pub fn from_spline(spline: &CatmullRomSpline, resolution: usize) -> Self {
        let mut polyline   = Vec::with_capacity(resolution + 1);
        let mut tangents   = Vec::with_capacity(resolution + 1);
        let mut normals    = Vec::with_capacity(resolution + 1);
        let mut curvatures = Vec::with_capacity(resolution + 1);
        for i in 0..=resolution {
            let t     = i as f32 / resolution as f32;
            let frame = spline.frenet_frame_at(t);
            polyline.push(frame.position);
            tangents.push(frame.tangent);
            normals.push(frame.normal);
            curvatures.push(frame.curvature);
        }
        SplinePreviewData { polyline, tangents, normals, curvatures }
    }

    /// Average curvature.
    pub fn mean_curvature(&self) -> f32 {
        if self.curvatures.is_empty() { return 0.0; }
        self.curvatures.iter().sum::<f32>() / self.curvatures.len() as f32
    }

    /// Axis-aligned bounding box.
    pub fn aabb(&self) -> (Vec3, Vec3) {
        let mut mn = Vec3::splat(f32::MAX);
        let mut mx = Vec3::splat(f32::MIN);
        for &p in &self.polyline { mn = mn.min(p); mx = mx.max(p); }
        if mn == Vec3::splat(f32::MAX) { (Vec3::ZERO, Vec3::ZERO) } else { (mn, mx) }
    }
}

// ============================================================
// SPLINE PAINT TOOL (stroke to spline conversion)
// ============================================================

/// Convert a sequence of raw stroke samples (mouse / pen) to a smoothed CatmullRom spline.
pub struct SplinePaintTool {
    pub raw_samples:    Vec<Vec3>,
    pub simplify_eps:   f32,
    pub smooth_passes:  u32,
    pub smooth_lambda:  f32,
}

impl SplinePaintTool {
    pub fn new(simplify_eps: f32, smooth_passes: u32, smooth_lambda: f32) -> Self {
        SplinePaintTool { raw_samples: Vec::new(), simplify_eps, smooth_passes, smooth_lambda }
    }

    pub fn add_sample(&mut self, p: Vec3) {
        self.raw_samples.push(p);
    }

    /// Finalise: simplify + smooth → CatmullRomSpline.
    pub fn finish(&mut self) -> CatmullRomSpline {
        let simplified = simplify_polyline(&self.raw_samples, self.simplify_eps);
        let mut spline = CatmullRomSpline::new(simplified, 0.5, false);
        laplacian_smooth_catmull(&mut spline, self.smooth_lambda, self.smooth_passes);
        self.raw_samples.clear();
        spline
    }
}

// ============================================================
// SPLINE COORDINATE FRAME SEQUENCE
// ============================================================

/// A pre-baked sequence of Frenet frames at uniform arc-length intervals.
pub struct BakedFrenetFrames {
    pub frames:      Vec<FrenetFrame>,
    pub arc_step:    f32,
    pub total_length: f32,
}

impl BakedFrenetFrames {
    pub fn bake(spline: &CatmullRomSpline, samples: usize) -> Self {
        let table       = build_arc_length_table(samples * 4, &|t| spline.evaluate(t));
        let total_length = table.last().map(|&(_, s)| s).unwrap_or(0.0);
        let arc_step    = if samples > 1 { total_length / (samples - 1) as f32 } else { 0.0 };
        let frames: Vec<FrenetFrame> = (0..samples).map(|i| {
            let s = i as f32 * arc_step;
            let t = arc_length_to_t(&table, s);
            spline.frenet_frame_at(t)
        }).collect();
        BakedFrenetFrames { frames, arc_step, total_length }
    }

    /// Sample a frame at arc-length position `s` using linear interpolation.
    pub fn sample(&self, s: f32) -> Option<FrenetFrame> {
        if self.frames.is_empty() || self.arc_step < 1e-10 { return None; }
        let idx = (s / self.arc_step) as usize;
        if idx + 1 >= self.frames.len() { return self.frames.last().cloned(); }
        let t   = (s / self.arc_step) - idx as f32;
        let a   = &self.frames[idx];
        let b   = &self.frames[idx + 1];
        Some(FrenetFrame {
            position:  a.position.lerp(b.position, t),
            tangent:   a.tangent.lerp(b.tangent, t).normalize_or_zero(),
            normal:    a.normal.lerp(b.normal, t).normalize_or_zero(),
            binormal:  a.binormal.lerp(b.binormal, t).normalize_or_zero(),
            curvature: a.curvature + (b.curvature - a.curvature) * t,
            torsion:   a.torsion   + (b.torsion   - a.torsion)   * t,
        })
    }
}

// ============================================================
// SPLINE OUTLINE (expanded polygon)
// ============================================================

/// Generate a 2-D outline (two parallel polylines) from a spline in the XZ plane.
pub fn spline_outline_xz(spline: &CatmullRomSpline, half_width: f32, samples: usize) -> (Vec<Vec3>, Vec<Vec3>) {
    let table = build_arc_length_table(samples * 4, &|t| spline.evaluate(t));
    let total  = table.last().map(|&(_, s)| s).unwrap_or(0.0);
    let mut left  = Vec::with_capacity(samples);
    let mut right = Vec::with_capacity(samples);
    for i in 0..samples {
        let s  = total * i as f32 / (samples - 1).max(1) as f32;
        let t  = arc_length_to_t(&table, s);
        let p  = spline.evaluate(t);
        let tn = spline.evaluate_derivative(t);
        let n  = Vec3::new(-tn.z, 0.0, tn.x).normalize_or_zero();
        left.push(p - n * half_width);
        right.push(p + n * half_width);
    }
    (left, right)
}

// ============================================================
// SPLINE INTERSECTION UTILITIES (standalone)
// ============================================================

/// Fast segment-segment distance (3-D); returns squared minimum distance.
pub fn segment_segment_dist_sq(p0: Vec3, p1: Vec3, q0: Vec3, q1: Vec3) -> f32 {
    let d1 = p1 - p0;
    let d2 = q1 - q0;
    let r  = p0 - q0;
    let a  = d1.dot(d1);
    let e  = d2.dot(d2);
    let f  = d2.dot(r);
    let (s, t);
    if a < 1e-10 && e < 1e-10 {
        s = 0.0; t = 0.0;
    } else if a < 1e-10 {
        s = 0.0; t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e < 1e-10 {
            t = 0.0; s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b  = d1.dot(d2);
            let denom = a * e - b * b;
            s = if denom.abs() > 1e-10 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            t = (b * s + f) / e;
            let (ss, tt);
            if t < 0.0 {
                tt = 0.0; ss = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                tt = 1.0; ss = ((b - c) / a).clamp(0.0, 1.0);
            } else {
                ss = s; tt = t;
            }
            let _ = (s, t);
            let cp1 = p0 + d1 * ss;
            let cp2 = q0 + d2 * tt;
            return (cp1 - cp2).length_squared();
        }
    }
    let cp1 = p0 + d1 * s;
    let cp2 = q0 + d2 * t;
    (cp1 - cp2).length_squared()
}

/// Find approximate intersection parameter pairs between two CatmullRom splines.
pub fn spline_spline_intersection_params(
    a: &CatmullRomSpline,
    b: &CatmullRomSpline,
    coarse_steps: usize,
    tol: f32,
) -> Vec<(f32, f32)> {
    let mut candidates = Vec::new();
    let step = 1.0 / coarse_steps as f32;
    for i in 0..coarse_steps {
        for j in 0..coarse_steps {
            let ta0 = i as f32 * step;
            let ta1 = ta0 + step;
            let tb0 = j as f32 * step;
            let tb1 = tb0 + step;
            let pa0 = a.evaluate(ta0); let pa1 = a.evaluate(ta1);
            let pb0 = b.evaluate(tb0); let pb1 = b.evaluate(tb1);
            if segment_segment_dist_sq(pa0, pa1, pb0, pb1) < tol * tol {
                candidates.push(((ta0 + ta1) * 0.5, (tb0 + tb1) * 0.5));
            }
        }
    }

    // Newton-Raphson refinement
    let mut results = Vec::new();
    for (mut ta, mut tb) in candidates {
        for _ in 0..20 {
            let fa  = a.evaluate(ta);
            let fb  = b.evaluate(tb);
            let dfa = a.evaluate_derivative(ta);
            let dfb = b.evaluate_derivative(tb);
            let res = fa - fb;
            // J = [dfa | -dfb], solve 2x2 least-squares in XZ
            let j00 =  dfa.x; let j01 = -dfb.x;
            let j10 =  dfa.z; let j11 = -dfb.z;
            let det = j00 * j11 - j01 * j10;
            if det.abs() < 1e-10 { break; }
            let dta = ( j11 * res.x - j01 * res.z) / det;
            let dtb = (-j10 * res.x + j00 * res.z) / det;
            ta -= dta;
            tb -= dtb;
            ta = ta.clamp(0.0, 1.0);
            tb = tb.clamp(0.0, 1.0);
            if dta.abs() < 1e-6 && dtb.abs() < 1e-6 { break; }
        }
        let dist = (a.evaluate(ta) - b.evaluate(tb)).length();
        if dist < tol * 2.0 { results.push((ta, tb)); }
    }
    results
}

// ============================================================
// MORE UNIT TESTS
// ============================================================

#[cfg(test)]
mod tests_spline_extra {
    use super::*;

    #[test]
    fn test_speed_profile_linear_interpolation() {
        let mut p = SpeedProfile::new();
        p.add_key(0.0, 0.0);
        p.add_key(1.0, 10.0);
        assert!((p.evaluate(0.5) - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_speed_profile_total_time_positive() {
        let p = SpeedProfile::quintic_ease(20.0);
        let t = p.total_travel_time(100.0, 100);
        assert!(t > 0.0);
    }

    #[test]
    fn test_spline_bundle_lane_count() {
        let centre = CatmullRomSpline {
            control_points: (0..4).map(|i| crate::editor::spline_editor::ControlPoint {
                position: glam::Vec3::new(i as f32, 0.0, 0.0), weight: 1.0, tension: 0.0
            }).collect(),
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        };
        let bundle = SplineBundle::from_centre(&centre, 3, 1.0, 32);
        assert_eq!(bundle.lane_count(), 3);
    }

    #[test]
    fn test_preview_data_aabb() {
        let s = super::tests_spline_advanced::simple_line(4);
        let preview = SplinePreviewData::from_spline(&s, 32);
        let (mn, mx) = preview.aabb();
        assert!(mx.x > mn.x || mx.y >= mn.y);
    }

    #[test]
    fn test_outline_xz_point_count() {
        let s = super::tests_spline_advanced::simple_line(4);
        let (left, right) = spline_outline_xz(&s, 0.5, 16);
        assert_eq!(left.len(), 16);
        assert_eq!(right.len(), 16);
    }

    #[test]
    fn test_baked_frenet_sample() {
        let s = super::tests_spline_advanced::simple_line(5);
        let baked = BakedFrenetFrames::bake(&s, 64);
        let f = baked.sample(baked.total_length * 0.5);
        assert!(f.is_some());
    }

    #[test]
    fn test_segment_segment_dist_sq_parallel() {
        let d = segment_segment_dist_sq(
            Vec3::new(0.0,0.0,0.0), Vec3::new(1.0,0.0,0.0),
            Vec3::new(0.0,1.0,0.0), Vec3::new(1.0,1.0,0.0),
        );
        assert!((d - 1.0).abs() < 0.01);
    }
}

// ============================================================
// SPLINE GRADIENT MATERIAL (colour ramp along arc-length)
// ============================================================

#[derive(Clone, Debug)]
pub struct SplineColorKey {
    pub t:     f32,
    pub color: Vec4,
}

pub struct SplineColorRamp {
    pub keys: Vec<SplineColorKey>,
}

impl SplineColorRamp {
    pub fn new() -> Self { SplineColorRamp { keys: Vec::new() } }

    pub fn add_key(&mut self, t: f32, color: Vec4) {
        let pos = self.keys.partition_point(|k| k.t < t);
        self.keys.insert(pos, SplineColorKey { t, color });
    }

    pub fn evaluate(&self, t: f32) -> Vec4 {
        if self.keys.is_empty() { return Vec4::ONE; }
        let t = t.clamp(0.0, 1.0);
        let idx = self.keys.partition_point(|k| k.t <= t);
        if idx == 0 { return self.keys[0].color; }
        if idx >= self.keys.len() { return self.keys.last().unwrap().color; }
        let a  = &self.keys[idx - 1];
        let b  = &self.keys[idx];
        let f  = if (b.t - a.t).abs() < 1e-10 { 0.0 } else { (t - a.t) / (b.t - a.t) };
        a.color.lerp(b.color, f)
    }
}

// ============================================================
// SPLINE MEASUREMENT TOOLS
// ============================================================

/// Distance from an external point to the nearest point on a spline.
pub fn point_to_spline_distance(spline: &CatmullRomSpline, point: Vec3, steps: usize) -> f32 {
    let (t, _) = spline.nearest_point(point);
    let np = spline.evaluate(t);
    (point - np).length()
}

/// Closest pair of points between two splines.
pub fn spline_spline_min_distance(a: &CatmullRomSpline, b: &CatmullRomSpline, steps: usize) -> (f32, f32, f32) {
    let mut min_dist = f32::MAX;
    let mut best_ta  = 0.0f32;
    let mut best_tb  = 0.0f32;
    for i in 0..=steps {
        let ta = i as f32 / steps as f32;
        let pa = a.evaluate(ta);
        let (tb_best, _) = b.nearest_point(pa);
        let pb_best = b.evaluate(tb_best);
        let d = (pa - pb_best).length();
        if d < min_dist { min_dist = d; best_ta = ta; best_tb = tb_best; }
    }
    (min_dist, best_ta, best_tb)
}

// ============================================================
// SPLINE SUBDIVISION (uniform T)
// ============================================================

/// Insert `n` new control points via uniform T subdivision.
pub fn subdivide_catmull(spline: &mut CatmullRomSpline, n: u32) {
    for _ in 0..n {
        let old: Vec<Vec3> = spline.control_points.iter().map(|cp| cp.position).collect();
        if old.len() < 2 { break; }
        let mut new_pts = Vec::with_capacity(old.len() * 2 - 1);
        for i in 0..old.len() - 1 {
            new_pts.push(old[i]);
            new_pts.push((old[i] + old[i + 1]) * 0.5);
        }
        new_pts.push(*old.last().unwrap());
        spline.control_points = new_pts.into_iter().map(|p| ControlPoint::new(p)).collect();
    }
    spline.rebuild_arc_length_table();
}

// ============================================================
// SPLINE META-DATA STORE
// ============================================================

/// Arbitrary key-value metadata attached to a spline.
pub struct SplineMetadata {
    pub data: HashMap<String, String>,
}

impl SplineMetadata {
    pub fn new() -> Self { SplineMetadata { data: HashMap::new() } }

    pub fn set(&mut self, key: &str, value: &str) { self.data.insert(key.to_string(), value.to_string()); }

    pub fn get(&self, key: &str) -> Option<&str> { self.data.get(key).map(String::as_str) }

    pub fn get_f32(&self, key: &str) -> Option<f32> { self.data.get(key)?.parse().ok() }

    pub fn get_bool(&self, key: &str) -> bool {
        self.data.get(key).map(|v| v == "true").unwrap_or(false)
    }
}

// ============================================================
// SPLINE CAMERA PATH EVALUATOR
// ============================================================

/// Evaluate a camera's world-space position and look-at target along a camera rail.
pub fn camera_path_evaluate(
    position_spline: &CatmullRomSpline,
    target_spline:   &CatmullRomSpline,
    t:               f32,
) -> (Vec3, Vec3, Mat4) {
    let pos    = position_spline.evaluate(t);
    let target = target_spline.evaluate(t);
    let forward = (target - pos).normalize_or_zero();
    let up      = Vec3::Y;
    let right   = forward.cross(up).normalize_or_zero();
    let true_up = right.cross(forward).normalize_or_zero();
    let mat = Mat4::from_cols(
        right.extend(0.0),
        true_up.extend(0.0),
        (-forward).extend(0.0),
        pos.extend(1.0),
    );
    (pos, target, mat)
}

// ============================================================
// SPLINE REPARAMETERISATION BY CURVATURE
// ============================================================

/// Reparameterise spline samples so high-curvature regions get more samples.
pub fn reparametrise_by_curvature(
    spline: &CatmullRomSpline,
    n:      usize,
    weight: f32,
) -> Vec<f32> {
    // Build curvature density: density(t) = 1 + weight * kappa(t)
    let raw: Vec<(f32, f32)> = (0..=n * 4).map(|i| {
        let t = i as f32 / (n * 4) as f32;
        let kappa = spline.frenet_frame_at(t).curvature;
        (t, 1.0 + weight * kappa)
    }).collect();

    // Integrate density to get cumulative weight
    let mut cum: Vec<f32> = Vec::with_capacity(raw.len());
    let mut acc = 0.0f32;
    cum.push(0.0);
    for i in 1..raw.len() {
        let dt = raw[i].0 - raw[i - 1].0;
        acc += (raw[i - 1].1 + raw[i].1) * 0.5 * dt;
        cum.push(acc);
    }
    let total = acc;
    if total < 1e-10 { return (0..n).map(|i| i as f32 / (n - 1) as f32).collect(); }

    // Sample n points uniformly in cumulative weight space
    (0..n).map(|i| {
        let target = total * i as f32 / (n - 1).max(1) as f32;
        let idx    = cum.partition_point(|&c| c < target).min(cum.len() - 1);
        if idx == 0 { return raw[0].0; }
        let c0 = cum[idx - 1];
        let c1 = cum[idx];
        let f  = if (c1 - c0).abs() < 1e-10 { 0.0 } else { (target - c0) / (c1 - c0) };
        raw[idx - 1].0 + (raw[idx].0 - raw[idx - 1].0) * f
    }).collect()
}

// ============================================================
// FINAL UNIT TESTS
// ============================================================

#[cfg(test)]
mod tests_final {
    use super::*;

    fn simple_line(n: usize) -> CatmullRomSpline {
        CatmullRomSpline {
            control_points: (0..n).map(|i| ControlPoint {
                position: Vec3::new(i as f32, 0.0, 0.0), weight: 1.0, tension: 0.0
            }).collect(),
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        }
    }

    #[test]
    fn test_color_ramp_midpoint() {
        let mut ramp = SplineColorRamp::new();
        ramp.add_key(0.0, Vec4::ZERO);
        ramp.add_key(1.0, Vec4::ONE);
        let mid = ramp.evaluate(0.5);
        assert!((mid.x - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_point_to_spline_distance() {
        let s = simple_line(3);
        let d = point_to_spline_distance(&s, Vec3::new(1.0, 1.0, 0.0), 64);
        assert!((d - 1.0).abs() < 0.05);
    }

    #[test]
    fn test_subdivide_catmull_doubles_count() {
        let mut s = simple_line(4);
        let n_before = s.control_points.len();
        subdivide_catmull(&mut s, 1);
        assert_eq!(s.control_points.len(), n_before * 2 - 1);
    }

    #[test]
    fn test_spline_metadata_set_get() {
        let mut m = SplineMetadata::new();
        m.set("name", "river");
        m.set("width", "3.5");
        assert_eq!(m.get("name"), Some("river"));
        assert!((m.get_f32("width").unwrap() - 3.5).abs() < 1e-5);
    }

    #[test]
    fn test_reparametrise_count() {
        let s = simple_line(5);
        let ts = reparametrise_by_curvature(&s, 20, 2.0);
        assert_eq!(ts.len(), 20);
        assert!(*ts.first().unwrap() >= 0.0);
        assert!(*ts.last().unwrap() <= 1.0 + 1e-5);
    }

    #[test]
    fn test_camera_path_mat4_finite() {
        let ps = simple_line(3);
        let ts = simple_line(3); // target slightly offset
        let (pos, target, mat) = camera_path_evaluate(&ps, &ts, 0.5);
        assert!(pos.is_finite());
        assert!(target.is_finite());
        for col in mat.to_cols_array() { assert!(col.is_finite()); }
    }

    #[test]
    fn test_spline_spline_min_distance_self_zero() {
        let s = simple_line(4);
        let (d, _ta, _tb) = spline_spline_min_distance(&s, &s, 32);
        assert!(d < 0.01);
    }
}

// ============================================================
// SPLINE WAYPOINT TRACKER
// ============================================================

/// A runtime tracker that advances a "current t" parameter along a spline
/// at a given world-space speed, firing waypoint events.
pub struct SplineWaypointTracker {
    pub t:            f32,
    pub speed:        f32,
    pub arc_length:   f32,
    pub arc_table:    Vec<(f32, f32)>,
    pub waypoints:    Vec<(f32, String)>,  // (arc-s, label)
    pub fired:        Vec<bool>,
    pub loop_mode:    bool,
}

impl SplineWaypointTracker {
    pub fn new(spline: &CatmullRomSpline, speed: f32, loop_mode: bool) -> Self {
        let arc_table   = build_arc_length_table(1024, &|t| spline.evaluate(t));
        let arc_length  = arc_table.last().map(|&(_, s)| s).unwrap_or(0.0);
        SplineWaypointTracker { t: 0.0, speed, arc_length, arc_table, waypoints: Vec::new(), fired: Vec::new(), loop_mode }
    }

    pub fn add_waypoint(&mut self, arc_s: f32, label: &str) {
        self.waypoints.push((arc_s, label.to_string()));
        self.fired.push(false);
    }

    /// Advance by dt seconds; return list of newly-fired waypoint labels.
    pub fn advance(&mut self, dt: f32) -> Vec<String> {
        if self.arc_length < 1e-6 { return Vec::new(); }
        let prev_s = arc_length_to_t(&self.arc_table, 0.0); // placeholder, use arc
        let cur_s  = {
            let t_cur = self.t;
            // Find arc-s corresponding to current t via reverse lookup
            let idx = self.arc_table.partition_point(|&(ti, _)| ti <= t_cur);
            if idx == 0 { 0.0 } else if idx >= self.arc_table.len() {
                self.arc_table.last().unwrap().1
            } else {
                let (t0, s0) = self.arc_table[idx - 1];
                let (t1, s1) = self.arc_table[idx];
                let f = if (t1 - t0).abs() < 1e-10 { 0.0 } else { (t_cur - t0) / (t1 - t0) };
                s0 + (s1 - s0) * f
            }
        };
        let ds   = self.speed * dt;
        let new_s = (cur_s + ds).min(if self.loop_mode { f32::MAX } else { self.arc_length });
        let new_s_wrapped = new_s % self.arc_length;
        self.t    = arc_length_to_t(&self.arc_table, new_s_wrapped);

        let mut fired = Vec::new();
        for (i, &(wp_s, ref label)) in self.waypoints.iter().enumerate() {
            if !self.fired[i] && cur_s < wp_s && new_s >= wp_s {
                self.fired[i] = true;
                fired.push(label.clone());
            }
        }
        let _ = prev_s;
        fired
    }

    pub fn reset(&mut self) { self.t = 0.0; for f in &mut self.fired { *f = false; } }

    pub fn position_on(&self, spline: &CatmullRomSpline) -> Vec3 { spline.evaluate(self.t) }
}

// ============================================================
// SPLINE UTILITY: CATMULL-ROM SECOND DERIVATIVE
// ============================================================

/// Compute the second derivative of a Catmull-Rom segment via finite differences of tangents.
pub fn catmull_second_derivative(spline: &CatmullRomSpline, t: f32, dt: f32) -> Vec3 {
    let t0 = (t - dt).max(0.0);
    let t1 = (t + dt).min(1.0);
    let tang0 = spline.evaluate_derivative(t0);
    let tang1 = spline.evaluate_derivative(t1);
    (tang1 - tang0) / (t1 - t0).max(1e-10)
}

// ============================================================
// SPLINE JERK (THIRD DERIVATIVE)
// ============================================================

/// Approximate jerk (3rd derivative) via finite differences of second derivative.
pub fn catmull_jerk(spline: &CatmullRomSpline, t: f32, dt: f32) -> Vec3 {
    let t0 = (t - dt).max(0.0);
    let t1 = (t + dt).min(1.0);
    let d2_0 = catmull_second_derivative(spline, t0, dt);
    let d2_1 = catmull_second_derivative(spline, t1, dt);
    (d2_1 - d2_0) / (t1 - t0).max(1e-10)
}

// ============================================================
// SPLINE SIGNED CURVATURE (2D XZ)
// ============================================================

/// Signed curvature in the XZ plane: positive = left turn.
pub fn signed_curvature_xz(spline: &CatmullRomSpline, t: f32) -> f32 {
    let d1 = spline.evaluate_derivative(t);
    let d2 = catmull_second_derivative(spline, t, 1e-4);
    let cross = d1.x * d2.z - d1.z * d2.x;
    let denom = (d1.x * d1.x + d1.z * d1.z).powf(1.5);
    if denom < 1e-10 { 0.0 } else { cross / denom }
}

// ============================================================
// SPLINE HEADING ANGLE
// ============================================================

/// Return the heading angle (yaw) in radians of the spline tangent at t.
pub fn spline_heading_yaw(spline: &CatmullRomSpline, t: f32) -> f32 {
    let tang = spline.evaluate_derivative(t);
    tang.z.atan2(tang.x)
}

// ============================================================
// FINAL EXTRA TESTS
// ============================================================

#[cfg(test)]
mod tests_waypoints {
    use super::*;

    fn simple_line(n: usize) -> CatmullRomSpline {
        CatmullRomSpline {
            control_points: (0..n).map(|i| ControlPoint {
                position: Vec3::new(i as f32, 0.0, 0.0), weight: 1.0, tension: 0.0
            }).collect(),
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        }
    }

    #[test]
    fn test_waypoint_tracker_advances() {
        let s = simple_line(5);
        let mut tracker = SplineWaypointTracker::new(&s, 1.0, false);
        tracker.advance(0.5);
        assert!(tracker.t >= 0.0 && tracker.t <= 1.0);
    }

    #[test]
    fn test_signed_curvature_straight_line_zero() {
        let s = simple_line(4);
        let kappa = signed_curvature_xz(&s, 0.5);
        // A straight line has zero curvature
        assert!(kappa.abs() < 0.1);
    }

    #[test]
    fn test_heading_yaw_positive_x() {
        let s = simple_line(3);
        let yaw = spline_heading_yaw(&s, 0.5);
        // Tangent is along +X, so atan2(0, 1) ~ 0
        assert!(yaw.abs() < 0.2);
    }

    #[test]
    fn test_subdivide_idempotent_positions() {
        let mut s = simple_line(3);
        subdivide_catmull(&mut s, 2);
        // All y and z should remain 0
        for cp in &s.control_points {
            assert!(cp.position.y.abs() < 1e-5);
            assert!(cp.position.z.abs() < 1e-5);
        }
    }
}

// ============================================================
// SPLINE CURVATURE COMB VISUALISATION
// ============================================================

/// Compute a curvature comb: for each sample point, a line from the point in
/// the normal direction scaled by curvature * scale.
pub struct CurvatureComb {
    pub base_points:  Vec<Vec3>,
    pub comb_tips:    Vec<Vec3>,
    pub curvatures:   Vec<f32>,
}

impl CurvatureComb {
    pub fn compute(spline: &CatmullRomSpline, samples: usize, scale: f32) -> Self {
        let mut base_points = Vec::with_capacity(samples + 1);
        let mut comb_tips   = Vec::with_capacity(samples + 1);
        let mut curvatures  = Vec::with_capacity(samples + 1);
        for i in 0..=samples {
            let t     = i as f32 / samples as f32;
            let frame = spline.frenet_frame_at(t);
            let tip   = frame.position + frame.normal * (frame.curvature * scale);
            base_points.push(frame.position);
            comb_tips.push(tip);
            curvatures.push(frame.curvature);
        }
        CurvatureComb { base_points, comb_tips, curvatures }
    }

    /// Maximum curvature comb height.
    pub fn max_height(&self) -> f32 {
        self.curvatures.iter().cloned().fold(0.0f32, f32::max)
    }
}

// ============================================================
// SPLINE ARC-LENGTH INTEGRAL HELPERS
// ============================================================

/// Integrate a scalar field f(t) weighted by arc-length ds/dt.
pub fn integrate_along_spline(
    spline:   &CatmullRomSpline,
    field:    &dyn Fn(Vec3) -> f32,
    steps:    usize,
) -> f32 {
    let mut acc = 0.0f32;
    let dt      = 1.0 / steps as f32;
    for i in 0..steps {
        let t0 = i as f32 * dt;
        let t1 = t0 + dt;
        let p0 = spline.evaluate(t0);
        let p1 = spline.evaluate(t1);
        let ds = (p1 - p0).length();
        let f0 = field(p0);
        let f1 = field(p1);
        acc += (f0 + f1) * 0.5 * ds;
    }
    acc
}

/// Average of a scalar field over arc-length.
pub fn average_along_spline(
    spline: &CatmullRomSpline,
    field:  &dyn Fn(Vec3) -> f32,
    steps:  usize,
) -> f32 {
    let integral   = integrate_along_spline(spline, field, steps);
    let arc_length = integrate_along_spline(spline, &|_| 1.0, steps);
    if arc_length < 1e-10 { 0.0 } else { integral / arc_length }
}

// ============================================================
// SPLINE WINDING NUMBER (2-D XZ)
// ============================================================

/// Compute approximate winding number of a closed spline around a query point in XZ.
pub fn winding_number_xz(spline: &CatmullRomSpline, query: Vec2, samples: usize) -> f32 {
    if !spline.closed || samples < 2 { return 0.0; }
    let mut winding = 0.0f32;
    for i in 0..samples {
        let t0 = i as f32 / samples as f32;
        let t1 = (i + 1) as f32 / samples as f32;
        let p0 = spline.evaluate(t0);
        let p1 = spline.evaluate(t1);
        let a  = Vec2::new(p0.x - query.x, p0.z - query.y);
        let b  = Vec2::new(p1.x - query.x, p1.z - query.y);
        // Angle increment using atan2 of cross/dot
        let cross = a.x * b.y - a.y * b.x;
        let dot   = a.x * b.x + a.y * b.y;
        winding += cross.atan2(dot);
    }
    winding / (2.0 * std::f32::consts::PI)
}

#[cfg(test)]
mod tests_comb {
    use super::*;
    fn simple_line(n: usize) -> CatmullRomSpline {
        CatmullRomSpline {
            control_points: (0..n).map(|i| ControlPoint {
                position: Vec3::new(i as f32, 0.0, 0.0), weight: 1.0, tension: 0.0
            }).collect(),
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        }
    }

    #[test]
    fn test_curvature_comb_sample_count() {
        let s    = simple_line(4);
        let comb = CurvatureComb::compute(&s, 32, 1.0);
        assert_eq!(comb.base_points.len(), 33);
    }

    #[test]
    fn test_integrate_along_constant_one() {
        let s   = simple_line(3);
        let val = integrate_along_spline(&s, &|_| 1.0, 128);
        // Arc length of a line 0..2 is approximately 2
        assert!((val - 2.0).abs() < 0.1);
    }
}

// ============================================================
// SPLINE UTILITY: PERPENDICULAR DISTANCE FROM LINE
// ============================================================

/// Signed perpendicular distance from `point` to the line through `a` and `b` in XZ.
pub fn xz_line_perpendicular_distance(a: Vec3, b: Vec3, point: Vec3) -> f32 {
    let ab = b - a;
    let ap = point - a;
    let ab_len = ab.length();
    if ab_len < 1e-10 { return (point - a).length(); }
    let ab_hat = ab / ab_len;
    let perp = ap - ab_hat * ap.dot(ab_hat);
    // signed via cross in XZ
    let sign = (ab_hat.x * perp.z - ab_hat.z * perp.x).signum();
    perp.length() * sign
}

/// Compute the deviation of each control point from the chord (first to last).
pub fn chord_deviation(spline: &CatmullRomSpline) -> Vec<f32> {
    let n = spline.control_points.len();
    if n < 2 { return vec![0.0; n]; }
    let a = spline.control_points[0].position;
    let b = spline.control_points[n - 1].position;
    spline.control_points.iter().map(|cp| xz_line_perpendicular_distance(a, b, cp.position)).collect()
}

/// Maximum absolute chord deviation.
pub fn max_chord_deviation(spline: &CatmullRomSpline) -> f32 {
    chord_deviation(spline).into_iter().map(|d| d.abs()).fold(0.0f32, f32::max)
}

#[cfg(test)]
mod tests_spline_geometry {
    use super::*;
    fn simple_line(n: usize) -> CatmullRomSpline {
        CatmullRomSpline {
            control_points: (0..n).map(|i| ControlPoint {
                position: Vec3::new(i as f32, 0.0, 0.0), weight: 1.0, tension: 0.0
            }).collect(),
            closed: false, alpha: 0.5, arc_length_table: Vec::new(), total_length: 0.0,
        }
    }

    #[test]
    fn test_xz_perpendicular_distance_on_line() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(4.0, 0.0, 0.0);
        let p = Vec3::new(2.0, 0.0, 3.0);
        let d = xz_line_perpendicular_distance(a, b, p);
        assert!((d.abs() - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_chord_deviation_straight_line_zero() {
        let s = simple_line(5);
        let max = max_chord_deviation(&s);
        assert!(max < 1e-4);
    }
}

// ============================================================
// SPLINE UTILITIES: CENTRIPETAL PARAMETER ESTIMATE
// ============================================================

/// Estimate a good alpha for centripetal Catmull-Rom given control points.
/// Returns the mean chord-length ratio exponent that minimises parameterisation error.
pub fn estimate_alpha(pts: &[Vec3]) -> f32 {
    if pts.len() < 3 { return 0.5; }
    let mut sum_ratio = 0.0f32;
    let n = pts.len() - 2;
    for i in 0..n {
        let d0 = (pts[i+1] - pts[i]).length().max(1e-10);
        let d1 = (pts[i+2] - pts[i+1]).length().max(1e-10);
        sum_ratio += (d0 / d1).ln().abs();
    }
    let mean_ratio = sum_ratio / n as f32;
    // alpha = 0 (uniform), 0.5 (centripetal), 1.0 (chordal)
    (0.5 * (1.0 + mean_ratio * 0.5)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests_alpha_estimate {
    use super::*;

    #[test]
    fn test_estimate_alpha_uniform_spacing_half() {
        let pts: Vec<Vec3> = (0..5).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect();
        let a = estimate_alpha(&pts);
        // Uniformly spaced → ratio = 1 → ln(1)=0 → alpha = 0.5
        assert!((a - 0.5).abs() < 0.01);
    }
}
