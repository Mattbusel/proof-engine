//! Graph serialization: custom TOML-like text format for saving/loading shader graphs,
//! with node ID mapping, connection serialization, parameter type tags, and round-trip fidelity.

use std::collections::HashMap;
use super::nodes::{
    Connection, DataType, NodeId, NodeType, ParamValue, ShaderGraph, ShaderNode, Socket,
    SocketDirection,
};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during serialization or deserialization.
#[derive(Debug, Clone)]
pub enum SerializeError {
    /// A required field is missing.
    MissingField(String),
    /// A value could not be parsed.
    ParseError(String),
    /// An unknown node type was encountered.
    UnknownNodeType(String),
    /// The format is structurally invalid.
    FormatError(String),
    /// A referenced node ID does not exist.
    InvalidNodeId(u64),
    /// IO error message (since we can't use std::io::Error in Clone).
    IoError(String),
}

impl std::fmt::Display for SerializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializeError::MissingField(s) => write!(f, "Missing field: {}", s),
            SerializeError::ParseError(s) => write!(f, "Parse error: {}", s),
            SerializeError::UnknownNodeType(s) => write!(f, "Unknown node type: {}", s),
            SerializeError::FormatError(s) => write!(f, "Format error: {}", s),
            SerializeError::InvalidNodeId(id) => write!(f, "Invalid node ID: {}", id),
            SerializeError::IoError(s) => write!(f, "IO error: {}", s),
        }
    }
}

// ---------------------------------------------------------------------------
// GraphSerializer
// ---------------------------------------------------------------------------

/// Serializer/deserializer for shader graphs in a custom TOML-like text format.
///
/// ## Format
///
/// ```text
/// [graph]
/// name = "my_shader"
/// next_id = 42
///
/// [[node]]
/// id = 1
/// type = "Color"
/// label = "Base Color"
/// enabled = true
/// editor_x = 100.0
/// editor_y = 200.0
/// input.0.default = "vec4:1.0,0.0,0.0,1.0"
/// property.my_key = "float:0.5"
///
/// [[connection]]
/// from = 1:0
/// to = 3:1
/// ```
pub struct GraphSerializer;

impl GraphSerializer {
    /// Serialize a shader graph to the custom text format.
    pub fn serialize(graph: &ShaderGraph) -> String {
        let mut out = String::new();

        // Header
        out.push_str("[graph]\n");
        out.push_str(&format!("name = \"{}\"\n", escape_string(&graph.name)));
        out.push_str(&format!("next_id = {}\n", graph.next_id_counter()));
        out.push('\n');

        // Nodes (sorted by ID for determinism)
        let mut node_ids: Vec<NodeId> = graph.node_ids().collect();
        node_ids.sort_by_key(|id| id.0);

        for nid in &node_ids {
            let node = match graph.node(nid) {
                Some(n) => n,
                None => continue,
            };

            out.push_str("[[node]]\n");
            out.push_str(&format!("id = {}\n", node.id.0));
            out.push_str(&format!("type = \"{}\"\n", node_type_to_string(&node.node_type)));
            out.push_str(&format!("label = \"{}\"\n", escape_string(&node.label)));
            out.push_str(&format!("enabled = {}\n", node.enabled));
            out.push_str(&format!("editor_x = {}\n", format_f32(node.editor_x)));
            out.push_str(&format!("editor_y = {}\n", format_f32(node.editor_y)));

            // Conditional
            if let Some(ref var) = node.conditional_var {
                out.push_str(&format!("conditional_var = \"{}\"\n", escape_string(var)));
                out.push_str(&format!("conditional_threshold = {}\n",
                    format_f32(node.conditional_threshold)));
            }

            // Input defaults
            for (idx, socket) in node.inputs.iter().enumerate() {
                if let Some(ref val) = socket.default_value {
                    out.push_str(&format!("input.{}.default = \"{}\"\n",
                        idx, serialize_param_value(val)));
                }
            }

            // Properties
            let mut prop_keys: Vec<&String> = node.properties.keys().collect();
            prop_keys.sort();
            for key in prop_keys {
                let val = &node.properties[key];
                out.push_str(&format!("property.{} = \"{}\"\n",
                    escape_string(key), serialize_param_value(val)));
            }

            out.push('\n');
        }

        // Connections
        for conn in graph.connections() {
            out.push_str("[[connection]]\n");
            out.push_str(&format!("from = {}:{}\n", conn.from_node.0, conn.from_socket));
            out.push_str(&format!("to = {}:{}\n", conn.to_node.0, conn.to_socket));
            out.push('\n');
        }

        out
    }

