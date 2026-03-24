//! General-purpose event sequencer — tracks, keyframes, sequences, and blend trees.
//!
//! This module provides building blocks for animating any value over time:
//!
//! * `Track<T>` — sorted list of `Keyframe<T>` with configurable interpolation.
//! * `FloatTrack`, `Vec3Track`, `QuatTrack`, `ColorTrack`, `BoolTrack` —
//!   typed convenience wrappers around `Track<T>`.
//! * `EventTrack` — fires string-keyed callbacks at specific times.
//! * `Sequence` — named collection of heterogeneous tracks with global duration.
//! * `SequencePlayer` — drives a `Sequence`, supports time-scaling, looping,
//!   and ping-pong.
//! * `AnimationClip` — maps bone names to transform tracks for skeletal animation.
//! * `BlendTree` — blends multiple `AnimationClip`s with per-clip weights.

use glam::{Vec3, Quat};
use std::collections::HashMap;

// ── Interpolation ─────────────────────────────────────────────────────────────

/// How a `Track` interpolates between keyframes.
#[derive(Debug, Clone)]
pub enum Interpolation {
    /// Hold the previous keyframe's value until the next.
    Constant,
    /// Linear blend between surrounding keyframes.
    Linear,
    /// Hermite cubic (smooth slopes).
    Cubic,
    /// Bézier with two normalised control points [0,1] for t and value.
    Bezier { cp1: f32, cp2: f32 },
}

impl Interpolation {
    /// Evaluate the interpolation parameter at normalised t ∈ [0,1].
    fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Interpolation::Constant => 0.0,
            Interpolation::Linear   => t,
            Interpolation::Cubic    => {
                // Smooth-step
                t * t * (3.0 - 2.0 * t)
            }
            Interpolation::Bezier { cp1, cp2 } => {
                // Approximate cubic Bézier via Newton-Raphson (3 iterations)
                // The curve goes (0,0)→(cp1,cp1)→(cp2,cp2)→(1,1) in t-value space
                // We solve for the parameter u such that B_x(u) = t.
                let p1 = *cp1;
                let p2 = *cp2;
                let mut u = t;
                for _ in 0..4 {
                    let bx  = 3.0 * p1 * u * (1.0-u)*(1.0-u)
                            + 3.0 * p2 * u * u * (1.0 - u)
                            + u * u * u;
                    let dbx = 3.0 * p1 * ((1.0-u)*(1.0-u) - 2.0*u*(1.0-u))
                            + 3.0 * p2 * (2.0*u*(1.0-u) - u*u)
                            + 3.0 * u * u;
                    if dbx.abs() > f32::EPSILON {
                        u -= (bx - t) / dbx;
                    }
                }
                // Evaluate value curve B_y(u)
                3.0 * p1 * u * (1.0-u)*(1.0-u)
                + 3.0 * p2 * u * u * (1.0-u)
                + u * u * u
            }
        }
    }
}

// ── Keyframe ──────────────────────────────────────────────────────────────────

/// A single timed keyframe with a value and interpolation mode.
#[derive(Debug, Clone)]
pub struct Keyframe<T: Clone> {
    pub time:          f32,
    pub value:         T,
    pub interpolation: Interpolation,
}

impl<T: Clone> Keyframe<T> {
    pub fn new(time: f32, value: T) -> Self {
        Self { time, value, interpolation: Interpolation::Linear }
    }

    pub fn constant(time: f32, value: T) -> Self {
        Self { time, value, interpolation: Interpolation::Constant }
    }

    pub fn cubic(time: f32, value: T) -> Self {
        Self { time, value, interpolation: Interpolation::Cubic }
    }

    pub fn bezier(time: f32, value: T, cp1: f32, cp2: f32) -> Self {
        Self { time, value, interpolation: Interpolation::Bezier { cp1, cp2 } }
    }
}

// ── Blendable trait ───────────────────────────────────────────────────────────

/// Types that can be linearly interpolated.
pub trait Blendable: Clone {
    fn blend(a: &Self, b: &Self, t: f32) -> Self;
}

impl Blendable for f32 {
    fn blend(a: &f32, b: &f32, t: f32) -> f32 {
        a + t * (b - a)
    }
}

impl Blendable for Vec3 {
    fn blend(a: &Vec3, b: &Vec3, t: f32) -> Vec3 {
        Vec3::new(
            a.x + t * (b.x - a.x),
            a.y + t * (b.y - a.y),
            a.z + t * (b.z - a.z),
        )
    }
}

impl Blendable for Quat {
    fn blend(a: &Quat, b: &Quat, t: f32) -> Quat {
        a.slerp(*b, t)
    }
}

