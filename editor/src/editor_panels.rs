//! All editor panels as separate functions for clean organization.
//!
//! Each panel takes egui Ui + shared editor state and renders itself.

use proof_engine::prelude::*;
use crate::scene::{SceneDocument, SceneNode, NodeKind, FieldType};
use crate::tools::{ToolKind, CHAR_PALETTES, COLOR_PALETTES};

/// Shared mutable state that panels read/write.
pub struct EditorState {
    pub document: SceneDocument,
    pub tool: ToolKind,
    pub char_palette_idx: usize,
    pub color_palette_idx: usize,
    pub field_type_idx: usize,
    pub emission: f32,
    pub glow_radius: f32,
    pub show_help: bool,
    pub show_console: bool,
    pub show_fields_panel: bool,
    pub show_asset_browser: bool,
    pub show_postfx_panel: bool,
    pub cam_x: f32,
    pub cam_y: f32,
    pub fps: f32,
    pub status_msg: String,
    pub status_timer: f32,
    pub needs_rebuild: bool,
    pub console_log: Vec<(String, egui::Color32)>,
    pub console_input: String,
    // Undo
    pub undo_stack: Vec<UndoEntry>,
    pub redo_stack: Vec<UndoEntry>,
    // New panel toggles
    pub show_world_editor: bool,
    pub show_ai_behavior: bool,
    pub show_physics: bool,
    pub show_render_graph: bool,
    pub show_dialogue: bool,
    pub show_quest: bool,
    pub show_spline: bool,
    pub show_cinematic: bool,
    pub show_inventory: bool,
    pub show_ability: bool,
    pub show_level_streaming: bool,
    pub show_audio_mixer: bool,
    // Per-panel state
    pub world_seed: u64,
    pub world_biome_filter: String,
    pub ai_selected_tree: String,
    pub physics_selected_body: String,
    pub dialogue_search: String,
    pub quest_search: String,
    pub inventory_search: String,
    pub ability_search: String,
    pub audio_master_volume: f32,
    pub audio_music_volume: f32,
    pub audio_sfx_volume: f32,
}

