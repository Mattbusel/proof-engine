//! Touch gesture recognizers and gesture arena.
//!
//! This module provides a complete gesture recognition system for touch input.
//! Individual recognizers implement the `GestureRecognizer` trait and can be
//! combined in a `GestureArena` which manages competing recognizers using an
//! exclusive-winner model: the first recognizer to move from Possible → Began
//! cancels all others.
//!
//! # Supported Gestures
//! - `TapGesture`: single, double, or triple tap
//! - `SwipeGesture`: directional swipe with 8-way direction support
//! - `PinchGesture`: two-finger pinch/zoom
//! - `RotationGesture`: two-finger rotation
//! - `LongPressGesture`: hold finger in place
//! - `PanGesture`: drag with inertia
//! - `EdgeSwipeGesture`: swipe from screen edge

use glam::Vec2;
use std::collections::HashMap;

// ── TouchPhase ────────────────────────────────────────────────────────────────

/// The phase of a touch event in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    /// The touch just started (finger contacted screen).
    Began,
    /// The touch is moving.
    Moved,
    /// The touch ended (finger lifted).
    Ended,
    /// The touch was cancelled (e.g., phone call interrupted).
    Cancelled,
}

// ── TouchPoint ────────────────────────────────────────────────────────────────

/// A single touch point on the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    /// Unique identifier for this touch (stable across Began/Moved/Ended).
    pub id: u64,
    /// Position in screen pixels (top-left origin).
    pub position: Vec2,
    /// Pressure [0.0, 1.0] (1.0 if device doesn't support pressure).
    pub pressure: f32,
    /// Current phase.
    pub phase: TouchPhase,
}

impl TouchPoint {
    pub fn new(id: u64, position: Vec2) -> Self {
        Self { id, position, pressure: 1.0, phase: TouchPhase::Began }
    }

    pub fn with_pressure(mut self, pressure: f32) -> Self {
        self.pressure = pressure.clamp(0.0, 1.0);
        self
    }

    pub fn with_phase(mut self, phase: TouchPhase) -> Self {
        self.phase = phase;
        self
    }
}

// ── GestureState ─────────────────────────────────────────────────────────────

/// The recognition state of a gesture recognizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureState {
    /// Recognition has not started or conditions are not yet met.
    Possible,
    /// The gesture has been recognized and just started.
    Began,
    /// The gesture is ongoing and its parameters have changed.
    Changed,
    /// The gesture recognition succeeded and is complete.
    Ended,
    /// The gesture was cancelled externally (e.g., arena cancelled it).
    Cancelled,
    /// Recognition failed — conditions cannot be met with current touches.
    Failed,
}

impl GestureState {
    /// Returns true if the gesture is currently active (Began or Changed).
    pub fn is_active(self) -> bool {
        matches!(self, GestureState::Began | GestureState::Changed)
    }

    /// Returns true if the gesture has reached a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, GestureState::Ended | GestureState::Cancelled | GestureState::Failed)
    }
}

// ── GestureRecognizer trait ───────────────────────────────────────────────────

/// A trait for all gesture recognizers.
pub trait GestureRecognizer {
    /// Update the recognizer with the current set of touches and elapsed time.
    /// Returns the new state of this recognizer.
    fn update(&mut self, touches: &[TouchPoint], dt: f32) -> GestureState;

    /// Cancel this recognizer (called by the arena).
    fn cancel(&mut self);

    /// Reset the recognizer to its initial state.
    fn reset(&mut self);

    /// Returns the current state without advancing.
    fn state(&self) -> GestureState;

    /// Returns true if this recognizer requires exclusive control once it begins.
    fn is_exclusive(&self) -> bool { true }
}

// ── Swipe Direction ───────────────────────────────────────────────────────────

/// Possible swipe directions including 8 diagonals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
    DiagUL,
    DiagUR,
    DiagDL,
    DiagDR,
}

impl SwipeDirection {
    /// Classify a displacement vector into the closest swipe direction.
    pub fn from_delta(delta: Vec2) -> Option<SwipeDirection> {
        let len = delta.length();
        if len < f32::EPSILON {
            return None;
        }
        let norm = delta / len;
        let angle = norm.y.atan2(norm.x).to_degrees();
        // angle: 0=right, 90=up, 180/-180=left, -90=down (standard math coords)
        // But screen Y is inverted, so we flip Y
        let screen_angle = (-norm.y).atan2(norm.x).to_degrees();
        let dir = match screen_angle {
            a if a >= -22.5  && a <  22.5  => SwipeDirection::Right,
            a if a >=  22.5  && a <  67.5  => SwipeDirection::DiagUR,
            a if a >=  67.5  && a < 112.5  => SwipeDirection::Up,
            a if a >= 112.5  && a < 157.5  => SwipeDirection::DiagUL,
            a if a >= 157.5  || a < -157.5 => SwipeDirection::Left,
            a if a >= -157.5 && a < -112.5 => SwipeDirection::DiagDL,
            a if a >= -112.5 && a <  -67.5 => SwipeDirection::Down,
            _                               => SwipeDirection::DiagDR,
        };
        let _ = angle;
        Some(dir)
    }
}

// ── Edge ──────────────────────────────────────────────────────────────────────

/// Screen edge for edge swipe detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

// ── TapGesture ────────────────────────────────────────────────────────────────

/// Recognizes single, double, or triple tap gestures.
pub struct TapGesture {
    /// Number of taps required to fire (1, 2, or 3).
    pub required_taps: u32,
    /// Maximum movement allowed between touch-down and touch-up (pixels).
    pub max_movement: f32,
    /// Maximum time between consecutive taps (seconds).
    pub max_interval: f32,
    /// Maximum duration of a single tap (seconds).
    pub max_tap_duration: f32,

    state: GestureState,
    tap_count: u32,
    touch_start_pos: Vec2,
    touch_start_time: f32,
    last_tap_time: f32,
    elapsed: f32,
    active_id: Option<u64>,
}

