import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 8
// ============================================================

// ─── Torque and Force Inspector ───────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ForceInspectorState {
    pub show_gravity: bool,
    pub show_contact_forces: bool,
    pub show_joint_forces: bool,
    pub show_applied_forces: bool,
    pub force_scale: f32,
    pub torque_scale: f32,
    pub min_magnitude_threshold: f32,
    pub selected_body: Option<usize>,
    pub forces_snapshot: Vec<(String, [f32; 2], f32)>,
}

pub fn show_force_inspector(ui: &mut egui::Ui, state: &mut ForceInspectorState) {
    ui.heading("Force & Torque Inspector");
    ui.separator();

    if state.force_scale == 0.0 { state.force_scale = 0.01; state.torque_scale = 0.1; state.min_magnitude_threshold = 0.01; state.show_gravity = true; state.show_contact_forces = true; state.show_joint_forces = true; state.show_applied_forces = true; }

    if state.forces_snapshot.is_empty() {
        state.forces_snapshot = vec![
            ("Gravity".into(), [0.0, -9810.0 * 1.0], 0.0),
            ("Contact_0".into(), [1240.3, 0.0], 15.2),
            ("Contact_1".into(), [-832.1, 450.0], -8.3),
            ("Joint_0".into(), [2104.6, -380.2], 42.7),
            ("Applied".into(), [500.0, 250.0], 5.0),
        ];
    }

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_gravity, "Gravity");
        ui.checkbox(&mut state.show_contact_forces, "Contact");
        ui.checkbox(&mut state.show_joint_forces, "Joints");
        ui.checkbox(&mut state.show_applied_forces, "Applied");
    });
    ui.horizontal(|ui| {
        ui.label("Force Scale:");
        ui.add(egui::Slider::new(&mut state.force_scale, 0.001..=0.1).logarithmic(true));
        ui.label("Torque Scale:");
        ui.add(egui::Slider::new(&mut state.torque_scale, 0.01..=1.0).logarithmic(true));
    });
    ui.horizontal(|ui| {
        ui.label("Min Mag Threshold:");
        ui.add(egui::DragValue::new(&mut state.min_magnitude_threshold).speed(0.001).clamp_range(0.0..=100.0));
    });

    ui.separator();
    egui::Grid::new("force_table").num_columns(4).spacing([12.0, 2.0]).striped(true).show(ui, |ui| {
        ui.label("Name"); ui.label("Fx"); ui.label("Fy"); ui.label("Torque"); ui.end_row();
        for (name, force, torque) in &state.forces_snapshot {
            let mag = (force[0]*force[0]+force[1]*force[1]).sqrt();
            if mag < state.min_magnitude_threshold && torque.abs() < state.min_magnitude_threshold { continue; }
            ui.label(name);
            ui.label(egui::RichText::new(format!("{:.2}", force[0])).monospace().small());
            ui.label(egui::RichText::new(format!("{:.2}", force[1])).monospace().small());
            ui.label(egui::RichText::new(format!("{:.2}", torque)).monospace().small());
            ui.end_row();
        }
    });

    draw_force_diagram(ui, &state.forces_snapshot, state.force_scale);
}

