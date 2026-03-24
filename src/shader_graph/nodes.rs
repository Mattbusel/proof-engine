//! Shader graph node definitions: 40+ node types with input/output sockets,
//! GLSL snippet generation, and default parameter values.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Core ID types
// ---------------------------------------------------------------------------

/// Unique identifier for a node within a graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

/// Unique identifier for a socket (node_id, socket_index, direction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketId {
    pub node_id: NodeId,
    pub index: usize,
    pub direction: SocketDirection,
}

/// Direction of a socket — input or output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketDirection {
    Input,
    Output,
}

/// Data type flowing through a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Mat3,
    Mat4,
    Sampler2D,
    Bool,
    Int,
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Float => write!(f, "float"),
            DataType::Vec2 => write!(f, "vec2"),
            DataType::Vec3 => write!(f, "vec3"),
            DataType::Vec4 => write!(f, "vec4"),
            DataType::Mat3 => write!(f, "mat3"),
            DataType::Mat4 => write!(f, "mat4"),
            DataType::Sampler2D => write!(f, "sampler2D"),
            DataType::Bool => write!(f, "bool"),
            DataType::Int => write!(f, "int"),
        }
    }
}

// ---------------------------------------------------------------------------
// Socket definition
// ---------------------------------------------------------------------------

/// A socket on a node — either an input or an output.
#[derive(Debug, Clone)]
pub struct Socket {
    pub name: String,
    pub data_type: DataType,
    pub direction: SocketDirection,
    pub default_value: Option<ParamValue>,
}

impl Socket {
    pub fn input(name: &str, dt: DataType) -> Self {
        Self { name: name.to_string(), data_type: dt, direction: SocketDirection::Input, default_value: None }
    }

    pub fn input_default(name: &str, dt: DataType, val: ParamValue) -> Self {
        Self { name: name.to_string(), data_type: dt, direction: SocketDirection::Input, default_value: Some(val) }
    }

    pub fn output(name: &str, dt: DataType) -> Self {
        Self { name: name.to_string(), data_type: dt, direction: SocketDirection::Output, default_value: None }
    }
}

// ---------------------------------------------------------------------------
// Parameter values
// ---------------------------------------------------------------------------

/// Concrete parameter value that can be stored in a socket default or node property.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    Bool(bool),
    String(String),
}

impl ParamValue {
    pub fn as_float(&self) -> Option<f32> {
        match self { ParamValue::Float(v) => Some(*v), _ => None }
    }
    pub fn as_vec2(&self) -> Option<[f32; 2]> {
        match self { ParamValue::Vec2(v) => Some(*v), _ => None }
    }
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        match self { ParamValue::Vec3(v) => Some(*v), _ => None }
    }
    pub fn as_vec4(&self) -> Option<[f32; 4]> {
        match self { ParamValue::Vec4(v) => Some(*v), _ => None }
    }
    pub fn as_int(&self) -> Option<i32> {
        match self { ParamValue::Int(v) => Some(*v), _ => None }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self { ParamValue::Bool(v) => Some(*v), _ => None }
    }
    pub fn as_string(&self) -> Option<&str> {
        match self { ParamValue::String(v) => Some(v.as_str()), _ => None }
    }

    /// Produce a GLSL literal for this value.
    pub fn to_glsl(&self) -> String {
        match self {
            ParamValue::Float(v) => format_float(*v),
            ParamValue::Vec2(v) => format!("vec2({}, {})", format_float(v[0]), format_float(v[1])),
            ParamValue::Vec3(v) => format!("vec3({}, {}, {})", format_float(v[0]), format_float(v[1]), format_float(v[2])),
            ParamValue::Vec4(v) => format!("vec4({}, {}, {}, {})", format_float(v[0]), format_float(v[1]), format_float(v[2]), format_float(v[3])),
            ParamValue::Int(v) => format!("{}", v),
            ParamValue::Bool(v) => if *v { "true".to_string() } else { "false".to_string() },
            ParamValue::String(v) => v.clone(),
        }
    }

    /// Return the DataType that corresponds to this value.
    pub fn data_type(&self) -> DataType {
        match self {
            ParamValue::Float(_) => DataType::Float,
            ParamValue::Vec2(_) => DataType::Vec2,
            ParamValue::Vec3(_) => DataType::Vec3,
            ParamValue::Vec4(_) => DataType::Vec4,
            ParamValue::Int(_) => DataType::Int,
            ParamValue::Bool(_) => DataType::Bool,
            ParamValue::String(_) => DataType::Float, // strings are typically uniform names
        }
    }
}

fn format_float(v: f32) -> String {
    if v == v.floor() && v.abs() < 1e9 {
        format!("{:.1}", v)
    } else {
        format!("{}", v)
    }
}

// ---------------------------------------------------------------------------
// Node types — 40+ variants
// ---------------------------------------------------------------------------

/// All supported shader node types.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    // ── Sources ──────────────────────────────────────────────
    Color,
    Texture,
    VertexPosition,
    VertexNormal,
    Time,
    CameraPos,
    GameStateVar,

    // ── Transforms ──────────────────────────────────────────
    Translate,
    Rotate,
    Scale,
    WorldToLocal,
    LocalToWorld,

    // ── Math ────────────────────────────────────────────────
    Add,
    Sub,
    Mul,
    Div,
    Dot,
    Cross,
    Normalize,
    Length,
    Abs,
    Floor,
    Ceil,
    Fract,
    Mod,
    Pow,
    Sqrt,
    Sin,
    Cos,
    Tan,
    Atan2,
    Lerp,
    Clamp,
    Smoothstep,
    Remap,
    Step,

    // ── Effects ─────────────────────────────────────────────
    Fresnel,
    Dissolve,
    Distortion,
    Blur,
    Sharpen,
    EdgeDetect,
    Outline,
    Bloom,
    ChromaticAberration,

    // ── Color ───────────────────────────────────────────────
    HSVToRGB,
    RGBToHSV,
    Contrast,
    Saturation,
    Hue,
    Invert,
    Posterize,
    GradientMap,

    // ── Noise ───────────────────────────────────────────────
    Perlin,
    Simplex,
    Voronoi,
    FBM,
    Turbulence,

    // ── Output ──────────────────────────────────────────────
    MainColor,
    EmissionBuffer,
    BloomBuffer,
    NormalOutput,
}

