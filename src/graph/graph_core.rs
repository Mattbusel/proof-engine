use glam::Vec2;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphKind {
    Directed,
    Undirected,
}

#[derive(Debug, Clone)]
pub struct NodeData<N> {
    pub id: NodeId,
    pub data: N,
    pub position: Vec2,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct EdgeData<E> {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub data: E,
    pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct Graph<N, E> {
    pub kind: GraphKind,
    nodes: HashMap<NodeId, NodeData<N>>,
    edges: HashMap<EdgeId, EdgeData<E>>,
    adjacency: HashMap<NodeId, Vec<(NodeId, EdgeId)>>,
    next_node_id: u32,
    next_edge_id: u32,
}

impl<N, E> Graph<N, E> {
    pub fn new(kind: GraphKind) -> Self {
        Self {
            kind,
            nodes: HashMap::new(),
            edges: HashMap::new(),
            adjacency: HashMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
        }
    }

    pub fn add_node(&mut self, data: N) -> NodeId {
        self.add_node_with_pos(data, Vec2::ZERO)
    }

    pub fn add_node_with_pos(&mut self, data: N, position: Vec2) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        self.nodes.insert(id, NodeData { id, data, position, weight: 1.0 });
        self.adjacency.insert(id, Vec::new());
        id
    }

    pub fn add_edge(&mut self, from: NodeId, to: NodeId, data: E) -> EdgeId {
        self.add_edge_weighted(from, to, data, 1.0)
    }

    pub fn add_edge_weighted(&mut self, from: NodeId, to: NodeId, data: E, weight: f32) -> EdgeId {
        let id = EdgeId(self.next_edge_id);
        self.next_edge_id += 1;
        self.edges.insert(id, EdgeData { id, from, to, data, weight });
        if let Some(adj) = self.adjacency.get_mut(&from) {
            adj.push((to, id));
        }
        if self.kind == GraphKind::Undirected {
            if let Some(adj) = self.adjacency.get_mut(&to) {
                adj.push((from, id));
            }
        }
        id
    }

    pub fn remove_node(&mut self, id: NodeId) {
        self.nodes.remove(&id);
        self.adjacency.remove(&id);
        // Remove all edges referencing this node
        let edge_ids: Vec<EdgeId> = self.edges.iter()
            .filter(|(_, e)| e.from == id || e.to == id)
            .map(|(eid, _)| *eid)
            .collect();
        for eid in &edge_ids {
            self.edges.remove(eid);
        }
        // Clean adjacency lists
        for (_, adj) in self.adjacency.iter_mut() {
            adj.retain(|(nid, eid)| *nid != id && !edge_ids.contains(eid));
        }
    }

    pub fn remove_edge(&mut self, id: EdgeId) {
        if let Some(edge) = self.edges.remove(&id) {
            if let Some(adj) = self.adjacency.get_mut(&edge.from) {
                adj.retain(|(_, eid)| *eid != id);
            }
            if self.kind == GraphKind::Undirected {
                if let Some(adj) = self.adjacency.get_mut(&edge.to) {
                    adj.retain(|(_, eid)| *eid != id);
                }
            }
        }
    }

    pub fn neighbors(&self, id: NodeId) -> Vec<NodeId> {
        self.adjacency.get(&id)
            .map(|adj| adj.iter().map(|(nid, _)| *nid).collect())
            .unwrap_or_default()
    }

    pub fn neighbor_edges(&self, id: NodeId) -> Vec<(NodeId, EdgeId)> {
        self.adjacency.get(&id).cloned().unwrap_or_default()
    }

    pub fn degree(&self, id: NodeId) -> usize {
        self.adjacency.get(&id).map(|a| a.len()).unwrap_or(0)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn has_node(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    pub fn has_edge(&self, id: EdgeId) -> bool {
        self.edges.contains_key(&id)
    }

    pub fn get_node(&self, id: NodeId) -> Option<&NodeData<N>> {
        self.nodes.get(&id)
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut NodeData<N>> {
        self.nodes.get_mut(&id)
    }

    pub fn get_edge(&self, id: EdgeId) -> Option<&EdgeData<E>> {
        self.edges.get(&id)
    }

    pub fn get_edge_mut(&mut self, id: EdgeId) -> Option<&mut EdgeData<E>> {
        self.edges.get_mut(&id)
    }

    pub fn find_edge(&self, from: NodeId, to: NodeId) -> Option<EdgeId> {
        self.adjacency.get(&from)
            .and_then(|adj| adj.iter().find(|(nid, _)| *nid == to).map(|(_, eid)| *eid))
    }

    pub fn edge_weight(&self, id: EdgeId) -> f32 {
        self.edges.get(&id).map(|e| e.weight).unwrap_or(f32::INFINITY)
    }

    pub fn set_node_position(&mut self, id: NodeId, pos: Vec2) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.position = pos;
        }
    }

    pub fn node_position(&self, id: NodeId) -> Vec2 {
        self.nodes.get(&id).map(|n| n.position).unwrap_or(Vec2::ZERO)
    }

    // Iterators
    pub fn nodes(&self) -> impl Iterator<Item = &NodeData<N>> {
        self.nodes.values()
    }

    pub fn node_ids(&self) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        ids.sort();
        ids
    }

    pub fn edges(&self) -> impl Iterator<Item = &EdgeData<E>> {
        self.edges.values()
    }

    pub fn edge_ids(&self) -> Vec<EdgeId> {
        let mut ids: Vec<EdgeId> = self.edges.keys().copied().collect();
        ids.sort();
        ids
    }

    pub fn bfs(&self, start: NodeId) -> BfsIterator<N, E> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        if self.has_node(start) {
            queue.push_back(start);
            visited.insert(start);
        }
        BfsIterator { graph: self, queue, visited }
    }

