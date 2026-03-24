//! Advanced optimization passes for shader graphs: type inference, algebraic simplification,
//! redundant cast removal, loop detection, node merging, instruction count estimation,
//! and shader variant caching.

use std::collections::{HashMap, HashSet};
use super::nodes::{
    Connection, DataType, NodeId, NodeType, ParamValue, ShaderGraph, ShaderNode,
};

// ---------------------------------------------------------------------------
// Optimization pass enum
// ---------------------------------------------------------------------------

/// Individual optimization passes that can be applied to a shader graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationPass {
    /// Infer types through the graph and insert implicit casts where needed.
    TypeInference,
    /// Remove redundant type casts (e.g., float -> float).
    RedundantCastRemoval,
    /// Apply algebraic simplifications: x*1=x, x+0=x, x*0=0, etc.
    AlgebraicSimplification,
    /// Detect cycles/loops in the graph (report errors).
    LoopDetection,
    /// Merge sequential compatible math operations into single nodes.
    NodeMerging,
    /// Estimate instruction count per node and total.
    InstructionCounting,
    /// Dead code elimination (remove nodes not reachable from outputs).
    DeadCodeElimination,
    /// Constant propagation through known-value chains.
    ConstantPropagation,
}

// ---------------------------------------------------------------------------
// Optimizer config
// ---------------------------------------------------------------------------

/// Configuration for the shader optimizer.
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    /// Which passes to run, in order.
    pub passes: Vec<OptimizationPass>,
    /// Maximum number of iterations for iterative passes.
    pub max_iterations: usize,
    /// If true, log optimization statistics.
    pub verbose: bool,
    /// Maximum allowed instruction count before warning.
    pub instruction_budget: u32,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            passes: vec![
                OptimizationPass::TypeInference,
                OptimizationPass::DeadCodeElimination,
                OptimizationPass::AlgebraicSimplification,
                OptimizationPass::RedundantCastRemoval,
                OptimizationPass::NodeMerging,
                OptimizationPass::ConstantPropagation,
                OptimizationPass::InstructionCounting,
                OptimizationPass::LoopDetection,
            ],
            max_iterations: 10,
            verbose: false,
            instruction_budget: 512,
        }
    }
}

// ---------------------------------------------------------------------------
// Optimization report
// ---------------------------------------------------------------------------

/// Report generated after optimization, containing statistics.
#[derive(Debug, Clone)]
pub struct OptimizationReport {
    /// Number of nodes before optimization.
    pub nodes_before: usize,
    /// Number of nodes after optimization.
    pub nodes_after: usize,
    /// Number of connections before.
    pub connections_before: usize,
    /// Number of connections after.
    pub connections_after: usize,
    /// Number of nodes removed by dead code elimination.
    pub dead_nodes_removed: usize,
    /// Number of algebraic simplifications applied.
    pub algebraic_simplifications: usize,
    /// Number of redundant casts removed.
    pub redundant_casts_removed: usize,
    /// Number of nodes merged.
    pub nodes_merged: usize,
    /// Whether a cycle was detected.
    pub cycle_detected: bool,
    /// Estimated total instruction count.
    pub estimated_instructions: u32,
    /// Whether the instruction budget was exceeded.
    pub over_budget: bool,
    /// Inferred types for each node output: (node_id, socket_idx) -> DataType.
    pub inferred_types: HashMap<(u64, usize), DataType>,
    /// Warnings generated during optimization.
    pub warnings: Vec<String>,
}

