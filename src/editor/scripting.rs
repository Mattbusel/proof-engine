// scripting.rs — Event/hook scripting system for proof-engine editor
// Provides a typed event bus, script hooks (pre/post render, on-update,
// entity lifecycle), a simple expression evaluator, and a condition graph
// for behavior authoring without a full scripting language.

use std::collections::{HashMap, VecDeque};
use std::fmt;

// ─── Event types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    // Entity lifecycle
    EntitySpawned,
    EntityDespawned,
    EntitySelected,
    EntityDeselected,
    EntityMoved,
    EntityRotated,
    EntityScaled,
    EntityReparented,
    EntityRenamed,
    EntityVisibilityChanged,

    // Scene lifecycle
    SceneOpened,
    SceneClosed,
    SceneSaved,
    SceneLoaded,
    SceneModified,

    // Playback
    PlaybackStarted,
    PlaybackStopped,
    PlaybackPaused,
    PlaybackResumed,
    FrameAdvanced,
    TimelineKeyframeAdded,
    TimelineKeyframeRemoved,
    TimelineKeyframeMoved,
    AnimClipChanged,

    // Editor actions
    UndoPerformed,
    RedoPerformed,
    SelectionChanged,
    GizmoModeChanged,
    CameraSnapped,
    PanelFocused,
    ShaderCompiled,
    AssetImported,
    AssetDeleted,
    AssetRenamed,

    // Kit events
    KitParamChanged,
    SdfGraphModified,
    MaterialPainted,
    BoneAdded,
    BoneRemoved,
    IkChainSolved,

    // Physics
    CollisionEntered,
    CollisionExited,
    TriggerEntered,
    TriggerExited,
    ConstraintBroken,

    // Custom
    Custom,
}

impl EventKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::EntitySpawned      => "Entity Spawned",
            Self::EntityDespawned    => "Entity Despawned",
            Self::EntitySelected     => "Entity Selected",
            Self::EntityDeselected   => "Entity Deselected",
            Self::EntityMoved        => "Entity Moved",
            Self::EntityRotated      => "Entity Rotated",
            Self::EntityScaled       => "Entity Scaled",
            Self::EntityReparented   => "Entity Reparented",
            Self::EntityRenamed      => "Entity Renamed",
            Self::EntityVisibilityChanged => "Entity Visibility Changed",
            Self::SceneOpened        => "Scene Opened",
            Self::SceneClosed        => "Scene Closed",
            Self::SceneSaved         => "Scene Saved",
            Self::SceneLoaded        => "Scene Loaded",
            Self::SceneModified      => "Scene Modified",
            Self::PlaybackStarted    => "Playback Started",
            Self::PlaybackStopped    => "Playback Stopped",
            Self::PlaybackPaused     => "Playback Paused",
            Self::PlaybackResumed    => "Playback Resumed",
            Self::FrameAdvanced      => "Frame Advanced",
            Self::TimelineKeyframeAdded   => "Keyframe Added",
            Self::TimelineKeyframeRemoved => "Keyframe Removed",
            Self::TimelineKeyframeMoved   => "Keyframe Moved",
            Self::AnimClipChanged    => "Anim Clip Changed",
            Self::UndoPerformed      => "Undo",
            Self::RedoPerformed      => "Redo",
            Self::SelectionChanged   => "Selection Changed",
            Self::GizmoModeChanged   => "Gizmo Mode Changed",
            Self::CameraSnapped      => "Camera Snapped",
            Self::PanelFocused       => "Panel Focused",
            Self::ShaderCompiled     => "Shader Compiled",
            Self::AssetImported      => "Asset Imported",
            Self::AssetDeleted       => "Asset Deleted",
            Self::AssetRenamed       => "Asset Renamed",
            Self::KitParamChanged    => "Kit Param Changed",
            Self::SdfGraphModified   => "SDF Graph Modified",
            Self::MaterialPainted    => "Material Painted",
            Self::BoneAdded          => "Bone Added",
            Self::BoneRemoved        => "Bone Removed",
            Self::IkChainSolved      => "IK Chain Solved",
            Self::CollisionEntered   => "Collision Entered",
            Self::CollisionExited    => "Collision Exited",
            Self::TriggerEntered     => "Trigger Entered",
            Self::TriggerExited      => "Trigger Exited",
            Self::ConstraintBroken   => "Constraint Broken",
            Self::Custom             => "Custom",
        }
    }

    pub fn is_entity_event(&self) -> bool {
        matches!(self,
            Self::EntitySpawned | Self::EntityDespawned | Self::EntitySelected
            | Self::EntityDeselected | Self::EntityMoved | Self::EntityRotated
            | Self::EntityScaled | Self::EntityReparented | Self::EntityRenamed
            | Self::EntityVisibilityChanged
        )
    }

    pub fn is_playback_event(&self) -> bool {
        matches!(self,
            Self::PlaybackStarted | Self::PlaybackStopped | Self::PlaybackPaused
            | Self::PlaybackResumed | Self::FrameAdvanced
        )
    }
}

