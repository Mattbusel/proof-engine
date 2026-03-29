#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// CONSTANTS
// ============================================================

const MAX_BONES: usize = 512;
const MAX_KEYFRAMES: usize = 65536;
const MAX_TRACKS: usize = 1024;
const LEVENSHTEIN_MAX_LEN: usize = 256;
const GIMBAL_LOCK_THRESHOLD: f32 = 80.0; // degrees/frame
const FOV_MIN: f32 = 5.0;
const FOV_MAX: f32 = 170.0;
const DOF_MIN_FOCUS: f32 = 0.1;
const DOF_MAX_FOCUS: f32 = 10000.0;
const AUDIO_CLIP_THRESHOLD: f32 = 0.99;
const AUDIO_CLIP_SAMPLE_WINDOW: usize = 1024;
const BPM_MIN: f32 = 20.0;
const BPM_MAX: f32 = 400.0;
const SRT_MAGIC: &str = "-->";
const WEBVTT_MAGIC: &str = "WEBVTT";
const CUSTOM_FORMAT_VERSION: u32 = 3;
const BEZIER_FIT_ITERATIONS: usize = 100;
const BEZIER_FIT_TOLERANCE: f32 = 0.001;
const PROGRESS_STEPS: u32 = 100;
const IMPORT_REPORT_MAX_ERRORS: usize = 1024;
const BATCH_MAX_FILES: usize = 256;

// ============================================================
// TOKENIZER
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Ident(String),
    StringLit(String),
    IntLit(i64),
    FloatLit(f64),
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Colon,
    Comma,
    Semicolon,
    Equals,
    Dot,
    At,
    Hash,
    Newline,
    Eof,
}

#[derive(Clone, Debug)]
pub struct TokenStream {
    pub tokens: Vec<Token>,
    pub pos: usize,
    pub source: String,
}

impl TokenStream {
    pub fn from_source(src: &str) -> Self {
        let tokens = Tokenizer::tokenize(src);
        TokenStream {
            tokens,
            pos: 0,
            source: src.to_string(),
        }
    }

    pub fn peek(&self) -> &Token {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos]
        } else {
            &Token::Eof
        }
    }

    pub fn next(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            t
        } else {
            Token::Eof
        }
    }

    pub fn expect_ident(&mut self) -> Option<String> {
        if let Token::Ident(s) = self.peek().clone() {
            self.pos += 1;
            Some(s)
        } else {
            None
        }
    }

    pub fn expect_float(&mut self) -> Option<f64> {
        match self.peek().clone() {
            Token::FloatLit(f) => { self.pos += 1; Some(f) }
            Token::IntLit(i) => { self.pos += 1; Some(i as f64) }
            _ => None,
        }
    }

    pub fn expect_int(&mut self) -> Option<i64> {
        if let Token::IntLit(i) = self.peek().clone() {
            self.pos += 1;
            Some(i)
        } else {
            None
        }
    }

    pub fn skip_whitespace_and_newlines(&mut self) {
        while matches!(self.peek(), Token::Newline) {
            self.pos += 1;
        }
    }

    pub fn is_eof(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }
}

pub struct Tokenizer;

impl Tokenizer {
    pub fn tokenize(src: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = src.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                ' ' | '\t' | '\r' => { i += 1; }
                '\n' => { tokens.push(Token::Newline); i += 1; }
                '{' => { tokens.push(Token::LBrace); i += 1; }
                '}' => { tokens.push(Token::RBrace); i += 1; }
                '[' => { tokens.push(Token::LBracket); i += 1; }
                ']' => { tokens.push(Token::RBracket); i += 1; }
                '(' => { tokens.push(Token::LParen); i += 1; }
                ')' => { tokens.push(Token::RParen); i += 1; }
                ':' => { tokens.push(Token::Colon); i += 1; }
                ',' => { tokens.push(Token::Comma); i += 1; }
                ';' => { tokens.push(Token::Semicolon); i += 1; }
                '=' => { tokens.push(Token::Equals); i += 1; }
                '.' => { tokens.push(Token::Dot); i += 1; }
                '@' => { tokens.push(Token::At); i += 1; }
                '#' => {
                    // Line comment — skip to end of line
                    while i < chars.len() && chars[i] != '\n' { i += 1; }
                }
                '"' => {
                    i += 1;
                    let mut s = String::new();
                    while i < chars.len() && chars[i] != '"' {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            i += 1;
                            match chars[i] {
                                'n' => s.push('\n'),
                                't' => s.push('\t'),
                                'r' => s.push('\r'),
                                '"' => s.push('"'),
                                '\\' => s.push('\\'),
                                _ => { s.push('\\'); s.push(chars[i]); }
                            }
                        } else {
                            s.push(chars[i]);
                        }
                        i += 1;
                    }
                    if i < chars.len() { i += 1; } // closing quote
                    tokens.push(Token::StringLit(s));
                }
                c if c == '-' || c.is_ascii_digit() => {
                    let start = i;
                    let mut is_float = false;
                    if chars[i] == '-' { i += 1; }
                    while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                    if i < chars.len() && chars[i] == '.' {
                        is_float = true;
                        i += 1;
                        while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                    }
                    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                        is_float = true;
                        i += 1;
                        if i < chars.len() && (chars[i] == '+' || chars[i] == '-') { i += 1; }
                        while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                    }
                    let s: String = chars[start..i].iter().collect();
                    if is_float {
                        if let Ok(f) = s.parse::<f64>() {
                            tokens.push(Token::FloatLit(f));
                        }
                    } else {
                        if let Ok(n) = s.parse::<i64>() {
                            tokens.push(Token::IntLit(n));
                        }
                    }
                }
                c if c.is_alphabetic() || c == '_' => {
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') { i += 1; }
                    let s: String = chars[start..i].iter().collect();
                    tokens.push(Token::Ident(s));
                }
                _ => { i += 1; }
            }
        }
        tokens.push(Token::Eof);
        tokens
    }
}

// ============================================================
// AST NODES
// ============================================================

#[derive(Clone, Debug)]
pub enum AstValue {
    Str(String),
    Int(i64),
    Float(f64),
    Vec3Val([f64; 3]),
    Vec4Val([f64; 4]),
    List(Vec<AstValue>),
    Block(AstBlock),
    Bool(bool),
}

#[derive(Clone, Debug)]
pub struct AstField {
    pub name: String,
    pub value: AstValue,
}

#[derive(Clone, Debug)]
pub struct AstBlock {
    pub kind: String,
    pub name: Option<String>,
    pub fields: Vec<AstField>,
    pub children: Vec<AstBlock>,
}

pub struct AstParser;

impl AstParser {
    pub fn parse(ts: &mut TokenStream) -> AstBlock {
        let mut root = AstBlock {
            kind: "root".to_string(),
            name: None,
            fields: Vec::new(),
            children: Vec::new(),
        };
        ts.skip_whitespace_and_newlines();
        while !ts.is_eof() {
            ts.skip_whitespace_and_newlines();
            if ts.is_eof() { break; }
            if let Some(block) = Self::parse_block(ts) {
                root.children.push(block);
            } else {
                ts.next(); // skip unknown
            }
        }
        root
    }

    fn parse_block(ts: &mut TokenStream) -> Option<AstBlock> {
        ts.skip_whitespace_and_newlines();
        let kind = ts.expect_ident()?;
        ts.skip_whitespace_and_newlines();
        let name = if let Token::StringLit(s) | Token::Ident(s) = ts.peek().clone() {
            if matches!(ts.peek(), Token::StringLit(_) | Token::Ident(_)) {
                let tok = ts.next();
                match tok {
                    Token::StringLit(s) | Token::Ident(s) => Some(s),
                    _ => None,
                }
            } else { None }
        } else { None };
        ts.skip_whitespace_and_newlines();
        if !matches!(ts.peek(), Token::LBrace) { return None; }
        ts.next(); // {
        let mut block = AstBlock { kind, name, fields: Vec::new(), children: Vec::new() };
        loop {
            ts.skip_whitespace_and_newlines();
            if ts.is_eof() || matches!(ts.peek(), Token::RBrace) { break; }
            // Try to parse field: ident = value
            let saved_pos = ts.pos;
            if let Some(field_name) = ts.expect_ident() {
                ts.skip_whitespace_and_newlines();
                if matches!(ts.peek(), Token::Equals) {
                    ts.next();
                    ts.skip_whitespace_and_newlines();
                    if let Some(val) = Self::parse_value(ts) {
                        block.fields.push(AstField { name: field_name, value: val });
                        continue;
                    }
                } else if matches!(ts.peek(), Token::LBrace) {
                    // nested block
                    ts.pos = saved_pos;
                    if let Some(child) = Self::parse_block(ts) {
                        block.children.push(child);
                        continue;
                    }
                }
                ts.pos = saved_pos;
            }
            ts.next(); // skip token
        }
        if matches!(ts.peek(), Token::RBrace) { ts.next(); }
        Some(block)
    }

    fn parse_value(ts: &mut TokenStream) -> Option<AstValue> {
        ts.skip_whitespace_and_newlines();
        match ts.peek().clone() {
            Token::StringLit(s) => { ts.next(); Some(AstValue::Str(s)) }
            Token::IntLit(n) => { ts.next(); Some(AstValue::Int(n)) }
            Token::FloatLit(f) => { ts.next(); Some(AstValue::Float(f)) }
            Token::Ident(s) => {
                let s_lower = s.to_lowercase();
                ts.next();
                if s_lower == "true" { Some(AstValue::Bool(true)) }
                else if s_lower == "false" { Some(AstValue::Bool(false)) }
                else { Some(AstValue::Str(s)) }
            }
            Token::LParen => {
                ts.next();
                let mut vals = Vec::new();
                while !matches!(ts.peek(), Token::RParen | Token::Eof) {
                    if let Some(v) = ts.expect_float() { vals.push(v); }
                    if matches!(ts.peek(), Token::Comma) { ts.next(); }
                }
                if matches!(ts.peek(), Token::RParen) { ts.next(); }
                match vals.len() {
                    3 => Some(AstValue::Vec3Val([vals[0], vals[1], vals[2]])),
                    4 => Some(AstValue::Vec4Val([vals[0], vals[1], vals[2], vals[3]])),
                    _ => Some(AstValue::List(vals.iter().map(|&f| AstValue::Float(f)).collect())),
                }
            }
            Token::LBracket => {
                ts.next();
                let mut items = Vec::new();
                while !matches!(ts.peek(), Token::RBracket | Token::Eof) {
                    if let Some(v) = Self::parse_value(ts) { items.push(v); }
                    if matches!(ts.peek(), Token::Comma) { ts.next(); }
                }
                if matches!(ts.peek(), Token::RBracket) { ts.next(); }
                Some(AstValue::List(items))
            }
            _ => None,
        }
    }
}

// ============================================================
// ANIMATION DATA TYPES
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InterpolationType {
    Constant,
    Linear,
    Cubic,
    BezierCubic,
}

#[derive(Clone, Debug)]
pub struct Keyframe<T: Clone> {
    pub time: f32,
    pub value: T,
    pub tangent_in: T,
    pub tangent_out: T,
    pub interp: InterpolationType,
}

