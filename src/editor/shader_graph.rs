// shader_graph.rs — Visual shader graph editor for proof-engine
// Builds GLSL fragment/vertex shaders from connected node networks.
// Supports PBR, unlit, post-processing, and custom compute passes.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

// ─── Value types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderValueType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
    Int,
    IVec2,
    IVec3,
    IVec4,
    UInt,
    Bool,
    Sampler2D,
    SamplerCube,
    Sampler2DArray,
    Sampler3D,
    Void,
}

impl ShaderValueType {
    pub fn glsl_type(&self) -> &'static str {
        match self {
            Self::Float        => "float",
            Self::Vec2         => "vec2",
            Self::Vec3         => "vec3",
            Self::Vec4         => "vec4",
            Self::Mat2         => "mat2",
            Self::Mat3         => "mat3",
            Self::Mat4         => "mat4",
            Self::Int          => "int",
            Self::IVec2        => "ivec2",
            Self::IVec3        => "ivec3",
            Self::IVec4        => "ivec4",
            Self::UInt         => "uint",
            Self::Bool         => "bool",
            Self::Sampler2D    => "sampler2D",
            Self::SamplerCube  => "samplerCube",
            Self::Sampler2DArray => "sampler2DArray",
            Self::Sampler3D    => "sampler3D",
            Self::Void         => "void",
        }
    }

    pub fn component_count(&self) -> usize {
        match self {
            Self::Float | Self::Int | Self::UInt | Self::Bool => 1,
            Self::Vec2 | Self::IVec2 => 2,
            Self::Vec3 | Self::IVec3 => 3,
            Self::Vec4 | Self::IVec4 => 4,
            Self::Mat2 => 4,
            Self::Mat3 => 9,
            Self::Mat4 => 16,
            _ => 0,
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Self::Float | Self::Vec2 | Self::Vec3 | Self::Vec4
            | Self::Int | Self::IVec2 | Self::IVec3 | Self::IVec4 | Self::UInt)
    }

    pub fn is_texture(&self) -> bool {
        matches!(self, Self::Sampler2D | Self::SamplerCube
            | Self::Sampler2DArray | Self::Sampler3D)
    }

    pub fn can_connect_to(&self, target: ShaderValueType) -> bool {
        if *self == target { return true; }
        // Allow float → vecN promotion
        if *self == Self::Float && matches!(target, Self::Vec2 | Self::Vec3 | Self::Vec4) {
            return true;
        }
        // Allow vec3 → vec4 (appends 1.0)
        if *self == Self::Vec3 && target == Self::Vec4 { return true; }
        // Allow int → float
        if *self == Self::Int && target == Self::Float { return true; }
        false
    }

    pub fn coerce_expr(&self, target: ShaderValueType, expr: &str) -> String {
        if *self == target { return expr.to_string(); }
        match (*self, target) {
            (Self::Float, Self::Vec2) => format!("vec2({})", expr),
            (Self::Float, Self::Vec3) => format!("vec3({})", expr),
            (Self::Float, Self::Vec4) => format!("vec4({}, 1.0)", expr),
            (Self::Vec3,  Self::Vec4) => format!("vec4({}, 1.0)", expr),
            (Self::Int,   Self::Float) => format!("float({})", expr),
            _ => expr.to_string(),
        }
    }

    pub fn default_value(&self) -> &'static str {
        match self {
            Self::Float => "0.0",
            Self::Vec2  => "vec2(0.0)",
            Self::Vec3  => "vec3(0.0)",
            Self::Vec4  => "vec4(0.0)",
            Self::Int   => "0",
            Self::Bool  => "false",
            _           => "0.0",
        }
    }
}

impl fmt::Display for ShaderValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.glsl_type())
    }
}

// ─── Port definitions ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderNodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderPortId {
    pub node: ShaderNodeId,
    pub port: u16,
    pub is_output: bool,
}

impl ShaderPortId {
    pub fn input(node: ShaderNodeId, port: u16) -> Self {
        Self { node, port, is_output: false }
    }
    pub fn output(node: ShaderNodeId, port: u16) -> Self {
        Self { node, port, is_output: true }
    }
}

#[derive(Debug, Clone)]
pub struct PortDef {
    pub name: &'static str,
    pub value_type: ShaderValueType,
    pub optional: bool,
    pub default_expr: Option<String>,
}

impl PortDef {
    pub fn required(name: &'static str, ty: ShaderValueType) -> Self {
        Self { name, value_type: ty, optional: false, default_expr: None }
    }
    pub fn optional(name: &'static str, ty: ShaderValueType, default: &str) -> Self {
        Self { name, value_type: ty, optional: true, default_expr: Some(default.to_string()) }
    }
}

#[derive(Debug, Clone)]
pub struct ShaderConnection {
    pub from: ShaderPortId,
    pub to: ShaderPortId,
}

// ─── Node kinds ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ShaderNodeKind {
    // Inputs
    ConstFloat(f32),
    ConstVec2([f32; 2]),
    ConstVec3([f32; 3]),
    ConstVec4([f32; 4]),
    ConstInt(i32),
    ConstBool(bool),
    Time,
    Resolution,
    FragCoord,
    VertexNormal,
    VertexTangent,
    VertexUV,
    VertexUV2,
    VertexColor,
    WorldPosition,
    ViewDirection,
    CameraPosition,
    ModelMatrix,
    ViewMatrix,
    ProjectionMatrix,
    NormalMatrix,
    CustomUniformFloat { name: String },
    CustomUniformVec3 { name: String },
    CustomUniformVec4 { name: String },
    CustomUniformSampler2D { name: String },

    // Math — arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    SquareRoot,
    AbsoluteValue,
    Negate,
    OneMinus,
    Reciprocal,
    Floor,
    Ceiling,
    Round,
    Fraction,
    Sign,
    Modulo,
    Min,
    Max,
    Clamp,
    Saturate,
    Lerp,
    SmoothStep,
    Step,

    // Math — trig
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Degrees,
    Radians,

    // Math — exponential / log
    Exp,
    Exp2,
    Log,
    Log2,

    // Vector ops
    Dot,
    Cross,
    Length,
    Normalize,
    Reflect,
    Refract,
    FaceForward,
    Distance,
    Mix,
    VectorSplit,   // vec3 → x, y, z
    VectorMerge,   // x, y, z → vec3
    VectorSwizzle { swizzle: [u8; 4], out_count: u8 },

    // Matrix ops
    MatrixMultiply,
    MatrixTranspose,
    MatrixInverse,
    TransformPoint,
    TransformVector,
    TransformNormal,

    // Texture sampling
    SampleTexture2D,
    SampleTextureCube,
    SampleTexture2DLod,
    SampleTexture2DGrad,
    SampleNormalMap,
    TextureSize,

    // Color ops
    ColorToLinear,
    ColorToGamma,
    HsvToRgb,
    RgbToHsv,
    Luminance,
    ColorBalance,
    Hue,
    Saturation,
    Brightness,

    // PBR nodes
    FresnelSchlick,
    GGXDistribution,
    SmithGeometry,
    BRDFSpecular,
    BRDFDiffuse,
    EnvBRDFApprox,
    SubsurfaceApprox,
    EmissionBlend,

    // SDF integration
    SdfSample { sdf_graph_id: u32 },
    SdfNormal { sdf_graph_id: u32 },
    SdfAO { sdf_graph_id: u32 },

    // Control flow / utility
    IfElse,
    IsNaN,
    IsInf,
    Dpdx,
    Dpdy,
    FWidth,
    Noise2D,
    Noise3D,
    VoronoiNoise,
    FbmNoise { octaves: u8 },
    CellularNoise,
    WhiteNoise,
    GradientNoise,

    // Output
    PbrOutput,
    UnlitOutput,
    PostProcessOutput,
    CustomOutput { name: String, value_type: ShaderValueType },
    VertexOffset,
    DepthOutput,
    CustomVarying { name: String, value_type: ShaderValueType },

    // Grouping / organization
    Reroute,
    Comment { text: String, width: f32, height: f32 },
    Group { label: String, nodes: Vec<ShaderNodeId> },
    SubGraph { graph_id: u32, label: String },
}

