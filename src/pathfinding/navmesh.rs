// src/pathfinding/navmesh.rs
// Navigation mesh implementation with convex polygon graph, portal edges,
// string-pulling path smoothing, dynamic obstacle cutting, and area cost modifiers.

use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Ordering;
use std::f32;

// ── Basic geometry types ─────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    #[inline] pub fn new(x: f32, y: f32) -> Self { Self { x, y } }
    #[inline] pub fn zero() -> Self { Self { x: 0.0, y: 0.0 } }
    #[inline] pub fn dot(self, o: Self) -> f32 { self.x * o.x + self.y * o.y }
    #[inline] pub fn cross(self, o: Self) -> f32 { self.x * o.y - self.y * o.x }
    #[inline] pub fn len_sq(self) -> f32 { self.dot(self) }
    #[inline] pub fn len(self) -> f32 { self.len_sq().sqrt() }
    #[inline] pub fn norm(self) -> Self {
        let l = self.len();
        if l < 1e-9 { Self::zero() } else { Self::new(self.x / l, self.y / l) }
    }
    #[inline] pub fn sub(self, o: Self) -> Self { Self::new(self.x - o.x, self.y - o.y) }
    #[inline] pub fn add(self, o: Self) -> Self { Self::new(self.x + o.x, self.y + o.y) }
    #[inline] pub fn scale(self, s: f32) -> Self { Self::new(self.x * s, self.y * s) }
    #[inline] pub fn lerp(self, o: Self, t: f32) -> Self {
        Self::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    #[inline] pub fn dist(self, o: Self) -> f32 { self.sub(o).len() }
    #[inline] pub fn dist_sq(self, o: Self) -> f32 { self.sub(o).len_sq() }
    #[inline] pub fn perp(self) -> Self { Self::new(-self.y, self.x) }
}

// ── Area flags and cost ──────────────────────────────────────────────────────

/// Bit flags for polygon area types (walkable, water, etc.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AreaFlags(pub u32);

impl AreaFlags {
    pub const WALKABLE: Self  = Self(1 << 0);
    pub const WATER:    Self  = Self(1 << 1);
    pub const ROAD:     Self  = Self(1 << 2);
    pub const GRASS:    Self  = Self(1 << 3);
    pub const HAZARD:   Self  = Self(1 << 4);
    pub const BLOCKED:  Self  = Self(1 << 5);
    pub const ALL:      Self  = Self(u32::MAX);
    pub const NONE:     Self  = Self(0);

    #[inline] pub fn contains(self, other: Self) -> bool { (self.0 & other.0) == other.0 }
    #[inline] pub fn union(self, other: Self) -> Self { Self(self.0 | other.0) }
    #[inline] pub fn intersect(self, other: Self) -> Self { Self(self.0 & other.0) }
}

/// Per-area traversal cost modifier. Default 1.0 = normal speed.
#[derive(Clone, Debug)]
pub struct AreaCost {
    pub costs: HashMap<AreaFlags, f32>,
}

impl Default for AreaCost {
    fn default() -> Self {
        let mut costs = HashMap::new();
        costs.insert(AreaFlags::WALKABLE, 1.0);
        costs.insert(AreaFlags::WATER,    3.0);
        costs.insert(AreaFlags::ROAD,     0.8);
        costs.insert(AreaFlags::GRASS,    1.2);
        costs.insert(AreaFlags::HAZARD,   5.0);
        Self { costs }
    }
}

impl AreaCost {
    pub fn get(&self, flags: AreaFlags) -> f32 {
        for (k, v) in &self.costs {
            if flags.contains(*k) { return *v; }
        }
        1.0
    }
    pub fn set(&mut self, flags: AreaFlags, cost: f32) {
        self.costs.insert(flags, cost);
    }
}

// ── NavPoly ──────────────────────────────────────────────────────────────────

/// Unique identifier for a navigation polygon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NavPolyId(pub u32);

