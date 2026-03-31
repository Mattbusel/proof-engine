use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Shape, RichText};
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum BodyType {
    Static,
    Dynamic,
    Kinematic,
}

impl BodyType {
    pub fn label(&self) -> &'static str {
        match self {
            BodyType::Static => "Static",
            BodyType::Dynamic => "Dynamic",
            BodyType::Kinematic => "Kinematic",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            BodyType::Static => Color32::from_rgb(150, 150, 150),
            BodyType::Dynamic => Color32::from_rgb(100, 180, 255),
            BodyType::Kinematic => Color32::from_rgb(180, 255, 100),
        }
    }

    pub fn all() -> &'static [BodyType] {
        &[BodyType::Static, BodyType::Dynamic, BodyType::Kinematic]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CombineMode {
    Average,
    Min,
    Max,
    Multiply,
}

impl CombineMode {
    pub fn label(&self) -> &'static str {
        match self {
            CombineMode::Average => "Average",
            CombineMode::Min => "Min",
            CombineMode::Max => "Max",
            CombineMode::Multiply => "Multiply",
        }
    }

    pub fn all() -> &'static [CombineMode] {
        &[CombineMode::Average, CombineMode::Min, CombineMode::Max, CombineMode::Multiply]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhysicsMaterial {
    pub name: String,
    pub restitution: f32,
    pub friction: f32,
    pub combine_mode: CombineMode,
    pub color: [u8; 3],
}

impl PhysicsMaterial {
    pub fn new(name: &str) -> Self {
        PhysicsMaterial {
            name: name.to_string(),
            restitution: 0.3,
            friction: 0.5,
            combine_mode: CombineMode::Average,
            color: [120, 120, 180],
        }
    }

    pub fn default_material() -> Self {
        PhysicsMaterial::new("Default")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Collider {
    Circle { radius: f32 },
    Box { width: f32, height: f32 },
    Capsule { radius: f32, height: f32 },
    Polygon { points: Vec<[f32; 2]> },
    Compound { colliders: Vec<Collider> },
}

impl Collider {
    pub fn label(&self) -> &'static str {
        match self {
            Collider::Circle { .. } => "Circle",
            Collider::Box { .. } => "Box",
            Collider::Capsule { .. } => "Capsule",
            Collider::Polygon { .. } => "Polygon",
            Collider::Compound { .. } => "Compound",
        }
    }

    pub fn type_index(&self) -> usize {
        match self {
            Collider::Circle { .. } => 0,
            Collider::Box { .. } => 1,
            Collider::Capsule { .. } => 2,
            Collider::Polygon { .. } => 3,
            Collider::Compound { .. } => 4,
        }
    }

    pub fn type_labels() -> &'static [&'static str] {
        &["Circle", "Box", "Capsule", "Polygon", "Compound"]
    }

    pub fn default_for_index(idx: usize) -> Collider {
        match idx {
            0 => Collider::Circle { radius: 0.5 },
            1 => Collider::Box { width: 1.0, height: 1.0 },
            2 => Collider::Capsule { radius: 0.3, height: 1.0 },
            3 => Collider::Polygon { points: vec![[0.0, 0.5], [0.5, -0.5], [-0.5, -0.5]] },
            _ => Collider::Compound { colliders: Vec::new() },
        }
    }

    pub fn approximate_area(&self) -> f32 {
        match self {
            Collider::Circle { radius } => std::f32::consts::PI * radius * radius,
            Collider::Box { width, height } => width * height,
            Collider::Capsule { radius, height } => std::f32::consts::PI * radius * radius + 2.0 * radius * height,
            Collider::Polygon { points } => {
                if points.len() < 3 { return 0.0; }
                let mut area = 0.0;
                let n = points.len();
                for i in 0..n {
                    let j = (i + 1) % n;
                    area += points[i][0] * points[j][1];
                    area -= points[j][0] * points[i][1];
                }
                (area / 2.0).abs()
            }
            Collider::Compound { colliders } => colliders.iter().map(|c| c.approximate_area()).sum(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColliderProperties {
    pub is_trigger: bool,
    pub restitution: f32,
    pub friction: f32,
    pub density: f32,
    pub material_idx: Option<usize>,
}

impl Default for ColliderProperties {
    fn default() -> Self {
        ColliderProperties {
            is_trigger: false,
            restitution: 0.3,
            friction: 0.5,
            density: 1.0,
            material_idx: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidBody {
    pub name: String,
    pub body_type: BodyType,
    pub mass: f32,
    pub linear_drag: f32,
    pub angular_drag: f32,
    pub gravity_scale: f32,
    pub position: [f32; 2],
    pub rotation: f32,
    pub velocity: [f32; 2],
    pub angular_velocity: f32,
    pub sleeping: bool,
    pub collision_layer: u32,
    pub collision_mask: u32,
    pub collider: Collider,
    pub collider_props: ColliderProperties,
    pub color: [u8; 3],
    pub notes: String,
}

impl Default for RigidBody {
    fn default() -> Self { RigidBody::new("", BodyType::Dynamic) }
}

impl RigidBody {
    pub fn new(name: &str, body_type: BodyType) -> Self {
        RigidBody {
            name: name.to_string(),
            body_type,
            mass: 1.0,
            linear_drag: 0.0,
            angular_drag: 0.05,
            gravity_scale: 1.0,
            position: [0.0, 0.0],
            rotation: 0.0,
            velocity: [0.0, 0.0],
            angular_velocity: 0.0,
            sleeping: false,
            collision_layer: 1,
            collision_mask: 0xFFFFFFFF,
            collider: Collider::Box { width: 1.0, height: 1.0 },
            collider_props: ColliderProperties::default(),
            color: [180, 180, 200],
            notes: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Joint {
    Fixed {
        body_a: usize,
        body_b: usize,
        break_force: f32,
    },
    Hinge {
        body_a: usize,
        body_b: usize,
        anchor: [f32; 2],
        lower_angle: f32,
        upper_angle: f32,
        motor_enabled: bool,
        motor_speed: f32,
        motor_max_torque: f32,
    },
    Slider {
        body_a: usize,
        body_b: usize,
        axis: [f32; 2],
        lower_limit: f32,
        upper_limit: f32,
        motor_enabled: bool,
        motor_speed: f32,
        motor_max_force: f32,
    },
    Spring {
        body_a: usize,
        body_b: usize,
        rest_length: f32,
        stiffness: f32,
        damping: f32,
    },
    Distance {
        body_a: usize,
        body_b: usize,
        min_distance: f32,
        max_distance: f32,
    },
    Pulley {
        body_a: usize,
        body_b: usize,
        anchor_a: [f32; 2],
        anchor_b: [f32; 2],
        ratio: f32,
    },
}

impl Joint {
    pub fn label(&self) -> &'static str {
        match self {
            Joint::Fixed { .. } => "Fixed",
            Joint::Hinge { .. } => "Hinge",
            Joint::Slider { .. } => "Slider",
            Joint::Spring { .. } => "Spring",
            Joint::Distance { .. } => "Distance",
            Joint::Pulley { .. } => "Pulley",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            Joint::Fixed { .. } => Color32::from_rgb(200, 200, 200),
            Joint::Hinge { .. } => Color32::from_rgb(100, 200, 255),
            Joint::Slider { .. } => Color32::from_rgb(100, 255, 180),
            Joint::Spring { .. } => Color32::from_rgb(255, 200, 80),
            Joint::Distance { .. } => Color32::from_rgb(200, 150, 255),
            Joint::Pulley { .. } => Color32::from_rgb(255, 150, 100),
        }
    }

    pub fn body_a(&self) -> usize {
        match self {
            Joint::Fixed { body_a, .. } => *body_a,
            Joint::Hinge { body_a, .. } => *body_a,
            Joint::Slider { body_a, .. } => *body_a,
            Joint::Spring { body_a, .. } => *body_a,
            Joint::Distance { body_a, .. } => *body_a,
            Joint::Pulley { body_a, .. } => *body_a,
        }
    }

    pub fn body_b(&self) -> usize {
        match self {
            Joint::Fixed { body_b, .. } => *body_b,
            Joint::Hinge { body_b, .. } => *body_b,
            Joint::Slider { body_b, .. } => *body_b,
            Joint::Spring { body_b, .. } => *body_b,
            Joint::Distance { body_b, .. } => *body_b,
            Joint::Pulley { body_b, .. } => *body_b,
        }
    }

    pub fn type_labels() -> &'static [&'static str] {
        &["Fixed", "Hinge", "Slider", "Spring", "Distance", "Pulley"]
    }

    pub fn default_for_index(idx: usize, a: usize, b: usize) -> Joint {
        match idx {
            0 => Joint::Fixed { body_a: a, body_b: b, break_force: f32::INFINITY },
            1 => Joint::Hinge { body_a: a, body_b: b, anchor: [0.0, 0.0], lower_angle: -180.0, upper_angle: 180.0, motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0 },
            2 => Joint::Slider { body_a: a, body_b: b, axis: [1.0, 0.0], lower_limit: -5.0, upper_limit: 5.0, motor_enabled: false, motor_speed: 0.0, motor_max_force: 0.0 },
            3 => Joint::Spring { body_a: a, body_b: b, rest_length: 2.0, stiffness: 100.0, damping: 5.0 },
            4 => Joint::Distance { body_a: a, body_b: b, min_distance: 0.0, max_distance: 5.0 },
            _ => Joint::Pulley { body_a: a, body_b: b, anchor_a: [-2.0, 4.0], anchor_b: [2.0, 4.0], ratio: 1.0 },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PhysicsView {
    Bodies,
    Joints,
    Materials,
    Matrix,
}

pub struct PhysicsEditor {
    pub bodies: Vec<RigidBody>,
    pub joints: Vec<Joint>,
    pub materials: Vec<PhysicsMaterial>,
    pub collision_matrix: [[bool; 16]; 16],
    pub layer_names: [String; 16],
    pub selected_body: Option<usize>,
    pub selected_joint: Option<usize>,
    pub simulation_running: bool,
    pub simulation_time: f32,
    pub gravity: [f32; 2],
    pub show_matrix: bool,
    pub show_materials: bool,
    pub show_joints: bool,
    pub view: PhysicsView,
    pub canvas_offset: Vec2,
    pub canvas_zoom: f32,
    pub show_velocities: bool,
    pub show_colliders: bool,
    pub show_joints_in_preview: bool,
    pub new_joint_type: usize,
    pub new_joint_body_a: usize,
    pub new_joint_body_b: usize,
    pub new_body_name: String,
    pub new_material_name: String,
    pub selected_material: Option<usize>,
    pub collider_type_sel: usize,
    pub polygon_edit_mode: bool,
    pub time_scale: f32,
    pub substeps: usize,
    pub global_damping: f32,
    pub preview_offset: Vec2,
    pub preview_zoom: f32,
    pub simulating: bool,
}

impl PhysicsEditor {
    pub fn new() -> Self {
        let mut layer_names: [String; 16] = Default::default();
        layer_names[0] = "Default".to_string();
        layer_names[1] = "Player".to_string();
        layer_names[2] = "Enemy".to_string();
        layer_names[3] = "Terrain".to_string();
        layer_names[4] = "Projectile".to_string();
        layer_names[5] = "Trigger".to_string();
        for i in 6..16 {
            layer_names[i] = format!("Layer {}", i);
        }

        let mut collision_matrix = [[true; 16]; 16];
        // Projectiles don't collide with each other
        collision_matrix[4][4] = false;
        // Triggers are triggers only
        for i in 0..16 {
            collision_matrix[5][i] = false;
            collision_matrix[i][5] = false;
        }

        let mut editor = PhysicsEditor {
            bodies: Vec::new(),
            joints: Vec::new(),
            materials: Vec::new(),
            collision_matrix,
            layer_names,
            selected_body: None,
            selected_joint: None,
            simulation_running: false,
            simulation_time: 0.0,
            gravity: [0.0, -9.81],
            show_matrix: false,
            show_materials: false,
            show_joints: true,
            view: PhysicsView::Bodies,
            canvas_offset: Vec2::new(300.0, 200.0),
            canvas_zoom: 40.0,
            show_velocities: true,
            show_colliders: true,
            show_joints_in_preview: true,
            new_joint_type: 0,
            new_joint_body_a: 0,
            new_joint_body_b: 1,
            new_body_name: "New Body".to_string(),
            new_material_name: "New Material".to_string(),
            selected_material: None,
            collider_type_sel: 1,
            polygon_edit_mode: false,
            time_scale: 1.0,
            substeps: 4,
            global_damping: 0.0,
            preview_offset: Vec2::new(300.0, 200.0),
            preview_zoom: 40.0,
            simulating: false,
        };
        editor.populate_demo_data();
        editor
    }

    fn populate_demo_data(&mut self) {
        // Ground
        let mut ground = RigidBody::new("Ground", BodyType::Static);
        ground.position = [0.0, -3.0];
        ground.collider = Collider::Box { width: 20.0, height: 1.0 };
        ground.collision_layer = 8; // Terrain bit
        ground.collision_mask = 0xFFFFFFFF;
        self.bodies.push(ground);

        // Dynamic box
        let mut box1 = RigidBody::new("Box A", BodyType::Dynamic);
        box1.position = [-2.0, 2.0];
        box1.rotation = 15.0;
        box1.collider = Collider::Box { width: 1.0, height: 1.0 };
        box1.velocity = [0.5, 0.0];
        self.bodies.push(box1);

        // Dynamic circle
        let mut circle = RigidBody::new("Ball", BodyType::Dynamic);
        circle.position = [2.0, 4.0];
        circle.collider = Collider::Circle { radius: 0.6 };
        circle.mass = 2.0;
        circle.velocity = [-1.0, 0.5];
        self.bodies.push(circle);

        // Kinematic platform
        let mut platform = RigidBody::new("Platform", BodyType::Kinematic);
        platform.position = [4.0, 0.0];
        platform.collider = Collider::Box { width: 3.0, height: 0.3 };
        platform.velocity = [1.5, 0.0];
        self.bodies.push(platform);

        // Capsule character
        let mut character = RigidBody::new("Character", BodyType::Dynamic);
        character.position = [-4.0, 1.0];
        character.collider = Collider::Capsule { radius: 0.4, height: 1.2 };
        character.mass = 70.0;
        character.linear_drag = 0.2;
        character.collision_layer = 2; // Player
        self.bodies.push(character);

        // Pentagon
        let mut pentagon = RigidBody::new("Pentagon", BodyType::Dynamic);
        pentagon.position = [0.0, 5.0];
        let n = 5_usize;
        let pts: Vec<[f32; 2]> = (0..n).map(|i| {
            let angle = i as f32 * 2.0 * std::f32::consts::PI / n as f32 - std::f32::consts::FRAC_PI_2;
            [angle.cos() * 0.7, angle.sin() * 0.7]
        }).collect();
        pentagon.collider = Collider::Polygon { points: pts };
        self.bodies.push(pentagon);

        // Spring joint between box and circle
        self.joints.push(Joint::Spring {
            body_a: 1,
            body_b: 2,
            rest_length: 3.0,
            stiffness: 80.0,
            damping: 4.0,
        });

        // Hinge on box A to ground
        self.joints.push(Joint::Hinge {
            body_a: 0,
            body_b: 1,
            anchor: [-2.0, 0.5],
            lower_angle: -90.0,
            upper_angle: 90.0,
            motor_enabled: false,
            motor_speed: 0.0,
            motor_max_torque: 0.0,
        });

        // Materials
        self.materials.push(PhysicsMaterial {
            name: "Rubber".to_string(),
            restitution: 0.8,
            friction: 0.9,
            combine_mode: CombineMode::Max,
            color: [200, 100, 80],
        });
        self.materials.push(PhysicsMaterial {
            name: "Ice".to_string(),
            restitution: 0.1,
            friction: 0.02,
            combine_mode: CombineMode::Min,
            color: [150, 200, 255],
        });
        self.materials.push(PhysicsMaterial {
            name: "Wood".to_string(),
            restitution: 0.2,
            friction: 0.6,
            combine_mode: CombineMode::Average,
            color: [180, 140, 80],
        });
        self.materials.push(PhysicsMaterial {
            name: "Metal".to_string(),
            restitution: 0.15,
            friction: 0.4,
            combine_mode: CombineMode::Average,
            color: [160, 170, 190],
        });
    }

    pub fn simulate_step(&mut self, dt: f32) {
        self.simulation_time += dt;
        let gx = self.gravity[0];
        let gy = self.gravity[1];

        for body in self.bodies.iter_mut() {
            if body.body_type == BodyType::Dynamic && !body.sleeping {
                body.velocity[0] += gx * body.gravity_scale * dt;
                body.velocity[1] += gy * body.gravity_scale * dt;

                body.velocity[0] *= (1.0 - body.linear_drag * dt).max(0.0);
                body.velocity[1] *= (1.0 - body.linear_drag * dt).max(0.0);

                body.position[0] += body.velocity[0] * dt;
                body.position[1] += body.velocity[1] * dt;
                body.rotation += body.angular_velocity * dt;

                // Simple ground collision
                let ground_y = -2.5_f32;
                let body_bottom = body.position[1] - match &body.collider {
                    Collider::Circle { radius } => *radius,
                    Collider::Box { height, .. } => height / 2.0,
                    Collider::Capsule { radius, height } => radius + height / 2.0,
                    _ => 0.5,
                };
                if body_bottom < ground_y {
                    body.position[1] += ground_y - body_bottom;
                    body.velocity[1] = -body.velocity[1] * 0.4;
                    body.velocity[0] *= 0.85;
                }

                // Wrap horizontally
                if body.position[0] > 12.0 { body.position[0] = -12.0; }
                if body.position[0] < -12.0 { body.position[0] = 12.0; }
            } else if body.body_type == BodyType::Kinematic {
                body.position[0] += body.velocity[0] * dt;
                body.position[1] += body.velocity[1] * dt;
                // Bounce kinematic bodies
                if body.position[0].abs() > 8.0 { body.velocity[0] = -body.velocity[0]; }
            }
        }
    }
}

pub fn show(ui: &mut egui::Ui, editor: &mut PhysicsEditor, dt: f32) {
    if editor.simulation_running {
        editor.simulate_step(dt);
    }

    ui.horizontal(|ui| {
        ui.heading(RichText::new("Physics Constraint Editor").size(18.0).color(Color32::from_rgb(180, 255, 120)));
        ui.separator();

        let sim_color = if editor.simulation_running { Color32::from_rgb(100, 220, 100) } else { Color32::from_rgb(220, 100, 100) };
        if ui.add(egui::Button::new(
            RichText::new(if editor.simulation_running { "⏸ Pause" } else { "▶ Simulate" }).color(sim_color)
        )).clicked() {
            editor.simulation_running = !editor.simulation_running;
        }
        if ui.button("Reset").clicked() {
            editor.simulation_running = false;
            editor.simulation_time = 0.0;
            for body in editor.bodies.iter_mut() {
                body.velocity = [0.0, 0.0];
                body.angular_velocity = 0.0;
            }
        }
        ui.label(RichText::new(format!("t={:.2}s", editor.simulation_time)).color(Color32::GRAY).small());
        ui.separator();
        ui.label("Gravity:");
        ui.add(egui::DragValue::new(&mut editor.gravity[0]).prefix("X:").speed(0.01).range(-50.0..=50.0));
        ui.add(egui::DragValue::new(&mut editor.gravity[1]).prefix("Y:").speed(0.01).range(-50.0..=50.0));
    });
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("View:");
        if ui.selectable_label(editor.view == PhysicsView::Bodies, "Bodies").clicked() { editor.view = PhysicsView::Bodies; }
        if ui.selectable_label(editor.view == PhysicsView::Joints, "Joints").clicked() { editor.view = PhysicsView::Joints; }
        if ui.selectable_label(editor.view == PhysicsView::Materials, "Materials").clicked() { editor.view = PhysicsView::Materials; }
        if ui.selectable_label(editor.view == PhysicsView::Matrix, "Collision Matrix").clicked() { editor.view = PhysicsView::Matrix; }
        ui.separator();
        ui.checkbox(&mut editor.show_velocities, "Velocities");
        ui.checkbox(&mut editor.show_colliders, "Colliders");
        ui.checkbox(&mut editor.show_joints_in_preview, "Joints");
    });
    ui.separator();

    egui::SidePanel::right("physics_inspector")
        .resizable(true)
        .default_width(300.0)
        .show_inside(ui, |ui| {
            match editor.view {
                PhysicsView::Bodies => {
                    if let Some(sel) = editor.selected_body {
                        if sel < editor.bodies.len() {
                            show_body_inspector(ui, editor, sel);
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("Select a body to inspect").color(Color32::GRAY));
                        });
                    }
                }
                PhysicsView::Joints => {
                    if let Some(sel) = editor.selected_joint {
                        if sel < editor.joints.len() {
                            show_joint_inspector(ui, editor, sel);
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("Select a joint to inspect").color(Color32::GRAY));
                        });
                    }
                }
                PhysicsView::Materials => {
                    show_material_inspector(ui, editor);
                }
                PhysicsView::Matrix => {
                    ui.label(RichText::new("Edit layer names").color(Color32::GRAY));
                    egui::ScrollArea::vertical().id_salt("layer_names_scroll").show(ui, |ui| {
                        for i in 0..16 {
                            ui.horizontal(|ui| {
                                ui.label(format!("Layer {}:", i));
                                ui.text_edit_singleline(&mut editor.layer_names[i]);
                            });
                        }
                    });
                }
            }
        });

    egui::SidePanel::left("physics_list_panel")
        .resizable(true)
        .default_width(200.0)
        .show_inside(ui, |ui| {
            match editor.view {
                PhysicsView::Bodies => show_body_list(ui, editor),
                PhysicsView::Joints => show_joint_list(ui, editor),
                PhysicsView::Materials => show_material_list(ui, editor),
                PhysicsView::Matrix => show_collision_matrix(ui, editor),
            }
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        show_physics_preview(ui, editor);
    });
}

fn show_body_list(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    ui.horizontal(|ui| {
        ui.strong("Bodies");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            egui::ComboBox::from_id_salt("new_body_type")
                .selected_text("Dynamic")
                .width(70.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(false, "Static").clicked() {
                        let mut b = RigidBody::new(&editor.new_body_name, BodyType::Static);
                        b.position = [0.0, 0.0];
                        editor.bodies.push(b);
                        editor.selected_body = Some(editor.bodies.len() - 1);
                    }
                    if ui.selectable_label(false, "Dynamic").clicked() {
                        let mut b = RigidBody::new(&editor.new_body_name, BodyType::Dynamic);
                        b.position = [0.0, 2.0];
                        editor.bodies.push(b);
                        editor.selected_body = Some(editor.bodies.len() - 1);
                    }
                    if ui.selectable_label(false, "Kinematic").clicked() {
                        let b = RigidBody::new(&editor.new_body_name, BodyType::Kinematic);
                        editor.bodies.push(b);
                        editor.selected_body = Some(editor.bodies.len() - 1);
                    }
                });
            ui.text_edit_singleline(&mut editor.new_body_name);
        });
    });
    ui.separator();

    let mut to_delete: Option<usize> = None;
    egui::ScrollArea::vertical()
        .id_salt("body_list_scroll")
        .show(ui, |ui| {
            egui::Grid::new("body_list_grid")
                .num_columns(4)
                .striped(true)
                .min_col_width(30.0)
                .show(ui, |ui| {
                    ui.strong("Type");
                    ui.strong("Name");
                    ui.strong("Shape");
                    ui.strong("");
                    ui.end_row();

                    let body_count = editor.bodies.len();
                    for bi in 0..body_count {
                        let is_sel = editor.selected_body == Some(bi);
                        let (type_color, body_name, collider_label, body_sleeping) = {
                            let body = &editor.bodies[bi];
                            (body.body_type.color(), body.name.clone(), body.collider.label(), body.sleeping)
                        };
                        let body_type = editor.bodies[bi].body_type.clone();

                        let (type_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
                        match body_type {
                            BodyType::Static => { ui.painter().rect_filled(type_rect, 0.0, type_color); }
                            BodyType::Dynamic => { ui.painter().circle_filled(type_rect.center(), 7.0, type_color); }
                            BodyType::Kinematic => {
                                let tri = vec![
                                    type_rect.center_top(),
                                    type_rect.right_bottom(),
                                    type_rect.left_bottom(),
                                ];
                                ui.painter().add(Shape::convex_polygon(tri, type_color, Stroke::NONE));
                            }
                        }

                        let name_resp = ui.selectable_label(is_sel, RichText::new(&body_name).color(if is_sel { Color32::WHITE } else { Color32::LIGHT_GRAY }));
                        if name_resp.clicked() {
                            editor.selected_body = Some(bi);
                        }
                        name_resp.context_menu(|ui| {
                            if ui.button("Duplicate").clicked() {
                                let mut new_body = editor.bodies[bi].clone();
                                new_body.name = format!("{} (copy)", new_body.name);
                                new_body.position[0] += 1.0;
                                editor.bodies.push(new_body);
                                ui.close_menu();
                            }
                            if ui.button(RichText::new("Delete").color(Color32::from_rgb(255, 80, 80))).clicked() {
                                to_delete = Some(bi);
                                ui.close_menu();
                            }
                        });

                        ui.label(RichText::new(collider_label).small().color(Color32::GRAY));

                        if body_sleeping {
                            ui.label(RichText::new("Z").color(Color32::GRAY).small());
                        } else {
                            ui.label("");
                        }

                        ui.end_row();
                    }
                });
        });

    if let Some(idx) = to_delete {
        editor.bodies.remove(idx);
        if editor.selected_body == Some(idx) { editor.selected_body = None; }
        editor.joints.retain(|j| j.body_a() != idx && j.body_b() != idx);
    }
}

fn show_body_inspector(ui: &mut egui::Ui, editor: &mut PhysicsEditor, sel: usize) {
    let body = &mut editor.bodies[sel];

    ui.heading(RichText::new("Body Inspector").size(14.0).color(Color32::from_rgb(180, 255, 120)));
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("body_inspector_scroll")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut body.name);
            });

            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::from_id_salt("body_type_combo")
                    .selected_text(body.body_type.label())
                    .show_ui(ui, |ui| {
                        for bt in BodyType::all() {
                            if ui.selectable_label(&body.body_type == bt, bt.label()).clicked() {
                                body.body_type = bt.clone();
                            }
                        }
                    });
            });

            ui.separator();
            ui.strong("Transform");
            ui.horizontal(|ui| {
                ui.label("Pos:");
                ui.add(egui::DragValue::new(&mut body.position[0]).prefix("X:").speed(0.01));
                ui.add(egui::DragValue::new(&mut body.position[1]).prefix("Y:").speed(0.01));
            });
            ui.horizontal(|ui| {
                ui.label("Rot:");
                ui.add(egui::DragValue::new(&mut body.rotation).suffix("°").speed(0.5).range(-180.0..=180.0));
            });

            ui.separator();
            ui.strong("Physics");
            if body.body_type == BodyType::Dynamic {
                ui.horizontal(|ui| {
                    ui.label("Mass:");
                    ui.add(egui::DragValue::new(&mut body.mass).range(0.001..=99999.0).speed(0.01));
                });
                ui.horizontal(|ui| {
                    ui.label("Lin. Drag:");
                    ui.add(egui::Slider::new(&mut body.linear_drag, 0.0..=10.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Ang. Drag:");
                    ui.add(egui::Slider::new(&mut body.angular_drag, 0.0..=10.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Gravity Scale:");
                    ui.add(egui::DragValue::new(&mut body.gravity_scale).speed(0.01).range(-10.0..=10.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Velocity:");
                    ui.add(egui::DragValue::new(&mut body.velocity[0]).prefix("X:").speed(0.01));
                    ui.add(egui::DragValue::new(&mut body.velocity[1]).prefix("Y:").speed(0.01));
                });
                ui.horizontal(|ui| {
                    ui.label("Ang. Vel:");
                    ui.add(egui::DragValue::new(&mut body.angular_velocity).suffix(" rad/s").speed(0.01));
                });
                ui.checkbox(&mut body.sleeping, "Sleeping");
            }

            ui.separator();
            ui.strong("Collision");
            ui.horizontal(|ui| {
                ui.label("Layer:");
                let mut layer_idx = body.collision_layer.trailing_zeros() as usize;
                if layer_idx >= 16 { layer_idx = 0; }
                egui::ComboBox::from_id_salt("body_layer")
                    .selected_text(&editor.layer_names[layer_idx])
                    .show_ui(ui, |ui| {
                        for i in 0..16 {
                            if ui.selectable_label(layer_idx == i, &editor.layer_names[i]).clicked() {
                                body.collision_layer = 1 << i;
                            }
                        }
                    });
            });

            ui.separator();
            ui.strong("Collider");
            let cur_type = body.collider.type_index();
            let mut sel_type = cur_type;
            egui::ComboBox::from_id_salt("collider_type")
                .selected_text(Collider::type_labels()[cur_type])
                .show_ui(ui, |ui| {
                    for (i, label) in Collider::type_labels().iter().enumerate() {
                        if ui.selectable_label(i == cur_type, *label).clicked() {
                            sel_type = i;
                        }
                    }
                });
            if sel_type != cur_type {
                body.collider = Collider::default_for_index(sel_type);
            }

            match &mut body.collider {
                Collider::Circle { radius } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius:");
                        ui.add(egui::DragValue::new(radius).range(0.01..=100.0).speed(0.01));
                    });
                }
                Collider::Box { width, height } => {
                    ui.horizontal(|ui| {
                        ui.label("Width:");
                        ui.add(egui::DragValue::new(width).range(0.01..=100.0).speed(0.01));
                        ui.label("Height:");
                        ui.add(egui::DragValue::new(height).range(0.01..=100.0).speed(0.01));
                    });
                }
                Collider::Capsule { radius, height } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius:");
                        ui.add(egui::DragValue::new(radius).range(0.01..=100.0).speed(0.01));
                        ui.label("Height:");
                        ui.add(egui::DragValue::new(height).range(0.01..=100.0).speed(0.01));
                    });
                }
                Collider::Polygon { points } => {
                    ui.label(format!("{} vertices", points.len()));
                    let mut to_remove: Option<usize> = None;
                    egui::ScrollArea::vertical().id_salt("polygon_pts").max_height(100.0).show(ui, |ui| {
                        for (pi, pt) in points.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("P{}:", pi));
                                ui.add(egui::DragValue::new(&mut pt[0]).prefix("x:").speed(0.01));
                                ui.add(egui::DragValue::new(&mut pt[1]).prefix("y:").speed(0.01));
                                if ui.small_button("x").clicked() { to_remove = Some(pi); }
                            });
                        }
                    });
                    if let Some(pi) = to_remove { points.remove(pi); }
                    if ui.small_button("+ Point").clicked() {
                        let n = points.len() as f32;
                        let angle = n * std::f32::consts::TAU / 5.0;
                        points.push([angle.cos() * 0.7, angle.sin() * 0.7]);
                    }
                }
                Collider::Compound { colliders } => {
                    ui.label(format!("{} sub-colliders", colliders.len()));
                }
            }

            ui.separator();
            ui.strong("Material");
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut body.collider_props.restitution, 0.0..=1.0).text("Restitution"));
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut body.collider_props.friction, 0.0..=1.0).text("Friction"));
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut body.collider_props.density, 0.01..=100.0).text("Density").logarithmic(true));
            });
            ui.checkbox(&mut body.collider_props.is_trigger, "Is Trigger");

            let mat_names: Vec<String> = editor.materials.iter().map(|m| m.name.clone()).collect();
            let cur_mat_name = body.collider_props.material_idx.and_then(|i| mat_names.get(i).cloned()).unwrap_or_else(|| "None".to_string());
            ui.horizontal(|ui| {
                ui.label("Phys. Material:");
                egui::ComboBox::from_id_salt("body_material")
                    .selected_text(cur_mat_name)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(body.collider_props.material_idx.is_none(), "None").clicked() {
                            body.collider_props.material_idx = None;
                        }
                        for (i, name) in mat_names.iter().enumerate() {
                            if ui.selectable_label(body.collider_props.material_idx == Some(i), name.as_str()).clicked() {
                                body.collider_props.material_idx = Some(i);
                            }
                        }
                    });
            });
        });
}

