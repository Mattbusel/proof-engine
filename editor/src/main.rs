//! Proof Editor — Visual staging and scene authoring environment.
//! Uses egui for UI, proof-engine for the viewport.

#[allow(unused)] mod scene;
#[allow(unused)] mod tools;
#[allow(unused)] mod commands;
#[allow(unused)] mod hotkeys;
#[allow(unused)] mod clipboard;
#[allow(unused)] mod preferences;
#[allow(unused)] mod viewport;
#[allow(unused)] mod layout;

use proof_engine::prelude::*;
use proof_engine::input::Key;
use scene::{SceneDocument, NodeKind, FieldType};
use tools::{ToolKind, CHAR_PALETTES, COLOR_PALETTES};
use std::sync::Arc;
use std::f32::consts::TAU;

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

    // State
    let mut document = SceneDocument::new();
    let mut tool = ToolKind::Place;
    let mut char_palette_idx = 0usize;
    let mut color_palette_idx = 0usize;
    let mut field_type_idx = 0usize;
    let mut emission = 1.5f32;
    let mut glow_radius = 1.0f32;
    let mut fps = 60.0f32;
    let mut fps_timer = 0.0f32;
    let mut fps_frames = 0u32;
    let mut egui_initialized = false;
    let mut egui_ctx = egui::Context::default();
    let mut egui_painter: Option<egui_glow::Painter> = None;
    let mut show_help = false;
    let mut cam_x = 0.0f32;
    let mut cam_y = 0.0f32;
    let mut spawn_counter = 0u32;
    let mut status_msg = String::new();
    let mut status_timer = 0.0f32;
    let mut needs_rebuild = false;

    // Viewport interaction state
    let mut dragging = false;
    let mut drag_start_world = Vec3::ZERO;
    let mut drag_start_mouse = (0.0f32, 0.0f32);
    let mut box_selecting = false;
    let mut box_start = (0.0f32, 0.0f32);
    let mut clipboard: Vec<scene::SceneNode> = Vec::new();

    spawn_grid(&mut engine);

    engine.run_with_overlay(move |engine, dt, gl| {
        fps_frames += 1;
        fps_timer += dt;
        if fps_timer >= 0.5 { fps = fps_frames as f32 / fps_timer; fps_frames = 0; fps_timer = 0.0; }
        status_timer = (status_timer - dt).max(0.0);

        // Init egui
        if !egui_initialized {
            let gl_arc: Arc<glow::Context> = unsafe {
                let ptr = gl as *const glow::Context;
                Arc::increment_strong_count(ptr);
                Arc::from_raw(ptr)
            };
            egui_painter = Some(egui_glow::Painter::new(gl_arc, "", None, false).expect("egui painter"));
            egui_initialized = true;
        }

        // Keyboard shortcuts (before egui so they always work)
        let input = engine.input.clone();
        if input.just_pressed(Key::F1) { show_help = !show_help; }
        if input.just_pressed(Key::Space) { engine.add_trauma(0.3); }
        if input.just_pressed(Key::Escape) { document.selection.clear(); dragging = false; box_selecting = false; }

        // Copy/paste
        if input.ctrl() && input.just_pressed(Key::C) {
            clipboard.clear();
            for &id in &document.selection {
                if let Some(node) = document.get_node(id) { clipboard.push(node.clone()); }
            }
            if !clipboard.is_empty() { set_status(&mut status_msg, &mut status_timer, &format!("Copied {}", clipboard.len())); }
        }
        if input.ctrl() && input.just_pressed(Key::V) && !clipboard.is_empty() {
            let mut new_ids = Vec::new();
            for node in &clipboard {
                let mut n = node.clone();
                n.position += Vec3::new(1.0, -1.0, 0.0); // offset paste
                let nid = document.next_id; document.next_id += 1;
                n.id = nid;
                n.name = format!("{}_copy", n.name);
                document.nodes.push(n);
                new_ids.push(nid);
            }
            document.selection = new_ids;
            needs_rebuild = true;
            set_status(&mut status_msg, &mut status_timer, &format!("Pasted {}", clipboard.len()));
        }

        if input.just_pressed(Key::V) && !input.ctrl() { tool = ToolKind::Select; }
        if input.just_pressed(Key::G) { tool = ToolKind::Move; }
        if input.just_pressed(Key::P) { tool = ToolKind::Place; }
        if input.just_pressed(Key::F) && !input.ctrl() { tool = ToolKind::Field; }
        if input.just_pressed(Key::E) { tool = ToolKind::Entity; }
        if input.just_pressed(Key::X) { tool = ToolKind::Particle; }
        if input.just_pressed(Key::Delete) {
            let sel = document.selection.clone();
            let count = sel.len();
            for id in sel { document.remove_node(id); }
            document.selection.clear();
            if count > 0 { needs_rebuild = true; set_status(&mut status_msg, &mut status_timer, &format!("Deleted {}", count)); }
        }
        if input.ctrl() && input.just_pressed(Key::S) {
            match document.save("scene.json") {
                Ok(_) => set_status(&mut status_msg, &mut status_timer, "Saved scene.json"),
                Err(e) => set_status(&mut status_msg, &mut status_timer, &format!("Save failed: {}", e)),
            }
        }
        if input.ctrl() && input.just_pressed(Key::O) {
            match SceneDocument::load("scene.json") {
                Ok(doc) => { document = doc; needs_rebuild = true; set_status(&mut status_msg, &mut status_timer, "Loaded scene.json"); }
                Err(e) => set_status(&mut status_msg, &mut status_timer, &format!("Load failed: {}", e)),
            }
        }
        if input.ctrl() && input.just_pressed(Key::N) {
            document = SceneDocument::new();
            needs_rebuild = true;
            set_status(&mut status_msg, &mut status_timer, "New scene");
        }
        if input.ctrl() && input.just_pressed(Key::A) { document.select_all(); }
        if input.ctrl() && input.just_pressed(Key::D) {
            let sel = document.selection.clone();
            let mut new_ids = Vec::new();
            for id in sel { if let Some(nid) = document.duplicate_node(id) { new_ids.push(nid); } }
            if !new_ids.is_empty() { document.selection = new_ids; needs_rebuild = true; set_status(&mut status_msg, &mut status_timer, "Duplicated"); }
        }

        // Camera pan (arrows always work, WASD only without ctrl)
        let cam_speed = 12.0 * dt;
        if input.is_pressed(Key::Up) || (input.is_pressed(Key::W) && !input.ctrl()) { cam_y += cam_speed; }
        if input.is_pressed(Key::Down) || (input.is_pressed(Key::S) && !input.ctrl()) { cam_y -= cam_speed; }
        if input.is_pressed(Key::Left) || (input.is_pressed(Key::A) && !input.ctrl()) { cam_x -= cam_speed; }
        if input.is_pressed(Key::Right) || (input.is_pressed(Key::D) && !input.ctrl()) { cam_x += cam_speed; }
        engine.camera.position.x.target = cam_x;
        engine.camera.position.y.target = cam_y;
        // Also directly set position for instant response (bypass spring lag)
        engine.camera.position.x.position = cam_x;
        engine.camera.position.y.position = cam_y;

        // Rebuild scene if needed
        if needs_rebuild {
            rebuild_scene(engine, &document);
            needs_rebuild = false;
        }

        // egui
        if let Some(painter) = egui_painter.as_mut() {
            let (win_w, win_h) = engine.window_size();
            let mut raw_input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(win_w as f32, win_h as f32))),
                ..Default::default()
            };
            raw_input.events.push(egui::Event::PointerMoved(egui::pos2(input.mouse_x, input.mouse_y)));
            if input.mouse_left_just_pressed {
                raw_input.events.push(egui::Event::PointerButton { pos: egui::pos2(input.mouse_x, input.mouse_y), button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
            }
            if input.mouse_left_just_released {
                raw_input.events.push(egui::Event::PointerButton { pos: egui::pos2(input.mouse_x, input.mouse_y), button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
            }
            if input.scroll_delta != 0.0 {
                raw_input.events.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, input.scroll_delta), modifiers: Default::default() });
            }

            let full_output = egui_ctx.run(raw_input, |ctx| {
                // Menu bar
                egui::TopBottomPanel::top("menu").show(ctx, |ui| {
                    egui::menu::bar(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("New (Ctrl+N)").clicked() { document = SceneDocument::new(); needs_rebuild = true; set_status(&mut status_msg, &mut status_timer, "New scene"); ui.close_menu(); }
                            if ui.button("Save (Ctrl+S)").clicked() { let _ = document.save("scene.json"); set_status(&mut status_msg, &mut status_timer, "Saved"); ui.close_menu(); }
                            if ui.button("Load (Ctrl+O)").clicked() {
                                if let Ok(doc) = SceneDocument::load("scene.json") { document = doc; needs_rebuild = true; set_status(&mut status_msg, &mut status_timer, "Loaded"); }
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Quit").clicked() { engine.request_quit(); }
                        });
                        ui.menu_button("Edit", |ui| {
                            if ui.button("Select All (Ctrl+A)").clicked() { document.select_all(); ui.close_menu(); }
                            if ui.button("Delete (Del)").clicked() {
                                let sel = document.selection.clone(); for id in sel { document.remove_node(id); }
                                document.selection.clear(); needs_rebuild = true; ui.close_menu();
                            }
                            if ui.button("Duplicate (Ctrl+D)").clicked() {
                                let sel = document.selection.clone();
                                for id in sel { document.duplicate_node(id); }
                                needs_rebuild = true; ui.close_menu();
                            }
                        });
                        ui.menu_button("View", |ui| {
                            if ui.button("Help (F1)").clicked() { show_help = !show_help; ui.close_menu(); }
                            if ui.button("Toggle Bloom").clicked() { engine.config.render.bloom_enabled = !engine.config.render.bloom_enabled; ui.close_menu(); }
                            if ui.button("Reset Camera").clicked() { cam_x = 0.0; cam_y = 0.0; ui.close_menu(); }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("FPS: {:.0}", fps));
                            ui.separator();
                            ui.label(format!("Nodes: {}", document.node_count()));
                            if status_timer > 0.0 { ui.separator(); ui.colored_label(egui::Color32::from_rgb(100, 200, 100), &status_msg); }
                        });
                    });
                });

                // Hierarchy
                egui::SidePanel::left("hierarchy").default_width(180.0).show(ctx, |ui| {
                    ui.heading("Hierarchy");
                    ui.separator();
                    if document.node_count() == 0 { ui.label("Empty scene"); ui.label("Click viewport to place."); }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut clicked_id = None;
                        for node in document.nodes() {
                            let sel = document.selection.contains(&node.id);
                            let icon = match node.kind { NodeKind::Glyph => "@", NodeKind::Field => "~", NodeKind::Entity => "#", _ => ">" };
                            if ui.selectable_label(sel, format!("{} {}", icon, node.name)).clicked() { clicked_id = Some(node.id); }
                        }
                        if let Some(id) = clicked_id { document.selection = vec![id]; }
                    });
                });

                // Inspector
                egui::SidePanel::right("inspector").default_width(220.0).show(ctx, |ui| {
                    ui.heading("Inspector");
                    ui.separator();
                    if let Some(&id) = document.selection.first() {
                        let mut changed = false;
                        if let Some(node) = document.get_node_mut(id) {
                            ui.horizontal(|ui| { ui.label("Name:"); ui.label(&node.name); });
                            ui.label(format!("Type: {:?}", node.kind));
                            ui.separator();
                            ui.label("Position");
                            changed |= ui.add(egui::Slider::new(&mut node.position.x, -20.0..=20.0).text("X")).changed();
                            changed |= ui.add(egui::Slider::new(&mut node.position.y, -20.0..=20.0).text("Y")).changed();
                            ui.separator();
                            changed |= ui.add(egui::Slider::new(&mut node.emission, 0.0..=5.0).text("Emission")).changed();
                            changed |= ui.add(egui::Slider::new(&mut node.glow_radius, 0.0..=5.0).text("Glow")).changed();
                            changed |= ui.add(egui::Slider::new(&mut node.scale, 0.1..=5.0).text("Scale")).changed();
                            if let Some(ch) = node.character { ui.label(format!("Char: '{}'", ch)); }
                            // Color
                            let mut rgb = [node.color.x, node.color.y, node.color.z];
                            if ui.color_edit_button_rgb(&mut rgb).changed() {
                                node.color.x = rgb[0]; node.color.y = rgb[1]; node.color.z = rgb[2];
                                changed = true;
                            }
                        }
                        if changed { needs_rebuild = true; }
                    } else {
                        ui.label("No selection");
                        ui.label("Click viewport to place.");
                    }
                });

                // Toolbar
                egui::TopBottomPanel::bottom("toolbar").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Tool:");
                        for (kind, label) in &[(ToolKind::Select,"Select(V)"),(ToolKind::Move,"Move(G)"),(ToolKind::Place,"Place(P)"),
                            (ToolKind::Field,"Field(F)"),(ToolKind::Entity,"Entity(E)"),(ToolKind::Particle,"Burst(X)")] {
                            if ui.selectable_label(tool == *kind, *label).clicked() { tool = *kind; }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Chars:");
                        let cn: Vec<&str> = CHAR_PALETTES.iter().map(|(n,_)| *n).collect();
                        egui::ComboBox::from_id_salt("ch").selected_text(cn[char_palette_idx]).show_ui(ui, |ui| {
                            for (i, n) in cn.iter().enumerate() { ui.selectable_value(&mut char_palette_idx, i, *n); }
                        });
                        ui.label("Colors:");
                        let ccn: Vec<&str> = COLOR_PALETTES.iter().map(|(n,_)| *n).collect();
                        egui::ComboBox::from_id_salt("co").selected_text(ccn[color_palette_idx]).show_ui(ui, |ui| {
                            for (i, n) in ccn.iter().enumerate() { ui.selectable_value(&mut color_palette_idx, i, *n); }
                        });
                        ui.label("Field:");
                        let fn_: Vec<&str> = FieldType::all().iter().map(|f| f.label()).collect();
                        egui::ComboBox::from_id_salt("fl").selected_text(fn_[field_type_idx]).show_ui(ui, |ui| {
                            for (i, n) in fn_.iter().enumerate() { ui.selectable_value(&mut field_type_idx, i, *n); }
                        });
                        ui.add(egui::Slider::new(&mut emission, 0.0..=5.0).text("Em"));
                        ui.add(egui::Slider::new(&mut glow_radius, 0.0..=5.0).text("Glow"));
                    });
                    ui.horizontal(|ui| {
                        let bloom_label = if engine.config.render.bloom_enabled { "Bloom ON" } else { "Bloom OFF" };
                        if ui.button(bloom_label).clicked() { engine.config.render.bloom_enabled = !engine.config.render.bloom_enabled; }
                        ui.add(egui::Slider::new(&mut engine.config.render.bloom_intensity, 0.0..=5.0).text("Bloom"));
                        ui.add(egui::Slider::new(&mut engine.config.render.chromatic_aberration, 0.0..=0.02).text("CA"));
                        ui.add(egui::Slider::new(&mut engine.config.render.film_grain, 0.0..=0.1).text("Grain"));
                        if ui.button("Shake!").clicked() { engine.add_trauma(0.5); }
                    });
                });

                if show_help {
                    egui::Window::new("Help").show(ctx, |ui| {
                        ui.label("Click viewport to place with current tool");
                        ui.label("WASD = Pan camera");
                        ui.label("V/G/P/F/E/X = Switch tool");
                        ui.label("Ctrl+S/O/N = Save/Load/New");
                        ui.label("Delete = Remove selection");
                        ui.label("Space = Screen shake");
                        if ui.button("Close").clicked() { show_help = false; }
                    });
                }
            });

            // Check if egui wants the pointer (clicked on a panel)
            let egui_wants_pointer = egui_ctx.is_pointer_over_area();

            // Paint egui
            let prims = egui_ctx.tessellate(full_output.shapes, egui_ctx.pixels_per_point());
            let (w, h) = engine.window_size();
            painter.paint_and_update_textures([w, h], egui_ctx.pixels_per_point(), &prims, &full_output.textures_delta);

            // ═══════════════════════════════════════════════════════════════
            // VIEWPORT CLICK → actually place things in the scene
            // Only if egui didn't consume the click (clicked in viewport area)
            // ═══════════════════════════════════════════════════════════════
            if input.mouse_left_just_pressed && !egui_wants_pointer {
                let mx = (input.mouse_x / win_w as f32 - 0.5) * 18.0 + cam_x;
                let my = -((input.mouse_y / win_h as f32 - 0.5) * 11.0) + cam_y;
                let world_pos = Vec3::new(mx, my, 0.0);

                match tool {
                    ToolKind::Place => {
                        let (_, chars) = CHAR_PALETTES[char_palette_idx];
                        let (_, colors) = COLOR_PALETTES[color_palette_idx];
                        let count = 5 + (spawn_counter % 4) as usize;
                        for i in 0..count {
                            let angle = (i as f32 / count as f32) * TAU;
                            let r = 0.3 + (i as f32 * 0.17).sin().abs() * 0.5;
                            let ch = chars[(spawn_counter as usize + i) % chars.len()];
                            let (cr, cg, cb) = colors[i % colors.len()];
                            let brightness = 0.6 + (i as f32 * 0.2).sin().abs() * 0.4;
                            let color = Vec4::new(cr * brightness, cg * brightness, cb * brightness, 0.9);
                            let pos = world_pos + Vec3::new(angle.cos() * r, angle.sin() * r, 0.0);
                            let nid = document.add_glyph_node(pos, ch, color, emission, glow_radius);
                            spawn_glyph_from_doc(engine, &document, nid);
                        }
                        spawn_counter += 1;
                        set_status(&mut status_msg, &mut status_timer, &format!("Placed {} glyphs", count));
                    }
                    ToolKind::Field => {
                        let ft = FieldType::all()[field_type_idx];
                        let nid = document.add_field_node(world_pos, ft);
                        engine.add_field(ft.to_force_field(world_pos));
                        // Visual marker for the field
                        engine.spawn_glyph(Glyph {
                            character: '~', position: world_pos,
                            color: Vec4::new(1.0, 0.7, 0.2, 0.8), emission: 0.5,
                            glow_color: Vec3::new(1.0, 0.5, 0.1), glow_radius: 2.0,
                            layer: RenderLayer::Entity, ..Default::default()
                        });
                        set_status(&mut status_msg, &mut status_timer, &format!("Placed {}", ft.label()));
                    }
                    ToolKind::Entity => {
                        let (_, colors) = COLOR_PALETTES[color_palette_idx];
                        let (cr, cg, cb) = colors[0];
                        let color = Vec4::new(cr, cg, cb, 0.9);
                        let nid = document.add_entity_node(world_pos);
                        let mut ent = AmorphousEntity::new("Entity", world_pos);
                        ent.entity_mass = 3.0; ent.cohesion = 0.7;
                        ent.pulse_rate = 0.5; ent.pulse_depth = 0.15;
                        ent.hp = 100.0; ent.max_hp = 100.0;
                        let chars = ['@','#','*','+','o','x','X','O','.',':','~','='];
                        for i in 0..12 {
                            let a = (i as f32 / 12.0) * TAU;
                            ent.formation.push(Vec3::new(a.cos() * 0.8, a.sin() * 0.8, 0.0));
                            ent.formation_chars.push(chars[i % chars.len()]);
                            ent.formation_colors.push(color);
                        }
                        engine.spawn_entity(ent);
                        set_status(&mut status_msg, &mut status_timer, "Placed entity");
                    }
                    ToolKind::Particle => {
                        let (_, colors) = COLOR_PALETTES[color_palette_idx];
                        let (cr, cg, cb) = colors[0];
                        engine.emit_particles(
                            proof_engine::particle::EmitterPreset::DeathExplosion { color: Vec4::new(cr, cg, cb, 1.0) },
                            world_pos,
                        );
                        set_status(&mut status_msg, &mut status_timer, "Particle burst!");
                    }
                    ToolKind::Select => {
                        if let Some(id) = document.pick_at(world_pos, 1.5) {
                            if input.shift() {
                                // Shift+click: toggle selection (multi-select)
                                document.toggle_selection(id);
                            } else {
                                document.selection = vec![id];
                            }
                            set_status(&mut status_msg, &mut status_timer, &format!("Selected ({})", document.selection.len()));
                        } else {
                            // Start box select
                            box_selecting = true;
                            box_start = (input.mouse_x, input.mouse_y);
                            if !input.shift() { document.selection.clear(); }
                        }
                    }
                    ToolKind::Move => {
                        // Click to select, then drag to move
                        if let Some(id) = document.pick_at(world_pos, 1.5) {
                            if !document.selection.contains(&id) {
                                if input.shift() { document.toggle_selection(id); }
                                else { document.selection = vec![id]; }
                            }
                            dragging = true;
                            drag_start_world = world_pos;
                            drag_start_mouse = (input.mouse_x, input.mouse_y);
                            set_status(&mut status_msg, &mut status_timer, "Dragging...");
                        }
                    }
                    _ => {}
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // DRAG TO MOVE — update while mouse held
            // ═══════════════════════════════════════════════════════════════
            if dragging && !egui_wants_pointer {
                if input.is_pressed(proof_engine::input::Key::Escape) {
                    dragging = false; // cancel drag
                } else if input.mouse_left_just_released {
                    // Finish drag — apply delta to all selected nodes
                    let mx = (input.mouse_x / win_w as f32 - 0.5) * 18.0 + cam_x;
                    let my = -((input.mouse_y / win_h as f32 - 0.5) * 11.0) + cam_y;
                    let delta = Vec3::new(mx, my, 0.0) - drag_start_world;
                    if delta.length() > 0.05 {
                        let sel = document.selection.clone();
                        for id in sel { document.translate_node(id, delta); }
                        needs_rebuild = true;
                        set_status(&mut status_msg, &mut status_timer, &format!("Moved by ({:.1}, {:.1})", delta.x, delta.y));
                    }
                    dragging = false;
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // BOX SELECT — drag rectangle to select multiple
            // ═══════════════════════════════════════════════════════════════
            if box_selecting && !egui_wants_pointer {
                if input.mouse_left_just_released {
                    let (sx, sy) = box_start;
                    let (ex, ey) = (input.mouse_x, input.mouse_y);
                    // Convert both corners to world space
                    let wx0 = (sx.min(ex) / win_w as f32 - 0.5) * 18.0 + cam_x;
                    let wy0 = -((sy.max(ey) / win_h as f32 - 0.5) * 11.0) + cam_y;
                    let wx1 = (sx.max(ex) / win_w as f32 - 0.5) * 18.0 + cam_x;
                    let wy1 = -((sy.min(ey) / win_h as f32 - 0.5) * 11.0) + cam_y;

                    // Select all nodes within the box
                    let mut selected = if input.shift() { document.selection.clone() } else { Vec::new() };
                    for node in document.nodes() {
                        let p = node.position;
                        if p.x >= wx0 && p.x <= wx1 && p.y >= wy0 && p.y <= wy1 {
                            if !selected.contains(&node.id) { selected.push(node.id); }
                        }
                    }
                    document.selection = selected;
                    let count = document.selection.len();
                    if count > 0 { set_status(&mut status_msg, &mut status_timer, &format!("Box selected {}", count)); }
                    box_selecting = false;
                }
            }

            // ═══════════════════════════════════════════════════════════════
            // GIZMO VISUALS — render selection indicators as glyphs
            // ═══════════════════════════════════════════════════════════════
            for &id in &document.selection {
                if let Some(node) = document.get_node(id) {
                    // Selection ring around selected nodes
                    let p = node.position;
                    for i in 0..8 {
                        let angle = (i as f32 / 8.0) * TAU;
                        let r = 1.2;
                        engine.spawn_glyph(Glyph {
                            character: '.',
                            position: Vec3::new(p.x + angle.cos() * r, p.y + angle.sin() * r, 0.1),
                            color: Vec4::new(1.0, 0.9, 0.2, 0.5),
                            emission: 0.3,
                            layer: RenderLayer::Overlay,
                            lifetime: 0.02,
                            ..Default::default()
                        });
                    }
                    // Center crosshair
                    for &ch in &['+'] {
                        engine.spawn_glyph(Glyph {
                            character: ch,
                            position: Vec3::new(p.x, p.y, 0.1),
                            color: Vec4::new(1.0, 1.0, 0.3, 0.6),
                            emission: 0.4,
                            layer: RenderLayer::Overlay,
                            lifetime: 0.02,
                            ..Default::default()
                        });
                    }
                    // Axis handles (if Move tool)
                    if tool == ToolKind::Move {
                        // X axis arrow
                        engine.spawn_glyph(Glyph {
                            character: '>',
                            position: Vec3::new(p.x + 1.5, p.y, 0.1),
                            color: Vec4::new(1.0, 0.2, 0.2, 0.8),
                            emission: 0.5,
                            layer: RenderLayer::Overlay,
                            lifetime: 0.02,
                            ..Default::default()
                        });
                        // Y axis arrow
                        engine.spawn_glyph(Glyph {
                            character: '^',
                            position: Vec3::new(p.x, p.y + 1.5, 0.1),
                            color: Vec4::new(0.2, 1.0, 0.2, 0.8),
                            emission: 0.5,
                            layer: RenderLayer::Overlay,
                            lifetime: 0.02,
                            ..Default::default()
                        });
                    }
                }
            }

            // Box select visual (rectangle outline while dragging)
            if box_selecting {
                let (sx, sy) = box_start;
                let (ex, ey) = (input.mouse_x, input.mouse_y);
                // Draw corners as glyphs
                for &(mx, my) in &[(sx, sy), (ex, sy), (sx, ey), (ex, ey)] {
                    let wx = (mx / win_w as f32 - 0.5) * 18.0 + cam_x;
                    let wy = -((my / win_h as f32 - 0.5) * 11.0) + cam_y;
                    engine.spawn_glyph(Glyph {
                        character: '+',
                        position: Vec3::new(wx, wy, 0.2),
                        color: Vec4::new(0.3, 0.7, 1.0, 0.6),
                        emission: 0.5,
                        layer: RenderLayer::Overlay,
                        lifetime: 0.02,
                        ..Default::default()
                    });
                }
            }
        }
    });
}

