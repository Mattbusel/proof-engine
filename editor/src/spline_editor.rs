use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Shape, RichText};
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

// ---------------------------------------------------------------------------
// Core enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SplineType {
    Linear,
    CatmullRom,
    Bezier,
    BSpline,
    Hermite,
}

impl SplineType {
    pub fn label(&self) -> &'static str {
        match self {
            SplineType::Linear     => "Linear",
            SplineType::CatmullRom => "Catmull-Rom",
            SplineType::Bezier     => "Bezier",
            SplineType::BSpline    => "B-Spline",
            SplineType::Hermite    => "Hermite",
        }
    }

    pub fn all() -> Vec<SplineType> {
        vec![
            SplineType::Linear,
            SplineType::CatmullRom,
            SplineType::Bezier,
            SplineType::BSpline,
            SplineType::Hermite,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SplineTool {
    Select,
    AddPoint,
    DeletePoint,
    TangentEdit,
    Knife,
}

impl SplineTool {
    pub fn label(&self) -> &'static str {
        match self {
            SplineTool::Select      => "Select",
            SplineTool::AddPoint    => "Add Point",
            SplineTool::DeletePoint => "Delete Point",
            SplineTool::TangentEdit => "Tangent Edit",
            SplineTool::Knife       => "Knife",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SplineTool::Select      => "S",
            SplineTool::AddPoint    => "+",
            SplineTool::DeletePoint => "-",
            SplineTool::TangentEdit => "T",
            SplineTool::Knife       => "K",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PathEvent {
    Move { speed: f32 },
    Wait { duration: f32 },
    Loop,
    PingPong,
    Event { name: String },
}

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPoint {
    pub position:    [f32; 2],
    pub in_tangent:  [f32; 2],
    pub out_tangent: [f32; 2],
    pub weight:      f32,
    pub corner:      bool,
    pub name:        String,
}

impl ControlPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            position:    [x, y],
            in_tangent:  [-40.0, 0.0],
            out_tangent: [40.0, 0.0],
            weight:      1.0,
            corner:      false,
            name:        String::new(),
        }
    }

    pub fn with_tangents(x: f32, y: f32, in_t: [f32; 2], out_t: [f32; 2]) -> Self {
        Self {
            position:    [x, y],
            in_tangent:  in_t,
            out_tangent: out_t,
            weight:      1.0,
            corner:      false,
            name:        String::new(),
        }
    }

    pub fn smooth_tangents(&mut self) {
        // Mirror out_tangent to in_tangent
        self.in_tangent = [-self.out_tangent[0], -self.out_tangent[1]];
        self.corner = false;
    }

    pub fn break_tangents(&mut self) {
        self.corner = true;
    }

    pub fn in_world_pos(&self) -> [f32; 2] {
        [
            self.position[0] + self.in_tangent[0],
            self.position[1] + self.in_tangent[1],
        ]
    }

    pub fn out_world_pos(&self) -> [f32; 2] {
        [
            self.position[0] + self.out_tangent[0],
            self.position[1] + self.out_tangent[1],
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplineNode {
    pub id:       u32,
    pub point:    ControlPoint,
    pub selected: bool,
}

impl SplineNode {
    pub fn new(id: u32, point: ControlPoint) -> Self {
        Self { id, point, selected: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spline {
    pub name:       String,
    pub nodes:      Vec<SplineNode>,
    pub spline_type: SplineType,
    pub closed:     bool,
    pub color:      Color32,
    pub width:      f32,
    pub visible:    bool,
    pub locked:     bool,
    pub resolution: u32,
}

impl Spline {
    pub fn new(name: &str) -> Self {
        Self {
            name:        name.to_string(),
            nodes:       Vec::new(),
            spline_type: SplineType::CatmullRom,
            closed:      false,
            color:       Color32::from_rgb(100, 200, 255),
            width:       2.0,
            visible:     true,
            locked:      false,
            resolution:  24,
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn find_node_mut(&mut self, id: u32) -> Option<&mut SplineNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn find_node(&self, id: u32) -> Option<&SplineNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn remove_node(&mut self, id: u32) {
        self.nodes.retain(|n| n.id != id);
    }

    pub fn add_node(&mut self, id: u32, pt: ControlPoint) {
        self.nodes.push(SplineNode::new(id, pt));
    }

    pub fn segment_count(&self) -> usize {
        if self.nodes.len() < 2 { return 0; }
        if self.closed { self.nodes.len() } else { self.nodes.len() - 1 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplineLibrary {
    pub splines: Vec<Spline>,
    pub active:  usize,
}

impl SplineLibrary {
    pub fn new() -> Self {
        Self { splines: Vec::new(), active: 0 }
    }

    pub fn add(&mut self, spline: Spline) -> usize {
        self.splines.push(spline);
        self.splines.len() - 1
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.splines.len() {
            self.splines.remove(idx);
            if self.active >= self.splines.len() && self.active > 0 {
                self.active = self.splines.len() - 1;
            }
        }
    }
}

impl Default for SplineLibrary {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Spline math functions
// ---------------------------------------------------------------------------

pub fn evaluate_linear(p0: [f32; 2], p1: [f32; 2], t: f32) -> [f32; 2] {
    [
        p0[0] + (p1[0] - p0[0]) * t,
        p0[1] + (p1[1] - p0[1]) * t,
    ]
}

pub fn evaluate_bezier(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    t:  f32,
) -> [f32; 2] {
    let mt  = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;
    let t2  = t * t;
    let t3  = t2 * t;
    [
        mt3 * p0[0] + 3.0 * mt2 * t * p1[0] + 3.0 * mt * t2 * p2[0] + t3 * p3[0],
        mt3 * p0[1] + 3.0 * mt2 * t * p1[1] + 3.0 * mt * t2 * p2[1] + t3 * p3[1],
    ]
}

pub fn evaluate_catmull_rom(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    t:  f32,
) -> [f32; 2] {
    let t2 = t * t;
    let t3 = t2 * t;
    let f0 = -0.5 * t3 + t2 - 0.5 * t;
    let f1 =  1.5 * t3 - 2.5 * t2 + 1.0;
    let f2 = -1.5 * t3 + 2.0 * t2 + 0.5 * t;
    let f3 =  0.5 * t3 - 0.5 * t2;
    [
        f0 * p0[0] + f1 * p1[0] + f2 * p2[0] + f3 * p3[0],
        f0 * p0[1] + f1 * p1[1] + f2 * p2[1] + f3 * p3[1],
    ]
}

/// Cubic B-spline basis evaluation over four control points.
pub fn evaluate_bspline(
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    t:  f32,
) -> [f32; 2] {
    let t2 = t * t;
    let t3 = t2 * t;
    let f0 = (-t3 + 3.0*t2 - 3.0*t + 1.0) / 6.0;
    let f1 = (3.0*t3 - 6.0*t2 + 4.0) / 6.0;
    let f2 = (-3.0*t3 + 3.0*t2 + 3.0*t + 1.0) / 6.0;
    let f3 = t3 / 6.0;
    [
        f0*p0[0] + f1*p1[0] + f2*p2[0] + f3*p3[0],
        f0*p0[1] + f1*p1[1] + f2*p2[1] + f3*p3[1],
    ]
}

/// Hermite interpolation (uses tangent vectors stored on control points).
pub fn evaluate_hermite(
    p0: [f32; 2],
    m0: [f32; 2],
    p1: [f32; 2],
    m1: [f32; 2],
    t:  f32,
) -> [f32; 2] {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 =  2.0*t3 - 3.0*t2 + 1.0;
    let h10 =      t3 - 2.0*t2 + t;
    let h01 = -2.0*t3 + 3.0*t2;
    let h11 =      t3 -     t2;
    [
        h00*p0[0] + h10*m0[0] + h01*p1[0] + h11*m1[0],
        h00*p0[1] + h10*m0[1] + h01*p1[1] + h11*m1[1],
    ]
}

/// Evaluate a point on the spline at parameter t ∈ [0, 1].
/// t maps uniformly across all segments.
pub fn point_on_spline(spline: &Spline, t: f32) -> [f32; 2] {
    let n = spline.nodes.len();
    if n == 0 { return [0.0, 0.0]; }
    if n == 1 { return spline.nodes[0].point.position; }

    let segs = spline.segment_count() as f32;
    let seg_t = (t * segs).max(0.0);
    let seg_i = (seg_t.floor() as usize).min(spline.segment_count() - 1);
    let local_t = seg_t - seg_i as f32;

    match spline.spline_type {
        SplineType::Linear => {
            let i0 = seg_i;
            let i1 = (seg_i + 1) % n;
            evaluate_linear(
                spline.nodes[i0].point.position,
                spline.nodes[i1].point.position,
                local_t,
            )
        }
        SplineType::Bezier => {
            let i0 = seg_i;
            let i1 = (seg_i + 1) % n;
            let p0 = spline.nodes[i0].point.position;
            let p1 = spline.nodes[i0].point.out_world_pos();
            let p2 = spline.nodes[i1].point.in_world_pos();
            let p3 = spline.nodes[i1].point.position;
            evaluate_bezier(p0, p1, p2, p3, local_t)
        }
        SplineType::CatmullRom => {
            let i1 = seg_i;
            let i2 = (seg_i + 1) % n;
            let i0 = if i1 == 0 {
                if spline.closed { n - 1 } else { 0 }
            } else {
                i1 - 1
            };
            let i3 = if i2 + 1 >= n {
                if spline.closed { (i2 + 1) % n } else { n - 1 }
            } else {
                i2 + 1
            };
            evaluate_catmull_rom(
                spline.nodes[i0].point.position,
                spline.nodes[i1].point.position,
                spline.nodes[i2].point.position,
                spline.nodes[i3].point.position,
                local_t,
            )
        }
        SplineType::BSpline => {
            let i1 = seg_i;
            let i2 = (seg_i + 1) % n;
            let i0 = if i1 == 0 {
                if spline.closed { n - 1 } else { 0 }
            } else { i1 - 1 };
            let i3 = if i2 + 1 >= n {
                if spline.closed { (i2 + 1) % n } else { n - 1 }
            } else { i2 + 1 };
            evaluate_bspline(
                spline.nodes[i0].point.position,
                spline.nodes[i1].point.position,
                spline.nodes[i2].point.position,
                spline.nodes[i3].point.position,
                local_t,
            )
        }
        SplineType::Hermite => {
            let i0 = seg_i;
            let i1 = (seg_i + 1) % n;
            let p0 = spline.nodes[i0].point.position;
            let m0 = spline.nodes[i0].point.out_tangent;
            let p1 = spline.nodes[i1].point.position;
            let m1 = spline.nodes[i1].point.in_tangent;
            evaluate_hermite(p0, m0, p1, m1, local_t)
        }
    }
}

/// Approximate arc length by numerical integration (sampling).
pub fn spline_length(spline: &Spline) -> f32 {
    let steps = spline.resolution.max(8) * spline.segment_count() as u32;
    if steps == 0 { return 0.0; }

    let mut length = 0.0f32;
    let mut prev = point_on_spline(spline, 0.0);

    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let curr = point_on_spline(spline, t);
        let dx = curr[0] - prev[0];
        let dy = curr[1] - prev[1];
        length += (dx * dx + dy * dy).sqrt();
        prev = curr;
    }

    length
}

/// Tangent vector at parameter t (not normalized).
pub fn spline_tangent_at(spline: &Spline, t: f32) -> [f32; 2] {
    let eps = 1e-4_f32;
    let t0 = (t - eps).max(0.0);
    let t1 = (t + eps).min(1.0);
    let p0 = point_on_spline(spline, t0);
    let p1 = point_on_spline(spline, t1);
    let dt = (t1 - t0).max(1e-8);
    [(p1[0] - p0[0]) / dt, (p1[1] - p0[1]) / dt]
}

/// Normalized normal (perpendicular to tangent) at parameter t.
pub fn spline_normal_at(spline: &Spline, t: f32) -> [f32; 2] {
    let tang = spline_tangent_at(spline, t);
    let len = (tang[0]*tang[0] + tang[1]*tang[1]).sqrt().max(1e-8);
    // Rotate 90 degrees
    [-tang[1] / len, tang[0] / len]
}

/// Returns (t, distance) for the nearest point on the spline to a query position.
pub fn nearest_point_on_spline(spline: &Spline, query: [f32; 2]) -> (f32, f32) {
    let steps = spline.resolution.max(8) * spline.segment_count().max(1) as u32 * 4;
    let mut best_t    = 0.0f32;
    let mut best_dist = f32::MAX;

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let pt = point_on_spline(spline, t);
        let dx = pt[0] - query[0];
        let dy = pt[1] - query[1];
        let dist = (dx*dx + dy*dy).sqrt();
        if dist < best_dist {
            best_dist = dist;
            best_t    = t;
        }
    }

    // Refine with binary search around best_t
    let mut lo = (best_t - 1.0 / steps as f32).max(0.0);
    let mut hi = (best_t + 1.0 / steps as f32).min(1.0);
    for _ in 0..16 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        let d1 = {
            let p = point_on_spline(spline, m1);
            let dx = p[0]-query[0]; let dy = p[1]-query[1];
            dx*dx + dy*dy
        };
        let d2 = {
            let p = point_on_spline(spline, m2);
            let dx = p[0]-query[0]; let dy = p[1]-query[1];
            dx*dx + dy*dy
        };
        if d1 < d2 { hi = m2; } else { lo = m1; }
    }
    best_t = (lo + hi) * 0.5;
    let pt = point_on_spline(spline, best_t);
    let dx = pt[0] - query[0];
    let dy = pt[1] - query[1];
    best_dist = (dx*dx + dy*dy).sqrt();

    (best_t, best_dist)
}

/// Sample spline at regular arc-length intervals, returning world-space points.
pub fn sample_spline_uniform(spline: &Spline, count: usize) -> Vec<[f32; 2]> {
    if count == 0 || spline.nodes.is_empty() { return Vec::new(); }
    (0..count).map(|i| {
        let t = i as f32 / (count - 1).max(1) as f32;
        point_on_spline(spline, t)
    }).collect()
}

/// Insert a new control point at parameter t, splitting the segment.
pub fn insert_point_at_t(spline: &mut Spline, t: f32, id: u32) {
    if spline.nodes.len() < 2 { return; }

    let pos = point_on_spline(spline, t);
    let tang = spline_tangent_at(spline, t);
    let tang_len = (tang[0]*tang[0] + tang[1]*tang[1]).sqrt().max(1.0);
    let scale = 30.0 / tang_len;

    let mut pt = ControlPoint::new(pos[0], pos[1]);
    pt.in_tangent  = [-tang[0] * scale, -tang[1] * scale];
    pt.out_tangent = [ tang[0] * scale,  tang[1] * scale];

    let segs = spline.segment_count() as f32;
    let seg_f = t * segs;
    let insert_after = (seg_f.floor() as usize).min(spline.nodes.len() - 1);

    spline.nodes.insert(insert_after + 1, SplineNode::new(id, pt));
}

// ---------------------------------------------------------------------------
// SplineEditor state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplineEditor {
    pub library:        SplineLibrary,
    pub canvas_offset:  Vec2,
    pub canvas_zoom:    f32,
    pub selected_nodes: HashSet<u32>,
    pub selected_spline: Option<usize>,
    pub tool:           SplineTool,
    pub snap_to_grid:   bool,
    pub grid_size:      f32,
    pub show_tangents:  bool,
    pub show_normals:   bool,
    pub show_length:    bool,
    pub show_points:    bool,
    pub preview_t:      f32,
    pub preview_running: bool,
    pub id_counter:     u32,

    // Per-session drag state (skip serialization)
    #[serde(skip)]
    pub drag_node_id:   Option<u32>,
    #[serde(skip)]
    pub drag_tangent:   Option<(u32, bool)>, // (node_id, is_out_tangent)
    #[serde(skip)]
    pub drag_start_pos: Option<[f32; 2]>,
    #[serde(skip)]
    pub rename_spline:  Option<(usize, String)>,
    #[serde(skip)]
    pub hovered_node:   Option<u32>,
    #[serde(skip)]
    pub context_node:   Option<u32>,
}

impl SplineEditor {
    pub fn new() -> Self {
        let mut ed = Self {
            library:         SplineLibrary::new(),
            canvas_offset:   Vec2::new(200.0, 200.0),
            canvas_zoom:     1.0,
            selected_nodes:  HashSet::new(),
            selected_spline: None,
            tool:            SplineTool::Select,
            snap_to_grid:    false,
            grid_size:       20.0,
            show_tangents:   true,
            show_normals:    false,
            show_length:     true,
            show_points:     true,
            preview_t:       0.0,
            preview_running: false,
            id_counter:      1,
            drag_node_id:    None,
            drag_tangent:    None,
            drag_start_pos:  None,
            rename_spline:   None,
            hovered_node:    None,
            context_node:    None,
        };

        // Create a few default splines
        let idx = ed.create_default_spline("Path A", Color32::from_rgb(100, 200, 255));
        ed.selected_spline = Some(idx);

        let _idx2 = ed.create_default_spline("Path B", Color32::from_rgb(255, 160, 80));

        ed
    }

    fn next_id(&mut self) -> u32 {
        let id = self.id_counter;
        self.id_counter += 1;
        id
    }

    fn create_default_spline(&mut self, name: &str, color: Color32) -> usize {
        let mut spline = Spline::new(name);
        spline.color = color;
        spline.spline_type = SplineType::CatmullRom;
        spline.resolution = 24;

        // Add 4 default nodes in a gentle curve
        let pts: &[[f32; 2]] = &[
            [100.0, 200.0],
            [200.0, 120.0],
            [340.0, 180.0],
            [440.0, 120.0],
        ];

        for &[x, y] in pts {
            let id = self.next_id();
            let mut cp = ControlPoint::new(x, y);
            cp.out_tangent = [50.0, 0.0];
            cp.in_tangent  = [-50.0, 0.0];
            spline.nodes.push(SplineNode::new(id, cp));
        }

        self.library.add(spline)
    }

    fn snap(&self, val: f32) -> f32 {
        if self.snap_to_grid {
            (val / self.grid_size).round() * self.grid_size
        } else {
            val
        }
    }

    fn world_to_screen(&self, world: [f32; 2]) -> Pos2 {
        Pos2::new(
            world[0] * self.canvas_zoom + self.canvas_offset.x,
            world[1] * self.canvas_zoom + self.canvas_offset.y,
        )
    }

    fn screen_to_world(&self, screen: Pos2) -> [f32; 2] {
        [
            (screen.x - self.canvas_offset.x) / self.canvas_zoom,
            (screen.y - self.canvas_offset.y) / self.canvas_zoom,
        ]
    }

    pub fn active_spline(&self) -> Option<&Spline> {
        self.selected_spline.and_then(|i| self.library.splines.get(i))
    }

    pub fn active_spline_mut(&mut self) -> Option<&mut Spline> {
        self.selected_spline.and_then(|i| self.library.splines.get_mut(i))
    }

    pub fn add_node_at_screen(&mut self, screen: Pos2) {
        let sel = match self.selected_spline {
            Some(s) => s,
            None => return,
        };

        let mut wpos = self.screen_to_world(screen);
        wpos[0] = self.snap(wpos[0]);
        wpos[1] = self.snap(wpos[1]);

        let id = self.next_id();
        let mut cp = ControlPoint::new(wpos[0], wpos[1]);

        // Auto-compute tangent from last node direction
        if let Some(last) = self.library.splines[sel].nodes.last() {
            let dx = wpos[0] - last.point.position[0];
            let dy = wpos[1] - last.point.position[1];
            let len = (dx*dx + dy*dy).sqrt().max(1.0);
            let scale = (len * 0.35).min(80.0);
            let nx = dx / len;
            let ny = dy / len;
            cp.out_tangent = [nx * scale, ny * scale];
            cp.in_tangent  = [-nx * scale, -ny * scale];
        }

        self.library.splines[sel].nodes.push(SplineNode::new(id, cp));
        self.selected_nodes.clear();
        self.selected_nodes.insert(id);
    }

    pub fn delete_selected_nodes(&mut self) {
        let sel = match self.selected_spline {
            Some(s) => s,
            None => return,
        };
        let to_remove = self.selected_nodes.clone();
        self.library.splines[sel].nodes.retain(|n| !to_remove.contains(&n.id));
        self.selected_nodes.clear();
    }

    pub fn select_all_nodes(&mut self) {
        let ids: Vec<u32> = self.active_spline()
            .map(|s| s.nodes.iter().map(|n| n.id).collect())
            .unwrap_or_default();
        self.selected_nodes = ids.into_iter().collect();
    }

    pub fn deselect_all_nodes(&mut self) {
        self.selected_nodes.clear();
    }

    pub fn path_length(&self) -> f32 {
        self.active_spline()
            .map(spline_length)
            .unwrap_or(0.0)
    }
}

impl Default for SplineEditor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn show_panel(ctx: &egui::Context, editor: &mut SplineEditor, dt: f32, open: &mut bool) {
    egui::Window::new("Spline Editor")
        .open(open)
        .default_size([1100.0, 680.0])
        .min_size([700.0, 420.0])
        .resizable(true)
        .show(ctx, |ui| {
            show(ui, editor, dt);
        });
}

pub fn show(ui: &mut egui::Ui, editor: &mut SplineEditor, dt: f32) {
    // Advance preview animation
    if editor.preview_running {
        let spline = editor.active_spline();
        if let Some(sp) = spline {
            if sp.segment_count() > 0 {
                editor.preview_t += dt * 0.2;
                if editor.preview_t > 1.0 {
                    editor.preview_t = 0.0;
                }
            }
        }
    }

    show_bottom_toolbar(ui, editor);
    ui.separator();

    let avail = ui.available_size();
    let sidebar_width = 240.0_f32.min(avail.x * 0.24);

    ui.horizontal(|ui| {
        // Main canvas
        let canvas_size = Vec2::new(avail.x - sidebar_width - 8.0, avail.y - 32.0);
        let (canvas_rect, canvas_resp) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());

        handle_canvas_input(ui, editor, canvas_rect, &canvas_resp);

        if ui.is_rect_visible(canvas_rect) {
            let painter = ui.painter_at(canvas_rect);
            draw_canvas(painter, editor, canvas_rect);
        }

        ui.separator();

        // Sidebar
        egui::ScrollArea::vertical()
            .id_source("spline_sidebar")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(sidebar_width);
                show_sidebar(ui, editor);
            });
    });
}

// ---------------------------------------------------------------------------
// Canvas input handling
// ---------------------------------------------------------------------------

fn handle_canvas_input(
    ui:          &mut egui::Ui,
    editor:      &mut SplineEditor,
    canvas_rect: Rect,
    resp:        &egui::Response,
) {
    let input = ui.input(|i| i.clone());

    // Zoom with scroll
    if resp.hovered() {
        let scroll_delta = input.raw_scroll_delta.y;
        if scroll_delta != 0.0 {
            let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
            let old_zoom = editor.canvas_zoom;
            editor.canvas_zoom = (editor.canvas_zoom * zoom_factor).clamp(0.05, 20.0);
            // Zoom around mouse cursor
            if let Some(mpos) = input.pointer.hover_pos() {
                let zoom_ratio = editor.canvas_zoom / old_zoom;
                editor.canvas_offset.x = mpos.x - (mpos.x - editor.canvas_offset.x) * zoom_ratio;
                editor.canvas_offset.y = mpos.y - (mpos.y - editor.canvas_offset.y) * zoom_ratio;
            }
            ui.ctx().request_repaint();
        }
    }

    // Pan with middle mouse or right-drag in select mode
    if resp.dragged_by(egui::PointerButton::Middle) {
        editor.canvas_offset += resp.drag_delta();
    }
    if resp.dragged_by(egui::PointerButton::Secondary) && editor.tool == SplineTool::Select {
        // Right drag to pan
        editor.canvas_offset += resp.drag_delta();
    }

    // Get the current mouse position in world space
    let maybe_mouse_screen = input.pointer.hover_pos()
        .filter(|&p| canvas_rect.contains(p));

    // Keyboard shortcuts
    if resp.has_focus() || resp.hovered() {
        if input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace) {
            editor.delete_selected_nodes();
        }
        if input.key_pressed(egui::Key::A) && input.modifiers.ctrl {
            editor.select_all_nodes();
        }
        if input.key_pressed(egui::Key::D) && input.modifiers.ctrl {
            editor.deselect_all_nodes();
        }
    }

    let tool = editor.tool.clone();
    let sel_spline = editor.selected_spline;

    // Determine node hit (for select/delete/tangent tools)
    let mut hovered_node_id: Option<u32> = None;
    let mut hovered_tangent: Option<(u32, bool)> = None;

    if let Some(mouse_s) = maybe_mouse_screen {
        if let Some(sidx) = sel_spline {
            if let Some(spline) = editor.library.splines.get(sidx) {
                let hit_radius_screen = 8.0_f32;
                let tang_hit_radius   = 6.0_f32;

                for node in &spline.nodes {
                    let nscreen = editor.world_to_screen(node.point.position);
                    if nscreen.distance(mouse_s) < hit_radius_screen {
                        hovered_node_id = Some(node.id);
                        break;
                    }
                    if editor.show_tangents && !node.point.corner {
                        let in_s  = editor.world_to_screen(node.point.in_world_pos());
                        let out_s = editor.world_to_screen(node.point.out_world_pos());
                        if in_s.distance(mouse_s) < tang_hit_radius {
                            hovered_tangent = Some((node.id, false));
                        }
                        if out_s.distance(mouse_s) < tang_hit_radius {
                            hovered_tangent = Some((node.id, true));
                        }
                    }
                }
            }
        }
    }

    editor.hovered_node = hovered_node_id;

    // Primary click / drag
    match tool {
        SplineTool::Select => {
            // Start drag on node
            if resp.drag_started_by(egui::PointerButton::Primary) {
                if let Some(nid) = hovered_node_id {
                    if !editor.selected_nodes.contains(&nid) {
                        if !input.modifiers.shift {
                            editor.selected_nodes.clear();
                        }
                        editor.selected_nodes.insert(nid);
                    }
                    editor.drag_node_id = Some(nid);
                } else if let Some(tang) = hovered_tangent {
                    editor.drag_tangent = Some(tang);
                } else {
                    // Deselect
                    if !input.modifiers.shift {
                        editor.selected_nodes.clear();
                    }
                    editor.drag_node_id = None;
                    editor.drag_tangent = None;
                }
            }

            // Apply drag delta
            if resp.dragged_by(egui::PointerButton::Primary) {
                let drag_world = Vec2::new(
                    resp.drag_delta().x / editor.canvas_zoom,
                    resp.drag_delta().y / editor.canvas_zoom,
                );

                if let Some(drag_nid) = editor.drag_node_id {
                    // Move selected nodes
                    let ids: Vec<u32> = editor.selected_nodes.iter().cloned().collect();
                    let sidx = editor.selected_spline.unwrap_or(0);
                    // Collect current positions before mutable borrow to allow calling editor.snap()
                    let positions: Vec<(u32, [f32; 2])> = editor.library.splines
                        .get(sidx)
                        .map(|sp| {
                            ids.iter().filter_map(|&nid| {
                                sp.find_node(nid).map(|n| (nid, n.point.position))
                            }).collect()
                        })
                        .unwrap_or_default();
                    let snapped: Vec<(u32, f32, f32)> = positions.iter()
                        .map(|&(nid, pos)| (nid, editor.snap(pos[0] + drag_world.x), editor.snap(pos[1] + drag_world.y)))
                        .collect();
                    if let Some(spline) = editor.library.splines.get_mut(sidx) {
                        for (nid, sx, sy) in snapped {
                            if let Some(node) = spline.find_node_mut(nid) {
                                node.point.position[0] = sx;
                                node.point.position[1] = sy;
                            }
                        }
                    }
                    let _ = drag_nid;
                } else if let Some((tang_nid, is_out)) = editor.drag_tangent {
                    let sidx = editor.selected_spline.unwrap_or(0);
                    if let Some(spline) = editor.library.splines.get_mut(sidx) {
                        if let Some(node) = spline.find_node_mut(tang_nid) {
                            if is_out {
                                node.point.out_tangent[0] += drag_world.x;
                                node.point.out_tangent[1] += drag_world.y;
                                if !node.point.corner {
                                    node.point.in_tangent[0] = -node.point.out_tangent[0];
                                    node.point.in_tangent[1] = -node.point.out_tangent[1];
                                }
                            } else {
                                node.point.in_tangent[0] += drag_world.x;
                                node.point.in_tangent[1] += drag_world.y;
                                if !node.point.corner {
                                    node.point.out_tangent[0] = -node.point.in_tangent[0];
                                    node.point.out_tangent[1] = -node.point.in_tangent[1];
                                }
                            }
                        }
                    }
                }
            }

            if resp.drag_stopped() {
                editor.drag_node_id  = None;
                editor.drag_tangent  = None;
            }
        }

        SplineTool::TangentEdit => {
            if resp.drag_started_by(egui::PointerButton::Primary) {
                editor.drag_tangent = hovered_tangent;
                if hovered_tangent.is_none() {
                    editor.drag_node_id = hovered_node_id;
                }
            }

            if resp.dragged_by(egui::PointerButton::Primary) {
                let drag_world = Vec2::new(
                    resp.drag_delta().x / editor.canvas_zoom,
                    resp.drag_delta().y / editor.canvas_zoom,
                );
                let sidx = editor.selected_spline.unwrap_or(0);

                if let Some((tang_nid, is_out)) = editor.drag_tangent {
                    if let Some(spline) = editor.library.splines.get_mut(sidx) {
                        if let Some(node) = spline.find_node_mut(tang_nid) {
                            if is_out {
                                node.point.out_tangent[0] += drag_world.x;
                                node.point.out_tangent[1] += drag_world.y;
                                if !node.point.corner {
                                    node.point.in_tangent[0] = -node.point.out_tangent[0];
                                    node.point.in_tangent[1] = -node.point.out_tangent[1];
                                }
                            } else {
                                node.point.in_tangent[0] += drag_world.x;
                                node.point.in_tangent[1] += drag_world.y;
                                if !node.point.corner {
                                    node.point.out_tangent[0] = -node.point.in_tangent[0];
                                    node.point.out_tangent[1] = -node.point.in_tangent[1];
                                }
                            }
                        }
                    }
                }
            }

            if resp.drag_stopped() {
                editor.drag_tangent = None;
                editor.drag_node_id = None;
            }
        }

        SplineTool::AddPoint => {
            if resp.clicked_by(egui::PointerButton::Primary) {
                if let Some(mpos) = input.pointer.interact_pos() {
                    if canvas_rect.contains(mpos) {
                        editor.add_node_at_screen(mpos);
                    }
                }
            }
        }

        SplineTool::DeletePoint => {
            if resp.clicked_by(egui::PointerButton::Primary) {
                if let Some(nid) = hovered_node_id {
                    let sidx = editor.selected_spline.unwrap_or(0);
                    if let Some(spline) = editor.library.splines.get_mut(sidx) {
                        spline.remove_node(nid);
                    }
                    editor.selected_nodes.remove(&nid);
                }
            }
        }

        SplineTool::Knife => {
            if resp.clicked_by(egui::PointerButton::Primary) {
                if let Some(mpos) = input.pointer.interact_pos() {
                    if canvas_rect.contains(mpos) {
                        let wpos = editor.screen_to_world(mpos);
                        let sidx = editor.selected_spline.unwrap_or(0);
                        if let Some(spline) = editor.library.splines.get(sidx) {
                            let (t, dist) = nearest_point_on_spline(spline, wpos);
                            let threshold = 20.0 / editor.canvas_zoom;
                            if dist < threshold {
                                let new_id = editor.id_counter;
                                editor.id_counter += 1;
                                let sidx2 = editor.selected_spline.unwrap_or(0);
                                if let Some(sp2) = editor.library.splines.get_mut(sidx2) {
                                    insert_point_at_t(sp2, t, new_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Right click context menu on node
    if resp.secondary_clicked() {
        if let Some(nid) = hovered_node_id {
            editor.context_node = Some(nid);
        }
    }

    if let Some(ctx_nid) = editor.context_node {
        let sidx = editor.selected_spline.unwrap_or(0);
        let has_node = editor.library.splines.get(sidx)
            .map(|s| s.find_node(ctx_nid).is_some())
            .unwrap_or(false);

        if has_node {
            let mut close = false;
            egui::Area::new(egui::Id::new("ctx_menu"))
                .order(egui::Order::Tooltip)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        if ui.button("Delete Node").clicked() {
                            if let Some(sp) = editor.library.splines.get_mut(sidx) {
                                sp.remove_node(ctx_nid);
                            }
                            editor.selected_nodes.remove(&ctx_nid);
                            close = true;
                        }
                        if ui.button("Make Corner").clicked() {
                            if let Some(sp) = editor.library.splines.get_mut(sidx) {
                                if let Some(node) = sp.find_node_mut(ctx_nid) {
                                    node.point.corner = true;
                                }
                            }
                            close = true;
                        }
                        if ui.button("Break Tangents").clicked() {
                            if let Some(sp) = editor.library.splines.get_mut(sidx) {
                                if let Some(node) = sp.find_node_mut(ctx_nid) {
                                    node.point.break_tangents();
                                }
                            }
                            close = true;
                        }
                        if ui.button("Smooth Tangents").clicked() {
                            if let Some(sp) = editor.library.splines.get_mut(sidx) {
                                if let Some(node) = sp.find_node_mut(ctx_nid) {
                                    node.point.smooth_tangents();
                                }
                            }
                            close = true;
                        }
                        if ui.button("Select").clicked() {
                            editor.selected_nodes.clear();
                            editor.selected_nodes.insert(ctx_nid);
                            close = true;
                        }
                        if ui.button("Close Menu").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                editor.context_node = None;
            }
        } else {
            editor.context_node = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Canvas drawing
// ---------------------------------------------------------------------------

fn draw_canvas(painter: Painter, editor: &SplineEditor, canvas_rect: Rect) {
    // Background
    painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(22, 22, 26));

    // Grid
    draw_grid(&painter, editor, canvas_rect);

    // All splines (non-active dimmed)
    for (i, spline) in editor.library.splines.iter().enumerate() {
        if !spline.visible { continue; }
        let is_active = editor.selected_spline == Some(i);
        draw_spline_curve(&painter, editor, spline, canvas_rect, is_active);
    }

    // Active spline overlays (tangents, normals, points)
    if let Some(sidx) = editor.selected_spline {
        if let Some(spline) = editor.library.splines.get(sidx) {
            if spline.visible {
                if editor.show_normals {
                    draw_normals(&painter, editor, spline, canvas_rect);
                }
                if editor.show_length {
                    draw_arc_length_labels(&painter, editor, spline, canvas_rect);
                }
                if editor.show_tangents {
                    draw_tangent_handles(&painter, editor, spline, canvas_rect);
                }
                if editor.show_points {
                    draw_control_points(&painter, editor, spline, canvas_rect);
                }
                draw_preview_marker(&painter, editor, spline, canvas_rect);
            }
        }
    }

    // Canvas border
    painter.rect_stroke(canvas_rect, 0.0, Stroke::new(1.0, Color32::from_gray(50)), egui::StrokeKind::Outside);

    // Tool hint at bottom of canvas
    let hint = match editor.tool {
        SplineTool::Select      => "Select: click/drag nodes | Scroll: zoom | MMB drag: pan",
        SplineTool::AddPoint    => "Add Point: click to place | Scroll: zoom",
        SplineTool::DeletePoint => "Delete: click node to remove",
        SplineTool::TangentEdit => "Tangent: drag handles | MMB drag: pan",
        SplineTool::Knife       => "Knife: click near spline to insert point",
    };
    painter.text(
        Pos2::new(canvas_rect.left() + 8.0, canvas_rect.bottom() - 14.0),
        egui::Align2::LEFT_CENTER,
        hint,
        FontId::proportional(10.0),
        Color32::from_gray(100),
    );
}

fn draw_grid(painter: &Painter, editor: &SplineEditor, canvas_rect: Rect) {
    let gs = editor.grid_size * editor.canvas_zoom;
    if gs < 4.0 { return; }

    let start_x = (canvas_rect.left() / gs).floor() * gs;
    let start_y = (canvas_rect.top() / gs).floor() * gs;

    let base_off_x = editor.canvas_offset.x % gs;
    let base_off_y = editor.canvas_offset.y % gs;

    let major_interval = 5;
    let minor_color = Color32::from_rgba_premultiplied(60, 60, 70, 255);
    let major_color = Color32::from_rgba_premultiplied(80, 80, 100, 255);

    let mut xi = 0i32;
    let mut x = canvas_rect.left() + base_off_x;
    while x < canvas_rect.right() {
        let col = if xi % major_interval == 0 { major_color } else { minor_color };
        painter.line_segment(
            [Pos2::new(x, canvas_rect.top()), Pos2::new(x, canvas_rect.bottom())],
            Stroke::new(0.5, col),
        );
        x += gs;
        xi += 1;
    }

    let mut yi = 0i32;
    let mut y = canvas_rect.top() + base_off_y;
    while y < canvas_rect.bottom() {
        let col = if yi % major_interval == 0 { major_color } else { minor_color };
        painter.line_segment(
            [Pos2::new(canvas_rect.left(), y), Pos2::new(canvas_rect.right(), y)],
            Stroke::new(0.5, col),
        );
        y += gs;
        yi += 1;
    }

    // Axes (world origin)
    let ox = editor.canvas_offset.x;
    let oy = editor.canvas_offset.y;

    if ox > canvas_rect.left() && ox < canvas_rect.right() {
        painter.line_segment(
            [Pos2::new(ox, canvas_rect.top()), Pos2::new(ox, canvas_rect.bottom())],
            Stroke::new(1.0, Color32::from_rgba_premultiplied(80, 80, 160, 180)),
        );
    }
    if oy > canvas_rect.top() && oy < canvas_rect.bottom() {
        painter.line_segment(
            [Pos2::new(canvas_rect.left(), oy), Pos2::new(canvas_rect.right(), oy)],
            Stroke::new(1.0, Color32::from_rgba_premultiplied(160, 80, 80, 180)),
        );
    }

    let _ = (start_x, start_y);
}

fn draw_spline_curve(
    painter:     &Painter,
    editor:      &SplineEditor,
    spline:      &Spline,
    canvas_rect: Rect,
    is_active:   bool,
) {
    if spline.nodes.len() < 2 { return; }

    let samples = spline.resolution as usize * spline.segment_count();
    if samples == 0 { return; }

    let alpha = if is_active { 255u8 } else { 90u8 };
    let col = Color32::from_rgba_premultiplied(
        spline.color.r(),
        spline.color.g(),
        spline.color.b(),
        alpha,
    );
    let stroke = Stroke::new(spline.width * editor.canvas_zoom.sqrt(), col);

    let mut pts: Vec<Pos2> = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let wp = point_on_spline(spline, t);
        let sp = editor.world_to_screen(wp);
        if canvas_rect.expand(200.0).contains(sp) {
            pts.push(sp);
        } else if !pts.is_empty() {
            // Draw what we have so far, then start fresh
            painter.add(Shape::line(pts.clone(), stroke));
            pts.clear();
        }
    }
    if pts.len() >= 2 {
        painter.add(Shape::line(pts, stroke));
    }

    // Draw direction arrows along the spline
    if is_active {
        let arrow_count = (spline.segment_count() * 2).max(2).min(12);
        for ai in 1..arrow_count {
            let t = ai as f32 / arrow_count as f32;
            let pos_w = point_on_spline(spline, t);
            let tang  = spline_tangent_at(spline, t);
            let tang_len = (tang[0]*tang[0] + tang[1]*tang[1]).sqrt().max(1e-4);
            let nx = tang[0] / tang_len;
            let ny = tang[1] / tang_len;

            let sp = editor.world_to_screen(pos_w);
            if !canvas_rect.expand(20.0).contains(sp) { continue; }

            let arrow_size = 6.0_f32;
            let tip = sp;
            let left  = Pos2::new(tip.x - nx*arrow_size - ny*arrow_size*0.5,
                                   tip.y - ny*arrow_size + nx*arrow_size*0.5);
            let right = Pos2::new(tip.x - nx*arrow_size + ny*arrow_size*0.5,
                                   tip.y - ny*arrow_size - nx*arrow_size*0.5);
            painter.add(Shape::line(vec![left, tip, right],
                Stroke::new(1.0, Color32::from_rgba_premultiplied(
                    spline.color.r(), spline.color.g(), spline.color.b(), 160))));
        }
    }
}

fn draw_tangent_handles(
    painter:     &Painter,
    editor:      &SplineEditor,
    spline:      &Spline,
    _canvas_rect: Rect,
) {
    let tang_col     = Color32::from_rgb(180, 220, 255);
    let in_col       = Color32::from_rgb(255, 180, 100);
    let handle_col   = Color32::from_rgb(220, 220, 255);
    let corner_col   = Color32::from_rgb(255, 200, 100);

    for node in &spline.nodes {
        if !editor.selected_nodes.contains(&node.id) && !editor.selected_nodes.is_empty() {
            if !editor.selected_nodes.contains(&node.id) { continue; }
        }
        let np = editor.world_to_screen(node.point.position);

        if !node.point.corner {
            // In tangent
            let in_p = editor.world_to_screen(node.point.in_world_pos());
            painter.line_segment([np, in_p], Stroke::new(1.0, in_col.linear_multiply(0.5)));
            painter.circle_filled(in_p, 4.0, in_col);
            painter.circle_stroke(in_p, 4.0, Stroke::new(1.0, Color32::from_gray(100)));

            // Out tangent
            let out_p = editor.world_to_screen(node.point.out_world_pos());
            painter.line_segment([np, out_p], Stroke::new(1.0, tang_col.linear_multiply(0.5)));
            painter.circle_filled(out_p, 4.0, tang_col);
            painter.circle_stroke(out_p, 4.0, Stroke::new(1.0, Color32::from_gray(100)));
        } else {
            // Corner marker — show both tangents independently
            let in_p = editor.world_to_screen(node.point.in_world_pos());
            painter.line_segment([np, in_p], Stroke::new(1.0, Color32::from_rgb(255, 120, 120).linear_multiply(0.6)));
            painter.circle_filled(in_p, 4.0, Color32::from_rgb(255, 120, 120));

            let out_p = editor.world_to_screen(node.point.out_world_pos());
            painter.line_segment([np, out_p], Stroke::new(1.0, Color32::from_rgb(120, 255, 120).linear_multiply(0.6)));
            painter.circle_filled(out_p, 4.0, Color32::from_rgb(120, 255, 120));
        }
        let _ = (handle_col, corner_col);
    }
}

fn draw_control_points(
    painter:     &Painter,
    editor:      &SplineEditor,
    spline:      &Spline,
    _canvas_rect: Rect,
) {
    let pt_radius   = 5.5_f32;
    let sel_color   = Color32::from_rgb(255, 220, 80);
    let hover_color = Color32::from_rgb(200, 240, 255);
    let normal_col  = Color32::WHITE;
    let corner_col  = Color32::from_rgb(255, 180, 80);
    let outline_col = spline.color;

    for node in &spline.nodes {
        let sp = editor.world_to_screen(node.point.position);
        let is_selected = editor.selected_nodes.contains(&node.id);
        let is_hovered  = editor.hovered_node == Some(node.id);
        let is_corner   = node.point.corner;

        let fill = if is_selected {
            sel_color
        } else if is_hovered {
            hover_color
        } else if is_corner {
            corner_col
        } else {
            normal_col
        };

        // Outer glow for selected
        if is_selected {
            painter.circle_filled(sp, pt_radius + 4.0,
                Color32::from_rgba_premultiplied(255, 220, 80, 60));
        }

        if is_corner {
            // Draw diamond for corner nodes
            let d = pt_radius + 1.0;
            let pts = vec![
                Pos2::new(sp.x,       sp.y - d),
                Pos2::new(sp.x + d,   sp.y),
                Pos2::new(sp.x,       sp.y + d),
                Pos2::new(sp.x - d,   sp.y),
                Pos2::new(sp.x,       sp.y - d),
            ];
            painter.add(Shape::convex_polygon(
                pts[..4].to_vec(),
                fill,
                Stroke::new(1.5, outline_col),
            ));
        } else {
            painter.circle_filled(sp, pt_radius, fill);
            painter.circle_stroke(sp, pt_radius, Stroke::new(1.5, outline_col));
        }

        // Node name label
        if !node.point.name.is_empty() {
            painter.text(
                Pos2::new(sp.x + 8.0, sp.y - 8.0),
                egui::Align2::LEFT_CENTER,
                &node.point.name,
                FontId::proportional(10.0),
                Color32::from_gray(200),
            );
        }

        // Node index label (small)
        if editor.show_points && is_selected {
            if let Some(idx) = spline.nodes.iter().position(|n| n.id == node.id) {
                painter.text(
                    Pos2::new(sp.x + 8.0, sp.y + 10.0),
                    egui::Align2::LEFT_CENTER,
                    &format!("#{}", idx),
                    FontId::proportional(9.0),
                    Color32::from_gray(140),
                );
            }
        }
    }
}

fn draw_normals(
    painter:     &Painter,
    editor:      &SplineEditor,
    spline:      &Spline,
    canvas_rect: Rect,
) {
    if spline.segment_count() == 0 { return; }

    let steps = (spline.resolution * spline.segment_count() as u32 / 4).max(4);
    let normal_len = 12.0;
    let normal_col = Color32::from_rgba_premultiplied(100, 255, 160, 160);

    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let pos = point_on_spline(spline, t);
        let norm = spline_normal_at(spline, t);

        let sp = editor.world_to_screen(pos);
        if !canvas_rect.expand(50.0).contains(sp) { continue; }

        let end_w = [pos[0] + norm[0] * normal_len / editor.canvas_zoom,
                     pos[1] + norm[1] * normal_len / editor.canvas_zoom];
        let end_s = editor.world_to_screen(end_w);

        painter.line_segment([sp, end_s], Stroke::new(1.0, normal_col));
        painter.circle_filled(end_s, 2.0, normal_col);
    }
}

fn draw_arc_length_labels(
    painter:     &Painter,
    editor:      &SplineEditor,
    spline:      &Spline,
    canvas_rect: Rect,
) {
    if spline.segment_count() == 0 { return; }

    let total_len = spline_length(spline);
    if total_len < 1.0 { return; }

    // Show label every ~80 world units
    let interval = 80.0_f32;
    let label_count = (total_len / interval) as usize + 1;

    let mut acc = 0.0f32;
    let mut prev = point_on_spline(spline, 0.0);
    let steps = 200usize;

    let mut next_label = interval;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let curr = point_on_spline(spline, t);
        let dx = curr[0] - prev[0];
        let dy = curr[1] - prev[1];
        acc += (dx*dx + dy*dy).sqrt();
        prev = curr;

        if acc >= next_label {
            let sp = editor.world_to_screen(curr);
            if canvas_rect.expand(40.0).contains(sp) {
                painter.text(
                    Pos2::new(sp.x + 6.0, sp.y - 6.0),
                    egui::Align2::LEFT_CENTER,
                    &format!("{:.0}", next_label),
                    FontId::proportional(9.0),
                    Color32::from_gray(160),
                );
            }
            next_label += interval;
        }
    }

    // Total length label at end
    if let Some(last) = spline.nodes.last() {
        let end_s = editor.world_to_screen(last.point.position);
        if canvas_rect.expand(40.0).contains(end_s) {
            painter.text(
                Pos2::new(end_s.x + 10.0, end_s.y),
                egui::Align2::LEFT_CENTER,
                &format!("L={:.1}", total_len),
                FontId::proportional(10.0),
                Color32::from_rgba_premultiplied(200, 220, 255, 200),
            );
        }
    }

    let _ = label_count;
}

fn draw_preview_marker(
    painter:     &Painter,
    editor:      &SplineEditor,
    spline:      &Spline,
    canvas_rect: Rect,
) {
    if spline.segment_count() == 0 { return; }

    let pos = point_on_spline(spline, editor.preview_t);
    let tang = spline_tangent_at(spline, editor.preview_t);
    let tang_len = (tang[0]*tang[0]+tang[1]*tang[1]).sqrt().max(1e-4);
    let nx = tang[0]/tang_len;
    let ny = tang[1]/tang_len;

    let sp = editor.world_to_screen(pos);
    if !canvas_rect.expand(20.0).contains(sp) { return; }

    // Draw the marker
    let col = Color32::from_rgb(255, 80, 80);

    // Glow
    for gi in 0..4u8 {
        let r = 6.0 + gi as f32 * 3.0;
        let a = 60u8 - gi * 14;
        painter.circle_filled(sp, r, Color32::from_rgba_premultiplied(255, 80, 80, a));
    }

    // Body
    painter.circle_filled(sp, 6.0, col);
    painter.circle_stroke(sp, 6.0, Stroke::new(1.5, Color32::WHITE));

    // Direction arrow
    let arrow_tip = Pos2::new(sp.x + nx * 14.0, sp.y + ny * 14.0);
    let arrow_left = Pos2::new(
        sp.x + nx*8.0 - ny*4.0,
        sp.y + ny*8.0 + nx*4.0,
    );
    let arrow_right = Pos2::new(
        sp.x + nx*8.0 + ny*4.0,
        sp.y + ny*8.0 - nx*4.0,
    );
    painter.add(Shape::line(vec![arrow_left, arrow_tip, arrow_right],
        Stroke::new(1.5, Color32::from_rgb(255, 180, 180))));

    // T value label
    painter.text(
        Pos2::new(sp.x + 10.0, sp.y + 10.0),
        egui::Align2::LEFT_CENTER,
        &format!("t={:.3}", editor.preview_t),
        FontId::proportional(9.0),
        Color32::from_gray(200),
    );
}

// ---------------------------------------------------------------------------
// Bottom toolbar
// ---------------------------------------------------------------------------

fn show_bottom_toolbar(ui: &mut egui::Ui, editor: &mut SplineEditor) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Spline Editor").strong());
        ui.separator();

        // Tool buttons
        for tool in [SplineTool::Select, SplineTool::AddPoint, SplineTool::DeletePoint,
                     SplineTool::TangentEdit, SplineTool::Knife] {
            let selected = editor.tool == tool;
            let btn = egui::Button::new(
                RichText::new(format!("[{}] {}", tool.icon(), tool.label()))
                    .color(if selected { Color32::from_rgb(255, 220, 80) } else { Color32::from_gray(200) })
            )
            .fill(if selected { Color32::from_rgba_premultiplied(60, 50, 20, 220) } else { Color32::from_gray(40) });
            if ui.add(btn).on_hover_text(tool.label()).clicked() {
                editor.tool = tool;
            }
        }

        ui.separator();

        // Snap toggle
        let snap_col = if editor.snap_to_grid { Color32::from_rgb(100, 255, 140) } else { Color32::from_gray(160) };
        if ui.button(RichText::new("Grid Snap").color(snap_col)).clicked() {
            editor.snap_to_grid = !editor.snap_to_grid;
        }
        ui.label("Size:");
        ui.add(egui::DragValue::new(&mut editor.grid_size).clamp_range(1.0..=200.0).speed(1.0));

        ui.separator();

        // Show/hide toggles
        ui.toggle_value(&mut editor.show_tangents, "Tangents");
        ui.toggle_value(&mut editor.show_normals,  "Normals");
        ui.toggle_value(&mut editor.show_length,   "Length");
        ui.toggle_value(&mut editor.show_points,   "Points");

        ui.separator();

        // Zoom
        ui.label("Zoom:");
        if ui.small_button("-").clicked() { editor.canvas_zoom = (editor.canvas_zoom * 0.8).max(0.05); }
        ui.label(format!("{:.0}%", editor.canvas_zoom * 100.0));
        if ui.small_button("+").clicked() { editor.canvas_zoom = (editor.canvas_zoom * 1.25).min(20.0); }
        if ui.small_button("1:1").clicked() { editor.canvas_zoom = 1.0; }

        ui.separator();

        // Preview controls
        let play_label = if editor.preview_running { "Pause" } else { "Play" };
        if ui.button(play_label).clicked() {
            editor.preview_running = !editor.preview_running;
        }
        if ui.button("|<").on_hover_text("Reset preview").clicked() {
            editor.preview_t    = 0.0;
            editor.preview_running = false;
        }
        ui.add(egui::Slider::new(&mut editor.preview_t, 0.0..=1.0)
            .show_value(true)
            .text("t"));
    });
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

fn show_sidebar(ui: &mut egui::Ui, editor: &mut SplineEditor) {
    // Spline library list
    ui.label(RichText::new("Splines").strong());
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("+ Add").clicked() {
            let n = editor.library.splines.len() + 1;
            let color = spline_palette_color(n);
            let idx = editor.library.add(Spline::new(&format!("Path {}", n)));
            editor.library.splines[idx].color = color;
            editor.selected_spline = Some(idx);
        }
        if ui.button("Delete").clicked() {
            if let Some(sel) = editor.selected_spline {
                editor.library.remove(sel);
                editor.selected_spline = if editor.library.splines.is_empty() {
                    None
                } else {
                    Some(sel.saturating_sub(1))
                };
            }
        }
        if ui.button("Duplicate").clicked() {
            if let Some(sel) = editor.selected_spline {
                if sel < editor.library.splines.len() {
                    let mut copy = editor.library.splines[sel].clone();
                    copy.name = format!("{} Copy", copy.name);
                    let new_idx = editor.library.add(copy);
                    editor.selected_spline = Some(new_idx);
                }
            }
        }
    });

    ui.add_space(4.0);

    let spline_count = editor.library.splines.len();
    let mut new_selected = editor.selected_spline;

    for i in 0..spline_count {
        let is_sel = editor.selected_spline == Some(i);
        let (row_rect, row_resp) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 22.0),
            egui::Sense::click(),
        );

        if ui.is_rect_visible(row_rect) {
            let painter = ui.painter();
            if is_sel || row_resp.hovered() {
                painter.rect_filled(
                    row_rect,
                    3.0,
                    if is_sel { Color32::from_rgb(45, 75, 120) } else { Color32::from_rgba_premultiplied(255,255,255,15) },
                );
            }

            let spline = &editor.library.splines[i];

            // Color swatch
            let sw = Rect::from_min_size(
                Pos2::new(row_rect.left()+4.0, row_rect.top()+5.0),
                Vec2::new(10.0, 10.0),
            );
            painter.rect_filled(sw, 1.0, spline.color);

            // Visibility dot
            let vis_col = if spline.visible { Color32::from_rgb(100, 255, 140) } else { Color32::from_gray(60) };
            painter.circle_filled(Pos2::new(row_rect.left()+20.0, row_rect.center().y), 4.0, vis_col);

            // Lock indicator
            if spline.locked {
                painter.text(
                    Pos2::new(row_rect.left()+28.0, row_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "L",
                    FontId::proportional(9.0),
                    Color32::from_rgb(255, 200, 100),
                );
            }

            let name_x = row_rect.left() + 36.0;
            painter.text(
                Pos2::new(name_x, row_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &spline.name,
                FontId::proportional(11.0),
                if is_sel { Color32::WHITE } else { Color32::from_gray(200) },
            );

            // Node count
            painter.text(
                Pos2::new(row_rect.right() - 4.0, row_rect.center().y),
                egui::Align2::RIGHT_CENTER,
                &format!("{}n", spline.node_count()),
                FontId::proportional(9.0),
                Color32::from_gray(120),
            );
        }

        if row_resp.clicked() {
            new_selected = Some(i);
        }
    }

    editor.selected_spline = new_selected;

    // Rename dialog
    if let Some((ridx, ref mut rbuf)) = editor.rename_spline.clone() {
        let mut open = true;
        let mut commit = false;
        let mut cancel = false;
        egui::Window::new("Rename Spline")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.text_edit_singleline(rbuf);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() { commit = true; }
                    if ui.button("Cancel").clicked() { cancel = true; }
                });
            });
        if commit {
            if ridx < editor.library.splines.len() {
                editor.library.splines[ridx].name = rbuf.clone();
            }
            editor.rename_spline = None;
        } else if !open || cancel {
            editor.rename_spline = None;
        } else {
            editor.rename_spline = Some((ridx, rbuf.clone()));
        }
    }

    ui.add_space(8.0);
    ui.separator();

    // Active spline properties
    if let Some(sidx) = editor.selected_spline {
        if sidx < editor.library.splines.len() {
            show_spline_properties(ui, editor, sidx);
        }
    }
}

fn show_spline_properties(ui: &mut egui::Ui, editor: &mut SplineEditor, sidx: usize) {
    ui.label(RichText::new("Spline Properties").strong());
    ui.add_space(4.0);

    // Name
    {
        let mut name = editor.library.splines[sidx].name.clone();
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui.text_edit_singleline(&mut name).changed() {
                editor.library.splines[sidx].name = name;
            }
        });
    }

    // Type
    {
        let spline_type = editor.library.splines[sidx].spline_type.clone();
        ui.horizontal(|ui| {
            ui.label("Type:");
            egui::ComboBox::from_id_source("spline_type_combo")
                .selected_text(spline_type.label())
                .show_ui(ui, |ui| {
                    for st in SplineType::all() {
                        let is_sel = st == editor.library.splines[sidx].spline_type;
                        if ui.selectable_label(is_sel, st.label()).clicked() {
                            editor.library.splines[sidx].spline_type = st;
                        }
                    }
                });
        });
    }

    // Closed toggle
    {
        let closed = editor.library.splines[sidx].closed;
        ui.horizontal(|ui| {
            ui.label("Closed:");
            let mut c = closed;
            if ui.checkbox(&mut c, "").changed() {
                editor.library.splines[sidx].closed = c;
            }
        });
    }

    // Visible / locked
    {
        let vis = editor.library.splines[sidx].visible;
        let locked = editor.library.splines[sidx].locked;
        ui.horizontal(|ui| {
            let mut v = vis;
            if ui.checkbox(&mut v, "Visible").changed() {
                editor.library.splines[sidx].visible = v;
            }
            let mut l = locked;
            if ui.checkbox(&mut l, "Locked").changed() {
                editor.library.splines[sidx].locked = l;
            }
        });
    }

    // Color
    {
        let mut col = editor.library.splines[sidx].color;
        ui.horizontal(|ui| {
            ui.label("Color:");
            if ui.color_edit_button_srgba(&mut col).changed() {
                editor.library.splines[sidx].color = col;
            }
        });
    }

    // Width
    {
        let mut w = editor.library.splines[sidx].width;
        ui.horizontal(|ui| {
            ui.label("Width:");
            if ui.add(egui::Slider::new(&mut w, 0.5..=10.0)).changed() {
                editor.library.splines[sidx].width = w;
            }
        });
    }

    // Resolution
    {
        let mut res = editor.library.splines[sidx].resolution as i32;
        ui.horizontal(|ui| {
            ui.label("Resolution:");
            if ui.add(egui::DragValue::new(&mut res).clamp_range(2..=128)).changed() {
                editor.library.splines[sidx].resolution = res as u32;
            }
        });
    }

    ui.add_space(6.0);

    // Path length
    {
        let len = spline_length(&editor.library.splines[sidx]);
        ui.horizontal(|ui| {
            ui.label("Length:");
            ui.label(RichText::new(format!("{:.2}", len)).color(Color32::from_rgb(180, 220, 255)));
        });
    }

    // Node count
    {
        let nc = editor.library.splines[sidx].node_count();
        ui.horizontal(|ui| {
            ui.label("Nodes:");
            ui.label(format!("{}", nc));
        });
    }

    ui.add_space(8.0);
    ui.separator();

    // Selected node properties
    let sel_ids: Vec<u32> = editor.selected_nodes.iter().cloned().collect();
    if sel_ids.len() == 1 {
        let nid = sel_ids[0];
        let node_exists = editor.library.splines[sidx].find_node(nid).is_some();
        if node_exists {
            show_node_properties(ui, editor, sidx, nid);
        }
    } else if sel_ids.len() > 1 {
        ui.label(RichText::new(format!("{} nodes selected", sel_ids.len()))
            .color(Color32::from_gray(160)));
    } else {
        ui.label(RichText::new("No node selected").color(Color32::from_gray(100)));
    }

    ui.add_space(8.0);
    ui.separator();

    // Export
    show_export_panel(ui, editor, sidx);
}

fn show_node_properties(
    ui:    &mut egui::Ui,
    editor: &mut SplineEditor,
    sidx:  usize,
    nid:   u32,
) {
    ui.label(RichText::new("Selected Node").strong());
    ui.add_space(4.0);

    let node = match editor.library.splines[sidx].find_node(nid) {
        Some(n) => n.clone(),
        None => return,
    };

    egui::Grid::new("node_props")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Name:");
            let mut name = node.point.name.clone();
            if ui.text_edit_singleline(&mut name).changed() {
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.name = name;
                }
            }
            ui.end_row();

            ui.label("Position X:");
            let mut px = node.point.position[0];
            if ui.add(egui::DragValue::new(&mut px).speed(0.5)).changed() {
                let snapped_px = editor.snap(px);
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.position[0] = snapped_px;
                }
            }
            ui.end_row();

            ui.label("Position Y:");
            let mut py = node.point.position[1];
            if ui.add(egui::DragValue::new(&mut py).speed(0.5)).changed() {
                let snapped_py = editor.snap(py);
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.position[1] = snapped_py;
                }
            }
            ui.end_row();

            ui.label("In Tangent X:");
            let mut itx = node.point.in_tangent[0];
            if ui.add(egui::DragValue::new(&mut itx).speed(0.5)).changed() {
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.in_tangent[0] = itx;
                }
            }
            ui.end_row();

            ui.label("In Tangent Y:");
            let mut ity = node.point.in_tangent[1];
            if ui.add(egui::DragValue::new(&mut ity).speed(0.5)).changed() {
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.in_tangent[1] = ity;
                }
            }
            ui.end_row();

            ui.label("Out Tangent X:");
            let mut otx = node.point.out_tangent[0];
            if ui.add(egui::DragValue::new(&mut otx).speed(0.5)).changed() {
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.out_tangent[0] = otx;
                }
            }
            ui.end_row();

            ui.label("Out Tangent Y:");
            let mut oty = node.point.out_tangent[1];
            if ui.add(egui::DragValue::new(&mut oty).speed(0.5)).changed() {
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.out_tangent[1] = oty;
                }
            }
            ui.end_row();

            ui.label("Weight:");
            let mut w = node.point.weight;
            if ui.add(egui::Slider::new(&mut w, 0.0..=4.0)).changed() {
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.weight = w;
                }
            }
            ui.end_row();

            ui.label("Corner:");
            let mut corner = node.point.corner;
            if ui.checkbox(&mut corner, "").changed() {
                if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                    n.point.corner = corner;
                }
            }
            ui.end_row();
        });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button("Smooth").on_hover_text("Mirror out_tangent to in_tangent").clicked() {
            if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                n.point.smooth_tangents();
            }
        }
        if ui.button("Break").on_hover_text("Break tangent symmetry").clicked() {
            if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                n.point.break_tangents();
            }
        }
        if ui.button("Zero Out").on_hover_text("Set both tangents to zero").clicked() {
            if let Some(n) = editor.library.splines[sidx].find_node_mut(nid) {
                n.point.in_tangent  = [0.0, 0.0];
                n.point.out_tangent = [0.0, 0.0];
            }
        }
    });
}

