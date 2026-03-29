
//! Input editor — action maps, binding remapper, gamepad configurator,
//! dead zone tuning, macro recorder, combo system, haptic editor, input replay.

use glam::{Vec2, Vec4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Device types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputDevice {
    Keyboard,
    Mouse,
    GamepadXbox,
    GamepadPlayStation,
    GamepadSwitch,
    GamepadGeneric,
    Joystick,
    Wheel,
    HOTAS,
    TouchScreen,
    Pen,
    Midi,
    Custom,
}

impl InputDevice {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Keyboard => "Keyboard",
            Self::Mouse => "Mouse",
            Self::GamepadXbox => "Xbox Controller",
            Self::GamepadPlayStation => "PlayStation Controller",
            Self::GamepadSwitch => "Switch Pro Controller",
            Self::GamepadGeneric => "Generic Gamepad",
            Self::Joystick => "Joystick",
            Self::Wheel => "Racing Wheel",
            Self::HOTAS => "HOTAS",
            Self::TouchScreen => "Touch Screen",
            Self::Pen => "Pen/Stylus",
            Self::Midi => "MIDI Controller",
            Self::Custom => "Custom Device",
        }
    }
    pub fn supports_rumble(&self) -> bool {
        matches!(self, Self::GamepadXbox | Self::GamepadPlayStation | Self::GamepadSwitch | Self::GamepadGeneric | Self::Wheel)
    }
    pub fn has_analog_sticks(&self) -> bool {
        matches!(self, Self::GamepadXbox | Self::GamepadPlayStation | Self::GamepadSwitch | Self::GamepadGeneric | Self::Joystick | Self::HOTAS)
    }
    pub fn has_gyro(&self) -> bool {
        matches!(self, Self::GamepadPlayStation | Self::GamepadSwitch)
    }
    pub fn has_touchpad(&self) -> bool {
        matches!(self, Self::GamepadPlayStation)
    }
}

// ---------------------------------------------------------------------------
// Key / button codes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    F13, F14, F15, F16, F17, F18, F19, F20,
    Space, Return, Escape, Backspace, Tab, CapsLock,
    LShift, RShift, LCtrl, RCtrl, LAlt, RAlt, LSuper, RSuper,
    Up, Down, Left, Right,
    Insert, Delete, Home, End, PageUp, PageDown,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4,
    Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadAdd, NumpadSub, NumpadMul, NumpadDiv, NumpadEnter, NumpadDecimal,
    NumLock, ScrollLock, PrintScreen, Pause, Menu,
    BracketLeft, BracketRight, Semicolon, Quote, Comma, Period, Slash, Backslash,
    Grave, Minus, Equals,
}

impl KeyCode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::A => "A", Self::B => "B", Self::C => "C", Self::D => "D",
            Self::E => "E", Self::F => "F", Self::G => "G", Self::H => "H",
            Self::I => "I", Self::J => "J", Self::K => "K", Self::L => "L",
            Self::M => "M", Self::N => "N", Self::O => "O", Self::P => "P",
            Self::Q => "Q", Self::R => "R", Self::S => "S", Self::T => "T",
            Self::U => "U", Self::V => "V", Self::W => "W", Self::X => "X",
            Self::Y => "Y", Self::Z => "Z",
            Self::Num0 => "0", Self::Num1 => "1", Self::Num2 => "2",
            Self::Num3 => "3", Self::Num4 => "4", Self::Num5 => "5",
            Self::Num6 => "6", Self::Num7 => "7", Self::Num8 => "8", Self::Num9 => "9",
            Self::F1 => "F1", Self::F2 => "F2", Self::F3 => "F3", Self::F4 => "F4",
            Self::F5 => "F5", Self::F6 => "F6", Self::F7 => "F7", Self::F8 => "F8",
            Self::F9 => "F9", Self::F10 => "F10", Self::F11 => "F11", Self::F12 => "F12",
            Self::F13 => "F13", Self::F14 => "F14", Self::F15 => "F15", Self::F16 => "F16",
            Self::F17 => "F17", Self::F18 => "F18", Self::F19 => "F19", Self::F20 => "F20",
            Self::Space => "Space", Self::Return => "Enter", Self::Escape => "Escape",
            Self::Backspace => "Backspace", Self::Tab => "Tab", Self::CapsLock => "Caps Lock",
            Self::LShift => "L.Shift", Self::RShift => "R.Shift",
            Self::LCtrl => "L.Ctrl", Self::RCtrl => "R.Ctrl",
            Self::LAlt => "L.Alt", Self::RAlt => "R.Alt",
            Self::LSuper => "L.Super", Self::RSuper => "R.Super",
            Self::Up => "Up", Self::Down => "Down", Self::Left => "Left", Self::Right => "Right",
            Self::Insert => "Insert", Self::Delete => "Delete",
            Self::Home => "Home", Self::End => "End",
            Self::PageUp => "Page Up", Self::PageDown => "Page Down",
            Self::Numpad0 => "Num0", Self::Numpad1 => "Num1", Self::Numpad2 => "Num2",
            Self::Numpad3 => "Num3", Self::Numpad4 => "Num4", Self::Numpad5 => "Num5",
            Self::Numpad6 => "Num6", Self::Numpad7 => "Num7", Self::Numpad8 => "Num8",
            Self::Numpad9 => "Num9",
            Self::NumpadAdd => "Num+", Self::NumpadSub => "Num-",
            Self::NumpadMul => "Num*", Self::NumpadDiv => "Num/",
            Self::NumpadEnter => "NumEnter", Self::NumpadDecimal => "Num.",
            Self::NumLock => "Num Lock", Self::ScrollLock => "Scroll Lock",
            Self::PrintScreen => "Print Screen", Self::Pause => "Pause", Self::Menu => "Menu",
            Self::BracketLeft => "[", Self::BracketRight => "]",
            Self::Semicolon => ";", Self::Quote => "'",
            Self::Comma => ",", Self::Period => ".", Self::Slash => "/",
            Self::Backslash => "\\", Self::Grave => "`",
            Self::Minus => "-", Self::Equals => "=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle, X1, X2 }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseAxis { X, Y, ScrollX, ScrollY }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadButton {
    South, East, West, North,
    LeftBumper, RightBumper,
    LeftTrigger, RightTrigger,
    LeftStickClick, RightStickClick,
    DPadUp, DPadDown, DPadLeft, DPadRight,
    Start, Select, Home, Touchpad,
    LeftStickUp, LeftStickDown, LeftStickLeft, LeftStickRight,
    RightStickUp, RightStickDown, RightStickLeft, RightStickRight,
    L4, R4, L5, R5,
}

