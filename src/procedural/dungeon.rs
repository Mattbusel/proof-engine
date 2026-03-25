//! Dungeon generation — BSP, cellular automata, WFC, room placement, maze algorithms.
//!
//! This module provides multiple dungeon-generation strategies plus shared graph
//! utilities (BFS shortest-path, Kruskal MST, connected-components).  All
//! generators consume a `super::Rng` so results are fully deterministic from a
//! seed.

use super::Rng;
use std::collections::{HashMap, HashSet, VecDeque};
use glam::IVec2;

// ── Theme ─────────────────────────────────────────────────────────────────────

/// Visual theme for a dungeon floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DungeonTheme {
    Cave,
    Cathedral,
    Laboratory,
    Temple,
    Ruins,
    Void,
}

impl DungeonTheme {
    pub fn floor_glyphs(self) -> &'static [char] {
        match self {
            DungeonTheme::Cave       => &['.', ',', '\''],
            DungeonTheme::Cathedral  => &['+', '.', '\u{00B7}'],
            DungeonTheme::Laboratory => &['.', ':', '\u{00B7}'],
            DungeonTheme::Temple     => &['\u{256C}', '\u{256A}', '\u{256B}', '.'],
            DungeonTheme::Ruins      => &['.', ',', '~', '"'],
            DungeonTheme::Void       => &['.', '\u{00B7}', '\u{2218}', '\u{00B0}'],
        }
    }

    pub fn wall_glyphs(self) -> &'static [char] {
        match self {
            DungeonTheme::Cave       => &['#', '\u{2588}', '\u{2593}'],
            DungeonTheme::Cathedral  => &['\u{2588}', '\u{2593}', '\u{2502}', '\u{2500}'],
            DungeonTheme::Laboratory => &['\u{2588}', '\u{2593}', '\u{2554}', '\u{2557}'],
            DungeonTheme::Temple     => &['\u{2588}', '\u{2593}', '\u{2560}', '\u{2563}'],
            DungeonTheme::Ruins      => &['#', '\u{2593}', '%'],
            DungeonTheme::Void       => &['\u{2593}', '\u{2591}', '\u{2592}'],
        }
    }

    pub fn ambient_color(self) -> glam::Vec4 {
        match self {
            DungeonTheme::Cave       => glam::Vec4::new(0.4, 0.3, 0.2, 1.0),
            DungeonTheme::Cathedral  => glam::Vec4::new(0.6, 0.5, 0.8, 1.0),
            DungeonTheme::Laboratory => glam::Vec4::new(0.3, 0.8, 0.4, 1.0),
            DungeonTheme::Temple     => glam::Vec4::new(0.8, 0.6, 0.2, 1.0),
            DungeonTheme::Ruins      => glam::Vec4::new(0.5, 0.5, 0.4, 1.0),
            DungeonTheme::Void       => glam::Vec4::new(0.1, 0.0, 0.2, 1.0),
        }
    }
}

// ── IRect ─────────────────────────────────────────────────────────────────────

/// Axis-aligned integer rectangle in tile space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self { Self { x, y, w, h } }

    pub fn center(&self) -> IVec2 {
        IVec2::new(self.x + self.w / 2, self.y + self.h / 2)
    }

    pub fn center_tuple(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    pub fn contains(&self, tx: i32, ty: i32) -> bool {
        tx >= self.x && tx < self.x + self.w && ty >= self.y && ty < self.y + self.h
    }

    pub fn overlaps(&self, other: &IRect) -> bool {
        self.x < other.x + other.w && self.x + self.w > other.x
            && self.y < other.y + other.h && self.y + self.h > other.y
    }

    pub fn area(&self) -> i32 { self.w * self.h }

    pub fn shrink(&self, margin: i32) -> Option<IRect> {
        let nw = self.w - margin * 2;
        let nh = self.h - margin * 2;
        if nw < 1 || nh < 1 { return None; }
        Some(IRect::new(self.x + margin, self.y + margin, nw, nh))
    }

    pub fn expand(&self, amount: i32) -> IRect {
        IRect::new(self.x - amount, self.y - amount, self.w + amount * 2, self.h + amount * 2)
    }
}

// ── Tile ──────────────────────────────────────────────────────────────────────

/// Tile type in the dungeon grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Wall,
    Floor,
    Door,
    Corridor,
    Stairs,
    Void,
}

impl Tile {
    pub fn is_walkable(self) -> bool {
        matches!(self, Tile::Floor | Tile::Door | Tile::Corridor | Tile::Stairs)
    }

    pub fn is_opaque(self) -> bool {
        matches!(self, Tile::Wall | Tile::Void)
    }
}

// ── RoomType ──────────────────────────────────────────────────────────────────

/// Type/purpose of a room.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomType {
    Normal,
    Start,
    Entrance,
    Exit,
    Combat(f32),
    Treasure,
    Boss,
    Shop,
    Puzzle,
    Rest,
    Secret,
    Shrine,
    Trap,
}

// ── Room ──────────────────────────────────────────────────────────────────────

/// A room in the dungeon with spatial bounds and graph connections.
#[derive(Debug, Clone)]
pub struct Room {
    pub id:          usize,
    pub rect:        IRect,
    pub room_type:   RoomType,
    pub connections: Vec<usize>,
    pub tags:        Vec<String>,
    pub spawns:      Vec<IVec2>,
    pub visited:     bool,
}

impl Room {
    pub fn new(id: usize, rect: IRect) -> Self {
        Self {
            id, rect, room_type: RoomType::Normal,
            connections: Vec::new(), tags: Vec::new(), spawns: Vec::new(), visited: false,
        }
    }

    pub fn center(&self) -> IVec2 { self.rect.center() }
    pub fn bounds(&self) -> IRect { self.rect }