impl Blendable for [f32; 4] {
    fn blend(a: &[f32; 4], b: &[f32; 4], t: f32) -> [f32; 4] {
        [
            a[0] + t * (b[0] - a[0]),
            a[1] + t * (b[1] - a[1]),
            a[2] + t * (b[2] - a[2]),
            a[3] + t * (b[3] - a[3]),
        ]
    }
}

impl Blendable for bool {
    fn blend(a: &bool, _b: &bool, _t: f32) -> bool {
        *a  // bool uses constant interpolation
    }
}

// ── Track<T> ──────────────────────────────────────────────────────────────────

/// A sorted list of `Keyframe<T>` that can be sampled at any time.
#[derive(Debug, Clone)]
pub struct Track<T: Clone> {
    /// Sorted by `keyframe.time`.
    pub keyframes: Vec<Keyframe<T>>,
    /// Default value when no keyframes exist.
    pub default:   Option<T>,
}

impl<T: Clone + Blendable> Track<T> {
    pub fn new() -> Self {
        Self { keyframes: Vec::new(), default: None }
    }

    pub fn with_default(mut self, v: T) -> Self {
        self.default = Some(v);
        self
    }

    /// Insert a keyframe (maintains sorted order).
    pub fn insert(&mut self, kf: Keyframe<T>) {
        let idx = self.keyframes.partition_point(|k| k.time <= kf.time);
        self.keyframes.insert(idx, kf);
    }

    /// Shorthand: linear keyframe at time.
    pub fn key(&mut self, time: f32, value: T) {
        self.insert(Keyframe::new(time, value));
    }

    /// Duration from first to last keyframe.
    pub fn duration(&self) -> f32 {
        match (self.keyframes.first(), self.keyframes.last()) {
            (Some(a), Some(b)) => (b.time - a.time).max(0.0),
            _ => 0.0,
        }
    }

    /// Sample the track at absolute time `t`.
    pub fn sample(&self, t: f32) -> Option<T> {
        if self.keyframes.is_empty() {
            return self.default.clone();
        }

        let first = &self.keyframes[0];
        let last  = &self.keyframes[self.keyframes.len() - 1];

        if t <= first.time { return Some(first.value.clone()); }
        if t >= last.time  { return Some(last.value.clone());  }

        // Binary search for surrounding keyframes
        let idx = self.keyframes.partition_point(|k| k.time <= t);
        // idx is the first keyframe with time > t
        let lo = &self.keyframes[idx - 1];
        let hi = &self.keyframes[idx];

        let range = hi.time - lo.time;
        if range < f32::EPSILON {
            return Some(lo.value.clone());
        }
        let raw_t = (t - lo.time) / range;
        let ease_t = lo.interpolation.apply(raw_t);

        // For Constant, return the lo value without blending
        match &lo.interpolation {
            Interpolation::Constant => Some(lo.value.clone()),
            _ => Some(T::blend(&lo.value, &hi.value, ease_t)),
        }
    }

    /// Sample with a fallback value.
    pub fn sample_or(&self, t: f32, fallback: T) -> T {
        self.sample(t).unwrap_or(fallback)
    }

    pub fn is_empty(&self) -> bool { self.keyframes.is_empty() }
    pub fn len(&self) -> usize { self.keyframes.len() }
}

impl<T: Clone + Blendable> Default for Track<T> {
    fn default() -> Self { Self::new() }
}

// ── Typed track aliases ────────────────────────────────────────────────────────

/// Track for animating `f32` values.
pub type FloatTrack = Track<f32>;
/// Track for animating `Vec3` values.
pub type Vec3Track  = Track<Vec3>;
/// Track for animating `Quat` rotations (uses slerp).
pub type QuatTrack  = Track<Quat>;
/// Track for animating RGBA color `[f32; 4]`.
pub type ColorTrack = Track<[f32; 4]>;
/// Track for boolean switching (constant interpolation).
pub type BoolTrack  = Track<bool>;

// ── EventKeyframe / EventTrack ────────────────────────────────────────────────

/// A timed event with a string id and optional numeric payload.
#[derive(Debug, Clone)]
pub struct EventKeyframe {
    pub time:    f32,
    pub id:      String,
    pub payload: Vec<f32>,
}

impl EventKeyframe {
    pub fn new(time: f32, id: impl Into<String>) -> Self {
        Self { time, id: id.into(), payload: Vec::new() }
    }

    pub fn with_payload(mut self, p: Vec<f32>) -> Self {
        self.payload = p;
        self
    }
}

/// Fires callbacks at specific times during playback.
#[derive(Debug, Clone)]
pub struct EventTrack {
    pub events: Vec<EventKeyframe>,
}

impl EventTrack {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Insert an event, maintaining time-sorted order.
    pub fn insert(&mut self, ev: EventKeyframe) {
        let idx = self.events.partition_point(|e| e.time <= ev.time);
        self.events.insert(idx, ev);
    }

