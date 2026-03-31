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
