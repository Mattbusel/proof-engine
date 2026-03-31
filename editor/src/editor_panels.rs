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
    pub show_modeling: bool,
    // 3D Modeling
    pub model_brush: String,
    pub model_brush_radius: f32,
    pub model_brush_strength: f32,
    pub model_brush_density: f32,
    pub model_active_char: char,
    pub model_symmetry: String,
    pub model_layer: usize,
    pub model_layers: Vec<(String, bool)>,
    pub model_particle_count: usize,
    pub model_lod_level: usize,
    pub model_snap_grid: bool,
    pub model_snap_size: f32,
    pub model_pivot_x: f32,
    pub model_pivot_y: f32,
    pub model_pivot_z: f32,
    pub model_3d_mode: bool,
    pub model_cam_azimuth: f32,
    pub model_cam_elevation: f32,
    pub model_cam_distance: f32,
    pub model_wireframe: bool,
    pub model_show_normals: bool,
    pub model_show_grid: bool,
    pub model_selected_primitive: String,
    pub model_prim_size: f32,
    pub model_prim_segments: u32,
    pub model_history: Vec<String>,
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
    // New full-featured panel toggles
    pub show_behavior_tree: bool,
    pub show_dialogue_graph: bool,
    pub show_particle_editor: bool,
    pub show_material_system: bool,
    pub show_spline_editor: bool,
    pub show_quest_system: bool,
    pub show_audio_mixer_full: bool,
    pub show_physics_editor: bool,
    pub show_inventory_system: bool,
    pub show_world_gen: bool,
    // Sub-editor states
    pub behavior_tree_editor: crate::behavior_tree::BehaviorTreeEditor,
    pub dialogue_editor: crate::dialogue_graph::DialogueEditor,
    pub particle_editor: crate::particle_editor::ParticleEditor,
    pub material_editor: crate::material_system::MaterialEditor,
    pub spline_editor: crate::spline_editor::SplineEditor,
    pub quest_editor: crate::quest_system::QuestEditor,
    pub audio_mixer_editor: crate::audio_mixer::AudioMixerEditor,
    pub physics_editor: crate::physics_editor::PhysicsEditor,
    pub inventory_editor: crate::inventory_system::InventoryEditor,
    pub world_gen_editor: crate::world_gen::WorldGenEditor,
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
            show_modeling: false,
            model_brush: "Add".to_string(),
            model_brush_radius: 1.0,
            model_brush_strength: 0.5,
            model_brush_density: 10.0,
            model_active_char: '@',
            model_symmetry: "None".to_string(),
            model_layer: 0,
            model_layers: vec![("Layer 0".to_string(), true)],
            model_particle_count: 0,
            model_lod_level: 0,
            model_snap_grid: false,
            model_snap_size: 0.5,
            model_pivot_x: 0.0,
            model_pivot_y: 0.0,
            model_pivot_z: 0.0,
            model_3d_mode: false,
            model_cam_azimuth: 45.0,
            model_cam_elevation: 30.0,
            model_cam_distance: 15.0,
            model_wireframe: false,
            model_show_normals: false,
            model_show_grid: true,
            model_selected_primitive: "Sphere".to_string(),
            model_prim_size: 2.0,
            model_prim_segments: 16,
            model_history: Vec::new(),
            world_seed: 42, world_biome_filter: String::new(),
            ai_selected_tree: "BehaviorTree_Enemy".to_string(),
            physics_selected_body: "RigidBody_0".to_string(),
            dialogue_search: String::new(), quest_search: String::new(),
            inventory_search: String::new(), ability_search: String::new(),
            audio_master_volume: 80.0, audio_music_volume: 60.0, audio_sfx_volume: 75.0,
            show_behavior_tree: false,
            show_dialogue_graph: false,
            show_particle_editor: false,
            show_material_system: false,
            show_spline_editor: false,
            show_quest_system: false,
            show_audio_mixer_full: false,
            show_physics_editor: false,
            show_inventory_system: false,
            show_world_gen: false,
            behavior_tree_editor: crate::behavior_tree::BehaviorTreeEditor::new(),
            dialogue_editor: crate::dialogue_graph::DialogueEditor::new(),
            particle_editor: crate::particle_editor::ParticleEditor::new(),
            material_editor: crate::material_system::MaterialEditor::new(),
            spline_editor: crate::spline_editor::SplineEditor::new(),
            quest_editor: crate::quest_system::QuestEditor::new(),
            audio_mixer_editor: crate::audio_mixer::AudioMixerEditor::new(),
            physics_editor: crate::physics_editor::PhysicsEditor::new(),
            inventory_editor: crate::inventory_system::InventoryEditor::new(),
            world_gen_editor: crate::world_gen::WorldGenEditor::new(),
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
                ui.set_min_width(200.0);
                if shortcut_item(ui, "New", "Ctrl+N") {
                    state.push_undo("New Scene");
                    state.document = SceneDocument::new();
                    state.needs_rebuild = true;
                    state.set_status("New scene");
                    ui.close_menu();
                }
                if shortcut_item(ui, "Save", "Ctrl+S") {
                    match state.document.save("scene.json") {
                        Ok(_) => { state.set_status("Saved scene.json"); state.log("Saved scene.json", egui::Color32::from_rgb(100, 200, 100)); }
                        Err(e) => { state.set_status(&format!("Save failed: {}", e)); state.log(&format!("Save error: {}", e), egui::Color32::from_rgb(255, 100, 100)); }
                    }
                    ui.close_menu();
                }
                if shortcut_item(ui, "Load", "Ctrl+O") {
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
                ui.set_min_width(200.0);
                if shortcut_item(ui, "Undo", "Ctrl+Z") { state.undo(); ui.close_menu(); }
                if shortcut_item(ui, "Redo", "Ctrl+Y") { state.redo(); ui.close_menu(); }
                ui.separator();
                if shortcut_item(ui, "Select All", "Ctrl+A") { state.document.select_all(); ui.close_menu(); }
                if shortcut_item(ui, "Delete", "Del") {
                    state.push_undo("Delete");
                    let sel = state.document.selection.clone();
                    for id in sel { state.document.remove_node(id); }
                    state.document.selection.clear();
                    state.needs_rebuild = true;
                    ui.close_menu();
                }
                if shortcut_item(ui, "Duplicate", "Ctrl+D") {
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
                ui.set_min_width(200.0);
                if shortcut_item(ui, "Help", "F1") { state.show_help = !state.show_help; ui.close_menu(); }
                if ui.button("Console").clicked() { state.show_console = !state.show_console; ui.close_menu(); }
                if ui.button("Force Fields").clicked() { state.show_fields_panel = !state.show_fields_panel; ui.close_menu(); }
                if ui.button("Asset Browser").clicked() { state.show_asset_browser = !state.show_asset_browser; ui.close_menu(); }
                if ui.button("Post-Processing").clicked() { state.show_postfx_panel = !state.show_postfx_panel; ui.close_menu(); }
                ui.separator();
                if ui.button("Toggle Bloom").clicked() { engine.config.render.bloom_enabled = !engine.config.render.bloom_enabled; ui.close_menu(); }
                if ui.button("Reset Camera").clicked() { state.cam_x = 0.0; state.cam_y = 0.0; ui.close_menu(); }
            });
            ui.menu_button("Tools", |ui| {
                ui.set_min_width(200.0);
                if ui.button("[W]  World Editor").clicked() { state.show_world_editor = !state.show_world_editor; ui.close_menu(); }
                if ui.button("[AI] AI Behavior").clicked() { state.show_ai_behavior = !state.show_ai_behavior; ui.close_menu(); }
                if ui.button("[PH] Physics").clicked() { state.show_physics = !state.show_physics; ui.close_menu(); }
                if ui.button("[RG] Render Graph").clicked() { state.show_render_graph = !state.show_render_graph; ui.close_menu(); }
                ui.separator();
                if ui.button("[DL] Dialogue Editor").clicked() { state.show_dialogue = !state.show_dialogue; ui.close_menu(); }
                if ui.button("[QT] Quest Editor").clicked() { state.show_quest = !state.show_quest; ui.close_menu(); }
                if ui.button("[SP] Spline Editor").clicked() { state.show_spline = !state.show_spline; ui.close_menu(); }
                if ui.button("[CN] Cinematic Editor").clicked() { state.show_cinematic = !state.show_cinematic; ui.close_menu(); }
                ui.separator();
                if ui.button("[IN] Inventory").clicked() { state.show_inventory = !state.show_inventory; ui.close_menu(); }
                if ui.button("[AB] Ability Editor").clicked() { state.show_ability = !state.show_ability; ui.close_menu(); }
                if ui.button("[LS] Level Streaming").clicked() { state.show_level_streaming = !state.show_level_streaming; ui.close_menu(); }
                if ui.button("[AU] Audio Mixer").clicked() { state.show_audio_mixer = !state.show_audio_mixer; ui.close_menu(); }
                ui.separator();
                if ui.button("[3D] 3D Modeler").clicked() { state.show_modeling = !state.show_modeling; ui.close_menu(); }
                ui.separator();
                if ui.button("[BT] Behavior Tree Editor").clicked() { state.show_behavior_tree = !state.show_behavior_tree; ui.close_menu(); }
                if ui.button("[DG] Dialogue Graph").clicked() { state.show_dialogue_graph = !state.show_dialogue_graph; ui.close_menu(); }
                if ui.button("[PE] Particle Editor").clicked() { state.show_particle_editor = !state.show_particle_editor; ui.close_menu(); }
                if ui.button("[MT] Material System").clicked() { state.show_material_system = !state.show_material_system; ui.close_menu(); }
                if ui.button("[SE] Spline Editor (Full)").clicked() { state.show_spline_editor = !state.show_spline_editor; ui.close_menu(); }
                if ui.button("[QS] Quest System").clicked() { state.show_quest_system = !state.show_quest_system; ui.close_menu(); }
                if ui.button("[AM] Audio Mixer (Full)").clicked() { state.show_audio_mixer_full = !state.show_audio_mixer_full; ui.close_menu(); }
                if ui.button("[PH] Physics Editor").clicked() { state.show_physics_editor = !state.show_physics_editor; ui.close_menu(); }
                if ui.button("[IV] Inventory System").clicked() { state.show_inventory_system = !state.show_inventory_system; ui.close_menu(); }
                if ui.button("[WG] World Generator").clicked() { state.show_world_gen = !state.show_world_gen; ui.close_menu(); }
            });
        });
    });
}