impl GamepadButton {
    pub fn label_xbox(&self) -> &'static str {
        match self {
            Self::South => "A", Self::East => "B", Self::West => "X", Self::North => "Y",
            Self::LeftBumper => "LB", Self::RightBumper => "RB",
            Self::LeftTrigger => "LT", Self::RightTrigger => "RT",
            Self::LeftStickClick => "LS", Self::RightStickClick => "RS",
            Self::DPadUp => "D-Up", Self::DPadDown => "D-Down",
            Self::DPadLeft => "D-Left", Self::DPadRight => "D-Right",
            Self::Start => "Menu", Self::Select => "View", Self::Home => "Xbox",
            Self::Touchpad => "Share",
            Self::LeftStickUp => "LS Up", Self::LeftStickDown => "LS Down",
            Self::LeftStickLeft => "LS Left", Self::LeftStickRight => "LS Right",
            Self::RightStickUp => "RS Up", Self::RightStickDown => "RS Down",
            Self::RightStickLeft => "RS Left", Self::RightStickRight => "RS Right",
            Self::L4 => "P1", Self::R4 => "P2", Self::L5 => "P3", Self::R5 => "P4",
        }
    }
    pub fn label_ps(&self) -> &'static str {
        match self {
            Self::South => "Cross", Self::East => "Circle", Self::West => "Square", Self::North => "Triangle",
            Self::LeftBumper => "L1", Self::RightBumper => "R1",
            Self::LeftTrigger => "L2", Self::RightTrigger => "R2",
            Self::LeftStickClick => "L3", Self::RightStickClick => "R3",
            Self::Start => "Options", Self::Select => "Create", Self::Home => "PS", Self::Touchpad => "Touchpad",
            _ => self.label_xbox(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadAxis {
    LeftStickX, LeftStickY,
    RightStickX, RightStickY,
    LeftTrigger, RightTrigger,
    GyroX, GyroY, GyroZ,
    AccelX, AccelY, AccelZ,
}

impl GamepadAxis {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LeftStickX => "Left Stick X", Self::LeftStickY => "Left Stick Y",
            Self::RightStickX => "Right Stick X", Self::RightStickY => "Right Stick Y",
            Self::LeftTrigger => "Left Trigger", Self::RightTrigger => "Right Trigger",
            Self::GyroX => "Gyro X", Self::GyroY => "Gyro Y", Self::GyroZ => "Gyro Z",
            Self::AccelX => "Accel X", Self::AccelY => "Accel Y", Self::AccelZ => "Accel Z",
        }
    }
}

// ---------------------------------------------------------------------------
// Input binding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum InputBinding {
    Key(KeyCode),
    KeyCombo(Vec<KeyCode>),
    MouseButton(MouseButton),
    MouseAxis(MouseAxis, f32),
    GamepadButton(GamepadButton),
    GamepadAxis(GamepadAxis, f32),
    GamepadAxisPositive(GamepadAxis),
    GamepadAxisNegative(GamepadAxis),
    Touch(u32),
    Any(Vec<InputBinding>),
    Sequence(Vec<InputBinding>),
    Hold { binding: Box<InputBinding>, duration_secs: f32 },
    DoubleTap { binding: Box<InputBinding>, max_interval_secs: f32 },
    Chord(Box<InputBinding>, Box<InputBinding>),
}

impl InputBinding {
    pub fn label(&self) -> String {
        match self {
            Self::Key(k) => k.label().to_string(),
            Self::KeyCombo(keys) => keys.iter().map(|k| k.label()).collect::<Vec<_>>().join("+"),
            Self::MouseButton(b) => format!("Mouse{:?}", b),
            Self::MouseAxis(a, scale) => format!("Mouse{:?}*{:.1}", a, scale),
            Self::GamepadButton(b) => b.label_xbox().to_string(),
            Self::GamepadAxis(a, scale) => format!("{} *{:.1}", a.label(), scale),
            Self::GamepadAxisPositive(a) => format!("{}+", a.label()),
            Self::GamepadAxisNegative(a) => format!("{}-", a.label()),
            Self::Touch(id) => format!("Touch{}", id),
            Self::Any(bindings) => bindings.iter().map(|b| b.label()).collect::<Vec<_>>().join(" | "),
            Self::Sequence(bindings) => bindings.iter().map(|b| b.label()).collect::<Vec<_>>().join(", "),
            Self::Hold { binding, duration_secs } => format!("Hold({}, {:.1}s)", binding.label(), duration_secs),
            Self::DoubleTap { binding, .. } => format!("2x{}", binding.label()),
            Self::Chord(a, b) => format!("{}+{}", a.label(), b.label()),
        }
    }

    pub fn is_analog(&self) -> bool {
        matches!(self, Self::MouseAxis(..) | Self::GamepadAxis(..) | Self::GamepadAxisPositive(..) | Self::GamepadAxisNegative(..))
    }
}

// ---------------------------------------------------------------------------
// Action value types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionValueType { Bool, Float, Vec2, Vec3 }

impl ActionValueType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Bool => "Digital",
            Self::Float => "Axis 1D",
            Self::Vec2 => "Axis 2D",
            Self::Vec3 => "Axis 3D",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ActionValue {
    Bool(bool),
    Float(f32),
    Vec2(Vec2),
    Vec3(glam::Vec3),
}

impl ActionValue {
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(v) => *v,
            Self::Float(v) => *v != 0.0,
            Self::Vec2(v) => v.length() > 0.0,
            Self::Vec3(v) => v.length() > 0.0,
        }
    }
    pub fn as_float(&self) -> f32 {
        match self {
            Self::Bool(v) => if *v { 1.0 } else { 0.0 },
            Self::Float(v) => *v,
            Self::Vec2(v) => v.length(),
            Self::Vec3(v) => v.length(),
        }
    }
}

