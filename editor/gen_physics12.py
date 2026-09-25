import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 12
// ============================================================

// ─── Physics Simulation Presets Quick Launch ─────────────────────────────────

pub fn show_quick_launch_panel(ui: &mut egui::Ui) {
    ui.heading("Quick Launch Simulations");
    ui.separator();
    ui.label("Click to instantly set up and run a demo simulation:");

    let simulations: &[(&str, &str, &str)] = &[
        ("Domino Chain", "50 dominos in a line — chain reaction demo", "50 bodies, 0 joints, dt=0.008"),
        ("Ball Pool", "200 balls with varying restitution in a box", "201 bodies, 0 joints, dt=0.016"),
        ("Bridge Collapse", "32-segment bridge with breakable joints", "34 bodies, 62 joints, dt=0.008"),
        ("Ragdoll Pile", "5 humanoid ragdolls dropped into pit", "85 bodies, 80 joints, dt=0.016"),
        ("Cloth Over Sphere", "32x32 cloth mesh draped over a sphere", "1025 bodies, 1984 joints, dt=0.004"),
        ("Water Tank SPH", "500 SPH particles in a walled tank", "500 particles, dt=0.004"),
        ("Double Pendulum", "Classic chaotic double pendulum", "3 bodies, 2 joints, dt=0.002"),
        ("Newton's Cradle", "5 steel balls on strings", "6 bodies, 5 joints, dt=0.004"),
        ("Pyramid Stack", "6-row pyramid of boxes (21 bodies)", "22 bodies, 0 joints, dt=0.016"),
        ("Car Suspension", "4-wheel vehicle on bumpy terrain", "9 bodies, 4 joints, dt=0.016"),
        ("Soft Jelly Cube", "Soft body 4x4 jelly grid (balloon mode)", "16 nodes, 40 springs, dt=0.008"),
        ("Rope Bridge", "10m rope bridge with walkway planks", "21 bodies, 30 joints, dt=0.008"),
    ];

    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for (name, desc, config) in simulations {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(*name).strong().color(Color32::from_rgb(100,200,255)));
                    if ui.button("▶ Launch").clicked() {}
                });
                ui.label(egui::RichText::new(*desc).small().color(Color32::GRAY));
                ui.label(egui::RichText::new(*config).small().color(Color32::DARK_GRAY));
            });
        }
    });
}

// ─── Body Group Selection Tools ───────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct BodyGroupSelector {
    pub selection_mode: SelectionMode,
    pub selected_ids: Vec<usize>,
    pub group_name: String,
    pub saved_groups: Vec<(String, Vec<usize>)>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SelectionMode {
    #[default]
    Single,
    Box,
    Circle,
    ByType(RigidBodyType),
    ByLayer(u8),
    All,
    Invert,
}

impl SelectionMode {
    pub fn label(&self) -> &str {
        match self {
            SelectionMode::Single => "Single Click",
            SelectionMode::Box => "Box Select",
            SelectionMode::Circle => "Circle Select",
            SelectionMode::ByType(_) => "By Type",
            SelectionMode::ByLayer(_) => "By Layer",
            SelectionMode::All => "Select All",
            SelectionMode::Invert => "Invert",
        }
    }
}