    pub fn at(&mut self, time: f32, id: impl Into<String>) {
        self.insert(EventKeyframe::new(time, id));
    }

    pub fn at_with_payload(&mut self, time: f32, id: impl Into<String>, payload: Vec<f32>) {
        self.insert(EventKeyframe::new(time, id).with_payload(payload));
    }

    /// Return all events whose time is in `(prev_time, current_time]`.
    pub fn events_in_range(&self, prev: f32, current: f32) -> Vec<&EventKeyframe> {
        self.events.iter()
            .filter(|e| e.time > prev && e.time <= current)
            .collect()
    }

    /// All events in range (cloned).
    pub fn drain_range(&self, prev: f32, current: f32) -> Vec<EventKeyframe> {
        self.events.iter()
            .filter(|e| e.time > prev && e.time <= current)
            .cloned()
            .collect()
    }

    pub fn duration(&self) -> f32 {
        self.events.last().map(|e| e.time).unwrap_or(0.0)
    }

    pub fn is_empty(&self) -> bool { self.events.is_empty() }
}

impl Default for EventTrack {
    fn default() -> Self { Self::new() }
}

// ── TimelineMarker ─────────────────────────────────────────────────────────────

/// A named time marker for director events or chapter labels.
#[derive(Debug, Clone)]
pub struct TimelineMarker {
    pub time:  f32,
    pub label: String,
}

impl TimelineMarker {
    pub fn new(time: f32, label: impl Into<String>) -> Self {
        Self { time, label: label.into() }
    }
}

// ── SequencePlaybackState ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequencePlaybackState {
    Stopped,
    Playing,
    Paused,
    Finished,
}

// ── SequencePlayMode ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequencePlayMode {
    /// Play once then stop.
    Once,
    /// Loop indefinitely.
    Loop,
    /// Reverse direction at each end (ping-pong).
    PingPong,
}

// ── Sequence ──────────────────────────────────────────────────────────────────

/// A named collection of typed tracks with a fixed global duration.
#[derive(Debug, Clone)]
pub struct Sequence {
    pub name:       String,
    pub duration:   f32,
    pub float_tracks: HashMap<String, FloatTrack>,
    pub vec3_tracks:  HashMap<String, Vec3Track>,
    pub quat_tracks:  HashMap<String, QuatTrack>,
    pub color_tracks: HashMap<String, ColorTrack>,
    pub bool_tracks:  HashMap<String, BoolTrack>,
    pub event_track:  EventTrack,
    pub markers:      Vec<TimelineMarker>,
}

impl Sequence {
    pub fn new(name: impl Into<String>, duration: f32) -> Self {
        Self {
            name:         name.into(),
            duration,
            float_tracks: HashMap::new(),
            vec3_tracks:  HashMap::new(),
            quat_tracks:  HashMap::new(),
            color_tracks: HashMap::new(),
            bool_tracks:  HashMap::new(),
            event_track:  EventTrack::new(),
            markers:      Vec::new(),
        }
    }

    // ── Track access ─────────────────────────────────────────────────────────

    pub fn float_track(&mut self, name: impl Into<String>) -> &mut FloatTrack {
        self.float_tracks.entry(name.into()).or_insert_with(FloatTrack::new)
    }

    pub fn vec3_track(&mut self, name: impl Into<String>) -> &mut Vec3Track {
        self.vec3_tracks.entry(name.into()).or_insert_with(Vec3Track::new)
    }

    pub fn quat_track(&mut self, name: impl Into<String>) -> &mut QuatTrack {
        self.quat_tracks.entry(name.into()).or_insert_with(QuatTrack::new)
    }

    pub fn color_track(&mut self, name: impl Into<String>) -> &mut ColorTrack {
        self.color_tracks.entry(name.into()).or_insert_with(ColorTrack::new)
    }

    pub fn bool_track(&mut self, name: impl Into<String>) -> &mut BoolTrack {
        self.bool_tracks.entry(name.into()).or_insert_with(BoolTrack::new)
    }

    // ── Sampling ─────────────────────────────────────────────────────────────

    pub fn sample_float(&self, name: &str, t: f32) -> Option<f32> {
        self.float_tracks.get(name)?.sample(t)
    }

    pub fn sample_vec3(&self, name: &str, t: f32) -> Option<Vec3> {
        self.vec3_tracks.get(name)?.sample(t)
    }

    pub fn sample_quat(&self, name: &str, t: f32) -> Option<Quat> {
        self.quat_tracks.get(name)?.sample(t)
    }

    pub fn sample_color(&self, name: &str, t: f32) -> Option<[f32; 4]> {
        self.color_tracks.get(name)?.sample(t)
    }

    pub fn sample_bool(&self, name: &str, t: f32) -> Option<bool> {
        self.bool_tracks.get(name)?.sample(t)
    }

