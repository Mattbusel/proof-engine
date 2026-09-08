import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 13 (Last)
// ============================================================

// ─── Physics Object Properties Clipboard ─────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsClipboard {
    pub copied_body: Option<RigidBodyProperties>,
    pub copied_joint: Option<BreakableJoint>,
    pub copy_count: u32,
    pub paste_count: u32,
}

pub fn show_physics_clipboard_panel(ui: &mut egui::Ui, clipboard: &mut PhysicsClipboard, body_list: &mut RigidBodyListState) {
    ui.heading("Physics Object Clipboard");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Copy Selected Body").clicked() {
            if let Some(idx) = body_list.selected {
                if idx < body_list.bodies.len() {
                    clipboard.copied_body = Some(body_list.bodies[idx].clone());
                    clipboard.copy_count += 1;
                }
            }
        }
        if ui.button("Paste Body").clicked() {
            if let Some(b) = clipboard.copied_body.clone() {
                let mut new_b = b.clone();
                new_b.id = body_list.bodies.len();
                new_b.name = format!("{}_copy_{}", new_b.name, clipboard.paste_count);
                new_b.position[0] += 0.5;
                body_list.bodies.push(new_b);
                clipboard.paste_count += 1;
            }
        }
        if ui.button("Clear").clicked() { clipboard.copied_body = None; clipboard.copied_joint = None; }
    });

    if let Some(b) = &clipboard.copied_body {
        ui.label(format!("Clipboard: {} [{}]", b.name, b.body_type.label()));
    } else {
        ui.label("Clipboard: empty");
    }

    ui.label(format!("Copies: {} | Pastes: {}", clipboard.copy_count, clipboard.paste_count));
}

// ─── Physics Legend Panel ─────────────────────────────────────────────────────

pub fn show_physics_legend(ui: &mut egui::Ui) {
    ui.heading("Physics Legend");
    ui.separator();

    ui.columns(2, |cols| {
        cols[0].label(egui::RichText::new("Body Types").strong());
        egui::Grid::new("legend_body").num_columns(2).spacing([8.0,2.0]).show(&mut cols[0], |ui| {
            ui.colored_label(RigidBodyType::Dynamic.color(), "●"); ui.label("Dynamic — moves with forces"); ui.end_row();
            ui.colored_label(RigidBodyType::Static.color(), "●"); ui.label("Static — immovable ground/walls"); ui.end_row();
            ui.colored_label(RigidBodyType::Kinematic.color(), "●"); ui.label("Kinematic — scripted movement"); ui.end_row();
        });
        cols[0].separator();
        cols[0].label(egui::RichText::new("Collider Shapes").strong());
        egui::Grid::new("legend_shapes").num_columns(2).spacing([8.0,2.0]).show(&mut cols[0], |ui| {
            ui.label("□"); ui.label("Box/Rectangle"); ui.end_row();
            ui.label("○"); ui.label("Circle/Sphere"); ui.end_row();
            ui.label("⊃"); ui.label("Capsule"); ui.end_row();
            ui.label("⬡"); ui.label("Polygon/Convex Hull"); ui.end_row();
            ui.label("⊕"); ui.label("Compound"); ui.end_row();
        });

        cols[1].label(egui::RichText::new("Joint Types").strong());
        egui::Grid::new("legend_joints").num_columns(2).spacing([8.0,2.0]).show(&mut cols[1], |ui| {
            ui.colored_label(JointType2D::Revolute.color(), "⟳"); ui.label("Revolute — rotation only"); ui.end_row();
            ui.colored_label(JointType2D::Distance.color(), "↔"); ui.label("Distance — fixed length"); ui.end_row();
            ui.colored_label(JointType2D::Weld.color(), "✚"); ui.label("Weld — fully rigid"); ui.end_row();
            ui.colored_label(JointType2D::Prismatic.color(), "⇆"); ui.label("Prismatic — slide along axis"); ui.end_row();
            ui.colored_label(JointType2D::Wheel.color(), "⊙"); ui.label("Wheel — revolute + prismatic"); ui.end_row();
        });
        cols[1].separator();
        cols[1].label(egui::RichText::new("Debug Colors").strong());
        egui::Grid::new("legend_debug").num_columns(2).spacing([8.0,2.0]).show(&mut cols[1], |ui| {
            ui.colored_label(Color32::from_rgb(80,200,100), "—"); ui.label("Sleeping body"); ui.end_row();
            ui.colored_label(Color32::from_rgb(80,140,255), "—"); ui.label("Dynamic body AABB"); ui.end_row();
            ui.colored_label(Color32::RED, "✕"); ui.label("Contact point"); ui.end_row();
            ui.colored_label(Color32::YELLOW, "●"); ui.label("Mass center"); ui.end_row();
            ui.colored_label(Color32::from_rgb(100,200,255), "→"); ui.label("Velocity vector"); ui.end_row();
        });
    });
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