fn show_joint_list(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    ui.horizontal(|ui| {
        ui.strong("Joints");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ Add").clicked() {
                let a = editor.new_joint_body_a.min(editor.bodies.len().saturating_sub(1));
                let b = editor.new_joint_body_b.min(editor.bodies.len().saturating_sub(1));
                let j = Joint::default_for_index(editor.new_joint_type, a, b);
                editor.joints.push(j);
                editor.selected_joint = Some(editor.joints.len() - 1);
            }
        });
    });

    ui.horizontal(|ui| {
        ui.label("Type:");
        egui::ComboBox::from_id_salt("new_joint_type")
            .selected_text(Joint::type_labels()[editor.new_joint_type])
            .show_ui(ui, |ui| {
                for (i, label) in Joint::type_labels().iter().enumerate() {
                    if ui.selectable_label(i == editor.new_joint_type, *label).clicked() {
                        editor.new_joint_type = i;
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        let body_names: Vec<String> = editor.bodies.iter().map(|b| b.name.clone()).collect();
        ui.label("A:");
        egui::ComboBox::from_id_salt("joint_body_a")
            .selected_text(body_names.get(editor.new_joint_body_a).cloned().unwrap_or_default())
            .width(80.0)
            .show_ui(ui, |ui| {
                for (i, name) in body_names.iter().enumerate() {
                    if ui.selectable_label(i == editor.new_joint_body_a, name.as_str()).clicked() {
                        editor.new_joint_body_a = i;
                    }
                }
            });
        ui.label("B:");
        egui::ComboBox::from_id_salt("joint_body_b")
            .selected_text(body_names.get(editor.new_joint_body_b).cloned().unwrap_or_default())
            .width(80.0)
            .show_ui(ui, |ui| {
                for (i, name) in body_names.iter().enumerate() {
                    if ui.selectable_label(i == editor.new_joint_body_b, name.as_str()).clicked() {
                        editor.new_joint_body_b = i;
                    }
                }
            });
    });

    ui.separator();

    let mut to_delete: Option<usize> = None;
    egui::ScrollArea::vertical().id_salt("joint_list_scroll").show(ui, |ui| {
        let joint_count = editor.joints.len();
        for ji in 0..joint_count {
            let joint = &editor.joints[ji];
            let is_sel = editor.selected_joint == Some(ji);
            let joint_color = joint.color();
            let joint_label = joint.label().to_string();
            let ba = joint.body_a();
            let bb = joint.body_b();
            let name_a = editor.bodies.get(ba).map(|b| b.name.clone()).unwrap_or_else(|| format!("#{}", ba));
            let name_b = editor.bodies.get(bb).map(|b| b.name.clone()).unwrap_or_else(|| format!("#{}", bb));

            ui.push_id(ji, |ui| {
                egui::Frame::none()
                    .fill(if is_sel { Color32::from_rgb(40, 50, 70) } else { Color32::from_rgb(28, 28, 35) })
                    .stroke(Stroke::new(1.0, if is_sel { joint_color } else { Color32::from_rgb(45, 45, 55) }))
                    .inner_margin(4.0)
                    .corner_radius(3.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&joint_label).color(joint_color).strong());
                            ui.label(RichText::new(format!("{} ↔ {}", name_a, name_b)).color(Color32::LIGHT_GRAY).small());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("x").clicked() { to_delete = Some(ji); }
                            });
                        });
                    });
                if ui.interact(ui.min_rect(), egui::Id::new("joint_click").with(ji), egui::Sense::click()).clicked() {
                    editor.selected_joint = Some(ji);
                }
                ui.add_space(2.0);
            });
        }
    });

    if let Some(idx) = to_delete {
        editor.joints.remove(idx);
        if editor.selected_joint == Some(idx) { editor.selected_joint = None; }
    }
}

fn show_joint_inspector(ui: &mut egui::Ui, editor: &mut PhysicsEditor, sel: usize) {
    let joint_label = editor.joints[sel].label().to_string();
    ui.heading(RichText::new(format!("{} Joint", joint_label)).size(14.0).color(editor.joints[sel].color()));
    ui.separator();

    let body_names: Vec<String> = editor.bodies.iter().map(|b| b.name.clone()).collect();

    egui::ScrollArea::vertical().id_salt("joint_inspector_scroll").show(ui, |ui| {
        match &mut editor.joints[sel] {
            Joint::Fixed { body_a, body_b, break_force } => {
                ui.horizontal(|ui| {
                    ui.label("Body A:");
                    egui::ComboBox::from_id_salt("ji_body_a").selected_text(body_names.get(*body_a).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_a == i, name.as_str()).clicked() { *body_a = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Body B:");
                    egui::ComboBox::from_id_salt("ji_body_b").selected_text(body_names.get(*body_b).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_b == i, name.as_str()).clicked() { *body_b = i; }
                        }
                    });
                });
                let is_inf = break_force.is_infinite();
                let mut has_break = !is_inf;
                if ui.checkbox(&mut has_break, "Break Force").changed() {
                    *break_force = if has_break { 1000.0 } else { f32::INFINITY };
                }
                if !is_inf {
                    ui.add(egui::DragValue::new(break_force).range(0.0..=999999.0).speed(10.0));
                }
            }

            Joint::Hinge { body_a, body_b, anchor, lower_angle, upper_angle, motor_enabled, motor_speed, motor_max_torque } => {
                ui.horizontal(|ui| {
                    ui.label("Body A:");
                    egui::ComboBox::from_id_salt("ji_hinge_a").selected_text(body_names.get(*body_a).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_a == i, name.as_str()).clicked() { *body_a = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Body B:");
                    egui::ComboBox::from_id_salt("ji_hinge_b").selected_text(body_names.get(*body_b).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_b == i, name.as_str()).clicked() { *body_b = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Anchor:");
                    ui.add(egui::DragValue::new(&mut anchor[0]).prefix("x:").speed(0.01));
                    ui.add(egui::DragValue::new(&mut anchor[1]).prefix("y:").speed(0.01));
                });
                ui.horizontal(|ui| {
                    ui.label("Angle Limits:");
                    ui.add(egui::DragValue::new(lower_angle).suffix("°").speed(0.5).range(-360.0..=0.0));
                    ui.label("to");
                    ui.add(egui::DragValue::new(upper_angle).suffix("°").speed(0.5).range(0.0..=360.0));
                });
                ui.checkbox(motor_enabled, "Motor");
                if *motor_enabled {
                    ui.add(egui::Slider::new(motor_speed, -100.0..=100.0).text("Speed rad/s"));
                    ui.add(egui::Slider::new(motor_max_torque, 0.0..=10000.0).text("Max Torque").logarithmic(true));
                }
            }

            Joint::Slider { body_a, body_b, axis, lower_limit, upper_limit, motor_enabled, motor_speed, motor_max_force } => {
                ui.horizontal(|ui| {
                    ui.label("Body A:");
                    egui::ComboBox::from_id_salt("ji_slider_a").selected_text(body_names.get(*body_a).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_a == i, name.as_str()).clicked() { *body_a = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Body B:");
                    egui::ComboBox::from_id_salt("ji_slider_b").selected_text(body_names.get(*body_b).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_b == i, name.as_str()).clicked() { *body_b = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Axis:");
                    ui.add(egui::DragValue::new(&mut axis[0]).prefix("x:").speed(0.01).range(-1.0..=1.0));
                    ui.add(egui::DragValue::new(&mut axis[1]).prefix("y:").speed(0.01).range(-1.0..=1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Limits:");
                    ui.add(egui::DragValue::new(lower_limit).speed(0.01));
                    ui.label("to");
                    ui.add(egui::DragValue::new(upper_limit).speed(0.01));
                });
                ui.checkbox(motor_enabled, "Motor");
                if *motor_enabled {
                    ui.add(egui::Slider::new(motor_speed, -50.0..=50.0).text("Speed m/s"));
                    ui.add(egui::Slider::new(motor_max_force, 0.0..=100000.0).text("Max Force").logarithmic(true));
                }
            }

            Joint::Spring { body_a, body_b, rest_length, stiffness, damping } => {
                ui.horizontal(|ui| {
                    ui.label("Body A:");
                    egui::ComboBox::from_id_salt("ji_spring_a").selected_text(body_names.get(*body_a).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_a == i, name.as_str()).clicked() { *body_a = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Body B:");
                    egui::ComboBox::from_id_salt("ji_spring_b").selected_text(body_names.get(*body_b).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_b == i, name.as_str()).clicked() { *body_b = i; }
                        }
                    });
                });
                ui.add(egui::Slider::new(rest_length, 0.1..=20.0).text("Rest Length").suffix(" m"));
                ui.add(egui::Slider::new(stiffness, 0.0..=5000.0).text("Stiffness").logarithmic(true));
                ui.add(egui::Slider::new(damping, 0.0..=500.0).text("Damping").logarithmic(true));

                // Spring visualization
                let (spring_rect, _) = ui.allocate_exact_size(Vec2::new(200.0, 60.0), egui::Sense::hover());
                let painter = ui.painter();
                painter.rect_filled(spring_rect, 2.0, Color32::from_rgb(20, 20, 25));
                let coils = 8;
                let coil_h = 12.0;
                let cx = spring_rect.center().x;
                let y0 = spring_rect.min.y + 8.0;
                let y1 = spring_rect.max.y - 8.0;
                let spring_len = y1 - y0;
                let coil_spacing = spring_len / coils as f32;
                painter.line_segment([Pos2::new(cx, spring_rect.min.y), Pos2::new(cx, y0)], Stroke::new(1.5, Color32::from_rgb(255, 200, 80)));
                let mut prev = Pos2::new(cx, y0);
                for c in 0..coils {
                    let y_mid = y0 + c as f32 * coil_spacing + coil_spacing / 2.0;
                    let y_end = y0 + (c + 1) as f32 * coil_spacing;
                    let side = if c % 2 == 0 { coil_h } else { -coil_h };
                    let mid = Pos2::new(cx + side, y_mid);
                    let end = Pos2::new(cx, y_end);
                    painter.line_segment([prev, mid], Stroke::new(1.5, Color32::from_rgb(255, 200, 80)));
                    painter.line_segment([mid, end], Stroke::new(1.5, Color32::from_rgb(255, 200, 80)));
                    prev = end;
                }
                painter.line_segment([prev, Pos2::new(cx, spring_rect.max.y)], Stroke::new(1.5, Color32::from_rgb(255, 200, 80)));
            }

            Joint::Distance { body_a, body_b, min_distance, max_distance } => {
                ui.horizontal(|ui| {
                    ui.label("Body A:");
                    egui::ComboBox::from_id_salt("ji_dist_a").selected_text(body_names.get(*body_a).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_a == i, name.as_str()).clicked() { *body_a = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Body B:");
                    egui::ComboBox::from_id_salt("ji_dist_b").selected_text(body_names.get(*body_b).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_b == i, name.as_str()).clicked() { *body_b = i; }
                        }
                    });
                });
                ui.add(egui::Slider::new(min_distance, 0.0..=*max_distance).text("Min Distance").suffix(" m"));
                ui.add(egui::Slider::new(max_distance, *min_distance..=50.0).text("Max Distance").suffix(" m"));
            }

            Joint::Pulley { body_a, body_b, anchor_a, anchor_b, ratio } => {
                ui.horizontal(|ui| {
                    ui.label("Body A:");
                    egui::ComboBox::from_id_salt("ji_pulley_a").selected_text(body_names.get(*body_a).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_a == i, name.as_str()).clicked() { *body_a = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Body B:");
                    egui::ComboBox::from_id_salt("ji_pulley_b").selected_text(body_names.get(*body_b).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(*body_b == i, name.as_str()).clicked() { *body_b = i; }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Anchor A:");
                    ui.add(egui::DragValue::new(&mut anchor_a[0]).prefix("x:").speed(0.01));
                    ui.add(egui::DragValue::new(&mut anchor_a[1]).prefix("y:").speed(0.01));
                });
                ui.horizontal(|ui| {
                    ui.label("Anchor B:");
                    ui.add(egui::DragValue::new(&mut anchor_b[0]).prefix("x:").speed(0.01));
                    ui.add(egui::DragValue::new(&mut anchor_b[1]).prefix("y:").speed(0.01));
                });
                ui.add(egui::Slider::new(ratio, 0.1..=10.0).text("Ratio").logarithmic(true));
            }
        }
    });
}

fn show_material_list(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    ui.horizontal(|ui| {
        ui.strong("Physics Materials");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ Add").clicked() {
                editor.materials.push(PhysicsMaterial::new(&editor.new_material_name.clone()));
                editor.selected_material = Some(editor.materials.len() - 1);
            }
        });
    });
    ui.add(egui::TextEdit::singleline(&mut editor.new_material_name).hint_text("Material name...").desired_width(f32::INFINITY));
    ui.separator();

    let mut to_delete: Option<usize> = None;
    egui::ScrollArea::vertical().id_salt("material_list_scroll").show(ui, |ui| {
        let mat_count = editor.materials.len();
        for mi in 0..mat_count {
            let mat = &editor.materials[mi];
            let is_sel = editor.selected_material == Some(mi);
            let mat_color = Color32::from_rgb(mat.color[0], mat.color[1], mat.color[2]);

            ui.push_id(mi, |ui| {
                egui::Frame::none()
                    .fill(if is_sel { Color32::from_rgb(40, 50, 70) } else { Color32::from_rgb(28, 28, 35) })
                    .stroke(Stroke::new(1.0, if is_sel { mat_color } else { Color32::from_rgb(45, 45, 55) }))
                    .inner_margin(4.0)
                    .corner_radius(3.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot_rect.center(), 5.0, mat_color);
                            ui.label(RichText::new(&mat.name).color(if is_sel { Color32::WHITE } else { Color32::LIGHT_GRAY }));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("x").clicked() { to_delete = Some(mi); }
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("R:{:.2} F:{:.2}", mat.restitution, mat.friction)).small().color(Color32::GRAY));
                        });
                    });
                if ui.interact(ui.min_rect(), egui::Id::new("mat_click").with(mi), egui::Sense::click()).clicked() {
                    editor.selected_material = Some(mi);
                }
                ui.add_space(2.0);
            });
        }
    });

    if let Some(idx) = to_delete {
        editor.materials.remove(idx);
        if editor.selected_material == Some(idx) { editor.selected_material = None; }
    }
}

fn show_material_inspector(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    if let Some(mi) = editor.selected_material {
        if mi < editor.materials.len() {
            let mat = &mut editor.materials[mi];
            ui.heading(RichText::new("Material Inspector").size(14.0));
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut mat.name);
            });
            ui.add(egui::Slider::new(&mut mat.restitution, 0.0..=1.0).text("Restitution (Bounciness)"));
            ui.add(egui::Slider::new(&mut mat.friction, 0.0..=1.0).text("Friction"));
            ui.horizontal(|ui| {
                ui.label("Combine Mode:");
                egui::ComboBox::from_id_salt("mat_combine")
                    .selected_text(mat.combine_mode.label())
                    .show_ui(ui, |ui| {
                        for mode in CombineMode::all() {
                            if ui.selectable_label(&mat.combine_mode == mode, mode.label()).clicked() {
                                mat.combine_mode = mode.clone();
                            }
                        }
                    });
            });

            // Bounce preview
            ui.separator();
            ui.label("Bounce Preview:");
            let (preview_rect, _) = ui.allocate_exact_size(Vec2::new(150.0, 100.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(preview_rect, 2.0, Color32::from_rgb(20, 20, 25));
            painter.line_segment(
                [Pos2::new(preview_rect.min.x, preview_rect.max.y - 5.0), Pos2::new(preview_rect.max.x, preview_rect.max.y - 5.0)],
                Stroke::new(2.0, Color32::GRAY),
            );
            let mat_color = Color32::from_rgb(mat.color[0], mat.color[1], mat.color[2]);
            let ball_radius = 8.0;
            let drop_height = preview_rect.height() - 20.0;
            for bounce in 0..5 {
                let t = bounce as f32 * 0.15 + 0.05;
                let x = preview_rect.min.x + t * preview_rect.width();
                let bounce_h = drop_height * mat.restitution.powi(bounce as i32);
                let y = preview_rect.max.y - 5.0 - ball_radius;
                if x < preview_rect.max.x {
                    painter.circle_filled(Pos2::new(x, y - bounce_h * 0.5), ball_radius.min(bounce_h * 0.5 + 2.0), mat_color);
                }
            }
        }
    } else {
        ui.label(RichText::new("Select a material to edit").color(Color32::GRAY));
    }
}

fn show_collision_matrix(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    ui.strong("Collision Matrix");
    ui.label(RichText::new("Check = layers collide").small().color(Color32::GRAY));
    ui.separator();

    egui::ScrollArea::both().id_salt("collision_matrix_scroll").show(ui, |ui| {
        let cell_size = 18.0;
        let label_w = 80.0;
        let total_w = label_w + 16.0 * cell_size + 20.0;

        // Column headers (rotated label simulation — just short names)
        ui.horizontal(|ui| {
            ui.add_space(label_w);
            for col in 0..16 {
                let (cell_rect, _) = ui.allocate_exact_size(Vec2::new(cell_size, cell_size), egui::Sense::hover());
                let short = format!("{}", col);
                ui.painter().text(
                    cell_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &short,
                    FontId::proportional(7.0),
                    Color32::GRAY,
                );
            }
        });

        for row in 0..16 {
            ui.horizontal(|ui| {
                let label_name = editor.layer_names[row].chars().take(10).collect::<String>();
                ui.add_sized(Vec2::new(label_w, cell_size), egui::Label::new(RichText::new(label_name).small()));
                for col in 0..16 {
                    let val = editor.collision_matrix[row][col];
                    let (cell_rect, cell_resp) = ui.allocate_exact_size(Vec2::new(cell_size, cell_size), egui::Sense::click());
                    let fill = if row == col {
                        Color32::from_rgb(60, 60, 80)
                    } else if val {
                        Color32::from_rgb(60, 160, 80)
                    } else {
                        Color32::from_rgb(40, 40, 40)
                    };
                    ui.painter().rect_filled(cell_rect.shrink(1.0), 2.0, fill);
                    if row != col && cell_resp.clicked() {
                        editor.collision_matrix[row][col] = !val;
                        editor.collision_matrix[col][row] = !val;
                    }
                }
            });
        }
    });
}

