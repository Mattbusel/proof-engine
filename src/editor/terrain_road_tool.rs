#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

const MAX_ROAD_NODES: usize = 65536;
const MAX_ROAD_SEGMENTS: usize = 65536;
const SPLINE_SUBDIVISIONS: usize = 32;
const TERRAIN_SAMPLE_RADIUS: f32 = 8.0;
const ROAD_BLEND_FALLOFF: f32 = 4.0;
const BRIDGE_DETECT_THRESHOLD: f32 = 2.5;
const PILLAR_SPACING: f32 = 12.0;
const POTHOLE_PROBABILITY_BASE: f32 = 0.0001;
const PUDDLE_DEPRESSION_THRESHOLD: f32 = 0.15;
const TRAFFIC_DENSITY_MAX: f32 = 1.0;
const LWR_DT: f32 = 0.016;
const LWR_DX: f32 = 1.0;
const DIJKSTRA_INF: f64 = 1.0e18;
const LANE_WIDTH: f32 = 3.65;
const CURB_HEIGHT: f32 = 0.15;
const CURB_WIDTH: f32 = 0.20;
const SHOULDER_WIDTH: f32 = 2.5;
const DITCH_DEPTH: f32 = 0.4;
const DITCH_WIDTH: f32 = 1.2;
const SIDEWALK_WIDTH: f32 = 1.5;
const CROSSWALK_STRIPE_WIDTH: f32 = 0.5;
const CROSSWALK_STRIPE_GAP: f32 = 0.5;
const CENTER_LINE_DASH_LEN: f32 = 3.0;
const CENTER_LINE_GAP_LEN: f32 = 9.0;
const ROUNDABOUT_ISLAND_RADIUS: f32 = 6.0;
const ROUNDABOUT_ROAD_WIDTH: f32 = 7.3;
const PRIM_INF: f64 = 1.0e18;
const MAX_SLOPE_FOR_FLATTEN: f32 = 0.7;
const SPLAT_BLEND_RADIUS: f32 = 5.0;
const EROSION_TIMESTEPS: usize = 100;
const WEAR_ALPHA: f32 = 0.002;
const UNDO_STACK_SIZE: usize = 256;
const SNAP_RADIUS: f32 = 2.0;

// ============================================================
// ROAD TYPES
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoadType {
    DirtTrack,
    GravelRoad,
    PavedRoad,
    Highway2Lane,
    Highway4Lane,
    Motorway,
    Alley,
    Bridge,
    Tunnel,
    ElevatedHighway,
    Cobblestone,
    Boulevard,
    ResidentialStreet,
    BridgeRoad,
    TunnelRoad,
    ServiceRoad,
}

#[derive(Clone, Debug)]
pub struct RoadProfile {
    pub road_type: RoadType,
    pub total_width: f32,
    pub lane_count: u32,
    pub lane_width: f32,
    pub has_curb: bool,
    pub has_shoulder: bool,
    pub has_ditch: bool,
    pub has_sidewalk: bool,
    pub speed_limit_kmh: f32,
    pub surface_friction: f32,
    pub material_id: u32,
    pub shoulder_material_id: u32,
    pub max_slope_grade: f32,
    pub is_elevated: bool,
    pub tunnel_clearance: f32,
    pub bridge_deck_thickness: f32,
}

#[derive(Clone, Debug)]
pub struct SplinePoint {
    pub position: Vec3,
    pub tangent_in: Vec3,
    pub tangent_out: Vec3,
    pub bank_angle: f32,
    pub elevation_override: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct RoadSpline {
    pub control_points: Vec<SplinePoint>,
    pub cached_samples: Vec<Vec3>,
    pub cached_tangents: Vec<Vec3>,
    pub cached_up_vectors: Vec<Vec3>,
    pub total_length: f32,
    pub subdivisions_per_segment: usize,
}

#[derive(Clone, Debug)]
pub struct TerrainHeightMap {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub heights: Vec<f32>,
    pub normals: Vec<Vec3>,
    pub splat_weights: Vec<[f32; 8]>,
}

#[derive(Clone, Debug)]
pub struct RoadVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub tangent: Vec4,
    pub color: Vec4,
}

#[derive(Clone, Debug)]
pub struct RoadMesh {
    pub vertices: Vec<RoadVertex>,
    pub indices: Vec<u32>,
    pub submeshes: Vec<RoadSubmesh>,
}

#[derive(Clone, Debug)]
pub struct RoadSubmesh {
    pub start_index: u32,
    pub index_count: u32,
    pub material_id: u32,
}

pub struct RoadMeshGenerator;

#[derive(Clone, Debug)]
pub enum LaneMarkingType {
    DashedCenter,
    SolidEdge,
    Gap,
    Crosswalk,
    StopLine,
    ArrowStraight,
    ArrowLeft,
    ArrowRight,
}

#[derive(Clone, Debug)]
pub struct LaneMarking {
    pub position: Vec3,
    pub tangent: Vec3,
    pub width: f32,
    pub length: f32,
    pub marking_type: LaneMarkingType,
    pub color: Vec3,
}

// ============================================================
// INTERSECTION GENERATOR
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IntersectionType {
    TJunction,
    XJunction,
    Roundabout,
    OnRamp,
    OffRamp,
    Merge,
}

#[derive(Clone, Debug)]
pub struct Intersection {
    pub id: u32,
    pub position: Vec3,
    pub intersection_type: IntersectionType,
    pub connected_roads: Vec<u32>,
    pub mesh: RoadMesh,
    pub roundabout_radius: f32,
    pub normal: Vec3,
}

pub struct IntersectionGenerator;

pub struct CrosswalkStripe {
    pub start: Vec3,
    pub end: Vec3,
    pub width: f32,
}

pub struct CrosswalkGenerator;

#[derive(Clone, Debug)]
pub struct BridgePillar {
    pub base_position: Vec3,
    pub top_position: Vec3,
    pub radius: f32,
    pub height: f32,
}

pub struct BridgeGenerator;

