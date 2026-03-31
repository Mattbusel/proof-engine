//! Timeline editor — keyframe animation, curve editing, playback control.
//!
//! Provides a visual timeline for authoring animations:
//! - Keyframe placement and editing
//! - Bezier curve editor for easing
//! - Playback controls (play, pause, step, loop, speed)
//! - Multiple tracks per property
//! - Onion skinning (ghost previous/next frames)
//! - Markers and labels
//! - Snap to frame/beat

use glam::{Vec2, Vec3, Vec4};
use proof_engine::prelude::*;
use std::collections::HashMap;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};

// ── Easing curves ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingType {
    Linear,
    EaseIn, EaseOut, EaseInOut,
    EaseInQuad, EaseOutQuad, EaseInOutQuad,
    EaseInCubic, EaseOutCubic, EaseInOutCubic,
    EaseInElastic, EaseOutElastic,
    EaseInBounce, EaseOutBounce,
    Step,
    Custom,
}

impl EasingType {
    pub fn evaluate(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 },
            Self::EaseInQuad => t * t,
            Self::EaseOutQuad => t * (2.0 - t),
            Self::EaseInOutQuad => if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t },
            Self::EaseInCubic => t * t * t,
            Self::EaseOutCubic => { let t = t - 1.0; t * t * t + 1.0 },
            Self::EaseInOutCubic => if t < 0.5 { 4.0 * t * t * t } else { (t - 1.0) * (2.0 * t - 2.0) * (2.0 * t - 2.0) + 1.0 },
            Self::EaseInElastic => {
                if t == 0.0 || t == 1.0 { return t; }
                let p = 0.3;
                -(2.0_f32.powf(10.0 * (t - 1.0)) * ((t - 1.0 - p / 4.0) * std::f32::consts::TAU / p).sin())
            }
            Self::EaseOutElastic => {
                if t == 0.0 || t == 1.0 { return t; }
                let p = 0.3;
                2.0_f32.powf(-10.0 * t) * ((t - p / 4.0) * std::f32::consts::TAU / p).sin() + 1.0
            }
            Self::EaseInBounce => 1.0 - Self::EaseOutBounce.evaluate(1.0 - t),
            Self::EaseOutBounce => {
                if t < 1.0 / 2.75 { 7.5625 * t * t }
                else if t < 2.0 / 2.75 { let t = t - 1.5 / 2.75; 7.5625 * t * t + 0.75 }
                else if t < 2.5 / 2.75 { let t = t - 2.25 / 2.75; 7.5625 * t * t + 0.9375 }
                else { let t = t - 2.625 / 2.75; 7.5625 * t * t + 0.984375 }
            }
            Self::Step => if t < 1.0 { 0.0 } else { 1.0 },
            Self::Custom => t,
        }
    }

    pub fn all() -> &'static [EasingType] {
        &[Self::Linear, Self::EaseIn, Self::EaseOut, Self::EaseInOut,
          Self::EaseInQuad, Self::EaseOutQuad, Self::EaseInOutQuad,
          Self::EaseInCubic, Self::EaseOutCubic, Self::EaseInOutCubic,
          Self::EaseInElastic, Self::EaseOutElastic,
          Self::EaseInBounce, Self::EaseOutBounce, Self::Step]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear", Self::EaseIn => "EaseIn", Self::EaseOut => "EaseOut",
            Self::EaseInOut => "EaseInOut", Self::EaseInQuad => "InQuad", Self::EaseOutQuad => "OutQuad",
            Self::EaseInOutQuad => "InOutQuad", Self::EaseInCubic => "InCubic", Self::EaseOutCubic => "OutCubic",
            Self::EaseInOutCubic => "InOutCubic", Self::EaseInElastic => "InElastic", Self::EaseOutElastic => "OutElastic",
            Self::EaseInBounce => "InBounce", Self::EaseOutBounce => "OutBounce", Self::Step => "Step",
            Self::Custom => "Custom",
        }
    }
}

// ── Keyframe ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Keyframe {
    pub time: f32,
    pub value: f32,
    pub easing: EasingType,
    pub tangent_in: f32,
    pub tangent_out: f32,
    pub selected: bool,
}

impl Keyframe {
    pub fn new(time: f32, value: f32) -> Self {
        Self { time, value, easing: EasingType::EaseInOut, tangent_in: 0.0, tangent_out: 0.0, selected: false }
    }

    pub fn with_easing(mut self, e: EasingType) -> Self { self.easing = e; self }
}

// ── Track ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackTarget {
    PositionX, PositionY, PositionZ,
    Rotation,
    ScaleX, ScaleY,
    ColorR, ColorG, ColorB, ColorA,
    Emission,
    GlowRadius,
    Custom(u32),
}

impl TrackTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::PositionX => "Pos.X", Self::PositionY => "Pos.Y", Self::PositionZ => "Pos.Z",
            Self::Rotation => "Rotation", Self::ScaleX => "Scale.X", Self::ScaleY => "Scale.Y",
            Self::ColorR => "Color.R", Self::ColorG => "Color.G", Self::ColorB => "Color.B", Self::ColorA => "Color.A",
            Self::Emission => "Emission", Self::GlowRadius => "Glow",
            Self::Custom(_) => "Custom",
        }
    }

    pub fn color(self) -> Vec4 {
        match self {
            Self::PositionX => Vec4::new(1.0, 0.3, 0.3, 1.0),
            Self::PositionY => Vec4::new(0.3, 1.0, 0.3, 1.0),
            Self::PositionZ => Vec4::new(0.3, 0.3, 1.0, 1.0),
            Self::Rotation  => Vec4::new(0.8, 0.8, 0.2, 1.0),
            Self::ColorR | Self::ColorG | Self::ColorB | Self::ColorA => Vec4::new(1.0, 0.5, 0.2, 1.0),
            Self::Emission | Self::GlowRadius => Vec4::new(0.5, 0.8, 1.0, 1.0),
            _ => Vec4::new(0.7, 0.7, 0.7, 1.0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: u32,
    pub node_id: u32,
    pub target: TrackTarget,
    pub keyframes: Vec<Keyframe>,
    pub muted: bool,
    pub locked: bool,
    pub expanded: bool,
    pub color: Vec4,
}

impl Track {
    pub fn new(id: u32, node_id: u32, target: TrackTarget) -> Self {
        Self {
            id, node_id, target,
            keyframes: Vec::new(),
            muted: false, locked: false, expanded: true,
            color: target.color(),
        }
    }

    pub fn add_keyframe(&mut self, kf: Keyframe) {
        // Insert sorted by time
        let pos = self.keyframes.iter().position(|k| k.time > kf.time).unwrap_or(self.keyframes.len());
        self.keyframes.insert(pos, kf);
    }

    pub fn remove_keyframe(&mut self, index: usize) {
        if index < self.keyframes.len() { self.keyframes.remove(index); }
    }

    /// Evaluate the track at a given time.
    pub fn evaluate(&self, time: f32) -> f32 {
        if self.keyframes.is_empty() { return 0.0; }
        if self.keyframes.len() == 1 { return self.keyframes[0].value; }

        // Find surrounding keyframes
        if time <= self.keyframes[0].time { return self.keyframes[0].value; }
        if time >= self.keyframes.last().unwrap().time { return self.keyframes.last().unwrap().value; }

        for i in 0..self.keyframes.len() - 1 {
            let a = &self.keyframes[i];
            let b = &self.keyframes[i + 1];
            if time >= a.time && time <= b.time {
                let t = (time - a.time) / (b.time - a.time);
                let eased = a.easing.evaluate(t);
                return a.value + (b.value - a.value) * eased;
            }
        }

        self.keyframes.last().unwrap().value
    }

    /// Duration (time of last keyframe).
    pub fn duration(&self) -> f32 {
        self.keyframes.last().map(|k| k.time).unwrap_or(0.0)
    }
}

// ── Marker ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TimelineMarker {
    pub time: f32,
    pub label: String,
    pub color: Vec4,
}

// ── Playback state ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState { Stopped, Playing, Paused }

#[derive(Debug, Clone)]
pub struct PlaybackControl {
    pub state: PlaybackState,
    pub current_time: f32,
    pub speed: f32,
    pub loop_enabled: bool,
    pub loop_start: f32,
    pub loop_end: f32,
    pub frame_rate: f32,
    pub snap_to_frames: bool,
}

impl Default for PlaybackControl {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stopped,
            current_time: 0.0,
            speed: 1.0,
            loop_enabled: true,
            loop_start: 0.0,
            loop_end: 10.0,
            frame_rate: 60.0,
            snap_to_frames: true,
        }
    }
}

impl PlaybackControl {
    pub fn tick(&mut self, dt: f32) {
        if self.state != PlaybackState::Playing { return; }
        self.current_time += dt * self.speed;
        if self.loop_enabled && self.current_time > self.loop_end {
            self.current_time = self.loop_start + (self.current_time - self.loop_end);
        }
        if self.snap_to_frames {
            let frame = (self.current_time * self.frame_rate).round();
            self.current_time = frame / self.frame_rate;
        }
    }

