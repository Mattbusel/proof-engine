
//! Navigation mesh editor — mesh generation, agent settings, dynamic obstacles, path visualization.

use glam::{Vec2, Vec3};
use std::collections::{HashMap, BinaryHeap, HashSet};
use std::cmp::Ordering;

// ---------------------------------------------------------------------------
// NavMesh geometry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct NavTriangle {
    pub vertices: [usize; 3],
    pub neighbors: [Option<usize>; 3], // adjacent triangle per edge
    pub area_type: u8,
    pub flags: u32,
}

impl NavTriangle {
    pub fn center(&self, verts: &[Vec3]) -> Vec3 {
        let a = verts[self.vertices[0]];
        let b = verts[self.vertices[1]];
        let c = verts[self.vertices[2]];
        (a + b + c) / 3.0
    }

    pub fn normal(&self, verts: &[Vec3]) -> Vec3 {
        let a = verts[self.vertices[0]];
        let b = verts[self.vertices[1]];
        let c = verts[self.vertices[2]];
        (b - a).cross(c - a).normalize()
    }

    pub fn area(&self, verts: &[Vec3]) -> f32 {
        let a = verts[self.vertices[0]];
        let b = verts[self.vertices[1]];
        let c = verts[self.vertices[2]];
        (b - a).cross(c - a).length() * 0.5
    }

    pub fn contains_point_2d(&self, verts: &[Vec3], p: Vec2) -> bool {
        let a = Vec2::new(verts[self.vertices[0]].x, verts[self.vertices[0]].z);
        let b = Vec2::new(verts[self.vertices[1]].x, verts[self.vertices[1]].z);
        let c = Vec2::new(verts[self.vertices[2]].x, verts[self.vertices[2]].z);
        let d1 = sign(p, a, b);
        let d2 = sign(p, b, c);
        let d3 = sign(p, c, a);
        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
        !(has_neg && has_pos)
    }
}

fn sign(p1: Vec2, p2: Vec2, p3: Vec2) -> f32 {
    (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
}

// ---------------------------------------------------------------------------
// Area types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AreaType {
    pub id: u8,
    pub name: String,
    pub cost: f32,
    pub color: [u8; 4],
    pub passable: bool,
}

impl AreaType {
    pub fn walkable() -> Self { Self { id: 0, name: "Walkable".into(), cost: 1.0, color: [0, 180, 0, 180], passable: true } }
    pub fn water() -> Self { Self { id: 1, name: "Water".into(), cost: 3.0, color: [0, 100, 200, 180], passable: true } }
    pub fn mud() -> Self { Self { id: 2, name: "Mud".into(), cost: 2.0, color: [100, 80, 40, 180], passable: true } }
    pub fn not_walkable() -> Self { Self { id: 3, name: "Not Walkable".into(), cost: f32::INFINITY, color: [200, 0, 0, 180], passable: false } }
    pub fn jump() -> Self { Self { id: 4, name: "Jump".into(), cost: 1.5, color: [200, 200, 0, 180], passable: true } }
}

// ---------------------------------------------------------------------------
// NavMesh
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NavMesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<NavTriangle>,
    pub area_types: Vec<AreaType>,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub cell_size: f32,
    pub cell_height: f32,
}

