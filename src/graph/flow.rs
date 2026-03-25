use std::collections::{HashMap, HashSet, VecDeque};
use glam::{Vec2, Vec4};
use super::graph_core::{Graph, GraphKind, NodeId, EdgeId};

#[derive(Debug, Clone)]
pub struct FlowNetwork {
    pub graph: Graph<(), f32>,
    pub source: NodeId,
    pub sink: NodeId,
}

impl FlowNetwork {
    /// Create a flow network. Edge data stores capacity.
    pub fn new(source: NodeId, sink: NodeId, graph: Graph<(), f32>) -> Self {
        Self { graph, source, sink }
    }

    /// Build a flow network from scratch.
    pub fn builder() -> FlowNetworkBuilder {
        FlowNetworkBuilder {
            graph: Graph::new(GraphKind::Directed),
            source: None,
            sink: None,
        }
    }

    pub fn capacity(&self, edge: EdgeId) -> f32 {
        self.graph.get_edge(edge).map(|e| e.data).unwrap_or(0.0)
    }
}

pub struct FlowNetworkBuilder {
    graph: Graph<(), f32>,
    source: Option<NodeId>,
    sink: Option<NodeId>,
}

impl FlowNetworkBuilder {
    pub fn add_node(&mut self) -> NodeId {
        self.graph.add_node(())
    }

    pub fn set_source(&mut self, id: NodeId) {
        self.source = Some(id);
    }

    pub fn set_sink(&mut self, id: NodeId) {
        self.sink = Some(id);
    }

    pub fn add_capacity(&mut self, from: NodeId, to: NodeId, capacity: f32) -> EdgeId {
        self.graph.add_edge(from, to, capacity)
    }

