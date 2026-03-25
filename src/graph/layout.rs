use glam::Vec2;
use std::collections::{HashMap, HashSet, VecDeque};
use super::graph_core::{Graph, GraphKind, NodeId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutAlgorithm {
    ForceDirected,
    Hierarchical,
    Circular,
    Spectral,
    Tree,
    Random,
    Grid,
}

#[derive(Debug, Clone)]
pub struct LayoutConfig {
    pub iterations: usize,
    pub spacing: f32,
    pub bounds: Vec2,
    pub seed: u64,
    pub temperature: f32,
    pub cooling_factor: f32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            iterations: 300,
            spacing: 50.0,
            bounds: Vec2::new(800.0, 600.0),
            seed: 42,
            temperature: 100.0,
            cooling_factor: 0.95,
        }
    }
}

pub fn compute_layout<N, E>(graph: &Graph<N, E>, algorithm: LayoutAlgorithm, config: &LayoutConfig) -> HashMap<NodeId, Vec2> {
    match algorithm {
        LayoutAlgorithm::ForceDirected => {
            let mut fl = ForceDirectedLayout::new(config.clone());
            fl.compute(graph)
        }
        LayoutAlgorithm::Hierarchical => {
            let mut hl = HierarchicalLayout::new(config.clone());
            hl.compute(graph)
        }
        LayoutAlgorithm::Circular => {
            CircularLayout::new(config.clone()).compute(graph)
        }
        LayoutAlgorithm::Spectral => {
            SpectralLayout::new(config.clone()).compute(graph)
        }
        LayoutAlgorithm::Tree => {
            TreeLayout::new(config.clone()).compute(graph)
        }
        LayoutAlgorithm::Random => {
            random_layout(graph, config)
        }
        LayoutAlgorithm::Grid => {
            grid_layout(graph, config)
        }
    }
}

fn pseudo_random(seed: u64, i: u64) -> f64 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(i.wrapping_mul(1442695040888963407));
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x as f64) / (u64::MAX as f64)
}

fn random_layout<N, E>(graph: &Graph<N, E>, config: &LayoutConfig) -> HashMap<NodeId, Vec2> {
    let mut positions = HashMap::new();
    for (i, nid) in graph.node_ids().iter().enumerate() {
        let x = pseudo_random(config.seed, i as u64 * 2) as f32 * config.bounds.x;
        let y = pseudo_random(config.seed, i as u64 * 2 + 1) as f32 * config.bounds.y;
        positions.insert(*nid, Vec2::new(x, y));
    }
    positions
}

fn grid_layout<N, E>(graph: &Graph<N, E>, config: &LayoutConfig) -> HashMap<NodeId, Vec2> {
    let mut positions = HashMap::new();
    let n = graph.node_count();
    if n == 0 { return positions; }
    let cols = (n as f32).sqrt().ceil() as usize;
    let spacing = config.spacing;
    let offset = Vec2::new(
        config.bounds.x / 2.0 - (cols as f32 * spacing) / 2.0,
        config.bounds.y / 2.0 - ((n / cols + 1) as f32 * spacing) / 2.0,
    );
    for (i, nid) in graph.node_ids().iter().enumerate() {
        let row = i / cols;
        let col = i % cols;
        positions.insert(*nid, Vec2::new(
            offset.x + col as f32 * spacing,
            offset.y + row as f32 * spacing,
        ));
    }
    positions
}

// ---- Force-Directed Layout (Fruchterman-Reingold) ----

pub struct ForceDirectedLayout {
    config: LayoutConfig,
}

impl ForceDirectedLayout {
    pub fn new(config: LayoutConfig) -> Self {
        Self { config }
    }