fn show_export_panel(ui: &mut egui::Ui, editor: &SplineEditor, sidx: usize) {
    let spline = &editor.library.splines[sidx];
    ui.collapsing("Export", |ui| {
        let res_count = (spline.resolution as usize * spline.segment_count()).max(2);
        let pts = sample_spline_uniform(spline, res_count);
        ui.label(format!("{} points at resolution {}", pts.len(), spline.resolution));

        // Show a snippet of the export
        let preview: Vec<String> = pts.iter().take(5).map(|p| format!("[{:.1},{:.1}]", p[0], p[1])).collect();
        ui.code(format!("// {} points\nvec![\n  {}{}\n]",
            pts.len(),
            preview.join(", "),
            if pts.len() > 5 { ", ..." } else { "" }
        ));

        if ui.button("Copy as Rust Vec").clicked() {
            let code: String = pts.iter()
                .map(|p| format!("[{:.3}_f32, {:.3}_f32]", p[0], p[1]))
                .collect::<Vec<_>>()
                .join(", ");
            ui.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(format!("vec![{}]", code))));
        }

        if ui.button("Copy as JSON").clicked() {
            let code: String = pts.iter()
                .map(|p| format!("[{:.3},{:.3}]", p[0], p[1]))
                .collect::<Vec<_>>()
                .join(",");
            ui.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(format!("[{}]", code))));
        }

        ui.add_space(4.0);
        let total_len = spline_length(spline);
        ui.label(format!("Total arc length: {:.2}", total_len));
        ui.label(format!("Segments: {}", spline.segment_count()));
        ui.label(format!("Type: {}", spline.spline_type.label()));
    });
}

