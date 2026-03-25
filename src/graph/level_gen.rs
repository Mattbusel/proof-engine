use glam::Vec2;
use std::collections::{HashMap, HashSet, VecDeque};
use super::graph_core::{Graph, GraphKind, NodeId, EdgeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomType {
    Start,
    End,
    Normal,
    Treasure,
    Boss,
    Secret,
}

#[derive(Debug, Clone)]
pub struct RoomNode {
    pub room_type: RoomType,
    pub position: Vec2,
    pub size: Vec2,
    pub connections: Vec<NodeId>,
}

#[derive(Debug, Clone)]
pub struct LevelGraph {
    pub graph: Graph<RoomNode, f32>,
    pub rooms: HashMap<NodeId, RoomNode>,
}

impl LevelGraph {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(GraphKind::Undirected),
            rooms: HashMap::new(),
        }
    }

    pub fn room_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn corridor_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn get_room(&self, id: NodeId) -> Option<&RoomNode> {
        self.rooms.get(&id)
    }

    pub fn room_ids(&self) -> Vec<NodeId> {
        self.graph.node_ids()
    }

    /// Check if all rooms are connected (graph is connected).
    pub fn is_connected(&self) -> bool {
        let ids = self.graph.node_ids();
        if ids.is_empty() { return true; }
        let visited: Vec<NodeId> = self.graph.bfs(ids[0]).collect();
        visited.len() == ids.len()
    }
}

fn pseudo_random(seed: u64, i: u64) -> f64 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(i.wrapping_mul(1442695040888963407));
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x as f64) / (u64::MAX as f64)
}

/// Generate a dungeon level graph.
/// `room_count`: number of rooms
/// `connectivity`: 0.0 = tree (minimum edges), 1.0 = many extra edges
pub fn generate_dungeon(room_count: usize, connectivity: f32) -> LevelGraph {
    generate_dungeon_seeded(room_count, connectivity, 12345)
}

fn generate_dungeon_seeded(room_count: usize, connectivity: f32, seed: u64) -> LevelGraph {
    let mut level = LevelGraph::new();
    if room_count == 0 { return level; }

    let connectivity = connectivity.clamp(0.0, 1.0);

    // Create room nodes with random positions
    let spread = (room_count as f32).sqrt() * 100.0;
    let mut node_ids = Vec::new();
    for i in 0..room_count {
        let x = (pseudo_random(seed, i as u64 * 2) as f32 - 0.5) * spread;
        let y = (pseudo_random(seed, i as u64 * 2 + 1) as f32 - 0.5) * spread;
        let pos = Vec2::new(x, y);

        let room_type = if i == 0 {
            RoomType::Start
        } else if i == room_count - 1 {
            RoomType::End
        } else if pseudo_random(seed + 100, i as u64) < 0.1 {
            RoomType::Treasure
        } else if pseudo_random(seed + 200, i as u64) < 0.05 {
            RoomType::Boss
        } else if pseudo_random(seed + 300, i as u64) < 0.05 {
            RoomType::Secret
        } else {
            RoomType::Normal
        };

        let size = Vec2::new(
            40.0 + pseudo_random(seed + 400, i as u64) as f32 * 60.0,
            40.0 + pseudo_random(seed + 500, i as u64) as f32 * 60.0,
        );

        let room = RoomNode {
            room_type,
            position: pos,
            size,
            connections: Vec::new(),
        };
        let nid = level.graph.add_node_with_pos(room.clone(), pos);
        level.rooms.insert(nid, room);
        node_ids.push(nid);
    }

    if room_count <= 1 { return level; }

    // Build minimum spanning tree using Prim's algorithm for connectivity
    let mut in_tree: HashSet<NodeId> = HashSet::new();
    in_tree.insert(node_ids[0]);
    let mut edges_added = Vec::new();

    while in_tree.len() < room_count {
        let mut best_dist = f32::INFINITY;
        let mut best_pair = (node_ids[0], node_ids[1]);

        for &a in &in_tree {
            for &b in &node_ids {
                if in_tree.contains(&b) { continue; }
                let pa = level.graph.node_position(a);
                let pb = level.graph.node_position(b);
                let dist = (pa - pb).length();
                if dist < best_dist {
                    best_dist = dist;
                    best_pair = (a, b);
                }
            }
        }

        let (a, b) = best_pair;
        in_tree.insert(b);
        let eid = level.graph.add_edge_weighted(a, b, best_dist, best_dist);
        edges_added.push((a, b));
        level.rooms.get_mut(&a).unwrap().connections.push(b);
        level.rooms.get_mut(&b).unwrap().connections.push(a);
    }

    // Add extra edges based on connectivity
    let max_extra = (room_count as f32 * connectivity * 1.5) as usize;
    let mut seed_counter = seed + 10000;
    let mut extra_added = 0;
    for i in 0..room_count {
        if extra_added >= max_extra { break; }
        for j in (i + 1)..room_count {
            if extra_added >= max_extra { break; }
            let a = node_ids[i];
            let b = node_ids[j];
            if level.graph.find_edge(a, b).is_some() { continue; }

            let pa = level.graph.node_position(a);
            let pb = level.graph.node_position(b);
            let dist = (pa - pb).length();
            let threshold = spread * 0.3;

            if dist < threshold && pseudo_random(seed_counter, (i * room_count + j) as u64) < connectivity as f64 * 0.5 {
                seed_counter += 1;
                level.graph.add_edge_weighted(a, b, dist, dist);
                level.rooms.get_mut(&a).unwrap().connections.push(b);
                level.rooms.get_mut(&b).unwrap().connections.push(a);
                extra_added += 1;
            }
        }
    }

    // Force-directed layout refinement then snap to grid
    apply_force_layout(&mut level, 50);
    snap_to_grid(&mut level, 50.0);

    level
}

