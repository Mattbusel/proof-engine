//! Keyframe tracks — time-stamped values with interpolated playback.
//!
//! A `KeyframeTrack<T>` stores a list of (time, value) pairs and provides
//! continuous interpolation between them using configurable easing per segment.
//!
//! This is the foundation for cinematic cutscenes, camera paths, and
//! math-driven animation curves exported from design tools.

use super::{Lerp, Easing};
use glam::{Vec2, Vec3, Vec4};

// ── Keyframe ──────────────────────────────────────────────────────────────────

/// A single keyframe: a value at a specific time.
#[derive(Clone, Debug)]
pub struct Keyframe<T: Lerp + Clone + std::fmt::Debug> {
    pub time:  f32,
    pub value: T,
    /// Easing applied from this keyframe to the next.
    pub easing_out: Easing,
    /// Optional tangent scale for Hermite interpolation (1.0 = auto).
    pub tangent: f32,
}

impl<T: Lerp + Clone + std::fmt::Debug> Keyframe<T> {
    pub fn new(time: f32, value: T) -> Self {
        Self { time, value, easing_out: Easing::EaseInOutCubic, tangent: 1.0 }
    }

    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing_out = easing;
        self
    }

    pub fn linear(time: f32, value: T) -> Self {
        Self { time, value, easing_out: Easing::Linear, tangent: 1.0 }
    }

    pub fn step(time: f32, value: T) -> Self {
        Self { time, value, easing_out: Easing::Step, tangent: 0.0 }
    }
}

// ── KeyframeTrack ─────────────────────────────────────────────────────────────

/// An ordered list of keyframes with time-based interpolation.
///
/// Automatically sorts keyframes by time on construction and rebuilds
/// segment lookup on insertion.
pub struct KeyframeTrack<T: Lerp + Clone + std::fmt::Debug> {
    pub frames:    Vec<Keyframe<T>>,
    pub extrapolate: ExtrapolateMode,
}

/// How to handle time values outside the keyframe range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExtrapolateMode {
    /// Clamp to first/last keyframe value.
    Clamp,
    /// Loop the animation back to the start.
    Loop,
    /// Ping-pong between start and end.
    PingPong,
    /// Linearly extrapolate using the first/last two keyframes.
    Linear,
}

impl<T: Lerp + Clone + std::fmt::Debug> KeyframeTrack<T> {
    pub fn new(extrapolate: ExtrapolateMode) -> Self {
        Self { frames: Vec::new(), extrapolate }
    }

    /// Create a track from a list of keyframes, sorted by time.
    pub fn from_keyframes(mut frames: Vec<Keyframe<T>>, extrapolate: ExtrapolateMode) -> Self {
        frames.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
        Self { frames, extrapolate }
    }

    /// Insert a keyframe, keeping the track sorted.
    pub fn insert(&mut self, frame: Keyframe<T>) {
        let pos = self.frames.partition_point(|f| f.time < frame.time);
        self.frames.insert(pos, frame);
    }

    pub fn is_empty(&self) -> bool { self.frames.is_empty() }

    /// Duration from first to last keyframe.
    pub fn duration(&self) -> f32 {
        if self.frames.len() < 2 { return 0.0; }
        self.frames.last().unwrap().time - self.frames.first().unwrap().time
    }

    /// Start time of the track.
    pub fn start_time(&self) -> f32 {
        self.frames.first().map(|f| f.time).unwrap_or(0.0)
    }

    /// End time of the track.
    pub fn end_time(&self) -> f32 {
        self.frames.last().map(|f| f.time).unwrap_or(0.0)
    }

    /// Evaluate the track at the given time.
    pub fn evaluate(&self, time: f32) -> T {
        if self.frames.is_empty() { return T::zero(); }
        if self.frames.len() == 1 { return self.frames[0].value.clone(); }

        let (start, end) = (self.start_time(), self.end_time());
        let span = (end - start).max(f32::EPSILON);

        let local_t = match self.extrapolate {
            ExtrapolateMode::Clamp => time.clamp(start, end),
            ExtrapolateMode::Loop  => {
                let offset = time - start;
                start + ((offset % span) + span) % span
            }
            ExtrapolateMode::PingPong => {
                let offset = (time - start) / span;
                let cycle = offset.floor() as u32;
                let frac  = offset - offset.floor();
                start + if cycle % 2 == 0 { frac * span } else { (1.0 - frac) * span }
            }
            ExtrapolateMode::Linear => time,
        };

        // Binary search for the segment containing local_t
        let right_idx = self.frames.partition_point(|f| f.time <= local_t);

        if right_idx == 0 {
            // Before first keyframe — return first value (or extrapolate)
            if self.extrapolate == ExtrapolateMode::Linear && self.frames.len() >= 2 {
                let a = &self.frames[0];
                let b = &self.frames[1];
                let seg = (b.time - a.time).max(f32::EPSILON);
                let t = (local_t - a.time) / seg;
                return T::lerp(&a.value, &b.value, t);
            }
            return self.frames[0].value.clone();
        }

        if right_idx >= self.frames.len() {
            // After last keyframe
            if self.extrapolate == ExtrapolateMode::Linear && self.frames.len() >= 2 {
                let n = self.frames.len();
                let a = &self.frames[n - 2];
                let b = &self.frames[n - 1];
                let seg = (b.time - a.time).max(f32::EPSILON);
                let t = (local_t - a.time) / seg;
                return T::lerp(&a.value, &b.value, t);
            }
            return self.frames.last().unwrap().value.clone();
        }

        // Interpolate between frames[right_idx - 1] and frames[right_idx]
        let left  = &self.frames[right_idx - 1];
        let right = &self.frames[right_idx];
        let seg_duration = (right.time - left.time).max(f32::EPSILON);
        let t = ((local_t - left.time) / seg_duration).clamp(0.0, 1.0);
        let curved_t = left.easing_out.apply(t);
        T::lerp(&left.value, &right.value, curved_t)
    }