// ---------------------------------------------------------------------------
// Input modifiers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum InputModifier {
    DeadZone { lower: f32, upper: f32, kind: DeadZoneKind },
    Scale(f32),
    ScaleByDeltaTime,
    Negate,
    Clamp { min: f32, max: f32 },
    Normalize,
    Smooth { speed: f32 },
    Accumulate,
    RelativeInput,
    ResponseCurve { kind: ResponseCurveKind, exponent: f32 },
    SwizzleAxes(SwizzleOrder),
    InvertAxis { x: bool, y: bool, z: bool },
    SensitivityMultiplier { x: f32, y: f32 },
    AimAssist { strength: f32, radius: f32 },
    FovScale { base_fov: f32 },
    CustomCode(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeadZoneKind { Axial, Radial, Hysteresis }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponseCurveKind { Linear, Exponential, Logarithmic, SineLike, Custom }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwizzleOrder { XYZ, XZY, YXZ, YZX, ZXY, ZYX }

impl InputModifier {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DeadZone { .. } => "Dead Zone",
            Self::Scale(_) => "Scale",
            Self::ScaleByDeltaTime => "Scale by Delta Time",
            Self::Negate => "Negate",
            Self::Clamp { .. } => "Clamp",
            Self::Normalize => "Normalize",
            Self::Smooth { .. } => "Smooth",
            Self::Accumulate => "Accumulate",
            Self::RelativeInput => "Relative Input",
            Self::ResponseCurve { .. } => "Response Curve",
            Self::SwizzleAxes(_) => "Swizzle Axes",
            Self::InvertAxis { .. } => "Invert Axis",
            Self::SensitivityMultiplier { .. } => "Sensitivity",
            Self::AimAssist { .. } => "Aim Assist",
            Self::FovScale { .. } => "FOV Scale",
            Self::CustomCode(_) => "Custom",
        }
    }

    pub fn apply_float(&self, value: f32, dt: f32) -> f32 {
        match self {
            Self::DeadZone { lower, upper, kind: DeadZoneKind::Axial } => {
                let abs = value.abs();
                if abs < *lower { 0.0 }
                else if abs > *upper { value.signum() }
                else { value.signum() * (abs - lower) / (upper - lower) }
            }
            Self::Scale(s) => value * s,
            Self::ScaleByDeltaTime => value * dt,
            Self::Negate => -value,
            Self::Clamp { min, max } => value.clamp(*min, *max),
            Self::Normalize => if value.abs() > 0.001 { value.signum() } else { 0.0 },
            Self::ResponseCurve { kind: ResponseCurveKind::Exponential, exponent, .. } => {
                value.signum() * value.abs().powf(*exponent)
            }
            _ => value,
        }
    }
}

// ---------------------------------------------------------------------------
// Input triggers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum InputTrigger {
    Down,
    Pressed,
    Released,
    Hold { hold_time: f32, is_one_shot: bool },
    HoldAndRelease { hold_time: f32 },
    Tap { release_time_limit: f32 },
    Pulse { interval: f32, trigger_on_start: bool, trigger_limit: u32 },
    ChordAction { chord_action: String },
    Combo { sequence: Vec<String>, max_time_between: f32 },
}

impl InputTrigger {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Down => "Down",
            Self::Pressed => "Pressed",
            Self::Released => "Released",
            Self::Hold { .. } => "Hold",
            Self::HoldAndRelease { .. } => "Hold & Release",
            Self::Tap { .. } => "Tap",
            Self::Pulse { .. } => "Pulse",
            Self::ChordAction { .. } => "Chord Action",
            Self::Combo { .. } => "Combo",
        }
    }
}

// ---------------------------------------------------------------------------
// Input action
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InputAction {
    pub name: String,
    pub value_type: ActionValueType,
    pub bindings: Vec<ActionBinding>,
    pub consume_input: bool,
    pub consume_lower_priority: bool,
    pub description: String,
    pub category: String,
    pub icon: String,
}