    /// Deserialize a shader graph from the custom text format.
    pub fn deserialize(input: &str) -> Result<ShaderGraph, SerializeError> {
        let mut graph_name = String::from("untitled");
        let mut next_id: u64 = 1;
        let mut nodes: Vec<ShaderNode> = Vec::new();
        let mut connections: Vec<Connection> = Vec::new();

        let mut current_section = Section::None;
        let mut current_node: Option<NodeBuilder> = None;
        let mut current_conn: Option<ConnBuilder> = None;

        for (line_num, raw_line) in input.lines().enumerate() {
            let line = raw_line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Section headers
            if line == "[graph]" {
                flush_node(&mut current_node, &mut nodes)?;
                flush_conn(&mut current_conn, &mut connections)?;
                current_section = Section::Graph;
                continue;
            }
            if line == "[[node]]" {
                flush_node(&mut current_node, &mut nodes)?;
                flush_conn(&mut current_conn, &mut connections)?;
                current_section = Section::Node;
                current_node = Some(NodeBuilder::default());
                continue;
            }
            if line == "[[connection]]" {
                flush_node(&mut current_node, &mut nodes)?;
                flush_conn(&mut current_conn, &mut connections)?;
                current_section = Section::Connection;
                current_conn = Some(ConnBuilder::default());
                continue;
            }

            // Key-value pairs
            let (key, value) = parse_kv(line)
                .ok_or_else(|| SerializeError::FormatError(
                    format!("Invalid line {}: '{}'", line_num + 1, line)))?;

            match current_section {
                Section::Graph => {
                    match key.as_str() {
                        "name" => graph_name = unquote(&value),
                        "next_id" => next_id = value.parse().map_err(|e| {
                            SerializeError::ParseError(format!("next_id: {}", e))
                        })?,
                        _ => {} // ignore unknown keys
                    }
                }
                Section::Node => {
                    if let Some(ref mut nb) = current_node {
                        parse_node_field(nb, &key, &value)?;
                    }
                }
                Section::Connection => {
                    if let Some(ref mut cb) = current_conn {
                        parse_conn_field(cb, &key, &value)?;
                    }
                }
                Section::None => {
                    // Ignore lines before any section
                }
            }
        }

        // Flush remaining
        flush_node(&mut current_node, &mut nodes)?;
        flush_conn(&mut current_conn, &mut connections)?;

        // Build graph
        let mut graph = ShaderGraph::new(&graph_name);
        graph.set_next_id(next_id);

        for node in nodes {
            graph.insert_node(node);
        }
        for conn in connections {
            graph.add_connection_raw(conn);
        }

        Ok(graph)
    }

    /// Serialize a graph to a string, then deserialize it back.
    /// Returns the round-tripped graph. Useful for testing fidelity.
    pub fn round_trip(graph: &ShaderGraph) -> Result<ShaderGraph, SerializeError> {
        let serialized = Self::serialize(graph);
        Self::deserialize(&serialized)
    }

