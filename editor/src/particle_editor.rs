// particle_editor.rs — Full Particle System Editor Panel
// Part of the proof-engine egui editor.

use egui::{self, Color32, Pos2, Rect, Stroke, Vec2, Painter, FontId, Shape, RichText};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

// ─────────────────────────────────────────────────────────────────────────────
// CORE DATA TYPES
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SimulationSpace {
    Local,
    World,
}

impl Default for SimulationSpace {
    fn default() -> Self { SimulationSpace::Local }
}

impl SimulationSpace {
    fn label(&self) -> &str {
        match self {
            SimulationSpace::Local => "Local",
            SimulationSpace::World => "World",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScalingMode {
    Hierarchy,
    Local,
    Shape,
}

impl Default for ScalingMode {
    fn default() -> Self { ScalingMode::Hierarchy }
}

impl ScalingMode {
    fn label(&self) -> &str {
        match self {
            ScalingMode::Hierarchy => "Hierarchy",
            ScalingMode::Local => "Local",
            ScalingMode::Shape => "Shape",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EmitterShape {
    Point,
    Sphere { radius: f32, hemisphere: bool },
    Cone { angle: f32, radius: f32 },
    Box { size: [f32; 3] },
    Circle { radius: f32, arc: f32 },
    Edge { length: f32 },
    Mesh,
}

impl Default for EmitterShape {
    fn default() -> Self { EmitterShape::Cone { angle: 25.0, radius: 1.0 } }
}

impl EmitterShape {
    fn label(&self) -> &str {
        match self {
            EmitterShape::Point => "Point",
            EmitterShape::Sphere { .. } => "Sphere",
            EmitterShape::Cone { .. } => "Cone",
            EmitterShape::Box { .. } => "Box",
            EmitterShape::Circle { .. } => "Circle",
            EmitterShape::Edge { .. } => "Edge",
            EmitterShape::Mesh => "Mesh",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Burst {
    pub time: f32,
    pub count: u32,
    pub repeat_interval: f32,
    pub repeat_count: i32, // -1 = infinite
}

impl Default for Burst {
    fn default() -> Self {
        Burst { time: 0.0, count: 30, repeat_interval: 1.0, repeat_count: 1 }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CURVE
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Interpolation {
    Linear,
    Smooth,
    Constant,
    Bezier,
}

impl Default for Interpolation {
    fn default() -> Self { Interpolation::Smooth }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurveKey {
    pub time: f32,       // 0..1
    pub value: f32,
    pub in_tangent: f32,
    pub out_tangent: f32,
    pub interpolation: Interpolation,
}

impl CurveKey {
    pub fn new(time: f32, value: f32) -> Self {
        CurveKey { time, value, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }
    }
    pub fn linear(time: f32, value: f32) -> Self {
        CurveKey { time, value, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Linear }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CurveMode {
    Constant,
    Curve,
    RandomBetweenTwoCurves,
}

impl Default for CurveMode {
    fn default() -> Self { CurveMode::Curve }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Curve {
    pub keys: Vec<CurveKey>,
    pub keys2: Vec<CurveKey>, // for RandomBetweenTwoCurves
    pub mode: CurveMode,
    pub multiplier: f32,
}

impl Default for Curve {
    fn default() -> Self {
        Curve {
            keys: vec![CurveKey::new(0.0, 1.0), CurveKey::new(1.0, 1.0)],
            keys2: vec![CurveKey::new(0.0, 0.0), CurveKey::new(1.0, 0.0)],
            mode: CurveMode::Curve,
            multiplier: 1.0,
        }
    }
}

impl Curve {
    pub fn constant(value: f32) -> Self {
        Curve {
            keys: vec![CurveKey::new(0.0, value), CurveKey::new(1.0, value)],
            keys2: vec![],
            mode: CurveMode::Constant,
            multiplier: 1.0,
        }
    }

    pub fn linear_zero_to_one() -> Self {
        Curve {
            keys: vec![CurveKey::linear(0.0, 0.0), CurveKey::linear(1.0, 1.0)],
            keys2: vec![],
            mode: CurveMode::Curve,
            multiplier: 1.0,
        }
    }

    pub fn linear_one_to_zero() -> Self {
        Curve {
            keys: vec![CurveKey::linear(0.0, 1.0), CurveKey::linear(1.0, 0.0)],
            keys2: vec![],
            mode: CurveMode::Curve,
            multiplier: 1.0,
        }
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if self.keys.len() == 0 { return 0.0; }
        if self.keys.len() == 1 { return self.keys[0].value * self.multiplier; }

        // Find segment
        let keys = &self.keys;
        let mut seg_idx = keys.len() - 2;
        for i in 0..keys.len()-1 {
            if t <= keys[i+1].time {
                seg_idx = i;
                break;
            }
        }

        let k0 = &keys[seg_idx];
        let k1 = &keys[seg_idx + 1];

        if k1.time <= k0.time { return k0.value * self.multiplier; }

        let local_t = (t - k0.time) / (k1.time - k0.time);

        let value = match k0.interpolation {
            Interpolation::Constant => k0.value,
            Interpolation::Linear => lerp(k0.value, k1.value, local_t),
            Interpolation::Smooth | Interpolation::Bezier => {
                // Cubic hermite
                let dt = k1.time - k0.time;
                let m0 = k0.out_tangent * dt;
                let m1 = k1.in_tangent * dt;
                let t2 = local_t * local_t;
                let t3 = t2 * local_t;
                let h00 = 2.0*t3 - 3.0*t2 + 1.0;
                let h10 = t3 - 2.0*t2 + local_t;
                let h01 = -2.0*t3 + 3.0*t2;
                let h11 = t3 - t2;
                h00*k0.value + h10*m0 + h01*k1.value + h11*m1
            }
        };

        value * self.multiplier
    }

    pub fn evaluate_random(&self, t: f32, rng_val: f32) -> f32 {
        match self.mode {
            CurveMode::Constant => {
                if self.keys.is_empty() { 0.0 } else { self.keys[0].value * self.multiplier }
            }
            CurveMode::Curve => self.evaluate(t),
            CurveMode::RandomBetweenTwoCurves => {
                let v0 = self.evaluate(t);
                let v1 = self.evaluate_curve2(t);
                lerp(v0, v1, rng_val)
            }
        }
    }

    fn evaluate_curve2(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if self.keys2.is_empty() { return 0.0; }
        if self.keys2.len() == 1 { return self.keys2[0].value * self.multiplier; }

        let keys = &self.keys2;
        let mut seg_idx = keys.len() - 2;
        for i in 0..keys.len()-1 {
            if t <= keys[i+1].time { seg_idx = i; break; }
        }
        let k0 = &keys[seg_idx];
        let k1 = &keys[seg_idx + 1];
        if k1.time <= k0.time { return k0.value * self.multiplier; }
        let local_t = (t - k0.time) / (k1.time - k0.time);
        lerp(k0.value, k1.value, local_t) * self.multiplier
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GRADIENT
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GradientColorKey {
    pub time: f32,
    pub color: [f32; 3], // RGB 0..1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GradientAlphaKey {
    pub time: f32,
    pub alpha: f32, // 0..1
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GradientMode {
    Blend,
    Fixed,
}

impl Default for GradientMode {
    fn default() -> Self { GradientMode::Blend }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorGradient {
    pub color_keys: Vec<GradientColorKey>,
    pub alpha_keys: Vec<GradientAlphaKey>,
    pub mode: GradientMode,
}

impl Default for ColorGradient {
    fn default() -> Self {
        ColorGradient {
            color_keys: vec![
                GradientColorKey { time: 0.0, color: [1.0, 1.0, 1.0] },
                GradientColorKey { time: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            alpha_keys: vec![
                GradientAlphaKey { time: 0.0, alpha: 1.0 },
                GradientAlphaKey { time: 1.0, alpha: 0.0 },
            ],
            mode: GradientMode::Blend,
        }
    }
}

impl ColorGradient {
    pub fn evaluate(&self, t: f32) -> Color32 {
        let t = t.clamp(0.0, 1.0);

        // Color
        let (r, g, b) = if self.color_keys.is_empty() {
            (1.0f32, 1.0f32, 1.0f32)
        } else if self.color_keys.len() == 1 {
            let c = &self.color_keys[0].color;
            (c[0], c[1], c[2])
        } else {
            let mut lo = &self.color_keys[0];
            let mut hi = &self.color_keys[self.color_keys.len()-1];
            for i in 0..self.color_keys.len()-1 {
                if t >= self.color_keys[i].time && t <= self.color_keys[i+1].time {
                    lo = &self.color_keys[i];
                    hi = &self.color_keys[i+1];
                    break;
                }
            }
            let span = hi.time - lo.time;
            let local_t = if span < 1e-6 { 0.0 } else { (t - lo.time) / span };
            let local_t = match self.mode {
                GradientMode::Blend => local_t,
                GradientMode::Fixed => 0.0,
            };
            (
                lerp(lo.color[0], hi.color[0], local_t),
                lerp(lo.color[1], hi.color[1], local_t),
                lerp(lo.color[2], hi.color[2], local_t),
            )
        };

        // Alpha
        let a = if self.alpha_keys.is_empty() {
            1.0f32
        } else if self.alpha_keys.len() == 1 {
            self.alpha_keys[0].alpha
        } else {
            let mut lo = &self.alpha_keys[0];
            let mut hi = &self.alpha_keys[self.alpha_keys.len()-1];
            for i in 0..self.alpha_keys.len()-1 {
                if t >= self.alpha_keys[i].time && t <= self.alpha_keys[i+1].time {
                    lo = &self.alpha_keys[i];
                    hi = &self.alpha_keys[i+1];
                    break;
                }
            }
            let span = hi.time - lo.time;
            let local_t = if span < 1e-6 { 0.0 } else { (t - lo.time) / span };
            match self.mode {
                GradientMode::Blend => lerp(lo.alpha, hi.alpha, local_t),
                GradientMode::Fixed => lo.alpha,
            }
        };

        Color32::from_rgba_unmultiplied(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        )
    }

    pub fn fire() -> Self {
        ColorGradient {
            color_keys: vec![
                GradientColorKey { time: 0.0, color: [1.0, 0.9, 0.1] },
                GradientColorKey { time: 0.4, color: [1.0, 0.4, 0.0] },
                GradientColorKey { time: 1.0, color: [0.2, 0.0, 0.0] },
            ],
            alpha_keys: vec![
                GradientAlphaKey { time: 0.0, alpha: 1.0 },
                GradientAlphaKey { time: 0.8, alpha: 0.5 },
                GradientAlphaKey { time: 1.0, alpha: 0.0 },
            ],
            mode: GradientMode::Blend,
        }
    }

    pub fn smoke() -> Self {
        ColorGradient {
            color_keys: vec![
                GradientColorKey { time: 0.0, color: [0.6, 0.6, 0.6] },
                GradientColorKey { time: 1.0, color: [0.3, 0.3, 0.3] },
            ],
            alpha_keys: vec![
                GradientAlphaKey { time: 0.0, alpha: 0.8 },
                GradientAlphaKey { time: 1.0, alpha: 0.0 },
            ],
            mode: GradientMode::Blend,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RANGE OR CURVE
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RangeOrCurve {
    Constant(f32),
    Random(f32, f32),
    Curve(Curve),
    RandomCurve(Curve, Curve),
}

impl Default for RangeOrCurve {
    fn default() -> Self { RangeOrCurve::Constant(1.0) }
}

impl RangeOrCurve {
    pub fn label(&self) -> &str {
        match self {
            RangeOrCurve::Constant(_) => "Constant",
            RangeOrCurve::Random(_, _) => "Random Between Two Constants",
            RangeOrCurve::Curve(_) => "Curve",
            RangeOrCurve::RandomCurve(_, _) => "Random Between Two Curves",
        }
    }

    pub fn sample(&self, t: f32, rng: f32) -> f32 {
        match self {
            RangeOrCurve::Constant(v) => *v,
            RangeOrCurve::Random(lo, hi) => lerp(*lo, *hi, rng),
            RangeOrCurve::Curve(c) => c.evaluate(t),
            RangeOrCurve::RandomCurve(c0, c1) => lerp(c0.evaluate(t), c1.evaluate(t), rng),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// COLOR MODE
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ColorMode {
    Solid([f32; 4]),
    RandomBetweenTwo([f32; 4], [f32; 4]),
    Gradient(ColorGradient),
    RandomFromGradient(ColorGradient),
}

impl Default for ColorMode {
    fn default() -> Self { ColorMode::Solid([1.0, 1.0, 1.0, 1.0]) }
}

impl ColorMode {
    pub fn label(&self) -> &str {
        match self {
            ColorMode::Solid(_) => "Solid",
            ColorMode::RandomBetweenTwo(_, _) => "Random Between Two",
            ColorMode::Gradient(_) => "Gradient",
            ColorMode::RandomFromGradient(_) => "Random From Gradient",
        }
    }

    pub fn sample_color(&self, rng: f32) -> [f32; 4] {
        match self {
            ColorMode::Solid(c) => *c,
            ColorMode::RandomBetweenTwo(a, b) => {
                [lerp(a[0], b[0], rng), lerp(a[1], b[1], rng), lerp(a[2], b[2], rng), lerp(a[3], b[3], rng)]
            }
            ColorMode::Gradient(g) => {
                let c = g.evaluate(0.0);
                [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, c.a() as f32 / 255.0]
            }
            ColorMode::RandomFromGradient(g) => {
                let c = g.evaluate(rng);
                [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, c.a() as f32 / 255.0]
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// COLLISION / TRIGGER / MISC ENUMS
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CollisionMode {
    Planes,
    World,
}

impl Default for CollisionMode {
    fn default() -> Self { CollisionMode::World }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TriggerAction {
    Kill,
    Callback,
    Ignore,
}

impl Default for TriggerAction {
    fn default() -> Self { TriggerAction::Ignore }
}

impl TriggerAction {
    fn label(&self) -> &str {
        match self {
            TriggerAction::Kill => "Kill",
            TriggerAction::Callback => "Callback",
            TriggerAction::Ignore => "Ignore",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnimationType {
    WholeSheet,
    SingleRow,
}

impl Default for AnimationType {
    fn default() -> Self { AnimationType::WholeSheet }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RenderMode {
    Billboard,
    StretchedBillboard,
    HorizontalBillboard,
    VerticalBillboard,
    Mesh,
    None,
}

impl Default for RenderMode {
    fn default() -> Self { RenderMode::Billboard }
}

impl RenderMode {
    fn label(&self) -> &str {
        match self {
            RenderMode::Billboard => "Billboard",
            RenderMode::StretchedBillboard => "Stretched Billboard",
            RenderMode::HorizontalBillboard => "Horizontal Billboard",
            RenderMode::VerticalBillboard => "Vertical Billboard",
            RenderMode::Mesh => "Mesh",
            RenderMode::None => "None",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BillboardAlignment {
    View,
    World,
    Local,
    Facing,
    Velocity,
}

impl Default for BillboardAlignment {
    fn default() -> Self { BillboardAlignment::View }
}

impl BillboardAlignment {
    fn label(&self) -> &str {
        match self {
            BillboardAlignment::View => "View",
            BillboardAlignment::World => "World",
            BillboardAlignment::Local => "Local",
            BillboardAlignment::Facing => "Facing",
            BillboardAlignment::Velocity => "Velocity",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SortMode {
    None,
    ByDistance,
    OldestFirst,
    NewestFirst,
}

impl Default for SortMode {
    fn default() -> Self { SortMode::None }
}

impl SortMode {
    fn label(&self) -> &str {
        match self {
            SortMode::None => "None",
            SortMode::ByDistance => "By Distance",
            SortMode::OldestFirst => "Oldest First",
            SortMode::NewestFirst => "Newest First",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EMITTER CONFIG
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmitterConfig {
    pub shape: EmitterShape,
    pub emission_rate: f32,
    pub emission_bursts: Vec<Burst>,
    pub start_lifetime: RangeOrCurve,
    pub start_speed: RangeOrCurve,
    pub start_size: RangeOrCurve,
    pub start_size_3d: bool,
    pub start_size_x: RangeOrCurve,
    pub start_size_y: RangeOrCurve,
    pub start_size_z: RangeOrCurve,
    pub start_rotation: RangeOrCurve,
    pub start_color: ColorMode,
    pub gravity_modifier: f32,
    pub inherit_velocity: f32,
    pub custom_data: HashMap<String, f32>,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        EmitterConfig {
            shape: EmitterShape::default(),
            emission_rate: 10.0,
            emission_bursts: vec![],
            start_lifetime: RangeOrCurve::Constant(5.0),
            start_speed: RangeOrCurve::Constant(5.0),
            start_size: RangeOrCurve::Constant(1.0),
            start_size_3d: false,
            start_size_x: RangeOrCurve::Constant(1.0),
            start_size_y: RangeOrCurve::Constant(1.0),
            start_size_z: RangeOrCurve::Constant(1.0),
            start_rotation: RangeOrCurve::Constant(0.0),
            start_color: ColorMode::Solid([1.0, 1.0, 1.0, 1.0]),
            gravity_modifier: 0.0,
            inherit_velocity: 0.0,
            custom_data: HashMap::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE MODULE
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ParticleModule {
    VelocityOverLifetime {
        enabled: bool,
        x: Curve,
        y: Curve,
        z: Curve,
        space: SimulationSpace,
    },
    LimitVelocityOverLifetime {
        enabled: bool,
        speed: Curve,
        dampen: f32,
    },
    ForceOverLifetime {
        enabled: bool,
        x: Curve,
        y: Curve,
        z: Curve,
        space: SimulationSpace,
    },
    ColorOverLifetime {
        enabled: bool,
        gradient: ColorGradient,
    },
    ColorBySpeed {
        enabled: bool,
        gradient: ColorGradient,
        range: (f32, f32),
    },
    SizeOverLifetime {
        enabled: bool,
        size: Curve,
    },
    SizeBySpeed {
        enabled: bool,
        size: Curve,
        range: (f32, f32),
    },
    RotationOverLifetime {
        enabled: bool,
        angular_velocity: Curve,
    },
    RotationBySpeed {
        enabled: bool,
        angular_velocity: Curve,
        range: (f32, f32),
    },
    ExternalForces {
        enabled: bool,
        multiplier: f32,
    },
    Noise {
        enabled: bool,
        strength: f32,
        frequency: f32,
        scroll_speed: f32,
        damping: bool,
        octaves: u32,
        remap: Option<Curve>,
    },
    Collision {
        enabled: bool,
        mode: CollisionMode,
        bounce: f32,
        lifetime_loss: f32,
        min_kill_speed: f32,
    },
    Triggers {
        enabled: bool,
        enter: TriggerAction,
        exit: TriggerAction,
        inside: TriggerAction,
    },
    SubEmitters {
        enabled: bool,
        birth: Option<usize>,
        death: Option<usize>,
        collision: Option<usize>,
    },
    TextureSheetAnimation {
        enabled: bool,
        tiles: (u32, u32),
        animation_type: AnimationType,
        frame_over_time: Curve,
        start_frame: RangeOrCurve,
    },
    Lights {
        enabled: bool,
        ratio: f32,
        intensity: Curve,
        range: Curve,
    },
    Trails {
        enabled: bool,
        ratio: f32,
        lifetime: f32,
        min_vertex_distance: f32,
        color: ColorGradient,
        width: Curve,
    },
    Renderer {
        enabled: bool,
        render_mode: RenderMode,
        billboard: BillboardAlignment,
        sort_mode: SortMode,
        material: String,
        shadow_casting: bool,
    },
}

impl ParticleModule {
    pub fn name(&self) -> &str {
        match self {
            ParticleModule::VelocityOverLifetime { .. } => "Velocity over Lifetime",
            ParticleModule::LimitVelocityOverLifetime { .. } => "Limit Velocity over Lifetime",
            ParticleModule::ForceOverLifetime { .. } => "Force over Lifetime",
            ParticleModule::ColorOverLifetime { .. } => "Color over Lifetime",
            ParticleModule::ColorBySpeed { .. } => "Color by Speed",
            ParticleModule::SizeOverLifetime { .. } => "Size over Lifetime",
            ParticleModule::SizeBySpeed { .. } => "Size by Speed",
            ParticleModule::RotationOverLifetime { .. } => "Rotation over Lifetime",
            ParticleModule::RotationBySpeed { .. } => "Rotation by Speed",
            ParticleModule::ExternalForces { .. } => "External Forces",
            ParticleModule::Noise { .. } => "Noise",
            ParticleModule::Collision { .. } => "Collision",
            ParticleModule::Triggers { .. } => "Triggers",
            ParticleModule::SubEmitters { .. } => "Sub Emitters",
            ParticleModule::TextureSheetAnimation { .. } => "Texture Sheet Animation",
            ParticleModule::Lights { .. } => "Lights",
            ParticleModule::Trails { .. } => "Trails",
            ParticleModule::Renderer { .. } => "Renderer",
        }
    }

    pub fn is_enabled(&self) -> bool {
        match self {
            ParticleModule::VelocityOverLifetime { enabled, .. } => *enabled,
            ParticleModule::LimitVelocityOverLifetime { enabled, .. } => *enabled,
            ParticleModule::ForceOverLifetime { enabled, .. } => *enabled,
            ParticleModule::ColorOverLifetime { enabled, .. } => *enabled,
            ParticleModule::ColorBySpeed { enabled, .. } => *enabled,
            ParticleModule::SizeOverLifetime { enabled, .. } => *enabled,
            ParticleModule::SizeBySpeed { enabled, .. } => *enabled,
            ParticleModule::RotationOverLifetime { enabled, .. } => *enabled,
            ParticleModule::RotationBySpeed { enabled, .. } => *enabled,
            ParticleModule::ExternalForces { enabled, .. } => *enabled,
            ParticleModule::Noise { enabled, .. } => *enabled,
            ParticleModule::Collision { enabled, .. } => *enabled,
            ParticleModule::Triggers { enabled, .. } => *enabled,
            ParticleModule::SubEmitters { enabled, .. } => *enabled,
            ParticleModule::TextureSheetAnimation { enabled, .. } => *enabled,
            ParticleModule::Lights { enabled, .. } => *enabled,
            ParticleModule::Trails { enabled, .. } => *enabled,
            ParticleModule::Renderer { enabled, .. } => *enabled,
        }
    }

    pub fn set_enabled(&mut self, val: bool) {
        match self {
            ParticleModule::VelocityOverLifetime { enabled, .. } => *enabled = val,
            ParticleModule::LimitVelocityOverLifetime { enabled, .. } => *enabled = val,
            ParticleModule::ForceOverLifetime { enabled, .. } => *enabled = val,
            ParticleModule::ColorOverLifetime { enabled, .. } => *enabled = val,
            ParticleModule::ColorBySpeed { enabled, .. } => *enabled = val,
            ParticleModule::SizeOverLifetime { enabled, .. } => *enabled = val,
            ParticleModule::SizeBySpeed { enabled, .. } => *enabled = val,
            ParticleModule::RotationOverLifetime { enabled, .. } => *enabled = val,
            ParticleModule::RotationBySpeed { enabled, .. } => *enabled = val,
            ParticleModule::ExternalForces { enabled, .. } => *enabled = val,
            ParticleModule::Noise { enabled, .. } => *enabled = val,
            ParticleModule::Collision { enabled, .. } => *enabled = val,
            ParticleModule::Triggers { enabled, .. } => *enabled = val,
            ParticleModule::SubEmitters { enabled, .. } => *enabled = val,
            ParticleModule::TextureSheetAnimation { enabled, .. } => *enabled = val,
            ParticleModule::Lights { enabled, .. } => *enabled = val,
            ParticleModule::Trails { enabled, .. } => *enabled = val,
            ParticleModule::Renderer { enabled, .. } => *enabled = val,
        }
    }

    pub fn category(&self) -> &str {
        match self {
            ParticleModule::VelocityOverLifetime { .. }
            | ParticleModule::LimitVelocityOverLifetime { .. }
            | ParticleModule::ForceOverLifetime { .. }
            | ParticleModule::ExternalForces { .. }
            | ParticleModule::Noise { .. } => "Physics",

            ParticleModule::ColorOverLifetime { .. }
            | ParticleModule::ColorBySpeed { .. }
            | ParticleModule::SizeOverLifetime { .. }
            | ParticleModule::SizeBySpeed { .. }
            | ParticleModule::RotationOverLifetime { .. }
            | ParticleModule::RotationBySpeed { .. } => "Appearance",

            ParticleModule::Collision { .. }
            | ParticleModule::Triggers { .. }
            | ParticleModule::SubEmitters { .. } => "Interaction",

            ParticleModule::TextureSheetAnimation { .. }
            | ParticleModule::Lights { .. }
            | ParticleModule::Trails { .. }
            | ParticleModule::Renderer { .. } => "Rendering",
        }
    }
}

// Default constructors for each module type
fn default_velocity_over_lifetime() -> ParticleModule {
    ParticleModule::VelocityOverLifetime {
        enabled: true,
        x: Curve::constant(0.0),
        y: Curve::constant(0.0),
        z: Curve::constant(0.0),
        space: SimulationSpace::Local,
    }
}

fn default_limit_velocity_over_lifetime() -> ParticleModule {
    ParticleModule::LimitVelocityOverLifetime {
        enabled: true,
        speed: Curve::constant(5.0),
        dampen: 0.5,
    }
}

fn default_force_over_lifetime() -> ParticleModule {
    ParticleModule::ForceOverLifetime {
        enabled: true,
        x: Curve::constant(0.0),
        y: Curve::constant(-9.8),
        z: Curve::constant(0.0),
        space: SimulationSpace::World,
    }
}

fn default_color_over_lifetime() -> ParticleModule {
    ParticleModule::ColorOverLifetime {
        enabled: true,
        gradient: ColorGradient::default(),
    }
}

fn default_color_by_speed() -> ParticleModule {
    ParticleModule::ColorBySpeed {
        enabled: true,
        gradient: ColorGradient::default(),
        range: (0.0, 10.0),
    }
}

fn default_size_over_lifetime() -> ParticleModule {
    ParticleModule::SizeOverLifetime {
        enabled: true,
        size: Curve::linear_one_to_zero(),
    }
}

fn default_size_by_speed() -> ParticleModule {
    ParticleModule::SizeBySpeed {
        enabled: true,
        size: Curve::linear_zero_to_one(),
        range: (0.0, 10.0),
    }
}

fn default_rotation_over_lifetime() -> ParticleModule {
    ParticleModule::RotationOverLifetime {
        enabled: true,
        angular_velocity: Curve::constant(45.0),
    }
}

fn default_rotation_by_speed() -> ParticleModule {
    ParticleModule::RotationBySpeed {
        enabled: true,
        angular_velocity: Curve::linear_zero_to_one(),
        range: (0.0, 10.0),
    }
}

fn default_external_forces() -> ParticleModule {
    ParticleModule::ExternalForces { enabled: true, multiplier: 1.0 }
}

fn default_noise() -> ParticleModule {
    ParticleModule::Noise {
        enabled: true,
        strength: 1.0,
        frequency: 0.5,
        scroll_speed: 0.0,
        damping: true,
        octaves: 1,
        remap: None,
    }
}

fn default_collision() -> ParticleModule {
    ParticleModule::Collision {
        enabled: true,
        mode: CollisionMode::World,
        bounce: 0.5,
        lifetime_loss: 0.0,
        min_kill_speed: 0.0,
    }
}

fn default_triggers() -> ParticleModule {
    ParticleModule::Triggers {
        enabled: true,
        enter: TriggerAction::Kill,
        exit: TriggerAction::Ignore,
        inside: TriggerAction::Ignore,
    }
}

fn default_sub_emitters() -> ParticleModule {
    ParticleModule::SubEmitters {
        enabled: true,
        birth: None,
        death: None,
        collision: None,
    }
}

fn default_texture_sheet_animation() -> ParticleModule {
    ParticleModule::TextureSheetAnimation {
        enabled: true,
        tiles: (4, 4),
        animation_type: AnimationType::WholeSheet,
        frame_over_time: Curve::linear_zero_to_one(),
        start_frame: RangeOrCurve::Constant(0.0),
    }
}

fn default_lights() -> ParticleModule {
    ParticleModule::Lights {
        enabled: true,
        ratio: 0.2,
        intensity: Curve::linear_one_to_zero(),
        range: Curve::constant(5.0),
    }
}

fn default_trails() -> ParticleModule {
    ParticleModule::Trails {
        enabled: true,
        ratio: 1.0,
        lifetime: 0.5,
        min_vertex_distance: 0.1,
        color: ColorGradient::default(),
        width: Curve::linear_one_to_zero(),
    }
}

fn default_renderer() -> ParticleModule {
    ParticleModule::Renderer {
        enabled: true,
        render_mode: RenderMode::Billboard,
        billboard: BillboardAlignment::View,
        sort_mode: SortMode::None,
        material: "Default-Particle".to_string(),
        shadow_casting: false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE SYSTEM
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticleSystem {
    pub name: String,
    pub emitter: EmitterConfig,
    pub modules: Vec<ParticleModule>,
    pub max_particles: u32,
    pub duration: f32,
    pub looping: bool,
    pub prewarm: bool,
    pub simulation_space: SimulationSpace,
    pub scaling_mode: ScalingMode,
    pub play_on_awake: bool,
}

impl Default for ParticleSystem {
    fn default() -> Self {
        ParticleSystem {
            name: "New Particle System".to_string(),
            emitter: EmitterConfig::default(),
            modules: vec![
                default_color_over_lifetime(),
                default_size_over_lifetime(),
                default_renderer(),
            ],
            max_particles: 1000,
            duration: 5.0,
            looping: true,
            prewarm: false,
            simulation_space: SimulationSpace::Local,
            scaling_mode: ScalingMode::Hierarchy,
            play_on_awake: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PREVIEW PARTICLE
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PreviewParticle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub age: f32,
    pub lifetime: f32,
    pub size: f32,
    pub base_size: f32,
    pub rotation: f32,
    pub angular_velocity: f32,
    pub color: [f32; 4],
    pub base_color: [f32; 4],
    pub rng_seed: f32,
    pub trail: Vec<[f32; 2]>,
}

impl PreviewParticle {
    pub fn life_ratio(&self) -> f32 {
        if self.lifetime < 1e-6 { 1.0 } else { (self.age / self.lifetime).clamp(0.0, 1.0) }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CURVE / GRADIENT EDITOR TARGETS
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum CurveTarget {
    EmitterStartLifetime,
    EmitterStartSpeed,
    EmitterStartSize,
    EmitterStartRotation,
    ModuleCurveX(usize),
    ModuleCurveY(usize),
    ModuleCurveZ(usize),
    ModuleCurveMain(usize),
    ModuleCurveRange(usize),
    ModuleCurveLights(usize, u8), // 0=intensity, 1=range
    ModuleCurveTrailWidth(usize),
    ModuleCurveFrameOverTime(usize),
}

#[derive(Clone, Debug)]
pub enum GradientTarget {
    EmitterStartColor,
    ModuleColorMain(usize),
    ModuleColorRange(usize),
    ModuleTrailColor(usize),
}

// ─────────────────────────────────────────────────────────────────────────────
// CURVE EDITOR STATE
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct CurveEditorState {
    pub selected_key: Option<usize>,
    pub dragging_key: Option<usize>,
    pub view_min: f32,
    pub view_max: f32,
    pub show_second_curve: bool,
    pub right_click_key: Option<usize>,
}

impl Default for CurveEditorState {
    fn default() -> Self {
        CurveEditorState {
            selected_key: None,
            dragging_key: None,
            view_min: -0.1,
            view_max: 1.1,
            show_second_curve: false,
            right_click_key: None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GRADIENT EDITOR STATE
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct GradientEditorState {
    pub selected_color_key: Option<usize>,
    pub selected_alpha_key: Option<usize>,
    pub dragging: Option<(bool, usize)>, // (is_alpha, idx)
    pub color_picker_open: bool,
    pub edit_color: [f32; 3],
}

impl Default for GradientEditorState {
    fn default() -> Self {
        GradientEditorState {
            selected_color_key: None,
            selected_alpha_key: None,
            dragging: None,
            color_picker_open: false,
            edit_color: [1.0, 1.0, 1.0],
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MAIN PARTICLE EDITOR
// ─────────────────────────────────────────────────────────────────────────────

pub struct ParticleEditor {
    pub systems: Vec<ParticleSystem>,
    pub active_system: usize,
    pub selected_module: Option<usize>,
    pub preview_running: bool,
    pub preview_time: f32,
    pub preview_speed: f32,
    pub show_preview: bool,
    pub particles: Vec<PreviewParticle>,
    pub curve_editor_open: bool,
    pub editing_curve: Option<CurveTarget>,
    pub gradient_editor_open: bool,
    pub editing_gradient: Option<GradientTarget>,
    pub search: String,

    // Internal state
    pub curve_editor_state: CurveEditorState,
    pub gradient_editor_state: GradientEditorState,
    pub emission_accumulator: f32,
    pub burst_timers: Vec<(usize, f32, u32)>, // (burst_idx, elapsed, repeats_done)
    pub show_add_module_menu: bool,
    pub module_expanded: Vec<bool>,
    pub custom_presets: Vec<(String, ParticleSystem)>,
    pub show_preset_save_dialog: bool,
    pub preset_save_name: String,
    pub next_rng: u64,
    pub preview_pan: Vec2,
    pub preview_zoom: f32,
    pub show_shape_visualizer: bool,
}

impl ParticleEditor {
    pub fn new() -> Self {
        let mut editor = ParticleEditor {
            systems: vec![ParticleSystem::default()],
            active_system: 0,
            selected_module: None,
            preview_running: false,
            preview_time: 0.0,
            preview_speed: 1.0,
            show_preview: true,
            particles: vec![],
            curve_editor_open: false,
            editing_curve: None,
            gradient_editor_open: false,
            editing_gradient: None,
            search: String::new(),
            curve_editor_state: CurveEditorState::default(),
            gradient_editor_state: GradientEditorState::default(),
            emission_accumulator: 0.0,
            burst_timers: vec![],
            show_add_module_menu: false,
            module_expanded: vec![false, false, true], // match default modules
            custom_presets: vec![],
            show_preset_save_dialog: false,
            preset_save_name: String::new(),
            next_rng: 12345,
            preview_pan: Vec2::ZERO,
            preview_zoom: 1.0,
            show_shape_visualizer: true,
        };
        editor
    }

    fn rng(&mut self) -> f32 {
        self.next_rng = self.next_rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let bits = (self.next_rng >> 33) as u32;
        bits as f32 / u32::MAX as f32
    }

    fn rng_range(&mut self, lo: f32, hi: f32) -> f32 {
        let r = self.rng();
        lerp(lo, hi, r)
    }

    fn spawn_particle(&mut self) {
        let (start_lifetime, start_speed, start_size, start_rotation, start_color) = {
            let em = &self.systems[self.active_system].emitter;
            (em.start_lifetime.clone(), em.start_speed.clone(), em.start_size.clone(),
             em.start_rotation.clone(), em.start_color.clone())
        };

        let rng_lifetime = self.rng();
        let lifetime = start_lifetime.sample(0.0, rng_lifetime);

        let rng_speed = self.rng();
        let speed = start_speed.sample(0.0, rng_speed);

        // Direction based on shape
        let (px, py, vx_dir, vy_dir) = self.emit_from_shape(speed);

        let rng_size = self.rng();
        let size = start_size.sample(0.0, rng_size);

        let rng_rot = self.rng();
        let rotation = start_rotation.sample(0.0, rng_rot);

        let rng_color = self.rng();
        let color = start_color.sample_color(rng_color);

        let rng_seed = self.rng();

        self.particles.push(PreviewParticle {
            position: [px, py],
            velocity: [vx_dir, vy_dir],
            age: 0.0,
            lifetime,
            size,
            base_size: size,
            rotation,
            angular_velocity: 0.0,
            color,
            base_color: color,
            rng_seed,
            trail: vec![],
        });
    }

    fn emit_from_shape(&mut self, speed: f32) -> (f32, f32, f32, f32) {
        let shape = self.systems[self.active_system].emitter.shape.clone();
        match &shape {
            EmitterShape::Point => (0.0, 0.0, 0.0, speed),
            EmitterShape::Sphere { radius, .. } => {
                let angle = self.rng() * std::f32::consts::TAU;
                let r = self.rng() * radius;
                let nx = angle.cos();
                let ny = angle.sin();
                (nx * r, ny * r, nx * speed, ny * speed)
            }
            EmitterShape::Cone { angle, radius } => {
                let half_angle = angle.to_radians() * 0.5;
                let spread = self.rng() * half_angle * 2.0 - half_angle;
                let vx = spread.sin() * speed;
                let vy = spread.cos() * speed;
                let r = self.rng() * radius;
                let side_angle = self.rng() * std::f32::consts::TAU;
                (side_angle.cos() * r, side_angle.sin() * r * 0.3, vx, vy)
            }
            EmitterShape::Box { size } => {
                let px = (self.rng() - 0.5) * size[0];
                let py = (self.rng() - 0.5) * size[1];
                (px, py, 0.0, speed)
            }
            EmitterShape::Circle { radius, arc } => {
                let t = self.rng() * arc.to_radians();
                let r = self.rng() * radius;
                let px = t.cos() * r;
                let py = t.sin() * r;
                let vx = t.cos() * speed;
                let vy = t.sin() * speed;
                (px, py, vx, vy)
            }
            EmitterShape::Edge { length } => {
                let px = (self.rng() - 0.5) * length;
                (px, 0.0, 0.0, speed)
            }
            EmitterShape::Mesh => (0.0, 0.0, 0.0, speed),
        }
    }

    fn simulate_step(&mut self, dt: f32) {
        if !self.preview_running { return; }

        let sys_idx = self.active_system;
        let dt_scaled = dt * self.preview_speed;
        self.preview_time += dt_scaled;

        let sys = &self.systems[sys_idx];
        if sys.looping && self.preview_time > sys.duration {
            self.preview_time = 0.0;
            self.emission_accumulator = 0.0;
        }

        // Spawn from emission rate
        let rate = sys.emitter.emission_rate;
        self.emission_accumulator += rate * dt_scaled;
        let to_spawn = self.emission_accumulator.floor() as u32;
        self.emission_accumulator -= to_spawn as f32;

        let current_count = self.particles.len() as u32;
        let max_p = sys.max_particles;

        for _ in 0..to_spawn {
            if self.particles.len() < max_p as usize {
                self.spawn_particle();
            }
        }

        // Collect module data for simulation (avoid borrow conflict)
        struct ModData {
            has_color_ol: bool,
            color_ol_gradient: Option<ColorGradient>,
            has_size_ol: bool,
            size_ol_curve: Option<Curve>,
            has_vel_ol: bool,
            vel_ol_x: Option<Curve>,
            vel_ol_y: Option<Curve>,
            has_rot_ol: bool,
            rot_ol_curve: Option<Curve>,
            has_noise: bool,
            noise_strength: f32,
            noise_frequency: f32,
            gravity: f32,
        }

        let mut md = ModData {
            has_color_ol: false,
            color_ol_gradient: None,
            has_size_ol: false,
            size_ol_curve: None,
            has_vel_ol: false,
            vel_ol_x: None,
            vel_ol_y: None,
            has_rot_ol: false,
            rot_ol_curve: None,
            has_noise: false,
            noise_strength: 0.0,
            noise_frequency: 0.5,
            gravity: self.systems[sys_idx].emitter.gravity_modifier,
        };

        for m in &self.systems[sys_idx].modules {
            match m {
                ParticleModule::ColorOverLifetime { enabled: true, gradient } => {
                    md.has_color_ol = true;
                    md.color_ol_gradient = Some(gradient.clone());
                }
                ParticleModule::SizeOverLifetime { enabled: true, size } => {
                    md.has_size_ol = true;
                    md.size_ol_curve = Some(size.clone());
                }
                ParticleModule::VelocityOverLifetime { enabled: true, x, y, .. } => {
                    md.has_vel_ol = true;
                    md.vel_ol_x = Some(x.clone());
                    md.vel_ol_y = Some(y.clone());
                }
                ParticleModule::RotationOverLifetime { enabled: true, angular_velocity } => {
                    md.has_rot_ol = true;
                    md.rot_ol_curve = Some(angular_velocity.clone());
                }
                ParticleModule::Noise { enabled: true, strength, frequency, .. } => {
                    md.has_noise = true;
                    md.noise_strength = *strength;
                    md.noise_frequency = *frequency;
                }
                _ => {}
            }
        }

        // Integrate particles
        let gravity_acc = [0.0f32, -9.8 * md.gravity];
        let mut i = 0;
        while i < self.particles.len() {
            let p = &mut self.particles[i];
            p.age += dt_scaled;

            if p.age >= p.lifetime {
                self.particles.remove(i);
                continue;
            }

            let t = p.life_ratio();

            // Gravity
            p.velocity[0] += gravity_acc[0] * dt_scaled;
            p.velocity[1] += gravity_acc[1] * dt_scaled;

            // Velocity over lifetime
            if md.has_vel_ol {
                if let Some(cx) = &md.vel_ol_x {
                    p.velocity[0] += cx.evaluate(t) * dt_scaled;
                }
                if let Some(cy) = &md.vel_ol_y {
                    p.velocity[1] += cy.evaluate(t) * dt_scaled;
                }
            }

            // Noise
            if md.has_noise {
                let nx = simple_noise(p.position[0] * md.noise_frequency + self.preview_time * 0.3,
                                      p.position[1] * md.noise_frequency);
                let ny = simple_noise(p.position[0] * md.noise_frequency,
                                      p.position[1] * md.noise_frequency + self.preview_time * 0.3);
                p.velocity[0] += nx * md.noise_strength * dt_scaled;
                p.velocity[1] += ny * md.noise_strength * dt_scaled;
            }

            // Move
            p.position[0] += p.velocity[0] * dt_scaled;
            p.position[1] += p.velocity[1] * dt_scaled;

            // Rotation
            if md.has_rot_ol {
                if let Some(rc) = &md.rot_ol_curve {
                    p.rotation += rc.evaluate(t) * dt_scaled;
                }
            }

            // Color over lifetime
            if md.has_color_ol {
                if let Some(g) = &md.color_ol_gradient {
                    let c = g.evaluate(t);
                    p.color = [
                        c.r() as f32 / 255.0 * p.base_color[0],
                        c.g() as f32 / 255.0 * p.base_color[1],
                        c.b() as f32 / 255.0 * p.base_color[2],
                        c.a() as f32 / 255.0 * p.base_color[3],
                    ];
                }
            }

            // Size over lifetime
            if md.has_size_ol {
                if let Some(sc) = &md.size_ol_curve {
                    p.size = p.base_size * sc.evaluate(t);
                }
            }

            // Trail
            let trail_enabled = self.systems[sys_idx].modules.iter().any(|m| {
                matches!(m, ParticleModule::Trails { enabled: true, .. })
            });
            if trail_enabled {
                p.trail.push(p.position);
                if p.trail.len() > 20 { p.trail.remove(0); }
            }

            i += 1;
        }
    }

    pub fn show(ui: &mut egui::Ui, editor: &mut ParticleEditor, dt: f32) {
        editor.simulate_step(dt);

        egui::TopBottomPanel::top("ps_toolbar")
            .resizable(false)
            .show_inside(ui, |ui| {
                show_toolbar(ui, editor);
            });

        egui::SidePanel::left("ps_left_panel")
            .default_width(260.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                show_left_panel(ui, editor);
            });

        egui::SidePanel::right("ps_right_panel")
            .default_width(300.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                show_right_panel(ui, editor);
            });

        egui::CentralPanel::default()
            .show_inside(ui, |ui| {
                show_center_panel(ui, editor);
            });
    }

    pub fn show_panel(ctx: &egui::Context, editor: &mut ParticleEditor, dt: f32, open: &mut bool) {
        egui::Window::new("Particle System Editor")
            .open(open)
            .default_size([1200.0, 800.0])
            .min_size([800.0, 600.0])
            .resizable(true)
            .show(ctx, |ui| {
                ParticleEditor::show(ui, editor, dt);
            });

        // Floating curve editor
        if editor.curve_editor_open {
            let mut curve_open = editor.curve_editor_open;
            egui::Window::new("Curve Editor")
                .open(&mut curve_open)
                .default_size([500.0, 300.0])
                .resizable(true)
                .show(ctx, |ui| {
                    show_curve_editor_window(ui, editor);
                });
            editor.curve_editor_open = curve_open;
        }

        // Floating gradient editor
        if editor.gradient_editor_open {
            let mut grad_open = editor.gradient_editor_open;
            egui::Window::new("Gradient Editor")
                .open(&mut grad_open)
                .default_size([500.0, 200.0])
                .resizable(true)
                .show(ctx, |ui| {
                    show_gradient_editor_window(ui, editor);
                });
            editor.gradient_editor_open = grad_open;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TOOLBAR
// ─────────────────────────────────────────────────────────────────────────────

fn show_toolbar(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
    ui.horizontal(|ui| {
        // System selector
        ui.label("System:");
        let active_name = editor.systems.get(editor.active_system)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        egui::ComboBox::from_id_source("ps_system_selector")
            .selected_text(&active_name)
            .width(140.0)
            .show_ui(ui, |ui| {
                let count = editor.systems.len();
                for i in 0..count {
                    let name = editor.systems[i].name.clone();
                    if ui.selectable_value(&mut editor.active_system, i, &name).changed() {
                        editor.particles.clear();
                        editor.emission_accumulator = 0.0;
                        editor.preview_time = 0.0;
                        let mod_count = editor.systems[editor.active_system].modules.len();
                        editor.module_expanded = vec![false; mod_count];
                        editor.selected_module = None;
                    }
                }
            });

        if ui.small_button("+").on_hover_text("New System").clicked() {
            let mut ps = ParticleSystem::default();
            ps.name = format!("System {}", editor.systems.len() + 1);
            editor.systems.push(ps);
            editor.active_system = editor.systems.len() - 1;
            let mod_count = editor.systems[editor.active_system].modules.len();
            editor.module_expanded = vec![false; mod_count];
            editor.selected_module = None;
            editor.particles.clear();
        }

        if editor.systems.len() > 1 {
            if ui.small_button("✕").on_hover_text("Remove System").clicked() {
                editor.systems.remove(editor.active_system);
                if editor.active_system >= editor.systems.len() {
                    editor.active_system = editor.systems.len().saturating_sub(1);
                }
                editor.particles.clear();
            }
        }

        ui.separator();

        // Playback controls
        let play_label = if editor.preview_running { "⏸ Pause" } else { "▶ Play" };
        if ui.button(play_label).clicked() {
            editor.preview_running = !editor.preview_running;
        }

        if ui.button("⏹ Stop").clicked() {
            editor.preview_running = false;
            editor.preview_time = 0.0;
            editor.particles.clear();
            editor.emission_accumulator = 0.0;
        }

        if ui.button("↺ Restart").clicked() {
            editor.preview_running = true;
            editor.preview_time = 0.0;
            editor.particles.clear();
            editor.emission_accumulator = 0.0;
        }

        ui.label("Speed:");
        ui.add(egui::DragValue::new(&mut editor.preview_speed)
            .speed(0.01)
            .range(0.01..=10.0)
            .suffix("x"));

        let p_count = editor.particles.len();
        let max_p = editor.systems.get(editor.active_system).map(|s| s.max_particles).unwrap_or(0);
        ui.separator();
        ui.label(format!("Particles: {}/{}", p_count, max_p));

        let t = editor.preview_time;
        let dur = editor.systems.get(editor.active_system).map(|s| s.duration).unwrap_or(5.0);
        ui.label(format!("t: {:.2}/{:.2}s", t, dur));

        ui.separator();
        ui.checkbox(&mut editor.show_preview, "Preview");
        ui.checkbox(&mut editor.show_shape_visualizer, "Shape");

        ui.separator();
        // Preset dropdown
        egui::menu::menu_button(ui, "Presets", |ui| {
            ui.label(RichText::new("Built-in Presets").strong());
            ui.separator();
            let preset_names = [
                "Fire", "Smoke", "Sparks", "Magic Dust", "Rain", "Snow",
                "Explosion", "Level Up", "Heal", "Poison", "Blood",
                "Shockwave", "Portal", "Stars", "Confetti",
            ];
            for name in preset_names {
                if ui.button(name).clicked() {
                    apply_preset(editor, name);
                    ui.close_menu();
                }
            }
            ui.separator();
            ui.label(RichText::new("Custom Presets").strong());
            let custom_names: Vec<String> = editor.custom_presets.iter().map(|(n,_)| n.clone()).collect();
            for (i, name) in custom_names.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.button(name).clicked() {
                        let sys = editor.custom_presets[i].1.clone();
                        editor.systems[editor.active_system] = sys;
                        editor.particles.clear();
                        let mod_count = editor.systems[editor.active_system].modules.len();
                        editor.module_expanded = vec![false; mod_count];
                        ui.close_menu();
                    }
                    if ui.small_button("✕").clicked() {
                        editor.custom_presets.remove(i);
                    }
                });
            }
            ui.separator();
            if ui.button("Save Current as Preset...").clicked() {
                editor.show_preset_save_dialog = true;
                editor.preset_save_name = editor.systems[editor.active_system].name.clone();
                ui.close_menu();
            }
        });

        if editor.show_preset_save_dialog {
            // shown inline for simplicity
            ui.separator();
            ui.label("Preset name:");
            ui.text_edit_singleline(&mut editor.preset_save_name);
            if ui.small_button("Save").clicked() {
                let name = editor.preset_save_name.clone();
                let sys = editor.systems[editor.active_system].clone();
                editor.custom_presets.push((name, sys));
                editor.show_preset_save_dialog = false;
            }
            if ui.small_button("Cancel").clicked() {
                editor.show_preset_save_dialog = false;
            }
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// LEFT PANEL — Emitter Config
// ─────────────────────────────────────────────────────────────────────────────

fn show_left_panel(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
    ui.heading("Emitter");
    ui.separator();

    egui::ScrollArea::vertical().id_source("ps_left_scroll").show(ui, |ui| {
        let sys = &mut editor.systems[editor.active_system];

        // Name
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut sys.name);
        });

        ui.separator();

        // Main properties collapsible
        egui::CollapsingHeader::new("Main Settings").default_open(true).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Duration");
                ui.add(egui::DragValue::new(&mut sys.duration).speed(0.1).suffix("s").range(0.1..=600.0));
            });
            ui.horizontal(|ui| {
                ui.label("Max Particles");
                ui.add(egui::DragValue::new(&mut sys.max_particles).speed(1.0).range(1..=100000));
            });
            ui.checkbox(&mut sys.looping, "Looping");
            ui.checkbox(&mut sys.prewarm, "Prewarm");
            ui.checkbox(&mut sys.play_on_awake, "Play on Awake");

            ui.horizontal(|ui| {
                ui.label("Simulation Space");
                egui::ComboBox::from_id_source("sim_space")
                    .selected_text(sys.simulation_space.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut sys.simulation_space, SimulationSpace::Local, "Local");
                        ui.selectable_value(&mut sys.simulation_space, SimulationSpace::World, "World");
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Scaling Mode");
                egui::ComboBox::from_id_source("scaling_mode")
                    .selected_text(sys.scaling_mode.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut sys.scaling_mode, ScalingMode::Hierarchy, "Hierarchy");
                        ui.selectable_value(&mut sys.scaling_mode, ScalingMode::Local, "Local");
                        ui.selectable_value(&mut sys.scaling_mode, ScalingMode::Shape, "Shape");
                    });
            });
        });

        ui.separator();

        // Emission
        egui::CollapsingHeader::new("Emission").default_open(true).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Rate over Time");
                ui.add(egui::DragValue::new(&mut sys.emitter.emission_rate).speed(0.1).range(0.0..=10000.0));
            });

            ui.label("Bursts:");
            let mut remove_burst = None;
            for (i, burst) in sys.emitter.emission_bursts.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("#{}", i));
                    ui.label("t:");
                    ui.add(egui::DragValue::new(&mut burst.time).speed(0.01).range(0.0..=600.0));
                    ui.label("n:");
                    ui.add(egui::DragValue::new(&mut burst.count).speed(1.0).range(1..=10000));
                    ui.label("rep:");
                    ui.add(egui::DragValue::new(&mut burst.repeat_count).speed(1.0).range(-1..=1000));
                    if ui.small_button("✕").clicked() { remove_burst = Some(i); }
                });
            }
            if let Some(i) = remove_burst { sys.emitter.emission_bursts.remove(i); }
            if ui.small_button("+ Burst").clicked() {
                sys.emitter.emission_bursts.push(Burst::default());
            }
        });

        ui.separator();

        // Shape
        egui::CollapsingHeader::new("Shape").default_open(true).show(ui, |ui| {
            // Shape selector
            let shape_label = sys.emitter.shape.label().to_string();
            egui::ComboBox::from_id_source("emitter_shape")
                .selected_text(&shape_label)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(matches!(sys.emitter.shape, EmitterShape::Point), "Point").clicked() {
                        sys.emitter.shape = EmitterShape::Point;
                    }
                    if ui.selectable_label(matches!(sys.emitter.shape, EmitterShape::Sphere{..}), "Sphere").clicked() {
                        sys.emitter.shape = EmitterShape::Sphere { radius: 1.0, hemisphere: false };
                    }
                    if ui.selectable_label(matches!(sys.emitter.shape, EmitterShape::Cone{..}), "Cone").clicked() {
                        sys.emitter.shape = EmitterShape::Cone { angle: 25.0, radius: 1.0 };
                    }
                    if ui.selectable_label(matches!(sys.emitter.shape, EmitterShape::Box{..}), "Box").clicked() {
                        sys.emitter.shape = EmitterShape::Box { size: [1.0, 1.0, 1.0] };
                    }
                    if ui.selectable_label(matches!(sys.emitter.shape, EmitterShape::Circle{..}), "Circle").clicked() {
                        sys.emitter.shape = EmitterShape::Circle { radius: 1.0, arc: 360.0 };
                    }
                    if ui.selectable_label(matches!(sys.emitter.shape, EmitterShape::Edge{..}), "Edge").clicked() {
                        sys.emitter.shape = EmitterShape::Edge { length: 2.0 };
                    }
                    if ui.selectable_label(matches!(sys.emitter.shape, EmitterShape::Mesh), "Mesh").clicked() {
                        sys.emitter.shape = EmitterShape::Mesh;
                    }
                });

            // Shape-specific params
            match &mut sys.emitter.shape {
                EmitterShape::Point => {}
                EmitterShape::Sphere { radius, hemisphere } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius");
                        ui.add(egui::DragValue::new(radius).speed(0.01).range(0.01..=100.0));
                    });
                    ui.checkbox(hemisphere, "Hemisphere");
                }
                EmitterShape::Cone { angle, radius } => {
                    ui.horizontal(|ui| {
                        ui.label("Angle");
                        ui.add(egui::DragValue::new(angle).speed(0.5).suffix("°").range(0.0..=180.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Radius");
                        ui.add(egui::DragValue::new(radius).speed(0.01).range(0.0..=100.0));
                    });
                }
                EmitterShape::Box { size } => {
                    ui.horizontal(|ui| {
                        ui.label("X");
                        ui.add(egui::DragValue::new(&mut size[0]).speed(0.01).range(0.0..=100.0));
                        ui.label("Y");
                        ui.add(egui::DragValue::new(&mut size[1]).speed(0.01).range(0.0..=100.0));
                        ui.label("Z");
                        ui.add(egui::DragValue::new(&mut size[2]).speed(0.01).range(0.0..=100.0));
                    });
                }
                EmitterShape::Circle { radius, arc } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius");
                        ui.add(egui::DragValue::new(radius).speed(0.01).range(0.01..=100.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Arc");
                        ui.add(egui::DragValue::new(arc).speed(1.0).suffix("°").range(1.0..=360.0));
                    });
                }
                EmitterShape::Edge { length } => {
                    ui.horizontal(|ui| {
                        ui.label("Length");
                        ui.add(egui::DragValue::new(length).speed(0.01).range(0.0..=100.0));
                    });
                }
                EmitterShape::Mesh => {
                    ui.label("(Mesh emitter — no 2D preview)");
                }
            }

            // Shape visualizer
            if editor.show_shape_visualizer {
                let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 120.0), egui::Sense::hover());
                draw_shape_visualizer(&painter, resp.rect, &sys.emitter.shape);
            }
        });

        ui.separator();

        // Start values
        egui::CollapsingHeader::new("Start Values").default_open(true).show(ui, |ui| {
            show_range_or_curve_row(ui, editor, "Lifetime", 0);
            show_range_or_curve_row(ui, editor, "Speed", 1);
            show_range_or_curve_row(ui, editor, "Size", 2);
            show_range_or_curve_row(ui, editor, "Rotation", 3);

            let sys = &mut editor.systems[editor.active_system];
            ui.horizontal(|ui| {
                ui.label("Gravity");
                ui.add(egui::DragValue::new(&mut sys.emitter.gravity_modifier).speed(0.01).range(-10.0..=10.0));
            });
            ui.horizontal(|ui| {
                ui.label("Inherit Velocity");
                ui.add(egui::DragValue::new(&mut sys.emitter.inherit_velocity).speed(0.01).range(0.0..=1.0));
            });

            // Start color
            ui.label("Start Color:");
            let color_mode_label = sys.emitter.start_color.label().to_string();
            egui::ComboBox::from_id_source("start_color_mode")
                .selected_text(&color_mode_label)
                .show_ui(ui, |ui| {
                    let current = sys.emitter.start_color.clone();
                    if ui.selectable_label(matches!(current, ColorMode::Solid(_)), "Solid").clicked() {
                        sys.emitter.start_color = ColorMode::Solid([1.0,1.0,1.0,1.0]);
                    }
                    if ui.selectable_label(matches!(current, ColorMode::RandomBetweenTwo(..)), "Random Between Two").clicked() {
                        sys.emitter.start_color = ColorMode::RandomBetweenTwo([1.0,1.0,1.0,1.0],[1.0,1.0,1.0,1.0]);
                    }
                    if ui.selectable_label(matches!(current, ColorMode::Gradient(_)), "Gradient").clicked() {
                        sys.emitter.start_color = ColorMode::Gradient(ColorGradient::default());
                    }
                    if ui.selectable_label(matches!(current, ColorMode::RandomFromGradient(_)), "Random From Gradient").clicked() {
                        sys.emitter.start_color = ColorMode::RandomFromGradient(ColorGradient::default());
                    }
                });

            match &mut sys.emitter.start_color {
                ColorMode::Solid(c) => {
                    let mut color = egui::Color32::from_rgba_premultiplied(
                        (c[0]*255.0) as u8, (c[1]*255.0) as u8,
                        (c[2]*255.0) as u8, (c[3]*255.0) as u8,
                    );
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        *c = [color.r() as f32/255.0, color.g() as f32/255.0,
                              color.b() as f32/255.0, color.a() as f32/255.0];
                    }
                }
                ColorMode::RandomBetweenTwo(c0, c1) => {
                    ui.horizontal(|ui| {
                        ui.label("A:");
                        let mut col0 = egui::Color32::from_rgba_premultiplied(
                            (c0[0]*255.0) as u8, (c0[1]*255.0) as u8,
                            (c0[2]*255.0) as u8, (c0[3]*255.0) as u8,
                        );
                        if ui.color_edit_button_srgba(&mut col0).changed() {
                            *c0 = [col0.r() as f32/255.0, col0.g() as f32/255.0,
                                   col0.b() as f32/255.0, col0.a() as f32/255.0];
                        }
                        ui.label("B:");
                        let mut col1 = egui::Color32::from_rgba_premultiplied(
                            (c1[0]*255.0) as u8, (c1[1]*255.0) as u8,
                            (c1[2]*255.0) as u8, (c1[3]*255.0) as u8,
                        );
                        if ui.color_edit_button_srgba(&mut col1).changed() {
                            *c1 = [col1.r() as f32/255.0, col1.g() as f32/255.0,
                                   col1.b() as f32/255.0, col1.a() as f32/255.0];
                        }
                    });
                }
                ColorMode::Gradient(g) | ColorMode::RandomFromGradient(g) => {
                    let target = GradientTarget::EmitterStartColor;
                    if show_gradient_preview_button(ui, g, "Edit Gradient") {
                        editor.gradient_editor_open = true;
                        editor.editing_gradient = Some(target);
                    }
                }
            }
        });

        ui.separator();

        // Custom data
        egui::CollapsingHeader::new("Custom Data").default_open(false).show(ui, |ui| {
            let sys = &mut editor.systems[editor.active_system];
            let mut remove_key: Option<String> = None;
            let pairs: Vec<(String, f32)> = sys.emitter.custom_data.iter().map(|(k,v)| (k.clone(), *v)).collect();
            for (k, mut v) in pairs {
                ui.horizontal(|ui| {
                    ui.label(&k);
                    if ui.add(egui::DragValue::new(&mut v).speed(0.01)).changed() {
                        sys.emitter.custom_data.insert(k.clone(), v);
                    }
                    if ui.small_button("✕").clicked() { remove_key = Some(k.clone()); }
                });
            }
            if let Some(k) = remove_key { sys.emitter.custom_data.remove(&k); }
            if ui.small_button("+ Add Field").clicked() {
                let n = sys.emitter.custom_data.len();
                sys.emitter.custom_data.insert(format!("field_{}", n), 0.0);
            }
        });
    });
}

fn show_range_or_curve_row(ui: &mut egui::Ui, editor: &mut ParticleEditor, label: &str, field: u8) {
    let sys = &mut editor.systems[editor.active_system];
    let roc: &mut RangeOrCurve = match field {
        0 => &mut sys.emitter.start_lifetime,
        1 => &mut sys.emitter.start_speed,
        2 => &mut sys.emitter.start_size,
        3 => &mut sys.emitter.start_rotation,
        _ => return,
    };

    ui.horizontal(|ui| {
        ui.label(label);

        let roc_label = roc.label().to_string();
        egui::ComboBox::from_id_source(format!("roc_mode_{}", field))
            .selected_text(&roc_label)
            .width(160.0)
            .show_ui(ui, |ui| {
                let prev = roc.clone();
                if ui.selectable_label(matches!(prev, RangeOrCurve::Constant(_)), "Constant").clicked() {
                    let v = match &prev { RangeOrCurve::Constant(v) => *v, _ => 1.0 };
                    *roc = RangeOrCurve::Constant(v);
                }
                if ui.selectable_label(matches!(prev, RangeOrCurve::Random(..)), "Random").clicked() {
                    *roc = RangeOrCurve::Random(0.0, 1.0);
                }
                if ui.selectable_label(matches!(prev, RangeOrCurve::Curve(_)), "Curve").clicked() {
                    *roc = RangeOrCurve::Curve(Curve::default());
                }
                if ui.selectable_label(matches!(prev, RangeOrCurve::RandomCurve(..)), "Random Curves").clicked() {
                    *roc = RangeOrCurve::RandomCurve(Curve::default(), Curve::default());
                }
            });
    });

    match roc {
        RangeOrCurve::Constant(v) => {
            ui.add(egui::DragValue::new(v).speed(0.01));
        }
        RangeOrCurve::Random(lo, hi) => {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(lo).speed(0.01));
                ui.label("–");
                ui.add(egui::DragValue::new(hi).speed(0.01));
            });
        }
        RangeOrCurve::Curve(_) | RangeOrCurve::RandomCurve(_, _) => {
            let target = match field {
                0 => CurveTarget::EmitterStartLifetime,
                1 => CurveTarget::EmitterStartSpeed,
                2 => CurveTarget::EmitterStartSize,
                3 => CurveTarget::EmitterStartRotation,
                _ => return,
            };
            if ui.small_button("Edit Curve").clicked() {
                editor.editing_curve = Some(target);
                editor.curve_editor_open = true;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RIGHT PANEL — Module Stack
// ─────────────────────────────────────────────────────────────────────────────

fn show_right_panel(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
    ui.heading("Modules");

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut editor.search);
    });

    ui.separator();

    // Add module menu button
    if ui.button("+ Add Module").clicked() {
        editor.show_add_module_menu = !editor.show_add_module_menu;
    }

    if editor.show_add_module_menu {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_max_width(220.0);
            ui.label(RichText::new("Physics").strong());
            if ui.button("Velocity over Lifetime").clicked() { add_module(editor, default_velocity_over_lifetime()); editor.show_add_module_menu = false; }
            if ui.button("Limit Velocity over Lifetime").clicked() { add_module(editor, default_limit_velocity_over_lifetime()); editor.show_add_module_menu = false; }
            if ui.button("Force over Lifetime").clicked() { add_module(editor, default_force_over_lifetime()); editor.show_add_module_menu = false; }
            if ui.button("External Forces").clicked() { add_module(editor, default_external_forces()); editor.show_add_module_menu = false; }
            if ui.button("Noise").clicked() { add_module(editor, default_noise()); editor.show_add_module_menu = false; }
            ui.separator();
            ui.label(RichText::new("Appearance").strong());
            if ui.button("Color over Lifetime").clicked() { add_module(editor, default_color_over_lifetime()); editor.show_add_module_menu = false; }
            if ui.button("Color by Speed").clicked() { add_module(editor, default_color_by_speed()); editor.show_add_module_menu = false; }
            if ui.button("Size over Lifetime").clicked() { add_module(editor, default_size_over_lifetime()); editor.show_add_module_menu = false; }
            if ui.button("Size by Speed").clicked() { add_module(editor, default_size_by_speed()); editor.show_add_module_menu = false; }
            if ui.button("Rotation over Lifetime").clicked() { add_module(editor, default_rotation_over_lifetime()); editor.show_add_module_menu = false; }
            if ui.button("Rotation by Speed").clicked() { add_module(editor, default_rotation_by_speed()); editor.show_add_module_menu = false; }
            ui.separator();
            ui.label(RichText::new("Interaction").strong());
            if ui.button("Collision").clicked() { add_module(editor, default_collision()); editor.show_add_module_menu = false; }
            if ui.button("Triggers").clicked() { add_module(editor, default_triggers()); editor.show_add_module_menu = false; }
            if ui.button("Sub Emitters").clicked() { add_module(editor, default_sub_emitters()); editor.show_add_module_menu = false; }
            ui.separator();
            ui.label(RichText::new("Rendering").strong());
            if ui.button("Texture Sheet Animation").clicked() { add_module(editor, default_texture_sheet_animation()); editor.show_add_module_menu = false; }
            if ui.button("Lights").clicked() { add_module(editor, default_lights()); editor.show_add_module_menu = false; }
            if ui.button("Trails").clicked() { add_module(editor, default_trails()); editor.show_add_module_menu = false; }
            if ui.button("Renderer").clicked() { add_module(editor, default_renderer()); editor.show_add_module_menu = false; }
        });
    }

    ui.separator();

    let search_lower = editor.search.to_lowercase();

    egui::ScrollArea::vertical().id_source("module_scroll").show(ui, |ui| {
        // Sync module_expanded length
        {
            let mod_count = editor.systems[editor.active_system].modules.len();
            if editor.module_expanded.len() != mod_count {
                editor.module_expanded.resize(mod_count, false);
            }
        }

        let mut move_up: Option<usize> = None;
        let mut move_down: Option<usize> = None;
        let mut remove_mod: Option<usize> = None;

        let mod_count = editor.systems[editor.active_system].modules.len();

        for i in 0..mod_count {
            let mod_name = editor.systems[editor.active_system].modules[i].name().to_string();

            if !search_lower.is_empty() && !mod_name.to_lowercase().contains(&search_lower) {
                continue;
            }

            let enabled = editor.systems[editor.active_system].modules[i].is_enabled();
            let is_selected = editor.selected_module == Some(i);

            // Module header row
            ui.horizontal(|ui| {
                let mut en = enabled;
                if ui.checkbox(&mut en, "").changed() {
                    editor.systems[editor.active_system].modules[i].set_enabled(en);
                }

                let header_text = if is_selected {
                    RichText::new(&mod_name).strong().color(Color32::from_rgb(100, 180, 255))
                } else {
                    RichText::new(&mod_name)
                };

                if ui.selectable_label(is_selected, header_text).clicked() {
                    if editor.selected_module == Some(i) {
                        editor.module_expanded[i] = !editor.module_expanded[i];
                    } else {
                        editor.selected_module = Some(i);
                        editor.module_expanded[i] = true;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").on_hover_text("Remove").clicked() {
                        remove_mod = Some(i);
                    }
                    if i + 1 < mod_count {
                        if ui.small_button("▼").on_hover_text("Move Down").clicked() {
                            move_down = Some(i);
                        }
                    }
                    if i > 0 {
                        if ui.small_button("▲").on_hover_text("Move Up").clicked() {
                            move_up = Some(i);
                        }
                    }
                });
            });

            // Expanded module body
            if editor.module_expanded.get(i).copied().unwrap_or(false) {
                egui::Frame::default()
                    .inner_margin(egui::Margin::same(6))
                    .fill(Color32::from_rgb(35, 35, 40))
                    .show(ui, |ui| {
                        show_module_properties(ui, editor, i);
                    });
            }

            ui.separator();
        }

        if let Some(i) = move_up {
            editor.systems[editor.active_system].modules.swap(i, i-1);
            editor.module_expanded.swap(i, i-1);
        }
        if let Some(i) = move_down {
            editor.systems[editor.active_system].modules.swap(i, i+1);
            editor.module_expanded.swap(i, i+1);
        }
        if let Some(i) = remove_mod {
            editor.systems[editor.active_system].modules.remove(i);
            editor.module_expanded.remove(i);
            if editor.selected_module == Some(i) {
                editor.selected_module = None;
            }
        }
    });
}

fn add_module(editor: &mut ParticleEditor, module: ParticleModule) {
    // Don't add duplicates for modules that are singletons
    let name = module.name().to_string();
    let already = editor.systems[editor.active_system].modules.iter().any(|m| m.name() == name);
    if already { return; }
    editor.systems[editor.active_system].modules.push(module);
    editor.module_expanded.push(true);
    editor.selected_module = Some(editor.systems[editor.active_system].modules.len() - 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// MODULE PROPERTIES UI
// ─────────────────────────────────────────────────────────────────────────────

fn show_module_properties(ui: &mut egui::Ui, editor: &mut ParticleEditor, idx: usize) {
    // We need to clone the module, edit it, then put it back to avoid borrow issues
    let module = editor.systems[editor.active_system].modules[idx].clone();

    match module {
        ParticleModule::VelocityOverLifetime { enabled, mut x, mut y, mut z, mut space } => {
            ui.label("Space:");
            egui::ComboBox::from_id_source(format!("vol_space_{}", idx))
                .selected_text(space.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut space, SimulationSpace::Local, "Local");
                    ui.selectable_value(&mut space, SimulationSpace::World, "World");
                });
            show_curve_field(ui, editor, idx, "X", &x, CurveTarget::ModuleCurveX(idx));
            show_curve_field(ui, editor, idx, "Y", &y, CurveTarget::ModuleCurveY(idx));
            show_curve_field(ui, editor, idx, "Z", &z, CurveTarget::ModuleCurveZ(idx));
            editor.systems[editor.active_system].modules[idx] = ParticleModule::VelocityOverLifetime { enabled, x, y, z, space };
        }

        ParticleModule::LimitVelocityOverLifetime { enabled, speed, mut dampen } => {
            show_curve_field(ui, editor, idx, "Speed Limit", &speed, CurveTarget::ModuleCurveMain(idx));
            ui.horizontal(|ui| {
                ui.label("Dampen");
                ui.add(egui::DragValue::new(&mut dampen).speed(0.01).range(0.0..=1.0));
            });
            editor.systems[editor.active_system].modules[idx] = ParticleModule::LimitVelocityOverLifetime { enabled, speed, dampen };
        }

        ParticleModule::ForceOverLifetime { enabled, x, y, z, mut space } => {
            ui.label("Space:");
            egui::ComboBox::from_id_source(format!("fol_space_{}", idx))
                .selected_text(space.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut space, SimulationSpace::Local, "Local");
                    ui.selectable_value(&mut space, SimulationSpace::World, "World");
                });
            show_curve_field(ui, editor, idx, "X", &x, CurveTarget::ModuleCurveX(idx));
            show_curve_field(ui, editor, idx, "Y", &y, CurveTarget::ModuleCurveY(idx));
            show_curve_field(ui, editor, idx, "Z", &z, CurveTarget::ModuleCurveZ(idx));
            editor.systems[editor.active_system].modules[idx] = ParticleModule::ForceOverLifetime { enabled, x, y, z, space };
        }

        ParticleModule::ColorOverLifetime { enabled, mut gradient } => {
            if show_gradient_preview_button(ui, &gradient, "Edit Gradient") {
                editor.gradient_editor_open = true;
                editor.editing_gradient = Some(GradientTarget::ModuleColorMain(idx));
            }
            editor.systems[editor.active_system].modules[idx] = ParticleModule::ColorOverLifetime { enabled, gradient };
        }

        ParticleModule::ColorBySpeed { enabled, mut gradient, mut range } => {
            if show_gradient_preview_button(ui, &gradient, "Edit Gradient") {
                editor.gradient_editor_open = true;
                editor.editing_gradient = Some(GradientTarget::ModuleColorMain(idx));
            }
            ui.horizontal(|ui| {
                ui.label("Speed Range");
                ui.add(egui::DragValue::new(&mut range.0).speed(0.1));
                ui.label("–");
                ui.add(egui::DragValue::new(&mut range.1).speed(0.1));
            });
            editor.systems[editor.active_system].modules[idx] = ParticleModule::ColorBySpeed { enabled, gradient, range };
        }

        ParticleModule::SizeOverLifetime { enabled, size } => {
            show_curve_field(ui, editor, idx, "Size", &size, CurveTarget::ModuleCurveMain(idx));
            editor.systems[editor.active_system].modules[idx] = ParticleModule::SizeOverLifetime { enabled, size };
        }

        ParticleModule::SizeBySpeed { enabled, size, mut range } => {
            show_curve_field(ui, editor, idx, "Size", &size, CurveTarget::ModuleCurveMain(idx));
            ui.horizontal(|ui| {
                ui.label("Speed Range");
                ui.add(egui::DragValue::new(&mut range.0).speed(0.1));
                ui.label("–");
                ui.add(egui::DragValue::new(&mut range.1).speed(0.1));
            });
            editor.systems[editor.active_system].modules[idx] = ParticleModule::SizeBySpeed { enabled, size, range };
        }

        ParticleModule::RotationOverLifetime { enabled, angular_velocity } => {
            show_curve_field(ui, editor, idx, "Angular Velocity", &angular_velocity, CurveTarget::ModuleCurveMain(idx));
            editor.systems[editor.active_system].modules[idx] = ParticleModule::RotationOverLifetime { enabled, angular_velocity };
        }

        ParticleModule::RotationBySpeed { enabled, angular_velocity, mut range } => {
            show_curve_field(ui, editor, idx, "Angular Velocity", &angular_velocity, CurveTarget::ModuleCurveMain(idx));
            ui.horizontal(|ui| {
                ui.label("Speed Range");
                ui.add(egui::DragValue::new(&mut range.0).speed(0.1));
                ui.label("–");
                ui.add(egui::DragValue::new(&mut range.1).speed(0.1));
            });
            editor.systems[editor.active_system].modules[idx] = ParticleModule::RotationBySpeed { enabled, angular_velocity, range };
        }

        ParticleModule::ExternalForces { enabled, mut multiplier } => {
            ui.horizontal(|ui| {
                ui.label("Multiplier");
                ui.add(egui::DragValue::new(&mut multiplier).speed(0.01).range(0.0..=10.0));
            });
            editor.systems[editor.active_system].modules[idx] = ParticleModule::ExternalForces { enabled, multiplier };
        }

        ParticleModule::Noise { enabled, mut strength, mut frequency, mut scroll_speed, mut damping, mut octaves, remap } => {
            ui.horizontal(|ui| {
                ui.label("Strength");
                ui.add(egui::DragValue::new(&mut strength).speed(0.01).range(0.0..=10.0));
            });
            ui.horizontal(|ui| {
                ui.label("Frequency");
                ui.add(egui::DragValue::new(&mut frequency).speed(0.01).range(0.0..=10.0));
            });
            ui.horizontal(|ui| {
                ui.label("Scroll Speed");
                ui.add(egui::DragValue::new(&mut scroll_speed).speed(0.01).range(0.0..=10.0));
            });
            ui.horizontal(|ui| {
                ui.label("Octaves");
                ui.add(egui::DragValue::new(&mut octaves).speed(1).range(1..=8));
            });
            ui.checkbox(&mut damping, "Damping");
            if let Some(ref c) = remap {
                show_curve_field(ui, editor, idx, "Remap", c, CurveTarget::ModuleCurveMain(idx));
            } else {
                if ui.small_button("Enable Remap Curve").clicked() {
                    editor.systems[editor.active_system].modules[idx] = ParticleModule::Noise {
                        enabled, strength, frequency, scroll_speed, damping, octaves,
                        remap: Some(Curve::default()),
                    };
                    return;
                }
            }
            editor.systems[editor.active_system].modules[idx] = ParticleModule::Noise { enabled, strength, frequency, scroll_speed, damping, octaves, remap };
        }

        ParticleModule::Collision { enabled, mut mode, mut bounce, mut lifetime_loss, mut min_kill_speed } => {
            ui.horizontal(|ui| {
                ui.label("Mode");
                egui::ComboBox::from_id_source(format!("col_mode_{}", idx))
                    .selected_text(match &mode { CollisionMode::Planes => "Planes", CollisionMode::World => "World" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut mode, CollisionMode::Planes, "Planes");
                        ui.selectable_value(&mut mode, CollisionMode::World, "World");
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Bounce");
                ui.add(egui::DragValue::new(&mut bounce).speed(0.01).range(0.0..=2.0));
            });
            ui.horizontal(|ui| {
                ui.label("Lifetime Loss");
                ui.add(egui::DragValue::new(&mut lifetime_loss).speed(0.01).range(0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Min Kill Speed");
                ui.add(egui::DragValue::new(&mut min_kill_speed).speed(0.01).range(0.0..=100.0));
            });
            editor.systems[editor.active_system].modules[idx] = ParticleModule::Collision { enabled, mode, bounce, lifetime_loss, min_kill_speed };
        }

        ParticleModule::Triggers { enabled, mut enter, mut exit, mut inside } => {
            ui.horizontal(|ui| {
                ui.label("On Enter");
                egui::ComboBox::from_id_source(format!("trig_enter_{}", idx))
                    .selected_text(enter.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut enter, TriggerAction::Kill, "Kill");
                        ui.selectable_value(&mut enter, TriggerAction::Callback, "Callback");
                        ui.selectable_value(&mut enter, TriggerAction::Ignore, "Ignore");
                    });
            });
            ui.horizontal(|ui| {
                ui.label("On Exit");
                egui::ComboBox::from_id_source(format!("trig_exit_{}", idx))
                    .selected_text(exit.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut exit, TriggerAction::Kill, "Kill");
                        ui.selectable_value(&mut exit, TriggerAction::Callback, "Callback");
                        ui.selectable_value(&mut exit, TriggerAction::Ignore, "Ignore");
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Inside");
                egui::ComboBox::from_id_source(format!("trig_inside_{}", idx))
                    .selected_text(inside.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut inside, TriggerAction::Kill, "Kill");
                        ui.selectable_value(&mut inside, TriggerAction::Callback, "Callback");
                        ui.selectable_value(&mut inside, TriggerAction::Ignore, "Ignore");
                    });
            });
            editor.systems[editor.active_system].modules[idx] = ParticleModule::Triggers { enabled, enter, exit, inside };
        }

        ParticleModule::SubEmitters { enabled, mut birth, mut death, mut collision } => {
            ui.label("System indices (0-based):");
            ui.horizontal(|ui| {
                ui.label("Birth:");
                let mut v = birth.map(|x| x as i32).unwrap_or(-1);
                if ui.add(egui::DragValue::new(&mut v).speed(1).range(-1..=100)).changed() {
                    birth = if v < 0 { None } else { Some(v as usize) };
                }
            });
            ui.horizontal(|ui| {
                ui.label("Death:");
                let mut v = death.map(|x| x as i32).unwrap_or(-1);
                if ui.add(egui::DragValue::new(&mut v).speed(1).range(-1..=100)).changed() {
                    death = if v < 0 { None } else { Some(v as usize) };
                }
            });
            ui.horizontal(|ui| {
                ui.label("Collision:");
                let mut v = collision.map(|x| x as i32).unwrap_or(-1);
                if ui.add(egui::DragValue::new(&mut v).speed(1).range(-1..=100)).changed() {
                    collision = if v < 0 { None } else { Some(v as usize) };
                }
            });
            editor.systems[editor.active_system].modules[idx] = ParticleModule::SubEmitters { enabled, birth, death, collision };
        }

        ParticleModule::TextureSheetAnimation { enabled, mut tiles, mut animation_type, frame_over_time, start_frame } => {
            ui.horizontal(|ui| {
                ui.label("Tiles X");
                ui.add(egui::DragValue::new(&mut tiles.0).speed(1).range(1..=64));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut tiles.1).speed(1).range(1..=64));
            });
            ui.horizontal(|ui| {
                ui.label("Animation");
                egui::ComboBox::from_id_source(format!("tsa_anim_{}", idx))
                    .selected_text(match &animation_type { AnimationType::WholeSheet => "Whole Sheet", AnimationType::SingleRow => "Single Row" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut animation_type, AnimationType::WholeSheet, "Whole Sheet");
                        ui.selectable_value(&mut animation_type, AnimationType::SingleRow, "Single Row");
                    });
            });
            show_curve_field(ui, editor, idx, "Frame over Time", &frame_over_time, CurveTarget::ModuleCurveFrameOverTime(idx));
            editor.systems[editor.active_system].modules[idx] = ParticleModule::TextureSheetAnimation { enabled, tiles, animation_type, frame_over_time, start_frame };
        }

        ParticleModule::Lights { enabled, mut ratio, intensity, range } => {
            ui.horizontal(|ui| {
                ui.label("Ratio");
                ui.add(egui::DragValue::new(&mut ratio).speed(0.01).range(0.0..=1.0));
            });
            show_curve_field(ui, editor, idx, "Intensity", &intensity, CurveTarget::ModuleCurveLights(idx, 0));
            show_curve_field(ui, editor, idx, "Range", &range, CurveTarget::ModuleCurveLights(idx, 1));
            editor.systems[editor.active_system].modules[idx] = ParticleModule::Lights { enabled, ratio, intensity, range };
        }

        ParticleModule::Trails { enabled, mut ratio, mut lifetime, mut min_vertex_distance, color, width } => {
            ui.horizontal(|ui| {
                ui.label("Ratio");
                ui.add(egui::DragValue::new(&mut ratio).speed(0.01).range(0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Lifetime");
                ui.add(egui::DragValue::new(&mut lifetime).speed(0.01).range(0.0..=10.0));
            });
            ui.horizontal(|ui| {
                ui.label("Min Vertex Distance");
                ui.add(egui::DragValue::new(&mut min_vertex_distance).speed(0.001).range(0.0..=1.0));
            });
            if show_gradient_preview_button(ui, &color, "Edit Color Gradient") {
                editor.gradient_editor_open = true;
                editor.editing_gradient = Some(GradientTarget::ModuleTrailColor(idx));
            }
            show_curve_field(ui, editor, idx, "Width", &width, CurveTarget::ModuleCurveTrailWidth(idx));
            editor.systems[editor.active_system].modules[idx] = ParticleModule::Trails { enabled, ratio, lifetime, min_vertex_distance, color, width };
        }

        ParticleModule::Renderer { enabled, mut render_mode, mut billboard, mut sort_mode, mut material, mut shadow_casting } => {
            ui.horizontal(|ui| {
                ui.label("Render Mode");
                egui::ComboBox::from_id_source(format!("rend_mode_{}", idx))
                    .selected_text(render_mode.label())
                    .show_ui(ui, |ui| {
                        let modes = [
                            RenderMode::Billboard, RenderMode::StretchedBillboard,
                            RenderMode::HorizontalBillboard, RenderMode::VerticalBillboard,
                            RenderMode::Mesh, RenderMode::None,
                        ];
                        for m in modes {
                            let lbl = m.label().to_string();
                            let sel = std::mem::discriminant(&render_mode) == std::mem::discriminant(&m);
                            if ui.selectable_label(sel, &lbl).clicked() {
                                render_mode = m;
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Billboard Align");
                egui::ComboBox::from_id_source(format!("rend_bb_{}", idx))
                    .selected_text(billboard.label())
                    .show_ui(ui, |ui| {
                        let aligns = [
                            BillboardAlignment::View, BillboardAlignment::World,
                            BillboardAlignment::Local, BillboardAlignment::Facing,
                            BillboardAlignment::Velocity,
                        ];
                        for a in aligns {
                            let lbl = a.label().to_string();
                            let sel = std::mem::discriminant(&billboard) == std::mem::discriminant(&a);
                            if ui.selectable_label(sel, &lbl).clicked() { billboard = a; }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Sort Mode");
                egui::ComboBox::from_id_source(format!("rend_sort_{}", idx))
                    .selected_text(sort_mode.label())
                    .show_ui(ui, |ui| {
                        let sorts = [SortMode::None, SortMode::ByDistance, SortMode::OldestFirst, SortMode::NewestFirst];
                        for s in sorts {
                            let lbl = s.label().to_string();
                            let sel = std::mem::discriminant(&sort_mode) == std::mem::discriminant(&s);
                            if ui.selectable_label(sel, &lbl).clicked() { sort_mode = s; }
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Material");
                ui.text_edit_singleline(&mut material);
            });
            ui.checkbox(&mut shadow_casting, "Cast Shadows");
            editor.systems[editor.active_system].modules[idx] = ParticleModule::Renderer { enabled, render_mode, billboard, sort_mode, material, shadow_casting };
        }
    }
}

fn show_curve_field(ui: &mut egui::Ui, editor: &mut ParticleEditor, _module_idx: usize, label: &str, curve: &Curve, target: CurveTarget) {
    ui.horizontal(|ui| {
        ui.label(label);
        // Mini inline curve preview
        let (resp, painter) = ui.allocate_painter(Vec2::new(80.0, 20.0), egui::Sense::hover());
        draw_curve_mini(&painter, resp.rect, curve);
        if ui.small_button("Edit").clicked() {
            editor.editing_curve = Some(target);
            editor.curve_editor_open = true;
        }
    });
}

fn show_gradient_preview_button(ui: &mut egui::Ui, gradient: &ColorGradient, label: &str) -> bool {
    ui.horizontal(|ui| {
        // Preview bar
        let (resp, painter) = ui.allocate_painter(Vec2::new(80.0, 16.0), egui::Sense::hover());
        draw_gradient_bar(&painter, resp.rect, gradient);
        ui.button(label).clicked()
    }).inner
}

// ─────────────────────────────────────────────────────────────────────────────
// CENTER PANEL — Preview
// ─────────────────────────────────────────────────────────────────────────────

fn show_center_panel(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
    if !editor.show_preview { return; }

    ui.heading("Preview");
    ui.separator();

    let avail = ui.available_size();
    let preview_size = Vec2::new(avail.x, (avail.y - 40.0).max(100.0));

    let (resp, painter) = ui.allocate_painter(preview_size, egui::Sense::drag());
    let rect = resp.rect;

    // Pan/zoom
    if resp.dragged() {
        editor.preview_pan += resp.drag_delta();
    }
    if let Some(hover_pos) = resp.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            editor.preview_zoom *= 1.0 + scroll * 0.001;
            editor.preview_zoom = editor.preview_zoom.clamp(0.1, 10.0);
        }
    }

    // Background
    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 25));

    // Grid
    draw_preview_grid(&painter, rect, editor.preview_pan, editor.preview_zoom);

    // Draw particles
    let center = rect.center() + editor.preview_pan;
    let pixels_per_unit = 30.0 * editor.preview_zoom;

    // Draw trails first
    for p in &editor.particles {
        if p.trail.len() > 1 {
            for i in 0..p.trail.len()-1 {
                let a = p.trail[i];
                let b = p.trail[i+1];
                let pa = Pos2::new(center.x + a[0] * pixels_per_unit, center.y - a[1] * pixels_per_unit);
                let pb = Pos2::new(center.x + b[0] * pixels_per_unit, center.y - b[1] * pixels_per_unit);
                let t_trail = i as f32 / p.trail.len() as f32;
                let alpha = (t_trail * p.color[3] * 0.5 * 255.0) as u8;
                let stroke_color = Color32::from_rgba_unmultiplied(
                    (p.color[0]*255.0) as u8, (p.color[1]*255.0) as u8,
                    (p.color[2]*255.0) as u8, alpha,
                );
                painter.line_segment([pa, pb], Stroke::new(1.0, stroke_color));
            }
        }
    }

    // Draw particles
    for p in &editor.particles {
        let px = center.x + p.position[0] * pixels_per_unit;
        let py = center.y - p.position[1] * pixels_per_unit;
        let pos = Pos2::new(px, py);
        let r = (p.size * pixels_per_unit * 0.1).max(1.0);
        let color = Color32::from_rgba_unmultiplied(
            (p.color[0] * 255.0) as u8,
            (p.color[1] * 255.0) as u8,
            (p.color[2] * 255.0) as u8,
            (p.color[3] * 255.0) as u8,
        );
        // Draw as circle with soft glow
        painter.circle_filled(pos, r, color);
        if r > 2.0 {
            painter.circle_stroke(pos, r + 1.0, Stroke::new(0.5, Color32::from_rgba_unmultiplied(
                color.r(), color.g(), color.b(), (color.a() as f32 * 0.3) as u8)));
        }
    }

    // Emitter origin marker
    painter.circle_stroke(center, 4.0, Stroke::new(1.0, Color32::from_rgb(80, 200, 80)));
    painter.line_segment([center - Vec2::new(6.0, 0.0), center + Vec2::new(6.0, 0.0)],
        Stroke::new(1.0, Color32::from_rgb(80, 200, 80)));
    painter.line_segment([center - Vec2::new(0.0, 6.0), center + Vec2::new(0.0, 6.0)],
        Stroke::new(1.0, Color32::from_rgb(80, 200, 80)));

    // HUD
    painter.text(rect.min + Vec2::new(8.0, 8.0), egui::Align2::LEFT_TOP,
        format!("Particles: {} | t: {:.2}s", editor.particles.len(), editor.preview_time),
        FontId::proportional(11.0), Color32::from_rgb(180, 180, 180));

    // Controls hint
    painter.text(rect.max - Vec2::new(8.0, 8.0), egui::Align2::RIGHT_BOTTOM,
        "Drag: pan | Scroll: zoom",
        FontId::proportional(10.0), Color32::from_rgb(100, 100, 100));
}

fn draw_preview_grid(painter: &Painter, rect: Rect, pan: Vec2, zoom: f32) {
    let grid_color = Color32::from_rgb(35, 35, 40);
    let grid_spacing = 30.0 * zoom;
    if grid_spacing < 5.0 { return; }

    let center = rect.center() + pan;
    let x_start = ((rect.min.x - center.x) / grid_spacing).floor() as i32;
    let x_end = ((rect.max.x - center.x) / grid_spacing).ceil() as i32;
    let y_start = ((rect.min.y - center.y) / grid_spacing).floor() as i32;
    let y_end = ((rect.max.y - center.y) / grid_spacing).ceil() as i32;

    for xi in x_start..=x_end {
        let x = center.x + xi as f32 * grid_spacing;
        painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            Stroke::new(1.0, grid_color));
    }
    for yi in y_start..=y_end {
        let y = center.y + yi as f32 * grid_spacing;
        painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            Stroke::new(1.0, grid_color));
    }

    // Axes
    let axis_color = Color32::from_rgb(60, 60, 70);
    painter.line_segment([Pos2::new(center.x, rect.min.y), Pos2::new(center.x, rect.max.y)],
        Stroke::new(1.0, axis_color));
    painter.line_segment([Pos2::new(rect.min.x, center.y), Pos2::new(rect.max.x, center.y)],
        Stroke::new(1.0, axis_color));
}

// ─────────────────────────────────────────────────────────────────────────────
// SHAPE VISUALIZER
// ─────────────────────────────────────────────────────────────────────────────

fn draw_shape_visualizer(painter: &Painter, rect: Rect, shape: &EmitterShape) {
    painter.rect_filled(rect, 2.0, Color32::from_rgb(25, 25, 30));
    let center = rect.center();
    let scale = rect.height() * 0.35;
    let line_color = Color32::from_rgb(80, 200, 120);
    let dim_color = Color32::from_rgb(50, 100, 60);

    match shape {
        EmitterShape::Point => {
            painter.circle_filled(center, 3.0, line_color);
        }
        EmitterShape::Sphere { radius, hemisphere } => {
            let r = (radius * scale * 0.5).min(rect.height() * 0.45);
            if *hemisphere {
                // Draw semicircle
                let pts: Vec<Pos2> = (0..=32).map(|i| {
                    let a = i as f32 / 32.0 * std::f32::consts::PI;
                    Pos2::new(center.x + a.cos() * r, center.y - a.sin() * r)
                }).collect();
                for i in 0..pts.len()-1 {
                    painter.line_segment([pts[i], pts[i+1]], Stroke::new(1.5, line_color));
                }
                painter.line_segment([pts[0], *pts.last().unwrap()], Stroke::new(1.0, dim_color));
            } else {
                painter.circle_stroke(center, r, Stroke::new(1.5, line_color));
            }
            // Radius indicator
            painter.line_segment([center, center + Vec2::new(r, 0.0)], Stroke::new(1.0, dim_color));
            painter.text(center + Vec2::new(r * 0.5, -8.0), egui::Align2::CENTER_BOTTOM,
                format!("r={:.1}", radius), FontId::proportional(9.0), dim_color);
        }
        EmitterShape::Cone { angle, radius } => {
            let half_a = angle.to_radians() * 0.5;
            let h = scale * 0.8;
            let r = (radius * scale * 0.3).min(rect.width() * 0.4);
            let tip = center + Vec2::new(0.0, h * 0.3);
            let left_dir = Vec2::new(-half_a.sin(), -half_a.cos());
            let right_dir = Vec2::new(half_a.sin(), -half_a.cos());
            let left = tip + left_dir * h;
            let right = tip + right_dir * h;
            painter.line_segment([tip, left], Stroke::new(1.5, line_color));
            painter.line_segment([tip, right], Stroke::new(1.5, line_color));
            // Base arc
            let base_y = tip.y - h;
            let pts: Vec<Pos2> = (0..=16).map(|i| {
                let t = i as f32 / 16.0;
                let x = lerp(-r, r, t);
                let spread = half_a * (x / r.max(0.001));
                Pos2::new(tip.x + x, base_y)
            }).collect();
            for i in 0..pts.len()-1 {
                painter.line_segment([pts[i], pts[i+1]], Stroke::new(1.0, line_color));
            }
            // Label angle
            painter.text(tip + Vec2::new(0.0, 12.0), egui::Align2::CENTER_TOP,
                format!("{}°", angle.round()), FontId::proportional(9.0), dim_color);
        }
        EmitterShape::Box { size } => {
            let w = (size[0] * scale * 0.3).min(rect.width() * 0.4);
            let h = (size[1] * scale * 0.3).min(rect.height() * 0.4);
            let r = Rect::from_center_size(center, Vec2::new(w * 2.0, h * 2.0));
            painter.rect_stroke(r, 0.0, Stroke::new(1.5, line_color), egui::StrokeKind::Outside);
            painter.text(center + Vec2::new(0.0, h + 6.0), egui::Align2::CENTER_TOP,
                format!("{:.1}x{:.1}", size[0], size[1]), FontId::proportional(9.0), dim_color);
        }
        EmitterShape::Circle { radius, arc } => {
            let r = (radius * scale * 0.4).min(rect.height() * 0.4);
            let arc_rad = arc.to_radians();
            let pts: Vec<Pos2> = (0..=48).map(|i| {
                let a = i as f32 / 48.0 * arc_rad - std::f32::consts::FRAC_PI_2;
                Pos2::new(center.x + a.cos() * r, center.y + a.sin() * r)
            }).collect();
            for i in 0..pts.len()-1 {
                painter.line_segment([pts[i], pts[i+1]], Stroke::new(1.5, line_color));
            }
            if *arc < 360.0 {
                painter.line_segment([center, pts[0]], Stroke::new(1.0, dim_color));
                painter.line_segment([center, *pts.last().unwrap()], Stroke::new(1.0, dim_color));
            }
        }
        EmitterShape::Edge { length } => {
            let half = (length * scale * 0.25).min(rect.width() * 0.4);
            let left = center - Vec2::new(half, 0.0);
            let right = center + Vec2::new(half, 0.0);
            painter.line_segment([left, right], Stroke::new(2.0, line_color));
            // Endpoints
            painter.circle_filled(left, 3.0, line_color);
            painter.circle_filled(right, 3.0, line_color);
            painter.text(center + Vec2::new(0.0, 10.0), egui::Align2::CENTER_TOP,
                format!("len={:.1}", length), FontId::proportional(9.0), dim_color);
        }
        EmitterShape::Mesh => {
            // Draw a simple triangle as stand-in
            let h = scale * 0.5;
            let pts = [
                center + Vec2::new(0.0, -h),
                center + Vec2::new(-h * 0.8, h * 0.6),
                center + Vec2::new(h * 0.8, h * 0.6),
            ];
            for i in 0..3 {
                painter.line_segment([pts[i], pts[(i+1)%3]], Stroke::new(1.5, line_color));
            }
            painter.text(center, egui::Align2::CENTER_CENTER, "MESH",
                FontId::proportional(10.0), dim_color);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CURVE EDITOR WINDOW
// ─────────────────────────────────────────────────────────────────────────────

fn show_curve_editor_window(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
    // Determine which curve we're editing
    let target = match &editor.editing_curve {
        Some(t) => t.clone(),
        None => { ui.label("No curve selected."); return; }
    };

    // Get mutable reference to curve — extract a copy, edit, put back
    let mut curve_opt = get_curve_copy(editor, &target);

    if let Some(mut curve) = curve_opt {
        // Mode selector
        ui.horizontal(|ui| {
            ui.label("Mode:");
            egui::ComboBox::from_id_source("curve_mode")
                .selected_text(match &curve.mode {
                    CurveMode::Constant => "Constant",
                    CurveMode::Curve => "Curve",
                    CurveMode::RandomBetweenTwoCurves => "Random Between Two",
                })
                .show_ui(ui, |ui| {
                    let prev = curve.mode.clone();
                    ui.selectable_value(&mut curve.mode, CurveMode::Constant, "Constant");
                    ui.selectable_value(&mut curve.mode, CurveMode::Curve, "Curve");
                    ui.selectable_value(&mut curve.mode, CurveMode::RandomBetweenTwoCurves, "Random Between Two");
                });
            ui.label("Multiplier:");
            ui.add(egui::DragValue::new(&mut curve.multiplier).speed(0.01));
        });

        match curve.mode {
            CurveMode::Constant => {
                let val = curve.keys.first().map(|k| k.value).unwrap_or(0.0);
                let mut v = val;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01)).changed() {
                    for k in &mut curve.keys { k.value = v; }
                }
            }
            CurveMode::Curve => {
                let changed = show_curve_canvas(ui, &mut curve.keys, &mut editor.curve_editor_state, false);
                show_keyframe_list(ui, &mut curve.keys, &mut editor.curve_editor_state);
            }
            CurveMode::RandomBetweenTwoCurves => {
                ui.label("Curve A:");
                show_curve_canvas_dual(ui, &mut curve.keys, &mut curve.keys2, &mut editor.curve_editor_state);
                ui.label("Curve A Keyframes:");
                show_keyframe_list(ui, &mut curve.keys, &mut editor.curve_editor_state);
            }
        }

        set_curve_from_copy(editor, &target, curve);
    } else {
        ui.label("Curve not found for this target.");
    }
}

fn show_curve_canvas(ui: &mut egui::Ui, keys: &mut Vec<CurveKey>, state: &mut CurveEditorState, _dual: bool) -> bool {
    let desired = Vec2::new(ui.available_width(), 200.0);
    let (resp, painter) = ui.allocate_painter(desired, egui::Sense::click_and_drag());
    let rect = resp.rect;

    // Background
    painter.rect_filled(rect, 2.0, Color32::from_rgb(22, 22, 28));

    // Grid
    draw_curve_grid(&painter, rect, state.view_min, state.view_max);

    // Draw curve
    draw_curve_path(&painter, rect, keys, Color32::from_rgb(80, 200, 100), state.view_min, state.view_max);

    // Draw control points
    let cp_radius = 5.0;
    let mut changed = false;

    for (i, key) in keys.iter().enumerate() {
        let p = curve_to_screen(rect, key.time, key.value, state.view_min, state.view_max);
        let selected = state.selected_key == Some(i);
        let col = if selected { Color32::from_rgb(255, 200, 50) } else { Color32::from_rgb(200, 200, 200) };
        painter.circle_filled(p, cp_radius, col);
        painter.circle_stroke(p, cp_radius, Stroke::new(1.0, Color32::WHITE));
    }

    // Click handling
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            // Check if near existing key
            let mut near = None;
            for (i, key) in keys.iter().enumerate() {
                let p = curve_to_screen(rect, key.time, key.value, state.view_min, state.view_max);
                if (pos - p).length() < 10.0 { near = Some(i); break; }
            }
            if let Some(i) = near {
                state.selected_key = Some(i);
            } else {
                // Add new key
                let (t, v) = screen_to_curve(rect, pos, state.view_min, state.view_max);
                let t = t.clamp(0.0, 1.0);
                let new_key = CurveKey::new(t, v);
                keys.push(new_key);
                keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                changed = true;
            }
        }
    }

    // Drag handling
    if resp.dragged() {
        if let Some(idx) = state.selected_key {
            if let Some(pos) = resp.interact_pointer_pos() {
                let (t, v) = screen_to_curve(rect, pos, state.view_min, state.view_max);
                let last_idx = keys.len().saturating_sub(1);
                if let Some(key) = keys.get_mut(idx) {
                    // Don't move first/last key's time beyond 0/1
                    key.time = if idx == 0 { 0.0 } else if idx == last_idx { 1.0 } else { t.clamp(0.001, 0.999) };
                    key.value = v;
                }
                changed = true;
            }
        }
    }

    // Right-click context menu
    if resp.secondary_clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            for (i, key) in keys.iter().enumerate() {
                let p = curve_to_screen(rect, key.time, key.value, state.view_min, state.view_max);
                if (pos - p).length() < 10.0 {
                    state.right_click_key = Some(i);
                    break;
                }
            }
        }
    }

    if let Some(right_key) = state.right_click_key {
        let id = egui::Id::new("curve_ctx_menu");
        egui::old_popup::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
            ui.label("Interpolation:");
            if right_key < keys.len() {
                if ui.button("Linear").clicked() { keys[right_key].interpolation = Interpolation::Linear; state.right_click_key = None; }
                if ui.button("Smooth").clicked() { keys[right_key].interpolation = Interpolation::Smooth; state.right_click_key = None; }
                if ui.button("Constant").clicked() { keys[right_key].interpolation = Interpolation::Constant; state.right_click_key = None; }
                if ui.button("Bezier").clicked() { keys[right_key].interpolation = Interpolation::Bezier; state.right_click_key = None; }
                ui.separator();
                if ui.button("Delete Key").clicked() {
                    if keys.len() > 2 {
                        keys.remove(right_key);
                    }
                    state.right_click_key = None;
                    state.selected_key = None;
                }
            }
        });
        if resp.clicked_elsewhere() { state.right_click_key = None; }
    }

    // Value range adjustment
    ui.horizontal(|ui| {
        ui.label("Y range:");
        ui.add(egui::DragValue::new(&mut state.view_min).speed(0.01).prefix("min:"));
        ui.add(egui::DragValue::new(&mut state.view_max).speed(0.01).prefix("max:"));
    });

    changed
}

fn show_curve_canvas_dual(ui: &mut egui::Ui, keys: &mut Vec<CurveKey>, keys2: &mut Vec<CurveKey>, state: &mut CurveEditorState) {
    let desired = Vec2::new(ui.available_width(), 200.0);
    let (resp, painter) = ui.allocate_painter(desired, egui::Sense::click_and_drag());
    let rect = resp.rect;

    painter.rect_filled(rect, 2.0, Color32::from_rgb(22, 22, 28));
    draw_curve_grid(&painter, rect, state.view_min, state.view_max);

    // Draw filled area between curves
    let steps = 64;
    let mut top_pts: Vec<Pos2> = Vec::with_capacity(steps+1);
    let mut bot_pts: Vec<Pos2> = Vec::with_capacity(steps+1);
    let temp_curve_a = Curve { keys: keys.clone(), keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 };
    let temp_curve_b = Curve { keys: keys2.clone(), keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 };

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let va = temp_curve_a.evaluate(t);
        let vb = temp_curve_b.evaluate(t);
        top_pts.push(curve_to_screen(rect, t, va.max(vb), state.view_min, state.view_max));
        bot_pts.push(curve_to_screen(rect, t, va.min(vb), state.view_min, state.view_max));
    }

    draw_curve_path(&painter, rect, keys, Color32::from_rgb(80, 200, 100), state.view_min, state.view_max);
    draw_curve_path(&painter, rect, keys2, Color32::from_rgb(100, 150, 255), state.view_min, state.view_max);

    // Keypoints for curve A
    for key in keys.iter() {
        let p = curve_to_screen(rect, key.time, key.value, state.view_min, state.view_max);
        painter.circle_filled(p, 5.0, Color32::from_rgb(80, 200, 100));
    }
    for key in keys2.iter() {
        let p = curve_to_screen(rect, key.time, key.value, state.view_min, state.view_max);
        painter.circle_filled(p, 5.0, Color32::from_rgb(100, 150, 255));
    }
}

fn draw_curve_grid(painter: &Painter, rect: Rect, view_min: f32, view_max: f32) {
    let grid_col = Color32::from_rgb(40, 40, 50);
    let label_col = Color32::from_rgb(100, 100, 120);

    // Vertical lines at 0, 0.25, 0.5, 0.75, 1.0
    for i in 0..=4 {
        let t = i as f32 / 4.0;
        let x = lerp(rect.min.x, rect.max.x, t);
        painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            Stroke::new(1.0, grid_col));
        painter.text(Pos2::new(x, rect.max.y - 2.0), egui::Align2::CENTER_BOTTOM,
            format!("{:.2}", t), FontId::proportional(9.0), label_col);
    }

    // Horizontal lines
    let range = view_max - view_min;
    if range < 1e-6 { return; }
    let steps = 4;
    for i in 0..=steps {
        let v = lerp(view_min, view_max, i as f32 / steps as f32);
        let y = value_to_screen_y(rect, v, view_min, view_max);
        painter.line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            Stroke::new(1.0, grid_col));
        painter.text(Pos2::new(rect.min.x + 2.0, y), egui::Align2::LEFT_CENTER,
            format!("{:.2}", v), FontId::proportional(9.0), label_col);
    }

    // Zero line
    if view_min < 0.0 && view_max > 0.0 {
        let y0 = value_to_screen_y(rect, 0.0, view_min, view_max);
        painter.line_segment([Pos2::new(rect.min.x, y0), Pos2::new(rect.max.x, y0)],
            Stroke::new(1.0, Color32::from_rgb(60, 80, 60)));
    }
}

fn draw_curve_path(painter: &Painter, rect: Rect, keys: &[CurveKey], color: Color32, view_min: f32, view_max: f32) {
    if keys.len() < 2 { return; }
    let temp = Curve { keys: keys.to_vec(), keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 };
    let steps = 128;
    let mut pts: Vec<Pos2> = Vec::with_capacity(steps+1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let v = temp.evaluate(t);
        pts.push(curve_to_screen(rect, t, v, view_min, view_max));
    }
    for i in 0..pts.len()-1 {
        painter.line_segment([pts[i], pts[i+1]], Stroke::new(2.0, color));
    }
}

fn show_keyframe_list(ui: &mut egui::Ui, keys: &mut Vec<CurveKey>, state: &mut CurveEditorState) {
    if keys.is_empty() { return; }

    egui::CollapsingHeader::new("Keyframes").default_open(false).show(ui, |ui| {
        let mut to_remove = None;
        let can_delete = keys.len() > 2;
        for (i, key) in keys.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("#{}", i));
                ui.add(egui::DragValue::new(&mut key.time).speed(0.001).range(0.0..=1.0).prefix("t:"));
                ui.add(egui::DragValue::new(&mut key.value).speed(0.01).prefix("v:"));
                ui.add(egui::DragValue::new(&mut key.in_tangent).speed(0.01).prefix("in:"));
                ui.add(egui::DragValue::new(&mut key.out_tangent).speed(0.01).prefix("out:"));
                let interp_str = match key.interpolation {
                    Interpolation::Linear => "Lin",
                    Interpolation::Smooth => "Smo",
                    Interpolation::Constant => "Con",
                    Interpolation::Bezier => "Bez",
                };
                egui::ComboBox::from_id_source(format!("interp_{}", i))
                    .selected_text(interp_str)
                    .width(50.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut key.interpolation, Interpolation::Linear, "Linear");
                        ui.selectable_value(&mut key.interpolation, Interpolation::Smooth, "Smooth");
                        ui.selectable_value(&mut key.interpolation, Interpolation::Constant, "Constant");
                        ui.selectable_value(&mut key.interpolation, Interpolation::Bezier, "Bezier");
                    });
                if can_delete {
                    if ui.small_button("✕").clicked() { to_remove = Some(i); }
                }
            });
        }
        if let Some(i) = to_remove { keys.remove(i); }
        keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    });
}

// Curve coordinate helpers
fn curve_to_screen(rect: Rect, t: f32, value: f32, view_min: f32, view_max: f32) -> Pos2 {
    let x = rect.min.x + t * rect.width();
    let y = value_to_screen_y(rect, value, view_min, view_max);
    Pos2::new(x, y)
}

fn screen_to_curve(rect: Rect, pos: Pos2, view_min: f32, view_max: f32) -> (f32, f32) {
    let t = (pos.x - rect.min.x) / rect.width();
    let v = view_max - (pos.y - rect.min.y) / rect.height() * (view_max - view_min);
    (t, v)
}

fn value_to_screen_y(rect: Rect, value: f32, view_min: f32, view_max: f32) -> f32 {
    let range = view_max - view_min;
    if range < 1e-6 { return rect.center().y; }
    rect.max.y - (value - view_min) / range * rect.height()
}

fn draw_curve_mini(painter: &Painter, rect: Rect, curve: &Curve) {
    painter.rect_filled(rect, 1.0, Color32::from_rgb(30, 30, 35));
    if curve.keys.len() < 2 { return; }

    let view_min = curve.keys.iter().map(|k| k.value).fold(f32::INFINITY, f32::min) - 0.05;
    let view_max = curve.keys.iter().map(|k| k.value).fold(f32::NEG_INFINITY, f32::max) + 0.05;
    let view_min = view_min.min(-0.05);
    let view_max = view_max.max(0.05);

    let steps = 32;
    let mut pts: Vec<Pos2> = Vec::with_capacity(steps+1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let v = curve.evaluate(t);
        pts.push(curve_to_screen(rect, t, v, view_min, view_max));
    }
    for i in 0..pts.len()-1 {
        painter.line_segment([pts[i], pts[i+1]], Stroke::new(1.0, Color32::from_rgb(80, 200, 100)));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CURVE GET/SET HELPERS
// ─────────────────────────────────────────────────────────────────────────────

fn get_curve_copy(editor: &ParticleEditor, target: &CurveTarget) -> Option<Curve> {
    let sys = &editor.systems[editor.active_system];
    match target {
        CurveTarget::EmitterStartLifetime => {
            if let RangeOrCurve::Curve(c) = &sys.emitter.start_lifetime { return Some(c.clone()); }
            Some(Curve::default())
        }
        CurveTarget::EmitterStartSpeed => {
            if let RangeOrCurve::Curve(c) = &sys.emitter.start_speed { return Some(c.clone()); }
            Some(Curve::default())
        }
        CurveTarget::EmitterStartSize => {
            if let RangeOrCurve::Curve(c) = &sys.emitter.start_size { return Some(c.clone()); }
            Some(Curve::default())
        }
        CurveTarget::EmitterStartRotation => {
            if let RangeOrCurve::Curve(c) = &sys.emitter.start_rotation { return Some(c.clone()); }
            Some(Curve::default())
        }
        CurveTarget::ModuleCurveMain(idx) => get_module_curve_main(sys, *idx),
        CurveTarget::ModuleCurveX(idx) => get_module_curve_axis(sys, *idx, 0),
        CurveTarget::ModuleCurveY(idx) => get_module_curve_axis(sys, *idx, 1),
        CurveTarget::ModuleCurveZ(idx) => get_module_curve_axis(sys, *idx, 2),
        CurveTarget::ModuleCurveRange(idx) => Some(Curve::default()),
        CurveTarget::ModuleCurveLights(idx, which) => get_module_curve_lights(sys, *idx, *which),
        CurveTarget::ModuleCurveTrailWidth(idx) => get_module_curve_trail_width(sys, *idx),
        CurveTarget::ModuleCurveFrameOverTime(idx) => get_module_curve_frame(sys, *idx),
    }
}

fn set_curve_from_copy(editor: &mut ParticleEditor, target: &CurveTarget, curve: Curve) {
    let sys = &mut editor.systems[editor.active_system];
    match target {
        CurveTarget::EmitterStartLifetime => {
            sys.emitter.start_lifetime = RangeOrCurve::Curve(curve);
        }
        CurveTarget::EmitterStartSpeed => {
            sys.emitter.start_speed = RangeOrCurve::Curve(curve);
        }
        CurveTarget::EmitterStartSize => {
            sys.emitter.start_size = RangeOrCurve::Curve(curve);
        }
        CurveTarget::EmitterStartRotation => {
            sys.emitter.start_rotation = RangeOrCurve::Curve(curve);
        }
        CurveTarget::ModuleCurveMain(idx) => set_module_curve_main(sys, *idx, curve),
        CurveTarget::ModuleCurveX(idx) => set_module_curve_axis(sys, *idx, 0, curve),
        CurveTarget::ModuleCurveY(idx) => set_module_curve_axis(sys, *idx, 1, curve),
        CurveTarget::ModuleCurveZ(idx) => set_module_curve_axis(sys, *idx, 2, curve),
        CurveTarget::ModuleCurveLights(idx, which) => set_module_curve_lights(sys, *idx, *which, curve),
        CurveTarget::ModuleCurveTrailWidth(idx) => set_module_curve_trail_width(sys, *idx, curve),
        CurveTarget::ModuleCurveFrameOverTime(idx) => set_module_curve_frame(sys, *idx, curve),
        _ => {}
    }
}

fn get_module_curve_main(sys: &ParticleSystem, idx: usize) -> Option<Curve> {
    match sys.modules.get(idx)? {
        ParticleModule::LimitVelocityOverLifetime { speed, .. } => Some(speed.clone()),
        ParticleModule::SizeOverLifetime { size, .. } => Some(size.clone()),
        ParticleModule::SizeBySpeed { size, .. } => Some(size.clone()),
        ParticleModule::RotationOverLifetime { angular_velocity, .. } => Some(angular_velocity.clone()),
        ParticleModule::RotationBySpeed { angular_velocity, .. } => Some(angular_velocity.clone()),
        ParticleModule::Noise { remap: Some(r), .. } => Some(r.clone()),
        _ => Some(Curve::default()),
    }
}

fn set_module_curve_main(sys: &mut ParticleSystem, idx: usize, curve: Curve) {
    if let Some(m) = sys.modules.get_mut(idx) {
        match m {
            ParticleModule::LimitVelocityOverLifetime { speed, .. } => *speed = curve,
            ParticleModule::SizeOverLifetime { size, .. } => *size = curve,
            ParticleModule::SizeBySpeed { size, .. } => *size = curve,
            ParticleModule::RotationOverLifetime { angular_velocity, .. } => *angular_velocity = curve,
            ParticleModule::RotationBySpeed { angular_velocity, .. } => *angular_velocity = curve,
            ParticleModule::Noise { remap, .. } => *remap = Some(curve),
            _ => {}
        }
    }
}

fn get_module_curve_axis(sys: &ParticleSystem, idx: usize, axis: u8) -> Option<Curve> {
    match sys.modules.get(idx)? {
        ParticleModule::VelocityOverLifetime { x, y, z, .. } => {
            Some(match axis { 0 => x.clone(), 1 => y.clone(), _ => z.clone() })
        }
        ParticleModule::ForceOverLifetime { x, y, z, .. } => {
            Some(match axis { 0 => x.clone(), 1 => y.clone(), _ => z.clone() })
        }
        _ => Some(Curve::default()),
    }
}

fn set_module_curve_axis(sys: &mut ParticleSystem, idx: usize, axis: u8, curve: Curve) {
    if let Some(m) = sys.modules.get_mut(idx) {
        match m {
            ParticleModule::VelocityOverLifetime { x, y, z, .. } => {
                match axis { 0 => *x = curve, 1 => *y = curve, _ => *z = curve }
            }
            ParticleModule::ForceOverLifetime { x, y, z, .. } => {
                match axis { 0 => *x = curve, 1 => *y = curve, _ => *z = curve }
            }
            _ => {}
        }
    }
}

fn get_module_curve_lights(sys: &ParticleSystem, idx: usize, which: u8) -> Option<Curve> {
    if let Some(ParticleModule::Lights { intensity, range, .. }) = sys.modules.get(idx) {
        Some(if which == 0 { intensity.clone() } else { range.clone() })
    } else { None }
}

fn set_module_curve_lights(sys: &mut ParticleSystem, idx: usize, which: u8, curve: Curve) {
    if let Some(ParticleModule::Lights { intensity, range, .. }) = sys.modules.get_mut(idx) {
        if which == 0 { *intensity = curve; } else { *range = curve; }
    }
}

fn get_module_curve_trail_width(sys: &ParticleSystem, idx: usize) -> Option<Curve> {
    if let Some(ParticleModule::Trails { width, .. }) = sys.modules.get(idx) {
        Some(width.clone())
    } else { None }
}

fn set_module_curve_trail_width(sys: &mut ParticleSystem, idx: usize, curve: Curve) {
    if let Some(ParticleModule::Trails { width, .. }) = sys.modules.get_mut(idx) {
        *width = curve;
    }
}

fn get_module_curve_frame(sys: &ParticleSystem, idx: usize) -> Option<Curve> {
    if let Some(ParticleModule::TextureSheetAnimation { frame_over_time, .. }) = sys.modules.get(idx) {
        Some(frame_over_time.clone())
    } else { None }
}

fn set_module_curve_frame(sys: &mut ParticleSystem, idx: usize, curve: Curve) {
    if let Some(ParticleModule::TextureSheetAnimation { frame_over_time, .. }) = sys.modules.get_mut(idx) {
        *frame_over_time = curve;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GRADIENT EDITOR WINDOW
// ─────────────────────────────────────────────────────────────────────────────

fn show_gradient_editor_window(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
    let target = match &editor.editing_gradient {
        Some(t) => t.clone(),
        None => { ui.label("No gradient selected."); return; }
    };

    let mut grad = get_gradient_copy(editor, &target);

    // Mode
    ui.horizontal(|ui| {
        ui.label("Mode:");
        egui::ComboBox::from_id_source("grad_mode")
            .selected_text(match &grad.mode { GradientMode::Blend => "Blend", GradientMode::Fixed => "Fixed" })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut grad.mode, GradientMode::Blend, "Blend");
                ui.selectable_value(&mut grad.mode, GradientMode::Fixed, "Fixed");
            });
    });

    // Preview bar
    let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), 32.0), egui::Sense::click());
    let bar_rect = resp.rect;
    draw_gradient_bar(&painter, bar_rect, &grad);

    // Color stops below bar
    let bar_y_bot = bar_rect.max.y;
    let bar_y_top = bar_rect.min.y;

    // Draw color key markers below bar
    for (i, ck) in grad.color_keys.iter().enumerate() {
        let x = bar_rect.min.x + ck.time * bar_rect.width();
        let marker_pos = Pos2::new(x, bar_y_bot + 2.0);
        let sel = editor.gradient_editor_state.selected_color_key == Some(i);
        let c = Color32::from_rgb((ck.color[0]*255.0) as u8, (ck.color[1]*255.0) as u8, (ck.color[2]*255.0) as u8);
        painter.rect_filled(Rect::from_center_size(marker_pos + Vec2::new(0.0, 6.0), Vec2::splat(10.0)), 1.0, c);
        painter.rect_stroke(Rect::from_center_size(marker_pos + Vec2::new(0.0, 6.0), Vec2::splat(10.0)), 1.0,
            Stroke::new(if sel { 2.0 } else { 1.0 }, Color32::WHITE), egui::StrokeKind::Outside);
        // Triangle
        let tri = vec![
            marker_pos,
            marker_pos + Vec2::new(-5.0, 5.0),
            marker_pos + Vec2::new(5.0, 5.0),
        ];
        painter.add(Shape::convex_polygon(tri, c, Stroke::NONE));
    }

    // Draw alpha key markers above bar
    for (i, ak) in grad.alpha_keys.iter().enumerate() {
        let x = bar_rect.min.x + ak.time * bar_rect.width();
        let marker_pos = Pos2::new(x, bar_y_top - 2.0);
        let sel = editor.gradient_editor_state.selected_alpha_key == Some(i);
        let a_col = Color32::from_gray((ak.alpha * 255.0) as u8);
        painter.rect_filled(Rect::from_center_size(marker_pos - Vec2::new(0.0, 6.0), Vec2::splat(10.0)), 1.0, a_col);
        painter.rect_stroke(Rect::from_center_size(marker_pos - Vec2::new(0.0, 6.0), Vec2::splat(10.0)), 1.0,
            Stroke::new(if sel { 2.0 } else { 1.0 }, Color32::WHITE), egui::StrokeKind::Outside);
    }

    // Click on bar area to add/select stops
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((pos.x - bar_rect.min.x) / bar_rect.width()).clamp(0.0, 1.0);
            // If clicking in lower region, add/select color key
            if pos.y > bar_rect.center().y {
                // Check if near existing
                let near = grad.color_keys.iter().enumerate().find(|(_, ck)| {
                    let kx = bar_rect.min.x + ck.time * bar_rect.width();
                    (pos.x - kx).abs() < 10.0
                }).map(|(i, _)| i);
                if let Some(i) = near {
                    editor.gradient_editor_state.selected_color_key = Some(i);
                    editor.gradient_editor_state.selected_alpha_key = None;
                    let ck = &grad.color_keys[i];
                    editor.gradient_editor_state.edit_color = ck.color;
                } else {
                    // Add new color key
                    let c = grad.evaluate(t);
                    grad.color_keys.push(GradientColorKey {
                        time: t,
                        color: [c.r() as f32/255.0, c.g() as f32/255.0, c.b() as f32/255.0],
                    });
                    grad.color_keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                }
            } else {
                // Alpha region
                let near = grad.alpha_keys.iter().enumerate().find(|(_, ak)| {
                    let kx = bar_rect.min.x + ak.time * bar_rect.width();
                    (pos.x - kx).abs() < 10.0
                }).map(|(i, _)| i);
                if let Some(i) = near {
                    editor.gradient_editor_state.selected_alpha_key = Some(i);
                    editor.gradient_editor_state.selected_color_key = None;
                } else {
                    let a = grad.evaluate(t).a() as f32 / 255.0;
                    grad.alpha_keys.push(GradientAlphaKey { time: t, alpha: a });
                    grad.alpha_keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                }
            }
        }
    }

    ui.separator();

    // Edit selected color key
    if let Some(sel_ci) = editor.gradient_editor_state.selected_color_key {
        if sel_ci < grad.color_keys.len() {
            ui.label(format!("Color Stop #{}", sel_ci));
            ui.horizontal(|ui| {
                ui.label("Time:");
                ui.add(egui::DragValue::new(&mut grad.color_keys[sel_ci].time)
                    .speed(0.001).range(0.0..=1.0));
            });
            let ck = &mut grad.color_keys[sel_ci];
            let mut col = Color32::from_rgb(
                (ck.color[0]*255.0) as u8, (ck.color[1]*255.0) as u8, (ck.color[2]*255.0) as u8);
            if ui.color_edit_button_srgba(&mut col).changed() {
                ck.color = [col.r() as f32/255.0, col.g() as f32/255.0, col.b() as f32/255.0];
            }
            if grad.color_keys.len() > 2 {
                if ui.small_button("Remove Color Stop").clicked() {
                    grad.color_keys.remove(sel_ci);
                    editor.gradient_editor_state.selected_color_key = None;
                }
            }
        }
    }

    // Edit selected alpha key
    if let Some(sel_ai) = editor.gradient_editor_state.selected_alpha_key {
        if sel_ai < grad.alpha_keys.len() {
            ui.label(format!("Alpha Stop #{}", sel_ai));
            ui.horizontal(|ui| {
                ui.label("Time:");
                ui.add(egui::DragValue::new(&mut grad.alpha_keys[sel_ai].time)
                    .speed(0.001).range(0.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Alpha:");
                ui.add(egui::DragValue::new(&mut grad.alpha_keys[sel_ai].alpha)
                    .speed(0.001).range(0.0..=1.0));
            });
            if grad.alpha_keys.len() > 2 {
                if ui.small_button("Remove Alpha Stop").clicked() {
                    grad.alpha_keys.remove(sel_ai);
                    editor.gradient_editor_state.selected_alpha_key = None;
                }
            }
        }
    }

    set_gradient_from_copy(editor, &target, grad);
}

fn draw_gradient_bar(painter: &Painter, rect: Rect, gradient: &ColorGradient) {
    let steps = 64;
    let step_w = rect.width() / steps as f32;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i+1) as f32 / steps as f32;
        let c0 = gradient.evaluate(t0);
        let c1 = gradient.evaluate(t1);
        // Simple gradient fill per strip
        let strip_rect = Rect::from_min_max(
            Pos2::new(rect.min.x + t0 * rect.width(), rect.min.y),
            Pos2::new(rect.min.x + t1 * rect.width(), rect.max.y),
        );
        painter.rect_filled(strip_rect, 0.0, c0);
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_rgb(80, 80, 80)), egui::StrokeKind::Outside);
}

fn get_gradient_copy(editor: &ParticleEditor, target: &GradientTarget) -> ColorGradient {
    let sys = &editor.systems[editor.active_system];
    match target {
        GradientTarget::EmitterStartColor => {
            match &sys.emitter.start_color {
                ColorMode::Gradient(g) | ColorMode::RandomFromGradient(g) => g.clone(),
                _ => ColorGradient::default(),
            }
        }
        GradientTarget::ModuleColorMain(idx) => {
            match sys.modules.get(*idx) {
                Some(ParticleModule::ColorOverLifetime { gradient, .. }) => gradient.clone(),
                Some(ParticleModule::ColorBySpeed { gradient, .. }) => gradient.clone(),
                _ => ColorGradient::default(),
            }
        }
        GradientTarget::ModuleTrailColor(idx) => {
            match sys.modules.get(*idx) {
                Some(ParticleModule::Trails { color, .. }) => color.clone(),
                _ => ColorGradient::default(),
            }
        }
        _ => ColorGradient::default(),
    }
}

fn set_gradient_from_copy(editor: &mut ParticleEditor, target: &GradientTarget, grad: ColorGradient) {
    let sys = &mut editor.systems[editor.active_system];
    match target {
        GradientTarget::EmitterStartColor => {
            match &mut sys.emitter.start_color {
                ColorMode::Gradient(g) => *g = grad,
                ColorMode::RandomFromGradient(g) => *g = grad,
                _ => { sys.emitter.start_color = ColorMode::Gradient(grad); }
            }
        }
        GradientTarget::ModuleColorMain(idx) => {
            if let Some(m) = sys.modules.get_mut(*idx) {
                match m {
                    ParticleModule::ColorOverLifetime { gradient, .. } => *gradient = grad,
                    ParticleModule::ColorBySpeed { gradient, .. } => *gradient = grad,
                    _ => {}
                }
            }
        }
        GradientTarget::ModuleTrailColor(idx) => {
            if let Some(ParticleModule::Trails { color, .. }) = sys.modules.get_mut(*idx) {
                *color = grad;
            }
        }
        _ => {}
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PRESETS
// ─────────────────────────────────────────────────────────────────────────────

fn apply_preset(editor: &mut ParticleEditor, name: &str) {
    let sys = match name {
        "Fire" => preset_fire(),
        "Smoke" => preset_smoke(),
        "Sparks" => preset_sparks(),
        "Magic Dust" => preset_magic_dust(),
        "Rain" => preset_rain(),
        "Snow" => preset_snow(),
        "Explosion" => preset_explosion(),
        "Level Up" => preset_level_up(),
        "Heal" => preset_heal(),
        "Poison" => preset_poison(),
        "Blood" => preset_blood(),
        "Shockwave" => preset_shockwave(),
        "Portal" => preset_portal(),
        "Stars" => preset_stars(),
        "Confetti" => preset_confetti(),
        _ => ParticleSystem::default(),
    };
    let mod_count = sys.modules.len();
    editor.systems[editor.active_system] = sys;
    editor.module_expanded = vec![false; mod_count];
    editor.selected_module = None;
    editor.particles.clear();
    editor.emission_accumulator = 0.0;
    editor.preview_time = 0.0;
}

fn preset_fire() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Fire".to_string();
    sys.emitter.emission_rate = 40.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(0.8, 1.8);
    sys.emitter.start_speed = RangeOrCurve::Random(1.5, 3.5);
    sys.emitter.start_size = RangeOrCurve::Random(0.5, 1.2);
    sys.emitter.start_rotation = RangeOrCurve::Random(-15.0, 15.0);
    sys.emitter.shape = EmitterShape::Cone { angle: 15.0, radius: 0.3 };
    sys.emitter.gravity_modifier = -0.2;
    sys.emitter.start_color = ColorMode::Solid([1.0, 0.9, 0.1, 1.0]);
    sys.max_particles = 300;
    sys.duration = 5.0;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime { enabled: true, gradient: ColorGradient::fire() },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        ParticleModule::VelocityOverLifetime {
            enabled: true,
            x: Curve::constant(0.0),
            y: Curve::constant(0.5),
            z: Curve::constant(0.0),
            space: SimulationSpace::Local,
        },
        ParticleModule::Noise {
            enabled: true,
            strength: 0.8,
            frequency: 1.5,
            scroll_speed: 0.5,
            damping: true,
            octaves: 2,
            remap: None,
        },
        default_renderer(),
    ];
    sys
}

fn preset_smoke() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Smoke".to_string();
    sys.emitter.emission_rate = 8.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(3.0, 6.0);
    sys.emitter.start_speed = RangeOrCurve::Random(0.3, 0.8);
    sys.emitter.start_size = RangeOrCurve::Random(0.8, 1.5);
    sys.emitter.start_rotation = RangeOrCurve::Random(0.0, 360.0);
    sys.emitter.shape = EmitterShape::Cone { angle: 10.0, radius: 0.2 };
    sys.emitter.gravity_modifier = -0.05;
    sys.emitter.start_color = ColorMode::Solid([0.5, 0.5, 0.5, 0.8]);
    sys.max_particles = 100;
    sys.duration = 5.0;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime { enabled: true, gradient: ColorGradient::smoke() },
        ParticleModule::SizeOverLifetime {
            enabled: true,
            size: Curve { keys: vec![CurveKey::new(0.0, 0.5), CurveKey::new(1.0, 1.5)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 },
        },
        ParticleModule::RotationOverLifetime { enabled: true, angular_velocity: Curve::constant(15.0) },
        default_renderer(),
    ];
    sys
}

fn preset_sparks() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Sparks".to_string();
    sys.emitter.emission_rate = 0.0;
    sys.emitter.emission_bursts = vec![Burst { time: 0.0, count: 80, repeat_interval: 2.0, repeat_count: -1 }];
    sys.emitter.start_lifetime = RangeOrCurve::Random(0.5, 1.5);
    sys.emitter.start_speed = RangeOrCurve::Random(3.0, 8.0);
    sys.emitter.start_size = RangeOrCurve::Constant(0.1);
    sys.emitter.shape = EmitterShape::Sphere { radius: 0.1, hemisphere: false };
    sys.emitter.gravity_modifier = 1.0;
    sys.emitter.start_color = ColorMode::Solid([1.0, 0.8, 0.2, 1.0]);
    sys.max_particles = 400;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime {
            enabled: true,
            gradient: ColorGradient {
                color_keys: vec![
                    GradientColorKey { time: 0.0, color: [1.0, 0.9, 0.5] },
                    GradientColorKey { time: 1.0, color: [0.8, 0.2, 0.0] },
                ],
                alpha_keys: vec![
                    GradientAlphaKey { time: 0.0, alpha: 1.0 },
                    GradientAlphaKey { time: 1.0, alpha: 0.0 },
                ],
                mode: GradientMode::Blend,
            },
        },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        default_renderer(),
    ];
    sys
}

fn preset_magic_dust() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Magic Dust".to_string();
    sys.emitter.emission_rate = 25.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(1.0, 2.5);
    sys.emitter.start_speed = RangeOrCurve::Random(0.5, 2.0);
    sys.emitter.start_size = RangeOrCurve::Random(0.1, 0.4);
    sys.emitter.shape = EmitterShape::Sphere { radius: 0.5, hemisphere: false };
    sys.emitter.gravity_modifier = -0.1;
    sys.emitter.start_color = ColorMode::RandomBetweenTwo([0.6, 0.2, 1.0, 1.0], [0.2, 0.6, 1.0, 1.0]);
    sys.max_particles = 200;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime { enabled: true, gradient: ColorGradient::default() },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        ParticleModule::RotationOverLifetime { enabled: true, angular_velocity: Curve::constant(90.0) },
        ParticleModule::Noise { enabled: true, strength: 0.5, frequency: 1.0, scroll_speed: 0.2, damping: true, octaves: 2, remap: None },
        default_renderer(),
    ];
    sys
}

fn preset_rain() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Rain".to_string();
    sys.emitter.emission_rate = 200.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(0.4, 0.8);
    sys.emitter.start_speed = RangeOrCurve::Random(8.0, 12.0);
    sys.emitter.start_size = RangeOrCurve::Constant(0.05);
    sys.emitter.shape = EmitterShape::Box { size: [10.0, 0.1, 1.0] };
    sys.emitter.gravity_modifier = 1.0;
    sys.emitter.start_color = ColorMode::Solid([0.6, 0.7, 1.0, 0.7]);
    sys.max_particles = 1000;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::constant(1.0) },
        default_renderer(),
    ];
    sys
}

fn preset_snow() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Snow".to_string();
    sys.emitter.emission_rate = 30.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(4.0, 8.0);
    sys.emitter.start_speed = RangeOrCurve::Random(0.3, 1.0);
    sys.emitter.start_size = RangeOrCurve::Random(0.1, 0.3);
    sys.emitter.shape = EmitterShape::Box { size: [10.0, 0.1, 1.0] };
    sys.emitter.gravity_modifier = 0.1;
    sys.emitter.start_color = ColorMode::Solid([0.9, 0.95, 1.0, 0.9]);
    sys.max_particles = 500;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::Noise { enabled: true, strength: 0.3, frequency: 0.5, scroll_speed: 0.1, damping: false, octaves: 1, remap: None },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::constant(1.0) },
        default_renderer(),
    ];
    sys
}

fn preset_explosion() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Explosion".to_string();
    sys.emitter.emission_rate = 0.0;
    sys.emitter.emission_bursts = vec![
        Burst { time: 0.0, count: 150, repeat_interval: 100.0, repeat_count: 1 },
    ];
    sys.emitter.start_lifetime = RangeOrCurve::Random(0.5, 1.5);
    sys.emitter.start_speed = RangeOrCurve::Random(5.0, 15.0);
    sys.emitter.start_size = RangeOrCurve::Random(0.2, 0.8);
    sys.emitter.shape = EmitterShape::Sphere { radius: 0.1, hemisphere: false };
    sys.emitter.gravity_modifier = 0.5;
    sys.emitter.start_color = ColorMode::Solid([1.0, 0.7, 0.1, 1.0]);
    sys.max_particles = 500;
    sys.looping = false;
    sys.duration = 2.0;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime { enabled: true, gradient: ColorGradient::fire() },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        default_renderer(),
    ];
    sys
}

fn preset_level_up() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Level Up".to_string();
    sys.emitter.emission_rate = 0.0;
    sys.emitter.emission_bursts = vec![
        Burst { time: 0.0, count: 60, repeat_interval: 100.0, repeat_count: 1 },
    ];
    sys.emitter.start_lifetime = RangeOrCurve::Random(1.0, 2.0);
    sys.emitter.start_speed = RangeOrCurve::Random(2.0, 5.0);
    sys.emitter.start_size = RangeOrCurve::Random(0.2, 0.5);
    sys.emitter.shape = EmitterShape::Circle { radius: 1.0, arc: 360.0 };
    sys.emitter.gravity_modifier = -0.5;
    sys.emitter.start_color = ColorMode::RandomBetweenTwo([1.0, 0.9, 0.0, 1.0], [0.5, 1.0, 0.0, 1.0]);
    sys.max_particles = 200;
    sys.looping = false;
    sys.modules = vec![
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        default_renderer(),
    ];
    sys
}

fn preset_heal() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Heal".to_string();
    sys.emitter.emission_rate = 15.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(1.0, 2.5);
    sys.emitter.start_speed = RangeOrCurve::Random(0.5, 1.5);
    sys.emitter.start_size = RangeOrCurve::Random(0.15, 0.4);
    sys.emitter.shape = EmitterShape::Sphere { radius: 0.5, hemisphere: false };
    sys.emitter.gravity_modifier = -0.3;
    sys.emitter.start_color = ColorMode::Solid([0.2, 1.0, 0.3, 1.0]);
    sys.max_particles = 100;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime {
            enabled: true,
            gradient: ColorGradient {
                color_keys: vec![
                    GradientColorKey { time: 0.0, color: [0.3, 1.0, 0.3] },
                    GradientColorKey { time: 1.0, color: [0.5, 1.0, 0.5] },
                ],
                alpha_keys: vec![
                    GradientAlphaKey { time: 0.0, alpha: 1.0 },
                    GradientAlphaKey { time: 1.0, alpha: 0.0 },
                ],
                mode: GradientMode::Blend,
            },
        },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        default_renderer(),
    ];
    sys
}

fn preset_poison() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Poison".to_string();
    sys.emitter.emission_rate = 20.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(1.5, 3.0);
    sys.emitter.start_speed = RangeOrCurve::Random(0.3, 1.0);
    sys.emitter.start_size = RangeOrCurve::Random(0.1, 0.35);
    sys.emitter.shape = EmitterShape::Sphere { radius: 0.4, hemisphere: true };
    sys.emitter.gravity_modifier = 0.05;
    sys.emitter.start_color = ColorMode::Solid([0.3, 0.8, 0.1, 0.9]);
    sys.max_particles = 150;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime {
            enabled: true,
            gradient: ColorGradient {
                color_keys: vec![
                    GradientColorKey { time: 0.0, color: [0.5, 1.0, 0.0] },
                    GradientColorKey { time: 0.5, color: [0.3, 0.6, 0.0] },
                    GradientColorKey { time: 1.0, color: [0.1, 0.3, 0.0] },
                ],
                alpha_keys: vec![
                    GradientAlphaKey { time: 0.0, alpha: 0.8 },
                    GradientAlphaKey { time: 1.0, alpha: 0.0 },
                ],
                mode: GradientMode::Blend,
            },
        },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        ParticleModule::Noise { enabled: true, strength: 0.4, frequency: 1.2, scroll_speed: 0.3, damping: true, octaves: 1, remap: None },
        default_renderer(),
    ];
    sys
}

fn preset_blood() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Blood".to_string();
    sys.emitter.emission_rate = 0.0;
    sys.emitter.emission_bursts = vec![Burst { time: 0.0, count: 30, repeat_interval: 100.0, repeat_count: 1 }];
    sys.emitter.start_lifetime = RangeOrCurve::Random(0.3, 0.8);
    sys.emitter.start_speed = RangeOrCurve::Random(2.0, 6.0);
    sys.emitter.start_size = RangeOrCurve::Random(0.05, 0.25);
    sys.emitter.shape = EmitterShape::Cone { angle: 45.0, radius: 0.1 };
    sys.emitter.gravity_modifier = 2.0;
    sys.emitter.start_color = ColorMode::Solid([0.7, 0.0, 0.0, 1.0]);
    sys.max_particles = 100;
    sys.looping = false;
    sys.duration = 1.0;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime {
            enabled: true,
            gradient: ColorGradient {
                color_keys: vec![
                    GradientColorKey { time: 0.0, color: [0.9, 0.0, 0.0] },
                    GradientColorKey { time: 1.0, color: [0.4, 0.0, 0.0] },
                ],
                alpha_keys: vec![
                    GradientAlphaKey { time: 0.0, alpha: 1.0 },
                    GradientAlphaKey { time: 0.8, alpha: 1.0 },
                    GradientAlphaKey { time: 1.0, alpha: 0.0 },
                ],
                mode: GradientMode::Blend,
            },
        },
        default_renderer(),
    ];
    sys
}

fn preset_shockwave() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Shockwave".to_string();
    sys.emitter.emission_rate = 0.0;
    sys.emitter.emission_bursts = vec![Burst { time: 0.0, count: 64, repeat_interval: 100.0, repeat_count: 1 }];
    sys.emitter.start_lifetime = RangeOrCurve::Constant(0.4);
    sys.emitter.start_speed = RangeOrCurve::Random(5.0, 8.0);
    sys.emitter.start_size = RangeOrCurve::Constant(0.3);
    sys.emitter.shape = EmitterShape::Circle { radius: 0.1, arc: 360.0 };
    sys.emitter.gravity_modifier = 0.0;
    sys.emitter.start_color = ColorMode::Solid([1.0, 1.0, 1.0, 1.0]);
    sys.max_particles = 200;
    sys.looping = false;
    sys.duration = 0.5;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime {
            enabled: true,
            gradient: ColorGradient {
                color_keys: vec![
                    GradientColorKey { time: 0.0, color: [1.0, 1.0, 1.0] },
                    GradientColorKey { time: 1.0, color: [0.4, 0.6, 1.0] },
                ],
                alpha_keys: vec![
                    GradientAlphaKey { time: 0.0, alpha: 1.0 },
                    GradientAlphaKey { time: 1.0, alpha: 0.0 },
                ],
                mode: GradientMode::Blend,
            },
        },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        default_renderer(),
    ];
    sys
}

fn preset_portal() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Portal".to_string();
    sys.emitter.emission_rate = 50.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(1.0, 2.0);
    sys.emitter.start_speed = RangeOrCurve::Random(0.5, 1.5);
    sys.emitter.start_size = RangeOrCurve::Random(0.1, 0.3);
    sys.emitter.shape = EmitterShape::Circle { radius: 1.5, arc: 360.0 };
    sys.emitter.gravity_modifier = 0.0;
    sys.emitter.start_color = ColorMode::RandomBetweenTwo([0.3, 0.0, 1.0, 1.0], [0.0, 0.5, 1.0, 1.0]);
    sys.max_particles = 300;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime {
            enabled: true,
            gradient: ColorGradient {
                color_keys: vec![
                    GradientColorKey { time: 0.0, color: [0.5, 0.0, 1.0] },
                    GradientColorKey { time: 0.5, color: [0.0, 0.5, 1.0] },
                    GradientColorKey { time: 1.0, color: [1.0, 0.0, 0.5] },
                ],
                alpha_keys: vec![
                    GradientAlphaKey { time: 0.0, alpha: 1.0 },
                    GradientAlphaKey { time: 1.0, alpha: 0.0 },
                ],
                mode: GradientMode::Blend,
            },
        },
        ParticleModule::SizeOverLifetime { enabled: true, size: Curve::linear_one_to_zero() },
        ParticleModule::VelocityOverLifetime {
            enabled: true,
            x: Curve::constant(0.0),
            y: Curve::constant(0.0),
            z: Curve::constant(0.0),
            space: SimulationSpace::Local,
        },
        default_renderer(),
    ];
    sys
}

fn preset_stars() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Stars".to_string();
    sys.emitter.emission_rate = 5.0;
    sys.emitter.start_lifetime = RangeOrCurve::Random(2.0, 5.0);
    sys.emitter.start_speed = RangeOrCurve::Random(0.1, 0.4);
    sys.emitter.start_size = RangeOrCurve::Random(0.05, 0.2);
    sys.emitter.shape = EmitterShape::Box { size: [8.0, 6.0, 1.0] };
    sys.emitter.gravity_modifier = 0.0;
    sys.emitter.start_color = ColorMode::RandomBetweenTwo([1.0, 1.0, 0.8, 1.0], [0.8, 0.8, 1.0, 1.0]);
    sys.max_particles = 200;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime {
            enabled: true,
            gradient: ColorGradient {
                color_keys: vec![
                    GradientColorKey { time: 0.0, color: [1.0, 1.0, 0.8] },
                    GradientColorKey { time: 0.5, color: [1.0, 1.0, 1.0] },
                    GradientColorKey { time: 1.0, color: [0.8, 0.9, 1.0] },
                ],
                alpha_keys: vec![
                    GradientAlphaKey { time: 0.0, alpha: 0.0 },
                    GradientAlphaKey { time: 0.2, alpha: 1.0 },
                    GradientAlphaKey { time: 0.8, alpha: 1.0 },
                    GradientAlphaKey { time: 1.0, alpha: 0.0 },
                ],
                mode: GradientMode::Blend,
            },
        },
        default_renderer(),
    ];
    sys
}

fn preset_confetti() -> ParticleSystem {
    let mut sys = ParticleSystem::default();
    sys.name = "Confetti".to_string();
    sys.emitter.emission_rate = 0.0;
    sys.emitter.emission_bursts = vec![Burst { time: 0.0, count: 100, repeat_interval: 3.0, repeat_count: -1 }];
    sys.emitter.start_lifetime = RangeOrCurve::Random(2.0, 4.0);
    sys.emitter.start_speed = RangeOrCurve::Random(3.0, 7.0);
    sys.emitter.start_size = RangeOrCurve::Random(0.1, 0.25);
    sys.emitter.start_rotation = RangeOrCurve::Random(0.0, 360.0);
    sys.emitter.shape = EmitterShape::Cone { angle: 45.0, radius: 0.5 };
    sys.emitter.gravity_modifier = 0.8;
    sys.emitter.start_color = ColorMode::RandomBetweenTwo([1.0, 0.2, 0.2, 1.0], [0.2, 0.5, 1.0, 1.0]);
    sys.max_particles = 400;
    sys.looping = true;
    sys.modules = vec![
        ParticleModule::ColorOverLifetime { enabled: true, gradient: ColorGradient::default() },
        ParticleModule::RotationOverLifetime { enabled: true, angular_velocity: Curve::constant(180.0) },
        ParticleModule::SizeOverLifetime {
            enabled: true,
            size: Curve {
                keys: vec![CurveKey::new(0.0, 1.0), CurveKey::new(0.8, 1.0), CurveKey::new(1.0, 0.0)],
                keys2: vec![],
                mode: CurveMode::Curve,
                multiplier: 1.0,
            },
        },
        default_renderer(),
    ];
    sys
}

// ─────────────────────────────────────────────────────────────────────────────
// UTILITY FUNCTIONS
// ─────────────────────────────────────────────────────────────────────────────

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn simple_noise(x: f32, y: f32) -> f32 {
    // Simple value noise approximation
    let xi = x.floor() as i64;
    let yi = y.floor() as i64;
    let xf = x - xi as f32;
    let yf = y - yi as f32;

    let xf = smooth_step(xf);
    let yf = smooth_step(yf);

    let h00 = hash2(xi, yi);
    let h10 = hash2(xi+1, yi);
    let h01 = hash2(xi, yi+1);
    let h11 = hash2(xi+1, yi+1);

    let r = lerp(lerp(h00, h10, xf), lerp(h01, h11, xf), yf);
    r * 2.0 - 1.0
}

fn hash2(x: i64, y: i64) -> f32 {
    let mut h = (x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263)) as u64;
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65535.0
}

fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

// ─────────────────────────────────────────────────────────────────────────────
// STATISTICS PANEL
// ─────────────────────────────────────────────────────────────────────────────

pub struct ParticleStats {
    pub particle_count: usize,
    pub max_particles: u32,
    pub emission_rate: f32,
    pub active_modules: usize,
    pub total_modules: usize,
    pub preview_time: f32,
    pub duration: f32,
    pub looping: bool,
    pub avg_lifetime: f32,
    pub avg_speed: f32,
    pub avg_size: f32,
}

impl ParticleStats {
    pub fn compute(editor: &ParticleEditor) -> Self {
        let sys = &editor.systems[editor.active_system];
        let active_mods = sys.modules.iter().filter(|m| m.is_enabled()).count();
        let total_mods = sys.modules.len();

        let avg_lifetime = if editor.particles.is_empty() {
            0.0
        } else {
            editor.particles.iter().map(|p| p.lifetime).sum::<f32>() / editor.particles.len() as f32
        };

        let avg_speed = if editor.particles.is_empty() {
            0.0
        } else {
            editor.particles.iter()
                .map(|p| (p.velocity[0] * p.velocity[0] + p.velocity[1] * p.velocity[1]).sqrt())
                .sum::<f32>() / editor.particles.len() as f32
        };

        let avg_size = if editor.particles.is_empty() {
            0.0
        } else {
            editor.particles.iter().map(|p| p.size).sum::<f32>() / editor.particles.len() as f32
        };

        ParticleStats {
            particle_count: editor.particles.len(),
            max_particles: sys.max_particles,
            emission_rate: sys.emitter.emission_rate,
            active_modules: active_mods,
            total_modules: total_mods,
            preview_time: editor.preview_time,
            duration: sys.duration,
            looping: sys.looping,
            avg_lifetime,
            avg_speed,
            avg_size,
        }
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Statistics").default_open(false).show(ui, |ui| {
            egui::Grid::new("ps_stats_grid")
                .num_columns(2)
                .striped(true)
                .spacing([8.0, 2.0])
                .show(ui, |ui| {
                    ui.label("Particles:");
                    let fill = if self.max_particles > 0 {
                        self.particle_count as f32 / self.max_particles as f32
                    } else { 0.0 };
                    ui.horizontal(|ui| {
                        ui.label(format!("{}/{}", self.particle_count, self.max_particles));
                        let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 10.0), egui::Sense::hover());
                        let painter = ui.painter();
                        painter.rect_filled(bar_rect, 1.0, Color32::from_rgb(40, 40, 50));
                        let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width() * fill, bar_rect.height()));
                        let bar_color = if fill > 0.9 { Color32::from_rgb(200, 80, 80) }
                            else if fill > 0.7 { Color32::from_rgb(200, 150, 50) }
                            else { Color32::from_rgb(50, 180, 100) };
                        painter.rect_filled(fill_rect, 1.0, bar_color);
                    });
                    ui.end_row();

                    ui.label("Emission Rate:");
                    ui.label(format!("{:.1}/s", self.emission_rate));
                    ui.end_row();

                    ui.label("Active Modules:");
                    ui.label(format!("{}/{}", self.active_modules, self.total_modules));
                    ui.end_row();

                    ui.label("Time:");
                    ui.label(format!("{:.2}/{:.2}s {}", self.preview_time, self.duration,
                        if self.looping { "(loop)" } else { "" }));
                    ui.end_row();

                    ui.label("Avg Lifetime:");
                    ui.label(format!("{:.2}s", self.avg_lifetime));
                    ui.end_row();

                    ui.label("Avg Speed:");
                    ui.label(format!("{:.2}", self.avg_speed));
                    ui.end_row();

                    ui.label("Avg Size:");
                    ui.label(format!("{:.3}", self.avg_size));
                    ui.end_row();
                });
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EXPORT / IMPORT (JSON via serde_json if available, else placeholder)
// ─────────────────────────────────────────────────────────────────────────────

impl ParticleSystem {
    /// Serialize to JSON string. Requires serde_json in Cargo.toml.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Deserialize from JSON string.
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
}

impl ParticleEditor {
    /// Show an import/export panel below the main UI.
    pub fn show_io_panel(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
        egui::CollapsingHeader::new("Export / Import").default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Copy JSON").clicked() {
                    if let Ok(json) = editor.systems[editor.active_system].to_json() {
                        ui.ctx().copy_text(json);
                    }
                }
                if ui.button("Paste JSON").clicked() {
                    // In a real integration, read from clipboard — here we mark intent
                    // clipboard reading is context-dependent in egui
                }
            });
        });
    }

    /// Show the statistics panel.
    pub fn show_stats_panel(ui: &mut egui::Ui, editor: &ParticleEditor) {
        let stats = ParticleStats::compute(editor);
        stats.show(ui);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ADVANCED CURVE EDITOR — Standalone widget callable from any UI
// ─────────────────────────────────────────────────────────────────────────────

/// A self-contained curve editing widget that owns its state.
pub struct CurveEditorWidget {
    pub state: CurveEditorState,
    pub label: String,
    pub y_min: f32,
    pub y_max: f32,
    pub height: f32,
}

impl CurveEditorWidget {
    pub fn new(label: impl Into<String>) -> Self {
        CurveEditorWidget {
            state: CurveEditorState::default(),
            label: label.into(),
            y_min: -0.1,
            y_max: 1.1,
            height: 180.0,
        }
    }

    pub fn with_range(mut self, min: f32, max: f32) -> Self {
        self.y_min = min;
        self.y_max = max;
        self
    }

    pub fn with_height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    /// Returns true if the curve was modified.
    pub fn show(&mut self, ui: &mut egui::Ui, curve: &mut Curve) -> bool {
        let mut changed = false;

        ui.label(RichText::new(&self.label).strong());

        // Multiplier row
        ui.horizontal(|ui| {
            ui.label("Multiplier:");
            if ui.add(egui::DragValue::new(&mut curve.multiplier).speed(0.01)).changed() {
                changed = true;
            }
            ui.label("Mode:");
            egui::ComboBox::from_id_source(format!("cew_mode_{}", self.label))
                .selected_text(match &curve.mode {
                    CurveMode::Constant => "Const",
                    CurveMode::Curve => "Curve",
                    CurveMode::RandomBetweenTwoCurves => "Rand",
                })
                .width(60.0)
                .show_ui(ui, |ui| {
                    let old_mode = curve.mode.clone();
                    ui.selectable_value(&mut curve.mode, CurveMode::Constant, "Constant");
                    ui.selectable_value(&mut curve.mode, CurveMode::Curve, "Curve");
                    ui.selectable_value(&mut curve.mode, CurveMode::RandomBetweenTwoCurves, "Random");
                    if curve.mode != old_mode { changed = true; }
                });
        });

        match curve.mode.clone() {
            CurveMode::Constant => {
                let val = curve.keys.first().map(|k| k.value).unwrap_or(0.0);
                let mut v = val;
                if ui.add(egui::DragValue::new(&mut v).speed(0.01)
                    .prefix("Value: ")).changed()
                {
                    for k in &mut curve.keys { k.value = v; }
                    changed = true;
                }
            }
            CurveMode::Curve => {
                let desired = Vec2::new(ui.available_width(), self.height);
                let (resp, painter) = ui.allocate_painter(desired, egui::Sense::click_and_drag());
                let rect = resp.rect;

                painter.rect_filled(rect, 2.0, Color32::from_rgb(22, 22, 28));
                draw_curve_grid(&painter, rect, self.y_min, self.y_max);
                draw_curve_path(&painter, rect, &curve.keys, Color32::from_rgb(100, 220, 120), self.y_min, self.y_max);

                let cp_r = 5.0;

                for (i, key) in curve.keys.iter().enumerate() {
                    let p = curve_to_screen(rect, key.time, key.value, self.y_min, self.y_max);
                    let is_sel = self.state.selected_key == Some(i);
                    let col = if is_sel { Color32::from_rgb(255, 220, 50) } else { Color32::WHITE };
                    painter.circle_filled(p, cp_r, col);
                    painter.circle_stroke(p, cp_r + 1.0, Stroke::new(1.0, Color32::from_rgb(80, 80, 80)));

                    // Draw tangent handles if bezier and selected
                    if is_sel && matches!(key.interpolation, Interpolation::Bezier) {
                        let handle_len = 30.0;
                        let in_p = p + Vec2::new(-handle_len, key.in_tangent * handle_len * 0.3);
                        let out_p = p + Vec2::new(handle_len, -key.out_tangent * handle_len * 0.3);
                        painter.line_segment([p, in_p], Stroke::new(1.0, Color32::from_rgb(150, 150, 200)));
                        painter.line_segment([p, out_p], Stroke::new(1.0, Color32::from_rgb(150, 200, 150)));
                        painter.circle_filled(in_p, 3.0, Color32::from_rgb(150, 150, 220));
                        painter.circle_filled(out_p, 3.0, Color32::from_rgb(150, 220, 150));
                    }
                }

                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let mut near_idx = None;
                        for (i, key) in curve.keys.iter().enumerate() {
                            let kp = curve_to_screen(rect, key.time, key.value, self.y_min, self.y_max);
                            if (pos - kp).length() < cp_r + 4.0 {
                                near_idx = Some(i);
                                break;
                            }
                        }
                        if let Some(ni) = near_idx {
                            self.state.selected_key = Some(ni);
                        } else {
                            let (t, v) = screen_to_curve(rect, pos, self.y_min, self.y_max);
                            let new_key = CurveKey::new(t.clamp(0.0, 1.0), v);
                            curve.keys.push(new_key);
                            curve.keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                            changed = true;
                        }
                    }
                }

                if resp.dragged() {
                    if let Some(idx) = self.state.selected_key {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            let (t, v) = screen_to_curve(rect, pos, self.y_min, self.y_max);
                            let last_key_idx = curve.keys.len().saturating_sub(1);
                            if let Some(key) = curve.keys.get_mut(idx) {
                                key.time = if idx == 0 { 0.0 }
                                    else if idx == last_key_idx { 1.0 }
                                    else { t.clamp(0.001, 0.999) };
                                key.value = v;
                            }
                            changed = true;
                        }
                    }
                }

                if resp.secondary_clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        for (i, key) in curve.keys.iter().enumerate() {
                            let kp = curve_to_screen(rect, key.time, key.value, self.y_min, self.y_max);
                            if (pos - kp).length() < cp_r + 4.0 {
                                self.state.right_click_key = Some(i);
                                break;
                            }
                        }
                    }
                }

                // Context menu
                if let Some(rk) = self.state.right_click_key {
                    if rk < curve.keys.len() {
                        let popup_id = egui::Id::new(format!("cew_ctx_{}", self.label));
                        egui::old_popup::popup_below_widget(ui, popup_id, &resp,
                            egui::PopupCloseBehavior::CloseOnClick, |ui| {
                                ui.set_min_width(120.0);
                                ui.label("Interpolation");
                                if ui.button("Linear").clicked() {
                                    curve.keys[rk].interpolation = Interpolation::Linear;
                                    self.state.right_click_key = None;
                                    changed = true;
                                }
                                if ui.button("Smooth").clicked() {
                                    curve.keys[rk].interpolation = Interpolation::Smooth;
                                    self.state.right_click_key = None;
                                    changed = true;
                                }
                                if ui.button("Constant").clicked() {
                                    curve.keys[rk].interpolation = Interpolation::Constant;
                                    self.state.right_click_key = None;
                                    changed = true;
                                }
                                if ui.button("Bezier").clicked() {
                                    curve.keys[rk].interpolation = Interpolation::Bezier;
                                    self.state.right_click_key = None;
                                    changed = true;
                                }
                                ui.separator();
                                if curve.keys.len() > 2 {
                                    if ui.button("Delete").clicked() {
                                        curve.keys.remove(rk);
                                        self.state.right_click_key = None;
                                        self.state.selected_key = None;
                                        changed = true;
                                    }
                                }
                            });
                    }
                    if resp.clicked_elsewhere() { self.state.right_click_key = None; }
                }

                // Selected key detail editor
                if let Some(sel) = self.state.selected_key {
                    if sel < curve.keys.len() {
                        ui.horizontal(|ui| {
                            ui.label(format!("Key #{}", sel));
                            ui.add(egui::DragValue::new(&mut curve.keys[sel].time)
                                .speed(0.001).range(0.0..=1.0).prefix("t:"));
                            ui.add(egui::DragValue::new(&mut curve.keys[sel].value)
                                .speed(0.01).prefix("v:"));
                            if matches!(curve.keys[sel].interpolation, Interpolation::Bezier) {
                                ui.add(egui::DragValue::new(&mut curve.keys[sel].in_tangent)
                                    .speed(0.01).prefix("in:"));
                                ui.add(egui::DragValue::new(&mut curve.keys[sel].out_tangent)
                                    .speed(0.01).prefix("out:"));
                            }
                        });
                        changed = true; // conservative: mark changed after any drag
                    }
                }

                // Range controls
                ui.horizontal(|ui| {
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut self.y_min).speed(0.01).prefix("min:")).changed() {}
                    if ui.add(egui::DragValue::new(&mut self.y_max).speed(0.01).prefix("max:")).changed() {}
                    if ui.small_button("Fit").clicked() {
                        let vals: Vec<f32> = curve.keys.iter().map(|k| k.value).collect();
                        if !vals.is_empty() {
                            self.y_min = vals.iter().cloned().fold(f32::INFINITY, f32::min) - 0.1;
                            self.y_max = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max) + 0.1;
                        }
                    }
                    if ui.small_button("0-1").clicked() {
                        self.y_min = -0.05;
                        self.y_max = 1.05;
                    }
                    if ui.small_button("Reset").clicked() {
                        curve.keys = vec![CurveKey::new(0.0, 1.0), CurveKey::new(1.0, 1.0)];
                        changed = true;
                    }
                });
            }
            CurveMode::RandomBetweenTwoCurves => {
                show_curve_canvas_dual(ui, &mut curve.keys, &mut curve.keys2, &mut self.state);
                ui.horizontal(|ui| {
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut self.y_min).speed(0.01).prefix("min:")).changed() {}
                    if ui.add(egui::DragValue::new(&mut self.y_max).speed(0.01).prefix("max:")).changed() {}
                });
            }
        }

        changed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ADVANCED GRADIENT EDITOR — Standalone widget
// ─────────────────────────────────────────────────────────────────────────────

pub struct GradientEditorWidget {
    pub state: GradientEditorState,
    pub label: String,
}

impl GradientEditorWidget {
    pub fn new(label: impl Into<String>) -> Self {
        GradientEditorWidget {
            state: GradientEditorState::default(),
            label: label.into(),
        }
    }

    /// Returns true if gradient was modified.
    pub fn show(&mut self, ui: &mut egui::Ui, gradient: &mut ColorGradient) -> bool {
        let mut changed = false;

        ui.label(RichText::new(&self.label).strong());

        // Mode selector
        ui.horizontal(|ui| {
            ui.label("Mode:");
            let old = gradient.mode.clone();
            egui::ComboBox::from_id_source(format!("gew_mode_{}", self.label))
                .selected_text(match gradient.mode { GradientMode::Blend => "Blend", GradientMode::Fixed => "Fixed" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut gradient.mode, GradientMode::Blend, "Blend");
                    ui.selectable_value(&mut gradient.mode, GradientMode::Fixed, "Fixed");
                });
            if gradient.mode != old { changed = true; }
        });

        // Main gradient bar (larger, interactive)
        let bar_h = 32.0;
        let (bar_resp, painter) = ui.allocate_painter(
            Vec2::new(ui.available_width(), bar_h),
            egui::Sense::click_and_drag(),
        );
        let bar_rect = bar_resp.rect;
        draw_gradient_bar(&painter, bar_rect, gradient);

        // Checkerboard behind bar for alpha visualization
        draw_checker(&painter, bar_rect, 8.0);
        draw_gradient_bar_premul(&painter, bar_rect, gradient);

        // Alpha stop markers (above bar)
        let alpha_y = bar_rect.min.y - 8.0;
        for (i, ak) in gradient.alpha_keys.iter().enumerate() {
            let mx = bar_rect.min.x + ak.time * bar_rect.width();
            let sel = self.state.selected_alpha_key == Some(i);
            let col = Color32::from_gray((ak.alpha * 255.0) as u8);
            draw_stop_marker(&painter, Pos2::new(mx, alpha_y), col, sel, true);
        }

        // Color stop markers (below bar)
        let color_y = bar_rect.max.y + 8.0;
        for (i, ck) in gradient.color_keys.iter().enumerate() {
            let mx = bar_rect.min.x + ck.time * bar_rect.width();
            let sel = self.state.selected_color_key == Some(i);
            let c = Color32::from_rgb(
                (ck.color[0]*255.0) as u8, (ck.color[1]*255.0) as u8, (ck.color[2]*255.0) as u8);
            draw_stop_marker(&painter, Pos2::new(mx, color_y), c, sel, false);
        }

        // Click handling
        if bar_resp.clicked() {
            if let Some(pos) = bar_resp.interact_pointer_pos() {
                let t = ((pos.x - bar_rect.min.x) / bar_rect.width()).clamp(0.0, 1.0);
                let in_color_zone = pos.y > bar_rect.center().y;
                if in_color_zone {
                    // Check near existing color stop
                    let near = gradient.color_keys.iter().enumerate()
                        .find(|(_, ck)| ((bar_rect.min.x + ck.time * bar_rect.width()) - pos.x).abs() < 8.0)
                        .map(|(i, _)| i);
                    if let Some(ni) = near {
                        self.state.selected_color_key = Some(ni);
                        self.state.selected_alpha_key = None;
                        let ck = &gradient.color_keys[ni];
                        self.state.edit_color = ck.color;
                    } else {
                        let c = gradient.evaluate(t);
                        gradient.color_keys.push(GradientColorKey {
                            time: t,
                            color: [c.r() as f32/255.0, c.g() as f32/255.0, c.b() as f32/255.0],
                        });
                        gradient.color_keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                        changed = true;
                    }
                } else {
                    let near = gradient.alpha_keys.iter().enumerate()
                        .find(|(_, ak)| ((bar_rect.min.x + ak.time * bar_rect.width()) - pos.x).abs() < 8.0)
                        .map(|(i, _)| i);
                    if let Some(ni) = near {
                        self.state.selected_alpha_key = Some(ni);
                        self.state.selected_color_key = None;
                    } else {
                        let a = gradient.evaluate(t).a() as f32 / 255.0;
                        gradient.alpha_keys.push(GradientAlphaKey { time: t, alpha: a });
                        gradient.alpha_keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                        changed = true;
                    }
                }
            }
        }

        // Drag
        if bar_resp.dragged() {
            if let Some(pos) = bar_resp.interact_pointer_pos() {
                let t = ((pos.x - bar_rect.min.x) / bar_rect.width()).clamp(0.0, 1.0);
                if let Some(ci) = self.state.selected_color_key {
                    if ci > 0 && ci < gradient.color_keys.len().saturating_sub(1) {
                        gradient.color_keys[ci].time = t;
                        changed = true;
                    }
                }
                if let Some(ai) = self.state.selected_alpha_key {
                    if ai > 0 && ai < gradient.alpha_keys.len().saturating_sub(1) {
                        gradient.alpha_keys[ai].time = t;
                        changed = true;
                    }
                }
            }
        }

        ui.add_space(20.0); // space for markers

        // Edit selected color key
        if let Some(ci) = self.state.selected_color_key {
            if ci < gradient.color_keys.len() {
                ui.separator();
                ui.label(format!("Color Stop #{}", ci));
                ui.horizontal(|ui| {
                    ui.label("t:");
                    if ui.add(egui::DragValue::new(&mut gradient.color_keys[ci].time)
                        .speed(0.001).range(0.0..=1.0)).changed()
                    {
                        gradient.color_keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                        changed = true;
                    }
                });
                let ck = &mut gradient.color_keys[ci];
                let mut col32 = Color32::from_rgb(
                    (ck.color[0]*255.0) as u8, (ck.color[1]*255.0) as u8, (ck.color[2]*255.0) as u8);
                if ui.color_edit_button_srgba(&mut col32).changed() {
                    ck.color = [col32.r() as f32/255.0, col32.g() as f32/255.0, col32.b() as f32/255.0];
                    changed = true;
                }
                if gradient.color_keys.len() > 2 {
                    if ui.small_button("Remove").clicked() {
                        gradient.color_keys.remove(ci);
                        self.state.selected_color_key = None;
                        changed = true;
                    }
                }
            }
        }

        // Edit selected alpha key
        if let Some(ai) = self.state.selected_alpha_key {
            if ai < gradient.alpha_keys.len() {
                ui.separator();
                ui.label(format!("Alpha Stop #{}", ai));
                ui.horizontal(|ui| {
                    ui.label("t:");
                    if ui.add(egui::DragValue::new(&mut gradient.alpha_keys[ai].time)
                        .speed(0.001).range(0.0..=1.0)).changed()
                    {
                        gradient.alpha_keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                        changed = true;
                    }
                });
                if ui.add(egui::Slider::new(&mut gradient.alpha_keys[ai].alpha, 0.0..=1.0)
                    .text("Alpha")).changed()
                {
                    changed = true;
                }
                if gradient.alpha_keys.len() > 2 {
                    if ui.small_button("Remove").clicked() {
                        gradient.alpha_keys.remove(ai);
                        self.state.selected_alpha_key = None;
                        changed = true;
                    }
                }
            }
        }

        // Preset buttons for the gradient
        ui.horizontal(|ui| {
            if ui.small_button("White→Clear").clicked() {
                *gradient = ColorGradient::default();
                changed = true;
            }
            if ui.small_button("Fire").clicked() {
                *gradient = ColorGradient::fire();
                changed = true;
            }
            if ui.small_button("Smoke").clicked() {
                *gradient = ColorGradient::smoke();
                changed = true;
            }
            if ui.small_button("Rainbow").clicked() {
                *gradient = gradient_rainbow();
                changed = true;
            }
        });

        changed
    }
}

fn draw_checker(painter: &Painter, rect: Rect, cell_size: f32) {
    let cols = (rect.width() / cell_size).ceil() as u32;
    let rows = (rect.height() / cell_size).ceil() as u32;
    for row in 0..rows {
        for col in 0..cols {
            if (row + col) % 2 == 0 {
                let cr = Rect::from_min_size(
                    Pos2::new(rect.min.x + col as f32 * cell_size, rect.min.y + row as f32 * cell_size),
                    Vec2::splat(cell_size),
                ).intersect(rect);
                painter.rect_filled(cr, 0.0, Color32::from_rgb(160, 160, 160));
            } else {
                let cr = Rect::from_min_size(
                    Pos2::new(rect.min.x + col as f32 * cell_size, rect.min.y + row as f32 * cell_size),
                    Vec2::splat(cell_size),
                ).intersect(rect);
                painter.rect_filled(cr, 0.0, Color32::from_rgb(100, 100, 100));
            }
        }
    }
}

fn draw_gradient_bar_premul(painter: &Painter, rect: Rect, gradient: &ColorGradient) {
    let steps = 64u32;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i+1) as f32 / steps as f32;
        let c = gradient.evaluate((t0 + t1) * 0.5);
        let strip = Rect::from_min_max(
            Pos2::new(rect.min.x + t0 * rect.width(), rect.min.y),
            Pos2::new(rect.min.x + t1 * rect.width(), rect.max.y),
        );
        painter.rect_filled(strip, 0.0, c);
    }
}

fn draw_stop_marker(painter: &Painter, pos: Pos2, color: Color32, selected: bool, above: bool) {
    let size = 8.0;
    let tip_offset = if above { Vec2::new(0.0, size) } else { Vec2::new(0.0, -size) };
    let side_offset = Vec2::new(size * 0.6, 0.0);

    // Arrow pointing toward bar
    let tip = pos + tip_offset;
    let pts = if above {
        vec![pos - side_offset, pos + side_offset, tip]
    } else {
        vec![pos - side_offset, pos + side_offset, tip]
    };
    painter.add(Shape::convex_polygon(pts, color, Stroke::new(if selected { 2.0 } else { 1.0 }, Color32::WHITE)));
    // Square top/bottom
    let sq = Rect::from_center_size(pos, Vec2::splat(size * 1.1));
    painter.rect_filled(sq, 1.0, color);
    painter.rect_stroke(sq, 1.0, Stroke::new(if selected { 2.0 } else { 1.0 }, Color32::WHITE), egui::StrokeKind::Outside);
}

fn gradient_rainbow() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [1.0, 0.0, 0.0] },
            GradientColorKey { time: 0.166, color: [1.0, 0.5, 0.0] },
            GradientColorKey { time: 0.333, color: [1.0, 1.0, 0.0] },
            GradientColorKey { time: 0.5, color: [0.0, 1.0, 0.0] },
            GradientColorKey { time: 0.666, color: [0.0, 0.5, 1.0] },
            GradientColorKey { time: 0.833, color: [0.3, 0.0, 1.0] },
            GradientColorKey { time: 1.0, color: [0.8, 0.0, 0.5] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 1.0 },
            GradientAlphaKey { time: 1.0, alpha: 1.0 },
        ],
        mode: GradientMode::Blend,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RANGE-OR-CURVE EDITOR — Standalone widget
// ─────────────────────────────────────────────────────────────────────────────

pub struct RangeOrCurveWidget {
    pub label: String,
    pub curve_state: CurveEditorState,
    pub y_min: f32,
    pub y_max: f32,
    pub curve_expanded: bool,
}

impl RangeOrCurveWidget {
    pub fn new(label: impl Into<String>) -> Self {
        RangeOrCurveWidget {
            label: label.into(),
            curve_state: CurveEditorState::default(),
            y_min: -0.1,
            y_max: 1.1,
            curve_expanded: false,
        }
    }

    pub fn with_range(mut self, min: f32, max: f32) -> Self {
        self.y_min = min;
        self.y_max = max;
        self
    }

    /// Returns true if modified.
    pub fn show(&mut self, ui: &mut egui::Ui, roc: &mut RangeOrCurve) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label(RichText::new(&self.label).strong());

            // Mode dropdown
            let lbl = roc.label().to_string();
            egui::ComboBox::from_id_source(format!("roc_wgt_{}", self.label))
                .selected_text(&lbl)
                .width(180.0)
                .show_ui(ui, |ui| {
                    let prev_discriminant = std::mem::discriminant(&*roc);
                    if ui.selectable_label(matches!(roc, RangeOrCurve::Constant(_)), "Constant").clicked() {
                        let v = match &*roc { RangeOrCurve::Constant(v) => *v, _ => 1.0 };
                        *roc = RangeOrCurve::Constant(v);
                        changed = true;
                    }
                    if ui.selectable_label(matches!(roc, RangeOrCurve::Random(..)), "Random Between Two Constants").clicked() {
                        *roc = RangeOrCurve::Random(0.0, 1.0);
                        changed = true;
                    }
                    if ui.selectable_label(matches!(roc, RangeOrCurve::Curve(_)), "Curve").clicked() {
                        *roc = RangeOrCurve::Curve(Curve::default());
                        self.curve_expanded = true;
                        changed = true;
                    }
                    if ui.selectable_label(matches!(roc, RangeOrCurve::RandomCurve(..)), "Random Between Two Curves").clicked() {
                        *roc = RangeOrCurve::RandomCurve(Curve::default(), Curve::default());
                        self.curve_expanded = true;
                        changed = true;
                    }
                });
        });

        match roc {
            RangeOrCurve::Constant(v) => {
                if ui.add(egui::DragValue::new(v).speed(0.01)).changed() { changed = true; }
            }
            RangeOrCurve::Random(lo, hi) => {
                ui.horizontal(|ui| {
                    ui.label("Min:");
                    if ui.add(egui::DragValue::new(lo).speed(0.01)).changed() { changed = true; }
                    ui.label("Max:");
                    if ui.add(egui::DragValue::new(hi).speed(0.01)).changed() { changed = true; }
                });
            }
            RangeOrCurve::Curve(c) => {
                // Mini preview + expand toggle
                ui.horizontal(|ui| {
                    let (mini_rect, _) = ui.allocate_exact_size(Vec2::new(80.0, 20.0), egui::Sense::hover());
                    draw_curve_mini(ui.painter(), mini_rect, c);
                    if ui.small_button(if self.curve_expanded { "▲" } else { "▼" }).clicked() {
                        self.curve_expanded = !self.curve_expanded;
                    }
                });
                if self.curve_expanded {
                    let desired = Vec2::new(ui.available_width(), 160.0);
                    let (resp, painter) = ui.allocate_painter(desired, egui::Sense::click_and_drag());
                    let rect = resp.rect;
                    painter.rect_filled(rect, 2.0, Color32::from_rgb(22, 22, 28));
                    draw_curve_grid(&painter, rect, self.y_min, self.y_max);
                    draw_curve_path(&painter, rect, &c.keys, Color32::from_rgb(100, 220, 120), self.y_min, self.y_max);

                    for (i, key) in c.keys.iter().enumerate() {
                        let p = curve_to_screen(rect, key.time, key.value, self.y_min, self.y_max);
                        let is_sel = self.curve_state.selected_key == Some(i);
                        painter.circle_filled(p, 5.0, if is_sel { Color32::from_rgb(255,220,50) } else { Color32::WHITE });
                    }

                    if resp.clicked() {
                        if let Some(pos) = resp.interact_pointer_pos() {
                            let mut near_idx = None;
                            for (i, k) in c.keys.iter().enumerate() {
                                let kp = curve_to_screen(rect, k.time, k.value, self.y_min, self.y_max);
                                if (pos - kp).length() < 8.0 { near_idx = Some(i); break; }
                            }
                            if let Some(ni) = near_idx {
                                self.curve_state.selected_key = Some(ni);
                            } else {
                                let (t, v) = screen_to_curve(rect, pos, self.y_min, self.y_max);
                                c.keys.push(CurveKey::new(t.clamp(0.0,1.0), v));
                                c.keys.sort_by(|a,b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
                                changed = true;
                            }
                        }
                    }

                    if resp.dragged() {
                        if let Some(idx) = self.curve_state.selected_key {
                            if let Some(pos) = resp.interact_pointer_pos() {
                                let (t, v) = screen_to_curve(rect, pos, self.y_min, self.y_max);
                                let last_key_c = c.keys.len().saturating_sub(1);
                                if let Some(k) = c.keys.get_mut(idx) {
                                    k.time = if idx == 0 { 0.0 } else if idx == last_key_c { 1.0 } else { t.clamp(0.001, 0.999) };
                                    k.value = v;
                                }
                                changed = true;
                            }
                        }
                    }
                }
            }
            RangeOrCurve::RandomCurve(c0, c1) => {
                ui.horizontal(|ui| {
                    let (mini0, _) = ui.allocate_exact_size(Vec2::new(60.0, 20.0), egui::Sense::hover());
                    draw_curve_mini(ui.painter(), mini0, c0);
                    ui.label("–");
                    let (mini1, _) = ui.allocate_exact_size(Vec2::new(60.0, 20.0), egui::Sense::hover());
                    draw_curve_mini(ui.painter(), mini1, c1);
                    if ui.small_button(if self.curve_expanded { "▲" } else { "▼" }).clicked() {
                        self.curve_expanded = !self.curve_expanded;
                    }
                });
                if self.curve_expanded {
                    let mut tmp_state = self.curve_state.clone();
                    show_curve_canvas_dual(ui, &mut c0.keys, &mut c1.keys, &mut tmp_state);
                    self.curve_state = tmp_state;
                }
            }
        }

        changed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EMITTER SHAPE EDITOR WIDGET
// ─────────────────────────────────────────────────────────────────────────────

/// A dedicated widget for editing emitter shapes with inline visualization.
pub struct EmitterShapeWidget;

impl EmitterShapeWidget {
    pub fn show(ui: &mut egui::Ui, config: &mut EmitterConfig) -> bool {
        let mut changed = false;

        let shape_label = config.shape.label().to_string();
        ui.horizontal(|ui| {
            ui.label("Shape:");
            egui::ComboBox::from_id_source("esw_shape")
                .selected_text(&shape_label)
                .show_ui(ui, |ui| {
                    macro_rules! shape_btn {
                        ($label:expr, $shape:expr) => {
                            if ui.selectable_label(
                                std::mem::discriminant(&config.shape) == std::mem::discriminant(&$shape),
                                $label
                            ).clicked() {
                                config.shape = $shape;
                                changed = true;
                            }
                        }
                    }
                    shape_btn!("Point", EmitterShape::Point);
                    shape_btn!("Sphere", EmitterShape::Sphere { radius: 1.0, hemisphere: false });
                    shape_btn!("Cone", EmitterShape::Cone { angle: 25.0, radius: 1.0 });
                    shape_btn!("Box", EmitterShape::Box { size: [1.0, 1.0, 1.0] });
                    shape_btn!("Circle", EmitterShape::Circle { radius: 1.0, arc: 360.0 });
                    shape_btn!("Edge", EmitterShape::Edge { length: 2.0 });
                    shape_btn!("Mesh", EmitterShape::Mesh);
                });
        });

        match &mut config.shape {
            EmitterShape::Point => {
                ui.label("All particles spawn from origin.");
            }
            EmitterShape::Sphere { radius, hemisphere } => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    if ui.add(egui::DragValue::new(radius).speed(0.01).range(0.001..=100.0)).changed() { changed = true; }
                });
                if ui.checkbox(hemisphere, "Hemisphere only").changed() { changed = true; }
            }
            EmitterShape::Cone { angle, radius } => {
                ui.horizontal(|ui| {
                    ui.label("Angle:");
                    if ui.add(egui::DragValue::new(angle).speed(0.5).suffix("°").range(0.0..=180.0)).changed() { changed = true; }
                });
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    if ui.add(egui::DragValue::new(radius).speed(0.01).range(0.0..=100.0)).changed() { changed = true; }
                });
            }
            EmitterShape::Box { size } => {
                ui.horizontal(|ui| {
                    ui.label("Size X:");
                    if ui.add(egui::DragValue::new(&mut size[0]).speed(0.01).range(0.0..=100.0)).changed() { changed = true; }
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut size[1]).speed(0.01).range(0.0..=100.0)).changed() { changed = true; }
                    ui.label("Z:");
                    if ui.add(egui::DragValue::new(&mut size[2]).speed(0.01).range(0.0..=100.0)).changed() { changed = true; }
                });
            }
            EmitterShape::Circle { radius, arc } => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    if ui.add(egui::DragValue::new(radius).speed(0.01).range(0.001..=100.0)).changed() { changed = true; }
                });
                ui.horizontal(|ui| {
                    ui.label("Arc:");
                    if ui.add(egui::DragValue::new(arc).speed(1.0).suffix("°").range(1.0..=360.0)).changed() { changed = true; }
                });
            }
            EmitterShape::Edge { length } => {
                ui.horizontal(|ui| {
                    ui.label("Length:");
                    if ui.add(egui::DragValue::new(length).speed(0.01).range(0.0..=100.0)).changed() { changed = true; }
                });
            }
            EmitterShape::Mesh => {
                ui.label("Mesh: not supported in preview.");
            }
        }

        // Inline visualizer
        let viz_size = Vec2::new(ui.available_width().min(200.0), 100.0);
        let (resp, painter) = ui.allocate_painter(viz_size, egui::Sense::hover());
        draw_shape_visualizer(&painter, resp.rect, &config.shape);

        changed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BURST EDITOR WIDGET
// ─────────────────────────────────────────────────────────────────────────────

pub struct BurstEditorWidget;

impl BurstEditorWidget {
    pub fn show(ui: &mut egui::Ui, bursts: &mut Vec<Burst>) -> bool {
        let mut changed = false;

        egui::CollapsingHeader::new("Bursts").default_open(false).show(ui, |ui| {
            egui::Grid::new("burst_grid")
                .num_columns(6)
                .striped(true)
                .spacing([4.0, 2.0])
                .show(ui, |ui| {
                    ui.label("Time");
                    ui.label("Count");
                    ui.label("Interval");
                    ui.label("Repeats");
                    ui.label("");
                    ui.end_row();

                    let mut to_remove = None;
                    for (i, burst) in bursts.iter_mut().enumerate() {
                        if ui.add(egui::DragValue::new(&mut burst.time)
                            .speed(0.01).range(0.0..=600.0)).changed() { changed = true; }
                        if ui.add(egui::DragValue::new(&mut burst.count)
                            .speed(1.0).range(1..=10000)).changed() { changed = true; }
                        if ui.add(egui::DragValue::new(&mut burst.repeat_interval)
                            .speed(0.01).range(0.0..=600.0)).changed() { changed = true; }
                        if ui.add(egui::DragValue::new(&mut burst.repeat_count)
                            .speed(1).range(-1..=1000)).changed() { changed = true; }
                        if ui.small_button("✕").clicked() { to_remove = Some(i); }
                        ui.end_row();
                    }
                    if let Some(i) = to_remove { bursts.remove(i); changed = true; }
                });

            if ui.button("+ Add Burst").clicked() {
                bursts.push(Burst::default());
                changed = true;
            }
        });

        changed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SYSTEM LIST WIDGET — for multi-system management
// ─────────────────────────────────────────────────────────────────────────────

pub struct SystemListWidget;

impl SystemListWidget {
    pub fn show(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
        egui::CollapsingHeader::new("Systems").default_open(true).show(ui, |ui| {
            let count = editor.systems.len();
            let mut swap_with_next: Option<usize> = None;
            let mut to_remove: Option<usize> = None;
            let mut to_duplicate: Option<usize> = None;

            for i in 0..count {
                let name = editor.systems[i].name.clone();
                let is_active = editor.active_system == i;

                ui.horizontal(|ui| {
                    if ui.selectable_label(is_active, &name).clicked() {
                        editor.active_system = i;
                        editor.particles.clear();
                        editor.emission_accumulator = 0.0;
                        editor.preview_time = 0.0;
                        let mc = editor.systems[i].modules.len();
                        editor.module_expanded = vec![false; mc];
                        editor.selected_module = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if count > 1 {
                            if ui.small_button("✕").on_hover_text("Delete").clicked() {
                                to_remove = Some(i);
                            }
                        }
                        if ui.small_button("⧉").on_hover_text("Duplicate").clicked() {
                            to_duplicate = Some(i);
                        }
                        if i + 1 < count && ui.small_button("▼").clicked() {
                            swap_with_next = Some(i);
                        }
                    });
                });
            }

            if let Some(i) = swap_with_next {
                editor.systems.swap(i, i+1);
                if editor.active_system == i { editor.active_system = i+1; }
                else if editor.active_system == i+1 { editor.active_system = i; }
            }
            if let Some(i) = to_duplicate {
                let mut cloned = editor.systems[i].clone();
                cloned.name = format!("{} (copy)", cloned.name);
                editor.systems.insert(i+1, cloned);
            }
            if let Some(i) = to_remove {
                editor.systems.remove(i);
                if editor.active_system >= editor.systems.len() {
                    editor.active_system = editor.systems.len().saturating_sub(1);
                }
                editor.particles.clear();
            }

            if ui.button("+ New System").clicked() {
                let mut ps = ParticleSystem::default();
                ps.name = format!("System {}", editor.systems.len() + 1);
                editor.systems.push(ps);
                editor.active_system = editor.systems.len() - 1;
                let mc = editor.systems[editor.active_system].modules.len();
                editor.module_expanded = vec![false; mc];
                editor.selected_module = None;
                editor.particles.clear();
            }
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TIMELINE / SCRUBBER WIDGET
// ─────────────────────────────────────────────────────────────────────────────

pub struct TimelineScrubberWidget;

impl TimelineScrubberWidget {
    pub fn show(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
        let sys = &editor.systems[editor.active_system];
        let duration = sys.duration;

        ui.horizontal(|ui| {
            // Play/pause/stop
            if ui.button(if editor.preview_running { "⏸" } else { "▶" }).clicked() {
                editor.preview_running = !editor.preview_running;
            }
            if ui.button("⏹").clicked() {
                editor.preview_running = false;
                editor.preview_time = 0.0;
                editor.particles.clear();
                editor.emission_accumulator = 0.0;
            }
            if ui.button("↺").clicked() {
                editor.preview_running = true;
                editor.preview_time = 0.0;
                editor.particles.clear();
                editor.emission_accumulator = 0.0;
            }
        });

        // Scrubber
        let scrubber_h = 20.0;
        let (resp, painter) = ui.allocate_painter(
            Vec2::new(ui.available_width(), scrubber_h),
            egui::Sense::click_and_drag(),
        );
        let rect = resp.rect;

        painter.rect_filled(rect, 2.0, Color32::from_rgb(30, 30, 40));

        // Time markers
        let num_marks = (duration / 0.5).floor() as u32 + 1;
        for i in 0..=num_marks {
            let t = i as f32 * 0.5;
            if t > duration { break; }
            let x = rect.min.x + (t / duration) * rect.width();
            painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                Stroke::new(1.0, Color32::from_rgb(50, 50, 70)));
            painter.text(Pos2::new(x, rect.min.y + 2.0), egui::Align2::CENTER_TOP,
                format!("{:.1}", t), FontId::proportional(8.0), Color32::from_rgb(90, 90, 110));
        }

        // Burst markers
        for burst in &sys.emitter.emission_bursts {
            if burst.time <= duration {
                let x = rect.min.x + (burst.time / duration) * rect.width();
                painter.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                    Stroke::new(2.0, Color32::from_rgb(255, 200, 50)));
                painter.circle_filled(Pos2::new(x, rect.min.y + scrubber_h * 0.5), 3.0,
                    Color32::from_rgb(255, 200, 50));
            }
        }

        // Playhead
        let head_x = rect.min.x + (editor.preview_time.min(duration) / duration) * rect.width();
        painter.line_segment(
            [Pos2::new(head_x, rect.min.y), Pos2::new(head_x, rect.max.y)],
            Stroke::new(2.0, Color32::from_rgb(220, 220, 220)),
        );
        painter.circle_filled(Pos2::new(head_x, rect.min.y), 4.0, Color32::WHITE);

        // Drag to scrub
        if resp.clicked() || resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = ((pos.x - rect.min.x) / rect.width()) * duration;
                editor.preview_time = t.clamp(0.0, duration);
                editor.particles.retain(|p| p.age < p.lifetime);
            }
        }

        // Speed control
        ui.horizontal(|ui| {
            ui.label("Speed:");
            ui.add(egui::Slider::new(&mut editor.preview_speed, 0.01..=5.0)
                .logarithmic(true)
                .suffix("x"));
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE SIMULATION — Extended with force field support
// ─────────────────────────────────────────────────────────────────────────────

/// A simple attractor/repulsor force field used by the preview simulation.
#[derive(Clone, Debug)]
pub struct ForceField {
    pub position: [f32; 2],
    pub strength: f32,
    pub radius: f32,
    pub kind: ForceFieldKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ForceFieldKind {
    Attract,
    Repel,
    Vortex,
}

impl ForceField {
    pub fn apply(&self, particle: &mut PreviewParticle, dt: f32) {
        let dx = self.position[0] - particle.position[0];
        let dy = self.position[1] - particle.position[1];
        let dist_sq = dx * dx + dy * dy;
        let dist = dist_sq.sqrt().max(0.001);

        if dist > self.radius { return; }

        let falloff = 1.0 - (dist / self.radius).clamp(0.0, 1.0);
        let force_mag = self.strength * falloff * dt;

        match self.kind {
            ForceFieldKind::Attract => {
                particle.velocity[0] += (dx / dist) * force_mag;
                particle.velocity[1] += (dy / dist) * force_mag;
            }
            ForceFieldKind::Repel => {
                particle.velocity[0] -= (dx / dist) * force_mag;
                particle.velocity[1] -= (dy / dist) * force_mag;
            }
            ForceFieldKind::Vortex => {
                // Perpendicular force
                particle.velocity[0] += (-dy / dist) * force_mag;
                particle.velocity[1] += (dx / dist) * force_mag;
            }
        }
    }
}

impl ParticleEditor {
    /// Draw force fields in the preview panel.
    pub fn draw_force_fields(fields: &[ForceField], painter: &Painter, center: Pos2, ppu: f32) {
        for field in fields {
            let fx = center.x + field.position[0] * ppu;
            let fy = center.y - field.position[1] * ppu;
            let fp = Pos2::new(fx, fy);
            let fr = field.radius * ppu;
            let col = match field.kind {
                ForceFieldKind::Attract => Color32::from_rgba_unmultiplied(50, 150, 255, 60),
                ForceFieldKind::Repel => Color32::from_rgba_unmultiplied(255, 80, 50, 60),
                ForceFieldKind::Vortex => Color32::from_rgba_unmultiplied(200, 100, 255, 60),
            };
            let stroke_col = match field.kind {
                ForceFieldKind::Attract => Color32::from_rgb(50, 150, 255),
                ForceFieldKind::Repel => Color32::from_rgb(255, 80, 50),
                ForceFieldKind::Vortex => Color32::from_rgb(200, 100, 255),
            };
            painter.circle_filled(fp, fr, col);
            painter.circle_stroke(fp, fr, Stroke::new(1.0, stroke_col));
            painter.circle_filled(fp, 3.0, stroke_col);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CURVE PRESETS
// ─────────────────────────────────────────────────────────────────────────────

impl Curve {
    /// A smooth ease-in ease-out curve.
    pub fn ease_in_out() -> Self {
        Curve {
            keys: vec![
                CurveKey { time: 0.0, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth },
                CurveKey { time: 1.0, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth },
            ],
            keys2: vec![],
            mode: CurveMode::Curve,
            multiplier: 1.0,
        }
    }

    /// Bell curve — rises to 1 at center, drops to 0 at edges.
    pub fn bell() -> Self {
        Curve {
            keys: vec![
                CurveKey::new(0.0, 0.0),
                CurveKey::new(0.5, 1.0),
                CurveKey::new(1.0, 0.0),
            ],
            keys2: vec![],
            mode: CurveMode::Curve,
            multiplier: 1.0,
        }
    }

    /// Sawtooth up — linear rise then instant drop.
    pub fn sawtooth() -> Self {
        Curve {
            keys: vec![
                CurveKey { time: 0.0, value: 0.0, in_tangent: 1.0, out_tangent: 1.0, interpolation: Interpolation::Linear },
                CurveKey { time: 0.9999, value: 1.0, in_tangent: 1.0, out_tangent: 0.0, interpolation: Interpolation::Linear },
                CurveKey { time: 1.0, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            ],
            keys2: vec![],
            mode: CurveMode::Curve,
            multiplier: 1.0,
        }
    }

    /// Bounce — falls to 0 with a bounce.
    pub fn bounce() -> Self {
        Curve {
            keys: vec![
                CurveKey::new(0.0, 1.0),
                CurveKey::new(0.6, 0.3),
                CurveKey::new(0.8, 0.15),
                CurveKey::new(0.9, 0.05),
                CurveKey::new(1.0, 0.0),
            ],
            keys2: vec![],
            mode: CurveMode::Curve,
            multiplier: 1.0,
        }
    }

    /// Pulse — brief spike then zero.
    pub fn pulse() -> Self {
        Curve {
            keys: vec![
                CurveKey::new(0.0, 0.0),
                CurveKey::new(0.1, 1.0),
                CurveKey::new(0.2, 0.0),
                CurveKey::new(1.0, 0.0),
            ],
            keys2: vec![],
            mode: CurveMode::Curve,
            multiplier: 1.0,
        }
    }

    /// Show a button that opens a preset picker. Returns the chosen curve if any.
    pub fn preset_picker(ui: &mut egui::Ui) -> Option<Curve> {
        let mut result = None;
        egui::menu::menu_button(ui, "Presets", |ui| {
            if ui.button("0 → 1 (Linear)").clicked() { result = Some(Curve::linear_zero_to_one()); ui.close_menu(); }
            if ui.button("1 → 0 (Linear)").clicked() { result = Some(Curve::linear_one_to_zero()); ui.close_menu(); }
            if ui.button("Ease In/Out").clicked() { result = Some(Curve::ease_in_out()); ui.close_menu(); }
            if ui.button("Bell").clicked() { result = Some(Curve::bell()); ui.close_menu(); }
            if ui.button("Sawtooth").clicked() { result = Some(Curve::sawtooth()); ui.close_menu(); }
            if ui.button("Bounce").clicked() { result = Some(Curve::bounce()); ui.close_menu(); }
            if ui.button("Pulse").clicked() { result = Some(Curve::pulse()); ui.close_menu(); }
            if ui.button("Constant 1").clicked() { result = Some(Curve::constant(1.0)); ui.close_menu(); }
            if ui.button("Constant 0").clicked() { result = Some(Curve::constant(0.0)); ui.close_menu(); }
        });
        result
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// INLINE PREVIEW THUMBNAIL
// ─────────────────────────────────────────────────────────────────────────────

/// Draws a tiny visual thumbnail of a particle system's color gradient + size curve.
pub fn draw_system_thumbnail(painter: &Painter, rect: Rect, sys: &ParticleSystem) {
    painter.rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 25));

    // Find ColorOverLifetime gradient
    let gradient_opt = sys.modules.iter().find_map(|m| {
        if let ParticleModule::ColorOverLifetime { enabled: true, gradient } = m { Some(gradient) } else { None }
    });

    // Find SizeOverLifetime curve
    let size_opt = sys.modules.iter().find_map(|m| {
        if let ParticleModule::SizeOverLifetime { enabled: true, size } = m { Some(size) } else { None }
    });

    let steps = 32u32;
    let step_w = rect.width() / steps as f32;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let col = gradient_opt.map(|g| g.evaluate(t))
            .unwrap_or_else(|| {
                let sc = &sys.emitter.start_color;
                let c = sc.sample_color(0.5);
                Color32::from_rgba_unmultiplied(
                    (c[0]*255.0) as u8, (c[1]*255.0) as u8,
                    (c[2]*255.0) as u8, (c[3]*255.0) as u8,
                )
            });

        let size_t = size_opt.map(|s| s.evaluate(t)).unwrap_or(1.0).clamp(0.0, 1.0);
        let bar_h = rect.height() * size_t;
        let bar_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + i as f32 * step_w, rect.center().y - bar_h * 0.5),
            Vec2::new(step_w, bar_h),
        );
        painter.rect_filled(bar_rect, 0.0, col);
    }
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(60, 60, 70)), egui::StrokeKind::Outside);
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE EDITOR — extended show_panel with stats + thumbnail
// ─────────────────────────────────────────────────────────────────────────────

impl ParticleEditor {
    /// Show a compact thumbnail strip for all systems.
    pub fn show_system_thumbnails(ui: &mut egui::Ui, editor: &mut ParticleEditor) {
        ui.horizontal_wrapped(|ui| {
            let count = editor.systems.len();
            for i in 0..count {
                let sys = &editor.systems[i];
                let is_active = editor.active_system == i;
                let thumb_size = Vec2::new(80.0, 40.0);

                let (resp, painter) = ui.allocate_painter(thumb_size, egui::Sense::click());
                draw_system_thumbnail(&painter, resp.rect, sys);

                if is_active {
                    painter.rect_stroke(resp.rect, 2.0, Stroke::new(2.0, Color32::from_rgb(100, 180, 255)), egui::StrokeKind::Outside);
                }

                // Name label below thumbnail
                painter.text(
                    resp.rect.center_bottom() + Vec2::new(0.0, -2.0),
                    egui::Align2::CENTER_BOTTOM,
                    &sys.name,
                    FontId::proportional(9.0),
                    Color32::from_rgb(200, 200, 200),
                );

                if resp.clicked() {
                    editor.active_system = i;
                    editor.particles.clear();
                    editor.emission_accumulator = 0.0;
                    editor.preview_time = 0.0;
                    let mc = editor.systems[i].modules.len();
                    editor.module_expanded = vec![false; mc];
                    editor.selected_module = None;
                }
            }
        });
    }

    /// Complete panel with thumbnails, stats, and main editor.
    pub fn show_full(ctx: &egui::Context, editor: &mut ParticleEditor, dt: f32, open: &mut bool) {
        ParticleEditor::show_panel(ctx, editor, dt, open);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SERDE SUPPORT HELPERS
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a ParticleSystem to a JSON-compatible representation for clipboard/file I/O.
/// In practice, call sys.to_json() which uses serde_json::to_string_pretty.
pub fn serialize_system_to_string(sys: &ParticleSystem) -> String {
    sys.to_json().unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
}

/// Attempts to parse a ParticleSystem from a string, returning an error message on failure.
pub fn deserialize_system_from_string(s: &str) -> Result<ParticleSystem, String> {
    ParticleSystem::from_json(s)
}

// ─────────────────────────────────────────────────────────────────────────────
// UNDO / REDO STUBS
// ─────────────────────────────────────────────────────────────────────────────

/// Snapshot for undo/redo operations.
#[derive(Clone)]
pub struct ParticleEditorSnapshot {
    pub systems: Vec<ParticleSystem>,
    pub active_system: usize,
}

pub struct UndoStack {
    pub past: Vec<ParticleEditorSnapshot>,
    pub future: Vec<ParticleEditorSnapshot>,
    pub max_depth: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        UndoStack { past: Vec::new(), future: Vec::new(), max_depth: 50 }
    }
}

impl UndoStack {
    pub fn push(&mut self, snap: ParticleEditorSnapshot) {
        self.future.clear();
        self.past.push(snap);
        if self.past.len() > self.max_depth {
            self.past.remove(0);
        }
    }

    pub fn undo(&mut self, editor: &mut ParticleEditor) {
        if let Some(snap) = self.past.pop() {
            let current = ParticleEditorSnapshot {
                systems: editor.systems.clone(),
                active_system: editor.active_system,
            };
            self.future.push(current);
            editor.systems = snap.systems;
            editor.active_system = snap.active_system.min(editor.systems.len().saturating_sub(1));
            editor.particles.clear();
        }
    }

    pub fn redo(&mut self, editor: &mut ParticleEditor) {
        if let Some(snap) = self.future.pop() {
            let current = ParticleEditorSnapshot {
                systems: editor.systems.clone(),
                active_system: editor.active_system,
            };
            self.past.push(current);
            editor.systems = snap.systems;
            editor.active_system = snap.active_system.min(editor.systems.len().saturating_sub(1));
            editor.particles.clear();
        }
    }

    pub fn snapshot(&self, editor: &ParticleEditor) -> ParticleEditorSnapshot {
        ParticleEditorSnapshot {
            systems: editor.systems.clone(),
            active_system: editor.active_system,
        }
    }

    pub fn show_buttons(&mut self, ui: &mut egui::Ui, editor: &mut ParticleEditor) {
        ui.horizontal(|ui| {
            ui.add_enabled(!self.past.is_empty(), egui::Button::new("↩ Undo"));
            if ui.add_enabled(!self.past.is_empty(), egui::Button::new("↩ Undo")).clicked() {
                self.undo(editor);
            }
            if ui.add_enabled(!self.future.is_empty(), egui::Button::new("↪ Redo")).clicked() {
                self.redo(editor);
            }
            ui.label(format!("({}/{})", self.past.len(), self.future.len()));
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE SYSTEM VALIDATION
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ValidationWarning {
    pub level: ValidationLevel,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ValidationLevel {
    Info,
    Warning,
    Error,
}

impl ValidationLevel {
    pub fn color(&self) -> Color32 {
        match self {
            ValidationLevel::Info => Color32::from_rgb(100, 200, 255),
            ValidationLevel::Warning => Color32::from_rgb(255, 200, 50),
            ValidationLevel::Error => Color32::from_rgb(255, 80, 80),
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            ValidationLevel::Info => "ℹ",
            ValidationLevel::Warning => "⚠",
            ValidationLevel::Error => "✖",
        }
    }
}

pub fn validate_particle_system(sys: &ParticleSystem) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    if sys.max_particles == 0 {
        warnings.push(ValidationWarning {
            level: ValidationLevel::Error,
            message: "Max particles is 0 — no particles will spawn.".to_string(),
        });
    }

    if sys.emitter.emission_rate <= 0.0 && sys.emitter.emission_bursts.is_empty() {
        warnings.push(ValidationWarning {
            level: ValidationLevel::Warning,
            message: "No emission rate and no bursts — no particles will emit.".to_string(),
        });
    }

    match &sys.emitter.start_lifetime {
        RangeOrCurve::Constant(v) if *v <= 0.0 => {
            warnings.push(ValidationWarning {
                level: ValidationLevel::Error,
                message: "Start lifetime is 0 — particles will die immediately.".to_string(),
            });
        }
        RangeOrCurve::Random(lo, hi) if *hi <= 0.0 => {
            warnings.push(ValidationWarning {
                level: ValidationLevel::Error,
                message: "Start lifetime max is 0 — particles will die immediately.".to_string(),
            });
        }
        _ => {}
    }

    if sys.duration <= 0.0 && !sys.looping {
        warnings.push(ValidationWarning {
            level: ValidationLevel::Warning,
            message: "Duration is 0 and not looping — system will not run.".to_string(),
        });
    }

    // Check for conflicting modules
    let has_color_ol = sys.modules.iter().any(|m| matches!(m, ParticleModule::ColorOverLifetime { enabled: true, .. }));
    let has_color_bs = sys.modules.iter().any(|m| matches!(m, ParticleModule::ColorBySpeed { enabled: true, .. }));
    if has_color_ol && has_color_bs {
        warnings.push(ValidationWarning {
            level: ValidationLevel::Info,
            message: "Both Color over Lifetime and Color by Speed are active — results will blend.".to_string(),
        });
    }

    let has_size_ol = sys.modules.iter().any(|m| matches!(m, ParticleModule::SizeOverLifetime { enabled: true, .. }));
    let has_size_bs = sys.modules.iter().any(|m| matches!(m, ParticleModule::SizeBySpeed { enabled: true, .. }));
    if has_size_ol && has_size_bs {
        warnings.push(ValidationWarning {
            level: ValidationLevel::Info,
            message: "Both Size over Lifetime and Size by Speed are active.".to_string(),
        });
    }

    // Renderer check
    let has_renderer = sys.modules.iter().any(|m| matches!(m, ParticleModule::Renderer { enabled: true, .. }));
    if !has_renderer {
        warnings.push(ValidationWarning {
            level: ValidationLevel::Warning,
            message: "No Renderer module — particles may not be visible at runtime.".to_string(),
        });
    }

    if sys.max_particles > 10000 {
        warnings.push(ValidationWarning {
            level: ValidationLevel::Warning,
            message: format!("Max particles ({}) is very high — may impact performance.", sys.max_particles),
        });
    }

    warnings
}

pub fn show_validation_panel(ui: &mut egui::Ui, sys: &ParticleSystem) {
    let warnings = validate_particle_system(sys);
    if warnings.is_empty() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("✔ No issues").color(Color32::from_rgb(80, 200, 80)));
        });
        return;
    }

    egui::CollapsingHeader::new(format!("⚠ {} Issue(s)", warnings.len())).default_open(true).show(ui, |ui| {
        for w in &warnings {
            ui.horizontal(|ui| {
                ui.label(RichText::new(w.level.icon()).color(w.level.color()));
                ui.label(RichText::new(&w.message).color(w.level.color()));
            });
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// COLOR UTILITIES
// ─────────────────────────────────────────────────────────────────────────────

pub fn rgba_to_hsva(r: f32, g: f32, b: f32, a: f32) -> (f32, f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max > 1e-6 { delta / max } else { 0.0 };

    let h = if delta < 1e-6 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h, s, v, a)
}

pub fn hsva_to_rgba(h: f32, s: f32, v: f32, a: f32) -> (f32, f32, f32, f32) {
    if s < 1e-6 { return (v, v, v, a); }
    let h = h / 60.0;
    let i = h.floor() as u32 % 6;
    let f = h - h.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => (v, t, p, a),
        1 => (q, v, p, a),
        2 => (p, v, t, a),
        3 => (p, q, v, a),
        4 => (t, p, v, a),
        _ => (v, p, q, a),
    }
}

/// Blend two RGBA colors together.
pub fn blend_colors(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
        lerp(a[3], b[3], t),
    ]
}

pub fn color32_from_arr(c: [f32; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}

pub fn arr_from_color32(c: Color32) -> [f32; 4] {
    [c.r() as f32/255.0, c.g() as f32/255.0, c.b() as f32/255.0, c.a() as f32/255.0]
}

// ─────────────────────────────────────────────────────────────────────────────
// MATH UTILITIES
// ─────────────────────────────────────────────────────────────────────────────

pub fn remap(value: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = if (in_max - in_min).abs() < 1e-9 { 0.0 }
        else { (value - in_min) / (in_max - in_min) };
    lerp(out_min, out_max, t.clamp(0.0, 1.0))
}

pub fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Integrate a curve over [0,1] using simple trapezoid rule.
pub fn integrate_curve(curve: &Curve, steps: u32) -> f32 {
    if steps == 0 { return 0.0; }
    let mut sum = 0.0f32;
    let mut prev = curve.evaluate(0.0);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let curr = curve.evaluate(t);
        sum += (prev + curr) * 0.5 / steps as f32;
        prev = curr;
    }
    sum
}

/// Find the maximum value of a curve over [0,1].
pub fn curve_max(curve: &Curve, steps: u32) -> f32 {
    (0..=steps).map(|i| curve.evaluate(i as f32 / steps as f32))
        .fold(f32::NEG_INFINITY, f32::max)
}

/// Find the minimum value of a curve over [0,1].
pub fn curve_min(curve: &Curve, steps: u32) -> f32 {
    (0..=steps).map(|i| curve.evaluate(i as f32 / steps as f32))
        .fold(f32::INFINITY, f32::min)
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE EDITOR HELP / DOCUMENTATION PANEL
// ─────────────────────────────────────────────────────────────────────────────

pub fn show_help_panel(ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().id_source("ps_help_scroll").show(ui, |ui| {
        ui.heading("Particle System Editor Help");
        ui.separator();

        egui::CollapsingHeader::new("Getting Started").default_open(true).show(ui, |ui| {
            ui.label("1. Select or create a Particle System using the system selector in the toolbar.");
            ui.label("2. Configure the Emitter in the left panel: choose a shape, set emission rate, and adjust start values.");
            ui.label("3. Add modules in the right panel to control particle behavior over their lifetime.");
            ui.label("4. Press ▶ Play to see the preview simulation in the center panel.");
        });

        egui::CollapsingHeader::new("Emitter Shapes").show(ui, |ui| {
            egui::Grid::new("help_shapes").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Point"); ui.label("All particles spawn from a single point."); ui.end_row();
                ui.label("Sphere"); ui.label("Particles spawn from a sphere surface or volume."); ui.end_row();
                ui.label("Cone"); ui.label("Particles spawn and travel outward in a cone."); ui.end_row();
                ui.label("Box"); ui.label("Particles spawn throughout a rectangular volume."); ui.end_row();
                ui.label("Circle"); ui.label("Particles spawn on a circle or arc."); ui.end_row();
                ui.label("Edge"); ui.label("Particles spawn along a line segment."); ui.end_row();
                ui.label("Mesh"); ui.label("Particles spawn from mesh vertices/faces (runtime only)."); ui.end_row();
            });
        });

        egui::CollapsingHeader::new("Modules Overview").show(ui, |ui| {
            egui::Grid::new("help_mods").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Velocity over Lifetime"); ui.label("Add/subtract velocity as particles age."); ui.end_row();
                ui.label("Limit Velocity"); ui.label("Clamp speed and optionally dampen it."); ui.end_row();
                ui.label("Force over Lifetime"); ui.label("Apply constant or curve-based forces."); ui.end_row();
                ui.label("Color over Lifetime"); ui.label("Change particle color/alpha via gradient."); ui.end_row();
                ui.label("Color by Speed"); ui.label("Map particle speed to a gradient color."); ui.end_row();
                ui.label("Size over Lifetime"); ui.label("Scale particle size via curve."); ui.end_row();
                ui.label("Size by Speed"); ui.label("Map speed to size."); ui.end_row();
                ui.label("Rotation over Lifetime"); ui.label("Spin particles as they age."); ui.end_row();
                ui.label("Noise"); ui.label("Add turbulent noise forces."); ui.end_row();
                ui.label("Collision"); ui.label("Bounce particles off planes or world geometry."); ui.end_row();
                ui.label("Trails"); ui.label("Add trailing ribbons behind particles."); ui.end_row();
                ui.label("Renderer"); ui.label("Control how particles are drawn at runtime."); ui.end_row();
            });
        });

        egui::CollapsingHeader::new("Curve Editor").show(ui, |ui| {
            ui.label("• Click empty canvas to add a keyframe.");
            ui.label("• Click a keyframe dot to select it.");
            ui.label("• Drag a selected keyframe to move it.");
            ui.label("• Right-click a keyframe to change interpolation mode.");
            ui.label("• Interpolation modes: Linear, Smooth, Constant, Bezier.");
            ui.label("• Use the Y min/max controls to adjust the visible value range.");
            ui.label("• Press 'Fit' to auto-fit the range to your keyframe values.");
        });

        egui::CollapsingHeader::new("Gradient Editor").show(ui, |ui| {
            ui.label("• Click below the bar to add/select color stops.");
            ui.label("• Click above the bar to add/select alpha stops.");
            ui.label("• Drag stops horizontally to move them.");
            ui.label("• Select a stop then use the color picker or alpha slider to edit it.");
            ui.label("• Click 'Remove' to delete a selected stop (minimum 2 stops required).");
        });

        egui::CollapsingHeader::new("Presets").show(ui, |ui| {
            ui.label("Use the Presets menu in the toolbar to apply a preset to the current system.");
            ui.label("You can also save the current system as a custom preset for later reuse.");
        });

        egui::CollapsingHeader::new("Preview Controls").show(ui, |ui| {
            ui.label("• Drag the preview canvas to pan.");
            ui.label("• Scroll wheel to zoom in/out.");
            ui.label("• The green crosshair marks the emitter origin (0,0).");
            ui.label("• Use the Speed control to slow down or speed up the simulation.");
        });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE SYSTEM COPY / PASTE UTILITIES
// ─────────────────────────────────────────────────────────────────────────────

impl ParticleEditor {
    /// Copy the active system to a clipboard string via egui output.
    pub fn copy_system_to_clipboard(ui: &mut egui::Ui, editor: &ParticleEditor) {
        let json = serialize_system_to_string(&editor.systems[editor.active_system]);
        ui.ctx().copy_text(json);
    }

    /// Attempt to paste a system from a string, replacing the active system.
    pub fn paste_system_from_string(editor: &mut ParticleEditor, s: &str) -> bool {
        match deserialize_system_from_string(s) {
            Ok(sys) => {
                let mc = sys.modules.len();
                editor.systems[editor.active_system] = sys;
                editor.module_expanded = vec![false; mc];
                editor.selected_module = None;
                editor.particles.clear();
                editor.emission_accumulator = 0.0;
                true
            }
            Err(_) => false,
        }
    }

    /// Duplicate the active system and make the clone active.
    pub fn duplicate_active_system(editor: &mut ParticleEditor) {
        let mut clone = editor.systems[editor.active_system].clone();
        clone.name = format!("{} (copy)", clone.name);
        let mc = clone.modules.len();
        let insert_at = editor.active_system + 1;
        editor.systems.insert(insert_at, clone);
        editor.active_system = insert_at;
        editor.module_expanded = vec![false; mc];
        editor.selected_module = None;
        editor.particles.clear();
    }

    /// Remove all disabled modules from the active system.
    pub fn strip_disabled_modules(editor: &mut ParticleEditor) {
        editor.systems[editor.active_system].modules.retain(|m| m.is_enabled());
        let mc = editor.systems[editor.active_system].modules.len();
        editor.module_expanded = vec![false; mc];
        editor.selected_module = None;
    }

    /// Sort modules by category for cleaner organization.
    pub fn sort_modules_by_category(editor: &mut ParticleEditor) {
        let category_order = ["Physics", "Appearance", "Interaction", "Rendering"];
        editor.systems[editor.active_system].modules.sort_by(|a, b| {
            let ai = category_order.iter().position(|c| *c == a.category()).unwrap_or(99);
            let bi = category_order.iter().position(|c| *c == b.category()).unwrap_or(99);
            ai.cmp(&bi)
        });
        let mc = editor.systems[editor.active_system].modules.len();
        editor.module_expanded.resize(mc, false);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ADVANCED PREVIEW — Camera + particle info overlay
// ─────────────────────────────────────────────────────────────────────────────

/// Draws a velocity vector for each particle (for debugging).
pub fn draw_velocity_vectors(
    painter: &Painter,
    particles: &[PreviewParticle],
    center: Pos2,
    ppu: f32,
    scale: f32,
) {
    for p in particles {
        let pos = Pos2::new(center.x + p.position[0] * ppu, center.y - p.position[1] * ppu);
        let vel_end = Pos2::new(
            pos.x + p.velocity[0] * ppu * scale,
            pos.y - p.velocity[1] * ppu * scale,
        );
        let spd = (p.velocity[0] * p.velocity[0] + p.velocity[1] * p.velocity[1]).sqrt();
        let alpha = (spd * 20.0).clamp(0.0, 200.0) as u8;
        painter.line_segment([pos, vel_end], Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 200, 255, alpha)));
        // Arrow head
        let dir = (vel_end - pos);
        let len = dir.length();
        if len > 2.0 {
            let d = dir / len;
            let perp = Vec2::new(-d.y, d.x) * 3.0;
            let tip = vel_end;
            let base = vel_end - d * 5.0;
            let pts = vec![tip, base + perp, base - perp];
            painter.add(Shape::convex_polygon(pts, Color32::from_rgba_unmultiplied(100, 200, 255, alpha), Stroke::NONE));
        }
    }
}

/// Draw an acceleration field visualization (grid of force arrows).
pub fn draw_force_field_grid(
    painter: &Painter,
    rect: Rect,
    center: Pos2,
    ppu: f32,
    gravity: f32,
    noise_strength: f32,
    noise_frequency: f32,
    time: f32,
) {
    let grid_cols = 12u32;
    let grid_rows = 8u32;
    let cell_w = rect.width() / grid_cols as f32;
    let cell_h = rect.height() / grid_rows as f32;
    let arrow_scale = 10.0;

    for row in 0..grid_rows {
        for col in 0..grid_cols {
            let sx = rect.min.x + (col as f32 + 0.5) * cell_w;
            let sy = rect.min.y + (row as f32 + 0.5) * cell_h;
            let world_x = (sx - center.x) / ppu;
            let world_y = -(sy - center.y) / ppu;

            let fx = simple_noise(world_x * noise_frequency + time * 0.3, world_y * noise_frequency) * noise_strength;
            let fy = simple_noise(world_x * noise_frequency, world_y * noise_frequency + time * 0.3) * noise_strength - gravity * 9.8 * 0.05;

            let total_f = (fx*fx + fy*fy).sqrt();
            if total_f < 0.01 { continue; }

            let p0 = Pos2::new(sx, sy);
            let p1 = Pos2::new(sx + fx * arrow_scale, sy - fy * arrow_scale);
            let alpha = (total_f * 100.0).clamp(30.0, 150.0) as u8;
            painter.line_segment([p0, p1], Stroke::new(1.0, Color32::from_rgba_unmultiplied(180, 180, 80, alpha)));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SPAWN BURST TIMELINE — visual representation of burst events
// ─────────────────────────────────────────────────────────────────────────────

pub fn draw_burst_timeline(
    ui: &mut egui::Ui,
    bursts: &[Burst],
    duration: f32,
    current_time: f32,
) {
    let h = 40.0;
    let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), h), egui::Sense::hover());
    let rect = resp.rect;

    painter.rect_filled(rect, 2.0, Color32::from_rgb(20, 20, 28));

    // Duration marks
    let mark_interval = if duration <= 2.0 { 0.25 } else if duration <= 10.0 { 0.5 } else { 1.0 };
    let mut t = 0.0f32;
    while t <= duration + 1e-4 {
        let x = rect.min.x + (t / duration) * rect.width();
        let is_major = (t / mark_interval).round() as u32 % 4 == 0;
        let mark_h = if is_major { h * 0.6 } else { h * 0.3 };
        painter.line_segment(
            [Pos2::new(x, rect.max.y - mark_h), Pos2::new(x, rect.max.y)],
            Stroke::new(1.0, Color32::from_rgb(60, 60, 80)),
        );
        if is_major {
            painter.text(Pos2::new(x, rect.max.y), egui::Align2::CENTER_BOTTOM,
                format!("{:.1}", t), FontId::proportional(8.0), Color32::from_rgb(90, 90, 110));
        }
        t += mark_interval;
    }

    // Burst events
    for burst in bursts {
        if burst.time > duration { continue; }
        let x = rect.min.x + (burst.time / duration) * rect.width();
        // Main burst spike
        let spike_h = (burst.count as f32 / 200.0).clamp(0.1, 1.0) * (h - 8.0);
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(x - 2.0, rect.max.y - spike_h - 4.0), Vec2::new(4.0, spike_h)),
            1.0,
            Color32::from_rgb(255, 200, 60),
        );
        painter.text(Pos2::new(x, rect.max.y - spike_h - 6.0), egui::Align2::CENTER_BOTTOM,
            format!("{}", burst.count), FontId::proportional(8.0), Color32::from_rgb(255, 220, 100));

        // Repeat ticks
        if burst.repeat_count != 0 && burst.repeat_interval > 0.0 {
            let repeats = if burst.repeat_count < 0 {
                ((duration - burst.time) / burst.repeat_interval).floor() as u32
            } else {
                burst.repeat_count as u32
            };
            for r in 1..=repeats {
                let rt = burst.time + r as f32 * burst.repeat_interval;
                if rt > duration { break; }
                let rx = rect.min.x + (rt / duration) * rect.width();
                painter.circle_filled(Pos2::new(rx, rect.max.y - 6.0), 3.0, Color32::from_rgb(200, 160, 50));
            }
        }
    }

    // Current time playhead
    let px = rect.min.x + (current_time.min(duration) / duration) * rect.width();
    painter.line_segment(
        [Pos2::new(px, rect.min.y), Pos2::new(px, rect.max.y)],
        Stroke::new(1.5, Color32::WHITE),
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ADVANCED EMITTER CONFIG — emission over distance, rate by speed
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdvancedEmissionConfig {
    pub rate_over_distance: f32,
    pub rate_over_time_curve: Option<Curve>,
    pub max_particles_over_distance: f32,
}

impl Default for AdvancedEmissionConfig {
    fn default() -> Self {
        AdvancedEmissionConfig {
            rate_over_distance: 0.0,
            rate_over_time_curve: None,
            max_particles_over_distance: 0.0,
        }
    }
}

pub fn show_advanced_emission_ui(ui: &mut egui::Ui, cfg: &mut AdvancedEmissionConfig) -> bool {
    let mut changed = false;
    egui::CollapsingHeader::new("Advanced Emission").default_open(false).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Rate over Distance:");
            if ui.add(egui::DragValue::new(&mut cfg.rate_over_distance).speed(0.01).range(0.0..=100.0)).changed() {
                changed = true;
            }
        });
        if cfg.rate_over_time_curve.is_none() {
            if ui.small_button("Enable Rate-over-Time Curve").clicked() {
                cfg.rate_over_time_curve = Some(Curve::constant(1.0));
                changed = true;
            }
        } else {
            ui.label("Rate Multiplier Curve:");
            let c = cfg.rate_over_time_curve.as_ref().unwrap().clone();
            draw_curve_mini(&ui.painter_at(ui.max_rect()), ui.max_rect(), &c);
            if ui.small_button("Remove").clicked() {
                cfg.rate_over_time_curve = None;
                changed = true;
            }
        }
    });
    changed
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE SYSTEM TAGS / METADATA
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParticleSystemMetadata {
    pub tags: Vec<String>,
    pub description: String,
    pub author: String,
    pub created_date: String,
    pub version: u32,
}

impl ParticleSystemMetadata {
    pub fn show_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Metadata").default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Author:");
                if ui.text_edit_singleline(&mut self.author).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Description:");
                if ui.text_edit_multiline(&mut self.description).changed() { changed = true; }
            });
            ui.horizontal(|ui| {
                ui.label("Version:");
                let mut v = self.version as i32;
                if ui.add(egui::DragValue::new(&mut v).speed(1).range(0..=9999)).changed() {
                    self.version = v as u32;
                    changed = true;
                }
            });
            ui.label("Tags:");
            let mut to_remove = None;
            for (i, tag) in self.tags.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if ui.text_edit_singleline(tag).changed() { changed = true; }
                    if ui.small_button("✕").clicked() { to_remove = Some(i); }
                });
            }
            if let Some(i) = to_remove { self.tags.remove(i); changed = true; }
            if ui.small_button("+ Tag").clicked() {
                self.tags.push("new_tag".to_string());
                changed = true;
            }
        });
        changed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE COLOR SPACE
// ─────────────────────────────────────────────────────────────────────────────

/// Applies a simple "HDR bloom" tint to a color for preview purposes.
pub fn apply_hdr_preview(color: Color32, intensity: f32) -> Color32 {
    let r = (color.r() as f32 * intensity).min(255.0) as u8;
    let g = (color.g() as f32 * intensity).min(255.0) as u8;
    let b = (color.b() as f32 * intensity).min(255.0) as u8;
    Color32::from_rgba_unmultiplied(r, g, b, color.a())
}

/// Convert a Color32 to linear space (approximate sRGB -> linear).
pub fn srgb_to_linear(c: Color32) -> [f32; 4] {
    let r = (c.r() as f32 / 255.0).powf(2.2);
    let g = (c.g() as f32 / 255.0).powf(2.2);
    let b = (c.b() as f32 / 255.0).powf(2.2);
    let a = c.a() as f32 / 255.0;
    [r, g, b, a]
}

/// Convert linear color back to sRGB.
pub fn linear_to_srgb(c: [f32; 4]) -> Color32 {
    let r = (c[0].powf(1.0/2.2) * 255.0).clamp(0.0, 255.0) as u8;
    let g = (c[1].powf(1.0/2.2) * 255.0).clamp(0.0, 255.0) as u8;
    let b = (c[2].powf(1.0/2.2) * 255.0).clamp(0.0, 255.0) as u8;
    let a = (c[3] * 255.0).clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE SYSTEM PERFORMANCE ESTIMATOR
// ─────────────────────────────────────────────────────────────────────────────

pub struct PerformanceEstimate {
    pub fill_rate_estimate: f32, // approximate pixel-fills per second
    pub cpu_cost: f32,           // rough CPU load 0..1
    pub memory_bytes: usize,
    pub warnings: Vec<String>,
}

impl PerformanceEstimate {
    pub fn compute(sys: &ParticleSystem) -> Self {
        let avg_lifetime = match &sys.emitter.start_lifetime {
            RangeOrCurve::Constant(v) => *v,
            RangeOrCurve::Random(lo, hi) => (lo + hi) * 0.5,
            _ => 2.0,
        };

        let avg_particle_count = (sys.emitter.emission_rate * avg_lifetime)
            .min(sys.max_particles as f32);

        let avg_size = match &sys.emitter.start_size {
            RangeOrCurve::Constant(v) => *v,
            RangeOrCurve::Random(lo, hi) => (lo + hi) * 0.5,
            _ => 1.0,
        };

        // Assume 1080p, particles are avg_size * 50 pixels across
        let px_per_particle = (avg_size * 50.0).max(1.0);
        let fill_rate = avg_particle_count * px_per_particle * 60.0; // at 60fps

        let module_count = sys.modules.iter().filter(|m| m.is_enabled()).count();
        let has_trails = sys.modules.iter().any(|m| matches!(m, ParticleModule::Trails { enabled: true, .. }));
        let has_noise = sys.modules.iter().any(|m| matches!(m, ParticleModule::Noise { enabled: true, .. }));

        let mut cpu_cost = avg_particle_count / 1000.0;
        cpu_cost += module_count as f32 * 0.01;
        if has_trails { cpu_cost += avg_particle_count * 0.0005; }
        if has_noise { cpu_cost += avg_particle_count * 0.001; }
        cpu_cost = cpu_cost.clamp(0.0, 1.0);

        let bytes_per_particle = std::mem::size_of::<PreviewParticle>();
        let memory_bytes = sys.max_particles as usize * bytes_per_particle;

        let mut warnings = Vec::new();
        if fill_rate > 100_000_000.0 {
            warnings.push(format!("Fill rate ~{:.0}M px/frame may cause overdraw issues.", fill_rate / 1_000_000.0 / 60.0));
        }
        if cpu_cost > 0.5 {
            warnings.push("High CPU cost estimate — consider reducing particle count or module complexity.".to_string());
        }
        if has_trails && avg_particle_count > 200.0 {
            warnings.push("Trails with high particle count can be expensive.".to_string());
        }

        PerformanceEstimate { fill_rate_estimate: fill_rate, cpu_cost, memory_bytes, warnings }
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Performance Estimate").default_open(false).show(ui, |ui| {
            egui::Grid::new("perf_grid").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Fill Rate:");
                ui.label(format!("{:.1}M px/s", self.fill_rate_estimate / 1_000_000.0));
                ui.end_row();

                ui.label("CPU Cost:");
                let cpu_pct = self.cpu_cost * 100.0;
                let cpu_color = if cpu_pct > 50.0 { Color32::from_rgb(255,80,80) }
                    else if cpu_pct > 25.0 { Color32::from_rgb(255,200,50) }
                    else { Color32::from_rgb(80,200,80) };
                ui.label(RichText::new(format!("{:.1}%", cpu_pct)).color(cpu_color));
                ui.end_row();

                ui.label("Memory:");
                ui.label(format!("{} KB", self.memory_bytes / 1024));
                ui.end_row();
            });

            for w in &self.warnings {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠").color(Color32::from_rgb(255, 200, 50)));
                    ui.label(w);
                });
            }
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GRADIENT PRESETS PANEL
// ─────────────────────────────────────────────────────────────────────────────

pub struct GradientPresetPanel;

impl GradientPresetPanel {
    pub fn show(ui: &mut egui::Ui, target: &mut ColorGradient) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Gradient Presets").default_open(false).show(ui, |ui| {
            let presets: &[(&str, fn() -> ColorGradient)] = &[
                ("White → Clear", || ColorGradient::default()),
                ("Fire", || ColorGradient::fire()),
                ("Smoke", || ColorGradient::smoke()),
                ("Rainbow", || gradient_rainbow()),
                ("Ocean", || gradient_ocean()),
                ("Sunset", || gradient_sunset()),
                ("Lava", || gradient_lava()),
                ("Ice", || gradient_ice()),
                ("Nature", || gradient_nature()),
                ("Gold", || gradient_gold()),
                ("Electric", || gradient_electric()),
                ("Blood", || gradient_blood()),
            ];

            for chunk in presets.chunks(3) {
                ui.horizontal(|ui| {
                    for (name, make) in chunk {
                        let g = make();
                        let (resp, painter) = ui.allocate_painter(Vec2::new(70.0, 18.0), egui::Sense::click());
                        draw_checker(&painter, resp.rect, 6.0);
                        draw_gradient_bar_premul(&painter, resp.rect, &g);
                        if resp.hovered() {
                            painter.rect_stroke(resp.rect, 1.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Outside);
                        }
                        if resp.clicked() {
                            *target = g;
                            changed = true;
                        }
                        ui.label(RichText::new(*name).size(9.0));
                    }
                });
            }
        });
        changed
    }
}

fn gradient_ocean() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [0.0, 0.2, 0.8] },
            GradientColorKey { time: 0.5, color: [0.0, 0.5, 0.9] },
            GradientColorKey { time: 1.0, color: [0.7, 0.95, 1.0] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 1.0 },
            GradientAlphaKey { time: 1.0, alpha: 0.0 },
        ],
        mode: GradientMode::Blend,
    }
}

fn gradient_sunset() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [1.0, 0.6, 0.0] },
            GradientColorKey { time: 0.4, color: [1.0, 0.2, 0.3] },
            GradientColorKey { time: 0.7, color: [0.5, 0.0, 0.5] },
            GradientColorKey { time: 1.0, color: [0.1, 0.0, 0.2] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 1.0 },
            GradientAlphaKey { time: 1.0, alpha: 0.0 },
        ],
        mode: GradientMode::Blend,
    }
}

fn gradient_lava() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [1.0, 1.0, 0.0] },
            GradientColorKey { time: 0.3, color: [1.0, 0.3, 0.0] },
            GradientColorKey { time: 0.7, color: [0.5, 0.0, 0.0] },
            GradientColorKey { time: 1.0, color: [0.1, 0.0, 0.0] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 1.0 },
            GradientAlphaKey { time: 1.0, alpha: 0.8 },
        ],
        mode: GradientMode::Blend,
    }
}

fn gradient_ice() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [1.0, 1.0, 1.0] },
            GradientColorKey { time: 0.4, color: [0.7, 0.9, 1.0] },
            GradientColorKey { time: 1.0, color: [0.4, 0.7, 1.0] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 0.9 },
            GradientAlphaKey { time: 1.0, alpha: 0.0 },
        ],
        mode: GradientMode::Blend,
    }
}

fn gradient_nature() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [0.8, 1.0, 0.2] },
            GradientColorKey { time: 0.5, color: [0.1, 0.8, 0.1] },
            GradientColorKey { time: 1.0, color: [0.0, 0.4, 0.0] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 1.0 },
            GradientAlphaKey { time: 1.0, alpha: 0.0 },
        ],
        mode: GradientMode::Blend,
    }
}

fn gradient_gold() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [1.0, 0.9, 0.3] },
            GradientColorKey { time: 0.5, color: [1.0, 0.7, 0.0] },
            GradientColorKey { time: 1.0, color: [0.7, 0.4, 0.0] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 1.0 },
            GradientAlphaKey { time: 1.0, alpha: 0.0 },
        ],
        mode: GradientMode::Blend,
    }
}

fn gradient_electric() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [1.0, 1.0, 1.0] },
            GradientColorKey { time: 0.3, color: [0.8, 0.9, 1.0] },
            GradientColorKey { time: 0.7, color: [0.2, 0.4, 1.0] },
            GradientColorKey { time: 1.0, color: [0.0, 0.0, 0.5] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 1.0 },
            GradientAlphaKey { time: 0.6, alpha: 0.8 },
            GradientAlphaKey { time: 1.0, alpha: 0.0 },
        ],
        mode: GradientMode::Blend,
    }
}

fn gradient_blood() -> ColorGradient {
    ColorGradient {
        color_keys: vec![
            GradientColorKey { time: 0.0, color: [1.0, 0.1, 0.0] },
            GradientColorKey { time: 0.5, color: [0.6, 0.0, 0.0] },
            GradientColorKey { time: 1.0, color: [0.2, 0.0, 0.0] },
        ],
        alpha_keys: vec![
            GradientAlphaKey { time: 0.0, alpha: 1.0 },
            GradientAlphaKey { time: 0.8, alpha: 1.0 },
            GradientAlphaKey { time: 1.0, alpha: 0.0 },
        ],
        mode: GradientMode::Blend,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PARTICLE SYSTEM DIFF / COMPARE
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SystemDiff {
    pub field_diffs: Vec<FieldDiff>,
}

#[derive(Clone, Debug)]
pub struct FieldDiff {
    pub path: String,
    pub before: String,
    pub after: String,
}

pub fn diff_systems(a: &ParticleSystem, b: &ParticleSystem) -> SystemDiff {
    let mut diffs = Vec::new();

    if a.name != b.name {
        diffs.push(FieldDiff { path: "name".to_string(), before: a.name.clone(), after: b.name.clone() });
    }
    if a.max_particles != b.max_particles {
        diffs.push(FieldDiff {
            path: "max_particles".to_string(),
            before: a.max_particles.to_string(),
            after: b.max_particles.to_string(),
        });
    }
    if a.duration != b.duration {
        diffs.push(FieldDiff {
            path: "duration".to_string(),
            before: format!("{:.2}", a.duration),
            after: format!("{:.2}", b.duration),
        });
    }
    if a.looping != b.looping {
        diffs.push(FieldDiff {
            path: "looping".to_string(),
            before: a.looping.to_string(),
            after: b.looping.to_string(),
        });
    }
    if a.emitter.emission_rate != b.emitter.emission_rate {
        diffs.push(FieldDiff {
            path: "emitter.emission_rate".to_string(),
            before: format!("{:.2}", a.emitter.emission_rate),
            after: format!("{:.2}", b.emitter.emission_rate),
        });
    }
    if a.emitter.gravity_modifier != b.emitter.gravity_modifier {
        diffs.push(FieldDiff {
            path: "emitter.gravity_modifier".to_string(),
            before: format!("{:.3}", a.emitter.gravity_modifier),
            after: format!("{:.3}", b.emitter.gravity_modifier),
        });
    }
    if a.modules.len() != b.modules.len() {
        diffs.push(FieldDiff {
            path: "modules.len".to_string(),
            before: a.modules.len().to_string(),
            after: b.modules.len().to_string(),
        });
    }
    SystemDiff { field_diffs: diffs }
}

pub fn show_diff_panel(ui: &mut egui::Ui, diff: &SystemDiff) {
    if diff.field_diffs.is_empty() {
        ui.label(RichText::new("No differences.").color(Color32::from_rgb(80, 200, 80)));
        return;
    }
    ui.label(format!("{} change(s):", diff.field_diffs.len()));
    egui::Grid::new("diff_grid").num_columns(3).striped(true).show(ui, |ui| {
        ui.label(RichText::new("Field").strong());
        ui.label(RichText::new("Before").strong());
        ui.label(RichText::new("After").strong());
        ui.end_row();
        for d in &diff.field_diffs {
            ui.label(&d.path);
            ui.label(RichText::new(&d.before).color(Color32::from_rgb(255, 120, 120)));
            ui.label(RichText::new(&d.after).color(Color32::from_rgb(120, 255, 120)));
            ui.end_row();
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// KEYBOARD SHORTCUT HANDLER FOR PARTICLE EDITOR
// ─────────────────────────────────────────────────────────────────────────────

pub struct ParticleEditorShortcuts;

impl ParticleEditorShortcuts {
    /// Process keyboard shortcuts. Call from the show() function after ui is allocated.
    pub fn process(ctx: &egui::Context, editor: &mut ParticleEditor, undo: Option<&mut UndoStack>) {
        let space_pressed = ctx.input(|i| i.key_pressed(egui::Key::Space));
        let ctrl = ctx.input(|i| i.modifiers.ctrl);
        let z_pressed = ctx.input(|i| i.key_pressed(egui::Key::Z));
        let y_pressed = ctx.input(|i| i.key_pressed(egui::Key::Y));
        let d_pressed = ctx.input(|i| i.key_pressed(egui::Key::D));
        let r_pressed = ctx.input(|i| i.key_pressed(egui::Key::R));
        let s_pressed = ctx.input(|i| i.key_pressed(egui::Key::S));
        let n_pressed = ctx.input(|i| i.key_pressed(egui::Key::N));

        if space_pressed {
            editor.preview_running = !editor.preview_running;
        }
        if r_pressed && ctrl {
            // Restart
            editor.preview_running = true;
            editor.preview_time = 0.0;
            editor.particles.clear();
            editor.emission_accumulator = 0.0;
        }
        if ctrl && d_pressed {
            ParticleEditor::duplicate_active_system(editor);
        }
        if ctrl && n_pressed {
            let mut ps = ParticleSystem::default();
            ps.name = format!("System {}", editor.systems.len() + 1);
            editor.systems.push(ps);
            editor.active_system = editor.systems.len() - 1;
            let mc = editor.systems[editor.active_system].modules.len();
            editor.module_expanded = vec![false; mc];
            editor.selected_module = None;
            editor.particles.clear();
        }
        if let Some(undo_stack) = undo {
            if ctrl && z_pressed {
                undo_stack.undo(editor);
            }
            if ctrl && y_pressed {
                undo_stack.redo(editor);
            }
        }
    }

    pub fn show_shortcut_reference(ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Keyboard Shortcuts").default_open(false).show(ui, |ui| {
            egui::Grid::new("shortcuts_grid").num_columns(2).striped(true).show(ui, |ui| {
                ui.label("Space"); ui.label("Play / Pause preview"); ui.end_row();
                ui.label("Ctrl+R"); ui.label("Restart preview"); ui.end_row();
                ui.label("Ctrl+D"); ui.label("Duplicate active system"); ui.end_row();
                ui.label("Ctrl+N"); ui.label("New particle system"); ui.end_row();
                ui.label("Ctrl+Z"); ui.label("Undo"); ui.end_row();
                ui.label("Ctrl+Y"); ui.label("Redo"); ui.end_row();
            });
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MODULE SEARCH AND FILTER
// ─────────────────────────────────────────────────────────────────────────────

pub struct ModuleFilterState {
    pub search: String,
    pub show_physics: bool,
    pub show_appearance: bool,
    pub show_interaction: bool,
    pub show_rendering: bool,
    pub show_disabled: bool,
}

impl Default for ModuleFilterState {
    fn default() -> Self {
        ModuleFilterState {
            search: String::new(),
            show_physics: true,
            show_appearance: true,
            show_interaction: true,
            show_rendering: true,
            show_disabled: true,
        }
    }
}

impl ModuleFilterState {
    pub fn show_filter_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.text_edit_singleline(&mut self.search);
            ui.checkbox(&mut self.show_physics, "Physics");
            ui.checkbox(&mut self.show_appearance, "Appearance");
            ui.checkbox(&mut self.show_interaction, "Interaction");
            ui.checkbox(&mut self.show_rendering, "Rendering");
            ui.checkbox(&mut self.show_disabled, "Show Disabled");
        });
    }

    pub fn module_passes(&self, module: &ParticleModule) -> bool {
        if !self.show_disabled && !module.is_enabled() { return false; }

        let cat = module.category();
        let cat_pass = match cat {
            "Physics" => self.show_physics,
            "Appearance" => self.show_appearance,
            "Interaction" => self.show_interaction,
            "Rendering" => self.show_rendering,
            _ => true,
        };

        if !cat_pass { return false; }

        if !self.search.is_empty() {
            let s = self.search.to_lowercase();
            if !module.name().to_lowercase().contains(&s) { return false; }
        }

        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// COMPLETE PARTICLE EDITOR MAIN ENTRY WITH ALL FEATURES INTEGRATED
// ─────────────────────────────────────────────────────────────────────────────

/// Full-featured wrapper that integrates undo, shortcuts, stats, perf, and validation.
pub struct ParticleEditorApp {
    pub editor: ParticleEditor,
    pub undo: UndoStack,
    pub filter: ModuleFilterState,
    pub show_help: bool,
    pub show_shortcuts: bool,
    pub show_perf: bool,
    pub show_validation: bool,
    pub show_diff: bool,
    pub diff_snapshot: Option<ParticleSystem>,
    pub last_snap: Option<ParticleEditorSnapshot>,
    pub snap_timer: f32,
}

impl ParticleEditorApp {
    pub fn new() -> Self {
        ParticleEditorApp {
            editor: ParticleEditor::new(),
            undo: UndoStack::default(),
            filter: ModuleFilterState::default(),
            show_help: false,
            show_shortcuts: false,
            show_perf: false,
            show_validation: true,
            show_diff: false,
            diff_snapshot: None,
            last_snap: None,
            snap_timer: 0.0,
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, dt: f32) {
        ParticleEditorShortcuts::process(ctx, &mut self.editor, Some(&mut self.undo));

        // Auto-snapshot for undo (every 1 second if changed)
        self.snap_timer += dt;
        if self.snap_timer >= 1.0 {
            self.snap_timer = 0.0;
            let new_snap = self.undo.snapshot(&self.editor);
            if let Some(ref prev) = self.last_snap {
                if prev.active_system != new_snap.active_system
                    || !systems_equal(&prev.systems, &new_snap.systems)
                {
                    self.undo.push(prev.clone());
                    self.last_snap = Some(new_snap);
                }
            } else {
                self.last_snap = Some(new_snap);
            }
        }

        self.editor.simulate_step(dt);
    }

    pub fn show_ui(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("Particle System Editor")
            .open(open)
            .default_size([1200.0, 800.0])
            .min_size([700.0, 500.0])
            .resizable(true)
            .show(ctx, |ui| {
                // Toolbar
                egui::TopBottomPanel::top("pea_toolbar").show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        show_toolbar(ui, &mut self.editor);
                        ui.separator();
                        self.undo.show_buttons(ui, &mut self.editor);
                        ui.separator();
                        ui.toggle_value(&mut self.show_help, "?");
                        ui.toggle_value(&mut self.show_perf, "Perf");
                        ui.toggle_value(&mut self.show_validation, "Validate");
                    });
                });

                // Bottom stats/validation/perf panel
                egui::TopBottomPanel::bottom("pea_bottom").show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        if self.show_validation {
                            show_validation_panel(ui, &self.editor.systems[self.editor.active_system]);
                        }
                    });
                    if self.show_perf {
                        let perf = PerformanceEstimate::compute(&self.editor.systems[self.editor.active_system]);
                        perf.show(ui);
                    }
                });

                // Left panel
                egui::SidePanel::left("pea_left").default_width(280.0).show_inside(ui, |ui| {
                    show_left_panel(ui, &mut self.editor);
                    ParticleEditor::show_io_panel(ui, &mut self.editor);
                });

                // Right panel (module stack with filter)
                egui::SidePanel::right("pea_right").default_width(320.0).show_inside(ui, |ui| {
                    self.filter.show_filter_ui(ui);
                    ui.separator();
                    show_right_panel_filtered(ui, &mut self.editor, &self.filter);
                });

                // Center preview
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if self.show_help {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            show_help_panel(ui);
                        });
                    } else {
                        show_center_panel(ui, &mut self.editor);
                    }
                });
            });

        // Floating windows
        if self.editor.curve_editor_open {
            let mut open = true;
            egui::Window::new("Curve Editor").open(&mut open).default_size([500.0, 300.0])
                .resizable(true).show(ctx, |ui| {
                    show_curve_editor_window(ui, &mut self.editor);
                });
            self.editor.curve_editor_open = open;
        }

        if self.editor.gradient_editor_open {
            let mut open = true;
            egui::Window::new("Gradient Editor").open(&mut open).default_size([500.0, 220.0])
                .resizable(true).show(ctx, |ui| {
                    show_gradient_editor_window(ui, &mut self.editor);
                });
            self.editor.gradient_editor_open = open;
        }
    }
}

fn systems_equal(a: &[ParticleSystem], b: &[ParticleSystem]) -> bool {
    if a.len() != b.len() { return false; }
    for (x, y) in a.iter().zip(b.iter()) {
        if x.name != y.name || x.max_particles != y.max_particles
            || x.duration != y.duration || x.looping != y.looping
        { return false; }
    }
    true
}

fn show_right_panel_filtered(ui: &mut egui::Ui, editor: &mut ParticleEditor, filter: &ModuleFilterState) {
    ui.heading("Modules");
    if ui.button("+ Add Module").clicked() {
        editor.show_add_module_menu = !editor.show_add_module_menu;
    }
    // reuse the same add module menu logic
    if editor.show_add_module_menu {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_max_width(220.0);
            macro_rules! add_btn {
                ($label:expr, $make:expr) => {
                    if ui.button($label).clicked() {
                        add_module(editor, $make());
                        editor.show_add_module_menu = false;
                    }
                }
            }
            ui.label(RichText::new("Physics").strong());
            add_btn!("Velocity over Lifetime", default_velocity_over_lifetime);
            add_btn!("Limit Velocity over Lifetime", default_limit_velocity_over_lifetime);
            add_btn!("Force over Lifetime", default_force_over_lifetime);
            add_btn!("External Forces", default_external_forces);
            add_btn!("Noise", default_noise);
            ui.separator();
            ui.label(RichText::new("Appearance").strong());
            add_btn!("Color over Lifetime", default_color_over_lifetime);
            add_btn!("Color by Speed", default_color_by_speed);
            add_btn!("Size over Lifetime", default_size_over_lifetime);
            add_btn!("Size by Speed", default_size_by_speed);
            add_btn!("Rotation over Lifetime", default_rotation_over_lifetime);
            add_btn!("Rotation by Speed", default_rotation_by_speed);
            ui.separator();
            ui.label(RichText::new("Interaction").strong());
            add_btn!("Collision", default_collision);
            add_btn!("Triggers", default_triggers);
            add_btn!("Sub Emitters", default_sub_emitters);
            ui.separator();
            ui.label(RichText::new("Rendering").strong());
            add_btn!("Texture Sheet Animation", default_texture_sheet_animation);
            add_btn!("Lights", default_lights);
            add_btn!("Trails", default_trails);
            add_btn!("Renderer", default_renderer);
        });
    }
    ui.separator();

    // Sync expanded vec
    {
        let mc = editor.systems[editor.active_system].modules.len();
        if editor.module_expanded.len() != mc {
            editor.module_expanded.resize(mc, false);
        }
    }

    let mut move_up = None;
    let mut move_down = None;
    let mut remove_mod = None;
    let mod_count = editor.systems[editor.active_system].modules.len();

    egui::ScrollArea::vertical().id_source("rp_filtered_scroll").show(ui, |ui| {
        for i in 0..mod_count {
            let passes = {
                let m = &editor.systems[editor.active_system].modules[i];
                filter.module_passes(m)
            };
            if !passes { continue; }

            let enabled = editor.systems[editor.active_system].modules[i].is_enabled();
            let mod_name = editor.systems[editor.active_system].modules[i].name().to_string();
            let is_sel = editor.selected_module == Some(i);

            ui.horizontal(|ui| {
                let mut en = enabled;
                if ui.checkbox(&mut en, "").changed() {
                    editor.systems[editor.active_system].modules[i].set_enabled(en);
                }
                let txt = if is_sel {
                    RichText::new(&mod_name).strong().color(Color32::from_rgb(100, 180, 255))
                } else { RichText::new(&mod_name) };
                if ui.selectable_label(is_sel, txt).clicked() {
                    if editor.selected_module == Some(i) {
                        editor.module_expanded[i] = !editor.module_expanded[i];
                    } else {
                        editor.selected_module = Some(i);
                        editor.module_expanded[i] = true;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() { remove_mod = Some(i); }
                    if i + 1 < mod_count && ui.small_button("▼").clicked() { move_down = Some(i); }
                    if i > 0 && ui.small_button("▲").clicked() { move_up = Some(i); }
                });
            });

            if editor.module_expanded.get(i).copied().unwrap_or(false) {
                egui::Frame::default()
                    .inner_margin(egui::Margin::same(6))
                    .fill(Color32::from_rgb(35, 35, 40))
                    .show(ui, |ui| {
                        show_module_properties(ui, editor, i);
                    });
            }
            ui.separator();
        }
    });

    if let Some(i) = move_up { editor.systems[editor.active_system].modules.swap(i, i-1); editor.module_expanded.swap(i, i-1); }
    if let Some(i) = move_down { editor.systems[editor.active_system].modules.swap(i, i+1); editor.module_expanded.swap(i, i+1); }
    if let Some(i) = remove_mod {
        editor.systems[editor.active_system].modules.remove(i);
        editor.module_expanded.remove(i);
        if editor.selected_module == Some(i) { editor.selected_module = None; }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SUB-EMITTER SPAWNING
// ─────────────────────────────────────────────────────────────────────────────

/// Represents a pending sub-emitter spawn request.
#[derive(Clone, Debug)]
pub struct SubEmitterRequest {
    pub system_index: usize,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
}

pub fn process_sub_emitter_requests(editor: &mut ParticleEditor, requests: &[SubEmitterRequest]) {
    for req in requests {
        if req.system_index >= editor.systems.len() { continue; }
        let prev = editor.active_system;
        editor.active_system = req.system_index;
        editor.spawn_particle();
        if let Some(p) = editor.particles.last_mut() {
            p.position = req.position;
            p.velocity[0] += req.velocity[0];
            p.velocity[1] += req.velocity[1];
        }
        editor.active_system = prev;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LOD SYSTEM
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticleSystemLod {
    pub enabled: bool,
    pub near_distance: f32,
    pub far_distance: f32,
    pub near_rate_multiplier: f32,
    pub far_rate_multiplier: f32,
    pub near_max_particles: u32,
    pub far_max_particles: u32,
}

impl Default for ParticleSystemLod {
    fn default() -> Self {
        ParticleSystemLod {
            enabled: false, near_distance: 10.0, far_distance: 50.0,
            near_rate_multiplier: 1.0, far_rate_multiplier: 0.1,
            near_max_particles: 0, far_max_particles: 50,
        }
    }
}

impl ParticleSystemLod {
    pub fn effective_rate(&self, distance: f32, base_rate: f32) -> f32 {
        if !self.enabled { return base_rate; }
        let t = ((distance - self.near_distance) / (self.far_distance - self.near_distance)).clamp(0.0, 1.0);
        base_rate * lerp(self.near_rate_multiplier, self.far_rate_multiplier, t)
    }

    pub fn effective_max(&self, distance: f32, base_max: u32) -> u32 {
        if !self.enabled { return base_max; }
        if distance < self.near_distance { return if self.near_max_particles > 0 { self.near_max_particles } else { base_max }; }
        if distance > self.far_distance { return self.far_max_particles; }
        base_max
    }

    pub fn show_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        egui::CollapsingHeader::new("Level of Detail").default_open(false).show(ui, |ui| {
            if ui.checkbox(&mut self.enabled, "Enable LOD").changed() { changed = true; }
            if self.enabled {
                ui.horizontal(|ui| {
                    ui.label("Near:");
                    if ui.add(egui::DragValue::new(&mut self.near_distance).speed(0.5).range(0.0..=1000.0)).changed() { changed = true; }
                    ui.label("Far:");
                    if ui.add(egui::DragValue::new(&mut self.far_distance).speed(0.5).range(0.0..=10000.0)).changed() { changed = true; }
                });
                ui.horizontal(|ui| {
                    ui.label("Near mult:");
                    if ui.add(egui::DragValue::new(&mut self.near_rate_multiplier).speed(0.01).range(0.0..=1.0)).changed() { changed = true; }
                    ui.label("Far mult:");
                    if ui.add(egui::DragValue::new(&mut self.far_rate_multiplier).speed(0.01).range(0.0..=1.0)).changed() { changed = true; }
                });
            }
        });
        changed
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OSCILLOSCOPE WIDGET
// ─────────────────────────────────────────────────────────────────────────────

pub struct Oscilloscope {
    pub history: Vec<f32>,
    pub max_samples: usize,
    pub label: String,
    pub color: Color32,
}

impl Oscilloscope {
    pub fn new(label: impl Into<String>, color: Color32, max_samples: usize) -> Self {
        Oscilloscope { history: Vec::new(), max_samples, label: label.into(), color }
    }

    pub fn push(&mut self, v: f32) {
        self.history.push(v);
        if self.history.len() > self.max_samples { self.history.remove(0); }
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        let h = 40.0;
        let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width(), h), egui::Sense::hover());
        let rect = resp.rect;
        painter.rect_filled(rect, 1.0, Color32::from_rgb(15, 15, 20));
        if self.history.len() < 2 { return; }
        let max_v = self.history.iter().cloned().fold(f32::NEG_INFINITY, f32::max).max(1.0);
        let min_v = self.history.iter().cloned().fold(f32::INFINITY, f32::min).min(0.0);
        let n = self.max_samples.max(1) - 1;
        let pts: Vec<Pos2> = self.history.iter().enumerate().map(|(i, &v)| {
            let t = i as f32 / n as f32;
            let vn = if (max_v - min_v).abs() < 1e-6 { 0.5 } else { (v - min_v) / (max_v - min_v) };
            Pos2::new(rect.min.x + t * rect.width(), rect.max.y - vn * rect.height())
        }).collect();
        for i in 0..pts.len()-1 { painter.line_segment([pts[i], pts[i+1]], Stroke::new(1.0, self.color)); }
        painter.text(rect.min + Vec2::new(3.0, 2.0), egui::Align2::LEFT_TOP, &self.label, FontId::proportional(9.0), self.color);
        if let Some(&last) = self.history.last() {
            painter.text(rect.max - Vec2::new(3.0, 2.0), egui::Align2::RIGHT_BOTTOM, format!("{:.1}", last), FontId::proportional(9.0), self.color);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RUNTIME EVENTS
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum ParticleEvent {
    SystemStarted { system_name: String },
    SystemStopped { system_name: String },
    ParticleBorn { position: [f32; 2] },
    ParticleDied { position: [f32; 2], age: f32 },
    BurstFired { burst_index: usize, count: u32 },
    CollisionHit { position: [f32; 2], normal: [f32; 2] },
    TriggerEntered { position: [f32; 2] },
}

impl ParticleEvent {
    pub fn label(&self) -> &str {
        match self {
            ParticleEvent::SystemStarted { .. } => "System Started",
            ParticleEvent::SystemStopped { .. } => "System Stopped",
            ParticleEvent::ParticleBorn { .. } => "Particle Born",
            ParticleEvent::ParticleDied { .. } => "Particle Died",
            ParticleEvent::BurstFired { .. } => "Burst Fired",
            ParticleEvent::CollisionHit { .. } => "Collision Hit",
            ParticleEvent::TriggerEntered { .. } => "Trigger Entered",
        }
    }
}

pub struct ParticleEventLog {
    pub events: Vec<(f32, ParticleEvent)>,
    pub max_events: usize,
}

impl ParticleEventLog {
    pub fn new(max_events: usize) -> Self { ParticleEventLog { events: Vec::new(), max_events } }

    pub fn push(&mut self, time: f32, event: ParticleEvent) {
        self.events.push((time, event));
        if self.events.len() > self.max_events { self.events.remove(0); }
    }

    pub fn clear(&mut self) { self.events.clear(); }

    pub fn show(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Event Log").default_open(false).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(120.0).id_source("pel_scroll").show(ui, |ui| {
                for (t, ev) in self.events.iter().rev().take(50) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("[{:.2}]", t)).color(Color32::from_rgb(120, 120, 140)).size(9.0));
                        ui.label(RichText::new(ev.label()).size(9.0));
                    });
                }
            });
        });
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// COLOR / MATH UTILITIES (unique extras)
// ─────────────────────────────────────────────────────────────────────────────

pub fn remap_val(value: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = if (in_max - in_min).abs() < 1e-9 { 0.0 } else { (value - in_min) / (in_max - in_min) };
    lerp(out_min, out_max, t.clamp(0.0, 1.0))
}

pub fn curve_value_max(curve: &Curve, steps: u32) -> f32 {
    (0..=steps).map(|i| curve.evaluate(i as f32 / steps as f32)).fold(f32::NEG_INFINITY, f32::max)
}

pub fn curve_value_min(curve: &Curve, steps: u32) -> f32 {
    (0..=steps).map(|i| curve.evaluate(i as f32 / steps as f32)).fold(f32::INFINITY, f32::min)
}

// ─────────────────────────────────────────────────────────────────────────────
// BATCH EDITOR
// ─────────────────────────────────────────────────────────────────────────────

pub struct BatchEditOp { pub field: BatchField, pub value: f32, pub enabled: bool }

#[derive(Clone, Debug, PartialEq)]
pub enum BatchField {
    EmissionRate, MaxParticles, Duration, GravityModifier, StartSize, StartSpeed, StartLifetime,
}

impl BatchField {
    pub fn label(&self) -> &str {
        match self {
            BatchField::EmissionRate => "Emission Rate",
            BatchField::MaxParticles => "Max Particles",
            BatchField::Duration => "Duration",
            BatchField::GravityModifier => "Gravity",
            BatchField::StartSize => "Start Size",
            BatchField::StartSpeed => "Start Speed",
            BatchField::StartLifetime => "Start Lifetime",
        }
    }
}

pub fn apply_batch_edit(systems: &mut [ParticleSystem], selected: &[usize], op: &BatchEditOp) {
    if !op.enabled { return; }
    for &idx in selected {
        if let Some(s) = systems.get_mut(idx) {
            match op.field {
                BatchField::EmissionRate => s.emitter.emission_rate = op.value,
                BatchField::MaxParticles => s.max_particles = op.value as u32,
                BatchField::Duration => s.duration = op.value,
                BatchField::GravityModifier => s.emitter.gravity_modifier = op.value,
                BatchField::StartSize => s.emitter.start_size = RangeOrCurve::Constant(op.value),
                BatchField::StartSpeed => s.emitter.start_speed = RangeOrCurve::Constant(op.value),
                BatchField::StartLifetime => s.emitter.start_lifetime = RangeOrCurve::Constant(op.value),
            }
        }
    }
}

pub struct BatchEditor { pub selected_systems: Vec<bool>, pub ops: Vec<BatchEditOp> }

impl BatchEditor {
    pub fn new(n: usize) -> Self {
        BatchEditor {
            selected_systems: vec![false; n],
            ops: vec![
                BatchEditOp { field: BatchField::EmissionRate, value: 10.0, enabled: false },
                BatchEditOp { field: BatchField::MaxParticles, value: 1000.0, enabled: false },
                BatchEditOp { field: BatchField::Duration, value: 5.0, enabled: false },
                BatchEditOp { field: BatchField::GravityModifier, value: 0.0, enabled: false },
            ],
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, systems: &mut Vec<ParticleSystem>) {
        egui::CollapsingHeader::new("Batch Edit").default_open(false).show(ui, |ui| {
            self.selected_systems.resize(systems.len(), false);
            ui.label("Select systems:");
            for (i, s) in systems.iter().enumerate() {
                ui.checkbox(&mut self.selected_systems[i], &s.name);
            }
            ui.separator();
            ui.label("Operations:");
            for op in &mut self.ops {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut op.enabled, op.field.label());
                    if op.enabled {
                        ui.add(egui::DragValue::new(&mut op.value).speed(0.1));
                    }
                });
            }
            let sel: Vec<usize> = self.selected_systems.iter().enumerate()
                .filter_map(|(i, &s)| if s { Some(i) } else { None }).collect();
            if !sel.is_empty() && ui.button(format!("Apply to {} systems", sel.len())).clicked() {
                for op in &self.ops { apply_batch_edit(systems, &sel, op); }
            }
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SYSTEM INSPECTOR
// ─────────────────────────────────────────────────────────────────────────────

pub fn show_system_inspector(ui: &mut egui::Ui, sys: &ParticleSystem) {
    egui::CollapsingHeader::new(format!("Inspector: {}", sys.name)).default_open(false).show(ui, |ui| {
        egui::Grid::new("inspector_final").num_columns(2).striped(true).show(ui, |ui| {
            ui.label("Max Particles"); ui.label(sys.max_particles.to_string()); ui.end_row();
            ui.label("Duration"); ui.label(format!("{:.2}s", sys.duration)); ui.end_row();
            ui.label("Looping"); ui.label(sys.looping.to_string()); ui.end_row();
            ui.label("Emission Rate"); ui.label(format!("{:.1}/s", sys.emitter.emission_rate)); ui.end_row();
            ui.label("Shape"); ui.label(sys.emitter.shape.label()); ui.end_row();
            ui.label("Gravity"); ui.label(format!("{:.3}", sys.emitter.gravity_modifier)); ui.end_row();
            ui.label("Modules"); ui.label(format!("{} ({} active)", sys.modules.len(), sys.modules.iter().filter(|m| m.is_enabled()).count())); ui.end_row();
        });
        ui.separator();
        for m in &sys.modules {
            let col = if m.is_enabled() { Color32::from_rgb(180, 220, 180) } else { Color32::from_rgb(100, 100, 100) };
            ui.label(RichText::new(format!("  {} {}", if m.is_enabled() { "+" } else { "-" }, m.name())).color(col).size(10.0));
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// CURVE LIBRARY
// ─────────────────────────────────────────────────────────────────────────────

pub struct CurveLibrary { pub entries: Vec<(String, Curve)> }

impl CurveLibrary {
    pub fn with_defaults() -> Self {
        CurveLibrary {
            entries: vec![
                ("0 to 1".to_string(), Curve::linear_zero_to_one()),
                ("1 to 0".to_string(), Curve::linear_one_to_zero()),
                ("Bell".to_string(), Curve::bell()),
                ("Bounce".to_string(), Curve::bounce()),
                ("Pulse".to_string(), Curve::pulse()),
                ("Ease In/Out".to_string(), Curve::ease_in_out()),
                ("Constant 1".to_string(), Curve::constant(1.0)),
                ("Constant 0".to_string(), Curve::constant(0.0)),
            ],
        }
    }

    pub fn show(&self, ui: &mut egui::Ui) -> Option<Curve> {
        let mut result = None;
        egui::CollapsingHeader::new("Curve Library").default_open(false).show(ui, |ui| {
            for (name, curve) in &self.entries {
                ui.horizontal(|ui| {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(60.0, 18.0), egui::Sense::click());
                    draw_curve_mini(ui.painter(), r, curve);
                    if resp.hovered() { ui.painter().rect_stroke(r, 1.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Outside); }
                    ui.label(RichText::new(name).size(10.0));
                    if resp.clicked() { result = Some(curve.clone()); }
                });
            }
        });
        result
    }

    pub fn add(&mut self, name: String, curve: Curve) { self.entries.push((name, curve)); }
    pub fn remove(&mut self, idx: usize) { if idx < self.entries.len() { self.entries.remove(idx); } }
}

// ─────────────────────────────────────────────────────────────────────────────
// ASSET BROWSER
// ─────────────────────────────────────────────────────────────────────────────

pub struct ParticleAssetRecord { pub path: String, pub name: String, pub tags: Vec<String>, pub preset_hint: String }
pub struct ParticleAssetBrowser { pub records: Vec<ParticleAssetRecord>, pub filter: String, pub selected: Option<usize> }

impl ParticleAssetBrowser {
    pub fn new() -> Self {
        ParticleAssetBrowser {
            selected: None,
            filter: String::new(),
            records: vec![
                ParticleAssetRecord { path: "vfx/fire.json".into(), name: "Fire".into(), tags: vec!["fire".into(), "vfx".into()], preset_hint: "Fire".into() },
                ParticleAssetRecord { path: "vfx/smoke.json".into(), name: "Smoke".into(), tags: vec!["smoke".into(), "vfx".into()], preset_hint: "Smoke".into() },
                ParticleAssetRecord { path: "vfx/sparks.json".into(), name: "Sparks".into(), tags: vec!["sparks".into()], preset_hint: "Sparks".into() },
                ParticleAssetRecord { path: "vfx/explosion.json".into(), name: "Explosion".into(), tags: vec!["combat".into(), "vfx".into()], preset_hint: "Explosion".into() },
                ParticleAssetRecord { path: "vfx/heal.json".into(), name: "Heal".into(), tags: vec!["ui".into(), "healing".into()], preset_hint: "Heal".into() },
                ParticleAssetRecord { path: "vfx/stars.json".into(), name: "Stars".into(), tags: vec!["ambient".into()], preset_hint: "Stars".into() },
                ParticleAssetRecord { path: "vfx/confetti.json".into(), name: "Confetti".into(), tags: vec!["celebration".into(), "ui".into()], preset_hint: "Confetti".into() },
                ParticleAssetRecord { path: "vfx/rain.json".into(), name: "Rain".into(), tags: vec!["weather".into()], preset_hint: "Rain".into() },
                ParticleAssetRecord { path: "vfx/snow.json".into(), name: "Snow".into(), tags: vec!["weather".into()], preset_hint: "Snow".into() },
                ParticleAssetRecord { path: "vfx/portal.json".into(), name: "Portal".into(), tags: vec!["magic".into()], preset_hint: "Portal".into() },
                ParticleAssetRecord { path: "vfx/poison.json".into(), name: "Poison".into(), tags: vec!["combat".into(), "magic".into()], preset_hint: "Poison".into() },
                ParticleAssetRecord { path: "vfx/magic_dust.json".into(), name: "Magic Dust".into(), tags: vec!["magic".into()], preset_hint: "Magic Dust".into() },
            ],
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, editor: &mut ParticleEditor) {
        ui.heading("Particle Assets");
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.filter);
            if ui.small_button("Clear").clicked() { self.filter.clear(); }
        });
        ui.separator();

        let filter_lower = self.filter.to_lowercase();

        egui::ScrollArea::vertical().id_source("pab_final").show(ui, |ui| {
            for (i, rec) in self.records.iter().enumerate() {
                let matches = filter_lower.is_empty()
                    || rec.name.to_lowercase().contains(&filter_lower)
                    || rec.tags.iter().any(|t| t.to_lowercase().contains(&filter_lower));
                if !matches { continue; }

                let is_sel = self.selected == Some(i);
                ui.horizontal(|ui| {
                    if ui.selectable_label(is_sel, RichText::new(&rec.name).strong()).clicked() {
                        self.selected = if is_sel { None } else { Some(i) };
                    }
                    ui.label(RichText::new(rec.tags.join(", ")).size(9.0).color(Color32::from_rgb(100, 100, 130)));
                });

                if is_sel {
                    ui.indent("asset_actions", |ui| {
                        ui.label(RichText::new(&rec.path).size(9.0).color(Color32::from_rgb(120, 120, 150)));
                        ui.horizontal(|ui| {
                            if ui.button("Load Preset").clicked() {
                                apply_preset(editor, &rec.preset_hint);
                            }
                            if ui.button("Load + Duplicate").clicked() {
                                apply_preset(editor, &rec.preset_hint);
                                ParticleEditor::duplicate_active_system(editor);
                            }
                        });
                    });
                }
            }
        });
    }
}

// =============================================================================
// RIBBON / TRAIL PARTICLE SYSTEM
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RibbonTextureMode {
    Stretch,
    Tile,
    DistributedDirectional,
    RepeatPerSegment,
}

impl Default for RibbonTextureMode {
    fn default() -> Self { RibbonTextureMode::Stretch }
}

impl RibbonTextureMode {
    pub fn label(&self) -> &str {
        match self {
            RibbonTextureMode::Stretch => "Stretch",
            RibbonTextureMode::Tile => "Tile",
            RibbonTextureMode::DistributedDirectional => "Distributed Directional",
            RibbonTextureMode::RepeatPerSegment => "Repeat Per Segment",
        }
    }
    pub fn all() -> &'static [RibbonTextureMode] {
        &[
            RibbonTextureMode::Stretch,
            RibbonTextureMode::Tile,
            RibbonTextureMode::DistributedDirectional,
            RibbonTextureMode::RepeatPerSegment,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RibbonEmitter {
    pub enabled: bool,
    pub width_curve: Curve,
    pub texture_mode: RibbonTextureMode,
    pub uv_speed: f32,
    pub min_vertex_distance: f32,
    pub max_ribbon_count: u32,
    pub lifetime: f32,
    pub inherit_velocity: f32,
    pub color_over_lifetime: ColorGradient,
    pub shadow_casting: bool,
    pub receive_shadows: bool,
    pub use_world_velocity: bool,
    pub ribbon_count: u32,
    pub split_sub_emitter_ribbons: bool,
    pub attach_ribbons_to_transform: bool,
    pub end_cap_vertices: u32,
    pub loop_ribbons: bool,
    pub generate_lighting_data: bool,
    pub stretch_over_lifetime: bool,
}

impl Default for RibbonEmitter {
    fn default() -> Self {
        RibbonEmitter {
            enabled: false,
            width_curve: Curve::constant(0.1),
            texture_mode: RibbonTextureMode::Stretch,
            uv_speed: 0.0,
            min_vertex_distance: 0.1,
            max_ribbon_count: 1,
            lifetime: 1.0,
            inherit_velocity: 0.0,
            color_over_lifetime: ColorGradient::default(),
            shadow_casting: false,
            receive_shadows: true,
            use_world_velocity: false,
            ribbon_count: 1,
            split_sub_emitter_ribbons: false,
            attach_ribbons_to_transform: false,
            end_cap_vertices: 0,
            loop_ribbons: false,
            generate_lighting_data: false,
            stretch_over_lifetime: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrailRenderer {
    pub enabled: bool,
    pub trail_width: Curve,
    pub trail_color: ColorGradient,
    pub trail_lifetime: f32,
    pub min_vertex_distance: f32,
    pub shadow_bias: f32,
    pub generate_lighting_data: bool,
    pub attach_ribbons_to_transform: bool,
    pub die_with_particles: bool,
    pub texture_mode: RibbonTextureMode,
    pub size_affects_width: bool,
    pub size_affects_lifetime: bool,
    pub inherit_particle_color: bool,
    pub width_over_trail: Curve,
    pub ratio_of_trail_to_particle_lifetime: f32,
    pub num_corner_vertices: u32,
    pub num_cap_vertices: u32,
    pub alignment: BillboardAlignment,
}

impl Default for TrailRenderer {
    fn default() -> Self {
        TrailRenderer {
            enabled: false,
            trail_width: Curve::constant(0.05),
            trail_color: ColorGradient::default(),
            trail_lifetime: 0.5,
            min_vertex_distance: 0.1,
            shadow_bias: 0.5,
            generate_lighting_data: false,
            attach_ribbons_to_transform: false,
            die_with_particles: true,
            texture_mode: RibbonTextureMode::Stretch,
            size_affects_width: true,
            size_affects_lifetime: false,
            inherit_particle_color: true,
            width_over_trail: Curve::linear_one_to_zero(),
            ratio_of_trail_to_particle_lifetime: 1.0,
            num_corner_vertices: 0,
            num_cap_vertices: 0,
            alignment: BillboardAlignment::View,
        }
    }
}

/// A single point in a ribbon: 2D canvas position, timestamp, color, width.
#[derive(Clone, Debug)]
pub struct RibbonPoint {
    pub pos: [f32; 2],
    pub time: f32,
    pub color: Color32,
    pub width: f32,
}

/// State for one live ribbon instance during preview.
#[derive(Clone, Debug)]
pub struct RibbonInstance {
    pub id: u32,
    pub points: std::collections::VecDeque<RibbonPoint>,
    pub velocity: [f32; 2],
    pub age: f32,
    pub max_age: f32,
}

impl RibbonInstance {
    pub fn new(id: u32, _start: [f32; 2], velocity: [f32; 2], lifetime: f32) -> Self {
        RibbonInstance {
            id,
            points: std::collections::VecDeque::new(),
            velocity,
            age: 0.0,
            max_age: lifetime,
        }
    }

    pub fn is_alive(&self) -> bool { self.age < self.max_age }

    pub fn add_point(&mut self, pos: [f32; 2], time: f32, color: Color32, width: f32, min_dist: f32) {
        if let Some(last) = self.points.back() {
            let dx = pos[0] - last.pos[0];
            let dy = pos[1] - last.pos[1];
            if (dx * dx + dy * dy).sqrt() < min_dist { return; }
        }
        self.points.push_back(RibbonPoint { pos, time, color, width });
    }

    pub fn prune_old_points(&mut self, current_time: f32, lifetime: f32) {
        while let Some(front) = self.points.front() {
            if current_time - front.time > lifetime { self.points.pop_front(); } else { break; }
        }
    }
}

/// Overall ribbon preview state for the particle editor canvas.
#[derive(Clone, Debug, Default)]
pub struct RibbonPreviewState {
    pub active_ribbons: Vec<RibbonInstance>,
    pub next_id: u32,
    pub simulation_time: f32,
    pub emit_accumulator: f32,
}

impl RibbonPreviewState {
    pub fn new() -> Self { RibbonPreviewState::default() }

    pub fn spawn_ribbon(&mut self, pos: [f32; 2], velocity: [f32; 2], lifetime: f32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.active_ribbons.push(RibbonInstance::new(id, pos, velocity, lifetime));
        id
    }

    pub fn tick(&mut self, dt: f32, emitter: &RibbonEmitter, canvas_origin: [f32; 2]) {
        self.simulation_time += dt;
        for ribbon in &mut self.active_ribbons {
            ribbon.age += dt;
            ribbon.velocity[1] += 50.0 * dt;
            let last_pos = ribbon.points.back().map(|p| p.pos).unwrap_or(canvas_origin);
            let new_pos = [last_pos[0] + ribbon.velocity[0] * dt, last_pos[1] + ribbon.velocity[1] * dt];
            let life_frac = (ribbon.age / ribbon.max_age).clamp(0.0, 1.0);
            let color = emitter.color_over_lifetime.evaluate(life_frac);
            let width = emitter.width_curve.evaluate(life_frac);
            ribbon.add_point(new_pos, self.simulation_time, color, width, emitter.min_vertex_distance);
            ribbon.prune_old_points(self.simulation_time, emitter.lifetime);
        }
        self.active_ribbons.retain(|r| r.is_alive() && !r.points.is_empty());
        self.emit_accumulator += dt * emitter.max_ribbon_count as f32;
        while self.emit_accumulator >= 1.0 {
            self.emit_accumulator -= 1.0;
            if (self.active_ribbons.len() as u32) < emitter.max_ribbon_count {
                let angle = (self.simulation_time * 3.7) % std::f32::consts::TAU;
                let speed = 80.0f32;
                self.spawn_ribbon(canvas_origin, [angle.cos() * speed, angle.sin() * speed - 60.0], emitter.lifetime);
            }
        }
    }

    pub fn draw(&self, painter: &Painter, rect: Rect) {
        for ribbon in &self.active_ribbons {
            let pts: Vec<_> = ribbon.points.iter().collect();
            if pts.len() < 2 { continue; }
            for i in 0..pts.len() - 1 {
                let a = pts[i];
                let b = pts[i + 1];
                let w = lerp(a.width, b.width, 0.5).max(0.5);
                let alpha_factor = 1.0 - i as f32 / pts.len() as f32;
                let col = Color32::from_rgba_unmultiplied(a.color.r(), a.color.g(), a.color.b(), (a.color.a() as f32 * alpha_factor) as u8);
                painter.line_segment(
                    [Pos2::new(rect.min.x + a.pos[0], rect.min.y + a.pos[1]), Pos2::new(rect.min.x + b.pos[0], rect.min.y + b.pos[1])],
                    Stroke::new(w, col),
                );
            }
        }
    }
}

pub fn show_ribbon_emitter_ui(ui: &mut egui::Ui, emitter: &mut RibbonEmitter) {
    egui::CollapsingHeader::new("Ribbon Emitter").default_open(false).show(ui, |ui| {
        ui.checkbox(&mut emitter.enabled, "Enabled");
        if !emitter.enabled { return; }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Max Ribbons:");
            ui.add(egui::DragValue::new(&mut emitter.max_ribbon_count).speed(1.0).range(1..=256));
        });
        ui.horizontal(|ui| {
            ui.label("Lifetime:");
            ui.add(egui::DragValue::new(&mut emitter.lifetime).speed(0.01).range(0.01..=60.0));
            ui.label("s");
        });
        ui.horizontal(|ui| {
            ui.label("UV Speed:");
            ui.add(egui::DragValue::new(&mut emitter.uv_speed).speed(0.01));
        });
        ui.horizontal(|ui| {
            ui.label("Min Vertex Dist:");
            ui.add(egui::DragValue::new(&mut emitter.min_vertex_distance).speed(0.001).range(0.001..=10.0));
        });
        ui.horizontal(|ui| {
            ui.label("Inherit Velocity:");
            ui.add(egui::DragValue::new(&mut emitter.inherit_velocity).speed(0.01).range(0.0..=1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Texture Mode:");
            egui::ComboBox::from_id_source("ribbon_tex_mode")
                .selected_text(emitter.texture_mode.label())
                .show_ui(ui, |ui| {
                    for m in RibbonTextureMode::all() {
                        ui.selectable_value(&mut emitter.texture_mode, m.clone(), m.label());
                    }
                });
        });
        ui.checkbox(&mut emitter.attach_ribbons_to_transform, "Attach to Transform");
        ui.checkbox(&mut emitter.generate_lighting_data, "Generate Lighting Data");
        ui.checkbox(&mut emitter.shadow_casting, "Shadow Casting");
        ui.checkbox(&mut emitter.receive_shadows, "Receive Shadows");
        ui.checkbox(&mut emitter.loop_ribbons, "Loop Ribbons");
        ui.separator();
        ui.label("Width Curve:");
        draw_curve_editor_inline(ui, &mut emitter.width_curve, 200.0, 40.0);
    });
}

pub fn show_trail_renderer_ui(ui: &mut egui::Ui, trail: &mut TrailRenderer) {
    egui::CollapsingHeader::new("Trail Renderer").default_open(false).show(ui, |ui| {
        ui.checkbox(&mut trail.enabled, "Enabled");
        if !trail.enabled { return; }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Trail Lifetime:");
            ui.add(egui::DragValue::new(&mut trail.trail_lifetime).speed(0.01).range(0.001..=60.0));
            ui.label("s");
        });
        ui.horizontal(|ui| {
            ui.label("Min Vertex Dist:");
            ui.add(egui::DragValue::new(&mut trail.min_vertex_distance).speed(0.001).range(0.001..=10.0));
        });
        ui.horizontal(|ui| {
            ui.label("Shadow Bias:");
            ui.add(egui::DragValue::new(&mut trail.shadow_bias).speed(0.01));
        });
        ui.horizontal(|ui| {
            ui.label("Lifetime Ratio:");
            ui.add(egui::DragValue::new(&mut trail.ratio_of_trail_to_particle_lifetime).speed(0.01).range(0.0..=1.0));
        });
        ui.checkbox(&mut trail.generate_lighting_data, "Generate Lighting Data");
        ui.checkbox(&mut trail.attach_ribbons_to_transform, "Attach to Transform");
        ui.checkbox(&mut trail.die_with_particles, "Die With Particles");
        ui.checkbox(&mut trail.size_affects_width, "Size Affects Width");
        ui.checkbox(&mut trail.size_affects_lifetime, "Size Affects Lifetime");
        ui.checkbox(&mut trail.inherit_particle_color, "Inherit Particle Color");
        ui.separator();
        ui.label("Width Over Trail:");
        draw_curve_editor_inline(ui, &mut trail.width_over_trail, 200.0, 40.0);
    });
}

fn draw_curve_editor_inline(ui: &mut egui::Ui, curve: &mut Curve, width: f32, height: f32) {
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    draw_curve_mini(ui.painter(), rect, curve);
    ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Outside);
}

// =============================================================================
// MESH PARTICLE SYSTEM
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MeshType {
    Sphere,
    Cube,
    Cylinder,
    Cone,
    Torus,
    Quad,
    Custom(String),
}

impl Default for MeshType {
    fn default() -> Self { MeshType::Quad }
}

impl MeshType {
    pub fn label(&self) -> &str {
        match self {
            MeshType::Sphere => "Sphere", MeshType::Cube => "Cube", MeshType::Cylinder => "Cylinder",
            MeshType::Cone => "Cone", MeshType::Torus => "Torus", MeshType::Quad => "Quad",
            MeshType::Custom(_) => "Custom Mesh",
        }
    }
    pub fn all_standard() -> &'static [MeshType] {
        &[MeshType::Sphere, MeshType::Cube, MeshType::Cylinder, MeshType::Cone, MeshType::Torus, MeshType::Quad]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MeshWeightMode { Uniform, ByVertex, ByFace, ByVertexAndFace }

impl Default for MeshWeightMode { fn default() -> Self { MeshWeightMode::Uniform } }

impl MeshWeightMode {
    pub fn label(&self) -> &str {
        match self {
            MeshWeightMode::Uniform => "Uniform", MeshWeightMode::ByVertex => "By Vertex",
            MeshWeightMode::ByFace => "By Face", MeshWeightMode::ByVertexAndFace => "By Vertex And Face",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshParticle {
    pub enabled: bool,
    pub mesh_type: MeshType,
    pub scale_curve: Curve,
    pub rotation_speed: [f32; 3],
    pub rotation_random_start: [f32; 3],
    pub face_velocity: bool,
    pub normal_offset: f32,
    pub mesh_weight_mode: MeshWeightMode,
    pub alignment: BillboardAlignment,
    pub flip_u: bool,
    pub flip_v: bool,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub motion_blur: bool,
}

impl Default for MeshParticle {
    fn default() -> Self {
        MeshParticle {
            enabled: false, mesh_type: MeshType::Quad,
            scale_curve: Curve::constant(1.0), rotation_speed: [0.0; 3],
            rotation_random_start: [0.0; 3], face_velocity: false, normal_offset: 0.0,
            mesh_weight_mode: MeshWeightMode::Uniform, alignment: BillboardAlignment::View,
            flip_u: false, flip_v: false, cast_shadows: true, receive_shadows: true, motion_blur: false,
        }
    }
}

/// GPU instancing data for mesh particles.
#[derive(Clone, Debug, Default)]
pub struct GpuInstanceData {
    pub transform: [f32; 16],
    pub color: [f32; 4],
    pub custom_data_1: [f32; 4],
    pub custom_data_2: [f32; 4],
}

impl GpuInstanceData {
    pub fn identity() -> Self {
        GpuInstanceData {
            transform: [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, 0.0,0.0,0.0,1.0],
            color: [1.0; 4], custom_data_1: [0.0; 4], custom_data_2: [0.0; 4],
        }
    }
    pub fn with_translation(mut self, x: f32, y: f32, z: f32) -> Self {
        self.transform[12] = x; self.transform[13] = y; self.transform[14] = z; self
    }
    pub fn with_scale(mut self, sx: f32, sy: f32, sz: f32) -> Self {
        self.transform[0] = sx; self.transform[5] = sy; self.transform[10] = sz; self
    }
    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a]; self
    }
}

pub fn show_mesh_particle_ui(ui: &mut egui::Ui, mesh: &mut MeshParticle) {
    egui::CollapsingHeader::new("Mesh Particles").default_open(false).show(ui, |ui| {
        ui.checkbox(&mut mesh.enabled, "Enabled");
        if !mesh.enabled { return; }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Mesh Type:");
            egui::ComboBox::from_id_source("mesh_type_select")
                .selected_text(mesh.mesh_type.label())
                .show_ui(ui, |ui| {
                    for mt in MeshType::all_standard() {
                        ui.selectable_value(&mut mesh.mesh_type, mt.clone(), mt.label());
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Alignment:");
            egui::ComboBox::from_id_source("mesh_alignment")
                .selected_text(mesh.alignment.label())
                .show_ui(ui, |ui| {
                    for a in [BillboardAlignment::View, BillboardAlignment::World, BillboardAlignment::Local, BillboardAlignment::Velocity] {
                        let lbl = a.label();
                        ui.selectable_value(&mut mesh.alignment, a, lbl);
                    }
                });
        });
        ui.label("Rotation Speed (deg/s):");
        ui.horizontal(|ui| {
            ui.label("X:"); ui.add(egui::DragValue::new(&mut mesh.rotation_speed[0]).speed(0.5));
            ui.label("Y:"); ui.add(egui::DragValue::new(&mut mesh.rotation_speed[1]).speed(0.5));
            ui.label("Z:"); ui.add(egui::DragValue::new(&mut mesh.rotation_speed[2]).speed(0.5));
        });
        ui.label("Random Start Rotation:");
        ui.horizontal(|ui| {
            ui.label("X:"); ui.add(egui::DragValue::new(&mut mesh.rotation_random_start[0]).speed(1.0));
            ui.label("Y:"); ui.add(egui::DragValue::new(&mut mesh.rotation_random_start[1]).speed(1.0));
            ui.label("Z:"); ui.add(egui::DragValue::new(&mut mesh.rotation_random_start[2]).speed(1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Normal Offset:");
            ui.add(egui::DragValue::new(&mut mesh.normal_offset).speed(0.001));
        });
        ui.checkbox(&mut mesh.face_velocity, "Face Velocity");
        ui.checkbox(&mut mesh.cast_shadows, "Cast Shadows");
        ui.checkbox(&mut mesh.receive_shadows, "Receive Shadows");
        ui.checkbox(&mut mesh.motion_blur, "Motion Blur");
        ui.checkbox(&mut mesh.flip_u, "Flip U");
        ui.checkbox(&mut mesh.flip_v, "Flip V");
        ui.separator();
        ui.label("Scale Curve:");
        draw_curve_editor_inline(ui, &mut mesh.scale_curve, 200.0, 40.0);
    });
}

// =============================================================================
// SUB-EMITTER CHAINS
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SubEmitterEvent {
    Birth, Death, Collision, Manual, TriggerZone,
    OnLifetimePercent(f32), OnDistance(f32),
}

impl SubEmitterEvent {
    pub fn label(&self) -> &str {
        match self {
            SubEmitterEvent::Birth => "Birth", SubEmitterEvent::Death => "Death",
            SubEmitterEvent::Collision => "Collision", SubEmitterEvent::Manual => "Manual",
            SubEmitterEvent::TriggerZone => "Trigger Zone",
            SubEmitterEvent::OnLifetimePercent(_) => "On Lifetime %",
            SubEmitterEvent::OnDistance(_) => "On Distance",
        }
    }
    pub fn all_base() -> Vec<SubEmitterEvent> {
        vec![SubEmitterEvent::Birth, SubEmitterEvent::Death, SubEmitterEvent::Collision, SubEmitterEvent::Manual, SubEmitterEvent::TriggerZone]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubEmitterConfig {
    pub event: SubEmitterEvent,
    pub system_index: usize,
    pub inherit_velocity: f32,
    pub inherit_color: bool,
    pub inherit_size: bool,
    pub emit_count: RangeOrCurve,
    pub probability: f32,
    pub emit_probability_affects_burst: bool,
    pub label: String,
}

impl Default for SubEmitterConfig {
    fn default() -> Self {
        SubEmitterConfig {
            event: SubEmitterEvent::Death, system_index: 0, inherit_velocity: 0.0,
            inherit_color: true, inherit_size: true, emit_count: RangeOrCurve::Constant(1.0),
            probability: 1.0, emit_probability_affects_burst: false, label: String::from("Sub Emitter"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SubEmitterTreeNode {
    pub system_index: usize,
    pub name: String,
    pub children: Vec<usize>,
    pub depth: usize,
    pub collapsed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SubEmitterGraph {
    pub nodes: Vec<SubEmitterTreeNode>,
    pub selected: Option<usize>,
}

impl SubEmitterGraph {
    pub fn new() -> Self { SubEmitterGraph::default() }

    pub fn build_from_systems(&mut self, configs: &[(usize, Vec<SubEmitterConfig>)], names: &[String]) {
        self.nodes.clear();
        for (i, name) in names.iter().enumerate() {
            let children: Vec<usize> = configs.iter()
                .filter(|(pi, cfgs)| *pi == i && cfgs.iter().any(|c| c.system_index != i))
                .flat_map(|(_, cfgs)| cfgs.iter().map(|c| c.system_index))
                .collect();
            self.nodes.push(SubEmitterTreeNode { system_index: i, name: name.clone(), children, depth: 0, collapsed: false });
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sub-Emitter Graph");
        ui.separator();
        egui::ScrollArea::vertical().id_source("sub_emit_graph").show(ui, |ui| {
            let nodes_len = self.nodes.len();
            for i in 0..nodes_len {
                let depth = self.nodes[i].depth;
                let name = self.nodes[i].name.clone();
                let is_selected = self.selected == Some(i);
                let children_count = self.nodes[i].children.len();
                let collapsed = self.nodes[i].collapsed;
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 16.0);
                    if children_count > 0 {
                        let arrow = if collapsed { "▶" } else { "▼" };
                        if ui.small_button(arrow).clicked() { self.nodes[i].collapsed = !collapsed; }
                    } else {
                        ui.add_space(18.0);
                    }
                    let resp = ui.selectable_label(is_selected, egui::RichText::new(format!("[{}] {}", i, name)));
                    if resp.clicked() { self.selected = if is_selected { None } else { Some(i) }; }
                    if children_count > 0 {
                        ui.label(egui::RichText::new(format!("({} children)", children_count)).size(9.0).color(Color32::from_gray(120)));
                    }
                });
            }
        });
    }
}

pub fn show_sub_emitters_ui(ui: &mut egui::Ui, configs: &mut Vec<SubEmitterConfig>, system_names: &[String], graph: &mut SubEmitterGraph) {
    egui::CollapsingHeader::new("Sub Emitters").default_open(false).show(ui, |ui| {
        if ui.button("+ Add Sub Emitter").clicked() { configs.push(SubEmitterConfig::default()); }
        ui.separator();
        let mut to_remove = None;
        for (i, cfg) in configs.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                egui::CollapsingHeader::new(format!("[{}] {}", i, cfg.label)).default_open(false).show(ui, |ui| {
                    ui.horizontal(|ui| { ui.label("Label:"); ui.text_edit_singleline(&mut cfg.label); });
                    ui.horizontal(|ui| {
                        ui.label("Event:");
                        egui::ComboBox::from_id_source(format!("sub_event_{}", i)).selected_text(cfg.event.label()).show_ui(ui, |ui| {
                            for ev in SubEmitterEvent::all_base() {
                                let lbl = ev.label().to_string();
                                ui.selectable_value(&mut cfg.event, ev, lbl);
                            }
                        });
                    });
                    if !system_names.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("System:");
                            egui::ComboBox::from_id_source(format!("sub_sys_{}", i))
                                .selected_text(system_names.get(cfg.system_index).map(|s| s.as_str()).unwrap_or("(none)"))
                                .show_ui(ui, |ui| {
                                    for (si, sn) in system_names.iter().enumerate() {
                                        ui.selectable_value(&mut cfg.system_index, si, sn.as_str());
                                    }
                                });
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Probability:"); ui.add(egui::DragValue::new(&mut cfg.probability).speed(0.01).range(0.0..=1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Inherit Velocity:"); ui.add(egui::DragValue::new(&mut cfg.inherit_velocity).speed(0.01).range(0.0..=1.0));
                    });
                    ui.checkbox(&mut cfg.inherit_color, "Inherit Color");
                    ui.checkbox(&mut cfg.inherit_size, "Inherit Size");
                    ui.checkbox(&mut cfg.emit_probability_affects_burst, "Probability Affects Bursts");
                    if ui.small_button("Remove").clicked() { to_remove = Some(i); }
                });
            });
        }
        if let Some(i) = to_remove { configs.remove(i); }
        ui.separator();
        graph.show(ui);
    });
}

// =============================================================================
// FORCE FIELD MODULE
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ForceFieldShape { Sphere, Box, Cylinder, Torus, Hemisphere }

impl Default for ForceFieldShape { fn default() -> Self { ForceFieldShape::Sphere } }

impl ForceFieldShape {
    pub fn label(&self) -> &str {
        match self {
            ForceFieldShape::Sphere => "Sphere", ForceFieldShape::Box => "Box",
            ForceFieldShape::Cylinder => "Cylinder", ForceFieldShape::Torus => "Torus",
            ForceFieldShape::Hemisphere => "Hemisphere",
        }
    }
    pub fn all() -> &'static [ForceFieldShape] {
        &[ForceFieldShape::Sphere, ForceFieldShape::Box, ForceFieldShape::Cylinder, ForceFieldShape::Torus, ForceFieldShape::Hemisphere]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ForceDirectionMode { Radial, Aligned, Vortex, Turbulence, Custom }

impl Default for ForceDirectionMode { fn default() -> Self { ForceDirectionMode::Radial } }

impl ForceDirectionMode {
    pub fn label(&self) -> &str {
        match self {
            ForceDirectionMode::Radial => "Radial", ForceDirectionMode::Aligned => "Aligned",
            ForceDirectionMode::Vortex => "Vortex", ForceDirectionMode::Turbulence => "Turbulence",
            ForceDirectionMode::Custom => "Custom",
        }
    }
    pub fn all() -> &'static [ForceDirectionMode] {
        &[ForceDirectionMode::Radial, ForceDirectionMode::Aligned, ForceDirectionMode::Vortex, ForceDirectionMode::Turbulence, ForceDirectionMode::Custom]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForceFieldConfig {
    pub label: String, pub enabled: bool,
    pub shape: ForceFieldShape, pub position: [f32; 3], pub rotation: [f32; 3], pub scale: [f32; 3],
    pub strength: Curve, pub direction_mode: ForceDirectionMode,
    pub attenuation: f32, pub gravity: f32, pub drag: f32, pub rotation_speed: f32,
    pub start_range: f32, pub end_range: f32, pub multiplier: f32,
    pub noise_strength: f32, pub noise_frequency: f32, pub noise_scroll_speed: f32, pub noise_damping: bool,
}

impl Default for ForceFieldConfig {
    fn default() -> Self {
        ForceFieldConfig {
            label: String::from("Force Field"), enabled: true,
            shape: ForceFieldShape::Sphere, position: [0.0; 3], rotation: [0.0; 3], scale: [1.0; 3],
            strength: Curve::constant(1.0), direction_mode: ForceDirectionMode::Radial,
            attenuation: 1.0, gravity: 0.0, drag: 0.0, rotation_speed: 0.0,
            start_range: 0.0, end_range: 1.0, multiplier: 1.0,
            noise_strength: 0.0, noise_frequency: 1.0, noise_scroll_speed: 0.0, noise_damping: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ForceFieldModule {
    pub enabled: bool,
    pub fields: Vec<ForceFieldConfig>,
}

impl ForceFieldModule {
    pub fn new() -> Self { ForceFieldModule { enabled: false, fields: vec![] } }

    pub fn compute_force(&self, pos: [f32; 3], _vel: [f32; 3], t: f32) -> [f32; 3] {
        let mut fx = 0.0f32; let mut fy = 0.0f32; let mut fz = 0.0f32;
        if !self.enabled { return [fx, fy, fz]; }
        for field in &self.fields {
            if !field.enabled { continue; }
            let dx = pos[0] - field.position[0];
            let dy = pos[1] - field.position[1];
            let dz = pos[2] - field.position[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist < field.start_range || dist > field.end_range { continue; }
            let nd = ((dist - field.start_range) / (field.end_range - field.start_range)).clamp(0.0, 1.0);
            let strength = field.strength.evaluate(nd) * field.multiplier;
            let att = 1.0 - nd.powf(field.attenuation);
            let eff = strength * att;
            match field.direction_mode {
                ForceDirectionMode::Radial => {
                    if dist > 1e-6 { let inv = 1.0 / dist; fx -= dx * inv * eff; fy -= dy * inv * eff; fz -= dz * inv * eff; }
                }
                ForceDirectionMode::Aligned => {
                    let (sa, ca) = field.rotation[1].to_radians().sin_cos();
                    fx += ca * eff; fz += sa * eff;
                }
                ForceDirectionMode::Vortex => {
                    if dist > 1e-6 { let inv = 1.0 / dist; fx += dz * inv * eff * field.rotation_speed; fz -= dx * inv * eff * field.rotation_speed; }
                }
                ForceDirectionMode::Turbulence => {
                    let nx = ((pos[0] * field.noise_frequency + t * field.noise_scroll_speed).sin() * 2.0 - 1.0);
                    let ny = ((pos[1] * field.noise_frequency + t * field.noise_scroll_speed * 1.3).cos() * 2.0 - 1.0);
                    let nz = ((pos[2] * field.noise_frequency + t * field.noise_scroll_speed * 0.7).sin() * 2.0 - 1.0);
                    fx += nx * field.noise_strength * eff; fy += ny * field.noise_strength * eff; fz += nz * field.noise_strength * eff;
                }
                ForceDirectionMode::Custom => {}
            }
            fy += field.gravity * eff;
        }
        [fx, fy, fz]
    }

    pub fn draw_preview(&self, painter: &Painter, canvas_rect: Rect, world_to_canvas: impl Fn([f32; 2]) -> Pos2) {
        if !self.enabled { return; }
        for field in &self.fields {
            if !field.enabled { continue; }
            let center = world_to_canvas([field.position[0], field.position[1]]);
            let radius_canvas = field.end_range * (canvas_rect.width() / 20.0);
            let wire_color = Color32::from_rgba_unmultiplied(80, 200, 255, 120);
            match field.shape {
                ForceFieldShape::Sphere | ForceFieldShape::Hemisphere => {
                    painter.circle_stroke(center, radius_canvas, Stroke::new(1.0, wire_color));
                    if matches!(field.shape, ForceFieldShape::Hemisphere) {
                        painter.line_segment([Pos2::new(center.x - radius_canvas, center.y), Pos2::new(center.x + radius_canvas, center.y)], Stroke::new(1.0, wire_color));
                    }
                }
                ForceFieldShape::Box => {
                    let half = radius_canvas;
                    painter.rect_stroke(Rect::from_center_size(center, Vec2::splat(half * 2.0)), 0.0, Stroke::new(1.0, wire_color), egui::StrokeKind::Outside);
                }
                ForceFieldShape::Cylinder => {
                    painter.rect_stroke(Rect::from_center_size(center, Vec2::new(radius_canvas, radius_canvas * 1.5)), 4.0, Stroke::new(1.0, wire_color), egui::StrokeKind::Outside);
                }
                ForceFieldShape::Torus => {
                    painter.circle_stroke(center, radius_canvas, Stroke::new(1.0, wire_color));
                    painter.circle_stroke(center, radius_canvas * 0.5, Stroke::new(1.0, wire_color));
                }
            }
            // Force direction arrows
            for j in 0..8usize {
                let angle = (j as f32 / 8.0) * std::f32::consts::TAU;
                let ax = field.position[0] + field.end_range * angle.cos() * 0.8;
                let ay = field.position[1] + field.end_range * angle.sin() * 0.8;
                let as_ = world_to_canvas([ax, ay]);
                let (fdx, fdy) = match field.direction_mode {
                    ForceDirectionMode::Radial => (-angle.cos(), -angle.sin()),
                    ForceDirectionMode::Vortex => (-angle.sin(), angle.cos()),
                    _ => (0.0, -1.0),
                };
                let arrow_end = Pos2::new(as_.x + fdx * 10.0, as_.y + fdy * 10.0);
                painter.arrow(as_, arrow_end - as_, Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 200, 80, 160)));
            }
        }
    }
}

pub fn show_force_field_module_ui(ui: &mut egui::Ui, module: &mut ForceFieldModule) {
    egui::CollapsingHeader::new("Force Field").default_open(false).show(ui, |ui| {
        ui.checkbox(&mut module.enabled, "Enabled");
        if !module.enabled { return; }
        ui.separator();
        if ui.button("+ Add Force Field").clicked() { module.fields.push(ForceFieldConfig::default()); }
        let mut to_remove = None;
        for (i, field) in module.fields.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                egui::CollapsingHeader::new(format!("[{}] {}", i, field.label)).default_open(false).show(ui, |ui| {
                    ui.checkbox(&mut field.enabled, "Active");
                    ui.horizontal(|ui| { ui.label("Label:"); ui.text_edit_singleline(&mut field.label); });
                    ui.horizontal(|ui| {
                        ui.label("Shape:");
                        egui::ComboBox::from_id_source(format!("ff_shape_{}", i)).selected_text(field.shape.label()).show_ui(ui, |ui| {
                            for s in ForceFieldShape::all() { ui.selectable_value(&mut field.shape, s.clone(), s.label()); }
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Direction:");
                        egui::ComboBox::from_id_source(format!("ff_dir_{}", i)).selected_text(field.direction_mode.label()).show_ui(ui, |ui| {
                            for d in ForceDirectionMode::all() { ui.selectable_value(&mut field.direction_mode, d.clone(), d.label()); }
                        });
                    });
                    ui.label("Position:");
                    ui.horizontal(|ui| {
                        ui.label("X:"); ui.add(egui::DragValue::new(&mut field.position[0]).speed(0.1));
                        ui.label("Y:"); ui.add(egui::DragValue::new(&mut field.position[1]).speed(0.1));
                        ui.label("Z:"); ui.add(egui::DragValue::new(&mut field.position[2]).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Start Range:"); ui.add(egui::DragValue::new(&mut field.start_range).speed(0.05).range(0.0..=100.0));
                        ui.label("End Range:"); ui.add(egui::DragValue::new(&mut field.end_range).speed(0.05).range(0.0..=100.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Attenuation:"); ui.add(egui::DragValue::new(&mut field.attenuation).speed(0.01).range(0.01..=10.0));
                        ui.label("Gravity:"); ui.add(egui::DragValue::new(&mut field.gravity).speed(0.01));
                        ui.label("Drag:"); ui.add(egui::DragValue::new(&mut field.drag).speed(0.01).range(0.0..=1.0));
                    });
                    if matches!(field.direction_mode, ForceDirectionMode::Turbulence) {
                        ui.separator();
                        ui.label("Noise:");
                        ui.horizontal(|ui| {
                            ui.label("Strength:"); ui.add(egui::DragValue::new(&mut field.noise_strength).speed(0.01).range(0.0..=10.0));
                            ui.label("Frequency:"); ui.add(egui::DragValue::new(&mut field.noise_frequency).speed(0.01).range(0.01..=20.0));
                        });
                        ui.checkbox(&mut field.noise_damping, "Noise Damping");
                    }
                    ui.separator();
                    ui.label("Strength Curve:");
                    draw_curve_editor_inline(ui, &mut field.strength, 200.0, 40.0);
                    if ui.small_button("Remove Field").clicked() { to_remove = Some(i); }
                });
            });
        }
        if let Some(i) = to_remove { module.fields.remove(i); }
    });
}

// =============================================================================
// COLLISION MODULE 2D (expanded)
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CollisionQuality { Low, Medium, High }
impl Default for CollisionQuality { fn default() -> Self { CollisionQuality::Medium } }
impl CollisionQuality {
    pub fn label(&self) -> &str { match self { CollisionQuality::Low => "Low", CollisionQuality::Medium => "Medium", CollisionQuality::High => "High" } }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionPlane {
    pub normal: [f32; 3], pub distance: f32, pub label: String, pub visible: bool,
}

impl Default for CollisionPlane {
    fn default() -> Self { CollisionPlane { normal: [0.0, 1.0, 0.0], distance: 0.0, label: String::from("Ground"), visible: true } }
}

impl CollisionPlane {
    pub fn floor_at(y: f32) -> Self {
        CollisionPlane { normal: [0.0, 1.0, 0.0], distance: y, label: format!("Floor y={}", y), visible: true }
    }
    pub fn is_below(&self, pos: [f32; 3]) -> bool {
        pos[0] * self.normal[0] + pos[1] * self.normal[1] + pos[2] * self.normal[2] < self.distance
    }
    pub fn reflect_velocity(&self, vel: [f32; 3], bounce: f32, damping: f32) -> [f32; 3] {
        let dot = vel[0] * self.normal[0] + vel[1] * self.normal[1] + vel[2] * self.normal[2];
        let scale = bounce * (1.0 - damping);
        [(vel[0] - 2.0 * dot * self.normal[0]) * scale,
         (vel[1] - 2.0 * dot * self.normal[1]) * scale,
         (vel[2] - 2.0 * dot * self.normal[2]) * scale]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollisionModule2D {
    pub enabled: bool, pub mode: CollisionMode, pub planes: Vec<CollisionPlane>,
    pub quality: CollisionQuality, pub collision_radius: f32, pub visualize_bounds: bool,
    pub kill_on_collision: bool, pub min_kill_speed: f32, pub max_kill_speed: f32,
    pub bounce: f32, pub damping: f32, pub lifetime_loss: f32,
    pub send_collision_messages: bool, pub max_collision_shapes: u32, pub radius_scale: f32,
    pub enable_dynamic_colliders: bool,
}

impl Default for CollisionModule2D {
    fn default() -> Self {
        CollisionModule2D {
            enabled: false, mode: CollisionMode::Planes,
            planes: vec![CollisionPlane::floor_at(0.0)],
            quality: CollisionQuality::Medium, collision_radius: 0.0, visualize_bounds: false,
            kill_on_collision: false, min_kill_speed: 0.0, max_kill_speed: 10000.0,
            bounce: 0.6, damping: 0.0, lifetime_loss: 0.0,
            send_collision_messages: false, max_collision_shapes: 256, radius_scale: 1.0,
            enable_dynamic_colliders: false,
        }
    }
}

pub fn show_collision_module_2d_ui(ui: &mut egui::Ui, col: &mut CollisionModule2D) {
    egui::CollapsingHeader::new("Collision (2D)").default_open(false).show(ui, |ui| {
        ui.checkbox(&mut col.enabled, "Enabled");
        if !col.enabled { return; }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Mode:");
            ui.selectable_value(&mut col.mode, CollisionMode::Planes, "Planes");
            ui.selectable_value(&mut col.mode, CollisionMode::World, "World");
        });
        ui.horizontal(|ui| {
            ui.label("Quality:");
            ui.selectable_value(&mut col.quality, CollisionQuality::Low, "Low");
            ui.selectable_value(&mut col.quality, CollisionQuality::Medium, "Medium");
            ui.selectable_value(&mut col.quality, CollisionQuality::High, "High");
        });
        ui.horizontal(|ui| {
            ui.label("Bounce:"); ui.add(egui::DragValue::new(&mut col.bounce).speed(0.01).range(0.0..=1.0));
            ui.label("Damping:"); ui.add(egui::DragValue::new(&mut col.damping).speed(0.01).range(0.0..=1.0));
        });
        ui.horizontal(|ui| {
            ui.label("Lifetime Loss:"); ui.add(egui::DragValue::new(&mut col.lifetime_loss).speed(0.01).range(0.0..=1.0));
        });
        ui.checkbox(&mut col.kill_on_collision, "Kill on Collision");
        if col.kill_on_collision {
            ui.horizontal(|ui| {
                ui.label("Min Kill Speed:"); ui.add(egui::DragValue::new(&mut col.min_kill_speed).speed(0.1));
                ui.label("Max Kill Speed:"); ui.add(egui::DragValue::new(&mut col.max_kill_speed).speed(1.0));
            });
        }
        ui.checkbox(&mut col.visualize_bounds, "Visualize Bounds");
        ui.checkbox(&mut col.send_collision_messages, "Send Collision Messages");
        ui.checkbox(&mut col.enable_dynamic_colliders, "Enable Dynamic Colliders");
        ui.horizontal(|ui| {
            ui.label("Max Shapes:"); ui.add(egui::DragValue::new(&mut col.max_collision_shapes).speed(1.0).range(1..=1024));
        });
        if matches!(col.mode, CollisionMode::Planes) {
            ui.separator();
            ui.label("Collision Planes:");
            if ui.button("+ Add Plane").clicked() { col.planes.push(CollisionPlane::default()); }
            let mut to_remove = None;
            for (i, plane) in col.planes.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    egui::CollapsingHeader::new(format!("[{}] {}", i, plane.label)).default_open(false).show(ui, |ui| {
                        ui.horizontal(|ui| { ui.label("Label:"); ui.text_edit_singleline(&mut plane.label); });
                        ui.label("Normal:");
                        ui.horizontal(|ui| {
                            ui.label("X:"); ui.add(egui::DragValue::new(&mut plane.normal[0]).speed(0.01).range(-1.0..=1.0));
                            ui.label("Y:"); ui.add(egui::DragValue::new(&mut plane.normal[1]).speed(0.01).range(-1.0..=1.0));
                            ui.label("Z:"); ui.add(egui::DragValue::new(&mut plane.normal[2]).speed(0.01).range(-1.0..=1.0));
                        });
                        ui.horizontal(|ui| { ui.label("Distance:"); ui.add(egui::DragValue::new(&mut plane.distance).speed(0.1)); });
                        ui.checkbox(&mut plane.visible, "Show in Preview");
                        if ui.small_button("Remove").clicked() { to_remove = Some(i); }
                    });
                });
            }
            if let Some(i) = to_remove { col.planes.remove(i); }
        }
    });
}

pub fn draw_collision_planes_preview(painter: &Painter, canvas_rect: Rect, planes: &[CollisionPlane], world_to_canvas: impl Fn([f32; 2]) -> Pos2) {
    for plane in planes {
        if !plane.visible { continue; }
        let col = Color32::from_rgba_unmultiplied(255, 80, 80, 180);
        if plane.normal[1].abs() > 0.7 {
            let y = world_to_canvas([0.0, plane.distance]).y;
            painter.line_segment([Pos2::new(canvas_rect.min.x, y), Pos2::new(canvas_rect.max.x, y)], Stroke::new(1.5, col));
        } else if plane.normal[0].abs() > 0.7 {
            let x = world_to_canvas([plane.distance, 0.0]).x;
            painter.line_segment([Pos2::new(x, canvas_rect.min.y), Pos2::new(x, canvas_rect.max.y)], Stroke::new(1.5, col));
        }
    }
}

// =============================================================================
// PARTICLE ATTRACTOR SYSTEM
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AttractorFalloffMode { Linear, Quadratic, None, Cubic, InverseSquare }
impl Default for AttractorFalloffMode { fn default() -> Self { AttractorFalloffMode::Linear } }
impl AttractorFalloffMode {
    pub fn label(&self) -> &str {
        match self {
            AttractorFalloffMode::Linear => "Linear", AttractorFalloffMode::Quadratic => "Quadratic",
            AttractorFalloffMode::None => "None", AttractorFalloffMode::Cubic => "Cubic",
            AttractorFalloffMode::InverseSquare => "Inverse Square",
        }
    }
    pub fn all() -> &'static [AttractorFalloffMode] {
        &[AttractorFalloffMode::None, AttractorFalloffMode::Linear, AttractorFalloffMode::Quadratic, AttractorFalloffMode::Cubic, AttractorFalloffMode::InverseSquare]
    }
    pub fn compute_factor(&self, dist: f32, max_dist: f32) -> f32 {
        if dist >= max_dist { return 0.0; }
        let t = dist / max_dist;
        match self {
            AttractorFalloffMode::None => 1.0,
            AttractorFalloffMode::Linear => 1.0 - t,
            AttractorFalloffMode::Quadratic => (1.0 - t) * (1.0 - t),
            AttractorFalloffMode::Cubic => (1.0 - t).powi(3),
            AttractorFalloffMode::InverseSquare => 1.0 / (1.0 + dist * dist),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AttractorType { Point, Line, Plane, Rotation, Orbit }
impl Default for AttractorType { fn default() -> Self { AttractorType::Point } }
impl AttractorType {
    pub fn label(&self) -> &str {
        match self {
            AttractorType::Point => "Point", AttractorType::Line => "Line",
            AttractorType::Plane => "Plane", AttractorType::Rotation => "Rotation",
            AttractorType::Orbit => "Full Orbit",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attractor {
    pub label: String, pub enabled: bool, pub position: [f32; 3],
    pub strength: f32, pub falloff_mode: AttractorFalloffMode, pub max_distance: f32,
    pub attractor_type: AttractorType, pub orbit_radius: f32, pub orbit_speed: f32,
    pub line_direction: [f32; 3], pub plane_normal: [f32; 3],
    pub kill_in_range: f32, pub visualize: bool,
}

impl Default for Attractor {
    fn default() -> Self {
        Attractor {
            label: String::from("Attractor"), enabled: true, position: [0.0; 3],
            strength: 1.0, falloff_mode: AttractorFalloffMode::Linear, max_distance: 5.0,
            attractor_type: AttractorType::Point, orbit_radius: 1.0, orbit_speed: 1.0,
            line_direction: [0.0, 1.0, 0.0], plane_normal: [0.0, 1.0, 0.0],
            kill_in_range: 0.0, visualize: true,
        }
    }
}

impl Attractor {
    pub fn compute_force(&self, pos: [f32; 3]) -> [f32; 3] {
        if !self.enabled { return [0.0; 3]; }
        let dx = self.position[0] - pos[0];
        let dy = self.position[1] - pos[1];
        let dz = self.position[2] - pos[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist < 1e-6 { return [0.0; 3]; }
        let falloff = self.falloff_mode.compute_factor(dist, self.max_distance);
        if falloff <= 0.0 { return [0.0; 3]; }
        let inv_dist = 1.0 / dist;
        let fm = self.strength * falloff;
        match self.attractor_type {
            AttractorType::Point => [dx * inv_dist * fm, dy * inv_dist * fm, dz * inv_dist * fm],
            AttractorType::Rotation | AttractorType::Orbit => [-dz * inv_dist * fm * self.orbit_speed, 0.0, dx * inv_dist * fm * self.orbit_speed],
            AttractorType::Plane => {
                let dot = dx * self.plane_normal[0] + dy * self.plane_normal[1] + dz * self.plane_normal[2];
                [self.plane_normal[0] * dot * fm, self.plane_normal[1] * dot * fm, self.plane_normal[2] * dot * fm]
            }
            AttractorType::Line => {
                let t = dx * self.line_direction[0] + dy * self.line_direction[1] + dz * self.line_direction[2];
                let ldx = self.position[0] + self.line_direction[0] * t - pos[0];
                let ldy = self.position[1] + self.line_direction[1] * t - pos[1];
                let ldz = self.position[2] + self.line_direction[2] * t - pos[2];
                let ld = (ldx * ldx + ldy * ldy + ldz * ldz).sqrt().max(1e-6);
                [ldx / ld * fm, ldy / ld * fm, ldz / ld * fm]
            }
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AttractorModule {
    pub enabled: bool, pub attractors: Vec<Attractor>,
}

impl AttractorModule {
    pub fn new() -> Self { AttractorModule { enabled: false, attractors: vec![] } }

    pub fn compute_total_force(&self, pos: [f32; 3]) -> [f32; 3] {
        if !self.enabled { return [0.0; 3]; }
        let mut fx = 0.0f32; let mut fy = 0.0f32; let mut fz = 0.0f32;
        for attr in &self.attractors { let f = attr.compute_force(pos); fx += f[0]; fy += f[1]; fz += f[2]; }
        [fx, fy, fz]
    }

    pub fn draw_preview(&self, painter: &Painter, world_to_canvas: impl Fn([f32; 2]) -> Pos2) {
        if !self.enabled { return; }
        for attr in &self.attractors {
            if !attr.enabled || !attr.visualize { continue; }
            let center = world_to_canvas([attr.position[0], attr.position[1]]);
            let max_r = attr.max_distance * 20.0;
            painter.circle_stroke(center, max_r, Stroke::new(1.0, Color32::from_rgba_unmultiplied(200, 200, 80, 100)));
            painter.circle_filled(center, 4.0, Color32::from_rgba_unmultiplied(255, 255, 100, 200));
            // Trajectory traces for 8 sample particles
            for j in 0..8usize {
                let start_angle = (j as f32 / 8.0) * std::f32::consts::TAU;
                let start_r = max_r * 0.9;
                let mut px = attr.position[0] + start_r * start_angle.cos() / 20.0;
                let mut py = attr.position[1] + start_r * start_angle.sin() / 20.0;
                let mut pvx = 0.0f32; let mut pvy = 0.0f32;
                let dt = 0.05f32;
                let mut prev_canvas = world_to_canvas([px, py]);
                for _ in 0..20 {
                    let force = attr.compute_force([px, py, 0.0]);
                    pvx += force[0] * dt; pvy += force[1] * dt;
                    px += pvx * dt; py += pvy * dt;
                    let next_canvas = world_to_canvas([px, py]);
                    painter.line_segment([prev_canvas, next_canvas], Stroke::new(0.8, Color32::from_rgba_unmultiplied(255, 255, 80, 60)));
                    prev_canvas = next_canvas;
                }
            }
        }
    }
}

pub fn show_attractor_module_ui(ui: &mut egui::Ui, module: &mut AttractorModule) {
    egui::CollapsingHeader::new("Attractors").default_open(false).show(ui, |ui| {
        ui.checkbox(&mut module.enabled, "Enabled");
        if !module.enabled { return; }
        ui.separator();
        if ui.button("+ Add Attractor").clicked() { module.attractors.push(Attractor::default()); }
        let mut to_remove = None;
        for (i, attr) in module.attractors.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                egui::CollapsingHeader::new(format!("[{}] {}", i, attr.label)).default_open(false).show(ui, |ui| {
                    ui.checkbox(&mut attr.enabled, "Active");
                    ui.horizontal(|ui| { ui.label("Label:"); ui.text_edit_singleline(&mut attr.label); });
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        egui::ComboBox::from_id_source(format!("attr_type_{}", i)).selected_text(attr.attractor_type.label()).show_ui(ui, |ui| {
                            for t in [AttractorType::Point, AttractorType::Line, AttractorType::Plane, AttractorType::Rotation, AttractorType::Orbit] {
                                let lbl = t.label().to_string();
                                ui.selectable_value(&mut attr.attractor_type, t, lbl);
                            }
                        });
                    });
                    ui.label("Position:");
                    ui.horizontal(|ui| {
                        ui.label("X:"); ui.add(egui::DragValue::new(&mut attr.position[0]).speed(0.1));
                        ui.label("Y:"); ui.add(egui::DragValue::new(&mut attr.position[1]).speed(0.1));
                        ui.label("Z:"); ui.add(egui::DragValue::new(&mut attr.position[2]).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Strength:"); ui.add(egui::DragValue::new(&mut attr.strength).speed(0.05));
                        ui.label("Max Dist:"); ui.add(egui::DragValue::new(&mut attr.max_distance).speed(0.1).range(0.01..=100.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Falloff:");
                        egui::ComboBox::from_id_source(format!("attr_falloff_{}", i)).selected_text(attr.falloff_mode.label()).show_ui(ui, |ui| {
                            for f in AttractorFalloffMode::all() { ui.selectable_value(&mut attr.falloff_mode, f.clone(), f.label()); }
                        });
                    });
                    if matches!(attr.attractor_type, AttractorType::Rotation | AttractorType::Orbit) {
                        ui.horizontal(|ui| {
                            ui.label("Orbit Radius:"); ui.add(egui::DragValue::new(&mut attr.orbit_radius).speed(0.05));
                            ui.label("Speed:"); ui.add(egui::DragValue::new(&mut attr.orbit_speed).speed(0.05));
                        });
                    }
                    ui.horizontal(|ui| { ui.label("Kill Range:"); ui.add(egui::DragValue::new(&mut attr.kill_in_range).speed(0.01).range(0.0..=10.0)); });
                    ui.checkbox(&mut attr.visualize, "Visualize");
                    if ui.small_button("Remove").clicked() { to_remove = Some(i); }
                });
            });
        }
        if let Some(i) = to_remove { module.attractors.remove(i); }
    });
}

// =============================================================================
// LOD SYSTEM FOR PARTICLES
// =============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LodLevel {
    pub distance: f32, pub max_particles_factor: f32, pub simulation_speed: f32,
    pub emit_rate_factor: f32, pub enabled_modules: Vec<bool>, pub label: String,
    pub simplify_rendering: bool, pub billboard_only: bool,
}

impl Default for LodLevel {
    fn default() -> Self {
        LodLevel {
            distance: 20.0, max_particles_factor: 1.0, simulation_speed: 1.0,
            emit_rate_factor: 1.0, enabled_modules: vec![true; 12], label: String::from("LOD 0"),
            simplify_rendering: false, billboard_only: false,
        }
    }
}

impl LodLevel {
    pub fn far_lod(distance: f32, level: usize) -> Self {
        LodLevel {
            distance, max_particles_factor: 1.0 / (level + 1) as f32, simulation_speed: 0.5,
            emit_rate_factor: 0.5 / (level + 1) as f32, enabled_modules: vec![true; 12],
            label: format!("LOD {}", level), simplify_rendering: level > 1, billboard_only: level > 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticleLod {
    pub enabled: bool, pub lod_levels: Vec<LodLevel>, pub current_lod: usize,
    pub preview_distance: f32, pub fade_between_lods: bool, pub fade_range: f32,
    pub animated_cross_fading: bool,
}

impl Default for ParticleLod {
    fn default() -> Self {
        ParticleLod {
            enabled: false,
            lod_levels: vec![
                LodLevel { label: String::from("LOD 0"), distance: 10.0, max_particles_factor: 1.0, simulation_speed: 1.0, emit_rate_factor: 1.0, enabled_modules: vec![true; 12], simplify_rendering: false, billboard_only: false },
                LodLevel { label: String::from("LOD 1"), distance: 20.0, max_particles_factor: 0.5, simulation_speed: 1.0, emit_rate_factor: 0.5, enabled_modules: vec![true; 12], simplify_rendering: true, billboard_only: false },
                LodLevel { label: String::from("LOD 2"), distance: 40.0, max_particles_factor: 0.25, simulation_speed: 0.5, emit_rate_factor: 0.2, enabled_modules: vec![true; 12], simplify_rendering: true, billboard_only: true },
            ],
            current_lod: 0, preview_distance: 0.0, fade_between_lods: true, fade_range: 2.0, animated_cross_fading: false,
        }
    }
}

impl ParticleLod {
    pub fn get_lod_for_distance(&self, dist: f32) -> usize {
        for (i, level) in self.lod_levels.iter().enumerate() { if dist <= level.distance { return i; } }
        self.lod_levels.len().saturating_sub(1)
    }
    pub fn current_level(&self) -> Option<&LodLevel> { self.lod_levels.get(self.current_lod) }
}

pub fn show_particle_lod_ui(ui: &mut egui::Ui, lod: &mut ParticleLod) {
    egui::CollapsingHeader::new("LOD (Level of Detail)").default_open(false).show(ui, |ui| {
        ui.checkbox(&mut lod.enabled, "Enabled");
        if !lod.enabled { return; }
        ui.separator();
        ui.checkbox(&mut lod.fade_between_lods, "Fade Between LODs");
        ui.checkbox(&mut lod.animated_cross_fading, "Animated Cross-Fading");
        if lod.fade_between_lods {
            ui.horizontal(|ui| { ui.label("Fade Range:"); ui.add(egui::DragValue::new(&mut lod.fade_range).speed(0.1).range(0.0..=20.0)); });
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Preview at distance:");
            ui.add(egui::DragValue::new(&mut lod.preview_distance).speed(0.5).range(0.0..=200.0));
            ui.label("m");
        });
        let active_lod = lod.get_lod_for_distance(lod.preview_distance);
        ui.label(egui::RichText::new(format!("Active LOD: {}", active_lod)).color(Color32::from_rgb(100, 200, 255)));
        ui.separator();
        if ui.button("+ Add LOD Level").clicked() {
            let next_dist = lod.lod_levels.last().map(|l| l.distance * 1.5).unwrap_or(10.0);
            let next_idx = lod.lod_levels.len();
            lod.lod_levels.push(LodLevel::far_lod(next_dist, next_idx));
        }
        let mut to_remove = None;
        for (i, level) in lod.lod_levels.iter_mut().enumerate() {
            let is_active = active_lod == i;
            ui.push_id(i, |ui| {
                let hdr_col = if is_active { Color32::from_rgb(100, 220, 100) } else { Color32::from_gray(180) };
                egui::CollapsingHeader::new(egui::RichText::new(format!("  {} {}", level.label, if is_active { "◀ ACTIVE" } else { "" })).color(hdr_col))
                    .default_open(is_active)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| { ui.label("Label:"); ui.text_edit_singleline(&mut level.label); });
                        ui.horizontal(|ui| { ui.label("Max Distance:"); ui.add(egui::DragValue::new(&mut level.distance).speed(1.0).range(0.1..=10000.0)); ui.label("m"); });
                        ui.horizontal(|ui| { ui.label("Max Particles:"); ui.add(egui::Slider::new(&mut level.max_particles_factor, 0.0..=1.0).text("x")); });
                        ui.horizontal(|ui| { ui.label("Sim Speed:"); ui.add(egui::Slider::new(&mut level.simulation_speed, 0.0..=2.0).text("x")); });
                        ui.horizontal(|ui| { ui.label("Emit Rate:"); ui.add(egui::Slider::new(&mut level.emit_rate_factor, 0.0..=1.0).text("x")); });
                        ui.checkbox(&mut level.simplify_rendering, "Simplify Rendering");
                        ui.checkbox(&mut level.billboard_only, "Billboard Only");
                        if ui.small_button("Remove LOD").clicked() { to_remove = Some(i); }
                    });
            });
        }
        if let Some(i) = to_remove { lod.lod_levels.remove(i); }
    });
}

// =============================================================================
// PARTICLE EVENT SYSTEM
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParticleEventTrigger {
    OnSpawn, OnDeath, OnCollision, OnLifetimePercent(f32), OnDistance(f32),
    OnVelocityBelow(f32), OnVelocityAbove(f32), Custom(String),
}

impl ParticleEventTrigger {
    pub fn label_str(&self) -> String {
        match self {
            ParticleEventTrigger::OnSpawn => "On Spawn".to_string(),
            ParticleEventTrigger::OnDeath => "On Death".to_string(),
            ParticleEventTrigger::OnCollision => "On Collision".to_string(),
            ParticleEventTrigger::OnLifetimePercent(p) => format!("Lifetime {}%", (p * 100.0) as u32),
            ParticleEventTrigger::OnDistance(d) => format!("Distance > {:.1}", d),
            ParticleEventTrigger::OnVelocityBelow(v) => format!("Velocity < {:.1}", v),
            ParticleEventTrigger::OnVelocityAbove(v) => format!("Velocity > {:.1}", v),
            ParticleEventTrigger::Custom(s) => format!("Custom: {}", s),
        }
    }
    pub fn base_variants() -> Vec<ParticleEventTrigger> {
        vec![ParticleEventTrigger::OnSpawn, ParticleEventTrigger::OnDeath, ParticleEventTrigger::OnCollision]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EventAction {
    SpawnSubEmitter(usize), TriggerEffect(String), SetBlackboard(String, f32),
    PlaySound(String), SpawnDecal(String), StopSystem, RestartSystem, ModifyProperty(String, f32),
}

impl EventAction {
    pub fn label_str(&self) -> String {
        match self {
            EventAction::SpawnSubEmitter(i) => format!("Spawn Sub[{}]", i),
            EventAction::TriggerEffect(s) => format!("Effect: {}", s),
            EventAction::SetBlackboard(k, v) => format!("BB[{}]={}", k, v),
            EventAction::PlaySound(s) => format!("Sound: {}", s),
            EventAction::SpawnDecal(s) => format!("Decal: {}", s),
            EventAction::StopSystem => "Stop System".to_string(),
            EventAction::RestartSystem => "Restart System".to_string(),
            EventAction::ModifyProperty(p, v) => format!("Set {}: {}", p, v),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticleEventRule {
    pub enabled: bool, pub trigger: ParticleEventTrigger, pub actions: Vec<EventAction>,
    pub cooldown: f32, pub max_fires: i32, pub label: String,
}

impl Default for ParticleEventRule {
    fn default() -> Self {
        ParticleEventRule {
            enabled: true, trigger: ParticleEventTrigger::OnDeath, actions: vec![],
            cooldown: 0.0, max_fires: -1, label: String::from("Event Rule"),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParticleEventSystem {
    pub enabled: bool, pub rules: Vec<ParticleEventRule>,
}

impl ParticleEventSystem {
    pub fn new() -> Self { ParticleEventSystem::default() }
}

pub fn show_particle_event_system_ui(ui: &mut egui::Ui, events: &mut ParticleEventSystem) {
    egui::CollapsingHeader::new("Event System").default_open(false).show(ui, |ui| {
        ui.checkbox(&mut events.enabled, "Enabled");
        if !events.enabled { return; }
        ui.separator();
        if ui.button("+ Add Event Rule").clicked() { events.rules.push(ParticleEventRule::default()); }
        let mut to_remove = None;
        for (i, rule) in events.rules.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                egui::CollapsingHeader::new(format!("[{}] {} — {}", i, rule.label, rule.trigger.label_str())).default_open(false).show(ui, |ui| {
                    ui.checkbox(&mut rule.enabled, "Active");
                    ui.horizontal(|ui| { ui.label("Label:"); ui.text_edit_singleline(&mut rule.label); });
                    ui.horizontal(|ui| {
                        ui.label("Trigger:");
                        egui::ComboBox::from_id_source(format!("evt_trigger_{}", i)).selected_text(rule.trigger.label_str()).show_ui(ui, |ui| {
                            for t in ParticleEventTrigger::base_variants() {
                                let lbl = t.label_str();
                                ui.selectable_value(&mut rule.trigger, t, lbl);
                            }
                            if ui.selectable_label(matches!(&rule.trigger, ParticleEventTrigger::OnLifetimePercent(_)), "On Lifetime %").clicked() {
                                rule.trigger = ParticleEventTrigger::OnLifetimePercent(0.5);
                            }
                            if ui.selectable_label(matches!(&rule.trigger, ParticleEventTrigger::OnDistance(_)), "On Distance").clicked() {
                                rule.trigger = ParticleEventTrigger::OnDistance(5.0);
                            }
                        });
                    });
                    match &mut rule.trigger {
                        ParticleEventTrigger::OnLifetimePercent(p) => {
                            ui.horizontal(|ui| { ui.label("Lifetime %:"); ui.add(egui::DragValue::new(p).speed(0.01).range(0.0..=1.0)); });
                        }
                        ParticleEventTrigger::OnDistance(d) => {
                            ui.horizontal(|ui| { ui.label("Distance:"); ui.add(egui::DragValue::new(d).speed(0.1).range(0.0..=1000.0)); });
                        }
                        _ => {}
                    }
                    ui.horizontal(|ui| {
                        ui.label("Cooldown:"); ui.add(egui::DragValue::new(&mut rule.cooldown).speed(0.01).range(0.0..=60.0)); ui.label("s");
                        ui.label("Max Fires:"); ui.add(egui::DragValue::new(&mut rule.max_fires).speed(1.0).range(-1..=10000)); ui.label("(-1=inf)");
                    });
                    ui.separator();
                    ui.label(format!("Actions ({}):", rule.actions.len()));
                    ui.horizontal(|ui| {
                        if ui.small_button("+ Sub-Emitter").clicked() { rule.actions.push(EventAction::SpawnSubEmitter(0)); }
                        if ui.small_button("+ Sound").clicked() { rule.actions.push(EventAction::PlaySound(String::from("explosion"))); }
                        if ui.small_button("+ Blackboard").clicked() { rule.actions.push(EventAction::SetBlackboard(String::from("key"), 0.0)); }
                        if ui.small_button("+ Effect").clicked() { rule.actions.push(EventAction::TriggerEffect(String::from("effect_name"))); }
                    });
                    let mut action_to_remove = None;
                    for (ai, action) in rule.actions.iter_mut().enumerate() {
                        ui.push_id(ai, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}.", ai));
                                match action {
                                    EventAction::SpawnSubEmitter(idx) => {
                                        ui.label("SpawnSub:"); ui.add(egui::DragValue::new(idx).speed(1.0).range(0..=64));
                                    }
                                    EventAction::SetBlackboard(key, val) => {
                                        ui.text_edit_singleline(key); ui.label("="); ui.add(egui::DragValue::new(val).speed(0.1));
                                    }
                                    EventAction::PlaySound(s) | EventAction::TriggerEffect(s) | EventAction::SpawnDecal(s) => {
                                        ui.text_edit_singleline(s);
                                    }
                                    EventAction::ModifyProperty(p, v) => {
                                        ui.text_edit_singleline(p); ui.add(egui::DragValue::new(v).speed(0.1));
                                    }
                                    _ => { ui.label(action.label_str()); }
                                }
                                if ui.small_button("X").clicked() { action_to_remove = Some(ai); }
                            });
                        });
                    }
                    if let Some(ai) = action_to_remove { rule.actions.remove(ai); }
                    if ui.small_button("Remove Rule").clicked() { to_remove = Some(i); }
                });
            });
        }
        if let Some(i) = to_remove { events.rules.remove(i); }
    });
}

// =============================================================================
// CURVE LIBRARY PRESETS (30+ curves) + MATH UTILITIES
// =============================================================================

pub fn all_curve_presets() -> Vec<(&'static str, Curve)> {
    vec![
        ("Linear 0→1", Curve::linear_zero_to_one()),
        ("Linear 1→0", Curve::linear_one_to_zero()),
        ("Constant 1", Curve::constant(1.0)),
        ("Constant 0", Curve::constant(0.0)),
        ("Ease In", curve_ease_in()),
        ("Ease Out", curve_ease_out()),
        ("Ease In-Out", curve_ease_in_out()),
        ("Bounce", curve_bounce()),
        ("Elastic", curve_elastic()),
        ("Back", curve_back()),
        ("Step 25%", curve_step_at(0.25)),
        ("Step 50%", curve_step_at(0.50)),
        ("Step 75%", curve_step_at(0.75)),
        ("Pulse", curve_pulse()),
        ("Sawtooth", curve_sawtooth()),
        ("Sine Wave", curve_sine_wave()),
        ("Square Wave", curve_square_wave()),
        ("Triangle Wave", curve_triangle_wave()),
        ("Exp Growth", curve_exponential_growth()),
        ("Exp Decay", curve_exponential_decay()),
        ("Logistic (S)", curve_logistic()),
        ("Bell Curve", curve_bell()),
        ("Double Peak", curve_double_peak()),
        ("Ramp Up", curve_ramp_up()),
        ("Ramp Down", curve_ramp_down()),
        ("Plateau", curve_plateau()),
        ("Valley", curve_valley()),
        ("Spike", curve_spike()),
        ("Fade In/Out", curve_fade_in_fade_out()),
        ("Flash", curve_flash()),
        ("Stairs 4", curve_staircase(4)),
        ("Stairs 8", curve_staircase(8)),
        ("Random Walk", curve_random_walk()),
    ]
}

fn curve_ease_in() -> Curve {
    Curve { keys: vec![CurveKey { time: 0.0, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey { time: 1.0, value: 1.0, in_tangent: 2.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}
fn curve_ease_out() -> Curve {
    Curve { keys: vec![CurveKey { time: 0.0, value: 0.0, in_tangent: 0.0, out_tangent: 2.0, interpolation: Interpolation::Smooth }, CurveKey { time: 1.0, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}
fn curve_ease_in_out() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 0.0), CurveKey::new(1.0, 1.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_bounce() -> Curve {
    Curve {
        keys: vec![
            CurveKey::new(0.0, 0.0),
            CurveKey { time: 0.36, value: 1.0, in_tangent: 0.0, out_tangent: -4.0, interpolation: Interpolation::Smooth },
            CurveKey { time: 0.6, value: 0.3, in_tangent: 0.0, out_tangent: 3.0, interpolation: Interpolation::Smooth },
            CurveKey { time: 0.78, value: 1.0, in_tangent: 0.0, out_tangent: -2.0, interpolation: Interpolation::Smooth },
            CurveKey { time: 0.9, value: 0.7, in_tangent: 0.0, out_tangent: 1.5, interpolation: Interpolation::Smooth },
            CurveKey::new(1.0, 1.0),
        ],
        keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0,
    }
}

fn curve_elastic() -> Curve {
    Curve {
        keys: vec![
            CurveKey::new(0.0, 0.0),
            CurveKey { time: 0.5, value: 1.2, in_tangent: 3.0, out_tangent: 0.0, interpolation: Interpolation::Smooth },
            CurveKey { time: 0.75, value: 0.9, in_tangent: 0.0, out_tangent: 1.0, interpolation: Interpolation::Smooth },
            CurveKey { time: 0.875, value: 1.05, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth },
            CurveKey::new(1.0, 1.0),
        ],
        keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0,
    }
}

fn curve_back() -> Curve {
    Curve {
        keys: vec![
            CurveKey::new(0.0, 0.0),
            CurveKey { time: 0.3, value: -0.1, in_tangent: 0.0, out_tangent: -1.5, interpolation: Interpolation::Smooth },
            CurveKey::new(1.0, 1.0),
        ],
        keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0,
    }
}

fn curve_step_at(threshold: f32) -> Curve {
    Curve {
        keys: vec![
            CurveKey { time: 0.0, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: threshold, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: threshold + 0.001, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 1.0, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
        ],
        keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0,
    }
}

fn curve_pulse() -> Curve {
    Curve {
        keys: vec![
            CurveKey { time: 0.0, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.2, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.201, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.4, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.401, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 1.0, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
        ],
        keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0,
    }
}

fn curve_sawtooth() -> Curve {
    Curve {
        keys: (0..=8).map(|i| { let t = i as f32 / 8.0; let v = t % 0.25 / 0.25; CurveKey { time: t, value: v, in_tangent: 4.0, out_tangent: 4.0, interpolation: Interpolation::Linear } }).collect(),
        keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0,
    }
}

fn curve_sine_wave() -> Curve {
    let n = 20usize;
    Curve {
        keys: (0..=n).map(|i| { let t = i as f32 / n as f32; let v = (t * std::f32::consts::TAU * 2.0).sin() * 0.5 + 0.5; CurveKey { time: t, value: v, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect(),
        keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0,
    }
}

fn curve_square_wave() -> Curve {
    Curve {
        keys: vec![
            CurveKey { time: 0.0, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.25, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.251, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.5, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.501, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.75, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 0.751, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
            CurveKey { time: 1.0, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant },
        ],
        keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0,
    }
}

fn curve_triangle_wave() -> Curve {
    Curve { keys: vec![CurveKey::linear(0.0, 0.0), CurveKey::linear(0.25, 1.0), CurveKey::linear(0.75, 0.0), CurveKey::linear(1.0, 1.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_exponential_growth() -> Curve {
    let n = 16usize;
    Curve { keys: (0..=n).map(|i| { let t = i as f32 / n as f32; let v = ((t * 5.0).exp() / (5.0f32).exp()).min(1.0); CurveKey { time: t, value: v, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect(), keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_exponential_decay() -> Curve {
    let n = 16usize;
    Curve { keys: (0..=n).map(|i| { let t = i as f32 / n as f32; let v = (-t * 5.0).exp(); CurveKey { time: t, value: v, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect(), keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_logistic() -> Curve {
    let n = 20usize;
    Curve { keys: (0..=n).map(|i| { let t = i as f32 / n as f32; let x = (t - 0.5) * 12.0; let v = 1.0 / (1.0 + (-x).exp()); CurveKey { time: t, value: v, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect(), keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_bell() -> Curve {
    let n = 20usize;
    Curve { keys: (0..=n).map(|i| { let t = i as f32 / n as f32; let x = (t - 0.5) * 6.0; let v = (-x * x * 0.5).exp(); CurveKey { time: t, value: v, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect(), keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_double_peak() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 0.0), CurveKey { time: 0.2, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey { time: 0.5, value: 0.2, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey { time: 0.8, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey::new(1.0, 0.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_ramp_up() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 0.0), CurveKey { time: 0.7, value: 0.1, in_tangent: 0.0, out_tangent: 3.0, interpolation: Interpolation::Smooth }, CurveKey::new(1.0, 1.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_ramp_down() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 1.0), CurveKey { time: 0.3, value: 0.9, in_tangent: -3.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey::new(1.0, 0.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_plateau() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 0.0), CurveKey { time: 0.2, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey { time: 0.8, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey::new(1.0, 0.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_valley() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 1.0), CurveKey { time: 0.2, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey { time: 0.8, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey::new(1.0, 1.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_spike() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 0.0), CurveKey { time: 0.5, value: 1.0, in_tangent: 10.0, out_tangent: -10.0, interpolation: Interpolation::Smooth }, CurveKey::new(1.0, 0.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_fade_in_fade_out() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 0.0), CurveKey { time: 0.15, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey { time: 0.85, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey::new(1.0, 0.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_flash() -> Curve {
    Curve { keys: vec![CurveKey::new(0.0, 1.0), CurveKey { time: 0.05, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey { time: 0.2, value: 0.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth }, CurveKey::new(1.0, 0.0)], keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_staircase(steps: usize) -> Curve {
    let mut keys = vec![];
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let v = i as f32 / (steps.max(2) - 1) as f32;
        keys.push(CurveKey { time: t0, value: v, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant });
        keys.push(CurveKey { time: t1 - 0.0001, value: v, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant });
    }
    keys.push(CurveKey { time: 1.0, value: 1.0, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Constant });
    Curve { keys, keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

fn curve_random_walk() -> Curve {
    let n = 12usize;
    let mut val = 0.5f32;
    let mut keys = vec![];
    for i in 0..=n {
        let t = i as f32 / n as f32;
        keys.push(CurveKey { time: t, value: val.clamp(0.0, 1.0), in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth });
        let hash = ((t * 73.1 + 17.3).sin() * 43758.5453).fract();
        val += (hash - 0.5) * 0.3;
        val = val.clamp(0.0, 1.0);
    }
    Curve { keys, keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

// Curve math utilities
pub fn curve_integrate(curve: &Curve, t0: f32, t1: f32) -> f32 {
    let n = 64usize;
    let h = (t1 - t0) / n as f32;
    let mut sum = curve.evaluate(t0) + curve.evaluate(t1);
    for i in 1..n { let t = t0 + i as f32 * h; let w = if i % 2 == 0 { 2.0 } else { 4.0 }; sum += w * curve.evaluate(t); }
    sum * h / 3.0
}

pub fn curve_find_root(curve: &Curve, target: f32) -> f32 {
    let mut lo = 0.0f32; let mut hi = 1.0f32;
    for _ in 0..48 { let mid = (lo + hi) * 0.5; if curve.evaluate(mid) < target { lo = mid; } else { hi = mid; } }
    (lo + hi) * 0.5
}

pub fn curve_invert(curve: &Curve) -> Curve {
    let n = 32usize;
    let keys: Vec<CurveKey> = (0..=n).map(|i| { let v = i as f32 / n as f32; let t = curve_find_root(curve, v); CurveKey { time: v, value: t, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect();
    Curve { keys, keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

pub fn curve_sample_uniform(curve: &Curve, n: usize) -> Vec<(f32, f32)> {
    (0..=n).map(|i| { let t = i as f32 / n as f32; (t, curve.evaluate(t)) }).collect()
}

pub fn curve_value_range(curve: &Curve) -> (f32, f32) {
    let (mut mn, mut mx) = (f32::MAX, f32::MIN);
    for i in 0..=64 { let v = curve.evaluate(i as f32 / 64.0); if v < mn { mn = v; } if v > mx { mx = v; } }
    (mn, mx)
}

pub fn curve_arc_length(curve: &Curve) -> f32 {
    let n = 64usize; let mut len = 0.0f32; let mut pt = 0.0f32; let mut pv = curve.evaluate(0.0);
    for i in 1..=n { let t = i as f32 / n as f32; let v = curve.evaluate(t); len += ((t-pt)*(t-pt)+(v-pv)*(v-pv)).sqrt(); pt = t; pv = v; }
    len
}

pub fn curve_blend(a: &Curve, b: &Curve, weight: f32) -> Curve {
    let n = 32usize;
    let keys: Vec<CurveKey> = (0..=n).map(|i| { let t = i as f32 / n as f32; let v = lerp(a.evaluate(t), b.evaluate(t), weight); CurveKey { time: t, value: v, in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect();
    Curve { keys, keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

pub fn curve_add(a: &Curve, b: &Curve) -> Curve {
    let n = 32usize;
    let keys: Vec<CurveKey> = (0..=n).map(|i| { let t = i as f32 / n as f32; CurveKey { time: t, value: a.evaluate(t) + b.evaluate(t), in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect();
    Curve { keys, keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

pub fn curve_multiply(a: &Curve, b: &Curve) -> Curve {
    let n = 32usize;
    let keys: Vec<CurveKey> = (0..=n).map(|i| { let t = i as f32 / n as f32; CurveKey { time: t, value: a.evaluate(t) * b.evaluate(t), in_tangent: 0.0, out_tangent: 0.0, interpolation: Interpolation::Smooth } }).collect();
    Curve { keys, keys2: vec![], mode: CurveMode::Curve, multiplier: 1.0 }
}

pub fn show_curve_comparison(ui: &mut egui::Ui, curves: &[(&str, &Curve)], width: f32, height: f32) {
    ui.label("Curve Comparison:");
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, Color32::from_gray(20));
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(60)), egui::StrokeKind::Outside);
    for i in 0..=4 {
        let gx = rect.min.x + i as f32 / 4.0 * rect.width();
        let gy = rect.min.y + i as f32 / 4.0 * rect.height();
        painter.line_segment([Pos2::new(gx, rect.min.y), Pos2::new(gx, rect.max.y)], Stroke::new(0.5, Color32::from_gray(35)));
        painter.line_segment([Pos2::new(rect.min.x, gy), Pos2::new(rect.max.x, gy)], Stroke::new(0.5, Color32::from_gray(35)));
    }
    let palette = [Color32::from_rgb(255,100,100), Color32::from_rgb(100,255,100), Color32::from_rgb(100,150,255), Color32::from_rgb(255,200,50), Color32::from_rgb(200,100,255), Color32::from_rgb(100,220,220)];
    let n = 80usize;
    for (ci, (name, curve)) in curves.iter().enumerate() {
        let color = palette[ci % palette.len()];
        let mut prev: Option<Pos2> = None;
        for s in 0..=n {
            let t = s as f32 / n as f32;
            let v = curve.evaluate(t).clamp(0.0, 1.0);
            let pt = Pos2::new(rect.min.x + t * rect.width(), rect.max.y - v * rect.height());
            if let Some(pp) = prev { painter.line_segment([pp, pt], Stroke::new(1.5, color)); }
            prev = Some(pt);
        }
        let ly = rect.min.y + 5.0 + ci as f32 * 14.0;
        painter.line_segment([Pos2::new(rect.min.x + 5.0, ly + 5.0), Pos2::new(rect.min.x + 20.0, ly + 5.0)], Stroke::new(2.0, color));
        painter.text(Pos2::new(rect.min.x + 24.0, ly), egui::Align2::LEFT_TOP, *name, FontId::proportional(10.0), color);
    }
}

// =============================================================================
// VFX COMPOSITION LAYER
// =============================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayerBlendMode { Additive, AlphaBlend, Multiply, Screen, Overlay, SoftLight, HardLight }

impl Default for LayerBlendMode { fn default() -> Self { LayerBlendMode::AlphaBlend } }

impl LayerBlendMode {
    pub fn label(&self) -> &str {
        match self {
            LayerBlendMode::Additive => "Additive", LayerBlendMode::AlphaBlend => "Alpha Blend",
            LayerBlendMode::Multiply => "Multiply", LayerBlendMode::Screen => "Screen",
            LayerBlendMode::Overlay => "Overlay", LayerBlendMode::SoftLight => "Soft Light",
            LayerBlendMode::HardLight => "Hard Light",
        }
    }
    pub fn all() -> &'static [LayerBlendMode] {
        &[LayerBlendMode::AlphaBlend, LayerBlendMode::Additive, LayerBlendMode::Multiply, LayerBlendMode::Screen, LayerBlendMode::Overlay, LayerBlendMode::SoftLight, LayerBlendMode::HardLight]
    }

    pub fn blend_colors(&self, bg: [f32; 4], fg: [f32; 4]) -> [f32; 4] {
        let fa = fg[3];
        match self {
            LayerBlendMode::AlphaBlend => [lerp(bg[0],fg[0],fa), lerp(bg[1],fg[1],fa), lerp(bg[2],fg[2],fa), bg[3] + fa*(1.0-bg[3])],
            LayerBlendMode::Additive => [(bg[0]+fg[0]*fa).min(1.0), (bg[1]+fg[1]*fa).min(1.0), (bg[2]+fg[2]*fa).min(1.0), bg[3]],
            LayerBlendMode::Multiply => [lerp(bg[0],bg[0]*fg[0],fa), lerp(bg[1],bg[1]*fg[1],fa), lerp(bg[2],bg[2]*fg[2],fa), bg[3]],
            LayerBlendMode::Screen => {
                let s = |b: f32, f: f32| 1.0 - (1.0-b)*(1.0-f);
                [lerp(bg[0],s(bg[0],fg[0]),fa), lerp(bg[1],s(bg[1],fg[1]),fa), lerp(bg[2],s(bg[2],fg[2]),fa), bg[3]]
            }
            LayerBlendMode::Overlay => {
                let o = |b: f32, f: f32| if b < 0.5 { 2.0*b*f } else { 1.0 - 2.0*(1.0-b)*(1.0-f) };
                [lerp(bg[0],o(bg[0],fg[0]),fa), lerp(bg[1],o(bg[1],fg[1]),fa), lerp(bg[2],o(bg[2],fg[2]),fa), bg[3]]
            }
            LayerBlendMode::SoftLight => {
                let sl = |b: f32, f: f32| { if f < 0.5 { b - (1.0-2.0*f)*b*(1.0-b) } else { b + (2.0*f-1.0)*(if b < 0.25 { ((16.0*b-12.0)*b+4.0)*b } else { b.sqrt() } - b) } };
                [lerp(bg[0],sl(bg[0],fg[0]),fa), lerp(bg[1],sl(bg[1],fg[1]),fa), lerp(bg[2],sl(bg[2],fg[2]),fa), bg[3]]
            }
            LayerBlendMode::HardLight => {
                let hl = |b: f32, f: f32| if f < 0.5 { 2.0*b*f } else { 1.0 - 2.0*(1.0-b)*(1.0-f) };
                [lerp(bg[0],hl(bg[0],fg[0]),fa), lerp(bg[1],hl(bg[1],fg[1]),fa), lerp(bg[2],hl(bg[2],fg[2]),fa), bg[3]]
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VfxLayerEffect {
    ColorGrade { hue_shift: f32, saturation: f32, brightness: f32, contrast: f32 },
    Bloom { threshold: f32, intensity: f32, scatter: f32 },
    ChromaticAberration { intensity: f32 },
    MotionBlur { strength: f32, samples: u32 },
    DepthOfField { focal_distance: f32, aperture: f32, focal_length: f32 },
}

impl VfxLayerEffect {
    pub fn label(&self) -> &str {
        match self {
            VfxLayerEffect::ColorGrade { .. } => "Color Grade",
            VfxLayerEffect::Bloom { .. } => "Bloom",
            VfxLayerEffect::ChromaticAberration { .. } => "Chromatic Aberration",
            VfxLayerEffect::MotionBlur { .. } => "Motion Blur",
            VfxLayerEffect::DepthOfField { .. } => "Depth of Field",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VfxLayer {
    pub name: String, pub systems: Vec<usize>,
    pub blend_mode: LayerBlendMode, pub opacity: f32, pub visible: bool,
    pub solo: bool, pub locked: bool, pub color: [f32; 3], pub effects: Vec<VfxLayerEffect>,
}

impl Default for VfxLayer {
    fn default() -> Self {
        VfxLayer {
            name: String::from("Layer"), systems: vec![],
            blend_mode: LayerBlendMode::AlphaBlend, opacity: 1.0, visible: true,
            solo: false, locked: false, color: [0.5, 0.5, 1.0], effects: vec![],
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VfxCompositor {
    pub layers: Vec<VfxLayer>,
    pub global_opacity: f32,
    pub preview_composition: bool,
}

impl VfxCompositor {
    pub fn new() -> Self {
        VfxCompositor { layers: vec![VfxLayer { name: String::from("Base Layer"), ..Default::default() }], global_opacity: 1.0, preview_composition: true }
    }

    pub fn add_layer(&mut self, name: &str) -> usize {
        let idx = self.layers.len();
        self.layers.push(VfxLayer { name: name.to_string(), ..Default::default() });
        idx
    }

    pub fn remove_layer(&mut self, idx: usize) { if idx < self.layers.len() { self.layers.remove(idx); } }
    pub fn move_layer_up(&mut self, idx: usize) { if idx > 0 { self.layers.swap(idx - 1, idx); } }
    pub fn move_layer_down(&mut self, idx: usize) { if idx + 1 < self.layers.len() { self.layers.swap(idx, idx + 1); } }

    pub fn visible_layers(&self) -> impl Iterator<Item = (usize, &VfxLayer)> {
        let any_solo = self.layers.iter().any(|l| l.solo);
        self.layers.iter().enumerate().filter(move |(_, l)| if any_solo { l.solo && l.visible } else { l.visible })
    }

    pub fn show(&mut self, ui: &mut egui::Ui, total_systems: usize) {
        ui.heading("VFX Composition");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Global Opacity:");
            ui.add(egui::Slider::new(&mut self.global_opacity, 0.0..=1.0).text(""));
            ui.checkbox(&mut self.preview_composition, "Preview");
        });
        ui.separator();
        if ui.button("+ Add Layer").clicked() {
            let n = format!("Layer {}", self.layers.len());
            self.add_layer(&n);
        }
        let mut action: Option<(usize, u8)> = None;
        egui::ScrollArea::vertical().id_source("vfx_comp_scroll").show(ui, |ui| {
            for i in 0..self.layers.len() {
                ui.push_id(i, |ui| {
                    let layer_color = {
                        let c = &self.layers[i].color;
                        Color32::from_rgb((c[0]*200.0+55.0) as u8, (c[1]*200.0+55.0) as u8, (c[2]*200.0+55.0) as u8)
                    };
                    ui.horizontal(|ui| {
                        let vis_char = if self.layers[i].visible { "O" } else { "_" };
                        if ui.small_button(vis_char).clicked() { self.layers[i].visible = !self.layers[i].visible; }
                        let solo_col = if self.layers[i].solo { Color32::YELLOW } else { Color32::from_gray(120) };
                        if ui.add(egui::Button::new(egui::RichText::new("S").color(solo_col)).small()).clicked() { self.layers[i].solo = !self.layers[i].solo; }
                        let lock_char = if self.layers[i].locked { "L" } else { "U" };
                        if ui.small_button(lock_char).clicked() { self.layers[i].locked = !self.layers[i].locked; }
                        ui.colored_label(layer_color, &self.layers[i].name.clone());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("X").clicked() { action = Some((i, 0)); }
                            if ui.small_button("v").clicked() { action = Some((i, 2)); }
                            if ui.small_button("^").clicked() { action = Some((i, 1)); }
                        });
                    });
                    if !self.layers[i].locked {
                        egui::CollapsingHeader::new(format!("  Layer {} Settings", i)).default_open(false).show(ui, |ui| {
                            ui.horizontal(|ui| { ui.label("Name:"); ui.text_edit_singleline(&mut self.layers[i].name); });
                            ui.horizontal(|ui| {
                                ui.label("Blend:");
                                egui::ComboBox::from_id_source(format!("layer_blend_{}", i)).selected_text(self.layers[i].blend_mode.label()).show_ui(ui, |ui| {
                                    for m in LayerBlendMode::all() { ui.selectable_value(&mut self.layers[i].blend_mode, m.clone(), m.label()); }
                                });
                            });
                            ui.horizontal(|ui| { ui.label("Opacity:"); ui.add(egui::Slider::new(&mut self.layers[i].opacity, 0.0..=1.0)); });
                            ui.separator();
                            ui.label(format!("Systems ({} available):", total_systems));
                            for sys_idx in 0..total_systems {
                                let is_in = self.layers[i].systems.contains(&sys_idx);
                                let mut enabled = is_in;
                                if ui.checkbox(&mut enabled, format!("System {}", sys_idx)).changed() {
                                    if enabled { self.layers[i].systems.push(sys_idx); }
                                    else { self.layers[i].systems.retain(|&s| s != sys_idx); }
                                }
                            }
                            ui.separator();
                            ui.label("Post Effects:");
                            let mut eff_remove = None;
                            for (ei, eff) in self.layers[i].effects.iter_mut().enumerate() {
                                ui.push_id(ei, |ui| {
                                    egui::CollapsingHeader::new(eff.label()).default_open(false).show(ui, |ui| {
                                        match eff {
                                            VfxLayerEffect::ColorGrade { hue_shift, saturation, brightness, contrast } => {
                                                ui.horizontal(|ui| { ui.label("Hue:"); ui.add(egui::DragValue::new(hue_shift).speed(1.0).range(-180.0..=180.0)); ui.label("deg"); });
                                                ui.horizontal(|ui| { ui.label("Sat:"); ui.add(egui::Slider::new(saturation, 0.0..=2.0)); });
                                                ui.horizontal(|ui| { ui.label("Bright:"); ui.add(egui::Slider::new(brightness, 0.0..=2.0)); });
                                                ui.horizontal(|ui| { ui.label("Contrast:"); ui.add(egui::Slider::new(contrast, 0.0..=2.0)); });
                                            }
                                            VfxLayerEffect::Bloom { threshold, intensity, scatter } => {
                                                ui.horizontal(|ui| { ui.label("Threshold:"); ui.add(egui::Slider::new(threshold, 0.0..=1.0)); });
                                                ui.horizontal(|ui| { ui.label("Intensity:"); ui.add(egui::Slider::new(intensity, 0.0..=5.0)); });
                                                ui.horizontal(|ui| { ui.label("Scatter:"); ui.add(egui::Slider::new(scatter, 0.0..=1.0)); });
                                            }
                                            VfxLayerEffect::ChromaticAberration { intensity } => {
                                                ui.horizontal(|ui| { ui.label("Intensity:"); ui.add(egui::Slider::new(intensity, 0.0..=1.0)); });
                                            }
                                            VfxLayerEffect::MotionBlur { strength, samples } => {
                                                ui.horizontal(|ui| { ui.label("Strength:"); ui.add(egui::Slider::new(strength, 0.0..=1.0)); });
                                                ui.horizontal(|ui| { ui.label("Samples:"); ui.add(egui::DragValue::new(samples).speed(1.0).range(2..=32)); });
                                            }
                                            VfxLayerEffect::DepthOfField { focal_distance, aperture, focal_length } => {
                                                ui.horizontal(|ui| { ui.label("Focus:"); ui.add(egui::DragValue::new(focal_distance).speed(0.1)); });
                                                ui.horizontal(|ui| { ui.label("Aperture:"); ui.add(egui::Slider::new(aperture, 0.1..=32.0)); });
                                                ui.horizontal(|ui| { ui.label("Focal Length:"); ui.add(egui::DragValue::new(focal_length).speed(1.0).range(10.0..=200.0)); });
                                            }
                                        }
                                        if ui.small_button("Remove Effect").clicked() { eff_remove = Some(ei); }
                                    });
                                });
                            }
                            if let Some(ei) = eff_remove { self.layers[i].effects.remove(ei); }
                            ui.horizontal(|ui| {
                                if ui.small_button("+ Color Grade").clicked() { self.layers[i].effects.push(VfxLayerEffect::ColorGrade { hue_shift: 0.0, saturation: 1.0, brightness: 1.0, contrast: 1.0 }); }
                                if ui.small_button("+ Bloom").clicked() { self.layers[i].effects.push(VfxLayerEffect::Bloom { threshold: 0.8, intensity: 1.0, scatter: 0.7 }); }
                                if ui.small_button("+ Motion Blur").clicked() { self.layers[i].effects.push(VfxLayerEffect::MotionBlur { strength: 0.3, samples: 8 }); }
                                if ui.small_button("+ Chroma Ab").clicked() { self.layers[i].effects.push(VfxLayerEffect::ChromaticAberration { intensity: 0.1 }); }
                            });
                        });
                    }
                    ui.separator();
                });
            }
        });
        if let Some((idx, op)) = action {
            match op { 0 => self.remove_layer(idx), 1 => self.move_layer_up(idx), 2 => self.move_layer_down(idx), _ => {} }
        }
    }
}

// =============================================================================
// PARTICLE SYSTEM EXTENSIONS CONTAINER
// =============================================================================

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParticleSystemExtensions {
    pub ribbon_emitter: RibbonEmitter,
    pub trail_renderer: TrailRenderer,
    pub mesh_particle: MeshParticle,
    pub sub_emitter_configs: Vec<SubEmitterConfig>,
    pub force_field: ForceFieldModule,
    pub collision_2d: CollisionModule2D,
    pub attractor: AttractorModule,
    pub lod: ParticleLod,
    pub event_system: ParticleEventSystem,
}

impl ParticleSystemExtensions {
    pub fn new() -> Self { ParticleSystemExtensions::default() }

    pub fn show_all_modules(&mut self, ui: &mut egui::Ui, system_names: &[String], sub_emitter_graph: &mut SubEmitterGraph) {
        show_ribbon_emitter_ui(ui, &mut self.ribbon_emitter);
        show_trail_renderer_ui(ui, &mut self.trail_renderer);
        show_mesh_particle_ui(ui, &mut self.mesh_particle);
        show_sub_emitters_ui(ui, &mut self.sub_emitter_configs, system_names, sub_emitter_graph);
        show_force_field_module_ui(ui, &mut self.force_field);
        show_collision_module_2d_ui(ui, &mut self.collision_2d);
        show_attractor_module_ui(ui, &mut self.attractor);
        show_particle_lod_ui(ui, &mut self.lod);
        show_particle_event_system_ui(ui, &mut self.event_system);
    }
}

/// Global extended preview state: ribbons, compositor, curve library, sub-emitter graph.
pub struct ParticleExtendedPreviewState {
    pub ribbon_state: RibbonPreviewState,
    pub compositor: VfxCompositor,
    pub sub_emitter_graph: SubEmitterGraph,
    pub curve_comparison_curves: Vec<(String, Curve)>,
    pub show_curve_library: bool,
    pub show_compositor: bool,
    pub show_sub_emitter_graph: bool,
}

impl ParticleExtendedPreviewState {
    pub fn new() -> Self {
        ParticleExtendedPreviewState {
            ribbon_state: RibbonPreviewState::new(),
            compositor: VfxCompositor::new(),
            sub_emitter_graph: SubEmitterGraph::new(),
            curve_comparison_curves: vec![
                ("Linear".into(), Curve::linear_zero_to_one()),
                ("Bounce".into(), curve_bounce()),
                ("Elastic".into(), curve_elastic()),
            ],
            show_curve_library: false, show_compositor: false, show_sub_emitter_graph: false,
        }
    }

    pub fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.toggle_value(&mut self.show_curve_library, "Curves");
            ui.toggle_value(&mut self.show_compositor, "Compositor");
            ui.toggle_value(&mut self.show_sub_emitter_graph, "Sub-Emitters");
        });
    }

    pub fn show_panels(&mut self, ui: &mut egui::Ui, extensions: &mut ParticleSystemExtensions) {
        if self.show_curve_library {
            egui::CollapsingHeader::new("Curve Library").default_open(true).show(ui, |ui| {
                let presets = all_curve_presets();
                let refs: Vec<(&str, &Curve)> = presets.iter().map(|(n, c)| (*n, c)).collect();
                let comp_refs: Vec<(&str, &Curve)> = self.curve_comparison_curves.iter().map(|(n, c)| (n.as_str(), c)).collect();
                show_curve_comparison(ui, &comp_refs, 300.0, 80.0);
                ui.separator();
                ui.label("Curve Math:");
                ui.horizontal(|ui| {
                    if ui.small_button("Integrate Bounce [0,1]").clicked() {
                        let area = curve_integrate(&curve_bounce(), 0.0, 1.0);
                        ui.label(format!("Area = {:.4}", area));
                    }
                    if ui.small_button("Invert Bell").clicked() {
                        let _inv = curve_invert(&curve_bell());
                    }
                    if ui.small_button("Add to Compare").clicked() {
                        self.curve_comparison_curves.push(("Bell".into(), curve_bell()));
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().id_source("curve_lib_extended").max_height(200.0).show(ui, |ui| {
                    for (name, curve) in &refs {
                        ui.horizontal(|ui| {
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(60.0, 18.0), egui::Sense::click());
                            draw_curve_mini(ui.painter(), rect, curve);
                            if resp.hovered() { ui.painter().rect_stroke(rect, 1.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Outside); }
                            ui.label(egui::RichText::new(*name).size(10.0));
                            if resp.double_clicked() {
                                let owned = name.to_string();
                                if !self.curve_comparison_curves.iter().any(|(n, _)| n == &owned) {
                                    self.curve_comparison_curves.push((owned, (*curve).clone()));
                                }
                            }
                        });
                    }
                });
            });
        }
        if self.show_compositor {
            egui::CollapsingHeader::new("VFX Compositor").default_open(true).show(ui, |ui| {
                self.compositor.show(ui, 0);
            });
        }
        if self.show_sub_emitter_graph {
            egui::CollapsingHeader::new("Sub-Emitter Graph").default_open(true).show(ui, |ui| {
                self.sub_emitter_graph.show(ui);
            });
        }
        let _ = extensions;
    }

    pub fn tick(&mut self, dt: f32, extensions: &ParticleSystemExtensions, canvas_origin: [f32; 2]) {
        self.ribbon_state.tick(dt, &extensions.ribbon_emitter, canvas_origin);
    }

    pub fn draw_preview_overlays(
        &self,
        painter: &Painter,
        canvas_rect: Rect,
        world_to_canvas: impl Fn([f32; 2]) -> Pos2 + Copy,
        extensions: &ParticleSystemExtensions,
    ) {
        self.ribbon_state.draw(painter, canvas_rect);
        extensions.force_field.draw_preview(painter, canvas_rect, world_to_canvas);
        draw_collision_planes_preview(painter, canvas_rect, &extensions.collision_2d.planes, world_to_canvas);
        extensions.attractor.draw_preview(painter, world_to_canvas);
    }
}
