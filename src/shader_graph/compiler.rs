//! Shader graph compiler: topological sort, dead-node elimination, constant folding,
//! common subexpression elimination, and GLSL code generation.

use std::collections::{HashMap, HashSet, VecDeque};
use super::nodes::{
    Connection, DataType, GlslSnippet, NodeId, NodeType, ParamValue, ShaderGraph, ShaderNode,
};

// ---------------------------------------------------------------------------
// Compilation options
// ---------------------------------------------------------------------------

/// Options controlling the compilation process.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// If true, run dead-node elimination (remove unreachable from outputs).
    pub dead_node_elimination: bool,
    /// If true, evaluate constant subtrees at compile time.
    pub constant_folding: bool,
    /// If true, merge common subexpressions.
    pub common_subexpression_elimination: bool,
    /// If true, include comments in generated GLSL for debugging.
    pub debug_comments: bool,
    /// GLSL version string (e.g., "330 core", "300 es").
    pub glsl_version: String,
    /// If true, generate conditional branches for nodes with conditions.
    pub enable_conditionals: bool,
    /// If true, generate animated uniform declarations for time-dependent parameters.
    pub animated_uniforms: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            dead_node_elimination: true,
            constant_folding: true,
            common_subexpression_elimination: true,
            debug_comments: false,
            glsl_version: "330 core".to_string(),
            enable_conditionals: true,
            animated_uniforms: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Compile errors
// ---------------------------------------------------------------------------