fn apply_force_layout(level: &mut LevelGraph, iterations: usize) {
    let node_ids = level.graph.node_ids();
    let n = node_ids.len();
    if n <= 1 { return; }

    let k = 120.0f32; // optimal distance
    let mut temperature = 200.0f32;

    for _ in 0..iterations {
        let mut displacements: HashMap<NodeId, Vec2> = HashMap::new();
        for &nid in &node_ids {
            displacements.insert(nid, Vec2::ZERO);
        }

        // Repulsive forces
        for i in 0..n {
            for j in (i + 1)..n {
                let ni = node_ids[i];
                let nj = node_ids[j];
                let pi = level.graph.node_position(ni);
                let pj = level.graph.node_position(nj);
                let delta = pi - pj;
                let dist = delta.length().max(1.0);
                let force = k * k / dist;
                let d = delta / dist * force;
                *displacements.get_mut(&ni).unwrap() += d;
                *displacements.get_mut(&nj).unwrap() -= d;
            }
        }

        // Attractive forces along edges
        for edge in level.graph.edges() {
            let pi = level.graph.node_position(edge.from);
            let pj = level.graph.node_position(edge.to);
            let delta = pi - pj;
            let dist = delta.length().max(1.0);
            let force = dist * dist / k;
            let d = delta / dist * force;
            *displacements.get_mut(&edge.from).unwrap() -= d;
            *displacements.get_mut(&edge.to).unwrap() += d;
        }

        for &nid in &node_ids {
            let disp = displacements[&nid];
            let len = disp.length().max(0.01);
            let clamped = disp / len * len.min(temperature);
            let pos = level.graph.node_position(nid) + clamped;
            level.graph.set_node_position(nid, pos);
        }

        temperature *= 0.95;
    }

    // Update room positions
    for &nid in &node_ids {
        let pos = level.graph.node_position(nid);
        if let Some(room) = level.rooms.get_mut(&nid) {
            room.position = pos;
        }
    }
}