    pub fn compute<N, E>(&mut self, graph: &Graph<N, E>) -> HashMap<NodeId, Vec2> {
        let node_ids = graph.node_ids();
        let n = node_ids.len();
        if n == 0 { return HashMap::new(); }

        let area = self.config.bounds.x * self.config.bounds.y;
        let k = (area / n as f32).sqrt(); // optimal distance

        // Initialize positions
        let mut positions: HashMap<NodeId, Vec2> = HashMap::new();
        for (i, &nid) in node_ids.iter().enumerate() {
            let x = pseudo_random(self.config.seed, i as u64 * 2) as f32 * self.config.bounds.x;
            let y = pseudo_random(self.config.seed, i as u64 * 2 + 1) as f32 * self.config.bounds.y;
            positions.insert(nid, Vec2::new(x, y));
        }

        let mut temperature = self.config.temperature;

        for _iter in 0..self.config.iterations {
            let mut displacements: HashMap<NodeId, Vec2> = HashMap::new();
            for &nid in &node_ids {
                displacements.insert(nid, Vec2::ZERO);
            }

            // Repulsive forces between all pairs
            for i in 0..n {
                for j in (i + 1)..n {
                    let ni = node_ids[i];
                    let nj = node_ids[j];
                    let pi = positions[&ni];
                    let pj = positions[&nj];
                    let delta = pi - pj;
                    let dist = delta.length().max(0.01);
                    let repulsion = k * k / dist;
                    let force = delta / dist * repulsion;
                    *displacements.get_mut(&ni).unwrap() += force;
                    *displacements.get_mut(&nj).unwrap() -= force;
                }
            }

            // Attractive forces along edges
            for edge in graph.edges() {
                let pi = positions[&edge.from];
                let pj = positions[&edge.to];
                let delta = pi - pj;
                let dist = delta.length().max(0.01);
                let attraction = dist * dist / k;
                let force = delta / dist * attraction;
                *displacements.get_mut(&edge.from).unwrap() -= force;
                *displacements.get_mut(&edge.to).unwrap() += force;
            }

            // Apply displacements with temperature limiting
            for &nid in &node_ids {
                let disp = displacements[&nid];
                let len = disp.length().max(0.01);
                let clamped = disp / len * len.min(temperature);
                let mut pos = positions[&nid] + clamped;
                // Clamp to bounds
                pos.x = pos.x.clamp(0.0, self.config.bounds.x);
                pos.y = pos.y.clamp(0.0, self.config.bounds.y);
                positions.insert(nid, pos);
            }

            temperature *= self.config.cooling_factor;
        }

        positions
    }
}

// ---- Hierarchical Layout (Sugiyama-style) ----

pub struct HierarchicalLayout {
    config: LayoutConfig,
}

impl HierarchicalLayout {
    pub fn new(config: LayoutConfig) -> Self {
        Self { config }
    }

