// src/pathfinding/astar.rs
// A* and pathfinding variants:
//   - Generic A* over NodeId graph
//   - Jump Point Search (JPS) for uniform-cost grid maps
//   - Hierarchical A* with cluster-level precomputation
//   - Flow fields for crowd simulation
//   - Path caching with invalidation

use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Ordering;
use std::f32;

// ── Vec2 (local, avoids cross-module dep) ────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    #[inline] pub fn new(x: f32, y: f32) -> Self { Self { x, y } }
    #[inline] pub fn zero() -> Self { Self { x: 0.0, y: 0.0 } }
    #[inline] pub fn dist(self, o: Self) -> f32 { ((self.x-o.x).powi(2)+(self.y-o.y).powi(2)).sqrt() }
    #[inline] pub fn sub(self, o: Self) -> Self { Self::new(self.x-o.x, self.y-o.y) }
    #[inline] pub fn add(self, o: Self) -> Self { Self::new(self.x+o.x, self.y+o.y) }
    #[inline] pub fn scale(self, s: f32) -> Self { Self::new(self.x*s, self.y*s) }
    #[inline] pub fn len(self) -> f32 { (self.x*self.x+self.y*self.y).sqrt() }
    #[inline] pub fn norm(self) -> Self { let l=self.len(); if l<1e-9 {Self::zero()} else {self.scale(1.0/l)} }
}

// ── Node identifier ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

// ── Generic A* graph trait ────────────────────────────────────────────────────

/// Trait implemented by any graph that wants generic A*.
pub trait AStarGraph {
    type Cost: Copy + PartialOrd + std::ops::Add<Output = Self::Cost>;
    fn zero_cost() -> Self::Cost;
    fn max_cost() -> Self::Cost;
    fn heuristic(&self, from: NodeId, to: NodeId) -> Self::Cost;
    fn neighbors(&self, node: NodeId) -> Vec<(NodeId, Self::Cost)>;
}

/// Result of A* search.
#[derive(Clone, Debug)]
pub struct AStarResult {
    pub path: Vec<NodeId>,
    pub cost: f32,
}

pub struct AStarNode {
    pub id:       NodeId,
    pub position: Vec2,
    pub walkable: bool,
}

// ── Priority entry ────────────────────────────────────────────────────────────

#[derive(PartialEq)]
struct PqEntry<C: PartialOrd> {
    node: NodeId,
    f:    C,
}

impl<C: PartialOrd> Eq for PqEntry<C> {}

impl<C: PartialOrd> PartialOrd for PqEntry<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl<C: PartialOrd> Ord for PqEntry<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

/// Run generic A* on any graph implementing AStarGraph.
pub fn astar_search<G: AStarGraph>(
    graph: &G,
    start: NodeId,
    goal: NodeId,
) -> Option<AStarResult>
where
    G::Cost: std::fmt::Debug,
    f32: From<G::Cost>,
{
    let mut open: BinaryHeap<PqEntry<G::Cost>> = BinaryHeap::new();
    let mut came_from: HashMap<NodeId, NodeId> = HashMap::new();
    let mut g_score: HashMap<NodeId, G::Cost> = HashMap::new();

    g_score.insert(start, G::zero_cost());
    open.push(PqEntry { node: start, f: graph.heuristic(start, goal) });

    while let Some(PqEntry { node: current, .. }) = open.pop() {
        if current == goal {
            let path = reconstruct(start, goal, &came_from);
            let cost = f32::from(*g_score.get(&goal).unwrap_or(&G::zero_cost()));
            return Some(AStarResult { path, cost });
        }
        let cur_g = *g_score.get(&current).unwrap_or(&G::max_cost());
        for (neighbor, edge_cost) in graph.neighbors(current) {
            let tentative = cur_g + edge_cost;
            if tentative < *g_score.get(&neighbor).unwrap_or(&G::max_cost()) {
                came_from.insert(neighbor, current);
                g_score.insert(neighbor, tentative);
                let h = graph.heuristic(neighbor, goal);
                open.push(PqEntry { node: neighbor, f: tentative + h });
            }
        }
    }
    None
}

fn reconstruct(start: NodeId, goal: NodeId, came_from: &HashMap<NodeId, NodeId>) -> Vec<NodeId> {
    let mut path = Vec::new();
    let mut cur = goal;
    while cur != start {
        path.push(cur);
        match came_from.get(&cur) {
            Some(&p) => cur = p,
            None => break,
        }
    }
    path.push(start);
    path.reverse();
    path
}

