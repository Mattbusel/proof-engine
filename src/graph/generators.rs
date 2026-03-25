use super::graph_core::{Graph, GraphKind, NodeId};
use glam::Vec2;

fn pseudo_random(seed: u64, i: u64) -> f64 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(i.wrapping_mul(1442695040888963407));
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x as f64) / (u64::MAX as f64)
}

/// Watts-Strogatz small-world graph.
/// Start with a ring lattice where each node connects to k nearest neighbors,
/// then rewire each edge with probability beta.
pub fn watts_strogatz(n: usize, k: usize, beta: f64) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    if n == 0 { return g; }
    let nodes: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();

    // Create ring lattice: each node connected to k/2 neighbors on each side
    let half_k = k / 2;
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        for j in 1..=half_k {
            let target = (i + j) % n;
            edges.push((i, target));
        }
    }

    // Add edges, potentially rewired
    let mut seed_counter: u64 = 42;
    for (i, target) in edges {
        let r = pseudo_random(seed_counter, seed_counter);
        seed_counter += 1;
        if r < beta {
            // Rewire: pick random target that isn't self or existing neighbor
            let mut new_target = i;
            for attempt in 0..100 {
                let candidate = (pseudo_random(seed_counter, attempt as u64) * n as f64) as usize % n;
                seed_counter += 1;
                if candidate != i && g.find_edge(nodes[i], nodes[candidate]).is_none() {
                    new_target = candidate;
                    break;
                }
            }
            if new_target != i {
                g.add_edge(nodes[i], nodes[new_target], ());
            }
        } else {
            if g.find_edge(nodes[i], nodes[target]).is_none() {
                g.add_edge(nodes[i], nodes[target], ());
            }
        }
    }
    g
}

/// Barabasi-Albert preferential attachment graph.
/// Start with m+1 fully connected nodes, then add n-(m+1) nodes,
/// each connecting to m existing nodes with probability proportional to degree.
pub fn barabasi_albert(n: usize, m: usize) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    if n == 0 || m == 0 { return g; }

    let initial = (m + 1).min(n);
    let nodes: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();

    // Fully connect initial nodes
    for i in 0..initial {
        for j in (i + 1)..initial {
            g.add_edge(nodes[i], nodes[j], ());
        }
    }

    let mut seed_counter: u64 = 1337;
    // Degree list for preferential attachment (repeated entries = higher probability)
    let mut degree_list: Vec<usize> = Vec::new();
    for i in 0..initial {
        for _ in 0..(initial - 1) {
            degree_list.push(i);
        }
    }

    for i in initial..n {
        let mut targets: Vec<usize> = Vec::new();
        let mut connected = std::collections::HashSet::new();
        for _ in 0..m {
            if degree_list.is_empty() { break; }
            for attempt in 0..100 {
                let idx = (pseudo_random(seed_counter, attempt as u64) * degree_list.len() as f64) as usize % degree_list.len();
                seed_counter += 1;
                let target = degree_list[idx];
                if target != i && !connected.contains(&target) {
                    connected.insert(target);
                    targets.push(target);
                    break;
                }
            }
        }
        for &t in &targets {
            g.add_edge(nodes[i], nodes[t], ());
            degree_list.push(i);
            degree_list.push(t);
        }
    }
    g
}

/// Erdos-Renyi random graph G(n, p).
/// Each possible edge exists independently with probability p.
pub fn erdos_renyi(n: usize, p: f64) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    let nodes: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();
    let mut seed_counter: u64 = 7919;
    for i in 0..n {
        for j in (i + 1)..n {
            let r = pseudo_random(seed_counter, (i * n + j) as u64);
            seed_counter += 1;
            if r < p {
                g.add_edge(nodes[i], nodes[j], ());
            }
        }
    }
    g
}

/// Complete graph K_n: every pair of nodes connected.
pub fn complete_graph(n: usize) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    let nodes: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();
    for i in 0..n {
        for j in (i + 1)..n {
            g.add_edge(nodes[i], nodes[j], ());
        }
    }
    g
}

/// Cycle graph C_n.
pub fn cycle_graph(n: usize) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    if n == 0 { return g; }
    let nodes: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();
    for i in 0..n {
        g.add_edge(nodes[i], nodes[(i + 1) % n], ());
    }
    g
}

/// Path graph P_n.
pub fn path_graph(n: usize) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    if n == 0 { return g; }
    let nodes: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();
    for i in 0..(n - 1) {
        g.add_edge(nodes[i], nodes[i + 1], ());
    }
    g
}

/// Star graph S_n: one center connected to n-1 leaves.
pub fn star_graph(n: usize) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    if n == 0 { return g; }
    let nodes: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();
    for i in 1..n {
        g.add_edge(nodes[0], nodes[i], ());
    }
    g
}

/// Grid graph with given rows and cols.
pub fn grid_graph(rows: usize, cols: usize) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    if rows == 0 || cols == 0 { return g; }
    let mut nodes = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            let pos = Vec2::new(c as f32 * 50.0, r as f32 * 50.0);
            nodes.push(g.add_node_with_pos((), pos));
        }
    }
    for r in 0..rows {
        for c in 0..cols {
            let idx = r * cols + c;
            if c + 1 < cols {
                g.add_edge(nodes[idx], nodes[idx + 1], ());
            }
            if r + 1 < rows {
                g.add_edge(nodes[idx], nodes[idx + cols], ());
            }
        }
    }
    g
}

