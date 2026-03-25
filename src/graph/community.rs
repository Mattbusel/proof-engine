use std::collections::{HashMap, HashSet};
use super::graph_core::{Graph, GraphKind, NodeId};

#[derive(Debug, Clone)]
pub struct Community {
    pub members: HashSet<NodeId>,
}

impl Community {
    pub fn new() -> Self {
        Self { members: HashSet::new() }
    }

    pub fn from_members(members: impl IntoIterator<Item = NodeId>) -> Self {
        Self { members: members.into_iter().collect() }
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.members.contains(&id)
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct CommunityResult {
    pub communities: Vec<Community>,
    pub modularity: f32,
    pub iterations: usize,
}

/// Compute modularity Q for a given partitioning of the graph.
/// Q = (1/2m) * sum_ij [ A_ij - k_i*k_j/(2m) ] * delta(c_i, c_j)
pub fn modularity<N, E>(graph: &Graph<N, E>, communities: &[Community]) -> f32 {
    let m = graph.edge_count() as f32;
    if m == 0.0 { return 0.0; }

    let m2 = if graph.kind == GraphKind::Undirected { 2.0 * m } else { m };

    // Build community assignment map
    let mut community_of: HashMap<NodeId, usize> = HashMap::new();
    for (ci, comm) in communities.iter().enumerate() {
        for &nid in &comm.members {
            community_of.insert(nid, ci);
        }
    }

    let mut q = 0.0f32;
    let node_ids = graph.node_ids();

    // Precompute degrees
    let degrees: HashMap<NodeId, f32> = node_ids.iter()
        .map(|&nid| (nid, graph.degree(nid) as f32))
        .collect();

    for edge in graph.edges() {
        let ci = community_of.get(&edge.from).copied().unwrap_or(usize::MAX);
        let cj = community_of.get(&edge.to).copied().unwrap_or(usize::MAX);
        if ci == cj {
            q += 1.0 - degrees[&edge.from] * degrees[&edge.to] / m2;
            if graph.kind == GraphKind::Undirected {
                q += 1.0 - degrees[&edge.to] * degrees[&edge.from] / m2;
            }
        }
    }

    q / m2
}

/// Louvain method for community detection.
/// Iteratively moves nodes to communities that maximize modularity gain.
pub fn louvain<N: Clone, E: Clone>(graph: &Graph<N, E>) -> CommunityResult {
    let node_ids = graph.node_ids();
    let n = node_ids.len();
    if n == 0 {
        return CommunityResult { communities: Vec::new(), modularity: 0.0, iterations: 0 };
    }

    let m = graph.edge_count() as f32;
    if m == 0.0 {
        let communities: Vec<Community> = node_ids.iter()
            .map(|&nid| Community::from_members(std::iter::once(nid)))
            .collect();
        return CommunityResult { communities, modularity: 0.0, iterations: 0 };
    }

    let m2 = if graph.kind == GraphKind::Undirected { 2.0 * m } else { m };

    // Each node starts in its own community
    let mut comm_of: HashMap<NodeId, usize> = HashMap::new();
    for (i, &nid) in node_ids.iter().enumerate() {
        comm_of.insert(nid, i);
    }
    let mut num_communities = n;

    // Precompute degrees and adjacency weights
    let degrees: HashMap<NodeId, f32> = node_ids.iter()
        .map(|&nid| (nid, graph.degree(nid) as f32))
        .collect();

    // Weighted adjacency
    let mut adj_weights: HashMap<NodeId, Vec<(NodeId, f32)>> = HashMap::new();
    for &nid in &node_ids {
        let mut ws = Vec::new();
        for (nbr, eid) in graph.neighbor_edges(nid) {
            ws.push((nbr, graph.edge_weight(eid)));
        }
        adj_weights.insert(nid, ws);
    }

    // Sum of weights in each community
    let mut sigma_tot: HashMap<usize, f32> = HashMap::new();
    for &nid in &node_ids {
        let c = comm_of[&nid];
        *sigma_tot.entry(c).or_insert(0.0) += degrees[&nid];
    }

    let mut iterations = 0;
    let max_iterations = 100;

    loop {
        iterations += 1;
        let mut improved = false;

        for &nid in &node_ids {
            let current_comm = comm_of[&nid];
            let ki = degrees[&nid];

            // Compute weights to each neighboring community
            let mut comm_weights: HashMap<usize, f32> = HashMap::new();
            for &(nbr, w) in adj_weights.get(&nid).unwrap_or(&Vec::new()) {
                let nc = comm_of[&nbr];
                *comm_weights.entry(nc).or_insert(0.0) += w;
            }

            // Remove node from current community
            *sigma_tot.get_mut(&current_comm).unwrap() -= ki;

            // Find best community
            let ki_in_current = comm_weights.get(&current_comm).copied().unwrap_or(0.0);
            let mut best_comm = current_comm;
            let mut best_gain = 0.0f32;

            for (&c, &ki_in) in &comm_weights {
                let st = sigma_tot.get(&c).copied().unwrap_or(0.0);
                let gain = ki_in / m2 - st * ki / (m2 * m2);
                let loss = ki_in_current / m2 - sigma_tot.get(&current_comm).copied().unwrap_or(0.0) * ki / (m2 * m2);
                let delta_q = gain - loss;
                if delta_q > best_gain {
                    best_gain = delta_q;
                    best_comm = c;
                }
            }

            // Move node to best community
            comm_of.insert(nid, best_comm);
            *sigma_tot.get_mut(&best_comm).unwrap_or(&mut 0.0) += ki;
            if !sigma_tot.contains_key(&best_comm) {
                sigma_tot.insert(best_comm, ki);
            }

            if best_comm != current_comm {
                improved = true;
            }
        }

        if !improved || iterations >= max_iterations {
            break;
        }
    }

    // Build communities from assignments
    let mut comm_map: HashMap<usize, Vec<NodeId>> = HashMap::new();
    for (&nid, &c) in &comm_of {
        comm_map.entry(c).or_default().push(nid);
    }

    let communities: Vec<Community> = comm_map.into_values()
        .map(|members| Community::from_members(members))
        .collect();

    let mod_val = modularity(graph, &communities);

    CommunityResult {
        communities,
        modularity: mod_val,
        iterations,
    }
}

/// Label propagation community detection.
/// Each node adopts the label most common among its neighbors.
pub fn label_propagation<N, E>(graph: &Graph<N, E>) -> CommunityResult {
    let node_ids = graph.node_ids();
    let n = node_ids.len();
    if n == 0 {
        return CommunityResult { communities: Vec::new(), modularity: 0.0, iterations: 0 };
    }

    // Initialize each node with its own label
    let mut labels: HashMap<NodeId, u32> = HashMap::new();
    for (i, &nid) in node_ids.iter().enumerate() {
        labels.insert(nid, i as u32);
    }

    let max_iterations = 100;
    let mut iterations = 0;

    // Simple deterministic ordering (could shuffle for randomness)
    loop {
        iterations += 1;
        let mut changed = false;

        for &nid in &node_ids {
            let neighbors = graph.neighbors(nid);
            if neighbors.is_empty() { continue; }

            // Count label frequencies among neighbors
            let mut freq: HashMap<u32, usize> = HashMap::new();
            for nbr in &neighbors {
                let lbl = labels[nbr];
                *freq.entry(lbl).or_insert(0) += 1;
            }

            // Pick most frequent label (ties broken by smallest label)
            let max_count = freq.values().copied().max().unwrap_or(0);
            let best_label = freq.iter()
                .filter(|(_, &c)| c == max_count)
                .map(|(&l, _)| l)
                .min()
                .unwrap_or(labels[&nid]);

            if labels[&nid] != best_label {
                labels.insert(nid, best_label);
                changed = true;
            }
        }

        if !changed || iterations >= max_iterations {
            break;
        }
    }

    // Build communities from labels
    let mut comm_map: HashMap<u32, Vec<NodeId>> = HashMap::new();
    for (&nid, &lbl) in &labels {
        comm_map.entry(lbl).or_default().push(nid);
    }

    let communities: Vec<Community> = comm_map.into_values()
        .map(|members| Community::from_members(members))
        .collect();

    let mod_val = modularity(graph, &communities);

    CommunityResult {
        communities,
        modularity: mod_val,
        iterations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::graph_core::GraphKind;

    fn make_two_cliques() -> Graph<(), ()> {
        let mut g = Graph::new(GraphKind::Undirected);
        // Clique 1: 0,1,2
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.add_edge(a, c, ());
        // Clique 2: 3,4,5
        let d = g.add_node(());
        let e = g.add_node(());
        let f = g.add_node(());
        g.add_edge(d, e, ());
        g.add_edge(e, f, ());
        g.add_edge(d, f, ());
        // Bridge
        g.add_edge(c, d, ());
        g
    }

    #[test]
    fn test_modularity_single_community() {
        let mut g = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.add_edge(a, c, ());
        let comms = vec![Community::from_members(vec![a, b, c])];
        let q = modularity(&g, &comms);
        // All in one community: modularity should be 0
        assert!((q - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_louvain_two_cliques() {
        let g = make_two_cliques();
        let result = louvain(&g);
        // Should find 2 communities
        assert!(result.communities.len() >= 2);
        assert!(result.modularity >= 0.0);
    }

    #[test]
    fn test_label_propagation_two_cliques() {
        let g = make_two_cliques();
        let result = label_propagation(&g);
        assert!(result.communities.len() >= 1);
    }

    #[test]
    fn test_louvain_empty() {
        let g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let result = louvain(&g);
        assert!(result.communities.is_empty());
    }

    #[test]
    fn test_label_propagation_disconnected() {
        let mut g = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        // No edges => each node is its own community
        let result = label_propagation(&g);
        assert_eq!(result.communities.len(), 3);
    }

    #[test]
    fn test_community_struct() {
        let c = Community::from_members(vec![NodeId(0), NodeId(1), NodeId(2)]);
        assert_eq!(c.len(), 3);
        assert!(c.contains(NodeId(1)));
        assert!(!c.contains(NodeId(5)));
        assert!(!c.is_empty());
    }

    #[test]
    fn test_louvain_single_node() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        g.add_node(());
        let result = louvain(&g);
        assert_eq!(result.communities.len(), 1);
    }

    #[test]
    fn test_modularity_two_perfect_communities() {
        let mut g = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        let d = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(c, d, ());
        let comms = vec![
            Community::from_members(vec![a, b]),
            Community::from_members(vec![c, d]),
        ];
        let q = modularity(&g, &comms);
        assert!(q > 0.0, "Modularity should be positive for good partition, got {}", q);
    }
}
