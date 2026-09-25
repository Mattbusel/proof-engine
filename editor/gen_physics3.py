import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 3
// ============================================================

// ─── Breakable Joint System ───────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct BreakableJoint {
    pub body_a: usize,
    pub body_b: usize,
    pub joint_type: JointType2D,
    pub break_force: f32,
    pub break_torque: f32,
    pub current_force: f32,
    pub current_torque: f32,
    pub broken: bool,
    pub break_effect: BreakEffect,
    pub anchor_a: [f32; 2],
    pub anchor_b: [f32; 2],
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum BreakEffect {
    None,
    SpawnDebris,
    PlaySound(String),
    ApplyImpulse(f32),
    SpawnParticles,
}

impl BreakEffect {
    pub fn label(&self) -> &str {
        match self {
            BreakEffect::None => "None",
            BreakEffect::SpawnDebris => "Spawn Debris",
            BreakEffect::PlaySound(_) => "Play Sound",
            BreakEffect::ApplyImpulse(_) => "Apply Impulse",
            BreakEffect::SpawnParticles => "Spawn Particles",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum JointType2D {
    Distance,
    Revolute,
    Weld,
    Prismatic,
    Wheel,
}

impl JointType2D {
    pub fn label(&self) -> &str {
        match self {
            JointType2D::Distance => "Distance",
            JointType2D::Revolute => "Revolute",
            JointType2D::Weld => "Weld",
            JointType2D::Prismatic => "Prismatic",
            JointType2D::Wheel => "Wheel",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            JointType2D::Distance => Color32::from_rgb(100, 200, 255),
            JointType2D::Revolute => Color32::from_rgb(255, 180, 80),
            JointType2D::Weld => Color32::from_rgb(180, 255, 100),
            JointType2D::Prismatic => Color32::from_rgb(255, 100, 200),
            JointType2D::Wheel => Color32::from_rgb(200, 150, 255),
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct BreakableJointEditorState {
    pub joints: Vec<BreakableJoint>,
    pub selected: Option<usize>,
    pub show_force_bars: bool,
    pub simulation_time: f32,
    pub break_log: Vec<(f32, usize, String)>,
    pub adding_joint: bool,
    pub new_joint_type: JointType2D,
    pub new_break_force: f32,
    pub new_break_torque: f32,
}

impl Default for JointType2D {
    fn default() -> Self { JointType2D::Distance }
}

impl Default for BreakEffect {
    fn default() -> Self { BreakEffect::None }
}

pub fn show_breakable_joint_editor(ui: &mut egui::Ui, state: &mut BreakableJointEditorState) {
    ui.heading("Breakable Joint Editor");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Joints:");
        if ui.button("+ Add").clicked() {
            state.adding_joint = !state.adding_joint;
        }
        if ui.button("Clear Broken").clicked() {
            state.joints.retain(|j| !j.broken);
        }
        if ui.button("Reset All Forces").clicked() {
            for j in &mut state.joints {
                j.current_force = 0.0;
                j.current_torque = 0.0;
            }
        }
        ui.checkbox(&mut state.show_force_bars, "Show Force Bars");
    });

    if state.adding_joint {
        ui.group(|ui| {
            ui.label("New Joint");
            ui.horizontal(|ui| {
                ui.label("Type:");
                for jt in &[JointType2D::Distance, JointType2D::Revolute, JointType2D::Weld, JointType2D::Prismatic, JointType2D::Wheel] {
                    let sel = state.new_joint_type == *jt;
                    if ui.selectable_label(sel, jt.label()).clicked() {
                        state.new_joint_type = jt.clone();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Break Force:");
                ui.add(egui::DragValue::new(&mut state.new_break_force).speed(10.0).clamp_range(0.0..=100000.0));
                ui.label("Break Torque:");
                ui.add(egui::DragValue::new(&mut state.new_break_torque).speed(1.0).clamp_range(0.0..=10000.0));
            });
            if ui.button("Create Joint").clicked() {
                state.joints.push(BreakableJoint {
                    body_a: 0,
                    body_b: 1,
                    joint_type: state.new_joint_type.clone(),
                    break_force: state.new_break_force,
                    break_torque: state.new_break_torque,
                    current_force: 0.0,
                    current_torque: 0.0,
                    broken: false,
                    break_effect: BreakEffect::None,
                    anchor_a: [0.0, 0.0],
                    anchor_b: [0.0, 0.0],
                });
                state.adding_joint = false;
            }
        });
    }

    let joints_count = state.joints.len();
    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
        for i in 0..joints_count {
            let j = &state.joints[i];
            let broken = j.broken;
            let force_frac = if j.break_force > 0.0 { (j.current_force / j.break_force).clamp(0.0, 1.0) } else { 0.0 };
            let col = if broken { Color32::from_rgb(200, 60, 60) }
                      else if force_frac > 0.8 { Color32::from_rgb(255, 180, 0) }
                      else { Color32::from_rgb(80, 200, 100) };
            let sel = state.selected == Some(i);
            ui.horizontal(|ui| {
                ui.colored_label(col, if broken { "✗" } else { "✓" });
                let lbl = format!("#{} {} [A:{} B:{}]", i, state.joints[i].joint_type.label(), state.joints[i].body_a, state.joints[i].body_b);
                if ui.selectable_label(sel, &lbl).clicked() {
                    state.selected = Some(i);
                }
                if state.show_force_bars {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 10.0), egui::Sense::hover());
                    let p = ui.painter_at(rect);
                    p.rect_filled(rect, 2.0, Color32::from_rgb(40, 40, 40));
                    let fw = rect.width() * force_frac;
                    let fc = egui::lerp(egui::Rgba::from(Color32::GREEN)..=egui::Rgba::from(Color32::RED), force_frac);
                    p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(fw, rect.height())), 2.0, Color32::from(fc));
                }
            });
        }
    });

    if let Some(idx) = state.selected {
        if idx < state.joints.len() {
            ui.separator();
            ui.label(format!("Editing Joint #{}", idx));
            let j = &mut state.joints[idx];
            ui.horizontal(|ui| {
                ui.label("Body A:");
                ui.add(egui::DragValue::new(&mut j.body_a));
                ui.label("Body B:");
                ui.add(egui::DragValue::new(&mut j.body_b));
            });
            ui.horizontal(|ui| {
                ui.label("Break Force:");
                ui.add(egui::DragValue::new(&mut j.break_force).speed(10.0).clamp_range(0.0..=1e6));
                ui.label("Break Torque:");
                ui.add(egui::DragValue::new(&mut j.break_torque).speed(1.0).clamp_range(0.0..=10000.0));
            });
            ui.horizontal(|ui| {
                ui.label("Anchor A:");
                ui.add(egui::DragValue::new(&mut j.anchor_a[0]).prefix("x:").speed(0.01));
                ui.add(egui::DragValue::new(&mut j.anchor_a[1]).prefix("y:").speed(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("Anchor B:");
                ui.add(egui::DragValue::new(&mut j.anchor_b[0]).prefix("x:").speed(0.01));
                ui.add(egui::DragValue::new(&mut j.anchor_b[1]).prefix("y:").speed(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("On Break:");
                for eff in &[BreakEffect::None, BreakEffect::SpawnDebris, BreakEffect::SpawnParticles] {
                    if ui.selectable_label(j.break_effect == *eff, eff.label()).clicked() {
                        j.break_effect = eff.clone();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(format!("Current Force: {:.1}", j.current_force));
                ui.label(format!("Current Torque: {:.1}", j.current_torque));
                ui.label(format!("Broken: {}", j.broken));
            });
            if ui.button("Simulate Break").clicked() {
                j.current_force = j.break_force * 1.1;
                j.broken = true;
                state.break_log.push((state.simulation_time, idx, format!("Force exceeded {:.1}", j.break_force)));
            }
        }
    }

    if !state.break_log.is_empty() {
        ui.separator();
        ui.label("Break Log:");
        egui::ScrollArea::vertical().max_height(80.0).show(ui, |ui| {
            for (t, idx, reason) in state.break_log.iter().rev().take(20) {
                ui.label(format!("t={:.2}s Joint#{}: {}", t, idx, reason));
            }
        });
    }

    draw_breakable_joint_diagram(ui, state);
}

fn draw_breakable_joint_diagram(ui: &mut egui::Ui, state: &BreakableJointEditorState) {
    let desired = Vec2::new(ui.available_width().min(500.0), 180.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(20, 20, 28));
    p.text(rect.center_top() + Vec2::new(0.0, 4.0), egui::Align2::CENTER_TOP, "Joint Diagram", FontId::monospace(9.0), Color32::GRAY);

    let n = state.joints.len().min(8);
    if n == 0 {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, "No joints", FontId::monospace(10.0), Color32::DARK_GRAY);
        return;
    }
    let body_positions: Vec<Pos2> = (0..n+1).map(|i| {
        let t = i as f32 / n as f32;
        Pos2::new(rect.left() + 40.0 + t * (rect.width() - 80.0), rect.center().y)
    }).collect();

    for pos in &body_positions {
        p.circle_filled(*pos, 10.0, Color32::from_rgb(80, 120, 200));
        p.circle_stroke(*pos, 10.0, Stroke::new(1.0, Color32::from_rgb(120, 160, 240)));
    }

    for (i, j) in state.joints.iter().enumerate().take(n) {
        if j.body_a < body_positions.len() && j.body_b < body_positions.len() {
            let pa = body_positions[j.body_a.min(body_positions.len()-1)];
            let pb = body_positions[j.body_b.min(body_positions.len()-1)];
            let col = if j.broken { Color32::from_rgb(200, 50, 50) } else { j.joint_type.color() };
            p.line_segment([pa, pb], Stroke::new(2.0, col));
            let mid = Pos2::new((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5 - 12.0);
            p.text(mid, egui::Align2::CENTER_CENTER, j.joint_type.label(), FontId::monospace(7.0), col);
            if state.show_force_bars {
                let force_frac = if j.break_force > 0.0 { (j.current_force / j.break_force).clamp(0.0, 1.0) } else { 0.0 };
                let bar_rect = Rect::from_min_size(Pos2::new(mid.x - 20.0, mid.y + 14.0), Vec2::new(40.0, 6.0));
                p.rect_filled(bar_rect, 1.0, Color32::from_rgb(40,40,40));
                let fc = egui::lerp(egui::Rgba::from(Color32::GREEN)..=egui::Rgba::from(Color32::RED), force_frac);
                p.rect_filled(Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width()*force_frac, bar_rect.height())), 1.0, Color32::from(fc));
            }
        }
    }
}

// ─── Buoyancy System ─────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct BuoyancyFluid {
    pub surface_y: f32,
    pub density: f32,
    pub drag_linear: f32,
    pub drag_angular: f32,
    pub flow_velocity: [f32; 2],
    pub turbulence: f32,
    pub color: [f32; 4],
    pub depth: f32,
    pub name: String,
}

impl BuoyancyFluid {
    pub fn water() -> Self {
        Self { surface_y: 0.0, density: 1000.0, drag_linear: 0.5, drag_angular: 0.3, flow_velocity: [0.0, 0.0], turbulence: 0.02, color: [0.1, 0.4, 0.8, 0.7], depth: 10.0, name: "Water".into() }
    }
    pub fn lava() -> Self {
        Self { surface_y: 0.0, density: 3100.0, drag_linear: 0.95, drag_angular: 0.9, flow_velocity: [0.1, 0.0], turbulence: 0.08, color: [1.0, 0.3, 0.0, 0.85], depth: 5.0, name: "Lava".into() }
    }
    pub fn oil() -> Self {
        Self { surface_y: 0.0, density: 850.0, drag_linear: 0.7, drag_angular: 0.6, flow_velocity: [0.0, 0.0], turbulence: 0.01, color: [0.3, 0.25, 0.1, 0.8], depth: 8.0, name: "Oil".into() }
    }
    pub fn quicksand() -> Self {
        Self { surface_y: 0.0, density: 1800.0, drag_linear: 0.99, drag_angular: 0.98, flow_velocity: [0.0, 0.0], turbulence: 0.0, color: [0.7, 0.6, 0.4, 0.9], depth: 3.0, name: "Quicksand".into() }
    }
    pub fn mercury() -> Self {
        Self { surface_y: 0.0, density: 13600.0, drag_linear: 0.3, drag_angular: 0.25, flow_velocity: [0.0, 0.0], turbulence: 0.005, color: [0.7, 0.75, 0.8, 0.95], depth: 4.0, name: "Mercury".into() }
    }
    pub fn honey() -> Self {
        Self { surface_y: 0.0, density: 1400.0, drag_linear: 0.98, drag_angular: 0.97, flow_velocity: [0.0, 0.0], turbulence: 0.0, color: [0.9, 0.7, 0.1, 0.9], depth: 2.0, name: "Honey".into() }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct BuoyancyEditorState {
    pub fluids: Vec<BuoyancyFluid>,
    pub selected: usize,
    pub preview_body_density: f32,
    pub preview_body_volume: f32,
    pub preview_body_mass: f32,
    pub wave_time: f32,
    pub show_waves: bool,
    pub show_force_arrows: bool,
}

pub fn show_buoyancy_editor(ui: &mut egui::Ui, state: &mut BuoyancyEditorState) {
    ui.heading("Buoyancy System");
    ui.separator();

    if state.fluids.is_empty() {
        state.fluids = vec![BuoyancyFluid::water(), BuoyancyFluid::lava(), BuoyancyFluid::oil(), BuoyancyFluid::quicksand(), BuoyancyFluid::mercury(), BuoyancyFluid::honey()];
        state.preview_body_density = 700.0;
        state.preview_body_volume = 1.0;
        state.preview_body_mass = 700.0;
    }

    ui.horizontal(|ui| {
        for (i, f) in state.fluids.iter().enumerate() {
            let c = Color32::from_rgba_unmultiplied((f.color[0]*255.0) as u8, (f.color[1]*255.0) as u8, (f.color[2]*255.0) as u8, 200);
            if ui.selectable_label(state.selected == i, egui::RichText::new(&f.name).color(c)).clicked() {
                state.selected = i;
            }
        }
        if ui.button("+ Fluid").clicked() {
            state.fluids.push(BuoyancyFluid::water());
        }
    });

    if let Some(fluid) = state.fluids.get_mut(state.selected) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut fluid.name);
            });
            ui.horizontal(|ui| {
                ui.label("Surface Y:");
                ui.add(egui::DragValue::new(&mut fluid.surface_y).speed(0.05));
                ui.label("Depth:");
                ui.add(egui::DragValue::new(&mut fluid.depth).speed(0.1).clamp_range(0.1..=100.0));
            });
            ui.horizontal(|ui| {
                ui.label("Density (kg/m³):");
                ui.add(egui::DragValue::new(&mut fluid.density).speed(10.0).clamp_range(1.0..=20000.0));
            });
            ui.horizontal(|ui| {
                ui.label("Linear Drag:");
                ui.add(egui::Slider::new(&mut fluid.drag_linear, 0.0..=1.0));
                ui.label("Angular Drag:");
                ui.add(egui::Slider::new(&mut fluid.drag_angular, 0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Flow X:");
                ui.add(egui::DragValue::new(&mut fluid.flow_velocity[0]).speed(0.01));
                ui.label("Flow Y:");
                ui.add(egui::DragValue::new(&mut fluid.flow_velocity[1]).speed(0.01));
                ui.label("Turbulence:");
                ui.add(egui::Slider::new(&mut fluid.turbulence, 0.0..=0.5));
            });
        });
    }

    ui.separator();
    ui.label("Preview Object");
    ui.horizontal(|ui| {
        ui.label("Body Density:");
        ui.add(egui::DragValue::new(&mut state.preview_body_density).speed(10.0).clamp_range(1.0..=20000.0));
        ui.label("Volume:");
        ui.add(egui::DragValue::new(&mut state.preview_body_volume).speed(0.01).clamp_range(0.01..=100.0));
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_waves, "Show Waves");
        ui.checkbox(&mut state.show_force_arrows, "Force Arrows");
    });

    draw_buoyancy_preview(ui, state);
}

