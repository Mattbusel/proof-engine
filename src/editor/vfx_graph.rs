
//! VFX Graph editor — node-based visual effects graph with GPU particle simulation,
//! event handling, spawn contexts, attribute system, and full block library.

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Attribute types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum VfxAttributeType {
    Float,
    Float2,
    Float3,
    Float4,
    Int,
    Uint,
    Bool,
    Color,
    Matrix4x4,
    Mesh,
    Texture2D,
    Texture3D,
    TextureCube,
    AnimationCurve,
    Gradient,
    SdfVolume,
    PointCache,
}

impl VfxAttributeType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Float => "Float",
            Self::Float2 => "Vector2",
            Self::Float3 => "Vector3",
            Self::Float4 => "Vector4",
            Self::Int => "Int",
            Self::Uint => "UInt",
            Self::Bool => "Bool",
            Self::Color => "Color",
            Self::Matrix4x4 => "Matrix4x4",
            Self::Mesh => "Mesh",
            Self::Texture2D => "Texture2D",
            Self::Texture3D => "Texture3D",
            Self::TextureCube => "TextureCube",
            Self::AnimationCurve => "AnimationCurve",
            Self::Gradient => "Gradient",
            Self::SdfVolume => "SDF Volume",
            Self::PointCache => "Point Cache",
        }
    }

    pub fn port_color(&self) -> Vec4 {
        match self {
            Self::Float => Vec4::new(0.55, 0.8, 0.55, 1.0),
            Self::Float2 => Vec4::new(0.55, 0.72, 0.87, 1.0),
            Self::Float3 => Vec4::new(0.55, 0.55, 0.87, 1.0),
            Self::Float4 => Vec4::new(0.72, 0.55, 0.87, 1.0),
            Self::Int => Vec4::new(0.5, 0.8, 0.5, 1.0),
            Self::Uint => Vec4::new(0.4, 0.75, 0.4, 1.0),
            Self::Bool => Vec4::new(0.87, 0.55, 0.55, 1.0),
            Self::Color => Vec4::new(0.87, 0.87, 0.25, 1.0),
            Self::Matrix4x4 => Vec4::new(0.75, 0.65, 0.45, 1.0),
            Self::Mesh => Vec4::new(0.87, 0.6, 0.35, 1.0),
            Self::Texture2D | Self::Texture3D | Self::TextureCube => Vec4::new(0.65, 0.45, 0.75, 1.0),
            Self::AnimationCurve => Vec4::new(0.35, 0.65, 0.65, 1.0),
            Self::Gradient => Vec4::new(0.85, 0.65, 0.35, 1.0),
            Self::SdfVolume => Vec4::new(0.35, 0.85, 0.65, 1.0),
            Self::PointCache => Vec4::new(0.65, 0.85, 0.35, 1.0),
        }
    }

    pub fn default_value(&self) -> VfxValue {
        match self {
            Self::Float => VfxValue::Float(0.0),
            Self::Float2 => VfxValue::Float2(Vec2::ZERO),
            Self::Float3 => VfxValue::Float3(Vec3::ZERO),
            Self::Float4 => VfxValue::Float4(Vec4::ZERO),
            Self::Int => VfxValue::Int(0),
            Self::Uint => VfxValue::Uint(0),
            Self::Bool => VfxValue::Bool(false),
            Self::Color => VfxValue::Color(Vec4::ONE),
            Self::Matrix4x4 => VfxValue::Matrix4x4(Mat4::IDENTITY),
            _ => VfxValue::None,
        }
    }
}

// ---------------------------------------------------------------------------
// VFX Values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum VfxValue {
    None,
    Float(f32),
    Float2(Vec2),
    Float3(Vec3),
    Float4(Vec4),
    Int(i32),
    Uint(u32),
    Bool(bool),
    Color(Vec4),
    Matrix4x4(Mat4),
}

impl VfxValue {
    pub fn as_float(&self) -> f32 {
        match self {
            Self::Float(v) => *v,
            Self::Int(v) => *v as f32,
            Self::Uint(v) => *v as f32,
            Self::Bool(v) => if *v { 1.0 } else { 0.0 },
            _ => 0.0,
        }
    }

    pub fn as_float3(&self) -> Vec3 {
        match self {
            Self::Float3(v) => *v,
            Self::Float4(v) => Vec3::new(v.x, v.y, v.z),
            Self::Color(v) => Vec3::new(v.x, v.y, v.z),
            Self::Float(v) => Vec3::splat(*v),
            _ => Vec3::ZERO,
        }
    }

    pub fn as_color(&self) -> Vec4 {
        match self {
            Self::Color(v) => *v,
            Self::Float4(v) => *v,
            Self::Float3(v) => Vec4::new(v.x, v.y, v.z, 1.0),
            _ => Vec4::ONE,
        }
    }

    pub fn type_of(&self) -> VfxAttributeType {
        match self {
            Self::None => VfxAttributeType::Bool,
            Self::Float(_) => VfxAttributeType::Float,
            Self::Float2(_) => VfxAttributeType::Float2,
            Self::Float3(_) => VfxAttributeType::Float3,
            Self::Float4(_) => VfxAttributeType::Float4,
            Self::Int(_) => VfxAttributeType::Int,
            Self::Uint(_) => VfxAttributeType::Uint,
            Self::Bool(_) => VfxAttributeType::Bool,
            Self::Color(_) => VfxAttributeType::Color,
            Self::Matrix4x4(_) => VfxAttributeType::Matrix4x4,
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in particle attributes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinAttribute {
    Position,
    Velocity,
    OldPosition,
    Direction,
    Size,
    SizeX,
    SizeY,
    SizeZ,
    Scale,
    Color,
    Alpha,
    AliveTime,
    TotalLifetime,
    Age,
    AgeNormalized,
    Mass,
    AngleX,
    AngleY,
    AngleZ,
    AngularVelocityX,
    AngularVelocityY,
    AngularVelocityZ,
    TexIndex,
    PivotX,
    PivotY,
    PivotZ,
    Random,
    ParticleId,
    SpawnIndex,
    StripIndex,
}

impl BuiltinAttribute {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Position => "position",
            Self::Velocity => "velocity",
            Self::OldPosition => "oldPosition",
            Self::Direction => "direction",
            Self::Size => "size",
            Self::SizeX => "sizeX",
            Self::SizeY => "sizeY",
            Self::SizeZ => "sizeZ",
            Self::Scale => "scale",
            Self::Color => "color",
            Self::Alpha => "alpha",
            Self::AliveTime => "alive",
            Self::TotalLifetime => "lifetime",
            Self::Age => "age",
            Self::AgeNormalized => "ageNormalized",
            Self::Mass => "mass",
            Self::AngleX => "angleX",
            Self::AngleY => "angleY",
            Self::AngleZ => "angleZ",
            Self::AngularVelocityX => "angularVelocityX",
            Self::AngularVelocityY => "angularVelocityY",
            Self::AngularVelocityZ => "angularVelocityZ",
            Self::TexIndex => "texIndex",
            Self::PivotX => "pivotX",
            Self::PivotY => "pivotY",
            Self::PivotZ => "pivotZ",
            Self::Random => "random",
            Self::ParticleId => "particleId",
            Self::SpawnIndex => "spawnIndex",
            Self::StripIndex => "stripIndex",
        }
    }

    pub fn attribute_type(&self) -> VfxAttributeType {
        match self {
            Self::Position | Self::Velocity | Self::OldPosition | Self::Direction => VfxAttributeType::Float3,
            Self::Color => VfxAttributeType::Color,
            Self::Alpha | Self::Size | Self::SizeX | Self::SizeY | Self::SizeZ | Self::Scale => VfxAttributeType::Float,
            Self::AliveTime | Self::TotalLifetime | Self::Age | Self::AgeNormalized | Self::Mass => VfxAttributeType::Float,
            Self::AngleX | Self::AngleY | Self::AngleZ => VfxAttributeType::Float,
            Self::AngularVelocityX | Self::AngularVelocityY | Self::AngularVelocityZ => VfxAttributeType::Float,
            Self::TexIndex | Self::ParticleId | Self::SpawnIndex | Self::StripIndex => VfxAttributeType::Uint,
            Self::PivotX | Self::PivotY | Self::PivotZ | Self::Random => VfxAttributeType::Float,
        }
    }
}

// ---------------------------------------------------------------------------
// VFX Node port definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VfxPort {
    pub name: String,
    pub data_type: VfxAttributeType,
    pub is_input: bool,
    pub optional: bool,
    pub default_value: VfxValue,
    pub tooltip: String,
}