impl TapGesture {
    pub fn new(required_taps: u32) -> Self {
        Self {
            required_taps,
            max_movement: 10.0,
            max_interval: 0.3,
            max_tap_duration: 0.5,
            state: GestureState::Possible,
            tap_count: 0,
            touch_start_pos: Vec2::ZERO,
            touch_start_time: 0.0,
            last_tap_time: f32::NEG_INFINITY,
            elapsed: 0.0,
            active_id: None,
        }
    }

    pub fn single() -> Self { Self::new(1) }
    pub fn double() -> Self { Self::new(2) }
    pub fn triple() -> Self { Self::new(3) }

    pub fn with_max_movement(mut self, px: f32) -> Self {
        self.max_movement = px;
        self
    }

    pub fn with_max_interval(mut self, secs: f32) -> Self {
        self.max_interval = secs;
        self
    }

    /// Returns the number of taps detected so far this sequence.
    pub fn tap_count(&self) -> u32 { self.tap_count }

    /// Position of the last recognized tap.
    pub fn tap_position(&self) -> Vec2 { self.touch_start_pos }
}

impl GestureRecognizer for TapGesture {
    fn update(&mut self, touches: &[TouchPoint], dt: f32) -> GestureState {
        self.elapsed += dt;

        if self.state == GestureState::Ended || self.state == GestureState::Failed {
            self.state = GestureState::Possible;
            self.tap_count = 0;
            self.active_id = None;
        }

        // Check tap interval timeout
        if self.tap_count > 0
            && self.active_id.is_none()
            && (self.elapsed - self.last_tap_time) > self.max_interval
        {
            self.state = GestureState::Failed;
            self.tap_count = 0;
            return self.state;
        }

        for touch in touches {
            match touch.phase {
                TouchPhase::Began => {
                    if self.active_id.is_none() {
                        // Check interval since last tap
                        if self.tap_count > 0
                            && (self.elapsed - self.last_tap_time) > self.max_interval
                        {
                            self.tap_count = 0;
                        }
                        self.active_id = Some(touch.id);
                        self.touch_start_pos = touch.position;
                        self.touch_start_time = self.elapsed;
                    }
                }
                TouchPhase::Moved => {
                    if Some(touch.id) == self.active_id {
                        let moved = (touch.position - self.touch_start_pos).length();
                        if moved > self.max_movement {
                            self.state = GestureState::Failed;
                            self.tap_count = 0;
                            self.active_id = None;
                            return self.state;
                        }
                    }
                }
                TouchPhase::Ended => {
                    if Some(touch.id) == self.active_id {
                        let duration = self.elapsed - self.touch_start_time;
                        let moved = (touch.position - self.touch_start_pos).length();

                        if duration > self.max_tap_duration || moved > self.max_movement {
                            self.state = GestureState::Failed;
                            self.tap_count = 0;
                            self.active_id = None;
                            return self.state;
                        }

                        self.tap_count += 1;
                        self.last_tap_time = self.elapsed;
                        self.active_id = None;

                        if self.tap_count == self.required_taps {
                            self.state = GestureState::Ended;
                            return self.state;
                        }
                    }
                }
                TouchPhase::Cancelled => {
                    if Some(touch.id) == self.active_id {
                        self.state = GestureState::Cancelled;
                        self.tap_count = 0;
                        self.active_id = None;
                        return self.state;
                    }
                }
            }
        }

        self.state
    }

    fn cancel(&mut self) {
        self.state = GestureState::Cancelled;
        self.tap_count = 0;
        self.active_id = None;
    }

    fn reset(&mut self) {
        self.state = GestureState::Possible;
        self.tap_count = 0;
        self.active_id = None;
        self.elapsed = 0.0;
        self.last_tap_time = f32::NEG_INFINITY;
    }

    fn state(&self) -> GestureState { self.state }
}

// ── SwipeGesture ──────────────────────────────────────────────────────────────

/// Recognizes directional swipe gestures.
pub struct SwipeGesture {
    /// Minimum distance in pixels for a swipe to be recognized.
    pub min_distance: f32,
    /// Maximum duration in seconds for a swipe.
    pub max_duration: f32,
    /// Optional: restrict to a specific direction.
    pub required_direction: Option<SwipeDirection>,

    state: GestureState,
    direction: Option<SwipeDirection>,
    start_pos: Vec2,
    start_time: f32,
    elapsed: f32,
    active_id: Option<u64>,
    velocity: Vec2,
}

impl SwipeGesture {
    pub fn new() -> Self {
        Self {
            min_distance: 50.0,
            max_duration: 0.5,
            required_direction: None,
            state: GestureState::Possible,
            direction: None,
            start_pos: Vec2::ZERO,
            start_time: 0.0,
            elapsed: 0.0,
            active_id: None,
            velocity: Vec2::ZERO,
        }
    }

    pub fn in_direction(dir: SwipeDirection) -> Self {
        let mut s = Self::new();
        s.required_direction = Some(dir);
        s
    }

    pub fn with_min_distance(mut self, px: f32) -> Self {
        self.min_distance = px;
        self
    }

    pub fn with_max_duration(mut self, secs: f32) -> Self {
        self.max_duration = secs;
        self
    }

    /// Direction of the recognized swipe.
    pub fn direction(&self) -> Option<SwipeDirection> { self.direction }

    /// Velocity of the swipe in pixels/second.
    pub fn velocity(&self) -> Vec2 { self.velocity }
}

impl Default for SwipeGesture {
    fn default() -> Self { Self::new() }
}