/// Render a menu item with a right-aligned keyboard shortcut hint.
fn shortcut_item(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    let mut clicked = false;
    let item_resp = ui.horizontal(|ui| {
        let w = ui.available_width();
        if ui.add_sized([w, 20.0], egui::Button::new(
            egui::RichText::new(format!("{:<24}{}", label, shortcut))
                .monospace()
        )).clicked() {
            clicked = true;
        }
    });
    let _ = item_resp;
    clicked
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Hierarchy panel — tree structure with search, icons, collapse
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn hierarchy_panel(ctx: &egui::Context, state: &mut EditorState) {
    static mut SEARCH: String = String::new();
    static mut FILTER: Option<NodeKind> = None;

    const ACCENT: egui::Color32 = egui::Color32::from_rgb(70, 130, 200);
    const PANEL_HEADER: egui::Color32 = egui::Color32::from_rgb(30, 33, 42);

    egui::SidePanel::left("hierarchy")
        .default_width(210.0)
        .min_width(160.0)
        .show(ctx, |ui| {
            // Panel title bar
            let title_rect = ui.available_rect_before_wrap();
            ui.painter().rect_filled(
                egui::Rect::from_min_size(title_rect.min, egui::vec2(title_rect.width(), 28.0)),
                0.0, PANEL_HEADER,
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("HIERARCHY").size(11.0).strong()
                    .color(egui::Color32::from_rgb(160, 170, 190)));
            });
            ui.add_space(4.0);
            ui.separator();

            // Search bar
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                unsafe { ui.add(egui::TextEdit::singleline(&mut SEARCH)
                    .hint_text("Search nodes...")
                    .desired_width(f32::INFINITY)); }
            });
            ui.add_space(4.0);

            // Tab-style filter buttons
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let filter = unsafe { &mut FILTER };
                let tabs = [
                    (None, "All"),
                    (Some(NodeKind::Glyph), "@ Glyphs"),
                    (Some(NodeKind::Field), "~ Fields"),
                    (Some(NodeKind::Entity), "# Entities"),
                ];
                for (kind, label) in &tabs {
                    let selected = *filter == *kind;
                    let txt = egui::RichText::new(*label).size(11.0);
                    let btn = egui::Button::new(txt)
                        .fill(if selected { ACCENT } else { egui::Color32::from_rgb(35, 37, 46) })
                        .stroke(egui::Stroke::new(1.0, if selected { ACCENT } else { egui::Color32::from_rgb(55, 58, 70) }));
                    if ui.add(btn).clicked() { *filter = *kind; }
                }
            });

            ui.add_space(4.0);
            ui.separator();

            if state.document.node_count() == 0 {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Empty scene").color(egui::Color32::from_rgb(100, 105, 120)));
                    ui.label(egui::RichText::new("Click viewport to place").size(11.0).color(egui::Color32::from_rgb(80, 85, 100)));
                });
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                let search = unsafe { SEARCH.to_lowercase() };
                let filter = unsafe { FILTER };
                let mut clicked_id: Option<u32> = None;
                let mut ctx_action: Option<(u32, &'static str)> = None;

                let entities: Vec<(u32, String)> = state.document.nodes()
                    .filter(|n| n.kind == NodeKind::Entity)
                    .filter(|n| filter.map_or(true, |f| f == n.kind))
                    .filter(|n| search.is_empty() || n.name.to_lowercase().contains(&search))
                    .map(|n| (n.id, n.name.clone()))
                    .collect();
                let fields: Vec<(u32, String, Option<FieldType>)> = state.document.nodes()
                    .filter(|n| n.kind == NodeKind::Field)
                    .filter(|n| filter.map_or(true, |f| f == n.kind))
                    .filter(|n| search.is_empty() || n.name.to_lowercase().contains(&search))
                    .map(|n| (n.id, n.name.clone(), n.field_type))
                    .collect();
                let glyphs: Vec<(u32, String, Option<char>)> = state.document.nodes()
                    .filter(|n| n.kind == NodeKind::Glyph)
                    .filter(|n| filter.map_or(true, |f| f == n.kind))
                    .filter(|n| search.is_empty() || n.name.to_lowercase().contains(&search))
                    .map(|n| (n.id, n.name.clone(), n.character))
                    .collect();

                // Section header helper (closure-like macro pattern)
                let section_color = egui::Color32::from_rgb(36, 38, 48);

                // Entities section
                if !entities.is_empty() && filter != Some(NodeKind::Glyph) && filter != Some(NodeKind::Field) {
                    let bg = ui.available_rect_before_wrap();
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(bg.min, egui::vec2(bg.width(), 20.0)),
                        0.0, section_color,
                    );
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("  # Entities  ({})", entities.len()))
                            .size(11.0).strong().color(egui::Color32::from_rgb(180, 120, 255))
                    ).default_open(true).show(ui, |ui| {
                        for (id, name) in &entities {
                            let sel = state.document.selection.contains(id);
                            let label = egui::RichText::new(format!("   # {}", name))
                                .color(if sel { egui::Color32::WHITE } else { egui::Color32::from_rgb(200, 160, 255) });
                            let resp = ui.selectable_label(sel, label);
                            if resp.clicked() { clicked_id = Some(*id); }
                            resp.context_menu(|ui| {
                                if ui.button("Duplicate").clicked() { ctx_action = Some((*id, "dup")); ui.close_menu(); }
                                if ui.button("Delete").clicked() { ctx_action = Some((*id, "del")); ui.close_menu(); }
                                if ui.button("Focus").clicked() { ctx_action = Some((*id, "focus")); ui.close_menu(); }
                            });
                        }
                    });
                }

                // Fields section
                if !fields.is_empty() && filter != Some(NodeKind::Glyph) && filter != Some(NodeKind::Entity) {
                    let bg = ui.available_rect_before_wrap();
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(bg.min, egui::vec2(bg.width(), 20.0)),
                        0.0, section_color,
                    );
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("  ~ Force Fields  ({})", fields.len()))
                            .size(11.0).strong().color(egui::Color32::from_rgb(255, 180, 80))
                    ).default_open(true).show(ui, |ui| {
                        for (id, name, ft) in &fields {
                            let sel = state.document.selection.contains(id);
                            let ft_name = ft.as_ref().map(|f| f.label()).unwrap_or("?");
                            let label = egui::RichText::new(format!("   ~ {} [{}]", name, ft_name))
                                .color(if sel { egui::Color32::WHITE } else { egui::Color32::from_rgb(255, 200, 120) });
                            let resp = ui.selectable_label(sel, label);
                            if resp.clicked() { clicked_id = Some(*id); }
                            resp.context_menu(|ui| {
                                if ui.button("Duplicate").clicked() { ctx_action = Some((*id, "dup")); ui.close_menu(); }
                                if ui.button("Delete").clicked() { ctx_action = Some((*id, "del")); ui.close_menu(); }
                                if ui.button("Focus").clicked() { ctx_action = Some((*id, "focus")); ui.close_menu(); }
                            });
                        }
                    });
                }

                // Glyphs section
                if !glyphs.is_empty() && filter != Some(NodeKind::Field) && filter != Some(NodeKind::Entity) {
                    let bg = ui.available_rect_before_wrap();
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(bg.min, egui::vec2(bg.width(), 20.0)),
                        0.0, section_color,
                    );
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("  @ Glyphs  ({})", glyphs.len()))
                            .size(11.0).strong().color(egui::Color32::from_rgb(150, 210, 150))
                    ).default_open(glyphs.len() < 30).show(ui, |ui| {
                        for (id, name, ch) in &glyphs {
                            let sel = state.document.selection.contains(id);
                            let c = ch.unwrap_or('?');
                            let label = egui::RichText::new(format!("   @ {} '{}'", name, c))
                                .color(if sel { egui::Color32::WHITE } else { egui::Color32::from_rgb(170, 220, 170) });
                            let resp = ui.selectable_label(sel, label);
                            if resp.clicked() { clicked_id = Some(*id); }
                            resp.context_menu(|ui| {
                                if ui.button("Duplicate").clicked() { ctx_action = Some((*id, "dup")); ui.close_menu(); }
                                if ui.button("Delete").clicked() { ctx_action = Some((*id, "del")); ui.close_menu(); }
                                if ui.button("Focus").clicked() { ctx_action = Some((*id, "focus")); ui.close_menu(); }
                            });
                        }
                    });
                }

                if let Some(id) = clicked_id {
                    state.document.selection = vec![id];
                }
                if let Some((id, action)) = ctx_action {
                    match action {
                        "dup" => { state.push_undo("Duplicate"); if let Some(nid) = state.document.duplicate_node(id) { state.document.selection = vec![nid]; } state.needs_rebuild = true; }
                        "del" => { state.push_undo("Delete"); state.document.remove_node(id); state.document.selection.retain(|s| *s != id); state.needs_rebuild = true; }
                        "focus" => { if let Some(n) = state.document.get_node(id) { state.cam_x = n.position.x; state.cam_y = n.position.y; } }
                        _ => {}
                    }
                }
            });
        });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Inspector — context-sensitive per node type
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn inspector_panel(ctx: &egui::Context, state: &mut EditorState) {
    egui::SidePanel::right("inspector")
        .default_width(260.0)
        .min_width(200.0)
        .show(ctx, |ui| {
        let title_rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(
            egui::Rect::from_min_size(title_rect.min, egui::vec2(title_rect.width(), 28.0)),
            0.0, egui::Color32::from_rgb(30, 33, 42),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("INSPECTOR").size(11.0).strong()
                .color(egui::Color32::from_rgb(160, 170, 190)));
        });
        ui.add_space(4.0);
        ui.separator();

        if let Some(&id) = state.document.selection.first() {
            if let Some(node) = state.document.get_node_mut(id) {
                let mut changed = false;

                ui.add_space(6.0);
                let kind_color = match node.kind {
                    NodeKind::Entity => egui::Color32::from_rgb(180, 120, 255),
                    NodeKind::Field  => egui::Color32::from_rgb(255, 180, 80),
                    NodeKind::Glyph  => egui::Color32::from_rgb(150, 210, 150),
                    _                => egui::Color32::GRAY,
                };
                let kind_label = match node.kind {
                    NodeKind::Entity => "# Entity",
                    NodeKind::Field  => "~ Field",
                    NodeKind::Glyph  => "@ Glyph",
                    _                => "? Node",
                };
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.colored_label(kind_color, kind_label);
                    ui.label(egui::RichText::new(format!("  ID:{}", node.id)).size(10.0)
                        .color(egui::Color32::from_rgb(100, 105, 120)));
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Name").size(11.0).color(egui::Color32::from_rgb(140, 150, 170)));
                    ui.add(egui::TextEdit::singleline(&mut node.name).desired_width(f32::INFINITY));
                });
                ui.add_space(6.0);

                egui::CollapsingHeader::new(egui::RichText::new("  Transform").size(12.0))
                    .default_open(true).show(ui, |ui| {
                    ui.add_space(2.0);
                    changed |= ui.add(egui::Slider::new(&mut node.position.x, -30.0..=30.0).text("X")).changed();
                    changed |= ui.add(egui::Slider::new(&mut node.position.y, -30.0..=30.0).text("Y")).changed();
                    changed |= ui.add(egui::Slider::new(&mut node.rotation, 0.0..=360.0).text("Rot")).changed();
                    changed |= ui.add(egui::Slider::new(&mut node.scale, 0.1..=5.0).text("Scale")).changed();
                });

                egui::CollapsingHeader::new(egui::RichText::new("  Visual").size(12.0))
                    .default_open(true).show(ui, |ui| {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Color").size(11.0).color(egui::Color32::from_rgb(140, 150, 170)));
                        let mut rgba = egui::Rgba::from_rgba_unmultiplied(node.color.x, node.color.y, node.color.z, node.color.w);
                        if egui::color_picker::color_edit_button_rgba(ui, &mut rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
                            node.color.x = rgba.r(); node.color.y = rgba.g(); node.color.z = rgba.b(); node.color.w = rgba.a();
                            changed = true;
                        }
                    });
                    changed |= ui.add(egui::Slider::new(&mut node.emission, 0.0..=5.0).text("Emission")).changed();
                    changed |= ui.add(egui::Slider::new(&mut node.glow_radius, 0.0..=5.0).text("Glow Radius")).changed();
                    if let Some(ref mut ch) = node.character {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("Char: '{}'", ch)).size(11.0));
                            for &c in &['@', '#', '*', '+', 'o', 'x', 'X', 'O'] {
                                if ui.small_button(&c.to_string()).clicked() { *ch = c; changed = true; }
                            }
                        });
                    }
                    ui.add_space(4.0);
                    if ui.add(egui::Button::new(egui::RichText::new("  Apply Changes  ").size(12.0))
                        .fill(egui::Color32::from_rgb(55, 110, 170))
                    ).clicked() {
                        changed = true;
                    }
                });

                if node.kind == NodeKind::Field {
                    egui::CollapsingHeader::new(egui::RichText::new("  Force Field").size(12.0))
                        .default_open(true).show(ui, |ui| {
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
                        for (k, v) in &node.properties {
                            ui.horizontal(|ui| { ui.label(format!("{}: {}", k, v)); });
                        }
                    });
                }

                if node.kind == NodeKind::Entity {
                    egui::CollapsingHeader::new(egui::RichText::new("  Entity").size(12.0))
                        .default_open(true).show(ui, |ui| {
                        ui.label("Formation: Ring (12 glyphs)");
                        ui.label("HP: 100 / 100");
                        ui.label("Cohesion: 0.7");
                        ui.label("Pulse Rate: 0.5 Hz");
                    });
                }

                egui::CollapsingHeader::new(egui::RichText::new("  Tags").size(12.0))
                    .default_open(false).show(ui, |ui| {
                    for tag in &node.tags { ui.label(format!("  {}", tag)); }
                    if node.tags.is_empty() {
                        ui.label(egui::RichText::new("  (none)").color(egui::Color32::from_rgb(100, 105, 120)));
                    }
                });

                if changed { state.needs_rebuild = true; }
            }
        } else {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("No Selection").color(egui::Color32::from_rgb(100, 105, 120)));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("Tool: {:?}", state.tool)).size(11.0).color(egui::Color32::from_rgb(80, 85, 100)));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Click viewport to select").size(11.0).color(egui::Color32::from_rgb(80, 85, 100)));
                ui.label(egui::RichText::new("Shift+click: multi-select").size(11.0).color(egui::Color32::from_rgb(80, 85, 100)));
            });
        }
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Top toolbar (tools + scene info + FPS) — replaces the old bottom toolbar
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn toolbar_panel(ctx: &egui::Context, state: &mut EditorState, _engine: &mut ProofEngine) {
    const ACCENT: egui::Color32 = egui::Color32::from_rgb(70, 130, 200);
    const TOOLBAR_BG: egui::Color32 = egui::Color32::from_rgb(24, 26, 32);

    egui::TopBottomPanel::top("toolbar")
        .exact_height(34.0)
        .show(ctx, |ui| {
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, TOOLBAR_BG);

        ui.horizontal_centered(|ui| {
            ui.add_space(6.0);

            // Tool buttons
            let tools = [
                (ToolKind::Select, "V  Select"),
                (ToolKind::Move,   "G  Move"),
                (ToolKind::Place,  "P  Place"),
                (ToolKind::Field,  "F  Field"),
                (ToolKind::Entity, "E  Entity"),
                (ToolKind::Particle, "X  Burst"),
            ];
            for (kind, label) in &tools {
                let selected = state.tool == *kind;
                let btn = egui::Button::new(egui::RichText::new(*label).size(11.5))
                    .fill(if selected { ACCENT } else { egui::Color32::from_rgb(38, 40, 50) })
                    .stroke(egui::Stroke::new(1.0, if selected { egui::Color32::from_rgb(120, 170, 230) } else { egui::Color32::from_rgb(55, 58, 70) }));
                if ui.add(btn).clicked() { state.tool = *kind; }
                ui.add_space(2.0);
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            // Palettes
            ui.label(egui::RichText::new("Chars:").size(11.0).color(egui::Color32::from_rgb(140, 150, 170)));
            let cn: Vec<&str> = CHAR_PALETTES.iter().map(|(n, _)| *n).collect();
            egui::ComboBox::from_id_salt("ch").selected_text(cn[state.char_palette_idx])
                .width(90.0).show_ui(ui, |ui| {
                for (i, n) in cn.iter().enumerate() { ui.selectable_value(&mut state.char_palette_idx, i, *n); }
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Colors:").size(11.0).color(egui::Color32::from_rgb(140, 150, 170)));
            let ccn: Vec<&str> = COLOR_PALETTES.iter().map(|(n, _)| *n).collect();
            egui::ComboBox::from_id_salt("co").selected_text(ccn[state.color_palette_idx])
                .width(90.0).show_ui(ui, |ui| {
                for (i, n) in ccn.iter().enumerate() { ui.selectable_value(&mut state.color_palette_idx, i, *n); }
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Field:").size(11.0).color(egui::Color32::from_rgb(140, 150, 170)));
            let fn_: Vec<&str> = FieldType::all().iter().map(|f| f.label()).collect();
            egui::ComboBox::from_id_salt("fl").selected_text(fn_[state.field_type_idx])
                .width(100.0).show_ui(ui, |ui| {
                for (i, n) in fn_.iter().enumerate() { ui.selectable_value(&mut state.field_type_idx, i, *n); }
            });
            ui.add_space(6.0);
            ui.add(egui::Slider::new(&mut state.emission, 0.0..=5.0).text("Em").max_decimals(1));
            ui.add_space(4.0);
            ui.add(egui::Slider::new(&mut state.glow_radius, 0.0..=5.0).text("Glow").max_decimals(1));

            // Right side: scene name + stats + FPS
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // FPS
                let fps = state.fps;
                let fps_color = if fps >= 55.0 { egui::Color32::from_rgb(80, 220, 100) }
                    else if fps >= 30.0 { egui::Color32::from_rgb(255, 210, 60) }
                    else { egui::Color32::from_rgb(255, 80, 80) };
                ui.label(egui::RichText::new(format!("{:.0} fps", fps)).color(fps_color).size(11.5));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(format!("({:.1}, {:.1})", state.cam_x, state.cam_y)).size(10.5).color(egui::Color32::from_rgb(120, 130, 150)));
                ui.separator();
                // Scene info
                let dirty = if !state.undo_stack.is_empty() { " *" } else { "" };
                ui.label(egui::RichText::new(format!("scene{} — {} nodes", dirty, state.document.node_count())).size(11.0).color(egui::Color32::from_rgb(180, 185, 200)));
            });
        });
    });

    // Status bar at bottom
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(26.0)
        .show(ctx, |ui| {
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 22, 28));

        ui.horizontal_centered(|ui| {
            ui.add_space(8.0);
            // Left: timed status message
            if state.status_timer > 0.0 {
                let alpha = (state.status_timer * 85.0).min(255.0) as u8;
                ui.label(egui::RichText::new(&state.status_msg).size(11.0)
                    .color(egui::Color32::from_rgba_unmultiplied(100, 220, 100, alpha)));
            } else {
                ui.label(egui::RichText::new("Ready").size(11.0).color(egui::Color32::from_rgb(70, 80, 95)));
            }

            // Right: undo count + node breakdown
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!(
                    "Nodes: {}  |  Glyphs: {}  |  Fields: {}  |  Entities: {}",
                    state.document.node_count(),
                    state.document.glyph_count(),
                    state.document.field_count(),
                    state.document.nodes().filter(|n| n.kind == NodeKind::Entity).count(),
                )).size(10.5).color(egui::Color32::from_rgb(120, 130, 150)));
                ui.separator();
                ui.label(egui::RichText::new(format!("Undo: {}  Redo: {}", state.undo_stack.len(), state.redo_stack.len()))
                    .size(10.5).color(egui::Color32::from_rgb(100, 110, 130)));
            });
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
    egui::Window::new("[W] World Editor").open(&mut open).default_width(320.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[AI] AI Behavior").open(&mut open).default_width(300.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[PH] Physics").open(&mut open).default_width(300.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[RG] Render Graph").open(&mut open).default_width(320.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[DL] Dialogue Editor").open(&mut open).default_width(320.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[QT] Quest Editor").open(&mut open).default_width(320.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[SP] Spline Editor").open(&mut open).default_width(280.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[CN] Cinematic Editor").open(&mut open).default_width(340.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[IN] Inventory").open(&mut open).default_width(360.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[AB] Ability Editor").open(&mut open).default_width(320.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[LS] Level Streaming").open(&mut open).default_width(320.0).resizable(true).collapsible(true).show(ctx, |ui| {
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
    egui::Window::new("[AU] Audio Mixer").open(&mut open).default_width(300.0).resizable(true).collapsible(true).show(ctx, |ui| {
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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 3D Particle Modeler panel
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub fn modeling_panel(ctx: &egui::Context, state: &mut EditorState, engine: &mut ProofEngine) {
    if !state.show_modeling { return; }
    let mut open = state.show_modeling;
    egui::Window::new("[3D] Particle Modeler")
        .open(&mut open)
        .default_width(320.0)
        .resizable(true)
        .show(ctx, |ui| {
            // ── Section 1: 3D Viewport Controls ──────────────────────────────
            egui::CollapsingHeader::new("3D Viewport Controls").default_open(true).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Azimuth");
                    ui.add(egui::Slider::new(&mut state.model_cam_azimuth, -180.0..=180.0).suffix("°"));
                });
                ui.horizontal(|ui| {
                    ui.label("Elevation");
                    ui.add(egui::Slider::new(&mut state.model_cam_elevation, -90.0..=90.0).suffix("°"));
                });
                ui.horizontal(|ui| {
                    ui.label("Distance");
                    ui.add(egui::Slider::new(&mut state.model_cam_distance, 1.0..=50.0));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.model_3d_mode, "3D Mode");
                    ui.checkbox(&mut state.model_wireframe, "Wireframe");
                    ui.checkbox(&mut state.model_show_normals, "Normals");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.model_show_grid, "Grid");
                    ui.checkbox(&mut state.model_snap_grid, "Snap");
                    ui.add(egui::DragValue::new(&mut state.model_snap_size).speed(0.05).range(0.05..=5.0).prefix("size:"));
                });
                ui.horizontal(|ui| {
                    if ui.button("Front").clicked() {
                        state.model_cam_azimuth = 0.0; state.model_cam_elevation = 0.0;
                    }
                    if ui.button("Top").clicked() {
                        state.model_cam_azimuth = 0.0; state.model_cam_elevation = 89.9;
                    }
                    if ui.button("Side").clicked() {
                        state.model_cam_azimuth = 90.0; state.model_cam_elevation = 0.0;
                    }
                    if ui.button("Iso").clicked() {
                        state.model_cam_azimuth = 45.0; state.model_cam_elevation = 35.0;
                    }
                });
                ui.separator();
                ui.label(egui::RichText::new(format!("Particles: {}", state.model_particle_count))
                    .color(egui::Color32::from_rgb(80, 220, 100)));
            });

            // ── Section 2: Brush ─────────────────────────────────────────────
            egui::CollapsingHeader::new("Brush").default_open(true).show(ui, |ui| {
                let brush_options = ["Add", "Remove", "Smooth", "Color", "Inflate", "Pinch", "Flatten", "Clone"];
                egui::ComboBox::from_label("Brush Type")
                    .selected_text(state.model_brush.clone())
                    .show_ui(ui, |ui| {
                        for opt in &brush_options {
                            ui.selectable_value(&mut state.model_brush, opt.to_string(), *opt);
                        }
                    });
                ui.add(egui::Slider::new(&mut state.model_brush_radius, 0.1..=5.0).text("Radius"));
                ui.add(egui::Slider::new(&mut state.model_brush_strength, 0.0..=1.0).text("Strength"));
                if state.model_brush == "Add" {
                    ui.add(egui::Slider::new(&mut state.model_brush_density, 1.0..=200.0).text("Density"));
                }
                ui.horizontal(|ui| {
                    ui.label("Char:");
                    let mut ch_str = state.model_active_char.to_string();
                    let resp = ui.add(egui::TextEdit::singleline(&mut ch_str).desired_width(24.0));
                    if resp.changed() {
                        if let Some(c) = ch_str.chars().next() {
                            state.model_active_char = c;
                        }
                    }
                });
                let sym_options = ["None", "X", "Y", "Z", "XYZ"];
                egui::ComboBox::from_label("Symmetry")
                    .selected_text(state.model_symmetry.clone())
                    .show_ui(ui, |ui| {
                        for opt in &sym_options {
                            ui.selectable_value(&mut state.model_symmetry, opt.to_string(), *opt);
                        }
                    });
            });

            // ── Section 3: Primitives ────────────────────────────────────────
            egui::CollapsingHeader::new("Primitives").default_open(false).show(ui, |ui| {
                let prim_options = ["Sphere", "Cube", "Cylinder", "Cone", "Torus", "Plane", "Metaballs"];
                egui::ComboBox::from_label("Primitive")
                    .selected_text(state.model_selected_primitive.clone())
                    .show_ui(ui, |ui| {
                        for opt in &prim_options {
                            ui.selectable_value(&mut state.model_selected_primitive, opt.to_string(), *opt);
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("Size:");
                    ui.add(egui::DragValue::new(&mut state.model_prim_size).speed(0.1).range(0.1..=20.0));
                    ui.label("Segments:");
                    ui.add(egui::DragValue::new(&mut state.model_prim_segments).speed(1).range(3..=64));
                });
                ui.horizontal(|ui| {
                    ui.label("Pivot X:");
                    ui.add(egui::DragValue::new(&mut state.model_pivot_x).speed(0.1));
                    ui.label("Y:");
                    ui.add(egui::DragValue::new(&mut state.model_pivot_y).speed(0.1));
                    ui.label("Z:");
                    ui.add(egui::DragValue::new(&mut state.model_pivot_z).speed(0.1));
                });
                if ui.button("Spawn Primitive").clicked() {
                    let pivot = Vec3::new(state.model_pivot_x, state.model_pivot_y, state.model_pivot_z);
                    let s = state.model_prim_size;
                    let segs = state.model_prim_segments;
                    let prim = state.model_selected_primitive.clone();
                    match prim.as_str() {
                        "Sphere" => {
                            let n_particles = (segs * segs) as usize;
                            let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
                            for i in 0..n_particles {
                                let y = 1.0 - (i as f32 / (n_particles - 1) as f32) * 2.0;
                                let radius = (1.0 - y * y).sqrt();
                                let theta = golden * i as f32;
                                let x = theta.cos() * radius;
                                let z = theta.sin() * radius;
                                let pos = Vec3::new(x, y, z) * s + pivot;
                                let chars = ['@', '#', '*', '+', 'o', 'x', '.', ':', '~'];
                                let ch = chars[i % chars.len()];
                                engine.spawn_glyph(Glyph {
                                    character: ch,
                                    position: pos,
                                    color: Vec4::new(0.4 + y * 0.3, 0.7, 0.9 - y * 0.2, 0.9),
                                    emission: 0.8,
                                    glow_radius: 0.3,
                                    mass: 0.0,
                                    layer: RenderLayer::Entity,
                                    ..Default::default()
                                });
                            }
                            state.model_particle_count += n_particles;
                            state.model_history.push(format!("Spawn {}", state.model_selected_primitive));
                        }
                        "Cube" => {
                            let half = s * 0.5;
                            let faces = [
                                (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
                                (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)),
                                (Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
                                (Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0)),
                                (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
                                (Vec3::new(0.0, 0.0, -1.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
                            ];
                            let n = segs as usize;
                            let chars = ['#', '@', '+', '*', 'o', '.'];
                            let mut count = 0usize;
                            for (fi, (normal, u_dir, v_dir)) in faces.iter().enumerate() {
                                for iu in 0..n {
                                    for iv in 0..n {
                                        let u = (iu as f32 / (n - 1) as f32) * 2.0 - 1.0;
                                        let v = (iv as f32 / (n - 1) as f32) * 2.0 - 1.0;
                                        let pos = (*normal + *u_dir * u + *v_dir * v) * half + pivot;
                                        let ch = chars[count % chars.len()];
                                        let t = (iu + iv) as f32 / (n * 2) as f32;
                                        engine.spawn_glyph(Glyph {
                                            character: ch,
                                            position: pos,
                                            color: Vec4::new(0.3 + t * 0.5, 0.5 + fi as f32 * 0.08, 0.8, 0.9),
                                            emission: 0.6,
                                            glow_radius: 0.2,
                                            mass: 0.0,
                                            layer: RenderLayer::Entity,
                                            ..Default::default()
                                        });
                                        count += 1;
                                    }
                                }
                            }
                            state.model_particle_count += count;
                            state.model_history.push(format!("Spawn {}", state.model_selected_primitive));
                        }
                        "Torus" => {
                            let major_r = s;
                            let minor_r = s * 0.35;
                            let nu = segs as usize;
                            let nv = (segs / 2).max(4) as usize;
                            let chars = ['o', 'O', '@', '#', '~', '*'];
                            let mut count = 0usize;
                            for iu in 0..nu {
                                for iv in 0..nv {
                                    let u = (iu as f32 / nu as f32) * std::f32::consts::TAU;
                                    let v = (iv as f32 / nv as f32) * std::f32::consts::TAU;
                                    let x = (major_r + minor_r * v.cos()) * u.cos();
                                    let y = (major_r + minor_r * v.cos()) * u.sin();
                                    let z = minor_r * v.sin();
                                    let pos = Vec3::new(x, y, z) + pivot;
                                    let ch = chars[count % chars.len()];
                                    let t = iu as f32 / nu as f32;
                                    engine.spawn_glyph(Glyph {
                                        character: ch,
                                        position: pos,
                                        color: Vec4::new(0.8 - t * 0.3, 0.4 + t * 0.4, 0.9, 0.9),
                                        emission: 0.7,
                                        glow_radius: 0.25,
                                        mass: 0.0,
                                        layer: RenderLayer::Entity,
                                        ..Default::default()
                                    });
                                    count += 1;
                                }
                            }
                            state.model_particle_count += count;
                            state.model_history.push(format!("Spawn {}", state.model_selected_primitive));
                        }
                        "Cylinder" => {
                            let n = segs as usize;
                            let chars = ['|', '/', '-', '\\', '@', '#'];
                            let mut count = 0usize;
                            for i in 0..n {
                                let theta = (i as f32 / n as f32) * std::f32::consts::TAU;
                                let x = theta.cos() * s;
                                let z = theta.sin() * s;
                                for j in 0..n {
                                    let y = (j as f32 / (n - 1) as f32) * 2.0 - 1.0;
                                    let pos = Vec3::new(x, y * s, z) + pivot;
                                    let ch = chars[count % chars.len()];
                                    engine.spawn_glyph(Glyph {
                                        character: ch,
                                        position: pos,
                                        color: Vec4::new(0.5, 0.8 - (y * 0.3).abs(), 0.6, 0.9),
                                        emission: 0.6,
                                        glow_radius: 0.2,
                                        mass: 0.0,
                                        layer: RenderLayer::Entity,
                                        ..Default::default()
                                    });
                                    count += 1;
                                }
                            }
                            state.model_particle_count += count;
                            state.model_history.push(format!("Spawn {}", state.model_selected_primitive));
                        }
                        "Cone" => {
                            let n = segs as usize;
                            let chars = ['^', '*', '+', '#', '@', '.'];
                            let mut count = 0usize;
                            for i in 0..n {
                                let theta = (i as f32 / n as f32) * std::f32::consts::TAU;
                                for j in 0..n {
                                    let t = j as f32 / (n - 1) as f32;
                                    let r = t * s;
                                    let y = (1.0 - t) * s;
                                    let x = theta.cos() * r;
                                    let z = theta.sin() * r;
                                    let pos = Vec3::new(x, y, z) + pivot;
                                    let ch = chars[count % chars.len()];
                                    engine.spawn_glyph(Glyph {
                                        character: ch,
                                        position: pos,
                                        color: Vec4::new(0.9 - t * 0.4, 0.5, 0.3 + t * 0.5, 0.9),
                                        emission: 0.6,
                                        glow_radius: 0.2,
                                        mass: 0.0,
                                        layer: RenderLayer::Entity,
                                        ..Default::default()
                                    });
                                    count += 1;
                                }
                            }
                            state.model_particle_count += count;
                            state.model_history.push(format!("Spawn {}", state.model_selected_primitive));
                        }
                        "Plane" => {
                            let n = segs as usize;
                            let chars = ['.', ',', '\'', '`', '-', '_'];
                            let mut count = 0usize;
                            for ix in 0..n {
                                for iz in 0..n {
                                    let x = (ix as f32 / (n - 1) as f32) * 2.0 - 1.0;
                                    let z = (iz as f32 / (n - 1) as f32) * 2.0 - 1.0;
                                    let pos = Vec3::new(x * s, 0.0, z * s) + pivot;
                                    let ch = chars[count % chars.len()];
                                    engine.spawn_glyph(Glyph {
                                        character: ch,
                                        position: pos,
                                        color: Vec4::new(0.4, 0.7, 0.4, 0.8),
                                        emission: 0.3,
                                        glow_radius: 0.1,
                                        mass: 0.0,
                                        layer: RenderLayer::Entity,
                                        ..Default::default()
                                    });
                                    count += 1;
                                }
                            }
                            state.model_particle_count += count;
                            state.model_history.push(format!("Spawn {}", state.model_selected_primitive));
                        }
                        "Metaballs" => {
                            let n_balls = segs.min(8) as usize;
                            let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
                            let total = n_balls * 20;
                            let chars = ['*', 'o', '@', '#', '+', '.'];
                            for i in 0..total {
                                let y = 1.0 - (i as f32 / (total - 1) as f32) * 2.0;
                                let radius = (1.0 - y * y).sqrt();
                                let theta = golden * i as f32;
                                let x = theta.cos() * radius;
                                let z = theta.sin() * radius;
                                let blob_idx = i % n_balls;
                                let blob_off = Vec3::new(
                                    (blob_idx as f32 * 1.3).sin() * s * 0.5,
                                    (blob_idx as f32 * 0.9).cos() * s * 0.5,
                                    (blob_idx as f32 * 0.7).sin() * s * 0.3,
                                );
                                let pos = Vec3::new(x, y, z) * (s * 0.5) + blob_off + pivot;
                                let ch = chars[i % chars.len()];
                                engine.spawn_glyph(Glyph {
                                    character: ch,
                                    position: pos,
                                    color: Vec4::new(0.9, 0.4 + y * 0.3, 0.7, 0.9),
                                    emission: 0.9,
                                    glow_radius: 0.4,
                                    mass: 0.0,
                                    layer: RenderLayer::Entity,
                                    ..Default::default()
                                });
                            }
                            state.model_particle_count += total;
                            state.model_history.push(format!("Spawn {}", state.model_selected_primitive));
                        }
                        _ => {}
                    }
                }
            });

            // ── Section 4: Layers ────────────────────────────────────────────
            egui::CollapsingHeader::new("Layers").default_open(false).show(ui, |ui| {
                let mut remove_idx: Option<usize> = None;
                egui::ScrollArea::vertical().max_height(120.0).id_salt("model_layers").show(ui, |ui| {
                    let active = state.model_layer;
                    for (idx, (name, visible)) in state.model_layers.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.checkbox(visible, "");
                            let is_active = idx == active;
                            let label = egui::RichText::new(name.as_str())
                                .color(if is_active { egui::Color32::from_rgb(100, 200, 255) } else { egui::Color32::LIGHT_GRAY });
                            ui.label(label);
                            if ui.small_button("X").clicked() {
                                remove_idx = Some(idx);
                            }
                        });
                    }
                });
                if let Some(idx) = remove_idx {
                    if state.model_layers.len() > 1 {
                        state.model_layers.remove(idx);
                        if state.model_layer >= state.model_layers.len() {
                            state.model_layer = state.model_layers.len() - 1;
                        }
                    }
                }
                if ui.button("Add Layer").clicked() {
                    let n = state.model_layers.len();
                    state.model_layers.push((format!("Layer {}", n), true));
                    state.model_layer = n;
                }
            });

            // ── Section 5: History ───────────────────────────────────────────
            egui::CollapsingHeader::new("History").default_open(false).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(100.0).id_salt("model_history").show(ui, |ui| {
                    let start = state.model_history.len().saturating_sub(20);
                    for entry in &state.model_history[start..] {
                        ui.label(entry);
                    }
                });
            });
        });
    state.show_modeling = open;
}

