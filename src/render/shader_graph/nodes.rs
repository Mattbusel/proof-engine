//! Shader graph node types and socket definitions.
//!
//! Each `NodeType` maps to a GLSL expression snippet. The `GraphCompiler`
//! collects these snippets and assembles them into a complete shader.

use super::NodeId;
use std::collections::HashMap;

// ── SocketType ────────────────────────────────────────────────────────────────

/// GLSL data type flowing through a socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Int,
    Bool,
    Sampler2D,
    /// Any scalar or vector — resolved at compile time.
    Any,
}

impl SocketType {
    pub fn glsl_type(self) -> &'static str {
        match self {
            SocketType::Float     => "float",
            SocketType::Vec2      => "vec2",
            SocketType::Vec3      => "vec3",
            SocketType::Vec4      => "vec4",
            SocketType::Int       => "int",
            SocketType::Bool      => "bool",
            SocketType::Sampler2D => "sampler2D",
            SocketType::Any       => "float",
        }
    }

    pub fn default_value(self) -> &'static str {
        match self {
            SocketType::Float     => "0.0",
            SocketType::Vec2      => "vec2(0.0)",
            SocketType::Vec3      => "vec3(0.0)",
            SocketType::Vec4      => "vec4(0.0, 0.0, 0.0, 1.0)",
            SocketType::Int       => "0",
            SocketType::Bool      => "false",
            SocketType::Sampler2D => "/* sampler */",
            SocketType::Any       => "0.0",
        }
    }

    pub fn is_compatible_with(self, other: SocketType) -> bool {
        if self == other { return true; }
        if self == SocketType::Any || other == SocketType::Any { return true; }
        // Vec3 ↔ Vec4 (auto-swizzle)
        matches!((self, other),
            (SocketType::Vec3, SocketType::Vec4) | (SocketType::Vec4, SocketType::Vec3) |
            (SocketType::Float, SocketType::Vec2) | (SocketType::Vec2, SocketType::Float)
        )
    }
}

// ── NodeSocket ────────────────────────────────────────────────────────────────

/// Definition of one input or output socket on a node.
#[derive(Debug, Clone)]
pub struct NodeSocket {
    pub name:     String,
    pub socket_type: SocketType,
    pub required: bool,
    /// Default value string (used when disconnected and no constant set).
    pub default:  String,
}

impl NodeSocket {
    pub fn required(name: &str, t: SocketType) -> Self {
        Self { name: name.to_string(), socket_type: t, required: true, default: t.default_value().to_string() }
    }
    pub fn optional(name: &str, t: SocketType, default: &str) -> Self {
        Self { name: name.to_string(), socket_type: t, required: false, default: default.to_string() }
    }
}

// ── NodeType ──────────────────────────────────────────────────────────────────