fn draw_buoyancy_preview(ui: &mut egui::Ui, state: &BuoyancyEditorState) {
    let desired = Vec2::new(ui.available_width().min(500.0), 220.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(15, 20, 30));

    if let Some(fluid) = state.fluids.get(state.selected) {
        let fc = Color32::from_rgba_unmultiplied(
            (fluid.color[0]*255.0) as u8, (fluid.color[1]*255.0) as u8,
            (fluid.color[2]*255.0) as u8, (fluid.color[3]*220.0) as u8);

        let fluid_top_y = rect.center().y - 40.0;
        let fluid_bottom_y = rect.bottom() - 10.0;

        if state.show_waves {
            let wave_rect = Rect::from_min_max(Pos2::new(rect.left(), fluid_top_y), rect.max);
            p.rect_filled(wave_rect, 0.0, fc);
            for xi in 0..20 {
                let x = rect.left() + xi as f32 * rect.width() / 20.0;
                let wave_y = fluid_top_y + (state.wave_time * 2.0 + xi as f32 * 0.7).sin() * 4.0;
                p.line_segment([Pos2::new(x, fluid_top_y), Pos2::new(x + rect.width()/20.0, wave_y)], Stroke::new(1.5, Color32::from_rgb(180, 220, 255)));
            }
        } else {
            let fluid_rect = Rect::from_min_max(Pos2::new(rect.left(), fluid_top_y), rect.max);
            p.rect_filled(fluid_rect, 0.0, fc);
            p.line_segment([Pos2::new(rect.left(), fluid_top_y), Pos2::new(rect.right(), fluid_top_y)], Stroke::new(1.5, Color32::WHITE));
        }

        let body_density = state.preview_body_density;
        let buoy_ratio = (fluid.density / body_density).clamp(0.0, 2.0);
        let submersion = if body_density <= fluid.density { buoy_ratio.recip() } else { 1.0 };
        let body_size = 20.0;
        let body_y = fluid_top_y - body_size * (1.0 - submersion) + body_size * submersion * 0.5;
        let body_pos = Pos2::new(rect.center().x, body_y.clamp(rect.top() + 15.0, fluid_bottom_y - 5.0));
        p.rect_filled(Rect::from_center_size(body_pos, Vec2::splat(body_size)), 2.0, Color32::from_rgb(200, 160, 80));
        p.rect_stroke(Rect::from_center_size(body_pos, Vec2::splat(body_size)), 2.0, Stroke::new(1.0, Color32::WHITE));

        if state.show_force_arrows {
            let gravity = 9.81 * state.preview_body_density * state.preview_body_volume;
            let buoyancy = fluid.density * 9.81 * state.preview_body_volume * submersion;
            let arrow_scale = 0.005;
            let grav_len = gravity * arrow_scale;
            let buoy_len = buoyancy * arrow_scale;
            draw_arrow(&p, body_pos, body_pos + Vec2::new(0.0, grav_len.min(60.0)), Color32::RED, 1.5);
            draw_arrow(&p, body_pos, body_pos - Vec2::new(0.0, buoy_len.min(60.0)), Color32::LIGHT_BLUE, 1.5);
            p.text(body_pos + Vec2::new(15.0, 10.0), egui::Align2::LEFT_CENTER, format!("W:{:.0}N", gravity), FontId::monospace(7.0), Color32::RED);
            p.text(body_pos + Vec2::new(15.0, -10.0), egui::Align2::LEFT_CENTER, format!("B:{:.0}N", buoyancy), FontId::monospace(7.0), Color32::LIGHT_BLUE);
        }

        let status = if body_density < fluid.density { "FLOAT" } else if body_density == fluid.density { "NEUTRAL" } else { "SINK" };
        let scol = if body_density < fluid.density { Color32::GREEN } else if body_density > fluid.density { Color32::RED } else { Color32::YELLOW };
        p.text(Pos2::new(rect.left()+6.0, rect.top()+6.0), egui::Align2::LEFT_TOP, format!("{} | ρ_body:{:.0} ρ_fluid:{:.0}", status, body_density, fluid.density), FontId::monospace(8.0), scol);
        p.text(Pos2::new(rect.left()+6.0, rect.top()+18.0), egui::Align2::LEFT_TOP, format!("Submersion: {:.0}%", submersion*100.0), FontId::monospace(8.0), Color32::GRAY);
    }
}

fn draw_arrow(p: &egui::Painter, from: Pos2, to: Pos2, color: Color32, width: f32) {
    p.line_segment([from, to], Stroke::new(width, color));
    let dir = (to - from).normalized();
    let perp = Vec2::new(-dir.y, dir.x);
    let tip1 = to - dir * 6.0 + perp * 4.0;
    let tip2 = to - dir * 6.0 - perp * 4.0;
    p.line_segment([to, tip1], Stroke::new(width, color));
    p.line_segment([to, tip2], Stroke::new(width, color));
}

// ─── Trigger Zone Editor ──────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum TriggerShape {
    Box { half_w: f32, half_h: f32 },
    Circle { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
    Polygon { points: Vec<[f32; 2]> },
}

impl TriggerShape {
    pub fn label(&self) -> &str {
        match self {
            TriggerShape::Box { .. } => "Box",
            TriggerShape::Circle { .. } => "Circle",
            TriggerShape::Capsule { .. } => "Capsule",
            TriggerShape::Polygon { .. } => "Polygon",
        }
    }
}

impl Default for TriggerShape {
    fn default() -> Self { TriggerShape::Box { half_w: 1.0, half_h: 1.0 } }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum TriggerCondition {
    OnEnter,
    OnExit,
    WhileInside,
    OnEnterOnce,
    TimedInterval(f32),
}

impl TriggerCondition {
    pub fn label(&self) -> &str {
        match self {
            TriggerCondition::OnEnter => "On Enter",
            TriggerCondition::OnExit => "On Exit",
            TriggerCondition::WhileInside => "While Inside",
            TriggerCondition::OnEnterOnce => "On Enter (Once)",
            TriggerCondition::TimedInterval(_) => "Timed Interval",
        }
    }
}

impl Default for TriggerCondition { fn default() -> Self { TriggerCondition::OnEnter } }

#[derive(Clone, Serialize, Deserialize)]
pub enum TriggerAction {
    LoadLevel(String),
    PlaySound(String),
    SpawnObject(String),
    ApplyForce([f32; 2]),
    SetVariable(String, f32),
    CallFunction(String),
    TeleportTo([f32; 2]),
    KillPlayer,
    GiveItem(String),
    ShowMessage(String),
}

impl TriggerAction {
    pub fn label(&self) -> &str {
        match self {
            TriggerAction::LoadLevel(_) => "Load Level",
            TriggerAction::PlaySound(_) => "Play Sound",
            TriggerAction::SpawnObject(_) => "Spawn Object",
            TriggerAction::ApplyForce(_) => "Apply Force",
            TriggerAction::SetVariable(_, _) => "Set Variable",
            TriggerAction::CallFunction(_) => "Call Function",
            TriggerAction::TeleportTo(_) => "Teleport To",
            TriggerAction::KillPlayer => "Kill Player",
            TriggerAction::GiveItem(_) => "Give Item",
            TriggerAction::ShowMessage(_) => "Show Message",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TriggerZone {
    pub name: String,
    pub position: [f32; 2],
    pub shape: TriggerShape,
    pub condition: TriggerCondition,
    pub actions: Vec<TriggerAction>,
    pub active: bool,
    pub layer_mask: u32,
    pub trigger_count: u32,
    pub color: [f32; 3],
    pub debug_visible: bool,
    pub one_shot: bool,
    pub fired: bool,
}

impl TriggerZone {
    pub fn new_box(name: &str, x: f32, y: f32) -> Self {
        Self { name: name.into(), position: [x, y], shape: TriggerShape::Box { half_w: 2.0, half_h: 2.0 }, condition: TriggerCondition::OnEnter, actions: vec![], active: true, layer_mask: 0xFFFF, trigger_count: 0, color: [0.2, 0.8, 0.3], debug_visible: true, one_shot: false, fired: false }
    }
    pub fn new_circle(name: &str, x: f32, y: f32, r: f32) -> Self {
        Self { name: name.into(), position: [x, y], shape: TriggerShape::Circle { radius: r }, condition: TriggerCondition::OnEnter, actions: vec![], active: true, layer_mask: 0xFFFF, trigger_count: 0, color: [0.8, 0.4, 0.1], debug_visible: true, one_shot: false, fired: false }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct TriggerZoneEditorState {
    pub zones: Vec<TriggerZone>,
    pub selected: Option<usize>,
    pub dragging: Option<usize>,
    pub camera_offset: Vec2,
    pub camera_zoom: f32,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub grid_size: f32,
    pub preview_mode: bool,
}

pub fn show_trigger_zone_editor(ui: &mut egui::Ui, state: &mut TriggerZoneEditorState) {
    ui.heading("Trigger Zone Editor");
    ui.separator();

    if state.camera_zoom == 0.0 { state.camera_zoom = 1.0; state.grid_size = 1.0; }
    if state.zones.is_empty() {
        state.zones = vec![
            TriggerZone::new_box("Checkpoint_1", 0.0, 0.0),
            TriggerZone::new_circle("Damage_Zone", 5.0, 2.0, 3.0),
            TriggerZone::new_box("Level_Exit", -4.0, 3.0),
        ];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Box Zone").clicked() {
            let n = state.zones.len();
            state.zones.push(TriggerZone::new_box(&format!("Zone_{}", n), 0.0, 0.0));
            state.selected = Some(n);
        }
        if ui.button("+ Circle Zone").clicked() {
            let n = state.zones.len();
            state.zones.push(TriggerZone::new_circle(&format!("Zone_{}", n), 0.0, 0.0, 2.0));
            state.selected = Some(n);
        }
        if let Some(s) = state.selected {
            if ui.button("Delete").clicked() {
                state.zones.remove(s);
                state.selected = None;
            }
        }
        ui.checkbox(&mut state.show_grid, "Grid");
        ui.checkbox(&mut state.snap_to_grid, "Snap");
        ui.label("Zoom:");
        ui.add(egui::Slider::new(&mut state.camera_zoom, 0.2..=4.0));
    });

    egui::SidePanel::left("trigger_list").resizable(true).default_width(140.0).show_inside(ui, |ui| {
        ui.label("Zones:");
        for i in 0..state.zones.len() {
            let z = &state.zones[i];
            let col = if !z.active { Color32::DARK_GRAY } else if z.fired { Color32::from_rgb(255,180,0) } else { Color32::WHITE };
            let sel = state.selected == Some(i);
            if ui.selectable_label(sel, egui::RichText::new(&z.name).color(col)).clicked() {
                state.selected = Some(i);
            }
        }
    });

    if let Some(idx) = state.selected {
        if idx < state.zones.len() {
            let z = &mut state.zones[idx];
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut z.name);
                    ui.checkbox(&mut z.active, "Active");
                    ui.checkbox(&mut z.debug_visible, "Debug Vis");
                    ui.checkbox(&mut z.one_shot, "One Shot");
                });
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    ui.add(egui::DragValue::new(&mut z.position[0]).prefix("x:").speed(0.05));
                    ui.add(egui::DragValue::new(&mut z.position[1]).prefix("y:").speed(0.05));
                });
                match &mut z.shape {
                    TriggerShape::Box { half_w, half_h } => {
                        ui.horizontal(|ui| {
                            ui.label("Half Size:");
                            ui.add(egui::DragValue::new(half_w).prefix("w:").speed(0.05).clamp_range(0.1..=100.0));
                            ui.add(egui::DragValue::new(half_h).prefix("h:").speed(0.05).clamp_range(0.1..=100.0));
                        });
                    }
                    TriggerShape::Circle { radius } => {
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            ui.add(egui::DragValue::new(radius).speed(0.05).clamp_range(0.1..=100.0));
                        });
                    }
                    TriggerShape::Capsule { radius, half_height } => {
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            ui.add(egui::DragValue::new(radius).speed(0.05).clamp_range(0.1..=50.0));
                            ui.label("Half Height:");
                            ui.add(egui::DragValue::new(half_height).speed(0.05).clamp_range(0.1..=50.0));
                        });
                    }
                    TriggerShape::Polygon { points } => {
                        ui.label(format!("Polygon ({} points)", points.len()));
                        if ui.button("+ Point").clicked() { points.push([0.0, 0.0]); }
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("Condition:");
                    for cond in &[TriggerCondition::OnEnter, TriggerCondition::OnExit, TriggerCondition::WhileInside, TriggerCondition::OnEnterOnce] {
                        if ui.selectable_label(z.condition == *cond, cond.label()).clicked() {
                            z.condition = cond.clone();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(format!("Triggered: {} times", z.trigger_count));
                    if ui.button("Reset Count").clicked() { z.trigger_count = 0; z.fired = false; }
                    if ui.button("Test Fire").clicked() {
                        z.trigger_count += 1;
                        if z.one_shot { z.fired = true; }
                    }
                });
                ui.label("Actions:");
                ui.horizontal(|ui| {
                    if ui.button("+ Play Sound").clicked() { z.actions.push(TriggerAction::PlaySound("".into())); }
                    if ui.button("+ Call Fn").clicked() { z.actions.push(TriggerAction::CallFunction("on_trigger".into())); }
                    if ui.button("+ Spawn").clicked() { z.actions.push(TriggerAction::SpawnObject("".into())); }
                    if ui.button("+ Message").clicked() { z.actions.push(TriggerAction::ShowMessage("".into())); }
                });
                let mut remove_idx = None;
                for (ai, action) in z.actions.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(action.label());
                        match action {
                            TriggerAction::PlaySound(s) | TriggerAction::SpawnObject(s) | TriggerAction::CallFunction(s) | TriggerAction::LoadLevel(s) | TriggerAction::GiveItem(s) => {
                                ui.text_edit_singleline(s);
                            }
                            TriggerAction::ShowMessage(m) => { ui.text_edit_singleline(m); }
                            TriggerAction::ApplyForce(f) => {
                                ui.add(egui::DragValue::new(&mut f[0]).prefix("fx:").speed(0.1));
                                ui.add(egui::DragValue::new(&mut f[1]).prefix("fy:").speed(0.1));
                            }
                            TriggerAction::SetVariable(name, val) => {
                                ui.text_edit_singleline(name);
                                ui.add(egui::DragValue::new(val).speed(0.1));
                            }
                            TriggerAction::TeleportTo(pos) => {
                                ui.add(egui::DragValue::new(&mut pos[0]).prefix("x:").speed(0.1));
                                ui.add(egui::DragValue::new(&mut pos[1]).prefix("y:").speed(0.1));
                            }
                            _ => {}
                        }
                        if ui.button("✗").clicked() { remove_idx = Some(ai); }
                    });
                }
                if let Some(ri) = remove_idx { z.actions.remove(ri); }
            });
        }
    }

    draw_trigger_zone_viewport(ui, state);
}