    pub fn generate_spawns(&mut self, rng: &mut Rng, count: usize) {
        let IRect { x, y, w, h } = self.rect;
        self.spawns.clear();
        for _ in 0..count {
            let sx = rng.range_i32(x + 1, (x + w - 2).max(x + 1));
            let sy = rng.range_i32(y + 1, (y + h - 2).max(y + 1));
            self.spawns.push(IVec2::new(sx, sy));
        }
    }

    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let t = tag.into();
        if !self.tags.contains(&t) { self.tags.push(t); }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

// ── Corridor ──────────────────────────────────────────────────────────────────

/// A corridor connecting two rooms.
#[derive(Debug, Clone)]
pub struct Corridor {
    pub from:     usize,
    pub to:       usize,
    pub path:     Vec<IVec2>,
    pub width:    u8,
    pub has_door: bool,
    pub bend:     IVec2,
}

impl Corridor {
    pub fn new(from: usize, to: usize, from_pos: IVec2, to_pos: IVec2, rng: &mut Rng) -> Self {
        let bend = if rng.chance(0.5) {
            IVec2::new(to_pos.x, from_pos.y)
        } else {
            IVec2::new(from_pos.x, to_pos.y)
        };
        let path = Self::l_path(from_pos, to_pos, bend);
        Self { from, to, path, width: 1, has_door: rng.chance(0.3), bend }
    }

    fn l_path(from: IVec2, to: IVec2, bend: IVec2) -> Vec<IVec2> {
        let mut tiles = Vec::new();
        let dx1 = (bend.x - from.x).signum();
        let dy1 = (bend.y - from.y).signum();
        let mut cur = from;
        while cur != bend {
            tiles.push(cur);
            cur.x += dx1;
            cur.y += dy1;
        }
        tiles.push(bend);
        let dx2 = (to.x - bend.x).signum();
        let dy2 = (to.y - bend.y).signum();
        while cur != to {
            cur.x += dx2;
            cur.y += dy2;
            tiles.push(cur);
        }
        tiles
    }

    pub fn tiles(&self) -> &[IVec2] { &self.path }
}

// ── DungeonGraph ──────────────────────────────────────────────────────────────

/// A graph of rooms and corridors.
#[derive(Debug, Clone, Default)]
pub struct DungeonGraph {
    pub rooms:     Vec<Room>,
    pub corridors: Vec<Corridor>,
    adj: Vec<Vec<(usize, usize)>>,
}

impl DungeonGraph {
    pub fn new() -> Self { Self::default() }

    pub fn add_room(&mut self, room: Room) -> usize {
        let id = self.rooms.len();
        self.rooms.push(room);
        self.adj.push(Vec::new());
        id
    }

    pub fn add_corridor(&mut self, corridor: Corridor) {
        let ci = self.corridors.len();
        let from = corridor.from;
        let to   = corridor.to;
        self.corridors.push(corridor);
        if from < self.adj.len() { self.adj[from].push((to, ci)); }
        if to   < self.adj.len() { self.adj[to].push((from, ci)); }
        if from < self.rooms.len() { self.rooms[from].connections.push(to); }
        if to   < self.rooms.len() { self.rooms[to].connections.push(from); }
    }

    pub fn connected_components(&self) -> usize {
        if self.rooms.is_empty() { return 0; }
        let n = self.rooms.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut Vec<usize>, x: usize) -> usize {
            if parent[x] != x { parent[x] = find(parent, parent[x]); }
            parent[x]
        }
        for c in &self.corridors {
            let rx = find(&mut parent, c.from);
            let ry = find(&mut parent, c.to);
            if rx != ry { parent[rx] = ry; }
        }
        let mut roots = HashSet::new();
        for i in 0..n { roots.insert(find(&mut parent, i)); }
        roots.len()
    }

    pub fn is_connected(&self) -> bool {
        self.connected_components() <= 1
    }

    pub fn shortest_path(&self, a: usize, b: usize) -> Option<Vec<usize>> {
        if a >= self.rooms.len() || b >= self.rooms.len() { return None; }
        if a == b { return Some(vec![a]); }
        let mut visited = vec![false; self.rooms.len()];
        let mut prev = vec![usize::MAX; self.rooms.len()];
        let mut queue = VecDeque::new();
        visited[a] = true;
        queue.push_back(a);
        while let Some(cur) = queue.pop_front() {
            if cur == b {
                let mut path = Vec::new();
                let mut node = b;
                while node != usize::MAX {
                    path.push(node);
                    node = prev[node];
                }
                path.reverse();
                return Some(path);
            }
            if cur < self.adj.len() {
                for &(nb, _) in &self.adj[cur] {
                    if !visited[nb] {
                        visited[nb] = true;
                        prev[nb] = cur;
                        queue.push_back(nb);
                    }
                }
            }
        }
        None
    }

    pub fn minimum_spanning_tree(&self) -> Vec<(usize, usize)> {
        let n = self.rooms.len();
        if n < 2 { return Vec::new(); }
        let mut edges: Vec<(f32, usize, usize)> = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                let ci = self.rooms[i].center();
                let cj = self.rooms[j].center();
                let dx = (ci.x - cj.x) as f32;
                let dy = (ci.y - cj.y) as f32;
                edges.push(((dx * dx + dy * dy).sqrt(), i, j));
            }
        }
        edges.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut Vec<usize>, x: usize) -> usize {
            if parent[x] != x { parent[x] = find(parent, parent[x]); }
            parent[x]
        }
        let mut mst = Vec::new();
        for (_, u, v) in edges {
            let ru = find(&mut parent, u);
            let rv = find(&mut parent, v);
            if ru != rv {
                parent[ru] = rv;
                mst.push((u, v));
                if mst.len() == n - 1 { break; }
            }
        }
        mst
    }
}

// ── BspSplitter ───────────────────────────────────────────────────────────────

pub struct BspSplitter {
    pub min_room_size: i32,
    pub split_jitter:  f32,
    pub max_depth:     u32,
}

