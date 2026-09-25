import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 10 (Final)
// ============================================================

// ─── Fluid Splash Effect ─────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct SplashEffect {
    pub position: [f32; 2],
    pub velocity_threshold: f32,
    pub splash_radius: f32,
    pub droplet_count: u32,
    pub droplet_speed: f32,
    pub sound_name: String,
    pub particle_effect: String,
    pub enabled: bool,
}

impl Default for SplashEffect {
    fn default() -> Self {
        Self { position: [0.0, 0.0], velocity_threshold: 3.0, splash_radius: 1.5, droplet_count: 8, droplet_speed: 6.0, sound_name: "splash_medium".into(), particle_effect: "fx_water_splash".into(), enabled: true }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SplashEffectEditorState {
    pub effects: Vec<SplashEffect>,
    pub selected: Option<usize>,
}

pub fn show_splash_effect_editor(ui: &mut egui::Ui, state: &mut SplashEffectEditorState) {
    ui.heading("Fluid Splash Effects");
    ui.separator();

    if state.effects.is_empty() { state.effects.push(SplashEffect::default()); }

    ui.horizontal(|ui| {
        if ui.button("+ Splash Zone").clicked() { state.effects.push(SplashEffect::default()); }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.effects.remove(s); state.selected = None; } }
    });

    for (i, e) in state.effects.iter().enumerate() {
        let sel = state.selected == Some(i);
        let col = if e.enabled { Color32::from_rgb(60, 160, 220) } else { Color32::DARK_GRAY };
        ui.horizontal(|ui| {
            ui.colored_label(col, "≈");
            if ui.selectable_label(sel, format!("Splash @ ({:.1},{:.1}) — thr:{:.1}", e.position[0], e.position[1], e.velocity_threshold)).clicked() { state.selected = Some(i); }
        });
    }

    if let Some(idx) = state.selected {
        if idx < state.effects.len() {
            let e = &mut state.effects[idx];
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Position:");
                ui.add(egui::DragValue::new(&mut e.position[0]).prefix("x:").speed(0.05));
                ui.add(egui::DragValue::new(&mut e.position[1]).prefix("y:").speed(0.05));
            });
            ui.horizontal(|ui| {
                ui.label("Vel. Threshold:");
                ui.add(egui::DragValue::new(&mut e.velocity_threshold).speed(0.1).clamp_range(0.1..=50.0).suffix(" m/s"));
                ui.label("Splash Radius:");
                ui.add(egui::DragValue::new(&mut e.splash_radius).speed(0.05).clamp_range(0.1..=20.0));
            });
            ui.horizontal(|ui| {
                ui.label("Droplets:");
                ui.add(egui::DragValue::new(&mut e.droplet_count).clamp_range(1..=100));
                ui.label("Speed:");
                ui.add(egui::DragValue::new(&mut e.droplet_speed).speed(0.1).clamp_range(0.1..=50.0));
            });
            ui.horizontal(|ui| { ui.label("Sound:"); ui.text_edit_singleline(&mut e.sound_name); });
            ui.horizontal(|ui| { ui.label("Particle FX:"); ui.text_edit_singleline(&mut e.particle_effect); });
            ui.checkbox(&mut e.enabled, "Enabled");
        }
    }
}

// ─── Debris Spawn Config ─────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct DebrisSpawnConfig {
    pub name: String,
    pub piece_count: u32,
    pub min_size: f32,
    pub max_size: f32,
    pub min_velocity: f32,
    pub max_velocity: f32,
    pub angular_velocity_range: [f32; 2],
    pub lifetime: f32,
    pub fade_out: bool,
    pub collide_with_world: bool,
    pub material: String,
    pub shapes: Vec<String>,
}

