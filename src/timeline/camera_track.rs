//! Cinematic camera control — shots, tracks, rigs, and animators.
//!
//! The camera system is layered:
//!
//! * `CameraShot` — a single timed camera pose with transition metadata.
//! * `CameraTrack` — ordered sequence of shots; evaluates a `CameraFrame`
//!   at any time `t` using transition curves.
//! * `CameraRig` — a virtual camera following a Catmull-Rom spline with an
//!   optional look-at target.
//! * `DollyTrack` — rail-based camera movement (waypoints + speed profile).
//! * `CameraAnimator` — combines a `CameraTrack` and a `CameraRig` into a
//!   single interface that outputs `CameraFrame` each tick.
//! * `VirtualCamera` — physical camera parameters (DoF, sensor, aperture).

use glam::{Vec3, Vec4, Quat, Mat4};
use std::collections::HashMap;

// ── EasingCurve ───────────────────────────────────────────────────────────────

/// Mathematical easing curve for interpolating between values.
#[derive(Debug, Clone)]
pub enum EasingCurve {
    /// No interpolation — constant step at t=0.
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Instant jump at t=0.5.
    Step,
    /// Springy overshoot.
    Spring { stiffness: f32, damping: f32 },
    /// User-defined cubic spline: list of [t, value] control points.
    /// Points are sorted by t; t and value are both in [0,1].
    Custom(Vec<[f32; 2]>),
}

/// Evaluate an easing curve at normalized time `t` ∈ [0, 1].
/// Returns a value in approximately [0, 1] (may exceed for spring).
pub fn easing_evaluate(curve: &EasingCurve, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match curve {
        EasingCurve::Linear    => t,
        EasingCurve::EaseIn    => t * t,
        EasingCurve::EaseOut   => 1.0 - (1.0 - t) * (1.0 - t),
        EasingCurve::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        EasingCurve::Step => {
            if t < 0.5 { 0.0 } else { 1.0 }
        }
        EasingCurve::Spring { stiffness, damping } => {
            // Critically-damped spring from 0 to 1
            let omega = stiffness.max(0.01).sqrt();
            let zeta  = damping / (2.0 * omega);
            if zeta >= 1.0 {
                // Overdamped
                let r1 = -omega * (zeta - (zeta * zeta - 1.0).max(0.0).sqrt());
                let r2 = -omega * (zeta + (zeta * zeta - 1.0).max(0.0).sqrt());
                let c2 = (r1) / (r1 - r2);
                let c1 = 1.0 - c2;
                1.0 - (c1 * (r1 * t).exp() + c2 * (r2 * t).exp())
            } else {
                // Underdamped
                let wd = omega * (1.0 - zeta * zeta).max(0.0).sqrt();
                let decay = (-zeta * omega * t).exp();
                1.0 - decay * (
                    (wd * t).cos() + (zeta * omega / wd.max(f32::EPSILON)) * (wd * t).sin()
                )
            }
        }
        EasingCurve::Custom(points) => {
            if points.is_empty() { return t; }
            if points.len() == 1 { return points[0][1]; }
            // Find surrounding points
            let mut lo = [0.0f32, 0.0f32];
            let mut hi = [1.0f32, 1.0f32];
            for &pt in points.iter() {
                if pt[0] <= t { lo = pt; }
                else          { hi = pt; break; }
            }
            let range = hi[0] - lo[0];
            if range < f32::EPSILON { return lo[1]; }
            let local_t = (t - lo[0]) / range;
            lo[1] + local_t * (hi[1] - lo[1])
        }
    }
}

// ── ShotTransition ────────────────────────────────────────────────────────────

/// How the director transitions between two `CameraShot`s.
#[derive(Debug, Clone)]
pub enum ShotTransition {
    /// Hard cut — no interpolation.
    Cut,
    /// Cross-dissolve over `duration` seconds (blends rendered frames).
    Dissolve { duration: f32 },
    /// Wipe from left — visual transition, camera cuts immediately.
    WipeLeft { duration: f32 },
    /// Zoom-blur — radial blur toward cut point.
    ZoomBlur { duration: f32 },
    /// Fade to black then in.
    FadeThrough { duration: f32 },
}

impl ShotTransition {
    pub fn duration(&self) -> f32 {
        match self {
            ShotTransition::Cut                 => 0.0,
            ShotTransition::Dissolve  { duration } => *duration,
            ShotTransition::WipeLeft  { duration } => *duration,
            ShotTransition::ZoomBlur  { duration } => *duration,
            ShotTransition::FadeThrough { duration } => *duration,
        }
    }

    pub fn is_cut(&self) -> bool { matches!(self, ShotTransition::Cut) }
}

// ── CameraShot ────────────────────────────────────────────────────────────────

/// A single cinematic shot with start/end pose and duration.
#[derive(Debug, Clone)]
pub struct CameraShot {
    /// World-space start position.
    pub start_pos:    Vec3,
    /// World-space end position.
    pub end_pos:      Vec3,
    /// Start orientation.
    pub start_rot:    Quat,
    /// End orientation.
    pub end_rot:      Quat,
    /// Start vertical field-of-view (degrees).
    pub start_fov:    f32,
    /// End field-of-view.
    pub end_fov:      f32,
    /// Near clip plane.
    pub near:         f32,
    /// Far clip plane.
    pub far:          f32,
    /// Duration of this shot in seconds.
    pub duration:     f32,
    /// Easing curve applied to position interpolation.
    pub pos_easing:   EasingCurve,
    /// Easing curve applied to rotation interpolation.
    pub rot_easing:   EasingCurve,
    /// Easing curve applied to FOV interpolation.
    pub fov_easing:   EasingCurve,
    /// Transition INTO the next shot.
    pub transition:   ShotTransition,
    /// Optional label for event system.
    pub label:        Option<String>,
}