impl NavMesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
            area_types: vec![
                AreaType::walkable(),
                AreaType::water(),
                AreaType::mud(),
                AreaType::not_walkable(),
                AreaType::jump(),
            ],
            bounds_min: Vec3::splat(-50.0),
            bounds_max: Vec3::splat(50.0),
            cell_size: 0.25,
            cell_height: 0.2,
        }
    }

    pub fn triangle_count(&self) -> usize { self.triangles.len() }
    pub fn vertex_count(&self) -> usize { self.vertices.len() }

    pub fn find_triangle_at(&self, p: Vec2) -> Option<usize> {
        self.triangles.iter().enumerate().find(|(_, t)| t.contains_point_2d(&self.vertices, p)).map(|(i, _)| i)
    }

    pub fn sample_height(&self, p: Vec2) -> Option<f32> {
        let tri_idx = self.find_triangle_at(p)?;
        let tri = &self.triangles[tri_idx];
        // Barycentric interpolation
        let a = self.vertices[tri.vertices[0]];
        let b = self.vertices[tri.vertices[1]];
        let c = self.vertices[tri.vertices[2]];
        let ap = Vec2::new(p.x - a.x, p.y - a.z);
        let ab = Vec2::new(b.x - a.x, b.z - a.z);
        let ac = Vec2::new(c.x - a.x, c.z - a.z);
        let inv_denom = 1.0 / (ab.x * ac.y - ac.x * ab.y).abs().max(1e-10);
        let u = (ap.x * ac.y - ac.x * ap.y) * inv_denom;
        let v = (ab.x * ap.y - ap.x * ab.y) * inv_denom;
        let w = 1.0 - u - v;
        Some(a.y * w + b.y * u + c.y * v)
    }

    /// Generate a simple flat nav mesh for testing.
    pub fn build_flat(bounds: f32, resolution: u32) -> Self {
        let mut nm = NavMesh::new();
        let step = bounds * 2.0 / resolution as f32;
        let n = (resolution + 1) as usize;
        for iz in 0..=resolution {
            for ix in 0..=resolution {
                let x = -bounds + ix as f32 * step;
                let z = -bounds + iz as f32 * step;
                nm.vertices.push(Vec3::new(x, 0.0, z));
            }
        }
        for iz in 0..resolution as usize {
            for ix in 0..resolution as usize {
                let base = iz * n + ix;
                nm.triangles.push(NavTriangle {
                    vertices: [base, base + 1, base + n],
                    neighbors: [None, None, None],
                    area_type: 0,
                    flags: 0,
                });
                nm.triangles.push(NavTriangle {
                    vertices: [base + 1, base + n + 1, base + n],
                    neighbors: [None, None, None],
                    area_type: 0,
                    flags: 0,
                });
            }
        }
        nm.bounds_min = Vec3::new(-bounds, -1.0, -bounds);
        nm.bounds_max = Vec3::new(bounds, 1.0, bounds);
        nm
    }
}

// ---------------------------------------------------------------------------
// A* pathfinding on triangle mesh
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AStarNode {
    pub tri_idx: usize,
    pub g_cost: f32,
    pub f_cost: f32,
    pub parent: Option<usize>,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool { self.tri_idx == other.tri_idx }
}
impl Eq for AStarNode {}
impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap
        other.f_cost.partial_cmp(&self.f_cost).unwrap_or(Ordering::Equal)
    }
}

#[derive(Debug, Clone)]
pub struct NavPath {
    pub waypoints: Vec<Vec3>,
    pub total_length: f32,
    pub status: PathStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathStatus { Complete, Partial, Invalid }

impl NavPath {
    pub fn empty() -> Self { Self { waypoints: Vec::new(), total_length: 0.0, status: PathStatus::Invalid } }

    pub fn from_waypoints(points: Vec<Vec3>) -> Self {
        let length: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
        Self { waypoints: points, total_length: length, status: PathStatus::Complete }
    }

