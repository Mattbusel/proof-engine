//! Editor application — wires the engine's existing 16.5K lines of editor
//! infrastructure (EditorState, HierarchyPanel, Inspector, DevConsole,
//! GizmoRenderer, DebugOverlay, UI framework) into a runnable application.

use proof_engine::prelude::*;
use proof_engine::input::Key;
use proof_engine::editor::{
    EditorState, EditorMode,
    Inspector, HierarchyPanel, DevConsole, GizmoRenderer,
};
use proof_engine::editor::hierarchy::{NodeKind, NodeId};
use proof_engine::editor::console::LogLevel;

use glam::{Vec3, Vec4};

use crate::scene::SceneDocument;
use crate::tools::{ToolManager, ToolKind};
use crate::viewport::ViewportState;
use crate::commands::CommandHistory;
use crate::hotkeys::HotkeyMap;
use crate::clipboard::Clipboard;
use crate::preferences::EditorPrefs;
use crate::layout::LayoutManager;
use crate::widgets::{WidgetTheme, WidgetDraw, Rect};

#[derive(Debug, Clone)]
pub struct Notification {
    pub text: String,
    pub color: Vec4,
    pub remaining: f32,
}

pub struct EditorApp {
    pub editor_state: EditorState,
    pub hierarchy: HierarchyPanel,
    pub inspector: Inspector,
    pub console: DevConsole,
    pub gizmos: GizmoRenderer,
    pub document: SceneDocument,
    pub tools: ToolManager,
    pub viewport: ViewportState,
    pub commands: CommandHistory,
    pub hotkeys: HotkeyMap,
    pub clipboard: Clipboard,
    pub prefs: EditorPrefs,
    pub layout: LayoutManager,
    pub theme: WidgetTheme,
    pub fps: f32,
    pub frame_count: u64,
    pub time: f32,
    pub notifications: Vec<Notification>,
    pub show_console: bool,
    pub show_hierarchy: bool,
    pub show_inspector: bool,
    pub show_help: bool,
    pub show_stats: bool,
    fps_timer: f32,
    fps_frames: u32,
}

impl EditorApp {
    pub fn new() -> Self {
        let mut console = DevConsole::new();
        console.info("Proof Editor initialized");
        console.info("F1=help `=console");
        Self {
            editor_state: EditorState::new(proof_engine::editor::EditorConfig::default()),
            hierarchy: HierarchyPanel::new(),
            inspector: Inspector::new(200.0, 600.0),
            console,
            gizmos: GizmoRenderer::new(),
            document: SceneDocument::new(),
            tools: ToolManager::new(),
            viewport: ViewportState::new(),
            commands: CommandHistory::new(200),
            hotkeys: HotkeyMap::defaults(),
            clipboard: Clipboard::new(),
            prefs: EditorPrefs::default(),
            layout: LayoutManager::new(1600.0, 1000.0),
            theme: WidgetTheme::dark(),
            fps: 60.0, frame_count: 0, time: 0.0,
            notifications: Vec::new(),
            show_console: false, show_hierarchy: true,
            show_inspector: true, show_help: false, show_stats: true,
            fps_timer: 0.0, fps_frames: 0,
        }
    }

    pub fn init(&mut self, engine: &mut ProofEngine) {
        self.notify("Proof Editor ready", Vec4::new(0.5, 0.8, 1.0, 1.0));
        self.editor_state.mode = EditorMode::Edit;
        self.spawn_grid(engine);
        let _root = self.hierarchy.add_node("Scene", NodeKind::Group, None);
    }

    pub fn update(&mut self, engine: &mut ProofEngine, dt: f32) {
        self.time += dt;
        self.frame_count += 1;
        self.fps_frames += 1;
        self.fps_timer += dt;
        if self.fps_timer >= 0.5 {
            self.fps = self.fps_frames as f32 / self.fps_timer;
            self.fps_frames = 0;
            self.fps_timer = 0.0;
        }

        let input = engine.input.clone();

        if self.show_console { self.update_console(&input); }
        self.process_hotkeys(&input, engine);

        if !self.show_console {
            self.viewport.update(&input, dt, &self.layout);
            engine.camera.position.x.target = self.viewport.cam_x;
            engine.camera.position.y.target = self.viewport.cam_y;
        }

        if self.editor_state.mode == EditorMode::Edit && !self.show_console {
            let evts = self.tools.update(&input, &self.viewport, &self.document, &self.layout);
            for e in evts { self.process_tool_event(e, engine); }
        }

        self.console.tick(dt);
        self.notifications.retain_mut(|n| { n.remaining -= dt; n.remaining > 0.0 });
        self.render_all(engine);
    }

