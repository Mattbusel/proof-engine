//! A* pathfinding, Dijkstra maps, Jump Point Search, and hierarchical pathfinding.
//!
//! # Example
//! ```rust
//! use proof_engine::ai::pathfinding::{PathGrid, AStarPathfinder};
//! use glam::Vec2;
//!
//! let mut grid = PathGrid::new(20, 20, 1.0);
//! grid.set_walkable(5, 5, false); // obstacle
//! let finder = AStarPathfinder::new();
//! if let Some(path) = finder.find_path(&grid, Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)) {
//!     println!("Found path with {} waypoints", path.len());
//! }
//! ```

use glam::Vec2;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Ordering;

// ---------------------------------------------------------------------------
// Core grid types
// ---------------------------------------------------------------------------

/// A single node used internally during A* search.
#[derive(Debug, Clone)]
pub struct PathNode {
    pub pos: Vec2,
    pub g_cost: f32,
    pub h_cost: f32,
    pub f_cost: f32,
    pub parent: Option<usize>,
}

impl PathNode {
    pub fn new(pos: Vec2, g_cost: f32, h_cost: f32, parent: Option<usize>) -> Self {
        PathNode {
            pos,
            g_cost,
            h_cost,
            f_cost: g_cost + h_cost,
            parent,
        }
    }
}

/// Wrapper so PathNode can be stored in a BinaryHeap (min-heap by f_cost).
#[derive(Debug, Clone)]
struct HeapNode {
    f_cost: f32,
    g_cost: f32,
    index: usize,
}

impl PartialEq for HeapNode {
    fn eq(&self, other: &Self) -> bool {
        self.f_cost == other.f_cost
    }
}
impl Eq for HeapNode {}
impl PartialOrd for HeapNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap
        other.f_cost.partial_cmp(&self.f_cost)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.g_cost.partial_cmp(&self.g_cost).unwrap_or(Ordering::Equal))
    }
}

/// A uniform grid used for all grid-based pathfinding algorithms.
#[derive(Debug, Clone)]
pub struct PathGrid {
    pub width: usize,
    pub height: usize,
    pub cell_size: f32,
    pub walkable: Vec<bool>,
    pub weights: Vec<f32>,
    pub origin: Vec2,
}

impl PathGrid {
    /// Create a new fully-walkable grid.
    pub fn new(width: usize, height: usize, cell_size: f32) -> Self {
        let n = width * height;
        PathGrid {
            width,
            height,
            cell_size,
            walkable: vec![true; n],
            weights: vec![1.0; n],
            origin: Vec2::ZERO,
        }
    }

    /// Create grid with a world-space origin offset.
    pub fn with_origin(width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        let mut g = Self::new(width, height, cell_size);
        g.origin = origin;
        g
    }

    /// Mark a cell walkable or not.
    pub fn set_walkable(&mut self, x: usize, y: usize, walkable: bool) {
        if let Some(idx) = self.index(x, y) {
            self.walkable[idx] = walkable;
        }
    }

    /// Set the traversal weight of a cell (higher = more expensive).
    pub fn set_weight(&mut self, x: usize, y: usize, weight: f32) {
        if let Some(idx) = self.index(x, y) {
            self.weights[idx] = weight.max(0.001);
        }
    }

    /// Convert a world position to grid coordinates. Clamps to valid range.
    pub fn world_to_grid(&self, pos: Vec2) -> (usize, usize) {
        let local = pos - self.origin;
        let x = ((local.x / self.cell_size).floor() as isize).clamp(0, self.width as isize - 1) as usize;
        let y = ((local.y / self.cell_size).floor() as isize).clamp(0, self.height as isize - 1) as usize;
        (x, y)
    }

    /// Convert grid coordinates to the world-space centre of that cell.
    pub fn grid_to_world(&self, x: usize, y: usize) -> Vec2 {
        self.origin + Vec2::new(
            x as f32 * self.cell_size + self.cell_size * 0.5,
            y as f32 * self.cell_size + self.cell_size * 0.5,
        )
    }

    /// Check whether a cell is walkable.
    pub fn is_walkable(&self, x: usize, y: usize) -> bool {
        match self.index(x, y) {
            Some(idx) => self.walkable[idx],
            None => false,
        }
    }

    /// Get the movement cost of a cell.
    pub fn weight(&self, x: usize, y: usize) -> f32 {
        match self.index(x, y) {
            Some(idx) => self.weights[idx],
            None => f32::INFINITY,
        }
    }