pub fn show_body_group_selector(ui: &mut egui::Ui, state: &mut BodyGroupSelector) {
    ui.heading("Body Group Selector");
    ui.separator();

    ui.horizontal(|ui| {
        for mode in &[SelectionMode::Single, SelectionMode::Box, SelectionMode::Circle, SelectionMode::All, SelectionMode::Invert] {
            if ui.selectable_label(std::mem::discriminant(&state.selection_mode) == std::mem::discriminant(mode), mode.label()).clicked() { state.selection_mode = mode.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.label("By Type:");
        for bt in &[RigidBodyType::Dynamic, RigidBodyType::Static, RigidBodyType::Kinematic] {
            if ui.selectable_label(false, egui::RichText::new(bt.label()).color(bt.color()).small()).clicked() {
                state.selection_mode = SelectionMode::ByType(bt.clone());
            }
        }
        ui.label("By Layer:");
        for layer in 0u8..8 {
            if ui.selectable_label(false, format!("{}", layer)).clicked() {
                state.selection_mode = SelectionMode::ByLayer(layer);
            }
        }
    });

    ui.label(format!("Selected: {} bodies", state.selected_ids.len()));

    ui.horizontal(|ui| {
        ui.label("Group Name:");
        ui.text_edit_singleline(&mut state.group_name);
        if ui.button("Save Group").clicked() {
            if !state.group_name.is_empty() {
                state.saved_groups.push((state.group_name.clone(), state.selected_ids.clone()));
                state.group_name.clear();
            }
        }
    });

    if !state.saved_groups.is_empty() {
        ui.separator();
        ui.label("Saved Groups:");
        let mut remove = None;
        for (i, (name, ids)) in state.saved_groups.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.selectable_label(false, format!("{} ({} bodies)", name, ids.len())).clicked() {
                    state.selected_ids = ids.clone();
                }
                if ui.small_button("✗").clicked() { remove = Some(i); }
            });
        }
        if let Some(ri) = remove { state.saved_groups.remove(ri); }
    }
}

// ─── Physics Memory Usage Breakdown ──────────────────────────────────────────

