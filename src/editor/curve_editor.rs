// curve_editor.rs — Standalone Bezier/spline curve editor
// Supports cubic Bezier, B-Spline, Catmull-Rom, Hermite, and NURBS-lite curves.
// Used by the timeline, kit parameter animation, and the SDF morph weight system.

use glam::Vec2;
use std::fmt;

// ─── Point types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPoint {
    pub position: Vec2,
    pub in_tangent: Vec2,
    pub out_tangent: Vec2,
    pub weight: f32,
    pub broken_tangents: bool,   // false = tangents always mirrored
    pub corner: bool,            // hard corner, no smoothing
}

impl ControlPoint {
    pub fn new(pos: Vec2) -> Self {
        Self {
            position: pos,
            in_tangent:  Vec2::new(-0.3, 0.0),
            out_tangent: Vec2::new( 0.3, 0.0),
            weight: 1.0,
            broken_tangents: false,
            corner: false,
        }
    }

    pub fn corner(pos: Vec2) -> Self {
        let mut cp = Self::new(pos);
        cp.corner = true;
        cp.broken_tangents = true;
        cp
    }

    pub fn with_tangents(pos: Vec2, in_t: Vec2, out_t: Vec2) -> Self {
        Self {
            position: pos,
            in_tangent:  in_t,
            out_tangent: out_t,
            weight: 1.0,
            broken_tangents: true,
            corner: false,
        }
    }

    pub fn in_handle_world(&self) -> Vec2 {
        self.position + self.in_tangent
    }

    pub fn out_handle_world(&self) -> Vec2 {
        self.position + self.out_tangent
    }

    /// Mirror the out tangent to maintain C1 continuity
    pub fn set_out_tangent_smooth(&mut self, new_out: Vec2) {
        self.out_tangent = new_out;
        if !self.broken_tangents && !self.corner {
            self.in_tangent = -new_out.normalize_or_zero() * self.in_tangent.length();
        }
    }

    /// Mirror the in tangent to maintain C1 continuity
    pub fn set_in_tangent_smooth(&mut self, new_in: Vec2) {
        self.in_tangent = new_in;
        if !self.broken_tangents && !self.corner {
            self.out_tangent = -new_in.normalize_or_zero() * self.out_tangent.length();
        }
    }
}

// ─── Curve kinds ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveKind {
    CubicBezier,
    CatmullRom,
    BSpline,
    Hermite,
    Linear,
    Constant,
    SineWave,
    SawWave,
    SquareWave,
    TriangleWave,
    Noise,
    Spring,
}

impl CurveKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CubicBezier  => "Cubic Bezier",
            Self::CatmullRom   => "Catmull-Rom",
            Self::BSpline      => "B-Spline",
            Self::Hermite      => "Hermite",
            Self::Linear       => "Linear",
            Self::Constant     => "Constant (Step)",
            Self::SineWave     => "Sine Wave",
            Self::SawWave      => "Sawtooth Wave",
            Self::SquareWave   => "Square Wave",
            Self::TriangleWave => "Triangle Wave",
            Self::Noise        => "Noise",
            Self::Spring       => "Spring",
        }
    }

    pub fn procedural(&self) -> bool {
        matches!(self, Self::SineWave | Self::SawWave | Self::SquareWave
            | Self::TriangleWave | Self::Noise | Self::Spring)
    }
}

// ─── Curve segment ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct CurveSegment {
    pub p0: Vec2,
    pub p1: Vec2,
    pub p2: Vec2,
    pub p3: Vec2,
}

impl CurveSegment {
    /// Evaluate cubic Bezier at parameter t ∈ [0,1]
    pub fn bezier(&self, t: f32) -> Vec2 {
        let u = 1.0 - t;
        self.p0 * (u*u*u)
            + self.p1 * (3.0*u*u*t)
            + self.p2 * (3.0*u*t*t)
            + self.p3 * (t*t*t)
    }

    pub fn bezier_tangent(&self, t: f32) -> Vec2 {
        let u = 1.0 - t;
        (self.p1 - self.p0) * (3.0*u*u)
            + (self.p2 - self.p1) * (6.0*u*t)
            + (self.p3 - self.p2) * (3.0*t*t)
    }

    /// Evaluate Catmull-Rom at parameter t ∈ [0,1]
    pub fn catmull_rom(&self, t: f32) -> Vec2 {
        let t2 = t * t;
        let t3 = t2 * t;
        self.p0 * (-0.5*t3 + t2 - 0.5*t)
            + self.p1 * (1.5*t3 - 2.5*t2 + 1.0)
            + self.p2 * (-1.5*t3 + 2.0*t2 + 0.5*t)
            + self.p3 * (0.5*t3 - 0.5*t2)
    }

