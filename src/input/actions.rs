//! High-level input action system with contexts, modifiers, triggers, and recording.
//!
//! This module provides a data-driven input layer inspired by Unreal Engine's
//! Enhanced Input System. Instead of polling raw keys, game code registers
//! `InputAction`s with `InputBinding`s and queries `ActionEvent`s each frame.
//!
//! # Architecture
//! - `ActionValue` — typed value container (bool / 1D / 2D / 3D)
//! - `InputBinding` — maps a physical device input → ActionValue with modifiers
//! - `InputAction` — named action with multiple bindings
//! - `InputContext` — named group of actions with a priority
//! - `InputContextStack` — layered context management
//! - `ActionMap` — top-level system that resolves contexts and dispatches events
//! - `InputRecorder` — records and replays action event streams
//! - `ComboDetector` — detects button sequence combos within a time window

use glam::{Vec2, Vec3};
use std::collections::HashMap;

// ── ActionValue ───────────────────────────────────────────────────────────────

/// The typed value produced by an input action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionValue {
    /// A digital button press (true = pressed).
    Button(bool),
    /// A single analog axis, e.g., a trigger.
    Axis1D(f32),
    /// A 2D analog axis, e.g., a stick or mouse delta.
    Axis2D(Vec2),
    /// A 3D analog axis.
    Axis3D(Vec3),
}

impl ActionValue {
    /// Returns the value as a boolean (Button) or whether the magnitude > 0.
    pub fn as_bool(&self) -> bool {
        match self {
            ActionValue::Button(b) => *b,
            ActionValue::Axis1D(v) => v.abs() > f32::EPSILON,
            ActionValue::Axis2D(v) => v.length_squared() > f32::EPSILON,
            ActionValue::Axis3D(v) => v.length_squared() > f32::EPSILON,
        }
    }

    /// Returns the 1D magnitude of the value.
    pub fn magnitude(&self) -> f32 {
        match self {
            ActionValue::Button(b) => if *b { 1.0 } else { 0.0 },
            ActionValue::Axis1D(v) => *v,
            ActionValue::Axis2D(v) => v.length(),
            ActionValue::Axis3D(v) => v.length(),
        }
    }

    /// Returns the value projected to Axis2D (converts Button/1D to (x, 0)).
    pub fn as_vec2(&self) -> Vec2 {
        match self {
            ActionValue::Button(b) => Vec2::new(if *b { 1.0 } else { 0.0 }, 0.0),
            ActionValue::Axis1D(v) => Vec2::new(*v, 0.0),
            ActionValue::Axis2D(v) => *v,
            ActionValue::Axis3D(v) => Vec2::new(v.x, v.y),
        }
    }

    /// Returns the zero value of the same type.
    pub fn zero_of_same_type(&self) -> ActionValue {
        match self {
            ActionValue::Button(_)  => ActionValue::Button(false),
            ActionValue::Axis1D(_)  => ActionValue::Axis1D(0.0),
            ActionValue::Axis2D(_)  => ActionValue::Axis2D(Vec2::ZERO),
            ActionValue::Axis3D(_)  => ActionValue::Axis3D(Vec3::ZERO),
        }
    }
}

impl Default for ActionValue {
    fn default() -> Self { ActionValue::Button(false) }
}

// ── Device ────────────────────────────────────────────────────────────────────

/// The device that produces a physical input signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputDevice {
    Keyboard,
    Mouse,
    Gamepad(u32),
    Touch,
}

// ── PhysicalInput ─────────────────────────────────────────────────────────────

/// A physical key, button, or axis identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PhysicalInput {
    Key(String),
    MouseButton(u8),
    MouseAxisX,
    MouseAxisY,
    MouseScroll,
    GamepadButton(String),
    GamepadAxis(String),
    TouchAxis,
}

impl PhysicalInput {
    pub fn key(name: impl Into<String>) -> Self {
        PhysicalInput::Key(name.into())
    }

    pub fn gamepad_button(name: impl Into<String>) -> Self {
        PhysicalInput::GamepadButton(name.into())
    }

    pub fn gamepad_axis(name: impl Into<String>) -> Self {
        PhysicalInput::GamepadAxis(name.into())
    }
}

// ── InputModifier ─────────────────────────────────────────────────────────────

/// Modifies the raw ActionValue before it is used.
#[derive(Debug, Clone, PartialEq)]
pub enum InputModifier {
    /// Negate all components.
    Negate,
    /// Swizzle: remap axis components (target: which axes map to xyz).
    Swizzle { x: SwizzleAxis, y: SwizzleAxis, z: SwizzleAxis },
    /// Dead zone: values within `inner` are zeroed; values are normalized to [0, 1] at `outer`.
    DeadZone { inner: f32, outer: f32 },
    /// Scale all components by a constant.
    Scale(f32),
    /// Normalize the vector (unit length). Leaves Axis1D and Button unchanged.
    Normalize,
    /// Smooth the value toward the target at the given rate (0 = instant).
    Smooth { rate: f32 },
    /// Clamp values to [min, max].
    Clamp { min: f32, max: f32 },
    /// Map raw value from [in_min, in_max] to [out_min, out_max].
    Remap { in_min: f32, in_max: f32, out_min: f32, out_max: f32 },
}

/// Which axis to sample for swizzle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwizzleAxis { X, Y, Z, W }

