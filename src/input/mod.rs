//! Input handling — keyboard, mouse, and window events.

pub mod keybindings;

/// The current input state for this frame.
#[derive(Default, Clone)]
pub struct InputState {
    pub keys_pressed: std::collections::HashSet<Key>,
    pub keys_just_pressed: std::collections::HashSet<Key>,
    pub keys_just_released: std::collections::HashSet<Key>,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_left: bool,
    pub mouse_right: bool,
    pub scroll_delta: f32,
    pub window_resized: Option<(u32, u32)>,
    pub quit_requested: bool,
}

impl InputState {
    pub fn new() -> Self { Self::default() }

    pub fn is_pressed(&self, key: Key) -> bool { self.keys_pressed.contains(&key) }
    pub fn just_pressed(&self, key: Key) -> bool { self.keys_just_pressed.contains(&key) }
    pub fn just_released(&self, key: Key) -> bool { self.keys_just_released.contains(&key) }

    /// Clear per-frame transient state (call at start of each frame).
    pub fn clear_frame(&mut self) {
        self.keys_just_pressed.clear();
        self.keys_just_released.clear();
        self.scroll_delta = 0.0;
        self.window_resized = None;
    }
}

/// Keyboard key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    Up, Down, Left, Right,
    Enter, Escape, Space, Backspace, Tab,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Slash, Backslash, Period, Comma, Semicolon, Quote,
    LBracket, RBracket, Minus, Equals, Backtick,
    LShift, RShift, LCtrl, RCtrl, LAlt, RAlt,
}