fn show_physics_preview(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
    let rect = response.rect;

    painter.rect_filled(rect, 0.0, Color32::from_rgb(15, 15, 20));

    // Grid
    let zoom = editor.canvas_zoom;
    let offset = editor.canvas_offset;
    let grid_units = 1.0_f32;
    let grid_px = grid_units * zoom;

    // Calculate grid start
    let world_min_x = (-offset.x) / zoom;
    let world_max_x = (rect.width() - offset.x) / zoom;
    let world_min_y = (-offset.y) / zoom;
    let world_max_y = (rect.height() - offset.y) / zoom;

    let gx_start = (world_min_x / grid_units).floor() as i32;
    let gx_end = (world_max_x / grid_units).ceil() as i32;
    let gy_start = (world_min_y / grid_units).floor() as i32;
    let gy_end = (world_max_y / grid_units).ceil() as i32;

    for gx in gx_start..=gx_end {
        let sx = rect.min.x + gx as f32 * zoom + offset.x;
        let is_major = gx % 5 == 0;
        painter.line_segment(
            [Pos2::new(sx, rect.min.y), Pos2::new(sx, rect.max.y)],
            Stroke::new(if is_major { 0.8 } else { 0.3 }, if is_major { Color32::from_rgb(45, 45, 55) } else { Color32::from_rgb(30, 30, 38) }),
        );
    }
    for gy in gy_start..=gy_end {
        let sy = rect.min.y + gy as f32 * zoom + offset.y;
        let is_major = gy % 5 == 0;
        painter.line_segment(
            [Pos2::new(rect.min.x, sy), Pos2::new(rect.max.x, sy)],
            Stroke::new(if is_major { 0.8 } else { 0.3 }, if is_major { Color32::from_rgb(45, 45, 55) } else { Color32::from_rgb(30, 30, 38) }),
        );
    }

    // Origin axes
    let origin = Pos2::new(rect.min.x + offset.x, rect.min.y + offset.y);
    painter.line_segment([Pos2::new(rect.min.x, origin.y), Pos2::new(rect.max.x, origin.y)], Stroke::new(1.0, Color32::from_rgb(60, 60, 80)));
    painter.line_segment([Pos2::new(origin.x, rect.min.y), Pos2::new(origin.x, rect.max.y)], Stroke::new(1.0, Color32::from_rgb(60, 60, 80)));

    let world_to_screen = |wx: f32, wy: f32| -> Pos2 {
        Pos2::new(
            rect.min.x + wx * zoom + offset.x,
            rect.min.y - wy * zoom + offset.y,
        )
    };

    // Pan
    if response.dragged_by(egui::PointerButton::Secondary) {
        editor.canvas_offset += response.drag_delta();
    }

    // Zoom
    if let Some(pos) = response.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = 1.0 + scroll * 0.002;
            let new_zoom = (editor.canvas_zoom * factor).clamp(5.0, 200.0);
            let zoom_ratio = new_zoom / editor.canvas_zoom;
            let local = pos - rect.min;
            editor.canvas_offset = local - (local - editor.canvas_offset) * zoom_ratio;
            editor.canvas_zoom = new_zoom;
        }
    }

    // Draw joints
    if editor.show_joints_in_preview {
        for joint in &editor.joints {
            let ba = joint.body_a();
            let bb = joint.body_b();
            if ba < editor.bodies.len() && bb < editor.bodies.len() {
                let pa = world_to_screen(editor.bodies[ba].position[0], editor.bodies[ba].position[1]);
                let pb = world_to_screen(editor.bodies[bb].position[0], editor.bodies[bb].position[1]);
                let jcolor = joint.color();

                match joint {
                    Joint::Spring { rest_length, .. } => {
                        // Draw spring coils
                        let dir = (pb - pa).normalized();
                        let perp = Vec2::new(-dir.y, dir.x);
                        let total_len = (pb - pa).length();
                        let coils = 8;
                        let coil_amp = 6.0_f32;
                        let mut prev = pa;
                        for c in 0..=(coils * 2) {
                            let t = c as f32 / (coils * 2) as f32;
                            let along = t * total_len;
                            let sine_val = (t * coils as f32 * std::f32::consts::PI).sin();
                            let pt = pa + dir * along + perp * (sine_val * coil_amp);
                            painter.line_segment([prev, pt], Stroke::new(1.5, jcolor));
                            prev = pt;
                        }
                    }
                    Joint::Distance { min_distance, max_distance, .. } => {
                        painter.line_segment([pa, pb], Stroke::new(1.0, jcolor));
                        // Draw min/max circles at both ends
                        let min_px = *min_distance * zoom;
                        let max_px = *max_distance * zoom;
                        painter.circle_stroke(pa, min_px, Stroke::new(0.5, Color32::from_rgba_unmultiplied(jcolor.r(), jcolor.g(), jcolor.b(), 80)));
                        painter.circle_stroke(pa, max_px, Stroke::new(0.5, Color32::from_rgba_unmultiplied(jcolor.r(), jcolor.g(), jcolor.b(), 120)));
                    }
                    Joint::Pulley { anchor_a, anchor_b, .. } => {
                        let paa = world_to_screen(anchor_a[0], anchor_a[1]);
                        let pab = world_to_screen(anchor_b[0], anchor_b[1]);
                        painter.line_segment([pa, paa], Stroke::new(1.0, jcolor));
                        painter.line_segment([pb, pab], Stroke::new(1.0, jcolor));
                        painter.line_segment([paa, pab], Stroke::new(1.0, Color32::from_rgba_unmultiplied(jcolor.r(), jcolor.g(), jcolor.b(), 120)));
                        painter.circle_filled(paa, 4.0, jcolor);
                        painter.circle_filled(pab, 4.0, jcolor);
                    }
                    Joint::Hinge { anchor, .. } => {
                        let panch = world_to_screen(anchor[0], anchor[1]);
                        painter.line_segment([pa, pb], Stroke::new(1.0, Color32::from_rgba_unmultiplied(jcolor.r(), jcolor.g(), jcolor.b(), 80)));
                        painter.circle_filled(panch, 5.0, jcolor);
                        painter.circle_stroke(panch, 8.0, Stroke::new(1.0, jcolor));
                    }
                    _ => {
                        painter.line_segment([pa, pb], Stroke::new(1.5, jcolor));
                    }
                }

                // Joint label
                let mid = Pos2::new((pa.x + pb.x) / 2.0, (pa.y + pb.y) / 2.0);
                painter.text(mid + Vec2::new(4.0, -4.0), egui::Align2::LEFT_BOTTOM, joint.label(), FontId::proportional(9.0), jcolor);
            }
        }
    }

    // Draw bodies
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let pointer_pressed = ui.input(|i| i.pointer.primary_pressed());
    let mut clicked_body: Option<usize> = None;

    let body_count = editor.bodies.len();
    for bi in 0..body_count {
        let body = &editor.bodies[bi];
        let center = world_to_screen(body.position[0], body.position[1]);

        if !rect.contains(center) && zoom < 50.0 {
            // Still draw if close to edge
        }

        let is_sel = editor.selected_body == Some(bi);
        let base_color = body.body_type.color();
        let body_color = if body.sleeping {
            Color32::from_rgba_unmultiplied(base_color.r() / 2, base_color.g() / 2, base_color.b() / 2, 180)
        } else {
            base_color
        };

        let fill_color = Color32::from_rgba_unmultiplied(body_color.r() / 3, body_color.g() / 3, body_color.b() / 3, 200);
        let border_stroke = Stroke::new(if is_sel { 2.5 } else { 1.5 }, if is_sel { Color32::WHITE } else { body_color });

        if editor.show_colliders {
            let rot_rad = body.rotation.to_radians();
            let cos_r = rot_rad.cos();
            let sin_r = rot_rad.sin();

            let rotate = |lx: f32, ly: f32| -> Pos2 {
                let rx = lx * cos_r - ly * sin_r;
                let ry = lx * sin_r + ly * cos_r;
                Pos2::new(center.x + rx * zoom, center.y - ry * zoom)
            };

            match &body.collider {
                Collider::Circle { radius } => {
                    let r_px = radius * zoom;
                    painter.circle_filled(center, r_px, fill_color);
                    painter.circle_stroke(center, r_px, border_stroke);
                    // Rotation indicator
                    let rot_end = rotate(*radius * 0.8, 0.0);
                    painter.line_segment([center, rot_end], Stroke::new(1.0, body_color));
                }
                Collider::Box { width, height } => {
                    let hw = width / 2.0;
                    let hh = height / 2.0;
                    let corners = [
                        rotate(-hw, -hh),
                        rotate(hw, -hh),
                        rotate(hw, hh),
                        rotate(-hw, hh),
                    ];
                    painter.add(Shape::convex_polygon(corners.to_vec(), fill_color, border_stroke));
                }
                Collider::Capsule { radius, height } => {
                    let r_px = radius * zoom;
                    let h_px = height / 2.0 * zoom;
                    // Approximate capsule with rounded rects
                    let top = rotate(0.0, height / 2.0);
                    let bot = rotate(0.0, -height / 2.0);
                    painter.circle_filled(top, r_px, fill_color);
                    painter.circle_filled(bot, r_px, fill_color);
                    painter.circle_stroke(top, r_px, border_stroke);
                    painter.circle_stroke(bot, r_px, border_stroke);
                    // Side lines
                    let t_l = rotate(-*radius, height / 2.0);
                    let t_r = rotate(*radius, height / 2.0);
                    let b_l = rotate(-*radius, -height / 2.0);
                    let b_r = rotate(*radius, -height / 2.0);
                    painter.line_segment([t_l, b_l], border_stroke);
                    painter.line_segment([t_r, b_r], border_stroke);
                }
                Collider::Polygon { points } => {
                    if points.len() >= 3 {
                        let screen_pts: Vec<Pos2> = points.iter().map(|p| rotate(p[0], p[1])).collect();
                        painter.add(Shape::convex_polygon(screen_pts, fill_color, border_stroke));
                    }
                }
                Collider::Compound { colliders } => {
                    painter.circle_filled(center, 8.0, fill_color);
                    painter.circle_stroke(center, 8.0, border_stroke);
                }
            }
        } else {
            // Just draw a dot
            painter.circle_filled(center, 5.0, body_color);
        }

        // Body name label
        painter.text(
            center + Vec2::new(0.0, -match &body.collider {
                Collider::Circle { radius } => radius * zoom + 12.0,
                Collider::Box { height, .. } => height / 2.0 * zoom + 12.0,
                Collider::Capsule { radius, height } => (radius + height / 2.0) * zoom + 12.0,
                _ => 12.0,
            }),
            egui::Align2::CENTER_BOTTOM,
            &body.name,
            FontId::proportional(10.0),
            if is_sel { Color32::WHITE } else { Color32::GRAY },
        );

        // Velocity arrows
        if editor.show_velocities && body.body_type != BodyType::Static {
            let vx = body.velocity[0];
            let vy = body.velocity[1];
            let speed = (vx * vx + vy * vy).sqrt();
            if speed > 0.01 {
                let arrow_scale = zoom * 0.3;
                let end_x = center.x + vx * arrow_scale;
                let end_y = center.y - vy * arrow_scale;
                let end = Pos2::new(end_x, end_y);
                let vel_color = Color32::from_rgb(255, 220, 50);
                painter.line_segment([center, end], Stroke::new(2.0, vel_color));
                // Arrow head
                let dir = (end - center).normalized();
                let perp = Vec2::new(-dir.y, dir.x);
                let tip = end;
                let al = tip - dir * 8.0 + perp * 4.0;
                let ar = tip - dir * 8.0 - perp * 4.0;
                painter.add(Shape::convex_polygon(vec![tip, al, ar], vel_color, Stroke::NONE));
            }
        }

        // Click detection
        let hit_r = match &body.collider {
            Collider::Circle { radius } => radius * zoom,
            Collider::Box { width, height } => (width.max(*height) / 2.0 * zoom).max(10.0),
            _ => 15.0,
        };
        if let Some(pos) = pointer_pos {
            if (pos - center).length() < hit_r && pointer_pressed {
                clicked_body = Some(bi);
            }
        }
    }

    if let Some(bi) = clicked_body {
        editor.selected_body = Some(bi);
        editor.view = PhysicsView::Bodies;
    }

    // Simulation info overlay
    let overlay_pos = rect.min + Vec2::new(10.0, 10.0);
    let overlay_text = format!(
        "Bodies: {} | Joints: {} | Gravity: ({:.1}, {:.1}) | {}",
        editor.bodies.len(),
        editor.joints.len(),
        editor.gravity[0],
        editor.gravity[1],
        if editor.simulation_running { format!("SIM t={:.1}s", editor.simulation_time) } else { "PAUSED".to_string() }
    );
    let overlay_rect = Rect::from_min_size(overlay_pos, Vec2::new(400.0, 20.0));
    painter.rect_filled(overlay_rect, 3.0, Color32::from_rgba_unmultiplied(0, 0, 0, 160));
    painter.text(
        overlay_rect.center(),
        egui::Align2::CENTER_CENTER,
        overlay_text,
        FontId::proportional(10.0),
        Color32::from_rgb(180, 200, 180),
    );

    // Zoom indicator
    painter.text(
        rect.max - Vec2::new(60.0, 20.0),
        egui::Align2::CENTER_CENTER,
        format!("{:.0}px/m", editor.canvas_zoom),
        FontId::proportional(10.0),
        Color32::from_rgb(120, 120, 140),
    );
}

pub fn show_panel(ctx: &egui::Context, editor: &mut PhysicsEditor, dt: f32, open: &mut bool) {
    egui::Window::new("Physics Constraint Editor")
        .open(open)
        .resizable(true)
        .default_size([1200.0, 700.0])
        .min_size([800.0, 400.0])
        .show(ctx, |ui| {
            show(ui, editor, dt);
        });
}

// ---- Physics math helpers ----

pub fn vec2_length(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

pub fn vec2_normalize(v: [f32; 2]) -> [f32; 2] {
    let len = vec2_length(v);
    if len < 0.0001 { return [0.0, 0.0]; }
    [v[0] / len, v[1] / len]
}

pub fn vec2_dot(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

pub fn vec2_sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn vec2_add(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn vec2_scale(v: [f32; 2], s: f32) -> [f32; 2] {
    [v[0] * s, v[1] * s]
}

pub fn vec2_perp(v: [f32; 2]) -> [f32; 2] {
    [-v[1], v[0]]
}

pub fn vec2_cross(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

pub fn body_aabb(body: &RigidBody) -> ([f32; 2], [f32; 2]) {
    let pos = body.position;
    match &body.collider {
        Collider::Circle { radius } => {
            ([pos[0] - radius, pos[1] - radius], [pos[0] + radius, pos[1] + radius])
        }
        Collider::Box { width, height } => {
            let hw = width / 2.0;
            let hh = height / 2.0;
            let r = body.rotation.to_radians();
            let cos_r = r.cos().abs();
            let sin_r = r.sin().abs();
            let ext_x = hw * cos_r + hh * sin_r;
            let ext_y = hw * sin_r + hh * cos_r;
            ([pos[0] - ext_x, pos[1] - ext_y], [pos[0] + ext_x, pos[1] + ext_y])
        }
        Collider::Capsule { radius, height } => {
            let half_h = height / 2.0 + radius;
            ([pos[0] - radius, pos[1] - half_h], [pos[0] + radius, pos[1] + half_h])
        }
        Collider::Polygon { points } => {
            if points.is_empty() {
                return (pos, pos);
            }
            let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) + pos[0];
            let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min) + pos[1];
            let max_x = points.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max) + pos[0];
            let max_y = points.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max) + pos[1];
            ([min_x, min_y], [max_x, max_y])
        }
        _ => (pos, pos),
    }
}

pub fn aabbs_overlap(a: &([f32; 2], [f32; 2]), b: &([f32; 2], [f32; 2])) -> bool {
    a.0[0] <= b.1[0] && a.1[0] >= b.0[0] &&
    a.0[1] <= b.1[1] && a.1[1] >= b.0[1]
}

pub fn broad_phase_pairs(bodies: &[RigidBody]) -> Vec<(usize, usize)> {
    let aabbs: Vec<_> = bodies.iter().map(body_aabb).collect();
    let mut pairs = Vec::new();
    for i in 0..bodies.len() {
        for j in i + 1..bodies.len() {
            if aabbs_overlap(&aabbs[i], &aabbs[j]) {
                pairs.push((i, j));
            }
        }
    }
    pairs
}

// ---- PhysicsEditor extended methods ----

impl PhysicsEditor {
    pub fn selected_body_name(&self) -> Option<&str> {
        self.selected_body.and_then(|i| self.bodies.get(i)).map(|b| b.name.as_str())
    }

    pub fn selected_joint_label(&self) -> Option<&'static str> {
        self.selected_joint.and_then(|i| self.joints.get(i)).map(|j| j.label())
    }

    pub fn body_count_by_type(&self) -> (usize, usize, usize) {
        let static_c = self.bodies.iter().filter(|b| b.body_type == BodyType::Static).count();
        let dynamic_c = self.bodies.iter().filter(|b| b.body_type == BodyType::Dynamic).count();
        let kinematic_c = self.bodies.iter().filter(|b| b.body_type == BodyType::Kinematic).count();
        (static_c, dynamic_c, kinematic_c)
    }

    pub fn joints_for_body(&self, body_idx: usize) -> Vec<usize> {
        self.joints.iter().enumerate()
            .filter(|(_, j)| j.body_a() == body_idx || j.body_b() == body_idx)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn remove_body(&mut self, idx: usize) {
        if idx >= self.bodies.len() { return; }
        self.bodies.remove(idx);
        self.joints.retain(|j| j.body_a() != idx && j.body_b() != idx);
        if self.selected_body == Some(idx) { self.selected_body = None; }
    }

    pub fn remove_joint(&mut self, idx: usize) {
        if idx >= self.joints.len() { return; }
        self.joints.remove(idx);
        if self.selected_joint == Some(idx) { self.selected_joint = None; }
    }

    pub fn center_view_on_body(&mut self, body_idx: usize) {
        if body_idx >= self.bodies.len() { return; }
        let pos = self.bodies[body_idx].position;
        self.canvas_offset = Vec2::new(
            300.0 - pos[0] * self.canvas_zoom,
            200.0 + pos[1] * self.canvas_zoom,
        );
    }

    pub fn fit_all_bodies_in_view(&mut self, view_size: Vec2) {
        if self.bodies.is_empty() { return; }
        let min_x = self.bodies.iter().map(|b| b.position[0]).fold(f32::INFINITY, f32::min);
        let max_x = self.bodies.iter().map(|b| b.position[0]).fold(f32::NEG_INFINITY, f32::max);
        let min_y = self.bodies.iter().map(|b| b.position[1]).fold(f32::INFINITY, f32::min);
        let max_y = self.bodies.iter().map(|b| b.position[1]).fold(f32::NEG_INFINITY, f32::max);

        let world_w = (max_x - min_x).max(10.0);
        let world_h = (max_y - min_y).max(10.0);
        let zoom_x = view_size.x / world_w * 0.8;
        let zoom_y = view_size.y / world_h * 0.8;
        self.canvas_zoom = zoom_x.min(zoom_y).clamp(5.0, 100.0);

        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;
        self.canvas_offset = Vec2::new(
            view_size.x / 2.0 - cx * self.canvas_zoom,
            view_size.y / 2.0 + cy * self.canvas_zoom,
        );
    }

    pub fn snap_to_grid(&mut self, grid_size: f32) {
        for body in self.bodies.iter_mut() {
            body.position[0] = (body.position[0] / grid_size).round() * grid_size;
            body.position[1] = (body.position[1] / grid_size).round() * grid_size;
        }
    }

    pub fn total_kinetic_energy(&self) -> f32 {
        self.bodies.iter()
            .filter(|b| b.body_type == BodyType::Dynamic)
            .map(|b| {
                let speed_sq = b.velocity[0] * b.velocity[0] + b.velocity[1] * b.velocity[1];
                0.5 * b.mass * speed_sq
            })
            .sum()
    }

    pub fn apply_impulse(&mut self, body_idx: usize, impulse: [f32; 2]) {
        if body_idx >= self.bodies.len() { return; }
        let body = &mut self.bodies[body_idx];
        if body.body_type != BodyType::Dynamic { return; }
        if body.mass > 0.0 {
            body.velocity[0] += impulse[0] / body.mass;
            body.velocity[1] += impulse[1] / body.mass;
        }
    }

    pub fn apply_force_field(&mut self, center: [f32; 2], radius: f32, strength: f32) {
        for body in self.bodies.iter_mut() {
            if body.body_type != BodyType::Dynamic { continue; }
            let dx = body.position[0] - center[0];
            let dy = body.position[1] - center[1];
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < radius && dist > 0.001 {
                let force_mag = strength * (1.0 - dist / radius);
                body.velocity[0] += (dx / dist) * force_mag;
                body.velocity[1] += (dy / dist) * force_mag;
            }
        }
    }

    pub fn reset_all_velocities(&mut self) {
        for body in self.bodies.iter_mut() {
            body.velocity = [0.0, 0.0];
            body.angular_velocity = 0.0;
        }
    }

    pub fn set_all_sleeping(&mut self, sleeping: bool) {
        for body in self.bodies.iter_mut() {
            if body.body_type == BodyType::Dynamic {
                body.sleeping = sleeping;
            }
        }
    }

    pub fn layers_collide(&self, layer_a: u32, layer_b: u32) -> bool {
        let idx_a = layer_a.trailing_zeros() as usize;
        let idx_b = layer_b.trailing_zeros() as usize;
        if idx_a >= 16 || idx_b >= 16 { return true; }
        self.collision_matrix[idx_a][idx_b]
    }

    pub fn collision_pairs(&self) -> Vec<(usize, usize)> {
        let broad = broad_phase_pairs(&self.bodies);
        broad.into_iter()
            .filter(|(i, j)| {
                let la = self.bodies[*i].collision_layer;
                let lb = self.bodies[*j].collision_layer;
                self.layers_collide(la, lb)
            })
            .collect()
    }
}