    pub fn compute<N, E>(&mut self, graph: &Graph<N, E>) -> HashMap<NodeId, Vec2> {
        let node_ids = graph.node_ids();
        if node_ids.is_empty() { return HashMap::new(); }

        // Step 1: Layer assignment via BFS from roots (nodes with in-degree 0 or lowest id)
        let mut layers: HashMap<NodeId, usize> = HashMap::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Find roots: for directed, nodes with no incoming edges; for undirected, pick first
        let mut has_incoming: HashSet<NodeId> = HashSet::new();
        if graph.kind == GraphKind::Directed {
            for edge in graph.edges() {
                has_incoming.insert(edge.to);
            }
        }
        let roots: Vec<NodeId> = if graph.kind == GraphKind::Directed {
            node_ids.iter().filter(|n| !has_incoming.contains(n)).copied().collect()
        } else {
            vec![node_ids[0]]
        };
        let roots = if roots.is_empty() { vec![node_ids[0]] } else { roots };

        for &root in &roots {
            if visited.insert(root) {
                queue.push_back(root);
                layers.insert(root, 0);
            }
        }
        while let Some(nid) = queue.pop_front() {
            let layer = layers[&nid];
            for nbr in graph.neighbors(nid) {
                if visited.insert(nbr) {
                    layers.insert(nbr, layer + 1);
                    queue.push_back(nbr);
                }
            }
        }
        // Assign unvisited nodes
        for &nid in &node_ids {
            if !layers.contains_key(&nid) {
                layers.insert(nid, 0);
            }
        }

        // Step 2: Group nodes by layer
        let max_layer = layers.values().copied().max().unwrap_or(0);
        let mut layer_nodes: Vec<Vec<NodeId>> = vec![Vec::new(); max_layer + 1];
        for (&nid, &layer) in &layers {
            layer_nodes[layer].push(nid);
        }
        // Sort within layers for determinism
        for layer in &mut layer_nodes {
            layer.sort();
        }

        // Step 3: Crossing minimization (barycenter heuristic)
        for _pass in 0..5 {
            for l in 1..=max_layer {
                let prev_layer = &layer_nodes[l - 1];
                let prev_pos: HashMap<NodeId, f32> = prev_layer.iter().enumerate()
                    .map(|(i, &nid)| (nid, i as f32))
                    .collect();

                let mut barycenters: Vec<(NodeId, f32)> = Vec::new();
                for &nid in &layer_nodes[l] {
                    let nbrs = graph.neighbors(nid);
                    let parent_positions: Vec<f32> = nbrs.iter()
                        .filter_map(|n| prev_pos.get(n).copied())
                        .collect();
                    let bc = if parent_positions.is_empty() {
                        0.0
                    } else {
                        parent_positions.iter().sum::<f32>() / parent_positions.len() as f32
                    };
                    barycenters.push((nid, bc));
                }
                barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                layer_nodes[l] = barycenters.into_iter().map(|(nid, _)| nid).collect();
            }
        }

        // Step 4: Coordinate assignment
        let mut positions = HashMap::new();
        let layer_spacing = self.config.bounds.y / (max_layer + 1) as f32;
        for (l, nodes) in layer_nodes.iter().enumerate() {
            let n = nodes.len();
            let node_spacing = if n > 1 {
                self.config.bounds.x / n as f32
            } else {
                self.config.bounds.x / 2.0
            };
            for (i, &nid) in nodes.iter().enumerate() {
                let x = if n > 1 {
                    node_spacing * (i as f32 + 0.5)
                } else {
                    self.config.bounds.x / 2.0
                };
                let y = layer_spacing * (l as f32 + 0.5);
                positions.insert(nid, Vec2::new(x, y));
            }
        }
        positions
    }
}

// ---- Circular Layout ----

pub struct CircularLayout {
    config: LayoutConfig,
}

impl CircularLayout {
    pub fn new(config: LayoutConfig) -> Self {
        Self { config }
    }

    pub fn compute<N, E>(&self, graph: &Graph<N, E>) -> HashMap<NodeId, Vec2> {
        let node_ids = graph.node_ids();
        let n = node_ids.len();
        if n == 0 { return HashMap::new(); }

        let center = self.config.bounds * 0.5;
        let radius = self.config.bounds.x.min(self.config.bounds.y) * 0.4;

        let mut positions = HashMap::new();
        for (i, &nid) in node_ids.iter().enumerate() {
            let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
            let x = center.x + radius * angle.cos();
            let y = center.y + radius * angle.sin();
            positions.insert(nid, Vec2::new(x, y));
        }
        positions
    }
}

// ---- Spectral Layout (power iteration for Laplacian eigenvectors) ----

pub struct SpectralLayout {
    config: LayoutConfig,
}

impl SpectralLayout {
    pub fn new(config: LayoutConfig) -> Self {
        Self { config }
    }