#[derive(Debug, Clone)]
pub struct ActionBinding {
    pub binding: InputBinding,
    pub device: InputDevice,
    pub modifiers: Vec<InputModifier>,
    pub triggers: Vec<InputTrigger>,
    pub platform_filter: Vec<Platform>,
    pub override_context: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform { PC, XboxConsole, PlayStation, Switch, Mobile, VR, All }

impl ActionBinding {
    pub fn new(binding: InputBinding, device: InputDevice) -> Self {
        Self {
            binding,
            device,
            modifiers: Vec::new(),
            triggers: vec![InputTrigger::Pressed],
            platform_filter: vec![Platform::All],
            override_context: None,
        }
    }
    pub fn with_modifier(mut self, modifier: InputModifier) -> Self {
        self.modifiers.push(modifier);
        self
    }
    pub fn with_trigger(mut self, trigger: InputTrigger) -> Self {
        self.triggers = vec![trigger];
        self
    }
    pub fn analog_binding(binding: InputBinding, device: InputDevice) -> Self {
        Self {
            binding,
            device,
            modifiers: vec![
                InputModifier::DeadZone { lower: 0.15, upper: 1.0, kind: DeadZoneKind::Radial },
                InputModifier::ResponseCurve { kind: ResponseCurveKind::Exponential, exponent: 1.5 },
            ],
            triggers: vec![InputTrigger::Down],
            platform_filter: vec![Platform::All],
            override_context: None,
        }
    }
}

impl InputAction {
    pub fn new(name: &str, value_type: ActionValueType) -> Self {
        Self {
            name: name.to_string(),
            value_type,
            bindings: Vec::new(),
            consume_input: true,
            consume_lower_priority: false,
            description: String::new(),
            category: "General".to_string(),
            icon: String::new(),
        }
    }
    pub fn with_binding(mut self, binding: ActionBinding) -> Self {
        self.bindings.push(binding);
        self
    }
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
    pub fn with_category(mut self, cat: &str) -> Self {
        self.category = cat.to_string();
        self
    }
}

// ---------------------------------------------------------------------------
// Action map context
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActionMapContext {
    pub name: String,
    pub priority: i32,
    pub block_lower_priority: bool,
    pub actions: Vec<InputAction>,
    pub enabled: bool,
    pub description: String,
}

impl ActionMapContext {
    pub fn new(name: &str, priority: i32) -> Self {
        Self {
            name: name.to_string(),
            priority,
            block_lower_priority: false,
            actions: Vec::new(),
            enabled: true,
            description: String::new(),
        }
    }
    pub fn with_action(mut self, action: InputAction) -> Self {
        self.actions.push(action);
        self
    }
    pub fn find_action(&self, name: &str) -> Option<&InputAction> {
        self.actions.iter().find(|a| a.name == name)
    }
    pub fn find_action_mut(&mut self, name: &str) -> Option<&mut InputAction> {
        self.actions.iter_mut().find(|a| a.name == name)
    }
    pub fn action_names(&self) -> Vec<&str> {
        self.actions.iter().map(|a| a.name.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Dead zone visualizer data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DeadZoneConfig {
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub axial_x: f32,
    pub axial_y: f32,
    pub kind: DeadZoneKind,
    pub snap_to_axis: bool,
    pub snap_threshold: f32,
    pub anti_deadzone: f32,
}

impl Default for DeadZoneConfig {
    fn default() -> Self {
        Self {
            inner_radius: 0.15,
            outer_radius: 0.95,
            axial_x: 0.15,
            axial_y: 0.15,
            kind: DeadZoneKind::Radial,
            snap_to_axis: false,
            snap_threshold: 0.1,
            anti_deadzone: 0.0,
        }
    }
}

impl DeadZoneConfig {
    pub fn apply_radial(&self, raw: Vec2) -> Vec2 {
        let len = raw.length();
        if len < self.inner_radius { return Vec2::ZERO; }
        if len > self.outer_radius { return raw / len; }
        let rescaled = (len - self.inner_radius) / (self.outer_radius - self.inner_radius);
        raw / len * rescaled
    }
    pub fn apply_axial(&self, raw: Vec2) -> Vec2 {
        let x = if raw.x.abs() < self.axial_x { 0.0 } else { (raw.x.abs() - self.axial_x) / (1.0 - self.axial_x) * raw.x.signum() };
        let y = if raw.y.abs() < self.axial_y { 0.0 } else { (raw.y.abs() - self.axial_y) / (1.0 - self.axial_y) * raw.y.signum() };
        Vec2::new(x, y)
    }
    pub fn apply(&self, raw: Vec2) -> Vec2 {
        match self.kind {
            DeadZoneKind::Radial => self.apply_radial(raw),
            DeadZoneKind::Axial => self.apply_axial(raw),
            DeadZoneKind::Hysteresis => self.apply_radial(raw),
        }
    }
}

// ---------------------------------------------------------------------------
// Sensitivity profiles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SensitivityProfile {
    pub name: String,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub right_stick_x: f32,
    pub right_stick_y: f32,
    pub aim_mouse_x: f32,
    pub aim_mouse_y: f32,
    pub aim_stick_x: f32,
    pub aim_stick_y: f32,
    pub invert_y_mouse: bool,
    pub invert_y_stick: bool,
    pub invert_x_stick: bool,
    pub ads_sensitivity_multiplier: f32,
    pub acceleration: f32,
    pub input_lag_compensation_ms: f32,
    pub mouse_smoothing: f32,
    pub raw_input: bool,
}

impl Default for SensitivityProfile {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            mouse_x: 1.0, mouse_y: 1.0,
            right_stick_x: 1.0, right_stick_y: 1.0,
            aim_mouse_x: 0.6, aim_mouse_y: 0.6,
            aim_stick_x: 0.6, aim_stick_y: 0.6,
            invert_y_mouse: false,
            invert_y_stick: false,
            invert_x_stick: false,
            ads_sensitivity_multiplier: 0.7,
            acceleration: 0.0,
            input_lag_compensation_ms: 0.0,
            mouse_smoothing: 0.0,
            raw_input: true,
        }
    }
}

impl SensitivityProfile {
    pub fn esports() -> Self {
        Self { name: "eSports".to_string(), mouse_x: 0.5, mouse_y: 0.5, raw_input: true, mouse_smoothing: 0.0, ..Default::default() }
    }
    pub fn casual() -> Self {
        Self { name: "Casual".to_string(), mouse_x: 1.5, mouse_y: 1.5, mouse_smoothing: 0.3, ..Default::default() }
    }
    pub fn console() -> Self {
        Self { name: "Console".to_string(), right_stick_x: 2.0, right_stick_y: 1.8, aim_stick_x: 1.2, aim_stick_y: 1.2, ..Default::default() }
    }
}

// ---------------------------------------------------------------------------
// Haptic feedback
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HapticEffectKind {
    Constant,
    Sine,
    Square,
    Triangle,
    SawUp,
    SawDown,
    Custom,
    Rumble,
    AdaptiveTriggerFeedback,
    AdaptiveTriggerWeapon,
    AdaptiveTriggerBow,
}

impl HapticEffectKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Constant => "Constant",
            Self::Sine => "Sine",
            Self::Square => "Square",
            Self::Triangle => "Triangle",
            Self::SawUp => "Saw Up",
            Self::SawDown => "Saw Down",
            Self::Custom => "Custom",
            Self::Rumble => "Rumble",
            Self::AdaptiveTriggerFeedback => "Adaptive: Feedback",
            Self::AdaptiveTriggerWeapon => "Adaptive: Weapon",
            Self::AdaptiveTriggerBow => "Adaptive: Bow",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HapticEffect {
    pub name: String,
    pub kind: HapticEffectKind,
    pub duration_secs: f32,
    pub low_freq_motor: f32,
    pub high_freq_motor: f32,
    pub left_trigger_intensity: f32,
    pub right_trigger_intensity: f32,
    pub start_delay_secs: f32,
    pub fade_in_secs: f32,
    pub fade_out_secs: f32,
    pub frequency_hz: f32,
    pub attack_intensity: f32,
    pub sustain_intensity: f32,
    pub release_intensity: f32,
}

impl HapticEffect {
    pub fn new(name: &str, kind: HapticEffectKind) -> Self {
        Self {
            name: name.to_string(),
            kind,
            duration_secs: 0.3,
            low_freq_motor: 0.5,
            high_freq_motor: 0.5,
            left_trigger_intensity: 0.0,
            right_trigger_intensity: 0.0,
            start_delay_secs: 0.0,
            fade_in_secs: 0.0,
            fade_out_secs: 0.1,
            frequency_hz: 60.0,
            attack_intensity: 1.0,
            sustain_intensity: 0.7,
            release_intensity: 0.0,
        }
    }

    pub fn gun_shot() -> Self {
        let mut e = Self::new("GunShot", HapticEffectKind::Rumble);
        e.duration_secs = 0.15;
        e.low_freq_motor = 0.9;
        e.high_freq_motor = 0.6;
        e.fade_out_secs = 0.08;
        e
    }

    pub fn explosion() -> Self {
        let mut e = Self::new("Explosion", HapticEffectKind::Rumble);
        e.duration_secs = 0.6;
        e.low_freq_motor = 1.0;
        e.high_freq_motor = 0.4;
        e.fade_in_secs = 0.05;
        e.fade_out_secs = 0.3;
        e
    }

    pub fn footstep() -> Self {
        let mut e = Self::new("Footstep", HapticEffectKind::Sine);
        e.duration_secs = 0.08;
        e.low_freq_motor = 0.3;
        e.high_freq_motor = 0.2;
        e
    }

    pub fn ui_click() -> Self {
        let mut e = Self::new("UIClick", HapticEffectKind::Constant);
        e.duration_secs = 0.04;
        e.low_freq_motor = 0.1;
        e.high_freq_motor = 0.4;
        e
    }

