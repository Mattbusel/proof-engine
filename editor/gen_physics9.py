import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 9 (Final)
// ============================================================

// ─── Contact Manifold Viewer ─────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct ContactManifold {
    pub body_a: usize,
    pub body_b: usize,
    pub contact_points: Vec<ContactPoint>,
    pub normal: [f32; 2],
    pub friction: f32,
    pub restitution: f32,
    pub is_active: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ContactPoint {
    pub position: [f32; 2],
    pub normal_impulse: f32,
    pub tangent_impulse: f32,
    pub penetration: f32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ContactManifoldViewerState {
    pub manifolds: Vec<ContactManifold>,
    pub selected: Option<usize>,
    pub show_normals: bool,
    pub show_impulses: bool,
    pub show_penetration_depth: bool,
    pub min_impulse_threshold: f32,
}

pub fn show_contact_manifold_viewer(ui: &mut egui::Ui, state: &mut ContactManifoldViewerState) {
    ui.heading("Contact Manifold Viewer");
    ui.separator();

    if state.manifolds.is_empty() {
        state.manifolds = vec![
            ContactManifold { body_a: 0, body_b: 1, contact_points: vec![ContactPoint { position: [0.5, 0.0], normal_impulse: 42.3, tangent_impulse: 8.1, penetration: 0.002 }], normal: [0.0, 1.0], friction: 0.5, restitution: 0.3, is_active: true },
            ContactManifold { body_a: 1, body_b: 2, contact_points: vec![ContactPoint { position: [1.5, 0.5], normal_impulse: 18.7, tangent_impulse: 2.3, penetration: 0.001 }, ContactPoint { position: [1.5, -0.5], normal_impulse: 22.1, tangent_impulse: 3.1, penetration: 0.002 }], normal: [-1.0, 0.0], friction: 0.4, restitution: 0.2, is_active: true },
        ];
    }

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_normals, "Show Normals");
        ui.checkbox(&mut state.show_impulses, "Show Impulses");
        ui.checkbox(&mut state.show_penetration_depth, "Pen. Depth");
        ui.label("Min Impulse:");
        ui.add(egui::DragValue::new(&mut state.min_impulse_threshold).speed(0.1).clamp_range(0.0..=100.0));
    });

    ui.label(format!("Active Manifolds: {}", state.manifolds.iter().filter(|m| m.is_active).count()));

    for (i, m) in state.manifolds.iter().enumerate() {
        if !m.is_active { continue; }
        let sel = state.selected == Some(i);
        ui.horizontal(|ui| {
            if ui.selectable_label(sel, format!("B#{} vs B#{} ({} pts)", m.body_a, m.body_b, m.contact_points.len())).clicked() { state.selected = Some(i); }
            ui.label(format!("n=({:.2},{:.2})", m.normal[0], m.normal[1]));
        });

        if sel {
            for (pi, cp) in m.contact_points.iter().enumerate() {
                if cp.normal_impulse < state.min_impulse_threshold { continue; }
                ui.horizontal(|ui| {
                    ui.label(format!("  CP#{}: pos=({:.3},{:.3})", pi, cp.position[0], cp.position[1]));
                    if state.show_impulses { ui.label(format!("J_n={:.2} J_t={:.2}", cp.normal_impulse, cp.tangent_impulse)); }
                    if state.show_penetration_depth { ui.label(format!("pen={:.4}m", cp.penetration)); }
                });
            }
        }
    }
}

// ─── Collision Material Pair Editor ──────────────────────────────────────────