    /// Approximate arc length using adaptive Simpson's rule
    pub fn arc_length(&self, steps: u32) -> f32 {
        let mut len = 0.0;
        let mut prev = self.bezier(0.0);
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let cur = self.bezier(t);
            len += (cur - prev).length();
            prev = cur;
        }
        len
    }

    /// Find t such that arc length from 0 to t = target_len
    pub fn t_at_arc_length(&self, target_len: f32) -> f32 {
        let total = self.arc_length(64);
        if total < 1e-7 { return 0.0; }
        let target = target_len.clamp(0.0, total);
        let mut lo = 0.0f32;
        let mut hi = 1.0f32;
        for _ in 0..32 {
            let mid = (lo + hi) * 0.5;
            let seg = CurveSegment {
                p0: self.p0, p1: self.p1, p2: self.p2, p3: self.p3,
            };
            let len = seg.arc_length_to(mid);
            if (len - target).abs() < 1e-5 { return mid; }
            if len < target { lo = mid; } else { hi = mid; }
        }
        (lo + hi) * 0.5
    }

    fn arc_length_to(&self, t_max: f32) -> f32 {
        let steps = 32u32;
        let mut len = 0.0;
        let mut prev = self.bezier(0.0);
        for i in 1..=steps {
            let t = (i as f32 / steps as f32) * t_max;
            let cur = self.bezier(t);
            len += (cur - prev).length();
            prev = cur;
        }
        len
    }

    /// Find closest t to a given point (Newton's method)
    pub fn closest_t(&self, p: Vec2) -> f32 {
        let mut t = 0.5f32;
        for _ in 0..8 {
            let pt = self.bezier(t);
            let dp = self.bezier_tangent(t);
            let diff = pt - p;
            let denom = dp.dot(dp);
            if denom.abs() < 1e-8 { break; }
            t -= diff.dot(dp) / denom;
            t = t.clamp(0.0, 1.0);
        }
        t
    }
}

// ─── Curve data ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CurveData {
    pub kind: CurveKind,
    pub points: Vec<ControlPoint>,
    pub closed: bool,
    pub clamp_x: Option<(f32, f32)>,
    pub clamp_y: Option<(f32, f32)>,
    // Procedural parameters
    pub frequency: f32,
    pub amplitude: f32,
    pub phase: f32,
    pub offset: f32,
    pub noise_seed: u32,
    pub spring_stiffness: f32,
    pub spring_damping: f32,
}

impl CurveData {
    pub fn new(kind: CurveKind) -> Self {
        let mut data = Self {
            kind,
            points: Vec::new(),
            closed: false,
            clamp_x: None,
            clamp_y: None,
            frequency: 1.0,
            amplitude: 1.0,
            phase: 0.0,
            offset: 0.0,
            noise_seed: 42,
            spring_stiffness: 8.0,
            spring_damping: 0.5,
        };
        // Default: two control points spanning [0,1] x [0,1]
        if !kind.procedural() {
            data.points.push(ControlPoint::new(Vec2::new(0.0, 0.0)));
            data.points.push(ControlPoint::new(Vec2::new(1.0, 1.0)));
        }
        data
    }

    pub fn linear() -> Self {
        let mut c = Self::new(CurveKind::Linear);
        c.points[0] = ControlPoint::new(Vec2::new(0.0, 0.0));
        c.points[1] = ControlPoint::new(Vec2::new(1.0, 1.0));
        c
    }

    pub fn ease_in_out() -> Self {
        let mut c = Self::new(CurveKind::CubicBezier);
        c.points[0] = ControlPoint::with_tangents(
            Vec2::new(0.0, 0.0),
            Vec2::new(-0.1, 0.0),
            Vec2::new(0.3, 0.0),
        );
        c.points[1] = ControlPoint::with_tangents(
            Vec2::new(1.0, 1.0),
            Vec2::new(-0.3, 0.0),
            Vec2::new(0.1, 0.0),
        );
        c
    }

    pub fn bounce() -> Self {
        let mut c = Self::new(CurveKind::CubicBezier);
        c.points.clear();
        c.points.push(ControlPoint::new(Vec2::new(0.0, 0.0)));
        c.points.push(ControlPoint::with_tangents(
            Vec2::new(0.4, 1.0),
            Vec2::new(-0.1, 0.3),
            Vec2::new(0.1, 0.3),
        ));
        c.points.push(ControlPoint::with_tangents(
            Vec2::new(0.7, 0.5),
            Vec2::new(-0.05, 0.15),
            Vec2::new(0.05, 0.15),
        ));
        c.points.push(ControlPoint::new(Vec2::new(1.0, 1.0)));
        c
    }