// ── Grid map ──────────────────────────────────────────────────────────────────

/// A 2-D uniform grid map for JPS and flow fields.
#[derive(Clone, Debug)]
pub struct GridMap {
    pub width:    usize,
    pub height:   usize,
    pub cells:    Vec<bool>,  // true = walkable
    pub cell_size: f32,
    pub origin:   Vec2,
}

impl GridMap {
    pub fn new(width: usize, height: usize, cell_size: f32, origin: Vec2) -> Self {
        Self {
            width, height,
            cells: vec![true; width * height],
            cell_size, origin,
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    #[inline]
    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.in_bounds(x, y) && self.cells[self.idx(x as usize, y as usize)]
    }

    pub fn set_walkable(&mut self, x: usize, y: usize, w: bool) {
        let i = self.idx(x, y);
        self.cells[i] = w;
    }

    /// Block a rectangular area.
    pub fn block_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        for ry in y..((y+h).min(self.height)) {
            for rx in x..((x+w).min(self.width)) {
                let i = self.idx(rx, ry);
                self.cells[i] = false;
            }
        }
    }

    pub fn node_id(&self, x: usize, y: usize) -> NodeId {
        NodeId((y * self.width + x) as u32)
    }

    pub fn coords(&self, id: NodeId) -> (usize, usize) {
        let i = id.0 as usize;
        (i % self.width, i / self.width)
    }

    pub fn world_pos(&self, x: usize, y: usize) -> Vec2 {
        Vec2::new(
            self.origin.x + (x as f32 + 0.5) * self.cell_size,
            self.origin.y + (y as f32 + 0.5) * self.cell_size,
        )
    }

    pub fn grid_coords_for_world(&self, p: Vec2) -> Option<(usize, usize)> {
        let gx = ((p.x - self.origin.x) / self.cell_size) as i32;
        let gy = ((p.y - self.origin.y) / self.cell_size) as i32;
        if self.in_bounds(gx, gy) {
            Some((gx as usize, gy as usize))
        } else {
            None
        }
    }
}

// ── Jump Point Search ─────────────────────────────────────────────────────────

/// JPS pathfinder for uniform-cost grid maps (8-directional movement).
pub struct JpsPathfinder<'a> {
    pub grid: &'a GridMap,
}

