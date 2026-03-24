//! Configurable key bindings.

use super::Key;
use std::collections::HashMap;

/// A named action that can be bound to a key.
pub type Action = String;

/// Maps action names to keys. Loaded from config, fallback to defaults.
#[derive(Default)]
pub struct KeyBindings {
    bindings: HashMap<Action, Key>,
}

impl KeyBindings {
    pub fn new() -> Self { Self::default() }

    pub fn bind(&mut self, action: impl Into<Action>, key: Key) {
        self.bindings.insert(action.into(), key);
    }

    pub fn key_for(&self, action: &str) -> Option<Key> {
        self.bindings.get(action).copied()
    }
}

/// Default CHAOS RPG keybindings.
pub fn chaos_rpg_defaults() -> KeyBindings {
    let mut b = KeyBindings::new();
    b.bind("attack", Key::A);
    b.bind("heavy_attack", Key::H);
    b.bind("defend", Key::D);
    b.bind("flee", Key::F);
    b.bind("taunt", Key::T);
    b.bind("char_sheet", Key::C);
    b.bind("passive_tree", Key::P);
    b.bind("chaos_viz", Key::V);
    b.bind("log_collapse", Key::Z);
    b.bind("confirm", Key::Enter);
    b.bind("back", Key::Escape);
    b.bind("cycle_theme", Key::T);
    b
}
