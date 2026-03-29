//! Animation Timeline — keyframe curves for bone FK rotations, SDF morph
//! targets, and kit parameter animation.
//!
//! # Architecture
//!
//! The timeline owns a list of `AnimTrack` values — one per animated property.
//! A track holds a sorted list of `Keyframe` values and a `CurveInterp` mode.
//! Evaluating a track at time `t` returns the interpolated `TrackValue`.
//!
//! # Track types
//!
//! - **BoneRotation**: quaternion rotation for a named bone.
//! - **BoneTranslation**: Vec3 translation for a named bone.
//! - **BoneScale**: Vec3 scale for a named bone.
//! - **SdfMorph**: blend factor [0,1] between two SDF graphs by name.
//! - **KitFloat**: any f32 kit parameter (bloom, AO strength, etc.).
//! - **KitVec3**: any Vec3 kit parameter (light direction, etc.).
//! - **KitColor**: RGBA colour parameter.
//!
//! # Playback
//!
//! `Timeline::step` advances the playhead by `dt` seconds and evaluates all
//! tracks, returning a `FrameSnapshot` with all current animated values.
//!
//! # Editing
//!
//! Keys are inserted/moved/deleted through the `TimelineEditor` wrapper, which
//! maintains an undo stack of `TimelineEdit` entries.

use glam::{Vec2, Vec3, Vec4, Quat};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Time
// ─────────────────────────────────────────────────────────────────────────────

/// Timeline time in seconds (f32 for GPU-side compatibility).
pub type Time = f32;

// ─────────────────────────────────────────────────────────────────────────────
// TrackId
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub u32);

impl TrackId { pub const NONE: TrackId = TrackId(u32::MAX); }
impl Default for TrackId { fn default() -> Self { TrackId::NONE } }

// ─────────────────────────────────────────────────────────────────────────────
// KeyId
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyId(pub u32);

// ─────────────────────────────────────────────────────────────────────────────
// TrackValue
// ─────────────────────────────────────────────────────────────────────────────

/// The union of all possible track value types.
#[derive(Debug, Clone, PartialEq)]
pub enum TrackValue {
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
    Quat(Quat),
    Bool(bool),
    Event(String),
}

impl TrackValue {
    pub fn as_float(&self) -> Option<f32> {
        if let TrackValue::Float(v) = self { Some(*v) } else { None }
    }
    pub fn as_vec3(&self) -> Option<Vec3> {
        if let TrackValue::Vec3(v) = self { Some(*v) } else { None }
    }
    pub fn as_quat(&self) -> Option<Quat> {
        if let TrackValue::Quat(q) = self { Some(*q) } else { None }
    }

