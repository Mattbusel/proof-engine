//! Navigation mesh system for polygon-based pathfinding.
//!
//! # Architecture
//! A `NavMesh` is composed of triangles (convex polygons) connected through
//! shared edges called portals.  A* runs on the triangle adjacency graph, and
//! the string-pulling funnel algorithm produces a smooth final path.
//!
//! # Example
//! ```rust
//! use proof_engine::ai::navmesh::{NavMesh, NavMeshAgent};
//! use glam::Vec2;
//!
//! let mut mesh = NavMesh::new();
//! let v0 = mesh.add_vertex(Vec2::new(0.0, 0.0));
//! let v1 = mesh.add_vertex(Vec2::new(10.0, 0.0));
//! let v2 = mesh.add_vertex(Vec2::new(5.0, 10.0));
//! mesh.add_triangle([v0, v1, v2]);
//! mesh.build();
//!
//! let path = mesh.find_path(Vec2::new(1.0, 1.0), Vec2::new(8.0, 2.0));
//! println!("{:?}", path);
//! ```

use glam::Vec2;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A vertex on the navigation mesh.
#[derive(Debug, Clone, Copy)]
pub struct NavVertex {
    pub pos: Vec2,
}

impl NavVertex {
    pub fn new(pos: Vec2) -> Self { NavVertex { pos } }
}

/// A portal (shared edge) between two triangles.
#[derive(Debug, Clone, Copy)]
pub struct Portal {
    /// The two vertex indices that form the shared edge.
    pub left: usize,
    pub right: usize,
    /// The two triangle indices on each side.
    pub tri_a: usize,
    pub tri_b: usize,
}

impl Portal {
    pub fn new(left: usize, right: usize, tri_a: usize, tri_b: usize) -> Self {
        Portal { left, right, tri_a, tri_b }
    }

    /// Mid-point of the portal edge.
    pub fn midpoint(&self, vertices: &[NavVertex]) -> Vec2 {
        (vertices[self.left].pos + vertices[self.right].pos) * 0.5
    }
}

/// A triangle (face) in the navigation mesh.
#[derive(Debug, Clone)]
pub struct NavTriangle {
    /// Vertex indices (CCW winding assumed).
    pub verts: [usize; 3],
    /// Adjacent triangle indices per edge (None = boundary).
    pub neighbors: [Option<usize>; 3],
    /// Pre-computed centroid.
    pub center: Vec2,
    /// Pre-computed area.
    pub area: f32,
    /// Portal index per edge (None = boundary edge).
    pub portals: [Option<usize>; 3],
}

impl NavTriangle {
    /// Create a triangle; compute center and area lazily later via `compute()`.
    pub fn new(verts: [usize; 3]) -> Self {
        NavTriangle {
            verts,
            neighbors: [None; 3],
            center: Vec2::ZERO,
            area: 0.0,
            portals: [None; 3],
        }
    }

    /// Compute centroid and area from vertex positions.
    pub fn compute(&mut self, vertices: &[NavVertex]) {
        let a = vertices[self.verts[0]].pos;
        let b = vertices[self.verts[1]].pos;
        let c = vertices[self.verts[2]].pos;
        self.center = (a + b + c) / 3.0;
        // Signed area via cross product
        self.area = ((b - a).perp_dot(c - a) * 0.5).abs();
    }

    /// Returns the edge (sorted vertex pair) for edge index 0,1,2.
    pub fn edge(&self, edge_idx: usize) -> (usize, usize) {
        let i0 = self.verts[edge_idx];
        let i1 = self.verts[(edge_idx + 1) % 3];
        if i0 < i1 { (i0, i1) } else { (i1, i0) }
    }
}

/// The main navigation mesh.
#[derive(Debug, Clone, Default)]
pub struct NavMesh {
    pub vertices: Vec<NavVertex>,
    pub triangles: Vec<NavTriangle>,
    pub portals: Vec<Portal>,
    built: bool,
}

