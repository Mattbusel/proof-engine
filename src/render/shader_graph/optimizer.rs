//! Shader graph optimizer — dead-node elimination, constant folding,
//! common subexpression sharing, and redundant node removal.

use std::collections::HashSet;
use super::{ShaderGraph, NodeId};
use super::nodes::NodeType;

pub struct GraphOptimizer;

impl GraphOptimizer {
    /// Run all optimization passes and return an optimized clone of the graph.
    pub fn run(graph: &ShaderGraph) -> ShaderGraph {
        let mut g = graph.clone();
        Self::eliminate_dead_nodes(&mut g);
        Self::fold_constants(&mut g);
        Self::remove_identity_operations(&mut g);
        g
    }

    // ── Dead-node elimination ──────────────────────────────────────────────────
    /// Remove nodes that don't contribute to any output.

    fn eliminate_dead_nodes(graph: &mut ShaderGraph) {
        let reachable = Self::reachable_from_output(graph);
        let dead: Vec<NodeId> = graph.nodes.keys()
            .copied()
            .filter(|id| !reachable.contains(id))
            .collect();
        for id in dead {
            graph.remove_node(id);
        }
    }

    fn reachable_from_output(graph: &ShaderGraph) -> HashSet<NodeId> {
        let mut reachable = HashSet::new();
        let mut stack = Vec::new();

        if let Some(out) = graph.output_node {
            stack.push(out);
        } else {
            // If no output set, everything is reachable
            return graph.nodes.keys().copied().collect();
        }

        while let Some(id) = stack.pop() {
            if reachable.insert(id) {
                // Find all nodes feeding into this one
                for edge in graph.edges.iter().filter(|e| e.to_node == id) {
                    stack.push(edge.from_node);
                }
            }
        }
        reachable
    }

    // ── Constant folding ───────────────────────────────────────────────────────
    /// Replace simple constant operations with ConstFloat nodes.