/// A convex polygon on the navigation mesh.
#[derive(Clone, Debug)]
pub struct NavPoly {
    pub id:       NavPolyId,
    pub verts:    Vec<Vec2>,      // vertices in CCW order
    pub centroid: Vec2,
    pub area:     AreaFlags,
    pub cost:     f32,            // base traversal cost
    // indices into NavMesh::portals
    pub portals:  Vec<usize>,
}

impl NavPoly {
    pub fn new(id: NavPolyId, verts: Vec<Vec2>, area: AreaFlags, cost: f32) -> Self {
        let centroid = Self::compute_centroid(&verts);
        Self { id, verts, centroid, area, cost, portals: Vec::new() }
    }

    fn compute_centroid(verts: &[Vec2]) -> Vec2 {
        if verts.is_empty() { return Vec2::zero(); }
        let sum = verts.iter().fold(Vec2::zero(), |a, &v| a.add(v));
        sum.scale(1.0 / verts.len() as f32)
    }

    /// Test whether a 2-D point lies inside this convex polygon (CCW winding).
    pub fn contains_point(&self, p: Vec2) -> bool {
        let n = self.verts.len();
        if n < 3 { return false; }
        for i in 0..n {
            let a = self.verts[i];
            let b = self.verts[(i + 1) % n];
            let ab = b.sub(a);
            let ap = p.sub(a);
            if ab.cross(ap) < 0.0 { return false; }
        }
        true
    }

    /// Closest point on the polygon boundary (or inside) to `p`.
    pub fn closest_point(&self, p: Vec2) -> Vec2 {
        if self.contains_point(p) { return p; }
        let n = self.verts.len();
        let mut best = self.verts[0];
        let mut best_dist = f32::MAX;
        for i in 0..n {
            let a = self.verts[i];
            let b = self.verts[(i + 1) % n];
            let c = closest_point_on_segment(a, b, p);
            let d = c.dist_sq(p);
            if d < best_dist { best_dist = d; best = c; }
        }
        best
    }

    /// Signed area (positive = CCW).
    pub fn signed_area(&self) -> f32 {
        let n = self.verts.len();
        let mut area = 0.0f32;
        for i in 0..n {
            let a = self.verts[i];
            let b = self.verts[(i + 1) % n];
            area += a.cross(b);
        }
        area * 0.5
    }
}

fn closest_point_on_segment(a: Vec2, b: Vec2, p: Vec2) -> Vec2 {
    let ab = b.sub(a);
    let ap = p.sub(a);
    let t = ap.dot(ab) / (ab.len_sq() + 1e-12);
    let t = t.clamp(0.0, 1.0);
    a.add(ab.scale(t))
}

// ── Portal edge ──────────────────────────────────────────────────────────────

/// A portal is the shared edge between two adjacent polygons.
#[derive(Clone, Debug)]
pub struct NavPortal {
    pub poly_a: NavPolyId,
    pub poly_b: NavPolyId,
    pub left:   Vec2,   // left endpoint of the portal edge
    pub right:  Vec2,   // right endpoint of the portal edge
}

impl NavPortal {
    pub fn midpoint(&self) -> Vec2 {
        self.left.lerp(self.right, 0.5)
    }
    pub fn width(&self) -> f32 {
        self.left.dist(self.right)
    }
    /// The "other" polygon given one side.
    pub fn other(&self, poly: NavPolyId) -> NavPolyId {
        if poly == self.poly_a { self.poly_b } else { self.poly_a }
    }
}

// ── NavMesh ──────────────────────────────────────────────────────────────────

/// The navigation mesh: a graph of convex polygons connected by portals.
#[derive(Clone, Debug, Default)]
pub struct NavMesh {
    pub polys:   Vec<NavPoly>,
    pub portals: Vec<NavPortal>,
    /// Lookup: poly id → index in polys vec
    poly_index:  HashMap<NavPolyId, usize>,
    next_id:     u32,
    pub area_cost: AreaCost,
}

impl NavMesh {
    pub fn new() -> Self { Self::default() }

    // ── Building ─────────────────────────────────────────────────────────────