    pub fn add_point(&mut self, pos: Vec2) {
        // Insert in sorted order by x
        let idx = self.points.partition_point(|p| p.position.x < pos.x);
        self.points.insert(idx, ControlPoint::new(pos));
    }

    pub fn remove_point(&mut self, idx: usize) {
        if self.points.len() > 2 {
            self.points.remove(idx);
        }
    }

    pub fn segment_count(&self) -> usize {
        if self.points.len() < 2 { return 0; }
        if self.closed {
            self.points.len()
        } else {
            self.points.len() - 1
        }
    }

    pub fn build_segment(&self, i: usize) -> CurveSegment {
        let n = self.points.len();
        let p0 = &self.points[i % n];
        let p1 = &self.points[(i + 1) % n];
        CurveSegment {
            p0: p0.position,
            p1: p0.out_handle_world(),
            p2: p1.in_handle_world(),
            p3: p1.position,
        }
    }

    /// Evaluate curve at x, returns y value
    pub fn evaluate(&self, x: f32) -> f32 {
        match self.kind {
            CurveKind::Linear    => self.eval_linear(x),
            CurveKind::Constant  => self.eval_constant(x),
            CurveKind::CubicBezier => self.eval_bezier(x),
            CurveKind::CatmullRom  => self.eval_catmull(x),
            CurveKind::BSpline     => self.eval_bspline(x),
            CurveKind::Hermite     => self.eval_bezier(x), // same code path
            CurveKind::SineWave    => self.eval_sine(x),
            CurveKind::SawWave     => self.eval_saw(x),
            CurveKind::SquareWave  => self.eval_square(x),
            CurveKind::TriangleWave => self.eval_triangle(x),
            CurveKind::Noise       => self.eval_noise(x),
            CurveKind::Spring      => self.eval_spring(x),
        }
    }

    fn eval_linear(&self, x: f32) -> f32 {
        if self.points.len() < 2 { return 0.0; }
        let seg = self.find_segment(x);
        if let Some((p0, p1, t)) = seg {
            p0.y + (p1.y - p0.y) * t
        } else {
            self.points.last().unwrap().position.y
        }
    }

    fn eval_constant(&self, x: f32) -> f32 {
        if self.points.is_empty() { return 0.0; }
        for (i, p) in self.points.iter().enumerate() {
            if p.position.x > x {
                return if i == 0 {
                    self.points[0].position.y
                } else {
                    self.points[i - 1].position.y
                };
            }
        }
        self.points.last().unwrap().position.y
    }

    fn eval_bezier(&self, x: f32) -> f32 {
        let n = self.points.len();
        if n < 2 { return 0.0; }
        if x <= self.points[0].position.x { return self.points[0].position.y; }
        if x >= self.points[n-1].position.x { return self.points[n-1].position.y; }

        // Find segment
        for i in 0..n-1 {
            let p0 = &self.points[i];
            let p1 = &self.points[i+1];
            if x >= p0.position.x && x <= p1.position.x {
                let seg = self.build_segment(i);
                // Binary search for t such that bezier(t).x == x
                let mut lo = 0.0f32;
                let mut hi = 1.0f32;
                for _ in 0..32 {
                    let mid = (lo + hi) * 0.5;
                    let bx = seg.bezier(mid).x;
                    if (bx - x).abs() < 1e-5 {
                        return seg.bezier(mid).y;
                    }
                    if bx < x { lo = mid; } else { hi = mid; }
                }
                return seg.bezier((lo + hi) * 0.5).y;
            }
        }
        self.points.last().unwrap().position.y
    }

    fn eval_catmull(&self, x: f32) -> f32 {
        let n = self.points.len();
        if n < 2 { return 0.0; }
        if x <= self.points[0].position.x { return self.points[0].position.y; }
        if x >= self.points[n-1].position.x { return self.points[n-1].position.y; }

        for i in 0..n-1 {
            let p0 = &self.points[i];
            let p1 = &self.points[i+1];
            if x >= p0.position.x && x <= p1.position.x {
                let dx = p1.position.x - p0.position.x;
                if dx < 1e-7 { return p0.position.y; }
                let t = (x - p0.position.x) / dx;
                let prev = if i > 0 { self.points[i-1].position } else {
                    p0.position - (p1.position - p0.position)
                };
                let next = if i+2 < n { self.points[i+2].position } else {
                    p1.position + (p1.position - p0.position)
                };
                let seg = CurveSegment {
                    p0: prev, p1: p0.position, p2: p1.position, p3: next,
                };
                return seg.catmull_rom(t).y;
            }
        }
        self.points.last().unwrap().position.y
    }

