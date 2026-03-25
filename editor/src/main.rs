//! Proof Editor — Visual staging and scene authoring environment.
//!
//! A full-featured editor for the Proof Engine. Create, manipulate, and
//! preview mathematical visual scenes with real-time feedback.
//!
//! # Architecture
//!
//! The editor is a ProofEngine application that renders both the scene
//! AND the editor UI as glyphs. Every panel, button, slider, and menu
//! is rendered through the same glyph pipeline as the scene content.
//!
//! ```text
//! ┌────────────────────────────────────────────────────────┐
//! │ Menu Bar: File  Edit  View  Tools  Scene  Help        │
//! ├──────────┬─────────────────────────────────┬───────────┤
//! │          │                                 │           │
//! │ Hierarchy│       VIEWPORT                  │ Inspector │
//! │          │   (live scene preview)          │           │
//! │ - Scene  │                                 │ Transform │
//! │   - Ent1 │   [gizmos, grid, overlays]     │ Visual    │
//! │   - Ent2 │                                 │ Physics   │
//! │   - Field│                                 │ Math Fn   │
//! │          │                                 │           │
//! ├──────────┴──────────┬──────────────────────┴───────────┤
//! │ Asset Browser / Timeline / Console                     │
//! │ [glyphs] [fields] [presets] [particles] [palettes]    │
//! └────────────────────────────────────────────────────────┘
//! ```
//!
//! Run: `cargo run -p proof-editor`

#[allow(unused)]
mod app;
#[allow(unused)]
mod panels;
#[allow(unused)]
mod tools;
#[allow(unused)]
mod scene;
#[allow(unused)]
mod ui;
#[allow(unused)]
mod viewport;
#[allow(unused)]
mod assets;
#[allow(unused)]
mod commands;
#[allow(unused)]
mod hotkeys;
#[allow(unused)]
mod clipboard;
#[allow(unused)]
mod preferences;
#[allow(unused)]
mod layout;

use proof_engine::prelude::*;
use app::EditorApp;

fn main() {
    env_logger::init();

    let config = EngineConfig {
        window_title: "Proof Editor".to_string(),
        window_width: 1600,
        window_height: 1000,
        render: proof_engine::config::RenderConfig {
            bloom_enabled: true,
            bloom_intensity: 1.2,
            chromatic_aberration: 0.001,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut engine = ProofEngine::new(config);
    let mut editor = EditorApp::new();
    editor.init(&mut engine);

    engine.run(move |engine, dt| {
        editor.update(engine, dt);
    });
}