impl VfxPort {
    pub fn input(name: &str, data_type: VfxAttributeType) -> Self {
        let default_value = data_type.default_value();
        Self {
            name: name.to_string(),
            data_type,
            is_input: true,
            optional: false,
            default_value,
            tooltip: String::new(),
        }
    }

    pub fn output(name: &str, data_type: VfxAttributeType) -> Self {
        let default_value = data_type.default_value();
        Self {
            name: name.to_string(),
            data_type,
            is_input: false,
            optional: false,
            default_value,
            tooltip: String::new(),
        }
    }

    pub fn optional(mut self) -> Self { self.optional = true; self }
    pub fn with_tooltip(mut self, tip: &str) -> Self { self.tooltip = tip.to_string(); self }
}

// ---------------------------------------------------------------------------
// VFX Node kinds / block library
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum VfxNodeKind {
    // --- Contexts ---
    SpawnContext,
    InitializeContext,
    UpdateContext,
    OutputParticleQuad,
    OutputParticleMesh,
    OutputParticleStrip,

    // --- Spawn operators ---
    ConstantRate,
    BurstSpawn,
    PeriodicBurst,
    OnCollision,
    OnTrigger,
    SpawnOnDeath,
    SpawnFromPointCache,

    // --- Initialize operators ---
    SetPositionShape,
    SetPositionSphere,
    SetPositionCone,
    SetPositionBox,
    SetPositionLine,
    SetPositionTorus,
    SetPositionMeshSurface,
    SetVelocityRandom,
    SetVelocityTangent,
    SetLifetime,
    SetSize,
    SetColor,
    SetColorFromGradient,
    SetAlpha,
    SetMass,
    SetAngle,
    SetTexIndex,
    InheritSourceVelocity,
    InheritSourceColor,
    InheritSourcePosition,

    // --- Update operators ---
    Gravity,
    Drag,
    Turbulence,
    VelocityField,
    ConformToSphere,
    ConformToSdf,
    OrbitForce,
    LinearDrag,
    AngularDrag,
    AttractToPosition,
    FlipbookAnimation,
    RotateOverTime,
    ScaleOverLife,
    ColorOverLife,
    AlphaOverLife,
    SpeedLimiter,
    Collision,
    KillOnCollision,
    KillOnBounds,
    TriggerOnDeath,
    UpdatePosition,
    EulerIntegration,
    Noise3D,
    CurlNoise,

    // --- Math nodes ---
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Sqrt,
    Abs,
    Sin,
    Cos,
    Tan,
    Atan2,
    Floor,
    Ceil,
    Round,
    Frac,
    Clamp,
    Lerp,
    Step,
    SmoothStep,
    Min,
    Max,
    Remap,
    Dot,
    Cross,
    Normalize,
    Length,
    Distance,
    Swizzle,
    Combine,
    Split,
    Negate,
    OneMinus,
    Reciprocal,

    // --- Sample operators ---
    SampleGradient,
    SampleCurve,
    SampleTexture2D,
    SampleTexture3D,
    SampleSdf,
    SampleMesh,
    SampleNoise2D,
    SampleNoise3D,

    // --- Attribute nodes ---
    GetAttribute,
    SetAttribute,
    GetBuiltin,

    // --- Flow control ---
    Branch,
    Loop,
    Random,
    RandomPerComponent,
    Sequence,

    // --- Utility ---
    Comment,
    Sticky,
    ExposedParameter,
    CustomHlsl,
}