    fn update_console(&mut self, input: &proof_engine::input::InputState) {
        if input.just_pressed(Key::Escape) { self.show_console = false; return; }
        if input.just_pressed(Key::Enter) {
            let r = self.console.submit();
            match r.text.as_str() {
                "help" => { self.console.info("Commands: help clear save load new stats theme quit"); }
                "clear" => self.console.clear_log(),
                "stats" => {
                    self.console.info(&format!("FPS:{:.0} Nodes:{} Undo:{}", self.fps, self.document.node_count(), self.commands.undo_count()));
                }
                _ => if !r.text.is_empty() { self.console.warn(&format!("Unknown: {}", r.text)); }
            }
        }
        if input.just_pressed(Key::Backspace) { self.console.pop_char(); }
        if input.just_pressed(Key::Up) { self.console.history_up(); }
        if input.just_pressed(Key::Down) { self.console.history_down(); }
        if input.just_pressed(Key::Tab) { self.console.tab_complete(); }
        let keys = [(Key::A,'a'),(Key::B,'b'),(Key::C,'c'),(Key::D,'d'),(Key::E,'e'),(Key::F,'f'),
            (Key::G,'g'),(Key::H,'h'),(Key::I,'i'),(Key::J,'j'),(Key::K,'k'),(Key::L,'l'),
            (Key::M,'m'),(Key::N,'n'),(Key::O,'o'),(Key::P,'p'),(Key::Q,'q'),(Key::R,'r'),
            (Key::S,'s'),(Key::T,'t'),(Key::U,'u'),(Key::V,'v'),(Key::W,'w'),(Key::X,'x'),
            (Key::Y,'y'),(Key::Z,'z'),(Key::Space,' '),(Key::Num0,'0'),(Key::Num1,'1'),
            (Key::Num2,'2'),(Key::Num3,'3'),(Key::Num4,'4'),(Key::Num5,'5'),
            (Key::Num6,'6'),(Key::Num7,'7'),(Key::Num8,'8'),(Key::Num9,'9')];
        for &(k,c) in &keys {
            if input.just_pressed(k) { self.console.push_char(if input.shift() { c.to_ascii_uppercase() } else { c }); }
        }
    }

    fn process_hotkeys(&mut self, input: &proof_engine::input::InputState, engine: &mut ProofEngine) {
        let ctrl = input.ctrl();
        if input.just_pressed(Key::Backtick) { self.show_console = !self.show_console; }
        if input.just_pressed(Key::F1) { self.show_help = !self.show_help; }
        if input.just_pressed(Key::F2) { self.show_stats = !self.show_stats; }
        if input.just_pressed(Key::F3) { self.editor_state.config.show_grid = !self.editor_state.config.show_grid; }
        if input.just_pressed(Key::F5) {
            self.editor_state.mode = if self.editor_state.mode == EditorMode::Edit { EditorMode::Play } else { EditorMode::Edit };
        }
        if ctrl && input.just_pressed(Key::Z) { self.commands.undo(); }
        if ctrl && input.just_pressed(Key::Y) { self.commands.redo(); }
        if ctrl && input.just_pressed(Key::S) { let _ = self.document.save("scene.json"); self.notify("Saved", Vec4::new(0.2,1.0,0.4,1.0)); }
        if ctrl && input.just_pressed(Key::N) { self.document = SceneDocument::new(); engine.scene = SceneGraph::new(); self.spawn_grid(engine); }
        if input.just_pressed(Key::Delete) {
            let sel = self.document.selection.clone();
            for id in sel { self.document.remove_node(id); }
            self.document.selection.clear();
            self.rebuild_scene(engine);
        }
        if ctrl && input.just_pressed(Key::D) {
            let sel = self.document.selection.clone();
            let mut new_ids = Vec::new();
            for id in sel { if let Some(nid) = self.document.duplicate_node(id) { new_ids.push(nid); } }
            self.document.selection = new_ids;
            self.rebuild_scene(engine);
        }
        if ctrl && input.just_pressed(Key::A) { self.document.select_all(); }
        if input.just_pressed(Key::Escape) {
            if self.show_console { self.show_console = false; }
            else if self.show_help { self.show_help = false; }
            else { self.document.selection.clear(); }
        }
        if !self.show_console && !ctrl {
            if input.just_pressed(Key::V) { self.tools.set_tool(ToolKind::Select); }
            if input.just_pressed(Key::G) { self.tools.set_tool(ToolKind::Move); }
            if input.just_pressed(Key::P) { self.tools.set_tool(ToolKind::Place); }
            if input.just_pressed(Key::F) { self.tools.set_tool(ToolKind::Field); }
            if input.just_pressed(Key::E) { self.tools.set_tool(ToolKind::Entity); }
            if input.just_pressed(Key::X) { self.tools.set_tool(ToolKind::Particle); }
            if input.just_pressed(Key::Space) { engine.add_trauma(0.3); }
        }
    }