    pub fn play(&mut self) { self.state = PlaybackState::Playing; }
    pub fn pause(&mut self) { self.state = PlaybackState::Paused; }
    pub fn stop(&mut self) { self.state = PlaybackState::Stopped; self.current_time = 0.0; }
    pub fn step_forward(&mut self) { self.current_time += 1.0 / self.frame_rate; }
    pub fn step_backward(&mut self) { self.current_time = (self.current_time - 1.0 / self.frame_rate).max(0.0); }

    pub fn current_frame(&self) -> u32 { (self.current_time * self.frame_rate) as u32 }
    pub fn total_frames(&self) -> u32 { (self.loop_end * self.frame_rate) as u32 }
}

// ── Timeline ────────────────────────────────────────────────────────────────

pub struct Timeline {
    pub tracks: Vec<Track>,
    pub markers: Vec<TimelineMarker>,
    pub playback: PlaybackControl,
    pub next_track_id: u32,
    pub visible_range: (f32, f32),
    pub scroll_y: f32,
    pub selected_keyframes: Vec<(u32, usize)>,
    pub onion_skin_enabled: bool,
    pub onion_skin_count: usize,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            markers: Vec::new(),
            playback: PlaybackControl::default(),
            next_track_id: 1,
            visible_range: (0.0, 10.0),
            scroll_y: 0.0,
            selected_keyframes: Vec::new(),
            onion_skin_enabled: false,
            onion_skin_count: 3,
        }
    }

    pub fn add_track(&mut self, node_id: u32, target: TrackTarget) -> u32 {
        let id = self.next_track_id;
        self.next_track_id += 1;
        self.tracks.push(Track::new(id, node_id, target));
        id
    }

    pub fn remove_track(&mut self, track_id: u32) {
        self.tracks.retain(|t| t.id != track_id);
    }

    pub fn get_track(&self, track_id: u32) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == track_id)
    }

    pub fn get_track_mut(&mut self, track_id: u32) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == track_id)
    }

    /// Add a marker at a time.
    pub fn add_marker(&mut self, time: f32, label: &str, color: Vec4) {
        self.markers.push(TimelineMarker { time, label: label.to_string(), color });
    }

    /// Evaluate all tracks at current time and return a map of (node_id, target) → value.
    pub fn evaluate_all(&self) -> HashMap<(u32, TrackTarget), f32> {
        let mut values = HashMap::new();
        let t = self.playback.current_time;
        for track in &self.tracks {
            if !track.muted {
                values.insert((track.node_id, track.target), track.evaluate(t));
            }
        }
        values
    }

    /// Duration of the longest track.
    pub fn duration(&self) -> f32 {
        self.tracks.iter().map(|t| t.duration()).fold(0.0f32, f32::max)
    }

    /// Render the timeline panel.
    pub fn render(&self, engine: &mut ProofEngine, x: f32, y: f32, width: f32, height: f32, theme: &WidgetTheme) {
        // Background
        WidgetDraw::fill_rect(engine, Rect::new(x, y, width, height), theme.bg);

        // Playback controls
        let ctrl_y = y - 0.1;
        let state_char = match self.playback.state {
            PlaybackState::Playing => ">", PlaybackState::Paused => "=", PlaybackState::Stopped => ".",
        };
        WidgetDraw::text(engine, x + 0.3, ctrl_y, state_char, theme.accent, 0.3, RenderLayer::UI);
        WidgetDraw::text(engine, x + 1.0, ctrl_y,
            &format!("{:.2}s  F:{}", self.playback.current_time, self.playback.current_frame()),
            theme.fg, 0.1, RenderLayer::UI);
        WidgetDraw::text(engine, x + 6.0, ctrl_y,
            &format!("x{:.1}  {}fps", self.playback.speed, self.playback.frame_rate as u32),
            theme.fg_dim, 0.06, RenderLayer::UI);

        if self.playback.loop_enabled {
            WidgetDraw::text(engine, x + 11.0, ctrl_y, "LOOP", theme.warning, 0.15, RenderLayer::UI);
        }

        // Time ruler
        let ruler_y = y - 0.8;
        let (t_start, t_end) = self.visible_range;
        let time_span = t_end - t_start;
        let px_per_sec = width / time_span;
        WidgetDraw::separator(engine, x, ruler_y, width, theme.separator);

        // Tick marks
        let tick_interval = if time_span > 20.0 { 5.0 } else if time_span > 5.0 { 1.0 } else { 0.5 };
        let mut tick_time = (t_start / tick_interval).ceil() * tick_interval;
        while tick_time <= t_end {
            let tx = x + (tick_time - t_start) / time_span * width;
            WidgetDraw::text(engine, tx, ruler_y + 0.3, "|", theme.fg_dim, 0.05, RenderLayer::UI);
            WidgetDraw::text(engine, tx + 0.1, ruler_y + 0.3, &format!("{:.0}", tick_time), theme.fg_dim, 0.04, RenderLayer::UI);
            tick_time += tick_interval;
        }

        // Playhead
        let playhead_x = x + (self.playback.current_time - t_start) / time_span * width;
        if playhead_x >= x && playhead_x <= x + width {
            for i in 0..(height / 0.55) as usize {
                WidgetDraw::text(engine, playhead_x, ruler_y - i as f32 * 0.55, "|", theme.accent, 0.4, RenderLayer::UI);
            }
        }

        // Tracks
        let track_start_y = ruler_y - 0.6;
        let track_height = 0.8;
        for (ti, track) in self.tracks.iter().enumerate() {
            let ty = track_start_y - ti as f32 * (track_height + 0.2) + self.scroll_y;
            if ty < y - height || ty > y { continue; }

            // Track label
            let label_color = if track.muted { theme.fg_dim } else { track.color };
            WidgetDraw::text(engine, x + 0.3, ty, track.target.label(), label_color, 0.1, RenderLayer::UI);

            // Keyframe diamonds
            for (ki, kf) in track.keyframes.iter().enumerate() {
                let kx = x + (kf.time - t_start) / time_span * width;
                if kx < x || kx > x + width { continue; }
                let kf_char = if kf.selected { "o" } else { "." };
                let kf_color = if kf.selected { theme.warning } else { track.color };
                WidgetDraw::text(engine, kx, ty - 0.2, kf_char, kf_color, 0.3, RenderLayer::UI);
            }

            // Curve preview (simplified — sample and draw)
            if track.expanded && track.keyframes.len() >= 2 {
                let samples = 40;
                let first_t = track.keyframes.first().unwrap().time;
                let last_t = track.keyframes.last().unwrap().time;
                for s in 0..samples {
                    let st = first_t + (last_t - first_t) * s as f32 / samples as f32;
                    let sx = x + (st - t_start) / time_span * width;
                    if sx < x || sx > x + width { continue; }
                    let sv = track.evaluate(st);
                    let normalized = (sv - track.keyframes.iter().map(|k| k.value).fold(f32::MAX, f32::min))
                        / (track.keyframes.iter().map(|k| k.value).fold(f32::MIN, f32::max)
                           - track.keyframes.iter().map(|k| k.value).fold(f32::MAX, f32::min)).max(0.01);
                    let cy = ty - 0.1 - normalized * (track_height - 0.2);
                    WidgetDraw::text(engine, sx, cy, ".", track.color * 0.5, 0.05, RenderLayer::UI);
                }
            }
        }

        // Markers
        for marker in &self.markers {
            let mx = x + (marker.time - t_start) / time_span * width;
            if mx >= x && mx <= x + width {
                WidgetDraw::text(engine, mx, ruler_y + 0.6, "v", marker.color, 0.2, RenderLayer::UI);
                WidgetDraw::text(engine, mx + 0.3, ruler_y + 0.6, &marker.label, marker.color, 0.06, RenderLayer::UI);
            }
        }
    }
}

// =============================================================================
// FULL ANIMATION PLAYBACK ENGINE
// =============================================================================

use std::collections::HashMap;

/// All 15 easing types fully evaluated (extended beyond the existing EasingType).
pub fn evaluate_easing_full(easing: EasingType, t: f32) -> f32 {
    easing.evaluate(t)
}

/// Evaluate a single track at a specific time.
pub fn evaluate_track(track: &Track, time: f32) -> f32 {
    track.evaluate(time)
}

/// Evaluate all tracks in the timeline at the given time (not current_time).
pub fn evaluate_all_at(timeline: &Timeline, time: f32) -> HashMap<(u32, TrackTarget), f32> {
    let mut values = HashMap::new();
    for track in &timeline.tracks {
        if !track.muted {
            values.insert((track.node_id, track.target), track.evaluate(time));
        }
    }
    values
}

/// A minimal scene node for animation application.
#[derive(Clone, Debug, Default)]
pub struct SceneNode {
    pub id: u32,
    pub name: String,
    pub position: [f32; 3],
    pub rotation: f32,
    pub scale: [f32; 2],
    pub color: [f32; 4],
    pub emission: f32,
    pub glow_radius: f32,
    pub custom: HashMap<u32, f32>,
    pub visible: bool,
}

/// A scene document: a list of scene nodes, keyed by id.
#[derive(Clone, Debug, Default)]
pub struct SceneDocument {
    pub nodes: Vec<SceneNode>,
}

impl SceneDocument {
    pub fn new() -> Self { SceneDocument::default() }

    pub fn add_node(&mut self, name: &str) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(SceneNode { id, name: name.to_string(), ..Default::default() });
        id
    }

    pub fn get_node(&self, id: u32) -> Option<&SceneNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut SceneNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
}