impl BspSplitter {
    pub fn new(min_room_size: i32, split_jitter: f32, max_depth: u32) -> Self {
        Self { min_room_size, split_jitter, max_depth }
    }

    pub fn generate(&self, width: i32, height: i32, rng: &mut Rng) -> DungeonGraph {
        let bounds = IRect::new(0, 0, width, height);
        let mut leaves = Vec::new();
        self.split_node(bounds, rng, 0, &mut leaves);

        let mut graph = DungeonGraph::new();
        let n = leaves.len();
        for (i, rect) in leaves.iter().enumerate() {
            let mut room = Room::new(i, *rect);
            let spawn_count = rng.range_usize(3) + 1;
            room.generate_spawns(rng, spawn_count);
            graph.add_room(room);
        }

        let mst_edges = graph.minimum_spanning_tree();
        for (a, b) in mst_edges {
            let fp = graph.rooms[a].center();
            let tp = graph.rooms[b].center();
            let c  = Corridor::new(a, b, fp, tp, rng);
            graph.add_corridor(c);
        }
        let extra = (n / 5).max(1);
        let total = graph.rooms.len();
        for _ in 0..extra {
            if total < 2 { break; }
            let a = rng.range_usize(total);
            let b = rng.range_usize(total);
            if a != b {
                let fp = graph.rooms[a].center();
                let tp = graph.rooms[b].center();
                let c  = Corridor::new(a, b, fp, tp, rng);
                graph.add_corridor(c);
            }
        }

        if !graph.rooms.is_empty() { graph.rooms[0].room_type = RoomType::Entrance; }
        let last = graph.rooms.len().saturating_sub(1);
        if graph.rooms.len() > 1 { graph.rooms[last].room_type = RoomType::Exit; }
        let specials = graph.rooms.len() / 6;
        let mut indices: Vec<usize> = (1..graph.rooms.len().saturating_sub(1)).collect();
        rng.shuffle(&mut indices);
        for &i in indices.iter().take(specials) {
            graph.rooms[i].room_type = RoomType::Treasure;
        }
        for &i in indices.iter().skip(specials).take(specials) {
            graph.rooms[i].room_type = RoomType::Combat(rng.range_f32(0.3, 0.9));
        }
        if let Some(&bi) = indices.last() {
            graph.rooms[bi].room_type = RoomType::Boss;
        }

        graph
    }

    fn split_node(&self, bounds: IRect, rng: &mut Rng, depth: u32, leaves: &mut Vec<IRect>) {
        if depth >= self.max_depth
            || (bounds.w < self.min_room_size * 2 && bounds.h < self.min_room_size * 2)
        {
            if let Some(inner) = bounds.shrink(2) {
                if inner.w >= self.min_room_size && inner.h >= self.min_room_size {
                    let rw = rng.range_i32(self.min_room_size, inner.w);
                    let rh = rng.range_i32(self.min_room_size, inner.h);
                    let rx = rng.range_i32(inner.x, inner.x + inner.w - rw);
                    let ry = rng.range_i32(inner.y, inner.y + inner.h - rh);
                    leaves.push(IRect::new(rx, ry, rw, rh));
                }
            }
            return;
        }
        let split_h = if bounds.w > bounds.h { false }
                      else if bounds.h > bounds.w { true }
                      else { rng.chance(0.5) };
        let jitter = rng.range_f32(-self.split_jitter, self.split_jitter);
        let ratio  = (0.5 + jitter).clamp(0.25, 0.75);
        if split_h {
            if bounds.h < self.min_room_size * 2 { leaves.push(bounds); return; }
            let sy = bounds.y + (bounds.h as f32 * ratio) as i32;
            self.split_node(IRect::new(bounds.x, bounds.y, bounds.w, sy - bounds.y), rng, depth + 1, leaves);
            self.split_node(IRect::new(bounds.x, sy, bounds.w, bounds.h - (sy - bounds.y)), rng, depth + 1, leaves);
        } else {
            if bounds.w < self.min_room_size * 2 { leaves.push(bounds); return; }
            let sx = bounds.x + (bounds.w as f32 * ratio) as i32;
            self.split_node(IRect::new(bounds.x, bounds.y, sx - bounds.x, bounds.h), rng, depth + 1, leaves);
            self.split_node(IRect::new(sx, bounds.y, bounds.w - (sx - bounds.x), bounds.h), rng, depth + 1, leaves);
        }
    }
}

// ── RoomPlacer ────────────────────────────────────────────────────────────────

pub struct RoomPlacer {
    pub min_room_w:   i32,
    pub max_room_w:   i32,
    pub min_room_h:   i32,
    pub max_room_h:   i32,
    pub separation:   i32,
    pub max_attempts: usize,
}

impl Default for RoomPlacer {
    fn default() -> Self {
        Self { min_room_w: 5, max_room_w: 14, min_room_h: 4, max_room_h: 10, separation: 2, max_attempts: 500 }
    }
}

impl RoomPlacer {
    pub fn new(min_w: i32, max_w: i32, min_h: i32, max_h: i32, sep: i32) -> Self {
        Self { min_room_w: min_w, max_room_w: max_w, min_room_h: min_h, max_room_h: max_h, separation: sep, max_attempts: 500 }
    }

