//! Editor application — owns all state, dispatches input, orchestrates panels.

use proof_engine::prelude::*;
use proof_engine::input::Key;
use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

use crate::panels::PanelManager;
use crate::tools::{ToolManager, ToolKind};
use crate::scene::SceneDocument;
use crate::ui::UiRenderer;
use crate::viewport::ViewportState;
use crate::commands::CommandHistory;
use crate::hotkeys::HotkeyMap;
use crate::clipboard::Clipboard;
use crate::preferences::EditorPrefs;
use crate::layout::LayoutManager;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Editor mode
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    /// Scene is live and running.
    Play,
    /// Scene is paused, user can edit.
    Edit,
    /// Frame-by-frame stepping.
    Step,
}

impl EditorMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Play => "PLAY",
            Self::Edit => "EDIT",
            Self::Step => "STEP",
        }
    }
    pub fn color(self) -> Vec4 {
        match self {
            Self::Play => Vec4::new(0.2, 1.0, 0.4, 1.0),
            Self::Edit => Vec4::new(0.3, 0.6, 1.0, 1.0),
            Self::Step => Vec4::new(1.0, 0.8, 0.2, 1.0),
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Editor notification
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Debug, Clone)]
pub struct Notification {
    pub text: String,
    pub color: Vec4,
    pub remaining: f32,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// EditorApp — the main struct
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct EditorApp {
    // ── Core state ──
    pub mode: EditorMode,
    pub document: SceneDocument,
    pub viewport: ViewportState,
    pub panels: PanelManager,
    pub tools: ToolManager,
    pub ui: UiRenderer,
    pub commands: CommandHistory,
    pub hotkeys: HotkeyMap,
    pub clipboard: Clipboard,
    pub prefs: EditorPrefs,
    pub layout: LayoutManager,

    // ── Status ──
    pub fps: f32,
    pub frame_count: u64,
    pub time: f32,
    pub notifications: Vec<Notification>,
    pub show_grid: bool,
    pub show_gizmos: bool,
    pub show_debug: bool,
    pub show_stats: bool,
    pub show_help: bool,

    // ── Accumulation for FPS ──
    fps_timer: f32,
    fps_frames: u32,
}

impl EditorApp {
    pub fn new() -> Self {
        Self {
            mode: EditorMode::Edit,
            document: SceneDocument::new(),
            viewport: ViewportState::new(),
            panels: PanelManager::new(),
            tools: ToolManager::new(),
            ui: UiRenderer::new(),
            commands: CommandHistory::new(200),
            hotkeys: HotkeyMap::defaults(),
            clipboard: Clipboard::new(),
            prefs: EditorPrefs::default(),
            layout: LayoutManager::new(1600.0, 1000.0),
            fps: 60.0,
            frame_count: 0,
            time: 0.0,
            notifications: Vec::new(),
            show_grid: true,
            show_gizmos: true,
            show_debug: false,
            show_stats: true,
            show_help: false,
            fps_timer: 0.0,
            fps_frames: 0,
        }
    }