pub fn behavior_tree_panel(ctx: &egui::Context, state: &mut EditorState) {
    crate::behavior_tree::show_panel(ctx, &mut state.behavior_tree_editor, &mut state.show_behavior_tree);
}

pub fn dialogue_graph_panel(ctx: &egui::Context, state: &mut EditorState) {
    crate::dialogue_graph::show_panel(ctx, &mut state.dialogue_editor, &mut state.show_dialogue_graph);
}

pub fn particle_editor_panel(ctx: &egui::Context, state: &mut EditorState, dt: f32) {
    crate::particle_editor::ParticleEditor::show_panel(ctx, &mut state.particle_editor, dt, &mut state.show_particle_editor);
}

pub fn material_system_panel(ctx: &egui::Context, state: &mut EditorState) {
    crate::material_system::MaterialEditor::show_panel(ctx, &mut state.material_editor, &mut state.show_material_system);
}

pub fn spline_editor_panel(ctx: &egui::Context, state: &mut EditorState, dt: f32) {
    crate::spline_editor::SplineEditor::show_panel(ctx, &mut state.spline_editor, dt, &mut state.show_spline_editor);
}

pub fn quest_system_panel(ctx: &egui::Context, state: &mut EditorState) {
    crate::quest_system::QuestEditor::show_panel(ctx, &mut state.quest_editor, &mut state.show_quest_system);
}