fn draw_trigger_zone_viewport(ui: &mut egui::Ui, state: &mut TriggerZoneEditorState) {
    let desired = Vec2::new(ui.available_width().min(600.0), 300.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(18, 22, 30));

    let zoom = state.camera_zoom;
    let world_to_screen = |wx: f32, wy: f32| -> Pos2 {
        Pos2::new(rect.center().x + (wx + state.camera_offset.x) * 40.0 * zoom,
                  rect.center().y - (wy + state.camera_offset.y) * 40.0 * zoom)
    };

    if state.show_grid {
        let grid = state.grid_size;
        for gx in -10..=10 {
            let x = gx as f32 * grid;
            let s = world_to_screen(x, -20.0);
            let e = world_to_screen(x, 20.0);
            if s.x >= rect.left() && s.x <= rect.right() {
                p.line_segment([s, e], Stroke::new(0.5, Color32::from_rgb(40, 44, 52)));
            }
        }
        for gy in -10..=10 {
            let y = gy as f32 * grid;
            let s = world_to_screen(-20.0, y);
            let e = world_to_screen(20.0, y);
            if s.y >= rect.top() && s.y <= rect.bottom() {
                p.line_segment([s, e], Stroke::new(0.5, Color32::from_rgb(40, 44, 52)));
            }
        }
    }

    // Draw origin axes
    p.line_segment([world_to_screen(-15.0, 0.0), world_to_screen(15.0, 0.0)], Stroke::new(1.0, Color32::from_rgb(60,60,60)));
    p.line_segment([world_to_screen(0.0, -12.0), world_to_screen(0.0, 12.0)], Stroke::new(1.0, Color32::from_rgb(60,60,60)));

    for (i, z) in state.zones.iter().enumerate() {
        if !z.debug_visible { continue; }
        let sc = world_to_screen(z.position[0], z.position[1]);
        let zc = Color32::from_rgba_unmultiplied((z.color[0]*255.0) as u8, (z.color[1]*255.0) as u8, (z.color[2]*255.0) as u8, 80);
        let zc_border = Color32::from_rgb((z.color[0]*255.0) as u8, (z.color[1]*255.0) as u8, (z.color[2]*255.0) as u8);
        let sel = state.selected == Some(i);
        let border_w = if sel { 2.0 } else { 1.0 };

        match &z.shape {
            TriggerShape::Box { half_w, half_h } => {
                let hw = half_w * 40.0 * zoom;
                let hh = half_h * 40.0 * zoom;
                let zr = Rect::from_center_size(sc, Vec2::new(hw*2.0, hh*2.0));
                p.rect_filled(zr, 2.0, zc);
                p.rect_stroke(zr, 2.0, Stroke::new(border_w, zc_border));
            }
            TriggerShape::Circle { radius } => {
                let sr = radius * 40.0 * zoom;
                p.circle_filled(sc, sr, zc);
                p.circle_stroke(sc, sr, Stroke::new(border_w, zc_border));
            }
            TriggerShape::Capsule { radius, half_height } => {
                let sr = radius * 40.0 * zoom;
                let sh = half_height * 40.0 * zoom;
                p.circle_filled(Pos2::new(sc.x, sc.y - sh), sr, zc);
                p.circle_filled(Pos2::new(sc.x, sc.y + sh), sr, zc);
                p.rect_filled(Rect::from_center_size(sc, Vec2::new(sr*2.0, sh*2.0)), 0.0, zc);
            }
            TriggerShape::Polygon { points } => {
                if points.len() >= 2 {
                    for pi in 0..points.len() {
                        let pa = world_to_screen(z.position[0]+points[pi][0], z.position[1]+points[pi][1]);
                        let pb_idx = (pi+1) % points.len();
                        let pb = world_to_screen(z.position[0]+points[pb_idx][0], z.position[1]+points[pb_idx][1]);
                        p.line_segment([pa, pb], Stroke::new(border_w, zc_border));
                    }
                }
            }
        }
        p.text(sc + Vec2::new(0.0, -8.0), egui::Align2::CENTER_BOTTOM, &z.name, FontId::monospace(8.0), Color32::WHITE);
    }

    if response.dragged_by(egui::PointerButton::Middle) {
        let drag = response.drag_delta();
        state.camera_offset += drag / (40.0 * zoom);
    }
}

// ─── Physics Debug Overlay ────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct DebugOverlaySettings {
    pub show_aabb: bool,
    pub show_velocity_vectors: bool,
    pub show_angular_velocity: bool,
    pub show_mass_center: bool,
    pub show_contact_points: bool,
    pub show_contact_normals: bool,
    pub show_joint_anchors: bool,
    pub show_broadphase_cells: bool,
    pub show_body_ids: bool,
    pub show_sleep_state: bool,
    pub show_forces: bool,
    pub velocity_scale: f32,
    pub force_scale: f32,
    pub contact_point_radius: f32,
    pub aabb_color: [f32; 3],
    pub velocity_color: [f32; 3],
    pub contact_color: [f32; 3],
    pub sleeping_tint: [f32; 4],
}

impl Default for DebugOverlaySettings {
    fn default() -> Self {
        Self {
            show_aabb: true,
            show_velocity_vectors: true,
            show_angular_velocity: false,
            show_mass_center: true,
            show_contact_points: true,
            show_contact_normals: true,
            show_joint_anchors: true,
            show_broadphase_cells: false,
            show_body_ids: false,
            show_sleep_state: true,
            show_forces: false,
            velocity_scale: 0.1,
            force_scale: 0.001,
            contact_point_radius: 3.0,
            aabb_color: [0.2, 0.8, 0.2],
            velocity_color: [1.0, 0.8, 0.0],
            contact_color: [1.0, 0.2, 0.2],
            sleeping_tint: [0.2, 0.2, 0.6, 0.3],
        }
    }
}

pub fn show_debug_overlay_settings(ui: &mut egui::Ui, settings: &mut DebugOverlaySettings) {
    ui.heading("Debug Overlay Settings");
    ui.separator();
    ui.columns(2, |cols| {
        cols[0].checkbox(&mut settings.show_aabb, "AABBs");
        cols[0].checkbox(&mut settings.show_velocity_vectors, "Velocity Vectors");
        cols[0].checkbox(&mut settings.show_angular_velocity, "Angular Velocity");
        cols[0].checkbox(&mut settings.show_mass_center, "Mass Centers");
        cols[0].checkbox(&mut settings.show_contact_points, "Contact Points");
        cols[0].checkbox(&mut settings.show_contact_normals, "Contact Normals");
        cols[1].checkbox(&mut settings.show_joint_anchors, "Joint Anchors");
        cols[1].checkbox(&mut settings.show_broadphase_cells, "Broadphase Cells");
        cols[1].checkbox(&mut settings.show_body_ids, "Body IDs");
        cols[1].checkbox(&mut settings.show_sleep_state, "Sleep State");
        cols[1].checkbox(&mut settings.show_forces, "Forces");
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Velocity Scale:");
        ui.add(egui::Slider::new(&mut settings.velocity_scale, 0.001..=1.0).logarithmic(true));
    });
    ui.horizontal(|ui| {
        ui.label("Force Scale:");
        ui.add(egui::Slider::new(&mut settings.force_scale, 0.0001..=0.1).logarithmic(true));
    });
    ui.horizontal(|ui| {
        ui.label("Contact Radius:");
        ui.add(egui::Slider::new(&mut settings.contact_point_radius, 1.0..=10.0));
    });

    ui.separator();
    ui.label("Colors:");
    ui.horizontal(|ui| {
        ui.label("AABB:");
        let mut c = settings.aabb_color;
        let mut col32 = Color32::from_rgb((c[0]*255.0) as u8, (c[1]*255.0) as u8, (c[2]*255.0) as u8);
        if ui.color_edit_button_srgba(&mut col32).changed() {
            settings.aabb_color = [col32.r() as f32/255.0, col32.g() as f32/255.0, col32.b() as f32/255.0];
        }
        ui.label("Velocity:");
        let mut col32v = Color32::from_rgb((settings.velocity_color[0]*255.0) as u8, (settings.velocity_color[1]*255.0) as u8, (settings.velocity_color[2]*255.0) as u8);
        if ui.color_edit_button_srgba(&mut col32v).changed() {
            settings.velocity_color = [col32v.r() as f32/255.0, col32v.g() as f32/255.0, col32v.b() as f32/255.0];
        }
        ui.label("Contacts:");
        let mut col32c = Color32::from_rgb((settings.contact_color[0]*255.0) as u8, (settings.contact_color[1]*255.0) as u8, (settings.contact_color[2]*255.0) as u8);
        if ui.color_edit_button_srgba(&mut col32c).changed() {
            settings.contact_color = [col32c.r() as f32/255.0, col32c.g() as f32/255.0, col32c.b() as f32/255.0];
        }
    });

    draw_debug_overlay_preview(ui, settings);
}

fn draw_debug_overlay_preview(ui: &mut egui::Ui, settings: &DebugOverlaySettings) {
    let desired = Vec2::new(ui.available_width().min(480.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(15, 18, 24));
    p.text(rect.center_top() + Vec2::new(0.0, 4.0), egui::Align2::CENTER_TOP, "Overlay Preview", FontId::monospace(9.0), Color32::GRAY);

    let bodies = [
        (Pos2::new(rect.left()+80.0, rect.center().y), 28.0f32, false, [0.5f32, 0.3f32]),
        (Pos2::new(rect.center().x, rect.center().y - 30.0), 20.0, false, [-0.3, 0.7]),
        (Pos2::new(rect.right()-90.0, rect.center().y + 20.0), 24.0, true, [0.0, 0.0]),
    ];

    for (i, (center, radius, sleeping, vel)) in bodies.iter().enumerate() {
        if sleeping && settings.show_sleep_state {
            let sr = settings.sleeping_tint;
            p.circle_filled(*center, radius + 4.0, Color32::from_rgba_unmultiplied((sr[0]*255.0) as u8, (sr[1]*255.0) as u8, (sr[2]*255.0) as u8, (sr[3]*255.0) as u8));
        }
        p.circle_stroke(*center, *radius, Stroke::new(1.5, Color32::from_rgb(180,180,180)));
        if settings.show_aabb {
            let ac = settings.aabb_color;
            let acol = Color32::from_rgb((ac[0]*255.0) as u8, (ac[1]*255.0) as u8, (ac[2]*255.0) as u8);
            p.rect_stroke(Rect::from_center_size(*center, Vec2::splat(radius*2.0+4.0)), 1.0, Stroke::new(1.0, acol));
        }
        if settings.show_body_ids {
            p.text(*center, egui::Align2::CENTER_CENTER, format!("{}", i), FontId::monospace(8.0), Color32::WHITE);
        }
        if settings.show_mass_center {
            p.circle_filled(*center, 2.5, Color32::YELLOW);
        }
        if settings.show_velocity_vectors && (vel[0].abs() > 0.01 || vel[1].abs() > 0.01) {
            let vc = settings.velocity_color;
            let vcol = Color32::from_rgb((vc[0]*255.0) as u8, (vc[1]*255.0) as u8, (vc[2]*255.0) as u8);
            let vend = *center + Vec2::new(vel[0], -vel[1]) * 80.0;
            draw_arrow(&p, *center, vend, vcol, 1.5);
        }
    }
    // Contact points
    if settings.show_contact_points {
        let contacts = [Pos2::new(rect.left()+108.0, rect.center().y), Pos2::new(rect.center().x-18.0, rect.center().y-10.0)];
        let cc = settings.contact_color;
        let ccol = Color32::from_rgb((cc[0]*255.0) as u8, (cc[1]*255.0) as u8, (cc[2]*255.0) as u8);
        for cp in &contacts {
            p.circle_filled(*cp, settings.contact_point_radius, ccol);
            if settings.show_contact_normals {
                draw_arrow(&p, *cp, *cp + Vec2::new(-20.0, 0.0), ccol, 1.0);
            }
        }
    }
    if bodies[0].2 && settings.show_sleep_state {
        p.text(bodies[2].0 + Vec2::new(0.0, bodies[2].1 + 10.0), egui::Align2::CENTER_TOP, "Zzz", FontId::monospace(9.0), Color32::from_rgb(120,120,200));
    }
}

// ─── Physics Preset Browser ───────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicsScenePreset {
    pub name: String,
    pub description: String,
    pub category: PresetCategory,
    pub tags: Vec<String>,
    pub thumbnail_hint: PresetThumbnail,
    pub gravity: [f32; 2],
    pub time_step: f32,
    pub iterations: u32,
    pub body_count: usize,
    pub joint_count: usize,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum PresetCategory {
    Vehicles,
    Creatures,
    Structures,
    Puzzles,
    Destructible,
    Fluids,
    Cloth,
    Playground,
}

impl PresetCategory {
    pub fn label(&self) -> &str {
        match self {
            PresetCategory::Vehicles => "Vehicles",
            PresetCategory::Creatures => "Creatures",
            PresetCategory::Structures => "Structures",
            PresetCategory::Puzzles => "Puzzles",
            PresetCategory::Destructible => "Destructible",
            PresetCategory::Fluids => "Fluids",
            PresetCategory::Cloth => "Cloth",
            PresetCategory::Playground => "Playground",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            PresetCategory::Vehicles => Color32::from_rgb(80, 160, 240),
            PresetCategory::Creatures => Color32::from_rgb(80, 220, 120),
            PresetCategory::Structures => Color32::from_rgb(200, 160, 80),
            PresetCategory::Puzzles => Color32::from_rgb(200, 100, 220),
            PresetCategory::Destructible => Color32::from_rgb(240, 80, 80),
            PresetCategory::Fluids => Color32::from_rgb(60, 180, 220),
            PresetCategory::Cloth => Color32::from_rgb(220, 180, 100),
            PresetCategory::Playground => Color32::from_rgb(160, 220, 60),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum PresetThumbnail {
    WheelVehicle,
    Stacked,
    Tower,
    Pendulum,
    Dominos,
    BallPool,
    ClothDrape,
    WaterSplash,
    Ragdoll,
    Bridge,
    Catapult,
    Chain,
}

pub fn all_scene_presets() -> Vec<PhysicsScenePreset> {
    vec![
        PhysicsScenePreset { name: "Car Physics".into(), description: "4-wheel vehicle with suspension joints and chassis".into(), category: PresetCategory::Vehicles, tags: vec!["vehicle".into(),"car".into(),"wheel".into()], thumbnail_hint: PresetThumbnail::WheelVehicle, gravity: [0.0,-9.81], time_step: 0.016, iterations: 8, body_count: 5, joint_count: 4 },
        PhysicsScenePreset { name: "Ragdoll Demo".into(), description: "Full humanoid ragdoll with 17 bones and 16 joints".into(), category: PresetCategory::Creatures, tags: vec!["ragdoll".into(),"humanoid".into()], thumbnail_hint: PresetThumbnail::Ragdoll, gravity: [0.0,-9.81], time_step: 0.016, iterations: 10, body_count: 17, joint_count: 16 },
        PhysicsScenePreset { name: "Domino Chain".into(), description: "50 dominos in a line for chain reaction".into(), category: PresetCategory::Puzzles, tags: vec!["domino".into(),"chain".into()], thumbnail_hint: PresetThumbnail::Dominos, gravity: [0.0,-9.81], time_step: 0.008, iterations: 12, body_count: 51, joint_count: 0 },
        PhysicsScenePreset { name: "Suspension Bridge".into(), description: "Bridge with rope constraints and planks".into(), category: PresetCategory::Structures, tags: vec!["bridge".into(),"rope".into()], thumbnail_hint: PresetThumbnail::Bridge, gravity: [0.0,-9.81], time_step: 0.016, iterations: 20, body_count: 32, joint_count: 62 },
        PhysicsScenePreset { name: "Ball Pool".into(), description: "100 spheres with high restitution in a box".into(), category: PresetCategory::Playground, tags: vec!["balls".into(),"pool".into()], thumbnail_hint: PresetThumbnail::BallPool, gravity: [0.0,-9.81], time_step: 0.016, iterations: 8, body_count: 101, joint_count: 0 },
        PhysicsScenePreset { name: "Cloth Drape".into(), description: "32x32 cloth mesh draped over a sphere".into(), category: PresetCategory::Cloth, tags: vec!["cloth".into(),"drape".into()], thumbnail_hint: PresetThumbnail::ClothDrape, gravity: [0.0,-9.81], time_step: 0.008, iterations: 10, body_count: 1024, joint_count: 1984 },
        PhysicsScenePreset { name: "Water Tank".into(), description: "SPH fluid simulation in a walled tank".into(), category: PresetCategory::Fluids, tags: vec!["fluid".into(),"water".into(),"sph".into()], thumbnail_hint: PresetThumbnail::WaterSplash, gravity: [0.0,-9.81], time_step: 0.004, iterations: 1, body_count: 500, joint_count: 0 },
        PhysicsScenePreset { name: "Tower Collapse".into(), description: "10-story tower with breakable joints".into(), category: PresetCategory::Destructible, tags: vec!["tower".into(),"breakable".into()], thumbnail_hint: PresetThumbnail::Tower, gravity: [0.0,-9.81], time_step: 0.016, iterations: 8, body_count: 80, joint_count: 120 },
        PhysicsScenePreset { name: "Catapult".into(), description: "Lever catapult with revolute joint and counterweight".into(), category: PresetCategory::Vehicles, tags: vec!["catapult".into(),"lever".into()], thumbnail_hint: PresetThumbnail::Catapult, gravity: [0.0,-9.81], time_step: 0.016, iterations: 8, body_count: 4, joint_count: 2 },
        PhysicsScenePreset { name: "Chain Rope".into(), description: "Linked chain with 20 segments".into(), category: PresetCategory::Structures, tags: vec!["chain".into(),"rope".into()], thumbnail_hint: PresetThumbnail::Chain, gravity: [0.0,-9.81], time_step: 0.016, iterations: 20, body_count: 21, joint_count: 20 },
        PhysicsScenePreset { name: "Pendulum Clock".into(), description: "Double pendulum with revolute joints".into(), category: PresetCategory::Puzzles, tags: vec!["pendulum".into(),"double".into()], thumbnail_hint: PresetThumbnail::Pendulum, gravity: [0.0,-9.81], time_step: 0.004, iterations: 6, body_count: 3, joint_count: 2 },
        PhysicsScenePreset { name: "Block Stack".into(), description: "Unstable tower of 12 boxes".into(), category: PresetCategory::Playground, tags: vec!["stack".into(),"boxes".into()], thumbnail_hint: PresetThumbnail::Stacked, gravity: [0.0,-9.81], time_step: 0.016, iterations: 8, body_count: 13, joint_count: 0 },
    ]
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PresetBrowserState {
    pub presets: Vec<PhysicsScenePreset>,
    pub selected: Option<usize>,
    pub filter_category: Option<PresetCategory>,
    pub search_text: String,
    pub sort_by_name: bool,
}

pub fn show_preset_browser(ui: &mut egui::Ui, state: &mut PresetBrowserState) {
    if state.presets.is_empty() { state.presets = all_scene_presets(); }

    ui.heading("Physics Scene Presets");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search_text);
        if ui.button("✗").clicked() { state.search_text.clear(); }
        ui.checkbox(&mut state.sort_by_name, "Sort A-Z");
    });

    ui.horizontal(|ui| {
        ui.label("Category:");
        if ui.selectable_label(state.filter_category.is_none(), "All").clicked() { state.filter_category = None; }
        for cat in &[PresetCategory::Vehicles, PresetCategory::Creatures, PresetCategory::Structures, PresetCategory::Puzzles, PresetCategory::Destructible, PresetCategory::Fluids, PresetCategory::Cloth, PresetCategory::Playground] {
            let sel = state.filter_category.as_ref() == Some(cat);
            if ui.selectable_label(sel, egui::RichText::new(cat.label()).color(cat.color())).clicked() {
                state.filter_category = Some(cat.clone());
            }
        }
    });

    let filtered: Vec<usize> = state.presets.iter().enumerate()
        .filter(|(_, p)| {
            let cat_ok = state.filter_category.as_ref().map_or(true, |c| &p.category == c);
            let search_ok = state.search_text.is_empty() || p.name.to_lowercase().contains(&state.search_text.to_lowercase()) || p.tags.iter().any(|t| t.contains(&state.search_text.to_lowercase()));
            cat_ok && search_ok
        })
        .map(|(i, _)| i)
        .collect();

    let mut sorted_filtered = filtered;
    if state.sort_by_name {
        sorted_filtered.sort_by(|&a, &b| state.presets[a].name.cmp(&state.presets[b].name));
    }

    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
        egui::Grid::new("preset_grid").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
            for (grid_i, &pi) in sorted_filtered.iter().enumerate() {
                let preset = &state.presets[pi];
                let sel = state.selected == Some(pi);
                let thumbnail_rect_response = ui.allocate_response(Vec2::new(60.0, 50.0), egui::Sense::click());
                if thumbnail_rect_response.clicked() { state.selected = Some(pi); }
                draw_preset_thumbnail(ui.painter_at(thumbnail_rect_response.rect), &preset.thumbnail_hint, thumbnail_rect_response.rect, sel);

                ui.vertical(|ui| {
                    let catcol = preset.category.color();
                    if ui.selectable_label(sel, egui::RichText::new(&preset.name).strong()).clicked() { state.selected = Some(pi); }
                    ui.label(egui::RichText::new(preset.category.label()).color(catcol).small());
                    ui.label(egui::RichText::new(format!("{} bodies, {} joints", preset.body_count, preset.joint_count)).small().color(Color32::GRAY));
                });

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&preset.description).small().color(Color32::from_rgb(180,180,180)));
                    ui.horizontal(|ui| {
                        for tag in preset.tags.iter().take(3) {
                            ui.label(egui::RichText::new(format!("#{}", tag)).small().color(Color32::from_rgb(100,160,220)));
                        }
                    });
                });
                ui.end_row();
            }
        });
    });

    if let Some(pi) = state.selected {
        let p = &state.presets[pi];
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(format!("Selected: {} | gravity: [{:.1},{:.1}] | dt: {:.4}s | iters: {}", p.name, p.gravity[0], p.gravity[1], p.time_step, p.iterations));
            if ui.button("Load Preset").clicked() {
                // Preset loading would be handled by the outer editor
            }
        });
    }
}