    pub fn init(&mut self, engine: &mut ProofEngine) {
        self.notify("Proof Editor ready. Press F1 for help.", Vec4::new(0.5, 0.8, 1.0, 1.0));
        self.spawn_grid(engine);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Main update loop
    // ════════════════════════════════════════════════════════════════════════

    pub fn update(&mut self, engine: &mut ProofEngine, dt: f32) {
        self.time += dt;
        self.frame_count += 1;
        self.update_fps(dt);

        // Clone input to avoid borrow conflicts
        let input = engine.input.clone();

        // Process global hotkeys
        self.process_hotkeys(&input, engine);

        // Update viewport (camera, picking)
        self.viewport.update(&input, dt, &self.layout);

        // Apply viewport camera to engine
        engine.camera.position.x.target = self.viewport.cam_x;
        engine.camera.position.y.target = self.viewport.cam_y;

        // Update tools
        if self.mode == EditorMode::Edit {
            let tool_events = self.tools.update(&input, &self.viewport, &self.document, &self.layout);
            for event in tool_events {
                self.process_tool_event(event, engine);
            }
        }

        // Update panels
        self.panels.update(&input, &self.document, &self.tools, &self.layout);

        // Tick notifications
        self.notifications.retain_mut(|n| {
            n.remaining -= dt;
            n.remaining > 0.0
        });

        // Render UI
        self.render_ui(engine, dt);
    }

    // ════════════════════════════════════════════════════════════════════════
    // Hotkey processing
    // ════════════════════════════════════════════════════════════════════════

    fn process_hotkeys(&mut self, input: &proof_engine::input::InputState, engine: &mut ProofEngine) {
        let ctrl = input.ctrl();
        let shift = input.shift();

        // F1 — Help overlay
        if input.just_pressed(Key::F1) {
            self.show_help = !self.show_help;
        }

        // F2 — Toggle stats
        if input.just_pressed(Key::F2) {
            self.show_stats = !self.show_stats;
        }

        // F3 — Toggle grid
        if input.just_pressed(Key::F3) {
            self.show_grid = !self.show_grid;
        }

        // F4 — Toggle gizmos
        if input.just_pressed(Key::F4) {
            self.show_gizmos = !self.show_gizmos;
        }

        // F5 — Play/Edit toggle
        if input.just_pressed(Key::F5) {
            self.mode = match self.mode {
                EditorMode::Edit => {
                    self.notify("PLAY", Vec4::new(0.2, 1.0, 0.4, 1.0));
                    EditorMode::Play
                }
                _ => {
                    self.notify("EDIT", Vec4::new(0.3, 0.6, 1.0, 1.0));
                    EditorMode::Edit
                }
            };
        }

        // F6 — Step one frame
        if input.just_pressed(Key::F6) {
            self.mode = EditorMode::Step;
            self.notify("STEP", Vec4::new(1.0, 0.8, 0.2, 1.0));
        }

        // Ctrl+Z — Undo
        if ctrl && input.just_pressed(Key::Z) && !shift {
            if let Some(name) = self.commands.undo() {
                self.notify(&format!("Undo: {}", name), Vec4::new(0.8, 0.8, 0.2, 1.0));
            }
        }

        // Ctrl+Shift+Z or Ctrl+Y — Redo
        if (ctrl && shift && input.just_pressed(Key::Z)) || (ctrl && input.just_pressed(Key::Y)) {
            if let Some(name) = self.commands.redo() {
                self.notify(&format!("Redo: {}", name), Vec4::new(0.2, 0.8, 0.8, 1.0));
            }
        }

        // Ctrl+S — Save
        if ctrl && input.just_pressed(Key::S) {
            let path = self.document.path.clone().unwrap_or_else(|| "scene.json".to_string());
            match self.document.save(&path) {
                Ok(_) => self.notify(&format!("Saved: {}", path), Vec4::new(0.2, 1.0, 0.4, 1.0)),
                Err(e) => self.notify(&format!("Save failed: {}", e), Vec4::new(1.0, 0.3, 0.2, 1.0)),
            }
        }

        // Ctrl+O — Load
        if ctrl && input.just_pressed(Key::O) {
            match SceneDocument::load("scene.json") {
                Ok(doc) => {
                    self.document = doc;
                    self.rebuild_scene(engine);
                    self.notify("Loaded: scene.json", Vec4::new(0.2, 1.0, 0.4, 1.0));
                }
                Err(e) => self.notify(&format!("Load failed: {}", e), Vec4::new(1.0, 0.3, 0.2, 1.0)),
            }
        }

        // Ctrl+N — New scene
        if ctrl && input.just_pressed(Key::N) {
            self.document = SceneDocument::new();
            engine.scene = SceneGraph::new();
            self.spawn_grid(engine);
            self.notify("New scene", Vec4::new(0.5, 0.8, 1.0, 1.0));
        }

        // Delete — Delete selection
        if input.just_pressed(Key::Delete) {
            let count = self.document.selection.len();
            if count > 0 {
                for &id in &self.document.selection.clone() {
                    self.document.remove_node(id);
                }
                self.document.selection.clear();
                self.rebuild_scene(engine);
                self.notify(&format!("Deleted {} items", count), Vec4::new(1.0, 0.5, 0.2, 1.0));
            }
        }

        // Ctrl+D — Duplicate selection
        if ctrl && input.just_pressed(Key::D) {
            let to_dupe: Vec<u32> = self.document.selection.clone();
            let mut new_ids = Vec::new();
            for &id in &to_dupe {
                if let Some(new_id) = self.document.duplicate_node(id) {
                    new_ids.push(new_id);
                }
            }
            if !new_ids.is_empty() {
                self.document.selection = new_ids;
                self.rebuild_scene(engine);
                self.notify(&format!("Duplicated {} items", to_dupe.len()), Vec4::new(0.5, 0.8, 1.0, 1.0));
            }
        }

        // Ctrl+A — Select all
        if ctrl && input.just_pressed(Key::A) {
            self.document.select_all();
            self.notify("Selected all", Vec4::new(0.5, 0.8, 1.0, 1.0));
        }

        // Escape — Deselect / cancel
        if input.just_pressed(Key::Escape) {
            if !self.document.selection.is_empty() {
                self.document.selection.clear();
            } else if self.show_help {
                self.show_help = false;
            }
        }

        // Tool shortcuts
        if input.just_pressed(Key::V) && !ctrl { self.tools.set_tool(ToolKind::Select); }
        if input.just_pressed(Key::G) && !ctrl { self.tools.set_tool(ToolKind::Move); }
        if input.just_pressed(Key::R) && !ctrl { self.tools.set_tool(ToolKind::Rotate); }
        if input.just_pressed(Key::T) && !ctrl { self.tools.set_tool(ToolKind::Scale); }
        if input.just_pressed(Key::P) && !ctrl { self.tools.set_tool(ToolKind::Place); }
        if input.just_pressed(Key::F) && !ctrl { self.tools.set_tool(ToolKind::Field); }
        if input.just_pressed(Key::E) && !ctrl { self.tools.set_tool(ToolKind::Entity); }
        if input.just_pressed(Key::X) && !ctrl { self.tools.set_tool(ToolKind::Particle); }

        // Space — shake preview
        if input.just_pressed(Key::Space) {
            engine.add_trauma(0.3);
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Tool events
    // ════════════════════════════════════════════════════════════════════════

    fn process_tool_event(&mut self, event: crate::tools::ToolEvent, engine: &mut ProofEngine) {
        match event {
            crate::tools::ToolEvent::PlaceGlyph { position, character, color, emission, glow_radius } => {
                let node_id = self.document.add_glyph_node(position, character, color, emission, glow_radius);
                self.spawn_document_glyph(engine, node_id);
                self.notify(&format!("Placed '{}'", character), Vec4::new(0.5, 1.0, 0.5, 1.0));
            }
            crate::tools::ToolEvent::PlaceField { position, field_type } => {
                let node_id = self.document.add_field_node(position, field_type);
                self.spawn_document_field(engine, node_id);
                self.notify(&format!("Placed field: {}", field_type.label()), Vec4::new(1.0, 0.7, 0.2, 1.0));
            }
            crate::tools::ToolEvent::PlaceEntity { position } => {
                let node_id = self.document.add_entity_node(position);
                self.spawn_document_entity(engine, node_id);
                self.notify("Placed entity", Vec4::new(0.6, 0.3, 1.0, 1.0));
            }
            crate::tools::ToolEvent::PlaceParticleBurst { position, color } => {
                engine.emit_particles(
                    proof_engine::particle::EmitterPreset::DeathExplosion { color },
                    position,
                );
                self.notify("Particle burst", Vec4::new(1.0, 0.4, 0.6, 1.0));
            }
            crate::tools::ToolEvent::MoveSelection { delta } => {
                let ids: Vec<u32> = self.document.selection.clone();
                for id in ids {
                    self.document.translate_node(id, delta);
                }
                self.rebuild_scene(engine);
            }
            crate::tools::ToolEvent::Select { node_id, additive } => {
                if additive {
                    self.document.toggle_selection(node_id);
                } else {
                    self.document.selection = vec![node_id];
                }
            }
            crate::tools::ToolEvent::BoxSelect { ids } => {
                self.document.selection = ids;
            }
            crate::tools::ToolEvent::Deselect => {
                self.document.selection.clear();
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Scene spawning
    // ════════════════════════════════════════════════════════════════════════

    fn spawn_document_glyph(&self, engine: &mut ProofEngine, node_id: u32) {
        if let Some(node) = self.document.get_node(node_id) {
            engine.spawn_glyph(Glyph {
                character: node.character.unwrap_or('@'),
                position: node.position,
                color: node.color,
                emission: node.emission,
                glow_color: Vec3::new(node.color.x, node.color.y, node.color.z),
                glow_radius: node.glow_radius,
                mass: 0.1,
                layer: RenderLayer::Entity,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            });
        }
    }

    fn spawn_document_field(&self, engine: &mut ProofEngine, node_id: u32) {
        if let Some(node) = self.document.get_node(node_id) {
            if let Some(ref ft) = node.field_type {
                engine.add_field(ft.to_force_field(node.position));
            }
        }
    }

    fn spawn_document_entity(&self, engine: &mut ProofEngine, node_id: u32) {
        if let Some(node) = self.document.get_node(node_id) {
            let mut entity = AmorphousEntity::new("Editor Entity", node.position);
            entity.entity_mass = 3.0;
            entity.cohesion = 0.7;
            entity.pulse_rate = 0.5;
            entity.pulse_depth = 0.15;
            entity.hp = 100.0;
            entity.max_hp = 100.0;

            let n = 12;
            let chars = ['@', '#', '*', '+', 'o', 'x', 'X', 'O', '.', ':', '~', '='];
            for i in 0..n {
                let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
                let r = 0.8;
                entity.formation.push(Vec3::new(angle.cos() * r, angle.sin() * r, 0.0));
                entity.formation_chars.push(chars[i % chars.len()]);
                entity.formation_colors.push(node.color);
            }
            engine.spawn_entity(entity);
        }
    }

    fn rebuild_scene(&self, engine: &mut ProofEngine) {
        engine.scene = SceneGraph::new();
        self.spawn_grid_on(engine);
        for node in self.document.nodes() {
            match node.kind {
                crate::scene::NodeKind::Glyph => { self.spawn_document_glyph(engine, node.id); }
                crate::scene::NodeKind::Field => { self.spawn_document_field(engine, node.id); }
                crate::scene::NodeKind::Entity => { self.spawn_document_entity(engine, node.id); }
                _ => {}
            }
        }
    }

    fn spawn_grid(&self, engine: &mut ProofEngine) {
        self.spawn_grid_on(engine);
    }

    fn spawn_grid_on(&self, engine: &mut ProofEngine) {
        if !self.show_grid { return; }
        for y in -25..=25 {
            for x in -35..=35 {
                let on_axis = x == 0 || y == 0;
                let on_major = x % 5 == 0 && y % 5 == 0;
                let ch = if on_axis && on_major { '+' }
                    else if on_axis { '-' }
                    else if on_major { '.' }
                    else if (x + y) % 4 == 0 { '.' }
                    else { continue };

                let brightness = if on_axis { 0.15 } else if on_major { 0.08 } else { 0.03 };
                let color = if x == 0 && y == 0 {
                    Vec4::new(1.0, 1.0, 0.3, 0.4) // origin
                } else if on_axis {
                    Vec4::new(0.2, 0.3, 0.5, 0.25) // axis lines
                } else {
                    Vec4::new(0.15, 0.15, 0.2, 0.12) // grid dots
                };

                engine.spawn_glyph(Glyph {
                    character: ch,
                    position: Vec3::new(x as f32, y as f32, -2.0),
                    color,
                    emission: brightness,
                    layer: RenderLayer::Background,
                    ..Default::default()
                });
            }
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // UI rendering
    // ════════════════════════════════════════════════════════════════════════

    fn render_ui(&mut self, engine: &mut ProofEngine, dt: f32) {
        let cam_x = self.viewport.cam_x;
        let cam_y = self.viewport.cam_y;

        // Menu bar
        self.render_menu_bar(engine, cam_x, cam_y);

        // Left panel — hierarchy
        self.render_hierarchy(engine, cam_x, cam_y);

        // Right panel — inspector
        self.render_inspector(engine, cam_x, cam_y);

        // Bottom panel — tool shelf + status
        self.render_bottom_bar(engine, cam_x, cam_y);

        // Status bar
        self.render_status(engine, cam_x, cam_y);

        // Notifications
        self.render_notifications(engine, cam_x, cam_y, dt);

        // Help overlay
        if self.show_help {
            self.render_help(engine, cam_x, cam_y);
        }
    }

    fn render_menu_bar(&self, engine: &mut ProofEngine, cx: f32, cy: f32) {
        let y = cy + 11.5;
        let x = cx - 17.0;

        let items = [
            ("File", "Ctrl+N/O/S"),
            ("Edit", "Ctrl+Z/Y/D"),
            ("View", "F1-F4"),
            ("Tools", "V/G/R/T/P/F/E/X"),
            ("Scene", "F5=Play F6=Step"),
        ];

        let mut offset = 0.0;
        for (label, _shortcut) in &items {
            self.ui.draw_text(engine, x + offset, y, label,
                Vec4::new(0.7, 0.7, 0.8, 0.8), 0.2, RenderLayer::UI);
            offset += label.len() as f32 * 0.45 + 1.0;
        }

        // Mode indicator
        let mode_x = cx + 12.0;
        self.ui.draw_text(engine, mode_x, y, self.mode.label(),
            self.mode.color(), 0.5, RenderLayer::UI);
    }

    fn render_hierarchy(&self, engine: &mut ProofEngine, cx: f32, cy: f32) {
        let x = cx - 17.0;
        let mut y = cy + 10.0;

        self.ui.draw_text(engine, x, y, "HIERARCHY",
            Vec4::new(0.5, 0.7, 1.0, 0.9), 0.3, RenderLayer::UI);
        y -= 0.6;
        self.ui.draw_text(engine, x, y, "----------",
            Vec4::new(0.3, 0.3, 0.4, 0.5), 0.1, RenderLayer::UI);
        y -= 0.6;

        for node in self.document.nodes().take(20) {
            let selected = self.document.selection.contains(&node.id);
            let icon = match node.kind {
                crate::scene::NodeKind::Glyph => "@",
                crate::scene::NodeKind::Field => "~",
                crate::scene::NodeKind::Entity => "#",
                crate::scene::NodeKind::Group => ">",
                crate::scene::NodeKind::Camera => "C",
            };
            let color = if selected {
                Vec4::new(1.0, 0.9, 0.3, 1.0)
            } else {
                Vec4::new(0.6, 0.6, 0.7, 0.7)
            };
            let label = format!("{} {}", icon, node.name);
            self.ui.draw_text(engine, x, y, &label, color, if selected { 0.3 } else { 0.1 }, RenderLayer::UI);
            y -= 0.5;
        }

        let total = self.document.node_count();
        if total > 20 {
            self.ui.draw_text(engine, x, y, &format!("... +{} more", total - 20),
                Vec4::new(0.4, 0.4, 0.5, 0.5), 0.1, RenderLayer::UI);
        }
    }

    fn render_inspector(&self, engine: &mut ProofEngine, cx: f32, cy: f32) {
        let x = cx + 10.0;
        let mut y = cy + 10.0;

        self.ui.draw_text(engine, x, y, "INSPECTOR",
            Vec4::new(0.5, 0.7, 1.0, 0.9), 0.3, RenderLayer::UI);
        y -= 0.6;
        self.ui.draw_text(engine, x, y, "----------",
            Vec4::new(0.3, 0.3, 0.4, 0.5), 0.1, RenderLayer::UI);
        y -= 0.6;

        if let Some(&id) = self.document.selection.first() {
            if let Some(node) = self.document.get_node(id) {
                self.ui.draw_text(engine, x, y, &format!("Name: {}", node.name),
                    Vec4::new(0.8, 0.8, 0.9, 0.9), 0.2, RenderLayer::UI);
                y -= 0.5;
                self.ui.draw_text(engine, x, y, &format!("Type: {:?}", node.kind),
                    Vec4::new(0.6, 0.7, 0.8, 0.8), 0.1, RenderLayer::UI);
                y -= 0.5;
                self.ui.draw_text(engine, x, y, &format!("Pos: ({:.1}, {:.1})",
                    node.position.x, node.position.y),
                    Vec4::new(0.6, 0.7, 0.8, 0.8), 0.1, RenderLayer::UI);
                y -= 0.5;
                if let Some(ch) = node.character {
                    self.ui.draw_text(engine, x, y, &format!("Char: '{}'", ch),
                        Vec4::new(0.6, 0.7, 0.8, 0.8), 0.1, RenderLayer::UI);
                    y -= 0.5;
                }
                self.ui.draw_text(engine, x, y, &format!("Emission: {:.2}", node.emission),
                    Vec4::new(0.6, 0.7, 0.8, 0.8), 0.1, RenderLayer::UI);
                y -= 0.5;
                self.ui.draw_text(engine, x, y, &format!("Color: ({:.1},{:.1},{:.1})",
                    node.color.x, node.color.y, node.color.z),
                    node.color, 0.2, RenderLayer::UI);
            }
        } else {
            self.ui.draw_text(engine, x, y, "No selection",
                Vec4::new(0.4, 0.4, 0.5, 0.5), 0.1, RenderLayer::UI);
        }
    }

    fn render_bottom_bar(&self, engine: &mut ProofEngine, cx: f32, cy: f32) {
        let y = cy - 10.5;
        let mut x = cx - 17.0;

        // Tool buttons
        let tools = [
            ("V", "Select", ToolKind::Select),
            ("G", "Move", ToolKind::Move),
            ("R", "Rotate", ToolKind::Rotate),
            ("T", "Scale", ToolKind::Scale),
            ("P", "Place", ToolKind::Place),
            ("F", "Field", ToolKind::Field),
            ("E", "Entity", ToolKind::Entity),
            ("X", "Burst", ToolKind::Particle),
        ];

        for (key, name, kind) in &tools {
            let active = self.tools.current() == *kind;
            let color = if active {
                Vec4::new(1.0, 0.9, 0.3, 1.0)
            } else {
                Vec4::new(0.5, 0.5, 0.6, 0.6)
            };
            let label = format!("[{}]{}", key, name);
            self.ui.draw_text(engine, x, y, &label, color, if active { 0.4 } else { 0.1 }, RenderLayer::UI);
            x += label.len() as f32 * 0.4 + 0.5;
        }

        // Tool settings on second line
        let y2 = y - 0.6;
        let settings = self.tools.settings_text();
        self.ui.draw_text(engine, cx - 17.0, y2, &settings,
            Vec4::new(0.5, 0.6, 0.7, 0.7), 0.1, RenderLayer::UI);
    }

    fn render_status(&self, engine: &mut ProofEngine, cx: f32, cy: f32) {
        if !self.show_stats { return; }
        let y = cy - 11.5;
        let x = cx - 17.0;

        let status = format!(
            "FPS:{:.0}  Glyphs:{}  Fields:{}  Nodes:{}  Selected:{}  Undo:{}",
            self.fps,
            self.document.glyph_count(),
            self.document.field_count(),
            self.document.node_count(),
            self.document.selection.len(),
            self.commands.undo_count(),
        );
        self.ui.draw_text(engine, x, y, &status,
            Vec4::new(0.4, 0.5, 0.6, 0.6), 0.1, RenderLayer::UI);
    }

    fn render_notifications(&self, engine: &mut ProofEngine, cx: f32, cy: f32, _dt: f32) {
        let mut y = cy + 8.0;
        for notif in self.notifications.iter().rev().take(5) {
            let alpha = (notif.remaining / 2.0).min(1.0);
            let mut c = notif.color;
            c.w *= alpha;
            self.ui.draw_text(engine, cx - 5.0, y, &notif.text, c, 0.3 * alpha, RenderLayer::UI);
            y -= 0.6;
        }
    }

    fn render_help(&self, engine: &mut ProofEngine, cx: f32, cy: f32) {
        let x = cx - 12.0;
        let mut y = cy + 7.0;
        let dim = Vec4::new(0.6, 0.6, 0.7, 0.8);
        let bright = Vec4::new(1.0, 0.9, 0.5, 0.9);

        let help = [
            ("PROOF EDITOR CONTROLS", true),
            ("", false),
            ("Mouse click      Place / Select", false),
            ("WASD / Arrows    Pan camera", false),
            ("Scroll           Zoom", false),
            ("", false),
            ("V  Select tool      G  Move tool", false),
            ("R  Rotate tool      T  Scale tool", false),
            ("P  Place glyph      F  Place field", false),
            ("E  Place entity     X  Particle burst", false),
            ("", false),
            ("Q/W  Cycle chars    1/2  Cycle colors", false),
            ("3/4  Cycle fields   5/6  Emission +/-", false),
            ("7/8  Glow +/-       9/0  Bloom +/-", false),
            ("", false),
            ("Ctrl+S  Save        Ctrl+O  Load", false),
            ("Ctrl+N  New         Ctrl+Z  Undo", false),
            ("Ctrl+Y  Redo        Ctrl+D  Duplicate", false),
            ("Ctrl+A  Select all  Delete  Remove", false),
            ("", false),
            ("F1  Help     F2  Stats     F3  Grid", false),
            ("F4  Gizmos   F5  Play/Edit F6  Step", false),
            ("Space  Screen shake    Esc  Deselect", false),
        ];

        // Background panel
        for row in 0..help.len() {
            for col in 0..42 {
                engine.spawn_glyph(Glyph {
                    character: ' ',
                    position: Vec3::new(x + col as f32 * 0.42, y - row as f32 * 0.55, 0.8),
                    color: Vec4::new(0.05, 0.05, 0.08, 0.85),
                    layer: RenderLayer::Overlay,
                    lifetime: 0.02,
                    ..Default::default()
                });
            }
        }

        for (text, is_heading) in &help {
            let color = if *is_heading { bright } else { dim };
            let em = if *is_heading { 0.4 } else { 0.1 };
            self.ui.draw_text(engine, x, y, text, color, em, RenderLayer::Overlay);
            y -= 0.55;
        }
    }

    // ════════════════════════════════════════════════════════════════════════
    // Utilities
    // ════════════════════════════════════════════════════════════════════════

    fn update_fps(&mut self, dt: f32) {
        self.fps_frames += 1;
        self.fps_timer += dt;
        if self.fps_timer >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_timer;
            self.fps_frames = 0;
            self.fps_timer = 0.0;
        }
    }

    pub fn notify(&mut self, text: &str, color: Vec4) {
        self.notifications.push(Notification {
            text: text.to_string(),
            color,
            remaining: 3.0,
        });
    }
}