// ---- Soft body simulation ----

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoftBodyNode {
    pub position: [f32; 2],
    pub prev_position: [f32; 2],
    pub pinned: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoftBodySpring {
    pub node_a: usize,
    pub node_b: usize,
    pub rest_length: f32,
    pub stiffness: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoftBody {
    pub name: String,
    pub nodes: Vec<SoftBodyNode>,
    pub springs: Vec<SoftBodySpring>,
    pub pinned_nodes: HashSet<usize>,
    pub damping: f32,
    pub gravity: [f32; 2],
}

impl SoftBody {
    pub fn new_cloth(rows: usize, cols: usize, spacing: f32) -> Self {
        let mut nodes = Vec::new();
        let mut springs = Vec::new();
        let mut pinned = HashSet::new();

        for row in 0..rows {
            for col in 0..cols {
                let x = col as f32 * spacing - (cols as f32 - 1.0) * spacing / 2.0;
                let y = -(row as f32) * spacing;
                nodes.push(SoftBodyNode {
                    position: [x, y],
                    prev_position: [x, y],
                    pinned: false,
                });
            }
        }

        // Pin top row
        for col in 0..cols {
            pinned.insert(col);
        }

        // Structural springs
        for row in 0..rows {
            for col in 0..cols {
                let idx = row * cols + col;
                if col + 1 < cols {
                    springs.push(SoftBodySpring { node_a: idx, node_b: idx + 1, rest_length: spacing, stiffness: 800.0 });
                }
                if row + 1 < rows {
                    springs.push(SoftBodySpring { node_a: idx, node_b: idx + cols, rest_length: spacing, stiffness: 800.0 });
                }
            }
        }

        // Shear springs
        for row in 0..rows - 1 {
            for col in 0..cols - 1 {
                let idx = row * cols + col;
                springs.push(SoftBodySpring { node_a: idx, node_b: idx + cols + 1, rest_length: spacing * std::f32::consts::SQRT_2, stiffness: 400.0 });
                springs.push(SoftBodySpring { node_a: idx + 1, node_b: idx + cols, rest_length: spacing * std::f32::consts::SQRT_2, stiffness: 400.0 });
            }
        }

        SoftBody {
            name: format!("Cloth {}x{}", rows, cols),
            nodes,
            springs,
            pinned_nodes: pinned,
            damping: 0.98,
            gravity: [0.0, -9.81],
        }
    }

    pub fn simulate_step(&mut self, dt: f32, iterations: usize) {
        // Verlet integration
        for (i, node) in self.nodes.iter_mut().enumerate() {
            if node.pinned { continue; }
            let vx = (node.position[0] - node.prev_position[0]) * 0.98;
            let vy = (node.position[1] - node.prev_position[1]) * 0.98;
            let new_x = node.position[0] + vx + self.gravity[0] * dt * dt;
            let new_y = node.position[1] + vy + self.gravity[1] * dt * dt;
            node.prev_position = node.position;
            node.position = [new_x, new_y];
        }

        // Constraint solving
        for _ in 0..iterations {
            let spring_data: Vec<(usize, usize, f32, f32)> = self.springs.iter()
                .map(|s| (s.node_a, s.node_b, s.rest_length, s.stiffness))
                .collect();
            for (a, b, rest, stiff) in spring_data {
                if a >= self.nodes.len() || b >= self.nodes.len() { continue; }
                let dx = self.nodes[b].position[0] - self.nodes[a].position[0];
                let dy = self.nodes[b].position[1] - self.nodes[a].position[1];
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 0.0001 { continue; }
                let error = (dist - rest) / dist;
                let correction_x = dx * error * 0.5;
                let correction_y = dy * error * 0.5;
                if !self.nodes[a].pinned && !self.pinned_nodes.contains(&a) {
                    self.nodes[a].position[0] += correction_x;
                    self.nodes[a].position[1] += correction_y;
                }
                if !self.nodes[b].pinned && !self.pinned_nodes.contains(&b) {
                    self.nodes[b].position[0] -= correction_x;
                    self.nodes[b].position[1] -= correction_y;
                }
            }
        }
    }
}

pub fn draw_soft_body(painter: &Painter, soft_body: &SoftBody, world_to_screen: &dyn Fn(f32, f32) -> Pos2) {
    // Draw springs
    for spring in &soft_body.springs {
        if spring.node_a >= soft_body.nodes.len() || spring.node_b >= soft_body.nodes.len() { continue; }
        let pa = world_to_screen(soft_body.nodes[spring.node_a].position[0], soft_body.nodes[spring.node_a].position[1]);
        let pb = world_to_screen(soft_body.nodes[spring.node_b].position[0], soft_body.nodes[spring.node_b].position[1]);
        painter.line_segment([pa, pb], Stroke::new(1.0, Color32::from_rgb(100, 150, 200)));
    }

    // Draw nodes
    for (i, node) in soft_body.nodes.iter().enumerate() {
        let p = world_to_screen(node.position[0], node.position[1]);
        let color = if soft_body.pinned_nodes.contains(&i) || node.pinned {
            Color32::from_rgb(255, 100, 100)
        } else {
            Color32::from_rgb(150, 200, 255)
        };
        painter.circle_filled(p, 3.0, color);
    }
}

// ---- Physics statistics panel ----

pub fn show_physics_stats(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    let (static_c, dynamic_c, kinematic_c) = editor.body_count_by_type();
    let ke = editor.total_kinetic_energy();
    let pairs = editor.collision_pairs();

    egui::CollapsingHeader::new(RichText::new("Physics Statistics").color(Color32::from_rgb(180, 220, 120)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("physics_stats_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Static Bodies:");
                    ui.label(RichText::new(format!("{}", static_c)).color(BodyType::Static.color()));
                    ui.end_row();
                    ui.label("Dynamic Bodies:");
                    ui.label(RichText::new(format!("{}", dynamic_c)).color(BodyType::Dynamic.color()));
                    ui.end_row();
                    ui.label("Kinematic Bodies:");
                    ui.label(RichText::new(format!("{}", kinematic_c)).color(BodyType::Kinematic.color()));
                    ui.end_row();
                    ui.label("Joints:");
                    ui.label(format!("{}", editor.joints.len()));
                    ui.end_row();
                    ui.label("Materials:");
                    ui.label(format!("{}", editor.materials.len()));
                    ui.end_row();
                    ui.label("Kinetic Energy:");
                    ui.label(format!("{:.2} J", ke));
                    ui.end_row();
                    ui.label("Broad Phase Pairs:");
                    ui.label(format!("{}", pairs.len()));
                    ui.end_row();
                    ui.label("Sim Time:");
                    ui.label(format!("{:.2}s", editor.simulation_time));
                    ui.end_row();
                });
        });
}

// ---- Joint visualization helpers ----

pub fn draw_hinge_arc(painter: &Painter, center: Pos2, radius: f32, lower_angle_deg: f32, upper_angle_deg: f32, color: Color32) {
    let steps = 32_usize;
    let lower_rad = lower_angle_deg.to_radians();
    let upper_rad = upper_angle_deg.to_radians();
    let angle_range = upper_rad - lower_rad;

    let mut prev: Option<Pos2> = None;
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let angle = lower_rad + angle_range * t;
        let x = center.x + radius * angle.cos();
        let y = center.y - radius * angle.sin();
        let pt = Pos2::new(x, y);
        if let Some(p) = prev {
            painter.line_segment([p, pt], Stroke::new(1.5, color));
        } else {
            painter.line_segment([center, pt], Stroke::new(1.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 100)));
        }
        prev = Some(pt);
    }
    if let Some(last) = prev {
        painter.line_segment([center, last], Stroke::new(1.0, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 100)));
    }
}

pub fn draw_slider_limits(painter: &Painter, from: Pos2, axis: [f32; 2], lower: f32, upper: f32, zoom: f32, color: Color32) {
    let ax = Vec2::new(axis[0], -axis[1]).normalized();
    let perp = Vec2::new(-ax.y, ax.x);

    let low_pt = from + ax * lower * zoom;
    let high_pt = from + ax * upper * zoom;

    painter.line_segment([low_pt, high_pt], Stroke::new(1.5, color));

    let tick_size = 5.0;
    painter.line_segment([low_pt - perp * tick_size, low_pt + perp * tick_size], Stroke::new(1.5, color));
    painter.line_segment([high_pt - perp * tick_size, high_pt + perp * tick_size], Stroke::new(1.5, color));
}

// ---- Material comparison ----

pub fn show_material_comparison(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new("Material Comparison")
        .default_open(false)
        .show(ui, |ui| {
            if editor.materials.is_empty() {
                ui.label("No materials defined.");
                return;
            }
            egui::Grid::new("mat_compare_grid")
                .num_columns(4)
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.strong("Name");
                    ui.strong("Restitution");
                    ui.strong("Friction");
                    ui.strong("Mode");
                    ui.end_row();

                    for mat in &editor.materials {
                        let mc = Color32::from_rgb(mat.color[0], mat.color[1], mat.color[2]);
                        ui.label(RichText::new(&mat.name).color(mc));

                        let rest_bar_w = 60.0;
                        let (rest_rect, _) = ui.allocate_exact_size(Vec2::new(rest_bar_w, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(rest_rect, 2.0, Color32::from_rgb(25, 25, 30));
                        let fill = Rect::from_min_size(rest_rect.min, Vec2::new(rest_bar_w * mat.restitution, rest_rect.height()));
                        ui.painter().rect_filled(fill, 2.0, Color32::from_rgb(100, 180, 255));

                        let fric_bar_w = 60.0;
                        let (fric_rect, _) = ui.allocate_exact_size(Vec2::new(fric_bar_w, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(fric_rect, 2.0, Color32::from_rgb(25, 25, 30));
                        let fill2 = Rect::from_min_size(fric_rect.min, Vec2::new(fric_bar_w * mat.friction, fric_rect.height()));
                        ui.painter().rect_filled(fill2, 2.0, Color32::from_rgb(255, 180, 80));

                        ui.label(RichText::new(mat.combine_mode.label()).small().color(Color32::GRAY));
                        ui.end_row();
                    }
                });
        });
}

// ---- Body proximity display ----

pub fn show_body_proximity_table(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    if let Some(sel_idx) = editor.selected_body {
        egui::CollapsingHeader::new("Nearby Bodies")
            .default_open(false)
            .show(ui, |ui| {
                let sel_pos = editor.bodies[sel_idx].position;
                let mut others: Vec<(usize, f32)> = editor.bodies.iter().enumerate()
                    .filter(|(i, _)| *i != sel_idx)
                    .map(|(i, b)| {
                        let dx = b.position[0] - sel_pos[0];
                        let dy = b.position[1] - sel_pos[1];
                        (i, (dx * dx + dy * dy).sqrt())
                    })
                    .collect();
                others.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                egui::Grid::new("proximity_grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Body");
                        ui.strong("Distance");
                        ui.strong("Connected?");
                        ui.end_row();

                        for (idx, dist) in others.iter().take(6) {
                            let body = &editor.bodies[*idx];
                            let type_color = body.body_type.color();
                            ui.label(RichText::new(&body.name).color(type_color));
                            ui.label(format!("{:.2}m", dist));
                            let connected = editor.joints.iter().any(|j| {
                                (j.body_a() == sel_idx && j.body_b() == *idx) ||
                                (j.body_b() == sel_idx && j.body_a() == *idx)
                            });
                            ui.label(if connected { RichText::new("Yes").color(Color32::from_rgb(100, 220, 100)) } else { RichText::new("No").color(Color32::GRAY) });
                            ui.end_row();
                        }
                    });
            });
    }
}

// ---- Gravity field visualization ----

pub fn draw_gravity_field(painter: &Painter, rect: Rect, gravity: [f32; 2], canvas_offset: Vec2, zoom: f32) {
    let gx = gravity[0];
    let gy = gravity[1];
    let g_len = (gx * gx + gy * gy).sqrt();
    if g_len < 0.001 { return; }

    let arrow_spacing = 60.0_f32;
    let cols = (rect.width() / arrow_spacing).ceil() as i32;
    let rows = (rect.height() / arrow_spacing).ceil() as i32;

    for row in 0..=rows {
        for col in 0..=cols {
            let sx = rect.min.x + col as f32 * arrow_spacing + arrow_spacing / 2.0;
            let sy = rect.min.y + row as f32 * arrow_spacing + arrow_spacing / 2.0;
            let origin = Pos2::new(sx, sy);
            let arrow_len = 18.0;
            let dx = gx / g_len * arrow_len;
            let dy = -gy / g_len * arrow_len;
            let end = Pos2::new(sx + dx, sy + dy);

            let alpha = 50_u8;
            let color = Color32::from_rgba_unmultiplied(150, 150, 200, alpha);
            painter.line_segment([origin, end], Stroke::new(0.8, color));

            let dir = Vec2::new(dx, dy).normalized();
            let perp = Vec2::new(-dir.y, dir.x);
            let tip = end;
            let al = tip - dir * 5.0 + perp * 2.5;
            let ar = tip - dir * 5.0 - perp * 2.5;
            painter.add(Shape::convex_polygon(
                vec![tip, al, ar],
                color,
                Stroke::NONE,
            ));
        }
    }
}

// ---- Layer mask editor ----

pub fn show_layer_mask_editor(ui: &mut egui::Ui, layer: &mut u32, mask: &mut u32, layer_names: &[String; 16]) {
    ui.horizontal(|ui| {
        ui.label("Collision Layer:");
        for i in 0..16 {
            let bit = 1u32 << i;
            let is_set = (*layer & bit) != 0;
            let mut checked = is_set;
            let label = format!("{}", i);
            if ui.checkbox(&mut checked, "").on_hover_text(&layer_names[i]).changed() {
                if checked {
                    *layer |= bit;
                } else {
                    *layer &= !bit;
                }
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Collision Mask:");
        for i in 0..16 {
            let bit = 1u32 << i;
            let is_set = (*mask & bit) != 0;
            let mut checked = is_set;
            if ui.checkbox(&mut checked, "").on_hover_text(&layer_names[i]).changed() {
                if checked {
                    *mask |= bit;
                } else {
                    *mask &= !bit;
                }
            }
        }
    });
}

// ---- Constraint solver display ----

pub fn show_constraint_details(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    if let Some(ji) = editor.selected_joint {
        if ji >= editor.joints.len() { return; }
        let joint = &editor.joints[ji];
        let ba = joint.body_a();
        let bb = joint.body_b();

        if ba >= editor.bodies.len() || bb >= editor.bodies.len() { return; }

        let pos_a = editor.bodies[ba].position;
        let pos_b = editor.bodies[bb].position;
        let dx = pos_b[0] - pos_a[0];
        let dy = pos_b[1] - pos_a[1];
        let dist = (dx * dx + dy * dy).sqrt();

        ui.separator();
        ui.strong("Runtime Constraint Info");
        egui::Grid::new("constraint_info_grid")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Body A Position:");
                ui.label(format!("({:.2}, {:.2})", pos_a[0], pos_a[1]));
                ui.end_row();
                ui.label("Body B Position:");
                ui.label(format!("({:.2}, {:.2})", pos_b[0], pos_b[1]));
                ui.end_row();
                ui.label("Current Distance:");
                ui.label(format!("{:.3} m", dist));
                ui.end_row();

                match joint {
                    Joint::Spring { rest_length, stiffness, damping, .. } => {
                        let extension = dist - rest_length;
                        let force = stiffness * extension;
                        ui.label("Extension:");
                        ui.label(format!("{:.3} m", extension));
                        ui.end_row();
                        ui.label("Spring Force:");
                        ui.label(format!("{:.1} N", force));
                        ui.end_row();
                    }
                    Joint::Distance { min_distance, max_distance, .. } => {
                        let in_range = dist >= *min_distance && dist <= *max_distance;
                        ui.label("In Range:");
                        ui.label(if in_range { RichText::new("Yes").color(Color32::from_rgb(100, 220, 100)) } else { RichText::new("No — Constraint Active").color(Color32::from_rgb(255, 180, 50)) });
                        ui.end_row();
                    }
                    _ => {}
                }
            });
    }
}

// ---- Full physics editor window ----

pub fn show_full_editor(ctx: &egui::Context, editor: &mut PhysicsEditor, soft_bodies: &mut Vec<SoftBody>, dt: f32, open: &mut bool) {
    egui::Window::new("Physics System — Full Editor")
        .open(open)
        .resizable(true)
        .default_size([1400.0, 800.0])
        .min_size([900.0, 500.0])
        .show(ctx, |ui| {
            egui::TopBottomPanel::top("phys_full_top")
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(RichText::new("Physics Constraint Editor").size(16.0).color(Color32::from_rgb(180, 255, 120)));
                        ui.separator();

                        let (static_c, dynamic_c, kinematic_c) = editor.body_count_by_type();
                        ui.label(RichText::new(format!(
                            "S:{} D:{} K:{} | Joints:{} | Mats:{}",
                            static_c, dynamic_c, kinematic_c,
                            editor.joints.len(),
                            editor.materials.len(),
                        )).small().color(Color32::GRAY));

                        ui.separator();
                        if ui.button("Fit View").clicked() {
                            editor.fit_all_bodies_in_view(Vec2::new(600.0, 400.0));
                        }
                        if ui.button("Snap to Grid 0.5").clicked() {
                            editor.snap_to_grid(0.5);
                        }
                        if ui.button("Reset Velocities").clicked() {
                            editor.reset_all_velocities();
                        }
                        if ui.button("Wake All").clicked() {
                            editor.set_all_sleeping(false);
                        }
                    });
                });

            egui::TopBottomPanel::bottom("phys_full_bottom")
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        let ke = editor.total_kinetic_energy();
                        ui.label(RichText::new(format!(
                            "KE: {:.2} J | Pairs: {} | t: {:.2}s",
                            ke,
                            editor.collision_pairs().len(),
                            editor.simulation_time,
                        )).small().color(Color32::from_rgb(150, 200, 150)));
                    });
                });

            egui::SidePanel::left("phys_full_sidebar")
                .resizable(true)
                .default_width(220.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("phys_sidebar_scroll").show(ui, |ui| {
                        show_physics_stats(ui, editor);
                        ui.separator();
                        show_material_comparison(ui, editor);
                        ui.separator();
                        show_body_proximity_table(ui, editor);
                        ui.separator();
                        show_constraint_details(ui, editor);
                    });
                });

            show(ui, editor, dt);
        });
}

// ---- Body velocity editor widget ----

pub fn show_velocity_editor(ui: &mut egui::Ui, body: &mut RigidBody) {
    if body.body_type == BodyType::Static { return; }

    ui.separator();
    ui.strong("Velocity Control");

    let speed = vec2_length(body.velocity);
    let speed_bar_w = 150.0;
    let (speed_rect, _) = ui.allocate_exact_size(Vec2::new(speed_bar_w, 14.0), egui::Sense::hover());
    let max_speed = 20.0_f32;
    ui.painter().rect_filled(speed_rect, 2.0, Color32::from_rgb(25, 25, 30));
    let fill = Rect::from_min_size(speed_rect.min, Vec2::new((speed / max_speed).min(1.0) * speed_bar_w, speed_rect.height()));
    let speed_color = if speed > 10.0 { Color32::from_rgb(220, 80, 80) } else if speed > 5.0 { Color32::from_rgb(220, 180, 50) } else { Color32::from_rgb(80, 180, 255) };
    ui.painter().rect_filled(fill, 2.0, speed_color);
    ui.label(RichText::new(format!("Speed: {:.2} m/s", speed)).small().color(Color32::GRAY));

    ui.horizontal(|ui| {
        ui.label("Vel X:");
        ui.add(egui::DragValue::new(&mut body.velocity[0]).speed(0.01));
        ui.label("Y:");
        ui.add(egui::DragValue::new(&mut body.velocity[1]).speed(0.01));
    });
    ui.horizontal(|ui| {
        ui.label("Ang. Vel:");
        ui.add(egui::DragValue::new(&mut body.angular_velocity).suffix(" rad/s").speed(0.01));
        if ui.small_button("Stop").clicked() {
            body.velocity = [0.0, 0.0];
            body.angular_velocity = 0.0;
        }
    });
}

// ---- Body transform gizmo (2D) ----

pub fn draw_body_gizmo(painter: &Painter, center: Pos2, rotation_deg: f32, zoom: f32, is_selected: bool) {
    if !is_selected { return; }

    let rot_rad = rotation_deg.to_radians();
    let axis_len = 20.0;

    // X axis (right, red)
    let x_dir = Vec2::new(rot_rad.cos(), -rot_rad.sin());
    let x_end = center + x_dir * axis_len;
    painter.line_segment([center, x_end], Stroke::new(2.0, Color32::from_rgb(220, 80, 80)));
    painter.text(x_end + x_dir * 4.0, egui::Align2::CENTER_CENTER, "X", FontId::proportional(9.0), Color32::from_rgb(220, 80, 80));

    // Y axis (up, green — inverted because screen Y is down)
    let y_dir = Vec2::new(rot_rad.sin(), -rot_rad.cos());
    let y_end = center + y_dir * axis_len;
    painter.line_segment([center, y_end], Stroke::new(2.0, Color32::from_rgb(80, 220, 80)));
    painter.text(y_end + y_dir * 4.0, egui::Align2::CENTER_CENTER, "Y", FontId::proportional(9.0), Color32::from_rgb(80, 220, 80));

    // Center dot
    painter.circle_filled(center, 4.0, Color32::WHITE);
}

// ---- Joint creation wizard ----

pub fn show_joint_creation_wizard(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Joint Wizard").color(Color32::from_rgb(100, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label("Select two bodies and a joint type to create a new constraint.");

            let body_names: Vec<String> = editor.bodies.iter().map(|b| b.name.clone()).collect();

            ui.horizontal(|ui| {
                ui.label("Body A:");
                let name_a = body_names.get(editor.new_joint_body_a).cloned().unwrap_or_else(|| "None".to_string());
                egui::ComboBox::from_id_salt("wizard_body_a")
                    .selected_text(name_a)
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(i == editor.new_joint_body_a, name.as_str()).clicked() {
                                editor.new_joint_body_a = i;
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Body B:");
                let name_b = body_names.get(editor.new_joint_body_b).cloned().unwrap_or_else(|| "None".to_string());
                egui::ComboBox::from_id_salt("wizard_body_b")
                    .selected_text(name_b)
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            if ui.selectable_label(i == editor.new_joint_body_b, name.as_str()).clicked() {
                                editor.new_joint_body_b = i;
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::from_id_salt("wizard_joint_type")
                    .selected_text(Joint::type_labels()[editor.new_joint_type])
                    .show_ui(ui, |ui| {
                        for (i, label) in Joint::type_labels().iter().enumerate() {
                            if ui.selectable_label(i == editor.new_joint_type, *label).clicked() {
                                editor.new_joint_type = i;
                            }
                        }
                    });
            });

            // Preview description
            let desc = match editor.new_joint_type {
                0 => "Fixed: Rigidly connects two bodies. No relative motion allowed.",
                1 => "Hinge: Allows rotation around a single axis (2D pivot).",
                2 => "Slider: Constrains motion to a single axis direction.",
                3 => "Spring: Elastic constraint with configurable stiffness and damping.",
                4 => "Distance: Maintains a minimum/maximum separation between bodies.",
                _ => "Pulley: Connects two bodies through a rope over a pulley system.",
            };
            ui.label(RichText::new(desc).small().color(Color32::GRAY).italics());

            let can_create = editor.new_joint_body_a != editor.new_joint_body_b
                && editor.new_joint_body_a < editor.bodies.len()
                && editor.new_joint_body_b < editor.bodies.len();

            ui.add_enabled_ui(can_create, |ui| {
                if ui.button("Create Joint").clicked() {
                    let a = editor.new_joint_body_a;
                    let b = editor.new_joint_body_b;
                    let j = Joint::default_for_index(editor.new_joint_type, a, b);
                    editor.joints.push(j);
                    editor.selected_joint = Some(editor.joints.len() - 1);
                    editor.view = PhysicsView::Joints;
                }
            });
        });
}

// ---- Simulation control panel ----

pub fn show_simulation_controls(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::Frame::none()
        .fill(Color32::from_rgb(22, 28, 35))
        .stroke(Stroke::new(1.0, Color32::from_rgb(50, 60, 70)))
        .inner_margin(6.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let run_color = if editor.simulation_running { Color32::from_rgb(100, 220, 100) } else { Color32::from_rgb(220, 100, 100) };
                if ui.add(egui::Button::new(
                    RichText::new(if editor.simulation_running { "Pause" } else { "Simulate" }).color(run_color)
                )).clicked() {
                    editor.simulation_running = !editor.simulation_running;
                }
                if ui.button("Step").clicked() {
                    editor.simulate_step(1.0 / 60.0);
                }
                if ui.button("Reset").clicked() {
                    editor.simulation_running = false;
                    editor.simulation_time = 0.0;
                    editor.reset_all_velocities();
                }
                ui.separator();
                ui.label(RichText::new(format!("t={:.2}s", editor.simulation_time)).monospace().color(Color32::GRAY).small());
            });
            ui.horizontal(|ui| {
                ui.label("Gravity:");
                ui.add(egui::DragValue::new(&mut editor.gravity[0]).prefix("X:").speed(0.01).range(-50.0..=50.0));
                ui.add(egui::DragValue::new(&mut editor.gravity[1]).prefix("Y:").speed(0.01).range(-50.0..=50.0));
                if ui.small_button("Earth").clicked() { editor.gravity = [0.0, -9.81]; }
                if ui.small_button("Moon").clicked() { editor.gravity = [0.0, -1.62]; }
                if ui.small_button("Zero-G").clicked() { editor.gravity = [0.0, 0.0]; }
            });
        });
}

// ---- World settings panel ----

pub fn show_world_settings(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("World Settings").color(Color32::from_rgb(180, 200, 255)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Gravity X:");
                ui.add(egui::Slider::new(&mut editor.gravity[0], -50.0..=50.0));
            });
            ui.horizontal(|ui| {
                ui.label("Gravity Y:");
                ui.add(egui::Slider::new(&mut editor.gravity[1], -50.0..=50.0));
            });

            ui.separator();
            ui.label("Presets:");
            ui.horizontal(|ui| {
                if ui.button("Earth").clicked() { editor.gravity = [0.0, -9.81]; }
                if ui.button("Moon").clicked() { editor.gravity = [0.0, -1.62]; }
                if ui.button("Mars").clicked() { editor.gravity = [0.0, -3.72]; }
                if ui.button("Zero-G").clicked() { editor.gravity = [0.0, 0.0]; }
                if ui.button("Sideways").clicked() { editor.gravity = [-9.81, 0.0]; }
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Simulation:");
                ui.checkbox(&mut editor.simulation_running, "Running");
            });
        });
}

// ---- Collision layer quick toggle ----

pub fn show_layer_quick_toggle(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Layer Visibility").color(Color32::from_rgb(200, 180, 255)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("layer_vis_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for i in 0..8 {
                        ui.label(RichText::new(&editor.layer_names[i]).small());
                        let body_count = editor.bodies.iter().filter(|b| b.collision_layer == (1 << i)).count();
                        ui.label(RichText::new(format!("{} bodies", body_count)).small().color(Color32::GRAY));
                        ui.end_row();
                    }
                });
        });
}