    // ── Markers ───────────────────────────────────────────────────────────────

    pub fn add_marker(&mut self, time: f32, label: impl Into<String>) {
        let idx = self.markers.partition_point(|m| m.time <= time);
        self.markers.insert(idx, TimelineMarker::new(time, label));
    }

    pub fn markers_in_range(&self, prev: f32, current: f32) -> Vec<&TimelineMarker> {
        self.markers.iter()
            .filter(|m| m.time > prev && m.time <= current)
            .collect()
    }
}

// ── SequencePlayer ────────────────────────────────────────────────────────────

/// Plays back a `Sequence`, handling time-scaling, looping, and ping-pong.
pub struct SequencePlayer {
    pub sequence:    Sequence,
    pub time:        f32,
    pub speed:       f32,
    pub state:       SequencePlaybackState,
    pub play_mode:   SequencePlayMode,
    /// True when currently playing in reverse (ping-pong mode).
    pub reversing:   bool,
    /// How many times playback has looped.
    pub loop_count:  u32,
    /// Previous frame's time (for event range checks).
    prev_time:       f32,
    /// Pending events collected this tick.
    pending_events:  Vec<EventKeyframe>,
}

impl SequencePlayer {
    pub fn new(sequence: Sequence) -> Self {
        Self {
            sequence,
            time:           0.0,
            speed:          1.0,
            state:          SequencePlaybackState::Stopped,
            play_mode:      SequencePlayMode::Once,
            reversing:      false,
            loop_count:     0,
            prev_time:      0.0,
            pending_events: Vec::new(),
        }
    }

    pub fn with_speed(mut self, s: f32) -> Self { self.speed = s; self }
    pub fn with_play_mode(mut self, m: SequencePlayMode) -> Self { self.play_mode = m; self }

    pub fn play(&mut self) {
        self.state    = SequencePlaybackState::Playing;
        self.reversing = false;
    }

    pub fn pause(&mut self) {
        if self.state == SequencePlaybackState::Playing {
            self.state = SequencePlaybackState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == SequencePlaybackState::Paused {
            self.state = SequencePlaybackState::Playing;
        }
    }

    pub fn stop(&mut self) {
        self.state    = SequencePlaybackState::Stopped;
        self.time     = 0.0;
        self.prev_time = 0.0;
        self.reversing = false;
        self.loop_count = 0;
    }

    pub fn seek(&mut self, t: f32) {
        let clamped   = t.clamp(0.0, self.sequence.duration);
        self.prev_time = clamped;
        self.time      = clamped;
    }

    /// Advance by `dt` seconds.  Returns events that fired this tick.
    pub fn tick(&mut self, dt: f32) -> Vec<EventKeyframe> {
        self.pending_events.clear();

        if self.state != SequencePlaybackState::Playing {
            return Vec::new();
        }

        let effective_dt = dt * self.speed * if self.reversing { -1.0 } else { 1.0 };
        let next_time    = self.time + effective_dt;
        let duration     = self.sequence.duration;

        let (new_time, wrapped) = if self.reversing {
            if next_time < 0.0 {
                (0.0, true)
            } else {
                (next_time, false)
            }
        } else {
            if next_time > duration {
                (duration, true)
            } else {
                (next_time, false)
            }
        };

        // Collect events in traversed range
        let (range_lo, range_hi) = if effective_dt >= 0.0 {
            (self.prev_time, new_time)
        } else {
            (new_time, self.prev_time)
        };
        let events = self.sequence.event_track.drain_range(range_lo, range_hi);
        self.pending_events.extend(events.into_iter());

        self.prev_time = new_time;
        self.time      = new_time;

        if wrapped {
            match self.play_mode {
                SequencePlayMode::Once => {
                    self.state = SequencePlaybackState::Finished;
                }
                SequencePlayMode::Loop => {
                    self.loop_count += 1;
                    self.time      = if self.reversing { duration } else { 0.0 };
                    self.prev_time = self.time;
                }
                SequencePlayMode::PingPong => {
                    self.loop_count += 1;
                    self.reversing  = !self.reversing;
                    self.time       = if self.reversing { duration } else { 0.0 };
                    self.prev_time  = self.time;
                }
            }
        }

        self.pending_events.clone()
    }

    // ── Sampling shortcuts ────────────────────────────────────────────────────

    pub fn float(&self, name: &str) -> Option<f32> {
        self.sequence.sample_float(name, self.time)
    }

    pub fn vec3(&self, name: &str) -> Option<Vec3> {
        self.sequence.sample_vec3(name, self.time)
    }

    pub fn quat(&self, name: &str) -> Option<Quat> {
        self.sequence.sample_quat(name, self.time)
    }

    pub fn color(&self, name: &str) -> Option<[f32; 4]> {
        self.sequence.sample_color(name, self.time)
    }

    pub fn bool_val(&self, name: &str) -> Option<bool> {
        self.sequence.sample_bool(name, self.time)
    }

    pub fn progress(&self) -> f32 {
        let d = self.sequence.duration;
        if d < f32::EPSILON { 0.0 } else { (self.time / d).clamp(0.0, 1.0) }
    }

    pub fn is_playing(&self)  -> bool { self.state == SequencePlaybackState::Playing  }
    pub fn is_finished(&self) -> bool { self.state == SequencePlaybackState::Finished }
    pub fn is_stopped(&self)  -> bool { self.state == SequencePlaybackState::Stopped  }
}

// ── BoneTransform ─────────────────────────────────────────────────────────────

/// Per-bone transform sample.
#[derive(Debug, Clone, Copy)]
pub struct BoneTransform {
    pub translation: Vec3,
    pub rotation:    Quat,
    pub scale:       Vec3,
}

impl BoneTransform {
    pub fn identity() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation:    Quat::IDENTITY,
            scale:       Vec3::ONE,
        }
    }

    /// Linearly blend two bone transforms (no lerp builtin).
    pub fn blend(a: &BoneTransform, b: &BoneTransform, t: f32) -> BoneTransform {
        let t = t.clamp(0.0, 1.0);
        BoneTransform {
            translation: Vec3::new(
                a.translation.x + t * (b.translation.x - a.translation.x),
                a.translation.y + t * (b.translation.y - a.translation.y),
                a.translation.z + t * (b.translation.z - a.translation.z),
            ),
            rotation: a.rotation.slerp(b.rotation, t),
            scale: Vec3::new(
                a.scale.x + t * (b.scale.x - a.scale.x),
                a.scale.y + t * (b.scale.y - a.scale.y),
                a.scale.z + t * (b.scale.z - a.scale.z),
            ),
        }
    }

    pub fn to_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for BoneTransform {
    fn default() -> Self { Self::identity() }
}