pub fn show_collision_material_pair_editor(ui: &mut egui::Ui) {
    ui.heading("Material Pair Override Editor");
    ui.separator();
    ui.label("Override collision properties for specific material combinations:");

    let pairs = [
        ("Ice", "Ice", 0.01, 0.0, "Super slippery, no bounce"),
        ("Rubber", "Concrete", 0.9, 0.6, "High friction, medium bounce"),
        ("Steel", "Steel", 0.4, 0.5, "Metal on metal"),
        ("Wood", "Water", 0.1, 0.05, "Low friction in water"),
        ("Glass", "Stone", 0.3, 0.1, "Glass shatters on stone"),
        ("Foam", "Foam", 0.5, 0.05, "Foam absorbs impact"),
    ];

    egui::Grid::new("mat_pairs").num_columns(5).spacing([10.0, 3.0]).striped(true).show(ui, |ui| {
        ui.label("Mat A"); ui.label("Mat B"); ui.label("Friction"); ui.label("Restitution"); ui.label("Notes"); ui.end_row();
        for (a, b, f, r, note) in &pairs {
            ui.label(egui::RichText::new(*a).strong().small());
            ui.label(egui::RichText::new(*b).strong().small());
            ui.label(format!("{:.2}", f));
            ui.label(format!("{:.2}", r));
            ui.label(egui::RichText::new(*note).small().color(Color32::GRAY));
            ui.end_row();
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("+ Add Pair Override").clicked() {}
        if ui.button("Reset All Overrides").clicked() {}
        if ui.button("Export Overrides").clicked() {}
    });
}

// ─── Collision Layer Matrix Quick Reference ───────────────────────────────────

pub fn show_collision_layer_quick_reference(ui: &mut egui::Ui) {
    ui.heading("Collision Layer Quick Reference");
    ui.separator();

    let layer_info: &[(&str, &str, &[usize])] = &[
        ("Default", "Layer 0 — general objects", &[0,1,2,3,4,5,6,7]),
        ("Player", "Layer 1 — player character", &[0,2,3,4]),
        ("Enemy", "Layer 2 — enemy bodies", &[0,1,3,4]),
        ("Ground", "Layer 3 — static terrain", &[0,1,2,5,6]),
        ("Trigger", "Layer 4 — trigger/sensor zones", &[1,2]),
        ("Projectile", "Layer 5 — bullets/projectiles", &[0,2,3]),
        ("Debris", "Layer 6 — debris/particles", &[3]),
        ("UI", "Layer 7 — UI interaction only", &[]),
    ];

    egui::Grid::new("layer_ref").num_columns(3).spacing([10.0, 3.0]).striped(true).show(ui, |ui| {
        ui.label("Layer"); ui.label("Description"); ui.label("Collides With"); ui.end_row();
        for (name, desc, collides) in layer_info {
            ui.label(egui::RichText::new(*name).strong().color(Color32::from_rgb(100,180,255)).small());
            ui.label(egui::RichText::new(*desc).small().color(Color32::GRAY));
            let collides_str = collides.iter().map(|l| format!("{}", l)).collect::<Vec<_>>().join(", ");
            ui.label(egui::RichText::new(collides_str).monospace().small());
            ui.end_row();
        }
    });
}

// ─── Physics State Serializer ─────────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsStateSerialization {
    pub format: SerializationFormat,
    pub compress: bool,
    pub include_velocities: bool,
    pub include_materials: bool,
    pub include_joints: bool,
    pub include_soft_bodies: bool,
    pub include_fluid: bool,
    pub last_export_size_bytes: usize,
    pub last_import_time_ms: f32,
    pub export_path: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SerializationFormat {
    #[default]
    Json,
    MessagePack,
    Cbor,
    Toml,
    Ron,
}

impl SerializationFormat {
    pub fn label(&self) -> &str {
        match self {
            SerializationFormat::Json => "JSON",
            SerializationFormat::MessagePack => "MessagePack (binary)",
            SerializationFormat::Cbor => "CBOR (binary)",
            SerializationFormat::Toml => "TOML",
            SerializationFormat::Ron => "RON (Rust Object Notation)",
        }
    }
    pub fn extension(&self) -> &str {
        match self {
            SerializationFormat::Json => ".json",
            SerializationFormat::MessagePack => ".msgpack",
            SerializationFormat::Cbor => ".cbor",
            SerializationFormat::Toml => ".toml",
            SerializationFormat::Ron => ".ron",
        }
    }
}

pub fn show_physics_serialization_panel(ui: &mut egui::Ui, state: &mut PhysicsStateSerialization) {
    ui.heading("Physics State Serialization");
    ui.separator();

    ui.label("Format:");
    for fmt in &[SerializationFormat::Json, SerializationFormat::MessagePack, SerializationFormat::Cbor, SerializationFormat::Toml, SerializationFormat::Ron] {
        if ui.selectable_label(state.format == *fmt, fmt.label()).clicked() { state.format = fmt.clone(); }
    }

    ui.separator();
    ui.label("Include in Export:");
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.include_velocities, "Velocities");
        ui.checkbox(&mut state.include_materials, "Materials");
        ui.checkbox(&mut state.include_joints, "Joints");
        ui.checkbox(&mut state.include_soft_bodies, "Soft Bodies");
        ui.checkbox(&mut state.include_fluid, "Fluid Particles");
    });
    ui.checkbox(&mut state.compress, "Compress Output");

    ui.horizontal(|ui| {
        ui.label("Export Path:");
        ui.text_edit_singleline(&mut state.export_path);
        if ui.button(format!("Export{}", state.format.extension())).clicked() {
            state.last_export_size_bytes = 1024 * 32; // simulated
        }
        if ui.button("Import").clicked() {
            state.last_import_time_ms = 4.2; // simulated
        }
    });

    if state.last_export_size_bytes > 0 {
        ui.label(format!("Last export: {} bytes ({:.1} KB)", state.last_export_size_bytes, state.last_export_size_bytes as f32 / 1024.0));
    }
    if state.last_import_time_ms > 0.0 {
        ui.label(format!("Last import: {:.2}ms", state.last_import_time_ms));
    }
}