impl CameraShot {
    pub fn new(duration: f32) -> Self {
        Self {
            start_pos:  Vec3::ZERO,
            end_pos:    Vec3::ZERO,
            start_rot:  Quat::IDENTITY,
            end_rot:    Quat::IDENTITY,
            start_fov:  60.0,
            end_fov:    60.0,
            near:       0.1,
            far:        1000.0,
            duration,
            pos_easing: EasingCurve::EaseInOut,
            rot_easing: EasingCurve::EaseInOut,
            fov_easing: EasingCurve::Linear,
            transition: ShotTransition::Cut,
            label:      None,
        }
    }

    pub fn from_to(from: Vec3, to: Vec3, duration: f32) -> Self {
        Self { start_pos: from, end_pos: to, ..Self::new(duration) }
    }

    pub fn static_at(pos: Vec3, rot: Quat, fov: f32, duration: f32) -> Self {
        Self {
            start_pos: pos, end_pos: pos,
            start_rot: rot, end_rot: rot,
            start_fov: fov, end_fov: fov,
            ..Self::new(duration)
        }
    }

    pub fn with_rotation(mut self, from: Quat, to: Quat) -> Self {
        self.start_rot = from;
        self.end_rot   = to;
        self
    }

    pub fn with_fov(mut self, from: f32, to: f32) -> Self {
        self.start_fov = from;
        self.end_fov   = to;
        self
    }

    pub fn with_clip(mut self, near: f32, far: f32) -> Self {
        self.near = near;
        self.far  = far;
        self
    }

    pub fn with_pos_easing(mut self, e: EasingCurve) -> Self { self.pos_easing = e; self }
    pub fn with_rot_easing(mut self, e: EasingCurve) -> Self { self.rot_easing = e; self }
    pub fn with_fov_easing(mut self, e: EasingCurve) -> Self { self.fov_easing = e; self }
    pub fn with_transition(mut self, t: ShotTransition) -> Self { self.transition = t; self }
    pub fn with_label(mut self, l: impl Into<String>) -> Self { self.label = Some(l.into()); self }

    /// Evaluate this shot at normalized time `t` ∈ [0, 1].
    pub fn evaluate(&self, t: f32) -> CameraFrame {
        let pt = easing_evaluate(&self.pos_easing, t);
        let rt = easing_evaluate(&self.rot_easing, t);
        let ft = easing_evaluate(&self.fov_easing, t);

        let pos = self.start_pos + pt * (self.end_pos - self.start_pos);
        let rot = self.start_rot.slerp(self.end_rot, rt);
        let fov = self.start_fov + ft * (self.end_fov - self.start_fov);

        CameraFrame { pos, rot, fov, near: self.near, far: self.far }
    }
}

// ── CameraFrame ────────────────────────────────────────────────────────────────

/// The complete camera state at a single instant.
#[derive(Debug, Clone, Copy)]
pub struct CameraFrame {
    pub pos:  Vec3,
    pub rot:  Quat,
    /// Vertical field of view in degrees.
    pub fov:  f32,
    pub near: f32,
    pub far:  f32,
}

impl CameraFrame {
    pub fn identity() -> Self {
        Self { pos: Vec3::ZERO, rot: Quat::IDENTITY, fov: 60.0, near: 0.1, far: 1000.0 }
    }

    /// View matrix (world-to-camera).
    pub fn view_matrix(&self) -> Mat4 {
        let forward = self.rot * Vec3::NEG_Z;
        let up      = self.rot * Vec3::Y;
        Mat4::look_at_rh(self.pos, self.pos + forward, up)
    }

    /// Projection matrix.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        let fov_rad = self.fov.to_radians();
        Mat4::perspective_rh(fov_rad, aspect, self.near, self.far)
    }

    /// Linearly blend toward another frame (no lerp builtin).
    pub fn blend(&self, other: &CameraFrame, t: f32) -> CameraFrame {
        let t = t.clamp(0.0, 1.0);
        CameraFrame {
            pos:  self.pos  + t * (other.pos  - self.pos),
            rot:  self.rot.slerp(other.rot, t),
            fov:  self.fov  + t * (other.fov  - self.fov),
            near: self.near + t * (other.near  - self.near),
            far:  self.far  + t * (other.far   - self.far),
        }
    }
}

impl Default for CameraFrame {
    fn default() -> Self { Self::identity() }
}

// ── CameraTrack ───────────────────────────────────────────────────────────────

/// Ordered sequence of `CameraShot`s composing a full cinematic sequence.
#[derive(Debug, Clone)]
pub struct CameraTrack {
    pub shots:  Vec<CameraShot>,
    /// Cached cumulative start times per shot.
    starts: Vec<f32>,
    pub total_duration: f32,
}

impl CameraTrack {
    pub fn new() -> Self {
        Self { shots: Vec::new(), starts: Vec::new(), total_duration: 0.0 }
    }