    pub fn generate(&self, width: i32, height: i32, num_rooms: usize, rng: &mut Rng) -> DungeonGraph {
        let mut placed: Vec<IRect> = Vec::new();
        let mut attempts = 0usize;
        while placed.len() < num_rooms && attempts < self.max_attempts {
            attempts += 1;
            let rw = rng.range_i32(self.min_room_w, self.max_room_w);
            let rh = rng.range_i32(self.min_room_h, self.max_room_h);
            let rx = rng.range_i32(1, (width - rw - 1).max(2));
            let ry = rng.range_i32(1, (height - rh - 1).max(2));
            let candidate = IRect::new(rx, ry, rw, rh);
            let expanded  = candidate.expand(self.separation);
            if !placed.iter().any(|r| r.overlaps(&expanded)) { placed.push(candidate); }
        }
        let mut graph = DungeonGraph::new();
        for (i, rect) in placed.iter().enumerate() {
            let mut room = Room::new(i, *rect);
            let spawn_count = rng.range_usize(3) + 1;
            room.generate_spawns(rng, spawn_count);
            graph.add_room(room);
        }
        let mst = graph.minimum_spanning_tree();
        for (a, b) in mst {
            let fp = graph.rooms[a].center();
            let tp = graph.rooms[b].center();
            let c  = Corridor::new(a, b, fp, tp, rng);
            graph.add_corridor(c);
        }
        let extra = (placed.len() / 5).max(1);
        let n = graph.rooms.len();
        for _ in 0..extra {
            if n < 2 { break; }
            let a = rng.range_usize(n);
            let b = (a + 1 + rng.range_usize(n - 1)) % n;
            let fp = graph.rooms[a].center();
            let tp = graph.rooms[b].center();
            let c  = Corridor::new(a, b, fp, tp, rng);
            graph.add_corridor(c);
        }
        if !graph.rooms.is_empty() { graph.rooms[0].room_type = RoomType::Entrance; }
        let last = graph.rooms.len().saturating_sub(1);
        if last > 0 { graph.rooms[last].room_type = RoomType::Exit; }
        graph
    }
}

// ── CellularDungeon ───────────────────────────────────────────────────────────

pub struct CellularDungeon {
    pub width:      usize,
    pub height:     usize,
    pub fill_prob:  f32,
    pub birth:      usize,
    pub survive:    usize,
    pub iterations: usize,
}

impl CellularDungeon {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, fill_prob: 0.45, birth: 5, survive: 4, iterations: 5 }
    }

    pub fn generate(&self, rng: &mut Rng) -> Vec<bool> {
        let n = self.width * self.height;
        let mut grid: Vec<bool> = (0..n).map(|_| !rng.chance(self.fill_prob)).collect();
        self.fill_border(&mut grid, false);
        let mut next = vec![false; n];
        for _ in 0..self.iterations {
            for y in 0..self.height {
                for x in 0..self.width {
                    let nb = self.count_neighbours(&grid, x, y);
                    let alive = grid[y * self.width + x];
                    next[y * self.width + x] = if alive { nb >= self.survive } else { nb >= self.birth };
                }
            }
            self.fill_border(&mut next, false);
            std::mem::swap(&mut grid, &mut next);
        }
        let largest = self.largest_component(&grid);
        for i in 0..n { if grid[i] && !largest.contains(&i) { grid[i] = false; } }
        grid
    }

    fn fill_border(&self, grid: &mut Vec<bool>, val: bool) {
        let (w, h) = (self.width, self.height);
        for x in 0..w { grid[x] = val; grid[(h-1)*w+x] = val; }
        for y in 0..h { grid[y*w] = val; grid[y*w+w-1] = val; }
    }

    fn count_neighbours(&self, grid: &[bool], x: usize, y: usize) -> usize {
        let mut count = 0;
        for dy in -1i32..=1 { for dx in -1i32..=1 {
            if dx == 0 && dy == 0 { continue; }
            let nx = x as i32 + dx; let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx as usize >= self.width || ny as usize >= self.height { count += 1; }
            else if grid[ny as usize * self.width + nx as usize] { count += 1; }
        }}
        count
    }

    fn largest_component(&self, grid: &[bool]) -> HashSet<usize> {
        let n = self.width * self.height;
        let mut visited = vec![false; n];
        let mut best: HashSet<usize> = HashSet::new();
        for start in 0..n {
            if !grid[start] || visited[start] { continue; }
            let mut comp = HashSet::new();
            let mut q = VecDeque::new();
            q.push_back(start); visited[start] = true;
            while let Some(idx) = q.pop_front() {
                comp.insert(idx);
                let (x, y) = ((idx % self.width) as i32, (idx / self.width) as i32);
                for (dx, dy) in &[(0i32,1),(0,-1),(1,0),(-1,0)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx as usize >= self.width || ny as usize >= self.height { continue; }
                    let ni = ny as usize * self.width + nx as usize;
                    if grid[ni] && !visited[ni] { visited[ni] = true; q.push_back(ni); }
                }
            }
            if comp.len() > best.len() { best = comp; }
        }
        best
    }
}

// ── WFC ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WfcTile {
    pub id:     usize,
    pub name:   String,
    pub weight: f32,
    pub allowed_neighbours: [Vec<usize>; 4],
}

impl WfcTile {
    pub fn new(id: usize, name: impl Into<String>, weight: f32) -> Self {
        Self { id, name: name.into(), weight, allowed_neighbours: [Vec::new(), Vec::new(), Vec::new(), Vec::new()] }
    }

    pub fn allow_neighbour(&mut self, direction: usize, neighbour_id: usize) {
        if direction < 4 { self.allowed_neighbours[direction].push(neighbour_id); }
    }
}

pub struct WfcDungeon {
    pub width:  usize,
    pub height: usize,
    tiles:      Vec<WfcTile>,
}

impl WfcDungeon {
    pub fn new(width: usize, height: usize, tiles: Vec<WfcTile>) -> Self {
        Self { width, height, tiles }
    }

