#!/usr/bin/env python
# Appends physics expansion to physics_editor.rs
import os

OUTFILE = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// SOFT BODY SIMULATION
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoftNode {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub mass: f32,
    pub radius: f32,
}

impl SoftNode {
    pub fn new(x: f32, y: f32) -> Self {
        SoftNode { position: [x, y], velocity: [0.0, 0.0], mass: 1.0, radius: 0.05 }
    }
    pub fn new_with_mass(x: f32, y: f32, mass: f32, radius: f32) -> Self {
        SoftNode { position: [x, y], velocity: [0.0, 0.0], mass, radius }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoftSpring {
    pub a: usize,
    pub b: usize,
    pub rest_length: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SoftSpring {
    pub fn new(a: usize, b: usize, rest_length: f32, stiffness: f32, damping: f32) -> Self {
        SoftSpring { a, b, rest_length, stiffness, damping }
    }
    pub fn length_between(nodes: &[SoftNode], a: usize, b: usize) -> f32 {
        let dx = nodes[b].position[0] - nodes[a].position[0];
        let dy = nodes[b].position[1] - nodes[a].position[1];
        (dx * dx + dy * dy).sqrt()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoftBody {
    pub nodes: Vec<SoftNode>,
    pub springs: Vec<SoftSpring>,
    pub name: String,
    pub pinned: HashSet<usize>,
    pub pressure: f32,
    pub volume_preservation: f32,
    pub damping: f32,
    pub stiffness: f32,
}

impl SoftBody {
    pub fn new(name: &str) -> Self {
        SoftBody {
            nodes: Vec::new(),
            springs: Vec::new(),
            name: name.to_string(),
            pinned: HashSet::new(),
            pressure: 0.0,
            volume_preservation: 0.5,
            damping: 0.98,
            stiffness: 200.0,
        }
    }

    pub fn compute_area(&self) -> f32 {
        let n = self.nodes.len();
        if n < 3 { return 0.0; }
        let mut area = 0.0f32;
        for i in 0..n {
            let j = (i + 1) % n;
            let xi = self.nodes[i].position[0];
            let yi = self.nodes[i].position[1];
            let xj = self.nodes[j].position[0];
            let yj = self.nodes[j].position[1];
            area += xi * yj - xj * yi;
        }
        (area * 0.5).abs()
    }

    pub fn compute_perimeter(&self) -> f32 {
        let n = self.nodes.len();
        (0..n).map(|i| {
            let j = (i + 1) % n;
            let dx = self.nodes[j].position[0] - self.nodes[i].position[0];
            let dy = self.nodes[j].position[1] - self.nodes[i].position[1];
            (dx*dx+dy*dy).sqrt()
        }).sum()
    }

    pub fn center_of_mass(&self) -> [f32; 2] {
        if self.nodes.is_empty() { return [0.0, 0.0]; }
        let (sx, sy, sm): (f32, f32, f32) = self.nodes.iter().fold((0.0,0.0,0.0), |acc, n| {
            (acc.0 + n.position[0]*n.mass, acc.1 + n.position[1]*n.mass, acc.2 + n.mass)
        });
        if sm < 1e-6 { [0.0, 0.0] } else { [sx/sm, sy/sm] }
    }

    pub fn total_kinetic_energy(&self) -> f32 {
        self.nodes.iter().map(|n| {
            0.5 * n.mass * (n.velocity[0]*n.velocity[0] + n.velocity[1]*n.velocity[1])
        }).sum()
    }

    pub fn preset_jelly(center: [f32; 2], radius: f32) -> Self {
        let mut body = SoftBody::new("Jelly");
        body.stiffness = 120.0;
        body.pressure = 0.8;
        body.damping = 0.96;
        let count = 12usize;
        for i in 0..count {
            let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
            let x = center[0] + radius * angle.cos();
            let y = center[1] + radius * angle.sin();
            body.nodes.push(SoftNode::new(x, y));
        }
        body.nodes.push(SoftNode::new_with_mass(center[0], center[1], 2.0, 0.04));
        let interior = count;
        for i in 0..count {
            let j = (i + 1) % count;
            let rl = SoftSpring::length_between(&body.nodes, i, j);
            body.springs.push(SoftSpring::new(i, j, rl, body.stiffness, 0.1));
            let rl2 = SoftSpring::length_between(&body.nodes, i, interior);
            body.springs.push(SoftSpring::new(i, interior, rl2, body.stiffness * 0.7, 0.05));
        }
        body
    }

    pub fn preset_balloon(center: [f32; 2], radius: f32) -> Self {
        let mut body = SoftBody::new("Balloon");
        body.stiffness = 80.0;
        body.pressure = 2.0;
        body.damping = 0.99;
        let count = 16usize;
        for i in 0..count {
            let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
            body.nodes.push(SoftNode::new(center[0] + radius * angle.cos(), center[1] + radius * angle.sin()));
        }
        for i in 0..count {
            let j = (i + 1) % count;
            let rl = SoftSpring::length_between(&body.nodes, i, j);
            body.springs.push(SoftSpring::new(i, j, rl, body.stiffness, 0.05));
        }
        body
    }

    pub fn preset_rope_chain(start: [f32; 2], end: [f32; 2], segments: usize) -> Self {
        let mut body = SoftBody::new("Rope Chain");
        body.stiffness = 500.0;
        body.damping = 0.95;
        body.pressure = 0.0;
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let x = start[0] + (end[0] - start[0]) * t;
            let y = start[1] + (end[1] - start[1]) * t;
            body.nodes.push(SoftNode::new(x, y));
        }
        body.pinned.insert(0);
        for i in 0..segments {
            let rl = SoftSpring::length_between(&body.nodes, i, i+1);
            body.springs.push(SoftSpring::new(i, i+1, rl, body.stiffness, 0.2));
        }
        body
    }

    pub fn preset_cloth_square(origin: [f32; 2], size: f32, divs: usize) -> Self {
        let mut body = SoftBody::new("Cloth Square");
        body.stiffness = 300.0;
        body.damping = 0.97;
        body.pressure = 0.0;
        let step = size / divs as f32;
        for row in 0..=divs {
            for col in 0..=divs {
                let x = origin[0] + col as f32 * step;
                let y = origin[1] + row as f32 * step;
                body.nodes.push(SoftNode::new(x, y));
            }
        }
        let w = divs + 1;
        for col in 0..=divs { body.pinned.insert(col); }
        for row in 0..=divs {
            for col in 0..divs {
                let i = row * w + col;
                let j = row * w + col + 1;
                let rl = SoftSpring::length_between(&body.nodes, i, j);
                body.springs.push(SoftSpring::new(i, j, rl, body.stiffness, 0.1));
            }
        }
        for row in 0..divs {
            for col in 0..=divs {
                let i = row * w + col;
                let j = (row+1) * w + col;
                let rl = SoftSpring::length_between(&body.nodes, i, j);
                body.springs.push(SoftSpring::new(i, j, rl, body.stiffness, 0.1));
            }
        }
        body
    }

    pub fn preset_muscle(start: [f32; 2], end: [f32; 2]) -> Self {
        let mut body = SoftBody::new("Muscle");
        body.stiffness = 400.0;
        body.damping = 0.95;
        body.pressure = 0.3;
        let segments = 8usize;
        let widths = [0.05f32, 0.08, 0.12, 0.15, 0.15, 0.12, 0.08, 0.05, 0.04];
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let length = (dx*dx+dy*dy).sqrt();
        let nx = -dy / length.max(1e-6);
        let ny = dx / length.max(1e-6);
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let cx = start[0] + dx * t;
            let cy = start[1] + dy * t;
            let w = widths[i.min(widths.len()-1)];
            body.nodes.push(SoftNode::new(cx + nx*w, cy + ny*w));
            body.nodes.push(SoftNode::new(cx - nx*w, cy - ny*w));
        }
        body.pinned.insert(0);
        body.pinned.insert(1);
        let last = (segments+1)*2;
        body.pinned.insert(last - 2);
        body.pinned.insert(last - 1);
        for i in 0..=segments {
            let top = i * 2;
            let bot = i * 2 + 1;
            let rl = SoftSpring::length_between(&body.nodes, top, bot);
            body.springs.push(SoftSpring::new(top, bot, rl, body.stiffness, 0.1));
            if i < segments {
                let ntop = (i+1)*2;
                let nbot = (i+1)*2+1;
                let rl2 = SoftSpring::length_between(&body.nodes, top, ntop);
                body.springs.push(SoftSpring::new(top, ntop, rl2, body.stiffness, 0.1));
                let rl3 = SoftSpring::length_between(&body.nodes, bot, nbot);
                body.springs.push(SoftSpring::new(bot, nbot, rl3, body.stiffness, 0.1));
                let rl4 = SoftSpring::length_between(&body.nodes, top, nbot);
                body.springs.push(SoftSpring::new(top, nbot, rl4, body.stiffness * 0.7, 0.05));
            }
        }
        body
    }

    pub fn preset_bouncy_ball(center: [f32; 2], radius: f32) -> Self {
        let mut body = SoftBody::new("Bouncy Ball");
        body.stiffness = 600.0;
        body.pressure = 1.5;
        body.damping = 0.995;
        let count = 20usize;
        for i in 0..count {
            let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
            body.nodes.push(SoftNode::new(center[0] + radius*angle.cos(), center[1] + radius*angle.sin()));
        }
        for i in 0..count {
            let j = (i+1) % count;
            let rl = SoftSpring::length_between(&body.nodes, i, j);
            body.springs.push(SoftSpring::new(i, j, rl, body.stiffness, 0.02));
            let k = (i + count/2) % count;
            let rl2 = SoftSpring::length_between(&body.nodes, i, k);
            body.springs.push(SoftSpring::new(i, k, rl2, body.stiffness * 0.5, 0.01));
        }
        body
    }
}

pub fn simulate_soft_body(body: &mut SoftBody, dt: f32, gravity: [f32; 2]) {
    let n = body.nodes.len();
    if n == 0 { return; }
    let mut forces = vec![[0.0f32; 2]; n];
    for i in 0..n {
        if !body.pinned.contains(&i) {
            forces[i][0] += gravity[0] * body.nodes[i].mass;
            forces[i][1] += gravity[1] * body.nodes[i].mass;
        }
    }
    for spring in &body.springs {
        let a = spring.a.min(n - 1);
        let b = spring.b.min(n - 1);
        if a == b { continue; }
        let dx = body.nodes[b].position[0] - body.nodes[a].position[0];
        let dy = body.nodes[b].position[1] - body.nodes[a].position[1];
        let len = (dx*dx + dy*dy).sqrt().max(1e-6);
        let nx = dx / len;
        let ny = dy / len;
        let extension = len - spring.rest_length;
        let spring_force = spring.stiffness * extension;
        let rel_vx = body.nodes[b].velocity[0] - body.nodes[a].velocity[0];
        let rel_vy = body.nodes[b].velocity[1] - body.nodes[a].velocity[1];
        let damp_force = spring.damping * (rel_vx * nx + rel_vy * ny);
        let total = spring_force + damp_force;
        forces[a][0] += total * nx;
        forces[a][1] += total * ny;
        forces[b][0] -= total * nx;
        forces[b][1] -= total * ny;
    }
    if body.pressure > 0.0 && n >= 3 {
        let area = body.compute_area();
        let perimeter = body.compute_perimeter().max(1e-6);
        let pressure_mag = body.pressure * area / perimeter;
        for i in 0..n {
            let j = (i + 1) % n;
            let dx = body.nodes[j].position[0] - body.nodes[i].position[0];
            let dy = body.nodes[j].position[1] - body.nodes[i].position[1];
            let seg_len = (dx*dx+dy*dy).sqrt().max(1e-6);
            let outward_x = dy / seg_len;
            let outward_y = -dx / seg_len;
            forces[i][0] += pressure_mag * outward_x * 0.5;
            forces[i][1] += pressure_mag * outward_y * 0.5;
            forces[j][0] += pressure_mag * outward_x * 0.5;
            forces[j][1] += pressure_mag * outward_y * 0.5;
        }
    }
    for i in 0..n {
        if body.pinned.contains(&i) {
            body.nodes[i].velocity = [0.0, 0.0];
            continue;
        }
        let inv_mass = 1.0 / body.nodes[i].mass.max(1e-6);
        body.nodes[i].velocity[0] = (body.nodes[i].velocity[0] + forces[i][0] * inv_mass * dt) * body.damping;
        body.nodes[i].velocity[1] = (body.nodes[i].velocity[1] + forces[i][1] * inv_mass * dt) * body.damping;
        body.nodes[i].position[0] += body.nodes[i].velocity[0] * dt;
        body.nodes[i].position[1] += body.nodes[i].velocity[1] * dt;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SoftBodyPreset { Jelly, Balloon, RopeChain, ClothSquare, Muscle, BouncyBall }

impl SoftBodyPreset {
    pub fn label(&self) -> &'static str {
        match self {
            SoftBodyPreset::Jelly => "Jelly",
            SoftBodyPreset::Balloon => "Balloon",
            SoftBodyPreset::RopeChain => "Rope Chain",
            SoftBodyPreset::ClothSquare => "Cloth Square",
            SoftBodyPreset::Muscle => "Muscle",
            SoftBodyPreset::BouncyBall => "Bouncy Ball",
        }
    }
    pub fn all() -> &'static [SoftBodyPreset] {
        &[SoftBodyPreset::Jelly, SoftBodyPreset::Balloon, SoftBodyPreset::RopeChain,
          SoftBodyPreset::ClothSquare, SoftBodyPreset::Muscle, SoftBodyPreset::BouncyBall]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SoftBodyEditorState {
    pub bodies: Vec<SoftBody>,
    pub selected_body: Option<usize>,
    pub selected_node: Option<usize>,
    pub selected_spring: Option<usize>,
    pub simulating: bool,
    pub sim_time: f32,
    pub gravity: [f32; 2],
    pub show_nodes: bool,
    pub show_springs: bool,
    pub show_forces: bool,
    pub node_drag: bool,
    pub pin_mode: bool,
    pub preset_selection: usize,
}

impl SoftBodyEditorState {
    pub fn new() -> Self {
        SoftBodyEditorState {
            bodies: Vec::new(), selected_body: None, selected_node: None, selected_spring: None,
            simulating: false, sim_time: 0.0, gravity: [0.0, -9.81],
            show_nodes: true, show_springs: true, show_forces: false,
            node_drag: false, pin_mode: false, preset_selection: 0,
        }
    }
}

pub fn show_soft_body_editor(ui: &mut egui::Ui, state: &mut SoftBodyEditorState) {
    ui.heading(RichText::new("Soft Body Simulation").color(Color32::from_rgb(180, 220, 255)));
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button(if state.simulating { "Pause" } else { "Simulate" }).clicked() { state.simulating = !state.simulating; }
        if ui.button("Reset").clicked() { state.sim_time = 0.0; }
        ui.label(RichText::new(format!("t={:.2}s  bodies={}", state.sim_time, state.bodies.len())).small().color(Color32::GRAY));
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label("Preset:");
        for (i, preset) in SoftBodyPreset::all().iter().enumerate() {
            if ui.selectable_label(state.preset_selection == i, preset.label()).clicked() { state.preset_selection = i; }
        }
        if ui.button("Add").clicked() {
            let center = [0.0f32, 0.0f32];
            let body = match state.preset_selection {
                0 => SoftBody::preset_jelly(center, 0.5),
                1 => SoftBody::preset_balloon(center, 0.5),
                2 => SoftBody::preset_rope_chain([-0.5, 0.5], [0.5, 0.5], 10),
                3 => SoftBody::preset_cloth_square([-0.5, -0.5], 1.0, 6),
                4 => SoftBody::preset_muscle([-0.5, 0.0], [0.5, 0.0]),
                _ => SoftBody::preset_bouncy_ball(center, 0.4),
            };
            state.bodies.push(body);
            state.selected_body = Some(state.bodies.len() - 1);
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("sb_list").max_height(120.0).show(ui, |ui| {
        let mut to_remove = None;
        for (i, body) in state.bodies.iter().enumerate() {
            ui.horizontal(|ui| {
                let selected = state.selected_body == Some(i);
                if ui.selectable_label(selected, &body.name).clicked() { state.selected_body = Some(i); }
                ui.label(RichText::new(format!("n:{} s:{}", body.nodes.len(), body.springs.len())).small().color(Color32::GRAY));
                if ui.small_button("X").clicked() { to_remove = Some(i); }
            });
        }
        if let Some(idx) = to_remove {
            state.bodies.remove(idx);
            if state.selected_body == Some(idx) { state.selected_body = None; }
        }
    });
    if let Some(bi) = state.selected_body {
        if let Some(body) = state.bodies.get_mut(bi) {
            ui.separator();
            egui::CollapsingHeader::new(RichText::new("Material Properties").color(Color32::from_rgb(200, 180, 255)))
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("sb_mat").num_columns(2).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
                        ui.label("Stiffness:"); ui.add(egui::Slider::new(&mut body.stiffness, 10.0f32..=2000.0).logarithmic(true)); ui.end_row();
                        ui.label("Damping:"); ui.add(egui::Slider::new(&mut body.damping, 0.8f32..=1.0)); ui.end_row();
                        ui.label("Pressure:"); ui.add(egui::Slider::new(&mut body.pressure, 0.0f32..=5.0)); ui.end_row();
                        ui.label("Vol Preserve:"); ui.add(egui::Slider::new(&mut body.volume_preservation, 0.0f32..=1.0)); ui.end_row();
                    });
                    ui.label(RichText::new(format!("Area: {:.4}  Perimeter: {:.4}  KE: {:.4}", body.compute_area(), body.compute_perimeter(), body.total_kinetic_energy())).small().color(Color32::GRAY));
                });
            egui::CollapsingHeader::new(RichText::new(format!("Nodes ({})", body.nodes.len())).color(Color32::from_rgb(180, 255, 180)))
                .default_open(false)
                .show(ui, |ui| {
                    ui.checkbox(&mut state.pin_mode, "Pin Mode");
                    egui::ScrollArea::vertical().id_salt("sb_nodes").max_height(150.0).show(ui, |ui| {
                        for (ni, node) in body.nodes.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                let is_pinned = body.pinned.contains(&ni);
                                ui.label(RichText::new(if is_pinned { "P" } else { " " }).color(Color32::from_rgb(255, 200, 80)));
                                ui.label(RichText::new(format!("#{:2}", ni)).monospace().small());
                                ui.add(egui::DragValue::new(&mut node.position[0]).speed(0.01).prefix("x:"));
                                ui.add(egui::DragValue::new(&mut node.position[1]).speed(0.01).prefix("y:"));
                                ui.add(egui::DragValue::new(&mut node.mass).speed(0.01).prefix("m:").clamp_range(0.01f32..=10.0));
                            });
                        }
                    });
                });
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Gravity:");
        ui.add(egui::DragValue::new(&mut state.gravity[0]).speed(0.1).prefix("x:").suffix(" m/s²"));
        ui.add(egui::DragValue::new(&mut state.gravity[1]).speed(0.1).prefix("y:").suffix(" m/s²"));
        if ui.small_button("Earth").clicked() { state.gravity = [0.0, -9.81]; }
        if ui.small_button("Moon").clicked() { state.gravity = [0.0, -1.62]; }
        if ui.small_button("Zero G").clicked() { state.gravity = [0.0, 0.0]; }
    });
    if state.simulating {
        let dt = 1.0 / 60.0;
        let gravity = state.gravity;
        for body in &mut state.bodies { simulate_soft_body(body, dt, gravity); }
        state.sim_time += dt;
        ui.ctx().request_repaint();
    }
    ui.separator();
    let desired = Vec2::new(ui.available_width(), 300.0);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(14, 14, 22));
    let scale = 120.0f32;
    let center = rect.center();
    let world_to_screen = |p: [f32; 2]| Pos2::new(center.x + p[0] * scale, center.y - p[1] * scale);
    if let Some(bi) = state.selected_body {
        if let Some(body) = state.bodies.get(bi) {
            if state.show_springs {
                for spring in &body.springs {
                    let a = spring.a.min(body.nodes.len().saturating_sub(1));
                    let b = spring.b.min(body.nodes.len().saturating_sub(1));
                    if a >= body.nodes.len() || b >= body.nodes.len() { continue; }
                    let pa = world_to_screen(body.nodes[a].position);
                    let pb = world_to_screen(body.nodes[b].position);
                    let stretch = if spring.rest_length > 1e-6 {
                        let dx = body.nodes[b].position[0] - body.nodes[a].position[0];
                        let dy = body.nodes[b].position[1] - body.nodes[a].position[1];
                        let l = (dx*dx+dy*dy).sqrt();
                        (l / spring.rest_length - 1.0).clamp(-1.0, 1.0)
                    } else { 0.0 };
                    let col = if stretch > 0.0 {
                        Color32::from_rgb((255.0 * stretch) as u8, (180.0 * (1.0 - stretch)) as u8, 60)
                    } else {
                        Color32::from_rgb(60, (180.0 * (1.0 + stretch)) as u8, (255.0 * (-stretch)) as u8)
                    };
                    painter.line_segment([pa, pb], Stroke::new(1.0, col));
                }
            }
            if state.show_nodes {
                for (ni, node) in body.nodes.iter().enumerate() {
                    let p = world_to_screen(node.position);
                    let pinned = body.pinned.contains(&ni);
                    let col = if pinned { Color32::from_rgb(255, 200, 80) } else { Color32::from_rgb(100, 200, 255) };
                    let r = (node.radius * scale).max(4.0);
                    painter.circle_filled(p, r, col);
                    painter.circle_stroke(p, r, Stroke::new(1.0, Color32::WHITE));
                    if state.show_forces {
                        let vp = Pos2::new(p.x + node.velocity[0] * scale * 0.1, p.y - node.velocity[1] * scale * 0.1);
                        painter.arrow(p, vp - p, Stroke::new(1.5, Color32::from_rgb(100, 255, 100)));
                    }
                }
            }
            let com = body.center_of_mass();
            let cp = world_to_screen(com);
            painter.circle_filled(cp, 3.0, Color32::from_rgb(255, 80, 80));
        }
    }
    let grid_lines = 10i32;
    for i in -grid_lines..=grid_lines {
        let x = center.x + i as f32 * scale / 2.0;
        painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(0.3, Color32::from_rgba_premultiplied(60, 60, 80, 80)));
    }
    for i in -grid_lines..=grid_lines {
        let y = center.y + i as f32 * scale / 2.0;
        painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], Stroke::new(0.3, Color32::from_rgba_premultiplied(60, 60, 80, 80)));
    }
    if resp.clicked() {
        if state.pin_mode {
            if let Some(pos) = resp.interact_pointer_pos() {
                let wx = (pos.x - center.x) / scale;
                let wy = -(pos.y - center.y) / scale;
                if let Some(bi) = state.selected_body {
                    if let Some(body) = state.bodies.get_mut(bi) {
                        let mut closest = None;
                        let mut best_dist = 0.08f32;
                        for (ni, node) in body.nodes.iter().enumerate() {
                            let dx = node.position[0] - wx;
                            let dy = node.position[1] - wy;
                            let d = (dx*dx+dy*dy).sqrt();
                            if d < best_dist { best_dist = d; closest = Some(ni); }
                        }
                        if let Some(ni) = closest {
                            if body.pinned.contains(&ni) { body.pinned.remove(&ni); }
                            else { body.pinned.insert(ni); }
                        }
                    }
                }
            }
        }
    }
}