impl ShaderNodeKind {
    pub fn label(&self) -> String {
        match self {
            Self::ConstFloat(v)  => format!("{:.3}", v),
            Self::ConstVec2(v)   => format!("({:.2}, {:.2})", v[0], v[1]),
            Self::ConstVec3(v)   => format!("({:.2},{:.2},{:.2})", v[0], v[1], v[2]),
            Self::ConstVec4(v)   => format!("({:.2},{:.2},{:.2},{:.2})", v[0],v[1],v[2],v[3]),
            Self::ConstInt(v)    => format!("{}", v),
            Self::ConstBool(v)   => format!("{}", v),
            Self::Time           => "Time".into(),
            Self::Resolution     => "Resolution".into(),
            Self::FragCoord      => "FragCoord".into(),
            Self::VertexNormal   => "Vertex Normal".into(),
            Self::VertexTangent  => "Vertex Tangent".into(),
            Self::VertexUV       => "UV0".into(),
            Self::VertexUV2      => "UV1".into(),
            Self::VertexColor    => "Vertex Color".into(),
            Self::WorldPosition  => "World Position".into(),
            Self::ViewDirection  => "View Dir".into(),
            Self::CameraPosition => "Camera Position".into(),
            Self::ModelMatrix    => "Model Matrix".into(),
            Self::ViewMatrix     => "View Matrix".into(),
            Self::ProjectionMatrix => "Projection Matrix".into(),
            Self::NormalMatrix   => "Normal Matrix".into(),
            Self::CustomUniformFloat { name } => format!("Uniform: {}", name),
            Self::CustomUniformVec3 { name }  => format!("Uniform: {}", name),
            Self::CustomUniformVec4 { name }  => format!("Uniform: {}", name),
            Self::CustomUniformSampler2D { name } => format!("Texture: {}", name),
            Self::Add            => "Add".into(),
            Self::Subtract       => "Subtract".into(),
            Self::Multiply       => "Multiply".into(),
            Self::Divide         => "Divide".into(),
            Self::Power          => "Power".into(),
            Self::SquareRoot     => "Sqrt".into(),
            Self::AbsoluteValue  => "Abs".into(),
            Self::Negate         => "Negate".into(),
            Self::OneMinus       => "1 - x".into(),
            Self::Reciprocal     => "1 / x".into(),
            Self::Floor          => "Floor".into(),
            Self::Ceiling        => "Ceil".into(),
            Self::Round          => "Round".into(),
            Self::Fraction       => "Frac".into(),
            Self::Sign           => "Sign".into(),
            Self::Modulo         => "Modulo".into(),
            Self::Min            => "Min".into(),
            Self::Max            => "Max".into(),
            Self::Clamp          => "Clamp".into(),
            Self::Saturate       => "Saturate".into(),
            Self::Lerp           => "Lerp".into(),
            Self::SmoothStep     => "Smoothstep".into(),
            Self::Step           => "Step".into(),
            Self::Sin            => "Sin".into(),
            Self::Cos            => "Cos".into(),
            Self::Tan            => "Tan".into(),
            Self::Asin           => "Asin".into(),
            Self::Acos           => "Acos".into(),
            Self::Atan           => "Atan".into(),
            Self::Atan2          => "Atan2".into(),
            Self::Degrees        => "Degrees".into(),
            Self::Radians        => "Radians".into(),
            Self::Exp            => "Exp".into(),
            Self::Exp2           => "Exp2".into(),
            Self::Log            => "Log".into(),
            Self::Log2           => "Log2".into(),
            Self::Dot            => "Dot".into(),
            Self::Cross          => "Cross".into(),
            Self::Length         => "Length".into(),
            Self::Normalize      => "Normalize".into(),
            Self::Reflect        => "Reflect".into(),
            Self::Refract        => "Refract".into(),
            Self::FaceForward    => "FaceForward".into(),
            Self::Distance       => "Distance".into(),
            Self::Mix            => "Mix".into(),
            Self::VectorSplit    => "Split".into(),
            Self::VectorMerge    => "Merge".into(),
            Self::VectorSwizzle { swizzle, out_count } => {
                let names = ['x','y','z','w'];
                let s: String = swizzle[..*out_count as usize].iter()
                    .map(|&i| names[i as usize % 4])
                    .collect();
                format!(".{}", s)
            }
            Self::MatrixMultiply  => "MatMul".into(),
            Self::MatrixTranspose => "Transpose".into(),
            Self::MatrixInverse   => "Inverse".into(),
            Self::TransformPoint  => "Transform Point".into(),
            Self::TransformVector => "Transform Vector".into(),
            Self::TransformNormal => "Transform Normal".into(),
            Self::SampleTexture2D     => "Sample 2D".into(),
            Self::SampleTextureCube   => "Sample Cube".into(),
            Self::SampleTexture2DLod  => "Sample 2D LOD".into(),
            Self::SampleTexture2DGrad => "Sample 2D Grad".into(),
            Self::SampleNormalMap     => "Normal Map".into(),
            Self::TextureSize         => "Texture Size".into(),
            Self::ColorToLinear  => "Linear".into(),
            Self::ColorToGamma   => "Gamma".into(),
            Self::HsvToRgb       => "HSV→RGB".into(),
            Self::RgbToHsv       => "RGB→HSV".into(),
            Self::Luminance      => "Luminance".into(),
            Self::ColorBalance   => "Color Balance".into(),
            Self::Hue            => "Hue".into(),
            Self::Saturation     => "Saturation".into(),
            Self::Brightness     => "Brightness".into(),
            Self::FresnelSchlick  => "Fresnel".into(),
            Self::GGXDistribution => "GGX NDF".into(),
            Self::SmithGeometry   => "Smith G".into(),
            Self::BRDFSpecular    => "BRDF Specular".into(),
            Self::BRDFDiffuse     => "BRDF Diffuse".into(),
            Self::EnvBRDFApprox   => "Env BRDF".into(),
            Self::SubsurfaceApprox => "SSS Approx".into(),
            Self::EmissionBlend   => "Emission".into(),
            Self::SdfSample { sdf_graph_id } => format!("SDF Sample #{}", sdf_graph_id),
            Self::SdfNormal { sdf_graph_id } => format!("SDF Normal #{}", sdf_graph_id),
            Self::SdfAO    { sdf_graph_id } => format!("SDF AO #{}", sdf_graph_id),
            Self::IfElse     => "If/Else".into(),
            Self::IsNaN      => "IsNaN".into(),
            Self::IsInf      => "IsInf".into(),
            Self::Dpdx       => "dFdx".into(),
            Self::Dpdy       => "dFdy".into(),
            Self::FWidth     => "fwidth".into(),
            Self::Noise2D    => "Noise 2D".into(),
            Self::Noise3D    => "Noise 3D".into(),
            Self::VoronoiNoise => "Voronoi".into(),
            Self::FbmNoise { octaves } => format!("FBM ({}oct)", octaves),
            Self::CellularNoise => "Cellular".into(),
            Self::WhiteNoise  => "White Noise".into(),
            Self::GradientNoise => "Gradient Noise".into(),
            Self::PbrOutput         => "PBR Output".into(),
            Self::UnlitOutput       => "Unlit Output".into(),
            Self::PostProcessOutput => "Post Output".into(),
            Self::CustomOutput { name, .. } => format!("Output: {}", name),
            Self::VertexOffset  => "Vertex Offset".into(),
            Self::DepthOutput   => "Depth Output".into(),
            Self::CustomVarying { name, .. } => format!("Varying: {}", name),
            Self::Reroute       => "•".into(),
            Self::Comment { text, .. } => text.chars().take(32).collect(),
            Self::Group { label, .. } => label.clone(),
            Self::SubGraph { label, .. } => format!("[{}]", label),
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::ConstFloat(_) | Self::ConstVec2(_) | Self::ConstVec3(_)
            | Self::ConstVec4(_) | Self::ConstInt(_) | Self::ConstBool(_)
            | Self::Time | Self::Resolution | Self::FragCoord
            | Self::VertexNormal | Self::VertexTangent | Self::VertexUV
            | Self::VertexUV2 | Self::VertexColor | Self::WorldPosition
            | Self::ViewDirection | Self::CameraPosition
            | Self::ModelMatrix | Self::ViewMatrix | Self::ProjectionMatrix
            | Self::NormalMatrix | Self::CustomUniformFloat { .. }
            | Self::CustomUniformVec3 { .. } | Self::CustomUniformVec4 { .. }
            | Self::CustomUniformSampler2D { .. } => "Input",

            Self::Add | Self::Subtract | Self::Multiply | Self::Divide
            | Self::Power | Self::SquareRoot | Self::AbsoluteValue
            | Self::Negate | Self::OneMinus | Self::Reciprocal
            | Self::Floor | Self::Ceiling | Self::Round | Self::Fraction
            | Self::Sign | Self::Modulo | Self::Min | Self::Max
            | Self::Clamp | Self::Saturate | Self::Lerp
            | Self::SmoothStep | Self::Step => "Math",

            Self::Sin | Self::Cos | Self::Tan | Self::Asin | Self::Acos
            | Self::Atan | Self::Atan2 | Self::Degrees | Self::Radians => "Trig",

            Self::Exp | Self::Exp2 | Self::Log | Self::Log2 => "Exponential",

            Self::Dot | Self::Cross | Self::Length | Self::Normalize
            | Self::Reflect | Self::Refract | Self::FaceForward
            | Self::Distance | Self::Mix | Self::VectorSplit
            | Self::VectorMerge | Self::VectorSwizzle { .. } => "Vector",

            Self::MatrixMultiply | Self::MatrixTranspose | Self::MatrixInverse
            | Self::TransformPoint | Self::TransformVector
            | Self::TransformNormal => "Matrix",

            Self::SampleTexture2D | Self::SampleTextureCube
            | Self::SampleTexture2DLod | Self::SampleTexture2DGrad
            | Self::SampleNormalMap | Self::TextureSize => "Texture",

            Self::ColorToLinear | Self::ColorToGamma | Self::HsvToRgb
            | Self::RgbToHsv | Self::Luminance | Self::ColorBalance
            | Self::Hue | Self::Saturation | Self::Brightness => "Color",

            Self::FresnelSchlick | Self::GGXDistribution | Self::SmithGeometry
            | Self::BRDFSpecular | Self::BRDFDiffuse | Self::EnvBRDFApprox
            | Self::SubsurfaceApprox | Self::EmissionBlend => "PBR",

            Self::SdfSample { .. } | Self::SdfNormal { .. }
            | Self::SdfAO { .. } => "SDF",

            Self::IfElse | Self::IsNaN | Self::IsInf | Self::Dpdx
            | Self::Dpdy | Self::FWidth => "Utility",

            Self::Noise2D | Self::Noise3D | Self::VoronoiNoise
            | Self::FbmNoise { .. } | Self::CellularNoise
            | Self::WhiteNoise | Self::GradientNoise => "Noise",

            Self::PbrOutput | Self::UnlitOutput | Self::PostProcessOutput
            | Self::CustomOutput { .. } | Self::VertexOffset
            | Self::DepthOutput | Self::CustomVarying { .. } => "Output",

            Self::Reroute | Self::Comment { .. }
            | Self::Group { .. } | Self::SubGraph { .. } => "Organization",
        }
    }

