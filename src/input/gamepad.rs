//! Gamepad input management, dead-zone processing, trigger curves, vibration, and UI navigation.
//!
//! This module handles up to 8 simultaneously connected gamepads. It provides:
//! - `GamepadState` per gamepad with button values and axis values
//! - `GamepadManager` for connect/disconnect, just-pressed/released tracking,
//!   and rumble requests
//! - `GamepadMapping` for button remapping
//! - `StickDeadzone` with circular, square, and cross modes
//! - `TriggerCurve` for applying non-linear response curves
//! - `VibrationPattern` for programmatic rumble sequences
//! - `GamepadNavigator` for D-pad/stick UI navigation

use std::collections::HashMap;

// ── GamepadId ─────────────────────────────────────────────────────────────────

/// Unique identifier for a connected gamepad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GamepadId(pub u32);

impl GamepadId {
    pub fn new(id: u32) -> Self { Self(id) }
    pub fn raw(&self) -> u32 { self.0 }
}

// ── GamepadButton ─────────────────────────────────────────────────────────────

/// Standard gamepad buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    // Face buttons
    A,
    B,
    X,
    Y,
    // Shoulder buttons
    LeftBumper,
    RightBumper,
    // Triggers as digital buttons
    LeftTriggerButton,
    RightTriggerButton,
    // Meta
    Start,
    Select,
    // Stick click
    LeftStickButton,
    RightStickButton,
    // D-Pad
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    // Generic extra buttons
    Generic0,
    Generic1,
    Generic2,
    Generic3,
    Generic4,
    Generic5,
    Generic6,
    Generic7,
    Generic8,
    Generic9,
    Generic10,
    Generic11,
    Generic12,
    Generic13,
    Generic14,
    Generic15,
}

impl GamepadButton {
    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            GamepadButton::A               => "A",
            GamepadButton::B               => "B",
            GamepadButton::X               => "X",
            GamepadButton::Y               => "Y",
            GamepadButton::LeftBumper      => "LB",
            GamepadButton::RightBumper     => "RB",
            GamepadButton::LeftTriggerButton  => "LT",
            GamepadButton::RightTriggerButton => "RT",
            GamepadButton::Start           => "Start",
            GamepadButton::Select          => "Select",
            GamepadButton::LeftStickButton => "LSB",
            GamepadButton::RightStickButton => "RSB",
            GamepadButton::DPadUp          => "DPadUp",
            GamepadButton::DPadDown        => "DPadDown",
            GamepadButton::DPadLeft        => "DPadLeft",
            GamepadButton::DPadRight       => "DPadRight",
            GamepadButton::Generic0        => "G0",
            GamepadButton::Generic1        => "G1",
            GamepadButton::Generic2        => "G2",
            GamepadButton::Generic3        => "G3",
            GamepadButton::Generic4        => "G4",
            GamepadButton::Generic5        => "G5",
            GamepadButton::Generic6        => "G6",
            GamepadButton::Generic7        => "G7",
            GamepadButton::Generic8        => "G8",
            GamepadButton::Generic9        => "G9",
            GamepadButton::Generic10       => "G10",
            GamepadButton::Generic11       => "G11",
            GamepadButton::Generic12       => "G12",
            GamepadButton::Generic13       => "G13",
            GamepadButton::Generic14       => "G14",
            GamepadButton::Generic15       => "G15",
        }
    }

    /// All standard (non-generic) buttons.
    pub fn standard_buttons() -> &'static [GamepadButton] {
        &[
            GamepadButton::A, GamepadButton::B, GamepadButton::X, GamepadButton::Y,
            GamepadButton::LeftBumper, GamepadButton::RightBumper,
            GamepadButton::LeftTriggerButton, GamepadButton::RightTriggerButton,
            GamepadButton::Start, GamepadButton::Select,
            GamepadButton::LeftStickButton, GamepadButton::RightStickButton,
            GamepadButton::DPadUp, GamepadButton::DPadDown,
            GamepadButton::DPadLeft, GamepadButton::DPadRight,
        ]
    }
}

// ── GamepadAxis ───────────────────────────────────────────────────────────────

/// Standard gamepad analog axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
    Generic0,
    Generic1,
    Generic2,
    Generic3,
}