impl<'a> JpsPathfinder<'a> {
    pub fn new(grid: &'a GridMap) -> Self { Self { grid } }

    /// Find a path from `start` to `goal`, both as grid (x,y) coordinates.
    pub fn find_path(&self, start: (usize, usize), goal: (usize, usize)) -> Option<Vec<(usize, usize)>> {
        if !self.grid.walkable(start.0 as i32, start.1 as i32) { return None; }
        if !self.grid.walkable(goal.0 as i32, goal.1 as i32) { return None; }
        if start == goal { return Some(vec![start]); }

        let mut open: BinaryHeap<JpsEntry> = BinaryHeap::new();
        let mut came_from: HashMap<(usize,usize), (usize,usize)> = HashMap::new();
        let mut g: HashMap<(usize,usize), f32> = HashMap::new();
        let mut closed: HashSet<(usize,usize)> = HashSet::new();

        g.insert(start, 0.0);
        open.push(JpsEntry { pos: start, f: self.h(start, goal) });

        while let Some(JpsEntry { pos: cur, .. }) = open.pop() {
            if cur == goal {
                return Some(self.reconstruct_path(start, goal, &came_from));
            }
            if closed.contains(&cur) { continue; }
            closed.insert(cur);

            let cur_g = *g.get(&cur).unwrap_or(&f32::MAX);
            let successors = self.identify_successors(cur, goal, &came_from);

            for succ in successors {
                if closed.contains(&succ) { continue; }
                let d = self.cost(cur, succ);
                let ng = cur_g + d;
                if ng < *g.get(&succ).unwrap_or(&f32::MAX) {
                    g.insert(succ, ng);
                    came_from.insert(succ, cur);
                    open.push(JpsEntry { pos: succ, f: ng + self.h(succ, goal) });
                }
            }
        }
        None
    }

    fn h(&self, a: (usize,usize), b: (usize,usize)) -> f32 {
        let dx = (a.0 as f32 - b.0 as f32).abs();
        let dy = (a.1 as f32 - b.1 as f32).abs();
        // Octile distance
        let (mn, mx) = if dx < dy { (dx, dy) } else { (dy, dx) };
        mx + mn * (std::f32::consts::SQRT_2 - 1.0)
    }

    fn cost(&self, a: (usize,usize), b: (usize,usize)) -> f32 {
        let dx = (a.0 as i32 - b.0 as i32).abs();
        let dy = (a.1 as i32 - b.1 as i32).abs();
        if dx + dy == 2 { std::f32::consts::SQRT_2 } else { 1.0 }
    }

    fn identify_successors(
        &self,
        node: (usize,usize),
        goal: (usize,usize),
        came_from: &HashMap<(usize,usize),(usize,usize)>,
    ) -> Vec<(usize,usize)> {
        let neighbors = self.prune_neighbors(node, came_from);
        let mut successors = Vec::new();
        for nb in neighbors {
            let dx = (nb.0 as i32 - node.0 as i32).signum();
            let dy = (nb.1 as i32 - node.1 as i32).signum();
            if let Some(jp) = self.jump(node, (dx, dy), goal) {
                successors.push(jp);
            }
        }
        successors
    }

    fn prune_neighbors(
        &self,
        node: (usize,usize),
        came_from: &HashMap<(usize,usize),(usize,usize)>,
    ) -> Vec<(usize,usize)> {
        let parent = came_from.get(&node);
        let (x, y) = (node.0 as i32, node.1 as i32);
        if parent.is_none() {
            // Start node: return all walkable neighbors
            return self.all_neighbors(node);
        }
        let parent = parent.unwrap();
        let dx = (x - parent.0 as i32).signum();
        let dy = (y - parent.1 as i32).signum();
        let mut neighbors = Vec::new();

        if dx != 0 && dy != 0 {
            // Diagonal
            if self.grid.walkable(x, y + dy)     { neighbors.push((x as usize, (y+dy) as usize)); }
            if self.grid.walkable(x + dx, y)     { neighbors.push(((x+dx) as usize, y as usize)); }
            if self.grid.walkable(x + dx, y + dy) { neighbors.push(((x+dx) as usize, (y+dy) as usize)); }
            if !self.grid.walkable(x - dx, y) && self.grid.walkable(x, y + dy) {
                neighbors.push((x as usize, (y + dy) as usize));
            }
            if !self.grid.walkable(x, y - dy) && self.grid.walkable(x + dx, y) {
                neighbors.push(((x + dx) as usize, y as usize));
            }
        } else if dx != 0 {
            // Horizontal
            if self.grid.walkable(x + dx, y) { neighbors.push(((x+dx) as usize, y as usize)); }
            if !self.grid.walkable(x, y + 1) && self.grid.walkable(x + dx, y + 1) {
                neighbors.push(((x+dx) as usize, (y+1) as usize));
            }
            if !self.grid.walkable(x, y - 1) && self.grid.walkable(x + dx, y - 1) {
                neighbors.push(((x+dx) as usize, (y-1) as usize));
            }
        } else {
            // Vertical
            if self.grid.walkable(x, y + dy) { neighbors.push((x as usize, (y+dy) as usize)); }
            if !self.grid.walkable(x + 1, y) && self.grid.walkable(x + 1, y + dy) {
                neighbors.push(((x+1) as usize, (y+dy) as usize));
            }
            if !self.grid.walkable(x - 1, y) && self.grid.walkable(x - 1, y + dy) {
                neighbors.push(((x-1) as usize, (y+dy) as usize));
            }
        }
        neighbors.dedup();
        neighbors
    }

    fn all_neighbors(&self, node: (usize,usize)) -> Vec<(usize,usize)> {
        let (x, y) = (node.0 as i32, node.1 as i32);
        let mut result = Vec::new();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 { continue; }
                if self.grid.walkable(x + dx, y + dy) {
                    result.push(((x + dx) as usize, (y + dy) as usize));
                }
            }
        }
        result
    }

    fn jump(&self, node: (usize,usize), dir: (i32,i32), goal: (usize,usize)) -> Option<(usize,usize)> {
        let (mut x, mut y) = (node.0 as i32, node.1 as i32);
        let (dx, dy) = dir;
        let max_steps = (self.grid.width + self.grid.height) * 2;
        let mut steps = 0;

        loop {
            x += dx;
            y += dy;
            steps += 1;
            if steps > max_steps { return None; }
            if !self.grid.walkable(x, y) { return None; }
            let cur = (x as usize, y as usize);
            if cur == goal { return Some(cur); }

            // Check for forced neighbors
            if self.has_forced_neighbor(cur, dir) { return Some(cur); }

            // Diagonal: recurse on both cardinal directions
            if dx != 0 && dy != 0 {
                if self.jump((x as usize, y as usize), (dx, 0), goal).is_some() { return Some(cur); }
                if self.jump((x as usize, y as usize), (0, dy), goal).is_some() { return Some(cur); }
            }
        }
    }

    fn has_forced_neighbor(&self, node: (usize,usize), dir: (i32,i32)) -> bool {
        let (x, y) = (node.0 as i32, node.1 as i32);
        let (dx, dy) = dir;
        if dx != 0 && dy != 0 {
            // diagonal forced: blocked adjacent cardinal
            (!self.grid.walkable(x - dx, y) && self.grid.walkable(x - dx, y + dy))
            || (!self.grid.walkable(x, y - dy) && self.grid.walkable(x + dx, y - dy))
        } else if dx != 0 {
            (!self.grid.walkable(x, y + 1) && self.grid.walkable(x + dx, y + 1))
            || (!self.grid.walkable(x, y - 1) && self.grid.walkable(x + dx, y - 1))
        } else {
            (!self.grid.walkable(x + 1, y) && self.grid.walkable(x + 1, y + dy))
            || (!self.grid.walkable(x - 1, y) && self.grid.walkable(x - 1, y + dy))
        }
    }

    fn reconstruct_path(
        &self,
        start: (usize,usize),
        goal: (usize,usize),
        came_from: &HashMap<(usize,usize),(usize,usize)>,
    ) -> Vec<(usize,usize)> {
        let mut path = Vec::new();
        let mut cur = goal;
        while cur != start {
            path.push(cur);
            match came_from.get(&cur) {
                Some(&p) => cur = p,
                None => break,
            }
        }
        path.push(start);
        path.reverse();
        // Expand jump-point path into full grid steps
        let mut expanded = Vec::new();
        for i in 0..path.len().saturating_sub(1) {
            expanded.push(path[i]);
            let (ax, ay) = (path[i].0 as i32, path[i].1 as i32);
            let (bx, by) = (path[i+1].0 as i32, path[i+1].1 as i32);
            let sdx = (bx - ax).signum();
            let sdy = (by - ay).signum();
            let mut cx = ax + sdx;
            let mut cy = ay + sdy;
            while (cx, cy) != (bx, by) {
                expanded.push((cx as usize, cy as usize));
                cx += sdx;
                cy += sdy;
            }
        }
        if let Some(&last) = path.last() { expanded.push(last); }
        expanded.dedup();
        expanded
    }
}