impl VfxNodeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SpawnContext => "Spawn",
            Self::InitializeContext => "Initialize",
            Self::UpdateContext => "Update",
            Self::OutputParticleQuad => "Output Particle (Quad)",
            Self::OutputParticleMesh => "Output Particle (Mesh)",
            Self::OutputParticleStrip => "Output Particle (Strip)",
            Self::ConstantRate => "Constant Rate",
            Self::BurstSpawn => "Burst",
            Self::PeriodicBurst => "Periodic Burst",
            Self::OnCollision => "On Collision",
            Self::OnTrigger => "On Trigger",
            Self::SpawnOnDeath => "Spawn on Death",
            Self::SpawnFromPointCache => "Spawn from Point Cache",
            Self::SetPositionShape => "Set Position (Shape)",
            Self::SetPositionSphere => "Set Position (Sphere)",
            Self::SetPositionCone => "Set Position (Cone)",
            Self::SetPositionBox => "Set Position (Box)",
            Self::SetPositionLine => "Set Position (Line)",
            Self::SetPositionTorus => "Set Position (Torus)",
            Self::SetPositionMeshSurface => "Set Position (Mesh Surface)",
            Self::SetVelocityRandom => "Set Velocity (Random)",
            Self::SetVelocityTangent => "Set Velocity (Tangent)",
            Self::SetLifetime => "Set Lifetime",
            Self::SetSize => "Set Size",
            Self::SetColor => "Set Color",
            Self::SetColorFromGradient => "Set Color (Gradient)",
            Self::SetAlpha => "Set Alpha",
            Self::SetMass => "Set Mass",
            Self::SetAngle => "Set Angle",
            Self::SetTexIndex => "Set Tex Index",
            Self::InheritSourceVelocity => "Inherit Source Velocity",
            Self::InheritSourceColor => "Inherit Source Color",
            Self::InheritSourcePosition => "Inherit Source Position",
            Self::Gravity => "Gravity",
            Self::Drag => "Drag",
            Self::Turbulence => "Turbulence",
            Self::VelocityField => "Velocity Field",
            Self::ConformToSphere => "Conform to Sphere",
            Self::ConformToSdf => "Conform to SDF",
            Self::OrbitForce => "Orbit Force",
            Self::LinearDrag => "Linear Drag",
            Self::AngularDrag => "Angular Drag",
            Self::AttractToPosition => "Attract to Position",
            Self::FlipbookAnimation => "Flipbook",
            Self::RotateOverTime => "Rotate over Time",
            Self::ScaleOverLife => "Scale over Life",
            Self::ColorOverLife => "Color over Life",
            Self::AlphaOverLife => "Alpha over Life",
            Self::SpeedLimiter => "Speed Limiter",
            Self::Collision => "Collision",
            Self::KillOnCollision => "Kill on Collision",
            Self::KillOnBounds => "Kill on Bounds",
            Self::TriggerOnDeath => "Trigger on Death",
            Self::UpdatePosition => "Update Position",
            Self::EulerIntegration => "Euler Integration",
            Self::Noise3D => "Noise 3D",
            Self::CurlNoise => "Curl Noise",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Multiply => "Multiply",
            Self::Divide => "Divide",
            Self::Power => "Power",
            Self::Sqrt => "Sqrt",
            Self::Abs => "Abs",
            Self::Sin => "Sin",
            Self::Cos => "Cos",
            Self::Tan => "Tan",
            Self::Atan2 => "Atan2",
            Self::Floor => "Floor",
            Self::Ceil => "Ceil",
            Self::Round => "Round",
            Self::Frac => "Frac",
            Self::Clamp => "Clamp",
            Self::Lerp => "Lerp",
            Self::Step => "Step",
            Self::SmoothStep => "Smooth Step",
            Self::Min => "Min",
            Self::Max => "Max",
            Self::Remap => "Remap",
            Self::Dot => "Dot Product",
            Self::Cross => "Cross Product",
            Self::Normalize => "Normalize",
            Self::Length => "Length",
            Self::Distance => "Distance",
            Self::Swizzle => "Swizzle",
            Self::Combine => "Combine",
            Self::Split => "Split",
            Self::Negate => "Negate",
            Self::OneMinus => "One Minus",
            Self::Reciprocal => "Reciprocal",
            Self::SampleGradient => "Sample Gradient",
            Self::SampleCurve => "Sample Curve",
            Self::SampleTexture2D => "Sample Texture 2D",
            Self::SampleTexture3D => "Sample Texture 3D",
            Self::SampleSdf => "Sample SDF",
            Self::SampleMesh => "Sample Mesh",
            Self::SampleNoise2D => "Sample Noise 2D",
            Self::SampleNoise3D => "Sample Noise 3D",
            Self::GetAttribute => "Get Attribute",
            Self::SetAttribute => "Set Attribute",
            Self::GetBuiltin => "Get Builtin Attribute",
            Self::Branch => "Branch",
            Self::Loop => "Loop",
            Self::Random => "Random",
            Self::RandomPerComponent => "Random (Per Component)",
            Self::Sequence => "Sequence",
            Self::Comment => "Comment",
            Self::Sticky => "Sticky Note",
            Self::ExposedParameter => "Exposed Parameter",
            Self::CustomHlsl => "Custom HLSL",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::SpawnContext | Self::InitializeContext | Self::UpdateContext
            | Self::OutputParticleQuad | Self::OutputParticleMesh | Self::OutputParticleStrip => "Context",
            Self::ConstantRate | Self::BurstSpawn | Self::PeriodicBurst
            | Self::OnCollision | Self::OnTrigger | Self::SpawnOnDeath | Self::SpawnFromPointCache => "Spawn",
            Self::SetPositionShape | Self::SetPositionSphere | Self::SetPositionCone
            | Self::SetPositionBox | Self::SetPositionLine | Self::SetPositionTorus
            | Self::SetPositionMeshSurface | Self::SetVelocityRandom | Self::SetVelocityTangent
            | Self::SetLifetime | Self::SetSize | Self::SetColor | Self::SetColorFromGradient
            | Self::SetAlpha | Self::SetMass | Self::SetAngle | Self::SetTexIndex
            | Self::InheritSourceVelocity | Self::InheritSourceColor | Self::InheritSourcePosition => "Initialize",
            Self::Gravity | Self::Drag | Self::Turbulence | Self::VelocityField
            | Self::ConformToSphere | Self::ConformToSdf | Self::OrbitForce | Self::LinearDrag
            | Self::AngularDrag | Self::AttractToPosition | Self::FlipbookAnimation
            | Self::RotateOverTime | Self::ScaleOverLife | Self::ColorOverLife | Self::AlphaOverLife
            | Self::SpeedLimiter | Self::Collision | Self::KillOnCollision | Self::KillOnBounds
            | Self::TriggerOnDeath | Self::UpdatePosition | Self::EulerIntegration
            | Self::Noise3D | Self::CurlNoise => "Update",
            Self::Add | Self::Subtract | Self::Multiply | Self::Divide | Self::Power
            | Self::Sqrt | Self::Abs | Self::Sin | Self::Cos | Self::Tan | Self::Atan2
            | Self::Floor | Self::Ceil | Self::Round | Self::Frac | Self::Clamp
            | Self::Lerp | Self::Step | Self::SmoothStep | Self::Min | Self::Max
            | Self::Remap | Self::Dot | Self::Cross | Self::Normalize | Self::Length
            | Self::Distance | Self::Swizzle | Self::Combine | Self::Split
            | Self::Negate | Self::OneMinus | Self::Reciprocal => "Math",
            Self::SampleGradient | Self::SampleCurve | Self::SampleTexture2D
            | Self::SampleTexture3D | Self::SampleSdf | Self::SampleMesh
            | Self::SampleNoise2D | Self::SampleNoise3D => "Sample",
            Self::GetAttribute | Self::SetAttribute | Self::GetBuiltin => "Attribute",
            Self::Branch | Self::Loop | Self::Random | Self::RandomPerComponent | Self::Sequence => "Flow",
            Self::Comment | Self::Sticky | Self::ExposedParameter | Self::CustomHlsl => "Utility",
        }
    }

    pub fn header_color(&self) -> Vec4 {
        match self.category() {
            "Context" => Vec4::new(0.16, 0.29, 0.48, 1.0),
            "Spawn" => Vec4::new(0.29, 0.48, 0.16, 1.0),
            "Initialize" => Vec4::new(0.16, 0.48, 0.35, 1.0),
            "Update" => Vec4::new(0.48, 0.29, 0.16, 1.0),
            "Math" => Vec4::new(0.35, 0.22, 0.48, 1.0),
            "Sample" => Vec4::new(0.48, 0.16, 0.35, 1.0),
            "Attribute" => Vec4::new(0.22, 0.40, 0.48, 1.0),
            "Flow" => Vec4::new(0.48, 0.40, 0.16, 1.0),
            _ => Vec4::new(0.30, 0.30, 0.30, 1.0),
        }
    }

    pub fn default_ports(&self) -> (Vec<VfxPort>, Vec<VfxPort>) {
        match self {
            Self::ConstantRate => (
                vec![VfxPort::input("Rate", VfxAttributeType::Float)],
                vec![VfxPort::output("Spawn Count", VfxAttributeType::Uint)],
            ),
            Self::SetLifetime => (
                vec![VfxPort::input("Lifetime", VfxAttributeType::Float)],
                vec![],
            ),
            Self::SetColor => (
                vec![VfxPort::input("Color", VfxAttributeType::Color)],
                vec![],
            ),
            Self::SetSize => (
                vec![VfxPort::input("Size", VfxAttributeType::Float)],
                vec![],
            ),
            Self::Add | Self::Subtract | Self::Multiply | Self::Divide => (
                vec![
                    VfxPort::input("A", VfxAttributeType::Float),
                    VfxPort::input("B", VfxAttributeType::Float),
                ],
                vec![VfxPort::output("Result", VfxAttributeType::Float)],
            ),
            Self::Lerp => (
                vec![
                    VfxPort::input("A", VfxAttributeType::Float),
                    VfxPort::input("B", VfxAttributeType::Float),
                    VfxPort::input("T", VfxAttributeType::Float),
                ],
                vec![VfxPort::output("Result", VfxAttributeType::Float)],
            ),
            Self::Clamp => (
                vec![
                    VfxPort::input("Value", VfxAttributeType::Float),
                    VfxPort::input("Min", VfxAttributeType::Float),
                    VfxPort::input("Max", VfxAttributeType::Float),
                ],
                vec![VfxPort::output("Result", VfxAttributeType::Float)],
            ),
            Self::SampleGradient => (
                vec![
                    VfxPort::input("Gradient", VfxAttributeType::Gradient),
                    VfxPort::input("T", VfxAttributeType::Float),
                ],
                vec![VfxPort::output("Color", VfxAttributeType::Color)],
            ),
            Self::SampleCurve => (
                vec![
                    VfxPort::input("Curve", VfxAttributeType::AnimationCurve),
                    VfxPort::input("T", VfxAttributeType::Float),
                ],
                vec![VfxPort::output("Value", VfxAttributeType::Float)],
            ),
            Self::GetBuiltin => (
                vec![],
                vec![VfxPort::output("Value", VfxAttributeType::Float)],
            ),
            Self::Gravity => (
                vec![VfxPort::input("Gravity", VfxAttributeType::Float3).optional()],
                vec![],
            ),
            Self::Noise3D => (
                vec![
                    VfxPort::input("Position", VfxAttributeType::Float3),
                    VfxPort::input("Frequency", VfxAttributeType::Float),
                    VfxPort::input("Octaves", VfxAttributeType::Int).optional(),
                ],
                vec![
                    VfxPort::output("Noise", VfxAttributeType::Float),
                    VfxPort::output("Noise3D", VfxAttributeType::Float3),
                ],
            ),
            Self::Branch => (
                vec![
                    VfxPort::input("Condition", VfxAttributeType::Bool),
                    VfxPort::input("True", VfxAttributeType::Float),
                    VfxPort::input("False", VfxAttributeType::Float),
                ],
                vec![VfxPort::output("Result", VfxAttributeType::Float)],
            ),
            Self::Combine => (
                vec![
                    VfxPort::input("X", VfxAttributeType::Float),
                    VfxPort::input("Y", VfxAttributeType::Float),
                    VfxPort::input("Z", VfxAttributeType::Float).optional(),
                    VfxPort::input("W", VfxAttributeType::Float).optional(),
                ],
                vec![VfxPort::output("Vector", VfxAttributeType::Float4)],
            ),
            Self::Split => (
                vec![VfxPort::input("Vector", VfxAttributeType::Float4)],
                vec![
                    VfxPort::output("X", VfxAttributeType::Float),
                    VfxPort::output("Y", VfxAttributeType::Float),
                    VfxPort::output("Z", VfxAttributeType::Float),
                    VfxPort::output("W", VfxAttributeType::Float),
                ],
            ),
            _ => (vec![], vec![]),
        }
    }
}