    pub fn push(&mut self, shot: CameraShot) {
        self.starts.push(self.total_duration);
        self.total_duration += shot.duration;
        self.shots.push(shot);
    }

    pub fn with_shot(mut self, shot: CameraShot) -> Self {
        self.push(shot);
        self
    }

    pub fn shot_count(&self) -> usize { self.shots.len() }

    /// Evaluate the track at time `t` (seconds from start).
    pub fn evaluate(&self, t: f32) -> CameraFrame {
        if self.shots.is_empty() {
            return CameraFrame::identity();
        }

        let t = t.clamp(0.0, self.total_duration);

        // Find which shot covers time t
        let idx = self.shot_index_at(t);
        let shot  = &self.shots[idx];
        let start = self.starts[idx];
        let local = (t - start) / shot.duration.max(f32::EPSILON);
        let local = local.clamp(0.0, 1.0);

        shot.evaluate(local)
    }

    /// Find the shot index active at absolute time `t`.
    fn shot_index_at(&self, t: f32) -> usize {
        let mut best = 0;
        for (i, &s) in self.starts.iter().enumerate() {
            if s <= t { best = i; }
            else      { break;    }
        }
        best
    }

    /// The active shot at time `t`.
    pub fn active_shot(&self, t: f32) -> Option<&CameraShot> {
        if self.shots.is_empty() { return None; }
        Some(&self.shots[self.shot_index_at(t.clamp(0.0, self.total_duration))])
    }

    /// Which transition type is incoming at time `t`.
    pub fn transition_at(&self, t: f32) -> &ShotTransition {
        if let Some(shot) = self.active_shot(t) {
            &shot.transition
        } else {
            &ShotTransition::Cut
        }
    }

    /// All shot labels and their absolute start times.
    pub fn markers(&self) -> Vec<(String, f32)> {
        self.shots.iter().zip(self.starts.iter())
            .filter_map(|(shot, &start)| {
                shot.label.as_ref().map(|l| (l.clone(), start))
            })
            .collect()
    }
}

impl Default for CameraTrack {
    fn default() -> Self { Self::new() }
}

// ── Catmull-Rom Spline ────────────────────────────────────────────────────────

/// A Catmull-Rom spline through an ordered set of `Vec3` control points.
#[derive(Debug, Clone)]
pub struct CatmullRomSpline {
    pub points:      Vec<Vec3>,
    /// Arc-length table: (cumulative_arc_length, t_in_segment)
    arc_table:       Vec<(f32, f32, usize)>, // (arc_len, local_t, segment_idx)
    pub total_length: f32,
    /// Whether to loop back to start.
    pub closed:      bool,
}

impl CatmullRomSpline {
    pub fn new(points: Vec<Vec3>) -> Self {
        let mut spline = Self {
            points,
            arc_table:    Vec::new(),
            total_length: 0.0,
            closed:       false,
        };
        spline.build_arc_table();
        spline
    }

    pub fn closed(mut self) -> Self {
        self.closed = true;
        self.build_arc_table();
        self
    }

    fn num_segments(&self) -> usize {
        if self.points.len() < 2 { return 0; }
        if self.closed { self.points.len() } else { self.points.len() - 1 }
    }

    fn get_point(&self, idx: i32) -> Vec3 {
        let n = self.points.len() as i32;
        if n == 0 { return Vec3::ZERO; }
        if self.closed {
            self.points[idx.rem_euclid(n) as usize]
        } else {
            self.points[idx.clamp(0, n - 1) as usize]
        }
    }

    /// Evaluate the spline at segment `seg` and local t ∈ [0,1].
    pub fn evaluate_segment(&self, seg: usize, t: f32) -> Vec3 {
        let i = seg as i32;
        let p0 = self.get_point(i - 1);
        let p1 = self.get_point(i);
        let p2 = self.get_point(i + 1);
        let p3 = self.get_point(i + 2);

        let t2 = t * t;
        let t3 = t2 * t;

        // Catmull-Rom basis
        let b0 = -t3 + 2.0 * t2 - t;
        let b1 =  3.0 * t3 - 5.0 * t2 + 2.0;
        let b2 = -3.0 * t3 + 4.0 * t2 + t;
        let b3 =  t3 - t2;

        (p0 * b0 + p1 * b1 + p2 * b2 + p3 * b3) * 0.5
    }

    /// Evaluate tangent at segment `seg` and local t.
    pub fn tangent_segment(&self, seg: usize, t: f32) -> Vec3 {
        let i = seg as i32;
        let p0 = self.get_point(i - 1);
        let p1 = self.get_point(i);
        let p2 = self.get_point(i + 1);
        let p3 = self.get_point(i + 2);

        let t2 = t * t;
        let b0 = -3.0 * t2 + 4.0 * t - 1.0;
        let b1 =  9.0 * t2 - 10.0 * t;
        let b2 = -9.0 * t2 + 8.0 * t + 1.0;
        let b3 =  3.0 * t2 - 2.0 * t;

        (p0 * b0 + p1 * b1 + p2 * b2 + p3 * b3) * 0.5
    }

