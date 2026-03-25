use glam::Vec2;
use std::collections::{HashMap, HashSet};
use super::graph_core::{Graph, GraphKind, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HyperedgeId(pub u32);

#[derive(Debug, Clone)]
pub struct HypernodeData<N> {
    pub id: NodeId,
    pub data: N,
    pub position: Vec2,
}

#[derive(Debug, Clone)]
pub struct HyperedgeData {
    pub id: HyperedgeId,
    pub members: Vec<NodeId>,
    pub color: [f32; 4], // RGBA
}

#[derive(Debug, Clone)]
pub struct Hypergraph<N> {
    nodes: HashMap<NodeId, HypernodeData<N>>,
    hyperedges: HashMap<HyperedgeId, HyperedgeData>,
    next_node_id: u32,
    next_edge_id: u32,
}

impl<N> Hypergraph<N> {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            hyperedges: HashMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
        }
    }

    pub fn add_node(&mut self, data: N, position: Vec2) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        self.nodes.insert(id, HypernodeData { id, data, position });
        id
    }

    pub fn add_hyperedge(&mut self, members: Vec<NodeId>) -> HyperedgeId {
        self.add_hyperedge_colored(members, [0.3, 0.5, 0.8, 0.3])
    }

    pub fn add_hyperedge_colored(&mut self, members: Vec<NodeId>, color: [f32; 4]) -> HyperedgeId {
        let id = HyperedgeId(self.next_edge_id);
        self.next_edge_id += 1;
        self.hyperedges.insert(id, HyperedgeData { id, members, color });
        id
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.remove(&id);
        // Remove from all hyperedges
        for he in self.hyperedges.values_mut() {
            he.members.retain(|&m| m != id);
        }
        // Remove empty hyperedges
        self.hyperedges.retain(|_, he| !he.members.is_empty());
    }

    pub fn remove_hyperedge(&mut self, id: HyperedgeId) {
        self.hyperedges.remove(&id);
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn hyperedge_count(&self) -> usize {
        self.hyperedges.len()
    }

    pub fn get_node(&self, id: NodeId) -> Option<&HypernodeData<N>> {
        self.nodes.get(&id)
    }

    pub fn get_hyperedge(&self, id: HyperedgeId) -> Option<&HyperedgeData> {
        self.hyperedges.get(&id)
    }

    pub fn node_ids(&self) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        ids.sort();
        ids
    }

    pub fn hyperedge_ids(&self) -> Vec<HyperedgeId> {
        let mut ids: Vec<HyperedgeId> = self.hyperedges.keys().copied().collect();
        ids.sort();
        ids
    }

    /// Compute convex hull of member node positions for rendering a hyperedge.
    pub fn convex_hull(&self, he_id: HyperedgeId) -> Vec<Vec2> {
        let he = match self.hyperedges.get(&he_id) {
            Some(he) => he,
            None => return Vec::new(),
        };

        let points: Vec<Vec2> = he.members.iter()
            .filter_map(|nid| self.nodes.get(nid).map(|n| n.position))
            .collect();

        if points.len() <= 2 {
            return points;
        }

        convex_hull_2d(&points)
    }

    /// Convert hypergraph to bipartite graph representation.
    /// Each hyperedge becomes a node, connected to all its member nodes.
    pub fn to_bipartite(&self) -> Graph<String, ()>
    where
        N: std::fmt::Debug,
    {
        let mut g = Graph::new(GraphKind::Undirected);
        let mut node_map: HashMap<NodeId, NodeId> = HashMap::new();
        let mut hedge_map: HashMap<HyperedgeId, NodeId> = HashMap::new();

        // Add original nodes
        for (&nid, nd) in &self.nodes {
            let gid = g.add_node_with_pos(format!("node_{}", nid.0), nd.position);
            node_map.insert(nid, gid);
        }

        // Add hyperedge nodes
        for (&hid, _) in &self.hyperedges {
            let gid = g.add_node(format!("hedge_{}", hid.0));
            hedge_map.insert(hid, gid);
        }

        // Connect hyperedge nodes to their members
        for (&hid, he) in &self.hyperedges {
            let hedge_gid = hedge_map[&hid];
            for &member in &he.members {
                if let Some(&member_gid) = node_map.get(&member) {
                    g.add_edge(hedge_gid, member_gid, ());
                }
            }
        }

        g
    }

    /// Create a hypergraph from a bipartite graph.
    /// Nodes labeled "hedge_*" become hyperedges, others become nodes.
    pub fn from_bipartite(graph: &Graph<String, ()>) -> Hypergraph<String> {
        let mut hg = Hypergraph::new();
        let mut node_map: HashMap<NodeId, NodeId> = HashMap::new();
        let mut hedge_nodes: Vec<NodeId> = Vec::new();

        for nd in graph.nodes() {
            if nd.data.starts_with("hedge_") {
                hedge_nodes.push(nd.id);
            } else {
                let hid = hg.add_node(nd.data.clone(), nd.position);
                node_map.insert(nd.id, hid);
            }
        }

        for &hn in &hedge_nodes {
            let members: Vec<NodeId> = graph.neighbors(hn)
                .iter()
                .filter_map(|&nbr| node_map.get(&nbr).copied())
                .collect();
            if !members.is_empty() {
                hg.add_hyperedge(members);
            }
        }

        hg
    }
}