impl GestureRecognizer for SwipeGesture {
    fn update(&mut self, touches: &[TouchPoint], dt: f32) -> GestureState {
        self.elapsed += dt;

        if self.state.is_terminal() {
            self.state = GestureState::Possible;
            self.active_id = None;
            self.direction = None;
        }

        for touch in touches {
            match touch.phase {
                TouchPhase::Began => {
                    if self.active_id.is_none() {
                        self.active_id = Some(touch.id);
                        self.start_pos = touch.position;
                        self.start_time = self.elapsed;
                    }
                }
                TouchPhase::Ended => {
                    if Some(touch.id) == self.active_id {
                        let delta = touch.position - self.start_pos;
                        let dist = delta.length();
                        let dur = self.elapsed - self.start_time;

                        if dist < self.min_distance || dur > self.max_duration {
                            self.state = GestureState::Failed;
                            self.active_id = None;
                            return self.state;
                        }

                        let dir = SwipeDirection::from_delta(delta);
                        if let Some(d) = dir {
                            if let Some(req) = self.required_direction {
                                if req != d {
                                    self.state = GestureState::Failed;
                                    self.active_id = None;
                                    return self.state;
                                }
                            }
                            self.direction = Some(d);
                            self.velocity = if dur > f32::EPSILON { delta / dur } else { Vec2::ZERO };
                            self.state = GestureState::Ended;
                            self.active_id = None;
                        } else {
                            self.state = GestureState::Failed;
                            self.active_id = None;
                        }
                        return self.state;
                    }
                }
                TouchPhase::Cancelled => {
                    if Some(touch.id) == self.active_id {
                        self.state = GestureState::Cancelled;
                        self.active_id = None;
                        return self.state;
                    }
                }
                TouchPhase::Moved => {}
            }
        }

        self.state
    }

    fn cancel(&mut self) {
        self.state = GestureState::Cancelled;
        self.active_id = None;
    }

    fn reset(&mut self) {
        self.state = GestureState::Possible;
        self.active_id = None;
        self.direction = None;
        self.elapsed = 0.0;
    }

    fn state(&self) -> GestureState { self.state }
}

// ── PinchGesture ──────────────────────────────────────────────────────────────

/// Recognizes two-finger pinch/zoom gesture.
pub struct PinchGesture {
    state: GestureState,
    /// Scale factor relative to the start of the gesture (1.0 = unchanged).
    pub scale: f32,
    /// Rate of scale change per second.
    pub velocity: f32,
    /// Midpoint between the two touch points.
    pub anchor: Vec2,

    touch_a: Option<u64>,
    touch_b: Option<u64>,
    pos_a: Vec2,
    pos_b: Vec2,
    initial_distance: f32,
    prev_distance: f32,
    elapsed: f32,
}

impl PinchGesture {
    pub fn new() -> Self {
        Self {
            state: GestureState::Possible,
            scale: 1.0,
            velocity: 0.0,
            anchor: Vec2::ZERO,
            touch_a: None,
            touch_b: None,
            pos_a: Vec2::ZERO,
            pos_b: Vec2::ZERO,
            initial_distance: 0.0,
            prev_distance: 0.0,
            elapsed: 0.0,
        }
    }

    fn update_from_positions(&mut self, dt: f32) {
        let dist = (self.pos_a - self.pos_b).length();
        self.anchor = (self.pos_a + self.pos_b) * 0.5;
        self.scale = if self.initial_distance > f32::EPSILON {
            dist / self.initial_distance
        } else {
            1.0
        };
        let prev = self.prev_distance;
        self.velocity = if dt > f32::EPSILON && prev > f32::EPSILON {
            (dist - prev) / dt / self.initial_distance.max(1.0)
        } else {
            0.0
        };
        self.prev_distance = dist;
    }
}

impl Default for PinchGesture {
    fn default() -> Self { Self::new() }
}

impl GestureRecognizer for PinchGesture {
    fn update(&mut self, touches: &[TouchPoint], dt: f32) -> GestureState {
        self.elapsed += dt;

        if self.state.is_terminal() {
            self.state = GestureState::Possible;
            self.touch_a = None;
            self.touch_b = None;
            self.scale = 1.0;
        }

        for touch in touches {
            match touch.phase {
                TouchPhase::Began => {
                    if self.touch_a.is_none() {
                        self.touch_a = Some(touch.id);
                        self.pos_a = touch.position;
                    } else if self.touch_b.is_none() && Some(touch.id) != self.touch_a {
                        self.touch_b = Some(touch.id);
                        self.pos_b = touch.position;
                        self.initial_distance = (self.pos_a - self.pos_b).length();
                        self.prev_distance = self.initial_distance;
                        self.state = GestureState::Began;
                    }
                }
                TouchPhase::Moved => {
                    if Some(touch.id) == self.touch_a {
                        self.pos_a = touch.position;
                    } else if Some(touch.id) == self.touch_b {
                        self.pos_b = touch.position;
                    }
                    if self.touch_a.is_some() && self.touch_b.is_some()
                        && self.state.is_active()
                    {
                        self.update_from_positions(dt);
                        self.state = GestureState::Changed;
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if Some(touch.id) == self.touch_a || Some(touch.id) == self.touch_b {
                        if self.state.is_active() {
                            self.state = if touch.phase == TouchPhase::Ended {
                                GestureState::Ended
                            } else {
                                GestureState::Cancelled
                            };
                        }
                        self.touch_a = None;
                        self.touch_b = None;
                    }
                }
            }
        }

        self.state
    }

    fn cancel(&mut self) {
        self.state = GestureState::Cancelled;
        self.touch_a = None;
        self.touch_b = None;
    }

    fn reset(&mut self) {
        self.state = GestureState::Possible;
        self.touch_a = None;
        self.touch_b = None;
        self.scale = 1.0;
        self.velocity = 0.0;
        self.elapsed = 0.0;
    }

    fn state(&self) -> GestureState { self.state }
}

// ── RotationGesture ───────────────────────────────────────────────────────────

/// Recognizes two-finger rotation gesture.
pub struct RotationGesture {
    state: GestureState,
    /// Cumulative rotation in radians (positive = counter-clockwise).
    pub rotation: f32,
    /// Angular velocity in radians/second.
    pub angular_velocity: f32,