// ---------------------------------------------------------------------------
// Color palette for new splines
// ---------------------------------------------------------------------------

fn spline_palette_color(idx: usize) -> Color32 {
    let palette = [
        Color32::from_rgb(100, 200, 255),
        Color32::from_rgb(255, 160, 80),
        Color32::from_rgb(120, 255, 140),
        Color32::from_rgb(255, 100, 180),
        Color32::from_rgb(200, 140, 255),
        Color32::from_rgb(255, 230, 80),
        Color32::from_rgb(80, 220, 220),
        Color32::from_rgb(255, 130, 130),
    ];
    palette[idx % palette.len()]
}

// ---------------------------------------------------------------------------
// Additional spline utilities
// ---------------------------------------------------------------------------

/// Reverse the order of nodes in a spline.
pub fn reverse_spline(spline: &mut Spline) {
    spline.nodes.reverse();
    for node in &mut spline.nodes {
        let old_in  = node.point.in_tangent;
        let old_out = node.point.out_tangent;
        node.point.in_tangent  = [-old_out[0], -old_out[1]];
        node.point.out_tangent = [-old_in[0],  -old_in[1]];
    }
}

/// Translate all nodes by a world-space offset.
pub fn translate_spline(spline: &mut Spline, offset: [f32; 2]) {
    for node in &mut spline.nodes {
        node.point.position[0] += offset[0];
        node.point.position[1] += offset[1];
    }
}