// ── AnimationClip ─────────────────────────────────────────────────────────────

/// Maps bone names to rotation + translation tracks for skeletal animation.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name:           String,
    pub duration:       f32,
    /// Per-bone rotation tracks.
    pub rot_tracks:     HashMap<String, QuatTrack>,
    /// Per-bone translation tracks.
    pub pos_tracks:     HashMap<String, Vec3Track>,
    /// Per-bone scale tracks.
    pub scale_tracks:   HashMap<String, FloatTrack>,
    /// Root motion track (optional).
    pub root_motion:    Option<Vec3Track>,
    /// Events embedded in this clip.
    pub event_track:    EventTrack,
}

impl AnimationClip {
    pub fn new(name: impl Into<String>, duration: f32) -> Self {
        Self {
            name:        name.into(),
            duration,
            rot_tracks:  HashMap::new(),
            pos_tracks:  HashMap::new(),
            scale_tracks: HashMap::new(),
            root_motion: None,
            event_track: EventTrack::new(),
        }
    }

    pub fn rot_track(&mut self, bone: impl Into<String>) -> &mut QuatTrack {
        self.rot_tracks.entry(bone.into()).or_insert_with(QuatTrack::new)
    }

    pub fn pos_track(&mut self, bone: impl Into<String>) -> &mut Vec3Track {
        self.pos_tracks.entry(bone.into()).or_insert_with(Vec3Track::new)
    }

    pub fn scale_track(&mut self, bone: impl Into<String>) -> &mut FloatTrack {
        self.scale_tracks.entry(bone.into()).or_insert_with(FloatTrack::new)
    }

    /// Sample a `BoneTransform` for `bone` at time `t`.
    pub fn sample_bone(&self, bone: &str, t: f32) -> BoneTransform {
        let rotation    = self.rot_tracks.get(bone)
            .and_then(|tr| tr.sample(t))
            .unwrap_or(Quat::IDENTITY);
        let translation = self.pos_tracks.get(bone)
            .and_then(|tr| tr.sample(t))
            .unwrap_or(Vec3::ZERO);
        let scale_f     = self.scale_tracks.get(bone)
            .and_then(|tr| tr.sample(t))
            .unwrap_or(1.0);
        let scale = Vec3::new(scale_f, scale_f, scale_f);
        BoneTransform { translation, rotation, scale }
    }

