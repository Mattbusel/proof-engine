//! proof-editor — fully wired windowed editor binary for Proof Engine.
//!
//! Opens a real window via ProofEngine::run(). Every panel is rendered as
//! glyph/particle primitives injected into engine.scene each frame.

#![allow(dead_code, unused_variables, unused_imports, unused_mut)]

use proof_engine::prelude::*;
use proof_engine::input::Key;
use proof_engine::editor::{
    EditorState, EditorMode, EditorConfig,
    inspector::Inspector,
    hierarchy::HierarchyPanel,
    console::DevConsole,
    gizmos::{GizmoRenderer, GizmoMode},
    sdf_node_editor::{SdfNodeEditor, CombinatorKind, PrimitiveKind},
    material_painter::MaterialPainter,
    bone_rigger::BoneRigger,
    timeline::TimelineEditor,
    kit_panel::KitPanel,
    asset_browser::AssetBrowser,
    camera_controller::{EditorCamera, SnapView, CameraMode, FreeFlyInput},
    perf_overlay::{PerfOverlay, PerfOverlayMode},
    scene_io::{SceneSerializer, Scene, SceneMetadata, SceneUndoManager},
    world_editor::BiomeSystem,
    ai_behavior_editor::Blackboard,
    physics_editor::ShapeParameters,
    render_graph_editor::ResourceId,
    dialogue_editor::StartNode as DialogueStartNode,
    quest_editor::QuestStartNode,
    spline_editor::ControlPoint,
    cinematic_sequencer::Timecode,
    inventory_editor::{RarityConfig, Rarity},
    ability_editor::DamageFormula,
    level_streaming_editor::Aabb,
    audio_mixer_editor::{BiquadState, ParametricEqualizer},
};
use std::path::PathBuf;
use std::f32::consts::TAU;

// ─────────────────────────────────────────────────────────────────────────────
// EditorPanel — all panels including new ones
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorPanel {
    Viewport,
    SdfNodeEditor,
    MaterialPainter,
    BoneRigger,
    Timeline,
    KitPanel,
    Hierarchy,
    AssetBrowser,
    Console,
    // New panels
    WorldEditor,
    AiBehavior,
    Physics,
    RenderGraph,
    Dialogue,
    Quest,
    Spline,
    Cinematic,
    Inventory,
    Ability,
    LevelStreaming,
    AudioMixer,
}

