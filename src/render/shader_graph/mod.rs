//! Node-based composable shader graph system.
//!
//! The shader graph compiles a directed acyclic graph of nodes into GLSL
//! fragment shader source code at runtime. Every visual effect in Proof
//! Engine can be described as a graph of mathematical operations.
//!
//! ## Architecture
//! - `ShaderGraph`      — owns nodes and edges, validates, compiles
//! - `ShaderNode`       — individual processing unit (40+ types)
//! - `ShaderEdge`       — connects an output socket to an input socket
//! - `GraphCompiler`    — walks the graph and emits GLSL
//! - `GraphOptimizer`   — dead-node elimination, constant folding
//! - `ShaderPreset`     — named, pre-built graphs for common effects
//! - `ShaderParameter`  — runtime-controllable uniform (bound to MathFunction)
//!
//! ## Quick Start
//! ```rust,no_run
//! use proof_engine::render::shader_graph::{ShaderGraph, ShaderPreset};
//! let graph = ShaderPreset::void_protocol();
//! let glsl  = graph.compile().unwrap();
//! println!("{}", glsl.fragment_source);
//! ```

pub mod nodes;
pub mod compiler;
pub mod optimizer;
pub mod presets;

pub use nodes::{ShaderNode, NodeType, SocketType, NodeSocket};
pub use compiler::{GraphCompiler, CompiledShader};
pub use optimizer::GraphOptimizer;
pub use presets::ShaderPreset;

use std::collections::HashMap;
use crate::math::MathFunction;

// ── Identifiers ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(pub u32);

// ── ShaderEdge ────────────────────────────────────────────────────────────────

/// A directed connection from one node's output socket to another's input.
#[derive(Debug, Clone)]
pub struct ShaderEdge {
    pub id:        EdgeId,
    pub from_node: NodeId,
    pub from_slot: u8,
    pub to_node:   NodeId,
    pub to_slot:   u8,
}

// ── ShaderParameter ───────────────────────────────────────────────────────────

/// A runtime-controllable parameter bound to a GLSL uniform.
#[derive(Debug, Clone)]
pub struct ShaderParameter {
    pub name:     String,
    pub glsl_name: String,
    pub value:    ParameterValue,
    /// Optional MathFunction driving this parameter over time.
    pub driver:   Option<MathFunction>,
    pub min:      f32,
    pub max:      f32,
}

#[derive(Debug, Clone)]
pub enum ParameterValue {
    Float(f32),
    Vec2(f32, f32),
    Vec3(f32, f32, f32),
    Vec4(f32, f32, f32, f32),
    Int(i32),
    Bool(bool),
}

impl ParameterValue {
    pub fn as_float(&self) -> Option<f32> {
        if let ParameterValue::Float(v) = self { Some(*v) } else { None }
    }

    pub fn glsl_type(&self) -> &'static str {
        match self {
            ParameterValue::Float(_)       => "float",
            ParameterValue::Vec2(_, _)     => "vec2",
            ParameterValue::Vec3(_, _, _)  => "vec3",
            ParameterValue::Vec4(_, _, _, _) => "vec4",
            ParameterValue::Int(_)         => "int",
            ParameterValue::Bool(_)        => "bool",
        }
    }

    pub fn glsl_literal(&self) -> String {
        match self {
            ParameterValue::Float(v)          => format!("{:.6}", v),
            ParameterValue::Vec2(x, y)        => format!("vec2({:.6}, {:.6})", x, y),
            ParameterValue::Vec3(x, y, z)     => format!("vec3({:.6}, {:.6}, {:.6})", x, y, z),
            ParameterValue::Vec4(x,y,z,w)     => format!("vec4({:.6},{:.6},{:.6},{:.6})",x,y,z,w),
            ParameterValue::Int(v)            => format!("{}", v),
            ParameterValue::Bool(v)           => if *v { "true".to_string() } else { "false".to_string() },
        }
    }
}

// ── ShaderGraph ───────────────────────────────────────────────────────────────