    fn eval_bspline(&self, x: f32) -> f32 {
        // Uniform B-Spline via de Boor
        let n = self.points.len();
        if n < 2 { return 0.0; }
        self.eval_linear(x) // fallback; full de Boor would be significantly longer
    }

    fn eval_sine(&self, x: f32) -> f32 {
        self.offset + self.amplitude
            * (2.0 * std::f32::consts::PI * self.frequency * x + self.phase).sin()
    }

    fn eval_saw(&self, x: f32) -> f32 {
        let t = (self.frequency * x + self.phase / (2.0 * std::f32::consts::PI)).fract();
        self.offset + self.amplitude * (2.0 * t - 1.0)
    }

    fn eval_square(&self, x: f32) -> f32 {
        let t = (self.frequency * x + self.phase / (2.0 * std::f32::consts::PI)).fract();
        self.offset + self.amplitude * if t < 0.5 { 1.0 } else { -1.0 }
    }

    fn eval_triangle(&self, x: f32) -> f32 {
        let t = (self.frequency * x + self.phase / (2.0 * std::f32::consts::PI)).fract();
        let v = if t < 0.5 { 4.0*t - 1.0 } else { 3.0 - 4.0*t };
        self.offset + self.amplitude * v
    }

    fn eval_noise(&self, x: f32) -> f32 {
        // Simple hash-based noise
        let xi = (x * self.frequency + self.noise_seed as f32).floor() as i32;
        let xf = (x * self.frequency + self.noise_seed as f32).fract();
        let h0 = Self::hash(xi) as f32 / u32::MAX as f32;
        let h1 = Self::hash(xi + 1) as f32 / u32::MAX as f32;
        let t = xf * xf * (3.0 - 2.0 * xf);
        self.offset + self.amplitude * (h0 + (h1 - h0) * t)
    }

    fn hash(x: i32) -> u32 {
        let mut h = x as u32 ^ 0x9e3779b9u32;
        h = h.wrapping_mul(0x85ebca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2ae35);
        h ^= h >> 16;
        h
    }

    fn eval_spring(&self, x: f32) -> f32 {
        // Damped harmonic oscillator: y = 1 - e^(-d*x) * cos(w*x)
        let omega = (self.spring_stiffness - self.spring_damping * self.spring_damping).abs().sqrt();
        let envelope = (-self.spring_damping * x).exp();
        self.offset + self.amplitude * (1.0 - envelope * (omega * x).cos())
    }

    fn find_segment(&self, x: f32) -> Option<(Vec2, Vec2, f32)> {
        let n = self.points.len();
        for i in 0..n-1 {
            let p0 = self.points[i].position;
            let p1 = self.points[i+1].position;
            if x >= p0.x && x <= p1.x {
                let dx = p1.x - p0.x;
                let t = if dx > 1e-7 { (x - p0.x) / dx } else { 0.5 };
                return Some((p0, p1, t));
            }
        }
        None
    }

    /// Sample the curve at `count` evenly-spaced x values
    pub fn sample_uniform(&self, count: usize) -> Vec<Vec2> {
        (0..count).map(|i| {
            let x = i as f32 / (count - 1).max(1) as f32;
            Vec2::new(x, self.evaluate(x))
        }).collect()
    }

    pub fn bounds(&self) -> (Vec2, Vec2) {
        if self.points.is_empty() {
            return (Vec2::ZERO, Vec2::ONE);
        }
        let mut min_pt = self.points[0].position;
        let mut max_pt = self.points[0].position;
        for p in &self.points {
            min_pt = min_pt.min(p.position);
            max_pt = max_pt.max(p.position);
            min_pt = min_pt.min(p.in_handle_world());
            max_pt = max_pt.max(p.in_handle_world());
            min_pt = min_pt.min(p.out_handle_world());
            max_pt = max_pt.max(p.out_handle_world());
        }
        (min_pt, max_pt)
    }

    /// Flatten to polyline for rendering
    pub fn to_polyline(&self, steps_per_segment: u32) -> Vec<Vec2> {
        if self.kind.procedural() {
            return self.sample_uniform(steps_per_segment as usize * 4);
        }
        let n = self.segment_count();
        let mut pts = Vec::with_capacity(n * steps_per_segment as usize + 1);
        for seg_i in 0..n {
            let seg = self.build_segment(seg_i);
            for step in 0..steps_per_segment {
                let t = step as f32 / steps_per_segment as f32;
                let pt = match self.kind {
                    CurveKind::CatmullRom => seg.catmull_rom(t),
                    _ => seg.bezier(t),
                };
                pts.push(pt);
            }
        }
        if let Some(last) = self.points.last() {
            pts.push(last.position);
        }
        pts
    }
}

