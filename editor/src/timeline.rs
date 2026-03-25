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