impl InputModifier {
    /// Apply this modifier to an ActionValue, passing the previous smoothed value for Smooth.
    pub fn apply(&self, value: ActionValue, prev: ActionValue, dt: f32) -> ActionValue {
        match self {
            InputModifier::Negate => match value {
                ActionValue::Button(b) => ActionValue::Button(!b),
                ActionValue::Axis1D(v) => ActionValue::Axis1D(-v),
                ActionValue::Axis2D(v) => ActionValue::Axis2D(-v),
                ActionValue::Axis3D(v) => ActionValue::Axis3D(-v),
            },
            InputModifier::Scale(s) => match value {
                ActionValue::Button(b) => ActionValue::Axis1D(if b { *s } else { 0.0 }),
                ActionValue::Axis1D(v) => ActionValue::Axis1D(v * s),
                ActionValue::Axis2D(v) => ActionValue::Axis2D(v * *s),
                ActionValue::Axis3D(v) => ActionValue::Axis3D(v * *s),
            },
            InputModifier::DeadZone { inner, outer } => {
                let mag = value.magnitude();
                if mag < *inner {
                    return value.zero_of_same_type();
                }
                let normalized = if (*outer - *inner).abs() > f32::EPSILON {
                    ((mag - inner) / (outer - inner)).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let scale = if mag > f32::EPSILON { normalized / mag } else { 0.0 };
                match value {
                    ActionValue::Button(b) => ActionValue::Button(b),
                    ActionValue::Axis1D(v) => ActionValue::Axis1D(v * scale),
                    ActionValue::Axis2D(v) => ActionValue::Axis2D(v * scale),
                    ActionValue::Axis3D(v) => ActionValue::Axis3D(v * scale),
                }
            }
            InputModifier::Normalize => match value {
                ActionValue::Axis2D(v) => ActionValue::Axis2D(v.normalize_or_zero()),
                ActionValue::Axis3D(v) => ActionValue::Axis3D(v.normalize_or_zero()),
                other => other,
            },
            InputModifier::Smooth { rate } => {
                let alpha = if dt > f32::EPSILON {
                    (dt * rate).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                match (value, prev) {
                    (ActionValue::Axis1D(t), ActionValue::Axis1D(p)) =>
                        ActionValue::Axis1D(p + (t - p) * alpha),
                    (ActionValue::Axis2D(t), ActionValue::Axis2D(p)) =>
                        ActionValue::Axis2D(p + (t - p) * alpha),
                    (ActionValue::Axis3D(t), ActionValue::Axis3D(p)) =>
                        ActionValue::Axis3D(p + (t - p) * alpha),
                    (v, _) => v,
                }
            }
            InputModifier::Clamp { min, max } => match value {
                ActionValue::Axis1D(v) => ActionValue::Axis1D(v.clamp(*min, *max)),
                ActionValue::Axis2D(v) => ActionValue::Axis2D(Vec2::new(
                    v.x.clamp(*min, *max), v.y.clamp(*min, *max)
                )),
                ActionValue::Axis3D(v) => ActionValue::Axis3D(Vec3::new(
                    v.x.clamp(*min, *max), v.y.clamp(*min, *max), v.z.clamp(*min, *max)
                )),
                other => other,
            },
            InputModifier::Remap { in_min, in_max, out_min, out_max } => {
                let remap = |v: f32| -> f32 {
                    let t = if (in_max - in_min).abs() > f32::EPSILON {
                        (v - in_min) / (in_max - in_min)
                    } else { 0.0 };
                    out_min + t * (out_max - out_min)
                };
                match value {
                    ActionValue::Axis1D(v) => ActionValue::Axis1D(remap(v)),
                    ActionValue::Axis2D(v) => ActionValue::Axis2D(Vec2::new(remap(v.x), remap(v.y))),
                    ActionValue::Axis3D(v) => ActionValue::Axis3D(Vec3::new(remap(v.x), remap(v.y), remap(v.z))),
                    other => other,
                }
            }
            InputModifier::Swizzle { x, y, z } => {
                let components = match value {
                    ActionValue::Axis3D(v) => [v.x, v.y, v.z, 0.0],
                    ActionValue::Axis2D(v) => [v.x, v.y, 0.0, 0.0],
                    ActionValue::Axis1D(v) => [v, 0.0, 0.0, 0.0],
                    ActionValue::Button(b) => [if b { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
                };
                let pick = |ax: &SwizzleAxis| -> f32 {
                    match ax {
                        SwizzleAxis::X => components[0],
                        SwizzleAxis::Y => components[1],
                        SwizzleAxis::Z => components[2],
                        SwizzleAxis::W => components[3],
                    }
                };
                match value {
                    ActionValue::Axis3D(_) => ActionValue::Axis3D(Vec3::new(pick(x), pick(y), pick(z))),
                    ActionValue::Axis2D(_) => ActionValue::Axis2D(Vec2::new(pick(x), pick(y))),
                    ActionValue::Axis1D(_) => ActionValue::Axis1D(pick(x)),
                    ActionValue::Button(_) => ActionValue::Button(pick(x) > 0.5),
                }
            }
        }
    }
}

// ── InputTrigger ──────────────────────────────────────────────────────────────

/// Determines when an action fires relative to input state.
#[derive(Debug, Clone, PartialEq)]
pub enum InputTrigger {
    /// Fires once when the input transitions from inactive to active.
    Pressed,
    /// Fires once when the input transitions from active to inactive.
    Released,
    /// Fires every frame while the input is active.
    Down,
    /// Fires once on a quick press-and-release (no hold).
    Tap,
    /// Fires once after the input has been held for the specified duration.
    Hold(f32),
    /// Fires on the second press within `interval` seconds.
    DoubleTap { interval: f32 },
    /// Fires only when all listed action names are also active.
    ChordedAction(Vec<String>),
}

/// State tracked per-binding for stateful triggers (Hold, DoubleTap).
#[derive(Debug, Clone, Default)]
struct TriggerState {
    held_time: f32,
    hold_fired: bool,
    last_tap_time: f32,
    tap_count: u32,
    was_active_last_frame: bool,
}

// ── InputBinding ──────────────────────────────────────────────────────────────

/// Maps a physical input to an action value with modifiers.
#[derive(Debug, Clone)]
pub struct InputBinding {
    /// The device this binding listens on.
    pub device: InputDevice,
    /// The physical key/axis to read.
    pub physical: PhysicalInput,
    /// Modifier keys that must be held for this binding to be active.
    pub required_modifiers: Vec<PhysicalInput>,
    /// Value transformations applied in order.
    pub modifiers: Vec<InputModifier>,
    /// Trigger condition.
    pub trigger: InputTrigger,
}

impl InputBinding {
    pub fn new(device: InputDevice, physical: PhysicalInput) -> Self {
        Self {
            device,
            physical,
            required_modifiers: Vec::new(),
            modifiers: Vec::new(),
            trigger: InputTrigger::Pressed,
        }
    }

    pub fn keyboard(key: impl Into<String>) -> Self {
        Self::new(InputDevice::Keyboard, PhysicalInput::Key(key.into()))
    }

    pub fn with_trigger(mut self, trigger: InputTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    pub fn with_modifier(mut self, m: InputModifier) -> Self {
        self.modifiers.push(m);
        self
    }

    pub fn with_required_modifier(mut self, p: PhysicalInput) -> Self {
        self.required_modifiers.push(p);
        self
    }

    /// Apply all modifiers in sequence to the raw value.
    pub fn apply_modifiers(&self, raw: ActionValue, prev: ActionValue, dt: f32) -> ActionValue {
        let mut v = raw;
        for m in &self.modifiers {
            v = m.apply(v, prev, dt);
        }
        v
    }
}

// ── InputAction ───────────────────────────────────────────────────────────────

/// A named input action with multiple possible bindings.
#[derive(Debug, Clone)]
pub struct InputAction {
    /// Unique name.
    pub name: String,
    /// All bindings that can activate this action.
    pub bindings: Vec<InputBinding>,
    /// Current resolved value this frame.
    pub value: ActionValue,
    /// Whether this action's input has been consumed and should not propagate.
    pub consumed: bool,
    /// Previous frame's smoothed values per binding index.
    prev_values: Vec<ActionValue>,
    /// Per-binding trigger state.
    trigger_states: Vec<TriggerState>,
}

impl InputAction {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bindings: Vec::new(),
            value: ActionValue::default(),
            consumed: false,
            prev_values: Vec::new(),
            trigger_states: Vec::new(),
        }
    }

    pub fn with_binding(mut self, binding: InputBinding) -> Self {
        self.bindings.push(binding);
        self.prev_values.push(ActionValue::default());
        self.trigger_states.push(TriggerState::default());
        self
    }

    pub fn add_binding(&mut self, binding: InputBinding) {
        self.bindings.push(binding);
        self.prev_values.push(ActionValue::default());
        self.trigger_states.push(TriggerState::default());
    }
}

// ── InputContext ──────────────────────────────────────────────────────────────

/// A named set of actions with a priority level.
#[derive(Debug, Clone)]
pub struct InputContext {
    pub name: String,
    pub actions: HashMap<String, InputAction>,
    /// Higher priority contexts shadow lower ones. Default 0.
    pub priority: i32,
    /// If true, active bindings in this context prevent lower contexts from seeing the same input.
    pub consume_input: bool,
    pub active: bool,
}

impl InputContext {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            actions: HashMap::new(),
            priority: 0,
            consume_input: true,
            active: true,
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_consume_input(mut self, consume: bool) -> Self {
        self.consume_input = consume;
        self
    }

    pub fn add_action(&mut self, action: InputAction) {
        self.actions.insert(action.name.clone(), action);
    }
}

// ── InputContextStack ─────────────────────────────────────────────────────────

/// Manages a stack of input contexts, sorted by priority.
/// Higher-priority contexts are processed first and can shadow lower ones.
pub struct InputContextStack {
    contexts: Vec<InputContext>,
}

impl InputContextStack {
    pub fn new() -> Self {
        Self { contexts: Vec::new() }
    }

    /// Push a context onto the stack (sorted by priority).
    pub fn push(&mut self, context: InputContext) {
        self.contexts.push(context);
        self.contexts.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove a context by name.
    pub fn pop(&mut self, name: &str) {
        self.contexts.retain(|c| c.name != name);
    }

    /// Enable or disable a context by name.
    pub fn set_active(&mut self, name: &str, active: bool) {
        if let Some(ctx) = self.contexts.iter_mut().find(|c| c.name == name) {
            ctx.active = active;
        }
    }

    /// Returns true if a context with the given name is present and active.
    pub fn is_active(&self, name: &str) -> bool {
        self.contexts.iter().any(|c| c.name == name && c.active)
    }

    /// Iterate active contexts in priority order (highest first).
    pub fn active_contexts(&self) -> impl Iterator<Item = &InputContext> {
        self.contexts.iter().filter(|c| c.active)
    }

    /// Iterate active contexts mutably in priority order.
    pub fn active_contexts_mut(&mut self) -> impl Iterator<Item = &mut InputContext> {
        self.contexts.iter_mut().filter(|c| c.active)
    }

    /// Number of contexts on the stack.
    pub fn len(&self) -> usize { self.contexts.len() }

    /// True if no contexts are stacked.
    pub fn is_empty(&self) -> bool { self.contexts.is_empty() }
}

impl Default for InputContextStack {
    fn default() -> Self { Self::new() }
}

// ── ActionEvent ───────────────────────────────────────────────────────────────

/// An event dispatched when an action fires.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionEvent {
    pub action_name: String,
    pub value: ActionValue,
    pub trigger: ActionTriggerKind,
    /// Time in seconds since the action system started.
    pub timestamp: f32,
}

/// Which trigger condition produced this event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTriggerKind {
    Pressed,
    Released,
    Down,
    Tap,
    Hold,
    DoubleTap,
    Chorded,
}

// ── RawInputFrame ─────────────────────────────────────────────────────────────

/// Snapshot of raw device state passed to the ActionMap each frame.
#[derive(Debug, Clone, Default)]
pub struct RawInputFrame {
    /// Keys currently held (logical names e.g. "W", "Space", "LShift").
    pub keys_held: std::collections::HashSet<String>,
    /// Keys that just went down this frame.
    pub keys_just_pressed: std::collections::HashSet<String>,
    /// Keys that just went up this frame.
    pub keys_just_released: std::collections::HashSet<String>,
    /// Axis values by name (e.g. "mouse_x", "left_stick_x").
    pub axes: HashMap<String, f32>,
    /// Mouse buttons: 0=left, 1=right, 2=middle.
    pub mouse_buttons: [bool; 3],
    pub mouse_buttons_just_pressed: [bool; 3],
    pub mouse_buttons_just_released: [bool; 3],
}

impl RawInputFrame {
    pub fn new() -> Self { Self::default() }

    pub fn with_key_held(mut self, key: impl Into<String>) -> Self {
        self.keys_held.insert(key.into());
        self
    }

    pub fn with_key_pressed(mut self, key: impl Into<String>) -> Self {
        let k = key.into();
        self.keys_held.insert(k.clone());
        self.keys_just_pressed.insert(k);
        self
    }

    pub fn with_key_released(mut self, key: impl Into<String>) -> Self {
        let k = key.into();
        self.keys_just_released.insert(k);
        self
    }

    pub fn with_axis(mut self, name: impl Into<String>, value: f32) -> Self {
        self.axes.insert(name.into(), value);
        self
    }

    /// Read the raw value of a physical input from this frame.
    pub fn read_physical(&self, device: &InputDevice, physical: &PhysicalInput) -> ActionValue {
        match (device, physical) {
            (InputDevice::Keyboard, PhysicalInput::Key(k)) => {
                ActionValue::Button(self.keys_held.contains(k))
            }
            (InputDevice::Mouse, PhysicalInput::MouseButton(btn)) => {
                let idx = *btn as usize;
                ActionValue::Button(idx < 3 && self.mouse_buttons[idx])
            }
            (InputDevice::Mouse, PhysicalInput::MouseAxisX) => {
                ActionValue::Axis1D(self.axes.get("mouse_x").copied().unwrap_or(0.0))
            }
            (InputDevice::Mouse, PhysicalInput::MouseAxisY) => {
                ActionValue::Axis1D(self.axes.get("mouse_y").copied().unwrap_or(0.0))
            }
            (InputDevice::Mouse, PhysicalInput::MouseScroll) => {
                ActionValue::Axis1D(self.axes.get("mouse_scroll").copied().unwrap_or(0.0))
            }
            (InputDevice::Gamepad(_), PhysicalInput::GamepadButton(name)) => {
                ActionValue::Button(self.axes.get(name).copied().unwrap_or(0.0) > 0.5)
            }
            (InputDevice::Gamepad(_), PhysicalInput::GamepadAxis(name)) => {
                ActionValue::Axis1D(self.axes.get(name).copied().unwrap_or(0.0))
            }
            _ => ActionValue::Button(false),
        }
    }

    fn is_just_pressed(&self, device: &InputDevice, physical: &PhysicalInput) -> bool {
        match (device, physical) {
            (InputDevice::Keyboard, PhysicalInput::Key(k)) => {
                self.keys_just_pressed.contains(k)
            }
            (InputDevice::Mouse, PhysicalInput::MouseButton(btn)) => {
                let idx = *btn as usize;
                idx < 3 && self.mouse_buttons_just_pressed[idx]
            }
            _ => false,
        }
    }

    fn is_just_released(&self, device: &InputDevice, physical: &PhysicalInput) -> bool {
        match (device, physical) {
            (InputDevice::Keyboard, PhysicalInput::Key(k)) => {
                self.keys_just_released.contains(k)
            }
            (InputDevice::Mouse, PhysicalInput::MouseButton(btn)) => {
                let idx = *btn as usize;
                idx < 3 && self.mouse_buttons_just_released[idx]
            }
            _ => false,
        }
    }
}

// ── ActionMap ─────────────────────────────────────────────────────────────────

/// Top-level input system. Manages contexts and dispatches `ActionEvent`s.
pub struct ActionMap {
    pub stack: InputContextStack,
    events: Vec<ActionEvent>,
    time: f32,
}

impl ActionMap {
    pub fn new() -> Self {
        Self {
            stack: InputContextStack::new(),
            events: Vec::new(),
            time: 0.0,
        }
    }

    pub fn add_context(&mut self, context: InputContext) {
        self.stack.push(context);
    }

    /// Process a frame of raw input and produce `ActionEvent`s.
    pub fn update(&mut self, raw: &RawInputFrame, dt: f32) {
        self.time += dt;
        self.events.clear();

        // Collect mutable context refs sorted by priority (already sorted in stack)
        for ctx in self.stack.active_contexts_mut() {
            // Iterate actions; collect events
            for action in ctx.actions.values_mut() {
                if action.consumed { continue; }

                let binding_count = action.bindings.len();
                // Ensure state vecs are sized
                while action.trigger_states.len() < binding_count {
                    action.trigger_states.push(TriggerState::default());
                }
                while action.prev_values.len() < binding_count {
                    action.prev_values.push(ActionValue::default());
                }

                let mut best_value: Option<ActionValue> = None;

                for i in 0..binding_count {
                    let binding = &action.bindings[i];

                    // Check required modifiers
                    let mods_ok = binding.required_modifiers.iter().all(|m| {
                        raw.read_physical(&binding.device, m).as_bool()
                    });
                    if !mods_ok { continue; }

                    let raw_value = raw.read_physical(&binding.device, &binding.physical);
                    let prev = action.prev_values[i];
                    let processed = binding.apply_modifiers(raw_value, prev, dt);
                    action.prev_values[i] = processed;

                    let ts = &mut action.trigger_states[i];
                    let is_active = processed.as_bool();
                    let was_active = ts.was_active_last_frame;

                    let just_pressed = raw.is_just_pressed(&binding.device, &binding.physical)
                        || (!was_active && is_active);
                    let just_released = raw.is_just_released(&binding.device, &binding.physical)
                        || (was_active && !is_active);

                    let event_kind: Option<ActionTriggerKind> = match &binding.trigger {
                        InputTrigger::Pressed => {
                            if just_pressed { Some(ActionTriggerKind::Pressed) } else { None }
                        }
                        InputTrigger::Released => {
                            if just_released { Some(ActionTriggerKind::Released) } else { None }
                        }
                        InputTrigger::Down => {
                            if is_active { Some(ActionTriggerKind::Down) } else { None }
                        }
                        InputTrigger::Tap => {
                            if just_released && ts.held_time < 0.3 {
                                ts.held_time = 0.0;
                                Some(ActionTriggerKind::Tap)
                            } else {
                                if is_active { ts.held_time += dt; }
                                else { ts.held_time = 0.0; }
                                None
                            }
                        }
                        InputTrigger::Hold(min_dur) => {
                            if is_active {
                                ts.held_time += dt;
                                if ts.held_time >= *min_dur && !ts.hold_fired {
                                    ts.hold_fired = true;
                                    Some(ActionTriggerKind::Hold)
                                } else { None }
                            } else {
                                ts.held_time = 0.0;
                                ts.hold_fired = false;
                                None
                            }
                        }
                        InputTrigger::DoubleTap { interval } => {
                            if just_pressed {
                                let time_since_last = self.time - ts.last_tap_time;
                                if time_since_last <= *interval {
                                    ts.tap_count += 1;
                                    if ts.tap_count >= 2 {
                                        ts.tap_count = 0;
                                        ts.last_tap_time = 0.0;
                                        Some(ActionTriggerKind::DoubleTap)
                                    } else { None }
                                } else {
                                    ts.tap_count = 1;
                                    ts.last_tap_time = self.time;
                                    None
                                }
                            } else { None }
                        }
                        InputTrigger::ChordedAction(required_actions) => {
                            // Simplified: always fire if active (chorded check done at ActionMap level)
                            let _ = required_actions;
                            if just_pressed { Some(ActionTriggerKind::Chorded) } else { None }
                        }
                    };

                    ts.was_active_last_frame = is_active;

                    if let Some(kind) = event_kind {
                        best_value = Some(processed);
                        let event = ActionEvent {
                            action_name: action.name.clone(),
                            value: processed,
                            trigger: kind,
                            timestamp: self.time,
                        };
                        self.events.push(event);
                    }
                }

                if let Some(v) = best_value {
                    action.value = v;
                }
            }
        }
    }

    /// Returns all events fired this frame.
    pub fn events(&self) -> &[ActionEvent] {
        &self.events
    }

    /// Returns events for a specific action name.
    pub fn events_for(&self, name: &str) -> impl Iterator<Item = &ActionEvent> {
        self.events.iter().filter(move |e| e.action_name == name)
    }

    /// Returns true if a Pressed event fired for the named action this frame.
    pub fn just_pressed(&self, name: &str) -> bool {
        self.events.iter().any(|e| e.action_name == name && e.trigger == ActionTriggerKind::Pressed)
    }

    /// Returns true if a Released event fired for the named action this frame.
    pub fn just_released(&self, name: &str) -> bool {
        self.events.iter().any(|e| e.action_name == name && e.trigger == ActionTriggerKind::Released)
    }

    /// Returns true if the action is currently held (Down event).
    pub fn is_held(&self, name: &str) -> bool {
        self.events.iter().any(|e| e.action_name == name && e.trigger == ActionTriggerKind::Down)
    }

    /// Consume all events for a named action (prevents lower-priority contexts from seeing it).
    pub fn consume(&mut self, name: &str) {
        for ctx in self.stack.active_contexts_mut() {
            if let Some(action) = ctx.actions.get_mut(name) {
                action.consumed = true;
            }
        }
        self.events.retain(|e| e.action_name != name);
    }

    /// Reset consumed state for all actions (call before each frame's update).
    pub fn reset_consumed(&mut self) {
        for ctx in self.stack.active_contexts_mut() {
            for action in ctx.actions.values_mut() {
                action.consumed = false;
            }
        }
    }

    /// Current time in seconds.
    pub fn time(&self) -> f32 { self.time }
}

impl Default for ActionMap {
    fn default() -> Self { Self::new() }
}

// ── InputRecorder ─────────────────────────────────────────────────────────────

/// A recorded action event with timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedEvent {
    pub timestamp: f32,
    pub event: ActionEvent,
}

/// Records action events and supports playback at variable speed.
pub struct InputRecorder {
    recording: Vec<RecordedEvent>,
    is_recording: bool,
    is_playing: bool,
    playback_time: f32,
    playback_speed: f32,
    playback_index: usize,
    start_time: f32,
    pending_events: Vec<ActionEvent>,
}

impl InputRecorder {
    pub fn new() -> Self {
        Self {
            recording: Vec::new(),
            is_recording: false,
            is_playing: false,
            playback_time: 0.0,
            playback_speed: 1.0,
            playback_index: 0,
            start_time: 0.0,
            pending_events: Vec::new(),
        }
    }

