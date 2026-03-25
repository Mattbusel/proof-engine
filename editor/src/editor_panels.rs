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