// ─── Event payload ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EventPayload {
    None,
    EntityId(u32),
    EntityPair { a: u32, b: u32 },
    String(String),
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Int(i64),
    KitParam { name: String, old: f32, new: f32 },
    AssetPath { old: Option<String>, new: String },
    Selection { ids: Vec<u32>, count: usize },
}

impl EventPayload {
    pub fn as_entity_id(&self) -> Option<u32> {
        match self { Self::EntityId(id) => Some(*id), _ => None }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self { Self::Float(v) => Some(*v), _ => None }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self { Self::String(s) => Some(s), _ => None }
    }
}

#[derive(Debug, Clone)]
pub struct EditorEvent {
    pub kind: EventKind,
    pub payload: EventPayload,
    pub timestamp: f64,
    pub source: EventSource,
    pub propagate: bool,
}

impl EditorEvent {
    pub fn new(kind: EventKind) -> Self {
        Self {
            kind,
            payload: EventPayload::None,
            timestamp: 0.0,
            source: EventSource::Editor,
            propagate: true,
        }
    }

    pub fn with_payload(mut self, payload: EventPayload) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_source(mut self, source: EventSource) -> Self {
        self.source = source;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventSource {
    Editor,
    Script,
    Physics,
    User,
    System,
    Plugin,
}

// ─── Script value ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vec2([f64; 2]),
    Vec3([f64; 3]),
    Vec4([f64; 4]),
    List(Vec<ScriptValue>),
    Map(Vec<(String, ScriptValue)>),
    EntityRef(u32),
}

impl ScriptValue {
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v)   => Some(*v as f64),
            Self::Bool(v)  => Some(if *v { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Null    => false,
            Self::Bool(v) => *v,
            Self::Int(v)  => *v != 0,
            Self::Float(v) => *v != 0.0,
            Self::String(s) => !s.is_empty(),
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null     => "null",
            Self::Bool(_)  => "bool",
            Self::Int(_)   => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Vec2(_)  => "vec2",
            Self::Vec3(_)  => "vec3",
            Self::Vec4(_)  => "vec4",
            Self::List(_)  => "list",
            Self::Map(_)   => "map",
            Self::EntityRef(_) => "entity",
        }
    }

    pub fn add(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a + b),
            (Self::Float(a), Self::Float(b)) => Self::Float(a + b),
            (Self::Int(a), Self::Float(b)) => Self::Float(*a as f64 + b),
            (Self::Float(a), Self::Int(b)) => Self::Float(a + *b as f64),
            (Self::String(a), Self::String(b)) => Self::String(format!("{}{}", a, b)),
            (Self::Vec3(a), Self::Vec3(b)) => Self::Vec3([a[0]+b[0], a[1]+b[1], a[2]+b[2]]),
            _ => Self::Null,
        }
    }

    pub fn mul(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a * b),
            (Self::Float(a), Self::Float(b)) => Self::Float(a * b),
            (Self::Int(a), Self::Float(b)) => Self::Float(*a as f64 * b),
            (Self::Float(a), Self::Int(b)) => Self::Float(a * *b as f64),
            (Self::Vec3(a), Self::Float(b)) => Self::Vec3([a[0]*b, a[1]*b, a[2]*b]),
            _ => Self::Null,
        }
    }

    pub fn equal(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for ScriptValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null       => write!(f, "null"),
            Self::Bool(v)    => write!(f, "{}", v),
            Self::Int(v)     => write!(f, "{}", v),
            Self::Float(v)   => write!(f, "{:.6}", v),
            Self::String(s)  => write!(f, "{}", s),
            Self::Vec2(v)    => write!(f, "({:.3}, {:.3})", v[0], v[1]),
            Self::Vec3(v)    => write!(f, "({:.3}, {:.3}, {:.3})", v[0], v[1], v[2]),
            Self::Vec4(v)    => write!(f, "({:.3}, {:.3}, {:.3}, {:.3})", v[0],v[1],v[2],v[3]),
            Self::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Self::Map(entries) => {
                write!(f, "{{")?;
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Self::EntityRef(id) => write!(f, "Entity({})", id),
        }
    }
}

// ─── Expression AST ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(ScriptValue),
    Variable(String),
    Not(Box<Expr>),
    Negate(Box<Expr>),
    BinOp { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Call { name: String, args: Vec<Expr> },
    Index { expr: Box<Expr>, index: Box<Expr> },
    Field { expr: Box<Expr>, field: String },
    If { cond: Box<Expr>, then: Box<Expr>, else_: Box<Expr> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

impl BinOp {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Add => "+", Self::Sub => "-", Self::Mul => "*",
            Self::Div => "/", Self::Mod => "%",
            Self::Eq => "==", Self::Ne => "!=",
            Self::Lt => "<", Self::Le => "<=", Self::Gt => ">", Self::Ge => ">=",
            Self::And => "&&", Self::Or => "||",
            Self::BitAnd => "&", Self::BitOr => "|", Self::BitXor => "^",
            Self::Shl => "<<", Self::Shr => ">>",
        }
    }
}

// ─── Expression evaluator ────────────────────────────────────────────────────

pub struct EvalContext {
    vars: HashMap<String, ScriptValue>,
    functions: HashMap<String, Box<dyn Fn(&[ScriptValue]) -> ScriptValue>>,
}