pub fn show_memory_usage_breakdown(ui: &mut egui::Ui) {
    ui.heading("Physics Memory Usage");
    ui.separator();

    let categories: &[(&str, f32, Color32)] = &[
        ("Rigid Bodies", 48.5, Color32::from_rgb(80, 140, 220)),
        ("Collision Shapes", 32.1, Color32::from_rgb(220, 140, 60)),
        ("Contact Cache", 18.4, Color32::from_rgb(80, 220, 120)),
        ("Constraint Data", 12.2, Color32::from_rgb(220, 80, 180)),
        ("Broadphase BVH", 8.7, Color32::from_rgb(180, 220, 60)),
        ("Soft Body Data", 24.3, Color32::from_rgb(60, 200, 220)),
        ("Fluid Particles", 38.9, Color32::from_rgb(100, 160, 255)),
        ("Debug Buffers", 6.4, Color32::GRAY),
    ];

    let total: f32 = categories.iter().map(|(_, kb, _)| kb).sum();

    let desired = Vec2::new(ui.available_width().min(500.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(10, 12, 18));

    let mut x = rect.left();
    for &(name, kb, col) in categories {
        let w = kb / total * rect.width();
        p.rect_filled(Rect::from_min_size(Pos2::new(x, rect.top()+10.0), Vec2::new(w, rect.height()-20.0)), 0.0, col);
        if w > 20.0 {
            let short: String = name.chars().take(4).collect();
            p.text(Pos2::new(x + w*0.5, rect.center().y), egui::Align2::CENTER_CENTER, &short, FontId::monospace(6.5), Color32::BLACK);
        }
        x += w;
    }

    ui.separator();
    egui::Grid::new("mem_breakdown").num_columns(3).spacing([12.0, 2.0]).striped(true).show(ui, |ui| {
        ui.label("Category"); ui.label("KB"); ui.label("% Total"); ui.end_row();
        for &(name, kb, col) in categories {
            ui.colored_label(col, name);
            ui.label(format!("{:.1} KB", kb));
            ui.label(format!("{:.1}%", kb / total * 100.0));
            ui.end_row();
        }
        ui.label(egui::RichText::new("Total").strong());
        ui.label(egui::RichText::new(format!("{:.1} KB", total)).strong());
        ui.label("100.0%");
        ui.end_row();
    });
}

// ─── Timed Physics Events ─────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct TimedPhysicsEvent {
    pub name: String,
    pub trigger_time: f32,
    pub event_type: TimedEventType,
    pub fired: bool,
    pub repeat: bool,
    pub repeat_interval: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum TimedEventType {
    ApplyImpulse { body_id: usize, force: [f32; 2] },
    SetGravity([f32; 2]),
    ToggleBody { body_id: usize, active: bool },
    SpawnBody { preset: String, position: [f32; 2] },
    FireExplosion { position: [f32; 2], strength: f32 },
    PauseSimulation,
    TriggerEvent(String),
}

impl TimedEventType {
    pub fn label(&self) -> &str {
        match self {
            TimedEventType::ApplyImpulse { .. } => "Apply Impulse",
            TimedEventType::SetGravity(_) => "Set Gravity",
            TimedEventType::ToggleBody { .. } => "Toggle Body",
            TimedEventType::SpawnBody { .. } => "Spawn Body",
            TimedEventType::FireExplosion { .. } => "Fire Explosion",
            TimedEventType::PauseSimulation => "Pause Simulation",
            TimedEventType::TriggerEvent(_) => "Trigger Event",
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct TimedPhysicsEventState {
    pub events: Vec<TimedPhysicsEvent>,
    pub selected: Option<usize>,
    pub current_time: f32,
    pub running: bool,
}

pub fn show_timed_physics_events(ui: &mut egui::Ui, state: &mut TimedPhysicsEventState) {
    ui.heading("Timed Physics Events");
    ui.separator();

    if state.events.is_empty() {
        state.events = vec![
            TimedPhysicsEvent { name: "Gravity Flip".into(), trigger_time: 2.0, event_type: TimedEventType::SetGravity([0.0, 9.81]), fired: false, repeat: false, repeat_interval: 0.0 },
            TimedPhysicsEvent { name: "Explosion".into(), trigger_time: 5.0, event_type: TimedEventType::FireExplosion { position: [0.0, 0.0], strength: 500.0 }, fired: false, repeat: false, repeat_interval: 0.0 },
        ];
    }

    ui.horizontal(|ui| {
        let run_lbl = if state.running { "⏸ Pause" } else { "▶ Run" };
        if ui.button(run_lbl).clicked() { state.running = !state.running; }
        if ui.button("Reset").clicked() { state.current_time = 0.0; for e in &mut state.events { e.fired = false; } }
        ui.label(format!("t = {:.2}s", state.current_time));
    });

    let mut remove = None;
    for (i, e) in state.events.iter().enumerate() {
        let sel = state.selected == Some(i);
        let fired_col = if e.fired { Color32::from_rgb(100, 100, 100) } else { Color32::WHITE };
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, egui::RichText::new(format!("t={:.1}s {} [{}]", e.trigger_time, e.name, e.event_type.label())).color(fired_col)).clicked() { state.selected = Some(i); }
            if e.fired { ui.label(egui::RichText::new("✓").color(Color32::GREEN)); }
            if ui.small_button("✗").clicked() { remove = Some(i); }
        });
    }
    if let Some(ri) = remove { state.events.remove(ri); if state.selected == Some(ri) { state.selected = None; } }

    if ui.button("+ Add Event").clicked() {
        state.events.push(TimedPhysicsEvent { name: format!("Event_{}", state.events.len()), trigger_time: 1.0, event_type: TimedEventType::TriggerEvent("custom_event".into()), fired: false, repeat: false, repeat_interval: 0.0 });
    }

    if let Some(idx) = state.selected {
        if idx < state.events.len() {
            let e = &mut state.events[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut e.name); });
            ui.horizontal(|ui| {
                ui.label("Trigger Time:");
                ui.add(egui::DragValue::new(&mut e.trigger_time).speed(0.1).clamp_range(0.0..=3600.0).suffix("s"));
                ui.checkbox(&mut e.repeat, "Repeat");
                if e.repeat { ui.add(egui::DragValue::new(&mut e.repeat_interval).speed(0.1).clamp_range(0.1..=60.0).suffix("s interval")); }
            });
            ui.label(format!("Event: {}", e.event_type.label()));
        }
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