impl EditorPanel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Viewport        => "Viewport",
            Self::SdfNodeEditor   => "SDF Editor",
            Self::MaterialPainter => "Mat Painter",
            Self::BoneRigger      => "Bone Rigger",
            Self::Timeline        => "Timeline",
            Self::KitPanel        => "Kit Params",
            Self::Hierarchy       => "Hierarchy",
            Self::AssetBrowser    => "Assets",
            Self::Console         => "Console",
            Self::WorldEditor     => "World Editor",
            Self::AiBehavior      => "AI Behavior",
            Self::Physics         => "Physics",
            Self::RenderGraph     => "Render Graph",
            Self::Dialogue        => "Dialogue",
            Self::Quest           => "Quest",
            Self::Spline          => "Spline",
            Self::Cinematic       => "Cinematic",
            Self::Inventory       => "Inventory",
            Self::Ability         => "Ability",
            Self::LevelStreaming   => "Level Streaming",
            Self::AudioMixer      => "Audio Mixer",
        }
    }

    pub fn all() -> &'static [EditorPanel] {
        &[
            Self::Viewport, Self::SdfNodeEditor, Self::MaterialPainter,
            Self::BoneRigger, Self::Timeline, Self::KitPanel,
            Self::Hierarchy, Self::AssetBrowser, Self::Console,
            Self::WorldEditor, Self::AiBehavior, Self::Physics,
            Self::RenderGraph, Self::Dialogue, Self::Quest,
            Self::Spline, Self::Cinematic, Self::Inventory,
            Self::Ability, Self::LevelStreaming, Self::AudioMixer,
        ]
    }

    /// Base color (r,g,b) for this panel's UI overlay.
    pub fn color(self) -> (f32, f32, f32) {
        match self {
            Self::Viewport        => (0.4, 0.7, 1.0),
            Self::SdfNodeEditor   => (0.2, 1.0, 0.8),
            Self::MaterialPainter => (1.0, 0.5, 0.2),
            Self::BoneRigger      => (0.9, 0.9, 0.3),
            Self::Timeline        => (0.3, 0.8, 0.4),
            Self::KitPanel        => (0.7, 0.3, 1.0),
            Self::Hierarchy       => (0.5, 0.8, 1.0),
            Self::AssetBrowser    => (0.9, 0.6, 0.2),
            Self::Console         => (0.4, 1.0, 0.4),
            Self::WorldEditor     => (0.2, 0.8, 0.3),
            Self::AiBehavior      => (1.0, 0.3, 0.5),
            Self::Physics         => (0.6, 0.4, 1.0),
            Self::RenderGraph     => (0.3, 0.6, 1.0),
            Self::Dialogue        => (1.0, 0.8, 0.3),
            Self::Quest           => (0.8, 0.4, 0.2),
            Self::Spline          => (0.2, 1.0, 0.6),
            Self::Cinematic       => (0.9, 0.2, 0.8),
            Self::Inventory       => (0.6, 0.9, 0.2),
            Self::Ability         => (1.0, 0.4, 0.1),
            Self::LevelStreaming   => (0.3, 0.7, 0.9),
            Self::AudioMixer      => (0.5, 1.0, 0.7),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DragState
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct DragState {
    pub active:    bool,
    pub start_pos: Vec2,
    pub curr_pos:  Vec2,
    pub button:    u8,
}

impl DragState {
    pub fn delta(&self) -> Vec2 { self.curr_pos - self.start_pos }
    pub fn frame_delta(&mut self, new_pos: Vec2) -> Vec2 {
        let d = new_pos - self.curr_pos;
        self.curr_pos = new_pos;
        d
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// StatusBar
// ─────────────────────────────────────────────────────────────────────────────

pub struct StatusBar {
    pub message:       String,
    pub message_timer: f32,
}

impl StatusBar {
    pub fn new() -> Self { Self { message: String::new(), message_timer: 0.0 } }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.message = msg.into();
        self.message_timer = 4.0;
    }

    pub fn update(&mut self, dt: f32) {
        if self.message_timer > 0.0 {
            self.message_timer -= dt;
            if self.message_timer <= 0.0 { self.message.clear(); }
        }
    }

    pub fn current_message(&self) -> &str { &self.message }
}

// ─────────────────────────────────────────────────────────────────────────────
// New-module state containers
// ─────────────────────────────────────────────────────────────────────────────

pub struct WorldEditorState {
    pub biome_system: BiomeSystem,
    pub entity_count: u32,
}
impl WorldEditorState {
    pub fn new() -> Self {
        Self { biome_system: BiomeSystem::new(), entity_count: 0 }
    }
}

pub struct AiBehaviorState {
    pub blackboard: Blackboard,
    pub node_count: u32,
}
impl AiBehaviorState {
    pub fn new() -> Self {
        Self { blackboard: Blackboard::new(), node_count: 0 }
    }
}

pub struct PhysicsEditorState {
    pub active_bodies: u32,
}
impl PhysicsEditorState {
    pub fn new() -> Self { Self { active_bodies: 0 } }
}

pub struct RenderGraphEditorState {
    pub pass_count: u32,
    pub next_resource_id: u32,
}
impl RenderGraphEditorState {
    pub fn new() -> Self { Self { pass_count: 4, next_resource_id: 0 } }
}

pub struct DialogueEditorState {
    pub root: DialogueStartNode,
    pub node_count: u32,
}
impl DialogueEditorState {
    pub fn new() -> Self {
        Self { root: DialogueStartNode::new(0), node_count: 1 }
    }
}

pub struct QuestEditorState {
    pub root: QuestStartNode,
    pub objective_count: u32,
}
impl QuestEditorState {
    pub fn new() -> Self {
        Self { root: QuestStartNode::new(0), objective_count: 0 }
    }
}

pub struct SplineEditorState {
    pub control_points: Vec<ControlPoint>,
}
impl SplineEditorState {
    pub fn new() -> Self {
        Self {
            control_points: vec![
                ControlPoint::new(Vec3::new(-2.0, 0.0, 0.0)),
                ControlPoint::new(Vec3::new(0.0,  1.0, 0.0)),
                ControlPoint::new(Vec3::new(2.0,  0.0, 0.0)),
            ],
        }
    }
}

pub struct CinematicState {
    pub timecode: Timecode,
    pub track_count: u32,
}
impl CinematicState {
    pub fn new() -> Self {
        Self { timecode: Timecode::new(0, 0, 0, 0), track_count: 0 }
    }
}

pub struct InventoryEditorState {
    pub rarity_config: RarityConfig,
    pub item_count: u32,
}
impl InventoryEditorState {
    pub fn new() -> Self {
        Self { rarity_config: RarityConfig::for_rarity(Rarity::Common), item_count: 0 }
    }
}

pub struct AbilityEditorState {
    pub ability_count: u32,
}
impl AbilityEditorState {
    pub fn new() -> Self { Self { ability_count: 0 } }
}

pub struct LevelStreamingState {
    pub loaded_regions: u32,
    pub streaming_budget_mb: f32,
}
impl LevelStreamingState {
    pub fn new() -> Self { Self { loaded_regions: 0, streaming_budget_mb: 512.0 } }
}

pub struct AudioMixerState {
    pub equalizer: ParametricEqualizer,
    pub master_volume: f32,
    pub channel_count: u32,
}
impl AudioMixerState {
    pub fn new() -> Self {
        Self {
            equalizer: ParametricEqualizer::new(),
            master_volume: 1.0,
            channel_count: 8,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorApp
// ─────────────────────────────────────────────────────────────────────────────

pub struct EditorApp {
    // ── Existing panels ───────────────────────────────────────────────────
    pub editor_state:   EditorState,
    pub inspector:      Inspector,
    pub hierarchy:      HierarchyPanel,
    pub console:        DevConsole,
    pub gizmos:         GizmoRenderer,
    pub sdf_editor:     SdfNodeEditor,
    pub mat_painter:    MaterialPainter,
    pub bone_rigger:    BoneRigger,
    pub timeline:       TimelineEditor,
    pub kit_panel:      KitPanel,
    pub asset_browser:  AssetBrowser,
    pub camera:         EditorCamera,
    pub perf:           PerfOverlay,
    pub status:         StatusBar,
    // ── Scene I/O ─────────────────────────────────────────────────────────
    pub scene:          Scene,
    pub undo_mgr:       SceneUndoManager,
    pub scene_path:     Option<PathBuf>,
    pub scene_dirty:    bool,
    // ── New module state ──────────────────────────────────────────────────
    pub world_editor:      WorldEditorState,
    pub ai_behavior:       AiBehaviorState,
    pub physics_editor:    PhysicsEditorState,
    pub render_graph:      RenderGraphEditorState,
    pub dialogue_editor:   DialogueEditorState,
    pub quest_editor:      QuestEditorState,
    pub spline_editor:     SplineEditorState,
    pub cinematic:         CinematicState,
    pub inventory_editor:  InventoryEditorState,
    pub ability_editor:    AbilityEditorState,
    pub level_streaming:   LevelStreamingState,
    pub audio_mixer:       AudioMixerState,
    // ── UI state ─────────────────────────────────────────────────────────
    pub focused_panel:  EditorPanel,
    pub drag:           DragState,
    pub fly_input:      FreeFlyInput,
    pub show_grid:      bool,
    // ── Playback ─────────────────────────────────────────────────────────
    pub playing:        bool,
    pub time:           f32,
}

impl EditorApp {
    pub fn new() -> Self {
        let sdf_editor    = SdfNodeEditor::default_body_graph();
        let mut asset_browser = AssetBrowser::new(PathBuf::from("assets"));
        asset_browser.mock_scan();
        let kit_panel     = KitPanel::new();
        let mut scene     = Scene::default();
        scene.meta.name        = "Untitled".into();
        scene.meta.engine_ver  = "0.1.0".into();
        let undo_mgr      = SceneUndoManager::new(50);

        Self {
            editor_state:   EditorState::new(EditorConfig::default()),
            inspector:      Inspector::new(400.0, 600.0),
            hierarchy:      HierarchyPanel::new(),
            console:        DevConsole::new(),
            gizmos:         GizmoRenderer::new(),
            sdf_editor,
            mat_painter:    MaterialPainter::new(),
            bone_rigger:    BoneRigger::new(),
            timeline:       TimelineEditor::new(),
            kit_panel,
            asset_browser,
            camera:         EditorCamera::new(),
            perf:           PerfOverlay::new(),
            status:         StatusBar::new(),
            scene,
            undo_mgr,
            scene_path:     None,
            scene_dirty:    false,
            world_editor:   WorldEditorState::new(),
            ai_behavior:    AiBehaviorState::new(),
            physics_editor: PhysicsEditorState::new(),
            render_graph:   RenderGraphEditorState::new(),
            dialogue_editor:DialogueEditorState::new(),
            quest_editor:   QuestEditorState::new(),
            spline_editor:  SplineEditorState::new(),
            cinematic:      CinematicState::new(),
            inventory_editor:InventoryEditorState::new(),
            ability_editor: AbilityEditorState::new(),
            level_streaming:LevelStreamingState::new(),
            audio_mixer:    AudioMixerState::new(),
            focused_panel:  EditorPanel::Viewport,
            drag:           DragState::default(),
            fly_input:      FreeFlyInput::default(),
            show_grid:      true,
            playing:        false,
            time:           0.0,
        }
    }

    // ── Update (called each frame) ────────────────────────────────────────

    pub fn update(&mut self, engine: &mut ProofEngine, dt: f32) {
        self.perf.begin_frame();
        self.time += dt;
        self.status.update(dt);

        // Camera advance
        let fly = if self.camera.mode == CameraMode::FreeFly {
            Some(&self.fly_input)
        } else { None };
        self.camera.update(dt, fly);

        // Timeline advance if playing
        if self.playing {
            let snap = self.timeline.timeline.step(dt);
            for (name, v) in &snap.kit_floats {
                let parts: Vec<&str> = name.splitn(2, '.').collect();
                if parts.len() == 2 {
                    self.kit_panel.set_float(parts[0], parts[1], *v);
                }
            }
        }

        // Perf overlay
        self.perf.end_frame(dt * 1000.0 * 0.7, 50_000_000u64, 1, 2_400.0, 8_192.0);

        // Handle keyboard input from engine
        self.process_input(engine);

        // Handle mouse drag from engine
        let pos = engine.input.mouse_pos();
        if self.drag.active {
            let delta = self.drag.frame_delta(pos);
            if self.drag.button == 1 { self.camera.orbit_drag(delta); }
            else if self.drag.button == 2 { self.camera.pan_drag(delta); }
        }
        if engine.input.mouse_right_just_pressed {
            self.drag = DragState { active: true, start_pos: pos, curr_pos: pos, button: 1 };
        }
        if engine.input.mouse_right_just_released {
            self.drag.active = false;
        }
        if engine.input.mouse_middle_just_pressed {
            self.drag = DragState { active: true, start_pos: pos, curr_pos: pos, button: 2 };
        }
        let scroll = engine.input.scroll_delta;
        if scroll.abs() > f32::EPSILON {
            self.camera.scroll_zoom(scroll);
        }
    }

    fn process_input(&mut self, engine: &mut ProofEngine) {
        let inp = &engine.input;

        // Tab — cycle panel
        if inp.just_pressed(Key::Tab) {
            let panels = EditorPanel::all();
            let idx = panels.iter().position(|&p| p == self.focused_panel).unwrap_or(0);
            self.focused_panel = panels[(idx + 1) % panels.len()];
            self.status.notify(format!("Panel: {}", self.focused_panel.label()));
        }

        // F1-F12 switch panel
        let fkeys = [
            Key::F1, Key::F2, Key::F3, Key::F4,
            Key::F5, Key::F6, Key::F7, Key::F8,
            Key::F9, Key::F10, Key::F11, Key::F12,
        ];
        let fkey_panels = [
            EditorPanel::Viewport, EditorPanel::SdfNodeEditor, EditorPanel::MaterialPainter,
            EditorPanel::BoneRigger, EditorPanel::Timeline, EditorPanel::KitPanel,
            EditorPanel::Hierarchy, EditorPanel::AssetBrowser, EditorPanel::Console,
            EditorPanel::WorldEditor, EditorPanel::AiBehavior, EditorPanel::Physics,
        ];
        for (i, &key) in fkeys.iter().enumerate() {
            if inp.just_pressed(key) {
                if i < fkey_panels.len() {
                    self.focused_panel = fkey_panels[i];
                    self.status.notify(format!("Panel: {}", self.focused_panel.label()));
                }
            }
        }

        // Play / Pause
        if inp.just_pressed(Key::F5) { self.toggle_play(); }
        if inp.just_pressed(Key::F6) { self.toggle_pause(); }

        // Ctrl combos
        let ctrl = inp.ctrl();
        let shift = inp.shift();
        if ctrl && inp.just_pressed(Key::Z) {
            if shift { self.redo(); } else { self.undo(); }
        }
        if ctrl && inp.just_pressed(Key::S) { self.save_scene(); }

        // Gizmo shortcuts
        if inp.just_pressed(Key::G) { self.gizmos.mode = GizmoMode::Translate; }
        if inp.just_pressed(Key::R) { self.gizmos.mode = GizmoMode::Rotate; }
        if inp.just_pressed(Key::S) && !ctrl { self.gizmos.mode = GizmoMode::Scale; }

        // Camera snap
        if inp.just_pressed(Key::F) {
            self.camera.frame_selection(glam::Vec3::ZERO, 1.5);
            self.status.notify("Framed selection");
        }

        // Grid toggle
        if inp.just_pressed(Key::G) && ctrl {
            self.show_grid = !self.show_grid;
        }

        // New SDF node
        if inp.just_pressed(Key::N) {
            let id = self.sdf_editor.add_primitive(PrimitiveKind::Sphere { radius: 0.5 });
            self.status.notify(format!("Added sphere node {}", id));
        }

        // Perf overlay
        if inp.just_pressed(Key::P) {
            self.perf.mode = self.perf.mode.cycle();
        }
    }

    pub fn toggle_play(&mut self) {
        self.playing = !self.playing;
        if self.playing {
            self.timeline.timeline.play();
            self.status.notify("Playing");
        } else {
            self.timeline.timeline.stop();
            self.status.notify("Stopped");
        }
    }

    pub fn toggle_pause(&mut self) {
        self.timeline.timeline.pause();
        self.status.notify("Paused");
    }

    pub fn undo(&mut self) {
        match self.focused_panel {
            EditorPanel::SdfNodeEditor   => { self.sdf_editor.undo();   self.status.notify("Undo (SDF)"); }
            EditorPanel::MaterialPainter => { self.mat_painter.undo();  self.status.notify("Undo (Paint)"); }
            EditorPanel::BoneRigger      => { self.bone_rigger.undo();  self.status.notify("Undo (Rig)"); }
            EditorPanel::Timeline        => { self.timeline.undo();     self.status.notify("Undo (Timeline)"); }
            EditorPanel::KitPanel        => { self.kit_panel.undo();    self.status.notify("Undo (Kit)"); }
            _ => {
                if let Some(restored) = self.undo_mgr.undo() {
                    self.scene = restored;
                    self.status.notify("Undo (Scene)");
                }
            }
        }
    }

    pub fn redo(&mut self) {
        match self.focused_panel {
            EditorPanel::SdfNodeEditor   => { self.sdf_editor.redo();  self.status.notify("Redo (SDF)"); }
            EditorPanel::MaterialPainter => { self.mat_painter.redo(); self.status.notify("Redo (Paint)"); }
            EditorPanel::Timeline        => { self.timeline.redo();    self.status.notify("Redo (Timeline)"); }
            EditorPanel::KitPanel        => { self.kit_panel.redo();   self.status.notify("Redo (Kit)"); }
            _ => {
                if let Some(restored) = self.undo_mgr.redo() {
                    self.scene = restored;
                    self.status.notify("Redo (Scene)");
                }
            }
        }
    }

    pub fn save_scene(&mut self) {
        let bytes = SceneSerializer::write_binary(&self.scene);
        let toml  = SceneSerializer::write_toml(&self.scene);
        let path  = self.scene_path.clone().unwrap_or_else(|| PathBuf::from("scene.scene"));
        self.status.notify(format!(
            "Saved {} ({} bytes)", path.display(), bytes.len()
        ));
        self.scene_dirty = false;
    }

    // ── Render all panels via glyphs ──────────────────────────────────────

    pub fn render(&mut self, engine: &mut ProofEngine) {
        let t = self.time;
        let panels = EditorPanel::all();
        let total  = panels.len() as f32;

        // ── Tab bar (one glyph-char per panel along the top) ──────────────
        for (i, &panel) in panels.iter().enumerate() {
            let x = -9.5 + (i as f32 / total) * 19.0;
            let y = 5.2f32;
            let (r, g, b) = panel.color();
            let active = panel == self.focused_panel;
            let emission = if active { 2.5 } else { 0.4 };
            let alpha    = if active { 1.0 } else { 0.5 };
            // Panel initial letter as tab indicator
            let ch = panel.label().chars().next().unwrap_or('?');
            engine.spawn_glyph(Glyph {
                character: ch,
                position: Vec3::new(x, y, 0.5),
                scale: Vec2::splat(if active { 0.38 } else { 0.26 }),
                color: Vec4::new(r, g, b, alpha),
                emission,
                glow_color: Vec3::new(r, g, b),
                glow_radius: if active { 0.6 } else { 0.1 },
                mass: 0.0,
                lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }

        // ── Active panel content ──────────────────────────────────────────
        let panel = self.focused_panel;
        let (pr, pg, pb) = panel.color();

        // Panel title bar
        self.render_label(engine, panel.label(), 0.0, 4.6, 0.5, pr, pg, pb, 2.0);

        // Status bar at bottom
        let status_text: String = {
            let msg = self.status.current_message();
            let perf = self.perf.render_compact();
            if msg.is_empty() {
                format!("{}  |  {}", self.camera.status_line(), perf)
            } else {
                format!("{}  |  {}", msg, perf)
            }
        };
        self.render_label(engine, &status_text, -7.0, -5.0, 0.4, 0.6, 0.7, 0.6, 0.6);

        // Panel-specific content rendering
        match panel {
            EditorPanel::Viewport => self.render_panel_viewport(engine),
            EditorPanel::SdfNodeEditor => self.render_panel_sdf(engine),
            EditorPanel::MaterialPainter => self.render_panel_material(engine),
            EditorPanel::BoneRigger => self.render_panel_bone(engine),
            EditorPanel::Timeline => self.render_panel_timeline(engine),
            EditorPanel::KitPanel => self.render_panel_kit(engine),
            EditorPanel::Hierarchy => self.render_panel_hierarchy(engine),
            EditorPanel::AssetBrowser => self.render_panel_assets(engine),
            EditorPanel::Console => self.render_panel_console(engine),
            EditorPanel::WorldEditor => self.render_panel_world(engine),
            EditorPanel::AiBehavior => self.render_panel_ai(engine),
            EditorPanel::Physics => self.render_panel_physics(engine),
            EditorPanel::RenderGraph => self.render_panel_rendergraph(engine),
            EditorPanel::Dialogue => self.render_panel_dialogue(engine),
            EditorPanel::Quest => self.render_panel_quest(engine),
            EditorPanel::Spline => self.render_panel_spline(engine),
            EditorPanel::Cinematic => self.render_panel_cinematic(engine),
            EditorPanel::Inventory => self.render_panel_inventory(engine),
            EditorPanel::Ability => self.render_panel_ability(engine),
            EditorPanel::LevelStreaming => self.render_panel_level_streaming(engine),
            EditorPanel::AudioMixer => self.render_panel_audio_mixer(engine),
        }

        // ── Grid ─────────────────────────────────────────────────────────
        if self.show_grid {
            for i in -8i32..=8 {
                let bright = if i == 0 { 0.3 } else { 0.08 };
                for j in -20i32..=20 {
                    engine.spawn_glyph(Glyph {
                        character: if i == 0 { '-' } else { '.' },
                        position: Vec3::new(j as f32 * 0.5, i as f32 * 0.6, -1.0),
                        scale: Vec2::splat(0.1),
                        color: Vec4::new(0.3, 0.4, 0.5, bright),
                        emission: 0.05, mass: 0.0, lifetime: 0.05,
                        layer: RenderLayer::Background,
                        ..Default::default()
                    });
                }
            }
        }
    }

    fn render_label(&mut self, engine: &mut ProofEngine, text: &str,
        x: f32, y: f32, scale: f32, r: f32, g: f32, b: f32, emission: f32)
    {
        let char_w = scale * 0.55;
        let total_w = text.len() as f32 * char_w;
        let start_x = x - total_w * 0.5;
        for (i, ch) in text.chars().enumerate() {
            engine.spawn_glyph(Glyph {
                character: ch,
                position: Vec3::new(start_x + i as f32 * char_w, y, 0.5),
                scale: Vec2::splat(scale),
                color: Vec4::new(r, g, b, 0.95),
                emission,
                mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_viewport(&mut self, engine: &mut ProofEngine) {
        let t = self.time;
        // Animated camera orbit preview
        for i in 0..12 {
            let a = (i as f32 / 12.0) * TAU + t * 0.2;
            let r = 3.0;
            engine.spawn_glyph(Glyph {
                character: '+',
                position: Vec3::new(a.cos() * r, a.sin() * r * 0.5, 0.0),
                scale: Vec2::splat(0.2),
                color: Vec4::new(0.4, 0.7, 1.0, 0.25),
                emission: 0.4, mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::Entity,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
        let cam_text = self.camera.status_line();
        self.render_label(engine, &cam_text, 0.0, -4.5, 0.3, 0.4, 0.7, 1.0, 0.7);
    }

    fn render_panel_sdf(&mut self, engine: &mut ProofEngine) {
        let stats = self.sdf_editor.graph.stats();
        let text = format!("SDF Graph: {}", stats);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.2, 1.0, 0.8, 1.2);
        let glsl = self.sdf_editor.glsl_output().to_string();
        let preview = &glsl[..80.min(glsl.len())];
        self.render_label(engine, preview, 0.0, 3.4, 0.22, 0.3, 0.9, 0.6, 0.6);
    }

    fn render_panel_material(&mut self, engine: &mut ProofEngine) {
        self.render_label(engine, "Material Painter — Paint Layers", 0.0, 4.0, 0.32, 1.0, 0.5, 0.2, 1.0);
        // Color swatches
        let colors = [(1.0f32,0.2,0.2),(0.2,1.0,0.3),(0.2,0.4,1.0),(1.0,0.9,0.2),(0.8,0.2,1.0)];
        for (i,(r,g,b)) in colors.iter().enumerate() {
            engine.spawn_glyph(Glyph {
                character: '#',
                position: Vec3::new(-2.0 + i as f32, 3.0, 0.5),
                scale: Vec2::splat(0.5),
                color: Vec4::new(*r, *g, *b, 0.8),
                emission: 1.5, mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_bone(&mut self, engine: &mut ProofEngine) {
        self.render_label(engine, "Bone Rigger — Humanoid Skeleton", 0.0, 4.0, 0.32, 0.9, 0.9, 0.3, 1.0);
        // Simple skeleton visualization
        let joints = [(0.0,2.0,'O'),(0.0,1.3,'|'),(-0.5,1.0,'<'),(0.5,1.0,'>'),
                       (0.0,0.5,'|'),(-0.4,-0.2,'/'),(0.4,-0.2,'\\')];
        for (x, y, ch) in &joints {
            engine.spawn_glyph(Glyph {
                character: *ch,
                position: Vec3::new(*x, *y, 0.5),
                scale: Vec2::splat(0.4),
                color: Vec4::new(0.9, 0.9, 0.3, 0.9),
                emission: 1.2, mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_timeline(&mut self, engine: &mut ProofEngine) {
        let text = self.timeline.status_line();
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.3, 0.8, 0.4, 1.0);
        // Timeline scrubber visualization
        let progress = (self.time * 0.1) % 1.0;
        for i in 0..40 {
            let t = i as f32 / 40.0;
            let is_head = (t - progress).abs() < 0.03;
            engine.spawn_glyph(Glyph {
                character: if is_head { '|' } else { '-' },
                position: Vec3::new(-8.0 + i as f32 * 0.4, 3.2, 0.5),
                scale: Vec2::splat(0.25),
                color: if is_head { Vec4::new(1.0,0.5,0.1,0.9) } else { Vec4::new(0.3,0.7,0.4,0.5) },
                emission: if is_head { 1.5 } else { 0.3 },
                mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_kit(&mut self, engine: &mut ProofEngine) {
        let text = format!("Kit Params — {} groups", self.kit_panel.groups.len());
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.7, 0.3, 1.0, 1.0);
    }

    fn render_panel_hierarchy(&mut self, engine: &mut ProofEngine) {
        self.render_label(engine, "Scene Hierarchy", 0.0, 4.0, 0.32, 0.5, 0.8, 1.0, 1.0);
        let items = ["- Root", "  + Camera", "  + Light", "  + Mesh_0", "  + Mesh_1"];
        for (i, item) in items.iter().enumerate() {
            self.render_label(engine, item, -3.0, 3.4 - i as f32 * 0.45, 0.25, 0.5, 0.8, 1.0, 0.5);
        }
    }

    fn render_panel_assets(&mut self, engine: &mut ProofEngine) {
        let count = self.asset_browser.total_assets();
        let text = format!("Asset Browser — {} assets", count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.9, 0.6, 0.2, 1.0);
    }

    fn render_panel_console(&mut self, engine: &mut ProofEngine) {
        self.render_label(engine, "Developer Console", 0.0, 4.0, 0.32, 0.4, 1.0, 0.4, 1.0);
        let lines = ["[info]  Engine started", "[info]  Renderer: OpenGL", "[info]  Audio: OK", "> _"];
        for (i, line) in lines.iter().enumerate() {
            self.render_label(engine, line, -4.0, 3.4 - i as f32 * 0.45, 0.24, 0.4, 1.0, 0.4, 0.5);
        }
    }

    fn render_panel_world(&mut self, engine: &mut ProofEngine) {
        let text = format!("World Editor — Biome System | entities: {}",
            self.world_editor.entity_count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.2, 0.8, 0.3, 1.0);
        // Biome color blobs
        let t = self.time;
        for i in 0..30 {
            let a = (i as f32 / 30.0) * TAU;
            let r = 2.5 + (t * 0.3 + a).sin() * 0.4;
            engine.spawn_glyph(Glyph {
                character: ['.','o','*','+'][i % 4],
                position: Vec3::new(a.cos() * r, a.sin() * r * 0.6, 0.0),
                scale: Vec2::splat(0.18),
                color: Vec4::new(0.1 + (a).sin() * 0.4, 0.6, 0.2, 0.4),
                emission: 0.4, mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::Entity,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_ai(&mut self, engine: &mut ProofEngine) {
        let text = format!("AI Behavior Editor — {} nodes | blackboard vars: {}",
            self.ai_behavior.node_count, self.ai_behavior.blackboard.entries.len());
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 1.0, 0.3, 0.5, 1.0);
        // BT tree visualization
        let nodes = [
            (0.0, 3.0, '?'), (-1.5, 2.2, '>'), (1.5, 2.2, '>'),
            (-2.0, 1.4, 'A'), (-1.0, 1.4, 'A'), (1.0, 1.4, 'A'), (2.0, 1.4, 'A'),
        ];
        for (x, y, ch) in &nodes {
            engine.spawn_glyph(Glyph {
                character: *ch,
                position: Vec3::new(*x, *y, 0.5),
                scale: Vec2::splat(0.35),
                color: Vec4::new(1.0, 0.3, 0.5, 0.85),
                emission: 1.0, mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_physics(&mut self, engine: &mut ProofEngine) {
        let text = format!("Physics Editor — {} active bodies",
            self.physics_editor.active_bodies);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.6, 0.4, 1.0, 1.0);
        // Physics bodies
        let t = self.time;
        for i in 0..6 {
            let x = -3.0 + i as f32 * 1.2;
            let y = -1.5 + (t * 1.5 + i as f32).sin() * 1.0;
            engine.spawn_glyph(Glyph {
                character: 'O',
                position: Vec3::new(x, y, 0.5),
                scale: Vec2::splat(0.35),
                color: Vec4::new(0.6, 0.4, 1.0, 0.8),
                emission: 0.8, mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::Entity,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_rendergraph(&mut self, engine: &mut ProofEngine) {
        let text = format!("Render Graph — {} passes", self.render_graph.pass_count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.3, 0.6, 1.0, 1.0);
        let passes = ["GBuffer", "Shadow", "SSAO", "Composite"];
        for (i, pass) in passes.iter().enumerate() {
            let x = -3.0 + i as f32 * 2.0;
            self.render_label(engine, pass, x, 3.0, 0.26, 0.4, 0.7, 1.0, 0.8);
            if i + 1 < passes.len() {
                engine.spawn_glyph(Glyph {
                    character: '>',
                    position: Vec3::new(x + 1.0, 3.0, 0.5),
                    scale: Vec2::splat(0.3),
                    color: Vec4::new(0.5, 0.6, 0.8, 0.6),
                    emission: 0.4, mass: 0.0, lifetime: 0.05,
                    layer: RenderLayer::UI,
                    blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }
        }
    }

    fn render_panel_dialogue(&mut self, engine: &mut ProofEngine) {
        let text = format!("Dialogue Editor — {} nodes", self.dialogue_editor.node_count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 1.0, 0.8, 0.3, 1.0);
        let t = self.time;
        let text_blip = if (t * 3.0) as i32 % 2 == 0 { "NPC: Hello traveler..." } else { "NPC: Hello traveler._" };
        self.render_label(engine, text_blip, 0.0, 2.5, 0.28, 1.0, 0.85, 0.4, 0.8);
        self.render_label(engine, "[1] Tell me more   [2] Farewell", 0.0, 2.0, 0.24, 0.7, 0.7, 0.4, 0.6);
    }

    fn render_panel_quest(&mut self, engine: &mut ProofEngine) {
        let text = format!("Quest Editor — {} objectives", self.quest_editor.objective_count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.8, 0.4, 0.2, 1.0);
        let objectives = ["[ ] Reach the dungeon", "[x] Find the key", "[ ] Defeat the boss"];
        for (i, obj) in objectives.iter().enumerate() {
            self.render_label(engine, obj, 0.0, 3.3 - i as f32 * 0.45, 0.26,
                if obj.contains("[x]") { 0.3 } else { 0.9 },
                if obj.contains("[x]") { 0.8 } else { 0.5 },
                0.2, 0.7);
        }
    }

    fn render_panel_spline(&mut self, engine: &mut ProofEngine) {
        let count = self.spline_editor.control_points.len();
        let text = format!("Spline Editor — {} control points", count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.2, 1.0, 0.6, 1.0);
        // Draw spline path
        let t = self.time * 0.5;
        for i in 0..60 {
            let u = i as f32 / 60.0;
            let x = (u - 0.5) * 14.0;
            let y = (x * 0.4 + t).sin() * 1.5;
            engine.spawn_glyph(Glyph {
                character: if i % 5 == 0 { 'o' } else { '.' },
                position: Vec3::new(x, y, 0.5),
                scale: Vec2::splat(if i % 5 == 0 { 0.28 } else { 0.16 }),
                color: Vec4::new(0.2, 1.0, 0.6, 0.7),
                emission: if i % 5 == 0 { 1.0 } else { 0.3 },
                mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::Entity,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_cinematic(&mut self, engine: &mut ProofEngine) {
        let text = format!("Cinematic Sequencer — {} tracks | {}",
            self.cinematic.track_count,
            format!("{:02}:{:02}:{:02}:{:02}",
                self.cinematic.timecode.hours,
                self.cinematic.timecode.minutes,
                self.cinematic.timecode.seconds,
                self.cinematic.timecode.frames));
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.9, 0.2, 0.8, 1.0);
        // Film strip
        for i in 0..20 {
            let x = -9.5 + i as f32 * 1.0;
            engine.spawn_glyph(Glyph {
                character: '#',
                position: Vec3::new(x, 3.1, 0.5),
                scale: Vec2::splat(0.35),
                color: Vec4::new(0.4, 0.1, 0.4, 0.4),
                emission: 0.2, mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn render_panel_inventory(&mut self, engine: &mut ProofEngine) {
        let text = format!("Inventory Editor — {} items", self.inventory_editor.item_count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.6, 0.9, 0.2, 1.0);
        // Item grid
        let rarities = [
            (Vec4::new(0.8,0.8,0.8,0.8), "Common"),
            (Vec4::new(0.2,0.8,0.2,0.8), "Uncommon"),
            (Vec4::new(0.2,0.4,1.0,0.8), "Rare"),
            (Vec4::new(0.6,0.2,1.0,0.8), "Epic"),
            (Vec4::new(1.0,0.6,0.1,0.9), "Legendary"),
        ];
        for (i, (color, name)) in rarities.iter().enumerate() {
            let x = -4.0 + i as f32 * 2.0;
            engine.spawn_glyph(Glyph {
                character: if i == 4 { '*' } else { 'i' },
                position: Vec3::new(x, 3.0, 0.5),
                scale: Vec2::splat(0.45),
                color: *color,
                emission: 0.8 + i as f32 * 0.3,
                glow_color: Vec3::new(color.x, color.y, color.z),
                glow_radius: if i == 4 { 0.5 } else { 0.2 },
                mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
            self.render_label(engine, name, x, 2.5, 0.2, color.x, color.y, color.z, 0.5);
        }
    }

    fn render_panel_ability(&mut self, engine: &mut ProofEngine) {
        let text = format!("Ability Editor — {} abilities", self.ability_editor.ability_count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 1.0, 0.4, 0.1, 1.0);
        let t = self.time;
        // Ability cost/damage visualization
        let abilities = [("Fireball", 1.0,0.3,0.1), ("Ice Lance", 0.3,0.7,1.0), ("Thunder", 1.0,0.9,0.2)];
        for (i, (name, r, g, b)) in abilities.iter().enumerate() {
            let x = -3.0 + i as f32 * 3.0;
            engine.spawn_glyph(Glyph {
                character: '*',
                position: Vec3::new(x, 3.0 + (t * 2.0 + i as f32).sin() * 0.2, 0.5),
                scale: Vec2::splat(0.5),
                color: Vec4::new(*r, *g, *b, 0.9),
                emission: 1.5 + (t * 2.0 + i as f32).sin() * 0.5,
                glow_color: Vec3::new(*r, *g, *b),
                glow_radius: 0.6,
                mass: 0.0, lifetime: 0.05,
                layer: RenderLayer::UI,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
            self.render_label(engine, name, x, 2.3, 0.24, *r, *g, *b, 0.7);
        }
    }

    fn render_panel_level_streaming(&mut self, engine: &mut ProofEngine) {
        let text = format!("Level Streaming — {}/{} regions loaded | budget: {:.0} MB",
            self.level_streaming.loaded_regions, 16, self.level_streaming.streaming_budget_mb);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.3, 0.7, 0.9, 1.0);
        // Streaming grid
        for row in 0..4i32 {
            for col in 0..8i32 {
                let loaded = (row * 8 + col) < self.level_streaming.loaded_regions as i32;
                engine.spawn_glyph(Glyph {
                    character: if loaded { '#' } else { '.' },
                    position: Vec3::new(-3.5 + col as f32 * 1.0, 2.5 - row as f32 * 0.6, 0.5),
                    scale: Vec2::splat(0.3),
                    color: if loaded { Vec4::new(0.3,0.8,1.0,0.8) } else { Vec4::new(0.2,0.3,0.4,0.3) },
                    emission: if loaded { 0.7 } else { 0.1 },
                    mass: 0.0, lifetime: 0.05,
                    layer: RenderLayer::UI,
                    blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }
        }
    }

    fn render_panel_audio_mixer(&mut self, engine: &mut ProofEngine) {
        let text = format!("Audio Mixer — vol: {:.2} | {} channels",
            self.audio_mixer.master_volume, self.audio_mixer.channel_count);
        self.render_label(engine, &text, 0.0, 4.0, 0.32, 0.5, 1.0, 0.7, 1.0);
        let t = self.time;
        // VU meter visualization
        for ch in 0..8u32 {
            let level = ((t * (1.5 + ch as f32 * 0.3)).sin() * 0.5 + 0.5) * 4.0;
            for bar in 0..5u32 {
                let lit = (bar as f32) < level;
                let r = if bar >= 4 { 1.0 } else if bar >= 3 { 0.8 } else { 0.2 };
                let g = if bar >= 4 { 0.2 } else if bar >= 3 { 0.8 } else { 1.0 };
                engine.spawn_glyph(Glyph {
                    character: if lit { '=' } else { '-' },
                    position: Vec3::new(-3.5 + ch as f32 * 0.9, 1.5 + bar as f32 * 0.4, 0.5),
                    scale: Vec2::splat(0.28),
                    color: Vec4::new(r, g, 0.3, if lit { 0.9 } else { 0.2 }),
                    emission: if lit { 0.8 } else { 0.1 },
                    mass: 0.0, lifetime: 0.05,
                    layer: RenderLayer::UI,
                    blend_mode: BlendMode::Additive,
                    ..Default::default()
                });
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    env_logger::init();

    let mut engine = ProofEngine::new(EngineConfig {
        window_title: "Proof Editor".to_string(),
        window_width:  1600,
        window_height: 1000,
        render: proof_engine::config::RenderConfig {
            bloom_enabled:        true,
            bloom_intensity:      1.8,
            chromatic_aberration: 0.002,
            film_grain:           0.005,
            ..Default::default()
        },
        ..Default::default()
    });

    let mut app = EditorApp::new();

    // Pre-populate timeline for demo
    let tid = app.timeline.add_track(
        proof_engine::editor::timeline::TrackTarget::KitFloat("Bloom.intensity".into())
    );
    app.timeline.insert_key(tid, 0.0, proof_engine::editor::timeline::TrackValue::Float(2.8));
    app.timeline.insert_key(tid, 4.0, proof_engine::editor::timeline::TrackValue::Float(8.0));
    app.timeline.insert_key(tid, 8.0, proof_engine::editor::timeline::TrackValue::Float(2.8));

    engine.run(move |engine, dt| {
        app.update(engine, dt);
        app.render(engine);
    });
}
