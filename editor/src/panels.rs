//! Panel manager — coordinates all editor panels.

use proof_engine::input::InputState;
use crate::scene::SceneDocument;
use crate::tools::ToolManager;
use crate::layout::LayoutManager;

pub struct PanelManager {
    pub hierarchy_scroll: f32,
    pub inspector_scroll: f32,
    pub console_open: bool,
    pub asset_browser_open: bool,
}

impl PanelManager {
    pub fn new() -> Self {
        Self {
            hierarchy_scroll: 0.0,
            inspector_scroll: 0.0,
            console_open: false,
            asset_browser_open: false,
        }
    }

    pub fn update(
        &mut self,
        _input: &InputState,
        _doc: &SceneDocument,
        _tools: &ToolManager,
        _layout: &LayoutManager,
    ) {
        // Panel interaction logic will go here as we build out
        // mouse-based panel interactions
    }
}