impl Default for DebrisSpawnConfig {
    fn default() -> Self {
        Self { name: "Generic Debris".into(), piece_count: 12, min_size: 0.05, max_size: 0.25, min_velocity: 2.0, max_velocity: 15.0, angular_velocity_range: [-10.0, 10.0], lifetime: 5.0, fade_out: true, collide_with_world: true, material: "Default".into(), shapes: vec!["Box".into(), "Triangle".into()] }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct DebrisSystemState {
    pub configs: Vec<DebrisSpawnConfig>,
    pub selected: Option<usize>,
}

pub fn show_debris_system_editor(ui: &mut egui::Ui, state: &mut DebrisSystemState) {
    ui.heading("Debris Spawn System");
    ui.separator();

    if state.configs.is_empty() {
        state.configs = vec![
            DebrisSpawnConfig::default(),
            DebrisSpawnConfig { name: "Wood Splinters".into(), piece_count: 20, min_size: 0.02, max_size: 0.15, min_velocity: 3.0, max_velocity: 20.0, angular_velocity_range: [-15.0, 15.0], lifetime: 8.0, fade_out: true, collide_with_world: true, material: "Wood".into(), shapes: vec!["Capsule".into(), "Box".into()] },
            DebrisSpawnConfig { name: "Glass Shards".into(), piece_count: 30, min_size: 0.01, max_size: 0.1, min_velocity: 5.0, max_velocity: 30.0, angular_velocity_range: [-25.0, 25.0], lifetime: 4.0, fade_out: true, collide_with_world: false, material: "Glass".into(), shapes: vec!["Triangle".into()] },
        ];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Config").clicked() { state.configs.push(DebrisSpawnConfig::default()); }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.configs.remove(s); state.selected = None; } }
    });

    for (i, c) in state.configs.iter().enumerate() {
        if ui.selectable_label(state.selected == Some(i), format!("{} ({} pcs)", c.name, c.piece_count)).clicked() { state.selected = Some(i); }
    }

    if let Some(idx) = state.selected {
        if idx < state.configs.len() {
            let c = &mut state.configs[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut c.name); });
            ui.horizontal(|ui| {
                ui.label("Pieces:");
                ui.add(egui::DragValue::new(&mut c.piece_count).clamp_range(1..=500));
                ui.label("Lifetime:");
                ui.add(egui::DragValue::new(&mut c.lifetime).speed(0.1).clamp_range(0.1..=60.0).suffix("s"));
            });
            ui.horizontal(|ui| {
                ui.label("Size:");
                ui.add(egui::DragValue::new(&mut c.min_size).prefix("min:").speed(0.005).clamp_range(0.001..=10.0));
                ui.add(egui::DragValue::new(&mut c.max_size).prefix("max:").speed(0.005).clamp_range(0.001..=10.0));
            });
            ui.horizontal(|ui| {
                ui.label("Velocity:");
                ui.add(egui::DragValue::new(&mut c.min_velocity).prefix("min:").speed(0.1));
                ui.add(egui::DragValue::new(&mut c.max_velocity).prefix("max:").speed(0.1));
            });
            ui.horizontal(|ui| {
                ui.label("Ang. Vel:");
                ui.add(egui::DragValue::new(&mut c.angular_velocity_range[0]).prefix("min:").speed(0.1));
                ui.add(egui::DragValue::new(&mut c.angular_velocity_range[1]).prefix("max:").speed(0.1));
            });
            ui.horizontal(|ui| { ui.label("Material:"); ui.text_edit_singleline(&mut c.material); });
            ui.horizontal(|ui| {
                ui.checkbox(&mut c.fade_out, "Fade Out");
                ui.checkbox(&mut c.collide_with_world, "World Collision");
            });
            if ui.button("Preview Spawn").clicked() {
                // Would spawn debris in preview
            }
        }
    }
}

// ─── Physics Performance Metrics Dashboard ────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PerformanceMetricsDashboard {
    pub broadphase_ms: f32,
    pub narrowphase_ms: f32,
    pub solver_ms: f32,
    pub integration_ms: f32,
    pub total_ms: f32,
    pub bodies_simulated: usize,
    pub sleeping_bodies: usize,
    pub active_contacts: usize,
    pub joint_count: usize,
    pub memory_kb: f32,
    pub history: Vec<f32>,
    pub history_cap: usize,
}

