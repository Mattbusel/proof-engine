
code = r'''

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
    if spline.points.is_empty() { return; }
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
            let stroke_w = (layer_w * editor.zoom).max(1.0);
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
    if spline.points.is_empty() { return; }
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
    if spline.points.is_empty() { return; }
    let steps = 80usize;
    let rail_col = track.track_color;
    let tie_col = Color32::from_rgb(80, 60, 40);
    let gauge_px = track.gauge * editor.zoom;

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
    if spline.points.is_empty() { return; }
    let arc_len = spline_arc_length(spline, 32).max(0.001);
    let mut car_t = train.t;

    for car in &train.cars {
        let pos = point_on_spline(spline, car_t);
        let sp = editor.world_to_screen(pos);
        let eps = 0.01f32;
        let ahead = point_on_spline(spline, (car_t + eps).min(1.0));
        let dx = ahead[0]-pos[0]; let dy = ahead[1]-pos[1];
        let angle = dy.atan2(dx);
        let car_w_px = (car.width * editor.zoom).max(3.0);
        let car_l_px = (car.length * editor.zoom * 0.5).max(5.0);
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
        if !canvas_rect.expand(deform.influence_radius * editor.zoom).contains(sp) { continue; }
        let radius = deform.influence_radius * editor.zoom;
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
'''

with open('C:/proof-engine/editor/src/spline_editor.rs', 'a', encoding='utf-8') as f:
    f.write(code)
import os
print(f"Done. Spline size: {os.path.getsize('C:/proof-engine/editor/src/spline_editor.rs')} bytes")