    pub fn intensity_at_time(&self, t: f32) -> f32 {
        if t < 0.0 || t > self.duration_secs { return 0.0; }
        let fade_in_end = self.fade_in_secs;
        let fade_out_start = self.duration_secs - self.fade_out_secs;
        let envelope = if t < fade_in_end {
            t / fade_in_end.max(0.001)
        } else if t > fade_out_start {
            1.0 - (t - fade_out_start) / self.fade_out_secs.max(0.001)
        } else {
            1.0
        };
        let wave = match self.kind {
            HapticEffectKind::Sine => (t * self.frequency_hz * std::f32::consts::TAU).sin() * 0.5 + 0.5,
            HapticEffectKind::Square => if (t * self.frequency_hz).fract() < 0.5 { 1.0 } else { 0.0 },
            HapticEffectKind::Triangle => {
                let phase = (t * self.frequency_hz).fract();
                if phase < 0.5 { phase * 2.0 } else { 2.0 - phase * 2.0 }
            }
            _ => 1.0,
        };
        envelope * wave * self.sustain_intensity
    }
}

// ---------------------------------------------------------------------------
// Input macro system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InputMacroStep {
    pub action_name: String,
    pub value: f32,
    pub delay_before_ms: u32,
    pub duration_ms: u32,
    pub repeat: u32,
}

#[derive(Debug, Clone)]
pub struct InputMacro {
    pub name: String,
    pub description: String,
    pub steps: Vec<InputMacroStep>,
    pub loop_count: u32,
    pub allow_during_gameplay: bool,
    pub trigger_binding: Option<InputBinding>,
    pub recording: bool,
    pub playback_speed: f32,
}

impl InputMacro {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            steps: Vec::new(),
            loop_count: 1,
            allow_during_gameplay: false,
            trigger_binding: None,
            recording: false,
            playback_speed: 1.0,
        }
    }
    pub fn total_duration_ms(&self) -> u32 {
        self.steps.iter().map(|s| s.delay_before_ms + s.duration_ms * s.repeat.max(1)).sum()
    }
    pub fn add_step(&mut self, action: &str, value: f32, delay_ms: u32, dur_ms: u32) {
        self.steps.push(InputMacroStep {
            action_name: action.to_string(),
            value,
            delay_before_ms: delay_ms,
            duration_ms: dur_ms,
            repeat: 1,
        });
    }
}

// ---------------------------------------------------------------------------
// Combo system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ComboInput {
    pub action: String,
    pub min_value: f32,
    pub max_hold_ms: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ComboDefinition {
    pub name: String,
    pub sequence: Vec<ComboInput>,
    pub max_gap_ms: u32,
    pub output_action: String,
    pub output_value: f32,
    pub priority: i32,
    pub enabled: bool,
}

impl ComboDefinition {
    pub fn new(name: &str, output: &str) -> Self {
        Self {
            name: name.to_string(),
            sequence: Vec::new(),
            max_gap_ms: 300,
            output_action: output.to_string(),
            output_value: 1.0,
            priority: 0,
            enabled: true,
        }
    }
    pub fn add_input(mut self, action: &str, min_value: f32) -> Self {
        self.sequence.push(ComboInput { action: action.to_string(), min_value, max_hold_ms: None });
        self
    }
    pub fn input_count(&self) -> usize { self.sequence.len() }
}

// ---------------------------------------------------------------------------
// Input event log for replay
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InputEvent {
    pub timestamp_ms: f64,
    pub device: InputDevice,
    pub action_name: String,
    pub value: f32,
    pub value2: f32,
    pub frame: u64,
}

#[derive(Debug, Clone, Default)]
pub struct InputReplay {
    pub events: Vec<InputEvent>,
    pub duration_ms: f64,
    pub recording: bool,
    pub playing: bool,
    pub playback_pos_ms: f64,
    pub loop_replay: bool,
    pub playback_speed: f32,
}

impl InputReplay {
    pub fn start_recording(&mut self) { self.recording = true; self.events.clear(); }
    pub fn stop_recording(&mut self) {
        self.recording = false;
        self.duration_ms = self.events.last().map(|e| e.timestamp_ms).unwrap_or(0.0);
    }
    pub fn start_playback(&mut self) { self.playing = true; self.playback_pos_ms = 0.0; }
    pub fn stop_playback(&mut self) { self.playing = false; }
    pub fn record_event(&mut self, event: InputEvent) {
        if self.recording { self.events.push(event); }
    }
    pub fn advance(&mut self, dt_ms: f64) {
        if !self.playing { return; }
        self.playback_pos_ms += dt_ms * self.playback_speed as f64;
        if self.playback_pos_ms >= self.duration_ms {
            if self.loop_replay { self.playback_pos_ms -= self.duration_ms; }
            else { self.playing = false; }
        }
    }
    pub fn current_events(&self) -> Vec<&InputEvent> {
        let t = self.playback_pos_ms;
        self.events.iter().filter(|e| (e.timestamp_ms - t).abs() < 16.0).collect()
    }
    pub fn event_count(&self) -> usize { self.events.len() }
}

// ---------------------------------------------------------------------------
// Virtual cursor / pointer emulation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VirtualCursor {
    pub position: Vec2,
    pub velocity: Vec2,
    pub acceleration: f32,
    pub max_speed: f32,
    pub friction: f32,
    pub deadzone: f32,
    pub screen_size: Vec2,
    pub enabled: bool,
    pub stick_axis: (GamepadAxis, GamepadAxis),
}

impl Default for VirtualCursor {
    fn default() -> Self {
        Self {
            position: Vec2::new(960.0, 540.0),
            velocity: Vec2::ZERO,
            acceleration: 1200.0,
            max_speed: 800.0,
            friction: 8.0,
            deadzone: 0.15,
            screen_size: Vec2::new(1920.0, 1080.0),
            enabled: false,
            stick_axis: (GamepadAxis::RightStickX, GamepadAxis::RightStickY),
        }
    }
}

impl VirtualCursor {
    pub fn update(&mut self, stick: Vec2, dt: f32) {
        let len = stick.length();
        if len < self.deadzone {
            self.velocity *= 1.0 - self.friction * dt;
        } else {
            let dir = stick / len;
            let t = (len - self.deadzone) / (1.0 - self.deadzone);
            self.velocity += dir * self.acceleration * t * dt;
            let speed = self.velocity.length();
            if speed > self.max_speed {
                self.velocity = self.velocity / speed * self.max_speed;
            }
        }
        self.position += self.velocity * dt;
        self.position.x = self.position.x.clamp(0.0, self.screen_size.x);
        self.position.y = self.position.y.clamp(0.0, self.screen_size.y);
    }
}