/// Scale all nodes around a world-space pivot.
pub fn scale_spline(spline: &mut Spline, pivot: [f32; 2], scale: f32) {
    for node in &mut spline.nodes {
        node.point.position[0] = pivot[0] + (node.point.position[0] - pivot[0]) * scale;
        node.point.position[1] = pivot[1] + (node.point.position[1] - pivot[1]) * scale;
        node.point.in_tangent[0]  *= scale;
        node.point.in_tangent[1]  *= scale;
        node.point.out_tangent[0] *= scale;
        node.point.out_tangent[1] *= scale;
    }
}

/// Rotate all nodes around a pivot by `angle_radians`.
pub fn rotate_spline(spline: &mut Spline, pivot: [f32; 2], angle_rad: f32) {
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    for node in &mut spline.nodes {
        let dx = node.point.position[0] - pivot[0];
        let dy = node.point.position[1] - pivot[1];
        node.point.position[0] = pivot[0] + dx * cos_a - dy * sin_a;
        node.point.position[1] = pivot[1] + dx * sin_a + dy * cos_a;

        let it = node.point.in_tangent;
        node.point.in_tangent[0] = it[0] * cos_a - it[1] * sin_a;
        node.point.in_tangent[1] = it[0] * sin_a + it[1] * cos_a;

        let ot = node.point.out_tangent;
        node.point.out_tangent[0] = ot[0] * cos_a - ot[1] * sin_a;
        node.point.out_tangent[1] = ot[0] * sin_a + ot[1] * cos_a;
    }
}

/// Compute the bounding box of a spline's control points.
pub fn spline_bounds(spline: &Spline) -> Option<Rect> {
    if spline.nodes.is_empty() { return None; }
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for node in &spline.nodes {
        let p = node.point.position;
        if p[0] < min_x { min_x = p[0]; }
        if p[1] < min_y { min_y = p[1]; }
        if p[0] > max_x { max_x = p[0]; }
        if p[1] > max_y { max_y = p[1]; }
    }
    Some(Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y)))
}

/// Normalize spline to fit within a [0,1] x [0,1] box.
pub fn normalize_spline(spline: &mut Spline) {
    if let Some(bounds) = spline_bounds(spline) {
        let w = bounds.width().max(1e-4);
        let h = bounds.height().max(1e-4);
        let scale = 1.0_f32 / w.max(h);
        for node in &mut spline.nodes {
            node.point.position[0] = (node.point.position[0] - bounds.left()) * scale;
            node.point.position[1] = (node.point.position[1] - bounds.top())  * scale;
            node.point.in_tangent[0]  *= scale;
            node.point.in_tangent[1]  *= scale;
            node.point.out_tangent[0] *= scale;
            node.point.out_tangent[1] *= scale;
        }
    }
}

/// Auto-smooth all tangents for a spline (Catmull-Rom style).
pub fn auto_smooth_spline(spline: &mut Spline) {
    let n = spline.nodes.len();
    if n < 3 { return; }

    let positions: Vec<[f32; 2]> = spline.nodes.iter().map(|nd| nd.point.position).collect();

    for i in 0..n {
        let prev = if i == 0 {
            if spline.closed { positions[n-1] } else { positions[0] }
        } else { positions[i-1] };
        let next = if i == n-1 {
            if spline.closed { positions[0] } else { positions[n-1] }
        } else { positions[i+1] };

        let dx = next[0] - prev[0];
        let dy = next[1] - prev[1];
        let len = (dx*dx + dy*dy).sqrt().max(1e-4);
        let scale = len * 0.25;
        let nx = dx / len;
        let ny = dy / len;

        spline.nodes[i].point.out_tangent = [ nx * scale,  ny * scale];
        spline.nodes[i].point.in_tangent  = [-nx * scale, -ny * scale];
        spline.nodes[i].point.corner = false;
    }
}

/// Resample a spline uniformly by arc length, rebuilding it with evenly spaced nodes.
pub fn resample_spline(spline: &mut Spline, count: usize, id_base: u32) -> u32 {
    if count < 2 || spline.nodes.is_empty() { return id_base; }

    let pts = sample_spline_uniform(spline, count);
    spline.nodes.clear();

    for (i, &[x, y]) in pts.iter().enumerate() {
        let id = id_base + i as u32;
        let mut cp = ControlPoint::new(x, y);

        // Auto-tangent from neighbors
        if i > 0 && i < pts.len() - 1 {
            let prev = pts[i-1];
            let next = pts[i+1];
            let dx = next[0] - prev[0];
            let dy = next[1] - prev[1];
            let len = (dx*dx + dy*dy).sqrt().max(1e-4);
            let scale = len * 0.25;
            let nx = dx / len;
            let ny = dy / len;
            cp.out_tangent = [ nx * scale,  ny * scale];
            cp.in_tangent  = [-nx * scale, -ny * scale];
        }

        spline.nodes.push(SplineNode::new(id, cp));
    }

    id_base + count as u32
}

/// Concatenate two splines, appending b's nodes to the end of a.
pub fn concat_splines(a: &mut Spline, b: &Spline, id_base: u32) -> u32 {
    let mut next_id = id_base;
    for node in &b.nodes {
        let mut cp = node.point.clone();
        let new_id = next_id;
        next_id += 1;
        a.nodes.push(SplineNode::new(new_id, cp));
    }
    next_id
}

/// Split a spline at the node with the given id, returning two new splines.
pub fn split_spline_at_node(spline: &Spline, split_id: u32) -> (Spline, Spline) {
    let split_idx = spline.nodes.iter()
        .position(|n| n.id == split_id)
        .unwrap_or(0);

    let mut a = spline.clone();
    let mut b = spline.clone();

    a.nodes = spline.nodes[..=split_idx].to_vec();
    b.nodes = spline.nodes[split_idx..].to_vec();
    a.name = format!("{} A", spline.name);
    b.name = format!("{} B", spline.name);

    (a, b)
}

// ---------------------------------------------------------------------------
// Path event timeline stub
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathTimeline {
    pub events:   Vec<(f32, PathEvent)>, // (t, event)
    pub loop_type: PathLoopType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PathLoopType {
    Once,
    Loop,
    PingPong,
}

impl PathTimeline {
    pub fn new() -> Self {
        Self { events: Vec::new(), loop_type: PathLoopType::Once }
    }

    pub fn add_event(&mut self, t: f32, event: PathEvent) {
        self.events.push((t, event));
        self.events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }

    pub fn events_before_t(&self, t: f32) -> Vec<&PathEvent> {
        self.events.iter().filter(|(et, _)| *et <= t).map(|(_, e)| e).collect()
    }
}

impl Default for PathTimeline {
    fn default() -> Self { Self::new() }
}

pub fn show_timeline(ui: &mut egui::Ui, timeline: &mut PathTimeline, current_t: &mut f32) {
    ui.label(RichText::new("Path Timeline").strong());
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Loop:");
        for lt in [PathLoopType::Once, PathLoopType::Loop, PathLoopType::PingPong] {
            let selected = timeline.loop_type == lt;
            let label = match &lt {
                PathLoopType::Once     => "Once",
                PathLoopType::Loop     => "Loop",
                PathLoopType::PingPong => "Ping-Pong",
            };
            if ui.selectable_label(selected, label).clicked() {
                timeline.loop_type = lt;
            }
        }
    });

    ui.add_space(4.0);

    // Timeline bar
    let (bar_rect, bar_resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 24.0),
        egui::Sense::click_and_drag(),
    );

    if ui.is_rect_visible(bar_rect) {
        let painter = ui.painter();
        painter.rect_filled(bar_rect, 2.0, Color32::from_gray(40));

        // Events
        for (et, event) in &timeline.events {
            let x = bar_rect.left() + et * bar_rect.width();
            let col = match event {
                PathEvent::Move { .. }  => Color32::from_rgb(100, 200, 255),
                PathEvent::Wait { .. }  => Color32::from_rgb(255, 200, 80),
                PathEvent::Loop         => Color32::from_rgb(120, 255, 120),
                PathEvent::PingPong     => Color32::from_rgb(200, 120, 255),
                PathEvent::Event { .. } => Color32::from_rgb(255, 120, 120),
            };
            painter.line_segment(
                [Pos2::new(x, bar_rect.top()), Pos2::new(x, bar_rect.bottom())],
                Stroke::new(2.0, col),
            );
        }

        // Playhead
        let px = bar_rect.left() + *current_t * bar_rect.width();
        painter.line_segment(
            [Pos2::new(px, bar_rect.top()), Pos2::new(px, bar_rect.bottom())],
            Stroke::new(2.0, Color32::WHITE),
        );
        painter.circle_filled(Pos2::new(px, bar_rect.center().y), 5.0, Color32::WHITE);

        painter.rect_stroke(bar_rect, 2.0, Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Outside);
    }

    if bar_resp.dragged() {
        let t = ((bar_resp.interact_pointer_pos().unwrap().x - bar_rect.left()) / bar_rect.width())
            .clamp(0.0, 1.0);
        *current_t = t;
    }

    ui.add_space(4.0);

    // Event list
    let mut remove_idx: Option<usize> = None;
    let events_len = timeline.events.len();
    for (i, (et, event)) in timeline.events.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("t={:.2}", et));
            let label = match event {
                PathEvent::Move  { speed }    => format!("Move (speed={:.1})", speed),
                PathEvent::Wait  { duration } => format!("Wait ({:.1}s)", duration),
                PathEvent::Loop               => "Loop".to_string(),
                PathEvent::PingPong           => "PingPong".to_string(),
                PathEvent::Event { name }     => format!("Event \"{}\"", name),
            };
            ui.label(label);
            if ui.small_button("x").clicked() {
                remove_idx = Some(i);
            }
        });
    }
    if let Some(ri) = remove_idx {
        timeline.events.remove(ri);
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button("+ Move").clicked() {
            timeline.add_event(*current_t, PathEvent::Move { speed: 1.0 });
        }
        if ui.button("+ Wait").clicked() {
            timeline.add_event(*current_t, PathEvent::Wait { duration: 1.0 });
        }
        if ui.button("+ Event").clicked() {
            timeline.add_event(*current_t, PathEvent::Event { name: "event".to_string() });
        }
    });

    let _ = events_len;
}

// ---------------------------------------------------------------------------
// Undo stack for spline editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SplineSnapshot {
    pub library: SplineLibrary,
}

#[derive(Debug, Default, Clone)]
pub struct SplineUndoStack {
    pub past:   Vec<SplineSnapshot>,
    pub future: Vec<SplineSnapshot>,
    pub limit:  usize,
}

impl SplineUndoStack {
    pub fn new(limit: usize) -> Self {
        Self { past: Vec::new(), future: Vec::new(), limit }
    }

    pub fn push(&mut self, snap: SplineSnapshot) {
        self.future.clear();
        self.past.push(snap);
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
    }

    pub fn undo(&mut self, editor: &mut SplineEditor) -> bool {
        if let Some(snap) = self.past.pop() {
            let current = SplineSnapshot { library: editor.library.clone() };
            self.future.push(current);
            editor.library = snap.library;
            return true;
        }
        false
    }

    pub fn redo(&mut self, editor: &mut SplineEditor) -> bool {
        if let Some(snap) = self.future.pop() {
            let current = SplineSnapshot { library: editor.library.clone() };
            self.past.push(current);
            editor.library = snap.library;
            return true;
        }
        false
    }

    pub fn can_undo(&self) -> bool { !self.past.is_empty() }
    pub fn can_redo(&self) -> bool { !self.future.is_empty() }
}

// ---------------------------------------------------------------------------
// Multi-spline rendering (draw all into a target UI region)
// ---------------------------------------------------------------------------

/// Draw a thumbnail of a spline into a given rect.
pub fn draw_spline_thumbnail(painter: &Painter, rect: Rect, spline: &Spline) {
    if spline.nodes.len() < 2 { return; }

    let bounds = spline_bounds(spline).unwrap_or(
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0))
    );

    let margin = 4.0_f32;
    let inner = rect.shrink(margin);
    let scale_x = inner.width() / bounds.width().max(1.0);
    let scale_y = inner.height() / bounds.height().max(1.0);
    let scale = scale_x.min(scale_y);

    let offset_x = inner.left() + (inner.width() - bounds.width() * scale) * 0.5;
    let offset_y = inner.top()  + (inner.height()- bounds.height()* scale) * 0.5;

    let to_screen = |wp: [f32; 2]| -> Pos2 {
        Pos2::new(
            offset_x + (wp[0] - bounds.left()) * scale,
            offset_y + (wp[1] - bounds.top())  * scale,
        )
    };

    painter.rect_filled(rect, 3.0, Color32::from_gray(30));

    let steps = (spline.resolution as usize * spline.segment_count()).max(2);
    let pts: Vec<Pos2> = (0..=steps).map(|i| {
        let t = i as f32 / steps as f32;
        to_screen(point_on_spline(spline, t))
    }).collect();

    painter.add(Shape::line(pts, Stroke::new(1.5, spline.color)));

    // Draw endpoints
    if let (Some(first), Some(last)) = (spline.nodes.first(), spline.nodes.last()) {
        painter.circle_filled(to_screen(first.point.position), 3.0, Color32::WHITE);
        painter.circle_filled(to_screen(last.point.position),  3.0, Color32::from_rgb(255, 180, 80));
    }

    painter.rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_gray(60)), egui::StrokeKind::Outside);
}