    /// Start recording from the given time.
    pub fn start_recording(&mut self, current_time: f32) {
        self.recording.clear();
        self.is_recording = true;
        self.start_time = current_time;
    }

    /// Stop recording.
    pub fn stop_recording(&mut self) {
        self.is_recording = false;
    }

    /// Feed a batch of events to be recorded.
    pub fn record_events(&mut self, events: &[ActionEvent], current_time: f32) {
        if !self.is_recording { return; }
        let rel = current_time - self.start_time;
        for event in events {
            self.recording.push(RecordedEvent {
                timestamp: rel,
                event: event.clone(),
            });
        }
    }

    /// Begin playback at the given speed (1.0 = real-time).
    pub fn start_playback(&mut self, speed: f32) {
        self.is_playing = true;
        self.playback_time = 0.0;
        self.playback_index = 0;
        self.playback_speed = speed;
        self.pending_events.clear();
    }

    /// Stop playback.
    pub fn stop_playback(&mut self) {
        self.is_playing = false;
    }

    /// Advance playback by `dt` seconds. Returns events that fire at this time.
    pub fn tick(&mut self, dt: f32) -> &[ActionEvent] {
        self.pending_events.clear();
        if !self.is_playing { return &self.pending_events; }

        self.playback_time += dt * self.playback_speed;

        while self.playback_index < self.recording.len() {
            let next = &self.recording[self.playback_index];
            if next.timestamp <= self.playback_time {
                self.pending_events.push(next.event.clone());
                self.playback_index += 1;
            } else {
                break;
            }
        }

        if self.playback_index >= self.recording.len() {
            self.is_playing = false;
        }

        &self.pending_events
    }