pub fn draw_soft_body_mini_preview(painter: &Painter, rect: Rect, body: &SoftBody) {
    painter.rect_filled(rect, 2.0, Color32::from_rgb(14, 14, 22));
    if body.nodes.is_empty() { return; }
    let mut min_x = f32::MAX; let mut max_x = f32::MIN;
    let mut min_y = f32::MAX; let mut max_y = f32::MIN;
    for n in &body.nodes {
        min_x = min_x.min(n.position[0]); max_x = max_x.max(n.position[0]);
        min_y = min_y.min(n.position[1]); max_y = max_y.max(n.position[1]);
    }
    let ext_x = (max_x - min_x).max(0.01);
    let ext_y = (max_y - min_y).max(0.01);
    let scale = (rect.width() / ext_x).min(rect.height() / ext_y) * 0.85;
    let cx = rect.center().x - (min_x + max_x) * 0.5 * scale;
    let cy = rect.center().y + (min_y + max_y) * 0.5 * scale;
    let to_screen = |p: [f32; 2]| Pos2::new(cx + p[0] * scale, cy - p[1] * scale);
    for s in &body.springs {
        let a = s.a.min(body.nodes.len().saturating_sub(1));
        let b = s.b.min(body.nodes.len().saturating_sub(1));
        if a >= body.nodes.len() || b >= body.nodes.len() { continue; }
        painter.line_segment([to_screen(body.nodes[a].position), to_screen(body.nodes[b].position)],
            Stroke::new(0.8, Color32::from_rgb(80, 140, 200)));
    }
    for (ni, node) in body.nodes.iter().enumerate() {
        let p = to_screen(node.position);
        let pinned = body.pinned.contains(&ni);
        painter.circle_filled(p, 2.5, if pinned { Color32::YELLOW } else { Color32::from_rgb(100, 200, 255) });
    }
}

// ============================================================
// CLOTH SIMULATION
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClothNode {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub old_pos: [f32; 3],
}