    /// Verify round-trip fidelity: serialize, deserialize, and check equality.
    pub fn verify_round_trip(graph: &ShaderGraph) -> Result<Vec<String>, SerializeError> {
        let restored = Self::round_trip(graph)?;
        let mut diffs = Vec::new();

        // Compare names
        if graph.name != restored.name {
            diffs.push(format!("Name mismatch: '{}' vs '{}'", graph.name, restored.name));
        }

        // Compare node counts
        if graph.node_count() != restored.node_count() {
            diffs.push(format!("Node count mismatch: {} vs {}",
                graph.node_count(), restored.node_count()));
        }

        // Compare connection counts
        if graph.connections().len() != restored.connections().len() {
            diffs.push(format!("Connection count mismatch: {} vs {}",
                graph.connections().len(), restored.connections().len()));
        }

        // Compare individual nodes
        for nid in graph.node_ids() {
            let orig = graph.node(&nid);
            let rest = restored.node(&nid);
            match (orig, rest) {
                (Some(o), Some(r)) => {
                    if o.node_type != r.node_type {
                        diffs.push(format!("Node {} type mismatch: {:?} vs {:?}",
                            nid.0, o.node_type, r.node_type));
                    }
                    if o.label != r.label {
                        diffs.push(format!("Node {} label mismatch: '{}' vs '{}'",
                            nid.0, o.label, r.label));
                    }
                    if o.enabled != r.enabled {
                        diffs.push(format!("Node {} enabled mismatch: {} vs {}",
                            nid.0, o.enabled, r.enabled));
                    }
                    // Compare input defaults
                    for (idx, (os, rs)) in o.inputs.iter().zip(r.inputs.iter()).enumerate() {
                        if os.default_value != rs.default_value {
                            diffs.push(format!("Node {} input {} default mismatch",
                                nid.0, idx));
                        }
                    }
                    // Compare properties
                    if o.properties.len() != r.properties.len() {
                        diffs.push(format!("Node {} property count mismatch: {} vs {}",
                            nid.0, o.properties.len(), r.properties.len()));
                    }
                }
                (Some(_), None) => {
                    diffs.push(format!("Node {} missing in restored graph", nid.0));
                }
                (None, Some(_)) => {
                    diffs.push(format!("Node {} unexpected in restored graph", nid.0));
                }
                (None, None) => {}
            }
        }

        // Compare connections
        let orig_conns: std::collections::HashSet<_> = graph.connections().iter()
            .map(|c| (c.from_node.0, c.from_socket, c.to_node.0, c.to_socket))
            .collect();
        let rest_conns: std::collections::HashSet<_> = restored.connections().iter()
            .map(|c| (c.from_node.0, c.from_socket, c.to_node.0, c.to_socket))
            .collect();

        for c in &orig_conns {
            if !rest_conns.contains(c) {
                diffs.push(format!("Connection {}:{} -> {}:{} missing in restored",
                    c.0, c.1, c.2, c.3));
            }
        }
        for c in &rest_conns {
            if !orig_conns.contains(c) {
                diffs.push(format!("Connection {}:{} -> {}:{} unexpected in restored",
                    c.0, c.1, c.2, c.3));
            }
        }

        Ok(diffs)
    }
}

// ---------------------------------------------------------------------------
// Parameter value serialization
// ---------------------------------------------------------------------------

/// Serialize a ParamValue to a type-tagged string: "type:value".
fn serialize_param_value(val: &ParamValue) -> String {
    match val {
        ParamValue::Float(v) => format!("float:{}", format_f32(*v)),
        ParamValue::Vec2(v) => format!("vec2:{},{}", format_f32(v[0]), format_f32(v[1])),
        ParamValue::Vec3(v) => format!("vec3:{},{},{}", format_f32(v[0]), format_f32(v[1]), format_f32(v[2])),
        ParamValue::Vec4(v) => format!("vec4:{},{},{},{}", format_f32(v[0]), format_f32(v[1]), format_f32(v[2]), format_f32(v[3])),
        ParamValue::Int(v) => format!("int:{}", v),
        ParamValue::Bool(v) => format!("bool:{}", v),
        ParamValue::String(v) => format!("string:{}", v),
    }
}