// ---------------------------------------------------------------------------
// VFX Graph Node
// ---------------------------------------------------------------------------

static mut VFX_NODE_ID_COUNTER: u64 = 1;
fn next_vfx_node_id() -> u64 {
    unsafe {
        let id = VFX_NODE_ID_COUNTER;
        VFX_NODE_ID_COUNTER += 1;
        id
    }
}

#[derive(Debug, Clone)]
pub struct VfxNode {
    pub id: u64,
    pub kind: VfxNodeKind,
    pub position: Vec2,
    pub size: Vec2,
    pub title_override: Option<String>,
    pub inputs: Vec<VfxPort>,
    pub outputs: Vec<VfxPort>,
    pub param_values: HashMap<String, VfxValue>,
    pub comment: String,
    pub collapsed: bool,
    pub enabled: bool,
    pub preview_enabled: bool,
    pub error_message: Option<String>,
    pub custom_hlsl_code: String,
    pub exposed_param_name: String,
    pub builtin_attribute: BuiltinAttribute,
}

impl VfxNode {
    pub fn new(kind: VfxNodeKind, position: Vec2) -> Self {
        let (inputs, outputs) = kind.default_ports();
        let mut node = Self {
            id: next_vfx_node_id(),
            kind,
            position,
            size: Vec2::new(200.0, 80.0),
            title_override: None,
            inputs,
            outputs,
            param_values: HashMap::new(),
            comment: String::new(),
            collapsed: false,
            enabled: true,
            preview_enabled: false,
            error_message: None,
            custom_hlsl_code: String::new(),
            exposed_param_name: String::new(),
            builtin_attribute: BuiltinAttribute::Age,
        };
        node.recalculate_size();
        node
    }

    pub fn label(&self) -> &str {
        if let Some(ref t) = self.title_override {
            t.as_str()
        } else {
            self.kind.label()
        }
    }

    pub fn recalculate_size(&mut self) {
        let port_count = self.inputs.len().max(self.outputs.len());
        let header_h = 28.0;
        let port_h = 22.0;
        let footer_h = 4.0;
        self.size.x = 200.0;
        self.size.y = header_h + port_count as f32 * port_h + footer_h;
        if self.size.y < 60.0 { self.size.y = 60.0; }
    }

    pub fn input_port_position(&self, index: usize) -> Vec2 {
        let y = self.position.y + 28.0 + index as f32 * 22.0 + 11.0;
        Vec2::new(self.position.x, y)
    }

    pub fn output_port_position(&self, index: usize) -> Vec2 {
        let y = self.position.y + 28.0 + index as f32 * 22.0 + 11.0;
        Vec2::new(self.position.x + self.size.x, y)
    }

    pub fn contains_point(&self, pt: Vec2) -> bool {
        pt.x >= self.position.x && pt.x <= self.position.x + self.size.x
            && pt.y >= self.position.y && pt.y <= self.position.y + self.size.y
    }

    pub fn set_param(&mut self, name: &str, value: VfxValue) {
        self.param_values.insert(name.to_string(), value);
    }

    pub fn get_param(&self, name: &str) -> Option<&VfxValue> {
        self.param_values.get(name)
    }
}

// ---------------------------------------------------------------------------
// VFX Connection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VfxConnection {
    pub id: u64,
    pub from_node: u64,
    pub from_port: usize,
    pub to_node: u64,
    pub to_port: usize,
}

// ---------------------------------------------------------------------------
// Context block (groups of nodes within a context)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VfxContextKind {
    Spawn,
    Initialize,
    Update,
    Output,
}

#[derive(Debug, Clone)]
pub struct VfxContext {
    pub id: u64,
    pub kind: VfxContextKind,
    pub node_id: u64,
    pub operator_node_ids: Vec<u64>,
    pub enabled: bool,
    pub capacity: u32,
    pub label: String,
}

impl VfxContext {
    pub fn new(kind: VfxContextKind, node_id: u64) -> Self {
        let label = match kind {
            VfxContextKind::Spawn => "Spawn".to_string(),
            VfxContextKind::Initialize => "Initialize".to_string(),
            VfxContextKind::Update => "Update".to_string(),
            VfxContextKind::Output => "Output".to_string(),
        };
        Self {
            id: next_vfx_node_id(),
            kind,
            node_id,
            operator_node_ids: Vec::new(),
            enabled: true,
            capacity: 4096,
            label,
        }
    }
}

// ---------------------------------------------------------------------------
// Exposed parameter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ExposedParameter {
    pub name: String,
    pub data_type: VfxAttributeType,
    pub value: VfxValue,
    pub min: f32,
    pub max: f32,
    pub tooltip: String,
    pub exposed: bool,
}

impl ExposedParameter {
    pub fn new(name: &str, data_type: VfxAttributeType) -> Self {
        let value = data_type.default_value();
        Self {
            name: name.to_string(),
            data_type,
            value,
            min: 0.0,
            max: 1.0,
            tooltip: String::new(),
            exposed: true,
        }
    }
}

// ---------------------------------------------------------------------------
// VFX Graph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VfxGraph {
    pub name: String,
    pub nodes: Vec<VfxNode>,
    pub connections: Vec<VfxConnection>,
    pub contexts: Vec<VfxContext>,
    pub exposed_parameters: Vec<ExposedParameter>,
    pub next_conn_id: u64,
    pub dirty: bool,
    pub last_compile_error: Option<String>,
    pub compile_warnings: Vec<String>,
    pub particle_capacity: u32,
    pub simulation_space: SimulationSpace,
    pub culling_mode: CullingMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimulationSpace { Local, World }
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CullingMode { Automatic, AlwaysSimulate, StopSimulating, Pause }