#[derive(PartialEq)]
struct JpsEntry { pos: (usize,usize), f: f32 }
impl Eq for JpsEntry {}
impl PartialOrd for JpsEntry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
}
impl Ord for JpsEntry {
    fn cmp(&self, o: &Self) -> Ordering { o.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal) }
}

// ── Hierarchical A* ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterId(pub u32);

/// A cluster groups nearby grid cells for hierarchical planning.
#[derive(Clone, Debug)]
pub struct Cluster {
    pub id:            ClusterId,
    pub x:             usize,   // grid cell offset
    pub y:             usize,
    pub width:         usize,
    pub height:        usize,
    pub entry_cells:   Vec<(usize, usize)>,   // border cells that connect to other clusters
    pub neighbors:     Vec<(ClusterId, f32)>, // neighbor cluster + estimated cost
}

impl Cluster {
    pub fn contains(&self, cx: usize, cy: usize) -> bool {
        cx >= self.x && cx < self.x + self.width
        && cy >= self.y && cy < self.y + self.height
    }
    pub fn center_cell(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

/// Hierarchical pathfinder: builds abstract cluster graph then refines.
pub struct HierarchicalPathfinder {
    pub clusters:      Vec<Cluster>,
    pub cluster_map:   Vec<Option<ClusterId>>,  // per grid cell
    pub grid_width:    usize,
    pub grid_height:   usize,
    pub cluster_size:  usize,
}

impl HierarchicalPathfinder {
    /// Build cluster graph from a GridMap with given cluster size.
    pub fn build(grid: &GridMap, cluster_size: usize) -> Self {
        let cw = (grid.width  + cluster_size - 1) / cluster_size;
        let ch = (grid.height + cluster_size - 1) / cluster_size;
        let mut clusters = Vec::with_capacity(cw * ch);
        let mut cluster_map = vec![None; grid.width * grid.height];
        let mut id = 0u32;

        for cy in 0..ch {
            for cx in 0..cw {
                let ox = cx * cluster_size;
                let oy = cy * cluster_size;
                let w  = cluster_size.min(grid.width  - ox);
                let h  = cluster_size.min(grid.height - oy);

                let mut entry_cells = Vec::new();
                // Top/bottom border
                for bx in ox..(ox+w) {
                    if grid.walkable(bx as i32, oy as i32)        { entry_cells.push((bx, oy)); }
                    let by = oy + h - 1;
                    if grid.walkable(bx as i32, by as i32)        { entry_cells.push((bx, by)); }
                }
                // Left/right border
                for by in oy..(oy+h) {
                    if grid.walkable(ox as i32, by as i32)        { entry_cells.push((ox, by)); }
                    let bx = ox + w - 1;
                    if grid.walkable(bx as i32, by as i32)        { entry_cells.push((bx, by)); }
                }
                entry_cells.sort();
                entry_cells.dedup();

                let cid = ClusterId(id);
                for gx in ox..(ox+w) {
                    for gy in oy..(oy+h) {
                        if grid.in_bounds(gx as i32, gy as i32) {
                            let gi = gy * grid.width + gx;
                            cluster_map[gi] = Some(cid);
                        }
                    }
                }
                clusters.push(Cluster {
                    id: cid, x: ox, y: oy, width: w, height: h,
                    entry_cells, neighbors: Vec::new(),
                });
                id += 1;
            }
        }

        // Build neighbor edges between adjacent clusters
        let mut hpf = HierarchicalPathfinder {
            clusters, cluster_map,
            grid_width: grid.width,
            grid_height: grid.height,
            cluster_size,
        };
        hpf.build_cluster_edges(grid);
        hpf
    }

    fn build_cluster_edges(&mut self, grid: &GridMap) {
        let nc = self.clusters.len();
        for i in 0..nc {
            let ci = &self.clusters[i];
            // Check 4 adjacent cluster positions
            let (cx, cy) = (ci.x, ci.y);
            let cs = self.cluster_size;
            let adj_offsets: [(i32,i32); 4] = [(1,0),(-1,0),(0,1),(0,-1)];
            let mut neighbors = Vec::new();
            for (aox, aoy) in adj_offsets {
                let nx = cx as i32 + aox * cs as i32;
                let ny = cy as i32 + aoy * cs as i32;
                if nx < 0 || ny < 0 { continue; }
                if let Some(j) = self.find_cluster_at(nx as usize, ny as usize) {
                    if j != i {
                        let cost = cs as f32; // approximate
                        neighbors.push((self.clusters[j].id, cost));
                    }
                }
            }
            // Update neighbors (can't borrow mut + immut simultaneously, so rebuild)
            let _ = neighbors; // will be set below
        }
        // Simplified: link adjacent grid clusters
        let cw = (grid.width  + self.cluster_size - 1) / self.cluster_size;
        let ch = (grid.height + self.cluster_size - 1) / self.cluster_size;
        for cy in 0..ch {
            for cx in 0..cw {
                let idx = cy * cw + cx;
                if idx >= self.clusters.len() { continue; }
                let mut nbrs = Vec::new();
                let pairs: [(i32,i32); 4] = [(1,0),(-1,0),(0,1),(0,-1)];
                for (ddx, ddy) in pairs {
                    let ncx = cx as i32 + ddx;
                    let ncy = cy as i32 + ddy;
                    if ncx < 0 || ncy < 0 || ncx >= cw as i32 || ncy >= ch as i32 { continue; }
                    let nidx = (ncy as usize) * cw + (ncx as usize);
                    if nidx < self.clusters.len() {
                        let nid = self.clusters[nidx].id;
                        nbrs.push((nid, self.cluster_size as f32));
                    }
                }
                self.clusters[idx].neighbors = nbrs;
            }
        }
    }

    fn find_cluster_at(&self, x: usize, y: usize) -> Option<usize> {
        self.clusters.iter().position(|c| c.contains(x, y))
    }

    pub fn cluster_for_cell(&self, x: usize, y: usize) -> Option<ClusterId> {
        if x >= self.grid_width || y >= self.grid_height { return None; }
        self.cluster_map[y * self.grid_width + x]
    }

    /// High-level path: returns sequence of ClusterIds.
    pub fn abstract_path(&self, start_cell: (usize,usize), goal_cell: (usize,usize)) -> Vec<ClusterId> {
        let sc = match self.cluster_for_cell(start_cell.0, start_cell.1) { Some(c) => c, None => return Vec::new() };
        let gc = match self.cluster_for_cell(goal_cell.0, goal_cell.1) { Some(c) => c, None => return Vec::new() };
        if sc == gc { return vec![sc]; }

        let mut open: BinaryHeap<ClusterEntry> = BinaryHeap::new();
        let mut came_from: HashMap<ClusterId, ClusterId> = HashMap::new();
        let mut g: HashMap<ClusterId, f32> = HashMap::new();

        g.insert(sc, 0.0);
        open.push(ClusterEntry { id: sc, f: self.cluster_heuristic(sc, gc) });

        while let Some(ClusterEntry { id: cur, .. }) = open.pop() {
            if cur == gc {
                let mut path = Vec::new();
                let mut c = gc;
                while c != sc {
                    path.push(c);
                    c = *came_from.get(&c).unwrap_or(&sc);
                }
                path.push(sc);
                path.reverse();
                return path;
            }
            let cur_g = *g.get(&cur).unwrap_or(&f32::MAX);
            if let Some(cluster) = self.clusters.iter().find(|c| c.id == cur) {
                for &(nid, edge_cost) in &cluster.neighbors {
                    let ng = cur_g + edge_cost;
                    if ng < *g.get(&nid).unwrap_or(&f32::MAX) {
                        g.insert(nid, ng);
                        came_from.insert(nid, cur);
                        let h = self.cluster_heuristic(nid, gc);
                        open.push(ClusterEntry { id: nid, f: ng + h });
                    }
                }
            }
        }
        Vec::new()
    }

    fn cluster_heuristic(&self, a: ClusterId, b: ClusterId) -> f32 {
        let ca = self.clusters.iter().find(|c| c.id == a).map(|c| c.center_cell()).unwrap_or((0,0));
        let cb = self.clusters.iter().find(|c| c.id == b).map(|c| c.center_cell()).unwrap_or((0,0));
        let dx = (ca.0 as f32 - cb.0 as f32).abs();
        let dy = (ca.1 as f32 - cb.1 as f32).abs();
        dx.max(dy)
    }
}

#[derive(PartialEq)]
struct ClusterEntry { id: ClusterId, f: f32 }
impl Eq for ClusterEntry {}
impl PartialOrd for ClusterEntry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
}
impl Ord for ClusterEntry {
    fn cmp(&self, o: &Self) -> Ordering { o.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal) }
}