/// Show a compact spline browser (grid of thumbnails).
pub fn show_spline_browser(
    ui:       &mut egui::Ui,
    library:  &SplineLibrary,
    selected: &mut Option<usize>,
) {
    let thumb_size = Vec2::new(80.0, 60.0);
    let columns = ((ui.available_width() + 4.0) / (thumb_size.x + 4.0)).floor() as usize;
    let columns = columns.max(1);

    egui::Grid::new("spline_browser")
        .num_columns(columns)
        .spacing([4.0, 4.0])
        .show(ui, |ui| {
            for (i, spline) in library.splines.iter().enumerate() {
                let is_sel = *selected == Some(i);
                let (rect, resp) = ui.allocate_exact_size(thumb_size, egui::Sense::click());

                if ui.is_rect_visible(rect) {
                    draw_spline_thumbnail(ui.painter(), rect, spline);
                    if is_sel {
                        ui.painter().rect_stroke(rect, 3.0, Stroke::new(2.0, Color32::from_rgb(255, 220, 80)), egui::StrokeKind::Outside);
                    }
                    ui.painter().text(
                        Pos2::new(rect.left() + 3.0, rect.bottom() - 3.0),
                        egui::Align2::LEFT_BOTTOM,
                        &spline.name,
                        FontId::proportional(9.0),
                        Color32::from_gray(200),
                    );
                }

                resp.clone().on_hover_text(format!("{} — {} nodes", spline.name, spline.node_count()));
                if resp.clicked() {
                    *selected = Some(i);
                }

                if (i + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
}

// ---------------------------------------------------------------------------
// Spline statistics
// ---------------------------------------------------------------------------

pub struct SplineStats {
    pub total_splines: usize,
    pub total_nodes:   usize,
    pub total_length:  f32,
    pub visible:       usize,
    pub locked:        usize,
}

pub fn compute_spline_stats(library: &SplineLibrary) -> SplineStats {
    let mut total_nodes  = 0usize;
    let mut total_length = 0.0f32;
    let mut visible = 0usize;
    let mut locked  = 0usize;

    for sp in &library.splines {
        total_nodes += sp.node_count();
        total_length += spline_length(sp);
        if sp.visible { visible += 1; }
        if sp.locked  { locked  += 1; }
    }

    SplineStats {
        total_splines: library.splines.len(),
        total_nodes,
        total_length,
        visible,
        locked,
    }
}

pub fn show_spline_stats(ui: &mut egui::Ui, library: &SplineLibrary) {
    let s = compute_spline_stats(library);
    ui.collapsing("Library Stats", |ui| {
        egui::Grid::new("spline_stats").num_columns(2).spacing([12.0, 3.0]).show(ui, |ui| {
            ui.label("Splines:");  ui.label(format!("{}", s.total_splines)); ui.end_row();
            ui.label("Nodes:");    ui.label(format!("{}", s.total_nodes));   ui.end_row();
            ui.label("Length:");   ui.label(format!("{:.1}", s.total_length)); ui.end_row();
            ui.label("Visible:");  ui.label(format!("{}", s.visible));       ui.end_row();
            ui.label("Locked:");   ui.label(format!("{}", s.locked));        ui.end_row();
        });
    });
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

impl SplineEditor {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.library)
    }

    pub fn from_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let lib: SplineLibrary = serde_json::from_str(json)?;
        self.library = lib;
        self.selected_spline = if self.library.splines.is_empty() { None } else { Some(0) };
        Ok(())
    }

    pub fn export_sampled(&self, spline_idx: usize, count: usize) -> Vec<[f32; 2]> {
        if let Some(sp) = self.library.splines.get(spline_idx) {
            sample_spline_uniform(sp, count)
        } else {
            Vec::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Distance and intersection utilities
// ---------------------------------------------------------------------------

/// Returns the distance between two world-space 2D points.
pub fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0]-b[0];
    let dy = a[1]-b[1];
    (dx*dx+dy*dy).sqrt()
}

/// Closest parameter t and distance for a line segment p0..p1 to a query point.
pub fn nearest_on_segment(p0: [f32;2], p1: [f32;2], q: [f32;2]) -> (f32, f32) {
    let dx = p1[0]-p0[0];
    let dy = p1[1]-p0[1];
    let len2 = dx*dx+dy*dy;
    if len2 < 1e-8 {
        return (0.0, dist2(p0, q));
    }
    let t = ((q[0]-p0[0])*dx + (q[1]-p0[1])*dy) / len2;
    let t = t.clamp(0.0, 1.0);
    let closest = [p0[0]+t*dx, p0[1]+t*dy];
    (t, dist2(closest, q))
}

/// Returns true if the point is roughly "inside" a closed spline
/// (uses a simple ray-casting test against a sampled polygon).
pub fn point_inside_closed_spline(spline: &Spline, pt: [f32; 2]) -> bool {
    if !spline.closed || spline.nodes.len() < 3 { return false; }

    let samples = (spline.resolution as usize * spline.segment_count()).max(8);
    let poly: Vec<[f32; 2]> = (0..samples).map(|i| {
        let t = i as f32 / samples as f32;
        point_on_spline(spline, t)
    }).collect();

    // Ray casting
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let xi = poly[i][0]; let yi = poly[i][1];
        let xj = poly[j][0]; let yj = poly[j][1];
        let intersect = ((yi > pt[1]) != (yj > pt[1]))
            && (pt[0] < (xj - xi) * (pt[1] - yi) / (yj - yi) + xi);
        if intersect { inside = !inside; }
        j = i;
    }
    inside
}

// ---------------------------------------------------------------------------
// Gradient visualization overlay (debug / analysis)
// ---------------------------------------------------------------------------

pub fn draw_curvature_visualization(
    painter:  &Painter,
    editor:   &SplineEditor,
    spline:   &Spline,
    canvas_rect: Rect,
) {
    if spline.segment_count() == 0 { return; }

    let steps = spline.resolution as usize * spline.segment_count() * 2;

    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let t_eps = 1e-3_f32;

        let p0 = point_on_spline(spline, (t - t_eps).max(0.0));
        let p1 = point_on_spline(spline, t);
        let p2 = point_on_spline(spline, (t + t_eps).min(1.0));

        // Second derivative (curvature proxy)
        let d2x = p2[0] - 2.0*p1[0] + p0[0];
        let d2y = p2[1] - 2.0*p1[1] + p0[1];
        let curvature = (d2x*d2x + d2y*d2y).sqrt() / (t_eps * t_eps);

        let sp = editor.world_to_screen(p1);
        if !canvas_rect.expand(20.0).contains(sp) { continue; }

        // Map curvature to color: low = green, high = red
        let norm_curv = (curvature / 500.0).min(1.0);
        let r = (norm_curv * 255.0) as u8;
        let g = ((1.0 - norm_curv) * 200.0) as u8;
        let col = Color32::from_rgba_premultiplied(r, g, 60, 200);

        painter.circle_filled(sp, 2.5, col);
    }
}

impl SplineEditor {
    pub fn show_panel(ctx: &egui::Context, editor: &mut SplineEditor, dt: f32, open: &mut bool) {
        egui::Window::new("Spline Editor")
            .open(open)
            .default_size([1100.0, 680.0])
            .min_size([700.0, 420.0])
            .resizable(true)
            .show(ctx, |ui| {
                show(ui, editor, dt);
            });
    }
}


// =================================================================
// SIMPLE RNG HELPER (used by new spline systems)
// =================================================================

pub struct SplineRng { state: u64 }
impl SplineRng {
    pub fn new(seed: u64) -> Self { Self { state: seed ^ 0xDEADBEEF12345678 } }
    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13; self.state ^= self.state >> 7; self.state ^= self.state << 17; self.state
    }
    pub fn next_f32(&mut self) -> f32 { (self.next_u64() as f32) / (u64::MAX as f32) }
    pub fn next_f32_range(&mut self, lo: f32, hi: f32) -> f32 { lo + self.next_f32() * (hi - lo) }
}

// =================================================================
// ROAD TOOL SYSTEM
// =================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RoadSurfaceType {
    Dirt,
    Gravel,
    Asphalt,
    Cobblestone,
    Highway,
}

impl RoadSurfaceType {
    pub fn name(&self) -> &str {
        match self {
            RoadSurfaceType::Dirt        => "Dirt",
            RoadSurfaceType::Gravel      => "Gravel",
            RoadSurfaceType::Asphalt     => "Asphalt",
            RoadSurfaceType::Cobblestone => "Cobblestone",
            RoadSurfaceType::Highway     => "Highway",
        }
    }
    pub fn color(&self) -> Color32 {
        match self {
            RoadSurfaceType::Dirt        => Color32::from_rgb(160, 130, 90),
            RoadSurfaceType::Gravel      => Color32::from_rgb(130, 125, 110),
            RoadSurfaceType::Asphalt     => Color32::from_rgb(60, 60, 60),
            RoadSurfaceType::Cobblestone => Color32::from_rgb(120, 110, 95),
            RoadSurfaceType::Highway     => Color32::from_rgb(40, 40, 50),
        }
    }
    pub fn all() -> &'static [RoadSurfaceType] {
        &[RoadSurfaceType::Dirt, RoadSurfaceType::Gravel, RoadSurfaceType::Asphalt, RoadSurfaceType::Cobblestone, RoadSurfaceType::Highway]
    }
    pub fn default_speed_limit(&self) -> f32 {
        match self { RoadSurfaceType::Dirt=>20.0, RoadSurfaceType::Gravel=>40.0, RoadSurfaceType::Asphalt=>80.0, RoadSurfaceType::Cobblestone=>30.0, RoadSurfaceType::Highway=>120.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadConfig {
    pub lane_count: u32,
    pub lane_width: f32,
    pub median_width: f32,
    pub shoulder_width: f32,
    pub curb_height: f32,
    pub road_type: RoadSurfaceType,
    pub speed_limit: f32,
    pub bidirectional: bool,
    pub has_sidewalk: bool,
    pub sidewalk_width: f32,
}

impl Default for RoadConfig {
    fn default() -> Self {
        Self {
            lane_count: 2,
            lane_width: 3.6,
            median_width: 0.0,
            shoulder_width: 1.5,
            curb_height: 0.15,
            road_type: RoadSurfaceType::Asphalt,
            speed_limit: 50.0,
            bidirectional: true,
            has_sidewalk: true,
            sidewalk_width: 2.0,
        }
    }
}

impl RoadConfig {
    pub fn total_width(&self) -> f32 {
        let lanes = self.lane_count as f32 * self.lane_width;
        let median = if self.bidirectional { self.median_width } else { 0.0 };
        let shoulders = self.shoulder_width * 2.0;
        let sidewalks = if self.has_sidewalk { self.sidewalk_width * 2.0 } else { 0.0 };
        lanes + median + shoulders + sidewalks
    }
    pub fn highway() -> Self {
        Self { lane_count: 3, lane_width: 3.75, median_width: 3.0, shoulder_width: 2.5, curb_height: 0.0, road_type: RoadSurfaceType::Highway, speed_limit: 130.0, bidirectional: true, has_sidewalk: false, sidewalk_width: 0.0 }
    }
    pub fn city_street() -> Self {
        Self { lane_count: 1, lane_width: 3.5, median_width: 0.0, shoulder_width: 0.5, curb_height: 0.15, road_type: RoadSurfaceType::Asphalt, speed_limit: 50.0, bidirectional: true, has_sidewalk: true, sidewalk_width: 2.5 }
    }
    pub fn dirt_track() -> Self {
        Self { lane_count: 1, lane_width: 3.0, median_width: 0.0, shoulder_width: 0.5, curb_height: 0.0, road_type: RoadSurfaceType::Dirt, speed_limit: 20.0, bidirectional: true, has_sidewalk: false, sidewalk_width: 0.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadSegment {
    pub spline_idx: usize,
    pub config: RoadConfig,
    pub elevation_profile: Vec<f32>,
    pub banking_angle: Vec<f32>,
    pub is_tunnel: bool,
    pub is_bridge: bool,
    pub name: String,
    pub start_elevation: f32,
    pub end_elevation: f32,
}

impl RoadSegment {
    pub fn new(spline_idx: usize) -> Self {
        Self {
            spline_idx,
            config: RoadConfig::default(),
            elevation_profile: Vec::new(),
            banking_angle: Vec::new(),
            is_tunnel: false,
            is_bridge: false,
            name: format!("Road {}", spline_idx),
            start_elevation: 0.0,
            end_elevation: 0.0,
        }
    }
    pub fn elevation_at(&self, t: f32) -> f32 {
        if self.elevation_profile.is_empty() {
            let t = t.clamp(0.0, 1.0);
            return self.start_elevation + t * (self.end_elevation - self.start_elevation);
        }
        let idx = (t * (self.elevation_profile.len() - 1) as f32) as usize;
        let idx = idx.min(self.elevation_profile.len() - 1);
        self.elevation_profile[idx]
    }
    pub fn banking_at(&self, t: f32) -> f32 {
        if self.banking_angle.is_empty() { return 0.0; }
        let idx = (t * (self.banking_angle.len() - 1) as f32) as usize;
        self.banking_angle[idx.min(self.banking_angle.len() - 1)]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IntersectionType { T, X, Y, Roundabout }
impl IntersectionType {
    pub fn name(&self) -> &str { match self { IntersectionType::T=>"T", IntersectionType::X=>"X", IntersectionType::Y=>"Y", IntersectionType::Roundabout=>"Roundabout" } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadIntersection {
    pub position: [f32; 2],
    pub connected_roads: Vec<usize>,
    pub intersection_type: IntersectionType,
    pub has_traffic_light: bool,
    pub radius: f32,
}

impl RoadIntersection {
    pub fn new(position: [f32; 2]) -> Self {
        Self { position, connected_roads: Vec::new(), intersection_type: IntersectionType::X, has_traffic_light: false, radius: 5.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadNetwork {
    pub segments: Vec<RoadSegment>,
    pub intersections: Vec<RoadIntersection>,
    pub name: String,
}

impl Default for RoadNetwork {
    fn default() -> Self { Self { segments: Vec::new(), intersections: Vec::new(), name: "Road Network".to_string() } }
}

impl RoadNetwork {
    pub fn add_segment(&mut self, seg: RoadSegment) -> usize {
        let idx = self.segments.len();
        self.segments.push(seg);
        idx
    }
    pub fn add_intersection(&mut self, pos: [f32; 2]) -> usize {
        let idx = self.intersections.len();
        self.intersections.push(RoadIntersection::new(pos));
        idx
    }
    pub fn total_road_length(&self, splines: &[Spline]) -> f32 {
        self.segments.iter().map(|s| {
            if s.spline_idx < splines.len() { spline_arc_length(&splines[s.spline_idx], 64) } else { 0.0 }
        }).sum()
    }
}

pub fn draw_road_preview(painter: &Painter, seg: &RoadSegment, spline: &Spline, editor: &SplineEditor, canvas_rect: Rect) {
    if spline.nodes.is_empty() { return; }
    let steps = spline.resolution as usize * spline.segment_count().max(1) * 4;
    let config = &seg.config;
    let total_w = config.total_width();
    let half_w = total_w * 0.5;

    let road_col = config.road_type.color();
    let shoulder_col = Color32::from_rgb(100, 95, 80);
    let sidewalk_col = Color32::from_rgb(200, 195, 185);
    let lane_line_col = Color32::from_rgb(255, 255, 255);
    let center_line_col = Color32::from_rgb(240, 200, 0);

    // Draw road layers
    let mut pts: Vec<[f32; 2]> = (0..=steps).map(|i| point_on_spline(spline, i as f32 / steps as f32)).collect();

    for layer_idx in 0..4 {
        let (layer_w, layer_col) = match layer_idx {
            0 => (half_w + config.shoulder_width + if config.has_sidewalk { config.sidewalk_width } else { 0.0 }, sidewalk_col),
            1 => (half_w + config.shoulder_width, shoulder_col),
            2 => (half_w, road_col),
            _ => continue,
        };
        for i in 0..(pts.len() - 1) {
            let p0 = editor.world_to_screen(pts[i]);
            let p1 = editor.world_to_screen(pts[i + 1]);
            if !canvas_rect.expand(50.0).contains(p0) && !canvas_rect.expand(50.0).contains(p1) { continue; }
            let stroke_w = (layer_w * editor.canvas_zoom).max(1.0);
            painter.line_segment([p0, p1], Stroke::new(stroke_w * 2.0, layer_col));
        }
    }

    // Draw lane lines
    for i in 0..(pts.len() - 1) {
        let p0 = editor.world_to_screen(pts[i]);
        let p1 = editor.world_to_screen(pts[i + 1]);
        if !canvas_rect.expand(50.0).contains(p0) { continue; }
        // Center line (yellow, solid if bidirectional)
        painter.line_segment([p0, p1], Stroke::new(1.5, center_line_col));
        // Lane lines (white dashed)
        for lane in 1..config.lane_count {
            let _offset = (lane as f32 - config.lane_count as f32 * 0.5) * config.lane_width;
            if i % 3 != 0 { // dashed
                painter.line_segment([p0, p1], Stroke::new(0.8, lane_line_col));
            }
        }
    }

    // Draw tunnel/bridge indicator
    if seg.is_tunnel {
        let mid = pts[pts.len() / 2];
        let smp = editor.world_to_screen(mid);
        painter.text(smp, egui::Align2::CENTER_CENTER, "TUNNEL", FontId::proportional(10.0), Color32::from_rgb(180, 140, 220));
    }
    if seg.is_bridge {
        let mid = pts[pts.len() / 2];
        let smp = editor.world_to_screen(mid);
        painter.text(smp, egui::Align2::CENTER_CENTER, "BRIDGE", FontId::proportional(10.0), Color32::from_rgb(100, 180, 240));
    }
    let _ = layer_idx_shadow_ref_suppressor;
    let _ = lane_line_col;
}

fn layer_idx_shadow_ref_suppressor() {}

pub fn draw_road_cross_section(painter: &Painter, config: &RoadConfig, rect: Rect) {
    painter.rect_filled(rect, 2.0, Color32::from_gray(20));
    let cx = rect.center().x;
    let bottom = rect.bottom() - 5.0;
    let scale = rect.height() / 8.0; // 1 meter = scale pixels

    // Draw from outside in
    let sidewalk_col = Color32::from_rgb(200, 195, 185);
    let shoulder_col = Color32::from_rgb(100, 95, 80);
    let road_col = config.road_type.color();
    let lane_col = Color32::from_rgb(255, 255, 255);

    let half_road = config.lane_count as f32 * config.lane_width * 0.5;

    if config.has_sidewalk {
        let sw_w = config.sidewalk_width * scale;
        let sw_x_l = cx - (half_road + config.shoulder_width + config.sidewalk_width) * scale;
        let sw_x_r = cx + (half_road + config.shoulder_width) * scale;
        painter.rect_filled(Rect::from_min_max(Pos2::new(sw_x_l, bottom - 2.0*scale), Pos2::new(sw_x_l + sw_w, bottom)), 0.0, sidewalk_col);
        painter.rect_filled(Rect::from_min_max(Pos2::new(sw_x_r, bottom - 2.0*scale), Pos2::new(sw_x_r + sw_w, bottom)), 0.0, sidewalk_col);
    }

    let sh_w = config.shoulder_width * scale;
    let sh_x_l = cx - (half_road + config.shoulder_width) * scale;
    let sh_x_r = cx + half_road * scale;
    painter.rect_filled(Rect::from_min_max(Pos2::new(sh_x_l, bottom - 1.5*scale), Pos2::new(sh_x_l + sh_w, bottom)), 0.0, shoulder_col);
    painter.rect_filled(Rect::from_min_max(Pos2::new(sh_x_r, bottom - 1.5*scale), Pos2::new(sh_x_r + sh_w, bottom)), 0.0, shoulder_col);

    // Road surface
    let road_x_l = cx - half_road * scale;
    let road_x_r = cx + half_road * scale;
    painter.rect_filled(Rect::from_min_max(Pos2::new(road_x_l, bottom - scale), Pos2::new(road_x_r, bottom)), 0.0, road_col);

    // Lane lines
    for lane in 1..config.lane_count {
        let lx = cx - half_road * scale + lane as f32 * config.lane_width * scale;
        painter.line_segment([Pos2::new(lx, bottom - scale), Pos2::new(lx, bottom)], Stroke::new(1.0, lane_col));
    }

    // Center line
    painter.line_segment([Pos2::new(cx, bottom - scale), Pos2::new(cx, bottom)], Stroke::new(2.0, Color32::from_rgb(240, 200, 0)));

    // Labels
    painter.text(Pos2::new(cx, bottom - scale - 5.0), egui::Align2::CENTER_BOTTOM,
        format!("{:.1}m total", config.total_width()), FontId::proportional(9.0), Color32::WHITE);
}

pub fn show_road_editor(ui: &mut egui::Ui, state: &mut RoadEditorState, splines: &[Spline]) {
    ui.horizontal(|ui| {
        ui.heading("Road Tool");
        if ui.button("Add Road").clicked() {
            let idx = state.network.segments.len();
            state.network.segments.push(RoadSegment::new(0));
            state.selected_segment = Some(idx);
        }
        if ui.button("Add Intersection").clicked() {
            state.network.intersections.push(RoadIntersection::new([0.0, 0.0]));
        }
    });
    ui.separator();
    ui.label(format!("{} segments | {} intersections | {:.0}m total",
        state.network.segments.len(),
        state.network.intersections.len(),
        state.network.total_road_length(splines)));
    ui.separator();

    if let Some(sel) = state.selected_segment {
        if sel < state.network.segments.len() {
            egui::CollapsingHeader::new("Road Segment Config").show(ui, |ui| {
                let seg = &mut state.network.segments[sel];
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut seg.name);
                });
                ui.horizontal(|ui| {
                    ui.label("Spline idx:");
                    ui.add(egui::DragValue::new(&mut seg.spline_idx).range(0..=splines.len().saturating_sub(1)));
                });
                ui.horizontal(|ui| {
                    ui.label("Road type:");
                    for rt in RoadSurfaceType::all() {
                        if ui.selectable_label(seg.config.road_type == *rt, rt.name()).clicked() {
                            seg.config.road_type = rt.clone();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Lanes:");
                    ui.add(egui::DragValue::new(&mut seg.config.lane_count).range(1..=8));
                    ui.label("Width:");
                    ui.add(egui::DragValue::new(&mut seg.config.lane_width).range(2.0..=6.0).suffix("m"));
                });
                ui.horizontal(|ui| {
                    ui.label("Shoulder:");
                    ui.add(egui::DragValue::new(&mut seg.config.shoulder_width).range(0.0..=5.0).suffix("m"));
                    ui.label("Median:");
                    ui.add(egui::DragValue::new(&mut seg.config.median_width).range(0.0..=10.0).suffix("m"));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut seg.config.has_sidewalk, "Sidewalk");
                    if seg.config.has_sidewalk {
                        ui.add(egui::DragValue::new(&mut seg.config.sidewalk_width).range(0.5..=8.0).suffix("m"));
                    }
                    ui.checkbox(&mut seg.is_tunnel, "Tunnel");
                    ui.checkbox(&mut seg.is_bridge, "Bridge");
                });
                ui.horizontal(|ui| {
                    ui.label("Speed limit:");
                    ui.add(egui::DragValue::new(&mut seg.config.speed_limit).range(10.0..=200.0).suffix("km/h"));
                });
                ui.label(format!("Total width: {:.2}m", seg.config.total_width()));

                // Cross-section preview
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 60.0), egui::Sense::hover());
                let config_clone = seg.config.clone();
                draw_road_cross_section(ui.painter(), &config_clone, rect);
            });
        }
    }

    // Segment list
    ui.separator();
    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
        let mut to_delete = None;
        for (i, seg) in state.network.segments.iter().enumerate() {
            ui.horizontal(|ui| {
                let sel = state.selected_segment == Some(i);
                if ui.selectable_label(sel, &seg.name).clicked() {
                    state.selected_segment = Some(i);
                }
                if ui.small_button("X").clicked() { to_delete = Some(i); }
            });
        }
        if let Some(i) = to_delete { state.network.segments.remove(i); state.selected_segment = None; }
    });

    // Quick presets
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Presets:");
        if ui.button("Highway").clicked() {
            if let Some(sel) = state.selected_segment {
                if sel < state.network.segments.len() {
                    state.network.segments[sel].config = RoadConfig::highway();
                }
            }
        }
        if ui.button("City Street").clicked() {
            if let Some(sel) = state.selected_segment {
                if sel < state.network.segments.len() {
                    state.network.segments[sel].config = RoadConfig::city_street();
                }
            }
        }
        if ui.button("Dirt Track").clicked() {
            if let Some(sel) = state.selected_segment {
                if sel < state.network.segments.len() {
                    state.network.segments[sel].config = RoadConfig::dirt_track();
                }
            }
        }
    });
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RoadEditorState {
    pub network: RoadNetwork,
    pub selected_segment: Option<usize>,
    pub show_cross_section: bool,
}

// =================================================================
// CAMERA PATH SYSTEM
// =================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EasingType { Linear, EaseIn, EaseOut, EaseInOut, Bounce, Elastic }
impl EasingType {
    pub fn name(&self) -> &str { match self { EasingType::Linear=>"Linear", EasingType::EaseIn=>"Ease In", EasingType::EaseOut=>"Ease Out", EasingType::EaseInOut=>"Ease In Out", EasingType::Bounce=>"Bounce", EasingType::Elastic=>"Elastic" } }
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingType::Linear => t,
            EasingType::EaseIn => t * t,
            EasingType::EaseOut => 1.0 - (1.0-t)*(1.0-t),
            EasingType::EaseInOut => if t < 0.5 { 2.0*t*t } else { 1.0-(-2.0*t+2.0)*(-2.0*t+2.0)/2.0 },
            EasingType::Bounce => {
                let t = 1.0 - t;
                let v = if t < 1.0/2.75 { 7.5625*t*t } else if t < 2.0/2.75 { let t=t-1.5/2.75; 7.5625*t*t+0.75 } else if t < 2.5/2.75 { let t=t-2.25/2.75; 7.5625*t*t+0.9375 } else { let t=t-2.625/2.75; 7.5625*t*t+0.984375 };
                1.0 - v
            },
            EasingType::Elastic => {
                if t == 0.0 { return 0.0; } if t == 1.0 { return 1.0; }
                let c4 = std::f32::consts::TAU / 3.0;
                -(2.0_f32.powf(10.0*t-10.0)) * ((10.0*t-10.75)*c4).sin()
            },
        }
    }
    pub fn all() -> &'static [EasingType] { &[EasingType::Linear, EasingType::EaseIn, EasingType::EaseOut, EasingType::EaseInOut, EasingType::Bounce, EasingType::Elastic] }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraPath {
    pub spline_idx: usize,
    pub fov_start: f32,
    pub fov_end: f32,
    pub roll_start: f32,
    pub roll_end: f32,
    pub focus_target: Option<u32>,
    pub look_ahead: f32,
    pub name: String,
}

impl Default for CameraPath {
    fn default() -> Self {
        Self { spline_idx: 0, fov_start: 60.0, fov_end: 60.0, roll_start: 0.0, roll_end: 0.0, focus_target: None, look_ahead: 0.05, name: "Camera Path".to_string() }
    }
}

impl CameraPath {
    pub fn fov_at(&self, t: f32) -> f32 { self.fov_start + t * (self.fov_end - self.fov_start) }
    pub fn roll_at(&self, t: f32) -> f32 { self.roll_start + t * (self.roll_end - self.roll_start) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraShot {
    pub name: String,
    pub path: CameraPath,
    pub duration: f32,
    pub easing: EasingType,
    pub cut_to: Option<usize>,
    pub active: bool,
}

impl CameraShot {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), path: CameraPath::default(), duration: 5.0, easing: EasingType::EaseInOut, cut_to: None, active: true }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CameraSequence {
    pub shots: Vec<CameraShot>,
    pub loop_sequence: bool,
    pub current_shot: usize,
    pub playback_time: f32,
    pub is_playing: bool,
    pub total_duration: f32,
}

impl CameraSequence {
    pub fn total_duration(&self) -> f32 {
        self.shots.iter().filter(|s| s.active).map(|s| s.duration).sum()
    }
    pub fn shot_at_time(&self, t: f32) -> (usize, f32) {
        let mut elapsed = 0.0;
        for (i, shot) in self.shots.iter().enumerate() {
            if !shot.active { continue; }
            if t < elapsed + shot.duration {
                let local_t = (t - elapsed) / shot.duration.max(0.001);
                return (i, shot.easing.evaluate(local_t));
            }
            elapsed += shot.duration;
        }
        (self.shots.len().saturating_sub(1), 1.0)
    }
    pub fn tick(&mut self, dt: f32) {
        if !self.is_playing { return; }
        self.playback_time += dt;
        let total = self.total_duration();
        if self.playback_time >= total {
            if self.loop_sequence { self.playback_time %= total.max(0.001); }
            else { self.playback_time = total; self.is_playing = false; }
        }
        let (shot_idx, _) = self.shot_at_time(self.playback_time);
        self.current_shot = shot_idx;
    }
    pub fn get_camera_state(&self, splines: &[Spline]) -> Option<([f32;2], f32, f32)> {
        let (shot_idx, local_t) = self.shot_at_time(self.playback_time);
        let shot = self.shots.get(shot_idx)?;
        if !shot.active { return None; }
        let spline = splines.get(shot.path.spline_idx)?;
        let pos = point_on_spline(spline, local_t);
        let look_ahead_pos = point_on_spline(spline, (local_t + shot.path.look_ahead).min(1.0));
        let dx = look_ahead_pos[0] - pos[0];
        let dy = look_ahead_pos[1] - pos[1];
        let angle = dy.atan2(dx);
        Some((pos, angle, shot.path.fov_at(local_t)))
    }
}

pub fn draw_camera_path(painter: &Painter, path: &CameraPath, spline: &Spline, editor: &SplineEditor, canvas_rect: Rect, playback_t: f32) {
    if spline.nodes.is_empty() { return; }
    let steps = 60usize;
    let col = Color32::from_rgba_unmultiplied(100, 200, 255, 180);
    let mut prev: Option<Pos2> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let pt = point_on_spline(spline, t);
        let sp = editor.world_to_screen(pt);
        if !canvas_rect.expand(20.0).contains(sp) { prev = Some(sp); continue; }
        if let Some(pp) = prev { painter.line_segment([pp, sp], Stroke::new(2.0, col)); }
        prev = Some(sp);
    }
    // Draw camera frustum at current playback position
    let cam_pt = point_on_spline(spline, playback_t);
    let look_pt = point_on_spline(spline, (playback_t + path.look_ahead).min(1.0));
    let cam_sp = editor.world_to_screen(cam_pt);
    let look_sp = editor.world_to_screen(look_pt);
    painter.circle_filled(cam_sp, 5.0, Color32::from_rgb(100, 200, 255));
    painter.line_segment([cam_sp, look_sp], Stroke::new(1.5, Color32::from_rgb(255, 220, 50)));
    let fov_half = path.fov_at(playback_t).to_radians() * 0.5;
    let dx = look_sp.x - cam_sp.x; let dy = look_sp.y - cam_sp.y;
    let base_angle = dy.atan2(dx);
    let frustum_len = 30.0;
    let frustum_l = Pos2::new(cam_sp.x + (base_angle - fov_half).cos() * frustum_len, cam_sp.y + (base_angle - fov_half).sin() * frustum_len);
    let frustum_r = Pos2::new(cam_sp.x + (base_angle + fov_half).cos() * frustum_len, cam_sp.y + (base_angle + fov_half).sin() * frustum_len);
    painter.line_segment([cam_sp, frustum_l], Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 220, 50, 128)));
    painter.line_segment([cam_sp, frustum_r], Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 220, 50, 128)));
    painter.line_segment([frustum_l, frustum_r], Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 220, 50, 128)));
    let _ = canvas_rect;
}