// ---- Body spawn presets ----

pub fn spawn_preset_bodies(editor: &mut PhysicsEditor, preset: &str) {
    match preset {
        "stack" => {
            for i in 0..5 {
                let mut b = RigidBody::new(&format!("Stack Box {}", i), BodyType::Dynamic);
                b.position = [0.0, i as f32 * 1.1 + 0.5];
                b.collider = Collider::Box { width: 1.0, height: 1.0 };
                editor.bodies.push(b);
            }
        }
        "pendulum" => {
            let pivot_idx = editor.bodies.len();
            let mut pivot = RigidBody::new("Pendulum Pivot", BodyType::Static);
            pivot.position = [0.0, 4.0];
            pivot.collider = Collider::Circle { radius: 0.1 };
            editor.bodies.push(pivot);

            let bob_idx = editor.bodies.len();
            let mut bob = RigidBody::new("Pendulum Bob", BodyType::Dynamic);
            bob.position = [2.0, 4.0];
            bob.mass = 1.0;
            bob.collider = Collider::Circle { radius: 0.3 };
            editor.bodies.push(bob);

            editor.joints.push(Joint::Distance {
                body_a: pivot_idx,
                body_b: bob_idx,
                min_distance: 2.0,
                max_distance: 2.0,
            });
        }
        "newton_cradle" => {
            let n = 5;
            let spacing = 0.85;
            for i in 0..n {
                let anchor_idx = editor.bodies.len();
                let mut anchor = RigidBody::new(&format!("Cradle Anchor {}", i), BodyType::Static);
                anchor.position = [i as f32 * spacing - (n as f32 - 1.0) * spacing / 2.0, 3.0];
                anchor.collider = Collider::Circle { radius: 0.05 };
                let anchor_pos = anchor.position;
                editor.bodies.push(anchor);

                let ball_idx = editor.bodies.len();
                let mut ball = RigidBody::new(&format!("Cradle Ball {}", i), BodyType::Dynamic);
                ball.position = anchor_pos;
                ball.position[1] = 3.0 - 2.0;
                ball.mass = 0.5;
                ball.collider_props.restitution = 1.0;
                ball.collider = Collider::Circle { radius: 0.35 };
                editor.bodies.push(ball);

                editor.joints.push(Joint::Distance {
                    body_a: anchor_idx,
                    body_b: ball_idx,
                    min_distance: 2.0,
                    max_distance: 2.0,
                });
            }
            // Give first ball a push
            if let Some(first_ball) = editor.bodies.iter_mut().find(|b| b.name == "Cradle Ball 0") {
                first_ball.velocity = [4.0, 0.0];
            }
        }
        "dominos" => {
            for i in 0..8 {
                let mut d = RigidBody::new(&format!("Domino {}", i), BodyType::Dynamic);
                d.position = [i as f32 * 0.8 - 3.5, 0.4];
                d.collider = Collider::Box { width: 0.15, height: 0.8 };
                editor.bodies.push(d);
            }
        }
        _ => {}
    }
}

pub fn show_spawn_presets(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Spawn Presets").color(Color32::from_rgb(255, 200, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Stack (5 boxes)").clicked() { spawn_preset_bodies(editor, "stack"); }
                if ui.button("Pendulum").clicked() { spawn_preset_bodies(editor, "pendulum"); }
                if ui.button("Newton Cradle").clicked() { spawn_preset_bodies(editor, "newton_cradle"); }
                if ui.button("Dominos").clicked() { spawn_preset_bodies(editor, "dominos"); }
            });
        });
}

// ---- Body rendering helpers ----

pub fn body_fill_color(body: &RigidBody, is_selected: bool) -> Color32 {
    let base = body.body_type.color();
    if body.sleeping {
        return Color32::from_rgba_unmultiplied(base.r() / 3, base.g() / 3, base.b() / 3, 150);
    }
    if is_selected {
        Color32::from_rgba_unmultiplied(base.r() / 2, base.g() / 2, base.b() / 2 + 30, 220)
    } else {
        Color32::from_rgba_unmultiplied(base.r() / 4, base.g() / 4, base.b() / 4, 200)
    }
}

pub fn body_stroke(body: &RigidBody, is_selected: bool) -> Stroke {
    let color = if is_selected { Color32::WHITE } else { body.body_type.color() };
    let width = if is_selected { 2.5 } else { 1.5 };
    Stroke::new(width, color)
}

// ---- Physics debug info ----

pub fn show_physics_debug_info(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::Frame::none()
        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 180))
        .inner_margin(6.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            let (sc, dc, kc) = editor.body_count_by_type();
            ui.label(RichText::new(format!("S:{} D:{} K:{} | J:{}", sc, dc, kc, editor.joints.len())).small().color(Color32::WHITE));
            ui.label(RichText::new(format!("KE:{:.1}J | t:{:.2}s | {}", editor.total_kinetic_energy(), editor.simulation_time, if editor.simulation_running { "SIM" } else { "PAUSED" })).small().color(Color32::GRAY));
            let pairs = editor.collision_pairs();
            ui.label(RichText::new(format!("Collision pairs: {} | Zoom: {:.0}px/m", pairs.len(), editor.canvas_zoom)).small().color(Color32::GRAY));
        });
}

// ---- Constraint force estimation ----

pub fn estimate_spring_force(joint: &Joint, bodies: &[RigidBody]) -> f32 {
    if let Joint::Spring { body_a, body_b, rest_length, stiffness, damping } = joint {
        if *body_a >= bodies.len() || *body_b >= bodies.len() { return 0.0; }
        let pa = bodies[*body_a].position;
        let pb = bodies[*body_b].position;
        let dx = pb[0] - pa[0];
        let dy = pb[1] - pa[1];
        let dist = (dx * dx + dy * dy).sqrt();
        let extension = dist - rest_length;
        (extension * stiffness).abs()
    } else {
        0.0
    }
}

pub fn show_force_display(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Joint Forces").color(Color32::from_rgb(255, 200, 80)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("joint_force_grid").num_columns(3).striped(true).show(ui, |ui| {
                ui.strong("Joint");
                ui.strong("Type");
                ui.strong("Force");
                ui.end_row();

                for (ji, joint) in editor.joints.iter().enumerate() {
                    let ba = joint.body_a();
                    let bb = joint.body_b();
                    let na = editor.bodies.get(ba).map(|b| b.name.as_str()).unwrap_or("?");
                    let nb = editor.bodies.get(bb).map(|b| b.name.as_str()).unwrap_or("?");
                    ui.label(RichText::new(format!("{}-{}", na, nb)).small());
                    ui.label(RichText::new(joint.label()).small().color(joint.color()));
                    let force = estimate_spring_force(joint, &editor.bodies);
                    let fc = if force > 500.0 { Color32::from_rgb(220, 80, 80) } else if force > 100.0 { Color32::from_rgb(220, 180, 50) } else { Color32::GRAY };
                    ui.label(RichText::new(format!("{:.0}N", force)).small().color(fc));
                    ui.end_row();
                }
            });
        });
}

// ---- Polygon convex hull helper ----

pub fn convex_hull(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if points.len() <= 3 { return points.to_vec(); }
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap().then(a[1].partial_cmp(&b[1]).unwrap()));

    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| -> f32 {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };

    let mut hull: Vec<[f32; 2]> = Vec::new();
    for &p in &pts {
        while hull.len() >= 2 && cross(hull[hull.len()-2], hull[hull.len()-1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    let lower_len = hull.len() + 1;
    for &p in pts.iter().rev() {
        while hull.len() >= lower_len && cross(hull[hull.len()-2], hull[hull.len()-1], p) <= 0.0 {
            hull.pop();
        }
        hull.push(p);
    }
    hull.pop();
    hull
}

pub fn make_convex_collider(points: &[[f32; 2]]) -> Collider {
    let hull = convex_hull(points);
    Collider::Polygon { points: hull }
}

// ---- AABB overlap display ----

pub fn show_aabb_overlaps(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new("AABB Overlaps (Broad Phase)")
        .default_open(false)
        .show(ui, |ui| {
            let pairs = broad_phase_pairs(&editor.bodies);
            if pairs.is_empty() {
                ui.label(RichText::new("No overlaps").color(Color32::GRAY));
                return;
            }
            for (a, b) in pairs.iter().take(10) {
                let na = &editor.bodies[*a].name;
                let nb = &editor.bodies[*b].name;
                let layers_ok = editor.layers_collide(editor.bodies[*a].collision_layer, editor.bodies[*b].collision_layer);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} x {}", na, nb)).small());
                    if layers_ok {
                        ui.label(RichText::new("Collides").small().color(Color32::from_rgb(220, 80, 80)));
                    } else {
                        ui.label(RichText::new("Layer masked").small().color(Color32::GRAY));
                    }
                });
            }
            if pairs.len() > 10 {
                ui.label(RichText::new(format!("...and {} more", pairs.len() - 10)).small().color(Color32::GRAY));
            }
        });
}

// ---- Rigid body copy/paste ----

#[derive(Clone, Debug, Default)]
pub struct PhysicsClipboard {
    pub bodies: Vec<RigidBody>,
    pub joints: Vec<Joint>,
}

impl PhysicsClipboard {
    pub fn copy_body(body: &RigidBody) -> PhysicsClipboard {
        PhysicsClipboard { bodies: vec![body.clone()], joints: Vec::new() }
    }

    pub fn paste_into(&self, editor: &mut PhysicsEditor, offset: [f32; 2]) {
        let base_idx = editor.bodies.len();
        for body in &self.bodies {
            let mut new_body = body.clone();
            new_body.name = format!("{} (copy)", new_body.name);
            new_body.position[0] += offset[0];
            new_body.position[1] += offset[1];
            new_body.velocity = [0.0, 0.0];
            new_body.angular_velocity = 0.0;
            editor.bodies.push(new_body);
        }
        for joint in &self.joints {
            let mut new_joint = joint.clone();
            // remap body indices
            match &mut new_joint {
                Joint::Fixed { body_a, body_b, .. } => { *body_a += base_idx; *body_b += base_idx; }
                Joint::Hinge { body_a, body_b, .. } => { *body_a += base_idx; *body_b += base_idx; }
                Joint::Slider { body_a, body_b, .. } => { *body_a += base_idx; *body_b += base_idx; }
                Joint::Spring { body_a, body_b, .. } => { *body_a += base_idx; *body_b += base_idx; }
                Joint::Distance { body_a, body_b, .. } => { *body_a += base_idx; *body_b += base_idx; }
                Joint::Pulley { body_a, body_b, .. } => { *body_a += base_idx; *body_b += base_idx; }
            }
            editor.joints.push(new_joint);
        }
    }
}

// ---- Body search ----

pub fn find_bodies_by_name(editor: &PhysicsEditor, query: &str) -> Vec<usize> {
    let q = query.to_lowercase();
    editor.bodies.iter().enumerate()
        .filter(|(_, b)| b.name.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

pub fn show_body_search(ui: &mut egui::Ui, editor: &mut PhysicsEditor, query: &mut String) {
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.add(egui::TextEdit::singleline(query).hint_text("Body name...").desired_width(120.0));
        if !query.is_empty() && ui.small_button("x").clicked() { query.clear(); }
    });
    if !query.is_empty() {
        let results = find_bodies_by_name(editor, query);
        ui.label(RichText::new(format!("{} matches", results.len())).small().color(Color32::GRAY));
        for &idx in results.iter().take(8) {
            let body = &editor.bodies[idx];
            let is_sel = editor.selected_body == Some(idx);
            if ui.selectable_label(is_sel, RichText::new(format!("[{}] {}", body.body_type.label()[0..1].to_string(), body.name)).small()).clicked() {
                editor.selected_body = Some(idx);
                editor.center_view_on_body(idx);
            }
        }
    }
}

// ---- Body type batch change ----

pub fn show_batch_type_change(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Batch Type Change").color(Color32::from_rgb(255, 200, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label("Change all selected-layer bodies:");
            let layer_names: Vec<String> = editor.layer_names.iter().take(8).cloned().collect();
            ui.horizontal(|ui| {
                ui.label("Layer:");
                let mut layer_sel = 0_usize;
                egui::ComboBox::from_id_salt("batch_layer_sel")
                    .selected_text(&layer_names[layer_sel])
                    .show_ui(ui, |ui| {
                        for (i, name) in layer_names.iter().enumerate() {
                            if ui.selectable_label(i == layer_sel, name.as_str()).clicked() {
                                layer_sel = i;
                            }
                        }
                    });
                ui.label("to:");
                egui::ComboBox::from_id_salt("batch_type_dest")
                    .selected_text("Static")
                    .show_ui(ui, |ui| {
                        for bt in BodyType::all() {
                            if ui.selectable_label(false, bt.label()).clicked() {
                                let mask = 1u32 << layer_sel;
                                for body in editor.bodies.iter_mut() {
                                    if body.collision_layer & mask != 0 {
                                        body.body_type = bt.clone();
                                    }
                                }
                            }
                        }
                    });
            });
        });
}

// ---- Physics export helpers ----

pub fn export_bodies_summary(editor: &PhysicsEditor) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Physics World: {} bodies, {} joints, {} materials", editor.bodies.len(), editor.joints.len(), editor.materials.len()));
    lines.push(format!("Gravity: ({:.2}, {:.2})", editor.gravity[0], editor.gravity[1]));
    lines.push(String::new());
    for body in &editor.bodies {
        lines.push(format!("[{}] {} at ({:.2}, {:.2}) rot:{:.1}deg | {:?} | {}", body.body_type.label(), body.name, body.position[0], body.position[1], body.rotation, body.collider.label(), if body.sleeping { "sleeping" } else { "awake" }));
    }
    lines.join("\n")
}

pub fn export_joints_summary(editor: &PhysicsEditor) -> String {
    let mut lines = Vec::new();
    for joint in &editor.joints {
        let na = editor.bodies.get(joint.body_a()).map(|b| b.name.as_str()).unwrap_or("?");
        let nb = editor.bodies.get(joint.body_b()).map(|b| b.name.as_str()).unwrap_or("?");
        lines.push(format!("{}: {} <-> {}", joint.label(), na, nb));
    }
    lines.join("\n")
}

// ---- 2D grid snap helpers ----

pub fn snap_pos_to_grid(pos: [f32; 2], grid: f32) -> [f32; 2] {
    [(pos[0] / grid).round() * grid, (pos[1] / grid).round() * grid]
}

pub fn nearest_body_at(editor: &PhysicsEditor, world_x: f32, world_y: f32, radius: f32) -> Option<usize> {
    editor.bodies.iter().enumerate()
        .filter_map(|(i, b)| {
            let dx = b.position[0] - world_x;
            let dy = b.position[1] - world_y;
            let d = (dx*dx + dy*dy).sqrt();
            if d <= radius { Some((i, d)) } else { None }
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)
}

// ---- Verlet particle simulation ----

#[derive(Clone, Debug)]
pub struct Particle {
    pub position: [f32; 2],
    pub prev_position: [f32; 2],
    pub pinned: bool,
    pub radius: f32,
    pub color: [u8; 3],
}

impl Particle {
    pub fn new(x: f32, y: f32) -> Self {
        Particle {
            position: [x, y],
            prev_position: [x, y],
            pinned: false,
            radius: 0.15,
            color: [150, 200, 255],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParticleSimulation {
    pub particles: Vec<Particle>,
    pub links: Vec<(usize, usize, f32)>,
    pub gravity: [f32; 2],
}

impl ParticleSimulation {
    pub fn new_rope(length: usize, spacing: f32) -> Self {
        let mut particles = Vec::new();
        let mut links = Vec::new();
        for i in 0..length {
            let mut p = Particle::new(i as f32 * spacing, 0.0);
            if i == 0 { p.pinned = true; }
            particles.push(p);
        }
        for i in 0..length-1 {
            links.push((i, i+1, spacing));
        }
        ParticleSimulation { particles, links, gravity: [0.0, -9.81] }
    }

    pub fn step(&mut self, dt: f32, iterations: usize) {
        for p in self.particles.iter_mut() {
            if p.pinned { continue; }
            let vx = p.position[0] - p.prev_position[0];
            let vy = p.position[1] - p.prev_position[1];
            p.prev_position = p.position;
            p.position[0] += vx * 0.99 + self.gravity[0] * dt * dt;
            p.position[1] += vy * 0.99 + self.gravity[1] * dt * dt;
        }
        for _ in 0..iterations {
            let links = self.links.clone();
            for (a, b, rest) in &links {
                if *a >= self.particles.len() || *b >= self.particles.len() { continue; }
                let dx = self.particles[*b].position[0] - self.particles[*a].position[0];
                let dy = self.particles[*b].position[1] - self.particles[*a].position[1];
                let dist = (dx*dx + dy*dy).sqrt().max(0.0001);
                let error = (dist - rest) / dist;
                let cx = dx * error * 0.5;
                let cy = dy * error * 0.5;
                if !self.particles[*a].pinned { self.particles[*a].position[0] += cx; self.particles[*a].position[1] += cy; }
                if !self.particles[*b].pinned { self.particles[*b].position[0] -= cx; self.particles[*b].position[1] -= cy; }
            }
        }
    }
}

pub fn draw_particle_sim(painter: &Painter, sim: &ParticleSimulation, world_to_screen: &dyn Fn(f32, f32) -> Pos2) {
    for (a, b, _) in &sim.links {
        if *a >= sim.particles.len() || *b >= sim.particles.len() { continue; }
        let pa = world_to_screen(sim.particles[*a].position[0], sim.particles[*a].position[1]);
        let pb = world_to_screen(sim.particles[*b].position[0], sim.particles[*b].position[1]);
        painter.line_segment([pa, pb], Stroke::new(1.5, Color32::from_rgb(180, 180, 220)));
    }
    for p in &sim.particles {
        let sp = world_to_screen(p.position[0], p.position[1]);
        let color = if p.pinned { Color32::from_rgb(255, 100, 100) } else { Color32::from_rgb(p.color[0], p.color[1], p.color[2]) };
        painter.circle_filled(sp, 4.0, color);
    }
}

// ---- Fluid particle placeholder ----

pub struct FluidParticles {
    pub positions: Vec<[f32; 2]>,
    pub velocities: Vec<[f32; 2]>,
    pub gravity: [f32; 2],
    pub bounds: [f32; 4],
    pub restitution: f32,
}

impl FluidParticles {
    pub fn new_splash(count: usize, center: [f32; 2]) -> Self {
        let mut positions = Vec::new();
        let mut velocities = Vec::new();
        for i in 0..count {
            let angle = i as f32 / count as f32 * std::f32::consts::TAU;
            let r = 0.3 + (i % 3) as f32 * 0.2;
            positions.push([center[0] + angle.cos() * r, center[1] + angle.sin() * r]);
            velocities.push([angle.cos() * 2.0, angle.sin() * 2.0]);
        }
        FluidParticles {
            positions,
            velocities,
            gravity: [0.0, -9.81],
            bounds: [-8.0, -4.0, 8.0, 8.0],
            restitution: 0.5,
        }
    }

    pub fn step(&mut self, dt: f32) {
        for (pos, vel) in self.positions.iter_mut().zip(self.velocities.iter_mut()) {
            vel[0] += self.gravity[0] * dt;
            vel[1] += self.gravity[1] * dt;
            pos[0] += vel[0] * dt;
            pos[1] += vel[1] * dt;
            if pos[0] < self.bounds[0] { pos[0] = self.bounds[0]; vel[0] = vel[0].abs() * self.restitution; }
            if pos[0] > self.bounds[2] { pos[0] = self.bounds[2]; vel[0] = -vel[0].abs() * self.restitution; }
            if pos[1] < self.bounds[1] { pos[1] = self.bounds[1]; vel[1] = vel[1].abs() * self.restitution; }
            if pos[1] > self.bounds[3] { pos[1] = self.bounds[3]; vel[1] = -vel[1].abs() * self.restitution; }
        }
    }

    pub fn draw(&self, painter: &Painter, world_to_screen: &dyn Fn(f32, f32) -> Pos2) {
        for pos in &self.positions {
            let sp = world_to_screen(pos[0], pos[1]);
            painter.circle_filled(sp, 3.5, Color32::from_rgba_unmultiplied(80, 150, 255, 200));
        }
    }
}

// ---- Extended PhysicsEditor display ----

pub fn show_physics_help_overlay(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Controls").color(Color32::GRAY).small())
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("phys_help_grid").num_columns(2).striped(true).show(ui, |ui| {
                for (key, action) in &[
                    ("Scroll", "Zoom preview"),
                    ("RMB Drag", "Pan preview"),
                    ("LMB", "Select body"),
                    ("Simulate", "Toggle simulation"),
                    ("Step", "Single physics step"),
                    ("Reset", "Stop & reset velocities"),
                ] {
                    ui.label(RichText::new(*key).monospace().small().color(Color32::from_rgb(200, 200, 100)));
                    ui.label(RichText::new(*action).small().color(Color32::GRAY));
                    ui.end_row();
                }
            });
        });
}

// ---- Gravity anomaly zones ----

#[derive(Clone, Debug)]
pub struct GravityZone {
    pub center: [f32; 2],
    pub radius: f32,
    pub gravity: [f32; 2],
    pub enabled: bool,
    pub label: String,
}

impl GravityZone {
    pub fn new(center: [f32; 2], radius: f32, gravity: [f32; 2]) -> Self {
        GravityZone {
            center,
            radius,
            gravity,
            enabled: true,
            label: "Gravity Zone".to_string(),
        }
    }

    pub fn applies_to(&self, pos: [f32; 2]) -> bool {
        if !self.enabled { return false; }
        let dx = pos[0] - self.center[0];
        let dy = pos[1] - self.center[1];
        (dx*dx + dy*dy).sqrt() <= self.radius
    }
}

pub fn draw_gravity_zone(painter: &Painter, zone: &GravityZone, world_to_screen: &dyn Fn(f32, f32) -> Pos2, zoom: f32) {
    if !zone.enabled { return; }
    let center = world_to_screen(zone.center[0], zone.center[1]);
    let r_px = zone.radius * zoom;

    painter.circle_filled(center, r_px, Color32::from_rgba_unmultiplied(80, 120, 255, 30));
    painter.circle_stroke(center, r_px, Stroke::new(1.5, Color32::from_rgba_unmultiplied(80, 150, 255, 150)));

    let gx = zone.gravity[0];
    let gy = zone.gravity[1];
    let g_len = (gx*gx + gy*gy).sqrt().max(0.001);
    let arrow_len = 20.0_f32.min(r_px * 0.6);
    let dx = gx / g_len * arrow_len;
    let dy = -gy / g_len * arrow_len;
    let end = Pos2::new(center.x + dx, center.y + dy);

    let dir = Vec2::new(dx, dy).normalized();
    let perp = Vec2::new(-dir.y, dir.x);
    painter.line_segment([center, end], Stroke::new(2.0, Color32::from_rgb(100, 180, 255)));
    painter.add(Shape::convex_polygon(
        vec![end, end - dir * 7.0 + perp * 3.5, end - dir * 7.0 - perp * 3.5],
        Color32::from_rgb(100, 180, 255),
        Stroke::NONE,
    ));

    painter.text(center + Vec2::new(0.0, -r_px - 10.0), egui::Align2::CENTER_BOTTOM, &zone.label, FontId::proportional(9.0), Color32::from_rgb(150, 180, 255));
}

// ---- Physics material preset library ----

pub fn physics_material_presets() -> Vec<PhysicsMaterial> {
    vec![
        PhysicsMaterial { name: "Rubber".to_string(), restitution: 0.8, friction: 0.9, combine_mode: CombineMode::Max, color: [200, 100, 80] },
        PhysicsMaterial { name: "Ice".to_string(), restitution: 0.1, friction: 0.02, combine_mode: CombineMode::Min, color: [150, 200, 255] },
        PhysicsMaterial { name: "Wood".to_string(), restitution: 0.2, friction: 0.6, combine_mode: CombineMode::Average, color: [180, 140, 80] },
        PhysicsMaterial { name: "Metal".to_string(), restitution: 0.15, friction: 0.4, combine_mode: CombineMode::Average, color: [160, 170, 190] },
        PhysicsMaterial { name: "Glass".to_string(), restitution: 0.7, friction: 0.1, combine_mode: CombineMode::Min, color: [200, 230, 255] },
        PhysicsMaterial { name: "Concrete".to_string(), restitution: 0.05, friction: 0.8, combine_mode: CombineMode::Max, color: [140, 140, 130] },
        PhysicsMaterial { name: "Foam".to_string(), restitution: 0.3, friction: 0.7, combine_mode: CombineMode::Multiply, color: [200, 200, 100] },
        PhysicsMaterial { name: "Slime".to_string(), restitution: 0.05, friction: 1.5, combine_mode: CombineMode::Max, color: [100, 200, 100] },
        PhysicsMaterial { name: "Bouncy".to_string(), restitution: 1.0, friction: 0.1, combine_mode: CombineMode::Max, color: [255, 200, 50] },
        PhysicsMaterial { name: "Sticky".to_string(), restitution: 0.0, friction: 2.0, combine_mode: CombineMode::Multiply, color: [200, 100, 200] },
    ]
}

pub fn show_material_preset_library(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Material Presets").color(Color32::from_rgb(200, 180, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Click to add to materials list").small().color(Color32::GRAY));
            for preset in physics_material_presets() {
                let mc = Color32::from_rgb(preset.color[0], preset.color[1], preset.color[2]);
                ui.horizontal(|ui| {
                    let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot_rect.center(), 5.0, mc);
                    let resp = ui.selectable_label(false, RichText::new(&preset.name).small().color(mc));
                    ui.label(RichText::new(format!("R:{:.2} F:{:.2}", preset.restitution, preset.friction)).small().color(Color32::GRAY));
                    if resp.clicked() {
                        editor.materials.push(preset);
                    }
                });
            }
        });
}