/// A directed acyclic graph of shader processing nodes.
#[derive(Debug, Clone)]
pub struct ShaderGraph {
    pub name:       String,
    pub nodes:      HashMap<NodeId, ShaderNode>,
    pub edges:      Vec<ShaderEdge>,
    pub parameters: Vec<ShaderParameter>,
    /// The node whose output is the final fragment color.
    pub output_node: Option<NodeId>,
    next_node_id:   u32,
    next_edge_id:   u32,
}

impl ShaderGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name:         name.into(),
            nodes:        HashMap::new(),
            edges:        Vec::new(),
            parameters:   Vec::new(),
            output_node:  None,
            next_node_id: 0,
            next_edge_id: 0,
        }
    }

    // ── Node management ────────────────────────────────────────────────────────

    pub fn add_node(&mut self, node_type: NodeType) -> NodeId {
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        self.nodes.insert(id, ShaderNode::new(id, node_type));
        id
    }

    pub fn add_node_at(&mut self, node_type: NodeType, x: f32, y: f32) -> NodeId {
        let id = self.add_node(node_type);
        if let Some(n) = self.nodes.get_mut(&id) {
            n.editor_x = x;
            n.editor_y = y;
        }
        id
    }

    pub fn remove_node(&mut self, id: NodeId) -> bool {
        if self.nodes.remove(&id).is_some() {
            self.edges.retain(|e| e.from_node != id && e.to_node != id);
            if self.output_node == Some(id) { self.output_node = None; }
            true
        } else {
            false
        }
    }

    pub fn node(&self, id: NodeId) -> Option<&ShaderNode> {
        self.nodes.get(&id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut ShaderNode> {
        self.nodes.get_mut(&id)
    }

    pub fn set_output(&mut self, id: NodeId) {
        self.output_node = Some(id);
    }

    // ── Edge management ────────────────────────────────────────────────────────

    pub fn connect(
        &mut self,
        from_node: NodeId, from_slot: u8,
        to_node:   NodeId, to_slot:   u8,
    ) -> Result<EdgeId, GraphError> {
        // Validate nodes exist
        if !self.nodes.contains_key(&from_node) {
            return Err(GraphError::NodeNotFound(from_node));
        }
        if !self.nodes.contains_key(&to_node) {
            return Err(GraphError::NodeNotFound(to_node));
        }
        // Prevent duplicate connections to same input slot
        if self.edges.iter().any(|e| e.to_node == to_node && e.to_slot == to_slot) {
            return Err(GraphError::SlotAlreadyConnected { node: to_node, slot: to_slot });
        }
        // Prevent cycles (simple reachability check)
        if self.would_create_cycle(from_node, to_node) {
            return Err(GraphError::CycleDetected);
        }
        let id = EdgeId(self.next_edge_id);
        self.next_edge_id += 1;
        self.edges.push(ShaderEdge { id, from_node, from_slot, to_node, to_slot });
        Ok(id)
    }

    pub fn disconnect(&mut self, edge_id: EdgeId) -> bool {
        let before = self.edges.len();
        self.edges.retain(|e| e.id != edge_id);
        self.edges.len() < before
    }

    pub fn disconnect_input(&mut self, to_node: NodeId, to_slot: u8) {
        self.edges.retain(|e| !(e.to_node == to_node && e.to_slot == to_slot));
    }

    // ── Parameter management ───────────────────────────────────────────────────

    pub fn add_parameter(&mut self, param: ShaderParameter) -> usize {
        let idx = self.parameters.len();
        self.parameters.push(param);
        idx
    }

    pub fn set_parameter_float(&mut self, name: &str, value: f32) {
        for p in &mut self.parameters {
            if p.name == name {
                p.value = ParameterValue::Float(value.clamp(p.min, p.max));
                break;
            }
        }
    }

    /// Update animated parameters by evaluating their MathFunction drivers.
    pub fn update_parameters(&mut self, time: f32) {
        for p in &mut self.parameters {
            if let Some(ref func) = p.driver {
                let v = func.evaluate(time, 0.0).clamp(p.min, p.max);
                p.value = ParameterValue::Float(v);
            }
        }
    }

    // ── Compilation ────────────────────────────────────────────────────────────

    /// Compile the graph to GLSL. Returns an error if the graph is invalid.
    pub fn compile(&self) -> Result<CompiledShader, GraphError> {
        let optimized = GraphOptimizer::run(self);
        compiler::GraphCompiler::compile(&optimized)
    }

    /// Validate graph structure without compiling.
    pub fn validate(&self) -> Vec<GraphError> {
        let mut errors = Vec::new();
        if self.output_node.is_none() {
            errors.push(GraphError::NoOutputNode);
        }
        if let Some(out) = self.output_node {
            if !self.nodes.contains_key(&out) {
                errors.push(GraphError::NodeNotFound(out));
            }
        }
        // Check for disconnected required inputs
        for (id, node) in &self.nodes {
            for (slot, sock) in node.node_type.input_sockets().iter().enumerate() {
                if sock.required {
                    let connected = self.edges.iter()
                        .any(|e| e.to_node == *id && e.to_slot == slot as u8);
                    if !connected && node.constant_inputs.get(&slot).is_none() {
                        errors.push(GraphError::RequiredInputDisconnected {
                            node: *id, slot: slot as u8,
                        });
                    }
                }
            }
        }
        errors
    }

    // ── Topological sort ───────────────────────────────────────────────────────

    /// Returns nodes in evaluation order (inputs before outputs).
    pub fn topological_order(&self) -> Result<Vec<NodeId>, GraphError> {
        let mut visited = std::collections::HashSet::new();
        let mut order   = Vec::new();

        fn visit(
            id: NodeId,
            graph: &ShaderGraph,
            visited: &mut std::collections::HashSet<NodeId>,
            order:   &mut Vec<NodeId>,
            stack:   &mut std::collections::HashSet<NodeId>,
        ) -> Result<(), GraphError> {
            if stack.contains(&id) { return Err(GraphError::CycleDetected); }
            if visited.contains(&id) { return Ok(()); }
            stack.insert(id);
            // Visit all nodes feeding into this one
            for edge in graph.edges.iter().filter(|e| e.to_node == id) {
                visit(edge.from_node, graph, visited, order, stack)?;
            }
            stack.remove(&id);
            visited.insert(id);
            order.push(id);
            Ok(())
        }

        let mut stack = std::collections::HashSet::new();
        if let Some(out) = self.output_node {
            visit(out, self, &mut visited, &mut order, &mut stack)?;
        } else {
            // Visit all nodes if no output set
            let ids: Vec<NodeId> = self.nodes.keys().copied().collect();
            for id in ids {
                visit(id, self, &mut visited, &mut order, &mut stack)?;
            }
        }
        Ok(order)
    }

    fn would_create_cycle(&self, from: NodeId, to: NodeId) -> bool {
        // DFS from `to` — if we can reach `from`, adding from→to creates a cycle
        let mut visited = std::collections::HashSet::new();
        let mut stack   = vec![to];
        while let Some(cur) = stack.pop() {
            if cur == from { return true; }
            if visited.insert(cur) {
                for e in self.edges.iter().filter(|e| e.from_node == cur) {
                    stack.push(e.to_node);
                }
            }
        }
        false
    }

    // ── Serialization ──────────────────────────────────────────────────────────

    pub fn to_toml(&self) -> String {
        let mut out = format!("[graph]\nname = {:?}\n\n", self.name);
        for (id, node) in &self.nodes {
            out.push_str(&format!(
                "[[nodes]]\nid = {}\ntype = {:?}\nx = {:.1}\ny = {:.1}\n\n",
                id.0, node.node_type.label(), node.editor_x, node.editor_y
            ));
        }
        for edge in &self.edges {
            out.push_str(&format!(
                "[[edges]]\nfrom = {}\nfrom_slot = {}\nto = {}\nto_slot = {}\n\n",
                edge.from_node.0, edge.from_slot, edge.to_node.0, edge.to_slot
            ));
        }
        out
    }

    /// Statistics about the graph.
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            node_count:      self.nodes.len(),
            edge_count:      self.edges.len(),
            parameter_count: self.parameters.len(),
        }
    }
}