    pub fn add_poly(&mut self, verts: Vec<Vec2>, area: AreaFlags, cost: f32) -> NavPolyId {
        let id = NavPolyId(self.next_id);
        self.next_id += 1;
        let idx = self.polys.len();
        self.polys.push(NavPoly::new(id, verts, area, cost));
        self.poly_index.insert(id, idx);
        id
    }

    /// Connect two polygons through a shared edge defined by endpoints.
    /// The edge is ordered so that `left` and `right` are from the perspective
    /// of standing in poly_a looking into poly_b.
    pub fn add_portal(&mut self, poly_a: NavPolyId, poly_b: NavPolyId, left: Vec2, right: Vec2) -> usize {
        let portal_idx = self.portals.len();
        self.portals.push(NavPortal { poly_a, poly_b, left, right });
        if let Some(&ia) = self.poly_index.get(&poly_a) {
            self.polys[ia].portals.push(portal_idx);
        }
        if let Some(&ib) = self.poly_index.get(&poly_b) {
            self.polys[ib].portals.push(portal_idx);
        }
        portal_idx
    }

    /// Auto-connect all pairs of adjacent polygons that share an edge.
    pub fn build_portals_from_edges(&mut self) {
        let n = self.polys.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let pid_a = self.polys[i].id;
                let pid_b = self.polys[j].id;
                if let Some((left, right)) = Self::find_shared_edge(&self.polys[i], &self.polys[j]) {
                    let portal_idx = self.portals.len();
                    self.portals.push(NavPortal { poly_a: pid_a, poly_b: pid_b, left, right });
                    self.polys[i].portals.push(portal_idx);
                    self.polys[j].portals.push(portal_idx);
                }
            }
        }
    }

    fn find_shared_edge(a: &NavPoly, b: &NavPoly) -> Option<(Vec2, Vec2)> {
        const EPS: f32 = 1e-4;
        let na = a.verts.len();
        let nb = b.verts.len();
        for i in 0..na {
            let va0 = a.verts[i];
            let va1 = a.verts[(i + 1) % na];
            for j in 0..nb {
                let vb0 = b.verts[j];
                let vb1 = b.verts[(j + 1) % nb];
                // shared edge if endpoints match (in any order)
                let fwd = va0.dist_sq(vb0) < EPS && va1.dist_sq(vb1) < EPS;
                let rev = va0.dist_sq(vb1) < EPS && va1.dist_sq(vb0) < EPS;
                if fwd || rev { return Some((va0, va1)); }
            }
        }
        None
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    pub fn poly_at_point(&self, p: Vec2) -> Option<NavPolyId> {
        for poly in &self.polys {
            if !poly.area.contains(AreaFlags::BLOCKED) && poly.contains_point(p) {
                return Some(poly.id);
            }
        }
        None
    }

    pub fn poly_by_id(&self, id: NavPolyId) -> Option<&NavPoly> {
        self.poly_index.get(&id).map(|&i| &self.polys[i])
    }

    fn poly_by_id_mut(&mut self, id: NavPolyId) -> Option<&mut NavPoly> {
        let i = *self.poly_index.get(&id)?;
        Some(&mut self.polys[i])
    }

    /// Returns the closest point on the navmesh to `p`.
    pub fn closest_point_on_mesh(&self, p: Vec2) -> NavPoint {
        let mut best_pt = p;
        let mut best_poly = NavPolyId(0);
        let mut best_dist = f32::MAX;
        for poly in &self.polys {
            if poly.area.contains(AreaFlags::BLOCKED) { continue; }
            let cp = poly.closest_point(p);
            let d = cp.dist_sq(p);
            if d < best_dist {
                best_dist = d;
                best_pt = cp;
                best_poly = poly.id;
            }
        }
        NavPoint { pos: best_pt, poly: best_poly }
    }

    // ── A* over navmesh ───────────────────────────────────────────────────────

    /// Pathfind from `start` to `end` position, returning polygon corridor and portals.
    pub fn find_path(&self, start: Vec2, end: Vec2, filter: AreaFlags) -> Option<NavPath> {
        let start_poly = self.poly_at_point(start)
            .unwrap_or_else(|| self.closest_point_on_mesh(start).poly);
        let end_poly = self.poly_at_point(end)
            .unwrap_or_else(|| self.closest_point_on_mesh(end).poly);

        if start_poly == end_poly {
            return Some(NavPath {
                polys:   vec![start_poly],
                portals: Vec::new(),
                waypoints: vec![start, end],
            });
        }

        // A* over polygon graph
        let mut open: BinaryHeap<AStarEntry> = BinaryHeap::new();
        let mut came_from: HashMap<NavPolyId, (NavPolyId, usize)> = HashMap::new(); // poly -> (prev_poly, portal_idx)
        let mut g_score: HashMap<NavPolyId, f32> = HashMap::new();

        g_score.insert(start_poly, 0.0);
        open.push(AStarEntry {
            poly: start_poly,
            f: self.heuristic(start_poly, end_poly),
        });

        while let Some(AStarEntry { poly: current, .. }) = open.pop() {
            if current == end_poly {
                let (polys, portal_indices) = self.reconstruct_corridor(start_poly, end_poly, &came_from);
                let portals: Vec<NavPortal> = portal_indices.iter().map(|&i| self.portals[i].clone()).collect();
                let waypoints = self.string_pull(start, end, &polys, &portals);
                return Some(NavPath { polys, portals, waypoints });
            }

            let current_g = *g_score.get(&current).unwrap_or(&f32::MAX);
            let portal_idxs: Vec<usize> = if let Some(p) = self.poly_by_id(current) {
                p.portals.clone()
            } else { continue };

            for pidx in portal_idxs {
                let portal = &self.portals[pidx];
                let neighbor = portal.other(current);
                if let Some(npoly) = self.poly_by_id(neighbor) {
                    if !npoly.area.intersect(filter).contains(AreaFlags::WALKABLE) { continue; }
                    if npoly.area.contains(AreaFlags::BLOCKED) { continue; }
                    let cost_mod = self.area_cost.get(npoly.area);
                    let edge_cost = current_g + portal.midpoint().dist(
                        self.poly_by_id(current).map(|p| p.centroid).unwrap_or(Vec2::zero())
                    ) * cost_mod;
                    let ng = current_g + edge_cost.max(0.1);
                    if ng < *g_score.get(&neighbor).unwrap_or(&f32::MAX) {
                        g_score.insert(neighbor, ng);
                        came_from.insert(neighbor, (current, pidx));
                        let h = self.heuristic(neighbor, end_poly);
                        open.push(AStarEntry { poly: neighbor, f: ng + h });
                    }
                }
            }
        }
        None
    }

    fn heuristic(&self, a: NavPolyId, b: NavPolyId) -> f32 {
        let ca = self.poly_by_id(a).map(|p| p.centroid).unwrap_or(Vec2::zero());
        let cb = self.poly_by_id(b).map(|p| p.centroid).unwrap_or(Vec2::zero());
        ca.dist(cb)
    }

    fn reconstruct_corridor(
        &self,
        start: NavPolyId,
        end: NavPolyId,
        came_from: &HashMap<NavPolyId, (NavPolyId, usize)>,
    ) -> (Vec<NavPolyId>, Vec<usize>) {
        let mut polys = Vec::new();
        let mut portal_indices = Vec::new();
        let mut cur = end;
        while cur != start {
            polys.push(cur);
            if let Some(&(prev, pidx)) = came_from.get(&cur) {
                portal_indices.push(pidx);
                cur = prev;
            } else { break; }
        }
        polys.push(start);
        polys.reverse();
        portal_indices.reverse();
        (polys, portal_indices)
    }

    // ── String-pulling (Simple Stupid Funnel Algorithm) ───────────────────────

    /// Smooth the polygon corridor into a minimal waypoint path using funnel algorithm.
    pub fn string_pull(&self, start: Vec2, end: Vec2, _polys: &[NavPolyId], portals: &[NavPortal]) -> Vec<Vec2> {
        if portals.is_empty() { return vec![start, end]; }

        // Build portal list: start point, portal edges, end point
        let mut port_lefts: Vec<Vec2> = Vec::new();
        let mut port_rights: Vec<Vec2> = Vec::new();

        port_lefts.push(start);
        port_rights.push(start);

        for portal in portals {
            port_lefts.push(portal.left);
            port_rights.push(portal.right);
        }
        port_lefts.push(end);
        port_rights.push(end);

        // SSFA
        let mut path = vec![start];
        let mut apex = start;
        let mut left = port_lefts[1];
        let mut right = port_rights[1];
        let mut apex_idx = 0usize;
        let mut left_idx = 1usize;
        let mut right_idx = 1usize;

        let n = port_lefts.len();
        for i in 2..n {
            let new_left = port_lefts[i];
            let new_right = port_rights[i];

            // Update right leg
            if triangle_area2(apex, right, new_right) <= 0.0 {
                if apex == right || triangle_area2(apex, left, new_right) > 0.0 {
                    right = new_right;
                    right_idx = i;
                } else {
                    // Right crossed left — left is next waypoint
                    path.push(left);
                    apex = left;
                    apex_idx = left_idx;
                    right = apex;
                    right_idx = apex_idx;
                    // Restart
                    if apex_idx + 1 < n { left = port_lefts[apex_idx + 1]; left_idx = apex_idx + 1; }
                    if apex_idx + 1 < n { right = port_rights[apex_idx + 1]; right_idx = apex_idx + 1; }
                    // Back up i to restart scanning from apex
                    // (simplified: continue, the next iteration re-evaluates)
                    continue;
                }
            }

            // Update left leg
            if triangle_area2(apex, left, new_left) >= 0.0 {
                if apex == left || triangle_area2(apex, right, new_left) < 0.0 {
                    left = new_left;
                    left_idx = i;
                } else {
                    path.push(right);
                    apex = right;
                    apex_idx = right_idx;
                    left = apex;
                    left_idx = apex_idx;
                    if apex_idx + 1 < n { right = port_rights[apex_idx + 1]; right_idx = apex_idx + 1; }
                    if apex_idx + 1 < n { left = port_lefts[apex_idx + 1]; left_idx = apex_idx + 1; }
                    continue;
                }
            }
        }

        path.push(end);
        // Remove duplicate consecutive points
        path.dedup_by(|a, b| a.dist_sq(*b) < 1e-8);
        path
    }
}