// ============================================================
// JOINT CONSTRAINT GRAPH
// ============================================================

pub fn draw_joint_constraint_graph(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    let desired = Vec2::new(ui.available_width(), 200.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 24));

    if editor.bodies.is_empty() { return; }

    let n = editor.bodies.len();
    let cx = rect.center().x;
    let cy = rect.center().y;
    let radius = (rect.width().min(rect.height()) * 0.42).min(80.0);

    let node_positions: Vec<Pos2> = (0..n).map(|i| {
        let angle = (i as f32 / n as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        Pos2::new(cx + radius * angle.cos(), cy + radius * angle.sin())
    }).collect();

    // Draw joints as edges
    for joint in &editor.joints {
        let (a, b) = match joint {
            Joint::Fixed { body_a, body_b, .. } => (*body_a, *body_b),
            Joint::Hinge { body_a, body_b, .. } => (*body_a, *body_b),
            Joint::Slider { body_a, body_b, .. } => (*body_a, *body_b),
            Joint::Spring { body_a, body_b, .. } => (*body_a, *body_b),
            Joint::Distance { body_a, body_b, .. } => (*body_a, *body_b),
            Joint::Pulley { body_a, body_b, .. } => (*body_a, *body_b),
        };
        let joint_color = match joint {
            Joint::Fixed { .. } => Color32::from_rgb(200, 200, 200),
            Joint::Hinge { .. } => Color32::from_rgb(100, 200, 255),
            Joint::Slider { .. } => Color32::from_rgb(200, 150, 80),
            Joint::Spring { .. } => Color32::from_rgb(100, 255, 150),
            Joint::Distance { .. } => Color32::from_rgb(200, 100, 200),
            Joint::Pulley { .. } => Color32::from_rgb(255, 200, 80),
        };
        if let (Some(&pa), Some(&pb)) = (node_positions.get(a), node_positions.get(b)) {
            painter.line_segment([pa, pb], Stroke::new(1.5, joint_color));
        }
    }

    // Draw body nodes
    for (i, body) in editor.bodies.iter().enumerate() {
        if let Some(&pos) = node_positions.get(i) {
            let bc = match body.body_type {
                BodyType::Static => Color32::from_rgb(160, 160, 200),
                BodyType::Dynamic => Color32::from_rgb(100, 200, 255),
                BodyType::Kinematic => Color32::from_rgb(200, 180, 80),
            };
            let is_selected = editor.selected_body == Some(i);
            let node_r = if is_selected { 8.0 } else { 5.5 };
            painter.circle_filled(pos, node_r, bc);
            if is_selected {
                painter.circle_stroke(pos, node_r + 2.0, Stroke::new(1.5, Color32::WHITE));
            }
            painter.text(pos + Vec2::new(0.0, -12.0), egui::Align2::CENTER_CENTER, &body.name, FontId::proportional(9.0), Color32::LIGHT_GRAY);
        }
    }

    // Legend
    let legend_items = [
        ("Fixed", Color32::from_rgb(200, 200, 200)),
        ("Hinge", Color32::from_rgb(100, 200, 255)),
        ("Slider", Color32::from_rgb(200, 150, 80)),
        ("Spring", Color32::from_rgb(100, 255, 150)),
        ("Distance", Color32::from_rgb(200, 100, 200)),
        ("Pulley", Color32::from_rgb(255, 200, 80)),
    ];
    for (j, (label, color)) in legend_items.iter().enumerate() {
        let lx = rect.left() + 5.0;
        let ly = rect.bottom() - 8.0 - j as f32 * 12.0;
        painter.line_segment([Pos2::new(lx, ly), Pos2::new(lx + 16.0, ly)], Stroke::new(2.0, *color));
        painter.text(Pos2::new(lx + 20.0, ly), egui::Align2::LEFT_CENTER, label, FontId::monospace(8.0), *color);
    }
}

// ============================================================
// PHYSICS SCENARIO TEMPLATES
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PhysicsScenario {
    Empty,
    StackOfBoxes { count: usize },
    PendulumChain { links: usize },
    NewtonsCradle { balls: usize },
    DominoeRun { count: usize },
    RopeSwing { segments: usize },
    ClothPatch { rows: usize, cols: usize },
    WreckingBall,
    Catapult,
    Trampoline,
}

impl PhysicsScenario {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Empty,
            Self::StackOfBoxes { count: 5 },
            Self::PendulumChain { links: 4 },
            Self::NewtonsCradle { balls: 5 },
            Self::DominoeRun { count: 8 },
            Self::RopeSwing { segments: 6 },
            Self::ClothPatch { rows: 5, cols: 5 },
            Self::WreckingBall,
            Self::Catapult,
            Self::Trampoline,
        ]
    }
    pub fn label(&self) -> String {
        match self {
            Self::Empty => "Empty".to_string(),
            Self::StackOfBoxes { count } => format!("Stack of {} Boxes", count),
            Self::PendulumChain { links } => format!("Pendulum Chain ({} links)", links),
            Self::NewtonsCradle { balls } => format!("Newton's Cradle ({} balls)", balls),
            Self::DominoeRun { count } => format!("Domino Run ({})", count),
            Self::RopeSwing { segments } => format!("Rope Swing ({} segs)", segments),
            Self::ClothPatch { rows, cols } => format!("Cloth {}x{}", rows, cols),
            Self::WreckingBall => "Wrecking Ball".to_string(),
            Self::Catapult => "Catapult".to_string(),
            Self::Trampoline => "Trampoline".to_string(),
        }
    }
    pub fn spawn(&self, editor: &mut PhysicsEditor) {
        editor.bodies.clear();
        editor.joints.clear();
        match self {
            Self::StackOfBoxes { count } => {
                editor.bodies.push(RigidBody { name: "Ground".to_string(), body_type: BodyType::Static, position: [0.0, -5.0], mass: 0.0, ..RigidBody::default() });
                for i in 0..*count {
                    editor.bodies.push(RigidBody {
                        name: format!("Box {}", i + 1),
                        body_type: BodyType::Dynamic,
                        position: [0.0, i as f32 * 1.2],
                        mass: 1.0,
                        ..RigidBody::default()
                    });
                }
            }
            Self::NewtonsCradle { balls } => {
                let n = *balls;
                for i in 0..n {
                    let x = (i as f32 - n as f32 * 0.5) * 1.2;
                    let anchor_body = editor.bodies.len();
                    editor.bodies.push(RigidBody {
                        name: format!("Anchor {}", i),
                        body_type: BodyType::Static,
                        position: [x, 5.0],
                        mass: 0.0,
                        ..RigidBody::default()
                    });
                    let ball_body = editor.bodies.len();
                    editor.bodies.push(RigidBody {
                        name: format!("Ball {}", i),
                        body_type: BodyType::Dynamic,
                        position: [x, 0.0],
                        mass: 1.0,
                        ..RigidBody::default()
                    });
                    editor.joints.push(Joint::Distance {
                        body_a: anchor_body, body_b: ball_body,
                        min_distance: 4.9, max_distance: 5.0,
                    });
                }
            }
            Self::PendulumChain { links } => {
                let n = *links;
                let anchor = editor.bodies.len();
                editor.bodies.push(RigidBody { name: "Anchor".to_string(), body_type: BodyType::Static, position: [0.0, 6.0], mass: 0.0, ..RigidBody::default() });
                let mut prev = anchor;
                for i in 0..n {
                    let next = editor.bodies.len();
                    editor.bodies.push(RigidBody {
                        name: format!("Link {}", i + 1),
                        body_type: BodyType::Dynamic,
                        position: [0.0, 6.0 - (i + 1) as f32 * 1.5],
                        mass: 0.5,
                        ..RigidBody::default()
                    });
                    editor.joints.push(Joint::Hinge {
                        body_a: prev, body_b: next, anchor: [0.0, 0.0],
                        lower_angle: -std::f32::consts::FRAC_PI_2, upper_angle: std::f32::consts::FRAC_PI_2,
                        motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0,
                    });
                    prev = next;
                }
            }
            _ => {}
        }
    }
}

pub fn show_scenario_spawner(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Scenario Spawner").color(Color32::from_rgb(255, 200, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Spawn a preset physics scenario:").small().color(Color32::GRAY));
            for scenario in PhysicsScenario::all() {
                let label = scenario.label();
                if ui.button(&label).clicked() {
                    scenario.spawn(editor);
                }
            }
        });
}

// ============================================================
// FORCE FIELD ZONES
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForceFieldZone {
    pub name: String,
    pub position: [f32; 2],
    pub radius: f32,
    pub force_type: ForceFieldType,
    pub strength: f32,
    pub enabled: bool,
    pub color: [u8; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ForceFieldType {
    Attractor,
    Repulsor,
    Vortex { clockwise: bool },
    Wind { angle_deg: f32 },
    Explosion { decay: f32 },
    Drag { coefficient: f32 },
}

impl ForceFieldType {
    pub fn label(&self) -> &str {
        match self {
            Self::Attractor => "Attractor",
            Self::Repulsor => "Repulsor",
            Self::Vortex { .. } => "Vortex",
            Self::Wind { .. } => "Wind",
            Self::Explosion { .. } => "Explosion",
            Self::Drag { .. } => "Drag",
        }
    }
    pub fn apply_to_body(&self, body_pos: [f32; 2], zone_pos: [f32; 2], strength: f32) -> [f32; 2] {
        let dx = body_pos[0] - zone_pos[0];
        let dy = body_pos[1] - zone_pos[1];
        let dist = (dx * dx + dy * dy).sqrt().max(0.01);
        match self {
            Self::Attractor => [-dx / dist * strength, -dy / dist * strength],
            Self::Repulsor => [dx / dist * strength, dy / dist * strength],
            Self::Vortex { clockwise } => {
                let sign = if *clockwise { 1.0 } else { -1.0 };
                [-dy / dist * strength * sign, dx / dist * strength * sign]
            }
            Self::Wind { angle_deg } => {
                let rad = angle_deg.to_radians();
                [rad.cos() * strength, rad.sin() * strength]
            }
            Self::Explosion { decay } => {
                let mag = strength * (-dist * decay).exp();
                [dx / dist * mag, dy / dist * mag]
            }
            Self::Drag { coefficient } => {
                [-dx * coefficient * 0.001, -dy * coefficient * 0.001]
            }
        }
    }
}

pub fn draw_force_field_zones(painter: &Painter, zones: &[ForceFieldZone], world_to_screen: impl Fn([f32; 2]) -> Pos2) {
    for zone in zones {
        if !zone.enabled { continue; }
        let center = world_to_screen(zone.position);
        let c = Color32::from_rgba_premultiplied(zone.color[0], zone.color[1], zone.color[2], 60);
        let border = Color32::from_rgb(zone.color[0], zone.color[1], zone.color[2]);
        painter.circle_filled(center, zone.radius * 20.0, c);
        painter.circle_stroke(center, zone.radius * 20.0, Stroke::new(1.5, border));

        // Draw force arrows
        match &zone.force_type {
            ForceFieldType::Attractor => {
                for i in 0..8 {
                    let angle = i as f32 / 8.0 * std::f32::consts::TAU;
                    let r = zone.radius * 15.0;
                    let from = Pos2::new(center.x + r * angle.cos(), center.y + r * angle.sin());
                    let dir = (center - from).normalized();
                    let to = from + dir * 12.0;
                    painter.line_segment([from, to], Stroke::new(1.0, border));
                    painter.circle_filled(to, 2.0, border);
                }
            }
            ForceFieldType::Repulsor => {
                for i in 0..8 {
                    let angle = i as f32 / 8.0 * std::f32::consts::TAU;
                    let r = zone.radius * 8.0;
                    let from = Pos2::new(center.x + r * angle.cos(), center.y + r * angle.sin());
                    let dir = from - center;
                    let dir = dir / dir.length();
                    let to = from + dir * 12.0;
                    painter.line_segment([from, to], Stroke::new(1.0, border));
                    painter.circle_filled(to, 2.0, border);
                }
            }
            ForceFieldType::Vortex { clockwise } => {
                let sign = if *clockwise { 1.0f32 } else { -1.0 };
                for i in 0..12 {
                    let angle = i as f32 / 12.0 * std::f32::consts::TAU;
                    let r = zone.radius * 12.0;
                    let from = Pos2::new(center.x + r * angle.cos(), center.y + r * angle.sin());
                    let perp_x = -angle.sin() * sign;
                    let perp_y = angle.cos() * sign;
                    let to = from + Vec2::new(perp_x * 10.0, perp_y * 10.0);
                    painter.line_segment([from, to], Stroke::new(1.0, border));
                }
            }
            ForceFieldType::Wind { angle_deg } => {
                let rad = angle_deg.to_radians();
                let dir = Vec2::new(rad.cos(), rad.sin());
                for i in -2..=2 {
                    let perp = Vec2::new(-dir.y, dir.x);
                    let base = center + perp * (i as f32 * 10.0);
                    let tip = base + dir * 20.0;
                    painter.line_segment([base, tip], Stroke::new(1.5, border));
                    painter.circle_filled(tip, 2.5, border);
                }
            }
            _ => {}
        }

        painter.text(center + Vec2::new(0.0, -(zone.radius * 20.0) - 8.0), egui::Align2::CENTER_CENTER,
            format!("{} ({})", zone.name, zone.force_type.label()), FontId::proportional(9.0), border);
    }
}

pub fn show_force_field_editor(ui: &mut egui::Ui, zones: &mut Vec<ForceFieldZone>) {
    egui::CollapsingHeader::new(RichText::new("Force Field Zones").color(Color32::from_rgb(200, 150, 255)))
        .default_open(false)
        .show(ui, |ui| {
            let mut to_remove = None;
            for (i, zone) in zones.iter_mut().enumerate() {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut zone.enabled, "");
                        ui.text_edit_singleline(&mut zone.name);
                        let mut c = [zone.color[0] as f32 / 255.0, zone.color[1] as f32 / 255.0, zone.color[2] as f32 / 255.0];
                        if ui.color_edit_button_rgb(&mut c).changed() {
                            zone.color = [(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8];
                        }
                        if ui.small_button("X").clicked() { to_remove = Some(i); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Pos:");
                        ui.add(egui::DragValue::new(&mut zone.position[0]).speed(0.1).prefix("x:"));
                        ui.add(egui::DragValue::new(&mut zone.position[1]).speed(0.1).prefix("y:"));
                        ui.label("Radius:");
                        ui.add(egui::DragValue::new(&mut zone.radius).speed(0.05).clamp_range(0.1f32..=20.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Strength:");
                        ui.add(egui::DragValue::new(&mut zone.strength).speed(0.1).clamp_range(0.0f32..=100.0));
                        ui.label("Type:");
                        let cur_label = zone.force_type.label().to_string();
                        egui::ComboBox::from_id_salt(format!("ff_type_{}", i))
                            .selected_text(&cur_label)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut zone.force_type, ForceFieldType::Attractor, "Attractor");
                                ui.selectable_value(&mut zone.force_type, ForceFieldType::Repulsor, "Repulsor");
                                ui.selectable_value(&mut zone.force_type, ForceFieldType::Vortex { clockwise: true }, "Vortex CW");
                                ui.selectable_value(&mut zone.force_type, ForceFieldType::Vortex { clockwise: false }, "Vortex CCW");
                                ui.selectable_value(&mut zone.force_type, ForceFieldType::Wind { angle_deg: 0.0 }, "Wind");
                                ui.selectable_value(&mut zone.force_type, ForceFieldType::Drag { coefficient: 1.0 }, "Drag");
                            });
                    });
                    // Type-specific params
                    match &mut zone.force_type {
                        ForceFieldType::Wind { angle_deg } => {
                            ui.add(egui::Slider::new(angle_deg, 0.0f32..=360.0).suffix("°").text("Direction"));
                        }
                        ForceFieldType::Vortex { clockwise } => {
                            ui.checkbox(clockwise, "Clockwise");
                        }
                        ForceFieldType::Explosion { decay } => {
                            ui.add(egui::Slider::new(decay, 0.1f32..=5.0).text("Decay"));
                        }
                        ForceFieldType::Drag { coefficient } => {
                            ui.add(egui::Slider::new(coefficient, 0.01f32..=10.0).logarithmic(true).text("Coefficient"));
                        }
                        _ => {}
                    }
                });
            }
            if let Some(idx) = to_remove { zones.remove(idx); }
            if ui.button("+ Add Force Field").clicked() {
                zones.push(ForceFieldZone {
                    name: format!("Zone {}", zones.len() + 1),
                    position: [0.0, 0.0], radius: 3.0,
                    force_type: ForceFieldType::Attractor,
                    strength: 5.0, enabled: true, color: [150, 100, 255],
                });
            }
        });
}

// ============================================================
// RAGDOLL BUILDER
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RagdollConfig {
    pub scale: f32,
    pub name: String,
    pub position: [f32; 2],
    pub body_ids: Vec<usize>,
    pub joint_ids: Vec<usize>,
}

pub fn spawn_ragdoll(editor: &mut PhysicsEditor, base_pos: [f32; 2], scale: f32) -> RagdollConfig {
    let mut cfg = RagdollConfig { scale, name: format!("Ragdoll_{}", editor.bodies.len()), position: base_pos, ..Default::default() };
    let s = scale;
    let bx = base_pos[0]; let by = base_pos[1];

    let torso = editor.bodies.len();
    editor.bodies.push(RigidBody { name: format!("{}_Torso", cfg.name), body_type: BodyType::Dynamic, position: [bx, by], mass: 4.0 * s, ..RigidBody::default() });
    cfg.body_ids.push(torso);

    let head = editor.bodies.len();
    editor.bodies.push(RigidBody { name: format!("{}_Head", cfg.name), body_type: BodyType::Dynamic, position: [bx, by + 1.5 * s], mass: 1.0 * s, ..RigidBody::default() });
    cfg.body_ids.push(head);

    let neck_joint = editor.joints.len();
    editor.joints.push(Joint::Hinge { body_a: torso, body_b: head, anchor: [0.0, 0.75 * s], lower_angle: -0.3, upper_angle: 0.3, motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0 });
    cfg.joint_ids.push(neck_joint);

    let lu_arm = editor.bodies.len();
    editor.bodies.push(RigidBody { name: format!("{}_L_UpperArm", cfg.name), body_type: BodyType::Dynamic, position: [bx - 1.0 * s, by + 0.5 * s], mass: 1.5 * s, ..RigidBody::default() });
    cfg.body_ids.push(lu_arm);

    let l_shoulder = editor.joints.len();
    editor.joints.push(Joint::Hinge { body_a: torso, body_b: lu_arm, anchor: [-0.5 * s, 0.5 * s], lower_angle: -1.5, upper_angle: 0.5, motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0 });
    cfg.joint_ids.push(l_shoulder);

    let ru_arm = editor.bodies.len();
    editor.bodies.push(RigidBody { name: format!("{}_R_UpperArm", cfg.name), body_type: BodyType::Dynamic, position: [bx + 1.0 * s, by + 0.5 * s], mass: 1.5 * s, ..RigidBody::default() });
    cfg.body_ids.push(ru_arm);

    let r_shoulder = editor.joints.len();
    editor.joints.push(Joint::Hinge { body_a: torso, body_b: ru_arm, anchor: [0.5 * s, 0.5 * s], lower_angle: -0.5, upper_angle: 1.5, motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0 });
    cfg.joint_ids.push(r_shoulder);

    let l_leg = editor.bodies.len();
    editor.bodies.push(RigidBody { name: format!("{}_L_Leg", cfg.name), body_type: BodyType::Dynamic, position: [bx - 0.4 * s, by - 1.5 * s], mass: 2.0 * s, ..RigidBody::default() });
    cfg.body_ids.push(l_leg);
    let l_hip = editor.joints.len();
    editor.joints.push(Joint::Hinge { body_a: torso, body_b: l_leg, anchor: [-0.3 * s, -0.5 * s], lower_angle: -0.4, upper_angle: 1.2, motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0 });
    cfg.joint_ids.push(l_hip);

    let r_leg = editor.bodies.len();
    editor.bodies.push(RigidBody { name: format!("{}_R_Leg", cfg.name), body_type: BodyType::Dynamic, position: [bx + 0.4 * s, by - 1.5 * s], mass: 2.0 * s, ..RigidBody::default() });
    cfg.body_ids.push(r_leg);
    let r_hip = editor.joints.len();
    editor.joints.push(Joint::Hinge { body_a: torso, body_b: r_leg, anchor: [0.3 * s, -0.5 * s], lower_angle: -1.2, upper_angle: 0.4, motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0 });
    cfg.joint_ids.push(r_hip);

    cfg
}

pub fn show_ragdoll_spawner(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Ragdoll Builder").color(Color32::from_rgb(255, 160, 120)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Position:");
                let mut px = 0.0f32; let mut py = 2.0f32;
                ui.add(egui::DragValue::new(&mut px).speed(0.1).prefix("x:"));
                ui.add(egui::DragValue::new(&mut py).speed(0.1).prefix("y:"));
                let mut scale = 1.0f32;
                ui.label("Scale:");
                ui.add(egui::DragValue::new(&mut scale).speed(0.05).clamp_range(0.2f32..=5.0));
                if ui.button("Spawn Ragdoll").clicked() {
                    spawn_ragdoll(editor, [px, py], scale);
                }
            });
            ui.label(RichText::new("Spawns a 7-body humanoid with hinge joints.").small().color(Color32::GRAY));
        });
}

// ============================================================
// EXTENDED PhysicsEditor METHODS
// ============================================================

impl PhysicsEditor {
    pub fn selected_body_ref(&self) -> Option<&RigidBody> {
        self.selected_body.and_then(|i| self.bodies.get(i))
    }

    pub fn selected_body_mut(&mut self) -> Option<&mut RigidBody> {
        self.selected_body.and_then(|i| self.bodies.get_mut(i))
    }

    pub fn duplicate_selected_body(&mut self) {
        if let Some(idx) = self.selected_body {
            if let Some(body) = self.bodies.get(idx).cloned() {
                let mut new_body = body;
                new_body.name = format!("{}_copy", new_body.name);
                new_body.position[0] += 1.5;
                new_body.position[1] += 0.5;
                self.bodies.push(new_body);
                self.selected_body = Some(self.bodies.len() - 1);
            }
        }
    }

    pub fn delete_selected_body(&mut self) {
        if let Some(idx) = self.selected_body {
            self.bodies.remove(idx);
            self.joints.retain(|j| {
                let (a, b) = match j {
                    Joint::Fixed { body_a, body_b, .. } => (*body_a, *body_b),
                    Joint::Hinge { body_a, body_b, .. } => (*body_a, *body_b),
                    Joint::Slider { body_a, body_b, .. } => (*body_a, *body_b),
                    Joint::Spring { body_a, body_b, .. } => (*body_a, *body_b),
                    Joint::Distance { body_a, body_b, .. } => (*body_a, *body_b),
                    Joint::Pulley { body_a, body_b, .. } => (*body_a, *body_b),
                };
                a != idx && b != idx
            });
            self.selected_body = None;
        }
    }

    pub fn total_mass(&self) -> f32 {
        self.bodies.iter().filter(|b| b.body_type == BodyType::Dynamic).map(|b| b.mass).sum()
    }

    pub fn joint_count_by_type(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for joint in &self.joints {
            let key = match joint {
                Joint::Fixed { .. } => "Fixed",
                Joint::Hinge { .. } => "Hinge",
                Joint::Slider { .. } => "Slider",
                Joint::Spring { .. } => "Spring",
                Joint::Distance { .. } => "Distance",
                Joint::Pulley { .. } => "Pulley",
            };
            *map.entry(key.to_string()).or_insert(0) += 1;
        }
        map
    }

    pub fn clear_all(&mut self) {
        self.bodies.clear();
        self.joints.clear();
        self.selected_body = None;
        self.selected_joint = None;
    }

    pub fn select_all_dynamic(&mut self) {
        // Mark first dynamic body as selected (single select)
        self.selected_body = self.bodies.iter().position(|b| b.body_type == BodyType::Dynamic);
    }

