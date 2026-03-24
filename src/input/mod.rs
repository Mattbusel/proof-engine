//! Input handling — keyboard, mouse, scroll, and window events.
//!
//! `InputState` is rebuilt fresh each frame by the render pipeline and handed
//! to game code inside `ProofGame::update()`. All transient state (just-pressed,
//! just-released, delta values) is cleared at the start of each frame.
//!
//! # Example
//!
//! ```rust,no_run
//! use proof_engine::input::{InputState, Key};
//!
//! fn my_update(input: &InputState) {
//!     if input.just_pressed(Key::Space) {
//!         println!("space!");
//!     }
//!     if input.mouse_left {
//!         println!("dragging at ({}, {})", input.mouse_x, input.mouse_y);
//!     }
//! }
//! ```

pub mod keybindings;

use glam::Vec2;
use std::collections::HashSet;

// ── InputState ────────────────────────────────────────────────────────────────

/// Complete snapshot of all input this frame.
///
/// Created and updated by the `Pipeline` each frame.  Game code receives a
/// `&mut ProofEngine` whose `.input` field holds this value.
#[derive(Clone, Default)]
pub struct InputState {
    // ── Keyboard ──────────────────────────────────────────────────────────────
    /// All keys currently held down.
    pub keys_pressed:       HashSet<Key>,
    /// Keys that transitioned to pressed this frame.
    pub keys_just_pressed:  HashSet<Key>,
    /// Keys that transitioned to released this frame.
    pub keys_just_released: HashSet<Key>,

    // ── Mouse position ────────────────────────────────────────────────────────
    /// Cursor X in window pixels (top-left origin).
    pub mouse_x:      f32,
    /// Cursor Y in window pixels (top-left origin).
    pub mouse_y:      f32,
    /// Mouse movement since last frame, in pixels.
    pub mouse_delta:  Vec2,
    /// Cursor in normalized device coordinates ([-1, 1], Y-up).
    pub mouse_ndc:    Vec2,

    // ── Mouse buttons ─────────────────────────────────────────────────────────
    pub mouse_left:               bool,
    pub mouse_left_just_pressed:  bool,
    pub mouse_left_just_released: bool,

    pub mouse_right:               bool,
    pub mouse_right_just_pressed:  bool,
    pub mouse_right_just_released: bool,

    pub mouse_middle:              bool,
    pub mouse_middle_just_pressed: bool,

    // ── Scroll ────────────────────────────────────────────────────────────────
    /// Vertical scroll delta this frame (positive = scroll up / zoom in).
    pub scroll_delta: f32,

    // ── Window ────────────────────────────────────────────────────────────────
    /// Set if the window was resized this frame. Contains (width, height).
    pub window_resized: Option<(u32, u32)>,
    /// Set if the user requested a quit (Alt+F4, OS close button).
    pub quit_requested: bool,
}

impl InputState {
    pub fn new() -> Self { Self::default() }

    // ── Keyboard queries ──────────────────────────────────────────────────────

    /// Returns true while the key is held down.
    pub fn is_pressed(&self, key: Key) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Returns true on the frame the key was first pressed.
    pub fn just_pressed(&self, key: Key) -> bool {
        self.keys_just_pressed.contains(&key)
    }

    /// Returns true on the frame the key was released.
    pub fn just_released(&self, key: Key) -> bool {
        self.keys_just_released.contains(&key)
    }

    /// Returns true if any of the given keys are held.
    pub fn any_pressed(&self, keys: &[Key]) -> bool {
        keys.iter().any(|k| self.keys_pressed.contains(k))
    }

    /// Returns true if all of the given keys are held simultaneously.
    pub fn all_pressed(&self, keys: &[Key]) -> bool {
        keys.iter().all(|k| self.keys_pressed.contains(k))
    }

    /// Returns true if a modifier key (Shift, Ctrl, Alt) is held.
    pub fn shift(&self) -> bool {
        self.is_pressed(Key::LShift) || self.is_pressed(Key::RShift)
    }

    pub fn ctrl(&self) -> bool {
        self.is_pressed(Key::LCtrl) || self.is_pressed(Key::RCtrl)
    }

    pub fn alt(&self) -> bool {
        self.is_pressed(Key::LAlt) || self.is_pressed(Key::RAlt)
    }

