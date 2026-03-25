use glam::{Vec2, Vec3, Vec4};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;
use super::graph_core::{Graph, NodeId, EdgeId};

#[derive(Debug, Clone)]
pub struct Path {
    pub nodes: Vec<NodeId>,
    pub total_weight: f32,
}

impl Path {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DijkstraEntry {
    node: NodeId,
    cost: f32,
}

impl Eq for DijkstraEntry {}

impl PartialOrd for DijkstraEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DijkstraEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

/// Dijkstra's shortest path from start to end.
pub fn dijkstra<N, E>(graph: &Graph<N, E>, start: NodeId, end: NodeId) -> Option<Path> {
    if !graph.has_node(start) || !graph.has_node(end) {
        return None;
    }
    if start == end {
        return Some(Path { nodes: vec![start], total_weight: 0.0 });
    }

    let mut dist: HashMap<NodeId, f32> = HashMap::new();
    let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
    let mut heap = BinaryHeap::new();

    dist.insert(start, 0.0);
    heap.push(DijkstraEntry { node: start, cost: 0.0 });

    while let Some(DijkstraEntry { node, cost }) = heap.pop() {
        if node == end {
            // Reconstruct path
            let mut path = Vec::new();
            let mut current = end;
            path.push(current);
            while let Some(&p) = prev.get(&current) {
                path.push(p);
                current = p;
            }
            path.reverse();
            return Some(Path { nodes: path, total_weight: cost });
        }

        if cost > dist.get(&node).copied().unwrap_or(f32::INFINITY) {
            continue;
        }

        for (nbr, eid) in graph.neighbor_edges(node) {
            let w = graph.edge_weight(eid);
            let new_cost = cost + w;
            if new_cost < dist.get(&nbr).copied().unwrap_or(f32::INFINITY) {
                dist.insert(nbr, new_cost);
                prev.insert(nbr, node);
                heap.push(DijkstraEntry { node: nbr, cost: new_cost });
            }
        }
    }

    None
}

/// A* shortest path with a heuristic function.
pub fn astar<N, E, H>(graph: &Graph<N, E>, start: NodeId, end: NodeId, heuristic: H) -> Option<Path>
where
    H: Fn(NodeId) -> f32,
{
    if !graph.has_node(start) || !graph.has_node(end) {
        return None;
    }
    if start == end {
        return Some(Path { nodes: vec![start], total_weight: 0.0 });
    }

    let mut g_score: HashMap<NodeId, f32> = HashMap::new();
    let mut prev: HashMap<NodeId, NodeId> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut closed = HashSet::new();

    g_score.insert(start, 0.0);
    heap.push(DijkstraEntry { node: start, cost: heuristic(start) });

    while let Some(DijkstraEntry { node, cost: _ }) = heap.pop() {
        if node == end {
            let total = g_score[&end];
            let mut path = Vec::new();
            let mut current = end;
            path.push(current);
            while let Some(&p) = prev.get(&current) {
                path.push(p);
                current = p;
            }
            path.reverse();
            return Some(Path { nodes: path, total_weight: total });
        }

        if !closed.insert(node) {
            continue;
        }

        let current_g = g_score[&node];

        for (nbr, eid) in graph.neighbor_edges(node) {
            if closed.contains(&nbr) { continue; }
            let w = graph.edge_weight(eid);
            let tentative_g = current_g + w;
            if tentative_g < g_score.get(&nbr).copied().unwrap_or(f32::INFINITY) {
                g_score.insert(nbr, tentative_g);
                prev.insert(nbr, node);
                let f = tentative_g + heuristic(nbr);
                heap.push(DijkstraEntry { node: nbr, cost: f });
            }
        }
    }

    None
}

/// Bellman-Ford: single-source shortest paths, handles negative weights.
/// Returns distances from start to all reachable nodes.
pub fn bellman_ford<N, E>(graph: &Graph<N, E>, start: NodeId) -> HashMap<NodeId, f32> {
    let node_ids = graph.node_ids();
    let mut dist: HashMap<NodeId, f32> = HashMap::new();
    for &nid in &node_ids {
        dist.insert(nid, f32::INFINITY);
    }
    dist.insert(start, 0.0);

    let n = node_ids.len();

    // Relax edges n-1 times
    for _ in 0..(n - 1).max(1) {
        let mut updated = false;
        for edge in graph.edges() {
            let du = dist.get(&edge.from).copied().unwrap_or(f32::INFINITY);
            if du < f32::INFINITY {
                let new_dist = du + edge.weight;
                let dv = dist.get(&edge.to).copied().unwrap_or(f32::INFINITY);
                if new_dist < dv {
                    dist.insert(edge.to, new_dist);
                    updated = true;
                }
            }
            // For undirected graphs, relax in both directions
            if graph.kind == super::graph_core::GraphKind::Undirected {
                let dv = dist.get(&edge.to).copied().unwrap_or(f32::INFINITY);
                if dv < f32::INFINITY {
                    let new_dist = dv + edge.weight;
                    let du_cur = dist.get(&edge.from).copied().unwrap_or(f32::INFINITY);
                    if new_dist < du_cur {
                        dist.insert(edge.from, new_dist);
                        updated = true;
                    }
                }
            }
        }
        if !updated { break; }
    }

    dist
}

/// Floyd-Warshall: all-pairs shortest paths.
pub fn all_pairs_shortest<N, E>(graph: &Graph<N, E>) -> HashMap<(NodeId, NodeId), f32> {
    let node_ids = graph.node_ids();
    let n = node_ids.len();
    let idx: HashMap<NodeId, usize> = node_ids.iter().enumerate().map(|(i, &nid)| (nid, i)).collect();

    let mut dist = vec![vec![f32::INFINITY; n]; n];
    for i in 0..n {
        dist[i][i] = 0.0;
    }

    for edge in graph.edges() {
        if let (Some(&i), Some(&j)) = (idx.get(&edge.from), idx.get(&edge.to)) {
            dist[i][j] = dist[i][j].min(edge.weight);
            if graph.kind == super::graph_core::GraphKind::Undirected {
                dist[j][i] = dist[j][i].min(edge.weight);
            }
        }
    }

    // Floyd-Warshall relaxation
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                let through_k = dist[i][k] + dist[k][j];
                if through_k < dist[i][j] {
                    dist[i][j] = through_k;
                }
            }
        }
    }

    let mut result = HashMap::new();
    for i in 0..n {
        for j in 0..n {
            if dist[i][j] < f32::INFINITY {
                result.insert((node_ids[i], node_ids[j]), dist[i][j]);
            }
        }
    }
    result
}