    /// Returns (input_ports, output_ports)
    pub fn port_definitions(&self) -> (Vec<PortDef>, Vec<PortDef>) {
        use ShaderValueType::*;
        match self {
            Self::ConstFloat(_) => (vec![], vec![PortDef::required("value", Float)]),
            Self::ConstVec2(_)  => (vec![], vec![PortDef::required("value", Vec2)]),
            Self::ConstVec3(_)  => (vec![], vec![PortDef::required("value", Vec3)]),
            Self::ConstVec4(_)  => (vec![], vec![PortDef::required("value", Vec4)]),
            Self::ConstInt(_)   => (vec![], vec![PortDef::required("value", Int)]),
            Self::ConstBool(_)  => (vec![], vec![PortDef::required("value", Bool)]),

            Self::Time          => (vec![], vec![PortDef::required("time", Float)]),
            Self::Resolution    => (vec![], vec![PortDef::required("resolution", Vec2)]),
            Self::FragCoord     => (vec![], vec![PortDef::required("fragcoord", Vec4)]),
            Self::VertexNormal  => (vec![], vec![PortDef::required("normal", Vec3)]),
            Self::VertexTangent => (vec![], vec![PortDef::required("tangent", Vec4)]),
            Self::VertexUV      => (vec![], vec![PortDef::required("uv", Vec2)]),
            Self::VertexUV2     => (vec![], vec![PortDef::required("uv2", Vec2)]),
            Self::VertexColor   => (vec![], vec![PortDef::required("color", Vec4)]),
            Self::WorldPosition => (vec![], vec![PortDef::required("position", Vec3)]),
            Self::ViewDirection => (vec![], vec![PortDef::required("viewDir", Vec3)]),
            Self::CameraPosition => (vec![], vec![PortDef::required("camPos", Vec3)]),
            Self::ModelMatrix   => (vec![], vec![PortDef::required("model", Mat4)]),
            Self::ViewMatrix    => (vec![], vec![PortDef::required("view", Mat4)]),
            Self::ProjectionMatrix => (vec![], vec![PortDef::required("proj", Mat4)]),
            Self::NormalMatrix  => (vec![], vec![PortDef::required("normalMat", Mat3)]),

            Self::CustomUniformFloat { .. } => (vec![], vec![PortDef::required("value", Float)]),
            Self::CustomUniformVec3  { .. } => (vec![], vec![PortDef::required("value", Vec3)]),
            Self::CustomUniformVec4  { .. } => (vec![], vec![PortDef::required("value", Vec4)]),
            Self::CustomUniformSampler2D { .. } => (vec![], vec![PortDef::required("tex", Sampler2D)]),

            Self::Add | Self::Subtract | Self::Multiply | Self::Divide => (
                vec![PortDef::required("a", Vec4), PortDef::required("b", Vec4)],
                vec![PortDef::required("result", Vec4)],
            ),
            Self::Power => (
                vec![PortDef::required("base", Float), PortDef::required("exp", Float)],
                vec![PortDef::required("result", Float)],
            ),
            Self::SquareRoot | Self::AbsoluteValue | Self::Negate
            | Self::OneMinus | Self::Reciprocal | Self::Floor
            | Self::Ceiling | Self::Round | Self::Fraction | Self::Sign => (
                vec![PortDef::required("x", Vec4)],
                vec![PortDef::required("result", Vec4)],
            ),
            Self::Modulo | Self::Min | Self::Max => (
                vec![PortDef::required("a", Vec4), PortDef::required("b", Vec4)],
                vec![PortDef::required("result", Vec4)],
            ),
            Self::Clamp => (
                vec![PortDef::required("x", Vec4),
                     PortDef::optional("min", Vec4, "0.0"),
                     PortDef::optional("max", Vec4, "1.0")],
                vec![PortDef::required("result", Vec4)],
            ),
            Self::Saturate => (
                vec![PortDef::required("x", Vec4)],
                vec![PortDef::required("result", Vec4)],
            ),
            Self::Lerp | Self::Mix => (
                vec![PortDef::required("a", Vec4),
                     PortDef::required("b", Vec4),
                     PortDef::required("t", Float)],
                vec![PortDef::required("result", Vec4)],
            ),
            Self::SmoothStep => (
                vec![PortDef::required("edge0", Float),
                     PortDef::required("edge1", Float),
                     PortDef::required("x", Float)],
                vec![PortDef::required("result", Float)],
            ),
            Self::Step => (
                vec![PortDef::required("edge", Float), PortDef::required("x", Float)],
                vec![PortDef::required("result", Float)],
            ),

            Self::Sin | Self::Cos | Self::Tan | Self::Asin | Self::Acos
            | Self::Atan | Self::Degrees | Self::Radians => (
                vec![PortDef::required("x", Vec4)],
                vec![PortDef::required("result", Vec4)],
            ),
            Self::Atan2 => (
                vec![PortDef::required("y", Float), PortDef::required("x", Float)],
                vec![PortDef::required("result", Float)],
            ),

            Self::Exp | Self::Exp2 | Self::Log | Self::Log2 => (
                vec![PortDef::required("x", Float)],
                vec![PortDef::required("result", Float)],
            ),

            Self::Dot => (
                vec![PortDef::required("a", Vec3), PortDef::required("b", Vec3)],
                vec![PortDef::required("result", Float)],
            ),
            Self::Cross => (
                vec![PortDef::required("a", Vec3), PortDef::required("b", Vec3)],
                vec![PortDef::required("result", Vec3)],
            ),
            Self::Length => (
                vec![PortDef::required("v", Vec3)],
                vec![PortDef::required("length", Float)],
            ),
            Self::Normalize => (
                vec![PortDef::required("v", Vec3)],
                vec![PortDef::required("normalized", Vec3)],
            ),
            Self::Reflect => (
                vec![PortDef::required("i", Vec3), PortDef::required("n", Vec3)],
                vec![PortDef::required("reflected", Vec3)],
            ),
            Self::Refract => (
                vec![PortDef::required("i", Vec3),
                     PortDef::required("n", Vec3),
                     PortDef::required("eta", Float)],
                vec![PortDef::required("refracted", Vec3)],
            ),
            Self::FaceForward => (
                vec![PortDef::required("n", Vec3),
                     PortDef::required("i", Vec3),
                     PortDef::required("nref", Vec3)],
                vec![PortDef::required("result", Vec3)],
            ),
            Self::Distance => (
                vec![PortDef::required("a", Vec3), PortDef::required("b", Vec3)],
                vec![PortDef::required("distance", Float)],
            ),

            Self::VectorSplit => (
                vec![PortDef::required("v", Vec4)],
                vec![PortDef::required("x", Float),
                     PortDef::required("y", Float),
                     PortDef::required("z", Float),
                     PortDef::required("w", Float)],
            ),
            Self::VectorMerge => (
                vec![PortDef::optional("x", Float, "0.0"),
                     PortDef::optional("y", Float, "0.0"),
                     PortDef::optional("z", Float, "0.0"),
                     PortDef::optional("w", Float, "1.0")],
                vec![PortDef::required("v", Vec4)],
            ),
            Self::VectorSwizzle { out_count, .. } => {
                let names = ["x","y","z","w"];
                let out_ty = match out_count {
                    1 => Float,
                    2 => Vec2,
                    3 => Vec3,
                    _ => Vec4,
                };
                (
                    vec![PortDef::required("v", Vec4)],
                    vec![PortDef::required(names[(*out_count as usize).min(3)], out_ty)],
                )
            }

            Self::MatrixMultiply => (
                vec![PortDef::required("a", Mat4), PortDef::required("b", Mat4)],
                vec![PortDef::required("result", Mat4)],
            ),
            Self::MatrixTranspose | Self::MatrixInverse => (
                vec![PortDef::required("m", Mat4)],
                vec![PortDef::required("result", Mat4)],
            ),
            Self::TransformPoint => (
                vec![PortDef::required("m", Mat4), PortDef::required("p", Vec3)],
                vec![PortDef::required("result", Vec3)],
            ),
            Self::TransformVector | Self::TransformNormal => (
                vec![PortDef::required("m", Mat4), PortDef::required("v", Vec3)],
                vec![PortDef::required("result", Vec3)],
            ),

            Self::SampleTexture2D => (
                vec![PortDef::required("tex", Sampler2D), PortDef::required("uv", Vec2)],
                vec![PortDef::required("rgba", Vec4),
                     PortDef::required("rgb", Vec3),
                     PortDef::required("r", Float)],
            ),
            Self::SampleTextureCube => (
                vec![PortDef::required("tex", SamplerCube), PortDef::required("dir", Vec3)],
                vec![PortDef::required("rgba", Vec4), PortDef::required("rgb", Vec3)],
            ),
            Self::SampleTexture2DLod => (
                vec![PortDef::required("tex", Sampler2D),
                     PortDef::required("uv", Vec2),
                     PortDef::required("lod", Float)],
                vec![PortDef::required("rgba", Vec4)],
            ),
            Self::SampleTexture2DGrad => (
                vec![PortDef::required("tex", Sampler2D),
                     PortDef::required("uv", Vec2),
                     PortDef::required("dpdx", Vec2),
                     PortDef::required("dpdy", Vec2)],
                vec![PortDef::required("rgba", Vec4)],
            ),
            Self::SampleNormalMap => (
                vec![PortDef::required("tex", Sampler2D),
                     PortDef::required("uv", Vec2),
                     PortDef::optional("strength", Float, "1.0")],
                vec![PortDef::required("normal", Vec3)],
            ),
            Self::TextureSize => (
                vec![PortDef::required("tex", Sampler2D), PortDef::optional("lod", Int, "0")],
                vec![PortDef::required("size", Vec2)],
            ),

            Self::ColorToLinear | Self::ColorToGamma => (
                vec![PortDef::required("color", Vec3)],
                vec![PortDef::required("result", Vec3)],
            ),
            Self::HsvToRgb | Self::RgbToHsv => (
                vec![PortDef::required("color", Vec3)],
                vec![PortDef::required("result", Vec3)],
            ),
            Self::Luminance => (
                vec![PortDef::required("color", Vec3)],
                vec![PortDef::required("luma", Float)],
            ),
            Self::ColorBalance => (
                vec![PortDef::required("color", Vec3),
                     PortDef::optional("shadows", Vec3, "vec3(0.0)"),
                     PortDef::optional("midtones", Vec3, "vec3(0.0)"),
                     PortDef::optional("highlights", Vec3, "vec3(0.0)")],
                vec![PortDef::required("result", Vec3)],
            ),
            Self::Hue | Self::Saturation | Self::Brightness => (
                vec![PortDef::required("color", Vec3), PortDef::required("value", Float)],
                vec![PortDef::required("result", Vec3)],
            ),

            Self::FresnelSchlick => (
                vec![PortDef::required("F0", Vec3),
                     PortDef::required("cosTheta", Float)],
                vec![PortDef::required("fresnel", Vec3)],
            ),
            Self::GGXDistribution => (
                vec![PortDef::required("NdotH", Float), PortDef::required("roughness", Float)],
                vec![PortDef::required("D", Float)],
            ),
            Self::SmithGeometry => (
                vec![PortDef::required("NdotV", Float),
                     PortDef::required("NdotL", Float),
                     PortDef::required("roughness", Float)],
                vec![PortDef::required("G", Float)],
            ),
            Self::BRDFSpecular => (
                vec![PortDef::required("normal", Vec3),
                     PortDef::required("viewDir", Vec3),
                     PortDef::required("lightDir", Vec3),
                     PortDef::required("F0", Vec3),
                     PortDef::required("roughness", Float)],
                vec![PortDef::required("specular", Vec3)],
            ),
            Self::BRDFDiffuse => (
                vec![PortDef::required("albedo", Vec3),
                     PortDef::required("normal", Vec3),
                     PortDef::required("lightDir", Vec3)],
                vec![PortDef::required("diffuse", Vec3)],
            ),
            Self::EnvBRDFApprox => (
                vec![PortDef::required("F0", Vec3),
                     PortDef::required("roughness", Float),
                     PortDef::required("NdotV", Float)],
                vec![PortDef::required("envBRDF", Vec3)],
            ),
            Self::SubsurfaceApprox => (
                vec![PortDef::required("albedo", Vec3),
                     PortDef::required("thickness", Float),
                     PortDef::optional("scatter", Vec3, "vec3(1.0,0.3,0.1)")],
                vec![PortDef::required("sss", Vec3)],
            ),
            Self::EmissionBlend => (
                vec![PortDef::required("color", Vec3),
                     PortDef::required("emission", Vec3),
                     PortDef::optional("strength", Float, "1.0")],
                vec![PortDef::required("result", Vec3)],
            ),

            Self::SdfSample { .. } => (
                vec![PortDef::required("pos", Vec3)],
                vec![PortDef::required("dist", Float)],
            ),
            Self::SdfNormal { .. } => (
                vec![PortDef::required("pos", Vec3), PortDef::optional("eps", Float, "0.001")],
                vec![PortDef::required("normal", Vec3)],
            ),
            Self::SdfAO { .. } => (
                vec![PortDef::required("pos", Vec3), PortDef::required("normal", Vec3)],
                vec![PortDef::required("ao", Float)],
            ),

            Self::IfElse => (
                vec![PortDef::required("condition", Bool),
                     PortDef::required("ifTrue", Vec4),
                     PortDef::required("ifFalse", Vec4)],
                vec![PortDef::required("result", Vec4)],
            ),
            Self::IsNaN | Self::IsInf => (
                vec![PortDef::required("x", Float)],
                vec![PortDef::required("result", Bool)],
            ),
            Self::Dpdx | Self::Dpdy | Self::FWidth => (
                vec![PortDef::required("p", Float)],
                vec![PortDef::required("result", Float)],
            ),

            Self::Noise2D | Self::GradientNoise => (
                vec![PortDef::required("uv", Vec2), PortDef::optional("scale", Float, "1.0")],
                vec![PortDef::required("noise", Float)],
            ),
            Self::Noise3D => (
                vec![PortDef::required("pos", Vec3), PortDef::optional("scale", Float, "1.0")],
                vec![PortDef::required("noise", Float)],
            ),
            Self::VoronoiNoise => (
                vec![PortDef::required("uv", Vec2), PortDef::optional("scale", Float, "5.0")],
                vec![PortDef::required("dist", Float),
                     PortDef::required("cell", Vec2)],
            ),
            Self::FbmNoise { .. } => (
                vec![PortDef::required("pos", Vec3),
                     PortDef::optional("scale", Float, "1.0"),
                     PortDef::optional("gain", Float, "0.5"),
                     PortDef::optional("lacunarity", Float, "2.0")],
                vec![PortDef::required("noise", Float)],
            ),
            Self::CellularNoise => (
                vec![PortDef::required("pos", Vec3)],
                vec![PortDef::required("f1", Float), PortDef::required("f2", Float)],
            ),
            Self::WhiteNoise => (
                vec![PortDef::required("seed", Vec2)],
                vec![PortDef::required("noise", Float)],
            ),

            Self::PbrOutput => (
                vec![PortDef::required("albedo", Vec3),
                     PortDef::optional("normal", Vec3, "vec3(0,0,1)"),
                     PortDef::optional("metallic", Float, "0.0"),
                     PortDef::optional("roughness", Float, "0.5"),
                     PortDef::optional("ao", Float, "1.0"),
                     PortDef::optional("emission", Vec3, "vec3(0.0)"),
                     PortDef::optional("alpha", Float, "1.0"),
                     PortDef::optional("sss", Float, "0.0"),
                     PortDef::optional("ior", Float, "1.5")],
                vec![],
            ),
            Self::UnlitOutput => (
                vec![PortDef::required("color", Vec4)],
                vec![],
            ),
            Self::PostProcessOutput => (
                vec![PortDef::required("color", Vec4)],
                vec![],
            ),
            Self::CustomOutput { value_type, .. } => (
                vec![PortDef::required("value", *value_type)],
                vec![],
            ),
            Self::VertexOffset => (
                vec![PortDef::required("offset", Vec3)],
                vec![],
            ),
            Self::DepthOutput => (
                vec![PortDef::required("depth", Float)],
                vec![],
            ),
            Self::CustomVarying { value_type, .. } => (
                vec![PortDef::required("value", *value_type)],
                vec![PortDef::required("out", *value_type)],
            ),

            Self::Reroute => (
                vec![PortDef::required("in", Vec4)],
                vec![PortDef::required("out", Vec4)],
            ),
            Self::Comment { .. } | Self::Group { .. } => (vec![], vec![]),
            Self::SubGraph { .. } => (vec![], vec![]),
        }
    }