pub fn show_performance_metrics_dashboard(ui: &mut egui::Ui, state: &mut PerformanceMetricsDashboard) {
    ui.heading("Performance Metrics");
    ui.separator();

    if state.history_cap == 0 { state.history_cap = 120; }

    // Simulate some demo data
    if state.total_ms == 0.0 {
        state.broadphase_ms = 0.32;
        state.narrowphase_ms = 0.85;
        state.solver_ms = 2.14;
        state.integration_ms = 0.28;
        state.total_ms = state.broadphase_ms + state.narrowphase_ms + state.solver_ms + state.integration_ms;
        state.bodies_simulated = 87;
        state.sleeping_bodies = 23;
        state.active_contacts = 42;
        state.joint_count = 31;
        state.memory_kb = 284.5;
    }

    let fps_equiv = if state.total_ms > 0.0 { 1000.0 / state.total_ms } else { 9999.0 };

    ui.horizontal(|ui| {
        let fps_col = if fps_equiv >= 60.0 { Color32::GREEN } else if fps_equiv >= 30.0 { Color32::YELLOW } else { Color32::RED };
        ui.colored_label(fps_col, egui::RichText::new(format!("{:.0} FPS equiv.", fps_equiv)).strong());
        ui.separator();
        ui.label(format!("Total: {:.2}ms", state.total_ms));
    });

    egui::Grid::new("perf_grid").num_columns(4).spacing([12.0, 2.0]).show(ui, |ui| {
        ui.label("Phase"); ui.label("Time (ms)"); ui.label("% of Total"); ui.label("Bar"); ui.end_row();
        let phases = [
            ("Broadphase", state.broadphase_ms, Color32::from_rgb(80, 180, 255)),
            ("Narrowphase", state.narrowphase_ms, Color32::from_rgb(255, 160, 60)),
            ("Solver", state.solver_ms, Color32::from_rgb(60, 220, 120)),
            ("Integration", state.integration_ms, Color32::from_rgb(220, 100, 255)),
        ];
        let total = state.total_ms.max(0.001);
        for (name, ms, col) in &phases {
            let pct = ms / total * 100.0;
            ui.label(*name);
            ui.label(format!("{:.3}", ms));
            ui.label(format!("{:.1}%", pct));
            let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(80.0, 10.0), egui::Sense::hover());
            let p = ui.painter_at(bar_rect);
            p.rect_filled(bar_rect, 1.0, Color32::from_rgb(30,35,45));
            p.rect_filled(Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width() * (pct/100.0).clamp(0.0,1.0), bar_rect.height())), 1.0, *col);
            ui.end_row();
        }
    });

    ui.separator();
    ui.columns(3, |cols| {
        cols[0].label(format!("Active Bodies: {}", state.bodies_simulated - state.sleeping_bodies));
        cols[0].label(format!("Sleeping Bodies: {}", state.sleeping_bodies));
        cols[1].label(format!("Active Contacts: {}", state.active_contacts));
        cols[1].label(format!("Joints: {}", state.joint_count));
        cols[2].label(format!("Memory: {:.1} KB", state.memory_kb));
        cols[2].label(format!("Total Bodies: {}", state.bodies_simulated));
    });

    // Frametime graph
    let desired = Vec2::new(ui.available_width().min(500.0), 60.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(10, 12, 18));

    // Add fake history for demo
    if state.history.is_empty() {
        for i in 0..60 { state.history.push(2.5 + (i as f32 * 0.15).sin() * 0.8); }
    }

    let max_t = state.history.iter().cloned().fold(0.0f32, f32::max).max(1.0);
    let n = state.history.len();
    if n > 1 {
        for i in 1..n {
            let x0 = rect.left() + (i-1) as f32 / n as f32 * rect.width();
            let x1 = rect.left() + i as f32 / n as f32 * rect.width();
            let y0 = rect.bottom() - (state.history[i-1] / max_t).clamp(0.0,1.0) * rect.height() * 0.9;
            let y1 = rect.bottom() - (state.history[i] / max_t).clamp(0.0,1.0) * rect.height() * 0.9;
            let t = state.history[i] / max_t;
            let lc = if t > 0.8 { Color32::RED } else if t > 0.5 { Color32::YELLOW } else { Color32::from_rgb(60, 200, 120) };
            p.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y1)], Stroke::new(1.0, lc));
        }
    }
    // 60fps reference line
    let ref_y = rect.bottom() - (16.7 / (max_t * 1000.0 / rect.height())).clamp(0.0, rect.height() * 0.95);
    p.text(Pos2::new(rect.left()+2.0, rect.top()+2.0), egui::Align2::LEFT_TOP, "Frame Time History", FontId::monospace(7.0), Color32::DARK_GRAY);
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