    touch_a: Option<u64>,
    touch_b: Option<u64>,
    pos_a: Vec2,
    pos_b: Vec2,
    prev_angle: f32,
    elapsed: f32,
}

impl RotationGesture {
    pub fn new() -> Self {
        Self {
            state: GestureState::Possible,
            rotation: 0.0,
            angular_velocity: 0.0,
            touch_a: None,
            touch_b: None,
            pos_a: Vec2::ZERO,
            pos_b: Vec2::ZERO,
            prev_angle: 0.0,
            elapsed: 0.0,
        }
    }

    fn current_angle(&self) -> f32 {
        let delta = self.pos_b - self.pos_a;
        delta.y.atan2(delta.x)
    }
}

impl Default for RotationGesture {
    fn default() -> Self { Self::new() }
}

impl GestureRecognizer for RotationGesture {
    fn update(&mut self, touches: &[TouchPoint], dt: f32) -> GestureState {
        self.elapsed += dt;

        if self.state.is_terminal() {
            self.state = GestureState::Possible;
            self.touch_a = None;
            self.touch_b = None;
            self.rotation = 0.0;
        }

        for touch in touches {
            match touch.phase {
                TouchPhase::Began => {
                    if self.touch_a.is_none() {
                        self.touch_a = Some(touch.id);
                        self.pos_a = touch.position;
                    } else if self.touch_b.is_none() && Some(touch.id) != self.touch_a {
                        self.touch_b = Some(touch.id);
                        self.pos_b = touch.position;
                        self.prev_angle = self.current_angle();
                        self.rotation = 0.0;
                        self.state = GestureState::Began;
                    }
                }
                TouchPhase::Moved => {
                    if Some(touch.id) == self.touch_a {
                        self.pos_a = touch.position;
                    } else if Some(touch.id) == self.touch_b {
                        self.pos_b = touch.position;
                    }
                    if self.touch_a.is_some() && self.touch_b.is_some()
                        && self.state.is_active()
                    {
                        let angle = self.current_angle();
                        let mut delta = angle - self.prev_angle;
                        // Wrap delta to [-π, π]
                        while delta > std::f32::consts::PI { delta -= std::f32::consts::TAU; }
                        while delta < -std::f32::consts::PI { delta += std::f32::consts::TAU; }
                        self.rotation += delta;
                        self.angular_velocity = if dt > f32::EPSILON { delta / dt } else { 0.0 };
                        self.prev_angle = angle;
                        self.state = GestureState::Changed;
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if Some(touch.id) == self.touch_a || Some(touch.id) == self.touch_b {
                        if self.state.is_active() {
                            self.state = if touch.phase == TouchPhase::Ended {
                                GestureState::Ended
                            } else {
                                GestureState::Cancelled
                            };
                        }
                        self.touch_a = None;
                        self.touch_b = None;
                    }
                }
            }
        }

        self.state
    }

    fn cancel(&mut self) {
        self.state = GestureState::Cancelled;
        self.touch_a = None;
        self.touch_b = None;
    }

    fn reset(&mut self) {
        self.state = GestureState::Possible;
        self.touch_a = None;
        self.touch_b = None;
        self.rotation = 0.0;
        self.angular_velocity = 0.0;
        self.elapsed = 0.0;
    }

    fn state(&self) -> GestureState { self.state }
}

// ── LongPressGesture ──────────────────────────────────────────────────────────

/// Recognizes a long press (hold finger in place for a minimum duration).
pub struct LongPressGesture {
    /// Minimum time in seconds before the gesture fires.
    pub min_duration: f32,
    /// Maximum movement in pixels before the gesture is cancelled.
    pub movement_threshold: f32,

    state: GestureState,
    start_pos: Vec2,
    hold_time: f32,
    active_id: Option<u64>,
    elapsed: f32,
}

impl LongPressGesture {
    pub fn new(min_duration: f32) -> Self {
        Self {
            min_duration,
            movement_threshold: 10.0,
            state: GestureState::Possible,
            start_pos: Vec2::ZERO,
            hold_time: 0.0,
            active_id: None,
            elapsed: 0.0,
        }
    }

    pub fn with_movement_threshold(mut self, px: f32) -> Self {
        self.movement_threshold = px;
        self
    }

    /// How long the press has been held (seconds).
    pub fn hold_duration(&self) -> f32 { self.hold_time }

    /// Position where the long press is occurring.
    pub fn position(&self) -> Vec2 { self.start_pos }
}

impl GestureRecognizer for LongPressGesture {
    fn update(&mut self, touches: &[TouchPoint], dt: f32) -> GestureState {
        self.elapsed += dt;

        if self.state == GestureState::Ended || self.state == GestureState::Failed {
            self.state = GestureState::Possible;
            self.active_id = None;
            self.hold_time = 0.0;
        }

        for touch in touches {
            match touch.phase {
                TouchPhase::Began => {
                    if self.active_id.is_none() {
                        self.active_id = Some(touch.id);
                        self.start_pos = touch.position;
                        self.hold_time = 0.0;
                    }
                }
                TouchPhase::Moved => {
                    if Some(touch.id) == self.active_id {
                        let moved = (touch.position - self.start_pos).length();
                        if moved > self.movement_threshold {
                            self.state = GestureState::Failed;
                            self.active_id = None;
                            self.hold_time = 0.0;
                            return self.state;
                        }
                    }
                }
                TouchPhase::Ended => {
                    if Some(touch.id) == self.active_id {
                        if self.state == GestureState::Began || self.state == GestureState::Changed {
                            self.state = GestureState::Ended;
                        } else {
                            self.state = GestureState::Failed;
                        }
                        self.active_id = None;
                        return self.state;
                    }
                }
                TouchPhase::Cancelled => {
                    if Some(touch.id) == self.active_id {
                        self.state = GestureState::Cancelled;
                        self.active_id = None;
                        return self.state;
                    }
                }
            }
        }

        // Accumulate hold time when a touch is active
        if self.active_id.is_some() {
            self.hold_time += dt;
            if self.state == GestureState::Possible && self.hold_time >= self.min_duration {
                self.state = GestureState::Began;
            } else if self.state == GestureState::Began {
                self.state = GestureState::Changed;
            }
        }

        self.state
    }