/// Errors that can occur during shader graph compilation.
#[derive(Debug, Clone)]
pub enum CompileError {
    /// The graph contains a cycle, making topological sort impossible.
    CycleDetected(Vec<NodeId>),
    /// A required input socket has no connection and no default value.
    MissingInput { node_id: NodeId, socket_index: usize, socket_name: String },
    /// The graph has no output nodes.
    NoOutputNodes,
    /// A type mismatch between connected sockets.
    TypeMismatch {
        from_node: NodeId,
        from_socket: usize,
        from_type: DataType,
        to_node: NodeId,
        to_socket: usize,
        to_type: DataType,
    },
    /// Graph validation failed.
    ValidationErrors(Vec<String>),
    /// Internal compiler error.
    Internal(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::CycleDetected(ids) => {
                write!(f, "Cycle detected involving nodes: {:?}",
                    ids.iter().map(|id| id.0).collect::<Vec<_>>())
            }
            CompileError::MissingInput { node_id, socket_index, socket_name } => {
                write!(f, "Node {} missing input at socket {} ('{}')",
                    node_id.0, socket_index, socket_name)
            }
            CompileError::NoOutputNodes => write!(f, "Graph has no output nodes"),
            CompileError::TypeMismatch { from_node, from_socket, from_type, to_node, to_socket, to_type } => {
                write!(f, "Type mismatch: node {}:{} ({}) -> node {}:{} ({})",
                    from_node.0, from_socket, from_type,
                    to_node.0, to_socket, to_type)
            }
            CompileError::ValidationErrors(errs) => {
                write!(f, "Validation errors: {}", errs.join("; "))
            }
            CompileError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

// ---------------------------------------------------------------------------
// Compiled shader output
// ---------------------------------------------------------------------------

/// The result of compiling a shader graph.
#[derive(Debug, Clone)]
pub struct CompiledShader {
    /// The generated GLSL fragment shader source.
    pub fragment_source: String,
    /// The generated GLSL vertex shader source (boilerplate).
    pub vertex_source: String,
    /// All uniform declarations needed.
    pub uniforms: Vec<UniformDecl>,
    /// All varying declarations needed.
    pub varyings: Vec<VaryingDecl>,
    /// Estimated total instruction count.
    pub instruction_count: u32,
    /// Number of texture samplers used.
    pub sampler_count: u32,
    /// Number of nodes after dead-node elimination.
    pub live_node_count: usize,
    /// Topology hash for caching.
    pub topology_hash: u64,
    /// Nodes in topological order after all optimizations.
    pub node_order: Vec<NodeId>,
    /// Map from node output (node_id, socket_index) to GLSL variable name.
    pub output_var_map: HashMap<(u64, usize), String>,
}

/// A uniform variable declaration.
#[derive(Debug, Clone)]
pub struct UniformDecl {
    pub name: String,
    pub data_type: DataType,
    pub default_value: Option<ParamValue>,
    pub is_animated: bool,
}

/// A varying variable declaration.
#[derive(Debug, Clone)]
pub struct VaryingDecl {
    pub name: String,
    pub data_type: DataType,
}

// ---------------------------------------------------------------------------
// Shader Compiler
// ---------------------------------------------------------------------------

/// The main shader graph compiler.
pub struct ShaderCompiler {
    options: CompileOptions,
}

impl ShaderCompiler {
    pub fn new(options: CompileOptions) -> Self {
        Self { options }
    }

    pub fn with_defaults() -> Self {
        Self::new(CompileOptions::default())
    }

    /// Compile a shader graph into GLSL source code.
    pub fn compile(&self, graph: &ShaderGraph) -> Result<CompiledShader, CompileError> {
        // Step 0: Validate
        let errors = graph.validate();
        if !errors.is_empty() {
            return Err(CompileError::ValidationErrors(errors));
        }

        // Step 1: Find output nodes
        let output_nodes = graph.output_nodes();
        if output_nodes.is_empty() {
            return Err(CompileError::NoOutputNodes);
        }

        // Step 2: Dead node elimination — find all nodes reachable from outputs
        let live_nodes = if self.options.dead_node_elimination {
            self.find_live_nodes(graph, &output_nodes)
        } else {
            graph.node_ids().collect()
        };

        // Step 3: Topological sort of live nodes
        let sorted = self.topological_sort(graph, &live_nodes)?;

        // Step 4: Constant folding
        let folded_values = if self.options.constant_folding {
            self.constant_fold(graph, &sorted)
        } else {
            HashMap::new()
        };

        // Step 5: Common subexpression elimination
        let cse_map = if self.options.common_subexpression_elimination {
            self.find_common_subexpressions(graph, &sorted)
        } else {
            HashMap::new()
        };

        // Step 6: Collect uniforms and varyings
        let (uniforms, varyings) = self.collect_declarations(graph, &sorted);

        // Step 7: Generate GLSL
        let (fragment_source, output_var_map) = self.generate_glsl(
            graph, &sorted, &folded_values, &cse_map, &uniforms, &varyings,
        );

        // Step 8: Generate vertex shader
        let vertex_source = self.generate_vertex_shader(&varyings);

        // Step 9: Compute stats
        let instruction_count: u32 = sorted.iter()
            .filter_map(|id| graph.node(*id).map(|n| n.estimated_cost()))
            .sum();
        let sampler_count = uniforms.iter()
            .filter(|u| u.data_type == DataType::Sampler2D)
            .count() as u32;

        Ok(CompiledShader {
            fragment_source,
            vertex_source,
            uniforms,
            varyings,
            instruction_count,
            sampler_count,
            live_node_count: sorted.len(),
            topology_hash: graph.topology_hash(),
            node_order: sorted,
            output_var_map,
        })
    }

    // -----------------------------------------------------------------------
    // Dead node elimination
    // -----------------------------------------------------------------------

    /// Walk backwards from output nodes, collecting all reachable node IDs.
    fn find_live_nodes(&self, graph: &ShaderGraph, outputs: &[NodeId]) -> HashSet<NodeId> {
        let mut live = HashSet::new();
        let mut queue: VecDeque<NodeId> = outputs.iter().copied().collect();

        while let Some(node_id) = queue.pop_front() {
            if !live.insert(node_id) {
                continue; // already visited
            }
            // Walk incoming connections
            for conn in graph.connections() {
                if conn.to_node == node_id && !live.contains(&conn.from_node) {
                    queue.push_back(conn.from_node);
                }
            }
        }

        live
    }

    // -----------------------------------------------------------------------
    // Topological sort
    // -----------------------------------------------------------------------

    /// Kahn's algorithm for topological sorting. Returns sorted node IDs or a cycle error.
    fn topological_sort(
        &self,
        graph: &ShaderGraph,
        live_nodes: &HashSet<NodeId>,
    ) -> Result<Vec<NodeId>, CompileError> {
        // Build adjacency and in-degree maps considering only live nodes
        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        for &nid in live_nodes {
            in_degree.entry(nid).or_insert(0);
            adjacency.entry(nid).or_insert_with(Vec::new);
        }

        for conn in graph.connections() {
            if live_nodes.contains(&conn.from_node) && live_nodes.contains(&conn.to_node) {
                adjacency.entry(conn.from_node).or_insert_with(Vec::new).push(conn.to_node);
                *in_degree.entry(conn.to_node).or_insert(0) += 1;
            }
        }

        // Start with all nodes that have zero in-degree
        let mut queue: VecDeque<NodeId> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        // Sort the queue for deterministic output
        let mut queue_vec: Vec<NodeId> = queue.drain(..).collect();
        queue_vec.sort_by_key(|id| id.0);
        queue = queue_vec.into_iter().collect();

        let mut sorted = Vec::new();

        while let Some(node_id) = queue.pop_front() {
            sorted.push(node_id);
            if let Some(neighbors) = adjacency.get(&node_id) {
                let mut next_neighbors: Vec<NodeId> = Vec::new();
                for &neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(&neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            next_neighbors.push(neighbor);
                        }
                    }
                }
                next_neighbors.sort_by_key(|id| id.0);
                for n in next_neighbors {
                    queue.push_back(n);
                }
            }
        }

        if sorted.len() != live_nodes.len() {
            // Cycle detected — find participating nodes
            let sorted_set: HashSet<NodeId> = sorted.iter().copied().collect();
            let cycle_nodes: Vec<NodeId> = live_nodes.iter()
                .filter(|id| !sorted_set.contains(id))
                .copied()
                .collect();
            return Err(CompileError::CycleDetected(cycle_nodes));
        }

        Ok(sorted)
    }