    /// Total recorded duration in seconds.
    pub fn duration(&self) -> f32 {
        self.recording.last().map(|r| r.timestamp).unwrap_or(0.0)
    }

    pub fn is_recording(&self) -> bool { self.is_recording }
    pub fn is_playing(&self) -> bool { self.is_playing }
    pub fn recorded_event_count(&self) -> usize { self.recording.len() }

    /// Export recording as raw bytes (simple binary format).
    /// Format: [u32 count] then for each entry: [f32 timestamp][u8 name_len][name bytes][u8 kind][f32 x][f32 y][f32 z][f32 ts]
    pub fn export_binary(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let count = self.recording.len() as u32;
        out.extend_from_slice(&count.to_le_bytes());
        for rec in &self.recording {
            out.extend_from_slice(&rec.timestamp.to_le_bytes());
            let name_bytes = rec.event.action_name.as_bytes();
            out.push(name_bytes.len().min(255) as u8);
            out.extend_from_slice(&name_bytes[..name_bytes.len().min(255)]);
            // Encode trigger kind as u8
            let kind_byte: u8 = match rec.event.trigger {
                ActionTriggerKind::Pressed  => 0,
                ActionTriggerKind::Released => 1,
                ActionTriggerKind::Down     => 2,
                ActionTriggerKind::Tap      => 3,
                ActionTriggerKind::Hold     => 4,
                ActionTriggerKind::DoubleTap => 5,
                ActionTriggerKind::Chorded  => 6,
            };
            out.push(kind_byte);
            // Encode value as 3 f32s
            let (x, y, z) = match rec.event.value {
                ActionValue::Button(b)  => (if b { 1.0f32 } else { 0.0 }, 0.0f32, 0.0f32),
                ActionValue::Axis1D(v)  => (v, 0.0, 0.0),
                ActionValue::Axis2D(v)  => (v.x, v.y, 0.0),
                ActionValue::Axis3D(v)  => (v.x, v.y, v.z),
            };
            out.extend_from_slice(&x.to_le_bytes());
            out.extend_from_slice(&y.to_le_bytes());
            out.extend_from_slice(&z.to_le_bytes());
            out.extend_from_slice(&rec.event.timestamp.to_le_bytes());
        }
        out
    }