impl ClothNode {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        ClothNode { pos: [x,y,z], vel: [0.0,0.0,0.0], old_pos: [x,y,z] }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClothSpringRef {
    pub a: usize,
    pub b: usize,
    pub rest_length: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClothMaterial {
    pub structural_stiffness: f32,
    pub shear_stiffness: f32,
    pub bend_stiffness: f32,
    pub damping: f32,
    pub mass_per_node: f32,
    pub wind_response: f32,
}

impl Default for ClothMaterial {
    fn default() -> Self {
        ClothMaterial { structural_stiffness: 0.9, shear_stiffness: 0.5, bend_stiffness: 0.3, damping: 0.98, mass_per_node: 0.1, wind_response: 1.0 }
    }
}

impl ClothMaterial {
    pub fn silk() -> Self { ClothMaterial { structural_stiffness: 0.7, shear_stiffness: 0.3, bend_stiffness: 0.1, damping: 0.99, mass_per_node: 0.05, wind_response: 1.5 } }
    pub fn denim() -> Self { ClothMaterial { structural_stiffness: 0.98, shear_stiffness: 0.8, bend_stiffness: 0.7, damping: 0.95, mass_per_node: 0.3, wind_response: 0.6 } }
    pub fn rubber_sheet() -> Self { ClothMaterial { structural_stiffness: 0.5, shear_stiffness: 0.6, bend_stiffness: 0.2, damping: 0.92, mass_per_node: 0.8, wind_response: 0.4 } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClothMesh {
    pub width: u32,
    pub height: u32,
    pub nodes: Vec<ClothNode>,
    pub structural_springs: Vec<ClothSpringRef>,
    pub shear_springs: Vec<ClothSpringRef>,
    pub bend_springs: Vec<ClothSpringRef>,
    pub pinned: HashSet<(u32, u32)>,
    pub material: ClothMaterial,
    pub name: String,
}

impl ClothMesh {
    pub fn new(width: u32, height: u32, cell_size: f32) -> Self {
        let mut nodes = Vec::new();
        let mut structural = Vec::new();
        let mut shear = Vec::new();
        let mut bend = Vec::new();
        let mut pinned = HashSet::new();
        let w = width as usize;
        let h = height as usize;
        for row in 0..=h {
            for col in 0..=w {
                let x = col as f32 * cell_size;
                let y = row as f32 * cell_size;
                nodes.push(ClothNode::new(x, 0.0, y));
            }
        }
        for col in 0..=w { pinned.insert((0, col as u32)); }
        let idx = |r: usize, c: usize| r * (w + 1) + c;
        for r in 0..=h { for c in 0..w {
            let i = idx(r,c); let j = idx(r,c+1);
            let dx = nodes[j].pos[0]-nodes[i].pos[0]; let dz = nodes[j].pos[2]-nodes[i].pos[2];
            structural.push(ClothSpringRef { a: i, b: j, rest_length: (dx*dx+dz*dz).sqrt() });
        }}
        for r in 0..h { for c in 0..=w {
            let i = idx(r,c); let j = idx(r+1,c);
            let dx = nodes[j].pos[0]-nodes[i].pos[0]; let dz = nodes[j].pos[2]-nodes[i].pos[2];
            structural.push(ClothSpringRef { a: i, b: j, rest_length: (dx*dx+dz*dz).sqrt() });
        }}
        for r in 0..h { for c in 0..w {
            let i=idx(r,c); let j=idx(r+1,c+1);
            let dx=nodes[j].pos[0]-nodes[i].pos[0]; let dz=nodes[j].pos[2]-nodes[i].pos[2];
            shear.push(ClothSpringRef { a: i, b: j, rest_length: (dx*dx+dz*dz).sqrt() });
            let i2=idx(r,c+1); let j2=idx(r+1,c);
            let dx2=nodes[j2].pos[0]-nodes[i2].pos[0]; let dz2=nodes[j2].pos[2]-nodes[i2].pos[2];
            shear.push(ClothSpringRef { a: i2, b: j2, rest_length: (dx2*dx2+dz2*dz2).sqrt() });
        }}
        for r in 0..=h { for c in 0..w.saturating_sub(1) {
            let i=idx(r,c); let j=idx(r,c+2);
            let dx=nodes[j].pos[0]-nodes[i].pos[0]; let dz=nodes[j].pos[2]-nodes[i].pos[2];
            bend.push(ClothSpringRef { a: i, b: j, rest_length: (dx*dx+dz*dz).sqrt() });
        }}
        for r in 0..h.saturating_sub(1) { for c in 0..=w {
            let i=idx(r,c); let j=idx(r+2,c);
            let dx=nodes[j].pos[0]-nodes[i].pos[0]; let dz=nodes[j].pos[2]-nodes[i].pos[2];
            bend.push(ClothSpringRef { a: i, b: j, rest_length: (dx*dx+dz*dz).sqrt() });
        }}
        ClothMesh { width, height, nodes, structural_springs: structural, shear_springs: shear, bend_springs: bend, pinned, material: ClothMaterial::default(), name: "Cloth".into() }
    }
    pub fn node_idx(&self, row: u32, col: u32) -> usize { row as usize * (self.width as usize + 1) + col as usize }
    pub fn is_pinned(&self, row: u32, col: u32) -> bool { self.pinned.contains(&(row, col)) }
}

pub fn simulate_cloth(cloth: &mut ClothMesh, dt: f32, gravity: [f32; 3], wind: [f32; 3]) {
    let n = cloth.nodes.len();
    if n == 0 { return; }
    let w = cloth.width as usize;
    let h = cloth.height as usize;
    let mat = cloth.material.clone();
    for r in 0..=h {
        for c in 0..=w {
            let idx = r * (w+1) + c;
            if cloth.pinned.contains(&(r as u32, c as u32)) { continue; }
            let old = cloth.nodes[idx].old_pos;
            let cur = cloth.nodes[idx].pos;
            let wf = [wind[0]*mat.wind_response, wind[1]*mat.wind_response, wind[2]*mat.wind_response];
            let nx = cur[0] + (cur[0]-old[0])*mat.damping + (gravity[0]+wf[0])*dt*dt;
            let ny = cur[1] + (cur[1]-old[1])*mat.damping + (gravity[1]+wf[1])*dt*dt;
            let nz = cur[2] + (cur[2]-old[2])*mat.damping + (gravity[2]+wf[2])*dt*dt;
            cloth.nodes[idx].old_pos = cur;
            cloth.nodes[idx].pos = [nx, ny, nz];
        }
    }
    let ss = mat.structural_stiffness;
    let shs = mat.shear_stiffness;
    let bs = mat.bend_stiffness;
    for _ in 0..10 {
        let springs: Vec<ClothSpringRef> = cloth.structural_springs.clone();
        for s in &springs { relax_cloth_spring(&mut cloth.nodes, &cloth.pinned, cloth.width, s, ss); }
        let springs: Vec<ClothSpringRef> = cloth.shear_springs.clone();
        for s in &springs { relax_cloth_spring(&mut cloth.nodes, &cloth.pinned, cloth.width, s, shs); }
        let springs: Vec<ClothSpringRef> = cloth.bend_springs.clone();
        for s in &springs { relax_cloth_spring(&mut cloth.nodes, &cloth.pinned, cloth.width, s, bs); }
    }
    for node in &mut cloth.nodes {
        node.vel[0] = (node.pos[0]-node.old_pos[0])/dt;
        node.vel[1] = (node.pos[1]-node.old_pos[1])/dt;
        node.vel[2] = (node.pos[2]-node.old_pos[2])/dt;
    }
}

fn relax_cloth_spring(nodes: &mut Vec<ClothNode>, pinned: &HashSet<(u32,u32)>, width: u32, s: &ClothSpringRef, stiffness: f32) {
    let a = s.a.min(nodes.len().saturating_sub(1));
    let b = s.b.min(nodes.len().saturating_sub(1));
    if a >= nodes.len() || b >= nodes.len() || a == b { return; }
    let dx = nodes[b].pos[0]-nodes[a].pos[0];
    let dy = nodes[b].pos[1]-nodes[a].pos[1];
    let dz = nodes[b].pos[2]-nodes[a].pos[2];
    let len = (dx*dx+dy*dy+dz*dz).sqrt().max(1e-8);
    let diff = (len - s.rest_length) / len * stiffness * 0.5;
    let wa = width + 1;
    let ra = (a / wa as usize) as u32;
    let ca = (a % wa as usize) as u32;
    let rb = (b / wa as usize) as u32;
    let cb = (b % wa as usize) as u32;
    let ap = pinned.contains(&(ra,ca));
    let bp = pinned.contains(&(rb,cb));
    if !ap { nodes[a].pos[0] += dx*diff; nodes[a].pos[1] += dy*diff; nodes[a].pos[2] += dz*diff; }
    if !bp { nodes[b].pos[0] -= dx*diff; nodes[b].pos[1] -= dy*diff; nodes[b].pos[2] -= dz*diff; }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClothEditorState {
    pub meshes: Vec<ClothMesh>,
    pub selected: Option<usize>,
    pub simulating: bool,
    pub gravity: [f32; 3],
    pub wind: [f32; 3],
    pub wind_direction: f32,
    pub wind_strength: f32,
    pub sim_time: f32,
    pub show_wireframe: bool,
}

impl ClothEditorState {
    pub fn new() -> Self { ClothEditorState { gravity: [0.0,-9.81,0.0], wind_strength: 0.0, wind_direction: 0.0, show_wireframe: true, ..Default::default() } }
}

pub fn show_cloth_editor(ui: &mut egui::Ui, state: &mut ClothEditorState) {
    ui.heading(RichText::new("Cloth Simulation").color(Color32::from_rgb(200, 255, 200)));
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button(if state.simulating { "Pause" } else { "Play" }).clicked() { state.simulating = !state.simulating; }
        if ui.button("Reset").clicked() { state.sim_time = 0.0; }
        if ui.button("Add 8x8 Cloth").clicked() { state.meshes.push(ClothMesh::new(8, 8, 0.1)); state.selected = Some(state.meshes.len()-1); }
        if ui.button("Add 16x16 Cloth").clicked() { state.meshes.push(ClothMesh::new(16, 16, 0.05)); state.selected = Some(state.meshes.len()-1); }
    });
    ui.separator();
    egui::CollapsingHeader::new(RichText::new("Wind Controls").color(Color32::from_rgb(180,220,255))).default_open(true).show(ui, |ui| {
        ui.horizontal(|ui| { ui.label("Direction:"); ui.add(egui::Slider::new(&mut state.wind_direction, 0.0f32..=360.0).suffix("°")); });
        ui.horizontal(|ui| { ui.label("Strength:"); ui.add(egui::Slider::new(&mut state.wind_strength, 0.0f32..=20.0)); });
        state.wind[0] = state.wind_direction.to_radians().cos() * state.wind_strength;
        state.wind[2] = state.wind_direction.to_radians().sin() * state.wind_strength;
        draw_wind_compass(ui, state.wind_direction, state.wind_strength);
    });
    if let Some(si) = state.selected {
        if let Some(mesh) = state.meshes.get_mut(si) {
            egui::CollapsingHeader::new(RichText::new("Cloth Material").color(Color32::from_rgb(255,200,180))).default_open(false).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Silk").clicked() { mesh.material = ClothMaterial::silk(); }
                    if ui.button("Denim").clicked() { mesh.material = ClothMaterial::denim(); }
                    if ui.button("Rubber").clicked() { mesh.material = ClothMaterial::rubber_sheet(); }
                    if ui.button("Default").clicked() { mesh.material = ClothMaterial::default(); }
                });
                egui::Grid::new("cloth_mat").num_columns(2).spacing(Vec2::new(8.0,4.0)).show(ui, |ui| {
                    ui.label("Structural K:"); ui.add(egui::Slider::new(&mut mesh.material.structural_stiffness, 0.1f32..=1.0)); ui.end_row();
                    ui.label("Shear K:"); ui.add(egui::Slider::new(&mut mesh.material.shear_stiffness, 0.1f32..=1.0)); ui.end_row();
                    ui.label("Bend K:"); ui.add(egui::Slider::new(&mut mesh.material.bend_stiffness, 0.0f32..=1.0)); ui.end_row();
                    ui.label("Damping:"); ui.add(egui::Slider::new(&mut mesh.material.damping, 0.85f32..=1.0)); ui.end_row();
                    ui.label("Wind response:"); ui.add(egui::Slider::new(&mut mesh.material.wind_response, 0.0f32..=3.0)); ui.end_row();
                });
            });
        }
    }
    if state.simulating {
        let dt = 1.0/60.0; let gravity = state.gravity; let wind = state.wind;
        for mesh in &mut state.meshes { simulate_cloth(mesh, dt, gravity, wind); }
        state.sim_time += dt;
        ui.ctx().request_repaint();
    }
    ui.separator();
    ui.label(RichText::new("Top-Down Preview (XZ plane)").small().color(Color32::GRAY));
    let desired = Vec2::new(ui.available_width(), 250.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(12, 16, 20));
    if let Some(si) = state.selected {
        if let Some(mesh) = state.meshes.get(si) {
            let w = mesh.width as usize; let h = mesh.height as usize; let wa = w+1;
            let scale = (rect.width()/((w+2) as f32 * 0.15)).min(rect.height()/((h+2) as f32 * 0.15));
            let ox = rect.center().x - (w as f32 * 0.15 * scale * 0.5);
            let oy = rect.top() + 20.0;
            for row in 0..h { for col in 0..w {
                let i00=row*wa+col; let i10=row*wa+col+1; let i01=(row+1)*wa+col; let i11=(row+1)*wa+col+1;
                if i11 >= mesh.nodes.len() { continue; }
                let p00=Pos2::new(ox+mesh.nodes[i00].pos[0]*scale, oy+mesh.nodes[i00].pos[2]*scale);
                let p10=Pos2::new(ox+mesh.nodes[i10].pos[0]*scale, oy+mesh.nodes[i10].pos[2]*scale);
                let p01=Pos2::new(ox+mesh.nodes[i01].pos[0]*scale, oy+mesh.nodes[i01].pos[2]*scale);
                let p11=Pos2::new(ox+mesh.nodes[i11].pos[0]*scale, oy+mesh.nodes[i11].pos[2]*scale);
                let pinned_any = mesh.pinned.contains(&(row as u32, col as u32));
                let vy = mesh.nodes[i00].vel[1].abs().min(5.0)/5.0;
                let col_fill = if pinned_any { Color32::from_rgba_premultiplied(255,200,80,60) }
                    else { Color32::from_rgba_premultiplied((20.0+vy*80.0) as u8, (80.0+vy*120.0) as u8, (150.0+vy*80.0) as u8, 80) };
                painter.add(Shape::convex_polygon(vec![p00,p10,p11,p01], col_fill, Stroke::new(0.5, Color32::from_rgb(60,100,120))));
            }}
        }
    }
    painter.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP,
        format!("t={:.2}s  meshes={}", state.sim_time, state.meshes.len()), FontId::monospace(9.0), Color32::GRAY);
}