// Signed 2D triangle area × 2
#[inline]
fn triangle_area2(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)
}

/// A point with polygon context.
#[derive(Clone, Copy, Debug)]
pub struct NavPoint {
    pub pos:  Vec2,
    pub poly: NavPolyId,
}

/// Result of navmesh pathfinding: polygon corridor + smoothed waypoints.
#[derive(Clone, Debug)]
pub struct NavPath {
    pub polys:     Vec<NavPolyId>,
    pub portals:   Vec<NavPortal>,
    pub waypoints: Vec<Vec2>,
}

// ── A* priority queue entry ───────────────────────────────────────────────────

#[derive(PartialEq)]
struct AStarEntry {
    poly: NavPolyId,
    f:    f32,
}

impl Eq for AStarEntry {}

impl PartialOrd for AStarEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for AStarEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

// ── Dynamic obstacle cutting (Recast-style) ───────────────────────────────────

/// An obstacle that can be cut into the navmesh (marks polygons as blocked).
#[derive(Clone, Debug)]
pub struct Obstacle {
    pub id:     u32,
    pub center: Vec2,
    pub radius: f32,
    /// Which polys were affected
    affected:   Vec<NavPolyId>,
}

/// Manages dynamic obstacle cutting on a NavMesh.
pub struct ObstacleCutter {
    obstacles: HashMap<u32, Obstacle>,
    next_id:   u32,
}