    fn cancel(&mut self) {
        self.state = GestureState::Cancelled;
        self.active_id = None;
    }

    fn reset(&mut self) {
        self.state = GestureState::Possible;
        self.active_id = None;
        self.hold_time = 0.0;
        self.elapsed = 0.0;
    }

    fn state(&self) -> GestureState { self.state }
}

// ── PanGesture ────────────────────────────────────────────────────────────────

/// Recognizes a pan (drag) gesture with inertia after release.
pub struct PanGesture {
    /// Movement since the last update call.
    pub translation_delta: Vec2,
    /// Total translation since gesture began.
    pub translation: Vec2,
    /// Current velocity in pixels/second.
    pub velocity: Vec2,
    /// Inertia decay factor [0, 1] applied per second after release.
    pub inertia_decay: f32,
    /// Minimum distance to start recognizing.
    pub min_distance: f32,

    state: GestureState,
    active_id: Option<u64>,
    last_pos: Vec2,
    start_pos: Vec2,
    coasting: bool,
    elapsed: f32,
}

impl PanGesture {
    pub fn new() -> Self {
        Self {
            translation_delta: Vec2::ZERO,
            translation: Vec2::ZERO,
            velocity: Vec2::ZERO,
            inertia_decay: 0.85,
            min_distance: 10.0,
            state: GestureState::Possible,
            active_id: None,
            last_pos: Vec2::ZERO,
            start_pos: Vec2::ZERO,
            coasting: false,
            elapsed: 0.0,
        }
    }

    pub fn with_inertia_decay(mut self, decay: f32) -> Self {
        self.inertia_decay = decay.clamp(0.0, 1.0);
        self
    }
}

impl Default for PanGesture {
    fn default() -> Self { Self::new() }
}

impl GestureRecognizer for PanGesture {
    fn update(&mut self, touches: &[TouchPoint], dt: f32) -> GestureState {
        self.elapsed += dt;
        self.translation_delta = Vec2::ZERO;

        if self.state.is_terminal() && !self.coasting {
            self.state = GestureState::Possible;
            self.active_id = None;
            self.translation = Vec2::ZERO;
            self.velocity = Vec2::ZERO;
        }

        // Inertia coasting
        if self.coasting {
            let decay = self.inertia_decay.powf(dt * 60.0);
            self.velocity *= decay;
            let delta = self.velocity * dt;
            self.translation_delta = delta;
            self.translation += delta;
            if self.velocity.length_squared() < 0.01 {
                self.coasting = false;
                self.state = GestureState::Ended;
            } else {
                self.state = GestureState::Changed;
            }
            return self.state;
        }

        for touch in touches {
            match touch.phase {
                TouchPhase::Began => {
                    if self.active_id.is_none() {
                        self.active_id = Some(touch.id);
                        self.start_pos = touch.position;
                        self.last_pos = touch.position;
                        self.translation = Vec2::ZERO;
                        self.velocity = Vec2::ZERO;
                    }
                }
                TouchPhase::Moved => {
                    if Some(touch.id) == self.active_id {
                        let delta = touch.position - self.last_pos;
                        self.translation_delta = delta;
                        self.translation += delta;

                        if dt > f32::EPSILON {
                            // Exponential moving average for velocity
                            let inst_vel = delta / dt;
                            self.velocity = self.velocity * 0.7 + inst_vel * 0.3;
                        }

                        self.last_pos = touch.position;

                        let total = (self.last_pos - self.start_pos).length();
                        if self.state == GestureState::Possible && total >= self.min_distance {
                            self.state = GestureState::Began;
                        } else if self.state == GestureState::Began {
                            self.state = GestureState::Changed;
                        }
                    }
                }
                TouchPhase::Ended => {
                    if Some(touch.id) == self.active_id {
                        self.active_id = None;
                        if self.state.is_active() {
                            // Start inertia coasting
                            if self.velocity.length_squared() > 1.0 {
                                self.coasting = true;
                                self.state = GestureState::Changed;
                            } else {
                                self.state = GestureState::Ended;
                            }
                        } else {
                            self.state = GestureState::Failed;
                        }
                        return self.state;
                    }
                }
                TouchPhase::Cancelled => {
                    if Some(touch.id) == self.active_id {
                        self.state = GestureState::Cancelled;
                        self.active_id = None;
                        self.coasting = false;
                        return self.state;
                    }
                }
            }
        }

        self.state
    }

    fn cancel(&mut self) {
        self.state = GestureState::Cancelled;
        self.active_id = None;
        self.coasting = false;
    }

    fn reset(&mut self) {
        self.state = GestureState::Possible;
        self.active_id = None;
        self.translation = Vec2::ZERO;
        self.velocity = Vec2::ZERO;
        self.coasting = false;
        self.elapsed = 0.0;
    }

    fn state(&self) -> GestureState { self.state }
}

// ── EdgeSwipeGesture ──────────────────────────────────────────────────────────

/// Recognizes a swipe that starts from a screen edge.
pub struct EdgeSwipeGesture {
    /// Which edge to detect from.
    pub edge: Edge,
    /// Width in pixels of the edge zone where the gesture must start.
    pub edge_width: f32,
    /// Screen dimensions needed for edge detection.
    pub screen_size: Vec2,
    /// Minimum swipe distance.
    pub min_distance: f32,