    pub fn generate(&self, rng: &mut Rng) -> Option<Vec<usize>> {
        let n = self.width * self.height;
        let tc = self.tiles.len();
        if tc == 0 { return None; }
        let all: Vec<usize> = (0..tc).collect();
        let mut cells: Vec<Vec<usize>> = vec![all.clone(); n];
        let max_iter = n * tc + 100;
        let mut iter = 0;
        loop {
            iter += 1;
            if iter > max_iter { return None; }
            if cells.iter().all(|c| c.len() == 1) { break; }
            let ci = cells.iter().enumerate().filter(|(_, c)| c.len() > 1).min_by_key(|(_, c)| c.len()).map(|(i, _)| i)?;
            let opts = cells[ci].clone();
            let weighted: Vec<(usize, f32)> = opts.iter().map(|&tid| (tid, self.tiles[tid].weight)).collect();
            let chosen = rng.pick_weighted(&weighted).copied()?;
            cells[ci] = vec![chosen];
            if !self.propagate(&mut cells) { return None; }
        }
        Some(cells.iter().map(|c| *c.first().unwrap_or(&0)).collect())
    }

    fn propagate(&self, cells: &mut Vec<Vec<usize>>) -> bool {
        let (w, h) = (self.width, self.height);
        let n = w * h;
        let mut changed = true;
        while changed {
            changed = false;
            for idx in 0..n {
                let (x, y) = ((idx % w) as i32, (idx / w) as i32);
                let nbrs: [(i32, i32, usize); 4] = [(x, y-1, 0),(x, y+1, 1),(x+1, y, 2),(x-1, y, 3)];
                for (nx, ny, dir) in nbrs {
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h { continue; }
                    let ni  = ny as usize * w + nx as usize;
                    let opp = match dir { 0 => 1, 1 => 0, 2 => 3, _ => 2 };
                    let cur = cells[idx].clone();
                    let before = cells[ni].len();
                    cells[ni].retain(|&nt| {
                        cur.iter().any(|&ct| {
                            (ct < self.tiles.len() && self.tiles[ct].allowed_neighbours[dir].contains(&nt))
                            || (nt < self.tiles.len() && self.tiles[nt].allowed_neighbours[opp].contains(&ct))
                        })
                    });
                    if cells[ni].is_empty() { return false; }
                    if cells[ni].len() < before { changed = true; }
                }
            }
        }
        true
    }

    pub fn default_tileset() -> Vec<WfcTile> {
        let mut wall  = WfcTile::new(0, "wall",  1.0);
        let mut floor = WfcTile::new(1, "floor", 3.0);
        let mut door  = WfcTile::new(2, "door",  0.3);
        for d in 0..4 { wall.allow_neighbour(d, 0); wall.allow_neighbour(d, 1); }
        for d in 0..4 { floor.allow_neighbour(d, 0); floor.allow_neighbour(d, 1); floor.allow_neighbour(d, 2); }
        for d in 0..4 { door.allow_neighbour(d, 0); door.allow_neighbour(d, 1); door.allow_neighbour(d, 2); }
        vec![wall, floor, door]
    }
}

// ── DungeonDecorator ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Enemy, TreasureChest, Trap, LightSource, SpawnPoint,
    ShopKeeper, BossMonster, Shrine, Puzzle, RestArea,
}

#[derive(Debug, Clone)]
pub struct PlacedObject {
    pub kind:     ObjectKind,
    pub position: IVec2,
    pub metadata: HashMap<String, String>,
}

impl PlacedObject {
    pub fn new(kind: ObjectKind, position: IVec2) -> Self {
        Self { kind, position, metadata: HashMap::new() }
    }
    pub fn with_meta(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.metadata.insert(k.into(), v.into()); self
    }
}

pub struct DungeonDecorator {
    pub enemy_density: f32,
    pub trap_density:  f32,
    pub light_density: f32,
}

impl Default for DungeonDecorator {
    fn default() -> Self { Self { enemy_density: 0.05, trap_density: 0.02, light_density: 0.03 } }
}

impl DungeonDecorator {
    pub fn new(ed: f32, td: f32, ld: f32) -> Self { Self { enemy_density: ed, trap_density: td, light_density: ld } }

    pub fn decorate(&self, graph: &DungeonGraph, depth: u32, rng: &mut Rng) -> Vec<PlacedObject> {
        let mut out = Vec::new();
        for room in &graph.rooms {
            let area = room.rect.area() as f32;
            match &room.room_type {
                RoomType::Entrance => {
                    out.push(PlacedObject::new(ObjectKind::SpawnPoint, room.center()).with_meta("room_id", room.id.to_string()));
                    self.lights(room, 2, rng, &mut out);
                }
                RoomType::Exit => {
                    out.push(PlacedObject::new(ObjectKind::SpawnPoint, room.center()).with_meta("type","exit"));
                }
                RoomType::Combat(diff) => {
                    let ne = ((area * self.enemy_density * diff) as usize + 1).min(8);
                    for _ in 0..ne {
                        let pos = self.rpos(room, rng);
                        let lv  = (depth as f32 * diff) as u32 + 1;
                        out.push(PlacedObject::new(ObjectKind::Enemy, pos).with_meta("level", lv.to_string()));
                    }
                    let nt = ((area * self.trap_density) as usize).min(3);
                    for _ in 0..nt {
                        out.push(PlacedObject::new(ObjectKind::Trap, self.rpos(room, rng)).with_meta("hidden","true"));
                    }
                }
                RoomType::Treasure => {
                    let nc = rng.range_usize(2) + 1;
                    for _ in 0..nc {
                        out.push(PlacedObject::new(ObjectKind::TreasureChest, self.rpos(room, rng)).with_meta("depth", depth.to_string()));
                    }
                    self.lights(room, 1, rng, &mut out);
                }
                RoomType::Boss => {
                    out.push(PlacedObject::new(ObjectKind::BossMonster, room.center()).with_meta("level",(depth*3).to_string()));
                    out.push(PlacedObject::new(ObjectKind::TreasureChest, self.rpos(room, rng)).with_meta("rarity","legendary"));
                    self.lights(room, 3, rng, &mut out);
                }
                RoomType::Shop => {
                    out.push(PlacedObject::new(ObjectKind::ShopKeeper, room.center()).with_meta("stock_seed", rng.next_u64().to_string()));
                    self.lights(room, 2, rng, &mut out);
                }
                RoomType::Rest | RoomType::Shrine => {
                    out.push(PlacedObject::new(ObjectKind::RestArea, room.center()));
                    self.lights(room, 1, rng, &mut out);
                }
                RoomType::Puzzle => {
                    out.push(PlacedObject::new(ObjectKind::Puzzle, room.center()).with_meta("seed", rng.next_u64().to_string()));
                }
                RoomType::Secret => {
                    out.push(PlacedObject::new(ObjectKind::TreasureChest, self.rpos(room, rng)).with_meta("hidden","true").with_meta("rarity","rare"));
                }
                _ => {
                    let ne = ((area * self.enemy_density) as usize).min(5);
                    for _ in 0..ne {
                        out.push(PlacedObject::new(ObjectKind::Enemy, self.rpos(room, rng)).with_meta("level", depth.to_string()));
                    }
                }
            }
            let nl = ((area * self.light_density) as usize).min(4);
            for _ in 0..nl { out.push(PlacedObject::new(ObjectKind::LightSource, self.rpos(room, rng))); }
        }
        out
    }