impl ObstacleCutter {
    pub fn new() -> Self {
        Self { obstacles: HashMap::new(), next_id: 0 }
    }

    /// Add a circular obstacle, marking overlapping polygons as blocked.
    pub fn add_obstacle(&mut self, mesh: &mut NavMesh, center: Vec2, radius: f32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let mut affected = Vec::new();

        for poly in &mut mesh.polys {
            if poly_overlaps_circle(&poly.verts, center, radius) {
                poly.area = poly.area.union(AreaFlags::BLOCKED);
                affected.push(poly.id);
            }
        }
        self.obstacles.insert(id, Obstacle { id, center, radius, affected });
        id
    }

    /// Remove an obstacle and restore polygon flags.
    pub fn remove_obstacle(&mut self, mesh: &mut NavMesh, id: u32) {
        if let Some(obs) = self.obstacles.remove(&id) {
            for pid in obs.affected {
                if let Some(idx) = mesh.poly_index.get(&pid) {
                    let poly = &mut mesh.polys[*idx];
                    poly.area = AreaFlags(poly.area.0 & !AreaFlags::BLOCKED.0);
                }
            }
        }
    }

    /// Move an obstacle (remove old, add new).
    pub fn move_obstacle(&mut self, mesh: &mut NavMesh, id: u32, new_center: Vec2) {
        if let Some(obs) = self.obstacles.get(&id).cloned() {
            self.remove_obstacle(mesh, id);
            self.add_obstacle(mesh, new_center, obs.radius);
        }
    }
}