    pub fn interpolate(&self, t: f32) -> Option<Vec3> {
        if self.waypoints.len() < 2 { return self.waypoints.first().copied(); }
        let target_dist = t.clamp(0.0, 1.0) * self.total_length;
        let mut walked = 0.0_f32;
        for window in self.waypoints.windows(2) {
            let seg_len = window[0].distance(window[1]);
            if walked + seg_len >= target_dist {
                let u = (target_dist - walked) / seg_len.max(1e-6);
                return Some(window[0].lerp(window[1], u));
            }
            walked += seg_len;
        }
        self.waypoints.last().copied()
    }
}

pub fn find_path(mesh: &NavMesh, start: Vec3, goal: Vec3) -> NavPath {
    let start_tri = mesh.find_triangle_at(Vec2::new(start.x, start.z));
    let goal_tri = mesh.find_triangle_at(Vec2::new(goal.x, goal.z));
    let (Some(start_idx), Some(goal_idx)) = (start_tri, goal_tri) else {
        return NavPath::empty();
    };
    if start_idx == goal_idx {
        return NavPath::from_waypoints(vec![start, goal]);
    }
    let goal_center = mesh.triangles[goal_idx].center(&mesh.vertices);
    let mut open: BinaryHeap<AStarNode> = BinaryHeap::new();
    let mut closed: HashSet<usize> = HashSet::new();
    let mut came_from: HashMap<usize, usize> = HashMap::new();
    let mut g_scores: HashMap<usize, f32> = HashMap::new();
    g_scores.insert(start_idx, 0.0);
    open.push(AStarNode {
        tri_idx: start_idx,
        g_cost: 0.0,
        f_cost: start.distance(goal_center),
        parent: None,
    });
    while let Some(current) = open.pop() {
        if current.tri_idx == goal_idx {
            // Reconstruct path
            let mut path_tris = vec![goal_idx];
            let mut cur = goal_idx;
            while let Some(&prev) = came_from.get(&cur) {
                path_tris.push(prev);
                cur = prev;
            }
            path_tris.reverse();
            let waypoints: Vec<Vec3> = path_tris.iter().map(|&i| mesh.triangles[i].center(&mesh.vertices)).collect();
            let mut wps = vec![start];
            wps.extend_from_slice(&waypoints[1..]);
            wps.push(goal);
            return NavPath::from_waypoints(wps);
        }
        closed.insert(current.tri_idx);
        let neighbors: Vec<usize> = mesh.triangles[current.tri_idx].neighbors.iter()
            .filter_map(|&n| n).collect();
        for nb_idx in neighbors {
            if closed.contains(&nb_idx) { continue; }
            let tri = &mesh.triangles[nb_idx];
            let area_cost = mesh.area_types.get(tri.area_type as usize).map(|a| a.cost).unwrap_or(1.0);
            if area_cost.is_infinite() { continue; }
            let nb_center = tri.center(&mesh.vertices);
            let cur_center = mesh.triangles[current.tri_idx].center(&mesh.vertices);
            let g = g_scores.get(&current.tri_idx).copied().unwrap_or(f32::INFINITY)
                + cur_center.distance(nb_center) * area_cost;
            if g < g_scores.get(&nb_idx).copied().unwrap_or(f32::INFINITY) {
                g_scores.insert(nb_idx, g);
                came_from.insert(nb_idx, current.tri_idx);
                let h = nb_center.distance(goal_center);
                open.push(AStarNode { tri_idx: nb_idx, g_cost: g, f_cost: g + h, parent: Some(current.tri_idx) });
            }
        }
    }
    NavPath::empty()
}

// ---------------------------------------------------------------------------
// NavMesh agent settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NavMeshAgentSettings {
    pub name: String,
    pub radius: f32,
    pub height: f32,
    pub step_height: f32,
    pub max_slope: f32,
    pub speed: f32,
    pub angular_speed: f32,
    pub acceleration: f32,
    pub stopping_distance: f32,
    pub auto_traverse_off_mesh: bool,
    pub auto_repath: bool,
    pub auto_brake: bool,
    pub obstacle_avoidance: ObstacleAvoidanceQuality,
    pub avoidance_priority: i32,
    pub area_mask: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObstacleAvoidanceQuality { None, LowQuality, MedQuality, GoodQuality, HighQuality }

impl Default for NavMeshAgentSettings {
    fn default() -> Self {
        Self {
            name: "Humanoid".into(),
            radius: 0.4,
            height: 1.8,
            step_height: 0.4,
            max_slope: 45.0,
            speed: 3.5,
            angular_speed: 120.0,
            acceleration: 8.0,
            stopping_distance: 0.0,
            auto_traverse_off_mesh: true,
            auto_repath: true,
            auto_brake: true,
            obstacle_avoidance: ObstacleAvoidanceQuality::HighQuality,
            avoidance_priority: 50,
            area_mask: 0xFFFF_FFFF,
        }
    }
}

// ---------------------------------------------------------------------------
// NavMesh build settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NavMeshBuildSettings {
    pub cell_size: f32,
    pub cell_height: f32,
    pub min_region_area: f32,
    pub merge_region_area: f32,
    pub edge_max_length: f32,
    pub edge_max_error: f32,
    pub verts_per_poly: u32,
    pub detail_sample_distance: f32,
    pub detail_sample_max_error: f32,
    pub walkable_slope: f32,
    pub walkable_height: f32,
    pub walkable_radius: f32,
    pub walkable_climb: f32,
    pub filter_low_hanging: bool,
    pub filter_ledge_spans: bool,
    pub filter_walkable_low: bool,
    pub include_static_objects: bool,
    pub include_dynamic_objects: bool,
}