    /// All bone names that have at least one track.
    pub fn bone_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.rot_tracks.keys()
            .chain(self.pos_tracks.keys())
            .map(|s| s.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Sample all bones and return a pose map.
    pub fn sample_pose(&self, t: f32) -> HashMap<String, BoneTransform> {
        let mut pose = HashMap::new();
        for name in self.bone_names() {
            pose.insert(name.to_string(), self.sample_bone(name, t));
        }
        pose
    }

    /// Root motion delta for a time range.
    pub fn root_motion_delta(&self, from: f32, to: f32) -> Vec3 {
        if let Some(ref rm) = self.root_motion {
            let a = rm.sample(from).unwrap_or(Vec3::ZERO);
            let b = rm.sample(to).unwrap_or(Vec3::ZERO);
            b - a
        } else {
            Vec3::ZERO
        }
    }
}

// ── BlendEntry ────────────────────────────────────────────────────────────────

/// An entry in a `BlendTree` — a clip with a normalized weight.
#[derive(Debug, Clone)]
pub struct BlendEntry {
    pub clip:   AnimationClip,
    pub weight: f32,
    pub time:   f32,
    pub speed:  f32,
}

impl BlendEntry {
    pub fn new(clip: AnimationClip) -> Self {
        Self { clip, weight: 1.0, time: 0.0, speed: 1.0 }
    }

    pub fn with_weight(mut self, w: f32) -> Self { self.weight = w.max(0.0); self }
    pub fn with_speed(mut self, s: f32) -> Self { self.speed = s; self }
}

// ── BlendTree ─────────────────────────────────────────────────────────────────

/// Blends multiple `AnimationClip`s by weight.
///
/// Weights are normalised automatically so they sum to 1.
/// Uses additive blend: `a + t*(b-a)` chained across clips.
pub struct BlendTree {
    entries: Vec<BlendEntry>,
}

impl BlendTree {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add(mut self, entry: BlendEntry) -> Self {
        self.entries.push(entry);
        self
    }

    pub fn add_clip(mut self, clip: AnimationClip, weight: f32) -> Self {
        self.entries.push(BlendEntry::new(clip).with_weight(weight));
        self
    }

    /// Advance all clip timers.
    pub fn tick(&mut self, dt: f32) {
        for entry in &mut self.entries {
            let dur = entry.clip.duration.max(f32::EPSILON);
            entry.time = (entry.time + dt * entry.speed) % dur;
        }
    }

    pub fn set_weight(&mut self, idx: usize, weight: f32) {
        if let Some(e) = self.entries.get_mut(idx) {
            e.weight = weight.max(0.0);
        }
    }

    pub fn entry_count(&self) -> usize { self.entries.len() }

    /// Evaluate the blended pose for all bones at current time.
    pub fn evaluate(&self) -> HashMap<String, BoneTransform> {
        if self.entries.is_empty() { return HashMap::new(); }

        // Collect total weight
        let total_weight: f32 = self.entries.iter().map(|e| e.weight).sum();
        if total_weight < f32::EPSILON { return HashMap::new(); }

        // Collect all bone names across all clips
        let mut all_bones: Vec<String> = self.entries.iter()
            .flat_map(|e| e.clip.bone_names().into_iter().map(|s| s.to_string()))
            .collect();
        all_bones.sort_unstable();
        all_bones.dedup();

        let mut result: HashMap<String, BoneTransform> = HashMap::new();

        for bone in &all_bones {
            // Weighted accumulation — start from identity, blend each entry in
            // using its normalised weight
            let mut accumulated = BoneTransform::identity();
            let mut accumulated_weight = 0.0f32;

            for entry in &self.entries {
                if entry.weight < f32::EPSILON { continue; }
                let pose  = entry.clip.sample_bone(bone, entry.time);
                let norm  = entry.weight / total_weight;
                let blend_t = if accumulated_weight + norm > f32::EPSILON {
                    norm / (accumulated_weight + norm)
                } else {
                    0.0
                };
                accumulated = BoneTransform::blend(&accumulated, &pose, blend_t);
                accumulated_weight += norm;
            }

            result.insert(bone.clone(), accumulated);
        }

        result
    }

    /// Evaluate only a specific bone.
    pub fn evaluate_bone(&self, bone: &str) -> BoneTransform {
        let total_weight: f32 = self.entries.iter().map(|e| e.weight).sum();
        if total_weight < f32::EPSILON { return BoneTransform::identity(); }

        let mut accumulated = BoneTransform::identity();
        let mut accumulated_weight = 0.0f32;

        for entry in &self.entries {
            if entry.weight < f32::EPSILON { continue; }
            let pose = entry.clip.sample_bone(bone, entry.time);
            let norm = entry.weight / total_weight;
            let blend_t = if accumulated_weight + norm > f32::EPSILON {
                norm / (accumulated_weight + norm)
            } else {
                0.0
            };
            accumulated = BoneTransform::blend(&accumulated, &pose, blend_t);
            accumulated_weight += norm;
        }
        accumulated
    }
}

impl Default for BlendTree {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Interpolation ─────────────────────────────────────────────────────────