/// Converts paths to visual glyph data: a glowing trail along edges.
pub struct PathVisualizer {
    pub trail_width: f32,
    pub trail_color: Vec4,
    pub glow_intensity: f32,
}

impl PathVisualizer {
    pub fn new() -> Self {
        Self {
            trail_width: 3.0,
            trail_color: Vec4::new(0.2, 0.8, 1.0, 1.0),
            glow_intensity: 1.5,
        }
    }

    pub fn with_color(mut self, color: Vec4) -> Self {
        self.trail_color = color;
        self
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.trail_width = width;
        self
    }

    /// Generate trail segments from a path and node positions.
    /// Returns Vec of (start_pos, end_pos, color, width) for each edge in the path.
    pub fn generate_trail<N, E>(
        &self,
        path: &Path,
        graph: &Graph<N, E>,
    ) -> Vec<TrailSegment> {
        let mut segments = Vec::new();
        if path.nodes.len() < 2 { return segments; }

        let total = path.nodes.len() - 1;
        for i in 0..total {
            let from = path.nodes[i];
            let to = path.nodes[i + 1];
            let p0 = graph.node_position(from);
            let p1 = graph.node_position(to);
            let progress = i as f32 / total as f32;
            // Glow fades along the trail
            let alpha = self.trail_color.w * (1.0 - progress * 0.5);
            let color = Vec4::new(
                self.trail_color.x * self.glow_intensity,
                self.trail_color.y * self.glow_intensity,
                self.trail_color.z * self.glow_intensity,
                alpha,
            );
            segments.push(TrailSegment {
                start: p0,
                end: p1,
                color,
                width: self.trail_width * (1.0 - progress * 0.3),
            });
        }
        segments
    }
}

#[derive(Debug, Clone)]
pub struct TrailSegment {
    pub start: Vec2,
    pub end: Vec2,
    pub color: Vec4,
    pub width: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::graph_core::{Graph, GraphKind, NodeId};

    fn make_weighted_graph() -> Graph<(), ()> {
        let mut g = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        let d = g.add_node(());
        g.add_edge_weighted(a, b, (), 1.0);
        g.add_edge_weighted(b, c, (), 2.0);
        g.add_edge_weighted(a, c, (), 10.0);
        g.add_edge_weighted(c, d, (), 1.0);
        g
    }