impl EvalContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            vars: HashMap::new(),
            functions: HashMap::new(),
        };
        ctx.register_builtins();
        ctx
    }

    fn register_builtins(&mut self) {
        self.functions.insert("abs".into(), Box::new(|args| {
            args.first().and_then(|v| v.as_float()).map(|f| ScriptValue::Float(f.abs())).unwrap_or(ScriptValue::Null)
        }));
        self.functions.insert("sqrt".into(), Box::new(|args| {
            args.first().and_then(|v| v.as_float()).map(|f| ScriptValue::Float(f.sqrt())).unwrap_or(ScriptValue::Null)
        }));
        self.functions.insert("floor".into(), Box::new(|args| {
            args.first().and_then(|v| v.as_float()).map(|f| ScriptValue::Float(f.floor())).unwrap_or(ScriptValue::Null)
        }));
        self.functions.insert("ceil".into(), Box::new(|args| {
            args.first().and_then(|v| v.as_float()).map(|f| ScriptValue::Float(f.ceil())).unwrap_or(ScriptValue::Null)
        }));
        self.functions.insert("round".into(), Box::new(|args| {
            args.first().and_then(|v| v.as_float()).map(|f| ScriptValue::Float(f.round())).unwrap_or(ScriptValue::Null)
        }));
        self.functions.insert("min".into(), Box::new(|args| {
            if args.len() < 2 { return ScriptValue::Null; }
            let a = match args[0].as_float() { Some(v) => v, None => return ScriptValue::Null };
            let b = match args[1].as_float() { Some(v) => v, None => return ScriptValue::Null };
            ScriptValue::Float(a.min(b))
        }));
        self.functions.insert("max".into(), Box::new(|args| {
            if args.len() < 2 { return ScriptValue::Null; }
            let a = match args[0].as_float() { Some(v) => v, None => return ScriptValue::Null };
            let b = match args[1].as_float() { Some(v) => v, None => return ScriptValue::Null };
            ScriptValue::Float(a.max(b))
        }));
        self.functions.insert("clamp".into(), Box::new(|args| {
            if args.len() < 3 { return ScriptValue::Null; }
            let v  = match args[0].as_float() { Some(v) => v, None => return ScriptValue::Null };
            let lo = match args[1].as_float() { Some(v) => v, None => return ScriptValue::Null };
            let hi = match args[2].as_float() { Some(v) => v, None => return ScriptValue::Null };
            ScriptValue::Float(v.clamp(lo, hi))
        }));
        self.functions.insert("lerp".into(), Box::new(|args| {
            if args.len() < 3 { return ScriptValue::Null; }
            let a = match args[0].as_float() { Some(v) => v, None => return ScriptValue::Null };
            let b = match args[1].as_float() { Some(v) => v, None => return ScriptValue::Null };
            let t = match args[2].as_float() { Some(v) => v, None => return ScriptValue::Null };
            ScriptValue::Float(a + (b - a) * t)
        }));
        self.functions.insert("sin".into(), Box::new(|args| {
            args.first().and_then(|v| v.as_float()).map(|f| ScriptValue::Float(f.sin())).unwrap_or(ScriptValue::Null)
        }));
        self.functions.insert("cos".into(), Box::new(|args| {
            args.first().and_then(|v| v.as_float()).map(|f| ScriptValue::Float(f.cos())).unwrap_or(ScriptValue::Null)
        }));
        self.functions.insert("len".into(), Box::new(|args| {
            match args.first() {
                Some(ScriptValue::List(v)) => ScriptValue::Int(v.len() as i64),
                Some(ScriptValue::String(s)) => ScriptValue::Int(s.len() as i64),
                _ => ScriptValue::Null,
            }
        }));
        self.functions.insert("str".into(), Box::new(|args| {
            ScriptValue::String(args.first().map(|v| v.to_string()).unwrap_or_default())
        }));
        self.functions.insert("int".into(), Box::new(|args| {
            match args.first() {
                Some(v) => ScriptValue::Int(v.as_float().unwrap_or(0.0) as i64),
                None => ScriptValue::Null,
            }
        }));
        self.functions.insert("float".into(), Box::new(|args| {
            match args.first() {
                Some(v) => ScriptValue::Float(v.as_float().unwrap_or(0.0)),
                None => ScriptValue::Null,
            }
        }));
        self.functions.insert("bool".into(), Box::new(|args| {
            ScriptValue::Bool(args.first().map(|v| v.as_bool()).unwrap_or(false))
        }));
        self.functions.insert("print".into(), Box::new(|args| {
            let s: Vec<String> = args.iter().map(|v| v.to_string()).collect();
            println!("[script] {}", s.join(" "));
            ScriptValue::Null
        }));
        self.functions.insert("vec3".into(), Box::new(|args| {
            let x = args.get(0).and_then(|v| v.as_float()).unwrap_or(0.0);
            let y = args.get(1).and_then(|v| v.as_float()).unwrap_or(0.0);
            let z = args.get(2).and_then(|v| v.as_float()).unwrap_or(0.0);
            ScriptValue::Vec3([x, y, z])
        }));
        self.functions.insert("dot".into(), Box::new(|args| {
            if let (Some(ScriptValue::Vec3(a)), Some(ScriptValue::Vec3(b))) =
                (args.get(0), args.get(1))
            {
                ScriptValue::Float(a[0]*b[0] + a[1]*b[1] + a[2]*b[2])
            } else {
                ScriptValue::Null
            }
        }));
        self.functions.insert("length".into(), Box::new(|args| {
            if let Some(ScriptValue::Vec3(v)) = args.first() {
                ScriptValue::Float((v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt())
            } else {
                ScriptValue::Null
            }
        }));
        self.functions.insert("normalize".into(), Box::new(|args| {
            if let Some(ScriptValue::Vec3(v)) = args.first() {
                let len = (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
                if len > 1e-10 {
                    ScriptValue::Vec3([v[0]/len, v[1]/len, v[2]/len])
                } else {
                    ScriptValue::Vec3([0.0, 1.0, 0.0])
                }
            } else {
                ScriptValue::Null
            }
        }));
    }

    pub fn set_var(&mut self, name: &str, value: ScriptValue) {
        self.vars.insert(name.to_string(), value);
    }

    pub fn get_var(&self, name: &str) -> &ScriptValue {
        self.vars.get(name).unwrap_or(&ScriptValue::Null)
    }

    pub fn eval(&self, expr: &Expr) -> ScriptValue {
        match expr {
            Expr::Literal(v) => v.clone(),

            Expr::Variable(name) => self.get_var(name).clone(),

            Expr::Not(e) => ScriptValue::Bool(!self.eval(e).as_bool()),

            Expr::Negate(e) => match self.eval(e) {
                ScriptValue::Int(v)   => ScriptValue::Int(-v),
                ScriptValue::Float(v) => ScriptValue::Float(-v),
                _ => ScriptValue::Null,
            },

            Expr::BinOp { op, lhs, rhs } => {
                let l = self.eval(lhs);
                let r = self.eval(rhs);
                match op {
                    BinOp::Add => l.add(&r),
                    BinOp::Mul => l.mul(&r),
                    BinOp::Sub => {
                        match (&l, &r) {
                            (ScriptValue::Int(a), ScriptValue::Int(b)) => ScriptValue::Int(a - b),
                            _ => {
                                let a = l.as_float().unwrap_or(0.0);
                                let b = r.as_float().unwrap_or(0.0);
                                ScriptValue::Float(a - b)
                            }
                        }
                    }
                    BinOp::Div => {
                        let a = l.as_float().unwrap_or(0.0);
                        let b = r.as_float().unwrap_or(0.0);
                        if b.abs() < 1e-300 { ScriptValue::Null } else { ScriptValue::Float(a / b) }
                    }
                    BinOp::Mod => {
                        match (&l, &r) {
                            (ScriptValue::Int(a), ScriptValue::Int(b)) =>
                                if *b != 0 { ScriptValue::Int(a % b) } else { ScriptValue::Null },
                            _ => {
                                let a = l.as_float().unwrap_or(0.0);
                                let b = r.as_float().unwrap_or(0.0);
                                ScriptValue::Float(a % b)
                            }
                        }
                    }
                    BinOp::Eq  => ScriptValue::Bool(l.equal(&r)),
                    BinOp::Ne  => ScriptValue::Bool(!l.equal(&r)),
                    BinOp::Lt  => ScriptValue::Bool(l.as_float().unwrap_or(0.0) < r.as_float().unwrap_or(0.0)),
                    BinOp::Le  => ScriptValue::Bool(l.as_float().unwrap_or(0.0) <= r.as_float().unwrap_or(0.0)),
                    BinOp::Gt  => ScriptValue::Bool(l.as_float().unwrap_or(0.0) > r.as_float().unwrap_or(0.0)),
                    BinOp::Ge  => ScriptValue::Bool(l.as_float().unwrap_or(0.0) >= r.as_float().unwrap_or(0.0)),
                    BinOp::And => ScriptValue::Bool(l.as_bool() && r.as_bool()),
                    BinOp::Or  => ScriptValue::Bool(l.as_bool() || r.as_bool()),
                    BinOp::BitAnd => {
                        match (&l, &r) {
                            (ScriptValue::Int(a), ScriptValue::Int(b)) => ScriptValue::Int(a & b),
                            _ => ScriptValue::Null,
                        }
                    }
                    BinOp::BitOr => {
                        match (&l, &r) {
                            (ScriptValue::Int(a), ScriptValue::Int(b)) => ScriptValue::Int(a | b),
                            _ => ScriptValue::Null,
                        }
                    }
                    BinOp::BitXor => {
                        match (&l, &r) {
                            (ScriptValue::Int(a), ScriptValue::Int(b)) => ScriptValue::Int(a ^ b),
                            _ => ScriptValue::Null,
                        }
                    }
                    BinOp::Shl => {
                        match (&l, &r) {
                            (ScriptValue::Int(a), ScriptValue::Int(b)) => {
                                let shift = (*b).clamp(0, 63) as u32;
                                ScriptValue::Int(a << shift)
                            }
                            _ => ScriptValue::Null,
                        }
                    }
                    BinOp::Shr => {
                        match (&l, &r) {
                            (ScriptValue::Int(a), ScriptValue::Int(b)) => {
                                let shift = (*b).clamp(0, 63) as u32;
                                ScriptValue::Int(a >> shift)
                            }
                            _ => ScriptValue::Null,
                        }
                    }
                }
            }

            Expr::Call { name, args } => {
                let arg_vals: Vec<ScriptValue> = args.iter().map(|a| self.eval(a)).collect();
                if let Some(f) = self.functions.get(name) {
                    f(&arg_vals)
                } else {
                    ScriptValue::Null
                }
            }

            Expr::Index { expr, index } => {
                let v = self.eval(expr);
                let i = self.eval(index);
                match (v, i) {
                    (ScriptValue::List(items), ScriptValue::Int(idx)) => {
                        items.get(idx as usize).cloned().unwrap_or(ScriptValue::Null)
                    }
                    (ScriptValue::Map(entries), ScriptValue::String(key)) => {
                        entries.iter().find(|(k, _)| k == &key)
                            .map(|(_, v)| v.clone())
                            .unwrap_or(ScriptValue::Null)
                    }
                    _ => ScriptValue::Null,
                }
            }

            Expr::Field { expr, field } => {
                match self.eval(expr) {
                    ScriptValue::Vec3(v) => match field.as_str() {
                        "x" => ScriptValue::Float(v[0]),
                        "y" => ScriptValue::Float(v[1]),
                        "z" => ScriptValue::Float(v[2]),
                        _ => ScriptValue::Null,
                    },
                    ScriptValue::Vec4(v) => match field.as_str() {
                        "x" => ScriptValue::Float(v[0]),
                        "y" => ScriptValue::Float(v[1]),
                        "z" => ScriptValue::Float(v[2]),
                        "w" => ScriptValue::Float(v[3]),
                        _ => ScriptValue::Null,
                    },
                    ScriptValue::Map(entries) => {
                        entries.iter().find(|(k, _)| k == field)
                            .map(|(_, v)| v.clone())
                            .unwrap_or(ScriptValue::Null)
                    }
                    _ => ScriptValue::Null,
                }
            }

            Expr::If { cond, then, else_ } => {
                if self.eval(cond).as_bool() {
                    self.eval(then)
                } else {
                    self.eval(else_)
                }
            }
        }
    }
}

impl Default for EvalContext {
    fn default() -> Self { Self::new() }
}

// ─── Condition node graph ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CondNodeId(pub u32);

#[derive(Debug, Clone)]
pub enum CondNode {
    /// Always true / false
    Constant(bool),
    /// Compare a context variable to a literal
    Compare {
        variable: String,
        op: CompareOp,
        value: ScriptValue,
    },
    /// Logic gate
    Gate {
        op: GateOp,
        inputs: Vec<CondNodeId>,
    },
    /// Evaluate an expression
    Expression(Expr),
    /// Random trigger with probability [0,1]
    Random(f64),
    /// Time-based condition: true after elapsed > threshold
    TimeThreshold { elapsed_var: String, threshold: f64 },
    /// Count-based: true after event fired N times
    CountThreshold { count_var: String, threshold: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp { Eq, Ne, Lt, Le, Gt, Ge }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOp { And, Or, Nand, Nor, Xor, Not }

#[derive(Debug, Clone)]
pub struct ConditionGraph {
    nodes: HashMap<CondNodeId, CondNode>,
    root: Option<CondNodeId>,
    next_id: u32,
}

impl ConditionGraph {
    pub fn new() -> Self {
        Self { nodes: HashMap::new(), root: None, next_id: 1 }
    }

    pub fn add(&mut self, node: CondNode) -> CondNodeId {
        let id = CondNodeId(self.next_id);
        self.next_id += 1;
        self.nodes.insert(id, node);
        id
    }

    pub fn set_root(&mut self, id: CondNodeId) { self.root = Some(id); }

    pub fn evaluate(&self, ctx: &EvalContext) -> bool {
        if let Some(root) = self.root {
            self.eval_node(root, ctx)
        } else {
            true
        }
    }

    fn eval_node(&self, id: CondNodeId, ctx: &EvalContext) -> bool {
        let node = match self.nodes.get(&id) {
            Some(n) => n,
            None => return false,
        };
        match node {
            CondNode::Constant(v) => *v,
            CondNode::Compare { variable, op, value } => {
                let var = ctx.get_var(variable);
                match op {
                    CompareOp::Eq => var.equal(value),
                    CompareOp::Ne => !var.equal(value),
                    CompareOp::Lt => var.as_float().unwrap_or(0.0)  < value.as_float().unwrap_or(0.0),
                    CompareOp::Le => var.as_float().unwrap_or(0.0) <= value.as_float().unwrap_or(0.0),
                    CompareOp::Gt => var.as_float().unwrap_or(0.0)  > value.as_float().unwrap_or(0.0),
                    CompareOp::Ge => var.as_float().unwrap_or(0.0) >= value.as_float().unwrap_or(0.0),
                }
            }
            CondNode::Gate { op, inputs } => {
                let vals: Vec<bool> = inputs.iter().map(|&i| self.eval_node(i, ctx)).collect();
                match op {
                    GateOp::And  => vals.iter().all(|&v| v),
                    GateOp::Or   => vals.iter().any(|&v| v),
                    GateOp::Nand => !vals.iter().all(|&v| v),
                    GateOp::Nor  => !vals.iter().any(|&v| v),
                    GateOp::Xor  => vals.iter().filter(|&&v| v).count() % 2 == 1,
                    GateOp::Not  => !vals.first().copied().unwrap_or(false),
                }
            }
            CondNode::Expression(e) => ctx.eval(e).as_bool(),
            CondNode::Random(prob) => {
                // Deterministic hash-based random using time variable
                let t = ctx.get_var("time").as_float().unwrap_or(0.0);
                let h = (t * 2654435761.0) as u64;
                let r = (h ^ (h >> 32)) as f64 / u64::MAX as f64;
                r < *prob
            }
            CondNode::TimeThreshold { elapsed_var, threshold } => {
                ctx.get_var(elapsed_var).as_float().unwrap_or(0.0) > *threshold
            }
            CondNode::CountThreshold { count_var, threshold } => {
                let count = ctx.get_var(count_var).as_float().unwrap_or(0.0) as u64;
                count >= *threshold
            }
        }
    }

    pub fn node_count(&self) -> usize { self.nodes.len() }
}

impl Default for ConditionGraph {
    fn default() -> Self { Self::new() }
}

// ─── Script hook ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HookId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookTiming {
    Before,
    After,
    Instead,
}

#[derive(Debug, Clone)]
pub struct ScriptHook {
    pub id: HookId,
    pub name: String,
    pub event: EventKind,
    pub timing: HookTiming,
    pub enabled: bool,
    pub condition: Option<ConditionGraph>,
    pub actions: Vec<ScriptAction>,
    pub priority: i32,
    pub run_in_editor: bool,
    pub run_in_play: bool,
    pub max_executions: Option<u64>,
    pub execution_count: u64,
}

impl ScriptHook {
    pub fn new(id: HookId, name: String, event: EventKind) -> Self {
        Self {
            id,
            name,
            event,
            timing: HookTiming::After,
            enabled: true,
            condition: None,
            actions: Vec::new(),
            priority: 0,
            run_in_editor: true,
            run_in_play: true,
            max_executions: None,
            execution_count: 0,
        }
    }

    pub fn can_execute(&self, is_playing: bool) -> bool {
        if !self.enabled { return false; }
        if is_playing && !self.run_in_play { return false; }
        if !is_playing && !self.run_in_editor { return false; }
        if let Some(max) = self.max_executions {
            if self.execution_count >= max { return false; }
        }
        true
    }

    pub fn should_fire(&self, ctx: &EvalContext, is_playing: bool) -> bool {
        if !self.can_execute(is_playing) { return false; }
        if let Some(cond) = &self.condition {
            cond.evaluate(ctx)
        } else {
            true
        }
    }
}

// ─── Script action ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ScriptAction {
    SetVar { name: String, expr: Expr },
    FireEvent { kind: EventKind, payload_expr: Option<Expr> },
    Print(Expr),
    If { cond: Expr, then: Vec<ScriptAction>, else_: Vec<ScriptAction> },
    Repeat { count: Expr, body: Vec<ScriptAction> },
    While { cond: Expr, body: Vec<ScriptAction>, max_iters: u32 },
    Break,
    Continue,
    Return(Option<Expr>),
    CallHook(String),
    LogInfo(Expr),
    LogWarn(Expr),
    LogError(Expr),
    Sequence(Vec<ScriptAction>),
    Parallel(Vec<ScriptAction>),
}

impl ScriptAction {
    pub fn execute(&self, ctx: &mut EvalContext, out_events: &mut Vec<EditorEvent>) -> bool {
        match self {
            Self::SetVar { name, expr } => {
                let v = ctx.eval(expr);
                ctx.set_var(name, v);
                true
            }
            Self::FireEvent { kind, payload_expr } => {
                let mut evt = EditorEvent::new(*kind);
                if let Some(e) = payload_expr {
                    let v = ctx.eval(e);
                    evt.payload = match v {
                        ScriptValue::EntityRef(id) => EventPayload::EntityId(id),
                        ScriptValue::Float(f) => EventPayload::Float(f as f32),
                        ScriptValue::String(s) => EventPayload::String(s),
                        ScriptValue::Bool(b) => EventPayload::Bool(b),
                        _ => EventPayload::None,
                    };
                }
                evt.source = EventSource::Script;
                out_events.push(evt);
                true
            }
            Self::Print(e) => {
                println!("[script] {}", ctx.eval(e));
                true
            }
            Self::LogInfo(e) => {
                log::info!("[script] {}", ctx.eval(e));
                true
            }
            Self::LogWarn(e) => {
                log::warn!("[script] {}", ctx.eval(e));
                true
            }
            Self::LogError(e) => {
                log::error!("[script] {}", ctx.eval(e));
                true
            }
            Self::If { cond, then, else_ } => {
                let branch = if ctx.eval(cond).as_bool() { then } else { else_ };
                for action in branch {
                    if !action.execute(ctx, out_events) { return false; }
                }
                true
            }
            Self::Repeat { count, body } => {
                let n = ctx.eval(count).as_float().unwrap_or(0.0) as u32;
                for i in 0..n {
                    ctx.set_var("_i", ScriptValue::Int(i as i64));
                    for action in body {
                        let cont = action.execute(ctx, out_events);
                        if !cont { break; }
                    }
                }
                true
            }
            Self::While { cond, body, max_iters } => {
                let mut iters = 0u32;
                while ctx.eval(cond).as_bool() && iters < *max_iters {
                    for action in body {
                        if !action.execute(ctx, out_events) { break; }
                    }
                    iters += 1;
                }
                true
            }
            Self::Sequence(actions) => {
                for a in actions {
                    if !a.execute(ctx, out_events) { return false; }
                }
                true
            }
            Self::Parallel(actions) => {
                // Simplified: just run sequentially (true parallel needs threading)
                for a in actions { a.execute(ctx, out_events); }
                true
            }
            Self::Break | Self::Continue | Self::Return(_) | Self::CallHook(_) => true,
        }
    }
}

// ─── Event bus ────────────────────────────────────────────────────────────────

pub struct EventBus {
    queue: VecDeque<EditorEvent>,
    history: Vec<EditorEvent>,
    max_history: usize,
    pub event_count: u64,
    pub dropped_count: u64,
    max_queue: usize,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            history: Vec::new(),
            max_history: 512,
            event_count: 0,
            dropped_count: 0,
            max_queue: 4096,
        }
    }

    pub fn push(&mut self, event: EditorEvent) {
        if self.queue.len() >= self.max_queue {
            self.dropped_count += 1;
            return;
        }
        self.queue.push_back(event);
        self.event_count += 1;
    }

    pub fn pop(&mut self) -> Option<EditorEvent> {
        let evt = self.queue.pop_front()?;
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(evt.clone());
        Some(evt)
    }

    pub fn drain(&mut self) -> Vec<EditorEvent> {
        let mut out = Vec::new();
        while let Some(e) = self.pop() {
            out.push(e);
        }
        out
    }

    pub fn peek(&self) -> Option<&EditorEvent> {
        self.queue.front()
    }

    pub fn is_empty(&self) -> bool { self.queue.is_empty() }
    pub fn len(&self) -> usize { self.queue.len() }

    pub fn history(&self) -> &[EditorEvent] { &self.history }

    pub fn last_of_kind(&self, kind: EventKind) -> Option<&EditorEvent> {
        self.history.iter().rev().find(|e| e.kind == kind)
    }
}