pub fn audio_mixer_full_panel(ctx: &egui::Context, state: &mut EditorState, dt: f32) {
    crate::audio_mixer::AudioMixerEditor::show_panel(ctx, &mut state.audio_mixer_editor, dt, &mut state.show_audio_mixer_full);
}

pub fn physics_editor_panel(ctx: &egui::Context, state: &mut EditorState, dt: f32) {
    crate::physics_editor::PhysicsEditor::show_panel(ctx, &mut state.physics_editor, dt, &mut state.show_physics_editor);
}

pub fn inventory_system_panel(ctx: &egui::Context, state: &mut EditorState) {
    crate::inventory_system::InventoryEditor::show_panel(ctx, &mut state.inventory_editor, &mut state.show_inventory_system);
}

pub fn world_gen_panel(ctx: &egui::Context, state: &mut EditorState, dt: f32) {
    crate::world_gen::WorldGenEditor::show_panel(ctx, &mut state.world_gen_editor, dt, &mut state.show_world_gen);
}

// ════════════════════════════════════════════════════════════════════════════
// EXPANDED HIERARCHY PANEL — drag reparent, multi-select, rename, groups
// ════════════════════════════════════════════════════════════════════════════

/// Extended hierarchy state stored separately so the static panel can access it.
pub struct HierarchyState {
    pub search: String,
    pub filter: Option<NodeKind>,
    pub rename_id: Option<u32>,
    pub rename_buf: String,
    pub collapsed_groups: std::collections::HashSet<u32>,
    pub multi_select: bool,
    pub drag_source: Option<u32>,
    pub drag_over: Option<u32>,
    pub pending_group_ids: Vec<u32>,
    pub show_group_name_input: bool,
    pub group_name_buf: String,
    pub move_up_id: Option<u32>,
    pub move_down_id: Option<u32>,
}