    #[test]
    fn test_dijkstra_shortest_path() {
        let g = make_weighted_graph();
        let ids = g.node_ids();
        let path = dijkstra(&g, ids[0], ids[3]).unwrap();
        assert_eq!(path.total_weight, 4.0); // a->b(1) + b->c(2) + c->d(1)
        assert_eq!(path.nodes.len(), 4);
    }

    #[test]
    fn test_dijkstra_same_node() {
        let g = make_weighted_graph();
        let ids = g.node_ids();
        let path = dijkstra(&g, ids[0], ids[0]).unwrap();
        assert_eq!(path.total_weight, 0.0);
        assert_eq!(path.nodes.len(), 1);
    }

    #[test]
    fn test_dijkstra_no_path() {
        let mut g = Graph::new(GraphKind::Directed);
        let a = g.add_node(());
        let b = g.add_node(());
        // No edge from a to b in directed graph
        assert!(dijkstra(&g, a, b).is_none());
    }

    #[test]
    fn test_astar() {
        let mut g = Graph::new(GraphKind::Undirected);
        let a = g.add_node_with_pos((), Vec2::new(0.0, 0.0));
        let b = g.add_node_with_pos((), Vec2::new(1.0, 0.0));
        let c = g.add_node_with_pos((), Vec2::new(2.0, 0.0));
        g.add_edge_weighted(a, b, (), 1.0);
        g.add_edge_weighted(b, c, (), 1.0);
        g.add_edge_weighted(a, c, (), 5.0);

        let positions: HashMap<NodeId, Vec2> = g.node_ids().iter()
            .map(|&nid| (nid, g.node_position(nid)))
            .collect();
        let goal_pos = positions[&c];

        let path = astar(&g, a, c, |nid| {
            let pos = positions.get(&nid).copied().unwrap_or(Vec2::ZERO);
            (pos - goal_pos).length()
        }).unwrap();

        assert_eq!(path.total_weight, 2.0);
        assert_eq!(path.nodes, vec![a, b, c]);
    }

    #[test]
    fn test_bellman_ford() {
        let g = make_weighted_graph();
        let ids = g.node_ids();
        let dist = bellman_ford(&g, ids[0]);
        assert_eq!(*dist.get(&ids[0]).unwrap(), 0.0);
        assert_eq!(*dist.get(&ids[1]).unwrap(), 1.0);
        assert_eq!(*dist.get(&ids[2]).unwrap(), 3.0);
        assert_eq!(*dist.get(&ids[3]).unwrap(), 4.0);
    }

    #[test]
    fn test_floyd_warshall() {
        let g = make_weighted_graph();
        let ids = g.node_ids();
        let apsp = all_pairs_shortest(&g);
        assert_eq!(*apsp.get(&(ids[0], ids[3])).unwrap(), 4.0);
        assert_eq!(*apsp.get(&(ids[3], ids[0])).unwrap(), 4.0);
        assert_eq!(*apsp.get(&(ids[0], ids[0])).unwrap(), 0.0);
    }

    #[test]
    fn test_path_visualizer() {
        let mut g = Graph::new(GraphKind::Undirected);
        let a = g.add_node_with_pos((), Vec2::new(0.0, 0.0));
        let b = g.add_node_with_pos((), Vec2::new(10.0, 0.0));
        let c = g.add_node_with_pos((), Vec2::new(20.0, 0.0));
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());

        let path = Path { nodes: vec![a, b, c], total_weight: 2.0 };
        let viz = PathVisualizer::new();
        let segments = viz.generate_trail(&path, &g);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start, Vec2::new(0.0, 0.0));
        assert_eq!(segments[0].end, Vec2::new(10.0, 0.0));
    }

    #[test]
    fn test_dijkstra_directed() {
        let mut g = Graph::new(GraphKind::Directed);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge_weighted(a, b, (), 1.0);
        g.add_edge_weighted(b, c, (), 1.0);
        g.add_edge_weighted(a, c, (), 5.0);
        let path = dijkstra(&g, a, c).unwrap();
        assert_eq!(path.total_weight, 2.0);
    }

    #[test]
    fn test_bellman_ford_directed() {
        let mut g = Graph::new(GraphKind::Directed);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge_weighted(a, b, (), 3.0);
        g.add_edge_weighted(b, c, (), 4.0);
        let dist = bellman_ford(&g, a);
        assert_eq!(*dist.get(&a).unwrap(), 0.0);
        assert_eq!(*dist.get(&b).unwrap(), 3.0);
        assert_eq!(*dist.get(&c).unwrap(), 7.0);
    }
}