    pub fn dfs(&self, start: NodeId) -> DfsIterator<N, E> {
        let mut stack = Vec::new();
        let mut visited = HashSet::new();
        if self.has_node(start) {
            stack.push(start);
            visited.insert(start);
        }
        DfsIterator { graph: self, stack, visited }
    }

    /// Extract a subgraph containing only the given node IDs
    pub fn subgraph(&self, node_ids: &[NodeId]) -> Graph<N, E>
    where
        N: Clone,
        E: Clone,
    {
        let set: HashSet<NodeId> = node_ids.iter().copied().collect();
        let mut sub = Graph::new(self.kind);
        // We need to preserve node IDs, so we manipulate internals
        for &nid in node_ids {
            if let Some(nd) = self.nodes.get(&nid) {
                sub.nodes.insert(nid, NodeData {
                    id: nid,
                    data: nd.data.clone(),
                    position: nd.position,
                    weight: nd.weight,
                });
                sub.adjacency.insert(nid, Vec::new());
            }
        }
        sub.next_node_id = self.next_node_id;
        sub.next_edge_id = self.next_edge_id;
        for (eid, ed) in &self.edges {
            if set.contains(&ed.from) && set.contains(&ed.to) {
                sub.edges.insert(*eid, EdgeData {
                    id: *eid,
                    from: ed.from,
                    to: ed.to,
                    data: ed.data.clone(),
                    weight: ed.weight,
                });
                if let Some(adj) = sub.adjacency.get_mut(&ed.from) {
                    adj.push((ed.to, *eid));
                }
                if self.kind == GraphKind::Undirected {
                    if let Some(adj) = sub.adjacency.get_mut(&ed.to) {
                        adj.push((ed.from, *eid));
                    }
                }
            }
        }
        sub
    }

    /// Union of two graphs (combines all nodes and edges)
    pub fn union(&self, other: &Graph<N, E>) -> Graph<N, E>
    where
        N: Clone,
        E: Clone,
    {
        let mut result = self.clone();
        let node_offset = result.next_node_id;
        let edge_offset = result.next_edge_id;
        let mut node_map: HashMap<NodeId, NodeId> = HashMap::new();
        for nd in other.nodes.values() {
            let new_id = NodeId(nd.id.0 + node_offset);
            node_map.insert(nd.id, new_id);
            result.nodes.insert(new_id, NodeData {
                id: new_id,
                data: nd.data.clone(),
                position: nd.position,
                weight: nd.weight,
            });
            result.adjacency.insert(new_id, Vec::new());
        }
        result.next_node_id = node_offset + other.next_node_id;
        for ed in other.edges.values() {
            let new_eid = EdgeId(ed.id.0 + edge_offset);
            let new_from = node_map[&ed.from];
            let new_to = node_map[&ed.to];
            result.edges.insert(new_eid, EdgeData {
                id: new_eid,
                from: new_from,
                to: new_to,
                data: ed.data.clone(),
                weight: ed.weight,
            });
            if let Some(adj) = result.adjacency.get_mut(&new_from) {
                adj.push((new_to, new_eid));
            }
            if result.kind == GraphKind::Undirected {
                if let Some(adj) = result.adjacency.get_mut(&new_to) {
                    adj.push((new_from, new_eid));
                }
            }
        }
        result.next_edge_id = edge_offset + other.next_edge_id;
        result
    }