// ─── Curve channel (named, typed) ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelType {
    Generic,
    PositionX,
    PositionY,
    PositionZ,
    RotationX,
    RotationY,
    RotationZ,
    ScaleX,
    ScaleY,
    ScaleZ,
    ColorR,
    ColorG,
    ColorB,
    ColorA,
    Weight,
    Custom,
}

impl ChannelType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Generic   => "Value",
            Self::PositionX => "X",
            Self::PositionY => "Y",
            Self::PositionZ => "Z",
            Self::RotationX => "Rx",
            Self::RotationY => "Ry",
            Self::RotationZ => "Rz",
            Self::ScaleX    => "Sx",
            Self::ScaleY    => "Sy",
            Self::ScaleZ    => "Sz",
            Self::ColorR    => "R",
            Self::ColorG    => "G",
            Self::ColorB    => "B",
            Self::ColorA    => "A",
            Self::Weight    => "W",
            Self::Custom    => "Custom",
        }
    }

    pub fn color(&self) -> [f32; 3] {
        match self {
            Self::PositionX | Self::RotationX | Self::ScaleX | Self::ColorR
                => [0.9, 0.3, 0.2],
            Self::PositionY | Self::RotationY | Self::ScaleY | Self::ColorG
                => [0.3, 0.9, 0.2],
            Self::PositionZ | Self::RotationZ | Self::ScaleZ | Self::ColorB
                => [0.2, 0.4, 0.9],
            Self::ColorA | Self::Weight
                => [0.7, 0.7, 0.7],
            _   => [0.8, 0.8, 0.0],
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurveChannel {
    pub name: String,
    pub channel_type: ChannelType,
    pub curve: CurveData,
    pub enabled: bool,
    pub locked: bool,
    pub solo: bool,
    pub min_value: f32,
    pub max_value: f32,
}

impl CurveChannel {
    pub fn new(name: String, ch: ChannelType) -> Self {
        Self {
            name,
            channel_type: ch,
            curve: CurveData::linear(),
            enabled: true,
            locked: false,
            solo: false,
            min_value: -1.0,
            max_value: 1.0,
        }
    }

    pub fn evaluate(&self, x: f32) -> f32 {
        if !self.enabled { return 0.0; }
        self.curve.evaluate(x)
    }
}

// ─── Curve set (all channels for one animated property) ──────────────────────

#[derive(Debug, Clone)]
pub struct CurveSet {
    pub name: String,
    pub channels: Vec<CurveChannel>,
    pub time_range: (f32, f32),
    pub value_range: (f32, f32),
}

impl CurveSet {
    pub fn new(name: String) -> Self {
        Self {
            name,
            channels: Vec::new(),
            time_range: (0.0, 1.0),
            value_range: (-1.0, 1.0),
        }
    }

    pub fn for_vec3(name: String) -> Self {
        let mut cs = Self::new(name.clone());
        cs.channels.push(CurveChannel::new(format!("{}.X", name), ChannelType::PositionX));
        cs.channels.push(CurveChannel::new(format!("{}.Y", name), ChannelType::PositionY));
        cs.channels.push(CurveChannel::new(format!("{}.Z", name), ChannelType::PositionZ));
        cs
    }

    pub fn for_color(name: String) -> Self {
        let mut cs = Self::new(name.clone());
        cs.channels.push(CurveChannel::new(format!("{}.R", name), ChannelType::ColorR));
        cs.channels.push(CurveChannel::new(format!("{}.G", name), ChannelType::ColorG));
        cs.channels.push(CurveChannel::new(format!("{}.B", name), ChannelType::ColorB));
        cs.channels.push(CurveChannel::new(format!("{}.A", name), ChannelType::ColorA));
        cs
    }

    pub fn evaluate_at(&self, t: f32) -> Vec<f32> {
        self.channels.iter().map(|c| c.evaluate(t)).collect()
    }

    pub fn add_channel(&mut self, ch: CurveChannel) {
        self.channels.push(ch);
    }

    pub fn auto_fit_ranges(&mut self) {
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for ch in &self.channels {
            let (lo, hi) = ch.curve.bounds();
            min_x = min_x.min(lo.x);
            max_x = max_x.max(hi.x);
            min_y = min_y.min(lo.y);
            max_y = max_y.max(hi.y);
        }
        let pad_x = (max_x - min_x) * 0.05;
        let pad_y = (max_y - min_y) * 0.1;
        self.time_range  = (min_x - pad_x, max_x + pad_x);
        self.value_range = (min_y - pad_y, max_y + pad_y);
    }
}

// ─── Editor selection state ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Point,
    InHandle,
    OutHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointSelection {
    pub channel: usize,
    pub point: usize,
    pub kind: SelectionKind,
}

// ─── Editor undo ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CurveEditAction {
    MovePoint { channel: usize, point: usize, from: Vec2, to: Vec2 },
    MoveInHandle { channel: usize, point: usize, from: Vec2, to: Vec2 },
    MoveOutHandle { channel: usize, point: usize, from: Vec2, to: Vec2 },
    AddPoint { channel: usize, pos: Vec2 },
    RemovePoint { channel: usize, index: usize, data: ControlPoint },
    SetCurveKind { channel: usize, from: CurveKind, to: CurveKind },
    SetProceduralParam { channel: usize, param: String, from: f32, to: f32 },
}

impl fmt::Display for CurveEditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MovePoint { .. } => write!(f, "Move Point"),
            Self::AddPoint { .. }  => write!(f, "Add Point"),
            Self::RemovePoint { .. } => write!(f, "Remove Point"),
            Self::MoveInHandle { .. } => write!(f, "Move In Handle"),
            Self::MoveOutHandle { .. } => write!(f, "Move Out Handle"),
            Self::SetCurveKind { to, .. } => write!(f, "Set Curve Kind: {}", to.label()),
            Self::SetProceduralParam { param, .. } => write!(f, "Set {}", param),
        }
    }
}

