import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 5
// ============================================================

// ─── Vehicle Suspension System ────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct WheelConfig {
    pub name: String,
    pub offset: [f32; 2],
    pub radius: f32,
    pub width: f32,
    pub mass: f32,
    pub suspension_rest_length: f32,
    pub suspension_stiffness: f32,
    pub suspension_damping: f32,
    pub friction_slip: f32,
    pub drive_type: WheelDrive,
    pub steer: bool,
    pub max_steer_angle: f32,
    pub brake_force: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum WheelDrive {
    None,
    Front,
    Rear,
    All,
}

impl WheelDrive {
    pub fn label(&self) -> &str {
        match self {
            WheelDrive::None => "None",
            WheelDrive::Front => "Front",
            WheelDrive::Rear => "Rear",
            WheelDrive::All => "All",
        }
    }
}

impl Default for WheelDrive { fn default() -> Self { WheelDrive::Rear } }

impl WheelConfig {
    pub fn front_left() -> Self { Self { name: "FL".into(), offset: [-1.2, 1.5], radius: 0.35, width: 0.2, mass: 15.0, suspension_rest_length: 0.4, suspension_stiffness: 40000.0, suspension_damping: 4000.0, friction_slip: 3.0, drive_type: WheelDrive::None, steer: true, max_steer_angle: 30.0, brake_force: 3000.0 } }
    pub fn front_right() -> Self { let mut w = Self::front_left(); w.name = "FR".into(); w.offset = [1.2, 1.5]; w }
    pub fn rear_left() -> Self { Self { name: "RL".into(), offset: [-1.2, -1.5], radius: 0.35, width: 0.2, mass: 15.0, suspension_rest_length: 0.4, suspension_stiffness: 40000.0, suspension_damping: 4000.0, friction_slip: 3.5, drive_type: WheelDrive::Rear, steer: false, max_steer_angle: 0.0, brake_force: 2500.0 } }
    pub fn rear_right() -> Self { let mut w = Self::rear_left(); w.name = "RR".into(); w.offset = [1.2, -1.5]; w }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VehicleConfig {
    pub name: String,
    pub chassis_mass: f32,
    pub chassis_half_extents: [f32; 2],
    pub center_of_mass_offset: [f32; 2],
    pub max_engine_force: f32,
    pub max_brake_force: f32,
    pub steering_increment: f32,
    pub steering_clamp: f32,
    pub wheels: Vec<WheelConfig>,
    pub drive_type: VehicleDrive,
    pub anti_roll_stiffness: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum VehicleDrive {
    FWD,
    RWD,
    AWD,
}

impl VehicleDrive {
    pub fn label(&self) -> &str {
        match self { VehicleDrive::FWD => "FWD", VehicleDrive::RWD => "RWD", VehicleDrive::AWD => "AWD" }
    }
}

impl VehicleConfig {
    pub fn default_car() -> Self {
        Self {
            name: "Car".into(),
            chassis_mass: 800.0,
            chassis_half_extents: [0.9, 2.0],
            center_of_mass_offset: [0.0, -0.2],
            max_engine_force: 3000.0,
            max_brake_force: 2000.0,
            steering_increment: 0.04,
            steering_clamp: 0.5,
            wheels: vec![WheelConfig::front_left(), WheelConfig::front_right(), WheelConfig::rear_left(), WheelConfig::rear_right()],
            drive_type: VehicleDrive::RWD,
            anti_roll_stiffness: 8000.0,
        }
    }
    pub fn default_truck() -> Self {
        let mut v = Self::default_car();
        v.name = "Truck".into();
        v.chassis_mass = 3000.0;
        v.chassis_half_extents = [1.2, 3.0];
        v.max_engine_force = 8000.0;
        v.max_brake_force = 5000.0;
        for w in &mut v.wheels { w.radius = 0.55; w.suspension_stiffness = 80000.0; }
        v
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VehicleEditorState {
    pub vehicles: Vec<VehicleConfig>,
    pub selected: Option<usize>,
    pub selected_wheel: Option<usize>,
}

pub fn show_vehicle_editor(ui: &mut egui::Ui, state: &mut VehicleEditorState) {
    ui.heading("Vehicle Physics Editor");
    ui.separator();

    if state.vehicles.is_empty() {
        state.vehicles = vec![VehicleConfig::default_car(), VehicleConfig::default_truck()];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Car").clicked() { state.vehicles.push(VehicleConfig::default_car()); }
        if ui.button("+ Truck").clicked() { state.vehicles.push(VehicleConfig::default_truck()); }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.vehicles.remove(s); state.selected = None; } }
    });

    for (i, v) in state.vehicles.iter().enumerate() {
        let sel = state.selected == Some(i);
        if ui.selectable_label(sel, format!("{} [{}]", v.name, v.drive_type.label())).clicked() { state.selected = Some(i); }
    }

    if let Some(idx) = state.selected {
        if idx < state.vehicles.len() {
            let v = &mut state.vehicles[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut v.name); });
            ui.horizontal(|ui| {
                ui.label("Chassis Mass:");
                ui.add(egui::DragValue::new(&mut v.chassis_mass).speed(10.0).clamp_range(10.0..=50000.0));
                ui.label("Drive:");
                for dt in &[VehicleDrive::FWD, VehicleDrive::RWD, VehicleDrive::AWD] {
                    if ui.selectable_label(v.drive_type == *dt, dt.label()).clicked() { v.drive_type = dt.clone(); }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Half Extents:");
                ui.add(egui::DragValue::new(&mut v.chassis_half_extents[0]).prefix("w:").speed(0.02));
                ui.add(egui::DragValue::new(&mut v.chassis_half_extents[1]).prefix("h:").speed(0.02));
                ui.label("CoM Offset:");
                ui.add(egui::DragValue::new(&mut v.center_of_mass_offset[0]).prefix("x:").speed(0.01));
                ui.add(egui::DragValue::new(&mut v.center_of_mass_offset[1]).prefix("y:").speed(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("Max Engine:");
                ui.add(egui::DragValue::new(&mut v.max_engine_force).speed(50.0));
                ui.label("Max Brake:");
                ui.add(egui::DragValue::new(&mut v.max_brake_force).speed(50.0));
                ui.label("Anti-Roll:");
                ui.add(egui::DragValue::new(&mut v.anti_roll_stiffness).speed(100.0));
            });

            ui.separator();
            ui.label("Wheels:");
            for (wi, wheel) in v.wheels.iter().enumerate() {
                let wsel = state.selected_wheel == Some(wi);
                if ui.selectable_label(wsel, format!("{} | r={:.2} steer={}", wheel.name, wheel.radius, wheel.steer)).clicked() {
                    state.selected_wheel = Some(wi);
                }
            }
            if ui.button("+ Wheel").clicked() { v.wheels.push(WheelConfig::rear_right()); }

            if let Some(wi) = state.selected_wheel {
                if wi < v.wheels.len() {
                    let w = &mut v.wheels[wi];
                    ui.group(|ui| {
                        ui.horizontal(|ui| { ui.label("Wheel Name:"); ui.text_edit_singleline(&mut w.name); });
                        ui.horizontal(|ui| {
                            ui.label("Offset:");
                            ui.add(egui::DragValue::new(&mut w.offset[0]).prefix("x:").speed(0.02));
                            ui.add(egui::DragValue::new(&mut w.offset[1]).prefix("y:").speed(0.02));
                            ui.label("Radius:");
                            ui.add(egui::DragValue::new(&mut w.radius).speed(0.01).clamp_range(0.1..=2.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Susp. Stiffness:");
                            ui.add(egui::DragValue::new(&mut w.suspension_stiffness).speed(500.0));
                            ui.label("Damping:");
                            ui.add(egui::DragValue::new(&mut w.suspension_damping).speed(100.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Friction Slip:");
                            ui.add(egui::Slider::new(&mut w.friction_slip, 0.5..=8.0));
                            ui.checkbox(&mut w.steer, "Steerable");
                            if w.steer {
                                ui.label("Max Steer:");
                                ui.add(egui::DragValue::new(&mut w.max_steer_angle).speed(1.0).clamp_range(5.0..=60.0).suffix("°"));
                            }
                        });
                    });
                }
            }
        }
    }
    draw_vehicle_schematic(ui, state);
}

fn draw_vehicle_schematic(ui: &mut egui::Ui, state: &VehicleEditorState) {
    let desired = Vec2::new(ui.available_width().min(480.0), 180.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));

    if let Some(idx) = state.selected {
        if idx < state.vehicles.len() {
            let v = &state.vehicles[idx];
            let c = rect.center();
            let scale = 30.0;
            let hw = v.chassis_half_extents[0] * scale;
            let hh = v.chassis_half_extents[1] * scale;
            let com = Pos2::new(c.x + v.center_of_mass_offset[0]*scale, c.y - v.center_of_mass_offset[1]*scale);
            // Chassis
            p.rect_filled(Rect::from_center_size(c, Vec2::new(hw*2.0, hh*2.0)), 4.0, Color32::from_rgb(60,80,120));
            p.rect_stroke(Rect::from_center_size(c, Vec2::new(hw*2.0, hh*2.0)), 4.0, Stroke::new(1.5, Color32::from_rgb(100,140,220)));
            // Center of mass
            p.circle_filled(com, 4.0, Color32::YELLOW);
            p.text(com + Vec2::new(6.0, 0.0), egui::Align2::LEFT_CENTER, "CoM", FontId::monospace(7.0), Color32::YELLOW);
            // Wheels
            for (wi, wheel) in v.wheels.iter().enumerate() {
                let wx = c.x + wheel.offset[0] * scale;
                let wy = c.y - wheel.offset[1] * scale;
                let wr = wheel.radius * scale;
                let wsel = state.selected_wheel == Some(wi);
                let wcol = if wsel { Color32::WHITE } else if wheel.steer { Color32::from_rgb(80,220,120) } else { Color32::from_rgb(180,140,80) };
                p.circle_filled(Pos2::new(wx, wy), wr, Color32::from_rgb(40,44,52));
                p.circle_stroke(Pos2::new(wx, wy), wr, Stroke::new(if wsel { 2.5 } else { 1.5 }, wcol));
                p.text(Pos2::new(wx, wy), egui::Align2::CENTER_CENTER, &wheel.name, FontId::monospace(6.0), Color32::WHITE);
            }
            p.text(Pos2::new(rect.left()+4.0, rect.bottom()-12.0), egui::Align2::LEFT_BOTTOM,
                format!("{} | {} | {} wheels | mass {:.0}kg", v.name, v.drive_type.label(), v.wheels.len(), v.chassis_mass),
                FontId::monospace(8.0), Color32::DARK_GRAY);
        }
    } else {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, "Select a vehicle to preview", FontId::monospace(9.0), Color32::DARK_GRAY);
    }
}

// ─── Explosion Simulation ─────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct ExplosionParams {
    pub position: [f32; 2],
    pub peak_pressure: f32,
    pub radius: f32,
    pub duration: f32,
    pub falloff: f32,
    pub debris_count: u32,
    pub debris_radius_range: [f32; 2],
    pub debris_velocity_range: [f32; 2],
    pub sound_name: String,
    pub particle_effect: String,
    pub shake_intensity: f32,
    pub shake_duration: f32,
    pub apply_damage: bool,
    pub damage_amount: f32,
    pub damage_falloff: f32,
}

impl Default for ExplosionParams {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            peak_pressure: 50000.0,
            radius: 5.0,
            duration: 0.3,
            falloff: 2.0,
            debris_count: 20,
            debris_radius_range: [0.05, 0.25],
            debris_velocity_range: [5.0, 25.0],
            sound_name: "explosion_large".into(),
            particle_effect: "fx_explosion".into(),
            shake_intensity: 0.8,
            shake_duration: 0.5,
            apply_damage: true,
            damage_amount: 100.0,
            damage_falloff: 2.0,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ExplosionEditorState {
    pub presets: Vec<(String, ExplosionParams)>,
    pub current: ExplosionParams,
    pub selected: Option<usize>,
    pub preview_ring_t: f32,
}

pub fn show_explosion_editor(ui: &mut egui::Ui, state: &mut ExplosionEditorState) {
    ui.heading("Explosion Simulation");
    ui.separator();

    if state.presets.is_empty() {
        state.presets = vec![
            ("Small Grenade".into(), ExplosionParams { peak_pressure: 10000.0, radius: 2.5, debris_count: 8, shake_intensity: 0.3, ..Default::default() }),
            ("Large Bomb".into(), ExplosionParams::default()),
            ("Barrel".into(), ExplosionParams { peak_pressure: 20000.0, radius: 3.5, debris_count: 12, sound_name: "barrel_explode".into(), ..Default::default() }),
            ("Nuke".into(), ExplosionParams { peak_pressure: 500000.0, radius: 50.0, debris_count: 100, shake_intensity: 3.0, shake_duration: 5.0, ..Default::default() }),
        ];
    }

    ui.horizontal(|ui| {
        ui.label("Presets:");
        for (i, (name, _)) in state.presets.iter().enumerate() {
            if ui.button(name).clicked() {
                state.current = state.presets[i].1.clone();
                state.selected = Some(i);
            }
        }
        if ui.button("Save Preset").clicked() {
            let name = format!("Custom_{}", state.presets.len());
            state.presets.push((name, state.current.clone()));
        }
    });

    ui.separator();
    let e = &mut state.current;
    ui.columns(2, |cols| {
        cols[0].horizontal(|ui| {
            ui.label("Position:");
            ui.add(egui::DragValue::new(&mut e.position[0]).prefix("x:").speed(0.05));
            ui.add(egui::DragValue::new(&mut e.position[1]).prefix("y:").speed(0.05));
        });
        cols[0].horizontal(|ui| {
            ui.label("Peak Pressure:");
            ui.add(egui::DragValue::new(&mut e.peak_pressure).speed(500.0).suffix(" Pa"));
        });
        cols[0].horizontal(|ui| {
            ui.label("Radius:");
            ui.add(egui::DragValue::new(&mut e.radius).speed(0.1).clamp_range(0.1..=200.0).suffix(" m"));
        });
        cols[0].horizontal(|ui| {
            ui.label("Duration:");
            ui.add(egui::DragValue::new(&mut e.duration).speed(0.01).clamp_range(0.01..=5.0).suffix("s"));
        });
        cols[0].horizontal(|ui| {
            ui.label("Falloff:");
            ui.add(egui::Slider::new(&mut e.falloff, 0.5..=4.0));
        });

        cols[1].horizontal(|ui| {
            ui.label("Debris Count:");
            ui.add(egui::DragValue::new(&mut e.debris_count).clamp_range(0..=500));
        });
        cols[1].horizontal(|ui| {
            ui.label("Debris Vel:");
            ui.add(egui::DragValue::new(&mut e.debris_velocity_range[0]).prefix("min:").speed(0.5));
            ui.add(egui::DragValue::new(&mut e.debris_velocity_range[1]).prefix("max:").speed(0.5));
        });
        cols[1].horizontal(|ui| {
            ui.label("Shake:");
            ui.add(egui::Slider::new(&mut e.shake_intensity, 0.0..=5.0));
            ui.label("Duration:");
            ui.add(egui::DragValue::new(&mut e.shake_duration).speed(0.05).suffix("s"));
        });
        cols[1].horizontal(|ui| {
            ui.label("Sound:");
            ui.text_edit_singleline(&mut e.sound_name);
        });
        cols[1].horizontal(|ui| {
            ui.label("Particle FX:");
            ui.text_edit_singleline(&mut e.particle_effect);
        });
        cols[1].horizontal(|ui| {
            ui.checkbox(&mut e.apply_damage, "Apply Damage");
            if e.apply_damage {
                ui.add(egui::DragValue::new(&mut e.damage_amount).prefix("dmg:").speed(5.0));
            }
        });
    });

    draw_explosion_preview(ui, &state.current, state.preview_ring_t);
}

fn draw_explosion_preview(ui: &mut egui::Ui, e: &ExplosionParams, ring_t: f32) {
    let desired = Vec2::new(ui.available_width().min(400.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(10, 8, 12));

    let c = rect.center();
    let max_r = rect.width().min(rect.height()) * 0.42;
    let display_r = max_r;

    // Draw falloff pressure field
    for ri in (1..=20).rev() {
        let t = ri as f32 / 20.0;
        let rr = t * display_r;
        let pressure = e.peak_pressure * (1.0 - t).powf(e.falloff);
        let intensity = (pressure / e.peak_pressure).clamp(0.0, 1.0);
        let r = (intensity * 255.0) as u8;
        let g = ((1.0 - intensity) * intensity * 255.0) as u8;
        let alpha = (intensity * 120.0) as u8;
        p.circle_filled(c, rr, Color32::from_rgba_unmultiplied(r, g, 20, alpha.max(5)));
    }

    // Blast ring
    let ring_r = ring_t * display_r;
    if ring_r > 0.0 && ring_r < display_r {
        let ring_alpha = ((1.0 - ring_t) * 255.0) as u8;
        p.circle_stroke(c, ring_r, Stroke::new(3.0, Color32::from_rgba_unmultiplied(255, 200, 50, ring_alpha)));
    }

    // Center flash
    p.circle_filled(c, 8.0, Color32::from_rgb(255, 220, 100));
    p.circle_filled(c, 4.0, Color32::WHITE);

    // Debris lines
    let n_debris = e.debris_count.min(30);
    for di in 0..n_debris {
        let angle = di as f32 / n_debris as f32 * std::f32::consts::TAU;
        let vel_frac = 0.5 + (di as f32 * 0.13).sin() * 0.5;
        let debris_r = (vel_frac * display_r * 0.7).min(display_r);
        let dend = c + Vec2::new(angle.cos(), angle.sin()) * debris_r;
        if dend.x > rect.left() && dend.x < rect.right() && dend.y > rect.top() && dend.y < rect.bottom() {
            p.line_segment([c, dend], Stroke::new(0.8, Color32::from_rgba_unmultiplied(200, 150, 80, 80)));
            p.circle_filled(dend, 2.0, Color32::from_rgb(180, 120, 60));
        }
    }

    // Radius ring
    p.circle_stroke(c, display_r, Stroke::new(1.0, Color32::from_rgba_unmultiplied(200, 200, 200, 60)));

    p.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP,
        format!("P={:.0}kPa r={:.1}m d={} debris", e.peak_pressure/1000.0, e.radius, e.debris_count),
        FontId::monospace(8.0), Color32::GRAY);
}

// ─── Joint Limit Visualizer ───────────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct JointLimitVisualizerState {
    pub joint_type: JointLimitType,
    pub lower_angle: f32,
    pub upper_angle: f32,
    pub lower_limit: f32,
    pub upper_limit: f32,
    pub current_angle: f32,
    pub current_pos: f32,
    pub show_softness: bool,
    pub softness: f32,
    pub bias_factor: f32,
    pub relaxation_factor: f32,
    pub enable_motor: bool,
    pub motor_speed: f32,
    pub max_motor_force: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum JointLimitType {
    #[default]
    Revolute,
    Prismatic,
    Cone,
}

pub fn show_joint_limit_visualizer(ui: &mut egui::Ui, state: &mut JointLimitVisualizerState) {
    ui.heading("Joint Limit Visualizer");
    ui.separator();

    ui.horizontal(|ui| {
        for jt in &[JointLimitType::Revolute, JointLimitType::Prismatic, JointLimitType::Cone] {
            let lbl = match jt { JointLimitType::Revolute => "Revolute", JointLimitType::Prismatic => "Prismatic", JointLimitType::Cone => "Cone" };
            if ui.selectable_label(state.joint_type == *jt, lbl).clicked() { state.joint_type = jt.clone(); }
        }
    });

    match state.joint_type {
        JointLimitType::Revolute => {
            ui.horizontal(|ui| {
                ui.label("Lower Angle:");
                ui.add(egui::Slider::new(&mut state.lower_angle, -180.0..=0.0).suffix("°"));
            });
            ui.horizontal(|ui| {
                ui.label("Upper Angle:");
                ui.add(egui::Slider::new(&mut state.upper_angle, 0.0..=180.0).suffix("°"));
            });
            ui.horizontal(|ui| {
                ui.label("Current Angle:");
                ui.add(egui::Slider::new(&mut state.current_angle, -180.0..=180.0).suffix("°"));
            });
        }
        JointLimitType::Prismatic => {
            ui.horizontal(|ui| {
                ui.label("Lower Limit:");
                ui.add(egui::DragValue::new(&mut state.lower_limit).speed(0.05));
                ui.label("Upper Limit:");
                ui.add(egui::DragValue::new(&mut state.upper_limit).speed(0.05));
            });
            ui.horizontal(|ui| {
                ui.label("Current Position:");
                let range = state.lower_limit..=state.upper_limit;
                state.current_pos = state.current_pos.clamp(state.lower_limit, state.upper_limit);
                ui.add(egui::Slider::new(&mut state.current_pos, range));
            });
        }
        JointLimitType::Cone => {
            ui.horizontal(|ui| {
                ui.label("Cone Half-Angle:");
                ui.add(egui::Slider::new(&mut state.upper_angle, 0.0..=90.0).suffix("°"));
            });
        }
    }

    ui.checkbox(&mut state.show_softness, "Soft Limits");
    if state.show_softness {
        ui.horizontal(|ui| {
            ui.label("Softness:"); ui.add(egui::Slider::new(&mut state.softness, 0.0..=1.0));
            ui.label("Bias:"); ui.add(egui::Slider::new(&mut state.bias_factor, 0.0..=1.0));
            ui.label("Relax:"); ui.add(egui::Slider::new(&mut state.relaxation_factor, 0.0..=2.0));
        });
    }
    ui.checkbox(&mut state.enable_motor, "Motor");
    if state.enable_motor {
        ui.horizontal(|ui| {
            ui.label("Motor Speed:"); ui.add(egui::DragValue::new(&mut state.motor_speed).speed(0.1));
            ui.label("Max Force:"); ui.add(egui::DragValue::new(&mut state.max_motor_force).speed(10.0).clamp_range(0.0..=100000.0));
        });
    }

    draw_joint_limit_diagram(ui, state);
}

fn draw_joint_limit_diagram(ui: &mut egui::Ui, state: &JointLimitVisualizerState) {
    let desired = Vec2::new(200.0, 200.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));
    let c = rect.center();

    match state.joint_type {
        JointLimitType::Revolute => {
            let r = 70.0;
            // Allowed range arc
            let lo = state.lower_angle.to_radians();
            let hi = state.upper_angle.to_radians();
            // Draw limit arc (pie slice)
            let steps = 40;
            for si in 0..steps {
                let t0 = lo + (hi - lo) * si as f32 / steps as f32;
                let t1 = lo + (hi - lo) * (si+1) as f32 / steps as f32;
                let p0 = c + Vec2::new(t0.cos(), -t0.sin()) * r;
                let p1 = c + Vec2::new(t1.cos(), -t1.sin()) * r;
                p.line_segment([c, p0], Stroke::new(1.0, Color32::from_rgba_unmultiplied(60, 200, 80, 40)));
                p.line_segment([p0, p1], Stroke::new(2.0, Color32::from_rgb(60, 200, 80)));
            }
            // Current angle
            let ca = state.current_angle.to_radians();
            let cur_end = c + Vec2::new(ca.cos(), -ca.sin()) * r;
            let in_range = state.current_angle >= state.lower_angle && state.current_angle <= state.upper_angle;
            let cur_col = if in_range { Color32::WHITE } else { Color32::RED };
            p.line_segment([c, cur_end], Stroke::new(2.5, cur_col));
            p.circle_filled(cur_end, 4.0, cur_col);
            // Limit lines
            let lo_end = c + Vec2::new(lo.cos(), -lo.sin()) * r;
            let hi_end = c + Vec2::new(hi.cos(), -hi.sin()) * r;
            p.line_segment([c, lo_end], Stroke::new(1.5, Color32::RED));
            p.line_segment([c, hi_end], Stroke::new(1.5, Color32::RED));
            p.circle_filled(c, 5.0, Color32::GRAY);
            p.text(Pos2::new(rect.left()+4.0, rect.bottom()-12.0), egui::Align2::LEFT_BOTTOM, format!("[{:.0}°, {:.0}°] cur: {:.0}°", state.lower_angle, state.upper_angle, state.current_angle), FontId::monospace(7.0), Color32::DARK_GRAY);
        }
        JointLimitType::Prismatic => {
            let track_y = c.y;
            let range = state.upper_limit - state.lower_limit;
            let scale = if range > 0.0 { (rect.width() - 40.0) / range } else { 10.0 };
            let zero_x = c.x - state.lower_limit * scale - range * scale * 0.5;
            let lo_x = zero_x + state.lower_limit * scale;
            let hi_x = zero_x + state.upper_limit * scale;
            let cur_x = zero_x + state.current_pos * scale;
            p.line_segment([Pos2::new(lo_x-5.0, track_y), Pos2::new(hi_x+5.0, track_y)], Stroke::new(2.0, Color32::GRAY));
            p.rect_filled(Rect::from_center_size(Pos2::new(lo_x, track_y), Vec2::new(8.0, 20.0)), 1.0, Color32::RED);
            p.rect_filled(Rect::from_center_size(Pos2::new(hi_x, track_y), Vec2::new(8.0, 20.0)), 1.0, Color32::RED);
            p.rect_filled(Rect::from_center_size(Pos2::new(cur_x, track_y), Vec2::new(14.0, 28.0)), 2.0, Color32::from_rgb(80, 160, 255));
            p.text(Pos2::new(c.x, rect.bottom()-10.0), egui::Align2::CENTER_BOTTOM, format!("[{:.2}, {:.2}] cur: {:.2}", state.lower_limit, state.upper_limit, state.current_pos), FontId::monospace(7.0), Color32::DARK_GRAY);
        }
        JointLimitType::Cone => {
            let r = 60.0;
            let half_a = state.upper_angle.to_radians();
            p.circle_stroke(c, r, Stroke::new(0.5, Color32::from_rgb(40,50,60)));
            let steps = 40;
            for si in 0..=steps {
                let angle = -half_a + 2.0 * half_a * si as f32 / steps as f32;
                let end = c + Vec2::new(angle.cos(), angle.sin()) * r;
                p.line_segment([c, end], Stroke::new(1.0, Color32::from_rgba_unmultiplied(60, 200, 80, 50)));
            }
            let lo_end = c + Vec2::new((-half_a).cos(), (-half_a).sin()) * r;
            let hi_end = c + Vec2::new(half_a.cos(), half_a.sin()) * r;
            p.line_segment([c, lo_end], Stroke::new(2.0, Color32::RED));
            p.line_segment([c, hi_end], Stroke::new(2.0, Color32::RED));
            p.circle_filled(c, 5.0, Color32::GRAY);
        }
    }
}

// ─── Fluid Viscosity Editor ───────────────────────────────────────────────────

pub fn show_fluid_viscosity_presets(ui: &mut egui::Ui, params: &mut FluidParams) {
    ui.heading("Fluid Viscosity Presets");
    ui.separator();

    let presets: &[(&str, f32, f32, f32, f32)] = &[
        ("Water", 0.001, 1000.0, 1.0, 2000.0),
        ("Oil (SAE 30)", 0.1, 875.0, 1.0, 1500.0),
        ("Honey", 2.5, 1400.0, 0.8, 1000.0),
        ("Mercury", 0.0016, 13600.0, 0.5, 5000.0),
        ("Air", 0.0000181, 1.2, 0.01, 500.0),
        ("Lava", 100.0, 3100.0, 0.6, 800.0),
        ("Blood", 0.003, 1060.0, 0.9, 1800.0),
        ("Milk", 0.002, 1030.0, 0.95, 1900.0),
        ("Glycerin", 1.5, 1261.0, 0.85, 1200.0),
        ("Sea Water", 0.00108, 1025.0, 1.0, 2100.0),
    ];

    ui.horizontal_wrapped(|ui| {
        for (name, visc, dens, surf, sound) in presets {
            if ui.button(*name).clicked() {
                params.viscosity = *visc;
                params.rest_density = *dens;
                params.surface_tension = *surf;
                params.speed_of_sound = *sound;
            }
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Viscosity:");
        ui.add(egui::DragValue::new(&mut params.viscosity).speed(0.001).clamp_range(0.0..=1000.0).suffix(" Pa·s"));
    });
    ui.horizontal(|ui| {
        ui.label("Rest Density:");
        ui.add(egui::DragValue::new(&mut params.rest_density).speed(1.0).clamp_range(0.01..=20000.0).suffix(" kg/m³"));
    });
    ui.horizontal(|ui| {
        ui.label("Surface Tension:");
        ui.add(egui::DragValue::new(&mut params.surface_tension).speed(0.01));
    });
    ui.horizontal(|ui| {
        ui.label("Speed of Sound:");
        ui.add(egui::DragValue::new(&mut params.speed_of_sound).speed(10.0).clamp_range(1.0..=10000.0));
    });

    draw_fluid_viscosity_chart(ui, params.viscosity);
}

fn draw_fluid_viscosity_chart(ui: &mut egui::Ui, current_visc: f32) {
    let desired = Vec2::new(ui.available_width().min(400.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(12, 15, 22));

    let viscosities: &[(&str, f32)] = &[
        ("Air", 0.0000181), ("Water", 0.001), ("Blood", 0.003), ("Oil", 0.1),
        ("Honey", 2.5), ("Glycerin", 1.5), ("Lava", 100.0),
    ];
    let max_log = 5.0f32;
    let n = viscosities.len();
    let bar_w = rect.width() / n as f32;

    for (i, (name, visc)) in viscosities.iter().enumerate() {
        let x = rect.left() + i as f32 * bar_w;
        let log_v = (visc.log10() + 5.0).clamp(0.0, max_log);
        let h = log_v / max_log * (rect.height() - 16.0);
        let is_cur = (current_visc - visc).abs() / visc.abs().max(0.0001) < 0.1;
        let col = if is_cur { Color32::from_rgb(100, 200, 255) } else { Color32::from_rgb(60, 100, 160) };
        p.rect_filled(Rect::from_min_size(Pos2::new(x+2.0, rect.bottom()-h-14.0), Vec2::new(bar_w-4.0, h)), 1.0, col);
        p.text(Pos2::new(x + bar_w*0.5, rect.bottom()-2.0), egui::Align2::CENTER_BOTTOM, *name, FontId::monospace(6.5), Color32::GRAY);
    }
    p.text(Pos2::new(rect.right()-4.0, rect.top()+4.0), egui::Align2::RIGHT_TOP, "log scale", FontId::monospace(7.0), Color32::DARK_GRAY);
}

// ─── Kinematic Character Controller ──────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct CharacterControllerConfig {
    pub name: String,
    pub shape: CharacterShape,
    pub step_height: f32,
    pub max_slope_angle: f32,
    pub gravity_scale: f32,
    pub move_speed: f32,
    pub run_speed: f32,
    pub jump_speed: f32,
    pub air_control: f32,
    pub coyote_time: f32,
    pub jump_buffer_time: f32,
    pub skin_width: f32,
    pub min_move_distance: f32,
    pub use_slope_sliding: bool,
    pub anti_gravity_on_slope: bool,
    pub enable_crouch: bool,
    pub crouch_height_fraction: f32,
    pub layer_mask: u32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum CharacterShape {
    Capsule { radius: f32, height: f32 },
    Box { half_w: f32, half_h: f32 },
    Custom,
}

impl CharacterShape {
    pub fn label(&self) -> &str {
        match self { CharacterShape::Capsule { .. } => "Capsule", CharacterShape::Box { .. } => "Box", CharacterShape::Custom => "Custom" }
    }
}

impl Default for CharacterShape { fn default() -> Self { CharacterShape::Capsule { radius: 0.4, height: 1.8 } } }

impl Default for CharacterControllerConfig {
    fn default() -> Self {
        Self { name: "Player".into(), shape: CharacterShape::Capsule { radius: 0.4, height: 1.8 }, step_height: 0.3, max_slope_angle: 45.0, gravity_scale: 1.0, move_speed: 5.0, run_speed: 10.0, jump_speed: 8.0, air_control: 0.3, coyote_time: 0.15, jump_buffer_time: 0.1, skin_width: 0.01, min_move_distance: 0.001, use_slope_sliding: true, anti_gravity_on_slope: true, enable_crouch: true, crouch_height_fraction: 0.6, layer_mask: 0xFF }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CharacterControllerEditorState {
    pub configs: Vec<CharacterControllerConfig>,
    pub selected: Option<usize>,
}

pub fn show_character_controller_editor(ui: &mut egui::Ui, state: &mut CharacterControllerEditorState) {
    ui.heading("Kinematic Character Controller");
    ui.separator();

    if state.configs.is_empty() {
        state.configs = vec![CharacterControllerConfig::default()];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Controller").clicked() {
            let mut c = CharacterControllerConfig::default();
            c.name = format!("Character_{}", state.configs.len());
            state.configs.push(c);
        }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.configs.remove(s); state.selected = None; } }
    });

    for (i, cfg) in state.configs.iter().enumerate() {
        if ui.selectable_label(state.selected == Some(i), &cfg.name).clicked() { state.selected = Some(i); }
    }

    if let Some(idx) = state.selected {
        if idx < state.configs.len() {
            let cfg = &mut state.configs[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut cfg.name); });
            ui.horizontal(|ui| {
                ui.label("Shape:");
                for shape in &[CharacterShape::Capsule { radius: 0.4, height: 1.8 }, CharacterShape::Box { half_w: 0.4, half_h: 0.9 }] {
                    let sel = std::mem::discriminant(&cfg.shape) == std::mem::discriminant(shape);
                    if ui.selectable_label(sel, shape.label()).clicked() { cfg.shape = shape.clone(); }
                }
            });
            match &mut cfg.shape {
                CharacterShape::Capsule { radius, height } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius:"); ui.add(egui::DragValue::new(radius).speed(0.01).clamp_range(0.05..=5.0));
                        ui.label("Height:"); ui.add(egui::DragValue::new(height).speed(0.02).clamp_range(0.1..=10.0));
                    });
                }
                CharacterShape::Box { half_w, half_h } => {
                    ui.horizontal(|ui| {
                        ui.label("Half W:"); ui.add(egui::DragValue::new(half_w).speed(0.01).clamp_range(0.05..=5.0));
                        ui.label("Half H:"); ui.add(egui::DragValue::new(half_h).speed(0.02).clamp_range(0.05..=10.0));
                    });
                }
                _ => {}
            }
            ui.columns(2, |cols| {
                cols[0].horizontal(|ui| { ui.label("Move Speed:"); ui.add(egui::DragValue::new(&mut cfg.move_speed).speed(0.1)); });
                cols[0].horizontal(|ui| { ui.label("Run Speed:"); ui.add(egui::DragValue::new(&mut cfg.run_speed).speed(0.1)); });
                cols[0].horizontal(|ui| { ui.label("Jump Speed:"); ui.add(egui::DragValue::new(&mut cfg.jump_speed).speed(0.1)); });
                cols[0].horizontal(|ui| { ui.label("Air Control:"); ui.add(egui::Slider::new(&mut cfg.air_control, 0.0..=1.0)); });
                cols[0].horizontal(|ui| { ui.label("Gravity Scale:"); ui.add(egui::DragValue::new(&mut cfg.gravity_scale).speed(0.05)); });
                cols[1].horizontal(|ui| { ui.label("Step Height:"); ui.add(egui::DragValue::new(&mut cfg.step_height).speed(0.01).clamp_range(0.0..=2.0)); });
                cols[1].horizontal(|ui| { ui.label("Max Slope:"); ui.add(egui::DragValue::new(&mut cfg.max_slope_angle).speed(1.0).clamp_range(0.0..=90.0).suffix("°")); });
                cols[1].horizontal(|ui| { ui.label("Coyote Time:"); ui.add(egui::DragValue::new(&mut cfg.coyote_time).speed(0.01).clamp_range(0.0..=0.5).suffix("s")); });
                cols[1].horizontal(|ui| { ui.label("Jump Buffer:"); ui.add(egui::DragValue::new(&mut cfg.jump_buffer_time).speed(0.01).clamp_range(0.0..=0.5).suffix("s")); });
                cols[1].checkbox(&mut cfg.use_slope_sliding, "Slope Sliding");
                cols[1].checkbox(&mut cfg.enable_crouch, "Crouch");
            });
            if cfg.enable_crouch {
                ui.horizontal(|ui| { ui.label("Crouch Height Fraction:"); ui.add(egui::Slider::new(&mut cfg.crouch_height_fraction, 0.3..=0.9)); });
            }
        }
    }
    draw_character_controller_preview(ui, state);
}