    /// Complement graph: has edges where original doesn't, and vice versa
    pub fn complement(&self) -> Graph<N, E>
    where
        N: Clone,
        E: Default + Clone,
    {
        let mut result = Graph::new(self.kind);
        for nd in self.nodes.values() {
            result.nodes.insert(nd.id, NodeData {
                id: nd.id,
                data: nd.data.clone(),
                position: nd.position,
                weight: nd.weight,
            });
            result.adjacency.insert(nd.id, Vec::new());
        }
        result.next_node_id = self.next_node_id;

        let node_ids = self.node_ids();
        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let a = node_ids[i];
                let b = node_ids[j];
                let has_edge = self.find_edge(a, b).is_some()
                    || (self.kind == GraphKind::Undirected && self.find_edge(b, a).is_some());
                if !has_edge {
                    let eid = EdgeId(result.next_edge_id);
                    result.next_edge_id += 1;
                    result.edges.insert(eid, EdgeData {
                        id: eid, from: a, to: b, data: E::default(), weight: 1.0,
                    });
                    if let Some(adj) = result.adjacency.get_mut(&a) {
                        adj.push((b, eid));
                    }
                    if result.kind == GraphKind::Undirected {
                        if let Some(adj) = result.adjacency.get_mut(&b) {
                            adj.push((a, eid));
                        }
                    }
                }
            }
        }
        result
    }
}

impl<N: Clone, E: Clone> Graph<N, E> {
    pub fn to_adjacency_matrix(&self) -> AdjacencyMatrix {
        let node_ids = self.node_ids();
        let n = node_ids.len();
        let mut index_map: HashMap<NodeId, usize> = HashMap::new();
        for (i, &nid) in node_ids.iter().enumerate() {
            index_map.insert(nid, i);
        }
        let mut matrix = vec![vec![0.0f32; n]; n];
        for ed in self.edges.values() {
            if let (Some(&i), Some(&j)) = (index_map.get(&ed.from), index_map.get(&ed.to)) {
                matrix[i][j] = ed.weight;
                if self.kind == GraphKind::Undirected {
                    matrix[j][i] = ed.weight;
                }
            }
        }
        AdjacencyMatrix { matrix, node_ids, index_map }
    }

    pub fn to_edge_list(&self) -> EdgeList {
        let edges: Vec<(NodeId, NodeId, f32)> = self.edges.values()
            .map(|e| (e.from, e.to, e.weight))
            .collect();
        let node_ids = self.node_ids();
        EdgeList { edges, node_ids }
    }
}

// BFS iterator
pub struct BfsIterator<'a, N, E> {
    graph: &'a Graph<N, E>,
    queue: VecDeque<NodeId>,
    visited: HashSet<NodeId>,
}

impl<'a, N, E> Iterator for BfsIterator<'a, N, E> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let current = self.queue.pop_front()?;
        for &(neighbor, _) in self.graph.adjacency.get(&current).unwrap_or(&Vec::new()) {
            if self.visited.insert(neighbor) {
                self.queue.push_back(neighbor);
            }
        }
        Some(current)
    }
}

// DFS iterator
pub struct DfsIterator<'a, N, E> {
    graph: &'a Graph<N, E>,
    stack: Vec<NodeId>,
    visited: HashSet<NodeId>,
}

impl<'a, N, E> Iterator for DfsIterator<'a, N, E> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        let current = self.stack.pop()?;
        for &(neighbor, _) in self.graph.adjacency.get(&current).unwrap_or(&Vec::new()) {
            if self.visited.insert(neighbor) {
                self.stack.push(neighbor);
            }
        }
        Some(current)
    }
}

// Adjacency matrix representation
#[derive(Debug, Clone)]
pub struct AdjacencyMatrix {
    pub matrix: Vec<Vec<f32>>,
    pub node_ids: Vec<NodeId>,
    pub index_map: HashMap<NodeId, usize>,
}

impl AdjacencyMatrix {
    pub fn to_graph(&self, kind: GraphKind) -> Graph<(), ()> {
        let mut g = Graph::new(kind);
        let mut id_map: HashMap<usize, NodeId> = HashMap::new();
        for (i, &orig_id) in self.node_ids.iter().enumerate() {
            let nid = g.add_node(());
            id_map.insert(i, nid);
        }
        let n = self.matrix.len();
        for i in 0..n {
            let start_j = if kind == GraphKind::Undirected { i + 1 } else { 0 };
            for j in start_j..n {
                if self.matrix[i][j] != 0.0 {
                    g.add_edge_weighted(id_map[&i], id_map[&j], (), self.matrix[i][j]);
                }
            }
        }
        g
    }

    pub fn get(&self, from: NodeId, to: NodeId) -> f32 {
        let i = self.index_map.get(&from).copied().unwrap_or(0);
        let j = self.index_map.get(&to).copied().unwrap_or(0);
        self.matrix[i][j]
    }
}

// Edge list representation
#[derive(Debug, Clone)]
pub struct EdgeList {
    pub edges: Vec<(NodeId, NodeId, f32)>,
    pub node_ids: Vec<NodeId>,
}

