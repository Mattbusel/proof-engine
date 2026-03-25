//! Proof Editor — Visual staging and scene authoring environment.
//!
//! Uses egui for all UI (panels, buttons, sliders, etc.) rendered on top
//! of the engine's glyph viewport via egui-glow.
//!
//! Run: `cargo run -p proof-editor`

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
use tools::ToolKind;
use std::sync::Arc;

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

    // Editor state
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
    let mut egui_state: Option<egui_winit::State> = None;
    let mut show_help = false;

    // Spawn editor grid
    spawn_grid(&mut engine);

    engine.run_with_overlay(move |engine, dt, gl| {
        // FPS tracking
        fps_frames += 1;
        fps_timer += dt;
        if fps_timer >= 0.5 {
            fps = fps_frames as f32 / fps_timer;
            fps_frames = 0;
            fps_timer = 0.0;
        }

        // Initialize egui on first frame (need GL context)
        if !egui_initialized {
            // SAFETY: The glow context lives for the entire duration of the engine.
            // We create a non-dropping Arc by leaking a clone of the Arc.
            let gl_arc: Arc<glow::Context> = unsafe {
                let ptr = gl as *const glow::Context;
                Arc::increment_strong_count(ptr);
                Arc::from_raw(ptr)
            };
            let painter = egui_glow::Painter::new(
                gl_arc,
                "",
                None,
                false,
            ).expect("Failed to create egui painter");
            egui_painter = Some(painter);

            if let Some(window) = engine.window() {
                let state = egui_winit::State::new(
                    egui_ctx.clone(),
                    egui::ViewportId::ROOT,
                    window,
                    None,
                    None,
                    None,
                );
                egui_state = Some(state);
            }

            egui_initialized = true;
        }

        // Run egui
        if let (Some(painter), Some(state)) = (egui_painter.as_mut(), egui_state.as_mut()) {
            let raw_input = state.take_egui_input(engine.window().unwrap());
            let full_output = egui_ctx.run(raw_input, |ctx| {
                // ── Menu bar ──
                egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
                    egui::menu::bar(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("New Scene (Ctrl+N)").clicked() {
                                document = SceneDocument::new();
                            }
                            if ui.button("Save (Ctrl+S)").clicked() {
                                let _ = document.save("scene.json");
                            }
                            if ui.button("Load (Ctrl+O)").clicked() {
                                if let Ok(doc) = SceneDocument::load("scene.json") {
                                    document = doc;
                                }
                            }
                            ui.separator();
                            if ui.button("Quit").clicked() {
                                engine.request_quit();
                            }
                        });
                        ui.menu_button("Edit", |ui| {
                            if ui.button("Select All (Ctrl+A)").clicked() {
                                document.select_all();
                            }
                            if ui.button("Delete Selection (Del)").clicked() {
                                let sel = document.selection.clone();
                                for id in sel { document.remove_node(id); }
                                document.selection.clear();
                            }
                            if ui.button("Duplicate (Ctrl+D)").clicked() {
                                let sel = document.selection.clone();
                                for id in sel { document.duplicate_node(id); }
                            }
                        });
                        ui.menu_button("View", |ui| {
                            if ui.button("Toggle Help (F1)").clicked() { show_help = !show_help; }
                            if ui.button("Toggle Bloom").clicked() {
                                engine.config.render.bloom_enabled = !engine.config.render.bloom_enabled;
                            }
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("FPS: {:.0}", fps));
                            ui.separator();
                            ui.label(format!("Nodes: {}", document.node_count()));
                        });
                    });
                });

                // ── Left panel: Hierarchy ──
                egui::SidePanel::left("hierarchy").default_width(200.0).show(ctx, |ui| {
                    ui.heading("Hierarchy");
                    ui.separator();

                    if document.node_count() == 0 {
                        ui.label("Empty scene. Click viewport to place.");
                    }

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut clicked_id = None;
                        for node in document.nodes() {
                            let selected = document.selection.contains(&node.id);
                            let icon = match node.kind {
                                NodeKind::Glyph => "@ ",
                                NodeKind::Field => "~ ",
                                NodeKind::Entity => "# ",
                                NodeKind::Group => "> ",
                                NodeKind::Camera => "C ",
                            };
                            let label = format!("{}{}", icon, node.name);
                            if ui.selectable_label(selected, &label).clicked() {
                                clicked_id = Some(node.id);
                            }
                        }
                        if let Some(id) = clicked_id {
                            if ui.input(|i| i.modifiers.ctrl) {
                                document.toggle_selection(id);
                            } else {
                                document.selection = vec![id];
                            }
                        }
                    });
                });

                // ── Right panel: Inspector ──
                egui::SidePanel::right("inspector").default_width(250.0).show(ctx, |ui| {
                    ui.heading("Inspector");
                    ui.separator();

                    if let Some(&id) = document.selection.first() {
                        if let Some(node) = document.get_node(id) {
                            ui.label(format!("Name: {}", node.name));
                            ui.label(format!("Type: {:?}", node.kind));
                            ui.separator();

                            ui.label("Position");
                            ui.horizontal(|ui| {
                                ui.label(format!("X: {:.2}  Y: {:.2}  Z: {:.2}",
                                    node.position.x, node.position.y, node.position.z));
                            });

                            ui.separator();
                            ui.label(format!("Emission: {:.2}", node.emission));
                            ui.label(format!("Glow: {:.2}", node.glow_radius));

                            if let Some(ch) = node.character {
                                ui.label(format!("Character: '{}'", ch));
                            }

                            // Color preview
                            let c = node.color;
                            let color = egui::Color32::from_rgba_unmultiplied(
                                (c.x * 255.0) as u8, (c.y * 255.0) as u8,
                                (c.z * 255.0) as u8, (c.w * 255.0) as u8,
                            );
                            ui.horizontal(|ui| {
                                ui.label("Color:");
                                let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 20.0), egui::Sense::hover());
                                ui.painter().rect_filled(rect, 2.0, color);
                            });
                        }
                    } else {
                        ui.label("No selection");
                        ui.label("Click in viewport to place objects.");
                    }
                });

                // ── Bottom panel: Toolbar + Settings ──
                egui::TopBottomPanel::bottom("toolbar").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Tool:");
                        for (kind, label) in &[
                            (ToolKind::Select, "Select (V)"),
                            (ToolKind::Move, "Move (G)"),
                            (ToolKind::Place, "Place (P)"),
                            (ToolKind::Field, "Field (F)"),
                            (ToolKind::Entity, "Entity (E)"),
                            (ToolKind::Particle, "Burst (X)"),
                        ] {
                            if ui.selectable_label(tool == *kind, *label).clicked() {
                                tool = *kind;
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Chars:");
                        let char_names = ["ASCII","Digits","Letters","Blocks","Box","Math"];
                        egui::ComboBox::from_id_salt("chars")
                            .selected_text(char_names[char_palette_idx])
                            .show_ui(ui, |ui| {
                                for (i, name) in char_names.iter().enumerate() {
                                    ui.selectable_value(&mut char_palette_idx, i, *name);
                                }
                            });

                        ui.label("Colors:");
                        let color_names = ["Matrix","Fire","Ice","Void","Gold","Mono","Neon","Cyan","Blood","Earth"];
                        egui::ComboBox::from_id_salt("colors")
                            .selected_text(color_names[color_palette_idx])
                            .show_ui(ui, |ui| {
                                for (i, name) in color_names.iter().enumerate() {
                                    ui.selectable_value(&mut color_palette_idx, i, *name);
                                }
                            });

                        ui.label("Field:");
                        let field_names: Vec<&str> = FieldType::all().iter().map(|f| f.label()).collect();
                        egui::ComboBox::from_id_salt("field")
                            .selected_text(field_names[field_type_idx])
                            .show_ui(ui, |ui| {
                                for (i, name) in field_names.iter().enumerate() {
                                    ui.selectable_value(&mut field_type_idx, i, *name);
                                }
                            });

                        ui.add(egui::Slider::new(&mut emission, 0.0..=5.0).text("Emission"));
                        ui.add(egui::Slider::new(&mut glow_radius, 0.0..=5.0).text("Glow"));
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Bloom").clicked() {
                            engine.config.render.bloom_enabled = !engine.config.render.bloom_enabled;
                        }
                        ui.add(egui::Slider::new(&mut engine.config.render.bloom_intensity, 0.0..=5.0).text("Bloom Int"));
                        ui.add(egui::Slider::new(&mut engine.config.render.chromatic_aberration, 0.0..=0.02).text("Chromatic"));
                        ui.add(egui::Slider::new(&mut engine.config.render.film_grain, 0.0..=0.1).text("Grain"));
                        if ui.button("Shake").clicked() {
                            engine.add_trauma(0.4);
                        }
                    });
                });

                // ── Help window ──
                if show_help {
                    egui::Window::new("Help").show(ctx, |ui| {
                        ui.label("PROOF EDITOR CONTROLS");
                        ui.separator();
                        ui.label("Click viewport — Place current tool");
                        ui.label("WASD / Arrows — Pan camera");
                        ui.label("V/G/P/F/E/X — Switch tool");
                        ui.label("Ctrl+S — Save  Ctrl+O — Load");
                        ui.label("Ctrl+N — New  Ctrl+Z — Undo");
                        ui.label("Delete — Remove selection");
                        ui.label("F1 — Toggle this help");
                        ui.label("Space — Screen shake");
                        if ui.button("Close").clicked() { show_help = false; }
                    });
                }
            });

            // Handle egui output
            state.handle_platform_output(engine.window().unwrap(), full_output.platform_output);

            // Paint egui
            let prims = egui_ctx.tessellate(full_output.shapes, egui_ctx.pixels_per_point());
            let (w, h) = engine.window_size();
            painter.paint_and_update_textures(
                [w, h],
                egui_ctx.pixels_per_point(),
                &prims,
                &full_output.textures_delta,
            );
        }

        // Handle keyboard shortcuts
        let input = engine.input.clone();
        if input.just_pressed(Key::F1) { show_help = !show_help; }
        if input.just_pressed(Key::Space) { engine.add_trauma(0.3); }
    });
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
                character: ch,
                position: Vec3::new(x as f32, y as f32, -2.0),
                color,
                emission: if on_axis { 0.15 } else { 0.03 },
                layer: RenderLayer::Background,
                ..Default::default()
            });
        }
    }
}