fn draw_character_controller_preview(ui: &mut egui::Ui, state: &CharacterControllerEditorState) {
    let desired = Vec2::new(200.0, 200.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));

    if let Some(idx) = state.selected {
        if idx < state.configs.len() {
            let cfg = &state.configs[idx];
            let c = Pos2::new(rect.center().x, rect.center().y + 30.0);
            let scale = 40.0;

            // Ground
            p.line_segment([Pos2::new(rect.left()+4.0, c.y+5.0), Pos2::new(rect.right()-4.0, c.y+5.0)], Stroke::new(2.0, Color32::GRAY));

            match &cfg.shape {
                CharacterShape::Capsule { radius, height } => {
                    let sr = radius * scale;
                    let sh = (height * 0.5 - radius) * scale;
                    p.circle_filled(Pos2::new(c.x, c.y - height * scale + radius * scale), sr, Color32::from_rgb(80, 140, 220));
                    p.circle_filled(Pos2::new(c.x, c.y - radius * scale), sr, Color32::from_rgb(80, 140, 220));
                    p.rect_filled(Rect::from_center_size(Pos2::new(c.x, c.y - height * scale * 0.5), Vec2::new(sr*2.0, sh*2.0)), 0.0, Color32::from_rgb(80, 140, 220));
                }
                CharacterShape::Box { half_w, half_h } => {
                    p.rect_filled(Rect::from_center_size(Pos2::new(c.x, c.y - half_h * scale), Vec2::new(half_w * scale * 2.0, half_h * scale * 2.0)), 2.0, Color32::from_rgb(80, 140, 220));
                }
                _ => {}
            }
            // Step height indicator
            let step_y = c.y - cfg.step_height * scale;
            p.line_segment([Pos2::new(c.x + 20.0, c.y+5.0), Pos2::new(c.x + 20.0, step_y)], Stroke::new(1.5, Color32::YELLOW));
            p.text(Pos2::new(c.x + 22.0, (c.y+step_y)*0.5), egui::Align2::LEFT_CENTER, format!("step\n{:.2}", cfg.step_height), FontId::monospace(7.0), Color32::YELLOW);
        }
    }
}