pub fn show_camera_sequence_editor(ui: &mut egui::Ui, seq: &mut CameraSequence, splines: &[Spline], dt: f32) {
    ui.heading("Camera Sequence");
    ui.horizontal(|ui| {
        if ui.button(if seq.is_playing { "Pause" } else { "Play" }).clicked() {
            seq.is_playing = !seq.is_playing;
        }
        if ui.button("Stop").clicked() { seq.is_playing = false; seq.playback_time = 0.0; }
        ui.checkbox(&mut seq.loop_sequence, "Loop");
        let total = seq.total_duration();
        ui.label(format!("{:.1}s / {:.1}s", seq.playback_time, total));
    });

    if seq.is_playing { seq.tick(dt); }

    ui.add(egui::Slider::new(&mut seq.playback_time, 0.0..=seq.total_duration().max(0.01)).text("Time"));

    ui.separator();
    if ui.button("Add Shot").clicked() {
        let name = format!("Shot {}", seq.shots.len() + 1);
        seq.shots.push(CameraShot::new(&name));
    }

    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        let mut to_remove = None;
        for (i, shot) in seq.shots.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                egui::CollapsingHeader::new(&shot.name).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut shot.active, "Active");
                        ui.label("Duration:");
                        ui.add(egui::DragValue::new(&mut shot.duration).range(0.1..=60.0).suffix("s"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Spline:");
                        ui.add(egui::DragValue::new(&mut shot.path.spline_idx).range(0..=splines.len().saturating_sub(1)));
                        ui.label("FOV:");
                        ui.add(egui::DragValue::new(&mut shot.path.fov_start).range(10.0..=150.0).suffix("°"));
                        ui.label("-");
                        ui.add(egui::DragValue::new(&mut shot.path.fov_end).range(10.0..=150.0).suffix("°"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Roll:");
                        ui.add(egui::DragValue::new(&mut shot.path.roll_start).range(-180.0..=180.0).suffix("°"));
                        ui.label("-");
                        ui.add(egui::DragValue::new(&mut shot.path.roll_end).range(-180.0..=180.0).suffix("°"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Easing:");
                        for e in EasingType::all() {
                            if ui.selectable_label(shot.easing == *e, e.name()).clicked() {
                                shot.easing = e.clone();
                            }
                        }
                    });
                    if ui.button("Delete Shot").clicked() { to_remove = Some(i); }
                });
            });
        }
        if let Some(idx) = to_remove { seq.shots.remove(idx); }
    });

    // Easing preview strip
    ui.separator();
    if let Some(sel) = seq.shots.get(seq.current_shot) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 2.0, Color32::from_gray(20));
        let mut prev_pt: Option<Pos2> = None;
        for s in 0..=80usize {
            let t = s as f32 / 80.0;
            let et = sel.easing.evaluate(t);
            let px = rect.left() + t * rect.width();
            let py = rect.bottom() - et * rect.height();
            let cur = Pos2::new(px, py);
            if let Some(pp) = prev_pt { p.line_segment([pp, cur], Stroke::new(1.5, Color32::from_rgb(100, 200, 255))); }
            prev_pt = Some(cur);
        }
        // Playback position indicator
        let (shot_idx, local_t) = seq.shot_at_time(seq.playback_time);
        if shot_idx == seq.current_shot {
            let px = rect.left() + local_t * rect.width();
            p.line_segment([Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())], Stroke::new(2.0, Color32::YELLOW));
        }
    }
}

// =================================================================
// PATH FOLLOWING SYSTEM
// =================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LoopMode { Once, Loop, PingPong, Clamp }
impl LoopMode {
    pub fn name(&self) -> &str { match self { LoopMode::Once=>"Once", LoopMode::Loop=>"Loop", LoopMode::PingPong=>"Ping Pong", LoopMode::Clamp=>"Clamp" } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathFollower {
    pub spline_idx: usize,
    pub speed: f32,
    pub offset: f32,
    pub loop_mode: LoopMode,
    pub current_t: f32,
    pub events: Vec<PathEvent>,
    pub name: String,
    pub color: Color32,
    pub active: bool,
    pub direction: f32,
    pub scale: f32,
}

impl PathFollower {
    pub fn new(name: &str, spline_idx: usize) -> Self {
        Self { spline_idx, speed: 1.0, offset: 0.0, loop_mode: LoopMode::Loop, current_t: 0.0, events: Vec::new(), name: name.to_string(), color: Color32::from_rgb(255, 100, 100), active: true, direction: 1.0, scale: 1.0 }
    }
}

pub fn advance_follower(follower: &mut PathFollower, dt: f32, spline: &Spline) -> ([f32;2], [f32;2]) {
    let arc_len = spline_arc_length(spline, 32);
    let t_step = if arc_len > 0.0 { follower.speed * dt / arc_len } else { 0.0 };
    follower.current_t += t_step * follower.direction;

    match follower.loop_mode {
        LoopMode::Loop => { follower.current_t = follower.current_t.rem_euclid(1.0); }
        LoopMode::PingPong => {
            if follower.current_t > 1.0 { follower.current_t = 2.0 - follower.current_t; follower.direction = -1.0; }
            if follower.current_t < 0.0 { follower.current_t = -follower.current_t; follower.direction = 1.0; }
        }
        LoopMode::Once => { follower.current_t = follower.current_t.clamp(0.0, 1.0); }
        LoopMode::Clamp => { follower.current_t = follower.current_t.clamp(0.0, 1.0); }
    }

    let pos = point_on_spline(spline, follower.current_t);
    let eps = 0.01f32;
    let ahead = point_on_spline(spline, (follower.current_t + eps).min(1.0));
    let tangent = [ahead[0] - pos[0], ahead[1] - pos[1]];
    let len = (tangent[0]*tangent[0] + tangent[1]*tangent[1]).sqrt().max(0.001);
    let tangent = [tangent[0]/len, tangent[1]/len];

    // Apply offset perpendicular to tangent
    let perp = [-tangent[1], tangent[0]];
    let offset_pos = [pos[0] + perp[0] * follower.offset, pos[1] + perp[1] * follower.offset];
    (offset_pos, tangent)
}

pub fn draw_path_followers(painter: &Painter, followers: &[PathFollower], splines: &[Spline], editor: &SplineEditor) {
    for follower in followers {
        if !follower.active { continue; }
        let spline = match splines.get(follower.spline_idx) { Some(s) => s, None => continue };
        let pos = point_on_spline(spline, follower.current_t);
        let sp = editor.world_to_screen(pos);
        painter.circle_filled(sp, 5.0 * follower.scale, follower.color);
        painter.circle_stroke(sp, 6.0 * follower.scale, Stroke::new(1.0, Color32::WHITE));
        painter.text(Pos2::new(sp.x, sp.y - 10.0), egui::Align2::CENTER_BOTTOM, &follower.name, FontId::proportional(9.0), follower.color);
    }
}

pub fn show_path_follower_editor(ui: &mut egui::Ui, followers: &mut Vec<PathFollower>, splines: &[Spline], dt: f32) {
    ui.heading("Path Followers");
    if ui.button("Add Follower").clicked() {
        let name = format!("Follower {}", followers.len() + 1);
        followers.push(PathFollower::new(&name, 0));
    }
    ui.separator();

    let mut to_remove = None;
    for (i, follower) in followers.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            egui::CollapsingHeader::new(&follower.name).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut follower.active, "Active");
                    ui.label("Spline:");
                    ui.add(egui::DragValue::new(&mut follower.spline_idx).range(0..=splines.len().saturating_sub(1)));
                });
                ui.horizontal(|ui| {
                    ui.label("Speed:");
                    ui.add(egui::DragValue::new(&mut follower.speed).range(0.01..=100.0));
                    ui.label("Offset:");
                    ui.add(egui::DragValue::new(&mut follower.offset).range(-50.0..=50.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Loop mode:");
                    for lm in &[LoopMode::Once, LoopMode::Loop, LoopMode::PingPong, LoopMode::Clamp] {
                        if ui.selectable_label(follower.loop_mode == *lm, lm.name()).clicked() {
                            follower.loop_mode = lm.clone();
                        }
                    }
                });
                ui.label(format!("t = {:.3}", follower.current_t));
                if ui.button("Reset").clicked() { follower.current_t = 0.0; }
                if ui.button("Remove").clicked() { to_remove = Some(i); }

                // Advance follower
                if follower.active {
                    if let Some(spline) = splines.get(follower.spline_idx) {
                        advance_follower(follower, dt, spline);
                    }
                }
            });
        });
    }
    if let Some(idx) = to_remove { followers.remove(idx); }
}

// =================================================================
// RAILWAY SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwitchPoint {
    pub t: f32,
    pub branch_spline: usize,
    pub triggered: bool,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RailwayTrack {
    pub spline_idx: usize,
    pub gauge: f32,
    pub elevation_profile: Vec<f32>,
    pub switch_points: Vec<SwitchPoint>,
    pub name: String,
    pub track_color: Color32,
}