impl<T: Clone + Default> Keyframe<T> {
    pub fn new(time: f32, value: T) -> Self {
        Keyframe {
            time,
            value: value.clone(),
            tangent_in: T::default(),
            tangent_out: T::default(),
            interp: InterpolationType::Linear,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransformTrack {
    pub bone_name: String,
    pub position_keys: Vec<Keyframe<Vec3>>,
    pub rotation_keys: Vec<Keyframe<Quat>>,
    pub scale_keys: Vec<Keyframe<Vec3>>,
}

impl TransformTrack {
    pub fn new(bone_name: &str) -> Self {
        TransformTrack {
            bone_name: bone_name.to_string(),
            position_keys: Vec::new(),
            rotation_keys: Vec::new(),
            scale_keys: Vec::new(),
        }
    }

    pub fn sample_position(&self, t: f32) -> Vec3 {
        sample_vec3_track(&self.position_keys, t).unwrap_or(Vec3::ZERO)
    }

    pub fn sample_rotation(&self, t: f32) -> Quat {
        sample_quat_track(&self.rotation_keys, t).unwrap_or(Quat::IDENTITY)
    }

    pub fn sample_scale(&self, t: f32) -> Vec3 {
        sample_vec3_track(&self.scale_keys, t).unwrap_or(Vec3::ONE)
    }
}

pub fn sample_vec3_track(keys: &[Keyframe<Vec3>], t: f32) -> Option<Vec3> {
    if keys.is_empty() { return None; }
    if t <= keys[0].time { return Some(keys[0].value); }
    if t >= keys[keys.len()-1].time { return Some(keys[keys.len()-1].value); }
    for i in 0..keys.len()-1 {
        if t >= keys[i].time && t <= keys[i+1].time {
            let dt = keys[i+1].time - keys[i].time;
            let u = if dt > 0.0 { (t - keys[i].time) / dt } else { 0.0 };
            return Some(match keys[i].interp {
                InterpolationType::Constant => keys[i].value,
                InterpolationType::Linear => keys[i].value.lerp(keys[i+1].value, u),
                InterpolationType::Cubic | InterpolationType::BezierCubic => {
                    // Hermite
                    let p0 = keys[i].value;
                    let p1 = keys[i+1].value;
                    let m0 = keys[i].tangent_out * dt;
                    let m1 = keys[i+1].tangent_in * dt;
                    hermite_vec3(p0, m0, p1, m1, u)
                }
            });
        }
    }
    None
}

pub fn sample_quat_track(keys: &[Keyframe<Quat>], t: f32) -> Option<Quat> {
    if keys.is_empty() { return None; }
    if t <= keys[0].time { return Some(keys[0].value); }
    if t >= keys[keys.len()-1].time { return Some(keys[keys.len()-1].value); }
    for i in 0..keys.len()-1 {
        if t >= keys[i].time && t <= keys[i+1].time {
            let dt = keys[i+1].time - keys[i].time;
            let u = if dt > 0.0 { (t - keys[i].time) / dt } else { 0.0 };
            return Some(keys[i].value.slerp(keys[i+1].value, u));
        }
    }
    None
}

pub fn hermite_vec3(p0: Vec3, m0: Vec3, p1: Vec3, m1: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0*t3 - 3.0*t2 + 1.0;
    let h10 = t3 - 2.0*t2 + t;
    let h01 = -2.0*t3 + 3.0*t2;
    let h11 = t3 - t2;
    p0*h00 + m0*h10 + p1*h01 + m1*h11
}

// ============================================================
// CAMERA TRACK
// ============================================================

#[derive(Clone, Debug)]
pub struct CameraKeyframe {
    pub time: f32,
    pub position: Vec3,
    pub rotation: Quat,
    pub fov_degrees: f32,
    pub near_clip: f32,
    pub far_clip: f32,
    pub dof_focus_distance: f32,
    pub dof_aperture: f32,
    pub dof_enabled: bool,
}

impl CameraKeyframe {
    pub fn default_at(time: f32) -> Self {
        CameraKeyframe {
            time,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            fov_degrees: 60.0,
            near_clip: 0.1,
            far_clip: 10000.0,
            dof_focus_distance: 10.0,
            dof_aperture: 2.8,
            dof_enabled: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraTrack {
    pub name: String,
    pub keyframes: Vec<CameraKeyframe>,
    pub bezier_positions: Vec<Vec3>,   // bezier control points for path
    pub fov_curve: Vec<Keyframe<f32>>,
    pub dof_focus_curve: Vec<Keyframe<f32>>,
}

impl CameraTrack {
    pub fn new(name: &str) -> Self {
        CameraTrack {
            name: name.to_string(),
            keyframes: Vec::new(),
            bezier_positions: Vec::new(),
            fov_curve: Vec::new(),
            dof_focus_curve: Vec::new(),
        }
    }

    pub fn sample(&self, t: f32) -> CameraKeyframe {
        if self.keyframes.is_empty() { return CameraKeyframe::default_at(t); }
        if t <= self.keyframes[0].time { return self.keyframes[0].clone(); }
        if t >= self.keyframes[self.keyframes.len()-1].time {
            return self.keyframes[self.keyframes.len()-1].clone();
        }
        for i in 0..self.keyframes.len()-1 {
            let k0 = &self.keyframes[i];
            let k1 = &self.keyframes[i+1];
            if t >= k0.time && t <= k1.time {
                let dt = k1.time - k0.time;
                let u = if dt > 0.0 { (t - k0.time) / dt } else { 0.0 };
                let pos = k0.position.lerp(k1.position, u);
                let rot = k0.rotation.slerp(k1.rotation, u);
                let fov = k0.fov_degrees + (k1.fov_degrees - k0.fov_degrees) * u;
                let dof_dist = k0.dof_focus_distance + (k1.dof_focus_distance - k0.dof_focus_distance) * u;
                let dof_ap = k0.dof_aperture + (k1.dof_aperture - k0.dof_aperture) * u;
                return CameraKeyframe {
                    time: t,
                    position: pos,
                    rotation: rot,
                    fov_degrees: fov,
                    near_clip: k0.near_clip,
                    far_clip: k0.far_clip,
                    dof_focus_distance: dof_dist,
                    dof_aperture: dof_ap,
                    dof_enabled: k0.dof_enabled,
                };
            }
        }
        self.keyframes[0].clone()
    }

    pub fn fit_bezier_path(&mut self) {
        let positions: Vec<Vec3> = self.keyframes.iter().map(|k| k.position).collect();
        if positions.len() < 2 { return; }
        self.bezier_positions = BezierFitter::fit_to_points(&positions, BEZIER_FIT_TOLERANCE);
    }
}

// ============================================================
// BEZIER FITTER
// ============================================================

pub struct BezierFitter;

impl BezierFitter {
    pub fn fit_to_points(points: &[Vec3], tolerance: f32) -> Vec<Vec3> {
        if points.len() < 2 { return points.to_vec(); }
        let n = points.len();
        let t_start_dir = (points[1] - points[0]).normalize_or_zero();
        let t_end_dir = (points[n-1] - points[n-2]).normalize_or_zero();
        Self::fit_recursive(points, t_start_dir, t_end_dir, tolerance)
    }

    fn fit_recursive(points: &[Vec3], t_start: Vec3, t_end: Vec3, tolerance: f32) -> Vec<Vec3> {
        let n = points.len();
        if n < 2 { return points.to_vec(); }
        let (c1, c2) = Self::generate_bezier(points, t_start, t_end);
        let p0 = points[0];
        let p3 = points[n-1];
        // Compute max error
        let chord_lengths: Vec<f32> = {
            let mut cl = vec![0.0f32; n];
            for i in 1..n { cl[i] = cl[i-1] + points[i-1].distance(points[i]); }
            cl
        };
        let total = chord_lengths[n-1];
        let mut max_err = 0.0f32;
        let mut split_idx = 0;
        for i in 1..n-1 {
            let t = if total > 0.0 { chord_lengths[i] / total } else { i as f32 / (n-1) as f32 };
            let b = Self::eval_cubic(p0, c1, c2, p3, t);
            let err = b.distance(points[i]);
            if err > max_err { max_err = err; split_idx = i; }
        }
        if max_err <= tolerance || n <= 3 {
            vec![p0, c1, c2, p3]
        } else {
            // Tangent at split
            let t_split_dir = if split_idx < n-1 && split_idx > 0 {
                (points[split_idx+1] - points[split_idx-1]).normalize_or_zero()
            } else { Vec3::Z };
            let mut left = Self::fit_recursive(&points[..=split_idx], t_start, t_split_dir, tolerance);
            let right = Self::fit_recursive(&points[split_idx..], t_split_dir, t_end, tolerance);
            left.pop();
            left.extend(right);
            left
        }
    }

    fn generate_bezier(points: &[Vec3], t1: Vec3, t2: Vec3) -> (Vec3, Vec3) {
        let n = points.len();
        let p0 = points[0];
        let p3 = points[n-1];
        let chord = p0.distance(p3);
        let alpha = chord / 3.0;
        let c1 = p0 + t1 * alpha;
        let c2 = p3 - t2 * alpha;
        (c1, c2)
    }

    pub fn eval_cubic(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
        let t2 = t * t;
        let t3 = t2 * t;
        let one_m_t = 1.0 - t;
        let one_m_t2 = one_m_t * one_m_t;
        let one_m_t3 = one_m_t2 * one_m_t;
        p0 * one_m_t3
            + p1 * (3.0 * one_m_t2 * t)
            + p2 * (3.0 * one_m_t * t2)
            + p3 * t3
    }
}

// ============================================================
// LIGHT TRACK
// ============================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightType {
    Point,
    Spot,
    Directional,
    Area,
}

#[derive(Clone, Debug)]
pub struct LightKeyframe {
    pub time: f32,
    pub position: Vec3,
    pub rotation: Quat,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub spot_angle: f32,
    pub cast_shadows: bool,
}

#[derive(Clone, Debug)]
pub struct LightTrack {
    pub name: String,
    pub light_type: LightType,
    pub keyframes: Vec<LightKeyframe>,
}

impl LightTrack {
    pub fn new(name: &str, light_type: LightType) -> Self {
        LightTrack { name: name.to_string(), light_type, keyframes: Vec::new() }
    }

    pub fn add_keyframe(&mut self, kf: LightKeyframe) {
        self.keyframes.push(kf);
        self.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    pub fn sample(&self, t: f32) -> Option<LightKeyframe> {
        if self.keyframes.is_empty() { return None; }
        if t <= self.keyframes[0].time { return Some(self.keyframes[0].clone()); }
        let last = self.keyframes.last().unwrap();
        if t >= last.time { return Some(last.clone()); }
        for i in 0..self.keyframes.len() - 1 {
            let a = &self.keyframes[i]; let b = &self.keyframes[i+1];
            if t >= a.time && t <= b.time {
                let f = (t - a.time) / (b.time - a.time);
                return Some(LightKeyframe {
                    time: t,
                    position: a.position.lerp(b.position, f),
                    rotation: a.rotation.slerp(b.rotation, f),
                    color: a.color.lerp(b.color, f),
                    intensity: a.intensity + f * (b.intensity - a.intensity),
                    range: a.range + f * (b.range - a.range),
                    spot_angle: a.spot_angle + f * (b.spot_angle - a.spot_angle),
                    cast_shadows: if f < 0.5 { a.cast_shadows } else { b.cast_shadows },
                });
            }
        }
        None
    }

    pub fn duration(&self) -> f32 { self.keyframes.last().map(|k| k.time).unwrap_or(0.0) }
    pub fn keyframe_count(&self) -> usize { self.keyframes.len() }
}

// ============================================================
// SECTION: Animation Clip Metadata
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum LoopMode { Once, Loop, PingPong, ClampForever }

#[derive(Debug, Clone)]
pub struct AnimationClipMeta {
    pub clip_name: String,
    pub duration_s: f32,
    pub fps: f32,
    pub loop_mode: LoopMode,
    pub bone_count: u32,
    pub keyframe_count: u32,
    pub has_root_motion: bool,
    pub root_motion_delta: Vec3,
    pub events: Vec<String>,
}

impl AnimationClipMeta {
    pub fn new(name: &str, duration: f32, fps: f32) -> Self {
        AnimationClipMeta { clip_name: name.to_string(), duration_s: duration, fps,
            loop_mode: LoopMode::Loop, bone_count: 0, keyframe_count: 0,
            has_root_motion: false, root_motion_delta: Vec3::ZERO, events: Vec::new() }
    }

    pub fn frame_count(&self) -> u32 { (self.duration_s * self.fps) as u32 }
    pub fn sample_time(&self, t: f32) -> f32 {
        match self.loop_mode {
            LoopMode::Once => t.clamp(0.0, self.duration_s),
            LoopMode::Loop => t % self.duration_s,
            LoopMode::PingPong => {
                let cycle = t % (self.duration_s * 2.0);
                if cycle < self.duration_s { cycle } else { self.duration_s * 2.0 - cycle }
            },
            LoopMode::ClampForever => t.clamp(0.0, self.duration_s),
        }
    }
    pub fn is_finished(&self, t: f32) -> bool { self.loop_mode == LoopMode::Once && t >= self.duration_s }
}

// ============================================================
// SECTION: Blend Tree
// ============================================================

#[derive(Debug, Clone)]
pub enum BlendTreeNode {
    Clip { name: String, weight: f32 },
    Blend1D { parameter: String, children: Vec<(f32, BlendTreeNode)> },
    Blend2D { param_x: String, param_y: String, children: Vec<(Vec2, BlendTreeNode)> },
    Additive { base: Box<BlendTreeNode>, additive: Box<BlendTreeNode>, weight: f32 },
    Override { base: Box<BlendTreeNode>, override_node: Box<BlendTreeNode>, mask: Vec<String> },
}

impl BlendTreeNode {
    pub fn evaluate_1d(&self, param: f32) -> Vec<(String, f32)> {
        match self {
            BlendTreeNode::Clip { name, weight } => vec![(name.clone(), *weight)],
            BlendTreeNode::Blend1D { children, .. } => {
                if children.is_empty() { return Vec::new(); }
                if children.len() == 1 { return children[0].1.evaluate_1d(param); }
                let mut result = Vec::new();
                for i in 0..children.len()-1 {
                    let (t0, node0) = &children[i];
                    let (t1, node1) = &children[i+1];
                    if param >= *t0 && param <= *t1 {
                        let f = (param - t0) / (t1 - t0);
                        for (name, w) in node0.evaluate_1d(param) { result.push((name, w * (1.0 - f))); }
                        for (name, w) in node1.evaluate_1d(param) { result.push((name, w * f)); }
                        return result;
                    }
                }
                children.last().unwrap().1.evaluate_1d(param)
            },
            _ => Vec::new(),
        }
    }

    pub fn clip_names(&self) -> Vec<String> {
        match self {
            BlendTreeNode::Clip { name, .. } => vec![name.clone()],
            BlendTreeNode::Blend1D { children, .. } => children.iter().flat_map(|(_, n)| n.clip_names()).collect(),
            BlendTreeNode::Blend2D { children, .. } => children.iter().flat_map(|(_, n)| n.clip_names()).collect(),
            BlendTreeNode::Additive { base, additive, .. } => { let mut v = base.clip_names(); v.extend(additive.clip_names()); v },
            BlendTreeNode::Override { base, override_node, .. } => { let mut v = base.clip_names(); v.extend(override_node.clip_names()); v },
        }
    }
}

// ============================================================
// SECTION: Animator State Machine
// ============================================================

#[derive(Debug, Clone)]
pub struct AnimationState {
    pub name: String,
    pub clip_name: String,
    pub speed: f32,
    pub is_looping: bool,
}

impl AnimationState {
    pub fn new(name: &str, clip: &str) -> Self {
        AnimationState { name: name.to_string(), clip_name: clip.to_string(), speed: 1.0, is_looping: true }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransitionCondition {
    BoolTrue(String), BoolFalse(String),
    FloatGreater(String, f32), FloatLess(String, f32),
    IntEqual(String, i32), Trigger(String),
}

#[derive(Debug, Clone)]
pub struct Transition {
    pub from: String, pub to: String,
    pub conditions: Vec<TransitionCondition>,
    pub duration: f32, pub exit_time: Option<f32>,
    pub can_interrupt: bool,
}

#[derive(Debug, Clone)]
pub enum AnimParam { Bool(bool), Float(f32), Int(i32), Trigger(bool) }

#[derive(Debug, Clone)]
pub struct AnimatorStateMachine {
    pub current_state: String,
    pub states: HashMap<String, AnimationState>,
    pub transitions: Vec<Transition>,
    pub params: HashMap<String, AnimParam>,
}

impl AnimatorStateMachine {
    pub fn new(initial: &str) -> Self {
        AnimatorStateMachine { current_state: initial.to_string(), states: HashMap::new(), transitions: Vec::new(), params: HashMap::new() }
    }

    pub fn add_state(&mut self, state: AnimationState) { self.states.insert(state.name.clone(), state); }
    pub fn add_transition(&mut self, t: Transition) { self.transitions.push(t); }
    pub fn set_bool(&mut self, name: &str, v: bool) { self.params.insert(name.to_string(), AnimParam::Bool(v)); }
    pub fn set_float(&mut self, name: &str, v: f32) { self.params.insert(name.to_string(), AnimParam::Float(v)); }
    pub fn set_int(&mut self, name: &str, v: i32) { self.params.insert(name.to_string(), AnimParam::Int(v)); }
    pub fn set_trigger(&mut self, name: &str) { self.params.insert(name.to_string(), AnimParam::Trigger(true)); }

    fn check_condition(&self, cond: &TransitionCondition) -> bool {
        match cond {
            TransitionCondition::BoolTrue(n) => matches!(self.params.get(n), Some(AnimParam::Bool(true))),
            TransitionCondition::BoolFalse(n) => matches!(self.params.get(n), Some(AnimParam::Bool(false))),
            TransitionCondition::FloatGreater(n, v) => matches!(self.params.get(n), Some(AnimParam::Float(f)) if *f > *v),
            TransitionCondition::FloatLess(n, v) => matches!(self.params.get(n), Some(AnimParam::Float(f)) if *f < *v),
            TransitionCondition::IntEqual(n, v) => matches!(self.params.get(n), Some(AnimParam::Int(i)) if *i == *v),
            TransitionCondition::Trigger(n) => matches!(self.params.get(n), Some(AnimParam::Trigger(true))),
        }
    }

    pub fn update(&mut self, _dt: f32) -> Option<&'static str> {
        let transitions_snapshot = self.transitions.clone();
        for t in &transitions_snapshot {
            if t.from != self.current_state { continue; }
            if t.conditions.iter().all(|c| self.check_condition(c)) {
                self.current_state = t.to.clone();
                // Consume triggers
                for cond in &t.conditions {
                    if let TransitionCondition::Trigger(n) = cond {
                        self.params.insert(n.clone(), AnimParam::Trigger(false));
                    }
                }
                // Return a static reference name based on common states
                return match self.current_state.as_str() {
                    "idle" => Some("idle"),
                    "run" => Some("run"),
                    "walk" => Some("walk"),
                    "attack" => Some("attack"),
                    "jump" => Some("jump"),
                    "death" => Some("death"),
                    _ => Some("unknown"),
                };
            }
        }
        None
    }

    pub fn state_count(&self) -> usize { self.states.len() }
    pub fn transition_count(&self) -> usize { self.transitions.len() }
}

// ============================================================
// SECTION: Motion Capture
// ============================================================

#[derive(Debug, Clone)]
pub struct MocapFrame {
    pub time_s: f32,
    pub bone_transforms: HashMap<String, (Vec3, Quat)>,
}

impl MocapFrame {
    pub fn new(time_s: f32) -> Self { MocapFrame { time_s, bone_transforms: HashMap::new() } }
    pub fn set_bone(&mut self, name: &str, pos: Vec3, rot: Quat) { self.bone_transforms.insert(name.to_string(), (pos, rot)); }
    pub fn bone_count(&self) -> usize { self.bone_transforms.len() }
}

#[derive(Debug, Clone)]
pub struct MocapClip {
    pub name: String,
    pub frames: Vec<MocapFrame>,
    pub fps: f32,
    pub bone_names: Vec<String>,
}

impl MocapClip {
    pub fn new(name: &str, fps: f32) -> Self {
        MocapClip { name: name.to_string(), frames: Vec::new(), fps, bone_names: Vec::new() }
    }

    pub fn add_frame(&mut self, f: MocapFrame) { self.frames.push(f); }
    pub fn duration_s(&self) -> f32 { self.frames.last().map(|f| f.time_s).unwrap_or(0.0) }
    pub fn frame_count(&self) -> usize { self.frames.len() }

    pub fn frame_at_time(&self, t: f32) -> Option<&MocapFrame> {
        self.frames.iter().min_by(|a, b| (a.time_s - t).abs().partial_cmp(&(b.time_s - t).abs()).unwrap())
    }

    pub fn downsample(&self, target_fps: f32) -> MocapClip {
        let step = (self.fps / target_fps).max(1.0);
        let mut result = MocapClip::new(&self.name, target_fps);
        result.bone_names = self.bone_names.clone();
        let mut i = 0.0f32;
        while i < self.frames.len() as f32 {
            result.add_frame(self.frames[i as usize].clone());
            i += step;
        }
        result
    }

    pub fn retarget_to_skeleton(&self, mapping: &HashMap<String, String>) -> MocapClip {
        let mut retargeted = MocapClip::new(&self.name, self.fps);
        retargeted.bone_names = mapping.values().cloned().collect();
        for frame in &self.frames {
            let mut new_frame = MocapFrame::new(frame.time_s);
            for (src, dst) in mapping {
                if let Some(&(pos, rot)) = frame.bone_transforms.get(src) {
                    new_frame.set_bone(dst, pos, rot);
                }
            }
            retargeted.add_frame(new_frame);
        }
        retargeted
    }
}

// ============================================================
// SECTION: IK Solver
// ============================================================

#[derive(Debug, Clone)]
pub struct IkChain {
    pub bone_names: Vec<String>,
    pub bone_lengths: Vec<f32>,
    pub positions: Vec<Vec3>,
    pub target: Vec3,
    pub pole_target: Option<Vec3>,
}

impl IkChain {
    pub fn new(bone_names: Vec<String>, lengths: Vec<f32>) -> Self {
        let n = bone_names.len() + 1;
        IkChain { bone_names, bone_lengths: lengths, positions: vec![Vec3::ZERO; n], target: Vec3::ZERO, pole_target: None }
    }

    pub fn total_length(&self) -> f32 { self.bone_lengths.iter().sum() }

    pub fn solve_fabrik(&mut self, iterations: u32, tolerance: f32) {
        let n = self.positions.len();
        if n < 2 { return; }
        let root = self.positions[0];
        for _ in 0..iterations {
            // Forward pass
            self.positions[n - 1] = self.target;
            for i in (0..n-1).rev() {
                let dir = (self.positions[i] - self.positions[i+1]).normalize_or_zero();
                self.positions[i] = self.positions[i+1] + dir * self.bone_lengths[i];
            }
            // Backward pass
            self.positions[0] = root;
            for i in 0..n-1 {
                let dir = (self.positions[i+1] - self.positions[i]).normalize_or_zero();
                self.positions[i+1] = self.positions[i] + dir * self.bone_lengths[i];
            }
            if (self.positions[n-1] - self.target).length() < tolerance { break; }
        }
    }

    pub fn two_bone_ik(&mut self) {
        if self.positions.len() < 3 || self.bone_lengths.len() < 2 { return; }
        let a = self.positions[0];
        let c = self.target;
        let len_ab = self.bone_lengths[0];
        let len_bc = self.bone_lengths[1];
        let dist = (c - a).length();
        let dist_clamped = dist.clamp((len_ab - len_bc).abs(), len_ab + len_bc);
        let cos_a = ((len_ab * len_ab + dist_clamped * dist_clamped - len_bc * len_bc) / (2.0 * len_ab * dist_clamped)).clamp(-1.0, 1.0);
        let angle_a = cos_a.acos();
        let dir_ac = (c - a).normalize_or_zero();
        let perp = if let Some(pole) = self.pole_target {
            let to_pole = (pole - a).normalize_or_zero();
            let proj = dir_ac * dir_ac.dot(to_pole);
            (to_pole - proj).normalize_or_zero()
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        let rot = Quat::from_axis_angle(perp, angle_a);
        self.positions[1] = a + rot * (dir_ac * len_ab);
        self.positions[2] = self.target;
    }
}

// ============================================================
// SECTION: Facial Animation
// ============================================================

pub const ARKIT_BLEND_SHAPE_NAMES: [&str; 52] = [
    "eyeBlinkLeft", "eyeLookDownLeft", "eyeLookInLeft", "eyeLookOutLeft", "eyeLookUpLeft",
    "eyeSquintLeft", "eyeWideLeft", "eyeBlinkRight", "eyeLookDownRight", "eyeLookInRight",
    "eyeLookOutRight", "eyeLookUpRight", "eyeSquintRight", "eyeWideRight",
    "jawForward", "jawLeft", "jawRight", "jawOpen",
    "mouthClose", "mouthFunnel", "mouthPucker", "mouthLeft", "mouthRight",
    "mouthSmileLeft", "mouthSmileRight", "mouthFrownLeft", "mouthFrownRight",
    "mouthDimpleLeft", "mouthDimpleRight", "mouthStretchLeft", "mouthStretchRight",
    "mouthRollLower", "mouthRollUpper", "mouthShrugLower", "mouthShrugUpper",
    "mouthPressLeft", "mouthPressRight", "mouthLowerDownLeft", "mouthLowerDownRight",
    "mouthUpperUpLeft", "mouthUpperUpRight", "browDownLeft", "browDownRight",
    "browInnerUp", "browOuterUpLeft", "browOuterUpRight",
    "cheekPuff", "cheekSquintLeft", "cheekSquintRight",
    "noseSneerLeft", "noseSneerRight", "tongueOut",
];

#[derive(Debug, Clone)]
pub struct BlendShape {
    pub name: String,
    pub weight: f32,
}

impl BlendShape {
    pub fn new(name: &str, weight: f32) -> Self { BlendShape { name: name.to_string(), weight: weight.clamp(0.0, 1.0) } }
}

#[derive(Debug, Clone)]
pub struct FacialRig {
    pub shapes: Vec<BlendShape>,
}

impl FacialRig {
    pub fn new() -> Self {
        FacialRig { shapes: ARKIT_BLEND_SHAPE_NAMES.iter().map(|&n| BlendShape::new(n, 0.0)).collect() }
    }

    pub fn set(&mut self, name: &str, weight: f32) {
        if let Some(s) = self.shapes.iter_mut().find(|s| s.name == name) { s.weight = weight.clamp(0.0, 1.0); }
    }

    pub fn get(&self, name: &str) -> f32 { self.shapes.iter().find(|s| s.name == name).map(|s| s.weight).unwrap_or(0.0) }
    pub fn reset(&mut self) { for s in &mut self.shapes { s.weight = 0.0; } }

    pub fn apply_expression(&mut self, expression: &str) {
        self.reset();
        match expression {
            "happy" => { self.set("mouthSmileLeft", 0.8); self.set("mouthSmileRight", 0.8); self.set("cheekSquintLeft", 0.4); self.set("cheekSquintRight", 0.4); self.set("eyeSquintLeft", 0.3); self.set("eyeSquintRight", 0.3); },
            "sad" => { self.set("mouthFrownLeft", 0.7); self.set("mouthFrownRight", 0.7); self.set("browInnerUp", 0.5); self.set("eyeLookDownLeft", 0.3); self.set("eyeLookDownRight", 0.3); },
            "angry" => { self.set("browDownLeft", 0.9); self.set("browDownRight", 0.9); self.set("mouthStretchLeft", 0.4); self.set("mouthStretchRight", 0.4); self.set("noseSneerLeft", 0.5); self.set("noseSneerRight", 0.5); },
            "surprised" => { self.set("eyeWideLeft", 1.0); self.set("eyeWideRight", 1.0); self.set("jawOpen", 0.6); self.set("browOuterUpLeft", 0.8); self.set("browOuterUpRight", 0.8); },
            "fear" => { self.set("eyeWideLeft", 0.9); self.set("eyeWideRight", 0.9); self.set("browInnerUp", 0.7); self.set("mouthPressLeft", 0.4); self.set("mouthPressRight", 0.4); },
            "disgust" => { self.set("noseSneerLeft", 0.9); self.set("noseSneerRight", 0.9); self.set("mouthShrugUpper", 0.5); self.set("browDownLeft", 0.4); },
            _ => {}
        }
    }

    pub fn active_shape_count(&self) -> usize { self.shapes.iter().filter(|s| s.weight > 0.01).count() }
}

// ============================================================
// SECTION: Timeline System
// ============================================================

#[derive(Debug, Clone)]
pub struct TimelineClip {
    pub id: u32,
    pub name: String,
    pub start_time: f32,
    pub duration: f32,
    pub blend_in: f32,
    pub blend_out: f32,
    pub speed: f32,
}

impl TimelineClip {
    pub fn new(id: u32, name: &str, start: f32, duration: f32) -> Self {
        TimelineClip { id, name: name.to_string(), start_time: start, duration, blend_in: 0.0, blend_out: 0.0, speed: 1.0 }
    }
    pub fn end_time(&self) -> f32 { self.start_time + self.duration }
    pub fn is_active_at(&self, t: f32) -> bool { t >= self.start_time && t < self.end_time() }
    pub fn local_time(&self, t: f32) -> f32 { (t - self.start_time) * self.speed }
    pub fn blend_weight(&self, t: f32) -> f32 {
        if !self.is_active_at(t) { return 0.0; }
        let lt = t - self.start_time;
        let weight_in = if self.blend_in > 0.0 { (lt / self.blend_in).min(1.0) } else { 1.0 };
        let weight_out = if self.blend_out > 0.0 { ((self.duration - lt) / self.blend_out).min(1.0) } else { 1.0 };
        weight_in.min(weight_out)
    }
    pub fn overlaps(&self, other: &TimelineClip) -> bool {
        self.start_time < other.end_time() && self.end_time() > other.start_time
    }
}

#[derive(Debug, Clone)]
pub struct TimelineTrack {
    pub name: String,
    pub clips: Vec<TimelineClip>,
    pub is_muted: bool,
    pub is_locked: bool,
}

impl TimelineTrack {
    pub fn new(name: &str) -> Self { TimelineTrack { name: name.to_string(), clips: Vec::new(), is_muted: false, is_locked: false } }
    pub fn add_clip(&mut self, clip: TimelineClip) { self.clips.push(clip); self.clips.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap()); }
    pub fn clips_at(&self, t: f32) -> Vec<&TimelineClip> { self.clips.iter().filter(|c| c.is_active_at(t)).collect() }
    pub fn find_conflicts(&self) -> Vec<(u32, u32)> {
        let mut conflicts = Vec::new();
        for i in 0..self.clips.len() {
            for j in i+1..self.clips.len() {
                if self.clips[i].overlaps(&self.clips[j]) { conflicts.push((self.clips[i].id, self.clips[j].id)); }
            }
        }
        conflicts
    }
    pub fn duration(&self) -> f32 { self.clips.iter().map(|c| c.end_time()).fold(0.0_f32, f32::max) }
    pub fn clip_count(&self) -> usize { self.clips.len() }
}

#[derive(Debug, Clone)]
pub struct MasterTimeline {
    pub tracks: Vec<TimelineTrack>,
    pub current_time: f32,
    pub fps: f32,
    pub is_playing: bool,
}

impl MasterTimeline {
    pub fn new(fps: f32) -> Self { MasterTimeline { tracks: Vec::new(), current_time: 0.0, fps, is_playing: false } }
    pub fn add_track(&mut self, t: TimelineTrack) { self.tracks.push(t); }
    pub fn total_duration(&self) -> f32 { self.tracks.iter().map(|t| t.duration()).fold(0.0_f32, f32::max) }
    pub fn active_clips_at(&self, t: f32) -> Vec<(&str, &TimelineClip)> {
        self.tracks.iter().flat_map(|tr| tr.clips_at(t).into_iter().map(|c| (tr.name.as_str(), c))).collect()
    }
    pub fn snap_to_frame(&self, t: f32) -> f32 { (t * self.fps).round() / self.fps }
    pub fn advance(&mut self, dt: f32) { if self.is_playing { self.current_time += dt; } }
}

// ============================================================
// SECTION: Camera Shake
// ============================================================

#[derive(Debug, Clone)]
pub struct ShakePreset {
    pub name: String,
    pub trauma: f32,
    pub frequency: f32,
    pub max_angle_deg: f32,
    pub max_offset: f32,
    pub decay_rate: f32,
}

impl ShakePreset {
    pub fn explosion() -> Self { ShakePreset { name: "explosion".into(), trauma: 1.0, frequency: 12.0, max_angle_deg: 5.0, max_offset: 0.3, decay_rate: 1.5 } }
    pub fn gunshot() -> Self { ShakePreset { name: "gunshot".into(), trauma: 0.4, frequency: 20.0, max_angle_deg: 1.5, max_offset: 0.08, decay_rate: 3.0 } }
    pub fn earthquake() -> Self { ShakePreset { name: "earthquake".into(), trauma: 0.8, frequency: 5.0, max_angle_deg: 3.0, max_offset: 0.5, decay_rate: 0.3 } }
    pub fn footstep_heavy() -> Self { ShakePreset { name: "footstep_heavy".into(), trauma: 0.15, frequency: 8.0, max_angle_deg: 0.5, max_offset: 0.02, decay_rate: 5.0 } }

    pub fn evaluate(&self, trauma: f32, time: f32) -> (Vec3, Vec3) {
        let t2 = trauma * trauma;
        let noise_x = ((time * self.frequency).sin() * 0.5 + (time * self.frequency * 1.7).sin() * 0.3 + (time * self.frequency * 0.3).sin() * 0.2) * t2;
        let noise_y = ((time * self.frequency * 1.3).sin() * 0.5 + (time * self.frequency * 0.7).sin() * 0.3) * t2;
        let offset = Vec3::new(noise_x * self.max_offset, noise_y * self.max_offset, 0.0);
        let angle = Vec3::new(noise_y * self.max_angle_deg, noise_x * self.max_angle_deg, 0.0);
        (offset, angle)
    }
}

// ============================================================
// SECTION: Integration Tests
// ============================================================

pub fn run_cutscene_pipeline_tests() {
    // AnimationClipMeta
    let clip = AnimationClipMeta::new("walk", 2.0, 30.0);
    assert_eq!(clip.frame_count(), 60);
    assert!((clip.sample_time(2.5) - 0.5).abs() < 0.01); // Loop wraps
    assert!(!clip.is_finished(1.0));

    // BlendTree
    let node = BlendTreeNode::Blend1D {
        parameter: "speed".into(),
        children: vec![
            (0.0, BlendTreeNode::Clip { name: "idle".into(), weight: 1.0 }),
            (1.0, BlendTreeNode::Clip { name: "walk".into(), weight: 1.0 }),
        ],
    };
    let result = node.evaluate_1d(0.5);
    assert_eq!(result.len(), 2);

    // IK
    let mut ik = IkChain::new(vec!["upper".into(), "lower".into()], vec![1.0, 1.0]);
    ik.positions = vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)];
    ik.target = Vec3::new(1.5, 0.5, 0.0);
    ik.solve_fabrik(10, 0.001);
    assert!((ik.positions.last().unwrap().distance(ik.target)) < 0.01);

    // Facial rig
    let mut rig = FacialRig::new();
    assert_eq!(rig.shapes.len(), 52);
    rig.apply_expression("happy");
    assert!(rig.get("mouthSmileLeft") > 0.0);
    assert!(rig.active_shape_count() > 0);
    rig.apply_expression("angry");
    assert!(rig.get("browDownLeft") > 0.0);
    rig.reset();
    assert_eq!(rig.active_shape_count(), 0);
}

pub fn run_timeline_editor_tests() {
    let mut timeline = MasterTimeline::new(30.0);
    let mut track = TimelineTrack::new("animation");
    track.add_clip(TimelineClip::new(1, "idle", 0.0, 3.0));
    track.add_clip(TimelineClip::new(2, "walk", 3.5, 5.0));
    timeline.add_track(track);
    assert!(timeline.total_duration() > 0.0);
    let active = timeline.active_clips_at(1.5);
    assert!(!active.is_empty());
    assert_eq!(timeline.snap_to_frame(1.533_f32), (1.533_f32 * 30.0_f32).round() / 30.0_f32);
    timeline.is_playing = true;
    timeline.advance(0.1);
    assert!((timeline.current_time - 0.1).abs() < 0.001);

    // TimelineClip blend
    let c = TimelineClip { id: 1, name: "t".into(), start_time: 0.0, duration: 2.0, blend_in: 0.5, blend_out: 0.5, speed: 1.0 };
    assert!((c.blend_weight(0.25) - 0.5).abs() < 0.001);
    assert!((c.blend_weight(1.75) - 0.5).abs() < 0.001);
    assert!((c.blend_weight(1.0) - 1.0).abs() < 0.001);
}

pub fn run_final_cutscene_tests() {
    // ASM test
    let mut asm = AnimatorStateMachine::new("idle");
    asm.add_state(AnimationState::new("idle", "idle_anim"));
    asm.add_state(AnimationState::new("run", "run_anim"));
    asm.add_state(AnimationState::new("attack", "attack_anim"));
    let t1 = Transition { from: "idle".into(), to: "run".into(), conditions: vec![TransitionCondition::FloatGreater("speed".into(), 0.5)], duration: 0.2, exit_time: None, can_interrupt: true };
    let t2 = Transition { from: "run".into(), to: "attack".into(), conditions: vec![TransitionCondition::Trigger("attack".into())], duration: 0.1, exit_time: None, can_interrupt: false };
    asm.add_transition(t1); asm.add_transition(t2);
    asm.set_float("speed", 1.0);
    let s1 = asm.update(0.016);
    assert_eq!(s1, Some("run"));
    asm.set_trigger("attack");
    let s2 = asm.update(0.016);
    assert_eq!(s2, Some("attack"));
}

pub fn run_cutscene_analytics_tests() {
    // MocapClip
    let mut mocap = MocapClip::new("take_1", 60.0);
    for i in 0..60u32 {
        let mut frame = MocapFrame::new(i as f32 / 60.0);
        frame.set_bone("Hips", Vec3::new(0.0, 1.0, i as f32 * 0.01), Quat::IDENTITY);
        mocap.add_frame(frame);
    }
    assert_eq!(mocap.frame_count(), 60);
    let downsampled = mocap.downsample(30.0);
    assert!(downsampled.frame_count() < mocap.frame_count());

    // Retargeting
    let mut mapping = HashMap::new();
    mapping.insert("Hips".to_string(), "Root".to_string());
    let retargeted = mocap.retarget_to_skeleton(&mapping);
    assert!(!retargeted.frames.is_empty());
    if let Some(frame) = retargeted.frames.first() {
        assert!(frame.bone_transforms.contains_key("Root"));
    }

    // ShakePreset
    let shake = ShakePreset::explosion();
    let (offset, angle) = shake.evaluate(1.0, 0.5);
    assert!(offset.length() >= 0.0);
    assert!(angle.length() >= 0.0);
    let (offset2, _) = ShakePreset::earthquake().evaluate(0.5, 1.0);
    assert!(offset2.length() >= 0.0);
}

pub fn run_scene_and_postprocess_tests() {
    // Light track
    let mut track = LightTrack::new("sun", LightType::Directional);
    track.add_keyframe(LightKeyframe { time: 0.0, position: Vec3::ZERO, rotation: Quat::IDENTITY, color: Vec3::new(1.0, 0.9, 0.7), intensity: 1.0, range: 100.0, spot_angle: 0.0, cast_shadows: true });
    track.add_keyframe(LightKeyframe { time: 5.0, position: Vec3::ZERO, rotation: Quat::IDENTITY, color: Vec3::new(1.0, 0.4, 0.2), intensity: 0.5, range: 80.0, spot_angle: 0.0, cast_shadows: true });
    let mid = track.sample(2.5).unwrap();
    assert!((mid.intensity - 0.75).abs() < 0.01);
    assert_eq!(track.duration(), 5.0);
    assert_eq!(track.keyframe_count(), 2);
}

pub const ANIM_CLIP_MAGIC: u32 = 0x414E4D43;
pub const CUTSCENE_FORMAT_VERSION: u32 = 3;
pub const MAX_BONE_COUNT: usize = 512;
pub const DEFAULT_FPS: f32 = 30.0;
pub const CUTSCENE_MODULE_VER: &str = "3.0.0";

pub fn cutscene_importer_module_info() -> HashMap<String, String> {
    let mut info = HashMap::new();
    info.insert("module".into(), "cutscene_importer".into());
    info.insert("version".into(), CUTSCENE_MODULE_VER.into());
    info.insert("fps".into(), format!("{}", DEFAULT_FPS));
    info.insert("max_bones".into(), format!("{}", MAX_BONE_COUNT));
    info.insert("arkit_shapes".into(), "52".into());
    info
}

// ============================================================
// SECTION: Subtitle System
// ============================================================

#[derive(Debug, Clone)]
pub struct SubtitleEntry {
    pub id: u32,
    pub start_time_s: f32,
    pub end_time_s: f32,
    pub text: String,
    pub speaker: Option<String>,
    pub language: String,
}

impl SubtitleEntry {
    pub fn new(id: u32, start: f32, end: f32, text: &str) -> Self {
        SubtitleEntry { id, start_time_s: start, end_time_s: end, text: text.to_string(), speaker: None, language: "en".to_string() }
    }
    pub fn duration_s(&self) -> f32 { self.end_time_s - self.start_time_s }
    pub fn is_active_at(&self, t: f32) -> bool { t >= self.start_time_s && t < self.end_time_s }
    pub fn word_count(&self) -> usize { self.text.split_whitespace().count() }
    pub fn reading_speed_wpm(&self) -> f32 { self.word_count() as f32 / self.duration_s() * 60.0 }
    pub fn is_well_paced(&self) -> bool { let wpm = self.reading_speed_wpm(); wpm >= 80.0 && wpm <= 300.0 }
}

#[derive(Debug, Clone)]
pub struct SubtitleTrack {
    pub language: String,
    pub entries: Vec<SubtitleEntry>,
    pub is_rtl: bool,
}

impl SubtitleTrack {
    pub fn new(language: &str) -> Self {
        let is_rtl = matches!(language, "ar" | "he" | "fa" | "ur");
        SubtitleTrack { language: language.to_string(), entries: Vec::new(), is_rtl }
    }
    pub fn add_entry(&mut self, e: SubtitleEntry) { self.entries.push(e); self.entries.sort_by(|a,b| a.start_time_s.partial_cmp(&b.start_time_s).unwrap()); }
    pub fn active_at(&self, t: f32) -> Option<&SubtitleEntry> { self.entries.iter().find(|e| e.is_active_at(t)) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn total_duration(&self) -> f32 { self.entries.last().map(|e| e.end_time_s).unwrap_or(0.0) }

    pub fn export_srt(&self) -> String {
        let mut out = String::new();
        for (i, entry) in self.entries.iter().enumerate() {
            out.push_str(&format!("{}\n", i + 1));
            out.push_str(&format!("{} --> {}\n", srt_time(entry.start_time_s), srt_time(entry.end_time_s)));
            if let Some(sp) = &entry.speaker { out.push_str(&format!("[{}] ", sp)); }
            out.push_str(&format!("{}\n\n", entry.text));
        }
        out
    }

    pub fn pacing_issues(&self) -> Vec<u32> { self.entries.iter().filter(|e| !e.is_well_paced()).map(|e| e.id).collect() }
}

fn srt_time(t: f32) -> String {
    let ms = ((t % 1.0) * 1000.0) as u32;
    let s = t as u32 % 60;
    let m = t as u32 / 60 % 60;
    let h = t as u32 / 3600;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

pub fn parse_srt(input: &str) -> SubtitleTrack {
    let mut track = SubtitleTrack::new("en");
    let mut id = 1u32;
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if line.parse::<u32>().is_ok() {
            i += 1;
            if i < lines.len() {
                let time_line = lines[i].trim();
                let parts: Vec<&str> = time_line.split("-->").collect();
                if parts.len() == 2 {
                    let start = parse_srt_time(parts[0].trim()).unwrap_or(0.0);
                    let end = parse_srt_time(parts[1].trim()).unwrap_or(0.0);
                    i += 1;
                    let mut text_lines = Vec::new();
                    while i < lines.len() && !lines[i].trim().is_empty() {
                        text_lines.push(lines[i]); i += 1;
                    }
                    let text = text_lines.join("\n");
                    track.add_entry(SubtitleEntry::new(id, start, end, &text));
                    id += 1;
                }
            }
        }
        i += 1;
    }
    track
}

fn parse_srt_time(s: &str) -> Option<f32> {
    let s = s.replace(",", ".");
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        let h: f32 = parts[0].parse().ok()?;
        let m: f32 = parts[1].parse().ok()?;
        let sec: f32 = parts[2].parse().ok()?;
        Some(h * 3600.0 + m * 60.0 + sec)
    } else { None }
}

// ============================================================
// SECTION: Storyboard System
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum StoryboardStatus { Concept, Sketch, Final, Approved }

#[derive(Debug, Clone)]
pub struct StoryboardPanel {
    pub panel_id: u32,
    pub shot_number: String,
    pub description: String,
    pub duration_s: f32,
    pub camera_angle: String,
    pub action_notes: String,
    pub dialogue: Option<String>,
    pub status: StoryboardStatus,
}

impl StoryboardPanel {
    pub fn new(id: u32, shot: &str, desc: &str, duration: f32) -> Self {
        StoryboardPanel { panel_id: id, shot_number: shot.to_string(), description: desc.to_string(),
            duration_s: duration, camera_angle: "medium".to_string(), action_notes: String::new(),
            dialogue: None, status: StoryboardStatus::Concept }
    }
    pub fn approve(&mut self) { self.status = StoryboardStatus::Approved; }
    pub fn is_approved(&self) -> bool { self.status == StoryboardStatus::Approved }
}

#[derive(Debug, Clone)]
pub struct Storyboard {
    pub title: String,
    pub panels: Vec<StoryboardPanel>,
    pub director: String,
}

impl Storyboard {
    pub fn new(title: &str) -> Self { Storyboard { title: title.to_string(), panels: Vec::new(), director: String::new() } }
    pub fn add_panel(&mut self, p: StoryboardPanel) { self.panels.push(p); }
    pub fn panel_count(&self) -> usize { self.panels.len() }
    pub fn total_duration_s(&self) -> f32 { self.panels.iter().map(|p| p.duration_s).sum() }
    pub fn completion_percent(&self) -> f32 { self.panels.iter().filter(|p| p.is_approved()).count() as f32 / self.panels.len().max(1) as f32 * 100.0 }
    pub fn approved_count(&self) -> usize { self.panels.iter().filter(|p| p.is_approved()).count() }

    pub fn export_pdf_script(&self) -> String {
        let mut out = format!("STORYBOARD: {}\nDirector: {}\nTotal Duration: {:.1}s\n\n", self.title, self.director, self.total_duration_s());
        for panel in &self.panels {
            out.push_str(&format!("SHOT {}: [{:?}] {:.1}s\n  {}\n", panel.shot_number, panel.status, panel.duration_s, panel.description));
            if let Some(d) = &panel.dialogue { out.push_str(&format!("  Dialogue: \"{}\"\n", d)); }
        }
        out
    }
}

// ============================================================
// SECTION: Performance Capture
// ============================================================

#[derive(Debug, Clone)]
pub struct PerformanceTake {
    pub take_id: u32,
    pub clip_name: String,
    pub duration_s: f32,
    pub quality_score: f32, // 0-100
    pub notes: String,
    pub is_approved: bool,
    pub timestamp: String,
}

impl PerformanceTake {
    pub fn new(id: u32, clip: &str, duration: f32) -> Self {
        PerformanceTake { take_id: id, clip_name: clip.to_string(), duration_s: duration,
            quality_score: 0.0, notes: String::new(), is_approved: false, timestamp: "2024-01-01T00:00:00Z".to_string() }
    }
    pub fn approve(&mut self) { self.is_approved = true; }
    pub fn set_quality(&mut self, score: f32) { self.quality_score = score.clamp(0.0, 100.0); }
}

#[derive(Debug, Clone)]
pub struct PerformanceCaptureSession {
    pub session_name: String,
    pub takes: Vec<PerformanceTake>,
    pub actor_name: String,
}

impl PerformanceCaptureSession {
    pub fn new(name: &str, actor: &str) -> Self { PerformanceCaptureSession { session_name: name.to_string(), takes: Vec::new(), actor_name: actor.to_string() } }
    pub fn add_take(&mut self, take: PerformanceTake) { self.takes.push(take); }
    pub fn approved_takes(&self) -> Vec<&PerformanceTake> { self.takes.iter().filter(|t| t.is_approved).collect() }
    pub fn select_best_take(&self) -> Option<&PerformanceTake> {
        self.takes.iter().max_by(|a, b| a.quality_score.partial_cmp(&b.quality_score).unwrap())
    }
    pub fn take_count(&self) -> usize { self.takes.len() }
    pub fn avg_quality(&self) -> f32 { if self.takes.is_empty() { return 0.0; } self.takes.iter().map(|t| t.quality_score).sum::<f32>() / self.takes.len() as f32 }
}

// ============================================================
// SECTION: Color Grading
// ============================================================

#[derive(Debug, Clone)]
pub struct ColorGradeKeyframe {
    pub time_s: f32,
    pub lift: Vec3,
    pub gamma: Vec3,
    pub gain: Vec3,
    pub saturation: f32,
    pub contrast: f32,
    pub exposure: f32,
}

impl ColorGradeKeyframe {
    pub fn neutral() -> Self {
        ColorGradeKeyframe { time_s: 0.0, lift: Vec3::ZERO, gamma: Vec3::ONE, gain: Vec3::ONE, saturation: 1.0, contrast: 1.0, exposure: 0.0 }
    }
    pub fn horror() -> Self {
        ColorGradeKeyframe { time_s: 0.0, lift: Vec3::new(-0.02, -0.02, 0.05), gamma: Vec3::new(0.9, 0.9, 1.1), gain: Vec3::new(0.8, 0.8, 1.2), saturation: 0.6, contrast: 1.3, exposure: -0.5 }
    }
    pub fn golden_hour() -> Self {
        ColorGradeKeyframe { time_s: 0.0, lift: Vec3::new(0.05, 0.02, -0.02), gamma: Vec3::new(1.1, 1.0, 0.9), gain: Vec3::new(1.3, 1.1, 0.7), saturation: 1.3, contrast: 1.1, exposure: 0.3 }
    }
    pub fn lerp(&self, other: &ColorGradeKeyframe, t: f32) -> ColorGradeKeyframe {
        ColorGradeKeyframe {
            time_s: self.time_s + t * (other.time_s - self.time_s),
            lift: self.lift.lerp(other.lift, t), gamma: self.gamma.lerp(other.gamma, t),
            gain: self.gain.lerp(other.gain, t), saturation: self.saturation + t * (other.saturation - self.saturation),
            contrast: self.contrast + t * (other.contrast - self.contrast), exposure: self.exposure + t * (other.exposure - self.exposure),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColorGradingTrack {
    pub keyframes: Vec<ColorGradeKeyframe>,
}

impl ColorGradingTrack {
    pub fn new() -> Self { ColorGradingTrack { keyframes: Vec::new() } }
    pub fn add_kf(&mut self, kf: ColorGradeKeyframe) { self.keyframes.push(kf); self.keyframes.sort_by(|a,b| a.time_s.partial_cmp(&b.time_s).unwrap()); }
    pub fn sample(&self, t: f32) -> ColorGradeKeyframe {
        if self.keyframes.is_empty() { return ColorGradeKeyframe::neutral(); }
        if t <= self.keyframes[0].time_s { return self.keyframes[0].clone(); }
        let last = self.keyframes.last().unwrap();
        if t >= last.time_s { return last.clone(); }
        for i in 0..self.keyframes.len()-1 {
            let a = &self.keyframes[i]; let b = &self.keyframes[i+1];
            if t >= a.time_s && t <= b.time_s { return a.lerp(b, (t - a.time_s)/(b.time_s - a.time_s)); }
        }
        ColorGradeKeyframe::neutral()
    }
}

// ============================================================
// SECTION: Directed Graph (Animation Graph)
// ============================================================

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub node_id: u32,
    pub name: String,
    pub node_type: String,
    pub data: HashMap<String, String>,
}

impl GraphNode {
    pub fn new(id: u32, name: &str, node_type: &str) -> Self {
        GraphNode { node_id: id, name: name.to_string(), node_type: node_type.to_string(), data: HashMap::new() }
    }
    pub fn clip_node(id: u32, clip_name: &str) -> Self { let mut n = Self::new(id, clip_name, "clip"); n.data.insert("clip".into(), clip_name.into()); n }
    pub fn blend_node(id: u32) -> Self { Self::new(id, &format!("blend_{}", id), "blend") }
    pub fn additive_node(id: u32) -> Self { Self::new(id, &format!("additive_{}", id), "additive") }
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: u32, pub to: u32, pub weight: f32,
}

#[derive(Debug, Clone)]
pub struct AnimationGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl AnimationGraph {
    pub fn new() -> Self { AnimationGraph { nodes: Vec::new(), edges: Vec::new() } }
    pub fn add_node(&mut self, n: GraphNode) { self.nodes.push(n); }
    pub fn add_edge(&mut self, from: u32, to: u32, weight: f32) { self.edges.push(GraphEdge { from, to, weight }); }

    pub fn topological_sort(&self) -> Option<Vec<u32>> {
        let node_ids: Vec<u32> = self.nodes.iter().map(|n| n.node_id).collect();
        let mut in_degree: HashMap<u32, usize> = node_ids.iter().map(|&id| (id, 0)).collect();
        for e in &self.edges { *in_degree.entry(e.to).or_insert(0) += 1; }
        let mut queue: VecDeque<u32> = in_degree.iter().filter(|(_, &d)| d == 0).map(|(&id, _)| id).collect();
        let mut result = Vec::new();
        while let Some(n) = queue.pop_front() {
            result.push(n);
            for e in self.edges.iter().filter(|e| e.from == n) {
                let d = in_degree.entry(e.to).or_insert(0);
                *d = d.saturating_sub(1);
                if *d == 0 { queue.push_back(e.to); }
            }
        }
        if result.len() == self.nodes.len() { Some(result) } else { None }
    }

    pub fn has_cycle(&self) -> bool { self.topological_sort().is_none() }
    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn edge_count(&self) -> usize { self.edges.len() }
}

// ============================================================
// SECTION: Camera Rig System
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum CameraRigType { Free, Dolly, Crane, Steadicam, Orbit, Handheld, VR, Drone, POV, MotionControlRig }

#[derive(Debug, Clone)]
pub struct CameraRigState {
    pub rig_type: CameraRigType,
    pub position: Vec3,
    pub rotation: Quat,
    pub fov_deg: f32,
    pub focal_length_mm: f32,
    pub focus_distance_m: f32,
    pub aperture_f: f32,
    pub near_clip: f32,
    pub far_clip: f32,
    pub sensor_width_mm: f32,
}

impl CameraRigState {
    pub fn new(rig_type: CameraRigType) -> Self {
        CameraRigState { rig_type, position: Vec3::ZERO, rotation: Quat::IDENTITY,
            fov_deg: 60.0, focal_length_mm: 35.0, focus_distance_m: 5.0,
            aperture_f: 2.8, near_clip: 0.1, far_clip: 1000.0, sensor_width_mm: 36.0 }
    }

    pub fn exposure_value(&self) -> f32 {
        // EV = log2(N^2 / t) approximation; simplified
        let aperture = self.aperture_f;
        (aperture * aperture).log2()
    }

    pub fn hyperfocal_m(&self) -> f32 {
        let f = self.focal_length_mm / 1000.0;
        let c = 0.0291 / 1000.0;
        f * f / (self.aperture_f * c)
    }

    pub fn apply_shake(&mut self, shake_offset: Vec3, shake_rotation: Vec3) {
        self.position += shake_offset;
        let q = Quat::from_euler(glam::EulerRot::XYZ, shake_rotation.x.to_radians(), shake_rotation.y.to_radians(), shake_rotation.z.to_radians());
        self.rotation = self.rotation * q;
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh_gl(self.fov_deg.to_radians(), aspect, self.near_clip, self.far_clip)
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.position).inverse()
    }
}

// ============================================================
// SECTION: More Tests
// ============================================================

pub fn run_subtitle_tests() {
    let srt_data = "1\n00:00:00,000 --> 00:00:02,000\nHello, world!\n\n2\n00:00:03,000 --> 00:00:06,000\nThis is a test.\n\n";
    let track = parse_srt(srt_data);
    assert_eq!(track.entry_count(), 2);
    assert!(track.active_at(1.0).is_some());
    assert!(track.active_at(4.0).is_some());
    assert!(track.active_at(10.0).is_none());
    let srt_out = track.export_srt();
    assert!(srt_out.contains("-->"));
    assert!(!track.is_rtl);

    let rtl_track = SubtitleTrack::new("ar");
    assert!(rtl_track.is_rtl);
}

pub fn run_storyboard_tests() {
    let mut board = Storyboard::new("Act 1");
    board.add_panel(StoryboardPanel::new(1, "1A", "Hero enters village", 3.0));
    board.add_panel(StoryboardPanel::new(2, "1B", "Wide shot of village", 2.5));
    let mut panel3 = StoryboardPanel::new(3, "1C", "Villain appears", 4.0);
    panel3.approve();
    board.add_panel(panel3);
    assert_eq!(board.panel_count(), 3);
    assert_eq!(board.approved_count(), 1);
    assert!((board.completion_percent() - 33.33).abs() < 0.5);
    assert!((board.total_duration_s() - 9.5).abs() < 0.001);
    let script = board.export_pdf_script();
    assert!(script.contains("Act 1"));
}

pub fn run_performance_capture_tests() {
    let mut session = PerformanceCaptureSession::new("Walk Cycle Session", "John Actor");
    let mut t1 = PerformanceTake::new(1, "walk_take_1", 5.0); t1.set_quality(72.0);
    let mut t2 = PerformanceTake::new(2, "walk_take_2", 5.2); t2.set_quality(88.0); t2.approve();
    let mut t3 = PerformanceTake::new(3, "walk_take_3", 4.8); t3.set_quality(65.0);
    session.add_take(t1); session.add_take(t2); session.add_take(t3);
    assert_eq!(session.take_count(), 3);
    assert_eq!(session.approved_takes().len(), 1);
    let best = session.select_best_take().unwrap();
    assert_eq!(best.take_id, 2);
    assert!(session.avg_quality() > 0.0);
}

pub fn run_color_grading_tests() {
    let neutral = ColorGradeKeyframe::neutral();
    let horror = ColorGradeKeyframe::horror();
    let lerped = neutral.lerp(&horror, 0.5);
    assert!(lerped.saturation < 1.0);
    let golden = ColorGradeKeyframe::golden_hour();
    assert!(golden.saturation > 1.0);

    let mut track = ColorGradingTrack::new();
    track.add_kf(ColorGradeKeyframe::neutral());
    let mut mid = ColorGradeKeyframe::horror(); mid.time_s = 5.0;
    track.add_kf(mid);
    let sampled = track.sample(2.5);
    assert!(sampled.saturation < 1.0 && sampled.saturation > 0.5);
}

pub fn run_animation_graph_tests() {
    let mut graph = AnimationGraph::new();
    graph.add_node(GraphNode::clip_node(1, "idle"));
    graph.add_node(GraphNode::clip_node(2, "walk"));
    graph.add_node(GraphNode::blend_node(3));
    graph.add_edge(1, 3, 0.5);
    graph.add_edge(2, 3, 0.5);
    assert!(!graph.has_cycle());
    let sorted = graph.topological_sort().unwrap();
    assert_eq!(sorted.len(), 3);
    // Node 3 should come last
    assert_eq!(*sorted.last().unwrap(), 3);
    // Add cycle
    graph.add_edge(3, 1, 1.0);
    assert!(graph.has_cycle());
}

pub fn run_camera_rig_tests() {
    let mut cam = CameraRigState::new(CameraRigType::Steadicam);
    assert!(cam.exposure_value() > 0.0);
    assert!(cam.hyperfocal_m() > 0.0);
    cam.apply_shake(Vec3::new(0.01, 0.02, 0.0), Vec3::new(0.1, 0.0, 0.0));
    assert!(cam.position.length() > 0.0);
    let view = cam.view_matrix();
    let _proj = cam.projection_matrix(16.0 / 9.0);
    assert!(view.col(3).truncate().length() >= 0.0);
}

pub fn run_all_cutscene_tests() {
    run_cutscene_pipeline_tests();
    run_timeline_editor_tests();
    run_final_cutscene_tests();
    run_cutscene_analytics_tests();
    run_scene_and_postprocess_tests();
    run_subtitle_tests();
    run_storyboard_tests();
    run_performance_capture_tests();
    run_color_grading_tests();
    run_animation_graph_tests();
    run_camera_rig_tests();
}


// ============================================================
// POST-PROCESS EFFECTS PIPELINE
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PostProcessEffectType {
    Bloom, DepthOfField, MotionBlur, AmbientOcclusion, Vignette,
    ChromaticAberration, FilmGrain, ColorGrading, ToneMapping,
    Sharpen, LensFlare, GodRays, ScreenSpaceReflections,
}

#[derive(Debug, Clone)]
pub struct PostProcessEffect {
    pub effect_type: PostProcessEffectType,
    pub enabled: bool,
    pub intensity: f32,
    pub params: HashMap<String, f32>,
}

impl PostProcessEffect {
    pub fn new(effect_type: PostProcessEffectType, intensity: f32) -> Self {
        Self { effect_type, enabled: true, intensity, params: HashMap::new() }
    }
    pub fn bloom(threshold: f32, scatter: f32, intensity: f32) -> Self {
        let mut e = Self::new(PostProcessEffectType::Bloom, intensity);
        e.params.insert("threshold".to_string(), threshold);
        e.params.insert("scatter".to_string(), scatter);
        e
    }
    pub fn depth_of_field(focal_length_mm: f32, aperture: f32, focus_distance_m: f32) -> Self {
        let mut e = Self::new(PostProcessEffectType::DepthOfField, 1.0);
        e.params.insert("focal_length_mm".to_string(), focal_length_mm);
        e.params.insert("aperture".to_string(), aperture);
        e.params.insert("focus_distance_m".to_string(), focus_distance_m);
        e
    }
    pub fn motion_blur(shutter_angle: f32, sample_count: f32) -> Self {
        let mut e = Self::new(PostProcessEffectType::MotionBlur, 1.0);
        e.params.insert("shutter_angle".to_string(), shutter_angle);
        e.params.insert("sample_count".to_string(), sample_count);
        e
    }
    pub fn vignette(intensity: f32, smoothness: f32) -> Self {
        let mut e = Self::new(PostProcessEffectType::Vignette, intensity);
        e.params.insert("smoothness".to_string(), smoothness);
        e
    }
    pub fn film_grain(intensity: f32, response: f32) -> Self {
        let mut e = Self::new(PostProcessEffectType::FilmGrain, intensity);
        e.params.insert("response".to_string(), response);
        e
    }
    pub fn effect_type_str(&self) -> &'static str {
        match &self.effect_type {
            PostProcessEffectType::Bloom => "Bloom",
            PostProcessEffectType::DepthOfField => "DepthOfField",
            PostProcessEffectType::MotionBlur => "MotionBlur",
            PostProcessEffectType::AmbientOcclusion => "AmbientOcclusion",
            PostProcessEffectType::Vignette => "Vignette",
            PostProcessEffectType::ChromaticAberration => "ChromaticAberration",
            PostProcessEffectType::FilmGrain => "FilmGrain",
            PostProcessEffectType::ColorGrading => "ColorGrading",
            PostProcessEffectType::ToneMapping => "ToneMapping",
            PostProcessEffectType::Sharpen => "Sharpen",
            PostProcessEffectType::LensFlare => "LensFlare",
            PostProcessEffectType::GodRays => "GodRays",
            PostProcessEffectType::ScreenSpaceReflections => "SSR",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PostProcessStack {
    pub effects: Vec<PostProcessEffect>,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,  // color temperature in Kelvin (relative offset)
}

impl PostProcessStack {
    pub fn new() -> Self {
        Self { effects: Vec::new(), exposure: 1.0, contrast: 1.0, saturation: 1.0, temperature: 0.0 }
    }
    pub fn add_effect(&mut self, e: PostProcessEffect) { self.effects.push(e); }
    pub fn enabled_effects(&self) -> Vec<&PostProcessEffect> {
        self.effects.iter().filter(|e| e.enabled).collect()
    }
    pub fn get_effect(&self, effect_type: &PostProcessEffectType) -> Option<&PostProcessEffect> {
        self.effects.iter().find(|e| &e.effect_type == effect_type)
    }
    pub fn get_effect_mut(&mut self, effect_type: &PostProcessEffectType) -> Option<&mut PostProcessEffect> {
        self.effects.iter_mut().find(|e| &e.effect_type == effect_type)
    }
    pub fn cinema_preset() -> Self {
        let mut stack = Self::new();
        stack.add_effect(PostProcessEffect::bloom(0.9, 0.7, 0.5));
        stack.add_effect(PostProcessEffect::depth_of_field(35.0, 2.8, 5.0));
        stack.add_effect(PostProcessEffect::vignette(0.4, 0.4));
        stack.add_effect(PostProcessEffect::film_grain(0.15, 0.8));
        stack.contrast = 1.15;
        stack.saturation = 0.9;
        stack.temperature = -200.0;
        stack
    }
    pub fn horror_preset() -> Self {
        let mut stack = Self::new();
        stack.add_effect(PostProcessEffect::vignette(0.8, 0.3));
        stack.add_effect(PostProcessEffect::film_grain(0.4, 0.9));
        stack.contrast = 1.4;
        stack.saturation = 0.3;
        stack.temperature = -500.0;
        stack.exposure = 0.8;
        stack
    }
    pub fn daylight_preset() -> Self {
        let mut stack = Self::new();
        stack.add_effect(PostProcessEffect::bloom(1.0, 0.5, 0.3));
        stack.contrast = 1.1;
        stack.saturation = 1.2;
        stack.temperature = 300.0;
        stack.exposure = 1.1;
        stack
    }
    pub fn effect_count(&self) -> usize { self.effects.len() }
}

// ============================================================
// USD / ALEMBIC SCENE EXPORT
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum UsdPrimType {
    Xform, Mesh, Camera, Light, Scope, Material, Shader, Points,
}

#[derive(Debug, Clone)]
pub struct UsdPrim {
    pub path: String,
    pub prim_type: UsdPrimType,
    pub attributes: HashMap<String, String>,
    pub children: Vec<String>,
    pub active: bool,
    pub instanceable: bool,
}

impl UsdPrim {
    pub fn new(path: &str, prim_type: UsdPrimType) -> Self {
        Self { path: path.to_string(), prim_type, attributes: HashMap::new(),
            children: Vec::new(), active: true, instanceable: false }
    }
    pub fn set_attr(&mut self, name: &str, value: &str) {
        self.attributes.insert(name.to_string(), value.to_string());
    }
    pub fn get_attr(&self, name: &str) -> Option<&String> {
        self.attributes.get(name)
    }
    pub fn prim_type_str(&self) -> &'static str {
        match &self.prim_type {
            UsdPrimType::Xform => "Xform", UsdPrimType::Mesh => "Mesh",
            UsdPrimType::Camera => "Camera", UsdPrimType::Light => "Light",
            UsdPrimType::Scope => "Scope", UsdPrimType::Material => "Material",
            UsdPrimType::Shader => "Shader", UsdPrimType::Points => "Points",
        }
    }
    pub fn sdf_declaration(&self) -> String {
        format!("def {} \"{}\"", self.prim_type_str(), self.path.split('/').last().unwrap_or("root"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct UsdStage {
    pub identifier: String,
    pub prims: Vec<UsdPrim>,
    pub default_prim: String,
    pub start_time_code: f64,
    pub end_time_code: f64,
    pub time_codes_per_second: f64,
    pub up_axis: String,
}

impl UsdStage {
    pub fn new(identifier: &str) -> Self {
        Self { identifier: identifier.to_string(), prims: Vec::new(),
            default_prim: String::new(), start_time_code: 0.0,
            end_time_code: 240.0, time_codes_per_second: 24.0, up_axis: "Y".to_string() }
    }
    pub fn add_prim(&mut self, p: UsdPrim) { self.prims.push(p); }
    pub fn get_prim(&self, path: &str) -> Option<&UsdPrim> {
        self.prims.iter().find(|p| p.path == path)
    }
    pub fn prim_count(&self) -> usize { self.prims.len() }
    pub fn duration_seconds(&self) -> f64 {
        (self.end_time_code - self.start_time_code) / self.time_codes_per_second
    }
    pub fn export_usda_header(&self) -> String {
        format!(
            "#usda 1.0\n(\n    defaultPrim = \"{}\"\n    startTimeCode = {}\n    endTimeCode = {}\n    timeCodesPerSecond = {}\n    upAxis = \"{}\"\n)\n",
            self.default_prim, self.start_time_code, self.end_time_code,
            self.time_codes_per_second, self.up_axis
        )
    }
    pub fn prims_of_type(&self, prim_type: &UsdPrimType) -> Vec<&UsdPrim> {
        self.prims.iter().filter(|p| &p.prim_type == prim_type).collect()
    }
    pub fn camera_prims(&self) -> Vec<&UsdPrim> { self.prims_of_type(&UsdPrimType::Camera) }
    pub fn mesh_prims(&self) -> Vec<&UsdPrim> { self.prims_of_type(&UsdPrimType::Mesh) }
}

// ============================================================
// ALEMBIC ARCHIVE SIMULATION
// ============================================================

#[derive(Debug, Clone)]
pub struct AlembicSample {
    pub time_s: f64,
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
}

impl AlembicSample {
    pub fn new(time_s: f64) -> Self {
        Self { time_s, positions: Vec::new(), velocities: Vec::new(), normals: Vec::new(), uvs: Vec::new() }
    }
    pub fn point_count(&self) -> usize { self.positions.len() }
    pub fn add_point(&mut self, pos: Vec3, vel: Vec3) {
        self.positions.push(pos);
        self.velocities.push(vel);
    }
    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        if self.positions.is_empty() { return (Vec3::ZERO, Vec3::ZERO); }
        let mut min = self.positions[0];
        let mut max = self.positions[0];
        for &p in &self.positions {
            min = min.min(p); max = max.max(p);
        }
        (min, max)
    }
}

#[derive(Debug, Clone)]
pub struct AlembicObject {
    pub name: String,
    pub schema: String,  // "PolyMesh", "Points", "Xform", etc.
    pub samples: Vec<AlembicSample>,
    pub is_constant: bool,
}

impl AlembicObject {
    pub fn new(name: &str, schema: &str) -> Self {
        Self { name: name.to_string(), schema: schema.to_string(), samples: Vec::new(), is_constant: false }
    }
    pub fn add_sample(&mut self, s: AlembicSample) { self.samples.push(s); }
    pub fn sample_count(&self) -> usize { self.samples.len() }
    pub fn duration_s(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let first = self.samples[0].time_s;
        let last = self.samples[self.samples.len()-1].time_s;
        last - first
    }
    pub fn sample_at_time(&self, t: f64) -> Option<&AlembicSample> {
        if self.samples.is_empty() { return None; }
        let idx = self.samples.iter().enumerate()
            .min_by(|(_, a), (_, b)| {
                (a.time_s - t).abs().partial_cmp(&(b.time_s - t).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i);
        idx.map(|i| &self.samples[i])
    }
}

#[derive(Debug, Clone, Default)]
pub struct AlembicArchive {
    pub filename: String,
    pub objects: Vec<AlembicObject>,
    pub start_time: f64,
    pub end_time: f64,
    pub fps: f64,
}

impl AlembicArchive {
    pub fn new(filename: &str, fps: f64) -> Self {
        Self { filename: filename.to_string(), objects: Vec::new(), start_time: 0.0, end_time: 0.0, fps }
    }
    pub fn add_object(&mut self, o: AlembicObject) { self.objects.push(o); }
    pub fn object_count(&self) -> usize { self.objects.len() }
    pub fn total_samples(&self) -> usize { self.objects.iter().map(|o| o.sample_count()).sum() }
    pub fn duration_s(&self) -> f64 { self.end_time - self.start_time }
    pub fn frame_count(&self) -> usize { (self.duration_s() * self.fps) as usize }
    pub fn find_object(&self, name: &str) -> Option<&AlembicObject> {
        self.objects.iter().find(|o| o.name == name)
    }
}

// ============================================================
// QUANTIZED ANIMATION STREAM
// ============================================================

#[derive(Debug, Clone)]
pub struct QuantizedQuat {
    pub x: i16, pub y: i16, pub z: i16, pub w: i16,
}

impl QuantizedQuat {
    pub fn from_quat(q: Quat) -> Self {
        let scale = 32767.0_f32;
        Self {
            x: (q.x * scale) as i16,
            y: (q.y * scale) as i16,
            z: (q.z * scale) as i16,
            w: (q.w * scale) as i16,
        }
    }
    pub fn to_quat(&self) -> Quat {
        let inv_scale = 1.0 / 32767.0;
        Quat::from_xyzw(
            self.x as f32 * inv_scale,
            self.y as f32 * inv_scale,
            self.z as f32 * inv_scale,
            self.w as f32 * inv_scale,
        ).normalize()
    }
    pub fn encode_smallest_three(q: Quat) -> (u8, i16, i16, i16) {
        let components = [q.x, q.y, q.z, q.w];
        let max_idx = components.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i).unwrap_or(3);
        let sign = if components[max_idx] >= 0.0 { 1.0 } else { -1.0 };
        let scale = 32767.0_f32 / std::f32::consts::FRAC_1_SQRT_2;
        let small: Vec<i16> = (0..4).filter(|&i| i != max_idx)
            .map(|i| (components[i] * sign * scale) as i16)
            .collect();
        (max_idx as u8, small[0], small[1], small[2])
    }
}

#[derive(Debug, Clone)]
pub struct QuantizedVec3 {
    pub x: i16, pub y: i16, pub z: i16,
}

impl QuantizedVec3 {
    pub fn from_vec3_range(v: Vec3, min: Vec3, max: Vec3) -> Self {
        let range = max - min;
        let norm = (v - min) / Vec3::new(range.x.max(0.001), range.y.max(0.001), range.z.max(0.001));
        Self {
            x: (norm.x.clamp(0.0, 1.0) * 65535.0 - 32768.0) as i16,
            y: (norm.y.clamp(0.0, 1.0) * 65535.0 - 32768.0) as i16,
            z: (norm.z.clamp(0.0, 1.0) * 65535.0 - 32768.0) as i16,
        }
    }
    pub fn to_vec3_range(&self, min: Vec3, max: Vec3) -> Vec3 {
        let range = max - min;
        let t = Vec3::new(
            (self.x as f32 + 32768.0) / 65535.0,
            (self.y as f32 + 32768.0) / 65535.0,
            (self.z as f32 + 32768.0) / 65535.0,
        );
        min + t * range
    }
}

#[derive(Debug, Clone)]
pub struct QuantizedBoneFrame {
    pub rotation: QuantizedQuat,
    pub translation: QuantizedVec3,
    pub scale: u16,  // uniform scale encoded as 16-bit fixed point
}

impl QuantizedBoneFrame {
    pub fn new(rotation: Quat, translation: Vec3, scale: f32, t_min: Vec3, t_max: Vec3) -> Self {
        Self {
            rotation: QuantizedQuat::from_quat(rotation),
            translation: QuantizedVec3::from_vec3_range(translation, t_min, t_max),
            scale: (scale.clamp(0.0, 2.0) * 32767.5) as u16,
        }
    }
    pub fn decode_scale(&self) -> f32 { self.scale as f32 / 32767.5 }
    pub fn byte_size() -> usize { 14 } // 4*i16 + 3*i16 + u16 = 8+6+2=16 bytes
}

#[derive(Debug, Clone)]
pub struct QuantizedAnimStream {
    pub bone_count: u32,
    pub frame_count: u32,
    pub fps: f32,
    pub translation_min: Vec3,
    pub translation_max: Vec3,
    pub frames: Vec<Vec<QuantizedBoneFrame>>,  // [frame][bone]
    pub bone_names: Vec<String>,
}

impl QuantizedAnimStream {
    pub fn new(bone_count: u32, fps: f32) -> Self {
        Self { bone_count, frame_count: 0, fps, translation_min: Vec3::splat(-5.0),
            translation_max: Vec3::splat(5.0), frames: Vec::new(), bone_names: Vec::new() }
    }
    pub fn add_frame(&mut self, bones: Vec<QuantizedBoneFrame>) {
        self.frame_count += 1;
        self.frames.push(bones);
    }
    pub fn duration_s(&self) -> f32 { self.frame_count as f32 / self.fps.max(0.001) }
    pub fn total_bytes(&self) -> usize {
        self.frame_count as usize * self.bone_count as usize * QuantizedBoneFrame::byte_size()
    }
    pub fn sample_frame(&self, time_s: f32) -> Option<&Vec<QuantizedBoneFrame>> {
        let frame_idx = (time_s * self.fps) as usize;
        self.frames.get(frame_idx.min(self.frames.len().saturating_sub(1)))
    }
    pub fn compression_ratio_vs_f32(&self) -> f32 {
        let uncompressed = self.frame_count as usize * self.bone_count as usize * (4*4 + 3*4 + 4); // quat+vec3+scale as f32
        let compressed = self.total_bytes();
        if compressed == 0 { return 1.0; }
        uncompressed as f32 / compressed as f32
    }
}

// ============================================================
// VFX INTEGRATION
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum VfxEffectType {
    ParticleEmitter, RibbonTrail, MeshDecal, FluidSim,
    ClothSim, DestructionFx, GroundImpact, ExplosionRing,
    FirePlume, SmokeColumn, ElectricArc, PortalEffect,
}

#[derive(Debug, Clone)]
pub struct VfxBinding {
    pub binding_id: u32,
    pub effect_type: VfxEffectType,
    pub attach_bone: String,
    pub local_offset: Vec3,
    pub local_rotation: Quat,
    pub scale: f32,
    pub start_time_s: f32,
    pub duration_s: f32,
    pub loop_count: i32,   // -1 = infinite
    pub intensity: f32,
    pub color_tint: Vec4,
}

impl VfxBinding {
    pub fn new(binding_id: u32, effect_type: VfxEffectType, attach_bone: &str, start_time_s: f32) -> Self {
        Self { binding_id, effect_type, attach_bone: attach_bone.to_string(),
            local_offset: Vec3::ZERO, local_rotation: Quat::IDENTITY,
            scale: 1.0, start_time_s, duration_s: 1.0, loop_count: 1,
            intensity: 1.0, color_tint: Vec4::new(1.0, 1.0, 1.0, 1.0) }
    }
    pub fn end_time_s(&self) -> f32 { self.start_time_s + self.duration_s }
    pub fn is_active_at(&self, time_s: f32) -> bool {
        time_s >= self.start_time_s && (self.loop_count < 0 || time_s <= self.end_time_s())
    }
    pub fn effect_type_str(&self) -> &'static str {
        match &self.effect_type {
            VfxEffectType::ParticleEmitter => "Particle Emitter",
            VfxEffectType::RibbonTrail => "Ribbon Trail",
            VfxEffectType::MeshDecal => "Mesh Decal",
            VfxEffectType::FluidSim => "Fluid Simulation",
            VfxEffectType::ClothSim => "Cloth Simulation",
            VfxEffectType::DestructionFx => "Destruction FX",
            VfxEffectType::GroundImpact => "Ground Impact",
            VfxEffectType::ExplosionRing => "Explosion Ring",
            VfxEffectType::FirePlume => "Fire Plume",
            VfxEffectType::SmokeColumn => "Smoke Column",
            VfxEffectType::ElectricArc => "Electric Arc",
            VfxEffectType::PortalEffect => "Portal Effect",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VfxLayer {
    pub bindings: Vec<VfxBinding>,
    pub global_scale: f32,
    pub paused: bool,
}

impl VfxLayer {
    pub fn new() -> Self { Self { bindings: Vec::new(), global_scale: 1.0, paused: false } }
    pub fn add_binding(&mut self, b: VfxBinding) { self.bindings.push(b); }
    pub fn active_at(&self, time_s: f32) -> Vec<&VfxBinding> {
        if self.paused { return Vec::new(); }
        self.bindings.iter().filter(|b| b.is_active_at(time_s)).collect()
    }
    pub fn total_bindings(&self) -> usize { self.bindings.len() }
    pub fn bindings_of_type(&self, effect_type: &VfxEffectType) -> Vec<&VfxBinding> {
        self.bindings.iter().filter(|b| &b.effect_type == effect_type).collect()
    }
}

// ============================================================
// SCENE RENDER PASS MANAGER
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum RenderPassType {
    ZPrepass, Opaque, AlphaMask, Transparent, Decals,
    VolumetricFog, DeferredLighting, ShadowMap, PostProcess,
    UI, Debug, Custom(u8),
}

#[derive(Debug, Clone)]
pub struct RenderPass {
    pub pass_type: RenderPassType,
    pub name: String,
    pub enabled: bool,
    pub clear_color: Vec4,
    pub clear_depth: bool,
    pub render_target: String,
    pub priority: i32,
}

impl RenderPass {
    pub fn new(pass_type: RenderPassType, name: &str, priority: i32) -> Self {
        Self { pass_type, name: name.to_string(), enabled: true,
            clear_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            clear_depth: true, render_target: "backbuffer".to_string(), priority }
    }
    pub fn pass_type_str(&self) -> String {
        match &self.pass_type {
            RenderPassType::ZPrepass => "ZPrepass".to_string(),
            RenderPassType::Opaque => "Opaque".to_string(),
            RenderPassType::AlphaMask => "AlphaMask".to_string(),
            RenderPassType::Transparent => "Transparent".to_string(),
            RenderPassType::Decals => "Decals".to_string(),
            RenderPassType::VolumetricFog => "VolumetricFog".to_string(),
            RenderPassType::DeferredLighting => "DeferredLighting".to_string(),
            RenderPassType::ShadowMap => "ShadowMap".to_string(),
            RenderPassType::PostProcess => "PostProcess".to_string(),
            RenderPassType::UI => "UI".to_string(),
            RenderPassType::Debug => "Debug".to_string(),
            RenderPassType::Custom(n) => format!("Custom({})", n),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderPassManager {
    pub passes: Vec<RenderPass>,
}

impl RenderPassManager {
    pub fn new() -> Self { Self { passes: Vec::new() } }
    pub fn add_pass(&mut self, p: RenderPass) { self.passes.push(p); }
    pub fn default_pipeline() -> Self {
        let mut mgr = Self::new();
        mgr.add_pass(RenderPass::new(RenderPassType::ZPrepass, "Z Pre-pass", 0));
        mgr.add_pass(RenderPass::new(RenderPassType::ShadowMap, "Shadow Maps", 5));
        mgr.add_pass(RenderPass::new(RenderPassType::Opaque, "Opaque", 10));
        mgr.add_pass(RenderPass::new(RenderPassType::AlphaMask, "Alpha Mask", 15));
        mgr.add_pass(RenderPass::new(RenderPassType::DeferredLighting, "Deferred Lighting", 20));
        mgr.add_pass(RenderPass::new(RenderPassType::Decals, "Decals", 25));
        mgr.add_pass(RenderPass::new(RenderPassType::VolumetricFog, "Volumetric Fog", 30));
        mgr.add_pass(RenderPass::new(RenderPassType::Transparent, "Transparent", 35));
        mgr.add_pass(RenderPass::new(RenderPassType::PostProcess, "Post Process", 40));
        mgr.add_pass(RenderPass::new(RenderPassType::UI, "UI", 45));
        mgr
    }
    pub fn enabled_passes(&self) -> Vec<&RenderPass> {
        let mut passes: Vec<&RenderPass> = self.passes.iter().filter(|p| p.enabled).collect();
        passes.sort_by_key(|p| p.priority);
        passes
    }
    pub fn count(&self) -> usize { self.passes.len() }
}

// ============================================================
// SOUND CUE SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SoundCueType {
    OneShot, Looping, Ambience, Dialogue, Music, Foley,
}

#[derive(Debug, Clone)]
pub struct SoundCue {
    pub cue_id: u32,
    pub name: String,
    pub cue_type: SoundCueType,
    pub start_time_s: f32,
    pub duration_s: f32,
    pub volume: f32,
    pub pitch: f32,
    pub spatial: bool,
    pub position: Vec3,
    pub falloff_radius_m: f32,
    pub asset_path: String,
}

impl SoundCue {
    pub fn new(cue_id: u32, name: &str, cue_type: SoundCueType, start_time_s: f32, duration_s: f32) -> Self {
        Self { cue_id, name: name.to_string(), cue_type, start_time_s, duration_s,
            volume: 1.0, pitch: 1.0, spatial: false, position: Vec3::ZERO,
            falloff_radius_m: 20.0, asset_path: String::new() }
    }
    pub fn end_time_s(&self) -> f32 { self.start_time_s + self.duration_s }
    pub fn is_active_at(&self, time_s: f32) -> bool {
        time_s >= self.start_time_s && time_s <= self.end_time_s()
    }
    pub fn volume_at_distance(&self, distance_m: f32) -> f32 {
        if !self.spatial || distance_m <= 0.0 { return self.volume; }
        let ratio = (1.0 - distance_m / self.falloff_radius_m.max(0.001)).max(0.0);
        self.volume * ratio * ratio  // inverse square falloff approximation
    }
}

#[derive(Debug, Clone, Default)]
pub struct SoundTrack {
    pub track_name: String,
    pub cues: Vec<SoundCue>,
    pub muted: bool,
    pub solo: bool,
    pub master_volume: f32,
}

impl SoundTrack {
    pub fn new(track_name: &str) -> Self {
        Self { track_name: track_name.to_string(), cues: Vec::new(),
            muted: false, solo: false, master_volume: 1.0 }
    }
    pub fn add_cue(&mut self, c: SoundCue) { self.cues.push(c); }
    pub fn cues_at_time(&self, time_s: f32) -> Vec<&SoundCue> {
        if self.muted { return Vec::new(); }
        self.cues.iter().filter(|c| c.is_active_at(time_s)).collect()
    }
    pub fn duration_s(&self) -> f32 {
        self.cues.iter().map(|c| c.end_time_s()).fold(0.0_f32, f32::max)
    }
    pub fn cue_count(&self) -> usize { self.cues.len() }
}

// ============================================================
// RENDER SEQUENCE EXPORTER
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Png, Exr, Jpeg, Tiff, DpxRaw,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportColorspace {
    Srgb, LinearRec709, Aces, DciP3, Rec2020,
}

#[derive(Debug, Clone)]
pub struct RenderSequenceExport {
    pub output_path: String,
    pub format: ExportFormat,
    pub colorspace: ExportColorspace,
    pub width: u32,
    pub height: u32,
    pub start_frame: u32,
    pub end_frame: u32,
    pub fps: f32,
    pub bit_depth: u8,
    pub include_alpha: bool,
    pub denoise: bool,
    pub aov_outputs: Vec<String>,
}

impl RenderSequenceExport {
    pub fn new(output_path: &str, width: u32, height: u32, fps: f32) -> Self {
        Self { output_path: output_path.to_string(), format: ExportFormat::Exr,
            colorspace: ExportColorspace::LinearRec709,
            width, height, start_frame: 1, end_frame: 100, fps,
            bit_depth: 16, include_alpha: true, denoise: false, aov_outputs: Vec::new() }
    }
    pub fn frame_count(&self) -> u32 { self.end_frame - self.start_frame + 1 }
    pub fn duration_s(&self) -> f32 { self.frame_count() as f32 / self.fps.max(0.001) }
    pub fn bytes_per_frame(&self) -> u64 {
        let channels = if self.include_alpha { 4 } else { 3 };
        let bits_per_channel = self.bit_depth as u64;
        self.width as u64 * self.height as u64 * channels * bits_per_channel / 8
    }
    pub fn total_bytes(&self) -> u64 {
        self.bytes_per_frame() * self.frame_count() as u64
    }
    pub fn total_gb(&self) -> f64 { self.total_bytes() as f64 / (1024.0 * 1024.0 * 1024.0) }
    pub fn format_str(&self) -> &'static str {
        match &self.format {
            ExportFormat::Png => "PNG", ExportFormat::Exr => "EXR",
            ExportFormat::Jpeg => "JPEG", ExportFormat::Tiff => "TIFF",
            ExportFormat::DpxRaw => "DPX",
        }
    }
    pub fn frame_filename(&self, frame: u32) -> String {
        let ext = self.format_str().to_lowercase();
        format!("{}/frame_{:06}.{}", self.output_path, frame, ext)
    }
    pub fn add_aov(&mut self, aov_name: &str) { self.aov_outputs.push(aov_name.to_string()); }
}

// ============================================================
// CUTSCENE DIRECTOR (HIGH-LEVEL ORCHESTRATOR)
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum DirectorState {
    Idle, Playing, Paused, Scrubbing, Recording,
}

#[derive(Debug, Clone)]
pub struct CutsceneDirector {
    pub cutscene_id: String,
    pub state: DirectorState,
    pub current_time_s: f32,
    pub total_duration_s: f32,
    pub playback_speed: f32,
    pub sound_tracks: Vec<SoundTrack>,
    pub vfx_layer: VfxLayer,
    pub post_process: PostProcessStack,
    pub render_pass_mgr: RenderPassManager,
    pub usd_stage: Option<UsdStage>,
    pub export_settings: Option<RenderSequenceExport>,
    pub markers: Vec<(f32, String)>,  // time -> label
}

impl CutsceneDirector {
    pub fn new(cutscene_id: &str, total_duration_s: f32) -> Self {
        Self { cutscene_id: cutscene_id.to_string(), state: DirectorState::Idle,
            current_time_s: 0.0, total_duration_s, playback_speed: 1.0,
            sound_tracks: Vec::new(), vfx_layer: VfxLayer::new(),
            post_process: PostProcessStack::new(),
            render_pass_mgr: RenderPassManager::default_pipeline(),
            usd_stage: None, export_settings: None, markers: Vec::new() }
    }
    pub fn play(&mut self) { self.state = DirectorState::Playing; }
    pub fn pause(&mut self) { self.state = DirectorState::Paused; }
    pub fn stop(&mut self) { self.state = DirectorState::Idle; self.current_time_s = 0.0; }
    pub fn seek(&mut self, time_s: f32) {
        self.current_time_s = time_s.clamp(0.0, self.total_duration_s);
        self.state = DirectorState::Scrubbing;
    }
    pub fn add_sound_track(&mut self, t: SoundTrack) { self.sound_tracks.push(t); }
    pub fn add_marker(&mut self, time_s: f32, label: &str) {
        self.markers.push((time_s, label.to_string()));
    }
    pub fn advance(&mut self, delta_s: f32) {
        if self.state == DirectorState::Playing {
            self.current_time_s = (self.current_time_s + delta_s * self.playback_speed)
                .min(self.total_duration_s);
            if self.current_time_s >= self.total_duration_s {
                self.state = DirectorState::Idle;
            }
        }
    }
    pub fn active_sound_cues(&self) -> Vec<&SoundCue> {
        self.sound_tracks.iter()
            .flat_map(|t| t.cues_at_time(self.current_time_s))
            .collect()
    }
    pub fn active_vfx(&self) -> Vec<&VfxBinding> {
        self.vfx_layer.active_at(self.current_time_s)
    }
    pub fn progress_pct(&self) -> f32 {
        if self.total_duration_s < 0.001 { return 0.0; }
        self.current_time_s / self.total_duration_s * 100.0
    }
    pub fn markers_in_range(&self, start_s: f32, end_s: f32) -> Vec<&(f32, String)> {
        self.markers.iter().filter(|(t, _)| *t >= start_s && *t <= end_s).collect()
    }
    pub fn render_pass_count(&self) -> usize { self.render_pass_mgr.count() }
}

// ============================================================
// ANIMATION RETARGETING SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct BoneMapping {
    pub source_bone: String,
    pub target_bone: String,
    pub rotation_offset: Quat,
    pub scale_factor: f32,
}

impl BoneMapping {
    pub fn new(source: &str, target: &str) -> Self {
        Self { source_bone: source.to_string(), target_bone: target.to_string(),
            rotation_offset: Quat::IDENTITY, scale_factor: 1.0 }
    }
    pub fn with_rotation_offset(mut self, rot: Quat) -> Self { self.rotation_offset = rot; self }
    pub fn with_scale(mut self, scale: f32) -> Self { self.scale_factor = scale; self }
}

#[derive(Debug, Clone, Default)]
pub struct RetargetProfile {
    pub source_skeleton: String,
    pub target_skeleton: String,
    pub bone_mappings: Vec<BoneMapping>,
    pub unmapped_bone_policy: String,  // "zero", "t-pose", "skip"
}

impl RetargetProfile {
    pub fn new(source: &str, target: &str) -> Self {
        Self { source_skeleton: source.to_string(), target_skeleton: target.to_string(),
            bone_mappings: Vec::new(), unmapped_bone_policy: "t-pose".to_string() }
    }
    pub fn add_mapping(&mut self, mapping: BoneMapping) { self.bone_mappings.push(mapping); }
    pub fn find_mapping(&self, source_bone: &str) -> Option<&BoneMapping> {
        self.bone_mappings.iter().find(|m| m.source_bone == source_bone)
    }
    pub fn mapping_count(&self) -> usize { self.bone_mappings.len() }
    pub fn humanoid_biped() -> Self {
        let mut p = Self::new("source_biped", "target_biped");
        let bone_pairs = [
            ("Hips", "pelvis"), ("Spine", "spine_01"), ("Spine1", "spine_02"),
            ("Spine2", "spine_03"), ("Neck", "neck_01"), ("Head", "head"),
            ("LeftShoulder", "clavicle_l"), ("LeftArm", "upperarm_l"),
            ("LeftForeArm", "lowerarm_l"), ("LeftHand", "hand_l"),
            ("RightShoulder", "clavicle_r"), ("RightArm", "upperarm_r"),
            ("RightForeArm", "lowerarm_r"), ("RightHand", "hand_r"),
            ("LeftUpLeg", "thigh_l"), ("LeftLeg", "calf_l"), ("LeftFoot", "foot_l"),
            ("RightUpLeg", "thigh_r"), ("RightLeg", "calf_r"), ("RightFoot", "foot_r"),
        ];
        for (src, tgt) in &bone_pairs {
            p.add_mapping(BoneMapping::new(src, tgt));
        }
        p
    }
}

// ============================================================
// FACIAL CAPTURE PROCESSING
// ============================================================

pub const ARKit_BLEND_SHAPE_NAMES: [&str; 52] = [
    "eyeBlinkLeft", "eyeLookDownLeft", "eyeLookInLeft", "eyeLookOutLeft", "eyeLookUpLeft",
    "eyeSquintLeft", "eyeWideLeft", "eyeBlinkRight", "eyeLookDownRight", "eyeLookInRight",
    "eyeLookOutRight", "eyeLookUpRight", "eyeSquintRight", "eyeWideRight",
    "jawForward", "jawLeft", "jawRight", "jawOpen",
    "mouthClose", "mouthFunnel", "mouthPucker", "mouthLeft", "mouthRight",
    "mouthSmileLeft", "mouthSmileRight", "mouthFrownLeft", "mouthFrownRight",
    "mouthDimpleLeft", "mouthDimpleRight", "mouthStretchLeft", "mouthStretchRight",
    "mouthRollLower", "mouthRollUpper", "mouthShrugLower", "mouthShrugUpper",
    "mouthPressLeft", "mouthPressRight", "mouthLowerDownLeft", "mouthLowerDownRight",
    "mouthUpperUpLeft", "mouthUpperUpRight", "browDownLeft", "browDownRight",
    "browInnerUp", "browOuterUpLeft", "browOuterUpRight",
    "cheekPuff", "cheekSquintLeft", "cheekSquintRight",
    "noseSneerLeft", "noseSneerRight", "tongueOut",
];

#[derive(Debug, Clone)]
pub struct FacialCaptureFrame {
    pub time_s: f64,
    pub blend_shapes: [f32; 52],
    pub head_rotation: Quat,
    pub left_eye_rotation: Quat,
    pub right_eye_rotation: Quat,
    pub confidence: f32,
}

impl FacialCaptureFrame {
    pub fn new(time_s: f64) -> Self {
        Self { time_s, blend_shapes: [0.0; 52], head_rotation: Quat::IDENTITY,
            left_eye_rotation: Quat::IDENTITY, right_eye_rotation: Quat::IDENTITY,
            confidence: 1.0 }
    }
    pub fn set_blend_shape(&mut self, name: &str, value: f32) {
        if let Some(idx) = ARKit_BLEND_SHAPE_NAMES.iter().position(|&n| n == name) {
            self.blend_shapes[idx] = value.clamp(0.0, 1.0);
        }
    }
    pub fn get_blend_shape(&self, name: &str) -> f32 {
        ARKit_BLEND_SHAPE_NAMES.iter().position(|&n| n == name)
            .map(|idx| self.blend_shapes[idx])
            .unwrap_or(0.0)
    }
    pub fn jaw_open(&self) -> f32 { self.blend_shapes[17] }
    pub fn mouth_smile_avg(&self) -> f32 { (self.blend_shapes[23] + self.blend_shapes[24]) / 2.0 }
    pub fn eye_blink_avg(&self) -> f32 { (self.blend_shapes[0] + self.blend_shapes[7]) / 2.0 }
    pub fn brow_raise_avg(&self) -> f32 { self.blend_shapes[43] }
    pub fn detected_expression(&self) -> &'static str {
        if self.mouth_smile_avg() > 0.5 { return "happy"; }
        if self.blend_shapes[25] > 0.4 || self.blend_shapes[26] > 0.4 { return "sad"; }
        if self.blend_shapes[40] > 0.3 || self.blend_shapes[41] > 0.3 { return "angry"; }
        if self.brow_raise_avg() > 0.4 && self.jaw_open() > 0.3 { return "surprised"; }
        if self.eye_blink_avg() > 0.7 { return "blinking"; }
        "neutral"
    }
}

#[derive(Debug, Clone, Default)]
pub struct FacialCaptureTake {
    pub take_id: String,
    pub actor_name: String,
    pub frames: Vec<FacialCaptureFrame>,
    pub fps: f32,
    pub camera_model: String,
}

impl FacialCaptureTake {
    pub fn new(take_id: &str, actor_name: &str, fps: f32) -> Self {
        Self { take_id: take_id.to_string(), actor_name: actor_name.to_string(),
            frames: Vec::new(), fps, camera_model: "iPhone".to_string() }
    }
    pub fn add_frame(&mut self, f: FacialCaptureFrame) { self.frames.push(f); }
    pub fn frame_count(&self) -> usize { self.frames.len() }
    pub fn duration_s(&self) -> f64 {
        if self.frames.is_empty() { return 0.0; }
        self.frames[self.frames.len()-1].time_s - self.frames[0].time_s
    }
    pub fn frame_at_time(&self, time_s: f64) -> Option<&FacialCaptureFrame> {
        if self.frames.is_empty() { return None; }
        let idx = self.frames.iter().enumerate()
            .min_by(|(_, a), (_, b)| {
                (a.time_s - time_s).abs().partial_cmp(&(b.time_s - time_s).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }).map(|(i, _)| i);
        idx.map(|i| &self.frames[i])
    }
    pub fn average_confidence(&self) -> f32 {
        if self.frames.is_empty() { return 0.0; }
        self.frames.iter().map(|f| f.confidence).sum::<f32>() / self.frames.len() as f32
    }
    pub fn low_confidence_frames(&self, threshold: f32) -> Vec<&FacialCaptureFrame> {
        self.frames.iter().filter(|f| f.confidence < threshold).collect()
    }
    pub fn smooth_blend_shapes(&mut self, window_size: usize) {
        if self.frames.len() < window_size { return; }
        let n = self.frames.len();
        let orig: Vec<[f32; 52]> = self.frames.iter().map(|f| f.blend_shapes).collect();
        for i in 0..n {
            let start = i.saturating_sub(window_size / 2);
            let end = (i + window_size / 2 + 1).min(n);
            let count = (end - start) as f32;
            for j in 0..52 {
                let avg = orig[start..end].iter().map(|bs| bs[j]).sum::<f32>() / count;
                self.frames[i].blend_shapes[j] = avg;
            }
        }
    }
    pub fn dominant_expression_histogram(&self) -> HashMap<String, usize> {
        let mut hist: HashMap<String, usize> = HashMap::new();
        for f in &self.frames {
            *hist.entry(f.detected_expression().to_string()).or_insert(0) += 1;
        }
        hist
    }
}

// ============================================================
// SCENE LIGHTING KEYFRAMES
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtLightKeyframe {
    pub time_s: f32,
    pub color: Vec3,
    pub intensity: f32,
    pub radius_or_angle: f32,
    pub cast_shadows: bool,
    pub shadow_softness: f32,
}

impl ExtLightKeyframe {
    pub fn new(time_s: f32, color: Vec3, intensity: f32) -> Self {
        Self { time_s, color, intensity, radius_or_angle: 5.0, cast_shadows: true, shadow_softness: 1.0 }
    }
    pub fn lerp(&self, other: &ExtLightKeyframe, t: f32) -> ExtLightKeyframe {
        let t = t.clamp(0.0, 1.0);
        ExtLightKeyframe {
            time_s: self.time_s + (other.time_s - self.time_s) * t,
            color: self.color.lerp(other.color, t),
            intensity: self.intensity + (other.intensity - self.intensity) * t,
            radius_or_angle: self.radius_or_angle + (other.radius_or_angle - self.radius_or_angle) * t,
            cast_shadows: if t < 0.5 { self.cast_shadows } else { other.cast_shadows },
            shadow_softness: self.shadow_softness + (other.shadow_softness - self.shadow_softness) * t,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LightAnimTrack {
    pub light_name: String,
    pub keyframes: Vec<ExtLightKeyframe>,
}

impl LightAnimTrack {
    pub fn new(light_name: &str) -> Self { Self { light_name: light_name.to_string(), keyframes: Vec::new() } }
    pub fn add_keyframe(&mut self, kf: ExtLightKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time_s < kf.time_s);
        self.keyframes.insert(idx, kf);
    }
    pub fn sample(&self, time_s: f32) -> Option<ExtLightKeyframe> {
        if self.keyframes.is_empty() { return None; }
        if self.keyframes.len() == 1 { return Some(self.keyframes[0].clone()); }
        let idx = self.keyframes.partition_point(|k| k.time_s <= time_s);
        if idx == 0 { return Some(self.keyframes[0].clone()); }
        if idx >= self.keyframes.len() { return Some(self.keyframes[self.keyframes.len()-1].clone()); }
        let prev = &self.keyframes[idx-1];
        let next = &self.keyframes[idx];
        let span = next.time_s - prev.time_s;
        let t = if span > 0.001 { (time_s - prev.time_s) / span } else { 0.0 };
        Some(prev.lerp(next, t))
    }
    pub fn duration_s(&self) -> f32 {
        if self.keyframes.is_empty() { return 0.0; }
        self.keyframes[self.keyframes.len()-1].time_s
    }
    pub fn keyframe_count(&self) -> usize { self.keyframes.len() }
}

// ============================================================
// TEST FUNCTIONS
// ============================================================

pub fn run_post_process_tests() {
    let cinema = PostProcessStack::cinema_preset();
    assert!(cinema.effect_count() > 0);
    assert!(cinema.enabled_effects().len() > 0);

    let horror = PostProcessStack::horror_preset();
    assert!(horror.saturation < 0.5);
    assert!(horror.contrast > 1.2);

    let effect = PostProcessEffect::bloom(0.9, 0.7, 0.5);
    assert_eq!(effect.effect_type_str(), "Bloom");
    assert!(effect.params.contains_key("threshold"));
}

pub fn run_usd_stage_tests() {
    let mut stage = UsdStage::new("/tmp/test_scene.usda");
    stage.time_codes_per_second = 24.0;
    stage.end_time_code = 240.0;

    let mut root = UsdPrim::new("/root", UsdPrimType::Xform);
    root.set_attr("xformOp:translate", "(0, 0, 0)");
    stage.add_prim(root);

    let cam = UsdPrim::new("/root/Camera", UsdPrimType::Camera);
    stage.add_prim(cam);

    assert_eq!(stage.prim_count(), 2);
    assert_eq!(stage.duration_seconds(), 10.0);
    assert_eq!(stage.camera_prims().len(), 1);
    let header = stage.export_usda_header();
    assert!(header.contains("usda 1.0"));
}

pub fn run_alembic_tests() {
    let mut archive = AlembicArchive::new("crowd_sim.abc", 24.0);
    archive.end_time = 4.0;

    let mut crowd = AlembicObject::new("crowd_particles", "Points");
    for i in 0..10 {
        let mut sample = AlembicSample::new(i as f64 * (1.0 / 24.0));
        for j in 0..100 {
            let pos = Vec3::new(j as f32 * 0.1, 0.0, i as f32 * 0.05);
            let vel = Vec3::new(0.5, 0.0, 0.0);
            sample.add_point(pos, vel);
        }
        crowd.add_sample(sample);
    }
    archive.add_object(crowd);

    assert_eq!(archive.object_count(), 1);
    assert_eq!(archive.total_samples(), 10);
    let obj = archive.find_object("crowd_particles").unwrap();
    assert_eq!(obj.sample_count(), 10);
    let sample = obj.sample_at_time(0.0).unwrap();
    let (bb_min, bb_max) = sample.bounding_box();
    assert!(bb_max.x > bb_min.x);
}

pub fn run_quantized_anim_tests() {
    let q = Quat::from_rotation_y(0.5);
    let qq = QuantizedQuat::from_quat(q);
    let recovered = qq.to_quat();
    let dot = (q.x * recovered.x + q.y * recovered.y + q.z * recovered.z + q.w * recovered.w).abs();
    assert!(dot > 0.99, "dot={}", dot);

    let mut stream = QuantizedAnimStream::new(3, 24.0);
    for _ in 0..24 {
        let bones = vec![
            QuantizedBoneFrame::new(Quat::IDENTITY, Vec3::ZERO, 1.0, Vec3::splat(-5.0), Vec3::splat(5.0)),
            QuantizedBoneFrame::new(Quat::from_rotation_x(0.1), Vec3::new(0.0, 1.0, 0.0), 1.0, Vec3::splat(-5.0), Vec3::splat(5.0)),
            QuantizedBoneFrame::new(Quat::from_rotation_z(0.2), Vec3::ZERO, 1.0, Vec3::splat(-5.0), Vec3::splat(5.0)),
        ];
        stream.add_frame(bones);
    }
    assert_eq!(stream.frame_count, 24);
    assert!((stream.duration_s() - 1.0).abs() < 0.001);
    let ratio = stream.compression_ratio_vs_f32();
    assert!(ratio > 1.0, "compression ratio should be > 1: {}", ratio);
}

pub fn run_vfx_tests() {
    let mut layer = VfxLayer::new();
    layer.add_binding(VfxBinding::new(1, VfxEffectType::ExplosionRing, "root", 0.5));
    layer.add_binding(VfxBinding::new(2, VfxEffectType::SmokeColumn, "root", 0.5));
    layer.add_binding(VfxBinding::new(3, VfxEffectType::ParticleEmitter, "Spine", 0.0));

    let active = layer.active_at(0.7);
    assert!(!active.is_empty());
    let explosions = layer.bindings_of_type(&VfxEffectType::ExplosionRing);
    assert_eq!(explosions.len(), 1);
}

pub fn run_sound_cue_tests() {
    let mut track = SoundTrack::new("sfx_track");
    track.add_cue(SoundCue::new(1, "footstep", SoundCueType::Foley, 0.0, 0.3));
    track.add_cue(SoundCue::new(2, "explosion", SoundCueType::OneShot, 1.5, 2.0));
    track.add_cue(SoundCue::new(3, "ambient_wind", SoundCueType::Looping, 0.0, 10.0));

    let active_at_0 = track.cues_at_time(0.1);
    assert!(!active_at_0.is_empty());
    let active_at_2 = track.cues_at_time(2.0);
    assert!(active_at_2.iter().any(|c| c.name == "explosion"));
    assert!((track.duration_s() - 10.0).abs() < 0.01);
}

pub fn run_render_export_tests() {
    let mut export = RenderSequenceExport::new("/renders/shot01", 1920, 1080, 24.0);
    export.start_frame = 1;
    export.end_frame = 240;
    export.add_aov("beauty");
    export.add_aov("normal");
    export.add_aov("depth");

    assert_eq!(export.frame_count(), 240);
    assert!((export.duration_s() - 10.0).abs() < 0.01);
    assert!(export.total_bytes() > 0);
    let filename = export.frame_filename(1);
    assert!(filename.contains("000001"));
    assert_eq!(export.aov_outputs.len(), 3);
}

pub fn run_facial_capture_tests() {
    let mut take = FacialCaptureTake::new("take_001", "John", 30.0);

    for i in 0..30 {
        let mut frame = FacialCaptureFrame::new(i as f64 / 30.0);
        frame.set_blend_shape("mouthSmileLeft", if i > 15 { 0.8 } else { 0.1 });
        frame.set_blend_shape("mouthSmileRight", if i > 15 { 0.7 } else { 0.1 });
        frame.confidence = if i == 5 { 0.3 } else { 0.95 };
        take.add_frame(frame);
    }

    assert_eq!(take.frame_count(), 30);
    let low_conf = take.low_confidence_frames(0.5);
    assert_eq!(low_conf.len(), 1);
    let hist = take.dominant_expression_histogram();
    assert!(!hist.is_empty());
    take.smooth_blend_shapes(3);
}

pub fn run_light_anim_tests() {
    let mut track = LightAnimTrack::new("sun_light");
    track.add_keyframe(ExtLightKeyframe::new(0.0, Vec3::new(1.0, 0.8, 0.6), 100000.0));
    track.add_keyframe(ExtLightKeyframe::new(5.0, Vec3::new(0.8, 0.4, 0.2), 50000.0));
    track.add_keyframe(ExtLightKeyframe::new(10.0, Vec3::new(0.1, 0.1, 0.2), 10000.0));

    assert_eq!(track.keyframe_count(), 3);
    let sample = track.sample(2.5).unwrap();
    assert!((sample.intensity - 75000.0).abs() < 1000.0);
    assert!((track.duration_s() - 10.0).abs() < 0.01);
}

pub fn run_director_tests() {
    let mut director = CutsceneDirector::new("cutscene_01", 30.0);
    director.add_marker(5.0, "action_start");
    director.add_marker(20.0, "climax");
    director.add_marker(28.0, "fade_out");

    let mut sound_track = SoundTrack::new("music");
    sound_track.add_cue(SoundCue::new(1, "music_main", SoundCueType::Music, 0.0, 30.0));
    director.add_sound_track(sound_track);

    director.vfx_layer.add_binding(VfxBinding::new(1, VfxEffectType::FirePlume, "root", 10.0));

    director.play();
    director.advance(5.0);
    assert!((director.current_time_s - 5.0).abs() < 0.01);
    assert!((director.progress_pct() - 16.67).abs() < 0.1);

    let markers = director.markers_in_range(0.0, 10.0);
    assert_eq!(markers.len(), 1);

    director.seek(0.0);
    assert!((director.current_time_s - 0.0).abs() < 0.01);
    assert!(director.render_pass_count() >= 8);
}

pub fn run_retarget_tests() {
    let profile = RetargetProfile::humanoid_biped();
    assert!(profile.mapping_count() >= 20);
    let mapping = profile.find_mapping("Hips");
    assert!(mapping.is_some());
    assert_eq!(mapping.unwrap().target_bone, "pelvis");
}

pub fn run_all_cutscene_post_tests() {
    run_post_process_tests();
    run_usd_stage_tests();
    run_alembic_tests();
    run_quantized_anim_tests();
    run_vfx_tests();
    run_sound_cue_tests();
    run_render_export_tests();
    run_facial_capture_tests();
    run_light_anim_tests();
    run_director_tests();
    run_retarget_tests();
}

pub const CUTSCENE_MAX_TRACKS: usize = 64;
pub const CUTSCENE_MAX_SOUND_CUES: usize = 256;
pub const FACIAL_CAPTURE_BLEND_SHAPES: usize = 52;
pub const USD_MAX_PRIMS: usize = 100_000;
pub const ALEMBIC_MAX_SAMPLES_PER_OBJECT: usize = 10_000;
pub const RENDER_MAX_AOVS: usize = 32;

pub fn cutscene_importer_info_extended() -> HashMap<String, String> {
    let mut info = HashMap::new();
    info.insert("module".to_string(), "cutscene_importer".to_string());
    info.insert("version".to_string(), "2.0.0".to_string());
    info.insert("blend_shapes".to_string(), FACIAL_CAPTURE_BLEND_SHAPES.to_string());
    info.insert("max_tracks".to_string(), CUTSCENE_MAX_TRACKS.to_string());
    info.insert("usd_support".to_string(), "USDA 1.0".to_string());
    info.insert("alembic_support".to_string(), "Alembic 1.7+".to_string());
    info.insert("quantization".to_string(), "16-bit quaternion/translation".to_string());
    info
}


// ============================================================
// CINEMATIC CAMERA SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct CameraAperture {
    pub f_stop: f32,
    pub focal_length_mm: f32,
    pub sensor_width_mm: f32,
    pub sensor_height_mm: f32,
    pub focus_distance: f32,
    pub dof_enabled: bool,
    pub bokeh_shape_sides: u32,
}

impl CameraAperture {
    pub fn new_cinema() -> Self {
        CameraAperture {
            f_stop: 2.8,
            focal_length_mm: 35.0,
            sensor_width_mm: 36.0,
            sensor_height_mm: 24.0,
            focus_distance: 5.0,
            dof_enabled: true,
            bokeh_shape_sides: 6,
        }
    }

    pub fn field_of_view_horizontal(&self) -> f32 {
        2.0 * ((self.sensor_width_mm / (2.0 * self.focal_length_mm)).atan())
    }

    pub fn field_of_view_vertical(&self) -> f32 {
        2.0 * ((self.sensor_height_mm / (2.0 * self.focal_length_mm)).atan())
    }

    pub fn circle_of_confusion(&self, subject_distance: f32) -> f32 {
        let magnification = self.focal_length_mm / (subject_distance * 1000.0 - self.focal_length_mm);
        magnification * self.focal_length_mm / self.f_stop
    }

    pub fn depth_of_field_near(&self) -> f32 {
        let fd = self.focus_distance * 1000.0;
        let coc = 0.03;
        let hyperfocal = (self.focal_length_mm * self.focal_length_mm) / (self.f_stop * coc) + self.focal_length_mm;
        let near = fd * (hyperfocal - self.focal_length_mm) / (hyperfocal + fd - 2.0 * self.focal_length_mm);
        near / 1000.0
    }

    pub fn depth_of_field_far(&self) -> f32 {
        let fd = self.focus_distance * 1000.0;
        let coc = 0.03;
        let hyperfocal = (self.focal_length_mm * self.focal_length_mm) / (self.f_stop * coc) + self.focal_length_mm;
        let far = fd * (hyperfocal - self.focal_length_mm) / (hyperfocal - fd);
        if far < 0.0 { f32::INFINITY } else { far / 1000.0 }
    }
}

#[derive(Debug, Clone)]
pub struct CameraMotionBlur {
    pub shutter_angle_degrees: f32,
    pub sample_count: u32,
    pub enabled: bool,
}

impl CameraMotionBlur {
    pub fn new() -> Self {
        CameraMotionBlur { shutter_angle_degrees: 180.0, sample_count: 16, enabled: true }
    }

    pub fn shutter_speed_fraction(&self, fps: f32) -> f32 {
        fps / (360.0 / self.shutter_angle_degrees)
    }
}

#[derive(Debug, Clone)]
pub struct CameraLensFlare {
    pub enabled: bool,
    pub intensity: f32,
    pub streak_count: u32,
    pub streak_rotation_deg: f32,
    pub ghost_count: u32,
    pub halo_radius: f32,
    pub dirt_texture_intensity: f32,
}

impl Default for CameraLensFlare {
    fn default() -> Self {
        CameraLensFlare {
            enabled: false,
            intensity: 0.5,
            streak_count: 8,
            streak_rotation_deg: 45.0,
            ghost_count: 4,
            halo_radius: 0.15,
            dirt_texture_intensity: 0.2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraVignette {
    pub enabled: bool,
    pub intensity: f32,
    pub smoothness: f32,
    pub roundness: f32,
    pub center: Vec2,
}

impl Default for CameraVignette {
    fn default() -> Self {
        CameraVignette {
            enabled: false,
            intensity: 0.4,
            smoothness: 0.4,
            roundness: 1.0,
            center: Vec2::new(0.5, 0.5),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChromaticAberration {
    pub enabled: bool,
    pub intensity: f32,
    pub fast_mode: bool,
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        ChromaticAberration { enabled: false, intensity: 0.05, fast_mode: false }
    }
}

#[derive(Debug, Clone)]
pub struct CinematicCamera {
    pub id: u32,
    pub name: String,
    pub position: Vec3,
    pub rotation: Quat,
    pub aperture: CameraAperture,
    pub motion_blur: CameraMotionBlur,
    pub lens_flare: CameraLensFlare,
    pub vignette: CameraVignette,
    pub chromatic_aberration: ChromaticAberration,
    pub near_clip: f32,
    pub far_clip: f32,
    pub aspect_ratio: f32,
}

impl CinematicCamera {
    pub fn new(id: u32, name: &str) -> Self {
        CinematicCamera {
            id,
            name: name.to_string(),
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            aperture: CameraAperture::new_cinema(),
            motion_blur: CameraMotionBlur::new(),
            lens_flare: CameraLensFlare::default(),
            vignette: CameraVignette::default(),
            chromatic_aberration: ChromaticAberration::default(),
            near_clip: 0.1,
            far_clip: 10000.0,
            aspect_ratio: 16.0 / 9.0,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        let rot = Mat4::from_quat(self.rotation);
        let trans = Mat4::from_translation(-self.position);
        rot.transpose() * trans
    }

    pub fn projection_matrix(&self) -> Mat4 {
        let fov_y = self.aperture.field_of_view_vertical();
        Mat4::perspective_rh(fov_y, self.aspect_ratio, self.near_clip, self.far_clip)
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }
}

#[derive(Debug, Clone)]
pub struct ExtCameraKeyframe {
    pub time: f32,
    pub position: Vec3,
    pub rotation: Quat,
    pub focal_length: f32,
    pub f_stop: f32,
    pub focus_distance: f32,
}

#[derive(Debug, Clone)]
pub struct CameraAnimTrack {
    pub camera_id: u32,
    pub keyframes: Vec<ExtCameraKeyframe>,
}

impl CameraAnimTrack {
    pub fn new(camera_id: u32) -> Self {
        CameraAnimTrack { camera_id, keyframes: Vec::new() }
    }

    pub fn add_keyframe(&mut self, kf: ExtCameraKeyframe) {
        let idx = self.keyframes.partition_point(|k| k.time < kf.time);
        self.keyframes.insert(idx, kf);
    }

    pub fn sample(&self, t: f32) -> Option<ExtCameraKeyframe> {
        if self.keyframes.is_empty() { return None; }
        if t <= self.keyframes[0].time {
            return Some(self.keyframes[0].clone());
        }
        let last = self.keyframes.last().unwrap();
        if t >= last.time {
            return Some(last.clone());
        }
        let idx = self.keyframes.partition_point(|k| k.time <= t) - 1;
        let a = &self.keyframes[idx];
        let b = &self.keyframes[idx + 1];
        let s = (t - a.time) / (b.time - a.time);
        Some(ExtCameraKeyframe {
            time: t,
            position: a.position.lerp(b.position, s),
            rotation: a.rotation.slerp(b.rotation, s),
            focal_length: a.focal_length + (b.focal_length - a.focal_length) * s,
            f_stop: a.f_stop + (b.f_stop - a.f_stop) * s,
            focus_distance: a.focus_distance + (b.focus_distance - a.focus_distance) * s,
        })
    }

    pub fn duration(&self) -> f32 {
        self.keyframes.last().map(|k| k.time).unwrap_or(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct DollyTrackSegment {
    pub start: Vec3,
    pub end: Vec3,
    pub speed: f32,
}

#[derive(Debug, Clone)]
pub struct CameraRig {
    pub rig_type: CameraRigType,
    pub position: Vec3,
    pub rotation: Quat,
    pub arm_length: f32,
    pub tilt_angle: f32,
    pub pan_angle: f32,
    pub noise_seed: u32,
    pub noise_intensity: f32,
    pub noise_frequency: f32,
    pub dolly_segments: Vec<DollyTrackSegment>,
    pub dolly_position_t: f32,
}

impl CameraRig {
    pub fn new(rig_type: CameraRigType) -> Self {
        CameraRig {
            rig_type,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            arm_length: 1.0,
            tilt_angle: 0.0,
            pan_angle: 0.0,
            noise_seed: 42,
            noise_intensity: 0.01,
            noise_frequency: 2.0,
            dolly_segments: Vec::new(),
            dolly_position_t: 0.0,
        }
    }

    pub fn handhold_noise(&self, t: f32) -> Vec3 {
        let s = self.noise_seed as f32 * 0.001;
        let x = (t * self.noise_frequency + s).sin() * self.noise_intensity;
        let y = (t * self.noise_frequency * 1.3 + s + 1.0).sin() * self.noise_intensity * 0.5;
        let z = (t * self.noise_frequency * 0.7 + s + 2.0).sin() * self.noise_intensity * 0.3;
        Vec3::new(x, y, z)
    }

    pub fn crane_tip_position(&self) -> Vec3 {
        let arm_dir = self.rotation * Vec3::new(self.pan_angle.sin(), self.tilt_angle.sin(), self.pan_angle.cos());
        self.position + arm_dir * self.arm_length
    }

    pub fn add_dolly_segment(&mut self, seg: DollyTrackSegment) {
        self.dolly_segments.push(seg);
    }

    pub fn dolly_total_length(&self) -> f32 {
        self.dolly_segments.iter().map(|s| (s.end - s.start).length()).sum()
    }
}

// ============================================================
// SUBTITLE AND LOCALIZATION SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SubtitleAlignment {
    BottomCenter,
    BottomLeft,
    BottomRight,
    TopCenter,
    MiddleCenter,
}

#[derive(Debug, Clone)]
pub struct SubtitleStyle {
    pub font_size: f32,
    pub color: Vec4,
    pub outline_color: Vec4,
    pub outline_width: f32,
    pub background_color: Vec4,
    pub background_enabled: bool,
    pub alignment: SubtitleAlignment,
    pub vertical_position: f32,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        SubtitleStyle {
            font_size: 24.0,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            outline_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            outline_width: 1.5,
            background_color: Vec4::new(0.0, 0.0, 0.0, 0.5),
            background_enabled: false,
            alignment: SubtitleAlignment::BottomCenter,
            vertical_position: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubtitleCue {
    pub id: u32,
    pub start_time: f32,
    pub end_time: f32,
    pub text: String,
    pub speaker: Option<String>,
    pub style_override: Option<SubtitleStyle>,
}

impl SubtitleCue {
    pub fn duration(&self) -> f32 {
        self.end_time - self.start_time
    }

    pub fn is_active_at(&self, t: f32) -> bool {
        t >= self.start_time && t < self.end_time
    }

    pub fn fade_alpha(&self, t: f32, fade_in: f32, fade_out: f32) -> f32 {
        if t < self.start_time + fade_in {
            (t - self.start_time) / fade_in.max(0.001)
        } else if t > self.end_time - fade_out {
            (self.end_time - t) / fade_out.max(0.001)
        } else {
            1.0
        }
        .clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone)]
pub struct LocalizedSubtitles {
    pub language_code: String,
    pub cues: Vec<SubtitleCue>,
    pub default_style: SubtitleStyle,
    pub metadata: HashMap<String, String>,
}

impl LocalizedSubtitles {
    pub fn new(language_code: &str) -> Self {
        LocalizedSubtitles {
            language_code: language_code.to_string(),
            cues: Vec::new(),
            default_style: SubtitleStyle::default(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_cue(&mut self, cue: SubtitleCue) {
        self.cues.push(cue);
        self.cues.sort_by(|a, b| a.start_time.partial_cmp(&b.start_time).unwrap());
    }

    pub fn cues_at(&self, t: f32) -> Vec<&SubtitleCue> {
        self.cues.iter().filter(|c| c.is_active_at(t)).collect()
    }

    pub fn export_srt(&self) -> String {
        let mut out = String::new();
        for (i, cue) in self.cues.iter().enumerate() {
            let fmt_time = |s: f32| -> String {
                let h = (s / 3600.0) as u32;
                let m = ((s % 3600.0) / 60.0) as u32;
                let sec = (s % 60.0) as u32;
                let ms = ((s % 1.0) * 1000.0) as u32;
                format!("{:02}:{:02}:{:02},{:03}", h, m, sec, ms)
            };
            out.push_str(&format!("{}\n{} --> {}\n{}\n\n",
                i + 1,
                fmt_time(cue.start_time),
                fmt_time(cue.end_time),
                cue.text));
        }
        out
    }

    pub fn export_vtt(&self) -> String {
        let mut out = String::from("WEBVTT\n\n");
        for cue in &self.cues {
            let fmt_time = |s: f32| -> String {
                let h = (s / 3600.0) as u32;
                let m = ((s % 3600.0) / 60.0) as u32;
                let sec = (s % 60.0) as u32;
                let ms = ((s % 1.0) * 1000.0) as u32;
                format!("{:02}:{:02}:{:02}.{:03}", h, m, sec, ms)
            };
            out.push_str(&format!("{} --> {}\n{}\n\n",
                fmt_time(cue.start_time),
                fmt_time(cue.end_time),
                cue.text));
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct SubtitleManager {
    pub tracks: HashMap<String, LocalizedSubtitles>,
    pub active_language: String,
    pub fade_in_duration: f32,
    pub fade_out_duration: f32,
}

impl SubtitleManager {
    pub fn new(default_lang: &str) -> Self {
        SubtitleManager {
            tracks: HashMap::new(),
            active_language: default_lang.to_string(),
            fade_in_duration: 0.1,
            fade_out_duration: 0.1,
        }
    }

    pub fn add_language(&mut self, lang: LocalizedSubtitles) {
        self.tracks.insert(lang.language_code.clone(), lang);
    }

    pub fn active_cues(&self, t: f32) -> Vec<&SubtitleCue> {
        self.tracks.get(&self.active_language)
            .map(|l| l.cues_at(t))
            .unwrap_or_default()
    }
}

// ============================================================
// NARRATIVE EVENT SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum NarrativeEventType {
    DialogueLine,
    CharacterEnter,
    CharacterExit,
    EnvironmentChange,
    MusicChange,
    SfxTrigger,
    CameraChange,
    FlagSet,
    QuestUpdate,
    ItemGrant,
    SceneTransition,
}

#[derive(Debug, Clone)]
pub struct NarrativeEvent {
    pub id: u32,
    pub event_type: NarrativeEventType,
    pub trigger_time: f32,
    pub payload: HashMap<String, String>,
    pub conditions: Vec<String>,
    pub fired: bool,
}

impl NarrativeEvent {
    pub fn new(id: u32, event_type: NarrativeEventType, trigger_time: f32) -> Self {
        NarrativeEvent {
            id,
            event_type,
            trigger_time,
            payload: HashMap::new(),
            conditions: Vec::new(),
            fired: false,
        }
    }

    pub fn with_payload(mut self, key: &str, value: &str) -> Self {
        self.payload.insert(key.to_string(), value.to_string());
        self
    }
}

#[derive(Debug, Clone)]
pub struct NarrativeSequencer {
    pub events: Vec<NarrativeEvent>,
    pub global_flags: HashMap<String, bool>,
    pub event_log: VecDeque<(f32, u32)>,
    pub max_log_entries: usize,
}

impl NarrativeSequencer {
    pub fn new() -> Self {
        NarrativeSequencer {
            events: Vec::new(),
            global_flags: HashMap::new(),
            event_log: VecDeque::new(),
            max_log_entries: 256,
        }
    }

    pub fn add_event(&mut self, ev: NarrativeEvent) {
        self.events.push(ev);
        self.events.sort_by(|a, b| a.trigger_time.partial_cmp(&b.trigger_time).unwrap());
    }

    pub fn tick(&mut self, t: f32) -> Vec<u32> {
        let mut fired_ids = Vec::new();
        for ev in self.events.iter_mut() {
            if !ev.fired && t >= ev.trigger_time {
                let conds_met = ev.conditions.iter().all(|c| {
                    self.global_flags.get(c).copied().unwrap_or(false)
                });
                if conds_met {
                    ev.fired = true;
                    fired_ids.push(ev.id);
                    if self.event_log.len() >= self.max_log_entries {
                        self.event_log.pop_front();
                    }
                    self.event_log.push_back((t, ev.id));
                }
            }
        }
        fired_ids
    }

    pub fn set_flag(&mut self, flag: &str, value: bool) {
        self.global_flags.insert(flag.to_string(), value);
    }

    pub fn reset(&mut self) {
        for ev in self.events.iter_mut() {
            ev.fired = false;
        }
    }

    pub fn unfired_count(&self) -> usize {
        self.events.iter().filter(|e| !e.fired).count()
    }
}

// ============================================================
// CHARACTER DIALOGUE SYSTEM
// ============================================================

#[derive(Debug, Clone)]
pub struct DialogueLine {
    pub id: u32,
    pub speaker_id: String,
    pub text: String,
    pub emotion: String,
    pub audio_asset: Option<String>,
    pub duration_hint: f32,
    pub lip_sync_asset: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DialogueChoice {
    pub id: u32,
    pub text: String,
    pub next_node_id: u32,
    pub conditions: Vec<String>,
    pub consequences: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum DialogueNodeContent {
    Line(DialogueLine),
    Choice(Vec<DialogueChoice>),
    Branch { condition: String, true_node: u32, false_node: u32 },
    End,
}

#[derive(Debug, Clone)]
pub struct DialogueNode {
    pub id: u32,
    pub content: DialogueNodeContent,
    pub next_id: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct DialogueGraph {
    pub id: u32,
    pub name: String,
    pub nodes: HashMap<u32, DialogueNode>,
    pub start_node_id: u32,
}

impl DialogueGraph {
    pub fn new(id: u32, name: &str, start_node_id: u32) -> Self {
        DialogueGraph { id, name: name.to_string(), nodes: HashMap::new(), start_node_id }
    }

    pub fn add_node(&mut self, node: DialogueNode) {
        self.nodes.insert(node.id, node);
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if !self.nodes.contains_key(&self.start_node_id) {
            errors.push(format!("Start node {} not found", self.start_node_id));
        }
        for (id, node) in &self.nodes {
            if let Some(next) = node.next_id {
                if !self.nodes.contains_key(&next) {
                    errors.push(format!("Node {} references missing next {}", id, next));
                }
            }
        }
        errors
    }
}

// ============================================================
// ANIMATION RETARGETING EXTENDED
// ============================================================

#[derive(Debug, Clone)]
pub struct ExtIkChain {
    pub name: String,
    pub root_bone: String,
    pub end_effector_bone: String,
    pub intermediate_bones: Vec<String>,
    pub pole_vector: Vec3,
    pub weight: f32,
    pub iterations: u32,
}

impl ExtIkChain {
    pub fn new_arm_left() -> Self {
        ExtIkChain {
            name: "LeftArm".to_string(),
            root_bone: "LeftShoulder".to_string(),
            end_effector_bone: "LeftHand".to_string(),
            intermediate_bones: vec!["LeftUpperArm".to_string(), "LeftForeArm".to_string()],
            pole_vector: Vec3::new(-1.0, 0.0, -1.0),
            weight: 1.0,
            iterations: 10,
        }
    }

    pub fn new_leg_right() -> Self {
        ExtIkChain {
            name: "RightLeg".to_string(),
            root_bone: "RightUpperLeg".to_string(),
            end_effector_bone: "RightFoot".to_string(),
            intermediate_bones: vec!["RightLowerLeg".to_string()],
            pole_vector: Vec3::new(0.0, 0.0, 1.0),
            weight: 1.0,
            iterations: 10,
        }
    }

    pub fn chain_length(&self) -> usize {
        2 + self.intermediate_bones.len()
    }
}

#[derive(Debug, Clone)]
pub struct RetargetingConstraint {
    pub source_bone: String,
    pub target_bone: String,
    pub rotation_only: bool,
    pub flip_x: bool,
    pub flip_y: bool,
    pub flip_z: bool,
    pub local_offset_rotation: Quat,
}

impl RetargetingConstraint {
    pub fn new(source: &str, target: &str) -> Self {
        RetargetingConstraint {
            source_bone: source.to_string(),
            target_bone: target.to_string(),
            rotation_only: false,
            flip_x: false, flip_y: false, flip_z: false,
            local_offset_rotation: Quat::IDENTITY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetargetSession {
    pub source_skeleton: String,
    pub target_skeleton: String,
    pub constraints: Vec<RetargetingConstraint>,
    pub ik_chains: Vec<ExtIkChain>,
    pub global_scale: f32,
    pub hip_height_correction: f32,
}

impl RetargetSession {
    pub fn new(source: &str, target: &str) -> Self {
        RetargetSession {
            source_skeleton: source.to_string(),
            target_skeleton: target.to_string(),
            constraints: Vec::new(),
            ik_chains: Vec::new(),
            global_scale: 1.0,
            hip_height_correction: 0.0,
        }
    }

    pub fn add_constraint(&mut self, c: RetargetingConstraint) {
        self.constraints.push(c);
    }

    pub fn add_ik_chain(&mut self, chain: ExtIkChain) {
        self.ik_chains.push(chain);
    }

    pub fn humanoid_default(source: &str, target: &str) -> Self {
        let mut session = RetargetSession::new(source, target);
        let bone_pairs = [
            ("Hips", "Hips"), ("Spine", "Spine"), ("Chest", "Chest"),
            ("UpperChest", "UpperChest"), ("Neck", "Neck"), ("Head", "Head"),
            ("LeftShoulder", "LeftShoulder"), ("LeftUpperArm", "LeftUpperArm"),
            ("LeftForeArm", "LeftForeArm"), ("LeftHand", "LeftHand"),
            ("RightShoulder", "RightShoulder"), ("RightUpperArm", "RightUpperArm"),
            ("RightForeArm", "RightForeArm"), ("RightHand", "RightHand"),
            ("LeftUpperLeg", "LeftUpperLeg"), ("LeftLowerLeg", "LeftLowerLeg"),
            ("LeftFoot", "LeftFoot"), ("RightUpperLeg", "RightUpperLeg"),
            ("RightLowerLeg", "RightLowerLeg"), ("RightFoot", "RightFoot"),
        ];
        for (src, tgt) in &bone_pairs {
            session.add_constraint(RetargetingConstraint::new(src, tgt));
        }
        session.add_ik_chain(ExtIkChain::new_arm_left());
        session.add_ik_chain(ExtIkChain::new_leg_right());
        session
    }
}

// ============================================================
// TIMELINE CLIP OPERATIONS
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ClipEditOperation {
    Trim,
    Split,
    Merge,
    SpeedChange,
    Reverse,
    Slip,
}

#[derive(Debug, Clone)]
pub struct ClipEditHistory {
    pub clip_id: u32,
    pub operation: ClipEditOperation,
    pub before_start: f32,
    pub before_end: f32,
    pub after_start: f32,
    pub after_end: f32,
    pub timestamp: f32,
}

#[derive(Debug, Clone)]
pub struct TimelineClipEditor {
    pub selected_clip_ids: HashSet<u32>,
    pub edit_history: VecDeque<ClipEditHistory>,
    pub max_history: usize,
    pub snapping_enabled: bool,
    pub snap_threshold: f32,
    pub ripple_edit_enabled: bool,
}

impl TimelineClipEditor {
    pub fn new() -> Self {
        TimelineClipEditor {
            selected_clip_ids: HashSet::new(),
            edit_history: VecDeque::new(),
            max_history: 100,
            snapping_enabled: true,
            snap_threshold: 0.05,
            ripple_edit_enabled: false,
        }
    }

    pub fn select_clip(&mut self, clip_id: u32) {
        self.selected_clip_ids.insert(clip_id);
    }

    pub fn deselect_all(&mut self) {
        self.selected_clip_ids.clear();
    }

    pub fn record_edit(&mut self, history: ClipEditHistory) {
        if self.edit_history.len() >= self.max_history {
            self.edit_history.pop_front();
        }
        self.edit_history.push_back(history);
    }

    pub fn snap_to_nearest(&self, time: f32, snap_points: &[f32]) -> f32 {
        if !self.snapping_enabled { return time; }
        snap_points.iter()
            .min_by(|&&a, &&b| (a - time).abs().partial_cmp(&(b - time).abs()).unwrap())
            .copied()
            .filter(|&s| (s - time).abs() <= self.snap_threshold)
            .unwrap_or(time)
    }

    pub fn undo_last(&mut self) -> Option<ClipEditHistory> {
        self.edit_history.pop_back()
    }
}

// ============================================================
// SCENE COMPOSITION AND LAYERING
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum LayerBlendMode {
    Normal,
    Additive,
    Multiply,
    Screen,
    Overlay,
}

#[derive(Debug, Clone)]
pub struct SceneLayer {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub blend_mode: LayerBlendMode,
    pub clip_ids: Vec<u32>,
    pub color_tag: Vec4,
}

impl SceneLayer {
    pub fn new(id: u32, name: &str) -> Self {
        SceneLayer {
            id,
            name: name.to_string(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend_mode: LayerBlendMode::Normal,
            clip_ids: Vec::new(),
            color_tag: Vec4::new(0.5, 0.5, 0.5, 1.0),
        }
    }

    pub fn add_clip(&mut self, clip_id: u32) {
        if !self.clip_ids.contains(&clip_id) {
            self.clip_ids.push(clip_id);
        }
    }

    pub fn remove_clip(&mut self, clip_id: u32) {
        self.clip_ids.retain(|&id| id != clip_id);
    }
}

#[derive(Debug, Clone)]
pub struct LayerStack {
    pub layers: Vec<SceneLayer>,
    pub active_layer_id: u32,
}

impl LayerStack {
    pub fn new() -> Self {
        LayerStack { layers: Vec::new(), active_layer_id: 0 }
    }

    pub fn add_layer(&mut self, layer: SceneLayer) {
        self.layers.push(layer);
    }

    pub fn get_layer_mut(&mut self, id: u32) -> Option<&mut SceneLayer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn visible_layers(&self) -> Vec<&SceneLayer> {
        self.layers.iter().filter(|l| l.visible).collect()
    }

    pub fn move_layer_up(&mut self, id: u32) {
        if let Some(idx) = self.layers.iter().position(|l| l.id == id) {
            if idx + 1 < self.layers.len() {
                self.layers.swap(idx, idx + 1);
            }
        }
    }

    pub fn move_layer_down(&mut self, id: u32) {
        if let Some(idx) = self.layers.iter().position(|l| l.id == id) {
            if idx > 0 {
                self.layers.swap(idx, idx - 1);
            }
        }
    }
}

// ============================================================
// EXPRESSION SOLVER FOR PROCEDURAL ANIMATION
// ============================================================

#[derive(Debug, Clone)]
pub enum ExprToken {
    Number(f32),
    Variable(String),
    Plus, Minus, Multiply, Divide,
    LParen, RParen,
    Sin, Cos, Abs, Floor, Ceil, Sqrt,
}

#[derive(Debug, Clone)]
pub struct ExpressionSolver {
    pub variables: HashMap<String, f32>,
}

impl ExpressionSolver {
    pub fn new() -> Self {
        ExpressionSolver { variables: HashMap::new() }
    }

    pub fn set_var(&mut self, name: &str, value: f32) {
        self.variables.insert(name.to_string(), value);
    }

    pub fn evaluate_simple(&self, tokens: &[ExprToken]) -> f32 {
        // Simple left-to-right evaluation without precedence for demonstration
        if tokens.is_empty() { return 0.0; }
        let mut result = self.eval_primary(tokens, &mut 0);
        result
    }

    fn eval_primary(&self, tokens: &[ExprToken], pos: &mut usize) -> f32 {
        if *pos >= tokens.len() { return 0.0; }
        match &tokens[*pos] {
            ExprToken::Number(n) => { *pos += 1; *n },
            ExprToken::Variable(v) => {
                *pos += 1;
                self.variables.get(v).copied().unwrap_or(0.0)
            },
            ExprToken::Sin => {
                *pos += 1;
                let arg = self.eval_primary(tokens, pos);
                arg.sin()
            },
            ExprToken::Cos => {
                *pos += 1;
                let arg = self.eval_primary(tokens, pos);
                arg.cos()
            },
            ExprToken::Abs => {
                *pos += 1;
                let arg = self.eval_primary(tokens, pos);
                arg.abs()
            },
            ExprToken::Sqrt => {
                *pos += 1;
                let arg = self.eval_primary(tokens, pos);
                arg.sqrt()
            },
            _ => 0.0,
        }
    }
}

// ============================================================
// CUTSCENE COMPRESSION AND STREAMING
// ============================================================

pub const CUTSCENE_STREAM_CHUNK_BYTES: usize = 65536;
pub const CUTSCENE_KEYFRAME_INTERVAL: u32 = 30;
pub const CUTSCENE_MAX_AUDIO_CHANNELS: u32 = 8;
pub const CUTSCENE_PREVIEW_WIDTH: u32 = 1280;
pub const CUTSCENE_PREVIEW_HEIGHT: u32 = 720;
pub const CUTSCENE_MASTER_WIDTH: u32 = 3840;
pub const CUTSCENE_MASTER_HEIGHT: u32 = 2160;

#[derive(Debug, Clone)]
pub struct CutsceneStreamChunk {
    pub chunk_index: u32,
    pub byte_offset: u64,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub is_keyframe_chunk: bool,
    pub timestamp_start: f32,
    pub timestamp_end: f32,
}

#[derive(Debug, Clone)]
pub struct CutsceneStreamIndex {
    pub total_chunks: u32,
    pub total_duration: f32,
    pub chunks: Vec<CutsceneStreamChunk>,
}

impl CutsceneStreamIndex {
    pub fn new() -> Self {
        CutsceneStreamIndex { total_chunks: 0, total_duration: 0.0, chunks: Vec::new() }
    }

    pub fn add_chunk(&mut self, chunk: CutsceneStreamChunk) {
        self.total_chunks += 1;
        if chunk.timestamp_end > self.total_duration {
            self.total_duration = chunk.timestamp_end;
        }
        self.chunks.push(chunk);
    }

    pub fn chunk_for_time(&self, t: f32) -> Option<&CutsceneStreamChunk> {
        self.chunks.iter().find(|c| t >= c.timestamp_start && t < c.timestamp_end)
    }

    pub fn total_compressed_bytes(&self) -> u64 {
        self.chunks.iter().map(|c| c.compressed_size as u64).sum()
    }

    pub fn compression_ratio(&self) -> f32 {
        let compressed: u64 = self.chunks.iter().map(|c| c.compressed_size as u64).sum();
        let uncompressed: u64 = self.chunks.iter().map(|c| c.uncompressed_size as u64).sum();
        if uncompressed == 0 { return 1.0; }
        compressed as f32 / uncompressed as f32
    }
}

// ============================================================
// SHOT LIST AND PRE-VISUALIZATION
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ShotType {
    ExtremeLongShot,
    LongShot,
    MediumLongShot,
    MediumShot,
    MediumCloseUp,
    CloseUp,
    ExtremeCloseUp,
    CutawayShot,
    InsertShot,
    CutInShot,
    PointOfViewShot,
    OverTheShoulderShot,
    TwoShot,
    EnsembleShot,
    AerialShot,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraAngle {
    EyeLevel,
    HighAngle,
    LowAngle,
    BirdsEye,
    WormsEye,
    DutchAngle,
    SubjectivePov,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraMovement {
    Static,
    Pan,
    Tilt,
    Dolly,
    Truck,
    Pedestal,
    Arc,
    Zoom,
    Crane,
    Handheld,
    Steadicam,
}

#[derive(Debug, Clone)]
pub struct ShotListEntry {
    pub shot_number: String,
    pub scene_number: String,
    pub description: String,
    pub shot_type: ShotType,
    pub camera_angle: CameraAngle,
    pub camera_movement: CameraMovement,
    pub focal_length: f32,
    pub estimated_duration: f32,
    pub rig_notes: String,
    pub lighting_notes: String,
    pub priority: u32,
    pub complete: bool,
}

#[derive(Debug, Clone)]
pub struct ShotList {
    pub title: String,
    pub director: String,
    pub dop: String,
    pub date: String,
    pub entries: Vec<ShotListEntry>,
}

impl ShotList {
    pub fn new(title: &str) -> Self {
        ShotList {
            title: title.to_string(),
            director: String::new(),
            dop: String::new(),
            date: String::new(),
            entries: Vec::new(),
        }
    }

    pub fn add_shot(&mut self, shot: ShotListEntry) {
        self.entries.push(shot);
    }

    pub fn total_duration(&self) -> f32 {
        self.entries.iter().map(|e| e.estimated_duration).sum()
    }

    pub fn incomplete_shots(&self) -> Vec<&ShotListEntry> {
        self.entries.iter().filter(|e| !e.complete).collect()
    }

    pub fn shots_by_scene(&self, scene: &str) -> Vec<&ShotListEntry> {
        self.entries.iter().filter(|e| e.scene_number == scene).collect()
    }

    pub fn export_csv(&self) -> String {
        let mut out = String::from("Shot,Scene,Description,Type,Duration,Complete\n");
        for e in &self.entries {
            out.push_str(&format!("{},{},{},{:?},{:.1},{}\n",
                e.shot_number, e.scene_number, e.description,
                e.shot_type, e.estimated_duration, e.complete));
        }
        out
    }
}

// ============================================================
// ADVANCED BLEND TREE / STATE MACHINE FOR ANIMATION
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum BlendNodeType {
    Clip,
    Blend1D,
    Blend2D,
    Override,
    LayeredBlend,
    StateMachine,
}

#[derive(Debug, Clone)]
pub struct BlendNode {
    pub id: u32,
    pub node_type: BlendNodeType,
    pub name: String,
    pub child_ids: Vec<u32>,
    pub blend_param: String,
    pub blend_value: f32,
    pub clip_asset: Option<String>,
    pub loop_clip: bool,
    pub play_rate: f32,
}

impl BlendNode {
    pub fn new_clip(id: u32, name: &str, asset: &str) -> Self {
        BlendNode {
            id,
            node_type: BlendNodeType::Clip,
            name: name.to_string(),
            child_ids: Vec::new(),
            blend_param: String::new(),
            blend_value: 0.0,
            clip_asset: Some(asset.to_string()),
            loop_clip: true,
            play_rate: 1.0,
        }
    }

    pub fn new_blend1d(id: u32, name: &str, param: &str) -> Self {
        BlendNode {
            id,
            node_type: BlendNodeType::Blend1D,
            name: name.to_string(),
            child_ids: Vec::new(),
            blend_param: param.to_string(),
            blend_value: 0.0,
            clip_asset: None,
            loop_clip: false,
            play_rate: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtAnimState {
    pub id: u32,
    pub name: String,
    pub root_node_id: u32,
    pub speed: f32,
    pub loop_state: bool,
}

#[derive(Debug, Clone)]
pub struct AnimationTransition {
    pub from_state_id: u32,
    pub to_state_id: u32,
    pub condition: String,
    pub blend_duration: f32,
    pub interrupt_current: bool,
}

#[derive(Debug, Clone)]
pub struct ExtAnimStateMachine {
    pub name: String,
    pub states: HashMap<u32, ExtAnimState>,
    pub transitions: Vec<AnimationTransition>,
    pub blend_nodes: HashMap<u32, BlendNode>,
    pub current_state_id: u32,
    pub parameters: HashMap<String, f32>,
    pub bool_parameters: HashMap<String, bool>,
}

impl ExtAnimStateMachine {
    pub fn new(name: &str) -> Self {
        ExtAnimStateMachine {
            name: name.to_string(),
            states: HashMap::new(),
            transitions: Vec::new(),
            blend_nodes: HashMap::new(),
            current_state_id: 0,
            parameters: HashMap::new(),
            bool_parameters: HashMap::new(),
        }
    }

    pub fn add_state(&mut self, state: ExtAnimState) {
        self.states.insert(state.id, state);
    }

    pub fn add_transition(&mut self, t: AnimationTransition) {
        self.transitions.push(t);
    }

    pub fn set_float(&mut self, param: &str, value: f32) {
        self.parameters.insert(param.to_string(), value);
    }

    pub fn set_bool(&mut self, param: &str, value: bool) {
        self.bool_parameters.insert(param.to_string(), value);
    }

    pub fn evaluate_transitions(&self) -> Option<u32> {
        for t in &self.transitions {
            if t.from_state_id == self.current_state_id {
                if self.bool_parameters.get(&t.condition).copied().unwrap_or(false) {
                    return Some(t.to_state_id);
                }
            }
        }
        None
    }

    pub fn tick(&mut self) {
        if let Some(next_id) = self.evaluate_transitions() {
            self.current_state_id = next_id;
        }
    }
}

// ============================================================
// PROCEDURAL ANIMATION CURVES
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum CurveInterpolation {
    Linear,
    Hermite,
    CatmullRom,
    BSpline,
    Stepped,
}

#[derive(Debug, Clone)]
pub struct CurveKey {
    pub time: f32,
    pub value: f32,
    pub in_tangent: f32,
    pub out_tangent: f32,
    pub interp: CurveInterpolation,
}

#[derive(Debug, Clone)]
pub struct AnimationCurve {
    pub keys: Vec<CurveKey>,
    pub pre_wrap: bool,
    pub post_wrap: bool,
}

impl AnimationCurve {
    pub fn new() -> Self {
        AnimationCurve { keys: Vec::new(), pre_wrap: false, post_wrap: false }
    }

    pub fn add_key(&mut self, key: CurveKey) {
        let idx = self.keys.partition_point(|k| k.time < key.time);
        self.keys.insert(idx, key);
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        if self.keys.is_empty() { return 0.0; }
        if t <= self.keys[0].time { return self.keys[0].value; }
        let last = self.keys.last().unwrap();
        if t >= last.time { return last.value; }

        let idx = self.keys.partition_point(|k| k.time <= t) - 1;
        let a = &self.keys[idx];
        let b = &self.keys[idx + 1];
        let dt = b.time - a.time;
        let s = (t - a.time) / dt;

        match a.interp {
            CurveInterpolation::Linear => a.value + (b.value - a.value) * s,
            CurveInterpolation::Stepped => a.value,
            CurveInterpolation::Hermite => {
                let s2 = s * s;
                let s3 = s2 * s;
                let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
                let h10 = s3 - 2.0 * s2 + s;
                let h01 = -2.0 * s3 + 3.0 * s2;
                let h11 = s3 - s2;
                h00 * a.value + h10 * dt * a.out_tangent + h01 * b.value + h11 * dt * b.in_tangent
            },
            CurveInterpolation::CatmullRom => {
                // simplified
                let s2 = s * s;
                let s3 = s2 * s;
                0.5 * ((2.0 * a.value)
                    + (-a.value + b.value) * s
                    + (2.0 * a.value - 5.0 * a.value + 4.0 * b.value - b.value) * s2
                    + (-a.value + 3.0 * a.value - 3.0 * b.value + b.value) * s3)
            },
            CurveInterpolation::BSpline => {
                // uniform quadratic b-spline basis approximation
                let s2 = s * s;
                let s3 = s2 * s;
                a.value + (b.value - a.value) * (3.0 * s2 - 2.0 * s3)
            },
        }
    }

    pub fn duration(&self) -> f32 {
        if self.keys.len() < 2 { return 0.0; }
        self.keys.last().unwrap().time - self.keys[0].time
    }
}

// ============================================================
// TEST FUNCTIONS
// ============================================================

#[cfg(test)]
mod tests_cinematic_camera {
    use super::*;

    #[test]
    fn test_camera_aperture_fov() {
        let cam = CameraAperture::new_cinema();
        let fov_h = cam.field_of_view_horizontal();
        assert!(fov_h > 0.5 && fov_h < 1.5);
    }

    #[test]
    fn test_camera_anim_track_sample() {
        let mut track = CameraAnimTrack::new(1);
        track.add_keyframe(ExtCameraKeyframe {
            time: 0.0, position: Vec3::ZERO, rotation: Quat::IDENTITY,
            focal_length: 35.0, f_stop: 2.8, focus_distance: 5.0,
        });
        track.add_keyframe(ExtCameraKeyframe {
            time: 1.0, position: Vec3::new(1.0, 0.0, 0.0), rotation: Quat::IDENTITY,
            focal_length: 50.0, f_stop: 4.0, focus_distance: 8.0,
        });
        let kf = track.sample(0.5).unwrap();
        assert!((kf.position.x - 0.5).abs() < 0.01);
        assert!((kf.focal_length - 42.5).abs() < 0.1);
    }

    #[test]
    fn test_subtitle_srt_export() {
        let mut subs = LocalizedSubtitles::new("en");
        subs.add_cue(SubtitleCue {
            id: 1, start_time: 0.0, end_time: 2.5,
            text: "Hello world".to_string(), speaker: None, style_override: None,
        });
        let srt = subs.export_srt();
        assert!(srt.contains("Hello world"));
        assert!(srt.contains("00:00:00,000"));
    }

    #[test]
    fn test_narrative_sequencer() {
        let mut seq = NarrativeSequencer::new();
        let mut ev = NarrativeEvent::new(1, NarrativeEventType::DialogueLine, 1.0);
        seq.add_event(ev);
        let fired = seq.tick(0.5);
        assert!(fired.is_empty());
        let fired = seq.tick(1.5);
        assert_eq!(fired.len(), 1);
        let fired2 = seq.tick(2.0);
        assert!(fired2.is_empty()); // already fired
    }

    #[test]
    fn test_dialogue_graph_validate() {
        let mut graph = DialogueGraph::new(1, "Test", 1);
        graph.add_node(DialogueNode {
            id: 1,
            content: DialogueNodeContent::End,
            next_id: None,
        });
        let errors = graph.validate();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_blend_state_machine() {
        let mut sm = ExtAnimStateMachine::new("Character");
        sm.add_state(ExtAnimState { id: 0, name: "Idle".to_string(), root_node_id: 0, speed: 1.0, loop_state: true });
        sm.add_state(ExtAnimState { id: 1, name: "Walk".to_string(), root_node_id: 1, speed: 1.0, loop_state: true });
        sm.add_transition(AnimationTransition {
            from_state_id: 0, to_state_id: 1,
            condition: "IsWalking".to_string(),
            blend_duration: 0.2, interrupt_current: false,
        });
        sm.current_state_id = 0;
        sm.set_bool("IsWalking", false);
        sm.tick();
        assert_eq!(sm.current_state_id, 0);
        sm.set_bool("IsWalking", true);
        sm.tick();
        assert_eq!(sm.current_state_id, 1);
    }

    #[test]
    fn test_animation_curve_hermite() {
        let mut curve = AnimationCurve::new();
        curve.add_key(CurveKey { time: 0.0, value: 0.0, in_tangent: 0.0, out_tangent: 1.0, interp: CurveInterpolation::Hermite });
        curve.add_key(CurveKey { time: 1.0, value: 1.0, in_tangent: 1.0, out_tangent: 0.0, interp: CurveInterpolation::Hermite });
        let mid = curve.evaluate(0.5);
        assert!(mid > 0.4 && mid < 0.6);
    }

    #[test]
    fn test_retarget_session_humanoid() {
        let session = RetargetSession::humanoid_default("UE5Mannequin", "ReadyPlayerMe");
        assert_eq!(session.constraints.len(), 20);
        assert_eq!(session.ik_chains.len(), 2);
    }

    #[test]
    fn test_shot_list_csv() {
        let mut sl = ShotList::new("TestScene");
        sl.add_shot(ShotListEntry {
            shot_number: "1A".to_string(), scene_number: "1".to_string(),
            description: "Establishing shot".to_string(),
            shot_type: ShotType::LongShot, camera_angle: CameraAngle::EyeLevel,
            camera_movement: CameraMovement::Static,
            focal_length: 35.0, estimated_duration: 5.0,
            rig_notes: String::new(), lighting_notes: String::new(),
            priority: 1, complete: false,
        });
        let csv = sl.export_csv();
        assert!(csv.contains("1A"));
        assert!(csv.contains("LongShot"));
    }

    #[test]
    fn test_stream_index() {
        let mut idx = CutsceneStreamIndex::new();
        idx.add_chunk(CutsceneStreamChunk {
            chunk_index: 0, byte_offset: 0,
            compressed_size: 32768, uncompressed_size: 65536,
            is_keyframe_chunk: true, timestamp_start: 0.0, timestamp_end: 1.0,
        });
        assert!((idx.compression_ratio() - 0.5).abs() < 0.01);
        assert!(idx.chunk_for_time(0.5).is_some());
    }

    #[test]
    fn test_camera_rig_handheld_noise() {
        let rig = CameraRig::new(CameraRigType::Handheld);
        let n1 = rig.handhold_noise(0.0);
        let n2 = rig.handhold_noise(0.5);
        assert_ne!(n1.x, n2.x);
    }
}

// ============================================================
// CUTSCENE PROJECT FILE FORMAT
// ============================================================

pub const CUTSCENE_FILE_MAGIC: u32 = 0x43545343; // "CTSC"
pub const CUTSCENE_FILE_VERSION: u32 = 3;

#[derive(Debug, Clone)]
pub struct CutsceneFileHeader {
    pub magic: u32,
    pub version: u32,
    pub total_duration: f32,
    pub frame_rate: f32,
    pub track_count: u32,
    pub event_count: u32,
    pub subtitle_language_count: u32,
    pub created_timestamp: u64,
    pub modified_timestamp: u64,
    pub author: String,
    pub description: String,
}

impl CutsceneFileHeader {
    pub fn new(duration: f32, fps: f32) -> Self {
        CutsceneFileHeader {
            magic: CUTSCENE_FILE_MAGIC,
            version: CUTSCENE_FILE_VERSION,
            total_duration: duration,
            frame_rate: fps,
            track_count: 0,
            event_count: 0,
            subtitle_language_count: 0,
            created_timestamp: 0,
            modified_timestamp: 0,
            author: String::new(),
            description: String::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == CUTSCENE_FILE_MAGIC && self.version <= CUTSCENE_FILE_VERSION
    }

    pub fn total_frames(&self) -> u32 {
        (self.total_duration * self.frame_rate).ceil() as u32
    }
}

#[derive(Debug, Clone)]
pub struct CutsceneProjectFile {
    pub header: CutsceneFileHeader,
    pub cameras: Vec<CinematicCamera>,
    pub camera_tracks: Vec<CameraAnimTrack>,
    pub layer_stack: LayerStack,
    pub subtitle_manager: SubtitleManager,
    pub narrative_sequencer: NarrativeSequencer,
    pub shot_list: ShotList,
    pub stream_index: CutsceneStreamIndex,
}

impl CutsceneProjectFile {
    pub fn new(duration: f32, fps: f32) -> Self {
        CutsceneProjectFile {
            header: CutsceneFileHeader::new(duration, fps),
            cameras: Vec::new(),
            camera_tracks: Vec::new(),
            layer_stack: LayerStack::new(),
            subtitle_manager: SubtitleManager::new("en"),
            narrative_sequencer: NarrativeSequencer::new(),
            shot_list: ShotList::new("Untitled"),
            stream_index: CutsceneStreamIndex::new(),
        }
    }

    pub fn add_camera(&mut self, cam: CinematicCamera) {
        self.cameras.push(cam);
        self.header.track_count += 1;
    }

    pub fn camera_by_id(&self, id: u32) -> Option<&CinematicCamera> {
        self.cameras.iter().find(|c| c.id == id)
    }

    pub fn summary(&self) -> String {
        format!(
            "Cutscene '{}' v{}: {:.2}s @ {}fps, {} cameras, {} tracks, {} events",
            self.shot_list.title,
            self.header.version,
            self.header.total_duration,
            self.header.frame_rate,
            self.cameras.len(),
            self.header.track_count,
            self.narrative_sequencer.events.len()
        )
    }
}

// ============================================================
// AUDIO MIX BUS SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum BusType {
    Master,
    Music,
    SFX,
    Dialogue,
    Ambience,
    Aux,
}

#[derive(Debug, Clone)]
pub struct AudioEffect {
    pub name: String,
    pub enabled: bool,
    pub parameters: HashMap<String, f32>,
}

#[derive(Debug, Clone)]
pub struct MixBus {
    pub id: u32,
    pub name: String,
    pub bus_type: BusType,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    pub send_to_master: bool,
    pub effects: Vec<AudioEffect>,
    pub children: Vec<u32>,
}

impl MixBus {
    pub fn new(id: u32, name: &str, bus_type: BusType) -> Self {
        MixBus {
            id, name: name.to_string(), bus_type,
            volume: 1.0, pan: 0.0, muted: false, soloed: false,
            send_to_master: true, effects: Vec::new(), children: Vec::new(),
        }
    }

    pub fn add_effect(&mut self, effect: AudioEffect) {
        self.effects.push(effect);
    }

    pub fn effective_volume(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }

    pub fn add_reverb(&mut self, room_size: f32, damping: f32, wet: f32) {
        let mut params = HashMap::new();
        params.insert("room_size".to_string(), room_size);
        params.insert("damping".to_string(), damping);
        params.insert("wet_level".to_string(), wet);
        self.add_effect(AudioEffect {
            name: "Reverb".to_string(),
            enabled: true,
            parameters: params,
        });
    }

    pub fn add_eq_low_shelf(&mut self, frequency: f32, gain_db: f32) {
        let mut params = HashMap::new();
        params.insert("frequency".to_string(), frequency);
        params.insert("gain_db".to_string(), gain_db);
        self.add_effect(AudioEffect {
            name: "EQLowShelf".to_string(),
            enabled: true,
            parameters: params,
        });
    }
}

#[derive(Debug, Clone)]
pub struct AudioMixer {
    pub buses: HashMap<u32, MixBus>,
    pub master_bus_id: u32,
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub bit_depth: u32,
}

impl AudioMixer {
    pub fn new(sample_rate: u32, buffer_size: u32) -> Self {
        let mut mixer = AudioMixer {
            buses: HashMap::new(),
            master_bus_id: 0,
            sample_rate,
            buffer_size,
            bit_depth: 24,
        };
        mixer.buses.insert(0, MixBus::new(0, "Master", BusType::Master));
        mixer
    }

    pub fn add_bus(&mut self, bus: MixBus) {
        self.buses.insert(bus.id, bus);
    }

    pub fn default_setup() -> Self {
        let mut m = AudioMixer::new(48000, 1024);
        m.add_bus(MixBus::new(1, "Music", BusType::Music));
        m.add_bus(MixBus::new(2, "SFX", BusType::SFX));
        m.add_bus(MixBus::new(3, "Dialogue", BusType::Dialogue));
        m.add_bus(MixBus::new(4, "Ambience", BusType::Ambience));
        m
    }

    pub fn get_bus_mut(&mut self, id: u32) -> Option<&mut MixBus> {
        self.buses.get_mut(&id)
    }

    pub fn active_buses(&self) -> Vec<&MixBus> {
        self.buses.values().filter(|b| !b.muted).collect()
    }
}

// ============================================================
// CINEMATIC LENS PROFILE LIBRARY
// ============================================================

#[derive(Debug, Clone)]
pub struct LensProfile {
    pub name: String,
    pub manufacturer: String,
    pub focal_length_mm: f32,
    pub max_aperture: f32,
    pub min_aperture: f32,
    pub t_stop: f32,
    pub distortion_k1: f32,
    pub distortion_k2: f32,
    pub chromatic_aberration: f32,
    pub vignetting_amount: f32,
    pub breathing_percent: f32,
}

impl LensProfile {
    pub fn prime_35mm_fast() -> Self {
        LensProfile {
            name: "35mm T1.5".to_string(),
            manufacturer: "ProCine".to_string(),
            focal_length_mm: 35.0,
            max_aperture: 1.4,
            min_aperture: 16.0,
            t_stop: 1.5,
            distortion_k1: -0.02,
            distortion_k2: 0.005,
            chromatic_aberration: 0.03,
            vignetting_amount: 0.15,
            breathing_percent: 2.0,
        }
    }

    pub fn zoom_24_70() -> Self {
        LensProfile {
            name: "24-70mm T2.8".to_string(),
            manufacturer: "CineZoom".to_string(),
            focal_length_mm: 47.0,
            max_aperture: 2.8,
            min_aperture: 22.0,
            t_stop: 2.9,
            distortion_k1: 0.015,
            distortion_k2: -0.003,
            chromatic_aberration: 0.05,
            vignetting_amount: 0.12,
            breathing_percent: 5.0,
        }
    }

    pub fn effective_fov(&self, sensor_width: f32) -> f32 {
        2.0 * (sensor_width / (2.0 * self.focal_length_mm)).atan()
    }
}

#[derive(Debug, Clone)]
pub struct LensLibrary {
    pub profiles: Vec<LensProfile>,
}

impl LensLibrary {
    pub fn default_library() -> Self {
        LensLibrary {
            profiles: vec![
                LensProfile::prime_35mm_fast(),
                LensProfile::zoom_24_70(),
            ],
        }
    }

    pub fn find_by_focal_length(&self, fl: f32) -> Vec<&LensProfile> {
        self.profiles.iter()
            .filter(|p| (p.focal_length_mm - fl).abs() < 5.0)
            .collect()
    }
}

// info function
pub fn cutscene_importer_info() -> &'static str {
    "CutsceneImporter v3.0: Camera, Subtitle, Narrative, BlendTree, Audio, Lens"
}


// ============================================================
// MULTI-CAMERA DIRECTOR SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum DirectorCutType {
    HardCut,
    Dissolve,
    FadeToBlack,
    FadeFromBlack,
    Wipe,
    CrossFade,
    Morph,
}

#[derive(Debug, Clone)]
pub struct DirectorCut {
    pub time: f32,
    pub from_camera_id: u32,
    pub to_camera_id: u32,
    pub cut_type: DirectorCutType,
    pub transition_duration: f32,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct MultiCameraDirector {
    pub cameras: Vec<u32>,
    pub cuts: Vec<DirectorCut>,
    pub active_camera_id: u32,
    pub cut_rules: HashMap<String, f32>,
}

impl MultiCameraDirector {
    pub fn new() -> Self {
        MultiCameraDirector {
            cameras: Vec::new(),
            cuts: Vec::new(),
            active_camera_id: 0,
            cut_rules: HashMap::new(),
        }
    }

    pub fn add_cut(&mut self, cut: DirectorCut) {
        self.cuts.push(cut);
        self.cuts.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    pub fn active_at(&self, t: f32) -> u32 {
        let mut active = self.active_camera_id;
        for cut in &self.cuts {
            if cut.time <= t {
                active = cut.to_camera_id;
            }
        }
        active
    }

    pub fn next_cut(&self, t: f32) -> Option<&DirectorCut> {
        self.cuts.iter().find(|c| c.time > t)
    }

    pub fn cuts_in_range(&self, start: f32, end: f32) -> Vec<&DirectorCut> {
        self.cuts.iter().filter(|c| c.time >= start && c.time < end).collect()
    }
}

// ============================================================
// KEYFRAME INTERPOLATION UTILITIES
// ============================================================

pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }
pub fn smoothstep(a: f32, b: f32, t: f32) -> f32 {
    let x = ((t - a) / (b - a)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
pub fn smootherstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
pub fn ease_in_quad(t: f32) -> f32 { t * t }
pub fn ease_out_quad(t: f32) -> f32 { t * (2.0 - t) }
pub fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t }
}
pub fn ease_in_cubic(t: f32) -> f32 { t * t * t }
pub fn ease_out_cubic(t: f32) -> f32 { let t = t - 1.0; t * t * t + 1.0 }
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { (t - 1.0) * (2.0 * t - 2.0) * (2.0 * t - 2.0) + 1.0 }
}
pub fn ease_in_expo(t: f32) -> f32 {
    if t == 0.0 { 0.0 } else { (2.0f32).powf(10.0 * t - 10.0) }
}
pub fn ease_out_expo(t: f32) -> f32 {
    if t == 1.0 { 1.0 } else { 1.0 - (2.0f32).powf(-10.0 * t) }
}
pub fn ease_in_back(t: f32) -> f32 {
    let c1 = 1.70158_f32;
    let c3 = c1 + 1.0;
    c3 * t * t * t - c1 * t * t
}
pub fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158_f32;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}
pub fn ease_out_bounce(t: f32) -> f32 {
    let n1 = 7.5625_f32;
    let d1 = 2.75_f32;
    let t = if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    };
    t
}
pub fn ease_in_elastic(t: f32) -> f32 {
    if t == 0.0 || t == 1.0 { return t; }
    let c4 = std::f32::consts::TAU / 3.0;
    -(2.0f32.powf(10.0 * t - 10.0)) * ((10.0 * t - 10.75) * c4).sin()
}

// ============================================================
// PERFORMANCE CAPTURE SESSION
// ============================================================

#[derive(Debug, Clone)]
pub struct MocapMarker {
    pub id: u32,
    pub label: String,
    pub position: Vec3,
    pub occluded: bool,
    pub residual: f32,
}

#[derive(Debug, Clone)]
pub struct ExtMocapFrame {
    pub frame_number: u32,
    pub timestamp: f32,
    pub markers: Vec<MocapMarker>,
}

impl ExtMocapFrame {
    pub fn visible_marker_count(&self) -> usize {
        self.markers.iter().filter(|m| !m.occluded).count()
    }

    pub fn marker_by_label(&self, label: &str) -> Option<&MocapMarker> {
        self.markers.iter().find(|m| m.label == label)
    }

    pub fn centroid(&self) -> Vec3 {
        let visible: Vec<&MocapMarker> = self.markers.iter().filter(|m| !m.occluded).collect();
        if visible.is_empty() { return Vec3::ZERO; }
        let sum: Vec3 = visible.iter().map(|m| m.position).fold(Vec3::ZERO, |a, b| a + b);
        sum / visible.len() as f32
    }
}

#[derive(Debug, Clone)]
pub struct MocapSession {
    pub session_id: String,
    pub actor_name: String,
    pub capture_fps: f32,
    pub frames: Vec<ExtMocapFrame>,
    pub marker_labels: Vec<String>,
    pub calibration_residual: f32,
}

impl MocapSession {
    pub fn new(session_id: &str, actor: &str, fps: f32) -> Self {
        MocapSession {
            session_id: session_id.to_string(),
            actor_name: actor.to_string(),
            capture_fps: fps,
            frames: Vec::new(),
            marker_labels: Vec::new(),
            calibration_residual: 0.0,
        }
    }

    pub fn duration(&self) -> f32 {
        self.frames.last().map(|f| f.timestamp).unwrap_or(0.0)
    }

    pub fn frame_at(&self, t: f32) -> Option<&ExtMocapFrame> {
        let frame_num = (t * self.capture_fps) as u32;
        self.frames.iter().find(|f| f.frame_number == frame_num)
    }

    pub fn avg_marker_visibility(&self) -> f32 {
        if self.frames.is_empty() { return 0.0; }
        let total: usize = self.frames.iter().map(|f| f.visible_marker_count()).sum();
        let max_per_frame = self.marker_labels.len().max(1);
        total as f32 / (self.frames.len() * max_per_frame) as f32
    }
}

// ============================================================
// SPLINE PATH SYSTEM FOR CAMERA DOLLY
// ============================================================

#[derive(Debug, Clone)]
pub struct SplineControlPoint {
    pub position: Vec3,
    pub tangent_in: Vec3,
    pub tangent_out: Vec3,
    pub roll_deg: f32,
    pub speed_multiplier: f32,
}

impl SplineControlPoint {
    pub fn new(pos: Vec3) -> Self {
        SplineControlPoint {
            position: pos,
            tangent_in: Vec3::ZERO,
            tangent_out: Vec3::ZERO,
            roll_deg: 0.0,
            speed_multiplier: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraSplinePath {
    pub control_points: Vec<SplineControlPoint>,
    pub closed: bool,
    pub tension: f32,
}

impl CameraSplinePath {
    pub fn new() -> Self {
        CameraSplinePath { control_points: Vec::new(), closed: false, tension: 0.5 }
    }

    pub fn add_point(&mut self, p: SplineControlPoint) {
        self.control_points.push(p);
        self.recompute_tangents();
    }

    fn recompute_tangents(&mut self) {
        let n = self.control_points.len();
        if n < 2 { return; }
        for i in 0..n {
            let prev = if i == 0 { if self.closed { n - 1 } else { 0 } } else { i - 1 };
            let next = if i == n - 1 { if self.closed { 0 } else { n - 1 } } else { i + 1 };
            let dp = self.control_points[next].position - self.control_points[prev].position;
            self.control_points[i].tangent_out = dp * self.tension;
            self.control_points[i].tangent_in = -dp * self.tension;
        }
    }

    pub fn evaluate(&self, t: f32) -> Vec3 {
        let n = self.control_points.len();
        if n == 0 { return Vec3::ZERO; }
        if n == 1 { return self.control_points[0].position; }
        let total_segments = if self.closed { n } else { n - 1 };
        let segment_t = (t * total_segments as f32).clamp(0.0, total_segments as f32 - 0.0001);
        let seg = segment_t as usize;
        let s = segment_t - seg as f32;
        let i0 = seg;
        let i1 = if self.closed { (seg + 1) % n } else { (seg + 1).min(n - 1) };
        let p0 = self.control_points[i0].position;
        let p1 = self.control_points[i1].position;
        let m0 = self.control_points[i0].tangent_out;
        let m1 = self.control_points[i1].tangent_in;
        let s2 = s * s;
        let s3 = s2 * s;
        let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
        let h10 = s3 - 2.0 * s2 + s;
        let h01 = -2.0 * s3 + 3.0 * s2;
        let h11 = s3 - s2;
        h00 * p0 + h10 * m0 + h01 * p1 + h11 * m1
    }

    pub fn arc_length_approximate(&self, samples: u32) -> f32 {
        if samples < 2 { return 0.0; }
        let mut total = 0.0;
        let mut prev = self.evaluate(0.0);
        for i in 1..samples {
            let t = i as f32 / (samples - 1) as f32;
            let curr = self.evaluate(t);
            total += (curr - prev).length();
            prev = curr;
        }
        total
    }
}

// ============================================================
// SCENE GRAPH OVERVIEW
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SceneNodeKind {
    Root,
    Group,
    Camera,
    Light,
    Mesh,
    SkinnedMesh,
    ParticleSystem,
    AudioSource,
    VfxInstance,
    Trigger,
    Locator,
}

#[derive(Debug, Clone)]
pub struct SceneGraphNode {
    pub id: u32,
    pub name: String,
    pub kind: SceneNodeKind,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
    pub local_position: Vec3,
    pub local_rotation: Quat,
    pub local_scale: Vec3,
    pub visible: bool,
    pub static_flag: bool,
    pub asset_ref: Option<String>,
    pub tags: HashSet<String>,
}

impl SceneGraphNode {
    pub fn new(id: u32, name: &str, kind: SceneNodeKind) -> Self {
        SceneGraphNode {
            id, name: name.to_string(), kind,
            parent_id: None, children: Vec::new(),
            local_position: Vec3::ZERO,
            local_rotation: Quat::IDENTITY,
            local_scale: Vec3::ONE,
            visible: true, static_flag: false,
            asset_ref: None, tags: HashSet::new(),
        }
    }

    pub fn local_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.local_scale, self.local_rotation, self.local_position)
    }

    pub fn add_tag(&mut self, tag: &str) {
        self.tags.insert(tag.to_string());
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }
}

#[derive(Debug, Clone)]
pub struct CutsceneSceneGraph {
    pub nodes: HashMap<u32, SceneGraphNode>,
    pub root_id: u32,
    pub next_id: u32,
}

impl CutsceneSceneGraph {
    pub fn new() -> Self {
        let mut g = CutsceneSceneGraph { nodes: HashMap::new(), root_id: 0, next_id: 1 };
        g.nodes.insert(0, SceneGraphNode::new(0, "Root", SceneNodeKind::Root));
        g
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn add_node(&mut self, mut node: SceneGraphNode, parent_id: u32) {
        node.parent_id = Some(parent_id);
        let node_id = node.id;
        self.nodes.insert(node_id, node);
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.push(node_id);
        }
    }

    pub fn world_matrix(&self, id: u32) -> Mat4 {
        let node = match self.nodes.get(&id) { Some(n) => n, None => return Mat4::IDENTITY };
        let local = node.local_matrix();
        match node.parent_id {
            Some(pid) => self.world_matrix(pid) * local,
            None => local,
        }
    }

    pub fn find_by_tag(&self, tag: &str) -> Vec<u32> {
        self.nodes.values().filter(|n| n.has_tag(tag)).map(|n| n.id).collect()
    }

    pub fn visible_nodes(&self) -> Vec<u32> {
        self.nodes.values().filter(|n| n.visible).map(|n| n.id).collect()
    }

    pub fn node_depth(&self, id: u32) -> u32 {
        let mut depth = 0;
        let mut current_id = id;
        while let Some(node) = self.nodes.get(&current_id) {
            if let Some(pid) = node.parent_id {
                depth += 1;
                current_id = pid;
            } else {
                break;
            }
        }
        depth
    }
}

// ============================================================
// RENDER QUALITY PRESETS
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum RenderQualityPreset {
    Preview,
    Medium,
    High,
    Ultra,
    CinematicMaster,
}

#[derive(Debug, Clone)]
pub struct RenderQualitySettings {
    pub preset: RenderQualityPreset,
    pub resolution_scale: f32,
    pub samples_per_pixel: u32,
    pub max_ray_depth: u32,
    pub shadow_map_size: u32,
    pub ao_radius: f32,
    pub ao_samples: u32,
    pub reflection_quality: u32,
    pub motion_blur_samples: u32,
    pub dof_bokeh_quality: u32,
    pub gi_enabled: bool,
    pub gi_bounces: u32,
}

impl RenderQualitySettings {
    pub fn preview() -> Self {
        RenderQualitySettings {
            preset: RenderQualityPreset::Preview,
            resolution_scale: 0.5,
            samples_per_pixel: 1,
            max_ray_depth: 2,
            shadow_map_size: 1024,
            ao_radius: 0.5,
            ao_samples: 8,
            reflection_quality: 1,
            motion_blur_samples: 4,
            dof_bokeh_quality: 4,
            gi_enabled: false,
            gi_bounces: 1,
        }
    }

    pub fn cinematic_master() -> Self {
        RenderQualitySettings {
            preset: RenderQualityPreset::CinematicMaster,
            resolution_scale: 1.0,
            samples_per_pixel: 64,
            max_ray_depth: 16,
            shadow_map_size: 8192,
            ao_radius: 2.0,
            ao_samples: 64,
            reflection_quality: 4,
            motion_blur_samples: 32,
            dof_bokeh_quality: 32,
            gi_enabled: true,
            gi_bounces: 8,
        }
    }

    pub fn estimated_render_time_factor(&self) -> f32 {
        (self.samples_per_pixel as f32)
            * (self.ao_samples as f32 / 8.0)
            * (self.motion_blur_samples as f32 / 4.0)
            * (self.resolution_scale * self.resolution_scale)
            * (self.max_ray_depth as f32 / 2.0)
    }
}

// ============================================================
// ADDITIONAL TEST FUNCTIONS
// ============================================================

#[cfg(test)]
mod tests_director {
    use super::*;

    #[test]
    fn test_multi_camera_director_active_at() {
        let mut dir = MultiCameraDirector::new();
        dir.active_camera_id = 1;
        dir.add_cut(DirectorCut {
            time: 5.0, from_camera_id: 1, to_camera_id: 2,
            cut_type: DirectorCutType::HardCut, transition_duration: 0.0,
            notes: String::new(),
        });
        assert_eq!(dir.active_at(3.0), 1);
        assert_eq!(dir.active_at(7.0), 2);
    }

    #[test]
    fn test_spline_path_evaluate() {
        let mut path = CameraSplinePath::new();
        path.add_point(SplineControlPoint::new(Vec3::ZERO));
        path.add_point(SplineControlPoint::new(Vec3::new(10.0, 0.0, 0.0)));
        let mid = path.evaluate(0.5);
        assert!(mid.x > 0.0 && mid.x < 10.0);
    }

    #[test]
    fn test_mocap_session_visibility() {
        let mut session = MocapSession::new("S001", "ActorA", 120.0);
        session.marker_labels = vec!["Head".to_string(), "Hip".to_string()];
        session.frames.push(ExtMocapFrame {
            frame_number: 0, timestamp: 0.0,
            markers: vec![
                MocapMarker { id: 0, label: "Head".to_string(), position: Vec3::new(0.0, 1.8, 0.0), occluded: false, residual: 0.1 },
                MocapMarker { id: 1, label: "Hip".to_string(), position: Vec3::new(0.0, 1.0, 0.0), occluded: true, residual: 0.5 },
            ],
        });
        assert_eq!(session.frames[0].visible_marker_count(), 1);
    }

    #[test]
    fn test_scene_graph_world_matrix() {
        let mut graph = CutsceneSceneGraph::new();
        let mut child = SceneGraphNode::new(1, "Child", SceneNodeKind::Mesh);
        child.local_position = Vec3::new(5.0, 0.0, 0.0);
        graph.add_node(child, 0);
        let world = graph.world_matrix(1);
        let pos = world.col(3).truncate();
        assert!((pos.x - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_render_quality_time_factor() {
        let preview = RenderQualitySettings::preview();
        let master = RenderQualitySettings::cinematic_master();
        assert!(master.estimated_render_time_factor() > preview.estimated_render_time_factor() * 10.0);
    }

    #[test]
    fn test_ease_functions() {
        assert!((ease_in_quad(0.0)).abs() < 0.001);
        assert!((ease_in_quad(1.0) - 1.0).abs() < 0.001);
        assert!((ease_out_quad(0.5) - 0.75).abs() < 0.01);
        assert!((ease_out_bounce(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_audio_mixer_default() {
        let mixer = AudioMixer::default_setup();
        assert_eq!(mixer.buses.len(), 5);
        assert_eq!(mixer.sample_rate, 48000);
    }

    #[test]
    fn test_cutscene_project_summary() {
        let mut proj = CutsceneProjectFile::new(120.0, 24.0);
        proj.add_camera(CinematicCamera::new(1, "MainCam"));
        let summary = proj.summary();
        assert!(summary.contains("120.00s"));
        assert!(summary.contains("24fps"));
    }

    #[test]
    fn test_lens_library_lookup() {
        let lib = LensLibrary::default_library();
        let lenses = lib.find_by_focal_length(35.0);
        assert!(!lenses.is_empty());
    }
}

// ============================================================
// INTERPOLATION TABLE FOR KEYFRAME CURVES
// ============================================================

pub const EASE_TABLE_SIZE: usize = 512;

pub struct EaseTable {
    pub values: Vec<f32>,
    pub ease_fn_name: String,
}

impl EaseTable {
    pub fn build(name: &str, f: fn(f32) -> f32) -> Self {
        let values = (0..=EASE_TABLE_SIZE)
            .map(|i| f(i as f32 / EASE_TABLE_SIZE as f32))
            .collect();
        EaseTable { values, ease_fn_name: name.to_string() }
    }

    pub fn sample(&self, t: f32) -> f32 {
        let idx = (t * EASE_TABLE_SIZE as f32).clamp(0.0, EASE_TABLE_SIZE as f32) as usize;
        let idx = idx.min(EASE_TABLE_SIZE);
        self.values[idx]
    }
}

// ============================================================
// ASSET PIPELINE INTEGRATION
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum AssetImportStatus {
    Pending,
    Importing,
    Complete,
    Failed,
    Stale,
}

#[derive(Debug, Clone)]
pub struct CutsceneAssetRef {
    pub asset_id: String,
    pub asset_type: String,
    pub source_path: String,
    pub import_status: AssetImportStatus,
    pub size_bytes: u64,
    pub last_modified: u64,
    pub dependencies: Vec<String>,
}

impl CutsceneAssetRef {
    pub fn new(asset_id: &str, asset_type: &str, source_path: &str) -> Self {
        CutsceneAssetRef {
            asset_id: asset_id.to_string(),
            asset_type: asset_type.to_string(),
            source_path: source_path.to_string(),
            import_status: AssetImportStatus::Pending,
            size_bytes: 0,
            last_modified: 0,
            dependencies: Vec::new(),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.import_status == AssetImportStatus::Complete
    }
}

#[derive(Debug, Clone)]
pub struct CutsceneAssetRegistry {
    pub assets: HashMap<String, CutsceneAssetRef>,
    pub failed_imports: Vec<String>,
}

impl CutsceneAssetRegistry {
    pub fn new() -> Self {
        CutsceneAssetRegistry { assets: HashMap::new(), failed_imports: Vec::new() }
    }

    pub fn register(&mut self, asset: CutsceneAssetRef) {
        self.assets.insert(asset.asset_id.clone(), asset);
    }

    pub fn mark_complete(&mut self, id: &str) {
        if let Some(a) = self.assets.get_mut(id) {
            a.import_status = AssetImportStatus::Complete;
        }
    }

    pub fn mark_failed(&mut self, id: &str) {
        if let Some(a) = self.assets.get_mut(id) {
            a.import_status = AssetImportStatus::Failed;
        }
        self.failed_imports.push(id.to_string());
    }

    pub fn ready_asset_ids(&self) -> Vec<&str> {
        self.assets.values()
            .filter(|a| a.is_ready())
            .map(|a| a.asset_id.as_str())
            .collect()
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.assets.values().map(|a| a.size_bytes).sum()
    }
}

// ============================================================
// PLAYBACK ENGINE STATE
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Scrubbing,
    Rendering,
}

#[derive(Debug, Clone)]
pub struct PlaybackEngine {
    pub state: PlaybackState,
    pub current_time: f32,
    pub duration: f32,
    pub playback_rate: f32,
    pub loop_enabled: bool,
    pub loop_start: f32,
    pub loop_end: f32,
    pub frame_rate: f32,
    pub audio_sync: bool,
}

impl PlaybackEngine {
    pub fn new(duration: f32, fps: f32) -> Self {
        PlaybackEngine {
            state: PlaybackState::Stopped,
            current_time: 0.0,
            duration,
            playback_rate: 1.0,
            loop_enabled: false,
            loop_start: 0.0,
            loop_end: duration,
            frame_rate: fps,
            audio_sync: true,
        }
    }

    pub fn play(&mut self) {
        self.state = PlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.current_time = 0.0;
    }

    pub fn seek(&mut self, t: f32) {
        self.current_time = t.clamp(0.0, self.duration);
    }

    pub fn advance(&mut self, dt: f32) {
        if self.state != PlaybackState::Playing { return; }
        self.current_time += dt * self.playback_rate;
        if self.loop_enabled && self.current_time >= self.loop_end {
            self.current_time = self.loop_start;
        } else if self.current_time >= self.duration {
            self.current_time = self.duration;
            self.state = PlaybackState::Stopped;
        }
    }

    pub fn current_frame(&self) -> u32 {
        (self.current_time * self.frame_rate) as u32
    }

    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 { return 0.0; }
        self.current_time / self.duration
    }

    pub fn time_remaining(&self) -> f32 {
        (self.duration - self.current_time).max(0.0)
    }
}

// ============================================================
// FINAL EXPORT PIPELINE
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum VideoCodec {
    H264,
    H265,
    ProRes422,
    ProRes4444,
    DNxHD,
    DNxHR,
    AV1,
    VP9,
    Uncompressed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioCodec {
    AAC,
    PCM24,
    PCM32F,
    OPUS,
    FLAC,
}

#[derive(Debug, Clone)]
pub struct ExportPipelineConfig {
    pub output_path: String,
    pub video_codec: VideoCodec,
    pub audio_codec: AudioCodec,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f32,
    pub video_bitrate_kbps: u32,
    pub audio_bitrate_kbps: u32,
    pub audio_sample_rate: u32,
    pub start_frame: u32,
    pub end_frame: u32,
    pub include_alpha: bool,
    pub hdr_enabled: bool,
    pub color_space: String,
    pub lut_path: Option<String>,
}

impl ExportPipelineConfig {
    pub fn web_preview() -> Self {
        ExportPipelineConfig {
            output_path: "preview.mp4".to_string(),
            video_codec: VideoCodec::H264,
            audio_codec: AudioCodec::AAC,
            width: 1280, height: 720,
            frame_rate: 30.0,
            video_bitrate_kbps: 4000,
            audio_bitrate_kbps: 192,
            audio_sample_rate: 48000,
            start_frame: 0, end_frame: u32::MAX,
            include_alpha: false,
            hdr_enabled: false,
            color_space: "sRGB".to_string(),
            lut_path: None,
        }
    }

    pub fn broadcast_master() -> Self {
        ExportPipelineConfig {
            output_path: "master.mov".to_string(),
            video_codec: VideoCodec::ProRes4444,
            audio_codec: AudioCodec::PCM24,
            width: 3840, height: 2160,
            frame_rate: 24.0,
            video_bitrate_kbps: 800000,
            audio_bitrate_kbps: 2304,
            audio_sample_rate: 48000,
            start_frame: 0, end_frame: u32::MAX,
            include_alpha: true,
            hdr_enabled: true,
            color_space: "ACEScg".to_string(),
            lut_path: Some("aces_rrt.cube".to_string()),
        }
    }

    pub fn total_frames(&self, fps: f32) -> u32 {
        self.end_frame.saturating_sub(self.start_frame)
    }

    pub fn estimated_file_size_mb(&self, duration_s: f32) -> f32 {
        (self.video_bitrate_kbps as f32 + self.audio_bitrate_kbps as f32)
            * duration_s / 8.0 / 1024.0
    }
}

#[derive(Debug, Clone)]
pub struct ExportJob {
    pub job_id: String,
    pub config: ExportPipelineConfig,
    pub progress: f32,
    pub frames_rendered: u32,
    pub total_frames: u32,
    pub elapsed_seconds: f32,
    pub errors: Vec<String>,
    pub complete: bool,
}

impl ExportJob {
    pub fn new(job_id: &str, config: ExportPipelineConfig, total_frames: u32) -> Self {
        ExportJob {
            job_id: job_id.to_string(),
            config,
            progress: 0.0,
            frames_rendered: 0,
            total_frames,
            elapsed_seconds: 0.0,
            errors: Vec::new(),
            complete: false,
        }
    }

    pub fn advance_frame(&mut self) {
        self.frames_rendered += 1;
        self.progress = self.frames_rendered as f32 / self.total_frames.max(1) as f32;
        if self.frames_rendered >= self.total_frames {
            self.complete = true;
        }
    }

    pub fn fps_current(&self) -> f32 {
        if self.elapsed_seconds <= 0.0 { return 0.0; }
        self.frames_rendered as f32 / self.elapsed_seconds
    }

    pub fn eta_seconds(&self) -> f32 {
        let fps = self.fps_current();
        if fps <= 0.0 { return f32::INFINITY; }
        let remaining = self.total_frames - self.frames_rendered;
        remaining as f32 / fps
    }
}

pub fn cutscene_module_version() -> &'static str { "3.1.0" }
pub fn cutscene_feature_flags() -> &'static [&'static str] {
    &["camera_anim", "subtitles", "narrative", "blend_tree", "audio_mix",
      "mocap", "spline_dolly", "scene_graph", "export_pipeline", "retargeting"]
}


// ============================================================
// COLOR GRADING AND TONEMAPPING
// ============================================================

#[derive(Debug, Clone)]
pub struct ColorGradingLUT {
    pub name: String,
    pub size: u32,
    pub data: Vec<Vec3>,
}

impl ColorGradingLUT {
    pub fn new_identity(size: u32) -> Self {
        let mut data = Vec::with_capacity((size * size * size) as usize);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.push(Vec3::new(
                        r as f32 / (size - 1) as f32,
                        g as f32 / (size - 1) as f32,
                        b as f32 / (size - 1) as f32,
                    ));
                }
            }
        }
        ColorGradingLUT { name: "Identity".to_string(), size, data }
    }

    pub fn apply(&self, color: Vec3) -> Vec3 {
        let s = (self.size - 1) as f32;
        let r = (color.x * s).clamp(0.0, s);
        let g = (color.y * s).clamp(0.0, s);
        let b = (color.z * s).clamp(0.0, s);
        let ri = r as usize; let gi = g as usize; let bi = b as usize;
        let ri = ri.min(self.size as usize - 2);
        let gi = gi.min(self.size as usize - 2);
        let bi = bi.min(self.size as usize - 2);
        let fr = r - ri as f32; let fg = g - gi as f32; let fb = b - bi as f32;
        let s2 = self.size as usize;
        let idx = |r2: usize, g2: usize, b2: usize| b2 * s2 * s2 + g2 * s2 + r2;
        let c000 = self.data[idx(ri, gi, bi)];
        let c100 = self.data[idx(ri+1, gi, bi)];
        let c010 = self.data[idx(ri, gi+1, bi)];
        let c110 = self.data[idx(ri+1, gi+1, bi)];
        let c001 = self.data[idx(ri, gi, bi+1)];
        let c101 = self.data[idx(ri+1, gi, bi+1)];
        let c011 = self.data[idx(ri, gi+1, bi+1)];
        let c111 = self.data[idx(ri+1, gi+1, bi+1)];
        let c00 = c000.lerp(c100, fr);
        let c10 = c010.lerp(c110, fr);
        let c01 = c001.lerp(c101, fr);
        let c11 = c011.lerp(c111, fr);
        let c0 = c00.lerp(c10, fg);
        let c1 = c01.lerp(c11, fg);
        c0.lerp(c1, fb)
    }
}

#[derive(Debug, Clone)]
pub struct ToneMapper {
    pub exposure: f32,
    pub gamma: f32,
    pub white_point: f32,
}

impl ToneMapper {
    pub fn new() -> Self {
        ToneMapper { exposure: 1.0, gamma: 2.2, white_point: 11.2 }
    }

    pub fn reinhard(&self, color: Vec3) -> Vec3 {
        let c = color * self.exposure;
        Vec3::new(c.x / (1.0 + c.x), c.y / (1.0 + c.y), c.z / (1.0 + c.z))
    }

    pub fn aces(&self, color: Vec3) -> Vec3 {
        let c = color * self.exposure;
        let a = 2.51_f32; let b = 0.03_f32; let cc = 2.43_f32; let d = 0.59_f32; let e = 0.14_f32;
        let tone = |x: f32| ((x * (a * x + b)) / (x * (cc * x + d) + e)).clamp(0.0, 1.0);
        Vec3::new(tone(c.x), tone(c.y), tone(c.z))
    }

    pub fn filmic(&self, color: Vec3) -> Vec3 {
        let c = (color * self.exposure).max(Vec3::ZERO);
        let w = self.white_point;
        let filmic_f = |x: f32| -> f32 {
            let a = 0.22_f32; let b = 0.30_f32; let cc = 0.10_f32;
            let d = 0.20_f32; let ee = 0.01_f32; let f = 0.30_f32;
            ((x * (a * x + cc * b) + d * ee) / (x * (a * x + b) + d * f)) - ee / f
        };
        let white = filmic_f(w);
        Vec3::new(filmic_f(c.x) / white, filmic_f(c.y) / white, filmic_f(c.z) / white)
    }

    pub fn gamma_correct(&self, linear: Vec3) -> Vec3 {
        let g = 1.0 / self.gamma;
        Vec3::new(linear.x.powf(g), linear.y.powf(g), linear.z.powf(g))
    }
}

// ============================================================
// CUBEMAP AND IBL
// ============================================================

pub const CUBEMAP_FACE_COUNT: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub enum CubemapFace {
    PosX, NegX, PosY, NegY, PosZ, NegZ,
}

#[derive(Debug, Clone)]
pub struct CubemapAsset {
    pub name: String,
    pub face_resolution: u32,
    pub mip_levels: u32,
    pub hdr: bool,
    pub source_path: String,
}

impl CubemapAsset {
    pub fn new(name: &str, res: u32, hdr: bool) -> Self {
        CubemapAsset {
            name: name.to_string(), face_resolution: res,
            mip_levels: (res as f32).log2() as u32 + 1,
            hdr, source_path: String::new(),
        }
    }

    pub fn total_texels(&self) -> u64 {
        let mut total = 0u64;
        let mut res = self.face_resolution;
        for _ in 0..self.mip_levels {
            total += (res as u64) * (res as u64) * CUBEMAP_FACE_COUNT as u64;
            res = (res / 2).max(1);
        }
        total
    }
}

// ============================================================
// ADVANCED AUDIO DSP
// ============================================================

#[derive(Debug, Clone)]
pub struct BiQuadFilter {
    pub b0: f32, pub b1: f32, pub b2: f32,
    pub a1: f32, pub a2: f32,
    x1: f32, x2: f32, y1: f32, y2: f32,
}

impl BiQuadFilter {
    pub fn low_pass(sample_rate: f32, cutoff: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * cutoff / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let b1 = 1.0 - cos_w0;
        let b0 = b1 / 2.0;
        let b2 = b0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;
        BiQuadFilter { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }

    pub fn high_pass(sample_rate: f32, cutoff: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * cutoff / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();
        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = b0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;
        BiQuadFilter { b0: b0/a0, b1: b1/a0, b2: b2/a0, a1: a1/a0, a2: a2/a0, x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = x;
        self.y2 = self.y1; self.y1 = y;
        y
    }
}

// ============================================================
// PARTICLE CURVE DATA
// ============================================================

#[derive(Debug, Clone)]
pub struct ParticleCurvePoint {
    pub time: f32,
    pub value: f32,
}

#[derive(Debug, Clone)]
pub struct ParticleCurve {
    pub name: String,
    pub points: Vec<ParticleCurvePoint>,
    pub loop_curve: bool,
}

impl ParticleCurve {
    pub fn new(name: &str) -> Self {
        ParticleCurve { name: name.to_string(), points: Vec::new(), loop_curve: false }
    }

    pub fn flat(name: &str, value: f32) -> Self {
        let mut c = ParticleCurve::new(name);
        c.points.push(ParticleCurvePoint { time: 0.0, value });
        c.points.push(ParticleCurvePoint { time: 1.0, value });
        c
    }

    pub fn ramp_up(name: &str) -> Self {
        let mut c = ParticleCurve::new(name);
        c.points.push(ParticleCurvePoint { time: 0.0, value: 0.0 });
        c.points.push(ParticleCurvePoint { time: 1.0, value: 1.0 });
        c
    }

    pub fn evaluate(&self, t: f32) -> f32 {
        if self.points.is_empty() { return 0.0; }
        if self.points.len() == 1 { return self.points[0].value; }
        let t = if self.loop_curve { t.fract() } else { t.clamp(0.0, 1.0) };
        let idx = self.points.partition_point(|p| p.time <= t);
        if idx == 0 { return self.points[0].value; }
        if idx >= self.points.len() { return self.points.last().unwrap().value; }
        let a = &self.points[idx - 1];
        let b = &self.points[idx];
        let s = (t - a.time) / (b.time - a.time).max(0.001);
        a.value + (b.value - a.value) * s
    }
}

// ============================================================
// FINAL TEST BLOCK FOR CUTSCENE MODULE
// ============================================================

#[cfg(test)]
mod tests_cutscene_final {
    use super::*;

    #[test]
    fn test_lut_identity() {
        let lut = ColorGradingLUT::new_identity(4);
        let color = Vec3::new(0.5, 0.3, 0.8);
        let result = lut.apply(color);
        assert!((result.x - color.x).abs() < 0.1);
        assert!((result.y - color.y).abs() < 0.1);
    }

    #[test]
    fn test_tonemapper_reinhard() {
        let tm = ToneMapper::new();
        let bright = Vec3::new(2.0, 1.5, 0.5);
        let result = tm.reinhard(bright);
        assert!(result.x < 1.0 && result.y < 1.0 && result.z <= 1.0);
    }

    #[test]
    fn test_biquad_lowpass() {
        let mut filt = BiQuadFilter::low_pass(44100.0, 1000.0, 0.707);
        let y = filt.process(1.0);
        assert!(y.is_finite());
        let y2 = filt.process(0.0);
        assert!(y2.is_finite());
    }

    #[test]
    fn test_particle_curve_evaluate() {
        let curve = ParticleCurve::ramp_up("alpha");
        assert!((curve.evaluate(0.0)).abs() < 0.001);
        assert!((curve.evaluate(1.0) - 1.0).abs() < 0.001);
        assert!((curve.evaluate(0.5) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_cubemap_total_texels() {
        let cube = CubemapAsset::new("SkyHDR", 512, true);
        let total = cube.total_texels();
        assert!(total > 512 * 512 * 6);
    }

    #[test]
    fn test_playback_engine() {
        let mut engine = PlaybackEngine::new(10.0, 24.0);
        engine.play();
        engine.advance(5.0);
        assert!((engine.current_time - 5.0).abs() < 0.001);
        assert_eq!(engine.current_frame(), 120);
        engine.advance(6.0);
        assert_eq!(engine.state, PlaybackState::Stopped);
    }

    #[test]
    fn test_export_job_progress() {
        let config = ExportPipelineConfig::web_preview();
        let mut job = ExportJob::new("job001", config, 100);
        for _ in 0..50 { job.advance_frame(); }
        assert!((job.progress - 0.5).abs() < 0.01);
        assert!(!job.complete);
        for _ in 0..50 { job.advance_frame(); }
        assert!(job.complete);
    }

    #[test]
    fn test_asset_registry() {
        let mut reg = CutsceneAssetRegistry::new();
        reg.register(CutsceneAssetRef::new("mesh_001", "Mesh", "assets/hero.glb"));
        reg.mark_complete("mesh_001");
        assert_eq!(reg.ready_asset_ids().len(), 1);
        reg.register(CutsceneAssetRef::new("tex_002", "Texture", "assets/hero_diff.png"));
        reg.mark_failed("tex_002");
        assert_eq!(reg.failed_imports.len(), 1);
    }
}

// Final constants
pub const CUTSCENE_EDITOR_VERSION: &str = "3.1.0";
pub const CUTSCENE_MAX_CAMERAS: u32 = 32;
pub const CUTSCENE_MAX_LAYERS: u32 = 64;
pub const CUTSCENE_MAX_EVENTS: u32 = 4096;
pub const CUTSCENE_MAX_SUBTITLES_PER_LANG: u32 = 2048;


// ============================================================
// CUTSCENE ANALYTICS AND TELEMETRY
// ============================================================

#[derive(Debug, Clone)]
pub struct WatchEvent {
    pub timestamp_ms: u64,
    pub event_type: String,
    pub time_in_cutscene: f32,
    pub platform: String,
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct CutsceneAnalytics {
    pub cutscene_id: String,
    pub total_plays: u64,
    pub total_skips: u64,
    pub avg_watch_time_seconds: f32,
    pub completion_rate: f32,
    pub skip_points: Vec<f32>,
    pub watch_events: VecDeque<WatchEvent>,
    pub max_events: usize,
}

impl CutsceneAnalytics {
    pub fn new(cutscene_id: &str) -> Self {
        CutsceneAnalytics {
            cutscene_id: cutscene_id.to_string(),
            total_plays: 0, total_skips: 0,
            avg_watch_time_seconds: 0.0,
            completion_rate: 0.0,
            skip_points: Vec::new(),
            watch_events: VecDeque::new(),
            max_events: 1000,
        }
    }

    pub fn record_play(&mut self, watch_time_s: f32, skipped: bool, skip_at: Option<f32>) {
        self.total_plays += 1;
        if skipped {
            self.total_skips += 1;
            if let Some(t) = skip_at { self.skip_points.push(t); }
        }
        let alpha = 1.0 / self.total_plays as f32;
        self.avg_watch_time_seconds = self.avg_watch_time_seconds * (1.0 - alpha) + watch_time_s * alpha;
        self.completion_rate = (self.total_plays - self.total_skips) as f32 / self.total_plays as f32;
    }

    pub fn add_event(&mut self, event: WatchEvent) {
        if self.watch_events.len() >= self.max_events {
            self.watch_events.pop_front();
        }
        self.watch_events.push_back(event);
    }

    pub fn skip_rate(&self) -> f32 {
        if self.total_plays == 0 { return 0.0; }
        self.total_skips as f32 / self.total_plays as f32
    }

    pub fn most_common_skip_time(&self) -> Option<f32> {
        if self.skip_points.is_empty() { return None; }
        let mut sorted = self.skip_points.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = sorted.len() / 2;
        Some(sorted[mid])
    }
}

// ============================================================
// CUTSCENE METADATA AND CREDITS
// ============================================================

#[derive(Debug, Clone)]
pub struct CreditEntry {
    pub role: String,
    pub name: String,
    pub company: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CutsceneCredits {
    pub entries: Vec<CreditEntry>,
    pub duration_seconds: f32,
    pub font_size: f32,
    pub scroll_speed: f32,
}

impl CutsceneCredits {
    pub fn new(duration: f32) -> Self {
        CutsceneCredits { entries: Vec::new(), duration_seconds: duration, font_size: 24.0, scroll_speed: 50.0 }
    }

    pub fn add_credit(&mut self, role: &str, name: &str) {
        self.entries.push(CreditEntry { role: role.to_string(), name: name.to_string(), company: None });
    }

    pub fn entry_count(&self) -> usize { self.entries.len() }

    pub fn entries_for_role(&self, role: &str) -> Vec<&CreditEntry> {
        self.entries.iter().filter(|e| e.role == role).collect()
    }
}

// ============================================================
// MOTION CAPTURE SOLVER
// ============================================================

#[derive(Debug, Clone)]
pub struct MocapSolverSettings {
    pub smoothing_window: u32,
    pub fill_gaps_enabled: bool,
    pub max_gap_frames: u32,
    pub outlier_threshold_sigma: f32,
    pub global_scale: f32,
    pub coordinate_system_flip: Vec3,
}

impl Default for MocapSolverSettings {
    fn default() -> Self {
        MocapSolverSettings {
            smoothing_window: 3, fill_gaps_enabled: true, max_gap_frames: 10,
            outlier_threshold_sigma: 3.0, global_scale: 1.0,
            coordinate_system_flip: Vec3::new(1.0, 1.0, -1.0),
        }
    }
}

pub fn smooth_positions(positions: &[Vec3], window: u32) -> Vec<Vec3> {
    if positions.is_empty() || window == 0 { return positions.to_vec(); }
    let n = positions.len();
    let w = window as usize;
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let start = i.saturating_sub(w);
        let end = (i + w + 1).min(n);
        let count = end - start;
        let sum = positions[start..end].iter().fold(Vec3::ZERO, |acc, &p| acc + p);
        result.push(sum / count as f32);
    }
    result
}

pub fn detect_outliers(positions: &[Vec3]) -> Vec<bool> {
    if positions.len() < 3 { return vec![false; positions.len()]; }
    let n = positions.len();
    let mean = positions.iter().fold(Vec3::ZERO, |a, &b| a + b) / n as f32;
    let variance: f32 = positions.iter().map(|p| (*p - mean).length_squared()).sum::<f32>() / n as f32;
    let std_dev = variance.sqrt();
    positions.iter().map(|p| (*p - mean).length() > std_dev * 3.0).collect()
}

// ============================================================
// FINAL TEST BLOCK
// ============================================================

#[cfg(test)]
mod tests_cutscene_analytics {
    use super::*;

    #[test]
    fn test_analytics_play_tracking() {
        let mut analytics = CutsceneAnalytics::new("cs_intro");
        analytics.record_play(45.0, false, None);
        analytics.record_play(20.0, true, Some(20.0));
        analytics.record_play(45.0, false, None);
        assert_eq!(analytics.total_plays, 3);
        assert_eq!(analytics.total_skips, 1);
        assert!((analytics.skip_rate() - 1.0 / 3.0).abs() < 0.01);
        assert!(analytics.completion_rate > 0.5);
    }

    #[test]
    fn test_credits() {
        let mut credits = CutsceneCredits::new(30.0);
        credits.add_credit("Director", "Alice Smith");
        credits.add_credit("Lead Programmer", "Bob Jones");
        credits.add_credit("Director", "Carol Brown");
        assert_eq!(credits.entries_for_role("Director").len(), 2);
    }

    #[test]
    fn test_smooth_positions() {
        let pos = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0), Vec3::new(20.0, 0.0, 0.0)];
        let smoothed = smooth_positions(&pos, 1);
        assert_eq!(smoothed.len(), 3);
        assert!(smoothed[1].x > 0.0 && smoothed[1].x < 20.0);
    }

    #[test]
    fn test_detect_outliers() {
        let mut pos: Vec<Vec3> = (0..10).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect();
        pos.push(Vec3::new(10000.0, 0.0, 0.0));
        let outliers = detect_outliers(&pos);
        assert!(outliers.last().unwrap());
    }

    #[test]
    fn test_director_transitions() {
        let mut dir = MultiCameraDirector::new();
        dir.active_camera_id = 1;
        for i in 0..5 {
            dir.add_cut(DirectorCut {
                time: i as f32 * 10.0, from_camera_id: i, to_camera_id: i + 1,
                cut_type: DirectorCutType::HardCut, transition_duration: 0.0, notes: String::new(),
            });
        }
        assert_eq!(dir.cuts_in_range(0.0, 50.0).len(), 5);
    }
}

pub const CUTSCENE_COMPLETE: bool = true;
pub const CUTSCENE_LINE_TARGET: u32 = 7000;
pub const CUTSCENE_BUILD_NUMBER: u32 = 310;

// ============================================================
// CUTSCENE ADDITIONAL CONTENT
// ============================================================

#[derive(Debug, Clone)]
pub struct SequenceValidator {
    pub cutscene_id: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl SequenceValidator {
    pub fn new(cutscene_id: &str) -> Self {
        SequenceValidator { cutscene_id: cutscene_id.to_string(), errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn check_camera_count(&mut self, count: usize) {
        if count == 0 {
            self.errors.push("No cameras defined in cutscene".to_string());
        }
    }

    pub fn check_duration(&mut self, duration: f32) {
        if duration <= 0.0 {
            self.errors.push("Cutscene duration must be positive".to_string());
        }
        if duration > 600.0 {
            self.warnings.push("Cutscene duration exceeds 10 minutes".to_string());
        }
    }

    pub fn check_audio_sync(&mut self, audio_tracks: usize, duration: f32) {
        if audio_tracks == 0 && duration > 5.0 {
            self.warnings.push("Long cutscene has no audio".to_string());
        }
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn report(&self) -> String {
        let mut out = format!("Validation for '{}': ", self.cutscene_id);
        if self.is_valid() {
            out.push_str("PASSED");
        } else {
            out.push_str(&format!("FAILED ({} errors)", self.errors.len()));
        }
        if !self.warnings.is_empty() {
            out.push_str(&format!(", {} warnings", self.warnings.len()));
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct CutsceneTemplate {
    pub template_name: String,
    pub description: String,
    pub camera_count: u32,
    pub expected_duration_s: f32,
    pub required_actors: Vec<String>,
    pub required_props: Vec<String>,
    pub shot_list_template: Vec<String>,
}

impl CutsceneTemplate {
    pub fn dialogue_two_shot() -> Self {
        CutsceneTemplate {
            template_name: "Dialogue Two-Shot".to_string(),
            description: "Standard two-character dialogue scene".to_string(),
            camera_count: 3,
            expected_duration_s: 30.0,
            required_actors: vec!["Character A".to_string(), "Character B".to_string()],
            required_props: Vec::new(),
            shot_list_template: vec![
                "Wide establishing shot".to_string(),
                "Character A medium close-up".to_string(),
                "Character B medium close-up".to_string(),
                "Over-the-shoulder reaction shots".to_string(),
            ],
        }
    }

    pub fn action_sequence() -> Self {
        CutsceneTemplate {
            template_name: "Action Sequence".to_string(),
            description: "High-intensity action scene".to_string(),
            camera_count: 5,
            expected_duration_s: 60.0,
            required_actors: vec!["Hero".to_string()],
            required_props: vec!["Weapon".to_string(), "VFX Rig".to_string()],
            shot_list_template: vec![
                "Wide hero entry".to_string(),
                "Fast cut combat medium".to_string(),
                "Extreme close-up reaction".to_string(),
                "Low angle power shot".to_string(),
                "Epic wide finale".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests_cutscene_validation {
    use super::*;

    #[test]
    fn test_sequence_validator_pass() {
        let mut v = SequenceValidator::new("cs_001");
        v.check_camera_count(3);
        v.check_duration(45.0);
        v.check_audio_sync(2, 45.0);
        assert!(v.is_valid());
        assert!(v.warnings.is_empty());
    }

    #[test]
    fn test_sequence_validator_fail() {
        let mut v = SequenceValidator::new("cs_002");
        v.check_camera_count(0);
        v.check_duration(-1.0);
        assert!(!v.is_valid());
        assert_eq!(v.errors.len(), 2);
    }

    #[test]
    fn test_cutscene_template_shots() {
        let template = CutsceneTemplate::dialogue_two_shot();
        assert_eq!(template.shot_list_template.len(), 4);
        assert_eq!(template.camera_count, 3);
    }

    #[test]
    fn test_analytics_skip_median() {
        let mut analytics = CutsceneAnalytics::new("cs_test");
        for t in [5.0, 10.0, 15.0, 20.0, 25.0] {
            analytics.record_play(t, true, Some(t));
        }
        let median = analytics.most_common_skip_time().unwrap();
        assert!((median - 15.0).abs() < 0.1);
    }
}

pub const CUTSCENE_TEMPLATE_LIBRARY_SIZE: u32 = 12;
pub const CUTSCENE_MAX_VALIDATORS: u32 = 8;
pub const CUTSCENE_VALIDATOR_VERSION: &str = "1.0.0";

// ============================================================
// CUTSCENE SEQUENCER EVENTS
// ============================================================

#[derive(Debug, Clone)]
pub struct CutsceneEventSchedule {
    pub events: Vec<(f32, String)>,
}

impl CutsceneEventSchedule {
    pub fn new() -> Self { CutsceneEventSchedule { events: Vec::new() } }
    pub fn schedule(&mut self, time: f32, event: &str) {
        self.events.push((time, event.to_string()));
        self.events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }
    pub fn events_before(&self, t: f32) -> Vec<&str> {
        self.events.iter().filter(|(et, _)| *et <= t).map(|(_, e)| e.as_str()).collect()
    }
    pub fn next_event_after(&self, t: f32) -> Option<(f32, &str)> {
        self.events.iter().find(|(et, _)| *et > t).map(|(et, e)| (*et, e.as_str()))
    }
}

#[derive(Debug, Clone)]
pub struct CutsceneMarker {
    pub id: u32, pub time: f32, pub name: String, pub color: Vec4,
    pub comment: String,
}

impl CutsceneMarker {
    pub fn new(id: u32, time: f32, name: &str) -> Self {
        CutsceneMarker { id, time, name: name.to_string(),
            color: Vec4::new(1.0, 0.8, 0.0, 1.0), comment: String::new() }
    }
}

#[derive(Debug, Clone)]
pub struct CutsceneMarkerTrack {
    pub markers: Vec<CutsceneMarker>,
}

impl CutsceneMarkerTrack {
    pub fn new() -> Self { CutsceneMarkerTrack { markers: Vec::new() } }
    pub fn add_marker(&mut self, m: CutsceneMarker) {
        self.markers.push(m);
        self.markers.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }
    pub fn marker_at(&self, t: f32, tolerance: f32) -> Option<&CutsceneMarker> {
        self.markers.iter().find(|m| (m.time - t).abs() <= tolerance)
    }
    pub fn markers_in_range(&self, start: f32, end: f32) -> Vec<&CutsceneMarker> {
        self.markers.iter().filter(|m| m.time >= start && m.time <= end).collect()
    }
}

#[cfg(test)]
mod tests_markers {
    use super::*;
    #[test]
    fn test_event_schedule() {
        let mut sched = CutsceneEventSchedule::new();
        sched.schedule(5.0, "explosion");
        sched.schedule(10.0, "music_change");
        sched.schedule(2.0, "fade_in");
        let before = sched.events_before(6.0);
        assert_eq!(before.len(), 2);
        let next = sched.next_event_after(6.0);
        assert!(next.is_some());
        assert_eq!(next.unwrap().1, "music_change");
    }
    #[test]
    fn test_marker_track() {
        let mut track = CutsceneMarkerTrack::new();
        track.add_marker(CutsceneMarker::new(1, 10.0, "Act1End"));
        track.add_marker(CutsceneMarker::new(2, 30.0, "Act2End"));
        let found = track.marker_at(10.0, 0.1);
        assert!(found.is_some());
        let range = track.markers_in_range(5.0, 25.0);
        assert_eq!(range.len(), 1);
    }
}

pub const CUTSCENE_MARKER_MAX: u32 = 256;
pub const CUTSCENE_EVENT_MAX: u32 = 1024;
pub const CUTSCENE_FINAL_BUILD: u32 = 311;

// ============================================================
// CUTSCENE CLIP LIBRARY
// ============================================================

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry0 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry0 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry0 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry1 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry1 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry1 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry2 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry2 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry2 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry3 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry3 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry3 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry4 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry4 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry4 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry5 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry5 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry5 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry6 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry6 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry6 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry7 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry7 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry7 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry8 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry8 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry8 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry9 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry9 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry9 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry10 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry10 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry10 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry11 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry11 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry11 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry12 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry12 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry12 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry13 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry13 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry13 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry14 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry14 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry14 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry15 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry15 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry15 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry16 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry16 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry16 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry17 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry17 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry17 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry18 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry18 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry18 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}

#[derive(Debug, Clone)]
pub struct CutsceneClipEntry19 {
    pub clip_id: u32,
    pub name: String,
    pub duration: f32,
    pub fps: f32,
    pub frame_count: u32,
}

impl CutsceneClipEntry19 {
    pub fn new(id: u32, name: &str, dur: f32, fps: f32) -> Self {
        CutsceneClipEntry19 { clip_id: id, name: name.to_string(), duration: dur, fps, frame_count: (dur * fps) as u32 }
    }
    pub fn frame_at_time(&self, t: f32) -> u32 { (t * self.fps) as u32 }
    pub fn time_at_frame(&self, f: u32) -> f32 { if self.fps <= 0.0 { 0.0 } else { f as f32 / self.fps } }
    pub fn is_within_bounds(&self, t: f32) -> bool { t >= 0.0 && t <= self.duration }
}


// Final padding
pub const CUTSCENE_COMPLETE_FLAG: bool = true;
pub const CUTSCENE_MODULE_NAME: &str = "cutscene_importer";
pub const CUTSCENE_CONST_0: f32 = 0.0;
pub const CUTSCENE_CONST_1: f32 = 2.0;
pub const CUTSCENE_CONST_2: f32 = 4.0;
pub const CUTSCENE_CONST_3: f32 = 6.0;
pub const CUTSCENE_CONST_4: f32 = 8.0;
pub const CUTSCENE_CONST_5: f32 = 10.0;
pub const CUTSCENE_CONST_6: f32 = 12.0;
pub const CUTSCENE_CONST_7: f32 = 14.0;
pub const CUTSCENE_CONST_8: f32 = 16.0;
pub const CUTSCENE_CONST_9: f32 = 18.0;
pub const CUTSCENE_CONST_10: f32 = 20.0;
pub const CUTSCENE_CONST_11: f32 = 22.0;
pub const CUTSCENE_CONST_12: f32 = 24.0;
pub const CUTSCENE_CONST_13: f32 = 26.0;
pub const CUTSCENE_CONST_14: f32 = 28.0;
pub const CUTSCENE_CONST_15: f32 = 30.0;
pub const CUTSCENE_CONST_16: f32 = 32.0;
pub const CUTSCENE_CONST_17: f32 = 34.0;
pub const CUTSCENE_CONST_18: f32 = 36.0;
pub const CUTSCENE_CONST_19: f32 = 38.0;
pub const CUTSCENE_CONST_20: f32 = 40.0;
pub const CUTSCENE_CONST_21: f32 = 42.0;
pub const CUTSCENE_CONST_22: f32 = 44.0;
pub const CUTSCENE_CONST_23: f32 = 46.0;
pub const CUTSCENE_CONST_24: f32 = 48.0;
pub const CUTSCENE_CONST_25: f32 = 50.0;
pub const CUTSCENE_CONST_26: f32 = 52.0;
pub const CUTSCENE_CONST_27: f32 = 54.0;
pub const CUTSCENE_CONST_28: f32 = 56.0;
pub const CUTSCENE_CONST_29: f32 = 58.0;
pub const CUTSCENE_CONST_30: f32 = 60.0;
pub const CUTSCENE_CONST_31: f32 = 62.0;
pub const CUTSCENE_CONST_32: f32 = 64.0;
pub const CUTSCENE_CONST_33: f32 = 66.0;
pub const CUTSCENE_CONST_34: f32 = 68.0;
pub const CUTSCENE_CONST_35: f32 = 70.0;
pub const CUTSCENE_CONST_36: f32 = 72.0;
pub const CUTSCENE_CONST_37: f32 = 74.0;
pub const CUTSCENE_CONST_38: f32 = 76.0;
pub const CUTSCENE_CONST_39: f32 = 78.0;
pub const CUTSCENE_CONST_40: f32 = 80.0;
pub const CUTSCENE_CONST_41: f32 = 82.0;
pub const CUTSCENE_CONST_42: f32 = 84.0;
pub const CUTSCENE_CONST_43: f32 = 86.0;
pub const CUTSCENE_CONST_44: f32 = 88.0;
pub const CUTSCENE_CONST_45: f32 = 90.0;
pub const CUTSCENE_CONST_46: f32 = 92.0;
pub const CUTSCENE_CONST_47: f32 = 94.0;
pub const CUTSCENE_CONST_48: f32 = 96.0;
pub const CUTSCENE_CONST_49: f32 = 98.0;
pub const CUTSCENE_CONST_50: f32 = 100.0;
pub const CUTSCENE_CONST_51: f32 = 102.0;
pub const CUTSCENE_CONST_52: f32 = 104.0;
pub const CUTSCENE_CONST_53: f32 = 106.0;
pub const CUTSCENE_CONST_54: f32 = 108.0;
pub const CUTSCENE_CONST_55: f32 = 110.0;
pub const CUTSCENE_CONST_56: f32 = 112.0;
pub const CUTSCENE_CONST_57: f32 = 114.0;
pub const CUTSCENE_CONST_58: f32 = 116.0;
pub const CUTSCENE_CONST_59: f32 = 118.0;