impl GamepadAxis {
    pub fn name(&self) -> &'static str {
        match self {
            GamepadAxis::LeftStickX   => "LeftStickX",
            GamepadAxis::LeftStickY   => "LeftStickY",
            GamepadAxis::RightStickX  => "RightStickX",
            GamepadAxis::RightStickY  => "RightStickY",
            GamepadAxis::LeftTrigger  => "LeftTrigger",
            GamepadAxis::RightTrigger => "RightTrigger",
            GamepadAxis::Generic0     => "Axis0",
            GamepadAxis::Generic1     => "Axis1",
            GamepadAxis::Generic2     => "Axis2",
            GamepadAxis::Generic3     => "Axis3",
        }
    }

    /// Returns true if this axis is a trigger (range [0, 1]).
    pub fn is_trigger(&self) -> bool {
        matches!(self, GamepadAxis::LeftTrigger | GamepadAxis::RightTrigger)
    }
}

// ── GamepadState ──────────────────────────────────────────────────────────────

/// Complete state of one gamepad.
#[derive(Debug, Clone)]
pub struct GamepadState {
    pub id: GamepadId,
    pub connected: bool,
    /// Button values in [0, 1]. Analog triggers may have intermediate values.
    pub buttons: HashMap<GamepadButton, f32>,
    /// Axis values.
    pub axes: HashMap<GamepadAxis, f32>,
    /// Current rumble (low-frequency, high-frequency) in [0, 1].
    pub rumble: (f32, f32),
}

impl GamepadState {
    pub fn new(id: GamepadId) -> Self {
        Self {
            id,
            connected: false,
            buttons: HashMap::new(),
            axes: HashMap::new(),
            rumble: (0.0, 0.0),
        }
    }

    /// Returns the value of a button (0.0 = not pressed, 1.0 = fully pressed).
    pub fn button(&self, btn: GamepadButton) -> f32 {
        self.buttons.get(&btn).copied().unwrap_or(0.0)
    }

    /// Returns true if the button is pressed (value > 0.5).
    pub fn is_pressed(&self, btn: GamepadButton) -> bool {
        self.button(btn) > 0.5
    }

    /// Returns the value of an axis.
    pub fn axis(&self, ax: GamepadAxis) -> f32 {
        self.axes.get(&ax).copied().unwrap_or(0.0)
    }

    /// Left stick as (x, y) tuple.
    pub fn left_stick(&self) -> (f32, f32) {
        (self.axis(GamepadAxis::LeftStickX), self.axis(GamepadAxis::LeftStickY))
    }

    /// Right stick as (x, y) tuple.
    pub fn right_stick(&self) -> (f32, f32) {
        (self.axis(GamepadAxis::RightStickX), self.axis(GamepadAxis::RightStickY))
    }

    /// Left trigger value [0, 1].
    pub fn left_trigger(&self) -> f32 {
        self.axis(GamepadAxis::LeftTrigger)
    }

    /// Right trigger value [0, 1].
    pub fn right_trigger(&self) -> f32 {
        self.axis(GamepadAxis::RightTrigger)
    }

    fn set_button(&mut self, btn: GamepadButton, value: f32) {
        self.buttons.insert(btn, value.clamp(0.0, 1.0));
    }

    fn set_axis(&mut self, ax: GamepadAxis, value: f32) {
        let clamped = if ax.is_trigger() {
            value.clamp(0.0, 1.0)
        } else {
            value.clamp(-1.0, 1.0)
        };
        self.axes.insert(ax, clamped);
    }
}

// ── GamepadManager ────────────────────────────────────────────────────────────

/// Maximum number of simultaneously connected gamepads.
pub const MAX_GAMEPADS: usize = 8;

/// Per-gamepad runtime tracking.
struct GamepadSlot {
    state: GamepadState,
    prev_buttons: HashMap<GamepadButton, f32>,
    rumble_remaining: f32,
}

/// Manages up to 8 gamepads, tracking connections and per-frame transitions.
pub struct GamepadManager {
    slots: HashMap<GamepadId, GamepadSlot>,
    connected_ids: Vec<GamepadId>,
}