#[derive(Debug)]
pub struct GraphStats {
    pub node_count:      usize,
    pub edge_count:      usize,
    pub parameter_count: usize,
}

// ── GraphError ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    NodeNotFound(NodeId),
    CycleDetected,
    NoOutputNode,
    SlotAlreadyConnected { node: NodeId, slot: u8 },
    RequiredInputDisconnected { node: NodeId, slot: u8 },
    TypeMismatch { from: SocketType, to: SocketType },
    CompileError(String),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::NodeNotFound(id)         => write!(f, "Node {:?} not found", id),
            GraphError::CycleDetected            => write!(f, "Graph contains a cycle"),
            GraphError::NoOutputNode             => write!(f, "No output node set"),
            GraphError::SlotAlreadyConnected { node, slot } =>
                write!(f, "Node {:?} slot {} already has an incoming connection", node, slot),
            GraphError::RequiredInputDisconnected { node, slot } =>
                write!(f, "Node {:?} required slot {} is not connected", node, slot),
            GraphError::TypeMismatch { from, to } =>
                write!(f, "Type mismatch: {:?} -> {:?}", from, to),
            GraphError::CompileError(msg)        => write!(f, "Compile error: {}", msg),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nodes::NodeType;

    #[test]
    fn test_add_remove_node() {
        let mut g = ShaderGraph::new("test");
        let id = g.add_node(NodeType::UvCoord);
        assert!(g.node(id).is_some());
        assert!(g.remove_node(id));
        assert!(g.node(id).is_none());
    }

    #[test]
    fn test_connect_nodes() {
        let mut g = ShaderGraph::new("test");
        let uv   = g.add_node(NodeType::UvCoord);
        let out  = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let result = g.connect(uv, 0, out, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cycle_detection() {
        let mut g  = ShaderGraph::new("test");
        let a = g.add_node(NodeType::Add);
        let b = g.add_node(NodeType::Add);
        let _ = g.connect(a, 0, b, 0);
        let result = g.connect(b, 0, a, 0);
        assert_eq!(result, Err(GraphError::CycleDetected));
    }

    #[test]
    fn test_duplicate_input_rejected() {
        let mut g  = ShaderGraph::new("test");
        let src1 = g.add_node(NodeType::ConstFloat(1.0));
        let src2 = g.add_node(NodeType::ConstFloat(2.0));
        let dst  = g.add_node(NodeType::Add);
        let _ = g.connect(src1, 0, dst, 0);
        let r = g.connect(src2, 0, dst, 0);
        assert!(matches!(r, Err(GraphError::SlotAlreadyConnected { .. })));
    }

    #[test]
    fn test_topological_order() {
        let mut g   = ShaderGraph::new("test");
        let uv  = g.add_node(NodeType::UvCoord);
        let sin = g.add_node(NodeType::SineWave);
        let out = g.add_node(NodeType::OutputColor);
        g.set_output(out);
        let _ = g.connect(uv, 0, sin, 0);
        let _ = g.connect(sin, 0, out, 0);
        let order = g.topological_order().unwrap();
        assert_eq!(order[0], uv);
        assert_eq!(order[1], sin);
        assert_eq!(order[2], out);
    }

    #[test]
    fn test_parameter_update() {
        let mut g = ShaderGraph::new("test");
        g.add_parameter(ShaderParameter {
            name:      "brightness".to_string(),
            glsl_name: "u_brightness".to_string(),
            value:     ParameterValue::Float(0.5),
            driver:    None,
            min:       0.0,
            max:       2.0,
        });
        g.set_parameter_float("brightness", 1.5);
        assert_eq!(g.parameters[0].value.as_float(), Some(1.5));
    }

    #[test]
    fn test_stats() {
        let mut g = ShaderGraph::new("test");
        g.add_node(NodeType::UvCoord);
        g.add_node(NodeType::OutputColor);
        let s = g.stats();
        assert_eq!(s.node_count, 2);
    }
}