    fn build_arc_table(&mut self) {
        self.arc_table.clear();
        self.total_length = 0.0;

        let segs = self.num_segments();
        if segs == 0 { return; }

        let steps_per_seg = 32usize;
        let mut prev = self.evaluate_segment(0, 0.0);
        self.arc_table.push((0.0, 0.0, 0));

        for seg in 0..segs {
            for step in 1..=steps_per_seg {
                let t = step as f32 / steps_per_seg as f32;
                let curr = self.evaluate_segment(seg, t);
                self.total_length += (curr - prev).length();
                prev = curr;
                self.arc_table.push((self.total_length, t, seg));
            }
        }
    }

    /// Evaluate the spline at arc-length parameter `s` ∈ [0, total_length].
    pub fn evaluate_arc(&self, s: f32) -> Vec3 {
        if self.arc_table.is_empty() {
            return self.points.first().copied().unwrap_or(Vec3::ZERO);
        }
        let s = s.clamp(0.0, self.total_length);
        // Binary search for surrounding entries
        let idx = self.arc_table.partition_point(|&(arc, _, _)| arc <= s);
        if idx == 0 {
            return self.evaluate_segment(0, 0.0);
        }
        if idx >= self.arc_table.len() {
            let &(_, t, seg) = self.arc_table.last().unwrap();
            return self.evaluate_segment(seg, t);
        }
        let (arc0, t0, seg0) = self.arc_table[idx - 1];
        let (arc1, t1, seg1) = self.arc_table[idx];
        let range = arc1 - arc0;
        if range < f32::EPSILON {
            return self.evaluate_segment(seg0, t0);
        }
        let alpha = (s - arc0) / range;
        if seg0 == seg1 {
            let t = t0 + alpha * (t1 - t0);
            self.evaluate_segment(seg0, t)
        } else {
            // Crossing a segment boundary
            let a = self.evaluate_segment(seg0, t0);
            let b = self.evaluate_segment(seg1, t1);
            a + alpha * (b - a)
        }
    }

    /// Evaluate tangent at arc-length parameter `s`.
    pub fn tangent_arc(&self, s: f32) -> Vec3 {
        if self.arc_table.is_empty() { return Vec3::Z; }
        let s = s.clamp(0.0, self.total_length);
        let idx = self.arc_table.partition_point(|&(arc, _, _)| arc <= s);
        let idx = idx.clamp(1, self.arc_table.len() - 1);
        let (_, t, seg) = self.arc_table[idx];
        let tan = self.tangent_segment(seg, t);
        if tan.length_squared() > f32::EPSILON { tan.normalize() } else { Vec3::Z }
    }

    /// Normalized arc parameter [0,1].
    pub fn evaluate_normalized(&self, u: f32) -> Vec3 {
        self.evaluate_arc(u * self.total_length)
    }

    /// Tangent at normalized arc parameter.
    pub fn tangent_normalized(&self, u: f32) -> Vec3 {
        self.tangent_arc(u * self.total_length)
    }
}

// ── CameraRig ─────────────────────────────────────────────────────────────────

/// A virtual camera that follows a spline path and optionally looks at a target.
#[derive(Debug, Clone)]
pub struct CameraRig {
    pub path:           CatmullRomSpline,
    /// Optional world-space look-at target.
    pub look_at:        Option<Vec3>,
    /// Camera-up override (world up if None).
    pub up_override:    Option<Vec3>,
    /// Field of view in degrees.
    pub fov:            f32,
    pub near:           f32,
    pub far:            f32,
    /// Path offset: 0 = start, 1 = end.
    pub path_position:  f32,
}

impl CameraRig {
    pub fn new(path: CatmullRomSpline) -> Self {
        Self {
            path,
            look_at:       None,
            up_override:   None,
            fov:           60.0,
            near:          0.1,
            far:           1000.0,
            path_position: 0.0,
        }
    }

    pub fn with_look_at(mut self, target: Vec3) -> Self { self.look_at = Some(target); self }
    pub fn with_up(mut self, up: Vec3) -> Self { self.up_override = Some(up); self }
    pub fn with_fov(mut self, fov: f32) -> Self { self.fov = fov; self }
    pub fn with_clip(mut self, near: f32, far: f32) -> Self { self.near = near; self.far = far; self }

    /// Evaluate the rig at path position `u` ∈ [0, 1].
    pub fn evaluate(&self, u: f32) -> CameraFrame {
        let pos = self.path.evaluate_normalized(u);

        let forward = if let Some(target) = self.look_at {
            let d = target - pos;
            if d.length_squared() > f32::EPSILON { d.normalize() } else { Vec3::NEG_Z }
        } else {
            let t = self.path.tangent_normalized(u);
            if t.length_squared() > f32::EPSILON { t } else { Vec3::NEG_Z }
        };

        let up = self.up_override.unwrap_or(Vec3::Y);
        let right   = forward.cross(up);
        let right_n = if right.length_squared() > f32::EPSILON { right.normalize() } else { Vec3::X };
        let up_cam  = right_n.cross(forward).normalize();

        let rot = Quat::from_mat4(&Mat4::from_cols(
            right_n.extend(0.0),
            up_cam.extend(0.0),
            (-forward).extend(0.0),
            Vec4::W,
        ));

        CameraFrame { pos, rot, fov: self.fov, near: self.near, far: self.far }
    }

    /// Advance path position by `delta` (0..1 normalized).
    pub fn advance(&mut self, delta: f32) {
        self.path_position = (self.path_position + delta).clamp(0.0, 1.0);
    }