fn snap_to_grid(level: &mut LevelGraph, grid_size: f32) {
    for nid in level.graph.node_ids() {
        let pos = level.graph.node_position(nid);
        let snapped = Vec2::new(
            (pos.x / grid_size).round() * grid_size,
            (pos.y / grid_size).round() * grid_size,
        );
        level.graph.set_node_position(nid, snapped);
        if let Some(room) = level.rooms.get_mut(&nid) {
            room.position = snapped;
        }
    }
}

/// Generate a corridor path between two positions.
/// Uses L-shaped corridors (horizontal then vertical) or straight if aligned.
pub fn corridor_path(from: Vec2, to: Vec2) -> Vec<Vec2> {
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();

    if dx < 1.0 || dy < 1.0 {
        // Straight corridor
        vec![from, to]
    } else {
        // L-shaped: go horizontal first, then vertical
        let midpoint = Vec2::new(to.x, from.y);
        vec![from, midpoint, to]
    }
}

/// Alternative corridor: Z-shaped (horizontal, vertical, horizontal).
pub fn corridor_path_z(from: Vec2, to: Vec2) -> Vec<Vec2> {
    let mid_y = (from.y + to.y) / 2.0;
    vec![
        from,
        Vec2::new(from.x, mid_y),
        Vec2::new(to.x, mid_y),
        to,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_dungeon_basic() {
        let level = generate_dungeon(10, 0.3);
        assert_eq!(level.room_count(), 10);
        assert!(level.corridor_count() >= 9); // at least spanning tree
        assert!(level.is_connected());
    }

    #[test]
    fn test_generate_dungeon_minimal() {
        let level = generate_dungeon(2, 0.0);
        assert_eq!(level.room_count(), 2);
        assert_eq!(level.corridor_count(), 1);
        assert!(level.is_connected());
    }

    #[test]
    fn test_generate_dungeon_empty() {
        let level = generate_dungeon(0, 0.5);
        assert_eq!(level.room_count(), 0);
    }

    #[test]
    fn test_generate_dungeon_single() {
        let level = generate_dungeon(1, 0.5);
        assert_eq!(level.room_count(), 1);
        assert_eq!(level.corridor_count(), 0);
    }

    #[test]
    fn test_connectivity_increases_edges() {
        let low = generate_dungeon_seeded(15, 0.0, 999);
        let high = generate_dungeon_seeded(15, 1.0, 999);
        assert!(high.corridor_count() >= low.corridor_count());
    }

    #[test]
    fn test_room_types() {
        let level = generate_dungeon(20, 0.3);
        let rooms: Vec<&RoomNode> = level.rooms.values().collect();
        let start_count = rooms.iter().filter(|r| r.room_type == RoomType::Start).count();
        let end_count = rooms.iter().filter(|r| r.room_type == RoomType::End).count();
        assert_eq!(start_count, 1);
        assert_eq!(end_count, 1);
    }

    #[test]
    fn test_corridor_path_straight() {
        let path = corridor_path(Vec2::new(0.0, 5.0), Vec2::new(10.0, 5.0));
        assert_eq!(path.len(), 2);
    }

    #[test]
    fn test_corridor_path_l_shaped() {
        let path = corridor_path(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        assert_eq!(path.len(), 3);
        assert_eq!(path[1], Vec2::new(10.0, 0.0)); // horizontal then vertical
    }

    #[test]
    fn test_corridor_path_z() {
        let path = corridor_path_z(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn test_grid_snapping() {
        let level = generate_dungeon(5, 0.0);
        for nid in level.room_ids() {
            let pos = level.graph.node_position(nid);
            assert_eq!(pos.x % 50.0, 0.0, "X not snapped: {}", pos.x);
            assert_eq!(pos.y % 50.0, 0.0, "Y not snapped: {}", pos.y);
        }
    }

    #[test]
    fn test_rooms_have_sizes() {
        let level = generate_dungeon(5, 0.3);
        for room in level.rooms.values() {
            assert!(room.size.x >= 40.0);
            assert!(room.size.y >= 40.0);
        }
    }
}