/// Deserialize a type-tagged string to a ParamValue.
fn deserialize_param_value(s: &str) -> Result<ParamValue, SerializeError> {
    let colon_pos = s.find(':')
        .ok_or_else(|| SerializeError::ParseError(format!("No type tag in value: '{}'", s)))?;
    let type_tag = &s[..colon_pos];
    let value_str = &s[colon_pos + 1..];

    match type_tag {
        "float" => {
            let v: f32 = value_str.parse()
                .map_err(|e| SerializeError::ParseError(format!("float: {}", e)))?;
            Ok(ParamValue::Float(v))
        }
        "vec2" => {
            let parts: Vec<&str> = value_str.split(',').collect();
            if parts.len() != 2 {
                return Err(SerializeError::ParseError("vec2 needs 2 components".to_string()));
            }
            let x: f32 = parts[0].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec2.x: {}", e)))?;
            let y: f32 = parts[1].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec2.y: {}", e)))?;
            Ok(ParamValue::Vec2([x, y]))
        }
        "vec3" => {
            let parts: Vec<&str> = value_str.split(',').collect();
            if parts.len() != 3 {
                return Err(SerializeError::ParseError("vec3 needs 3 components".to_string()));
            }
            let x: f32 = parts[0].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec3.x: {}", e)))?;
            let y: f32 = parts[1].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec3.y: {}", e)))?;
            let z: f32 = parts[2].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec3.z: {}", e)))?;
            Ok(ParamValue::Vec3([x, y, z]))
        }
        "vec4" => {
            let parts: Vec<&str> = value_str.split(',').collect();
            if parts.len() != 4 {
                return Err(SerializeError::ParseError("vec4 needs 4 components".to_string()));
            }
            let x: f32 = parts[0].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec4.x: {}", e)))?;
            let y: f32 = parts[1].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec4.y: {}", e)))?;
            let z: f32 = parts[2].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec4.z: {}", e)))?;
            let w: f32 = parts[3].trim().parse().map_err(|e| SerializeError::ParseError(format!("vec4.w: {}", e)))?;
            Ok(ParamValue::Vec4([x, y, z, w]))
        }
        "int" => {
            let v: i32 = value_str.parse()
                .map_err(|e| SerializeError::ParseError(format!("int: {}", e)))?;
            Ok(ParamValue::Int(v))
        }
        "bool" => {
            let v: bool = value_str.parse()
                .map_err(|e| SerializeError::ParseError(format!("bool: {}", e)))?;
            Ok(ParamValue::Bool(v))
        }
        "string" => {
            Ok(ParamValue::String(value_str.to_string()))
        }
        _ => Err(SerializeError::ParseError(format!("Unknown type tag: '{}'", type_tag))),
    }
}

// ---------------------------------------------------------------------------
// Node type string mapping
// ---------------------------------------------------------------------------

fn node_type_to_string(nt: &NodeType) -> &'static str {
    match nt {
        NodeType::Color => "Color",
        NodeType::Texture => "Texture",
        NodeType::VertexPosition => "VertexPosition",
        NodeType::VertexNormal => "VertexNormal",
        NodeType::Time => "Time",
        NodeType::CameraPos => "CameraPos",
        NodeType::GameStateVar => "GameStateVar",
        NodeType::Translate => "Translate",
        NodeType::Rotate => "Rotate",
        NodeType::Scale => "Scale",
        NodeType::WorldToLocal => "WorldToLocal",
        NodeType::LocalToWorld => "LocalToWorld",
        NodeType::Add => "Add",
        NodeType::Sub => "Sub",
        NodeType::Mul => "Mul",
        NodeType::Div => "Div",
        NodeType::Dot => "Dot",
        NodeType::Cross => "Cross",
        NodeType::Normalize => "Normalize",
        NodeType::Length => "Length",
        NodeType::Abs => "Abs",
        NodeType::Floor => "Floor",
        NodeType::Ceil => "Ceil",
        NodeType::Fract => "Fract",
        NodeType::Mod => "Mod",
        NodeType::Pow => "Pow",
        NodeType::Sqrt => "Sqrt",
        NodeType::Sin => "Sin",
        NodeType::Cos => "Cos",
        NodeType::Tan => "Tan",
        NodeType::Atan2 => "Atan2",
        NodeType::Lerp => "Lerp",
        NodeType::Clamp => "Clamp",
        NodeType::Smoothstep => "Smoothstep",
        NodeType::Remap => "Remap",
        NodeType::Step => "Step",
        NodeType::Fresnel => "Fresnel",
        NodeType::Dissolve => "Dissolve",
        NodeType::Distortion => "Distortion",
        NodeType::Blur => "Blur",
        NodeType::Sharpen => "Sharpen",
        NodeType::EdgeDetect => "EdgeDetect",
        NodeType::Outline => "Outline",
        NodeType::Bloom => "Bloom",
        NodeType::ChromaticAberration => "ChromaticAberration",
        NodeType::HSVToRGB => "HSVToRGB",
        NodeType::RGBToHSV => "RGBToHSV",
        NodeType::Contrast => "Contrast",
        NodeType::Saturation => "Saturation",
        NodeType::Hue => "Hue",
        NodeType::Invert => "Invert",
        NodeType::Posterize => "Posterize",
        NodeType::GradientMap => "GradientMap",
        NodeType::Perlin => "Perlin",
        NodeType::Simplex => "Simplex",
        NodeType::Voronoi => "Voronoi",
        NodeType::FBM => "FBM",
        NodeType::Turbulence => "Turbulence",
        NodeType::MainColor => "MainColor",
        NodeType::EmissionBuffer => "EmissionBuffer",
        NodeType::BloomBuffer => "BloomBuffer",
        NodeType::NormalOutput => "NormalOutput",
    }
}