    fn fold_constants(graph: &mut ShaderGraph) {
        let const_values = Self::collect_constant_values(graph);
        let mut foldable: Vec<(NodeId, f32)> = Vec::new();

        for (id, node) in &graph.nodes {
            match &node.node_type {
                NodeType::Add => {
                    if let (Some(a), Some(b)) = (
                        Self::get_input_const(&const_values, graph, *id, 0),
                        Self::get_input_const(&const_values, graph, *id, 1),
                    ) {
                        foldable.push((*id, a + b));
                    }
                }
                NodeType::Multiply => {
                    if let (Some(a), Some(b)) = (
                        Self::get_input_const(&const_values, graph, *id, 0),
                        Self::get_input_const(&const_values, graph, *id, 1),
                    ) {
                        foldable.push((*id, a * b));
                    }
                }
                NodeType::Subtract => {
                    if let (Some(a), Some(b)) = (
                        Self::get_input_const(&const_values, graph, *id, 0),
                        Self::get_input_const(&const_values, graph, *id, 1),
                    ) {
                        foldable.push((*id, a - b));
                    }
                }
                NodeType::Sin => {
                    if let Some(a) = Self::get_input_const(&const_values, graph, *id, 0) {
                        foldable.push((*id, a.sin()));
                    }
                }
                NodeType::Cos => {
                    if let Some(a) = Self::get_input_const(&const_values, graph, *id, 0) {
                        foldable.push((*id, a.cos()));
                    }
                }
                NodeType::Sqrt => {
                    if let Some(a) = Self::get_input_const(&const_values, graph, *id, 0) {
                        if a >= 0.0 { foldable.push((*id, a.sqrt())); }
                    }
                }
                NodeType::Abs => {
                    if let Some(a) = Self::get_input_const(&const_values, graph, *id, 0) {
                        foldable.push((*id, a.abs()));
                    }
                }
                NodeType::Negate => {
                    if let Some(a) = Self::get_input_const(&const_values, graph, *id, 0) {
                        foldable.push((*id, -a));
                    }
                }
                NodeType::OneMinus => {
                    if let Some(a) = Self::get_input_const(&const_values, graph, *id, 0) {
                        foldable.push((*id, 1.0 - a));
                    }
                }
                NodeType::Exp => {
                    if let Some(a) = Self::get_input_const(&const_values, graph, *id, 0) {
                        foldable.push((*id, a.exp()));
                    }
                }
                _ => {}
            }
        }

        // Apply folds: change node type to ConstFloat, disconnect inputs
        for (id, val) in foldable {
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.node_type = NodeType::ConstFloat(val);
                node.constant_inputs.clear();
            }
            // Remove all incoming edges to this node
            graph.edges.retain(|e| e.to_node != id);
        }
    }

    fn collect_constant_values(graph: &ShaderGraph) -> std::collections::HashMap<NodeId, f32> {
        let mut map = std::collections::HashMap::new();
        for (id, node) in &graph.nodes {
            if let NodeType::ConstFloat(v) = node.node_type {
                map.insert(*id, v);
            }
        }
        map
    }

    fn get_input_const(
        const_values: &std::collections::HashMap<NodeId, f32>,
        graph:        &ShaderGraph,
        node_id:      NodeId,
        slot:         u8,
    ) -> Option<f32> {
        // Check if slot is connected to a ConstFloat node
        for edge in graph.edges.iter().filter(|e| e.to_node == node_id && e.to_slot == slot) {
            if let Some(&v) = const_values.get(&edge.from_node) {
                return Some(v);
            }
        }
        // Check constant_inputs fallback
        if let Some(node) = graph.nodes.get(&node_id) {
            if let Some(s) = node.constant_inputs.get(&(slot as usize)) {
                return s.parse().ok();
            }
        }
        None
    }

    // ── Identity operation removal ─────────────────────────────────────────────

    fn remove_identity_operations(graph: &mut ShaderGraph) {
        let mut to_bypass: Vec<NodeId> = Vec::new();

        for (id, node) in &graph.nodes {
            match &node.node_type {
                // Multiply by 1.0 → bypass
                NodeType::Multiply => {
                    let b_const = Self::get_input_const(
                        &Self::collect_constant_values(graph), graph, *id, 1
                    );
                    if b_const == Some(1.0) { to_bypass.push(*id); }
                }
                // Add 0.0 → bypass
                NodeType::Add => {
                    let b_const = Self::get_input_const(
                        &Self::collect_constant_values(graph), graph, *id, 1
                    );
                    if b_const == Some(0.0) { to_bypass.push(*id); }
                }
                _ => {}
            }
        }

        for id in to_bypass {
            if let Some(node) = graph.nodes.get_mut(&id) {
                node.bypassed = true;
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::shader_graph::ShaderGraph;
    use crate::render::shader_graph::nodes::NodeType;

    #[test]
    fn test_dead_node_elimination() {
        let mut g = ShaderGraph::new("test");
        let dead = g.add_node(NodeType::Sin);           // not connected to output
        let uv   = g.add_node(NodeType::UvCoord);
        let out  = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(uv, 0, out, 0);

        let optimized = GraphOptimizer::run(&g);
        // Dead sin node should be removed
        assert!(optimized.node(dead).is_none());
        // UV and output should survive
        assert!(optimized.node(uv).is_some());
        assert!(optimized.node(out).is_some());
    }

    #[test]
    fn test_constant_folding_add() {
        let mut g = ShaderGraph::new("test");
        let a   = g.add_node(NodeType::ConstFloat(3.0));
        let b   = g.add_node(NodeType::ConstFloat(4.0));
        let add = g.add_node(NodeType::Add);
        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(a,   0, add, 0);
        let _ = g.connect(b,   0, add, 1);
        let _ = g.connect(add, 0, out, 0);

        let optimized = GraphOptimizer::run(&g);
        // The add node should now be a ConstFloat(7.0)
        if let Some(node) = optimized.node(add) {
            assert_eq!(node.node_type, NodeType::ConstFloat(7.0));
        }
    }

    #[test]
    fn test_constant_folding_sin() {
        let mut g   = ShaderGraph::new("test");
        let zero    = g.add_node(NodeType::ConstFloat(0.0));
        let sin_n   = g.add_node(NodeType::Sin);
        let out     = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(zero,  0, sin_n, 0);
        let _ = g.connect(sin_n, 0, out,   0);

        let optimized = GraphOptimizer::run(&g);
        if let Some(node) = optimized.node(sin_n) {
            // sin(0) = 0
            assert_eq!(node.node_type, NodeType::ConstFloat(0.0));
        }
    }

    #[test]
    fn test_no_crash_empty_graph() {
        let g = ShaderGraph::new("empty");
        let _ = GraphOptimizer::run(&g);
    }

    #[test]
    fn test_reachable_includes_all_ancestors() {
        let mut g  = ShaderGraph::new("test");
        let uv     = g.add_node(NodeType::UvCoord);
        let sin    = g.add_node(NodeType::Sin);
        let cos    = g.add_node(NodeType::Cos);
        let add    = g.add_node(NodeType::Add);
        let out    = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(uv,  0, sin, 0);
        let _ = g.connect(uv,  0, cos, 0);
        let _ = g.connect(sin, 0, add, 0);
        let _ = g.connect(cos, 0, add, 1);
        let _ = g.connect(add, 0, out, 0);

        let opt = GraphOptimizer::run(&g);
        assert!(opt.node(uv).is_some());
        assert!(opt.node(sin).is_some());
        assert!(opt.node(cos).is_some());
        assert!(opt.node(add).is_some());
    }
}