    /// Emit GLSL expression for this node given named expressions for each input.
    pub fn emit_glsl(&self, inputs: &[String], var: &str) -> String {
        let i = |idx: usize| inputs.get(idx).map(|s| s.as_str()).unwrap_or("0.0");
        match self {
            Self::ConstFloat(v) => format!("float {} = {:.8};", var, v),
            Self::ConstVec2(v)  => format!("vec2 {} = vec2({:.8},{:.8});", var, v[0], v[1]),
            Self::ConstVec3(v)  => format!("vec3 {} = vec3({:.8},{:.8},{:.8});", var, v[0],v[1],v[2]),
            Self::ConstVec4(v)  => format!("vec4 {} = vec4({:.8},{:.8},{:.8},{:.8});", var, v[0],v[1],v[2],v[3]),
            Self::ConstInt(v)   => format!("int {} = {};", var, v),
            Self::ConstBool(v)  => format!("bool {} = {};", var, v),

            Self::Time          => format!("float {} = u_time;", var),
            Self::Resolution    => format!("vec2 {} = u_resolution;", var),
            Self::FragCoord     => format!("vec4 {} = gl_FragCoord;", var),
            Self::VertexNormal  => format!("vec3 {} = v_normal;", var),
            Self::VertexTangent => format!("vec4 {} = v_tangent;", var),
            Self::VertexUV      => format!("vec2 {} = v_uv;", var),
            Self::VertexUV2     => format!("vec2 {} = v_uv2;", var),
            Self::VertexColor   => format!("vec4 {} = v_color;", var),
            Self::WorldPosition => format!("vec3 {} = v_world_pos;", var),
            Self::ViewDirection => format!("vec3 {} = normalize(u_camera_pos - v_world_pos);", var),
            Self::CameraPosition => format!("vec3 {} = u_camera_pos;", var),
            Self::ModelMatrix   => format!("mat4 {} = u_model;", var),
            Self::ViewMatrix    => format!("mat4 {} = u_view;", var),
            Self::ProjectionMatrix => format!("mat4 {} = u_proj;", var),
            Self::NormalMatrix  => format!("mat3 {} = u_normal_mat;", var),
            Self::CustomUniformFloat { name } => format!("float {} = {};", var, name),
            Self::CustomUniformVec3  { name } => format!("vec3 {} = {};", var, name),
            Self::CustomUniformVec4  { name } => format!("vec4 {} = {};", var, name),
            Self::CustomUniformSampler2D { name } => format!("// texture {} bound as {}", name, var),

            Self::Add      => format!("vec4 {} = {} + {};", var, i(0), i(1)),
            Self::Subtract => format!("vec4 {} = {} - {};", var, i(0), i(1)),
            Self::Multiply => format!("vec4 {} = {} * {};", var, i(0), i(1)),
            Self::Divide   => format!("vec4 {} = {} / max({}, vec4(1e-7));", var, i(0), i(1)),
            Self::Power    => format!("float {} = pow({}, {});", var, i(0), i(1)),
            Self::SquareRoot   => format!("vec4 {} = sqrt({});", var, i(0)),
            Self::AbsoluteValue => format!("vec4 {} = abs({});", var, i(0)),
            Self::Negate       => format!("vec4 {} = -{};", var, i(0)),
            Self::OneMinus     => format!("vec4 {} = 1.0 - {};", var, i(0)),
            Self::Reciprocal   => format!("vec4 {} = 1.0 / max({}, vec4(1e-7));", var, i(0)),
            Self::Floor        => format!("vec4 {} = floor({});", var, i(0)),
            Self::Ceiling      => format!("vec4 {} = ceil({});", var, i(0)),
            Self::Round        => format!("vec4 {} = round({});", var, i(0)),
            Self::Fraction     => format!("vec4 {} = fract({});", var, i(0)),
            Self::Sign         => format!("vec4 {} = sign({});", var, i(0)),
            Self::Modulo       => format!("vec4 {} = mod({}, {});", var, i(0), i(1)),
            Self::Min          => format!("vec4 {} = min({}, {});", var, i(0), i(1)),
            Self::Max          => format!("vec4 {} = max({}, {});", var, i(0), i(1)),
            Self::Clamp        => format!("vec4 {} = clamp({}, {}, {});", var, i(0), i(1), i(2)),
            Self::Saturate     => format!("vec4 {} = clamp({}, 0.0, 1.0);", var, i(0)),
            Self::Lerp | Self::Mix
                               => format!("vec4 {} = mix({}, {}, {});", var, i(0), i(1), i(2)),
            Self::SmoothStep   => format!("float {} = smoothstep({}, {}, {});", var, i(0), i(1), i(2)),
            Self::Step         => format!("float {} = step({}, {});", var, i(0), i(1)),

            Self::Sin     => format!("vec4 {} = sin({});", var, i(0)),
            Self::Cos     => format!("vec4 {} = cos({});", var, i(0)),
            Self::Tan     => format!("vec4 {} = tan({});", var, i(0)),
            Self::Asin    => format!("vec4 {} = asin({});", var, i(0)),
            Self::Acos    => format!("vec4 {} = acos({});", var, i(0)),
            Self::Atan    => format!("vec4 {} = atan({});", var, i(0)),
            Self::Atan2   => format!("float {} = atan({}, {});", var, i(0), i(1)),
            Self::Degrees => format!("vec4 {} = degrees({});", var, i(0)),
            Self::Radians => format!("vec4 {} = radians({});", var, i(0)),

            Self::Exp  => format!("float {} = exp({});", var, i(0)),
            Self::Exp2 => format!("float {} = exp2({});", var, i(0)),
            Self::Log  => format!("float {} = log({});", var, i(0)),
            Self::Log2 => format!("float {} = log2({});", var, i(0)),

            Self::Dot       => format!("float {} = dot({}, {});", var, i(0), i(1)),
            Self::Cross     => format!("vec3 {} = cross({}, {});", var, i(0), i(1)),
            Self::Length    => format!("float {} = length({});", var, i(0)),
            Self::Normalize => format!("vec3 {} = normalize({});", var, i(0)),
            Self::Reflect   => format!("vec3 {} = reflect({}, {});", var, i(0), i(1)),
            Self::Refract   => format!("vec3 {} = refract({}, {}, {});", var, i(0), i(1), i(2)),
            Self::FaceForward => format!("vec3 {} = faceforward({}, {}, {});", var, i(0), i(1), i(2)),
            Self::Distance  => format!("float {} = distance({}, {});", var, i(0), i(1)),
            Self::VectorSplit => format!(
                "float {}_x={}.x; float {}_y={}.y; float {}_z={}.z; float {}_w={}.w;",
                var, i(0), var, i(0), var, i(0), var, i(0)
            ),
            Self::VectorMerge => format!("vec4 {} = vec4({},{},{},{});", var, i(0), i(1), i(2), i(3)),
            Self::VectorSwizzle { swizzle, out_count } => {
                let channels = ['x','y','z','w'];
                let s: String = swizzle[..*out_count as usize].iter()
                    .map(|&idx| channels[idx as usize % 4])
                    .collect();
                let ty = match out_count {
                    1 => "float",
                    2 => "vec2",
                    3 => "vec3",
                    _ => "vec4",
                };
                format!("{} {} = {}.{};", ty, var, i(0), s)
            }

            Self::MatrixMultiply  => format!("mat4 {} = {} * {};", var, i(0), i(1)),
            Self::MatrixTranspose => format!("mat4 {} = transpose({});", var, i(0)),
            Self::MatrixInverse   => format!("mat4 {} = inverse({});", var, i(0)),
            Self::TransformPoint  => format!("vec3 {} = ({} * vec4({}, 1.0)).xyz;", var, i(0), i(1)),
            Self::TransformVector => format!("vec3 {} = ({} * vec4({}, 0.0)).xyz;", var, i(0), i(1)),
            Self::TransformNormal => format!("vec3 {} = normalize(mat3({}) * {});", var, i(0), i(1)),

            Self::SampleTexture2D => format!(
                "vec4 {}_rgba = texture({}, {}); vec3 {}_rgb = {}_rgba.rgb; float {}_r = {}_rgba.r;",
                var, i(0), i(1), var, var, var, var
            ),
            Self::SampleTextureCube => format!(
                "vec4 {}_rgba = texture({}, {}); vec3 {}_rgb = {}_rgba.rgb;",
                var, i(0), i(1), var, var
            ),
            Self::SampleTexture2DLod => format!(
                "vec4 {} = textureLod({}, {}, {});", var, i(0), i(1), i(2)
            ),
            Self::SampleTexture2DGrad => format!(
                "vec4 {} = textureGrad({}, {}, {}, {});", var, i(0), i(1), i(2), i(3)
            ),
            Self::SampleNormalMap => format!(
                "vec3 {} = normalize(texture({},{}).rgb * 2.0 - 1.0) * vec3({},{},1.0);",
                var, i(0), i(1), i(2), i(2)
            ),
            Self::TextureSize => format!(
                "vec2 {} = vec2(textureSize({}, {}));", var, i(0), i(1)
            ),

            Self::ColorToLinear => format!("vec3 {} = pow({}, vec3(2.2));", var, i(0)),
            Self::ColorToGamma  => format!("vec3 {} = pow({}, vec3(1.0/2.2));", var, i(0)),
            Self::HsvToRgb => format!(
                "vec3 {} = clamp(abs(mod({}.x*6.0+vec3(0,4,2),6.0)-3.0)-1.0,0.0,1.0); {} = {}.z*mix(vec3(1.0),{},{}.y);",
                var, i(0), var, i(0), var, i(0)
            ),
            Self::RgbToHsv => format!(
                "vec4 _p{}=mix(vec4({}.bg,vec2(-1.0/3.0,2.0/3.0)),vec4({}.gb,vec2(0.0,-1.0/3.0)),step({}.b,{}.g));vec4 _q{}=mix(vec4(_p{}.xyw,{}.r),vec4({}.r,_p{}.yzx),step(_p{}.x,{}.r));float _d{}=_q{}.x-min(_q{}.w,_q{}.y);vec3 {}=vec3(abs(_q{}.z+((_q{}.w-_q{}.y)/(6.0*_d{}+1e-10))),_d{}/(_q{}.x+1e-10),_q{}.x);",
                var,i(0),i(0),i(0),i(0),
                var,var,i(0),i(0),var,var,i(0),
                var,var,var,var,
                var,var,var,var,var,var,var,var
            ),
            Self::Luminance => format!("float {} = dot({}, vec3(0.2126,0.7152,0.0722));", var, i(0)),
            Self::ColorBalance => format!(
                "vec3 {} = {} + {} * (1.0 - {}) + {} * clamp(length({} - 0.5)*2.0,0.0,1.0) + {} * {};",
                var, i(0), i(1), i(0), i(2), i(0), i(3), i(0)
            ),
            Self::Hue => format!(
                "vec3 _hsv{}=vec3(atan({}.g-{}.b,{}.r-{}.g)/(2.0*3.14159)+0.5,length({}.rgb),dot({}.rgb,vec3(0.333)));vec3 {}=vec3(mod(_hsv{}.x+{},1.0),_hsv{}.yz);",
                var,i(0),i(0),i(0),i(0),i(0),i(0),var,var,i(1),var
            ),
            Self::Saturation => format!("vec3 {} = mix(vec3(dot({},vec3(0.2126,0.7152,0.0722))),{},{});",var,i(0),i(0),i(1)),
            Self::Brightness => format!("vec3 {} = {} * {};", var, i(0), i(1)),

            Self::FresnelSchlick => format!(
                "vec3 {} = {} + (1.0-{}) * pow(1.0-clamp({},0.0,1.0),5.0);",
                var, i(0), i(0), i(1)
            ),
            Self::GGXDistribution => format!(
                "float _a{}={}*{};float _a2{}=_a{}*_a{};float _denom{}=({}-1.0)*_a2{}+1.0;float {}=_a2{}/(3.14159*_denom{}*_denom{});",
                var,i(1),i(1),var,var,var,var,i(0),var,var,var,var,var
            ),
            Self::SmithGeometry => format!(
                "float _k{}={}*{}/8.0;float _gv{}={}/({}*(1.0-_k{})+_k{});float _gl{}={}/({}*(1.0-_k{})+_k{});float {}=_gv{}*_gl{};",
                var,i(2),i(2),var,i(0),i(0),var,var,var,i(1),i(1),var,var,var,var,var
            ),
            Self::BRDFSpecular => format!(
                "// BRDFSpecular: full Cook-Torrance stored in {}", var
            ),
            Self::BRDFDiffuse => format!(
                "vec3 {} = {} * max(dot({},{}),0.0) / 3.14159;", var, i(0), i(1), i(2)
            ),
            Self::EnvBRDFApprox => format!(
                "vec2 _env{}=vec2(clamp({},0.0,1.0),{}); vec3 {}={}*(_env{}.x+vec2(-0.0048,0.0,0.0).x)+_env{}.y;",
                var,i(2),i(1),var,i(0),var,var
            ),
            Self::SubsurfaceApprox => format!(
                "float _trans{}=exp(-{}/max({},0.001));vec3 {}={}*{}_trans;",
                var,i(1),i(1),var,i(0),var
            ),
            Self::EmissionBlend => format!(
                "vec3 {} = {} + {} * {};", var, i(0), i(1), i(2)
            ),

            Self::SdfSample { sdf_graph_id } => format!(
                "float {} = sdf_graph_{}({});", var, sdf_graph_id, i(0)
            ),
            Self::SdfNormal { sdf_graph_id } => format!(
                "vec3 {} = sdf_normal_{}({}, {});", var, sdf_graph_id, i(0), i(1)
            ),
            Self::SdfAO { sdf_graph_id } => format!(
                "float {} = sdf_ao_{}({}, {});", var, sdf_graph_id, i(0), i(1)
            ),

            Self::IfElse => format!("vec4 {} = {} ? {} : {};", var, i(0), i(1), i(2)),
            Self::IsNaN  => format!("bool {} = isnan({});", var, i(0)),
            Self::IsInf  => format!("bool {} = isinf({});", var, i(0)),
            Self::Dpdx   => format!("float {} = dFdx({});", var, i(0)),
            Self::Dpdy   => format!("float {} = dFdy({});", var, i(0)),
            Self::FWidth => format!("float {} = fwidth({});", var, i(0)),

            Self::Noise2D | Self::GradientNoise => format!(
                "float {} = noise_grad2({} * {});", var, i(0), i(1)
            ),
            Self::Noise3D => format!(
                "float {} = noise_grad3({} * {});", var, i(0), i(1)
            ),
            Self::VoronoiNoise => format!(
                "vec3 _vor{}=voronoi({} * {}); float {}_dist=_vor{}.x; vec2 {}_cell=_vor{}.yz;",
                var, i(0), i(1), var, var, var, var
            ),
            Self::FbmNoise { octaves } => format!(
                "float {} = fbm({}, {}, {}, {}, {});", var, i(0), i(1), i(2), i(3), octaves
            ),
            Self::CellularNoise => format!(
                "vec2 _cell{}=cellular({});float {}_f1=_cell{}.x;float {}_f2=_cell{}.y;",
                var, i(0), var, var, var, var
            ),
            Self::WhiteNoise => format!(
                "float {} = fract(sin(dot({},vec2(127.1,311.7)))*43758.5453);", var, i(0)
            ),

            Self::PbrOutput => format!(
                "// PBR output: albedo={} normal={} metallic={} roughness={} ao={} emission={} alpha={} sss={} ior={}",
                i(0), i(1), i(2), i(3), i(4), i(5), i(6), i(7), i(8)
            ),
            Self::UnlitOutput | Self::PostProcessOutput => format!(
                "// Output color: {}", i(0)
            ),
            Self::CustomOutput { name, .. } => format!("// {} = {};", name, i(0)),
            Self::VertexOffset  => format!("// Vertex offset: {}", i(0)),
            Self::DepthOutput   => format!("gl_FragDepth = {};", i(0)),
            Self::CustomVarying { name, value_type } => format!(
                "{} {} = {};", value_type.glsl_type(), name, i(0)
            ),

            Self::Reroute => format!("vec4 {} = {};", var, i(0)),
            Self::Comment { .. } | Self::Group { .. } | Self::SubGraph { .. } => String::new(),
        }
    }
}