impl GamepadManager {
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            connected_ids: Vec::new(),
        }
    }

    // ── Connection ────────────────────────────────────────────────────────────

    /// Register a new gamepad connection.
    pub fn connect(&mut self, id: GamepadId) -> bool {
        if self.slots.len() >= MAX_GAMEPADS { return false; }
        if self.slots.contains_key(&id) { return false; }

        let mut state = GamepadState::new(id);
        state.connected = true;

        self.slots.insert(id, GamepadSlot {
            state,
            prev_buttons: HashMap::new(),
            rumble_remaining: 0.0,
        });
        self.connected_ids.push(id);
        true
    }

    /// Unregister a disconnected gamepad.
    pub fn disconnect(&mut self, id: GamepadId) {
        self.slots.remove(&id);
        self.connected_ids.retain(|&i| i != id);
    }

    /// Returns true if the given gamepad is connected.
    pub fn is_connected(&self, id: GamepadId) -> bool {
        self.slots.get(&id).map(|s| s.state.connected).unwrap_or(false)
    }

    /// IDs of all connected gamepads.
    pub fn connected_ids(&self) -> &[GamepadId] {
        &self.connected_ids
    }

    pub fn connected_count(&self) -> usize { self.connected_ids.len() }

    // ── State updates ─────────────────────────────────────────────────────────

    /// Update a button value. Call once per event.
    pub fn update_button(&mut self, id: GamepadId, btn: GamepadButton, value: f32) {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.state.set_button(btn, value);
        }
    }

    /// Update an axis value. Call once per event.
    pub fn update_axis(&mut self, id: GamepadId, ax: GamepadAxis, value: f32) {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.state.set_axis(ax, value);
        }
    }

    /// Call once per frame after all events are processed.
    /// Advances previous-state tracking and decrements rumble timers.
    pub fn end_frame(&mut self, dt: f32) {
        for slot in self.slots.values_mut() {
            slot.prev_buttons = slot.state.buttons.clone();
            if slot.rumble_remaining > 0.0 {
                slot.rumble_remaining -= dt;
                if slot.rumble_remaining <= 0.0 {
                    slot.rumble_remaining = 0.0;
                    slot.state.rumble = (0.0, 0.0);
                }
            }
        }
    }

    // ── Query ─────────────────────────────────────────────────────────────────

    /// Current state of a gamepad.
    pub fn state(&self, id: GamepadId) -> Option<&GamepadState> {
        self.slots.get(&id).map(|s| &s.state)
    }

    /// Returns true if the button was just pressed this frame (rising edge).
    pub fn just_pressed(&self, id: GamepadId, btn: GamepadButton) -> bool {
        if let Some(slot) = self.slots.get(&id) {
            let prev = slot.prev_buttons.get(&btn).copied().unwrap_or(0.0);
            let cur  = slot.state.button(btn);
            prev <= 0.5 && cur > 0.5
        } else {
            false
        }
    }

    /// Returns true if the button was just released this frame (falling edge).
    pub fn just_released(&self, id: GamepadId, btn: GamepadButton) -> bool {
        if let Some(slot) = self.slots.get(&id) {
            let prev = slot.prev_buttons.get(&btn).copied().unwrap_or(0.0);
            let cur  = slot.state.button(btn);
            prev > 0.5 && cur <= 0.5
        } else {
            false
        }
    }

    /// Returns true if the button is currently held (value > 0.5).
    pub fn is_held(&self, id: GamepadId, btn: GamepadButton) -> bool {
        self.slots.get(&id).map(|s| s.state.is_pressed(btn)).unwrap_or(false)
    }

    /// Axis value for a gamepad.
    pub fn axis(&self, id: GamepadId, ax: GamepadAxis) -> f32 {
        self.slots.get(&id).map(|s| s.state.axis(ax)).unwrap_or(0.0)
    }

    // ── Rumble ────────────────────────────────────────────────────────────────

    /// Set rumble on a gamepad for a given duration.
    /// `low_hz` = low-frequency motor (0..1), `high_hz` = high-frequency motor (0..1).
    pub fn set_rumble(&mut self, id: GamepadId, low_hz: f32, high_hz: f32, duration: f32) {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.state.rumble = (low_hz.clamp(0.0, 1.0), high_hz.clamp(0.0, 1.0));
            slot.rumble_remaining = duration.max(0.0);
        }
    }

    /// Stop rumble immediately.
    pub fn stop_rumble(&mut self, id: GamepadId) {
        if let Some(slot) = self.slots.get_mut(&id) {
            slot.state.rumble = (0.0, 0.0);
            slot.rumble_remaining = 0.0;
        }
    }

    /// Current rumble values for a gamepad.
    pub fn rumble(&self, id: GamepadId) -> (f32, f32) {
        self.slots.get(&id).map(|s| s.state.rumble).unwrap_or((0.0, 0.0))
    }
}

impl Default for GamepadManager {
    fn default() -> Self { Self::new() }
}

// ── GamepadMapping ────────────────────────────────────────────────────────────