    pub fn bodies_in_aabb(&self, min: [f32; 2], max: [f32; 2]) -> Vec<usize> {
        self.bodies.iter().enumerate().filter(|(_, b)| {
            b.position[0] >= min[0] && b.position[0] <= max[0] &&
            b.position[1] >= min[1] && b.position[1] <= max[1]
        }).map(|(i, _)| i).collect()
    }

    pub fn center_of_mass(&self) -> [f32; 2] {
        let dynamic: Vec<&RigidBody> = self.bodies.iter().filter(|b| b.body_type == BodyType::Dynamic).collect();
        if dynamic.is_empty() { return [0.0, 0.0]; }
        let total_mass: f32 = dynamic.iter().map(|b| b.mass).sum();
        if total_mass < 0.0001 { return [0.0, 0.0]; }
        let cx = dynamic.iter().map(|b| b.position[0] * b.mass).sum::<f32>() / total_mass;
        let cy = dynamic.iter().map(|b| b.position[1] * b.mass).sum::<f32>() / total_mass;
        [cx, cy]
    }

    pub fn apply_impulse_to_all_dynamic(&mut self, impulse: [f32; 2]) {
        for body in &mut self.bodies {
            if body.body_type == BodyType::Dynamic {
                body.velocity[0] += impulse[0] / body.mass.max(0.001);
                body.velocity[1] += impulse[1] / body.mass.max(0.001);
            }
        }
    }

    pub fn zero_all_velocities(&mut self) {
        for body in &mut self.bodies { body.velocity = [0.0, 0.0]; body.angular_velocity = 0.0; }
    }

    pub fn randomize_colors(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        for (i, body) in self.bodies.iter_mut().enumerate() {
            let mut h = DefaultHasher::new();
            i.hash(&mut h);
            body.name.hash(&mut h);
            let hash = h.finish();
            body.color = [
                ((hash >> 0) & 0xFF) as u8,
                ((hash >> 8) & 0xFF) as u8,
                ((hash >> 16) & 0xFF) as u8,
            ];
        }
    }

    pub fn snap_all_to_grid(&mut self, grid: f32) {
        for body in &mut self.bodies {
            body.position[0] = (body.position[0] / grid).round() * grid;
            body.position[1] = (body.position[1] / grid).round() * grid;
        }
    }

    pub fn scene_bounds(&self) -> ([f32; 2], [f32; 2]) {
        if self.bodies.is_empty() { return ([-10.0, -10.0], [10.0, 10.0]); }
        let mut min = [f32::MAX, f32::MAX];
        let mut max = [f32::MIN, f32::MIN];
        for body in &self.bodies {
            min[0] = min[0].min(body.position[0]);
            min[1] = min[1].min(body.position[1]);
            max[0] = max[0].max(body.position[0]);
            max[1] = max[1].max(body.position[1]);
        }
        (min, max)
    }

    pub fn fit_view_to_scene(&mut self) {
        let (min, max) = self.scene_bounds();
        let cx = (min[0] + max[0]) * 0.5;
        let cy = (min[1] + max[1]) * 0.5;
        let span = ((max[0] - min[0]).max(max[1] - min[1]) + 4.0).max(5.0);
        self.preview_offset = Vec2::new(-cx * 40.0, cy * 40.0);
        self.preview_zoom = 400.0 / span;
    }

    pub fn show_panel(ctx: &egui::Context, editor: &mut PhysicsEditor, dt: f32, open: &mut bool) {
        let mut soft_bodies = Vec::new();
        show_full_editor(ctx, editor, &mut soft_bodies, dt, open);
    }
}

// ============================================================
// PHYSICS SIMULATION STATS
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PhysicsSimStats {
    pub frame_count: u64,
    pub total_sim_time_ms: f64,
    pub avg_frame_ms: f64,
    pub peak_frame_ms: f64,
    pub broad_phase_pairs: usize,
    pub narrow_phase_pairs: usize,
    pub contact_points: usize,
    pub awake_bodies: usize,
    pub sleeping_bodies: usize,
    pub history_ms: Vec<f64>,
}

impl PhysicsSimStats {
    pub fn record_frame(&mut self, ms: f64, broad: usize, awake: usize, total: usize) {
        self.frame_count += 1;
        self.total_sim_time_ms += ms;
        self.avg_frame_ms = self.total_sim_time_ms / self.frame_count as f64;
        self.peak_frame_ms = self.peak_frame_ms.max(ms);
        self.broad_phase_pairs = broad;
        self.awake_bodies = awake;
        self.sleeping_bodies = total.saturating_sub(awake);
        self.history_ms.push(ms);
        if self.history_ms.len() > 120 { self.history_ms.remove(0); }
    }
}

pub fn show_sim_stats_panel(ui: &mut egui::Ui, stats: &PhysicsSimStats) {
    egui::CollapsingHeader::new(RichText::new("Simulation Stats").color(Color32::from_rgb(150, 220, 150)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("sim_stats").num_columns(2).spacing(Vec2::new(8.0, 2.0)).show(ui, |ui| {
                ui.label(RichText::new("Frame:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", stats.frame_count)).small().monospace()); ui.end_row();
                ui.label(RichText::new("Avg ms:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{:.2}", stats.avg_frame_ms)).small().monospace()); ui.end_row();
                ui.label(RichText::new("Peak ms:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{:.2}", stats.peak_frame_ms)).small().monospace().color(if stats.peak_frame_ms > 16.0 { Color32::RED } else { Color32::from_rgb(80, 200, 80) })); ui.end_row();
                ui.label(RichText::new("BP pairs:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", stats.broad_phase_pairs)).small().monospace()); ui.end_row();
                ui.label(RichText::new("Awake:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", stats.awake_bodies)).small().monospace()); ui.end_row();
                ui.label(RichText::new("Sleeping:").small().color(Color32::GRAY)); ui.label(RichText::new(format!("{}", stats.sleeping_bodies)).small().monospace()); ui.end_row();
            });
            // Frame time graph
            if !stats.history_ms.is_empty() {
                let desired = Vec2::new(ui.available_width(), 40.0);
                let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 25));
                let n = stats.history_ms.len();
                let max_ms = stats.history_ms.iter().cloned().fold(0.001f64, f64::max).max(16.0);
                for i in 1..n {
                    let x1 = rect.left() + rect.width() * (i - 1) as f32 / n as f32;
                    let x2 = rect.left() + rect.width() * i as f32 / n as f32;
                    let y1 = rect.bottom() - rect.height() * (stats.history_ms[i - 1] / max_ms) as f32;
                    let y2 = rect.bottom() - rect.height() * (stats.history_ms[i] / max_ms) as f32;
                    let color = if stats.history_ms[i] > 16.0 { Color32::RED } else { Color32::from_rgb(80, 200, 80) };
                    painter.line_segment([Pos2::new(x1, y1.clamp(rect.top(), rect.bottom())), Pos2::new(x2, y2.clamp(rect.top(), rect.bottom()))], Stroke::new(1.0, color));
                }
                let target_y = rect.bottom() - rect.height() * (16.667 / max_ms) as f32;
                painter.line_segment([Pos2::new(rect.left(), target_y.clamp(rect.top(), rect.bottom())), Pos2::new(rect.right(), target_y.clamp(rect.top(), rect.bottom()))], Stroke::new(1.0, Color32::from_rgb(255, 200, 50)));
                painter.text(Pos2::new(rect.right() - 2.0, rect.top() + 2.0), egui::Align2::RIGHT_TOP, "16ms", FontId::monospace(7.0), Color32::from_rgb(255, 200, 50));
            }
        });
}

// ============================================================
// EXTENDED WINDOW: FULL PHYSICS SETTINGS
// ============================================================

pub fn show_physics_settings_window(ctx: &egui::Context, open: &mut bool, editor: &mut PhysicsEditor) {
    egui::Window::new("Physics Settings")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(380.0, 500.0))
        .show(ctx, |ui| {
            ui.heading("Physics World Settings");
            ui.separator();
            egui::Grid::new("phys_settings").num_columns(2).spacing(Vec2::new(10.0, 5.0)).show(ui, |ui| {
                ui.label("Gravity X:");
                ui.add(egui::DragValue::new(&mut editor.gravity[0]).speed(0.05));
                ui.end_row();
                ui.label("Gravity Y:");
                ui.add(egui::DragValue::new(&mut editor.gravity[1]).speed(0.05));
                ui.end_row();
                ui.label("Time scale:");
                ui.add(egui::Slider::new(&mut editor.time_scale, 0.0f32..=5.0).logarithmic(false));
                ui.end_row();
                ui.label("Substeps:");
                ui.add(egui::DragValue::new(&mut editor.substeps).clamp_range(1usize..=20));
                ui.end_row();
                ui.label("Damping:");
                ui.add(egui::Slider::new(&mut editor.global_damping, 0.0f32..=1.0));
                ui.end_row();
            });
            ui.separator();
            ui.label(RichText::new("Body Stats").strong());
            let (s, d, k) = editor.body_count_by_type();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Static: {}  Dynamic: {}  Kinematic: {}", s, d, k)).small().color(Color32::GRAY));
            });
            ui.label(RichText::new(format!("Total mass: {:.2} kg", editor.total_mass())).small().color(Color32::GRAY));
            let cm = editor.center_of_mass();
            ui.label(RichText::new(format!("Center of mass: ({:.2}, {:.2})", cm[0], cm[1])).small().color(Color32::GRAY));
            ui.separator();
            ui.label(RichText::new("Joint Stats").strong());
            let jtypes = editor.joint_count_by_type();
            for (jtype, count) in &jtypes {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}: {}", jtype, count)).small().color(Color32::from_rgb(200, 180, 100)));
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Fit View").clicked() { editor.fit_view_to_scene(); }
                if ui.button("Zero Velocities").clicked() { editor.zero_all_velocities(); }
                if ui.button("Snap to Grid").clicked() { editor.snap_all_to_grid(0.5); }
                if ui.button("Randomize Colors").clicked() { editor.randomize_colors(); }
            });
            if ui.button(RichText::new("Clear All Bodies & Joints").color(Color32::RED)).clicked() {
                editor.clear_all();
            }
        });
}

pub fn show_joint_constraint_graph_window(ctx: &egui::Context, open: &mut bool, editor: &PhysicsEditor) {
    egui::Window::new("Constraint Graph")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(400.0, 300.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Joint topology — bodies as nodes, joints as edges.").small().color(Color32::GRAY));
            draw_joint_constraint_graph(ui, editor);
        });
}

pub fn show_collision_layer_details(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Collision Layers Detail").color(Color32::from_rgb(200, 200, 100)))
        .default_open(false)
        .show(ui, |ui| {
            let layer_names = ["Default","Player","Enemy","Projectile","Terrain","Trigger","Debris","Vehicle","NPC","Water","Sensor","UI","Overlay","Ghost","Custom1","Custom2"];
            for body in editor.bodies.iter_mut() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&body.name).small().strong());
                    ui.label(RichText::new(format!("Layer: {}", layer_names.get(body.collision_layer as usize).copied().unwrap_or("?"))).small().color(Color32::GRAY));
                    egui::ComboBox::from_id_salt(format!("body_layer_{}", body.name))
                        .selected_text(layer_names.get(body.collision_layer as usize).copied().unwrap_or("?"))
                        .show_ui(ui, |ui| {
                            for (i, &name) in layer_names.iter().enumerate() {
                                ui.selectable_value(&mut body.collision_layer, i as u32, name);
                            }
                        });
                });
            }
        });
}

pub fn show_body_mass_distribution(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Mass Distribution").color(Color32::from_rgb(100, 200, 220)))
        .default_open(false)
        .show(ui, |ui| {
            let dynamic_bodies: Vec<&RigidBody> = editor.bodies.iter().filter(|b| b.body_type == BodyType::Dynamic).collect();
            if dynamic_bodies.is_empty() { ui.label("No dynamic bodies."); return; }
            let total_mass: f32 = dynamic_bodies.iter().map(|b| b.mass).sum::<f32>().max(0.001);
            let max_mass = dynamic_bodies.iter().map(|b| b.mass).fold(0.0f32, f32::max);
            for body in &dynamic_bodies {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&body.name).small());
                    let pct = body.mass / total_mass;
                    let bar_w = ui.available_width() * 0.5 * body.mass / max_mass.max(0.001);
                    let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(bar_w.max(2.0), 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(bar_rect, 1.0, Color32::from_rgb(80, 160, 220));
                    ui.label(RichText::new(format!("{:.2} kg ({:.1}%)", body.mass, pct * 100.0)).small().color(Color32::GRAY));
                });
            }
            ui.separator();
            let cm = editor.center_of_mass();
            ui.label(RichText::new(format!("Total: {:.2} kg  CoM: ({:.2}, {:.2})", total_mass, cm[0], cm[1])).small().color(Color32::from_rgb(150, 200, 255)));
        });
}

// ============================================================
// BODY VELOCITY VECTORS OVERLAY
// ============================================================

pub fn draw_velocity_vectors(painter: &Painter, bodies: &[RigidBody], world_to_screen: impl Fn([f32; 2]) -> Pos2) {
    for body in bodies {
        if body.body_type != BodyType::Dynamic { continue; }
        let spd = (body.velocity[0] * body.velocity[0] + body.velocity[1] * body.velocity[1]).sqrt();
        if spd < 0.05 { continue; }
        let from = world_to_screen(body.position);
        let scale = 6.0;
        let to = Pos2::new(from.x + body.velocity[0] * scale, from.y - body.velocity[1] * scale);
        let speed_color = if spd < 2.0 { Color32::from_rgb(80, 200, 80) }
            else if spd < 5.0 { Color32::YELLOW }
            else { Color32::RED };
        painter.line_segment([from, to], Stroke::new(1.5, speed_color));
        // Arrow head
        let dir = (to - from).normalized();
        let perp = Vec2::new(-dir.y, dir.x);
        let head_size = 5.0;
        let tip = to;
        let left = tip - dir * head_size + perp * head_size * 0.5;
        let right = tip - dir * head_size - perp * head_size * 0.5;
        painter.add(Shape::Path(egui::epaint::PathShape::closed_line(vec![tip, left, right], Stroke::new(1.0, speed_color))));
    }
}

// ============================================================
// BODY BOUNDING BOX OVERLAY
// ============================================================

pub fn draw_body_aabb_overlays(painter: &Painter, bodies: &[RigidBody], world_to_screen: impl Fn([f32; 2]) -> Pos2, scale: f32) {
    for body in bodies {
        let (aabb_min, aabb_max) = body_aabb(body);
        let min_scr = world_to_screen(aabb_min);
        let max_scr = world_to_screen(aabb_max);
        let box_rect = Rect::from_min_max(
            Pos2::new(min_scr.x.min(max_scr.x), min_scr.y.min(max_scr.y)),
            Pos2::new(min_scr.x.max(max_scr.x), min_scr.y.max(max_scr.y)),
        );
        let _ = scale;
        painter.rect_stroke(box_rect, 0.0, Stroke::new(0.5, Color32::from_rgba_premultiplied(200, 200, 100, 80)), egui::StrokeKind::Outside);
    }
}

// ============================================================
// PHYSICS DEBUG OVERLAY OPTIONS
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PhysicsDebugOverlay {
    pub show_aabbs: bool,
    pub show_velocity: bool,
    pub show_center_of_mass: bool,
    pub show_contacts: bool,
    pub show_joint_anchors: bool,
    pub show_body_names: bool,
    pub show_grid: bool,
    pub grid_size: f32,
}

impl PhysicsDebugOverlay {
    pub fn new() -> Self {
        Self { show_grid: true, grid_size: 1.0, show_body_names: true, show_velocity: true, ..Default::default() }
    }
}

pub fn show_debug_overlay_controls(ui: &mut egui::Ui, overlay: &mut PhysicsDebugOverlay) {
    egui::CollapsingHeader::new(RichText::new("Debug Overlays").color(Color32::from_rgb(180, 200, 150)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("dbg_overlay").num_columns(2).spacing(Vec2::new(10.0, 3.0)).show(ui, |ui| {
                ui.checkbox(&mut overlay.show_aabbs, "AABBs"); ui.checkbox(&mut overlay.show_velocity, "Velocity vectors"); ui.end_row();
                ui.checkbox(&mut overlay.show_center_of_mass, "Center of mass"); ui.checkbox(&mut overlay.show_contacts, "Contact points"); ui.end_row();
                ui.checkbox(&mut overlay.show_joint_anchors, "Joint anchors"); ui.checkbox(&mut overlay.show_body_names, "Body names"); ui.end_row();
                ui.checkbox(&mut overlay.show_grid, "Grid");
                ui.add(egui::DragValue::new(&mut overlay.grid_size).speed(0.05).prefix("size:").clamp_range(0.1f32..=10.0));
                ui.end_row();
            });
        });
}

// ============================================================
// CONTACT MANIFOLD VISUALIZATION
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ContactPoint {
    pub position: [f32; 2],
    pub normal: [f32; 2],
    pub depth: f32,
    pub body_a: usize,
    pub body_b: usize,
}

pub fn draw_contact_points(painter: &Painter, contacts: &[ContactPoint], world_to_screen: impl Fn([f32; 2]) -> Pos2) {
    for cp in contacts {
        let pos = world_to_screen(cp.position);
        painter.circle_filled(pos, 3.5, Color32::from_rgb(255, 80, 80));
        let nx = cp.normal[0] * 12.0;
        let ny = -cp.normal[1] * 12.0;
        painter.line_segment([pos, Pos2::new(pos.x + nx, pos.y + ny)], Stroke::new(1.5, Color32::from_rgb(255, 150, 50)));
        painter.text(pos + Vec2::new(3.0, -8.0), egui::Align2::LEFT_BOTTOM, format!("{:.2}m", cp.depth), FontId::monospace(7.0), Color32::from_rgb(255, 180, 100));
    }
}

// ============================================================
// PHYSICS BODY PROPERTY EDITOR (BATCH)
// ============================================================

pub fn show_batch_property_editor(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Batch Properties").color(Color32::from_rgb(220, 160, 80)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Apply value to all selected bodies").small().color(Color32::GRAY));
            let mut mass_val = 1.0f32;
            let mut restitution_val = 0.5f32;
            let mut friction_val = 0.5f32;
            let mut lin_damping_val = 0.1f32;
            ui.horizontal(|ui| {
                ui.label("Mass:");
                ui.add(egui::DragValue::new(&mut mass_val).speed(0.1).clamp_range(0.001f32..=1000.0));
                if ui.small_button("Apply All").clicked() {
                    for body in &mut editor.bodies {
                        if body.body_type == BodyType::Dynamic { body.mass = mass_val; }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Restitution:");
                ui.add(egui::DragValue::new(&mut restitution_val).speed(0.01).clamp_range(0.0f32..=1.0));
                if ui.small_button("Apply All").clicked() {
                    for body in &mut editor.bodies { body.collider_props.restitution = restitution_val; }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Friction:");
                ui.add(egui::DragValue::new(&mut friction_val).speed(0.01).clamp_range(0.0f32..=5.0));
                if ui.small_button("Apply All").clicked() {
                    for body in &mut editor.bodies { body.collider_props.friction = friction_val; }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Lin. Damping:");
                ui.add(egui::DragValue::new(&mut lin_damping_val).speed(0.01).clamp_range(0.0f32..=1.0));
                if ui.small_button("Apply All").clicked() {
                    for body in &mut editor.bodies { body.linear_drag = lin_damping_val; }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Make All Static").clicked() { for body in &mut editor.bodies { body.body_type = BodyType::Static; } }
                if ui.button("Make All Dynamic").clicked() { for body in &mut editor.bodies { body.body_type = BodyType::Dynamic; } }
            });
        });
}

// ============================================================
// PHYSICS JOINT QUICK CONNECT
// ============================================================

pub fn show_joint_quick_connect(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Quick Connect").color(Color32::from_rgb(100, 200, 180)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Quickly connect two bodies with a joint.").small().color(Color32::GRAY));
            let n = editor.bodies.len();
            if n < 2 { ui.label("Need at least 2 bodies."); return; }
            let mut body_a = 0usize;
            let mut body_b = 1usize;
            let mut joint_type = 0usize;

            let body_names: Vec<String> = editor.bodies.iter().map(|b| b.name.clone()).collect();

            ui.horizontal(|ui| {
                ui.label("A:");
                egui::ComboBox::from_id_salt("qc_body_a")
                    .selected_text(&body_names[body_a])
                    .show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            ui.selectable_value(&mut body_a, i, name);
                        }
                    });
                ui.label("B:");
                egui::ComboBox::from_id_salt("qc_body_b")
                    .selected_text(&body_names[body_b])
                    .show_ui(ui, |ui| {
                        for (i, name) in body_names.iter().enumerate() {
                            ui.selectable_value(&mut body_b, i, name);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::from_id_salt("qc_joint_type")
                    .selected_text(["Fixed","Hinge","Slider","Spring","Distance","Pulley"][joint_type])
                    .show_ui(ui, |ui| {
                        for (i, &name) in ["Fixed","Hinge","Slider","Spring","Distance","Pulley"].iter().enumerate() {
                            ui.selectable_value(&mut joint_type, i, name);
                        }
                    });
                if ui.button("Connect").clicked() && body_a != body_b {
                    let joint = match joint_type {
                        0 => Joint::Fixed { body_a, body_b, break_force: f32::INFINITY },
                        1 => Joint::Hinge { body_a, body_b, anchor: [0.0, 0.0], lower_angle: -1.57, upper_angle: 1.57, motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0 },
                        2 => Joint::Slider { body_a, body_b, axis: [1.0, 0.0], lower_limit: -2.0, upper_limit: 2.0, motor_enabled: false, motor_speed: 0.0, motor_max_force: 0.0 },
                        3 => Joint::Spring { body_a, body_b, rest_length: 2.0, stiffness: 5.0, damping: 0.3 },
                        4 => Joint::Distance { body_a, body_b, min_distance: 1.0, max_distance: 3.0 },
                        _ => Joint::Fixed { body_a, body_b, break_force: f32::INFINITY },
                    };
                    editor.joints.push(joint);
                }
            });
        });
}

// ============================================================
// PHYSICS UNDO HISTORY
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhysicsHistoryEntry {
    pub label: String,
    pub bodies_snapshot: Vec<RigidBody>,
    pub joints_snapshot: Vec<Joint>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PhysicsUndoHistory {
    pub entries: Vec<PhysicsHistoryEntry>,
    pub current_index: usize,
    pub max_entries: usize,
}

impl PhysicsUndoHistory {
    pub fn new() -> Self { Self { entries: Vec::new(), current_index: 0, max_entries: 50 } }

    pub fn push(&mut self, label: &str, editor: &PhysicsEditor) {
        self.entries.truncate(self.current_index);
        self.entries.push(PhysicsHistoryEntry {
            label: label.to_string(),
            bodies_snapshot: editor.bodies.clone(),
            joints_snapshot: editor.joints.clone(),
        });
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.current_index = self.entries.len();
    }

    pub fn undo(&mut self, editor: &mut PhysicsEditor) {
        if self.current_index > 1 {
            self.current_index -= 1;
            let entry = &self.entries[self.current_index - 1];
            editor.bodies = entry.bodies_snapshot.clone();
            editor.joints = entry.joints_snapshot.clone();
        }
    }

    pub fn redo(&mut self, editor: &mut PhysicsEditor) {
        if self.current_index < self.entries.len() {
            let entry = &self.entries[self.current_index];
            editor.bodies = entry.bodies_snapshot.clone();
            editor.joints = entry.joints_snapshot.clone();
            self.current_index += 1;
        }
    }

    pub fn can_undo(&self) -> bool { self.current_index > 1 }
    pub fn can_redo(&self) -> bool { self.current_index < self.entries.len() }
}

pub fn show_undo_history_panel(ui: &mut egui::Ui, history: &mut PhysicsUndoHistory, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Undo History").color(Color32::from_rgb(180, 180, 220)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let undo_enabled = history.can_undo();
                let redo_enabled = history.can_redo();
                if ui.add_enabled(undo_enabled, egui::Button::new("Undo")).clicked() {
                    history.undo(editor);
                }
                if ui.add_enabled(redo_enabled, egui::Button::new("Redo")).clicked() {
                    history.redo(editor);
                }
                ui.label(RichText::new(format!("{}/{}", history.current_index, history.entries.len())).small().color(Color32::GRAY));
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                for (i, entry) in history.entries.iter().enumerate() {
                    let is_current = i + 1 == history.current_index;
                    let label_color = if is_current { Color32::from_rgb(100, 220, 150) } else { Color32::GRAY };
                    ui.label(RichText::new(format!("{}: {}", i + 1, entry.label)).small().color(label_color));
                }
            });
        });
}

// ============================================================
// PHYSICS SELECTION MULTI-TOOL
// ============================================================

