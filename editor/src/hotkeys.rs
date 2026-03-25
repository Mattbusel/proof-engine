//! Hotkey mapping — all keyboard shortcuts in one place.

use proof_engine::input::Key;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorAction {
    Undo, Redo, Save, Load, New, SelectAll, Delete, Duplicate,
    TogglePlay, StepFrame, ToggleGrid, ToggleGizmos, ToggleStats, ToggleHelp,
    ToolSelect, ToolMove, ToolRotate, ToolScale, ToolPlace, ToolField, ToolEntity, ToolParticle,
    Deselect, ScreenShake,
}

pub struct HotkeyMap {
    pub bindings: HashMap<(Key, bool, bool), EditorAction>,
}

impl HotkeyMap {
    pub fn defaults() -> Self {
        let mut b = HashMap::new();
        // Ctrl+ shortcuts
        b.insert((Key::Z, true, false), EditorAction::Undo);
        b.insert((Key::Y, true, false), EditorAction::Redo);
        b.insert((Key::S, true, false), EditorAction::Save);
        b.insert((Key::O, true, false), EditorAction::Load);
        b.insert((Key::N, true, false), EditorAction::New);
        b.insert((Key::A, true, false), EditorAction::SelectAll);
        b.insert((Key::D, true, false), EditorAction::Duplicate);
        // Function keys
        b.insert((Key::F1, false, false), EditorAction::ToggleHelp);
        b.insert((Key::F2, false, false), EditorAction::ToggleStats);
        b.insert((Key::F3, false, false), EditorAction::ToggleGrid);
        b.insert((Key::F4, false, false), EditorAction::ToggleGizmos);
        b.insert((Key::F5, false, false), EditorAction::TogglePlay);
        b.insert((Key::F6, false, false), EditorAction::StepFrame);
        // Tool keys
        b.insert((Key::V, false, false), EditorAction::ToolSelect);
        b.insert((Key::G, false, false), EditorAction::ToolMove);
        b.insert((Key::R, false, false), EditorAction::ToolRotate);
        b.insert((Key::T, false, false), EditorAction::ToolScale);
        b.insert((Key::P, false, false), EditorAction::ToolPlace);
        b.insert((Key::F, false, false), EditorAction::ToolField);
        b.insert((Key::E, false, false), EditorAction::ToolEntity);
        b.insert((Key::X, false, false), EditorAction::ToolParticle);
        // Misc
        b.insert((Key::Escape, false, false), EditorAction::Deselect);
        b.insert((Key::Delete, false, false), EditorAction::Delete);
        b.insert((Key::Space, false, false), EditorAction::ScreenShake);

        Self { bindings: b }
    }
}