/// All supported shader node types.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    // ── Inputs ────────────────────────────────────────────────────────────────
    /// UV texture coordinates (vec2).
    UvCoord,
    /// World-space position of the fragment (vec3).
    WorldPos,
    /// Camera position (vec3).
    CameraPos,
    /// Scene time (float).
    Time,
    /// Screen resolution (vec2).
    Resolution,
    /// Constant float.
    ConstFloat(f32),
    /// Constant vec2.
    ConstVec2(f32, f32),
    /// Constant vec3 (color or vector).
    ConstVec3(f32, f32, f32),
    /// Constant vec4.
    ConstVec4(f32, f32, f32, f32),
    /// A named uniform parameter.
    Uniform(String, SocketType),
    /// Sample a texture.
    TextureSample,
    /// Vertex color passed from vertex shader.
    VertexColor,
    /// Fragment screen-space coordinates (vec2).
    ScreenCoord,

    // ── Math ──────────────────────────────────────────────────────────────────
    /// A + B
    Add,
    /// A - B
    Subtract,
    /// A * B
    Multiply,
    /// A / B (safe: returns 0 when B=0)
    Divide,
    /// Power: A^B
    Power,
    /// sqrt(A)
    Sqrt,
    /// abs(A)
    Abs,
    /// sign(A)
    Sign,
    /// floor(A)
    Floor,
    /// ceil(A)
    Ceil,
    /// fract(A)
    Fract,
    /// min(A, B)
    Min,
    /// max(A, B)
    Max,
    /// clamp(A, min, max)
    Clamp,
    /// mix(A, B, T)
    Mix,
    /// smoothstep(edge0, edge1, x)
    Smoothstep,
    /// step(edge, x)
    Step,
    /// dot(A, B)
    Dot,
    /// cross(A, B)
    Cross,
    /// normalize(A)
    Normalize,
    /// length(A)
    Length,
    /// distance(A, B)
    Distance,
    /// reflect(I, N)
    Reflect,
    /// refract(I, N, eta)
    Refract,
    /// mod(A, B)
    Mod,
    /// sin(A)
    Sin,
    /// cos(A)
    Cos,
    /// tan(A)
    Tan,
    /// atan(A) or atan(Y, X)
    Atan,
    /// exp(A)
    Exp,
    /// log(A)
    Log,
    /// log2(A)
    Log2,
    /// Remap: rescales A from [in_min,in_max] to [out_min,out_max]
    Remap,
    /// 1.0 - A
    OneMinus,
    /// Saturate: clamp to [0,1]
    Saturate,
    /// Negate: -A
    Negate,
    /// Reciprocal: 1.0 / A
    Reciprocal,

    // ── Vector ────────────────────────────────────────────────────────────────
    /// Combine (float, float) → vec2
    CombineVec2,
    /// Combine (float, float, float) → vec3
    CombineVec3,
    /// Combine (vec3, float) → vec4
    CombineVec4,
    /// Split vec2 → (x, y)
    SplitVec2,
    /// Split vec3 → (x, y, z)
    SplitVec3,
    /// Split vec4 → (x, y, z, w)
    SplitVec4,
    /// Swizzle: extract named components (e.g. ".xyz", ".yyy")
    Swizzle(String),
    /// vec3 length squared
    LengthSquared,
    /// Rotate a 2D vector by angle (radians)
    RotateVec2,

    // ── Color ─────────────────────────────────────────────────────────────────
    /// HSV → RGB conversion
    HsvToRgb,
    /// RGB → HSV conversion
    RgbToHsv,
    /// Luminance (grayscale value)
    Luminance,
    /// Saturation adjustment
    Saturation,
    /// Hue rotation
    HueRotate,
    /// Color burn blend
    ColorBurn,
    /// Color dodge blend
    ColorDodge,
    /// Screen blend: 1 - (1-A)(1-B)
    ScreenBlend,
    /// Overlay blend
    OverlayBlend,
    /// Hard light blend
    HardLight,
    /// Soft light blend
    SoftLight,
    /// Difference blend
    Difference,
    /// Gamma correction
    GammaCorrect,
    /// Linear to sRGB
    LinearToSrgb,
    /// sRGB to linear
    SrgbToLinear,

    // ── Noise and Patterns ────────────────────────────────────────────────────
    /// Value noise
    ValueNoise,
    /// Gradient (Perlin) noise
    PerlinNoise,
    /// Simplex noise
    SimplexNoise,
    /// Fractal Brownian Motion (fBm)
    Fbm,
    /// Voronoi / cellular noise
    Voronoi,
    /// Worley noise (F1, F2, F1-F2)
    Worley,
    /// Checkerboard pattern
    Checkerboard,
    /// Polka dots pattern
    PolkaDots,
    /// Sine wave pattern
    SineWave,
    /// Square wave pattern
    SquareWave,
    /// Triangle wave pattern
    TriangleWave,
    /// Sawtooth wave pattern
    SawtoothWave,
    /// Grid pattern
    Grid,
    /// Radial gradient from center
    RadialGradient,
    /// Linear gradient along an axis
    LinearGradient,
    /// Spiral pattern
    Spiral,
    /// Concentric rings
    Rings,
    /// Star burst pattern
    StarBurst,
    /// Hexagonal tiling
    HexTile,

    // ── Effects ───────────────────────────────────────────────────────────────
    /// Chromatic aberration (RGB split)
    ChromaticAberration,
    /// Screen-space edge detection
    EdgeDetect,
    /// Pixelation
    Pixelate,
    /// Barrel distortion
    BarrelDistort,
    /// Fish-eye distortion
    FishEye,
    /// Vignette darkening
    Vignette,
    /// Film grain
    FilmGrain,
    /// CRT scanlines
    Scanlines,
    /// Heat haze / refraction
    HeatHaze,
    /// Glitch offset
    GlitchOffset,
    /// Screen shake (UV offset)
    ScreenShake,
    /// Blur (box blur via sampling)
    BoxBlur,
    /// Sharpen filter
    Sharpen,
    /// Emboss filter
    Emboss,
    /// Invert colors
    Invert,
    /// Posterize
    Posterize,
    /// Duotone (shadows one color, highlights another)
    Duotone,
    /// Outline (find edge and colorize)
    Outline,

    // ── SDF (Signed Distance Fields) ─────────────────────────────────────────
    /// SDF Circle
    SdfCircle,
    /// SDF Box
    SdfBox,
    /// SDF Line segment
    SdfLine,
    /// SDF Triangle
    SdfTriangle,
    /// SDF Ring/Annulus
    SdfRing,
    /// SDF Star
    SdfStar,
    /// SDF smooth union
    SdfSmoothUnion,
    /// SDF smooth subtraction
    SdfSmoothSubtract,
    /// SDF smooth intersection
    SdfSmoothIntersect,
    /// SDF → alpha (step at edge)
    SdfToAlpha,
    /// SDF → soft alpha (smoothstep at edge)
    SdfToSoftAlpha,

    // ── Attractor / Math-Driven ───────────────────────────────────────────────
    /// Evaluate a Lorenz attractor at UV position
    LorenzAttractor,
    /// Mandelbrot set iteration count
    Mandelbrot,
    /// Julia set
    Julia,
    /// Burning Ship fractal
    BurningShip,
    /// Newton fractal
    NewtonFractal,
    /// Lyapunov exponent visualization
    LyapunovViz,

    // ── Logic / Conditional ────────────────────────────────────────────────────
    /// if A > threshold, output B else C
    IfGreater,
    /// if A < threshold, output B else C
    IfLess,
    /// Conditional blend (threshold with smooth transition)
    ConditionalBlend,
    /// Boolean AND
    BoolAnd,
    /// Boolean OR
    BoolOr,
    /// Boolean NOT
    BoolNot,

    // ── Output ────────────────────────────────────────────────────────────────
    /// Final fragment color output (must be vec4).
    OutputColor,
    /// Secondary output to a named render target.
    OutputTarget(String),
    /// Output to bloom buffer simultaneously.
    OutputWithBloom,
}