impl VfxGraph {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            nodes: Vec::new(),
            connections: Vec::new(),
            contexts: Vec::new(),
            exposed_parameters: Vec::new(),
            next_conn_id: 1,
            dirty: false,
            last_compile_error: None,
            compile_warnings: Vec::new(),
            particle_capacity: 4096,
            simulation_space: SimulationSpace::World,
            culling_mode: CullingMode::Automatic,
        }
    }

    pub fn add_node(&mut self, node: VfxNode) -> u64 {
        let id = node.id;
        self.nodes.push(node);
        self.dirty = true;
        id
    }

    pub fn remove_node(&mut self, id: u64) {
        self.nodes.retain(|n| n.id != id);
        self.connections.retain(|c| c.from_node != id && c.to_node != id);
        for ctx in &mut self.contexts {
            ctx.operator_node_ids.retain(|&nid| nid != id);
        }
        self.dirty = true;
    }

    pub fn find_node(&self, id: u64) -> Option<&VfxNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn find_node_mut(&mut self, id: u64) -> Option<&mut VfxNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn connect(&mut self, from_node: u64, from_port: usize, to_node: u64, to_port: usize) -> u64 {
        // Remove existing connection to the same input port
        self.connections.retain(|c| !(c.to_node == to_node && c.to_port == to_port));
        let id = self.next_conn_id;
        self.next_conn_id += 1;
        self.connections.push(VfxConnection { id, from_node, from_port, to_node, to_port });
        self.dirty = true;
        id
    }

    pub fn disconnect(&mut self, conn_id: u64) {
        self.connections.retain(|c| c.id != conn_id);
        self.dirty = true;
    }

    pub fn validate(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let node_ids: std::collections::HashSet<u64> = self.nodes.iter().map(|n| n.id).collect();
        for conn in &self.connections {
            if !node_ids.contains(&conn.from_node) {
                errors.push(format!("Connection {} references missing node {}", conn.id, conn.from_node));
            }
            if !node_ids.contains(&conn.to_node) {
                errors.push(format!("Connection {} references missing node {}", conn.id, conn.to_node));
            }
        }
        // Check all required inputs are connected
        let connected_inputs: std::collections::HashSet<(u64, usize)> =
            self.connections.iter().map(|c| (c.to_node, c.to_port)).collect();
        for node in &self.nodes {
            for (i, port) in node.inputs.iter().enumerate() {
                if !port.optional && !connected_inputs.contains(&(node.id, i)) {
                    // Only warn, not error (they may have inline values)
                    self.compile_warnings.push(format!(
                        "Node '{}' port '{}' has no connection (will use default)",
                        node.label(), port.name
                    ));
                }
            }
        }
        if errors.is_empty() {
            self.last_compile_error = None;
        } else {
            self.last_compile_error = Some(errors.join("; "));
        }
        errors
    }

    pub fn topological_order(&self) -> Vec<u64> {
        let mut in_degree: HashMap<u64, usize> = self.nodes.iter().map(|n| (n.id, 0)).collect();
        let mut adjacency: HashMap<u64, Vec<u64>> = self.nodes.iter().map(|n| (n.id, Vec::new())).collect();
        for conn in &self.connections {
            *in_degree.entry(conn.to_node).or_insert(0) += 1;
            adjacency.entry(conn.from_node).or_default().push(conn.to_node);
        }
        let mut queue: std::collections::VecDeque<u64> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();
        let mut order = Vec::new();
        while let Some(id) = queue.pop_front() {
            order.push(id);
            if let Some(neighbors) = adjacency.get(&id) {
                for &next in neighbors {
                    let deg = in_degree.entry(next).or_insert(0);
                    if *deg > 0 { *deg -= 1; }
                    if *deg == 0 { queue.push_back(next); }
                }
            }
        }
        order
    }

    pub fn generate_hlsl(&self) -> String {
        let order = self.topological_order();
        let mut lines = Vec::new();
        lines.push("// Auto-generated VFX HLSL".to_string());
        lines.push("struct ParticleData {".to_string());
        lines.push("    float3 position;".to_string());
        lines.push("    float3 velocity;".to_string());
        lines.push("    float4 color;".to_string());
        lines.push("    float size;".to_string());
        lines.push("    float age;".to_string());
        lines.push("    float lifetime;".to_string());
        lines.push("};".to_string());
        lines.push(String::new());
        lines.push("[numthreads(64, 1, 1)]".to_string());
        lines.push("void VFXMain(uint id : SV_DispatchThreadID) {".to_string());
        lines.push("    ParticleData p = particleBuffer[id];".to_string());
        for &node_id in &order {
            if let Some(node) = self.find_node(node_id) {
                let snippet = self.node_to_hlsl(node);
                if !snippet.is_empty() {
                    lines.push(format!("    // {}", node.label()));
                    lines.push(format!("    {}", snippet));
                }
            }
        }
        lines.push("    particleBuffer[id] = p;".to_string());
        lines.push("}".to_string());
        lines.join("\n")
    }

    fn node_to_hlsl(&self, node: &VfxNode) -> String {
        match node.kind {
            VfxNodeKind::Gravity => {
                let g = node.get_param("Gravity")
                    .map(|v| v.as_float3())
                    .unwrap_or(Vec3::new(0.0, -9.81, 0.0));
                format!("p.velocity += float3({}, {}, {}) * deltaTime;", g.x, g.y, g.z)
            }
            VfxNodeKind::Drag => {
                let drag = node.get_param("Drag")
                    .map(|v| v.as_float())
                    .unwrap_or(0.1);
                format!("p.velocity *= (1.0 - {:.4} * deltaTime);", drag)
            }
            VfxNodeKind::EulerIntegration => {
                "p.position += p.velocity * deltaTime;".to_string()
            }
            VfxNodeKind::ScaleOverLife => {
                "p.size = lerp(p.size, 0.0, saturate(p.age / p.lifetime));".to_string()
            }
            VfxNodeKind::AlphaOverLife => {
                "p.color.a = saturate(1.0 - p.age / p.lifetime);".to_string()
            }
            VfxNodeKind::CustomHlsl => node.custom_hlsl_code.clone(),
            _ => String::new(),
        }
    }

    pub fn estimate_memory_bytes(&self) -> usize {
        // Per-particle: position(12) + velocity(12) + color(16) + size(4) + age(4) + lifetime(4) = 52 bytes
        self.particle_capacity as usize * 52
    }

    pub fn build_fire_effect() -> Self {
        let mut graph = Self::new("Fire");
        graph.particle_capacity = 2048;

        let spawn = graph.add_node(VfxNode::new(VfxNodeKind::SpawnContext, Vec2::new(20.0, 20.0)));
        let rate = graph.add_node(VfxNode::new(VfxNodeKind::ConstantRate, Vec2::new(20.0, 80.0)));
        if let Some(n) = graph.find_node_mut(rate) {
            n.set_param("Rate", VfxValue::Float(50.0));
        }

        let init = graph.add_node(VfxNode::new(VfxNodeKind::InitializeContext, Vec2::new(300.0, 20.0)));
        let set_pos = graph.add_node(VfxNode::new(VfxNodeKind::SetPositionCone, Vec2::new(300.0, 80.0)));
        let set_life = graph.add_node(VfxNode::new(VfxNodeKind::SetLifetime, Vec2::new(300.0, 140.0)));
        if let Some(n) = graph.find_node_mut(set_life) {
            n.set_param("Lifetime", VfxValue::Float(1.5));
        }
        let set_color = graph.add_node(VfxNode::new(VfxNodeKind::SetColor, Vec2::new(300.0, 200.0)));
        if let Some(n) = graph.find_node_mut(set_color) {
            n.set_param("Color", VfxValue::Color(Vec4::new(1.0, 0.4, 0.0, 1.0)));
        }

        let update = graph.add_node(VfxNode::new(VfxNodeKind::UpdateContext, Vec2::new(580.0, 20.0)));
        let gravity_node = graph.add_node(VfxNode::new(VfxNodeKind::Gravity, Vec2::new(580.0, 80.0)));
        if let Some(n) = graph.find_node_mut(gravity_node) {
            n.set_param("Gravity", VfxValue::Float3(Vec3::new(0.0, -2.0, 0.0)));
        }
        let turbulence = graph.add_node(VfxNode::new(VfxNodeKind::Turbulence, Vec2::new(580.0, 140.0)));
        let color_life = graph.add_node(VfxNode::new(VfxNodeKind::ColorOverLife, Vec2::new(580.0, 200.0)));
        let alpha_life = graph.add_node(VfxNode::new(VfxNodeKind::AlphaOverLife, Vec2::new(580.0, 260.0)));
        let euler = graph.add_node(VfxNode::new(VfxNodeKind::EulerIntegration, Vec2::new(580.0, 320.0)));

        let output = graph.add_node(VfxNode::new(VfxNodeKind::OutputParticleQuad, Vec2::new(860.0, 20.0)));

        // Wire spawn -> rate
        graph.connect(rate, 0, spawn, 0);
        // Wire contexts (conceptual ordering)
        graph.connect(spawn, 0, init, 0);
        graph.connect(init, 0, update, 0);
        graph.connect(update, 0, output, 0);
        // Operators
        graph.connect(set_pos, 0, init, 1);
        graph.connect(set_life, 0, init, 2);
        graph.connect(set_color, 0, init, 3);
        graph.connect(gravity_node, 0, update, 1);
        graph.connect(turbulence, 0, update, 2);
        graph.connect(color_life, 0, update, 3);
        graph.connect(alpha_life, 0, update, 4);
        graph.connect(euler, 0, update, 5);

        graph
    }

    pub fn build_smoke_effect() -> Self {
        let mut graph = Self::new("Smoke");
        graph.particle_capacity = 1024;

        let spawn = graph.add_node(VfxNode::new(VfxNodeKind::SpawnContext, Vec2::new(20.0, 20.0)));
        let rate = graph.add_node(VfxNode::new(VfxNodeKind::ConstantRate, Vec2::new(20.0, 80.0)));
        if let Some(n) = graph.find_node_mut(rate) {
            n.set_param("Rate", VfxValue::Float(15.0));
        }

        let init = graph.add_node(VfxNode::new(VfxNodeKind::InitializeContext, Vec2::new(300.0, 20.0)));
        let set_pos = graph.add_node(VfxNode::new(VfxNodeKind::SetPositionSphere, Vec2::new(300.0, 80.0)));
        let set_life = graph.add_node(VfxNode::new(VfxNodeKind::SetLifetime, Vec2::new(300.0, 140.0)));
        if let Some(n) = graph.find_node_mut(set_life) {
            n.set_param("Lifetime", VfxValue::Float(4.0));
        }
        let set_size = graph.add_node(VfxNode::new(VfxNodeKind::SetSize, Vec2::new(300.0, 200.0)));
        if let Some(n) = graph.find_node_mut(set_size) {
            n.set_param("Size", VfxValue::Float(0.5));
        }
        let set_color = graph.add_node(VfxNode::new(VfxNodeKind::SetColor, Vec2::new(300.0, 260.0)));
        if let Some(n) = graph.find_node_mut(set_color) {
            n.set_param("Color", VfxValue::Color(Vec4::new(0.6, 0.6, 0.6, 0.8)));
        }

        let update = graph.add_node(VfxNode::new(VfxNodeKind::UpdateContext, Vec2::new(580.0, 20.0)));
        let gravity_node = graph.add_node(VfxNode::new(VfxNodeKind::Gravity, Vec2::new(580.0, 80.0)));
        if let Some(n) = graph.find_node_mut(gravity_node) {
            n.set_param("Gravity", VfxValue::Float3(Vec3::new(0.0, 0.5, 0.0)));
        }
        let scale_life = graph.add_node(VfxNode::new(VfxNodeKind::ScaleOverLife, Vec2::new(580.0, 140.0)));
        let alpha_life = graph.add_node(VfxNode::new(VfxNodeKind::AlphaOverLife, Vec2::new(580.0, 200.0)));
        let euler = graph.add_node(VfxNode::new(VfxNodeKind::EulerIntegration, Vec2::new(580.0, 260.0)));

        let output = graph.add_node(VfxNode::new(VfxNodeKind::OutputParticleQuad, Vec2::new(860.0, 20.0)));

        graph.connect(rate, 0, spawn, 0);
        graph.connect(spawn, 0, init, 0);
        graph.connect(init, 0, update, 0);
        graph.connect(update, 0, output, 0);
        graph.connect(set_pos, 0, init, 1);
        graph.connect(set_life, 0, init, 2);
        graph.connect(set_size, 0, init, 3);
        graph.connect(set_color, 0, init, 4);
        graph.connect(gravity_node, 0, update, 1);
        graph.connect(scale_life, 0, update, 2);
        graph.connect(alpha_life, 0, update, 3);
        graph.connect(euler, 0, update, 4);

        graph
    }

    pub fn build_sparks_effect() -> Self {
        let mut graph = Self::new("Sparks");
        graph.particle_capacity = 8192;

        let spawn = graph.add_node(VfxNode::new(VfxNodeKind::SpawnContext, Vec2::new(20.0, 20.0)));
        let burst = graph.add_node(VfxNode::new(VfxNodeKind::BurstSpawn, Vec2::new(20.0, 80.0)));
        if let Some(n) = graph.find_node_mut(burst) {
            n.set_param("Count", VfxValue::Uint(200));
        }

        let init = graph.add_node(VfxNode::new(VfxNodeKind::InitializeContext, Vec2::new(300.0, 20.0)));
        let set_pos = graph.add_node(VfxNode::new(VfxNodeKind::SetPositionShape, Vec2::new(300.0, 80.0)));
        let set_vel = graph.add_node(VfxNode::new(VfxNodeKind::SetVelocityRandom, Vec2::new(300.0, 140.0)));
        let set_life = graph.add_node(VfxNode::new(VfxNodeKind::SetLifetime, Vec2::new(300.0, 200.0)));
        if let Some(n) = graph.find_node_mut(set_life) {
            n.set_param("Lifetime", VfxValue::Float(0.8));
        }
        let set_size = graph.add_node(VfxNode::new(VfxNodeKind::SetSize, Vec2::new(300.0, 260.0)));
        if let Some(n) = graph.find_node_mut(set_size) {
            n.set_param("Size", VfxValue::Float(0.05));
        }
        let set_color = graph.add_node(VfxNode::new(VfxNodeKind::SetColor, Vec2::new(300.0, 320.0)));
        if let Some(n) = graph.find_node_mut(set_color) {
            n.set_param("Color", VfxValue::Color(Vec4::new(1.0, 0.9, 0.3, 1.0)));
        }

        let update = graph.add_node(VfxNode::new(VfxNodeKind::UpdateContext, Vec2::new(580.0, 20.0)));
        let gravity_node = graph.add_node(VfxNode::new(VfxNodeKind::Gravity, Vec2::new(580.0, 80.0)));
        let drag_node = graph.add_node(VfxNode::new(VfxNodeKind::Drag, Vec2::new(580.0, 140.0)));
        if let Some(n) = graph.find_node_mut(drag_node) {
            n.set_param("Drag", VfxValue::Float(2.0));
        }
        let alpha_life = graph.add_node(VfxNode::new(VfxNodeKind::AlphaOverLife, Vec2::new(580.0, 200.0)));
        let euler = graph.add_node(VfxNode::new(VfxNodeKind::EulerIntegration, Vec2::new(580.0, 260.0)));

        let output = graph.add_node(VfxNode::new(VfxNodeKind::OutputParticleStrip, Vec2::new(860.0, 20.0)));

        graph.connect(burst, 0, spawn, 0);
        graph.connect(spawn, 0, init, 0);
        graph.connect(init, 0, update, 0);
        graph.connect(update, 0, output, 0);
        graph.connect(set_pos, 0, init, 1);
        graph.connect(set_vel, 0, init, 2);
        graph.connect(set_life, 0, init, 3);
        graph.connect(set_size, 0, init, 4);
        graph.connect(set_color, 0, init, 5);
        graph.connect(gravity_node, 0, update, 1);
        graph.connect(drag_node, 0, update, 2);
        graph.connect(alpha_life, 0, update, 3);
        graph.connect(euler, 0, update, 4);

        graph
    }
}