    fn lights(&self, room: &Room, n: usize, rng: &mut Rng, out: &mut Vec<PlacedObject>) {
        for _ in 0..n { out.push(PlacedObject::new(ObjectKind::LightSource, self.rpos(room, rng))); }
    }

    fn rpos(&self, room: &Room, rng: &mut Rng) -> IVec2 {
        let r = room.rect;
        IVec2::new(
            rng.range_i32(r.x + 1, (r.x + r.w - 2).max(r.x + 1)),
            rng.range_i32(r.y + 1, (r.y + r.h - 2).max(r.y + 1)),
        )
    }
}

// ── MazeGenerator ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MazeCell {
    pub walls:   [bool; 4],
    pub visited: bool,
}

impl Default for MazeCell {
    fn default() -> Self { Self { walls: [true; 4], visited: false } }
}

pub struct MazeGenerator {
    pub width:  usize,
    pub height: usize,
}

impl MazeGenerator {
    pub fn new(width: usize, height: usize) -> Self { Self { width, height } }

    fn idx(&self, x: usize, y: usize) -> usize { y * self.width + x }

    fn remove_wall(cells: &mut Vec<MazeCell>, a: usize, b: usize, dir: usize) {
        let opp = [1usize, 0, 3, 2];
        cells[a].walls[dir]      = false;
        cells[b].walls[opp[dir]] = false;
        cells[a].visited = true;
        cells[b].visited = true;
    }

    fn nbrs(&self, x: usize, y: usize) -> Vec<(usize, usize, usize)> {
        let mut v = Vec::new();
        if y > 0               { v.push((x, y-1, 0)); }
        if y+1 < self.height   { v.push((x, y+1, 1)); }
        if x+1 < self.width    { v.push((x+1, y, 2)); }
        if x > 0               { v.push((x-1, y, 3)); }
        v
    }

    pub fn recursive_backtracker(&self, rng: &mut Rng) -> Vec<MazeCell> {
        let n = self.width * self.height;
        let mut cells = vec![MazeCell::default(); n];
        let mut stack = Vec::new();
        let s = self.idx(0, 0);
        cells[s].visited = true;
        stack.push((0usize, 0usize));
        while let Some(&(cx, cy)) = stack.last() {
            let mut unvisited: Vec<_> = self.nbrs(cx, cy).into_iter().filter(|&(nx, ny, _)| !cells[self.idx(nx,ny)].visited).collect();
            if unvisited.is_empty() { stack.pop(); }
            else {
                rng.shuffle(&mut unvisited);
                let (nx, ny, dir) = unvisited[0];
                let (ai, bi) = (self.idx(cx,cy), self.idx(nx,ny));
                Self::remove_wall(&mut cells, ai, bi, dir);
                cells[bi].visited = true;
                stack.push((nx, ny));
            }
        }
        cells
    }

    pub fn ellers_algorithm(&self, rng: &mut Rng) -> Vec<MazeCell> {
        let n = self.width * self.height;
        let mut cells = vec![MazeCell::default(); n];
        let (w, h) = (self.width, self.height);
        let mut set_id = (1..=w).collect::<Vec<usize>>();
        let mut next_set = w + 1;
        for y in 0..h {
            let last = y + 1 == h;
            for x in 0..(w-1) {
                let merge = if last { set_id[x] != set_id[x+1] } else { rng.chance(0.5) && set_id[x] != set_id[x+1] };
                if merge {
                    let old = set_id[x+1]; let new = set_id[x];
                    for s in &mut set_id { if *s == old { *s = new; } }
                    let (ai, bi) = (self.idx(x, y), self.idx(x+1, y));
                    Self::remove_wall(&mut cells, ai, bi, 2);
                }
            }
            if !last {
                let mut sets: HashMap<usize, Vec<usize>> = HashMap::new();
                for x in 0..w { sets.entry(set_id[x]).or_default().push(x); }
                let mut nid: Vec<usize> = (next_set..next_set+w).collect();
                next_set += w;
                for (sid, xs) in &sets {
                    let nv = rng.range_usize(xs.len()) + 1;
                    let mut chosen = xs.clone(); rng.shuffle(&mut chosen);
                    for &cx in chosen.iter().take(nv) {
                        let (ai, bi) = (self.idx(cx,y), self.idx(cx,y+1));
                        Self::remove_wall(&mut cells, ai, bi, 1);
                        nid[cx] = *sid;
                    }
                }
                set_id = nid;
            }
        }
        cells
    }