#[derive(Clone)]
pub struct UndoEntry {
    pub label: String,
    pub snapshot: Vec<u8>, // serialized document
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            document: SceneDocument::new(),
            tool: ToolKind::Place,
            char_palette_idx: 0, color_palette_idx: 0, field_type_idx: 0,
            emission: 1.5, glow_radius: 1.0,
            show_help: false, show_console: false,
            show_fields_panel: false, show_asset_browser: false,
            show_postfx_panel: false,
            cam_x: 0.0, cam_y: 0.0, fps: 60.0,
            status_msg: String::new(), status_timer: 0.0,
            needs_rebuild: false,
            console_log: vec![("Proof Editor ready. F1=help".into(), egui::Color32::from_rgb(100, 180, 255))],
            console_input: String::new(),
            undo_stack: Vec::new(), redo_stack: Vec::new(),
            show_world_editor: false, show_ai_behavior: false, show_physics: false,
            show_render_graph: false, show_dialogue: false, show_quest: false,
            show_spline: false, show_cinematic: false, show_inventory: false,
            show_ability: false, show_level_streaming: false, show_audio_mixer: false,
            world_seed: 42, world_biome_filter: String::new(),
            ai_selected_tree: "BehaviorTree_Enemy".to_string(),
            physics_selected_body: "RigidBody_0".to_string(),
            dialogue_search: String::new(), quest_search: String::new(),
            inventory_search: String::new(), ability_search: String::new(),
            audio_master_volume: 80.0, audio_music_volume: 60.0, audio_sfx_volume: 75.0,
        }
    }

    pub fn set_status(&mut self, text: &str) {
        self.status_msg = text.to_string();
        self.status_timer = 3.0;
    }

    pub fn log(&mut self, text: &str, color: egui::Color32) {
        self.console_log.push((text.to_string(), color));
        if self.console_log.len() > 200 { self.console_log.remove(0); }
    }

    pub fn push_undo(&mut self, label: &str) {
        if let Ok(json) = serde_json::to_vec(&self.document) {
            self.undo_stack.push(UndoEntry { label: label.to_string(), snapshot: json });
            self.redo_stack.clear();
            if self.undo_stack.len() > 100 { self.undo_stack.remove(0); }
        }
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            // Save current state to redo
            if let Ok(json) = serde_json::to_vec(&self.document) {
                self.redo_stack.push(UndoEntry { label: entry.label.clone(), snapshot: json });
            }
            if let Ok(doc) = serde_json::from_slice(&entry.snapshot) {
                self.document = doc;
                self.needs_rebuild = true;
                self.set_status(&format!("Undo: {}", entry.label));
            }
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            if let Ok(json) = serde_json::to_vec(&self.document) {
                self.undo_stack.push(UndoEntry { label: entry.label.clone(), snapshot: json });
            }
            if let Ok(doc) = serde_json::from_slice(&entry.snapshot) {
                self.document = doc;
                self.needs_rebuild = true;
                self.set_status(&format!("Redo: {}", entry.label));
            }
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Menu bar
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn menu_bar(ctx: &egui::Context, state: &mut EditorState, engine: &mut ProofEngine) {
    egui::TopBottomPanel::top("menu").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New (Ctrl+N)").clicked() {
                    state.push_undo("New Scene");
                    state.document = SceneDocument::new();
                    state.needs_rebuild = true;
                    state.set_status("New scene");
                    ui.close_menu();
                }
                if ui.button("Save (Ctrl+S)").clicked() {
                    match state.document.save("scene.json") {
                        Ok(_) => { state.set_status("Saved scene.json"); state.log("Saved scene.json", egui::Color32::from_rgb(100, 200, 100)); }
                        Err(e) => { state.set_status(&format!("Save failed: {}", e)); state.log(&format!("Save error: {}", e), egui::Color32::from_rgb(255, 100, 100)); }
                    }
                    ui.close_menu();
                }
                if ui.button("Load (Ctrl+O)").clicked() {
                    state.push_undo("Before Load");
                    match SceneDocument::load("scene.json") {
                        Ok(doc) => { state.document = doc; state.needs_rebuild = true; state.set_status("Loaded"); state.log("Loaded scene.json", egui::Color32::from_rgb(100, 200, 100)); }
                        Err(e) => { state.set_status(&format!("Load failed: {}", e)); state.log(&format!("Load error: {}", e), egui::Color32::from_rgb(255, 100, 100)); }
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Quit").clicked() { engine.request_quit(); }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo (Ctrl+Z)").clicked() { state.undo(); ui.close_menu(); }
                if ui.button("Redo (Ctrl+Y)").clicked() { state.redo(); ui.close_menu(); }
                ui.separator();
                if ui.button("Select All (Ctrl+A)").clicked() { state.document.select_all(); ui.close_menu(); }
                if ui.button("Delete (Del)").clicked() {
                    state.push_undo("Delete");
                    let sel = state.document.selection.clone();
                    for id in sel { state.document.remove_node(id); }
                    state.document.selection.clear();
                    state.needs_rebuild = true;
                    ui.close_menu();
                }
                if ui.button("Duplicate (Ctrl+D)").clicked() {
                    state.push_undo("Duplicate");
                    let sel = state.document.selection.clone();
                    let mut new_ids = Vec::new();
                    for id in sel { if let Some(nid) = state.document.duplicate_node(id) { new_ids.push(nid); } }
                    state.document.selection = new_ids;
                    state.needs_rebuild = true;
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Help (F1)").clicked() { state.show_help = !state.show_help; ui.close_menu(); }
                if ui.button("Console").clicked() { state.show_console = !state.show_console; ui.close_menu(); }
                if ui.button("Force Fields").clicked() { state.show_fields_panel = !state.show_fields_panel; ui.close_menu(); }
                if ui.button("Asset Browser").clicked() { state.show_asset_browser = !state.show_asset_browser; ui.close_menu(); }
                if ui.button("Post-Processing").clicked() { state.show_postfx_panel = !state.show_postfx_panel; ui.close_menu(); }
                ui.separator();
                if ui.button("Toggle Bloom").clicked() { engine.config.render.bloom_enabled = !engine.config.render.bloom_enabled; ui.close_menu(); }
                if ui.button("Reset Camera").clicked() { state.cam_x = 0.0; state.cam_y = 0.0; ui.close_menu(); }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("World Editor").clicked() { state.show_world_editor = !state.show_world_editor; ui.close_menu(); }
                if ui.button("AI Behavior").clicked() { state.show_ai_behavior = !state.show_ai_behavior; ui.close_menu(); }
                if ui.button("Physics").clicked() { state.show_physics = !state.show_physics; ui.close_menu(); }
                if ui.button("Render Graph").clicked() { state.show_render_graph = !state.show_render_graph; ui.close_menu(); }
                ui.separator();
                if ui.button("Dialogue Editor").clicked() { state.show_dialogue = !state.show_dialogue; ui.close_menu(); }
                if ui.button("Quest Editor").clicked() { state.show_quest = !state.show_quest; ui.close_menu(); }
                if ui.button("Spline Editor").clicked() { state.show_spline = !state.show_spline; ui.close_menu(); }
                if ui.button("Cinematic Editor").clicked() { state.show_cinematic = !state.show_cinematic; ui.close_menu(); }
                ui.separator();
                if ui.button("Inventory").clicked() { state.show_inventory = !state.show_inventory; ui.close_menu(); }
                if ui.button("Ability Editor").clicked() { state.show_ability = !state.show_ability; ui.close_menu(); }
                if ui.button("Level Streaming").clicked() { state.show_level_streaming = !state.show_level_streaming; ui.close_menu(); }
                if ui.button("Audio Mixer").clicked() { state.show_audio_mixer = !state.show_audio_mixer; ui.close_menu(); }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("FPS: {:.0}", state.fps));
                ui.separator();
                ui.label(format!("Nodes: {} | Undo: {}", state.document.node_count(), state.undo_stack.len()));
                if state.status_timer > 0.0 {
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(100, 220, 100), &state.status_msg);
                }
            });
        });
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Hierarchy panel — tree structure with search, icons, collapse
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn hierarchy_panel(ctx: &egui::Context, state: &mut EditorState) {
    static mut SEARCH: String = String::new();
    static mut FILTER: Option<NodeKind> = None;

    egui::SidePanel::left("hierarchy").default_width(200.0).show(ctx, |ui| {
        ui.heading("Hierarchy");

        // Search bar
        ui.horizontal(|ui| {
            ui.label("Search:");
            // SAFETY: single-threaded editor, these statics are fine
            unsafe { ui.text_edit_singleline(&mut SEARCH); }
        });

        // Filter buttons
        ui.horizontal(|ui| {
            let filter = unsafe { &mut FILTER };
            if ui.selectable_label(filter.is_none(), "All").clicked() { *filter = None; }
            if ui.selectable_label(*filter == Some(NodeKind::Glyph), "@Glyph").clicked() { *filter = Some(NodeKind::Glyph); }
            if ui.selectable_label(*filter == Some(NodeKind::Field), "~Field").clicked() { *filter = Some(NodeKind::Field); }
            if ui.selectable_label(*filter == Some(NodeKind::Entity), "#Entity").clicked() { *filter = Some(NodeKind::Entity); }
        });

        ui.separator();

        if state.document.node_count() == 0 {
            ui.label("Empty scene. Click viewport to place.");
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            let search = unsafe { SEARCH.to_lowercase() };
            let filter = unsafe { FILTER };
            let mut clicked_id = None;

            // Group by kind for tree structure
            let entities: Vec<&SceneNode> = state.document.nodes()
                .filter(|n| n.kind == NodeKind::Entity)
                .filter(|n| filter.map_or(true, |f| f == n.kind))
                .filter(|n| search.is_empty() || n.name.to_lowercase().contains(&search))
                .collect();
            let fields: Vec<&SceneNode> = state.document.nodes()
                .filter(|n| n.kind == NodeKind::Field)
                .filter(|n| filter.map_or(true, |f| f == n.kind))
                .filter(|n| search.is_empty() || n.name.to_lowercase().contains(&search))
                .collect();
            let glyphs: Vec<&SceneNode> = state.document.nodes()
                .filter(|n| n.kind == NodeKind::Glyph)
                .filter(|n| filter.map_or(true, |f| f == n.kind))
                .filter(|n| search.is_empty() || n.name.to_lowercase().contains(&search))
                .collect();

            // Entities section
            if !entities.is_empty() && filter != Some(NodeKind::Glyph) && filter != Some(NodeKind::Field) {
                let header = egui::CollapsingHeader::new(format!("Entities ({})", entities.len()))
                    .default_open(true);
                header.show(ui, |ui| {
                    for node in &entities {
                        let sel = state.document.selection.contains(&node.id);
                        let label = egui::RichText::new(format!("# {}", node.name))
                            .color(if sel { egui::Color32::YELLOW } else { egui::Color32::from_rgb(180, 120, 255) });
                        if ui.selectable_label(sel, label).clicked() { clicked_id = Some(node.id); }
                    }
                });
            }

            // Fields section
            if !fields.is_empty() && filter != Some(NodeKind::Glyph) && filter != Some(NodeKind::Entity) {
                let header = egui::CollapsingHeader::new(format!("Force Fields ({})", fields.len()))
                    .default_open(true);
                header.show(ui, |ui| {
                    for node in &fields {
                        let sel = state.document.selection.contains(&node.id);
                        let ft_name = node.field_type.as_ref().map(|f| f.label()).unwrap_or("Unknown");
                        let label = egui::RichText::new(format!("~ {} [{}]", node.name, ft_name))
                            .color(if sel { egui::Color32::YELLOW } else { egui::Color32::from_rgb(255, 180, 80) });
                        if ui.selectable_label(sel, label).clicked() { clicked_id = Some(node.id); }
                    }
                });
            }

            // Glyphs section (collapsible since there are many)
            if !glyphs.is_empty() && filter != Some(NodeKind::Field) && filter != Some(NodeKind::Entity) {
                let header = egui::CollapsingHeader::new(format!("Glyphs ({})", glyphs.len()))
                    .default_open(glyphs.len() < 30);
                header.show(ui, |ui| {
                    for node in &glyphs {
                        let sel = state.document.selection.contains(&node.id);
                        let ch = node.character.unwrap_or('?');
                        let label = egui::RichText::new(format!("@ {} '{}'", node.name, ch))
                            .color(if sel { egui::Color32::YELLOW } else { egui::Color32::from_rgb(150, 200, 150) });
                        if ui.selectable_label(sel, label).clicked() { clicked_id = Some(node.id); }
                    }
                });
            }

            if let Some(id) = clicked_id {
                state.document.selection = vec![id];
            }
        });
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Inspector — context-sensitive per node type
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn inspector_panel(ctx: &egui::Context, state: &mut EditorState) {
    egui::SidePanel::right("inspector").default_width(250.0).show(ctx, |ui| {
        ui.heading("Inspector");
        ui.separator();

        if let Some(&id) = state.document.selection.first() {
            if let Some(node) = state.document.get_node_mut(id) {
                let mut changed = false;

                // Name
                ui.horizontal(|ui| { ui.label("Name:"); ui.label(&node.name); });
                ui.label(format!("Type: {:?}  ID: {}", node.kind, node.id));
                ui.separator();

                // ── Transform section ──
                egui::CollapsingHeader::new("Transform").default_open(true).show(ui, |ui| {
                    changed |= ui.add(egui::Slider::new(&mut node.position.x, -30.0..=30.0).text("X")).changed();
                    changed |= ui.add(egui::Slider::new(&mut node.position.y, -30.0..=30.0).text("Y")).changed();
                    changed |= ui.add(egui::Slider::new(&mut node.rotation, 0.0..=360.0).text("Rotation")).changed();
                    changed |= ui.add(egui::Slider::new(&mut node.scale, 0.1..=5.0).text("Scale")).changed();
                });

                // ── Visual section ──
                egui::CollapsingHeader::new("Visual").default_open(true).show(ui, |ui| {
                    changed |= ui.add(egui::Slider::new(&mut node.emission, 0.0..=5.0).text("Emission")).changed();
                    changed |= ui.add(egui::Slider::new(&mut node.glow_radius, 0.0..=5.0).text("Glow Radius")).changed();

                    if let Some(ref mut ch) = node.character {
                        ui.horizontal(|ui| {
                            ui.label(format!("Char: '{}'", ch));
                            let chars = ['@', '#', '*', '+', 'o', 'x', 'X', 'O', '.', ':', '~', '=', '>', '<', '^', 'v'];
                            for &c in &chars[..8] {
                                if ui.small_button(&c.to_string()).clicked() { *ch = c; changed = true; }
                            }
                        });
                    }

                    // Color
                    let mut rgb = [node.color.x, node.color.y, node.color.z];
                    if ui.color_edit_button_rgb(&mut rgb).changed() {
                        node.color.x = rgb[0]; node.color.y = rgb[1]; node.color.z = rgb[2];
                        changed = true;
                    }
                    changed |= ui.add(egui::Slider::new(&mut node.color.w, 0.0..=1.0).text("Alpha")).changed();
                });

                // ── Force Field section (only for Field nodes) ──
                if node.kind == NodeKind::Field {
                    egui::CollapsingHeader::new("Force Field").default_open(true).show(ui, |ui| {
                        if let Some(ref ft) = node.field_type {
                            ui.label(format!("Type: {}", ft.label()));
                        }
                        // Field type selector
                        let mut ft_idx = node.field_type.as_ref()
                            .and_then(|ft| FieldType::all().iter().position(|f| std::mem::discriminant(f) == std::mem::discriminant(ft)))
                            .unwrap_or(0);
                        let ft_names: Vec<&str> = FieldType::all().iter().map(|f| f.label()).collect();
                        if egui::ComboBox::from_id_salt("ft_sel")
                            .selected_text(ft_names[ft_idx])
                            .show_ui(ui, |ui| {
                                for (i, name) in ft_names.iter().enumerate() {
                                    ui.selectable_value(&mut ft_idx, i, *name);
                                }
                            }).inner.is_some()
                        {
                            node.field_type = Some(FieldType::all()[ft_idx]);
                            changed = true;
                        }

                        ui.separator();
                        ui.label("Parameters (adjust via field presets):");
                        // Per-field-type sliders would go here
                        // For now, show the field properties as labels
                        for (k, v) in &node.properties {
                            ui.horizontal(|ui| { ui.label(format!("{}: {}", k, v)); });
                        }
                    });
                }

                // ── Entity section (only for Entity nodes) ──
                if node.kind == NodeKind::Entity {
                    egui::CollapsingHeader::new("Entity").default_open(true).show(ui, |ui| {
                        ui.label("Formation: Ring (12 glyphs)");
                        ui.label("Binding: Force cohesion");
                        // These would be editable in a full implementation:
                        ui.label("HP: 100 / 100");
                        ui.label("Cohesion: 0.7");
                        ui.label("Pulse Rate: 0.5 Hz");
                    });
                }

                // ── Tags ──
                egui::CollapsingHeader::new("Tags").default_open(false).show(ui, |ui| {
                    for tag in &node.tags {
                        ui.label(format!("  {}", tag));
                    }
                    if node.tags.is_empty() { ui.label("  (none)"); }
                });

                if changed { state.needs_rebuild = true; }
            }
        } else {
            ui.label("No selection");
            ui.label("Click in viewport to place objects.");
            ui.separator();
            ui.label(format!("Tool: {:?}", state.tool));
            ui.label("Shift+click to multi-select");
            ui.label("Ctrl+C/V to copy/paste");
        }
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Toolbar
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn toolbar_panel(ctx: &egui::Context, state: &mut EditorState, engine: &mut ProofEngine) {
    egui::TopBottomPanel::bottom("toolbar").min_height(60.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Tool:");
            for (kind, label) in &[(ToolKind::Select, "Select(V)"), (ToolKind::Move, "Move(G)"),
                (ToolKind::Place, "Place(P)"), (ToolKind::Field, "Field(F)"),
                (ToolKind::Entity, "Entity(E)"), (ToolKind::Particle, "Burst(X)")] {
                if ui.selectable_label(state.tool == *kind, *label).clicked() { state.tool = *kind; }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Chars:");
            let cn: Vec<&str> = CHAR_PALETTES.iter().map(|(n, _)| *n).collect();
            egui::ComboBox::from_id_salt("ch").selected_text(cn[state.char_palette_idx]).show_ui(ui, |ui| {
                for (i, n) in cn.iter().enumerate() { ui.selectable_value(&mut state.char_palette_idx, i, *n); }
            });
            ui.label("Colors:");
            let ccn: Vec<&str> = COLOR_PALETTES.iter().map(|(n, _)| *n).collect();
            egui::ComboBox::from_id_salt("co").selected_text(ccn[state.color_palette_idx]).show_ui(ui, |ui| {
                for (i, n) in ccn.iter().enumerate() { ui.selectable_value(&mut state.color_palette_idx, i, *n); }
            });
            ui.label("Field:");
            let fn_: Vec<&str> = FieldType::all().iter().map(|f| f.label()).collect();
            egui::ComboBox::from_id_salt("fl").selected_text(fn_[state.field_type_idx]).show_ui(ui, |ui| {
                for (i, n) in fn_.iter().enumerate() { ui.selectable_value(&mut state.field_type_idx, i, *n); }
            });
            ui.add(egui::Slider::new(&mut state.emission, 0.0..=5.0).text("Em"));
            ui.add(egui::Slider::new(&mut state.glow_radius, 0.0..=5.0).text("Glow"));
        });
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Force Field editing panel
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn force_field_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_fields_panel { return; }

    egui::Window::new("Force Fields")
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.heading("Active Force Fields");
            ui.separator();

            let field_nodes: Vec<(u32, String, Option<FieldType>)> = state.document.nodes()
                .filter(|n| n.kind == NodeKind::Field)
                .map(|n| (n.id, n.name.clone(), n.field_type))
                .collect();

            if field_nodes.is_empty() {
                ui.label("No force fields in scene.");
                ui.label("Use Field(F) tool to place one.");
                return;
            }

            for (id, name, ft) in &field_nodes {
                let sel = state.document.selection.contains(id);
                let ft_label = ft.as_ref().map(|f| f.label()).unwrap_or("Unknown");

                let header = egui::CollapsingHeader::new(
                    egui::RichText::new(format!("~ {} [{}]", name, ft_label))
                        .color(if sel { egui::Color32::YELLOW } else { egui::Color32::from_rgb(255, 180, 80) })
                ).default_open(sel);

                header.show(ui, |ui| {
                    if ui.button("Select").clicked() { state.document.selection = vec![*id]; }

                    if let Some(node) = state.document.get_node_mut(*id) {
                        let mut changed = false;
                        changed |= ui.add(egui::Slider::new(&mut node.position.x, -20.0..=20.0).text("X")).changed();
                        changed |= ui.add(egui::Slider::new(&mut node.position.y, -20.0..=20.0).text("Y")).changed();

                        // Per-type parameter sliders
                        match ft {
                            Some(FieldType::GravityWell) | Some(FieldType::Repulsor) => {
                                ui.label("Strength: 2.0 (default)");
                                ui.label("Falloff: InverseSquare");
                            }
                            Some(FieldType::Vortex) => {
                                ui.label("Strength: 0.5");
                                ui.label("Radius: 8.0");
                            }
                            Some(FieldType::LorenzAttractor) => {
                                ui.label("Lorenz Parameters:");
                                ui.label("  sigma = 10.0");
                                ui.label("  rho = 28.0");
                                ui.label("  beta = 2.667");
                                ui.label("  scale = 0.2");
                            }
                            Some(FieldType::RosslerAttractor) => {
                                ui.label("Rossler Parameters:");
                                ui.label("  a = 0.2, b = 0.2, c = 5.7");
                            }
                            Some(FieldType::Flow) => {
                                ui.label("Direction: (0, -1)");
                                ui.label("Strength: 0.3");
                                ui.label("Turbulence: 0.2");
                            }
                            _ => { ui.label("(default parameters)"); }
                        }

                        if changed { state.needs_rebuild = true; }
                    }

                    if ui.button("Delete Field").clicked() {
                        state.push_undo("Delete Field");
                        state.document.remove_node(*id);
                        state.document.selection.retain(|s| s != id);
                        state.needs_rebuild = true;
                    }
                });
            }
        });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Post-processing panel
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn postfx_panel(ctx: &egui::Context, state: &mut EditorState, engine: &mut ProofEngine) {
    if !state.show_postfx_panel { return; }

    egui::Window::new("Post-Processing")
        .default_width(300.0)
        .show(ctx, |ui| {
            // Bloom
            egui::CollapsingHeader::new("Bloom").default_open(true).show(ui, |ui| {
                ui.checkbox(&mut engine.config.render.bloom_enabled, "Enabled");
                ui.add(egui::Slider::new(&mut engine.config.render.bloom_intensity, 0.0..=5.0).text("Intensity"));
                ui.add(egui::Slider::new(&mut engine.config.render.bloom_radius, 1.0..=32.0).text("Radius"));
            });

            // Chromatic Aberration
            egui::CollapsingHeader::new("Chromatic Aberration").default_open(true).show(ui, |ui| {
                ui.add(egui::Slider::new(&mut engine.config.render.chromatic_aberration, 0.0..=0.02).text("Strength"));
            });

            // Film Grain
            egui::CollapsingHeader::new("Film Grain").default_open(true).show(ui, |ui| {
                ui.add(egui::Slider::new(&mut engine.config.render.film_grain, 0.0..=0.1).text("Amount"));
            });

            // Motion Blur
            egui::CollapsingHeader::new("Motion Blur").default_open(false).show(ui, |ui| {
                ui.checkbox(&mut engine.config.render.motion_blur_enabled, "Enabled");
            });

            ui.separator();
            ui.heading("Presets");
            ui.horizontal(|ui| {
                if ui.button("Cinematic").clicked() {
                    engine.config.render.bloom_enabled = true;
                    engine.config.render.bloom_intensity = 2.0;
                    engine.config.render.chromatic_aberration = 0.005;
                    engine.config.render.film_grain = 0.03;
                }
                if ui.button("Clean").clicked() {
                    engine.config.render.bloom_enabled = true;
                    engine.config.render.bloom_intensity = 1.0;
                    engine.config.render.chromatic_aberration = 0.0;
                    engine.config.render.film_grain = 0.0;
                }
                if ui.button("Neon").clicked() {
                    engine.config.render.bloom_enabled = true;
                    engine.config.render.bloom_intensity = 3.5;
                    engine.config.render.chromatic_aberration = 0.003;
                    engine.config.render.film_grain = 0.01;
                }
                if ui.button("Retro").clicked() {
                    engine.config.render.bloom_enabled = true;
                    engine.config.render.bloom_intensity = 0.5;
                    engine.config.render.chromatic_aberration = 0.008;
                    engine.config.render.film_grain = 0.05;
                }
            });

            ui.separator();
            if ui.button("Shake!").clicked() { engine.add_trauma(0.5); }
        });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Asset browser
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn asset_browser(ctx: &egui::Context, state: &mut EditorState, engine: &mut ProofEngine) {
    if !state.show_asset_browser { return; }

    egui::Window::new("Asset Browser")
        .default_width(400.0)
        .default_height(250.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Prefabs");
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label("Entity Presets:");
                let entity_presets = [
                    ("Lorenz Cluster", "12 glyphs orbiting a Lorenz attractor", FieldType::LorenzAttractor),
                    ("Vortex Ring", "Ring formation with vortex field", FieldType::Vortex),
                    ("Gravity Well", "Particles pulled to center", FieldType::GravityWell),
                    ("Rossler Flow", "Rossler attractor particle stream", FieldType::RosslerAttractor),
                    ("Repulsor Shield", "Outward-pushing field", FieldType::Repulsor),
                ];

                for (name, desc, field_type) in &entity_presets {
                    ui.horizontal(|ui| {
                        if ui.button(*name).clicked() {
                            // Spawn prefab at center
                            let pos = Vec3::new(state.cam_x, state.cam_y, 0.0);
                            state.push_undo("Spawn Prefab");

                            // Add entity
                            let eid = state.document.add_entity_node(pos);
                            // Add field
                            let fid = state.document.add_field_node(pos, *field_type);

                            state.needs_rebuild = true;
                            state.set_status(&format!("Spawned: {}", name));
                            state.log(&format!("Prefab: {} at ({:.1}, {:.1})", name, pos.x, pos.y), egui::Color32::from_rgb(100, 200, 255));
                        }
                        ui.label(*desc);
                    });
                }

                ui.separator();
                ui.label("Color Schemes:");
                for (name, colors) in COLOR_PALETTES {
                    ui.horizontal(|ui| {
                        ui.label(*name);
                        for &(r, g, b) in *colors {
                            let color = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, color);
                        }
                    });
                }

                ui.separator();
                ui.label("Character Sets:");
                for (name, chars) in CHAR_PALETTES {
                    ui.horizontal(|ui| {
                        ui.label(*name);
                        ui.label(chars.iter().collect::<String>());
                    });
                }
            });
        });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Console/log panel
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn console_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_console { return; }

    egui::Window::new("Console")
        .default_width(500.0)
        .default_height(200.0)
        .show(ctx, |ui| {
            // Log area
            let scroll = egui::ScrollArea::vertical()
                .max_height(150.0)
                .stick_to_bottom(true);
            scroll.show(ui, |ui| {
                for (text, color) in &state.console_log {
                    ui.colored_label(*color, text);
                }
            });

            ui.separator();

            // Command input
            ui.horizontal(|ui| {
                ui.label(">");
                let response = ui.text_edit_singleline(&mut state.console_input);
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let cmd = state.console_input.clone();
                    state.console_log.push((format!("> {}", cmd), egui::Color32::WHITE));

                    // Process command
                    match cmd.trim() {
                        "help" => {
                            state.log("Commands: help, clear, stats, save, load, new", egui::Color32::from_rgb(100, 180, 255));
                        }
                        "clear" => { state.console_log.clear(); }
                        "stats" => {
                            state.log(&format!("Nodes: {}, Glyphs: {}, Fields: {}, Undo: {}",
                                state.document.node_count(), state.document.glyph_count(),
                                state.document.field_count(), state.undo_stack.len()),
                                egui::Color32::from_rgb(200, 200, 200));
                        }
                        "save" => { let _ = state.document.save("scene.json"); state.log("Saved", egui::Color32::GREEN); }
                        "load" => {
                            if let Ok(doc) = SceneDocument::load("scene.json") {
                                state.document = doc; state.needs_rebuild = true; state.log("Loaded", egui::Color32::GREEN);
                            }
                        }
                        "new" => { state.document = SceneDocument::new(); state.needs_rebuild = true; state.log("New scene", egui::Color32::GREEN); }
                        _ => { state.log(&format!("Unknown command: {}", cmd), egui::Color32::from_rgb(255, 150, 100)); }
                    }

                    state.console_input.clear();
                }
            });
        });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Help window
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn help_window(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_help { return; }

    egui::Window::new("Help").show(ctx, |ui| {
        ui.heading("Proof Editor Controls");
        ui.separator();

        egui::Grid::new("help_grid").show(ui, |ui| {
            ui.label("Click viewport"); ui.label("Place with current tool"); ui.end_row();
            ui.label("WASD / Arrows"); ui.label("Pan camera"); ui.end_row();
            ui.label("V / G / P / F / E / X"); ui.label("Select/Move/Place/Field/Entity/Burst"); ui.end_row();
            ui.label("Shift+Click"); ui.label("Multi-select"); ui.end_row();
            ui.label("Drag (Select tool)"); ui.label("Box select"); ui.end_row();
            ui.label("Drag (Move tool)"); ui.label("Reposition selected"); ui.end_row();
            ui.label("Ctrl+C / Ctrl+V"); ui.label("Copy / Paste"); ui.end_row();
            ui.label("Ctrl+Z / Ctrl+Y"); ui.label("Undo / Redo"); ui.end_row();
            ui.label("Ctrl+S / Ctrl+O"); ui.label("Save / Load"); ui.end_row();
            ui.label("Ctrl+N"); ui.label("New scene"); ui.end_row();
            ui.label("Ctrl+D"); ui.label("Duplicate selection"); ui.end_row();
            ui.label("Delete"); ui.label("Remove selection"); ui.end_row();
            ui.label("Escape"); ui.label("Cancel / Deselect"); ui.end_row();
            ui.label("Space"); ui.label("Screen shake"); ui.end_row();
            ui.label("F1"); ui.label("Toggle this help"); ui.end_row();
        });

        if ui.button("Close").clicked() { state.show_help = false; }
    });
}

// ============================================================
// World Editor Panel
// ============================================================

pub fn world_editor_panel(ctx: &egui::Context, state: &mut EditorState, _engine: &mut ProofEngine) {
    if !state.show_world_editor { return; }
    let mut open = state.show_world_editor;
    egui::Window::new("World Editor").open(&mut open).default_width(320.0).show(ctx, |ui| {
        egui::CollapsingHeader::new("Terrain").default_open(true).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Seed:");
                let mut seed_str = state.world_seed.to_string();
                if ui.text_edit_singleline(&mut seed_str).changed() {
                    if let Ok(v) = seed_str.parse::<u64>() { state.world_seed = v; }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Size:");
                egui::ComboBox::from_id_salt("world_size").selected_text("1024").show_ui(ui, |ui| {
                    let _ = ui.selectable_label(false, "512");
                    let _ = ui.selectable_label(true, "1024");
                    let _ = ui.selectable_label(false, "2048");
                });
            });
            let _ = ui.button("Erode Terrain");
        });
        egui::CollapsingHeader::new("Biomes").default_open(true).show(ui, |ui| {
            ui.horizontal(|ui| { ui.colored_label(egui::Color32::from_rgb(100, 160, 255), "Tundra"); ui.label("- Cold, icy plains"); });
            ui.horizontal(|ui| { ui.colored_label(egui::Color32::from_rgb(255, 180, 60), "Desert"); ui.label("- Arid, sandy dunes"); });
            ui.horizontal(|ui| { ui.colored_label(egui::Color32::from_rgb(60, 200, 80), "Forest"); ui.label("- Dense woodland"); });
            ui.horizontal(|ui| { ui.colored_label(egui::Color32::from_rgb(60, 220, 220), "Ocean"); ui.label("- Deep water biome"); });
            ui.horizontal(|ui| { ui.colored_label(egui::Color32::from_rgb(160, 160, 160), "Mountain"); ui.label("- High altitude peaks"); });
        });
        egui::CollapsingHeader::new("Atmosphere").default_open(false).show(ui, |ui| {
            static mut SUN: f32 = 45.0; static mut MIE: f32 = 0.3; static mut RAY: f32 = 0.5;
            #[allow(static_mut_refs)]
            unsafe {
                ui.add(egui::Slider::new(&mut SUN, 0.0..=360.0).text("Sun Angle"));
                ui.add(egui::Slider::new(&mut MIE, 0.0..=1.0).text("Mie Scattering"));
                ui.add(egui::Slider::new(&mut RAY, 0.0..=1.0).text("Rayleigh"));
            }
        });
        egui::CollapsingHeader::new("Weather").default_open(false).show(ui, |ui| {
            static mut WEATHER: usize = 0;
            #[allow(static_mut_refs)]
            unsafe {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut WEATHER, 0, "Clear");
                    ui.radio_value(&mut WEATHER, 1, "Cloudy");
                    ui.radio_value(&mut WEATHER, 2, "Rain");
                    ui.radio_value(&mut WEATHER, 3, "Storm");
                });
            }
        });
    });
    state.show_world_editor = open;
}