/// Compute 2D convex hull using Graham scan.
fn convex_hull_2d(points: &[Vec2]) -> Vec<Vec2> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut pts = points.to_vec();

    // Find lowest point (and leftmost if tie)
    let mut lowest = 0;
    for i in 1..pts.len() {
        if pts[i].y < pts[lowest].y || (pts[i].y == pts[lowest].y && pts[i].x < pts[lowest].x) {
            lowest = i;
        }
    }
    pts.swap(0, lowest);
    let pivot = pts[0];

    // Sort by polar angle
    pts[1..].sort_by(|a, b| {
        let da = *a - pivot;
        let db = *b - pivot;
        let angle_a = da.y.atan2(da.x);
        let angle_b = db.y.atan2(db.x);
        angle_a.partial_cmp(&angle_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut hull: Vec<Vec2> = Vec::new();
    for &p in &pts {
        while hull.len() >= 2 {
            let a = hull[hull.len() - 2];
            let b = hull[hull.len() - 1];
            let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            if cross <= 0.0 {
                hull.pop();
            } else {
                break;
            }
        }
        hull.push(p);
    }

    hull
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_hypergraph() {
        let mut hg: Hypergraph<&str> = Hypergraph::new();
        let a = hg.add_node("A", Vec2::new(0.0, 0.0));
        let b = hg.add_node("B", Vec2::new(10.0, 0.0));
        let c = hg.add_node("C", Vec2::new(5.0, 10.0));
        let he = hg.add_hyperedge(vec![a, b, c]);
        assert_eq!(hg.node_count(), 3);
        assert_eq!(hg.hyperedge_count(), 1);
        assert_eq!(hg.get_hyperedge(he).unwrap().members.len(), 3);
    }

    #[test]
    fn test_remove_node_from_hyperedge() {
        let mut hg: Hypergraph<()> = Hypergraph::new();
        let a = hg.add_node((), Vec2::ZERO);
        let b = hg.add_node((), Vec2::ZERO);
        let c = hg.add_node((), Vec2::ZERO);
        let he = hg.add_hyperedge(vec![a, b, c]);
        hg.remove_node(b);
        assert_eq!(hg.node_count(), 2);
        assert_eq!(hg.get_hyperedge(he).unwrap().members.len(), 2);
    }

    #[test]
    fn test_remove_all_members_removes_hyperedge() {
        let mut hg: Hypergraph<()> = Hypergraph::new();
        let a = hg.add_node((), Vec2::ZERO);
        let he = hg.add_hyperedge(vec![a]);
        hg.remove_node(a);
        assert_eq!(hg.hyperedge_count(), 0);
    }

    #[test]
    fn test_convex_hull() {
        let mut hg: Hypergraph<()> = Hypergraph::new();
        let a = hg.add_node((), Vec2::new(0.0, 0.0));
        let b = hg.add_node((), Vec2::new(10.0, 0.0));
        let c = hg.add_node((), Vec2::new(10.0, 10.0));
        let d = hg.add_node((), Vec2::new(0.0, 10.0));
        let e = hg.add_node((), Vec2::new(5.0, 5.0)); // interior point
        let he = hg.add_hyperedge(vec![a, b, c, d, e]);
        let hull = hg.convex_hull(he);
        assert_eq!(hull.len(), 4); // interior point excluded
    }

    #[test]
    fn test_bipartite_roundtrip() {
        let mut hg: Hypergraph<String> = Hypergraph::new();
        let a = hg.add_node("a".into(), Vec2::new(0.0, 0.0));
        let b = hg.add_node("b".into(), Vec2::new(1.0, 0.0));
        let c = hg.add_node("c".into(), Vec2::new(0.0, 1.0));
        hg.add_hyperedge(vec![a, b, c]);
        hg.add_hyperedge(vec![a, b]);

        let bip = hg.to_bipartite();
        assert_eq!(bip.node_count(), 5); // 3 nodes + 2 hyperedge nodes
        // 3 + 2 = 5 edges
        assert_eq!(bip.edge_count(), 5);

        let hg2 = Hypergraph::from_bipartite(&bip);
        assert_eq!(hg2.node_count(), 3);
        assert_eq!(hg2.hyperedge_count(), 2);
    }

    #[test]
    fn test_convex_hull_degenerate() {
        let hull = convex_hull_2d(&[Vec2::new(1.0, 1.0)]);
        assert_eq!(hull.len(), 1);
        let hull = convex_hull_2d(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)]);
        assert_eq!(hull.len(), 2);
    }

    #[test]
    fn test_multiple_hyperedges() {
        let mut hg: Hypergraph<()> = Hypergraph::new();
        let a = hg.add_node((), Vec2::ZERO);
        let b = hg.add_node((), Vec2::ZERO);
        let c = hg.add_node((), Vec2::ZERO);
        let d = hg.add_node((), Vec2::ZERO);
        hg.add_hyperedge(vec![a, b, c]);
        hg.add_hyperedge(vec![b, c, d]);
        hg.add_hyperedge(vec![a, d]);
        assert_eq!(hg.hyperedge_count(), 3);
    }
}