/// Complete binary tree of given depth (0 = just root).
pub fn binary_tree(depth: usize) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    let n = (1 << (depth + 1)) - 1; // 2^(depth+1) - 1 nodes
    let nodes: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();
    for i in 0..n {
        let left = 2 * i + 1;
        let right = 2 * i + 2;
        if left < n {
            g.add_edge(nodes[i], nodes[left], ());
        }
        if right < n {
            g.add_edge(nodes[i], nodes[right], ());
        }
    }
    g
}

/// Petersen graph: 10 nodes, 15 edges.
pub fn petersen_graph() -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    let nodes: Vec<NodeId> = (0..10).map(|_| g.add_node(())).collect();
    // Outer cycle: 0-1-2-3-4-0
    for i in 0..5 {
        g.add_edge(nodes[i], nodes[(i + 1) % 5], ());
    }
    // Inner pentagram: 5-7-9-6-8-5
    g.add_edge(nodes[5], nodes[7], ());
    g.add_edge(nodes[7], nodes[9], ());
    g.add_edge(nodes[9], nodes[6], ());
    g.add_edge(nodes[6], nodes[8], ());
    g.add_edge(nodes[8], nodes[5], ());
    // Spokes: i <-> i+5
    for i in 0..5 {
        g.add_edge(nodes[i], nodes[i + 5], ());
    }
    g
}

/// Complete bipartite graph K_{m,n}.
pub fn complete_bipartite(m: usize, n: usize) -> Graph<(), ()> {
    let mut g = Graph::new(GraphKind::Undirected);
    let left: Vec<NodeId> = (0..m).map(|_| g.add_node(())).collect();
    let right: Vec<NodeId> = (0..n).map(|_| g.add_node(())).collect();
    for &l in &left {
        for &r in &right {
            g.add_edge(l, r, ());
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_graph_edges() {
        let g = complete_graph(5);
        assert_eq!(g.node_count(), 5);
        assert_eq!(g.edge_count(), 10); // 5*4/2
    }

    #[test]
    fn test_complete_graph_1() {
        let g = complete_graph(1);
        assert_eq!(g.node_count(), 1);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn test_cycle_graph() {
        let g = cycle_graph(6);
        assert_eq!(g.node_count(), 6);
        assert_eq!(g.edge_count(), 6);
        // Every node has degree 2
        for nid in g.node_ids() {
            assert_eq!(g.degree(nid), 2);
        }
    }

    #[test]
    fn test_path_graph() {
        let g = path_graph(5);
        assert_eq!(g.node_count(), 5);
        assert_eq!(g.edge_count(), 4);
        let ids = g.node_ids();
        assert_eq!(g.degree(ids[0]), 1);
        assert_eq!(g.degree(ids[2]), 2);
        assert_eq!(g.degree(ids[4]), 1);
    }

    #[test]
    fn test_star_graph() {
        let g = star_graph(6);
        assert_eq!(g.node_count(), 6);
        assert_eq!(g.edge_count(), 5);
        let ids = g.node_ids();
        assert_eq!(g.degree(ids[0]), 5); // center
    }

    #[test]
    fn test_grid_graph() {
        let g = grid_graph(3, 4);
        assert_eq!(g.node_count(), 12);
        // horizontal: 3*3 = 9, vertical: 2*4 = 8 => 17
        assert_eq!(g.edge_count(), 17);
    }

    #[test]
    fn test_binary_tree() {
        let g = binary_tree(3);
        // 2^4 - 1 = 15 nodes, 14 edges
        assert_eq!(g.node_count(), 15);
        assert_eq!(g.edge_count(), 14);
    }

    #[test]
    fn test_petersen_graph() {
        let g = petersen_graph();
        assert_eq!(g.node_count(), 10);
        assert_eq!(g.edge_count(), 15);
        // Every node in Petersen graph has degree 3
        for nid in g.node_ids() {
            assert_eq!(g.degree(nid), 3);
        }
    }

    #[test]
    fn test_complete_bipartite() {
        let g = complete_bipartite(3, 4);
        assert_eq!(g.node_count(), 7);
        assert_eq!(g.edge_count(), 12); // 3*4
    }

    #[test]
    fn test_erdos_renyi_bounds() {
        let g = erdos_renyi(20, 0.5);
        assert_eq!(g.node_count(), 20);
        // With p=0.5, expect roughly n*(n-1)/4 = 95 edges, but it's random
        assert!(g.edge_count() > 0);
    }

    #[test]
    fn test_watts_strogatz() {
        let g = watts_strogatz(20, 4, 0.0);
        assert_eq!(g.node_count(), 20);
        // With beta=0, should be a ring lattice with k/2*n = 2*20 = 40 edges
        assert!(g.edge_count() > 0);
    }

    #[test]
    fn test_barabasi_albert() {
        let g = barabasi_albert(20, 2);
        assert_eq!(g.node_count(), 20);
        assert!(g.edge_count() > 0);
    }

    #[test]
    fn test_empty_generators() {
        assert_eq!(complete_graph(0).node_count(), 0);
        assert_eq!(cycle_graph(0).node_count(), 0);
        assert_eq!(path_graph(0).node_count(), 0);
        assert_eq!(star_graph(0).node_count(), 0);
        assert_eq!(grid_graph(0, 5).node_count(), 0);
    }
}