// ============================================================
// AI Behavior Panel
// ============================================================

pub fn ai_behavior_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_ai_behavior { return; }
    let mut open = state.show_ai_behavior;
    egui::Window::new("AI Behavior").open(&mut open).default_width(300.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Tree:");
            ui.text_edit_singleline(&mut state.ai_selected_tree);
            if ui.button("New Tree").clicked() { state.ai_selected_tree = "NewTree".to_string(); }
        });
        egui::CollapsingHeader::new("Node Types").default_open(true).show(ui, |ui| {
            egui::Grid::new("ai_nodes").show(ui, |ui| {
                ui.colored_label(egui::Color32::from_rgb(60, 200, 80), "●"); ui.label("Sequence"); ui.label("Runs children in order"); ui.end_row();
                ui.colored_label(egui::Color32::from_rgb(60, 140, 255), "●"); ui.label("Selector"); ui.label("Tries until one succeeds"); ui.end_row();
                ui.colored_label(egui::Color32::from_rgb(255, 220, 40), "●"); ui.label("Parallel"); ui.label("Runs all children"); ui.end_row();
                ui.colored_label(egui::Color32::WHITE, "●"); ui.label("Action"); ui.label("Leaf node, executes"); ui.end_row();
                ui.colored_label(egui::Color32::from_rgb(255, 160, 60), "●"); ui.label("Condition"); ui.label("Boolean check"); ui.end_row();
            });
        });
        egui::CollapsingHeader::new("Blackboard").default_open(true).show(ui, |ui| {
            egui::Grid::new("bb_grid").show(ui, |ui| {
                ui.label("health"); ui.label("85.0"); ui.end_row();
                ui.label("target_visible"); ui.label("true"); ui.end_row();
                ui.label("distance_to_target"); ui.label("12.4"); ui.end_row();
                ui.label("last_seen_pos"); ui.label("(3.2, -1.5)"); ui.end_row();
            });
        });
        egui::CollapsingHeader::new("Debug").default_open(false).show(ui, |ui| {
            static mut SHOW_TREE: bool = true; static mut SHOW_PATH: bool = false; static mut SHOW_VIS: bool = false;
            #[allow(static_mut_refs)]
            unsafe {
                ui.checkbox(&mut SHOW_TREE, "Show Tree");
                ui.checkbox(&mut SHOW_PATH, "Show Path");
                ui.checkbox(&mut SHOW_VIS, "Show Vision");
            }
        });
    });
    state.show_ai_behavior = open;
}