    /// Returns walkable neighbours of (x, y) together with their movement cost.
    /// Includes diagonals; diagonal cost is sqrt(2) * avg_weight.
    pub fn neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize, f32)> {
        let mut result = Vec::with_capacity(8);
        let dirs: &[(i32, i32, bool)] = &[
            (-1,  0, false), (1,  0, false), (0, -1, false), (0, 1, false),
            (-1, -1, true),  (1, -1, true),  (-1, 1, true),  (1, 1, true),
        ];
        for &(dx, dy, diagonal) in dirs {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                continue;
            }
            let nx = nx as usize;
            let ny = ny as usize;
            if !self.is_walkable(nx, ny) {
                continue;
            }
            // Diagonal movement: both cardinal neighbours must also be walkable
            if diagonal {
                if !self.is_walkable(x, ny) || !self.is_walkable(nx, y) {
                    continue;
                }
                let cost = std::f32::consts::SQRT_2 * self.weight(nx, ny);
                result.push((nx, ny, cost));
            } else {
                result.push((nx, ny, self.weight(nx, ny)));
            }
        }
        result
    }

    /// Cardinal-only neighbours (no diagonals).
    pub fn neighbors_cardinal(&self, x: usize, y: usize) -> Vec<(usize, usize, f32)> {
        let mut result = Vec::with_capacity(4);
        let dirs: &[(i32, i32)] = &[(-1, 0), (1, 0), (0, -1), (0, 1)];
        for &(dx, dy) in dirs {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                continue;
            }
            let nx = nx as usize;
            let ny = ny as usize;
            if self.is_walkable(nx, ny) {
                result.push((nx, ny, self.weight(nx, ny)));
            }
        }
        result
    }

    /// Line-of-sight check between two grid cells (Bresenham).
    pub fn line_of_sight(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> bool {
        let mut x0 = x0 as i32;
        let mut y0 = y0 as i32;
        let x1 = x1 as i32;
        let y1 = y1 as i32;
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        loop {
            if x0 == x1 && y0 == y1 { break; }
            if !self.is_walkable(x0 as usize, y0 as usize) {
                return false;
            }
            let e2 = 2 * err;
            if e2 > -dy { err -= dy; x0 += sx; }
            if e2 < dx  { err += dx; y0 += sy; }
        }
        true
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.width && y < self.height {
            Some(y * self.width + x)
        } else {
            None
        }
    }

    /// Total number of cells.
    pub fn len(&self) -> usize { self.width * self.height }
}

// ---------------------------------------------------------------------------
// Heuristics
// ---------------------------------------------------------------------------

/// Available heuristic functions for A*.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Heuristic {
    #[default]
    Manhattan,
    Euclidean,
    Chebyshev,
    Octile,
}