    state: GestureState,
    active_id: Option<u64>,
    start_pos: Vec2,
    current_pos: Vec2,
    direction: Option<SwipeDirection>,
    elapsed: f32,
    start_time: f32,
    max_duration: f32,
}

impl EdgeSwipeGesture {
    pub fn new(edge: Edge, screen_size: Vec2) -> Self {
        Self {
            edge,
            edge_width: 20.0,
            screen_size,
            min_distance: 40.0,
            state: GestureState::Possible,
            active_id: None,
            start_pos: Vec2::ZERO,
            current_pos: Vec2::ZERO,
            direction: None,
            elapsed: 0.0,
            start_time: 0.0,
            max_duration: 0.5,
        }
    }

    pub fn with_edge_width(mut self, px: f32) -> Self {
        self.edge_width = px;
        self
    }

    fn is_in_edge_zone(&self, pos: Vec2) -> bool {
        match self.edge {
            Edge::Left   => pos.x <= self.edge_width,
            Edge::Right  => pos.x >= self.screen_size.x - self.edge_width,
            Edge::Top    => pos.y <= self.edge_width,
            Edge::Bottom => pos.y >= self.screen_size.y - self.edge_width,
        }
    }

    pub fn direction(&self) -> Option<SwipeDirection> { self.direction }
    pub fn translation(&self) -> Vec2 { self.current_pos - self.start_pos }
}

impl GestureRecognizer for EdgeSwipeGesture {
    fn update(&mut self, touches: &[TouchPoint], dt: f32) -> GestureState {
        self.elapsed += dt;

        if self.state.is_terminal() {
            self.state = GestureState::Possible;
            self.active_id = None;
            self.direction = None;
        }

        for touch in touches {
            match touch.phase {
                TouchPhase::Began => {
                    if self.active_id.is_none() && self.is_in_edge_zone(touch.position) {
                        self.active_id = Some(touch.id);
                        self.start_pos = touch.position;
                        self.current_pos = touch.position;
                        self.start_time = self.elapsed;
                    }
                }
                TouchPhase::Moved => {
                    if Some(touch.id) == self.active_id {
                        self.current_pos = touch.position;
                        let delta = self.current_pos - self.start_pos;
                        let dist = delta.length();
                        if dist >= self.min_distance && self.state == GestureState::Possible {
                            self.direction = SwipeDirection::from_delta(delta);
                            self.state = GestureState::Began;
                        } else if self.state == GestureState::Began {
                            self.state = GestureState::Changed;
                        }
                    }
                }
                TouchPhase::Ended => {
                    if Some(touch.id) == self.active_id {
                        let dur = self.elapsed - self.start_time;
                        if self.state.is_active() && dur <= self.max_duration {
                            self.state = GestureState::Ended;
                        } else {
                            self.state = GestureState::Failed;
                        }
                        self.active_id = None;
                        return self.state;
                    }
                }
                TouchPhase::Cancelled => {
                    if Some(touch.id) == self.active_id {
                        self.state = GestureState::Cancelled;
                        self.active_id = None;
                        return self.state;
                    }
                }
            }
        }

        self.state
    }

    fn cancel(&mut self) {
        self.state = GestureState::Cancelled;
        self.active_id = None;
    }

    fn reset(&mut self) {
        self.state = GestureState::Possible;
        self.active_id = None;
        self.direction = None;
        self.elapsed = 0.0;
    }

    fn state(&self) -> GestureState { self.state }
}

// ── GestureArena ─────────────────────────────────────────────────────────────

/// A named recognizer entry in the arena.
struct ArenaEntry {
    name: String,
    recognizer: Box<dyn GestureRecognizer>,
    state: GestureState,
    just_changed: bool,
}

/// Manages competing gesture recognizers with an exclusive-winner model.
///
/// When one recognizer moves to `Began`, all others are cancelled. Events
/// from the winning recognizer are then propagated exclusively until the
/// gesture ends.
pub struct GestureArena {
    entries: Vec<ArenaEntry>,
    winner: Option<usize>,
}

impl GestureArena {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            winner: None,
        }
    }

    /// Add a named recognizer to the arena.
    pub fn add(&mut self, name: impl Into<String>, recognizer: Box<dyn GestureRecognizer>) {
        self.entries.push(ArenaEntry {
            name: name.into(),
            recognizer,
            state: GestureState::Possible,
            just_changed: false,
        });
    }

    /// Update all recognizers with current touches. Returns the name of the
    /// active winner if any recognizer has claimed the gesture.
    pub fn update(&mut self, touches: &[TouchPoint], dt: f32) -> Option<&str> {
        // Clear just_changed flags
        for entry in &mut self.entries {
            entry.just_changed = false;
        }

        // If we have a winner, only update them
        if let Some(winner_idx) = self.winner {
            let entry = &mut self.entries[winner_idx];
            let new_state = entry.recognizer.update(touches, dt);
            entry.just_changed = new_state != entry.state;
            entry.state = new_state;

            if new_state.is_terminal() {
                self.winner = None;
                // Reset all others
                for (i, entry) in self.entries.iter_mut().enumerate() {
                    if i != winner_idx {
                        entry.recognizer.reset();
                        entry.state = GestureState::Possible;
                    }
                }
                return None;
            }
            return Some(&self.entries[winner_idx].name);
        }

        // No winner yet — update all
        let mut new_winner: Option<usize> = None;
        for (i, entry) in self.entries.iter_mut().enumerate() {
            let new_state = entry.recognizer.update(touches, dt);
            entry.just_changed = new_state != entry.state;
            entry.state = new_state;

            if new_state == GestureState::Began && new_winner.is_none() {
                new_winner = Some(i);
            }
        }

        if let Some(winner_idx) = new_winner {
            // Cancel all others
            for (i, entry) in self.entries.iter_mut().enumerate() {
                if i != winner_idx && entry.recognizer.is_exclusive() {
                    entry.recognizer.cancel();
                    entry.state = GestureState::Cancelled;
                }
            }
            self.winner = Some(winner_idx);
            return Some(&self.entries[winner_idx].name);
        }

        None
    }

    /// Returns the current state of a recognizer by name.
    pub fn state(&self, name: &str) -> GestureState {
        self.entries.iter()
            .find(|e| e.name == name)
            .map(|e| e.state)
            .unwrap_or(GestureState::Failed)
    }