impl NodeType {
    pub fn label(&self) -> &str {
        match self {
            NodeType::UvCoord            => "UV Coord",
            NodeType::WorldPos           => "World Pos",
            NodeType::CameraPos          => "Camera Pos",
            NodeType::Time               => "Time",
            NodeType::Resolution         => "Resolution",
            NodeType::ConstFloat(_)      => "Float",
            NodeType::ConstVec2(_, _)    => "Vec2",
            NodeType::ConstVec3(..)      => "Vec3",
            NodeType::ConstVec4(..)      => "Vec4",
            NodeType::Uniform(n, _)      => n.as_str(),
            NodeType::TextureSample      => "Texture Sample",
            NodeType::VertexColor        => "Vertex Color",
            NodeType::ScreenCoord        => "Screen Coord",
            NodeType::Add                => "Add",
            NodeType::Subtract           => "Subtract",
            NodeType::Multiply           => "Multiply",
            NodeType::Divide             => "Divide",
            NodeType::Power              => "Power",
            NodeType::Sqrt               => "Sqrt",
            NodeType::Abs                => "Abs",
            NodeType::Sign               => "Sign",
            NodeType::Floor              => "Floor",
            NodeType::Ceil               => "Ceil",
            NodeType::Fract              => "Fract",
            NodeType::Min                => "Min",
            NodeType::Max                => "Max",
            NodeType::Clamp              => "Clamp",
            NodeType::Mix                => "Mix",
            NodeType::Smoothstep         => "Smoothstep",
            NodeType::Step               => "Step",
            NodeType::Dot                => "Dot",
            NodeType::Cross              => "Cross",
            NodeType::Normalize          => "Normalize",
            NodeType::Length             => "Length",
            NodeType::Distance           => "Distance",
            NodeType::Reflect            => "Reflect",
            NodeType::Refract            => "Refract",
            NodeType::Mod                => "Mod",
            NodeType::Sin                => "Sin",
            NodeType::Cos                => "Cos",
            NodeType::Tan                => "Tan",
            NodeType::Atan               => "Atan",
            NodeType::Exp                => "Exp",
            NodeType::Log                => "Log",
            NodeType::Log2               => "Log2",
            NodeType::Remap              => "Remap",
            NodeType::OneMinus           => "One Minus",
            NodeType::Saturate           => "Saturate",
            NodeType::Negate             => "Negate",
            NodeType::Reciprocal         => "Reciprocal",
            NodeType::CombineVec2        => "Combine Vec2",
            NodeType::CombineVec3        => "Combine Vec3",
            NodeType::CombineVec4        => "Combine Vec4",
            NodeType::SplitVec2          => "Split Vec2",
            NodeType::SplitVec3          => "Split Vec3",
            NodeType::SplitVec4          => "Split Vec4",
            NodeType::Swizzle(s)         => s.as_str(),
            NodeType::LengthSquared      => "Length²",
            NodeType::RotateVec2         => "Rotate Vec2",
            NodeType::HsvToRgb           => "HSV → RGB",
            NodeType::RgbToHsv           => "RGB → HSV",
            NodeType::Luminance          => "Luminance",
            NodeType::Saturation         => "Saturation",
            NodeType::HueRotate          => "Hue Rotate",
            NodeType::ColorBurn          => "Color Burn",
            NodeType::ColorDodge         => "Color Dodge",
            NodeType::ScreenBlend        => "Screen",
            NodeType::OverlayBlend       => "Overlay",
            NodeType::HardLight          => "Hard Light",
            NodeType::SoftLight          => "Soft Light",
            NodeType::Difference         => "Difference",
            NodeType::GammaCorrect       => "Gamma",
            NodeType::LinearToSrgb       => "Linear→sRGB",
            NodeType::SrgbToLinear       => "sRGB→Linear",
            NodeType::ValueNoise         => "Value Noise",
            NodeType::PerlinNoise        => "Perlin Noise",
            NodeType::SimplexNoise       => "Simplex Noise",
            NodeType::Fbm                => "fBm",
            NodeType::Voronoi            => "Voronoi",
            NodeType::Worley             => "Worley",
            NodeType::Checkerboard       => "Checkerboard",
            NodeType::PolkaDots          => "Polka Dots",
            NodeType::SineWave           => "Sine Wave",
            NodeType::SquareWave         => "Square Wave",
            NodeType::TriangleWave       => "Triangle Wave",
            NodeType::SawtoothWave       => "Sawtooth Wave",
            NodeType::Grid               => "Grid",
            NodeType::RadialGradient     => "Radial Gradient",
            NodeType::LinearGradient     => "Linear Gradient",
            NodeType::Spiral             => "Spiral",
            NodeType::Rings              => "Rings",
            NodeType::StarBurst          => "Star Burst",
            NodeType::HexTile            => "Hex Tile",
            NodeType::ChromaticAberration => "Chromatic Ab.",
            NodeType::EdgeDetect         => "Edge Detect",
            NodeType::Pixelate           => "Pixelate",
            NodeType::BarrelDistort      => "Barrel Distort",
            NodeType::FishEye            => "Fish Eye",
            NodeType::Vignette           => "Vignette",
            NodeType::FilmGrain          => "Film Grain",
            NodeType::Scanlines          => "Scanlines",
            NodeType::HeatHaze           => "Heat Haze",
            NodeType::GlitchOffset       => "Glitch",
            NodeType::ScreenShake        => "Screen Shake",
            NodeType::BoxBlur            => "Box Blur",
            NodeType::Sharpen            => "Sharpen",
            NodeType::Emboss             => "Emboss",
            NodeType::Invert             => "Invert",
            NodeType::Posterize          => "Posterize",
            NodeType::Duotone            => "Duotone",
            NodeType::Outline            => "Outline",
            NodeType::SdfCircle          => "SDF Circle",
            NodeType::SdfBox             => "SDF Box",
            NodeType::SdfLine            => "SDF Line",
            NodeType::SdfTriangle        => "SDF Triangle",
            NodeType::SdfRing            => "SDF Ring",
            NodeType::SdfStar            => "SDF Star",
            NodeType::SdfSmoothUnion     => "SDF Union",
            NodeType::SdfSmoothSubtract  => "SDF Subtract",
            NodeType::SdfSmoothIntersect => "SDF Intersect",
            NodeType::SdfToAlpha         => "SDF Alpha",
            NodeType::SdfToSoftAlpha     => "SDF Soft Alpha",
            NodeType::LorenzAttractor    => "Lorenz",
            NodeType::Mandelbrot         => "Mandelbrot",
            NodeType::Julia              => "Julia",
            NodeType::BurningShip        => "Burning Ship",
            NodeType::NewtonFractal      => "Newton",
            NodeType::LyapunovViz        => "Lyapunov",
            NodeType::IfGreater          => "If Greater",
            NodeType::IfLess             => "If Less",
            NodeType::ConditionalBlend   => "Cond. Blend",
            NodeType::BoolAnd            => "AND",
            NodeType::BoolOr             => "OR",
            NodeType::BoolNot            => "NOT",
            NodeType::OutputColor        => "Output Color",
            NodeType::OutputTarget(n)    => n.as_str(),
            NodeType::OutputWithBloom    => "Output+Bloom",
        }
    }