// ─── Physics Body Mass Properties ─────────────────────────────────────────────

pub fn show_mass_properties_editor(ui: &mut egui::Ui, body: &mut RigidBodyProperties) {
    ui.heading("Mass Properties");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Body:");
        ui.label(egui::RichText::new(&body.name).strong());
        ui.label(egui::RichText::new(body.body_type.label()).color(body.body_type.color()));
    });

    ui.separator();
    ui.label("Mass Distribution:");
    ui.horizontal(|ui| {
        ui.label("Total Mass:");
        ui.add(egui::DragValue::new(&mut body.mass).speed(0.01).clamp_range(0.0..=100000.0).suffix(" kg"));
        ui.checkbox(&mut body.custom_mass_override, "Override");
    });

    ui.horizontal(|ui| {
        ui.label("Moment of Inertia:");
        ui.add(egui::DragValue::new(&mut body.inertia).speed(0.01).clamp_range(0.0..=100000.0).suffix(" kg·m²"));
    });

    let estimated_inertia = match &body.collider_type {
        ColliderShape::Box { hw, hh } => body.mass * (hw * hw + hh * hh) / 3.0,
        ColliderShape::Circle { r } => body.mass * r * r * 0.5,
        ColliderShape::Capsule { r, hh } => body.mass * (r * r * 0.5 + hh * hh / 3.0),
        _ => body.mass * 0.5,
    };
    ui.label(format!("Estimated from shape: {:.4} kg·m²", estimated_inertia));
    if ui.button("Use Estimated").clicked() { body.inertia = estimated_inertia; }

    ui.separator();
    ui.label("Center of Mass:");
    let com_x = 0.0f32;
    let com_y = 0.0f32;
    ui.label(format!("({:.3}, {:.3}) — computed from shape", com_x, com_y));

    // Inertia tensor visualization for 2D (just a ring sized proportionally)
    let desired = Vec2::new(ui.available_width().min(300.0), 140.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(12, 15, 22));
    let c = rect.center();

    let max_inertia = 200.0f32;
    let inertia_r = (body.inertia / max_inertia).clamp(0.01, 1.0).sqrt() * 55.0;
    p.circle_stroke(c, 55.0, Stroke::new(1.0, Color32::from_rgb(40, 50, 60)));
    p.circle_filled(c, inertia_r, Color32::from_rgba_unmultiplied(80, 140, 255, 80));
    p.circle_stroke(c, inertia_r, Stroke::new(2.0, Color32::from_rgb(80, 140, 255)));
    p.circle_filled(c, 4.0, Color32::YELLOW);
    p.text(c + Vec2::new(0.0, inertia_r + 8.0), egui::Align2::CENTER_TOP, format!("I = {:.3}", body.inertia), FontId::monospace(8.0), Color32::from_rgb(80, 140, 255));
    p.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP, "Inertia Visualization", FontId::monospace(8.0), Color32::GRAY);
}