    #[test]
    fn interpolation_linear() {
        let i = Interpolation::Linear;
        assert!((i.apply(0.0) - 0.0).abs() < 1e-5);
        assert!((i.apply(0.5) - 0.5).abs() < 1e-5);
        assert!((i.apply(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn interpolation_constant_returns_zero() {
        let i = Interpolation::Constant;
        assert_eq!(i.apply(0.9), 0.0);
    }

    #[test]
    fn interpolation_cubic_smooth() {
        let i = Interpolation::Cubic;
        let mid = i.apply(0.5);
        // Smooth-step at 0.5 should be 0.5
        assert!((mid - 0.5).abs() < 1e-5);
    }

    #[test]
    fn interpolation_bezier_endpoints() {
        let i = Interpolation::Bezier { cp1: 0.4, cp2: 0.6 };
        assert!((i.apply(0.0) - 0.0).abs() < 0.01);
        assert!((i.apply(1.0) - 1.0).abs() < 0.01);
    }

    // ── FloatTrack ────────────────────────────────────────────────────────────

    #[test]
    fn float_track_sample_between_keys() {
        let mut track = FloatTrack::new();
        track.key(0.0, 0.0);
        track.key(1.0, 1.0);
        let v = track.sample(0.5).unwrap();
        assert!((v - 0.5).abs() < 0.01, "v={}", v);
    }

    #[test]
    fn float_track_clamps_before_first() {
        let mut track = FloatTrack::new();
        track.key(1.0, 42.0);
        let v = track.sample(0.0).unwrap();
        assert!((v - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn float_track_clamps_after_last() {
        let mut track = FloatTrack::new();
        track.key(0.0, 10.0);
        track.key(2.0, 20.0);
        let v = track.sample(5.0).unwrap();
        assert!((v - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn float_track_constant_no_blend() {
        let mut track = FloatTrack::new();
        track.insert(Keyframe::constant(0.0, 5.0f32));
        track.insert(Keyframe::constant(1.0, 10.0f32));
        let v = track.sample(0.5).unwrap();
        assert!((v - 5.0).abs() < f32::EPSILON, "expected 5.0 (constant), got {}", v);
    }

    #[test]
    fn float_track_duration() {
        let mut track = FloatTrack::new();
        track.key(0.0, 0.0);
        track.key(3.0, 1.0);
        assert!((track.duration() - 3.0).abs() < f32::EPSILON);
    }

    // ── Vec3Track ─────────────────────────────────────────────────────────────

    #[test]
    fn vec3_track_interpolates() {
        let mut track = Vec3Track::new();
        track.key(0.0, Vec3::ZERO);
        track.key(2.0, Vec3::new(10.0, 0.0, 0.0));
        let v = track.sample(1.0).unwrap();
        assert!((v.x - 5.0).abs() < 0.1, "x={}", v.x);
    }

    // ── QuatTrack ─────────────────────────────────────────────────────────────

    #[test]
    fn quat_track_slerps() {
        let mut track = QuatTrack::new();
        let q0 = Quat::IDENTITY;
        let q1 = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        track.key(0.0, q0);
        track.key(1.0, q1);
        let mid = track.sample(0.5).unwrap();
        let angle = mid.angle_between(q0);
        // should be roughly 45 degrees
        assert!(angle > 0.3 && angle < 1.0, "angle={}", angle);
    }

    // ── ColorTrack ────────────────────────────────────────────────────────────

    #[test]
    fn color_track_blends() {
        let mut track = ColorTrack::new();
        track.key(0.0, [0.0, 0.0, 0.0, 1.0]);
        track.key(1.0, [1.0, 1.0, 1.0, 1.0]);
        let c = track.sample(0.5).unwrap();
        assert!((c[0] - 0.5).abs() < 0.01);
    }

    // ── EventTrack ────────────────────────────────────────────────────────────

    #[test]
    fn event_track_fires_in_range() {
        let mut track = EventTrack::new();
        track.at(0.5, "start");
        track.at(1.0, "mid");
        track.at(2.0, "end");
        let events = track.events_in_range(0.0, 1.0);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, "start");
        assert_eq!(events[1].id, "mid");
    }

    #[test]
    fn event_track_empty_range() {
        let mut track = EventTrack::new();
        track.at(5.0, "late");
        let events = track.events_in_range(0.0, 3.0);
        assert!(events.is_empty());
    }

    // ── SequencePlayer ────────────────────────────────────────────────────────

    #[test]
    fn sequence_player_plays_and_finishes() {
        let mut seq = Sequence::new("test", 1.0);
        seq.float_track("alpha").key(0.0, 0.0);
        seq.float_track("alpha").key(1.0, 1.0);

        let mut player = SequencePlayer::new(seq);
        player.play();
        player.tick(0.5);
        assert!(player.is_playing());
        player.tick(0.6);
        assert!(player.is_finished());
    }

    #[test]
    fn sequence_player_loop_mode() {
        let seq = Sequence::new("loop", 1.0);
        let mut player = SequencePlayer::new(seq)
            .with_play_mode(SequencePlayMode::Loop);
        player.play();
        player.tick(1.5);
        assert!(player.is_playing());
        assert!(player.loop_count >= 1);
        assert!(player.time < 1.0, "time should have wrapped: {}", player.time);
    }

    #[test]
    fn sequence_player_ping_pong() {
        let seq = Sequence::new("pp", 1.0);
        let mut player = SequencePlayer::new(seq)
            .with_play_mode(SequencePlayMode::PingPong);
        player.play();
        player.tick(1.2); // goes past end, should reverse
        assert!(player.reversing, "should be reversing after first ping");
    }

    #[test]
    fn sequence_player_events_fire() {
        let mut seq = Sequence::new("ev", 2.0);
        seq.event_track.at(0.5, "halfway");
        let mut player = SequencePlayer::new(seq);
        player.play();
        let events = player.tick(0.7);
        assert!(!events.is_empty());
        assert_eq!(events[0].id, "halfway");
    }

    #[test]
    fn sequence_player_progress() {
        let seq = Sequence::new("prog", 4.0);
        let mut player = SequencePlayer::new(seq);
        player.play();
        player.tick(2.0);
        assert!((player.progress() - 0.5).abs() < 0.01);
    }

    // ── AnimationClip ─────────────────────────────────────────────────────────

    #[test]
    fn animation_clip_sample_bone() {
        let mut clip = AnimationClip::new("walk", 1.0);
        clip.rot_track("spine").key(0.0, Quat::IDENTITY);
        clip.rot_track("spine").key(1.0, Quat::from_rotation_y(1.0));
        let bone = clip.sample_bone("spine", 0.5);
        assert!(bone.rotation.angle_between(Quat::IDENTITY) > 0.0);
    }

    #[test]
    fn animation_clip_bone_names() {
        let mut clip = AnimationClip::new("run", 1.0);
        clip.rot_track("hip").key(0.0, Quat::IDENTITY);
        clip.pos_track("foot_l").key(0.0, Vec3::ZERO);
        clip.pos_track("foot_r").key(0.0, Vec3::ZERO);
        let names = clip.bone_names();
        assert!(names.contains(&"hip"));
        assert!(names.contains(&"foot_l"));
        assert!(names.contains(&"foot_r"));
    }

    // ── BlendTree ─────────────────────────────────────────────────────────────

    #[test]
    fn blend_tree_single_clip() {
        let mut clip = AnimationClip::new("idle", 1.0);
        clip.pos_track("head").key(0.0, Vec3::new(0.0, 2.0, 0.0));
        clip.pos_track("head").key(1.0, Vec3::new(0.0, 2.0, 0.0));

        let tree = BlendTree::new().add_clip(clip, 1.0);
        let pose = tree.evaluate();
        let head = pose.get("head").unwrap();
        assert!((head.translation.y - 2.0).abs() < 0.01);
    }

    #[test]
    fn blend_tree_two_clips_50_50() {
        let mut clip_a = AnimationClip::new("a", 1.0);
        clip_a.pos_track("bone").key(0.0, Vec3::new(0.0, 0.0, 0.0));
        clip_a.pos_track("bone").key(1.0, Vec3::new(0.0, 0.0, 0.0));

        let mut clip_b = AnimationClip::new("b", 1.0);
        clip_b.pos_track("bone").key(0.0, Vec3::new(10.0, 0.0, 0.0));
        clip_b.pos_track("bone").key(1.0, Vec3::new(10.0, 0.0, 0.0));

        let tree = BlendTree::new()
            .add_clip(clip_a, 1.0)
            .add_clip(clip_b, 1.0);

        let bone = tree.evaluate_bone("bone");
        assert!((bone.translation.x - 5.0).abs() < 0.5, "x={}", bone.translation.x);
    }

    #[test]
    fn bone_transform_blend_halfway() {
        let a = BoneTransform { translation: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE };
        let b = BoneTransform { translation: Vec3::new(10.0, 0.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::ONE };
        let mid = BoneTransform::blend(&a, &b, 0.5);
        assert!((mid.translation.x - 5.0).abs() < 0.01);
    }

    #[test]
    fn timeline_marker_sorted_insert() {
        let mut seq = Sequence::new("markers", 10.0);
        seq.add_marker(5.0, "mid");
        seq.add_marker(2.0, "early");
        seq.add_marker(8.0, "late");
        assert_eq!(seq.markers[0].label, "early");
        assert_eq!(seq.markers[1].label, "mid");
        assert_eq!(seq.markers[2].label, "late");
    }
}