    /// Returns true if the named gesture changed state this update.
    pub fn just_changed(&self, name: &str) -> bool {
        self.entries.iter()
            .find(|e| e.name == name)
            .map(|e| e.just_changed)
            .unwrap_or(false)
    }

    /// Clear all recognizers and reset the arena.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.winner = None;
    }

    /// Reset all recognizers to Possible state.
    pub fn reset_all(&mut self) {
        for entry in &mut self.entries {
            entry.recognizer.reset();
            entry.state = GestureState::Possible;
        }
        self.winner = None;
    }

    /// Names of all registered recognizers.
    pub fn recognizer_names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// Returns the name of the current winner if any.
    pub fn winner_name(&self) -> Option<&str> {
        self.winner.map(|i| self.entries[i].name.as_str())
    }
}

impl Default for GestureArena {
    fn default() -> Self { Self::new() }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a sequence of touch events simulating a single tap.
pub fn simulate_tap(id: u64, pos: Vec2) -> Vec<Vec<TouchPoint>> {
    vec![
        vec![TouchPoint::new(id, pos).with_phase(TouchPhase::Began)],
        vec![TouchPoint::new(id, pos).with_phase(TouchPhase::Ended)],
    ]
}

/// Build frames simulating a swipe from `start` to `end` over `steps` frames.
pub fn simulate_swipe(id: u64, start: Vec2, end: Vec2, steps: usize) -> Vec<Vec<TouchPoint>> {
    let mut frames = Vec::new();
    frames.push(vec![TouchPoint::new(id, start).with_phase(TouchPhase::Began)]);
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let pos = start.lerp(end, t);
        frames.push(vec![TouchPoint::new(id, pos).with_phase(TouchPhase::Moved)]);
    }
    frames.push(vec![TouchPoint::new(id, end).with_phase(TouchPhase::Ended)]);
    frames
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn run_frames(recognizer: &mut dyn GestureRecognizer, frames: &[Vec<TouchPoint>]) -> GestureState {
        let mut state = GestureState::Possible;
        for frame in frames {
            state = recognizer.update(frame, DT);
        }
        state
    }

    #[test]
    fn tap_single_recognizes() {
        let mut g = TapGesture::single();
        let frames = simulate_tap(1, Vec2::new(100.0, 100.0));
        let state = run_frames(&mut g, &frames);
        assert_eq!(state, GestureState::Ended);
    }

    #[test]
    fn tap_double_requires_two_taps() {
        let mut g = TapGesture::double();
        let pos = Vec2::new(50.0, 50.0);
        // First tap
        let f1 = simulate_tap(1, pos);
        for frame in &f1 { g.update(frame, DT); }
        // Second tap (same id reused)
        let f2 = simulate_tap(1, pos);
        let mut state = GestureState::Possible;
        for frame in &f2 { state = g.update(frame, DT); }
        assert_eq!(state, GestureState::Ended);
    }

    #[test]
    fn tap_fails_on_too_much_movement() {
        let mut g = TapGesture::single();
        g.max_movement = 5.0;
        let frames = vec![
            vec![TouchPoint::new(1, Vec2::new(0.0, 0.0)).with_phase(TouchPhase::Began)],
            vec![TouchPoint::new(1, Vec2::new(50.0, 0.0)).with_phase(TouchPhase::Moved)],
            vec![TouchPoint::new(1, Vec2::new(50.0, 0.0)).with_phase(TouchPhase::Ended)],
        ];
        let state = run_frames(&mut g, &frames);
        assert_eq!(state, GestureState::Failed);
    }

    #[test]
    fn swipe_right_recognized() {
        let mut g = SwipeGesture::new();
        g.min_distance = 40.0;
        let frames = simulate_swipe(1, Vec2::new(0.0, 100.0), Vec2::new(200.0, 100.0), 5);
        let state = run_frames(&mut g, &frames);
        assert_eq!(state, GestureState::Ended);
        assert_eq!(g.direction(), Some(SwipeDirection::Right));
    }

    #[test]
    fn swipe_too_short_fails() {
        let mut g = SwipeGesture::new();
        g.min_distance = 100.0;
        let frames = simulate_swipe(1, Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 3);
        let state = run_frames(&mut g, &frames);
        assert_eq!(state, GestureState::Failed);
    }

    #[test]
    fn pinch_two_fingers_recognized() {
        let mut g = PinchGesture::new();
        // Two fingers start close
        let f1 = vec![
            TouchPoint::new(1, Vec2::new(100.0, 200.0)).with_phase(TouchPhase::Began),
            TouchPoint::new(2, Vec2::new(200.0, 200.0)).with_phase(TouchPhase::Began),
        ];
        g.update(&f1, DT);
        assert_eq!(g.state(), GestureState::Began);

        // Move apart
        let f2 = vec![
            TouchPoint::new(1, Vec2::new(50.0, 200.0)).with_phase(TouchPhase::Moved),
            TouchPoint::new(2, Vec2::new(250.0, 200.0)).with_phase(TouchPhase::Moved),
        ];
        g.update(&f2, DT);
        assert!(g.scale > 1.0, "scale should increase when fingers spread");
    }

    #[test]
    fn rotation_gesture_detects_angle() {
        let mut g = RotationGesture::new();
        let f1 = vec![
            TouchPoint::new(1, Vec2::new(100.0, 100.0)).with_phase(TouchPhase::Began),
            TouchPoint::new(2, Vec2::new(200.0, 100.0)).with_phase(TouchPhase::Began),
        ];
        g.update(&f1, DT);
        assert_eq!(g.state(), GestureState::Began);

        // Rotate: move finger 2 upward
        let f2 = vec![
            TouchPoint::new(1, Vec2::new(100.0, 100.0)).with_phase(TouchPhase::Moved),
            TouchPoint::new(2, Vec2::new(150.0, 50.0)).with_phase(TouchPhase::Moved),
        ];
        g.update(&f2, DT);
        assert_ne!(g.rotation, 0.0);
    }