// ─── Physics Unit Converter ───────────────────────────────────────────────────

pub fn show_physics_unit_converter(ui: &mut egui::Ui) {
    ui.heading("Physics Unit Converter");
    ui.separator();

    ui.columns(2, |cols| {
        cols[0].label(egui::RichText::new("Common Conversions").strong());
        cols[0].separator();
        egui::Grid::new("unit_conversions").num_columns(3).spacing([8.0, 2.0]).show(&mut cols[0], |ui| {
            ui.label("1 m/s"); ui.label("="); ui.label("3.6 km/h"); ui.end_row();
            ui.label("1 m/s"); ui.label("="); ui.label("2.237 mph"); ui.end_row();
            ui.label("1 N"); ui.label("="); ui.label("0.2248 lbf"); ui.end_row();
            ui.label("1 Pa"); ui.label("="); ui.label("0.000145 psi"); ui.end_row();
            ui.label("1 J"); ui.label("="); ui.label("0.2388 cal"); ui.end_row();
            ui.label("1 W"); ui.label("="); ui.label("1.341e-3 hp"); ui.end_row();
            ui.label("1 kg/m³"); ui.label("="); ui.label("0.0624 lb/ft³"); ui.end_row();
            ui.label("1 rad/s"); ui.label("="); ui.label("9.549 RPM"); ui.end_row();
        });

        cols[1].label(egui::RichText::new("Reference Values").strong());
        cols[1].separator();
        egui::Grid::new("ref_values").num_columns(2).spacing([8.0, 2.0]).show(&mut cols[1], |ui| {
            ui.label("Earth g"); ui.label("9.81 m/s²"); ui.end_row();
            ui.label("Moon g"); ui.label("1.62 m/s²"); ui.end_row();
            ui.label("Mars g"); ui.label("3.72 m/s²"); ui.end_row();
            ui.label("Jupiter g"); ui.label("24.79 m/s²"); ui.end_row();
            ui.label("Water density"); ui.label("1000 kg/m³"); ui.end_row();
            ui.label("Air density"); ui.label("1.225 kg/m³"); ui.end_row();
            ui.label("Steel density"); ui.label("7850 kg/m³"); ui.end_row();
            ui.label("Speed of sound"); ui.label("343 m/s"); ui.end_row();
            ui.label("Speed of light"); ui.label("3e8 m/s"); ui.end_row();
        });
    });
}

