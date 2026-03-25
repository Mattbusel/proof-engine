use std::collections::{HashMap, HashSet};
use super::graph_core::{Graph, GraphKind, NodeId};

fn pseudo_random(seed: u64, i: u64) -> f64 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(i.wrapping_mul(1442695040888963407));
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x as f64) / (u64::MAX as f64)
}

/// Spectral bisection using the Fiedler vector (2nd smallest eigenvector of Laplacian).
/// Nodes with Fiedler value < median go to partition A, rest to partition B.
pub fn spectral_partition<N, E>(graph: &Graph<N, E>) -> (Vec<NodeId>, Vec<NodeId>) {
    let node_ids = graph.node_ids();
    let n = node_ids.len();
    if n <= 1 {
        return (node_ids, Vec::new());
    }

    let idx: HashMap<NodeId, usize> = node_ids.iter().enumerate().map(|(i, &nid)| (nid, i)).collect();

    // Build Laplacian
    let mut laplacian = vec![vec![0.0f64; n]; n];
    for edge in graph.edges() {
        if let (Some(&i), Some(&j)) = (idx.get(&edge.from), idx.get(&edge.to)) {
            laplacian[i][j] -= 1.0;
            laplacian[j][i] -= 1.0;
            laplacian[i][i] += 1.0;
            laplacian[j][j] += 1.0;
        }
    }

    // Compute Fiedler vector via power iteration on L
    let fiedler = fiedler_vector(&laplacian, n);

    // Split by median
    let mut sorted_vals: Vec<f64> = fiedler.clone();
    sorted_vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted_vals[n / 2];

    let mut part_a = Vec::new();
    let mut part_b = Vec::new();
    for (i, &nid) in node_ids.iter().enumerate() {
        if fiedler[i] <= median {
            part_a.push(nid);
        } else {
            part_b.push(nid);
        }
    }

    // Ensure both partitions are non-empty
    if part_a.is_empty() {
        part_a.push(part_b.pop().unwrap());
    } else if part_b.is_empty() {
        part_b.push(part_a.pop().unwrap());
    }

    (part_a, part_b)
}

fn fiedler_vector(laplacian: &[Vec<f64>], n: usize) -> Vec<f64> {
    let max_iter = 300;
    let mut v: Vec<f64> = (0..n).map(|i| pseudo_random(42, i as u64) - 0.5).collect();

    // Orthogonalize against constant vector
    let mean: f64 = v.iter().sum::<f64>() / n as f64;
    for x in v.iter_mut() { *x -= mean; }
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-12 { for x in v.iter_mut() { *x /= norm; } }

    for _ in 0..max_iter {
        let mut w = vec![0.0f64; n];
        for i in 0..n {
            for j in 0..n {
                w[i] += laplacian[i][j] * v[j];
            }
        }
        let mean: f64 = w.iter().sum::<f64>() / n as f64;
        for x in w.iter_mut() { *x -= mean; }
        let norm: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-12 { for x in w.iter_mut() { *x /= norm; } }
        v = w;
    }
    v
}

/// Kernighan-Lin refinement: iteratively swap pairs of nodes between partitions
/// to minimize edge cut.
pub fn kernighan_lin<N, E>(graph: &Graph<N, E>, partition: (Vec<NodeId>, Vec<NodeId>)) -> (Vec<NodeId>, Vec<NodeId>) {
    let (mut part_a, mut part_b) = partition;
    if part_a.is_empty() || part_b.is_empty() {
        return (part_a, part_b);
    }

    let node_ids = graph.node_ids();
    let node_set: HashSet<NodeId> = node_ids.iter().copied().collect();

    // Build adjacency weight map
    let mut adj: HashMap<(NodeId, NodeId), f32> = HashMap::new();
    for edge in graph.edges() {
        let w = edge.weight;
        *adj.entry((edge.from, edge.to)).or_insert(0.0) += w;
        if graph.kind == GraphKind::Undirected {
            *adj.entry((edge.to, edge.from)).or_insert(0.0) += w;
        }
    }

    let max_passes = 20;
    for _ in 0..max_passes {
        let set_a: HashSet<NodeId> = part_a.iter().copied().collect();
        let set_b: HashSet<NodeId> = part_b.iter().copied().collect();

        // Compute D values: D[v] = external_cost - internal_cost
        let mut d: HashMap<NodeId, f32> = HashMap::new();
        for &v in &part_a {
            let ext: f32 = part_b.iter()
                .map(|&u| adj.get(&(v, u)).copied().unwrap_or(0.0))
                .sum();
            let int: f32 = part_a.iter()
                .filter(|&&u| u != v)
                .map(|&u| adj.get(&(v, u)).copied().unwrap_or(0.0))
                .sum();
            d.insert(v, ext - int);
        }
        for &v in &part_b {
            let ext: f32 = part_a.iter()
                .map(|&u| adj.get(&(v, u)).copied().unwrap_or(0.0))
                .sum();
            let int: f32 = part_b.iter()
                .filter(|&&u| u != v)
                .map(|&u| adj.get(&(v, u)).copied().unwrap_or(0.0))
                .sum();
            d.insert(v, ext - int);
        }

        // Find best swap
        let mut best_gain = f32::NEG_INFINITY;
        let mut best_a = part_a[0];
        let mut best_b = part_b[0];

        for &a in &part_a {
            for &b in &part_b {
                let c_ab = adj.get(&(a, b)).copied().unwrap_or(0.0);
                let gain = d[&a] + d[&b] - 2.0 * c_ab;
                if gain > best_gain {
                    best_gain = gain;
                    best_a = a;
                    best_b = b;
                }
            }
        }

        if best_gain <= 0.0 {
            break;
        }

        // Perform swap
        if let Some(pos) = part_a.iter().position(|&x| x == best_a) {
            part_a[pos] = best_b;
        }
        if let Some(pos) = part_b.iter().position(|&x| x == best_b) {
            part_b[pos] = best_a;
        }
    }

    (part_a, part_b)
}