fn string_to_node_type(s: &str) -> Result<NodeType, SerializeError> {
    match s {
        "Color" => Ok(NodeType::Color),
        "Texture" => Ok(NodeType::Texture),
        "VertexPosition" => Ok(NodeType::VertexPosition),
        "VertexNormal" => Ok(NodeType::VertexNormal),
        "Time" => Ok(NodeType::Time),
        "CameraPos" => Ok(NodeType::CameraPos),
        "GameStateVar" => Ok(NodeType::GameStateVar),
        "Translate" => Ok(NodeType::Translate),
        "Rotate" => Ok(NodeType::Rotate),
        "Scale" => Ok(NodeType::Scale),
        "WorldToLocal" => Ok(NodeType::WorldToLocal),
        "LocalToWorld" => Ok(NodeType::LocalToWorld),
        "Add" => Ok(NodeType::Add),
        "Sub" => Ok(NodeType::Sub),
        "Mul" => Ok(NodeType::Mul),
        "Div" => Ok(NodeType::Div),
        "Dot" => Ok(NodeType::Dot),
        "Cross" => Ok(NodeType::Cross),
        "Normalize" => Ok(NodeType::Normalize),
        "Length" => Ok(NodeType::Length),
        "Abs" => Ok(NodeType::Abs),
        "Floor" => Ok(NodeType::Floor),
        "Ceil" => Ok(NodeType::Ceil),
        "Fract" => Ok(NodeType::Fract),
        "Mod" => Ok(NodeType::Mod),
        "Pow" => Ok(NodeType::Pow),
        "Sqrt" => Ok(NodeType::Sqrt),
        "Sin" => Ok(NodeType::Sin),
        "Cos" => Ok(NodeType::Cos),
        "Tan" => Ok(NodeType::Tan),
        "Atan2" => Ok(NodeType::Atan2),
        "Lerp" => Ok(NodeType::Lerp),
        "Clamp" => Ok(NodeType::Clamp),
        "Smoothstep" => Ok(NodeType::Smoothstep),
        "Remap" => Ok(NodeType::Remap),
        "Step" => Ok(NodeType::Step),
        "Fresnel" => Ok(NodeType::Fresnel),
        "Dissolve" => Ok(NodeType::Dissolve),
        "Distortion" => Ok(NodeType::Distortion),
        "Blur" => Ok(NodeType::Blur),
        "Sharpen" => Ok(NodeType::Sharpen),
        "EdgeDetect" => Ok(NodeType::EdgeDetect),
        "Outline" => Ok(NodeType::Outline),
        "Bloom" => Ok(NodeType::Bloom),
        "ChromaticAberration" => Ok(NodeType::ChromaticAberration),
        "HSVToRGB" => Ok(NodeType::HSVToRGB),
        "RGBToHSV" => Ok(NodeType::RGBToHSV),
        "Contrast" => Ok(NodeType::Contrast),
        "Saturation" => Ok(NodeType::Saturation),
        "Hue" => Ok(NodeType::Hue),
        "Invert" => Ok(NodeType::Invert),
        "Posterize" => Ok(NodeType::Posterize),
        "GradientMap" => Ok(NodeType::GradientMap),
        "Perlin" => Ok(NodeType::Perlin),
        "Simplex" => Ok(NodeType::Simplex),
        "Voronoi" => Ok(NodeType::Voronoi),
        "FBM" => Ok(NodeType::FBM),
        "Turbulence" => Ok(NodeType::Turbulence),
        "MainColor" => Ok(NodeType::MainColor),
        "EmissionBuffer" => Ok(NodeType::EmissionBuffer),
        "BloomBuffer" => Ok(NodeType::BloomBuffer),
        "NormalOutput" => Ok(NodeType::NormalOutput),
        _ => Err(SerializeError::UnknownNodeType(s.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Section {
    None,
    Graph,
    Node,
    Connection,
}

/// Builder for constructing a ShaderNode during parsing.
#[derive(Default)]
struct NodeBuilder {
    id: Option<u64>,
    node_type: Option<String>,
    label: Option<String>,
    enabled: Option<bool>,
    editor_x: Option<f32>,
    editor_y: Option<f32>,
    conditional_var: Option<String>,
    conditional_threshold: Option<f32>,
    input_defaults: HashMap<usize, ParamValue>,
    properties: HashMap<String, ParamValue>,
}

/// Builder for constructing a Connection during parsing.
#[derive(Default)]
struct ConnBuilder {
    from_node: Option<u64>,
    from_socket: Option<usize>,
    to_node: Option<u64>,
    to_socket: Option<usize>,
}

/// Parse a "key = value" line.
fn parse_kv(line: &str) -> Option<(String, String)> {
    let eq_pos = line.find('=')?;
    let key = line[..eq_pos].trim().to_string();
    let value = line[eq_pos + 1..].trim().to_string();
    Some((key, value))
}

/// Remove surrounding quotes from a string.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        unescape_string(&s[1..s.len() - 1])
    } else {
        s.to_string()
    }
}

/// Escape special characters for serialization.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// Unescape special characters during deserialization.
fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some(other) => { result.push('\\'); result.push(other); }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Format a float for serialization (enough precision for round-trip).
fn format_f32(v: f32) -> String {
    if v == 0.0 {
        "0".to_string()
    } else if v == v.floor() && v.abs() < 1e7 {
        format!("{}", v as i64)
    } else {
        format!("{:.6}", v)
    }
}

fn parse_node_field(nb: &mut NodeBuilder, key: &str, value: &str) -> Result<(), SerializeError> {
    match key {
        "id" => {
            nb.id = Some(value.parse().map_err(|e| {
                SerializeError::ParseError(format!("node id: {}", e))
            })?);
        }
        "type" => {
            nb.node_type = Some(unquote(value));
        }
        "label" => {
            nb.label = Some(unquote(value));
        }
        "enabled" => {
            nb.enabled = Some(value.parse().map_err(|e| {
                SerializeError::ParseError(format!("enabled: {}", e))
            })?);
        }
        "editor_x" => {
            nb.editor_x = Some(value.parse().map_err(|e| {
                SerializeError::ParseError(format!("editor_x: {}", e))
            })?);
        }
        "editor_y" => {
            nb.editor_y = Some(value.parse().map_err(|e| {
                SerializeError::ParseError(format!("editor_y: {}", e))
            })?);
        }
        "conditional_var" => {
            nb.conditional_var = Some(unquote(value));
        }
        "conditional_threshold" => {
            nb.conditional_threshold = Some(value.parse().map_err(|e| {
                SerializeError::ParseError(format!("conditional_threshold: {}", e))
            })?);
        }
        _ if key.starts_with("input.") => {
            // Parse "input.N.default"
            let rest = &key["input.".len()..];
            if let Some(dot_pos) = rest.find('.') {
                let idx: usize = rest[..dot_pos].parse().map_err(|e| {
                    SerializeError::ParseError(format!("input index: {}", e))
                })?;
                let field = &rest[dot_pos + 1..];
                if field == "default" {
                    let val_str = unquote(value);
                    let pv = deserialize_param_value(&val_str)?;
                    nb.input_defaults.insert(idx, pv);
                }
            }
        }
        _ if key.starts_with("property.") => {
            let prop_name = key["property.".len()..].to_string();
            let val_str = unquote(value);
            let pv = deserialize_param_value(&val_str)?;
            nb.properties.insert(prop_name, pv);
        }
        _ => {} // ignore unknown
    }
    Ok(())
}

fn parse_conn_field(cb: &mut ConnBuilder, key: &str, value: &str) -> Result<(), SerializeError> {
    match key {
        "from" => {
            let (node, socket) = parse_node_socket(value)?;
            cb.from_node = Some(node);
            cb.from_socket = Some(socket);
        }
        "to" => {
            let (node, socket) = parse_node_socket(value)?;
            cb.to_node = Some(node);
            cb.to_socket = Some(socket);
        }
        _ => {}
    }
    Ok(())
}

/// Parse "node_id:socket_idx" format.
fn parse_node_socket(s: &str) -> Result<(u64, usize), SerializeError> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.len() != 2 {
        return Err(SerializeError::ParseError(format!("Expected node:socket format, got '{}'", s)));
    }
    let node: u64 = parts[0].parse().map_err(|e| {
        SerializeError::ParseError(format!("node id in connection: {}", e))
    })?;
    let socket: usize = parts[1].parse().map_err(|e| {
        SerializeError::ParseError(format!("socket index in connection: {}", e))
    })?;
    Ok((node, socket))
}

/// Flush the current node builder into the nodes list.
fn flush_node(current: &mut Option<NodeBuilder>, nodes: &mut Vec<ShaderNode>) -> Result<(), SerializeError> {
    if let Some(nb) = current.take() {
        let id = nb.id.ok_or_else(|| SerializeError::MissingField("node id".to_string()))?;
        let type_str = nb.node_type.ok_or_else(|| SerializeError::MissingField("node type".to_string()))?;
        let node_type = string_to_node_type(&type_str)?;

        let mut node = ShaderNode::new(NodeId(id), node_type);
        if let Some(label) = nb.label {
            node.label = label;
        }
        if let Some(enabled) = nb.enabled {
            node.enabled = enabled;
        }
        if let Some(x) = nb.editor_x {
            node.editor_x = x;
        }
        if let Some(y) = nb.editor_y {
            node.editor_y = y;
        }
        node.conditional_var = nb.conditional_var;
        node.conditional_threshold = nb.conditional_threshold.unwrap_or(0.0);

        // Apply input defaults
        for (idx, val) in nb.input_defaults {
            if idx < node.inputs.len() {
                node.inputs[idx].default_value = Some(val);
            }
        }

        // Apply properties
        node.properties = nb.properties;

        nodes.push(node);
    }
    Ok(())
}

/// Flush the current connection builder into the connections list.
fn flush_conn(current: &mut Option<ConnBuilder>, conns: &mut Vec<Connection>) -> Result<(), SerializeError> {
    if let Some(cb) = current.take() {
        let from_node = cb.from_node.ok_or_else(|| SerializeError::MissingField("connection from".to_string()))?;
        let from_socket = cb.from_socket.ok_or_else(|| SerializeError::MissingField("connection from socket".to_string()))?;
        let to_node = cb.to_node.ok_or_else(|| SerializeError::MissingField("connection to".to_string()))?;
        let to_socket = cb.to_socket.ok_or_else(|| SerializeError::MissingField("connection to socket".to_string()))?;

        conns.push(Connection::new(
            NodeId(from_node), from_socket,
            NodeId(to_node), to_socket,
        ));
    }
    Ok(())
}