// ============================================================
// Physics Panel
// ============================================================

pub fn physics_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_physics { return; }
    let mut open = state.show_physics;
    egui::Window::new("Physics").open(&mut open).default_width(300.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Body:");
            ui.text_edit_singleline(&mut state.physics_selected_body);
        });
        egui::CollapsingHeader::new("Rigid Body").default_open(true).show(ui, |ui| {
            static mut MASS: f32 = 1.0; static mut REST: f32 = 0.3; static mut FRIC: f32 = 0.5; static mut LDAMP: f32 = 0.1;
            #[allow(static_mut_refs)]
            unsafe {
                ui.add(egui::Slider::new(&mut MASS, 0.1..=1000.0).text("Mass"));
                ui.add(egui::Slider::new(&mut REST, 0.0..=1.0).text("Restitution"));
                ui.add(egui::Slider::new(&mut FRIC, 0.0..=1.0).text("Friction"));
                ui.add(egui::Slider::new(&mut LDAMP, 0.0..=1.0).text("Linear Damping"));
            }
        });
        egui::CollapsingHeader::new("Constraints").default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                let _ = ui.button("Fixed");
                let _ = ui.button("Hinge");
                let _ = ui.button("Slider");
                let _ = ui.button("Ball-Socket");
            });
        });
        egui::CollapsingHeader::new("Simulation").default_open(false).show(ui, |ui| {
            static mut GX: f32 = 0.0; static mut GY: f32 = -9.81; static mut STEPS: u32 = 4;
            static mut CCD: bool = false; static mut SLEEP: bool = true;
            #[allow(static_mut_refs)]
            unsafe {
                ui.add(egui::Slider::new(&mut GX, -20.0..=20.0).text("Gravity X"));
                ui.add(egui::Slider::new(&mut GY, -20.0..=20.0).text("Gravity Y"));
                ui.add(egui::Slider::new(&mut STEPS, 1..=10).text("Substeps"));
                ui.checkbox(&mut CCD, "Enable CCD");
                ui.checkbox(&mut SLEEP, "Enable Sleeping");
            }
        });
    });
    state.show_physics = open;
}