    /// Return all times where the value reaches (approximately) a given threshold.
    /// Used for detecting when an animation crosses a boundary.
    pub fn crossing_times(&self, threshold: f32, steps_per_segment: u32) -> Vec<f32>
    where T: Into<f32> + Copy,
    {
        let mut crossings = Vec::new();
        if self.frames.len() < 2 { return crossings; }

        for w in self.frames.windows(2) {
            let a = &w[0];
            let b = &w[1];
            let seg_duration = (b.time - a.time).max(f32::EPSILON);
            let dt = seg_duration / steps_per_segment as f32;

            let mut prev_val: f32 = a.value.clone().into();
            let mut prev_t = a.time;

            for s in 1..=steps_per_segment {
                let t = a.time + s as f32 * dt;
                let v: f32 = self.evaluate(t).into();
                if (prev_val < threshold) != (v < threshold) {
                    // Linearly interpolate crossing time
                    let cross_frac = (threshold - prev_val) / (v - prev_val).max(f32::EPSILON);
                    crossings.push(prev_t + cross_frac * dt);
                }
                prev_val = v;
                prev_t = t;
            }
        }
        crossings
    }

    /// Sample the track at uniform time steps and return the values.
    pub fn bake(&self, step: f32) -> Vec<(f32, T)> {
        if self.frames.is_empty() { return Vec::new(); }
        let start = self.start_time();
        let end   = self.end_time();
        let mut result = Vec::new();
        let mut t = start;
        while t <= end + f32::EPSILON {
            result.push((t, self.evaluate(t)));
            t += step;
        }
        result
    }
}

// ── MultiTrack ────────────────────────────────────────────────────────────────

/// A collection of named `KeyframeTrack<f32>` tracks played together.
///
/// Ideal for driving multiple engine parameters from a single timeline clock.
pub struct MultiTrack {
    pub tracks:  std::collections::HashMap<String, KeyframeTrack<f32>>,
    pub elapsed: f32,
    pub looping: bool,
    duration:    f32,
}

impl MultiTrack {
    pub fn new(looping: bool) -> Self {
        Self { tracks: std::collections::HashMap::new(), elapsed: 0.0, looping, duration: 0.0 }
    }

    pub fn add(&mut self, name: impl Into<String>, track: KeyframeTrack<f32>) {
        self.duration = self.duration.max(track.end_time());
        self.tracks.insert(name.into(), track);
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt;
        if self.looping && self.elapsed > self.duration {
            self.elapsed -= self.duration;
        }
    }

    pub fn get(&self, name: &str) -> f32 {
        self.tracks.get(name).map(|t| t.evaluate(self.elapsed)).unwrap_or(0.0)
    }

    pub fn is_done(&self) -> bool {
        !self.looping && self.elapsed >= self.duration
    }

    pub fn reset(&mut self) { self.elapsed = 0.0; }

    pub fn duration(&self) -> f32 { self.duration }
}

// ── Camera path ───────────────────────────────────────────────────────────────

/// A smooth camera path through a list of positions and targets.
///
/// Uses `KeyframeTrack<Vec3>` internally for both position and look-at target.
pub struct CameraPath {
    pub positions: KeyframeTrack<Vec3>,
    pub targets:   KeyframeTrack<Vec3>,
    pub fov:       KeyframeTrack<f32>,
    pub elapsed:   f32,
    pub speed:     f32,
    pub looping:   bool,
}

impl CameraPath {
    pub fn new(speed: f32, looping: bool) -> Self {
        Self {
            positions: KeyframeTrack::new(
                if looping { ExtrapolateMode::Loop } else { ExtrapolateMode::Clamp }
            ),
            targets: KeyframeTrack::new(
                if looping { ExtrapolateMode::Loop } else { ExtrapolateMode::Clamp }
            ),
            fov: KeyframeTrack::new(ExtrapolateMode::Clamp),
            elapsed: 0.0,
            speed,
            looping,
        }
    }