/// Button remapping from physical to logical buttons.
pub struct GamepadMapping {
    /// physical → logical
    button_map: HashMap<GamepadButton, GamepadButton>,
    /// axis swaps
    axis_map: HashMap<GamepadAxis, GamepadAxis>,
    /// Per-axis inversion flags.
    axis_invert: HashMap<GamepadAxis, bool>,
}

impl GamepadMapping {
    pub fn new() -> Self {
        Self {
            button_map: HashMap::new(),
            axis_map: HashMap::new(),
            axis_invert: HashMap::new(),
        }
    }

    /// Create an identity mapping (no remapping).
    pub fn identity() -> Self { Self::new() }

    /// Remap a physical button to a logical button.
    pub fn remap(&mut self, physical: GamepadButton, logical: GamepadButton) {
        self.button_map.insert(physical, logical);
    }

    /// Remap a physical axis to a logical axis.
    pub fn remap_axis(&mut self, physical: GamepadAxis, logical: GamepadAxis) {
        self.axis_map.insert(physical, logical);
    }

    /// Invert an axis.
    pub fn invert_axis(&mut self, axis: GamepadAxis) {
        self.axis_invert.insert(axis, true);
    }

    /// Apply this mapping to a GamepadState, returning a new mapped state.
    pub fn apply(&self, state: &GamepadState) -> GamepadState {
        let mut out = GamepadState::new(state.id);
        out.connected = state.connected;
        out.rumble = state.rumble;

        // Map buttons
        for (physical, &value) in &state.buttons {
            let logical = self.button_map.get(physical).copied().unwrap_or(*physical);
            out.buttons.insert(logical, value);
        }

        // Map axes
        for (physical, &value) in &state.axes {
            let logical = self.axis_map.get(physical).copied().unwrap_or(*physical);
            let inverted = self.axis_invert.get(&logical).copied().unwrap_or(false);
            out.axes.insert(logical, if inverted { -value } else { value });
        }

        out
    }

    /// Returns the logical button for a physical button.
    pub fn logical_button(&self, physical: GamepadButton) -> GamepadButton {
        self.button_map.get(&physical).copied().unwrap_or(physical)
    }

    /// Returns the logical axis for a physical axis.
    pub fn logical_axis(&self, physical: GamepadAxis) -> GamepadAxis {
        self.axis_map.get(&physical).copied().unwrap_or(physical)
    }
}

impl Default for GamepadMapping {
    fn default() -> Self { Self::new() }
}

// ── StickDeadzone ─────────────────────────────────────────────────────────────

/// Dead-zone processing mode for analog sticks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeadzoneMode {
    /// Circular dead zone: based on stick vector length.
    Circular,
    /// Square dead zone: independent per-axis.
    Square,
    /// Cross dead zone: only one axis at a time.
    Cross,
}

/// Applies dead-zone processing to a 2D stick input.
pub struct StickDeadzone {
    pub mode: DeadzoneMode,
    /// Inner dead zone radius (values within are zero'd).
    pub inner: f32,
    /// Outer saturation radius (values beyond are clamped to 1).
    pub outer: f32,
}

impl StickDeadzone {
    pub fn new(mode: DeadzoneMode, inner: f32, outer: f32) -> Self {
        Self {
            mode,
            inner: inner.clamp(0.0, 1.0),
            outer: outer.clamp(inner, 1.0),
        }
    }

    pub fn circular(inner: f32, outer: f32) -> Self {
        Self::new(DeadzoneMode::Circular, inner, outer)
    }

    pub fn square(inner: f32, outer: f32) -> Self {
        Self::new(DeadzoneMode::Square, inner, outer)
    }

    pub fn cross(inner: f32, outer: f32) -> Self {
        Self::new(DeadzoneMode::Cross, inner, outer)
    }

    /// Apply the dead zone to a raw (x, y) stick value. Returns processed (x, y).
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        match self.mode {
            DeadzoneMode::Circular => {
                let len = (x * x + y * y).sqrt();
                if len < self.inner { return (0.0, 0.0); }
                let range = self.outer - self.inner;
                let normalized = if range > f32::EPSILON {
                    ((len - self.inner) / range).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let scale = if len > f32::EPSILON { normalized / len } else { 0.0 };
                (x * scale, y * scale)
            }
            DeadzoneMode::Square => {
                let ax = deadzone_1d(x, self.inner, self.outer);
                let ay = deadzone_1d(y, self.inner, self.outer);
                (ax, ay)
            }
            DeadzoneMode::Cross => {
                // Dominant axis gets processed; other axis is zeroed
                let ax = x.abs();
                let ay = y.abs();
                if ax >= ay {
                    (deadzone_1d(x, self.inner, self.outer), 0.0)
                } else {
                    (0.0, deadzone_1d(y, self.inner, self.outer))
                }
            }
        }
    }
}