impl Default for EventBus {
    fn default() -> Self { Self::new() }
}

// ─── Scripting manager ────────────────────────────────────────────────────────

pub struct ScriptingManager {
    pub hooks: HashMap<HookId, ScriptHook>,
    next_hook_id: u32,
    pub context: EvalContext,
    pub bus: EventBus,
    pub is_playing: bool,
    pending_events: Vec<EditorEvent>,
    pub stats: ScriptingStats,
}

#[derive(Debug, Default, Clone)]
pub struct ScriptingStats {
    pub hooks_fired: u64,
    pub events_processed: u64,
    pub events_dropped: u64,
    pub eval_errors: u64,
    pub total_actions: u64,
}

impl ScriptingManager {
    pub fn new() -> Self {
        let mut ctx = EvalContext::new();
        ctx.set_var("time", ScriptValue::Float(0.0));
        ctx.set_var("frame", ScriptValue::Int(0));
        ctx.set_var("is_playing", ScriptValue::Bool(false));
        ctx.set_var("dt", ScriptValue::Float(0.0));
        ctx.set_var("selection_count", ScriptValue::Int(0));
        Self {
            hooks: HashMap::new(),
            next_hook_id: 1,
            context: ctx,
            bus: EventBus::new(),
            is_playing: false,
            pending_events: Vec::new(),
            stats: ScriptingStats::default(),
        }
    }