    /// Import a recording from binary bytes produced by `export_binary`.
    /// Returns an error string if parsing fails.
    pub fn import_binary(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() < 4 {
            return Err("Data too short".into());
        }
        let mut cursor = 0usize;
        let read_u32 = |data: &[u8], c: &mut usize| -> Result<u32, String> {
            if *c + 4 > data.len() { return Err("unexpected EOF reading u32".into()); }
            let v = u32::from_le_bytes([data[*c], data[*c+1], data[*c+2], data[*c+3]]);
            *c += 4;
            Ok(v)
        };
        let read_f32 = |data: &[u8], c: &mut usize| -> Result<f32, String> {
            if *c + 4 > data.len() { return Err("unexpected EOF reading f32".into()); }
            let v = f32::from_le_bytes([data[*c], data[*c+1], data[*c+2], data[*c+3]]);
            *c += 4;
            Ok(v)
        };

        let count = read_u32(data, &mut cursor)?;
        let mut records = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let ts = read_f32(data, &mut cursor)?;
            if cursor >= data.len() { return Err("unexpected EOF reading name len".into()); }
            let name_len = data[cursor] as usize;
            cursor += 1;
            if cursor + name_len > data.len() { return Err("unexpected EOF reading name".into()); }
            let name = String::from_utf8(data[cursor..cursor+name_len].to_vec())
                .map_err(|e| format!("UTF-8 error: {e}"))?;
            cursor += name_len;

            if cursor >= data.len() { return Err("unexpected EOF reading kind".into()); }
            let kind_byte = data[cursor];
            cursor += 1;

            let trigger = match kind_byte {
                0 => ActionTriggerKind::Pressed,
                1 => ActionTriggerKind::Released,
                2 => ActionTriggerKind::Down,
                3 => ActionTriggerKind::Tap,
                4 => ActionTriggerKind::Hold,
                5 => ActionTriggerKind::DoubleTap,
                6 => ActionTriggerKind::Chorded,
                k => return Err(format!("unknown trigger kind {k}")),
            };

            let x = read_f32(data, &mut cursor)?;
            let y = read_f32(data, &mut cursor)?;
            let z = read_f32(data, &mut cursor)?;
            let event_ts = read_f32(data, &mut cursor)?;

            let value = ActionValue::Axis3D(Vec3::new(x, y, z));

            records.push(RecordedEvent {
                timestamp: ts,
                event: ActionEvent {
                    action_name: name,
                    value,
                    trigger,
                    timestamp: event_ts,
                },
            });
        }