// ─── Node struct ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ShaderNode {
    pub id: ShaderNodeId,
    pub kind: ShaderNodeKind,
    pub position: [f32; 2],
    pub collapsed: bool,
    pub disabled: bool,
    pub comment: Option<String>,
    pub preview_enabled: bool,
}

impl ShaderNode {
    pub fn new(id: ShaderNodeId, kind: ShaderNodeKind, x: f32, y: f32) -> Self {
        Self {
            id,
            kind,
            position: [x, y],
            collapsed: false,
            disabled: false,
            comment: None,
            preview_enabled: false,
        }
    }

    pub fn width(&self) -> f32 {
        let (ins, outs) = self.kind.port_definitions();
        let max_ports = ins.len().max(outs.len());
        (160.0 + max_ports as f32 * 4.0).min(300.0)
    }

    pub fn height(&self) -> f32 {
        if self.collapsed { return 28.0; }
        let (ins, outs) = self.kind.port_definitions();
        let rows = ins.len().max(outs.len()).max(1);
        28.0 + rows as f32 * 22.0
    }

    pub fn port_position(&self, port: u16, is_output: bool) -> [f32; 2] {
        let y = self.position[1] + 28.0 + port as f32 * 22.0 + 11.0;
        let x = if is_output {
            self.position[0] + self.width()
        } else {
            self.position[0]
        };
        [x, y]
    }
}