impl HierarchyState {
    pub fn new() -> Self {
        HierarchyState {
            search: String::new(),
            filter: None,
            rename_id: None,
            rename_buf: String::new(),
            collapsed_groups: std::collections::HashSet::new(),
            multi_select: false,
            drag_source: None,
            drag_over: None,
            pending_group_ids: Vec::new(),
            show_group_name_input: false,
            group_name_buf: String::new(),
            move_up_id: None,
            move_down_id: None,
        }
    }
}

/// A full-featured hierarchy panel using egui, supplementing the existing one.
pub fn hierarchy_panel_extended(ctx: &egui::Context, state: &mut EditorState, hs: &mut HierarchyState) {
    const ACCENT: egui::Color32 = egui::Color32::from_rgb(70, 130, 200);
    const PANEL_HEADER: egui::Color32 = egui::Color32::from_rgb(30, 33, 42);

    egui::SidePanel::left("hierarchy_ext")
        .default_width(220.0)
        .min_width(160.0)
        .show(ctx, |ui| {
            // Title bar
            ui.painter().rect_filled(
                egui::Rect::from_min_size(ui.available_rect_before_wrap().min, egui::vec2(ui.available_width(), 28.0)),
                0.0, PANEL_HEADER,
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("HIERARCHY").size(11.0).strong()
                    .color(egui::Color32::from_rgb(160, 170, 190)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    let sel_cnt = state.document.selection.len();
                    if sel_cnt > 0 {
                        ui.label(egui::RichText::new(format!("{} sel", sel_cnt)).size(10.0)
                            .color(egui::Color32::from_rgb(100, 180, 255)));
                    }
                });
            });
            ui.add_space(4.0);
            ui.separator();

            // Search bar
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut hs.search)
                    .hint_text("Search nodes...")
                    .desired_width(f32::INFINITY));
                if !hs.search.is_empty() {
                    if ui.small_button("x").clicked() { hs.search.clear(); }
                }
            });
            ui.add_space(4.0);

            // Node type filter tabs
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let tabs = [
                    (None, "All"),
                    (Some(NodeKind::Glyph),  "◆"),
                    (Some(NodeKind::Field),  "~"),
                    (Some(NodeKind::Entity), "@"),
                    (Some(NodeKind::Group),  "□"),
                    (Some(NodeKind::Camera), "⊙"),
                ];
                for (kind, label) in &tabs {
                    let selected = hs.filter == *kind;
                    let btn = egui::Button::new(egui::RichText::new(*label).size(11.0))
                        .fill(if selected { ACCENT } else { egui::Color32::from_rgb(35, 37, 46) })
                        .stroke(egui::Stroke::new(1.0, if selected { ACCENT } else { egui::Color32::from_rgb(55,58,70) }));
                    if ui.add(btn).on_hover_text(match kind {
                        None => "All nodes",
                        Some(NodeKind::Glyph)  => "Glyphs",
                        Some(NodeKind::Field)  => "Force Fields",
                        Some(NodeKind::Entity) => "Entities",
                        Some(NodeKind::Group)  => "Groups",
                        Some(NodeKind::Camera) => "Cameras",
                    }).clicked() { hs.filter = *kind; }
                }
            });

            // Multi-select toggle + group button
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.toggle_value(&mut hs.multi_select, "Multi-Select");
                let sel_cnt = state.document.selection.len();
                if sel_cnt > 1 {
                    if ui.small_button("Group Selected").clicked() {
                        hs.pending_group_ids = state.document.selection.clone();
                        hs.show_group_name_input = true;
                    }
                }
            });

            // Group name input
            if hs.show_group_name_input {
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.add(egui::TextEdit::singleline(&mut hs.group_name_buf)
                        .hint_text("Group name...")
                        .desired_width(120.0));
                    if ui.small_button("OK").clicked() {
                        // In a full implementation, create a group node and reparent the selected nodes
                        let name = if hs.group_name_buf.is_empty() {
                            format!("Group_{}", state.document.node_count())
                        } else {
                            hs.group_name_buf.clone()
                        };
                        state.set_status(&format!("Group '{}' created", name));
                        state.log(&format!("Grouped {} nodes as '{}'", hs.pending_group_ids.len(), name), egui::Color32::from_rgb(100, 220, 100));
                        hs.show_group_name_input = false;
                        hs.group_name_buf.clear();
                        hs.pending_group_ids.clear();
                    }
                    if ui.small_button("Cancel").clicked() {
                        hs.show_group_name_input = false;
                        hs.pending_group_ids.clear();
                    }
                });
            }

            ui.add_space(4.0);
            ui.separator();

            if state.document.node_count() == 0 {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("Empty scene").color(egui::Color32::from_rgb(100,105,120)));
                    ui.label(egui::RichText::new("Click viewport to place").size(11.0).color(egui::Color32::from_rgb(80,85,100)));
                });
                return;
            }

            // Node list with icons and reparent via up/down buttons
            egui::ScrollArea::vertical().show(ui, |ui| {
                let search = hs.search.to_lowercase();

                let nodes: Vec<(u32, String, NodeKind, Option<FieldType>, Option<char>)> = state.document.nodes()
                    .filter(|n| hs.filter.map_or(true, |f| f == n.kind))
                    .filter(|n| search.is_empty() || n.name.to_lowercase().contains(&search))
                    .map(|n| (n.id, n.name.clone(), n.kind, n.field_type, n.character))
                    .collect();

                let total = nodes.len();
                let mut clicked_id: Option<u32> = None;
                let mut ctx_action: Option<(u32, &'static str)> = None;
                let mut rename_commit: Option<(u32, String)> = None;

                for (idx, (id, name, kind, ft, ch)) in nodes.iter().enumerate() {
                    let sel = state.document.selection.contains(id);
                    let is_renaming = hs.rename_id == Some(*id);

                    // Node type icon and color
                    let (icon, node_color) = node_icon_color(*kind, *ft);

                    // Indentation indicator (depth 0 for now, can be extended for parent-child)
                    let row_bg = if sel { egui::Color32::from_rgb(50,80,130) } else { egui::Color32::TRANSPARENT };

                    let row_resp = ui.horizontal(|ui| {
                        ui.add_space(6.0);

                        // ── Up/Down reparent buttons ─────────────────────
                        ui.vertical(|ui| {
                            ui.add_space(2.0);
                            if idx > 0 && ui.small_button("^").on_hover_text("Move up").clicked() {
                                hs.move_up_id = Some(*id);
                            }
                            if idx + 1 < total && ui.small_button("v").on_hover_text("Move down").clicked() {
                                hs.move_down_id = Some(*id);
                            }
                        });

                        // ── Icon ─────────────────────────────────────────
                        ui.colored_label(node_color, icon);

                        // ── Name (or rename input) ────────────────────────
                        if is_renaming {
                            let resp = ui.add(egui::TextEdit::singleline(&mut hs.rename_buf)
                                .desired_width(120.0));
                            if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                rename_commit = Some((*id, hs.rename_buf.clone()));
                            }
                        } else {
                            let txt = egui::RichText::new(name)
                                .color(if sel { egui::Color32::WHITE } else { egui::Color32::from_rgb(200,205,215) })
                                .size(11.5);
                            let resp = ui.selectable_label(sel, txt);

                            if resp.clicked() {
                                if hs.multi_select {
                                    clicked_id = Some(*id);
                                } else {
                                    clicked_id = Some(*id);
                                }
                            }

                            resp.context_menu(|ui| {
                                if ui.button("Rename").clicked() {
                                    hs.rename_id = Some(*id);
                                    hs.rename_buf = name.clone();
                                    ui.close_menu();
                                }
                                if ui.button("Duplicate").clicked() { ctx_action = Some((*id, "dup")); ui.close_menu(); }
                                if ui.button("Delete").clicked() { ctx_action = Some((*id, "del")); ui.close_menu(); }
                                ui.separator();
                                if ui.button("Focus Camera").clicked() { ctx_action = Some((*id, "focus")); ui.close_menu(); }
                                if ui.button("Select Children").clicked() { ctx_action = Some((*id, "select_children")); ui.close_menu(); }
                                if ui.button("Move to New Group").clicked() { ctx_action = Some((*id, "new_group")); ui.close_menu(); }
                            });
                        }

                        // ── Character hint for glyphs ────────────────────
                        if *kind == NodeKind::Glyph {
                            if let Some(c) = ch {
                                ui.label(egui::RichText::new(format!("'{}'", c)).size(10.0)
                                    .color(egui::Color32::from_rgb(140,160,140)));
                            }
                        }
                        // ── Field type hint ──────────────────────────────
                        if *kind == NodeKind::Field {
                            if let Some(ftype) = ft {
                                ui.label(egui::RichText::new(format!("[{}]", ftype.label())).size(9.5)
                                    .color(egui::Color32::from_rgb(180,140,80)));
                            }
                        }
                    });

                    // Row background tint for selection
                    if sel {
                        let rr = row_resp.response.rect;
                        ui.painter().rect_filled(rr, 0.0, egui::Color32::from_rgba_premultiplied(50,80,130,80));
                    }
                }

                // Apply rename commit
                if let Some((id, new_name)) = rename_commit {
                    if let Some(node) = state.document.get_node_mut(id) {
                        node.name = new_name;
                        state.needs_rebuild = true;
                    }
                    hs.rename_id = None;
                }

                // Apply click selection
                if let Some(id) = clicked_id {
                    if hs.multi_select {
                        if state.document.selection.contains(&id) {
                            state.document.selection.retain(|&s| s != id);
                        } else {
                            state.document.selection.push(id);
                        }
                    } else {
                        state.document.selection = vec![id];
                    }
                }

                // Apply context menu actions
                if let Some((id, action)) = ctx_action {
                    match action {
                        "dup" => {
                            state.push_undo("Duplicate");
                            if let Some(nid) = state.document.duplicate_node(id) {
                                state.document.selection = vec![nid];
                            }
                            state.needs_rebuild = true;
                        }
                        "del" => {
                            state.push_undo("Delete");
                            state.document.remove_node(id);
                            state.document.selection.retain(|s| *s != id);
                            state.needs_rebuild = true;
                        }
                        "focus" => {
                            if let Some(n) = state.document.get_node(id) {
                                state.cam_x = n.position.x;
                                state.cam_y = n.position.y;
                            }
                        }
                        "select_children" => {
                            // Placeholder: in full impl would walk child tree
                            state.document.selection = vec![id];
                        }
                        "new_group" => {
                            hs.pending_group_ids = vec![id];
                            hs.show_group_name_input = true;
                        }
                        _ => {}
                    }
                }
            });

            // Apply up/down moves (stub — full impl would reorder document node list)
            if let Some(_id) = hs.move_up_id.take() {
                state.set_status("Move up (WIP)");
            }
            if let Some(_id) = hs.move_down_id.take() {
                state.set_status("Move down (WIP)");
            }
        });
}