    fn process_tool_event(&mut self, event: crate::tools::ToolEvent, engine: &mut ProofEngine) {
        match event {
            crate::tools::ToolEvent::PlaceGlyph { position, character, color, emission, glow_radius } => {
                let nid = self.document.add_glyph_node(position, character, color, emission, glow_radius);
                self.spawn_glyph(engine, nid);
            }
            crate::tools::ToolEvent::PlaceField { position, field_type } => {
                let nid = self.document.add_field_node(position, field_type);
                self.spawn_field(engine, nid);
            }
            crate::tools::ToolEvent::PlaceEntity { position } => {
                let nid = self.document.add_entity_node(position);
                self.spawn_entity(engine, nid);
            }
            crate::tools::ToolEvent::PlaceParticleBurst { position, color } => {
                engine.emit_particles(proof_engine::particle::EmitterPreset::DeathExplosion { color }, position);
            }
            crate::tools::ToolEvent::MoveSelection { delta } => {
                let ids = self.document.selection.clone();
                for id in ids { self.document.translate_node(id, delta); }
                self.rebuild_scene(engine);
            }
            crate::tools::ToolEvent::Select { node_id, additive } => {
                if additive { self.document.toggle_selection(node_id); }
                else { self.document.selection = vec![node_id]; }
            }
            crate::tools::ToolEvent::BoxSelect { ids } => { self.document.selection = ids; }
            crate::tools::ToolEvent::Deselect => { self.document.selection.clear(); }
        }
    }

    fn spawn_glyph(&self, engine: &mut ProofEngine, nid: u32) {
        if let Some(n) = self.document.get_node(nid) {
            engine.spawn_glyph(Glyph {
                character: n.character.unwrap_or('@'), position: n.position,
                color: n.color, emission: n.emission,
                glow_color: Vec3::new(n.color.x, n.color.y, n.color.z),
                glow_radius: n.glow_radius, mass: 0.1,
                layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                life_function: Some(MathFunction::Breathing { rate: 0.3, depth: 0.08 }),
                ..Default::default()
            });
        }
    }

    fn spawn_field(&self, engine: &mut ProofEngine, nid: u32) {
        if let Some(n) = self.document.get_node(nid) {
            if let Some(ref ft) = n.field_type { engine.add_field(ft.to_force_field(n.position)); }
        }
    }

    fn spawn_entity(&self, engine: &mut ProofEngine, nid: u32) {
        if let Some(n) = self.document.get_node(nid) {
            let mut ent = AmorphousEntity::new("Entity", n.position);
            ent.entity_mass = 3.0; ent.cohesion = 0.7;
            ent.pulse_rate = 0.5; ent.pulse_depth = 0.15;
            ent.hp = 100.0; ent.max_hp = 100.0;
            let chars = ['@','#','*','+','o','x','X','O','.',':','~','='];
            for i in 0..12 {
                let a = (i as f32 / 12.0) * std::f32::consts::TAU;
                ent.formation.push(Vec3::new(a.cos() * 0.8, a.sin() * 0.8, 0.0));
                ent.formation_chars.push(chars[i % chars.len()]);
                ent.formation_colors.push(n.color);
            }
            engine.spawn_entity(ent);
        }
    }