    // -----------------------------------------------------------------------
    // Constant folding
    // -----------------------------------------------------------------------

    /// Identify nodes whose inputs are all constant (literal defaults or other folded nodes)
    /// and evaluate them at compile time.
    fn constant_fold(
        &self,
        graph: &ShaderGraph,
        sorted: &[NodeId],
    ) -> HashMap<NodeId, Vec<ParamValue>> {
        let mut folded: HashMap<NodeId, Vec<ParamValue>> = HashMap::new();

        for &node_id in sorted {
            let node = match graph.node(node_id) {
                Some(n) => n,
                None => continue,
            };

            if !node.node_type.is_pure_math() {
                continue;
            }

            // Check if all inputs are constants
            let incoming = graph.incoming_connections(node_id);
            let mut input_values: Vec<Option<ParamValue>> = Vec::new();
            let mut all_constant = true;

            for (idx, socket) in node.inputs.iter().enumerate() {
                // Find connection to this socket
                let conn = incoming.iter().find(|c| c.to_socket == idx);
                if let Some(c) = conn {
                    // Check if source is folded
                    if let Some(folded_vals) = folded.get(&c.from_node) {
                        if c.from_socket < folded_vals.len() {
                            input_values.push(Some(folded_vals[c.from_socket].clone()));
                            continue;
                        }
                    }
                    all_constant = false;
                    break;
                } else if let Some(def) = &socket.default_value {
                    input_values.push(Some(def.clone()));
                } else {
                    all_constant = false;
                    break;
                }
            }

            if !all_constant {
                continue;
            }

            // Try to evaluate
            let values: Vec<ParamValue> = input_values.into_iter().filter_map(|v| v).collect();
            if let Some(result) = self.evaluate_constant(&node.node_type, &values) {
                folded.insert(node_id, result);
            }
        }

        folded
    }