/// Apply animated values from the timeline to the scene document.
pub fn apply_animation(timeline: &Timeline, time: f32, scene: &mut SceneDocument) {
    let values = evaluate_all_at(timeline, time);
    for ((node_id, target), value) in &values {
        if let Some(node) = scene.get_node_mut(*node_id) {
            match target {
                TrackTarget::PositionX => node.position[0] = *value,
                TrackTarget::PositionY => node.position[1] = *value,
                TrackTarget::PositionZ => node.position[2] = *value,
                TrackTarget::Rotation  => node.rotation = *value,
                TrackTarget::ScaleX    => node.scale[0] = *value,
                TrackTarget::ScaleY    => node.scale[1] = *value,
                TrackTarget::ColorR    => node.color[0] = *value,
                TrackTarget::ColorG    => node.color[1] = *value,
                TrackTarget::ColorB    => node.color[2] = *value,
                TrackTarget::ColorA    => node.color[3] = *value,
                TrackTarget::Emission  => node.emission = *value,
                TrackTarget::GlowRadius => node.glow_radius = *value,
                TrackTarget::Custom(idx) => { node.custom.insert(*idx, *value); }
            }
        }
    }
}

// =============================================================================
// ONION SKINNING
// =============================================================================

/// Configuration for onion skin ghost rendering.
#[derive(Clone, Debug)]
pub struct OnionSkinConfig {
    pub enabled: bool,
    pub frames_before: u32,
    pub frames_after: u32,
    pub opacity_decay: f32,
    /// Color tint for ghost frames before current time.
    pub color_before: [f32; 4],
    /// Color tint for ghost frames after current time.
    pub color_after: [f32; 4],
    pub show_lines: bool,
    pub max_ghost_count: u32,
}

impl Default for OnionSkinConfig {
    fn default() -> Self {
        OnionSkinConfig {
            enabled: false,
            frames_before: 3,
            frames_after: 3,
            opacity_decay: 0.5,
            color_before: [0.2, 0.4, 1.0, 0.5],
            color_after: [1.0, 0.4, 0.2, 0.5],
            show_lines: true,
            max_ghost_count: 5,
        }
    }
}

/// Data for a single onion skin ghost: time and evaluated node positions.
#[derive(Clone, Debug)]
pub struct OnionGhost {
    pub time: f32,
    pub is_before: bool,
    pub frame_offset: i32,
    pub node_states: HashMap<u32, [f32; 3]>,
    pub opacity: f32,
}

/// Build onion skin ghosts from the timeline at the current time.
pub fn build_onion_ghosts(timeline: &Timeline, config: &OnionSkinConfig) -> Vec<OnionGhost> {
    if !config.enabled { return vec![]; }
    let current_time = timeline.playback.current_time;
    let frame_dt = 1.0 / timeline.playback.frame_rate;
    let mut ghosts = vec![];

    // Before ghosts
    for i in 1..=config.frames_before.min(config.max_ghost_count) {
        let ghost_time = current_time - i as f32 * frame_dt;
        if ghost_time < 0.0 { continue; }
        let opacity = config.color_before[3] * config.opacity_decay.powi(i as i32);
        let values = evaluate_all_at(timeline, ghost_time);
        let mut node_states: HashMap<u32, [f32; 3]> = HashMap::new();
        for ((node_id, target), value) in &values {
            let entry = node_states.entry(*node_id).or_insert([0.0; 3]);
            match target {
                TrackTarget::PositionX => entry[0] = *value,
                TrackTarget::PositionY => entry[1] = *value,
                TrackTarget::PositionZ => entry[2] = *value,
                _ => {}
            }
        }
        ghosts.push(OnionGhost { time: ghost_time, is_before: true, frame_offset: -(i as i32), node_states, opacity });
    }

    // After ghosts
    for i in 1..=config.frames_after.min(config.max_ghost_count) {
        let ghost_time = current_time + i as f32 * frame_dt;
        if ghost_time > timeline.duration() { continue; }
        let opacity = config.color_after[3] * config.opacity_decay.powi(i as i32);
        let values = evaluate_all_at(timeline, ghost_time);
        let mut node_states: HashMap<u32, [f32; 3]> = HashMap::new();
        for ((node_id, target), value) in &values {
            let entry = node_states.entry(*node_id).or_insert([0.0; 3]);
            match target {
                TrackTarget::PositionX => entry[0] = *value,
                TrackTarget::PositionY => entry[1] = *value,
                TrackTarget::PositionZ => entry[2] = *value,
                _ => {}
            }
        }
        ghosts.push(OnionGhost { time: ghost_time, is_before: false, frame_offset: i as i32, node_states, opacity });
    }

    ghosts
}

// =============================================================================
// TANGENT HANDLE TYPES
// =============================================================================

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum TangentHandleType {
    Auto,
    Flat,
    Broken,
    Clamped,
    Free,
    Linear,
    Constant,
}

impl TangentHandleType {
    pub fn label(self) -> &'static str {
        match self {
            TangentHandleType::Auto => "Auto",
            TangentHandleType::Flat => "Flat",
            TangentHandleType::Broken => "Broken",
            TangentHandleType::Clamped => "Clamped",
            TangentHandleType::Free => "Free",
            TangentHandleType::Linear => "Linear",
            TangentHandleType::Constant => "Constant",
        }
    }
    pub fn all() -> &'static [TangentHandleType] {
        &[TangentHandleType::Auto, TangentHandleType::Flat, TangentHandleType::Broken,
          TangentHandleType::Clamped, TangentHandleType::Free, TangentHandleType::Linear, TangentHandleType::Constant]
    }
}

// =============================================================================
// CURVE EDITOR (Bezier handles)
// =============================================================================

/// State for the bezier curve editor panel.
#[derive(Clone, Debug)]
pub struct CurveEditorState {
    pub visible: bool,
    pub selected_track_ids: Vec<u32>,
    pub selected_keyframes: Vec<(u32, usize)>,   // (track_id, keyframe_index)
    pub box_select_start: Option<[f32; 2]>,
    pub box_select_end: Option<[f32; 2]>,
    pub view_min: [f32; 2],
    pub view_max: [f32; 2],
    pub tangent_mode: TangentHandleType,
    pub show_all_tracks: bool,
    pub show_tangent_handles: bool,
    pub snap_value: f32,
    pub snap_enabled: bool,
    pub frame_on_fit: bool,
}

impl Default for CurveEditorState {
    fn default() -> Self {
        CurveEditorState {
            visible: false,
            selected_track_ids: vec![],
            selected_keyframes: vec![],
            box_select_start: None,
            box_select_end: None,
            view_min: [0.0, -1.0],
            view_max: [10.0, 2.0],
            tangent_mode: TangentHandleType::Auto,
            show_all_tracks: true,
            show_tangent_handles: true,
            snap_value: 0.1,
            snap_enabled: false,
            frame_on_fit: true,
        }
    }
}

impl CurveEditorState {
    pub fn new() -> Self { CurveEditorState::default() }

    /// Fit the view to all selected tracks' keyframe ranges.
    pub fn fit_view(&mut self, tracks: &[Track]) {
        let mut min_t = f32::MAX; let mut max_t = f32::MIN;
        let mut min_v = f32::MAX; let mut max_v = f32::MIN;
        for track in tracks {
            if !self.selected_track_ids.is_empty() && !self.selected_track_ids.contains(&track.id) { continue; }
            for kf in &track.keyframes {
                if kf.time < min_t { min_t = kf.time; }
                if kf.time > max_t { max_t = kf.time; }
                if kf.value < min_v { min_v = kf.value; }
                if kf.value > max_v { max_v = kf.value; }
            }
        }
        if min_t == f32::MAX { return; }
        let t_pad = (max_t - min_t).max(0.1) * 0.1;
        let v_pad = (max_v - min_v).max(0.1) * 0.2;
        self.view_min = [min_t - t_pad, min_v - v_pad];
        self.view_max = [max_t + t_pad, max_v + v_pad];
    }

    /// Convert world (time, value) to canvas pixel coordinates.
    pub fn world_to_canvas(&self, time: f32, value: f32, canvas_rect: [f32; 4]) -> [f32; 2] {
        let cx = canvas_rect[0]; let cy = canvas_rect[1];
        let cw = canvas_rect[2]; let ch = canvas_rect[3];
        let tx = (time - self.view_min[0]) / (self.view_max[0] - self.view_min[0]);
        let tv = (value - self.view_min[1]) / (self.view_max[1] - self.view_min[1]);
        [cx + tx * cw, cy + (1.0 - tv) * ch]
    }

    /// Convert canvas pixel to world (time, value).
    pub fn canvas_to_world(&self, px: f32, py: f32, canvas_rect: [f32; 4]) -> [f32; 2] {
        let cx = canvas_rect[0]; let cy = canvas_rect[1];
        let cw = canvas_rect[2]; let ch = canvas_rect[3];
        let tx = (px - cx) / cw;
        let tv = 1.0 - (py - cy) / ch;
        [
            self.view_min[0] + tx * (self.view_max[0] - self.view_min[0]),
            self.view_min[1] + tv * (self.view_max[1] - self.view_min[1]),
        ]
    }