fn draw_wind_compass(ui: &mut egui::Ui, direction_deg: f32, strength: f32) {
    let size = Vec2::new(60.0, 60.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.circle_stroke(rect.center(), 25.0, Stroke::new(1.0, Color32::from_rgb(60,80,100)));
    let angle = direction_deg.to_radians() - std::f32::consts::FRAC_PI_2;
    let len = (strength / 20.0).clamp(0.0, 1.0) * 20.0 + 5.0;
    let tip = Pos2::new(rect.center().x + len*angle.cos(), rect.center().y + len*angle.sin());
    painter.arrow(rect.center(), tip - rect.center(), Stroke::new(2.0, Color32::from_rgb(100,180,255)));
    for (label, deg) in [("N",0.0f32),("E",90.0),("S",180.0),("W",270.0)] {
        let a = deg.to_radians() - std::f32::consts::FRAC_PI_2;
        let p = Pos2::new(rect.center().x + 21.0*a.cos(), rect.center().y + 21.0*a.sin());
        painter.text(p, egui::Align2::CENTER_CENTER, label, FontId::monospace(7.0), Color32::from_rgb(140,140,160));
    }
}

// ============================================================
// ROPE / CHAIN SIMULATION
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RopeNode {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub mass: f32,
    pub pinned: bool,
}

impl RopeNode {
    pub fn new(x: f32, y: f32) -> Self { RopeNode { pos: [x,y], vel: [0.0,0.0], mass: 0.1, pinned: false } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RopeConstraint { pub a: usize, pub b: usize, pub rest_length: f32 }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum RopeMaterial { Rubber, Steel, String, Chain, Bungee }

impl RopeMaterial {
    pub fn label(&self) -> &'static str {
        match self { RopeMaterial::Rubber=>"Rubber", RopeMaterial::Steel=>"Steel", RopeMaterial::String=>"String", RopeMaterial::Chain=>"Chain", RopeMaterial::Bungee=>"Bungee" }
    }
    pub fn damping(&self) -> f32 { match self { RopeMaterial::Rubber=>0.93, RopeMaterial::Steel=>0.99, RopeMaterial::String=>0.97, RopeMaterial::Chain=>0.96, RopeMaterial::Bungee=>0.90 } }
    pub fn stiffness_iters(&self) -> u32 { match self { RopeMaterial::Rubber=>10, RopeMaterial::Steel=>30, RopeMaterial::String=>20, RopeMaterial::Chain=>25, RopeMaterial::Bungee=>5 } }
    pub fn color(&self) -> Color32 { match self { RopeMaterial::Rubber=>Color32::from_rgb(60,60,60), RopeMaterial::Steel=>Color32::from_rgb(180,180,200), RopeMaterial::String=>Color32::from_rgb(200,180,120), RopeMaterial::Chain=>Color32::from_rgb(160,160,180), RopeMaterial::Bungee=>Color32::from_rgb(255,80,80) } }
    pub fn all() -> &'static [RopeMaterial] { &[RopeMaterial::Rubber, RopeMaterial::Steel, RopeMaterial::String, RopeMaterial::Chain, RopeMaterial::Bungee] }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rope {
    pub nodes: Vec<RopeNode>,
    pub constraints: Vec<RopeConstraint>,
    pub segment_length: f32,
    pub total_length: f32,
    pub material: RopeMaterial,
    pub name: String,
    pub thickness: f32,
}

impl Rope {
    pub fn new(name: &str, start: [f32;2], end: [f32;2], segments: usize, material: RopeMaterial) -> Self {
        let mut nodes = Vec::new();
        let mut constraints = Vec::new();
        let dx = end[0]-start[0]; let dy = end[1]-start[1];
        let total = (dx*dx+dy*dy).sqrt();
        let seg = total/segments as f32;
        for i in 0..=segments {
            let t = i as f32/segments as f32;
            nodes.push(RopeNode::new(start[0]+dx*t, start[1]+dy*t));
        }
        nodes[0].pinned = true;
        for i in 0..segments {
            let dx2=nodes[i+1].pos[0]-nodes[i].pos[0]; let dy2=nodes[i+1].pos[1]-nodes[i].pos[1];
            constraints.push(RopeConstraint { a:i, b:i+1, rest_length: (dx2*dx2+dy2*dy2).sqrt() });
        }
        Rope { nodes, constraints, segment_length: seg, total_length: total, material, name: name.into(), thickness: 4.0 }
    }
    pub fn preset_pendulum(pivot: [f32;2], length: f32, segs: usize) -> Self {
        let end = [pivot[0], pivot[1]-length];
        let mut r = Rope::new("Pendulum", pivot, end, segs, RopeMaterial::String);
        if let Some(n) = r.nodes.last_mut() { n.mass = 1.0; }
        r
    }
    pub fn preset_bridge(left: [f32;2], right: [f32;2], sag: f32, segs: usize) -> Self {
        let mut nodes = Vec::new(); let mut constraints = Vec::new();
        let dx=right[0]-left[0]; let dy=right[1]-left[1];
        let len = (dx*dx+dy*dy).sqrt();
        for i in 0..=segs {
            let t = i as f32/segs as f32;
            let x = left[0]+dx*t;
            let parab = sag*4.0*t*(1.0-t);
            let y = left[1]+dy*t - parab;
            nodes.push(RopeNode::new(x, y));
        }
        nodes[0].pinned = true; nodes[segs].pinned = true;
        for i in 0..segs {
            let dx2=nodes[i+1].pos[0]-nodes[i].pos[0]; let dy2=nodes[i+1].pos[1]-nodes[i].pos[1];
            constraints.push(RopeConstraint { a:i, b:i+1, rest_length: (dx2*dx2+dy2*dy2).sqrt() });
        }
        Rope { nodes, constraints, segment_length: len/segs as f32, total_length: len, material: RopeMaterial::Steel, name: "Bridge".into(), thickness: 3.0 }
    }
}

pub fn simulate_rope(rope: &mut Rope, dt: f32, gravity: [f32;2]) {
    let n = rope.nodes.len();
    if n == 0 { return; }
    let damping = rope.material.damping();
    let iters = rope.material.stiffness_iters().max(20);
    for node in &mut rope.nodes {
        if node.pinned { node.vel = [0.0,0.0]; continue; }
        node.vel[0] = (node.vel[0]+gravity[0]*dt)*damping;
        node.vel[1] = (node.vel[1]+gravity[1]*dt)*damping;
        node.pos[0] += node.vel[0]*dt;
        node.pos[1] += node.vel[1]*dt;
    }
    for _ in 0..iters {
        let constraints: Vec<RopeConstraint> = rope.constraints.clone();
        for c in &constraints {
            let a = c.a.min(n-1); let b = c.b.min(n-1);
            if a >= n || b >= n { continue; }
            let dx=rope.nodes[b].pos[0]-rope.nodes[a].pos[0];
            let dy=rope.nodes[b].pos[1]-rope.nodes[a].pos[1];
            let len=(dx*dx+dy*dy).sqrt().max(1e-8);
            let diff=(len-c.rest_length)/len;
            let ap=rope.nodes[a].pinned; let bp=rope.nodes[b].pinned;
            if !ap && !bp {
                rope.nodes[a].pos[0]+=dx*diff*0.5; rope.nodes[a].pos[1]+=dy*diff*0.5;
                rope.nodes[b].pos[0]-=dx*diff*0.5; rope.nodes[b].pos[1]-=dy*diff*0.5;
            } else if !ap { rope.nodes[a].pos[0]+=dx*diff; rope.nodes[a].pos[1]+=dy*diff; }
            else if !bp { rope.nodes[b].pos[0]-=dx*diff; rope.nodes[b].pos[1]-=dy*diff; }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RopeEditorState {
    pub ropes: Vec<Rope>,
    pub selected: Option<usize>,
    pub simulating: bool,
    pub gravity: [f32;2],
    pub sim_time: f32,
    pub show_indices: bool,
    pub material_selection: usize,
}

impl RopeEditorState {
    pub fn new() -> Self { RopeEditorState { gravity:[0.0,-9.81], material_selection:2, ..Default::default() } }
}

pub fn show_rope_editor(ui: &mut egui::Ui, state: &mut RopeEditorState) {
    ui.heading(RichText::new("Rope / Chain Simulation").color(Color32::from_rgb(255,200,140)));
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button(if state.simulating { "Pause" } else { "Play" }).clicked() { state.simulating = !state.simulating; }
        if ui.button("Reset").clicked() { state.sim_time = 0.0; for r in &mut state.ropes { for n in &mut r.nodes { n.vel=[0.0,0.0]; } } }
    });
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label("Material:");
        for (i, mat) in RopeMaterial::all().iter().enumerate() {
            if ui.selectable_label(state.material_selection==i, mat.label()).clicked() { state.material_selection=i; }
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Add Rope").clicked() {
            let mat = RopeMaterial::all()[state.material_selection.min(RopeMaterial::all().len()-1)].clone();
            state.ropes.push(Rope::new("Rope", [-0.5,0.5], [0.5,0.5], 16, mat));
            state.selected = Some(state.ropes.len()-1);
        }
        if ui.button("Add Pendulum").clicked() { state.ropes.push(Rope::preset_pendulum([0.0,1.0],1.0,12)); state.selected=Some(state.ropes.len()-1); }
        if ui.button("Add Bridge").clicked() { state.ropes.push(Rope::preset_bridge([-1.0,0.0],[1.0,0.0],0.3,20)); state.selected=Some(state.ropes.len()-1); }
    });
    if let Some(ri) = state.selected {
        if let Some(rope) = state.ropes.get_mut(ri) {
            ui.separator();
            egui::Grid::new("rope_props").num_columns(2).spacing(Vec2::new(8.0,4.0)).show(ui, |ui| {
                ui.label("Name:"); ui.text_edit_singleline(&mut rope.name); ui.end_row();
                ui.label("Thickness:"); ui.add(egui::DragValue::new(&mut rope.thickness).speed(0.1).clamp_range(1.0f32..=20.0).suffix("px")); ui.end_row();
                ui.label("Segments:"); ui.label(format!("{}", rope.nodes.len()-1)); ui.end_row();
            });
            ui.horizontal(|ui| {
                ui.label("Pin A:");
                let pa = rope.nodes.first().map(|n| n.pinned).unwrap_or(false);
                let mut pa2 = pa;
                if ui.checkbox(&mut pa2,"").changed() { if let Some(n)=rope.nodes.first_mut(){n.pinned=pa2;} }
                ui.label("  Pin B:");
                let pb = rope.nodes.last().map(|n| n.pinned).unwrap_or(false);
                let mut pb2 = pb;
                if ui.checkbox(&mut pb2,"").changed() { if let Some(n)=rope.nodes.last_mut(){n.pinned=pb2;} }
            });
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Gravity:");
        ui.add(egui::DragValue::new(&mut state.gravity[0]).speed(0.1).prefix("x:"));
        ui.add(egui::DragValue::new(&mut state.gravity[1]).speed(0.1).prefix("y:"));
        if ui.small_button("Earth").clicked() { state.gravity=[0.0,-9.81]; }
    });
    if state.simulating {
        let dt=1.0/120.0; let g=state.gravity;
        for rope in &mut state.ropes { simulate_rope(rope, dt, g); }
        state.sim_time += dt; ui.ctx().request_repaint();
    }
    ui.separator();
    let desired = Vec2::new(ui.available_width(), 280.0);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(14,14,22));
    let scale=100.0f32; let center=rect.center();
    let to_screen = |p:[f32;2]| Pos2::new(center.x+p[0]*scale, center.y-p[1]*scale);
    for (ri, rope) in state.ropes.iter().enumerate() {
        let col = rope.material.color();
        let sel = state.selected==Some(ri);
        let th = if sel { rope.thickness+1.0 } else { rope.thickness };
        for i in 0..rope.nodes.len().saturating_sub(1) {
            let a=to_screen(rope.nodes[i].pos); let b=to_screen(rope.nodes[i+1].pos);
            if matches!(rope.material, RopeMaterial::Chain) {
                let mid=Pos2::new((a.x+b.x)*0.5,(a.y+b.y)*0.5);
                painter.line_segment([a,mid],Stroke::new(th,col));
                painter.circle_stroke(mid,th*0.6,Stroke::new(1.0,col));
                painter.line_segment([mid,b],Stroke::new(th,col));
            } else { painter.line_segment([a,b],Stroke::new(th,col)); }
        }
        for node in &rope.nodes {
            let p=to_screen(node.pos);
            if node.pinned { painter.rect_filled(Rect::from_center_size(p,Vec2::splat(8.0)),2.0,Color32::YELLOW); }
        }
    }
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let wx=(pos.x-center.x)/scale; let wy=-(pos.y-center.y)/scale;
            let mut best=None; let mut bd=0.2f32;
            for (ri,rope) in state.ropes.iter().enumerate() {
                for node in &rope.nodes { let dx=node.pos[0]-wx; let dy=node.pos[1]-wy; let d=(dx*dx+dy*dy).sqrt(); if d<bd{bd=d;best=Some(ri);} }
            }
            if let Some(idx)=best{state.selected=Some(idx);}
        }
    }
    painter.text(Pos2::new(rect.left()+4.0,rect.bottom()-12.0),egui::Align2::LEFT_BOTTOM,
        format!("t={:.2}s  ropes={}",state.sim_time,state.ropes.len()),FontId::monospace(8.0),Color32::GRAY);
}