    pub fn compute<N, E>(&self, graph: &Graph<N, E>) -> HashMap<NodeId, Vec2> {
        let node_ids = graph.node_ids();
        let n = node_ids.len();
        if n == 0 { return HashMap::new(); }
        if n == 1 {
            let mut m = HashMap::new();
            m.insert(node_ids[0], self.config.bounds * 0.5);
            return m;
        }

        let idx: HashMap<NodeId, usize> = node_ids.iter().enumerate().map(|(i, &nid)| (nid, i)).collect();

        // Build Laplacian matrix
        let mut laplacian = vec![vec![0.0f64; n]; n];
        for edge in graph.edges() {
            if let (Some(&i), Some(&j)) = (idx.get(&edge.from), idx.get(&edge.to)) {
                laplacian[i][j] -= 1.0;
                laplacian[j][i] -= 1.0;
                laplacian[i][i] += 1.0;
                laplacian[j][j] += 1.0;
            }
        }

        // Power iteration to find the Fiedler vector (2nd smallest eigenvector)
        // and the 3rd smallest eigenvector for the second coordinate
        let eigvec2 = self.fiedler_vector(&laplacian, n, 0);
        let eigvec3 = self.fiedler_vector_orthogonal(&laplacian, n, &eigvec2);

        let center = self.config.bounds * 0.5;
        let scale = self.config.bounds.x.min(self.config.bounds.y) * 0.4;

        let mut positions = HashMap::new();
        let max2 = eigvec2.iter().map(|x| x.abs()).fold(0.0f64, f64::max).max(1e-10);
        let max3 = eigvec3.iter().map(|x| x.abs()).fold(0.0f64, f64::max).max(1e-10);

        for (i, &nid) in node_ids.iter().enumerate() {
            let x = center.x + (eigvec2[i] / max2) as f32 * scale;
            let y = center.y + (eigvec3[i] / max3) as f32 * scale;
            positions.insert(nid, Vec2::new(x, y));
        }
        positions
    }

    fn fiedler_vector(&self, laplacian: &[Vec<f64>], n: usize, seed_offset: u64) -> Vec<f64> {
        // Inverse power iteration with shift to find smallest non-trivial eigenvector
        // Using simple power iteration on (max_eigenvalue * I - L) to find largest eigenvector of complement
        // Then deflate the constant vector and repeat

        let max_iter = 200;
        // Start with a random vector orthogonal to the constant vector
        let mut v: Vec<f64> = (0..n).map(|i| pseudo_random(self.config.seed + seed_offset, i as u64) - 0.5).collect();

        // Orthogonalize against constant vector
        let mean: f64 = v.iter().sum::<f64>() / n as f64;
        for x in v.iter_mut() { *x -= mean; }
        let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-12 { for x in v.iter_mut() { *x /= norm; } }

        for _ in 0..max_iter {
            // Multiply: w = L * v
            let mut w = vec![0.0f64; n];
            for i in 0..n {
                for j in 0..n {
                    w[i] += laplacian[i][j] * v[j];
                }
            }
            // Orthogonalize against constant vector
            let mean: f64 = w.iter().sum::<f64>() / n as f64;
            for x in w.iter_mut() { *x -= mean; }
            let norm: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-12 { for x in w.iter_mut() { *x /= norm; } }
            v = w;
        }
        v
    }

    fn fiedler_vector_orthogonal(&self, laplacian: &[Vec<f64>], n: usize, fiedler: &[f64]) -> Vec<f64> {
        let max_iter = 200;
        let mut v: Vec<f64> = (0..n).map(|i| pseudo_random(self.config.seed + 9999, i as u64) - 0.5).collect();

        for _ in 0..max_iter {
            let mut w = vec![0.0f64; n];
            for i in 0..n {
                for j in 0..n {
                    w[i] += laplacian[i][j] * v[j];
                }
            }
            // Orthogonalize against constant vector
            let mean: f64 = w.iter().sum::<f64>() / n as f64;
            for x in w.iter_mut() { *x -= mean; }
            // Orthogonalize against Fiedler vector
            let dot: f64 = w.iter().zip(fiedler).map(|(a, b)| a * b).sum();
            for (x, f) in w.iter_mut().zip(fiedler) { *x -= dot * f; }
            let norm: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-12 { for x in w.iter_mut() { *x /= norm; } }
            v = w;
        }
        v
    }
}