fn deadzone_1d(v: f32, inner: f32, outer: f32) -> f32 {
    let abs = v.abs();
    if abs < inner { return 0.0; }
    let range = outer - inner;
    let normalized = if range > f32::EPSILON {
        ((abs - inner) / range).clamp(0.0, 1.0)
    } else {
        1.0
    };
    normalized * v.signum()
}

impl Default for StickDeadzone {
    fn default() -> Self {
        Self::circular(0.1, 0.9)
    }
}

// ── TriggerCurve ──────────────────────────────────────────────────────────────

/// Response curve for analog trigger values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TriggerCurve {
    /// No transformation (identity).
    Linear,
    /// Quadratic (value²) — gentle response near zero.
    Quadratic,
    /// Cubic (value³) — even gentler near zero.
    Cubic,
    /// Step: below threshold = 0, above = 1.
    Stepped { threshold: f32 },
    /// Custom power curve.
    Power { exponent: f32 },
    /// S-curve (smoothstep).
    SmoothStep,
}

impl TriggerCurve {
    /// Apply the curve to a raw trigger value in [0, 1].
    pub fn apply(&self, raw: f32) -> f32 {
        let v = raw.clamp(0.0, 1.0);
        match self {
            TriggerCurve::Linear          => v,
            TriggerCurve::Quadratic       => v * v,
            TriggerCurve::Cubic           => v * v * v,
            TriggerCurve::Stepped { threshold } => {
                if v >= *threshold { 1.0 } else { 0.0 }
            }
            TriggerCurve::Power { exponent } => {
                v.powf(*exponent)
            }
            TriggerCurve::SmoothStep => {
                // smoothstep(0, 1, v) = 3v² - 2v³
                v * v * (3.0 - 2.0 * v)
            }
        }
    }
}

// ── VibrationPattern ──────────────────────────────────────────────────────────

/// A vibration pulse segment.
#[derive(Debug, Clone, Copy)]
pub struct VibrationPulse {
    pub duration: f32,
    pub low: f32,
    pub high: f32,
}

impl VibrationPulse {
    pub fn new(duration: f32, low: f32, high: f32) -> Self {
        Self { duration, low: low.clamp(0.0, 1.0), high: high.clamp(0.0, 1.0) }
    }

    pub fn silent(duration: f32) -> Self {
        Self::new(duration, 0.0, 0.0)
    }
}

/// A vibration pattern that produces rumble values over time.
#[derive(Debug, Clone)]
pub enum VibrationPattern {
    /// Sequence of (duration, intensity) pulses.
    Pulse(Vec<VibrationPulse>),
    /// Hold at constant intensity for a duration.
    Hold { low: f32, high: f32, duration: f32 },
    /// Fade in from 0 to target over the given duration.
    FadeIn { low_target: f32, high_target: f32, duration: f32 },
    /// Fade out from target to 0 over the given duration.
    FadeOut { low_start: f32, high_start: f32, duration: f32 },
    /// Heartbeat: two quick pulses with a pause.
    Heartbeat { intensity: f32, beat_duration: f32, pause_duration: f32, count: u32 },
}

/// Runtime state for playing back a `VibrationPattern`.
pub struct VibrationPlayer {
    pub pattern: VibrationPattern,
    elapsed: f32,
    beat_index: u32,
    done: bool,
}

impl VibrationPlayer {
    pub fn new(pattern: VibrationPattern) -> Self {
        Self { pattern, elapsed: 0.0, beat_index: 0, done: false }
    }

    pub fn is_done(&self) -> bool { self.done }