/// Returns (icon_str, color) for a node kind.
fn node_icon_color(kind: NodeKind, ft: Option<FieldType>) -> (&'static str, egui::Color32) {
    match kind {
        NodeKind::Glyph  => ("◆", egui::Color32::from_rgb(150, 210, 150)),
        NodeKind::Field  => ("~",  egui::Color32::from_rgb(255, 180, 80)),
        NodeKind::Entity => ("@",  egui::Color32::from_rgb(180, 120, 255)),
        NodeKind::Group  => ("□",  egui::Color32::from_rgb(160, 200, 255)),
        NodeKind::Camera => ("⊙",  egui::Color32::from_rgb(255, 230, 100)),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// EXPANDED INSPECTOR PANEL — tabbed Transform|Appearance|Physics|Tags|Advanced
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Transform,
    Appearance,
    Physics,
    Tags,
    Advanced,
}

impl InspectorTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Transform  => "Transform",
            Self::Appearance => "Appearance",
            Self::Physics    => "Physics",
            Self::Tags       => "Tags",
            Self::Advanced   => "Advanced",
        }
    }
    pub fn all() -> &'static [InspectorTab] {
        &[Self::Transform, Self::Appearance, Self::Physics, Self::Tags, Self::Advanced]
    }
}

pub struct InspectorState {
    pub active_tab: InspectorTab,
    pub pos_inc: f32,
    pub scale_linked: bool,
    pub tag_input: String,
    pub show_full_color: bool,
    pub color_edit: [f32; 4],
}

impl InspectorState {
    pub fn new() -> Self {
        InspectorState {
            active_tab: InspectorTab::Transform,
            pos_inc: 1.0,
            scale_linked: true,
            tag_input: String::new(),
            show_full_color: false,
            color_edit: [1.0; 4],
        }
    }
}

pub fn inspector_panel_extended(ctx: &egui::Context, state: &mut EditorState, ins: &mut InspectorState) {
    const ACCENT: egui::Color32 = egui::Color32::from_rgb(70, 130, 200);

    egui::SidePanel::right("inspector_ext")
        .default_width(280.0)
        .min_width(200.0)
        .show(ctx, |ui| {
            // Header
            ui.painter().rect_filled(
                egui::Rect::from_min_size(ui.available_rect_before_wrap().min, egui::vec2(ui.available_width(), 28.0)),
                0.0, egui::Color32::from_rgb(30, 33, 42),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("INSPECTOR").size(11.0).strong()
                    .color(egui::Color32::from_rgb(160, 170, 190)));
                let sel_cnt = state.document.selection.len();
                if sel_cnt > 1 {
                    ui.label(egui::RichText::new(format!("({})", sel_cnt)).size(10.0)
                        .color(egui::Color32::from_rgb(255,200,80)));
                }
            });
            ui.add_space(4.0);
            ui.separator();

            if state.document.selection.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("No Selection").color(egui::Color32::from_rgb(100,105,120)));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(format!("Tool: {:?}", state.tool)).size(11.0)
                        .color(egui::Color32::from_rgb(80,85,100)));
                    ui.label(egui::RichText::new("Click to select").size(11.0)
                        .color(egui::Color32::from_rgb(80,85,100)));
                    ui.label(egui::RichText::new("Ctrl+click: multi-select").size(11.0)
                        .color(egui::Color32::from_rgb(70,75,90)));
                });
                return;
            }

            // Multi-select banner
            let is_multi = state.document.selection.len() > 1;
            if is_multi {
                ui.horizontal(|ui| {
                    let cnt = state.document.selection.len();
                    ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, egui::Color32::from_rgb(50,45,20));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(format!("{} nodes selected — editing all", cnt))
                        .size(11.0).color(egui::Color32::from_rgb(255,210,60)));
                });
                ui.add_space(4.0);
            }

            let id = state.document.selection[0];
            if let Some(node) = state.document.get_node_mut(id) {
                // Node header
                let (icon, kind_color) = node_icon_color(node.kind, node.field_type);
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.colored_label(kind_color, icon);
                    ui.add(egui::TextEdit::singleline(&mut node.name)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Heading));
                });
                ui.label(egui::RichText::new(format!("ID: {}  Kind: {:?}", node.id, node.kind))
                    .size(9.5).color(egui::Color32::from_rgb(100,105,120)));
                ui.add_space(4.0);
                ui.separator();

                // Tab bar
                ui.horizontal(|ui| {
                    for tab in InspectorTab::all() {
                        let sel = ins.active_tab == *tab;
                        // Skip tabs not relevant for this node kind
                        let relevant = match tab {
                            InspectorTab::Appearance | InspectorTab::Advanced => true,
                            InspectorTab::Physics => node.kind != NodeKind::Group,
                            _ => true,
                        };
                        if !relevant { continue; }
                        let btn = egui::Button::new(egui::RichText::new(tab.label()).size(10.0))
                            .fill(if sel { ACCENT } else { egui::Color32::from_rgb(35,37,46) })
                            .stroke(egui::Stroke::new(1.0, if sel { ACCENT } else { egui::Color32::from_rgb(55,58,70) }));
                        if ui.add(btn).clicked() { ins.active_tab = *tab; }
                    }
                });
                ui.add_space(4.0);
                ui.separator();

                let mut changed = false;
                egui::ScrollArea::vertical().id_salt("inspector_scroll").show(ui, |ui| {
                    match ins.active_tab {
                        InspectorTab::Transform => {
                            changed |= show_transform_tab(ui, node, ins);
                        }
                        InspectorTab::Appearance => {
                            changed |= show_appearance_tab(ui, node, ins);
                        }
                        InspectorTab::Physics => {
                            changed |= show_physics_tab(ui, node);
                        }
                        InspectorTab::Tags => {
                            changed |= show_tags_tab(ui, node, ins);
                        }
                        InspectorTab::Advanced => {
                            changed |= show_advanced_tab(ui, node);
                        }
                    }
                });

                if changed { state.needs_rebuild = true; }
            }
        });
}