pub fn show_multi_select_panel(ui: &mut egui::Ui, editor: &mut PhysicsEditor, selected_bodies: &mut Vec<usize>) {
    egui::CollapsingHeader::new(RichText::new("Multi-Select").color(Color32::from_rgb(200, 200, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.small_button("Select All").clicked() { *selected_bodies = (0..editor.bodies.len()).collect(); }
                if ui.small_button("Select None").clicked() { selected_bodies.clear(); }
                if ui.small_button("Invert").clicked() {
                    let all: Vec<usize> = (0..editor.bodies.len()).collect();
                    *selected_bodies = all.into_iter().filter(|i| !selected_bodies.contains(i)).collect();
                }
            });
            ui.horizontal(|ui| {
                if ui.small_button("Select Dynamic").clicked() {
                    *selected_bodies = editor.bodies.iter().enumerate().filter(|(_, b)| b.body_type == BodyType::Dynamic).map(|(i, _)| i).collect();
                }
                if ui.small_button("Select Static").clicked() {
                    *selected_bodies = editor.bodies.iter().enumerate().filter(|(_, b)| b.body_type == BodyType::Static).map(|(i, _)| i).collect();
                }
            });
            ui.separator();
            ui.label(RichText::new(format!("{} bodies selected", selected_bodies.len())).small().color(Color32::GRAY));
            for body in editor.bodies.iter() {
                if let Some(idx) = editor.bodies.iter().position(|b| std::ptr::eq(b, body)) {
                    let mut checked = selected_bodies.contains(&idx);
                    if ui.checkbox(&mut checked, &body.name).changed() {
                        if checked { selected_bodies.push(idx); } else { selected_bodies.retain(|&x| x != idx); }
                    }
                }
            }
            ui.separator();
            if !selected_bodies.is_empty() {
                ui.horizontal(|ui| {
                    if ui.small_button("Delete Selected").clicked() {
                        let mut to_del: Vec<usize> = selected_bodies.clone();
                        to_del.sort_unstable_by(|a, b| b.cmp(a));
                        for idx in to_del { if idx < editor.bodies.len() { editor.bodies.remove(idx); } }
                        selected_bodies.clear();
                        editor.joints.clear();
                    }
                    if ui.small_button("Zero Vel").clicked() {
                        for &idx in selected_bodies.iter() {
                            if let Some(body) = editor.bodies.get_mut(idx) { body.velocity = [0.0, 0.0]; }
                        }
                    }
                });
            }
        });
}

// ============================================================
// INERTIA TENSOR DISPLAY
// ============================================================

pub fn compute_box_inertia(mass: f32, width: f32, height: f32) -> f32 {
    mass / 12.0 * (width * width + height * height)
}

pub fn compute_circle_inertia(mass: f32, radius: f32) -> f32 {
    0.5 * mass * radius * radius
}

pub fn show_inertia_display(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Inertia Tensors").color(Color32::from_rgb(180, 180, 255)))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("inertia_grid").num_columns(3).spacing(Vec2::new(8.0, 3.0)).striped(true).show(ui, |ui| {
                ui.label(RichText::new("Body").strong().small()); ui.label(RichText::new("Mass").strong().small()); ui.label(RichText::new("MOI").strong().small()); ui.end_row();
                for body in &editor.bodies {
                    if body.body_type != BodyType::Dynamic { continue; }
                    let moi = match &body.collider {
                        Collider::Circle { radius } => compute_circle_inertia(body.mass, *radius),
                        Collider::Box { width, height } => compute_box_inertia(body.mass, *width, *height),
                        _ => body.mass * 1.0,
                    };
                    ui.label(RichText::new(&body.name).small());
                    ui.label(RichText::new(format!("{:.2}kg", body.mass)).small().monospace());
                    ui.label(RichText::new(format!("{:.4}", moi)).small().monospace().color(Color32::from_rgb(180, 200, 255)));
                    ui.end_row();
                }
            });
        });
}

// ============================================================
// PHYSICS QUICK ACTIONS BAR
// ============================================================

pub fn show_physics_quick_actions(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    ui.horizontal(|ui| {
        if ui.button("Add Box").clicked() {
            let n = editor.bodies.len();
            editor.bodies.push(RigidBody {
                name: format!("Box_{}", n),
                body_type: BodyType::Dynamic,
                position: [0.0, 2.0 + n as f32 * 0.5],
                mass: 1.0,
                collider: Collider::Box { width: 1.0, height: 1.0 },
                color: [100, 180, 255],
                ..RigidBody::default()
            });
        }
        if ui.button("Add Circle").clicked() {
            let n = editor.bodies.len();
            editor.bodies.push(RigidBody {
                name: format!("Circle_{}", n),
                body_type: BodyType::Dynamic,
                position: [2.0 + n as f32 * 0.5, 2.0],
                mass: 1.0,
                collider: Collider::Circle { radius: 0.5 },
                color: [255, 180, 80],
                ..RigidBody::default()
            });
        }
        if ui.button("Add Ground").clicked() {
            editor.bodies.push(RigidBody {
                name: "Ground".to_string(),
                body_type: BodyType::Static,
                position: [0.0, -5.0],
                mass: 0.0,
                collider: Collider::Box { width: 20.0, height: 1.0 },
                color: [160, 160, 120],
                ..RigidBody::default()
            });
        }
        ui.separator();
        if ui.small_button("Simulate ON").clicked() { editor.simulating = true; }
        if ui.small_button("Simulate OFF").clicked() { editor.simulating = false; editor.zero_all_velocities(); }
        ui.label(RichText::new(if editor.simulating { "SIM" } else { "PAUSED" }).color(if editor.simulating { Color32::from_rgb(100, 220, 100) } else { Color32::GRAY }).strong().small());
    });
}

// ============================================================
// COLLISION FILTER EDITOR
// ============================================================

pub fn show_collision_filter_editor(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Collision Filters").color(Color32::from_rgb(220, 180, 100)))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(RichText::new("Set per-body collision layer and mask.").small().color(Color32::GRAY));
            let layer_names = ["Default","Player","Enemy","Projectile","Terrain","Trigger","Debris","Vehicle","NPC","Water","Sensor","UI","Overlay","Ghost","Custom1","Custom2"];
            for body in editor.bodies.iter_mut() {
                ui.collapsing(body.name.clone(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Layer:");
                        egui::ComboBox::from_id_salt(format!("cf_layer_{}", body.name))
                            .selected_text(layer_names.get(body.collision_layer as usize).copied().unwrap_or("?"))
                            .show_ui(ui, |ui| {
                                for (i, &name) in layer_names.iter().enumerate() {
                                    ui.selectable_value(&mut body.collision_layer, i as u32, name);
                                }
                            });
                    });
                    ui.label("Collision mask:");
                    let mut mask = body.collision_mask;
                    let mut changed = false;
                    ui.horizontal_wrapped(|ui| {
                        for (i, &name) in layer_names.iter().enumerate() {
                            let mut bit = (mask >> i) & 1 == 1;
                            if ui.checkbox(&mut bit, name).changed() {
                                if bit { mask |= 1 << i; } else { mask &= !(1 << i); }
                                changed = true;
                            }
                        }
                    });
                    if changed { body.collision_mask = mask; }
                });
            }
        });
}

// ============================================================
// PHYSICS PRESET JOINTS
// ============================================================

pub fn show_preset_joint_library(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Joint Presets").color(Color32::from_rgb(180, 220, 180)))
        .default_open(false)
        .show(ui, |ui| {
            let presets: Vec<(&str, Joint)> = vec![
                ("Door Hinge", Joint::Hinge { body_a: 0, body_b: 1, anchor: [0.0, 0.0], lower_angle: 0.0, upper_angle: std::f32::consts::FRAC_PI_2, motor_enabled: false, motor_speed: 0.0, motor_max_torque: 0.0 }),
                ("Stiff Spring", Joint::Spring { body_a: 0, body_b: 1, rest_length: 2.0, stiffness: 50.0, damping: 5.0 }),
                ("Loose Spring", Joint::Spring { body_a: 0, body_b: 1, rest_length: 3.0, stiffness: 5.0, damping: 0.5 }),
                ("Fixed Weld", Joint::Fixed { body_a: 0, body_b: 1, break_force: f32::INFINITY }),
                ("Short Rope", Joint::Distance { body_a: 0, body_b: 1, min_distance: 0.0, max_distance: 2.0 }),
                ("Rail Slider", Joint::Slider { body_a: 0, body_b: 1, axis: [1.0, 0.0], lower_limit: -5.0, upper_limit: 5.0, motor_enabled: false, motor_speed: 0.0, motor_max_force: 0.0 }),
            ];
            ui.label(RichText::new("Click to add. Requires 2+ bodies.").small().color(Color32::GRAY));
            if editor.bodies.len() < 2 { ui.label(RichText::new("Need at least 2 bodies first.").color(Color32::YELLOW).small()); return; }
            for (label, preset) in presets {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).color(Color32::from_rgb(180, 200, 180)));
                    if ui.small_button("Add").clicked() {
                        editor.joints.push(preset);
                    }
                });
            }
        });
}

// ============================================================
// BODY NOTES PANEL
// ============================================================

pub fn show_body_notes(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Body Notes").color(Color32::from_rgb(200, 220, 160)))
        .default_open(false)
        .show(ui, |ui| {
            for body in editor.bodies.iter_mut() {
                ui.horizontal(|ui| { ui.label(RichText::new(&body.name).small().strong()); });
                ui.add(egui::TextEdit::multiline(&mut body.notes).desired_rows(2).hint_text("Notes...").desired_width(f32::INFINITY));
                ui.separator();
            }
        });
}

// ============================================================
// PHYSICS FULL PANEL WINDOW
// ============================================================

pub fn show_full_physics_panel(ctx: &egui::Context, editor: &mut PhysicsEditor, dt: f32, open: &mut bool) {
    egui::Window::new("Full Physics Editor")
        .open(open)
        .resizable(true)
        .default_size(Vec2::new(1000.0, 700.0))
        .show(ctx, |ui| {
            show_physics_quick_actions(ui, editor);
            ui.separator();
            egui::SidePanel::left("phys_sidebar").min_width(220.0).show_inside(ui, |ui| {
                show_scenario_spawner(ui, editor);
                show_ragdoll_spawner(ui, editor);
                show_joint_quick_connect(ui, editor);
                show_preset_joint_library(ui, editor);
            });
            egui::SidePanel::right("phys_right").min_width(200.0).show_inside(ui, |ui| {
                show_batch_property_editor(ui, editor);
                show_body_mass_distribution(ui, editor);
                show_inertia_display(ui, editor);
                show_collision_filter_editor(ui, editor);
            });
            egui::CentralPanel::default().show_inside(ui, |ui| {
                show(ui, editor, dt);
            });
        });
}

// ============================================================
// POLYGON COLLIDER VERTEX EDITOR
// ============================================================

pub fn show_polygon_vertex_editor(ui: &mut egui::Ui, vertices: &mut Vec<[f32; 2]>) {
    egui::CollapsingHeader::new(RichText::new("Polygon Vertices").color(Color32::from_rgb(200, 200, 100)))
        .default_open(false)
        .show(ui, |ui| {
            let desired = Vec2::new(ui.available_width().min(200.0), 150.0);
            let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 26));

            let cx = rect.center().x; let cy = rect.center().y;
            let scale = 30.0;

            if vertices.len() >= 3 {
                let pts: Vec<Pos2> = vertices.iter().map(|v| Pos2::new(cx + v[0] * scale, cy - v[1] * scale)).collect();
                painter.add(Shape::Path(egui::epaint::PathShape::closed_line(pts.clone(), Stroke::new(1.0, Color32::from_rgba_premultiplied(80, 120, 200, 60)))));
                for w in pts.windows(2) { painter.line_segment([w[0], w[1]], Stroke::new(1.5, Color32::from_rgb(100, 160, 255))); }
                if let (Some(&first), Some(&last)) = (pts.first(), pts.last()) {
                    painter.line_segment([last, first], Stroke::new(1.5, Color32::from_rgb(100, 160, 255)));
                }
            }
            for (i, v) in vertices.iter().enumerate() {
                let p = Pos2::new(cx + v[0] * scale, cy - v[1] * scale);
                painter.circle_filled(p, 4.0, Color32::from_rgb(255, 200, 80));
                painter.text(p + Vec2::new(5.0, -5.0), egui::Align2::LEFT_BOTTOM, format!("{}", i), FontId::monospace(8.0), Color32::WHITE);
            }
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let vx = (pos.x - cx) / scale;
                    let vy = -(pos.y - cy) / scale;
                    vertices.push([vx, vy]);
                }
            }
            ui.label(RichText::new("Click in preview to add vertices").small().color(Color32::GRAY));

            let mut to_remove = None;
            for (i, v) in vertices.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}: ", i)).small().monospace());
                    ui.add(egui::DragValue::new(&mut v[0]).speed(0.05).prefix("x:"));
                    ui.add(egui::DragValue::new(&mut v[1]).speed(0.05).prefix("y:"));
                    if ui.small_button("X").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(idx) = to_remove { vertices.remove(idx); }
            ui.horizontal(|ui| {
                if ui.small_button("Clear").clicked() { vertices.clear(); }
                if ui.small_button("Square").clicked() { *vertices = vec![[-1.0,-1.0],[1.0,-1.0],[1.0,1.0],[-1.0,1.0]]; }
                if ui.small_button("Triangle").clicked() { *vertices = vec![[0.0,1.0],[-1.0,-1.0],[1.0,-1.0]]; }
            });
        });
}

// ============================================================
// SPRING CONSTANT CALCULATOR
// ============================================================

pub fn show_spring_calculator(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Spring Calculator").color(Color32::from_rgb(100, 220, 160)))
        .default_open(false)
        .show(ui, |ui| {
            let mut mass = 1.0f32;
            let mut target_freq = 2.0f32;
            let mut damping_ratio = 0.5f32;
            ui.horizontal(|ui| {
                ui.label("Mass:");
                ui.add(egui::DragValue::new(&mut mass).speed(0.1).suffix("kg").clamp_range(0.01f32..=1000.0));
            });
            ui.horizontal(|ui| {
                ui.label("Target freq:");
                ui.add(egui::DragValue::new(&mut target_freq).speed(0.1).suffix("Hz").clamp_range(0.1f32..=50.0));
            });
            ui.horizontal(|ui| {
                ui.label("Damping ratio:");
                ui.add(egui::DragValue::new(&mut damping_ratio).speed(0.01).clamp_range(0.0f32..=2.0));
            });
            let omega = 2.0 * std::f32::consts::PI * target_freq;
            let k = mass * omega * omega;
            let c = 2.0 * damping_ratio * (mass * k).sqrt();
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Stiffness k = {:.2}", k)).color(Color32::from_rgb(100, 220, 160)).monospace());
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Damping c = {:.4}", c)).color(Color32::from_rgb(200, 180, 100)).monospace());
            });
            let period = 1.0 / target_freq;
            ui.label(RichText::new(format!("Period: {:.3}s  Angular freq: {:.2} rad/s", period, omega)).small().color(Color32::GRAY));
        });
}

// ============================================================
// BODY TRANSFORM PANEL
// ============================================================

pub fn show_body_transform_panel(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Body Transform").color(Color32::from_rgb(200, 180, 100)))
        .default_open(true)
        .show(ui, |ui| {
            if let Some(idx) = editor.selected_body {
                if let Some(body) = editor.bodies.get_mut(idx) {
                    egui::Grid::new("body_transform").num_columns(2).spacing(Vec2::new(8.0, 4.0)).show(ui, |ui| {
                        ui.label("Position X:"); ui.add(egui::DragValue::new(&mut body.position[0]).speed(0.05)); ui.end_row();
                        ui.label("Position Y:"); ui.add(egui::DragValue::new(&mut body.position[1]).speed(0.05)); ui.end_row();
                        ui.label("Rotation:"); ui.add(egui::DragValue::new(&mut body.rotation).speed(0.01).suffix("rad")); ui.end_row();
                        ui.label("Vel X:"); ui.add(egui::DragValue::new(&mut body.velocity[0]).speed(0.05)); ui.end_row();
                        ui.label("Vel Y:"); ui.add(egui::DragValue::new(&mut body.velocity[1]).speed(0.05)); ui.end_row();
                        ui.label("Ang vel:"); ui.add(egui::DragValue::new(&mut body.angular_velocity).speed(0.01)); ui.end_row();
                    });
                    let spd = (body.velocity[0] * body.velocity[0] + body.velocity[1] * body.velocity[1]).sqrt();
                    let ke = 0.5 * body.mass * spd * spd;
                    ui.label(RichText::new(format!("Speed: {:.3}m/s  KE: {:.3}J", spd, ke)).small().color(Color32::from_rgb(180, 200, 255)));
                }
            } else {
                ui.label(RichText::new("No body selected.").small().color(Color32::GRAY));
            }
        });
}

// ============================================================
// PHYSICS WORLD SERIALIZE/DESERIALIZE
// ============================================================

pub fn show_world_io_panel(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("World I/O").color(Color32::from_rgb(180, 220, 200)))
        .default_open(false)
        .show(ui, |ui| {
            let bodies_json = serde_json::to_string_pretty(&editor.bodies).unwrap_or_else(|_| "{}".to_string());
            let joints_json = serde_json::to_string_pretty(&editor.joints).unwrap_or_else(|_| "{}".to_string());
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Bodies: {} chars", bodies_json.len())).small().color(Color32::GRAY));
                if ui.small_button("Copy Bodies JSON").clicked() { ui.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(bodies_json))); }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Joints: {} chars", joints_json.len())).small().color(Color32::GRAY));
                if ui.small_button("Copy Joints JSON").clicked() { ui.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(joints_json))); }
            });
            let (min, max) = editor.scene_bounds();
            ui.label(RichText::new(format!("Bounds: ({:.1},{:.1}) to ({:.1},{:.1})", min[0], min[1], max[0], max[1])).small().color(Color32::GRAY));
            let cm = editor.center_of_mass();
            ui.label(RichText::new(format!("Center of mass: ({:.2}, {:.2})", cm[0], cm[1])).small().color(Color32::GRAY));
        });
}

// ============================================================
// PHYSICS KEYBOARD SHORTCUTS HELP
// ============================================================

pub fn show_physics_keyboard_help(ui: &mut egui::Ui) {
    egui::CollapsingHeader::new(RichText::new("Keyboard Shortcuts").color(Color32::from_rgb(200, 200, 200)))
        .default_open(false)
        .show(ui, |ui| {
            let shortcuts = [
                ("Space", "Toggle simulation"),
                ("G", "Toggle debug grid"),
                ("V", "Toggle velocity vectors"),
                ("B", "Toggle AABBs"),
                ("N", "Toggle body names"),
                ("Delete", "Delete selected body"),
                ("Ctrl+D", "Duplicate selected"),
                ("Ctrl+Z", "Undo"),
                ("Ctrl+Y", "Redo"),
                ("Ctrl+A", "Select all dynamic"),
                ("Ctrl+G", "Snap to grid"),
                ("F", "Fit view to scene"),
                ("R", "Randomize colors"),
                ("0", "Zero all velocities"),
                ("Ctrl+S", "Save scene JSON"),
            ];
            egui::Grid::new("phys_shortcuts").num_columns(2).spacing(Vec2::new(12.0, 2.0)).show(ui, |ui| {
                for (key, action) in shortcuts {
                    ui.label(RichText::new(key).monospace().color(Color32::from_rgb(255, 220, 80)));
                    ui.label(RichText::new(action).small().color(Color32::LIGHT_GRAY));
                    ui.end_row();
                }
            });
        });
}

// ============================================================
// QUICK JOINT INSPECTOR (STATUS BAR)
// ============================================================

pub fn show_quick_joint_inspector(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    if let Some(idx) = editor.selected_joint {
        if let Some(joint) = editor.joints.get(idx) {
            let (a, b, type_name) = match joint {
                Joint::Fixed { body_a, body_b, .. } => (*body_a, *body_b, "Fixed"),
                Joint::Hinge { body_a, body_b, .. } => (*body_a, *body_b, "Hinge"),
                Joint::Slider { body_a, body_b, .. } => (*body_a, *body_b, "Slider"),
                Joint::Spring { body_a, body_b, .. } => (*body_a, *body_b, "Spring"),
                Joint::Distance { body_a, body_b, .. } => (*body_a, *body_b, "Distance"),
                Joint::Pulley { body_a, body_b, .. } => (*body_a, *body_b, "Pulley"),
            };
            let name_a = editor.bodies.get(a).map(|b| b.name.as_str()).unwrap_or("?");
            let name_b = editor.bodies.get(b).map(|b| b.name.as_str()).unwrap_or("?");
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Joint #{}: {} -- {} -- {}", idx, name_a, type_name, name_b)).color(Color32::from_rgb(180, 220, 180)).small());
            });
        }
    }
}

// ============================================================
// PHYSICS STATUS BAR
// ============================================================

pub fn show_physics_status_bar(ui: &mut egui::Ui, editor: &PhysicsEditor) {
    ui.horizontal(|ui| {
        let (s, d, k) = editor.body_count_by_type();
        let sim_status = if editor.simulating { RichText::new("SIMULATING").color(Color32::from_rgb(100, 220, 100)).strong().small() } else { RichText::new("PAUSED").color(Color32::GRAY).strong().small() };
        ui.label(sim_status);
        ui.separator();
        ui.label(RichText::new(format!("S:{} D:{} K:{}  J:{}  Gravity:({:.1},{:.1})", s, d, k, editor.joints.len(), editor.gravity[0], editor.gravity[1])).small().color(Color32::GRAY));
        ui.separator();
        let cm = editor.center_of_mass();
        ui.label(RichText::new(format!("CoM:({:.1},{:.1})  Mass:{:.1}kg", cm[0], cm[1], editor.total_mass())).small().color(Color32::from_rgb(180, 200, 220)));
    });
}

// ============================================================
// TORQUE MOTOR CONTROLLER DISPLAY
// ============================================================

pub fn draw_motor_controller(ui: &mut egui::Ui, motor_speed: f32, max_torque: f32, current_angle: f32, lower: f32, upper: f32) {
    let desired = Vec2::new(ui.available_width().min(160.0), 80.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 18, 26));
    let center = Pos2::new(rect.center().x, rect.center().y + 5.0);
    let r = 28.0;
    painter.circle_stroke(center, r, Stroke::new(1.5, Color32::from_rgb(60, 60, 80)));
    // Limit arc
    let a1 = lower - std::f32::consts::FRAC_PI_2;
    let a2 = upper - std::f32::consts::FRAC_PI_2;
    let arc_pts: Vec<Pos2> = (0..=20).map(|i| {
        let t = i as f32 / 20.0;
        let ang = a1 + (a2 - a1) * t;
        Pos2::new(center.x + r * ang.cos(), center.y + r * ang.sin())
    }).collect();
    for w in arc_pts.windows(2) { painter.line_segment([w[0], w[1]], Stroke::new(3.0, Color32::from_rgba_premultiplied(80, 200, 80, 80))); }
    // Current angle needle
    let needle_ang = current_angle - std::f32::consts::FRAC_PI_2;
    let needle_tip = Pos2::new(center.x + (r - 4.0) * needle_ang.cos(), center.y + (r - 4.0) * needle_ang.sin());
    painter.line_segment([center, needle_tip], Stroke::new(2.0, Color32::from_rgb(255, 200, 80)));
    painter.circle_filled(center, 3.0, Color32::from_rgb(200, 200, 200));
    painter.text(Pos2::new(rect.center().x, rect.top() + 4.0), egui::Align2::CENTER_TOP, format!("{:.1} rpm  {:.1} Nm", motor_speed * 60.0 / std::f32::consts::TAU, max_torque), FontId::monospace(7.0), Color32::GRAY);
}

// ============================================================
// ANGULAR IMPULSE SHOOTER
// ============================================================

pub fn show_impulse_shooter(ui: &mut egui::Ui, editor: &mut PhysicsEditor) {
    egui::CollapsingHeader::new(RichText::new("Impulse Shooter").color(Color32::from_rgb(255, 180, 80)))
        .default_open(false)
        .show(ui, |ui| {
            let mut angle_deg = 90.0f32;
            let mut magnitude = 5.0f32;
            ui.add(egui::Slider::new(&mut angle_deg, 0.0f32..=360.0).suffix("°").text("Angle"));
            ui.add(egui::Slider::new(&mut magnitude, 0.1f32..=50.0).text("Magnitude").logarithmic(false));
            let rad = angle_deg.to_radians();
            let impulse = [rad.cos() * magnitude, rad.sin() * magnitude];
            ui.label(RichText::new(format!("Impulse: ({:.2}, {:.2})", impulse[0], impulse[1])).small().color(Color32::GRAY));
            if ui.button("Apply to selected").clicked() {
                if let Some(idx) = editor.selected_body {
                    if let Some(body) = editor.bodies.get_mut(idx) {
                        body.velocity[0] += impulse[0] / body.mass.max(0.001);
                        body.velocity[1] += impulse[1] / body.mass.max(0.001);
                    }
                }
            }
            if ui.small_button("Apply to all dynamic").clicked() {
                editor.apply_impulse_to_all_dynamic(impulse);
            }
        });
}