    /// Input socket definitions.
    pub fn input_sockets(&self) -> Vec<NodeSocket> {
        match self {
            NodeType::Add | NodeType::Subtract | NodeType::Multiply |
            NodeType::Divide | NodeType::Power | NodeType::Mod |
            NodeType::Min | NodeType::Max | NodeType::Dot | NodeType::Distance => vec![
                NodeSocket::required("A", SocketType::Any),
                NodeSocket::required("B", SocketType::Any),
            ],
            NodeType::Mix => vec![
                NodeSocket::required("A",  SocketType::Any),
                NodeSocket::required("B",  SocketType::Any),
                NodeSocket::required("T",  SocketType::Float),
            ],
            NodeType::Smoothstep => vec![
                NodeSocket::optional("Edge0", SocketType::Float, "0.0"),
                NodeSocket::optional("Edge1", SocketType::Float, "1.0"),
                NodeSocket::required("X",     SocketType::Any),
            ],
            NodeType::Step => vec![
                NodeSocket::optional("Edge", SocketType::Float, "0.5"),
                NodeSocket::required("X",    SocketType::Any),
            ],
            NodeType::Clamp => vec![
                NodeSocket::required("X",   SocketType::Any),
                NodeSocket::optional("Min", SocketType::Float, "0.0"),
                NodeSocket::optional("Max", SocketType::Float, "1.0"),
            ],
            NodeType::Remap => vec![
                NodeSocket::required("X",      SocketType::Any),
                NodeSocket::optional("InMin",  SocketType::Float, "0.0"),
                NodeSocket::optional("InMax",  SocketType::Float, "1.0"),
                NodeSocket::optional("OutMin", SocketType::Float, "0.0"),
                NodeSocket::optional("OutMax", SocketType::Float, "1.0"),
            ],
            NodeType::Sqrt | NodeType::Abs | NodeType::Sign |
            NodeType::Floor | NodeType::Ceil | NodeType::Fract |
            NodeType::Normalize | NodeType::Length | NodeType::LengthSquared |
            NodeType::OneMinus | NodeType::Saturate | NodeType::Negate |
            NodeType::Reciprocal | NodeType::Exp | NodeType::Log |
            NodeType::Log2 | NodeType::Sin | NodeType::Cos |
            NodeType::Tan | NodeType::Atan | NodeType::HsvToRgb |
            NodeType::RgbToHsv | NodeType::Luminance | NodeType::Invert |
            NodeType::LinearToSrgb | NodeType::SrgbToLinear |
            NodeType::BoolNot => vec![
                NodeSocket::required("In", SocketType::Any),
            ],
            NodeType::Reflect | NodeType::Cross | NodeType::BoolAnd | NodeType::BoolOr => vec![
                NodeSocket::required("A", SocketType::Any),
                NodeSocket::required("B", SocketType::Any),
            ],
            NodeType::Refract => vec![
                NodeSocket::required("I",   SocketType::Vec3),
                NodeSocket::required("N",   SocketType::Vec3),
                NodeSocket::optional("Eta", SocketType::Float, "1.5"),
            ],
            NodeType::CombineVec2 => vec![
                NodeSocket::required("X", SocketType::Float),
                NodeSocket::required("Y", SocketType::Float),
            ],
            NodeType::CombineVec3 => vec![
                NodeSocket::required("X", SocketType::Float),
                NodeSocket::required("Y", SocketType::Float),
                NodeSocket::required("Z", SocketType::Float),
            ],
            NodeType::CombineVec4 => vec![
                NodeSocket::required("RGB", SocketType::Vec3),
                NodeSocket::required("A",   SocketType::Float),
            ],
            NodeType::SplitVec2 | NodeType::SplitVec3 | NodeType::SplitVec4 |
            NodeType::Swizzle(_) => vec![
                NodeSocket::required("In", SocketType::Any),
            ],
            NodeType::RotateVec2 => vec![
                NodeSocket::required("UV",    SocketType::Vec2),
                NodeSocket::optional("Angle", SocketType::Float, "0.0"),
                NodeSocket::optional("Center",SocketType::Vec2, "vec2(0.5)"),
            ],
            NodeType::Saturation => vec![
                NodeSocket::required("Color", SocketType::Vec3),
                NodeSocket::optional("Sat",   SocketType::Float, "1.0"),
            ],
            NodeType::HueRotate => vec![
                NodeSocket::required("Color",   SocketType::Vec3),
                NodeSocket::optional("Degrees", SocketType::Float, "0.0"),
            ],
            NodeType::GammaCorrect => vec![
                NodeSocket::required("Color", SocketType::Vec3),
                NodeSocket::optional("Gamma", SocketType::Float, "2.2"),
            ],
            NodeType::ColorBurn | NodeType::ColorDodge | NodeType::ScreenBlend |
            NodeType::OverlayBlend | NodeType::HardLight | NodeType::SoftLight |
            NodeType::Difference => vec![
                NodeSocket::required("A", SocketType::Vec3),
                NodeSocket::required("B", SocketType::Vec3),
            ],
            NodeType::ValueNoise | NodeType::PerlinNoise | NodeType::SimplexNoise => vec![
                NodeSocket::required("UV",    SocketType::Vec2),
                NodeSocket::optional("Scale", SocketType::Float, "1.0"),
                NodeSocket::optional("Seed",  SocketType::Float, "0.0"),
            ],
            NodeType::Fbm => vec![
                NodeSocket::required("UV",       SocketType::Vec2),
                NodeSocket::optional("Octaves",  SocketType::Float, "4.0"),
                NodeSocket::optional("Lacunarity",SocketType::Float,"2.0"),
                NodeSocket::optional("Gain",     SocketType::Float, "0.5"),
            ],
            NodeType::Voronoi | NodeType::Worley => vec![
                NodeSocket::required("UV",    SocketType::Vec2),
                NodeSocket::optional("Scale", SocketType::Float, "1.0"),
                NodeSocket::optional("Jitter",SocketType::Float, "1.0"),
            ],
            NodeType::Checkerboard | NodeType::PolkaDots | NodeType::Grid => vec![
                NodeSocket::required("UV",    SocketType::Vec2),
                NodeSocket::optional("Scale", SocketType::Float, "10.0"),
            ],
            NodeType::SineWave | NodeType::SquareWave | NodeType::TriangleWave |
            NodeType::SawtoothWave => vec![
                NodeSocket::required("UV",        SocketType::Any),
                NodeSocket::optional("Frequency", SocketType::Float, "1.0"),
                NodeSocket::optional("Amplitude", SocketType::Float, "1.0"),
                NodeSocket::optional("Phase",     SocketType::Float, "0.0"),
            ],
            NodeType::RadialGradient => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Center", SocketType::Vec2, "vec2(0.5)"),
                NodeSocket::optional("Radius", SocketType::Float, "0.5"),
            ],
            NodeType::LinearGradient => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Angle",  SocketType::Float, "0.0"),
            ],
            NodeType::Spiral => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Arms",   SocketType::Float, "3.0"),
                NodeSocket::optional("Speed",  SocketType::Float, "1.0"),
                NodeSocket::optional("Time",   SocketType::Float, "0.0"),
            ],
            NodeType::Rings => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Count",  SocketType::Float, "5.0"),
                NodeSocket::optional("Width",  SocketType::Float, "0.5"),
            ],
            NodeType::StarBurst => vec![
                NodeSocket::required("UV",    SocketType::Vec2),
                NodeSocket::optional("Arms",  SocketType::Float, "8.0"),
                NodeSocket::optional("Sharp", SocketType::Float, "0.5"),
            ],
            NodeType::HexTile => vec![
                NodeSocket::required("UV",    SocketType::Vec2),
                NodeSocket::optional("Scale", SocketType::Float, "10.0"),
            ],
            NodeType::Vignette => vec![
                NodeSocket::required("UV",       SocketType::Vec2),
                NodeSocket::optional("Strength", SocketType::Float, "0.5"),
                NodeSocket::optional("Feather",  SocketType::Float, "0.5"),
            ],
            NodeType::FilmGrain => vec![
                NodeSocket::required("UV",       SocketType::Vec2),
                NodeSocket::optional("Time",     SocketType::Float, "0.0"),
                NodeSocket::optional("Strength", SocketType::Float, "0.05"),
            ],
            NodeType::Scanlines => vec![
                NodeSocket::required("UV",        SocketType::Vec2),
                NodeSocket::optional("Intensity", SocketType::Float, "0.2"),
                NodeSocket::optional("Count",     SocketType::Float, "300.0"),
            ],
            NodeType::ChromaticAberration => vec![
                NodeSocket::required("UV",       SocketType::Vec2),
                NodeSocket::optional("Strength", SocketType::Float, "0.005"),
            ],
            NodeType::EdgeDetect | NodeType::Sharpen | NodeType::Emboss => vec![
                NodeSocket::required("Tex",       SocketType::Sampler2D),
                NodeSocket::required("UV",        SocketType::Vec2),
                NodeSocket::optional("Strength",  SocketType::Float, "1.0"),
                NodeSocket::optional("TexelSize", SocketType::Vec2, "vec2(0.001)"),
            ],
            NodeType::Pixelate => vec![
                NodeSocket::required("UV",        SocketType::Vec2),
                NodeSocket::optional("Resolution",SocketType::Float,"64.0"),
            ],
            NodeType::BarrelDistort | NodeType::FishEye => vec![
                NodeSocket::required("UV",        SocketType::Vec2),
                NodeSocket::optional("Strength",  SocketType::Float, "0.3"),
            ],
            NodeType::HeatHaze => vec![
                NodeSocket::required("UV",        SocketType::Vec2),
                NodeSocket::optional("Time",      SocketType::Float, "0.0"),
                NodeSocket::optional("Strength",  SocketType::Float, "0.02"),
                NodeSocket::optional("Speed",     SocketType::Float, "1.0"),
            ],
            NodeType::GlitchOffset => vec![
                NodeSocket::required("UV",        SocketType::Vec2),
                NodeSocket::optional("Time",      SocketType::Float, "0.0"),
                NodeSocket::optional("Intensity", SocketType::Float, "0.5"),
                NodeSocket::optional("Seed",      SocketType::Float, "0.0"),
            ],
            NodeType::BoxBlur => vec![
                NodeSocket::required("Tex",      SocketType::Sampler2D),
                NodeSocket::required("UV",       SocketType::Vec2),
                NodeSocket::optional("Radius",   SocketType::Float,"1.0"),
                NodeSocket::optional("TexelSize",SocketType::Vec2, "vec2(0.001)"),
            ],
            NodeType::Posterize => vec![
                NodeSocket::required("Color",  SocketType::Vec3),
                NodeSocket::optional("Steps",  SocketType::Float, "4.0"),
            ],
            NodeType::Duotone => vec![
                NodeSocket::required("Color",     SocketType::Vec3),
                NodeSocket::optional("Shadow",    SocketType::Vec3, "vec3(0.0,0.0,0.3)"),
                NodeSocket::optional("Highlight", SocketType::Vec3, "vec3(1.0,0.8,0.2)"),
            ],
            NodeType::Outline => vec![
                NodeSocket::required("SDF",      SocketType::Float),
                NodeSocket::optional("Color",    SocketType::Vec3, "vec3(1.0)"),
                NodeSocket::optional("Thickness",SocketType::Float,"0.02"),
            ],
            NodeType::SdfCircle => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Center", SocketType::Vec2, "vec2(0.5)"),
                NodeSocket::optional("Radius", SocketType::Float, "0.3"),
            ],
            NodeType::SdfBox => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Center", SocketType::Vec2, "vec2(0.5)"),
                NodeSocket::optional("Size",   SocketType::Vec2, "vec2(0.3)"),
                NodeSocket::optional("Corner", SocketType::Float, "0.0"),
            ],
            NodeType::SdfLine => vec![
                NodeSocket::required("UV", SocketType::Vec2),
                NodeSocket::required("A",  SocketType::Vec2),
                NodeSocket::required("B",  SocketType::Vec2),
            ],
            NodeType::SdfTriangle => vec![
                NodeSocket::required("UV", SocketType::Vec2),
                NodeSocket::required("A",  SocketType::Vec2),
                NodeSocket::required("B",  SocketType::Vec2),
                NodeSocket::required("C",  SocketType::Vec2),
            ],
            NodeType::SdfRing => vec![
                NodeSocket::required("UV",         SocketType::Vec2),
                NodeSocket::optional("Center",     SocketType::Vec2,  "vec2(0.5)"),
                NodeSocket::optional("OuterRadius",SocketType::Float, "0.4"),
                NodeSocket::optional("InnerRadius",SocketType::Float, "0.3"),
            ],
            NodeType::SdfStar => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Points", SocketType::Float, "5.0"),
                NodeSocket::optional("Inner",  SocketType::Float, "0.2"),
                NodeSocket::optional("Outer",  SocketType::Float, "0.4"),
            ],
            NodeType::SdfSmoothUnion | NodeType::SdfSmoothSubtract | NodeType::SdfSmoothIntersect => vec![
                NodeSocket::required("A", SocketType::Float),
                NodeSocket::required("B", SocketType::Float),
                NodeSocket::optional("K", SocketType::Float, "0.1"),
            ],
            NodeType::SdfToAlpha | NodeType::SdfToSoftAlpha => vec![
                NodeSocket::required("SDF",       SocketType::Float),
                NodeSocket::optional("Threshold", SocketType::Float, "0.0"),
                NodeSocket::optional("Feather",   SocketType::Float, "0.01"),
            ],
            NodeType::Mandelbrot | NodeType::Julia | NodeType::BurningShip |
            NodeType::NewtonFractal => vec![
                NodeSocket::required("UV",       SocketType::Vec2),
                NodeSocket::optional("MaxIter",  SocketType::Float, "100.0"),
                NodeSocket::optional("Zoom",     SocketType::Float, "1.0"),
                NodeSocket::optional("Cx",       SocketType::Float, "-0.7"),
                NodeSocket::optional("Cy",       SocketType::Float, "0.27"),
            ],
            NodeType::LorenzAttractor => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Time",   SocketType::Float, "0.0"),
                NodeSocket::optional("Scale",  SocketType::Float, "0.05"),
            ],
            NodeType::LyapunovViz => vec![
                NodeSocket::required("UV",     SocketType::Vec2),
                NodeSocket::optional("Seq",    SocketType::Float, "0.0"),
                NodeSocket::optional("Iters",  SocketType::Float, "100.0"),
            ],
            NodeType::IfGreater | NodeType::IfLess => vec![
                NodeSocket::required("A",         SocketType::Any),
                NodeSocket::optional("Threshold", SocketType::Float, "0.5"),
                NodeSocket::required("TrueVal",   SocketType::Any),
                NodeSocket::required("FalseVal",  SocketType::Any),
            ],
            NodeType::ConditionalBlend => vec![
                NodeSocket::required("Condition", SocketType::Float),
                NodeSocket::required("A",         SocketType::Any),
                NodeSocket::required("B",         SocketType::Any),
                NodeSocket::optional("Feather",   SocketType::Float, "0.05"),
            ],
            NodeType::TextureSample => vec![
                NodeSocket::required("Tex", SocketType::Sampler2D),
                NodeSocket::required("UV",  SocketType::Vec2),
            ],
            NodeType::ScreenShake => vec![
                NodeSocket::required("UV",        SocketType::Vec2),
                NodeSocket::optional("Strength",  SocketType::Float, "0.0"),
                NodeSocket::optional("Time",      SocketType::Float, "0.0"),
            ],
            NodeType::OutputColor | NodeType::OutputWithBloom => vec![
                NodeSocket::required("Color", SocketType::Vec4),
            ],
            NodeType::OutputTarget(_) => vec![
                NodeSocket::required("Color", SocketType::Vec4),
            ],
            // Input nodes — no inputs
            _ => vec![],
        }
    }

    /// Output socket definitions.
    pub fn output_sockets(&self) -> Vec<NodeSocket> {
        match self {
            NodeType::UvCoord | NodeType::ScreenCoord | NodeType::RotateVec2 => vec![
                NodeSocket::optional("UV", SocketType::Vec2, "vec2(0.0)"),
            ],
            NodeType::WorldPos | NodeType::CameraPos => vec![
                NodeSocket::optional("Pos", SocketType::Vec3, "vec3(0.0)"),
            ],
            NodeType::Time => vec![
                NodeSocket::optional("T", SocketType::Float, "0.0"),
            ],
            NodeType::Resolution => vec![
                NodeSocket::optional("Res", SocketType::Vec2, "vec2(1.0)"),
            ],
            NodeType::ConstFloat(_) => vec![
                NodeSocket::optional("Value", SocketType::Float, "0.0"),
            ],
            NodeType::ConstVec2(_, _) => vec![
                NodeSocket::optional("Value", SocketType::Vec2, "vec2(0.0)"),
            ],
            NodeType::ConstVec3(..) => vec![
                NodeSocket::optional("Value", SocketType::Vec3, "vec3(0.0)"),
            ],
            NodeType::ConstVec4(..) | NodeType::VertexColor => vec![
                NodeSocket::optional("Value", SocketType::Vec4, "vec4(0.0)"),
            ],
            NodeType::Uniform(_, t) => vec![
                NodeSocket::optional("Value", *t, t.default_value()),
            ],
            NodeType::TextureSample => vec![
                NodeSocket::optional("RGBA", SocketType::Vec4, "vec4(0.0)"),
                NodeSocket::optional("RGB",  SocketType::Vec3, "vec3(0.0)"),
                NodeSocket::optional("A",    SocketType::Float, "0.0"),
            ],
            NodeType::CombineVec2 => vec![
                NodeSocket::optional("XY", SocketType::Vec2, "vec2(0.0)"),
            ],
            NodeType::CombineVec3 | NodeType::HsvToRgb | NodeType::RgbToHsv |
            NodeType::WorldPos | NodeType::Normalize | NodeType::Reflect | NodeType::Cross => vec![
                NodeSocket::optional("Out", SocketType::Vec3, "vec3(0.0)"),
            ],
            NodeType::CombineVec4 => vec![
                NodeSocket::optional("RGBA", SocketType::Vec4, "vec4(0.0)"),
            ],
            NodeType::SplitVec2 => vec![
                NodeSocket::optional("X", SocketType::Float, "0.0"),
                NodeSocket::optional("Y", SocketType::Float, "0.0"),
            ],
            NodeType::SplitVec3 => vec![
                NodeSocket::optional("X", SocketType::Float, "0.0"),
                NodeSocket::optional("Y", SocketType::Float, "0.0"),
                NodeSocket::optional("Z", SocketType::Float, "0.0"),
            ],
            NodeType::SplitVec4 => vec![
                NodeSocket::optional("X", SocketType::Float, "0.0"),
                NodeSocket::optional("Y", SocketType::Float, "0.0"),
                NodeSocket::optional("Z", SocketType::Float, "0.0"),
                NodeSocket::optional("W", SocketType::Float, "0.0"),
            ],
            // Output nodes — no outputs
            NodeType::OutputColor | NodeType::OutputTarget(_) | NodeType::OutputWithBloom => vec![],
            // Most nodes produce a single "Out" of Any type
            _ => vec![
                NodeSocket::optional("Out", SocketType::Any, "0.0"),
            ],
        }
    }

    pub fn is_input_node(&self) -> bool {
        matches!(self,
            NodeType::UvCoord | NodeType::WorldPos | NodeType::CameraPos |
            NodeType::Time | NodeType::Resolution | NodeType::ConstFloat(_) |
            NodeType::ConstVec2(..) | NodeType::ConstVec3(..) | NodeType::ConstVec4(..) |
            NodeType::Uniform(..) | NodeType::VertexColor | NodeType::ScreenCoord
        )
    }

    pub fn is_output_node(&self) -> bool {
        matches!(self,
            NodeType::OutputColor | NodeType::OutputTarget(_) | NodeType::OutputWithBloom
        )
    }

    pub fn output_count(&self) -> usize { self.output_sockets().len() }
    pub fn input_count(&self)  -> usize { self.input_sockets().len() }
}