// ── Flow Field ────────────────────────────────────────────────────────────────

/// Flow direction per cell: an 8-directional flow vector.
#[derive(Clone, Copy, Debug, Default)]
pub struct FlowVector {
    pub dx: i8,   // -1, 0, +1
    pub dy: i8,
}

impl FlowVector {
    pub fn as_vec2(self) -> Vec2 {
        Vec2::new(self.dx as f32, self.dy as f32).norm()
    }
    pub fn is_valid(self) -> bool { self.dx != 0 || self.dy != 0 }
}

/// A flow field: precomputed for a single goal, steers any number of agents.
#[derive(Clone, Debug)]
pub struct FlowField {
    pub width:   usize,
    pub height:  usize,
    pub flow:    Vec<FlowVector>,
    pub cost:    Vec<f32>,          // integration field (distance to goal)
    pub goal:    (usize, usize),
}

/// Flow field grid: builds and stores flow fields.
pub struct FlowFieldGrid<'a> {
    pub grid: &'a GridMap,
}

impl<'a> FlowFieldGrid<'a> {
    pub fn new(grid: &'a GridMap) -> Self { Self { grid } }

    /// Build a flow field toward `goal` using Dijkstra integration.
    pub fn build(&self, goal: (usize, usize)) -> FlowField {
        let w = self.grid.width;
        let h = self.grid.height;
        let inf = f32::MAX / 2.0;
        let mut cost = vec![inf; w * h];
        let mut flow = vec![FlowVector::default(); w * h];

        if !self.grid.walkable(goal.0 as i32, goal.1 as i32) {
            return FlowField { width: w, height: h, flow, cost, goal };
        }

        let gi = goal.1 * w + goal.0;
        cost[gi] = 0.0;

        // BFS/Dijkstra integration field
        let mut queue: VecDeque<(usize,usize)> = VecDeque::new();
        queue.push_back(goal);

        while let Some((cx, cy)) = queue.pop_front() {
            let cur_cost = cost[cy * w + cx];
            for (dx, dy) in DIRS_8 {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if !self.grid.walkable(nx, ny) { continue; }
                let (nxi, nyi) = (nx as usize, ny as usize);
                let ni = nyi * w + nxi;
                let step_cost = if dx != 0 && dy != 0 { std::f32::consts::SQRT_2 } else { 1.0 };
                let nc = cur_cost + step_cost;
                if nc < cost[ni] {
                    cost[ni] = nc;
                    queue.push_back((nxi, nyi));
                }
            }
        }

        // Build flow vectors: each cell points toward the lowest-cost neighbor
        for cy in 0..h {
            for cx in 0..w {
                if !self.grid.walkable(cx as i32, cy as i32) { continue; }
                let ci = cy * w + cx;
                if cost[ci] >= inf { continue; }

                let mut best_cost = cost[ci];
                let mut best_dx = 0i8;
                let mut best_dy = 0i8;
                for (dx, dy) in DIRS_8 {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if !self.grid.walkable(nx, ny) { continue; }
                    let ni = ny as usize * w + nx as usize;
                    if cost[ni] < best_cost {
                        best_cost = cost[ni];
                        best_dx = dx as i8;
                        best_dy = dy as i8;
                    }
                }
                flow[ci] = FlowVector { dx: best_dx, dy: best_dy };
            }
        }

        FlowField { width: w, height: h, flow, cost, goal }
    }
}

const DIRS_8: [(i32,i32); 8] = [
    (1,0),(-1,0),(0,1),(0,-1),(1,1),(1,-1),(-1,1),(-1,-1)
];

impl FlowField {
    /// Get the flow vector at grid cell (x, y).
    pub fn get_flow(&self, x: usize, y: usize) -> FlowVector {
        if x < self.width && y < self.height {
            self.flow[y * self.width + x]
        } else {
            FlowVector::default()
        }
    }