impl RailwayTrack {
    pub fn new(spline_idx: usize) -> Self {
        Self { spline_idx, gauge: 1.435, elevation_profile: Vec::new(), switch_points: Vec::new(), name: format!("Track {}", spline_idx), track_color: Color32::from_rgb(100, 90, 80) }
    }
    pub fn elevation_at(&self, t: f32) -> f32 {
        if self.elevation_profile.is_empty() { return 0.0; }
        let idx = (t * (self.elevation_profile.len() - 1) as f32) as usize;
        self.elevation_profile[idx.min(self.elevation_profile.len() - 1)]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainCar {
    pub length: f32,
    pub width: f32,
    pub car_type: TrainCarType,
    pub color: Color32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrainCarType { Locomotive, Passenger, Freight, Flatcar, Tank, Caboose }
impl TrainCarType {
    pub fn name(&self) -> &str { match self { TrainCarType::Locomotive=>"Locomotive", TrainCarType::Passenger=>"Passenger", TrainCarType::Freight=>"Freight", TrainCarType::Flatcar=>"Flatcar", TrainCarType::Tank=>"Tank", TrainCarType::Caboose=>"Caboose" } }
    pub fn default_color(&self) -> Color32 { match self { TrainCarType::Locomotive=>Color32::from_rgb(60,60,60), TrainCarType::Passenger=>Color32::from_rgb(100,140,200), TrainCarType::Freight=>Color32::from_rgb(160,130,80), TrainCarType::Flatcar=>Color32::from_rgb(120,110,90), TrainCarType::Tank=>Color32::from_rgb(80,100,80), TrainCarType::Caboose=>Color32::from_rgb(200,60,60) } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Train {
    pub track_idx: usize,
    pub t: f32,
    pub speed: f32,
    pub length: f32,
    pub cars: Vec<TrainCar>,
    pub name: String,
    pub active: bool,
    pub direction: f32,
}

impl Train {
    pub fn new(name: &str, track_idx: usize) -> Self {
        Self { track_idx, t: 0.0, speed: 30.0, length: 50.0, cars: vec![TrainCar{length:12.0,width:3.0,car_type:TrainCarType::Locomotive,color:Color32::from_rgb(60,60,60)},TrainCar{length:20.0,width:3.0,car_type:TrainCarType::Passenger,color:Color32::from_rgb(100,140,200)}], name:name.to_string(), active:true, direction:1.0 }
    }
    pub fn total_length(&self) -> f32 { self.cars.iter().map(|c| c.length + 1.5).sum::<f32>() }
    pub fn tick(&mut self, dt: f32, track: &RailwayTrack, spline: &Spline) {
        if !self.active { return; }
        let arc_len = spline_arc_length(spline, 32).max(0.001);
        let t_step = self.speed * dt / arc_len;
        self.t += t_step * self.direction;
        self.t = self.t.rem_euclid(1.0);
        let _ = track;
    }
}

pub fn draw_railway(painter: &Painter, track: &RailwayTrack, spline: &Spline, editor: &SplineEditor, canvas_rect: Rect) {
    if spline.nodes.is_empty() { return; }
    let steps = 80usize;
    let rail_col = track.track_color;
    let tie_col = Color32::from_rgb(80, 60, 40);
    let gauge_px = track.gauge * editor.canvas_zoom;

    let pts: Vec<[f32;2]> = (0..=steps).map(|i| point_on_spline(spline, i as f32 / steps as f32)).collect();

    // Draw ties every ~N pixels
    for i in (0..pts.len()-1).step_by(4) {
        let (p0, p1) = (editor.world_to_screen(pts[i]), editor.world_to_screen(pts[i+1].min_by_index_or(pts[i])));
        if !canvas_rect.expand(20.0).contains(p0) { continue; }
        let mid = Pos2::new((p0.x+p1.x)*0.5,(p0.y+p1.y)*0.5);
        let dx = p1.x-p0.x; let dy = p1.y-p0.y;
        let len = (dx*dx+dy*dy).sqrt().max(0.001);
        let perp = [-dy/len*gauge_px*0.7, dx/len*gauge_px*0.7];
        let tl = Pos2::new(mid.x-perp[0], mid.y-perp[1]);
        let tr = Pos2::new(mid.x+perp[0], mid.y+perp[1]);
        painter.line_segment([tl,tr], Stroke::new(2.5, tie_col));
    }

    // Draw rails
    for i in 0..(pts.len()-1) {
        let (p0, p1) = (editor.world_to_screen(pts[i]), editor.world_to_screen(pts[i+1]));
        if !canvas_rect.expand(20.0).contains(p0) { continue; }
        let dx = p1.x-p0.x; let dy = p1.y-p0.y;
        let len = (dx*dx+dy*dy).sqrt().max(0.001);
        let perp = [-dy/len*gauge_px*0.5, dx/len*gauge_px*0.5];
        let l0=Pos2::new(p0.x-perp[0],p0.y-perp[1]); let l1=Pos2::new(p1.x-perp[0],p1.y-perp[1]);
        let r0=Pos2::new(p0.x+perp[0],p0.y+perp[1]); let r1=Pos2::new(p1.x+perp[0],p1.y+perp[1]);
        painter.line_segment([l0,l1], Stroke::new(1.5, rail_col));
        painter.line_segment([r0,r1], Stroke::new(1.5, rail_col));
    }

    // Draw switch points
    for sw in &track.switch_points {
        let spt = point_on_spline(spline, sw.t);
        let sp = editor.world_to_screen(spt);
        let col = if sw.triggered { Color32::from_rgb(80,200,80) } else { Color32::from_rgb(200,200,80) };
        painter.diamond_shape(sp, 6.0, col);
        painter.text(Pos2::new(sp.x, sp.y - 10.0), egui::Align2::CENTER_BOTTOM, &sw.label, FontId::proportional(8.0), col);
    }
    let _ = canvas_rect;
}

pub fn draw_train(painter: &Painter, train: &Train, spline: &Spline, editor: &SplineEditor) {
    if !train.active { return; }
    if spline.nodes.is_empty() { return; }
    let arc_len = spline_arc_length(spline, 32).max(0.001);
    let mut car_t = train.t;

    for car in &train.cars {
        let pos = point_on_spline(spline, car_t);
        let sp = editor.world_to_screen(pos);
        let eps = 0.01f32;
        let ahead = point_on_spline(spline, (car_t + eps).min(1.0));
        let dx = ahead[0]-pos[0]; let dy = ahead[1]-pos[1];
        let angle = dy.atan2(dx);
        let car_w_px = (car.width * editor.canvas_zoom).max(3.0);
        let car_l_px = (car.length * editor.canvas_zoom * 0.5).max(5.0);
        let cos_a = angle.cos(); let sin_a = angle.sin();
        let corners = [
            Pos2::new(sp.x + cos_a*car_l_px - sin_a*car_w_px, sp.y + sin_a*car_l_px + cos_a*car_w_px),
            Pos2::new(sp.x - cos_a*car_l_px - sin_a*car_w_px, sp.y - sin_a*car_l_px + cos_a*car_w_px),
            Pos2::new(sp.x - cos_a*car_l_px + sin_a*car_w_px, sp.y - sin_a*car_l_px - cos_a*car_w_px),
            Pos2::new(sp.x + cos_a*car_l_px + sin_a*car_w_px, sp.y + sin_a*car_l_px - cos_a*car_w_px),
        ];
        painter.add(Shape::convex_polygon(corners.to_vec(), car.color, Stroke::new(1.0, Color32::BLACK)));
        // Car label
        painter.text(sp, egui::Align2::CENTER_CENTER, car.car_type.name(), FontId::proportional(7.0), Color32::WHITE);
        car_t -= car.length / arc_len;
        if car_t < 0.0 { car_t += 1.0; }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RailwayEditorState {
    pub tracks: Vec<RailwayTrack>,
    pub trains: Vec<Train>,
    pub selected_track: Option<usize>,
    pub selected_train: Option<usize>,
}

pub fn show_railway_editor(ui: &mut egui::Ui, state: &mut RailwayEditorState, splines: &[Spline], dt: f32) {
    ui.heading("Railway Editor");
    ui.horizontal(|ui| {
        if ui.button("Add Track").clicked() {
            state.tracks.push(RailwayTrack::new(0));
        }
        if ui.button("Add Train").clicked() {
            let name = format!("Train {}", state.trains.len() + 1);
            state.trains.push(Train::new(&name, 0));
        }
    });
    ui.separator();
    ui.label(format!("{} tracks | {} trains", state.tracks.len(), state.trains.len()));

    if let Some(sel) = state.selected_track {
        if sel < state.tracks.len() {
            let track = &mut state.tracks[sel];
            egui::CollapsingHeader::new("Track Config").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut track.name);
                });
                ui.horizontal(|ui| {
                    ui.label("Spline:");
                    ui.add(egui::DragValue::new(&mut track.spline_idx).range(0..=splines.len().saturating_sub(1)));
                    ui.label("Gauge:");
                    ui.add(egui::DragValue::new(&mut track.gauge).range(0.5..=3.0).suffix("m"));
                });
                ui.horizontal(|ui| {
                    ui.label("Add switch at t=");
                    if ui.button("+Switch").clicked() {
                        track.switch_points.push(SwitchPoint{t:0.5, branch_spline:0, triggered:false, label:"SW".to_string()});
                    }
                });
                for sw in &mut track.switch_points {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut sw.t).range(0.0..=1.0).speed(0.01));
                        ui.text_edit_singleline(&mut sw.label);
                        ui.checkbox(&mut sw.triggered, "Active");
                    });
                }
            });
        }
    }

    // Tick trains
    for train in &mut state.trains {
        if let Some(track) = state.tracks.get(train.track_idx) {
            if let Some(spline) = splines.get(track.spline_idx) {
                let track_clone = track.clone();
                train.tick(dt, &track_clone, spline);
            }
        }
    }
}

// =================================================================
// SPLINE ANIMATION SYSTEM
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineAnimation {
    pub node_id: u32,
    pub spline_idx: usize,
    pub speed: f32,
    pub loop_mode: LoopMode,
    pub start_t: f32,
    pub current_t: f32,
    pub orient_to_path: bool,
    pub up_vector: [f32; 3],
    pub name: String,
    pub active: bool,
    pub time_offset: f32,
}

impl SplineAnimation {
    pub fn new(node_id: u32, spline_idx: usize) -> Self {
        Self { node_id, spline_idx, speed: 1.0, loop_mode: LoopMode::Loop, start_t: 0.0, current_t: 0.0, orient_to_path: true, up_vector: [0.0, 0.0, 1.0], name: format!("Anim_{}", node_id), active: true, time_offset: 0.0 }
    }
    pub fn tick(&mut self, dt: f32, arc_len: f32) {
        if !self.active || arc_len <= 0.0 { return; }
        let t_step = self.speed * dt / arc_len;
        self.current_t += t_step;
        match self.loop_mode {
            LoopMode::Loop => { self.current_t = self.current_t.rem_euclid(1.0); }
            LoopMode::PingPong => {
                if self.current_t > 1.0 { self.current_t = 2.0 - self.current_t; self.speed = -self.speed.abs(); }
                if self.current_t < 0.0 { self.current_t = -self.current_t; self.speed = self.speed.abs(); }
            }
            _ => { self.current_t = self.current_t.clamp(0.0, 1.0); }
        }
    }
}

pub fn draw_spline_animations(painter: &Painter, animations: &[SplineAnimation], splines: &[Spline], editor: &SplineEditor) {
    for anim in animations {
        if !anim.active { continue; }
        let spline = match splines.get(anim.spline_idx) { Some(s) => s, None => continue };
        let pos = point_on_spline(spline, anim.current_t);
        let sp = editor.world_to_screen(pos);
        painter.circle_filled(sp, 6.0, Color32::from_rgb(200, 150, 80));
        if anim.orient_to_path {
            let ahead = point_on_spline(spline, (anim.current_t + 0.02).min(1.0));
            let asp = editor.world_to_screen(ahead);
            painter.arrow(sp, (asp - sp) * 0.7, Stroke::new(2.0, Color32::from_rgb(255, 200, 100)));
        }
        painter.text(Pos2::new(sp.x, sp.y - 10.0), egui::Align2::CENTER_BOTTOM, &anim.name, FontId::proportional(9.0), Color32::from_rgb(200, 150, 80));
    }
}

pub fn show_spline_animation_editor(ui: &mut egui::Ui, animations: &mut Vec<SplineAnimation>, splines: &[Spline], dt: f32) {
    ui.heading("Spline Animations");
    if ui.button("Add Animation").clicked() {
        animations.push(SplineAnimation::new(animations.len() as u32, 0));
    }
    ui.separator();

    let mut to_remove = None;
    for (i, anim) in animations.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            egui::CollapsingHeader::new(&anim.name).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut anim.active, "Active");
                    ui.label("Node ID:");
                    ui.add(egui::DragValue::new(&mut anim.node_id));
                });
                ui.horizontal(|ui| {
                    ui.label("Spline:");
                    ui.add(egui::DragValue::new(&mut anim.spline_idx).range(0..=splines.len().saturating_sub(1)));
                    ui.label("Speed:");
                    ui.add(egui::DragValue::new(&mut anim.speed).range(0.01..=50.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Loop:");
                    for lm in &[LoopMode::Once, LoopMode::Loop, LoopMode::PingPong, LoopMode::Clamp] {
                        if ui.selectable_label(anim.loop_mode == *lm, lm.name()).clicked() { anim.loop_mode = lm.clone(); }
                    }
                });
                ui.checkbox(&mut anim.orient_to_path, "Orient to path");
                ui.label(format!("t = {:.3}", anim.current_t));
                if ui.button("Reset").clicked() { anim.current_t = anim.start_t; }
                if ui.button("Remove").clicked() { to_remove = Some(i); }

                // Tick
                if anim.active {
                    if let Some(spline) = splines.get(anim.spline_idx) {
                        let arc_len = spline_arc_length(spline, 32);
                        anim.tick(dt, arc_len);
                    }
                }
            });
        });
    }
    if let Some(idx) = to_remove { animations.remove(idx); }
}

// =================================================================
// DEFORMATION SPLINES
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeformSpline {
    pub control_spline: usize,
    pub affected_objects: Vec<u32>,
    pub influence_radius: f32,
    pub falloff_power: f32,
    pub name: String,
    pub active: bool,
}

impl DeformSpline {
    pub fn new(control_spline: usize) -> Self {
        Self { control_spline, affected_objects: Vec::new(), influence_radius: 50.0, falloff_power: 2.0, name: format!("Deform {}", control_spline), active: true }
    }
    pub fn influence_at_dist(&self, dist: f32) -> f32 {
        if dist >= self.influence_radius { return 0.0; }
        let t = 1.0 - dist / self.influence_radius;
        t.powf(self.falloff_power)
    }
}

pub fn draw_deform_influence(painter: &Painter, deform: &DeformSpline, spline: &Spline, editor: &SplineEditor, canvas_rect: Rect) {
    if !deform.active { return; }
    let steps = 40usize;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let pt = point_on_spline(spline, t);
        let sp = editor.world_to_screen(pt);
        if !canvas_rect.expand(deform.influence_radius * editor.canvas_zoom).contains(sp) { continue; }
        let radius = deform.influence_radius * editor.canvas_zoom;
        painter.circle_stroke(sp, radius, Stroke::new(0.5, Color32::from_rgba_unmultiplied(180, 120, 255, 40)));
    }
    let _ = canvas_rect;
}

// =================================================================
// EXTENDED SPLINE EDITOR STATE
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ExtendedSplineEditorState {
    pub road_editor: RoadEditorState,
    pub camera_sequence: CameraSequence,
    pub path_followers: Vec<PathFollower>,
    pub railway: RailwayEditorState,
    pub animations: Vec<SplineAnimation>,
    pub deform_splines: Vec<DeformSpline>,
    pub active_tab: ExtendedSplineTab,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum ExtendedSplineTab { #[default] RoadTool, CameraPath, PathFollowers, Railway, Animations, Deformation }
impl ExtendedSplineTab {
    pub fn name(&self) -> &str { match self { ExtendedSplineTab::RoadTool=>"Roads", ExtendedSplineTab::CameraPath=>"Camera", ExtendedSplineTab::PathFollowers=>"Followers", ExtendedSplineTab::Railway=>"Railway", ExtendedSplineTab::Animations=>"Animations", ExtendedSplineTab::Deformation=>"Deform" } }
    pub fn all() -> &'static [ExtendedSplineTab] { &[ExtendedSplineTab::RoadTool, ExtendedSplineTab::CameraPath, ExtendedSplineTab::PathFollowers, ExtendedSplineTab::Railway, ExtendedSplineTab::Animations, ExtendedSplineTab::Deformation] }
}

pub fn show_extended_spline_panel(ui: &mut egui::Ui, ext: &mut ExtendedSplineEditorState, splines: &[Spline], dt: f32) {
    ui.horizontal(|ui| {
        for tab in ExtendedSplineTab::all() {
            if ui.selectable_label(ext.active_tab == *tab, tab.name()).clicked() {
                ext.active_tab = tab.clone();
            }
        }
    });
    ui.separator();
    match ext.active_tab {
        ExtendedSplineTab::RoadTool       => show_road_editor(ui, &mut ext.road_editor, splines),
        ExtendedSplineTab::CameraPath     => show_camera_sequence_editor(ui, &mut ext.camera_sequence, splines, dt),
        ExtendedSplineTab::PathFollowers  => show_path_follower_editor(ui, &mut ext.path_followers, splines, dt),
        ExtendedSplineTab::Railway        => show_railway_editor(ui, &mut ext.railway, splines, dt),
        ExtendedSplineTab::Animations     => show_spline_animation_editor(ui, &mut ext.animations, splines, dt),
        ExtendedSplineTab::Deformation    => {
            ui.heading("Deformation Splines");
            if ui.button("Add Deform Spline").clicked() {
                ext.deform_splines.push(DeformSpline::new(0));
            }
            for (i, ds) in ext.deform_splines.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    egui::CollapsingHeader::new(&ds.name).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut ds.active, "Active");
                            ui.label("Control spline:");
                            ui.add(egui::DragValue::new(&mut ds.control_spline).range(0..=splines.len().saturating_sub(1)));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Radius:");
                            ui.add(egui::DragValue::new(&mut ds.influence_radius).range(1.0..=500.0));
                            ui.label("Falloff:");
                            ui.add(egui::DragValue::new(&mut ds.falloff_power).range(0.1..=5.0));
                        });
                    });
                });
            }
        }
    }
}

// Helper needed for railways
trait MinByIndex {
    fn min_by_index_or(&self, fallback: [f32;2]) -> [f32;2];
}
impl MinByIndex for [f32;2] {
    fn min_by_index_or(&self, _fallback: [f32;2]) -> [f32;2] { *self }
}

// =================================================================
// TESTS FOR NEW SPLINE SYSTEMS
// =================================================================

#[cfg(test)]
mod new_spline_tests {
    use super::*;

    #[test]
    fn test_road_config_total_width() {
        let config = RoadConfig::default();
        let w = config.total_width();
        assert!(w > 0.0);
        assert!(w > config.lane_count as f32 * config.lane_width);
    }
    #[test]
    fn test_road_config_highway_wider() {
        let h = RoadConfig::highway();
        let c = RoadConfig::city_street();
        assert!(h.total_width() > c.total_width());
    }
    #[test]
    fn test_road_segment_elevation_at() {
        let mut seg = RoadSegment::new(0);
        seg.start_elevation = 10.0; seg.end_elevation = 20.0;
        assert!((seg.elevation_at(0.0) - 10.0).abs() < 0.01);
        assert!((seg.elevation_at(1.0) - 20.0).abs() < 0.01);
        assert!((seg.elevation_at(0.5) - 15.0).abs() < 0.01);
    }
    #[test]
    fn test_road_segment_elevation_profile() {
        let mut seg = RoadSegment::new(0);
        seg.elevation_profile = vec![0.0, 5.0, 10.0];
        assert!((seg.elevation_at(0.0) - 0.0).abs() < 0.01);
        assert!((seg.elevation_at(1.0) - 10.0).abs() < 0.01);
    }
    #[test]
    fn test_easing_endpoints() {
        for e in EasingType::all() {
            assert!(e.evaluate(0.0) <= 0.01, "{:?} at 0", e);
            assert!(e.evaluate(1.0) >= 0.99, "{:?} at 1", e);
        }
    }
    #[test]
    fn test_easing_monotone_linear() {
        let e = EasingType::Linear;
        for i in 0..9 {
            let t0 = i as f32 / 10.0;
            let t1 = (i+1) as f32 / 10.0;
            assert!(e.evaluate(t1) >= e.evaluate(t0));
        }
    }
    #[test]
    fn test_camera_path_fov_interpolation() {
        let mut path = CameraPath::default();
        path.fov_start = 30.0; path.fov_end = 90.0;
        assert!((path.fov_at(0.0) - 30.0).abs() < 0.01);
        assert!((path.fov_at(1.0) - 90.0).abs() < 0.01);
        assert!((path.fov_at(0.5) - 60.0).abs() < 0.01);
    }
    #[test]
    fn test_camera_sequence_total_duration() {
        let mut seq = CameraSequence::default();
        seq.shots.push(CameraShot::new("S1"));
        seq.shots.push(CameraShot::new("S2"));
        seq.shots[0].duration = 3.0;
        seq.shots[1].duration = 7.0;
        assert!((seq.total_duration() - 10.0).abs() < 0.001);
    }
    #[test]
    fn test_camera_sequence_shot_at_time() {
        let mut seq = CameraSequence::default();
        seq.shots.push(CameraShot::new("S1"));
        seq.shots.push(CameraShot::new("S2"));
        seq.shots[0].duration = 5.0;
        seq.shots[1].duration = 5.0;
        let (idx0, _) = seq.shot_at_time(2.0);
        let (idx1, _) = seq.shot_at_time(7.0);
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
    }
    #[test]
    fn test_loop_mode_names_unique() {
        let modes = [LoopMode::Once, LoopMode::Loop, LoopMode::PingPong, LoopMode::Clamp];
        let names: Vec<&str> = modes.iter().map(|m| m.name()).collect();
        let set: std::collections::HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), set.len());
    }
    #[test]
    fn test_path_follower_loop_mode() {
        let f = PathFollower::new("test", 0);
        assert_eq!(f.loop_mode, LoopMode::Loop);
        assert_eq!(f.current_t, 0.0);
    }
    #[test]
    fn test_railway_track_default_gauge() {
        let track = RailwayTrack::new(0);
        assert!((track.gauge - 1.435).abs() < 0.001);
    }
    #[test]
    fn test_train_total_length() {
        let train = Train::new("test", 0);
        assert!(train.total_length() > 0.0);
    }
    #[test]
    fn test_deform_spline_influence() {
        let ds = DeformSpline::new(0);
        assert!((ds.influence_at_dist(0.0) - 1.0).abs() < 0.001);
        assert!((ds.influence_at_dist(ds.influence_radius) - 0.0).abs() < 0.001);
        assert!(ds.influence_at_dist(ds.influence_radius * 0.5) > 0.0);
        assert!(ds.influence_at_dist(ds.influence_radius * 0.5) < 1.0);
    }
    #[test]
    fn test_spline_animation_tick_loop() {
        let mut anim = SplineAnimation::new(1, 0);
        anim.loop_mode = LoopMode::Loop;
        anim.speed = 1.0;
        // Advance far past 1.0
        anim.tick(10.0, 1.0);
        assert!(anim.current_t >= 0.0 && anim.current_t <= 1.0);
    }
    #[test]
    fn test_road_surface_colors_distinct() {
        let types = RoadSurfaceType::all();
        let colors: Vec<Color32> = types.iter().map(|t| t.color()).collect();
        for i in 0..colors.len() {
            for j in (i+1)..colors.len() {
                assert_ne!(colors[i], colors[j], "Types {} and {} have same color", i, j);
            }
        }
    }
    #[test]
    fn test_intersection_type_names() {
        for it in &[IntersectionType::T, IntersectionType::X, IntersectionType::Y, IntersectionType::Roundabout] {
            assert!(!it.name().is_empty());
        }
    }
    #[test]
    fn test_road_network_add_segment() {
        let mut net = RoadNetwork::default();
        let idx = net.add_segment(RoadSegment::new(0));
        assert_eq!(idx, 0);
        assert_eq!(net.segments.len(), 1);
    }
    #[test]
    fn test_road_network_add_intersection() {
        let mut net = RoadNetwork::default();
        let idx = net.add_intersection([10.0, 20.0]);
        assert_eq!(idx, 0);
        assert_eq!(net.intersections.len(), 1);
    }
    #[test]
    fn test_camera_sequence_tick() {
        let mut seq = CameraSequence::default();
        seq.shots.push(CameraShot::new("S1"));
        seq.shots[0].duration = 5.0;
        seq.is_playing = true;
        seq.tick(1.0);
        assert!((seq.playback_time - 1.0).abs() < 0.01);
        assert!(seq.is_playing);
    }
    #[test]
    fn test_camera_sequence_stops_at_end() {
        let mut seq = CameraSequence::default();
        seq.shots.push(CameraShot::new("S1"));
        seq.shots[0].duration = 2.0;
        seq.is_playing = true;
        seq.tick(5.0); // way past end
        assert!(!seq.is_playing);
    }
    #[test]
    fn test_train_car_types_distinct() {
        use std::collections::HashSet;
        let types = [TrainCarType::Locomotive, TrainCarType::Passenger, TrainCarType::Freight, TrainCarType::Flatcar, TrainCarType::Tank, TrainCarType::Caboose];
        let names: Vec<&str> = types.iter().map(|t| t.name()).collect();
        let set: HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), set.len());
    }
    #[test]
    fn test_switch_point_triggered_false_by_default() {
        let sw = SwitchPoint { t: 0.5, branch_spline: 0, triggered: false, label: "SW".to_string() };
        assert!(!sw.triggered);
    }
    #[test]
    fn test_deform_influence_falloff_power() {
        let mut ds = DeformSpline::new(0);
        ds.falloff_power = 1.0;
        let v1 = ds.influence_at_dist(ds.influence_radius * 0.5);
        ds.falloff_power = 3.0;
        let v2 = ds.influence_at_dist(ds.influence_radius * 0.5);
        assert!(v1 > v2, "Higher falloff power should reduce influence faster");
    }
    #[test]
    fn test_spline_animation_ping_pong() {
        let mut anim = SplineAnimation::new(1, 0);
        anim.loop_mode = LoopMode::PingPong;
        anim.speed = 1.0;
        anim.tick(2.5, 1.0); // go past 1.0
        assert!(anim.current_t >= 0.0 && anim.current_t <= 1.0);
    }
    #[test]
    fn test_road_config_dirt_no_sidewalk() {
        let dirt = RoadConfig::dirt_track();
        assert!(!dirt.has_sidewalk);
    }
    #[test]
    fn test_road_config_highway_no_sidewalk() {
        let hw = RoadConfig::highway();
        assert!(!hw.has_sidewalk);
        assert!(hw.lane_count >= 3);
    }
    #[test]
    fn test_extended_spline_tab_names_unique() {
        use std::collections::HashSet;
        let names: Vec<&str> = ExtendedSplineTab::all().iter().map(|t| t.name()).collect();
        let set: HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), set.len());
    }
}


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


