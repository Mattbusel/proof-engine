import os
path = r"C:\proof-engine\editor\src\physics_editor.rs"

EXPANSION = r"""
// ============================================================
// ADVANCED PHYSICS EDITOR — EXPANSION BLOCK 4
// ============================================================

// ─── Constraint Solver Settings ───────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct ConstraintSolverSettings {
    pub solver_type: SolverType,
    pub velocity_iterations: u32,
    pub position_iterations: u32,
    pub warm_starting: bool,
    pub split_impulse: bool,
    pub erp: f32,
    pub cfm: f32,
    pub contact_baumgarte: f32,
    pub contact_penetration_threshold: f32,
    pub joint_baumgarte: f32,
    pub max_linear_correction: f32,
    pub max_angular_correction: f32,
    pub linear_slop: f32,
    pub angular_slop: f32,
    pub time_to_sleep: f32,
    pub linear_sleep_tolerance: f32,
    pub angular_sleep_tolerance: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum SolverType {
    SequentialImpulse,
    ProjectedGaussSeidel,
    Dantzig,
    MLCP,
    XPBD,
}

impl SolverType {
    pub fn label(&self) -> &str {
        match self {
            SolverType::SequentialImpulse => "Sequential Impulse (SI)",
            SolverType::ProjectedGaussSeidel => "Projected Gauss-Seidel",
            SolverType::Dantzig => "Dantzig (LCP)",
            SolverType::MLCP => "MLCP",
            SolverType::XPBD => "XPBD",
        }
    }
    pub fn description(&self) -> &str {
        match self {
            SolverType::SequentialImpulse => "Fast iterative solver. Best for most real-time use cases.",
            SolverType::ProjectedGaussSeidel => "More stable for stiff chains/stacks, slower per iter.",
            SolverType::Dantzig => "Exact solver for small systems. Not scalable.",
            SolverType::MLCP => "Mixed LCP with complementarity constraints. High quality.",
            SolverType::XPBD => "Extended PBD. Stable, substep-friendly, good for soft bodies.",
        }
    }
}

impl Default for ConstraintSolverSettings {
    fn default() -> Self {
        Self {
            solver_type: SolverType::SequentialImpulse,
            velocity_iterations: 10,
            position_iterations: 4,
            warm_starting: true,
            split_impulse: true,
            erp: 0.2,
            cfm: 0.0,
            contact_baumgarte: 0.2,
            contact_penetration_threshold: 0.005,
            joint_baumgarte: 0.4,
            max_linear_correction: 0.2,
            max_angular_correction: 0.34,
            linear_slop: 0.005,
            angular_slop: 0.0035,
            time_to_sleep: 0.5,
            linear_sleep_tolerance: 0.01,
            angular_sleep_tolerance: 0.01,
        }
    }
}

pub fn show_constraint_solver_settings(ui: &mut egui::Ui, settings: &mut ConstraintSolverSettings) {
    ui.heading("Constraint Solver Settings");
    ui.separator();

    ui.label("Solver Type:");
    for st in &[SolverType::SequentialImpulse, SolverType::ProjectedGaussSeidel, SolverType::XPBD, SolverType::MLCP] {
        let sel = settings.solver_type == *st;
        if ui.selectable_label(sel, st.label()).clicked() { settings.solver_type = st.clone(); }
    }
    ui.label(egui::RichText::new(settings.solver_type.description()).small().color(Color32::GRAY));
    ui.separator();

    ui.columns(2, |cols| {
        cols[0].label("Velocity Iterations:");
        cols[0].add(egui::DragValue::new(&mut settings.velocity_iterations).clamp_range(1..=40));
        cols[0].label("Position Iterations:");
        cols[0].add(egui::DragValue::new(&mut settings.position_iterations).clamp_range(0..=20));
        cols[0].checkbox(&mut settings.warm_starting, "Warm Starting");
        cols[0].checkbox(&mut settings.split_impulse, "Split Impulse");

        cols[1].label("ERP (Error Reduction):");
        cols[1].add(egui::Slider::new(&mut settings.erp, 0.0..=1.0));
        cols[1].label("CFM (Constraint Force Mix):");
        cols[1].add(egui::Slider::new(&mut settings.cfm, 0.0..=0.1));
        cols[1].label("Contact Baumgarte:");
        cols[1].add(egui::Slider::new(&mut settings.contact_baumgarte, 0.0..=1.0));
        cols[1].label("Joint Baumgarte:");
        cols[1].add(egui::Slider::new(&mut settings.joint_baumgarte, 0.0..=1.0));
    });

    ui.separator();
    ui.label("Sleep Settings:");
    ui.horizontal(|ui| {
        ui.label("Time to Sleep:");
        ui.add(egui::DragValue::new(&mut settings.time_to_sleep).speed(0.05).clamp_range(0.0..=5.0).suffix("s"));
        ui.label("Lin Tol:");
        ui.add(egui::DragValue::new(&mut settings.linear_sleep_tolerance).speed(0.001).clamp_range(0.0001..=0.1));
        ui.label("Ang Tol:");
        ui.add(egui::DragValue::new(&mut settings.angular_sleep_tolerance).speed(0.001).clamp_range(0.0001..=0.1));
    });

    ui.separator();
    ui.label("Slop & Correction:");
    ui.horizontal(|ui| {
        ui.label("Pen. Threshold:");
        ui.add(egui::DragValue::new(&mut settings.contact_penetration_threshold).speed(0.0005).clamp_range(0.0001..=0.05));
        ui.label("Lin Slop:");
        ui.add(egui::DragValue::new(&mut settings.linear_slop).speed(0.0005));
        ui.label("Ang Slop:");
        ui.add(egui::DragValue::new(&mut settings.angular_slop).speed(0.0001));
    });
    ui.horizontal(|ui| {
        ui.label("Max Lin Correction:");
        ui.add(egui::DragValue::new(&mut settings.max_linear_correction).speed(0.01));
        ui.label("Max Ang Correction:");
        ui.add(egui::DragValue::new(&mut settings.max_angular_correction).speed(0.01));
    });

    draw_solver_convergence_graph(ui, settings);
}

fn draw_solver_convergence_graph(ui: &mut egui::Ui, settings: &ConstraintSolverSettings) {
    let desired = Vec2::new(ui.available_width().min(480.0), 90.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(12, 14, 20));
    p.text(Pos2::new(rect.left()+4.0, rect.top()+3.0), egui::Align2::LEFT_TOP, "Simulated Convergence (Velocity Error per Iteration)", FontId::monospace(8.0), Color32::GRAY);

    let iters = settings.velocity_iterations as usize;
    let warm = if settings.warm_starting { 0.5 } else { 1.0 };
    for i in 1..=iters {
        let t = i as f32;
        let err = warm * (1.0 - settings.erp).powf(t) * 1.5;
        let x0 = rect.left() + (i-1) as f32 / iters as f32 * rect.width();
        let x1 = rect.left() + i as f32 / iters as f32 * rect.width();
        let warm_prev = warm * (1.0 - settings.erp).powf(t - 1.0) * 1.5;
        let y0 = rect.bottom() - (warm_prev.clamp(0.0, 1.5) / 1.5) * (rect.height() - 16.0);
        let y1 = rect.bottom() - (err.clamp(0.0, 1.5) / 1.5) * (rect.height() - 16.0);
        p.line_segment([Pos2::new(x0, y0), Pos2::new(x1, y1)], Stroke::new(1.5, Color32::from_rgb(80, 200, 120)));
    }
    p.text(Pos2::new(rect.right()-4.0, rect.bottom()-2.0), egui::Align2::RIGHT_BOTTOM, format!("{} iters", iters), FontId::monospace(7.0), Color32::DARK_GRAY);
}

// ─── Sensor Body System ───────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct SensorBody {
    pub id: usize,
    pub name: String,
    pub position: [f32; 2],
    pub sensor_type: SensorType,
    pub radius: f32,
    pub active: bool,
    pub overlapping_bodies: Vec<usize>,
    pub total_trigger_count: u32,
    pub on_enter_event: String,
    pub on_exit_event: String,
    pub layer_mask: u32,
    pub one_way: bool,
    pub one_way_normal: [f32; 2],
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum SensorType {
    Sphere,
    Box { half_extents: [f32; 2] },
    Capsule { radius: f32, half_height: f32 },
    ConvexHull,
    Ray { direction: [f32; 2], max_distance: f32 },
}

impl SensorType {
    pub fn label(&self) -> &str {
        match self {
            SensorType::Sphere => "Sphere",
            SensorType::Box { .. } => "Box",
            SensorType::Capsule { .. } => "Capsule",
            SensorType::ConvexHull => "Convex Hull",
            SensorType::Ray { .. } => "Ray",
        }
    }
}

impl Default for SensorType { fn default() -> Self { SensorType::Sphere } }

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SensorBodyEditorState {
    pub sensors: Vec<SensorBody>,
    pub selected: Option<usize>,
    pub show_sensor_ranges: bool,
    pub simulation_time: f32,
}

pub fn show_sensor_body_editor(ui: &mut egui::Ui, state: &mut SensorBodyEditorState) {
    ui.heading("Sensor Body Editor");
    ui.separator();

    if state.sensors.is_empty() {
        state.sensors = vec![
            SensorBody { id: 0, name: "Proximity_Sensor".into(), position: [0.0, 0.0], sensor_type: SensorType::Sphere, radius: 3.0, active: true, overlapping_bodies: vec![], total_trigger_count: 0, on_enter_event: "on_proximity_enter".into(), on_exit_event: "on_proximity_exit".into(), layer_mask: 0xFF, one_way: false, one_way_normal: [0.0, 1.0] },
            SensorBody { id: 1, name: "Door_Trigger".into(), position: [5.0, 0.0], sensor_type: SensorType::Box { half_extents: [1.0, 2.0] }, radius: 1.0, active: true, overlapping_bodies: vec![], total_trigger_count: 0, on_enter_event: "open_door".into(), on_exit_event: "close_door".into(), layer_mask: 0x01, one_way: false, one_way_normal: [0.0, 1.0] },
        ];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Sphere Sensor").clicked() {
            let n = state.sensors.len();
            state.sensors.push(SensorBody { id: n, name: format!("Sensor_{}", n), position: [0.0, 0.0], sensor_type: SensorType::Sphere, radius: 2.0, active: true, overlapping_bodies: vec![], total_trigger_count: 0, on_enter_event: "on_enter".into(), on_exit_event: "on_exit".into(), layer_mask: 0xFF, one_way: false, one_way_normal: [0.0, 1.0] });
        }
        if ui.button("+ Box Sensor").clicked() {
            let n = state.sensors.len();
            state.sensors.push(SensorBody { id: n, name: format!("BoxSensor_{}", n), position: [0.0, 0.0], sensor_type: SensorType::Box { half_extents: [1.5, 1.5] }, radius: 1.5, active: true, overlapping_bodies: vec![], total_trigger_count: 0, on_enter_event: "on_enter".into(), on_exit_event: "on_exit".into(), layer_mask: 0xFF, one_way: false, one_way_normal: [0.0, 1.0] });
        }
        if ui.button("+ Ray Sensor").clicked() {
            let n = state.sensors.len();
            state.sensors.push(SensorBody { id: n, name: format!("Ray_{}", n), position: [0.0, 0.0], sensor_type: SensorType::Ray { direction: [1.0, 0.0], max_distance: 10.0 }, radius: 0.1, active: true, overlapping_bodies: vec![], total_trigger_count: 0, on_enter_event: "on_ray_hit".into(), on_exit_event: "".into(), layer_mask: 0xFF, one_way: false, one_way_normal: [0.0, 1.0] });
        }
        ui.checkbox(&mut state.show_sensor_ranges, "Show Ranges");
    });

    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
        for i in 0..state.sensors.len() {
            let s = &state.sensors[i];
            let sel = state.selected == Some(i);
            let active_col = if s.active { Color32::from_rgb(80, 200, 120) } else { Color32::DARK_GRAY };
            ui.horizontal(|ui| {
                ui.colored_label(active_col, "◉");
                if ui.selectable_label(sel, format!("{} [{}]", s.name, s.sensor_type.label())).clicked() {
                    state.selected = Some(i);
                }
                ui.label(format!("overlaps: {} | fired: {}", s.overlapping_bodies.len(), s.total_trigger_count));
            });
        }
    });

    if let Some(idx) = state.selected {
        if idx < state.sensors.len() {
            let s = &mut state.sensors[idx];
            ui.separator();
            ui.group(|ui| {
                ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut s.name); ui.checkbox(&mut s.active, "Active"); });
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    ui.add(egui::DragValue::new(&mut s.position[0]).prefix("x:").speed(0.05));
                    ui.add(egui::DragValue::new(&mut s.position[1]).prefix("y:").speed(0.05));
                });
                match &mut s.sensor_type {
                    SensorType::Sphere => {
                        ui.horizontal(|ui| { ui.label("Radius:"); ui.add(egui::DragValue::new(&mut s.radius).speed(0.05).clamp_range(0.1..=50.0)); });
                    }
                    SensorType::Box { half_extents } => {
                        ui.horizontal(|ui| {
                            ui.label("Half Extents:");
                            ui.add(egui::DragValue::new(&mut half_extents[0]).prefix("w:").speed(0.05).clamp_range(0.1..=50.0));
                            ui.add(egui::DragValue::new(&mut half_extents[1]).prefix("h:").speed(0.05).clamp_range(0.1..=50.0));
                        });
                    }
                    SensorType::Capsule { radius, half_height } => {
                        ui.horizontal(|ui| {
                            ui.label("Radius:"); ui.add(egui::DragValue::new(radius).speed(0.05).clamp_range(0.1..=20.0));
                            ui.label("Half H:"); ui.add(egui::DragValue::new(half_height).speed(0.05).clamp_range(0.1..=20.0));
                        });
                    }
                    SensorType::Ray { direction, max_distance } => {
                        ui.horizontal(|ui| {
                            ui.label("Direction:");
                            ui.add(egui::DragValue::new(&mut direction[0]).prefix("dx:").speed(0.01));
                            ui.add(egui::DragValue::new(&mut direction[1]).prefix("dy:").speed(0.01));
                            ui.label("Max Dist:");
                            ui.add(egui::DragValue::new(max_distance).speed(0.1).clamp_range(0.1..=1000.0));
                        });
                    }
                    _ => {}
                }
                ui.horizontal(|ui| {
                    ui.label("On Enter Event:"); ui.text_edit_singleline(&mut s.on_enter_event);
                });
                ui.horizontal(|ui| {
                    ui.label("On Exit Event:"); ui.text_edit_singleline(&mut s.on_exit_event);
                });
                ui.horizontal(|ui| {
                    ui.label("Layer Mask:"); ui.add(egui::DragValue::new(&mut s.layer_mask).hexadecimal(4, false, true));
                    ui.checkbox(&mut s.one_way, "One-Way");
                });
                if s.one_way {
                    ui.horizontal(|ui| {
                        ui.label("One-Way Normal:");
                        ui.add(egui::DragValue::new(&mut s.one_way_normal[0]).prefix("nx:").speed(0.01));
                        ui.add(egui::DragValue::new(&mut s.one_way_normal[1]).prefix("ny:").speed(0.01));
                    });
                }
                if ui.button("Simulate Enter (Body 0)").clicked() {
                    if !s.overlapping_bodies.contains(&0) { s.overlapping_bodies.push(0); s.total_trigger_count += 1; }
                }
                if ui.button("Simulate Exit (Body 0)").clicked() { s.overlapping_bodies.retain(|&b| b != 0); }
            });
        }
    }
}

// ─── Rigid Body Properties Panel ─────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct RigidBodyProperties {
    pub id: usize,
    pub name: String,
    pub body_type: RigidBodyType,
    pub mass: f32,
    pub inertia: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    pub position: [f32; 2],
    pub rotation: f32,
    pub linear_velocity: [f32; 2],
    pub angular_velocity: f32,
    pub is_bullet: bool,
    pub fixed_rotation: bool,
    pub wake_on_collision: bool,
    pub collider_type: ColliderShape,
    pub material_idx: usize,
    pub layer: u8,
    pub mask: u32,
    pub custom_mass_override: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum RigidBodyType {
    Dynamic,
    Static,
    Kinematic,
}

impl RigidBodyType {
    pub fn label(&self) -> &str {
        match self {
            RigidBodyType::Dynamic => "Dynamic",
            RigidBodyType::Static => "Static",
            RigidBodyType::Kinematic => "Kinematic",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            RigidBodyType::Dynamic => Color32::from_rgb(80, 160, 255),
            RigidBodyType::Static => Color32::from_rgb(180, 180, 180),
            RigidBodyType::Kinematic => Color32::from_rgb(255, 180, 60),
        }
    }
}

impl Default for RigidBodyType { fn default() -> Self { RigidBodyType::Dynamic } }

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ColliderShape {
    Box { hw: f32, hh: f32 },
    Circle { r: f32 },
    Capsule { r: f32, hh: f32 },
    Polygon,
    Compound,
}

impl ColliderShape {
    pub fn label(&self) -> &str {
        match self {
            ColliderShape::Box { .. } => "Box",
            ColliderShape::Circle { .. } => "Circle",
            ColliderShape::Capsule { .. } => "Capsule",
            ColliderShape::Polygon => "Polygon",
            ColliderShape::Compound => "Compound",
        }
    }
}

impl Default for ColliderShape { fn default() -> Self { ColliderShape::Box { hw: 0.5, hh: 0.5 } } }

impl Default for RigidBodyProperties {
    fn default() -> Self {
        Self { id: 0, name: "Body_0".into(), body_type: RigidBodyType::Dynamic, mass: 1.0, inertia: 0.5, linear_damping: 0.0, angular_damping: 0.01, gravity_scale: 1.0, position: [0.0, 0.0], rotation: 0.0, linear_velocity: [0.0, 0.0], angular_velocity: 0.0, is_bullet: false, fixed_rotation: false, wake_on_collision: true, collider_type: ColliderShape::Box { hw: 0.5, hh: 0.5 }, material_idx: 0, layer: 0, mask: 0xFFFF, custom_mass_override: false }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct RigidBodyListState {
    pub bodies: Vec<RigidBodyProperties>,
    pub selected: Option<usize>,
    pub filter_type: Option<RigidBodyType>,
    pub search: String,
    pub sort_mode: BodySortMode,
    pub show_advanced: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum BodySortMode {
    #[default]
    ById,
    ByName,
    ByMass,
    ByType,
}

pub fn show_rigid_body_list(ui: &mut egui::Ui, state: &mut RigidBodyListState) {
    ui.heading("Rigid Bodies");
    ui.separator();

    if state.bodies.is_empty() {
        let mut b0 = RigidBodyProperties::default(); b0.id = 0; b0.name = "Ground".into(); b0.body_type = RigidBodyType::Static; b0.mass = 0.0; b0.position = [0.0, -5.0]; b0.collider_type = ColliderShape::Box { hw: 20.0, hh: 0.5 };
        let mut b1 = RigidBodyProperties::default(); b1.id = 1; b1.name = "Box_1".into(); b1.mass = 2.0; b1.position = [0.0, 2.0];
        let mut b2 = RigidBodyProperties::default(); b2.id = 2; b2.name = "Ball_1".into(); b2.mass = 1.0; b2.position = [2.0, 4.0]; b2.collider_type = ColliderShape::Circle { r: 0.5 };
        let mut b3 = RigidBodyProperties::default(); b3.id = 3; b3.name = "Mover".into(); b3.body_type = RigidBodyType::Kinematic; b3.position = [-3.0, 0.0];
        state.bodies = vec![b0, b1, b2, b3];
    }

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search);
        ui.label("Filter:");
        if ui.selectable_label(state.filter_type.is_none(), "All").clicked() { state.filter_type = None; }
        for bt in &[RigidBodyType::Dynamic, RigidBodyType::Static, RigidBodyType::Kinematic] {
            let sel = state.filter_type.as_ref() == Some(bt);
            if ui.selectable_label(sel, egui::RichText::new(bt.label()).color(bt.color())).clicked() {
                state.filter_type = Some(bt.clone());
            }
        }
    });

    ui.horizontal(|ui| {
        if ui.button("+ Dynamic").clicked() {
            let n = state.bodies.len();
            let mut b = RigidBodyProperties::default(); b.id = n; b.name = format!("Body_{}", n);
            state.bodies.push(b);
            state.selected = Some(n);
        }
        if ui.button("+ Static").clicked() {
            let n = state.bodies.len();
            let mut b = RigidBodyProperties::default(); b.id = n; b.name = format!("Static_{}", n); b.body_type = RigidBodyType::Static; b.mass = 0.0;
            state.bodies.push(b);
            state.selected = Some(n);
        }
        if ui.button("+ Kinematic").clicked() {
            let n = state.bodies.len();
            let mut b = RigidBodyProperties::default(); b.id = n; b.name = format!("Kinematic_{}", n); b.body_type = RigidBodyType::Kinematic;
            state.bodies.push(b);
            state.selected = Some(n);
        }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.bodies.remove(s); state.selected = None; } }
        if ui.button("Duplicate").clicked() {
            if let Some(s) = state.selected {
                if s < state.bodies.len() {
                    let mut clone = state.bodies[s].clone();
                    clone.id = state.bodies.len();
                    clone.name = format!("{}_copy", clone.name);
                    clone.position[0] += 0.5;
                    state.bodies.push(clone);
                    state.selected = Some(state.bodies.len()-1);
                }
            }
        }
        ui.checkbox(&mut state.show_advanced, "Advanced");
    });

    let indices: Vec<usize> = state.bodies.iter().enumerate()
        .filter(|(_, b)| {
            let type_ok = state.filter_type.as_ref().map_or(true, |t| &b.body_type == t);
            let search_ok = state.search.is_empty() || b.name.to_lowercase().contains(&state.search.to_lowercase());
            type_ok && search_ok
        })
        .map(|(i, _)| i)
        .collect();

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        egui::Grid::new("body_list_grid").num_columns(5).spacing([8.0, 2.0]).striped(true).show(ui, |ui| {
            ui.label("ID"); ui.label("Name"); ui.label("Type"); ui.label("Mass"); ui.label("Pos"); ui.end_row();
            for &i in &indices {
                let b = &state.bodies[i];
                let sel = state.selected == Some(i);
                let tc = b.body_type.color();
                ui.colored_label(if sel { Color32::WHITE } else { Color32::GRAY }, format!("{}", b.id));
                if ui.selectable_label(sel, &b.name).clicked() { state.selected = Some(i); }
                ui.colored_label(tc, b.body_type.label());
                ui.label(format!("{:.2}", b.mass));
                ui.label(format!("({:.1},{:.1})", b.position[0], b.position[1]));
                ui.end_row();
            }
        });
    });

    if let Some(idx) = state.selected {
        if idx < state.bodies.len() {
            let b = &mut state.bodies[idx];
            ui.separator();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut b.name);
                    for bt in &[RigidBodyType::Dynamic, RigidBodyType::Static, RigidBodyType::Kinematic] {
                        let sel = b.body_type == *bt;
                        if ui.selectable_label(sel, egui::RichText::new(bt.label()).color(bt.color())).clicked() { b.body_type = bt.clone(); }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    ui.add(egui::DragValue::new(&mut b.position[0]).prefix("x:").speed(0.05));
                    ui.add(egui::DragValue::new(&mut b.position[1]).prefix("y:").speed(0.05));
                    ui.label("Rotation:");
                    ui.add(egui::DragValue::new(&mut b.rotation).speed(0.5).suffix("°"));
                });
                ui.horizontal(|ui| {
                    ui.label("Mass:");
                    ui.add(egui::DragValue::new(&mut b.mass).speed(0.01).clamp_range(0.0..=10000.0));
                    ui.label("Inertia:");
                    ui.add(egui::DragValue::new(&mut b.inertia).speed(0.01).clamp_range(0.0..=10000.0));
                    ui.checkbox(&mut b.custom_mass_override, "Custom");
                });
                ui.horizontal(|ui| {
                    ui.label("Lin Damp:");
                    ui.add(egui::Slider::new(&mut b.linear_damping, 0.0..=1.0));
                    ui.label("Ang Damp:");
                    ui.add(egui::Slider::new(&mut b.angular_damping, 0.0..=1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Grav Scale:");
                    ui.add(egui::DragValue::new(&mut b.gravity_scale).speed(0.05));
                    ui.checkbox(&mut b.is_bullet, "Bullet (CCD)");
                    ui.checkbox(&mut b.fixed_rotation, "Fixed Rotation");
                });
                if state.show_advanced {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Lin Vel:");
                        ui.add(egui::DragValue::new(&mut b.linear_velocity[0]).prefix("vx:").speed(0.05));
                        ui.add(egui::DragValue::new(&mut b.linear_velocity[1]).prefix("vy:").speed(0.05));
                        ui.label("Ang Vel:");
                        ui.add(egui::DragValue::new(&mut b.angular_velocity).speed(0.05));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Layer:");
                        ui.add(egui::DragValue::new(&mut b.layer).clamp_range(0..=15));
                        ui.label("Mask:");
                        ui.add(egui::DragValue::new(&mut b.mask).hexadecimal(4, false, true));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Collider:");
                        for cs in &[ColliderShape::Box { hw: 0.5, hh: 0.5 }, ColliderShape::Circle { r: 0.5 }, ColliderShape::Capsule { r: 0.3, hh: 0.5 }, ColliderShape::Polygon] {
                            let sel = std::mem::discriminant(&b.collider_type) == std::mem::discriminant(cs);
                            if ui.selectable_label(sel, cs.label()).clicked() { b.collider_type = cs.clone(); }
                        }
                    });
                }
            });
        }
    }
}

// ─── Physics Event Logger ─────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicsLogEntry {
    pub frame: u64,
    pub time: f32,
    pub category: LogCategory,
    pub message: String,
    pub body_id: Option<usize>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum LogCategory {
    Collision,
    Sleep,
    Wake,
    JointBreak,
    TriggerEnter,
    TriggerExit,
    Error,
    Warning,
    Info,
}

impl LogCategory {
    pub fn label(&self) -> &str {
        match self {
            LogCategory::Collision => "COLLISION",
            LogCategory::Sleep => "SLEEP",
            LogCategory::Wake => "WAKE",
            LogCategory::JointBreak => "JOINT_BREAK",
            LogCategory::TriggerEnter => "TRIGGER_ENTER",
            LogCategory::TriggerExit => "TRIGGER_EXIT",
            LogCategory::Error => "ERROR",
            LogCategory::Warning => "WARNING",
            LogCategory::Info => "INFO",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            LogCategory::Collision => Color32::from_rgb(240, 120, 60),
            LogCategory::Sleep => Color32::from_rgb(100, 100, 200),
            LogCategory::Wake => Color32::from_rgb(100, 200, 255),
            LogCategory::JointBreak => Color32::from_rgb(240, 60, 60),
            LogCategory::TriggerEnter => Color32::from_rgb(80, 220, 120),
            LogCategory::TriggerExit => Color32::from_rgb(180, 100, 60),
            LogCategory::Error => Color32::RED,
            LogCategory::Warning => Color32::YELLOW,
            LogCategory::Info => Color32::GRAY,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsEventLoggerState {
    pub entries: Vec<PhysicsLogEntry>,
    pub max_entries: usize,
    pub paused: bool,
    pub filter: Option<LogCategory>,
    pub search_text: String,
    pub auto_scroll: bool,
    pub frame_counter: u64,
    pub sim_time: f32,
}

pub fn show_physics_event_logger(ui: &mut egui::Ui, state: &mut PhysicsEventLoggerState) {
    ui.heading("Physics Event Logger");
    ui.separator();

    if state.max_entries == 0 { state.max_entries = 1000; state.auto_scroll = true; }

    if state.entries.is_empty() {
        let sample_entries = vec![
            (LogCategory::Info, "Simulation initialized", None),
            (LogCategory::Collision, "Body#1 vs Body#2 — normal impact v=3.2", Some(1)),
            (LogCategory::Sleep, "Body#3 went to sleep (KE < threshold)", Some(3)),
            (LogCategory::TriggerEnter, "Body#0 entered Zone 'Checkpoint_1'", Some(0)),
            (LogCategory::Wake, "Body#3 woken by collision from Body#1", Some(3)),
            (LogCategory::JointBreak, "Joint#2 broke — force 452.3 > limit 400.0", None),
            (LogCategory::Warning, "Detected tunnel on Body#4 (CCD disabled)", Some(4)),
            (LogCategory::Collision, "Body#2 vs Ground — restitution 0.75", Some(2)),
        ];
        for (i, (cat, msg, bid)) in sample_entries.into_iter().enumerate() {
            state.entries.push(PhysicsLogEntry { frame: i as u64, time: i as f32 * 0.016, category: cat, message: msg.into(), body_id: bid });
        }
    }

    ui.horizontal(|ui| {
        let pause_lbl = if state.paused { "▶ Resume" } else { "⏸ Pause" };
        if ui.button(pause_lbl).clicked() { state.paused = !state.paused; }
        if ui.button("Clear").clicked() { state.entries.clear(); }
        ui.checkbox(&mut state.auto_scroll, "Auto Scroll");
        ui.label("Filter:");
        if ui.selectable_label(state.filter.is_none(), "All").clicked() { state.filter = None; }
        for cat in &[LogCategory::Collision, LogCategory::Sleep, LogCategory::Wake, LogCategory::JointBreak, LogCategory::TriggerEnter, LogCategory::Error, LogCategory::Warning] {
            let sel = state.filter.as_ref() == Some(cat);
            if ui.selectable_label(sel, egui::RichText::new(cat.label()).color(cat.color()).small()).clicked() {
                state.filter = if sel { None } else { Some(cat.clone()) };
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.search_text);
        if ui.button("✗").clicked() { state.search_text.clear(); }
        ui.label(format!("Entries: {}/{}", state.entries.len(), state.max_entries));
    });

    egui::ScrollArea::vertical().max_height(280.0).auto_shrink([false, false]).show(ui, |ui| {
        for entry in state.entries.iter().rev() {
            let cat_ok = state.filter.as_ref().map_or(true, |f| f == &entry.category);
            let search_ok = state.search_text.is_empty() || entry.message.to_lowercase().contains(&state.search_text.to_lowercase());
            if !cat_ok || !search_ok { continue; }
            let col = entry.category.color();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("[{:.3}]", entry.time)).monospace().color(Color32::DARK_GRAY).small());
                ui.colored_label(col, format!("[{}]", entry.category.label()));
                if let Some(bid) = entry.body_id { ui.label(egui::RichText::new(format!("B#{}", bid)).color(Color32::from_rgb(150,150,220)).small()); }
                ui.label(egui::RichText::new(&entry.message).small());
            });
        }
    });

    // Simulate adding entries
    if !state.paused && ui.ctx().input(|i| i.time as u64) % 60 == 0 {
        // Would add new entries in real simulation
    }
}

// ─── Broadphase Settings ──────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct BroadphaseSettings {
    pub algorithm: BroadphaseAlgorithm,
    pub aabb_margin: f32,
    pub update_frequency: u32,
    pub spatial_hash_cell_size: f32,
    pub dbvt_fatness_factor: f32,
    pub sap_axis: SapAxis,
    pub multithread_broadphase: bool,
    pub debug_draw: bool,
    pub last_pair_count: usize,
    pub last_query_time_us: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum BroadphaseAlgorithm {
    SpatialHash,
    DBVT,
    SAP,
    Brute,
}

impl BroadphaseAlgorithm {
    pub fn label(&self) -> &str {
        match self {
            BroadphaseAlgorithm::SpatialHash => "Spatial Hash",
            BroadphaseAlgorithm::DBVT => "DBVT (Dynamic BVH)",
            BroadphaseAlgorithm::SAP => "Sweep and Prune",
            BroadphaseAlgorithm::Brute => "Brute Force (Debug)",
        }
    }
    pub fn description(&self) -> &str {
        match self {
            BroadphaseAlgorithm::SpatialHash => "O(n) average. Best for uniformly distributed objects.",
            BroadphaseAlgorithm::DBVT => "O(n log n). Excellent for varied object sizes and sparse scenes.",
            BroadphaseAlgorithm::SAP => "O(n + k). Excellent for slowly moving objects.",
            BroadphaseAlgorithm::Brute => "O(n²). Only for debugging with very small scenes.",
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SapAxis {
    #[default]
    X,
    Y,
    Auto,
}

impl Default for BroadphaseSettings {
    fn default() -> Self {
        Self { algorithm: BroadphaseAlgorithm::DBVT, aabb_margin: 0.1, update_frequency: 1, spatial_hash_cell_size: 2.0, dbvt_fatness_factor: 0.1, sap_axis: SapAxis::X, multithread_broadphase: false, debug_draw: false, last_pair_count: 0, last_query_time_us: 0.0 }
    }
}

pub fn show_broadphase_settings(ui: &mut egui::Ui, settings: &mut BroadphaseSettings) {
    ui.heading("Broadphase Settings");
    ui.separator();

    ui.label("Algorithm:");
    for algo in &[BroadphaseAlgorithm::DBVT, BroadphaseAlgorithm::SpatialHash, BroadphaseAlgorithm::SAP, BroadphaseAlgorithm::Brute] {
        let sel = settings.algorithm == *algo;
        if ui.selectable_label(sel, algo.label()).clicked() { settings.algorithm = algo.clone(); }
    }
    ui.label(egui::RichText::new(settings.algorithm.description()).small().color(Color32::GRAY));
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("AABB Margin:");
        ui.add(egui::DragValue::new(&mut settings.aabb_margin).speed(0.005).clamp_range(0.0..=1.0));
        ui.label("Update Freq:");
        ui.add(egui::DragValue::new(&mut settings.update_frequency).clamp_range(1..=4));
    });

    match settings.algorithm {
        BroadphaseAlgorithm::SpatialHash => {
            ui.horizontal(|ui| {
                ui.label("Cell Size:");
                ui.add(egui::DragValue::new(&mut settings.spatial_hash_cell_size).speed(0.05).clamp_range(0.1..=20.0));
            });
        }
        BroadphaseAlgorithm::DBVT => {
            ui.horizontal(|ui| {
                ui.label("Fatness Factor:");
                ui.add(egui::Slider::new(&mut settings.dbvt_fatness_factor, 0.0..=0.5));
            });
        }
        BroadphaseAlgorithm::SAP => {
            ui.horizontal(|ui| {
                ui.label("Sort Axis:");
                if ui.selectable_label(settings.sap_axis == SapAxis::X, "X").clicked() { settings.sap_axis = SapAxis::X; }
                if ui.selectable_label(settings.sap_axis == SapAxis::Y, "Y").clicked() { settings.sap_axis = SapAxis::Y; }
                if ui.selectable_label(settings.sap_axis == SapAxis::Auto, "Auto").clicked() { settings.sap_axis = SapAxis::Auto; }
            });
        }
        _ => {}
    }

    ui.horizontal(|ui| {
        ui.checkbox(&mut settings.multithread_broadphase, "Multithread Broadphase");
        ui.checkbox(&mut settings.debug_draw, "Debug Draw Cells");
    });

    ui.separator();
    ui.label("Performance Stats:");
    ui.horizontal(|ui| {
        ui.label(format!("Candidate Pairs: {}", settings.last_pair_count));
        ui.separator();
        ui.label(format!("Query Time: {:.1}μs", settings.last_query_time_us));
    });

    draw_broadphase_visualization(ui, settings);
}

fn draw_broadphase_visualization(ui: &mut egui::Ui, settings: &BroadphaseSettings) {
    let desired = Vec2::new(ui.available_width().min(480.0), 160.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));
    p.text(Pos2::new(rect.left()+4.0, rect.top()+4.0), egui::Align2::LEFT_TOP, format!("Broadphase: {}", settings.algorithm.label()), FontId::monospace(8.0), Color32::GRAY);

    let bodies: &[(f32, f32, f32)] = &[
        (0.2, 0.3, 0.06), (0.4, 0.5, 0.08), (0.6, 0.3, 0.05),
        (0.3, 0.7, 0.07), (0.7, 0.6, 0.09), (0.5, 0.8, 0.04),
        (0.1, 0.5, 0.05), (0.8, 0.2, 0.06), (0.5, 0.4, 0.08),
    ];

    match settings.algorithm {
        BroadphaseAlgorithm::SpatialHash => {
            let cell_px = settings.spatial_hash_cell_size * 40.0;
            let cols = (rect.width() / cell_px).ceil() as usize + 1;
            let rows = (rect.height() / cell_px).ceil() as usize + 1;
            for row in 0..rows {
                for col in 0..cols {
                    let cx = rect.left() + col as f32 * cell_px;
                    let cy = rect.top() + row as f32 * cell_px;
                    let cr = Rect::from_min_size(Pos2::new(cx, cy), Vec2::splat(cell_px));
                    p.rect_stroke(cr, 0.0, Stroke::new(0.5, Color32::from_rgb(35, 40, 50)));
                }
            }
        }
        BroadphaseAlgorithm::DBVT => {
            let bvh_rects: &[(f32, f32, f32, f32)] = &[
                (0.05, 0.1, 0.9, 0.9), (0.05, 0.1, 0.55, 0.9), (0.55, 0.1, 0.9, 0.9),
                (0.05, 0.1, 0.55, 0.55), (0.05, 0.55, 0.55, 0.9),
            ];
            for (i, &(x0, y0, x1, y1)) in bvh_rects.iter().enumerate() {
                let alpha = 40 + i as u8 * 20;
                p.rect_stroke(Rect::from_min_max(Pos2::new(rect.left() + x0*rect.width(), rect.top() + y0*rect.height()), Pos2::new(rect.left() + x1*rect.width(), rect.top() + y1*rect.height())), 2.0, Stroke::new(0.8, Color32::from_rgba_unmultiplied(80, 160, 255, alpha)));
            }
        }
        BroadphaseAlgorithm::SAP => {
            for &(x, _, r) in bodies {
                let px = rect.left() + x * rect.width();
                let pr = r * rect.width();
                p.line_segment([Pos2::new(px-pr, rect.bottom()-8.0), Pos2::new(px+pr, rect.bottom()-8.0)], Stroke::new(3.0, Color32::from_rgb(100, 200, 100)));
            }
            p.text(Pos2::new(rect.center().x, rect.bottom()-2.0), egui::Align2::CENTER_BOTTOM, "X-axis projection", FontId::monospace(7.0), Color32::DARK_GRAY);
        }
        _ => {}
    }

    for &(x, y, r) in bodies {
        let px = rect.left() + x * rect.width();
        let py = rect.top() + y * rect.height();
        let pr = r * rect.width();
        let margin = settings.aabb_margin * rect.width() * 0.2;
        p.rect_stroke(Rect::from_center_size(Pos2::new(px, py), Vec2::splat(pr*2.0+margin*2.0)), 1.0, Stroke::new(0.8, Color32::from_rgb(60, 120, 60)));
        p.circle_filled(Pos2::new(px, py), pr, Color32::from_rgb(80, 100, 160));
    }
}

// ─── Narrowphase Settings ─────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct NarrowphaseSettings {
    pub algorithm: NarrowphaseAlgorithm,
    pub max_contact_points: u32,
    pub persistent_contacts: bool,
    pub contact_cache_size: u32,
    pub speculative_contacts: bool,
    pub gjk_tolerance: f32,
    pub gjk_max_iterations: u32,
    pub epa_tolerance: f32,
    pub epa_max_iterations: u32,
    pub enable_convex_convex: bool,
    pub enable_concave_support: bool,
    pub stats_avg_contacts: f32,
    pub stats_gjk_iters: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum NarrowphaseAlgorithm {
    GjkEpa,
    Sat,
    Mpr,
    Hybrid,
}

impl NarrowphaseAlgorithm {
    pub fn label(&self) -> &str {
        match self {
            NarrowphaseAlgorithm::GjkEpa => "GJK + EPA",
            NarrowphaseAlgorithm::Sat => "SAT (2D only)",
            NarrowphaseAlgorithm::Mpr => "MPR",
            NarrowphaseAlgorithm::Hybrid => "Hybrid (SAT+GJK)",
        }
    }
}

impl Default for NarrowphaseSettings {
    fn default() -> Self {
        Self { algorithm: NarrowphaseAlgorithm::GjkEpa, max_contact_points: 4, persistent_contacts: true, contact_cache_size: 64, speculative_contacts: false, gjk_tolerance: 0.0001, gjk_max_iterations: 32, epa_tolerance: 0.001, epa_max_iterations: 64, enable_convex_convex: true, enable_concave_support: true, stats_avg_contacts: 0.0, stats_gjk_iters: 0.0 }
    }
}

pub fn show_narrowphase_settings(ui: &mut egui::Ui, settings: &mut NarrowphaseSettings) {
    ui.heading("Narrowphase Settings");
    ui.separator();

    ui.horizontal(|ui| {
        for algo in &[NarrowphaseAlgorithm::GjkEpa, NarrowphaseAlgorithm::Sat, NarrowphaseAlgorithm::Mpr, NarrowphaseAlgorithm::Hybrid] {
            if ui.selectable_label(settings.algorithm == *algo, algo.label()).clicked() { settings.algorithm = algo.clone(); }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Max Contacts:");
        ui.add(egui::DragValue::new(&mut settings.max_contact_points).clamp_range(1..=8));
        ui.checkbox(&mut settings.persistent_contacts, "Persistent Contacts");
        ui.label("Cache Size:");
        ui.add(egui::DragValue::new(&mut settings.contact_cache_size).clamp_range(16..=256));
    });
    ui.checkbox(&mut settings.speculative_contacts, "Speculative Contacts");
    ui.checkbox(&mut settings.enable_convex_convex, "Convex-Convex");
    ui.checkbox(&mut settings.enable_concave_support, "Concave Support");

    if matches!(settings.algorithm, NarrowphaseAlgorithm::GjkEpa | NarrowphaseAlgorithm::Hybrid) {
        ui.separator();
        ui.label("GJK Settings:");
        ui.horizontal(|ui| {
            ui.label("Tolerance:");
            ui.add(egui::DragValue::new(&mut settings.gjk_tolerance).speed(0.00001).clamp_range(0.00001..=0.01));
            ui.label("Max Iters:");
            ui.add(egui::DragValue::new(&mut settings.gjk_max_iterations).clamp_range(4..=128));
        });
        ui.label("EPA Settings:");
        ui.horizontal(|ui| {
            ui.label("Tolerance:");
            ui.add(egui::DragValue::new(&mut settings.epa_tolerance).speed(0.0001).clamp_range(0.0001..=0.01));
            ui.label("Max Iters:");
            ui.add(egui::DragValue::new(&mut settings.epa_max_iterations).clamp_range(8..=256));
        });
    }

    ui.separator();
    ui.label(format!("Avg Contacts/Pair: {:.2} | GJK Iters Avg: {:.1}", settings.stats_avg_contacts, settings.stats_gjk_iters));
}

// ─── Multi-Body Chain Builder ─────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize)]
pub struct ChainLink {
    pub length: f32,
    pub width: f32,
    pub mass: f32,
    pub angle_offset: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MultiBodyChain {
    pub name: String,
    pub links: Vec<ChainLink>,
    pub joint_type: JointType2D,
    pub root_fixed: bool,
    pub end_mass: Option<f32>,
    pub color: [f32; 3],
}

impl MultiBodyChain {
    pub fn default_pendulum() -> Self {
        Self { name: "Pendulum".into(), links: vec![ChainLink { length: 3.0, width: 0.15, mass: 0.5, angle_offset: 0.0 }], joint_type: JointType2D::Revolute, root_fixed: true, end_mass: Some(2.0), color: [0.6, 0.8, 1.0] }
    }
    pub fn default_double_pendulum() -> Self {
        Self { name: "Double Pendulum".into(), links: vec![ChainLink { length: 2.0, width: 0.12, mass: 0.5, angle_offset: 0.1 }, ChainLink { length: 2.0, width: 0.12, mass: 0.5, angle_offset: -0.15 }], joint_type: JointType2D::Revolute, root_fixed: true, end_mass: Some(1.5), color: [1.0, 0.7, 0.4] }
    }
    pub fn default_chain(n: usize) -> Self {
        let links = (0..n).map(|_| ChainLink { length: 0.5, width: 0.1, mass: 0.2, angle_offset: 0.0 }).collect();
        Self { name: format!("Chain_{}", n), links, joint_type: JointType2D::Revolute, root_fixed: true, end_mass: Some(3.0), color: [0.7, 0.7, 0.8] }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct MultiBodyChainState {
    pub chains: Vec<MultiBodyChain>,
    pub selected: Option<usize>,
    pub preview_angle: f32,
}

pub fn show_multi_body_chain_builder(ui: &mut egui::Ui, state: &mut MultiBodyChainState) {
    ui.heading("Multi-Body Chain Builder");
    ui.separator();

    if state.chains.is_empty() {
        state.chains = vec![MultiBodyChain::default_pendulum(), MultiBodyChain::default_double_pendulum(), MultiBodyChain::default_chain(8)];
    }

    ui.horizontal(|ui| {
        if ui.button("+ Pendulum").clicked() { state.chains.push(MultiBodyChain::default_pendulum()); }
        if ui.button("+ Chain (6)").clicked() { state.chains.push(MultiBodyChain::default_chain(6)); }
        if ui.button("+ Double Pendulum").clicked() { state.chains.push(MultiBodyChain::default_double_pendulum()); }
        if let Some(s) = state.selected { if ui.button("Delete").clicked() { state.chains.remove(s); state.selected = None; } }
    });

    for (i, chain) in state.chains.iter().enumerate() {
        let sel = state.selected == Some(i);
        let cc = Color32::from_rgb((chain.color[0]*255.0) as u8, (chain.color[1]*255.0) as u8, (chain.color[2]*255.0) as u8);
        if ui.selectable_label(sel, egui::RichText::new(format!("{} ({} links)", chain.name, chain.links.len())).color(cc)).clicked() {
            state.selected = Some(i);
        }
    }

    if let Some(idx) = state.selected {
        if idx < state.chains.len() {
            let chain = &mut state.chains[idx];
            ui.separator();
            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut chain.name); ui.checkbox(&mut chain.root_fixed, "Root Fixed"); });
            ui.horizontal(|ui| {
                ui.label("Joint:");
                for jt in &[JointType2D::Revolute, JointType2D::Distance, JointType2D::Weld] {
                    if ui.selectable_label(chain.joint_type == *jt, jt.label()).clicked() { chain.joint_type = jt.clone(); }
                }
            });
            ui.label("Links:");
            let mut remove = None;
            for (li, link) in chain.links.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{}", li));
                    ui.add(egui::DragValue::new(&mut link.length).prefix("L:").speed(0.02).clamp_range(0.1..=10.0));
                    ui.add(egui::DragValue::new(&mut link.width).prefix("W:").speed(0.005).clamp_range(0.01..=1.0));
                    ui.add(egui::DragValue::new(&mut link.mass).prefix("M:").speed(0.01).clamp_range(0.01..=10.0));
                    ui.add(egui::DragValue::new(&mut link.angle_offset).prefix("θ:").speed(0.01).suffix("°"));
                    if ui.button("✗").clicked() { remove = Some(li); }
                });
            }
            if let Some(ri) = remove { chain.links.remove(ri); }
            ui.horizontal(|ui| {
                if ui.button("+ Link").clicked() { chain.links.push(ChainLink { length: 0.5, width: 0.1, mass: 0.2, angle_offset: 0.0 }); }
                ui.label("End Mass:");
                let mut has_end = chain.end_mass.is_some();
                if ui.checkbox(&mut has_end, "").changed() { chain.end_mass = if has_end { Some(1.0) } else { None }; }
                if let Some(em) = chain.end_mass.as_mut() { ui.add(egui::DragValue::new(em).speed(0.1).clamp_range(0.01..=100.0)); }
            });
        }
    }

    draw_chain_preview(ui, state);
}

fn draw_chain_preview(ui: &mut egui::Ui, state: &MultiBodyChainState) {
    let desired = Vec2::new(ui.available_width().min(400.0), 240.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_rgb(12, 15, 22));

    if let Some(idx) = state.selected {
        if idx < state.chains.len() {
            let chain = &state.chains[idx];
            let cc = Color32::from_rgb((chain.color[0]*255.0) as u8, (chain.color[1]*255.0) as u8, (chain.color[2]*255.0) as u8);
            let anchor = Pos2::new(rect.center().x, rect.top() + 30.0);
            let scale = 40.0;

            if chain.root_fixed {
                p.line_segment([anchor - Vec2::new(12.0, 0.0), anchor + Vec2::new(12.0, 0.0)], Stroke::new(2.0, Color32::GRAY));
                for hatch in 0..4 {
                    let hx = anchor.x - 9.0 + hatch as f32 * 6.0;
                    p.line_segment([Pos2::new(hx, anchor.y), Pos2::new(hx - 5.0, anchor.y + 8.0)], Stroke::new(1.0, Color32::DARK_GRAY));
                }
            }
            p.circle_filled(anchor, 3.0, Color32::from_rgb(200, 200, 200));

            let mut cur = anchor;
            let mut angle = std::f32::consts::PI * 0.5; // hanging down
            for link in &chain.links {
                angle += link.angle_offset;
                let next = cur + Vec2::new(angle.cos(), angle.sin()) * link.length * scale;
                if next.y < rect.bottom() - 5.0 && next.x > rect.left()+5.0 && next.x < rect.right()-5.0 {
                    let perp = Vec2::new(-angle.sin(), angle.cos()) * link.width * scale * 0.5;
                    let corners = [cur + perp, cur - perp, next - perp, next + perp];
                    p.add(egui::Shape::convex_polygon(corners.to_vec(), cc, Stroke::new(1.0, Color32::WHITE)));
                    p.circle_filled(cur, 3.0, Color32::WHITE);
                    cur = next;
                }
            }
            if let Some(em) = chain.end_mass {
                let r = (em * 2.0).sqrt().clamp(3.0, 14.0);
                p.circle_filled(cur, r, Color32::from_rgb(240, 160, 60));
                p.circle_stroke(cur, r, Stroke::new(1.5, Color32::WHITE));
                p.text(cur, egui::Align2::CENTER_CENTER, format!("{:.1}", em), FontId::monospace(7.0), Color32::WHITE);
            }
            p.text(Pos2::new(rect.left()+4.0, rect.bottom()-12.0), egui::Align2::LEFT_BOTTOM, format!("{} | {} links | joint: {}", chain.name, chain.links.len(), chain.joint_type.label()), FontId::monospace(8.0), Color32::DARK_GRAY);
        }
    } else {
        p.text(rect.center(), egui::Align2::CENTER_CENTER, "Select a chain to preview", FontId::monospace(9.0), Color32::DARK_GRAY);
    }
}

// ─── Physics Material Comparison ─────────────────────────────────────────────

pub fn show_physics_material_comparison(ui: &mut egui::Ui) {
    ui.heading("Material Property Comparison");
    ui.separator();

    let presets = all_material_presets();
    let selected: Vec<&PhysicsMaterialPreset> = presets.iter().take(8).collect();

    let desired = Vec2::new(ui.available_width().min(600.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_rgb(12, 14, 20));

    let n = selected.len();
    let bar_w = rect.width() / n as f32;

    // Draw bars for friction and restitution
    for (i, mat) in selected.iter().enumerate() {
        let x = rect.left() + i as f32 * bar_w;

        // Friction bar (blue-ish)
        let fh = mat.friction.clamp(0.0, 1.5) / 1.5 * (rect.height() * 0.45);
        let fr = Rect::from_min_size(Pos2::new(x + bar_w*0.1, rect.bottom() - fh - rect.height()*0.05), Vec2::new(bar_w*0.35, fh));
        p.rect_filled(fr, 1.0, Color32::from_rgb(60, 100, 220));

        // Restitution bar (orange)
        let rh = mat.restitution.clamp(0.0, 1.0) * (rect.height() * 0.45);
        let rr = Rect::from_min_size(Pos2::new(x + bar_w*0.55, rect.bottom() - rh - rect.height()*0.05), Vec2::new(bar_w*0.35, rh));
        p.rect_filled(rr, 1.0, Color32::from_rgb(220, 140, 40));

        // Label
        let name_short = &mat.name[..mat.name.len().min(6)];
        p.text(Pos2::new(x + bar_w*0.5, rect.bottom()-2.0), egui::Align2::CENTER_BOTTOM, name_short, FontId::monospace(6.5), Color32::GRAY);
    }

    // Legend
    let legend_y = rect.top() + 8.0;
    p.rect_filled(Rect::from_min_size(Pos2::new(rect.left()+4.0, legend_y), Vec2::new(10.0, 8.0)), 1.0, Color32::from_rgb(60,100,220));
    p.text(Pos2::new(rect.left()+18.0, legend_y+4.0), egui::Align2::LEFT_CENTER, "Friction", FontId::monospace(7.0), Color32::GRAY);
    p.rect_filled(Rect::from_min_size(Pos2::new(rect.left()+70.0, legend_y), Vec2::new(10.0, 8.0)), 1.0, Color32::from_rgb(220,140,40));
    p.text(Pos2::new(rect.left()+84.0, legend_y+4.0), egui::Align2::LEFT_CENTER, "Restitution", FontId::monospace(7.0), Color32::GRAY);

    ui.separator();
    egui::Grid::new("mat_compare").num_columns(6).spacing([12.0, 2.0]).striped(true).show(ui, |ui| {
        ui.label("Material"); ui.label("Friction"); ui.label("Restitution"); ui.label("Density"); ui.label("Category"); ui.label("Notes"); ui.end_row();
        for mat in presets.iter().take(12) {
            ui.label(egui::RichText::new(&mat.name).small());
            ui.label(egui::RichText::new(format!("{:.2}", mat.friction)).small().monospace());
            ui.label(egui::RichText::new(format!("{:.2}", mat.restitution)).small().monospace());
            ui.label(egui::RichText::new(format!("{:.0}", mat.density)).small().monospace());
            ui.label(egui::RichText::new(mat.category.label()).small().color(mat.category.color()));
            ui.label(egui::RichText::new(mat.description.chars().take(30).collect::<String>()).small().color(Color32::GRAY));
            ui.end_row();
        }
    });
}

// ─── Physics Editor Main Panel Integration ────────────────────────────────────

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PhysicsEditorFullState {
    pub breakable_joints: BreakableJointEditorState,
    pub buoyancy: BuoyancyEditorState,
    pub trigger_zones: TriggerZoneEditorState,
    pub debug_overlay: DebugOverlaySettings,
    pub preset_browser: PresetBrowserState,
    pub ccd_settings: CcdSettings,
    pub island_manager: IslandManagerState,
    pub force_field_editor: ForceFieldEditorState2,
    pub sensor_bodies: SensorBodyEditorState,
    pub rigid_body_list: RigidBodyListState,
    pub event_logger: PhysicsEventLoggerState,
    pub broadphase: BroadphaseSettings,
    pub narrowphase: NarrowphaseSettings,
    pub solver_settings: ConstraintSolverSettings,
    pub chain_builder: MultiBodyChainState,
    pub velocity_field_painter: VelocityFieldPainterState,
    pub gizmo_settings: GizmoState,
    pub material_interaction: MaterialInteractionState,
    pub scenario_export: ScenarioExportState,
    pub timeline: PhysicsTimelineState,
    pub fluid_boundary_editor2: FluidBoundaryEditorState2,
    pub soft_body_advanced_selected: Option<usize>,
    pub active_sub_panel: PhysicsSubPanel,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum PhysicsSubPanel {
    #[default]
    Bodies,
    Joints,
    Constraints,
    Broadphase,
    Narrowphase,
    Solver,
    BreakableJoints,
    Buoyancy,
    TriggerZones,
    Sensors,
    ForceFields,
    ChainBuilder,
    DebugOverlay,
    PresetBrowser,
    CcdSettings,
    Islands,
    EventLogger,
    MaterialInteraction,
    ScenarioExport,
    Timeline,
    VelocityField,
    GizmoSettings,
    Hotkeys,
}

impl PhysicsSubPanel {
    pub fn label(&self) -> &str {
        match self {
            PhysicsSubPanel::Bodies => "Bodies",
            PhysicsSubPanel::Joints => "Joints",
            PhysicsSubPanel::Constraints => "Constraints",
            PhysicsSubPanel::Broadphase => "Broadphase",
            PhysicsSubPanel::Narrowphase => "Narrowphase",
            PhysicsSubPanel::Solver => "Solver",
            PhysicsSubPanel::BreakableJoints => "Breakable Joints",
            PhysicsSubPanel::Buoyancy => "Buoyancy",
            PhysicsSubPanel::TriggerZones => "Trigger Zones",
            PhysicsSubPanel::Sensors => "Sensors",
            PhysicsSubPanel::ForceFields => "Force Fields",
            PhysicsSubPanel::ChainBuilder => "Chain Builder",
            PhysicsSubPanel::DebugOverlay => "Debug Overlay",
            PhysicsSubPanel::PresetBrowser => "Presets",
            PhysicsSubPanel::CcdSettings => "CCD",
            PhysicsSubPanel::Islands => "Islands",
            PhysicsSubPanel::EventLogger => "Event Log",
            PhysicsSubPanel::MaterialInteraction => "Mat Interaction",
            PhysicsSubPanel::ScenarioExport => "Export/Import",
            PhysicsSubPanel::Timeline => "Timeline",
            PhysicsSubPanel::VelocityField => "Velocity Field",
            PhysicsSubPanel::GizmoSettings => "Gizmo",
            PhysicsSubPanel::Hotkeys => "Hotkeys",
        }
    }
}

pub fn show_physics_editor_full(ui: &mut egui::Ui, state: &mut PhysicsEditorFullState, soft_state: &mut SoftBodyEditorState, cloth_state: &mut ClothEditorState) {
    ui.horizontal_wrapped(|ui| {
        for panel in &[PhysicsSubPanel::Bodies, PhysicsSubPanel::BreakableJoints, PhysicsSubPanel::Buoyancy, PhysicsSubPanel::TriggerZones, PhysicsSubPanel::Sensors, PhysicsSubPanel::ForceFields, PhysicsSubPanel::ChainBuilder, PhysicsSubPanel::Broadphase, PhysicsSubPanel::Narrowphase, PhysicsSubPanel::Solver, PhysicsSubPanel::DebugOverlay, PhysicsSubPanel::PresetBrowser, PhysicsSubPanel::CcdSettings, PhysicsSubPanel::Islands, PhysicsSubPanel::EventLogger, PhysicsSubPanel::MaterialInteraction, PhysicsSubPanel::Timeline, PhysicsSubPanel::VelocityField, PhysicsSubPanel::GizmoSettings, PhysicsSubPanel::Hotkeys] {
            let sel = state.active_sub_panel == *panel;
            if ui.selectable_label(sel, panel.label()).clicked() { state.active_sub_panel = panel.clone(); }
        }
    });
    ui.separator();

    match state.active_sub_panel {
        PhysicsSubPanel::Bodies => show_rigid_body_list(ui, &mut state.rigid_body_list),
        PhysicsSubPanel::BreakableJoints => show_breakable_joint_editor(ui, &mut state.breakable_joints),
        PhysicsSubPanel::Buoyancy => show_buoyancy_editor(ui, &mut state.buoyancy),
        PhysicsSubPanel::TriggerZones => show_trigger_zone_editor(ui, &mut state.trigger_zones),
        PhysicsSubPanel::Sensors => show_sensor_body_editor(ui, &mut state.sensor_bodies),
        PhysicsSubPanel::ForceFields => show_force_field_editor2(ui, &mut state.force_field_editor),
        PhysicsSubPanel::ChainBuilder => show_multi_body_chain_builder(ui, &mut state.chain_builder),
        PhysicsSubPanel::Broadphase => show_broadphase_settings(ui, &mut state.broadphase),
        PhysicsSubPanel::Narrowphase => show_narrowphase_settings(ui, &mut state.narrowphase),
        PhysicsSubPanel::Solver => show_constraint_solver_settings(ui, &mut state.solver_settings),
        PhysicsSubPanel::DebugOverlay => show_debug_overlay_settings(ui, &mut state.debug_overlay),
        PhysicsSubPanel::PresetBrowser => show_preset_browser(ui, &mut state.preset_browser),
        PhysicsSubPanel::CcdSettings => show_ccd_settings(ui, &mut state.ccd_settings),
        PhysicsSubPanel::Islands => show_island_manager(ui, &mut state.island_manager),
        PhysicsSubPanel::EventLogger => show_physics_event_logger(ui, &mut state.event_logger),
        PhysicsSubPanel::MaterialInteraction => show_material_interaction_matrix(ui, &mut state.material_interaction),
        PhysicsSubPanel::ScenarioExport => show_scenario_export_import(ui, &mut state.scenario_export),
        PhysicsSubPanel::Timeline => show_physics_timeline(ui, &mut state.timeline),
        PhysicsSubPanel::VelocityField => show_velocity_field_painter(ui, &mut state.velocity_field_painter),
        PhysicsSubPanel::GizmoSettings => show_gizmo_settings(ui, &mut state.gizmo_settings),
        PhysicsSubPanel::Hotkeys => show_physics_hotkey_reference(ui),
        _ => { ui.label("Panel not yet implemented."); }
    }
}
"""

with open(path, "a", encoding="utf-8") as f:
    f.write(EXPANSION)

import subprocess
result = subprocess.run(["wc", "-l", path], capture_output=True, text=True, shell=False)
print(f"physics_editor.rs now has {result.stdout.strip()} lines")