    /// Get the integration cost at grid cell (x, y).
    pub fn get_cost(&self, x: usize, y: usize) -> f32 {
        if x < self.width && y < self.height {
            self.cost[y * self.width + x]
        } else {
            f32::MAX
        }
    }

    /// Sample the flow direction at world position `p` (grid-space integer lookup).
    pub fn sample(&self, gx: usize, gy: usize) -> Vec2 {
        self.get_flow(gx, gy).as_vec2()
    }
}

// ── Path cache with invalidation ──────────────────────────────────────────────

/// A cached path entry.
#[derive(Clone, Debug)]
pub struct CachedPath {
    pub start:   (usize, usize),
    pub goal:    (usize, usize),
    pub path:    Vec<(usize, usize)>,
    pub version: u64,
}

/// Cache of computed paths, invalidated when the grid changes.
pub struct PathCache {
    entries:       HashMap<((usize,usize),(usize,usize)), CachedPath>,
    pub version:   u64,
    capacity:      usize,
    // LRU tracking via insertion order
    order:         VecDeque<((usize,usize),(usize,usize))>,
}

impl PathCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            version: 0,
            capacity,
            order: VecDeque::new(),
        }
    }

    /// Increment version, invalidating all stale cache entries.
    pub fn invalidate(&mut self) {
        self.version += 1;
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    /// Look up a cached path; returns None if not present or stale.
    pub fn get(&self, start: (usize,usize), goal: (usize,usize)) -> Option<&Vec<(usize,usize)>> {
        let key = (start, goal);
        let entry = self.entries.get(&key)?;
        if entry.version == self.version {
            Some(&entry.path)
        } else {
            None
        }
    }

    /// Store a path in the cache, evicting LRU entry if over capacity.
    pub fn insert(&mut self, start: (usize,usize), goal: (usize,usize), path: Vec<(usize,usize)>) {
        let key = (start, goal);
        if self.entries.contains_key(&key) {
            self.entries.get_mut(&key).unwrap().path = path;
            self.entries.get_mut(&key).unwrap().version = self.version;
        } else {
            if self.entries.len() >= self.capacity {
                if let Some(evict_key) = self.order.pop_front() {
                    self.entries.remove(&evict_key);
                }
            }
            self.entries.insert(key, CachedPath { start, goal, path, version: self.version });
            self.order.push_back(key);
        }
    }

    /// Get or compute a path, using JPS if not cached.
    pub fn get_or_compute<'g>(&mut self, grid: &'g GridMap, start: (usize,usize), goal: (usize,usize)) -> Vec<(usize,usize)> {
        if let Some(cached) = self.get(start, goal) {
            return cached.clone();
        }
        let jps = JpsPathfinder::new(grid);
        let path = jps.find_path(start, goal).unwrap_or_default();
        self.insert(start, goal, path.clone());
        path
    }

    pub fn entry_count(&self) -> usize { self.entries.len() }
}