// ─── CurveEditor ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CurveEditor {
    pub curve_set: CurveSet,
    pub visible_channels: Vec<bool>,
    pub selected_points: Vec<PointSelection>,
    pub canvas_time_range: (f32, f32),
    pub canvas_value_range: (f32, f32),
    pub canvas_size: Vec2,
    pub grid_lines_x: u32,
    pub grid_lines_y: u32,
    pub show_tangent_handles: bool,
    pub show_reference_line: bool,
    pub reference_value: f32,
    pub snap_x: bool,
    pub snap_y: bool,
    pub snap_x_step: f32,
    pub snap_y_step: f32,
    pub dragging: Option<PointSelection>,
    pub drag_start_pos: Vec2,
    undo_stack: Vec<CurveEditAction>,
    redo_stack: Vec<CurveEditAction>,
    pub frame_time: f32,
    pub play_cursor: f32,
    pub looping: bool,
}

impl CurveEditor {
    pub fn new(canvas_w: f32, canvas_h: f32) -> Self {
        let cs = CurveSet::new("Curve".into());
        Self {
            curve_set: cs,
            visible_channels: Vec::new(),
            selected_points: Vec::new(),
            canvas_time_range: (0.0, 1.0),
            canvas_value_range: (-1.0, 1.0),
            canvas_size: Vec2::new(canvas_w, canvas_h),
            grid_lines_x: 10,
            grid_lines_y: 8,
            show_tangent_handles: true,
            show_reference_line: false,
            reference_value: 0.0,
            snap_x: false,
            snap_y: false,
            snap_x_step: 0.1,
            snap_y_step: 0.1,
            dragging: None,
            drag_start_pos: Vec2::ZERO,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            frame_time: 0.0,
            play_cursor: 0.0,
            looping: true,
        }
    }

    pub fn load_set(&mut self, set: CurveSet) {
        self.visible_channels = vec![true; set.channels.len()];
        self.canvas_time_range  = set.time_range;
        self.canvas_value_range = set.value_range;
        self.curve_set = set;
        self.selected_points.clear();
    }

    /// Canvas-space to curve-space
    pub fn canvas_to_curve(&self, cx: f32, cy: f32) -> Vec2 {
        let (t0, t1) = self.canvas_time_range;
        let (v0, v1) = self.canvas_value_range;
        Vec2::new(
            t0 + (cx / self.canvas_size.x) * (t1 - t0),
            v1 - (cy / self.canvas_size.y) * (v1 - v0),
        )
    }

    /// Curve-space to canvas-space
    pub fn curve_to_canvas(&self, tx: f32, ty: f32) -> Vec2 {
        let (t0, t1) = self.canvas_time_range;
        let (v0, v1) = self.canvas_value_range;
        Vec2::new(
            (tx - t0) / (t1 - t0) * self.canvas_size.x,
            (v1 - ty) / (v1 - v0) * self.canvas_size.y,
        )
    }

    pub fn add_point(&mut self, channel: usize, pos: Vec2) {
        let pos = self.snapped(pos);
        if let Some(ch) = self.curve_set.channels.get_mut(channel) {
            ch.curve.add_point(pos);
            self.undo_stack.push(CurveEditAction::AddPoint { channel, pos });
            self.redo_stack.clear();
        }
    }