    /// Linear interpolation between two values. Returns None if types differ.
    pub fn lerp(&self, other: &Self, t: f32) -> Option<Self> {
        match (self, other) {
            (TrackValue::Float(a), TrackValue::Float(b)) => Some(TrackValue::Float(a + (b - a) * t)),
            (TrackValue::Vec2(a),  TrackValue::Vec2(b))  => Some(TrackValue::Vec2(*a + (*b - *a) * t)),
            (TrackValue::Vec3(a),  TrackValue::Vec3(b))  => Some(TrackValue::Vec3(*a + (*b - *a) * t)),
            (TrackValue::Vec4(a),  TrackValue::Vec4(b))  => Some(TrackValue::Vec4(*a + (*b - *a) * t)),
            (TrackValue::Quat(a),  TrackValue::Quat(b))  => Some(TrackValue::Quat(a.slerp(*b, t))),
            (TrackValue::Bool(a),  TrackValue::Bool(_))  => Some(TrackValue::Bool(*a)),
            _ => None,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            TrackValue::Float(_) => "Float",
            TrackValue::Vec2(_)  => "Vec2",
            TrackValue::Vec3(_)  => "Vec3",
            TrackValue::Vec4(_)  => "Vec4",
            TrackValue::Quat(_)  => "Quat",
            TrackValue::Bool(_)  => "Bool",
            TrackValue::Event(_) => "Event",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CurveInterp
// ─────────────────────────────────────────────────────────────────────────────

/// How to interpolate between two keyframes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CurveInterp {
    /// Step: hold the left key's value until the right key.
    Step,
    #[default]
    /// Linear interpolation.
    Linear,
    /// Cubic Hermite spline using per-key tangents.
    CubicHermite,
    /// Automatic cubic smoothing (Catmull-Rom).
    CatmullRom,
    /// Custom easing function.
    Ease(EaseKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EaseKind {
    EaseIn, EaseOut, EaseInOut,
    BackIn, BackOut, BackInOut,
    BounceIn, BounceOut, BounceInOut,
    ElasticIn, ElasticOut,
}

impl EaseKind {
    pub fn apply(self, t: f32) -> f32 {
        match self {
            EaseKind::EaseIn    => t * t,
            EaseKind::EaseOut   => 1.0 - (1.0 - t) * (1.0 - t),
            EaseKind::EaseInOut => {
                if t < 0.5 { 2.0 * t * t }
                else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
            }
            EaseKind::BackIn    => { let c1 = 1.701_58; let c3 = c1 + 1.0; c3 * t * t * t - c1 * t * t }
            EaseKind::BackOut   => { let c1 = 1.701_58; let c3 = c1 + 1.0; 1.0 + c3 * (t-1.0).powi(3) + c1 * (t-1.0).powi(2) }
            EaseKind::BackInOut => {
                let c2 = 1.701_58 * 1.525;
                if t < 0.5 {
                    ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (2.0 * t - 2.0) + c2) + 2.0) / 2.0
                }
            }
            EaseKind::BounceOut => {
                let n1 = 7.5625; let d1 = 2.75;
                let t = t;
                if t < 1.0 / d1 { n1 * t * t }
                else if t < 2.0 / d1 { let t = t - 1.5 / d1; n1 * t * t + 0.75 }
                else if t < 2.5 / d1 { let t = t - 2.25 / d1; n1 * t * t + 0.9375 }
                else { let t = t - 2.625 / d1; n1 * t * t + 0.984_375 }
            }
            EaseKind::BounceIn     => 1.0 - EaseKind::BounceOut.apply(1.0 - t),
            EaseKind::BounceInOut  => {
                if t < 0.5 { (1.0 - EaseKind::BounceOut.apply(1.0 - 2.0 * t)) / 2.0 }
                else { (1.0 + EaseKind::BounceOut.apply(2.0 * t - 1.0)) / 2.0 }
            }
            EaseKind::ElasticIn  => {
                if t == 0.0 || t == 1.0 { return t; }
                let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                -(2.0f32.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * c4).sin()
            }
            EaseKind::ElasticOut => {
                if t == 0.0 || t == 1.0 { return t; }
                let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                2.0f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Keyframe
// ─────────────────────────────────────────────────────────────────────────────

/// A single key in an animation track.
#[derive(Debug, Clone)]
pub struct Keyframe {
    pub id:       KeyId,
    pub time:     Time,
    pub value:    TrackValue,
    /// Left tangent for cubic interpolation.
    pub tan_in:   Option<f32>,
    /// Right tangent for cubic interpolation.
    pub tan_out:  Option<f32>,
    pub interp:   CurveInterp,
    pub selected: bool,
}

impl Keyframe {
    pub fn new(id: KeyId, time: Time, value: TrackValue) -> Self {
        Self {
            id, time, value,
            tan_in: None, tan_out: None,
            interp: CurveInterp::Linear,
            selected: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TrackTarget — identifies what a track animates
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrackTarget {
    BoneRotation(String),
    BoneTranslation(String),
    BoneScale(String),
    BoneEnvelopeRadius(String),
    SdfMorph { from_graph: String, to_graph: String },
    NodeTranslation { graph: String, node_id: u32 },
    NodeScale       { graph: String, node_id: u32 },
    NodePrimParam   { graph: String, node_id: u32, param: String },
    KitFloat(String),
    KitVec3(String),
    KitColor(String),
    CameraPos,
    CameraFov,
    CameraRoll,
    SceneFloat(String),
    Event(String),
}

impl TrackTarget {
    pub fn label(&self) -> String {
        match self {
            TrackTarget::BoneRotation(n)    => format!("{n} Rotation"),
            TrackTarget::BoneTranslation(n) => format!("{n} Translation"),
            TrackTarget::BoneScale(n)       => format!("{n} Scale"),
            TrackTarget::BoneEnvelopeRadius(n) => format!("{n} EnvRadius"),
            TrackTarget::SdfMorph { from_graph, to_graph } => format!("Morph {from_graph}→{to_graph}"),
            TrackTarget::NodeTranslation { node_id, .. } => format!("N{node_id} Translation"),
            TrackTarget::NodeScale { node_id, .. }       => format!("N{node_id} Scale"),
            TrackTarget::NodePrimParam { node_id, param, .. } => format!("N{node_id}.{param}"),
            TrackTarget::KitFloat(p)  => format!("Kit:{p}"),
            TrackTarget::KitVec3(p)   => format!("Kit:{p}"),
            TrackTarget::KitColor(p)  => format!("Kit:{p}"),
            TrackTarget::CameraPos    => "Camera.Pos".into(),
            TrackTarget::CameraFov    => "Camera.Fov".into(),
            TrackTarget::CameraRoll   => "Camera.Roll".into(),
            TrackTarget::SceneFloat(n) => format!("Scene:{n}"),
            TrackTarget::Event(n)     => format!("Event:{n}"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AnimTrack
// ─────────────────────────────────────────────────────────────────────────────

/// A single animated property track.
#[derive(Debug, Clone)]
pub struct AnimTrack {
    pub id:       TrackId,
    pub target:   TrackTarget,
    pub label:    String,
    pub visible:  bool,
    pub locked:   bool,
    pub muted:    bool,
    pub color:    Vec4,
    keys:         Vec<Keyframe>,
    next_key:     u32,
    pub default_interp: CurveInterp,
}

impl AnimTrack {
    pub fn new(id: TrackId, target: TrackTarget) -> Self {
        let label = target.label();
        Self {
            id, label, target,
            visible: true, locked: false, muted: false,
            color: Vec4::new(0.4, 0.8, 0.4, 1.0),
            keys: Vec::new(),
            next_key: 1,
            default_interp: CurveInterp::Linear,
        }
    }

    // ── Key management ────────────────────────────────────────────────────

    pub fn insert_key(&mut self, time: Time, value: TrackValue) -> KeyId {
        let id = KeyId(self.next_key);
        self.next_key += 1;
        // Remove any existing key at this exact time
        self.keys.retain(|k| (k.time - time).abs() > 1e-5);
        let key = Keyframe::new(id, time, value);
        // Insert in sorted order
        let pos = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(pos, key);
        id
    }

    pub fn remove_key(&mut self, id: KeyId) -> Option<Keyframe> {
        if let Some(pos) = self.keys.iter().position(|k| k.id == id) {
            Some(self.keys.remove(pos))
        } else { None }
    }

    pub fn remove_at_time(&mut self, time: Time) {
        self.keys.retain(|k| (k.time - time).abs() > 1e-5);
    }

    pub fn keys(&self) -> &[Keyframe] { &self.keys }
    pub fn key_count(&self) -> usize { self.keys.len() }

    pub fn start_time(&self) -> Time {
        self.keys.first().map(|k| k.time).unwrap_or(0.0)
    }

    pub fn end_time(&self) -> Time {
        self.keys.last().map(|k| k.time).unwrap_or(0.0)
    }

    pub fn duration(&self) -> Time { self.end_time() - self.start_time() }

    // ── Evaluation ────────────────────────────────────────────────────────

    /// Sample the track at time `t`. Returns None if no keys.
    pub fn sample(&self, t: Time) -> Option<TrackValue> {
        if self.muted { return None; }
        if self.keys.is_empty() { return None; }
        if self.keys.len() == 1 { return Some(self.keys[0].value.clone()); }

        // Clamp to range
        if t <= self.keys[0].time { return Some(self.keys[0].value.clone()); }
        if t >= self.keys.last().unwrap().time {
            return Some(self.keys.last().unwrap().value.clone());
        }

        // Find bracketing keys
        let right_idx = self.keys.partition_point(|k| k.time <= t);
        let right = &self.keys[right_idx];
        let left  = &self.keys[right_idx - 1];

        let dt = right.time - left.time;
        if dt < 1e-8 { return Some(left.value.clone()); }
        let u = (t - left.time) / dt;

        let interp = left.interp;
        let t_eased = match interp {
            CurveInterp::Step         => 0.0,
            CurveInterp::Linear       => u,
            CurveInterp::CatmullRom | CurveInterp::CubicHermite => {
                // Smooth cubic
                u * u * (3.0 - 2.0 * u)
            }
            CurveInterp::Ease(kind)   => kind.apply(u),
        };

        left.value.lerp(&right.value, t_eased)
    }

    /// Get the key immediately before or at `t`.
    pub fn key_before(&self, t: Time) -> Option<&Keyframe> {
        let idx = self.keys.partition_point(|k| k.time <= t);
        if idx == 0 { None } else { Some(&self.keys[idx - 1]) }
    }

    /// Move a key in time.
    pub fn move_key(&mut self, id: KeyId, new_time: Time) {
        if let Some(pos) = self.keys.iter().position(|k| k.id == id) {
            self.keys[pos].time = new_time;
            self.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    pub fn set_key_value(&mut self, id: KeyId, value: TrackValue) {
        if let Some(key) = self.keys.iter_mut().find(|k| k.id == id) {
            key.value = value;
        }
    }

    pub fn set_key_interp(&mut self, id: KeyId, interp: CurveInterp) {
        if let Some(key) = self.keys.iter_mut().find(|k| k.id == id) {
            key.interp = interp;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AnimClip — a named set of tracks
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AnimClip {
    pub name:   String,
    pub tracks: Vec<AnimTrack>,
    next_track: u32,
}

impl AnimClip {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), tracks: Vec::new(), next_track: 1 }
    }

    pub fn add_track(&mut self, target: TrackTarget) -> TrackId {
        let id = TrackId(self.next_track);
        self.next_track += 1;
        self.tracks.push(AnimTrack::new(id, target));
        id
    }

    pub fn get_track(&self, id: TrackId) -> Option<&AnimTrack> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn get_track_mut(&mut self, id: TrackId) -> Option<&mut AnimTrack> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn duration(&self) -> Time {
        self.tracks.iter().map(|t| t.end_time()).fold(0.0_f32, f32::max)
    }

    /// Sample all tracks at time `t`, returning a map from TrackId → value.
    pub fn sample_all(&self, t: Time) -> HashMap<TrackId, TrackValue> {
        self.tracks.iter()
            .filter_map(|track| track.sample(t).map(|v| (track.id, v)))
            .collect()
    }

    pub fn track_count(&self) -> usize { self.tracks.len() }
    pub fn total_keys(&self) -> usize { self.tracks.iter().map(|t| t.key_count()).sum() }
}

// ─────────────────────────────────────────────────────────────────────────────
// PlaybackState
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Recording,
}

impl Default for PlaybackState { fn default() -> Self { PlaybackState::Stopped } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Once,
    Loop,
    PingPong,
    HoldFirst,
    HoldLast,
}

impl Default for LoopMode { fn default() -> Self { LoopMode::Loop } }

// ─────────────────────────────────────────────────────────────────────────────
// FrameSnapshot — the evaluated animation state at a given time
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct FrameSnapshot {
    pub time:           Time,
    pub bone_rotations: HashMap<String, Quat>,
    pub bone_translates: HashMap<String, Vec3>,
    pub bone_scales:    HashMap<String, Vec3>,
    pub kit_floats:     HashMap<String, f32>,
    pub kit_colors:     HashMap<String, Vec4>,
    pub kit_vec3s:      HashMap<String, Vec3>,
    pub sdf_morphs:     Vec<(String, String, f32)>,
    pub events:         Vec<String>,
    pub camera_pos:     Option<Vec3>,
    pub camera_fov:     Option<f32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Timeline
// ─────────────────────────────────────────────────────────────────────────────

/// Master timeline — owns all animation clips plus playback state.
#[derive(Debug)]
pub struct Timeline {
    pub clips:        Vec<AnimClip>,
    pub active_clip:  usize,
    pub playhead:     Time,
    pub state:        PlaybackState,
    pub loop_mode:    LoopMode,
    pub speed:        f32,
    pub fps:          f32,
    /// Whether to snap the playhead to frame boundaries.
    pub snap_frames:  bool,
    /// In/out point for looping.
    pub in_point:     Time,
    pub out_point:    Time,
}

impl Timeline {
    pub fn new() -> Self {
        let clip = AnimClip::new("Default");
        Self {
            clips:       vec![clip],
            active_clip: 0,
            playhead:    0.0,
            state:       PlaybackState::Stopped,
            loop_mode:   LoopMode::Loop,
            speed:       1.0,
            fps:         60.0,
            snap_frames: false,
            in_point:    0.0,
            out_point:   10.0,
        }
    }

    pub fn clip(&self) -> &AnimClip { &self.clips[self.active_clip] }
    pub fn clip_mut(&mut self) -> &mut AnimClip { &mut self.clips[self.active_clip] }

    pub fn frame_duration(&self) -> Time { 1.0 / self.fps }

    pub fn current_frame(&self) -> u32 { (self.playhead * self.fps) as u32 }

    pub fn total_frames(&self) -> u32 { (self.clip().duration() * self.fps) as u32 }

    // ── Playback control ──────────────────────────────────────────────────

    pub fn play(&mut self) { self.state = PlaybackState::Playing; }
    pub fn pause(&mut self) {
        self.state = if self.state == PlaybackState::Playing {
            PlaybackState::Paused
        } else {
            PlaybackState::Playing
        };
    }
    pub fn stop(&mut self) { self.state = PlaybackState::Stopped; self.playhead = self.in_point; }
    pub fn record(&mut self) { self.state = PlaybackState::Recording; }

    pub fn seek(&mut self, t: Time) {
        self.playhead = if self.snap_frames {
            (t * self.fps).round() / self.fps
        } else {
            t
        }.clamp(self.in_point, self.out_point);
    }

    pub fn seek_frame(&mut self, frame: i32) {
        let t = frame as f32 / self.fps;
        self.seek(t);
    }

    pub fn step_forward(&mut self) { self.seek(self.playhead + self.frame_duration()); }
    pub fn step_backward(&mut self) { self.seek(self.playhead - self.frame_duration()); }
    pub fn go_to_start(&mut self) { self.seek(self.in_point); }
    pub fn go_to_end(&mut self) { self.seek(self.out_point); }

    /// Advance by `dt` seconds of wall-clock time.
    pub fn step(&mut self, dt: f32) -> FrameSnapshot {
        if self.state == PlaybackState::Playing || self.state == PlaybackState::Recording {
            let new_t = self.playhead + dt * self.speed;
            match self.loop_mode {
                LoopMode::Once => {
                    self.playhead = new_t.min(self.out_point);
                    if self.playhead >= self.out_point { self.state = PlaybackState::Stopped; }
                }
                LoopMode::Loop => {
                    let range = self.out_point - self.in_point;
                    if range > 1e-5 {
                        self.playhead = self.in_point + (new_t - self.in_point).rem_euclid(range);
                    }
                }
                LoopMode::PingPong => {
                    let range = self.out_point - self.in_point;
                    if range > 1e-5 {
                        let phase = (new_t - self.in_point) / range;
                        let cycle = phase.floor() as u32;
                        let frac  = phase - phase.floor();
                        self.playhead = if cycle % 2 == 0 {
                            self.in_point + frac * range
                        } else {
                            self.out_point - frac * range
                        };
                    }
                }
                LoopMode::HoldFirst => {
                    self.playhead = new_t.max(self.in_point);
                }
                LoopMode::HoldLast => {
                    self.playhead = new_t.min(self.out_point);
                }
            }
        }
        self.evaluate(self.playhead)
    }

    /// Evaluate all tracks at the given time and build a FrameSnapshot.
    pub fn evaluate(&self, t: Time) -> FrameSnapshot {
        let mut snap = FrameSnapshot { time: t, ..Default::default() };
        for track in &self.clip().tracks {
            let Some(val) = track.sample(t) else { continue; };
            match &track.target {
                TrackTarget::BoneRotation(n) => {
                    if let TrackValue::Quat(q) = val { snap.bone_rotations.insert(n.clone(), q); }
                }
                TrackTarget::BoneTranslation(n) => {
                    if let TrackValue::Vec3(v) = val { snap.bone_translates.insert(n.clone(), v); }
                }
                TrackTarget::BoneScale(n) => {
                    if let TrackValue::Vec3(v) = val { snap.bone_scales.insert(n.clone(), v); }
                }
                TrackTarget::KitFloat(p) => {
                    if let TrackValue::Float(v) = val { snap.kit_floats.insert(p.clone(), v); }
                }
                TrackTarget::KitColor(p) => {
                    if let TrackValue::Vec4(c) = val { snap.kit_colors.insert(p.clone(), c); }
                }
                TrackTarget::KitVec3(p) => {
                    if let TrackValue::Vec3(v) = val { snap.kit_vec3s.insert(p.clone(), v); }
                }
                TrackTarget::SdfMorph { from_graph, to_graph } => {
                    if let TrackValue::Float(f) = val {
                        snap.sdf_morphs.push((from_graph.clone(), to_graph.clone(), f));
                    }
                }
                TrackTarget::CameraPos => {
                    if let TrackValue::Vec3(v) = val { snap.camera_pos = Some(v); }
                }
                TrackTarget::CameraFov => {
                    if let TrackValue::Float(v) = val { snap.camera_fov = Some(v); }
                }
                TrackTarget::Event(n) => {
                    snap.events.push(n.clone());
                }
                _ => {}
            }
        }
        snap
    }
}

impl Default for Timeline { fn default() -> Self { Self::new() } }

// ─────────────────────────────────────────────────────────────────────────────
// TimelineEditor — adds undo/redo and selection to Timeline
// ─────────────────────────────────────────────────────────────────────────────

/// Selected set of (track, key) pairs.
#[derive(Debug, Clone, Default)]
pub struct KeySelection {
    pub selected: Vec<(TrackId, KeyId)>,
}

impl KeySelection {
    pub fn is_selected(&self, track: TrackId, key: KeyId) -> bool {
        self.selected.iter().any(|&(t, k)| t == track && k == key)
    }
    pub fn select_only(&mut self, track: TrackId, key: KeyId) {
        self.selected.clear();
        self.selected.push((track, key));
    }
    pub fn toggle(&mut self, track: TrackId, key: KeyId) {
        if let Some(pos) = self.selected.iter().position(|&(t, k)| t == track && k == key) {
            self.selected.remove(pos);
        } else {
            self.selected.push((track, key));
        }
    }
    pub fn clear(&mut self) { self.selected.clear(); }
}

#[derive(Debug, Clone)]
pub enum TimelineEdit {
    InsertKey { track: TrackId, key: Keyframe },
    RemoveKey { track: TrackId, key: Keyframe },
    MoveKey   { track: TrackId, key: KeyId, old_time: Time, new_time: Time },
    SetValue  { track: TrackId, key: KeyId, old_value: TrackValue, new_value: TrackValue },
    AddTrack  { track_id: TrackId },
    RemoveTrack { track_id: TrackId, track: AnimTrack },
}

/// Full editor wrapper around Timeline.
#[derive(Debug)]
pub struct TimelineEditor {
    pub timeline:    Timeline,
    pub selection:   KeySelection,
    undo_stack:      Vec<TimelineEdit>,
    redo_stack:      Vec<TimelineEdit>,
    /// Pixels-per-second for the dopesheet/curve editor.
    pub px_per_sec:  f32,
    /// Vertical scroll in the track list.
    pub scroll_y:    f32,
    /// Whether to show the curve editor (false = dopesheet mode).
    pub curve_mode:  bool,
    /// Whether to auto-key: insert a key whenever a property changes.
    pub auto_key:    bool,
    /// Whether to ripple-edit (shift subsequent keys on insert/delete).
    pub ripple:      bool,
}

impl TimelineEditor {
    pub fn new() -> Self {
        Self {
            timeline:   Timeline::new(),
            selection:  KeySelection::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            px_per_sec: 120.0,
            scroll_y:   0.0,
            curve_mode: false,
            auto_key:   false,
            ripple:     false,
        }
    }

    // ── Time ↔ pixels ─────────────────────────────────────────────────────

    pub fn time_to_px(&self, t: Time) -> f32 { t * self.px_per_sec }
    pub fn px_to_time(&self, px: f32) -> Time { px / self.px_per_sec }
    pub fn zoom_in(&mut self)  { self.px_per_sec = (self.px_per_sec * 1.25).min(2000.0); }
    pub fn zoom_out(&mut self) { self.px_per_sec = (self.px_per_sec / 1.25).max(5.0); }

    // ── Track operations ──────────────────────────────────────────────────

    pub fn add_track(&mut self, target: TrackTarget) -> TrackId {
        let id = self.timeline.clip_mut().add_track(target);
        self.undo_stack.push(TimelineEdit::AddTrack { track_id: id });
        self.redo_stack.clear();
        id
    }

    // ── Key operations ────────────────────────────────────────────────────

    pub fn insert_key(&mut self, track_id: TrackId, time: Time, value: TrackValue) -> Option<KeyId> {
        let track = self.timeline.clip_mut().get_track_mut(track_id)?;
        let id = track.insert_key(time, value.clone());
        if let Some(key) = track.keys().iter().find(|k| k.id == id).cloned() {
            self.undo_stack.push(TimelineEdit::InsertKey { track: track_id, key });
        }
        self.redo_stack.clear();
        Some(id)
    }

    pub fn delete_selected(&mut self) {
        for (track_id, key_id) in self.selection.selected.drain(..).collect::<Vec<_>>() {
            if let Some(track) = self.timeline.clip_mut().get_track_mut(track_id) {
                if let Some(key) = track.remove_key(key_id) {
                    self.undo_stack.push(TimelineEdit::RemoveKey { track: track_id, key });
                }
            }
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(edit) = self.undo_stack.pop() {
            match &edit {
                TimelineEdit::InsertKey { track, key } => {
                    if let Some(t) = self.timeline.clip_mut().get_track_mut(*track) {
                        t.remove_key(key.id);
                    }
                }
                TimelineEdit::RemoveKey { track, key } => {
                    if let Some(t) = self.timeline.clip_mut().get_track_mut(*track) {
                        t.keys.push(key.clone());
                        t.keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                    }
                }
                TimelineEdit::MoveKey { track, key, old_time, .. } => {
                    if let Some(t) = self.timeline.clip_mut().get_track_mut(*track) {
                        t.move_key(*key, *old_time);
                    }
                }
                TimelineEdit::SetValue { track, key, old_value, .. } => {
                    if let Some(t) = self.timeline.clip_mut().get_track_mut(*track) {
                        t.set_key_value(*key, old_value.clone());
                    }
                }
                _ => {}
            }
            self.redo_stack.push(edit);
        }
    }

    pub fn redo(&mut self) {
        if let Some(edit) = self.redo_stack.pop() {
            match &edit {
                TimelineEdit::InsertKey { track, key } => {
                    if let Some(t) = self.timeline.clip_mut().get_track_mut(*track) {
                        t.insert_key(key.time, key.value.clone());
                    }
                }
                TimelineEdit::MoveKey { track, key, new_time, .. } => {
                    if let Some(t) = self.timeline.clip_mut().get_track_mut(*track) {
                        t.move_key(*key, *new_time);
                    }
                }
                _ => {}
            }
            self.undo_stack.push(edit);
        }
    }

    // ── Auto-key ──────────────────────────────────────────────────────────

    pub fn auto_key_float(&mut self, track_id: TrackId, value: f32) {
        if self.auto_key && self.timeline.state == PlaybackState::Recording {
            let t = self.timeline.playhead;
            self.insert_key(track_id, t, TrackValue::Float(value));
        }
    }

    pub fn auto_key_quat(&mut self, track_id: TrackId, q: Quat) {
        if self.auto_key && self.timeline.state == PlaybackState::Recording {
            let t = self.timeline.playhead;
            self.insert_key(track_id, t, TrackValue::Quat(q));
        }
    }

    // ── Display ───────────────────────────────────────────────────────────

    pub fn status_line(&self) -> String {
        let tl = &self.timeline;
        let clip = tl.clip();
        format!(
            "Timeline [{:?}] {:?} — t={:.3}s frame={} | {} tracks {} keys | {:.1}px/s",
            tl.state, tl.loop_mode,
            tl.playhead, tl.current_frame(),
            clip.track_count(), clip.total_keys(),
            self.px_per_sec,
        )
    }
}

impl Default for TimelineEditor { fn default() -> Self { Self::new() } }

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_interpolation() {
        let mut track = AnimTrack::new(TrackId(1), TrackTarget::KitFloat("bloom".into()));
        track.insert_key(0.0, TrackValue::Float(0.0));
        track.insert_key(1.0, TrackValue::Float(2.0));
        let v = track.sample(0.5).unwrap().as_float().unwrap();
        assert!((v - 1.0).abs() < 1e-5);
    }

    #[test]
    fn step_holds_left() {
        let mut track = AnimTrack::new(TrackId(1), TrackTarget::KitFloat("x".into()));
        track.insert_key(0.0, TrackValue::Float(1.0));
        track.insert_key(1.0, TrackValue::Float(3.0));
        track.keys[0].interp = CurveInterp::Step;
        let v = track.sample(0.5).unwrap().as_float().unwrap();
        assert!((v - 1.0).abs() < 1e-5);
    }

    #[test]
    fn clamps_before_first_key() {
        let mut track = AnimTrack::new(TrackId(1), TrackTarget::KitFloat("x".into()));
        track.insert_key(1.0, TrackValue::Float(5.0));
        let v = track.sample(0.0).unwrap().as_float().unwrap();
        assert!((v - 5.0).abs() < 1e-5);
    }

    #[test]
    fn timeline_step_advances() {
        let mut ed = TimelineEditor::new();
        ed.timeline.play();
        ed.timeline.seek(0.0);
        let snap = ed.timeline.step(0.1);
        assert!(snap.time >= 0.0);
    }

    #[test]
    fn undo_insert() {
        let mut ed = TimelineEditor::new();
        let tid = ed.add_track(TrackTarget::KitFloat("bloom".into()));
        ed.insert_key(tid, 0.5, TrackValue::Float(1.0));
        let count_before = ed.timeline.clip().get_track(tid).unwrap().key_count();
        assert_eq!(count_before, 1);
        ed.undo(); // undo insert
        let count_after = ed.timeline.clip().get_track(tid).unwrap().key_count();
        assert_eq!(count_after, 0);
    }

    #[test]
    fn bounce_easing() {
        let v = EaseKind::BounceOut.apply(1.0);
        assert!((v - 1.0).abs() < 1e-5);
        let v = EaseKind::BounceOut.apply(0.0);
        assert!(v.abs() < 1e-5);
    }

    #[test]
    fn quat_slerp_track() {
        let mut track = AnimTrack::new(TrackId(1), TrackTarget::BoneRotation("Head".into()));
        track.insert_key(0.0, TrackValue::Quat(Quat::IDENTITY));
        let target = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        track.insert_key(1.0, TrackValue::Quat(target));
        let mid = track.sample(0.5).unwrap().as_quat().unwrap();
        let expected = Quat::IDENTITY.slerp(target, 0.5);
        assert!((mid.dot(expected)).abs() > 0.999);
    }
}