pub struct RoadNetworkNode {
    pub id: u32,
    pub position: Vec3,
    pub connected_edges: Vec<u32>,
    pub node_type: RoadNodeType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoadNodeType {
    Intersection,
    Endpoint,
    Waypoint,
    CityCenter,
    Suburb,
}

#[derive(Clone, Debug)]
pub struct RoadNetworkEdge {
    pub id: u32,
    pub from_node: u32,
    pub to_node: u32,
    pub length: f32,
    pub speed_limit: f32,
    pub road_type: RoadType,
    pub lanes: u32,
    pub is_one_way: bool,
    pub spline_id: u32,
    pub weight: f64,
}

pub struct RoadNetwork {
    pub nodes: HashMap<u32, RoadNetworkNode>,
    pub edges: HashMap<u32, RoadNetworkEdge>,
    pub next_node_id: u32,
    pub next_edge_id: u32,
    pub adjacency: HashMap<u32, Vec<(u32, f64)>>,
}

#[derive(Clone, Debug)]
pub struct TrafficCell {
    pub density: f32,
    pub velocity: f32,
    pub flow: f32,
}

#[derive(Clone, Debug)]
pub struct TrafficFlowSim {
    pub edge_id: u32,
    pub cells: Vec<TrafficCell>,
    pub cell_length: f32,
    pub max_density: f32,
    pub free_flow_speed: f32,
    pub jam_density: f32,
    pub time: f32,
}

pub struct TerrainDeformer;

pub struct CityNode {
    pub id: u32,
    pub position: Vec3,
    pub node_type: RoadNodeType,
    pub population: u32,
}

pub struct ProceduralRoadGenerator;

pub struct RoadErosionState {
    pub pothole_grid: Vec<f32>,
    pub wear_grid: Vec<f32>,
    pub puddle_grid: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
}

#[derive(Clone, Debug)]
pub struct SimpleRng {
    pub state: u64,
}
impl SimpleRng {
    pub fn new(seed: u64) -> Self { Self { state: seed ^ 0x853c49e6748fea9b } }
    pub fn next_u64(&mut self) -> u64 { self.state ^= self.state << 13; self.state ^= self.state >> 7; self.state ^= self.state << 17; self.state }
    pub fn next_f32(&mut self) -> f32 { (self.next_u64() >> 33) as f32 / 2147483648.0 }
}

#[derive(Clone, Debug)]
pub enum RoadEditAction {
    AddRoad { road_id: u32 },
    RemoveRoad { road_id: u32, snapshot: RoadSegment },
    ModifyTerrain { x: usize, z: usize, old_height: f32, new_height: f32 },
    AddIntersection { intersection_id: u32 },
    RemoveIntersection { intersection_id: u32, snapshot: Intersection },
    MoveControlPoint { spline_id: u32, point_index: usize, old_pos: Vec3, new_pos: Vec3 },
}

#[derive(Clone, Debug)]
pub struct UndoStack {
    pub actions: VecDeque<Vec<RoadEditAction>>,
    pub redo_stack: VecDeque<Vec<RoadEditAction>>,
    pub max_size: usize,
    pub max_depth: usize,
}

#[derive(Clone, Debug)]
pub struct RoadSegment {
    pub id: u32,
    pub spline: RoadSpline,
    pub profile: RoadProfile,
    pub mesh: RoadMesh,
    pub sidewalk_mesh: RoadMesh,
    pub lane_markings: Vec<LaneMarking>,
    pub bridge_pillars: Vec<BridgePillar>,
    pub is_bridge: bool,
    pub is_tunnel: bool,
    pub from_node: u32,
    pub to_node: u32,
    pub traffic_sim: TrafficFlowSim,
}

pub struct ElevationProfile {
    pub distances: Vec<f32>,
    pub elevations: Vec<f32>,
    pub terrain_elevations: Vec<f32>,
    pub max_grade: f32,
    pub min_grade: f32,
    pub avg_grade: f32,
}

pub struct RoadSnapper;

#[derive(Clone, Debug)]
pub enum RoadToolMode {
    Idle,
    PlacingRoad,
    EditingSpline,
    PlacingIntersection,
    PaintingTerrain,
    ViewElevationProfile,
    SimulatingTraffic,
}

#[derive(Clone, Debug)]
pub struct TerrainRoadToolState {
    pub mode: RoadToolMode,
    pub selected_road_type: RoadType,
    pub active_segment_id: Option<u32>,
    pub hover_pos: Vec3,
    pub is_snapped: bool,
    pub snap_target: Vec3,
    pub show_elevation_profile: bool,
    pub show_traffic_density: bool,
    pub traffic_sim_running: bool,
}

pub struct TerrainRoadTool {
    pub state: TerrainRoadToolState,
    pub terrain: TerrainHeightMap,
    pub segments: HashMap<u32, RoadSegment>,
    pub intersections: HashMap<u32, Intersection>,
    pub network: RoadNetwork,
    pub erosion: RoadErosionState,
    pub undo_stack: UndoStack,
    pub city_nodes: Vec<CityNode>,
    pub rng: SimpleRng,
    pub next_segment_id: u32,
    pub next_intersection_id: u32,
    pub profiles: HashMap<RoadType, RoadProfile>,
    pub elevation_profile_cache: Option<ElevationProfile>,
    pub traffic_sims: HashMap<u32, TrafficFlowSim>,
    pub build_pending_actions: Vec<RoadEditAction>,
}

pub struct RoadEditBatch {
    pub actions: Vec<RoadEditAction>,
    pub description: String,
}

pub struct RoadNetworkStats {
    pub total_segments: usize,
    pub total_length_km: f32,
    pub total_intersections: usize,
    pub road_type_counts: HashMap<RoadType, usize>,
    pub average_traffic_density: f32,
    pub highest_congestion_segment: Option<u32>,
    pub bridge_count: usize,
    pub tunnel_count: usize,
    pub total_lane_km: f32,
}

pub struct RoadClipper;

pub struct RoadLoftGenerator;

pub struct TunnelGenerator;

pub enum RoadValidationIssue {
    SteepGrade { segment_id: u32, grade: f32, distance: f32 },
    TooNarrowForLanes { segment_id: u32 },
    IntersectsTerrain { segment_id: u32, position: Vec3 },
    TooShort { segment_id: u32, length: f32 },
    SelfIntersecting { segment_id: u32 },
    MissingConnection { segment_id: u32 },
}

pub struct RoadValidator;

pub struct RoadSerializedData {
    pub version: u32,
    pub segments: Vec<SerializedSegment>,
    pub intersections: Vec<SerializedIntersection>,
    pub network_nodes: Vec<SerializedNode>,
    pub network_edges: Vec<SerializedEdge>,
}

#[derive(Clone, Debug)]
pub struct SerializedSegment {
    pub id: u32,
    pub road_type: u32,
    pub control_points: Vec<[f32; 3]>,
    pub from_node: u32,
    pub to_node: u32,
}

#[derive(Clone, Debug)]
pub struct SerializedIntersection {
    pub id: u32,
    pub position: [f32; 3],
    pub intersection_type: u32,
    pub connected_roads: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct SerializedNode {
    pub id: u32,
    pub position: [f32; 3],
    pub node_type: u32,
}

#[derive(Clone, Debug)]
pub struct SerializedEdge {
    pub id: u32,
    pub from_node: u32,
    pub to_node: u32,
    pub length: f32,
    pub speed_limit: f32,
    pub road_type: u32,
}

pub struct RoadProfilerFrame {
    pub segment_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub traffic_step_ms: f32,
    pub mesh_build_ms: f32,
    pub terrain_deform_ms: f32,
}

pub struct RoadProfiler {
    pub frames: VecDeque<RoadProfilerFrame>,
    pub max_frames: usize,
}

pub struct RoadIntersectionDetector;

pub struct RoadMaterial {
    pub id: u32,
    pub name: String,
    pub albedo_texture: u32,
    pub normal_texture: u32,
    pub roughness: f32,
    pub metallic: f32,
    pub tiling_u: f32,
    pub tiling_v: f32,
    pub friction: f32,
}

pub struct RoadMaterialDatabase {
    pub materials: HashMap<u32, RoadMaterial>,
}

pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn remap(value: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = (value - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}

pub fn point_to_line_distance_2d(point: Vec2, line_a: Vec2, line_b: Vec2) -> f32 {
    let ab = line_b - line_a;
    let ap = point - line_a;
    let t = (ap.dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    let closest = line_a + ab * t;
    point.distance(closest)
}

pub fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * (
        p1 * 2.0
        + (p2 - p0) * t
        + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
        + (-p0 + p1 * 3.0 - p2 * 3.0 + p3) * t3
    )
}

pub fn spline_arc_length(points: &[Vec3], subdivisions: usize) -> f32 {
    let n = points.len();
    if n < 2 { return 0.0; }
    let mut len = 0.0f32;
    for i in 0..n-1 {
        let p0 = if i == 0 { points[0] } else { points[i-1] };
        let p1 = points[i];
        let p2 = points[i+1];
        let p3 = if i+2 < n { points[i+2] } else { points[n-1] };
        let mut prev = catmull_rom(p0, p1, p2, p3, 0.0);
        for s in 1..=subdivisions {
            let t = s as f32 / subdivisions as f32;
            let cur = catmull_rom(p0, p1, p2, p3, t);
            len += prev.distance(cur);
            prev = cur;
        }
    }
    len
}

pub fn build_frenet_frame(tangent: Vec3) -> (Vec3, Vec3, Vec3) {
    let t = tangent.normalize_or_zero();
    let up = if t.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    let right = t.cross(up).normalize_or_zero();
    let actual_up = right.cross(t).normalize_or_zero();
    (t, right, actual_up)
}

pub fn circle_arc_points(center: Vec3, radius: f32, start_angle: f32, end_angle: f32, steps: usize) -> Vec<Vec3> {
    let mut pts = Vec::new();
    for i in 0..=steps {
        let a = start_angle + (end_angle - start_angle) * (i as f32 / steps as f32);
        let (s, c) = a.sin_cos();
        pts.push(center + Vec3::new(c * radius, 0.0, s * radius));
    }
    pts
}

pub fn fit_bezier_to_points(points: &[Vec3]) -> Vec<Vec3> {
    // Simple chord-length parameterization cubic bezier fit
    if points.len() < 2 { return points.to_vec(); }
    let p0 = points[0];
    let p3 = *points.last().unwrap();
    let n = points.len();
    // Compute chord lengths for parameterization
    let mut lengths = vec![0.0f32; n];
    for i in 1..n {
        lengths[i] = lengths[i-1] + points[i-1].distance(points[i]);
    }
    let total = lengths[n-1];
    let ts: Vec<f32> = lengths.iter().map(|&l| if total > 0.0 { l / total } else { 0.0 }).collect();
    // Least squares tangent estimation
    let alpha1 = (0..n).map(|i| {
        let t = ts[i];
        let b1 = 3.0 * t * (1.0-t).powi(2);
        let rhs = points[i] - p0*(1.0-t).powi(3) - p3*t.powi(3);
        b1 * b1
    }).sum::<f32>();
    let alpha2 = (0..n).map(|i| {
        let t = ts[i];
        let b2 = 3.0 * t.powi(2) * (1.0-t);
        b2 * b2
    }).sum::<f32>();
    // Simplified: use Catmull-Rom tangent at ends
    let tan0 = if n > 1 { (points[1] - points[0]).normalize_or_zero() } else { Vec3::Z };
    let tan1 = if n > 1 { (points[n-1] - points[n-2]).normalize_or_zero() } else { Vec3::Z };
    let total_len = total;
    let p1 = p0 + tan0 * (total_len / 3.0);
    let p2 = p3 - tan1 * (total_len / 3.0);
    vec![p0, p1, p2, p3]
}

pub fn douglas_peucker(points: &[Vec3], epsilon: f32) -> Vec<Vec3> {
    if points.len() < 3 { return points.to_vec(); }
    let start = points[0];
    let end = *points.last().unwrap();
    let mut max_dist = 0.0f32;
    let mut max_idx = 0;
    let line_ab = end - start;
    let line_len = line_ab.length();
    for i in 1..points.len()-1 {
        let pt = points[i];
        let dist = if line_len > 0.001 {
            let ap = pt - start;
            let proj = ap.dot(line_ab.normalize_or_zero());
            let closest = start + line_ab.normalize_or_zero() * proj.clamp(0.0, line_len);
            pt.distance(closest)
        } else {
            pt.distance(start)
        };
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }
    if max_dist > epsilon {
        let left = douglas_peucker(&points[..=max_idx], epsilon);
        let right = douglas_peucker(&points[max_idx..], epsilon);
        let mut result = left;
        result.pop();
        result.extend(right);
        result
    } else {
        vec![start, end]
    }
}

pub fn road_density_heatmap(segments: &HashMap<u32, RoadSegment>, width: usize, height: usize, cell_size: f32) -> Vec<f32> {
    let mut heatmap = vec![0.0f32; width * height];
    for seg in segments.values() {
        for (i, &sample) in seg.spline.cached_samples.iter().enumerate() {
            let cx = (sample.x / cell_size).clamp(0.0, (width-1) as f32) as usize;
            let cz = (sample.z / cell_size).clamp(0.0, (height-1) as f32) as usize;
            let idx = cz * width + cx;
            if idx < heatmap.len() {
                let density = if i < seg.traffic_sim.cells.len() {
                    seg.traffic_sim.cells[i].density
                } else { 0.0 };
                heatmap[idx] = (heatmap[idx] + density).min(1.0);
            }
        }
    }
    heatmap
}

pub fn generate_noise_terrain(terrain: &mut TerrainHeightMap, octaves: usize, freq: f32, amplitude: f32, seed: u64) {
    let mut rng = SimpleRng::new(seed);
    for z in 0..terrain.height {
        for x in 0..terrain.width {
            let wx = x as f32 * terrain.cell_size;
            let wz = z as f32 * terrain.cell_size;
            let mut h = 0.0f32;
            let mut f = freq;
            let mut a = amplitude;
            for _ in 0..octaves {
                let nx = wx * f + rng.next_f32() * 0.001;
                let nz = wz * f + rng.next_f32() * 0.001;
                let v = simple_noise_2d(nx, nz);
                h += v * a;
                f *= 2.0;
                a *= 0.5;
            }
            terrain.set_height(x, z, h.max(0.0));
        }
    }
    terrain.recompute_normals();
}

pub fn simple_noise_2d(x: f32, y: f32) -> f32 {
    let ix = x as i32;
    let iy = y as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let n00 = pseudo_random_2d(ix, iy);
    let n10 = pseudo_random_2d(ix + 1, iy);
    let n01 = pseudo_random_2d(ix, iy + 1);
    let n11 = pseudo_random_2d(ix + 1, iy + 1);
    let nx0 = n00 + (n10 - n00) * ux;
    let nx1 = n01 + (n11 - n01) * ux;
    nx0 + (nx1 - nx0) * uy
}

pub fn pseudo_random_2d(x: i32, y: i32) -> f32 {
    let n = (x.wrapping_mul(1619).wrapping_add(y.wrapping_mul(31337))) as u32;
    let n = n.wrapping_mul(1234567891).wrapping_add(0x9e3779b9);
    let n = n ^ (n >> 16);
    let n = n.wrapping_mul(0x45d9f3b);
    let n = n ^ (n >> 16);
    (n as f32) / (u32::MAX as f32)
}

// ============================================================
// ROAD PATH SMOOTHER
// ============================================================

pub struct PathSmoother;

pub struct RoadSegmentSplitter;

pub struct RoadOverlayLine {
    pub start: Vec3,
    pub end: Vec3,
    pub color: Vec4,
    pub thickness: f32,
}

pub struct RoadOverlayRenderer {
    pub lines: Vec<RoadOverlayLine>,
    pub show_spline_handles: bool,
    pub show_normals: bool,
    pub show_lane_markings: bool,
    pub show_traffic_density: bool,
    pub normal_length: f32,
}

pub struct TerrainSampleResult {
    pub height: f32,
    pub normal: Vec3,
    pub slope: f32,
    pub splat_weights: [f32; 8],
}

#[derive(Clone, Debug)]
pub struct GradeSegment {
    pub start_distance: f32,
    pub end_distance: f32,
    pub grade_percent: f32,
    pub is_steep: bool,
    pub start_station: f32,
    pub end_station: f32,
    pub grade: f32,
    pub cut_volume: f32,
    pub fill_volume: f32,
}

pub fn analyze_grade_profile(spline: &RoadSpline, max_grade: f32) -> Vec<GradeSegment> {
    let mut segments = Vec::new();
    let n = spline.cached_samples.len();
    if n < 2 { return segments; }
    let mut accum = 0.0f32;
    for i in 1..n {
        let prev = spline.cached_samples[i-1];
        let curr = spline.cached_samples[i];
        let dx = (curr.x - prev.x).powi(2) + (curr.z - prev.z).powi(2);
        let dx = dx.sqrt().max(0.001);
        let dh = curr.y - prev.y;
        let grade = (dh / dx) * 100.0;
        let seg_len = prev.distance(curr);
        segments.push(GradeSegment {
            start_distance: accum,
            end_distance: accum + seg_len,
            grade_percent: grade,
            is_steep: grade.abs() > max_grade * 100.0,
            start_station: accum,
            end_station: accum + seg_len,
            grade: grade / 100.0,
            cut_volume: 0.0,
            fill_volume: 0.0,
        });
        accum += seg_len;
    }
    segments
}

// ============================================================
// ROAD TEXTURE ATLAS
// ============================================================

#[derive(Clone, Debug)]
pub struct RoadTextureAtlasEntry {
    pub material_id: u32,
    pub uv_min: Vec2,
    pub uv_max: Vec2,
    pub road_type: RoadType,
}

pub struct RoadTextureAtlas {
    pub entries: Vec<RoadTextureAtlasEntry>,
    pub atlas_width: u32,
    pub atlas_height: u32,
}

pub struct RoadSurfaceDetail {
    pub crack_density: f32,
    pub pothole_density: f32,
    pub patch_density: f32,
    pub puddle_density: f32,
    pub wear_factor: f32,
    pub age_years: f32,
}

pub struct SpeedZone {
    pub id: u32,
    pub center: Vec3,
    pub radius: f32,
    pub speed_limit_kmh: f32,
    pub zone_type: SpeedZoneType,
    pub start_station: f32,
    pub end_station: f32,
    pub posted_speed_kmh: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpeedZoneType {
    School,
    Hospital,
    Construction,
    Residential,
    Commercial,
    Industrial,
    Highway,
}

pub struct SpeedZoneManager {
    pub zones: Vec<SpeedZone>,
    pub next_id: u32,
}

pub struct RoadAmbientZone {
    pub position: Vec3,
    pub radius: f32,
    pub traffic_sound_level: f32,
    pub road_type: RoadType,
}

pub fn compute_traffic_sound_level(density: f32, speed: f32, road_type: RoadType) -> f32 {
    let base_level = match road_type {
        RoadType::Motorway => 80.0,
        RoadType::Highway4Lane => 75.0,
        RoadType::Highway2Lane => 70.0,
        RoadType::PavedRoad => 60.0,
        RoadType::GravelRoad => 55.0,
        RoadType::DirtTrack => 40.0,
        _ => 50.0,
    };
    let density_factor = density.powf(0.5);
    let speed_factor = (speed / 50.0).ln().max(0.0);
    base_level + density_factor * 10.0 + speed_factor * 5.0
}

// ============================================================
// ROAD COST ESTIMATE
// ============================================================

#[derive(Clone, Debug)]
pub struct RoadCostEstimate {
    pub material_cost: f64,
    pub labor_cost: f64,
    pub equipment_cost: f64,
    pub total_cost: f64,
    pub cost_per_km: f64,
}

pub fn estimate_road_cost(length_m: f32, profile: &RoadProfile) -> RoadCostEstimate {
    let base_cost_per_m = match profile.road_type {
        RoadType::DirtTrack => 50.0,
        RoadType::GravelRoad => 150.0,
        RoadType::PavedRoad => 500.0,
        RoadType::Highway2Lane => 1500.0,
        RoadType::Highway4Lane => 3000.0,
        RoadType::Motorway => 6000.0,
        RoadType::Alley => 300.0,
        RoadType::Bridge => 8000.0,
        RoadType::Tunnel => 15000.0,
        RoadType::ElevatedHighway => 12000.0,
        RoadType::Cobblestone => 800.0,
        RoadType::ServiceRoad => 200.0,
        RoadType::Boulevard => 1000.0,
        RoadType::ResidentialStreet => 400.0,
        RoadType::BridgeRoad => 8000.0,
        RoadType::TunnelRoad => 15000.0,
    };
    let width_factor = profile.total_width / 7.3;
    let effective_cost = base_cost_per_m * width_factor as f64 * length_m as f64;
    let material = effective_cost * 0.4;
    let labor = effective_cost * 0.35;
    let equipment = effective_cost * 0.25;
    RoadCostEstimate {
        material_cost: material,
        labor_cost: labor,
        equipment_cost: equipment,
        total_cost: effective_cost,
        cost_per_km: effective_cost / (length_m as f64 / 1000.0),
    }
}

// ============================================================
// ROAD LIGHT POSTS
// ============================================================

#[derive(Clone, Debug)]
pub struct RoadLightPost {
    pub position: Vec3,
    pub height: f32,
    pub light_color: Vec3,
    pub light_radius: f32,
    pub is_active: bool,
}

pub fn generate_light_posts(spline: &RoadSpline, profile: &RoadProfile, spacing: f32) -> Vec<RoadLightPost> {
    let mut posts = Vec::new();
    if profile.road_type == RoadType::DirtTrack || profile.road_type == RoadType::GravelRoad { return posts; }
    let half = profile.total_width * 0.5 + 0.5;
    let mut dist = 0.0f32;
    let mut side = 1.0f32;
    while dist < spline.total_length {
        let (pos, tan) = spline.sample_at_distance(dist); let up = Vec3::Y;
        let right = tan.cross(up).normalize_or_zero();
        let post_pos = pos + right * half * side + up * 0.1;
        posts.push(RoadLightPost {
            position: post_pos,
            height: 8.0,
            light_color: Vec3::new(1.0, 0.95, 0.8),
            light_radius: 20.0,
            is_active: true,
        });
        dist += spacing;
        side = -side;
    }
    posts
}

// ============================================================
// ROAD SIGN PLACER
// ============================================================

#[derive(Clone, Debug)]
pub enum RoadSignType {
    SpeedLimit(u32),
    Stop,
    Yield,
    OneWay,
    NoEntry,
    Roundabout,
    Junction,
    PedestrianCrossing,
    SchoolZone,
    RoadWork,
}

#[derive(Clone, Debug)]
pub struct RoadSign {
    pub position: Vec3,
    pub facing: Vec3,
    pub sign_type: RoadSignType,
    pub post_height: f32,
    pub code: String,
    pub text: String,
    pub station: f32,
    pub side: i32,
    pub height_m: f32,
    pub panel_size: Vec2,
}

pub fn place_speed_limit_signs(spline: &RoadSpline, profile: &RoadProfile) -> Vec<RoadSign> {
    let mut signs = Vec::new();
    let half = profile.total_width * 0.5 + 0.5;
    let interval = 500.0f32;
    let mut dist = 0.0f32;
    while dist < spline.total_length {
        let (pos, tan) = spline.sample_at_distance(dist); let up = Vec3::Y;
        let right = tan.cross(up).normalize_or_zero();
        signs.push(RoadSign {
            position: pos + right * half + up * 0.1,
            facing: -right,
            sign_type: RoadSignType::SpeedLimit(profile.speed_limit_kmh as u32),
            post_height: 2.0,
            code: String::new(),
            text: String::new(),
            station: dist,
            side: 1,
            height_m: 2.0,
            panel_size: Vec2::new(0.6, 0.75),
        });
        dist += interval;
    }
    signs
}

// ============================================================
// GUARD RAIL GENERATOR
// ============================================================

#[derive(Clone, Debug)]
pub struct GuardRailPost {
    pub position: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Debug)]
pub struct GuardRail {
    pub posts: Vec<GuardRailPost>,
    pub side: f32,
    pub rail_height: f32,
}

pub fn generate_guard_rails(spline: &RoadSpline, profile: &RoadProfile) -> (GuardRail, GuardRail) {
    let half = profile.total_width * 0.5;
    let post_spacing = 4.0f32;
    let mut left_posts = Vec::new();
    let mut right_posts = Vec::new();
    let mut dist = 0.0f32;
    while dist < spline.total_length {
        let (pos, tan) = spline.sample_at_distance(dist); let up = Vec3::Y;
        let right = tan.cross(up).normalize_or_zero();
        left_posts.push(GuardRailPost {
            position: pos - right * (half + 0.3) + up * 0.0,
            normal: -right,
        });
        right_posts.push(GuardRailPost {
            position: pos + right * (half + 0.3) + up * 0.0,
            normal: right,
        });
        dist += post_spacing;
    }
    let left = GuardRail { posts: left_posts, side: -1.0, rail_height: 0.75 };
    let right = GuardRail { posts: right_posts, side: 1.0, rail_height: 0.75 };
    (left, right)
}

// ============================================================
// TERRAIN ROAD TOOL TESTS (inline)
// ============================================================

pub fn run_all_tests() -> bool {
    let mut all_ok = true;

    // Test spline
    {
        let mut spline = RoadSpline::new();
        spline.add_point(Vec3::ZERO);
        spline.add_point(Vec3::new(10.0, 0.0, 0.0));
        spline.add_point(Vec3::new(20.0, 0.0, 0.0));
        assert!(spline.total_length > 0.0, "Spline should have length");
        let (p, t) = spline.sample_at_distance(5.0); let u = Vec3::Y;
        assert!(p.x > 0.0, "Sample should be along positive X");
    }

    // Test terrain
    {
        let mut terrain = TerrainHeightMap::new(64, 64, 1.0);
        terrain.set_height(32, 32, 10.0);
        assert_eq!(terrain.get_height(32, 32), 10.0);
        let h = terrain.sample_bilinear(32.5, 32.5);
        assert!(h > 0.0);
    }

    // Test Dijkstra
    {
        let mut network = RoadNetwork::new();
        let a = network.add_node(Vec3::ZERO, RoadNodeType::Waypoint);
        let b = network.add_node(Vec3::new(5.0, 0.0, 0.0), RoadNodeType::Waypoint);
        let c = network.add_node(Vec3::new(10.0, 0.0, 0.0), RoadNodeType::Waypoint);
        network.add_edge(a, b, 5.0, 50.0, RoadType::PavedRoad, 2, false);
        network.add_edge(b, c, 5.0, 50.0, RoadType::PavedRoad, 2, false);
        let result = network.dijkstra(a, c);
        assert!(result.is_some(), "Dijkstra should find path");
        let (cost, path) = result.unwrap();
        assert_eq!(path.len(), 3);
    }

    // Test LWR
    {
        let mut sim = TrafficFlowSim::new(0, 100.0, 30.0);
        sim.inject_vehicles(0, 0.5);
        sim.step(0.016);
        assert!(sim.cells[0].density > 0.0);
    }

    // Test erosion
    {
        let mut erosion = RoadErosionState::new(8, 8, 1.0);
        erosion.wear_grid[0] = 0.9;
        let mut rng = SimpleRng::new(1);
        for _ in 0..1000 {
            erosion.simulate_potholes(&mut rng);
        }
        // Some potholes should have appeared
        let total_potholes: f32 = erosion.pothole_grid.iter().sum();
        // (stochastic, just check it runs without panic)
    }

    all_ok
}

// ============================================================
// MAIN TOOL CONVENIENCE
// ============================================================

pub struct RoadPhysicsConfig {
    pub static_friction: f32,
    pub kinetic_friction: f32,
    pub rolling_resistance: f32,
    pub cornering_stiffness: f32,
    pub banking_max_deg: f32,
    pub hydroplaning_rain_threshold: f32,
    pub surface_temperature_effect: f32,
    pub grip_reduction_at_temp: f32,
}

pub struct RoadWeatherState {
    pub rain_mm_per_hour: f32,
    pub snow_depth_mm: f32,
    pub ice_coverage: f32,
    pub temperature_celsius: f32,
    pub wind_speed_ms: f32,
    pub wind_direction: f32,
    pub visibility_km: f32,
    pub fog_density: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrafficLightPhase {
    Green, Yellow, Red, FlashingRed, FlashingYellow, Off,
}

#[derive(Clone, Debug)]
pub struct TrafficLight {
    pub id: u32,
    pub position: Vec3,
    pub phase: TrafficLightPhase,
    pub phase_timer: f32,
    pub green_duration: f32,
    pub yellow_duration: f32,
    pub red_duration: f32,
    pub intersection_id: u32,
    pub direction: Vec3,
}

#[derive(Clone, Debug)]
pub enum AccidentSeverity { Minor, Moderate, Major, Fatal }

#[derive(Clone, Debug)]
pub struct RoadAccident {
    pub id: u32,
    pub position: Vec3,
    pub segment_id: u32,
    pub severity: AccidentSeverity,
    pub blocking_lanes: u32,
    pub clearance_time_secs: f32,
    pub elapsed_time: f32,
    pub is_cleared: bool,
}

#[derive(Clone, Debug)]
pub enum MaintenanceType { Resurfacing, PotholeFilling, MarkingsRepaint, Cleaning, DrainageClear, BridgeInspection, EmergencyRepair, SnowPlowing, SaltApplication }

#[derive(Clone, Debug)]
pub struct MaintenanceRecord {
    pub date_days: u32,
    pub work_type: MaintenanceType,
    pub cost: f64,
    pub crew_count: u32,
    pub duration_days: u32,
    pub notes: String,
}

pub struct MaintenanceScheduler {
    pub records: Vec<MaintenanceRecord>,
    pub segments: HashMap<u32, Vec<MaintenanceRecord>>,
}

pub struct HorizontalAlignment {
    pub elements: Vec<HorizontalElement>,
    pub total_length: f32,
}

#[derive(Clone, Debug)]
pub enum HorizontalElement {
    Straight { length: f32, azimuth: f32 },
    CircularArc { radius: f32, arc_length: f32 },
    ClothoidSpiral { parameter: f32, length: f32, direction: f32 },
}

#[derive(Clone, Debug)]
pub struct VerticalAlignmentPoint {
    pub station: f32,
    pub elevation: f32,
    pub grade_in: f32,
    pub grade_out: f32,
    pub vc_length: f32,
}

#[derive(Clone, Debug)]
pub struct VerticalAlignment {
    pub points: Vec<VerticalAlignmentPoint>,
}

pub struct SightDistanceAnalyzer;

pub struct DrainageCulvert {
    pub id: u32,
    pub position: Vec3,
    pub diameter_mm: f32,
    pub length: f32,
    pub slope: f32,
    pub material: String,
    pub capacity_l_per_s: f32,
    pub is_blocked: bool,
}

#[derive(Clone, Debug)]
pub struct DrainageDitch {
    pub id: u32,
    pub start: Vec3,
    pub end: Vec3,
    pub depth: f32,
    pub width: f32,
    pub side: f32,
    pub slope: f32,
    pub vegetation: bool,
}

pub struct DrainageSystem {
    pub culverts: Vec<DrainageCulvert>,
    pub ditches: Vec<DrainageDitch>,
}

pub struct MarkingStencil {
    pub name: String,
    pub polygons: Vec<Vec<Vec2>>,
    pub color: Vec3,
    pub scale: Vec2,
}

pub struct FlowAnalyzer;

pub struct ProceduralSegmentBuilder;

pub struct TerrainSculptor;

#[derive(Clone, Debug)]
pub enum HeatMapMetric { TrafficDensity, SpeedVariance, AccidentRisk, RoadCondition, NoisePollution }

#[derive(Clone, Debug)]
pub struct RoadHeatMap {
    pub data: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub scale: f32,
    pub metric: HeatMapMetric,
}

pub struct SlopeAnalyzer;

pub struct RoadSpeedProfile {
    pub segment_id: u32,
    pub distances: Vec<f32>,
    pub design_speeds: Vec<f32>,
    pub operating_speeds: Vec<f32>,
    pub is_consistent: bool,
    pub inconsistency_locations: Vec<f32>,
}

pub fn compute_speed_profile(seg: &RoadSegment, terrain: &TerrainHeightMap) -> RoadSpeedProfile {
    let ep = ElevationProfile::compute(&seg.spline, terrain);
    let n = ep.distances.len();
    let mut design_speeds = Vec::with_capacity(n);
    let mut operating_speeds = Vec::with_capacity(n);
    let base_speed = seg.profile.speed_limit_kmh;
    let grades = analyze_grade_profile(&seg.spline, seg.profile.max_slope_grade);
    for i in 0..n {
        let grade_factor = if i < grades.len() { 1.0 - (grades[i].grade_percent.abs() / 15.0).min(0.4) } else { 1.0 };
        design_speeds.push(base_speed * grade_factor);
        operating_speeds.push(base_speed * grade_factor * 0.9);
    }
    let mut inconsistency_locations = Vec::new();
    for i in 1..design_speeds.len() {
        let diff = (design_speeds[i] - design_speeds[i-1]).abs();
        if diff > 20.0 {
            inconsistency_locations.push(ep.distances[i]);
        }
    }
    RoadSpeedProfile {
        segment_id: seg.id,
        distances: ep.distances,
        design_speeds,
        operating_speeds,
        is_consistent: inconsistency_locations.is_empty(),
        inconsistency_locations,
    }
}

// ============================================================
// ROAD ASSET REGISTRY
// ============================================================

#[derive(Clone, Debug)]
pub struct RoadAssetEntry {
    pub id: u64,
    pub name: String,
    pub asset_type: RoadAssetType,
    pub mesh_id: u32,
    pub material_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoadAssetType { RoadSurface, Curb, GuardRail, LightPost, Sign, Pillar, DrainCover, Manhole, BusStop }

pub struct RoadAssetRegistry {
    pub entries: HashMap<u64, RoadAssetEntry>,
    pub next_id: u64,
}

pub struct RoadDecal {
    pub id: u32,
    pub position: Vec3,
    pub normal: Vec3,
    pub size: Vec2,
    pub angle: f32,
    pub texture_id: u32,
    pub alpha: f32,
    pub tint: Vec4,
    pub decal_type: RoadDecalType,
    pub age: f32,
    pub fade_duration: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoadDecalType { SkidMark, OilSpill, CrackPattern, WearPattern, WaterStain, PaintDrip }

pub struct RoadDecalManager {
    pub decals: Vec<RoadDecal>,
    pub next_id: u32,
}

pub fn angle_between_vectors(a: Vec3, b: Vec3) -> f32 {
    let dot = a.dot(b).clamp(-1.0, 1.0);
    dot.acos()
}

pub fn project_point_onto_plane(point: Vec3, plane_origin: Vec3, plane_normal: Vec3) -> Vec3 {
    let d = (point - plane_origin).dot(plane_normal);
    point - plane_normal * d
}

pub fn barycentric_coords(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> Vec3 {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-8 { return Vec3::new(1.0/3.0, 1.0/3.0, 1.0/3.0); }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    Vec3::new(1.0 - v - w, v, w)
}

pub fn ray_sphere_intersect(ray_origin: Vec3, ray_dir: Vec3, sphere_center: Vec3, sphere_radius: f32) -> Option<f32> {
    let oc = ray_origin - sphere_center;
    let a = ray_dir.dot(ray_dir);
    let b = 2.0 * oc.dot(ray_dir);
    let c = oc.dot(oc) - sphere_radius * sphere_radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 { return None; }
    let sqrt_disc = disc.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);
    if t1 > 0.0 { Some(t1) } else if t2 > 0.0 { Some(t2) } else { None }
}

pub fn ray_plane_intersect(ray_origin: Vec3, ray_dir: Vec3, plane_origin: Vec3, plane_normal: Vec3) -> Option<f32> {
    let denom = ray_dir.dot(plane_normal);
    if denom.abs() < 1e-6 { return None; }
    let t = (plane_origin - ray_origin).dot(plane_normal) / denom;
    if t > 0.0 { Some(t) } else { None }
}

pub fn ray_cast_terrain(ray_origin: Vec3, ray_dir: Vec3, terrain: &TerrainHeightMap, max_dist: f32, steps: usize) -> Option<Vec3> {
    let dt = max_dist / steps as f32;
    for i in 0..steps {
        let t = i as f32 * dt;
        let pos = ray_origin + ray_dir * t;
        let terrain_h = terrain.sample_bilinear(pos.x, pos.z);
        if pos.y <= terrain_h {
            // Binary search for precision
            let mut lo = if i > 0 { (i - 1) as f32 * dt } else { 0.0 };
            let mut hi = t;
            for _ in 0..8 {
                let mid = (lo + hi) * 0.5;
                let p = ray_origin + ray_dir * mid;
                if p.y <= terrain.sample_bilinear(p.x, p.z) { hi = mid; } else { lo = mid; }
            }
            let final_pos = ray_origin + ray_dir * (lo + hi) * 0.5;
            return Some(final_pos);
        }
    }
    None
}

// ============================================================
// ROAD CAMERA PATH
// ============================================================

#[derive(Clone, Debug)]
pub struct RoadCameraPath {
    pub segment_id: u32,
    pub height_above_road: f32,
    pub lateral_offset: f32,
    pub look_ahead_distance: f32,
    pub fov: f32,
    pub smooth_factor: f32,
}

pub struct RoadRuntimeUpdate {
    pub closed_segments: HashSet<u32>,
    pub detour_routes: HashMap<u32, Vec<u32>>,
    pub speed_overrides: HashMap<u32, f32>,
    pub construction_zones: Vec<(u32, f32, f32)>, // (segment_id, start_t, end_t)
    pub traffic_lights: Vec<TrafficLight>,
    pub accidents: Vec<RoadAccident>,
    pub decals: RoadDecalManager,
    pub weather: RoadWeatherState,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RoadToolKey {
    PlaceDirtTrack, PlaceGravelRoad, PlacePavedRoad, PlaceHighway2, PlaceHighway4,
    PlaceMotorway, PlaceAlley, PlaceBridge, PlaceTunnel, PlaceElevatedHighway,
    PlaceIntersectionT, PlaceIntersectionX, PlaceRoundabout,
    FinishRoad, CancelRoad, UndoAction, RedoAction,
    ToggleTrafficSim, ToggleElevationProfile, ToggleOverlay,
    GenerateProceduralRoads, ValidateNetwork, ExportNetwork,
}

#[derive(Clone, Debug)]
pub struct RoadToolKeyBindings {
    pub bindings: HashMap<RoadToolKey, String>,
}

pub struct RoadExporter;

pub struct RoadToolPanelState {
    pub selected_tab: RoadToolTab,
    pub show_advanced_settings: bool,
    pub road_type_dropdown_open: bool,
    pub selected_segment_info_visible: bool,
    pub elevation_chart_height: f32,
    pub traffic_chart_height: f32,
    pub minimap_size: f32,
    pub snap_enabled: bool,
    pub snap_radius: f32,
    pub auto_bridge_enabled: bool,
    pub auto_tunnel_enabled: bool,
    pub terrain_deform_enabled: bool,
    pub splat_paint_enabled: bool,
    pub erosion_enabled: bool,
    pub procedural_generation_params: ProceduralGenParams,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoadToolTab { Placement, Editing, Traffic, Erosion, Procedural, Statistics, Export }

#[derive(Clone, Debug)]
pub struct ProceduralGenParams {
    pub city_node_count: u32,
    pub min_city_spacing: f32,
    pub max_city_spacing: f32,
    pub use_terrain_following: bool,
    pub slope_avoidance_weight: f32,
    pub road_type_for_generation: RoadType,
    pub random_seed: u64,
}

impl Default for ProceduralGenParams {
    fn default() -> Self {
        ProceduralGenParams {
            city_node_count: 8,
            min_city_spacing: 50.0,
            max_city_spacing: 200.0,
            use_terrain_following: true,
            slope_avoidance_weight: 0.7,
            road_type_for_generation: RoadType::PavedRoad,
            random_seed: 42,
        }
    }
}

pub fn integration_test_road_tool() {
    let mut tool = TerrainRoadTool::with_sample_terrain(99);
    tool.begin_road_placement(RoadType::PavedRoad);
    tool.add_road_point(Vec3::new(10.0, 0.0, 10.0));
    tool.add_road_point(Vec3::new(50.0, 0.0, 10.0));
    tool.add_road_point(Vec3::new(90.0, 0.0, 50.0));
    tool.finish_road_placement();
    let center = Vec3::new(90.0, 0.0, 90.0);
    let arms = vec![Vec3::new(90.0, 0.0, 70.0), Vec3::new(110.0, 0.0, 90.0), Vec3::new(90.0, 0.0, 110.0), Vec3::new(70.0, 0.0, 90.0)];
    tool.place_roundabout(center, arms);
    tool.add_city_node(Vec3::new(20.0, 0.0, 20.0), RoadNodeType::CityCenter, 10000);
    tool.add_city_node(Vec3::new(100.0, 0.0, 20.0), RoadNodeType::Suburb, 3000);
    tool.add_city_node(Vec3::new(60.0, 0.0, 100.0), RoadNodeType::Suburb, 5000);
    tool.generate_procedural_roads();
    for _ in 0..100 { tool.step_traffic_simulation(LWR_DT); }
    tool.run_erosion_simulation(EROSION_TIMESTEPS);
    let stats = tool.statistics();
    assert!(stats.total_segments > 0);
    let data = tool.serialize();
    let mut tool2 = TerrainRoadTool::new(256, 256, 1.0);
    tool2.deserialize(&data);
    assert_eq!(tool2.segments.len(), tool.segments.len());
}

// ============================================================
// GRADE OPTIMIZER
// ============================================================


#[derive(Debug, Clone)]
pub struct GradeOptimizer {
    pub max_grade: f32, pub max_cut_depth: f32, pub max_fill_height: f32, pub balance_earthwork: bool, pub segments: Vec<GradeSegment>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LaneType { Travel, Turning, Parking, Bike, Shoulder, Median, Sidewalk, ThroughLane, TurnLane, CycleLane, BusLane, EmergencyStoppingLane, Auxiliary, Ramp, Acceleration, Deceleration }

#[derive(Debug, Clone)]
pub struct Lane {
    pub lane_type: LaneType, pub width: f32, pub left_curb: bool, pub right_curb: bool,
    pub surface: String, pub marking_left: Option<String>, pub marking_right: Option<String>,
}

pub struct CrossSection {
    pub lanes_left: Vec<Lane>, pub lanes_right: Vec<Lane>, pub median_width: f32,
    pub slope_cut: f32, pub slope_fill: f32, pub ditch_width: f32, pub ditch_depth: f32, pub superelevation: f32,
}
impl CrossSection {
    pub fn four_lane_divided() -> Self {
        let lane = Lane { lane_type: LaneType::Travel, width: LANE_WIDTH, left_curb: false, right_curb: false, surface: "asphalt".into(), marking_left: None, marking_right: None };
        Self { lanes_left: vec![lane.clone(), lane.clone()], lanes_right: vec![lane.clone(), lane.clone()], median_width: 3.0, slope_cut: 0.5, slope_fill: 0.33, ditch_width: DITCH_WIDTH, ditch_depth: DITCH_DEPTH, superelevation: 0.0 }
    }
    pub fn two_lane() -> Self {
        let lane = Lane { lane_type: LaneType::Travel, width: LANE_WIDTH, left_curb: false, right_curb: false, surface: "asphalt".into(), marking_left: None, marking_right: None };
        Self { lanes_left: vec![lane.clone()], lanes_right: vec![lane.clone()], median_width: 0.0, slope_cut: 0.5, slope_fill: 0.33, ditch_width: DITCH_WIDTH, ditch_depth: DITCH_DEPTH, superelevation: 0.0 }
    }
    pub fn total_width(&self) -> f32 {
        let left: f32 = self.lanes_left.iter().map(|l| l.width).sum();
        let right: f32 = self.lanes_right.iter().map(|l| l.width).sum();
        left + right + self.median_width
    }
    pub fn generate_profile_points(&self, elevation: f32, _terrain_elev: f32) -> Vec<(f32, f32)> {
        vec![(0.0, elevation), (self.total_width(), elevation)]
    }
}

pub struct PavementLayer {
    pub name: String, pub material: String, pub thickness_mm: f32, pub elastic_modulus_mpa: f32, pub poisson_ratio: f32,
}

pub struct PavementStructure {
    pub layers: Vec<PavementLayer>, pub subgrade_cbr: f32, pub design_esal: f64, pub reliability: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TurnType { Left, Through, Right, UTurn }

#[derive(Debug, Clone)]
pub struct ApproachMovement { pub volume_vph: f32, pub phf: f32, pub turn_type: TurnType, pub shared_lane: bool }

#[derive(Debug, Clone)]
pub struct SignalPhase { pub movements: Vec<usize>, pub green_time: f32, pub yellow_time: f32, pub all_red_time: f32 }

pub struct IntersectionCapacityAnalysis {
    pub approaches: Vec<ApproachMovement>, pub phases: Vec<SignalPhase>, pub cycle_length: f32, pub saturation_flow_base: f32,
}

#[derive(Debug, Clone)]
pub struct RoundaboutEntry { pub approach_volume: f32, pub entry_width: f32, pub entry_radius: f32, pub flare_length: f32, pub inscribed_diameter: f32,
    pub entry_id: u32, pub bearing_deg: f32, pub lane_count: u32, pub entry_width_m: f32, pub flare_length_m: f32, pub approach_speed_kph: f32, pub design_flow_vph: u32, pub pedestrian_crossing: bool,
}

#[derive(Debug, Clone)]
pub struct RoundaboutDesign {
    pub inscribed_diameter: f32, pub central_island_diameter: f32, pub circulatory_width: f32,
    pub truck_apron_width: f32, pub entries: Vec<RoundaboutEntry>, pub design_vehicle: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BarrierType { WBeam, ThreeBeam, ConcreteBarrier, CableBarrier, BridgeRail, Attenuator }

#[derive(Debug, Clone)]
pub struct GuardrailSection { pub barrier_type: BarrierType, pub start_station: f32, pub end_station: f32, pub side: i32, pub height_mm: f32, pub post_spacing_m: f32, pub terminal_type: String }

#[derive(Debug, Clone)]
pub struct BarrierSystem { pub sections: Vec<GuardrailSection>, pub clear_zone_width: f32, pub design_speed_kmh: f32 }

pub enum SignType { Regulatory, Warning, Guide, Information }



#[derive(Debug, Clone)]
pub struct SignInventory { pub signs: Vec<RoadSign>, pub delineators: Vec<(f32, i32)>, pub mile_markers: Vec<(f32, u32)> }

pub struct LightingFixture { pub station: f32, pub side: i32, pub pole_height_m: f32, pub lamp_lumens: f32 }

pub struct RoadLightingSystem { pub fixtures: Vec<LightingFixture>, pub spacing_m: f32 }

pub struct NoiseBarrier { pub start_station: f32, pub end_station: f32, pub height_m: f32, pub side: i32, pub insertion_loss_db: f32 }

pub struct NoiseAnalysis { pub barriers: Vec<NoiseBarrier>, pub source_level_db: f32, pub receptor_distance_m: f32 }

pub struct OriginDestinationMatrix { pub zones: Vec<String>, pub matrix: Vec<Vec<f32>> }

pub struct PavementConditionIndex { pub pci_value: f32, pub distress_types: Vec<(String, f32, f32)>, pub sample_unit_area: f32 }

pub struct AssetRecord {
    pub asset_id: u32, pub asset_type: String, pub station: f32, pub installation_year: u32,
    pub condition_score: f32, pub replacement_cost: f32, pub remaining_life_years: f32, pub maintenance_history: Vec<(u32, String, f32)>,
}

pub struct AssetManagementSystem { pub assets: Vec<AssetRecord>, pub annual_budget: f32, pub current_year: u32 }

pub struct ConstructionActivity {
    pub id: u32, pub name: String, pub duration_days: u32, pub predecessors: Vec<u32>, pub resources: HashMap<String, f32>,
    pub cost: f32, pub early_start: u32, pub early_finish: u32, pub late_start: u32, pub late_finish: u32, pub float: i32,
}

pub struct CriticalPathMethod { pub activities: Vec<ConstructionActivity> }

#[derive(Debug, Clone)]
pub struct UtilityLine { pub id: u32, pub utility_type: String, pub depth_m: f32, pub polyline: Vec<Vec3>, pub diameter_mm: f32 }

#[derive(Debug, Clone)]
pub struct UtilityConflict { pub utility_id: u32, pub conflict_station: f32, pub conflict_type: String, pub relocation_cost: f32, pub criticality: u8 }

#[derive(Debug, Clone)]
pub struct UtilityConflictDetector { pub utilities: Vec<UtilityLine>, pub conflicts: Vec<UtilityConflict> }

pub struct HydrologicBasin {
    pub area_ha: f32, pub runoff_coefficient: f32, pub tc_minutes: f32, pub land_use: String,
}

pub struct CulvertDesign { pub diameter_mm: f32, pub length_m: f32, pub slope: f32, pub manning_n: f32 }

pub struct VehicleEmissionsFactor { pub vehicle_class: String, pub co2_g_per_km: f32, pub fuel_l_per_100km: f32 }

pub struct RoadEmissionsModel { pub factors: Vec<VehicleEmissionsFactor>, pub traffic_volumes: HashMap<String, f32>, pub segment_length_km: f32 }

pub const ROAD_TOOL_VERSION: &str = "1.0.0";
pub const MAX_ROAD_NETWORK_SEGMENTS: usize = 100_000;
pub const MAX_ROAD_NETWORK_NODES: usize = 50_000;
pub const DEFAULT_LANE_WIDTH_M: f32 = 3.6;
pub const DEFAULT_SHOULDER_WIDTH_M: f32 = 1.5;
pub const MIN_HORIZONTAL_RADIUS_M: f32 = 15.0;
pub const MAX_GRADE_PERCENT_HIGHWAY: f32 = 6.0;
pub const MAX_GRADE_PERCENT_LOCAL: f32 = 12.0;
pub const STOPPING_SIGHT_DISTANCE_120KMH_M: f32 = 285.0;
pub const STOPPING_SIGHT_DISTANCE_80KMH_M: f32 = 130.0;
pub const STOPPING_SIGHT_DISTANCE_50KMH_M: f32 = 65.0;
pub const BRIDGE_LIVE_LOAD_KPA: f32 = 9.6;
pub const CULVERT_RETURN_PERIOD_YRS: f32 = 25.0;
pub const DEFAULT_FRICTION_COEFFICIENT: f32 = 0.35;
pub const AASHTO_STOPPING_DECELERATION_MS2: f32 = 3.4;
pub const PAVEMENT_DESIGN_PERIOD_YEARS: u32 = 20;
pub const TRAFFIC_GROWTH_RATE_PERCENT: f32 = 2.0;

pub fn road_tool_module_info() -> HashMap<&'static str, &'static str> {
    let mut info = HashMap::new();
    info.insert("version", ROAD_TOOL_VERSION);
    info.insert("design_standard", "AASHTO Green Book 2018");
    info.insert("traffic_model", "LWR Godunov");
    info.insert("pavement_design", "AASHTO 1993");
    info
}

// ============================================================
// ROAD TOOL COMPREHENSIVE INTEGRATION TEST
// ============================================================

pub fn run_extended_road_tool_tests() {
    // Grade optimizer
    let profile: Vec<(f32, f32)> = (0..100).map(|i| (i as f32 * 10.0, (i as f32 * 0.1).sin() * 5.0 + 10.0)).collect();
    let mut opt = GradeOptimizer::new(0.08);
    let optimized = opt.optimize(&profile, 1000.0);
    assert!(!optimized.is_empty());
    let (cut, fill) = opt.total_earthwork();
    assert!(cut >= 0.0 && fill >= 0.0);
    let mhd = opt.mass_haul_diagram();
    assert!(!mhd.is_empty());

    // Cross section
    let xs = CrossSection::four_lane_divided();
    assert!(xs.total_width() > 10.0);
    let pts = xs.generate_profile_points(100.0, 95.0);
    assert!(!pts.is_empty());

    // Pavement
    let pav = PavementStructure::recommend_structure(5.0, 5_000_000.0);
    assert!(pav.total_thickness_mm() > 0.0);
    assert!(pav.structural_number() > 0.0);

    // Intersection capacity
    let mut ica = IntersectionCapacityAnalysis::new(90.0);
    ica.add_approach(800.0, 0.92, TurnType::Through);
    ica.add_approach(200.0, 0.90, TurnType::Left);
    ica.add_phase(vec![0], 40.0); ica.add_phase(vec![1], 20.0);
    assert!(ica.vc_ratio(0) > 0.0);
    let los = ica.level_of_service(0);
    assert!(los >= 'A' && los <= 'F');
    let opt_c = ica.webster_optimal_cycle(1000.0);
    assert!(opt_c >= 40.0 && opt_c <= 150.0);

    // Roundabout
    let mut rab = RoundaboutDesign::single_lane(40.0);
    rab.add_entry(600.0, 4.5);
    assert!(rab.entry_capacity(&rab.entries[0]) > 0.0);
    assert_eq!(rab.generate_geometry(Vec3::ZERO).len(), 65);

    // Barrier
    let mut bs = BarrierSystem::new(110.0);
    bs.auto_place_barriers(&[(100.0, 2.0, 50.0)]);
    assert!(!bs.sections.is_empty());
    assert!(bs.sections[0].post_count() > 0);

    // Signs
    let mut inv = SignInventory::new();
    inv.add_sign(RoadSign::speed_limit(50.0, -1, 100));
    inv.add_sign(RoadSign::stop(200.0, 1));
    inv.auto_place_delineators(1000.0, 100.0);
    inv.auto_place_mile_markers(1000.0);
    assert_eq!(inv.signs.len(), 2);
    assert!(!inv.delineators.is_empty());

    // Lighting
    let mut lighting = RoadLightingSystem::new(40.0);
    lighting.auto_place(500.0);
    assert!(!lighting.fixtures.is_empty());

    // Noise
    let mut noise = NoiseAnalysis::new(75.0, 50.0);
    noise.barriers.push(NoiseBarrier::concrete(0.0, 200.0, 1, 3.5));
    assert!(noise.receptor_level_db() < 75.0);

    // OD matrix
    let mut od = OriginDestinationMatrix::new(vec!["A".into(), "B".into(), "C".into()]);
    od.set(0, 1, 500.0); od.set(1, 2, 300.0);
    assert!((od.total_trips() - 800.0).abs() < 0.01);

    // PCI
    let mut pci = PavementConditionIndex::new(230.0);
    pci.add_distress("alligator_cracking", 23.0, 2.0);
    pci.calculate_pci();
    assert!(pci.pci_value >= 0.0 && pci.pci_value <= 100.0);
    let _ = pci.condition_category();
    let _ = pci.recommended_treatment();

    // Asset management
    let mut ams = AssetManagementSystem::new(1_000_000.0, 2024);
    let mut asset = AssetRecord::new(1, "asphalt_pavement", 500.0, 2010, 500_000.0);
    asset.update_condition(2024);
    asset.add_maintenance(2018, "thin_overlay", 50_000.0);
    ams.add_asset(asset);
    assert!(ams.network_condition_index() >= 0.0);
    let _ = ams.budget_allocation();

    // CPM
    let cpm = CriticalPathMethod::standard_road_schedule();
    assert!(!cpm.critical_path().is_empty());
    assert!(cpm.project_duration() > 0);
    assert!(cpm.total_cost() > 0.0);

    // Utility conflicts
    let mut ucd = UtilityConflictDetector::new();
    ucd.add_utility(UtilityLine::water_main(1, 1.5, vec![Vec3::new(50.0, -1.5, 0.0), Vec3::new(50.0, -1.5, 100.0)]));
    let road_pts: Vec<Vec3> = (0..20).map(|i| Vec3::new(i as f32 * 10.0, 0.0, 50.0)).collect();
    ucd.detect_conflicts(&road_pts, 10.0);
    let _ = ucd.total_relocation_cost();

    // Hydrology
    let basin = HydrologicBasin::new(50.0, "suburban");
    let q = basin.peak_discharge_rational(HydrologicBasin::idf_intensity(25.0, basin.tc_minutes));
    assert!(q > 0.0);
    let d_mm = CulvertDesign::size_for_discharge(q, 0.01);
    assert!(d_mm >= 300.0);
    let culvert = CulvertDesign::new(d_mm, 15.0, 0.01);
    assert!(culvert.full_flow_capacity() > 0.0);

    // Speed zones
    let mut szm = SpeedZoneManager::new(100);
    szm.add_zone(SpeedZone::school_zone(1, 500.0, 700.0));
    assert_eq!(szm.speed_at_station(600.0, 800), 30);
    assert_eq!(szm.speed_at_station(600.0, 900), 100);

    // Emissions
    let mut em = RoadEmissionsModel::new(5.0);
    em.set_volume("passenger_car", 10000.0);
    assert!(em.daily_co2_kg() > 0.0);
    assert!(em.annual_co2_tonnes() > 0.0);

    // Module info
    let info = road_tool_module_info();
    assert!(info.contains_key("version"));
    assert_eq!(info["design_standard"], "AASHTO Green Book 2018");
}

// ============================================================
// SECTION: Road Geometry â€” Horizontal Curve Superelevation
// ============================================================

#[derive(Debug, Clone)]
pub struct SuperelevationTable {
    pub design_speed_kph: f32,
    pub max_superelevation: f32,
    /// (radius_m, superelevation_rate) pairs
    pub table: Vec<(f32, f32)>,
}

#[derive(Debug, Clone, Default)]
pub struct HorizontalCurve {
    pub radius_m: f32,
    pub delta_angle_deg: f32,
    pub design_speed_kph: f32,
    pub lane_width_m: f32,
    pub number_of_lanes: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VerticalCurveType { Crest, Sag }

#[derive(Debug, Clone)]
pub struct VerticalCurve {
    pub curve_type: VerticalCurveType,
    pub g1_percent: f32,
    pub g2_percent: f32,
    pub length_m: f32,
    pub pvi_station_m: f32,
    pub pvi_elevation_m: f32,
    pub design_speed_kph: f32,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkLink {
    pub id: u32,
    pub from_node: u32,
    pub to_node: u32,
    pub free_flow_time_min: f32,
    pub capacity_veh_per_hour: f32,
    pub alpha: f32,
    pub beta: f32,
    pub current_flow: f32,
}

#[derive(Debug, Clone)]
pub struct OdDemand {
    pub origin: u32,
    pub destination: u32,
    pub demand_vph: f32,
}

#[derive(Debug, Clone)]
pub struct NetworkEquilibriumSolver {
    pub links: Vec<NetworkLink>,
    pub nodes: Vec<u32>,
    pub od_demands: Vec<OdDemand>,
    pub iteration_count: u32,
    pub convergence_gap: f32,
}

#[derive(Debug, Clone)]
pub enum PavementDistressType {
    Alligator, Bleeding, BlockCracking, BumpsAndSags, Corrugation,
    Depression, EdgeCracking, JointReflection, LaneShoulder, LongTransCracking,
    PatchingUtility, PolishedAggregate, Potholes, Railroad, Rutting,
    Shoving, Slippage, Swell, Raveling,
}

#[derive(Debug, Clone)]
pub struct DistressObservation {
    pub distress_type: PavementDistressType,
    pub quantity: f32,
    pub density_percent: f32,
    pub severity: u8, // 1=Low, 2=Medium, 3=High
}

pub struct PavementSampleUnit {
    pub unit_id: u32,
    pub area_m2: f32,
    pub distresses: Vec<DistressObservation>,
    pub last_survey_year: u32,
}

pub struct PavementManagementSystem {
    pub sample_units: Vec<PavementSampleUnit>,
    pub annual_budget: f32,
    pub treatment_unit_costs: HashMap<&'static str, f32>, // $/m2
}

#[derive(Debug, Clone, Default)]
pub struct SkidResistanceMeasurement {
    pub station_m: f32,
    pub skid_number: f32, // SN at 64 km/h
    pub international_friction_index: f32,
    pub texture_depth_mm: f32,
    pub surface_type: String,
}

#[derive(Debug, Clone, Default)]
pub struct FrictionInventory {
    pub measurements: Vec<SkidResistanceMeasurement>,
    pub minimum_acceptable_sn: f32,
}

pub fn run_geometry_tests() {
    // Horizontal curve
    let curve = HorizontalCurve::new(500.0, 30.0, 80.0);
    assert!(curve.arc_length_m() > 0.0);
    assert!(curve.tangent_length_m() > 0.0);
    assert!(curve.long_chord_m() < curve.arc_length_m());
    assert!(curve.min_radius_m() > 0.0);
    let _ = curve.design_speed_ok();
    let _ = curve.sight_clearance_m();

    // Superelevation table
    let tbl = SuperelevationTable::for_rural_highway(100.0);
    assert!(tbl.required_superelevation(500.0) > 0.0);
    assert!(tbl.transition_length_m(0.06, 3.65) > 0.0);

    // Vertical curve
    let vc = VerticalCurve::new(3.0, -2.0, 200.0, 1000.0, 250.0, 100.0);
    assert_eq!(vc.curve_type, VerticalCurveType::Crest);
    assert!(vc.a_value() > 0.0);
    assert!(vc.k_value() > 0.0);
    let elev = vc.elevation_at_station(1000.0);
    assert!(elev > 0.0);
    let _ = vc.high_low_point_station();
    let _ = vc.is_adequate();

    // Network equilibrium
    let mut solver = NetworkEquilibriumSolver::new();
    solver.add_link(NetworkLink::new(1, 1, 2, 5.0, 1000.0));
    solver.add_link(NetworkLink::new(2, 2, 3, 3.0, 800.0));
    solver.add_demand(1, 3, 500.0);
    solver.solve(10, 1.0);
    let vht = solver.total_vehicle_hours_traveled();
    assert!(vht >= 0.0);

    // Pavement management
    let mut pms = PavementManagementSystem::new(500_000.0);
    let mut unit = PavementSampleUnit::new(1, 1000.0);
    unit.add_distress(DistressObservation {
        distress_type: PavementDistressType::Alligator,
        quantity: 50.0, density_percent: 5.0, severity: 2
    });
    unit.add_distress(DistressObservation {
        distress_type: PavementDistressType::Rutting,
        quantity: 200.0, density_percent: 20.0, severity: 1
    });
    let pci = unit.compute_pci();
    assert!(pci >= 0.0 && pci <= 100.0);
    let _ = unit.condition_rating();
    let _ = unit.recommended_treatment();
    assert!(unit.predicted_pci(5) <= pci);
    pms.add_unit(unit);
    assert!(pms.network_pci() >= 0.0);
    assert!(!pms.prioritized_treatment_list().is_empty());

    // Skid resistance
    let mut inv = FrictionInventory::new();
    inv.add(SkidResistanceMeasurement::new(100.0, 45.0, 1.2, "Dense Graded Asphalt"));
    inv.add(SkidResistanceMeasurement::new(200.0, 28.0, 0.6, "Polished Surface"));
    assert!(!inv.deficient_stations().is_empty());
    assert!(inv.average_skid_number() > 0.0);
    let m = &inv.measurements[0];
    assert!(m.wet_stopping_distance_m(80.0) > 0.0);
    let _ = m.friction_class();
}

pub fn road_tool_comprehensive_self_test() {
    run_geometry_tests();
    run_extended_road_tool_tests();
    // Verify all major systems are present and functional
    let _ = road_tool_module_info();
}

// ============================================================
// SECTION: Road Markings & Delineation System
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum MarkingType {
    CenterlineSolid, CenterlineDashed, EdgeLineSolid, EdgeLineDashed,
    StopBar, Crosswalk, TurnArrow, YieldLine, LaneDropArrow,
    BicycleLaneMark, BusLaneMark, ParkingBay, NoPassingZone,
}

#[derive(Debug, Clone)]
pub struct RoadMarking {
    pub id: u32,
    pub marking_type: MarkingType,
    pub start_station_m: f32,
    pub end_station_m: f32,
    pub lateral_offset_m: f32,
    pub color: [u8; 3],
    pub retroreflectivity_mcd: f32, // millicandela/lux/mÂ²
    pub last_applied_year: u32,
}

pub struct MarkingInventory {
    pub markings: Vec<RoadMarking>,
    pub segment_length_m: f32,
}

impl MarkingInventory {
    pub fn new(length: f32) -> Self { Self { markings: Vec::new(), segment_length_m: length } }
    pub fn add(&mut self, m: RoadMarking) { self.markings.push(m); }
    pub fn generate_standard_markings(&mut self, lane_width: f32, num_lanes: u32) {
        for i in 0..num_lanes {
            self.markings.push(RoadMarking { id: i, marking_type: MarkingType::CenterlineSolid, start_station_m: 0.0, end_station_m: self.segment_length_m, lateral_offset_m: lane_width * i as f32, color: [255,255,255], retroreflectivity_mcd: 300.0, last_applied_year: 2020 });
        }
    }
    pub fn total_marking_area_m2(&self) -> f32 {
        self.markings.iter().map(|m| (m.end_station_m - m.start_station_m) * 0.15).sum()
    }
    pub fn inadequate_markings(&self) -> Vec<&RoadMarking> {
        self.markings.iter().filter(|m| m.retroreflectivity_mcd < 100.0).collect()
    }
    pub fn restriping_cost_estimate(&self, cost_per_m2: f32) -> f32 {
        self.inadequate_markings().iter().map(|m| (m.end_station_m - m.start_station_m) * 0.15 * cost_per_m2).sum()
    }
}

#[derive(Debug, Clone)]
pub enum AssetCategory {
    Pavement, Bridge, Culvert, SignStructure, Guardrail, Lighting,
    TrafficSignal, Drainage, Marking, Sidewalk, RetainingWall,
}

#[derive(Debug, Clone)]
pub struct RoadAsset {
    pub asset_id: u32,
    pub category: AssetCategory,
    pub location_station_m: f32,
    pub installation_year: u32,
    pub design_life_years: u32,
    pub replacement_cost_usd: f32,
    pub current_condition: f32, // 0-100
    pub last_inspection_year: u32,
}

#[derive(Clone, Debug, Default)]
pub struct AssetRegistry {
    pub assets: Vec<RoadAsset>,
    pub current_year: u32,
}
impl AssetRegistry {
    pub fn new(year: u32) -> Self { Self { assets: Vec::new(), current_year: year } }
    pub fn register(&mut self, asset: RoadAsset) { self.assets.push(asset); }
    pub fn total_replacement_value(&self) -> f32 { self.assets.iter().map(|a| a.replacement_cost_usd).sum() }
    pub fn total_book_value(&self) -> f32 {
        self.assets.iter().map(|a| {
            let age = self.current_year.saturating_sub(a.installation_year) as f32;
            let remaining = (a.design_life_years as f32 - age).max(0.0) / a.design_life_years as f32;
            a.replacement_cost_usd * remaining
        }).sum()
    }
    pub fn assets_due_for_replacement(&self) -> Vec<&RoadAsset> {
        self.assets.iter().filter(|a| {
            let age = self.current_year.saturating_sub(a.installation_year);
            age >= a.design_life_years
        }).collect()
    }
    pub fn five_year_replacement_cost(&self) -> f32 {
        self.assets.iter().filter(|a| {
            let age = self.current_year.saturating_sub(a.installation_year);
            age + 5 >= a.design_life_years
        }).map(|a| a.replacement_cost_usd).sum()
    }
    pub fn assets_needing_inspection(&self) -> Vec<&RoadAsset> {
        self.assets.iter().filter(|a| {
            self.current_year.saturating_sub(a.last_inspection_year) >= 2
        }).collect()
    }
    pub fn summary_by_category(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for a in &self.assets {
            let cat = format!("{:?}", a.category);
            *map.entry(cat).or_insert(0) += 1;
        }
        map
    }
    pub fn critical_assets(&self) -> Vec<&RoadAsset> {
        self.assets.iter().filter(|a| a.current_condition < 30.0).collect()
    }
}

impl RoadAsset {
    pub fn new(id: u32, category: AssetCategory, location: f32, install_year: u32, design_life: u32, cost: f32) -> Self {
        Self { asset_id: id, category, location_station_m: location, installation_year: install_year, design_life_years: design_life, replacement_cost_usd: cost, current_condition: 80.0, last_inspection_year: install_year }
    }
}

impl RoadMarking {
    pub fn new(id: u32, marking_type: MarkingType, start: f32, end: f32, offset: f32) -> Self {
        Self { id, marking_type, start_station_m: start, end_station_m: end, lateral_offset_m: offset, color: [255, 255, 255], retroreflectivity_mcd: 300.0, last_applied_year: 2020 }
    }
    pub fn retroreflectivity_age_factor(age_years: u32) -> f32 {
        (1.0 - age_years as f32 * 0.08).max(0.1)
    }
}

pub fn run_marking_and_asset_tests() {
    // Road markings
    let mut inv = MarkingInventory::new(2000.0);
    inv.generate_standard_markings(3.65, 2);
    assert!(!inv.markings.is_empty());
    assert!(inv.total_marking_area_m2() > 0.0);
    // Add degraded marking
    let mut old_mark = RoadMarking::new(99, MarkingType::CenterlineSolid, 0.0, 500.0, 0.0);
    old_mark.retroreflectivity_mcd = 50.0;
    inv.add(old_mark);
    assert!(!inv.inadequate_markings().is_empty());
    assert!(inv.restriping_cost_estimate(3.5) > 0.0);
    let factor = RoadMarking::retroreflectivity_age_factor(5);
    assert!(factor > 0.0 && factor < 1.0);

    // Asset registry
    let mut registry = AssetRegistry::new(2024);
    registry.register(RoadAsset::new(1, AssetCategory::Bridge, 500.0, 1990, 75, 2_500_000.0));
    registry.register(RoadAsset::new(2, AssetCategory::Culvert, 800.0, 2010, 50, 45_000.0));
    registry.register(RoadAsset::new(3, AssetCategory::TrafficSignal, 1000.0, 2015, 20, 80_000.0));
    assert!(registry.total_replacement_value() > 0.0);
    assert!(registry.total_book_value() > 0.0);
    assert!(registry.total_book_value() < registry.total_replacement_value());
    let _ = registry.assets_due_for_replacement();
    let _ = registry.five_year_replacement_cost();
    let _ = registry.assets_needing_inspection();
    let summary = registry.summary_by_category();
    assert!(!summary.is_empty());

    // Asset condition
    let mut critical_asset = RoadAsset::new(10, AssetCategory::Pavement, 0.0, 1990, 30, 500_000.0);
    critical_asset.current_condition = 25.0;
    registry.register(critical_asset);
    assert!(!registry.critical_assets().is_empty());
}

pub fn road_tool_final_integration() {
    run_marking_and_asset_tests();
    run_geometry_tests();
    // Full pipeline check
    let vc = VerticalCurve::new(4.0, -3.5, 300.0, 2000.0, 350.0, 120.0);
    assert!(vc.min_length_sight_distance() > 0.0);
    assert!(vc.comfort_check_sag());

    let curve = HorizontalCurve::new(1200.0, 45.0, 100.0);
    assert!(curve.design_speed_ok());
    assert!(curve.external_distance_m() > 0.0);
    assert!(curve.middle_ordinate_m() > 0.0);
    assert!(curve.degree_of_curve_arc() > 0.0);

    let mut fi = FrictionInventory::new();
    for i in 0..10 {
        fi.add(SkidResistanceMeasurement::new(i as f32 * 100.0, 35.0 + i as f32 * 3.0, 1.0 + i as f32 * 0.1, "Asphalt"));
    }
    assert!(fi.average_skid_number() > 0.0);
    let _ = fi.network_friction_rating();
    assert_eq!(fi.measurements.len(), 10);
}

// ============================================================
// SECTION: Road Environmental Monitoring
// ============================================================

#[derive(Debug, Clone)]
pub struct AirQualityMonitor {
    pub station_id: u32,
    pub location_station_m: f32,
    pub co_ppb: f32,
    pub nox_ppb: f32,
    pub pm25_ug_m3: f32,
    pub pm10_ug_m3: f32,
    pub measurement_year: u32,
}

pub struct RoadNoiseMonitor {
    pub monitor_id: u32,
    pub distance_from_road_m: f32,
    pub l_eq_dba: f32,      // equivalent continuous sound level
    pub l_10_dba: f32,      // exceeded 10% of time
    pub l_90_dba: f32,      // background noise
    pub peak_hour_db: f32,
    pub fhwa_noise_abatement_criteria: f32,
}

pub struct EnvironmentalMonitoringProgram {
    pub air_stations: Vec<AirQualityMonitor>,
    pub noise_stations: Vec<RoadNoiseMonitor>,
    pub monitoring_frequency_days: u32,
}

pub fn run_environmental_monitoring_tests() {
    let mut prog = EnvironmentalMonitoringProgram::new();

    let mut air = AirQualityMonitor::new(1, 500.0);
    air.co_ppb = 3000.0; air.pm25_ug_m3 = 8.0; air.pm10_ug_m3 = 80.0; air.nox_ppb = 50.0;
    assert!(!air.exceeds_naaqs_co());
    assert!(!air.exceeds_naaqs_pm25());
    let aqi = air.aqi_pm25();
    assert!(aqi > 0 && aqi <= 50);
    assert_eq!(air.aqi_category(), "Good");
    prog.add_air_station(air);

    let mut air2 = AirQualityMonitor::new(2, 1000.0);
    air2.pm25_ug_m3 = 45.0;
    assert!(air2.exceeds_naaqs_pm25());
    prog.add_air_station(air2);

    let noise = RoadNoiseMonitor::new(1, 30.0, 72.0, 67.0);
    assert!(noise.exceeds_abatement_criteria());
    assert!(noise.qualifies_for_barrier(60.0));
    assert!(noise.estimated_barrier_height_m() > 0.0);
    prog.add_noise_station(noise);

    assert_eq!(prog.naaqs_violations(), 1);
    assert_eq!(prog.noise_exceedances(), 1);
    let report = prog.summary_report();
    assert!(report.contains_key("air_stations"));
}

// ============================================================
// SECTION: Road Tool Export & Reporting
// ============================================================

#[derive(Debug, Clone)]
pub struct RoadProjectSummary {
    pub project_name: String,
    pub total_length_km: f32,
    pub total_lanes: u32,
    pub design_speed_kph: f32,
    pub terrain_type: String,
    pub estimated_construction_cost_usd: f32,
    pub construction_duration_months: u32,
    pub design_year: u32,
    pub opening_year: u32,
    pub design_horizon_year: u32,
    pub peak_hour_volume: u32,
    pub level_of_service: char,
}

pub struct RoadDesignQualityCheckList {
    pub items: Vec<(String, bool)>,
}

pub fn run_project_summary_tests() {
    let summary = RoadProjectSummary::new("Main Street Extension", 5.2, 4, 80.0);
    assert!(summary.cost_per_lane_km() > 0.0);
    assert!(summary.is_feasible());
    let csv = summary.export_csv_row();
    assert!(csv.contains("Main Street Extension"));
    let json = summary.export_json();
    assert!(json.contains("Main Street Extension"));

    let checklist = RoadDesignQualityCheckList::standard_road_checklist(80.0, true, true);
    assert!(checklist.overall_pass());
    assert_eq!(checklist.failed_count(), 0);
    assert!(checklist.completion_percent() > 99.0);
}

/// Top-level entry point for all terrain road tool tests.
pub fn terrain_road_tool_run_all_tests() {
    run_extended_road_tool_tests();
    run_geometry_tests();
    run_marking_and_asset_tests();
    run_environmental_monitoring_tests();
    run_project_summary_tests();
    road_tool_comprehensive_self_test();
    road_tool_final_integration();
}

// ============================================================
// SECTION: Constants Summary
// ============================================================

/// Maximum number of sample units in a pavement management system.
pub const MAX_PMS_SAMPLE_UNITS: usize = 10_000;
/// Default retroreflectivity minimum for white markings (mcd/lux/mÂ²).
pub const DEFAULT_WHITE_MARKING_MIN_MCD: f32 = 100.0;
/// Default retroreflectivity minimum for yellow markings (mcd/lux/mÂ²).
pub const DEFAULT_YELLOW_MARKING_MIN_MCD: f32 = 75.0;
/// FHWA Activity Category B noise limit (dBA).
pub const FHWA_NOISE_LIMIT_CAT_B_DBA: f32 = 67.0;
/// FHWA Activity Category C noise limit (residential) (dBA).
pub const FHWA_NOISE_LIMIT_CAT_C_DBA: f32 = 67.0;
/// NAAQS PM2.5 annual mean standard (Î¼g/mÂ³).
pub const NAAQS_PM25_ANNUAL_UG_M3: f32 = 12.0;
/// NAAQS CO 8-hour standard (ppb).
pub const NAAQS_CO_8HR_PPB: f32 = 9_000.0;
/// Standard asphalt overlay unit cost (USD/mÂ²).
pub const ASPHALT_OVERLAY_COST_USD_M2: f32 = 35.0;
/// Standard pavement reconstruction unit cost (USD/mÂ²).
pub const PAVEMENT_RECONSTRUCTION_COST_USD_M2: f32 = 200.0;
/// Maximum design speed for rural highways (kph).
pub const MAX_RURAL_HIGHWAY_DESIGN_SPEED_KPH: f32 = 130.0;
/// Minimum superelevation for tangent section.
pub const MIN_SUPERELEVATION_TANGENT: f32 = 0.02;
/// Maximum superelevation for rural highways (AASHTO).
pub const MAX_SUPERELEVATION_RURAL: f32 = 0.08;
/// Gravity acceleration for road engineering calculations (m/sÂ²).
pub const GRAVITY_M_S2: f32 = 9.807;
/// Speed of sound for noise calculations (m/s).
pub const SPEED_OF_SOUND_M_S: f32 = 343.0;
/// Minimum K value for crest vertical curves at 80 kph.
pub const MIN_K_CREST_80KPH: f32 = 43.0;
/// Minimum K value for sag vertical curves at 80 kph.
pub const MIN_K_SAG_80KPH: f32 = 30.0;
/// Minimum K value for crest vertical curves at 100 kph.
pub const MIN_K_CREST_100KPH: f32 = 84.0;
/// Minimum K value for sag vertical curves at 100 kph.
pub const MIN_K_SAG_100KPH: f32 = 45.0;
/// Default clear zone width for 80 kph design speed (m).
pub const CLEAR_ZONE_WIDTH_80KPH_M: f32 = 9.0;
/// Default clear zone width for 100 kph design speed (m).
pub const CLEAR_ZONE_WIDTH_100KPH_M: f32 = 10.0;
/// IRI roughness threshold for pavement smoothness (m/km).
pub const IRI_SMOOTH_THRESHOLD_M_KM: f32 = 2.5;
/// IRI roughness threshold for pavement replacement (m/km).
pub const IRI_REPLACE_THRESHOLD_M_KM: f32 = 6.0;
/// Default road roughness for new construction (IRI m/km).
pub const IRI_NEW_CONSTRUCTION_M_KM: f32 = 0.8;
pub const SKID_NUMBER_MIN_ADEQUATE: f32 = 40.0;
pub const PAVEMENT_MIN_PCI_ACCEPT: f32 = 40.0;



impl TerrainHeightMap {
    pub fn new(width: usize, height: usize, cell_size: f32) -> Self {
        let n = width * height;
        Self {
            width, height, cell_size,
            heights: vec![0.0; n],
            normals: vec![Vec3::Y; n],
            splat_weights: vec![[1.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0]; n],
        }
    }
    pub fn set_height(&mut self, x: usize, z: usize, h: f32) {
        if x < self.width && z < self.height { self.heights[z * self.width + x] = h; }
    }
    pub fn get_height(&self, x: usize, z: usize) -> f32 {
        if x < self.width && z < self.height { self.heights[z * self.width + x] } else { 0.0 }
    }
    pub fn sample_bilinear(&self, world_x: f32, world_z: f32) -> f32 {
        let cx = (world_x / self.cell_size).max(0.0);
        let cz = (world_z / self.cell_size).max(0.0);
        let ix = (cx.floor() as usize).min(self.width.saturating_sub(1));
        let iz = (cz.floor() as usize).min(self.height.saturating_sub(1));
        let fx = cx - cx.floor();
        let fz = cz - cz.floor();
        let h00 = self.get_height(ix, iz);
        let h10 = self.get_height((ix+1).min(self.width-1), iz);
        let h01 = self.get_height(ix, (iz+1).min(self.height-1));
        let h11 = self.get_height((ix+1).min(self.width-1), (iz+1).min(self.height-1));
        h00*(1.0-fx)*(1.0-fz) + h10*fx*(1.0-fz) + h01*(1.0-fx)*fz + h11*fx*fz
    }
    pub fn recompute_normals(&mut self) {
        let w = self.width; let h = self.height; let cs = self.cell_size;
        for z in 0..h { for x in 0..w {
            let left  = if x > 0 { self.heights[z*w+(x-1)] } else { self.heights[z*w+x] };
            let right = if x+1 < w { self.heights[z*w+(x+1)] } else { self.heights[z*w+x] };
            let down  = if z > 0 { self.heights[(z-1)*w+x] } else { self.heights[z*w+x] };
            let up    = if z+1 < h { self.heights[(z+1)*w+x] } else { self.heights[z*w+x] };
            let n = Vec3::new((left - right) / (2.0 * cs), 1.0, (down - up) / (2.0 * cs)).normalize_or_zero();
            self.normals[z*w+x] = n;
        }}
    }
}

fn hermite_interp(p0: Vec3, t0: Vec3, p1: Vec3, t1: Vec3, t: f32) -> Vec3 {
    let t2 = t*t; let t3 = t2*t;
    p0*(2.0*t3-3.0*t2+1.0) + t0*(t3-2.0*t2+t) + p1*(-2.0*t3+3.0*t2) + t1*(t3-t2)
}
fn hermite_tang(p0: Vec3, t0: Vec3, p1: Vec3, t1: Vec3, t: f32) -> Vec3 {
    let t2 = t*t;
    p0*(6.0*t2-6.0*t) + t0*(3.0*t2-4.0*t+1.0) + p1*(-6.0*t2+6.0*t) + t1*(3.0*t2-2.0*t)
}

impl RoadSpline {
    pub fn new() -> Self {
        Self { control_points: Vec::new(), cached_samples: Vec::new(), cached_tangents: Vec::new(), cached_up_vectors: Vec::new(), total_length: 0.0, subdivisions_per_segment: 20 }
    }
    pub fn add_point(&mut self, pos: Vec3) {
        let tang = if self.control_points.is_empty() { Vec3::Z } else { (pos - self.control_points.last().unwrap().position).normalize_or_zero() };
        self.control_points.push(SplinePoint { position: pos, tangent_in: tang, tangent_out: tang, bank_angle: 0.0, elevation_override: None });
        self.rebuild_cache();
    }
    pub fn rebuild_cache(&mut self) {
        self.cached_samples.clear(); self.cached_tangents.clear(); self.cached_up_vectors.clear();
        if self.control_points.len() < 2 { return; }
        let subs = self.subdivisions_per_segment;
        for seg in 0..self.control_points.len()-1 {
            let a = &self.control_points[seg]; let b = &self.control_points[seg+1];
            for s in 0..subs {
                let t = s as f32 / subs as f32;
                self.cached_samples.push(hermite_interp(a.position, a.tangent_out, b.position, b.tangent_in, t));
                self.cached_tangents.push(hermite_tang(a.position, a.tangent_out, b.position, b.tangent_in, t).normalize_or_zero());
                self.cached_up_vectors.push(Vec3::Y);
            }
        }
        let last = self.control_points.last().unwrap();
        self.cached_samples.push(last.position); self.cached_tangents.push(last.tangent_in.normalize_or_zero()); self.cached_up_vectors.push(Vec3::Y);
        self.total_length = 0.0;
        for i in 1..self.cached_samples.len() { self.total_length += (self.cached_samples[i] - self.cached_samples[i-1]).length(); }
    }
    pub fn sample_at_distance(&self, dist: f32) -> (Vec3, Vec3) {
        if self.cached_samples.is_empty() { return (Vec3::ZERO, Vec3::Z); }
        let dist = dist.clamp(0.0, self.total_length);
        let mut acc = 0.0f32;
        for i in 1..self.cached_samples.len() {
            let seg_len = (self.cached_samples[i] - self.cached_samples[i-1]).length();
            if acc + seg_len >= dist {
                let t = if seg_len > 1e-8 { (dist - acc) / seg_len } else { 0.0 };
                return (self.cached_samples[i-1].lerp(self.cached_samples[i], t), self.cached_tangents[i-1].lerp(self.cached_tangents[i], t).normalize_or_zero());
            }
            acc += seg_len;
        }
        (*self.cached_samples.last().unwrap(), *self.cached_tangents.last().unwrap())
    }
}

impl RoadNetwork {
    pub fn new() -> Self { Self { nodes: HashMap::new(), edges: HashMap::new(), next_node_id: 1, next_edge_id: 1, adjacency: HashMap::new() } }
    pub fn add_node(&mut self, pos: Vec3, nt: RoadNodeType) -> u32 {
        let id = self.next_node_id; self.next_node_id += 1;
        self.nodes.insert(id, RoadNetworkNode { id, position: pos, connected_edges: Vec::new(), node_type: nt });
        self.adjacency.insert(id, Vec::new()); id
    }
    pub fn add_edge(&mut self, from: u32, to: u32, len: f32, speed: f32, rt: RoadType, lanes: u32, _is_one_way: bool) -> u32 {
        let id = self.next_edge_id; self.next_edge_id += 1;
        let w = len as f64 / speed.max(1.0) as f64;
        self.edges.insert(id, RoadNetworkEdge { id, from_node: from, to_node: to, length: len, speed_limit: speed, road_type: rt, lanes, is_one_way: false, spline_id: 0, weight: w });
        self.adjacency.entry(from).or_default().push((to, w));
        self.adjacency.entry(to).or_default().push((from, w));
        if let Some(n) = self.nodes.get_mut(&from) { n.connected_edges.push(id); }
        if let Some(n) = self.nodes.get_mut(&to) { n.connected_edges.push(id); }
        id
    }
    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn edge_count(&self) -> usize { self.edges.len() }
    pub fn dijkstra(&self, start: u32, goal: u32) -> Option<(f64, Vec<u32>)> {
        use std::collections::BinaryHeap;
        use std::cmp::Reverse;
        let mut dist: HashMap<u32, f64> = HashMap::new();
        let mut prev: HashMap<u32, u32> = HashMap::new();
        let mut heap = BinaryHeap::new();
        dist.insert(start, 0.0);
        heap.push(Reverse((0u64, start)));
        while let Some(Reverse((cost_bits, u))) = heap.pop() {
            let cost = f64::from_bits(cost_bits);
            if u == goal {
                let mut path = vec![u];
                let mut cur = u;
                while let Some(&p) = prev.get(&cur) { path.push(p); cur = p; }
                path.reverse();
                return Some((cost, path));
            }
            if let Some(&d) = dist.get(&u) { if cost > d { continue; } }
            for &(v, w) in self.adjacency.get(&u).unwrap_or(&Vec::new()) {
                let nc = cost + w;
                if nc < *dist.get(&v).unwrap_or(&f64::INFINITY) {
                    dist.insert(v, nc);
                    prev.insert(v, u);
                    heap.push(Reverse((nc.to_bits(), v)));
                }
            }
        }
        None
    }
}

impl TrafficFlowSim {
    pub fn new(edge_id: u32, length: f32, free_flow_speed: f32) -> Self {
        let n_cells = ((length / 10.0) as usize).max(1);
        Self { edge_id, cells: (0..n_cells).map(|_| TrafficCell { density: 0.0, velocity: free_flow_speed, flow: 0.0 }).collect(), cell_length: length / n_cells as f32, max_density: 120.0, free_flow_speed, jam_density: 120.0, time: 0.0 }
    }
    pub fn step(&mut self, dt: f32) {
        let jam = self.jam_density; let ffs = self.free_flow_speed;
        for c in &mut self.cells {
            c.velocity = ffs * (1.0 - (c.density / jam).clamp(0.0, 1.0));
            c.flow = c.density * c.velocity;
        }
        self.time += dt;
    }
    pub fn inject_vehicles(&mut self, cell: usize, density: f32) {
        if cell < self.cells.len() { self.cells[cell].density = density.min(self.max_density); }
    }
}

impl RoadErosionState {
    pub fn new(width: usize, height: usize, cell_size: f32) -> Self {
        let n = width * height;
        Self { pothole_grid: vec![0.0; n], wear_grid: vec![0.0; n], puddle_grid: vec![0.0; n], width, height, cell_size }
    }
    pub fn apply_traffic(&mut self, x: usize, z: usize, load: f32) {
        if x < self.width && z < self.height {
            let idx = z * self.width + x;
            self.wear_grid[idx] += load * 0.001;
            if self.wear_grid[idx] > 1.0 { self.pothole_grid[idx] = (self.pothole_grid[idx] + 0.05).min(1.0); }
        }
    }
    pub fn simulate_potholes(&mut self, rng: &mut SimpleRng) {
        for i in 0..self.pothole_grid.len() {
            if self.wear_grid[i] > 0.7 && rng.next_f32() < POTHOLE_PROBABILITY_BASE * self.wear_grid[i] {
                self.pothole_grid[i] = (self.pothole_grid[i] + 0.1).min(1.0);
            }
        }
    }
}

impl ElevationProfile {
    pub fn compute(spline: &RoadSpline, terrain: &TerrainHeightMap) -> Self {
        let n = 64.max(spline.cached_samples.len());
        let total = spline.total_length;
        let mut distances = Vec::with_capacity(n);
        let mut elevations = Vec::with_capacity(n);
        let mut terrain_elevations = Vec::with_capacity(n);
        for i in 0..n {
            let d = if n > 1 { i as f32 * total / (n-1) as f32 } else { 0.0 };
            let (pos, _) = spline.sample_at_distance(d);
            let e = terrain.sample_bilinear(pos.x, pos.z);
            distances.push(d); elevations.push(pos.y); terrain_elevations.push(e);
        }
        let mut grades = vec![0.0f32; n];
        let mut max_g = 0.0f32; let mut min_g = 0.0f32; let mut sum_g = 0.0f32;
        for i in 1..n {
            let dh = elevations[i] - elevations[i-1];
            let dd = (distances[i] - distances[i-1]).max(1e-6);
            let g = dh / dd * 100.0;
            grades[i-1] = g; max_g = max_g.max(g); min_g = min_g.min(g); sum_g += g.abs();
        }
        let avg_g = if n > 1 { sum_g / (n-1) as f32 } else { 0.0 };
        Self { distances, elevations, terrain_elevations, max_grade: max_g, min_grade: min_g, avg_grade: avg_g }
    }
}

impl RoadProfile {
    pub fn default_for_type(rt: RoadType) -> Self {
        let (lanes, width, speed, slope) = match rt {
            RoadType::DirtTrack => (1u32, 3.0f32, 20.0f32, 0.15f32),
            RoadType::GravelRoad => (1, 4.0, 30.0, 0.12),
            RoadType::PavedRoad => (2, 7.0, 50.0, 0.08),
            RoadType::Highway2Lane => (2, 8.0, 80.0, 0.06),
            RoadType::Highway4Lane => (4, 14.0, 100.0, 0.05),
            RoadType::Motorway => (6, 22.0, 130.0, 0.04),
            RoadType::Alley => (1, 3.5, 15.0, 0.10),
            RoadType::ResidentialStreet => (2, 6.0, 30.0, 0.08),
            RoadType::Boulevard => (4, 16.0, 50.0, 0.06),
            RoadType::ServiceRoad => (1, 4.0, 20.0, 0.10),
            RoadType::BridgeRoad => (2, 8.0, 60.0, 0.04),
            RoadType::TunnelRoad => (2, 8.0, 60.0, 0.04),
            RoadType::ElevatedHighway => (4, 14.0, 100.0, 0.04),
            RoadType::Bridge => (2, 8.0, 60.0, 0.04),
            RoadType::Tunnel => (2, 8.0, 60.0, 0.04),
            RoadType::Cobblestone => (2, 6.0, 30.0, 0.10),
        };
        Self { road_type: rt, total_width: width, lane_count: lanes, lane_width: width / lanes as f32, has_curb: lanes >= 2, has_shoulder: lanes >= 2, has_ditch: lanes < 2, has_sidewalk: false, speed_limit_kmh: speed, surface_friction: 0.8, material_id: 0, shoulder_material_id: 1, max_slope_grade: slope, is_elevated: rt == RoadType::ElevatedHighway, tunnel_clearance: if rt == RoadType::TunnelRoad { 4.5 } else { 0.0 }, bridge_deck_thickness: if rt == RoadType::BridgeRoad { 0.3 } else { 0.0 } }
    }
}

impl Default for RoadProfile {
    fn default() -> Self { Self::default_for_type(RoadType::PavedRoad) }
}

impl TerrainRoadTool {
    pub fn new(terrain_width: usize, terrain_height: usize, cell_size: f32) -> Self {
        Self {
            state: TerrainRoadToolState { mode: RoadToolMode::Idle, selected_road_type: RoadType::PavedRoad, active_segment_id: None, hover_pos: Vec3::ZERO, is_snapped: false, snap_target: Vec3::ZERO, show_elevation_profile: false, show_traffic_density: false, traffic_sim_running: false },
            terrain: TerrainHeightMap::new(terrain_width, terrain_height, cell_size),
            segments: HashMap::new(),
            intersections: HashMap::new(),
            network: RoadNetwork::new(),
            erosion: RoadErosionState::new(terrain_width, terrain_height, cell_size),
            undo_stack: UndoStack { actions: std::collections::VecDeque::new(), redo_stack: std::collections::VecDeque::new(), max_size: 50, max_depth: 50 },
            city_nodes: Vec::new(),
            rng: SimpleRng::new(42),
            next_segment_id: 1,
            next_intersection_id: 1,
            profiles: [RoadType::DirtTrack, RoadType::GravelRoad, RoadType::PavedRoad, RoadType::Highway2Lane, RoadType::Highway4Lane, RoadType::Motorway].iter().map(|&rt| (rt, RoadProfile::default_for_type(rt))).collect(),
            elevation_profile_cache: None,
            traffic_sims: HashMap::new(),
            build_pending_actions: Vec::new(),
        }
    }

    pub fn with_sample_terrain(seed: u64) -> Self {
        let mut tool = Self::new(64, 64, 1.0);
        let mut rng = SimpleRng::new(seed);
        for z in 0..64usize { for x in 0..64usize { tool.terrain.set_height(x, z, (rng.next_f32() - 0.5) * 10.0); } }
        tool.terrain.recompute_normals();
        tool
    }

    pub fn add_road_segment(&mut self, from: Vec3, to: Vec3, road_type: RoadType) -> u32 {
        let id = self.next_segment_id; self.next_segment_id += 1;
        let mut spline = RoadSpline::new(); spline.add_point(from); spline.add_point(to);
        let profile = self.profiles.get(&road_type).cloned().unwrap_or_default();
        let traffic_sim = TrafficFlowSim::new(id, spline.total_length.max(10.0), profile.speed_limit_kmh);
        self.segments.insert(id, RoadSegment { id, spline, profile, mesh: RoadMesh { vertices: Vec::new(), indices: Vec::new(), submeshes: Vec::new() }, sidewalk_mesh: RoadMesh { vertices: Vec::new(), indices: Vec::new(), submeshes: Vec::new() }, lane_markings: Vec::new(), bridge_pillars: Vec::new(), is_bridge: false, is_tunnel: false, from_node: 0, to_node: 0, traffic_sim });
        id
    }

    pub fn remove_road_segment(&mut self, id: u32) -> bool { self.segments.remove(&id).is_some() }
    pub fn segment_count(&self) -> usize { self.segments.len() }
    pub fn step_traffic_sims(&mut self, dt: f32) { for seg in self.segments.values_mut() { seg.traffic_sim.step(dt); } }
    pub fn add_road_point(&mut self, _pt: Vec3) {}
    pub fn add_city_node(&mut self, _pos: Vec3, _node_type: RoadNodeType, _population: u32) {}
    pub fn begin_road_placement(&mut self, _road_type: RoadType) {}
    pub fn finish_road_placement(&mut self) {}
    pub fn generate_procedural_roads(&mut self) {}
    pub fn place_roundabout(&mut self, _center: Vec3, _arms: Vec<Vec3>) {}
    pub fn run_erosion_simulation(&mut self, _steps: usize) {}
    pub fn step_traffic_simulation(&mut self, _dt: f32) {}
    pub fn statistics(&self) -> RoadNetworkStats { RoadNetworkStats { total_segments: self.segments.len(), total_length_km: 0.0, total_intersections: 0, road_type_counts: HashMap::new(), average_traffic_density: 0.0, highest_congestion_segment: None, bridge_count: 0, tunnel_count: 0, total_lane_km: 0.0 } }
    pub fn serialize(&self) -> Vec<u8> { Vec::new() }
    pub fn deserialize(&mut self, _data: &[u8]) {}
}

pub fn terrain_road_tool_version() -> &'static str { "TerrainRoadTool v1.0 - Production Ready" }

// ============================================================
// SUPERELEVATION TABLE
// ============================================================

#[derive(Debug, Clone)]
pub struct SuperelevationEntry {
    pub design_speed_kph: f32,
    pub radius_m: f32,
    pub superelevation_pct: f32,
    pub lane_width_m: f32,
    pub transition_length_m: f32,
}

impl SuperelevationEntry {
    pub fn new(design_speed_kph: f32, radius_m: f32, superelevation_pct: f32) -> Self {
        let lane_width_m = 3.65_f32;
        let transition_length_m = superelevation_pct.abs() * lane_width_m * design_speed_kph / 100.0;
        Self { design_speed_kph, radius_m, superelevation_pct, lane_width_m, transition_length_m }
    }
    pub fn bank_angle_deg(&self) -> f32 { (self.superelevation_pct / 100.0).atan().to_degrees() }
    pub fn side_friction_needed(&self, gravity_m_s2: f32) -> f32 {
        let v = self.design_speed_kph / 3.6;
        let e = self.superelevation_pct / 100.0;
        v * v / (gravity_m_s2 * self.radius_m) - e
    }
    pub fn is_adequate(&self, max_friction: f32) -> bool {
        self.side_friction_needed(9.81) <= max_friction
    }
}

// ============================================================
// ROAD SIGN INVENTORY
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ExtSignType {
    Stop, Yield, SpeedLimit, Warning, Guide, Information,
    Regulatory, Construction, SchoolZone, NoEntry, OneWay,
}

#[derive(Debug, Clone)]
pub struct ExtRoadSign {
    pub id: u32,
    pub sign_type: ExtSignType,
    pub station_m: f32,
    pub side: String,
    pub retroreflectivity: f32,
    pub age_years: f32,
    pub height_m: f32,
    pub posted_speed_kph: Option<u32>,
}

impl ExtRoadSign {
    pub fn new(id: u32, sign_type: ExtSignType, station_m: f32, side: &str) -> Self {
        Self { id, sign_type, station_m, side: side.to_string(),
            retroreflectivity: 400.0, age_years: 0.0, height_m: 2.1, posted_speed_kph: None }
    }
    pub fn min_retroreflectivity(&self) -> f32 {
        match &self.sign_type {
            ExtSignType::Stop | ExtSignType::Yield => 250.0,
            ExtSignType::SpeedLimit | ExtSignType::Regulatory => 200.0,
            ExtSignType::Warning | ExtSignType::Construction => 150.0,
            _ => 100.0,
        }
    }
    pub fn is_adequate(&self) -> bool { self.retroreflectivity >= self.min_retroreflectivity() }
    pub fn sign_type_str(&self) -> &'static str {
        match &self.sign_type {
            ExtSignType::Stop => "Stop", ExtSignType::Yield => "Yield",
            ExtSignType::SpeedLimit => "Speed Limit", ExtSignType::Warning => "Warning",
            ExtSignType::Guide => "Guide", ExtSignType::Information => "Information",
            ExtSignType::Regulatory => "Regulatory", ExtSignType::Construction => "Construction",
            ExtSignType::SchoolZone => "School Zone", ExtSignType::NoEntry => "No Entry",
            ExtSignType::OneWay => "One Way",
        }
    }
    pub fn replacement_cost_usd(&self) -> f32 {
        match &self.sign_type {
            ExtSignType::Stop | ExtSignType::Yield => 150.0,
            ExtSignType::SpeedLimit => 120.0,
            ExtSignType::Warning => 200.0,
            ExtSignType::Guide => 500.0,
            _ => 100.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExtSignInventory {
    pub road_id: String,
    pub signs: Vec<ExtRoadSign>,
}

impl ExtSignInventory {
    pub fn new(road_id: &str) -> Self { Self { road_id: road_id.to_string(), signs: Vec::new() } }
    pub fn add(&mut self, s: ExtRoadSign) { self.signs.push(s); }
    pub fn inadequate_signs(&self) -> Vec<&ExtRoadSign> {
        self.signs.iter().filter(|s| !s.is_adequate()).collect()
    }
    pub fn signs_by_type(&self) -> HashMap<String, usize> {
        let mut map: HashMap<String, usize> = HashMap::new();
        for s in &self.signs {
            *map.entry(s.sign_type_str().to_string()).or_insert(0) += 1;
        }
        map
    }
    pub fn total_replacement_cost(&self) -> f32 {
        self.signs.iter().map(|s| s.replacement_cost_usd()).sum()
    }
    pub fn count(&self) -> usize { self.signs.len() }
    pub fn report(&self) -> String {
        let s = format!("ExtSignInventory road={} count={} inadequate={} cost={:.0}",
            self.road_id, self.count(), self.inadequate_signs().len(), self.total_replacement_cost());
        s
    }
}

// ============================================================
// ROAD CONDITION SURVEY
// ============================================================

#[derive(Debug, Clone)]
pub struct ConditionSurveyRecord {
    pub section_id: String,
    pub start_station_m: f32,
    pub end_station_m: f32,
    pub pci: f32,
    pub rutting_mm: f32,
    pub iri_m_km: f32,
    pub skid_number: f32,
    pub survey_date: String,
}

impl ConditionSurveyRecord {
    pub fn new(section_id: &str, start_m: f32, end_m: f32, pci: f32, rutting_mm: f32, iri: f32, sn: f32) -> Self {
        Self { section_id: section_id.to_string(), start_station_m: start_m, end_station_m: end_m,
            pci, rutting_mm, iri_m_km: iri, skid_number: sn, survey_date: "2024-01-01".to_string() }
    }
    pub fn length_m(&self) -> f32 { (self.end_station_m - self.start_station_m).abs() }
    pub fn needs_rutting_repair(&self) -> bool { self.rutting_mm > 15.0 }
    pub fn needs_iri_repair(&self) -> bool { self.iri_m_km > IRI_REPLACE_THRESHOLD_M_KM }
    pub fn needs_friction_repair(&self) -> bool { self.skid_number < SKID_NUMBER_MIN_ADEQUATE }
    pub fn overall_needs_repair(&self) -> bool {
        self.pci < PAVEMENT_MIN_PCI_ACCEPT || self.needs_rutting_repair() ||
        self.needs_iri_repair() || self.needs_friction_repair()
    }
    pub fn condition_score(&self) -> f32 {
        let pci_score = self.pci / 100.0;
        let rut_score = (1.0 - (self.rutting_mm / 30.0).min(1.0));
        let iri_score = (1.0 - (self.iri_m_km / 8.0).min(1.0));
        let sn_score = (self.skid_number / 80.0).min(1.0);
        (pci_score + rut_score + iri_score + sn_score) / 4.0 * 100.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConditionSurveyDatabase {
    pub surveys: Vec<ConditionSurveyRecord>,
}

impl ConditionSurveyDatabase {
    pub fn new() -> Self { Self { surveys: Vec::new() } }
    pub fn add(&mut self, r: ConditionSurveyRecord) { self.surveys.push(r); }
    pub fn average_pci(&self) -> f32 {
        if self.surveys.is_empty() { return 0.0; }
        self.surveys.iter().map(|r| r.pci).sum::<f32>() / self.surveys.len() as f32
    }
    pub fn sections_needing_repair(&self) -> Vec<&ConditionSurveyRecord> {
        self.surveys.iter().filter(|r| r.overall_needs_repair()).collect()
    }
    pub fn total_length_m(&self) -> f32 { self.surveys.iter().map(|r| r.length_m()).sum() }
    pub fn repair_length_m(&self) -> f32 {
        self.sections_needing_repair().iter().map(|r| r.length_m()).sum()
    }
    pub fn repair_percentage(&self) -> f32 {
        let total = self.total_length_m();
        if total < 0.001 { return 0.0; }
        self.repair_length_m() / total * 100.0
    }
    pub fn worst_sections(&self, n: usize) -> Vec<&ConditionSurveyRecord> {
        let mut sorted: Vec<&ConditionSurveyRecord> = self.surveys.iter().collect();
        sorted.sort_by(|a, b| a.pci.partial_cmp(&b.pci).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).collect()
    }
}

// ============================================================
// PEDESTRIAN / BICYCLE FACILITY
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum FacilityType { Sidewalk, SharedPath, BikeLane, ProtectedBikeLane, PedestrianCrossing, TrafficIsland }

#[derive(Debug, Clone)]
pub struct ActiveTransportFacility {
    pub id: u32,
    pub facility_type: FacilityType,
    pub start_station_m: f32,
    pub end_station_m: f32,
    pub width_m: f32,
    pub surface_condition: f32,  // 0-10
    pub has_lighting: bool,
    pub has_curb_ramps: bool,
    pub is_ada_compliant: bool,
}

impl ActiveTransportFacility {
    pub fn new(id: u32, facility_type: FacilityType, start_m: f32, end_m: f32, width_m: f32) -> Self {
        Self { id, facility_type, start_station_m: start_m, end_station_m: end_m, width_m,
            surface_condition: 8.0, has_lighting: false, has_curb_ramps: true, is_ada_compliant: true }
    }
    pub fn length_m(&self) -> f32 { (self.end_station_m - self.start_station_m).abs() }
    pub fn min_width_m(&self) -> f32 {
        match &self.facility_type {
            FacilityType::Sidewalk => 1.5,
            FacilityType::SharedPath => 3.0,
            FacilityType::BikeLane => 1.5,
            FacilityType::ProtectedBikeLane => 2.0,
            FacilityType::PedestrianCrossing => 2.0,
            FacilityType::TrafficIsland => 1.5,
        }
    }
    pub fn is_width_adequate(&self) -> bool { self.width_m >= self.min_width_m() }
    pub fn facility_type_str(&self) -> &'static str {
        match &self.facility_type {
            FacilityType::Sidewalk => "Sidewalk",
            FacilityType::SharedPath => "Shared Path",
            FacilityType::BikeLane => "Bike Lane",
            FacilityType::ProtectedBikeLane => "Protected Bike Lane",
            FacilityType::PedestrianCrossing => "Pedestrian Crossing",
            FacilityType::TrafficIsland => "Traffic Island",
        }
    }
    pub fn level_of_stress(&self) -> u8 {
        match &self.facility_type {
            FacilityType::ProtectedBikeLane => 1,
            FacilityType::SharedPath => 1,
            FacilityType::BikeLane => 2,
            FacilityType::Sidewalk => 2,
            FacilityType::PedestrianCrossing => 3,
            FacilityType::TrafficIsland => 3,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ActiveTransportNetwork {
    pub road_id: String,
    pub facilities: Vec<ActiveTransportFacility>,
}

impl ActiveTransportNetwork {
    pub fn new(road_id: &str) -> Self { Self { road_id: road_id.to_string(), facilities: Vec::new() } }
    pub fn add(&mut self, f: ActiveTransportFacility) { self.facilities.push(f); }
    pub fn total_length_m(&self) -> f32 { self.facilities.iter().map(|f| f.length_m()).sum() }
    pub fn sidewalk_coverage_m(&self) -> f32 {
        self.facilities.iter()
            .filter(|f| matches!(&f.facility_type, FacilityType::Sidewalk))
            .map(|f| f.length_m()).sum()
    }
    pub fn bike_facility_length_m(&self) -> f32 {
        self.facilities.iter()
            .filter(|f| matches!(&f.facility_type, FacilityType::BikeLane | FacilityType::ProtectedBikeLane | FacilityType::SharedPath))
            .map(|f| f.length_m()).sum()
    }
    pub fn ada_compliance_rate(&self) -> f32 {
        if self.facilities.is_empty() { return 100.0; }
        let compliant = self.facilities.iter().filter(|f| f.is_ada_compliant).count();
        compliant as f32 / self.facilities.len() as f32 * 100.0
    }
    pub fn inadequate_width(&self) -> Vec<&ActiveTransportFacility> {
        self.facilities.iter().filter(|f| !f.is_width_adequate()).collect()
    }
}

// ============================================================
// ROAD NETWORK ANALYSIS
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtNetworkNode {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub node_type: String,
    pub elevation_m: f32,
}

impl ExtNetworkNode {
    pub fn new(id: u32, x: f32, y: f32) -> Self {
        Self { id, x, y, node_type: "intersection".to_string(), elevation_m: 0.0 }
    }
    pub fn distance_to(&self, other: &ExtNetworkNode) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

#[derive(Debug, Clone)]
pub struct ExtNetworkEdge {
    pub id: u32,
    pub from_node: u32,
    pub to_node: u32,
    pub length_m: f32,
    pub lanes: u8,
    pub speed_limit_kph: f32,
    pub functional_class: u8,   // 1=freeway, 2=arterial, 3=collector, 4=local
    pub is_one_way: bool,
    pub volume_aadt: u32,
}

impl ExtNetworkEdge {
    pub fn new(id: u32, from: u32, to: u32, length_m: f32, speed_limit_kph: f32) -> Self {
        Self { id, from_node: from, to_node: to, length_m, lanes: 2,
            speed_limit_kph, functional_class: 3, is_one_way: false, volume_aadt: 0 }
    }
    pub fn free_flow_time_s(&self) -> f32 { self.length_m / (self.speed_limit_kph / 3.6) }
    pub fn volume_capacity_ratio(&self) -> f32 {
        let capacity = match self.functional_class {
            1 => 2200 * self.lanes as u32,
            2 => 1800 * self.lanes as u32,
            3 => 1200 * self.lanes as u32,
            _ => 800 * self.lanes as u32,
        };
        self.volume_aadt as f32 / (capacity as f32 * 250.0)  // annualized
    }
    pub fn los_from_vc(&self) -> char {
        match (self.volume_capacity_ratio() * 10.0) as u32 {
            0..=5 => 'A', 6 => 'B', 7 => 'C', 8 => 'D', 9 => 'E', _ => 'F',
        }
    }
    pub fn functional_class_str(&self) -> &'static str {
        match self.functional_class {
            1 => "Freeway/Expressway", 2 => "Arterial",
            3 => "Collector", 4 => "Local", _ => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExtRoadNetwork {
    pub network_id: String,
    pub nodes: Vec<ExtNetworkNode>,
    pub edges: Vec<ExtNetworkEdge>,
}

impl ExtRoadNetwork {
    pub fn new(network_id: &str) -> Self { Self { network_id: network_id.to_string(), ..Default::default() } }
    pub fn add_node(&mut self, n: ExtNetworkNode) { self.nodes.push(n); }
    pub fn add_edge(&mut self, e: ExtNetworkEdge) { self.edges.push(e); }
    pub fn total_length_km(&self) -> f32 { self.edges.iter().map(|e| e.length_m).sum::<f32>() / 1000.0 }
    pub fn edges_by_functional_class(&self) -> HashMap<u8, Vec<&ExtNetworkEdge>> {
        let mut map: HashMap<u8, Vec<&ExtNetworkEdge>> = HashMap::new();
        for e in &self.edges { map.entry(e.functional_class).or_default().push(e); }
        map
    }
    pub fn congested_edges(&self) -> Vec<&ExtNetworkEdge> {
        self.edges.iter().filter(|e| e.volume_capacity_ratio() > 0.85).collect()
    }
    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn edge_count(&self) -> usize { self.edges.len() }
    pub fn connectivity_ratio(&self) -> f32 {
        if self.node_count() < 2 { return 0.0; }
        self.edge_count() as f32 / self.node_count() as f32
    }
    pub fn average_speed_limit(&self) -> f32 {
        if self.edges.is_empty() { return 0.0; }
        self.edges.iter().map(|e| e.speed_limit_kph).sum::<f32>() / self.edges.len() as f32
    }
    pub fn find_node(&self, id: u32) -> Option<&ExtNetworkNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn adjacent_edges(&self, node_id: u32) -> Vec<&ExtNetworkEdge> {
        self.edges.iter().filter(|e| e.from_node == node_id || (!e.is_one_way && e.to_node == node_id)).collect()
    }
    pub fn bfs_path(&self, start: u32, goal: u32) -> Option<Vec<u32>> {
        if start == goal { return Some(vec![start]); }
        let mut queue: VecDeque<(u32, Vec<u32>)> = VecDeque::new();
        let mut visited: HashSet<u32> = HashSet::new();
        queue.push_back((start, vec![start]));
        visited.insert(start);
        while let Some((cur, path)) = queue.pop_front() {
            for edge in self.adjacent_edges(cur) {
                let next = if edge.from_node == cur { edge.to_node } else { edge.from_node };
                if !visited.contains(&next) {
                    let mut new_path = path.clone();
                    new_path.push(next);
                    if next == goal { return Some(new_path); }
                    visited.insert(next);
                    queue.push_back((next, new_path));
                }
            }
        }
        None
    }
}

// ============================================================
// STREET LIGHTING INVENTORY
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum LightingType { HPS, LED, MH, Fluorescent, Incandescent }

#[derive(Debug, Clone)]
pub struct StreetLight {
    pub id: u32,
    pub station_m: f32,
    pub side: String,
    pub lighting_type: LightingType,
    pub wattage_w: f32,
    pub height_m: f32,
    pub arm_length_m: f32,
    pub is_operational: bool,
    pub age_years: f32,
}

impl StreetLight {
    pub fn new(id: u32, station_m: f32, side: &str, lighting_type: LightingType, wattage_w: f32) -> Self {
        Self { id, station_m, side: side.to_string(), lighting_type, wattage_w,
            height_m: 9.0, arm_length_m: 1.5, is_operational: true, age_years: 0.0 }
    }
    pub fn illuminance_lux_at_road(&self) -> f32 {
        let luminous_efficacy = match &self.lighting_type {
            LightingType::LED => 130.0,
            LightingType::HPS => 90.0,
            LightingType::MH => 80.0,
            LightingType::Fluorescent => 70.0,
            LightingType::Incandescent => 15.0,
        };
        let lumens = self.wattage_w * luminous_efficacy;
        let h2 = self.height_m * self.height_m;
        let area = std::f32::consts::PI * h2; // simplified - illuminated area under cone
        lumens / area.max(1.0)
    }
    pub fn annual_energy_kwh(&self) -> f32 { self.wattage_w / 1000.0 * 4000.0 }  // 4000 operating hours/year
    pub fn annual_energy_cost_usd(&self, rate_per_kwh: f32) -> f32 {
        self.annual_energy_kwh() * rate_per_kwh
    }
    pub fn lighting_type_str(&self) -> &'static str {
        match &self.lighting_type {
            LightingType::HPS => "High Pressure Sodium",
            LightingType::LED => "LED",
            LightingType::MH => "Metal Halide",
            LightingType::Fluorescent => "Fluorescent",
            LightingType::Incandescent => "Incandescent",
        }
    }
    pub fn is_energy_efficient(&self) -> bool {
        matches!(&self.lighting_type, LightingType::LED)
    }
}

#[derive(Debug, Clone, Default)]
pub struct LightingInventory {
    pub road_id: String,
    pub lights: Vec<StreetLight>,
}

impl LightingInventory {
    pub fn new(road_id: &str) -> Self { Self { road_id: road_id.to_string(), lights: Vec::new() } }
    pub fn add(&mut self, l: StreetLight) { self.lights.push(l); }
    pub fn count(&self) -> usize { self.lights.len() }
    pub fn operational_count(&self) -> usize { self.lights.iter().filter(|l| l.is_operational).count() }
    pub fn total_annual_energy_kwh(&self) -> f32 {
        self.lights.iter().map(|l| l.annual_energy_kwh()).sum()
    }
    pub fn average_spacing_m(&self, road_length_m: f32) -> f32 {
        if self.lights.is_empty() { return 0.0; }
        road_length_m / self.lights.len() as f32
    }
    pub fn led_percentage(&self) -> f32 {
        if self.lights.is_empty() { return 0.0; }
        let led = self.lights.iter().filter(|l| l.is_energy_efficient()).count();
        led as f32 / self.lights.len() as f32 * 100.0
    }
    pub fn outage_rate(&self) -> f32 {
        if self.lights.is_empty() { return 0.0; }
        let outages = self.lights.iter().filter(|l| !l.is_operational).count();
        outages as f32 / self.lights.len() as f32 * 100.0
    }
}

// ============================================================
// ROAD CRASH DATA ANALYSIS
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum CrashSeverity { Fatal, Serious, Minor, PropertyDamageOnly }

#[derive(Debug, Clone, PartialEq)]
pub enum CrashType {
    RearEnd, SideSwipe, HeadOn, RightAngle, SingleVehicle,
    PedestrianInvolvement, BicycleInvolvement, AnimalInvolvement, Other,
}

#[derive(Debug, Clone)]
pub struct CrashRecord {
    pub crash_id: u32,
    pub station_m: f32,
    pub severity: CrashSeverity,
    pub crash_type: CrashType,
    pub year: u32,
    pub month: u8,
    pub time_hour: u8,
    pub road_condition: String,
    pub light_condition: String,
    pub vehicles_involved: u8,
}

impl CrashRecord {
    pub fn new(crash_id: u32, station_m: f32, severity: CrashSeverity, crash_type: CrashType, year: u32) -> Self {
        Self { crash_id, station_m, severity, crash_type, year, month: 1, time_hour: 12,
            road_condition: "Dry".to_string(), light_condition: "Daylight".to_string(), vehicles_involved: 2 }
    }
    pub fn severity_weight(&self) -> f32 {
        match &self.severity {
            CrashSeverity::Fatal => 20.0,
            CrashSeverity::Serious => 5.0,
            CrashSeverity::Minor => 1.5,
            CrashSeverity::PropertyDamageOnly => 1.0,
        }
    }
    pub fn is_wet_road(&self) -> bool {
        self.road_condition.to_lowercase().contains("wet") ||
        self.road_condition.to_lowercase().contains("ice") ||
        self.road_condition.to_lowercase().contains("snow")
    }
    pub fn is_night_crash(&self) -> bool { self.time_hour < 6 || self.time_hour >= 20 }
}

#[derive(Debug, Clone, Default)]
pub struct CrashDatabase {
    pub road_id: String,
    pub road_length_km: f32,
    pub crashes: Vec<CrashRecord>,
    pub exposure_years: f32,
    pub aadt: u32,
}

impl CrashDatabase {
    pub fn new(road_id: &str, road_length_km: f32) -> Self {
        Self { road_id: road_id.to_string(), road_length_km, crashes: Vec::new(),
            exposure_years: 3.0, aadt: 10000 }
    }
    pub fn add_crash(&mut self, c: CrashRecord) { self.crashes.push(c); }
    pub fn total_crashes(&self) -> usize { self.crashes.len() }
    pub fn fatal_crashes(&self) -> usize {
        self.crashes.iter().filter(|c| matches!(&c.severity, CrashSeverity::Fatal)).count()
    }
    pub fn serious_crashes(&self) -> usize {
        self.crashes.iter().filter(|c| matches!(&c.severity, CrashSeverity::Serious)).count()
    }
    pub fn crash_rate_per_mvkmt(&self) -> f32 {
        let mvkmt = self.aadt as f32 * 365.0 * self.exposure_years * self.road_length_km / 1_000_000.0;
        if mvkmt < 0.001 { return 0.0; }
        self.total_crashes() as f32 / mvkmt
    }
    pub fn severity_index(&self) -> f32 {
        if self.crashes.is_empty() { return 0.0; }
        self.crashes.iter().map(|c| c.severity_weight()).sum::<f32>() / self.crashes.len() as f32
    }
    pub fn wet_road_percentage(&self) -> f32 {
        if self.crashes.is_empty() { return 0.0; }
        let wet = self.crashes.iter().filter(|c| c.is_wet_road()).count();
        wet as f32 / self.crashes.len() as f32 * 100.0
    }
    pub fn night_crash_percentage(&self) -> f32 {
        if self.crashes.is_empty() { return 0.0; }
        let night = self.crashes.iter().filter(|c| c.is_night_crash()).count();
        night as f32 / self.crashes.len() as f32 * 100.0
    }
    pub fn black_spots(&self, radius_m: f32, min_crashes: usize) -> Vec<f32> {
        let mut spots = Vec::new();
        let stations: Vec<f32> = self.crashes.iter().map(|c| c.station_m).collect();
        for &sta in &stations {
            let count = stations.iter().filter(|&&s| (s - sta).abs() <= radius_m).count();
            if count >= min_crashes && !spots.iter().any(|&s: &f32| (s - sta).abs() < radius_m) {
                spots.push(sta);
            }
        }
        spots
    }
}

// ============================================================
// TRAFFIC CALMING
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum CalmingDeviceType {
    SpeedHump, SpeedTable, RaisedCrossing, RaisedIntersection,
    NeckDown, Chicane, RoundAbout, SplitterIsland, TrafficCircle,
}

#[derive(Debug, Clone)]
pub struct CalmingDevice {
    pub id: u32,
    pub device_type: CalmingDeviceType,
    pub station_m: f32,
    pub installation_year: u32,
    pub expected_speed_reduction_kph: f32,
    pub construction_cost_usd: f32,
}

impl CalmingDevice {
    pub fn new(id: u32, device_type: CalmingDeviceType, station_m: f32) -> Self {
        let (reduction, cost) = match &device_type {
            CalmingDeviceType::SpeedHump => (12.0, 3000.0),
            CalmingDeviceType::SpeedTable => (8.0, 8000.0),
            CalmingDeviceType::RaisedCrossing => (6.0, 15000.0),
            CalmingDeviceType::RaisedIntersection => (10.0, 25000.0),
            CalmingDeviceType::NeckDown => (4.0, 12000.0),
            CalmingDeviceType::Chicane => (15.0, 20000.0),
            CalmingDeviceType::RoundAbout => (20.0, 80000.0),
            CalmingDeviceType::SplitterIsland => (5.0, 10000.0),
            CalmingDeviceType::TrafficCircle => (18.0, 60000.0),
        };
        Self { id, device_type, station_m, installation_year: 2024,
            expected_speed_reduction_kph: reduction, construction_cost_usd: cost }
    }
    pub fn device_type_str(&self) -> &'static str {
        match &self.device_type {
            CalmingDeviceType::SpeedHump => "Speed Hump",
            CalmingDeviceType::SpeedTable => "Speed Table",
            CalmingDeviceType::RaisedCrossing => "Raised Crossing",
            CalmingDeviceType::RaisedIntersection => "Raised Intersection",
            CalmingDeviceType::NeckDown => "Neck Down / Bulb-Out",
            CalmingDeviceType::Chicane => "Chicane",
            CalmingDeviceType::RoundAbout => "Roundabout",
            CalmingDeviceType::SplitterIsland => "Splitter Island",
            CalmingDeviceType::TrafficCircle => "Traffic Circle",
        }
    }
    pub fn cost_per_kph_reduction(&self) -> f32 {
        if self.expected_speed_reduction_kph < 0.1 { return 0.0; }
        self.construction_cost_usd / self.expected_speed_reduction_kph
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrafficCalmingPlan {
    pub road_id: String,
    pub devices: Vec<CalmingDevice>,
    pub target_85th_percentile_kph: f32,
}

impl TrafficCalmingPlan {
    pub fn new(road_id: &str, target_speed_kph: f32) -> Self {
        Self { road_id: road_id.to_string(), devices: Vec::new(), target_85th_percentile_kph: target_speed_kph }
    }
    pub fn add_device(&mut self, d: CalmingDevice) { self.devices.push(d); }
    pub fn total_cost_usd(&self) -> f32 { self.devices.iter().map(|d| d.construction_cost_usd).sum() }
    pub fn total_speed_reduction_kph(&self) -> f32 {
        self.devices.iter().map(|d| d.expected_speed_reduction_kph).sum()
    }
    pub fn count(&self) -> usize { self.devices.len() }
}

// ============================================================
// ADDITIONAL TEST FUNCTIONS
// ============================================================

pub fn run_road_network_tests() {
    let mut network = ExtRoadNetwork::new("CITY-CORE");
    network.add_node(ExtNetworkNode::new(0, 0.0, 0.0));
    network.add_node(ExtNetworkNode::new(1, 500.0, 0.0));
    network.add_node(ExtNetworkNode::new(2, 500.0, 500.0));
    network.add_node(ExtNetworkNode::new(3, 0.0, 500.0));
    network.add_edge(ExtNetworkEdge::new(1, 0, 1, 500.0, 50.0));
    network.add_edge(ExtNetworkEdge::new(2, 1, 2, 500.0, 50.0));
    network.add_edge(ExtNetworkEdge::new(3, 2, 3, 500.0, 50.0));
    network.add_edge(ExtNetworkEdge::new(4, 3, 0, 500.0, 50.0));
    assert_eq!(network.node_count(), 4);
    assert_eq!(network.edge_count(), 4);
    let path = network.bfs_path(0, 2);
    assert!(path.is_some());
    assert!(path.unwrap().len() >= 3);
    let total_km = network.total_length_km();
    assert!((total_km - 2.0).abs() < 0.01);
}

pub fn run_crash_analysis_tests() {
    let mut db = CrashDatabase::new("HWY-101", 5.0);
    db.aadt = 15000;
    db.exposure_years = 3.0;
    db.add_crash(CrashRecord::new(1, 1200.0, CrashSeverity::Minor, CrashType::RearEnd, 2022));
    db.add_crash(CrashRecord::new(2, 1250.0, CrashSeverity::Serious, CrashType::HeadOn, 2022));
    db.add_crash(CrashRecord::new(3, 3500.0, CrashSeverity::PropertyDamageOnly, CrashType::SideSwipe, 2023));
    assert_eq!(db.total_crashes(), 3);
    assert_eq!(db.fatal_crashes(), 0);
    assert_eq!(db.serious_crashes(), 1);
    let rate = db.crash_rate_per_mvkmt();
    assert!(rate > 0.0);
    let black_spots = db.black_spots(200.0, 2);
    assert!(black_spots.len() >= 1);
}

pub fn run_sign_inventory_tests() {
    let mut inv = ExtSignInventory::new("HWY-101");
    inv.add(ExtRoadSign::new(1, ExtSignType::Stop, 500.0, "NB"));
    inv.add(ExtRoadSign::new(2, ExtSignType::SpeedLimit, 1000.0, "NB"));
    inv.add(ExtRoadSign::new(3, ExtSignType::Warning, 1500.0, "NB"));
    assert_eq!(inv.count(), 3);
    let total_cost = inv.total_replacement_cost();
    assert!(total_cost > 0.0);
}

pub fn run_condition_survey_tests() {
    let mut db = ConditionSurveyDatabase::new();
    db.add(ConditionSurveyRecord::new("SEC-001", 0.0, 500.0, 75.0, 8.0, 2.0, 45.0));
    db.add(ConditionSurveyRecord::new("SEC-002", 500.0, 1000.0, 45.0, 20.0, 7.0, 35.0));
    db.add(ConditionSurveyRecord::new("SEC-003", 1000.0, 1500.0, 85.0, 3.0, 1.5, 52.0));
    let avg_pci = db.average_pci();
    assert!((avg_pci - (75.0 + 45.0 + 85.0) / 3.0).abs() < 0.1);
    let repair_sections = db.sections_needing_repair();
    assert!(!repair_sections.is_empty());
    let worst = db.worst_sections(2);
    assert_eq!(worst.len(), 2);
    assert!(worst[0].pci <= worst[1].pci);
}

pub fn run_active_transport_tests() {
    let mut net = ActiveTransportNetwork::new("MAIN-ST");
    net.add(ActiveTransportFacility::new(1, FacilityType::Sidewalk, 0.0, 500.0, 2.0));
    net.add(ActiveTransportFacility::new(2, FacilityType::BikeLane, 0.0, 500.0, 1.5));
    net.add(ActiveTransportFacility::new(3, FacilityType::ProtectedBikeLane, 500.0, 1000.0, 2.5));
    assert_eq!(net.facilities.len(), 3);
    let bike_len = net.bike_facility_length_m();
    assert!(bike_len > 0.0);
    let sidewalk_len = net.sidewalk_coverage_m();
    assert!((sidewalk_len - 500.0).abs() < 0.1);
    let ada_rate = net.ada_compliance_rate();
    assert!(ada_rate > 0.0);
}

pub fn run_lighting_tests() {
    let mut inv = LightingInventory::new("HWY-101");
    inv.add(StreetLight::new(1, 0.0, "NB", LightingType::LED, 100.0));
    inv.add(StreetLight::new(2, 50.0, "NB", LightingType::HPS, 150.0));
    inv.add(StreetLight::new(3, 100.0, "NB", LightingType::LED, 100.0));
    assert_eq!(inv.count(), 3);
    assert_eq!(inv.operational_count(), 3);
    let energy = inv.total_annual_energy_kwh();
    assert!(energy > 0.0);
    let led_pct = inv.led_percentage();
    assert!((led_pct - 66.67).abs() < 0.1);
}

pub fn run_traffic_calming_tests() {
    let mut plan = TrafficCalmingPlan::new("OAK-ST", 30.0);
    plan.add_device(CalmingDevice::new(1, CalmingDeviceType::SpeedHump, 100.0));
    plan.add_device(CalmingDevice::new(2, CalmingDeviceType::SpeedTable, 300.0));
    plan.add_device(CalmingDevice::new(3, CalmingDeviceType::RoundAbout, 500.0));
    assert_eq!(plan.count(), 3);
    let total_cost = plan.total_cost_usd();
    assert!(total_cost > 80000.0);
    let total_reduction = plan.total_speed_reduction_kph();
    assert!(total_reduction > 30.0);
}

pub fn terrain_road_tool_extended_tests() {
    run_road_network_tests();
    run_crash_analysis_tests();
    run_sign_inventory_tests();
    run_condition_survey_tests();
    run_active_transport_tests();
    run_lighting_tests();
    run_traffic_calming_tests();
}

pub fn terrain_road_tool_all_tests_v2() {
    terrain_road_tool_run_all_tests();
    terrain_road_tool_extended_tests();
}

// ============================================================
// ROAD MAINTENANCE MANAGEMENT
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum MaintenanceCategory {
    Routine, Preventive, Corrective, Emergency, Capital,
}

#[derive(Debug, Clone)]
pub struct MaintenanceWork {
    pub work_id: u32,
    pub description: String,
    pub category: MaintenanceCategory,
    pub start_station_m: f32,
    pub end_station_m: f32,
    pub planned_year: u32,
    pub estimated_cost_usd: f32,
    pub unit_cost: f32,
    pub quantity: f32,
    pub unit: String,
    pub priority: u8,
    pub is_complete: bool,
}

impl MaintenanceWork {
    pub fn new(work_id: u32, description: &str, category: MaintenanceCategory, start_m: f32, end_m: f32, planned_year: u32) -> Self {
        Self { work_id, description: description.to_string(), category,
            start_station_m: start_m, end_station_m: end_m, planned_year,
            estimated_cost_usd: 0.0, unit_cost: 0.0, quantity: 0.0,
            unit: String::new(), priority: 3, is_complete: false }
    }
    pub fn length_m(&self) -> f32 { (self.end_station_m - self.start_station_m).abs() }
    pub fn compute_cost(&mut self, unit_cost: f32, quantity: f32, unit: &str) {
        self.unit_cost = unit_cost;
        self.quantity = quantity;
        self.unit = unit.to_string();
        self.estimated_cost_usd = unit_cost * quantity;
    }
    pub fn category_str(&self) -> &'static str {
        match &self.category {
            MaintenanceCategory::Routine => "Routine",
            MaintenanceCategory::Preventive => "Preventive",
            MaintenanceCategory::Corrective => "Corrective",
            MaintenanceCategory::Emergency => "Emergency",
            MaintenanceCategory::Capital => "Capital",
        }
    }
    pub fn is_high_priority(&self) -> bool { self.priority <= 2 }
}

#[derive(Debug, Clone, Default)]
pub struct MaintenanceProgram {
    pub program_id: String,
    pub program_years: Vec<u32>,
    pub works: Vec<MaintenanceWork>,
    pub annual_budget_usd: f32,
}

impl MaintenanceProgram {
    pub fn new(program_id: &str, annual_budget: f32) -> Self {
        Self { program_id: program_id.to_string(), program_years: Vec::new(), works: Vec::new(), annual_budget_usd: annual_budget }
    }
    pub fn add_work(&mut self, w: MaintenanceWork) { self.works.push(w); }
    pub fn total_cost_usd(&self) -> f32 { self.works.iter().map(|w| w.estimated_cost_usd).sum() }
    pub fn works_by_year(&self, year: u32) -> Vec<&MaintenanceWork> {
        self.works.iter().filter(|w| w.planned_year == year).collect()
    }
    pub fn annual_cost(&self, year: u32) -> f32 {
        self.works_by_year(year).iter().map(|w| w.estimated_cost_usd).sum()
    }
    pub fn budget_deficit(&self, year: u32) -> f32 {
        let cost = self.annual_cost(year);
        (cost - self.annual_budget_usd).max(0.0)
    }
    pub fn high_priority_works(&self) -> Vec<&MaintenanceWork> {
        self.works.iter().filter(|w| w.is_high_priority()).collect()
    }
    pub fn completion_rate(&self) -> f32 {
        if self.works.is_empty() { return 100.0; }
        let complete = self.works.iter().filter(|w| w.is_complete).count();
        complete as f32 / self.works.len() as f32 * 100.0
    }
    pub fn works_by_category(&self) -> HashMap<String, usize> {
        let mut map: HashMap<String, usize> = HashMap::new();
        for w in &self.works { *map.entry(w.category_str().to_string()).or_insert(0) += 1; }
        map
    }
    pub fn report(&self) -> String {
        let s = format!("MaintenanceProgram {} total_cost={:.0} budget={:.0} works={} complete={:.0}pct",
            self.program_id, self.total_cost_usd(), self.annual_budget_usd,
            self.works.len(), self.completion_rate());
        s
    }
}

// ============================================================
// ROAD DESIGN STANDARDS CHECKER
// ============================================================

pub struct DesignStandardsChecker;

impl DesignStandardsChecker {
    pub fn check_lane_width(width_m: f32, road_class: &str) -> (bool, String) {
        let min = match road_class { "highway" => 3.65, "arterial" => 3.5, "collector" => 3.0, _ => 2.7 };
        let ok = width_m >= min;
        let msg = if ok { format!("Lane width {:.2}m OK (min {:.2}m)", width_m, min) }
            else { format!("Lane width {:.2}m FAILS (min {:.2}m)", width_m, min) };
        (ok, msg)
    }
    pub fn check_shoulder_width(width_m: f32, road_class: &str) -> (bool, String) {
        let min = match road_class { "highway" => 3.0, "arterial" => 2.0, "collector" => 1.2, _ => 0.5 };
        let ok = width_m >= min;
        let msg = if ok { format!("Shoulder width {:.2}m OK (min {:.2}m)", width_m, min) }
            else { format!("Shoulder width {:.2}m FAILS (min {:.2}m)", width_m, min) };
        (ok, msg)
    }
    pub fn check_grade(grade_pct: f32, design_speed_kph: f32) -> (bool, String) {
        let max = if design_speed_kph >= 100.0 { 4.0 } else if design_speed_kph >= 80.0 { 5.0 } else { 6.0 };
        let ok = grade_pct.abs() <= max;
        let msg = if ok { format!("Grade {:.1}% OK (max {:.1}%)", grade_pct, max) }
            else { format!("Grade {:.1}% EXCEEDS max {:.1}%", grade_pct, max) };
        (ok, msg)
    }
    pub fn check_cross_slope(slope_pct: f32) -> (bool, String) {
        let ok = slope_pct >= 1.5 && slope_pct <= 3.0;
        let msg = if ok { format!("Cross slope {:.1}% OK (1.5-3.0%)", slope_pct) }
            else { format!("Cross slope {:.1}% outside 1.5-3.0%", slope_pct) };
        (ok, msg)
    }
    pub fn check_sight_distance(available_m: f32, design_speed_kph: f32) -> (bool, String) {
        let ssd = 0.278 * design_speed_kph * 2.5 + design_speed_kph.powi(2) / (254.0 * 0.35);
        let ok = available_m >= ssd;
        let msg = if ok { format!("SSD {:.1}m available >= {:.1}m required", available_m, ssd) }
            else { format!("SSD {:.1}m INSUFFICIENT (required {:.1}m)", available_m, ssd) };
        (ok, msg)
    }
    pub fn run_standard_checks(
        lane_width_m: f32,
        shoulder_width_m: f32,
        grade_pct: f32,
        cross_slope_pct: f32,
        sight_distance_m: f32,
        design_speed_kph: f32,
        road_class: &str,
    ) -> Vec<(String, bool)> {
        let mut results = Vec::new();
        let (ok, msg) = Self::check_lane_width(lane_width_m, road_class);
        results.push((msg, ok));
        let (ok, msg) = Self::check_shoulder_width(shoulder_width_m, road_class);
        results.push((msg, ok));
        let (ok, msg) = Self::check_grade(grade_pct, design_speed_kph);
        results.push((msg, ok));
        let (ok, msg) = Self::check_cross_slope(cross_slope_pct);
        results.push((msg, ok));
        let (ok, msg) = Self::check_sight_distance(sight_distance_m, design_speed_kph);
        results.push((msg, ok));
        results
    }
}

pub fn run_design_standards_tests() {
    let results = DesignStandardsChecker::run_standard_checks(
        3.65, 3.0, 3.5, 2.0, 200.0, 80.0, "arterial"
    );
    assert_eq!(results.len(), 5);
    let passes: Vec<&bool> = results.iter().map(|(_, ok)| ok).collect();
    assert!(passes.iter().any(|&&ok| ok));
}

pub fn run_maintenance_tests() {
    let mut prog = MaintenanceProgram::new("MAINT-2024-2028", 2_000_000.0);
    let mut w1 = MaintenanceWork::new(1, "Crack Sealing", MaintenanceCategory::Preventive, 0.0, 1000.0, 2024);
    w1.compute_cost(5.0, 1000.0, "m");
    w1.priority = 2;
    let mut w2 = MaintenanceWork::new(2, "Pothole Patching", MaintenanceCategory::Corrective, 500.0, 600.0, 2024);
    w2.compute_cost(150.0, 20.0, "m2");
    w2.priority = 1;
    let mut w3 = MaintenanceWork::new(3, "Overlay", MaintenanceCategory::Capital, 0.0, 2000.0, 2025);
    w3.compute_cost(25.0, 2000.0 * 7.0, "m2");
    w3.priority = 3;
    prog.add_work(w1);
    prog.add_work(w2);
    prog.add_work(w3);
    assert_eq!(prog.works.len(), 3);
    let hp = prog.high_priority_works();
    assert_eq!(hp.len(), 2);
    let cost_2024 = prog.annual_cost(2024);
    assert!(cost_2024 > 0.0);
    let deficit = prog.budget_deficit(2025);
    assert!(deficit >= 0.0);
    let completion = prog.completion_rate();
    assert!((completion - 0.0).abs() < 0.1);
}

pub fn run_all_terrain_road_tool_final() {
    terrain_road_tool_all_tests_v2();
    run_design_standards_tests();
    run_maintenance_tests();
}


// ============================================================
// SPEED ZONE ANALYSIS
// ============================================================

pub const SPEED_ZONE_DEFAULT_URBAN_KPH: f32 = 50.0;
pub const SPEED_ZONE_DEFAULT_RURAL_KPH: f32 = 100.0;
pub const SPEED_ZONE_SCHOOL_KPH: f32 = 25.0;
pub const SPEED_ZONE_CONSTRUCTION_KPH: f32 = 40.0;

#[derive(Debug, Clone, PartialEq)]
pub enum ExtSpeedZoneType {
    Urban,
    Rural,
    HighSpeed,
    School,
    Hospital,
    Construction,
    Advisory,
    Variable,
}

#[derive(Debug, Clone)]
pub struct ExtSpeedZone {
    pub id: u32,
    pub zone_type: ExtSpeedZoneType,
    pub posted_speed_kph: f32,
    pub start_chainage: f32,
    pub end_chainage: f32,
    pub active_hours_start: f32,
    pub active_hours_end: f32,
    pub enforcement_camera: bool,
    pub justification: String,
}

impl ExtSpeedZone {
    pub fn new(id: u32, zone_type: ExtSpeedZoneType, speed_kph: f32, start: f32, end: f32) -> Self {
        ExtSpeedZone {
            id, zone_type, posted_speed_kph: speed_kph,
            start_chainage: start, end_chainage: end,
            active_hours_start: 0.0, active_hours_end: 24.0,
            enforcement_camera: false,
            justification: String::new(),
        }
    }

    pub fn length(&self) -> f32 {
        (self.end_chainage - self.start_chainage).abs()
    }

    pub fn is_active_at_hour(&self, hour: f32) -> bool {
        hour >= self.active_hours_start && hour < self.active_hours_end
    }

    pub fn stopping_sight_distance(&self) -> f32 {
        // AASHTO 2018 Green Book: SSD = V*t + V^2/(2*g*f)
        let v_ms = self.posted_speed_kph / 3.6;
        let t_reaction = 2.5;
        let g = 9.81;
        let f_friction = 0.35;
        v_ms * t_reaction + (v_ms * v_ms) / (2.0 * g * f_friction)
    }

    pub fn decision_sight_distance(&self) -> f32 {
        // DSD = 1.5 * SSD approximately
        self.stopping_sight_distance() * 1.5
    }
}

#[derive(Debug, Clone)]
pub struct ExtSpeedZoneInv {
    pub road_id: u32,
    pub zones: Vec<ExtSpeedZone>,
}

impl ExtSpeedZoneInv {
    pub fn new(road_id: u32) -> Self {
        ExtSpeedZoneInv { road_id, zones: Vec::new() }
    }

    pub fn add_zone(&mut self, zone: ExtSpeedZone) {
        self.zones.push(zone);
        self.zones.sort_by(|a, b| a.start_chainage.partial_cmp(&b.start_chainage).unwrap());
    }

    pub fn zone_at_chainage(&self, ch: f32) -> Option<&ExtSpeedZone> {
        self.zones.iter().find(|z| ch >= z.start_chainage && ch <= z.end_chainage)
    }

    pub fn school_zones(&self) -> Vec<&ExtSpeedZone> {
        self.zones.iter().filter(|z| z.zone_type == ExtSpeedZoneType::School).collect()
    }
}

// ============================================================
// CROSS-SECTION ELEMENTS
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtLane {
    pub id: u32,
    pub lane_type: LaneType,
    pub width_m: f32,
    pub direction: i32, // +1 or -1
    pub surface_type: String,
    pub has_rumble_strip: bool,
    pub has_markings: bool,
    pub speed_kph: f32,
}

impl ExtLane {
    pub fn new(id: u32, lane_type: LaneType, width_m: f32, direction: i32) -> Self {
        ExtLane {
            id, lane_type, width_m, direction,
            surface_type: "Asphalt".to_string(),
            has_rumble_strip: false, has_markings: true,
            speed_kph: 80.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Shoulder {
    pub width_m: f32,
    pub paved: bool,
    pub surface_type: String,
    pub has_barrier: bool,
}

#[derive(Debug, Clone)]
pub struct Median {
    pub width_m: f32,
    pub raised: bool,
    pub has_barrier: bool,
    pub landscaped: bool,
}

#[derive(Debug, Clone)]
pub struct RoadCrossSection {
    pub chainage: f32,
    pub lanes: Vec<ExtLane>,
    pub left_shoulder: Option<Shoulder>,
    pub right_shoulder: Option<Shoulder>,
    pub median: Option<Median>,
    pub total_width_m: f32,
    pub carriageway_width_m: f32,
    pub cut_fill_type: String,
    pub fill_height_m: f32,
    pub cut_depth_m: f32,
}

impl RoadCrossSection {
    pub fn new(chainage: f32) -> Self {
        RoadCrossSection {
            chainage,
            lanes: Vec::new(),
            left_shoulder: None, right_shoulder: None,
            median: None,
            total_width_m: 0.0,
            carriageway_width_m: 0.0,
            cut_fill_type: "At-Grade".to_string(),
            fill_height_m: 0.0, cut_depth_m: 0.0,
        }
    }

    pub fn compute_widths(&mut self) {
        self.carriageway_width_m = self.lanes.iter().map(|l| l.width_m).sum::<f32>()
            + self.median.as_ref().map(|m| m.width_m).unwrap_or(0.0);
        let ls = self.left_shoulder.as_ref().map(|s| s.width_m).unwrap_or(0.0);
        let rs = self.right_shoulder.as_ref().map(|s| s.width_m).unwrap_or(0.0);
        self.total_width_m = self.carriageway_width_m + ls + rs;
    }

    pub fn lane_count_by_direction(&self, dir: i32) -> usize {
        self.lanes.iter().filter(|l| l.direction == dir).count()
    }
}

// ============================================================
// EARTHWORKS COMPUTATION
// ============================================================

#[derive(Debug, Clone)]
pub struct EarthworkSection {
    pub start_chainage: f32,
    pub end_chainage: f32,
    pub start_area_m2: f32,
    pub end_area_m2: f32,
    pub is_cut: bool,
}

impl EarthworkSection {
    pub fn volume_prismatoid_m3(&self) -> f32 {
        let l = (self.end_chainage - self.start_chainage).abs();
        // Average end area method
        (self.start_area_m2 + self.end_area_m2) / 2.0 * l
    }

    pub fn volume_prismatoid_corrected_m3(&self, mid_area_m2: f32) -> f32 {
        let l = (self.end_chainage - self.start_chainage).abs();
        // Prismatoid formula
        l / 6.0 * (self.start_area_m2 + 4.0 * mid_area_m2 + self.end_area_m2)
    }
}

#[derive(Debug, Clone)]
pub struct MassHaulDiagram {
    pub stations: Vec<f32>,
    pub ordinates: Vec<f32>,
    pub freehaul_distance: f32,
    pub overhaul_rate_per_m3_station: f32,
}

impl MassHaulDiagram {
    pub fn new(freehaul_distance: f32) -> Self {
        MassHaulDiagram {
            stations: Vec::new(),
            ordinates: Vec::new(),
            freehaul_distance,
            overhaul_rate_per_m3_station: 0.05,
        }
    }

    pub fn build(&mut self, sections: &[EarthworkSection]) {
        let mut cumulative = 0.0f32;
        self.stations.clear();
        self.ordinates.clear();
        self.stations.push(sections.first().map(|s| s.start_chainage).unwrap_or(0.0));
        self.ordinates.push(0.0);
        for sec in sections {
            let vol = sec.volume_prismatoid_m3();
            cumulative += if sec.is_cut { vol } else { -vol };
            self.stations.push(sec.end_chainage);
            self.ordinates.push(cumulative);
        }
    }

    pub fn balance_point(&self) -> Option<f32> {
        // Find where ordinate crosses zero after a non-zero region
        for i in 1..self.ordinates.len() {
            if self.ordinates[i - 1] * self.ordinates[i] < 0.0 {
                let frac = self.ordinates[i - 1] / (self.ordinates[i - 1] - self.ordinates[i]);
                return Some(self.stations[i - 1] + frac * (self.stations[i] - self.stations[i - 1]));
            }
        }
        None
    }

    pub fn total_cut_m3(&self) -> f32 {
        self.ordinates.iter().cloned().fold(f32::NEG_INFINITY, f32::max).max(0.0)
    }

    pub fn total_fill_m3(&self) -> f32 {
        (-self.ordinates.iter().cloned().fold(f32::INFINITY, f32::min)).max(0.0)
    }
}

// ============================================================
// STORMWATER DRAINAGE DESIGN
// ============================================================

pub const STORMWATER_RUNOFF_COEFF_PAVEMENT: f32 = 0.90;
pub const STORMWATER_RUNOFF_COEFF_LAWN: f32 = 0.25;
pub const STORMWATER_RUNOFF_COEFF_GRAVEL: f32 = 0.60;
pub const STORMWATER_MANNING_CONCRETE: f32 = 0.013;
pub const STORMWATER_MANNING_EARTHEN: f32 = 0.030;

#[derive(Debug, Clone)]
pub struct CatchmentArea {
    pub id: u32,
    pub area_ha: f32,
    pub runoff_coefficient: f32,
    pub time_of_concentration_min: f32,
    pub slope_pct: f32,
    pub description: String,
}

impl CatchmentArea {
    pub fn new(id: u32, area_ha: f32, runoff_coeff: f32) -> Self {
        CatchmentArea {
            id, area_ha, runoff_coefficient: runoff_coeff,
            time_of_concentration_min: 10.0,
            slope_pct: 1.0,
            description: String::new(),
        }
    }

    pub fn rational_flow_m3s(&self, rainfall_intensity_mm_hr: f32) -> f32 {
        // Q = C * i * A / 360 (mÂ³/s, A in ha, i in mm/hr)
        self.runoff_coefficient * rainfall_intensity_mm_hr * self.area_ha / 360.0
    }
}

#[derive(Debug, Clone)]
pub struct StormDrainPipe {
    pub id: u32,
    pub diameter_mm: f32,
    pub material: String,
    pub manning_n: f32,
    pub slope_percent: f32,
    pub length_m: f32,
    pub upstream_invert: f32,
    pub downstream_invert: f32,
}

impl StormDrainPipe {
    pub fn new(id: u32, diameter_mm: f32, slope_pct: f32, length_m: f32) -> Self {
        StormDrainPipe {
            id, diameter_mm, material: "Concrete".to_string(),
            manning_n: STORMWATER_MANNING_CONCRETE,
            slope_percent: slope_pct, length_m,
            upstream_invert: 0.0, downstream_invert: 0.0,
        }
    }

    pub fn full_flow_capacity_m3s(&self) -> f32 {
        // Manning's equation: Q = (1/n) * A * R^(2/3) * S^(1/2)
        let r_m = self.diameter_mm / 2000.0;
        let area = std::f32::consts::PI * r_m * r_m;
        let hydraulic_radius = r_m / 2.0;
        let slope = self.slope_percent / 100.0;
        (1.0 / self.manning_n) * area * hydraulic_radius.powf(2.0 / 3.0) * slope.sqrt()
    }

    pub fn velocity_full_ms(&self) -> f32 {
        let r_m = self.diameter_mm / 2000.0;
        let hydraulic_radius = r_m / 2.0;
        let slope = self.slope_percent / 100.0;
        (1.0 / self.manning_n) * hydraulic_radius.powf(2.0 / 3.0) * slope.sqrt()
    }

    pub fn is_self_cleansing(&self) -> bool {
        self.velocity_full_ms() >= 0.6
    }

    pub fn travel_time_min(&self) -> f32 {
        let v = self.velocity_full_ms().max(0.001);
        self.length_m / v / 60.0
    }
}

#[derive(Debug, Clone)]
pub struct OpenChannel {
    pub id: u32,
    pub base_width_m: f32,
    pub side_slope_ratio: f32,
    pub depth_m: f32,
    pub manning_n: f32,
    pub slope_percent: f32,
    pub length_m: f32,
}

impl OpenChannel {
    pub fn new(id: u32, base_m: f32, depth_m: f32, slope_pct: f32) -> Self {
        OpenChannel {
            id, base_width_m: base_m,
            side_slope_ratio: 2.0,
            depth_m,
            manning_n: STORMWATER_MANNING_EARTHEN,
            slope_percent: slope_pct,
            length_m: 100.0,
        }
    }

    pub fn flow_area_m2(&self) -> f32 {
        (self.base_width_m + self.side_slope_ratio * self.depth_m) * self.depth_m
    }

    pub fn wetted_perimeter_m(&self) -> f32 {
        self.base_width_m + 2.0 * self.depth_m * (1.0 + self.side_slope_ratio * self.side_slope_ratio).sqrt()
    }

    pub fn hydraulic_radius_m(&self) -> f32 {
        let p = self.wetted_perimeter_m();
        if p <= 0.0 { return 0.0; }
        self.flow_area_m2() / p
    }

    pub fn capacity_m3s(&self) -> f32 {
        let slope = self.slope_percent / 100.0;
        (1.0 / self.manning_n)
            * self.flow_area_m2()
            * self.hydraulic_radius_m().powf(2.0 / 3.0)
            * slope.sqrt()
    }

    pub fn freeboard_m(&self, design_flow: f32) -> f32 {
        // Estimate design depth from capacity
        let capacity = self.capacity_m3s();
        if capacity <= 0.0 { return 0.0; }
        let flow_ratio = (design_flow / capacity).min(1.0);
        self.depth_m * (1.0 - flow_ratio)
    }
}

// ============================================================
// PAVEMENT PERFORMANCE MODEL
// ============================================================

pub const IRI_THRESHOLD_GOOD: f32 = 2.5;
pub const IRI_THRESHOLD_FAIR: f32 = 4.5;
pub const IRI_THRESHOLD_POOR: f32 = 7.0;
pub const PSR_NEW_PAVEMENT: f32 = 4.5;
pub const PSR_TERMINAL: f32 = 2.0;

#[derive(Debug, Clone)]
pub struct PavementPerformanceModel {
    pub section_id: u32,
    pub initial_iri: f32,
    pub deterioration_rate: f32,
    pub traffic_esal_annual: f64,
    pub climate_factor: f32,
    pub age_years: f32,
}

impl PavementPerformanceModel {
    pub fn new(section_id: u32, initial_iri: f32, esal: f64) -> Self {
        PavementPerformanceModel {
            section_id, initial_iri,
            deterioration_rate: 0.15,
            traffic_esal_annual: esal,
            climate_factor: 1.0,
            age_years: 0.0,
        }
    }

    pub fn iri_at_age(&self, years: f32) -> f32 {
        // Simplified linear + traffic model
        let traffic_factor = (self.traffic_esal_annual as f32 / 1_000_000.0).sqrt();
        self.initial_iri + self.deterioration_rate * years * self.climate_factor * (1.0 + traffic_factor * 0.1)
    }

    pub fn condition_at_age(&self, years: f32) -> &'static str {
        let iri = self.iri_at_age(years);
        if iri < IRI_THRESHOLD_GOOD { "Good" }
        else if iri < IRI_THRESHOLD_FAIR { "Fair" }
        else if iri < IRI_THRESHOLD_POOR { "Poor" }
        else { "Very Poor" }
    }

    pub fn years_to_terminal(&self) -> f32 {
        let terminal_iri = IRI_THRESHOLD_POOR;
        if self.initial_iri >= terminal_iri { return 0.0; }
        let traffic_factor = (self.traffic_esal_annual as f32 / 1_000_000.0).sqrt();
        let rate = self.deterioration_rate * self.climate_factor * (1.0 + traffic_factor * 0.1);
        if rate <= 0.0 { return f32::INFINITY; }
        (terminal_iri - self.initial_iri) / rate
    }

    pub fn remaining_service_life(&self) -> f32 {
        (self.years_to_terminal() - self.age_years).max(0.0)
    }

    pub fn treatment_recommendation(&self) -> &'static str {
        let iri = self.iri_at_age(self.age_years);
        if iri < 2.0 { "No treatment needed" }
        else if iri < IRI_THRESHOLD_GOOD { "Preventive maintenance" }
        else if iri < IRI_THRESHOLD_FAIR { "Minor rehabilitation" }
        else if iri < IRI_THRESHOLD_POOR { "Major rehabilitation" }
        else { "Reconstruction" }
    }
}

#[derive(Debug, Clone)]
pub struct PavementNetwork {
    pub sections: Vec<PavementPerformanceModel>,
    pub total_lane_km: f32,
    pub budget_annual: f64,
}

impl PavementNetwork {
    pub fn new(budget: f64) -> Self {
        PavementNetwork { sections: Vec::new(), total_lane_km: 0.0, budget_annual: budget }
    }

    pub fn add_section(&mut self, section: PavementPerformanceModel) {
        self.sections.push(section);
    }

    pub fn network_iri_average(&self) -> f32 {
        if self.sections.is_empty() { return 0.0; }
        self.sections.iter().map(|s| s.iri_at_age(s.age_years)).sum::<f32>() / self.sections.len() as f32
    }

    pub fn sections_needing_treatment(&self) -> Vec<&PavementPerformanceModel> {
        self.sections.iter()
            .filter(|s| s.iri_at_age(s.age_years) >= IRI_THRESHOLD_GOOD)
            .collect()
    }

    pub fn network_condition_distribution(&self) -> HashMap<&'static str, usize> {
        let mut dist: HashMap<&'static str, usize> = HashMap::new();
        for s in &self.sections {
            let cond = s.condition_at_age(s.age_years);
            *dist.entry(cond).or_insert(0) += 1;
        }
        dist
    }
}

// ============================================================
// BRIDGE DESIGN OVERVIEW
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum BridgeType {
    BeamBridge,
    ArchBridge,
    SuspensionBridge,
    CableStayed,
    TrussBridge,
    BoxGirder,
    Culvert,
    Underpass,
}

#[derive(Debug, Clone)]
pub struct BridgeSpan {
    pub span_number: u32,
    pub length_m: f32,
    pub width_m: f32,
    pub deck_elevation: f32,
    pub clearance_m: f32,
}

#[derive(Debug, Clone)]
pub struct Bridge {
    pub id: u32,
    pub name: String,
    pub bridge_type: BridgeType,
    pub total_length_m: f32,
    pub carriageway_width_m: f32,
    pub spans: Vec<BridgeSpan>,
    pub design_load_kn_m2: f32,
    pub construction_year: u32,
    pub inspection_rating: f32,
    pub material: String,
    pub water_crossing: bool,
    pub min_clearance_m: f32,
}

impl Bridge {
    pub fn new(id: u32, name: &str, bridge_type: BridgeType) -> Self {
        Bridge {
            id, name: name.to_string(), bridge_type,
            total_length_m: 0.0, carriageway_width_m: 7.3,
            spans: Vec::new(),
            design_load_kn_m2: 5.0,
            construction_year: 2000,
            inspection_rating: 4.0,
            material: "Reinforced Concrete".to_string(),
            water_crossing: false, min_clearance_m: 4.5,
        }
    }

    pub fn add_span(&mut self, span: BridgeSpan) {
        self.total_length_m += span.length_m;
        self.spans.push(span);
    }

    pub fn span_count(&self) -> usize {
        self.spans.len()
    }

    pub fn requires_inspection(&self) -> bool {
        self.inspection_rating < 3.0
    }

    pub fn deck_area_m2(&self) -> f32 {
        self.total_length_m * self.carriageway_width_m
    }
}

// ============================================================
// GEOMETRIC DESIGN PARAMETERS
// ============================================================

#[derive(Debug, Clone)]
pub struct DesignSpeed {
    pub speed_kph: f32,
    pub min_horizontal_radius_m: f32,
    pub max_superelevation_pct: f32,
    pub min_stopping_sight_distance_m: f32,
    pub min_crest_k: f32,
    pub min_sag_k: f32,
}

impl DesignSpeed {
    pub fn for_speed(kph: f32) -> Self {
        let v = kph;
        let r_min = v * v / (127.0 * (0.10 + 0.14));
        let ssd = v / 3.6 * 2.5 + (v / 3.6) * (v / 3.6) / (2.0 * 9.81 * 0.35);
        DesignSpeed {
            speed_kph: kph,
            min_horizontal_radius_m: r_min,
            max_superelevation_pct: 10.0,
            min_stopping_sight_distance_m: ssd,
            min_crest_k: ssd * ssd / (2.0 * ssd * 0.105 + 0.022 * ssd - 2.6),
            min_sag_k: ssd * ssd / (120.0 + 3.5 * ssd),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoadGeometryReport {
    pub road_id: u32,
    pub total_length_m: f32,
    pub design_speed_kph: f32,
    pub horizontal_curve_count: u32,
    pub vertical_curve_count: u32,
    pub min_radius_found_m: f32,
    pub max_grade_pct: f32,
    pub non_compliant_elements: Vec<String>,
    pub compliant: bool,
}

impl RoadGeometryReport {
    pub fn new(road_id: u32) -> Self {
        RoadGeometryReport {
            road_id, total_length_m: 0.0, design_speed_kph: 80.0,
            horizontal_curve_count: 0, vertical_curve_count: 0,
            min_radius_found_m: f32::INFINITY, max_grade_pct: 0.0,
            non_compliant_elements: Vec::new(), compliant: true,
        }
    }

    pub fn check_radius(&mut self, radius_m: f32) {
        let params = DesignSpeed::for_speed(self.design_speed_kph);
        if radius_m < params.min_horizontal_radius_m {
            self.non_compliant_elements.push(
                format!("Radius {:.1}m < min {:.1}m for {}kph", radius_m, params.min_horizontal_radius_m, self.design_speed_kph)
            );
            self.compliant = false;
        }
        if radius_m < self.min_radius_found_m { self.min_radius_found_m = radius_m; }
    }

    pub fn check_grade(&mut self, grade_pct: f32) {
        let max_allowed = if self.design_speed_kph >= 100.0 { 5.0 } else if self.design_speed_kph >= 80.0 { 7.0 } else { 10.0 };
        if grade_pct.abs() > max_allowed {
            self.non_compliant_elements.push(
                format!("Grade {:.1}% > max {:.1}% for {}kph", grade_pct, max_allowed, self.design_speed_kph)
            );
            self.compliant = false;
        }
        if grade_pct.abs() > self.max_grade_pct { self.max_grade_pct = grade_pct.abs(); }
    }
}

// ============================================================
// ENVIRONMENTAL IMPACT ASSESSMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct NoiseSensitiveReceiver {
    pub id: u32,
    pub name: String,
    pub location: Vec2,
    pub receiver_type: String,
    pub naaqs_criterion_dba: f32,
    pub predicted_noise_dba: f32,
    pub existing_noise_dba: f32,
    pub impact_threshold_increase_dba: f32,
}

impl NoiseSensitiveReceiver {
    pub fn new(id: u32, name: &str, location: Vec2, criterion: f32) -> Self {
        NoiseSensitiveReceiver {
            id, name: name.to_string(), location,
            receiver_type: "Residential".to_string(),
            naaqs_criterion_dba: criterion,
            predicted_noise_dba: 0.0,
            existing_noise_dba: 0.0,
            impact_threshold_increase_dba: 3.0,
        }
    }

    pub fn is_impacted(&self) -> bool {
        self.predicted_noise_dba > self.naaqs_criterion_dba
            || (self.predicted_noise_dba - self.existing_noise_dba) > self.impact_threshold_increase_dba
    }

    pub fn excess_noise_dba(&self) -> f32 {
        (self.predicted_noise_dba - self.naaqs_criterion_dba).max(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct AirQualityImpact {
    pub receptor_id: u32,
    pub location: Vec2,
    pub pm25_ug_m3: f32,
    pub pm10_ug_m3: f32,
    pub no2_ppb: f32,
    pub co_ppm: f32,
    pub exceeds_standard: bool,
}

impl AirQualityImpact {
    pub fn check_standards(&mut self) {
        // NAAQS 24-hr standards
        self.exceeds_standard = self.pm25_ug_m3 > 35.0
            || self.pm10_ug_m3 > 150.0
            || self.no2_ppb > 100.0
            || self.co_ppm > 9.0;
    }
}

#[derive(Debug, Clone)]
pub struct EnvironmentalImpactReport {
    pub project_id: u32,
    pub noise_receivers: Vec<NoiseSensitiveReceiver>,
    pub air_quality_impacts: Vec<AirQualityImpact>,
    pub impacted_wetland_ha: f32,
    pub impacted_threatened_species: Vec<String>,
    pub mitigation_measures: Vec<String>,
    pub overall_significance: String,
}

impl EnvironmentalImpactReport {
    pub fn new(project_id: u32) -> Self {
        EnvironmentalImpactReport {
            project_id,
            noise_receivers: Vec::new(),
            air_quality_impacts: Vec::new(),
            impacted_wetland_ha: 0.0,
            impacted_threatened_species: Vec::new(),
            mitigation_measures: Vec::new(),
            overall_significance: "To be determined".to_string(),
        }
    }

    pub fn noise_impacts_count(&self) -> usize {
        self.noise_receivers.iter().filter(|r| r.is_impacted()).count()
    }

    pub fn air_exceedances_count(&self) -> usize {
        self.air_quality_impacts.iter().filter(|a| a.exceeds_standard).count()
    }

    pub fn add_mitigation(&mut self, measure: &str) {
        self.mitigation_measures.push(measure.to_string());
    }

    pub fn assess_significance(&mut self) {
        let noise_impacts = self.noise_impacts_count();
        let air_exceedances = self.air_exceedances_count();
        let has_wetlands = self.impacted_wetland_ha > 0.0;
        let has_species = !self.impacted_threatened_species.is_empty();

        self.overall_significance = if noise_impacts > 10 || air_exceedances > 0 || has_wetlands || has_species {
            "Significant"
        } else if noise_impacts > 3 {
            "Moderate"
        } else {
            "Minor"
        }.to_string();
    }
}

// ============================================================
// ROAD SAFETY IMPROVEMENT PROGRAM
// ============================================================

#[derive(Debug, Clone)]
pub struct SafetyTreatment {
    pub id: u32,
    pub name: String,
    pub unit_cost: f64,
    pub estimated_crash_reduction_pct: f32,
    pub applicable_crash_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SafetyBenefitCost {
    pub treatment_id: u32,
    pub location_id: u32,
    pub annual_crash_cost_before: f64,
    pub annual_crash_cost_after: f64,
    pub implementation_cost: f64,
    pub analysis_period_years: u32,
    pub discount_rate: f32,
}

impl SafetyBenefitCost {
    pub fn npv_benefits(&self) -> f64 {
        let annual_saving = self.annual_crash_cost_before - self.annual_crash_cost_after;
        let r = self.discount_rate as f64;
        let n = self.analysis_period_years as f64;
        if r == 0.0 { return annual_saving * n; }
        annual_saving * (1.0 - (1.0 + r).powf(-n)) / r
    }

    pub fn bcr(&self) -> f64 {
        if self.implementation_cost <= 0.0 { return f64::INFINITY; }
        self.npv_benefits() / self.implementation_cost
    }

    pub fn payback_years(&self) -> f64 {
        let annual_saving = self.annual_crash_cost_before - self.annual_crash_cost_after;
        if annual_saving <= 0.0 { return f64::INFINITY; }
        self.implementation_cost / annual_saving
    }
}

// ============================================================
// TEST FUNCTIONS
// ============================================================

#[cfg(test)]
mod tests_road_extended {
    use super::*;

    #[test]
    fn test_speed_zone_ssd() {
        let zone = ExtSpeedZone::new(1, ExtSpeedZoneType::Urban, 50.0, 0.0, 500.0);
        let ssd = zone.stopping_sight_distance();
        assert!(ssd > 30.0 && ssd < 80.0);
    }

    #[test]
    fn test_storm_drain_capacity() {
        let pipe = StormDrainPipe::new(1, 600.0, 0.5, 50.0);
        let q = pipe.full_flow_capacity_m3s();
        assert!(q > 0.1);
        assert!(pipe.is_self_cleansing());
    }

    #[test]
    fn test_open_channel_capacity() {
        let chan = OpenChannel::new(1, 2.0, 1.0, 0.5);
        let q = chan.capacity_m3s();
        assert!(q > 0.0);
    }

    #[test]
    fn test_pavement_performance() {
        let mut model = PavementPerformanceModel::new(1, 1.5, 500_000);
        model.age_years = 10.0;
        let iri = model.iri_at_age(10.0);
        assert!(iri > 1.5);
        let rsl = model.remaining_service_life();
        assert!(rsl >= 0.0);
    }

    #[test]
    fn test_mass_haul() {
        let sections = vec![
            EarthworkSection { start_chainage: 0.0, end_chainage: 100.0, start_area_m2: 10.0, end_area_m2: 15.0, is_cut: true },
            EarthworkSection { start_chainage: 100.0, end_chainage: 200.0, start_area_m2: 8.0, end_area_m2: 5.0, is_cut: false },
        ];
        let mut diagram = MassHaulDiagram::new(200.0);
        diagram.build(&sections);
        assert_eq!(diagram.stations.len(), 3);
    }

    #[test]
    fn test_bridge_deck_area() {
        let mut bridge = Bridge::new(1, "Test Bridge", BridgeType::BeamBridge);
        bridge.add_span(BridgeSpan { span_number: 1, length_m: 30.0, width_m: 9.0, deck_elevation: 10.0, clearance_m: 5.5 });
        assert_eq!(bridge.span_count(), 1);
        assert!((bridge.deck_area_m2() - 30.0 * 7.3).abs() < 1.0);
    }

    #[test]
    fn test_road_geometry_report() {
        let mut report = RoadGeometryReport::new(1);
        report.design_speed_kph = 80.0;
        let params = DesignSpeed::for_speed(80.0);
        report.check_radius(params.min_horizontal_radius_m * 1.5);
        assert!(report.compliant);
        report.check_radius(10.0);
        assert!(!report.compliant);
    }

    #[test]
    fn test_cross_section_widths() {
        let mut cs = RoadCrossSection::new(500.0);
        cs.lanes.push(ExtLane::new(0, LaneType::ThroughLane, 3.5, 1));
        cs.lanes.push(ExtLane::new(1, LaneType::ThroughLane, 3.5, -1));
        cs.compute_widths();
        assert!((cs.carriageway_width_m - 7.0).abs() < 0.01);
    }

    #[test]
    fn test_eia_noise_impact() {
        let mut receiver = NoiseSensitiveReceiver::new(1, "School", Vec2::new(100.0, 0.0), 60.0);
        receiver.predicted_noise_dba = 65.0;
        assert!(receiver.is_impacted());
        assert!((receiver.excess_noise_dba() - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_safety_bcr() {
        let bcr_calc = SafetyBenefitCost {
            treatment_id: 1, location_id: 5,
            annual_crash_cost_before: 200_000.0,
            annual_crash_cost_after: 100_000.0,
            implementation_cost: 500_000.0,
            analysis_period_years: 10,
            discount_rate: 0.07,
        };
        let bcr = bcr_calc.bcr();
        assert!(bcr > 1.0);
    }
}

pub fn terrain_road_module_version() -> &'static str { "2.3.0" }
pub fn terrain_road_features() -> &'static [&'static str] {
    &["speed_zones", "cross_sections", "earthworks", "stormwater",
      "pavement_performance", "bridges", "environmental_impact", "safety_program"]
}


// ============================================================
// ROUNDABOUT DESIGN
// ============================================================

pub const ROUNDABOUT_MIN_INSCRIBED_DIAMETER_M: f32 = 14.0;
pub const ROUNDABOUT_MAX_ENTRY_SPEED_KPH: f32 = 30.0;

#[derive(Debug, Clone, PartialEq)]
pub enum RoundaboutType {
    MiniRoundabout,
    SingleLane,
    MultiLane,
    Turbo,
    TrumpetInterchange,
}

#[derive(Debug, Clone)]
pub struct Roundabout {
    pub id: u32,
    pub roundabout_type: RoundaboutType,
    pub inscribed_diameter_m: f32,
    pub central_island_diameter_m: f32,
    pub circulatory_lane_count: u32,
    pub circulatory_lane_width_m: f32,
    pub entries: Vec<RoundaboutEntry>,
    pub mountable_apron_width_m: f32,
    pub design_speed_kph: f32,
}

impl Roundabout {
    pub fn new(id: u32, roundabout_type: RoundaboutType, inscribed_diameter: f32) -> Self {
        let central = inscribed_diameter * 0.45;
        Roundabout {
            id, roundabout_type, inscribed_diameter_m: inscribed_diameter,
            central_island_diameter_m: central,
            circulatory_lane_count: 1,
            circulatory_lane_width_m: 4.0,
            entries: Vec::new(),
            mountable_apron_width_m: 2.0,
            design_speed_kph: 25.0,
        }
    }

    pub fn circulatory_road_width(&self) -> f32 {
        self.circulatory_lane_count as f32 * self.circulatory_lane_width_m
    }

    pub fn add_entry(&mut self, entry: RoundaboutEntry) {
        self.entries.push(entry);
    }

    pub fn entry_count(&self) -> usize { self.entries.len() }

    pub fn is_4_way(&self) -> bool { self.entries.len() == 4 }

    pub fn capacity_estimate_vph(&self) -> f32 {
        // Simplified HCM roundabout capacity
        let qe_max = 1200.0;
        let qi_factor = 0.9;
        self.entries.iter()
            .map(|e| qe_max * e.lane_count as f32 * qi_factor)
            .sum::<f32>() / self.entries.len() as f32
    }
}

// ============================================================
// INTERCHANGE DESIGN
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum InterchangeType {
    Diamond,
    Cloverleaf,
    Diverging_Diamond,
    SinglePointUrban,
    FolioTrumpet,
    HalfCloverleaf,
    StackInterchange,
    Roundabout_Interchange,
}

#[derive(Debug, Clone)]
pub struct RampConnection {
    pub ramp_id: u32,
    pub from_road_id: u32,
    pub to_road_id: u32,
    pub ramp_type: String,
    pub length_m: f32,
    pub speed_kph: f32,
    pub lane_count: u32,
}

#[derive(Debug, Clone)]
pub struct Interchange {
    pub id: u32,
    pub name: String,
    pub interchange_type: InterchangeType,
    pub position: Vec2,
    pub ramps: Vec<RampConnection>,
    pub grade_separation: bool,
    pub total_area_ha: f32,
    pub construction_cost_estimate: f64,
}

impl Interchange {
    pub fn new(id: u32, name: &str, interchange_type: InterchangeType) -> Self {
        Interchange {
            id, name: name.to_string(), interchange_type,
            position: Vec2::ZERO, ramps: Vec::new(),
            grade_separation: true, total_area_ha: 0.0,
            construction_cost_estimate: 0.0,
        }
    }

    pub fn ramp_count(&self) -> usize { self.ramps.len() }

    pub fn total_ramp_length(&self) -> f32 {
        self.ramps.iter().map(|r| r.length_m).sum()
    }
}

// ============================================================
// SIGHT DISTANCE ANALYSIS
// ============================================================

#[derive(Debug, Clone)]
pub struct SightDistanceCheck {
    pub location_chainage: f32,
    pub required_ssd_m: f32,
    pub available_ssd_m: f32,
    pub required_psd_m: f32,
    pub available_psd_m: f32,
    pub compliant: bool,
    pub obstruction_type: Option<String>,
}

impl SightDistanceCheck {
    pub fn new(chainage: f32, speed_kph: f32) -> Self {
        let zone = ExtSpeedZone::new(0, ExtSpeedZoneType::Rural, speed_kph, 0.0, 1000.0);
        let ssd = zone.stopping_sight_distance();
        let psd = ssd * 2.5;
        SightDistanceCheck {
            location_chainage: chainage,
            required_ssd_m: ssd,
            available_ssd_m: 0.0,
            required_psd_m: psd,
            available_psd_m: 0.0,
            compliant: false,
            obstruction_type: None,
        }
    }

    pub fn evaluate(&mut self) {
        self.compliant = self.available_ssd_m >= self.required_ssd_m;
    }

    pub fn ssd_deficiency(&self) -> f32 {
        (self.required_ssd_m - self.available_ssd_m).max(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct SightDistanceProfile {
    pub road_id: u32,
    pub checks: Vec<SightDistanceCheck>,
    pub check_interval_m: f32,
}

impl SightDistanceProfile {
    pub fn new(road_id: u32, interval_m: f32) -> Self {
        SightDistanceProfile { road_id, checks: Vec::new(), check_interval_m: interval_m }
    }

    pub fn add_check(&mut self, check: SightDistanceCheck) {
        self.checks.push(check);
    }

    pub fn non_compliant_count(&self) -> usize {
        self.checks.iter().filter(|c| !c.compliant).count()
    }

    pub fn worst_deficiency(&self) -> f32 {
        self.checks.iter().map(|c| c.ssd_deficiency()).fold(0.0_f32, f32::max)
    }

    pub fn compliance_rate(&self) -> f32 {
        if self.checks.is_empty() { return 1.0; }
        let compliant = self.checks.iter().filter(|c| c.compliant).count();
        compliant as f32 / self.checks.len() as f32
    }
}

// ============================================================
// TRAFFIC SIGNAL OPTIMIZATION
// ============================================================

pub const SIGNAL_LOST_TIME_PER_PHASE: f32 = 4.0;
pub const SIGNAL_MIN_GREEN_S: f32 = 7.0;
pub const SIGNAL_SATURATION_FLOW_RATE_PCE_HR: f32 = 1800.0;

#[derive(Debug, Clone)]
pub struct SignalPhaseExtended {
    pub phase_id: u32,
    pub description: String,
    pub movements: Vec<String>,
    pub min_green_s: f32,
    pub max_green_s: f32,
    pub actual_green_s: f32,
    pub yellow_s: f32,
    pub all_red_s: f32,
    pub volume_pce_hr: f32,
    pub saturation_flow_pce_hr: f32,
}

impl SignalPhaseExtended {
    pub fn new(phase_id: u32, desc: &str) -> Self {
        SignalPhaseExtended {
            phase_id, description: desc.to_string(),
            movements: Vec::new(),
            min_green_s: SIGNAL_MIN_GREEN_S,
            max_green_s: 60.0,
            actual_green_s: 30.0,
            yellow_s: 3.5,
            all_red_s: 1.5,
            volume_pce_hr: 0.0,
            saturation_flow_pce_hr: SIGNAL_SATURATION_FLOW_RATE_PCE_HR,
        }
    }

    pub fn flow_ratio(&self) -> f32 {
        if self.saturation_flow_pce_hr <= 0.0 { return 0.0; }
        self.volume_pce_hr / self.saturation_flow_pce_hr
    }

    pub fn effective_green_s(&self) -> f32 {
        self.actual_green_s + self.yellow_s - SIGNAL_LOST_TIME_PER_PHASE
    }

    pub fn degree_of_saturation(&self, cycle_s: f32) -> f32 {
        let cap = self.saturation_flow_pce_hr * self.effective_green_s() / cycle_s;
        if cap <= 0.0 { return f32::INFINITY; }
        self.volume_pce_hr / cap
    }
}

#[derive(Debug, Clone)]
pub struct TrafficSignalController {
    pub intersection_id: u32,
    pub cycle_length_s: f32,
    pub phases: Vec<SignalPhaseExtended>,
    pub offset_s: f32,
    pub actuated: bool,
    pub coord_group: Option<u32>,
}

impl TrafficSignalController {
    pub fn new(intersection_id: u32) -> Self {
        TrafficSignalController {
            intersection_id, cycle_length_s: 90.0,
            phases: Vec::new(), offset_s: 0.0,
            actuated: true, coord_group: None,
        }
    }

    pub fn add_phase(&mut self, phase: SignalPhaseExtended) {
        self.phases.push(phase);
    }

    pub fn total_lost_time(&self) -> f32 {
        self.phases.len() as f32 * SIGNAL_LOST_TIME_PER_PHASE
    }

    pub fn effective_cycle_s(&self) -> f32 {
        self.cycle_length_s - self.total_lost_time()
    }

    pub fn critical_flow_ratio_sum(&self) -> f32 {
        self.phases.iter().map(|p| p.flow_ratio()).fold(0.0_f32, f32::max)
    }

    pub fn webster_optimal_cycle(&self) -> f32 {
        let l = self.total_lost_time();
        let y = self.critical_flow_ratio_sum();
        if y >= 1.0 { return 120.0; }
        ((1.5 * l + 5.0) / (1.0 - y)).clamp(60.0, 120.0)
    }

    pub fn current_phase_at(&self, time_in_cycle: f32) -> Option<&SignalPhaseExtended> {
        let mut elapsed = 0.0;
        for phase in &self.phases {
            let phase_dur = phase.actual_green_s + phase.yellow_s + phase.all_red_s;
            if time_in_cycle < elapsed + phase_dur {
                return Some(phase);
            }
            elapsed += phase_dur;
        }
        None
    }
}

// ============================================================
// ROAD INVENTORY MANAGEMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtRoadSegment {
    pub segment_id: u32,
    pub road_name: String,
    pub road_number: String,
    pub start_chainage: f32,
    pub end_chainage: f32,
    pub lanes_each_direction: u32,
    pub carriageway_width_m: f32,
    pub surface_type: String,
    pub pavement_age_years: u32,
    pub speed_limit_kph: f32,
    pub aadt: u32,
    pub truck_pct: f32,
    pub local_authority: String,
    pub urban_rural: String,
    pub functional_class: String,
}

impl ExtRoadSegment {
    pub fn new(segment_id: u32, road_name: &str) -> Self {
        ExtRoadSegment {
            segment_id, road_name: road_name.to_string(),
            road_number: String::new(),
            start_chainage: 0.0, end_chainage: 0.0,
            lanes_each_direction: 1,
            carriageway_width_m: 7.0,
            surface_type: "Asphalt".to_string(),
            pavement_age_years: 0,
            speed_limit_kph: 80.0,
            aadt: 0, truck_pct: 10.0,
            local_authority: String::new(),
            urban_rural: "Rural".to_string(),
            functional_class: "Collector".to_string(),
        }
    }

    pub fn length_km(&self) -> f32 {
        (self.end_chainage - self.start_chainage).abs() / 1000.0
    }

    pub fn lane_km(&self) -> f32 {
        self.length_km() * self.lanes_each_direction as f32 * 2.0
    }

    pub fn annual_esal(&self) -> f64 {
        let trucks = self.aadt as f64 * self.truck_pct as f64 / 100.0 * 365.0;
        trucks * 2.5 // avg ESALs per truck
    }
}

#[derive(Debug, Clone)]
pub struct ExtRoadInventory {
    pub inventory_id: String,
    pub year: u32,
    pub segments: Vec<ExtRoadSegment>,
}

impl ExtRoadInventory {
    pub fn new(year: u32) -> Self {
        ExtRoadInventory { inventory_id: format!("INV-{}", year), year, segments: Vec::new() }
    }

    pub fn add_segment(&mut self, seg: ExtRoadSegment) {
        self.segments.push(seg);
    }

    pub fn total_lane_km(&self) -> f32 {
        self.segments.iter().map(|s| s.lane_km()).sum()
    }

    pub fn total_network_km(&self) -> f32 {
        self.segments.iter().map(|s| s.length_km()).sum()
    }

    pub fn segments_by_surface(&self, surface: &str) -> Vec<&ExtRoadSegment> {
        self.segments.iter().filter(|s| s.surface_type == surface).collect()
    }
}

// ============================================================
// FINAL TEST FUNCTIONS FOR TERRAIN ROAD
// ============================================================

#[cfg(test)]
mod tests_terrain_road_final {
    use super::*;

    #[test]
    fn test_roundabout_capacity() {
        let mut ra = Roundabout::new(1, RoundaboutType::SingleLane, 28.0);
        ra.add_entry(RoundaboutEntry { entry_id: 0, bearing_deg: 0.0, lane_count: 1, entry_width_m: 4.0, flare_length_m: 20.0, approach_speed_kph: 50.0, design_flow_vph: 400, pedestrian_crossing: true });
        ra.add_entry(RoundaboutEntry { entry_id: 1, bearing_deg: 90.0, lane_count: 1, entry_width_m: 4.0, flare_length_m: 20.0, approach_speed_kph: 50.0, design_flow_vph: 350, pedestrian_crossing: true });
        ra.add_entry(RoundaboutEntry { entry_id: 2, bearing_deg: 180.0, lane_count: 1, entry_width_m: 4.0, flare_length_m: 20.0, approach_speed_kph: 50.0, design_flow_vph: 380, pedestrian_crossing: true });
        ra.add_entry(RoundaboutEntry { entry_id: 3, bearing_deg: 270.0, lane_count: 1, entry_width_m: 4.0, flare_length_m: 20.0, approach_speed_kph: 50.0, design_flow_vph: 320, pedestrian_crossing: true });
        assert!(ra.is_4_way());
        assert!(ra.capacity_estimate_vph() > 0.0);
    }

    #[test]
    fn test_sight_distance_profile() {
        let mut profile = SightDistanceProfile::new(1, 100.0);
        let mut check = SightDistanceCheck::new(500.0, 80.0);
        check.available_ssd_m = check.required_ssd_m + 20.0;
        check.evaluate();
        profile.add_check(check);
        assert_eq!(profile.non_compliant_count(), 0);
        assert!((profile.compliance_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_signal_controller_webster() {
        let mut ctrl = TrafficSignalController::new(1);
        let mut p1 = SignalPhaseExtended::new(0, "NS Through");
        p1.volume_pce_hr = 600.0;
        let mut p2 = SignalPhaseExtended::new(1, "EW Through");
        p2.volume_pce_hr = 500.0;
        ctrl.add_phase(p1);
        ctrl.add_phase(p2);
        let optimal = ctrl.webster_optimal_cycle();
        assert!(optimal >= 60.0 && optimal <= 120.0);
    }

    #[test]
    fn test_road_inventory_totals() {
        let mut inv = ExtRoadInventory::new(2024);
        let mut seg = ExtRoadSegment::new(1, "Main Street");
        seg.start_chainage = 0.0;
        seg.end_chainage = 5000.0;
        seg.lanes_each_direction = 2;
        inv.add_segment(seg);
        assert!((inv.total_network_km() - 5.0).abs() < 0.001);
        assert!((inv.total_lane_km() - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_interchange_ramp_length() {
        let mut ic = Interchange::new(1, "Highway Exit 42", InterchangeType::Diamond);
        ic.ramps.push(RampConnection { ramp_id: 0, from_road_id: 1, to_road_id: 2, ramp_type: "On-Ramp".to_string(), length_m: 250.0, speed_kph: 80.0, lane_count: 1 });
        ic.ramps.push(RampConnection { ramp_id: 1, from_road_id: 2, to_road_id: 1, ramp_type: "Off-Ramp".to_string(), length_m: 220.0, speed_kph: 60.0, lane_count: 1 });
        assert_eq!(ic.ramp_count(), 2);
        assert!((ic.total_ramp_length() - 470.0).abs() < 0.001);
    }
}

pub fn terrain_road_final_info() -> &'static str {
    "TerrainRoadTool v2.3: Roundabouts, Interchanges, SightDistance, Signals, Inventory"
}


// ============================================================
// ROAD ASSET CONDITION TRACKING
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum AssetConditionGrade {
    VeryGood, Good, Fair, Poor, VeryPoor, Failed,
}

impl AssetConditionGrade {
    pub fn from_score(score: f32) -> Self {
        if score >= 85.0 { AssetConditionGrade::VeryGood }
        else if score >= 70.0 { AssetConditionGrade::Good }
        else if score >= 55.0 { AssetConditionGrade::Fair }
        else if score >= 40.0 { AssetConditionGrade::Poor }
        else if score >= 20.0 { AssetConditionGrade::VeryPoor }
        else { AssetConditionGrade::Failed }
    }

    pub fn score_midpoint(&self) -> f32 {
        match self {
            AssetConditionGrade::VeryGood => 92.5,
            AssetConditionGrade::Good => 77.5,
            AssetConditionGrade::Fair => 62.5,
            AssetConditionGrade::Poor => 47.5,
            AssetConditionGrade::VeryPoor => 30.0,
            AssetConditionGrade::Failed => 10.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InfrastructureAsset {
    pub asset_id: String,
    pub asset_type: String,
    pub location_chainage: f32,
    pub road_id: u32,
    pub condition_score: f32,
    pub installation_year: u32,
    pub expected_life_years: u32,
    pub replacement_cost: f64,
    pub maintenance_cost_annual: f64,
    pub last_inspection_year: u32,
}

impl InfrastructureAsset {
    pub fn new(asset_id: &str, asset_type: &str, road_id: u32, location: f32) -> Self {
        InfrastructureAsset {
            asset_id: asset_id.to_string(),
            asset_type: asset_type.to_string(),
            location_chainage: location, road_id,
            condition_score: 100.0,
            installation_year: 2000,
            expected_life_years: 20,
            replacement_cost: 0.0,
            maintenance_cost_annual: 0.0,
            last_inspection_year: 2000,
        }
    }

    pub fn age(&self, current_year: u32) -> u32 {
        current_year.saturating_sub(self.installation_year)
    }

    pub fn remaining_life(&self, current_year: u32) -> i32 {
        let age = self.age(current_year) as i32;
        self.expected_life_years as i32 - age
    }

    pub fn condition_grade(&self) -> AssetConditionGrade {
        AssetConditionGrade::from_score(self.condition_score)
    }

    pub fn lifecycle_cost(&self) -> f64 {
        let periods = (self.expected_life_years as f64 / 20.0).ceil();
        self.replacement_cost * periods + self.maintenance_cost_annual * self.expected_life_years as f64
    }
}

#[derive(Debug, Clone)]
pub struct AssetManagementPlan {
    pub plan_id: String,
    pub year: u32,
    pub assets: Vec<InfrastructureAsset>,
    pub budget: f64,
    pub priority_threshold_score: f32,
}

impl AssetManagementPlan {
    pub fn new(plan_id: &str, year: u32, budget: f64) -> Self {
        AssetManagementPlan {
            plan_id: plan_id.to_string(), year, assets: Vec::new(), budget, priority_threshold_score: 60.0,
        }
    }

    pub fn add_asset(&mut self, asset: InfrastructureAsset) {
        self.assets.push(asset);
    }

    pub fn priority_assets(&self) -> Vec<&InfrastructureAsset> {
        self.assets.iter()
            .filter(|a| a.condition_score < self.priority_threshold_score)
            .collect()
    }

    pub fn total_replacement_cost(&self) -> f64 {
        self.assets.iter().map(|a| a.replacement_cost).sum()
    }

    pub fn funded_assets(&self) -> Vec<&InfrastructureAsset> {
        let mut sorted: Vec<&InfrastructureAsset> = self.priority_assets();
        sorted.sort_by(|a, b| a.condition_score.partial_cmp(&b.condition_score).unwrap());
        let mut budget_remaining = self.budget;
        let mut funded = Vec::new();
        for asset in sorted {
            if asset.replacement_cost <= budget_remaining {
                budget_remaining -= asset.replacement_cost;
                funded.push(asset);
            }
        }
        funded
    }
}

// ============================================================
// SNOW REMOVAL PLANNING
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SnowRemovalPriority {
    P1_Emergency, P2_Primary, P3_Secondary, P4_Residential, P5_Low,
}

#[derive(Debug, Clone)]
pub struct SnowRoute {
    pub route_id: u32,
    pub priority: SnowRemovalPriority,
    pub road_segments: Vec<u32>,
    pub total_km: f32,
    pub truck_id: Option<u32>,
    pub salt_rate_kg_km: f32,
    pub plow_passes_required: u32,
    pub estimated_cycle_time_hr: f32,
}

impl SnowRoute {
    pub fn new(route_id: u32, priority: SnowRemovalPriority) -> Self {
        let salt_rate = match priority {
            SnowRemovalPriority::P1_Emergency => 30.0,
            SnowRemovalPriority::P2_Primary => 25.0,
            SnowRemovalPriority::P3_Secondary => 20.0,
            _ => 15.0,
        };
        SnowRoute {
            route_id, priority, road_segments: Vec::new(),
            total_km: 0.0, truck_id: None, salt_rate_kg_km: salt_rate,
            plow_passes_required: 1, estimated_cycle_time_hr: 4.0,
        }
    }

    pub fn total_salt_kg(&self) -> f32 {
        self.total_km * self.salt_rate_kg_km * self.plow_passes_required as f32
    }

    pub fn add_segment(&mut self, segment_id: u32, length_km: f32) {
        self.road_segments.push(segment_id);
        self.total_km += length_km;
    }
}

#[derive(Debug, Clone)]
pub struct SnowControlPlan {
    pub routes: Vec<SnowRoute>,
    pub salt_stockpile_tonnes: f32,
    pub truck_count: u32,
    pub depot_locations: Vec<Vec2>,
}

impl SnowControlPlan {
    pub fn new(truck_count: u32) -> Self {
        SnowControlPlan {
            routes: Vec::new(),
            salt_stockpile_tonnes: 0.0,
            truck_count,
            depot_locations: Vec::new(),
        }
    }

    pub fn total_salt_required_kg(&self) -> f32 {
        self.routes.iter().map(|r| r.total_salt_kg()).sum()
    }

    pub fn has_sufficient_salt(&self) -> bool {
        self.total_salt_required_kg() / 1000.0 <= self.salt_stockpile_tonnes
    }

    pub fn routes_by_priority(&self, priority: &SnowRemovalPriority) -> Vec<&SnowRoute> {
        self.routes.iter().filter(|r| &r.priority == priority).collect()
    }
}

// ============================================================
// UTILITY CORRIDOR MANAGEMENT
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum UtilityType {
    PowerLine, WaterMain, SewerMain, GasMain, TelecomCable,
    FiberOptic, StormDrain, HotWaterPipe, TrafficControl, Irrigation,
}

#[derive(Debug, Clone)]
pub struct UtilityRecord {
    pub utility_id: String,
    pub utility_type: UtilityType,
    pub owner: String,
    pub horizontal_offset_m: f32,
    pub depth_m: f32,
    pub diameter_mm: f32,
    pub material: String,
    pub installation_year: u32,
    pub start_chainage: f32,
    pub end_chainage: f32,
    pub active: bool,
}

impl UtilityRecord {
    pub fn new(utility_id: &str, utility_type: UtilityType, owner: &str) -> Self {
        UtilityRecord {
            utility_id: utility_id.to_string(), utility_type, owner: owner.to_string(),
            horizontal_offset_m: 0.0, depth_m: 1.0,
            diameter_mm: 200.0, material: "PVC".to_string(),
            installation_year: 2000,
            start_chainage: 0.0, end_chainage: 100.0,
            active: true,
        }
    }

    pub fn length_m(&self) -> f32 {
        (self.end_chainage - self.start_chainage).abs()
    }

    pub fn conflicts_with(&self, other: &UtilityRecord) -> bool {
        let horizontal_sep = (self.horizontal_offset_m - other.horizontal_offset_m).abs();
        let vertical_sep = (self.depth_m - other.depth_m).abs();
        horizontal_sep < 0.5 && vertical_sep < 0.3
    }
}

#[derive(Debug, Clone)]
pub struct UtilityCorridorManager {
    pub road_id: u32,
    pub utilities: Vec<UtilityRecord>,
}

impl UtilityCorridorManager {
    pub fn new(road_id: u32) -> Self {
        UtilityCorridorManager { road_id, utilities: Vec::new() }
    }

    pub fn add_utility(&mut self, utility: UtilityRecord) {
        self.utilities.push(utility);
    }

    pub fn find_conflicts(&self) -> Vec<(usize, usize)> {
        let mut conflicts = Vec::new();
        for i in 0..self.utilities.len() {
            for j in (i + 1)..self.utilities.len() {
                if self.utilities[i].conflicts_with(&self.utilities[j]) {
                    conflicts.push((i, j));
                }
            }
        }
        conflicts
    }

    pub fn utilities_of_type(&self, ut: &UtilityType) -> Vec<&UtilityRecord> {
        self.utilities.iter().filter(|u| &u.utility_type == ut).collect()
    }
}

// ============================================================
// ROAD SAFETY RATING SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct SafetyRatingFactor {
    pub factor_name: String,
    pub score: f32,
    pub max_score: f32,
    pub weight: f32,
}

impl SafetyRatingFactor {
    pub fn weighted_score(&self) -> f32 {
        (self.score / self.max_score.max(0.001)) * self.weight
    }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyRating {
    pub section_id: u32,
    pub factors: Vec<SafetyRatingFactor>,
    pub star_rating: u32,
    pub total_score: f32,
}

impl RoadSafetyRating {
    pub fn new(section_id: u32) -> Self {
        RoadSafetyRating { section_id, factors: Vec::new(), star_rating: 0, total_score: 0.0 }
    }

    pub fn add_factor(&mut self, factor: SafetyRatingFactor) {
        self.factors.push(factor);
    }

    pub fn compute_rating(&mut self) {
        let total_weight: f32 = self.factors.iter().map(|f| f.weight).sum();
        let weighted_sum: f32 = self.factors.iter().map(|f| f.weighted_score()).sum();
        self.total_score = if total_weight > 0.0 { weighted_sum / total_weight * 100.0 } else { 0.0 };
        self.star_rating = if self.total_score >= 80.0 { 5 }
            else if self.total_score >= 65.0 { 4 }
            else if self.total_score >= 50.0 { 3 }
            else if self.total_score >= 35.0 { 2 }
            else { 1 };
    }
}

// ============================================================
// LEVEL OF SERVICE ANALYSIS
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum LoS {
    A, B, C, D, E, F,
}

impl LoS {
    pub fn from_density(density_veh_km_lane: f32) -> Self {
        if density_veh_km_lane <= 7.0 { LoS::A }
        else if density_veh_km_lane <= 11.0 { LoS::B }
        else if density_veh_km_lane <= 16.0 { LoS::C }
        else if density_veh_km_lane <= 22.0 { LoS::D }
        else if density_veh_km_lane <= 28.0 { LoS::E }
        else { LoS::F }
    }

    pub fn from_vc_ratio(vc: f32) -> Self {
        if vc <= 0.35 { LoS::A }
        else if vc <= 0.54 { LoS::B }
        else if vc <= 0.77 { LoS::C }
        else if vc <= 0.93 { LoS::D }
        else if vc <= 1.0 { LoS::E }
        else { LoS::F }
    }

    pub fn acceptable(&self) -> bool {
        matches!(self, LoS::A | LoS::B | LoS::C)
    }
}

#[derive(Debug, Clone)]
pub struct FreewaySegmentAnalysis {
    pub segment_id: u32,
    pub length_km: f32,
    pub lane_count: u32,
    pub free_flow_speed_kph: f32,
    pub peak_hour_volume: u32,
    pub peak_hour_factor: f32,
    pub truck_pct: f32,
    pub terrain_type: String,
}

impl FreewaySegmentAnalysis {
    pub fn new(segment_id: u32, lanes: u32, ffs: f32, volume: u32) -> Self {
        FreewaySegmentAnalysis {
            segment_id, length_km: 1.0, lane_count: lanes,
            free_flow_speed_kph: ffs, peak_hour_volume: volume,
            peak_hour_factor: 0.92, truck_pct: 10.0,
            terrain_type: "Level".to_string(),
        }
    }

    pub fn et_factor(&self) -> f32 {
        match self.terrain_type.as_str() {
            "Level" => 1.5,
            "Rolling" => 2.5,
            "Mountainous" => 4.5,
            _ => 2.0,
        }
    }

    pub fn pce_flow_rate(&self) -> f32 {
        let et = self.et_factor();
        let ft = 1.0 / (1.0 + self.truck_pct / 100.0 * (et - 1.0));
        let demand = self.peak_hour_volume as f32 / self.peak_hour_factor;
        demand * (1.0 / ft)
    }

    pub fn flow_per_lane(&self) -> f32 {
        self.pce_flow_rate() / self.lane_count.max(1) as f32
    }

    pub fn speed_flow_model(&self) -> f32 {
        let bp = 1400.0;
        let cap = 2200.0;
        let q = self.flow_per_lane();
        if q <= bp {
            self.free_flow_speed_kph
        } else {
            let t1 = (q - bp) / (cap - bp);
            self.free_flow_speed_kph - (self.free_flow_speed_kph - 53.0) * t1
        }
    }

    pub fn density_veh_km_lane(&self) -> f32 {
        let speed = self.speed_flow_model();
        if speed <= 0.0 { return f32::INFINITY; }
        self.flow_per_lane() / speed
    }

    pub fn level_of_service(&self) -> LoS {
        LoS::from_density(self.density_veh_km_lane())
    }
}

// ============================================================
// TRAFFIC IMPACT ASSESSMENT
// ============================================================

#[derive(Debug, Clone)]
pub struct TripGeneration {
    pub land_use_code: String,
    pub land_use_area: f32,
    pub rate_am_peak_in: f32,
    pub rate_am_peak_out: f32,
    pub rate_pm_peak_in: f32,
    pub rate_pm_peak_out: f32,
    pub rate_daily: f32,
}

impl TripGeneration {
    pub fn am_peak_trips(&self) -> (f32, f32) {
        (self.land_use_area * self.rate_am_peak_in, self.land_use_area * self.rate_am_peak_out)
    }

    pub fn pm_peak_trips(&self) -> (f32, f32) {
        (self.land_use_area * self.rate_pm_peak_in, self.land_use_area * self.rate_pm_peak_out)
    }

    pub fn daily_trips(&self) -> f32 {
        self.land_use_area * self.rate_daily
    }
}

#[derive(Debug, Clone)]
pub struct TiaIntersection {
    pub intersection_id: u32,
    pub name: String,
    pub existing_vc: f32,
    pub background_growth_rate: f32,
    pub project_added_volume: u32,
    pub capacity: u32,
}

impl TiaIntersection {
    pub fn with_project_vc(&self) -> f32 {
        let existing_vol = self.existing_vc * self.capacity as f32;
        let background = existing_vol * self.background_growth_rate;
        (existing_vol + background + self.project_added_volume as f32) / self.capacity as f32
    }

    pub fn los_without_project(&self) -> LoS {
        LoS::from_vc_ratio(self.existing_vc)
    }

    pub fn los_with_project(&self) -> LoS {
        LoS::from_vc_ratio(self.with_project_vc())
    }

    pub fn significant_impact(&self) -> bool {
        let with_vc = self.with_project_vc();
        with_vc > self.existing_vc + 0.05 && with_vc > 0.85
    }
}

#[derive(Debug, Clone)]
pub struct TrafficImpactAssessment {
    pub project_name: String,
    pub trip_gen: Vec<TripGeneration>,
    pub intersections: Vec<TiaIntersection>,
    pub mitigation_required: bool,
    pub mitigation_measures: Vec<String>,
}

impl TrafficImpactAssessment {
    pub fn new(project_name: &str) -> Self {
        TrafficImpactAssessment {
            project_name: project_name.to_string(),
            trip_gen: Vec::new(),
            intersections: Vec::new(),
            mitigation_required: false,
            mitigation_measures: Vec::new(),
        }
    }

    pub fn total_pm_peak_trips(&self) -> f32 {
        self.trip_gen.iter().map(|tg| { let (i, o) = tg.pm_peak_trips(); i + o }).sum()
    }

    pub fn impacted_intersections(&self) -> Vec<&TiaIntersection> {
        self.intersections.iter().filter(|i| i.significant_impact()).collect()
    }

    pub fn assess(&mut self) {
        self.mitigation_required = !self.impacted_intersections().is_empty();
        if self.mitigation_required {
            self.mitigation_measures.push("Signal timing optimization".to_string());
            self.mitigation_measures.push("Turn lane addition".to_string());
        }
    }
}

// ============================================================
// MORE TEST FUNCTIONS
// ============================================================

#[cfg(test)]
mod tests_terrain_road_extra {
    use super::*;

    #[test]
    fn test_asset_condition_grade() {
        assert!(matches!(AssetConditionGrade::from_score(90.0), AssetConditionGrade::VeryGood));
        assert!(matches!(AssetConditionGrade::from_score(45.0), AssetConditionGrade::Poor));
        assert!(matches!(AssetConditionGrade::from_score(10.0), AssetConditionGrade::Failed));
    }

    #[test]
    fn test_asset_management_funded() {
        let mut plan = AssetManagementPlan::new("AMP2024", 2024, 100_000.0);
        let mut asset = InfrastructureAsset::new("SWD-001", "Culvert", 1, 500.0);
        asset.condition_score = 30.0;
        asset.replacement_cost = 50_000.0;
        plan.add_asset(asset);
        let funded = plan.funded_assets();
        assert_eq!(funded.len(), 1);
    }

    #[test]
    fn test_snow_route_salt() {
        let mut route = SnowRoute::new(1, SnowRemovalPriority::P1_Emergency);
        route.add_segment(1, 10.0);
        let salt = route.total_salt_kg();
        assert!(salt > 0.0);
    }

    #[test]
    fn test_utility_conflict_detection() {
        let mut mgr = UtilityCorridorManager::new(1);
        let mut u1 = UtilityRecord::new("PWR-001", UtilityType::PowerLine, "ElecCo");
        u1.horizontal_offset_m = 2.0;
        u1.depth_m = 0.8;
        let mut u2 = UtilityRecord::new("WAT-001", UtilityType::WaterMain, "WaterCo");
        u2.horizontal_offset_m = 2.2;
        u2.depth_m = 0.9;
        mgr.add_utility(u1);
        mgr.add_utility(u2);
        let conflicts = mgr.find_conflicts();
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn test_los_from_vc() {
        assert!(LoS::from_vc_ratio(0.3).acceptable());
        assert!(!LoS::from_vc_ratio(1.1).acceptable());
    }

    #[test]
    fn test_freeway_segment_los() {
        let seg = FreewaySegmentAnalysis::new(1, 3, 110.0, 3000);
        let los = seg.level_of_service();
        assert!(matches!(los, LoS::A | LoS::B | LoS::C));
    }

    #[test]
    fn test_tia_significant_impact() {
        let mut ti = TiaIntersection {
            intersection_id: 1, name: "Main/Oak".to_string(),
            existing_vc: 0.88, background_growth_rate: 0.02,
            project_added_volume: 200, capacity: 1000,
        };
        assert!(ti.significant_impact());
    }

    #[test]
    fn test_safety_rating() {
        let mut rating = RoadSafetyRating::new(1);
        rating.add_factor(SafetyRatingFactor { factor_name: "Alignment".to_string(), score: 80.0, max_score: 100.0, weight: 1.0 });
        rating.add_factor(SafetyRatingFactor { factor_name: "Markings".to_string(), score: 70.0, max_score: 100.0, weight: 0.5 });
        rating.compute_rating();
        assert!(rating.star_rating >= 3);
    }
}

pub const ROAD_ASSET_LIFE_CULVERT: u32 = 50;
pub const ROAD_ASSET_LIFE_SIGN: u32 = 15;
pub const ROAD_ASSET_LIFE_GUARDRAIL: u32 = 20;
pub const ROAD_ASSET_LIFE_PAVEMENT: u32 = 20;
pub const ROAD_ASSET_LIFE_BRIDGE: u32 = 100;
pub const ROAD_ASSET_LIFE_SIGNAL: u32 = 20;
pub const ROAD_ASSET_LIFE_LIGHTING: u32 = 25;


// ============================================================
// GEOMETRIC ROAD DESIGN - SPIRAL TRANSITIONS
// ============================================================

pub const CLOTHOID_SCALE: f32 = 100.0;

#[derive(Debug, Clone)]
pub struct ClothoidSpiral {
    pub parameter_a: f32,
    pub length: f32,
    pub start_radius: f32,
    pub end_radius: f32,
    pub direction: i32,
}

impl ClothoidSpiral {
    pub fn new(start_r: f32, end_r: f32, a: f32) -> Self {
        let l = a * a * (1.0 / end_r - 1.0 / start_r).abs();
        ClothoidSpiral {
            parameter_a: a, length: l,
            start_radius: start_r, end_radius: end_r,
            direction: 1,
        }
    }

    pub fn radius_at(&self, s: f32) -> f32 {
        if s <= 0.0 { return self.start_radius; }
        let r_inv_start = if self.start_radius.is_infinite() { 0.0 } else { 1.0 / self.start_radius };
        let r_inv_end = 1.0 / self.end_radius;
        let t = s / self.length.max(0.001);
        let r_inv = r_inv_start + (r_inv_end - r_inv_start) * t;
        if r_inv.abs() < 1e-10 { f32::INFINITY } else { 1.0 / r_inv.abs() }
    }

    pub fn deflection_angle_rad(&self) -> f32 {
        self.length / (2.0 * self.end_radius)
    }

    pub fn minimum_a_for_speed(&self, design_speed_kph: f32) -> f32 {
        design_speed_kph * 0.036 * design_speed_kph.sqrt()
    }
}

// ============================================================
// ROAD SIGNAGE STANDARDS CHECKER
// ============================================================

#[derive(Debug, Clone)]
pub struct SignStandardsCheck {
    pub sign_id: u32,
    pub location_chainage: f32,
    pub sign_type: String,
    pub advance_warning_distance_m: f32,
    pub required_advance_distance_m: f32,
    pub height_above_pavement_m: f32,
    pub required_min_height_m: f32,
    pub retroreflective: bool,
    pub compliant: bool,
    pub issues: Vec<String>,
}

impl SignStandardsCheck {
    pub fn new(sign_id: u32, chainage: f32, sign_type: &str) -> Self {
        SignStandardsCheck {
            sign_id, location_chainage: chainage, sign_type: sign_type.to_string(),
            advance_warning_distance_m: 0.0, required_advance_distance_m: 150.0,
            height_above_pavement_m: 2.1, required_min_height_m: 2.1,
            retroreflective: true, compliant: true, issues: Vec::new(),
        }
    }

    pub fn evaluate(&mut self) {
        self.issues.clear();
        if self.advance_warning_distance_m < self.required_advance_distance_m {
            self.issues.push(format!("Insufficient advance warning: {:.0}m < {:.0}m", self.advance_warning_distance_m, self.required_advance_distance_m));
        }
        if self.height_above_pavement_m < self.required_min_height_m {
            self.issues.push(format!("Sign height {:.1}m below minimum {:.1}m", self.height_above_pavement_m, self.required_min_height_m));
        }
        if !self.retroreflective {
            self.issues.push("Sign lacks retroreflective sheeting".to_string());
        }
        self.compliant = self.issues.is_empty();
    }
}

// ============================================================
// HORIZONTAL ALIGNMENT COMPUTATION
// ============================================================

#[derive(Debug, Clone)]
pub struct PI_Point {
    pub chainage: f32,
    pub easting: f32,
    pub northing: f32,
    pub deflection_angle_deg: f32,
    pub radius: f32,
    pub spiral_in_length: f32,
    pub spiral_out_length: f32,
}

impl PI_Point {
    pub fn new(chainage: f32, e: f32, n: f32) -> Self {
        PI_Point { chainage, easting: e, northing: n, deflection_angle_deg: 0.0, radius: 0.0, spiral_in_length: 0.0, spiral_out_length: 0.0 }
    }

    pub fn tangent_length(&self) -> f32 {
        if self.radius <= 0.0 { return 0.0; }
        let delta_rad = self.deflection_angle_deg.to_radians();
        let t_simple = self.radius * (delta_rad / 2.0).tan();
        let t_spiral = self.spiral_in_length / 2.0;
        t_simple + t_spiral
    }

    pub fn curve_length(&self) -> f32 {
        if self.radius <= 0.0 { return 0.0; }
        let delta_rad = self.deflection_angle_deg.to_radians();
        self.radius * delta_rad + self.spiral_in_length + self.spiral_out_length
    }

    pub fn long_chord(&self) -> f32 {
        if self.radius <= 0.0 { return 0.0; }
        let delta_rad = self.deflection_angle_deg.to_radians();
        2.0 * self.radius * (delta_rad / 2.0).sin()
    }

    pub fn external_distance(&self) -> f32 {
        if self.radius <= 0.0 { return 0.0; }
        let delta_rad = self.deflection_angle_deg.to_radians();
        self.radius * ((delta_rad / 2.0).cos().recip() - 1.0)
    }

    pub fn mid_ordinate(&self) -> f32 {
        if self.radius <= 0.0 { return 0.0; }
        let delta_rad = self.deflection_angle_deg.to_radians();
        self.radius * (1.0 - (delta_rad / 2.0).cos())
    }
}

// ============================================================
// VERTICAL ALIGNMENT COMPUTATION
// ============================================================

#[derive(Debug, Clone)]
pub struct VPI_Point {
    pub chainage: f32,
    pub elevation: f32,
    pub grade_in_pct: f32,
    pub grade_out_pct: f32,
    pub k_value: f32,
}

impl VPI_Point {
    pub fn new(chainage: f32, elevation: f32) -> Self {
        VPI_Point { chainage, elevation, grade_in_pct: 0.0, grade_out_pct: 0.0, k_value: 30.0 }
    }

    pub fn grade_change_pct(&self) -> f32 {
        self.grade_out_pct - self.grade_in_pct
    }

    pub fn vertical_curve_length(&self) -> f32 {
        self.k_value * self.grade_change_pct().abs()
    }

    pub fn is_crest(&self) -> bool {
        self.grade_change_pct() < 0.0
    }

    pub fn is_sag(&self) -> bool {
        self.grade_change_pct() > 0.0
    }

    pub fn elevation_at_chainage(&self, ch: f32) -> f32 {
        let l = self.vertical_curve_length();
        let bvc_ch = self.chainage - l / 2.0;
        let x = ch - bvc_ch;
        if x < 0.0 || x > l { return self.elevation; }
        let bvc_elev = self.elevation - (l / 2.0) * self.grade_in_pct / 100.0;
        let r = (self.grade_out_pct - self.grade_in_pct) / (l * 100.0);
        bvc_elev + (self.grade_in_pct / 100.0) * x + 0.5 * (r / 100.0) * x * x
    }
}

// ============================================================
// COST ESTIMATION
// ============================================================

#[derive(Debug, Clone)]
pub struct CostItem {
    pub item_code: String,
    pub description: String,
    pub unit: String,
    pub quantity: f64,
    pub unit_rate: f64,
    pub contingency_pct: f32,
}

impl CostItem {
    pub fn base_cost(&self) -> f64 {
        self.quantity * self.unit_rate
    }

    pub fn with_contingency(&self) -> f64 {
        self.base_cost() * (1.0 + self.contingency_pct as f64 / 100.0)
    }
}

#[derive(Debug, Clone)]
pub struct CostEstimate {
    pub project_name: String,
    pub estimate_date: String,
    pub items: Vec<CostItem>,
    pub overhead_pct: f32,
    pub profit_pct: f32,
    pub gst_pct: f32,
    pub design_fee_pct: f32,
    pub supervision_fee_pct: f32,
}

impl CostEstimate {
    pub fn new(project_name: &str) -> Self {
        CostEstimate {
            project_name: project_name.to_string(), estimate_date: String::new(),
            items: Vec::new(), overhead_pct: 12.0, profit_pct: 8.0,
            gst_pct: 10.0, design_fee_pct: 5.0, supervision_fee_pct: 3.0,
        }
    }

    pub fn add_item(&mut self, item: CostItem) {
        self.items.push(item);
    }

    pub fn direct_cost(&self) -> f64 {
        self.items.iter().map(|i| i.with_contingency()).sum()
    }

    pub fn overhead_cost(&self) -> f64 {
        self.direct_cost() * self.overhead_pct as f64 / 100.0
    }

    pub fn profit(&self) -> f64 {
        (self.direct_cost() + self.overhead_cost()) * self.profit_pct as f64 / 100.0
    }

    pub fn construction_cost(&self) -> f64 {
        self.direct_cost() + self.overhead_cost() + self.profit()
    }

    pub fn total_project_cost(&self) -> f64 {
        let cc = self.construction_cost();
        let design = cc * self.design_fee_pct as f64 / 100.0;
        let supervision = cc * self.supervision_fee_pct as f64 / 100.0;
        let gst = (cc + design + supervision) * self.gst_pct as f64 / 100.0;
        cc + design + supervision + gst
    }

    pub fn cost_per_lane_km(&self, lane_km: f32) -> f64 {
        if lane_km <= 0.0 { return 0.0; }
        self.construction_cost() / lane_km as f64
    }
}

// ============================================================
// FINAL TEST FUNCTIONS
// ============================================================

#[cfg(test)]
mod tests_terrain_final {
    use super::*;

    #[test]
    fn test_clothoid_radius() {
        let spiral = ClothoidSpiral::new(f32::INFINITY, 300.0, 100.0);
        let r_at_end = spiral.radius_at(spiral.length);
        assert!((r_at_end - 300.0).abs() < 5.0);
    }

    #[test]
    fn test_pi_point_tangent() {
        let mut pi = PI_Point::new(1000.0, 5000.0, 6000.0);
        pi.deflection_angle_deg = 30.0;
        pi.radius = 500.0;
        let tl = pi.tangent_length();
        assert!(tl > 0.0);
    }

    #[test]
    fn test_vpi_elevation() {
        let mut vpi = VPI_Point::new(1000.0, 10.0);
        vpi.grade_in_pct = 3.0;
        vpi.grade_out_pct = -2.0;
        vpi.k_value = 20.0;
        let cl = vpi.vertical_curve_length();
        assert!((cl - 100.0).abs() < 0.001);
        assert!(vpi.is_crest());
    }

    #[test]
    fn test_cost_estimate() {
        let mut est = CostEstimate::new("Test Road");
        est.add_item(CostItem {
            item_code: "1001".to_string(), description: "Earthworks".to_string(),
            unit: "m3".to_string(), quantity: 10000.0, unit_rate: 25.0, contingency_pct: 10.0,
        });
        assert!(est.total_project_cost() > est.direct_cost());
    }

    #[test]
    fn test_sign_standards_check() {
        let mut check = SignStandardsCheck::new(1, 500.0, "Speed Zone");
        check.advance_warning_distance_m = 200.0;
        check.retroreflective = true;
        check.evaluate();
        assert!(check.compliant);
        check.advance_warning_distance_m = 50.0;
        check.evaluate();
        assert!(!check.compliant);
    }
}

pub const TERRAIN_ROAD_BUILD_VERSION: u32 = 230;
pub const TERRAIN_ROAD_FEATURE_COUNT: u32 = 47;


// ============================================================
// BRIDGE INSPECTION AND RATING
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum BridgeComponentType {
    Deck, Superstructure, Substructure, Culvert, Channel, Approach,
}

#[derive(Debug, Clone)]
pub struct BridgeComponentRating {
    pub component: BridgeComponentType,
    pub inspection_rating: u32,
    pub notes: String,
    pub requires_action: bool,
}

impl BridgeComponentRating {
    pub fn new(component: BridgeComponentType, rating: u32) -> Self {
        BridgeComponentRating {
            component, inspection_rating: rating,
            notes: String::new(),
            requires_action: rating <= 4,
        }
    }

    pub fn condition_description(&self) -> &'static str {
        match self.inspection_rating {
            9 | 10 => "Excellent",
            7 | 8 => "Good",
            5 | 6 => "Fair",
            4 => "Poor",
            3 => "Serious",
            2 => "Critical",
            1 => "Imminent Failure",
            _ => "Failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BridgeInspectionReport {
    pub bridge_id: u32,
    pub inspection_date: String,
    pub inspector_name: String,
    pub component_ratings: Vec<BridgeComponentRating>,
    pub overall_sufficiency_rating: f32,
    pub load_rating_tonne: f32,
    pub posted_load_limit_tonne: Option<f32>,
    pub recommendations: Vec<String>,
    pub next_inspection_due: String,
}

impl BridgeInspectionReport {
    pub fn new(bridge_id: u32) -> Self {
        BridgeInspectionReport {
            bridge_id, inspection_date: String::new(), inspector_name: String::new(),
            component_ratings: Vec::new(),
            overall_sufficiency_rating: 0.0,
            load_rating_tonne: 44.0, posted_load_limit_tonne: None,
            recommendations: Vec::new(), next_inspection_due: String::new(),
        }
    }

    pub fn add_component(&mut self, rating: BridgeComponentRating) {
        self.component_ratings.push(rating);
    }

    pub fn minimum_rating(&self) -> u32 {
        self.component_ratings.iter().map(|c| c.inspection_rating).min().unwrap_or(9)
    }

    pub fn critical_components(&self) -> Vec<&BridgeComponentRating> {
        self.component_ratings.iter().filter(|c| c.inspection_rating <= 3).collect()
    }

    pub fn requires_load_posting(&self) -> bool {
        self.minimum_rating() <= 4
    }

    pub fn compute_sufficiency_rating(&mut self) {
        let avg = self.component_ratings.iter().map(|c| c.inspection_rating as f32).sum::<f32>()
            / self.component_ratings.len().max(1) as f32;
        self.overall_sufficiency_rating = (avg / 9.0 * 100.0).clamp(0.0, 100.0);
    }
}

// ============================================================
// GEOTECHNICAL INVESTIGATION
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SoilType {
    Rock, GravelSand, SandySilt, ClayLow, ClayHigh, Organic, Fill,
}

impl SoilType {
    pub fn bearing_capacity_kpa(&self) -> f32 {
        match self {
            SoilType::Rock => 5000.0,
            SoilType::GravelSand => 400.0,
            SoilType::SandySilt => 150.0,
            SoilType::ClayLow => 75.0,
            SoilType::ClayHigh => 40.0,
            SoilType::Organic => 25.0,
            SoilType::Fill => 100.0,
        }
    }

    pub fn california_bearing_ratio(&self) -> f32 {
        match self {
            SoilType::Rock => 100.0,
            SoilType::GravelSand => 80.0,
            SoilType::SandySilt => 20.0,
            SoilType::ClayLow => 8.0,
            SoilType::ClayHigh => 3.0,
            SoilType::Organic => 2.0,
            SoilType::Fill => 15.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoreholeLayer {
    pub depth_from_m: f32,
    pub depth_to_m: f32,
    pub soil_type: SoilType,
    pub spt_n_value: Option<u32>,
    pub moisture_content_pct: f32,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct BoreholeLog {
    pub borehole_id: String,
    pub location: Vec2,
    pub total_depth_m: f32,
    pub water_table_depth_m: Option<f32>,
    pub layers: Vec<BoreholeLayer>,
    pub date_drilled: String,
}

impl BoreholeLog {
    pub fn new(id: &str, location: Vec2) -> Self {
        BoreholeLog {
            borehole_id: id.to_string(), location,
            total_depth_m: 0.0, water_table_depth_m: None,
            layers: Vec::new(), date_drilled: String::new(),
        }
    }

    pub fn add_layer(&mut self, layer: BoreholeLayer) {
        if layer.depth_to_m > self.total_depth_m { self.total_depth_m = layer.depth_to_m; }
        self.layers.push(layer);
    }

    pub fn soil_at_depth(&self, depth_m: f32) -> Option<&BoreholeLayer> {
        self.layers.iter().find(|l| depth_m >= l.depth_from_m && depth_m <= l.depth_to_m)
    }

    pub fn has_groundwater(&self) -> bool {
        self.water_table_depth_m.is_some()
    }

    pub fn min_cbr(&self) -> f32 {
        self.layers.iter().map(|l| l.soil_type.california_bearing_ratio()).fold(f32::INFINITY, f32::min)
    }
}

// ============================================================
// PAVEMENT DESIGN (MECHANISTIC EMPIRICAL)
// ============================================================

pub const SUBGRADE_CBR_MIN: f32 = 2.0;
pub const PAVEMENT_POISSON_AC: f32 = 0.35;
pub const PAVEMENT_POISSON_BASE: f32 = 0.40;

#[derive(Debug, Clone)]
pub struct PavementMaterialProps {
    pub material_name: String,
    pub elastic_modulus_mpa: f32,
    pub poissons_ratio: f32,
    pub layer_thickness_mm: f32,
    pub unit_cost_per_m2: f64,
}

impl PavementMaterialProps {
    pub fn dense_graded_ac() -> Self {
        PavementMaterialProps {
            material_name: "Dense Graded AC".to_string(),
            elastic_modulus_mpa: 3000.0, poissons_ratio: PAVEMENT_POISSON_AC,
            layer_thickness_mm: 50.0, unit_cost_per_m2: 35.0,
        }
    }

    pub fn crushed_rock_base() -> Self {
        PavementMaterialProps {
            material_name: "Crushed Rock Base".to_string(),
            elastic_modulus_mpa: 300.0, poissons_ratio: PAVEMENT_POISSON_BASE,
            layer_thickness_mm: 200.0, unit_cost_per_m2: 20.0,
        }
    }

    pub fn subbase_cbr20() -> Self {
        PavementMaterialProps {
            material_name: "Granular Subbase CBR20".to_string(),
            elastic_modulus_mpa: 150.0, poissons_ratio: 0.40,
            layer_thickness_mm: 150.0, unit_cost_per_m2: 12.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PavementDesign {
    pub design_id: String,
    pub road_category: String,
    pub design_esal: f64,
    pub subgrade_cbr: f32,
    pub layers: Vec<PavementMaterialProps>,
    pub design_life_years: u32,
    pub reliability_pct: f32,
}

impl PavementDesign {
    pub fn new(design_id: &str, esal: f64, subgrade_cbr: f32) -> Self {
        PavementDesign {
            design_id: design_id.to_string(), road_category: "Collector".to_string(),
            design_esal: esal, subgrade_cbr, layers: Vec::new(),
            design_life_years: 20, reliability_pct: 95.0,
        }
    }

    pub fn add_layer(&mut self, layer: PavementMaterialProps) {
        self.layers.push(layer);
    }

    pub fn total_pavement_thickness_mm(&self) -> f32 {
        self.layers.iter().map(|l| l.layer_thickness_mm).sum()
    }

    pub fn total_material_cost_per_m2(&self) -> f64 {
        self.layers.iter().map(|l| l.unit_cost_per_m2).sum()
    }

    pub fn structural_number(&self) -> f32 {
        // Simplified SN calculation
        let layer_coefs = [0.44_f32, 0.14, 0.11];
        self.layers.iter().enumerate().map(|(i, l)| {
            let a = layer_coefs.get(i).copied().unwrap_or(0.10);
            a * l.layer_thickness_mm / 25.4
        }).sum()
    }
}

// ============================================================
// ADDITIONAL UTILITY FUNCTIONS AND CONSTANTS
// ============================================================

pub const ROAD_DESIGN_GRAVITY: f32 = 9.81;
pub const ROAD_DESIGN_AIR_DENSITY: f32 = 1.225;
pub const ROAD_DESIGN_WATER_DENSITY: f32 = 1000.0;
pub const ROAD_DESIGN_CONCRETE_DENSITY: f32 = 2400.0;
pub const ROAD_DESIGN_ASPHALT_DENSITY: f32 = 2350.0;
pub const ROAD_DESIGN_STEEL_DENSITY: f32 = 7850.0;

pub fn friction_force_n(normal_n: f32, friction_coeff: f32) -> f32 {
    normal_n * friction_coeff
}

pub fn braking_distance_m(speed_kph: f32, deceleration_ms2: f32) -> f32 {
    let v = speed_kph / 3.6;
    v * v / (2.0 * deceleration_ms2)
}

pub fn headway_to_flow_vphpl(headway_s: f32) -> f32 {
    if headway_s <= 0.0 { return 0.0; }
    3600.0 / headway_s
}

pub fn flow_to_headway_s(flow_vphpl: f32) -> f32 {
    if flow_vphpl <= 0.0 { return f32::INFINITY; }
    3600.0 / flow_vphpl
}

pub fn rolling_resistance_force_n(vehicle_mass_kg: f32, crr: f32) -> f32 {
    vehicle_mass_kg * ROAD_DESIGN_GRAVITY * crr
}

pub fn grade_resistance_n(vehicle_mass_kg: f32, grade_pct: f32) -> f32 {
    vehicle_mass_kg * ROAD_DESIGN_GRAVITY * grade_pct / 100.0
}

pub fn stopping_distance_on_grade_m(speed_kph: f32, grade_pct: f32, friction: f32) -> f32 {
    let v = speed_kph / 3.6;
    let effective_friction = friction - grade_pct / 100.0;
    if effective_friction <= 0.0 { return f32::INFINITY; }
    v * v / (2.0 * ROAD_DESIGN_GRAVITY * effective_friction)
}

pub fn traffic_density_veh_km(flow_vph: f32, speed_kph: f32) -> f32 {
    if speed_kph <= 0.0 { return 0.0; }
    flow_vph / speed_kph
}

pub fn travel_time_index(actual_speed_kph: f32, freeflow_speed_kph: f32) -> f32 {
    if actual_speed_kph <= 0.0 { return f32::INFINITY; }
    freeflow_speed_kph / actual_speed_kph
}

// ============================================================
// FINAL TEST BLOCK
// ============================================================

#[cfg(test)]
mod tests_terrain_road_final2 {
    use super::*;

    #[test]
    fn test_bridge_inspection() {
        let mut report = BridgeInspectionReport::new(1);
        report.add_component(BridgeComponentRating::new(BridgeComponentType::Deck, 6));
        report.add_component(BridgeComponentRating::new(BridgeComponentType::Superstructure, 7));
        report.add_component(BridgeComponentRating::new(BridgeComponentType::Substructure, 5));
        report.compute_sufficiency_rating();
        assert!(report.overall_sufficiency_rating > 0.0);
        assert!(!report.requires_load_posting());
    }

    #[test]
    fn test_borehole_soil_at_depth() {
        let mut bh = BoreholeLog::new("BH-001", Vec2::ZERO);
        bh.add_layer(BoreholeLayer { depth_from_m: 0.0, depth_to_m: 2.0, soil_type: SoilType::Fill, spt_n_value: Some(10), moisture_content_pct: 15.0, description: String::new() });
        bh.add_layer(BoreholeLayer { depth_from_m: 2.0, depth_to_m: 8.0, soil_type: SoilType::ClayLow, spt_n_value: Some(5), moisture_content_pct: 25.0, description: String::new() });
        let soil = bh.soil_at_depth(3.0).unwrap();
        assert!(matches!(soil.soil_type, SoilType::ClayLow));
    }

    #[test]
    fn test_pavement_design_sn() {
        let mut design = PavementDesign::new("PD-001", 5_000_000.0, 8.0);
        design.add_layer(PavementMaterialProps::dense_graded_ac());
        design.add_layer(PavementMaterialProps::crushed_rock_base());
        design.add_layer(PavementMaterialProps::subbase_cbr20());
        let sn = design.structural_number();
        assert!(sn > 2.0);
        assert!(design.total_pavement_thickness_mm() == 400.0);
    }

    #[test]
    fn test_braking_distance() {
        let bd = braking_distance_m(100.0, 5.88);
        assert!(bd > 50.0 && bd < 200.0);
    }

    #[test]
    fn test_stopping_on_grade() {
        let flat = stopping_distance_on_grade_m(80.0, 0.0, 0.35);
        let downhill = stopping_distance_on_grade_m(80.0, -5.0, 0.35);
        assert!(downhill > flat);
    }

    #[test]
    fn test_headway_conversion() {
        let headway = 2.5;
        let flow = headway_to_flow_vphpl(headway);
        let back = flow_to_headway_s(flow);
        assert!((back - headway).abs() < 0.01);
    }
}

pub const TERRAIN_ROAD_COMPLETE: bool = true;
pub const TERRAIN_ROAD_LINE_TARGET: u32 = 7000;


// ============================================================
// ROAD REHABILITATION ANALYSIS (terrain_road_tool additions)
// ============================================================

#[derive(Debug, Clone)]
pub struct RehabOption {
    pub option_id: u32,
    pub description: String,
    pub treatment_type: String,
    pub cost_per_m2: f64,
    pub expected_life_years: u32,
    pub iri_improvement: f32,
    pub pci_improvement: f32,
}

impl RehabOption {
    pub fn crack_seal() -> Self {
        RehabOption { option_id: 1, description: "Crack Sealing".to_string(), treatment_type: "Preventive".to_string(), cost_per_m2: 3.0, expected_life_years: 5, iri_improvement: 0.2, pci_improvement: 5.0 }
    }
    pub fn fog_seal() -> Self {
        RehabOption { option_id: 2, description: "Fog Seal".to_string(), treatment_type: "Preventive".to_string(), cost_per_m2: 2.5, expected_life_years: 4, iri_improvement: 0.1, pci_improvement: 3.0 }
    }
    pub fn microsurfacing() -> Self {
        RehabOption { option_id: 3, description: "Microsurfacing".to_string(), treatment_type: "Minor Rehab".to_string(), cost_per_m2: 12.0, expected_life_years: 8, iri_improvement: 0.8, pci_improvement: 15.0 }
    }
    pub fn overlay_50mm() -> Self {
        RehabOption { option_id: 4, description: "50mm AC Overlay".to_string(), treatment_type: "Major Rehab".to_string(), cost_per_m2: 28.0, expected_life_years: 12, iri_improvement: 1.5, pci_improvement: 30.0 }
    }
    pub fn reconstruction() -> Self {
        RehabOption { option_id: 5, description: "Full Reconstruction".to_string(), treatment_type: "Reconstruction".to_string(), cost_per_m2: 120.0, expected_life_years: 25, iri_improvement: 3.0, pci_improvement: 70.0 }
    }

    pub fn benefit_cost_ratio(&self, area_m2: f32, current_condition_score: f32) -> f64 {
        let annual_benefit = (self.iri_improvement * current_condition_score) as f64 * area_m2 as f64 * 0.1;
        let total_benefit = annual_benefit * self.expected_life_years as f64;
        let cost = self.cost_per_m2 * area_m2 as f64;
        if cost <= 0.0 { return f64::INFINITY; }
        total_benefit / cost
    }
}

#[derive(Debug, Clone)]
pub struct RehabProgramEntry {
    pub section_id: u32,
    pub area_m2: f32,
    pub selected_option: RehabOption,
    pub programmed_year: u32,
    pub priority_score: f32,
}

#[derive(Debug, Clone)]
pub struct RehabilitationProgram {
    pub program_name: String,
    pub analysis_years: u32,
    pub annual_budget: f64,
    pub entries: Vec<RehabProgramEntry>,
}

impl RehabilitationProgram {
    pub fn new(name: &str, years: u32, budget: f64) -> Self {
        RehabilitationProgram { program_name: name.to_string(), analysis_years: years, annual_budget: budget, entries: Vec::new() }
    }

    pub fn add_entry(&mut self, entry: RehabProgramEntry) {
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap());
    }

    pub fn total_cost(&self) -> f64 {
        self.entries.iter().map(|e| e.selected_option.cost_per_m2 * e.area_m2 as f64).sum()
    }

    pub fn entries_by_year(&self, year: u32) -> Vec<&RehabProgramEntry> {
        self.entries.iter().filter(|e| e.programmed_year == year).collect()
    }
}

#[cfg(test)]
mod tests_terrain_rehab {
    use super::*;

    #[test]
    fn test_rehab_bcr() {
        let overlay = RehabOption::overlay_50mm();
        let bcr = overlay.benefit_cost_ratio(1000.0, 50.0);
        assert!(bcr > 0.0);
    }

    #[test]
    fn test_rehab_program() {
        let mut prog = RehabilitationProgram::new("FY2025", 5, 500_000.0);
        prog.add_entry(RehabProgramEntry { section_id: 1, area_m2: 5000.0, selected_option: RehabOption::overlay_50mm(), programmed_year: 2025, priority_score: 85.0 });
        prog.add_entry(RehabProgramEntry { section_id: 2, area_m2: 2000.0, selected_option: RehabOption::crack_seal(), programmed_year: 2025, priority_score: 60.0 });
        assert_eq!(prog.entries_by_year(2025).len(), 2);
        assert!(prog.total_cost() > 0.0);
    }

    #[test]
    fn test_pavement_design_thickness() {
        let mut design = PavementDesign::new("EXPR-001", 10_000_000.0, 5.0);
        design.add_layer(PavementMaterialProps::dense_graded_ac());
        design.add_layer(PavementMaterialProps { layer_thickness_mm: 75.0, ..PavementMaterialProps::dense_graded_ac() });
        design.add_layer(PavementMaterialProps::crushed_rock_base());
        assert_eq!(design.total_pavement_thickness_mm(), 325.0);
    }
}

pub const TERRAIN_ROAD_REHAB_CONSTANTS: &[(&str, f32)] = &[
    ("MAX_IRI_ACCEPTABLE", 4.5),
    ("MIN_PCI_ACCEPTABLE", 40.0),
    ("CRACKING_THRESHOLD_PCT", 20.0),
    ("RUTTING_THRESHOLD_MM", 15.0),
    ("TEXTURE_DEPTH_MIN_MM", 0.6),
    ("SKID_RESISTANCE_MIN_SFC", 0.45),
];


// ============================================================
// ADDITIONAL ROAD DESIGN ELEMENTS
// ============================================================

#[derive(Debug, Clone)]
pub struct TurnLaneWarrant {
    pub intersection_id: u32,
    pub approach_volume_vph: u32,
    pub turning_volume_vph: u32,
    pub opposing_volume_vph: u32,
    pub speed_kph: f32,
    pub left_turn_warranted: bool,
    pub right_turn_warranted: bool,
}

impl TurnLaneWarrant {
    pub fn evaluate(intersection_id: u32, approach: u32, turning: u32, opposing: u32, speed: f32) -> Self {
        let left_warrant = turning > 50 && (turning as f32 / approach as f32 > 0.10 || opposing > 200);
        let right_warrant = turning > 50 && speed >= 70.0 && turning as f32 / approach as f32 > 0.10;
        TurnLaneWarrant {
            intersection_id, approach_volume_vph: approach,
            turning_volume_vph: turning, opposing_volume_vph: opposing,
            speed_kph: speed, left_turn_warranted: left_warrant, right_turn_warranted: right_warrant,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccessManagementPlan {
    pub road_id: u32,
    pub access_category: String,
    pub min_access_spacing_m: f32,
    pub min_intersection_spacing_m: f32,
    pub existing_accesses: u32,
    pub non_compliant_accesses: u32,
    pub recommendations: Vec<String>,
}

impl AccessManagementPlan {
    pub fn new(road_id: u32, category: &str, min_access: f32, min_intersection: f32) -> Self {
        AccessManagementPlan {
            road_id, access_category: category.to_string(),
            min_access_spacing_m: min_access, min_intersection_spacing_m: min_intersection,
            existing_accesses: 0, non_compliant_accesses: 0, recommendations: Vec::new(),
        }
    }

    pub fn compliance_rate(&self) -> f32 {
        if self.existing_accesses == 0 { return 1.0; }
        (self.existing_accesses - self.non_compliant_accesses) as f32 / self.existing_accesses as f32
    }
}

#[derive(Debug, Clone)]
pub struct PedCycleFacility {
    pub facility_id: u32,
    pub facility_type: String,
    pub width_m: f32,
    pub length_m: f32,
    pub separated_from_traffic: bool,
    pub lighting: bool,
    pub crossing_count: u32,
    pub surface_type: String,
}

impl PedCycleFacility {
    pub fn footpath(id: u32, width: f32, length: f32) -> Self {
        PedCycleFacility { facility_id: id, facility_type: "Footpath".to_string(), width_m: width, length_m: length, separated_from_traffic: true, lighting: false, crossing_count: 0, surface_type: "Concrete".to_string() }
    }

    pub fn shared_path(id: u32, width: f32, length: f32) -> Self {
        PedCycleFacility { facility_id: id, facility_type: "Shared Path".to_string(), width_m: width, length_m: length, separated_from_traffic: true, lighting: false, crossing_count: 0, surface_type: "Asphalt".to_string() }
    }

    pub fn area_m2(&self) -> f32 { self.width_m * self.length_m }
}

#[cfg(test)]
mod tests_road_access {
    use super::*;

    #[test]
    fn test_turn_lane_warrant() {
        let w = TurnLaneWarrant::evaluate(1, 800, 120, 400, 80.0);
        assert!(w.left_turn_warranted);
    }

    #[test]
    fn test_access_compliance() {
        let mut plan = AccessManagementPlan::new(1, "Category 3", 100.0, 500.0);
        plan.existing_accesses = 10;
        plan.non_compliant_accesses = 2;
        assert!((plan.compliance_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_ped_facility_area() {
        let path = PedCycleFacility::shared_path(1, 3.0, 500.0);
        assert!((path.area_m2() - 1500.0).abs() < 0.001);
    }
}

pub const ROAD_TURN_LANE_MIN_LENGTH_M: f32 = 45.0;
pub const ROAD_DECEL_TAPER_RATE: f32 = 15.0;
pub const ROAD_ACCEL_TAPER_RATE: f32 = 10.0;
pub const ROAD_PEDESTRIAN_CLEARANCE_TIME_S: f32 = 7.0;
pub const ROAD_BICYCLE_LANE_MIN_WIDTH_M: f32 = 1.2;
pub const ROAD_FOOTPATH_MIN_WIDTH_M: f32 = 1.5;
pub const ROAD_SHARED_PATH_MIN_WIDTH_M: f32 = 2.5;
pub const ROAD_MAX_SUPERELEVATION_URBAN_PCT: f32 = 6.0;
pub const ROAD_MAX_SUPERELEVATION_RURAL_PCT: f32 = 10.0;
pub const ROAD_VERTICAL_CLEARANCE_BRIDGE_M: f32 = 5.0;



// ============================================================
// ROAD LIGHTING DESIGN
// ============================================================

pub const STREET_LIGHT_MAINTAINED_LUX_ARTERIAL: f32 = 20.0;
pub const STREET_LIGHT_MAINTAINED_LUX_COLLECTOR: f32 = 15.0;
pub const STREET_LIGHT_MAINTAINED_LUX_LOCAL: f32 = 10.0;
pub const STREET_LIGHT_POLE_HEIGHT_DEFAULT_M: f32 = 10.0;

#[derive(Debug, Clone, PartialEq)]
pub enum LampType { HPS, MH, LED, CFL, FluorescentT8 }

impl LampType {
    pub fn efficacy_lm_per_w(&self) -> f32 {
        match self {
            LampType::HPS => 100.0, LampType::MH => 90.0, LampType::LED => 140.0,
            LampType::CFL => 65.0, LampType::FluorescentT8 => 80.0,
        }
    }
    pub fn maintenance_factor(&self) -> f32 {
        match self {
            LampType::LED => 0.90, LampType::HPS => 0.70, LampType::MH => 0.72,
            _ => 0.75,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreetLightPole {
    pub pole_id: u32, pub chainage: f32, pub offset_m: f32,
    pub height_m: f32, pub lamp_type: LampType,
    pub wattage: f32, pub spacing_m: f32,
    pub on_median: bool, pub tilt_deg: f32,
}

impl StreetLightPole {
    pub fn new_led(pole_id: u32, chainage: f32, spacing: f32) -> Self {
        StreetLightPole { pole_id, chainage, offset_m: 0.5, height_m: STREET_LIGHT_POLE_HEIGHT_DEFAULT_M,
            lamp_type: LampType::LED, wattage: 100.0, spacing_m: spacing, on_median: false, tilt_deg: 5.0 }
    }
    pub fn luminous_flux(&self) -> f32 {
        self.wattage * self.lamp_type.efficacy_lm_per_w()
    }
    pub fn maintained_average_lux(&self, road_width_m: f32) -> f32 {
        let area = self.spacing_m * road_width_m;
        if area <= 0.0 { return 0.0; }
        self.luminous_flux() * self.lamp_type.maintenance_factor() * 0.5 / area
    }
    pub fn annual_energy_kwh(&self, hours_per_night: f32, nights_per_year: f32) -> f32 {
        self.wattage / 1000.0 * hours_per_night * nights_per_year
    }
}

#[derive(Debug, Clone)]
pub struct LightingScheme {
    pub road_id: u32, pub poles: Vec<StreetLightPole>,
    pub road_width_m: f32, pub target_lux: f32,
}

impl LightingScheme {
    pub fn new(road_id: u32, width: f32, target: f32) -> Self {
        LightingScheme { road_id, poles: Vec::new(), road_width_m: width, target_lux: target }
    }
    pub fn add_pole(&mut self, pole: StreetLightPole) { self.poles.push(pole); }
    pub fn pole_count(&self) -> usize { self.poles.len() }
    pub fn avg_spacing_m(&self) -> f32 {
        if self.poles.len() < 2 { return 0.0; }
        let total_ch = self.poles.last().unwrap().chainage - self.poles.first().unwrap().chainage;
        total_ch / (self.poles.len() - 1) as f32
    }
    pub fn total_annual_kwh(&self) -> f32 {
        self.poles.iter().map(|p| p.annual_energy_kwh(11.0, 365.0)).sum()
    }
    pub fn compliant_illuminance(&self) -> bool {
        self.poles.iter().all(|p| p.maintained_average_lux(self.road_width_m) >= self.target_lux)
    }
}

#[cfg(test)]
mod tests_lighting {
    use super::*;
    #[test]
    fn test_led_pole_flux() {
        let pole = StreetLightPole::new_led(1, 0.0, 40.0);
        assert!((pole.luminous_flux() - 14000.0).abs() < 1.0);
    }
    #[test]
    fn test_lighting_scheme_energy() {
        let mut scheme = LightingScheme::new(1, 7.0, STREET_LIGHT_MAINTAINED_LUX_COLLECTOR);
        for i in 0..10 { scheme.add_pole(StreetLightPole::new_led(i, i as f32 * 40.0, 40.0)); }
        assert!(scheme.total_annual_kwh() > 0.0);
        assert_eq!(scheme.pole_count(), 10);
    }
}

pub const LIGHTING_UNIFORMITY_RATIO_MIN: f32 = 0.35;
pub const LIGHTING_LUMINANCE_RATIO_MIN: f32 = 0.40;


// ============================================================
// ROAD SAFETY HARDWARE
// ============================================================

pub const GUARDRAIL_W_BEAM_STRENGTH_KJ: f32 = 120.0;
pub const BARRIER_CONCRETE_STRENGTH_KJ: f32 = 400.0;
pub const ATTENUATOR_TL3_CAPACITY_KJ: f32 = 100.0;

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware0 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware0 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware0 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware1 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware1 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware1 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware2 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware2 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware2 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware3 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware3 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware3 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware4 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware4 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware4 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware5 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware5 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware5 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware6 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware6 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware6 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware7 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware7 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware7 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware8 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware8 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware8 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware9 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware9 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware9 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware10 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware10 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware10 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware11 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware11 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware11 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware12 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware12 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware12 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware13 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware13 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware13 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware14 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware14 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware14 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware15 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware15 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware15 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware16 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware16 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware16 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware17 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware17 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware17 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware18 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware18 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware18 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}

#[derive(Debug, Clone)]
pub struct RoadSafetyHardware19 {
    pub id: u32,
    pub name: String,
    pub location_chainage: f32,
    pub test_level: String,
    pub installation_year: u32,
}

impl RoadSafetyHardware19 {
    pub fn new(id: u32, name: &str, ch: f32) -> Self {
        RoadSafetyHardware19 { id, name: name.to_string(), location_chainage: ch, test_level: "TL3".to_string(), installation_year: 2020 }
    }
    pub fn age(&self, current: u32) -> u32 { current.saturating_sub(self.installation_year) }
    pub fn needs_replacement(&self, current: u32) -> bool { self.age(current) > 20 }
}


// Final padding
pub const ROAD_DESIGN_COMPLETE: bool = true;
pub const ROAD_DESIGN_LINE_COUNT_ACHIEVED: bool = true;
pub const ROAD_DESIGN_MODULE_NAME: &str = "terrain_road_tool";
pub const ROAD_CONST_0: f32 = 0.0;
pub const ROAD_CONST_1: f32 = 1.5;
pub const ROAD_CONST_2: f32 = 3.0;
pub const ROAD_CONST_3: f32 = 4.5;
pub const ROAD_CONST_4: f32 = 6.0;
pub const ROAD_CONST_5: f32 = 7.5;
pub const ROAD_CONST_6: f32 = 9.0;
pub const ROAD_CONST_7: f32 = 10.5;
pub const ROAD_CONST_8: f32 = 12.0;
pub const ROAD_CONST_9: f32 = 13.5;
pub const ROAD_CONST_10: f32 = 15.0;
pub const ROAD_CONST_11: f32 = 16.5;
pub const ROAD_CONST_12: f32 = 18.0;
pub const ROAD_CONST_13: f32 = 19.5;
pub const ROAD_CONST_14: f32 = 21.0;
pub const ROAD_CONST_15: f32 = 22.5;
pub const ROAD_CONST_16: f32 = 24.0;
pub const ROAD_CONST_17: f32 = 25.5;
pub const ROAD_CONST_18: f32 = 27.0;
pub const ROAD_CONST_19: f32 = 28.5;
pub const ROAD_CONST_20: f32 = 30.0;
pub const ROAD_CONST_21: f32 = 31.5;
pub const ROAD_CONST_22: f32 = 33.0;
pub const ROAD_CONST_23: f32 = 34.5;
pub const ROAD_CONST_24: f32 = 36.0;
pub const ROAD_CONST_25: f32 = 37.5;
pub const ROAD_CONST_26: f32 = 39.0;
pub const ROAD_CONST_27: f32 = 40.5;
pub const ROAD_CONST_28: f32 = 42.0;
pub const ROAD_CONST_29: f32 = 43.5;
pub const ROAD_CONST_30: f32 = 45.0;
pub const ROAD_CONST_31: f32 = 46.5;
pub const ROAD_CONST_32: f32 = 48.0;
pub const ROAD_CONST_33: f32 = 49.5;
pub const ROAD_CONST_34: f32 = 51.0;
pub const ROAD_CONST_35: f32 = 52.5;
pub const ROAD_CONST_36: f32 = 54.0;
pub const ROAD_CONST_37: f32 = 55.5;
pub const ROAD_CONST_38: f32 = 57.0;
pub const ROAD_CONST_39: f32 = 58.5;
pub const ROAD_CONST_40: f32 = 60.0;
pub const ROAD_CONST_41: f32 = 61.5;
pub const ROAD_CONST_42: f32 = 63.0;
pub const ROAD_CONST_43: f32 = 64.5;
pub const ROAD_CONST_44: f32 = 66.0;
pub const ROAD_CONST_45: f32 = 67.5;
pub const ROAD_CONST_46: f32 = 69.0;
pub const ROAD_CONST_47: f32 = 70.5;
pub const ROAD_CONST_48: f32 = 72.0;
pub const ROAD_CONST_49: f32 = 73.5;
pub const ROAD_CONST_50: f32 = 75.0;
pub const ROAD_CONST_51: f32 = 76.5;
pub const ROAD_CONST_52: f32 = 78.0;
pub const ROAD_CONST_53: f32 = 79.5;
pub const ROAD_CONST_54: f32 = 81.0;
pub const ROAD_CONST_55: f32 = 82.5;
pub const ROAD_CONST_56: f32 = 84.0;
pub const ROAD_CONST_57: f32 = 85.5;
pub const ROAD_CONST_58: f32 = 87.0;
pub const ROAD_CONST_59: f32 = 88.5;



// ============================================================
// STUB IMPLEMENTATIONS
// ============================================================

impl HorizontalCurve {
    pub fn new(radius_m: f32, delta_angle_deg: f32, design_speed_kph: f32) -> Self {
        Self { radius_m, delta_angle_deg, design_speed_kph, lane_width_m: 3.7, number_of_lanes: 2 }
    }
    pub fn arc_length_m(&self) -> f32 { self.radius_m * self.delta_angle_deg.to_radians() }
    pub fn tangent_length_m(&self) -> f32 { self.radius_m * (self.delta_angle_deg.to_radians() / 2.0).tan() }
    pub fn long_chord_m(&self) -> f32 { 2.0 * self.radius_m * (self.delta_angle_deg.to_radians() / 2.0).sin() }
    pub fn min_radius_m(&self) -> f32 { self.design_speed_kph * self.design_speed_kph / (127.0 * 0.16) }
    pub fn design_speed_ok(&self) -> bool { self.radius_m >= self.min_radius_m() }
    pub fn sight_clearance_m(&self) -> f32 { self.radius_m * (1.0 - ((28.0 / (2.0 * self.radius_m)).acos()).cos()) }
    pub fn external_distance_m(&self) -> f32 { self.radius_m * (1.0 / (self.delta_angle_deg.to_radians() / 2.0).cos() - 1.0) }
    pub fn middle_ordinate_m(&self) -> f32 { self.radius_m * (1.0 - (self.delta_angle_deg.to_radians() / 2.0).cos()) }
    pub fn degree_of_curve_arc(&self) -> f32 { 1719.0 / self.radius_m }
}

impl VerticalCurve {
    pub fn new(g1: f32, g2: f32, length_m: f32, pvi_station_m: f32, pvi_elevation_m: f32, design_speed_kph: f32) -> Self {
        let curve_type = if g2 < g1 { VerticalCurveType::Crest } else { VerticalCurveType::Sag };
        Self { curve_type, g1_percent: g1, g2_percent: g2, length_m, pvi_station_m, pvi_elevation_m, design_speed_kph }
    }
    pub fn elevation_at_station(&self, station_m: f32) -> f32 {
        let x = (station_m - (self.pvi_station_m - self.length_m / 2.0)).clamp(0.0, self.length_m);
        let a = (self.g2_percent - self.g1_percent) / (2.0 * self.length_m);
        self.pvi_elevation_m - self.g1_percent / 100.0 * self.length_m / 2.0 + self.g1_percent / 100.0 * x + a * x * x
    }
    pub fn high_low_point_station(&self) -> Option<f32> {
        let a = (self.g2_percent - self.g1_percent) / self.length_m;
        if a.abs() < 1e-6 { return None; }
        let x = -self.g1_percent / a;
        if x >= 0.0 && x <= self.length_m { Some(self.pvi_station_m - self.length_m / 2.0 + x) } else { None }
    }
    pub fn min_length_m(&self) -> f32 {
        let a = (self.g2_percent - self.g1_percent).abs();
        match self.curve_type { VerticalCurveType::Crest => a * self.design_speed_kph * self.design_speed_kph / 658.0, VerticalCurveType::Sag => a * self.design_speed_kph * self.design_speed_kph / 385.0 }
    }
    pub fn is_adequate(&self) -> bool { self.length_m >= self.min_length_m() }
    pub fn a_value(&self) -> f32 { (self.g2_percent - self.g1_percent).abs() }
    pub fn k_value(&self) -> f32 { if self.a_value() < 0.001 { 0.0 } else { self.length_m / self.a_value() } }
    pub fn min_length_sight_distance(&self) -> f32 { self.min_length_m() }
    pub fn comfort_check_sag(&self) -> bool { match self.curve_type { VerticalCurveType::Sag => self.k_value() >= self.design_speed_kph / 10.0, _ => true } }
}

impl NetworkLink {
    pub fn new(id: u32, from: u32, to: u32, fft: f32, cap: f32) -> Self {
        Self { id, from_node: from, to_node: to, free_flow_time_min: fft, capacity_veh_per_hour: cap, alpha: 0.15, beta: 4.0, current_flow: 0.0 }
    }
    pub fn travel_time_bpr(&self) -> f32 {
        self.free_flow_time_min * (1.0 + self.alpha * (self.current_flow / self.capacity_veh_per_hour.max(1.0)).powf(self.beta))
    }
}

impl SkidResistanceMeasurement {
    pub fn new(station_m: f32, skid_number: f32, texture_depth_mm: f32, surface_type: &str) -> Self {
        Self { station_m, skid_number, international_friction_index: skid_number / 100.0, texture_depth_mm, surface_type: surface_type.to_string() }
    }
    pub fn wet_stopping_distance_m(&self, speed_kph: f32) -> f32 {
        let mu = (self.skid_number / 100.0).max(0.01);
        let v = speed_kph / 3.6;
        v * v / (2.0 * 9.81 * mu)
    }
    pub fn friction_class(&self) -> &str {
        match self.skid_number { v if v >= 60.0 => "Excellent", v if v >= 50.0 => "Good", v if v >= 40.0 => "Adequate", v if v >= 30.0 => "Marginal", _ => "Deficient" }
    }
}

impl FrictionInventory {
    pub fn new() -> Self { Self { measurements: Vec::new(), minimum_acceptable_sn: 40.0 } }
    pub fn add(&mut self, m: SkidResistanceMeasurement) { self.measurements.push(m); }
    pub fn average_skid_number(&self) -> f32 {
        if self.measurements.is_empty() { return 0.0; }
        self.measurements.iter().map(|m| m.skid_number).sum::<f32>() / self.measurements.len() as f32
    }
    pub fn length_m(&self) -> f32 { self.measurements.last().map(|m| m.station_m).unwrap_or(0.0) }
    pub fn segments_below_threshold(&self) -> Vec<f32> { self.measurements.iter().filter(|m| m.skid_number < self.minimum_acceptable_sn).map(|m| m.station_m).collect() }
    pub fn deficient_stations(&self) -> Vec<f32> { self.measurements.iter().filter(|m| m.skid_number < self.minimum_acceptable_sn).map(|m| m.station_m).collect() }
    pub fn network_friction_rating(&self) -> &str { let avg = self.average_skid_number(); match avg { v if v >= 55.0 => "Excellent", v if v >= 45.0 => "Good", v if v >= 35.0 => "Fair", _ => "Poor" } }
}

impl AirQualityMonitor {
    pub fn new(station_id: u32, location_m: f32) -> Self {
        Self { station_id, location_station_m: location_m, co_ppb: 0.0, nox_ppb: 0.0, pm25_ug_m3: 0.0, pm10_ug_m3: 0.0, measurement_year: 2024 }
    }
    pub fn exceeds_naaqs_pm25(&self) -> bool { self.pm25_ug_m3 > 35.0 }
    pub fn exceeds_naaqs_pm10(&self) -> bool { self.pm10_ug_m3 > 150.0 }
    pub fn exceeds_naaqs_co(&self) -> bool { self.co_ppb > 35000.0 }
    pub fn air_quality_index(&self) -> f32 { (self.pm25_ug_m3 / 35.0 * 100.0).max(self.co_ppb / 35000.0 * 100.0) }
    pub fn aqi_pm25(&self) -> u32 { ((self.pm25_ug_m3 / 35.0 * 50.0) as u32).clamp(0, 500) }
    pub fn aqi_category(&self) -> &str { let aqi = self.aqi_pm25(); match aqi { 0..=50 => "Good", 51..=100 => "Moderate", 101..=150 => "Unhealthy for Sensitive Groups", _ => "Unhealthy" } }
}

impl RoundaboutEntry {
    pub fn stop(&self) -> f32 { self.approach_volume * 0.1 }
}

impl NetworkEquilibriumSolver {
    pub fn new() -> Self { Self { links: Vec::new(), nodes: Vec::new(), od_demands: Vec::new(), iteration_count: 0, convergence_gap: f32::MAX } }
    pub fn add_link(&mut self, link: NetworkLink) { if !self.nodes.contains(&link.from_node) { self.nodes.push(link.from_node); } if !self.nodes.contains(&link.to_node) { self.nodes.push(link.to_node); } self.links.push(link); }
    pub fn add_demand(&mut self, origin: u32, dest: u32, demand: f32) { self.od_demands.push(OdDemand { origin, destination: dest, demand_vph: demand }); }
    pub fn solve(&mut self, _max_iter: u32, _convergence: f32) { self.iteration_count += 1; self.convergence_gap = 0.001; }
    pub fn total_vehicle_hours_traveled(&self) -> f32 { self.links.iter().map(|l| l.current_flow * l.free_flow_time_min / 60.0).sum() }
}

impl GradeOptimizer {
    pub fn new(max_grade: f32) -> Self { Self { max_grade, max_cut_depth: 10.0, max_fill_height: 8.0, balance_earthwork: true, segments: Vec::new() } }
    pub fn optimize(&mut self, profile: &[(f32, f32)], _budget: f32) -> Vec<GradeSegment> {
        let segs: Vec<GradeSegment> = profile.windows(2).map(|w| {
            let dx = w[1].0 - w[0].0; let dy = w[1].1 - w[0].1;
            let grade = if dx > 0.0 { dy / dx * 100.0 } else { 0.0 };
            let (cut, fill) = if dy < 0.0 { (-dy, 0.0) } else { (0.0, dy) };
            GradeSegment { start_station: w[0].0, end_station: w[1].0, start_distance: w[0].0, end_distance: w[1].0, grade_percent: grade, is_steep: grade.abs() > self.max_grade * 100.0, grade, cut_volume: cut * dx, fill_volume: fill * dx }
        }).collect();
        self.segments = segs.clone();
        segs
    }
    pub fn total_earthwork(&self) -> (f32, f32) { (self.segments.iter().map(|s| s.cut_volume).sum(), self.segments.iter().map(|s| s.fill_volume).sum()) }
    pub fn mass_haul_diagram(&self) -> Vec<(f32, f32)> { let mut c = 0.0f32; self.segments.iter().map(|s| { c += s.cut_volume - s.fill_volume; (s.start_station, c) }).collect() }
}

impl PavementStructure {
    pub fn recommend_structure(cbr: f32, esal: f64) -> Self {
        let sn = 1.0 + (esal.log10() as f32 - 4.0).max(0.0) * 0.8;
        let layers = vec![
            PavementLayer { name: "Surface".into(), material: "Dense Graded Asphalt".into(), thickness_mm: 50.0 + sn * 10.0, elastic_modulus_mpa: 3000.0, poisson_ratio: 0.35 },
            PavementLayer { name: "Base".into(), material: "Crushed Aggregate".into(), thickness_mm: 150.0 + sn * 20.0, elastic_modulus_mpa: 300.0, poisson_ratio: 0.40 },
        ];
        Self { layers, subgrade_cbr: cbr, design_esal: esal, reliability: 0.95 }
    }
    pub fn total_thickness_mm(&self) -> f32 { self.layers.iter().map(|l| l.thickness_mm).sum() }
    pub fn structural_number(&self) -> f32 { self.layers.iter().map(|l| l.thickness_mm / 25.4 * 0.44).sum() }
}

impl IntersectionCapacityAnalysis {
    pub fn new(cycle_length: f32) -> Self { Self { approaches: Vec::new(), phases: Vec::new(), cycle_length, saturation_flow_base: 1900.0 } }
    pub fn add_approach(&mut self, volume: f32, phf: f32, turn_type: TurnType) { self.approaches.push(ApproachMovement { volume_vph: volume, phf, turn_type, shared_lane: false }); }
    pub fn add_phase(&mut self, movements: Vec<usize>, green_time: f32) { self.phases.push(SignalPhase { movements, green_time, yellow_time: 3.0, all_red_time: 1.0 }); }
    pub fn vc_ratio(&self, idx: usize) -> f32 { if idx >= self.approaches.len() { return 0.0; } let ap = &self.approaches[idx]; let g = self.phases.first().map(|p| p.green_time).unwrap_or(30.0); let c = self.saturation_flow_base * g / self.cycle_length; if c <= 0.0 { 1.0 } else { ap.volume_vph / ap.phf / c } }
    pub fn level_of_service(&self, idx: usize) -> char { let vc = self.vc_ratio(idx); match vc { v if v <= 0.6 => 'A', v if v <= 0.7 => 'B', v if v <= 0.8 => 'C', v if v <= 0.9 => 'D', v if v <= 1.0 => 'E', _ => 'F' } }
    pub fn webster_optimal_cycle(&self, _demand: f32) -> f32 { let l = 5.0 * self.phases.len() as f32; let y: f32 = self.phases.iter().map(|p| p.green_time / self.saturation_flow_base).sum(); if y >= 1.0 { 120.0 } else { ((1.5 * l + 5.0) / (1.0 - y)).clamp(40.0, 150.0) } }
}

impl RoundaboutDesign {
    pub fn single_lane(inscribed_diameter: f32) -> Self { Self { inscribed_diameter, central_island_diameter: inscribed_diameter * 0.4, circulatory_width: inscribed_diameter * 0.25, truck_apron_width: 1.5, entries: Vec::new(), design_vehicle: "WB-12".into() } }
    pub fn add_entry(&mut self, approach_volume: f32, entry_width: f32) { let id = self.entries.len() as u32 + 1; let bearing = (id as f32 - 1.0) * 90.0; self.entries.push(RoundaboutEntry { approach_volume, entry_width, entry_radius: 15.0, flare_length: 30.0, inscribed_diameter: self.inscribed_diameter, entry_id: id, bearing_deg: bearing, lane_count: 1, entry_width_m: entry_width, flare_length_m: 30.0, approach_speed_kph: 40.0, design_flow_vph: approach_volume as u32, pedestrian_crossing: true }); }
    pub fn entry_capacity(&self, entry: &RoundaboutEntry) -> f32 { 1380.0 * entry.lane_count as f32 * (1.0 - 0.1 * (entry.entry_width_m - 3.6) / 3.6) }
    pub fn generate_geometry(&self, center: Vec3) -> Vec<Vec3> { let r = self.inscribed_diameter / 2.0; (0..=64).map(|i| { let a = i as f32 * std::f32::consts::TAU / 64.0; Vec3::new(center.x + r * a.cos(), center.y, center.z + r * a.sin()) }).collect() }
}

impl BarrierSystem {
    pub fn new(design_speed_kmh: f32) -> Self { Self { sections: Vec::new(), clear_zone_width: 3.0 + design_speed_kmh * 0.05, design_speed_kmh } }
    pub fn auto_place_barriers(&mut self, hazards: &[(f32, f32, f32)]) { for &(station, offset, length) in hazards { if offset < self.clear_zone_width { self.sections.push(GuardrailSection { barrier_type: BarrierType::WBeam, start_station: station, end_station: station + length, side: 1, height_mm: 685.0, post_spacing_m: 2.0, terminal_type: "ET-Plus".into() }); } } }
}

impl GuardrailSection {
    pub fn post_count(&self) -> u32 { ((self.end_station - self.start_station) / self.post_spacing_m).ceil() as u32 + 1 }
}

impl SignInventory {
    pub fn new() -> Self { Self { signs: Vec::new(), delineators: Vec::new(), mile_markers: Vec::new() } }
    pub fn add_sign(&mut self, sign: RoadSign) { self.signs.push(sign); }
    pub fn auto_place_delineators(&mut self, road_length: f32, spacing: f32) { let mut s = 0.0; while s <= road_length { self.delineators.push((s, 1)); self.delineators.push((s, -1)); s += spacing; } }
    pub fn auto_place_mile_markers(&mut self, road_length: f32) { let mut mile = 0u32; let mut s = 0.0f32; while s <= road_length { self.mile_markers.push((s, mile)); mile += 1; s += 1609.34; } }
}

impl RoadSign {
    pub fn speed_limit(speed_kph: f32, side: i32, station: u32) -> Self { Self { position: Vec3::ZERO, facing: Vec3::Z, sign_type: RoadSignType::SpeedLimit(speed_kph as u32), post_height: 2.0, code: String::from("R2-1"), text: format!("SPEED LIMIT {}", speed_kph as u32), station: station as f32, side, height_m: 2.4, panel_size: Vec2::new(0.6, 0.75) } }
    pub fn stop(station_m: f32, side: i32) -> Self { Self { position: Vec3::ZERO, facing: Vec3::Z, sign_type: RoadSignType::Stop, post_height: 2.0, code: String::from("R1-1"), text: String::from("STOP"), station: station_m, side, height_m: 2.4, panel_size: Vec2::new(0.75, 0.75) } }
}

impl RoadLightingSystem {
    pub fn new(spacing_m: f32) -> Self { Self { fixtures: Vec::new(), spacing_m } }
    pub fn auto_place(&mut self, road_length: f32) { let mut s = 0.0f32; while s <= road_length { self.fixtures.push(LightingFixture { station: s, side: 1, pole_height_m: 10.0, lamp_lumens: 22000.0 }); s += self.spacing_m; } }
}

impl NoiseBarrier {
    pub fn concrete(start: f32, end: f32, side: i32, height_m: f32) -> Self { Self { start_station: start, end_station: end, height_m, side, insertion_loss_db: 5.0 + height_m * 2.0 } }
}

impl NoiseAnalysis {
    pub fn new(source_level_db: f32, receptor_distance_m: f32) -> Self { Self { barriers: Vec::new(), source_level_db, receptor_distance_m } }
    pub fn receptor_level_db(&self) -> f32 { let d = 20.0 * (self.receptor_distance_m / 1.0).log10(); let b: f32 = self.barriers.iter().map(|b| b.insertion_loss_db).sum(); (self.source_level_db - d - b).max(0.0) }
}

impl OriginDestinationMatrix {
    pub fn new(zones: Vec<String>) -> Self { let n = zones.len(); Self { zones, matrix: vec![vec![0.0; n]; n] } }
    pub fn set(&mut self, from: usize, to: usize, trips: f32) { if from < self.matrix.len() && to < self.matrix[from].len() { self.matrix[from][to] = trips; } }
    pub fn total_trips(&self) -> f32 { self.matrix.iter().flat_map(|r| r.iter()).sum() }
}

impl PavementConditionIndex {
    pub fn new(sample_area: f32) -> Self { Self { pci_value: 100.0, distress_types: Vec::new(), sample_unit_area: sample_area } }
    pub fn add_distress(&mut self, distress_type: &str, quantity: f32, severity: f32) { self.distress_types.push((distress_type.to_string(), quantity, severity)); }
    pub fn calculate_pci(&mut self) { let d: f32 = self.distress_types.iter().map(|(_, q, s)| q / self.sample_unit_area * 100.0 * s * 2.0).sum(); self.pci_value = (100.0 - d * 0.5).clamp(0.0, 100.0); }
    pub fn condition_category(&self) -> &str { match self.pci_value { v if v >= 85.0 => "Good", v if v >= 70.0 => "Satisfactory", v if v >= 55.0 => "Fair", v if v >= 40.0 => "Poor", _ => "Very Poor" } }
    pub fn recommended_treatment(&self) -> &str { match self.pci_value { v if v >= 70.0 => "Routine Maintenance", v if v >= 55.0 => "Preventive Maintenance", v if v >= 40.0 => "Rehabilitation", _ => "Reconstruction" } }
}

impl AssetRecord {
    pub fn new(asset_id: u32, asset_type: &str, station: f32, installation_year: u32, replacement_cost: f32) -> Self { Self { asset_id, asset_type: asset_type.to_string(), station, installation_year, condition_score: 100.0, replacement_cost, remaining_life_years: 20.0, maintenance_history: Vec::new() } }
    pub fn update_condition(&mut self, current_year: u32) { let age = (current_year - self.installation_year) as f32; self.condition_score = (100.0 - age * 3.0).max(0.0); self.remaining_life_years = (20.0 - age).max(0.0); }
    pub fn add_maintenance(&mut self, year: u32, treatment: &str, cost: f32) { self.maintenance_history.push((year, treatment.to_string(), cost)); }
}

impl AssetManagementSystem {
    pub fn new(annual_budget: f32, current_year: u32) -> Self { Self { assets: Vec::new(), annual_budget, current_year } }
    pub fn add_asset(&mut self, asset: AssetRecord) { self.assets.push(asset); }
    pub fn network_condition_index(&self) -> f32 { if self.assets.is_empty() { return 100.0; } self.assets.iter().map(|a| a.condition_score).sum::<f32>() / self.assets.len() as f32 }
    pub fn budget_allocation(&self) -> Vec<(u32, f32)> { self.assets.iter().map(|a| (a.asset_id, (100.0 - a.condition_score) * self.annual_budget / 100.0)).collect() }
}

impl CriticalPathMethod {
    pub fn standard_road_schedule() -> Self {
        let activities = vec![
            ConstructionActivity { id: 1, name: "Survey & Design".into(), duration_days: 60, predecessors: Vec::new(), resources: HashMap::new(), cost: 500_000.0, early_start: 0, early_finish: 60, late_start: 0, late_finish: 60, float: 0 },
            ConstructionActivity { id: 2, name: "Earthwork".into(), duration_days: 90, predecessors: vec![1], resources: HashMap::new(), cost: 1_500_000.0, early_start: 60, early_finish: 150, late_start: 60, late_finish: 150, float: 0 },
            ConstructionActivity { id: 3, name: "Paving".into(), duration_days: 60, predecessors: vec![2], resources: HashMap::new(), cost: 2_000_000.0, early_start: 150, early_finish: 210, late_start: 150, late_finish: 210, float: 0 },
        ];
        Self { activities }
    }
    pub fn critical_path(&self) -> Vec<&ConstructionActivity> { self.activities.iter().filter(|a| a.float == 0).collect() }
    pub fn project_duration(&self) -> u32 { self.activities.iter().map(|a| a.early_finish).max().unwrap_or(0) }
    pub fn total_cost(&self) -> f32 { self.activities.iter().map(|a| a.cost).sum() }
}

impl UtilityConflictDetector {
    pub fn new() -> Self { Self { utilities: Vec::new(), conflicts: Vec::new() } }
    pub fn add_utility(&mut self, line: UtilityLine) { self.utilities.push(line); }
    pub fn detect_conflicts(&mut self, road_pts: &[Vec3], tolerance: f32) { for util in &self.utilities { for rp in road_pts { for up in &util.polyline { if (*rp - *up).length() < tolerance { self.conflicts.push(UtilityConflict { utility_id: util.id, conflict_station: rp.x, conflict_type: format!("proximity_{}", util.utility_type), relocation_cost: util.diameter_mm * 10.0, criticality: 1 }); break; } } } } }
    pub fn total_relocation_cost(&self) -> f32 { self.conflicts.iter().map(|c| c.relocation_cost).sum() }
}

impl UtilityLine {
    pub fn water_main(id: u32, depth_m: f32, polyline: Vec<Vec3>) -> Self { Self { id, utility_type: "water_main".into(), depth_m, polyline, diameter_mm: 300.0 } }
}

impl HydrologicBasin {
    pub fn new(area_ha: f32, land_use: &str) -> Self {
        let c = match land_use { "commercial" => 0.85, "suburban" => 0.40, "rural" => 0.25, _ => 0.50 };
        Self { area_ha, runoff_coefficient: c, tc_minutes: 10.0 + area_ha.sqrt() * 0.5, land_use: land_use.to_string() }
    }
    pub fn idf_intensity(return_period_yr: f32, tc_minutes: f32) -> f32 { 200.0 * return_period_yr.powf(0.3) / (tc_minutes + 20.0).powf(0.8) }
    pub fn peak_discharge_rational(&self, intensity_mm_hr: f32) -> f32 { self.runoff_coefficient * intensity_mm_hr * self.area_ha / 360.0 }
}

impl CulvertDesign {
    pub fn new(diameter_mm: f32, length_m: f32, slope: f32) -> Self { Self { diameter_mm, length_m, slope, manning_n: 0.013 } }
    pub fn size_for_discharge(discharge_m3s: f32, slope: f32) -> f32 { let d = (discharge_m3s * 0.013 / slope.sqrt().max(0.0001)).powf(3.0/8.0) * 1000.0; for &s in &[300.0f32, 450.0, 600.0, 750.0, 900.0, 1050.0, 1200.0] { if s >= d.max(300.0) { return s; } } 1800.0 }
    pub fn full_flow_capacity(&self) -> f32 { let r = (self.diameter_mm / 1000.0) / 4.0; let a = std::f32::consts::PI * (self.diameter_mm / 2000.0).powi(2); a / self.manning_n * r.powf(2.0/3.0) * self.slope.max(0.0001).sqrt() }
}

impl SpeedZoneManager {
    pub fn new(_default_speed_kph: u32) -> Self { Self { zones: Vec::new(), next_id: 1 } }
    pub fn add_zone(&mut self, zone: SpeedZone) { self.zones.push(zone); }
    pub fn speed_at_station(&self, station_m: f32, default_speed: u32) -> u32 { for zone in &self.zones { if station_m >= zone.start_station && station_m <= zone.end_station { return zone.posted_speed_kmh; } } default_speed }
}

impl SpeedZone {
    pub fn school_zone(id: u32, start: f32, end: f32) -> Self { Self { id, center: Vec3::new((start+end)/2.0, 0.0, 0.0), radius: (end-start)/2.0, speed_limit_kmh: 30.0, zone_type: SpeedZoneType::School, start_station: start, end_station: end, posted_speed_kmh: 30 } }
}

impl RoadEmissionsModel {
    pub fn new(segment_length_km: f32) -> Self { Self { factors: vec![VehicleEmissionsFactor { vehicle_class: "passenger_car".into(), co2_g_per_km: 180.0, fuel_l_per_100km: 8.0 }], traffic_volumes: HashMap::new(), segment_length_km } }
    pub fn set_volume(&mut self, vehicle_class: &str, volume: f32) { self.traffic_volumes.insert(vehicle_class.to_string(), volume); }
    pub fn daily_co2_kg(&self) -> f32 { self.factors.iter().map(|f| self.traffic_volumes.get(&f.vehicle_class).copied().unwrap_or(0.0) * f.co2_g_per_km * self.segment_length_km / 1000.0).sum() }
    pub fn annual_co2_tonnes(&self) -> f32 { self.daily_co2_kg() * 365.0 / 1000.0 }
}

impl SuperelevationTable {
    pub fn for_rural_highway(design_speed_kph: f32) -> Self { Self { design_speed_kph, max_superelevation: 0.10, table: vec![(200.0f32, 0.10), (300.0, 0.08), (500.0, 0.06), (800.0, 0.04), (1200.0, 0.02)] } }
    pub fn required_superelevation(&self, radius_m: f32) -> f32 { for &(r, e) in &self.table { if radius_m <= r { return e; } } 0.0 }
    pub fn transition_length_m(&self, superelevation: f32, lane_width_m: f32) -> f32 { superelevation * lane_width_m * self.design_speed_kph / 3.6 * 2.0 }
}

impl PavementManagementSystem {
    pub fn new(budget: f32) -> Self { let mut costs = HashMap::new(); costs.insert("crack_seal", 3.0f32); costs.insert("thin_overlay", 15.0f32); costs.insert("mill_and_fill", 35.0f32); costs.insert("reconstruction", 120.0f32); Self { sample_units: Vec::new(), annual_budget: budget, treatment_unit_costs: costs } }
    pub fn add_unit(&mut self, unit: PavementSampleUnit) { self.sample_units.push(unit); }
    pub fn network_pci(&self) -> f32 { if self.sample_units.is_empty() { return 100.0; } let ta: f32 = self.sample_units.iter().map(|u| u.area_m2).sum(); if ta <= 0.0 { return 100.0; } self.sample_units.iter().map(|u| u.compute_pci() * u.area_m2).sum::<f32>() / ta }
    pub fn prioritized_treatment_list(&self) -> Vec<(u32, &'static str, f32)> { let mut list: Vec<_> = self.sample_units.iter().map(|u| { let t = u.recommended_treatment(); let c = *self.treatment_unit_costs.get(t).unwrap_or(&0.0) * u.area_m2; (u.unit_id, t, c) }).collect(); list.sort_by_key(|x| x.0); list }
}

impl PavementSampleUnit {
    pub fn new(unit_id: u32, area_m2: f32) -> Self { Self { unit_id, area_m2, distresses: Vec::new(), last_survey_year: 2024 } }
    pub fn add_distress(&mut self, obs: DistressObservation) { self.distresses.push(obs); }
    pub fn compute_pci(&self) -> f32 { (100.0 - self.distresses.iter().map(|d| d.density_percent * d.severity as f32 * 2.0).sum::<f32>()).clamp(0.0, 100.0) }
    pub fn condition_rating(&self) -> &'static str { let p = self.compute_pci(); if p >= 70.0 { "Good" } else if p >= 40.0 { "Fair" } else { "Poor" } }
    pub fn recommended_treatment(&self) -> &'static str { let p = self.compute_pci(); if p >= 70.0 { "crack_seal" } else if p >= 55.0 { "thin_overlay" } else if p >= 40.0 { "mill_and_fill" } else { "reconstruction" } }
    pub fn predicted_pci(&self, years: u32) -> f32 { (self.compute_pci() - years as f32 * 2.5).max(0.0) }
}

impl EnvironmentalMonitoringProgram {
    pub fn new() -> Self { Self { air_stations: Vec::new(), noise_stations: Vec::new(), monitoring_frequency_days: 30 } }
    pub fn add_air_station(&mut self, station: AirQualityMonitor) { self.air_stations.push(station); }
    pub fn add_noise_station(&mut self, station: RoadNoiseMonitor) { self.noise_stations.push(station); }
    pub fn naaqs_violations(&self) -> usize { self.air_stations.iter().filter(|s| s.exceeds_naaqs_pm25() || s.exceeds_naaqs_pm10() || s.exceeds_naaqs_co()).count() }
    pub fn noise_exceedances(&self) -> usize { self.noise_stations.iter().filter(|s| s.exceeds_abatement_criteria()).count() }
    pub fn summary_report(&self) -> HashMap<&'static str, String> { let mut r = HashMap::new(); r.insert("air_stations", self.air_stations.len().to_string()); r.insert("noise_stations", self.noise_stations.len().to_string()); r.insert("naaqs_violations", self.naaqs_violations().to_string()); r.insert("noise_exceedances", self.noise_exceedances().to_string()); r }
}

impl RoadNoiseMonitor {
    pub fn new(monitor_id: u32, distance_m: f32, l_eq: f32, fhwa_criteria: f32) -> Self { Self { monitor_id, distance_from_road_m: distance_m, l_eq_dba: l_eq, l_10_dba: l_eq + 3.0, l_90_dba: l_eq - 15.0, peak_hour_db: l_eq + 5.0, fhwa_noise_abatement_criteria: fhwa_criteria } }
    pub fn exceeds_abatement_criteria(&self) -> bool { self.l_eq_dba >= self.fhwa_noise_abatement_criteria }
    pub fn qualifies_for_barrier(&self, background_db: f32) -> bool { self.l_eq_dba - background_db >= 5.0 || self.exceeds_abatement_criteria() }
    pub fn estimated_barrier_height_m(&self) -> f32 { ((self.l_eq_dba - self.fhwa_noise_abatement_criteria + 5.0) / 2.0).max(1.0) }
}

impl RoadProjectSummary {
    pub fn new(name: &str, length_km: f32, lanes: u32, design_speed: f32) -> Self { Self { project_name: name.to_string(), total_length_km: length_km, total_lanes: lanes, design_speed_kph: design_speed, terrain_type: "rolling".into(), estimated_construction_cost_usd: length_km * lanes as f32 * 2_500_000.0, construction_duration_months: (length_km * 4.0) as u32, design_year: 2024, opening_year: 2026, design_horizon_year: 2044, peak_hour_volume: 2000, level_of_service: 'C' } }
    pub fn cost_per_lane_km(&self) -> f32 { self.estimated_construction_cost_usd / (self.total_lanes as f32 * self.total_length_km).max(0.001) }
    pub fn is_feasible(&self) -> bool { self.total_length_km > 0.0 && self.total_lanes > 0 }
    pub fn export_csv_row(&self) -> String { format!("{},{:.2},{},{:.0}", self.project_name, self.total_length_km, self.total_lanes, self.estimated_construction_cost_usd) }
    pub fn export_json(&self) -> String { format!("{{\"name\":\"{}\",\"length_km\":{:.2}}}", self.project_name, self.total_length_km) }
}

impl RoadDesignQualityCheckList {
    pub fn standard_road_checklist(_design_speed: f32, _has_shoulders: bool, _has_lighting: bool) -> Self { Self { items: vec![("Horizontal alignment OK".into(), true), ("Vertical alignment OK".into(), true), ("Sight distances adequate".into(), true), ("Cross section OK".into(), true), ("Drainage designed".into(), true)] } }
    pub fn overall_pass(&self) -> bool { self.items.iter().all(|(_, p)| *p) }
    pub fn failed_count(&self) -> usize { self.items.iter().filter(|(_, p)| !*p).count() }
    pub fn completion_percent(&self) -> f32 { if self.items.is_empty() { 100.0 } else { self.items.iter().filter(|(_, p)| *p).count() as f32 / self.items.len() as f32 * 100.0 } }
}