    /// Scale the view (zoom around center).
    pub fn zoom(&mut self, factor: f32, center_time: f32, center_value: f32) {
        let dt = (self.view_max[0] - self.view_min[0]) * (1.0 - factor);
        let dv = (self.view_max[1] - self.view_min[1]) * (1.0 - factor);
        let ct = (center_time - self.view_min[0]) / (self.view_max[0] - self.view_min[0]);
        let cv = (center_value - self.view_min[1]) / (self.view_max[1] - self.view_min[1]);
        self.view_min[0] += dt * ct;
        self.view_min[1] += dv * cv;
        self.view_max[0] -= dt * (1.0 - ct);
        self.view_max[1] -= dv * (1.0 - cv);
    }

    /// Pan the view.
    pub fn pan(&mut self, dt: f32, dv: f32) {
        self.view_min[0] += dt; self.view_max[0] += dt;
        self.view_min[1] += dv; self.view_max[1] += dv;
    }

    /// Select all keyframes within a box (in world coordinates).
    pub fn box_select(&mut self, tracks: &mut Vec<Track>, t_min: f32, t_max: f32, v_min: f32, v_max: f32) {
        self.selected_keyframes.clear();
        for track in tracks.iter_mut() {
            if !self.selected_track_ids.is_empty() && !self.selected_track_ids.contains(&track.id) { continue; }
            for (ki, kf) in track.keyframes.iter_mut().enumerate() {
                let in_box = kf.time >= t_min && kf.time <= t_max && kf.value >= v_min && kf.value <= v_max;
                kf.selected = in_box;
                if in_box { self.selected_keyframes.push((track.id, ki)); }
            }
        }
    }