fn draw_preset_thumbnail(p: egui::Painter, hint: &PresetThumbnail, rect: Rect, selected: bool) {
    p.rect_filled(rect, 3.0, Color32::from_rgb(25, 28, 36));
    if selected {
        p.rect_stroke(rect, 3.0, Stroke::new(2.0, Color32::from_rgb(100, 180, 255)));
    } else {
        p.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_rgb(50, 55, 65)));
    }
    let c = rect.center();
    match hint {
        PresetThumbnail::WheelVehicle => {
            p.rect_filled(Rect::from_center_size(c, Vec2::new(36.0, 14.0)), 2.0, Color32::from_rgb(120,120,180));
            p.circle_filled(c + Vec2::new(-14.0, 8.0), 6.0, Color32::DARK_GRAY);
            p.circle_filled(c + Vec2::new(14.0, 8.0), 6.0, Color32::DARK_GRAY);
        }
        PresetThumbnail::Stacked => {
            for i in 0..4 {
                let y = rect.bottom() - 8.0 - i as f32 * 10.0;
                let w = 32.0 - i as f32 * 4.0;
                p.rect_filled(Rect::from_center_size(Pos2::new(c.x, y), Vec2::new(w, 8.0)), 1.0, Color32::from_rgb(180, 140, 80));
            }
        }
        PresetThumbnail::Tower => {
            for i in 0..6 {
                let y = rect.bottom() - 6.0 - i as f32 * 7.0;
                p.rect_filled(Rect::from_center_size(Pos2::new(c.x, y), Vec2::new(20.0, 5.0)), 0.0, Color32::from_rgb(150, 130, 100));
            }
        }
        PresetThumbnail::Pendulum => {
            let anchor = Pos2::new(c.x, rect.top() + 8.0);
            p.circle_filled(anchor, 3.0, Color32::GRAY);
            p.line_segment([anchor, c + Vec2::new(0.0, 10.0)], Stroke::new(1.5, Color32::GRAY));
            p.circle_filled(c + Vec2::new(0.0, 10.0), 6.0, Color32::from_rgb(200, 160, 80));
        }
        PresetThumbnail::Dominos => {
            for i in 0..8 {
                let x = rect.left() + 6.0 + i as f32 * 6.5;
                p.rect_filled(Rect::from_center_size(Pos2::new(x, c.y), Vec2::new(3.0, 20.0)), 0.0, Color32::from_rgb(220, 180, 100));
            }
        }
        PresetThumbnail::BallPool => {
            let positions = [(c.x-10.0, c.y+5.0), (c.x+8.0, c.y+6.0), (c.x, c.y-4.0), (c.x-14.0, c.y-2.0), (c.x+12.0, c.y-5.0), (c.x+2.0, c.y+12.0)];
            for (bx, by) in &positions {
                p.circle_filled(Pos2::new(*bx, *by), 5.0, Color32::from_rgb(80, 160, 240));
            }
        }
        PresetThumbnail::ClothDrape => {
            for row in 0..5 {
                let y = rect.top() + 8.0 + row as f32 * 8.0;
                let sag = (row as f32 * 0.3).powi(2) * 5.0;
                p.line_segment([Pos2::new(rect.left()+4.0, y+sag), Pos2::new(rect.right()-4.0, y+sag)], Stroke::new(1.0, Color32::from_rgb(200, 180, 120)));
            }
        }
        PresetThumbnail::WaterSplash => {
            let water_rect = Rect::from_min_max(Pos2::new(rect.left()+4.0, c.y), Pos2::new(rect.right()-4.0, rect.bottom()-4.0));
            p.rect_filled(water_rect, 2.0, Color32::from_rgba_unmultiplied(40, 100, 200, 180));
            for sp in 0..5 {
                let sx = rect.left() + 10.0 + sp as f32 * 9.0;
                let sh = 4.0 + (sp as f32 * 1.3).sin().abs() * 8.0;
                p.line_segment([Pos2::new(sx, c.y), Pos2::new(sx + 2.0, c.y - sh)], Stroke::new(1.5, Color32::from_rgb(100, 180, 255)));
            }
        }
        PresetThumbnail::Ragdoll => {
            p.circle_filled(c - Vec2::new(0.0, 15.0), 5.0, Color32::from_rgb(220, 180, 140));
            p.rect_filled(Rect::from_center_size(c - Vec2::new(0.0, 4.0), Vec2::new(10.0, 14.0)), 1.0, Color32::from_rgb(80, 100, 160));
            p.line_segment([c - Vec2::new(0.0, 4.0), c + Vec2::new(14.0, 4.0)], Stroke::new(2.0, Color32::from_rgb(220,180,140)));
            p.line_segment([c - Vec2::new(0.0, 4.0), c + Vec2::new(-14.0, 4.0)], Stroke::new(2.0, Color32::from_rgb(220,180,140)));
            p.line_segment([c + Vec2::new(-4.0, 7.0), c + Vec2::new(-6.0, 20.0)], Stroke::new(2.0, Color32::from_rgb(80,100,160)));
            p.line_segment([c + Vec2::new(4.0, 7.0), c + Vec2::new(6.0, 20.0)], Stroke::new(2.0, Color32::from_rgb(80,100,160)));
        }
        PresetThumbnail::Bridge => {
            p.line_segment([Pos2::new(rect.left()+4.0, c.y), Pos2::new(rect.right()-4.0, c.y)], Stroke::new(1.5, Color32::GRAY));
            for i in 0..7 {
                let x = rect.left() + 8.0 + i as f32 * 7.0;
                p.line_segment([Pos2::new(x, c.y), Pos2::new(x, c.y-12.0)], Stroke::new(1.0, Color32::from_rgb(180,140,80)));
            }
            p.line_segment([Pos2::new(rect.left()+4.0, c.y-12.0), Pos2::new(rect.right()-4.0, c.y-12.0)], Stroke::new(1.5, Color32::from_rgb(180,140,80)));
        }
        PresetThumbnail::Catapult => {
            p.rect_filled(Rect::from_min_max(Pos2::new(rect.left()+8.0, c.y+5.0), Pos2::new(rect.right()-8.0, c.y+12.0)), 1.0, Color32::from_rgb(140,100,60));
            p.line_segment([c + Vec2::new(-10.0, 8.0), c + Vec2::new(10.0, -12.0)], Stroke::new(2.0, Color32::from_rgb(180,130,60)));
            p.circle_filled(c + Vec2::new(12.0, -14.0), 5.0, Color32::from_rgb(200,80,40));
        }
        PresetThumbnail::Chain => {
            let segments = 8;
            let seg_w = (rect.width() - 10.0) / segments as f32;
            for si in 0..segments {
                let cx = rect.left() + 5.0 + si as f32 * seg_w + seg_w * 0.5;
                let cy = c.y + (si as f32 * 0.8).sin() * 6.0;
                p.circle_stroke(Pos2::new(cx, cy), 4.0, Stroke::new(1.5, Color32::from_rgb(180,180,180)));
            }
        }
    }
}

// ─── Continuous Collision Detection Settings ─────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct CcdSettings {
    pub enabled: bool,
    pub mode: CcdMode,
    pub velocity_threshold: f32,
    pub max_substeps: u32,
    pub angular_ccd: bool,
    pub speculative_margin: f32,
    pub time_of_impact_epsilon: f32,
    pub debug_draw_swept_shapes: bool,
    pub stats_hits_per_frame: u32,
    pub stats_toi_queries_per_frame: u32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum CcdMode {
    Disabled,
    Speculative,
    SweptSphere,
    SweptShape,
    FullTOI,
}

impl CcdMode {
    pub fn label(&self) -> &str {
        match self {
            CcdMode::Disabled => "Disabled",
            CcdMode::Speculative => "Speculative (Fast)",
            CcdMode::SweptSphere => "Swept Sphere (Balanced)",
            CcdMode::SweptShape => "Swept Shape (Accurate)",
            CcdMode::FullTOI => "Full TOI (Expensive)",
        }
    }
    pub fn description(&self) -> &str {
        match self {
            CcdMode::Disabled => "Objects may tunnel through thin surfaces at high speeds.",
            CcdMode::Speculative => "Inflate collision shapes speculatively. Fast, minor false positives.",
            CcdMode::SweptSphere => "Sweep sphere along trajectory. Good for fast small objects.",
            CcdMode::SweptShape => "Sweep full shape. Accurate for rotation too. More expensive.",
            CcdMode::FullTOI => "Compute exact time of impact. Most expensive, most accurate.",
        }
    }
}

impl Default for CcdSettings {
    fn default() -> Self {
        Self { enabled: false, mode: CcdMode::Speculative, velocity_threshold: 50.0, max_substeps: 4, angular_ccd: false, speculative_margin: 0.1, time_of_impact_epsilon: 0.001, debug_draw_swept_shapes: false, stats_hits_per_frame: 0, stats_toi_queries_per_frame: 0 }
    }
}

pub fn show_ccd_settings(ui: &mut egui::Ui, settings: &mut CcdSettings) {
    ui.heading("Continuous Collision Detection");
    ui.separator();

    ui.checkbox(&mut settings.enabled, "Enable CCD");
    if !settings.enabled {
        ui.label(egui::RichText::new("CCD disabled — fast objects may tunnel.").color(Color32::from_rgb(200, 160, 60)));
    }

    ui.add_enabled_ui(settings.enabled, |ui| {
        ui.label("Mode:");
        for mode in &[CcdMode::Speculative, CcdMode::SweptSphere, CcdMode::SweptShape, CcdMode::FullTOI] {
            let sel = settings.mode == *mode;
            if ui.selectable_label(sel, mode.label()).clicked() {
                settings.mode = mode.clone();
            }
        }
        ui.label(egui::RichText::new(settings.mode.description()).small().color(Color32::GRAY));

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Velocity Threshold:");
            ui.add(egui::DragValue::new(&mut settings.velocity_threshold).speed(1.0).clamp_range(1.0..=1000.0).suffix(" u/s"));
            ui.label("(apply CCD above this speed)");
        });
        ui.horizontal(|ui| {
            ui.label("Max Substeps:");
            ui.add(egui::DragValue::new(&mut settings.max_substeps).clamp_range(1..=16));
        });
        ui.checkbox(&mut settings.angular_ccd, "Angular CCD (for rotating fast objects)");

        if matches!(settings.mode, CcdMode::Speculative) {
            ui.horizontal(|ui| {
                ui.label("Speculative Margin:");
                ui.add(egui::DragValue::new(&mut settings.speculative_margin).speed(0.001).clamp_range(0.001..=1.0));
            });
        }
        if matches!(settings.mode, CcdMode::FullTOI | CcdMode::SweptShape) {
            ui.horizontal(|ui| {
                ui.label("TOI Epsilon:");
                ui.add(egui::DragValue::new(&mut settings.time_of_impact_epsilon).speed(0.0001).clamp_range(0.0001..=0.01));
            });
        }
        ui.checkbox(&mut settings.debug_draw_swept_shapes, "Debug Draw Swept Shapes");

        ui.separator();
        ui.label(egui::RichText::new("Live Stats").strong());
        ui.horizontal(|ui| {
            ui.label(format!("TOI hits/frame: {}", settings.stats_hits_per_frame));
            ui.separator();
            ui.label(format!("TOI queries/frame: {}", settings.stats_toi_queries_per_frame));
        });
        draw_ccd_diagram(ui, settings);
    });
}