fn show_transform_tab(ui: &mut egui::Ui, node: &mut SceneNode, ins: &mut InspectorState) -> bool {
    let mut changed = false;
    ui.add_space(4.0);

    // Position with ± increment buttons
    ui.label(egui::RichText::new("Position").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    egui::Grid::new("pos_grid").num_columns(4).spacing(egui::Vec2::new(4.0, 3.0)).show(ui, |ui| {
        for (label, val) in [("X", &mut node.position.x), ("Y", &mut node.position.y), ("Z", &mut node.position.z)] {
            ui.label(egui::RichText::new(label).size(10.5).color(egui::Color32::from_rgb(180,130,130)));
            let r = ui.add(egui::DragValue::new(val).speed(0.05).max_decimals(3));
            if r.changed() { changed = true; }
            if ui.small_button("-").clicked() { *val -= ins.pos_inc; changed = true; }
            if ui.small_button("+").clicked() { *val += ins.pos_inc; changed = true; }
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Step:").size(10.0).color(egui::Color32::GRAY));
        ui.add(egui::DragValue::new(&mut ins.pos_inc).range(0.01..=100.0).speed(0.05));
        if ui.small_button("Reset").clicked() { node.position = glam::Vec3::ZERO; changed = true; }
    });
    ui.add_space(6.0);

    // Rotation
    ui.label(egui::RichText::new("Rotation").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    ui.horizontal(|ui| {
        if ui.add(egui::Slider::new(&mut node.rotation, 0.0..=360.0).suffix("°")).changed() { changed = true; }
        if ui.small_button("0°").clicked() { node.rotation = 0.0; changed = true; }
        if ui.small_button("90°").clicked() { node.rotation = 90.0; changed = true; }
        if ui.small_button("180°").clicked() { node.rotation = 180.0; changed = true; }
    });
    ui.add_space(6.0);

    // Scale — linked/unlinked
    ui.label(egui::RichText::new("Scale").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    ui.horizontal(|ui| {
        let link_icon = if ins.scale_linked { "🔗" } else { "🔓" };
        if ui.small_button(link_icon).on_hover_text("Toggle uniform/non-uniform scale").clicked() {
            ins.scale_linked = !ins.scale_linked;
        }
        if ins.scale_linked {
            if ui.add(egui::Slider::new(&mut node.scale, 0.01..=10.0).text("Uniform")).changed() { changed = true; }
        } else {
            if ui.add(egui::DragValue::new(&mut node.scale).speed(0.01).prefix("S:")).changed() { changed = true; }
        }
        if ui.small_button("1.0").clicked() { node.scale = 1.0; changed = true; }
    });

    changed
}

fn show_appearance_tab(ui: &mut egui::Ui, node: &mut SceneNode, ins: &mut InspectorState) -> bool {
    let mut changed = false;
    ui.add_space(4.0);

    // Glyph character
    if node.character.is_some() || node.kind == NodeKind::Glyph {
        ui.label(egui::RichText::new("Character").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
        let glyphs: &[char] = &['@', '#', '*', '+', 'o', 'O', '0', 'x', 'X', '.', '~', '^', '|', '-',
                                 '!', '?', '&', '%', '$', '=', '/', '\\', '<', '>', '{', '}', '[', ']'];
        ui.horizontal_wrapped(|ui| {
            for &ch in glyphs {
                let sel = node.character == Some(ch);
                let btn = egui::Button::new(egui::RichText::new(ch.to_string()).size(14.0).monospace())
                    .fill(if sel { egui::Color32::from_rgb(50, 90, 160) } else { egui::Color32::from_rgb(35,37,46) });
                if ui.add(btn).clicked() {
                    node.character = Some(ch);
                    changed = true;
                }
            }
        });
        ui.add_space(4.0);
    }

    // Color — swatch opens full picker
    ui.label(egui::RichText::new("Color").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    ui.horizontal(|ui| {
        ins.color_edit = [node.color.x, node.color.y, node.color.z, node.color.w];
        let mut rgba = egui::Rgba::from_rgba_unmultiplied(node.color.x, node.color.y, node.color.z, node.color.w);
        if egui::color_picker::color_edit_button_rgba(ui, &mut rgba, egui::color_picker::Alpha::OnlyBlend).changed() {
            node.color.x = rgba.r(); node.color.y = rgba.g(); node.color.z = rgba.b(); node.color.w = rgba.a();
            changed = true;
        }
        ui.label(egui::RichText::new(format!("({:.2},{:.2},{:.2},{:.2})", rgba.r(), rgba.g(), rgba.b(), rgba.a()))
            .size(9.5).color(egui::Color32::GRAY));
    });

    // Color presets
    let presets: &[(egui::Color32, &str)] = &[
        (egui::Color32::from_rgb(255,80,80),  "Red"),
        (egui::Color32::from_rgb(80,180,255), "Blue"),
        (egui::Color32::from_rgb(80,255,120), "Green"),
        (egui::Color32::from_rgb(255,220,50), "Gold"),
        (egui::Color32::from_rgb(200,100,255),"Purple"),
        (egui::Color32::WHITE,                "White"),
        (egui::Color32::from_rgb(60,60,80),   "Dark"),
    ];
    ui.horizontal_wrapped(|ui| {
        for &(col, name) in presets {
            let (rect, resp) = ui.allocate_exact_size(egui::Vec2::new(18.0, 18.0), egui::Sense::click());
            ui.painter().rect_filled(rect, 3.0, col);
            if resp.on_hover_text(name).clicked() {
                let [r,g,b,a] = col.to_array();
                node.color.x = r as f32/255.0; node.color.y = g as f32/255.0;
                node.color.z = b as f32/255.0; node.color.w = a as f32/255.0;
                changed = true;
            }
        }
    });
    ui.add_space(4.0);

    // Emission & Glow
    ui.label(egui::RichText::new("Emission").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    if ui.add(egui::Slider::new(&mut node.emission, 0.0..=5.0).max_decimals(2)).changed() { changed = true; }
    ui.label(egui::RichText::new("Glow Radius").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    if ui.add(egui::Slider::new(&mut node.glow_radius, 0.0..=5.0).max_decimals(2)).changed() { changed = true; }

    changed
}

fn show_physics_tab(ui: &mut egui::Ui, node: &mut SceneNode) -> bool {
    let mut changed = false;
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Physics").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    ui.add_space(4.0);

    egui::Grid::new("phys_grid").num_columns(2).spacing(egui::Vec2::new(8.0, 4.0)).show(ui, |ui| {
        ui.label("Mass:");
        if ui.add(egui::Slider::new(&mut node.mass, 0.0..=100.0).suffix(" kg").max_decimals(2)).changed() { changed = true; }
        ui.end_row();
        ui.label("Charge:");
        if ui.add(egui::Slider::new(&mut node.charge, -5.0..=5.0).max_decimals(2)).changed() { changed = true; }
        ui.end_row();
        ui.label("Temperature:");
        if ui.add(egui::Slider::new(&mut node.temperature, -100.0..=100.0).suffix("°").max_decimals(1)).changed() { changed = true; }
        ui.end_row();
        ui.label("Entropy:");
        if ui.add(egui::Slider::new(&mut node.entropy, 0.0..=1.0).max_decimals(3)).changed() { changed = true; }
        ui.end_row();
    });

    ui.add_space(6.0);
    ui.label(egui::RichText::new("Velocity").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    egui::Grid::new("vel_grid").num_columns(2).spacing(egui::Vec2::new(4.0, 3.0)).show(ui, |ui| {
        for (label, val) in [("Vx", &mut node.velocity.x), ("Vy", &mut node.velocity.y), ("Vz", &mut node.velocity.z)] {
            ui.label(label);
            if ui.add(egui::DragValue::new(val).speed(0.01).max_decimals(3)).changed() { changed = true; }
            ui.end_row();
        }
    });
    if ui.small_button("Zero Velocity").clicked() {
        node.velocity = glam::Vec3::ZERO;
        changed = true;
    }

    changed
}

fn show_tags_tab(ui: &mut egui::Ui, node: &mut SceneNode, ins: &mut InspectorState) -> bool {
    let mut changed = false;
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Tags").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    ui.add_space(4.0);

    // Existing tags as removable pills
    let mut to_remove: Option<usize> = None;
    let pill_colors: &[egui::Color32] = &[
        egui::Color32::from_rgb(60,100,160),
        egui::Color32::from_rgb(80,120,60),
        egui::Color32::from_rgb(120,60,100),
        egui::Color32::from_rgb(100,80,140),
        egui::Color32::from_rgb(140,100,40),
    ];
    if node.tags.is_empty() {
        ui.label(egui::RichText::new("(no tags)").size(10.5).color(egui::Color32::from_rgb(100,105,120)));
    } else {
        ui.horizontal_wrapped(|ui| {
            for (i, tag) in node.tags.iter().enumerate() {
                let pill_col = pill_colors[i % pill_colors.len()];
                let (rect, _) = ui.allocate_exact_size(
                    egui::Vec2::new(tag.len() as f32 * 6.5 + 28.0, 20.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 10.0, pill_col);
                ui.painter().text(
                    rect.left_center() + egui::Vec2::new(6.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    tag,
                    egui::FontId::proportional(10.5),
                    egui::Color32::WHITE,
                );
                // × button
                let x_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(rect.max.x - 16.0, rect.min.y + 2.0),
                    egui::Vec2::new(14.0, 16.0),
                );
                if ui.put(x_rect, egui::Button::new(egui::RichText::new("×").size(10.0))
                    .fill(egui::Color32::TRANSPARENT)).clicked() {
                    to_remove = Some(i);
                }
            }
        });
    }
    if let Some(i) = to_remove {
        node.tags.remove(i);
        changed = true;
    }

    // Add tag input
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut ins.tag_input)
            .hint_text("Add tag...")
            .desired_width(160.0));
        if ui.button("Add").clicked() || (ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ins.tag_input.is_empty()) {
            let tag = ins.tag_input.trim().to_string();
            if !tag.is_empty() && !node.tags.contains(&tag) {
                node.tags.push(tag);
                changed = true;
            }
            ins.tag_input.clear();
        }
    });

    // Quick tag suggestions based on node kind
    let suggestions: &[&str] = match node.kind {
        NodeKind::Glyph  => &["sys:static", "fx:glow", "trigger:on_click", "group:foreground"],
        NodeKind::Entity => &["sys:enemy", "sys:player", "trigger:on_death", "fx:explosion"],
        NodeKind::Field  => &["sys:active", "trigger:on_enter", "group:hazard"],
        NodeKind::Group  => &["sys:layer", "group:environment"],
        NodeKind::Camera => &["sys:main_cam", "sys:cutscene_cam"],
    };
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Suggestions:").size(10.0).color(egui::Color32::GRAY));
    ui.horizontal_wrapped(|ui| {
        for &sug in suggestions {
            if !node.tags.iter().any(|t| t == sug) {
                if ui.small_button(sug).clicked() {
                    node.tags.push(sug.to_string());
                    changed = true;
                }
            }
        }
    });

    changed
}

fn show_advanced_tab(ui: &mut egui::Ui, node: &mut SceneNode) -> bool {
    let mut changed = false;
    ui.add_space(4.0);

    ui.label(egui::RichText::new("Blend Mode").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    let blend_modes = ["Normal", "Additive", "Multiply", "Screen", "Overlay"];
    let mut bm_idx = node.blend_mode_idx.min(blend_modes.len() - 1);
    egui::ComboBox::from_id_salt("blend_mode")
        .selected_text(blend_modes[bm_idx])
        .show_ui(ui, |ui| {
            for (i, &name) in blend_modes.iter().enumerate() {
                if ui.selectable_label(bm_idx == i, name).clicked() { bm_idx = i; changed = true; }
            }
        });
    node.blend_mode_idx = bm_idx;

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Render Layer").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    let layers = ["Background", "Entity", "Overlay", "UI"];
    let mut layer_idx = node.render_layer_idx.min(layers.len() - 1);
    egui::ComboBox::from_id_salt("render_layer")
        .selected_text(layers[layer_idx])
        .show_ui(ui, |ui| {
            for (i, &name) in layers.iter().enumerate() {
                if ui.selectable_label(layer_idx == i, name).clicked() { layer_idx = i; changed = true; }
            }
        });
    node.render_layer_idx = layer_idx;

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Z-Order").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    if ui.add(egui::Slider::new(&mut node.z_order, -100.0..=100.0).max_decimals(1)).changed() { changed = true; }

    ui.add_space(6.0);
    ui.separator();
    ui.label(egui::RichText::new("Physics Override").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    ui.add_space(2.0);
    ui.checkbox(&mut node.is_static, "Is Static");
    ui.checkbox(&mut node.is_trigger, "Is Trigger");

    ui.add_space(6.0);
    ui.label(egui::RichText::new("Collision Response").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    let responses = ["Bounce", "Absorb", "PassThrough"];
    let mut cr_idx = node.collision_response_idx.min(responses.len() - 1);
    egui::ComboBox::from_id_salt("collision_resp")
        .selected_text(responses[cr_idx])
        .show_ui(ui, |ui| {
            for (i, &name) in responses.iter().enumerate() {
                if ui.selectable_label(cr_idx == i, name).clicked() { cr_idx = i; changed = true; }
            }
        });
    node.collision_response_idx = cr_idx;

    ui.add_space(6.0);
    ui.separator();
    ui.label(egui::RichText::new("Lifetime").size(11.0).color(egui::Color32::from_rgb(160,170,190)));
    ui.checkbox(&mut node.finite_lifetime, "Finite Lifetime");
    if node.finite_lifetime {
        if ui.add(egui::Slider::new(&mut node.lifetime_seconds, 0.1..=60.0).suffix("s").text("Duration")).changed() {
            changed = true;
        }
    }

    changed
}

// ════════════════════════════════════════════════════════════════════════════
// EXPANDED TOOLBAR — unicode symbols, shortcuts, separator groups, quick palette
// ════════════════════════════════════════════════════════════════════════════

pub fn toolbar_panel_extended(ctx: &egui::Context, state: &mut EditorState, _engine: &mut ProofEngine) {
    const ACCENT: egui::Color32 = egui::Color32::from_rgb(70, 130, 200);
    const TOOLBAR_BG: egui::Color32 = egui::Color32::from_rgb(22, 24, 30);
    const SEP_COLOR: egui::Color32 = egui::Color32::from_rgb(50, 54, 68);

    egui::TopBottomPanel::top("toolbar_ext")
        .exact_height(42.0)
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            ui.painter().rect_filled(rect, 0.0, TOOLBAR_BG);

            ui.horizontal_centered(|ui| {
                ui.add_space(6.0);

                // ── Group 1: Selection & Transform tools ─────────────────
                let tools_g1 = [
                    (ToolKind::Select,   "V",  "Select",  "V"),
                    (ToolKind::Move,     "✥",  "Move",    "G"),
                ];
                for (kind, icon, tooltip, shortcut) in &tools_g1 {
                    let sel = state.tool == *kind;
                    let label = egui::RichText::new(*icon).size(14.0).monospace();
                    let btn = egui::Button::new(label)
                        .fill(if sel { ACCENT } else { egui::Color32::from_rgb(38, 40, 52) })
                        .stroke(egui::Stroke::new(1.0, if sel { egui::Color32::from_rgb(120,170,230) } else { SEP_COLOR }))
                        .min_size(egui::Vec2::new(32.0, 32.0));
                    let r = ui.add(btn).on_hover_text(format!("{} [{}]", tooltip, shortcut));
                    if r.clicked() { state.tool = *kind; }
                }

                // Separator
                ui.add_space(4.0);
                ui.painter().vline(ui.cursor().min.x, ui.max_rect().y_range(), egui::Stroke::new(1.0, SEP_COLOR));
                ui.add_space(4.0);

                // ── Group 2: Placement tools ─────────────────────────────
                let tools_g2 = [
                    (ToolKind::Place,    "◆",  "Place Glyph",  "P"),
                    (ToolKind::Field,    "~",  "Place Field",  "F"),
                    (ToolKind::Entity,   "@",  "Place Entity", "E"),
                    (ToolKind::Particle, "✦",  "Particle Burst","X"),
                ];
                for (kind, icon, tooltip, shortcut) in &tools_g2 {
                    let sel = state.tool == *kind;
                    let label = egui::RichText::new(*icon).size(14.0).monospace();
                    let btn = egui::Button::new(label)
                        .fill(if sel { ACCENT } else { egui::Color32::from_rgb(38, 40, 52) })
                        .stroke(egui::Stroke::new(1.0, if sel { egui::Color32::from_rgb(120,170,230) } else { SEP_COLOR }))
                        .min_size(egui::Vec2::new(32.0, 32.0));
                    let r = ui.add(btn).on_hover_text(format!("{} [{}]", tooltip, shortcut));
                    if r.clicked() { state.tool = *kind; }
                }

                // Separator
                ui.add_space(4.0);
                ui.painter().vline(ui.cursor().min.x, ui.max_rect().y_range(), egui::Stroke::new(1.0, SEP_COLOR));
                ui.add_space(4.0);

                // ── Group 3: Recent palette ──────────────────────────────
                ui.label(egui::RichText::new("Recent:").size(10.0).color(egui::Color32::from_rgb(120,130,150)));
                // Last 5 chars
                let recent_chars: Vec<char> = {
                    let palette = CHAR_PALETTES.get(state.char_palette_idx).map(|(_, c)| *c).unwrap_or(&[]);
                    palette.iter().take(5).cloned().collect()
                };
                for ch in &recent_chars {
                    let btn = egui::Button::new(egui::RichText::new(ch.to_string()).size(13.0).monospace())
                        .fill(egui::Color32::from_rgb(35,37,46))
                        .stroke(egui::Stroke::new(1.0, SEP_COLOR))
                        .min_size(egui::Vec2::new(24.0, 28.0));
                    ui.add(btn).on_hover_text(format!("Char: '{}'", ch));
                }

                ui.add_space(4.0);

                // Last 5 colors
                let recent_colors: Vec<(f32,f32,f32)> = {
                    let palette = COLOR_PALETTES.get(state.color_palette_idx).map(|(_, c)| *c).unwrap_or(&[]);
                    palette.iter().take(5).cloned().collect()
                };
                for &(r, g, b) in &recent_colors {
                    let col = egui::Color32::from_rgb((r*255.0) as u8, (g*255.0) as u8, (b*255.0) as u8);
                    let (rect, resp) = ui.allocate_exact_size(egui::Vec2::new(22.0, 28.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 3.0, col);
                    resp.on_hover_text(format!("RGB({:.2},{:.2},{:.2})", r, g, b));
                }

                // Separator
                ui.add_space(4.0);
                ui.painter().vline(ui.cursor().min.x, ui.max_rect().y_range(), egui::Stroke::new(1.0, SEP_COLOR));
                ui.add_space(4.0);

                // ── Group 4: Field type selector ─────────────────────────
                let fn_: Vec<&str> = FieldType::all().iter().map(|f| f.label()).collect();
                ui.label(egui::RichText::new("Field:").size(10.5).color(egui::Color32::from_rgb(140,150,170)));
                egui::ComboBox::from_id_salt("fl_ext").selected_text(fn_[state.field_type_idx])
                    .width(100.0).show_ui(ui, |ui| {
                    for (i, n) in fn_.iter().enumerate() {
                        ui.selectable_value(&mut state.field_type_idx, i, *n);
                    }
                });

                // Emission & Glow quick sliders
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Em:").size(10.0).color(egui::Color32::from_rgb(140,150,170)));
                ui.add(egui::Slider::new(&mut state.emission, 0.0..=5.0).max_decimals(1).min_decimals(1));
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Glow:").size(10.0).color(egui::Color32::from_rgb(140,150,170)));
                ui.add(egui::Slider::new(&mut state.glow_radius, 0.0..=5.0).max_decimals(1).min_decimals(1));

                // Right-aligned info block
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    // FPS with color coding
                    let fps = state.fps;
                    let fps_color = if fps >= 55.0 { egui::Color32::from_rgb(80,220,100) }
                        else if fps >= 30.0 { egui::Color32::from_rgb(255,210,60) }
                        else { egui::Color32::from_rgb(255,80,80) };
                    ui.label(egui::RichText::new(format!("{:.0} fps", fps)).color(fps_color).size(12.0).strong());
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("({:.1}, {:.1})", state.cam_x, state.cam_y))
                        .size(10.5).color(egui::Color32::from_rgb(120,130,150)));
                    ui.separator();
                    let dirty = if !state.undo_stack.is_empty() { " *" } else { "" };
                    ui.label(egui::RichText::new(format!("scene{}  {} nodes", dirty, state.document.node_count()))
                        .size(11.0).color(egui::Color32::from_rgb(180,185,200)));
                    ui.separator();
                    let sel = state.document.selection.len();
                    if sel > 0 {
                        ui.label(egui::RichText::new(format!("{} selected", sel)).size(10.5)
                            .color(egui::Color32::from_rgb(100,180,255)));
                    }
                });
            });
        });
}

// ════════════════════════════════════════════════════════════════════════════
// EXPANDED STATUS BAR
// ════════════════════════════════════════════════════════════════════════════

pub fn status_bar_extended(ctx: &egui::Context, state: &EditorState) {
    egui::TopBottomPanel::bottom("status_bar_ext")
        .exact_height(28.0)
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 20, 26));

            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);

                // Status message (fades)
                if state.status_timer > 0.0 {
                    let alpha = (state.status_timer * 85.0).min(255.0) as u8;
                    let dot_col = egui::Color32::from_rgba_unmultiplied(100,220,100,alpha);
                    let (dr, _) = ui.allocate_exact_size(egui::Vec2::new(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(dr.center(), 4.0, dot_col);
                    ui.label(egui::RichText::new(&state.status_msg).size(11.0)
                        .color(egui::Color32::from_rgba_unmultiplied(200,225,200,alpha)));
                } else {
                    let (dr, _) = ui.allocate_exact_size(egui::Vec2::new(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(dr.center(), 4.0, egui::Color32::from_rgb(60,70,80));
                    ui.label(egui::RichText::new("Ready").size(11.0).color(egui::Color32::from_rgb(70,80,95)));
                }

                ui.separator();

                // Camera position
                ui.label(egui::RichText::new(format!("Cam: ({:.1}, {:.1})", state.cam_x, state.cam_y))
                    .size(10.5).color(egui::Color32::from_rgb(120,130,150)));
                ui.separator();

                // Zoom level (stored as 1/cam_zoom, approximated here)
                ui.label(egui::RichText::new("Zoom: 100%").size(10.5).color(egui::Color32::from_rgb(120,130,150)));
                ui.separator();

                // Node stats
                let nc = state.document.node_count();
                let sc = state.document.selection.len();
                let gc = state.document.glyph_count();
                let fc = state.document.field_count();
                let ec = state.document.nodes().filter(|n| n.kind == NodeKind::Entity).count();
                ui.label(egui::RichText::new(format!("Nodes: {}", nc)).size(10.5)
                    .color(egui::Color32::from_rgb(160,165,180)));
                if sc > 0 {
                    ui.label(egui::RichText::new(format!("| Sel: {}", sc)).size(10.5)
                        .color(egui::Color32::from_rgb(100,180,255)));
                }
                ui.label(egui::RichText::new(format!("| G:{} F:{} E:{}", gc, fc, ec)).size(10.0)
                    .color(egui::Color32::from_rgb(120,130,150)));

                // Right side: FPS, undo/redo, memory
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);

                    // FPS
                    let fps = state.fps;
                    let fps_color = if fps >= 55.0 { egui::Color32::from_rgb(80,220,100) }
                        else if fps >= 30.0 { egui::Color32::from_rgb(255,210,60) }
                        else { egui::Color32::from_rgb(255,80,80) };
                    ui.label(egui::RichText::new(format!("{:.0} fps", fps)).size(11.0).color(fps_color));
                    ui.separator();

                    // Undo/redo depth
                    ui.label(egui::RichText::new(
                        format!("Undo:{} Redo:{}", state.undo_stack.len(), state.redo_stack.len()))
                        .size(10.0).color(egui::Color32::from_rgb(100,110,130)));
                    ui.separator();

                    // Memory estimate (rough: 1KB per node)
                    let mem_kb = state.document.node_count() * 2;
                    let mem_col = if mem_kb > 10000 { egui::Color32::from_rgb(255,80,80) }
                        else if mem_kb > 2000 { egui::Color32::from_rgb(255,200,60) }
                        else { egui::Color32::from_rgb(120,130,150) };
                    ui.label(egui::RichText::new(format!("~{} KB", mem_kb)).size(10.0).color(mem_col));
                });
            });
        });
}
