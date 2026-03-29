
//! Cinematic / sequencer editor — timeline tracks, keyframes, camera cuts, director sequences.

use glam::{Vec2, Vec3, Vec4, Quat};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Time / playback
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoopMode { Hold, Loop, PingPong }

#[derive(Debug, Clone, Copy)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

impl TimeRange {
    pub fn new(start: f64, end: f64) -> Self { Self { start, end } }
    pub fn duration(&self) -> f64 { self.end - self.start }
    pub fn contains(&self, t: f64) -> bool { t >= self.start && t <= self.end }
    pub fn normalized(&self, t: f64) -> f64 { ((t - self.start) / self.duration().max(1e-10)).clamp(0.0, 1.0) }
}

// ---------------------------------------------------------------------------
// Easing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingType { Linear, SmoothStep, SmootherStep, EaseIn, EaseOut, EaseInOut, Spring, Bounce, Elastic, Back, Custom }

impl EasingType {
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingType::Linear => t,
            EasingType::SmoothStep => t * t * (3.0 - 2.0 * t),
            EasingType::SmootherStep => t * t * t * (t * (t * 6.0 - 15.0) + 10.0),
            EasingType::EaseIn => t * t,
            EasingType::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            EasingType::EaseInOut => if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 },
            EasingType::Spring => {
                let c4 = 2.0 * std::f32::consts::PI / 3.0;
                if t <= 0.0 { 0.0 } else if t >= 1.0 { 1.0 } else {
                    -(2.0_f32.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * c4).sin()
                }
            }
            EasingType::Bounce => {
                let n1 = 7.5625_f32;
                let d1 = 2.75_f32;
                let t = 1.0 - t;
                let v = if t < 1.0 / d1 { n1 * t * t }
                    else if t < 2.0 / d1 { let t = t - 1.5 / d1; n1 * t * t + 0.75 }
                    else if t < 2.5 / d1 { let t = t - 2.25 / d1; n1 * t * t + 0.9375 }
                    else { let t = t - 2.625 / d1; n1 * t * t + 0.984375 };
                1.0 - v
            }
            EasingType::Elastic => {
                let c = 2.0 * std::f32::consts::PI / 3.0;
                if t <= 0.0 { 0.0 } else if t >= 1.0 { 1.0 } else {
                    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c).sin() + 1.0
                }
            }
            EasingType::Back => {
                let c1 = 1.70158_f32;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            EasingType::Custom => t,
        }
    }
}

// ---------------------------------------------------------------------------
// Keyframe types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum KeyframeValue {
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
    Quat(Quat),
    Bool(bool),
    Int(i32),
    String(String),
    Color(Vec4),
    Event(String, Vec<String>), // event_name, args
}

#[derive(Debug, Clone)]
pub struct Keyframe {
    pub time: f64,
    pub value: KeyframeValue,
    pub in_tangent: f32,
    pub out_tangent: f32,
    pub easing: EasingType,
    pub broken_tangents: bool,
}

impl Keyframe {
    pub fn new(time: f64, value: KeyframeValue) -> Self {
        Self { time, value, in_tangent: 0.0, out_tangent: 0.0, easing: EasingType::SmoothStep, broken_tangents: false }
    }
    pub fn event(time: f64, name: impl Into<String>) -> Self {
        Self::new(time, KeyframeValue::Event(name.into(), Vec::new()))
    }
}

// ---------------------------------------------------------------------------
// Tracks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackKind {
    Transform,
    Rotation,
    Scale,
    FloatProperty,
    ColorProperty,
    BoolProperty,
    IntProperty,
    EventTrack,
    AudioTrack,
    VideoTrack,
    CameraTrack,
    SubsequenceTrack,
    ActivationTrack,
    ControlTrack,
    AnimationTrack,
    MaterialTrack,
    ParticleTrack,
    TextTrack,
    CustomPropTrack,
}