// ============================================================
// FLUID SIMULATION (2D SPH)
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FluidParticle {
    pub pos: [f32;2], pub vel: [f32;2], pub density: f32, pub pressure: f32, pub mass: f32,
}
impl FluidParticle {
    pub fn new(x: f32, y: f32) -> Self { FluidParticle { pos:[x,y], vel:[0.0,0.0], density:1.0, pressure:0.0, mass:1.0 } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FluidParams {
    pub particle_radius: f32, pub rest_density: f32, pub gas_constant: f32,
    pub viscosity: f32, pub surface_tension: f32, pub gravity: [f32;2],
    pub boundary_damping: f32, pub smoothing_radius: f32,
    pub boundary_min: [f32;2], pub boundary_max: [f32;2],
}
impl Default for FluidParams {
    fn default() -> Self { FluidParams { particle_radius:0.05, rest_density:1000.0, gas_constant:2.0, viscosity:0.1, surface_tension:0.0728, gravity:[0.0,-9.81], boundary_damping:0.3, smoothing_radius:0.12, boundary_min:[-1.0,-1.0], boundary_max:[1.0,1.0] } }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SpatialGrid { pub cell_size: f32, pub cells: HashMap<(i32,i32), Vec<usize>> }
impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self { SpatialGrid { cell_size, cells: HashMap::new() } }
    pub fn clear(&mut self) { self.cells.clear(); }
    pub fn insert(&mut self, idx: usize, pos: [f32;2]) {
        let cx=(pos[0]/self.cell_size).floor() as i32;
        let cy=(pos[1]/self.cell_size).floor() as i32;
        self.cells.entry((cx,cy)).or_default().push(idx);
    }
    pub fn neighbors(&self, pos: [f32;2], radius: f32) -> Vec<usize> {
        let r=(radius/self.cell_size).ceil() as i32;
        let cx=(pos[0]/self.cell_size).floor() as i32;
        let cy=(pos[1]/self.cell_size).floor() as i32;
        let mut result=Vec::new();
        for dx in -r..=r { for dy in -r..=r {
            if let Some(ids)=self.cells.get(&(cx+dx,cy+dy)) { result.extend_from_slice(ids); }
        }}
        result
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FluidEmitter {
    pub position: [f32;2], pub direction: [f32;2], pub velocity: f32,
    pub rate: f32, pub emit_timer: f32, pub enabled: bool,
}
impl FluidEmitter {
    pub fn new(pos: [f32;2]) -> Self { FluidEmitter { position:pos, direction:[0.0,-1.0], velocity:1.0, rate:20.0, emit_timer:0.0, enabled:true } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FluidSim {
    pub particles: Vec<FluidParticle>,
    pub grid: SpatialGrid,
    pub params: FluidParams,
    pub emitters: Vec<FluidEmitter>,
    pub max_particles: usize,
    pub sim_time: f32,
}
impl FluidSim {
    pub fn new(params: FluidParams) -> Self {
        let h=params.smoothing_radius;
        FluidSim { particles:Vec::new(), grid:SpatialGrid::new(h), params, emitters:Vec::new(), max_particles:500, sim_time:0.0 }
    }
    fn sph_poly6(r: f32, h: f32) -> f32 {
        if r>h{return 0.0;} let x=h*h-r*r;
        315.0/(64.0*std::f32::consts::PI*h.powi(9))*x*x*x
    }
    fn sph_spiky_grad(r: f32, h: f32) -> f32 {
        if r>h||r<1e-8{return 0.0;} let x=h-r;
        -45.0/(std::f32::consts::PI*h.powi(6))*x*x
    }
    fn sph_visc_lap(r: f32, h: f32) -> f32 {
        if r>h{return 0.0;} 45.0/(std::f32::consts::PI*h.powi(6))*(h-r)
    }
}

pub fn simulate_fluid(sim: &mut FluidSim, dt: f32) {
    let n=sim.particles.len(); if n==0{return;}
    let h=sim.params.smoothing_radius; let k=sim.params.gas_constant;
    let rho0=sim.params.rest_density; let mu=sim.params.viscosity;
    let g=sim.params.gravity; let bmin=sim.params.boundary_min; let bmax=sim.params.boundary_max;
    let bdamp=sim.params.boundary_damping;
    sim.grid.clear();
    for (i,p) in sim.particles.iter().enumerate(){sim.grid.insert(i,p.pos);}
    let positions:Vec<[f32;2]>=sim.particles.iter().map(|p|p.pos).collect();
    let masses:Vec<f32>=sim.particles.iter().map(|p|p.mass).collect();
    let mut densities=vec![0.0f32;n];
    for i in 0..n {
        let nbrs=sim.grid.neighbors(positions[i],h);
        for j in &nbrs {
            let j=*j; if j>=n{continue;}
            let dx=positions[i][0]-positions[j][0]; let dy=positions[i][1]-positions[j][1];
            let r=(dx*dx+dy*dy).sqrt();
            densities[i]+=masses[j]*FluidSim::sph_poly6(r,h);
        }
        densities[i]=densities[i].max(rho0*0.01);
        sim.particles[i].density=densities[i];
        sim.particles[i].pressure=k*(densities[i]-rho0).max(0.0);
    }
    let velocities:Vec<[f32;2]>=sim.particles.iter().map(|p|p.vel).collect();
    let pressures:Vec<f32>=sim.particles.iter().map(|p|p.pressure).collect();
    let mut forces=vec![[0.0f32;2];n];
    for i in 0..n {
        let nbrs=sim.grid.neighbors(positions[i],h);
        let mut fp=[0.0f32;2]; let mut fv=[0.0f32;2];
        for j in &nbrs {
            let j=*j; if j==i||j>=n{continue;}
            let dx=positions[i][0]-positions[j][0]; let dy=positions[i][1]-positions[j][1];
            let r=(dx*dx+dy*dy).sqrt().max(1e-8);
            let nx=dx/r; let ny=dy/r;
            let pm=masses[j]*(pressures[i]+pressures[j])/(2.0*densities[j].max(1e-6));
            let grad=FluidSim::sph_spiky_grad(r,h);
            fp[0]-=pm*grad*nx; fp[1]-=pm*grad*ny;
            let lap=FluidSim::sph_visc_lap(r,h);
            let dvf=masses[j]/densities[j].max(1e-6)*lap;
            fv[0]+=(velocities[j][0]-velocities[i][0])*dvf;
            fv[1]+=(velocities[j][1]-velocities[i][1])*dvf;
        }
        let inv=1.0/densities[i].max(1e-6);
        forces[i][0]=fp[0]*inv+mu*fv[0]*inv+g[0]*masses[i];
        forces[i][1]=fp[1]*inv+mu*fv[1]*inv+g[1]*masses[i];
    }
    for i in 0..n {
        let im=1.0/sim.particles[i].mass.max(1e-6);
        sim.particles[i].vel[0]+=forces[i][0]*im*dt;
        sim.particles[i].vel[1]+=forces[i][1]*im*dt;
        sim.particles[i].pos[0]+=sim.particles[i].vel[0]*dt;
        sim.particles[i].pos[1]+=sim.particles[i].vel[1]*dt;
        if sim.particles[i].pos[0]<bmin[0]{sim.particles[i].pos[0]=bmin[0];sim.particles[i].vel[0]*=-bdamp;}
        if sim.particles[i].pos[0]>bmax[0]{sim.particles[i].pos[0]=bmax[0];sim.particles[i].vel[0]*=-bdamp;}
        if sim.particles[i].pos[1]<bmin[1]{sim.particles[i].pos[1]=bmin[1];sim.particles[i].vel[1]*=-bdamp;}
        if sim.particles[i].pos[1]>bmax[1]{sim.particles[i].pos[1]=bmax[1];sim.particles[i].vel[1]*=-bdamp;}
    }
    let max_p=sim.max_particles;
    for emitter in &mut sim.emitters {
        if !emitter.enabled{continue;}
        emitter.emit_timer+=dt;
        let interval=1.0/emitter.rate.max(0.1);
        while emitter.emit_timer>=interval && sim.particles.len()<max_p {
            emitter.emit_timer-=interval;
            let mut p=FluidParticle::new(emitter.position[0],emitter.position[1]);
            p.vel=[emitter.direction[0]*emitter.velocity, emitter.direction[1]*emitter.velocity];
            sim.particles.push(p);
        }
    }
    sim.sim_time+=dt;
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FluidEditorState {
    pub sim: Option<FluidSim>,
    pub simulating: bool,
    pub show_density: bool,
    pub show_velocity: bool,
    pub particle_size: f32,
}
impl FluidEditorState {
    pub fn new() -> Self { FluidEditorState { show_density:true, particle_size:4.0, ..Default::default() } }
}

pub fn show_fluid_editor(ui: &mut egui::Ui, state: &mut FluidEditorState) {
    ui.heading(RichText::new("Fluid Simulation (SPH)").color(Color32::from_rgb(100,180,255)));
    ui.separator();
    ui.horizontal(|ui| {
        if state.sim.is_none() {
            if ui.button("Initialize Fluid").clicked() {
                let mut params = FluidParams::default();
                params.boundary_min=[-1.0,-1.5]; params.boundary_max=[1.0,0.5];
                let mut sim=FluidSim::new(params);
                let mut y=-0.5f32;
                while y<0.4 {
                    let mut x=-0.8f32;
                    while x<0.8 {
                        if sim.particles.len()<sim.max_particles { sim.particles.push(FluidParticle::new(x,y)); }
                        x+=0.08;
                    }
                    y+=0.08;
                }
                sim.emitters.push(FluidEmitter::new([-0.5,0.3]));
                state.sim=Some(sim);
            }
        } else {
            if ui.button(if state.simulating{"Pause"}else{"Play"}).clicked(){state.simulating=!state.simulating;}
            if ui.button("Reset").clicked(){state.sim=None; state.simulating=false;}
        }
    });
    if let Some(sim) = &mut state.sim {
        ui.separator();
        egui::Grid::new("fluid_params").num_columns(2).spacing(Vec2::new(8.0,4.0)).show(ui, |ui| {
            ui.label("Rest density:"); ui.add(egui::DragValue::new(&mut sim.params.rest_density).speed(10.0).clamp_range(100.0f32..=2000.0)); ui.end_row();
            ui.label("Gas constant:"); ui.add(egui::DragValue::new(&mut sim.params.gas_constant).speed(0.1).clamp_range(0.1f32..=20.0)); ui.end_row();
            ui.label("Viscosity:"); ui.add(egui::DragValue::new(&mut sim.params.viscosity).speed(0.01).clamp_range(0.0f32..=2.0)); ui.end_row();
            ui.label("Max particles:"); ui.add(egui::DragValue::new(&mut sim.max_particles).speed(10).clamp_range(10usize..=2000)); ui.end_row();
            ui.label("Particle count:"); ui.label(format!("{}",sim.particles.len())); ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.show_density,"Density colors");
            ui.checkbox(&mut state.show_velocity,"Velocity");
            ui.label("Size:"); ui.add(egui::Slider::new(&mut state.particle_size,1.0f32..=12.0));
        });
        if state.simulating { simulate_fluid(sim, 1.0/60.0); ui.ctx().request_repaint(); }
    }
    ui.separator();
    let desired=Vec2::new(ui.available_width(),320.0);
    let (rect, resp)=ui.allocate_exact_size(desired, egui::Sense::click());
    let painter=ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(10,14,24));
    if let Some(sim) = &state.sim {
        let bmin=sim.params.boundary_min; let bmax=sim.params.boundary_max;
        let bw=bmax[0]-bmin[0]; let bh=bmax[1]-bmin[1];
        let scale=(rect.width()/bw).min(rect.height()/bh)*0.9;
        let ox=rect.center().x-(bmin[0]+bmax[0])*0.5*scale;
        let oy=rect.center().y+(bmin[1]+bmax[1])*0.5*scale;
        let to_screen=|p:[f32;2]|Pos2::new(ox+p[0]*scale,oy-p[1]*scale);
        let bl=to_screen(bmin); let tr=to_screen(bmax);
        painter.rect_stroke(Rect::from_min_max(Pos2::new(bl.x,tr.y),Pos2::new(tr.x,bl.y)),0.0,Stroke::new(1.5,Color32::from_rgb(60,80,120)));
        let max_d=sim.particles.iter().map(|p|p.density).fold(0.0f32,f32::max).max(1.0);
        let min_d=sim.particles.iter().map(|p|p.density).fold(f32::MAX,f32::min);
        for particle in &sim.particles {
            let p=to_screen(particle.pos);
            let col=if state.show_density {
                let t=((particle.density-min_d)/(max_d-min_d+1e-6)).clamp(0.0,1.0);
                Color32::from_rgb((40.0+t*215.0) as u8,(80.0+t*120.0) as u8,(200.0+t*55.0) as u8)
            } else { Color32::from_rgb(60,140,220) };
            painter.circle_filled(p, state.particle_size, col);
            if state.show_velocity {
                let spd=(particle.vel[0]*particle.vel[0]+particle.vel[1]*particle.vel[1]).sqrt().min(5.0);
                if spd>0.1 {
                    let ve=to_screen([particle.pos[0]+particle.vel[0]*0.05, particle.pos[1]+particle.vel[1]*0.05]);
                    painter.line_segment([p,ve],Stroke::new(1.0,Color32::from_rgba_premultiplied(255,255,100,180)));
                }
            }
        }
        for em in &sim.emitters {
            let ep=to_screen(em.position);
            painter.circle_stroke(ep,6.0,Stroke::new(2.0,Color32::from_rgb(255,150,50)));
        }
        painter.text(Pos2::new(rect.left()+4.0,rect.top()+4.0),egui::Align2::LEFT_TOP,
            format!("SPH n={} t={:.2}s",sim.particles.len(),sim.sim_time),FontId::monospace(9.0),Color32::from_rgb(140,180,220));
    } else {
        painter.text(rect.center(),egui::Align2::CENTER_CENTER,"Click 'Initialize Fluid' to begin",FontId::monospace(11.0),Color32::GRAY);
    }
    if resp.clicked() {
        if let Some(pos)=resp.interact_pointer_pos() {
            if let Some(sim)=&mut state.sim {
                let bmin=sim.params.boundary_min; let bmax=sim.params.boundary_max;
                let bw=bmax[0]-bmin[0]; let bh=bmax[1]-bmin[1];
                let scale=(rect.width()/bw).min(rect.height()/bh)*0.9;
                let ox=rect.center().x-(bmin[0]+bmax[0])*0.5*scale;
                let oy=rect.center().y+(bmin[1]+bmax[1])*0.5*scale;
                let wx=(pos.x-ox)/scale; let wy=-(pos.y-oy)/scale;
                let wx=wx.clamp(bmin[0]+0.05, bmax[0]-0.05);
                let wy=wy.clamp(bmin[1]+0.05, bmax[1]-0.05);
                sim.emitters.push(FluidEmitter::new([wx,wy]));
            }
        }
    }
}

// ============================================================
// RAGDOLL SYSTEM
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum BoneRole { Head, Torso, UpperArm, LowerArm, Hand, Thigh, Shin, Foot, Spine, Pelvis, Neck, Clavicle, Custom(String) }
impl BoneRole {
    pub fn label(&self) -> String {
        match self { BoneRole::Head=>"Head".into(), BoneRole::Torso=>"Torso".into(), BoneRole::UpperArm=>"Upper Arm".into(), BoneRole::LowerArm=>"Lower Arm".into(), BoneRole::Hand=>"Hand".into(), BoneRole::Thigh=>"Thigh".into(), BoneRole::Shin=>"Shin".into(), BoneRole::Foot=>"Foot".into(), BoneRole::Spine=>"Spine".into(), BoneRole::Pelvis=>"Pelvis".into(), BoneRole::Neck=>"Neck".into(), BoneRole::Clavicle=>"Clavicle".into(), BoneRole::Custom(s)=>s.clone() }
    }
    pub fn color(&self) -> Color32 {
        match self { BoneRole::Head=>Color32::from_rgb(255,220,180), BoneRole::Torso|BoneRole::Spine=>Color32::from_rgb(100,160,255), BoneRole::UpperArm|BoneRole::LowerArm=>Color32::from_rgb(180,255,180), BoneRole::Hand=>Color32::from_rgb(255,240,180), BoneRole::Thigh|BoneRole::Shin=>Color32::from_rgb(255,160,100), BoneRole::Foot=>Color32::from_rgb(200,200,200), BoneRole::Pelvis=>Color32::from_rgb(180,120,255), BoneRole::Neck=>Color32::from_rgb(255,200,160), BoneRole::Clavicle=>Color32::from_rgb(160,200,255), BoneRole::Custom(_)=>Color32::from_rgb(180,180,180) }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RagdollBone { pub name: String, pub body_idx: usize, pub length: f32, pub width: f32, pub mass_factor: f32, pub role: BoneRole, pub local_pivot: [f32;2] }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JointLimits { pub swing1: f32, pub swing2: f32, pub twist: f32 }
impl Default for JointLimits { fn default() -> Self { JointLimits { swing1:0.5, swing2:0.5, twist:0.3 } } }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum RagdollJointType { Ball, Hinge, Fixed, Cone, Planar }
impl RagdollJointType {
    pub fn label(&self) -> &'static str { match self { RagdollJointType::Ball=>"Ball", RagdollJointType::Hinge=>"Hinge", RagdollJointType::Fixed=>"Fixed", RagdollJointType::Cone=>"Cone", RagdollJointType::Planar=>"Planar" } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RagdollJoint { pub bone_a: usize, pub bone_b: usize, pub joint_type: RagdollJointType, pub limits: JointLimits, pub anchor_a: [f32;2], pub anchor_b: [f32;2], pub name: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RagdollTemplate { pub name: String, pub bones: Vec<RagdollBone>, pub joints: Vec<RagdollJoint>, pub total_mass: f32, pub height: f32 }

impl RagdollTemplate {
    pub fn humanoid() -> Self {
        let bones=vec![
            RagdollBone{name:"Head".into(),body_idx:0,length:0.22,width:0.18,mass_factor:0.08,role:BoneRole::Head,local_pivot:[0.0,-0.11]},
            RagdollBone{name:"Neck".into(),body_idx:1,length:0.10,width:0.07,mass_factor:0.02,role:BoneRole::Neck,local_pivot:[0.0,0.05]},
            RagdollBone{name:"Torso".into(),body_idx:2,length:0.45,width:0.30,mass_factor:0.25,role:BoneRole::Torso,local_pivot:[0.0,0.225]},
            RagdollBone{name:"Pelvis".into(),body_idx:3,length:0.20,width:0.26,mass_factor:0.12,role:BoneRole::Pelvis,local_pivot:[0.0,0.10]},
            RagdollBone{name:"L UpperArm".into(),body_idx:4,length:0.28,width:0.08,mass_factor:0.06,role:BoneRole::UpperArm,local_pivot:[0.14,0.0]},
            RagdollBone{name:"L LowerArm".into(),body_idx:5,length:0.25,width:0.07,mass_factor:0.04,role:BoneRole::LowerArm,local_pivot:[0.125,0.0]},
            RagdollBone{name:"L Hand".into(),body_idx:6,length:0.09,width:0.07,mass_factor:0.015,role:BoneRole::Hand,local_pivot:[0.045,0.0]},
            RagdollBone{name:"R UpperArm".into(),body_idx:7,length:0.28,width:0.08,mass_factor:0.06,role:BoneRole::UpperArm,local_pivot:[-0.14,0.0]},
            RagdollBone{name:"R LowerArm".into(),body_idx:8,length:0.25,width:0.07,mass_factor:0.04,role:BoneRole::LowerArm,local_pivot:[-0.125,0.0]},
            RagdollBone{name:"R Hand".into(),body_idx:9,length:0.09,width:0.07,mass_factor:0.015,role:BoneRole::Hand,local_pivot:[-0.045,0.0]},
            RagdollBone{name:"L Thigh".into(),body_idx:10,length:0.40,width:0.11,mass_factor:0.12,role:BoneRole::Thigh,local_pivot:[0.09,0.20]},
            RagdollBone{name:"L Shin".into(),body_idx:11,length:0.38,width:0.09,mass_factor:0.08,role:BoneRole::Shin,local_pivot:[0.09,0.19]},
            RagdollBone{name:"L Foot".into(),body_idx:12,length:0.12,width:0.08,mass_factor:0.015,role:BoneRole::Foot,local_pivot:[0.0,0.06]},
            RagdollBone{name:"R Thigh".into(),body_idx:13,length:0.40,width:0.11,mass_factor:0.12,role:BoneRole::Thigh,local_pivot:[-0.09,0.20]},
            RagdollBone{name:"R Shin".into(),body_idx:14,length:0.38,width:0.09,mass_factor:0.08,role:BoneRole::Shin,local_pivot:[-0.09,0.19]},
            RagdollBone{name:"R Foot".into(),body_idx:15,length:0.12,width:0.08,mass_factor:0.015,role:BoneRole::Foot,local_pivot:[0.0,0.06]},
            RagdollBone{name:"Spine".into(),body_idx:16,length:0.20,width:0.10,mass_factor:0.06,role:BoneRole::Spine,local_pivot:[0.0,0.10]},
        ];
        let joints=vec![
            RagdollJoint{bone_a:1,bone_b:0,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:0.4,swing2:0.3,twist:0.2},anchor_a:[0.0,-0.05],anchor_b:[0.0,0.11],name:"Neck-Head".into()},
            RagdollJoint{bone_a:2,bone_b:1,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:0.3,swing2:0.2,twist:0.1},anchor_a:[0.0,-0.225],anchor_b:[0.0,0.05],name:"Torso-Neck".into()},
            RagdollJoint{bone_a:2,bone_b:16,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:0.3,swing2:0.1,twist:0.1},anchor_a:[0.0,0.10],anchor_b:[0.0,-0.10],name:"Torso-Spine".into()},
            RagdollJoint{bone_a:16,bone_b:3,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:0.4,swing2:0.1,twist:0.2},anchor_a:[0.0,0.10],anchor_b:[0.0,-0.10],name:"Spine-Pelvis".into()},
            RagdollJoint{bone_a:2,bone_b:4,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:1.5,swing2:0.8,twist:0.5},anchor_a:[0.15,-0.20],anchor_b:[0.0,-0.14],name:"Torso-LUpperArm".into()},
            RagdollJoint{bone_a:4,bone_b:5,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:2.2,swing2:0.0,twist:0.0},anchor_a:[0.0,0.14],anchor_b:[0.0,-0.125],name:"LUpperArm-LLowerArm".into()},
            RagdollJoint{bone_a:5,bone_b:6,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:0.8,swing2:0.5,twist:0.3},anchor_a:[0.0,0.125],anchor_b:[0.0,-0.045],name:"LLowerArm-LHand".into()},
            RagdollJoint{bone_a:2,bone_b:7,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:1.5,swing2:0.8,twist:0.5},anchor_a:[-0.15,-0.20],anchor_b:[0.0,-0.14],name:"Torso-RUpperArm".into()},
            RagdollJoint{bone_a:7,bone_b:8,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:2.2,swing2:0.0,twist:0.0},anchor_a:[0.0,0.14],anchor_b:[0.0,-0.125],name:"RUpperArm-RLowerArm".into()},
            RagdollJoint{bone_a:8,bone_b:9,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:0.8,swing2:0.5,twist:0.3},anchor_a:[0.0,0.125],anchor_b:[0.0,-0.045],name:"RLowerArm-RHand".into()},
            RagdollJoint{bone_a:3,bone_b:10,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:2.0,swing2:0.5,twist:0.4},anchor_a:[0.09,0.10],anchor_b:[0.0,-0.20],name:"Pelvis-LThigh".into()},
            RagdollJoint{bone_a:10,bone_b:11,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:2.5,swing2:0.0,twist:0.0},anchor_a:[0.0,0.20],anchor_b:[0.0,-0.19],name:"LThigh-LShin".into()},
            RagdollJoint{bone_a:11,bone_b:12,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:0.6,swing2:0.0,twist:0.0},anchor_a:[0.0,0.19],anchor_b:[0.0,-0.06],name:"LShin-LFoot".into()},
            RagdollJoint{bone_a:3,bone_b:13,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:2.0,swing2:0.5,twist:0.4},anchor_a:[-0.09,0.10],anchor_b:[0.0,-0.20],name:"Pelvis-RThigh".into()},
            RagdollJoint{bone_a:13,bone_b:14,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:2.5,swing2:0.0,twist:0.0},anchor_a:[0.0,0.20],anchor_b:[0.0,-0.19],name:"RThigh-RShin".into()},
            RagdollJoint{bone_a:14,bone_b:15,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:0.6,swing2:0.0,twist:0.0},anchor_a:[0.0,0.19],anchor_b:[0.0,-0.06],name:"RShin-RFoot".into()},
        ];
        RagdollTemplate{name:"Humanoid".into(),bones,joints,total_mass:70.0,height:1.75}
    }

    pub fn quadruped() -> Self {
        let bones=vec![
            RagdollBone{name:"Body".into(),body_idx:0,length:0.6,width:0.25,mass_factor:0.35,role:BoneRole::Torso,local_pivot:[0.0,0.0]},
            RagdollBone{name:"Head".into(),body_idx:1,length:0.25,width:0.18,mass_factor:0.10,role:BoneRole::Head,local_pivot:[0.0,0.0]},
            RagdollBone{name:"Neck".into(),body_idx:2,length:0.20,width:0.10,mass_factor:0.05,role:BoneRole::Neck,local_pivot:[0.0,0.0]},
            RagdollBone{name:"FL Upper".into(),body_idx:3,length:0.22,width:0.07,mass_factor:0.06,role:BoneRole::UpperArm,local_pivot:[0.0,0.0]},
            RagdollBone{name:"FL Lower".into(),body_idx:4,length:0.20,width:0.06,mass_factor:0.04,role:BoneRole::LowerArm,local_pivot:[0.0,0.0]},
            RagdollBone{name:"FR Upper".into(),body_idx:5,length:0.22,width:0.07,mass_factor:0.06,role:BoneRole::UpperArm,local_pivot:[0.0,0.0]},
            RagdollBone{name:"FR Lower".into(),body_idx:6,length:0.20,width:0.06,mass_factor:0.04,role:BoneRole::LowerArm,local_pivot:[0.0,0.0]},
            RagdollBone{name:"BL Upper".into(),body_idx:7,length:0.25,width:0.08,mass_factor:0.07,role:BoneRole::Thigh,local_pivot:[0.0,0.0]},
            RagdollBone{name:"BL Lower".into(),body_idx:8,length:0.22,width:0.07,mass_factor:0.05,role:BoneRole::Shin,local_pivot:[0.0,0.0]},
            RagdollBone{name:"BR Upper".into(),body_idx:9,length:0.25,width:0.08,mass_factor:0.07,role:BoneRole::Thigh,local_pivot:[0.0,0.0]},
            RagdollBone{name:"BR Lower".into(),body_idx:10,length:0.22,width:0.07,mass_factor:0.05,role:BoneRole::Shin,local_pivot:[0.0,0.0]},
        ];
        let joints=vec![
            RagdollJoint{bone_a:0,bone_b:2,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:0.6,swing2:0.4,twist:0.2},anchor_a:[0.28,0.08],anchor_b:[0.0,-0.10],name:"Body-Neck".into()},
            RagdollJoint{bone_a:2,bone_b:1,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:0.5,swing2:0.4,twist:0.2},anchor_a:[0.0,0.10],anchor_b:[0.0,-0.125],name:"Neck-Head".into()},
            RagdollJoint{bone_a:0,bone_b:3,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:1.2,swing2:0.5,twist:0.3},anchor_a:[0.22,-0.08],anchor_b:[0.0,-0.11],name:"Body-FLUpper".into()},
            RagdollJoint{bone_a:3,bone_b:4,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:2.0,swing2:0.0,twist:0.0},anchor_a:[0.0,0.11],anchor_b:[0.0,-0.10],name:"FLUpper-FLLower".into()},
            RagdollJoint{bone_a:0,bone_b:5,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:1.2,swing2:0.5,twist:0.3},anchor_a:[0.22,0.08],anchor_b:[0.0,-0.11],name:"Body-FRUpper".into()},
            RagdollJoint{bone_a:5,bone_b:6,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:2.0,swing2:0.0,twist:0.0},anchor_a:[0.0,0.11],anchor_b:[0.0,-0.10],name:"FRUpper-FRLower".into()},
            RagdollJoint{bone_a:0,bone_b:7,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:1.5,swing2:0.5,twist:0.4},anchor_a:[-0.22,-0.08],anchor_b:[0.0,-0.125],name:"Body-BLUpper".into()},
            RagdollJoint{bone_a:7,bone_b:8,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:2.2,swing2:0.0,twist:0.0},anchor_a:[0.0,0.125],anchor_b:[0.0,-0.11],name:"BLUpper-BLLower".into()},
            RagdollJoint{bone_a:0,bone_b:9,joint_type:RagdollJointType::Ball,limits:JointLimits{swing1:1.5,swing2:0.5,twist:0.4},anchor_a:[-0.22,0.08],anchor_b:[0.0,-0.125],name:"Body-BRUpper".into()},
            RagdollJoint{bone_a:9,bone_b:10,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:2.2,swing2:0.0,twist:0.0},anchor_a:[0.0,0.125],anchor_b:[0.0,-0.11],name:"BRUpper-BRLower".into()},
        ];
        RagdollTemplate{name:"Quadruped".into(),bones,joints,total_mass:30.0,height:0.9}
    }

    pub fn snake_template(segments: usize) -> Self {
        let mut bones=Vec::new();
        let mut joints=Vec::new();
        let seg_len=0.18f32; let seg_w=0.06f32;
        bones.push(RagdollBone{name:"Head".into(),body_idx:0,length:0.15,width:0.08,mass_factor:0.05,role:BoneRole::Head,local_pivot:[0.0,0.0]});
        for i in 0..segments {
            let bi=bones.len();
            bones.push(RagdollBone{name:format!("Seg{}",i+1),body_idx:bi,length:seg_len,width:seg_w,mass_factor:0.95/segments as f32,role:BoneRole::Spine,local_pivot:[0.0,0.0]});
            if i==0 { joints.push(RagdollJoint{bone_a:0,bone_b:bi,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:0.6,swing2:0.0,twist:0.0},anchor_a:[0.0,-0.075],anchor_b:[0.0,seg_len*0.5],name:"Head-Seg1".into()}); }
            else { joints.push(RagdollJoint{bone_a:bi-1,bone_b:bi,joint_type:RagdollJointType::Hinge,limits:JointLimits{swing1:0.5,swing2:0.0,twist:0.0},anchor_a:[0.0,-seg_len*0.5],anchor_b:[0.0,seg_len*0.5],name:format!("Seg{}-Seg{}",i,i+1)}); }
        }
        RagdollTemplate{name:"Snake".into(),bones,joints,total_mass:2.5,height:seg_len*(segments+1) as f32}
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RagdollEditorState {
    pub templates: Vec<RagdollTemplate>,
    pub selected_template: Option<usize>,
    pub selected_bone: Option<usize>,
    pub selected_joint: Option<usize>,
    pub preview_oscillate: f32,
    pub show_limits: bool,
    pub show_bone_names: bool,
    pub show_anchors: bool,
}
impl RagdollEditorState {
    pub fn new() -> Self {
        let mut s=RagdollEditorState::default();
        s.show_bone_names=true; s.show_limits=true;
        s.templates.push(RagdollTemplate::humanoid());
        s.templates.push(RagdollTemplate::quadruped());
        s.templates.push(RagdollTemplate::snake_template(20));
        s
    }
}