impl Default for NavMeshBuildSettings {
    fn default() -> Self {
        Self {
            cell_size: 0.167,
            cell_height: 0.1,
            min_region_area: 2.0,
            merge_region_area: 400.0,
            edge_max_length: 12.0,
            edge_max_error: 1.3,
            verts_per_poly: 6,
            detail_sample_distance: 6.0,
            detail_sample_max_error: 1.0,
            walkable_slope: 45.0,
            walkable_height: 2.0,
            walkable_radius: 0.4,
            walkable_climb: 0.5,
            filter_low_hanging: true,
            filter_ledge_spans: true,
            filter_walkable_low: true,
            include_static_objects: true,
            include_dynamic_objects: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Off-mesh connections
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OffMeshConnectionType { Jump, Drop, Ladder, Teleport, Custom }

#[derive(Debug, Clone)]
pub struct OffMeshConnection {
    pub id: u32,
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f32,
    pub bidirectional: bool,
    pub connection_type: OffMeshConnectionType,
    pub cost: f32,
    pub area_mask: u32,
    pub activated: bool,
}

impl OffMeshConnection {
    pub fn new_jump(id: u32, start: Vec3, end: Vec3) -> Self {
        Self {
            id, start, end, radius: 0.5, bidirectional: false,
            connection_type: OffMeshConnectionType::Jump, cost: 1.0,
            area_mask: 0xFFFF_FFFF, activated: true,
        }
    }
}

// ---------------------------------------------------------------------------
// NavMesh editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavMeshEditorTab { Build, Areas, Agents, OffMeshLinks, Visualization, Debug }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavMeshVisualizationMode { None, Triangles, Walkable, Areas, Regions, Portals }

#[derive(Debug, Clone)]
pub struct NavMeshEditor {
    pub mesh: Option<NavMesh>,
    pub build_settings: NavMeshBuildSettings,
    pub agent_settings: Vec<NavMeshAgentSettings>,
    pub off_mesh_connections: Vec<OffMeshConnection>,
    pub active_tab: NavMeshEditorTab,
    pub vis_mode: NavMeshVisualizationMode,
    pub show_agent_cylinder: bool,
    pub selected_area_type: u8,
    pub build_progress: f32,
    pub is_building: bool,
    pub last_build_time_ms: f32,
    pub debug_path: Option<NavPath>,
    pub debug_path_start: Vec3,
    pub debug_path_goal: Vec3,
    pub show_debug_path: bool,
    pub next_connection_id: u32,
}

impl NavMeshEditor {
    pub fn new() -> Self {
        let mut ed = Self {
            mesh: None,
            build_settings: NavMeshBuildSettings::default(),
            agent_settings: vec![NavMeshAgentSettings::default()],
            off_mesh_connections: Vec::new(),
            active_tab: NavMeshEditorTab::Build,
            vis_mode: NavMeshVisualizationMode::Triangles,
            show_agent_cylinder: true,
            selected_area_type: 0,
            build_progress: 0.0,
            is_building: false,
            last_build_time_ms: 0.0,
            debug_path: None,
            debug_path_start: Vec3::new(-5.0, 0.0, -5.0),
            debug_path_goal: Vec3::new(5.0, 0.0, 5.0),
            show_debug_path: false,
            next_connection_id: 1,
        };
        // Build a default flat mesh
        ed.mesh = Some(NavMesh::build_flat(20.0, 10));
        ed
    }

    pub fn start_build(&mut self) {
        self.is_building = true;
        self.build_progress = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        if self.is_building {
            self.build_progress += dt * 0.5;
            if self.build_progress >= 1.0 {
                self.build_progress = 1.0;
                self.is_building = false;
                self.last_build_time_ms = 1000.0 / 0.5;
                // Build a synthetic nav mesh
                self.mesh = Some(NavMesh::build_flat(20.0, 20));
            }
        }
    }

    pub fn compute_debug_path(&mut self) {
        if let Some(mesh) = &self.mesh {
            let path = find_path(mesh, self.debug_path_start, self.debug_path_goal);
            self.debug_path = Some(path);
        }
    }

    pub fn add_off_mesh_connection(&mut self, conn: OffMeshConnection) {
        self.next_connection_id += 1;
        self.off_mesh_connections.push(conn);
    }

    pub fn remove_off_mesh_connection(&mut self, id: u32) {
        self.off_mesh_connections.retain(|c| c.id != id);
    }

    pub fn triangle_count(&self) -> usize {
        self.mesh.as_ref().map(|m| m.triangle_count()).unwrap_or(0)
    }

    pub fn vertex_count(&self) -> usize {
        self.mesh.as_ref().map(|m| m.vertex_count()).unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_navmesh() {
        let nm = NavMesh::build_flat(10.0, 5);
        assert!(!nm.vertices.is_empty());
        assert!(!nm.triangles.is_empty());
    }

    #[test]
    fn test_point_in_triangle() {
        let verts = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
        ];
        let tri = NavTriangle { vertices: [0, 1, 2], neighbors: [None; 3], area_type: 0, flags: 0 };
        assert!(tri.contains_point_2d(&verts, Vec2::new(0.5, 0.5)));
        assert!(!tri.contains_point_2d(&verts, Vec2::new(5.0, 5.0)));
    }

    #[test]
    fn test_nav_path_interpolation() {
        let path = NavPath::from_waypoints(vec![Vec3::ZERO, Vec3::X * 10.0]);
        let mid = path.interpolate(0.5).unwrap();
        assert!((mid.x - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_editor() {
        let mut ed = NavMeshEditor::new();
        ed.start_build();
        ed.update(3.0);
        assert!(!ed.is_building);
        assert!(ed.triangle_count() > 0);
    }
}