// ─── Physics Keyboard Shortcut Executor ──────────────────────────────────────

pub fn handle_physics_keyboard_shortcuts(ctx: &egui::Context, simulation_running: &mut bool, show_debug: &mut bool, selected_body: &mut Option<usize>) {
    ctx.input(|i| {
        if i.key_pressed(egui::Key::Space) { *simulation_running = !*simulation_running; }
        if i.key_pressed(egui::Key::D) { *show_debug = !*show_debug; }
        if i.key_pressed(egui::Key::Escape) { *selected_body = None; }
    });
}

// ─── Compound Collider Builder ────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct CompoundColliderPart {
    pub shape: ColliderShape,
    pub offset: [f32; 2],
    pub rotation: f32,
    pub density: f32,
    pub name: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CompoundColliderBuilderState {
    pub parts: Vec<CompoundColliderPart>,
    pub selected_part: Option<usize>,
    pub total_mass: f32,
    pub computed_com: [f32; 2],
}

pub fn show_compound_collider_builder(ui: &mut egui::Ui, state: &mut CompoundColliderBuilderState) {
    ui.heading("Compound Collider Builder");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ Box").clicked() { state.parts.push(CompoundColliderPart { shape: ColliderShape::Box { hw: 0.5, hh: 0.5 }, offset: [0.0, 0.0], rotation: 0.0, density: 1.0, name: format!("Box_{}", state.parts.len()) }); }
        if ui.button("+ Circle").clicked() { state.parts.push(CompoundColliderPart { shape: ColliderShape::Circle { r: 0.5 }, offset: [0.0, 0.0], rotation: 0.0, density: 1.0, name: format!("Circle_{}", state.parts.len()) }); }
        if ui.button("+ Capsule").clicked() { state.parts.push(CompoundColliderPart { shape: ColliderShape::Capsule { r: 0.3, hh: 0.5 }, offset: [0.0, 0.0], rotation: 0.0, density: 1.0, name: format!("Capsule_{}", state.parts.len()) }); }
        if let Some(s) = state.selected_part { if ui.button("Delete").clicked() { state.parts.remove(s); state.selected_part = None; } }
    });

    for (i, part) in state.parts.iter().enumerate() {
        let sel = state.selected_part == Some(i);
        if ui.selectable_label(sel, format!("{} [{}] @ ({:.2},{:.2})", part.name, part.shape.label(), part.offset[0], part.offset[1])).clicked() { state.selected_part = Some(i); }
    }

    if let Some(idx) = state.selected_part {
        if idx < state.parts.len() {
            let part = &mut state.parts[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut part.name); });
            ui.horizontal(|ui| {
                ui.label("Offset:");
                ui.add(egui::DragValue::new(&mut part.offset[0]).prefix("x:").speed(0.02));
                ui.add(egui::DragValue::new(&mut part.offset[1]).prefix("y:").speed(0.02));
                ui.label("Rotation:");
                ui.add(egui::DragValue::new(&mut part.rotation).speed(1.0).suffix("°"));
            });
            ui.horizontal(|ui| {
                ui.label("Density:");
                ui.add(egui::DragValue::new(&mut part.density).speed(0.05).clamp_range(0.01..=10000.0));
            });
            match &mut part.shape {
                ColliderShape::Box { hw, hh } => {
                    ui.horizontal(|ui| {
                        ui.label("Half W:"); ui.add(egui::DragValue::new(hw).speed(0.01).clamp_range(0.01..=50.0));
                        ui.label("Half H:"); ui.add(egui::DragValue::new(hh).speed(0.01).clamp_range(0.01..=50.0));
                    });
                }
                ColliderShape::Circle { r } => {
                    ui.horizontal(|ui| { ui.label("Radius:"); ui.add(egui::DragValue::new(r).speed(0.01).clamp_range(0.01..=50.0)); });
                }
                ColliderShape::Capsule { r, hh } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius:"); ui.add(egui::DragValue::new(r).speed(0.01).clamp_range(0.01..=20.0));
                        ui.label("Half H:"); ui.add(egui::DragValue::new(hh).speed(0.01).clamp_range(0.01..=20.0));
                    });
                }
                _ => {}
            }
        }
    }

    // Compute total mass
    state.total_mass = state.parts.iter().map(|p| {
        let area = match &p.shape {
            ColliderShape::Box { hw, hh } => 4.0 * hw * hh,
            ColliderShape::Circle { r } => std::f32::consts::PI * r * r,
            ColliderShape::Capsule { r, hh } => std::f32::consts::PI * r * r + 2.0 * r * hh,
            _ => 1.0,
        };
        area * p.density
    }).sum();

    // Compute center of mass
    let total = state.total_mass.max(0.001);
    let cx: f32 = state.parts.iter().map(|p| {
        let area = match &p.shape { ColliderShape::Box { hw, hh } => 4.0 * hw * hh, ColliderShape::Circle { r } => std::f32::consts::PI * r * r, _ => 1.0 };
        p.offset[0] * area * p.density
    }).sum::<f32>() / total;
    let cy: f32 = state.parts.iter().map(|p| {
        let area = match &p.shape { ColliderShape::Box { hw, hh } => 4.0 * hw * hh, ColliderShape::Circle { r } => std::f32::consts::PI * r * r, _ => 1.0 };
        p.offset[1] * area * p.density
    }).sum::<f32>() / total;
    state.computed_com = [cx, cy];

    ui.separator();
    ui.label(format!("Total Mass: {:.4} | CoM: ({:.4}, {:.4})", state.total_mass, state.computed_com[0], state.computed_com[1]));
    draw_compound_collider_preview(ui, state);
}