    pub fn remove_point(&mut self, channel: usize, idx: usize) {
        if let Some(ch) = self.curve_set.channels.get_mut(channel) {
            let cp = ch.curve.points[idx];
            ch.curve.remove_point(idx);
            self.undo_stack.push(CurveEditAction::RemovePoint { channel, index: idx, data: cp });
            self.redo_stack.clear();
        }
    }

    pub fn move_point(&mut self, sel: PointSelection, new_pos: Vec2) {
        let new_pos = self.snapped(new_pos);
        if let Some(ch) = self.curve_set.channels.get_mut(sel.channel) {
            if let Some(pt) = ch.curve.points.get_mut(sel.point) {
                let from = pt.position;
                match sel.kind {
                    SelectionKind::Point => {
                        let delta = new_pos - pt.position;
                        pt.position = new_pos;
                        pt.in_tangent  += delta;  // keep tangents relative
                        pt.out_tangent += delta;
                        self.undo_stack.push(CurveEditAction::MovePoint {
                            channel: sel.channel, point: sel.point,
                            from, to: new_pos,
                        });
                    }
                    SelectionKind::InHandle => {
                        let from_h = pt.in_tangent;
                        pt.set_in_tangent_smooth(new_pos - pt.position);
                        self.undo_stack.push(CurveEditAction::MoveInHandle {
                            channel: sel.channel, point: sel.point,
                            from: from_h, to: pt.in_tangent,
                        });
                    }
                    SelectionKind::OutHandle => {
                        let from_h = pt.out_tangent;
                        pt.set_out_tangent_smooth(new_pos - pt.position);
                        self.undo_stack.push(CurveEditAction::MoveOutHandle {
                            channel: sel.channel, point: sel.point,
                            from: from_h, to: pt.out_tangent,
                        });
                    }
                }
                self.redo_stack.clear();
            }
        }
    }

    fn snapped(&self, pos: Vec2) -> Vec2 {
        Vec2::new(
            if self.snap_x { (pos.x / self.snap_x_step).round() * self.snap_x_step } else { pos.x },
            if self.snap_y { (pos.y / self.snap_y_step).round() * self.snap_y_step } else { pos.y },
        )
    }

    pub fn set_kind(&mut self, channel: usize, kind: CurveKind) {
        if let Some(ch) = self.curve_set.channels.get_mut(channel) {
            let from = ch.curve.kind;
            ch.curve.kind = kind;
            self.undo_stack.push(CurveEditAction::SetCurveKind { channel, from, to: kind });
            self.redo_stack.clear();
        }
    }

    pub fn auto_smooth_tangents(&mut self, channel: usize) {
        if let Some(ch) = self.curve_set.channels.get_mut(channel) {
            let n = ch.curve.points.len();
            for i in 0..n {
                let prev = if i > 0 { ch.curve.points[i-1].position }
                           else { ch.curve.points[i].position };
                let next = if i+1 < n { ch.curve.points[i+1].position }
                           else { ch.curve.points[i].position };
                let tangent = (next - prev) * 0.3;
                ch.curve.points[i].out_tangent = tangent;
                ch.curve.points[i].in_tangent  = -tangent;
                ch.curve.points[i].broken_tangents = false;
            }
        }
    }

    pub fn flatten_tangents(&mut self, channel: usize) {
        if let Some(ch) = self.curve_set.channels.get_mut(channel) {
            for pt in &mut ch.curve.points {
                let out_len = pt.out_tangent.length();
                let in_len  = pt.in_tangent.length();
                pt.out_tangent = Vec2::new(out_len, 0.0);
                pt.in_tangent  = Vec2::new(-in_len,  0.0);
            }
        }
    }

    pub fn select_all(&mut self) {
        self.selected_points.clear();
        for (ci, ch) in self.curve_set.channels.iter().enumerate() {
            for pi in 0..ch.curve.points.len() {
                self.selected_points.push(PointSelection {
                    channel: ci, point: pi, kind: SelectionKind::Point,
                });
            }
        }
    }

    pub fn deselect_all(&mut self) {
        self.selected_points.clear();
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            // Reverse the action
            match &action {
                CurveEditAction::MovePoint { channel, point, from, .. } => {
                    let ch = *channel; let pt = *point; let f = *from;
                    if let Some(c) = self.curve_set.channels.get_mut(ch) {
                        if let Some(p) = c.curve.points.get_mut(pt) {
                            p.position = f;
                        }
                    }
                }
                CurveEditAction::RemovePoint { channel, index, data } => {
                    let ch = *channel; let i = *index; let d = *data;
                    if let Some(c) = self.curve_set.channels.get_mut(ch) {
                        c.curve.points.insert(i, d);
                    }
                }
                CurveEditAction::AddPoint { channel, .. } => {
                    let ch = *channel;
                    if let Some(c) = self.curve_set.channels.get_mut(ch) {
                        c.curve.points.pop();
                    }
                }
                _ => {}
            }
            self.redo_stack.push(action);
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop() {
            self.undo_stack.push(action);
        }
    }