    #[test]
    fn long_press_fires_after_threshold() {
        let mut g = LongPressGesture::new(0.5);
        let touch = vec![TouchPoint::new(1, Vec2::new(100.0, 100.0)).with_phase(TouchPhase::Began)];
        g.update(&touch, DT);
        assert_eq!(g.state(), GestureState::Possible);

        // Hold for 0.5 seconds
        let held = vec![TouchPoint::new(1, Vec2::new(100.0, 100.0)).with_phase(TouchPhase::Moved)];
        let steps = (0.5 / DT) as usize + 2;
        let mut state = GestureState::Possible;
        for _ in 0..steps {
            state = g.update(&held, DT);
        }
        assert!(state == GestureState::Began || state == GestureState::Changed,
            "expected active state, got {:?}", state);
    }

    #[test]
    fn long_press_cancelled_on_movement() {
        let mut g = LongPressGesture::new(0.5);
        g.movement_threshold = 5.0;
        let f1 = vec![TouchPoint::new(1, Vec2::new(0.0, 0.0)).with_phase(TouchPhase::Began)];
        g.update(&f1, DT);
        let f2 = vec![TouchPoint::new(1, Vec2::new(50.0, 0.0)).with_phase(TouchPhase::Moved)];
        let state = g.update(&f2, DT);
        assert_eq!(state, GestureState::Failed);
    }

    #[test]
    fn pan_gesture_tracks_translation() {
        let mut g = PanGesture::new();
        g.min_distance = 5.0;
        let f1 = vec![TouchPoint::new(1, Vec2::new(0.0, 0.0)).with_phase(TouchPhase::Began)];
        g.update(&f1, DT);

        let f2 = vec![TouchPoint::new(1, Vec2::new(50.0, 30.0)).with_phase(TouchPhase::Moved)];
        g.update(&f2, DT);

        assert!(g.translation.x > 0.0);
        assert!(g.translation.y > 0.0);
    }

    #[test]
    fn edge_swipe_from_left_recognized() {
        let screen = Vec2::new(1920.0, 1080.0);
        let mut g = EdgeSwipeGesture::new(Edge::Left, screen);
        g.edge_width = 30.0;
        g.min_distance = 40.0;

        let f1 = vec![TouchPoint::new(1, Vec2::new(10.0, 500.0)).with_phase(TouchPhase::Began)];
        g.update(&f1, DT);

        let frames = simulate_swipe(1, Vec2::new(10.0, 500.0), Vec2::new(200.0, 500.0), 5);
        // Skip the Began frame (already sent)
        let mut state = GestureState::Possible;
        for frame in frames.iter().skip(1) {
            state = g.update(frame, DT);
        }
        assert!(state == GestureState::Ended || state == GestureState::Began
            || state == GestureState::Changed,
            "unexpected state: {:?}", state);
    }

    #[test]
    fn edge_swipe_not_from_edge_ignored() {
        let screen = Vec2::new(1920.0, 1080.0);
        let mut g = EdgeSwipeGesture::new(Edge::Left, screen);
        g.edge_width = 20.0;

        // Touch starts in the middle — not an edge touch
        let f1 = vec![TouchPoint::new(1, Vec2::new(500.0, 500.0)).with_phase(TouchPhase::Began)];
        g.update(&f1, DT);
        assert_eq!(g.active_id, None, "should not track non-edge touch");
    }

    #[test]
    fn arena_cancels_loser_on_first_winner() {
        let mut arena = GestureArena::new();
        // Two competing pan gestures
        let mut pan1 = PanGesture::new();
        pan1.min_distance = 0.1;
        let mut pan2 = PanGesture::new();
        pan2.min_distance = 0.1;
        arena.add("pan1", Box::new(pan1));
        arena.add("pan2", Box::new(pan2));

        let f_begin = vec![TouchPoint::new(1, Vec2::new(0.0, 0.0)).with_phase(TouchPhase::Began)];
        arena.update(&f_begin, DT);

        let f_move = vec![TouchPoint::new(1, Vec2::new(50.0, 0.0)).with_phase(TouchPhase::Moved)];
        let winner = arena.update(&f_move, DT);

        // One of them should be the winner
        assert!(winner.is_some(), "expected a winner after move");
    }

    #[test]
    fn arena_reset_clears_winner() {
        let mut arena = GestureArena::new();
        arena.add("tap", Box::new(TapGesture::single()));
        arena.reset_all();
        assert!(arena.winner_name().is_none());
    }

    #[test]
    fn swipe_direction_from_delta() {
        assert_eq!(SwipeDirection::from_delta(Vec2::new(100.0, 0.0)), Some(SwipeDirection::Right));
        assert_eq!(SwipeDirection::from_delta(Vec2::new(-100.0, 0.0)), Some(SwipeDirection::Left));
        assert_eq!(SwipeDirection::from_delta(Vec2::new(0.0, 100.0)), Some(SwipeDirection::Down));
        assert_eq!(SwipeDirection::from_delta(Vec2::new(0.0, -100.0)), Some(SwipeDirection::Up));
    }

    #[test]
    fn touch_point_pressure_clamped() {
        let t = TouchPoint::new(1, Vec2::ZERO).with_pressure(2.5);
        assert_eq!(t.pressure, 1.0);
        let t2 = TouchPoint::new(1, Vec2::ZERO).with_pressure(-0.5);
        assert_eq!(t2.pressure, 0.0);
    }

    #[test]
    fn gesture_state_is_terminal() {
        assert!(GestureState::Ended.is_terminal());
        assert!(GestureState::Cancelled.is_terminal());
        assert!(GestureState::Failed.is_terminal());
        assert!(!GestureState::Began.is_terminal());
        assert!(!GestureState::Changed.is_terminal());
        assert!(!GestureState::Possible.is_terminal());
    }
}