// ---------------------------------------------------------------------------
// VFX Graph editor view state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VfxEditorTool {
    Select,
    Connect,
    Comment,
    Pan,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VfxEditorPanel {
    Graph,
    Inspector,
    Preview,
    Blackboard,
    Profiler,
}

#[derive(Debug, Clone)]
pub struct VfxEditorView {
    pub zoom: f32,
    pub pan: Vec2,
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub grid_size: f32,
    pub show_grid: bool,
    pub show_node_minimap: bool,
}

impl Default for VfxEditorView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            min_zoom: 0.1,
            max_zoom: 4.0,
            grid_size: 20.0,
            show_grid: true,
            show_node_minimap: true,
        }
    }
}

impl VfxEditorView {
    pub fn screen_to_graph(&self, screen: Vec2) -> Vec2 {
        (screen - self.pan) / self.zoom
    }

    pub fn graph_to_screen(&self, graph: Vec2) -> Vec2 {
        graph * self.zoom + self.pan
    }

    pub fn zoom_at(&mut self, focus: Vec2, delta: f32) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * (1.0 + delta * 0.1)).clamp(self.min_zoom, self.max_zoom);
        let scale = self.zoom / old_zoom;
        self.pan = focus + (self.pan - focus) * scale;
    }

    pub fn snap_to_grid(&self, pos: Vec2) -> Vec2 {
        let g = self.grid_size;
        Vec2::new((pos.x / g).round() * g, (pos.y / g).round() * g)
    }

    pub fn fit_to_graph(&mut self, nodes: &[VfxNode], viewport_size: Vec2) {
        if nodes.is_empty() { return; }
        let min_x = nodes.iter().map(|n| n.position.x).fold(f32::MAX, f32::min);
        let min_y = nodes.iter().map(|n| n.position.y).fold(f32::MAX, f32::min);
        let max_x = nodes.iter().map(|n| n.position.x + n.size.x).fold(f32::MIN, f32::max);
        let max_y = nodes.iter().map(|n| n.position.y + n.size.y).fold(f32::MIN, f32::max);
        let content_w = max_x - min_x + 80.0;
        let content_h = max_y - min_y + 80.0;
        let scale_x = viewport_size.x / content_w;
        let scale_y = viewport_size.y / content_h;
        self.zoom = scale_x.min(scale_y).clamp(self.min_zoom, self.max_zoom);
        let center = Vec2::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
        self.pan = viewport_size * 0.5 - center * self.zoom;
    }
}