    /// Current camera frame.
    pub fn frame(&self) -> CameraFrame {
        self.evaluate(self.path_position)
    }
}

// ── SpeedProfile ──────────────────────────────────────────────────────────────

/// A piecewise-linear speed profile for dolly/rail movement.
#[derive(Debug, Clone)]
pub struct SpeedProfile {
    /// List of (normalized_position, speed_multiplier) pairs, sorted by position.
    entries: Vec<(f32, f32)>,
}

impl SpeedProfile {
    pub fn constant(speed: f32) -> Self {
        Self { entries: vec![(0.0, speed), (1.0, speed)] }
    }

    pub fn new() -> Self {
        Self { entries: vec![(0.0, 1.0), (1.0, 1.0)] }
    }

    pub fn add_point(mut self, pos: f32, speed: f32) -> Self {
        let idx = self.entries.partition_point(|&(p, _)| p <= pos);
        self.entries.insert(idx, (pos.clamp(0.0, 1.0), speed.max(0.0)));
        self
    }

    /// Evaluate speed at normalized position `u`.
    pub fn evaluate(&self, u: f32) -> f32 {
        if self.entries.is_empty() { return 1.0; }
        let idx = self.entries.partition_point(|&(p, _)| p <= u);
        if idx == 0 { return self.entries[0].1; }
        if idx >= self.entries.len() { return self.entries.last().unwrap().1; }
        let (p0, s0) = self.entries[idx - 1];
        let (p1, s1) = self.entries[idx];
        let range = p1 - p0;
        if range < f32::EPSILON { return s0; }
        let t = (u - p0) / range;
        s0 + t * (s1 - s0)
    }
}

impl Default for SpeedProfile {
    fn default() -> Self { Self::new() }
}

// ── DollyTrack ────────────────────────────────────────────────────────────────

/// Rail-based camera movement along a linear path with a speed profile.
#[derive(Debug, Clone)]
pub struct DollyTrack {
    /// Ordered rail waypoints.
    pub waypoints:    Vec<Vec3>,
    /// Total rail length (computed on build).
    pub total_length: f32,
    /// Speed profile along the rail (normalized position → speed m/s).
    pub speed_profile: SpeedProfile,
    /// Look-at target (if any).
    pub look_at:       Option<Vec3>,
    /// Current position along the rail in metres.
    pub position:      f32,
    pub fov:           f32,
    pub near:          f32,
    pub far:           f32,
}

impl DollyTrack {
    pub fn new(waypoints: Vec<Vec3>) -> Self {
        let length = waypoints.windows(2)
            .map(|w| (w[1] - w[0]).length())
            .sum::<f32>();
        Self {
            waypoints,
            total_length: length,
            speed_profile: SpeedProfile::constant(2.0),
            look_at:       None,
            position:      0.0,
            fov:           60.0,
            near:          0.1,
            far:           1000.0,
        }
    }

    pub fn with_speed_profile(mut self, sp: SpeedProfile) -> Self { self.speed_profile = sp; self }
    pub fn with_look_at(mut self, t: Vec3) -> Self { self.look_at = Some(t); self }
    pub fn with_fov(mut self, fov: f32) -> Self { self.fov = fov; self }

    /// Advance position by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        let u = if self.total_length > f32::EPSILON {
            self.position / self.total_length
        } else {
            0.0
        };
        let speed = self.speed_profile.evaluate(u);
        self.position = (self.position + speed * dt).clamp(0.0, self.total_length);
    }

    /// Evaluate world position at track distance `s`.
    pub fn position_at(&self, s: f32) -> Vec3 {
        if self.waypoints.is_empty() { return Vec3::ZERO; }
        if self.waypoints.len() == 1 { return self.waypoints[0]; }
        let s = s.clamp(0.0, self.total_length);
        let mut acc = 0.0f32;
        for i in 0..self.waypoints.len() - 1 {
            let seg_len = (self.waypoints[i + 1] - self.waypoints[i]).length();
            if acc + seg_len >= s || i == self.waypoints.len() - 2 {
                let local = if seg_len > f32::EPSILON { (s - acc) / seg_len } else { 0.0 };
                let local = local.clamp(0.0, 1.0);
                return self.waypoints[i] + local * (self.waypoints[i + 1] - self.waypoints[i]);
            }
            acc += seg_len;
        }
        *self.waypoints.last().unwrap()
    }

    /// Current camera frame.
    pub fn frame(&self) -> CameraFrame {
        let pos     = self.position_at(self.position);
        let forward = if let Some(target) = self.look_at {
            let d = target - pos;
            if d.length_squared() > f32::EPSILON { d.normalize() } else { Vec3::NEG_Z }
        } else {
            // Face direction of travel
            let next = self.position_at((self.position + 0.01).min(self.total_length));
            let d = next - pos;
            if d.length_squared() > f32::EPSILON { d.normalize() } else { Vec3::NEG_Z }
        };

        let up    = Vec3::Y;
        let right = forward.cross(up);
        let right_n = if right.length_squared() > f32::EPSILON { right.normalize() } else { Vec3::X };
        let up_cam  = right_n.cross(forward).normalize();

        let rot = Quat::from_mat4(&Mat4::from_cols(
            right_n.extend(0.0),
            up_cam.extend(0.0),
            (-forward).extend(0.0),
            Vec4::W,
        ));

        CameraFrame { pos, rot, fov: self.fov, near: self.near, far: self.far }
    }

    /// Normalized position [0,1].
    pub fn normalized_position(&self) -> f32 {
        if self.total_length > f32::EPSILON {
            self.position / self.total_length
        } else {
            0.0
        }
    }

    pub fn is_at_end(&self) -> bool {
        self.position >= self.total_length - f32::EPSILON
    }
}