    /// Advance by `dt` seconds. Returns `Some((low, high))` or `None` when done.
    pub fn tick(&mut self, dt: f32) -> Option<(f32, f32)> {
        if self.done { return None; }
        self.elapsed += dt;

        match &self.pattern {
            VibrationPattern::Hold { low, high, duration } => {
                if self.elapsed >= *duration {
                    self.done = true;
                    return None;
                }
                Some((*low, *high))
            }
            VibrationPattern::FadeIn { low_target, high_target, duration } => {
                if self.elapsed >= *duration {
                    self.done = true;
                    return None;
                }
                let t = self.elapsed / duration;
                Some((t * low_target, t * high_target))
            }
            VibrationPattern::FadeOut { low_start, high_start, duration } => {
                if self.elapsed >= *duration {
                    self.done = true;
                    return None;
                }
                let t = 1.0 - (self.elapsed / duration);
                Some((t * low_start, t * high_start))
            }
            VibrationPattern::Pulse(pulses) => {
                // Find which pulse we're in
                let mut t = 0.0f32;
                for pulse in pulses {
                    if self.elapsed < t + pulse.duration {
                        return Some((pulse.low, pulse.high));
                    }
                    t += pulse.duration;
                }
                self.done = true;
                None
            }
            VibrationPattern::Heartbeat { intensity, beat_duration, pause_duration, count } => {
                // Pattern per beat: [0, beat_dur] bump, [beat_dur, beat_dur*2] bump, [beat_dur*2, beat_dur*2+pause] silence
                let cycle = beat_duration * 2.0 + pause_duration;
                let total = cycle * *count as f32;
                if self.elapsed >= total {
                    self.done = true;
                    return None;
                }
                let phase = self.elapsed % cycle;
                if phase < *beat_duration || (phase >= *beat_duration && phase < beat_duration * 2.0) {
                    Some((*intensity, *intensity))
                } else {
                    Some((0.0, 0.0))
                }
            }
        }
    }
}

// ── GamepadNavigator ──────────────────────────────────────────────────────────

/// Direction for UI navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Focus move event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavEvent {
    pub direction: NavDirection,
}

/// Gamepad-driven UI navigation with auto-repeat.
pub struct GamepadNavigator {
    /// Initial delay before auto-repeat begins (seconds).
    pub initial_delay: f32,
    /// Repeat rate once auto-repeat is active (seconds between events).
    pub repeat_rate: f32,
    /// Whether to wrap focus at edges.
    pub wrap: bool,
    /// Dead zone for analog stick navigation.
    pub stick_threshold: f32,

    held_direction: Option<NavDirection>,
    hold_timer: f32,
    is_repeating: bool,

    // Focus tracking: list of item IDs, current focused index.
    items: Vec<u64>,
    focused_index: usize,

    // Pending nav events this frame.
    pending_events: Vec<NavEvent>,
}

impl GamepadNavigator {
    pub fn new() -> Self {
        Self {
            initial_delay: 0.4,
            repeat_rate: 0.1,
            wrap: true,
            stick_threshold: 0.5,
            held_direction: None,
            hold_timer: 0.0,
            is_repeating: false,
            items: Vec::new(),
            focused_index: 0,
            pending_events: Vec::new(),
        }
    }

    /// Set the list of navigable item IDs.
    pub fn set_items(&mut self, items: Vec<u64>) {
        self.items = items;
        self.focused_index = 0;
    }

    pub fn add_item(&mut self, id: u64) {
        self.items.push(id);
    }

    /// Currently focused item ID.
    pub fn focused_id(&self) -> Option<u64> {
        self.items.get(self.focused_index).copied()
    }

    /// Index of the currently focused item.
    pub fn focused_index(&self) -> usize { self.focused_index }

    /// Set focus by ID. Returns true if found.
    pub fn focus(&mut self, id: u64) -> bool {
        if let Some(i) = self.items.iter().position(|&x| x == id) {
            self.focused_index = i;
            true
        } else {
            false
        }
    }

    /// Process a GamepadState and advance navigation state.
    /// Returns nav events that fired this frame.
    pub fn update(&mut self, state: &GamepadState, dt: f32) -> &[NavEvent] {
        self.pending_events.clear();

        let direction = self.read_direction(state);

        if direction != self.held_direction {
            // Direction changed
            if let Some(dir) = direction {
                self.held_direction = Some(dir);
                self.hold_timer = 0.0;
                self.is_repeating = false;
                self.fire_event(dir);
            } else {
                self.held_direction = None;
                self.hold_timer = 0.0;
                self.is_repeating = false;
            }
        } else if let Some(dir) = direction {
            // Same direction held
            self.hold_timer += dt;
            if !self.is_repeating {
                if self.hold_timer >= self.initial_delay {
                    self.is_repeating = true;
                    self.hold_timer = 0.0;
                    self.fire_event(dir);
                }
            } else {
                while self.hold_timer >= self.repeat_rate {
                    self.hold_timer -= self.repeat_rate;
                    self.fire_event(dir);
                }
            }
        }

        &self.pending_events
    }

