
code = r'''

// ============================================================
// EXPANSION 4: Spline Physics Simulation
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplinePhysicsBody {
    pub node_idx: usize,
    pub mass: f32,
    pub velocity: (f32, f32),
    pub acceleration: (f32, f32),
    pub is_pinned: bool,
    pub damping: f32,
    pub restitution: f32,
}

impl SplinePhysicsBody {
    pub fn new(node_idx: usize, mass: f32) -> Self {
        SplinePhysicsBody {
            node_idx,
            mass,
            velocity: (0.0, 0.0),
            acceleration: (0.0, 0.0),
            is_pinned: false,
            damping: 0.98,
            restitution: 0.3,
        }
    }

    pub fn apply_force(&mut self, fx: f32, fy: f32) {
        if self.is_pinned { return; }
        self.acceleration.0 += fx / self.mass;
        self.acceleration.1 += fy / self.mass;
    }

    pub fn step(&mut self, dt: f32) {
        if self.is_pinned { return; }
        self.velocity.0 = (self.velocity.0 + self.acceleration.0 * dt) * self.damping;
        self.velocity.1 = (self.velocity.1 + self.acceleration.1 * dt) * self.damping;
        self.acceleration = (0.0, 0.0);
    }

    pub fn kinetic_energy(&self) -> f32 {
        0.5 * self.mass * (self.velocity.0 * self.velocity.0 + self.velocity.1 * self.velocity.1)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineSpring {
    pub node_a: usize,
    pub node_b: usize,
    pub rest_length: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl SplineSpring {
    pub fn new(a: usize, b: usize, rest_len: f32, stiffness: f32) -> Self {
        SplineSpring { node_a: a, node_b: b, rest_length: rest_len, stiffness, damping: 0.1 }
    }

    pub fn compute_forces(&self, positions: &[(f32, f32)], velocities: &[(f32, f32)])
        -> ((f32, f32), (f32, f32)) {
        if self.node_a >= positions.len() || self.node_b >= positions.len() {
            return ((0.0, 0.0), (0.0, 0.0));
        }
        let pa = positions[self.node_a];
        let pb = positions[self.node_b];
        let dx = pb.0 - pa.0;
        let dy = pb.1 - pa.1;
        let dist = (dx * dx + dy * dy).sqrt().max(0.001);
        let stretch = dist - self.rest_length;
        let nx = dx / dist;
        let ny = dy / dist;

        let va = velocities[self.node_a];
        let vb = velocities[self.node_b];
        let rel_vel = (vb.0 - va.0) * nx + (vb.1 - va.1) * ny;

        let force = self.stiffness * stretch + self.damping * rel_vel;
        let fa = (nx * force, ny * force);
        let fb = (-nx * force, -ny * force);
        (fa, fb)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplinePhysicsConfig {
    pub gravity: (f32, f32),
    pub time_step: f32,
    pub substeps: u32,
    pub wind_force: (f32, f32),
    pub enable_gravity: bool,
    pub enable_wind: bool,
    pub enable_springs: bool,
    pub collision_bounds: Option<(f32, f32, f32, f32)>,
    pub node_radius: f32,
    pub ground_y: Option<f32>,
}

impl Default for SplinePhysicsConfig {
    fn default() -> Self {
        SplinePhysicsConfig {
            gravity: (0.0, 9.81),
            time_step: 1.0 / 60.0,
            substeps: 4,
            wind_force: (0.5, 0.0),
            enable_gravity: true,
            enable_wind: false,
            enable_springs: true,
            collision_bounds: None,
            node_radius: 3.0,
            ground_y: Some(500.0),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SplinePhysicsState {
    pub bodies: Vec<SplinePhysicsBody>,
    pub springs: Vec<SplineSpring>,
    pub config: SplinePhysicsConfig,
    pub time_elapsed: f32,
    pub is_simulating: bool,
    pub node_positions: Vec<(f32, f32)>,
    pub total_energy: f32,
}

impl SplinePhysicsState {
    pub fn build_from_spline(spline: &Spline, stiffness: f32) -> Self {
        let n = spline.nodes.len();
        let mut state = SplinePhysicsState::default();
        state.node_positions = spline.nodes.iter().map(|nd| (nd.position.x, nd.position.y)).collect();
        for i in 0..n {
            state.bodies.push(SplinePhysicsBody::new(i, 1.0));
        }
        if state.config.enable_springs {
            for i in 0..n.saturating_sub(1) {
                let pa = state.node_positions[i];
                let pb = state.node_positions[i+1];
                let dx = pb.0 - pa.0;
                let dy = pb.1 - pa.1;
                let rest = (dx*dx + dy*dy).sqrt();
                state.springs.push(SplineSpring::new(i, i+1, rest, stiffness));
            }
        }
        if n > 0 { state.bodies[0].is_pinned = true; }
        state
    }

    pub fn step(&mut self) {
        let dt = self.config.time_step / self.config.substeps as f32;
        for _ in 0..self.config.substeps {
            self.sub_step(dt);
        }
        self.time_elapsed += self.config.time_step;
        self.total_energy = self.bodies.iter().map(|b| b.kinetic_energy()).sum();
    }

    fn sub_step(&mut self, dt: f32) {
        // Apply global forces
        for body in self.bodies.iter_mut() {
            if body.is_pinned { continue; }
            if self.config.enable_gravity {
                body.apply_force(self.config.gravity.0 * body.mass, self.config.gravity.1 * body.mass);
            }
            if self.config.enable_wind {
                body.apply_force(self.config.wind_force.0, self.config.wind_force.1);
            }
        }

        // Apply spring forces
        if self.config.enable_springs {
            let velocities: Vec<(f32, f32)> = self.bodies.iter()
                .map(|b| b.velocity).collect();
            let forces: Vec<((f32, f32), (f32, f32))> = self.springs.iter()
                .map(|s| s.compute_forces(&self.node_positions, &velocities))
                .collect();

            for (spring_idx, spring) in self.springs.iter().enumerate() {
                let (fa, fb) = forces[spring_idx];
                if spring.node_a < self.bodies.len() {
                    self.bodies[spring.node_a].apply_force(fa.0, fa.1);
                }
                if spring.node_b < self.bodies.len() {
                    self.bodies[spring.node_b].apply_force(fb.0, fb.1);
                }
            }
        }

        // Integrate
        for (i, body) in self.bodies.iter_mut().enumerate() {
            body.step(dt);
            if i < self.node_positions.len() && !body.is_pinned {
                self.node_positions[i].0 += body.velocity.0 * dt;
                self.node_positions[i].1 += body.velocity.1 * dt;

                // Ground collision
                if let Some(ground_y) = self.config.ground_y {
                    if self.node_positions[i].1 > ground_y {
                        self.node_positions[i].1 = ground_y;
                        body.velocity.1 = -body.velocity.1 * body.restitution;
                    }
                }
            }
        }
    }
}

pub fn show_spline_physics(ui: &mut egui::Ui, state: &mut SplinePhysicsState, spline: &Spline) {
    ui.heading("Spline Physics Simulation");
    ui.horizontal(|ui| {
        if ui.button(if state.is_simulating { "Pause" } else { "Simulate" }).clicked() {
            state.is_simulating = !state.is_simulating;
        }
        if ui.button("Reset").clicked() {
            *state = SplinePhysicsState::build_from_spline(spline, 50.0);
        }
        ui.label(format!("t={:.2}s Energy={:.1}", state.time_elapsed, state.total_energy));
    });

    ui.checkbox(&mut state.config.enable_gravity, "Gravity");
    ui.checkbox(&mut state.config.enable_wind, "Wind");
    ui.checkbox(&mut state.config.enable_springs, "Springs");

    if state.config.enable_gravity {
        ui.horizontal(|ui| {
            ui.label("Gravity:");
            ui.add(egui::DragValue::new(&mut state.config.gravity.1).prefix("y:").speed(0.1));
        });
    }
    if state.config.enable_wind {
        ui.horizontal(|ui| {
            ui.label("Wind:");
            ui.add(egui::DragValue::new(&mut state.config.wind_force.0).prefix("x:").speed(0.01));
        });
    }

    ui.add(egui::Slider::new(&mut state.config.substeps, 1..=16).text("Substeps"));

    if state.is_simulating {
        state.step();
        ui.ctx().request_repaint();
    }

    // Draw simulation
    let painter_rect = ui.available_rect_before_wrap();
    let painter = ui.painter_at(painter_rect);

    // Draw simulated spline
    if state.node_positions.len() >= 2 {
        for w in state.node_positions.windows(2) {
            let p0 = egui::pos2(
                painter_rect.min.x + w[0].0,
                painter_rect.min.y + w[0].1,
            );
            let p1 = egui::pos2(
                painter_rect.min.x + w[1].0,
                painter_rect.min.y + w[1].1,
            );
            painter.line_segment([p0, p1], egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE));
        }
    }

    // Draw nodes
    for (i, pos) in state.node_positions.iter().enumerate() {
        let is_pinned = state.bodies.get(i).map(|b| b.is_pinned).unwrap_or(false);
        let p = egui::pos2(painter_rect.min.x + pos.0, painter_rect.min.y + pos.1);
        painter.circle_filled(p, state.config.node_radius,
            if is_pinned { egui::Color32::RED } else { egui::Color32::WHITE });
    }

    // Draw ground
    if let Some(gy) = state.config.ground_y {
        let y = painter_rect.min.y + gy;
        painter.line_segment(
            [egui::pos2(painter_rect.min.x, y), egui::pos2(painter_rect.max.x, y)],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 60, 20)),
        );
    }
}

// ============================================================
// EXPANSION 4: Spline Mesh Generator
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshVertex {
    pub position: (f32, f32, f32),
    pub normal: (f32, f32, f32),
    pub uv: (f32, f32),
    pub color: [u8; 4],
}

impl MeshVertex {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        MeshVertex {
            position: (x, y, z),
            normal: (0.0, 1.0, 0.0),
            uv: (0.0, 0.0),
            color: [255, 255, 255, 255],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub name: String,
    pub material_id: u32,
    pub is_closed: bool,
}

impl SplineMesh {
    pub fn new(name: String) -> Self {
        SplineMesh {
            vertices: vec![],
            indices: vec![],
            name,
            material_id: 0,
            is_closed: false,
        }
    }

    pub fn triangle_count(&self) -> usize { self.indices.len() / 3 }
    pub fn vertex_count(&self) -> usize { self.vertices.len() }

    pub fn compute_normals(&mut self) {
        let mut normals = vec![(0.0f32, 0.0f32, 0.0f32); self.vertices.len()];
        for tri in self.indices.chunks(3) {
            if tri.len() < 3 { continue; }
            let v0 = self.vertices[tri[0] as usize].position;
            let v1 = self.vertices[tri[1] as usize].position;
            let v2 = self.vertices[tri[2] as usize].position;
            let e1 = (v1.0-v0.0, v1.1-v0.1, v1.2-v0.2);
            let e2 = (v2.0-v0.0, v2.1-v0.1, v2.2-v0.2);
            let n = (
                e1.1*e2.2 - e1.2*e2.1,
                e1.2*e2.0 - e1.0*e2.2,
                e1.0*e2.1 - e1.1*e2.0,
            );
            for &vi in tri {
                let ni = &mut normals[vi as usize];
                ni.0 += n.0; ni.1 += n.1; ni.2 += n.2;
            }
        }
        for (v, n) in self.vertices.iter_mut().zip(normals.iter()) {
            let len = (n.0*n.0 + n.1*n.1 + n.2*n.2).sqrt().max(0.001);
            v.normal = (n.0/len, n.1/len, n.2/len);
        }
    }

    pub fn generate_uvs_by_length(&mut self) {
        if self.vertices.is_empty() { return; }
        let mut total_len = 0.0f32;
        let mut lengths = vec![0.0f32; self.vertices.len()];
        for i in 1..self.vertices.len() {
            let p0 = self.vertices[i-1].position;
            let p1 = self.vertices[i].position;
            let dx = p1.0 - p0.0;
            let dy = p1.1 - p0.1;
            let dz = p1.2 - p0.2;
            let d = (dx*dx + dy*dy + dz*dz).sqrt();
            total_len += d;
            lengths[i] = total_len;
        }
        if total_len < 0.001 { return; }
        for (v, l) in self.vertices.iter_mut().zip(lengths.iter()) {
            v.uv.0 = l / total_len;
        }
    }

    pub fn to_obj_string(&self) -> String {
        let mut out = format!("# Mesh: {}\n", self.name);
        for v in &self.vertices {
            out.push_str(&format!("v {} {} {}\n", v.position.0, v.position.1, v.position.2));
        }
        for v in &self.vertices {
            out.push_str(&format!("vn {} {} {}\n", v.normal.0, v.normal.1, v.normal.2));
        }
        for v in &self.vertices {
            out.push_str(&format!("vt {} {}\n", v.uv.0, v.uv.1));
        }
        for tri in self.indices.chunks(3) {
            if tri.len() == 3 {
                out.push_str(&format!("f {}/{}/{} {}/{}/{} {}/{}/{}\n",
                    tri[0]+1, tri[0]+1, tri[0]+1,
                    tri[1]+1, tri[1]+1, tri[1]+1,
                    tri[2]+1, tri[2]+1, tri[2]+1));
            }
        }
        out
    }
}

pub fn generate_tube_mesh(spline: &Spline, radius: f32, segments: u32, rings: u32) -> SplineMesh {
    let mut mesh = SplineMesh::new("SplineTube".to_string());
    if spline.nodes.len() < 2 { return mesh; }

    let samples = rings.max(2);
    let ring_segs = segments.max(3);

    for ring_idx in 0..=samples {
        let t = ring_idx as f32 / samples as f32;
        let t_scaled = t * (spline.nodes.len() - 1) as f32;
        let node_idx = (t_scaled as usize).min(spline.nodes.len() - 2);
        let local_t = t_scaled - node_idx as f32;

        let p0 = spline.nodes[node_idx].position;
        let p1 = spline.nodes[(node_idx + 1).min(spline.nodes.len() - 1)].position;

        let cx = p0.x + (p1.x - p0.x) * local_t;
        let cy = p0.y + (p1.y - p0.y) * local_t;

        let tan_x = (p1.x - p0.x).max(0.001);
        let tan_y = (p1.y - p0.y);
        let tan_len = (tan_x*tan_x + tan_y*tan_y).sqrt().max(0.001);
        let nx = -tan_y / tan_len;
        let ny = tan_x / tan_len;

        let v_start = mesh.vertices.len() as u32;
        for seg in 0..=ring_segs {
            let angle = seg as f32 / ring_segs as f32 * std::f32::consts::TAU;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let vx = cx + nx * cos_a * radius;
            let vy = cy + ny * cos_a * radius;
            let vz = sin_a * radius;
            let mut vtx = MeshVertex::new(vx, vy, vz);
            vtx.uv = (t, seg as f32 / ring_segs as f32);
            mesh.vertices.push(vtx);
        }

        if ring_idx > 0 {
            let prev_start = v_start - (ring_segs + 1);
            for seg in 0..ring_segs {
                let a = prev_start + seg;
                let b = prev_start + seg + 1;
                let c = v_start + seg;
                let d = v_start + seg + 1;
                mesh.indices.extend_from_slice(&[a, b, c, b, d, c]);
            }
        }
    }

    mesh.compute_normals();
    mesh.generate_uvs_by_length();
    mesh
}

pub fn generate_ribbon_mesh(spline: &Spline, width: f32, samples: u32) -> SplineMesh {
    let mut mesh = SplineMesh::new("SplineRibbon".to_string());
    if spline.nodes.len() < 2 { return mesh; }

    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let t_scaled = t * (spline.nodes.len() - 1) as f32;
        let node_idx = (t_scaled as usize).min(spline.nodes.len() - 2);
        let local_t = t_scaled - node_idx as f32;

        let p0 = spline.nodes[node_idx].position;
        let p1 = spline.nodes[(node_idx + 1).min(spline.nodes.len() - 1)].position;
        let cx = p0.x + (p1.x - p0.x) * local_t;
        let cy = p0.y + (p1.y - p0.y) * local_t;

        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let len = (dx*dx + dy*dy).sqrt().max(0.001);
        let nx = -dy / len;
        let ny = dx / len;

        let v_start = mesh.vertices.len() as u32;
        let mut v0 = MeshVertex::new(cx + nx * width * 0.5, cy + ny * width * 0.5, 0.0);
        v0.uv = (t, 0.0);
        let mut v1 = MeshVertex::new(cx - nx * width * 0.5, cy - ny * width * 0.5, 0.0);
        v1.uv = (t, 1.0);
        mesh.vertices.push(v0);
        mesh.vertices.push(v1);

        if i > 0 {
            let a = v_start - 2;
            let b = v_start - 1;
            let c = v_start;
            let d = v_start + 1;
            mesh.indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }

    mesh.compute_normals();
    mesh
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SplineMeshState {
    pub meshes: Vec<SplineMesh>,
    pub selected_mesh: Option<usize>,
    pub tube_radius: f32,
    pub tube_segments: u32,
    pub tube_rings: u32,
    pub ribbon_width: f32,
    pub ribbon_samples: u32,
    pub show_wireframe: bool,
    pub show_normals: bool,
    pub auto_update: bool,
}

impl SplineMeshState {
    pub fn new() -> Self {
        SplineMeshState {
            meshes: vec![],
            selected_mesh: None,
            tube_radius: 5.0,
            tube_segments: 8,
            tube_rings: 32,
            ribbon_width: 10.0,
            ribbon_samples: 64,
            show_wireframe: false,
            show_normals: false,
            auto_update: true,
        }
    }
}

pub fn show_spline_mesh_editor(ui: &mut egui::Ui, state: &mut SplineMeshState, spline: &Spline) {
    ui.heading("Spline Mesh Generator");
    ui.horizontal(|ui| {
        if ui.button("Generate Tube").clicked() {
            let mesh = generate_tube_mesh(spline, state.tube_radius,
                state.tube_segments, state.tube_rings);
            state.meshes.push(mesh);
        }
        if ui.button("Generate Ribbon").clicked() {
            let mesh = generate_ribbon_mesh(spline, state.ribbon_width, state.ribbon_samples);
            state.meshes.push(mesh);
        }
        if ui.button("Clear All").clicked() {
            state.meshes.clear();
            state.selected_mesh = None;
        }
    });

    ui.separator();
    ui.heading("Tube Settings");
    ui.add(egui::Slider::new(&mut state.tube_radius, 0.5..=100.0).text("Radius"));
    ui.add(egui::Slider::new(&mut state.tube_segments, 3..=32).text("Segments"));
    ui.add(egui::Slider::new(&mut state.tube_rings, 4..=128).text("Rings"));

    ui.heading("Ribbon Settings");
    ui.add(egui::Slider::new(&mut state.ribbon_width, 1.0..=200.0).text("Width"));
    ui.add(egui::Slider::new(&mut state.ribbon_samples, 8..=256).text("Samples"));

    ui.separator();
    ui.checkbox(&mut state.show_wireframe, "Wireframe");
    ui.checkbox(&mut state.show_normals, "Normals");
    ui.checkbox(&mut state.auto_update, "Auto Update");

    ui.separator();
    ui.heading("Generated Meshes");
    let n = state.meshes.len();
    for i in 0..n {
        let mesh = &state.meshes[i];
        let label = format!("{}: {} verts, {} tris",
            mesh.name, mesh.vertex_count(), mesh.triangle_count());
        if ui.selectable_label(state.selected_mesh == Some(i), label).clicked() {
            state.selected_mesh = Some(i);
        }
    }

    if let Some(idx) = state.selected_mesh {
        if idx < state.meshes.len() {
            let mesh = &state.meshes[idx];
            ui.separator();
            ui.label(format!("Selected: {}", mesh.name));
            ui.label(format!("Vertices: {}", mesh.vertex_count()));
            ui.label(format!("Triangles: {}", mesh.triangle_count()));
            ui.label(format!("Closed: {}", mesh.is_closed));
            if ui.button("Export OBJ").clicked() {
                let _obj = mesh.to_obj_string();
                // In real usage would write to file
            }
        }
    }
}

// ============================================================
// EXPANSION 4: Spline Animation Keyframe System
// ============================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AnimationInterpolation {
    Linear,
    CubicBezier,
    CatmullRom,
    Constant,
    EaseIn,
    EaseOut,
    EaseInOut,
    Bounce,
    Elastic,
}

impl AnimationInterpolation {
    pub fn name(&self) -> &'static str {
        match self {
            AnimationInterpolation::Linear => "Linear",
            AnimationInterpolation::CubicBezier => "Cubic Bezier",
            AnimationInterpolation::CatmullRom => "Catmull-Rom",
            AnimationInterpolation::Constant => "Constant",
            AnimationInterpolation::EaseIn => "Ease In",
            AnimationInterpolation::EaseOut => "Ease Out",
            AnimationInterpolation::EaseInOut => "Ease In-Out",
            AnimationInterpolation::Bounce => "Bounce",
            AnimationInterpolation::Elastic => "Elastic",
        }
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        match self {
            AnimationInterpolation::Linear => t,
            AnimationInterpolation::EaseIn => t * t,
            AnimationInterpolation::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            AnimationInterpolation::EaseInOut => {
                if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
            },
            AnimationInterpolation::Bounce => {
                let t2 = 1.0 - t;
                1.0 - if t2 < 1.0/2.75 {
                    7.5625 * t2 * t2
                } else if t2 < 2.0/2.75 {
                    let t3 = t2 - 1.5/2.75;
                    7.5625 * t3 * t3 + 0.75
                } else if t2 < 2.5/2.75 {
                    let t3 = t2 - 2.25/2.75;
                    7.5625 * t3 * t3 + 0.9375
                } else {
                    let t3 = t2 - 2.625/2.75;
                    7.5625 * t3 * t3 + 0.984375
                }
            },
            AnimationInterpolation::Elastic => {
                if t == 0.0 || t == 1.0 { return t; }
                let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                -(2.0f32.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * c4).sin()
            },
            AnimationInterpolation::Constant => if t >= 1.0 { 1.0 } else { 0.0 },
            _ => t,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimKeyframe {
    pub time: f32,
    pub value: f32,
    pub interpolation: AnimationInterpolation,
    pub in_tangent: f32,
    pub out_tangent: f32,
}

impl AnimKeyframe {
    pub fn new(time: f32, value: f32) -> Self {
        AnimKeyframe {
            time,
            value,
            interpolation: AnimationInterpolation::Linear,
            in_tangent: 0.0,
            out_tangent: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimCurve {
    pub name: String,
    pub keyframes: Vec<AnimKeyframe>,
    pub loop_mode: AnimLoopMode,
    pub duration: f32,
    pub min_value: f32,
    pub max_value: f32,
    pub color: egui::Color32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AnimLoopMode {
    Once,
    Loop,
    PingPong,
    Clamp,
}

impl AnimCurve {
    pub fn new(name: String) -> Self {
        AnimCurve {
            name,
            keyframes: vec![
                AnimKeyframe::new(0.0, 0.0),
                AnimKeyframe::new(1.0, 1.0),
            ],
            loop_mode: AnimLoopMode::Once,
            duration: 1.0,
            min_value: 0.0,
            max_value: 1.0,
            color: egui::Color32::from_rgb(100, 200, 100),
        }
    }

    pub fn evaluate(&self, time: f32) -> f32 {
        let t = match self.loop_mode {
            AnimLoopMode::Clamp => time.clamp(0.0, self.duration),
            AnimLoopMode::Loop => time % self.duration,
            AnimLoopMode::PingPong => {
                let t2 = time % (self.duration * 2.0);
                if t2 < self.duration { t2 } else { self.duration * 2.0 - t2 }
            },
            AnimLoopMode::Once => time.min(self.duration),
        };

        if self.keyframes.is_empty() { return self.min_value; }
        if self.keyframes.len() == 1 { return self.keyframes[0].value; }

        let first = &self.keyframes[0];
        let last = &self.keyframes[self.keyframes.len() - 1];

        if t <= first.time { return first.value; }
        if t >= last.time { return last.value; }

        for i in 0..self.keyframes.len()-1 {
            let k0 = &self.keyframes[i];
            let k1 = &self.keyframes[i+1];
            if t >= k0.time && t <= k1.time {
                let span = k1.time - k0.time;
                let local_t = if span > 0.0 { (t - k0.time) / span } else { 0.0 };
                let eased = k0.interpolation.evaluate(local_t);
                return k0.value + (k1.value - k0.value) * eased;
            }
        }
        last.value
    }

    pub fn add_keyframe(&mut self, time: f32, value: f32) {
        let kf = AnimKeyframe::new(time, value);
        let pos = self.keyframes.iter().position(|k| k.time > time).unwrap_or(self.keyframes.len());
        self.keyframes.insert(pos, kf);
        self.duration = self.keyframes.last().map(|k| k.time).unwrap_or(1.0);
    }

    pub fn remove_keyframe(&mut self, idx: usize) {
        if idx < self.keyframes.len() { self.keyframes.remove(idx); }
        self.duration = self.keyframes.last().map(|k| k.time).unwrap_or(1.0);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimClip {
    pub name: String,
    pub curves: Vec<AnimCurve>,
    pub duration: f32,
    pub fps: f32,
    pub tags: Vec<String>,
}

impl AnimClip {
    pub fn new(name: String, duration: f32) -> Self {
        AnimClip {
            name,
            curves: vec![],
            duration,
            fps: 30.0,
            tags: vec![],
        }
    }

    pub fn total_frames(&self) -> u32 { (self.duration * self.fps) as u32 }

    pub fn evaluate_all(&self, time: f32) -> Vec<(String, f32)> {
        self.curves.iter().map(|c| (c.name.clone(), c.evaluate(time))).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SplineAnimEditorState {
    pub clips: Vec<AnimClip>,
    pub selected_clip: Option<usize>,
    pub selected_curve: Option<usize>,
    pub current_time: f32,
    pub is_playing: bool,
    pub play_speed: f32,
    pub show_grid: bool,
    pub selected_keyframe: Option<usize>,
    pub curve_height: f32,
}

impl SplineAnimEditorState {
    pub fn new() -> Self {
        let mut state = SplineAnimEditorState {
            clips: vec![],
            selected_clip: None,
            selected_curve: None,
            current_time: 0.0,
            is_playing: false,
            play_speed: 1.0,
            show_grid: true,
            selected_keyframe: None,
            curve_height: 120.0,
        };
        let mut default_clip = AnimClip::new("Default".to_string(), 2.0);
        let mut pos_x = AnimCurve::new("position.x".to_string());
        pos_x.add_keyframe(0.5, 0.8);
        let mut pos_y = AnimCurve::new("position.y".to_string());
        pos_y.color = egui::Color32::from_rgb(200, 100, 100);
        pos_y.add_keyframe(0.5, 0.3);
        pos_y.add_keyframe(1.5, 0.7);
        default_clip.curves.push(pos_x);
        default_clip.curves.push(pos_y);
        state.clips.push(default_clip);
        state.selected_clip = Some(0);
        state.selected_curve = Some(0);
        state
    }
}

pub fn show_spline_anim_editor(ui: &mut egui::Ui, state: &mut SplineAnimEditorState) {
    ui.heading("Spline Animation Editor");

    ui.horizontal(|ui| {
        if ui.button(if state.is_playing { "⏸" } else { "▶" }).clicked() {
            state.is_playing = !state.is_playing;
        }
        if ui.button("⏹").clicked() {
            state.is_playing = false;
            state.current_time = 0.0;
        }
        ui.add(egui::Slider::new(&mut state.play_speed, 0.1..=5.0).text("Speed"));
        if let Some(clip_idx) = state.selected_clip {
            if let Some(clip) = state.clips.get(clip_idx) {
                ui.add(egui::Slider::new(&mut state.current_time, 0.0..=clip.duration).text("Time"));
            }
        }
        if state.is_playing {
            if let Some(clip_idx) = state.selected_clip {
                if let Some(clip) = state.clips.get(clip_idx) {
                    state.current_time += ui.input(|i| i.predicted_dt) * state.play_speed;
                    if state.current_time > clip.duration { state.current_time = 0.0; }
                }
            }
            ui.ctx().request_repaint();
        }
    });

    egui::SidePanel::left("anim_clips").show_inside(ui, |ui| {
        ui.heading("Clips");
        for (i, clip) in state.clips.iter().enumerate() {
            if ui.selectable_label(state.selected_clip == Some(i),
                format!("{} ({:.1}s)", clip.name, clip.duration)).clicked() {
                state.selected_clip = Some(i);
                state.selected_curve = None;
            }
        }
        if ui.button("+ Add Clip").clicked() {
            let n = state.clips.len() + 1;
            state.clips.push(AnimClip::new(format!("Clip{}", n), 1.0));
        }

        if let Some(clip_idx) = state.selected_clip {
            if let Some(clip) = state.clips.get_mut(clip_idx) {
                ui.separator();
                ui.heading("Curves");
                for (i, curve) in clip.curves.iter().enumerate() {
                    if ui.selectable_label(state.selected_curve == Some(i), &curve.name).clicked() {
                        state.selected_curve = Some(i);
                    }
                }
                if ui.button("+ Add Curve").clicked() {
                    let n = clip.curves.len() + 1;
                    clip.curves.push(AnimCurve::new(format!("curve{}", n)));
                }
            }
        }
    });

    // Curve editor panel
    if let (Some(clip_idx), Some(curve_idx)) = (state.selected_clip, state.selected_curve) {
        if let Some(clip) = state.clips.get_mut(clip_idx) {
            if let Some(curve) = clip.curves.get_mut(curve_idx) {
                let rect = ui.available_rect_before_wrap();
                let painter = ui.painter_at(rect);

                if state.show_grid {
                    for i in 0..=10 {
                        let x = rect.min.x + rect.width() * i as f32 / 10.0;
                        painter.line_segment([egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                            egui::Stroke::new(0.5, egui::Color32::from_gray(50)));
                        let y = rect.min.y + rect.height() * i as f32 / 10.0;
                        painter.line_segment([egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                            egui::Stroke::new(0.5, egui::Color32::from_gray(50)));
                    }
                }

                // Draw curve
                let steps = 100;
                let mut pts = vec![];
                for step in 0..=steps {
                    let t = step as f32 / steps as f32 * curve.duration;
                    let v = curve.evaluate(t);
                    let x = rect.min.x + rect.width() * t / curve.duration.max(0.001);
                    let y = rect.max.y - rect.height() * ((v - curve.min_value) / (curve.max_value - curve.min_value).max(0.001));
                    pts.push(egui::pos2(x, y));
                }
                for w in pts.windows(2) {
                    painter.line_segment([w[0], w[1]], egui::Stroke::new(2.0, curve.color));
                }

                // Draw keyframes
                for (i, kf) in curve.keyframes.iter().enumerate() {
                    let x = rect.min.x + rect.width() * kf.time / curve.duration.max(0.001);
                    let y = rect.max.y - rect.height() * ((kf.value - curve.min_value) / (curve.max_value - curve.min_value).max(0.001));
                    let is_sel = state.selected_keyframe == Some(i);
                    painter.circle_filled(egui::pos2(x, y), if is_sel { 7.0 } else { 5.0 },
                        if is_sel { egui::Color32::WHITE } else { curve.color });
                }

                // Draw playhead
                let ph_x = rect.min.x + rect.width() * state.current_time / curve.duration.max(0.001);
                painter.line_segment(
                    [egui::pos2(ph_x, rect.min.y), egui::pos2(ph_x, rect.max.y)],
                    egui::Stroke::new(1.5, egui::Color32::RED),
                );

                ui.allocate_rect(rect, egui::Sense::click());
            }
        }
    }
}

// ============================================================
// EXPANSION 4: Spline Tests
// ============================================================

#[cfg(test)]
mod spline_expansion4_tests {
    use super::*;

    #[test]
    fn test_physics_body_step() {
        let mut body = SplinePhysicsBody::new(0, 1.0);
        body.apply_force(0.0, 9.81);
        body.step(1.0 / 60.0);
        assert!(body.velocity.1 > 0.0);
    }

    #[test]
    fn test_spring_forces() {
        let spring = SplineSpring::new(0, 1, 100.0, 10.0);
        let positions = vec![(0.0f32, 0.0f32), (200.0f32, 0.0f32)];
        let velocities = vec![(0.0f32, 0.0f32), (0.0f32, 0.0f32)];
        let (fa, fb) = spring.compute_forces(&positions, &velocities);
        // Spring stretched 100 units, force should pull node_a toward node_b
        assert!(fa.0 > 0.0);
        assert!(fb.0 < 0.0);
    }

    #[test]
    fn test_anim_interpolation() {
        let interp = AnimationInterpolation::EaseInOut;
        assert!((interp.evaluate(0.0) - 0.0).abs() < 0.001);
        assert!((interp.evaluate(1.0) - 1.0).abs() < 0.001);
        let mid = interp.evaluate(0.5);
        assert!(mid > 0.4 && mid < 0.6);
    }

    #[test]
    fn test_anim_curve_evaluate() {
        let mut curve = AnimCurve::new("test".to_string());
        curve.keyframes.clear();
        curve.add_keyframe(0.0, 0.0);
        curve.add_keyframe(1.0, 10.0);
        let v = curve.evaluate(0.5);
        assert!((v - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_mesh_generation() {
        let mut spline = Spline::new();
        for i in 0..5 {
            let nd = SplineNode {
                position: egui::pos2(i as f32 * 20.0, 0.0),
                ..SplineNode::default()
            };
            spline.nodes.push(nd);
        }
        let mesh = generate_ribbon_mesh(&spline, 10.0, 32);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_anim_clip_evaluate_all() {
        let mut clip = AnimClip::new("test".to_string(), 1.0);
        clip.curves.push(AnimCurve::new("x".to_string()));
        let vals = clip.evaluate_all(0.5);
        assert_eq!(vals.len(), 1);
    }

    #[test]
    fn test_height_colormap() {
        let cm = HeightColormap::Terrain;
        let _ = cm.sample(0.0);
        let _ = cm.sample(0.5);
        let _ = cm.sample(1.0);
    }
}
'''

with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'a', encoding='utf-8') as f:
    f.write(code)

import os
size = os.path.getsize(r'C:\proof-engine\editor\src\spline_editor.rs')
print(f"spline_editor.rs size: {size} bytes")