// ============================================================
// Render Graph Panel
// ============================================================

pub fn render_graph_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_render_graph { return; }
    let mut open = state.show_render_graph;
    egui::Window::new("Render Graph").open(&mut open).default_width(320.0).show(ctx, |ui| {
        egui::CollapsingHeader::new("Pass List").default_open(true).show(ui, |ui| {
            let passes = [
                ("GBuffer", egui::Color32::from_rgb(60, 200, 80)),
                ("Shadow Map", egui::Color32::from_rgb(60, 140, 255)),
                ("Lighting", egui::Color32::from_rgb(255, 220, 40)),
                ("SSAO", egui::Color32::from_rgb(180, 80, 255)),
                ("Bloom", egui::Color32::from_rgb(255, 120, 180)),
                ("Tonemap", egui::Color32::WHITE),
            ];
            egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                egui::Grid::new("passes").show(ui, |ui| {
                    for (name, color) in &passes {
                        ui.colored_label(*color, "●");
                        ui.label(*name);
                        ui.label("Ready");
                        ui.end_row();
                    }
                });
            });
        });
        egui::CollapsingHeader::new("Resource Budget").default_open(true).show(ui, |ui| {
            egui::Grid::new("rg_budget").show(ui, |ui| {
                ui.label("Textures:"); ui.label("128 MB"); ui.end_row();
                ui.label("Buffers:"); ui.label("32 MB"); ui.end_row();
                ui.label("Render targets:"); ui.label("64 MB"); ui.end_row();
            });
        });
        let _ = ui.button("Compile Graph");
    });
    state.show_render_graph = open;
}