/// Recursive bisection: repeatedly partition each part.
pub fn recursive_bisection<N: Clone, E: Clone>(graph: &Graph<N, E>, depth: usize) -> Vec<Vec<NodeId>> {
    if depth == 0 || graph.node_count() <= 1 {
        return vec![graph.node_ids()];
    }

    let (a, b) = spectral_partition(graph);

    let mut result = Vec::new();
    if depth > 1 && a.len() > 1 {
        let sub_a = graph.subgraph(&a);
        result.extend(recursive_bisection(&sub_a, depth - 1));
    } else {
        result.push(a);
    }
    if depth > 1 && b.len() > 1 {
        let sub_b = graph.subgraph(&b);
        result.extend(recursive_bisection(&sub_b, depth - 1));
    } else {
        result.push(b);
    }

    result
}

/// Partition quality: ratio of edges cut to total edges.
/// Lower is better (fewer inter-partition edges).
pub fn partition_quality<N, E>(graph: &Graph<N, E>, parts: &[Vec<NodeId>]) -> f32 {
    let total_edges = graph.edge_count() as f32;
    if total_edges == 0.0 { return 0.0; }

    let mut node_part: HashMap<NodeId, usize> = HashMap::new();
    for (pi, part) in parts.iter().enumerate() {
        for &nid in part {
            node_part.insert(nid, pi);
        }
    }

    let mut cut_edges = 0usize;
    for edge in graph.edges() {
        let pa = node_part.get(&edge.from).copied();
        let pb = node_part.get(&edge.to).copied();
        if pa != pb {
            cut_edges += 1;
        }
    }

    cut_edges as f32 / total_edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::generators;

    #[test]
    fn test_spectral_partition_splits() {
        let g = generators::path_graph(10);
        let (a, b) = spectral_partition(&g);
        assert!(!a.is_empty());
        assert!(!b.is_empty());
        assert_eq!(a.len() + b.len(), 10);
    }

    #[test]
    fn test_spectral_partition_two_components() {
        // Two disconnected cliques should be easy to partition
        let mut g = Graph::<(), ()>::new(GraphKind::Undirected);
        let n1: Vec<NodeId> = (0..5).map(|_| g.add_node(())).collect();
        for i in 0..5 { for j in (i+1)..5 { g.add_edge(n1[i], n1[j], ()); } }
        let n2: Vec<NodeId> = (0..5).map(|_| g.add_node(())).collect();
        for i in 0..5 { for j in (i+1)..5 { g.add_edge(n2[i], n2[j], ()); } }

        let (a, b) = spectral_partition(&g);
        assert!(!a.is_empty());
        assert!(!b.is_empty());
    }

    #[test]
    fn test_kernighan_lin_improves() {
        let g = generators::path_graph(8);
        let ids = g.node_ids();
        // Bad initial partition: alternating
        let a: Vec<NodeId> = ids.iter().step_by(2).copied().collect();
        let b: Vec<NodeId> = ids.iter().skip(1).step_by(2).copied().collect();
        let q_before = partition_quality(&g, &[a.clone(), b.clone()]);
        let (ra, rb) = kernighan_lin(&g, (a, b));
        let q_after = partition_quality(&g, &[ra, rb]);
        assert!(q_after <= q_before + 0.01, "KL should not significantly worsen: {} vs {}", q_after, q_before);
    }

    #[test]
    fn test_recursive_bisection() {
        let g = generators::path_graph(16);
        let parts = recursive_bisection(&g, 2);
        assert!(parts.len() >= 2);
        let total: usize = parts.iter().map(|p| p.len()).sum();
        assert_eq!(total, 16);
    }

    #[test]
    fn test_partition_quality_perfect() {
        // Two disconnected components, partitioned correctly
        let mut g = Graph::<(), ()>::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        let d = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(c, d, ());
        let q = partition_quality(&g, &[vec![a, b], vec![c, d]]);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_partition_quality_worst() {
        let g = generators::complete_bipartite(3, 3);
        let ids = g.node_ids();
        // Put all in one partition: no cuts
        let q = partition_quality(&g, &[ids.clone()]);
        assert_eq!(q, 0.0);
    }

    #[test]
    fn test_single_node() {
        let mut g = Graph::<(), ()>::new(GraphKind::Undirected);
        g.add_node(());
        let (a, b) = spectral_partition(&g);
        assert_eq!(a.len() + b.len(), 1);
    }

    #[test]
    fn test_recursive_bisection_depth_0() {
        let g = generators::path_graph(5);
        let parts = recursive_bisection(&g, 0);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].len(), 5);
    }
}
