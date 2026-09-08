code = r'''

// =================================================================
// SPLINE EXPANSION 2: BEZIER CONTROL POINT EDITOR
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BezierControlPoint {
    pub position: [f32; 2],
    pub control_in: [f32; 2],
    pub control_out: [f32; 2],
    pub smooth: bool,
    pub weight_in: f32,
    pub weight_out: f32,
    pub id: u32,
}

impl BezierControlPoint {
    pub fn new(id: u32, pos: [f32;2]) -> Self {
        Self {
            position: pos,
            control_in: [pos[0] - 0.1, pos[1]],
            control_out: [pos[0] + 0.1, pos[1]],
            smooth: true,
            weight_in: 1.0,
            weight_out: 1.0,
            id,
        }
    }

    pub fn move_to(&mut self, new_pos: [f32;2]) {
        let dx = new_pos[0] - self.position[0];
        let dy = new_pos[1] - self.position[1];
        self.position = new_pos;
        self.control_in[0] += dx;
        self.control_in[1] += dy;
        self.control_out[0] += dx;
        self.control_out[1] += dy;
    }

    pub fn set_control_out(&mut self, cp: [f32;2]) {
        self.control_out = cp;
        if self.smooth {
            // Mirror the control point through the anchor
            let dx = self.position[0] - cp[0];
            let dy = self.position[1] - cp[1];
            self.control_in = [self.position[0] + dx, self.position[1] + dy];
        }
    }

    pub fn set_control_in(&mut self, cp: [f32;2]) {
        self.control_in = cp;
        if self.smooth {
            let dx = self.position[0] - cp[0];
            let dy = self.position[1] - cp[1];
            self.control_out = [self.position[0] + dx, self.position[1] + dy];
        }
    }

    pub fn control_handle_len(&self) -> f32 {
        let dx = self.control_out[0] - self.position[0];
        let dy = self.control_out[1] - self.position[1];
        (dx*dx + dy*dy).sqrt()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BezierCurve {
    pub name: String,
    pub control_points: Vec<BezierControlPoint>,
    pub closed: bool,
    pub color: egui::Color32,
    pub width: f32,
    pub resolution: u32,
    pub next_id: u32,
}

impl BezierCurve {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), control_points: Vec::new(), closed: false, color: egui::Color32::from_rgb(255, 200, 50), width: 2.0, resolution: 50, next_id: 0 }
    }

    pub fn add_point(&mut self, pos: [f32;2]) {
        let id = self.next_id;
        self.next_id += 1;
        self.control_points.push(BezierControlPoint::new(id, pos));
    }

    pub fn sample(&self, t: f32) -> Option<[f32;2]> {
        let n = self.control_points.len();
        if n < 2 { return None; }
        let segments = if self.closed { n } else { n - 1 };
        let seg = (t * segments as f32) as usize;
        let local_t = t * segments as f32 - seg as f32;
        let seg = seg.min(segments - 1);
        let p0 = &self.control_points[seg];
        let p1 = &self.control_points[(seg + 1) % n];
        let pt = interpolate_cubic_bezier(p0.position, p0.control_out, p1.control_in, p1.position, local_t);
        Some(pt)
    }

    pub fn sample_all(&self) -> Vec<[f32;2]> {
        let steps = self.resolution as usize * (self.control_points.len().max(2) - 1);
        (0..=steps).filter_map(|i| self.sample(i as f32 / steps as f32)).collect()
    }

    pub fn remove_point(&mut self, id: u32) {
        self.control_points.retain(|p| p.id != id);
    }

    pub fn split_at_t(&self, t: f32) -> (BezierCurve, BezierCurve) {
        let mut left = BezierCurve::new(&format!("{}_L", self.name));
        let mut right = BezierCurve::new(&format!("{}_R", self.name));
        let mid = self.sample(t).unwrap_or([0.0, 0.0]);
        for (i, cp) in self.control_points.iter().enumerate() {
            if i as f32 / self.control_points.len() as f32 <= t {
                left.add_point(cp.position);
            } else {
                right.add_point(cp.position);
            }
        }
        left.add_point(mid);
        right.control_points.insert(0, BezierControlPoint::new(99999, mid));
        (left, right)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct BezierEditorState {
    pub curves: Vec<BezierCurve>,
    pub selected_curve: Option<usize>,
    pub selected_point: Option<u32>,
    pub dragging_control_in: bool,
    pub dragging_control_out: bool,
    pub show_control_handles: bool,
    pub show_normals: bool,
    pub show_frame: bool,
    pub sample_count: u32,
}

impl BezierEditorState {
    pub fn new() -> Self {
        Self { show_control_handles: true, sample_count: 100, ..Default::default() }
    }
}

pub fn draw_bezier_curve(painter: &egui::Painter, curve: &BezierCurve, canvas_rect: egui::Rect, zoom: f32, offset: egui::Vec2) {
    if curve.control_points.len() < 2 { return; }
    let pts = curve.sample_all();
    let screen_pts: Vec<egui::Pos2> = pts.iter().map(|p| {
        egui::Pos2::new(canvas_rect.left() + (p[0] + offset.x) * zoom, canvas_rect.top() + (p[1] + offset.y) * zoom)
    }).collect();
    for w in screen_pts.windows(2) {
        painter.line_segment([w[0], w[1]], egui::Stroke::new(curve.width, curve.color));
    }
    for cp in &curve.control_points {
        let px = canvas_rect.left() + (cp.position[0] + offset.x) * zoom;
        let py = canvas_rect.top() + (cp.position[1] + offset.y) * zoom;
        painter.circle_filled(egui::Pos2::new(px, py), 5.0, curve.color);
    }
}

pub fn show_bezier_editor(ui: &mut egui::Ui, state: &mut BezierEditorState) {
    ui.horizontal(|ui| {
        ui.label("Bezier Curve Editor");
        if ui.button("+ New Curve").clicked() {
            state.curves.push(BezierCurve::new(&format!("Curve {}", state.curves.len())));
        }
        ui.checkbox(&mut state.show_control_handles, "Handles");
        ui.checkbox(&mut state.show_normals, "Normals");
        ui.add(egui::Slider::new(&mut state.sample_count, 10..=500).text("Samples"));
    });

    for (i, curve) in state.curves.iter().enumerate() {
        let sel = state.selected_curve == Some(i);
        if ui.selectable_label(sel, &curve.name).clicked() { state.selected_curve = Some(i); }
    }

    if let Some(ci) = state.selected_curve {
        if let Some(curve) = state.curves.get_mut(ci) {
            ui.separator();
            ui.text_edit_singleline(&mut curve.name);
            ui.checkbox(&mut curve.closed, "Closed");
            ui.add(egui::Slider::new(&mut curve.width, 0.5..=10.0).text("Width"));
            ui.add(egui::Slider::new(&mut curve.resolution, 10..=200).text("Resolution"));
            ui.label(format!("Control Points: {}", curve.control_points.len()));
            if ui.button("Add Point at 0,0").clicked() { curve.add_point([0.0, 0.0]); }
            for cp in curve.control_points.iter_mut() {
                ui.horizontal(|ui| {
                    ui.label(format!("CP {}:", cp.id));
                    ui.add(egui::DragValue::new(&mut cp.position[0]).speed(0.001).prefix("x:"));
                    ui.add(egui::DragValue::new(&mut cp.position[1]).speed(0.001).prefix("y:"));
                    ui.checkbox(&mut cp.smooth, "Smooth");
                });
            }
        }
    }
}

// =================================================================
// SPLINE EXPANSION 2: HEIGHTMAP BRUSH OPERATIONS
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeightmapBrushState {
    pub heightmap: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub active_brush: TerrainBrush,
    pub history: Vec<Vec<f32>>,
    pub undo_limit: usize,
    pub cursor_position: Option<[f32;2]>,
    pub painting: bool,
    pub show_wireframe: bool,
    pub show_contours: bool,
    pub contour_levels: u32,
    pub contour_interval: f32,
}

impl HeightmapBrushState {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            heightmap: vec![0.5; w * h],
            width: w, height: h,
            active_brush: TerrainBrush::default(),
            history: Vec::new(),
            undo_limit: 20,
            cursor_position: None,
            painting: false,
            show_wireframe: false,
            show_contours: false,
            contour_levels: 10,
            contour_interval: 0.1,
        }
    }

    pub fn push_history(&mut self) {
        self.history.push(self.heightmap.clone());
        if self.history.len() > self.undo_limit {
            self.history.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.heightmap = prev;
        }
    }

    pub fn apply_brush_at(&mut self, cx: f32, cy: f32, rng: &mut SplineRng) {
        let w = self.width;
        let h = self.height;
        let brush = self.active_brush.clone();
        brush.apply_to_heightmap(&mut self.heightmap, w, h, cx, cy, rng);
    }

    pub fn normalize(&mut self) {
        let min = self.heightmap.iter().cloned().fold(f32::MAX, f32::min);
        let max = self.heightmap.iter().cloned().fold(f32::MIN, f32::max);
        let range = (max - min).max(0.001);
        for v in self.heightmap.iter_mut() { *v = (*v - min) / range; }
    }

    pub fn invert(&mut self) {
        for v in self.heightmap.iter_mut() { *v = 1.0 - *v; }
    }

    pub fn clamp_values(&mut self, min: f32, max: f32) {
        for v in self.heightmap.iter_mut() { *v = v.clamp(min, max); }
    }

    pub fn fill_all(&mut self, value: f32) {
        for v in self.heightmap.iter_mut() { *v = value; }
    }

    pub fn ocean_mask(&self, sea_level: f32) -> Vec<bool> {
        self.heightmap.iter().map(|&h| h < sea_level).collect()
    }

    pub fn histogram(&self, bins: usize) -> Vec<u32> {
        let mut hist = vec![0u32; bins];
        for &v in &self.heightmap {
            let bin = ((v * bins as f32) as usize).min(bins - 1);
            hist[bin] += 1;
        }
        hist
    }
}

pub fn draw_heightmap_preview(painter: &egui::Painter, state: &HeightmapBrushState, rect: egui::Rect) {
    let w = state.width;
    let h = state.height;
    let cell_w = rect.width() / w as f32;
    let cell_h = rect.height() / h as f32;
    for y in 0..h {
        for x in 0..w {
            let v = state.heightmap[y * w + x];
            let gray = (v * 255.0) as u8;
            let col = egui::Color32::from_gray(gray);
            let px = rect.left() + x as f32 * cell_w;
            let py = rect.top() + y as f32 * cell_h;
            painter.rect_filled(egui::Rect::from_min_size(egui::Pos2::new(px, py), egui::Vec2::new(cell_w, cell_h)), 0.0, col);
        }
    }
}

pub fn show_heightmap_brush_panel(ui: &mut egui::Ui, state: &mut HeightmapBrushState) {
    ui.heading("Heightmap Brush");
    show_terrain_brush_editor(ui, &mut state.active_brush);
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Undo").clicked() { state.undo(); }
        if ui.button("Normalize").clicked() { state.normalize(); }
        if ui.button("Invert").clicked() { state.invert(); }
        if ui.button("Fill 0.5").clicked() { state.fill_all(0.5); }
    });
    ui.checkbox(&mut state.show_wireframe, "Wireframe");
    ui.checkbox(&mut state.show_contours, "Contours");
    if state.show_contours {
        ui.add(egui::Slider::new(&mut state.contour_levels, 2..=20).text("Contour Levels"));
    }
    ui.label(format!("History: {}/{}", state.history.len(), state.undo_limit));
    let hist = state.histogram(16);
    ui.label("Height Distribution:");
    for (i, &count) in hist.iter().enumerate() {
        let frac = count as f32 / state.heightmap.len() as f32;
        let bar_w = frac * 100.0;
        ui.label(format!("  {:.2}-{:.2}: {:>5} ({:.1}%)", i as f32 / 16.0, (i+1) as f32 / 16.0, count, frac*100.0));
    }
}

// =================================================================
// SPLINE EXPANSION 2: SPLINE TRANSFORM TOOLS
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineTransform {
    pub translate: [f32; 2],
    pub rotate_degrees: f32,
    pub scale: [f32; 2],
    pub pivot: [f32; 2],
    pub shear_x: f32,
    pub shear_y: f32,
}

impl Default for SplineTransform {
    fn default() -> Self {
        Self { translate: [0.0, 0.0], rotate_degrees: 0.0, scale: [1.0, 1.0], pivot: [0.0, 0.0], shear_x: 0.0, shear_y: 0.0 }
    }
}

impl SplineTransform {
    pub fn apply_to_point(&self, p: [f32;2]) -> [f32;2] {
        // Translate to pivot
        let px = p[0] - self.pivot[0];
        let py = p[1] - self.pivot[1];
        // Scale
        let sx = px * self.scale[0];
        let sy = py * self.scale[1];
        // Shear
        let shx = sx + sy * self.shear_x;
        let shy = sy + sx * self.shear_y;
        // Rotate
        let angle = self.rotate_degrees * std::f32::consts::PI / 180.0;
        let cos = angle.cos();
        let sin = angle.sin();
        let rx = shx * cos - shy * sin;
        let ry = shx * sin + shy * cos;
        // Translate back and apply translation
        [rx + self.pivot[0] + self.translate[0], ry + self.pivot[1] + self.translate[1]]
    }

    pub fn apply_to_points(&self, points: &[[f32;2]]) -> Vec<[f32;2]> {
        points.iter().map(|&p| self.apply_to_point(p)).collect()
    }

    pub fn identity() -> Self { Self::default() }

    pub fn translation(tx: f32, ty: f32) -> Self { Self { translate: [tx, ty], ..Default::default() } }

    pub fn rotation(degrees: f32, pivot: [f32;2]) -> Self { Self { rotate_degrees: degrees, pivot, ..Default::default() } }

    pub fn uniform_scale(s: f32, pivot: [f32;2]) -> Self { Self { scale: [s, s], pivot, ..Default::default() } }

    pub fn compose(&self, other: &SplineTransform) -> SplineTransform {
        // Simplified composition (translation only for simplicity)
        SplineTransform {
            translate: [self.translate[0] + other.translate[0], self.translate[1] + other.translate[1]],
            rotate_degrees: self.rotate_degrees + other.rotate_degrees,
            scale: [self.scale[0] * other.scale[0], self.scale[1] * other.scale[1]],
            pivot: self.pivot,
            shear_x: self.shear_x + other.shear_x,
            shear_y: self.shear_y + other.shear_y,
        }
    }
}

pub fn show_spline_transform_editor(ui: &mut egui::Ui, xform: &mut SplineTransform) {
    ui.collapsing("Transform", |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut xform.translate[0]).speed(0.001).prefix("TX:"));
            ui.add(egui::DragValue::new(&mut xform.translate[1]).speed(0.001).prefix("TY:"));
        });
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut xform.scale[0]).speed(0.01).prefix("SX:"));
            ui.add(egui::DragValue::new(&mut xform.scale[1]).speed(0.01).prefix("SY:"));
        });
        ui.add(egui::Slider::new(&mut xform.rotate_degrees, -180.0..=180.0).text("Rotation"));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut xform.pivot[0]).speed(0.001).prefix("PivX:"));
            ui.add(egui::DragValue::new(&mut xform.pivot[1]).speed(0.001).prefix("PivY:"));
        });
        ui.add(egui::Slider::new(&mut xform.shear_x, -1.0..=1.0).text("Shear X"));
        ui.add(egui::Slider::new(&mut xform.shear_y, -1.0..=1.0).text("Shear Y"));
        if ui.button("Reset").clicked() { *xform = SplineTransform::default(); }
    });
}

// =================================================================
// SPLINE EXPANSION 2: PARAMETRIC CURVE LIBRARY
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ParametricCurveType {
    Circle,
    Ellipse,
    Spiral,
    Lissajous,
    Rose,
    Hypotrochoid,
    Epitrochoid,
    Sine,
    Cardioid,
    Figure8,
}

impl ParametricCurveType {
    pub fn name(&self) -> &'static str {
        match self {
            ParametricCurveType::Circle => "Circle",
            ParametricCurveType::Ellipse => "Ellipse",
            ParametricCurveType::Spiral => "Spiral",
            ParametricCurveType::Lissajous => "Lissajous",
            ParametricCurveType::Rose => "Rose",
            ParametricCurveType::Hypotrochoid => "Hypotrochoid",
            ParametricCurveType::Epitrochoid => "Epitrochoid",
            ParametricCurveType::Sine => "Sine Wave",
            ParametricCurveType::Cardioid => "Cardioid",
            ParametricCurveType::Figure8 => "Figure-8 (Lemniscate)",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParametricCurveConfig {
    pub curve_type: ParametricCurveType,
    pub param_a: f32,
    pub param_b: f32,
    pub param_c: f32,
    pub param_d: f32,
    pub param_n: u32,
    pub resolution: u32,
    pub t_start: f32,
    pub t_end: f32,
    pub center: [f32; 2],
    pub phase: f32,
}

impl Default for ParametricCurveConfig {
    fn default() -> Self {
        Self { curve_type: ParametricCurveType::Circle, param_a: 1.0, param_b: 1.0, param_c: 1.0, param_d: 0.0, param_n: 3, resolution: 200, t_start: 0.0, t_end: std::f32::consts::TAU, center: [0.0, 0.0], phase: 0.0 }
    }
}

pub fn generate_parametric_curve(cfg: &ParametricCurveConfig) -> Vec<[f32;2]> {
    let n = cfg.resolution as usize;
    let tau = std::f32::consts::TAU;
    (0..=n).map(|i| {
        let t = cfg.t_start + (cfg.t_end - cfg.t_start) * i as f32 / n as f32;
        let (x, y) = match cfg.curve_type {
            ParametricCurveType::Circle => (cfg.param_a * t.cos(), cfg.param_a * t.sin()),
            ParametricCurveType::Ellipse => (cfg.param_a * t.cos(), cfg.param_b * t.sin()),
            ParametricCurveType::Spiral => (cfg.param_a * t * t.cos(), cfg.param_a * t * t.sin()),
            ParametricCurveType::Lissajous => {
                let a = cfg.param_a; let b = cfg.param_b;
                (a * (cfg.param_n as f32 * t + cfg.phase).sin(), b * t.sin())
            }
            ParametricCurveType::Rose => {
                let r = cfg.param_a * (cfg.param_n as f32 * t).cos();
                (r * t.cos(), r * t.sin())
            }
            ParametricCurveType::Hypotrochoid => {
                let R = cfg.param_a; let r = cfg.param_b; let d = cfg.param_c;
                ((R - r) * t.cos() + d * ((R - r) / r * t).cos(),
                 (R - r) * t.sin() - d * ((R - r) / r * t).sin())
            }
            ParametricCurveType::Epitrochoid => {
                let R = cfg.param_a; let r = cfg.param_b; let d = cfg.param_c;
                ((R + r) * t.cos() - d * ((R + r) / r * t).cos(),
                 (R + r) * t.sin() - d * ((R + r) / r * t).sin())
            }
            ParametricCurveType::Sine => (t, cfg.param_a * (cfg.param_b * t + cfg.phase).sin()),
            ParametricCurveType::Cardioid => {
                let r = cfg.param_a * (1.0 - t.cos());
                (r * t.cos(), r * t.sin())
            }
            ParametricCurveType::Figure8 => {
                let a = cfg.param_a;
                (a * t.sin(), a * t.sin() * t.cos())
            }
        };
        [x + cfg.center[0], y + cfg.center[1]]
    }).collect()
}

pub fn show_parametric_curve_editor(ui: &mut egui::Ui, cfg: &mut ParametricCurveConfig, preview_pts: &mut Vec<[f32;2]>) {
    ui.heading("Parametric Curve Generator");
    ui.horizontal(|ui| {
        for (ct, label) in [
            (ParametricCurveType::Circle, "Circle"),
            (ParametricCurveType::Ellipse, "Ellipse"),
            (ParametricCurveType::Spiral, "Spiral"),
            (ParametricCurveType::Lissajous, "Lissajous"),
            (ParametricCurveType::Rose, "Rose"),
        ] {
            if ui.selectable_label(cfg.curve_type == ct, label).clicked() { cfg.curve_type = ct; }
        }
    });
    ui.horizontal(|ui| {
        for (ct, label) in [
            (ParametricCurveType::Hypotrochoid, "Hypo"),
            (ParametricCurveType::Epitrochoid, "Epi"),
            (ParametricCurveType::Sine, "Sine"),
            (ParametricCurveType::Cardioid, "Cardioid"),
            (ParametricCurveType::Figure8, "Figure-8"),
        ] {
            if ui.selectable_label(cfg.curve_type == ct, label).clicked() { cfg.curve_type = ct; }
        }
    });
    ui.add(egui::Slider::new(&mut cfg.param_a, 0.01..=10.0).text("Param A"));
    ui.add(egui::Slider::new(&mut cfg.param_b, 0.01..=10.0).text("Param B"));
    ui.add(egui::Slider::new(&mut cfg.param_c, 0.01..=5.0).text("Param C"));
    ui.add(egui::DragValue::new(&mut cfg.param_n).clamp_range(1..=20u32).prefix("n: "));
    ui.add(egui::Slider::new(&mut cfg.phase, 0.0..=std::f32::consts::TAU).text("Phase"));
    ui.add(egui::Slider::new(&mut cfg.resolution, 10..=1000).text("Resolution"));
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut cfg.center[0]).speed(0.01).prefix("Cx:"));
        ui.add(egui::DragValue::new(&mut cfg.center[1]).speed(0.01).prefix("Cy:"));
    });
    if ui.button("Generate").clicked() {
        *preview_pts = generate_parametric_curve(cfg);
    }
    ui.label(format!("Points: {}", preview_pts.len()));
}

// =================================================================
// SPLINE EXPANSION 2: FULL STATE & TESTS
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SplineEditorSuperState {
    pub bezier_state: BezierEditorState,
    pub heightmap_brush: HeightmapBrushState,
    pub transform: SplineTransform,
    pub parametric_cfg: ParametricCurveConfig,
    pub parametric_preview: Vec<[f32;2]>,
    pub full_state: FullSplineEditorState,
    pub active_mega_tab: SplineMegaTab,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum SplineMegaTab {
    #[default]
    Basic,
    Bezier,
    Heightmap,
    Parametric,
    Advanced,
}

impl SplineEditorSuperState {
    pub fn new() -> Self {
        Self {
            heightmap_brush: HeightmapBrushState::new(64, 64),
            bezier_state: BezierEditorState::new(),
            parametric_cfg: ParametricCurveConfig::default(),
            ..Default::default()
        }
    }
}

impl Default for HeightmapBrushState {
    fn default() -> Self { HeightmapBrushState::new(64, 64) }
}

pub fn show_spline_super_editor(ui: &mut egui::Ui, state: &mut SplineEditorSuperState) {
    ui.horizontal(|ui| {
        for (tab, label) in [
            (SplineMegaTab::Basic, "Basic"),
            (SplineMegaTab::Bezier, "Bezier"),
            (SplineMegaTab::Heightmap, "Heightmap"),
            (SplineMegaTab::Parametric, "Parametric"),
            (SplineMegaTab::Advanced, "Advanced"),
        ] {
            if ui.selectable_label(state.active_mega_tab == tab, label).clicked() { state.active_mega_tab = tab; }
        }
    });
    ui.separator();
    match state.active_mega_tab {
        SplineMegaTab::Bezier => show_bezier_editor(ui, &mut state.bezier_state),
        SplineMegaTab::Heightmap => show_heightmap_brush_panel(ui, &mut state.heightmap_brush),
        SplineMegaTab::Parametric => show_parametric_curve_editor(ui, &mut state.parametric_cfg, &mut state.parametric_preview),
        SplineMegaTab::Advanced => {
            show_spline_transform_editor(ui, &mut state.transform);
            show_full_spline_panel(ui, &mut state.full_state);
        }
        _ => { ui.label("Select a tab above."); }
    }
}

#[cfg(test)]
mod spline_expansion2_tests {
    use super::*;

    #[test]
    fn test_bezier_control_point_smooth() {
        let mut cp = BezierControlPoint::new(0, [0.0, 0.0]);
        cp.set_control_out([1.0, 0.0]);
        assert_eq!(cp.control_in, [-1.0, 0.0]);
    }

    #[test]
    fn test_bezier_curve_sample() {
        let mut curve = BezierCurve::new("Test");
        curve.add_point([0.0, 0.0]);
        curve.add_point([1.0, 0.0]);
        let pt = curve.sample(0.5);
        assert!(pt.is_some());
    }

    #[test]
    fn test_spline_transform_apply() {
        let xform = SplineTransform::translation(1.0, 2.0);
        let result = xform.apply_to_point([0.0, 0.0]);
        assert!((result[0] - 1.0).abs() < 0.001);
        assert!((result[1] - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_spline_transform_rotation() {
        let xform = SplineTransform::rotation(90.0, [0.0, 0.0]);
        let result = xform.apply_to_point([1.0, 0.0]);
        assert!(result[0].abs() < 0.001);
        assert!((result[1] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_spline_transform_scale() {
        let xform = SplineTransform::uniform_scale(2.0, [0.0, 0.0]);
        let result = xform.apply_to_point([3.0, 4.0]);
        assert!((result[0] - 6.0).abs() < 0.001);
        assert!((result[1] - 8.0).abs() < 0.001);
    }

    #[test]
    fn test_parametric_circle() {
        let mut cfg = ParametricCurveConfig::default();
        cfg.curve_type = ParametricCurveType::Circle;
        cfg.param_a = 1.0;
        cfg.resolution = 10;
        let pts = generate_parametric_curve(&cfg);
        assert!(!pts.is_empty());
        // All points should be approximately distance 1 from center
        for pt in &pts {
            let dist = (pt[0]*pt[0] + pt[1]*pt[1]).sqrt();
            assert!((dist - 1.0).abs() < 0.01, "dist = {}", dist);
        }
    }

    #[test]
    fn test_parametric_ellipse() {
        let mut cfg = ParametricCurveConfig::default();
        cfg.curve_type = ParametricCurveType::Ellipse;
        cfg.param_a = 2.0; cfg.param_b = 1.0;
        cfg.resolution = 20;
        let pts = generate_parametric_curve(&cfg);
        assert!(!pts.is_empty());
    }

    #[test]
    fn test_heightmap_brush_raise() {
        let mut state = HeightmapBrushState::new(32, 32);
        state.active_brush.mode = TerrainBrushMode::Raise;
        state.active_brush.radius = 5.0;
        let center_before = state.heightmap[16 * 32 + 16];
        let mut rng = SplineRng::new(42);
        state.apply_brush_at(16.0, 16.0, &mut rng);
        let center_after = state.heightmap[16 * 32 + 16];
        assert!(center_after >= center_before);
    }

    #[test]
    fn test_heightmap_normalize() {
        let mut state = HeightmapBrushState::new(4, 4);
        state.heightmap = vec![0.0, 0.25, 0.5, 0.75, 1.0, 0.3, 0.6, 0.9, 0.1, 0.4, 0.7, 0.8, 0.2, 0.45, 0.65, 0.85];
        state.normalize();
        let min = state.heightmap.iter().cloned().fold(f32::MAX, f32::min);
        let max = state.heightmap.iter().cloned().fold(f32::MIN, f32::max);
        assert!(min < 0.01);
        assert!(max > 0.99);
    }

    #[test]
    fn test_heightmap_histogram() {
        let mut state = HeightmapBrushState::new(4, 4);
        state.fill_all(0.5);
        let hist = state.histogram(10);
        let total: u32 = hist.iter().sum();
        assert_eq!(total, 16);
    }

    #[test]
    fn test_bezier_curve_sampling() {
        let mut curve = BezierCurve::new("Test");
        curve.add_point([0.0, 0.0]);
        curve.add_point([5.0, 0.0]);
        curve.add_point([10.0, 5.0]);
        let pts = curve.sample_all();
        assert!(!pts.is_empty());
    }
}
'''

with open(r'C:\proof-engine\editor\src\spline_editor.rs', 'a', encoding='utf-8') as f:
    f.write(code)
print(f"Done. spline_editor size: {__import__('os').path.getsize(r'C:\proof-engine\editor\src\spline_editor.rs')} bytes")