// ============================================================
// Dialogue Panel
// ============================================================

pub fn dialogue_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_dialogue { return; }
    let mut open = state.show_dialogue;
    egui::Window::new("Dialogue Editor").open(&mut open).default_width(320.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("dlg_tree").selected_text("Merchant_01").show_ui(ui, |ui| {
                let _ = ui.selectable_label(true, "Merchant_01");
                let _ = ui.selectable_label(false, "Guard_Intro");
                let _ = ui.selectable_label(false, "QuestGiver_A");
            });
            if ui.button("New Dialogue").clicked() {}
        });
        ui.label("Nodes: 24");
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut state.dialogue_search);
        });
        egui::CollapsingHeader::new("Node Breakdown").default_open(true).show(ui, |ui| {
            egui::Grid::new("dlg_types").show(ui, |ui| {
                ui.label("Speaker:"); ui.label("12"); ui.end_row();
                ui.label("Choice:"); ui.label("7"); ui.end_row();
                ui.label("Condition:"); ui.label("3"); ui.end_row();
                ui.label("Jump:"); ui.label("2"); ui.end_row();
            });
        });
        egui::CollapsingHeader::new("Localization").default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Language:");
                egui::ComboBox::from_id_salt("dlg_lang").selected_text("EN").show_ui(ui, |ui| {
                    for lang in &["EN", "FR", "DE", "ES", "JA"] {
                        let _ = ui.selectable_label(false, *lang);
                    }
                });
            });
            ui.label("Missing keys: 3");
        });
    });
    state.show_dialogue = open;
}

// ============================================================
// Quest Panel
// ============================================================