impl TrackKind {
    pub fn label(self) -> &'static str {
        match self {
            TrackKind::Transform => "Transform",
            TrackKind::Rotation => "Rotation",
            TrackKind::Scale => "Scale",
            TrackKind::FloatProperty => "Float Property",
            TrackKind::ColorProperty => "Color",
            TrackKind::BoolProperty => "Bool",
            TrackKind::IntProperty => "Int",
            TrackKind::EventTrack => "Event",
            TrackKind::AudioTrack => "Audio",
            TrackKind::VideoTrack => "Video",
            TrackKind::CameraTrack => "Camera",
            TrackKind::SubsequenceTrack => "Subsequence",
            TrackKind::ActivationTrack => "Activation",
            TrackKind::ControlTrack => "Control",
            TrackKind::AnimationTrack => "Animation",
            TrackKind::MaterialTrack => "Material",
            TrackKind::ParticleTrack => "Particle",
            TrackKind::TextTrack => "Text",
            TrackKind::CustomPropTrack => "Custom Property",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackClip {
    pub id: u32,
    pub start: f64,
    pub end: f64,
    pub clip_in: f64,      // within-asset start
    pub speed: f64,
    pub blend_in: f64,
    pub blend_out: f64,
    pub asset_id: Option<u64>,
    pub label: String,
    pub muted: bool,
    pub locked: bool,
    pub color: Vec4,
    pub loop_mode: LoopMode,
}

impl TrackClip {
    pub fn new(id: u32, start: f64, duration: f64, label: impl Into<String>) -> Self {
        Self {
            id, start, end: start + duration, clip_in: 0.0, speed: 1.0,
            blend_in: 0.0, blend_out: 0.0, asset_id: None,
            label: label.into(), muted: false, locked: false,
            color: Vec4::new(0.2, 0.5, 0.8, 1.0), loop_mode: LoopMode::Hold,
        }
    }

    pub fn duration(&self) -> f64 { self.end - self.start }

    pub fn local_time(&self, global_t: f64) -> f64 {
        let t = (global_t - self.start) * self.speed + self.clip_in;
        t.max(0.0)
    }

    pub fn blend_weight(&self, global_t: f64) -> f32 {
        if global_t < self.start || global_t > self.end { return 0.0; }
        let t = global_t - self.start;
        let dur = self.duration();
        let from_start = if self.blend_in > 0.0 { (t / self.blend_in).min(1.0) as f32 } else { 1.0 };
        let from_end = if self.blend_out > 0.0 { ((dur - t) / self.blend_out).min(1.0) as f32 } else { 1.0 };
        from_start.min(from_end)
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.start < other.end && self.end > other.start
    }
}

#[derive(Debug, Clone)]
pub struct SequenceTrack {
    pub id: u32,
    pub name: String,
    pub kind: TrackKind,
    pub entity_id: Option<u64>,
    pub property_path: String,
    pub keyframes: Vec<Keyframe>,
    pub clips: Vec<TrackClip>,
    pub muted: bool,
    pub locked: bool,
    pub solo: bool,
    pub collapsed: bool,
    pub height: f32,
    pub color: Vec4,
    pub subtracks: Vec<SequenceTrack>,
    pub infinite_clip_active: bool,
}

impl SequenceTrack {
    pub fn new(id: u32, name: impl Into<String>, kind: TrackKind) -> Self {
        Self {
            id, name: name.into(), kind, entity_id: None, property_path: String::new(),
            keyframes: Vec::new(), clips: Vec::new(),
            muted: false, locked: false, solo: false, collapsed: false,
            height: 36.0, color: Vec4::new(0.25, 0.55, 0.78, 1.0),
            subtracks: Vec::new(), infinite_clip_active: false,
        }
    }

    pub fn add_keyframe(&mut self, kf: Keyframe) {
        let i = self.keyframes.partition_point(|k| k.time <= kf.time);
        self.keyframes.insert(i, kf);
    }

    pub fn add_clip(&mut self, clip: TrackClip) {
        self.clips.push(clip);
        self.clips.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn remove_keyframe_at(&mut self, time: f64, tolerance: f64) {
        self.keyframes.retain(|k| (k.time - time).abs() > tolerance);
    }

    pub fn evaluate_float(&self, t: f64) -> f32 {
        if self.keyframes.is_empty() { return 0.0; }
        let i = self.keyframes.partition_point(|k| k.time <= t);
        if i == 0 {
            return match &self.keyframes[0].value { KeyframeValue::Float(v) => *v, _ => 0.0 };
        }
        if i >= self.keyframes.len() {
            let last = self.keyframes.len() - 1;
            return match &self.keyframes[last].value { KeyframeValue::Float(v) => *v, _ => 0.0 };
        }
        let a = &self.keyframes[i-1];
        let b = &self.keyframes[i];
        let u = ((t - a.time) / (b.time - a.time).max(1e-10)) as f32;
        let u = a.easing.apply(u);
        match (&a.value, &b.value) {
            (KeyframeValue::Float(va), KeyframeValue::Float(vb)) => va + (vb - va) * u,
            _ => 0.0,
        }
    }

    pub fn evaluate_vec3(&self, t: f64) -> Vec3 {
        if self.keyframes.is_empty() { return Vec3::ZERO; }
        let i = self.keyframes.partition_point(|k| k.time <= t);
        if i == 0 { return match &self.keyframes[0].value { KeyframeValue::Vec3(v) => *v, _ => Vec3::ZERO }; }
        if i >= self.keyframes.len() {
            let last = self.keyframes.len() - 1;
            return match &self.keyframes[last].value { KeyframeValue::Vec3(v) => *v, _ => Vec3::ZERO };
        }
        let a = &self.keyframes[i-1];
        let b = &self.keyframes[i];
        let u = ((t - a.time) / (b.time - a.time).max(1e-10)) as f32;
        let u = a.easing.apply(u);
        match (&a.value, &b.value) {
            (KeyframeValue::Vec3(va), KeyframeValue::Vec3(vb)) => va.lerp(*vb, u),
            _ => Vec3::ZERO,
        }
    }

    pub fn duration(&self) -> f64 {
        let kf_end = self.keyframes.last().map(|k| k.time).unwrap_or(0.0);
        let clip_end = self.clips.iter().map(|c| c.end).fold(0.0_f64, f64::max);
        kf_end.max(clip_end)
    }
}

// ---------------------------------------------------------------------------
// Camera cuts and shots
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraBlendType { Cut, Linear, EaseIn, EaseOut, EaseInOut, Custom }

#[derive(Debug, Clone)]
pub struct CameraShot {
    pub id: u32,
    pub start: f64,
    pub duration: f64,
    pub camera_entity: u64,
    pub blend_in: CameraBlendType,
    pub blend_out: CameraBlendType,
    pub blend_duration: f64,
    pub look_at_target: Option<u64>,
    pub fov_override: Option<f32>,
}

impl CameraShot {
    pub fn cut(id: u32, start: f64, duration: f64, cam_entity: u64) -> Self {
        Self {
            id, start, duration, camera_entity: cam_entity,
            blend_in: CameraBlendType::Cut, blend_out: CameraBlendType::Cut,
            blend_duration: 0.0, look_at_target: None, fov_override: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Sequence (master timeline)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Sequence {
    pub id: u64,
    pub name: String,
    pub duration: f64,
    pub frame_rate: f64,
    pub tracks: Vec<SequenceTrack>,
    pub camera_shots: Vec<CameraShot>,
    pub markers: Vec<(f64, String)>,
    pub loop_mode: LoopMode,
    pub current_time: f64,
    pub is_playing: bool,
    pub playback_speed: f64,
    pub gravity: bool,
    pub next_track_id: u32,
    pub next_clip_id: u32,
    pub next_shot_id: u32,
}

impl Sequence {
    pub fn new(name: impl Into<String>, duration_secs: f64, fps: f64) -> Self {
        Self {
            id: 1, name: name.into(), duration: duration_secs, frame_rate: fps,
            tracks: Vec::new(), camera_shots: Vec::new(), markers: Vec::new(),
            loop_mode: LoopMode::Hold, current_time: 0.0, is_playing: false,
            playback_speed: 1.0, gravity: false,
            next_track_id: 1, next_clip_id: 1, next_shot_id: 1,
        }
    }

    pub fn frame_count(&self) -> u64 { (self.duration * self.frame_rate) as u64 }
    pub fn frame_to_time(&self, frame: u64) -> f64 { frame as f64 / self.frame_rate }
    pub fn time_to_frame(&self, t: f64) -> u64 { (t * self.frame_rate) as u64 }

    pub fn add_track(&mut self, mut track: SequenceTrack) -> u32 {
        let id = self.next_track_id;
        track.id = id;
        self.next_track_id += 1;
        self.tracks.push(track);
        id
    }

    pub fn add_shot(&mut self, mut shot: CameraShot) -> u32 {
        let id = self.next_shot_id;
        shot.id = id;
        self.next_shot_id += 1;
        self.camera_shots.push(shot);
        self.camera_shots.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
        id
    }

    pub fn add_marker(&mut self, time: f64, label: impl Into<String>) {
        self.markers.push((time, label.into()));
        self.markers.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    pub fn update(&mut self, dt: f64) {
        if !self.is_playing { return; }
        self.current_time += dt * self.playback_speed;
        match self.loop_mode {
            LoopMode::Hold => self.current_time = self.current_time.min(self.duration),
            LoopMode::Loop => {
                if self.current_time >= self.duration { self.current_time = self.current_time % self.duration; }
            }
            LoopMode::PingPong => {
                if self.current_time >= self.duration {
                    self.playback_speed = -self.playback_speed.abs();
                } else if self.current_time <= 0.0 {
                    self.playback_speed = self.playback_speed.abs();
                }
            }
        }
    }

    pub fn play(&mut self) { self.is_playing = true; }
    pub fn pause(&mut self) { self.is_playing = false; }
    pub fn stop(&mut self) { self.is_playing = false; self.current_time = 0.0; }

    pub fn seek_to_frame(&mut self, frame: u64) {
        self.current_time = self.frame_to_time(frame);
    }

    pub fn current_camera_shot(&self) -> Option<&CameraShot> {
        self.camera_shots.iter().rev().find(|s| {
            self.current_time >= s.start && self.current_time < s.start + s.duration
        })
    }

    pub fn current_frame(&self) -> u64 { self.time_to_frame(self.current_time) }

    pub fn active_markers_at(&self, t: f64, window: f64) -> Vec<&str> {
        self.markers.iter()
            .filter(|(mt, _)| (mt - t).abs() < window)
            .map(|(_, label)| label.as_str())
            .collect()
    }

    pub fn populate_demo(&mut self) {
        // Transform track for an entity
        let mut pos_track = SequenceTrack::new(0, "Camera Position", TrackKind::Transform);
        pos_track.entity_id = Some(1);
        pos_track.add_keyframe(Keyframe::new(0.0, KeyframeValue::Vec3(Vec3::new(0.0, 2.0, -10.0))));
        pos_track.add_keyframe(Keyframe::new(3.0, KeyframeValue::Vec3(Vec3::new(5.0, 3.0, -5.0))));
        pos_track.add_keyframe(Keyframe::new(6.0, KeyframeValue::Vec3(Vec3::new(0.0, 1.0, 0.0))));
        self.add_track(pos_track);

        // Float property track (e.g., FOV)
        let mut fov_track = SequenceTrack::new(0, "Camera FOV", TrackKind::FloatProperty);
        fov_track.entity_id = Some(1);
        fov_track.property_path = "fov".into();
        fov_track.add_keyframe(Keyframe::new(0.0, KeyframeValue::Float(60.0)));
        fov_track.add_keyframe(Keyframe::new(3.0, KeyframeValue::Float(35.0)));
        fov_track.add_keyframe(Keyframe::new(6.0, KeyframeValue::Float(60.0)));
        self.add_track(fov_track);

        // Audio clip track
        let mut audio_track = SequenceTrack::new(0, "Music", TrackKind::AudioTrack);
        let music_clip = TrackClip::new(0, 0.0, 10.0, "BackgroundMusic");
        audio_track.add_clip(music_clip);
        self.add_track(audio_track);

        // Camera shots
        self.add_shot(CameraShot::cut(0, 0.0, 3.0, 1));
        self.add_shot(CameraShot::cut(0, 3.0, 3.0, 2));
        self.add_shot(CameraShot::cut(0, 6.0, 4.0, 3));

        // Markers
        self.add_marker(0.0, "Scene Start");
        self.add_marker(3.0, "Act 1");
        self.add_marker(6.0, "Act 2");
    }
}

// ---------------------------------------------------------------------------
// Cinematic editor state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CinematicEditorTool { Select, Scrub, KeyframeEdit, CutEdit, BladeEdit, Pan }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapMode { None, Frame, Bar, Custom }

#[derive(Debug, Clone)]
pub struct CinematicEditorState {
    pub sequences: Vec<Sequence>,
    pub active_sequence: usize,
    pub selected_tracks: Vec<u32>,
    pub selected_keyframes: Vec<(u32, usize)>,  // (track_id, kf_index)
    pub selected_clips: Vec<(u32, u32)>,         // (track_id, clip_id)
    pub tool: CinematicEditorTool,
    pub zoom_h: f64,    // horizontal zoom (seconds per pixel)
    pub zoom_v: f32,    // vertical zoom
    pub scroll_h: f64,
    pub scroll_v: f32,
    pub canvas_width: f32,
    pub canvas_height: f32,
    pub header_height: f32,
    pub track_header_width: f32,
    pub snap_mode: SnapMode,
    pub snap_custom_interval: f64,
    pub show_waveforms: bool,
    pub show_thumbnails: bool,
    pub show_markers: bool,
    pub show_track_groups: bool,
    pub preview_live: bool,
    pub record_mode: bool,
    pub follow_playhead: bool,
    pub search_query: String,
    pub copy_buffer: Vec<SequenceTrack>,
}

impl CinematicEditorState {
    pub fn new() -> Self {
        let mut seq = Sequence::new("MainSequence", 10.0, 30.0);
        seq.populate_demo();
        Self {
            sequences: vec![seq],
            active_sequence: 0,
            selected_tracks: Vec::new(),
            selected_keyframes: Vec::new(),
            selected_clips: Vec::new(),
            tool: CinematicEditorTool::Select,
            zoom_h: 0.01,       // 0.01 s/px = 100 px/s
            zoom_v: 1.0,
            scroll_h: 0.0,
            scroll_v: 0.0,
            canvas_width: 1200.0,
            canvas_height: 600.0,
            header_height: 40.0,
            track_header_width: 200.0,
            snap_mode: SnapMode::Frame,
            snap_custom_interval: 0.1,
            show_waveforms: true,
            show_thumbnails: true,
            show_markers: true,
            show_track_groups: true,
            preview_live: true,
            record_mode: false,
            follow_playhead: true,
            search_query: String::new(),
            copy_buffer: Vec::new(),
        }
    }

    pub fn active_sequence(&self) -> &Sequence { &self.sequences[self.active_sequence] }
    pub fn active_sequence_mut(&mut self) -> &mut Sequence { &mut self.sequences[self.active_sequence] }

    pub fn time_to_x(&self, time: f64) -> f32 {
        self.track_header_width + ((time - self.scroll_h) / self.zoom_h) as f32
    }

    pub fn x_to_time(&self, x: f32) -> f64 {
        (x - self.track_header_width) as f64 * self.zoom_h + self.scroll_h
    }

    pub fn snap_time(&self, t: f64, fps: f64) -> f64 {
        match self.snap_mode {
            SnapMode::None => t,
            SnapMode::Frame => (t * fps).round() / fps,
            SnapMode::Bar => (t * fps / 4.0).round() * 4.0 / fps,
            SnapMode::Custom => (t / self.snap_custom_interval).round() * self.snap_custom_interval,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.active_sequence_mut().update(dt as f64);
        if self.follow_playhead {
            let t = self.active_sequence().current_time;
            let x = self.time_to_x(t);
            let visible_start = self.track_header_width;
            let visible_end = self.canvas_width - 20.0;
            if x < visible_start + 50.0 || x > visible_end - 50.0 {
                self.scroll_h = (t - (self.canvas_width - self.track_header_width) as f64 * self.zoom_h * 0.4).max(0.0);
            }
        }
    }

    pub fn zoom_to_fit(&mut self) {
        let dur = self.active_sequence().duration;
        let visible_w = (self.canvas_width - self.track_header_width) as f64;
        self.zoom_h = dur / visible_w;
        self.scroll_h = 0.0;
    }

    pub fn playhead_x(&self) -> f32 {
        self.time_to_x(self.active_sequence().current_time)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing() {
        let e = EasingType::EaseInOut;
        assert!((e.apply(0.0)).abs() < 1e-5);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-5);
        assert!(e.apply(0.5) >= 0.0 && e.apply(0.5) <= 1.0);
    }

    #[test]
    fn test_track_evaluate() {
        let mut track = SequenceTrack::new(1, "test", TrackKind::FloatProperty);
        track.add_keyframe(Keyframe::new(0.0, KeyframeValue::Float(0.0)));
        track.add_keyframe(Keyframe::new(1.0, KeyframeValue::Float(10.0)));
        let v = track.evaluate_float(0.5);
        assert!(v > 0.0 && v < 10.0);
    }

    #[test]
    fn test_sequence_playback() {
        let mut seq = Sequence::new("test", 5.0, 30.0);
        seq.play();
        seq.update(2.5);
        assert!((seq.current_time - 2.5).abs() < 0.01);
        seq.update(5.0);
        assert!(seq.current_time <= seq.duration);
    }

    #[test]
    fn test_sequence_demo() {
        let mut seq = Sequence::new("test", 10.0, 30.0);
        seq.populate_demo();
        assert!(!seq.tracks.is_empty());
        assert!(!seq.camera_shots.is_empty());
    }

    #[test]
    fn test_editor() {
        let mut ed = CinematicEditorState::new();
        ed.update(0.016);
        assert!(ed.active_sequence().tracks.len() > 0);
    }
}