    pub fn prims_algorithm(&self, rng: &mut Rng) -> Vec<MazeCell> {
        let n = self.width * self.height;
        let mut cells = vec![MazeCell::default(); n];
        let mut in_maze = vec![false; n];
        let s = self.idx(0,0); in_maze[s] = true; cells[s].visited = true;
        let mut frontier: Vec<(usize,usize,usize,usize,usize)> = self.nbrs(0,0).into_iter().map(|(nx,ny,d)| (0,0,nx,ny,d)).collect();
        while !frontier.is_empty() {
            let fi = rng.range_usize(frontier.len());
            let (ax,ay,nx,ny,dir) = frontier.swap_remove(fi);
            let bi = self.idx(nx,ny);
            if in_maze[bi] { continue; }
            in_maze[bi] = true; cells[bi].visited = true;
            Self::remove_wall(&mut cells, self.idx(ax,ay), bi, dir);
            for (nnx,nny,nd) in self.nbrs(nx,ny) { if !in_maze[self.idx(nnx,nny)] { frontier.push((nx,ny,nnx,nny,nd)); } }
        }
        cells
    }

    pub fn kruskals_algorithm(&self, rng: &mut Rng) -> Vec<MazeCell> {
        let n = self.width * self.height;
        let mut cells = vec![MazeCell::default(); n];
        let mut edges: Vec<(usize,usize,usize,usize,usize)> = Vec::new();
        for y in 0..self.height { for x in 0..self.width {
            if x+1 < self.width  { edges.push((x, y, x+1, y, 2)); }
            if y+1 < self.height { edges.push((x, y, x, y+1, 1)); }
        }}
        rng.shuffle(&mut edges);
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize {
            if p[x] != x { p[x] = find(p, p[x]); } p[x]
        }
        for (ax,ay,bx,by,dir) in edges {
            let (ai, bi) = (self.idx(ax,ay), self.idx(bx,by));
            let (ra, rb) = (find(&mut parent, ai), find(&mut parent, bi));
            if ra != rb { parent[ra] = rb; Self::remove_wall(&mut cells, ai, bi, dir); }
        }
        cells
    }

    pub fn to_tiles(&self, cells: &[MazeCell]) -> Vec<Tile> {
        let (tw, th) = (self.width*2+1, self.height*2+1);
        let mut tiles = vec![Tile::Wall; tw*th];
        for y in 0..self.height { for x in 0..self.width {
            let cell = cells[self.idx(x,y)];
            let (tx,ty) = (x*2+1, y*2+1);
            tiles[ty*tw+tx] = Tile::Floor;
            if !cell.walls[2] && x+1 < self.width  { tiles[ty*tw+tx+1]     = Tile::Corridor; }
            if !cell.walls[1] && y+1 < self.height { tiles[(ty+1)*tw+tx]   = Tile::Corridor; }
        }}
        tiles
    }
}

// ── DungeonFloor (legacy) ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DungeonFloor {
    pub width:     usize,
    pub height:    usize,
    pub theme:     DungeonTheme,
    pub rooms:     Vec<Room>,
    pub corridors: Vec<Corridor>,
    pub tiles:     Vec<Tile>,
    pub depth:     u32,
    pub start:     (i32, i32),
    pub exit:      (i32, i32),
    pub boss_room: Option<usize>,
}

impl DungeonFloor {
    pub fn generate(seed: u64, depth: u32, theme: DungeonTheme) -> Self {
        let mut rng = Rng::new(seed ^ (depth as u64).wrapping_mul(0xdeadbeef));
        let w = (60 + depth as usize * 5).min(200);
        let h = (40 + depth as usize * 3).min(120);
        let graph = BspSplitter::new(5, 0.2, 5 + depth/2).generate(w as i32, h as i32, &mut rng);
        let rooms     = graph.rooms.clone();
        let corridors = graph.corridors.clone();
        let start = rooms.first().map(|r| { let c=r.center(); (c.x,c.y) }).unwrap_or((1,1));
        let exit  = rooms.last() .map(|r| { let c=r.center(); (c.x,c.y) }).unwrap_or((w as i32-2, h as i32-2));
        let boss_room = rooms.iter().position(|r| r.room_type == RoomType::Boss);
        let mut tiles = vec![Tile::Wall; w*h];
        for room in &rooms {
            let IRect{x,y,w:rw,h:rh} = room.rect;
            for ty in y..(y+rh) { for tx in x..(x+rw) {
                if tx>=0 && ty>=0 && (tx as usize)<w && (ty as usize)<h {
                    tiles[ty as usize*w+tx as usize] = Tile::Floor;
                }
            }}
        }
        for corr in &corridors {
            for pos in corr.tiles() {
                let (tx,ty) = (pos.x, pos.y);
                if tx>=0 && ty>=0 && (tx as usize)<w && (ty as usize)<h {
                    let i = ty as usize*w+tx as usize;
                    if tiles[i] == Tile::Wall { tiles[i] = Tile::Corridor; }
                }
            }
            if corr.has_door {
                let (bx,by) = (corr.bend.x, corr.bend.y);
                if bx>=0 && by>=0 && (bx as usize)<w && (by as usize)<h {
                    tiles[by as usize*w+bx as usize] = Tile::Door;
                }
            }
        }
        let (ex,ey) = exit;
        if ex>=0 && ey>=0 && (ex as usize)<w && (ey as usize)<h {
            tiles[ey as usize*w+ex as usize] = Tile::Stairs;
        }
        Self { width:w, height:h, theme, rooms, corridors, tiles, depth, start, exit, boss_room }
    }

    pub fn get(&self, x: i32, y: i32) -> Tile {
        if x<0||y<0||(x as usize)>=self.width||(y as usize)>=self.height { return Tile::Void; }
        self.tiles[y as usize*self.width+x as usize]
    }