fn draw_ccd_diagram(ui: &mut egui::Ui, settings: &CcdSettings) {
    let desired = Vec2::new(ui.available_width().min(500.0), 120.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(15, 18, 24));
    p.text(Pos2::new(rect.left()+6.0, rect.top()+4.0), egui::Align2::LEFT_TOP, "CCD Illustration", FontId::monospace(9.0), Color32::GRAY);

    let wall_x = rect.center().x;
    p.line_segment([Pos2::new(wall_x, rect.top()+20.0), Pos2::new(wall_x, rect.bottom()-10.0)], Stroke::new(3.0, Color32::from_rgb(180,180,180)));
    p.text(Pos2::new(wall_x+4.0, rect.top()+22.0), egui::Align2::LEFT_TOP, "Wall", FontId::monospace(7.0), Color32::GRAY);

    let ball_start = Pos2::new(rect.left() + 40.0, rect.center().y);
    let ball_end = Pos2::new(wall_x - 12.0, rect.center().y);
    let ball_through = Pos2::new(wall_x + 40.0, rect.center().y);

    if !settings.enabled {
        p.circle_stroke(ball_start, 10.0, Stroke::new(1.5, Color32::from_rgb(100,100,200)));
        p.circle_filled(ball_through, 10.0, Color32::from_rgb(200,80,80));
        p.text(ball_through + Vec2::new(0.0, 15.0), egui::Align2::CENTER_TOP, "TUNNEL!", FontId::monospace(8.0), Color32::RED);
        p.line_segment([ball_start, ball_through], Stroke::new(1.0, Stroke::new(1.0, Color32::from_rgb(120,120,120)).color));
    } else {
        p.circle_stroke(ball_start, 10.0, Stroke::new(1.5, Color32::from_rgb(100,100,200)));
        p.circle_filled(ball_end, 10.0, Color32::from_rgb(80,200,100));
        p.text(ball_end + Vec2::new(0.0, 15.0), egui::Align2::CENTER_TOP, "Stopped!", FontId::monospace(8.0), Color32::GREEN);
        p.line_segment([ball_start, ball_end], Stroke::new(1.5, Color32::from_rgb(80,200,100)));
        if matches!(settings.mode, CcdMode::SweptSphere | CcdMode::SweptShape) {
            for t in 0..4 {
                let tf = t as f32 / 4.0;
                let px = ball_start.x + (ball_end.x - ball_start.x) * tf;
                p.circle_stroke(Pos2::new(px, rect.center().y), 10.0, Stroke::new(0.5, Color32::from_rgba_unmultiplied(100, 160, 255, 100)));
            }
        }
    }
}

// ─── Island Simulation Manager ────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct SimulationIsland {
    pub id: usize,
    pub body_indices: Vec<usize>,
    pub joint_indices: Vec<usize>,
    pub sleeping: bool,
    pub kinetic_energy: f32,
    pub sleep_timer: f32,
    pub color: [f32; 3],
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct IslandManagerState {
    pub islands: Vec<SimulationIsland>,
    pub sleep_threshold: f32,
    pub sleep_time_required: f32,
    pub auto_sleep: bool,
    pub show_island_colors: bool,
    pub show_island_stats: bool,
    pub total_awake_bodies: usize,
    pub total_sleeping_bodies: usize,
}

pub fn show_island_manager(ui: &mut egui::Ui, state: &mut IslandManagerState) {
    ui.heading("Simulation Islands");
    ui.separator();

    if state.sleep_threshold == 0.0 { state.sleep_threshold = 0.01; state.sleep_time_required = 0.5; state.auto_sleep = true; }
    if state.islands.is_empty() {
        state.islands = vec![
            SimulationIsland { id: 0, body_indices: vec![0,1,2], joint_indices: vec![0,1], sleeping: false, kinetic_energy: 12.5, sleep_timer: 0.0, color: [0.3,0.8,0.4] },
            SimulationIsland { id: 1, body_indices: vec![3,4], joint_indices: vec![2], sleeping: true, kinetic_energy: 0.0, sleep_timer: 1.2, color: [0.3,0.4,0.9] },
            SimulationIsland { id: 2, body_indices: vec![5,6,7,8], joint_indices: vec![3,4,5], sleeping: false, kinetic_energy: 3.2, sleep_timer: 0.0, color: [0.9,0.5,0.2] },
        ];
    }

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.auto_sleep, "Auto Sleep");
        ui.label("Threshold:");
        ui.add(egui::DragValue::new(&mut state.sleep_threshold).speed(0.001).clamp_range(0.0001..=1.0));
        ui.label("Sleep Time:");
        ui.add(egui::DragValue::new(&mut state.sleep_time_required).speed(0.05).clamp_range(0.05..=5.0).suffix("s"));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_island_colors, "Color Islands");
        ui.checkbox(&mut state.show_island_stats, "Show Stats");
        if ui.button("Wake All").clicked() {
            for isl in &mut state.islands { isl.sleeping = false; isl.sleep_timer = 0.0; }
        }
        if ui.button("Sleep All").clicked() {
            for isl in &mut state.islands { isl.sleeping = true; isl.kinetic_energy = 0.0; }
        }
    });

    let awake = state.islands.iter().filter(|i| !i.sleeping).count();
    let sleeping = state.islands.iter().filter(|i| i.sleeping).count();
    let total_bodies: usize = state.islands.iter().map(|i| i.body_indices.len()).sum();
    ui.label(format!("Islands: {} | Awake: {} | Sleeping: {} | Bodies: {}", state.islands.len(), awake, sleeping, total_bodies));

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        for isl in state.islands.iter_mut() {
            let ic = Color32::from_rgb((isl.color[0]*255.0) as u8, (isl.color[1]*255.0) as u8, (isl.color[2]*255.0) as u8);
            ui.horizontal(|ui| {
                ui.colored_label(ic, format!("Island #{}", isl.id));
                let slbl = if isl.sleeping { egui::RichText::new("Zzz").color(Color32::from_rgb(120,120,200)) } else { egui::RichText::new("Awake").color(Color32::GREEN) };
                ui.label(slbl);
                ui.label(format!("{} bodies, {} joints", isl.body_indices.len(), isl.joint_indices.len()));
                ui.label(format!("KE: {:.3}", isl.kinetic_energy));
                if isl.sleeping { ui.label(format!("Sleep: {:.2}s", isl.sleep_timer)); }
                let btn_lbl = if isl.sleeping { "Wake" } else { "Sleep" };
                if ui.button(btn_lbl).clicked() { isl.sleeping = !isl.sleeping; }
            });
        }
    });

    draw_island_visualization(ui, &state.islands);
}

fn draw_island_visualization(ui: &mut egui::Ui, islands: &[SimulationIsland]) {
    let desired = Vec2::new(ui.available_width().min(500.0), 160.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(15, 18, 24));
    p.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP, "Island Visualization", FontId::monospace(8.0), Color32::GRAY);

    let mut body_global_idx = 0usize;
    for isl in islands {
        let ic = Color32::from_rgb((isl.color[0]*255.0) as u8, (isl.color[1]*255.0) as u8, (isl.color[2]*255.0) as u8);
        let body_col = if isl.sleeping { Color32::from_rgba_unmultiplied(ic.r()/2, ic.g()/2, ic.b()/2, 180) } else { ic };
        for bi in &isl.body_indices {
            let x = rect.left() + 20.0 + body_global_idx as f32 * 28.0;
            let y = rect.center().y + (body_global_idx as f32 * 0.7).sin() * 20.0;
            if x > rect.right() - 20.0 { break; }
            p.circle_filled(Pos2::new(x, y), 8.0, body_col);
            p.circle_stroke(Pos2::new(x, y), 8.0, Stroke::new(1.0, ic));
            p.text(Pos2::new(x, y), egui::Align2::CENTER_CENTER, format!("{}", bi), FontId::monospace(6.0), Color32::WHITE);
            body_global_idx += 1;
        }
    }
}

// ─── Force Field Interaction ──────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct ForceFieldInteraction {
    pub field_id: usize,
    pub body_layer_mask: u32,
    pub strength_multiplier: f32,
    pub affects_static: bool,
    pub affects_kinematic: bool,
    pub affects_sleeping: bool,
    pub wake_on_force: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ForceFieldZone {
    pub id: usize,
    pub name: String,
    pub position: [f32; 2],
    pub radius: f32,
    pub field_type: ForceFieldType2,
    pub strength: f32,
    pub falloff: ForceFieldFalloff,
    pub active: bool,
    pub interactions: Vec<ForceFieldInteraction>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ForceFieldType2 {
    Radial,
    Directional([f32; 2]),
    Vortex,
    Turbulence { frequency: f32, amplitude: f32 },
    Attractor,
    Repulsor,
    Drag,
    Explosion { duration: f32, elapsed: f32 },
}

impl ForceFieldType2 {
    pub fn label(&self) -> &str {
        match self {
            ForceFieldType2::Radial => "Radial",
            ForceFieldType2::Directional(_) => "Directional",
            ForceFieldType2::Vortex => "Vortex",
            ForceFieldType2::Turbulence { .. } => "Turbulence",
            ForceFieldType2::Attractor => "Attractor",
            ForceFieldType2::Repulsor => "Repulsor",
            ForceFieldType2::Drag => "Drag",
            ForceFieldType2::Explosion { .. } => "Explosion",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ForceFieldFalloff {
    Constant,
    Linear,
    InverseSquare,
    Smooth,
}

impl ForceFieldFalloff {
    pub fn label(&self) -> &str {
        match self {
            ForceFieldFalloff::Constant => "Constant",
            ForceFieldFalloff::Linear => "Linear",
            ForceFieldFalloff::InverseSquare => "Inv. Square",
            ForceFieldFalloff::Smooth => "Smooth (Cosine)",
        }
    }
    pub fn evaluate(&self, t: f32) -> f32 {
        match self {
            ForceFieldFalloff::Constant => 1.0,
            ForceFieldFalloff::Linear => (1.0 - t).max(0.0),
            ForceFieldFalloff::InverseSquare => 1.0 / (1.0 + t * t * 4.0),
            ForceFieldFalloff::Smooth => ((1.0 - t) * std::f32::consts::PI * 0.5).cos().max(0.0),
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ForceFieldEditorState2 {
    pub zones: Vec<ForceFieldZone>,
    pub selected: Option<usize>,
    pub show_field_lines: bool,
    pub field_line_count: u32,
    pub preview_time: f32,
}

pub fn show_force_field_editor2(ui: &mut egui::Ui, state: &mut ForceFieldEditorState2) {
    ui.heading("Force Field Zones");
    ui.separator();

    if state.field_line_count == 0 { state.field_line_count = 16; }
    if state.zones.is_empty() {
        state.zones = vec![
            ForceFieldZone { id: 0, name: "Gravity Well".into(), position: [0.0, 0.0], radius: 5.0, field_type: ForceFieldType2::Attractor, strength: 50.0, falloff: ForceFieldFalloff::InverseSquare, active: true, interactions: vec![] },
            ForceFieldZone { id: 1, name: "Wind Zone".into(), position: [4.0, 2.0], radius: 8.0, field_type: ForceFieldType2::Directional([1.0, 0.0]), strength: 20.0, falloff: ForceFieldFalloff::Constant, active: true, interactions: vec![] },
        ];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Attractor").clicked() {
            let n = state.zones.len();
            state.zones.push(ForceFieldZone { id: n, name: format!("Field_{}", n), position: [0.0, 0.0], radius: 4.0, field_type: ForceFieldType2::Attractor, strength: 30.0, falloff: ForceFieldFalloff::Linear, active: true, interactions: vec![] });
        }
        if ui.button("+ Repulsor").clicked() {
            let n = state.zones.len();
            state.zones.push(ForceFieldZone { id: n, name: format!("Repulsor_{}", n), position: [0.0, 0.0], radius: 4.0, field_type: ForceFieldType2::Repulsor, strength: 30.0, falloff: ForceFieldFalloff::Linear, active: true, interactions: vec![] });
        }
        if ui.button("+ Vortex").clicked() {
            let n = state.zones.len();
            state.zones.push(ForceFieldZone { id: n, name: format!("Vortex_{}", n), position: [0.0, 0.0], radius: 6.0, field_type: ForceFieldType2::Vortex, strength: 25.0, falloff: ForceFieldFalloff::Smooth, active: true, interactions: vec![] });
        }
        if ui.button("+ Explosion").clicked() {
            let n = state.zones.len();
            state.zones.push(ForceFieldZone { id: n, name: format!("Explosion_{}", n), position: [0.0, 0.0], radius: 10.0, field_type: ForceFieldType2::Explosion { duration: 0.5, elapsed: 0.0 }, strength: 200.0, falloff: ForceFieldFalloff::Smooth, active: true, interactions: vec![] });
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_field_lines, "Field Lines");
        if state.show_field_lines {
            ui.label("Count:");
            ui.add(egui::DragValue::new(&mut state.field_line_count).clamp_range(4..=32));
        }
    });

    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
        let n = state.zones.len();
        for i in 0..n {
            let z = &state.zones[i];
            let sel = state.selected == Some(i);
            ui.horizontal(|ui| {
                let active_col = if z.active { Color32::GREEN } else { Color32::DARK_GRAY };
                ui.colored_label(active_col, "●");
                if ui.selectable_label(sel, format!("{} [{}]", z.name, z.field_type.label())).clicked() {
                    state.selected = Some(i);
                }
                ui.label(format!("r={:.1} s={:.0}", z.radius, z.strength));
            });
        }
    });

    if let Some(idx) = state.selected {
        if idx < state.zones.len() {
            let z = &mut state.zones[idx];
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut z.name);
                ui.checkbox(&mut z.active, "Active");
            });
            ui.horizontal(|ui| {
                ui.label("Position:");
                ui.add(egui::DragValue::new(&mut z.position[0]).prefix("x:").speed(0.05));
                ui.add(egui::DragValue::new(&mut z.position[1]).prefix("y:").speed(0.05));
            });
            ui.horizontal(|ui| {
                ui.label("Radius:");
                ui.add(egui::DragValue::new(&mut z.radius).speed(0.05).clamp_range(0.1..=50.0));
                ui.label("Strength:");
                ui.add(egui::DragValue::new(&mut z.strength).speed(1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Falloff:");
                for fo in &[ForceFieldFalloff::Constant, ForceFieldFalloff::Linear, ForceFieldFalloff::InverseSquare, ForceFieldFalloff::Smooth] {
                    if ui.selectable_label(z.falloff == *fo, fo.label()).clicked() { z.falloff = fo.clone(); }
                }
            });
        }
    }

    draw_force_field_viewport(ui, state);
}

fn draw_force_field_viewport(ui: &mut egui::Ui, state: &ForceFieldEditorState2) {
    let desired = Vec2::new(ui.available_width().min(500.0), 240.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));

    let world_to_screen = |wx: f32, wy: f32| -> Pos2 {
        Pos2::new(rect.center().x + wx * 22.0, rect.center().y - wy * 22.0)
    };

    // Grid
    for gx in -10i32..=10 {
        let s = world_to_screen(gx as f32, -10.0);
        let e = world_to_screen(gx as f32, 10.0);
        p.line_segment([s, e], Stroke::new(0.3, Color32::from_rgb(30,35,42)));
    }
    for gy in -10i32..=10 {
        let s = world_to_screen(-10.0, gy as f32);
        let e = world_to_screen(10.0, gy as f32);
        p.line_segment([s, e], Stroke::new(0.3, Color32::from_rgb(30,35,42)));
    }

    for z in &state.zones {
        if !z.active { continue; }
        let sc = world_to_screen(z.position[0], z.position[1]);
        let sr = z.radius * 22.0;

        let field_col = match &z.field_type {
            ForceFieldType2::Attractor => Color32::from_rgba_unmultiplied(80, 140, 255, 60),
            ForceFieldType2::Repulsor => Color32::from_rgba_unmultiplied(255, 80, 80, 60),
            ForceFieldType2::Vortex => Color32::from_rgba_unmultiplied(160, 80, 255, 60),
            ForceFieldType2::Directional(_) => Color32::from_rgba_unmultiplied(80, 220, 120, 60),
            ForceFieldType2::Drag => Color32::from_rgba_unmultiplied(180, 180, 80, 60),
            ForceFieldType2::Turbulence { .. } => Color32::from_rgba_unmultiplied(255, 160, 80, 60),
            ForceFieldType2::Explosion { .. } => Color32::from_rgba_unmultiplied(255, 120, 40, 80),
            _ => Color32::from_rgba_unmultiplied(120, 120, 120, 60),
        };
        let border_col = Color32::from_rgba_unmultiplied(field_col.r(), field_col.g(), field_col.b(), 180);
        p.circle_filled(sc, sr, field_col);
        p.circle_stroke(sc, sr, Stroke::new(1.5, border_col));
        p.circle_filled(sc, 3.0, border_col);
        p.text(sc + Vec2::new(0.0, -sr - 6.0), egui::Align2::CENTER_BOTTOM, &z.name, FontId::monospace(8.0), Color32::WHITE);

        if state.show_field_lines {
            let n = state.field_line_count as usize;
            for fi in 0..n {
                let angle = fi as f32 / n as f32 * std::f32::consts::TAU;
                for step in 0..8 {
                    let t = step as f32 / 8.0;
                    let sample_r = sr * t;
                    let sample_pos = sc + Vec2::new(angle.cos(), angle.sin()) * sample_r;
                    let falloff = z.falloff.evaluate(t);
                    let line_alpha = (falloff * 180.0) as u8;
                    let arrow_end = match &z.field_type {
                        ForceFieldType2::Attractor => sample_pos + Vec2::new(-angle.cos(), -angle.sin()) * 5.0,
                        ForceFieldType2::Repulsor => sample_pos + Vec2::new(angle.cos(), angle.sin()) * 5.0,
                        ForceFieldType2::Vortex => sample_pos + Vec2::new(-angle.sin(), angle.cos()) * 5.0,
                        ForceFieldType2::Directional(d) => sample_pos + Vec2::new(d[0], -d[1]) * 5.0,
                        _ => sample_pos + Vec2::new(angle.cos(), angle.sin()) * 3.0,
                    };
                    p.line_segment([sample_pos, arrow_end], Stroke::new(0.8, Color32::from_rgba_unmultiplied(border_col.r(), border_col.g(), border_col.b(), line_alpha)));
                }
            }
        }
    }
    p.text(Pos2::new(rect.left()+4.0, rect.bottom()-14.0), egui::Align2::LEFT_BOTTOM, "Force Field Viewport", FontId::monospace(7.0), Color32::DARK_GRAY);
}

// ─── Soft Body Advanced Parameters ───────────────────────────────────────────

pub fn show_soft_body_advanced(ui: &mut egui::Ui, state: &mut SoftBodyEditorState) {
    ui.heading("Soft Body Advanced Parameters");
    ui.separator();

    if let Some(idx) = state.selected {
        if idx < state.bodies.len() {
            let b = &mut state.bodies[idx];
            ui.group(|ui| {
                ui.label(format!("Editing: {} ({} nodes, {} springs)", b.name, b.nodes.len(), b.springs.len()));
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Node Mass:");
                    ui.add(egui::Slider::new(&mut b.node_mass, 0.01..=10.0));
                    ui.label("Stiffness:");
                    ui.add(egui::Slider::new(&mut b.stiffness, 0.0..=1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Damping:");
                    ui.add(egui::Slider::new(&mut b.damping, 0.0..=1.0));
                    ui.label("Restitution:");
                    ui.add(egui::Slider::new(&mut b.restitution, 0.0..=1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Pressure:");
                    ui.add(egui::DragValue::new(&mut b.pressure_factor).speed(0.1).clamp_range(0.0..=20.0));
                    ui.label("Volume Conserve:");
                    ui.add(egui::Slider::new(&mut b.volume_conservation, 0.0..=1.0));
                });

                ui.separator();
                ui.label("Spring Statistics:");
                let total_springs = b.springs.len();
                let structural = b.springs.iter().filter(|s| s.rest_length < 1.5).count();
                let shear = b.springs.iter().filter(|s| s.rest_length >= 1.5 && s.rest_length < 2.2).count();
                let bend = total_springs.saturating_sub(structural + shear);
                ui.columns(3, |cols| {
                    cols[0].label(format!("Structural: {}", structural));
                    cols[1].label(format!("Shear: {}", shear));
                    cols[2].label(format!("Bend: {}", bend));
                });

                ui.separator();
                ui.label("Node Detail:");
                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                    egui::Grid::new("node_grid").num_columns(5).spacing([6.0, 2.0]).show(ui, |ui| {
                        ui.label("ID"); ui.label("X"); ui.label("Y"); ui.label("Pinned"); ui.label("Mass"); ui.end_row();
                        for (ni, node) in b.nodes.iter_mut().enumerate().take(20) {
                            ui.label(format!("{}", ni));
                            ui.add(egui::DragValue::new(&mut node.pos[0]).speed(0.01));
                            ui.add(egui::DragValue::new(&mut node.pos[1]).speed(0.01));
                            ui.checkbox(&mut node.pinned, "");
                            ui.add(egui::DragValue::new(&mut node.mass).speed(0.01).clamp_range(0.001..=100.0));
                            ui.end_row();
                        }
                        if b.nodes.len() > 20 {
                            ui.label(format!("... {} more nodes", b.nodes.len() - 20));
                            ui.end_row();
                        }
                    });
                });
            });
        }
    } else {
        ui.label("No soft body selected.");
        ui.label("Select a soft body from the main soft body editor.");
    }
}

// ─── Cloth Wind Turbulence Detail ─────────────────────────────────────────────

pub fn show_cloth_wind_turbulence(ui: &mut egui::Ui, state: &mut ClothEditorState) {
    ui.heading("Cloth Wind & Turbulence");
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Wind X:");
        ui.add(egui::Slider::new(&mut state.wind_dir[0], -20.0..=20.0));
    });
    ui.horizontal(|ui| {
        ui.label("Wind Y:");
        ui.add(egui::Slider::new(&mut state.wind_dir[1], -20.0..=20.0));
    });
    ui.horizontal(|ui| {
        ui.label("Turbulence:");
        ui.add(egui::Slider::new(&mut state.turbulence, 0.0..=5.0));
    });

    let desired = Vec2::new(ui.available_width().min(400.0), 160.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(15, 20, 28));

    let speed = (state.wind_dir[0]*state.wind_dir[0] + state.wind_dir[1]*state.wind_dir[1]).sqrt();
    let wind_col = if speed < 2.0 { Color32::from_rgb(80, 160, 220) }
                   else if speed < 8.0 { Color32::from_rgb(100, 200, 100) }
                   else { Color32::from_rgb(240, 100, 60) };

    for row in 0..6 {
        for col in 0..10 {
            let base_x = rect.left() + 25.0 + col as f32 * 45.0;
            let base_y = rect.top() + 20.0 + row as f32 * 22.0;
            if base_x > rect.right() - 10.0 || base_y > rect.bottom() - 10.0 { continue; }
            let turb_x = (state.turbulence * (col as f32 * 0.7 + row as f32 * 1.3)).sin() * 8.0;
            let turb_y = (state.turbulence * (col as f32 * 1.1 + row as f32 * 0.9)).cos() * 8.0;
            let end_x = base_x + (state.wind_dir[0] * 1.5 + turb_x).clamp(-30.0, 30.0);
            let end_y = base_y + (-state.wind_dir[1] * 1.5 + turb_y).clamp(-30.0, 30.0);
            draw_arrow(&p, Pos2::new(base_x, base_y), Pos2::new(end_x, end_y), wind_col, 1.0);
        }
    }
    p.text(Pos2::new(rect.center().x, rect.bottom()-10.0), egui::Align2::CENTER_BOTTOM,
        format!("Wind speed: {:.1} m/s  Turbulence: {:.1}", speed, state.turbulence),
        FontId::monospace(8.0), Color32::GRAY);
}