// ─── Shader graph ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderGraphType {
    Fragment,
    Vertex,
    PostProcess,
    Compute,
}

#[derive(Debug, Clone)]
pub struct ShaderGraph {
    pub id: u32,
    pub name: String,
    pub graph_type: ShaderGraphType,
    nodes: HashMap<ShaderNodeId, ShaderNode>,
    connections: Vec<ShaderConnection>,
    next_id: u32,
    pub canvas_offset: [f32; 2],
    pub canvas_zoom: f32,
    pub glsl_version: u32,
}

impl ShaderGraph {
    pub fn new(id: u32, name: String, graph_type: ShaderGraphType) -> Self {
        Self {
            id,
            name,
            graph_type,
            nodes: HashMap::new(),
            connections: Vec::new(),
            next_id: 1,
            canvas_offset: [0.0; 2],
            canvas_zoom: 1.0,
            glsl_version: 450,
        }
    }

    pub fn add_node(&mut self, kind: ShaderNodeKind, x: f32, y: f32) -> ShaderNodeId {
        let id = ShaderNodeId(self.next_id);
        self.next_id += 1;
        self.nodes.insert(id, ShaderNode::new(id, kind, x, y));
        id
    }

    pub fn remove_node(&mut self, id: ShaderNodeId) {
        self.nodes.remove(&id);
        self.connections.retain(|c| c.from.node != id && c.to.node != id);
    }

    pub fn connect(&mut self, from: ShaderPortId, to: ShaderPortId) -> Result<(), &'static str> {
        // Validate ports exist
        if !self.nodes.contains_key(&from.node) { return Err("Source node not found"); }
        if !self.nodes.contains_key(&to.node)   { return Err("Target node not found"); }
        if from.node == to.node { return Err("Cannot connect node to itself"); }

        // Remove existing connection to same input
        self.connections.retain(|c| !(c.to.node == to.node && c.to.port == to.port));

        // Check for cycles
        if self.would_create_cycle(from.node, to.node) {
            return Err("Connection would create a cycle");
        }