    /// Evaluate a pure-math node with constant inputs.
    fn evaluate_constant(&self, node_type: &NodeType, inputs: &[ParamValue]) -> Option<Vec<ParamValue>> {
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
            NodeType::Mod => {
                let x = inputs.first()?.as_float()?;
                let y = inputs.get(1)?.as_float()?;
                if y.abs() < 1e-10 { return None; }
                Some(vec![ParamValue::Float(x % y)])
            }
            NodeType::Pow => {
                let base = inputs.first()?.as_float()?;
                let exp = inputs.get(1)?.as_float()?;
                Some(vec![ParamValue::Float(base.max(0.0).powf(exp))])
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
            NodeType::Tan => {
                let x = inputs.first()?.as_float()?;
                Some(vec![ParamValue::Float(x.tan())])
            }
            NodeType::Atan2 => {
                let y = inputs.first()?.as_float()?;
                let x = inputs.get(1)?.as_float()?;
                Some(vec![ParamValue::Float(y.atan2(x))])
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
            NodeType::Smoothstep => {
                let e0 = inputs.first()?.as_float()?;
                let e1 = inputs.get(1)?.as_float()?;
                let x = inputs.get(2)?.as_float()?;
                let range = e1 - e0;
                if range.abs() < 1e-10 {
                    return Some(vec![ParamValue::Float(if x < e0 { 0.0 } else { 1.0 })]);
                }
                let t = ((x - e0) / range).clamp(0.0, 1.0);
                Some(vec![ParamValue::Float(t * t * (3.0 - 2.0 * t))])
            }
            NodeType::Remap => {
                let x = inputs.first()?.as_float()?;
                let in_min = inputs.get(1)?.as_float()?;
                let in_max = inputs.get(2)?.as_float()?;
                let out_min = inputs.get(3)?.as_float()?;
                let out_max = inputs.get(4)?.as_float()?;
                let range = in_max - in_min;
                if range.abs() < 1e-10 { return None; }
                let t = (x - in_min) / range;
                Some(vec![ParamValue::Float(out_min + (out_max - out_min) * t)])
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
            NodeType::Posterize => {
                let c = inputs.first()?.as_vec3()?;
                let levels = inputs.get(1)?.as_float()?;
                if levels < 1.0 { return None; }
                Some(vec![ParamValue::Vec3([
                    (c[0] * levels).floor() / levels,
                    (c[1] * levels).floor() / levels,
                    (c[2] * levels).floor() / levels,
                ])])
            }
            NodeType::Contrast => {
                let c = inputs.first()?.as_vec3()?;
                let amount = inputs.get(1)?.as_float()?;
                Some(vec![ParamValue::Vec3([
                    (c[0] - 0.5) * amount + 0.5,
                    (c[1] - 0.5) * amount + 0.5,
                    (c[2] - 0.5) * amount + 0.5,
                ])])
            }
            NodeType::Saturation => {
                let c = inputs.first()?.as_vec3()?;
                let amount = inputs.get(1)?.as_float()?;
                let lum = c[0] * 0.299 + c[1] * 0.587 + c[2] * 0.114;
                Some(vec![ParamValue::Vec3([
                    lum + (c[0] - lum) * amount,
                    lum + (c[1] - lum) * amount,
                    lum + (c[2] - lum) * amount,
                ])])
            }
            _ => None, // Not implemented for this node type
        }
    }

    // -----------------------------------------------------------------------
    // Common subexpression elimination
    // -----------------------------------------------------------------------

    /// Identify nodes that produce identical results and map duplicates to the canonical version.
    fn find_common_subexpressions(
        &self,
        graph: &ShaderGraph,
        sorted: &[NodeId],
    ) -> HashMap<NodeId, NodeId> {
        let mut cse_map: HashMap<NodeId, NodeId> = HashMap::new();
        // Signature: (node_type_name, inputs_signature) -> canonical node ID
        let mut signatures: HashMap<String, NodeId> = HashMap::new();

        for &node_id in sorted {
            let node = match graph.node(node_id) {
                Some(n) => n,
                None => continue,
            };

            // Build signature
            let incoming = graph.incoming_connections(node_id);
            let mut sig_parts: Vec<String> = vec![node.node_type.display_name().to_string()];

            for (idx, socket) in node.inputs.iter().enumerate() {
                let conn = incoming.iter().find(|c| c.to_socket == idx);
                if let Some(c) = conn {
                    // Resolve through CSE map
                    let resolved = cse_map.get(&c.from_node).copied().unwrap_or(c.from_node);
                    sig_parts.push(format!("c{}:{}", resolved.0, c.from_socket));
                } else if let Some(def) = &socket.default_value {
                    sig_parts.push(format!("d:{}", def.to_glsl()));
                } else {
                    sig_parts.push("none".to_string());
                }
            }

            let signature = sig_parts.join("|");

            if let Some(&canonical) = signatures.get(&signature) {
                cse_map.insert(node_id, canonical);
            } else {
                signatures.insert(signature, node_id);
            }
        }

        cse_map
    }

    // -----------------------------------------------------------------------
    // Declaration collection
    // -----------------------------------------------------------------------

    fn collect_declarations(
        &self,
        graph: &ShaderGraph,
        sorted: &[NodeId],
    ) -> (Vec<UniformDecl>, Vec<VaryingDecl>) {
        let mut uniforms: Vec<UniformDecl> = Vec::new();
        let mut uniform_names: HashSet<String> = HashSet::new();
        let mut varyings: Vec<VaryingDecl> = Vec::new();
        let mut varying_names: HashSet<String> = HashSet::new();

        // Always include standard uniforms
        let standard_uniforms = vec![
            ("u_time", DataType::Float, true),
            ("u_model", DataType::Mat4, false),
            ("u_view", DataType::Mat4, false),
            ("u_projection", DataType::Mat4, false),
            ("u_camera_pos", DataType::Vec3, false),
            ("u_inv_model", DataType::Mat4, false),
        ];
        for (name, dt, animated) in standard_uniforms {
            if uniform_names.insert(name.to_string()) {
                uniforms.push(UniformDecl {
                    name: name.to_string(),
                    data_type: dt,
                    default_value: None,
                    is_animated: animated,
                });
            }
        }

        // Standard varyings
        let standard_varyings = vec![
            ("v_position", DataType::Vec3),
            ("v_normal", DataType::Vec3),
            ("v_uv", DataType::Vec2),
        ];
        for (name, dt) in standard_varyings {
            if varying_names.insert(name.to_string()) {
                varyings.push(VaryingDecl { name: name.to_string(), data_type: dt });
            }
        }

        for &node_id in sorted {
            let node = match graph.node(node_id) {
                Some(n) => n,
                None => continue,
            };

            match &node.node_type {
                NodeType::Texture => {
                    // Add sampler uniform
                    let sampler_idx = node.inputs.get(1)
                        .and_then(|s| s.default_value.as_ref())
                        .and_then(|v| v.as_int())
                        .unwrap_or(0);
                    let name = format!("u_texture{}", sampler_idx);
                    if uniform_names.insert(name.clone()) {
                        uniforms.push(UniformDecl {
                            name,
                            data_type: DataType::Sampler2D,
                            default_value: None,
                            is_animated: false,
                        });
                    }
                }
                NodeType::GameStateVar => {
                    // Add game state uniform
                    let var_name = node.inputs.first()
                        .and_then(|s| s.default_value.as_ref())
                        .and_then(|v| v.as_string())
                        .unwrap_or("game_var_0");
                    let name = format!("u_gs_{}", var_name);
                    if uniform_names.insert(name.clone()) {
                        uniforms.push(UniformDecl {
                            name,
                            data_type: DataType::Float,
                            default_value: Some(ParamValue::Float(0.0)),
                            is_animated: false,
                        });
                    }
                }
                _ => {}
            }

            // Conditional node uniforms
            if let Some(ref var_name) = node.conditional_var {
                let name = format!("u_gs_{}", var_name);
                if uniform_names.insert(name.clone()) {
                    uniforms.push(UniformDecl {
                        name,
                        data_type: DataType::Float,
                        default_value: Some(ParamValue::Float(0.0)),
                        is_animated: false,
                    });
                }
            }

            // Check properties for any that need uniform binding
            for (key, val) in &node.properties {
                if key.starts_with("uniform_") {
                    let name = format!("u_prop_{}_{}", node.id.0, key.trim_start_matches("uniform_"));
                    if uniform_names.insert(name.clone()) {
                        uniforms.push(UniformDecl {
                            name,
                            data_type: val.data_type(),
                            default_value: Some(val.clone()),
                            is_animated: self.options.animated_uniforms,
                        });
                    }
                }
            }
        }

        (uniforms, varyings)
    }

    // -----------------------------------------------------------------------
    // GLSL code generation
    // -----------------------------------------------------------------------

    fn generate_glsl(
        &self,
        graph: &ShaderGraph,
        sorted: &[NodeId],
        folded: &HashMap<NodeId, Vec<ParamValue>>,
        cse_map: &HashMap<NodeId, NodeId>,
        uniforms: &[UniformDecl],
        varyings: &[VaryingDecl],
    ) -> (String, HashMap<(u64, usize), String>) {
        let mut code = String::new();
        let mut output_var_map: HashMap<(u64, usize), String> = HashMap::new();

        // Header
        code.push_str(&format!("#version {}\n", self.options.glsl_version));
        code.push_str("precision highp float;\n\n");

        // Uniform declarations
        for u in uniforms {
            code.push_str(&format!("uniform {} {};\n", u.data_type, u.name));
        }
        code.push('\n');

        // Varying declarations
        for v in varyings {
            code.push_str(&format!("in {} {};\n", v.data_type, v.name));
        }
        code.push('\n');

        // Output declarations for MRT
        code.push_str("layout(location = 0) out vec4 fragColor;\n");
        code.push_str("layout(location = 1) out vec4 fragEmission;\n");
        code.push_str("layout(location = 2) out vec4 fragBloom;\n");
        code.push_str("layout(location = 3) out vec4 fragNormal;\n");
        code.push('\n');

        // Main function
        code.push_str("void main() {\n");

        // Track which CSE nodes have already been emitted
        let mut emitted_cse: HashSet<NodeId> = HashSet::new();

        for &node_id in sorted {
            // If this node is a CSE duplicate, skip it but register its output vars
            if let Some(&canonical) = cse_map.get(&node_id) {
                // Map this node's outputs to the canonical node's outputs
                if let Some(node) = graph.node(node_id) {
                    for (idx, _) in node.outputs.iter().enumerate() {
                        if let Some(var) = output_var_map.get(&(canonical.0, idx)) {
                            output_var_map.insert((node_id.0, idx), var.clone());
                        }
                    }
                }
                continue;
            }

            let node = match graph.node(node_id) {
                Some(n) => n,
                None => continue,
            };

            if !node.enabled {
                continue;
            }

            // Handle constant-folded nodes
            if let Some(folded_vals) = folded.get(&node_id) {
                if self.options.debug_comments {
                    code.push_str(&format!("  // [FOLDED] {} (node {})\n",
                        node.node_type.display_name(), node_id.0));
                }
                for (idx, val) in folded_vals.iter().enumerate() {
                    let var_name = format!("n{}_{}", node_id.0, idx);
                    code.push_str(&format!("  {} {} = {};\n",
                        val.data_type(), var_name, val.to_glsl()));
                    output_var_map.insert((node_id.0, idx), var_name);
                }
                continue;
            }

            // Debug comment
            if self.options.debug_comments {
                code.push_str(&format!("  // {} (node {})\n",
                    node.node_type.display_name(), node_id.0));
            }

            // Conditional open
            let has_condition = self.options.enable_conditionals && node.conditional_var.is_some();
            if has_condition {
                let var_name = node.conditional_var.as_ref().unwrap();
                code.push_str(&format!("  if (u_gs_{} > {}) {{\n",
                    var_name, format_float_glsl(node.conditional_threshold)));
            }

            // Resolve input variables
            let incoming = graph.incoming_connections(node_id);
            let mut input_vars: Vec<String> = Vec::new();
            for (idx, socket) in node.inputs.iter().enumerate() {
                let conn = incoming.iter().find(|c| c.to_socket == idx);
                if let Some(c) = conn {
                    let resolved_from = cse_map.get(&c.from_node).copied().unwrap_or(c.from_node);
                    if let Some(var) = output_var_map.get(&(resolved_from.0, c.from_socket)) {
                        input_vars.push(var.clone());
                    } else {
                        // Fallback: use default
                        input_vars.push(socket.default_value.as_ref()
                            .map(|v| v.to_glsl())
                            .unwrap_or_default());
                    }
                } else {
                    input_vars.push(String::new());
                }
            }

            // Generate GLSL for this node
            let prefix = node.var_prefix();
            let snippet = node.node_type.generate_glsl(&prefix, &input_vars);

            let indent = if has_condition { "    " } else { "  " };
            for line in &snippet.lines {
                code.push_str(&format!("{}{}\n", indent, line));
            }

            // Register output variables
            for (idx, var) in snippet.output_vars.iter().enumerate() {
                output_var_map.insert((node_id.0, idx), var.clone());
            }

            let _ = emitted_cse.insert(node_id);

            // Conditional close
            if has_condition {
                code.push_str("  }\n");
            }
        }

        code.push_str("}\n");

        (code, output_var_map)
    }

    fn generate_vertex_shader(&self, varyings: &[VaryingDecl]) -> String {
        let mut code = String::new();
        code.push_str(&format!("#version {}\n", self.options.glsl_version));
        code.push_str("precision highp float;\n\n");

        // Vertex attributes
        code.push_str("layout(location = 0) in vec3 a_position;\n");
        code.push_str("layout(location = 1) in vec3 a_normal;\n");
        code.push_str("layout(location = 2) in vec2 a_uv;\n\n");

        // Uniforms
        code.push_str("uniform mat4 u_model;\n");
        code.push_str("uniform mat4 u_view;\n");
        code.push_str("uniform mat4 u_projection;\n\n");

        // Varyings
        for v in varyings {
            code.push_str(&format!("out {} {};\n", v.data_type, v.name));
        }
        code.push('\n');

        code.push_str("void main() {\n");
        code.push_str("  vec4 world_pos = u_model * vec4(a_position, 1.0);\n");
        code.push_str("  v_position = world_pos.xyz;\n");
        code.push_str("  v_normal = normalize((u_model * vec4(a_normal, 0.0)).xyz);\n");
        code.push_str("  v_uv = a_uv;\n");
        code.push_str("  gl_Position = u_projection * u_view * world_pos;\n");
        code.push_str("}\n");

        code
    }
}

fn format_float_glsl(v: f32) -> String {
    if v == v.floor() && v.abs() < 1e9 {
        format!("{:.1}", v)
    } else {
        format!("{}", v)
    }
}

// ---------------------------------------------------------------------------
// Convenience function
// ---------------------------------------------------------------------------

/// Compile a shader graph with default options.
pub fn compile_graph(graph: &ShaderGraph) -> Result<CompiledShader, CompileError> {
    ShaderCompiler::with_defaults().compile(graph)
}

/// Compile a shader graph with custom options.
pub fn compile_graph_with(graph: &ShaderGraph, options: CompileOptions) -> Result<CompiledShader, CompileError> {
    ShaderCompiler::new(options).compile(graph)
}

// ---------------------------------------------------------------------------
// Type compatibility checking
// ---------------------------------------------------------------------------

/// Check if a source type can be implicitly cast to a destination type.
pub fn types_compatible(from: DataType, to: DataType) -> bool {
    if from == to {
        return true;
    }
    // Implicit promotions
    matches!((from, to),
        (DataType::Float, DataType::Vec2)
        | (DataType::Float, DataType::Vec3)
        | (DataType::Float, DataType::Vec4)
        | (DataType::Int, DataType::Float)
        | (DataType::Bool, DataType::Float)
        | (DataType::Bool, DataType::Int)
    )
}

/// Generate GLSL cast expression from one type to another.
pub fn generate_cast(expr: &str, from: DataType, to: DataType) -> String {
    if from == to {
        return expr.to_string();
    }
    match (from, to) {
        (DataType::Float, DataType::Vec2) => format!("vec2({})", expr),
        (DataType::Float, DataType::Vec3) => format!("vec3({})", expr),
        (DataType::Float, DataType::Vec4) => format!("vec4({})", expr),
        (DataType::Int, DataType::Float) => format!("float({})", expr),
        (DataType::Bool, DataType::Float) => format!("float({})", expr),
        (DataType::Bool, DataType::Int) => format!("int({})", expr),
        (DataType::Vec2, DataType::Vec3) => format!("vec3({}, 0.0)", expr),
        (DataType::Vec2, DataType::Vec4) => format!("vec4({}, 0.0, 1.0)", expr),
        (DataType::Vec3, DataType::Vec4) => format!("vec4({}, 1.0)", expr),
        (DataType::Vec4, DataType::Vec3) => format!("{}.xyz", expr),
        (DataType::Vec3, DataType::Vec2) => format!("{}.xy", expr),
        (DataType::Vec4, DataType::Vec2) => format!("{}.xy", expr),
        (DataType::Vec3, DataType::Float) => format!("length({})", expr),
        (DataType::Vec4, DataType::Float) => format!("{}.x", expr),
        _ => format!("{}({})", to, expr), // best-effort
    }
}

// ---------------------------------------------------------------------------
// Shader variant cache
// ---------------------------------------------------------------------------

/// A cache for compiled shader variants, keyed by topology hash.
pub struct ShaderVariantCache {
    cache: HashMap<u64, CompiledShader>,
}

impl ShaderVariantCache {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    /// Get a cached shader by topology hash, or compile and cache it.
    pub fn get_or_compile(
        &mut self,
        graph: &ShaderGraph,
        compiler: &ShaderCompiler,
    ) -> Result<&CompiledShader, CompileError> {
        let hash = graph.topology_hash();
        if !self.cache.contains_key(&hash) {
            let compiled = compiler.compile(graph)?;
            self.cache.insert(hash, compiled);
        }
        Ok(self.cache.get(&hash).unwrap())
    }

    /// Invalidate a specific cache entry.
    pub fn invalidate(&mut self, hash: u64) {
        self.cache.remove(&hash);
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Number of cached variants.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for ShaderVariantCache {
    fn default() -> Self {
        Self::new()
    }
}