pub fn show_ragdoll_editor(ui: &mut egui::Ui, state: &mut RagdollEditorState) {
    state.preview_oscillate += ui.input(|i| i.unstable_dt).min(0.05);
    ui.heading(RichText::new("Ragdoll System").color(Color32::from_rgb(255,200,140)));
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label("Template:");
        for (i,t) in state.templates.iter().enumerate() {
            if ui.selectable_label(state.selected_template==Some(i),&t.name).clicked() {
                state.selected_template=Some(i); state.selected_bone=None; state.selected_joint=None;
            }
        }
    });
    ui.separator();
    if let Some(ti) = state.selected_template {
        if let Some(template) = state.templates.get_mut(ti) {
            ui.label(RichText::new(format!("{}: {} bones, {} joints, {:.1}kg, {:.2}m",template.name,template.bones.len(),template.joints.len(),template.total_mass,template.height)).small().color(Color32::from_rgb(200,220,255)));
            ui.separator();
            egui::CollapsingHeader::new(RichText::new("Bones").color(Color32::from_rgb(255,200,100))).default_open(true).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("rdoll_bones").max_height(160.0).show(ui, |ui| {
                    for (bi, bone) in template.bones.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let sel=state.selected_bone==Some(bi);
                            if ui.selectable_label(sel,RichText::new(&bone.name).color(bone.role.color())).clicked(){state.selected_bone=Some(bi);}
                            ui.label(RichText::new(format!("L:{:.3} W:{:.3}",bone.length,bone.width)).small().color(Color32::GRAY));
                        });
                    }
                });
            });
            if let Some(bi)=state.selected_bone {
                if let Some(bone)=template.bones.get_mut(bi) {
                    egui::CollapsingHeader::new(RichText::new("Edit Bone").color(Color32::from_rgb(220,200,255))).default_open(true).show(ui, |ui| {
                        egui::Grid::new("bone_ed").num_columns(2).spacing(Vec2::new(8.0,4.0)).show(ui, |ui| {
                            ui.label("Name:"); ui.text_edit_singleline(&mut bone.name); ui.end_row();
                            ui.label("Length:"); ui.add(egui::DragValue::new(&mut bone.length).speed(0.005).clamp_range(0.01f32..=2.0).suffix("m")); ui.end_row();
                            ui.label("Width:"); ui.add(egui::DragValue::new(&mut bone.width).speed(0.005).clamp_range(0.01f32..=1.0).suffix("m")); ui.end_row();
                            ui.label("Mass factor:"); ui.add(egui::DragValue::new(&mut bone.mass_factor).speed(0.001).clamp_range(0.001f32..=1.0)); ui.end_row();
                        });
                    });
                }
            }
            egui::CollapsingHeader::new(RichText::new("Joints").color(Color32::from_rgb(100,220,255))).default_open(false).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("rdoll_jts").max_height(140.0).show(ui, |ui| {
                    for (ji, joint) in template.joints.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let sel=state.selected_joint==Some(ji);
                            if ui.selectable_label(sel,RichText::new(&joint.name).small()).clicked(){state.selected_joint=Some(ji);}
                            let jcol=match joint.joint_type{RagdollJointType::Ball=>Color32::from_rgb(100,200,255),RagdollJointType::Hinge=>Color32::from_rgb(255,180,80),_=>Color32::GRAY};
                            ui.label(RichText::new(joint.joint_type.label()).color(jcol).small());
                        });
                    }
                });
            });
            if let Some(ji)=state.selected_joint {
                if let Some(joint)=template.joints.get_mut(ji) {
                    egui::CollapsingHeader::new(RichText::new("Edit Joint").color(Color32::from_rgb(180,255,200))).default_open(true).show(ui, |ui| {
                        egui::Grid::new("jlim_ed").num_columns(2).spacing(Vec2::new(8.0,4.0)).show(ui, |ui| {
                            ui.label("Swing 1:"); ui.add(egui::Slider::new(&mut joint.limits.swing1,0.0f32..=std::f32::consts::PI)); ui.end_row();
                            ui.label("Swing 2:"); ui.add(egui::Slider::new(&mut joint.limits.swing2,0.0f32..=std::f32::consts::PI)); ui.end_row();
                            ui.label("Twist:"); ui.add(egui::Slider::new(&mut joint.limits.twist,0.0f32..=std::f32::consts::PI)); ui.end_row();
                        });
                    });
                }
            }
        }
    }
    ui.separator();
    ui.horizontal(|ui|{ui.checkbox(&mut state.show_bone_names,"Names"); ui.checkbox(&mut state.show_limits,"Limits"); ui.checkbox(&mut state.show_anchors,"Anchors");});
    let desired=Vec2::new(ui.available_width(),320.0);
    let (rect, _)=ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter=ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(12,14,20));
    if let Some(ti)=state.selected_template {
        if let Some(template)=state.templates.get(ti) {
            let t=state.preview_oscillate;
            let scale=(rect.height()/(template.height*1.3)).min(rect.width()/(template.height*0.8));
            let cx=rect.center().x;
            let cy=rect.center().y+template.height*scale*0.4;
            let bpos:Vec<Pos2>=template.bones.iter().enumerate().map(|(bi,bone)|{
                let osc=(t*1.2+bi as f32*0.3).sin()*0.02*scale;
                Pos2::new(cx+bone.local_pivot[0]*scale+osc, cy-bone.local_pivot[1]*scale)
            }).collect();
            for joint in &template.joints {
                let a=joint.bone_a.min(bpos.len().saturating_sub(1));
                let b=joint.bone_b.min(bpos.len().saturating_sub(1));
                if a>=bpos.len()||b>=bpos.len(){continue;}
                let jcol=match joint.joint_type{RagdollJointType::Ball=>Color32::from_rgb(100,200,255),RagdollJointType::Hinge=>Color32::from_rgb(255,180,80),_=>Color32::GRAY};
                painter.line_segment([bpos[a],bpos[b]],Stroke::new(1.0,jcol));
            }
            for (bi, bone) in template.bones.iter().enumerate() {
                if bi>=bpos.len(){continue;}
                let p=bpos[bi];
                let col=bone.role.color();
                let hw=bone.width*scale*0.5; let hh=bone.length*scale*0.5;
                let br=Rect::from_center_size(p,Vec2::new(hw*2.0,hh*2.0));
                painter.rect_filled(br,hw*0.4,Color32::from_rgba_premultiplied(col.r(),col.g(),col.b(),160));
                painter.rect_stroke(br,hw*0.4,Stroke::new(1.0,col));
                if state.show_bone_names { painter.text(p,egui::Align2::CENTER_CENTER,&bone.name,FontId::monospace(6.5),Color32::from_rgba_premultiplied(255,255,255,180)); }
            }
        }
    }
}

// ============================================================
// CONSTRAINT GRAPH VISUALIZATION
// ============================================================

#[derive(Clone, Debug, Default)]
pub struct ConstraintGraphState {
    pub selected_body: Option<usize>,
    pub selected_joint: Option<usize>,
    pub oscillate_time: f32,
    pub node_positions: Vec<[f32;2]>,
    pub layout_dirty: bool,
}