        self.connections.push(ShaderConnection { from, to });
        Ok(())
    }

    fn would_create_cycle(&self, from: ShaderNodeId, to: ShaderNodeId) -> bool {
        // BFS from `to` — if we can reach `from`, it's a cycle
        let mut visited = HashSet::new();
        let mut queue   = VecDeque::new();
        queue.push_back(to);
        while let Some(node) = queue.pop_front() {
            if node == from { return true; }
            if visited.contains(&node) { continue; }
            visited.insert(node);
            for conn in &self.connections {
                if conn.from.node == node {
                    queue.push_back(conn.to.node);
                }
            }
        }
        false
    }

    pub fn disconnect(&mut self, to: ShaderPortId) {
        self.connections.retain(|c| !(c.to.node == to.node && c.to.port == to.port));
    }

    pub fn topological_order(&self) -> Vec<ShaderNodeId> {
        let mut in_degree: HashMap<ShaderNodeId, usize> =
            self.nodes.keys().map(|&k| (k, 0)).collect();
        for conn in &self.connections {
            *in_degree.entry(conn.to.node).or_insert(0) += 1;
        }
        let mut queue: VecDeque<ShaderNodeId> =
            in_degree.iter().filter(|(_, &d)| d == 0).map(|(&k, _)| k).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            for conn in &self.connections {
                if conn.from.node == n {
                    let d = in_degree.entry(conn.to.node).or_insert(1);
                    *d -= 1;
                    if *d == 0 { queue.push_back(conn.to.node); }
                }
            }
        }
        order
    }

    /// Compile the full GLSL shader source for this graph
    pub fn compile(&self) -> ShaderCompileResult {
        let order = self.topological_order();
        let mut lines: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        let mut uniforms: Vec<String> = Vec::new();
        let mut functions: Vec<String> = Vec::new();

        // Header
        lines.push(format!("#version {} core", self.glsl_version));
        lines.push(String::new());

        // Collect uniforms from input nodes
        for id in &order {
            if let Some(node) = self.nodes.get(id) {
                match &node.kind {
                    ShaderNodeKind::CustomUniformFloat { name } =>
                        uniforms.push(format!("uniform float {};", name)),
                    ShaderNodeKind::CustomUniformVec3  { name } =>
                        uniforms.push(format!("uniform vec3 {};", name)),
                    ShaderNodeKind::CustomUniformVec4  { name } =>
                        uniforms.push(format!("uniform vec4 {};", name)),
                    ShaderNodeKind::CustomUniformSampler2D { name } =>
                        uniforms.push(format!("uniform sampler2D {};", name)),
                    _ => {}
                }
            }
        }

        // Built-in uniforms
        lines.push("uniform float u_time;".into());
        lines.push("uniform vec2 u_resolution;".into());
        lines.push("uniform mat4 u_model;".into());
        lines.push("uniform mat4 u_view;".into());
        lines.push("uniform mat4 u_proj;".into());
        lines.push("uniform mat3 u_normal_mat;".into());
        lines.push("uniform vec3 u_camera_pos;".into());
        for u in &uniforms { lines.push(u.clone()); }
        lines.push(String::new());

        // Varyings from vertex shader
        lines.push("in vec3 v_world_pos;".into());
        lines.push("in vec3 v_normal;".into());
        lines.push("in vec4 v_tangent;".into());
        lines.push("in vec2 v_uv;".into());
        lines.push("in vec2 v_uv2;".into());
        lines.push("in vec4 v_color;".into());
        lines.push(String::new());

        // Out
        lines.push("out vec4 frag_color;".into());
        lines.push(String::new());

        // Helper function stubs
        functions.push("// ---- noise helpers ----".into());
        functions.push("float noise_grad2(vec2 p) { return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5); }".into());
        functions.push("float noise_grad3(vec3 p) { return fract(sin(dot(p,vec3(127.1,311.7,74.7)))*43758.5); }".into());
        functions.push("vec3 voronoi(vec2 x) { vec2 p=floor(x),f=fract(x); float res=8.0; vec2 mr=vec2(0.0); for(int j=-1;j<=1;j++)for(int i=-1;i<=1;i++){vec2 b=vec2(float(i),float(j));vec2 r=b-f+fract(sin(vec2(dot(p+b,vec2(127.1,311.7)),dot(p+b,vec2(269.5,183.3))))*43758.5);float d=dot(r,r);if(d<res){res=d;mr=r;}}return vec3(sqrt(res),mr); }".into());
        functions.push("float fbm(vec3 p, float scale, float gain, float lac, int oct) { float s=0.0,a=0.5; p*=scale; for(int i=0;i<oct;i++){s+=a*noise_grad3(p);p*=lac;a*=gain;} return s; }".into());
        functions.push("vec2 cellular(vec3 p) { vec3 b=floor(p); float f1=9e9,f2=9e9; for(int z=-1;z<=1;z++)for(int y=-1;y<=1;y++)for(int x=-1;x<=1;x++){vec3 nb=b+vec3(x,y,z);vec3 c=nb+fract(sin(vec3(dot(nb,vec3(127.1,311.7,74.7)),dot(nb,vec3(269.5,183.3,246.1)),dot(nb,vec3(113.5,271.9,124.6))))*43758.5);float d=length(p-c);if(d<f1){f2=f1;f1=d;}else if(d<f2){f2=d;}}return vec2(f1,f2); }".into());
        functions.push(String::new());

        for f in &functions { lines.push(f.clone()); }
        lines.push("void main() {".into());

        // Emit node code in topological order
        for id in &order {
            if let Some(node) = self.nodes.get(id) {
                if node.disabled { continue; }
                let (in_defs, _) = node.kind.port_definitions();
                let mut input_exprs: Vec<String> = Vec::new();
                for (port_idx, _port_def) in in_defs.iter().enumerate() {
                    let conn = self.connections.iter().find(|c| {
                        c.to.node == *id && c.to.port == port_idx as u16
                    });
                    if let Some(c) = conn {
                        input_exprs.push(format!("_n{}_{}", c.from.node.0, c.from.port));
                    } else if let Some(default) = &_port_def.default_expr {
                        input_exprs.push(default.clone());
                    } else {
                        input_exprs.push(_port_def.value_type.default_value().to_string());
                        errors.push(format!("Node {} port {} has no connection and no default",
                            node.id.0, port_idx));
                    }
                }
                let var = format!("_n{}", id.0);
                let code = node.kind.emit_glsl(&input_exprs, &var);
                if !code.is_empty() {
                    lines.push(format!("    {}", code));
                }
            }
        }

        lines.push("}".into());

        ShaderCompileResult {
            source: lines.join("\n"),
            errors,
            warnings: Vec::new(),
            uniform_names: uniforms,
        }
    }

    pub fn node(&self, id: ShaderNodeId) -> Option<&ShaderNode> {
        self.nodes.get(&id)
    }

    pub fn node_mut(&mut self, id: ShaderNodeId) -> Option<&mut ShaderNode> {
        self.nodes.get_mut(&id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ShaderNode> {
        self.nodes.values()
    }

    pub fn connections(&self) -> &[ShaderConnection] {
        &self.connections
    }

    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn connection_count(&self) -> usize { self.connections.len() }
}

#[derive(Debug, Clone)]
pub struct ShaderCompileResult {
    pub source: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub uniform_names: Vec<String>,
}

impl ShaderCompileResult {
    pub fn is_ok(&self) -> bool { self.errors.is_empty() }
}

// ─── Graph library ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ShaderGraphLibrary {
    graphs: HashMap<u32, ShaderGraph>,
    next_id: u32,
    pub active_graph: Option<u32>,
}

impl ShaderGraphLibrary {
    pub fn new() -> Self {
        let mut lib = Self {
            graphs: HashMap::new(),
            next_id: 1,
            active_graph: None,
        };
        // Create default PBR graph
        let id = lib.create_graph("Default PBR".into(), ShaderGraphType::Fragment);
        lib.build_default_pbr(id);
        lib.active_graph = Some(id);
        lib
    }

    pub fn create_graph(&mut self, name: String, ty: ShaderGraphType) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.graphs.insert(id, ShaderGraph::new(id, name, ty));
        id
    }

    pub fn remove_graph(&mut self, id: u32) {
        self.graphs.remove(&id);
        if self.active_graph == Some(id) {
            self.active_graph = self.graphs.keys().next().copied();
        }
    }

    pub fn graph(&self, id: u32) -> Option<&ShaderGraph> {
        self.graphs.get(&id)
    }

    pub fn graph_mut(&mut self, id: u32) -> Option<&mut ShaderGraph> {
        self.graphs.get_mut(&id)
    }

    pub fn graphs(&self) -> impl Iterator<Item = &ShaderGraph> {
        self.graphs.values()
    }

    pub fn active_graph_mut(&mut self) -> Option<&mut ShaderGraph> {
        let id = self.active_graph?;
        self.graphs.get_mut(&id)
    }

    pub fn active_graph_ref(&self) -> Option<&ShaderGraph> {
        let id = self.active_graph?;
        self.graphs.get(&id)
    }

    fn build_default_pbr(&mut self, graph_id: u32) {
        let g = match self.graphs.get_mut(&graph_id) {
            Some(g) => g,
            None => return,
        };

        // Position nodes in a sensible layout
        let albedo    = g.add_node(ShaderNodeKind::ConstVec3([0.8, 0.6, 0.4]), 50.0,  50.0);
        let roughness = g.add_node(ShaderNodeKind::ConstFloat(0.4),             50.0, 150.0);
        let metallic  = g.add_node(ShaderNodeKind::ConstFloat(0.0),             50.0, 250.0);
        let normal_uv = g.add_node(ShaderNodeKind::VertexUV,                    50.0, 350.0);
        let normal_n  = g.add_node(ShaderNodeKind::VertexNormal,                50.0, 430.0);
        let output    = g.add_node(ShaderNodeKind::PbrOutput,                  450.0, 100.0);

        let _ = g.connect(
            ShaderPortId::output(albedo, 0),
            ShaderPortId::input(output, 0),
        );
        let _ = g.connect(
            ShaderPortId::output(normal_n, 0),
            ShaderPortId::input(output, 1),
        );
        let _ = g.connect(
            ShaderPortId::output(metallic, 0),
            ShaderPortId::input(output, 2),
        );
        let _ = g.connect(
            ShaderPortId::output(roughness, 0),
            ShaderPortId::input(output, 3),
        );
        let _ = normal_uv; // reserve for later use
    }
}

impl Default for ShaderGraphLibrary {
    fn default() -> Self { Self::new() }
}

// ─── Editor state ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderEditorTool {
    Select,
    Connect,
    Pan,
    Comment,
    Reroute,
}

#[derive(Debug, Clone)]
pub enum ShaderEditorAction {
    AddNode { id: ShaderNodeId, kind_label: String, graph: u32 },
    RemoveNode { id: ShaderNodeId, graph: u32 },
    MoveNode { id: ShaderNodeId, from: [f32; 2], to: [f32; 2] },
    AddConnection { from: ShaderPortId, to: ShaderPortId, graph: u32 },
    RemoveConnection { from: ShaderPortId, to: ShaderPortId, graph: u32 },
    ChangeParam { node: ShaderNodeId, param: String, before: String, after: String },
    CreateGraph { id: u32, name: String },
    DeleteGraph { id: u32, name: String },
}

#[derive(Debug, Clone)]
pub struct NodeSearchResult {
    pub kind_label: String,
    pub category: String,
    pub description: &'static str,
}

#[derive(Debug)]
pub struct ShaderGraphEditor {
    pub library: ShaderGraphLibrary,
    pub active_tool: ShaderEditorTool,
    pub selected_nodes: HashSet<ShaderNodeId>,
    pub dragging_node: Option<(ShaderNodeId, [f32; 2])>,
    pub connecting_from: Option<ShaderPortId>,
    pub search_query: String,
    pub search_open: bool,
    pub search_results: Vec<NodeSearchResult>,
    pub compile_result: Option<ShaderCompileResult>,
    pub auto_compile: bool,
    pub show_node_previews: bool,
    pub grid_snap: bool,
    pub grid_size: f32,
    undo_stack: Vec<ShaderEditorAction>,
    redo_stack: Vec<ShaderEditorAction>,
}