fn draw_force_diagram(ui: &mut egui::Ui, forces: &[(String, [f32; 2], f32)], scale: f32) {
    let desired = Vec2::new(ui.available_width().min(300.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(10, 12, 18));
    let c = rect.center();

    // Body
    p.rect_stroke(Rect::from_center_size(c, Vec2::splat(24.0)), 2.0, Stroke::new(1.5, Color32::from_rgb(120,140,180)));

    let colors = [Color32::from_rgb(80,180,220), Color32::from_rgb(220,120,60), Color32::from_rgb(80,220,120), Color32::from_rgb(220,80,180), Color32::from_rgb(220,220,60)];
    for (i, (name, force, _)) in forces.iter().enumerate() {
        let col = colors[i % colors.len()];
        let fx = force[0] * scale * 0.05;
        let fy = -force[1] * scale * 0.05;
        let end = c + Vec2::new(fx.clamp(-60.0, 60.0), fy.clamp(-60.0, 60.0));
        p.line_segment([c, end], Stroke::new(1.5, col));
        let dir = (end - c).normalized();
        let perp = Vec2::new(-dir.y, dir.x);
        p.line_segment([end, end - dir * 5.0 + perp * 3.0], Stroke::new(1.5, col));
        p.line_segment([end, end - dir * 5.0 - perp * 3.0], Stroke::new(1.5, col));
        p.text(end + dir * 6.0, egui::Align2::CENTER_CENTER, name, FontId::monospace(6.5), col);
    }
}

// ─── Spatial Query Tools ──────────────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SpatialQueryState {
    pub query_type: SpatialQueryType,
    pub query_pos: [f32; 2],
    pub query_dir: [f32; 2],
    pub query_radius: f32,
    pub query_half_extents: [f32; 2],
    pub max_hits: u32,
    pub hit_results: Vec<SpatialQueryHit>,
    pub filter_mask: u32,
    pub exclude_sensors: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SpatialQueryType {
    #[default]
    RayCast,
    CircleOverlap,
    BoxOverlap,
    PointQuery,
    ShapeCast,
}

impl SpatialQueryType {
    pub fn label(&self) -> &str {
        match self {
            SpatialQueryType::RayCast => "Ray Cast",
            SpatialQueryType::CircleOverlap => "Circle Overlap",
            SpatialQueryType::BoxOverlap => "Box Overlap",
            SpatialQueryType::PointQuery => "Point Query",
            SpatialQueryType::ShapeCast => "Shape Cast",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpatialQueryHit {
    pub body_id: usize,
    pub body_name: String,
    pub hit_point: [f32; 2],
    pub hit_normal: [f32; 2],
    pub distance: f32,
    pub fraction: f32,
}

pub fn show_spatial_query_tool(ui: &mut egui::Ui, state: &mut SpatialQueryState) {
    ui.heading("Spatial Query Tool");
    ui.separator();

    if state.query_radius == 0.0 { state.query_radius = 2.0; state.query_half_extents = [1.0, 1.0]; state.max_hits = 10; state.query_dir = [1.0, 0.0]; state.filter_mask = 0xFFFF; }

    ui.horizontal(|ui| {
        for qt in &[SpatialQueryType::RayCast, SpatialQueryType::CircleOverlap, SpatialQueryType::BoxOverlap, SpatialQueryType::PointQuery, SpatialQueryType::ShapeCast] {
            if ui.selectable_label(state.query_type == *qt, qt.label()).clicked() { state.query_type = qt.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Query Origin:");
        ui.add(egui::DragValue::new(&mut state.query_pos[0]).prefix("x:").speed(0.05));
        ui.add(egui::DragValue::new(&mut state.query_pos[1]).prefix("y:").speed(0.05));
    });

    match state.query_type {
        SpatialQueryType::RayCast | SpatialQueryType::ShapeCast => {
            ui.horizontal(|ui| {
                ui.label("Direction:");
                ui.add(egui::DragValue::new(&mut state.query_dir[0]).prefix("dx:").speed(0.01));
                ui.add(egui::DragValue::new(&mut state.query_dir[1]).prefix("dy:").speed(0.01));
            });
        }
        SpatialQueryType::CircleOverlap => {
            ui.horizontal(|ui| { ui.label("Radius:"); ui.add(egui::DragValue::new(&mut state.query_radius).speed(0.05).clamp_range(0.01..=100.0)); });
        }
        SpatialQueryType::BoxOverlap => {
            ui.horizontal(|ui| {
                ui.label("Half Extents:");
                ui.add(egui::DragValue::new(&mut state.query_half_extents[0]).prefix("w:").speed(0.05));
                ui.add(egui::DragValue::new(&mut state.query_half_extents[1]).prefix("h:").speed(0.05));
            });
        }
        _ => {}
    }

    ui.horizontal(|ui| {
        ui.label("Max Hits:"); ui.add(egui::DragValue::new(&mut state.max_hits).clamp_range(1..=100));
        ui.label("Filter Mask:"); ui.add(egui::DragValue::new(&mut state.filter_mask).hexadecimal(4, false, true));
        ui.checkbox(&mut state.exclude_sensors, "Exclude Sensors");
    });

    if ui.button("Execute Query").clicked() {
        state.hit_results = vec![
            SpatialQueryHit { body_id: 1, body_name: "Box_1".into(), hit_point: [1.2, 0.5], hit_normal: [-1.0, 0.0], distance: 1.2, fraction: 0.12 },
            SpatialQueryHit { body_id: 3, body_name: "Ground".into(), hit_point: [2.8, -0.1], hit_normal: [0.0, 1.0], distance: 2.8, fraction: 0.28 },
        ];
    }

    if !state.hit_results.is_empty() {
        ui.separator();
        ui.label(format!("Hits: {}", state.hit_results.len()));
        egui::Grid::new("query_hits").num_columns(5).spacing([8.0, 2.0]).show(ui, |ui| {
            ui.label("Body"); ui.label("Hit Point"); ui.label("Normal"); ui.label("Distance"); ui.label("Fraction"); ui.end_row();
            for hit in &state.hit_results {
                ui.label(format!("#{} {}", hit.body_id, hit.body_name));
                ui.label(format!("({:.2},{:.2})", hit.hit_point[0], hit.hit_point[1]));
                ui.label(format!("({:.2},{:.2})", hit.hit_normal[0], hit.hit_normal[1]));
                ui.label(format!("{:.3}", hit.distance));
                ui.label(format!("{:.3}", hit.fraction));
                ui.end_row();
            }
        });
    }
}

// ─── Gravity Well Visualizer ──────────────────────────────────────────────────

pub fn draw_gravity_field_visualization(ui: &mut egui::Ui, gravity: [f32; 2], field_type: &str) {
    let desired = Vec2::new(ui.available_width().min(400.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(8, 10, 16));

    let gx = gravity[0];
    let gy = gravity[1];
    let g_len = (gx*gx+gy*gy).sqrt().max(0.001);

    let rows = 8; let cols = 12;
    for row in 0..rows {
        for col in 0..cols {
            let x = rect.left() + (col as f32 + 0.5) / cols as f32 * rect.width();
            let y = rect.top() + (row as f32 + 0.5) / rows as f32 * rect.height();
            let base = Pos2::new(x, y);

            let (arrow_dx, arrow_dy) = match field_type {
                "radial" => {
                    let cx = rect.center().x; let cy = rect.center().y;
                    let dx = x - cx; let dy = y - cy;
                    let d = (dx*dx+dy*dy).sqrt().max(0.001);
                    (-dx/d * 12.0, -dy/d * 12.0)
                },
                "vortex" => {
                    let cx = rect.center().x; let cy = rect.center().y;
                    let dx = x - cx; let dy = y - cy;
                    let d = (dx*dx+dy*dy).sqrt().max(0.001);
                    (-dy/d * 12.0, dx/d * 12.0)
                },
                _ => (gx / g_len * 12.0, -gy / g_len * 12.0),
            };
            let end = base + Vec2::new(arrow_dx, arrow_dy);
            let alpha = 120u8;
            p.line_segment([base, end], Stroke::new(1.0, Color32::from_rgba_unmultiplied(80, 160, 255, alpha)));
        }
    }

    p.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP, format!("{} | g=({:.2},{:.2})", field_type, gravity[0], gravity[1]), FontId::monospace(8.0), Color32::GRAY);
}

// ─── Physics Configuration Presets ───────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicsConfig {
    pub name: String,
    pub gravity: [f32; 2],
    pub time_step: f32,
    pub velocity_iterations: u32,
    pub position_iterations: u32,
    pub sleep_enabled: bool,
    pub ccd_enabled: bool,
    pub broadphase: String,
    pub description: String,
}

pub fn all_physics_configs() -> Vec<PhysicsConfig> {
    vec![
        PhysicsConfig { name: "Earth Standard".into(), gravity: [0.0, -9.81], time_step: 0.0167, velocity_iterations: 8, position_iterations: 3, sleep_enabled: true, ccd_enabled: false, broadphase: "DBVT".into(), description: "Standard Earth gravity, 60fps, good for most games.".into() },
        PhysicsConfig { name: "Moon Physics".into(), gravity: [0.0, -1.62], time_step: 0.0167, velocity_iterations: 6, position_iterations: 2, sleep_enabled: true, ccd_enabled: false, broadphase: "DBVT".into(), description: "Moon gravity for space game scenarios.".into() },
        PhysicsConfig { name: "High Accuracy".into(), gravity: [0.0, -9.81], time_step: 0.004, velocity_iterations: 20, position_iterations: 8, sleep_enabled: true, ccd_enabled: true, broadphase: "DBVT".into(), description: "High precision at 250fps substeps, expensive.".into() },
        PhysicsConfig { name: "Arcade Physics".into(), gravity: [0.0, -20.0], time_step: 0.0167, velocity_iterations: 4, position_iterations: 2, sleep_enabled: false, ccd_enabled: false, broadphase: "SpatialHash".into(), description: "Exaggerated gravity for arcade feel.".into() },
        PhysicsConfig { name: "Zero Gravity".into(), gravity: [0.0, 0.0], time_step: 0.0167, velocity_iterations: 6, position_iterations: 2, sleep_enabled: true, ccd_enabled: false, broadphase: "DBVT".into(), description: "Space/underwater zero-gravity simulation.".into() },
        PhysicsConfig { name: "Side Scroller".into(), gravity: [0.0, -15.0], time_step: 0.0167, velocity_iterations: 8, position_iterations: 3, sleep_enabled: true, ccd_enabled: false, broadphase: "SpatialHash".into(), description: "Tuned for 2D side-scrolling platformers.".into() },
        PhysicsConfig { name: "Top Down".into(), gravity: [0.0, 0.0], time_step: 0.0167, velocity_iterations: 6, position_iterations: 2, sleep_enabled: true, ccd_enabled: false, broadphase: "SpatialHash".into(), description: "Top-down perspective, no gravity.".into() },
        PhysicsConfig { name: "Ragdoll Mode".into(), gravity: [0.0, -9.81], time_step: 0.008, velocity_iterations: 15, position_iterations: 5, sleep_enabled: true, ccd_enabled: false, broadphase: "DBVT".into(), description: "Optimized for ragdoll simulation with many joints.".into() },
        PhysicsConfig { name: "Fluid Mode".into(), gravity: [0.0, -9.81], time_step: 0.004, velocity_iterations: 1, position_iterations: 0, sleep_enabled: false, ccd_enabled: false, broadphase: "SpatialHash".into(), description: "Optimized substeps for SPH fluid simulation.".into() },
        PhysicsConfig { name: "Soft Body Mode".into(), gravity: [0.0, -9.81], time_step: 0.008, velocity_iterations: 10, position_iterations: 4, sleep_enabled: false, ccd_enabled: false, broadphase: "DBVT".into(), description: "Tuned for cloth/soft body simulations.".into() },
    ]
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsConfigBrowserState {
    pub configs: Vec<PhysicsConfig>,
    pub selected: Option<usize>,
    pub active_config: usize,
}

pub fn show_physics_config_browser(ui: &mut egui::Ui, state: &mut PhysicsConfigBrowserState) {
    if state.configs.is_empty() { state.configs = all_physics_configs(); }

    ui.heading("Physics Configuration Presets");
    ui.separator();

    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
        for (i, cfg) in state.configs.iter().enumerate() {
            let sel = state.selected == Some(i);
            let is_active = state.active_config == i;
            ui.horizontal(|ui| {
                if is_active { ui.colored_label(Color32::GREEN, "✓"); } else { ui.label(" "); }
                if ui.selectable_label(sel, &cfg.name).clicked() { state.selected = Some(i); }
                ui.label(egui::RichText::new(&cfg.description).small().color(Color32::GRAY));
            });
        }
    });

    if let Some(idx) = state.selected {
        if idx < state.configs.len() {
            let cfg = &state.configs[idx];
            ui.separator();
            egui::Grid::new("cfg_details").num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
                ui.label("Gravity"); ui.label(format!("({:.2}, {:.2})", cfg.gravity[0], cfg.gravity[1])); ui.end_row();
                ui.label("Time Step"); ui.label(format!("{:.4}s ({:.0}fps)", cfg.time_step, 1.0/cfg.time_step)); ui.end_row();
                ui.label("Vel Iters"); ui.label(format!("{}", cfg.velocity_iterations)); ui.end_row();
                ui.label("Pos Iters"); ui.label(format!("{}", cfg.position_iterations)); ui.end_row();
                ui.label("Sleep"); ui.label(format!("{}", cfg.sleep_enabled)); ui.end_row();
                ui.label("CCD"); ui.label(format!("{}", cfg.ccd_enabled)); ui.end_row();
                ui.label("Broadphase"); ui.label(&cfg.broadphase); ui.end_row();
            });
            if ui.button("Apply Configuration").clicked() {
                state.active_config = idx;
            }
        }
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