    pub fn register_hook(&mut self, hook: ScriptHook) -> HookId {
        let id = hook.id;
        self.hooks.insert(id, hook);
        id
    }

    pub fn new_hook(&mut self, name: &str, event: EventKind) -> HookId {
        let id = HookId(self.next_hook_id);
        self.next_hook_id += 1;
        let hook = ScriptHook::new(id, name.to_string(), event);
        self.hooks.insert(id, hook);
        id
    }

    pub fn remove_hook(&mut self, id: HookId) {
        self.hooks.remove(&id);
    }

    pub fn fire_event(&mut self, event: EditorEvent) {
        self.bus.push(event);
    }

    pub fn update(&mut self, dt: f32) {
        self.context.set_var("dt", ScriptValue::Float(dt as f64));
        let time = self.context.get_var("time").as_float().unwrap_or(0.0) + dt as f64;
        self.context.set_var("time", ScriptValue::Float(time));
        let frame = self.context.get_var("frame").as_float().unwrap_or(0.0) as i64 + 1;
        self.context.set_var("frame", ScriptValue::Int(frame));
        self.context.set_var("is_playing", ScriptValue::Bool(self.is_playing));

        // Process queued events
        let events = self.bus.drain();
        self.stats.events_processed += events.len() as u64;

        for event in events {
            // Set event context vars
            self.context.set_var("event_kind", ScriptValue::String(event.kind.label().to_string()));
            if let Some(id) = event.payload.as_entity_id() {
                self.context.set_var("event_entity", ScriptValue::EntityRef(id));
            }
            if let Some(f) = event.payload.as_float() {
                self.context.set_var("event_value", ScriptValue::Float(f as f64));
            }

            // Collect matching hooks sorted by priority
            let mut matching: Vec<HookId> = self.hooks.iter()
                .filter(|(_, h)| h.event == event.kind)
                .map(|(&id, _)| id)
                .collect();
            matching.sort_by_key(|id| self.hooks[id].priority);

            for hook_id in matching {
                let should_fire = {
                    let hook = &self.hooks[&hook_id];
                    hook.should_fire(&self.context, self.is_playing)
                };
                if should_fire {
                    let actions: Vec<ScriptAction> = self.hooks[&hook_id].actions.clone();
                    for action in &actions {
                        action.execute(&mut self.context, &mut self.pending_events);
                        self.stats.total_actions += 1;
                    }
                    if let Some(hook) = self.hooks.get_mut(&hook_id) {
                        hook.execution_count += 1;
                    }
                    self.stats.hooks_fired += 1;
                }
            }
        }

        // Re-queue any events generated by hooks
        for evt in self.pending_events.drain(..) {
            self.bus.push(evt);
        }

        self.stats.events_dropped = self.bus.dropped_count;
    }