    /// Scale selected keyframes around center.
    pub fn scale_selection(&self, tracks: &mut Vec<Track>, time_scale: f32, value_scale: f32, pivot_time: f32, pivot_value: f32) {
        for track in tracks.iter_mut() {
            for kf in track.keyframes.iter_mut() {
                if kf.selected {
                    kf.time = pivot_time + (kf.time - pivot_time) * time_scale;
                    kf.value = pivot_value + (kf.value - pivot_value) * value_scale;
                }
            }
            track.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    /// Offset selected keyframes.
    pub fn offset_selection(&self, tracks: &mut Vec<Track>, dt: f32, dv: f32) {
        for track in tracks.iter_mut() {
            for kf in track.keyframes.iter_mut() {
                if kf.selected {
                    kf.time = (kf.time + dt).max(0.0);
                    kf.value += dv;
                }
            }
            track.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    /// Apply a tangent mode to selected keyframes.
    pub fn apply_tangent_mode(&self, tracks: &mut Vec<Track>, mode: TangentHandleType) {
        for track in tracks.iter_mut() {
            for kf in track.keyframes.iter_mut() {
                if !kf.selected { continue; }
                match mode {
                    TangentHandleType::Flat => { kf.tangent_in = 0.0; kf.tangent_out = 0.0; }
                    TangentHandleType::Linear => {
                        // Tangents would be set to finite difference in a full impl
                        kf.tangent_in = 1.0; kf.tangent_out = 1.0;
                    }
                    TangentHandleType::Auto => {
                        // Auto smooth — leave defaults
                        kf.tangent_in = 0.0; kf.tangent_out = 0.0;
                    }
                    TangentHandleType::Constant => {
                        kf.easing = EasingType::Step;
                    }
                    _ => {}
                }
            }
        }
    }
}

// =============================================================================
// DOPE SHEET VIEW
// =============================================================================

/// State for the dope sheet panel.
#[derive(Clone, Debug, Default)]
pub struct DopeSheetState {
    pub visible: bool,
    pub scroll_y: f32,
    pub selected_keyframes: Vec<(u32, usize)>,
    pub clipboard: Vec<(TrackTarget, f32, f32)>,  // (target, time, value)
    pub span_select_start: Option<f32>,
    pub row_height: f32,
    pub zoom_time: f32,
    pub show_children: bool,
}

impl DopeSheetState {
    pub fn new() -> Self {
        DopeSheetState {
            visible: false,
            scroll_y: 0.0,
            selected_keyframes: vec![],
            clipboard: vec![],
            span_select_start: None,
            row_height: 20.0,
            zoom_time: 1.0,
            show_children: true,
        }
    }

    /// Copy selected keyframes into clipboard.
    pub fn copy_selection(&mut self, tracks: &[Track]) {
        self.clipboard.clear();
        for (track_id, ki) in &self.selected_keyframes {
            if let Some(track) = tracks.iter().find(|t| t.id == *track_id) {
                if let Some(kf) = track.keyframes.get(*ki) {
                    self.clipboard.push((track.target, kf.time, kf.value));
                }
            }
        }
    }

    /// Paste keyframes from clipboard at offset time.
    pub fn paste_at(&self, tracks: &mut Vec<Track>, time_offset: f32) {
        for (target, t, v) in &self.clipboard {
            if let Some(track) = tracks.iter_mut().find(|tr| tr.target == *target) {
                track.add_keyframe(Keyframe::new(t + time_offset, *v));
            }
        }
    }

    /// Nudge selected keyframes by N frames.
    pub fn nudge_frames(&self, tracks: &mut Vec<Track>, frames: i32, frame_rate: f32) {
        let dt = frames as f32 / frame_rate;
        for track in tracks.iter_mut() {
            for (i, kf) in track.keyframes.iter_mut().enumerate() {
                let is_selected = self.selected_keyframes.iter().any(|(tid, ki)| *tid == track.id && *ki == i);
                if is_selected {
                    kf.time = (kf.time + dt).max(0.0);
                }
            }
            track.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    /// Select all keyframes in a time range.
    pub fn select_range(&mut self, tracks: &mut Vec<Track>, t_start: f32, t_end: f32) {
        self.selected_keyframes.clear();
        for track in tracks.iter_mut() {
            for (i, kf) in track.keyframes.iter_mut().enumerate() {
                if kf.time >= t_start && kf.time <= t_end {
                    kf.selected = true;
                    self.selected_keyframes.push((track.id, i));
                } else {
                    kf.selected = false;
                }
            }
        }
    }

    /// Render the dope sheet into a given rect area.
    pub fn render(
        &self,
        tracks: &[Track],
        markers: &[TimelineMarker],
        playback: &PlaybackControl,
        visible_range: (f32, f32),
        rect_x: f32, rect_y: f32, rect_w: f32, rect_h: f32,
    ) {
        // NOTE: Rendering logic in real egui would use painter — this is a stub
        // showing the structure. Full egui render calls would go here.
        let _ = (tracks, markers, playback, visible_range, rect_x, rect_y, rect_w, rect_h);
    }
}

// =============================================================================
// MOTION PATHS
// =============================================================================

/// A motion path: the 2D trajectory of a node over time.
#[derive(Clone, Debug)]
pub struct MotionPath {
    pub node_id: u32,
    pub positions: Vec<([f32; 2], f32)>,  // (position, time)
    pub show_path: bool,
    pub path_color: [f32; 4],
    pub show_dots_at_frames: bool,
    pub dot_size: f32,
}

impl Default for MotionPath {
    fn default() -> Self {
        MotionPath {
            node_id: 0, positions: vec![],
            show_path: true, path_color: [0.5, 0.8, 1.0, 0.8],
            show_dots_at_frames: true, dot_size: 3.0,
        }
    }
}

/// Build a motion path by evaluating X/Y tracks at sample intervals.
pub fn build_motion_path(timeline: &Timeline, node_id: u32) -> MotionPath {
    let x_track = timeline.tracks.iter().find(|t| t.node_id == node_id && t.target == TrackTarget::PositionX);
    let y_track = timeline.tracks.iter().find(|t| t.node_id == node_id && t.target == TrackTarget::PositionY);
    let duration = timeline.duration();
    if duration <= 0.0 { return MotionPath { node_id, ..Default::default() }; }
    let samples = 120usize;
    let mut positions = vec![];
    for i in 0..=samples {
        let t = i as f32 / samples as f32 * duration;
        let x = x_track.map(|tr| tr.evaluate(t)).unwrap_or(0.0);
        let y = y_track.map(|tr| tr.evaluate(t)).unwrap_or(0.0);
        positions.push(([x, y], t));
    }
    MotionPath { node_id, positions, show_path: true, path_color: [0.5, 0.8, 1.0, 0.8], show_dots_at_frames: true, dot_size: 3.0 }
}

/// Render motion paths as dotted lines in a viewport.
pub fn render_motion_paths(
    paths: &[MotionPath],
    painter: &egui::Painter,
    rect: egui::Rect,
    world_to_screen: impl Fn([f32; 2]) -> egui::Pos2,
    frame_rate: f32,
    current_time: f32,
) {
    for path in paths {
        if !path.show_path || path.positions.len() < 2 { continue; }
        let col = egui::Color32::from_rgba_unmultiplied(
            (path.path_color[0] * 255.0) as u8,
            (path.path_color[1] * 255.0) as u8,
            (path.path_color[2] * 255.0) as u8,
            (path.path_color[3] * 255.0) as u8,
        );
        // Draw dotted path segments
        for i in 0..path.positions.len() - 1 {
            let (pos_a, t_a) = path.positions[i];
            let (pos_b, _t_b) = path.positions[i + 1];
            let sa = world_to_screen(pos_a);
            let sb = world_to_screen(pos_b);
            if !rect.contains(sa) && !rect.contains(sb) { continue; }
            // Dashed line: draw only every other segment
            if i % 2 == 0 {
                painter.line_segment([sa, sb], egui::Stroke::new(1.5, col));
            }
            // Frame dots
            if path.show_dots_at_frames {
                let frame_time = t_a * frame_rate;
                if frame_time.fract() < 0.05 {
                    let is_current = (t_a - current_time).abs() < 0.5 / frame_rate;
                    let dot_col = if is_current { egui::Color32::WHITE } else { col };
                    painter.circle_filled(sa, if is_current { path.dot_size + 2.0 } else { path.dot_size }, dot_col);
                }
            }
        }
    }
}

// =============================================================================
// TRACK GROUPS & FOLDERS
// =============================================================================

#[derive(Clone, Debug)]
pub struct TrackGroup {
    pub id: u32,
    pub name: String,
    pub tracks: Vec<u32>,
    pub collapsed: bool,
    pub color: [f32; 4],
    pub visible: bool,
    pub locked: bool,
    pub solo: bool,
}

impl Default for TrackGroup {
    fn default() -> Self {
        TrackGroup {
            id: 0, name: String::from("Group"),
            tracks: vec![], collapsed: false,
            color: [0.5, 0.5, 1.0, 1.0],
            visible: true, locked: false, solo: false,
        }
    }
}

impl TrackGroup {
    pub fn new(id: u32, name: &str) -> Self {
        TrackGroup { id, name: name.to_string(), ..Default::default() }
    }

    pub fn contains_track(&self, track_id: u32) -> bool {
        self.tracks.contains(&track_id)
    }

    pub fn add_track(&mut self, track_id: u32) {
        if !self.tracks.contains(&track_id) { self.tracks.push(track_id); }
    }

    pub fn remove_track(&mut self, track_id: u32) {
        self.tracks.retain(|&t| t != track_id);
    }
}

/// Manager for track groups in the timeline.
#[derive(Clone, Debug, Default)]
pub struct TrackGroupManager {
    pub groups: Vec<TrackGroup>,
    pub next_group_id: u32,
}

impl TrackGroupManager {
    pub fn new() -> Self { TrackGroupManager::default() }

    pub fn add_group(&mut self, name: &str) -> u32 {
        let id = self.next_group_id;
        self.next_group_id += 1;
        self.groups.push(TrackGroup::new(id, name));
        id
    }

    pub fn remove_group(&mut self, group_id: u32) {
        self.groups.retain(|g| g.id != group_id);
    }

    pub fn get_group(&self, group_id: u32) -> Option<&TrackGroup> {
        self.groups.iter().find(|g| g.id == group_id)
    }

    pub fn get_group_mut(&mut self, group_id: u32) -> Option<&mut TrackGroup> {
        self.groups.iter_mut().find(|g| g.id == group_id)
    }

    pub fn group_for_track(&self, track_id: u32) -> Option<u32> {
        self.groups.iter().find(|g| g.contains_track(track_id)).map(|g| g.id)
    }

    pub fn move_track_to_group(&mut self, track_id: u32, target_group_id: u32) {
        // Remove from any existing group
        for group in self.groups.iter_mut() {
            group.remove_track(track_id);
        }
        // Add to target
        if let Some(group) = self.get_group_mut(target_group_id) {
            group.add_track(track_id);
        }
    }

    pub fn render_group_header(
        ui: &mut egui::Ui,
        group: &mut TrackGroup,
    ) {
        let group_col = egui::Color32::from_rgba_unmultiplied(
            (group.color[0] * 200.0 + 55.0) as u8,
            (group.color[1] * 200.0 + 55.0) as u8,
            (group.color[2] * 200.0 + 55.0) as u8,
            (group.color[3] * 255.0) as u8,
        );
        ui.horizontal(|ui| {
            let arrow = if group.collapsed { "▶" } else { "▼" };
            if ui.small_button(arrow).clicked() { group.collapsed = !group.collapsed; }
            let vis = if group.visible { "O" } else { " " };
            if ui.small_button(vis).clicked() { group.visible = !group.visible; }
            let lock = if group.locked { "L" } else { "U" };
            if ui.small_button(lock).clicked() { group.locked = !group.locked; }
            ui.colored_label(group_col, egui::RichText::new(&group.name).strong());
            ui.label(egui::RichText::new(format!("({} tracks)", group.tracks.len())).size(9.0).color(egui::Color32::from_gray(120)));
        });
    }
}

// =============================================================================
// ANIMATION CLIPS & STATE MACHINE
// =============================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopMode { Once, Loop, PingPong, ClampForever }

impl LoopMode {
    pub fn label(self) -> &'static str {
        match self {
            LoopMode::Once => "Once", LoopMode::Loop => "Loop",
            LoopMode::PingPong => "Ping Pong", LoopMode::ClampForever => "Clamp Forever",
        }
    }
    pub fn all() -> &'static [LoopMode] {
        &[LoopMode::Once, LoopMode::Loop, LoopMode::PingPong, LoopMode::ClampForever]
    }
}

#[derive(Clone, Debug)]
pub struct AnimClip {
    pub id: u32,
    pub name: String,
    pub start_time: f32,
    pub end_time: f32,
    pub loop_mode: LoopMode,
    pub speed: f32,
    pub weight: f32,
    pub enabled: bool,
}

impl Default for AnimClip {
    fn default() -> Self {
        AnimClip { id: 0, name: String::from("Clip"), start_time: 0.0, end_time: 1.0, loop_mode: LoopMode::Once, speed: 1.0, weight: 1.0, enabled: true }
    }
}

impl AnimClip {
    pub fn new(id: u32, name: &str, start_time: f32, end_time: f32) -> Self {
        AnimClip { id, name: name.to_string(), start_time, end_time, ..Default::default() }
    }

    pub fn duration(&self) -> f32 { (self.end_time - self.start_time).max(0.0) }

    /// Evaluate local time within this clip given absolute time.
    pub fn evaluate_local_time(&self, abs_time: f32) -> f32 {
        let local = (abs_time - self.start_time) * self.speed;
        let dur = self.duration().max(1e-6);
        match self.loop_mode {
            LoopMode::Once => local.clamp(0.0, dur),
            LoopMode::Loop => local.rem_euclid(dur),
            LoopMode::PingPong => {
                let cycle = (local / dur) as u32;
                let t = local.rem_euclid(dur);
                if cycle % 2 == 0 { t } else { dur - t }
            }
            LoopMode::ClampForever => local.clamp(0.0, dur),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AnimTransition {
    pub id: u32,
    pub condition: String,
    pub condition_value: f32,
    pub condition_op: TransitionOp,
    pub target_state: usize,
    pub blend_time: f32,
    pub has_exit_time: bool,
    pub exit_time: f32,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum TransitionOp { GreaterThan, LessThan, Equals, NotEquals, Always }

impl TransitionOp {
    pub fn label(self) -> &'static str {
        match self { TransitionOp::GreaterThan => ">", TransitionOp::LessThan => "<", TransitionOp::Equals => "==", TransitionOp::NotEquals => "!=", TransitionOp::Always => "Always" }
    }
    pub fn evaluate(self, a: f32, b: f32) -> bool {
        match self { TransitionOp::GreaterThan => a > b, TransitionOp::LessThan => a < b, TransitionOp::Equals => (a - b).abs() < 0.001, TransitionOp::NotEquals => (a - b).abs() >= 0.001, TransitionOp::Always => true }
    }
}

#[derive(Clone, Debug)]
pub struct AnimState {
    pub id: usize,
    pub name: String,
    pub clip: Option<usize>,
    pub transitions: Vec<AnimTransition>,
    pub position: [f32; 2],  // Position in state machine graph
    pub is_default: bool,
    pub speed_multiplier: f32,
}

impl AnimState {
    pub fn new(id: usize, name: &str) -> Self {
        AnimState { id, name: name.to_string(), clip: None, transitions: vec![], position: [id as f32 * 150.0, 0.0], is_default: id == 0, speed_multiplier: 1.0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnimStateMachine {
    pub states: Vec<AnimState>,
    pub current_state: usize,
    pub previous_state: Option<usize>,
    pub clips: Vec<AnimClip>,
    pub parameters: HashMap<String, f32>,
    pub blend_time_remaining: f32,
    pub current_blend_duration: f32,
    pub next_clip_id: u32,
    pub selected_state: Option<usize>,
}

impl AnimStateMachine {
    pub fn new() -> Self { AnimStateMachine::default() }

    pub fn add_state(&mut self, name: &str) -> usize {
        let id = self.states.len();
        self.states.push(AnimState::new(id, name));
        id
    }

    pub fn add_clip(&mut self, name: &str, start: f32, end: f32) -> usize {
        let id = self.clips.len();
        self.clips.push(AnimClip::new(self.next_clip_id, name, start, end));
        self.next_clip_id += 1;
        id
    }

    pub fn set_parameter(&mut self, name: &str, value: f32) {
        self.parameters.insert(name.to_string(), value);
    }

    pub fn get_parameter(&self, name: &str) -> f32 {
        *self.parameters.get(name).unwrap_or(&0.0)
    }

    /// Evaluate transitions for the current state.
    pub fn tick_transitions(&mut self, dt: f32, timeline_time: f32) {
        if self.blend_time_remaining > 0.0 {
            self.blend_time_remaining -= dt;
            return;
        }
        if self.current_state >= self.states.len() { return; }
        let transitions = self.states[self.current_state].transitions.clone();
        for transition in &transitions {
            let condition_met = if transition.has_exit_time {
                // Check if clip has reached exit time
                if let Some(clip_idx) = self.states[self.current_state].clip {
                    if let Some(clip) = self.clips.get(clip_idx) {
                        let local = clip.evaluate_local_time(timeline_time);
                        local >= clip.duration() * transition.exit_time
                    } else { false }
                } else { false }
            } else {
                transition.condition_op.evaluate(self.get_parameter(&transition.condition), transition.condition_value)
            };
            if condition_met {
                self.previous_state = Some(self.current_state);
                self.current_state = transition.target_state;
                self.blend_time_remaining = transition.blend_time;
                self.current_blend_duration = transition.blend_time;
                break;
            }
        }
    }

    pub fn show_visualizer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Anim State Machine");
        ui.separator();
        if self.states.is_empty() {
            ui.label(egui::RichText::new("No states. Click + State to add one.").color(egui::Color32::from_gray(120)));
        }
        if ui.button("+ State").clicked() {
            let n = format!("State {}", self.states.len());
            self.add_state(&n);
        }
        ui.separator();
        // State boxes
        let (canvas, _resp) = ui.allocate_exact_size(egui::Vec2::new(400.0, 300.0), egui::Sense::click_and_drag());
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, 4.0, egui::Color32::from_gray(18));
        // Draw transition arrows
        for state in &self.states {
            for trans in &state.transitions {
                if trans.target_state < self.states.len() {
                    let src = &self.states[state.id];
                    let dst = &self.states[trans.target_state];
                    let sx = canvas.min.x + src.position[0].min(380.0);
                    let sy = canvas.min.y + src.position[1].min(280.0) + 15.0;
                    let dx = canvas.min.x + dst.position[0].min(380.0);
                    let dy = canvas.min.y + dst.position[1].min(280.0) + 15.0;
                    painter.arrow(
                        egui::Pos2::new(sx + 40.0, sy),
                        egui::Pos2::new(dx, dy) - egui::Pos2::new(sx + 40.0, sy),
                        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(180, 180, 80, 200)),
                    );
                    let mx = (sx + dx) / 2.0;
                    let my = (sy + dy) / 2.0;
                    painter.text(egui::Pos2::new(mx, my), egui::Align2::CENTER_CENTER, &trans.condition, egui::FontId::proportional(9.0), egui::Color32::from_gray(160));
                }
            }
        }
        // Draw state boxes
        for (i, state) in self.states.iter().enumerate() {
            let bx = canvas.min.x + state.position[0].min(380.0 - 80.0);
            let by = canvas.min.y + state.position[1].min(280.0 - 30.0);
            let box_rect = egui::Rect::from_min_size(egui::Pos2::new(bx, by), egui::Vec2::new(80.0, 30.0));
            let is_current = self.current_state == i;
            let is_selected = self.selected_state == Some(i);
            let bg = if is_current { egui::Color32::from_rgb(40, 120, 200) }
                     else if is_selected { egui::Color32::from_rgb(60, 80, 110) }
                     else if state.is_default { egui::Color32::from_rgb(60, 100, 60) }
                     else { egui::Color32::from_gray(45) };
            painter.rect_filled(box_rect, 4.0, bg);
            painter.rect_stroke(box_rect, 4.0, egui::Stroke::new(1.0, if is_current { egui::Color32::WHITE } else { egui::Color32::from_gray(100) }), egui::StrokeKind::Outside);
            painter.text(egui::Pos2::new(bx + 4.0, by + 8.0), egui::Align2::LEFT_TOP, &state.name, egui::FontId::proportional(10.0), egui::Color32::WHITE);
            if let Some(clip_idx) = state.clip {
                if let Some(clip) = self.clips.get(clip_idx) {
                    painter.text(egui::Pos2::new(bx + 4.0, by + 19.0), egui::Align2::LEFT_TOP, &clip.name, egui::FontId::proportional(8.0), egui::Color32::from_gray(180));
                }
            }
        }
        ui.separator();
        // State settings
        if let Some(sel) = self.selected_state {
            if sel < self.states.len() {
                ui.label(egui::RichText::new(format!("State: {}", self.states[sel].name)).strong());
                ui.horizontal(|ui| {
                    ui.label("Clip:");
                    let clip_name = self.states[sel].clip.and_then(|ci| self.clips.get(ci)).map(|c| c.name.clone()).unwrap_or_else(|| "(none)".into());
                    egui::ComboBox::from_id_source("state_clip_sel").selected_text(&clip_name).show_ui(ui, |ui| {
                        for (ci, clip) in self.clips.iter().enumerate() {
                            if ui.selectable_label(self.states[sel].clip == Some(ci), &clip.name).clicked() {
                                self.states[sel].clip = Some(ci);
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Speed:");
                    ui.add(egui::DragValue::new(&mut self.states[sel].speed_multiplier).speed(0.01).range(0.0..=10.0));
                });
                ui.checkbox(&mut self.states[sel].is_default, "Default State");
            }
        }
        ui.separator();
        // Parameters
        ui.label("Parameters:");
        let mut param_to_add: Option<String> = None;
        let mut param_to_remove: Option<String> = None;
        for (key, val) in self.parameters.iter_mut() {
            ui.horizontal(|ui| {
                ui.label(key.clone());
                ui.add(egui::DragValue::new(val).speed(0.01));
                if ui.small_button("X").clicked() { param_to_remove = Some(key.clone()); }
            });
        }
        if let Some(k) = param_to_remove { self.parameters.remove(&k); }
        ui.horizontal(|ui| {
            if ui.small_button("+ Add Parameter").clicked() {
                param_to_add = Some(format!("param{}", self.parameters.len()));
            }
        });
        if let Some(k) = param_to_add { self.parameters.insert(k, 0.0); }
    }
}

// =============================================================================
// SCRUBBING & PREVIEW — EXTENDED PLAYBACK
// =============================================================================

/// Frame rate presets.
pub const FRAME_RATE_PRESETS: &[(u32, f32)] = &[
    (24, 24.0), (25, 25.0), (30, 30.0), (48, 48.0), (60, 60.0), (120, 120.0),
];

/// Format a time value as SMPTE timecode HH:MM:SS:FF.
pub fn format_smpte(time: f32, frame_rate: f32) -> String {
    let total_frames = (time * frame_rate).round() as u32;
    let fps = frame_rate.round() as u32;
    let ff = total_frames % fps;
    let total_secs = total_frames / fps;
    let ss = total_secs % 60;
    let mm = (total_secs / 60) % 60;
    let hh = total_secs / 3600;
    format!("{:02}:{:02}:{:02}:{:02}", hh, mm, ss, ff)
}

/// Parse SMPTE timecode back to seconds.
pub fn parse_smpte(smpte: &str, frame_rate: f32) -> Option<f32> {
    let parts: Vec<&str> = smpte.split(':').collect();
    if parts.len() != 4 { return None; }
    let hh: u32 = parts[0].parse().ok()?;
    let mm: u32 = parts[1].parse().ok()?;
    let ss: u32 = parts[2].parse().ok()?;
    let ff: u32 = parts[3].parse().ok()?;
    let total_frames = hh * 3600 * frame_rate as u32 + mm * 60 * frame_rate as u32 + ss * frame_rate as u32 + ff;
    Some(total_frames as f32 / frame_rate)
}

/// Region of interest: in/out points for looping playback.
#[derive(Clone, Debug)]
pub struct RegionOfInterest {
    pub in_point: f32,
    pub out_point: f32,
    pub active: bool,
}

impl Default for RegionOfInterest {
    fn default() -> Self {
        RegionOfInterest { in_point: 0.0, out_point: 10.0, active: false }
    }
}

impl RegionOfInterest {
    pub fn set_in(&mut self, time: f32) { self.in_point = time; self.active = true; }
    pub fn set_out(&mut self, time: f32) { self.out_point = time; self.active = true; }
    pub fn clear(&mut self) { self.in_point = 0.0; self.out_point = f32::MAX; self.active = false; }
    pub fn clamp_time(&self, t: f32) -> f32 {
        if !self.active { return t; }
        t.clamp(self.in_point, self.out_point)
    }
    pub fn loop_time(&self, t: f32) -> f32 {
        if !self.active { return t; }
        let dur = (self.out_point - self.in_point).max(1e-6);
        self.in_point + (t - self.in_point).rem_euclid(dur)
    }
}

/// Extended scrubbing controls.
#[derive(Clone, Debug)]
pub struct ScrubController {
    pub roi: RegionOfInterest,
    pub frame_rate_preset_idx: usize,
    pub smpte_display: bool,
    pub scrub_step_frames: u32,
    pub ping_pong: bool,
    pub is_ping_pong_forward: bool,
}

impl Default for ScrubController {
    fn default() -> Self {
        ScrubController {
            roi: RegionOfInterest::default(),
            frame_rate_preset_idx: 2,  // 30fps
            smpte_display: false,
            scrub_step_frames: 1,
            ping_pong: false,
            is_ping_pong_forward: true,
        }
    }
}

impl ScrubController {
    pub fn current_frame_rate(&self) -> f32 {
        FRAME_RATE_PRESETS.get(self.frame_rate_preset_idx).map(|&(_, fps)| fps).unwrap_or(30.0)
    }

    pub fn tick_ping_pong(&mut self, playback: &mut PlaybackControl) {
        if !self.ping_pong || !self.roi.active { return; }
        if self.is_ping_pong_forward {
            if playback.current_time >= self.roi.out_point {
                playback.current_time = self.roi.out_point;
                self.is_ping_pong_forward = false;
                playback.speed = -playback.speed.abs();
            }
        } else {
            if playback.current_time <= self.roi.in_point {
                playback.current_time = self.roi.in_point;
                self.is_ping_pong_forward = true;
                playback.speed = playback.speed.abs();
            }
        }
    }

    pub fn show_controls(&mut self, ui: &mut egui::Ui, playback: &mut PlaybackControl) {
        ui.horizontal(|ui| {
            // Transport buttons
            if ui.button("⏮").clicked() { playback.current_time = 0.0; }
            if ui.button("◀").on_hover_text("Previous frame (,)").clicked() { playback.step_backward(); }
            match playback.state {
                PlaybackState::Playing => { if ui.button("⏸").clicked() { playback.pause(); } }
                _ => { if ui.button("▶").clicked() { playback.play(); } }
            }
            if ui.button("▶").on_hover_text("Next frame (.)").clicked() { playback.step_forward(); }
            if ui.button("⏭").clicked() { playback.current_time = playback.loop_end; }
            if ui.button("■").on_hover_text("Stop").clicked() { playback.stop(); }
        });
        ui.horizontal(|ui| {
            ui.label("FPS:");
            for (i, &(fps_label, fps_val)) in FRAME_RATE_PRESETS.iter().enumerate() {
                let selected = self.frame_rate_preset_idx == i;
                if ui.selectable_label(selected, format!("{}", fps_label)).clicked() {
                    self.frame_rate_preset_idx = i;
                    playback.frame_rate = fps_val;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Speed:");
            ui.add(egui::DragValue::new(&mut playback.speed).speed(0.05).range(-8.0..=8.0));
            ui.label("x");
            ui.checkbox(&mut playback.snap_to_frames, "Snap");
            ui.checkbox(&mut self.smpte_display, "SMPTE");
        });
        // Time display
        ui.horizontal(|ui| {
            let fps = playback.frame_rate;
            if self.smpte_display {
                ui.label(egui::RichText::new(format_smpte(playback.current_time, fps)).monospace().size(13.0));
            } else {
                ui.label(egui::RichText::new(format!("{:.3}s  F:{}", playback.current_time, playback.current_frame())).monospace().size(12.0));
            }
            ui.label(egui::RichText::new(format!("/ {:.1}s", playback.loop_end)).color(egui::Color32::from_gray(120)).size(10.0));
        });
        ui.separator();
        // Region of Interest
        ui.label("Region of Interest:");
        ui.horizontal(|ui| {
            if ui.button("Set In").clicked() { self.roi.set_in(playback.current_time); }
            if ui.button("Set Out").clicked() { self.roi.set_out(playback.current_time); }
            if ui.button("Clear").clicked() { self.roi.clear(); }
            ui.checkbox(&mut self.roi.active, "Active");
            ui.checkbox(&mut self.ping_pong, "Ping-Pong");
        });
        if self.roi.active {
            ui.horizontal(|ui| {
                let fps = playback.frame_rate;
                ui.label(egui::RichText::new(format!("In: {}  Out: {}", format_smpte(self.roi.in_point, fps), format_smpte(self.roi.out_point, fps))).size(10.0));
            });
        }
    }
}

// =============================================================================
// EXTENDED TIMELINE STATE — ties everything together
// =============================================================================

/// Full extended timeline state: groups, dope sheet, curve editor, state machine, etc.
pub struct TimelineExtended {
    pub curve_editor: CurveEditorState,
    pub dope_sheet: DopeSheetState,
    pub group_manager: TrackGroupManager,
    pub state_machine: AnimStateMachine,
    pub scrub: ScrubController,
    pub onion_skin: OnionSkinConfig,
    pub motion_paths: Vec<MotionPath>,
    pub show_curve_editor: bool,
    pub show_dope_sheet: bool,
    pub show_state_machine: bool,
    pub show_motion_paths: bool,
}

impl Default for TimelineExtended {
    fn default() -> Self {
        TimelineExtended {
            curve_editor: CurveEditorState::default(),
            dope_sheet: DopeSheetState::new(),
            group_manager: TrackGroupManager::new(),
            state_machine: AnimStateMachine::new(),
            scrub: ScrubController::default(),
            onion_skin: OnionSkinConfig::default(),
            motion_paths: vec![],
            show_curve_editor: false,
            show_dope_sheet: true,
            show_state_machine: false,
            show_motion_paths: false,
        }
    }
}

impl TimelineExtended {
    pub fn new() -> Self { TimelineExtended::default() }

    /// Tick playback and all subsystems.
    pub fn tick(&mut self, dt: f32, timeline: &mut Timeline, scene: &mut SceneDocument) {
        timeline.playback.tick(dt);
        // Clamp to ROI
        if self.scrub.roi.active && timeline.playback.state == PlaybackState::Playing {
            if timeline.playback.current_time > self.scrub.roi.out_point {
                if timeline.playback.loop_enabled {
                    timeline.playback.current_time = self.scrub.roi.loop_time(timeline.playback.current_time);
                } else {
                    timeline.playback.current_time = self.scrub.roi.out_point;
                    timeline.playback.pause();
                }
            }
        }
        self.scrub.tick_ping_pong(&mut timeline.playback);
        self.state_machine.tick_transitions(dt, timeline.playback.current_time);
        apply_animation(timeline, timeline.playback.current_time, scene);
    }

    /// Rebuild motion paths for all tracked node IDs.
    pub fn rebuild_motion_paths(&mut self, timeline: &Timeline, node_ids: &[u32]) {
        self.motion_paths = node_ids.iter().map(|&id| build_motion_path(timeline, id)).collect();
    }

    /// Show the full extended timeline UI.
    pub fn show(&mut self, ui: &mut egui::Ui, timeline: &mut Timeline, scene: &mut SceneDocument) {
        // Toolbar
        ui.horizontal(|ui| {
            self.scrub.show_controls(ui, &mut timeline.playback);
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.toggle_value(&mut self.show_dope_sheet, "Dope Sheet");
            ui.toggle_value(&mut self.show_curve_editor, "Curve Editor");
            ui.toggle_value(&mut self.show_state_machine, "State Machine");
            ui.toggle_value(&mut self.show_motion_paths, "Motion Paths");
            ui.separator();
            ui.checkbox(&mut self.onion_skin.enabled, "Onion Skin");
        });
        ui.separator();

        // Onion skin settings
        if self.onion_skin.enabled {
            egui::CollapsingHeader::new("Onion Skin Settings").default_open(false).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Before:");
                    ui.add(egui::DragValue::new(&mut self.onion_skin.frames_before).speed(1.0).range(0..=10));
                    ui.label("After:");
                    ui.add(egui::DragValue::new(&mut self.onion_skin.frames_after).speed(1.0).range(0..=10));
                });
                ui.horizontal(|ui| {
                    ui.label("Opacity Decay:");
                    ui.add(egui::Slider::new(&mut self.onion_skin.opacity_decay, 0.0..=1.0));
                });
                ui.checkbox(&mut self.onion_skin.show_lines, "Show Lines");
            });
        }

        // Track group management
        egui::CollapsingHeader::new("Track Groups").default_open(false).show(ui, |ui| {
            if ui.button("+ Add Group").clicked() {
                let n = format!("Group {}", self.group_manager.groups.len());
                self.group_manager.add_group(&n);
            }
            let mut to_remove = None;
            for i in 0..self.group_manager.groups.len() {
                ui.push_id(i, |ui| {
                    TrackGroupManager::render_group_header(ui, &mut self.group_manager.groups[i]);
                    if !self.group_manager.groups[i].collapsed {
                        ui.indent(format!("group_tracks_{}", i), |ui| {
                            let tracks_in_group = self.group_manager.groups[i].tracks.clone();
                            for tid in &tracks_in_group {
                                if let Some(track) = timeline.tracks.iter().find(|t| t.id == *tid) {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(format!("  Track {}: {}", tid, track.target.label())).size(10.0));
                                        if ui.small_button("Remove from group").clicked() {
                                            self.group_manager.groups[i].remove_track(*tid);
                                        }
                                    });
                                }
                            }
                        });
                    }
                    if ui.small_button("Delete Group").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(i) = to_remove { self.group_manager.groups.remove(i); }
        });

        // Dope Sheet
        if self.show_dope_sheet {
            egui::CollapsingHeader::new("Dope Sheet").default_open(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Selected: {} keyframes", self.dope_sheet.selected_keyframes.len()));
                    if ui.small_button("Copy").clicked() { self.dope_sheet.copy_selection(&timeline.tracks); }
                    if ui.small_button("Paste").clicked() { self.dope_sheet.paste_at(&mut timeline.tracks, timeline.playback.current_time); }
                    if ui.small_button("Deselect All").clicked() {
                        self.dope_sheet.selected_keyframes.clear();
                        for track in &mut timeline.tracks { for kf in &mut track.keyframes { kf.selected = false; } }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Nudge:");
                    if ui.small_button("< 1f").clicked() { self.dope_sheet.nudge_frames(&mut timeline.tracks, -1, timeline.playback.frame_rate); }
                    if ui.small_button("> 1f").clicked() { self.dope_sheet.nudge_frames(&mut timeline.tracks, 1, timeline.playback.frame_rate); }
                    if ui.small_button("< 5f").clicked() { self.dope_sheet.nudge_frames(&mut timeline.tracks, -5, timeline.playback.frame_rate); }
                    if ui.small_button("> 5f").clicked() { self.dope_sheet.nudge_frames(&mut timeline.tracks, 5, timeline.playback.frame_rate); }
                });
                let (t_start, t_end) = timeline.visible_range;
                let time_span = (t_end - t_start).max(0.001);
                egui::ScrollArea::vertical().id_source("dope_sheet_scroll").max_height(200.0).show(ui, |ui| {
                    for (ti, track) in timeline.tracks.iter_mut().enumerate() {
                        ui.push_id(ti, |ui| {
                            ui.horizontal(|ui| {
                                let lbl_col = if track.muted { egui::Color32::from_gray(80) } else { track.color };
                                ui.colored_label(lbl_col, egui::RichText::new(format!("T{}: {}", track.id, track.target.label())).size(10.0));
                                // Draw keyframe strip
                                let (strip_rect, _strip_resp) = ui.allocate_exact_size(egui::Vec2::new(200.0, 16.0), egui::Sense::click());
                                let painter = ui.painter_at(strip_rect);
                                painter.rect_filled(strip_rect, 0.0, egui::Color32::from_gray(22));
                                for (ki, kf) in track.keyframes.iter_mut().enumerate() {
                                    let tx = (kf.time - t_start) / time_span;
                                    if tx < 0.0 || tx > 1.0 { continue; }
                                    let kx = strip_rect.min.x + tx * strip_rect.width();
                                    let ky = strip_rect.center().y;
                                    let kf_color = if kf.selected { egui::Color32::YELLOW } else { track.color };
                                    painter.circle_filled(egui::Pos2::new(kx, ky), 4.0, kf_color);
                                    painter.circle_stroke(egui::Pos2::new(kx, ky), 4.0, egui::Stroke::new(1.0, egui::Color32::from_gray(80)), egui::StrokeKind::Outside);
                                    let _ = ki;
                                }
                            });
                        });
                    }
                });
            });
        }

        // Curve Editor
        if self.show_curve_editor {
            egui::CollapsingHeader::new("Curve Editor").default_open(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Fit View").clicked() {
                        self.curve_editor.fit_view(&timeline.tracks);
                    }
                    ui.checkbox(&mut self.curve_editor.show_tangent_handles, "Show Handles");
                    ui.checkbox(&mut self.curve_editor.show_all_tracks, "All Tracks");
                    ui.separator();
                    ui.label("Tangent Mode:");
                    for &mode in TangentHandleType::all() {
                        if ui.selectable_label(self.curve_editor.tangent_mode == mode, mode.label()).clicked() {
                            self.curve_editor.tangent_mode = mode;
                            self.curve_editor.apply_tangent_mode(&mut timeline.tracks, mode);
                        }
                    }
                });
                let (canvas_rect, canvas_resp) = ui.allocate_exact_size(egui::Vec2::new(400.0, 200.0), egui::Sense::drag());
                let painter = ui.painter_at(canvas_rect);
                painter.rect_filled(canvas_rect, 2.0, egui::Color32::from_gray(18));
                // Grid lines
                let grid_lines = 8;
                for i in 0..=grid_lines {
                    let gt = self.curve_editor.view_min[0] + i as f32 / grid_lines as f32 * (self.curve_editor.view_max[0] - self.curve_editor.view_min[0]);
                    let gv = self.curve_editor.view_min[1] + i as f32 / grid_lines as f32 * (self.curve_editor.view_max[1] - self.curve_editor.view_min[1]);
                    let cx = [canvas_rect.min.x, canvas_rect.min.y, canvas_rect.width(), canvas_rect.height()];
                    let px = self.curve_editor.world_to_canvas(gt, self.curve_editor.view_min[1], cx);
                    let py = self.curve_editor.world_to_canvas(self.curve_editor.view_min[0], gv, cx);
                    painter.line_segment([egui::Pos2::new(px[0], canvas_rect.min.y), egui::Pos2::new(px[0], canvas_rect.max.y)], egui::Stroke::new(0.5, egui::Color32::from_gray(32)));
                    painter.line_segment([egui::Pos2::new(canvas_rect.min.x, py[1]), egui::Pos2::new(canvas_rect.max.x, py[1])], egui::Stroke::new(0.5, egui::Color32::from_gray(32)));
                    // Time label
                    painter.text(egui::Pos2::new(px[0] + 2.0, canvas_rect.min.y + 3.0), egui::Align2::LEFT_TOP, &format!("{:.1}", gt), egui::FontId::proportional(8.0), egui::Color32::from_gray(80));
                    // Value label
                    painter.text(egui::Pos2::new(canvas_rect.min.x + 2.0, py[1] - 9.0), egui::Align2::LEFT_TOP, &format!("{:.2}", gv), egui::FontId::proportional(8.0), egui::Color32::from_gray(80));
                }
                // Draw curves
                let canvas_arr = [canvas_rect.min.x, canvas_rect.min.y, canvas_rect.width(), canvas_rect.height()];
                let samples = 120usize;
                for track in &timeline.tracks {
                    if !self.curve_editor.show_all_tracks && !self.curve_editor.selected_track_ids.contains(&track.id) { continue; }
                    if track.keyframes.len() < 2 { continue; }
                    let first_t = track.keyframes.first().unwrap().time;
                    let last_t = track.keyframes.last().unwrap().time;
                    let mut prev: Option<egui::Pos2> = None;
                    for s in 0..=samples {
                        let t = first_t + (last_t - first_t) * s as f32 / samples as f32;
                        let v = track.evaluate(t);
                        let p = self.curve_editor.world_to_canvas(t, v, canvas_arr);
                        let pt = egui::Pos2::new(p[0], p[1]);
                        if !canvas_rect.contains(pt) { prev = None; continue; }
                        if let Some(pp) = prev {
                            painter.line_segment([pp, pt], egui::Stroke::new(1.5, track.color));
                        }
                        prev = Some(pt);
                    }
                    // Draw keyframe diamonds
                    for kf in &track.keyframes {
                        let p = self.curve_editor.world_to_canvas(kf.time, kf.value, canvas_arr);
                        let kp = egui::Pos2::new(p[0], p[1]);
                        if !canvas_rect.contains(kp) { continue; }
                        let kf_col = if kf.selected { egui::Color32::YELLOW } else { track.color };
                        painter.circle_filled(kp, 4.0, kf_col);
                        painter.circle_stroke(kp, 4.0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Outside);
                        // Tangent handles
                        if self.curve_editor.show_tangent_handles && kf.selected {
                            let handle_len = 20.0;
                            let in_angle = kf.tangent_in.atan();
                            let out_angle = kf.tangent_out.atan();
                            let in_h = egui::Pos2::new(kp.x - in_angle.cos() * handle_len, kp.y + in_angle.sin() * handle_len);
                            let out_h = egui::Pos2::new(kp.x + out_angle.cos() * handle_len, kp.y - out_angle.sin() * handle_len);
                            painter.line_segment([kp, in_h], egui::Stroke::new(1.0, egui::Color32::from_gray(150)));
                            painter.line_segment([kp, out_h], egui::Stroke::new(1.0, egui::Color32::from_gray(150)));
                            painter.circle_filled(in_h, 3.0, egui::Color32::from_rgb(80, 200, 80));
                            painter.circle_filled(out_h, 3.0, egui::Color32::from_rgb(200, 80, 80));
                        }
                    }
                }
                // Playhead
                let ph = self.curve_editor.world_to_canvas(timeline.playback.current_time, self.curve_editor.view_min[1], canvas_arr);
                painter.line_segment([egui::Pos2::new(ph[0], canvas_rect.min.y), egui::Pos2::new(ph[0], canvas_rect.max.y)], egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 200, 50)));
                // Pan on drag
                if canvas_resp.dragged() {
                    let delta = canvas_resp.drag_delta();
                    let dt_pan = delta.x / canvas_rect.width() * (self.curve_editor.view_max[0] - self.curve_editor.view_min[0]);
                    let dv_pan = delta.y / canvas_rect.height() * (self.curve_editor.view_max[1] - self.curve_editor.view_min[1]);
                    self.curve_editor.pan(-dt_pan, dv_pan);
                }
            });
        }

        // State Machine
        if self.show_state_machine {
            egui::CollapsingHeader::new("Animation State Machine").default_open(true).show(ui, |ui| {
                self.state_machine.show_visualizer(ui);
            });
        }

        // Apply animation to scene
        apply_animation(timeline, timeline.playback.current_time, scene);
        let _ = scene;
    }
}