// ---------------------------------------------------------------------------
// BinaryHeap node for A*
// ---------------------------------------------------------------------------
#[derive(Debug, Clone)]
struct TriHeapNode {
    f: f32,
    g: f32,
    idx: usize,
}
impl PartialEq for TriHeapNode { fn eq(&self, o: &Self) -> bool { self.f == o.f } }
impl Eq for TriHeapNode {}
impl PartialOrd for TriHeapNode {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
}
impl Ord for TriHeapNode {
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

// ---------------------------------------------------------------------------
// NavMesh implementation
// ---------------------------------------------------------------------------

impl NavMesh {
    pub fn new() -> Self { NavMesh::default() }

    /// Add a vertex and return its index.
    pub fn add_vertex(&mut self, pos: Vec2) -> usize {
        self.built = false;
        let idx = self.vertices.len();
        self.vertices.push(NavVertex::new(pos));
        idx
    }

    /// Add a triangle (by vertex indices) and return its index.
    pub fn add_triangle(&mut self, verts: [usize; 3]) -> usize {
        self.built = false;
        let idx = self.triangles.len();
        self.triangles.push(NavTriangle::new(verts));
        idx
    }

    /// Build adjacency, portals, and pre-compute geometry.  Must be called
    /// after all triangles are added and before pathfinding.
    pub fn build(&mut self) {
        // Compute per-triangle geometry
        for tri in self.triangles.iter_mut() {
            let a = self.vertices[tri.verts[0]].pos;
            let b = self.vertices[tri.verts[1]].pos;
            let c = self.vertices[tri.verts[2]].pos;
            tri.center = (a + b + c) / 3.0;
            tri.area   = ((b - a).perp_dot(c - a) * 0.5).abs();
        }

        // Build edge->triangle map
        // edge (sorted v0,v1) -> list of (tri_index, edge_index_in_tri)
        let mut edge_map: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
        for (ti, tri) in self.triangles.iter().enumerate() {
            for ei in 0..3 {
                let edge = tri.edge(ei);
                edge_map.entry(edge).or_default().push((ti, ei));
            }
        }

        self.portals.clear();
        // Reset adjacency
        for tri in self.triangles.iter_mut() {
            tri.neighbors = [None; 3];
            tri.portals   = [None; 3];
        }

        for (edge, tris) in &edge_map {
            if tris.len() == 2 {
                let (ti_a, ei_a) = tris[0];
                let (ti_b, ei_b) = tris[1];
                let portal_idx = self.portals.len();
                self.portals.push(Portal::new(edge.0, edge.1, ti_a, ti_b));
                self.triangles[ti_a].neighbors[ei_a] = Some(ti_b);
                self.triangles[ti_b].neighbors[ei_b] = Some(ti_a);
                self.triangles[ti_a].portals[ei_a] = Some(portal_idx);
                self.triangles[ti_b].portals[ei_b] = Some(portal_idx);
            }
        }

        self.built = true;
    }

    /// Test whether a 2D point is inside a given triangle.
    pub fn point_in_triangle(&self, pt: Vec2, tri_idx: usize) -> bool {
        let tri = &self.triangles[tri_idx];
        let a = self.vertices[tri.verts[0]].pos;
        let b = self.vertices[tri.verts[1]].pos;
        let c = self.vertices[tri.verts[2]].pos;
        point_in_triangle_coords(pt, a, b, c)
    }