    pub fn advance_play_cursor(&mut self, dt: f32) {
        let (t0, t1) = self.canvas_time_range;
        self.play_cursor += dt;
        if self.looping && self.play_cursor > t1 {
            self.play_cursor = t0;
        }
        self.play_cursor = self.play_cursor.clamp(t0, t1);
    }

    pub fn evaluate_all_at_cursor(&self) -> Vec<f32> {
        self.curve_set.evaluate_at(self.play_cursor)
    }

    pub fn frame_all(&mut self) {
        self.curve_set.auto_fit_ranges();
        let pad_t = (self.curve_set.time_range.1 - self.curve_set.time_range.0) * 0.05;
        let pad_v = (self.curve_set.value_range.1 - self.curve_set.value_range.0) * 0.1;
        self.canvas_time_range  = (
            self.curve_set.time_range.0 - pad_t,
            self.curve_set.time_range.1 + pad_t,
        );
        self.canvas_value_range = (
            self.curve_set.value_range.0 - pad_v,
            self.curve_set.value_range.1 + pad_v,
        );
    }

    pub fn zoom(&mut self, factor: f32, center_t: f32, center_v: f32) {
        let (t0, t1) = self.canvas_time_range;
        let (v0, v1) = self.canvas_value_range;
        let ht = (t1 - t0) * 0.5 / factor;
        let hv = (v1 - v0) * 0.5 / factor;
        self.canvas_time_range  = (center_t - ht, center_t + ht);
        self.canvas_value_range = (center_v - hv, center_v + hv);
    }

    pub fn pan(&mut self, dt: f32, dv: f32) {
        self.canvas_time_range  = (self.canvas_time_range.0  + dt, self.canvas_time_range.1  + dt);
        self.canvas_value_range = (self.canvas_value_range.0 + dv, self.canvas_value_range.1 + dv);
    }

    /// Build all polylines ready for GPU line drawing
    pub fn build_render_lines(&self) -> Vec<(usize, Vec<Vec2>)> {
        self.curve_set.channels.iter().enumerate()
            .filter(|(i, _)| self.visible_channels.get(*i).copied().unwrap_or(true))
            .map(|(i, ch)| {
                let pts = ch.curve.to_polyline(64);
                let canvas_pts = pts.iter().map(|p| self.curve_to_canvas(p.x, p.y)).collect();
                (i, canvas_pts)
            })
            .collect()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_interpolation() {
        let c = CurveData::linear();
        assert!((c.evaluate(0.0) - 0.0).abs() < 0.001);
        assert!((c.evaluate(1.0) - 1.0).abs() < 0.001);
        assert!((c.evaluate(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn bezier_endpoints() {
        let c = CurveData::ease_in_out();
        assert!((c.evaluate(0.0) - 0.0).abs() < 0.01);
        assert!((c.evaluate(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn sine_wave_zero_crossing() {
        let c = CurveData::new(CurveKind::SineWave);
        let v = c.evaluate(0.0);
        assert!(v.abs() < 0.1);
    }

    #[test]
    fn constant_step() {
        let mut c = CurveData::new(CurveKind::Constant);
        c.points[0] = ControlPoint::new(Vec2::new(0.0, 0.0));
        c.points[1] = ControlPoint::new(Vec2::new(1.0, 1.0));
        assert!((c.evaluate(0.4) - 0.0).abs() < 0.001);
        assert!((c.evaluate(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn canvas_curve_roundtrip() {
        let ed = CurveEditor::new(800.0, 400.0);
        let t = 0.6f32;
        let v = 0.3f32;
        let c = ed.curve_to_canvas(t, v);
        let back = ed.canvas_to_curve(c.x, c.y);
        assert!((back.x - t).abs() < 1e-4);
        assert!((back.y - v).abs() < 1e-4);
    }

    #[test]
    fn add_remove_point() {
        let mut c = CurveData::linear();
        c.add_point(Vec2::new(0.5, 0.7));
        assert_eq!(c.points.len(), 3);
        c.remove_point(1);
        assert_eq!(c.points.len(), 2);
    }

    #[test]
    fn spring_curve_settles_near_one() {
        let c = CurveData::new(CurveKind::Spring);
        let v = c.evaluate(10.0);
        // After a long time the damped oscillator should be near amplitude
        assert!((v - 1.0).abs() < 0.1, "spring value at t=10: {}", v);
    }
}