pub fn show_constraint_graph(ui: &mut egui::Ui, editor: &PhysicsEditor, cgraph: &mut ConstraintGraphState) {
    cgraph.oscillate_time += ui.input(|i| i.unstable_dt).min(0.05);
    ui.heading(RichText::new("Constraint Graph").color(Color32::from_rgb(200,200,255)));
    ui.separator();
    if cgraph.layout_dirty || cgraph.node_positions.len() != editor.bodies.len() {
        layout_constraint_graph(editor, cgraph);
        cgraph.layout_dirty = false;
    }
    ui.horizontal(|ui| {
        if ui.button("Re-layout").clicked() { layout_constraint_graph(editor, cgraph); }
        ui.label(RichText::new(format!("{} bodies, {} constraints",editor.bodies.len(),editor.joints.len())).small().color(Color32::GRAY));
    });
    let desired=Vec2::new(ui.available_width(),380.0);
    let (rect, resp)=ui.allocate_exact_size(desired, egui::Sense::click());
    let painter=ui.painter_at(rect);
    painter.rect_filled(rect,4.0,Color32::from_rgb(10,12,18));
    let t=cgraph.oscillate_time;
    for (ji, joint) in editor.joints.iter().enumerate() {
        let (ba,bb)=joint_bodies(joint);
        if ba>=cgraph.node_positions.len()||bb>=cgraph.node_positions.len(){continue;}
        let pa=graph_node_screen(cgraph.node_positions[ba],rect,t,ba);
        let pb=graph_node_screen(cgraph.node_positions[bb],rect,t,bb);
        let jcol=joint_col(joint);
        let sel=cgraph.selected_joint==Some(ji);
        painter.line_segment([pa,pb],Stroke::new(if sel{3.0}else{1.5},jcol));
        let mid=Pos2::new((pa.x+pb.x)*0.5,(pa.y+pb.y)*0.5);
        painter.circle_filled(mid,3.5,jcol);
        if sel { painter.text(mid+Vec2::new(5.0,-8.0),egui::Align2::LEFT_BOTTOM,joint_name(joint),FontId::monospace(8.0),jcol); }
    }
    for (bi, body) in editor.bodies.iter().enumerate() {
        if bi>=cgraph.node_positions.len(){continue;}
        let p=graph_node_screen(cgraph.node_positions[bi],rect,t,bi);
        let r=12.0+(body.mass*0.5).min(8.0);
        let sel=cgraph.selected_body==Some(bi);
        let bc=body.body_type.color();
        let fill=Color32::from_rgba_premultiplied(bc.r(),bc.g(),bc.b(),if sel{230}else{120});
        painter.circle_filled(p,r,fill);
        painter.circle_stroke(p,r,Stroke::new(if sel{2.5}else{1.0},bc));
        painter.text(p,egui::Align2::CENTER_CENTER,&body.name[..body.name.len().min(6)],FontId::monospace(7.0),Color32::WHITE);
        if sel {
            let mut iy=p.y-r-2.0;
            for joint in &editor.joints {
                let (a,b)=joint_bodies(joint);
                if a==bi||b==bi {
                    painter.text(Pos2::new(p.x+r+4.0,iy),egui::Align2::LEFT_BOTTOM,
                        format!("{} ({}<->{})",joint_name(joint),a,b),FontId::monospace(7.0),joint_col(joint));
                    iy-=9.0;
                }
            }
        }
    }
    if resp.clicked() {
        if let Some(pos)=resp.interact_pointer_pos() {
            let mut found=false;
            for (bi,_) in editor.bodies.iter().enumerate() {
                if bi>=cgraph.node_positions.len(){continue;}
                let p=graph_node_screen(cgraph.node_positions[bi],rect,t,bi);
                if (pos-p).length()<16.0{cgraph.selected_body=Some(bi);found=true;break;}
            }
            if !found{cgraph.selected_body=None;}
        }
    }
    if let Some(bi)=cgraph.selected_body {
        if let Some(body)=editor.bodies.get(bi) {
            let cc=editor.joints.iter().filter(|j|{let(a,b)=joint_bodies(j);a==bi||b==bi}).count();
            ui.separator();
            ui.label(RichText::new(format!("Body #{}: {} | {} | mass:{:.2} | {} connections",bi,body.name,body.body_type.label(),body.mass,cc)).small().color(Color32::from_rgb(200,220,255)));
        }
    }
}

fn layout_constraint_graph(editor: &PhysicsEditor, cgraph: &mut ConstraintGraphState) {
    let n=editor.bodies.len(); if n==0{cgraph.node_positions.clear();return;}
    cgraph.node_positions=(0..n).map(|i|{let a=(i as f32/n as f32)*std::f32::consts::TAU;let r=0.35+(i%3) as f32*0.08;[0.5+r*a.cos(),0.5+r*a.sin()]}).collect();
    for _ in 0..50 {
        let mut forces=vec![[0.0f32;2];n];
        for i in 0..n { for j in 0..n { if i==j{continue;}
            let dx=cgraph.node_positions[i][0]-cgraph.node_positions[j][0];
            let dy=cgraph.node_positions[i][1]-cgraph.node_positions[j][1];
            let d=(dx*dx+dy*dy).sqrt().max(0.01);
            let rep=0.008/(d*d);
            forces[i][0]+=dx/d*rep; forces[i][1]+=dy/d*rep;
        }}
        for joint in &editor.joints {
            let (a,b)=joint_bodies(joint);
            if a>=n||b>=n{continue;}
            let dx=cgraph.node_positions[b][0]-cgraph.node_positions[a][0];
            let dy=cgraph.node_positions[b][1]-cgraph.node_positions[a][1];
            let d=(dx*dx+dy*dy).sqrt().max(0.01);
            let att=(d-0.15)*0.02;
            forces[a][0]+=dx/d*att; forces[a][1]+=dy/d*att;
            forces[b][0]-=dx/d*att; forces[b][1]-=dy/d*att;
        }
        for i in 0..n {
            cgraph.node_positions[i][0]=(cgraph.node_positions[i][0]+forces[i][0]).clamp(0.05,0.95);
            cgraph.node_positions[i][1]=(cgraph.node_positions[i][1]+forces[i][1]).clamp(0.05,0.95);
        }
    }
}

fn graph_node_screen(pos:[f32;2],rect:Rect,t:f32,idx:usize)->Pos2 {
    Pos2::new(rect.left()+pos[0]*rect.width()+(t*1.1+idx as f32*0.7).sin()*2.0, rect.top()+pos[1]*rect.height()+(t*0.9+idx as f32*0.5).cos()*2.0)
}
fn joint_bodies(joint:&Joint)->(usize,usize){match joint{Joint::Fixed{body_a,body_b,..}=>(*body_a,*body_b),Joint::Hinge{body_a,body_b,..}=>(*body_a,*body_b),Joint::Slider{body_a,body_b,..}=>(*body_a,*body_b),Joint::Spring{body_a,body_b,..}=>(*body_a,*body_b),Joint::Distance{body_a,body_b,..}=>(*body_a,*body_b),Joint::Pulley{body_a,body_b,..}=>(*body_a,*body_b)}}
fn joint_col(joint:&Joint)->Color32{match joint{Joint::Fixed{..}=>Color32::from_rgb(150,150,150),Joint::Hinge{..}=>Color32::from_rgb(255,180,80),Joint::Slider{..}=>Color32::from_rgb(100,200,255),Joint::Spring{..}=>Color32::from_rgb(100,255,150),Joint::Distance{..}=>Color32::from_rgb(255,100,200),Joint::Pulley{..}=>Color32::from_rgb(200,100,255)}}
fn joint_name(joint:&Joint)->&'static str{match joint{Joint::Fixed{..}=>"Fixed",Joint::Hinge{..}=>"Hinge",Joint::Slider{..}=>"Slider",Joint::Spring{..}=>"Spring",Joint::Distance{..}=>"Distance",Joint::Pulley{..}=>"Pulley"}}

// ============================================================
// COLLISION RESPONSE EDITOR
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionEvent { pub body_a: usize, pub body_b: usize, pub point: [f32;2], pub normal: [f32;2], pub impulse: f32, pub relative_velocity: f32 }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CollisionAction {
    PlaySound { sound_name: String, volume: f32 },
    SpawnParticles { count: u32, color: [u8;3], spread: f32 },
    ApplyDamage { amount: f32 },
    TriggerCallback { callback_name: String },
    None,
}
impl CollisionAction {
    pub fn label(&self) -> &'static str { match self { CollisionAction::PlaySound{..}=>"Play Sound", CollisionAction::SpawnParticles{..}=>"Spawn Particles", CollisionAction::ApplyDamage{..}=>"Apply Damage", CollisionAction::TriggerCallback{..}=>"Trigger Callback", CollisionAction::None=>"None" } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionEventFilter { pub name: String, pub min_impulse: f32, pub max_impulse: f32, pub layer_mask_a: u16, pub layer_mask_b: u16, pub action: CollisionAction, pub enabled: bool }
impl CollisionEventFilter {
    pub fn new(name: &str) -> Self { CollisionEventFilter { name:name.into(), min_impulse:0.0, max_impulse:1000.0, layer_mask_a:0xFFFF, layer_mask_b:0xFFFF, action:CollisionAction::None, enabled:true } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionMatrix { pub matrix: [[bool;16];16], pub layer_names: [String;16] }
impl Default for CollisionMatrix { fn default() -> Self { CollisionMatrix::new() } }
impl CollisionMatrix {
    pub fn new() -> Self {
        let mut m=CollisionMatrix{matrix:[[true;16];16],layer_names:Default::default()};
        let names=["Default","Player","Enemy","Projectile","Terrain","Trigger","UI","Water","Debris","Vehicle","NPC","Sensor","Overlay","Camera","Custom1","Custom2"];
        for (i,n) in names.iter().enumerate(){m.layer_names[i]=n.to_string();}
        m
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CollisionEditorState { pub filters: Vec<CollisionEventFilter>, pub matrix: CollisionMatrix, pub recent_events: Vec<CollisionEvent>, pub selected_filter: Option<usize>, pub show_matrix: bool, pub show_events: bool }
impl CollisionEditorState {
    pub fn new() -> Self {
        let mut s=CollisionEditorState::default(); s.matrix=CollisionMatrix::new();
        s.filters.push(CollisionEventFilter::new("High Impact"));
        s.filters[0].min_impulse=10.0;
        s.filters[0].action=CollisionAction::SpawnParticles{count:8,color:[255,200,80],spread:0.5};
        s
    }
}

pub fn show_collision_editor(ui: &mut egui::Ui, state: &mut CollisionEditorState) {
    ui.heading(RichText::new("Collision Response Editor").color(Color32::from_rgb(255,180,100)));
    ui.separator();
    egui::CollapsingHeader::new(RichText::new("Event Filters").color(Color32::from_rgb(255,200,140))).default_open(true).show(ui, |ui| {
        if ui.button("Add Filter").clicked() { state.filters.push(CollisionEventFilter::new(&format!("Filter {}",state.filters.len()+1))); state.selected_filter=Some(state.filters.len()-1); }
        let mut to_remove=None;
        egui::ScrollArea::vertical().id_salt("col_filt").max_height(120.0).show(ui, |ui| {
            for (i, filter) in state.filters.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.selectable_label(state.selected_filter==Some(i),&filter.name).clicked(){state.selected_filter=Some(i);}
                    ui.label(RichText::new(format!("[{:.0}-{:.0}N] {}",filter.min_impulse,filter.max_impulse,filter.action.label())).small().color(Color32::GRAY));
                    if ui.small_button("X").clicked(){to_remove=Some(i);}
                });
            }
        });
        if let Some(idx)=to_remove{state.filters.remove(idx);if state.selected_filter==Some(idx){state.selected_filter=None;}}
        if let Some(fi)=state.selected_filter {
            if let Some(filter)=state.filters.get_mut(fi) {
                ui.separator();
                egui::Grid::new("cf_ed").num_columns(2).spacing(Vec2::new(8.0,4.0)).show(ui, |ui| {
                    ui.label("Name:"); ui.text_edit_singleline(&mut filter.name); ui.end_row();
                    ui.label("Min impulse:"); ui.add(egui::DragValue::new(&mut filter.min_impulse).speed(0.5).suffix("N")); ui.end_row();
                    ui.label("Max impulse:"); ui.add(egui::DragValue::new(&mut filter.max_impulse).speed(1.0).suffix("N")); ui.end_row();
                    ui.label("Enabled:"); ui.checkbox(&mut filter.enabled,""); ui.end_row();
                });
                ui.label("Action:");
                ui.horizontal(|ui| {
                    for al in ["None","Play Sound","Spawn Particles","Apply Damage","Trigger Callback"] {
                        if ui.selectable_label(filter.action.label()==al,al).clicked() {
                            filter.action=match al {
                                "Play Sound"=>CollisionAction::PlaySound{sound_name:"impact.wav".into(),volume:1.0},
                                "Spawn Particles"=>CollisionAction::SpawnParticles{count:5,color:[255,180,80],spread:0.3},
                                "Apply Damage"=>CollisionAction::ApplyDamage{amount:10.0},
                                "Trigger Callback"=>CollisionAction::TriggerCallback{callback_name:"on_hit".into()},
                                _=>CollisionAction::None,
                            };
                        }
                    }
                });
                match &mut filter.action {
                    CollisionAction::PlaySound{sound_name,volume}=>{ui.horizontal(|ui|{ui.label("Sound:");ui.text_edit_singleline(sound_name);ui.label("Vol:");ui.add(egui::DragValue::new(volume).speed(0.05).clamp_range(0.0f32..=2.0));});}
                    CollisionAction::SpawnParticles{count,spread,..}=>{ui.horizontal(|ui|{ui.label("Count:");ui.add(egui::DragValue::new(count).speed(1).clamp_range(1u32..=100));ui.label("Spread:");ui.add(egui::DragValue::new(spread).speed(0.05));});}
                    CollisionAction::ApplyDamage{amount}=>{ui.horizontal(|ui|{ui.label("Damage:");ui.add(egui::DragValue::new(amount).speed(0.5));});}
                    CollisionAction::TriggerCallback{callback_name}=>{ui.horizontal(|ui|{ui.label("Callback:");ui.text_edit_singleline(callback_name);});}
                    CollisionAction::None=>{}
                }
            }
        }
    });
    ui.checkbox(&mut state.show_matrix,"Show Layer Matrix");
    if state.show_matrix {
        egui::CollapsingHeader::new(RichText::new("16x16 Layer Matrix").color(Color32::from_rgb(180,220,255))).default_open(true).show(ui, |ui| {
            draw_collision_matrix(ui, &mut state.matrix);
        });
    }
    ui.checkbox(&mut state.show_events,"Show Recent Events");
    if state.show_events {
        egui::CollapsingHeader::new(RichText::new("Recent Events").color(Color32::from_rgb(200,200,200))).default_open(false).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("col_ev").max_height(100.0).show(ui, |ui| {
                for ev in &state.recent_events {
                    ui.label(RichText::new(format!("A:{} B:{} impl:{:.2}N rv:{:.2}m/s",ev.body_a,ev.body_b,ev.impulse,ev.relative_velocity)).small().color(Color32::GRAY));
                }
                if state.recent_events.is_empty() { ui.label(RichText::new("No events").color(Color32::DARK_GRAY).small()); }
            });
        });
    }
}

fn draw_collision_matrix(ui: &mut egui::Ui, matrix: &mut CollisionMatrix) {
    let cell=14.0f32; let lw=60.0f32;
    let total_w=lw+16.0*cell+4.0; let total_h=lw+16.0*cell+4.0;
    let desired=Vec2::new(total_w.min(ui.available_width()), total_h.min(280.0));
    let (rect, resp)=ui.allocate_exact_size(desired, egui::Sense::click());
    let painter=ui.painter_at(rect);
    painter.rect_filled(rect,2.0,Color32::from_rgb(14,14,22));
    let ox=rect.left()+lw; let oy=rect.top()+lw;
    for j in 0..16 { painter.text(Pos2::new(ox+j as f32*cell+cell*0.5,oy-4.0),egui::Align2::CENTER_BOTTOM,format!("{}",j),FontId::monospace(6.0),Color32::GRAY); }
    for i in 0..16 {
        let y=oy+i as f32*cell+cell*0.5;
        let n=&matrix.layer_names[i]; let a=&n[..n.len().min(7)];
        painter.text(Pos2::new(ox-2.0,y),egui::Align2::RIGHT_CENTER,a,FontId::monospace(6.0),Color32::GRAY);
    }
    for i in 0..16 { for j in 0..16 {
        let x=ox+j as f32*cell; let y=oy+i as f32*cell;
        let cr=Rect::from_min_size(Pos2::new(x+1.0,y+1.0),Vec2::splat(cell-2.0));
        painter.rect_filled(cr,1.0,if matrix.matrix[i][j]{Color32::from_rgb(40,160,60)}else{Color32::from_rgb(60,20,20)});
        if i==j{painter.rect_stroke(cr,1.0,Stroke::new(0.5,Color32::from_rgb(200,200,80)));}
    }}
    if resp.clicked() {
        if let Some(pos)=resp.interact_pointer_pos() {
            let ci=((pos.x-ox)/cell).floor() as i32;
            let ri=((pos.y-oy)/cell).floor() as i32;
            if ci>=0&&ci<16&&ri>=0&&ri<16 {
                let c=ci as usize; let r=ri as usize;
                matrix.matrix[r][c]=!matrix.matrix[r][c];
                matrix.matrix[c][r]=matrix.matrix[r][c];
            }
        }
    }
}