    pub fn reachable_tiles(&self, sx: i32, sy: i32) -> Vec<(i32,i32)> {
        let mut visited = vec![false; self.width*self.height];
        let mut queue = VecDeque::new();
        let mut result = Vec::new();
        if self.get(sx,sy).is_walkable() { queue.push_back((sx,sy)); }
        while let Some((x,y)) = queue.pop_front() {
            if x<0||y<0||(x as usize)>=self.width||(y as usize)>=self.height { continue; }
            let idx = y as usize*self.width+x as usize;
            if visited[idx] { continue; }
            visited[idx] = true;
            if self.tiles[idx].is_walkable() {
                result.push((x,y));
                for (dx,dy) in &[(0i32,1),(0,-1),(1,0),(-1,0)] { queue.push_back((x+dx, y+dy)); }
            }
        }
        result
    }

    pub fn walkable_count(&self) -> usize { self.tiles.iter().filter(|t| t.is_walkable()).count() }

    pub fn room_at(&self, x: i32, y: i32) -> Option<usize> {
        self.rooms.iter().position(|r| r.rect.contains(x,y))
    }

    pub fn doors(&self) -> impl Iterator<Item=(i32,i32)> + '_ {
        (0..self.height).flat_map(move |y| (0..self.width).filter_map(move |x| {
            if self.tiles[y*self.width+x] == Tile::Door { Some((x as i32, y as i32)) } else { None }
        }))
    }

    pub fn dimensions(&self) -> (usize,usize) { (self.width, self.height) }
}

#[derive(Debug, Clone)]
pub struct FloorMetrics {
    pub room_count: usize, pub corridor_count: usize, pub walkable_tiles: usize,
    pub total_tiles: usize, pub fill_ratio: f32, pub has_boss: bool,
    pub treasure_rooms: usize, pub avg_room_area: f32,
}

impl FloorMetrics {
    pub fn compute(floor: &DungeonFloor) -> Self {
        let walkable = floor.walkable_count();
        let total    = floor.width * floor.height;
        let treasure = floor.rooms.iter().filter(|r| r.room_type == RoomType::Treasure).count();
        let avg_area = if floor.rooms.is_empty() { 0.0 } else {
            floor.rooms.iter().map(|r| r.rect.area()).sum::<i32>() as f32 / floor.rooms.len() as f32
        };
        Self { room_count: floor.rooms.len(), corridor_count: floor.corridors.len(), walkable_tiles: walkable,
               total_tiles: total, fill_ratio: walkable as f32/total as f32, has_boss: floor.boss_room.is_some(),
               treasure_rooms: treasure, avg_room_area: avg_area }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn rng() -> Rng { Rng::new(42) }

    #[test] fn dungeon_floor_generates_rooms() { assert!(!DungeonFloor::generate(42,1,DungeonTheme::Cave).rooms.is_empty()); }
    #[test] fn dungeon_floor_start_walkable() { let f=DungeonFloor::generate(99,1,DungeonTheme::Cave); let (sx,sy)=f.start; assert!(f.get(sx,sy).is_walkable()); }
    #[test] fn floor_fill_ratio_reasonable() { let m=FloorMetrics::compute(&DungeonFloor::generate(7,1,DungeonTheme::Cave)); assert!(m.fill_ratio>0.01&&m.fill_ratio<0.95,"fill: {}",m.fill_ratio); }

    #[test]
    fn bsp_splitter_connected() {
        let mut r = rng();
        let g = BspSplitter::new(5,0.15,4).generate(80,60,&mut r);
        assert!(!g.rooms.is_empty());
        assert!(g.is_connected());
    }

    #[test]
    fn room_placer_generates_rooms() {
        let mut r = rng();
        let g = RoomPlacer::default().generate(100,80,10,&mut r);
        assert!(g.rooms.len()>=3,"got {}",g.rooms.len());
    }

    #[test]
    fn cellular_has_floor_tiles() {
        let mut r = rng();
        let grid = CellularDungeon::new(60,40).generate(&mut r);
        assert!(grid.iter().filter(|&&b| b).count()>50);
    }

    #[test]
    fn maze_rb_all_visited() {
        let mut r = rng();
        assert!(MazeGenerator::new(10,10).recursive_backtracker(&mut r).iter().all(|c| c.visited));
    }

    #[test]
    fn maze_prims_all_visited() {
        let mut r = rng();
        assert!(MazeGenerator::new(8,8).prims_algorithm(&mut r).iter().all(|c| c.visited));
    }

    #[test]
    fn maze_kruskals_all_visited() {
        let mut r = rng();
        assert!(MazeGenerator::new(8,8).kruskals_algorithm(&mut r).iter().all(|c| c.visited));
    }

    #[test]
    fn graph_shortest_path() {
        let mut r = rng();
        let g = RoomPlacer::default().generate(80,60,6,&mut r);
        if g.rooms.len()>=2 { assert!(g.shortest_path(0,g.rooms.len()-1).is_some()); }
    }

    #[test]
    fn graph_mst_edges() {
        let mut r = rng();
        let g = RoomPlacer::default().generate(80,60,8,&mut r);
        let n = g.rooms.len();
        if n>=2 { assert_eq!(g.minimum_spanning_tree().len(), n-1); }
    }

    #[test]
    fn decorator_places_objects() {
        let mut r = rng();
        let g = BspSplitter::new(6,0.2,4).generate(80,60,&mut r);
        assert!(!DungeonDecorator::default().decorate(&g,3,&mut r).is_empty());
    }

    #[test]
    fn wfc_does_not_panic() {
        let mut r = rng();
        let _ = WfcDungeon::new(8,8,WfcDungeon::default_tileset()).generate(&mut r);
    }

    #[test]
    fn maze_tiles_correct_size() {
        let mut r = rng();
        let g = MazeGenerator::new(5,5);
        let cells = g.recursive_backtracker(&mut r);
        assert_eq!(g.to_tiles(&cells).len(), 11*11);
    }

    #[test]
    fn ellers_all_visited() {
        let mut r = rng();
        assert!(MazeGenerator::new(8,8).ellers_algorithm(&mut r).iter().all(|c| c.visited));
    }
}