    pub fn hook_count(&self) -> usize { self.hooks.len() }
    pub fn enabled_hook_count(&self) -> usize {
        self.hooks.values().filter(|h| h.enabled).count()
    }
}

impl Default for ScriptingManager {
    fn default() -> Self { Self::new() }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_arithmetic() {
        let ctx = EvalContext::new();
        let expr = Expr::BinOp {
            op: BinOp::Add,
            lhs: Box::new(Expr::Literal(ScriptValue::Float(3.0))),
            rhs: Box::new(Expr::Literal(ScriptValue::Float(4.0))),
        };
        let result = ctx.eval(&expr);
        assert_eq!(result.as_float(), Some(7.0));
    }

    #[test]
    fn eval_builtin_clamp() {
        let ctx = EvalContext::new();
        let expr = Expr::Call {
            name: "clamp".into(),
            args: vec![
                Expr::Literal(ScriptValue::Float(1.5)),
                Expr::Literal(ScriptValue::Float(0.0)),
                Expr::Literal(ScriptValue::Float(1.0)),
            ],
        };
        let result = ctx.eval(&expr);
        assert!((result.as_float().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn condition_graph_and() {
        let mut g = ConditionGraph::new();
        let t = g.add(CondNode::Constant(true));
        let f = g.add(CondNode::Constant(false));
        let gate = g.add(CondNode::Gate { op: GateOp::And, inputs: vec![t, f] });
        g.set_root(gate);
        let ctx = EvalContext::new();
        assert!(!g.evaluate(&ctx));
    }

    #[test]
    fn condition_graph_or() {
        let mut g = ConditionGraph::new();
        let t = g.add(CondNode::Constant(true));
        let f = g.add(CondNode::Constant(false));
        let gate = g.add(CondNode::Gate { op: GateOp::Or, inputs: vec![t, f] });
        g.set_root(gate);
        let ctx = EvalContext::new();
        assert!(g.evaluate(&ctx));
    }

    #[test]
    fn event_bus_fifo() {
        let mut bus = EventBus::new();
        bus.push(EditorEvent::new(EventKind::EntitySpawned));
        bus.push(EditorEvent::new(EventKind::EntityDespawned));
        let first = bus.pop().unwrap();
        assert_eq!(first.kind, EventKind::EntitySpawned);
    }

    #[test]
    fn script_set_var() {
        let mut ctx = EvalContext::new();
        let mut events = Vec::new();
        let action = ScriptAction::SetVar {
            name: "x".into(),
            expr: Expr::Literal(ScriptValue::Float(42.0)),
        };
        action.execute(&mut ctx, &mut events);
        assert_eq!(ctx.get_var("x").as_float(), Some(42.0));
    }

    #[test]
    fn scripting_manager_update() {
        let mut mgr = ScriptingManager::new();
        mgr.fire_event(EditorEvent::new(EventKind::EntitySpawned));
        mgr.update(0.016);
        assert!(mgr.stats.events_processed > 0);
    }

    #[test]
    fn eval_vec3_field() {
        let ctx = EvalContext::new();
        let expr = Expr::Field {
            expr: Box::new(Expr::Literal(ScriptValue::Vec3([1.0, 2.0, 3.0]))),
            field: "y".into(),
        };
        let result = ctx.eval(&expr);
        assert!((result.as_float().unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn interpolation_args() {
        use super::super::localization::TranslationArgs;
        let args = TranslationArgs::new().set("name", "World");
        assert_eq!(args.interpolate("Hello, {name}!"), "Hello, World!");
    }
}