fn draw_compound_collider_preview(ui: &mut egui::Ui, state: &CompoundColliderBuilderState) {
    let desired = Vec2::new(ui.available_width().min(280.0), 180.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(10, 12, 18));
    let c = rect.center();
    let scale = 35.0;

    for (i, part) in state.parts.iter().enumerate() {
        let pc = c + Vec2::new(part.offset[0] * scale, -part.offset[1] * scale);
        let sel = state.selected_part == Some(i);
        let col = if sel { Color32::WHITE } else { Color32::from_rgb(80, 140, 220) };
        let fill = Color32::from_rgba_unmultiplied(col.r()/3, col.g()/3, col.b()/3, 180);
        match &part.shape {
            ColliderShape::Box { hw, hh } => {
                let r = Rect::from_center_size(pc, Vec2::new(hw*scale*2.0, hh*scale*2.0));
                p.rect_filled(r, 2.0, fill);
                p.rect_stroke(r, 2.0, Stroke::new(if sel { 2.0 } else { 1.0 }, col));
            }
            ColliderShape::Circle { r } => {
                p.circle_filled(pc, r * scale, fill);
                p.circle_stroke(pc, r * scale, Stroke::new(if sel { 2.0 } else { 1.0 }, col));
            }
            ColliderShape::Capsule { r, hh } => {
                let sr = r * scale; let sh = hh * scale;
                p.circle_filled(pc - Vec2::new(0.0, sh), sr, fill);
                p.circle_filled(pc + Vec2::new(0.0, sh), sr, fill);
                p.rect_filled(Rect::from_center_size(pc, Vec2::new(sr*2.0, sh*2.0)), 0.0, fill);
                p.circle_stroke(pc - Vec2::new(0.0, sh), sr, Stroke::new(1.0, col));
                p.circle_stroke(pc + Vec2::new(0.0, sh), sr, Stroke::new(1.0, col));
            }
            _ => {}
        }
    }

    // CoM marker
    if !state.parts.is_empty() {
        let com_screen = c + Vec2::new(state.computed_com[0] * scale, -state.computed_com[1] * scale);
        p.circle_filled(com_screen, 4.0, Color32::YELLOW);
        p.circle_stroke(com_screen, 5.0, Stroke::new(1.0, Color32::YELLOW));
    }
    p.text(Pos2::new(rect.left()+4.0, rect.bottom()-12.0), egui::Align2::LEFT_BOTTOM, format!("{} parts | mass {:.3}", state.parts.len(), state.total_mass), FontId::monospace(7.0), Color32::DARK_GRAY);
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