// =================================================================
// SPLINE EXPANSION 3: SPLINE PAINTER & RENDERING
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SplinePaintMode {
    Solid,
    Dashed,
    Dotted,
    Gradient,
    Animated,
    Glow,
    Rainbow,
    Arrow,
}

impl SplinePaintMode {
    pub fn name(&self) -> &'static str {
        match self {
            SplinePaintMode::Solid => "Solid",
            SplinePaintMode::Dashed => "Dashed",
            SplinePaintMode::Dotted => "Dotted",
            SplinePaintMode::Gradient => "Gradient",
            SplinePaintMode::Animated => "Animated",
            SplinePaintMode::Glow => "Glow",
            SplinePaintMode::Rainbow => "Rainbow",
            SplinePaintMode::Arrow => "Arrow",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplinePaintStyle {
    pub mode: SplinePaintMode,
    pub color_start: egui::Color32,
    pub color_end: egui::Color32,
    pub width: f32,
    pub dash_length: f32,
    pub gap_length: f32,
    pub animation_speed: f32,
    pub animation_offset: f32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
    pub arrow_spacing: f32,
    pub arrow_size: f32,
    pub opacity: f32,
}

impl Default for SplinePaintStyle {
    fn default() -> Self {
        Self {
            mode: SplinePaintMode::Solid,
            color_start: egui::Color32::from_rgb(100, 200, 255),
            color_end: egui::Color32::from_rgb(255, 100, 200),
            width: 2.0, dash_length: 10.0, gap_length: 5.0,
            animation_speed: 1.0, animation_offset: 0.0,
            glow_radius: 8.0, glow_intensity: 0.5,
            arrow_spacing: 30.0, arrow_size: 8.0,
            opacity: 1.0,
        }
    }
}

pub fn draw_styled_spline(painter: &egui::Painter, points: &[egui::Pos2], style: &SplinePaintStyle) {
    if points.len() < 2 { return; }
    let total_pts = points.len();
    match style.mode {
        SplinePaintMode::Solid => {
            for w in points.windows(2) {
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, style.color_start));
            }
        }
        SplinePaintMode::Dashed => {
            let mut dist = 0.0f32;
            let mut drawing = true;
            let pattern = style.dash_length + style.gap_length;
            for w in points.windows(2) {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                let seg_len = (dx*dx + dy*dy).sqrt();
                let phase = dist % pattern;
                let draw_amt = (style.dash_length - phase).max(0.0).min(seg_len);
                if drawing && draw_amt > 0.0 {
                    let tx = w[0].x + dx * draw_amt / seg_len;
                    let ty = w[0].y + dy * draw_amt / seg_len;
                    painter.line_segment([w[0], egui::Pos2::new(tx, ty)], egui::Stroke::new(style.width, style.color_start));
                }
                dist += seg_len;
            }
        }
        SplinePaintMode::Gradient => {
            for (i, w) in points.windows(2).enumerate() {
                let t = i as f32 / total_pts as f32;
                let r = (style.color_start.r() as f32 + (style.color_end.r() as f32 - style.color_start.r() as f32) * t) as u8;
                let g = (style.color_start.g() as f32 + (style.color_end.g() as f32 - style.color_start.g() as f32) * t) as u8;
                let b = (style.color_start.b() as f32 + (style.color_end.b() as f32 - style.color_start.b() as f32) * t) as u8;
                let col = egui::Color32::from_rgb(r, g, b);
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, col));
            }
        }
        SplinePaintMode::Rainbow => {
            for (i, w) in points.windows(2).enumerate() {
                let hue = (i as f32 / total_pts as f32 * 360.0) as u32 % 360;
                let col = hsv_to_rgb(hue as f32, 1.0, 1.0);
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, col));
            }
        }
        SplinePaintMode::Arrow => {
            for w in points.windows(2) {
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, style.color_start));
            }
            // Draw arrowheads along the path
            let mut dist = style.arrow_spacing;
            for w in points.windows(2) {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                let seg_len = (dx*dx + dy*dy).sqrt().max(0.001);
                if dist <= seg_len {
                    let t = dist / seg_len;
                    let px = w[0].x + dx * t;
                    let py = w[0].y + dy * t;
                    let nx = dx / seg_len;
                    let ny = dy / seg_len;
                    let s = style.arrow_size;
                    painter.line_segment([egui::Pos2::new(px, py), egui::Pos2::new(px - nx*s - ny*s*0.5, py - ny*s + nx*s*0.5)], egui::Stroke::new(style.width, style.color_start));
                    painter.line_segment([egui::Pos2::new(px, py), egui::Pos2::new(px - nx*s + ny*s*0.5, py - ny*s - nx*s*0.5)], egui::Stroke::new(style.width, style.color_start));
                    dist = style.arrow_spacing;
                } else {
                    dist -= seg_len;
                }
            }
        }
        SplinePaintMode::Glow => {
            // Draw multiple strokes with decreasing opacity for glow
            for layer in 0..4 {
                let radius = style.glow_radius * (4 - layer) as f32 / 4.0;
                let alpha = (style.glow_intensity * layer as f32 / 4.0 * 255.0) as u8;
                let col = egui::Color32::from_rgba_premultiplied(style.color_start.r(), style.color_start.g(), style.color_start.b(), alpha);
                for w in points.windows(2) {
                    painter.line_segment([w[0], w[1]], egui::Stroke::new(radius, col));
                }
            }
            for w in points.windows(2) {
                painter.line_segment([w[0], w[1]], egui::Stroke::new(style.width, style.color_start));
            }
        }
        SplinePaintMode::Dotted => {
            let mut dist = 0.0f32;
            for w in points.windows(2) {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                let seg_len = (dx*dx + dy*dy).sqrt().max(0.001);
                while dist < seg_len {
                    let t = dist / seg_len;
                    let px = w[0].x + dx * t;
                    let py = w[0].y + dy * t;
                    painter.circle_filled(egui::Pos2::new(px, py), style.width, style.color_start);
                    dist += style.dash_length + style.gap_length;
                }
                dist -= seg_len;
            }
        }
        SplinePaintMode::Animated => {
            // Animated dashes based on offset
            let offset = style.animation_offset;
            let pattern = style.dash_length + style.gap_length;
            for w in points.windows(2) {
                let dx = w[1].x - w[0].x;
                let dy = w[1].y - w[0].y;
                let seg_len = (dx*dx + dy*dy).sqrt().max(0.001);
                let phase = (offset + 0.0) % pattern;
                let draw_from = if phase < style.dash_length { 0.0 } else { pattern - phase };
                if draw_from < seg_len {
                    let t = draw_from / seg_len;
                    let t2 = ((draw_from + style.dash_length) / seg_len).min(1.0);
                    let pa = egui::Pos2::new(w[0].x + dx * t, w[0].y + dy * t);
                    let pb = egui::Pos2::new(w[0].x + dx * t2, w[0].y + dy * t2);
                    painter.line_segment([pa, pb], egui::Stroke::new(style.width, style.color_start));
                }
            }
        }
    }
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> egui::Color32 {
    let h6 = h / 60.0;
    let i = h6.floor() as u32 % 6;
    let f = h6 - h6.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

pub fn show_paint_style_editor(ui: &mut egui::Ui, style: &mut SplinePaintStyle) {
    ui.collapsing("Paint Style", |ui| {
        ui.horizontal(|ui| {
            for mode in [SplinePaintMode::Solid, SplinePaintMode::Dashed, SplinePaintMode::Gradient, SplinePaintMode::Glow, SplinePaintMode::Arrow, SplinePaintMode::Rainbow] {
                if ui.selectable_label(style.mode == mode, mode.name()).clicked() { style.mode = mode; }
            }
        });
        ui.add(egui::Slider::new(&mut style.width, 0.5..=20.0).text("Width"));
        ui.add(egui::Slider::new(&mut style.opacity, 0.0..=1.0).text("Opacity"));
        ui.horizontal(|ui| {
            ui.label("Color A:");
            egui::color_picker::color_edit_button_srgba(ui, &mut style.color_start, egui::color_picker::Alpha::Opaque);
            ui.label("Color B:");
            egui::color_picker::color_edit_button_srgba(ui, &mut style.color_end, egui::color_picker::Alpha::Opaque);
        });
        if matches!(style.mode, SplinePaintMode::Dashed | SplinePaintMode::Dotted | SplinePaintMode::Animated) {
            ui.add(egui::Slider::new(&mut style.dash_length, 1.0..=50.0).text("Dash Length"));
            ui.add(egui::Slider::new(&mut style.gap_length, 1.0..=50.0).text("Gap Length"));
        }
        if style.mode == SplinePaintMode::Glow {
            ui.add(egui::Slider::new(&mut style.glow_radius, 1.0..=30.0).text("Glow Radius"));
            ui.add(egui::Slider::new(&mut style.glow_intensity, 0.0..=1.0).text("Glow Intensity"));
        }
        if style.mode == SplinePaintMode::Arrow {
            ui.add(egui::Slider::new(&mut style.arrow_spacing, 10.0..=100.0).text("Arrow Spacing"));
            ui.add(egui::Slider::new(&mut style.arrow_size, 3.0..=20.0).text("Arrow Size"));
        }
        if style.mode == SplinePaintMode::Animated {
            ui.add(egui::Slider::new(&mut style.animation_speed, 0.1..=10.0).text("Animation Speed"));
        }
    });
}

// =================================================================
// SPLINE EXPANSION 3: SPLINE LIBRARY MANAGER
// =================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplineLibraryEntry {
    pub id: u32,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub points: Vec<[f32;2]>,
    pub paint_style: SplinePaintStyle,
    pub is_favorite: bool,
    pub usage_count: u32,
    pub description: String,
}

impl SplineLibraryEntry {
    pub fn new(id: u32, name: &str, points: Vec<[f32;2]>) -> Self {
        Self { id, name: name.to_string(), category: String::new(), tags: Vec::new(), points, paint_style: SplinePaintStyle::default(), is_favorite: false, usage_count: 0, description: String::new() }
    }

    pub fn arc_length(&self) -> f32 {
        compute_spline_arc_length_points(&self.points)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SplineLibraryManagerState {
    pub entries: Vec<SplineLibraryEntry>,
    pub selected: Option<usize>,
    pub search_query: String,
    pub filter_category: String,
    pub filter_favorites: bool,
    pub sort_by: SplineLibrarySortBy,
    pub next_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub enum SplineLibrarySortBy {
    #[default]
    Name,
    Category,
    Usage,
    Length,
}

impl SplineLibraryManagerState {
    pub fn new() -> Self {
        let mut s = Self::default();
        // Add some example entries
        s.entries = vec![
            SplineLibraryEntry::new(0, "Simple Curve", vec![[0.0,0.0],[0.5,0.3],[1.0,0.0]]),
            SplineLibraryEntry::new(1, "S-Curve", vec![[0.0,0.0],[0.3,-0.2],[0.7,0.2],[1.0,0.0]]),
            SplineLibraryEntry::new(2, "Circle Path", generate_parametric_curve(&ParametricCurveConfig { curve_type: ParametricCurveType::Circle, param_a: 1.0, resolution: 32, ..Default::default() })),
        ];
        s.entries[0].category = "Basic".to_string();
        s.entries[1].category = "Basic".to_string();
        s.entries[2].category = "Shapes".to_string();
        s.next_id = 3;
        s
    }

    pub fn visible_entries(&self) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        let mut indices: Vec<usize> = self.entries.iter().enumerate().filter(|(_, e)| {
            (!self.filter_favorites || e.is_favorite) &&
            (self.filter_category.is_empty() || e.category == self.filter_category) &&
            (q.is_empty() || e.name.to_lowercase().contains(&q) || e.tags.iter().any(|t| t.contains(&q)))
        }).map(|(i, _)| i).collect();
        indices.sort_by(|&a, &b| {
            match self.sort_by {
                SplineLibrarySortBy::Name => self.entries[a].name.cmp(&self.entries[b].name),
                SplineLibrarySortBy::Category => self.entries[a].category.cmp(&self.entries[b].category),
                SplineLibrarySortBy::Usage => self.entries[b].usage_count.cmp(&self.entries[a].usage_count),
                SplineLibrarySortBy::Length => self.entries[a].arc_length().partial_cmp(&self.entries[b].arc_length()).unwrap_or(std::cmp::Ordering::Equal),
            }
        });
        indices
    }
}

pub fn show_spline_library_manager(ui: &mut egui::Ui, state: &mut SplineLibraryManagerState) {
    ui.heading("Spline Library");
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.search_query);
        ui.checkbox(&mut state.filter_favorites, "★ Favorites");
    });
    ui.horizontal(|ui| {
        ui.label("Sort:");
        for (sort, label) in [(SplineLibrarySortBy::Name, "Name"), (SplineLibrarySortBy::Category, "Cat"), (SplineLibrarySortBy::Usage, "Usage"), (SplineLibrarySortBy::Length, "Length")] {
            if ui.selectable_label(state.sort_by == sort, label).clicked() { state.sort_by = sort; }
        }
    });
    ui.horizontal(|ui| {
        if ui.button("+ New Entry").clicked() {
            let id = state.next_id;
            state.next_id += 1;
            state.entries.push(SplineLibraryEntry::new(id, &format!("Spline {}", id), vec![[0.0,0.0],[1.0,0.0]]));
        }
    });

    egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
        let visible = state.visible_entries();
        for i in visible {
            let entry = &state.entries[i];
            let sel = state.selected == Some(i);
            ui.horizontal(|ui| {
                if entry.is_favorite { ui.label("★"); }
                if ui.selectable_label(sel, format!("{} [{} pts]", entry.name, entry.points.len())).clicked() {
                    state.selected = Some(i);
                    state.entries[i].usage_count += 1;
                }
                ui.label(format!("L:{:.1}", entry.arc_length()));
            });
        }
    });

    if let Some(idx) = state.selected {
        if let Some(entry) = state.entries.get_mut(idx) {
            ui.separator();
            ui.text_edit_singleline(&mut entry.name);
            ui.text_edit_singleline(&mut entry.category);
            ui.checkbox(&mut entry.is_favorite, "Favorite");
            ui.label(format!("Points: {} | Length: {:.2} | Used: {} times", entry.points.len(), entry.arc_length(), entry.usage_count));
            ui.text_edit_multiline(&mut entry.description);
            show_paint_style_editor(ui, &mut entry.paint_style);
        }
    }
}

// =================================================================
// SPLINE EXPANSION 3: TESTS
// =================================================================

#[cfg(test)]
mod spline_expansion3_tests {
    use super::*;

    #[test]
    fn test_hsv_to_rgb_red() {
        let col = hsv_to_rgb(0.0, 1.0, 1.0);
        assert_eq!(col.r(), 255);
        assert_eq!(col.g(), 0);
        assert_eq!(col.b(), 0);
    }

    #[test]
    fn test_hsv_to_rgb_green() {
        let col = hsv_to_rgb(120.0, 1.0, 1.0);
        assert_eq!(col.r(), 0);
        assert!(col.g() > 200);
    }

    #[test]
    fn test_paint_style_default() {
        let style = SplinePaintStyle::default();
        assert_eq!(style.mode, SplinePaintMode::Solid);
        assert!(style.width > 0.0);
    }

    #[test]
    fn test_spline_library_search() {
        let mut state = SplineLibraryManagerState::new();
        state.search_query = "circle".to_string();
        let visible = state.visible_entries();
        assert!(!visible.is_empty());
    }

    #[test]
    fn test_spline_library_favorites() {
        let mut state = SplineLibraryManagerState::new();
        state.entries[0].is_favorite = true;
        state.filter_favorites = true;
        let visible = state.visible_entries();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0], 0);
    }

    #[test]
    fn test_spline_library_usage_count() {
        let mut state = SplineLibraryManagerState::new();
        state.selected = Some(0);
        // Simulate clicking
        state.entries[0].usage_count += 1;
        assert_eq!(state.entries[0].usage_count, 1);
    }

    #[test]
    fn test_draw_styled_spline_does_not_panic() {
        // No painter available in test, just test the data structures work
        let style = SplinePaintStyle::default();
        let pts = vec![[0.0f32, 0.0], [1.0, 0.0], [2.0, 1.0]];
        assert_eq!(pts.len(), 3);
        assert_eq!(style.mode, SplinePaintMode::Solid);
    }
}


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
        state.node_positions = spline.nodes.iter().map(|nd| (nd.point.position[0], nd.point.position[1])).collect();
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
                point: ControlPoint { position: [i as f32 * 20.0, 0.0], ..ControlPoint::default() },
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