impl OptimizationReport {
    fn new(graph: &ShaderGraph) -> Self {
        Self {
            nodes_before: graph.node_count(),
            nodes_after: graph.node_count(),
            connections_before: graph.connections().len(),
            connections_after: graph.connections().len(),
            dead_nodes_removed: 0,
            algebraic_simplifications: 0,
            redundant_casts_removed: 0,
            nodes_merged: 0,
            cycle_detected: false,
            estimated_instructions: 0,
            over_budget: false,
            inferred_types: HashMap::new(),
            warnings: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shader Optimizer
// ---------------------------------------------------------------------------

/// The main shader graph optimizer.
pub struct ShaderOptimizer {
    config: OptimizerConfig,
}

impl ShaderOptimizer {
    pub fn new(config: OptimizerConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(OptimizerConfig::default())
    }

    /// Run all configured optimization passes on the graph.
    /// Returns the optimized graph and a report.
    pub fn optimize(&self, graph: &ShaderGraph) -> (ShaderGraph, OptimizationReport) {
        let mut optimized = graph.clone();
        let mut report = OptimizationReport::new(graph);

        for pass in &self.config.passes {
            match pass {
                OptimizationPass::TypeInference => {
                    self.run_type_inference(&optimized, &mut report);
                }
                OptimizationPass::RedundantCastRemoval => {
                    let removed = self.run_redundant_cast_removal(&mut optimized, &report.inferred_types);
                    report.redundant_casts_removed += removed;
                }
                OptimizationPass::AlgebraicSimplification => {
                    let count = self.run_algebraic_simplification(&mut optimized);
                    report.algebraic_simplifications += count;
                }
                OptimizationPass::LoopDetection => {
                    report.cycle_detected = self.detect_loops(&optimized);
                    if report.cycle_detected {
                        report.warnings.push("Cycle detected in shader graph".to_string());
                    }
                }
                OptimizationPass::NodeMerging => {
                    let merged = self.run_node_merging(&mut optimized);
                    report.nodes_merged += merged;
                }
                OptimizationPass::InstructionCounting => {
                    report.estimated_instructions = self.estimate_instructions(&optimized);
                    report.over_budget = report.estimated_instructions > self.config.instruction_budget;
                    if report.over_budget {
                        report.warnings.push(format!(
                            "Instruction count {} exceeds budget {}",
                            report.estimated_instructions, self.config.instruction_budget
                        ));
                    }
                }
                OptimizationPass::DeadCodeElimination => {
                    let removed = self.run_dead_code_elimination(&mut optimized);
                    report.dead_nodes_removed += removed;
                }
                OptimizationPass::ConstantPropagation => {
                    self.run_constant_propagation(&mut optimized);
                }
            }
        }

        report.nodes_after = optimized.node_count();
        report.connections_after = optimized.connections().len();

        (optimized, report)
    }

    // -----------------------------------------------------------------------
    // Type inference
    // -----------------------------------------------------------------------

    /// Infer output types for all nodes based on their input connections.
    fn run_type_inference(&self, graph: &ShaderGraph, report: &mut OptimizationReport) {
        for node in graph.nodes() {
            for (idx, socket) in node.outputs.iter().enumerate() {
                let inferred = self.infer_output_type(graph, node, idx);
                report.inferred_types.insert((node.id.0, idx), inferred.unwrap_or(socket.data_type));
            }
        }
    }

    /// Infer the output type for a specific socket of a node, considering connected inputs.
    fn infer_output_type(&self, graph: &ShaderGraph, node: &ShaderNode, output_idx: usize) -> Option<DataType> {
        // For most nodes, the output type is fixed by the node definition
        let base_type = node.outputs.get(output_idx)?.data_type;

        // For math ops, the output type should match the "widest" input type
        match &node.node_type {
            NodeType::Add | NodeType::Sub | NodeType::Mul | NodeType::Div
            | NodeType::Lerp | NodeType::Clamp | NodeType::Smoothstep => {
                let incoming = graph.incoming_connections(node.id);
                let mut widest = base_type;
                for conn in &incoming {
                    if let Some(src_node) = graph.node(conn.from_node) {
                        if let Some(src_type) = src_node.output_type(conn.from_socket) {
                            widest = wider_type(widest, src_type);
                        }
                    }
                }
                Some(widest)
            }
            _ => Some(base_type),
        }
    }

    // -----------------------------------------------------------------------
    // Redundant cast removal
    // -----------------------------------------------------------------------

    /// Remove nodes that perform identity casts (same type in and out).
    fn run_redundant_cast_removal(
        &self,
        graph: &mut ShaderGraph,
        inferred_types: &HashMap<(u64, usize), DataType>,
    ) -> usize {
        let mut to_remove: Vec<NodeId> = Vec::new();

        // Find all connections where source and dest types match and the node
        // is essentially a pass-through
        let node_ids: Vec<NodeId> = graph.node_ids().collect();
        for nid in &node_ids {
            let node = match graph.node(*nid) {
                Some(n) => n,
                None => continue,
            };

            // Check if this is a single-input, single-output math node where
            // the operation is identity-like
            if node.inputs.len() != 1 || node.outputs.len() != 1 {
                continue;
            }

            let incoming = graph.incoming_connections(*nid);
            if incoming.len() != 1 {
                continue;
            }

            let conn = incoming[0];
            let src_type = inferred_types.get(&(conn.from_node.0, conn.from_socket))
                .copied()
                .unwrap_or(DataType::Float);
            let dst_type = node.outputs[0].data_type;

            // If types are the same and this is a Normalize of an already-normalized vector,
            // or similar identity operation, we could remove it.
            // For now, just check if types match exactly for pass-through detection
            if src_type == dst_type {
                // Check if the node type is effectively a no-op
                let is_noop = match &node.node_type {
                    NodeType::Abs => {
                        // abs is no-op if input is known non-negative
                        false // conservative
                    }
                    _ => false,
                };
                if is_noop {
                    to_remove.push(*nid);
                }
            }
        }

        let count = to_remove.len();

        for nid in to_remove {
            self.bypass_node(graph, nid);
        }

        count
    }

    /// Remove a single-input, single-output node by connecting its input source
    /// directly to all its output destinations.
    fn bypass_node(&self, graph: &mut ShaderGraph, node_id: NodeId) {
        // Find the single incoming connection
        let incoming: Vec<Connection> = graph.incoming_connections(node_id)
            .into_iter().cloned().collect();
        let outgoing: Vec<Connection> = graph.outgoing_connections(node_id)
            .into_iter().cloned().collect();

        if incoming.len() != 1 {
            return;
        }

        let source = &incoming[0];

        // Redirect all outgoing connections to point to the source
        for out_conn in &outgoing {
            graph.disconnect(node_id, out_conn.from_socket, out_conn.to_node, out_conn.to_socket);
            graph.connect(source.from_node, source.from_socket, out_conn.to_node, out_conn.to_socket);
        }

        // Remove the node
        graph.remove_node(node_id);
    }

    // -----------------------------------------------------------------------
    // Algebraic simplification
    // -----------------------------------------------------------------------

    /// Apply algebraic simplifications: x*1=x, x+0=x, x*0=0, x-0=x, x/1=x, x^1=x, x^0=1.
    fn run_algebraic_simplification(&self, graph: &mut ShaderGraph) -> usize {
        let mut simplifications = 0;

        for _iteration in 0..self.config.max_iterations {
            let mut changes_this_round = 0;

            let node_ids: Vec<NodeId> = graph.node_ids().collect();
            for &nid in &node_ids {
                let node = match graph.node(&nid) {
                    Some(n) => n.clone(),
                    None => continue,
                };

                let result = self.try_simplify_node(graph, &node);
                match result {
                    SimplifyResult::NoChange => {}
                    SimplifyResult::ReplaceWithInput(input_idx) => {
                        // This node reduces to one of its inputs — bypass it
                        let incoming: Vec<Connection> = graph.incoming_connections(nid)
                            .into_iter().cloned().collect();
                        let source_conn = incoming.iter().find(|c| c.to_socket == input_idx);
                        if let Some(src) = source_conn {
                            let outgoing: Vec<Connection> = graph.outgoing_connections(nid)
                                .into_iter().cloned().collect();
                            for out in &outgoing {
                                graph.connect(src.from_node, src.from_socket, out.to_node, out.to_socket);
                            }
                            graph.remove_node(nid);
                            changes_this_round += 1;
                        }
                    }
                    SimplifyResult::ReplaceWithConstant(value) => {
                        // Replace this node with a Color source holding the constant
                        let outgoing: Vec<Connection> = graph.outgoing_connections(nid)
                            .into_iter().cloned().collect();

                        // Create a replacement Color node with the constant value
                        let mut replacement = ShaderNode::new(NodeId(0), NodeType::Color);
                        replacement.inputs[0].default_value = Some(match &value {
                            ParamValue::Float(v) => ParamValue::Vec4([*v, *v, *v, 1.0]),
                            ParamValue::Vec3(v) => ParamValue::Vec4([v[0], v[1], v[2], 1.0]),
                            other => other.clone(),
                        });
                        replacement.properties.insert("folded_constant".to_string(), value);

                        let new_id = graph.add_node_with(replacement);

                        // Redirect outputs
                        for out in &outgoing {
                            graph.connect(new_id, 0, out.to_node, out.to_socket);
                        }

                        graph.remove_node(nid);
                        changes_this_round += 1;
                    }
                }
            }

            simplifications += changes_this_round;
            if changes_this_round == 0 {
                break;
            }
        }

        simplifications
    }

    /// Try to simplify a single node.
    fn try_simplify_node(&self, graph: &ShaderGraph, node: &ShaderNode) -> SimplifyResult {
        let incoming: Vec<&Connection> = graph.incoming_connections(node.id);

        match &node.node_type {
            // x + 0 = x
            NodeType::Add => {
                if let Some(result) = self.check_identity_binary(graph, node, &incoming, 0.0) {
                    return result;
                }
            }
            // x - 0 = x (only right operand)
            NodeType::Sub => {
                if self.is_input_constant(graph, node, &incoming, 1, 0.0) {
                    return SimplifyResult::ReplaceWithInput(0);
                }
            }
            // x * 1 = x; x * 0 = 0
            NodeType::Mul => {
                if let Some(result) = self.check_identity_binary(graph, node, &incoming, 1.0) {
                    return result;
                }
                // x * 0 = 0
                if self.is_input_constant(graph, node, &incoming, 0, 0.0) {
                    return SimplifyResult::ReplaceWithConstant(ParamValue::Float(0.0));
                }
                if self.is_input_constant(graph, node, &incoming, 1, 0.0) {
                    return SimplifyResult::ReplaceWithConstant(ParamValue::Float(0.0));
                }
            }
            // x / 1 = x
            NodeType::Div => {
                if self.is_input_constant(graph, node, &incoming, 1, 1.0) {
                    return SimplifyResult::ReplaceWithInput(0);
                }
            }
            // pow(x, 1) = x; pow(x, 0) = 1
            NodeType::Pow => {
                if self.is_input_constant(graph, node, &incoming, 1, 1.0) {
                    return SimplifyResult::ReplaceWithInput(0);
                }
                if self.is_input_constant(graph, node, &incoming, 1, 0.0) {
                    return SimplifyResult::ReplaceWithConstant(ParamValue::Float(1.0));
                }
            }
            // lerp(a, b, 0) = a; lerp(a, b, 1) = b
            NodeType::Lerp => {
                if self.is_input_constant(graph, node, &incoming, 2, 0.0) {
                    return SimplifyResult::ReplaceWithInput(0);
                }
                if self.is_input_constant(graph, node, &incoming, 2, 1.0) {
                    return SimplifyResult::ReplaceWithInput(1);
                }
            }
            // clamp(x, -inf, inf) effectively = x (we check 0..1 identity)
            NodeType::Clamp => {
                // If min=0.0, max=1.0, and x is known to be in [0,1], this is identity
                // For now, conservative — no simplification
            }
            // step(0, x) = 1 for all x >= 0
            NodeType::Step => {
                if self.is_input_constant(graph, node, &incoming, 0, 0.0) {
                    // step(0, x) = 1 if x >= 0 — we can't prove x >= 0 in general
                }
            }
            _ => {}
        }

        SimplifyResult::NoChange
    }

    /// Check if one of the two inputs to a binary op is a specific constant (identity element).
    /// If so, the result equals the other input.
    fn check_identity_binary(
        &self,
        graph: &ShaderGraph,
        node: &ShaderNode,
        incoming: &[&Connection],
        identity: f32,
    ) -> Option<SimplifyResult> {
        if self.is_input_constant(graph, node, incoming, 0, identity) {
            return Some(SimplifyResult::ReplaceWithInput(1));
        }
        if self.is_input_constant(graph, node, incoming, 1, identity) {
            return Some(SimplifyResult::ReplaceWithInput(0));
        }
        None
    }

    /// Check if a specific input socket has a constant float value.
    fn is_input_constant(
        &self,
        _graph: &ShaderGraph,
        node: &ShaderNode,
        incoming: &[&Connection],
        socket_idx: usize,
        expected: f32,
    ) -> bool {
        // First, check if there's a connection to this socket
        let has_connection = incoming.iter().any(|c| c.to_socket == socket_idx);
        if has_connection {
            // We'd need to trace back to the source node to check if it's a constant
            // For simplicity, we only check unconnected sockets with default values
            return false;
        }

        // Check the default value
        if let Some(default) = node.input_default(socket_idx) {
            if let Some(val) = default.as_float() {
                return (val - expected).abs() < 1e-7;
            }
        }

        false
    }

    // -----------------------------------------------------------------------
    // Loop/cycle detection
    // -----------------------------------------------------------------------

    /// Detect if the graph contains any cycles using DFS coloring.
    fn detect_loops(&self, graph: &ShaderGraph) -> bool {
        let mut color: HashMap<NodeId, u8> = HashMap::new(); // 0=white, 1=grey, 2=black
        for nid in graph.node_ids() {
            color.insert(nid, 0);
        }

        for nid in graph.node_ids() {
            if color[&nid] == 0 {
                if self.dfs_cycle(graph, nid, &mut color) {
                    return true;
                }
            }
        }

        false
    }

    fn dfs_cycle(&self, graph: &ShaderGraph, node_id: NodeId, color: &mut HashMap<NodeId, u8>) -> bool {
        color.insert(node_id, 1); // grey

        for conn in graph.outgoing_connections(node_id) {
            let neighbor = conn.to_node;
            match color.get(&neighbor) {
                Some(1) => return true,  // back edge => cycle
                Some(0) => {
                    if self.dfs_cycle(graph, neighbor, color) {
                        return true;
                    }
                }
                _ => {} // already visited (black)
            }
        }

        color.insert(node_id, 2); // black
        false
    }

    // -----------------------------------------------------------------------
    // Node merging
    // -----------------------------------------------------------------------

    /// Merge chains of compatible sequential math operations.
    /// E.g., Add(Add(a, b), c) can note that it's a 3-way add (though GLSL
    /// doesn't have a single instruction, we can eliminate intermediate variables).
    fn run_node_merging(&self, graph: &mut ShaderGraph) -> usize {
        let mut merged = 0;

        // Strategy: find chains of the same binary op where the intermediate result
        // is used only once. E.g., if Add(a,b) feeds only into Add(_, c), we can
        // eliminate the intermediate by rewriting as Add(a, Add_inline(b, c)).
        // In practice, we mark the intermediate node as "inline" by removing it
        // and adjusting the downstream node's GLSL.

        let node_ids: Vec<NodeId> = graph.node_ids().collect();
        let mut removed_set: HashSet<NodeId> = HashSet::new();

        for &nid in &node_ids {
            if removed_set.contains(&nid) {
                continue;
            }

            let node = match graph.node(&nid) {
                Some(n) => n,
                None => continue,
            };

            // Only merge binary math ops
            let is_mergeable = matches!(
                node.node_type,
                NodeType::Add | NodeType::Sub | NodeType::Mul
            );
            if !is_mergeable {
                continue;
            }

            // Check if this node has exactly one outgoing connection
            let outgoing = graph.outgoing_connections(nid);
            if outgoing.len() != 1 {
                continue;
            }

            let out_conn = outgoing[0].clone();
            let downstream = match graph.node(&out_conn.to_node) {
                Some(n) => n,
                None => continue,
            };

            // Must be the same operation type
            if downstream.node_type != node.node_type {
                continue;
            }

            // Don't merge if the downstream node is already in the removed set
            if removed_set.contains(&out_conn.to_node) {
                continue;
            }

            // The current node's output feeds into one of the downstream's inputs.
            // We'll propagate the current node's inputs to the downstream node's properties
            // so that the GLSL generator can inline the expression.

            // For now, mark the merge in properties and skip actual structural changes
            // to avoid complex graph rewiring. The compiler will handle inlining.
            if let Some(downstream_mut) = graph.node_mut(out_conn.to_node) {
                downstream_mut.properties.insert(
                    format!("merged_from_{}", nid.0),
                    ParamValue::Bool(true),
                );
                merged += 1;
            }
        }

        merged
    }

    // -----------------------------------------------------------------------
    // Dead code elimination
    // -----------------------------------------------------------------------

    fn run_dead_code_elimination(&self, graph: &mut ShaderGraph) -> usize {
        let outputs = graph.output_nodes();
        if outputs.is_empty() {
            return 0;
        }

        // BFS from outputs to find all reachable nodes
        let mut reachable: HashSet<NodeId> = HashSet::new();
        let mut queue: Vec<NodeId> = outputs;

        while let Some(nid) = queue.pop() {
            if !reachable.insert(nid) {
                continue;
            }
            for conn in graph.connections() {
                if conn.to_node == nid && !reachable.contains(&conn.from_node) {
                    queue.push(conn.from_node);
                }
            }
        }

        // Remove unreachable nodes
        let all_ids: Vec<NodeId> = graph.node_ids().collect();
        let mut removed = 0;
        for nid in all_ids {
            if !reachable.contains(&nid) {
                graph.remove_node(nid);
                removed += 1;
            }
        }

        removed
    }

    // -----------------------------------------------------------------------
    // Constant propagation
    // -----------------------------------------------------------------------

    /// Propagate known constant values through chains of pure math nodes.
    fn run_constant_propagation(&self, graph: &mut ShaderGraph) {
        // Build a map of known constant outputs
        let mut known_constants: HashMap<(NodeId, usize), ParamValue> = HashMap::new();

        // First, find all Color nodes with explicit constant values
        let node_ids: Vec<NodeId> = graph.node_ids().collect();
        for &nid in &node_ids {
            let node = match graph.node(&nid) {
                Some(n) => n,
                None => continue,
            };

            if node.node_type == NodeType::Color {
                if let Some(val) = &node.inputs[0].default_value {
                    // Check if this node has no incoming connections (truly constant)
                    let incoming = graph.incoming_connections(nid);
                    if incoming.is_empty() {
                        known_constants.insert((nid, 0), val.clone());
                    }
                }
            }
        }

        // Propagate through pure math nodes
        // (In a full implementation, we would do a topological traversal here.
        // For now, we store the constants for downstream use by the compiler.)
        for &nid in &node_ids {
            let node = match graph.node(&nid) {
                Some(n) => n,
                None => continue,
            };

            if !node.node_type.is_pure_math() {
                continue;
            }

            let incoming = graph.incoming_connections(nid);
            let mut all_inputs_known = true;
            let mut input_vals: Vec<ParamValue> = Vec::new();

            for (idx, socket) in node.inputs.iter().enumerate() {
                let conn = incoming.iter().find(|c| c.to_socket == idx);
                if let Some(c) = conn {
                    if let Some(val) = known_constants.get(&(c.from_node, c.from_socket)) {
                        input_vals.push(val.clone());
                    } else {
                        all_inputs_known = false;
                        break;
                    }
                } else if let Some(def) = &socket.default_value {
                    input_vals.push(def.clone());
                } else {
                    all_inputs_known = false;
                    break;
                }
            }

            if all_inputs_known && !input_vals.is_empty() {
                // Try to evaluate
                if let Some(result) = evaluate_pure_node(&node.node_type, &input_vals) {
                    for (idx, val) in result.iter().enumerate() {
                        known_constants.insert((nid, idx), val.clone());
                    }
                    // Store the folded value in the node's properties for the compiler
                    if let Some(node_mut) = graph.node_mut(nid) {
                        if let Some(first) = result.into_iter().next() {
                            node_mut.properties.insert(
                                "propagated_constant".to_string(),
                                first,
                            );
                        }
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Instruction counting
    // -----------------------------------------------------------------------

    fn estimate_instructions(&self, graph: &ShaderGraph) -> u32 {
        graph.estimated_cost()
    }
}

// ---------------------------------------------------------------------------
// Helper types and functions
// ---------------------------------------------------------------------------

enum SimplifyResult {
    NoChange,
    /// Replace node with one of its inputs (by socket index).
    ReplaceWithInput(usize),
    /// Replace node with a constant value.
    ReplaceWithConstant(ParamValue),
}

/// Return the "wider" of two types for type promotion.
fn wider_type(a: DataType, b: DataType) -> DataType {
    let rank = |t: DataType| -> u8 {
        match t {
            DataType::Bool => 0,
            DataType::Int => 1,
            DataType::Float => 2,
            DataType::Vec2 => 3,
            DataType::Vec3 => 4,
            DataType::Vec4 => 5,
            DataType::Mat3 => 6,
            DataType::Mat4 => 7,
            DataType::Sampler2D => 8,
        }
    };
    if rank(a) >= rank(b) { a } else { b }
}

/// Evaluate a pure math node with known inputs (used for constant propagation).
fn evaluate_pure_node(node_type: &NodeType, inputs: &[ParamValue]) -> Option<Vec<ParamValue>> {
    match node_type {
        NodeType::Add => {
            let a = inputs.first()?.as_float()?;
            let b = inputs.get(1)?.as_float()?;
            Some(vec![ParamValue::Float(a + b)])
        }
        NodeType::Sub => {
            let a = inputs.first()?.as_float()?;
            let b = inputs.get(1)?.as_float()?;
            Some(vec![ParamValue::Float(a - b)])
        }
        NodeType::Mul => {
            let a = inputs.first()?.as_float()?;
            let b = inputs.get(1)?.as_float()?;
            Some(vec![ParamValue::Float(a * b)])
        }
        NodeType::Div => {
            let a = inputs.first()?.as_float()?;
            let b = inputs.get(1)?.as_float()?;
            if b.abs() < 1e-10 { return None; }
            Some(vec![ParamValue::Float(a / b)])
        }
        NodeType::Abs => {
            let x = inputs.first()?.as_float()?;
            Some(vec![ParamValue::Float(x.abs())])
        }
        NodeType::Floor => {
            let x = inputs.first()?.as_float()?;
            Some(vec![ParamValue::Float(x.floor())])
        }
        NodeType::Ceil => {
            let x = inputs.first()?.as_float()?;
            Some(vec![ParamValue::Float(x.ceil())])
        }
        NodeType::Fract => {
            let x = inputs.first()?.as_float()?;
            Some(vec![ParamValue::Float(x.fract())])
        }
        NodeType::Sqrt => {
            let x = inputs.first()?.as_float()?;
            Some(vec![ParamValue::Float(x.max(0.0).sqrt())])
        }
        NodeType::Sin => {
            let x = inputs.first()?.as_float()?;
            Some(vec![ParamValue::Float(x.sin())])
        }
        NodeType::Cos => {
            let x = inputs.first()?.as_float()?;
            Some(vec![ParamValue::Float(x.cos())])
        }
        NodeType::Pow => {
            let base = inputs.first()?.as_float()?;
            let exp = inputs.get(1)?.as_float()?;
            Some(vec![ParamValue::Float(base.max(0.0).powf(exp))])
        }
        NodeType::Lerp => {
            let a = inputs.first()?.as_float()?;
            let b = inputs.get(1)?.as_float()?;
            let t = inputs.get(2)?.as_float()?;
            Some(vec![ParamValue::Float(a + (b - a) * t)])
        }
        NodeType::Clamp => {
            let x = inputs.first()?.as_float()?;
            let lo = inputs.get(1)?.as_float()?;
            let hi = inputs.get(2)?.as_float()?;
            Some(vec![ParamValue::Float(x.clamp(lo, hi))])
        }
        NodeType::Step => {
            let edge = inputs.first()?.as_float()?;
            let x = inputs.get(1)?.as_float()?;
            Some(vec![ParamValue::Float(if x >= edge { 1.0 } else { 0.0 })])
        }
        NodeType::Invert => {
            let c = inputs.first()?.as_vec3()?;
            Some(vec![ParamValue::Vec3([1.0 - c[0], 1.0 - c[1], 1.0 - c[2]])])
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Convenience
// ---------------------------------------------------------------------------

/// Optimize a shader graph with default settings.
pub fn optimize_graph(graph: &ShaderGraph) -> (ShaderGraph, OptimizationReport) {
    ShaderOptimizer::with_defaults().optimize(graph)
}

/// Estimate the instruction count of a shader graph.
pub fn estimate_instruction_count(graph: &ShaderGraph) -> u32 {
    graph.estimated_cost()
}

/// Check if a graph has cycles.
pub fn has_cycles(graph: &ShaderGraph) -> bool {
    ShaderOptimizer::with_defaults().detect_loops(graph)
}
