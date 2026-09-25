#!/usr/bin/env python
# Second physics expansion pass — adds more bodies to reach target
import os

OUTFILE = r"C:\proof-engine\editor\src\physics_editor.rs"

# We need ~10000 more lines. We'll add deep implementations across all the major systems.
EXPANSION = r"""
// ============================================================
// SOFT BODY SPRING EDITOR (Deep Node/Spring Editing)
// ============================================================

pub fn show_soft_body_spring_editor(ui: &mut egui::Ui, state: &mut SoftBodyEditorState) {
    if let Some(bi) = state.selected_body {
        if let Some(body) = state.bodies.get_mut(bi) {
            egui::CollapsingHeader::new(RichText::new(format!("Springs ({}) - Edit", body.springs.len())).color(Color32::from_rgb(255, 220, 140)))
                .default_open(false)
                .show(ui, |ui| {
                    if ui.button("Add Spring Between Selected Nodes").clicked() {
                        if let (Some(na), Some(nb)) = (state.selected_node, state.selected_node) {
                            if na != nb && na < body.nodes.len() && nb < body.nodes.len() {
                                let rl = SoftSpring::length_between(&body.nodes, na, nb);
                                body.springs.push(SoftSpring::new(na, nb, rl, body.stiffness, 0.1));
                            }
                        }
                    }
                    let mut to_remove = None;
                    egui::ScrollArea::vertical().id_salt("spring_ed").max_height(200.0).show(ui, |ui| {
                        for (si, spring) in body.springs.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("#{:3}", si)).monospace().small().color(Color32::GRAY));
                                ui.label(RichText::new(format!("{} -> {}", spring.a, spring.b)).small());
                                ui.add(egui::DragValue::new(&mut spring.rest_length).speed(0.001).prefix("L:").suffix("m").clamp_range(0.001f32..=10.0));
                                ui.add(egui::DragValue::new(&mut spring.stiffness).speed(1.0).prefix("K:").clamp_range(1.0f32..=5000.0));
                                ui.add(egui::DragValue::new(&mut spring.damping).speed(0.001).prefix("D:").clamp_range(0.0f32..=10.0));
                                if ui.small_button("X").clicked() { to_remove = Some(si); }
                            });
                        }
                    });
                    if let Some(idx) = to_remove { body.springs.remove(idx); }
                    // Spring statistics
                    if !body.springs.is_empty() {
                        let avg_k = body.springs.iter().map(|s| s.stiffness).sum::<f32>() / body.springs.len() as f32;
                        let avg_d = body.springs.iter().map(|s| s.damping).sum::<f32>() / body.springs.len() as f32;
                        let max_stretch = body.springs.iter().map(|s| {
                            if s.a < body.nodes.len() && s.b < body.nodes.len() {
                                let dx = body.nodes[s.b].position[0] - body.nodes[s.a].position[0];
                                let dy = body.nodes[s.b].position[1] - body.nodes[s.a].position[1];
                                let l = (dx*dx+dy*dy).sqrt();
                                (l - s.rest_length).abs()
                            } else { 0.0 }
                        }).fold(0.0f32, f32::max);
                        ui.label(RichText::new(format!("Avg K:{:.1}  Avg D:{:.4}  Max Stretch:{:.4}m", avg_k, avg_d, max_stretch)).small().color(Color32::GRAY));
                    }
                });
        }
    }
}

pub fn show_soft_body_energy_graph(ui: &mut egui::Ui, history: &[f32], label: &str, color: Color32) {
    let desired = Vec2::new(ui.available_width(), 60.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgb(14, 14, 22));
    if history.len() < 2 { return; }
    let max_val = history.iter().cloned().fold(0.0f32, f32::max).max(0.001);
    let n = history.len().min(rect.width() as usize);
    let start = history.len().saturating_sub(n);
    let mut prev = None;
    for (i, &val) in history[start..].iter().enumerate() {
        let x = rect.left() + i as f32 / n as f32 * rect.width();
        let y = rect.bottom() - (val / max_val).clamp(0.0, 1.0) * rect.height() * 0.9;
        let p = Pos2::new(x, y);
        if let Some(prev_p) = prev { painter.line_segment([prev_p, p], Stroke::new(1.0, color)); }
        prev = Some(p);
    }
    painter.text(Pos2::new(rect.left()+2.0, rect.top()+2.0), egui::Align2::LEFT_TOP,
        format!("{} max:{:.4}", label, max_val), FontId::monospace(7.0), color);
}

pub fn show_soft_body_material_presets_panel(ui: &mut egui::Ui, state: &mut SoftBodyEditorState) {
    egui::CollapsingHeader::new(RichText::new("Material Presets").color(Color32::from_rgb(200, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Apply to selected body:").small().color(Color32::GRAY));
            let presets: &[(&str, f32, f32, f32, f32)] = &[
                ("Soft Jelly",    120.0, 0.0, 0.96, 0.5),
                ("Hard Rubber",   800.0, 0.0, 0.98, 0.0),
                ("Steel Cable",  2000.0, 0.0, 0.995, 0.0),
                ("Elastic Band",  300.0, 0.0, 0.92, 0.0),
                ("Balloon",        80.0, 2.0, 0.99, 0.5),
                ("Muscle",        400.0, 0.3, 0.95, 0.3),
                ("Rope",          500.0, 0.0, 0.95, 0.0),
                ("Sponge",         60.0, 0.0, 0.88, 0.0),
                ("Cloth",         300.0, 0.0, 0.97, 0.0),
                ("Spring",       1000.0, 0.0, 0.99, 0.0),
            ];
            egui::Grid::new("sb_presets").num_columns(5).spacing(Vec2::new(6.0, 3.0)).show(ui, |ui| {
                ui.label(RichText::new("Name").strong().small());
                ui.label(RichText::new("K").strong().small());
                ui.label(RichText::new("P").strong().small());
                ui.label(RichText::new("Damp").strong().small());
                ui.label(RichText::new("Apply").strong().small());
                ui.end_row();
                for (name, k, pressure, damp, vp) in presets {
                    ui.label(RichText::new(*name).small());
                    ui.label(RichText::new(format!("{:.0}", k)).small().color(Color32::GRAY));
                    ui.label(RichText::new(format!("{:.1}", pressure)).small().color(Color32::GRAY));
                    ui.label(RichText::new(format!("{:.3}", damp)).small().color(Color32::GRAY));
                    if ui.small_button("Set").clicked() {
                        if let Some(bi) = state.selected_body {
                            if let Some(body) = state.bodies.get_mut(bi) {
                                body.stiffness = *k;
                                body.pressure = *pressure;
                                body.damping = *damp;
                                body.volume_preservation = *vp;
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        });
}

pub fn show_soft_body_simulation_settings(ui: &mut egui::Ui, state: &mut SoftBodyEditorState) {
    egui::CollapsingHeader::new(RichText::new("Simulation Settings").color(Color32::from_rgb(180, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("sb_sim_set").num_columns(2).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
                ui.label("Show nodes:"); ui.checkbox(&mut state.show_nodes, ""); ui.end_row();
                ui.label("Show springs:"); ui.checkbox(&mut state.show_springs, ""); ui.end_row();
                ui.label("Show forces:"); ui.checkbox(&mut state.show_forces, ""); ui.end_row();
                ui.label("Pin mode:"); ui.checkbox(&mut state.pin_mode, ""); ui.end_row();
            });
            ui.separator();
            ui.label(RichText::new("Simulation time").small().color(Color32::GRAY));
            let bar_w = ui.available_width() * 0.8;
            let max_t = 10.0f32;
            let t = state.sim_time.min(max_t) / max_t;
            let desired = Vec2::new(bar_w, 12.0);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 2.0, Color32::from_rgb(30, 30, 40));
            painter.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width() * t, rect.height())), 2.0, Color32::from_rgb(80, 180, 255));
            painter.text(Pos2::new(rect.right() + 4.0, rect.center().y), egui::Align2::LEFT_CENTER, format!("{:.1}s", state.sim_time), FontId::monospace(8.0), Color32::GRAY);
        });
}

// ============================================================
// CLOTH VISUALIZATION HELPERS
// ============================================================

pub fn draw_cloth_node_velocities(painter: &Painter, mesh: &ClothMesh, rect: Rect, scale: f32, ox: f32, oy: f32) {
    let w = mesh.width as usize;
    let h = mesh.height as usize;
    let wa = w + 1;
    for row in 0..=h {
        for col in 0..=w {
            let idx = row * wa + col;
            if idx >= mesh.nodes.len() { continue; }
            let node = &mesh.nodes[idx];
            let px = ox + node.pos[0] * scale;
            let py = oy + node.pos[2] * scale;
            let spd = (node.vel[0]*node.vel[0]+node.vel[1]*node.vel[1]+node.vel[2]*node.vel[2]).sqrt().min(5.0);
            if spd > 0.05 {
                let alpha = (spd / 5.0 * 200.0) as u8;
                let vx = ox + (node.pos[0] + node.vel[0] * 0.05) * scale;
                let vy = oy + (node.pos[2] + node.vel[2] * 0.05) * scale;
                painter.line_segment([Pos2::new(px, py), Pos2::new(vx, vy)], Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 200, 80, alpha)));
            }
            let is_pinned = mesh.is_pinned(row as u32, col as u32);
            let r = if is_pinned { 3.0 } else { 1.5 };
            let col_dot = if is_pinned { Color32::YELLOW } else { Color32::from_rgba_premultiplied(100, 200, 255, 120) };
            painter.circle_filled(Pos2::new(px, py), r, col_dot);
        }
    }
}

pub fn draw_cloth_stats(ui: &mut egui::Ui, state: &ClothEditorState) {
    if let Some(si) = state.selected {
        if let Some(mesh) = state.meshes.get(si) {
            let total_nodes = mesh.nodes.len();
            let pinned_count = mesh.pinned.len();
            let total_springs = mesh.structural_springs.len() + mesh.shear_springs.len() + mesh.bend_springs.len();
            let avg_vel: f32 = if total_nodes > 0 {
                mesh.nodes.iter().map(|n| (n.vel[0]*n.vel[0]+n.vel[1]*n.vel[1]+n.vel[2]*n.vel[2]).sqrt()).sum::<f32>() / total_nodes as f32
            } else { 0.0 };
            egui::Grid::new("cloth_stats").num_columns(2).spacing(Vec2::new(8.0, 3.0)).show(ui, |ui| {
                ui.label("Nodes:"); ui.label(RichText::new(format!("{} ({} pinned)", total_nodes, pinned_count)).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Springs:"); ui.label(RichText::new(format!("{} (struct:{} shear:{} bend:{})", total_springs, mesh.structural_springs.len(), mesh.shear_springs.len(), mesh.bend_springs.len())).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Avg velocity:"); ui.label(RichText::new(format!("{:.4} m/s", avg_vel)).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Sim time:"); ui.label(RichText::new(format!("{:.2}s", state.sim_time)).small().color(Color32::GRAY)); ui.end_row();
            });
        }
    }
}

pub fn show_cloth_pin_editor(ui: &mut egui::Ui, state: &mut ClothEditorState) {
    if let Some(si) = state.selected {
        if let Some(mesh) = state.meshes.get_mut(si) {
            egui::CollapsingHeader::new(RichText::new("Pin Editor").color(Color32::from_rgb(255, 220, 100)))
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Pin Top Row").clicked() {
                            for col in 0..=mesh.width { mesh.pinned.insert((0, col)); }
                        }
                        if ui.button("Pin Bottom Row").clicked() {
                            for col in 0..=mesh.width { mesh.pinned.insert((mesh.height, col)); }
                        }
                        if ui.button("Pin Left Col").clicked() {
                            for row in 0..=mesh.height { mesh.pinned.insert((row, 0)); }
                        }
                        if ui.button("Pin Right Col").clicked() {
                            for row in 0..=mesh.height { mesh.pinned.insert((row, mesh.width)); }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Pin Corners").clicked() {
                            mesh.pinned.insert((0, 0));
                            mesh.pinned.insert((0, mesh.width));
                            mesh.pinned.insert((mesh.height, 0));
                            mesh.pinned.insert((mesh.height, mesh.width));
                        }
                        if ui.button("Unpin All").clicked() { mesh.pinned.clear(); }
                        if ui.button("Pin All").clicked() {
                            for r in 0..=mesh.height { for c in 0..=mesh.width { mesh.pinned.insert((r,c)); } }
                        }
                    });
                    ui.label(RichText::new(format!("{} nodes pinned out of {}", mesh.pinned.len(), mesh.nodes.len())).small().color(Color32::GRAY));
                });
        }
    }
}

// ============================================================
// ROPE TENSION VISUALIZATION
// ============================================================

pub fn draw_rope_tension_heatmap(painter: &Painter, rope: &Rope, scale: f32, center: Pos2) {
    let n = rope.nodes.len();
    if n < 2 { return; }
    let to_screen = |p: [f32;2]| Pos2::new(center.x + p[0]*scale, center.y - p[1]*scale);
    for i in 0..n-1 {
        if i >= rope.constraints.len() { break; }
        let c = &rope.constraints[i];
        let dx = rope.nodes[c.b.min(n-1)].pos[0] - rope.nodes[c.a.min(n-1)].pos[0];
        let dy = rope.nodes[c.b.min(n-1)].pos[1] - rope.nodes[c.a.min(n-1)].pos[1];
        let len = (dx*dx+dy*dy).sqrt();
        let strain = ((len - c.rest_length) / c.rest_length.max(1e-6)).abs().min(1.0);
        let r = (255.0 * strain) as u8;
        let g = (200.0 * (1.0 - strain)) as u8;
        let tension_col = Color32::from_rgb(r, g, 40);
        let pa = to_screen(rope.nodes[c.a.min(n-1)].pos);
        let pb = to_screen(rope.nodes[c.b.min(n-1)].pos);
        painter.line_segment([pa, pb], Stroke::new(rope.thickness * (1.0 + strain * 1.5), tension_col));
    }
}

pub fn show_rope_tension_display(ui: &mut egui::Ui, state: &RopeEditorState) {
    egui::CollapsingHeader::new(RichText::new("Tension Analysis").color(Color32::from_rgb(255, 200, 140)))
        .default_open(false)
        .show(ui, |ui| {
            if let Some(ri) = state.selected {
                if let Some(rope) = state.ropes.get(ri) {
                    let mut max_tension = 0.0f32;
                    let mut max_idx = 0;
                    let tensions: Vec<f32> = rope.constraints.iter().map(|c| {
                        let a = c.a.min(rope.nodes.len().saturating_sub(1));
                        let b = c.b.min(rope.nodes.len().saturating_sub(1));
                        if a >= rope.nodes.len() || b >= rope.nodes.len() { return 0.0; }
                        let dx = rope.nodes[b].pos[0]-rope.nodes[a].pos[0];
                        let dy = rope.nodes[b].pos[1]-rope.nodes[a].pos[1];
                        let l = (dx*dx+dy*dy).sqrt();
                        (l - c.rest_length).abs()
                    }).collect();
                    for (i, &t) in tensions.iter().enumerate() {
                        if t > max_tension { max_tension = t; max_idx = i; }
                    }
                    ui.label(RichText::new(format!("Max stretch: {:.4}m at segment #{}", max_tension, max_idx)).small().color(Color32::from_rgb(255, 180, 80)));
                    ui.label(RichText::new(format!("Segments: {}  Total length: {:.3}m", rope.nodes.len()-1, rope.total_length)).small().color(Color32::GRAY));
                    // Mini tension bar graph
                    let desired = Vec2::new(ui.available_width(), 40.0);
                    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 2.0, Color32::from_rgb(14, 14, 22));
                    if !tensions.is_empty() && max_tension > 1e-6 {
                        let bar_w = rect.width() / tensions.len() as f32;
                        for (i, &t) in tensions.iter().enumerate() {
                            let h = (t / max_tension).clamp(0.0, 1.0) * rect.height() * 0.9;
                            let x = rect.left() + i as f32 * bar_w;
                            let y = rect.bottom() - h;
                            let strain_col = Color32::from_rgb((255.0 * t / max_tension) as u8, (200.0 * (1.0 - t / max_tension)) as u8, 40);
                            painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(bar_w * 0.8, h)), 0.0, strain_col);
                        }
                    }
                }
            } else {
                ui.label(RichText::new("Select a rope to analyze tension").small().color(Color32::GRAY));
            }
        });
}

pub fn show_rope_material_comparison(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Material Comparison").color(Color32::from_rgb(200, 220, 255)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("rope_mat_cmp").num_columns(4).spacing(Vec2::new(8.0, 3.0)).show(ui, |ui| {
                ui.label(RichText::new("Material").strong().small());
                ui.label(RichText::new("Damping").strong().small());
                ui.label(RichText::new("Iterations").strong().small());
                ui.label(RichText::new("Color").strong().small());
                ui.end_row();
                for mat in RopeMaterial::all() {
                    ui.label(RichText::new(mat.label()).small().color(mat.color()));
                    ui.label(RichText::new(format!("{:.3}", mat.damping())).small().color(Color32::GRAY));
                    ui.label(RichText::new(format!("{}", mat.stiffness_iters())).small().color(Color32::GRAY));
                    let (cr, _) = ui.allocate_exact_size(Vec2::new(30.0, 10.0), egui::Sense::hover());
                    ui.painter_at(cr).rect_filled(cr, 2.0, mat.color());
                    ui.end_row();
                }
            });
        });
}

// ============================================================
// FLUID SIMULATION ADVANCED UI
// ============================================================

pub fn show_fluid_emitter_panel(ui: &mut egui::Ui, state: &mut FluidEditorState) {
    if let Some(sim) = &mut state.sim {
        egui::CollapsingHeader::new(RichText::new(format!("Emitters ({})", sim.emitters.len())).color(Color32::from_rgb(255, 150, 50)))
            .default_open(true)
            .show(ui, |ui| {
                if ui.button("Add Emitter").clicked() {
                    sim.emitters.push(FluidEmitter::new([0.0, 0.3]));
                }
                let mut to_remove = None;
                for (i, emitter) in sim.emitters.iter_mut().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Emitter #{}", i)).small().color(Color32::from_rgb(255, 150, 50)));
                            ui.checkbox(&mut emitter.enabled, "enabled");
                            if ui.small_button("X").clicked() { to_remove = Some(i); }
                        });
                        egui::Grid::new(format!("em_{}", i)).num_columns(2).spacing(Vec2::new(6.0, 3.0)).show(ui, |ui| {
                            ui.label("Pos X:"); ui.add(egui::DragValue::new(&mut emitter.position[0]).speed(0.01)); ui.end_row();
                            ui.label("Pos Y:"); ui.add(egui::DragValue::new(&mut emitter.position[1]).speed(0.01)); ui.end_row();
                            ui.label("Dir X:"); ui.add(egui::DragValue::new(&mut emitter.direction[0]).speed(0.01).clamp_range(-1.0f32..=1.0)); ui.end_row();
                            ui.label("Dir Y:"); ui.add(egui::DragValue::new(&mut emitter.direction[1]).speed(0.01).clamp_range(-1.0f32..=1.0)); ui.end_row();
                            ui.label("Velocity:"); ui.add(egui::DragValue::new(&mut emitter.velocity).speed(0.05).clamp_range(0.0f32..=20.0)); ui.end_row();
                            ui.label("Rate:"); ui.add(egui::DragValue::new(&mut emitter.rate).speed(0.5).clamp_range(0.1f32..=200.0).suffix("/s")); ui.end_row();
                        });
                    });
                }
                if let Some(idx) = to_remove { sim.emitters.remove(idx); }
            });
    }
}

pub fn show_fluid_boundary_editor(ui: &mut egui::Ui, state: &mut FluidEditorState) {
    if let Some(sim) = &mut state.sim {
        egui::CollapsingHeader::new(RichText::new("Boundary & Parameters").color(Color32::from_rgb(100, 180, 255)))
            .default_open(false)
            .show(ui, |ui| {
                egui::Grid::new("fluid_bound").num_columns(2).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
                    ui.label("Boundary min X:"); ui.add(egui::DragValue::new(&mut sim.params.boundary_min[0]).speed(0.05)); ui.end_row();
                    ui.label("Boundary min Y:"); ui.add(egui::DragValue::new(&mut sim.params.boundary_min[1]).speed(0.05)); ui.end_row();
                    ui.label("Boundary max X:"); ui.add(egui::DragValue::new(&mut sim.params.boundary_max[0]).speed(0.05)); ui.end_row();
                    ui.label("Boundary max Y:"); ui.add(egui::DragValue::new(&mut sim.params.boundary_max[1]).speed(0.05)); ui.end_row();
                    ui.label("Boundary damp:"); ui.add(egui::DragValue::new(&mut sim.params.boundary_damping).speed(0.01).clamp_range(0.0f32..=1.0)); ui.end_row();
                    ui.label("Smoothing H:"); ui.add(egui::DragValue::new(&mut sim.params.smoothing_radius).speed(0.005).clamp_range(0.01f32..=1.0)); ui.end_row();
                    ui.label("Gravity X:"); ui.add(egui::DragValue::new(&mut sim.params.gravity[0]).speed(0.1)); ui.end_row();
                    ui.label("Gravity Y:"); ui.add(egui::DragValue::new(&mut sim.params.gravity[1]).speed(0.1)); ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.button("Earth Gravity").clicked() { sim.params.gravity = [0.0, -9.81]; }
                    if ui.button("Zero G").clicked() { sim.params.gravity = [0.0, 0.0]; }
                    if ui.button("Upward").clicked() { sim.params.gravity = [0.0, 9.81]; }
                    if ui.button("Sideways").clicked() { sim.params.gravity = [9.81, 0.0]; }
                });
            });
    }
}

pub fn show_fluid_particle_count_history(ui: &mut egui::Ui, history: &[usize]) {
    let desired = Vec2::new(ui.available_width(), 50.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgb(14, 14, 22));
    if history.len() < 2 { return; }
    let max_val = *history.iter().max().unwrap_or(&1) as f32;
    let n = history.len().min(rect.width() as usize);
    let start = history.len().saturating_sub(n);
    let mut prev = None;
    for (i, &val) in history[start..].iter().enumerate() {
        let x = rect.left() + i as f32 / n as f32 * rect.width();
        let y = rect.bottom() - (val as f32 / max_val).clamp(0.0, 1.0) * rect.height() * 0.9;
        let p = Pos2::new(x, y);
        if let Some(pp) = prev { painter.line_segment([pp, p], Stroke::new(1.0, Color32::from_rgb(60, 140, 220))); }
        prev = Some(p);
    }
    painter.text(Pos2::new(rect.left()+2.0, rect.top()+2.0), egui::Align2::LEFT_TOP,
        format!("Particles: {}", history.last().unwrap_or(&0)), FontId::monospace(7.0), Color32::GRAY);
}

pub fn draw_fluid_density_field(painter: &Painter, rect: Rect, sim: &FluidSim, grid_size: u32) {
    if sim.particles.is_empty() { return; }
    let bmin = sim.params.boundary_min;
    let bmax = sim.params.boundary_max;
    let bw = bmax[0]-bmin[0]; let bh = bmax[1]-bmin[1];
    let scale = (rect.width()/bw).min(rect.height()/bh)*0.9;
    let ox = rect.center().x-(bmin[0]+bmax[0])*0.5*scale;
    let oy = rect.center().y+(bmin[1]+bmax[1])*0.5*scale;
    let gs = grid_size as f32;
    let cell_w = rect.width() / gs;
    let cell_h = rect.height() / gs;
    for gx in 0..grid_size {
        for gy in 0..grid_size {
            let world_x = bmin[0] + (gx as f32 + 0.5) / gs * bw;
            let world_y = bmin[1] + (gy as f32 + 0.5) / gs * bh;
            let mut density_here = 0.0f32;
            for p in &sim.particles {
                let dx = p.pos[0] - world_x;
                let dy = p.pos[1] - world_y;
                let r = (dx*dx+dy*dy).sqrt();
                if r < sim.params.smoothing_radius {
                    density_here += FluidSim::sph_poly6(r, sim.params.smoothing_radius);
                }
            }
            let t = (density_here / sim.params.rest_density).clamp(0.0, 1.0);
            let screen_x = rect.left() + gx as f32 * cell_w;
            let screen_y = rect.top() + gy as f32 * cell_h;
            let col = Color32::from_rgba_premultiplied((40.0 + t*180.0) as u8, (60.0 + t*100.0) as u8, (200.0 + t*55.0) as u8, (t*150.0) as u8);
            painter.rect_filled(Rect::from_min_size(Pos2::new(screen_x, screen_y), Vec2::new(cell_w, cell_h)), 0.0, col);
        }
    }
}

// ============================================================
// RAGDOLL POSE EDITOR
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RagdollPoseKeyframe {
    pub time: f32,
    pub bone_rotations: Vec<f32>,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RagdollPoseAnimation {
    pub keyframes: Vec<RagdollPoseKeyframe>,
    pub loop_anim: bool,
    pub playback_speed: f32,
    pub current_time: f32,
    pub playing: bool,
}

impl RagdollPoseAnimation {
    pub fn new() -> Self {
        RagdollPoseAnimation { keyframes: Vec::new(), loop_anim: true, playback_speed: 1.0, current_time: 0.0, playing: false }
    }
    pub fn add_keyframe(&mut self, time: f32, bone_count: usize, name: &str) {
        self.keyframes.push(RagdollPoseKeyframe { time, bone_rotations: vec![0.0; bone_count], name: name.into() });
        self.keyframes.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    }
}

pub fn show_ragdoll_pose_editor(ui: &mut egui::Ui, state: &mut RagdollEditorState, anim: &mut RagdollPoseAnimation) {
    egui::CollapsingHeader::new(RichText::new("Pose Animation").color(Color32::from_rgb(255, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(if anim.playing { "Stop" } else { "Play" }).clicked() { anim.playing = !anim.playing; }
                if ui.button("Add Keyframe").clicked() {
                    if let Some(ti) = state.selected_template {
                        if let Some(template) = state.templates.get(ti) {
                            anim.add_keyframe(anim.current_time, template.bones.len(), &format!("KF {}", anim.keyframes.len()+1));
                        }
                    }
                }
                ui.add(egui::Slider::new(&mut anim.playback_speed, 0.1f32..=4.0).text("Speed"));
                ui.checkbox(&mut anim.loop_anim, "Loop");
            });
            if !anim.keyframes.is_empty() {
                let total_time = anim.keyframes.last().map(|k| k.time).unwrap_or(1.0).max(0.1);
                ui.add(egui::Slider::new(&mut anim.current_time, 0.0f32..=total_time).text("Time"));
                // Timeline bar
                let desired = Vec2::new(ui.available_width(), 30.0);
                let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 2.0, Color32::from_rgb(18, 18, 26));
                let t = anim.current_time / total_time;
                painter.rect_filled(Rect::from_min_size(rect.min, Vec2::new(rect.width() * t, rect.height())), 0.0, Color32::from_rgba_premultiplied(60, 120, 200, 60));
                for kf in &anim.keyframes {
                    let kx = rect.left() + (kf.time / total_time) * rect.width();
                    painter.line_segment([Pos2::new(kx, rect.top()), Pos2::new(kx, rect.bottom())], Stroke::new(2.0, Color32::from_rgb(255, 200, 80)));
                    painter.text(Pos2::new(kx, rect.top()+2.0), egui::Align2::CENTER_TOP, &kf.name[..kf.name.len().min(4)], FontId::monospace(6.5), Color32::from_rgb(255, 200, 80));
                }
                let needle = rect.left() + t * rect.width();
                painter.line_segment([Pos2::new(needle, rect.top()), Pos2::new(needle, rect.bottom())], Stroke::new(1.5, Color32::WHITE));
                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        anim.current_time = ((pos.x - rect.left()) / rect.width() * total_time).clamp(0.0, total_time);
                    }
                }
            }
            // Keyframe list
            egui::ScrollArea::vertical().id_salt("anim_kfs").max_height(100.0).show(ui, |ui| {
                let mut to_remove = None;
                for (i, kf) in anim.keyframes.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut kf.time).speed(0.05).suffix("s").clamp_range(0.0f32..=60.0));
                        ui.text_edit_singleline(&mut kf.name);
                        if ui.small_button("Go").clicked() { anim.current_time = kf.time; }
                        if ui.small_button("X").clicked() { to_remove = Some(i); }
                    });
                }
                if let Some(idx) = to_remove { anim.keyframes.remove(idx); }
            });
            if anim.playing {
                let dt = 0.016 * anim.playback_speed;
                anim.current_time += dt;
                let max_t = anim.keyframes.last().map(|k| k.time).unwrap_or(1.0);
                if anim.current_time > max_t {
                    if anim.loop_anim { anim.current_time = 0.0; } else { anim.playing = false; anim.current_time = max_t; }
                }
                ui.ctx().request_repaint();
            }
        });
}

pub fn show_ragdoll_ik_panel(ui: &mut egui::Ui, state: &mut RagdollEditorState) {
    egui::CollapsingHeader::new(RichText::new("Inverse Kinematics").color(Color32::from_rgb(200, 255, 200)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("IK chain setup (FABRIK)").small().color(Color32::GRAY));
            ui.horizontal(|ui| {
                ui.label("Root bone:");
                if let Some(ti) = state.selected_template {
                    if let Some(template) = state.templates.get(ti) {
                        let mut root_idx = state.selected_bone.unwrap_or(0);
                        egui::ComboBox::from_id_salt("ik_root").selected_text(template.bones.get(root_idx).map(|b| b.name.as_str()).unwrap_or("None"))
                            .show_ui(ui, |ui| {
                                for (i, bone) in template.bones.iter().enumerate() {
                                    ui.selectable_value(&mut root_idx, i, &bone.name);
                                }
                            });
                        state.selected_bone = Some(root_idx);
                    }
                }
            });
            ui.label(RichText::new("Target position:").small().color(Color32::GRAY));
            ui.horizontal(|ui| {
                let mut tx = 0.5f32;
                let mut ty = -0.5f32;
                ui.add(egui::DragValue::new(&mut tx).speed(0.01).prefix("x:"));
                ui.add(egui::DragValue::new(&mut ty).speed(0.01).prefix("y:"));
                if ui.button("Solve IK").clicked() {
                    // IK solve would happen here in a full implementation
                    ui.ctx().request_repaint();
                }
            });
            ui.label(RichText::new("IK assists pose editing by iteratively solving bone chain positions toward target.").small().color(Color32::DARK_GRAY));
        });
}

// ============================================================
// CONSTRAINT GRAPH ADDITIONAL TOOLS
// ============================================================

pub fn show_constraint_graph_filters(ui: &mut egui::Ui, cgraph: &mut ConstraintGraphState) {
    egui::CollapsingHeader::new(RichText::new("Graph Filters").color(Color32::from_rgb(180, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Filter by joint type:").small().color(Color32::GRAY));
            ui.horizontal_wrapped(|ui| {
                for joint_name in ["Fixed", "Hinge", "Slider", "Spring", "Distance", "Pulley"] {
                    let _ = ui.selectable_label(true, joint_name);
                }
            });
            ui.separator();
            ui.label(RichText::new("Filter by body type:").small().color(Color32::GRAY));
            ui.horizontal(|ui| {
                for btype in ["Static", "Dynamic", "Kinematic"] {
                    let _ = ui.selectable_label(true, btype);
                }
            });
            ui.separator();
            ui.label(RichText::new("Layout algorithm:").small().color(Color32::GRAY));
            ui.horizontal(|ui| {
                for algo in ["Force-Directed", "Circular", "Tree", "Grid"] {
                    let _ = ui.selectable_label(algo == "Force-Directed", algo);
                }
            });
        });
}

pub fn draw_constraint_graph_legend(painter: &Painter, rect: Rect) {
    let joint_types = [
        ("Fixed",    Color32::from_rgb(150, 150, 150)),
        ("Hinge",    Color32::from_rgb(255, 180, 80)),
        ("Slider",   Color32::from_rgb(100, 200, 255)),
        ("Spring",   Color32::from_rgb(100, 255, 150)),
        ("Distance", Color32::from_rgb(255, 100, 200)),
        ("Pulley",   Color32::from_rgb(200, 100, 255)),
    ];
    let lx = rect.right() - 80.0;
    let mut ly = rect.top() + 6.0;
    for (name, col) in &joint_types {
        painter.line_segment([Pos2::new(lx, ly+5.0), Pos2::new(lx+20.0, ly+5.0)], Stroke::new(2.0, *col));
        painter.text(Pos2::new(lx+22.0, ly+5.0), egui::Align2::LEFT_CENTER, *name, FontId::monospace(7.0), *col);
        ly += 12.0;
    }
}

pub fn show_constraint_graph_stats(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Graph Statistics").color(Color32::from_rgb(200, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let n = editor.bodies.len();
            let e = editor.joints.len();
            let density = if n > 1 { e as f32 / (n * (n-1)) as f32 } else { 0.0 };
            let avg_degree = if n > 0 { e as f32 * 2.0 / n as f32 } else { 0.0 };
            egui::Grid::new("cg_stats").num_columns(2).spacing(Vec2::new(8.0, 3.0)).show(ui, |ui| {
                ui.label("Nodes (bodies):"); ui.label(RichText::new(format!("{}", n)).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Edges (joints):"); ui.label(RichText::new(format!("{}", e)).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Graph density:"); ui.label(RichText::new(format!("{:.4}", density)).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Avg degree:"); ui.label(RichText::new(format!("{:.2}", avg_degree)).small().color(Color32::GRAY)); ui.end_row();
            });
            // Degree distribution mini bar chart
            if n > 0 {
                let mut degrees = vec![0u32; n];
                for joint in &editor.joints {
                    let (a, b) = match joint {
                        Joint::Fixed{body_a,body_b,..}=>(*body_a,*body_b),
                        Joint::Hinge{body_a,body_b,..}=>(*body_a,*body_b),
                        Joint::Slider{body_a,body_b,..}=>(*body_a,*body_b),
                        Joint::Spring{body_a,body_b,..}=>(*body_a,*body_b),
                        Joint::Distance{body_a,body_b,..}=>(*body_a,*body_b),
                        Joint::Pulley{body_a,body_b,..}=>(*body_a,*body_b),
                    };
                    if a < n { degrees[a] += 1; }
                    if b < n { degrees[b] += 1; }
                }
                let max_deg = *degrees.iter().max().unwrap_or(&1) as f32;
                let desired = Vec2::new(ui.available_width(), 40.0);
                let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 2.0, Color32::from_rgb(14, 14, 22));
                let bar_w = rect.width() / n as f32;
                for (i, &deg) in degrees.iter().enumerate() {
                    let h = (deg as f32 / max_deg.max(1.0)) * rect.height() * 0.9;
                    let x = rect.left() + i as f32 * bar_w;
                    let y = rect.bottom() - h;
                    painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(bar_w * 0.8, h)), 0.0, Color32::from_rgb(100, 160, 255));
                }
                painter.text(Pos2::new(rect.left()+2.0, rect.top()+2.0), egui::Align2::LEFT_TOP, "Degree distribution", FontId::monospace(7.0), Color32::GRAY);
            }
        });
}

// ============================================================
// PHYSICS SCENE EXPORT / IMPORT
// ============================================================

pub fn show_scene_export_panel(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Scene Export").color(Color32::from_rgb(200, 255, 200)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Export Options:").small().color(Color32::GRAY));
            ui.horizontal_wrapped(|ui| {
                let _ = ui.selectable_label(true, "JSON");
                let _ = ui.selectable_label(false, "TOML");
                let _ = ui.selectable_label(false, "Binary");
                let _ = ui.selectable_label(false, "Lua Table");
            });
            ui.separator();
            let json_preview = format!(
                r#"{{"bodies":{}, "joints":{}, "gravity":[{:.2},{:.2}]}}"#,
                editor.bodies.len(), editor.joints.len(), editor.gravity[0], editor.gravity[1]
            );
            let mut preview_text = json_preview;
            ui.add(egui::TextEdit::multiline(&mut preview_text).font(egui::TextStyle::Monospace).desired_rows(4).desired_width(f32::INFINITY).interactive(false));
            ui.horizontal(|ui| {
                if ui.button("Copy to Clipboard").clicked() {
                    ui.ctx().copy_text(format!("{{\"bodies\":{},\"joints\":{}}}", editor.bodies.len(), editor.joints.len()));
                }
                if ui.button("Save to File...").clicked() {}
            });
        });
}

pub fn show_scene_import_panel(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Scene Import").color(Color32::from_rgb(255, 220, 180)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Load from:").small().color(Color32::GRAY));
            ui.horizontal(|ui| {
                if ui.button("Open File...").clicked() {}
                if ui.button("Paste from Clipboard").clicked() {}
            });
            ui.separator();
            ui.label(RichText::new("Recent scenes:").small().color(Color32::GRAY));
            for name in &["scene_001.json", "test_ragdoll.json", "bridge_demo.json"] {
                if ui.small_button(*name).clicked() {}
            }
            ui.separator();
            ui.label(RichText::new("Scene templates:").small().color(Color32::GRAY));
            ui.horizontal_wrapped(|ui| {
                for template in &["Empty", "Falling Boxes", "Pendulum Chain", "Bridge", "Domino Run", "Ragdoll Test", "Soft Body Demo", "Fluid Demo"] {
                    if ui.small_button(*template).clicked() {
                        // Would load template
                        let _ = editor.bodies.len();
                    }
                }
            });
        });
}

// ============================================================
// PHYSICS PROFILER
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct PhysicsProfilerFrame {
    pub collision_ms: f32,
    pub integration_ms: f32,
    pub constraint_ms: f32,
    pub broad_phase_ms: f32,
    pub narrow_phase_ms: f32,
    pub total_ms: f32,
}

#[derive(Clone, Debug, Default)]
pub struct PhysicsProfiler {
    pub frames: Vec<PhysicsProfilerFrame>,
    pub max_frames: usize,
    pub enabled: bool,
    pub show_breakdown: bool,
}

impl PhysicsProfiler {
    pub fn new() -> Self {
        PhysicsProfiler { frames: Vec::new(), max_frames: 120, enabled: false, show_breakdown: true }
    }
    pub fn push_frame(&mut self, frame: PhysicsProfilerFrame) {
        if !self.enabled { return; }
        self.frames.push(frame);
        if self.frames.len() > self.max_frames { self.frames.remove(0); }
    }
    pub fn avg_total(&self) -> f32 {
        if self.frames.is_empty() { return 0.0; }
        self.frames.iter().map(|f| f.total_ms).sum::<f32>() / self.frames.len() as f32
    }
    pub fn peak_total(&self) -> f32 {
        self.frames.iter().map(|f| f.total_ms).fold(0.0f32, f32::max)
    }
}

pub fn show_physics_profiler(ui: &mut egui::Ui, profiler: &mut PhysicsProfiler) {
    ui.heading(RichText::new("Physics Profiler").color(Color32::from_rgb(255, 200, 100)));
    ui.separator();
    ui.horizontal(|ui| {
        ui.checkbox(&mut profiler.enabled, "Enable profiling");
        ui.checkbox(&mut profiler.show_breakdown, "Show breakdown");
        if ui.button("Clear").clicked() { profiler.frames.clear(); }
        ui.label(RichText::new(format!("Avg: {:.2}ms  Peak: {:.2}ms  Frames: {}", profiler.avg_total(), profiler.peak_total(), profiler.frames.len())).small().color(Color32::GRAY));
    });
    if !profiler.frames.is_empty() {
        // Stacked bar chart
        let desired = Vec2::new(ui.available_width(), 80.0);
        let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_rgb(14, 14, 22));
        let max_ms = profiler.frames.iter().map(|f| f.total_ms).fold(0.0f32, f32::max).max(1.0);
        let n = profiler.frames.len();
        let bar_w = rect.width() / n as f32;
        for (i, frame) in profiler.frames.iter().enumerate() {
            let x = rect.left() + i as f32 * bar_w;
            let scale = rect.height() / max_ms;
            // Stacked bars
            let mut y = rect.bottom();
            let parts = [
                (frame.broad_phase_ms,  Color32::from_rgb(80, 80, 200)),
                (frame.narrow_phase_ms, Color32::from_rgb(150, 80, 200)),
                (frame.collision_ms,    Color32::from_rgb(200, 80, 80)),
                (frame.constraint_ms,   Color32::from_rgb(200, 180, 80)),
                (frame.integration_ms,  Color32::from_rgb(80, 200, 80)),
            ];
            for (ms, col) in &parts {
                let h = ms * scale;
                painter.rect_filled(Rect::from_min_size(Pos2::new(x, y-h), Vec2::new(bar_w*0.85, h)), 0.0, *col);
                y -= h;
            }
        }
        // Legend
        let legend = [("Broad", Color32::from_rgb(80,80,200)),("Narrow",Color32::from_rgb(150,80,200)),("Coll",Color32::from_rgb(200,80,80)),("Constr",Color32::from_rgb(200,180,80)),("Integ",Color32::from_rgb(80,200,80))];
        let mut lx = rect.left() + 2.0;
        for (name, col) in &legend {
            painter.rect_filled(Rect::from_min_size(Pos2::new(lx, rect.top()+2.0), Vec2::new(8.0, 6.0)), 0.0, *col);
            painter.text(Pos2::new(lx+10.0, rect.top()+5.0), egui::Align2::LEFT_CENTER, *name, FontId::monospace(6.5), *col);
            lx += 50.0;
        }
        // Target line (16ms = 60fps)
        let target_y = rect.bottom() - 16.0 * (rect.height() / max_ms);
        painter.line_segment([Pos2::new(rect.left(), target_y), Pos2::new(rect.right(), target_y)], Stroke::new(0.5, Color32::from_rgba_premultiplied(255, 80, 80, 120)));
        painter.text(Pos2::new(rect.right()-2.0, target_y), egui::Align2::RIGHT_BOTTOM, "60fps", FontId::monospace(7.0), Color32::from_rgb(255, 80, 80));
    }
    // Per-category breakdown
    if profiler.show_breakdown && !profiler.frames.is_empty() {
        ui.separator();
        let avg_f = &PhysicsProfilerFrame {
            collision_ms: profiler.frames.iter().map(|f| f.collision_ms).sum::<f32>() / profiler.frames.len() as f32,
            integration_ms: profiler.frames.iter().map(|f| f.integration_ms).sum::<f32>() / profiler.frames.len() as f32,
            constraint_ms: profiler.frames.iter().map(|f| f.constraint_ms).sum::<f32>() / profiler.frames.len() as f32,
            broad_phase_ms: profiler.frames.iter().map(|f| f.broad_phase_ms).sum::<f32>() / profiler.frames.len() as f32,
            narrow_phase_ms: profiler.frames.iter().map(|f| f.narrow_phase_ms).sum::<f32>() / profiler.frames.len() as f32,
            total_ms: profiler.avg_total(),
        };
        let breakdown = [
            ("Broad Phase", avg_f.broad_phase_ms, Color32::from_rgb(80,80,200)),
            ("Narrow Phase", avg_f.narrow_phase_ms, Color32::from_rgb(150,80,200)),
            ("Collision", avg_f.collision_ms, Color32::from_rgb(200,80,80)),
            ("Constraints", avg_f.constraint_ms, Color32::from_rgb(200,180,80)),
            ("Integration", avg_f.integration_ms, Color32::from_rgb(80,200,80)),
        ];
        egui::Grid::new("prof_bd").num_columns(3).spacing(Vec2::new(8.0, 2.0)).show(ui, |ui| {
            for (name, ms, col) in &breakdown {
                ui.label(RichText::new(*name).small().color(*col));
                ui.label(RichText::new(format!("{:.3}ms", ms)).small().color(Color32::GRAY));
                let pct = if avg_f.total_ms > 0.0 { ms / avg_f.total_ms * 100.0 } else { 0.0 };
                ui.label(RichText::new(format!("{:.1}%", pct)).small().color(*col));
                ui.end_row();
            }
            ui.label(RichText::new("TOTAL").small().strong().color(Color32::WHITE));
            ui.label(RichText::new(format!("{:.3}ms", avg_f.total_ms)).small().strong().color(Color32::WHITE));
            ui.label(RichText::new("100%").small().color(Color32::WHITE));
            ui.end_row();
        });
    }
}

// ============================================================
// PHYSICS GRAVITY FIELD EDITOR
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GravityField {
    pub field_type: GravityFieldType,
    pub strength: f32,
    pub position: [f32; 2],
    pub radius: f32,
    pub enabled: bool,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GravityFieldType {
    Uniform { direction: [f32; 2] },
    Radial { attract: bool },
    Vortex { clockwise: bool },
    Repulsor,
    Wind { direction: [f32; 2], turbulence: f32 },
}

impl GravityFieldType {
    pub fn label(&self) -> &'static str {
        match self {
            GravityFieldType::Uniform { .. } => "Uniform",
            GravityFieldType::Radial { .. } => "Radial",
            GravityFieldType::Vortex { .. } => "Vortex",
            GravityFieldType::Repulsor => "Repulsor",
            GravityFieldType::Wind { .. } => "Wind",
        }
    }
}

impl GravityField {
    pub fn new_uniform(name: &str, direction: [f32; 2], strength: f32) -> Self {
        GravityField { field_type: GravityFieldType::Uniform { direction }, strength, position: [0.0, 0.0], radius: f32::INFINITY, enabled: true, name: name.into() }
    }
    pub fn new_radial(name: &str, pos: [f32; 2], radius: f32, strength: f32, attract: bool) -> Self {
        GravityField { field_type: GravityFieldType::Radial { attract }, strength, position: pos, radius, enabled: true, name: name.into() }
    }
    pub fn force_at(&self, pos: [f32; 2]) -> [f32; 2] {
        if !self.enabled { return [0.0, 0.0]; }
        match &self.field_type {
            GravityFieldType::Uniform { direction } => {
                [direction[0] * self.strength, direction[1] * self.strength]
            }
            GravityFieldType::Radial { attract } => {
                let dx = self.position[0] - pos[0];
                let dy = self.position[1] - pos[1];
                let dist = (dx*dx+dy*dy).sqrt().max(0.1);
                if dist > self.radius { return [0.0, 0.0]; }
                let mag = self.strength / (dist * dist).max(0.01);
                let sign = if *attract { 1.0 } else { -1.0 };
                [dx / dist * mag * sign, dy / dist * mag * sign]
            }
            GravityFieldType::Vortex { clockwise } => {
                let dx = pos[0] - self.position[0];
                let dy = pos[1] - self.position[1];
                let dist = (dx*dx+dy*dy).sqrt().max(0.01);
                if dist > self.radius { return [0.0, 0.0]; }
                let tang_x = if *clockwise { dy } else { -dy };
                let tang_y = if *clockwise { -dx } else { dx };
                let mag = self.strength / dist;
                [tang_x / dist * mag, tang_y / dist * mag]
            }
            GravityFieldType::Repulsor => {
                let dx = pos[0] - self.position[0];
                let dy = pos[1] - self.position[1];
                let dist = (dx*dx+dy*dy).sqrt().max(0.1);
                if dist > self.radius { return [0.0, 0.0]; }
                let mag = self.strength / (dist * dist).max(0.01);
                [dx / dist * mag, dy / dist * mag]
            }
            GravityFieldType::Wind { direction, turbulence } => {
                let turb = (*turbulence * (pos[0] * 1.7 + pos[1] * 2.3).sin()) * 0.1;
                [(direction[0] + turb) * self.strength, (direction[1] + turb * 0.5) * self.strength]
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GravityFieldEditorState {
    pub fields: Vec<GravityField>,
    pub selected: Option<usize>,
    pub show_field_viz: bool,
    pub viz_resolution: u32,
}

impl GravityFieldEditorState {
    pub fn new() -> Self {
        let mut s = GravityFieldEditorState { fields: Vec::new(), selected: None, show_field_viz: true, viz_resolution: 16 };
        s.fields.push(GravityField::new_uniform("Earth Gravity", [0.0, -1.0], 9.81));
        s
    }
}

pub fn show_gravity_field_editor(ui: &mut egui::Ui, state: &mut GravityFieldEditorState) {
    ui.heading(RichText::new("Gravity Field Editor").color(Color32::from_rgb(200, 180, 255)));
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Add Uniform").clicked() { state.fields.push(GravityField::new_uniform("Gravity", [0.0,-1.0], 9.81)); state.selected=Some(state.fields.len()-1); }
        if ui.button("Add Attractor").clicked() { state.fields.push(GravityField::new_radial("Attractor", [0.0,0.0], 2.0, 5.0, true)); state.selected=Some(state.fields.len()-1); }
        if ui.button("Add Vortex").clicked() { state.fields.push(GravityField { field_type:GravityFieldType::Vortex{clockwise:true}, strength:3.0, position:[0.0,0.0], radius:2.0, enabled:true, name:"Vortex".into() }); state.selected=Some(state.fields.len()-1); }
        if ui.button("Add Wind").clicked() { state.fields.push(GravityField { field_type:GravityFieldType::Wind{direction:[1.0,0.0],turbulence:0.2}, strength:2.0, position:[0.0,0.0], radius:f32::INFINITY, enabled:true, name:"Wind".into() }); state.selected=Some(state.fields.len()-1); }
    });
    ui.separator();
    // Field list
    let mut to_remove = None;
    for (i, field) in state.fields.iter().enumerate() {
        ui.horizontal(|ui| {
            let sel = state.selected == Some(i);
            let col = if field.enabled { Color32::from_rgb(200, 180, 255) } else { Color32::DARK_GRAY };
            if ui.selectable_label(sel, RichText::new(&field.name).color(col)).clicked() { state.selected = Some(i); }
            ui.label(RichText::new(format!("{} str:{:.2}", field.field_type.label(), field.strength)).small().color(Color32::GRAY));
            if ui.small_button(if field.enabled { "ON" } else { "OFF" }).clicked() {
                if let Some(f) = state.fields.get_mut(i) { f.enabled = !f.enabled; }
            }
            if ui.small_button("X").clicked() { to_remove = Some(i); }
        });
    }
    if let Some(idx) = to_remove { state.fields.remove(idx); if state.selected == Some(idx) { state.selected = None; } }
    // Selected field editor
    if let Some(si) = state.selected {
        if let Some(field) = state.fields.get_mut(si) {
            ui.separator();
            egui::Grid::new("gf_ed").num_columns(2).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
                ui.label("Name:"); ui.text_edit_singleline(&mut field.name); ui.end_row();
                ui.label("Strength:"); ui.add(egui::DragValue::new(&mut field.strength).speed(0.05).clamp_range(0.0f32..=200.0)); ui.end_row();
                if !field.radius.is_infinite() {
                    ui.label("Radius:"); ui.add(egui::DragValue::new(&mut field.radius).speed(0.05).clamp_range(0.1f32..=50.0)); ui.end_row();
                    ui.label("Pos X:"); ui.add(egui::DragValue::new(&mut field.position[0]).speed(0.05)); ui.end_row();
                    ui.label("Pos Y:"); ui.add(egui::DragValue::new(&mut field.position[1]).speed(0.05)); ui.end_row();
                }
            });
            match &mut field.field_type {
                GravityFieldType::Uniform { direction } => {
                    ui.horizontal(|ui| {
                        ui.label("Direction:");
                        ui.add(egui::DragValue::new(&mut direction[0]).speed(0.01).prefix("x:").clamp_range(-1.0f32..=1.0));
                        ui.add(egui::DragValue::new(&mut direction[1]).speed(0.01).prefix("y:").clamp_range(-1.0f32..=1.0));
                    });
                }
                GravityFieldType::Radial { attract } => {
                    ui.checkbox(attract, "Attract (vs repel)");
                }
                GravityFieldType::Vortex { clockwise } => {
                    ui.checkbox(clockwise, "Clockwise rotation");
                }
                GravityFieldType::Wind { direction, turbulence } => {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut direction[0]).speed(0.01).prefix("x:").clamp_range(-1.0f32..=1.0));
                        ui.add(egui::DragValue::new(&mut direction[1]).speed(0.01).prefix("y:").clamp_range(-1.0f32..=1.0));
                        ui.add(egui::DragValue::new(turbulence).speed(0.01).prefix("turb:").clamp_range(0.0f32..=1.0));
                    });
                }
                _ => {}
            }
        }
    }
    // Visualization
    ui.checkbox(&mut state.show_field_viz, "Show field visualization");
    if state.show_field_viz {
        let desired = Vec2::new(ui.available_width(), 260.0);
        let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 12, 18));
        let res = state.viz_resolution as usize;
        let extent = 3.0f32;
        for row in 0..res {
            for col in 0..res {
                let wx = (col as f32 / (res-1) as f32 * 2.0 - 1.0) * extent;
                let wy = (row as f32 / (res-1) as f32 * 2.0 - 1.0) * extent;
                let mut fx = 0.0f32; let mut fy = 0.0f32;
                for field in &state.fields {
                    let [dfx, dfy] = field.force_at([wx, wy]);
                    fx += dfx; fy += dfy;
                }
                let mag = (fx*fx+fy*fy).sqrt().min(20.0);
                if mag < 0.01 { continue; }
                let nx = fx / (mag + 1e-8); let ny = fy / (mag + 1e-8);
                let arrow_len = (mag / 20.0 * 12.0).max(3.0).min(12.0);
                let scale = rect.width() / (extent * 2.0);
                let px = rect.center().x + wx * scale;
                let py = rect.center().y - wy * scale;
                let ep = Pos2::new(px + nx * arrow_len, py - ny * arrow_len);
                let sp = Pos2::new(px, py);
                let alpha = (mag / 20.0 * 200.0).clamp(40.0, 200.0) as u8;
                let col = Color32::from_rgba_premultiplied((100.0 + mag/20.0*155.0) as u8, (80.0 + mag/20.0*80.0) as u8, (200.0 - mag/20.0*100.0) as u8, alpha);
                painter.arrow(sp, ep - sp, Stroke::new(1.0, col));
            }
        }
        // Draw field sources
        for field in &state.fields {
            if !field.enabled { continue; }
            if !field.radius.is_infinite() {
                let scale = rect.width() / (extent * 2.0);
                let px = rect.center().x + field.position[0] * scale;
                let py = rect.center().y - field.position[1] * scale;
                let sr = (field.radius * scale).min(150.0);
                painter.circle_stroke(Pos2::new(px, py), sr, Stroke::new(1.0, Color32::from_rgba_premultiplied(200, 180, 255, 80)));
                painter.circle_filled(Pos2::new(px, py), 4.0, Color32::from_rgb(200, 180, 255));
            }
        }
        painter.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP,
            format!("{} active fields", state.fields.iter().filter(|f| f.enabled).count()), FontId::monospace(9.0), Color32::GRAY);
    }
}

// ============================================================
// BODY PROPERTY BATCH EDITOR
// ============================================================

pub fn show_batch_body_editor(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Batch Body Editor").color(Color32::from_rgb(255, 220, 180)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Apply to all dynamic bodies:").small().color(Color32::GRAY));
            ui.separator();
            let mut apply_mass = false;
            let mut apply_restitution = false;
            let mut apply_friction = false;
            let mut apply_damping = false;
            let mut mass_val = 1.0f32;
            let mut rest_val = 0.3f32;
            let mut fric_val = 0.5f32;
            let mut damp_val = 0.01f32;
            egui::Grid::new("batch_ed").num_columns(3).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
                ui.label("Mass:"); ui.add(egui::DragValue::new(&mut mass_val).speed(0.05).clamp_range(0.001f32..=1000.0)); ui.checkbox(&mut apply_mass, "Apply"); ui.end_row();
                ui.label("Restitution:"); ui.add(egui::Slider::new(&mut rest_val, 0.0f32..=1.0)); ui.checkbox(&mut apply_restitution, "Apply"); ui.end_row();
                ui.label("Friction:"); ui.add(egui::Slider::new(&mut fric_val, 0.0f32..=2.0)); ui.checkbox(&mut apply_friction, "Apply"); ui.end_row();
                ui.label("Lin Damp:"); ui.add(egui::DragValue::new(&mut damp_val).speed(0.001).clamp_range(0.0f32..=1.0)); ui.checkbox(&mut apply_damping, "Apply"); ui.end_row();
            });
            if ui.button("Apply Selected Changes").clicked() {
                for body in &mut editor.bodies {
                    if matches!(body.body_type, BodyType::Dynamic) {
                        if apply_mass { body.mass = mass_val; }
                        if apply_restitution { body.material.restitution = rest_val; }
                        if apply_friction { body.material.friction = fric_val; }
                        if apply_damping { body.linear_damping = damp_val; }
                    }
                }
            }
            ui.separator();
            ui.label(RichText::new("Randomize:").small().color(Color32::GRAY));
            ui.horizontal(|ui| {
                if ui.button("Randomize Colors").clicked() {
                    for body in &mut editor.bodies {
                        use std::hash::{Hash, Hasher};
                        let mut h = std::collections::hash_map::DefaultHasher::new();
                        body.name.hash(&mut h);
                        let hash = h.finish();
                        body.color = [((hash >> 16) as u8).max(60), ((hash >> 8) as u8).max(60), (hash as u8).max(60)];
                    }
                }
                if ui.button("Randomize Velocities").clicked() {
                    for (i, body) in editor.bodies.iter_mut().enumerate() {
                        if matches!(body.body_type, BodyType::Dynamic) {
                            let t = i as f32 * 1.618;
                            body.velocity = [t.sin() * 2.0, t.cos() * 2.0];
                        }
                    }
                }
                if ui.button("Zero All Velocities").clicked() {
                    editor.apply_impulse_to_all_dynamic([0.0, 0.0]);
                    for body in &mut editor.bodies { body.velocity = [0.0, 0.0]; body.angular_velocity = 0.0; }
                }
            });
        });
}

// ============================================================
// PHYSICS MATERIAL EDITOR (INLINE)
// ============================================================

pub fn show_inline_material_editor(ui: &mut egui::Ui, material: &mut PhysicsMaterial, id: &str) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            let [r, g, b] = material.color;
            let swatch = Color32::from_rgb(r, g, b);
            let (crect, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
            ui.painter_at(crect).rect_filled(crect, 2.0, swatch);
            ui.text_edit_singleline(&mut material.name);
        });
        egui::Grid::new(format!("mat_ed_{}", id)).num_columns(2).spacing(Vec2::new(8.0, 3.0)).show(ui, |ui| {
            ui.label("Restitution:");
            ui.add(egui::Slider::new(&mut material.restitution, 0.0f32..=1.0));
            ui.end_row();
            ui.label("Friction:");
            ui.add(egui::Slider::new(&mut material.friction, 0.0f32..=2.0));
            ui.end_row();
            ui.label("Combine Mode:");
            egui::ComboBox::from_id_salt(format!("cm_{}", id))
                .selected_text(material.combine_mode.label())
                .show_ui(ui, |ui| {
                    for cm in CombineMode::all() {
                        ui.selectable_value(&mut material.combine_mode, cm.clone(), cm.label());
                    }
                });
            ui.end_row();
        });
        // Restitution vs friction visual
        let desired = Vec2::new(ui.available_width().min(200.0), 80.0);
        let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 2.0, Color32::from_rgb(14, 14, 22));
        let m = 15.0f32;
        let pr = Rect::from_min_max(Pos2::new(rect.left()+m, rect.top()+5.0), Pos2::new(rect.right()-5.0, rect.bottom()-m));
        painter.line_segment([Pos2::new(pr.left(), pr.bottom()), Pos2::new(pr.right(), pr.bottom())], Stroke::new(1.0, Color32::from_rgb(60,60,80)));
        painter.line_segment([Pos2::new(pr.left(), pr.top()), Pos2::new(pr.left(), pr.bottom())], Stroke::new(1.0, Color32::from_rgb(60,60,80)));
        let fx = (material.friction / 2.0).clamp(0.0, 1.0);
        let fy = material.restitution.clamp(0.0, 1.0);
        let px = pr.left() + fx * pr.width();
        let py = pr.bottom() - fy * pr.height();
        painter.circle_filled(Pos2::new(px, py), 6.0, Color32::from_rgb(r as u8, g as u8, b as u8));
        painter.circle_stroke(Pos2::new(px, py), 6.0, Stroke::new(1.0, Color32::WHITE));
        painter.text(Pos2::new(pr.center().x, rect.bottom()-2.0), egui::Align2::CENTER_BOTTOM, "friction", FontId::monospace(6.0), Color32::GRAY);
        painter.text(Pos2::new(rect.left()+2.0, pr.center().y), egui::Align2::LEFT_CENTER, "rest.", FontId::monospace(6.0), Color32::GRAY);
        let [r, g, b] = material.color;
        let _ = (r, g, b);
    });
}

// ============================================================
// PHYSICS JOINT TEMPLATES
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JointTemplate {
    pub name: String,
    pub joint_type_name: String,
    pub description: String,
    pub params: JointTemplateParams,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JointTemplateParams {
    pub lower_limit: f32,
    pub upper_limit: f32,
    pub motor_speed: f32,
    pub max_torque: f32,
    pub stiffness: f32,
    pub damping: f32,
    pub length: f32,
}

impl Default for JointTemplateParams {
    fn default() -> Self {
        JointTemplateParams { lower_limit: -1.0, upper_limit: 1.0, motor_speed: 0.0, max_torque: 100.0, stiffness: 100.0, damping: 10.0, length: 1.0 }
    }
}

pub fn all_joint_templates() -> Vec<JointTemplate> {
    vec![
        JointTemplate { name: "Door Hinge".into(), joint_type_name: "Hinge".into(), description: "Swings -90° to +90°".into(), params: JointTemplateParams { lower_limit: -1.57, upper_limit: 1.57, motor_speed: 0.0, max_torque: 500.0, ..Default::default() } },
        JointTemplate { name: "Wheel Motor".into(), joint_type_name: "Hinge".into(), description: "Free spinning motor".into(), params: JointTemplateParams { lower_limit: -f32::INFINITY, upper_limit: f32::INFINITY, motor_speed: 5.0, max_torque: 200.0, ..Default::default() } },
        JointTemplate { name: "Piston".into(), joint_type_name: "Slider".into(), description: "Linear actuator".into(), params: JointTemplateParams { lower_limit: 0.0, upper_limit: 2.0, motor_speed: 1.0, max_torque: 1000.0, ..Default::default() } },
        JointTemplate { name: "Bungee".into(), joint_type_name: "Spring".into(), description: "Elastic spring link".into(), params: JointTemplateParams { stiffness: 30.0, damping: 2.0, length: 1.0, ..Default::default() } },
        JointTemplate { name: "Stiff Spring".into(), joint_type_name: "Spring".into(), description: "High stiffness spring".into(), params: JointTemplateParams { stiffness: 500.0, damping: 20.0, length: 0.5, ..Default::default() } },
        JointTemplate { name: "Shoulder Joint".into(), joint_type_name: "Hinge".into(), description: "Shoulder rotation limit".into(), params: JointTemplateParams { lower_limit: -2.0, upper_limit: 2.0, ..Default::default() } },
        JointTemplate { name: "Knee Joint".into(), joint_type_name: "Hinge".into(), description: "One-directional bend".into(), params: JointTemplateParams { lower_limit: 0.0, upper_limit: 2.5, ..Default::default() } },
        JointTemplate { name: "Rope Link".into(), joint_type_name: "Distance".into(), description: "Max distance constraint".into(), params: JointTemplateParams { length: 0.5, ..Default::default() } },
    ]
}

pub fn show_joint_template_picker(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Joint Templates").color(Color32::from_rgb(200, 220, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let templates = all_joint_templates();
            egui::Grid::new("jt_tbl").num_columns(3).spacing(Vec2::new(8.0, 3.0)).show(ui, |ui| {
                ui.label(RichText::new("Name").strong().small());
                ui.label(RichText::new("Type").strong().small());
                ui.label(RichText::new("Description").strong().small());
                ui.end_row();
                for tmpl in &templates {
                    let type_col = match tmpl.joint_type_name.as_str() {
                        "Hinge" => Color32::from_rgb(255, 180, 80),
                        "Slider" => Color32::from_rgb(100, 200, 255),
                        "Spring" => Color32::from_rgb(100, 255, 150),
                        "Distance" => Color32::from_rgb(255, 100, 200),
                        _ => Color32::GRAY,
                    };
                    if ui.small_button(&tmpl.name).clicked() {}
                    ui.label(RichText::new(&tmpl.joint_type_name).small().color(type_col));
                    ui.label(RichText::new(&tmpl.description).small().color(Color32::GRAY));
                    ui.end_row();
                }
            });
        });
}

// ============================================================
// PHYSICS SIMULATION PARAMETERS PANEL
// ============================================================

pub fn show_simulation_parameters(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Simulation Parameters").color(Color32::from_rgb(180, 220, 255)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("sim_params").num_columns(2).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
                ui.label("Gravity X:"); ui.add(egui::DragValue::new(&mut editor.gravity[0]).speed(0.05).suffix(" m/s²")); ui.end_row();
                ui.label("Gravity Y:"); ui.add(egui::DragValue::new(&mut editor.gravity[1]).speed(0.05).suffix(" m/s²")); ui.end_row();
                ui.label("Time scale:"); ui.add(egui::Slider::new(&mut editor.time_scale, 0.0f32..=4.0)); ui.end_row();
                ui.label("Sub-steps:"); ui.add(egui::DragValue::new(&mut editor.sub_steps).speed(1).clamp_range(1u32..=20)); ui.end_row();
                ui.label("Sleep threshold:"); ui.add(egui::DragValue::new(&mut editor.sleep_threshold).speed(0.001).clamp_range(0.0f32..=1.0)); ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.button("Earth").clicked() { editor.gravity = [0.0, -9.81]; }
                if ui.button("Moon").clicked() { editor.gravity = [0.0, -1.62]; }
                if ui.button("Mars").clicked() { editor.gravity = [0.0, -3.72]; }
                if ui.button("Jupiter").clicked() { editor.gravity = [0.0, -24.79]; }
                if ui.button("Zero G").clicked() { editor.gravity = [0.0, 0.0]; }
            });
            ui.separator();
            // Gravity direction widget
            let desired = Vec2::new(80.0, 80.0);
            let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.circle_stroke(rect.center(), 35.0, Stroke::new(1.0, Color32::from_rgb(60, 60, 80)));
            let gx = editor.gravity[0]; let gy = editor.gravity[1];
            let gmag = (gx*gx+gy*gy).sqrt().max(0.01);
            let gscale = (gmag / 30.0).min(1.0) * 28.0 + 4.0;
            let gnx = gx / gmag; let gny = -gy / gmag;
            let tip = Pos2::new(rect.center().x + gnx * gscale, rect.center().y + gny * gscale);
            painter.arrow(rect.center(), tip - rect.center(), Stroke::new(2.0, Color32::from_rgb(255, 200, 80)));
            painter.text(Pos2::new(rect.center().x, rect.bottom() - 2.0), egui::Align2::CENTER_BOTTOM,
                format!("{:.2} m/s²", gmag), FontId::monospace(7.0), Color32::GRAY);
            if resp.dragged() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let dx = pos.x - rect.center().x;
                    let dy = -(pos.y - rect.center().y);
                    let d = (dx*dx+dy*dy).sqrt().max(0.01);
                    let scale = 30.0 / 35.0;
                    editor.gravity[0] = dx / d * d.min(35.0) * scale;
                    editor.gravity[1] = dy / d * d.min(35.0) * scale;
                }
            }
        });
}

// ============================================================
// COLLIDER VISUALIZATION HELPERS
// ============================================================

pub fn draw_collider_shape_preview(ui: &mut egui::Ui, collider: &Collider) {
    let desired = Vec2::new(100.0, 70.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, Color32::from_rgb(18, 18, 26));
    let center = rect.center();
    let scale = 28.0f32;
    let col = Color32::from_rgb(100, 180, 255);
    let fill = Color32::from_rgba_premultiplied(100, 180, 255, 40);
    match collider {
        Collider::Circle { radius } => {
            let r = (radius * scale).max(5.0).min(32.0);
            painter.circle_filled(center, r, fill);
            painter.circle_stroke(center, r, Stroke::new(1.5, col));
            painter.text(Pos2::new(center.x, rect.bottom()-2.0), egui::Align2::CENTER_BOTTOM, format!("r={:.2}", radius), FontId::monospace(7.0), Color32::GRAY);
        }
        Collider::Box { width, height } => {
            let hw = (width * scale * 0.5).max(4.0).min(40.0);
            let hh = (height * scale * 0.5).max(4.0).min(32.0);
            let r = Rect::from_center_size(center, Vec2::new(hw*2.0, hh*2.0));
            painter.rect_filled(r, 0.0, fill);
            painter.rect_stroke(r, 0.0, Stroke::new(1.5, col));
            painter.text(Pos2::new(center.x, rect.bottom()-2.0), egui::Align2::CENTER_BOTTOM, format!("{:.1}x{:.1}", width, height), FontId::monospace(7.0), Color32::GRAY);
        }
        Collider::Capsule { radius, height } => {
            let r = (radius * scale).max(4.0).min(20.0);
            let h = (height * scale * 0.5).max(4.0).min(24.0);
            painter.line_segment([Pos2::new(center.x-r, center.y-h), Pos2::new(center.x-r, center.y+h)], Stroke::new(1.5, col));
            painter.line_segment([Pos2::new(center.x+r, center.y-h), Pos2::new(center.x+r, center.y+h)], Stroke::new(1.5, col));
            painter.circle_stroke(Pos2::new(center.x, center.y-h), r, Stroke::new(1.5, col));
            painter.circle_stroke(Pos2::new(center.x, center.y+h), r, Stroke::new(1.5, col));
            painter.text(Pos2::new(center.x, rect.bottom()-2.0), egui::Align2::CENTER_BOTTOM, format!("r={:.1} h={:.1}", radius, height), FontId::monospace(7.0), Color32::GRAY);
        }
        Collider::Polygon { points } => {
            if points.len() >= 3 {
                let pts: Vec<Pos2> = points.iter().map(|p| Pos2::new(center.x + p[0]*scale*0.6, center.y - p[1]*scale*0.6)).collect();
                painter.add(Shape::convex_polygon(pts.clone(), fill, Stroke::new(1.5, col)));
            }
            painter.text(Pos2::new(center.x, rect.bottom()-2.0), egui::Align2::CENTER_BOTTOM, format!("{} pts", points.len()), FontId::monospace(7.0), Color32::GRAY);
        }
        Collider::Compound { colliders } => {
            painter.text(center, egui::Align2::CENTER_CENTER, format!("{} sub", colliders.len()), FontId::monospace(9.0), col);
        }
    }
    painter.text(Pos2::new(rect.left()+2.0, rect.top()+2.0), egui::Align2::LEFT_TOP, collider.label(), FontId::monospace(8.0), col);
}

pub fn show_collider_editor_panel(ui: &mut egui::Ui, collider: &mut Collider) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Collider Type:").small().color(Color32::GRAY));
            for type_name in ["Circle", "Box", "Capsule", "Polygon"] {
                let is_sel = collider.label() == type_name;
                if ui.selectable_label(is_sel, type_name).clicked() && !is_sel {
                    *collider = match type_name {
                        "Circle" => Collider::Circle { radius: 0.5 },
                        "Box" => Collider::Box { width: 1.0, height: 1.0 },
                        "Capsule" => Collider::Capsule { radius: 0.25, height: 0.5 },
                        "Polygon" => Collider::Polygon { points: vec![[0.0,0.5],[-0.5,-0.5],[0.5,-0.5]] },
                        _ => Collider::Circle { radius: 0.5 },
                    };
                }
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                match collider {
                    Collider::Circle { radius } => {
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            ui.add(egui::DragValue::new(radius).speed(0.01).clamp_range(0.01f32..=20.0).suffix("m"));
                        });
                    }
                    Collider::Box { width, height } => {
                        ui.horizontal(|ui| {
                            ui.label("Width:");
                            ui.add(egui::DragValue::new(width).speed(0.01).clamp_range(0.01f32..=20.0).suffix("m"));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Height:");
                            ui.add(egui::DragValue::new(height).speed(0.01).clamp_range(0.01f32..=20.0).suffix("m"));
                        });
                    }
                    Collider::Capsule { radius, height } => {
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            ui.add(egui::DragValue::new(radius).speed(0.01).clamp_range(0.01f32..=10.0).suffix("m"));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Half-height:");
                            ui.add(egui::DragValue::new(height).speed(0.01).clamp_range(0.01f32..=20.0).suffix("m"));
                        });
                    }
                    Collider::Polygon { points } => {
                        ui.label(RichText::new(format!("{} vertices", points.len())).small().color(Color32::GRAY));
                        if ui.small_button("Add vertex").clicked() { points.push([0.0, 0.0]); }
                        let mut to_remove = None;
                        egui::ScrollArea::vertical().id_salt("poly_pts").max_height(100.0).show(ui, |ui| {
                            for (i, pt) in points.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add(egui::DragValue::new(&mut pt[0]).speed(0.01).prefix("x:"));
                                    ui.add(egui::DragValue::new(&mut pt[1]).speed(0.01).prefix("y:"));
                                    if ui.small_button("X").clicked() { to_remove = Some(i); }
                                });
                            }
                        });
                        if let Some(idx) = to_remove { if points.len() > 3 { points.remove(idx); } }
                    }
                    Collider::Compound { .. } => { ui.label(RichText::new("Compound shape").small().color(Color32::GRAY)); }
                }
            });
            ui.separator();
            draw_collider_shape_preview(ui, collider);
        });
    });
}

// ============================================================
// LAYER MANAGER
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhysicsLayer {
    pub name: String,
    pub color: [u8; 3],
    pub visible: bool,
    pub locked: bool,
    pub body_count: u32,
}

impl PhysicsLayer {
    pub fn new(name: &str, color: [u8; 3]) -> Self {
        PhysicsLayer { name: name.into(), color, visible: true, locked: false, body_count: 0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LayerManager {
    pub layers: Vec<PhysicsLayer>,
    pub active_layer: usize,
}

impl LayerManager {
    pub fn default_layers() -> Self {
        LayerManager {
            layers: vec![
                PhysicsLayer::new("Default",    [120, 120, 200]),
                PhysicsLayer::new("Terrain",    [120, 80,  40]),
                PhysicsLayer::new("Dynamic",    [80,  160, 255]),
                PhysicsLayer::new("Triggers",   [255, 100, 200]),
                PhysicsLayer::new("Ragdolls",   [255, 200, 100]),
                PhysicsLayer::new("Projectiles",[255, 80,  80]),
                PhysicsLayer::new("Sensors",    [100, 255, 150]),
                PhysicsLayer::new("Debug",      [200, 200, 200]),
            ],
            active_layer: 0,
        }
    }
}

pub fn show_layer_manager(ui: &mut egui::Ui, layers: &mut LayerManager) {
    ui.heading(RichText::new("Layer Manager").color(Color32::from_rgb(200, 200, 255)));
    ui.separator();
    if ui.button("Add Layer").clicked() {
        let n = layers.layers.len();
        layers.layers.push(PhysicsLayer::new(&format!("Layer {}", n+1), [150, 150, 200]));
    }
    let mut to_remove = None;
    egui::ScrollArea::vertical().id_salt("layer_mgr").max_height(200.0).show(ui, |ui| {
        for (i, layer) in layers.layers.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let sel = layers.active_layer == i;
                let [r,g,b] = layer.color;
                let col = Color32::from_rgb(r,g,b);
                let (swatch, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                ui.painter_at(swatch).rect_filled(swatch, 2.0, col);
                if ui.selectable_label(sel, RichText::new(&layer.name).color(if layer.visible{col}else{Color32::DARK_GRAY})).clicked() {
                    layers.active_layer = i;
                }
                ui.label(RichText::new(format!("({})", layer.body_count)).small().color(Color32::GRAY));
                let eye = if layer.visible { "O" } else { "-" };
                if ui.small_button(eye).clicked() { layer.visible = !layer.visible; }
                let lock = if layer.locked { "L" } else { "U" };
                if ui.small_button(lock).clicked() { layer.locked = !layer.locked; }
                if ui.small_button("X").clicked() && layers.layers.len() > 1 { to_remove = Some(i); }
            });
        }
    });
    if let Some(idx) = to_remove {
        layers.layers.remove(idx);
        if layers.active_layer >= layers.layers.len() { layers.active_layer = layers.layers.len().saturating_sub(1); }
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Show All").clicked() { for l in &mut layers.layers { l.visible = true; } }
        if ui.button("Hide All").clicked() { for l in &mut layers.layers { l.visible = false; } }
        if ui.button("Unlock All").clicked() { for l in &mut layers.layers { l.locked = false; } }
    });
    if let Some(layer) = layers.layers.get_mut(layers.active_layer) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Active layer:");
            ui.label(RichText::new(&layer.name).color(Color32::from_rgb(layer.color[0], layer.color[1], layer.color[2])));
        });
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut layer.name);
        });
    }
}

// ============================================================
// PHYSICS SCENE STATISTICS PANEL
// ============================================================

pub fn show_scene_statistics(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Scene Statistics").color(Color32::from_rgb(180, 200, 255)))
        .default_open(true)
        .show(ui, |ui| {
            let (static_c, dynamic_c, kinematic_c) = editor.body_count_by_type();
            let total_mass = editor.total_mass();
            let com = editor.center_of_mass();
            let total_ke: f32 = editor.bodies.iter().map(|b| {
                let v2 = b.velocity[0]*b.velocity[0]+b.velocity[1]*b.velocity[1];
                0.5 * b.mass * v2
            }).sum();
            let total_momentum: [f32; 2] = editor.bodies.iter().fold([0.0, 0.0], |acc, b| {
                [acc[0] + b.mass*b.velocity[0], acc[1] + b.mass*b.velocity[1]]
            });
            egui::Grid::new("scene_stats").num_columns(2).spacing(Vec2::new(12.0, 3.0)).show(ui, |ui| {
                ui.label("Bodies (total):"); ui.label(RichText::new(format!("{}", editor.bodies.len())).small().color(Color32::from_rgb(200, 220, 255))); ui.end_row();
                ui.label("  Static:"); ui.label(RichText::new(format!("{}", static_c)).small().color(BodyType::Static.color())); ui.end_row();
                ui.label("  Dynamic:"); ui.label(RichText::new(format!("{}", dynamic_c)).small().color(BodyType::Dynamic.color())); ui.end_row();
                ui.label("  Kinematic:"); ui.label(RichText::new(format!("{}", kinematic_c)).small().color(BodyType::Kinematic.color())); ui.end_row();
                ui.label("Joints:"); ui.label(RichText::new(format!("{}", editor.joints.len())).small().color(Color32::from_rgb(255, 200, 100))); ui.end_row();
                ui.label("Total mass:"); ui.label(RichText::new(format!("{:.3} kg", total_mass)).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Center of mass:"); ui.label(RichText::new(format!("({:.2}, {:.2})", com[0], com[1])).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Kinetic energy:"); ui.label(RichText::new(format!("{:.4} J", total_ke)).small().color(Color32::from_rgb(255, 220, 80))); ui.end_row();
                ui.label("Momentum:"); ui.label(RichText::new(format!("({:.3}, {:.3}) kg·m/s", total_momentum[0], total_momentum[1])).small().color(Color32::GRAY)); ui.end_row();
                ui.label("Simulating:"); ui.label(RichText::new(if editor.simulating { "YES" } else { "NO" }).small().color(if editor.simulating { Color32::from_rgb(80, 220, 80) } else { Color32::GRAY })); ui.end_row();
            });
            // Mini energy histogram
            if !editor.bodies.is_empty() {
                let desired = Vec2::new(ui.available_width(), 40.0);
                let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 2.0, Color32::from_rgb(14, 14, 22));
                let n = editor.bodies.len();
                let kes: Vec<f32> = editor.bodies.iter().map(|b| {
                    0.5 * b.mass * (b.velocity[0]*b.velocity[0]+b.velocity[1]*b.velocity[1])
                }).collect();
                let max_ke = kes.iter().cloned().fold(0.0f32, f32::max).max(0.001);
                let bar_w = rect.width() / n as f32;
                for (i, &ke) in kes.iter().enumerate() {
                    let h = (ke / max_ke).clamp(0.0, 1.0) * rect.height() * 0.9;
                    let x = rect.left() + i as f32 * bar_w;
                    let bc = editor.bodies[i].body_type.color();
                    painter.rect_filled(Rect::from_min_size(Pos2::new(x, rect.bottom()-h), Vec2::new(bar_w*0.8, h)), 0.0, bc);
                }
                painter.text(Pos2::new(rect.left()+2.0, rect.top()+2.0), egui::Align2::LEFT_TOP, "KE per body", FontId::monospace(7.0), Color32::GRAY);
            }
        });
}
"""

with open(OUTFILE, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

lines = sum(1 for _ in open(OUTFILE, encoding="utf-8"))
print(f"physics_editor.rs now has {lines} lines")