// ── VirtualCamera ─────────────────────────────────────────────────────────────

/// Physical camera parameters for depth-of-field calculations.
#[derive(Debug, Clone)]
pub struct VirtualCamera {
    /// Distance to focus plane in world units.
    pub focus_distance: f32,
    /// Aperture diameter (f-stop like: lower = shallower DoF).
    pub aperture:       f32,
    /// Sensor height in millimetres (full-frame = 24mm).
    pub sensor_height:  f32,
    /// Focal length in millimetres.
    pub focal_length:   f32,
    /// Motion blur shutter angle (degrees, 180 = classic film look).
    pub shutter_angle:  f32,
}

impl VirtualCamera {
    pub fn new() -> Self {
        Self {
            focus_distance: 10.0,
            aperture:       2.8,
            sensor_height:  24.0,
            focal_length:   35.0,
            shutter_angle:  180.0,
        }
    }

    pub fn cinematic() -> Self {
        Self {
            focus_distance: 8.0,
            aperture:       1.8,
            sensor_height:  24.0,
            focal_length:   50.0,
            shutter_angle:  180.0,
        }
    }

    /// Circle of confusion radius at depth `d` from camera.
    pub fn coc_radius(&self, depth: f32) -> f32 {
        let fl  = self.focal_length / 1000.0; // mm → m
        let fd  = self.focus_distance;
        let ap  = self.focal_length / self.aperture.max(f32::EPSILON) / 1000.0;
        let sensor_m = self.sensor_height / 1000.0;
        // |fd - d| / d * (fl * ap) / (fd - fl) clamped
        let num = (fd - depth).abs() * fl * ap;
        let den = depth.max(f32::EPSILON) * (fd - fl).abs().max(f32::EPSILON);
        // Normalise to [0,1] using sensor size
        (num / den / sensor_m).clamp(0.0, 1.0)
    }

    /// Vertical field-of-view in degrees for this sensor/focal length.
    pub fn fov_degrees(&self) -> f32 {
        2.0 * ((self.sensor_height / (2.0 * self.focal_length.max(f32::EPSILON))).atan()).to_degrees()
    }

    /// Shutter speed in seconds for given FPS.
    pub fn shutter_speed(&self, fps: f32) -> f32 {
        (self.shutter_angle / 360.0) / fps.max(f32::EPSILON)
    }

    pub fn with_focus(mut self, d: f32) -> Self { self.focus_distance = d; self }
    pub fn with_aperture(mut self, a: f32) -> Self { self.aperture = a; self }
    pub fn with_focal_length(mut self, fl: f32) -> Self { self.focal_length = fl; self }
    pub fn with_sensor(mut self, h: f32) -> Self { self.sensor_height = h; self }
}

impl Default for VirtualCamera {
    fn default() -> Self { Self::new() }
}

// ── CameraAnimatorMode ────────────────────────────────────────────────────────

/// Which source drives the `CameraAnimator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraAnimatorMode {
    /// Use the `CameraTrack` (shot-based).
    Track,
    /// Use the `CameraRig` (spline-based).
    Rig,
    /// Use the `DollyTrack`.
    Dolly,
    /// Output is blended between Track and Rig.
    Blend,
}

// ── CameraAnimator ────────────────────────────────────────────────────────────

/// Combines a `CameraTrack`, a `CameraRig`, and a `DollyTrack` into one
/// interface.  Each tick returns a `CameraFrame` ready for rendering.
pub struct CameraAnimator {
    pub track:      CameraTrack,
    pub rig:        Option<CameraRig>,
    pub dolly:      Option<DollyTrack>,
    pub virtual_cam: VirtualCamera,
    pub mode:       CameraAnimatorMode,
    /// Blend factor between Track (0) and Rig (1) when mode = Blend.
    pub blend:      f32,
    /// Current playback time.
    pub time:       f32,
    pub speed:      f32,
    pub looping:    bool,
    pub playing:    bool,
    /// Post-process shake offset applied after evaluation.
    pub shake_offset: Vec3,
}

impl CameraAnimator {
    pub fn new() -> Self {
        Self {
            track:       CameraTrack::new(),
            rig:         None,
            dolly:       None,
            virtual_cam: VirtualCamera::new(),
            mode:        CameraAnimatorMode::Track,
            blend:       0.0,
            time:        0.0,
            speed:       1.0,
            looping:     false,
            playing:     false,
            shake_offset: Vec3::ZERO,
        }
    }

    pub fn with_track(mut self, t: CameraTrack) -> Self { self.track = t; self }
    pub fn with_rig(mut self, r: CameraRig) -> Self { self.rig = Some(r); self }
    pub fn with_dolly(mut self, d: DollyTrack) -> Self { self.dolly = Some(d); self }
    pub fn with_virtual_cam(mut self, v: VirtualCamera) -> Self { self.virtual_cam = v; self }
    pub fn with_mode(mut self, m: CameraAnimatorMode) -> Self { self.mode = m; self }
    pub fn with_blend(mut self, b: f32) -> Self { self.blend = b.clamp(0.0, 1.0); self }
    pub fn with_speed(mut self, s: f32) -> Self { self.speed = s; self }
    pub fn with_looping(mut self, l: bool) -> Self { self.looping = l; self }