// ============================================================
// EXPANDED MATERIAL LIBRARY (20+ presets)
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhysicsMaterialPreset { pub material: PhysicsMaterial, pub density: f32, pub description: String, pub category: MaterialCategory }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MaterialCategory { Metal, Organic, Stone, Synthetic, Liquid, Soft }
impl MaterialCategory {
    pub fn label(&self) -> &'static str { match self { MaterialCategory::Metal=>"Metal", MaterialCategory::Organic=>"Organic", MaterialCategory::Stone=>"Stone", MaterialCategory::Synthetic=>"Synthetic", MaterialCategory::Liquid=>"Liquid", MaterialCategory::Soft=>"Soft" } }
    pub fn color(&self) -> Color32 { match self { MaterialCategory::Metal=>Color32::from_rgb(200,200,220), MaterialCategory::Organic=>Color32::from_rgb(120,200,100), MaterialCategory::Stone=>Color32::from_rgb(160,140,120), MaterialCategory::Synthetic=>Color32::from_rgb(100,180,255), MaterialCategory::Liquid=>Color32::from_rgb(80,160,255), MaterialCategory::Soft=>Color32::from_rgb(255,180,200) } }
}

pub fn all_material_presets() -> Vec<PhysicsMaterialPreset> {
    vec![
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Ice".into(),restitution:0.1,friction:0.01,combine_mode:CombineMode::Multiply,color:[180,220,255]},density:917.0,description:"Frozen water, very slippery".into(),category:MaterialCategory::Liquid},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Rubber".into(),restitution:0.9,friction:0.8,combine_mode:CombineMode::Max,color:[40,40,40]},density:1200.0,description:"Elastic, high grip".into(),category:MaterialCategory::Synthetic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Steel".into(),restitution:0.3,friction:0.4,combine_mode:CombineMode::Average,color:[180,180,200]},density:7800.0,description:"Hard metal".into(),category:MaterialCategory::Metal},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Wood".into(),restitution:0.4,friction:0.6,combine_mode:CombineMode::Average,color:[160,110,60]},density:700.0,description:"Natural timber".into(),category:MaterialCategory::Organic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Concrete".into(),restitution:0.1,friction:0.7,combine_mode:CombineMode::Average,color:[140,140,140]},density:2300.0,description:"Dense construction material".into(),category:MaterialCategory::Stone},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Glass".into(),restitution:0.5,friction:0.1,combine_mode:CombineMode::Min,color:[180,220,240]},density:2500.0,description:"Smooth, moderate elasticity".into(),category:MaterialCategory::Synthetic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Foam".into(),restitution:0.05,friction:0.5,combine_mode:CombineMode::Multiply,color:[240,230,200]},density:50.0,description:"Very soft, near zero bounce".into(),category:MaterialCategory::Soft},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Mud".into(),restitution:0.02,friction:0.9,combine_mode:CombineMode::Max,color:[100,70,40]},density:1800.0,description:"High friction, almost no bounce".into(),category:MaterialCategory::Organic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Sand".into(),restitution:0.05,friction:0.6,combine_mode:CombineMode::Average,color:[220,200,140]},density:1600.0,description:"Granular, low elasticity".into(),category:MaterialCategory::Stone},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Water".into(),restitution:0.0,friction:0.0,combine_mode:CombineMode::Min,color:[60,120,220]},density:1000.0,description:"Liquid, no friction".into(),category:MaterialCategory::Liquid},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Bouncy".into(),restitution:1.0,friction:0.5,combine_mode:CombineMode::Max,color:[255,80,80]},density:800.0,description:"Super elastic".into(),category:MaterialCategory::Synthetic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Sticky".into(),restitution:0.0,friction:1.5,combine_mode:CombineMode::Max,color:[200,255,100]},density:1100.0,description:"Adheres to surfaces".into(),category:MaterialCategory::Synthetic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Bronze".into(),restitution:0.25,friction:0.35,combine_mode:CombineMode::Average,color:[200,140,60]},density:8900.0,description:"Copper alloy".into(),category:MaterialCategory::Metal},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Aluminum".into(),restitution:0.28,friction:0.38,combine_mode:CombineMode::Average,color:[220,220,240]},density:2700.0,description:"Light metal alloy".into(),category:MaterialCategory::Metal},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Granite".into(),restitution:0.15,friction:0.65,combine_mode:CombineMode::Average,color:[120,100,100]},density:2700.0,description:"Hard igneous rock".into(),category:MaterialCategory::Stone},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Cork".into(),restitution:0.6,friction:0.7,combine_mode:CombineMode::Average,color:[200,170,120]},density:120.0,description:"Very light natural material".into(),category:MaterialCategory::Organic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Leather".into(),restitution:0.2,friction:0.5,combine_mode:CombineMode::Average,color:[140,80,40]},density:860.0,description:"Processed animal hide".into(),category:MaterialCategory::Organic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Carbon Fiber".into(),restitution:0.2,friction:0.3,combine_mode:CombineMode::Average,color:[30,30,40]},density:1600.0,description:"High strength composite".into(),category:MaterialCategory::Synthetic},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Lead".into(),restitution:0.02,friction:0.5,combine_mode:CombineMode::Average,color:[80,80,100]},density:11340.0,description:"Very dense soft metal".into(),category:MaterialCategory::Metal},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Titanium".into(),restitution:0.35,friction:0.36,combine_mode:CombineMode::Average,color:[200,200,220]},density:4500.0,description:"Lightweight strong metal".into(),category:MaterialCategory::Metal},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Gel".into(),restitution:0.3,friction:0.6,combine_mode:CombineMode::Multiply,color:[180,255,200]},density:1050.0,description:"Silicone gel, squishy".into(),category:MaterialCategory::Soft},
        PhysicsMaterialPreset{material:PhysicsMaterial{name:"Clay".into(),restitution:0.05,friction:0.8,combine_mode:CombineMode::Max,color:[200,140,100]},density:1800.0,description:"Wet clay, deformable".into(),category:MaterialCategory::Organic},
    ]
}

#[derive(Clone, Debug, Default)]
pub struct MaterialLibraryState { pub presets: Vec<PhysicsMaterialPreset>, pub selected: Option<usize>, pub filter_category: Option<MaterialCategory>, pub show_chart: bool, pub search: String }
impl MaterialLibraryState {
    pub fn new() -> Self { MaterialLibraryState { presets:all_material_presets(), ..Default::default() } }
}

pub fn show_material_library(ui: &mut egui::Ui, state: &mut MaterialLibraryState) {
    ui.heading(RichText::new("Physics Material Library").color(Color32::from_rgb(220,200,255)));
    ui.separator();
    ui.horizontal(|ui|{ui.label("Search:");ui.text_edit_singleline(&mut state.search);if ui.small_button("X").clicked(){state.search.clear();}});
    ui.horizontal_wrapped(|ui| {
        ui.label("Category:");
        if ui.selectable_label(state.filter_category.is_none(),"All").clicked(){state.filter_category=None;}
        for cat in [MaterialCategory::Metal,MaterialCategory::Stone,MaterialCategory::Organic,MaterialCategory::Synthetic,MaterialCategory::Liquid,MaterialCategory::Soft] {
            let sel=state.filter_category.as_ref()==Some(&cat);
            if ui.selectable_label(sel,RichText::new(cat.label()).color(cat.color())).clicked(){state.filter_category=if sel{None}else{Some(cat)};}
        }
    });
    ui.separator();
    let search_lower=state.search.to_lowercase();
    egui::ScrollArea::vertical().id_salt("mat_lib").max_height(200.0).show(ui, |ui| {
        egui::Grid::new("mat_tbl").num_columns(5).spacing(Vec2::new(8.0,3.0)).show(ui, |ui| {
            ui.label(RichText::new("Name").strong().small());
            ui.label(RichText::new("Restitution").strong().small());
            ui.label(RichText::new("Friction").strong().small());
            ui.label(RichText::new("Density").strong().small());
            ui.label(RichText::new("Category").strong().small());
            ui.end_row();
            for (i, preset) in state.presets.iter().enumerate() {
                if let Some(ref cat)=state.filter_category{if &preset.category!=cat{continue;}}
                if !search_lower.is_empty()&&!preset.material.name.to_lowercase().contains(&search_lower){continue;}
                let sel=state.selected==Some(i);
                let c=preset.category.color();
                if ui.selectable_label(sel,RichText::new(&preset.material.name).color(c)).clicked(){state.selected=Some(i);}
                ui.label(RichText::new(format!("{:.2}",preset.material.restitution)).small().color(if preset.material.restitution>0.7{Color32::from_rgb(100,255,100)}else{Color32::GRAY}));
                ui.label(RichText::new(format!("{:.2}",preset.material.friction)).small());
                ui.label(RichText::new(format!("{:.0} kg/m³",preset.density)).small().color(Color32::from_rgb(180,180,220)));
                ui.label(RichText::new(preset.category.label()).small().color(c));
                ui.end_row();
            }
        });
    });
    if let Some(si)=state.selected {
        if let Some(preset)=state.presets.get(si) {
            ui.separator();
            ui.horizontal(|ui|{
                let [r,g,b]=preset.material.color;
                let (cr,_)=ui.allocate_exact_size(Vec2::splat(18.0),egui::Sense::hover());
                ui.painter_at(cr).rect_filled(cr,3.0,Color32::from_rgb(r,g,b));
                ui.label(RichText::new(&preset.material.name).strong().color(preset.category.color()));
                ui.label(RichText::new(&preset.description).small().color(Color32::LIGHT_GRAY));
            });
            egui::Grid::new("mat_det").num_columns(2).spacing(Vec2::new(8.0,3.0)).show(ui, |ui| {
                ui.label("Restitution:"); draw_mat_bar(ui, preset.material.restitution, 0.0, 1.0, Color32::from_rgb(100,200,255)); ui.end_row();
                ui.label("Friction:"); draw_mat_bar(ui, preset.material.friction, 0.0, 2.0, Color32::from_rgb(255,180,80)); ui.end_row();
                ui.label("Density:"); ui.label(RichText::new(format!("{:.0} kg/m³",preset.density)).small()); ui.end_row();
            });
        }
    }
    ui.separator();
    ui.checkbox(&mut state.show_chart,"Show Friction vs Restitution Chart");
    if state.show_chart { draw_material_scatter(ui, &state.presets, state.selected); }
}

fn draw_mat_bar(ui: &mut egui::Ui, value: f32, min: f32, max: f32, color: Color32) {
    let desired=Vec2::new(120.0,12.0);
    let (rect,_)=ui.allocate_exact_size(desired,egui::Sense::hover());
    let painter=ui.painter_at(rect);
    painter.rect_filled(rect,2.0,Color32::from_rgb(30,30,40));
    let t=((value-min)/(max-min)).clamp(0.0,1.0);
    painter.rect_filled(Rect::from_min_size(rect.min,Vec2::new(rect.width()*t,rect.height())),2.0,color);
    painter.text(Pos2::new(rect.right()-2.0,rect.center().y),egui::Align2::RIGHT_CENTER,format!("{:.3}",value),FontId::monospace(7.0),Color32::WHITE);
}

fn draw_material_scatter(ui: &mut egui::Ui, presets: &[PhysicsMaterialPreset], selected: Option<usize>) {
    let desired=Vec2::new(ui.available_width().min(380.0),240.0);
    let (rect,_)=ui.allocate_exact_size(desired,egui::Sense::hover());
    let painter=ui.painter_at(rect);
    painter.rect_filled(rect,4.0,Color32::from_rgb(12,14,22));
    let m=30.0f32;
    let pr=Rect::from_min_max(Pos2::new(rect.left()+m,rect.top()+10.0),Pos2::new(rect.right()-10.0,rect.bottom()-m));
    painter.line_segment([Pos2::new(pr.left(),pr.bottom()),Pos2::new(pr.right(),pr.bottom())],Stroke::new(1.0,Color32::GRAY));
    painter.line_segment([Pos2::new(pr.left(),pr.top()),Pos2::new(pr.left(),pr.bottom())],Stroke::new(1.0,Color32::GRAY));
    painter.text(Pos2::new(pr.center().x,rect.bottom()-2.0),egui::Align2::CENTER_BOTTOM,"Friction ->",FontId::monospace(8.0),Color32::GRAY);
    for i in 0..=4 {
        let t=i as f32/4.0;
        let gx=pr.left()+t*pr.width(); let gy=pr.bottom()-t*pr.height();
        painter.line_segment([Pos2::new(gx,pr.top()),Pos2::new(gx,pr.bottom())],Stroke::new(0.3,Color32::from_rgba_premultiplied(60,60,80,80)));
        painter.line_segment([Pos2::new(pr.left(),gy),Pos2::new(pr.right(),gy)],Stroke::new(0.3,Color32::from_rgba_premultiplied(60,60,80,80)));
        painter.text(Pos2::new(gx,pr.bottom()+3.0),egui::Align2::CENTER_TOP,format!("{:.1}",t*2.0),FontId::monospace(6.0),Color32::GRAY);
        painter.text(Pos2::new(pr.left()-2.0,gy),egui::Align2::RIGHT_CENTER,format!("{:.1}",t),FontId::monospace(6.0),Color32::GRAY);
    }
    for (i,preset) in presets.iter().enumerate() {
        let fx=(preset.material.friction/2.0).clamp(0.0,1.0);
        let fy=preset.material.restitution.clamp(0.0,1.0);
        let px=pr.left()+fx*pr.width(); let py=pr.bottom()-fy*pr.height();
        let p=Pos2::new(px,py);
        let c=preset.category.color();
        let is_sel=selected==Some(i);
        let r=if is_sel{7.0}else{4.5};
        painter.circle_filled(p,r,c);
        if is_sel{
            painter.circle_stroke(p,r+2.0,Stroke::new(1.5,Color32::WHITE));
            painter.text(Pos2::new(p.x+6.0,p.y-6.0),egui::Align2::LEFT_BOTTOM,&preset.material.name,FontId::monospace(8.0),Color32::WHITE);
        }
    }
    let cats=[MaterialCategory::Metal,MaterialCategory::Stone,MaterialCategory::Organic,MaterialCategory::Synthetic,MaterialCategory::Liquid,MaterialCategory::Soft];
    for (i,cat) in cats.iter().enumerate(){
        let lx=pr.right()-60.0; let ly=pr.top()+i as f32*13.0;
        painter.circle_filled(Pos2::new(lx,ly+4.0),4.0,cat.color());
        painter.text(Pos2::new(lx+7.0,ly+4.0),egui::Align2::LEFT_CENTER,cat.label(),FontId::monospace(7.0),cat.color());
    }
}
"""

with open(OUTFILE, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import os
lines = sum(1 for _ in open(OUTFILE, encoding="utf-8"))
print(f"physics_editor.rs now has {lines} lines")