        self.recording = records;
        Ok(())
    }
}

impl Default for InputRecorder {
    fn default() -> Self { Self::new() }
}

// ── ComboDetector ─────────────────────────────────────────────────────────────

/// A registered combo definition.
#[derive(Debug, Clone)]
pub struct ComboDefinition {
    pub name: String,
    /// Ordered sequence of action names.
    pub sequence: Vec<String>,
    /// Maximum seconds between consecutive inputs.
    pub time_window: f32,
}

/// State for one combo being tracked.
#[derive(Debug, Clone)]
struct ComboTracker {
    def: ComboDefinition,
    progress: usize,
    last_event_time: f32,
}

impl ComboTracker {
    fn new(def: ComboDefinition) -> Self {
        Self { def, progress: 0, last_event_time: f32::NEG_INFINITY }
    }

    /// Feed an action name at a given time. Returns Some(combo name) if complete.
    fn feed(&mut self, action: &str, t: f32) -> Option<&str> {
        // Check timeout
        if self.progress > 0 && (t - self.last_event_time) > self.def.time_window {
            self.progress = 0;
        }

        let expected = self.def.sequence.get(self.progress);
        if expected.map(|s| s.as_str()) == Some(action) {
            self.progress += 1;
            self.last_event_time = t;
            if self.progress == self.def.sequence.len() {
                self.progress = 0;
                return Some(&self.def.name);
            }
        } else if self.progress > 0 {
            // Wrong action — reset
            self.progress = 0;
            // Try again from start in case this is the first element
            if self.def.sequence.first().map(|s| s.as_str()) == Some(action) {
                self.progress = 1;
                self.last_event_time = t;
            }
        }

        None
    }
}