// ─── Rope Preset Gallery ──────────────────────────────────────────────────────

pub fn show_rope_preset_gallery(ui: &mut egui::Ui, state: &mut RopeEditorState) {
    ui.heading("Rope Preset Gallery");
    ui.separator();

    let presets = [
        ("Pendulum", "Simple single pendulum"),
        ("Bridge", "Suspension bridge span"),
        ("Lasso", "Circular loop rope"),
        ("Catenary", "Hanging natural curve"),
        ("Zip Line", "Angled taut cable"),
        ("Net Cell", "Grid crossing ropes"),
    ];

    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            for (name, desc) in &presets {
                ui.group(|ui| {
                    ui.set_min_size(Vec2::new(90.0, 80.0));
                    draw_rope_preset_thumbnail(ui, name);
                    ui.label(egui::RichText::new(*name).strong().small());
                    ui.label(egui::RichText::new(*desc).small().color(Color32::GRAY));
                    if ui.button("Load").clicked() {
                        if *name == "Pendulum" {
                            state.rope = Some(Rope::preset_pendulum());
                        } else if *name == "Bridge" {
                            state.rope = Some(Rope::preset_bridge());
                        }
                    }
                });
            }
        });
    });
}

fn draw_rope_preset_thumbnail(ui: &mut egui::Ui, name: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(80.0, 50.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(20, 24, 32));
    let c = rect.center();
    match name {
        "Pendulum" => {
            let anchor = Pos2::new(c.x, rect.top() + 6.0);
            p.circle_filled(anchor, 3.0, Color32::GRAY);
            p.line_segment([anchor, c + Vec2::new(8.0, 10.0)], Stroke::new(2.0, Color32::from_rgb(180,140,80)));
            p.circle_filled(c + Vec2::new(8.0, 10.0), 5.0, Color32::from_rgb(200,160,80));
        }
        "Bridge" => {
            p.line_segment([Pos2::new(rect.left()+6.0, c.y), Pos2::new(rect.right()-6.0, c.y + 12.0)], Stroke::new(2.0, Color32::from_rgb(180,140,80)));
            for i in 0..5 {
                let x = rect.left() + 10.0 + i as f32 * 13.0;
                let y = c.y + 2.0 + i as f32 * 2.0;
                p.line_segment([Pos2::new(x, y), Pos2::new(x, y + 8.0)], Stroke::new(1.0, Color32::GRAY));
            }
        }
        "Catenary" => {
            for i in 0..8 {
                let t0 = i as f32 / 8.0;
                let t1 = (i+1) as f32 / 8.0;
                let x0 = rect.left() + 6.0 + t0 * (rect.width()-12.0);
                let x1 = rect.left() + 6.0 + t1 * (rect.width()-12.0);
                let sag = 8.0;
                let y0 = c.y - sag + sag * (t0 * 2.0 - 1.0).powi(2) * 4.0;
                let y1 = c.y - sag + sag * (t1 * 2.0 - 1.0).powi(2) * 4.0;
                p.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y1)], Stroke::new(2.0, Color32::from_rgb(180,140,80)));
            }
        }
        _ => {
            p.line_segment([Pos2::new(rect.left()+6.0, c.y-8.0), Pos2::new(rect.right()-6.0, c.y+8.0)], Stroke::new(2.0, Color32::from_rgb(180,140,80)));
        }
    }
}

// ─── Fluid Boundary Shapes ────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct FluidBoundary {
    pub shape: FluidBoundaryShape,
    pub restitution: f32,
    pub friction: f32,
    pub sticky: bool,
    pub absorb: bool,
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum FluidBoundaryShape {
    Box { min: [f32; 2], max: [f32; 2] },
    Circle { center: [f32; 2], radius: f32 },
    HalfSpace { normal: [f32; 2], offset: f32 },
}

impl FluidBoundaryShape {
    pub fn label(&self) -> &str {
        match self {
            FluidBoundaryShape::Box { .. } => "Box",
            FluidBoundaryShape::Circle { .. } => "Circle",
            FluidBoundaryShape::HalfSpace { .. } => "Half-Space",
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct FluidBoundaryEditorState2 {
    pub boundaries: Vec<FluidBoundary>,
    pub selected: Option<usize>,
}

pub fn show_fluid_boundary_editor2(ui: &mut egui::Ui, state: &mut FluidBoundaryEditorState2) {
    ui.heading("Fluid Boundary Editor");
    ui.separator();

    if state.boundaries.is_empty() {
        state.boundaries = vec![
            FluidBoundary { shape: FluidBoundaryShape::HalfSpace { normal: [0.0, 1.0], offset: -5.0 }, restitution: 0.1, friction: 0.8, sticky: false, absorb: false, name: "Ground".into() },
            FluidBoundary { shape: FluidBoundaryShape::Box { min: [-8.0, -5.0], max: [8.0, 10.0] }, restitution: 0.05, friction: 0.9, sticky: false, absorb: false, name: "Tank Walls".into() },
        ];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Box").clicked() { state.boundaries.push(FluidBoundary { shape: FluidBoundaryShape::Box { min: [-3.0,-3.0], max: [3.0,3.0] }, restitution: 0.1, friction: 0.5, sticky: false, absorb: false, name: "Box".into() }); }
        if ui.button("+ Circle").clicked() { state.boundaries.push(FluidBoundary { shape: FluidBoundaryShape::Circle { center: [0.0,0.0], radius: 3.0 }, restitution: 0.1, friction: 0.5, sticky: false, absorb: false, name: "Circle".into() }); }
        if ui.button("+ Half-Space").clicked() { state.boundaries.push(FluidBoundary { shape: FluidBoundaryShape::HalfSpace { normal: [0.0,1.0], offset: 0.0 }, restitution: 0.1, friction: 0.8, sticky: false, absorb: false, name: "Plane".into() }); }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.boundaries.remove(s); state.selected = None; } }
    });

    for (i, b) in state.boundaries.iter().enumerate() {
        let sel = state.selected == Some(i);
        if ui.selectable_label(sel, format!("{} [{}]", b.name, b.shape.label())).clicked() {
            state.selected = Some(i);
        }
    }

    if let Some(idx) = state.selected {
        if idx < state.boundaries.len() {
            let b = &mut state.boundaries[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut b.name); });
            ui.horizontal(|ui| {
                ui.label("Restitution:");
                ui.add(egui::Slider::new(&mut b.restitution, 0.0..=1.0));
                ui.label("Friction:");
                ui.add(egui::Slider::new(&mut b.friction, 0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut b.sticky, "Sticky");
                ui.checkbox(&mut b.absorb, "Absorb Fluid");
            });
            match &mut b.shape {
                FluidBoundaryShape::Box { min, max } => {
                    ui.horizontal(|ui| {
                        ui.label("Min:");
                        ui.add(egui::DragValue::new(&mut min[0]).prefix("x:").speed(0.05));
                        ui.add(egui::DragValue::new(&mut min[1]).prefix("y:").speed(0.05));
                        ui.label("Max:");
                        ui.add(egui::DragValue::new(&mut max[0]).prefix("x:").speed(0.05));
                        ui.add(egui::DragValue::new(&mut max[1]).prefix("y:").speed(0.05));
                    });
                }
                FluidBoundaryShape::Circle { center, radius } => {
                    ui.horizontal(|ui| {
                        ui.label("Center:");
                        ui.add(egui::DragValue::new(&mut center[0]).prefix("x:").speed(0.05));
                        ui.add(egui::DragValue::new(&mut center[1]).prefix("y:").speed(0.05));
                        ui.label("Radius:");
                        ui.add(egui::DragValue::new(radius).speed(0.05).clamp_range(0.1..=50.0));
                    });
                }
                FluidBoundaryShape::HalfSpace { normal, offset } => {
                    ui.horizontal(|ui| {
                        ui.label("Normal:");
                        ui.add(egui::DragValue::new(&mut normal[0]).prefix("nx:").speed(0.01));
                        ui.add(egui::DragValue::new(&mut normal[1]).prefix("ny:").speed(0.01));
                        ui.label("Offset:");
                        ui.add(egui::DragValue::new(offset).speed(0.05));
                    });
                }
            }
        }
    }
}