impl NodeType {
    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            NodeType::Color => "Color",
            NodeType::Texture => "Texture Sample",
            NodeType::VertexPosition => "Vertex Position",
            NodeType::VertexNormal => "Vertex Normal",
            NodeType::Time => "Time",
            NodeType::CameraPos => "Camera Position",
            NodeType::GameStateVar => "Game State Variable",
            NodeType::Translate => "Translate",
            NodeType::Rotate => "Rotate",
            NodeType::Scale => "Scale",
            NodeType::WorldToLocal => "World To Local",
            NodeType::LocalToWorld => "Local To World",
            NodeType::Add => "Add",
            NodeType::Sub => "Subtract",
            NodeType::Mul => "Multiply",
            NodeType::Div => "Divide",
            NodeType::Dot => "Dot Product",
            NodeType::Cross => "Cross Product",
            NodeType::Normalize => "Normalize",
            NodeType::Length => "Length",
            NodeType::Abs => "Absolute",
            NodeType::Floor => "Floor",
            NodeType::Ceil => "Ceil",
            NodeType::Fract => "Fract",
            NodeType::Mod => "Modulo",
            NodeType::Pow => "Power",
            NodeType::Sqrt => "Square Root",
            NodeType::Sin => "Sine",
            NodeType::Cos => "Cosine",
            NodeType::Tan => "Tangent",
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
            NodeType::EdgeDetect => "Edge Detect",
            NodeType::Outline => "Outline",
            NodeType::Bloom => "Bloom",
            NodeType::ChromaticAberration => "Chromatic Aberration",
            NodeType::HSVToRGB => "HSV to RGB",
            NodeType::RGBToHSV => "RGB to HSV",
            NodeType::Contrast => "Contrast",
            NodeType::Saturation => "Saturation",
            NodeType::Hue => "Hue Shift",
            NodeType::Invert => "Invert",
            NodeType::Posterize => "Posterize",
            NodeType::GradientMap => "Gradient Map",
            NodeType::Perlin => "Perlin Noise",
            NodeType::Simplex => "Simplex Noise",
            NodeType::Voronoi => "Voronoi",
            NodeType::FBM => "FBM",
            NodeType::Turbulence => "Turbulence",
            NodeType::MainColor => "Main Color Output",
            NodeType::EmissionBuffer => "Emission Buffer",
            NodeType::BloomBuffer => "Bloom Buffer",
            NodeType::NormalOutput => "Normal Output",
        }
    }

    /// Category string for grouping in a UI palette.
    pub fn category(&self) -> &'static str {
        match self {
            NodeType::Color | NodeType::Texture | NodeType::VertexPosition
            | NodeType::VertexNormal | NodeType::Time | NodeType::CameraPos
            | NodeType::GameStateVar => "Source",

            NodeType::Translate | NodeType::Rotate | NodeType::Scale
            | NodeType::WorldToLocal | NodeType::LocalToWorld => "Transform",

            NodeType::Add | NodeType::Sub | NodeType::Mul | NodeType::Div
            | NodeType::Dot | NodeType::Cross | NodeType::Normalize | NodeType::Length
            | NodeType::Abs | NodeType::Floor | NodeType::Ceil | NodeType::Fract
            | NodeType::Mod | NodeType::Pow | NodeType::Sqrt | NodeType::Sin
            | NodeType::Cos | NodeType::Tan | NodeType::Atan2 | NodeType::Lerp
            | NodeType::Clamp | NodeType::Smoothstep | NodeType::Remap
            | NodeType::Step => "Math",

            NodeType::Fresnel | NodeType::Dissolve | NodeType::Distortion
            | NodeType::Blur | NodeType::Sharpen | NodeType::EdgeDetect
            | NodeType::Outline | NodeType::Bloom
            | NodeType::ChromaticAberration => "Effect",

            NodeType::HSVToRGB | NodeType::RGBToHSV | NodeType::Contrast
            | NodeType::Saturation | NodeType::Hue | NodeType::Invert
            | NodeType::Posterize | NodeType::GradientMap => "Color",

            NodeType::Perlin | NodeType::Simplex | NodeType::Voronoi
            | NodeType::FBM | NodeType::Turbulence => "Noise",

            NodeType::MainColor | NodeType::EmissionBuffer | NodeType::BloomBuffer
            | NodeType::NormalOutput => "Output",
        }
    }

    /// Whether this node type is an output sink (terminal node).
    pub fn is_output(&self) -> bool {
        matches!(self, NodeType::MainColor | NodeType::EmissionBuffer | NodeType::BloomBuffer | NodeType::NormalOutput)
    }

    /// Whether this node is a pure math operation (eligible for constant folding).
    pub fn is_pure_math(&self) -> bool {
        matches!(self,
            NodeType::Add | NodeType::Sub | NodeType::Mul | NodeType::Div
            | NodeType::Dot | NodeType::Cross | NodeType::Normalize | NodeType::Length
            | NodeType::Abs | NodeType::Floor | NodeType::Ceil | NodeType::Fract
            | NodeType::Mod | NodeType::Pow | NodeType::Sqrt | NodeType::Sin
            | NodeType::Cos | NodeType::Tan | NodeType::Atan2 | NodeType::Lerp
            | NodeType::Clamp | NodeType::Smoothstep | NodeType::Remap | NodeType::Step
            | NodeType::HSVToRGB | NodeType::RGBToHSV | NodeType::Contrast
            | NodeType::Saturation | NodeType::Hue | NodeType::Invert
            | NodeType::Posterize
        )
    }

    /// Whether this node is a source that requires no inputs from other nodes.
    pub fn is_source(&self) -> bool {
        matches!(self,
            NodeType::Color | NodeType::Texture | NodeType::VertexPosition
            | NodeType::VertexNormal | NodeType::Time | NodeType::CameraPos
            | NodeType::GameStateVar
        )
    }

    /// Estimated GPU instruction cost of this node (for budgeting).
    pub fn instruction_cost(&self) -> u32 {
        match self {
            // Sources are essentially free (reads)
            NodeType::Color => 0,
            NodeType::VertexPosition | NodeType::VertexNormal | NodeType::CameraPos => 1,
            NodeType::Time => 0,
            NodeType::GameStateVar => 1,
            NodeType::Texture => 4,

            // Transforms
            NodeType::Translate | NodeType::Scale => 3,
            NodeType::Rotate => 8,
            NodeType::WorldToLocal | NodeType::LocalToWorld => 16,

            // Simple math
            NodeType::Add | NodeType::Sub => 1,
            NodeType::Mul => 1,
            NodeType::Div => 2,
            NodeType::Dot => 3,
            NodeType::Cross => 6,
            NodeType::Normalize => 4,
            NodeType::Length => 3,
            NodeType::Abs | NodeType::Floor | NodeType::Ceil | NodeType::Fract => 1,
            NodeType::Mod => 2,
            NodeType::Pow => 4,
            NodeType::Sqrt => 2,
            NodeType::Sin | NodeType::Cos | NodeType::Tan => 4,
            NodeType::Atan2 => 6,
            NodeType::Lerp => 3,
            NodeType::Clamp => 2,
            NodeType::Smoothstep => 5,
            NodeType::Remap => 6,
            NodeType::Step => 1,

            // Effects
            NodeType::Fresnel => 8,
            NodeType::Dissolve => 12,
            NodeType::Distortion => 10,
            NodeType::Blur => 32,
            NodeType::Sharpen => 20,
            NodeType::EdgeDetect => 24,
            NodeType::Outline => 16,
            NodeType::Bloom => 28,
            NodeType::ChromaticAberration => 18,

            // Color ops
            NodeType::HSVToRGB | NodeType::RGBToHSV => 10,
            NodeType::Contrast | NodeType::Saturation | NodeType::Hue => 6,
            NodeType::Invert => 1,
            NodeType::Posterize => 4,
            NodeType::GradientMap => 8,

            // Noise
            NodeType::Perlin => 20,
            NodeType::Simplex => 24,
            NodeType::Voronoi => 30,
            NodeType::FBM => 60,
            NodeType::Turbulence => 50,

            // Outputs are essentially writes
            NodeType::MainColor | NodeType::EmissionBuffer
            | NodeType::BloomBuffer | NodeType::NormalOutput => 1,
        }
    }

    /// Build the default input sockets for this node type.
    pub fn default_inputs(&self) -> Vec<Socket> {
        match self {
            // ── Sources ─────────────────────────────────────
            NodeType::Color => vec![
                Socket::input_default("color", DataType::Vec4, ParamValue::Vec4([1.0, 1.0, 1.0, 1.0])),
            ],
            NodeType::Texture => vec![
                Socket::input("uv", DataType::Vec2),
                Socket::input_default("sampler", DataType::Sampler2D, ParamValue::Int(0)),
            ],
            NodeType::VertexPosition => vec![],
            NodeType::VertexNormal => vec![],
            NodeType::Time => vec![
                Socket::input_default("speed", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::CameraPos => vec![],
            NodeType::GameStateVar => vec![
                Socket::input_default("var_name", DataType::Float, ParamValue::String("game_var_0".to_string())),
            ],

            // ── Transforms ──────────────────────────────────
            NodeType::Translate => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input_default("offset", DataType::Vec3, ParamValue::Vec3([0.0, 0.0, 0.0])),
            ],
            NodeType::Rotate => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input_default("axis", DataType::Vec3, ParamValue::Vec3([0.0, 1.0, 0.0])),
                Socket::input_default("angle", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Scale => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input_default("factor", DataType::Vec3, ParamValue::Vec3([1.0, 1.0, 1.0])),
            ],
            NodeType::WorldToLocal => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input("matrix", DataType::Mat4),
            ],
            NodeType::LocalToWorld => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input("matrix", DataType::Mat4),
            ],

            // ── Math (binary) ───────────────────────────────
            NodeType::Add => vec![
                Socket::input_default("a", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("b", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Sub => vec![
                Socket::input_default("a", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("b", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Mul => vec![
                Socket::input_default("a", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("b", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Div => vec![
                Socket::input_default("a", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("b", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Dot => vec![
                Socket::input("a", DataType::Vec3),
                Socket::input("b", DataType::Vec3),
            ],
            NodeType::Cross => vec![
                Socket::input("a", DataType::Vec3),
                Socket::input("b", DataType::Vec3),
            ],
            NodeType::Normalize => vec![
                Socket::input("v", DataType::Vec3),
            ],
            NodeType::Length => vec![
                Socket::input("v", DataType::Vec3),
            ],
            NodeType::Abs => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Floor => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Ceil => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Fract => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Mod => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("y", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Pow => vec![
                Socket::input_default("base", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("exp", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Sqrt => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Sin => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Cos => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Tan => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Atan2 => vec![
                Socket::input_default("y", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("x", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Lerp => vec![
                Socket::input_default("a", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("b", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("t", DataType::Float, ParamValue::Float(0.5)),
            ],
            NodeType::Clamp => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("min_val", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("max_val", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Smoothstep => vec![
                Socket::input_default("edge0", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("edge1", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.5)),
            ],
            NodeType::Remap => vec![
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.5)),
                Socket::input_default("in_min", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("in_max", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("out_min", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("out_max", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Step => vec![
                Socket::input_default("edge", DataType::Float, ParamValue::Float(0.5)),
                Socket::input_default("x", DataType::Float, ParamValue::Float(0.0)),
            ],

            // ── Effects ─────────────────────────────────────
            NodeType::Fresnel => vec![
                Socket::input("normal", DataType::Vec3),
                Socket::input("view_dir", DataType::Vec3),
                Socket::input_default("power", DataType::Float, ParamValue::Float(2.0)),
                Socket::input_default("bias", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Dissolve => vec![
                Socket::input("color", DataType::Vec4),
                Socket::input_default("noise", DataType::Float, ParamValue::Float(0.5)),
                Socket::input_default("threshold", DataType::Float, ParamValue::Float(0.5)),
                Socket::input_default("edge_width", DataType::Float, ParamValue::Float(0.05)),
                Socket::input_default("edge_color", DataType::Vec4, ParamValue::Vec4([1.0, 0.5, 0.0, 1.0])),
            ],
            NodeType::Distortion => vec![
                Socket::input("uv", DataType::Vec2),
                Socket::input_default("strength", DataType::Float, ParamValue::Float(0.1)),
                Socket::input_default("direction", DataType::Vec2, ParamValue::Vec2([1.0, 0.0])),
                Socket::input_default("noise", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Blur => vec![
                Socket::input("uv", DataType::Vec2),
                Socket::input_default("radius", DataType::Float, ParamValue::Float(2.0)),
                Socket::input_default("samples", DataType::Int, ParamValue::Int(8)),
                Socket::input("sampler", DataType::Sampler2D),
            ],
            NodeType::Sharpen => vec![
                Socket::input("uv", DataType::Vec2),
                Socket::input_default("strength", DataType::Float, ParamValue::Float(1.0)),
                Socket::input("sampler", DataType::Sampler2D),
            ],
            NodeType::EdgeDetect => vec![
                Socket::input("uv", DataType::Vec2),
                Socket::input_default("threshold", DataType::Float, ParamValue::Float(0.1)),
                Socket::input("sampler", DataType::Sampler2D),
            ],
            NodeType::Outline => vec![
                Socket::input("color", DataType::Vec4),
                Socket::input("depth", DataType::Float),
                Socket::input("normal", DataType::Vec3),
                Socket::input_default("width", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("outline_color", DataType::Vec4, ParamValue::Vec4([0.0, 0.0, 0.0, 1.0])),
            ],
            NodeType::Bloom => vec![
                Socket::input("color", DataType::Vec4),
                Socket::input_default("threshold", DataType::Float, ParamValue::Float(0.8)),
                Socket::input_default("intensity", DataType::Float, ParamValue::Float(1.5)),
                Socket::input_default("radius", DataType::Float, ParamValue::Float(4.0)),
            ],
            NodeType::ChromaticAberration => vec![
                Socket::input("uv", DataType::Vec2),
                Socket::input_default("offset", DataType::Float, ParamValue::Float(0.005)),
                Socket::input("sampler", DataType::Sampler2D),
            ],

            // ── Color ───────────────────────────────────────
            NodeType::HSVToRGB => vec![
                Socket::input_default("h", DataType::Float, ParamValue::Float(0.0)),
                Socket::input_default("s", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("v", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::RGBToHSV => vec![
                Socket::input("rgb", DataType::Vec3),
            ],
            NodeType::Contrast => vec![
                Socket::input("color", DataType::Vec3),
                Socket::input_default("amount", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Saturation => vec![
                Socket::input("color", DataType::Vec3),
                Socket::input_default("amount", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::Hue => vec![
                Socket::input("color", DataType::Vec3),
                Socket::input_default("shift", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Invert => vec![
                Socket::input("color", DataType::Vec3),
            ],
            NodeType::Posterize => vec![
                Socket::input("color", DataType::Vec3),
                Socket::input_default("levels", DataType::Float, ParamValue::Float(4.0)),
            ],
            NodeType::GradientMap => vec![
                Socket::input_default("t", DataType::Float, ParamValue::Float(0.5)),
                Socket::input_default("color_a", DataType::Vec3, ParamValue::Vec3([0.0, 0.0, 0.0])),
                Socket::input_default("color_b", DataType::Vec3, ParamValue::Vec3([1.0, 1.0, 1.0])),
            ],

            // ── Noise ───────────────────────────────────────
            NodeType::Perlin => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input_default("scale", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("seed", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Simplex => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input_default("scale", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("seed", DataType::Float, ParamValue::Float(0.0)),
            ],
            NodeType::Voronoi => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input_default("scale", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("jitter", DataType::Float, ParamValue::Float(1.0)),
            ],
            NodeType::FBM => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input_default("scale", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("octaves", DataType::Int, ParamValue::Int(4)),
                Socket::input_default("lacunarity", DataType::Float, ParamValue::Float(2.0)),
                Socket::input_default("gain", DataType::Float, ParamValue::Float(0.5)),
            ],
            NodeType::Turbulence => vec![
                Socket::input("position", DataType::Vec3),
                Socket::input_default("scale", DataType::Float, ParamValue::Float(1.0)),
                Socket::input_default("octaves", DataType::Int, ParamValue::Int(4)),
                Socket::input_default("lacunarity", DataType::Float, ParamValue::Float(2.0)),
                Socket::input_default("gain", DataType::Float, ParamValue::Float(0.5)),
            ],

            // ── Outputs ─────────────────────────────────────
            NodeType::MainColor => vec![
                Socket::input("color", DataType::Vec4),
            ],
            NodeType::EmissionBuffer => vec![
                Socket::input("emission", DataType::Vec4),
            ],
            NodeType::BloomBuffer => vec![
                Socket::input("bloom", DataType::Vec4),
            ],
            NodeType::NormalOutput => vec![
                Socket::input("normal", DataType::Vec3),
            ],
        }
    }

    /// Build the default output sockets for this node type.
    pub fn default_outputs(&self) -> Vec<Socket> {
        match self {
            // Sources
            NodeType::Color => vec![Socket::output("color", DataType::Vec4)],
            NodeType::Texture => vec![
                Socket::output("color", DataType::Vec4),
                Socket::output("r", DataType::Float),
                Socket::output("g", DataType::Float),
                Socket::output("b", DataType::Float),
                Socket::output("a", DataType::Float),
            ],
            NodeType::VertexPosition => vec![Socket::output("position", DataType::Vec3)],
            NodeType::VertexNormal => vec![Socket::output("normal", DataType::Vec3)],
            NodeType::Time => vec![
                Socket::output("time", DataType::Float),
                Socket::output("sin_time", DataType::Float),
                Socket::output("cos_time", DataType::Float),
                Socket::output("fract_time", DataType::Float),
            ],
            NodeType::CameraPos => vec![Socket::output("position", DataType::Vec3)],
            NodeType::GameStateVar => vec![Socket::output("value", DataType::Float)],

            // Transforms
            NodeType::Translate | NodeType::Rotate | NodeType::Scale
            | NodeType::WorldToLocal | NodeType::LocalToWorld => {
                vec![Socket::output("result", DataType::Vec3)]
            }

            // Binary math
            NodeType::Add | NodeType::Sub | NodeType::Mul | NodeType::Div => {
                vec![Socket::output("result", DataType::Float)]
            }
            NodeType::Dot => vec![Socket::output("result", DataType::Float)],
            NodeType::Cross => vec![Socket::output("result", DataType::Vec3)],
            NodeType::Normalize => vec![Socket::output("result", DataType::Vec3)],
            NodeType::Length => vec![Socket::output("result", DataType::Float)],
            NodeType::Abs | NodeType::Floor | NodeType::Ceil | NodeType::Fract
            | NodeType::Mod | NodeType::Pow | NodeType::Sqrt => {
                vec![Socket::output("result", DataType::Float)]
            }
            NodeType::Sin | NodeType::Cos | NodeType::Tan | NodeType::Atan2 => {
                vec![Socket::output("result", DataType::Float)]
            }
            NodeType::Lerp | NodeType::Clamp | NodeType::Smoothstep
            | NodeType::Remap | NodeType::Step => {
                vec![Socket::output("result", DataType::Float)]
            }

            // Effects
            NodeType::Fresnel => vec![Socket::output("factor", DataType::Float)],
            NodeType::Dissolve => vec![
                Socket::output("color", DataType::Vec4),
                Socket::output("mask", DataType::Float),
            ],
            NodeType::Distortion => vec![Socket::output("uv", DataType::Vec2)],
            NodeType::Blur => vec![Socket::output("color", DataType::Vec4)],
            NodeType::Sharpen => vec![Socket::output("color", DataType::Vec4)],
            NodeType::EdgeDetect => vec![Socket::output("edges", DataType::Float)],
            NodeType::Outline => vec![Socket::output("color", DataType::Vec4)],
            NodeType::Bloom => vec![
                Socket::output("color", DataType::Vec4),
                Socket::output("bloom_mask", DataType::Float),
            ],
            NodeType::ChromaticAberration => vec![Socket::output("color", DataType::Vec4)],

            // Color
            NodeType::HSVToRGB => vec![Socket::output("rgb", DataType::Vec3)],
            NodeType::RGBToHSV => vec![
                Socket::output("h", DataType::Float),
                Socket::output("s", DataType::Float),
                Socket::output("v", DataType::Float),
            ],
            NodeType::Contrast | NodeType::Saturation | NodeType::Hue
            | NodeType::Invert | NodeType::Posterize => {
                vec![Socket::output("color", DataType::Vec3)]
            }
            NodeType::GradientMap => vec![Socket::output("color", DataType::Vec3)],

            // Noise
            NodeType::Perlin | NodeType::Simplex => vec![
                Socket::output("value", DataType::Float),
                Socket::output("gradient", DataType::Vec3),
            ],
            NodeType::Voronoi => vec![
                Socket::output("distance", DataType::Float),
                Socket::output("cell_id", DataType::Float),
                Socket::output("cell_pos", DataType::Vec3),
            ],
            NodeType::FBM | NodeType::Turbulence => vec![
                Socket::output("value", DataType::Float),
            ],

            // Outputs (terminal, no outputs)
            NodeType::MainColor | NodeType::EmissionBuffer
            | NodeType::BloomBuffer | NodeType::NormalOutput => vec![],
        }
    }

    /// Generate the GLSL code snippet for this node.
    ///
    /// `var_prefix` is the unique variable name prefix for this node instance (e.g., "n42").
    /// `input_vars` maps input socket index to the GLSL expression feeding it.
    ///
    /// Returns a vector of (line_of_code, output_socket_index).
    pub fn generate_glsl(&self, var_prefix: &str, input_vars: &[String]) -> GlslSnippet {
        let mut lines = Vec::new();
        let mut outputs: Vec<String> = Vec::new();

        match self {
            // ── Sources ─────────────────────────────────────
            NodeType::Color => {
                let inp = input_or_default(input_vars, 0, "vec4(1.0, 1.0, 1.0, 1.0)");
                let out = format!("{}_color", var_prefix);
                lines.push(format!("vec4 {} = {};", out, inp));
                outputs.push(out);
            }
            NodeType::Texture => {
                let uv = input_or_default(input_vars, 0, "v_uv");
                let sampler = input_or_default(input_vars, 1, "u_texture0");
                let out = format!("{}_tex", var_prefix);
                lines.push(format!("vec4 {} = texture2D({}, {});", out, sampler, uv));
                outputs.push(format!("{}", out));
                outputs.push(format!("{}.r", out));
                outputs.push(format!("{}.g", out));
                outputs.push(format!("{}.b", out));
                outputs.push(format!("{}.a", out));
            }
            NodeType::VertexPosition => {
                let out = format!("{}_vpos", var_prefix);
                lines.push(format!("vec3 {} = v_position;", out));
                outputs.push(out);
            }
            NodeType::VertexNormal => {
                let out = format!("{}_vnorm", var_prefix);
                lines.push(format!("vec3 {} = v_normal;", out));
                outputs.push(out);
            }
            NodeType::Time => {
                let speed = input_or_default(input_vars, 0, "1.0");
                let t = format!("{}_t", var_prefix);
                lines.push(format!("float {} = u_time * {};", t, speed));
                outputs.push(t.clone());
                outputs.push(format!("sin({})", t));
                outputs.push(format!("cos({})", t));
                outputs.push(format!("fract({})", t));
            }
            NodeType::CameraPos => {
                let out = format!("{}_campos", var_prefix);
                lines.push(format!("vec3 {} = u_camera_pos;", out));
                outputs.push(out);
            }
            NodeType::GameStateVar => {
                let var_name = input_or_default(input_vars, 0, "game_var_0");
                let out = format!("{}_gsv", var_prefix);
                // Game state vars are bound as uniforms named u_gs_<var_name>
                let uniform_name = if var_name.starts_with("u_gs_") {
                    var_name.clone()
                } else {
                    format!("u_gs_{}", var_name.trim_matches('"'))
                };
                lines.push(format!("float {} = {};", out, uniform_name));
                outputs.push(out);
            }

            // ── Transforms ──────────────────────────────────
            NodeType::Translate => {
                let pos = input_or_default(input_vars, 0, "vec3(0.0)");
                let offset = input_or_default(input_vars, 1, "vec3(0.0)");
                let out = format!("{}_trans", var_prefix);
                lines.push(format!("vec3 {} = {} + {};", out, pos, offset));
                outputs.push(out);
            }
            NodeType::Rotate => {
                let pos = input_or_default(input_vars, 0, "vec3(0.0)");
                let axis = input_or_default(input_vars, 1, "vec3(0.0, 1.0, 0.0)");
                let angle = input_or_default(input_vars, 2, "0.0");
                let out = format!("{}_rot", var_prefix);
                // Rodrigues' rotation formula
                lines.push(format!("vec3 {out}_k = normalize({axis});"));
                lines.push(format!("float {out}_c = cos({angle});"));
                lines.push(format!("float {out}_s = sin({angle});"));
                lines.push(format!(
                    "vec3 {out} = {pos} * {out}_c + cross({out}_k, {pos}) * {out}_s + {out}_k * dot({out}_k, {pos}) * (1.0 - {out}_c);",
                ));
                outputs.push(out);
            }
            NodeType::Scale => {
                let pos = input_or_default(input_vars, 0, "vec3(0.0)");
                let factor = input_or_default(input_vars, 1, "vec3(1.0)");
                let out = format!("{}_scl", var_prefix);
                lines.push(format!("vec3 {} = {} * {};", out, pos, factor));
                outputs.push(out);
            }
            NodeType::WorldToLocal => {
                let pos = input_or_default(input_vars, 0, "vec3(0.0)");
                let mat = input_or_default(input_vars, 1, "u_inv_model");
                let out = format!("{}_w2l", var_prefix);
                lines.push(format!("vec3 {} = ({} * vec4({}, 1.0)).xyz;", out, mat, pos));
                outputs.push(out);
            }
            NodeType::LocalToWorld => {
                let pos = input_or_default(input_vars, 0, "vec3(0.0)");
                let mat = input_or_default(input_vars, 1, "u_model");
                let out = format!("{}_l2w", var_prefix);
                lines.push(format!("vec3 {} = ({} * vec4({}, 1.0)).xyz;", out, mat, pos));
                outputs.push(out);
            }

            // ── Math ────────────────────────────────────────
            NodeType::Add => {
                let a = input_or_default(input_vars, 0, "0.0");
                let b = input_or_default(input_vars, 1, "0.0");
                let out = format!("{}_add", var_prefix);
                lines.push(format!("float {} = {} + {};", out, a, b));
                outputs.push(out);
            }
            NodeType::Sub => {
                let a = input_or_default(input_vars, 0, "0.0");
                let b = input_or_default(input_vars, 1, "0.0");
                let out = format!("{}_sub", var_prefix);
                lines.push(format!("float {} = {} - {};", out, a, b));
                outputs.push(out);
            }
            NodeType::Mul => {
                let a = input_or_default(input_vars, 0, "1.0");
                let b = input_or_default(input_vars, 1, "1.0");
                let out = format!("{}_mul", var_prefix);
                lines.push(format!("float {} = {} * {};", out, a, b));
                outputs.push(out);
            }
            NodeType::Div => {
                let a = input_or_default(input_vars, 0, "1.0");
                let b = input_or_default(input_vars, 1, "1.0");
                let out = format!("{}_div", var_prefix);
                lines.push(format!("float {} = {} / max({}, 0.0001);", out, a, b));
                outputs.push(out);
            }
            NodeType::Dot => {
                let a = input_or_default(input_vars, 0, "vec3(0.0)");
                let b = input_or_default(input_vars, 1, "vec3(0.0)");
                let out = format!("{}_dot", var_prefix);
                lines.push(format!("float {} = dot({}, {});", out, a, b));
                outputs.push(out);
            }
            NodeType::Cross => {
                let a = input_or_default(input_vars, 0, "vec3(0.0)");
                let b = input_or_default(input_vars, 1, "vec3(0.0)");
                let out = format!("{}_cross", var_prefix);
                lines.push(format!("vec3 {} = cross({}, {});", out, a, b));
                outputs.push(out);
            }
            NodeType::Normalize => {
                let v = input_or_default(input_vars, 0, "vec3(0.0, 1.0, 0.0)");
                let out = format!("{}_norm", var_prefix);
                lines.push(format!("vec3 {} = normalize({});", out, v));
                outputs.push(out);
            }
            NodeType::Length => {
                let v = input_or_default(input_vars, 0, "vec3(0.0)");
                let out = format!("{}_len", var_prefix);
                lines.push(format!("float {} = length({});", out, v));
                outputs.push(out);
            }
            NodeType::Abs => {
                let x = input_or_default(input_vars, 0, "0.0");
                let out = format!("{}_abs", var_prefix);
                lines.push(format!("float {} = abs({});", out, x));
                outputs.push(out);
            }
            NodeType::Floor => {
                let x = input_or_default(input_vars, 0, "0.0");
                let out = format!("{}_floor", var_prefix);
                lines.push(format!("float {} = floor({});", out, x));
                outputs.push(out);
            }
            NodeType::Ceil => {
                let x = input_or_default(input_vars, 0, "0.0");
                let out = format!("{}_ceil", var_prefix);
                lines.push(format!("float {} = ceil({});", out, x));
                outputs.push(out);
            }
            NodeType::Fract => {
                let x = input_or_default(input_vars, 0, "0.0");
                let out = format!("{}_fract", var_prefix);
                lines.push(format!("float {} = fract({});", out, x));
                outputs.push(out);
            }
            NodeType::Mod => {
                let x = input_or_default(input_vars, 0, "0.0");
                let y = input_or_default(input_vars, 1, "1.0");
                let out = format!("{}_mod", var_prefix);
                lines.push(format!("float {} = mod({}, {});", out, x, y));
                outputs.push(out);
            }
            NodeType::Pow => {
                let base = input_or_default(input_vars, 0, "1.0");
                let exp = input_or_default(input_vars, 1, "1.0");
                let out = format!("{}_pow", var_prefix);
                lines.push(format!("float {} = pow(max({}, 0.0), {});", out, base, exp));
                outputs.push(out);
            }
            NodeType::Sqrt => {
                let x = input_or_default(input_vars, 0, "1.0");
                let out = format!("{}_sqrt", var_prefix);
                lines.push(format!("float {} = sqrt(max({}, 0.0));", out, x));
                outputs.push(out);
            }
            NodeType::Sin => {
                let x = input_or_default(input_vars, 0, "0.0");
                let out = format!("{}_sin", var_prefix);
                lines.push(format!("float {} = sin({});", out, x));
                outputs.push(out);
            }
            NodeType::Cos => {
                let x = input_or_default(input_vars, 0, "0.0");
                let out = format!("{}_cos", var_prefix);
                lines.push(format!("float {} = cos({});", out, x));
                outputs.push(out);
            }
            NodeType::Tan => {
                let x = input_or_default(input_vars, 0, "0.0");
                let out = format!("{}_tan", var_prefix);
                lines.push(format!("float {} = tan({});", out, x));
                outputs.push(out);
            }
            NodeType::Atan2 => {
                let y = input_or_default(input_vars, 0, "0.0");
                let x = input_or_default(input_vars, 1, "1.0");
                let out = format!("{}_atan2", var_prefix);
                lines.push(format!("float {} = atan({}, {});", out, y, x));
                outputs.push(out);
            }
            NodeType::Lerp => {
                let a = input_or_default(input_vars, 0, "0.0");
                let b = input_or_default(input_vars, 1, "1.0");
                let t = input_or_default(input_vars, 2, "0.5");
                let out = format!("{}_lerp", var_prefix);
                lines.push(format!("float {} = mix({}, {}, {});", out, a, b, t));
                outputs.push(out);
            }
            NodeType::Clamp => {
                let x = input_or_default(input_vars, 0, "0.0");
                let lo = input_or_default(input_vars, 1, "0.0");
                let hi = input_or_default(input_vars, 2, "1.0");
                let out = format!("{}_clamp", var_prefix);
                lines.push(format!("float {} = clamp({}, {}, {});", out, x, lo, hi));
                outputs.push(out);
            }
            NodeType::Smoothstep => {
                let e0 = input_or_default(input_vars, 0, "0.0");
                let e1 = input_or_default(input_vars, 1, "1.0");
                let x = input_or_default(input_vars, 2, "0.5");
                let out = format!("{}_ss", var_prefix);
                lines.push(format!("float {} = smoothstep({}, {}, {});", out, e0, e1, x));
                outputs.push(out);
            }
            NodeType::Remap => {
                let x = input_or_default(input_vars, 0, "0.5");
                let in_min = input_or_default(input_vars, 1, "0.0");
                let in_max = input_or_default(input_vars, 2, "1.0");
                let out_min = input_or_default(input_vars, 3, "0.0");
                let out_max = input_or_default(input_vars, 4, "1.0");
                let out = format!("{}_remap", var_prefix);
                lines.push(format!(
                    "float {} = {} + ({} - {}) * (({}) - {}) / max(({}) - {}, 0.0001);",
                    out, out_min, out_max, out_min, x, in_min, in_max, in_min
                ));
                outputs.push(out);
            }
            NodeType::Step => {
                let edge = input_or_default(input_vars, 0, "0.5");
                let x = input_or_default(input_vars, 1, "0.0");
                let out = format!("{}_step", var_prefix);
                lines.push(format!("float {} = step({}, {});", out, edge, x));
                outputs.push(out);
            }

            // ── Effects ─────────────────────────────────────
            NodeType::Fresnel => {
                let normal = input_or_default(input_vars, 0, "v_normal");
                let view = input_or_default(input_vars, 1, "normalize(u_camera_pos - v_position)");
                let power = input_or_default(input_vars, 2, "2.0");
                let bias = input_or_default(input_vars, 3, "0.0");
                let out = format!("{}_fresnel", var_prefix);
                lines.push(format!(
                    "float {} = {} + (1.0 - {}) * pow(1.0 - max(dot({}, {}), 0.0), {});",
                    out, bias, bias, normal, view, power
                ));
                outputs.push(out);
            }
            NodeType::Dissolve => {
                let color = input_or_default(input_vars, 0, "vec4(1.0)");
                let noise = input_or_default(input_vars, 1, "0.5");
                let thresh = input_or_default(input_vars, 2, "0.5");
                let edge_w = input_or_default(input_vars, 3, "0.05");
                let edge_c = input_or_default(input_vars, 4, "vec4(1.0, 0.5, 0.0, 1.0)");
                let out = format!("{}_diss", var_prefix);
                lines.push(format!("float {out}_mask = step({thresh}, {noise});"));
                lines.push(format!(
                    "float {out}_edge = smoothstep({thresh} - {edge_w}, {thresh}, {noise}) - {out}_mask;"
                ));
                lines.push(format!(
                    "vec4 {out} = mix({edge_c}, {color}, {out}_mask) * ({out}_mask + {out}_edge);"
                ));
                outputs.push(out.clone());
                outputs.push(format!("{}_mask", out));
            }
            NodeType::Distortion => {
                let uv = input_or_default(input_vars, 0, "v_uv");
                let strength = input_or_default(input_vars, 1, "0.1");
                let dir = input_or_default(input_vars, 2, "vec2(1.0, 0.0)");
                let noise = input_or_default(input_vars, 3, "0.0");
                let out = format!("{}_dist", var_prefix);
                lines.push(format!(
                    "vec2 {} = {} + {} * {} * (1.0 + {});",
                    out, uv, dir, strength, noise
                ));
                outputs.push(out);
            }
            NodeType::Blur => {
                let uv = input_or_default(input_vars, 0, "v_uv");
                let radius = input_or_default(input_vars, 1, "2.0");
                let _samples = input_or_default(input_vars, 2, "8");
                let sampler = input_or_default(input_vars, 3, "u_texture0");
                let out = format!("{}_blur", var_prefix);
                lines.push(format!("vec4 {out} = vec4(0.0);"));
                lines.push(format!("float {out}_total = 0.0;"));
                // Unrolled 9-tap gaussian
                lines.push(format!("for (int i = -4; i <= 4; i++) {{"));
                lines.push(format!("  for (int j = -4; j <= 4; j++) {{"));
                lines.push(format!("    vec2 {out}_off = vec2(float(i), float(j)) * {radius} / 512.0;"));
                lines.push(format!("    float {out}_w = exp(-float(i*i + j*j) / (2.0 * {radius} * {radius}));"));
                lines.push(format!("    {out} += texture2D({sampler}, {uv} + {out}_off) * {out}_w;"));
                lines.push(format!("    {out}_total += {out}_w;"));
                lines.push(format!("  }}"));
                lines.push(format!("}}"));
                lines.push(format!("{out} /= max({out}_total, 0.001);"));
                outputs.push(out);
            }
            NodeType::Sharpen => {
                let uv = input_or_default(input_vars, 0, "v_uv");
                let strength = input_or_default(input_vars, 1, "1.0");
                let sampler = input_or_default(input_vars, 2, "u_texture0");
                let out = format!("{}_sharp", var_prefix);
                lines.push(format!("vec2 {out}_px = vec2(1.0/512.0, 1.0/512.0);"));
                lines.push(format!("vec4 {out}_c = texture2D({sampler}, {uv});"));
                lines.push(format!("vec4 {out}_n = texture2D({sampler}, {uv} + vec2(0.0, {out}_px.y));"));
                lines.push(format!("vec4 {out}_s = texture2D({sampler}, {uv} - vec2(0.0, {out}_px.y));"));
                lines.push(format!("vec4 {out}_e = texture2D({sampler}, {uv} + vec2({out}_px.x, 0.0));"));
                lines.push(format!("vec4 {out}_w = texture2D({sampler}, {uv} - vec2({out}_px.x, 0.0));"));
                lines.push(format!(
                    "vec4 {out} = {out}_c + ({out}_c * 4.0 - {out}_n - {out}_s - {out}_e - {out}_w) * {strength};"
                ));
                outputs.push(out);
            }
            NodeType::EdgeDetect => {
                let uv = input_or_default(input_vars, 0, "v_uv");
                let threshold = input_or_default(input_vars, 1, "0.1");
                let sampler = input_or_default(input_vars, 2, "u_texture0");
                let out = format!("{}_edge", var_prefix);
                lines.push(format!("vec2 {out}_px = vec2(1.0/512.0);"));
                lines.push(format!("float {out}_tl = dot(texture2D({sampler}, {uv} + vec2(-1.0, 1.0)*{out}_px).rgb, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("float {out}_t  = dot(texture2D({sampler}, {uv} + vec2( 0.0, 1.0)*{out}_px).rgb, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("float {out}_tr = dot(texture2D({sampler}, {uv} + vec2( 1.0, 1.0)*{out}_px).rgb, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("float {out}_l  = dot(texture2D({sampler}, {uv} + vec2(-1.0, 0.0)*{out}_px).rgb, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("float {out}_r  = dot(texture2D({sampler}, {uv} + vec2( 1.0, 0.0)*{out}_px).rgb, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("float {out}_bl = dot(texture2D({sampler}, {uv} + vec2(-1.0,-1.0)*{out}_px).rgb, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("float {out}_b  = dot(texture2D({sampler}, {uv} + vec2( 0.0,-1.0)*{out}_px).rgb, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("float {out}_br = dot(texture2D({sampler}, {uv} + vec2( 1.0,-1.0)*{out}_px).rgb, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("float {out}_gx = -1.0*{out}_tl + 1.0*{out}_tr - 2.0*{out}_l + 2.0*{out}_r - 1.0*{out}_bl + 1.0*{out}_br;"));
                lines.push(format!("float {out}_gy = -1.0*{out}_tl - 2.0*{out}_t - 1.0*{out}_tr + 1.0*{out}_bl + 2.0*{out}_b + 1.0*{out}_br;"));
                lines.push(format!("float {out} = step({threshold}, sqrt({out}_gx*{out}_gx + {out}_gy*{out}_gy));"));
                outputs.push(out);
            }
            NodeType::Outline => {
                let color = input_or_default(input_vars, 0, "vec4(1.0)");
                let _depth = input_or_default(input_vars, 1, "0.0");
                let normal = input_or_default(input_vars, 2, "v_normal");
                let width = input_or_default(input_vars, 3, "1.0");
                let outline_c = input_or_default(input_vars, 4, "vec4(0.0, 0.0, 0.0, 1.0)");
                let out = format!("{}_outline", var_prefix);
                lines.push(format!(
                    "float {out}_rim = 1.0 - abs(dot(normalize({normal}), normalize(u_camera_pos - v_position)));"
                ));
                lines.push(format!("float {out}_factor = smoothstep(1.0 - {width} * 0.1, 1.0, {out}_rim);"));
                lines.push(format!("vec4 {out} = mix({color}, {outline_c}, {out}_factor);"));
                outputs.push(out);
            }
            NodeType::Bloom => {
                let color = input_or_default(input_vars, 0, "vec4(1.0)");
                let threshold = input_or_default(input_vars, 1, "0.8");
                let intensity = input_or_default(input_vars, 2, "1.5");
                let _radius = input_or_default(input_vars, 3, "4.0");
                let out = format!("{}_bloom", var_prefix);
                lines.push(format!(
                    "float {out}_lum = dot({color}.rgb, vec3(0.299, 0.587, 0.114));"
                ));
                lines.push(format!(
                    "float {out}_mask = max(0.0, {out}_lum - {threshold}) / max(1.0 - {threshold}, 0.001);"
                ));
                lines.push(format!("vec4 {out} = {color} + {color} * {out}_mask * {intensity};"));
                outputs.push(out.clone());
                outputs.push(format!("{}_mask", out));
            }
            NodeType::ChromaticAberration => {
                let uv = input_or_default(input_vars, 0, "v_uv");
                let offset = input_or_default(input_vars, 1, "0.005");
                let sampler = input_or_default(input_vars, 2, "u_texture0");
                let out = format!("{}_ca", var_prefix);
                lines.push(format!("vec2 {out}_dir = normalize({uv} - vec2(0.5));"));
                lines.push(format!("float {out}_r = texture2D({sampler}, {uv} + {out}_dir * {offset}).r;"));
                lines.push(format!("float {out}_g = texture2D({sampler}, {uv}).g;"));
                lines.push(format!("float {out}_b = texture2D({sampler}, {uv} - {out}_dir * {offset}).b;"));
                lines.push(format!("float {out}_a = texture2D({sampler}, {uv}).a;"));
                lines.push(format!("vec4 {out} = vec4({out}_r, {out}_g, {out}_b, {out}_a);"));
                outputs.push(out);
            }

            // ── Color ───────────────────────────────────────
            NodeType::HSVToRGB => {
                let h = input_or_default(input_vars, 0, "0.0");
                let s = input_or_default(input_vars, 1, "1.0");
                let v = input_or_default(input_vars, 2, "1.0");
                let out = format!("{}_h2r", var_prefix);
                lines.push(format!("vec3 {out}_k = vec3(1.0, 2.0/3.0, 1.0/3.0);"));
                lines.push(format!("vec3 {out}_p = abs(fract(vec3({h}) + {out}_k) * 6.0 - 3.0);"));
                lines.push(format!("vec3 {out} = {v} * mix(vec3(1.0), clamp({out}_p - 1.0, 0.0, 1.0), {s});"));
                outputs.push(out);
            }
            NodeType::RGBToHSV => {
                let rgb = input_or_default(input_vars, 0, "vec3(1.0)");
                let out = format!("{}_r2h", var_prefix);
                lines.push(format!("vec4 {out}_K = vec4(0.0, -1.0/3.0, 2.0/3.0, -1.0);"));
                lines.push(format!("vec4 {out}_p = mix(vec4({rgb}.bg, {out}_K.wz), vec4({rgb}.gb, {out}_K.xy), step({rgb}.b, {rgb}.g));"));
                lines.push(format!("vec4 {out}_q = mix(vec4({out}_p.xyw, {rgb}.r), vec4({rgb}.r, {out}_p.yzx), step({out}_p.x, {rgb}.r));"));
                lines.push(format!("float {out}_d = {out}_q.x - min({out}_q.w, {out}_q.y);"));
                lines.push(format!("float {out}_e = 1.0e-10;"));
                lines.push(format!("float {out}_h = abs({out}_q.z + ({out}_q.w - {out}_q.y) / (6.0 * {out}_d + {out}_e));"));
                lines.push(format!("float {out}_s = {out}_d / ({out}_q.x + {out}_e);"));
                lines.push(format!("float {out}_v = {out}_q.x;"));
                outputs.push(format!("{}_h", out));
                outputs.push(format!("{}_s", out));
                outputs.push(format!("{}_v", out));
            }
            NodeType::Contrast => {
                let color = input_or_default(input_vars, 0, "vec3(0.5)");
                let amount = input_or_default(input_vars, 1, "1.0");
                let out = format!("{}_contrast", var_prefix);
                lines.push(format!("vec3 {out} = (({color} - 0.5) * {amount}) + 0.5;"));
                outputs.push(out);
            }
            NodeType::Saturation => {
                let color = input_or_default(input_vars, 0, "vec3(0.5)");
                let amount = input_or_default(input_vars, 1, "1.0");
                let out = format!("{}_sat", var_prefix);
                lines.push(format!("float {out}_lum = dot({color}, vec3(0.299, 0.587, 0.114));"));
                lines.push(format!("vec3 {out} = mix(vec3({out}_lum), {color}, {amount});"));
                outputs.push(out);
            }
            NodeType::Hue => {
                let color = input_or_default(input_vars, 0, "vec3(0.5)");
                let shift = input_or_default(input_vars, 1, "0.0");
                let out = format!("{}_hue", var_prefix);
                // Simple hue rotation via angle in YIQ-like space
                lines.push(format!("float {out}_angle = {shift} * 6.28318;"));
                lines.push(format!("float {out}_cs = cos({out}_angle);"));
                lines.push(format!("float {out}_sn = sin({out}_angle);"));
                lines.push(format!("vec3 {out}_w = vec3(0.299, 0.587, 0.114);"));
                lines.push(format!("vec3 {out} = vec3("));
                lines.push(format!("  dot({color}, {out}_w + vec3(1.0-0.299, -0.587, -0.114)*{out}_cs + vec3(0.0, 0.0, 1.0)*{out}_sn*0.5),"));
                lines.push(format!("  dot({color}, {out}_w + vec3(-0.299, 1.0-0.587, -0.114)*{out}_cs + vec3(0.0, 0.0, -0.5)*{out}_sn),"));
                lines.push(format!("  dot({color}, {out}_w + vec3(-0.299, -0.587, 1.0-0.114)*{out}_cs + vec3(0.0, 1.0, 0.0)*{out}_sn*0.5)"));
                lines.push(format!(");"));
                outputs.push(out);
            }
            NodeType::Invert => {
                let color = input_or_default(input_vars, 0, "vec3(0.5)");
                let out = format!("{}_inv", var_prefix);
                lines.push(format!("vec3 {out} = 1.0 - {color};"));
                outputs.push(out);
            }
            NodeType::Posterize => {
                let color = input_or_default(input_vars, 0, "vec3(0.5)");
                let levels = input_or_default(input_vars, 1, "4.0");
                let out = format!("{}_poster", var_prefix);
                lines.push(format!("vec3 {out} = floor({color} * {levels}) / {levels};"));
                outputs.push(out);
            }
            NodeType::GradientMap => {
                let t = input_or_default(input_vars, 0, "0.5");
                let color_a = input_or_default(input_vars, 1, "vec3(0.0)");
                let color_b = input_or_default(input_vars, 2, "vec3(1.0)");
                let out = format!("{}_gmap", var_prefix);
                lines.push(format!("vec3 {out} = mix({color_a}, {color_b}, clamp({t}, 0.0, 1.0));"));
                outputs.push(out);
            }

            // ── Noise ───────────────────────────────────────
            NodeType::Perlin => {
                let pos = input_or_default(input_vars, 0, "v_position");
                let scale = input_or_default(input_vars, 1, "1.0");
                let seed = input_or_default(input_vars, 2, "0.0");
                let out = format!("{}_perlin", var_prefix);
                // Perlin noise implementation
                lines.push(format!("vec3 {out}_p = {pos} * {scale} + vec3({seed});"));
                lines.push(format!("vec3 {out}_i = floor({out}_p);"));
                lines.push(format!("vec3 {out}_f = fract({out}_p);"));
                lines.push(format!("vec3 {out}_u = {out}_f * {out}_f * (3.0 - 2.0 * {out}_f);"));
                // Hash function inline
                lines.push(format!("float {out}_h000 = fract(sin(dot({out}_i, vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_h100 = fract(sin(dot({out}_i + vec3(1.0,0.0,0.0), vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_h010 = fract(sin(dot({out}_i + vec3(0.0,1.0,0.0), vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_h110 = fract(sin(dot({out}_i + vec3(1.0,1.0,0.0), vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_h001 = fract(sin(dot({out}_i + vec3(0.0,0.0,1.0), vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_h101 = fract(sin(dot({out}_i + vec3(1.0,0.0,1.0), vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_h011 = fract(sin(dot({out}_i + vec3(0.0,1.0,1.0), vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_h111 = fract(sin(dot({out}_i + vec3(1.0,1.0,1.0), vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_x0 = mix(mix({out}_h000, {out}_h100, {out}_u.x), mix({out}_h010, {out}_h110, {out}_u.x), {out}_u.y);"));
                lines.push(format!("float {out}_x1 = mix(mix({out}_h001, {out}_h101, {out}_u.x), mix({out}_h011, {out}_h111, {out}_u.x), {out}_u.y);"));
                lines.push(format!("float {out}_val = mix({out}_x0, {out}_x1, {out}_u.z);"));
                lines.push(format!("vec3 {out}_grad = normalize(vec3({out}_h100 - {out}_h000, {out}_h010 - {out}_h000, {out}_h001 - {out}_h000));"));
                outputs.push(format!("{out}_val"));
                outputs.push(format!("{out}_grad"));
            }
            NodeType::Simplex => {
                let pos = input_or_default(input_vars, 0, "v_position");
                let scale = input_or_default(input_vars, 1, "1.0");
                let seed = input_or_default(input_vars, 2, "0.0");
                let out = format!("{}_simplex", var_prefix);
                // Simplex noise approximation (using hash-based approach)
                lines.push(format!("vec3 {out}_p = {pos} * {scale} + vec3({seed});"));
                lines.push(format!("float {out}_F3 = 1.0/3.0;"));
                lines.push(format!("float {out}_s = ({out}_p.x + {out}_p.y + {out}_p.z) * {out}_F3;"));
                lines.push(format!("vec3 {out}_i = floor({out}_p + vec3({out}_s));"));
                lines.push(format!("float {out}_G3 = 1.0/6.0;"));
                lines.push(format!("float {out}_t = ({out}_i.x + {out}_i.y + {out}_i.z) * {out}_G3;"));
                lines.push(format!("vec3 {out}_x0 = {out}_p - ({out}_i - vec3({out}_t));"));
                // Simplified: use hash-based value for approximation
                lines.push(format!("float {out}_val = fract(sin(dot({out}_i, vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_val2 = fract(sin(dot({out}_i + 1.0, vec3(127.1, 311.7, 74.7))) * 43758.5453);"));
                lines.push(format!("float {out}_result = mix({out}_val, {out}_val2, fract({out}_s)) * 2.0 - 1.0;"));
                lines.push(format!("vec3 {out}_grad = normalize(vec3({out}_val, {out}_val2, {out}_result));"));
                outputs.push(format!("{out}_result"));
                outputs.push(format!("{out}_grad"));
            }
            NodeType::Voronoi => {
                let pos = input_or_default(input_vars, 0, "v_position");
                let scale = input_or_default(input_vars, 1, "1.0");
                let jitter = input_or_default(input_vars, 2, "1.0");
                let out = format!("{}_voronoi", var_prefix);
                lines.push(format!("vec3 {out}_p = {pos} * {scale};"));
                lines.push(format!("vec3 {out}_n = floor({out}_p);"));
                lines.push(format!("vec3 {out}_f = fract({out}_p);"));
                lines.push(format!("float {out}_md = 8.0;"));
                lines.push(format!("float {out}_id = 0.0;"));
                lines.push(format!("vec3 {out}_mr = vec3(0.0);"));
                lines.push(format!("for (int k = -1; k <= 1; k++) {{"));
                lines.push(format!("  for (int j = -1; j <= 1; j++) {{"));
                lines.push(format!("    for (int i = -1; i <= 1; i++) {{"));
                lines.push(format!("      vec3 {out}_g = vec3(float(i), float(j), float(k));"));
                lines.push(format!("      vec3 {out}_cell = {out}_n + {out}_g;"));
                lines.push(format!("      vec3 {out}_o = fract(sin(vec3(dot({out}_cell, vec3(127.1,311.7,74.7)), dot({out}_cell, vec3(269.5,183.3,246.1)), dot({out}_cell, vec3(113.5,271.9,124.6)))) * 43758.5453) * {jitter};"));
                lines.push(format!("      vec3 {out}_r = {out}_g + {out}_o - {out}_f;"));
                lines.push(format!("      float {out}_d = dot({out}_r, {out}_r);"));
                lines.push(format!("      if ({out}_d < {out}_md) {{"));
                lines.push(format!("        {out}_md = {out}_d;"));
                lines.push(format!("        {out}_id = dot({out}_cell, vec3(7.0, 157.0, 113.0));"));
                lines.push(format!("        {out}_mr = {out}_r;"));
                lines.push(format!("      }}"));
                lines.push(format!("    }}"));
                lines.push(format!("  }}"));
                lines.push(format!("}}"));
                outputs.push(format!("sqrt({out}_md)"));
                outputs.push(format!("fract({out}_id)"));
                outputs.push(format!("{out}_mr"));
            }
            NodeType::FBM => {
                let pos = input_or_default(input_vars, 0, "v_position");
                let scale = input_or_default(input_vars, 1, "1.0");
                let _octaves = input_or_default(input_vars, 2, "4");
                let lacunarity = input_or_default(input_vars, 3, "2.0");
                let gain = input_or_default(input_vars, 4, "0.5");
                let out = format!("{}_fbm", var_prefix);
                lines.push(format!("float {out} = 0.0;"));
                lines.push(format!("float {out}_amp = 1.0;"));
                lines.push(format!("vec3 {out}_p = {pos} * {scale};"));
                lines.push(format!("for (int {out}_i = 0; {out}_i < 4; {out}_i++) {{"));
                lines.push(format!("  vec3 {out}_fi = floor({out}_p);"));
                lines.push(format!("  vec3 {out}_ff = fract({out}_p);"));
                lines.push(format!("  vec3 {out}_fu = {out}_ff*{out}_ff*(3.0-2.0*{out}_ff);"));
                lines.push(format!("  float {out}_a = fract(sin(dot({out}_fi, vec3(127.1,311.7,74.7)))*43758.5453);"));
                lines.push(format!("  float {out}_b = fract(sin(dot({out}_fi+vec3(1,0,0), vec3(127.1,311.7,74.7)))*43758.5453);"));
                lines.push(format!("  float {out}_c = fract(sin(dot({out}_fi+vec3(0,1,0), vec3(127.1,311.7,74.7)))*43758.5453);"));
                lines.push(format!("  float {out}_d = fract(sin(dot({out}_fi+vec3(1,1,0), vec3(127.1,311.7,74.7)))*43758.5453);"));
                lines.push(format!("  float {out}_v = mix(mix({out}_a,{out}_b,{out}_fu.x), mix({out}_c,{out}_d,{out}_fu.x), {out}_fu.y);"));
                lines.push(format!("  {out} += {out}_amp * {out}_v;"));
                lines.push(format!("  {out}_amp *= {gain};"));
                lines.push(format!("  {out}_p *= {lacunarity};"));
                lines.push(format!("}}"));
                outputs.push(out);
            }
            NodeType::Turbulence => {
                let pos = input_or_default(input_vars, 0, "v_position");
                let scale = input_or_default(input_vars, 1, "1.0");
                let _octaves = input_or_default(input_vars, 2, "4");
                let lacunarity = input_or_default(input_vars, 3, "2.0");
                let gain = input_or_default(input_vars, 4, "0.5");
                let out = format!("{}_turb", var_prefix);
                lines.push(format!("float {out} = 0.0;"));
                lines.push(format!("float {out}_amp = 1.0;"));
                lines.push(format!("vec3 {out}_p = {pos} * {scale};"));
                lines.push(format!("for (int {out}_i = 0; {out}_i < 4; {out}_i++) {{"));
                lines.push(format!("  vec3 {out}_fi = floor({out}_p);"));
                lines.push(format!("  vec3 {out}_ff = fract({out}_p);"));
                lines.push(format!("  vec3 {out}_fu = {out}_ff*{out}_ff*(3.0-2.0*{out}_ff);"));
                lines.push(format!("  float {out}_a = fract(sin(dot({out}_fi, vec3(127.1,311.7,74.7)))*43758.5453);"));
                lines.push(format!("  float {out}_b = fract(sin(dot({out}_fi+vec3(1,0,0), vec3(127.1,311.7,74.7)))*43758.5453);"));
                lines.push(format!("  float {out}_c = fract(sin(dot({out}_fi+vec3(0,1,0), vec3(127.1,311.7,74.7)))*43758.5453);"));
                lines.push(format!("  float {out}_d = fract(sin(dot({out}_fi+vec3(1,1,0), vec3(127.1,311.7,74.7)))*43758.5453);"));
                lines.push(format!("  float {out}_v = mix(mix({out}_a,{out}_b,{out}_fu.x), mix({out}_c,{out}_d,{out}_fu.x), {out}_fu.y);"));
                lines.push(format!("  {out} += {out}_amp * abs({out}_v * 2.0 - 1.0);"));
                lines.push(format!("  {out}_amp *= {gain};"));
                lines.push(format!("  {out}_p *= {lacunarity};"));
                lines.push(format!("}}"));
                outputs.push(out);
            }

            // ── Outputs ─────────────────────────────────────
            NodeType::MainColor => {
                let color = input_or_default(input_vars, 0, "vec4(1.0)");
                lines.push(format!("gl_FragColor = {};", color));
            }
            NodeType::EmissionBuffer => {
                let emission = input_or_default(input_vars, 0, "vec4(0.0)");
                lines.push(format!("gl_FragData[1] = {};", emission));
            }
            NodeType::BloomBuffer => {
                let bloom = input_or_default(input_vars, 0, "vec4(0.0)");
                lines.push(format!("gl_FragData[2] = {};", bloom));
            }
            NodeType::NormalOutput => {
                let normal = input_or_default(input_vars, 0, "v_normal");
                lines.push(format!("gl_FragData[3] = vec4({} * 0.5 + 0.5, 1.0);", normal));
            }
        }

        GlslSnippet { lines, output_vars: outputs }
    }
}

fn input_or_default(input_vars: &[String], index: usize, default: &str) -> String {
    if index < input_vars.len() && !input_vars[index].is_empty() {
        input_vars[index].clone()
    } else {
        default.to_string()
    }
}

// ---------------------------------------------------------------------------
// GLSL snippet result
// ---------------------------------------------------------------------------

/// Result of generating GLSL for a single node.
#[derive(Debug, Clone)]
pub struct GlslSnippet {
    /// Lines of GLSL code to insert.
    pub lines: Vec<String>,
    /// GLSL expressions for each output socket.
    pub output_vars: Vec<String>,
}

// ---------------------------------------------------------------------------
// Shader Node — a concrete instance in a graph
// ---------------------------------------------------------------------------

/// A single node instance in a shader graph.
#[derive(Debug, Clone)]
pub struct ShaderNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub label: String,
    pub inputs: Vec<Socket>,
    pub outputs: Vec<Socket>,
    /// Extra per-node properties keyed by name.
    pub properties: HashMap<String, ParamValue>,
    /// Position in the editor (for serialization).
    pub editor_x: f32,
    pub editor_y: f32,
    /// Whether this node is enabled. Disabled nodes are skipped during compilation.
    pub enabled: bool,
    /// Conditional: if set, this node is only active when the named game state variable
    /// exceeds the threshold.
    pub conditional_var: Option<String>,
    pub conditional_threshold: f32,
}

impl ShaderNode {
    /// Create a new node with default sockets for the given type.
    pub fn new(id: NodeId, node_type: NodeType) -> Self {
        let inputs = node_type.default_inputs();
        let outputs = node_type.default_outputs();
        let label = node_type.display_name().to_string();
        Self {
            id,
            node_type,
            label,
            inputs,
            outputs,
            properties: HashMap::new(),
            editor_x: 0.0,
            editor_y: 0.0,
            enabled: true,
            conditional_var: None,
            conditional_threshold: 0.0,
        }
    }

    /// Set editor position.
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.editor_x = x;
        self.editor_y = y;
        self
    }

    /// Set a property value.
    pub fn with_property(mut self, key: &str, value: ParamValue) -> Self {
        self.properties.insert(key.to_string(), value);
        self
    }

    /// Set a default value for an input socket by name.
    pub fn with_input_default(mut self, socket_name: &str, value: ParamValue) -> Self {
        for s in &mut self.inputs {
            if s.name == socket_name {
                s.default_value = Some(value.clone());
            }
        }
        self
    }

    /// Set the conditional gate on this node.
    pub fn with_condition(mut self, var_name: &str, threshold: f32) -> Self {
        self.conditional_var = Some(var_name.to_string());
        self.conditional_threshold = threshold;
        self
    }

    /// Get the output socket data type at the given index.
    pub fn output_type(&self, index: usize) -> Option<DataType> {
        self.outputs.get(index).map(|s| s.data_type)
    }

    /// Get the input socket data type at the given index.
    pub fn input_type(&self, index: usize) -> Option<DataType> {
        self.inputs.get(index).map(|s| s.data_type)
    }

    /// Get the default value for input socket at the given index.
    pub fn input_default(&self, index: usize) -> Option<&ParamValue> {
        self.inputs.get(index).and_then(|s| s.default_value.as_ref())
    }

    /// Return the GLSL variable prefix for this node.
    pub fn var_prefix(&self) -> String {
        format!("n{}", self.id.0)
    }

    /// Compute a rough cost estimate for this node.
    pub fn estimated_cost(&self) -> u32 {
        self.node_type.instruction_cost()
    }
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

/// A connection between two sockets in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Connection {
    pub from_node: NodeId,
    pub from_socket: usize,
    pub to_node: NodeId,
    pub to_socket: usize,
}

impl Connection {
    pub fn new(from_node: NodeId, from_socket: usize, to_node: NodeId, to_socket: usize) -> Self {
        Self { from_node, from_socket, to_node, to_socket }
    }
}

// ---------------------------------------------------------------------------
// ShaderGraph — the full graph container
// ---------------------------------------------------------------------------

/// A complete shader graph containing nodes and connections.
#[derive(Debug, Clone)]
pub struct ShaderGraph {
    pub name: String,
    nodes: HashMap<NodeId, ShaderNode>,
    connections: Vec<Connection>,
    next_id: u64,
}

impl ShaderGraph {
    /// Create a new empty shader graph.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            nodes: HashMap::new(),
            connections: Vec::new(),
            next_id: 1,
        }
    }

    /// Allocate a fresh node ID.
    pub fn alloc_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Add a node to the graph, returning its ID.
    pub fn add_node(&mut self, node_type: NodeType) -> NodeId {
        let id = self.alloc_id();
        let node = ShaderNode::new(id, node_type);
        self.nodes.insert(id, node);
        id
    }

    /// Add a pre-built node to the graph.
    pub fn add_node_with(&mut self, mut node: ShaderNode) -> NodeId {
        let id = self.alloc_id();
        node.id = id;
        self.nodes.insert(id, node);
        id
    }

    /// Add a pre-built node, reusing its existing ID if set, or allocating one.
    pub fn insert_node(&mut self, node: ShaderNode) -> NodeId {
        let id = node.id;
        if id.0 >= self.next_id {
            self.next_id = id.0 + 1;
        }
        self.nodes.insert(id, node);
        id
    }

    /// Connect an output socket of one node to an input socket of another.
    pub fn connect(&mut self, from_node: NodeId, from_socket: usize, to_node: NodeId, to_socket: usize) {
        // Remove any existing connection to this input socket
        self.connections.retain(|c| !(c.to_node == to_node && c.to_socket == to_socket));
        self.connections.push(Connection::new(from_node, from_socket, to_node, to_socket));
    }

    /// Remove all connections involving a node.
    pub fn disconnect_node(&mut self, node_id: NodeId) {
        self.connections.retain(|c| c.from_node != node_id && c.to_node != node_id);
    }

    /// Remove a specific connection.
    pub fn disconnect(&mut self, from_node: NodeId, from_socket: usize, to_node: NodeId, to_socket: usize) {
        self.connections.retain(|c| {
            !(c.from_node == from_node && c.from_socket == from_socket
              && c.to_node == to_node && c.to_socket == to_socket)
        });
    }

    /// Remove a node and all its connections.
    pub fn remove_node(&mut self, node_id: NodeId) {
        self.disconnect_node(node_id);
        self.nodes.remove(&node_id);
    }

    /// Get a reference to a node.
    pub fn node(&self, id: impl std::borrow::Borrow<NodeId>) -> Option<&ShaderNode> {
        self.nodes.get(id.borrow())
    }

    /// Get a mutable reference to a node.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut ShaderNode> {
        self.nodes.get_mut(&id)
    }

    /// Iterate over all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &ShaderNode> {
        self.nodes.values()
    }

    /// Iterate over all node IDs.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.keys().copied()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get all connections.
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Get connections feeding into a specific node's input socket.
    pub fn incoming_connections(&self, node_id: NodeId) -> Vec<&Connection> {
        self.connections.iter().filter(|c| c.to_node == node_id).collect()
    }

    /// Get connections going out of a specific node.
    pub fn outgoing_connections(&self, node_id: NodeId) -> Vec<&Connection> {
        self.connections.iter().filter(|c| c.from_node == node_id).collect()
    }

    /// Find all output nodes (MainColor, EmissionBuffer, BloomBuffer, NormalOutput).
    pub fn output_nodes(&self) -> Vec<NodeId> {
        self.nodes.values()
            .filter(|n| n.node_type.is_output())
            .map(|n| n.id)
            .collect()
    }

    /// Find all source nodes (no required input connections).
    pub fn source_nodes(&self) -> Vec<NodeId> {
        self.nodes.values()
            .filter(|n| n.node_type.is_source())
            .map(|n| n.id)
            .collect()
    }

    /// Compute the total estimated instruction cost of the graph.
    pub fn estimated_cost(&self) -> u32 {
        self.nodes.values().map(|n| n.estimated_cost()).sum()
    }

    /// Validate basic graph integrity: all connections reference existing nodes,
    /// socket indices are in range, no self-loops.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for conn in &self.connections {
            if !self.nodes.contains_key(&conn.from_node) {
                errors.push(format!("Connection references missing source node {}", conn.from_node.0));
            }
            if !self.nodes.contains_key(&conn.to_node) {
                errors.push(format!("Connection references missing target node {}", conn.to_node.0));
            }
            if conn.from_node == conn.to_node {
                errors.push(format!("Self-loop on node {}", conn.from_node.0));
            }
            if let Some(src) = self.nodes.get(&conn.from_node) {
                if conn.from_socket >= src.outputs.len() {
                    errors.push(format!(
                        "Node {} output socket {} out of range (has {})",
                        conn.from_node.0, conn.from_socket, src.outputs.len()
                    ));
                }
            }
            if let Some(dst) = self.nodes.get(&conn.to_node) {
                if conn.to_socket >= dst.inputs.len() {
                    errors.push(format!(
                        "Node {} input socket {} out of range (has {})",
                        conn.to_node.0, conn.to_socket, dst.inputs.len()
                    ));
                }
            }
        }
        // Check for duplicate input connections (two connections to same input)
        let mut seen_inputs: HashMap<(u64, usize), u64> = HashMap::new();
        for conn in &self.connections {
            let key = (conn.to_node.0, conn.to_socket);
            if let Some(prev) = seen_inputs.insert(key, conn.from_node.0) {
                errors.push(format!(
                    "Duplicate connection to node {} socket {}: from {} and {}",
                    conn.to_node.0, conn.to_socket, prev, conn.from_node.0
                ));
            }
        }
        errors
    }

    /// Compute a topology hash for caching/deduplication.
    pub fn topology_hash(&self) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325; // FNV offset
        let prime: u64 = 0x100000001b3;

        // Sort node IDs for deterministic hashing
        let mut node_ids: Vec<u64> = self.nodes.keys().map(|k| k.0).collect();
        node_ids.sort();

        for nid in &node_ids {
            hash ^= *nid;
            hash = hash.wrapping_mul(prime);
            if let Some(node) = self.nodes.get(&NodeId(*nid)) {
                // Hash node type via its display name
                for b in node.node_type.display_name().bytes() {
                    hash ^= b as u64;
                    hash = hash.wrapping_mul(prime);
                }
            }
        }

        // Hash connections
        let mut conns: Vec<(u64, usize, u64, usize)> = self.connections.iter()
            .map(|c| (c.from_node.0, c.from_socket, c.to_node.0, c.to_socket))
            .collect();
        conns.sort();
        for (a, b, c, d) in conns {
            hash ^= a;
            hash = hash.wrapping_mul(prime);
            hash ^= b as u64;
            hash = hash.wrapping_mul(prime);
            hash ^= c;
            hash = hash.wrapping_mul(prime);
            hash ^= d as u64;
            hash = hash.wrapping_mul(prime);
        }

        hash
    }

    /// Return the internal nodes map (used for serialization).
    pub fn nodes_map(&self) -> &HashMap<NodeId, ShaderNode> {
        &self.nodes
    }

    /// Return the next_id counter (used for serialization).
    pub fn next_id_counter(&self) -> u64 {
        self.next_id
    }

    /// Set the next_id counter (used during deserialization).
    pub fn set_next_id(&mut self, val: u64) {
        self.next_id = val;
    }

    /// Add a raw connection without duplicate-input removal (used during deserialization).
    pub fn add_connection_raw(&mut self, conn: Connection) {
        self.connections.push(conn);
    }
}

impl Default for ShaderGraph {
    fn default() -> Self {
        Self::new("untitled")
    }
}