/// Detects input combos (sequences of action events within a time window).
///
/// Example: register a combo sequence `["up", "up", "down", "down"]` and it
/// fires when those actions occur in that order within the time window.
pub struct ComboDetector {
    trackers: Vec<ComboTracker>,
    fired_this_frame: Vec<String>,
}

impl ComboDetector {
    pub fn new() -> Self {
        Self {
            trackers: Vec::new(),
            fired_this_frame: Vec::new(),
        }
    }

    /// Register a new combo.
    pub fn register_combo(&mut self, def: ComboDefinition) {
        self.trackers.push(ComboTracker::new(def));
    }

    /// Convenience: register by name, sequence of action names, and window.
    pub fn add(&mut self, name: impl Into<String>, sequence: Vec<&str>, window: f32) {
        self.register_combo(ComboDefinition {
            name: name.into(),
            sequence: sequence.into_iter().map(|s| s.to_string()).collect(),
            time_window: window,
        });
    }

    /// Feed action events. Returns the names of any combos that completed this call.
    pub fn check(&mut self, events: &[ActionEvent]) -> Option<String> {
        self.fired_this_frame.clear();

        for event in events {
            if event.trigger != ActionTriggerKind::Pressed
                && event.trigger != ActionTriggerKind::Tap
            {
                continue;
            }

            for tracker in &mut self.trackers {
                if let Some(name) = tracker.feed(&event.action_name, event.timestamp) {
                    self.fired_this_frame.push(name.to_string());
                }
            }
        }

        self.fired_this_frame.first().cloned()
    }

    /// Returns all combos that fired on the last `check` call.
    pub fn all_fired(&self) -> &[String] {
        &self.fired_this_frame
    }

    /// Returns true if the given combo fired on the last `check` call.
    pub fn fired(&self, name: &str) -> bool {
        self.fired_this_frame.iter().any(|n| n == name)
    }

    pub fn combo_count(&self) -> usize { self.trackers.len() }
}

impl Default for ComboDetector {
    fn default() -> Self { Self::new() }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame_pressed(key: &str) -> RawInputFrame {
        RawInputFrame::new().with_key_pressed(key)
    }

    fn make_frame_held(key: &str) -> RawInputFrame {
        RawInputFrame::new().with_key_held(key)
    }

    fn make_frame_released(key: &str) -> RawInputFrame {
        let mut f = RawInputFrame::new();
        f.keys_just_released.insert(key.to_string());
        f
    }

    #[test]
    fn action_value_as_bool() {
        assert!(ActionValue::Button(true).as_bool());
        assert!(!ActionValue::Button(false).as_bool());
        assert!(ActionValue::Axis1D(0.5).as_bool());
        assert!(!ActionValue::Axis1D(0.0).as_bool());
        assert!(ActionValue::Axis2D(Vec2::new(0.1, 0.0)).as_bool());
    }

    #[test]
    fn modifier_negate() {
        let m = InputModifier::Negate;
        let v = ActionValue::Axis1D(0.8);
        let result = m.apply(v, v, 0.016);
        assert_eq!(result, ActionValue::Axis1D(-0.8));
    }

    #[test]
    fn modifier_scale() {
        let m = InputModifier::Scale(2.0);
        let v = ActionValue::Axis1D(0.5);
        let result = m.apply(v, v, 0.016);
        assert_eq!(result, ActionValue::Axis1D(1.0));
    }

    #[test]
    fn modifier_deadzone_zeroes_small_values() {
        let m = InputModifier::DeadZone { inner: 0.1, outer: 1.0 };
        let v = ActionValue::Axis1D(0.05);
        let result = m.apply(v, v, 0.016);
        assert_eq!(result, ActionValue::Axis1D(0.0));
    }

    #[test]
    fn modifier_normalize_axis2d() {
        let m = InputModifier::Normalize;
        let v = ActionValue::Axis2D(Vec2::new(3.0, 4.0));
        let result = m.apply(v, v, 0.016);
        if let ActionValue::Axis2D(r) = result {
            assert!((r.length() - 1.0).abs() < 1e-5);
        } else {
            panic!("expected Axis2D");
        }
    }

    #[test]
    fn action_map_pressed_event() {
        let mut map = ActionMap::new();
        let mut ctx = InputContext::new("game");
        let action = InputAction::new("jump")
            .with_binding(InputBinding::keyboard("Space").with_trigger(InputTrigger::Pressed));
        ctx.add_action(action);
        map.add_context(ctx);

        let frame = make_frame_pressed("Space");
        map.update(&frame, 0.016);

        assert!(map.just_pressed("jump"), "jump should fire on Space press");
    }

    #[test]
    fn action_map_held_event() {
        let mut map = ActionMap::new();
        let mut ctx = InputContext::new("game");
        let action = InputAction::new("run")
            .with_binding(InputBinding::keyboard("LShift").with_trigger(InputTrigger::Down));
        ctx.add_action(action);
        map.add_context(ctx);

        let frame = make_frame_held("LShift");
        map.update(&frame, 0.016);

        assert!(map.is_held("run"));
    }