// ── Simple concrete graph for generic A* ─────────────────────────────────────

/// Simple flat graph with node positions and weighted edges.
pub struct SimpleGraph {
    pub nodes:     Vec<Vec2>,
    pub edges:     Vec<Vec<(NodeId, f32)>>,
}

impl SimpleGraph {
    pub fn new() -> Self { Self { nodes: Vec::new(), edges: Vec::new() } }

    pub fn add_node(&mut self, pos: Vec2) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(pos);
        self.edges.push(Vec::new());
        id
    }

    pub fn add_edge(&mut self, a: NodeId, b: NodeId, cost: f32) {
        let ai = a.0 as usize;
        let bi = b.0 as usize;
        if ai < self.edges.len() { self.edges[ai].push((b, cost)); }
        if bi < self.edges.len() { self.edges[bi].push((a, cost)); }
    }
}

impl AStarGraph for SimpleGraph {
    type Cost = f32;
    fn zero_cost() -> f32 { 0.0 }
    fn max_cost() -> f32  { f32::MAX / 2.0 }
    fn heuristic(&self, from: NodeId, to: NodeId) -> f32 {
        let a = self.nodes.get(from.0 as usize).copied().unwrap_or(Vec2::zero());
        let b = self.nodes.get(to.0  as usize).copied().unwrap_or(Vec2::zero());
        a.dist(b)
    }
    fn neighbors(&self, node: NodeId) -> Vec<(NodeId, f32)> {
        self.edges.get(node.0 as usize).cloned().unwrap_or_default()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_astar_simple() {
        let mut g = SimpleGraph::new();
        let a = g.add_node(Vec2::new(0.0, 0.0));
        let b = g.add_node(Vec2::new(1.0, 0.0));
        let c = g.add_node(Vec2::new(2.0, 0.0));
        g.add_edge(a, b, 1.0);
        g.add_edge(b, c, 1.0);
        let res = astar_search(&g, a, c).unwrap();
        assert_eq!(res.path, vec![a, b, c]);
        assert!((res.cost - 2.0).abs() < 1e-4);
    }

    #[test]
    fn test_jps_straight() {
        let mut grid = GridMap::new(10, 10, 1.0, Vec2::zero());
        let jps = JpsPathfinder::new(&grid);
        let path = jps.find_path((0,0), (5,0)).unwrap();
        assert!(!path.is_empty());
        assert_eq!(path[0], (0,0));
        assert_eq!(*path.last().unwrap(), (5,0));
    }

    #[test]
    fn test_jps_with_obstacle() {
        let mut grid = GridMap::new(10, 10, 1.0, Vec2::zero());
        // Wall in the middle
        for y in 0..8 { grid.set_walkable(5, y, false); }
        let jps = JpsPathfinder::new(&grid);
        let path = jps.find_path((0,5), (9,5));
        // Should find path around wall
        assert!(path.is_some());
    }

    #[test]
    fn test_flow_field() {
        let grid = GridMap::new(8, 8, 1.0, Vec2::zero());
        let ffg = FlowFieldGrid::new(&grid);
        let ff = ffg.build((7, 7));
        // Cell (0,0) should have valid flow toward goal
        let fv = ff.get_flow(0, 0);
        assert!(fv.is_valid());
    }

    #[test]
    fn test_path_cache() {
        let grid = GridMap::new(10, 10, 1.0, Vec2::zero());
        let mut cache = PathCache::new(16);
        let path = cache.get_or_compute(&grid, (0,0), (9,9));
        assert!(!path.is_empty());
        // Second call should hit cache
        let path2 = cache.get_or_compute(&grid, (0,0), (9,9));
        assert_eq!(path, path2);
        // After invalidation, cache entry is stale
        cache.invalidate();
        let cached = cache.get((0,0), (9,9));
        assert!(cached.is_none());
    }

    #[test]
    fn test_hierarchical_abstract_path() {
        let grid = GridMap::new(16, 16, 1.0, Vec2::zero());
        let hpf = HierarchicalPathfinder::build(&grid, 4);
        let abstract_path = hpf.abstract_path((0,0), (15,15));
        assert!(!abstract_path.is_empty());
    }
}