    pub fn play(&mut self) { self.playing = true; }
    pub fn pause(&mut self) { self.playing = false; }
    pub fn stop(&mut self) { self.playing = false; self.time = 0.0; }
    pub fn seek(&mut self, t: f32) { self.time = t.max(0.0); }

    /// Advance the animator by `dt` seconds and return the current `CameraFrame`.
    pub fn tick(&mut self, dt: f32) -> CameraFrame {
        if self.playing {
            self.time += dt * self.speed;
            let duration = self.duration();
            if duration > 0.0 && self.time > duration {
                if self.looping {
                    self.time -= duration;
                } else {
                    self.time = duration;
                    self.playing = false;
                }
            }
        }

        // Advance dolly independently
        if let Some(ref mut dolly) = self.dolly {
            if self.playing {
                dolly.tick(dt * self.speed);
            }
        }

        let mut frame = self.evaluate(self.time);
        frame.pos = frame.pos + self.shake_offset;
        frame
    }

    /// Evaluate without advancing time.
    pub fn evaluate(&self, t: f32) -> CameraFrame {
        match self.mode {
            CameraAnimatorMode::Track => {
                self.track.evaluate(t)
            }
            CameraAnimatorMode::Rig => {
                let u = if self.track.total_duration > 0.0 {
                    (t / self.track.total_duration).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if let Some(ref rig) = self.rig {
                    rig.evaluate(u)
                } else {
                    self.track.evaluate(t)
                }
            }
            CameraAnimatorMode::Dolly => {
                if let Some(ref dolly) = self.dolly {
                    dolly.frame()
                } else {
                    self.track.evaluate(t)
                }
            }
            CameraAnimatorMode::Blend => {
                let track_frame = self.track.evaluate(t);
                let rig_frame = if let Some(ref rig) = self.rig {
                    let u = if self.track.total_duration > 0.0 {
                        (t / self.track.total_duration).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    rig.evaluate(u)
                } else {
                    track_frame
                };
                track_frame.blend(&rig_frame, self.blend)
            }
        }
    }

    pub fn duration(&self) -> f32 {
        match self.mode {
            CameraAnimatorMode::Track | CameraAnimatorMode::Blend => self.track.total_duration,
            CameraAnimatorMode::Rig   => {
                self.rig.as_ref().map(|_| self.track.total_duration).unwrap_or(0.0)
            }
            CameraAnimatorMode::Dolly => {
                self.dolly.as_ref().map(|d| d.total_length / 2.0_f32.max(f32::EPSILON)).unwrap_or(0.0)
            }
        }
    }

    pub fn progress(&self) -> f32 {
        let d = self.duration();
        if d < f32::EPSILON { 0.0 } else { (self.time / d).clamp(0.0, 1.0) }
    }

    pub fn is_playing(&self) -> bool { self.playing }
    pub fn is_finished(&self) -> bool { !self.playing && self.time >= self.duration() }

    /// Apply a shake displacement this frame.
    pub fn apply_shake(&mut self, offset: Vec3) {
        self.shake_offset = offset;
    }
}

impl Default for CameraAnimator {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── EasingCurve ──────────────────────────────────────────────────────────

    #[test]
    fn easing_linear_identity() {
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let v = easing_evaluate(&EasingCurve::Linear, t);
            assert!((v - t).abs() < 1e-5, "linear easing failed at t={}", t);
        }
    }

    #[test]
    fn easing_ease_in_out_symmetry() {
        let a = easing_evaluate(&EasingCurve::EaseInOut, 0.25);
        let b = easing_evaluate(&EasingCurve::EaseInOut, 0.75);
        assert!((a - (1.0 - b)).abs() < 1e-5);
    }

    #[test]
    fn easing_step_binary() {
        assert_eq!(easing_evaluate(&EasingCurve::Step, 0.3), 0.0);
        assert_eq!(easing_evaluate(&EasingCurve::Step, 0.7), 1.0);
    }

    #[test]
    fn easing_spring_settles_near_one() {
        let curve = EasingCurve::Spring { stiffness: 50.0, damping: 14.0 };
        let v = easing_evaluate(&curve, 1.0);
        // Should be reasonably close to 1 after a full second
        assert!(v > 0.5, "spring didn't settle: v={}", v);
    }

    // ── CameraShot ────────────────────────────────────────────────────────────

    #[test]
    fn shot_evaluate_at_zero_is_start() {
        let shot = CameraShot::from_to(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 2.0);
        let frame = shot.evaluate(0.0);
        assert!((frame.pos - Vec3::ZERO).length() < 1e-5);
    }

    #[test]
    fn shot_evaluate_at_one_is_end() {
        let shot = CameraShot::from_to(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 2.0);
        let frame = shot.evaluate(1.0);
        assert!((frame.pos - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn shot_fov_interpolates() {
        let shot = CameraShot::new(1.0).with_fov(60.0, 90.0);
        let frame = shot.evaluate(0.5);
        assert!((frame.fov - 75.0).abs() < 0.5, "fov={}", frame.fov);
    }

    // ── CameraTrack ───────────────────────────────────────────────────────────

    #[test]
    fn track_single_shot() {
        let mut track = CameraTrack::new();
        track.push(CameraShot::static_at(Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY, 60.0, 3.0));
        let frame = track.evaluate(1.5);
        assert!((frame.pos - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn track_two_shots_transition() {
        let mut track = CameraTrack::new();
        track.push(CameraShot::from_to(Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), 2.0));
        track.push(CameraShot::from_to(Vec3::new(5.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0), 2.0));
        assert!((track.total_duration - 4.0).abs() < 1e-5);
        let frame = track.evaluate(2.0); // start of second shot
        assert!(frame.pos.x >= 4.9 && frame.pos.x <= 5.1, "pos.x={}", frame.pos.x);
    }

    #[test]
    fn track_markers() {
        let mut track = CameraTrack::new();
        track.push(CameraShot::new(1.0).with_label("intro"));
        track.push(CameraShot::new(2.0));
        let markers = track.markers();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].0, "intro");
        assert!((markers[0].1 - 0.0).abs() < 1e-5);
    }

    // ── CatmullRomSpline ──────────────────────────────────────────────────────

    #[test]
    fn spline_passes_through_endpoints() {
        let pts = vec![Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0)];
        let spline = CatmullRomSpline::new(pts);
        let start = spline.evaluate_normalized(0.0);
        let end   = spline.evaluate_normalized(1.0);
        assert!((start - Vec3::ZERO).length() < 0.5);
        assert!((end - Vec3::new(10.0, 0.0, 0.0)).length() < 0.5);
    }

    #[test]
    fn spline_arc_length_positive() {
        let pts = vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 0.0)];
        let spline = CatmullRomSpline::new(pts);
        assert!(spline.total_length > 0.0);
    }

    // ── DollyTrack ────────────────────────────────────────────────────────────

    #[test]
    fn dolly_advances_on_tick() {
        let mut dolly = DollyTrack::new(vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)]);
        dolly.tick(1.0);
        assert!(dolly.position > 0.0);
    }

    #[test]
    fn dolly_frame_is_valid() {
        let mut dolly = DollyTrack::new(vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 10.0)]);
        dolly.tick(1.0);
        let frame = dolly.frame();
        assert!(frame.pos.length() >= 0.0); // non-NaN check
        assert!(frame.fov > 0.0);
    }

    // ── VirtualCamera ─────────────────────────────────────────────────────────

    #[test]
    fn virtual_camera_fov() {
        let cam = VirtualCamera::new();
        let fov = cam.fov_degrees();
        assert!(fov > 10.0 && fov < 120.0, "fov={}", fov);
    }

    #[test]
    fn virtual_camera_coc_at_focus_is_zero() {
        let cam = VirtualCamera::new();
        let coc = cam.coc_radius(cam.focus_distance);
        assert!(coc < 0.01, "coc={}", coc);
    }

    // ── CameraAnimator ────────────────────────────────────────────────────────

    #[test]
    fn animator_track_mode() {
        let mut track = CameraTrack::new();
        track.push(CameraShot::static_at(Vec3::new(0.0, 5.0, 0.0), Quat::IDENTITY, 60.0, 2.0));

        let mut anim = CameraAnimator::new()
            .with_track(track)
            .with_mode(CameraAnimatorMode::Track);
        anim.play();

        let frame = anim.tick(0.5);
        assert!((frame.pos.y - 5.0).abs() < 0.01, "pos.y={}", frame.pos.y);
    }

    #[test]
    fn animator_progress() {
        let mut track = CameraTrack::new();
        track.push(CameraShot::new(4.0));
        let mut anim = CameraAnimator::new().with_track(track);
        anim.play();
        anim.tick(2.0);
        let p = anim.progress();
        assert!((p - 0.5).abs() < 0.01, "progress={}", p);
    }

    #[test]
    fn animator_stops_at_end() {
        let mut track = CameraTrack::new();
        track.push(CameraShot::new(1.0));
        let mut anim = CameraAnimator::new().with_track(track);
        anim.play();
        anim.tick(2.0);
        assert!(anim.is_finished());
    }

    #[test]
    fn animator_loops() {
        let mut track = CameraTrack::new();
        track.push(CameraShot::new(1.0));
        let mut anim = CameraAnimator::new()
            .with_track(track)
            .with_looping(true);
        anim.play();
        anim.tick(1.5);
        assert!(anim.is_playing());
        assert!(anim.time < 1.0, "time should have wrapped: {}", anim.time);
    }

    #[test]
    fn camera_frame_blend() {
        let a = CameraFrame { pos: Vec3::ZERO, rot: Quat::IDENTITY, fov: 60.0, near: 0.1, far: 100.0 };
        let b = CameraFrame { pos: Vec3::new(10.0, 0.0, 0.0), rot: Quat::IDENTITY, fov: 90.0, near: 0.1, far: 100.0 };
        let mid = a.blend(&b, 0.5);
        assert!((mid.pos.x - 5.0).abs() < 0.01);
        assert!((mid.fov - 75.0).abs() < 0.01);
    }

    #[test]
    fn shot_transition_duration() {
        assert_eq!(ShotTransition::Cut.duration(), 0.0);
        assert!((ShotTransition::Dissolve { duration: 0.5 }.duration() - 0.5).abs() < 1e-5);
    }
}