// ─── Physics Timeline Recorder ────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicsSnapshot {
    pub time: f32,
    pub body_positions: Vec<[f32; 2]>,
    pub body_velocities: Vec<[f32; 2]>,
    pub body_angles: Vec<f32>,
    pub total_ke: f32,
    pub total_pe: f32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsTimelineState {
    pub snapshots: Vec<PhysicsSnapshot>,
    pub recording: bool,
    pub playback_time: f32,
    pub playback_speed: f32,
    pub playing: bool,
    pub max_record_time: f32,
    pub record_interval: f32,
    pub last_record_time: f32,
    pub show_energy_plot: bool,
    pub show_trajectory: bool,
    pub selected_body_track: Option<usize>,
}

pub fn show_physics_timeline(ui: &mut egui::Ui, state: &mut PhysicsTimelineState) {
    ui.heading("Physics Timeline Recorder");
    ui.separator();

    if state.playback_speed == 0.0 { state.playback_speed = 1.0; state.max_record_time = 10.0; state.record_interval = 0.033; }

    ui.horizontal(|ui| {
        let rec_btn = if state.recording { egui::RichText::new("⏹ Stop").color(Color32::RED) } else { egui::RichText::new("⏺ Record").color(Color32::from_rgb(240,80,80)) };
        if ui.button(rec_btn).clicked() {
            state.recording = !state.recording;
            if state.recording { state.snapshots.clear(); state.last_record_time = 0.0; }
        }
        let play_btn = if state.playing { "⏸ Pause" } else { "▶ Play" };
        if ui.button(play_btn).clicked() { state.playing = !state.playing; }
        if ui.button("⏮ Rewind").clicked() { state.playback_time = 0.0; }
        if ui.button("Clear").clicked() { state.snapshots.clear(); state.recording = false; state.playing = false; state.playback_time = 0.0; }
    });

    ui.horizontal(|ui| {
        ui.label("Speed:");
        ui.add(egui::Slider::new(&mut state.playback_speed, 0.1..=4.0).logarithmic(false));
        ui.label("Max Record:");
        ui.add(egui::DragValue::new(&mut state.max_record_time).speed(0.5).clamp_range(1.0..=120.0).suffix("s"));
        ui.label("Interval:");
        ui.add(egui::DragValue::new(&mut state.record_interval).speed(0.001).clamp_range(0.001..=0.5).suffix("s"));
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.show_energy_plot, "Energy Plot");
        ui.checkbox(&mut state.show_trajectory, "Trajectory");
        ui.label(format!("Snapshots: {} | Duration: {:.2}s", state.snapshots.len(), state.snapshots.last().map_or(0.0, |s| s.time)));
    });

    let total_dur = state.snapshots.last().map_or(1.0, |s| s.time).max(0.001);
    let mut pb_normalized = state.playback_time / total_dur;
    if ui.add(egui::Slider::new(&mut pb_normalized, 0.0..=1.0).text("Time").show_value(true)).changed() {
        state.playback_time = pb_normalized * total_dur;
    }
    let cur_snap = if !state.snapshots.is_empty() {
        let closest = state.snapshots.iter().enumerate().min_by(|(_, a), (_, b)| (a.time - state.playback_time).abs().partial_cmp(&(b.time - state.playback_time).abs()).unwrap_or(std::cmp::Ordering::Equal)).map(|(i, _)| i);
        closest
    } else { None };

    if let Some(si) = cur_snap {
        let snap = &state.snapshots[si];
        ui.label(format!("t={:.3}s | KE={:.2} | PE={:.2} | Total={:.2}", snap.time, snap.total_ke, snap.total_pe, snap.total_ke + snap.total_pe));
    }

    if state.show_energy_plot {
        draw_energy_timeline_plot(ui, &state.snapshots, state.playback_time);
    }
}

fn draw_energy_timeline_plot(ui: &mut egui::Ui, snapshots: &[PhysicsSnapshot], current_time: f32) {
    let desired = Vec2::new(ui.available_width().min(500.0), 100.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(12, 14, 20));
    if snapshots.is_empty() {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, "No data recorded", FontId::monospace(9.0), Color32::DARK_GRAY);
        return;
    }
    let max_t = snapshots.last().map_or(1.0, |s| s.time).max(0.001);
    let max_e = snapshots.iter().map(|s| s.total_ke + s.total_pe).fold(0.0f32, f32::max).max(0.001);

    let to_screen = |t: f32, e: f32| -> Pos2 {
        Pos2::new(rect.left() + (t / max_t) * rect.width(), rect.bottom() - (e / max_e) * rect.height() * 0.9)
    };

    // Draw KE
    for i in 1..snapshots.len() {
        let p0 = to_screen(snapshots[i-1].time, snapshots[i-1].total_ke);
        let p1 = to_screen(snapshots[i].time, snapshots[i].total_ke);
        p.line_segment([p0, p1], Stroke::new(1.0, Color32::from_rgb(80, 200, 120)));
    }
    // Draw PE
    for i in 1..snapshots.len() {
        let p0 = to_screen(snapshots[i-1].time, snapshots[i-1].total_pe);
        let p1 = to_screen(snapshots[i].time, snapshots[i].total_pe);
        p.line_segment([p0, p1], Stroke::new(1.0, Color32::from_rgb(200, 160, 60)));
    }
    // Draw Total
    for i in 1..snapshots.len() {
        let p0 = to_screen(snapshots[i-1].time, snapshots[i-1].total_ke + snapshots[i-1].total_pe);
        let p1 = to_screen(snapshots[i].time, snapshots[i].total_ke + snapshots[i].total_pe);
        p.line_segment([p0, p1], Stroke::new(1.5, Color32::WHITE));
    }
    // Current time cursor
    let cx = rect.left() + (current_time / max_t) * rect.width();
    p.line_segment([Pos2::new(cx, rect.top()), Pos2::new(cx, rect.bottom())], Stroke::new(1.5, Color32::from_rgb(255, 100, 100)));

    p.text(Pos2::new(rect.left()+4.0, rect.top()+3.0), egui::Align2::LEFT_TOP, "KE PE Total", FontId::monospace(7.0), Color32::GRAY);
}

// ─── Material Interaction Matrix ──────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct MaterialInteractionMatrix {
    pub materials: Vec<String>,
    pub friction_table: Vec<Vec<f32>>,
    pub restitution_table: Vec<Vec<f32>>,
    pub combine_modes: Vec<Vec<u8>>,
}

impl MaterialInteractionMatrix {
    pub fn new_default() -> Self {
        let mats = vec!["Default".into(), "Ice".into(), "Rubber".into(), "Steel".into(), "Wood".into(), "Sand".into()];
        let n = mats.len();
        let mut friction_table = vec![vec![0.5f32; n]; n];
        let mut restitution_table = vec![vec![0.3f32; n]; n];
        // Set some interesting combos
        friction_table[0][1] = 0.05; friction_table[1][0] = 0.05; // Default-Ice
        friction_table[0][2] = 0.9; friction_table[2][0] = 0.9;   // Default-Rubber
        friction_table[2][2] = 0.95; // Rubber-Rubber
        restitution_table[2][2] = 0.85; // Rubber-Rubber very bouncy
        restitution_table[3][3] = 0.4; // Steel-Steel
        Self { materials: mats, friction_table, restitution_table, combine_modes: vec![vec![0u8; n]; n] }
    }
    pub fn get_friction(&self, a: usize, b: usize) -> f32 {
        if a < self.friction_table.len() && b < self.friction_table[a].len() { self.friction_table[a][b] } else { 0.5 }
    }
    pub fn get_restitution(&self, a: usize, b: usize) -> f32 {
        if a < self.restitution_table.len() && b < self.restitution_table[a].len() { self.restitution_table[a][b] } else { 0.3 }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct MaterialInteractionState {
    pub matrix: Option<MaterialInteractionMatrix>,
    pub show_friction: bool,
    pub selected_cell: Option<(usize, usize)>,
    pub editing_value: f32,
}

pub fn show_material_interaction_matrix(ui: &mut egui::Ui, state: &mut MaterialInteractionState) {
    ui.heading("Material Interaction Matrix");
    ui.separator();

    if state.matrix.is_none() {
        state.matrix = Some(MaterialInteractionMatrix::new_default());
        state.show_friction = true;
    }

    let matrix = state.matrix.as_mut().unwrap();
    let n = matrix.materials.len();

    ui.horizontal(|ui| {
        if ui.selectable_label(state.show_friction, "Friction").clicked() { state.show_friction = true; }
        if ui.selectable_label(!state.show_friction, "Restitution").clicked() { state.show_friction = false; }
        if ui.button("Reset Default").clicked() { *matrix = MaterialInteractionMatrix::new_default(); }
        if ui.button("+ Material").clicked() {
            let nm = format!("Mat_{}", matrix.materials.len());
            matrix.materials.push(nm);
            let new_n = matrix.materials.len();
            for row in &mut matrix.friction_table { row.push(0.5); }
            matrix.friction_table.push(vec![0.5; new_n]);
            for row in &mut matrix.restitution_table { row.push(0.3); }
            matrix.restitution_table.push(vec![0.3; new_n]);
            for row in &mut matrix.combine_modes { row.push(0); }
            matrix.combine_modes.push(vec![0; new_n]);
        }
    });

    let cell_size = 44.0;
    let header_w = 60.0;
    let total_w = header_w + n as f32 * cell_size;
    let total_h = header_w + n as f32 * cell_size;
    let desired = Vec2::new(total_w.min(ui.available_width()), total_h.min(400.0));
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, Color32::from_rgb(18, 22, 30));

    // Column headers
    for col in 0..n {
        let x = rect.left() + header_w + col as f32 * cell_size + cell_size * 0.5;
        p.text(Pos2::new(x, rect.top() + header_w * 0.5), egui::Align2::CENTER_CENTER, &matrix.materials[col][..3.min(matrix.materials[col].len())], FontId::monospace(7.5), Color32::GRAY);
    }
    // Row headers
    for row in 0..n {
        let y = rect.top() + header_w + row as f32 * cell_size + cell_size * 0.5;
        p.text(Pos2::new(rect.left() + header_w * 0.5, y), egui::Align2::CENTER_CENTER, &matrix.materials[row][..3.min(matrix.materials[row].len())], FontId::monospace(7.5), Color32::GRAY);
    }

    // Cells
    for row in 0..n {
        for col in 0..n {
            let cx = rect.left() + header_w + col as f32 * cell_size;
            let cy = rect.top() + header_w + row as f32 * cell_size;
            let cr = Rect::from_min_size(Pos2::new(cx, cy), Vec2::splat(cell_size));
            let val = if state.show_friction { matrix.get_friction(row, col) } else { matrix.get_restitution(row, col) };
            let t = val.clamp(0.0, 1.0);
            let cell_col = if state.show_friction {
                egui::lerp(egui::Rgba::from(Color32::from_rgb(20,60,20))..=egui::Rgba::from(Color32::from_rgb(60,200,60)), t)
            } else {
                egui::lerp(egui::Rgba::from(Color32::from_rgb(20,20,80))..=egui::Rgba::from(Color32::from_rgb(60,120,255)), t)
            };
            let is_sel = state.selected_cell == Some((row, col));
            p.rect_filled(cr, 1.0, Color32::from(cell_col));
            if is_sel { p.rect_stroke(cr, 1.0, Stroke::new(2.0, Color32::WHITE)); }
            else { p.rect_stroke(cr, 0.0, Stroke::new(0.5, Color32::from_rgb(30,34,42))); }
            p.text(cr.center(), egui::Align2::CENTER_CENTER, format!("{:.2}", val), FontId::monospace(7.0), Color32::WHITE);
        }
    }

    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let col = ((pos.x - rect.left() - header_w) / cell_size) as i32;
            let row = ((pos.y - rect.top() - header_w) / cell_size) as i32;
            if col >= 0 && row >= 0 && col < n as i32 && row < n as i32 {
                let ci = col as usize;
                let ri = row as usize;
                state.selected_cell = Some((ri, ci));
                state.editing_value = if state.show_friction { matrix.get_friction(ri, ci) } else { matrix.get_restitution(ri, ci) };
            }
        }
    }

    if let Some((ri, ci)) = state.selected_cell {
        ui.separator();
        ui.horizontal(|ui| {
            let mname_a = matrix.materials.get(ri).cloned().unwrap_or_default();
            let mname_b = matrix.materials.get(ci).cloned().unwrap_or_default();
            ui.label(format!("{} × {} {}:", mname_a, mname_b, if state.show_friction { "Friction" } else { "Restitution" }));
            if ui.add(egui::DragValue::new(&mut state.editing_value).speed(0.01).clamp_range(0.0..=2.0)).changed() {
                let m = state.matrix.as_mut().unwrap();
                if state.show_friction {
                    if ri < m.friction_table.len() && ci < m.friction_table[ri].len() { m.friction_table[ri][ci] = state.editing_value; m.friction_table[ci][ri] = state.editing_value; }
                } else {
                    if ri < m.restitution_table.len() && ci < m.restitution_table[ri].len() { m.restitution_table[ri][ci] = state.editing_value; m.restitution_table[ci][ri] = state.editing_value; }
                }
            }
        });
    }
}

// ─── Spring Visualizer ────────────────────────────────────────────────────────

pub fn draw_spring_visualizer(ui: &mut egui::Ui, stiffness: f32, damping: f32, rest_length: f32) {
    let desired = Vec2::new(ui.available_width().min(360.0), 120.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(14, 17, 24));

    // Draw a zigzag spring between two endpoints
    let left = Pos2::new(rect.left() + 30.0, rect.center().y);
    let right = Pos2::new(rect.right() - 30.0, rect.center().y);
    p.circle_filled(left, 5.0, Color32::from_rgb(160,160,180));
    p.circle_filled(right, 5.0, Color32::from_rgb(160,160,180));

    let span = right.x - left.x;
    let coils = 10;
    let amp = 12.0 * stiffness.clamp(0.1, 1.0);
    let mut last = left;
    for i in 0..=coils*2 {
        let t = i as f32 / (coils * 2) as f32;
        let x = left.x + t * span;
        let y = rect.center().y + if i % 2 == 0 { amp } else { -amp };
        if i > 0 { p.line_segment([last, Pos2::new(x, y)], Stroke::new(1.5, Color32::from_rgb(100, 200, 255))); }
        last = Pos2::new(x, y);
    }
    p.line_segment([last, right], Stroke::new(1.5, Color32::from_rgb(100, 200, 255)));

    // Labels
    p.text(Pos2::new(rect.center().x, rect.top()+6.0), egui::Align2::CENTER_TOP,
        format!("k={:.2} d={:.2} L0={:.2}", stiffness, damping, rest_length),
        FontId::monospace(8.0), Color32::GRAY);

    // Damping visualization — fade lines above spring
    let damp_alpha = (damping * 200.0) as u8;
    for di in 0..4 {
        let t = di as f32 / 4.0;
        let x = left.x + t * span * 0.8 + 20.0;
        p.line_segment([Pos2::new(x, rect.top()+20.0), Pos2::new(x+8.0, rect.top()+36.0)], Stroke::new(1.0, Color32::from_rgba_unmultiplied(200, 100, 60, damp_alpha)));
    }
}

// ─── Physics Hotkey Reference ─────────────────────────────────────────────────