    fn read_direction(&self, state: &GamepadState) -> Option<NavDirection> {
        // D-pad takes priority over stick
        if state.is_pressed(GamepadButton::DPadUp) { return Some(NavDirection::Up); }
        if state.is_pressed(GamepadButton::DPadDown) { return Some(NavDirection::Down); }
        if state.is_pressed(GamepadButton::DPadLeft) { return Some(NavDirection::Left); }
        if state.is_pressed(GamepadButton::DPadRight) { return Some(NavDirection::Right); }

        // Analog left stick
        let lx = state.axis(GamepadAxis::LeftStickX);
        let ly = state.axis(GamepadAxis::LeftStickY);
        let t = self.stick_threshold;

        if ly < -t { return Some(NavDirection::Up); }
        if ly >  t { return Some(NavDirection::Down); }
        if lx < -t { return Some(NavDirection::Left); }
        if lx >  t { return Some(NavDirection::Right); }

        None
    }

    fn fire_event(&mut self, dir: NavDirection) {
        self.pending_events.push(NavEvent { direction: dir });
        // Move focus
        if self.items.is_empty() { return; }
        let len = self.items.len();
        match dir {
            NavDirection::Up | NavDirection::Left => {
                if self.focused_index == 0 {
                    if self.wrap { self.focused_index = len - 1; }
                } else {
                    self.focused_index -= 1;
                }
            }
            NavDirection::Down | NavDirection::Right => {
                if self.focused_index + 1 >= len {
                    if self.wrap { self.focused_index = 0; }
                } else {
                    self.focused_index += 1;
                }
            }
        }
    }

    /// Returns pending events without advancing state.
    pub fn events(&self) -> &[NavEvent] {
        &self.pending_events
    }
}

impl Default for GamepadNavigator {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(id: u32) -> GamepadState {
        let mut s = GamepadState::new(GamepadId::new(id));
        s.connected = true;
        s
    }

    #[test]
    fn gamepad_connect_disconnect() {
        let mut mgr = GamepadManager::new();
        let id = GamepadId::new(0);
        assert!(mgr.connect(id));
        assert!(mgr.is_connected(id));
        mgr.disconnect(id);
        assert!(!mgr.is_connected(id));
    }

    #[test]
    fn gamepad_max_connections() {
        let mut mgr = GamepadManager::new();
        for i in 0..MAX_GAMEPADS as u32 {
            assert!(mgr.connect(GamepadId::new(i)));
        }
        assert!(!mgr.connect(GamepadId::new(99)), "should reject 9th gamepad");
    }

    #[test]
    fn gamepad_just_pressed() {
        let mut mgr = GamepadManager::new();
        let id = GamepadId::new(0);
        mgr.connect(id);

        mgr.end_frame(0.016); // snapshot prev state (all 0)
        mgr.update_button(id, GamepadButton::A, 1.0);

        assert!(mgr.just_pressed(id, GamepadButton::A));
        assert!(!mgr.just_released(id, GamepadButton::A));
    }

    #[test]
    fn gamepad_just_released() {
        let mut mgr = GamepadManager::new();
        let id = GamepadId::new(0);
        mgr.connect(id);

        mgr.update_button(id, GamepadButton::B, 1.0);
        mgr.end_frame(0.016);
        mgr.update_button(id, GamepadButton::B, 0.0);

        assert!(!mgr.just_pressed(id, GamepadButton::B));
        assert!(mgr.just_released(id, GamepadButton::B));
    }

    #[test]
    fn gamepad_rumble_decays() {
        let mut mgr = GamepadManager::new();
        let id = GamepadId::new(0);
        mgr.connect(id);

        mgr.set_rumble(id, 1.0, 0.5, 0.1);
        assert_eq!(mgr.rumble(id), (1.0, 0.5));

        mgr.end_frame(0.2); // longer than duration
        assert_eq!(mgr.rumble(id), (0.0, 0.0));
    }

    #[test]
    fn gamepad_mapping_remap() {
        let mut mapping = GamepadMapping::new();
        mapping.remap(GamepadButton::A, GamepadButton::B);

        let mut state = make_state(0);
        state.buttons.insert(GamepadButton::A, 1.0);

        let mapped = mapping.apply(&state);
        assert_eq!(mapped.button(GamepadButton::B), 1.0);
        assert_eq!(mapped.button(GamepadButton::A), 0.0);
    }

    #[test]
    fn gamepad_mapping_axis_invert() {
        let mut mapping = GamepadMapping::new();
        mapping.invert_axis(GamepadAxis::LeftStickY);

        let mut state = make_state(0);
        state.axes.insert(GamepadAxis::LeftStickY, 0.8);

        let mapped = mapping.apply(&state);
        assert!((mapped.axis(GamepadAxis::LeftStickY) - (-0.8)).abs() < 1e-5);
    }