impl ShaderGraphEditor {
    pub fn new() -> Self {
        Self {
            library: ShaderGraphLibrary::new(),
            active_tool: ShaderEditorTool::Select,
            selected_nodes: HashSet::new(),
            dragging_node: None,
            connecting_from: None,
            search_query: String::new(),
            search_open: false,
            search_results: Vec::new(),
            compile_result: None,
            auto_compile: true,
            show_node_previews: false,
            grid_snap: true,
            grid_size: 16.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn compile_active(&mut self) {
        if let Some(graph) = self.library.active_graph_ref() {
            self.compile_result = Some(graph.compile());
        }
    }

    pub fn add_node_at(
        &mut self,
        kind: ShaderNodeKind,
        x: f32,
        y: f32,
    ) -> Option<ShaderNodeId> {
        let graph_id = self.library.active_graph?;
        let label = kind.label();
        let g = self.library.graph_mut(graph_id)?;
        let nx = if self.grid_snap { (x / self.grid_size).round() * self.grid_size } else { x };
        let ny = if self.grid_snap { (y / self.grid_size).round() * self.grid_size } else { y };
        let id = g.add_node(kind, nx, ny);
        self.undo_stack.push(ShaderEditorAction::AddNode {
            id, kind_label: label, graph: graph_id,
        });
        self.redo_stack.clear();
        if self.auto_compile { self.compile_active(); }
        Some(id)
    }

    pub fn remove_selected(&mut self) {
        let ids: Vec<ShaderNodeId> = self.selected_nodes.drain().collect();
        if let Some(graph_id) = self.library.active_graph {
            if let Some(g) = self.library.graph_mut(graph_id) {
                for id in ids {
                    g.remove_node(id);
                    self.undo_stack.push(ShaderEditorAction::RemoveNode {
                        id, graph: graph_id,
                    });
                }
            }
        }
        self.redo_stack.clear();
        if self.auto_compile { self.compile_active(); }
    }

    pub fn try_connect(&mut self, from: ShaderPortId, to: ShaderPortId) -> bool {
        let graph_id = match self.library.active_graph {
            Some(id) => id,
            None => return false,
        };
        let g = match self.library.graph_mut(graph_id) {
            Some(g) => g,
            None => return false,
        };
        if g.connect(from, to).is_ok() {
            self.undo_stack.push(ShaderEditorAction::AddConnection {
                from, to, graph: graph_id,
            });
            self.redo_stack.clear();
            if self.auto_compile { self.compile_active(); }
            true
        } else {
            false
        }
    }

    pub fn move_node(&mut self, id: ShaderNodeId, dx: f32, dy: f32) {
        let graph_id = match self.library.active_graph {
            Some(gid) => gid,
            None => return,
        };
        if let Some(g) = self.library.graph_mut(graph_id) {
            if let Some(node) = g.node_mut(id) {
                let from = node.position;
                let nx = if self.grid_snap {
                    ((from[0] + dx) / self.grid_size).round() * self.grid_size
                } else {
                    from[0] + dx
                };
                let ny = if self.grid_snap {
                    ((from[1] + dy) / self.grid_size).round() * self.grid_size
                } else {
                    from[1] + dy
                };
                node.position = [nx, ny];
                self.undo_stack.push(ShaderEditorAction::MoveNode {
                    id, from, to: [nx, ny],
                });
            }
        }
    }

    pub fn select_all(&mut self) {
        if let Some(gid) = self.library.active_graph {
            if let Some(g) = self.library.graph(gid) {
                self.selected_nodes = g.nodes().map(|n| n.id).collect();
            }
        }
    }

    pub fn deselect_all(&mut self) {
        self.selected_nodes.clear();
    }

    pub fn search(&mut self, query: &str) {
        self.search_query = query.to_string();
        let q = query.to_lowercase();
        self.search_results.clear();

        let candidates: &[(&str, &str, &'static str)] = &[
            ("Add",           "Math",    "Add two values component-wise"),
            ("Subtract",      "Math",    "Subtract B from A"),
            ("Multiply",      "Math",    "Multiply two values"),
            ("Divide",        "Math",    "Divide A by B (safe)"),
            ("Lerp",          "Math",    "Linear interpolation between A and B"),
            ("Clamp",         "Math",    "Clamp to [min, max]"),
            ("Smoothstep",    "Math",    "Hermite interpolation"),
            ("Power",         "Math",    "Raise base to exponent"),
            ("Sqrt",          "Math",    "Square root"),
            ("Abs",           "Math",    "Absolute value"),
            ("Sin",           "Trig",    "Sine of angle in radians"),
            ("Cos",           "Trig",    "Cosine of angle in radians"),
            ("Dot",           "Vector",  "Dot product of two vectors"),
            ("Cross",         "Vector",  "Cross product of two vec3"),
            ("Normalize",     "Vector",  "Normalize a vector to unit length"),
            ("Length",        "Vector",  "Length of a vector"),
            ("Reflect",       "Vector",  "Reflect a direction about a normal"),
            ("Split",         "Vector",  "Split vec4 into x, y, z, w"),
            ("Merge",         "Vector",  "Merge x, y, z, w into vec4"),
            ("Sample 2D",     "Texture", "Sample a 2D texture at UV"),
            ("Normal Map",    "Texture", "Decode a tangent-space normal map"),
            ("Fresnel",       "PBR",     "Schlick Fresnel approximation"),
            ("GGX NDF",       "PBR",     "GGX normal distribution function"),
            ("BRDF Specular", "PBR",     "Full Cook-Torrance specular BRDF"),
            ("SSS Approx",    "PBR",     "Subsurface scattering approximation"),
            ("Noise 2D",      "Noise",   "2D gradient noise"),
            ("Noise 3D",      "Noise",   "3D gradient noise"),
            ("FBM",           "Noise",   "Fractal Brownian Motion (multi-octave noise)"),
            ("Voronoi",       "Noise",   "Voronoi/Worley cellular noise"),
            ("PBR Output",    "Output",  "PBR material output node"),
            ("Unlit Output",  "Output",  "Unlit color output node"),
            ("Time",          "Input",   "Current time in seconds"),
            ("UV0",           "Input",   "Primary texture coordinates"),
            ("World Position","Input",   "World-space fragment position"),
            ("View Dir",      "Input",   "Normalized view direction"),
            ("Camera Position","Input",  "World-space camera position"),
            ("Vertex Normal", "Input",   "Interpolated vertex normal"),
        ];

        for (label, cat, desc) in candidates {
            if label.to_lowercase().contains(&q) || cat.to_lowercase().contains(&q) {
                self.search_results.push(NodeSearchResult {
                    kind_label: label.to_string(),
                    category: cat.to_string(),
                    description: desc,
                });
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            // Simplified: just record it
            self.redo_stack.push(action);
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.redo_stack.pop() {
            self.undo_stack.push(action);
        }
    }

    pub fn glsl_source(&self) -> Option<String> {
        self.compile_result.as_ref().map(|r| r.source.clone())
    }

    pub fn has_errors(&self) -> bool {
        self.compile_result.as_ref().map(|r| !r.errors.is_empty()).unwrap_or(false)
    }

    pub fn graph_stats(&self) -> Option<(usize, usize)> {
        let g = self.library.active_graph_ref()?;
        Some((g.node_count(), g.connection_count()))
    }
}

impl Default for ShaderGraphEditor {
    fn default() -> Self { Self::new() }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_compat() {
        assert!(ShaderValueType::Float.can_connect_to(ShaderValueType::Vec3));
        assert!(!ShaderValueType::Vec2.can_connect_to(ShaderValueType::Vec3));
        assert!(ShaderValueType::Vec3.can_connect_to(ShaderValueType::Vec4));
    }

    #[test]
    fn test_default_graph_compiles() {
        let lib = ShaderGraphLibrary::new();
        let gid = lib.active_graph.unwrap();
        let g = lib.graph(gid).unwrap();
        let result = g.compile();
        assert!(!result.source.is_empty());
    }

    #[test]
    fn test_cycle_detection() {
        let mut g = ShaderGraph::new(1, "test".into(), ShaderGraphType::Fragment);
        let a = g.add_node(ShaderNodeKind::ConstFloat(1.0), 0.0, 0.0);
        let b = g.add_node(ShaderNodeKind::Sin, 100.0, 0.0);
        g.connect(ShaderPortId::output(a, 0), ShaderPortId::input(b, 0)).unwrap();
        let res = g.connect(ShaderPortId::output(b, 0), ShaderPortId::input(a, 0));
        assert!(res.is_err());
    }

    #[test]
    fn test_topo_order() {
        let mut g = ShaderGraph::new(1, "test".into(), ShaderGraphType::Fragment);
        let a = g.add_node(ShaderNodeKind::ConstFloat(1.0), 0.0, 0.0);
        let b = g.add_node(ShaderNodeKind::Sin, 100.0, 0.0);
        let c = g.add_node(ShaderNodeKind::PbrOutput, 200.0, 0.0);
        g.connect(ShaderPortId::output(a, 0), ShaderPortId::input(b, 0)).unwrap();
        g.connect(ShaderPortId::output(b, 0), ShaderPortId::input(c, 0)).unwrap();
        let order = g.topological_order();
        let ai = order.iter().position(|&x| x == a).unwrap();
        let bi = order.iter().position(|&x| x == b).unwrap();
        let ci = order.iter().position(|&x| x == c).unwrap();
        assert!(ai < bi && bi < ci);
    }
}