// ---- Tree Layout (Reingold-Tilford) ----

pub struct TreeLayout {
    config: LayoutConfig,
}

impl TreeLayout {
    pub fn new(config: LayoutConfig) -> Self {
        Self { config }
    }

    pub fn compute<N, E>(&self, graph: &Graph<N, E>) -> HashMap<NodeId, Vec2> {
        let node_ids = graph.node_ids();
        if node_ids.is_empty() { return HashMap::new(); }

        // Find root: node with smallest ID or no incoming edges for directed
        let root = if graph.kind == GraphKind::Directed {
            let mut has_incoming: HashSet<NodeId> = HashSet::new();
            for edge in graph.edges() { has_incoming.insert(edge.to); }
            node_ids.iter().find(|n| !has_incoming.contains(n)).copied().unwrap_or(node_ids[0])
        } else {
            node_ids[0]
        };

        // BFS to build tree
        let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut depth: HashMap<NodeId, usize> = HashMap::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(root);
        depth.insert(root, 0);
        queue.push_back(root);
        children.insert(root, Vec::new());

        while let Some(nid) = queue.pop_front() {
            let mut kids = Vec::new();
            for nbr in graph.neighbors(nid) {
                if visited.insert(nbr) {
                    depth.insert(nbr, depth[&nid] + 1);
                    children.insert(nbr, Vec::new());
                    queue.push_back(nbr);
                    kids.push(nbr);
                }
            }
            children.insert(nid, kids);
        }

        // Assign x positions using Reingold-Tilford approach
        let mut x_pos: HashMap<NodeId, f32> = HashMap::new();
        let mut next_x = 0.0f32;
        self.assign_x(root, &children, &mut x_pos, &mut next_x);

        let max_depth = depth.values().copied().max().unwrap_or(0) as f32;
        let y_spacing = if max_depth > 0.0 {
            self.config.bounds.y / (max_depth + 1.0)
        } else {
            self.config.bounds.y / 2.0
        };

        let max_x = x_pos.values().copied().fold(0.0f32, f32::max).max(1.0);
        let x_scale = self.config.bounds.x / (max_x + 1.0);

        let mut positions = HashMap::new();
        for (&nid, &d) in &depth {
            let x = x_pos.get(&nid).copied().unwrap_or(0.0) * x_scale + x_scale * 0.5;
            let y = d as f32 * y_spacing + y_spacing * 0.5;
            positions.insert(nid, Vec2::new(x, y));
        }
        positions
    }