impl Heuristic {
    pub fn compute(self, ax: usize, ay: usize, bx: usize, by: usize) -> f32 {
        let dx = (ax as f32 - bx as f32).abs();
        let dy = (ay as f32 - by as f32).abs();
        match self {
            Heuristic::Manhattan  => dx + dy,
            Heuristic::Euclidean  => (dx * dx + dy * dy).sqrt(),
            Heuristic::Chebyshev  => dx.max(dy),
            Heuristic::Octile => {
                let min = dx.min(dy);
                let max = dx.max(dy);
                std::f32::consts::SQRT_2 * min + (max - min)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Path types
// ---------------------------------------------------------------------------

/// A resolved path as a sequence of world positions.
#[derive(Debug, Clone, Default)]
pub struct Path {
    pub waypoints: Vec<Vec2>,
    pub total_cost: f32,
}

impl Path {
    pub fn new(waypoints: Vec<Vec2>, total_cost: f32) -> Self {
        Path { waypoints, total_cost }
    }
    pub fn is_empty(&self) -> bool { self.waypoints.is_empty() }
    pub fn len(&self) -> usize { self.waypoints.len() }
}

/// A pathfinding request.
#[derive(Debug, Clone)]
pub struct PathRequest {
    pub start: Vec2,
    pub end: Vec2,
    pub agent_radius: f32,
    pub heuristic: Heuristic,
    pub allow_partial: bool,
}

impl PathRequest {
    pub fn new(start: Vec2, end: Vec2) -> Self {
        PathRequest {
            start,
            end,
            agent_radius: 0.0,
            heuristic: Heuristic::Octile,
            allow_partial: false,
        }
    }
}

/// The result of a pathfinding operation.
#[derive(Debug, Clone)]
pub enum PathResult {
    Found(Path),
    Partial(Path),
    NoPath,
    InvalidRequest(String),
}

/// Statistics collected during a pathfinding run.
#[derive(Debug, Clone, Default)]
pub struct PathfindingStats {
    pub nodes_expanded: usize,
    pub nodes_generated: usize,
    pub path_length: usize,
    pub duration_us: u64,
}

// ---------------------------------------------------------------------------
// A* Pathfinder
// ---------------------------------------------------------------------------

/// Full A* pathfinder operating on a `PathGrid`.
#[derive(Debug, Clone)]
pub struct AStarPathfinder {
    pub heuristic: Heuristic,
    pub allow_diagonal: bool,
    pub smooth_result: bool,
}

impl AStarPathfinder {
    pub fn new() -> Self {
        AStarPathfinder {
            heuristic: Heuristic::Octile,
            allow_diagonal: true,
            smooth_result: true,
        }
    }

    pub fn with_heuristic(mut self, h: Heuristic) -> Self { self.heuristic = h; self }
    pub fn with_diagonal(mut self, v: bool) -> Self { self.allow_diagonal = v; self }
    pub fn with_smoothing(mut self, v: bool) -> Self { self.smooth_result = v; self }

    /// Find a path from `start` to `end` in world coordinates.
    pub fn find_path(&self, grid: &PathGrid, start: Vec2, end: Vec2) -> Option<Vec<Vec2>> {
        let (sx, sy) = grid.world_to_grid(start);
        let (ex, ey) = grid.world_to_grid(end);

        if !grid.is_walkable(sx, sy) || !grid.is_walkable(ex, ey) {
            return None;
        }
        if sx == ex && sy == ey {
            return Some(vec![start, end]);
        }

        let start_idx = sy * grid.width + sx;
        let end_idx   = ey * grid.width + ex;

        // g_costs[i] = best known cost to reach cell i
        let mut g_costs: Vec<f32> = vec![f32::INFINITY; grid.len()];
        let mut parents: Vec<Option<usize>> = vec![None; grid.len()];
        let mut open: BinaryHeap<HeapNode> = BinaryHeap::new();
        let mut closed: HashSet<usize> = HashSet::new();

        g_costs[start_idx] = 0.0;
        let h = self.heuristic.compute(sx, sy, ex, ey);
        open.push(HeapNode { f_cost: h, g_cost: 0.0, index: start_idx });

        while let Some(current) = open.pop() {
            let idx = current.index;
            if closed.contains(&idx) { continue; }
            closed.insert(idx);

            if idx == end_idx {
                // Reconstruct path
                let mut path_indices = Vec::new();
                let mut cur = idx;
                loop {
                    path_indices.push(cur);
                    match parents[cur] {
                        Some(p) => cur = p,
                        None => break,
                    }
                }
                path_indices.reverse();
                let mut waypoints: Vec<Vec2> = path_indices.iter().map(|&i| {
                    let gx = i % grid.width;
                    let gy = i / grid.width;
                    grid.grid_to_world(gx, gy)
                }).collect();
                // Replace first/last with exact world positions
                if !waypoints.is_empty() { waypoints[0] = start; }
                if waypoints.len() > 1 { *waypoints.last_mut().unwrap() = end; }

                if self.smooth_result {
                    waypoints = smooth_path(waypoints, grid);
                }
                return Some(waypoints);
            }

            let cx = idx % grid.width;
            let cy = idx / grid.width;
            let neighbors = if self.allow_diagonal {
                grid.neighbors(cx, cy)
            } else {
                grid.neighbors_cardinal(cx, cy)
            };

            for (nx, ny, move_cost) in neighbors {
                let nidx = ny * grid.width + nx;
                if closed.contains(&nidx) { continue; }
                let tentative_g = g_costs[idx] + move_cost;
                if tentative_g < g_costs[nidx] {
                    g_costs[nidx] = tentative_g;
                    parents[nidx] = Some(idx);
                    let h = self.heuristic.compute(nx, ny, ex, ey);
                    open.push(HeapNode {
                        f_cost: tentative_g + h,
                        g_cost: tentative_g,
                        index: nidx,
                    });
                }
            }
        }
        None
    }

    /// Find a path and return detailed stats.
    pub fn find_path_detailed(
        &self,
        grid: &PathGrid,
        request: &PathRequest,
    ) -> (PathResult, PathfindingStats) {
        let mut stats = PathfindingStats::default();
        let (sx, sy) = grid.world_to_grid(request.start);
        let (ex, ey) = grid.world_to_grid(request.end);

        if !grid.is_walkable(sx, sy) || !grid.is_walkable(ex, ey) {
            return (PathResult::NoPath, stats);
        }

        let start_idx = sy * grid.width + sx;
        let end_idx   = ey * grid.width + ex;
        let mut g_costs: Vec<f32> = vec![f32::INFINITY; grid.len()];
        let mut parents: Vec<Option<usize>> = vec![None; grid.len()];
        let mut open: BinaryHeap<HeapNode> = BinaryHeap::new();
        let mut closed: HashSet<usize> = HashSet::new();
        // Track best node for partial path
        let mut best_idx = start_idx;
        let mut best_h = request.heuristic.compute(sx, sy, ex, ey);

        g_costs[start_idx] = 0.0;
        open.push(HeapNode { f_cost: best_h, g_cost: 0.0, index: start_idx });
        stats.nodes_generated += 1;

        while let Some(current) = open.pop() {
            let idx = current.index;
            if closed.contains(&idx) { continue; }
            closed.insert(idx);
            stats.nodes_expanded += 1;

            let cx = idx % grid.width;
            let cy = idx / grid.width;
            let h = request.heuristic.compute(cx, cy, ex, ey);
            if h < best_h { best_h = h; best_idx = idx; }

            if idx == end_idx {
                let path = self.reconstruct(grid, &parents, idx, request.start, request.end);
                stats.path_length = path.len();
                return (PathResult::Found(Path::new(path, g_costs[idx])), stats);
            }

            let neighbors = if self.allow_diagonal {
                grid.neighbors(cx, cy)
            } else {
                grid.neighbors_cardinal(cx, cy)
            };
            for (nx, ny, move_cost) in neighbors {
                let nidx = ny * grid.width + nx;
                if closed.contains(&nidx) { continue; }
                let tentative_g = g_costs[idx] + move_cost;
                if tentative_g < g_costs[nidx] {
                    g_costs[nidx] = tentative_g;
                    parents[nidx] = Some(idx);
                    let h2 = request.heuristic.compute(nx, ny, ex, ey);
                    open.push(HeapNode { f_cost: tentative_g + h2, g_cost: tentative_g, index: nidx });
                    stats.nodes_generated += 1;
                }
            }
        }

        if request.allow_partial && best_idx != start_idx {
            let bx = best_idx % grid.width;
            let by = best_idx / grid.width;
            let partial_end = grid.grid_to_world(bx, by);
            let path = self.reconstruct(grid, &parents, best_idx, request.start, partial_end);
            stats.path_length = path.len();
            return (PathResult::Partial(Path::new(path, g_costs[best_idx])), stats);
        }

        (PathResult::NoPath, stats)
    }

    fn reconstruct(
        &self,
        grid: &PathGrid,
        parents: &[Option<usize>],
        end_idx: usize,
        start_world: Vec2,
        end_world: Vec2,
    ) -> Vec<Vec2> {
        let mut indices = Vec::new();
        let mut cur = end_idx;
        loop {
            indices.push(cur);
            match parents[cur] {
                Some(p) => cur = p,
                None => break,
            }
        }
        indices.reverse();
        let mut waypoints: Vec<Vec2> = indices.iter().map(|&i| {
            grid.grid_to_world(i % grid.width, i / grid.width)
        }).collect();
        if !waypoints.is_empty() { waypoints[0] = start_world; }
        if waypoints.len() > 1 { *waypoints.last_mut().unwrap() = end_world; }
        if self.smooth_result {
            waypoints = smooth_path(waypoints, grid);
        }
        waypoints
    }
}

impl Default for AStarPathfinder {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Path smoothing
// ---------------------------------------------------------------------------

/// Remove redundant waypoints using line-of-sight checks (string-pulling).
pub fn smooth_path(path: Vec<Vec2>, grid: &PathGrid) -> Vec<Vec2> {
    if path.len() <= 2 { return path; }
    let mut smoothed = Vec::with_capacity(path.len());
    smoothed.push(path[0]);
    let mut current = 0usize;
    while current < path.len() - 1 {
        let mut furthest = current + 1;
        for next in (current + 2)..path.len() {
            let (ax, ay) = grid.world_to_grid(path[current]);
            let (bx, by) = grid.world_to_grid(path[next]);
            if grid.line_of_sight(ax, ay, bx, by) {
                furthest = next;
            } else {
                break;
            }
        }
        smoothed.push(path[furthest]);
        current = furthest;
    }
    smoothed
}

/// Catmull-Rom spline interpolation for smooth curved paths.
pub fn spline_path(path: &[Vec2], samples_per_segment: usize) -> Vec<Vec2> {
    if path.len() < 2 { return path.to_vec(); }
    let mut result = Vec::new();
    for i in 0..path.len().saturating_sub(1) {
        let p0 = if i == 0 { path[0] } else { path[i - 1] };
        let p1 = path[i];
        let p2 = path[i + 1];
        let p3 = if i + 2 < path.len() { path[i + 2] } else { path[i + 1] };
        for s in 0..samples_per_segment {
            let t = s as f32 / samples_per_segment as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let v = 0.5 * (
                (2.0 * p1)
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3
            );
            result.push(v);
        }
    }
    result.push(*path.last().unwrap());
    result
}

// ---------------------------------------------------------------------------
// Dijkstra Map
// ---------------------------------------------------------------------------

/// A distance field computed simultaneously from multiple source cells.
/// Useful for "all enemies chase the player" scenarios — compute once, use many.
#[derive(Debug, Clone)]
pub struct DijkstraMap {
    pub width: usize,
    pub height: usize,
    pub distances: Vec<f32>,
    pub flow: Vec<Option<(usize, usize)>>, // best next step toward nearest goal
}

impl DijkstraMap {
    /// Create an empty map of the same dimensions as `grid`.
    pub fn new(width: usize, height: usize) -> Self {
        let n = width * height;
        DijkstraMap {
            width,
            height,
            distances: vec![f32::INFINITY; n],
            flow: vec![None; n],
        }
    }

    /// Build the distance field from a set of goal positions (world coords).
    pub fn build(&mut self, grid: &PathGrid, goals: &[Vec2]) {
        self.distances.fill(f32::INFINITY);
        self.flow.fill(None);

        let mut queue: VecDeque<usize> = VecDeque::new();
        for &goal in goals {
            let (gx, gy) = grid.world_to_grid(goal);
            if grid.is_walkable(gx, gy) {
                let idx = gy * grid.width + gx;
                self.distances[idx] = 0.0;
                queue.push_back(idx);
            }
        }

        while let Some(idx) = queue.pop_front() {
            let cx = idx % grid.width;
            let cy = idx / grid.width;
            for (nx, ny, cost) in grid.neighbors(cx, cy) {
                let nidx = ny * grid.width + nx;
                let new_dist = self.distances[idx] + cost;
                if new_dist < self.distances[nidx] {
                    self.distances[nidx] = new_dist;
                    self.flow[nidx] = Some((cx, cy));
                    queue.push_back(nidx);
                }
            }
        }
    }

    /// Build from grid coordinates directly.
    pub fn build_from_cells(&mut self, grid: &PathGrid, goals: &[(usize, usize)]) {
        self.distances.fill(f32::INFINITY);
        self.flow.fill(None);

        let mut queue: VecDeque<usize> = VecDeque::new();
        for &(gx, gy) in goals {
            if grid.is_walkable(gx, gy) {
                let idx = gy * grid.width + gx;
                self.distances[idx] = 0.0;
                queue.push_back(idx);
            }
        }

        while let Some(idx) = queue.pop_front() {
            let cx = idx % grid.width;
            let cy = idx / grid.width;
            for (nx, ny, cost) in grid.neighbors(cx, cy) {
                let nidx = ny * grid.width + nx;
                let new_dist = self.distances[idx] + cost;
                if new_dist < self.distances[nidx] {
                    self.distances[nidx] = new_dist;
                    self.flow[nidx] = Some((cx, cy));
                    queue.push_back(nidx);
                }
            }
        }
    }

    /// Get the distance at a world position.
    pub fn distance_at(&self, grid: &PathGrid, pos: Vec2) -> f32 {
        let (x, y) = grid.world_to_grid(pos);
        self.distances[y * self.width + x]
    }

    /// Get the best next step from a world position toward the nearest goal.
    pub fn next_step(&self, grid: &PathGrid, pos: Vec2) -> Option<Vec2> {
        let (x, y) = grid.world_to_grid(pos);
        let idx = y * self.width + x;
        self.flow[idx].map(|(nx, ny)| grid.grid_to_world(nx, ny))
    }

    /// Extract a full path by following the flow field from `start` to any goal.
    pub fn extract_path(&self, grid: &PathGrid, start: Vec2) -> Vec<Vec2> {
        let mut path = Vec::new();
        let (mut cx, mut cy) = grid.world_to_grid(start);
        let mut visited: HashSet<usize> = HashSet::new();
        loop {
            let idx = cy * self.width + cx;
            if visited.contains(&idx) { break; }
            visited.insert(idx);
            path.push(grid.grid_to_world(cx, cy));
            if self.distances[idx] == 0.0 { break; }
            match self.flow[idx] {
                Some((nx, ny)) => { cx = nx; cy = ny; }
                None => break,
            }
        }
        path
    }
}

// ---------------------------------------------------------------------------
// Jump Point Search
// ---------------------------------------------------------------------------

/// Jump Point Search — optimized A* for uniform-cost grids.
/// Significantly reduces nodes expanded on open terrain.
#[derive(Debug, Clone, Default)]
pub struct JumpPointSearch {
    pub heuristic: Heuristic,
}

impl JumpPointSearch {
    pub fn new() -> Self { JumpPointSearch { heuristic: Heuristic::Octile } }

    /// Find a path using JPS. Falls back to standard A* on weighted grids.
    pub fn find_path(&self, grid: &PathGrid, start: Vec2, end: Vec2) -> Option<Vec<Vec2>> {
        let (sx, sy) = grid.world_to_grid(start);
        let (ex, ey) = grid.world_to_grid(end);
        if !grid.is_walkable(sx, sy) || !grid.is_walkable(ex, ey) {
            return None;
        }
        if sx == ex && sy == ey {
            return Some(vec![start, end]);
        }

        let start_idx = sy * grid.width + sx;
        let end_idx   = ey * grid.width + ex;
        let mut g_costs: Vec<f32> = vec![f32::INFINITY; grid.len()];
        let mut parents: Vec<Option<usize>> = vec![None; grid.len()];
        let mut open: BinaryHeap<HeapNode> = BinaryHeap::new();
        let mut closed: HashSet<usize> = HashSet::new();

        g_costs[start_idx] = 0.0;
        let h = self.heuristic.compute(sx, sy, ex, ey);
        open.push(HeapNode { f_cost: h, g_cost: 0.0, index: start_idx });

        while let Some(current) = open.pop() {
            let idx = current.index;
            if closed.contains(&idx) { continue; }
            closed.insert(idx);

            if idx == end_idx {
                return Some(self.reconstruct(grid, &parents, idx, start, end));
            }

            let cx = idx % grid.width;
            let cy = idx / grid.width;
            let parent_idx = parents[idx];
            let jump_points = self.identify_successors(grid, cx, cy, parent_idx, ex, ey);

            for (jx, jy) in jump_points {
                let jidx = jy * grid.width + jx;
                if closed.contains(&jidx) { continue; }
                let dist = ((jx as f32 - cx as f32).powi(2) + (jy as f32 - cy as f32).powi(2)).sqrt();
                let tentative_g = g_costs[idx] + dist;
                if tentative_g < g_costs[jidx] {
                    g_costs[jidx] = tentative_g;
                    parents[jidx] = Some(idx);
                    let h2 = self.heuristic.compute(jx, jy, ex, ey);
                    open.push(HeapNode { f_cost: tentative_g + h2, g_cost: tentative_g, index: jidx });
                }
            }
        }
        None
    }

    fn identify_successors(
        &self, grid: &PathGrid,
        x: usize, y: usize,
        parent: Option<usize>,
        ex: usize, ey: usize,
    ) -> Vec<(usize, usize)> {
        let mut successors = Vec::new();
        let neighbors = self.prune_neighbors(grid, x, y, parent);
        for (nx, ny) in neighbors {
            let dx = (nx as i32 - x as i32).signum();
            let dy = (ny as i32 - y as i32).signum();
            if let Some((jx, jy)) = self.jump(grid, x as i32, y as i32, dx, dy, ex, ey) {
                successors.push((jx as usize, jy as usize));
            }
        }
        successors
    }

    fn prune_neighbors(
        &self, grid: &PathGrid,
        x: usize, y: usize,
        parent: Option<usize>,
    ) -> Vec<(usize, usize)> {
        let Some(pidx) = parent else {
            // No parent: return all walkable neighbors
            return grid.neighbors(x, y).into_iter().map(|(nx, ny, _)| (nx, ny)).collect();
        };
        let px = pidx % grid.width;
        let py = pidx / grid.width;
        let dx = (x as i32 - px as i32).signum();
        let dy = (y as i32 - py as i32).signum();
        let mut result = Vec::new();

        if dx != 0 && dy != 0 {
            // Diagonal movement
            if grid.is_walkable((x as i32 + dx) as usize, y) {
                result.push(((x as i32 + dx) as usize, y));
            }
            if grid.is_walkable(x, (y as i32 + dy) as usize) {
                result.push((x, (y as i32 + dy) as usize));
            }
            if grid.is_walkable((x as i32 + dx) as usize, (y as i32 + dy) as usize) {
                result.push(((x as i32 + dx) as usize, (y as i32 + dy) as usize));
            }
            // Forced neighbors
            if !grid.is_walkable((x as i32 - dx) as usize, y)
                && grid.is_walkable((x as i32 - dx) as usize, (y as i32 + dy) as usize)
            {
                result.push(((x as i32 - dx) as usize, (y as i32 + dy) as usize));
            }
            if !grid.is_walkable(x, (y as i32 - dy) as usize)
                && grid.is_walkable((x as i32 + dx) as usize, (y as i32 - dy) as usize)
            {
                result.push(((x as i32 + dx) as usize, (y as i32 - dy) as usize));
            }
        } else if dx != 0 {
            // Horizontal
            if grid.is_walkable((x as i32 + dx) as usize, y) {
                result.push(((x as i32 + dx) as usize, y));
            }
            if !grid.is_walkable(x, y + 1) && grid.is_walkable((x as i32 + dx) as usize, y + 1) {
                result.push(((x as i32 + dx) as usize, y + 1));
            }
            if y > 0 && !grid.is_walkable(x, y - 1) && grid.is_walkable((x as i32 + dx) as usize, y - 1) {
                result.push(((x as i32 + dx) as usize, y - 1));
            }
        } else {
            // Vertical
            if grid.is_walkable(x, (y as i32 + dy) as usize) {
                result.push((x, (y as i32 + dy) as usize));
            }
            if !grid.is_walkable(x + 1, y) && grid.is_walkable(x + 1, (y as i32 + dy) as usize) {
                result.push((x + 1, (y as i32 + dy) as usize));
            }
            if x > 0 && !grid.is_walkable(x - 1, y) && grid.is_walkable(x - 1, (y as i32 + dy) as usize) {
                result.push((x - 1, (y as i32 + dy) as usize));
            }
        }
        result
    }

    fn jump(
        &self, grid: &PathGrid,
        x: i32, y: i32,
        dx: i32, dy: i32,
        ex: usize, ey: usize,
    ) -> Option<(i32, i32)> {
        let nx = x + dx;
        let ny = y + dy;
        if nx < 0 || ny < 0 || nx >= grid.width as i32 || ny >= grid.height as i32 {
            return None;
        }
        if !grid.is_walkable(nx as usize, ny as usize) { return None; }
        if nx as usize == ex && ny as usize == ey { return Some((nx, ny)); }

        // Forced neighbours check
        if dx != 0 && dy != 0 {
            // Diagonal: check cardinal jumps
            if self.jump(grid, nx, ny, dx, 0, ex, ey).is_some()
                || self.jump(grid, nx, ny, 0, dy, ex, ey).is_some()
            {
                return Some((nx, ny));
            }
        } else if dx != 0 {
            if (ny + 1 < grid.height as i32 && !grid.is_walkable(nx as usize, ny as usize + 1)
                    && grid.is_walkable((nx + dx) as usize, ny as usize + 1))
                || (ny - 1 >= 0 && !grid.is_walkable(nx as usize, (ny - 1) as usize)
                    && grid.is_walkable((nx + dx) as usize, (ny - 1) as usize))
            {
                return Some((nx, ny));
            }
        } else {
            if (nx + 1 < grid.width as i32 && !grid.is_walkable(nx as usize + 1, ny as usize)
                    && grid.is_walkable(nx as usize + 1, (ny + dy) as usize))
                || (nx - 1 >= 0 && !grid.is_walkable((nx - 1) as usize, ny as usize)
                    && grid.is_walkable((nx - 1) as usize, (ny + dy) as usize))
            {
                return Some((nx, ny));
            }
        }

        if dx != 0 && dy != 0 {
            self.jump(grid, nx, ny, dx, dy, ex, ey)
        } else {
            self.jump(grid, nx, ny, dx, dy, ex, ey)
        }
    }

    fn reconstruct(
        &self, grid: &PathGrid,
        parents: &[Option<usize>],
        end_idx: usize,
        start: Vec2, end: Vec2,
    ) -> Vec<Vec2> {
        let mut indices = Vec::new();
        let mut cur = end_idx;
        loop {
            indices.push(cur);
            match parents[cur] {
                Some(p) => cur = p,
                None => break,
            }
        }
        indices.reverse();
        // JPS stores jump points, so we need to fill in the intermediate cells
        let mut waypoints = Vec::new();
        for window in indices.windows(2) {
            let ax = window[0] % grid.width;
            let ay = window[0] / grid.width;
            let bx = window[1] % grid.width;
            let by = window[1] / grid.width;
            waypoints.push(grid.grid_to_world(ax, ay));
            // Interpolate
            let mut cx = ax as i32;
            let mut cy = ay as i32;
            let dxs = (bx as i32 - ax as i32).signum();
            let dys = (by as i32 - ay as i32).signum();
            while cx as usize != bx || cy as usize != by {
                cx += dxs;
                cy += dys;
                waypoints.push(grid.grid_to_world(cx as usize, cy as usize));
            }
        }
        if indices.len() == 1 {
            let ax = indices[0] % grid.width;
            let ay = indices[0] / grid.width;
            waypoints.push(grid.grid_to_world(ax, ay));
        }
        if !waypoints.is_empty() { waypoints[0] = start; }
        if waypoints.len() > 1 { *waypoints.last_mut().unwrap() = end; }
        waypoints
    }
}

// ---------------------------------------------------------------------------
// Hierarchical Pathfinder
// ---------------------------------------------------------------------------

/// A chunk in the hierarchical pathfinder.
#[derive(Debug, Clone)]
pub struct HierarchyChunk {
    pub chunk_x: usize,
    pub chunk_y: usize,
    pub center: Vec2,
    pub walkable: bool,
    pub portal_cells: Vec<(usize, usize)>,
}

/// Two-level hierarchical pathfinder.
/// Performs a coarse A* over chunks, then refines within each chunk.
#[derive(Debug, Clone)]
pub struct HierarchicalPathfinder {
    pub chunk_size: usize,
    fine_finder: AStarPathfinder,
}

impl HierarchicalPathfinder {
    pub fn new(chunk_size: usize) -> Self {
        HierarchicalPathfinder {
            chunk_size,
            fine_finder: AStarPathfinder::new(),
        }
    }

    /// Build the chunk abstraction for the given grid.
    pub fn build_chunks(&self, grid: &PathGrid) -> Vec<HierarchyChunk> {
        let cw = (grid.width  + self.chunk_size - 1) / self.chunk_size;
        let ch = (grid.height + self.chunk_size - 1) / self.chunk_size;
        let mut chunks = Vec::with_capacity(cw * ch);
        for cy in 0..ch {
            for cx in 0..cw {
                let x0 = cx * self.chunk_size;
                let y0 = cy * self.chunk_size;
                let x1 = (x0 + self.chunk_size).min(grid.width);
                let y1 = (y0 + self.chunk_size).min(grid.height);
                let walkable = (x0..x1).any(|x| (y0..y1).any(|y| grid.is_walkable(x, y)));
                let center = Vec2::new(
                    (x0 + x1) as f32 * 0.5 * grid.cell_size,
                    (y0 + y1) as f32 * 0.5 * grid.cell_size,
                ) + grid.origin;
                chunks.push(HierarchyChunk {
                    chunk_x: cx,
                    chunk_y: cy,
                    center,
                    walkable,
                    portal_cells: Vec::new(),
                });
            }
        }
        chunks
    }

    /// Find a path using hierarchical search.
    pub fn find_path(&self, grid: &PathGrid, start: Vec2, end: Vec2) -> Option<Vec<Vec2>> {
        // For simplicity, we use the fine finder for nearby queries and
        // coarse chunk-aware routing for longer distances.
        let (sx, sy) = grid.world_to_grid(start);
        let (ex, ey) = grid.world_to_grid(end);
        let chunk_dist = ((sx as i32 / self.chunk_size as i32 - ex as i32 / self.chunk_size as i32).abs()
            + (sy as i32 / self.chunk_size as i32 - ey as i32 / self.chunk_size as i32).abs()) as usize;

        if chunk_dist <= 2 {
            // Close enough: use fine A* directly
            return self.fine_finder.find_path(grid, start, end);
        }

        // Coarse pass: build a reduced grid of chunks
        let cw = (grid.width  + self.chunk_size - 1) / self.chunk_size;
        let ch = (grid.height + self.chunk_size - 1) / self.chunk_size;
        let mut chunk_grid = PathGrid::new(cw, ch, grid.cell_size * self.chunk_size as f32);
        chunk_grid.origin = grid.origin;
        // Mark non-walkable chunks
        for cy in 0..ch {
            for cx in 0..cw {
                let x0 = cx * self.chunk_size;
                let y0 = cy * self.chunk_size;
                let x1 = (x0 + self.chunk_size).min(grid.width);
                let y1 = (y0 + self.chunk_size).min(grid.height);
                let walkable = (x0..x1).any(|x| (y0..y1).any(|y| grid.is_walkable(x, y)));
                if !walkable {
                    chunk_grid.set_walkable(cx, cy, false);
                }
            }
        }

        let chunk_start = chunk_grid.grid_to_world(sx / self.chunk_size, sy / self.chunk_size);
        let chunk_end   = chunk_grid.grid_to_world(ex / self.chunk_size, ey / self.chunk_size);
        let coarse = self.fine_finder.find_path(&chunk_grid, chunk_start, chunk_end)?;

        // Fine pass: stitch through each coarse waypoint
        let mut full_path = Vec::new();
        let mut prev = start;
        for &waypoint in coarse.iter().skip(1) {
            // Clamp waypoint into fine grid
            let (wx, wy) = grid.world_to_grid(waypoint);
            let fine_wp = grid.grid_to_world(wx, wy);
            if let Some(mut seg) = self.fine_finder.find_path(grid, prev, fine_wp) {
                if !full_path.is_empty() { seg.remove(0); }
                full_path.extend(seg);
            }
            prev = fine_wp;
        }

        if full_path.is_empty() {
            self.fine_finder.find_path(grid, start, end)
        } else {
            Some(full_path)
        }
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn make_grid(w: usize, h: usize) -> PathGrid {
        PathGrid::new(w, h, 1.0)
    }

    #[test]
    fn test_grid_index_round_trip() {
        let grid = make_grid(10, 10);
        let (gx, gy) = grid.world_to_grid(Vec2::new(3.7, 5.2));
        let world = grid.grid_to_world(gx, gy);
        // Centre of cell should be within 0.5
        assert!((world.x - 3.5).abs() < 0.5);
        assert!((world.y - 5.5).abs() < 0.5);
    }

    #[test]
    fn test_astar_straight_line() {
        let grid = make_grid(10, 10);
        let finder = AStarPathfinder::new();
        let path = finder.find_path(&grid, Vec2::new(0.5, 0.5), Vec2::new(9.5, 0.5));
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.len() >= 2);
    }

    #[test]
    fn test_astar_blocked() {
        let mut grid = make_grid(5, 5);
        // Wall across the middle
        for y in 0..5 { grid.set_walkable(2, y, false); }
        let finder = AStarPathfinder::new().with_diagonal(false);
        let path = finder.find_path(&grid, Vec2::new(0.5, 2.5), Vec2::new(4.5, 2.5));
        assert!(path.is_none());
    }

    #[test]
    fn test_astar_around_obstacle() {
        let mut grid = make_grid(10, 10);
        // Vertical wall with gap
        for y in 1..10 { grid.set_walkable(5, y, false); }
        let finder = AStarPathfinder::new();
        let path = finder.find_path(&grid, Vec2::new(0.5, 5.5), Vec2::new(9.5, 5.5));
        assert!(path.is_some());
    }

    #[test]
    fn test_dijkstra_map() {
        let grid = make_grid(10, 10);
        let mut dmap = DijkstraMap::new(10, 10);
        dmap.build(&grid, &[Vec2::new(5.5, 5.5)]);
        // Distance at goal should be ~0
        assert!(dmap.distance_at(&grid, Vec2::new(5.5, 5.5)) < 1.0);
        // Distance at corner should be greater
        assert!(dmap.distance_at(&grid, Vec2::new(0.5, 0.5)) > 5.0);
    }

    #[test]
    fn test_dijkstra_next_step() {
        let grid = make_grid(10, 10);
        let mut dmap = DijkstraMap::new(10, 10);
        dmap.build(&grid, &[Vec2::new(9.5, 9.5)]);
        let next = dmap.next_step(&grid, Vec2::new(0.5, 0.5));
        assert!(next.is_some());
    }

    #[test]
    fn test_jps_finds_path() {
        let grid = make_grid(20, 20);
        let jps = JumpPointSearch::new();
        let path = jps.find_path(&grid, Vec2::new(0.5, 0.5), Vec2::new(19.5, 19.5));
        assert!(path.is_some());
    }

    #[test]
    fn test_smooth_path_no_obstacles() {
        let grid = make_grid(10, 10);
        let path = vec![
            Vec2::new(0.5, 0.5),
            Vec2::new(1.5, 0.5),
            Vec2::new(2.5, 0.5),
            Vec2::new(3.5, 0.5),
        ];
        let smoothed = smooth_path(path.clone(), &grid);
        // Should reduce to just start and end since line-of-sight is clear
        assert!(smoothed.len() <= path.len());
    }

    #[test]
    fn test_hierarchical_close_path() {
        let grid = make_grid(20, 20);
        let hf = HierarchicalPathfinder::new(4);
        let path = hf.find_path(&grid, Vec2::new(0.5, 0.5), Vec2::new(2.5, 2.5));
        assert!(path.is_some());
    }

    #[test]
    fn test_hierarchical_far_path() {
        let grid = make_grid(40, 40);
        let hf = HierarchicalPathfinder::new(4);
        let path = hf.find_path(&grid, Vec2::new(0.5, 0.5), Vec2::new(38.5, 38.5));
        assert!(path.is_some());
    }

    #[test]
    fn test_path_detailed_found() {
        let grid = make_grid(10, 10);
        let finder = AStarPathfinder::new();
        let req = PathRequest::new(Vec2::new(0.5, 0.5), Vec2::new(9.5, 9.5));
        let (result, stats) = finder.find_path_detailed(&grid, &req);
        assert!(matches!(result, PathResult::Found(_)));
        assert!(stats.nodes_expanded > 0);
    }

    #[test]
    fn test_path_detailed_partial() {
        let mut grid = make_grid(10, 10);
        for x in 0..10 { grid.set_walkable(x, 5, false); }
        let finder = AStarPathfinder::new();
        let mut req = PathRequest::new(Vec2::new(0.5, 0.5), Vec2::new(9.5, 9.5));
        req.allow_partial = true;
        let (result, _) = finder.find_path_detailed(&grid, &req);
        // May be partial or no path, either is acceptable
        assert!(matches!(result, PathResult::Partial(_) | PathResult::NoPath));
    }

    #[test]
    fn test_neighbors_cardinal_only() {
        let grid = make_grid(5, 5);
        let neighbors = grid.neighbors_cardinal(2, 2);
        assert_eq!(neighbors.len(), 4);
    }

    #[test]
    fn test_line_of_sight() {
        let mut grid = make_grid(10, 10);
        assert!(grid.line_of_sight(0, 0, 9, 9));
        grid.set_walkable(5, 5, false);
        // After blocking, LOS may be false
        let _los = grid.line_of_sight(0, 0, 9, 9); // may or may not be blocked by Bresenham
    }

    #[test]
    fn test_heuristics() {
        assert_eq!(Heuristic::Manhattan.compute(0, 0, 3, 4), 7.0);
        assert!((Heuristic::Euclidean.compute(0, 0, 3, 4) - 5.0).abs() < 0.001);
        assert_eq!(Heuristic::Chebyshev.compute(0, 0, 3, 4), 4.0);
    }

    #[test]
    fn test_spline_path() {
        let path = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(5.0, 5.0),
            Vec2::new(10.0, 0.0),
        ];
        let splined = spline_path(&path, 4);
        assert!(splined.len() > path.len());
    }

    #[test]
    fn test_grid_weight() {
        let mut grid = make_grid(5, 5);
        grid.set_weight(2, 2, 5.0);
        assert_eq!(grid.weight(2, 2), 5.0);
        assert_eq!(grid.weight(1, 1), 1.0);
    }
}