    pub fn build(self) -> FlowNetwork {
        FlowNetwork {
            graph: self.graph,
            source: self.source.expect("source must be set"),
            sink: self.sink.expect("sink must be set"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlowResult {
    pub max_flow: f32,
    pub edge_flows: HashMap<EdgeId, f32>,
}

/// Ford-Fulkerson max flow using BFS (Edmonds-Karp).
pub fn ford_fulkerson(network: &FlowNetwork, source: NodeId, sink: NodeId) -> FlowResult {
    let node_ids = network.graph.node_ids();
    let edge_ids = network.graph.edge_ids();

    // Build residual capacity structure
    // For each edge, track forward capacity and flow
    let mut flow: HashMap<EdgeId, f32> = HashMap::new();
    for &eid in &edge_ids {
        flow.insert(eid, 0.0);
    }

    // Build adjacency with edge info for residual graph traversal
    // We need both forward and backward edges
    // residual_adj[node] = Vec<(neighbor, edge_id, is_forward)>
    let mut residual_adj: HashMap<NodeId, Vec<(NodeId, EdgeId, bool)>> = HashMap::new();
    for &nid in &node_ids {
        residual_adj.insert(nid, Vec::new());
    }
    for &eid in &edge_ids {
        if let Some(edge) = network.graph.get_edge(eid) {
            residual_adj.get_mut(&edge.from).unwrap().push((edge.to, eid, true));
            // Backward edge
            if !residual_adj.contains_key(&edge.to) {
                residual_adj.insert(edge.to, Vec::new());
            }
            residual_adj.get_mut(&edge.to).unwrap().push((edge.from, eid, false));
        }
    }

    let mut total_flow = 0.0f32;

    // BFS to find augmenting path
    loop {
        // BFS
        let mut visited: HashMap<NodeId, (NodeId, EdgeId, bool)> = HashMap::new();
        let mut queue = VecDeque::new();
        queue.push_back(source);
        let mut found_sink = false;

        while let Some(node) = queue.pop_front() {
            if node == sink {
                found_sink = true;
                break;
            }
            for &(nbr, eid, is_forward) in residual_adj.get(&node).unwrap_or(&Vec::new()) {
                if visited.contains_key(&nbr) || nbr == source {
                    continue;
                }
                let residual = if is_forward {
                    let cap = network.graph.get_edge(eid).map(|e| e.data).unwrap_or(0.0);
                    cap - flow.get(&eid).copied().unwrap_or(0.0)
                } else {
                    flow.get(&eid).copied().unwrap_or(0.0)
                };
                if residual > 0.0 {
                    visited.insert(nbr, (node, eid, is_forward));
                    queue.push_back(nbr);
                }
            }
        }

        if !found_sink { break; }

        // Find bottleneck
        let mut bottleneck = f32::INFINITY;
        let mut current = sink;
        while current != source {
            let (prev, eid, is_forward) = visited[&current];
            let residual = if is_forward {
                let cap = network.graph.get_edge(eid).map(|e| e.data).unwrap_or(0.0);
                cap - flow.get(&eid).copied().unwrap_or(0.0)
            } else {
                flow.get(&eid).copied().unwrap_or(0.0)
            };
            bottleneck = bottleneck.min(residual);
            current = prev;
        }

        // Update flows
        current = sink;
        while current != source {
            let (prev, eid, is_forward) = visited[&current];
            if is_forward {
                *flow.get_mut(&eid).unwrap() += bottleneck;
            } else {
                *flow.get_mut(&eid).unwrap() -= bottleneck;
            }
            current = prev;
        }

        total_flow += bottleneck;
    }

    FlowResult {
        max_flow: total_flow,
        edge_flows: flow,
    }
}

impl FlowResult {
    /// Extract min-cut: returns (S, T) partition where S contains the source.
    /// S = nodes reachable from source in the residual graph after max flow.
    pub fn min_cut<N, E>(&self, network: &FlowNetwork) -> (HashSet<NodeId>, HashSet<NodeId>) {
        let node_ids = network.graph.node_ids();

        // BFS from source in residual graph
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();
        reachable.insert(network.source);
        queue.push_back(network.source);

        while let Some(node) = queue.pop_front() {
            for (nbr, eid) in network.graph.neighbor_edges(node) {
                let cap = network.graph.get_edge(eid).map(|e| e.data).unwrap_or(0.0);
                let f = self.edge_flows.get(&eid).copied().unwrap_or(0.0);
                if cap - f > 1e-9 && !reachable.contains(&nbr) {
                    reachable.insert(nbr);
                    queue.push_back(nbr);
                }
            }
        }

        let s_set = reachable;
        let t_set: HashSet<NodeId> = node_ids.into_iter().filter(|n| !s_set.contains(n)).collect();
        (s_set, t_set)
    }
}

/// Visualizes flow as particle speed along edges.
pub struct FlowVisualizer {
    pub base_speed: f32,
    pub max_speed: f32,
    pub particle_color: Vec4,
}

impl FlowVisualizer {
    pub fn new() -> Self {
        Self {
            base_speed: 1.0,
            max_speed: 10.0,
            particle_color: Vec4::new(0.3, 0.6, 1.0, 1.0),
        }
    }

    /// Generate particle data for each edge based on flow.
    /// Returns Vec of (edge_id, start, end, speed, color).
    pub fn generate_particles(&self, network: &FlowNetwork, result: &FlowResult) -> Vec<FlowParticle> {
        let mut particles = Vec::new();
        let max_flow = result.max_flow.max(1e-6);

        for (&eid, &flow) in &result.edge_flows {
            if flow <= 0.0 { continue; }
            if let Some(edge) = network.graph.get_edge(eid) {
                let start = network.graph.node_position(edge.from);
                let end = network.graph.node_position(edge.to);
                let ratio = flow / max_flow;
                let speed = self.base_speed + ratio * (self.max_speed - self.base_speed);
                let alpha = 0.3 + 0.7 * ratio;
                let color = Vec4::new(
                    self.particle_color.x,
                    self.particle_color.y,
                    self.particle_color.z,
                    alpha,
                );
                particles.push(FlowParticle {
                    edge_id: eid,
                    start,
                    end,
                    speed,
                    color,
                    flow,
                });
            }
        }
        particles
    }
}

#[derive(Debug, Clone)]
pub struct FlowParticle {
    pub edge_id: EdgeId,
    pub start: Vec2,
    pub end: Vec2,
    pub speed: f32,
    pub color: Vec4,
    pub flow: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_network() -> FlowNetwork {
        // s -> a -> t with cap 10
        // s -> b -> t with cap 5
        // a -> b with cap 3
        let mut b = FlowNetwork::builder();
        let s = b.add_node();
        let a = b.add_node();
        let bb = b.add_node();
        let t = b.add_node();
        b.set_source(s);
        b.set_sink(t);
        b.add_capacity(s, a, 10.0);
        b.add_capacity(s, bb, 5.0);
        b.add_capacity(a, t, 8.0);
        b.add_capacity(bb, t, 7.0);
        b.add_capacity(a, bb, 3.0);
        b.build()
    }

    #[test]
    fn test_max_flow_simple() {
        let net = simple_network();
        let result = ford_fulkerson(&net, net.source, net.sink);
        // Max flow = min(supply from s, demand to t)
        // s can push 15 total (10 + 5), t can accept 15 (8 + 7)
        // a->t is bottleneck: 10 from s->a, only 8 to t, but 3 can go a->b->t
        // Total: 8 + min(5+3, 7) = 8 + 7 = 15
        assert!((result.max_flow - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_max_flow_single_edge() {
        let mut b = FlowNetwork::builder();
        let s = b.add_node();
        let t = b.add_node();
        b.set_source(s);
        b.set_sink(t);
        b.add_capacity(s, t, 42.0);
        let net = b.build();
        let result = ford_fulkerson(&net, net.source, net.sink);
        assert!((result.max_flow - 42.0).abs() < 0.01);
    }

    #[test]
    fn test_max_flow_no_path() {
        let mut b = FlowNetwork::builder();
        let s = b.add_node();
        let t = b.add_node();
        b.set_source(s);
        b.set_sink(t);
        // No edge
        let net = b.build();
        let result = ford_fulkerson(&net, net.source, net.sink);
        assert_eq!(result.max_flow, 0.0);
    }

    #[test]
    fn test_min_cut() {
        let mut b = FlowNetwork::builder();
        let s = b.add_node();
        let a = b.add_node();
        let t = b.add_node();
        b.set_source(s);
        b.set_sink(t);
        b.add_capacity(s, a, 5.0);
        b.add_capacity(a, t, 3.0);
        let net = b.build();
        let result = ford_fulkerson(&net, net.source, net.sink);
        assert!((result.max_flow - 3.0).abs() < 0.01);
        let (s_set, t_set) = result.min_cut::<(), f32>(&net);
        assert!(s_set.contains(&s));
        assert!(t_set.contains(&t));
    }

    #[test]
    fn test_flow_visualizer() {
        let mut b = FlowNetwork::builder();
        let s = b.add_node();
        let t = b.add_node();
        b.set_source(s);
        b.set_sink(t);
        b.add_capacity(s, t, 10.0);
        let net = b.build();
        let result = ford_fulkerson(&net, net.source, net.sink);
        let viz = FlowVisualizer::new();
        let particles = viz.generate_particles(&net, &result);
        assert_eq!(particles.len(), 1);
        assert!(particles[0].speed > 0.0);
    }

    #[test]
    fn test_parallel_paths() {
        let mut b = FlowNetwork::builder();
        let s = b.add_node();
        let t = b.add_node();
        b.set_source(s);
        b.set_sink(t);
        b.add_capacity(s, t, 5.0);
        b.add_capacity(s, t, 3.0);
        let net = b.build();
        let result = ford_fulkerson(&net, net.source, net.sink);
        assert!((result.max_flow - 8.0).abs() < 0.01);
    }
}