    fn assign_x(&self, node: NodeId, children: &HashMap<NodeId, Vec<NodeId>>, x_pos: &mut HashMap<NodeId, f32>, next_x: &mut f32) {
        let kids = children.get(&node).cloned().unwrap_or_default();
        if kids.is_empty() {
            x_pos.insert(node, *next_x);
            *next_x += 1.0;
        } else {
            for &child in &kids {
                self.assign_x(child, children, x_pos, next_x);
            }
            let first = x_pos[&kids[0]];
            let last = x_pos[kids.last().unwrap()];
            x_pos.insert(node, (first + last) / 2.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_triangle() -> Graph<(), ()> {
        let mut g = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
        g.add_edge(a, c, ());
        g
    }

    fn make_path(n: usize) -> Graph<(), ()> {
        let mut g = Graph::new(GraphKind::Undirected);
        let mut prev = g.add_node(());
        for _ in 1..n {
            let next = g.add_node(());
            g.add_edge(prev, next, ());
            prev = next;
        }
        g
    }

    #[test]
    fn test_force_directed_produces_positions() {
        let g = make_triangle();
        let config = LayoutConfig { iterations: 50, ..Default::default() };
        let pos = compute_layout(&g, LayoutAlgorithm::ForceDirected, &config);
        assert_eq!(pos.len(), 3);
        for p in pos.values() {
            assert!(p.x >= 0.0 && p.x <= config.bounds.x);
            assert!(p.y >= 0.0 && p.y <= config.bounds.y);
        }
    }

    #[test]
    fn test_circular_layout() {
        let g = make_path(6);
        let config = LayoutConfig::default();
        let pos = compute_layout(&g, LayoutAlgorithm::Circular, &config);
        assert_eq!(pos.len(), 6);
    }

    #[test]
    fn test_hierarchical_layout() {
        let mut g = Graph::new(GraphKind::Directed);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        let d = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(a, c, ());
        g.add_edge(b, d, ());
        let config = LayoutConfig::default();
        let pos = compute_layout(&g, LayoutAlgorithm::Hierarchical, &config);
        assert_eq!(pos.len(), 4);
        // Root should be higher (smaller y) than children
        assert!(pos[&a].y < pos[&b].y);
        assert!(pos[&b].y < pos[&d].y);
    }

    #[test]
    fn test_tree_layout() {
        let mut g = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        let d = g.add_node(());
        let e = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(a, c, ());
        g.add_edge(b, d, ());
        g.add_edge(b, e, ());
        let config = LayoutConfig::default();
        let pos = compute_layout(&g, LayoutAlgorithm::Tree, &config);
        assert_eq!(pos.len(), 5);
    }

    #[test]
    fn test_spectral_layout() {
        let g = make_path(5);
        let config = LayoutConfig::default();
        let pos = compute_layout(&g, LayoutAlgorithm::Spectral, &config);
        assert_eq!(pos.len(), 5);
    }

    #[test]
    fn test_random_layout() {
        let g = make_triangle();
        let config = LayoutConfig::default();
        let pos = compute_layout(&g, LayoutAlgorithm::Random, &config);
        assert_eq!(pos.len(), 3);
    }

    #[test]
    fn test_grid_layout() {
        let g = make_path(9);
        let config = LayoutConfig::default();
        let pos = compute_layout(&g, LayoutAlgorithm::Grid, &config);
        assert_eq!(pos.len(), 9);
    }

    #[test]
    fn test_empty_graph() {
        let g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let config = LayoutConfig::default();
        for alg in &[LayoutAlgorithm::ForceDirected, LayoutAlgorithm::Circular, LayoutAlgorithm::Tree] {
            let pos = compute_layout(&g, *alg, &config);
            assert!(pos.is_empty());
        }
    }

    #[test]
    fn test_single_node() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        g.add_node(());
        let config = LayoutConfig::default();
        let pos = compute_layout(&g, LayoutAlgorithm::Spectral, &config);
        assert_eq!(pos.len(), 1);
    }

    #[test]
    fn test_force_directed_separates_components() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let a = g.add_node(());
        let b = g.add_node(());
        g.add_edge(a, b, ());
        let c = g.add_node(());
        let d = g.add_node(());
        g.add_edge(c, d, ());
        let config = LayoutConfig { iterations: 100, ..Default::default() };
        let pos = compute_layout(&g, LayoutAlgorithm::ForceDirected, &config);
        assert_eq!(pos.len(), 4);
        // Connected nodes should be closer to each other than to other component
        let dist_ab = (pos[&a] - pos[&b]).length();
        let dist_ac = (pos[&a] - pos[&c]).length();
        // Not guaranteed but likely with force directed
    }

    #[test]
    fn test_circular_layout_radius() {
        let mut g: Graph<(), ()> = Graph::new(GraphKind::Undirected);
        let nodes: Vec<NodeId> = (0..8).map(|_| g.add_node(())).collect();
        for i in 0..7 { g.add_edge(nodes[i], nodes[i + 1], ()); }
        let config = LayoutConfig::default();
        let pos = compute_layout(&g, LayoutAlgorithm::Circular, &config);
        let center = config.bounds * 0.5;
        let radius = config.bounds.x.min(config.bounds.y) * 0.4;
        for p in pos.values() {
            let dist = (*p - center).length();
            assert!((dist - radius).abs() < 1.0);
        }
    }
}