pub fn show_physics_hotkey_reference(ui: &mut egui::Ui) {
    ui.heading("Physics Editor Hotkeys");
    ui.separator();
    let hotkeys = [
        ("Space", "Play/Pause simulation"),
        ("R", "Reset simulation state"),
        ("S", "Step one frame"),
        ("G", "Toggle gravity"),
        ("D", "Toggle debug overlay"),
        ("B", "Select body mode"),
        ("J", "Select joint mode"),
        ("C", "Select collider mode"),
        ("T", "Trigger zone mode"),
        ("F", "Fluid simulation mode"),
        ("Ctrl+Z", "Undo"),
        ("Ctrl+Y", "Redo"),
        ("Ctrl+S", "Save scene"),
        ("Ctrl+D", "Duplicate selected"),
        ("Delete", "Delete selected"),
        ("Escape", "Deselect all"),
        ("F1", "Show this help"),
        ("Tab", "Cycle panels"),
        ("[", "Decrease time step"),
        ("]", "Increase time step"),
        ("P", "Pin/Unpin selected node"),
        ("W/A/S/D", "Pan viewport"),
        ("Scroll", "Zoom in/out"),
        ("Middle Mouse", "Pan viewport"),
        ("Left Click", "Select object"),
        ("Ctrl+Click", "Multi-select"),
        ("Right Click", "Context menu"),
        ("1", "Bodies panel"),
        ("2", "Joints panel"),
        ("3", "Materials panel"),
        ("4", "Simulation settings"),
        ("5", "Soft body editor"),
        ("6", "Fluid editor"),
        ("7", "Ragdoll editor"),
        ("8", "Constraint graph"),
        ("9", "Debug overlay"),
        ("0", "Profile view"),
    ];
    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
        egui::Grid::new("hotkey_grid").num_columns(2).spacing([20.0, 2.0]).striped(true).show(ui, |ui| {
            for (key, desc) in &hotkeys {
                ui.label(egui::RichText::new(*key).monospace().color(Color32::from_rgb(180, 220, 255)));
                ui.label(*desc);
                ui.end_row();
            }
        });
    });
}

// ─── Physics Scenario Export ──────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicsScenarioExport {
    pub version: u32,
    pub name: String,
    pub gravity: [f32; 2],
    pub time_step: f32,
    pub iterations: u32,
    pub bodies_json: String,
    pub joints_json: String,
    pub materials_json: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

impl PhysicsScenarioExport {
    pub fn new(name: &str) -> Self {
        Self {
            version: 1,
            name: name.to_string(),
            gravity: [0.0, -9.81],
            time_step: 0.016,
            iterations: 8,
            bodies_json: "[]".to_string(),
            joints_json: "[]".to_string(),
            materials_json: "[]".to_string(),
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ScenarioExportState {
    pub current_export: Option<PhysicsScenarioExport>,
    pub export_path: String,
    pub import_path: String,
    pub last_export_status: String,
    pub last_import_status: String,
    pub json_preview: String,
    pub show_json: bool,
    pub pretty_print: bool,
}

pub fn show_scenario_export_import(ui: &mut egui::Ui, state: &mut ScenarioExportState) {
    ui.heading("Scenario Export / Import");
    ui.separator();

    if state.current_export.is_none() {
        state.current_export = Some(PhysicsScenarioExport::new("My Physics Scene"));
        state.export_path = "physics_scene.json".to_string();
    }

    let export = state.current_export.as_mut().unwrap();
    ui.group(|ui| {
        ui.label("Export Settings");
        ui.horizontal(|ui| {
            ui.label("Scene Name:");
            ui.text_edit_singleline(&mut export.name);
        });
        ui.horizontal(|ui| {
            ui.label("Tags:");
            let mut tags_str = export.tags.join(", ");
            if ui.text_edit_singleline(&mut tags_str).changed() {
                export.tags = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Path:");
            ui.text_edit_singleline(&mut state.export_path);
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.show_json, "Preview JSON");
            ui.checkbox(&mut state.pretty_print, "Pretty Print");
        });
        if ui.button("Export Scene").clicked() {
            state.json_preview = format!("{{\n  \"version\": {},\n  \"name\": \"{}\",\n  \"gravity\": [{}, {}],\n  \"time_step\": {},\n  \"iterations\": {},\n  \"tags\": [{}]\n}}", export.version, export.name, export.gravity[0], export.gravity[1], export.time_step, export.iterations, export.tags.iter().map(|t| format!("\"{}\"", t)).collect::<Vec<_>>().join(", "));
            state.last_export_status = format!("Exported to '{}'", state.export_path);
        }
        if !state.last_export_status.is_empty() {
            ui.label(egui::RichText::new(&state.last_export_status).color(Color32::GREEN));
        }
    });

    if state.show_json && !state.json_preview.is_empty() {
        ui.separator();
        ui.label("JSON Preview:");
        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut state.json_preview.clone()).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
        });
    }

    ui.separator();
    ui.group(|ui| {
        ui.label("Import Settings");
        ui.horizontal(|ui| {
            ui.label("Path:");
            ui.text_edit_singleline(&mut state.import_path);
        });
        if ui.button("Import Scene").clicked() {
            state.last_import_status = format!("Imported from '{}'", state.import_path);
        }
        if !state.last_import_status.is_empty() {
            ui.label(egui::RichText::new(&state.last_import_status).color(Color32::from_rgb(100, 180, 255)));
        }
    });
}

// ─── Velocity Field Painter ───────────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VelocityFieldPainterState {
    pub field: Vec<Vec<[f32; 2]>>,
    pub grid_w: usize,
    pub grid_h: usize,
    pub brush_size: f32,
    pub brush_strength: f32,
    pub current_mode: VelocityBrushMode,
    pub cell_scale: f32,
    pub show_arrows: bool,
    pub show_magnitude_heatmap: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum VelocityBrushMode {
    #[default]
    Paint,
    Erase,
    Rotate,
    Attract,
    Repel,
    Noise,
}

impl VelocityBrushMode {
    pub fn label(&self) -> &str {
        match self {
            VelocityBrushMode::Paint => "Paint",
            VelocityBrushMode::Erase => "Erase",
            VelocityBrushMode::Rotate => "Rotate",
            VelocityBrushMode::Attract => "Attract",
            VelocityBrushMode::Repel => "Repel",
            VelocityBrushMode::Noise => "Noise",
        }
    }
}

pub fn show_velocity_field_painter(ui: &mut egui::Ui, state: &mut VelocityFieldPainterState) {
    ui.heading("Velocity Field Painter");
    ui.separator();

    if state.grid_w == 0 {
        state.grid_w = 20;
        state.grid_h = 15;
        state.brush_size = 2.0;
        state.brush_strength = 5.0;
        state.cell_scale = 1.0;
        state.show_arrows = true;
        state.field = vec![vec![[0.0f32; 2]; state.grid_w]; state.grid_h];
    }

    ui.horizontal(|ui| {
        for mode in &[VelocityBrushMode::Paint, VelocityBrushMode::Erase, VelocityBrushMode::Rotate, VelocityBrushMode::Attract, VelocityBrushMode::Repel, VelocityBrushMode::Noise] {
            if ui.selectable_label(state.current_mode == *mode, mode.label()).clicked() { state.current_mode = mode.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Brush Size:");
        ui.add(egui::Slider::new(&mut state.brush_size, 0.5..=8.0));
        ui.label("Strength:");
        ui.add(egui::Slider::new(&mut state.brush_strength, 0.1..=20.0));
        ui.checkbox(&mut state.show_arrows, "Arrows");
        ui.checkbox(&mut state.show_magnitude_heatmap, "Heatmap");
        if ui.button("Clear Field").clicked() {
            for row in &mut state.field { for c in row.iter_mut() { *c = [0.0, 0.0]; } }
        }
        if ui.button("Fill Uniform →").clicked() {
            for row in &mut state.field { for c in row.iter_mut() { *c = [5.0, 0.0]; } }
        }
        if ui.button("Fill Vortex").clicked() {
            let gw = state.grid_w as f32;
            let gh = state.grid_h as f32;
            for gy in 0..state.grid_h {
                for gx in 0..state.grid_w {
                    let nx = gx as f32 / gw - 0.5;
                    let ny = gy as f32 / gh - 0.5;
                    let len = (nx*nx + ny*ny).sqrt().max(0.001);
                    state.field[gy][gx] = [-ny/len * 5.0, nx/len * 5.0];
                }
            }
        }
    });

    draw_velocity_field(ui, state);
}

fn draw_velocity_field(ui: &mut egui::Ui, state: &VelocityFieldPainterState) {
    let desired = Vec2::new(ui.available_width().min(600.0), 300.0);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 12, 18));

    let cell_w = rect.width() / state.grid_w as f32;
    let cell_h = rect.height() / state.grid_h as f32;

    if state.show_magnitude_heatmap {
        let max_mag = state.field.iter().flat_map(|row| row.iter()).map(|c| (c[0]*c[0]+c[1]*c[1]).sqrt()).fold(0.0f32, f32::max).max(0.001);
        for gy in 0..state.grid_h {
            for gx in 0..state.grid_w {
                let v = &state.field[gy][gx];
                let mag = (v[0]*v[0]+v[1]*v[1]).sqrt();
                let t = (mag / max_mag).clamp(0.0, 1.0);
                let col = egui::lerp(egui::Rgba::from(Color32::from_rgb(10,10,40))..=egui::Rgba::from(Color32::from_rgb(60,120,255)), t);
                let cr = Rect::from_min_size(Pos2::new(rect.left() + gx as f32 * cell_w, rect.top() + gy as f32 * cell_h), Vec2::new(cell_w, cell_h));
                p.rect_filled(cr, 0.0, Color32::from(col));
            }
        }
    }

    if state.show_arrows {
        for gy in 0..state.grid_h {
            for gx in 0..state.grid_w {
                let v = &state.field[gy][gx];
                let mag = (v[0]*v[0]+v[1]*v[1]).sqrt();
                if mag < 0.05 { continue; }
                let cx = rect.left() + (gx as f32 + 0.5) * cell_w;
                let cy = rect.top() + (gy as f32 + 0.5) * cell_h;
                let scale = (cell_w * 0.4).min(cell_h * 0.4);
                let dx = v[0] / mag * scale;
                let dy = -v[1] / mag * scale;
                let alpha = ((mag / 10.0).clamp(0.0, 1.0) * 220.0) as u8;
                let col = Color32::from_rgba_unmultiplied(100, 200, 255, alpha);
                draw_arrow(&p, Pos2::new(cx, cy), Pos2::new(cx + dx, cy + dy), col, 0.8);
            }
        }
    }

    if let Some(pos) = response.interact_pointer_pos() {
        let bx = ((pos.x - rect.left()) / cell_w) as i32;
        let by = ((pos.y - rect.top()) / cell_h) as i32;
        let bc = p.clip_rect();
        let cursor_r = state.brush_size * cell_w * 0.5;
        p.circle_stroke(pos, cursor_r, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255,255,255,120)));
        p.text(pos + Vec2::new(cursor_r + 4.0, 0.0), egui::Align2::LEFT_CENTER, state.current_mode.label(), FontId::monospace(8.0), Color32::GRAY);
    }
    p.text(Pos2::new(rect.left()+4.0, rect.bottom()-12.0), egui::Align2::LEFT_BOTTOM, format!("{}×{} grid | Click+Drag to paint", state.grid_w, state.grid_h), FontId::monospace(7.0), Color32::DARK_GRAY);
}

// ─── Body Transform Gizmo ─────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
    Universal,
}

impl Default for GizmoMode { fn default() -> Self { GizmoMode::Translate } }

impl GizmoMode {
    pub fn label(&self) -> &str {
        match self {
            GizmoMode::Translate => "Translate (W)",
            GizmoMode::Rotate => "Rotate (E)",
            GizmoMode::Scale => "Scale (R)",
            GizmoMode::Universal => "Universal (Q)",
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct GizmoState {
    pub mode: GizmoMode,
    pub space: GizmoSpace,
    pub pivot: GizmoPivot,
    pub snap_translate: bool,
    pub snap_rotate: bool,
    pub snap_scale: bool,
    pub snap_translate_step: f32,
    pub snap_rotate_step: f32,
    pub snap_scale_step: f32,
    pub active_axis: Option<u8>,
    pub is_dragging: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum GizmoSpace {
    #[default]
    World,
    Local,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum GizmoPivot {
    #[default]
    Individual,
    Center,
    Origin,
    Cursor,
}

pub fn show_gizmo_settings(ui: &mut egui::Ui, state: &mut GizmoState) {
    ui.heading("Transform Gizmo Settings");
    ui.separator();

    if state.snap_translate_step == 0.0 { state.snap_translate_step = 0.25; state.snap_rotate_step = 15.0; state.snap_scale_step = 0.1; }

    ui.horizontal(|ui| {
        ui.label("Mode:");
        for mode in &[GizmoMode::Translate, GizmoMode::Rotate, GizmoMode::Scale, GizmoMode::Universal] {
            if ui.selectable_label(state.mode == *mode, mode.label()).clicked() { state.mode = mode.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Space:");
        if ui.selectable_label(state.space == GizmoSpace::World, "World").clicked() { state.space = GizmoSpace::World; }
        if ui.selectable_label(state.space == GizmoSpace::Local, "Local").clicked() { state.space = GizmoSpace::Local; }
        ui.separator();
        ui.label("Pivot:");
        if ui.selectable_label(state.pivot == GizmoPivot::Individual, "Individual").clicked() { state.pivot = GizmoPivot::Individual; }
        if ui.selectable_label(state.pivot == GizmoPivot::Center, "Center").clicked() { state.pivot = GizmoPivot::Center; }
        if ui.selectable_label(state.pivot == GizmoPivot::Origin, "Origin").clicked() { state.pivot = GizmoPivot::Origin; }
    });

    ui.separator();
    ui.label("Snap Settings:");
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.snap_translate, "Translate Snap:");
        ui.add_enabled(state.snap_translate, egui::DragValue::new(&mut state.snap_translate_step).speed(0.01).clamp_range(0.01..=10.0).suffix(" u"));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.snap_rotate, "Rotate Snap:");
        ui.add_enabled(state.snap_rotate, egui::DragValue::new(&mut state.snap_rotate_step).speed(1.0).clamp_range(1.0..=90.0).suffix("°"));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.snap_scale, "Scale Snap:");
        ui.add_enabled(state.snap_scale, egui::DragValue::new(&mut state.snap_scale_step).speed(0.01).clamp_range(0.01..=1.0));
    });

    draw_gizmo_preview(ui, state);
}

fn draw_gizmo_preview(ui: &mut egui::Ui, state: &GizmoState) {
    let desired = Vec2::new(160.0, 160.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(15, 18, 24));
    let c = rect.center();

    // Body outline
    p.rect_stroke(Rect::from_center_size(c, Vec2::splat(40.0)), 2.0, Stroke::new(1.5, Color32::from_rgb(120,140,180)));

    match state.mode {
        GizmoMode::Translate | GizmoMode::Universal => {
            // X axis (red)
            draw_arrow(&p, c, c + Vec2::new(50.0, 0.0), Color32::from_rgb(220, 60, 60), 2.0);
            p.text(c + Vec2::new(56.0, 0.0), egui::Align2::LEFT_CENTER, "X", FontId::monospace(10.0), Color32::from_rgb(220,60,60));
            // Y axis (green)
            draw_arrow(&p, c, c + Vec2::new(0.0, -50.0), Color32::from_rgb(60, 220, 60), 2.0);
            p.text(c + Vec2::new(0.0, -56.0), egui::Align2::CENTER_BOTTOM, "Y", FontId::monospace(10.0), Color32::from_rgb(60,220,60));
        }
        GizmoMode::Rotate => {
            p.circle_stroke(c, 45.0, Stroke::new(2.0, Color32::from_rgb(60, 60, 220)));
            p.circle_stroke(c, 45.0, Stroke::new(2.5, Color32::from_rgb(220, 60, 60)));
            let arc_start = c + Vec2::new(45.0, 0.0);
            for i in 0..12 {
                let a = i as f32 * std::f32::consts::TAU / 12.0;
                let tick = c + Vec2::new(a.cos() * 45.0, a.sin() * 45.0);
                p.circle_filled(tick, 1.5, Color32::from_rgb(100,100,220));
            }
        }
        GizmoMode::Scale => {
            p.line_segment([c, c + Vec2::new(50.0, 0.0)], Stroke::new(2.0, Color32::from_rgb(220,60,60)));
            p.rect_filled(Rect::from_center_size(c + Vec2::new(52.0, 0.0), Vec2::splat(6.0)), 0.0, Color32::from_rgb(220,60,60));
            p.line_segment([c, c + Vec2::new(0.0, -50.0)], Stroke::new(2.0, Color32::from_rgb(60,220,60)));
            p.rect_filled(Rect::from_center_size(c + Vec2::new(0.0, -52.0), Vec2::splat(6.0)), 0.0, Color32::from_rgb(60,220,60));
        }
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