fn poly_overlaps_circle(verts: &[Vec2], center: Vec2, radius: f32) -> bool {
    let r2 = radius * radius;
    // Check if center inside poly
    if verts.len() >= 3 {
        let mut inside = true;
        let n = verts.len();
        for i in 0..n {
            let a = verts[i];
            let b = verts[(i + 1) % n];
            if b.sub(a).cross(center.sub(a)) < 0.0 { inside = false; break; }
        }
        if inside { return true; }
    }
    // Check if any edge is close enough
    let n = verts.len();
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let cp = closest_point_on_segment(a, b, center);
        if cp.dist_sq(center) <= r2 { return true; }
    }
    false
}

// ── NavMeshQuery facade ───────────────────────────────────────────────────────

/// High-level query interface wrapping NavMesh for typical game use.
pub struct NavMeshQuery<'a> {
    pub mesh:   &'a NavMesh,
    pub filter: AreaFlags,
}

impl<'a> NavMeshQuery<'a> {
    pub fn new(mesh: &'a NavMesh) -> Self {
        Self { mesh, filter: AreaFlags::WALKABLE }
    }

    pub fn with_filter(mut self, filter: AreaFlags) -> Self {
        self.filter = filter;
        self
    }

    /// Find path from `start` to `end`, returning smoothed waypoints.
    pub fn find_path(&self, start: Vec2, end: Vec2) -> Vec<Vec2> {
        self.mesh.find_path(start, end, self.filter)
            .map(|p| p.waypoints)
            .unwrap_or_default()
    }

    /// Snap point to navmesh.
    pub fn snap(&self, p: Vec2) -> Vec2 {
        self.mesh.closest_point_on_mesh(p).pos
    }

    /// Raycast on navmesh, returns None if unobstructed, Some(hit) if blocked.
    pub fn raycast(&self, start: Vec2, end: Vec2) -> Option<Vec2> {
        let start_poly = self.mesh.poly_at_point(start)?;
        let dir = end.sub(start);
        let len = dir.len();
        if len < 1e-9 { return None; }
        let step = dir.scale(1.0 / len);
        let steps = (len / 0.5) as usize + 1;
        for i in 1..=steps {
            let t = (i as f32 * 0.5).min(len);
            let p = start.add(step.scale(t));
            if let Some(poly_id) = self.mesh.poly_at_point(p) {
                if let Some(poly) = self.mesh.poly_by_id(poly_id) {
                    if poly.area.contains(AreaFlags::BLOCKED) { return Some(p); }
                }
            } else {
                return Some(p);
            }
        }
        None
    }
}