// ---------------------------------------------------------------------------
// Gesture recognizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureKind {
    Tap,
    DoubleTap,
    LongPress,
    Swipe,
    Pinch,
    Rotate,
    Pan,
    Flick,
    TwoFingerTap,
    ThreeFingerTap,
    EdgeSwipe,
    ForceTap,
}

impl GestureKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tap => "Tap",
            Self::DoubleTap => "Double Tap",
            Self::LongPress => "Long Press",
            Self::Swipe => "Swipe",
            Self::Pinch => "Pinch",
            Self::Rotate => "Rotate",
            Self::Pan => "Pan",
            Self::Flick => "Flick",
            Self::TwoFingerTap => "Two-Finger Tap",
            Self::ThreeFingerTap => "Three-Finger Tap",
            Self::EdgeSwipe => "Edge Swipe",
            Self::ForceTap => "Force Tap",
        }
    }
    pub fn min_touch_points(&self) -> u32 {
        match self {
            Self::Pinch | Self::Rotate | Self::TwoFingerTap => 2,
            Self::ThreeFingerTap => 3,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GestureBinding {
    pub gesture: GestureKind,
    pub action: String,
    pub min_velocity: f32,
    pub max_duration_secs: f32,
    pub direction_filter: Option<Vec2>,
    pub direction_tolerance_deg: f32,
}

// ---------------------------------------------------------------------------
// Accessibility options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AccessibilitySettings {
    pub sticky_keys: bool,
    pub slow_keys: bool,
    pub slow_keys_delay_ms: u32,
    pub toggle_keys: bool,
    pub filter_keys: bool,
    pub filter_keys_repeat_delay_ms: u32,
    pub mouse_keys: bool,
    pub mouse_keys_speed: f32,
    pub high_contrast_cursor: bool,
    pub cursor_size_scale: f32,
    pub single_button_mode: bool,
    pub auto_sprint: bool,
    pub aim_assist_strength: f32,
    pub auto_aim: bool,
    pub hold_to_toggle_aim: bool,
    pub simplified_controls: bool,
    pub button_remapping_enabled: bool,
    pub hold_alternatives: HashMap<String, String>,
    pub color_blind_mode: ColorBlindMode,
    pub text_size_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorBlindMode { None, Protanopia, Deuteranopia, Tritanopia, Monochromacy }

impl ColorBlindMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Protanopia => "Protanopia (Red-Green)",
            Self::Deuteranopia => "Deuteranopia (Green-Red)",
            Self::Tritanopia => "Tritanopia (Blue-Yellow)",
            Self::Monochromacy => "Monochromacy",
        }
    }
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            sticky_keys: false,
            slow_keys: false,
            slow_keys_delay_ms: 500,
            toggle_keys: false,
            filter_keys: false,
            filter_keys_repeat_delay_ms: 200,
            mouse_keys: false,
            mouse_keys_speed: 1.0,
            high_contrast_cursor: false,
            cursor_size_scale: 1.0,
            single_button_mode: false,
            auto_sprint: false,
            aim_assist_strength: 0.0,
            auto_aim: false,
            hold_to_toggle_aim: false,
            simplified_controls: false,
            button_remapping_enabled: true,
            hold_alternatives: HashMap::new(),
            color_blind_mode: ColorBlindMode::None,
            text_size_scale: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Input device state (runtime)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GamepadState {
    pub device_id: u32,
    pub device: InputDevice,
    pub connected: bool,
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub buttons_pressed: std::collections::HashSet<u32>,
    pub gyro: glam::Vec3,
    pub accel: glam::Vec3,
    pub touchpad_pos: Vec2,
    pub touchpad_pressed: bool,
    pub battery_level: f32,
    pub is_wireless: bool,
}

impl Default for GamepadState {
    fn default() -> Self {
        Self {
            device_id: 0,
            device: InputDevice::GamepadGeneric,
            connected: false,
            left_stick: Vec2::ZERO,
            right_stick: Vec2::ZERO,
            left_trigger: 0.0,
            right_trigger: 0.0,
            buttons_pressed: std::collections::HashSet::new(),
            gyro: glam::Vec3::ZERO,
            accel: glam::Vec3::ZERO,
            touchpad_pos: Vec2::ZERO,
            touchpad_pressed: false,
            battery_level: 1.0,
            is_wireless: false,
        }
    }
}