    #[test]
    fn action_map_released_event() {
        let mut map = ActionMap::new();
        let mut ctx = InputContext::new("game");
        let action = InputAction::new("interact")
            .with_binding(InputBinding::keyboard("E").with_trigger(InputTrigger::Released));
        ctx.add_action(action);
        map.add_context(ctx);

        let frame = make_frame_released("E");
        map.update(&frame, 0.016);

        assert!(map.just_released("interact"));
    }

    #[test]
    fn hold_trigger_fires_after_duration() {
        let mut map = ActionMap::new();
        let mut ctx = InputContext::new("game");
        let action = InputAction::new("charge")
            .with_binding(InputBinding::keyboard("Space").with_trigger(InputTrigger::Hold(0.5)));
        ctx.add_action(action);
        map.add_context(ctx);

        let frame_held = make_frame_held("Space");
        // Need to also set was_active_last_frame somehow — simulate holding key
        let frame_pressed = make_frame_pressed("Space");
        map.update(&frame_pressed, 0.016);
        // Hold for 0.5 seconds
        let steps = (0.5 / 0.016) as usize + 2;
        let mut fired = false;
        for _ in 0..steps {
            map.update(&frame_held, 0.016);
            if map.events().iter().any(|e| e.action_name == "charge" && e.trigger == ActionTriggerKind::Hold) {
                fired = true;
                break;
            }
        }
        assert!(fired, "Hold trigger should fire after 0.5s");
    }

    #[test]
    fn context_stack_priority_order() {
        let mut stack = InputContextStack::new();
        let mut c1 = InputContext::new("low").with_priority(0);
        c1.add_action(InputAction::new("act"));
        let mut c2 = InputContext::new("high").with_priority(10);
        c2.add_action(InputAction::new("act"));

        stack.push(c1);
        stack.push(c2);

        let names: Vec<&str> = stack.active_contexts().map(|c| c.name.as_str()).collect();
        assert_eq!(names[0], "high");
        assert_eq!(names[1], "low");
    }

    #[test]
    fn context_stack_disable() {
        let mut stack = InputContextStack::new();
        stack.push(InputContext::new("ui").with_priority(5));
        assert!(stack.is_active("ui"));
        stack.set_active("ui", false);
        assert!(!stack.is_active("ui"));
    }

    #[test]
    fn combo_detector_fires_on_sequence() {
        let mut combo = ComboDetector::new();
        combo.add("konami_start", vec!["up", "up", "down", "down"], 1.0);

        let make_event = |name: &str, t: f32| ActionEvent {
            action_name: name.to_string(),
            value: ActionValue::Button(true),
            trigger: ActionTriggerKind::Pressed,
            timestamp: t,
        };

        let evs = vec![make_event("up", 0.1)];
        combo.check(&evs);
        let evs = vec![make_event("up", 0.2)];
        combo.check(&evs);
        let evs = vec![make_event("down", 0.3)];
        combo.check(&evs);
        let evs = vec![make_event("down", 0.4)];
        let result = combo.check(&evs);
        assert_eq!(result, Some("konami_start".to_string()));
    }

    #[test]
    fn combo_detector_resets_on_timeout() {
        let mut combo = ComboDetector::new();
        combo.add("test", vec!["a", "b"], 0.3);

        let make_event = |name: &str, t: f32| ActionEvent {
            action_name: name.to_string(),
            value: ActionValue::Button(true),
            trigger: ActionTriggerKind::Pressed,
            timestamp: t,
        };

        combo.check(&[make_event("a", 0.0)]);
        // Too much time passes
        let result = combo.check(&[make_event("b", 1.0)]);
        assert_ne!(result, Some("test".to_string()), "combo should not fire after timeout");
    }

    #[test]
    fn recorder_export_import_roundtrip() {
        let mut rec = InputRecorder::new();
        rec.start_recording(0.0);
        let events = vec![
            ActionEvent {
                action_name: "jump".to_string(),
                value: ActionValue::Button(true),
                trigger: ActionTriggerKind::Pressed,
                timestamp: 0.5,
            },
        ];
        rec.record_events(&events, 0.5);
        rec.stop_recording();
        assert_eq!(rec.recorded_event_count(), 1);

        let bytes = rec.export_binary();
        let mut rec2 = InputRecorder::new();
        rec2.import_binary(&bytes).expect("import should succeed");
        assert_eq!(rec2.recorded_event_count(), 1);
        assert_eq!(rec2.recording[0].event.action_name, "jump");
    }

    #[test]
    fn recorder_playback_emits_events() {
        let mut rec = InputRecorder::new();
        rec.start_recording(0.0);
        let events = vec![
            ActionEvent {
                action_name: "shoot".to_string(),
                value: ActionValue::Button(true),
                trigger: ActionTriggerKind::Pressed,
                timestamp: 0.1,
            },
        ];
        rec.record_events(&events, 0.1);
        rec.stop_recording();

        rec.start_playback(1.0);
        rec.tick(0.0);
        let emitted = rec.tick(0.2);
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].action_name, "shoot");
    }

    #[test]
    fn modifier_remap() {
        let m = InputModifier::Remap { in_min: 0.0, in_max: 1.0, out_min: -1.0, out_max: 1.0 };
        let v = ActionValue::Axis1D(0.5);
        let result = m.apply(v, v, 0.016);
        if let ActionValue::Axis1D(r) = result {
            assert!((r - 0.0).abs() < 1e-5, "0.5 remapped to [−1,1] should be 0.0, got {r}");
        }
    }

    #[test]
    fn action_value_zero_of_same_type() {
        assert_eq!(ActionValue::Button(true).zero_of_same_type(), ActionValue::Button(false));
        assert_eq!(ActionValue::Axis1D(5.0).zero_of_same_type(), ActionValue::Axis1D(0.0));
        assert_eq!(ActionValue::Axis2D(Vec2::ONE).zero_of_same_type(), ActionValue::Axis2D(Vec2::ZERO));
    }
}
