code = r'''

// =================================================================
// SPLINE EXPANSION: TERRAIN DEFORMATION TOOLS
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TerrainBrushMode {
    Raise,
    Lower,
    Smooth,
    Flatten,
    Noise,
    Stamp,
    Paint,
    Erosion,
}

impl TerrainBrushMode {
    pub fn name(&self) -> &'static str {
        match self {
            TerrainBrushMode::Raise => "Raise",
            TerrainBrushMode::Lower => "Lower",
            TerrainBrushMode::Smooth => "Smooth",
            TerrainBrushMode::Flatten => "Flatten",
            TerrainBrushMode::Noise => "Noise",
            TerrainBrushMode::Stamp => "Stamp",
            TerrainBrushMode::Paint => "Paint",
            TerrainBrushMode::Erosion => "Erosion",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainBrush {
    pub mode: TerrainBrushMode,
    pub radius: f32,
    pub strength: f32,
    pub hardness: f32,
    pub falloff: BrushFalloff,
    pub noise_frequency: f32,
    pub noise_amplitude: f32,
    pub flatten_height: f32,
    pub stamp_shape: StampShape,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum BrushFalloff {
    Linear,
    Smooth,
    Constant,
    Spike,
    Gaussian,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum StampShape {
    Circle,
    Square,
    Diamond,
    Star,
    Custom,
}

impl Default for TerrainBrush {
    fn default() -> Self {
        Self {
            mode: TerrainBrushMode::Raise,
            radius: 10.0,
            strength: 0.5,
            hardness: 0.7,
            falloff: BrushFalloff::Smooth,
            noise_frequency: 0.1,
            noise_amplitude: 0.3,
            flatten_height: 0.5,
            stamp_shape: StampShape::Circle,
        }
    }
}

impl TerrainBrush {
    pub fn falloff_weight(&self, dist: f32, radius: f32) -> f32 {
        if dist >= radius { return 0.0; }
        let t = 1.0 - dist / radius;
        match self.falloff {
            BrushFalloff::Constant => 1.0,
            BrushFalloff::Linear => t,
            BrushFalloff::Smooth => t * t * (3.0 - 2.0 * t),
            BrushFalloff::Spike => t * t,
            BrushFalloff::Gaussian => {
                let sigma = radius * 0.4;
                (-dist * dist / (2.0 * sigma * sigma)).exp()
            }
        }
    }

    pub fn apply_to_heightmap(&self, heightmap: &mut Vec<f32>, w: usize, h: usize, cx: f32, cy: f32, rng: &mut SplineRng) {
        let r = self.radius as i32 + 1;
        let ix = cx as i32;
        let iy = cy as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let nx = ix + dx;
                let ny = iy + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
                let dist = ((dx*dx + dy*dy) as f32).sqrt();
                let weight = self.falloff_weight(dist, self.radius) * self.strength;
                if weight <= 0.0 { continue; }
                let idx = ny as usize * w + nx as usize;
                match self.mode {
                    TerrainBrushMode::Raise => heightmap[idx] = (heightmap[idx] + weight * 0.01).min(1.0),
                    TerrainBrushMode::Lower => heightmap[idx] = (heightmap[idx] - weight * 0.01).max(0.0),
                    TerrainBrushMode::Flatten => heightmap[idx] += (self.flatten_height - heightmap[idx]) * weight * 0.1,
                    TerrainBrushMode::Smooth => {
                        let mut sum = 0.0f32; let mut cnt = 0u32;
                        for sy in -1i32..=1 { for sx in -1i32..=1 {
                            let snx = nx + sx; let sny = ny + sy;
                            if snx >= 0 && sny >= 0 && snx < w as i32 && sny < h as i32 {
                                sum += heightmap[sny as usize * w + snx as usize]; cnt += 1;
                            }
                        }}
                        if cnt > 0 { heightmap[idx] += (sum / cnt as f32 - heightmap[idx]) * weight * 0.3; }
                    }
                    TerrainBrushMode::Noise => {
                        let noise = rng.next_f32_range(-1.0, 1.0) * self.noise_amplitude;
                        heightmap[idx] = (heightmap[idx] + noise * weight).clamp(0.0, 1.0);
                    }
                    _ => {}
                }
            }
        }
    }
}

pub fn show_terrain_brush_editor(ui: &mut egui::Ui, brush: &mut TerrainBrush) {
    ui.collapsing("Terrain Brush", |ui| {
        ui.horizontal(|ui| {
            for mode in [TerrainBrushMode::Raise, TerrainBrushMode::Lower, TerrainBrushMode::Smooth, TerrainBrushMode::Flatten, TerrainBrushMode::Noise] {
                if ui.selectable_label(brush.mode == mode, mode.name()).clicked() { brush.mode = mode; }
            }
        });
        ui.add(egui::Slider::new(&mut brush.radius, 1.0..=100.0).text("Radius"));
        ui.add(egui::Slider::new(&mut brush.strength, 0.0..=1.0).text("Strength"));
        ui.add(egui::Slider::new(&mut brush.hardness, 0.0..=1.0).text("Hardness"));
        ui.horizontal(|ui| {
            ui.label("Falloff:");
            for (fo, label) in [
                (BrushFalloff::Smooth, "Smooth"),
                (BrushFalloff::Linear, "Linear"),
                (BrushFalloff::Constant, "Flat"),
                (BrushFalloff::Gaussian, "Gaussian"),
            ] {
                if ui.selectable_label(brush.falloff == fo, label).clicked() { brush.falloff = fo; }
            }
        });
        if brush.mode == TerrainBrushMode::Noise {
            ui.add(egui::Slider::new(&mut brush.noise_frequency, 0.01..=1.0).text("Noise Freq"));
            ui.add(egui::Slider::new(&mut brush.noise_amplitude, 0.0..=1.0).text("Noise Amp"));
        }
        if brush.mode == TerrainBrushMode::Flatten {
            ui.add(egui::Slider::new(&mut brush.flatten_height, 0.0..=1.0).text("Flatten Height"));
        }
    });
}

// =================================================================
// SPLINE EXPANSION: CURVE FITTING & INTERPOLATION
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum InterpolationMode {
    Linear,
    CatmullRom,
    BezierCubic,
    BSpline,
    Hermite,
    Monotone,
}

impl InterpolationMode {
    pub fn name(&self) -> &'static str {
        match self {
            InterpolationMode::Linear => "Linear",
            InterpolationMode::CatmullRom => "Catmull-Rom",
            InterpolationMode::BezierCubic => "Bezier Cubic",
            InterpolationMode::BSpline => "B-Spline",
            InterpolationMode::Hermite => "Hermite",
            InterpolationMode::Monotone => "Monotone",
        }
    }
}

pub fn interpolate_catmull_rom(p0: [f32;2], p1: [f32;2], p2: [f32;2], p3: [f32;2], t: f32) -> [f32;2] {
    let t2 = t * t;
    let t3 = t2 * t;
    let m = 0.5f32;
    [
        m * ((2.0*p1[0]) + (-p0[0]+p2[0])*t + (2.0*p0[0]-5.0*p1[0]+4.0*p2[0]-p3[0])*t2 + (-p0[0]+3.0*p1[0]-3.0*p2[0]+p3[0])*t3),
        m * ((2.0*p1[1]) + (-p0[1]+p2[1])*t + (2.0*p0[1]-5.0*p1[1]+4.0*p2[1]-p3[1])*t2 + (-p0[1]+3.0*p1[1]-3.0*p2[1]+p3[1])*t3),
    ]
}

pub fn interpolate_cubic_bezier(p0: [f32;2], p1: [f32;2], p2: [f32;2], p3: [f32;2], t: f32) -> [f32;2] {
    let u = 1.0 - t;
    let tt = t*t; let uu = u*u;
    let ttt = tt*t; let uuu = uu*u;
    [
        uuu*p0[0] + 3.0*uu*t*p1[0] + 3.0*u*tt*p2[0] + ttt*p3[0],
        uuu*p0[1] + 3.0*uu*t*p1[1] + 3.0*u*tt*p2[1] + ttt*p3[1],
    ]
}

pub fn interpolate_hermite(p0: [f32;2], m0: [f32;2], p1: [f32;2], m1: [f32;2], t: f32) -> [f32;2] {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0*t3 - 3.0*t2 + 1.0;
    let h10 = t3 - 2.0*t2 + t;
    let h01 = -2.0*t3 + 3.0*t2;
    let h11 = t3 - t2;
    [
        h00*p0[0] + h10*m0[0] + h01*p1[0] + h11*m1[0],
        h00*p0[1] + h10*m0[1] + h01*p1[1] + h11*m1[1],
    ]
}

pub fn fit_spline_to_points(points: &[[f32;2]], mode: InterpolationMode, resolution: usize) -> Vec<[f32;2]> {
    if points.len() < 2 { return points.to_vec(); }
    let mut result = Vec::new();
    match mode {
        InterpolationMode::Linear => {
            for i in 0..points.len()-1 {
                for j in 0..resolution {
                    let t = j as f32 / resolution as f32;
                    result.push([
                        points[i][0] + (points[i+1][0] - points[i][0]) * t,
                        points[i][1] + (points[i+1][1] - points[i][1]) * t,
                    ]);
                }
            }
            result.push(*points.last().unwrap());
        }
        InterpolationMode::CatmullRom => {
            let n = points.len();
            for i in 0..n-1 {
                let p0 = if i > 0 { points[i-1] } else { [2.0*points[0][0]-points[1][0], 2.0*points[0][1]-points[1][1]] };
                let p1 = points[i];
                let p2 = points[i+1];
                let p3 = if i+2 < n { points[i+2] } else { [2.0*points[n-1][0]-points[n-2][0], 2.0*points[n-1][1]-points[n-2][1]] };
                for j in 0..resolution {
                    let t = j as f32 / resolution as f32;
                    result.push(interpolate_catmull_rom(p0, p1, p2, p3, t));
                }
            }
            result.push(*points.last().unwrap());
        }
        InterpolationMode::BezierCubic => {
            let n = points.len();
            let segments = (n - 1).max(1);
            for i in 0..segments {
                let p0 = points[i];
                let p3 = points[(i+1).min(n-1)];
                let cp1 = [p0[0] + (p3[0]-p0[0])/3.0, p0[1] + (p3[1]-p0[1])/3.0];
                let cp2 = [p0[0] + 2.0*(p3[0]-p0[0])/3.0, p0[1] + 2.0*(p3[1]-p0[1])/3.0];
                for j in 0..resolution {
                    let t = j as f32 / resolution as f32;
                    result.push(interpolate_cubic_bezier(p0, cp1, cp2, p3, t));
                }
            }
            result.push(*points.last().unwrap());
        }
        _ => {
            return fit_spline_to_points(points, InterpolationMode::CatmullRom, resolution);
        }
    }
    result
}

pub fn compute_spline_arc_length_points(points: &[[f32;2]]) -> f32 {
    let mut length = 0.0f32;
    for i in 1..points.len() {
        let dx = points[i][0] - points[i-1][0];
        let dy = points[i][1] - points[i-1][1];
        length += (dx*dx + dy*dy).sqrt();
    }
    length
}

pub fn resample_spline_uniform(points: &[[f32;2]], target_count: usize) -> Vec<[f32;2]> {
    if points.len() < 2 || target_count < 2 { return points.to_vec(); }
    let total_len = compute_spline_arc_length_points(points);
    let step = total_len / (target_count - 1) as f32;
    let mut result = vec![points[0]];
    let mut dist_so_far = 0.0f32;
    let mut target_dist = step;
    let mut prev = points[0];
    for i in 1..points.len() {
        let seg_len = {
            let dx = points[i][0] - prev[0];
            let dy = points[i][1] - prev[1];
            (dx*dx + dy*dy).sqrt()
        };
        while target_dist <= dist_so_far + seg_len && result.len() < target_count {
            let t = (target_dist - dist_so_far) / seg_len.max(0.0001);
            result.push([
                prev[0] + (points[i][0] - prev[0]) * t,
                prev[1] + (points[i][1] - prev[1]) * t,
            ]);
            target_dist += step;
        }
        dist_so_far += seg_len;
        prev = points[i];
    }
    if result.len() < target_count { result.push(*points.last().unwrap()); }
    result.truncate(target_count);
    result
}

// =================================================================
// SPLINE EXPANSION: MOTION PATH EDITOR
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MotionPathBehavior {
    Loop,
    PingPong,
    OneShot,
    Clamped,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotionKeyframe {
    pub time: f32,
    pub position: [f32; 2],
    pub rotation: f32,
    pub scale: f32,
    pub easing: EasingType,
    pub event_trigger: Option<String>,
}

impl MotionKeyframe {
    pub fn new(time: f32, pos: [f32;2]) -> Self {
        Self { time, position: pos, rotation: 0.0, scale: 1.0, easing: EasingType::Linear, event_trigger: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotionPath {
    pub name: String,
    pub keyframes: Vec<MotionKeyframe>,
    pub behavior: MotionPathBehavior,
    pub duration: f32,
    pub current_time: f32,
    pub is_playing: bool,
    pub interpolation: InterpolationMode,
    pub spline_idx: Option<usize>,
}

impl MotionPath {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            keyframes: Vec::new(),
            behavior: MotionPathBehavior::Loop,
            duration: 5.0,
            current_time: 0.0,
            is_playing: false,
            interpolation: InterpolationMode::CatmullRom,
            spline_idx: None,
        }
    }

    pub fn add_keyframe(&mut self, kf: MotionKeyframe) {
        let pos = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(pos, kf);
    }

    pub fn sample(&self, t: f32) -> [f32; 2] {
        if self.keyframes.is_empty() { return [0.0, 0.0]; }
        if self.keyframes.len() == 1 { return self.keyframes[0].position; }
        let n = self.keyframes.len();
        let wrapped_t = match self.behavior {
            MotionPathBehavior::Loop => t % self.duration,
            MotionPathBehavior::PingPong => {
                let t2 = t % (self.duration * 2.0);
                if t2 > self.duration { self.duration * 2.0 - t2 } else { t2 }
            }
            MotionPathBehavior::OneShot | MotionPathBehavior::Clamped => t.clamp(0.0, self.duration),
        };
        let normalized = wrapped_t / self.duration;
        let seg = (normalized * (n - 1) as f32) as usize;
        let local_t = normalized * (n - 1) as f32 - seg as f32;
        if seg >= n - 1 { return self.keyframes[n-1].position; }
        let p0 = self.keyframes[seg].position;
        let p1 = self.keyframes[seg+1].position;
        match self.interpolation {
            InterpolationMode::Linear => [
                p0[0] + (p1[0] - p0[0]) * local_t,
                p0[1] + (p1[1] - p0[1]) * local_t,
            ],
            InterpolationMode::CatmullRom => {
                let pa = if seg > 0 { self.keyframes[seg-1].position } else { p0 };
                let pd = if seg + 2 < n { self.keyframes[seg+2].position } else { p1 };
                interpolate_catmull_rom(pa, p0, p1, pd, local_t)
            }
            _ => [p0[0] + (p1[0] - p0[0]) * local_t, p0[1] + (p1[1] - p0[1]) * local_t],
        }
    }

    pub fn sample_rotation(&self, t: f32) -> f32 {
        if self.keyframes.len() < 2 { return 0.0; }
        let normalized = (t / self.duration).clamp(0.0, 1.0);
        let n = self.keyframes.len();
        let seg = ((normalized * (n - 1) as f32) as usize).min(n - 2);
        let local_t = normalized * (n - 1) as f32 - seg as f32;
        self.keyframes[seg].rotation + (self.keyframes[seg+1].rotation - self.keyframes[seg].rotation) * local_t
    }

    pub fn advance(&mut self, dt: f32) {
        if !self.is_playing { return; }
        self.current_time += dt;
        match self.behavior {
            MotionPathBehavior::OneShot => {
                if self.current_time >= self.duration { self.current_time = self.duration; self.is_playing = false; }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MotionPathEditorState {
    pub paths: Vec<MotionPath>,
    pub selected_path: Option<usize>,
    pub selected_keyframe: Option<usize>,
    pub show_tangents: bool,
    pub show_timing: bool,
    pub preview_time: f32,
    pub playback_speed: f32,
}

impl MotionPathEditorState {
    pub fn new() -> Self {
        Self { paths: Vec::new(), selected_path: None, selected_keyframe: None, show_tangents: false, show_timing: true, preview_time: 0.0, playback_speed: 1.0 }
    }
}

pub fn draw_motion_path(painter: &egui::Painter, path: &MotionPath, canvas_rect: egui::Rect, zoom: f32, offset: egui::Vec2) {
    if path.keyframes.len() < 2 { return; }
    let steps = path.keyframes.len() * 20;
    let pts: Vec<egui::Pos2> = (0..=steps).map(|i| {
        let t = i as f32 / steps as f32 * path.duration;
        let pos = path.sample(t);
        egui::Pos2::new(canvas_rect.left() + (pos[0] + offset.x) * zoom, canvas_rect.top() + (pos[1] + offset.y) * zoom)
    }).collect();

    for w2 in pts.windows(2) {
        painter.line_segment([w2[0], w2[1]], egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 200, 255)));
    }

    for kf in &path.keyframes {
        let px = canvas_rect.left() + (kf.position[0] + offset.x) * zoom;
        let py = canvas_rect.top() + (kf.position[1] + offset.y) * zoom;
        painter.circle_filled(egui::Pos2::new(px, py), 5.0, egui::Color32::from_rgb(255, 200, 50));
    }

    // Show current position
    let cur_pos = path.sample(path.current_time);
    let cpx = canvas_rect.left() + (cur_pos[0] + offset.x) * zoom;
    let cpy = canvas_rect.top() + (cur_pos[1] + offset.y) * zoom;
    painter.circle_filled(egui::Pos2::new(cpx, cpy), 8.0, egui::Color32::from_rgb(255, 50, 50));
}

pub fn show_motion_path_editor(ui: &mut egui::Ui, state: &mut MotionPathEditorState) {
    ui.horizontal(|ui| {
        ui.label("Motion Paths");
        if ui.button("+ New Path").clicked() {
            state.paths.push(MotionPath::new(&format!("Path {}", state.paths.len())));
        }
    });

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        let mut sel = state.selected_path;
        for (i, path) in state.paths.iter().enumerate() {
            if ui.selectable_label(sel == Some(i), &path.name).clicked() { sel = Some(i); }
        }
        state.selected_path = sel;
    });

    if let Some(pi) = state.selected_path {
        if let Some(path) = state.paths.get_mut(pi) {
            ui.separator();
            ui.text_edit_singleline(&mut path.name);
            ui.add(egui::Slider::new(&mut path.duration, 0.1..=60.0).text("Duration (s)"));
            ui.add(egui::Slider::new(&mut state.playback_speed, 0.1..=5.0).text("Speed"));
            ui.horizontal(|ui| {
                if ui.button(if path.is_playing { "⏸ Pause" } else { "▶ Play" }).clicked() {
                    path.is_playing = !path.is_playing;
                }
                if ui.button("⏮ Reset").clicked() { path.current_time = 0.0; }
            });
            ui.label(format!("Time: {:.2}/{:.2}", path.current_time, path.duration));

            if ui.button("+ Add Keyframe at 0,0").clicked() {
                path.add_keyframe(MotionKeyframe::new(state.preview_time, [0.0, 0.0]));
            }

            for (i, kf) in path.keyframes.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("KF {}: t={:.2}", i, kf.time));
                    ui.add(egui::DragValue::new(&mut kf.position[0]).speed(0.1).prefix("x:"));
                    ui.add(egui::DragValue::new(&mut kf.position[1]).speed(0.1).prefix("y:"));
                    ui.add(egui::DragValue::new(&mut kf.rotation).speed(0.5).prefix("rot:").suffix("°"));
                });
            }
        }
    }
}

// =================================================================
// SPLINE EXPANSION: SPLINE-BASED MESH EXTRUSION
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtrusionProfile {
    pub name: String,
    pub profile_points: Vec<[f32; 2]>,
    pub closed: bool,
    pub scale_along_path: bool,
    pub scale_start: f32,
    pub scale_end: f32,
    pub twist_degrees: f32,
    pub uv_scale: f32,
}

impl ExtrusionProfile {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            profile_points: vec![[0.0, -0.5], [0.0, 0.5]],
            closed: false,
            scale_along_path: false,
            scale_start: 1.0,
            scale_end: 1.0,
            twist_degrees: 0.0,
            uv_scale: 1.0,
        }
    }

    pub fn circle(radius: f32, segments: u32) -> Self {
        let mut p = Self::new("Circle");
        p.profile_points = (0..segments).map(|i| {
            let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
            [angle.cos() * radius, angle.sin() * radius]
        }).collect();
        p.closed = true;
        p
    }

    pub fn rectangle(w: f32, h: f32) -> Self {
        let mut p = Self::new("Rectangle");
        p.profile_points = vec![[-w/2.0, -h/2.0], [w/2.0, -h/2.0], [w/2.0, h/2.0], [-w/2.0, h/2.0]];
        p.closed = true;
        p
    }

    pub fn road_profile(lane_width: f32, lane_count: u32) -> Self {
        let hw = lane_width * lane_count as f32 / 2.0;
        let mut p = Self::new("Road");
        p.profile_points = vec![[-hw - 1.0, 0.0], [-hw, 0.0], [hw, 0.0], [hw + 1.0, 0.0]];
        p
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtrudedMeshSegment {
    pub path_t: f32,
    pub position: [f32; 3],
    pub tangent: [f32; 3],
    pub normal: [f32; 3],
    pub binormal: [f32; 3],
    pub scale: f32,
    pub twist: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtrusionResult {
    pub vertices: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub face_normals: Vec<[f32; 3]>,
    pub segment_count: usize,
    pub profile_count: usize,
    pub total_length: f32,
}

pub fn extrude_profile_along_path(profile: &ExtrusionProfile, path_points: &[[f32;2]], resolution: usize) -> ExtrusionResult {
    let resampled = resample_spline_uniform(path_points, resolution);
    let mut vertices = Vec::new();
    let mut uvs = Vec::new();
    let mut face_normals = Vec::new();
    let total_len = compute_spline_arc_length_points(&resampled);
    let mut dist_accum = 0.0f32;
    let pc = profile.profile_points.len();

    for (seg_i, point) in resampled.iter().enumerate() {
        let t = seg_i as f32 / resolution.max(1) as f32;
        let scale = if profile.scale_along_path {
            profile.scale_start + (profile.scale_end - profile.scale_start) * t
        } else { 1.0 };
        let twist = profile.twist_degrees * t * std::f32::consts::PI / 180.0;

        // Tangent
        let tangent = if seg_i + 1 < resampled.len() {
            let dx = resampled[seg_i+1][0] - point[0];
            let dy = resampled[seg_i+1][1] - point[1];
            let len = (dx*dx + dy*dy).sqrt().max(0.001);
            [dx/len, dy/len]
        } else if seg_i > 0 {
            let dx = point[0] - resampled[seg_i-1][0];
            let dy = point[1] - resampled[seg_i-1][1];
            let len = (dx*dx + dy*dy).sqrt().max(0.001);
            [dx/len, dy/len]
        } else { [1.0, 0.0] };

        // Normal (perpendicular to tangent in 2D, up in 3D)
        let normal = [-tangent[1], tangent[0]];

        for (pi, pp) in profile.profile_points.iter().enumerate() {
            let cos_t = twist.cos();
            let sin_t = twist.sin();
            let rx = pp[0] * cos_t - pp[1] * sin_t;
            let ry = pp[0] * sin_t + pp[1] * cos_t;
            let vx = point[0] + normal[0] * rx * scale - tangent[1] * ry * scale;
            let vy = point[1] + normal[1] * rx * scale + tangent[0] * ry * scale;
            vertices.push([vx, vy, pp[1] * scale]);
            uvs.push([pi as f32 / pc as f32 * profile.uv_scale, dist_accum / total_len.max(0.001)]);
        }

        if seg_i > 0 {
            let dx = point[0] - resampled[seg_i-1][0];
            let dy = point[1] - resampled[seg_i-1][1];
            dist_accum += (dx*dx + dy*dy).sqrt();
        }
    }

    ExtrusionResult {
        vertices,
        uvs,
        face_normals,
        segment_count: resampled.len(),
        profile_count: pc,
        total_length: total_len,
    }
}

pub fn show_extrusion_editor(ui: &mut egui::Ui, profiles: &mut Vec<ExtrusionProfile>, selected: &mut Option<usize>) {
    ui.heading("Mesh Extrusion Editor");
    ui.horizontal(|ui| {
        if ui.button("+ Circle Profile").clicked() { profiles.push(ExtrusionProfile::circle(1.0, 8)); }
        if ui.button("+ Rect Profile").clicked() { profiles.push(ExtrusionProfile::rectangle(2.0, 1.0)); }
        if ui.button("+ Road Profile").clicked() { profiles.push(ExtrusionProfile::road_profile(3.0, 2)); }
    });

    for (i, profile) in profiles.iter().enumerate() {
        if ui.selectable_label(*selected == Some(i), &profile.name).clicked() { *selected = Some(i); }
    }

    if let Some(idx) = *selected {
        if let Some(profile) = profiles.get_mut(idx) {
            ui.separator();
            ui.text_edit_singleline(&mut profile.name);
            ui.checkbox(&mut profile.closed, "Closed Profile");
            ui.checkbox(&mut profile.scale_along_path, "Scale Along Path");
            if profile.scale_along_path {
                ui.add(egui::Slider::new(&mut profile.scale_start, 0.01..=5.0).text("Start Scale"));
                ui.add(egui::Slider::new(&mut profile.scale_end, 0.01..=5.0).text("End Scale"));
            }
            ui.add(egui::Slider::new(&mut profile.twist_degrees, -360.0..=360.0).text("Twist °"));
            ui.add(egui::Slider::new(&mut profile.uv_scale, 0.1..=10.0).text("UV Scale"));
            ui.label(format!("Profile points: {}", profile.profile_points.len()));
        }
    }
}

// =================================================================
// SPLINE EXPANSION: PROCEDURAL ROAD GENERATION
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProceduralRoadConfig {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub num_control_points: u32,
    pub max_deviation: f32,
    pub follow_terrain: bool,
    pub avoid_water: bool,
    pub prefer_flat: bool,
    pub seed: u64,
    pub smoothing_iterations: u32,
    pub road_config: RoadConfig,
}

impl Default for ProceduralRoadConfig {
    fn default() -> Self {
        Self {
            start: [0.1, 0.5],
            end: [0.9, 0.5],
            num_control_points: 5,
            max_deviation: 0.15,
            follow_terrain: true,
            avoid_water: true,
            prefer_flat: true,
            seed: 42,
            smoothing_iterations: 3,
            road_config: RoadConfig::default(),
        }
    }
}

pub fn generate_procedural_road(cfg: &ProceduralRoadConfig, heightmap: Option<&[f32]>, ocean_map: Option<&[bool]>, map_w: usize, map_h: usize) -> Vec<[f32;2]> {
    let mut rng = SplineRng::new(cfg.seed);
    let mut points = vec![cfg.start];

    // Generate intermediate control points
    let dx = cfg.end[0] - cfg.start[0];
    let dy = cfg.end[1] - cfg.start[1];
    for i in 1..=cfg.num_control_points {
        let t = i as f32 / (cfg.num_control_points + 1) as f32;
        let base_x = cfg.start[0] + dx * t;
        let base_y = cfg.start[1] + dy * t;
        let perp_x = -dy;
        let perp_y = dx;
        let deviate = rng.next_f32_range(-cfg.max_deviation, cfg.max_deviation);
        let mut px = base_x + perp_x * deviate;
        let mut py = base_y + perp_y * deviate;

        // Avoid water if heightmap is available
        if cfg.avoid_water {
            if let (Some(hm), Some(ocean)) = (heightmap, ocean_map) {
                let hx = (px * map_w as f32) as usize;
                let hy = (py * map_h as f32) as usize;
                if hx < map_w && hy < map_h && ocean[hy * map_w + hx] {
                    // Nudge away from water
                    px += rng.next_f32_range(-0.05, 0.05);
                    py += rng.next_f32_range(-0.05, 0.05);
                    px = px.clamp(0.0, 1.0);
                    py = py.clamp(0.0, 1.0);
                }
            }
        }

        points.push([px.clamp(0.0, 1.0), py.clamp(0.0, 1.0)]);
    }
    points.push(cfg.end);

    // Smooth the control points
    for _ in 0..cfg.smoothing_iterations {
        let n = points.len();
        for i in 1..n-1 {
            points[i][0] = (points[i-1][0] + points[i][0] * 2.0 + points[i+1][0]) / 4.0;
            points[i][1] = (points[i-1][1] + points[i][1] * 2.0 + points[i+1][1]) / 4.0;
        }
    }

    fit_spline_to_points(&points, InterpolationMode::CatmullRom, 20)
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProceduralRoadEditorState {
    pub config: ProceduralRoadConfig,
    pub generated_points: Vec<[f32;2]>,
    pub show_control_points: bool,
    pub show_road_preview: bool,
    pub extrusion_profiles: Vec<ExtrusionProfile>,
    pub selected_profile: Option<usize>,
}

pub fn show_procedural_road_editor(ui: &mut egui::Ui, state: &mut ProceduralRoadEditorState) {
    ui.heading("Procedural Road Generator");
    ui.add(egui::DragValue::new(&mut state.config.start[0]).speed(0.001).prefix("Start X:"));
    ui.add(egui::DragValue::new(&mut state.config.start[1]).speed(0.001).prefix("Start Y:"));
    ui.add(egui::DragValue::new(&mut state.config.end[0]).speed(0.001).prefix("End X:"));
    ui.add(egui::DragValue::new(&mut state.config.end[1]).speed(0.001).prefix("End Y:"));
    ui.add(egui::Slider::new(&mut state.config.num_control_points, 1..=10).text("Control Points"));
    ui.add(egui::Slider::new(&mut state.config.max_deviation, 0.0..=0.5).text("Max Deviation"));
    ui.add(egui::Slider::new(&mut state.config.smoothing_iterations, 0..=10).text("Smoothing"));
    ui.add(egui::DragValue::new(&mut state.config.seed).prefix("Seed: "));
    ui.checkbox(&mut state.config.follow_terrain, "Follow Terrain");
    ui.checkbox(&mut state.config.avoid_water, "Avoid Water");
    ui.checkbox(&mut state.config.prefer_flat, "Prefer Flat");

    if ui.button("Generate Road").clicked() {
        state.generated_points = generate_procedural_road(&state.config, None, None, 256, 256);
    }
    ui.label(format!("Generated points: {}", state.generated_points.len()));
    ui.checkbox(&mut state.show_control_points, "Show Control Points");
    ui.checkbox(&mut state.show_road_preview, "Show Road Preview");
}

// =================================================================
// SPLINE EXPANSION: SPLINE CONSTRAINT SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ConstraintType {
    FixedPoint,
    TangentAlign,
    EqualSpacing,
    SurfaceAttach,
    AxisAlign,
    Symmetry,
    Length,
    Angle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineConstraint {
    pub id: u32,
    pub constraint_type: ConstraintType,
    pub node_indices: Vec<usize>,
    pub enabled: bool,
    pub strength: f32,
    pub target_value: f32,
    pub target_point: [f32; 2],
    pub axis: [f32; 2],
}

impl SplineConstraint {
    pub fn fixed_point(id: u32, node_idx: usize, pos: [f32;2]) -> Self {
        Self { id, constraint_type: ConstraintType::FixedPoint, node_indices: vec![node_idx], enabled: true, strength: 1.0, target_value: 0.0, target_point: pos, axis: [1.0, 0.0] }
    }

    pub fn equal_spacing(id: u32, nodes: Vec<usize>) -> Self {
        Self { id, constraint_type: ConstraintType::EqualSpacing, node_indices: nodes, enabled: true, strength: 1.0, target_value: 0.0, target_point: [0.0;2], axis: [1.0, 0.0] }
    }

    pub fn axis_align(id: u32, node_idx: usize, axis: [f32;2]) -> Self {
        Self { id, constraint_type: ConstraintType::AxisAlign, node_indices: vec![node_idx], enabled: true, strength: 1.0, target_value: 0.0, target_point: [0.0;2], axis }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ConstraintEditorState {
    pub constraints: Vec<SplineConstraint>,
    pub selected: Option<usize>,
    pub next_id: u32,
    pub show_constraints: bool,
}

pub fn show_constraint_editor(ui: &mut egui::Ui, state: &mut ConstraintEditorState) {
    ui.collapsing("Spline Constraints", |ui| {
        ui.checkbox(&mut state.show_constraints, "Show Constraints");
        ui.horizontal(|ui| {
            if ui.button("+ Fixed Point").clicked() {
                state.constraints.push(SplineConstraint::fixed_point(state.next_id, 0, [0.0, 0.0]));
                state.next_id += 1;
            }
            if ui.button("+ Equal Spacing").clicked() {
                state.constraints.push(SplineConstraint::equal_spacing(state.next_id, vec![0, 1, 2]));
                state.next_id += 1;
            }
            if ui.button("+ Axis Align").clicked() {
                state.constraints.push(SplineConstraint::axis_align(state.next_id, 0, [1.0, 0.0]));
                state.next_id += 1;
            }
        });

        let mut to_remove = None;
        for (i, c) in state.constraints.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut c.enabled, "");
                ui.label(format!("{:?} (nodes: {:?})", c.constraint_type, c.node_indices));
                ui.add(egui::Slider::new(&mut c.strength, 0.0..=1.0).text("Str"));
                if ui.button("🗑").clicked() { to_remove = Some(i); }
            });
        }
        if let Some(i) = to_remove { state.constraints.remove(i); }
    });
}

// =================================================================
// SPLINE EXPANSION: SPLINE NETWORK ANALYSIS
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineNetworkStats {
    pub total_splines: usize,
    pub total_nodes: usize,
    pub total_arc_length: f32,
    pub min_arc_length: f32,
    pub max_arc_length: f32,
    pub avg_arc_length: f32,
    pub intersection_count: usize,
    pub isolated_nodes: usize,
    pub max_curvature: f32,
    pub avg_curvature: f32,
}

impl SplineNetworkStats {
    pub fn compute_for_spline_nodes(nodes: &[[f32;2]]) -> Self {
        let total = compute_spline_arc_length_points(nodes);
        Self {
            total_splines: 1,
            total_nodes: nodes.len(),
            total_arc_length: total,
            min_arc_length: total,
            max_arc_length: total,
            avg_arc_length: total,
            intersection_count: 0,
            isolated_nodes: 0,
            max_curvature: 0.0,
            avg_curvature: 0.0,
        }
    }
}

pub fn compute_local_curvature(p0: [f32;2], p1: [f32;2], p2: [f32;2]) -> f32 {
    let a = ((p1[0]-p0[0]).powi(2) + (p1[1]-p0[1]).powi(2)).sqrt();
    let b = ((p2[0]-p1[0]).powi(2) + (p2[1]-p1[1]).powi(2)).sqrt();
    let c = ((p2[0]-p0[0]).powi(2) + (p2[1]-p0[1]).powi(2)).sqrt();
    let area = ((p1[0]-p0[0])*(p2[1]-p0[1]) - (p2[0]-p0[0])*(p1[1]-p0[1])).abs() * 0.5;
    let denom = (a * b * c).max(0.0001);
    2.0 * area / denom
}

pub fn find_spline_self_intersections(points: &[[f32;2]], tolerance: f32) -> Vec<[f32;2]> {
    let mut intersections = Vec::new();
    let n = points.len();
    for i in 0..n {
        for j in i+2..n {
            if j == n-1 && i == 0 { continue; }
            let p1 = points[i];
            let p2 = if i+1 < n { points[i+1] } else { continue };
            let p3 = points[j];
            let p4 = if j+1 < n { points[j+1] } else { continue };
            if let Some(inter) = segment_intersect(p1, p2, p3, p4) {
                intersections.push(inter);
            }
        }
    }
    intersections
}

pub fn segment_intersect(p1: [f32;2], p2: [f32;2], p3: [f32;2], p4: [f32;2]) -> Option<[f32;2]> {
    let d1 = [p2[0]-p1[0], p2[1]-p1[1]];
    let d2 = [p4[0]-p3[0], p4[1]-p3[1]];
    let cross = d1[0]*d2[1] - d1[1]*d2[0];
    if cross.abs() < 0.0001 { return None; }
    let t = ((p3[0]-p1[0])*d2[1] - (p3[1]-p1[1])*d2[0]) / cross;
    let u = ((p3[0]-p1[0])*d1[1] - (p3[1]-p1[1])*d1[0]) / cross;
    if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
        Some([p1[0] + t*d1[0], p1[1] + t*d1[1]])
    } else { None }
}

// =================================================================
// SPLINE EXPANSION: SPLINE IMPORT/EXPORT
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineExportConfig {
    pub format: SplineExportFormat,
    pub resolution: usize,
    pub include_normals: bool,
    pub include_tangents: bool,
    pub coordinate_scale: f32,
    pub flip_y: bool,
    pub close_open_splines: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SplineExportFormat {
    Csv,
    Json,
    Svg,
    ObjPath,
    DxfPolyline,
}

impl Default for SplineExportConfig {
    fn default() -> Self {
        Self { format: SplineExportFormat::Json, resolution: 100, include_normals: false, include_tangents: false, coordinate_scale: 1.0, flip_y: false, close_open_splines: false }
    }
}

pub fn export_spline_to_csv(points: &[[f32;2]], tangents: Option<&[[f32;2]]>, scale: f32, flip_y: bool) -> String {
    let mut output = String::from("x,y");
    if tangents.is_some() { output.push_str(",tx,ty"); }
    output.push('\n');
    for (i, pt) in points.iter().enumerate() {
        let y = if flip_y { -pt[1] } else { pt[1] };
        if let Some(tans) = tangents {
            output.push_str(&format!("{},{},{},{}\n", pt[0]*scale, y*scale, tans[i][0], tans[i][1]));
        } else {
            output.push_str(&format!("{},{}\n", pt[0]*scale, y*scale));
        }
    }
    output
}

pub fn export_spline_to_svg_path(points: &[[f32;2]], closed: bool, scale: f32, flip_y: bool) -> String {
    if points.is_empty() { return String::new(); }
    let mut d = String::new();
    let first = points[0];
    let fy = if flip_y { -first[1] } else { first[1] };
    d.push_str(&format!("M {:.3} {:.3}", first[0]*scale, fy*scale));
    for pt in points.iter().skip(1) {
        let y = if flip_y { -pt[1] } else { pt[1] };
        d.push_str(&format!(" L {:.3} {:.3}", pt[0]*scale, y*scale));
    }
    if closed { d.push_str(" Z"); }
    format!("<path d=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"1\"/>", d)
}

pub fn export_splines_to_json(all_points: &[Vec<[f32;2]>]) -> String {
    let mut json = String::from("{\"splines\":[");
    for (i, pts) in all_points.iter().enumerate() {
        if i > 0 { json.push(','); }
        json.push_str("{\"points\":[");
        for (j, pt) in pts.iter().enumerate() {
            if j > 0 { json.push(','); }
            json.push_str(&format!("[{:.4},{:.4}]", pt[0], pt[1]));
        }
        json.push_str("]}");
    }
    json.push_str("]}");
    json
}

pub fn show_spline_export_panel(ui: &mut egui::Ui, cfg: &mut SplineExportConfig, preview: &mut String) {
    ui.heading("Spline Export");
    ui.horizontal(|ui| {
        for (fmt, label) in [
            (SplineExportFormat::Json, "JSON"),
            (SplineExportFormat::Csv, "CSV"),
            (SplineExportFormat::Svg, "SVG"),
            (SplineExportFormat::ObjPath, "OBJ"),
        ] {
            if ui.selectable_label(cfg.format == fmt, label).clicked() { cfg.format = fmt; }
        }
    });
    ui.add(egui::Slider::new(&mut cfg.resolution, 10..=1000).text("Resolution"));
    ui.add(egui::Slider::new(&mut cfg.coordinate_scale, 0.001..=1000.0).text("Scale"));
    ui.checkbox(&mut cfg.flip_y, "Flip Y");
    ui.checkbox(&mut cfg.include_normals, "Include Normals");
    ui.checkbox(&mut cfg.include_tangents, "Include Tangents");
    ui.checkbox(&mut cfg.close_open_splines, "Close Open Splines");
    if ui.button("Export Preview").clicked() {
        let dummy_pts = vec![[0.0f32,0.0],[0.5,0.3],[1.0,0.0]];
        *preview = match cfg.format {
            SplineExportFormat::Csv => export_spline_to_csv(&dummy_pts, None, cfg.coordinate_scale, cfg.flip_y),
            SplineExportFormat::Svg => export_spline_to_svg_path(&dummy_pts, false, cfg.coordinate_scale, cfg.flip_y),
            SplineExportFormat::Json => export_splines_to_json(&[dummy_pts]),
            _ => String::from("Format not yet implemented"),
        };
    }
    if !preview.is_empty() {
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            ui.code(preview.as_str());
        });
    }
}

// =================================================================
// SPLINE EXPANSION: MULTI-SPLINE OPERATIONS
// =================================================================

pub fn blend_splines(a: &[[f32;2]], b: &[[f32;2]], t: f32, resolution: usize) -> Vec<[f32;2]> {
    let ra = resample_spline_uniform(a, resolution);
    let rb = resample_spline_uniform(b, resolution);
    ra.iter().zip(rb.iter()).map(|(pa, pb)| [
        pa[0] + (pb[0] - pa[0]) * t,
        pa[1] + (pb[1] - pa[1]) * t,
    ]).collect()
}

pub fn offset_spline(points: &[[f32;2]], offset_dist: f32) -> Vec<[f32;2]> {
    let n = points.len();
    if n < 2 { return points.to_vec(); }
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let prev = if i > 0 { points[i-1] } else { points[0] };
        let next = if i + 1 < n { points[i+1] } else { points[n-1] };
        let dx = next[0] - prev[0];
        let dy = next[1] - prev[1];
        let len = (dx*dx + dy*dy).sqrt().max(0.001);
        let nx = -dy / len;
        let ny = dx / len;
        result.push([points[i][0] + nx * offset_dist, points[i][1] + ny * offset_dist]);
    }
    result
}

pub fn reverse_spline(points: &[[f32;2]]) -> Vec<[f32;2]> {
    let mut r = points.to_vec();
    r.reverse();
    r
}

pub fn connect_splines(a: &[[f32;2]], b: &[[f32;2]], blend_steps: usize) -> Vec<[f32;2]> {
    let mut result = a.to_vec();
    if a.is_empty() || b.is_empty() { return result; }
    let end = *a.last().unwrap();
    let start = b[0];
    for i in 1..=blend_steps {
        let t = i as f32 / (blend_steps + 1) as f32;
        result.push([end[0] + (start[0] - end[0]) * t, end[1] + (start[1] - end[1]) * t]);
    }
    result.extend_from_slice(b);
    result
}

pub fn boolean_spline_union(a: &[[f32;2]], b: &[[f32;2]]) -> Vec<[f32;2]> {
    // Simplified: just concatenate
    let mut result = a.to_vec();
    result.extend_from_slice(b);
    result
}

// =================================================================
// SPLINE EXPANSION: EXTENDED EDITOR PANEL (FULL INTEGRATION)
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FullSplineEditorState {
    pub terrain_brush: TerrainBrush,
    pub motion_path_state: MotionPathEditorState,
    pub extrusion_profiles: Vec<ExtrusionProfile>,
    pub selected_extrusion: Option<usize>,
    pub procedural_road_state: ProceduralRoadEditorState,
    pub constraint_state: ConstraintEditorState,
    pub export_config: SplineExportConfig,
    pub export_preview: String,
    pub active_tab: FullSplineTab,
    pub show_self_intersections: bool,
    pub blend_spline_t: f32,
    pub offset_distance: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum FullSplineTab {
    #[default]
    RoadNetwork,
    CameraSequence,
    Railway,
    Animation,
    TerrainBrush,
    MotionPaths,
    Extrusion,
    ProceduralRoad,
    Constraints,
    Analysis,
    Export,
}

pub fn show_full_spline_panel(ui: &mut egui::Ui, state: &mut FullSplineEditorState) {
    ui.horizontal(|ui| {
        for (tab, label) in [
            (FullSplineTab::RoadNetwork, "Roads"),
            (FullSplineTab::CameraSequence, "Camera"),
            (FullSplineTab::Railway, "Railway"),
            (FullSplineTab::Animation, "Anim"),
            (FullSplineTab::TerrainBrush, "Terrain"),
            (FullSplineTab::MotionPaths, "Motion"),
            (FullSplineTab::Extrusion, "Extrude"),
            (FullSplineTab::ProceduralRoad, "Proc Road"),
            (FullSplineTab::Constraints, "Constraints"),
            (FullSplineTab::Analysis, "Analyze"),
            (FullSplineTab::Export, "Export"),
        ] {
            if ui.selectable_label(state.active_tab == tab, label).clicked() { state.active_tab = tab; }
        }
    });
    ui.separator();
    match state.active_tab {
        FullSplineTab::TerrainBrush => { show_terrain_brush_editor(ui, &mut state.terrain_brush); }
        FullSplineTab::MotionPaths => { show_motion_path_editor(ui, &mut state.motion_path_state); }
        FullSplineTab::Extrusion => { show_extrusion_editor(ui, &mut state.extrusion_profiles, &mut state.selected_extrusion); }
        FullSplineTab::ProceduralRoad => { show_procedural_road_editor(ui, &mut state.procedural_road_state); }
        FullSplineTab::Constraints => { show_constraint_editor(ui, &mut state.constraint_state); }
        FullSplineTab::Analysis => {
            ui.heading("Spline Analysis");
            ui.checkbox(&mut state.show_self_intersections, "Show Self-Intersections");
            ui.add(egui::Slider::new(&mut state.offset_distance, -50.0..=50.0).text("Offset Distance"));
            ui.add(egui::Slider::new(&mut state.blend_spline_t, 0.0..=1.0).text("Blend T"));
            ui.label("Select splines in the viewport to analyze them.");
        }
        FullSplineTab::Export => { show_spline_export_panel(ui, &mut state.export_config, &mut state.export_preview); }
        _ => { ui.label("Use this tab for the corresponding spline tools."); }
    }
}

// =================================================================
// SPLINE EXPANSION: TESTS
// =================================================================

#[cfg(test)]
mod spline_expansion_tests {
    use super::*;

    #[test]
    fn test_catmull_rom_interpolation() {
        let p0 = [0.0f32, 0.0];
        let p1 = [1.0, 0.0];
        let p2 = [2.0, 1.0];
        let p3 = [3.0, 0.0];
        let mid = interpolate_catmull_rom(p0, p1, p2, p3, 0.5);
        assert!(mid[0] > 0.0 && mid[0] < 3.0);
    }

    #[test]
    fn test_cubic_bezier_interpolation() {
        let p = interpolate_cubic_bezier([0.0,0.0], [0.33,1.0], [0.67,1.0], [1.0,0.0], 0.5);
        assert!(p[0] > 0.0 && p[0] < 1.0);
    }

    #[test]
    fn test_fit_spline_to_points_linear() {
        let points = vec![[0.0f32,0.0],[1.0,1.0],[2.0,0.0]];
        let fitted = fit_spline_to_points(&points, InterpolationMode::Linear, 10);
        assert!(!fitted.is_empty());
    }

    #[test]
    fn test_arc_length_computation() {
        let pts = vec![[0.0f32,0.0],[3.0,4.0]]; // 5 units
        let len = compute_spline_arc_length_points(&pts);
        assert!((len - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_resample_spline_uniform() {
        let pts = vec![[0.0f32,0.0],[10.0,0.0],[20.0,0.0]];
        let resampled = resample_spline_uniform(&pts, 5);
        assert_eq!(resampled.len(), 5);
    }

    #[test]
    fn test_offset_spline() {
        let pts = vec![[0.0f32,0.0],[1.0,0.0],[2.0,0.0]];
        let offset = offset_spline(&pts, 1.0);
        assert_eq!(offset.len(), pts.len());
    }

    #[test]
    fn test_terrain_brush_falloff() {
        let brush = TerrainBrush::default();
        assert_eq!(brush.falloff_weight(0.0, 10.0), 1.0);
        assert_eq!(brush.falloff_weight(15.0, 10.0), 0.0);
        assert!(brush.falloff_weight(5.0, 10.0) > 0.0);
    }

    #[test]
    fn test_motion_path_sample() {
        let mut path = MotionPath::new("Test");
        path.keyframes.push(MotionKeyframe::new(0.0, [0.0, 0.0]));
        path.keyframes.push(MotionKeyframe::new(5.0, [10.0, 5.0]));
        let mid = path.sample(2.5);
        assert!(mid[0] > 0.0);
    }

    #[test]
    fn test_procedural_road_generation() {
        let cfg = ProceduralRoadConfig::default();
        let pts = generate_procedural_road(&cfg, None, None, 256, 256);
        assert!(!pts.is_empty());
    }

    #[test]
    fn test_spline_export_csv() {
        let pts = vec![[0.0f32,0.0],[1.0,1.0]];
        let csv = export_spline_to_csv(&pts, None, 1.0, false);
        assert!(csv.contains("x,y"));
        assert!(csv.contains("0,0"));
    }

    #[test]
    fn test_spline_export_svg() {
        let pts = vec![[0.0f32,0.0],[1.0,1.0],[2.0,0.0]];
        let svg = export_spline_to_svg_path(&pts, false, 1.0, false);
        assert!(svg.contains("<path"));
    }

    #[test]
    fn test_segment_intersect() {
        let result = segment_intersect([0.0,0.0], [1.0,1.0], [0.0,1.0], [1.0,0.0]);
        assert!(result.is_some());
        let pt = result.unwrap();
        assert!((pt[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_blend_splines() {
        let a = vec![[0.0f32,0.0],[1.0,0.0]];
        let b = vec![[0.0f32,2.0],[1.0,2.0]];
        let blended = blend_splines(&a, &b, 0.5, 2);
        assert!(!blended.is_empty());
        assert!((blended[0][1] - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_extrusion_profile_circle() {
        let p = ExtrusionProfile::circle(1.0, 8);
        assert_eq!(p.profile_points.len(), 8);
        assert!(p.closed);
    }

    #[test]
    fn test_curvature_computation() {
        let curv = compute_local_curvature([0.0,0.0], [1.0,0.0], [2.0,0.0]);
        assert!(curv < 0.001); // Straight line has near-zero curvature
        let curv2 = compute_local_curvature([0.0,0.0], [1.0,1.0], [0.0,2.0]);
        assert!(curv2 > 0.0);
    }
}
'''

with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'a', encoding='utf-8') as f:
    f.write(code)

print(f"Done. spline_editor size: {__import__('os').path.getsize(r'C:\proof-engine\editor\src\spline_editor.rs')} bytes")