// ── Precomputed navmesh region graph for hierarchical planning ────────────────

/// A region groups several polygons for hierarchical pathfinding.
#[derive(Clone, Debug)]
pub struct NavRegion {
    pub id:    u32,
    pub polys: HashSet<NavPolyId>,
    pub entry_portals: Vec<usize>, // portals connecting to other regions
}

/// Graph of regions for hierarchical pathfinding pre-computation.
#[derive(Clone, Debug, Default)]
pub struct RegionGraph {
    pub regions: Vec<NavRegion>,
    pub poly_to_region: HashMap<NavPolyId, u32>,
}

impl RegionGraph {
    /// Build by flood-filling polygons into clusters of `max_size`.
    pub fn build(mesh: &NavMesh, max_cluster_size: usize) -> Self {
        let mut graph = Self::default();
        let mut visited: HashSet<NavPolyId> = HashSet::new();
        let mut region_id = 0u32;

        for poly in &mesh.polys {
            if visited.contains(&poly.id) { continue; }
            if poly.area.contains(AreaFlags::BLOCKED) { continue; }

            // BFS flood fill
            let mut cluster = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(poly.id);

            while let Some(pid) = queue.pop_front() {
                if visited.contains(&pid) { continue; }
                if cluster.len() >= max_cluster_size { break; }
                visited.insert(pid);
                cluster.insert(pid);
                if let Some(p) = mesh.poly_by_id(pid) {
                    for &portal_idx in &p.portals {
                        let portal = &mesh.portals[portal_idx];
                        let neighbor = portal.other(pid);
                        if !visited.contains(&neighbor) {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }

            for &pid in &cluster {
                graph.poly_to_region.insert(pid, region_id);
            }
            graph.regions.push(NavRegion { id: region_id, polys: cluster, entry_portals: Vec::new() });
            region_id += 1;
        }
        // Mark entry portals
        let region_count = graph.regions.len();
        for (pidx, portal) in mesh.portals.iter().enumerate() {
            let ra = graph.poly_to_region.get(&portal.poly_a).copied();
            let rb = graph.poly_to_region.get(&portal.poly_b).copied();
            if let (Some(ra), Some(rb)) = (ra, rb) {
                if ra != rb && ra < region_count as u32 && rb < region_count as u32 {
                    graph.regions[ra as usize].entry_portals.push(pidx);
                    graph.regions[rb as usize].entry_portals.push(pidx);
                }
            }
        }
        graph
    }
}

// ── Additional path utilities ─────────────────────────────────────────────────

/// Compute the total length of a waypoint path.
pub fn path_length(waypoints: &[Vec2]) -> f32 {
    waypoints.windows(2).map(|w| w[0].dist(w[1])).sum()
}

/// Sample a position along a waypoint path at arc-length parameter `t` in [0,1].
pub fn path_sample(waypoints: &[Vec2], t: f32) -> Vec2 {
    if waypoints.is_empty() { return Vec2::zero(); }
    if waypoints.len() == 1 { return waypoints[0]; }
    let total = path_length(waypoints);
    if total < 1e-9 { return waypoints[0]; }
    let target = (t.clamp(0.0, 1.0) * total).min(total - 1e-9);
    let mut acc = 0.0f32;
    for i in 0..waypoints.len() - 1 {
        let seg = waypoints[i].dist(waypoints[i + 1]);
        if acc + seg >= target {
            let local_t = (target - acc) / seg.max(1e-9);
            return waypoints[i].lerp(waypoints[i + 1], local_t);
        }
        acc += seg;
    }
    *waypoints.last().unwrap()
}

/// Find the index of the nearest waypoint to position `p`.
pub fn nearest_waypoint_index(waypoints: &[Vec2], p: Vec2) -> usize {
    waypoints.iter().enumerate()
        .min_by(|(_, a), (_, b)| a.dist_sq(p).partial_cmp(&b.dist_sq(p)).unwrap_or(Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Simplify a path by removing intermediate points within `tolerance` of the line.
pub fn simplify_path(waypoints: &[Vec2], tolerance: f32) -> Vec<Vec2> {
    if waypoints.len() <= 2 { return waypoints.to_vec(); }
    let tol2 = tolerance * tolerance;
    let mut result = vec![waypoints[0]];
    let mut i = 0usize;
    while i < waypoints.len() - 1 {
        let mut farthest = i + 1;
        for j in (i + 1)..waypoints.len() {
            let cp = closest_point_on_segment(waypoints[i], waypoints[j.min(waypoints.len()-1)], waypoints[j]);
            if cp.dist_sq(waypoints[j]) > tol2 { break; }
            farthest = j;
        }
        result.push(waypoints[farthest]);
        i = farthest;
    }
    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn square_poly(id: u32, ox: f32, oy: f32, sz: f32, area: AreaFlags) -> (NavPolyId, Vec<Vec2>) {
        let verts = vec![
            Vec2::new(ox, oy),
            Vec2::new(ox + sz, oy),
            Vec2::new(ox + sz, oy + sz),
            Vec2::new(ox, oy + sz),
        ];
        (NavPolyId(id), verts)
    }

    #[test]
    fn test_point_in_poly() {
        let verts = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 4.0),
            Vec2::new(0.0, 4.0),
        ];
        let poly = NavPoly::new(NavPolyId(0), verts, AreaFlags::WALKABLE, 1.0);
        assert!(poly.contains_point(Vec2::new(2.0, 2.0)));
        assert!(!poly.contains_point(Vec2::new(5.0, 5.0)));
    }

    #[test]
    fn test_navmesh_build_and_path() {
        let mut mesh = NavMesh::new();
        let a = mesh.add_poly(vec![
            Vec2::new(0.0,0.0), Vec2::new(4.0,0.0),
            Vec2::new(4.0,4.0), Vec2::new(0.0,4.0),
        ], AreaFlags::WALKABLE, 1.0);
        let b = mesh.add_poly(vec![
            Vec2::new(4.0,0.0), Vec2::new(8.0,0.0),
            Vec2::new(8.0,4.0), Vec2::new(4.0,4.0),
        ], AreaFlags::WALKABLE, 1.0);
        mesh.build_portals_from_edges();
        let path = mesh.find_path(Vec2::new(1.0, 2.0), Vec2::new(7.0, 2.0), AreaFlags::WALKABLE);
        assert!(path.is_some());
        let wp = path.unwrap().waypoints;
        assert!(wp.len() >= 2);
    }

    #[test]
    fn test_closest_point_on_segment() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(4.0, 0.0);
        let p = Vec2::new(2.0, 3.0);
        let c = closest_point_on_segment(a, b, p);
        assert!((c.x - 2.0).abs() < 1e-5);
        assert!((c.y - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_obstacle_cutter() {
        let mut mesh = NavMesh::new();
        let id = mesh.add_poly(vec![
            Vec2::new(0.0,0.0), Vec2::new(4.0,0.0),
            Vec2::new(4.0,4.0), Vec2::new(0.0,4.0),
        ], AreaFlags::WALKABLE, 1.0);
        let mut cutter = ObstacleCutter::new();
        let oid = cutter.add_obstacle(&mut mesh, Vec2::new(2.0, 2.0), 1.0);
        assert!(mesh.poly_by_id(id).unwrap().area.contains(AreaFlags::BLOCKED));
        cutter.remove_obstacle(&mut mesh, oid);
        assert!(!mesh.poly_by_id(id).unwrap().area.contains(AreaFlags::BLOCKED));
    }

    #[test]
    fn test_path_sample() {
        let pts = vec![Vec2::new(0.0,0.0), Vec2::new(10.0,0.0)];
        let mid = path_sample(&pts, 0.5);
        assert!((mid.x - 5.0).abs() < 1e-4);
    }
}