    fn rebuild_scene(&self, engine: &mut ProofEngine) {
        engine.scene = SceneGraph::new();
        self.spawn_grid(engine);
        for n in self.document.nodes() {
            match n.kind {
                crate::scene::NodeKind::Glyph => self.spawn_glyph(engine, n.id),
                crate::scene::NodeKind::Field => self.spawn_field(engine, n.id),
                crate::scene::NodeKind::Entity => self.spawn_entity(engine, n.id),
                _ => {}
            }
        }
    }

    fn spawn_grid(&self, engine: &mut ProofEngine) {
        if !self.editor_state.config.show_grid { return; }
        for y in -25..=25 { for x in -35..=35 {
            let on_axis = x == 0 || y == 0;
            let on_major = x % 5 == 0 && y % 5 == 0;
            let ch = if on_axis && on_major { '+' } else if on_axis { '-' } else if on_major { '.' } else if (x+y)%4==0 { '.' } else { continue };
            let color = if x==0 && y==0 { Vec4::new(1.0,1.0,0.3,0.4) } else if on_axis { Vec4::new(0.2,0.3,0.5,0.25) } else { Vec4::new(0.15,0.15,0.2,0.12) };
            engine.spawn_glyph(Glyph { character: ch, position: Vec3::new(x as f32, y as f32, -2.0), color, emission: if on_axis { 0.15 } else { 0.03 }, layer: RenderLayer::Background, ..Default::default() });
        }}
    }