    // ── Mouse queries ──────────────────────────────────────────────────────────

    /// Mouse position as a Vec2 in window pixels.
    pub fn mouse_pos(&self) -> Vec2 {
        Vec2::new(self.mouse_x, self.mouse_y)
    }

    /// Returns true while the left mouse button is held.
    pub fn mouse_left_down(&self) -> bool { self.mouse_left }

    /// Returns true on the frame the left button was first pressed.
    pub fn mouse_left_click(&self) -> bool { self.mouse_left_just_pressed }

    /// Returns true while the right mouse button is held.
    pub fn mouse_right_down(&self) -> bool { self.mouse_right }

    /// Returns true on the frame the right button was first pressed.
    pub fn mouse_right_click(&self) -> bool { self.mouse_right_just_pressed }

    /// Returns true if the mouse is being dragged with the left button held.
    pub fn mouse_drag(&self) -> bool {
        self.mouse_left && self.mouse_delta.length_squared() > f32::EPSILON
    }

    /// Returns the mouse delta scaled to NDC space (suitable for camera control).
    pub fn mouse_delta_ndc(&self, window_width: u32, window_height: u32) -> Vec2 {
        Vec2::new(
            self.mouse_delta.x / window_width.max(1) as f32 * 2.0,
            -self.mouse_delta.y / window_height.max(1) as f32 * 2.0,
        )
    }

    // ── WASD / arrow movement helpers ─────────────────────────────────────────

    /// Returns a movement vector from WASD keys: (right, up) in [-1, 1].
    /// `W`/`S` → Y axis, `A`/`D` → X axis.
    pub fn wasd(&self) -> Vec2 {
        let x = if self.is_pressed(Key::D) || self.is_pressed(Key::Right) { 1.0 }
                else if self.is_pressed(Key::A) || self.is_pressed(Key::Left) { -1.0 }
                else { 0.0 };
        let y = if self.is_pressed(Key::W) || self.is_pressed(Key::Up) { 1.0 }
                else if self.is_pressed(Key::S) || self.is_pressed(Key::Down) { -1.0 }
                else { 0.0 };
        Vec2::new(x, y)
    }

    /// Returns a normalized movement vector from arrow keys.
    pub fn arrows(&self) -> Vec2 {
        let x = if self.is_pressed(Key::Right) { 1.0 }
                else if self.is_pressed(Key::Left) { -1.0 }
                else { 0.0 };
        let y = if self.is_pressed(Key::Up) { 1.0 }
                else if self.is_pressed(Key::Down) { -1.0 }
                else { 0.0 };
        Vec2::new(x, y)
    }

    // ── Internal: called by pipeline at start of frame ─────────────────────────

    /// Clear all per-frame transient state. Called by the pipeline before processing events.
    pub fn clear_frame(&mut self) {
        self.keys_just_pressed.clear();
        self.keys_just_released.clear();
        self.scroll_delta              = 0.0;
        self.mouse_delta               = Vec2::ZERO;
        self.mouse_left_just_pressed   = false;
        self.mouse_left_just_released  = false;
        self.mouse_right_just_pressed  = false;
        self.mouse_right_just_released = false;
        self.mouse_middle_just_pressed = false;
        self.window_resized            = None;
        self.quit_requested            = false;
    }
}

// ── Key enum ──────────────────────────────────────────────────────────────────

/// Keyboard key codes understood by the engine.
///
/// Maps 1:1 to logical key names rather than physical scan codes.
/// Use `InputState::is_pressed(Key::X)` to query any key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // ── Letters ───────────────────────────────────────────────────────────────
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // ── Digits ────────────────────────────────────────────────────────────────
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,

    // ── Navigation ────────────────────────────────────────────────────────────
    Up, Down, Left, Right,
    PageUp, PageDown,
    Home, End,
    Insert, Delete,

    // ── Action ────────────────────────────────────────────────────────────────
    Enter, Escape, Space, Backspace, Tab,

    // ── Function ──────────────────────────────────────────────────────────────
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // ── Punctuation ───────────────────────────────────────────────────────────
    Slash, Backslash, Period, Comma, Semicolon, Quote,
    LBracket, RBracket, Minus, Equals, Backtick,

    // ── Modifiers ─────────────────────────────────────────────────────────────
    LShift, RShift, LCtrl, RCtrl, LAlt, RAlt,
}