    /// Add a camera waypoint: position, look-at target, fov at given time.
    pub fn add_waypoint(&mut self, time: f32, position: Vec3, target: Vec3, fov: f32) {
        self.positions.insert(Keyframe::new(time, position)
            .with_easing(Easing::EaseInOutCubic));
        self.targets.insert(Keyframe::new(time, target)
            .with_easing(Easing::EaseInOutCubic));
        self.fov.insert(Keyframe::new(time, fov)
            .with_easing(Easing::EaseInOutSine));
    }

    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt * self.speed;
        let duration = self.positions.end_time();
        if self.looping && self.elapsed > duration {
            self.elapsed -= duration;
        }
    }

    pub fn position(&self) -> Vec3 { self.positions.evaluate(self.elapsed) }
    pub fn target(&self)   -> Vec3 { self.targets.evaluate(self.elapsed) }
    pub fn fov(&self)      -> f32  { self.fov.evaluate(self.elapsed) }

    pub fn is_done(&self) -> bool {
        !self.looping && self.elapsed >= self.positions.end_time()
    }

    /// Build a cinematic orbit path around a center point.
    pub fn orbit(center: Vec3, radius: f32, height: f32, duration: f32, fov: f32) -> Self {
        let mut path = Self::new(1.0, true);
        let steps = 16;
        for i in 0..=steps {
            let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
            let pos = center + Vec3::new(angle.cos() * radius, height, angle.sin() * radius);
            let t = (i as f32 / steps as f32) * duration;
            path.add_waypoint(t, pos, center, fov);
        }
        path
    }

    /// Build a flythrough path through a list of points.
    pub fn flythrough(waypoints: &[(Vec3, Vec3)], duration_each: f32, fov: f32) -> Self {
        let mut path = Self::new(1.0, false);
        for (i, (pos, target)) in waypoints.iter().enumerate() {
            path.add_waypoint(i as f32 * duration_each, *pos, *target, fov);
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn track_clamp_extrapolation() {
        let mut track: KeyframeTrack<f32> = KeyframeTrack::new(ExtrapolateMode::Clamp);
        track.insert(Keyframe::linear(0.0, 0.0));
        track.insert(Keyframe::linear(1.0, 1.0));
        assert!((track.evaluate(-1.0) - 0.0).abs() < 1e-5, "before start should clamp to first");
        assert!((track.evaluate(2.0) - 1.0).abs() < 1e-5, "after end should clamp to last");
    }

    #[test]
    fn track_midpoint_linear() {
        let mut track: KeyframeTrack<f32> = KeyframeTrack::new(ExtrapolateMode::Clamp);
        track.insert(Keyframe::linear(0.0, 0.0));
        track.insert(Keyframe::linear(2.0, 4.0));
        let mid = track.evaluate(1.0);
        assert!((mid - 2.0).abs() < 1e-4, "midpoint of linear should be 2.0, got {mid}");
    }

    #[test]
    fn track_loop_wraps() {
        let mut track: KeyframeTrack<f32> = KeyframeTrack::new(ExtrapolateMode::Loop);
        track.insert(Keyframe::linear(0.0, 0.0));
        track.insert(Keyframe::linear(1.0, 1.0));
        let v = track.evaluate(1.5);
        assert!(v >= 0.0 && v <= 1.0, "looped value should wrap: {v}");
    }

    #[test]
    fn track_sorted_on_insert() {
        let mut track: KeyframeTrack<f32> = KeyframeTrack::new(ExtrapolateMode::Clamp);
        track.insert(Keyframe::linear(2.0, 2.0));
        track.insert(Keyframe::linear(0.0, 0.0));
        track.insert(Keyframe::linear(1.0, 1.0));
        assert_eq!(track.frames[0].time, 0.0);
        assert_eq!(track.frames[1].time, 1.0);
        assert_eq!(track.frames[2].time, 2.0);
    }

    #[test]
    fn vec3_track_interpolates() {
        let mut track: KeyframeTrack<Vec3> = KeyframeTrack::new(ExtrapolateMode::Clamp);
        track.insert(Keyframe::linear(0.0, Vec3::ZERO));
        track.insert(Keyframe::linear(1.0, Vec3::ONE));
        let mid = track.evaluate(0.5);
        assert!((mid.x - 0.5).abs() < 1e-4);
    }

    #[test]
    fn bake_returns_correct_count() {
        let mut track: KeyframeTrack<f32> = KeyframeTrack::new(ExtrapolateMode::Clamp);
        track.insert(Keyframe::linear(0.0, 0.0));
        track.insert(Keyframe::linear(1.0, 1.0));
        let baked = track.bake(0.1);
        assert!(baked.len() >= 10 && baked.len() <= 12, "expected ~11 samples, got {}", baked.len());
    }

    #[test]
    fn camera_path_orbit() {
        let path = CameraPath::orbit(Vec3::ZERO, 5.0, 2.0, 10.0, 60.0);
        let pos = path.position();
        let dist = glam::Vec2::new(pos.x, pos.z).length();
        assert!((dist - 5.0).abs() < 0.5, "orbit radius should be ~5, got {dist}");
    }
}