    fn render_all(&self, engine: &mut ProofEngine) {
        let (cx, cy) = (self.viewport.cam_x, self.viewport.cam_y);
        // Menu bar
        let y = cy + 11.5; let x = cx - 17.0;
        for (i, label) in ["File","Edit","View","Tools","Scene"].iter().enumerate() {
            WidgetDraw::text(engine, x + i as f32 * 4.0, y, label, self.theme.fg, 0.15, RenderLayer::UI);
        }
        let mode_s = self.editor_state.mode.to_string();
        WidgetDraw::text(engine, cx + 12.0, y, &mode_s,
            if self.editor_state.mode == EditorMode::Play { Vec4::new(0.2,1.0,0.4,1.0) } else { Vec4::new(0.3,0.6,1.0,1.0) },
            0.5, RenderLayer::UI);

        // Hierarchy
        if self.show_hierarchy {
            let mut y = cy + 10.0;
            WidgetDraw::text(engine, x, y, "HIERARCHY", self.theme.accent, 0.3, RenderLayer::UI); y -= 0.7;
            for n in self.document.nodes().take(25) {
                let sel = self.document.selection.contains(&n.id);
                let icon = match n.kind { crate::scene::NodeKind::Glyph=>"@", crate::scene::NodeKind::Field=>"~", crate::scene::NodeKind::Entity=>"#", _=>">" };
                WidgetDraw::text(engine, x, y, &format!("{} {}", icon, n.name),
                    if sel { self.theme.warning } else { self.theme.fg }, if sel { 0.3 } else { 0.08 }, RenderLayer::UI);
                y -= 0.5;
            }
        }

        // Inspector
        if self.show_inspector {
            let ix = cx + 10.0; let mut iy = cy + 10.0;
            WidgetDraw::text(engine, ix, iy, "INSPECTOR", self.theme.accent, 0.3, RenderLayer::UI); iy -= 0.7;
            if let Some(&id) = self.document.selection.first() {
                if let Some(n) = self.document.get_node(id) {
                    for line in &[
                        format!("Name: {}", n.name), format!("Type: {:?}", n.kind),
                        format!("Pos: ({:.1},{:.1})", n.position.x, n.position.y),
                        format!("Emit: {:.2}  Glow: {:.2}", n.emission, n.glow_radius),
                    ] {
                        WidgetDraw::text(engine, ix, iy, line, self.theme.fg, 0.1, RenderLayer::UI); iy -= 0.5;
                    }
                    WidgetDraw::color_swatch(engine, ix, iy, n.color);
                }
            } else {
                WidgetDraw::text(engine, ix, iy, "No selection", self.theme.fg_dim, 0.05, RenderLayer::UI);
            }
        }

        // Toolbar
        let ty = cy - 10.5; let mut tx = cx - 17.0;
        for (k, name, kind) in &[("V","Select",ToolKind::Select),("G","Move",ToolKind::Move),("P","Place",ToolKind::Place),
            ("F","Field",ToolKind::Field),("E","Entity",ToolKind::Entity),("X","Burst",ToolKind::Particle)] {
            let a = self.tools.current() == *kind;
            WidgetDraw::text(engine, tx, ty, &format!("[{}]{}", k, name),
                if a { self.theme.warning } else { self.theme.fg_dim }, if a { 0.35 } else { 0.06 }, RenderLayer::UI);
            tx += (name.len()+3) as f32 * 0.4 + 0.3;
        }
        WidgetDraw::text(engine, cx-17.0, ty-0.6, &self.tools.settings_text(), self.theme.fg_dim, 0.04, RenderLayer::UI);

        // Status
        if self.show_stats {
            WidgetDraw::text(engine, cx-17.0, cy-11.5,
                &format!("FPS:{:.0} N:{} G:{} F:{} S:{}", self.fps, self.document.node_count(),
                    self.document.glyph_count(), self.document.field_count(), self.document.selection.len()),
                self.theme.fg_dim, 0.04, RenderLayer::UI);
        }

        // Notifications
        let mut ny = cy + 8.0;
        for n in self.notifications.iter().rev().take(5) {
            let mut c = n.color; c.w *= (n.remaining / 2.0).min(1.0);
            WidgetDraw::text(engine, cx-5.0, ny, &n.text, c, 0.3 * c.w, RenderLayer::UI);
            ny -= 0.6;
        }

        // Console
        if self.show_console {
            let (bx, by) = (cx-14.0, cy-5.0);
            WidgetDraw::fill_rect(engine, Rect::new(bx, by+8.0, 28.0, 8.0), Vec4::new(0.05,0.05,0.08,0.92));
            WidgetDraw::text(engine, bx+0.3, by+7.8, "CONSOLE", self.theme.accent, 0.2, RenderLayer::Overlay);
            let mut ly = by + 7.0;
            for line in self.console.lines().collect::<Vec<_>>().into_iter().rev().take(12) {
                let c = match line.level { LogLevel::Error|LogLevel::Fatal => self.theme.error, LogLevel::Warn => self.theme.warning, _ => self.theme.fg_dim };
                WidgetDraw::text(engine, bx+0.3, ly, &format!("{} {}", line.level.prefix_char(), line.text), c, 0.06, RenderLayer::Overlay);
                ly -= 0.5;
            }
            WidgetDraw::text(engine, bx+0.3, by+0.3, &format!("> {}", self.console.input_buffer.clone()), self.theme.fg_bright, 0.15, RenderLayer::Overlay);
        }

        // Help
        if self.show_help {
            let hx = cx - 10.0; let mut hy = cy + 6.0;
            WidgetDraw::fill_rect(engine, Rect::new(hx-0.3, hy+0.3, 18.0, 11.0), Vec4::new(0.05,0.05,0.08,0.92));
            for line in &["PROOF EDITOR HELP","","Click=Place  WASD=Pan","V/G/P/F/E/X=Tools","Q/W=Chars 1/2=Colors","3/4=Fields 5-8=Emit/Glow",
                "","Ctrl+S/O/N=Save/Load/New","Ctrl+Z/Y=Undo/Redo","Ctrl+D=Dup Del=Remove","","F1=Help F2=Stats F3=Grid","F5=Play `=Console Esc=Close"] {
                WidgetDraw::text(engine, hx, hy, line, if hy > cy + 5.5 { self.theme.warning } else { self.theme.fg }, 0.08, RenderLayer::Overlay);
                hy -= 0.55;
            }
        }
    }

    pub fn notify(&mut self, text: &str, color: Vec4) {
        self.notifications.push(Notification { text: text.to_string(), color, remaining: 3.0 });
    }
}