impl Key {
    /// Human-readable name for the key (for display in key-binding menus).
    pub fn display_name(&self) -> &'static str {
        match self {
            Key::A => "A", Key::B => "B", Key::C => "C", Key::D => "D",
            Key::E => "E", Key::F => "F", Key::G => "G", Key::H => "H",
            Key::I => "I", Key::J => "J", Key::K => "K", Key::L => "L",
            Key::M => "M", Key::N => "N", Key::O => "O", Key::P => "P",
            Key::Q => "Q", Key::R => "R", Key::S => "S", Key::T => "T",
            Key::U => "U", Key::V => "V", Key::W => "W", Key::X => "X",
            Key::Y => "Y", Key::Z => "Z",
            Key::Num0 => "0", Key::Num1 => "1", Key::Num2 => "2", Key::Num3 => "3",
            Key::Num4 => "4", Key::Num5 => "5", Key::Num6 => "6", Key::Num7 => "7",
            Key::Num8 => "8", Key::Num9 => "9",
            Key::Up => "↑", Key::Down => "↓", Key::Left => "←", Key::Right => "→",
            Key::PageUp => "PgUp", Key::PageDown => "PgDn",
            Key::Home => "Home", Key::End => "End",
            Key::Insert => "Ins", Key::Delete => "Del",
            Key::Enter => "Enter", Key::Escape => "Esc", Key::Space => "Space",
            Key::Backspace => "Backspace", Key::Tab => "Tab",
            Key::F1 => "F1", Key::F2 => "F2", Key::F3 => "F3", Key::F4 => "F4",
            Key::F5 => "F5", Key::F6 => "F6", Key::F7 => "F7", Key::F8 => "F8",
            Key::F9 => "F9", Key::F10 => "F10", Key::F11 => "F11", Key::F12 => "F12",
            Key::Slash => "/", Key::Backslash => "\\", Key::Period => ".",
            Key::Comma => ",", Key::Semicolon => ";", Key::Quote => "'",
            Key::LBracket => "[", Key::RBracket => "]",
            Key::Minus => "-", Key::Equals => "=", Key::Backtick => "`",
            Key::LShift | Key::RShift => "Shift",
            Key::LCtrl  | Key::RCtrl  => "Ctrl",
            Key::LAlt   | Key::RAlt   => "Alt",
        }
    }

    /// All printable/action keys (excludes modifiers). Useful for rebinding UIs.
    pub fn all_bindable() -> &'static [Key] {
        &[
            Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H,
            Key::I, Key::J, Key::K, Key::L, Key::M, Key::N, Key::O, Key::P,
            Key::Q, Key::R, Key::S, Key::T, Key::U, Key::V, Key::W, Key::X,
            Key::Y, Key::Z,
            Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4,
            Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9,
            Key::Up, Key::Down, Key::Left, Key::Right,
            Key::Enter, Key::Escape, Key::Space, Key::Backspace, Key::Tab,
            Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6,
            Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_frame_resets_transient() {
        let mut input = InputState::new();
        input.keys_just_pressed.insert(Key::Space);
        input.scroll_delta = 3.0;
        input.mouse_delta  = Vec2::new(10.0, 5.0);
        input.clear_frame();
        assert!(input.keys_just_pressed.is_empty());
        assert_eq!(input.scroll_delta, 0.0);
        assert_eq!(input.mouse_delta, Vec2::ZERO);
    }

    #[test]
    fn wasd_returns_correct_direction() {
        let mut input = InputState::new();
        input.keys_pressed.insert(Key::D);
        input.keys_pressed.insert(Key::W);
        let v = input.wasd();
        assert!(v.x > 0.0 && v.y > 0.0);
    }

    #[test]
    fn shift_ctrl_helpers() {
        let mut input = InputState::new();
        assert!(!input.shift());
        input.keys_pressed.insert(Key::LShift);
        assert!(input.shift());
        assert!(!input.ctrl());
    }

    #[test]
    fn mouse_drag_only_when_moving() {
        let mut input = InputState::new();
        input.mouse_left = true;
        assert!(!input.mouse_drag()); // no movement
        input.mouse_delta = Vec2::new(3.0, 0.0);
        assert!(input.mouse_drag());
    }
}