// ---------------------------------------------------------------------------
// Drag state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum VfxDragState {
    None,
    DraggingNode { node_id: u64, offset: Vec2 },
    DraggingConnection { from_node: u64, from_port: usize, is_input: bool, current_pos: Vec2 },
    BoxSelect { start: Vec2, current: Vec2 },
    Panning { last: Vec2 },
}

// ---------------------------------------------------------------------------
// GPU profiler data for VFX systems
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct VfxProfilerSample {
    pub graph_name: String,
    pub spawn_ms: f32,
    pub init_ms: f32,
    pub update_ms: f32,
    pub render_ms: f32,
    pub particle_count: u32,
    pub alive_count: u32,
    pub memory_bytes: usize,
    pub draw_calls: u32,
}

impl VfxProfilerSample {
    pub fn total_ms(&self) -> f32 {
        self.spawn_ms + self.init_ms + self.update_ms + self.render_ms
    }

    pub fn memory_kb(&self) -> f32 {
        self.memory_bytes as f32 / 1024.0
    }
}

#[derive(Debug, Clone)]
pub struct VfxProfiler {
    pub samples: Vec<VfxProfilerSample>,
    pub history_len: usize,
    pub paused: bool,
}

impl Default for VfxProfiler {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            history_len: 128,
            paused: false,
        }
    }
}

impl VfxProfiler {
    pub fn push_sample(&mut self, sample: VfxProfilerSample) {
        if self.paused { return; }
        self.samples.push(sample);
        if self.samples.len() > self.history_len {
            self.samples.remove(0);
        }
    }

    pub fn avg_total_ms(&self) -> f32 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().map(|s| s.total_ms()).sum::<f32>() / self.samples.len() as f32
    }

    pub fn peak_particle_count(&self) -> u32 {
        self.samples.iter().map(|s| s.alive_count).max().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// VFX editor blackboard (shared variables)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VfxBlackboardEntry {
    pub name: String,
    pub value: VfxValue,
    pub description: String,
    pub exposed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct VfxBlackboard {
    pub entries: Vec<VfxBlackboardEntry>,
}

impl VfxBlackboard {
    pub fn add(&mut self, name: &str, value: VfxValue) {
        self.entries.push(VfxBlackboardEntry {
            name: name.to_string(),
            value,
            description: String::new(),
            exposed: false,
        });
    }

    pub fn get(&self, name: &str) -> Option<&VfxValue> {
        self.entries.iter().find(|e| e.name == name).map(|e| &e.value)
    }

    pub fn set(&mut self, name: &str, value: VfxValue) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.name == name) {
            e.value = value;
        } else {
            self.add(name, value);
        }
    }
}

// ---------------------------------------------------------------------------
// VFX instance runtime data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VfxInstance {
    pub graph_name: String,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub playing: bool,
    pub looping: bool,
    pub play_time: f32,
    pub duration: f32,
    pub speed: f32,
    pub prewarm: bool,
    pub seed: u32,
    pub attached_to_entity: Option<u64>,
}

impl VfxInstance {
    pub fn new(graph_name: &str) -> Self {
        Self {
            graph_name: graph_name.to_string(),
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            playing: false,
            looping: true,
            play_time: 0.0,
            duration: 5.0,
            speed: 1.0,
            prewarm: false,
            seed: 12345,
            attached_to_entity: None,
        }
    }

    pub fn play(&mut self) { self.playing = true; }
    pub fn stop(&mut self) { self.playing = false; self.play_time = 0.0; }
    pub fn pause(&mut self) { self.playing = false; }

    pub fn update(&mut self, dt: f32) {
        if !self.playing { return; }
        self.play_time += dt * self.speed;
        if self.looping && self.play_time >= self.duration {
            self.play_time -= self.duration;
        } else if self.play_time >= self.duration {
            self.playing = false;
            self.play_time = self.duration;
        }
    }

    pub fn normalized_time(&self) -> f32 {
        if self.duration > 0.0 { (self.play_time / self.duration).clamp(0.0, 1.0) } else { 0.0 }
    }

    pub fn transform_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

// ---------------------------------------------------------------------------
// Main VFX Graph Editor
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct VfxGraphEditor {
    pub graphs: Vec<VfxGraph>,
    pub active_graph: usize,
    pub view: VfxEditorView,
    pub tool: VfxEditorTool,
    pub active_panel: VfxEditorPanel,
    pub selected_nodes: Vec<u64>,
    pub clipboard: Vec<VfxNode>,
    pub drag_state: VfxDragState,
    pub hovered_node: Option<u64>,
    pub search_query: String,
    pub show_block_library: bool,
    pub blackboard: VfxBlackboard,
    pub profiler: VfxProfiler,
    pub instances: Vec<VfxInstance>,
    pub preview_playing: bool,
    pub preview_time: f32,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    pub history: Vec<Vec<VfxNode>>,
    pub history_pos: usize,
    pub category_filter: Option<String>,
    pub show_warnings: bool,
}

impl Default for VfxGraphEditor {
    fn default() -> Self {
        let fire = VfxGraph::build_fire_effect();
        let smoke = VfxGraph::build_smoke_effect();
        let sparks = VfxGraph::build_sparks_effect();

        let mut blackboard = VfxBlackboard::default();
        blackboard.add("DeltaTime", VfxValue::Float(0.016));
        blackboard.add("ElapsedTime", VfxValue::Float(0.0));
        blackboard.add("Gravity", VfxValue::Float3(Vec3::new(0.0, -9.81, 0.0)));

        let instances = vec![
            VfxInstance::new("Fire"),
            VfxInstance::new("Smoke"),
            VfxInstance::new("Sparks"),
        ];

        Self {
            graphs: vec![fire, smoke, sparks],
            active_graph: 0,
            view: VfxEditorView::default(),
            tool: VfxEditorTool::Select,
            active_panel: VfxEditorPanel::Graph,
            selected_nodes: Vec::new(),
            clipboard: Vec::new(),
            drag_state: VfxDragState::None,
            hovered_node: None,
            search_query: String::new(),
            show_block_library: false,
            blackboard,
            profiler: VfxProfiler::default(),
            instances,
            preview_playing: false,
            preview_time: 0.0,
            show_grid: true,
            snap_to_grid: true,
            history: Vec::new(),
            history_pos: 0,
            category_filter: None,
            show_warnings: true,
        }
    }
}

impl VfxGraphEditor {
    pub fn active_graph(&self) -> &VfxGraph {
        &self.graphs[self.active_graph]
    }