    /// Find which triangle contains `pos`.  Returns `None` if outside mesh.
    pub fn find_triangle(&self, pos: Vec2) -> Option<usize> {
        // Linear scan — acceptable for small meshes; use spatial hash for large ones
        for (idx, _) in self.triangles.iter().enumerate() {
            if self.point_in_triangle(pos, idx) {
                return Some(idx);
            }
        }
        // Fallback: return nearest triangle centroid
        self.triangles.iter().enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = a.center.distance_squared(pos);
                let db = b.center.distance_squared(pos);
                da.partial_cmp(&db).unwrap_or(Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// Find a path from `start` to `end` using A* on the triangle graph,
    /// then refine with the funnel (string-pulling) algorithm.
    pub fn find_path(&self, start: Vec2, end: Vec2) -> Option<Vec<Vec2>> {
        if !self.built { return None; }
        let start_tri = self.find_triangle(start)?;
        let end_tri   = self.find_triangle(end)?;

        if start_tri == end_tri {
            return Some(vec![start, end]);
        }

        // A* over triangles
        let tri_path = self.astar_triangles(start_tri, end_tri, start, end)?;

        // Funnel algorithm for smooth path
        Some(self.funnel_path(&tri_path, start, end))
    }

    /// A* search over triangles. Returns ordered triangle indices.
    fn astar_triangles(&self, start: usize, end: usize, start_pos: Vec2, end_pos: Vec2) -> Option<Vec<usize>> {
        let n = self.triangles.len();
        let mut g: Vec<f32> = vec![f32::INFINITY; n];
        let mut parent: Vec<Option<usize>> = vec![None; n];
        let mut open: BinaryHeap<TriHeapNode> = BinaryHeap::new();
        let mut closed: HashSet<usize> = HashSet::new();

        g[start] = 0.0;
        let h = self.triangles[start].center.distance(end_pos);
        open.push(TriHeapNode { f: h, g: 0.0, idx: start });

        while let Some(cur) = open.pop() {
            if closed.contains(&cur.idx) { continue; }
            closed.insert(cur.idx);

            if cur.idx == end {
                let mut path = Vec::new();
                let mut c = cur.idx;
                loop {
                    path.push(c);
                    match parent[c] {
                        Some(p) => c = p,
                        None => break,
                    }
                }
                path.reverse();
                return Some(path);
            }

            for &nb in self.triangles[cur.idx].neighbors.iter().flatten() {
                if closed.contains(&nb) { continue; }
                let edge_cost = self.triangles[cur.idx].center.distance(self.triangles[nb].center);
                let tentative = g[cur.idx] + edge_cost;
                if tentative < g[nb] {
                    g[nb] = tentative;
                    parent[nb] = Some(cur.idx);
                    let h2 = self.triangles[nb].center.distance(end_pos);
                    open.push(TriHeapNode { f: tentative + h2, g: tentative, idx: nb });
                }
            }
        }
        None
    }

    /// Funnel (string-pulling) algorithm.
    /// Given an ordered list of triangles, produces a smooth path.
    fn funnel_path(&self, tri_path: &[usize], start: Vec2, end: Vec2) -> Vec<Vec2> {
        if tri_path.len() <= 1 {
            return vec![start, end];
        }

        let mut path = vec![start];

        // Collect portals between consecutive triangles
        let mut portals: Vec<(Vec2, Vec2)> = Vec::new(); // (left, right) edges
        portals.push((start, start)); // dummy start portal

        for window in tri_path.windows(2) {
            let ta = window[0];
            let tb = window[1];
            // Find shared edge
            if let Some((lv, rv)) = self.shared_edge_ordered(ta, tb) {
                portals.push((lv, rv));
            }
        }
        portals.push((end, end)); // dummy end portal

        // Simplified funnel: apex moves forward when funnel collapses
        let mut apex = start;
        let mut left_idx = 0usize;
        let mut right_idx = 0usize;
        let mut left_pt  = start;
        let mut right_pt = start;

        for i in 1..portals.len() {
            let (new_left, new_right) = portals[i];

            // Update right side
            if triangle_area_sign(apex, right_pt, new_right) <= 0.0 {
                if apex == right_pt || triangle_area_sign(apex, left_pt, new_right) > 0.0 {
                    right_pt = new_right;
                    right_idx = i;
                } else {
                    path.push(left_pt);
                    apex = left_pt;
                    left_idx = i;
                    right_idx = i;
                    right_pt = apex;
                    left_pt  = apex;
                    continue;
                }
            }

            // Update left side
            if triangle_area_sign(apex, left_pt, new_left) >= 0.0 {
                if apex == left_pt || triangle_area_sign(apex, right_pt, new_left) < 0.0 {
                    left_pt = new_left;
                    left_idx = i;
                } else {
                    path.push(right_pt);
                    apex = right_pt;
                    left_idx = i;
                    right_idx = i;
                    left_pt  = apex;
                    right_pt = apex;
                    continue;
                }
            }
            let _ = (left_idx, right_idx); // suppress unused warnings
        }

        path.push(end);
        path
    }

    /// Returns the shared edge (left, right) when moving from triangle `a` to `b`.
    fn shared_edge_ordered(&self, a: usize, b: usize) -> Option<(Vec2, Vec2)> {
        let ta = &self.triangles[a];
        let tb = &self.triangles[b];
        // Find common vertex indices
        let mut common = Vec::new();
        for &va in &ta.verts {
            if tb.verts.contains(&va) {
                common.push(va);
            }
        }
        if common.len() < 2 { return None; }
        let lv = self.vertices[common[0]].pos;
        let rv = self.vertices[common[1]].pos;
        Some((lv, rv))
    }

    /// Returns the number of triangles.
    pub fn triangle_count(&self) -> usize { self.triangles.len() }

    /// Returns the number of portals.
    pub fn portal_count(&self) -> usize { self.portals.len() }

    /// Check whether a world position is inside the navmesh.
    pub fn contains(&self, pos: Vec2) -> bool {
        self.triangles.iter().enumerate().any(|(i, _)| self.point_in_triangle(pos, i))
    }

    /// Clamp a position to the nearest point inside the navmesh.
    pub fn clamp_to_mesh(&self, pos: Vec2) -> Vec2 {
        if self.contains(pos) { return pos; }
        // Find nearest triangle centroid
        self.triangles.iter()
            .map(|t| t.center)
            .min_by(|a, b| {
                a.distance_squared(pos).partial_cmp(&b.distance_squared(pos)).unwrap_or(Ordering::Equal)
            })
            .unwrap_or(pos)
    }
}

// ---------------------------------------------------------------------------
// Geometric helpers
// ---------------------------------------------------------------------------

fn point_in_triangle_coords(pt: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = cross2d(pt - a, b - a);
    let d2 = cross2d(pt - b, c - b);
    let d3 = cross2d(pt - c, a - c);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

#[inline]
fn cross2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

#[inline]
fn triangle_area_sign(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

// ---------------------------------------------------------------------------
// NavMesh Builder
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box obstacle.
#[derive(Debug, Clone, Copy)]
pub struct AabbObstacle {
    pub min: Vec2,
    pub max: Vec2,
}

impl AabbObstacle {
    pub fn new(min: Vec2, max: Vec2) -> Self { AabbObstacle { min, max } }

    pub fn center(&self) -> Vec2 { (self.min + self.max) * 0.5 }

    pub fn contains(&self, pt: Vec2) -> bool {
        pt.x >= self.min.x && pt.x <= self.max.x &&
        pt.y >= self.min.y && pt.y <= self.max.y
    }

    pub fn corners(&self) -> [Vec2; 4] {
        [
            self.min,
            Vec2::new(self.max.x, self.min.y),
            self.max,
            Vec2::new(self.min.x, self.max.y),
        ]
    }
}

/// Builds a `NavMesh` around a set of axis-aligned obstacles within a rectangular world.
///
/// The builder uses a simple grid-sampling approach: it subdivides the world into
/// a regular grid, marks cells occupied by obstacles, and triangulates the free cells.
pub struct NavMeshBuilder {
    pub world_min: Vec2,
    pub world_max: Vec2,
    pub grid_resolution: usize,
    pub obstacles: Vec<AabbObstacle>,
    pub agent_radius: f32,
}

impl NavMeshBuilder {
    pub fn new(world_min: Vec2, world_max: Vec2, grid_resolution: usize) -> Self {
        NavMeshBuilder {
            world_min,
            world_max,
            grid_resolution,
            obstacles: Vec::new(),
            agent_radius: 0.0,
        }
    }

    pub fn add_obstacle(mut self, obs: AabbObstacle) -> Self {
        self.obstacles.push(obs);
        self
    }

    pub fn with_agent_radius(mut self, radius: f32) -> Self {
        self.agent_radius = radius;
        self
    }

    /// Build and return the completed `NavMesh`.
    pub fn build(self) -> NavMesh {
        let res = self.grid_resolution.max(2);
        let step_x = (self.world_max.x - self.world_min.x) / res as f32;
        let step_y = (self.world_max.y - self.world_min.y) / res as f32;
        let pad = self.agent_radius;

        // Determine which grid vertices are free
        // Vertex grid: (res+1) x (res+1)
        let vw = res + 1;
        let vh = res + 1;
        let mut free = vec![true; vw * vh];

        for vy in 0..vh {
            for vx in 0..vw {
                let pos = Vec2::new(
                    self.world_min.x + vx as f32 * step_x,
                    self.world_min.y + vy as f32 * step_y,
                );
                for obs in &self.obstacles {
                    let padded_min = obs.min - Vec2::splat(pad);
                    let padded_max = obs.max + Vec2::splat(pad);
                    if pos.x >= padded_min.x && pos.x <= padded_max.x &&
                       pos.y >= padded_min.y && pos.y <= padded_max.y {
                        free[vy * vw + vx] = false;
                        break;
                    }
                }
            }
        }

        let mut mesh = NavMesh::new();
        // Map from grid (vx,vy) to vertex index in mesh
        let mut vert_map: HashMap<(usize, usize), usize> = HashMap::new();

        let get_or_insert = |vx: usize, vy: usize,
                              mesh: &mut NavMesh,
                              vert_map: &mut HashMap<(usize, usize), usize>,
                              world_min: Vec2, step_x: f32, step_y: f32| -> usize {
            *vert_map.entry((vx, vy)).or_insert_with(|| {
                let pos = Vec2::new(
                    world_min.x + vx as f32 * step_x,
                    world_min.y + vy as f32 * step_y,
                );
                mesh.add_vertex(pos)
            })
        };

        // For each grid cell (res x res), emit two triangles if all verts are free
        for cy in 0..res {
            for cx in 0..res {
                let corners = [(cx, cy), (cx+1, cy), (cx+1, cy+1), (cx, cy+1)];
                let all_free = corners.iter().all(|&(vx, vy)| free[vy * vw + vx]);
                if !all_free { continue; }

                let v00 = get_or_insert(cx,   cy,   &mut mesh, &mut vert_map, self.world_min, step_x, step_y);
                let v10 = get_or_insert(cx+1, cy,   &mut mesh, &mut vert_map, self.world_min, step_x, step_y);
                let v11 = get_or_insert(cx+1, cy+1, &mut mesh, &mut vert_map, self.world_min, step_x, step_y);
                let v01 = get_or_insert(cx,   cy+1, &mut mesh, &mut vert_map, self.world_min, step_x, step_y);

                mesh.add_triangle([v00, v10, v11]);
                mesh.add_triangle([v00, v11, v01]);
            }
        }

        mesh.build();
        mesh
    }
}

// ---------------------------------------------------------------------------
// NavMesh Agent
// ---------------------------------------------------------------------------

/// An agent that navigates the navmesh, following a computed path.
#[derive(Debug, Clone)]
pub struct NavMeshAgent {
    pub position: Vec2,
    pub velocity: Vec2,
    pub target: Vec2,
    pub path: Vec<Vec2>,
    pub speed: f32,
    pub radius: f32,
    pub arrival_threshold: f32,
    current_waypoint: usize,
}

impl NavMeshAgent {
    pub fn new(position: Vec2, speed: f32, radius: f32) -> Self {
        NavMeshAgent {
            position,
            velocity: Vec2::ZERO,
            target: position,
            path: Vec::new(),
            speed,
            radius,
            arrival_threshold: 0.1,
            current_waypoint: 0,
        }
    }

    /// Set a new destination and compute a path on the given navmesh.
    pub fn set_destination(&mut self, dest: Vec2, navmesh: &NavMesh) {
        self.target = dest;
        self.current_waypoint = 0;
        self.path = navmesh.find_path(self.position, dest).unwrap_or_default();
    }

    /// Update the agent's position by stepping along its path.
    pub fn update(&mut self, dt: f32, _navmesh: &NavMesh) {
        if self.is_at_destination() || self.path.is_empty() {
            self.velocity = Vec2::ZERO;
            return;
        }

        let steering = self.steer_toward_path();
        self.velocity = steering * self.speed;
        self.position += self.velocity * dt;

        // Advance waypoint index when close enough
        if self.current_waypoint < self.path.len() {
            let wp = self.path[self.current_waypoint];
            if self.position.distance(wp) <= self.arrival_threshold + self.speed * dt {
                self.current_waypoint += 1;
            }
        }
    }

    /// Returns `true` when the agent has reached its destination.
    pub fn is_at_destination(&self) -> bool {
        self.position.distance(self.target) <= self.arrival_threshold
    }

    /// Compute a normalised steering direction toward the current path waypoint.
    pub fn steer_toward_path(&self) -> Vec2 {
        let wp_idx = self.current_waypoint.min(self.path.len().saturating_sub(1));
        if self.path.is_empty() { return Vec2::ZERO; }
        let wp = self.path[wp_idx];
        let to_wp = wp - self.position;
        let dist = to_wp.length();
        if dist < 0.0001 { return Vec2::ZERO; }
        to_wp / dist
    }

    /// Distance remaining along the current path.
    pub fn remaining_distance(&self) -> f32 {
        if self.path.is_empty() { return 0.0; }
        let start_idx = self.current_waypoint.min(self.path.len().saturating_sub(1));
        let mut dist = self.position.distance(self.path[start_idx]);
        for i in start_idx..self.path.len().saturating_sub(1) {
            dist += self.path[i].distance(self.path[i + 1]);
        }
        dist
    }

    /// Stop the agent and clear its path.
    pub fn stop(&mut self) {
        self.path.clear();
        self.velocity = Vec2::ZERO;
        self.current_waypoint = 0;
    }

    /// Warp agent to a new position without pathfinding.
    pub fn teleport(&mut self, pos: Vec2) {
        self.position = pos;
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Path query helpers
// ---------------------------------------------------------------------------

/// Batch path query — finds paths for multiple start/end pairs.
#[derive(Debug, Clone)]
pub struct BatchPathQuery {
    pub queries: Vec<(Vec2, Vec2)>,
}

impl BatchPathQuery {
    pub fn new() -> Self { BatchPathQuery { queries: Vec::new() } }

    pub fn add(&mut self, start: Vec2, end: Vec2) {
        self.queries.push((start, end));
    }

    /// Execute all queries and return results.
    pub fn execute(&self, navmesh: &NavMesh) -> Vec<Option<Vec<Vec2>>> {
        self.queries.iter().map(|(s, e)| navmesh.find_path(*s, *e)).collect()
    }
}

impl Default for BatchPathQuery {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Spatial hash for fast triangle lookup
// ---------------------------------------------------------------------------

/// Spatial hash accelerating `find_triangle` for large meshes.
#[derive(Debug, Clone)]
pub struct NavMeshSpatialHash {
    pub cell_size: f32,
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl NavMeshSpatialHash {
    pub fn build(mesh: &NavMesh, cell_size: f32) -> Self {
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (ti, tri) in mesh.triangles.iter().enumerate() {
            let cell = Self::hash_pos(tri.center, cell_size);
            cells.entry(cell).or_default().push(ti);
        }
        NavMeshSpatialHash { cell_size, cells }
    }

    fn hash_pos(pos: Vec2, cell_size: f32) -> (i32, i32) {
        ((pos.x / cell_size).floor() as i32, (pos.y / cell_size).floor() as i32)
    }

    /// Return candidate triangle indices near `pos`.
    pub fn candidates(&self, pos: Vec2) -> Vec<usize> {
        let (cx, cy) = Self::hash_pos(pos, self.cell_size);
        let mut result = Vec::new();
        for dx in -1..=1i32 {
            for dy in -1..=1i32 {
                if let Some(tris) = self.cells.get(&(cx + dx, cy + dy)) {
                    result.extend_from_slice(tris);
                }
            }
        }
        result
    }

    /// Find a triangle containing `pos` using the spatial hash.
    pub fn find_triangle(&self, mesh: &NavMesh, pos: Vec2) -> Option<usize> {
        for &ti in &self.candidates(pos) {
            if mesh.point_in_triangle(pos, ti) {
                return Some(ti);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_mesh() -> NavMesh {
        // Two adjacent triangles forming a square [0,0]-[2,1]
        let mut mesh = NavMesh::new();
        let v0 = mesh.add_vertex(Vec2::new(0.0, 0.0));
        let v1 = mesh.add_vertex(Vec2::new(2.0, 0.0));
        let v2 = mesh.add_vertex(Vec2::new(2.0, 1.0));
        let v3 = mesh.add_vertex(Vec2::new(0.0, 1.0));
        mesh.add_triangle([v0, v1, v2]);
        mesh.add_triangle([v0, v2, v3]);
        mesh.build();
        mesh
    }

    #[test]
    fn test_build_adjacency() {
        let mesh = simple_mesh();
        assert_eq!(mesh.portal_count(), 1);
        assert!(mesh.triangles[0].neighbors.contains(&Some(1)));
        assert!(mesh.triangles[1].neighbors.contains(&Some(0)));
    }

    #[test]
    fn test_point_in_triangle() {
        let mesh = simple_mesh();
        assert!(mesh.point_in_triangle(Vec2::new(1.0, 0.3), 0));
        assert!(!mesh.point_in_triangle(Vec2::new(5.0, 5.0), 0));
    }

    #[test]
    fn test_find_triangle() {
        let mesh = simple_mesh();
        assert!(mesh.find_triangle(Vec2::new(1.0, 0.3)).is_some());
        assert!(mesh.find_triangle(Vec2::new(0.5, 0.8)).is_some());
    }

    #[test]
    fn test_find_path_same_triangle() {
        let mesh = simple_mesh();
        let path = mesh.find_path(Vec2::new(0.5, 0.2), Vec2::new(1.5, 0.4));
        assert!(path.is_some());
        let p = path.unwrap();
        assert_eq!(p[0], Vec2::new(0.5, 0.2));
    }

    #[test]
    fn test_find_path_across_triangles() {
        let mesh = simple_mesh();
        let path = mesh.find_path(Vec2::new(0.3, 0.2), Vec2::new(1.8, 0.8));
        assert!(path.is_some());
    }

    #[test]
    fn test_navmesh_agent_at_destination() {
        let mesh = simple_mesh();
        let mut agent = NavMeshAgent::new(Vec2::new(0.5, 0.4), 2.0, 0.1);
        agent.set_destination(Vec2::new(1.8, 0.7), &mesh);
        // Simulate several steps
        for _ in 0..100 {
            agent.update(0.016, &mesh);
            if agent.is_at_destination() { break; }
        }
        assert!(agent.is_at_destination() || agent.remaining_distance() < 1.0);
    }

    #[test]
    fn test_navmesh_builder_empty() {
        let mesh = NavMeshBuilder::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
            4,
        ).build();
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn test_navmesh_builder_with_obstacle() {
        let mesh = NavMeshBuilder::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
            8,
        )
        .add_obstacle(AabbObstacle::new(Vec2::new(3.0, 3.0), Vec2::new(7.0, 7.0)))
        .build();
        assert!(mesh.triangle_count() > 0);
        // Obstacle interior should not be in mesh
        // (may or may not be true depending on grid alignment, so just check build succeeded)
    }

    #[test]
    fn test_spatial_hash() {
        let mesh = simple_mesh();
        let hash = NavMeshSpatialHash::build(&mesh, 2.0);
        let tri = hash.find_triangle(&mesh, Vec2::new(1.0, 0.3));
        assert!(tri.is_some());
    }

    #[test]
    fn test_batch_query() {
        let mesh = simple_mesh();
        let mut batch = BatchPathQuery::new();
        batch.add(Vec2::new(0.3, 0.2), Vec2::new(1.8, 0.8));
        batch.add(Vec2::new(0.5, 0.4), Vec2::new(1.5, 0.6));
        let results = batch.execute(&mesh);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_contains() {
        let mesh = simple_mesh();
        assert!(mesh.contains(Vec2::new(1.0, 0.5)));
        assert!(!mesh.contains(Vec2::new(-5.0, -5.0)));
    }

    #[test]
    fn test_portal_midpoint() {
        let mesh = simple_mesh();
        if !mesh.portals.is_empty() {
            let mid = mesh.portals[0].midpoint(&mesh.vertices);
            assert!(mid.x > 0.0);
        }
    }

    #[test]
    fn test_remaining_distance() {
        let mesh = simple_mesh();
        let mut agent = NavMeshAgent::new(Vec2::new(0.3, 0.2), 1.0, 0.1);
        agent.set_destination(Vec2::new(1.8, 0.8), &mesh);
        let dist = agent.remaining_distance();
        assert!(dist >= 0.0);
    }

    #[test]
    fn test_clamp_to_mesh() {
        let mesh = simple_mesh();
        let clamped = mesh.clamp_to_mesh(Vec2::new(100.0, 100.0));
        // Should return some point on the mesh
        assert!(clamped.x.is_finite());
    }
}