    #[test]
    fn deadzone_circular_zeros_inner() {
        let dz = StickDeadzone::circular(0.2, 0.9);
        let (x, y) = dz.apply(0.1, 0.1);
        // magnitude = sqrt(0.02) ≈ 0.141 < 0.2
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn deadzone_circular_normalizes_outer() {
        let dz = StickDeadzone::circular(0.1, 0.9);
        let (x, y) = dz.apply(1.0, 0.0);
        // Should be nearly 1.0 on x
        assert!((x - 1.0).abs() < 0.01, "expected ~1.0 got {x}");
        assert!((y).abs() < 1e-5);
    }

    #[test]
    fn deadzone_square() {
        let dz = StickDeadzone::square(0.2, 1.0);
        let (x, _y) = dz.apply(0.1, 0.5);
        assert_eq!(x, 0.0, "x=0.1 should be zeroed by square deadzone inner=0.2");
    }

    #[test]
    fn trigger_curve_quadratic() {
        let c = TriggerCurve::Quadratic;
        assert!((c.apply(0.5) - 0.25).abs() < 1e-5);
        assert!((c.apply(1.0) - 1.0).abs() < 1e-5);
        assert!((c.apply(0.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn trigger_curve_stepped() {
        let c = TriggerCurve::Stepped { threshold: 0.5 };
        assert_eq!(c.apply(0.4), 0.0);
        assert_eq!(c.apply(0.5), 1.0);
        assert_eq!(c.apply(0.9), 1.0);
    }

    #[test]
    fn trigger_curve_smoothstep() {
        let c = TriggerCurve::SmoothStep;
        assert!((c.apply(0.0) - 0.0).abs() < 1e-5);
        assert!((c.apply(1.0) - 1.0).abs() < 1e-5);
        // At 0.5: smoothstep = 3(0.25) - 2(0.125) = 0.75 - 0.25 = 0.5
        assert!((c.apply(0.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn vibration_hold_pattern() {
        let pattern = VibrationPattern::Hold { low: 0.8, high: 0.4, duration: 0.5 };
        let mut player = VibrationPlayer::new(pattern);

        let r = player.tick(0.1);
        assert_eq!(r, Some((0.8, 0.4)));

        let r2 = player.tick(0.5);
        assert_eq!(r2, None);
        assert!(player.is_done());
    }

    #[test]
    fn vibration_fade_in() {
        let pattern = VibrationPattern::FadeIn { low_target: 1.0, high_target: 1.0, duration: 1.0 };
        let mut player = VibrationPlayer::new(pattern);

        let r = player.tick(0.5);
        // After 0.5s elapsed: t = 0.5
        assert!(r.is_some());
        let (low, _high) = r.unwrap();
        assert!((low - 0.5).abs() < 0.05, "expected ~0.5 got {low}");
    }

    #[test]
    fn navigator_dpad_moves_focus() {
        let mut nav = GamepadNavigator::new();
        nav.set_items(vec![1, 2, 3]);

        let mut state = make_state(0);
        state.buttons.insert(GamepadButton::DPadDown, 1.0);

        let events = nav.update(&state, 0.016);
        assert!(!events.is_empty());
        assert_eq!(nav.focused_index(), 1);
    }

    #[test]
    fn navigator_wrap_around() {
        let mut nav = GamepadNavigator::new();
        nav.wrap = true;
        nav.set_items(vec![10, 20, 30]);

        let mut state = make_state(0);
        state.buttons.insert(GamepadButton::DPadUp, 1.0);

        nav.update(&state, 0.016); // at 0, move up → wraps to 2
        assert_eq!(nav.focused_index(), 2);
        assert_eq!(nav.focused_id(), Some(30));
    }

    #[test]
    fn navigator_auto_repeat() {
        let mut nav = GamepadNavigator::new();
        nav.initial_delay = 0.1;
        nav.repeat_rate = 0.05;
        nav.set_items(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

        let mut state = make_state(0);
        state.buttons.insert(GamepadButton::DPadDown, 1.0);

        // First update: fires once
        nav.update(&state, 0.016);
        assert_eq!(nav.focused_index(), 1);

        // Hold for initial_delay to trigger auto-repeat
        let steps = (0.1 / 0.016) as usize + 2;
        for _ in 0..steps {
            nav.update(&state, 0.016);
        }

        // Should have moved more than just the initial press
        assert!(nav.focused_index() > 1, "auto-repeat should advance focus further");
    }
}
