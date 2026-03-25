//! Proof Editor — Visual staging and scene authoring environment.
//!
//! Run: `cargo run -p proof-editor`

#[allow(unused)] mod app;
#[allow(unused)] mod panels;
#[allow(unused)] mod tools;
#[allow(unused)] mod scene;
#[allow(unused)] mod ui;
#[allow(unused)] mod viewport;
#[allow(unused)] mod assets;
#[allow(unused)] mod commands;
#[allow(unused)] mod hotkeys;
#[allow(unused)] mod clipboard;
#[allow(unused)] mod preferences;
#[allow(unused)] mod layout;
#[allow(unused)] mod widgets;
#[allow(unused)] mod node_graph;
#[allow(unused)] mod timeline;
#[allow(unused)] mod render_settings;
#[allow(unused)] mod asset_browser;
#[allow(unused)] mod profiler_panel;
#[allow(unused)] mod project;

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