    pub fn active_graph_mut(&mut self) -> &mut VfxGraph {
        &mut self.graphs[self.active_graph]
    }

    pub fn snapshot(&mut self) {
        let nodes = self.active_graph().nodes.clone();
        self.history.truncate(self.history_pos);
        self.history.push(nodes);
        self.history_pos = self.history.len();
    }

    pub fn undo(&mut self) {
        if self.history_pos > 1 {
            self.history_pos -= 1;
            let nodes = self.history[self.history_pos - 1].clone();
            self.active_graph_mut().nodes = nodes;
        }
    }

    pub fn redo(&mut self) {
        if self.history_pos < self.history.len() {
            let nodes = self.history[self.history_pos].clone();
            self.active_graph_mut().nodes = nodes;
            self.history_pos += 1;
        }
    }

    pub fn select_node(&mut self, id: u64) {
        self.selected_nodes = vec![id];
    }

    pub fn add_to_selection(&mut self, id: u64) {
        if !self.selected_nodes.contains(&id) {
            self.selected_nodes.push(id);
        }
    }

    pub fn deselect_all(&mut self) {
        self.selected_nodes.clear();
    }

    pub fn delete_selected(&mut self) {
        self.snapshot();
        let ids = self.selected_nodes.clone();
        for id in ids {
            self.active_graph_mut().remove_node(id);
        }
        self.selected_nodes.clear();
    }

    pub fn copy_selected(&mut self) {
        let ids: std::collections::HashSet<u64> = self.selected_nodes.iter().copied().collect();
        self.clipboard = self.active_graph().nodes.iter()
            .filter(|n| ids.contains(&n.id))
            .cloned()
            .collect();
    }

    pub fn paste(&mut self) {
        self.snapshot();
        let nodes_to_paste: Vec<VfxNode> = self.clipboard.clone();
        let graph = self.active_graph_mut();
        let mut new_ids = Vec::new();
        for mut node in nodes_to_paste {
            node.id = next_vfx_node_id();
            node.position += Vec2::new(20.0, 20.0);
            let id = node.id;
            graph.nodes.push(node);
            new_ids.push(id);
        }
        self.selected_nodes = new_ids;
    }

    pub fn duplicate_selected(&mut self) {
        self.copy_selected();
        self.paste();
    }

    pub fn add_node_at(&mut self, kind: VfxNodeKind, pos: Vec2) -> u64 {
        self.snapshot();
        let graph_pos = if self.snap_to_grid {
            self.view.snap_to_grid(pos)
        } else {
            pos
        };
        let node = VfxNode::new(kind, graph_pos);
        self.active_graph_mut().add_node(node)
    }

    pub fn hit_test(&self, screen_pos: Vec2) -> Option<u64> {
        let graph_pos = self.view.screen_to_graph(screen_pos);
        // Iterate in reverse to test top-most nodes first
        for node in self.active_graph().nodes.iter().rev() {
            if node.contains_point(graph_pos) {
                return Some(node.id);
            }
        }
        None
    }

    pub fn begin_drag_node(&mut self, node_id: u64, screen_pos: Vec2) {
        let graph_pos = self.view.screen_to_graph(screen_pos);
        if let Some(node) = self.active_graph().find_node(node_id) {
            let offset = node.position - graph_pos;
            self.drag_state = VfxDragState::DraggingNode { node_id, offset };
        }
    }

    pub fn update_drag(&mut self, screen_pos: Vec2) {
        let graph_pos = self.view.screen_to_graph(screen_pos);
        match self.drag_state.clone() {
            VfxDragState::DraggingNode { node_id, offset } => {
                let new_pos = if self.snap_to_grid {
                    self.view.snap_to_grid(graph_pos + offset)
                } else {
                    graph_pos + offset
                };
                if let Some(node) = self.active_graph_mut().find_node_mut(node_id) {
                    node.position = new_pos;
                }
            }
            VfxDragState::Panning { last } => {
                self.view.pan += screen_pos - last;
                self.drag_state = VfxDragState::Panning { last: screen_pos };
            }
            VfxDragState::BoxSelect { start, .. } => {
                self.drag_state = VfxDragState::BoxSelect { start, current: graph_pos };
            }
            VfxDragState::DraggingConnection { from_node, from_port, is_input, .. } => {
                self.drag_state = VfxDragState::DraggingConnection {
                    from_node, from_port, is_input, current_pos: screen_pos,
                };
            }
            _ => {}
        }
    }

    pub fn end_drag(&mut self) {
        if let VfxDragState::BoxSelect { start, current } = &self.drag_state.clone() {
            let min = Vec2::new(start.x.min(current.x), start.y.min(current.y));
            let max = Vec2::new(start.x.max(current.x), start.y.max(current.y));
            let newly_selected: Vec<u64> = self.active_graph().nodes.iter()
                .filter(|n| {
                    let cx = n.position.x + n.size.x * 0.5;
                    let cy = n.position.y + n.size.y * 0.5;
                    cx >= min.x && cx <= max.x && cy >= min.y && cy <= max.y
                })
                .map(|n| n.id)
                .collect();
            self.selected_nodes = newly_selected;
        }
        self.drag_state = VfxDragState::None;
    }

    pub fn zoom(&mut self, screen_focus: Vec2, delta: f32) {
        self.view.zoom_at(screen_focus, delta);
    }

    pub fn frame_all(&mut self, viewport: Vec2) {
        let nodes = self.active_graph().nodes.clone();
        self.view.fit_to_graph(&nodes, viewport);
    }

    pub fn search_blocks(&self, query: &str) -> Vec<VfxNodeKind> {
        let q = query.to_lowercase();
        let all: Vec<VfxNodeKind> = vec![
            VfxNodeKind::ConstantRate, VfxNodeKind::BurstSpawn, VfxNodeKind::SetLifetime,
            VfxNodeKind::SetSize, VfxNodeKind::SetColor, VfxNodeKind::SetVelocityRandom,
            VfxNodeKind::Gravity, VfxNodeKind::Drag, VfxNodeKind::Turbulence,
            VfxNodeKind::EulerIntegration, VfxNodeKind::ColorOverLife, VfxNodeKind::AlphaOverLife,
            VfxNodeKind::ScaleOverLife, VfxNodeKind::Add, VfxNodeKind::Multiply,
            VfxNodeKind::Lerp, VfxNodeKind::Clamp, VfxNodeKind::Noise3D, VfxNodeKind::CurlNoise,
            VfxNodeKind::GetBuiltin, VfxNodeKind::SetAttribute, VfxNodeKind::Branch,
            VfxNodeKind::SampleGradient, VfxNodeKind::SampleCurve, VfxNodeKind::CustomHlsl,
        ];
        if q.is_empty() {
            all
        } else {
            all.into_iter().filter(|k| {
                k.label().to_lowercase().contains(&q) || k.category().to_lowercase().contains(&q)
            }).collect()
        }
    }

    pub fn update_preview(&mut self, dt: f32) {
        if self.preview_playing {
            self.preview_time += dt;
            for inst in &mut self.instances {
                inst.update(dt);
            }
            // Simulate profiler data
            let sample = VfxProfilerSample {
                graph_name: self.active_graph().name.clone(),
                spawn_ms: 0.05 + (self.preview_time.sin() * 0.01).abs(),
                init_ms: 0.02,
                update_ms: 0.4 + (self.preview_time.cos() * 0.05).abs(),
                render_ms: 0.3,
                particle_count: self.active_graph().particle_capacity,
                alive_count: (self.active_graph().particle_capacity as f32 * 0.7) as u32,
                memory_bytes: self.active_graph().estimate_memory_bytes(),
                draw_calls: 2,
            };
            self.profiler.push_sample(sample);
        }
    }

    pub fn validate_active(&mut self) -> Vec<String> {
        self.active_graph_mut().validate()
    }

    pub fn generate_hlsl_for_active(&self) -> String {
        self.active_graph().generate_hlsl()
    }
}