impl EdgeList {
    pub fn to_graph(&self, kind: GraphKind) -> Graph<(), ()> {
        let mut g = Graph::new(kind);
        let mut id_map: HashMap<NodeId, NodeId> = HashMap::new();
        for &orig_id in &self.node_ids {
            let nid = g.add_node(());
            id_map.insert(orig_id, nid);
        }
        for &(from, to, w) in &self.edges {
            if let (Some(&f), Some(&t)) = (id_map.get(&from), id_map.get(&to)) {
                g.add_edge_weighted(f, t, (), w);
            }
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_remove_nodes() {
        let mut g: Graph<&str, ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node("A");
        let b = g.add_node("B");
        let c = g.add_node("C");
        assert_eq!(g.node_count(), 3);
        g.remove_node(b);
        assert_eq!(g.node_count(), 2);
        assert!(!g.has_node(b));
    }

    #[test]
    fn test_add_remove_edges() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let e = g.add_edge(a, b, ());
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.degree(a), 1);
        assert_eq!(g.degree(b), 1);
        g.remove_edge(e);
        assert_eq!(g.edge_count(), 0);
        assert_eq!(g.degree(a), 0);
    }

    #[test]
    fn test_neighbors() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(a, c, ());
        let mut nbrs = g.neighbors(a);
        nbrs.sort();
        assert_eq!(nbrs, vec![b, c]);
    }

    #[test]
    fn test_directed() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Directed);
        let a = g.add_node(());
        let b = g.add_node(());
        g.add_edge(a, b, ());
        assert_eq!(g.neighbors(a), vec![b]);
        assert!(g.neighbors(b).is_empty());
    }

    #[test]
    fn test_bfs() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        let d = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.add_edge(c, d, ());
        let bfs: Vec<NodeId> = g.bfs(a).collect();
        assert_eq!(bfs.len(), 4);
        assert_eq!(bfs[0], a);
    }

    #[test]
    fn test_dfs() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(a, c, ());
        let dfs: Vec<NodeId> = g.dfs(a).collect();
        assert_eq!(dfs.len(), 3);
        assert_eq!(dfs[0], a);
    }

    #[test]
    fn test_adjacency_matrix_roundtrip() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge_weighted(a, b, (), 2.0);
        g.add_edge_weighted(b, c, (), 3.0);
        let mat = g.to_adjacency_matrix();
        assert_eq!(mat.matrix.len(), 3);
        let g2 = mat.to_graph(GraphKind::Undirected);
        assert_eq!(g2.node_count(), 3);
        assert_eq!(g2.edge_count(), 2);
    }

    #[test]
    fn test_edge_list() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        g.add_edge(a, b, ());
        let el = g.to_edge_list();
        assert_eq!(el.edges.len(), 1);
        let g2 = el.to_graph(GraphKind::Undirected);
        assert_eq!(g2.edge_count(), 1);
    }

    #[test]
    fn test_subgraph() {
        let mut g: Graph<i32, ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(1);
        let b = g.add_node(2);
        let c = g.add_node(3);
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.add_edge(a, c, ());
        let sub = g.subgraph(&[a, b]);
        assert_eq!(sub.node_count(), 2);
        assert_eq!(sub.edge_count(), 1);
    }

    #[test]
    fn test_union() {
        let mut g1: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g1.add_node(());
        let b = g1.add_node(());
        g1.add_edge(a, b, ());

        let mut g2: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let c = g2.add_node(());
        let d = g2.add_node(());
        g2.add_edge(c, d, ());

        let u = g1.union(&g2);
        assert_eq!(u.node_count(), 4);
        assert_eq!(u.edge_count(), 2);
    }

    #[test]
    fn test_complement() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        // Complete graph has 3 edges, we have 1, complement should have 2
        let comp = g.complement();
        assert_eq!(comp.edge_count(), 2);
    }

    #[test]
    fn test_find_edge() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let e = g.add_edge(a, b, ());
        assert_eq!(g.find_edge(a, b), Some(e));
        assert_eq!(g.find_edge(b, a), Some(e));
    }

    #[test]
    fn test_node_position() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node_with_pos((), Vec2::new(3.0, 4.0));
        assert_eq!(g.node_position(a), Vec2::new(3.0, 4.0));
        g.set_node_position(a, Vec2::new(1.0, 2.0));
        assert_eq!(g.node_position(a), Vec2::new(1.0, 2.0));
    }

    #[test]
    fn test_weighted_edge() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Directed);
        let a = g.add_node(());
        let b = g.add_node(());
        let e = g.add_edge_weighted(a, b, (), 5.0);
        assert_eq!(g.edge_weight(e), 5.0);
    }

    #[test]
    fn test_remove_node_removes_edges() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.remove_node(b);
        assert_eq!(g.edge_count(), 0);
        assert_eq!(g.degree(a), 0);
        assert_eq!(g.degree(c), 0);
    }
}