fn set_status(msg: &mut String, timer: &mut f32, text: &str) {
    *msg = text.to_string();
    *timer = 3.0;
}

fn spawn_glyph_from_doc(engine: &mut ProofEngine, doc: &SceneDocument, nid: u32) {
    if let Some(n) = doc.get_node(nid) {
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

fn rebuild_scene(engine: &mut ProofEngine, doc: &SceneDocument) {
    engine.scene = SceneGraph::new();
    spawn_grid(engine);
    for node in doc.nodes() {
        match node.kind {
            NodeKind::Glyph => {
                engine.spawn_glyph(Glyph {
                    character: node.character.unwrap_or('@'), position: node.position,
                    color: node.color, emission: node.emission,
                    glow_color: Vec3::new(node.color.x, node.color.y, node.color.z),
                    glow_radius: node.glow_radius, mass: 0.1,
                    layer: RenderLayer::Entity, blend_mode: BlendMode::Additive,
                    life_function: Some(MathFunction::Breathing { rate: 0.3, depth: 0.08 }),
                    ..Default::default()
                });
            }
            NodeKind::Field => {
                if let Some(ref ft) = node.field_type {
                    engine.add_field(ft.to_force_field(node.position));
                    engine.spawn_glyph(Glyph {
                        character: '~', position: node.position,
                        color: Vec4::new(1.0, 0.7, 0.2, 0.8), emission: 0.5,
                        glow_color: Vec3::new(1.0, 0.5, 0.1), glow_radius: 2.0,
                        layer: RenderLayer::Entity, ..Default::default()
                    });
                }
            }
            NodeKind::Entity => {
                let mut ent = AmorphousEntity::new(&node.name, node.position);
                ent.entity_mass = 3.0; ent.cohesion = 0.7;
                ent.pulse_rate = 0.5; ent.pulse_depth = 0.15;
                ent.hp = 100.0; ent.max_hp = 100.0;
                let chars = ['@','#','*','+','o','x','X','O','.',':','~','='];
                for i in 0..12 {
                    let a = (i as f32 / 12.0) * TAU;
                    ent.formation.push(Vec3::new(a.cos() * 0.8, a.sin() * 0.8, 0.0));
                    ent.formation_chars.push(chars[i % chars.len()]);
                    ent.formation_colors.push(node.color);
                }
                engine.spawn_entity(ent);
            }
            _ => {}
        }
    }
}

fn spawn_grid(engine: &mut ProofEngine) {
    for y in -15..=15 {
        for x in -20..=20 {
            let on_axis = x == 0 || y == 0;
            let on_major = x % 5 == 0 && y % 5 == 0;
            let ch = if on_axis && on_major { '+' }
                else if on_axis { '-' }
                else if on_major { '.' }
                else if (x + y) % 4 == 0 { '.' }
                else { continue };
            let color = if x == 0 && y == 0 { Vec4::new(1.0, 1.0, 0.3, 0.4) }
                else if on_axis { Vec4::new(0.2, 0.3, 0.5, 0.25) }
                else { Vec4::new(0.15, 0.15, 0.2, 0.12) };
            engine.spawn_glyph(Glyph {
                character: ch, position: Vec3::new(x as f32, y as f32, -2.0),
                color, emission: if on_axis { 0.15 } else { 0.03 },
                layer: RenderLayer::Background, ..Default::default()
            });
        }
    }
}