pub fn quest_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_quest { return; }
    let mut open = state.show_quest;
    egui::Window::new("Quest Editor").open(&mut open).default_width(320.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut state.quest_search);
        });
        egui::CollapsingHeader::new("Quests").default_open(true).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                let quests = [
                    ("The Lost Sword", egui::Color32::from_rgb(60, 200, 80), "Active"),
                    ("Merchant Plea", egui::Color32::from_rgb(60, 200, 80), "Active"),
                    ("Clear the Mines", egui::Color32::from_rgb(160, 160, 160), "Complete"),
                    ("Bandit Camp", egui::Color32::from_rgb(255, 100, 80), "Failed"),
                    ("Ancient Relic", egui::Color32::from_rgb(60, 200, 80), "Active"),
                ];
                for (name, color, status) in &quests {
                    ui.horizontal(|ui| {
                        ui.colored_label(*color, *status);
                        ui.label(*name);
                    });
                }
            });
        });
        egui::CollapsingHeader::new("Objectives").default_open(true).show(ui, |ui| {
            ui.label("Kill Bandits:");
            ui.add(egui::ProgressBar::new(0.6).text("6/10"));
            static mut REACHED: bool = false;
            #[allow(static_mut_refs)]
            unsafe { ui.checkbox(&mut REACHED, "Reach Location"); }
        });
        egui::CollapsingHeader::new("Rewards").default_open(false).show(ui, |ui| {
            egui::Grid::new("quest_rewards").show(ui, |ui| {
                ui.label("XP:"); ui.label("500"); ui.end_row();
                ui.label("Gold:"); ui.label("100"); ui.end_row();
                ui.label("Item:"); ui.label("Iron Sword"); ui.end_row();
            });
        });
        egui::CollapsingHeader::new("Faction Standing").default_open(false).show(ui, |ui| {
            static mut F1: f32 = 20.0; static mut F2: f32 = -40.0; static mut F3: f32 = 10.0;
            #[allow(static_mut_refs)]
            unsafe {
                ui.add(egui::Slider::new(&mut F1, -100.0..=100.0).text("Merchants"));
                ui.add(egui::Slider::new(&mut F2, -100.0..=100.0).text("Bandits"));
                ui.add(egui::Slider::new(&mut F3, -100.0..=100.0).text("Guards"));
            }
        });
    });
    state.show_quest = open;
}

// ============================================================
// Spline Panel
// ============================================================

pub fn spline_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_spline { return; }
    let mut open = state.show_spline;
    egui::Window::new("Spline Editor").open(&mut open).default_width(280.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Type:");
            egui::ComboBox::from_id_salt("spline_type").selected_text("Catmull-Rom").show_ui(ui, |ui| {
                let _ = ui.selectable_label(true, "Catmull-Rom");
                let _ = ui.selectable_label(false, "Bezier");
                let _ = ui.selectable_label(false, "B-Spline");
            });
        });
        egui::Grid::new("spline_info").show(ui, |ui| {
            ui.label("Control Points:"); ui.label("8"); ui.end_row();
            ui.label("Total Length:"); ui.label("42.3 units"); ui.end_row();
        });
        static mut ARC_LEN: bool = true; static mut CLOSED: bool = false; static mut TENSION: f32 = 0.5;
        #[allow(static_mut_refs)]
        unsafe {
            ui.checkbox(&mut ARC_LEN, "Arc-length Parameterization");
            ui.checkbox(&mut CLOSED, "Closed Loop");
            ui.add(egui::Slider::new(&mut TENSION, 0.0..=1.0).text("Tension"));
        }
        egui::CollapsingHeader::new("Rail System").default_open(false).show(ui, |ui| {
            static mut GAUGE: f32 = 1.435; static mut BANK: f32 = 5.0;
            #[allow(static_mut_refs)]
            unsafe {
                ui.add(egui::Slider::new(&mut GAUGE, 0.5..=3.0).text("Gauge Width"));
                ui.label(format!("Banking Angle: {:.1}°", BANK));
                ui.add(egui::Slider::new(&mut BANK, -45.0..=45.0).text("Banking"));
            }
        });
    });
    state.show_spline = open;
}

// ============================================================
// Cinematic Panel
// ============================================================

pub fn cinematic_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_cinematic { return; }
    let mut open = state.show_cinematic;
    egui::Window::new("Cinematic Editor").open(&mut open).default_width(340.0).show(ctx, |ui| {
        egui::CollapsingHeader::new("Timeline").default_open(true).show(ui, |ui| {
            static mut PLAYING: bool = false; static mut TIME: f32 = 0.0;
            #[allow(static_mut_refs)]
            unsafe {
                ui.horizontal(|ui| {
                    if ui.button(if PLAYING { "Pause" } else { "Play" }).clicked() { PLAYING = !PLAYING; }
                    if ui.button("Stop").clicked() { PLAYING = false; TIME = 0.0; }
                });
                ui.label(format!("Time: {:.2}s / 120.00s", TIME));
                ui.label("Timecode: 00:02:00:00 (SMPTE)");
            }
        });
        egui::CollapsingHeader::new("Tracks").default_open(true).show(ui, |ui| {
            static mut CAM_VIS: bool = true; static mut ACT_VIS: bool = true;
            static mut ANI_VIS: bool = true; static mut AUD_VIS: bool = true;
            #[allow(static_mut_refs)]
            unsafe {
                egui::Grid::new("tracks").show(ui, |ui| {
                    ui.checkbox(&mut CAM_VIS, ""); ui.label("Camera"); ui.label("CameraTrack_01"); ui.end_row();
                    ui.checkbox(&mut ACT_VIS, ""); ui.label("Actor"); ui.label("Player_Anim"); ui.end_row();
                    ui.checkbox(&mut ANI_VIS, ""); ui.label("Animation"); ui.label("Cutscene_A"); ui.end_row();
                    ui.checkbox(&mut AUD_VIS, ""); ui.label("Audio"); ui.label("Music_Dramatic"); ui.end_row();
                });
            }
        });
        egui::CollapsingHeader::new("Export").default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Format:");
                egui::ComboBox::from_id_salt("cin_export").selected_text("MP4").show_ui(ui, |ui| {
                    let _ = ui.selectable_label(false, "EDL");
                    let _ = ui.selectable_label(false, "SRT");
                    let _ = ui.selectable_label(true, "MP4");
                });
                let _ = ui.button("Export");
            });
        });
    });
    state.show_cinematic = open;
}

// ============================================================
// Inventory Panel
// ============================================================

pub fn inventory_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_inventory { return; }
    let mut open = state.show_inventory;
    egui::Window::new("Inventory").open(&mut open).default_width(360.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut state.inventory_search);
        });
        ui.horizontal(|ui| {
            let _ = ui.button("All");
            let _ = ui.button("Weapon");
            let _ = ui.button("Armor");
            let _ = ui.button("Consumable");
        });
        let items = [
            ("Iron Sword", egui::Color32::WHITE, "Common", "1H Blade, 15 dmg"),
            ("Steel Shield", egui::Color32::from_rgb(60, 200, 80), "Uncommon", "Block 40%"),
            ("Elven Bow", egui::Color32::from_rgb(60, 140, 255), "Rare", "Range 30m, 22 dmg"),
            ("Arcane Staff", egui::Color32::from_rgb(180, 80, 255), "Epic", "Spell power +40"),
            ("Dragon Scale Armor", egui::Color32::from_rgb(255, 160, 40), "Legendary", "Armor 80, Fire res"),
            ("Health Potion", egui::Color32::WHITE, "Common", "Restore 50 HP"),
            ("Mana Crystal", egui::Color32::from_rgb(60, 200, 80), "Uncommon", "Restore 30 MP"),
            ("Shadow Cloak", egui::Color32::from_rgb(60, 140, 255), "Rare", "Stealth +25"),
        ];
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            egui::Grid::new("inv_items").striped(true).show(ui, |ui| {
                for (name, color, rarity, desc) in &items {
                    ui.colored_label(*color, *rarity);
                    ui.label(*name);
                    ui.label(*desc);
                    ui.end_row();
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Sort by:");
            egui::ComboBox::from_id_salt("inv_sort").selected_text("Rarity").show_ui(ui, |ui| {
                let _ = ui.selectable_label(false, "Name");
                let _ = ui.selectable_label(true, "Rarity");
                let _ = ui.selectable_label(false, "Value");
                let _ = ui.selectable_label(false, "Weight");
            });
        });
    });
    state.show_inventory = open;
}