// ── ShaderNode ────────────────────────────────────────────────────────────────

/// A single node in the shader graph.
#[derive(Debug, Clone)]
pub struct ShaderNode {
    pub id:        NodeId,
    pub node_type: NodeType,
    /// Editor layout position.
    pub editor_x:  f32,
    pub editor_y:  f32,
    /// Per-input constant fallback values (used when socket is not connected).
    pub constant_inputs: HashMap<usize, String>,
    /// Optional label override.
    pub label:     Option<String>,
    /// Whether this node is bypassed (output = first input).
    pub bypassed:  bool,
    /// Whether this node is muted (output = zero/transparent).
    pub muted:     bool,
}

impl ShaderNode {
    pub fn new(id: NodeId, node_type: NodeType) -> Self {
        Self {
            id, node_type,
            editor_x:        0.0,
            editor_y:        0.0,
            constant_inputs: HashMap::new(),
            label:           None,
            bypassed:        false,
            muted:           false,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_constant(mut self, slot: usize, value: impl Into<String>) -> Self {
        self.constant_inputs.insert(slot, value.into());
        self
    }

    pub fn display_label(&self) -> &str {
        self.label.as_deref().unwrap_or_else(|| self.node_type.label())
    }

    /// Variable name used in compiled GLSL for the output of this node.
    pub fn var_name(&self, slot: usize) -> String {
        format!("n{}_{}", self.id.0, slot)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_node_types_have_labels() {
        let types = [
            NodeType::Add, NodeType::Multiply, NodeType::Sin, NodeType::Cos,
            NodeType::PerlinNoise, NodeType::Mandelbrot, NodeType::OutputColor,
        ];
        for t in &types {
            assert!(!t.label().is_empty());
        }
    }

    #[test]
    fn test_socket_compatibility() {
        assert!(SocketType::Float.is_compatible_with(SocketType::Float));
        assert!(SocketType::Any.is_compatible_with(SocketType::Vec3));
        assert!(!SocketType::Float.is_compatible_with(SocketType::Vec4));
    }

    #[test]
    fn test_node_input_output_counts() {
        let add = NodeType::Add;
        assert_eq!(add.input_count(), 2);
        assert_eq!(add.output_count(), 1);

        let uv = NodeType::UvCoord;
        assert_eq!(uv.input_count(), 0);
        assert_eq!(uv.output_count(), 1);

        let out = NodeType::OutputColor;
        assert_eq!(out.input_count(), 1);
        assert_eq!(out.output_count(), 0);
    }

    #[test]
    fn test_var_name() {
        let node = ShaderNode::new(NodeId(42), NodeType::Add);
        assert_eq!(node.var_name(0), "n42_0");
        assert_eq!(node.var_name(1), "n42_1");
    }

    #[test]
    fn test_socket_default_values() {
        let s = NodeSocket::optional("test", SocketType::Vec3, "vec3(0.0)");
        assert_eq!(s.default, "vec3(0.0)");
        assert!(!s.required);
    }

    #[test]
    fn test_is_input_output_node() {
        assert!(NodeType::UvCoord.is_input_node());
        assert!(!NodeType::Add.is_input_node());
        assert!(NodeType::OutputColor.is_output_node());
        assert!(!NodeType::Add.is_output_node());
    }
}