impl GamepadState {
    pub fn synthetic_idle() -> Self {
        Self {
            device_id: 0,
            device: InputDevice::GamepadXbox,
            connected: true,
            battery_level: 0.85,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct KeyboardState {
    pub keys_down: std::collections::HashSet<u32>,
    pub keys_just_pressed: std::collections::HashSet<u32>,
    pub keys_just_released: std::collections::HashSet<u32>,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MouseState {
    pub position: Vec2,
    pub delta: Vec2,
    pub scroll: Vec2,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
    pub x1: bool,
    pub x2: bool,
    pub just_pressed: std::collections::HashSet<u32>,
    pub just_released: std::collections::HashSet<u32>,
    pub captured: bool,
    pub dpi: u32,
}

// ---------------------------------------------------------------------------
// Binding conflict detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BindingConflict {
    pub context_a: String,
    pub action_a: String,
    pub context_b: String,
    pub action_b: String,
    pub conflicting_binding: String,
    pub severity: ConflictSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConflictSeverity { Warning, Error }

// ---------------------------------------------------------------------------
// Full input asset (serializable config)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InputAsset {
    pub name: String,
    pub contexts: Vec<ActionMapContext>,
    pub haptic_effects: Vec<HapticEffect>,
    pub macros: Vec<InputMacro>,
    pub combos: Vec<ComboDefinition>,
    pub sensitivity_profiles: Vec<SensitivityProfile>,
    pub active_sensitivity_profile: usize,
    pub deadzone_configs: HashMap<GamepadAxis, DeadZoneConfig>,
    pub accessibility: AccessibilitySettings,
    pub gesture_bindings: Vec<GestureBinding>,
    pub virtual_cursor: VirtualCursor,
    pub replay: InputReplay,
}

impl InputAsset {
    pub fn default_fps() -> Self {
        let gameplay = ActionMapContext::new("Gameplay", 100)
            .with_action(
                InputAction::new("Move", ActionValueType::Vec2)
                    .with_category("Movement")
                    .with_description("WASD / Left Stick movement")
                    .with_binding(ActionBinding::new(InputBinding::KeyCombo(vec![KeyCode::W]), InputDevice::Keyboard))
                    .with_binding(ActionBinding::analog_binding(InputBinding::GamepadAxis(GamepadAxis::LeftStickY, 1.0), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Look", ActionValueType::Vec2)
                    .with_category("Movement")
                    .with_description("Mouse / Right Stick look")
                    .with_binding(ActionBinding::new(InputBinding::MouseAxis(MouseAxis::X, 1.0), InputDevice::Mouse))
                    .with_binding(ActionBinding::analog_binding(InputBinding::GamepadAxis(GamepadAxis::RightStickX, 1.0), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Jump", ActionValueType::Bool)
                    .with_category("Movement")
                    .with_description("Jump / A button")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::Space), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::South), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Sprint", ActionValueType::Bool)
                    .with_category("Movement")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::LShift), InputDevice::Keyboard)
                        .with_trigger(InputTrigger::Down))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::LeftStickClick), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Crouch", ActionValueType::Bool)
                    .with_category("Movement")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::LCtrl), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::RightStickClick), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Fire", ActionValueType::Bool)
                    .with_category("Combat")
                    .with_binding(ActionBinding::new(InputBinding::MouseButton(MouseButton::Left), InputDevice::Mouse)
                        .with_trigger(InputTrigger::Down))
                    .with_binding(ActionBinding::new(InputBinding::GamepadAxis(GamepadAxis::RightTrigger, 1.0), InputDevice::GamepadXbox)
                        .with_modifier(InputModifier::DeadZone { lower: 0.3, upper: 1.0, kind: DeadZoneKind::Axial }))
            )
            .with_action(
                InputAction::new("Aim", ActionValueType::Bool)
                    .with_category("Combat")
                    .with_binding(ActionBinding::new(InputBinding::MouseButton(MouseButton::Right), InputDevice::Mouse)
                        .with_trigger(InputTrigger::Down))
                    .with_binding(ActionBinding::new(InputBinding::GamepadAxis(GamepadAxis::LeftTrigger, 1.0), InputDevice::GamepadXbox)
                        .with_modifier(InputModifier::DeadZone { lower: 0.3, upper: 1.0, kind: DeadZoneKind::Axial }))
            )
            .with_action(
                InputAction::new("Reload", ActionValueType::Bool)
                    .with_category("Combat")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::R), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::West), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Interact", ActionValueType::Bool)
                    .with_category("Interaction")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::E), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::North), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Inventory", ActionValueType::Bool)
                    .with_category("UI")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::I), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::DPadUp), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Pause", ActionValueType::Bool)
                    .with_category("System")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::Escape), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::Start), InputDevice::GamepadXbox))
            );

        let ui_context = ActionMapContext::new("UI", 50)
            .with_action(
                InputAction::new("Navigate", ActionValueType::Vec2)
                    .with_category("UI")
                    .with_binding(ActionBinding::analog_binding(InputBinding::GamepadAxis(GamepadAxis::LeftStickY, 1.0), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Confirm", ActionValueType::Bool)
                    .with_category("UI")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::Return), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::South), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("Cancel", ActionValueType::Bool)
                    .with_category("UI")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::Escape), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::East), InputDevice::GamepadXbox))
            )
            .with_action(
                InputAction::new("TabNext", ActionValueType::Bool)
                    .with_category("UI")
                    .with_binding(ActionBinding::new(InputBinding::Key(KeyCode::Tab), InputDevice::Keyboard))
                    .with_binding(ActionBinding::new(InputBinding::GamepadButton(GamepadButton::RightBumper), InputDevice::GamepadXbox))
            );

        let mut deadzone_configs = HashMap::new();
        deadzone_configs.insert(GamepadAxis::LeftStickX, DeadZoneConfig::default());
        deadzone_configs.insert(GamepadAxis::LeftStickY, DeadZoneConfig::default());
        deadzone_configs.insert(GamepadAxis::RightStickX, DeadZoneConfig { inner_radius: 0.12, ..Default::default() });
        deadzone_configs.insert(GamepadAxis::RightStickY, DeadZoneConfig { inner_radius: 0.12, ..Default::default() });

        let haptic_effects = vec![
            HapticEffect::gun_shot(),
            HapticEffect::explosion(),
            HapticEffect::footstep(),
            HapticEffect::ui_click(),
        ];

        let sensitivity_profiles = vec![
            SensitivityProfile::default(),
            SensitivityProfile::esports(),
            SensitivityProfile::casual(),
            SensitivityProfile::console(),
        ];

        let combos = vec![
            ComboDefinition::new("DashForward", "Dash")
                .add_input("Move_Forward", 0.8)
                .add_input("Move_Forward", 0.8),
            ComboDefinition::new("SpinAttack", "SpinAttack")
                .add_input("Fire", 0.5)
                .add_input("Fire", 0.5)
                .add_input("Fire", 0.5),
        ];

        Self {
            name: "FPS Default".to_string(),
            contexts: vec![gameplay, ui_context],
            haptic_effects,
            macros: Vec::new(),
            combos,
            sensitivity_profiles,
            active_sensitivity_profile: 0,
            deadzone_configs,
            accessibility: AccessibilitySettings::default(),
            gesture_bindings: Vec::new(),
            virtual_cursor: VirtualCursor::default(),
            replay: InputReplay::default(),
        }
    }

    pub fn detect_conflicts(&self) -> Vec<BindingConflict> {
        let mut conflicts = Vec::new();
        for (ci, ctx) in self.contexts.iter().enumerate() {
            for (ai, action) in ctx.actions.iter().enumerate() {
                for (bi, binding) in action.bindings.iter().enumerate() {
                    for (ci2, ctx2) in self.contexts.iter().enumerate() {
                        if ci2 < ci { continue; }
                        for (ai2, action2) in ctx2.actions.iter().enumerate() {
                            if ci2 == ci && ai2 <= ai { continue; }
                            for binding2 in &action2.bindings {
                                if binding.binding == binding2.binding {
                                    conflicts.push(BindingConflict {
                                        context_a: ctx.name.clone(),
                                        action_a: action.name.clone(),
                                        context_b: ctx2.name.clone(),
                                        action_b: action2.name.clone(),
                                        conflicting_binding: binding.binding.label(),
                                        severity: ConflictSeverity::Warning,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        conflicts
    }

    pub fn all_actions(&self) -> Vec<(&str, &InputAction)> {
        self.contexts.iter()
            .flat_map(|c| c.actions.iter().map(move |a| (c.name.as_str(), a)))
            .collect()
    }

    pub fn actions_by_category(&self) -> HashMap<&str, Vec<&InputAction>> {
        let mut map: HashMap<&str, Vec<&InputAction>> = HashMap::new();
        for ctx in &self.contexts {
            for action in &ctx.actions {
                map.entry(action.category.as_str()).or_default().push(action);
            }
        }
        map
    }
}

// ---------------------------------------------------------------------------
// Input editor state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEditorPanel {
    ActionMaps,
    Bindings,
    Deadzone,
    Haptics,
    Sensitivity,
    Macros,
    Combos,
    Accessibility,
    Devices,
    Replay,
    Gestures,
    Conflicts,
}

impl InputEditorPanel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ActionMaps => "Action Maps",
            Self::Bindings => "Bindings",
            Self::Deadzone => "Dead Zone",
            Self::Haptics => "Haptics",
            Self::Sensitivity => "Sensitivity",
            Self::Macros => "Macros",
            Self::Combos => "Combos",
            Self::Accessibility => "Accessibility",
            Self::Devices => "Devices",
            Self::Replay => "Replay",
            Self::Gestures => "Gestures",
            Self::Conflicts => "Conflicts",
        }
    }
}

#[derive(Debug)]
pub struct InputEditor {
    pub asset: InputAsset,
    pub active_panel: InputEditorPanel,
    pub selected_context: Option<usize>,
    pub selected_action: Option<usize>,
    pub selected_binding: Option<usize>,
    pub selected_haptic: Option<usize>,
    pub selected_macro: Option<usize>,
    pub rebinding_mode: bool,
    pub rebind_action: Option<String>,
    pub rebind_binding_index: Option<usize>,
    pub gamepad_state: GamepadState,
    pub keyboard_state: KeyboardState,
    pub mouse_state: MouseState,
    pub live_input_values: HashMap<String, f32>,
    pub deadzone_test_input: Vec2,
    pub deadzone_test_output: Vec2,
    pub selected_deadzone_axis: GamepadAxis,
    pub haptic_preview_time: f32,
    pub haptic_playing: bool,
    pub search_query: String,
    pub show_all_platforms: bool,
    pub active_device_tab: InputDevice,
    pub conflicts: Vec<BindingConflict>,
    pub conflict_filter: Option<ConflictSeverity>,
    pub history: Vec<InputAsset>,
    pub history_pos: usize,
}

impl Default for InputEditor {
    fn default() -> Self {
        let asset = InputAsset::default_fps();
        let conflicts = asset.detect_conflicts();
        Self {
            asset,
            active_panel: InputEditorPanel::ActionMaps,
            selected_context: Some(0),
            selected_action: None,
            selected_binding: None,
            selected_haptic: None,
            selected_macro: None,
            rebinding_mode: false,
            rebind_action: None,
            rebind_binding_index: None,
            gamepad_state: GamepadState::synthetic_idle(),
            keyboard_state: KeyboardState::default(),
            mouse_state: MouseState::default(),
            live_input_values: HashMap::new(),
            deadzone_test_input: Vec2::ZERO,
            deadzone_test_output: Vec2::ZERO,
            selected_deadzone_axis: GamepadAxis::LeftStickX,
            haptic_preview_time: 0.0,
            haptic_playing: false,
            search_query: String::new(),
            show_all_platforms: true,
            active_device_tab: InputDevice::Keyboard,
            conflicts,
            conflict_filter: None,
            history: Vec::new(),
            history_pos: 0,
        }
    }
}

impl InputEditor {
    pub fn snapshot(&mut self) {
        self.history.truncate(self.history_pos);
        self.history.push(self.asset.clone());
        self.history_pos = self.history.len();
    }

    pub fn undo(&mut self) {
        if self.history_pos > 1 {
            self.history_pos -= 1;
            self.asset = self.history[self.history_pos - 1].clone();
            self.refresh_conflicts();
        }
    }

    pub fn redo(&mut self) {
        if self.history_pos < self.history.len() {
            self.asset = self.history[self.history_pos].clone();
            self.history_pos += 1;
            self.refresh_conflicts();
        }
    }

    pub fn refresh_conflicts(&mut self) {
        self.conflicts = self.asset.detect_conflicts();
    }

    pub fn begin_rebind(&mut self, action: &str, binding_index: usize) {
        self.rebinding_mode = true;
        self.rebind_action = Some(action.to_string());
        self.rebind_binding_index = Some(binding_index);
    }

    pub fn cancel_rebind(&mut self) {
        self.rebinding_mode = false;
        self.rebind_action = None;
        self.rebind_binding_index = None;
    }

    pub fn update_deadzone_test(&mut self, raw: Vec2) {
        self.deadzone_test_input = raw;
        if let Some(config) = self.asset.deadzone_configs.get(&self.selected_deadzone_axis) {
            self.deadzone_test_output = config.apply(raw);
        }
    }

    pub fn update_haptic_preview(&mut self, dt: f32) {
        if self.haptic_playing {
            self.haptic_preview_time += dt;
            if let Some(idx) = self.selected_haptic {
                if let Some(effect) = self.asset.haptic_effects.get(idx) {
                    if self.haptic_preview_time > effect.duration_secs {
                        self.haptic_playing = false;
                        self.haptic_preview_time = 0.0;
                    }
                }
            }
        }
    }

    pub fn current_haptic_intensity(&self) -> f32 {
        if !self.haptic_playing { return 0.0; }
        if let Some(idx) = self.selected_haptic {
            if let Some(effect) = self.asset.haptic_effects.get(idx) {
                return effect.intensity_at_time(self.haptic_preview_time);
            }
        }
        0.0
    }

    pub fn filtered_actions(&self) -> Vec<(&str, &InputAction)> {
        let q = self.search_query.to_lowercase();
        self.asset.all_actions().into_iter().filter(|(_, a)| {
            q.is_empty() || a.name.to_lowercase().contains(&q) || a.category.to_lowercase().contains(&q)
        }).collect()
    }

    pub fn active_context(&self) -> Option<&ActionMapContext> {
        self.selected_context.and_then(|i| self.asset.contexts.get(i))
    }

    pub fn active_context_mut(&mut self) -> Option<&mut ActionMapContext> {
        self.selected_context.and_then(|i| self.asset.contexts.get_mut(i))
    }

    pub fn generate_code_bindings(&self) -> String {
        let mut lines = Vec::new();
        lines.push("// Auto-generated input bindings".to_string());
        for ctx in &self.asset.contexts {
            lines.push(format!("// Context: {} (priority {})", ctx.name, ctx.priority));
            for action in &ctx.actions {
                lines.push(format!("//   Action: {} ({:?})", action.name, action.value_type));
                for binding in &action.bindings {
                    lines.push(format!("//     Binding: {} [{}]", binding.binding.label(), binding.device.label()));
                }
            }
        }
        lines.join("\n")
    }
}