// ============================================================
// Ability Panel
// ============================================================

pub fn ability_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_ability { return; }
    let mut open = state.show_ability;
    egui::Window::new("Ability Editor").open(&mut open).default_width(320.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut state.ability_search);
        });
        ui.horizontal(|ui| {
            let _ = ui.button("Active");
            let _ = ui.button("Passive");
            let _ = ui.button("Toggle");
        });
        let abilities = [
            ("Fireball", "8.0s", "Active"),
            ("Ice Lance", "3.0s", "Active"),
            ("Shield Bash", "5.0s", "Active"),
            ("Berserker", "-", "Passive"),
            ("Stealth", "toggle", "Toggle"),
            ("Healing Aura", "15.0s", "Active"),
        ];
        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
            egui::Grid::new("ability_list").show(ui, |ui| {
                for (name, cd, kind) in &abilities {
                    ui.label(*kind);
                    ui.label(*name);
                    ui.label(format!("CD: {}", cd));
                    ui.end_row();
                }
            });
        });
        egui::CollapsingHeader::new("Selected: Fireball").default_open(true).show(ui, |ui| {
            static mut DMG_MIN: f32 = 40.0; static mut DMG_MAX: f32 = 80.0;
            static mut COST: f32 = 30.0; static mut RANGE: f32 = 20.0; static mut AREA: f32 = 4.0;
            #[allow(static_mut_refs)]
            unsafe {
                ui.add(egui::Slider::new(&mut DMG_MIN, 0.0..=200.0).text("Damage Min"));
                ui.add(egui::Slider::new(&mut DMG_MAX, 0.0..=200.0).text("Damage Max"));
                ui.add(egui::Slider::new(&mut COST, 0.0..=100.0).text("Mana Cost"));
                ui.add(egui::Slider::new(&mut RANGE, 0.0..=50.0).text("Range"));
                ui.add(egui::Slider::new(&mut AREA, 0.0..=20.0).text("Area"));
            }
        });
        egui::CollapsingHeader::new("Status Effects").default_open(false).show(ui, |ui| {
            let effects = [("Burning", 0.6f32), ("Stunned", 0.2f32), ("Weakened", 0.8f32)];
            for (name, pct) in &effects {
                ui.horizontal(|ui| {
                    ui.label(*name);
                    ui.add(egui::ProgressBar::new(*pct).desired_width(100.0));
                });
            }
        });
    });
    state.show_ability = open;
}

// ============================================================
// Level Streaming Panel
// ============================================================

pub fn level_streaming_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_level_streaming { return; }
    let mut open = state.show_level_streaming;
    egui::Window::new("Level Streaming").open(&mut open).default_width(320.0).show(ctx, |ui| {
        egui::Grid::new("ls_info").show(ui, |ui| {
            ui.label("World Size:"); ui.label("4096 x 4096 m"); ui.end_row();
            ui.label("Loaded Cells:"); ui.label("12 / 256"); ui.end_row();
        });
        static mut MEM_BUDGET: f32 = 2048.0; static mut STREAM_DIST: f32 = 800.0;
        #[allow(static_mut_refs)]
        unsafe {
            ui.add(egui::Slider::new(&mut MEM_BUDGET, 0.0..=8192.0).text("Memory Budget MB"));
            ui.add(egui::Slider::new(&mut STREAM_DIST, 100.0..=5000.0).text("Streaming Distance m"));
        }
        egui::CollapsingHeader::new("Cell List").default_open(true).show(ui, |ui| {
            let cells = [
                ("Cell_0_0", egui::Color32::from_rgb(60, 200, 80), "Loaded"),
                ("Cell_0_1", egui::Color32::from_rgb(60, 200, 80), "Loaded"),
                ("Cell_1_0", egui::Color32::from_rgb(255, 220, 40), "Loading"),
                ("Cell_1_1", egui::Color32::from_rgb(60, 200, 80), "Loaded"),
                ("Cell_2_0", egui::Color32::from_rgb(130, 130, 130), "Unloaded"),
                ("Cell_2_1", egui::Color32::from_rgb(130, 130, 130), "Unloaded"),
                ("Cell_3_0", egui::Color32::from_rgb(130, 130, 130), "Unloaded"),
                ("Cell_3_1", egui::Color32::from_rgb(255, 220, 40), "Loading"),
            ];
            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                egui::Grid::new("cell_list").show(ui, |ui| {
                    for (name, color, status) in &cells {
                        ui.colored_label(*color, *status);
                        ui.label(*name);
                        ui.end_row();
                    }
                });
            });
        });
        egui::CollapsingHeader::new("Priority Rules").default_open(false).show(ui, |ui| {
            ui.label("1. Player proximity (radius 800m)");
            ui.label("2. Camera frustum priority");
            ui.label("3. Last-used eviction policy");
        });
    });
    state.show_level_streaming = open;
}

// ============================================================
// Audio Mixer Panel
// ============================================================

pub fn audio_mixer_panel(ctx: &egui::Context, state: &mut EditorState) {
    if !state.show_audio_mixer { return; }
    let mut open = state.show_audio_mixer;
    egui::Window::new("Audio Mixer").open(&mut open).default_width(300.0).show(ctx, |ui| {
        ui.add(egui::Slider::new(&mut state.audio_master_volume, 0.0..=100.0).text("Master"));
        ui.separator();
        ui.add(egui::Slider::new(&mut state.audio_music_volume, 0.0..=100.0).text("Music"));
        ui.add(egui::Slider::new(&mut state.audio_sfx_volume, 0.0..=100.0).text("SFX"));
        static mut VOICE_VOL: f32 = 85.0;
        #[allow(static_mut_refs)]
        unsafe { ui.add(egui::Slider::new(&mut VOICE_VOL, 0.0..=100.0).text("Voice")); }
        egui::CollapsingHeader::new("Effects Chain (SFX Bus)").default_open(true).show(ui, |ui| {
            static mut EQ: bool = true; static mut COMP: bool = true; static mut REVERB: bool = false;
            static mut ROOM: f32 = 0.4;
            #[allow(static_mut_refs)]
            unsafe {
                ui.checkbox(&mut EQ, "EQ");
                ui.checkbox(&mut COMP, "Compressor");
                ui.checkbox(&mut REVERB, "Reverb");
                if REVERB {
                    ui.add(egui::Slider::new(&mut ROOM, 0.0..=1.0).text("Room Size"));
                }
            }
        });
        egui::CollapsingHeader::new("Spatial Audio").default_open(false).show(ui, |ui| {
            static mut MAX_DIST: f32 = 50.0;
            #[allow(static_mut_refs)]
            unsafe { ui.add(egui::Slider::new(&mut MAX_DIST, 1.0..=500.0).text("Max Distance")); }
            ui.horizontal(|ui| {
                ui.label("Rolloff:");
                egui::ComboBox::from_id_salt("rolloff").selected_text("Inverse").show_ui(ui, |ui| {
                    let _ = ui.selectable_label(false, "Linear");
                    let _ = ui.selectable_label(true, "Inverse");
                    let _ = ui.selectable_label(false, "Log");
                });
            });
        });
        ui.separator();
        ui.label("Active voices: 14 / 64");
    });
    state.show_audio_mixer = open;
}