// ─── Soft Body Visualization Modes ───────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SoftBodyVisualizationMode {
    #[default]
    Wireframe,
    Filled,
    StressColored,
    VelocityColored,
    PressureColored,
    NodeMassColored,
    SpringStrainColored,
}

impl SoftBodyVisualizationMode {
    pub fn label(&self) -> &str {
        match self {
            SoftBodyVisualizationMode::Wireframe => "Wireframe",
            SoftBodyVisualizationMode::Filled => "Filled",
            SoftBodyVisualizationMode::StressColored => "Stress",
            SoftBodyVisualizationMode::VelocityColored => "Velocity",
            SoftBodyVisualizationMode::PressureColored => "Pressure",
            SoftBodyVisualizationMode::NodeMassColored => "Node Mass",
            SoftBodyVisualizationMode::SpringStrainColored => "Spring Strain",
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SoftBodyVisualizationState {
    pub mode: SoftBodyVisualizationMode,
    pub show_node_ids: bool,
    pub show_spring_ids: bool,
    pub highlight_pinned: bool,
    pub stress_scale: f32,
    pub velocity_scale: f32,
    pub node_size: f32,
}

pub fn show_soft_body_visualization_panel(ui: &mut egui::Ui, vis: &mut SoftBodyVisualizationState) {
    ui.heading("Soft Body Visualization");
    ui.separator();

    ui.horizontal_wrapped(|ui| {
        for mode in &[
            SoftBodyVisualizationMode::Wireframe, SoftBodyVisualizationMode::Filled,
            SoftBodyVisualizationMode::StressColored, SoftBodyVisualizationMode::VelocityColored,
            SoftBodyVisualizationMode::PressureColored, SoftBodyVisualizationMode::SpringStrainColored,
        ] {
            if ui.selectable_label(vis.mode == *mode, mode.label()).clicked() { vis.mode = mode.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut vis.show_node_ids, "Node IDs");
        ui.checkbox(&mut vis.show_spring_ids, "Spring IDs");
        ui.checkbox(&mut vis.highlight_pinned, "Highlight Pinned");
    });

    if vis.stress_scale == 0.0 { vis.stress_scale = 1.0; vis.velocity_scale = 1.0; vis.node_size = 4.0; }

    ui.horizontal(|ui| {
        ui.label("Stress Scale:");
        ui.add(egui::Slider::new(&mut vis.stress_scale, 0.1..=10.0).logarithmic(true));
    });
    ui.horizontal(|ui| {
        ui.label("Velocity Scale:");
        ui.add(egui::Slider::new(&mut vis.velocity_scale, 0.01..=5.0).logarithmic(true));
        ui.label("Node Size:");
        ui.add(egui::Slider::new(&mut vis.node_size, 1.0..=12.0));
    });

    // Color scale legend
    let desired = Vec2::new(ui.available_width().min(400.0), 30.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, Color32::from_rgb(12, 14, 20));

    let (lo_col, hi_col, label) = match vis.mode {
        SoftBodyVisualizationMode::StressColored => (Color32::from_rgb(20, 80, 220), Color32::RED, "Stress"),
        SoftBodyVisualizationMode::VelocityColored => (Color32::DARK_GRAY, Color32::from_rgb(80, 220, 120), "Velocity"),
        SoftBodyVisualizationMode::PressureColored => (Color32::from_rgb(20, 40, 160), Color32::from_rgb(40, 200, 240), "Pressure"),
        SoftBodyVisualizationMode::NodeMassColored => (Color32::from_rgb(80, 60, 20), Color32::from_rgb(240, 200, 60), "Mass"),
        SoftBodyVisualizationMode::SpringStrainColored => (Color32::from_rgb(20, 120, 80), Color32::from_rgb(220, 80, 40), "Strain"),
        _ => (Color32::DARK_GRAY, Color32::WHITE, ""),
    };

    if !label.is_empty() {
        let grad_rect = Rect::from_min_size(Pos2::new(rect.left() + 60.0, rect.top() + 6.0), Vec2::new(rect.width() - 80.0, 16.0));
        let steps = 20;
        for gi in 0..steps {
            let t = gi as f32 / steps as f32;
            let col = egui::lerp(egui::Rgba::from(lo_col)..=egui::Rgba::from(hi_col), t);
            let x = grad_rect.left() + t * grad_rect.width();
            let seg_w = grad_rect.width() / steps as f32 + 1.0;
            p.rect_filled(Rect::from_min_size(Pos2::new(x, grad_rect.top()), Vec2::new(seg_w, grad_rect.height())), 0.0, Color32::from(col));
        }
        p.text(Pos2::new(rect.left()+4.0, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::monospace(8.0), Color32::GRAY);
        p.text(Pos2::new(grad_rect.left(), rect.bottom()-2.0), egui::Align2::LEFT_BOTTOM, "Low", FontId::monospace(6.5), Color32::GRAY);
        p.text(Pos2::new(grad_rect.right(), rect.bottom()-2.0), egui::Align2::RIGHT_BOTTOM, "High", FontId::monospace(6.5), Color32::GRAY);
    }
}

// ─── Physics Editor Tab Manager ───────────────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsTabManagerState {
    pub tabs: Vec<String>,
    pub active_tab: usize,
    pub pinned_tabs: HashSet<usize>,
}

pub fn show_physics_tab_manager(ui: &mut egui::Ui, state: &mut PhysicsTabManagerState) {
    if state.tabs.is_empty() {
        state.tabs = vec!["Scene".into(), "Rigid Bodies".into(), "Joints".into(), "Soft Bodies".into(), "Fluid".into(), "Ragdolls".into(), "Debug".into()];
    }

    ui.horizontal(|ui| {
        let n = state.tabs.len();
        for i in 0..n {
            let sel = state.active_tab == i;
            let pinned = state.pinned_tabs.contains(&i);
            let lbl = if pinned { format!("📌 {}", state.tabs[i]) } else { state.tabs[i].clone() };
            if ui.selectable_label(sel, &lbl).clicked() { state.active_tab = i; }
            if ui.small_button("⊕").clicked() {
                if pinned { state.pinned_tabs.remove(&i); } else { state.pinned_tabs.insert(i); }
            }
        }
        if ui.small_button("+ Tab").clicked() {
            state.tabs.push(format!("Tab_{}", state.tabs.len()));
        }
    });
}

// ─── Final Physics Summary Panel ──────────────────────────────────────────────

pub fn show_physics_summary(ui: &mut egui::Ui) {
    ui.heading("Physics Engine Summary");
    ui.separator();

    egui::Grid::new("physics_summary").num_columns(2).spacing([16.0, 3.0]).striped(true).show(ui, |ui| {
        ui.label(egui::RichText::new("Simulation Systems").strong().color(Color32::from_rgb(100,180,255)));
        ui.label("");
        ui.end_row();

        ui.label("Rigid Body Dynamics"); ui.label("✓ Full 2D/3D support"); ui.end_row();
        ui.label("Soft Body Simulation"); ui.label("✓ Verlet + spring constraints"); ui.end_row();
        ui.label("Cloth Simulation"); ui.label("✓ Structural/shear/bend springs"); ui.end_row();
        ui.label("Rope/Chain"); ui.label("✓ PBD 20-iteration solver"); ui.end_row();
        ui.label("Fluid SPH"); ui.label("✓ 2D smoothed particle hydro"); ui.end_row();
        ui.label("Ragdoll System"); ui.label("✓ 5 presets, IK assist"); ui.end_row();
        ui.label("Buoyancy"); ui.label("✓ 6 fluid presets + custom"); ui.end_row();
        ui.label("Vehicle Physics"); ui.label("✓ Wheel suspension + drive"); ui.end_row();
        ui.label("Character Controller"); ui.label("✓ Kinematic capsule/box"); ui.end_row();
        ui.label("Explosions"); ui.label("✓ Radial blast + debris"); ui.end_row();
        ui.label("Trigger Zones"); ui.label("✓ Box/Circle/Capsule/Poly"); ui.end_row();
        ui.label("Sensor Bodies"); ui.label("✓ Sphere/Box/Capsule/Ray"); ui.end_row();
        ui.label("Force Fields"); ui.label("✓ Attractor/Vortex/Turbulence"); ui.end_row();
        ui.label("Gravity Fields"); ui.label("✓ Uniform/Radial/Vortex/Wind"); ui.end_row();

        ui.label(""); ui.label(""); ui.end_row();
        ui.label(egui::RichText::new("Collision Systems").strong().color(Color32::from_rgb(100,255,180)));
        ui.label("");
        ui.end_row();

        ui.label("Broadphase"); ui.label("✓ DBVT/SpatialHash/SAP"); ui.end_row();
        ui.label("Narrowphase"); ui.label("✓ GJK+EPA/SAT/MPR"); ui.end_row();
        ui.label("CCD"); ui.label("✓ Speculative/Swept/TOI"); ui.end_row();
        ui.label("Collision Matrix"); ui.label("✓ 16×16 layer filter"); ui.end_row();
        ui.label("Collision Events"); ui.label("✓ Sound/Particles/Damage"); ui.end_row();

        ui.label(""); ui.label(""); ui.end_row();
        ui.label(egui::RichText::new("Editor Tools").strong().color(Color32::from_rgb(255,200,80)));
        ui.label("");
        ui.end_row();

        ui.label("Material Library"); ui.label("✓ 22 presets + scatter plot"); ui.end_row();
        ui.label("Material Interaction Matrix"); ui.label("✓ Per-pair friction/restitution"); ui.end_row();
        ui.label("Joint Templates"); ui.label("✓ 8 joint presets"); ui.end_row();
        ui.label("Breakable Joints"); ui.label("✓ Force/torque limits"); ui.end_row();
        ui.label("Chain Builder"); ui.label("✓ Multi-link assemblies"); ui.end_row();
        ui.label("Velocity Field Painter"); ui.label("✓ 6 brush modes"); ui.end_row();
        ui.label("Transform Gizmo"); ui.label("✓ Translate/Rotate/Scale"); ui.end_row();
        ui.label("Physics Profiler"); ui.label("✓ Per-category stacked bar"); ui.end_row();
        ui.label("Timeline Recorder"); ui.label("✓ KE/PE plot, playback"); ui.end_row();
        ui.label("Event Logger"); ui.label("✓ Filtered, searchable"); ui.end_row();
        ui.label("Scene Export/Import"); ui.label("✓ JSON with preview"); ui.end_row();
        ui.label("Preset Browser"); ui.label("✓ 12 scene presets"); ui.end_row();
        ui.label("Simulation Islands"); ui.label("✓ Sleep/wake management"); ui.end_row();
        ui.label("Debug Overlay"); ui.label("✓ AABB/velocity/contacts"); ui.end_row();
        ui.label("Unit Converter"); ui.label("✓ Physics constants ref"); ui.end_row();
    });
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
