#[allow(dead_code, unused_variables, unused_mut, unused_imports)]

use glam::{Vec2, Vec3, Vec4, Quat, Mat4};
use std::collections::{HashMap, VecDeque, HashSet, BTreeMap};

// ============================================================
// SECTION 1: TOKEN TYPES
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    StringLit(String),
    // Identifiers & keywords
    Ident(String),
    // Types
    KwFloat, KwInt, KwUint, KwBool,
    KwVec2, KwVec3, KwVec4,
    KwIVec2, KwIVec3, KwIVec4,
    KwUVec2, KwUVec3, KwUVec4,
    KwMat2, KwMat3, KwMat4,
    KwSampler2D, KwSampler3D, KwSamplerCube,
    KwVoid,
    // Keywords
    KwIf, KwElse, KwFor, KwWhile, KwDo, KwReturn, KwBreak, KwContinue,
    KwStruct, KwIn, KwOut, KwInout, KwUniform, KwConst, KwLayout,
    KwAttribute, KwVarying, KwPrecision, KwHighp, KwMediump, KwLowp,
    KwDiscard,
    // Operators
    Plus, Minus, Star, Slash, Percent,
    PlusEq, MinusEq, StarEq, SlashEq, PercentEq,
    PlusPlus, MinusMinus,
    Amp, Pipe, Caret, Tilde, LShift, RShift,
    AmpEq, PipeEq, CaretEq, LShiftEq, RShiftEq,
    Eq, EqEq, BangEq,
    Lt, Gt, LtEq, GtEq,
    AmpAmp, PipePipe, Bang,
    Question, Colon,
    Dot, Arrow,
    Semicolon, Comma,
    LParen, RParen,
    LBrace, RBrace,
    LBracket, RBracket,
    Hash,
    // Special
    Eof,
    Newline,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: u32,
    pub col: u32,
    pub source: String,
}

impl Token {
    pub fn new(kind: TokenKind, line: u32, col: u32, source: &str) -> Self {
        Self { kind, line, col, source: source.to_string() }
    }
    pub fn is_type_keyword(&self) -> bool {
        matches!(self.kind,
            TokenKind::KwFloat | TokenKind::KwInt | TokenKind::KwUint | TokenKind::KwBool |
            TokenKind::KwVec2 | TokenKind::KwVec3 | TokenKind::KwVec4 |
            TokenKind::KwMat2 | TokenKind::KwMat3 | TokenKind::KwMat4 |
            TokenKind::KwSampler2D | TokenKind::KwSampler3D | TokenKind::KwSamplerCube |
            TokenKind::KwVoid)
    }
}

// ============================================================
// SECTION 2: LEXER / TOKENIZER
// ============================================================

pub struct Lexer {
    pub source: Vec<char>,
    pub pos: usize,
    pub line: u32,
    pub col: u32,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self { source: source.chars().collect(), pos: 0, line: 1, col: 1 }
    }

    fn peek(&self) -> Option<char> { self.source.get(self.pos).copied() }
    fn peek2(&self) -> Option<char> { self.source.get(self.pos + 1).copied() }

    fn advance(&mut self) -> Option<char> {
        let c = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' { self.line += 1; self.col = 1; } else { self.col += 1; }
        Some(c)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\r' { self.advance(); }
            else if c == '\n' { self.advance(); }
            else if c == '/' && self.peek2() == Some('/') {
                while let Some(c) = self.peek() { if c == '\n' { break; } self.advance(); }
            } else if c == '/' && self.peek2() == Some('*') {
                self.advance(); self.advance();
                loop {
                    match self.advance() {
                        None => break,
                        Some('*') if self.peek() == Some('/') => { self.advance(); break; }
                        _ => {}
                    }
                }
            } else { break; }
        }
    }

    fn read_ident(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' { s.push(c); self.advance(); } else { break; }
        }
        s
    }

    fn read_number(&mut self) -> TokenKind {
        let mut s = String::new();
        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() { s.push(c); self.advance(); }
            else if c == '.' && !is_float {
                is_float = true; s.push(c); self.advance();
            } else if (c == 'e' || c == 'E') && is_float {
                s.push(c); self.advance();
                if let Some('+') | Some('-') = self.peek() {
                    s.push(self.advance().unwrap());
                }
            } else if c == 'f' || c == 'u' { self.advance(); break; }
            else { break; }
        }
        if is_float {
            TokenKind::FloatLit(s.parse().unwrap_or(0.0))
        } else {
            if s.starts_with("0x") || s.starts_with("0X") {
                TokenKind::IntLit(i64::from_str_radix(&s[2..], 16).unwrap_or(0))
            } else {
                TokenKind::IntLit(s.parse().unwrap_or(0))
            }
        }
    }

    fn ident_to_keyword(s: &str) -> TokenKind {
        match s {
            "float" => TokenKind::KwFloat,
            "int" => TokenKind::KwInt,
            "uint" => TokenKind::KwUint,
            "bool" => TokenKind::KwBool,
            "vec2" => TokenKind::KwVec2,
            "vec3" => TokenKind::KwVec3,
            "vec4" => TokenKind::KwVec4,
            "ivec2" => TokenKind::KwIVec2,
            "ivec3" => TokenKind::KwIVec3,
            "ivec4" => TokenKind::KwIVec4,
            "uvec2" => TokenKind::KwUVec2,
            "uvec3" => TokenKind::KwUVec3,
            "uvec4" => TokenKind::KwUVec4,
            "mat2" => TokenKind::KwMat2,
            "mat3" => TokenKind::KwMat3,
            "mat4" => TokenKind::KwMat4,
            "sampler2D" => TokenKind::KwSampler2D,
            "sampler3D" => TokenKind::KwSampler3D,
            "samplerCube" => TokenKind::KwSamplerCube,
            "void" => TokenKind::KwVoid,
            "if" => TokenKind::KwIf,
            "else" => TokenKind::KwElse,
            "for" => TokenKind::KwFor,
            "while" => TokenKind::KwWhile,
            "do" => TokenKind::KwDo,
            "return" => TokenKind::KwReturn,
            "break" => TokenKind::KwBreak,
            "continue" => TokenKind::KwContinue,
            "struct" => TokenKind::KwStruct,
            "in" => TokenKind::KwIn,
            "out" => TokenKind::KwOut,
            "inout" => TokenKind::KwInout,
            "uniform" => TokenKind::KwUniform,
            "const" => TokenKind::KwConst,
            "layout" => TokenKind::KwLayout,
            "attribute" => TokenKind::KwAttribute,
            "varying" => TokenKind::KwVarying,
            "precision" => TokenKind::KwPrecision,
            "highp" => TokenKind::KwHighp,
            "mediump" => TokenKind::KwMediump,
            "lowp" => TokenKind::KwLowp,
            "discard" => TokenKind::KwDiscard,
            "true" => TokenKind::BoolLit(true),
            "false" => TokenKind::BoolLit(false),
            other => TokenKind::Ident(other.to_string()),
        }
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            let line = self.line;
            let col = self.col;
            match self.peek() {
                None => { tokens.push(Token::new(TokenKind::Eof, line, col, "")); break; }
                Some(c) => {
                    let tok = match c {
                        'a'..='z' | 'A'..='Z' | '_' => {
                            let s = self.read_ident();
                            let kind = Self::ident_to_keyword(&s);
                            Token::new(kind, line, col, &s)
                        }
                        '0'..='9' => {
                            let kind = self.read_number();
                            Token::new(kind, line, col, "")
                        }
                        '+' => {
                            self.advance();
                            if self.peek() == Some('+') { self.advance(); Token::new(TokenKind::PlusPlus, line, col, "++") }
                            else if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::PlusEq, line, col, "+=") }
                            else { Token::new(TokenKind::Plus, line, col, "+") }
                        }
                        '-' => {
                            self.advance();
                            if self.peek() == Some('-') { self.advance(); Token::new(TokenKind::MinusMinus, line, col, "--") }
                            else if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::MinusEq, line, col, "-=") }
                            else if self.peek() == Some('>') { self.advance(); Token::new(TokenKind::Arrow, line, col, "->") }
                            else { Token::new(TokenKind::Minus, line, col, "-") }
                        }
                        '*' => {
                            self.advance();
                            if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::StarEq, line, col, "*=") }
                            else { Token::new(TokenKind::Star, line, col, "*") }
                        }
                        '/' => {
                            self.advance();
                            if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::SlashEq, line, col, "/=") }
                            else { Token::new(TokenKind::Slash, line, col, "/") }
                        }
                        '%' => {
                            self.advance();
                            if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::PercentEq, line, col, "%=") }
                            else { Token::new(TokenKind::Percent, line, col, "%") }
                        }
                        '=' => {
                            self.advance();
                            if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::EqEq, line, col, "==") }
                            else { Token::new(TokenKind::Eq, line, col, "=") }
                        }
                        '!' => {
                            self.advance();
                            if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::BangEq, line, col, "!=") }
                            else { Token::new(TokenKind::Bang, line, col, "!") }
                        }
                        '<' => {
                            self.advance();
                            if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::LtEq, line, col, "<=") }
                            else if self.peek() == Some('<') {
                                self.advance();
                                if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::LShiftEq, line, col, "<<=") }
                                else { Token::new(TokenKind::LShift, line, col, "<<") }
                            }
                            else { Token::new(TokenKind::Lt, line, col, "<") }
                        }
                        '>' => {
                            self.advance();
                            if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::GtEq, line, col, ">=") }
                            else if self.peek() == Some('>') {
                                self.advance();
                                if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::RShiftEq, line, col, ">>=") }
                                else { Token::new(TokenKind::RShift, line, col, ">>") }
                            }
                            else { Token::new(TokenKind::Gt, line, col, ">") }
                        }
                        '&' => {
                            self.advance();
                            if self.peek() == Some('&') { self.advance(); Token::new(TokenKind::AmpAmp, line, col, "&&") }
                            else if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::AmpEq, line, col, "&=") }
                            else { Token::new(TokenKind::Amp, line, col, "&") }
                        }
                        '|' => {
                            self.advance();
                            if self.peek() == Some('|') { self.advance(); Token::new(TokenKind::PipePipe, line, col, "||") }
                            else if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::PipeEq, line, col, "|=") }
                            else { Token::new(TokenKind::Pipe, line, col, "|") }
                        }
                        '^' => {
                            self.advance();
                            if self.peek() == Some('=') { self.advance(); Token::new(TokenKind::CaretEq, line, col, "^=") }
                            else { Token::new(TokenKind::Caret, line, col, "^") }
                        }
                        '~' => { self.advance(); Token::new(TokenKind::Tilde, line, col, "~") }
                        '?' => { self.advance(); Token::new(TokenKind::Question, line, col, "?") }
                        ':' => { self.advance(); Token::new(TokenKind::Colon, line, col, ":") }
                        '.' => { self.advance(); Token::new(TokenKind::Dot, line, col, ".") }
                        ';' => { self.advance(); Token::new(TokenKind::Semicolon, line, col, ";") }
                        ',' => { self.advance(); Token::new(TokenKind::Comma, line, col, ",") }
                        '(' => { self.advance(); Token::new(TokenKind::LParen, line, col, "(") }
                        ')' => { self.advance(); Token::new(TokenKind::RParen, line, col, ")") }
                        '{' => { self.advance(); Token::new(TokenKind::LBrace, line, col, "{") }
                        '}' => { self.advance(); Token::new(TokenKind::RBrace, line, col, "}") }
                        '[' => { self.advance(); Token::new(TokenKind::LBracket, line, col, "[") }
                        ']' => { self.advance(); Token::new(TokenKind::RBracket, line, col, "]") }
                        '#' => { self.advance(); Token::new(TokenKind::Hash, line, col, "#") }
                        _ => { self.advance(); continue; }
                    };
                    tokens.push(tok);
                }
            }
        }
        tokens
    }
}

// ============================================================
// SECTION 3: TYPE SYSTEM
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShaderType {
    Void,
    Bool,
    Int,
    Uint,
    Float,
    BVec2, BVec3, BVec4,
    IVec2, IVec3, IVec4,
    UVec2, UVec3, UVec4,
    Vec2, Vec3, Vec4,
    Mat2, Mat3, Mat4,
    Sampler2D, Sampler3D, SamplerCube,
    Struct(String),
    Array(Box<ShaderType>, Option<u32>), // type, optional size
    Function(Vec<ShaderType>, Box<ShaderType>), // params, return
    Unknown,
}

impl ShaderType {
    pub fn is_scalar(&self) -> bool {
        matches!(self, ShaderType::Bool | ShaderType::Int | ShaderType::Uint | ShaderType::Float)
    }
    pub fn is_vector(&self) -> bool {
        matches!(self,
            ShaderType::BVec2|ShaderType::BVec3|ShaderType::BVec4|
            ShaderType::IVec2|ShaderType::IVec3|ShaderType::IVec4|
            ShaderType::UVec2|ShaderType::UVec3|ShaderType::UVec4|
            ShaderType::Vec2|ShaderType::Vec3|ShaderType::Vec4)
    }
    pub fn is_matrix(&self) -> bool {
        matches!(self, ShaderType::Mat2|ShaderType::Mat3|ShaderType::Mat4)
    }
    pub fn is_numeric(&self) -> bool {
        matches!(self,
            ShaderType::Int|ShaderType::Uint|ShaderType::Float|
            ShaderType::IVec2|ShaderType::IVec3|ShaderType::IVec4|
            ShaderType::UVec2|ShaderType::UVec3|ShaderType::UVec4|
            ShaderType::Vec2|ShaderType::Vec3|ShaderType::Vec4|
            ShaderType::Mat2|ShaderType::Mat3|ShaderType::Mat4)
    }
    pub fn component_count(&self) -> u32 {
        match self {
            ShaderType::Float|ShaderType::Int|ShaderType::Uint|ShaderType::Bool => 1,
            ShaderType::Vec2|ShaderType::IVec2|ShaderType::UVec2|ShaderType::BVec2 => 2,
            ShaderType::Vec3|ShaderType::IVec3|ShaderType::UVec3|ShaderType::BVec3 => 3,
            ShaderType::Vec4|ShaderType::IVec4|ShaderType::UVec4|ShaderType::BVec4 => 4,
            ShaderType::Mat2 => 4,
            ShaderType::Mat3 => 9,
            ShaderType::Mat4 => 16,
            _ => 0,
        }
    }
    pub fn base_scalar(&self) -> ShaderType {
        match self {
            ShaderType::Vec2|ShaderType::Vec3|ShaderType::Vec4 => ShaderType::Float,
            ShaderType::IVec2|ShaderType::IVec3|ShaderType::IVec4 => ShaderType::Int,
            ShaderType::UVec2|ShaderType::UVec3|ShaderType::UVec4 => ShaderType::Uint,
            ShaderType::BVec2|ShaderType::BVec3|ShaderType::BVec4 => ShaderType::Bool,
            ShaderType::Mat2|ShaderType::Mat3|ShaderType::Mat4 => ShaderType::Float,
            other => other.clone(),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            ShaderType::Void => "void",
            ShaderType::Bool => "bool",
            ShaderType::Int => "int",
            ShaderType::Uint => "uint",
            ShaderType::Float => "float",
            ShaderType::BVec2 => "bvec2",
            ShaderType::BVec3 => "bvec3",
            ShaderType::BVec4 => "bvec4",
            ShaderType::IVec2 => "ivec2",
            ShaderType::IVec3 => "ivec3",
            ShaderType::IVec4 => "ivec4",
            ShaderType::UVec2 => "uvec2",
            ShaderType::UVec3 => "uvec3",
            ShaderType::UVec4 => "uvec4",
            ShaderType::Vec2 => "vec2",
            ShaderType::Vec3 => "vec3",
            ShaderType::Vec4 => "vec4",
            ShaderType::Mat2 => "mat2",
            ShaderType::Mat3 => "mat3",
            ShaderType::Mat4 => "mat4",
            ShaderType::Sampler2D => "sampler2D",
            ShaderType::Sampler3D => "sampler3D",
            ShaderType::SamplerCube => "samplerCube",
            ShaderType::Struct(n) => n.as_str(),
            ShaderType::Array(_, _) => "array",
            ShaderType::Function(_, _) => "function",
            ShaderType::Unknown => "unknown",
        }
    }
    /// Can we implicitly convert `from` to `self`?
    pub fn can_implicit_convert(from: &ShaderType, to: &ShaderType) -> bool {
        if from == to { return true; }
        matches!((from, to),
            (ShaderType::Int, ShaderType::Float) |
            (ShaderType::Uint, ShaderType::Float) |
            (ShaderType::Int, ShaderType::Uint) |
            (ShaderType::Bool, ShaderType::Int) |
            (ShaderType::Bool, ShaderType::Float) |
            (ShaderType::IVec2, ShaderType::Vec2) |
            (ShaderType::IVec3, ShaderType::Vec3) |
            (ShaderType::IVec4, ShaderType::Vec4)
        )
    }
    pub fn from_token(t: &TokenKind) -> Option<ShaderType> {
        match t {
            TokenKind::KwFloat => Some(ShaderType::Float),
            TokenKind::KwInt => Some(ShaderType::Int),
            TokenKind::KwUint => Some(ShaderType::Uint),
            TokenKind::KwBool => Some(ShaderType::Bool),
            TokenKind::KwVec2 => Some(ShaderType::Vec2),
            TokenKind::KwVec3 => Some(ShaderType::Vec3),
            TokenKind::KwVec4 => Some(ShaderType::Vec4),
            TokenKind::KwIVec2 => Some(ShaderType::IVec2),
            TokenKind::KwIVec3 => Some(ShaderType::IVec3),
            TokenKind::KwIVec4 => Some(ShaderType::IVec4),
            TokenKind::KwUVec2 => Some(ShaderType::UVec2),
            TokenKind::KwUVec3 => Some(ShaderType::UVec3),
            TokenKind::KwUVec4 => Some(ShaderType::UVec4),
            TokenKind::KwMat2 => Some(ShaderType::Mat2),
            TokenKind::KwMat3 => Some(ShaderType::Mat3),
            TokenKind::KwMat4 => Some(ShaderType::Mat4),
            TokenKind::KwSampler2D => Some(ShaderType::Sampler2D),
            TokenKind::KwSampler3D => Some(ShaderType::Sampler3D),
            TokenKind::KwSamplerCube => Some(ShaderType::SamplerCube),
            TokenKind::KwVoid => Some(ShaderType::Void),
            _ => None,
        }
    }
}

// ============================================================
// SECTION 4: AST NODES
// ============================================================

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    Ident(String),
    Unary { op: UnaryOp, operand: Box<Expr> },
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
    Assign { op: AssignOp, target: Box<Expr>, value: Box<Expr> },
    Ternary { cond: Box<Expr>, then_expr: Box<Expr>, else_expr: Box<Expr> },
    Call { function: String, args: Vec<Expr> },
    Index { array: Box<Expr>, index: Box<Expr> },
    Field { object: Box<Expr>, field: String },
    Swizzle { object: Box<Expr>, components: String },
    Cast { target_type: ShaderType, expr: Box<Expr> },
    Construction { ty: ShaderType, args: Vec<Expr> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Negate,
    Not,
    BitNot,
    PreIncrement,
    PreDecrement,
    PostIncrement,
    PostDecrement,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignOp {
    Assign, AddAssign, SubAssign, MulAssign, DivAssign, ModAssign,
    AndAssign, OrAssign, XorAssign, ShlAssign, ShrAssign,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Expr(Expr),
    Block(Vec<Stmt>),
    VarDecl { ty: ShaderType, name: String, qualifier: Option<VarQualifier>, init: Option<Expr> },
    If { cond: Expr, then_body: Box<Stmt>, else_body: Option<Box<Stmt>> },
    For { init: Option<Box<Stmt>>, cond: Option<Expr>, step: Option<Expr>, body: Box<Stmt> },
    While { cond: Expr, body: Box<Stmt> },
    DoWhile { body: Box<Stmt>, cond: Expr },
    Return(Option<Expr>),
    Break,
    Continue,
    Discard,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VarQualifier {
    In, Out, Inout, Uniform, Const, Attribute, Varying,
}

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub ty: ShaderType,
    pub name: String,
    pub qualifier: Option<VarQualifier>,
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub return_type: ShaderType,
    pub name: String,
    pub params: Vec<FunctionParam>,
    pub body: Vec<Stmt>,
    pub is_builtin: bool,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub ty: ShaderType,
    pub name: String,
    pub array_size: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone)]
pub struct UniformDecl {
    pub ty: ShaderType,
    pub name: String,
    pub binding: Option<u32>,
    pub set: Option<u32>,
    pub array_size: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct AttributeDecl {
    pub ty: ShaderType,
    pub name: String,
    pub location: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ShaderAst {
    pub version: u32,
    pub extensions: Vec<String>,
    pub structs: Vec<StructDecl>,
    pub uniforms: Vec<UniformDecl>,
    pub attributes: Vec<AttributeDecl>,
    pub varyings: Vec<AttributeDecl>,
    pub functions: Vec<FunctionDecl>,
    pub global_vars: Vec<Stmt>,
    pub defines: HashMap<String, String>,
}

impl ShaderAst {
    pub fn new() -> Self {
        Self {
            version: 450,
            extensions: Vec::new(),
            structs: Vec::new(),
            uniforms: Vec::new(),
            attributes: Vec::new(),
            varyings: Vec::new(),
            functions: Vec::new(),
            global_vars: Vec::new(),
            defines: HashMap::new(),
        }
    }
    pub fn find_function(&self, name: &str) -> Option<&FunctionDecl> {
        self.functions.iter().find(|f| f.name == name)
    }
    pub fn find_uniform(&self, name: &str) -> Option<&UniformDecl> {
        self.uniforms.iter().find(|u| u.name == name)
    }
}

// ============================================================
// SECTION 5: PARSER
// ============================================================

pub struct Parser {
    pub tokens: Vec<Token>,
    pub pos: usize,
    pub errors: Vec<ParseError>,
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: u32,
    pub col: u32,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0, errors: Vec::new() }
    }
    fn peek(&self) -> &Token { &self.tokens[self.pos.min(self.tokens.len()-1)] }
    fn peek2(&self) -> &Token { &self.tokens[(self.pos+1).min(self.tokens.len()-1)] }
    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos.min(self.tokens.len()-1)];
        if self.pos < self.tokens.len()-1 { self.pos += 1; }
        t
    }
    fn expect(&mut self, kind: &TokenKind) -> bool {
        if &self.peek().kind == kind { self.advance(); true }
        else {
            let tok = self.peek().clone();
            self.errors.push(ParseError {
                message: format!("Expected {:?}, got {:?}", kind, tok.kind),
                line: tok.line, col: tok.col,
            });
            false
        }
    }
    fn at_eof(&self) -> bool { matches!(self.peek().kind, TokenKind::Eof) }

    fn parse_type(&mut self) -> ShaderType {
        let tok = self.peek().kind.clone();
        if let Some(ty) = ShaderType::from_token(&tok) {
            self.advance();
            return ty;
        }
        if let TokenKind::Ident(name) = &tok {
            let name = name.clone();
            self.advance();
            return ShaderType::Struct(name);
        }
        ShaderType::Unknown
    }

    fn parse_expr_primary(&mut self) -> Expr {
        match self.peek().kind.clone() {
            TokenKind::IntLit(v) => { self.advance(); Expr::IntLit(v) }
            TokenKind::FloatLit(v) => { self.advance(); Expr::FloatLit(v) }
            TokenKind::BoolLit(v) => { self.advance(); Expr::BoolLit(v) }
            TokenKind::Ident(name) => {
                self.advance();
                if self.peek().kind == TokenKind::LParen {
                    // function call
                    self.advance();
                    let mut args = Vec::new();
                    while self.peek().kind != TokenKind::RParen && !self.at_eof() {
                        args.push(self.parse_expr());
                        if self.peek().kind == TokenKind::Comma { self.advance(); }
                        else { break; }
                    }
                    self.expect(&TokenKind::RParen);
                    Expr::Call { function: name, args }
                } else {
                    Expr::Ident(name)
                }
            }
            TokenKind::LParen => {
                self.advance();
                let e = self.parse_expr();
                self.expect(&TokenKind::RParen);
                e
            }
            tok if ShaderType::from_token(&tok).is_some() => {
                let ty = self.parse_type();
                self.expect(&TokenKind::LParen);
                let mut args = Vec::new();
                while self.peek().kind != TokenKind::RParen && !self.at_eof() {
                    args.push(self.parse_expr());
                    if self.peek().kind == TokenKind::Comma { self.advance(); }
                    else { break; }
                }
                self.expect(&TokenKind::RParen);
                Expr::Construction { ty, args }
            }
            _ => {
                let tok = self.peek().clone();
                self.errors.push(ParseError { message: format!("Unexpected token {:?}", tok.kind), line: tok.line, col: tok.col });
                self.advance();
                Expr::IntLit(0)
            }
        }
    }

    fn parse_expr_postfix(&mut self) -> Expr {
        let mut e = self.parse_expr_primary();
        loop {
            match self.peek().kind.clone() {
                TokenKind::Dot => {
                    self.advance();
                    if let TokenKind::Ident(field) = self.peek().kind.clone() {
                        self.advance();
                        e = Expr::Field { object: Box::new(e), field };
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let idx = self.parse_expr();
                    self.expect(&TokenKind::RBracket);
                    e = Expr::Index { array: Box::new(e), index: Box::new(idx) };
                }
                TokenKind::PlusPlus => { self.advance(); e = Expr::Unary { op: UnaryOp::PostIncrement, operand: Box::new(e) }; }
                TokenKind::MinusMinus => { self.advance(); e = Expr::Unary { op: UnaryOp::PostDecrement, operand: Box::new(e) }; }
                _ => break,
            }
        }
        e
    }

    fn parse_expr_unary(&mut self) -> Expr {
        match self.peek().kind {
            TokenKind::Minus => { self.advance(); Expr::Unary { op: UnaryOp::Negate, operand: Box::new(self.parse_expr_unary()) } }
            TokenKind::Bang => { self.advance(); Expr::Unary { op: UnaryOp::Not, operand: Box::new(self.parse_expr_unary()) } }
            TokenKind::Tilde => { self.advance(); Expr::Unary { op: UnaryOp::BitNot, operand: Box::new(self.parse_expr_unary()) } }
            TokenKind::PlusPlus => { self.advance(); Expr::Unary { op: UnaryOp::PreIncrement, operand: Box::new(self.parse_expr_unary()) } }
            TokenKind::MinusMinus => { self.advance(); Expr::Unary { op: UnaryOp::PreDecrement, operand: Box::new(self.parse_expr_unary()) } }
            _ => self.parse_expr_postfix(),
        }
    }

    fn parse_expr_mul(&mut self) -> Expr {
        let mut left = self.parse_expr_unary();
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_expr_unary();
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_add(&mut self) -> Expr {
        let mut left = self.parse_expr_mul();
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_expr_mul();
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_shift(&mut self) -> Expr {
        let mut left = self.parse_expr_add();
        loop {
            let op = match self.peek().kind {
                TokenKind::LShift => BinaryOp::Shl,
                TokenKind::RShift => BinaryOp::Shr,
                _ => break,
            };
            self.advance();
            let right = self.parse_expr_add();
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_compare(&mut self) -> Expr {
        let mut left = self.parse_expr_shift();
        loop {
            let op = match self.peek().kind {
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::LtEq => BinaryOp::Le,
                TokenKind::GtEq => BinaryOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_expr_shift();
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_equality(&mut self) -> Expr {
        let mut left = self.parse_expr_compare();
        loop {
            let op = match self.peek().kind {
                TokenKind::EqEq => BinaryOp::Eq,
                TokenKind::BangEq => BinaryOp::Ne,
                _ => break,
            };
            self.advance();
            let right = self.parse_expr_compare();
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_bitand(&mut self) -> Expr {
        let mut left = self.parse_expr_equality();
        while self.peek().kind == TokenKind::Amp {
            self.advance();
            let right = self.parse_expr_equality();
            left = Expr::Binary { op: BinaryOp::BitAnd, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_bitor(&mut self) -> Expr {
        let mut left = self.parse_expr_bitand();
        while self.peek().kind == TokenKind::Pipe {
            self.advance();
            let right = self.parse_expr_bitand();
            left = Expr::Binary { op: BinaryOp::BitOr, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_and(&mut self) -> Expr {
        let mut left = self.parse_expr_bitor();
        while self.peek().kind == TokenKind::AmpAmp {
            self.advance();
            let right = self.parse_expr_bitor();
            left = Expr::Binary { op: BinaryOp::And, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_or(&mut self) -> Expr {
        let mut left = self.parse_expr_and();
        while self.peek().kind == TokenKind::PipePipe {
            self.advance();
            let right = self.parse_expr_and();
            left = Expr::Binary { op: BinaryOp::Or, left: Box::new(left), right: Box::new(right) };
        }
        left
    }

    fn parse_expr_ternary(&mut self) -> Expr {
        let cond = self.parse_expr_or();
        if self.peek().kind == TokenKind::Question {
            self.advance();
            let then_expr = self.parse_expr();
            self.expect(&TokenKind::Colon);
            let else_expr = self.parse_expr_ternary();
            Expr::Ternary { cond: Box::new(cond), then_expr: Box::new(then_expr), else_expr: Box::new(else_expr) }
        } else { cond }
    }

    fn parse_expr_assign(&mut self) -> Expr {
        let left = self.parse_expr_ternary();
        let op = match self.peek().kind {
            TokenKind::Eq => AssignOp::Assign,
            TokenKind::PlusEq => AssignOp::AddAssign,
            TokenKind::MinusEq => AssignOp::SubAssign,
            TokenKind::StarEq => AssignOp::MulAssign,
            TokenKind::SlashEq => AssignOp::DivAssign,
            TokenKind::PercentEq => AssignOp::ModAssign,
            TokenKind::AmpEq => AssignOp::AndAssign,
            TokenKind::PipeEq => AssignOp::OrAssign,
            TokenKind::CaretEq => AssignOp::XorAssign,
            TokenKind::LShiftEq => AssignOp::ShlAssign,
            TokenKind::RShiftEq => AssignOp::ShrAssign,
            _ => return left,
        };
        self.advance();
        let value = self.parse_expr_assign();
        Expr::Assign { op, target: Box::new(left), value: Box::new(value) }
    }

    pub fn parse_expr(&mut self) -> Expr { self.parse_expr_assign() }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        match self.peek().kind.clone() {
            TokenKind::LBrace => {
                self.advance();
                let mut stmts = Vec::new();
                while self.peek().kind != TokenKind::RBrace && !self.at_eof() {
                    if let Some(s) = self.parse_stmt() { stmts.push(s); }
                }
                self.expect(&TokenKind::RBrace);
                Some(Stmt::Block(stmts))
            }
            TokenKind::KwIf => {
                self.advance();
                self.expect(&TokenKind::LParen);
                let cond = self.parse_expr();
                self.expect(&TokenKind::RParen);
                let then_body = Box::new(self.parse_stmt()?);
                let else_body = if self.peek().kind == TokenKind::KwElse {
                    self.advance();
                    Some(Box::new(self.parse_stmt()?))
                } else { None };
                Some(Stmt::If { cond, then_body, else_body })
            }
            TokenKind::KwFor => {
                self.advance();
                self.expect(&TokenKind::LParen);
                let init = if self.peek().kind != TokenKind::Semicolon {
                    self.parse_stmt().map(Box::new)
                } else { self.advance(); None };
                let cond = if self.peek().kind != TokenKind::Semicolon {
                    Some(self.parse_expr())
                } else { None };
                self.expect(&TokenKind::Semicolon);
                let step = if self.peek().kind != TokenKind::RParen {
                    Some(self.parse_expr())
                } else { None };
                self.expect(&TokenKind::RParen);
                let body = Box::new(self.parse_stmt()?);
                Some(Stmt::For { init, cond, step, body })
            }
            TokenKind::KwWhile => {
                self.advance();
                self.expect(&TokenKind::LParen);
                let cond = self.parse_expr();
                self.expect(&TokenKind::RParen);
                let body = Box::new(self.parse_stmt()?);
                Some(Stmt::While { cond, body })
            }
            TokenKind::KwDo => {
                self.advance();
                let body = Box::new(self.parse_stmt()?);
                self.expect(&TokenKind::KwWhile);
                self.expect(&TokenKind::LParen);
                let cond = self.parse_expr();
                self.expect(&TokenKind::RParen);
                self.expect(&TokenKind::Semicolon);
                Some(Stmt::DoWhile { body, cond })
            }
            TokenKind::KwReturn => {
                self.advance();
                let val = if self.peek().kind != TokenKind::Semicolon {
                    Some(self.parse_expr())
                } else { None };
                self.expect(&TokenKind::Semicolon);
                Some(Stmt::Return(val))
            }
            TokenKind::KwBreak => { self.advance(); self.expect(&TokenKind::Semicolon); Some(Stmt::Break) }
            TokenKind::KwContinue => { self.advance(); self.expect(&TokenKind::Semicolon); Some(Stmt::Continue) }
            TokenKind::KwDiscard => { self.advance(); self.expect(&TokenKind::Semicolon); Some(Stmt::Discard) }
            tok if ShaderType::from_token(&tok).is_some() || matches!(tok, TokenKind::Ident(_)) => {
                // Could be var decl or expr
                let saved_pos = self.pos;
                let ty = self.parse_type();
                if let TokenKind::Ident(name) = self.peek().kind.clone() {
                    self.advance();
                    let init = if self.peek().kind == TokenKind::Eq {
                        self.advance();
                        Some(self.parse_expr())
                    } else { None };
                    self.expect(&TokenKind::Semicolon);
                    Some(Stmt::VarDecl { ty, name, qualifier: None, init })
                } else {
                    // rewind — it's an expression
                    self.pos = saved_pos;
                    let e = self.parse_expr();
                    self.expect(&TokenKind::Semicolon);
                    Some(Stmt::Expr(e))
                }
            }
            _ => {
                let e = self.parse_expr();
                self.expect(&TokenKind::Semicolon);
                Some(Stmt::Expr(e))
            }
        }
    }

    fn parse_function(&mut self, return_type: ShaderType, name: String) -> FunctionDecl {
        // already consumed return_type and name
        self.expect(&TokenKind::LParen);
        let mut params = Vec::new();
        while self.peek().kind != TokenKind::RParen && !self.at_eof() {
            let qualifier = match self.peek().kind {
                TokenKind::KwIn => { self.advance(); Some(VarQualifier::In) }
                TokenKind::KwOut => { self.advance(); Some(VarQualifier::Out) }
                TokenKind::KwInout => { self.advance(); Some(VarQualifier::Inout) }
                TokenKind::KwConst => { self.advance(); Some(VarQualifier::Const) }
                _ => None,
            };
            let ty = self.parse_type();
            let pname = if let TokenKind::Ident(n) = self.peek().kind.clone() { self.advance(); n } else { String::new() };
            params.push(FunctionParam { ty, name: pname, qualifier });
            if self.peek().kind == TokenKind::Comma { self.advance(); } else { break; }
        }
        self.expect(&TokenKind::RParen);
        let mut body = Vec::new();
        if self.peek().kind == TokenKind::LBrace {
            self.advance();
            while self.peek().kind != TokenKind::RBrace && !self.at_eof() {
                if let Some(s) = self.parse_stmt() { body.push(s); }
            }
            self.expect(&TokenKind::RBrace);
        } else {
            self.expect(&TokenKind::Semicolon);
        }
        FunctionDecl { return_type, name, params, body, is_builtin: false }
    }

    pub fn parse_translation_unit(&mut self) -> ShaderAst {
        let mut ast = ShaderAst::new();
        while !self.at_eof() {
            // Handle preprocessor directives
            if self.peek().kind == TokenKind::Hash {
                self.advance();
                if let TokenKind::Ident(directive) = self.peek().kind.clone() {
                    self.advance();
                    match directive.as_str() {
                        "version" => {
                            if let TokenKind::IntLit(v) = self.peek().kind { self.advance(); ast.version = v as u32; }
                        }
                        "define" => {
                            if let TokenKind::Ident(macro_name) = self.peek().kind.clone() {
                                self.advance();
                                let val = if let TokenKind::Ident(v) = self.peek().kind.clone() { self.advance(); v } else { String::new() };
                                ast.defines.insert(macro_name, val);
                            }
                        }
                        _ => {}
                    }
                }
                continue;
            }
            // Layout qualifier
            let mut binding = None;
            let mut set_num = None;
            if self.peek().kind == TokenKind::KwLayout {
                self.advance();
                self.expect(&TokenKind::LParen);
                while self.peek().kind != TokenKind::RParen && !self.at_eof() {
                    if let TokenKind::Ident(k) = self.peek().kind.clone() {
                        self.advance();
                        if self.peek().kind == TokenKind::Eq {
                            self.advance();
                            if let TokenKind::IntLit(v) = self.peek().kind { self.advance();
                                if k == "binding" { binding = Some(v as u32); }
                                else if k == "set" { set_num = Some(v as u32); }
                            }
                        }
                    } else { self.advance(); }
                    if self.peek().kind == TokenKind::Comma { self.advance(); } else { break; }
                }
                self.expect(&TokenKind::RParen);
            }
            // Qualifiers
            let qualifier = match self.peek().kind {
                TokenKind::KwUniform => { self.advance(); Some(VarQualifier::Uniform) }
                TokenKind::KwIn | TokenKind::KwAttribute => { self.advance(); Some(VarQualifier::In) }
                TokenKind::KwOut | TokenKind::KwVarying => { self.advance(); Some(VarQualifier::Out) }
                TokenKind::KwConst => { self.advance(); Some(VarQualifier::Const) }
                _ => None,
            };
            // Precision qualifier
            if matches!(self.peek().kind, TokenKind::KwPrecision|TokenKind::KwHighp|TokenKind::KwMediump|TokenKind::KwLowp) {
                self.advance();
                if matches!(self.peek().kind, TokenKind::KwHighp|TokenKind::KwMediump|TokenKind::KwLowp) { self.advance(); }
                self.parse_type();
                self.expect(&TokenKind::Semicolon);
                continue;
            }
            // Struct
            if self.peek().kind == TokenKind::KwStruct {
                self.advance();
                let sname = if let TokenKind::Ident(n) = self.peek().kind.clone() { self.advance(); n } else { String::new() };
                let mut fields = Vec::new();
                self.expect(&TokenKind::LBrace);
                while self.peek().kind != TokenKind::RBrace && !self.at_eof() {
                    let fty = self.parse_type();
                    while let TokenKind::Ident(fname) = self.peek().kind.clone() {
                        self.advance();
                        let arr = if self.peek().kind == TokenKind::LBracket {
                            self.advance();
                            let sz = if let TokenKind::IntLit(v) = self.peek().kind { self.advance(); Some(v as u32) } else { None };
                            self.expect(&TokenKind::RBracket);
                            sz
                        } else { None };
                        fields.push(StructField { ty: fty.clone(), name: fname, array_size: arr });
                        if self.peek().kind == TokenKind::Comma { self.advance(); } else { break; }
                    }
                    self.expect(&TokenKind::Semicolon);
                }
                self.expect(&TokenKind::RBrace);
                self.expect(&TokenKind::Semicolon);
                ast.structs.push(StructDecl { name: sname, fields });
                continue;
            }
            // Type + name
            if self.peek().is_type_keyword() || matches!(self.peek().kind, TokenKind::Ident(_)) {
                let ty = self.parse_type();
                let name = if let TokenKind::Ident(n) = self.peek().kind.clone() { self.advance(); n } else { String::new() };
                // Array?
                let arr_size = if self.peek().kind == TokenKind::LBracket {
                    self.advance();
                    let sz = if let TokenKind::IntLit(v) = self.peek().kind { self.advance(); Some(v as u32) } else { None };
                    self.expect(&TokenKind::RBracket);
                    sz
                } else { None };
                // Function?
                if self.peek().kind == TokenKind::LParen {
                    let func = self.parse_function(ty, name);
                    ast.functions.push(func);
                } else {
                    // Variable declaration
                    match qualifier {
                        Some(VarQualifier::Uniform) => {
                            ast.uniforms.push(UniformDecl { ty, name, binding, set: set_num, array_size: arr_size });
                            self.expect(&TokenKind::Semicolon);
                        }
                        Some(VarQualifier::In) => {
                            ast.attributes.push(AttributeDecl { ty, name, location: binding });
                            self.expect(&TokenKind::Semicolon);
                        }
                        Some(VarQualifier::Out) => {
                            ast.varyings.push(AttributeDecl { ty, name, location: binding });
                            self.expect(&TokenKind::Semicolon);
                        }
                        _ => {
                            let init = if self.peek().kind == TokenKind::Eq { self.advance(); Some(self.parse_expr()) } else { None };
                            self.expect(&TokenKind::Semicolon);
                            ast.global_vars.push(Stmt::VarDecl { ty, name, qualifier, init });
                        }
                    }
                }
            } else {
                self.advance(); // skip unknown token
            }
        }
        ast
    }
}

// ============================================================
// SECTION 6: SEMANTIC ANALYSIS
// ============================================================

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub ty: ShaderType,
    pub qualifier: Option<VarQualifier>,
    pub used: bool,
    pub defined_at_line: u32,
}

pub struct Scope {
    pub symbols: HashMap<String, Symbol>,
    pub parent: Option<*mut Scope>,
}

impl Scope {
    pub fn new() -> Self { Self { symbols: HashMap::new(), parent: None } }
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        if let Some(s) = self.symbols.get(name) { return Some(s); }
        None
    }
    pub fn insert(&mut self, sym: Symbol) { self.symbols.insert(sym.name.clone(), sym); }
}

#[derive(Debug, Clone)]
pub struct SemanticError {
    pub kind: SemanticErrorKind,
    pub message: String,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub enum SemanticErrorKind {
    UndeclaredVariable,
    TypeMismatch,
    RedefinedSymbol,
    InvalidOperation,
    UnusedVariable,
    RecursionDetected,
    ReturnTypeMismatch,
    InvalidArgCount,
}

pub struct SemanticAnalyzer {
    pub scopes: Vec<HashMap<String, Symbol>>,
    pub errors: Vec<SemanticError>,
    pub warnings: Vec<SemanticError>,
    pub current_function: Option<String>,
    pub call_stack: HashSet<String>,
    pub built_in_functions: HashMap<String, BuiltinSignature>,
}

#[derive(Debug, Clone)]
pub struct BuiltinSignature {
    pub name: String,
    pub overloads: Vec<(Vec<ShaderType>, ShaderType)>, // (params, return)
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        let mut a = Self {
            scopes: vec![HashMap::new()],
            errors: Vec::new(),
            warnings: Vec::new(),
            current_function: None,
            call_stack: HashSet::new(),
            built_in_functions: HashMap::new(),
        };
        a.register_builtins();
        a
    }

    fn register_builtins(&mut self) {
        let math_float_vec = |name: &str| {
            let overloads = vec![
                (vec![ShaderType::Float], ShaderType::Float),
                (vec![ShaderType::Vec2], ShaderType::Vec2),
                (vec![ShaderType::Vec3], ShaderType::Vec3),
                (vec![ShaderType::Vec4], ShaderType::Vec4),
            ];
            BuiltinSignature { name: name.to_string(), overloads }
        };
        let names = &["sin","cos","tan","asin","acos","atan","sinh","cosh","tanh","exp","exp2","log","log2","sqrt","inversesqrt","abs","sign","floor","ceil","round","fract","normalize","length","radians","degrees"];
        for n in names {
            self.built_in_functions.insert(n.to_string(), math_float_vec(n));
        }
        self.built_in_functions.insert("pow".to_string(), BuiltinSignature { name: "pow".to_string(), overloads: vec![(vec![ShaderType::Float,ShaderType::Float],ShaderType::Float),(vec![ShaderType::Vec2,ShaderType::Vec2],ShaderType::Vec2),(vec![ShaderType::Vec3,ShaderType::Vec3],ShaderType::Vec3),(vec![ShaderType::Vec4,ShaderType::Vec4],ShaderType::Vec4)] });
        self.built_in_functions.insert("mix".to_string(), BuiltinSignature { name: "mix".to_string(), overloads: vec![(vec![ShaderType::Float,ShaderType::Float,ShaderType::Float],ShaderType::Float),(vec![ShaderType::Vec2,ShaderType::Vec2,ShaderType::Float],ShaderType::Vec2),(vec![ShaderType::Vec3,ShaderType::Vec3,ShaderType::Float],ShaderType::Vec3)] });
        self.built_in_functions.insert("clamp".to_string(), BuiltinSignature { name: "clamp".to_string(), overloads: vec![(vec![ShaderType::Float,ShaderType::Float,ShaderType::Float],ShaderType::Float),(vec![ShaderType::Vec2,ShaderType::Float,ShaderType::Float],ShaderType::Vec2)] });
        self.built_in_functions.insert("smoothstep".to_string(), BuiltinSignature { name: "smoothstep".to_string(), overloads: vec![(vec![ShaderType::Float,ShaderType::Float,ShaderType::Float],ShaderType::Float)] });
        self.built_in_functions.insert("dot".to_string(), BuiltinSignature { name: "dot".to_string(), overloads: vec![(vec![ShaderType::Vec2,ShaderType::Vec2],ShaderType::Float),(vec![ShaderType::Vec3,ShaderType::Vec3],ShaderType::Float),(vec![ShaderType::Vec4,ShaderType::Vec4],ShaderType::Float)] });
        self.built_in_functions.insert("cross".to_string(), BuiltinSignature { name: "cross".to_string(), overloads: vec![(vec![ShaderType::Vec3,ShaderType::Vec3],ShaderType::Vec3)] });
        self.built_in_functions.insert("reflect".to_string(), BuiltinSignature { name: "reflect".to_string(), overloads: vec![(vec![ShaderType::Vec3,ShaderType::Vec3],ShaderType::Vec3)] });
        self.built_in_functions.insert("refract".to_string(), BuiltinSignature { name: "refract".to_string(), overloads: vec![(vec![ShaderType::Vec3,ShaderType::Vec3,ShaderType::Float],ShaderType::Vec3)] });
        self.built_in_functions.insert("max".to_string(), BuiltinSignature { name: "max".to_string(), overloads: vec![(vec![ShaderType::Float,ShaderType::Float],ShaderType::Float)] });
        self.built_in_functions.insert("min".to_string(), BuiltinSignature { name: "min".to_string(), overloads: vec![(vec![ShaderType::Float,ShaderType::Float],ShaderType::Float)] });
        self.built_in_functions.insert("mod".to_string(), BuiltinSignature { name: "mod".to_string(), overloads: vec![(vec![ShaderType::Float,ShaderType::Float],ShaderType::Float),(vec![ShaderType::Vec2,ShaderType::Float],ShaderType::Vec2)] });
        self.built_in_functions.insert("texture".to_string(), BuiltinSignature { name: "texture".to_string(), overloads: vec![(vec![ShaderType::Sampler2D,ShaderType::Vec2],ShaderType::Vec4),(vec![ShaderType::SamplerCube,ShaderType::Vec3],ShaderType::Vec4)] });
        self.built_in_functions.insert("textureLod".to_string(), BuiltinSignature { name: "textureLod".to_string(), overloads: vec![(vec![ShaderType::Sampler2D,ShaderType::Vec2,ShaderType::Float],ShaderType::Vec4)] });
        self.built_in_functions.insert("dFdx".to_string(), BuiltinSignature { name: "dFdx".to_string(), overloads: vec![(vec![ShaderType::Float],ShaderType::Float),(vec![ShaderType::Vec2],ShaderType::Vec2),(vec![ShaderType::Vec3],ShaderType::Vec3)] });
        self.built_in_functions.insert("dFdy".to_string(), BuiltinSignature { name: "dFdy".to_string(), overloads: vec![(vec![ShaderType::Float],ShaderType::Float),(vec![ShaderType::Vec2],ShaderType::Vec2),(vec![ShaderType::Vec3],ShaderType::Vec3)] });
    }

    fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop_scope(&mut self) { self.scopes.pop(); }
    fn current_scope(&mut self) -> &mut HashMap<String, Symbol> { self.scopes.last_mut().unwrap() }

    fn declare(&mut self, sym: Symbol) {
        let name = sym.name.clone();
        if self.current_scope().contains_key(&name) {
            self.errors.push(SemanticError { kind: SemanticErrorKind::RedefinedSymbol, message: format!("Redefined symbol '{}'", name), line: sym.defined_at_line });
        }
        self.current_scope().insert(name, sym);
    }

    fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(s) = scope.get(name) { return Some(s); }
        }
        None
    }

    fn infer_type(&self, expr: &Expr) -> ShaderType {
        match expr {
            Expr::IntLit(_) => ShaderType::Int,
            Expr::FloatLit(_) => ShaderType::Float,
            Expr::BoolLit(_) => ShaderType::Bool,
            Expr::Ident(name) => {
                if let Some(sym) = self.lookup(name) { sym.ty.clone() } else { ShaderType::Unknown }
            }
            Expr::Binary { op, left, right } => {
                let lt = self.infer_type(left);
                let rt = self.infer_type(right);
                match op {
                    BinaryOp::Eq|BinaryOp::Ne|BinaryOp::Lt|BinaryOp::Gt|BinaryOp::Le|BinaryOp::Ge|BinaryOp::And|BinaryOp::Or => ShaderType::Bool,
                    _ => if lt == rt { lt } else if ShaderType::can_implicit_convert(&rt, &lt) { lt } else { lt }
                }
            }
            Expr::Unary { op, operand } => self.infer_type(operand),
            Expr::Call { function, args } => {
                if let Some(sig) = self.built_in_functions.get(function) {
                    let arg_types: Vec<ShaderType> = args.iter().map(|a| self.infer_type(a)).collect();
                    for (params, ret) in &sig.overloads {
                        if params.len() == arg_types.len() { return ret.clone(); }
                    }
                }
                ShaderType::Unknown
            }
            Expr::Field { object, field } => ShaderType::Float, // simplified
            Expr::Swizzle { object, components } => {
                let n = components.len();
                match n {
                    1 => ShaderType::Float,
                    2 => ShaderType::Vec2,
                    3 => ShaderType::Vec3,
                    4 => ShaderType::Vec4,
                    _ => ShaderType::Unknown,
                }
            }
            Expr::Index { array, .. } => {
                match self.infer_type(array) {
                    ShaderType::Array(inner, _) => *inner,
                    ShaderType::Vec2|ShaderType::Vec3|ShaderType::Vec4 => ShaderType::Float,
                    ShaderType::Mat2|ShaderType::Mat3|ShaderType::Mat4 => ShaderType::Vec4, // simplified
                    other => other,
                }
            }
            Expr::Construction { ty, .. } => ty.clone(),
            Expr::Cast { target_type, .. } => target_type.clone(),
            Expr::Assign { value, .. } => self.infer_type(value),
            Expr::Ternary { then_expr, .. } => self.infer_type(then_expr),
        }
    }

    fn check_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(name) => {
                if self.lookup(name).is_none() && !self.built_in_functions.contains_key(name.as_str()) {
                    // Only error if not a known builtin constant
                    let known = ["gl_Position","gl_FragCoord","gl_FragDepth","gl_VertexID","gl_InstanceID","true","false","PI","E"];
                    if !known.contains(&name.as_str()) {
                        self.errors.push(SemanticError { kind: SemanticErrorKind::UndeclaredVariable, message: format!("Undeclared variable '{}'", name), line: 0 });
                    }
                }
            }
            Expr::Binary { op, left, right } => {
                self.check_expr(left);
                self.check_expr(right);
                let lt = self.infer_type(left);
                let rt = self.infer_type(right);
                if lt != rt && !ShaderType::can_implicit_convert(&lt, &rt) && !ShaderType::can_implicit_convert(&rt, &lt) && lt != ShaderType::Unknown && rt != ShaderType::Unknown {
                    // warn about potential type mismatch
                }
            }
            Expr::Call { function, args } => {
                for a in args { self.check_expr(a); }
                if !self.built_in_functions.contains_key(function.as_str()) {
                    // Check user-defined function call for recursion
                    if let Some(fname) = &self.current_function {
                        if fname == function {
                            self.errors.push(SemanticError { kind: SemanticErrorKind::RecursionDetected, message: format!("Recursive call to '{}'", function), line: 0 });
                        }
                    }
                }
            }
            Expr::Assign { target, value, .. } => {
                self.check_expr(target);
                self.check_expr(value);
            }
            Expr::Unary { operand, .. } => self.check_expr(operand),
            Expr::Ternary { cond, then_expr, else_expr } => {
                self.check_expr(cond);
                self.check_expr(then_expr);
                self.check_expr(else_expr);
            }
            Expr::Field { object, .. } | Expr::Swizzle { object, .. } => self.check_expr(object),
            Expr::Index { array, index } => { self.check_expr(array); self.check_expr(index); }
            Expr::Construction { args, .. } => { for a in args { self.check_expr(a); } }
            _ => {}
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(e) => self.check_expr(e),
            Stmt::Block(stmts) => {
                self.push_scope();
                for s in stmts { self.check_stmt(s); }
                self.pop_scope();
            }
            Stmt::VarDecl { ty, name, qualifier, init } => {
                if let Some(init_expr) = init { self.check_expr(init_expr); }
                self.declare(Symbol { name: name.clone(), ty: ty.clone(), qualifier: *qualifier, used: false, defined_at_line: 0 });
            }
            Stmt::If { cond, then_body, else_body } => {
                self.check_expr(cond);
                self.check_stmt(then_body);
                if let Some(e) = else_body { self.check_stmt(e); }
            }
            Stmt::For { init, cond, step, body } => {
                self.push_scope();
                if let Some(i) = init { self.check_stmt(i); }
                if let Some(c) = cond { self.check_expr(c); }
                if let Some(s) = step { self.check_expr(s); }
                self.check_stmt(body);
                self.pop_scope();
            }
            Stmt::While { cond, body } => { self.check_expr(cond); self.check_stmt(body); }
            Stmt::DoWhile { body, cond } => { self.check_stmt(body); self.check_expr(cond); }
            Stmt::Return(val) => { if let Some(v) = val { self.check_expr(v); } }
            _ => {}
        }
    }

    pub fn analyze(&mut self, ast: &ShaderAst) {
        // Register global uniforms
        for u in &ast.uniforms {
            self.declare(Symbol { name: u.name.clone(), ty: u.ty.clone(), qualifier: Some(VarQualifier::Uniform), used: false, defined_at_line: 0 });
        }
        for a in &ast.attributes {
            self.declare(Symbol { name: a.name.clone(), ty: a.ty.clone(), qualifier: Some(VarQualifier::In), used: false, defined_at_line: 0 });
        }
        for v in &ast.varyings {
            self.declare(Symbol { name: v.name.clone(), ty: v.ty.clone(), qualifier: Some(VarQualifier::Out), used: false, defined_at_line: 0 });
        }
        // Register struct types
        for s in &ast.structs {
            self.declare(Symbol { name: s.name.clone(), ty: ShaderType::Struct(s.name.clone()), qualifier: None, used: false, defined_at_line: 0 });
        }
        // Analyze functions
        for func in &ast.functions {
            self.current_function = Some(func.name.clone());
            self.push_scope();
            for param in &func.params {
                self.declare(Symbol { name: param.name.clone(), ty: param.ty.clone(), qualifier: param.qualifier, used: false, defined_at_line: 0 });
            }
            for stmt in &func.body { self.check_stmt(stmt); }
            self.pop_scope();
        }
        self.current_function = None;
    }
}

// ============================================================
// SECTION 7: CONSTANT FOLDING
// ============================================================

pub fn fold_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Binary { op, left, right } => {
            let left = fold_expr(*left);
            let right = fold_expr(*right);
            match (&left, &right) {
                (Expr::FloatLit(a), Expr::FloatLit(b)) => {
                    match op {
                        BinaryOp::Add => Expr::FloatLit(a + b),
                        BinaryOp::Sub => Expr::FloatLit(a - b),
                        BinaryOp::Mul => Expr::FloatLit(a * b),
                        BinaryOp::Div => if *b != 0.0 { Expr::FloatLit(a / b) } else { Expr::Binary { op, left: Box::new(left), right: Box::new(right) } },
                        BinaryOp::Eq => Expr::BoolLit(a == b),
                        BinaryOp::Ne => Expr::BoolLit(a != b),
                        BinaryOp::Lt => Expr::BoolLit(a < b),
                        BinaryOp::Gt => Expr::BoolLit(a > b),
                        BinaryOp::Le => Expr::BoolLit(a <= b),
                        BinaryOp::Ge => Expr::BoolLit(a >= b),
                        _ => Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                    }
                }
                (Expr::IntLit(a), Expr::IntLit(b)) => {
                    match op {
                        BinaryOp::Add => Expr::IntLit(a + b),
                        BinaryOp::Sub => Expr::IntLit(a - b),
                        BinaryOp::Mul => Expr::IntLit(a * b),
                        BinaryOp::Div => if *b != 0 { Expr::IntLit(a / b) } else { Expr::Binary { op, left: Box::new(left), right: Box::new(right) } },
                        BinaryOp::Mod => if *b != 0 { Expr::IntLit(a % b) } else { Expr::Binary { op, left: Box::new(left), right: Box::new(right) } },
                        BinaryOp::BitAnd => Expr::IntLit(a & b),
                        BinaryOp::BitOr => Expr::IntLit(a | b),
                        BinaryOp::BitXor => Expr::IntLit(a ^ b),
                        BinaryOp::Shl => Expr::IntLit(a << (b & 63)),
                        BinaryOp::Shr => Expr::IntLit(a >> (b & 63)),
                        BinaryOp::Eq => Expr::BoolLit(a == b),
                        BinaryOp::Ne => Expr::BoolLit(a != b),
                        BinaryOp::Lt => Expr::BoolLit(a < b),
                        BinaryOp::Gt => Expr::BoolLit(a > b),
                        BinaryOp::Le => Expr::BoolLit(a <= b),
                        BinaryOp::Ge => Expr::BoolLit(a >= b),
                        _ => Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                    }
                }
                (Expr::BoolLit(a), Expr::BoolLit(b)) => {
                    match op {
                        BinaryOp::And => Expr::BoolLit(*a && *b),
                        BinaryOp::Or => Expr::BoolLit(*a || *b),
                        BinaryOp::Eq => Expr::BoolLit(a == b),
                        BinaryOp::Ne => Expr::BoolLit(a != b),
                        _ => Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
                    }
                }
                // Algebraic simplifications
                (_, Expr::FloatLit(v)) if op == BinaryOp::Mul && *v == 1.0 => left,
                (Expr::FloatLit(v), _) if op == BinaryOp::Mul && *v == 1.0 => right,
                (_, Expr::FloatLit(v)) if op == BinaryOp::Mul && *v == 0.0 => Expr::FloatLit(0.0),
                (Expr::FloatLit(v), _) if op == BinaryOp::Mul && *v == 0.0 => Expr::FloatLit(0.0),
                (_, Expr::FloatLit(v)) if op == BinaryOp::Add && *v == 0.0 => left,
                (Expr::FloatLit(v), _) if op == BinaryOp::Add && *v == 0.0 => right,
                (_, Expr::FloatLit(v)) if op == BinaryOp::Sub && *v == 0.0 => left,
                (_, Expr::FloatLit(v)) if op == BinaryOp::Div && *v == 1.0 => left,
                (_, Expr::IntLit(v)) if op == BinaryOp::Mul && *v == 1 => left,
                (Expr::IntLit(v), _) if op == BinaryOp::Mul && *v == 1 => right,
                (_, Expr::IntLit(v)) if op == BinaryOp::Add && *v == 0 => left,
                (Expr::IntLit(v), _) if op == BinaryOp::Add && *v == 0 => right,
                _ => Expr::Binary { op, left: Box::new(left), right: Box::new(right) },
            }
        }
        Expr::Unary { op, operand } => {
            let operand = fold_expr(*operand);
            match (&operand, op) {
                (Expr::FloatLit(v), UnaryOp::Negate) => Expr::FloatLit(-v),
                (Expr::IntLit(v), UnaryOp::Negate) => Expr::IntLit(-v),
                (Expr::BoolLit(v), UnaryOp::Not) => Expr::BoolLit(!v),
                (Expr::IntLit(v), UnaryOp::BitNot) => Expr::IntLit(!v),
                _ => Expr::Unary { op, operand: Box::new(operand) },
            }
        }
        Expr::Ternary { cond, then_expr, else_expr } => {
            let cond = fold_expr(*cond);
            let then_expr = fold_expr(*then_expr);
            let else_expr = fold_expr(*else_expr);
            match &cond {
                Expr::BoolLit(true) => then_expr,
                Expr::BoolLit(false) => else_expr,
                _ => Expr::Ternary { cond: Box::new(cond), then_expr: Box::new(then_expr), else_expr: Box::new(else_expr) },
            }
        }
        Expr::Call { function, args } => {
            let args: Vec<Expr> = args.into_iter().map(fold_expr).collect();
            // Fold constant built-in calls
            let all_float_lits: Vec<f64> = args.iter().filter_map(|a| if let Expr::FloatLit(v) = a { Some(*v) } else { None }).collect();
            if all_float_lits.len() == args.len() {
                let result = fold_builtin_call(&function, &all_float_lits);
                if let Some(v) = result { return Expr::FloatLit(v); }
            }
            Expr::Call { function, args }
        }
        other => other,
    }
}

fn fold_builtin_call(name: &str, args: &[f64]) -> Option<f64> {
    match (name, args) {
        ("sin", [x]) => Some(x.sin()),
        ("cos", [x]) => Some(x.cos()),
        ("tan", [x]) => Some(x.tan()),
        ("asin", [x]) => Some(x.asin()),
        ("acos", [x]) => Some(x.acos()),
        ("atan", [x]) => Some(x.atan()),
        ("sqrt", [x]) if *x >= 0.0 => Some(x.sqrt()),
        ("inversesqrt", [x]) if *x > 0.0 => Some(1.0 / x.sqrt()),
        ("exp", [x]) => Some(x.exp()),
        ("exp2", [x]) => Some(x.exp2()),
        ("log", [x]) if *x > 0.0 => Some(x.ln()),
        ("log2", [x]) if *x > 0.0 => Some(x.log2()),
        ("abs", [x]) => Some(x.abs()),
        ("sign", [x]) => Some(x.signum()),
        ("floor", [x]) => Some(x.floor()),
        ("ceil", [x]) => Some(x.ceil()),
        ("round", [x]) => Some(x.round()),
        ("fract", [x]) => Some(x.fract()),
        ("pow", [x, y]) => Some(x.powf(*y)),
        ("min", [a, b]) => Some(a.min(*b)),
        ("max", [a, b]) => Some(a.max(*b)),
        ("clamp", [x, mn, mx]) => Some(x.clamp(*mn, *mx)),
        ("mix", [a, b, t]) => Some(a + (b - a) * t),
        ("smoothstep", [e0, e1, x]) => {
            let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
            Some(t * t * (3.0 - 2.0 * t))
        }
        ("radians", [x]) => Some(x.to_radians()),
        ("degrees", [x]) => Some(x.to_degrees()),
        _ => None,
    }
}

// ============================================================
// SECTION 8: DEAD CODE ELIMINATION
// ============================================================

pub fn eliminate_dead_code(stmts: Vec<Stmt>) -> Vec<Stmt> {
    let mut result = Vec::new();
    let mut unreachable = false;
    for stmt in stmts {
        if unreachable { break; }
        match stmt {
            Stmt::If { ref cond, ref then_body, ref else_body } => {
                if let Expr::BoolLit(true) = cond {
                    // Always-true: keep then, discard else
                    result.push(*then_body.clone());
                } else if let Expr::BoolLit(false) = cond {
                    // Always-false: keep else if present
                    if let Some(e) = else_body { result.push(*e.clone()); }
                } else {
                    result.push(stmt);
                }
            }
            Stmt::Return(_) | Stmt::Break | Stmt::Continue | Stmt::Discard => {
                result.push(stmt);
                unreachable = true;
            }
            Stmt::Block(inner) => {
                let cleaned = eliminate_dead_code(inner);
                result.push(Stmt::Block(cleaned));
            }
            other => result.push(other),
        }
    }
    result
}

// ============================================================
// SECTION 9: COMMON SUBEXPRESSION ELIMINATION
// ============================================================

pub struct CSEPass {
    pub expressions: HashMap<String, String>, // expr_key -> temp_var_name
    pub temps: Vec<(String, Expr)>,
    pub counter: u32,
}

impl CSEPass {
    pub fn new() -> Self { Self { expressions: HashMap::new(), temps: Vec::new(), counter: 0 } }
    pub fn expr_key(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Binary { op, left, right } => {
                let lk = Self::expr_key(left)?;
                let rk = Self::expr_key(right)?;
                Some(format!("{:?}_{}_{}", op, lk, rk))
            }
            Expr::Call { function, args } => {
                let arg_keys: Vec<String> = args.iter().filter_map(|a| Self::expr_key(a)).collect();
                if arg_keys.len() == args.len() { Some(format!("{}({})", function, arg_keys.join(","))) } else { None }
            }
            Expr::FloatLit(v) => Some(format!("f{:.6}", v)),
            Expr::IntLit(v) => Some(format!("i{}", v)),
            Expr::BoolLit(v) => Some(format!("b{}", v)),
            Expr::Ident(n) => Some(n.clone()),
            _ => None,
        }
    }
    pub fn process_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Binary { op, left, right } => {
                let left = self.process_expr(*left);
                let right = self.process_expr(*right);
                let folded = Expr::Binary { op, left: Box::new(left), right: Box::new(right) };
                if let Some(key) = Self::expr_key(&folded) {
                    if let Some(temp) = self.expressions.get(&key) {
                        return Expr::Ident(temp.clone());
                    }
                    let temp_name = format!("_cse_{}", self.counter);
                    self.counter += 1;
                    self.expressions.insert(key, temp_name.clone());
                    self.temps.push((temp_name.clone(), folded));
                    Expr::Ident(temp_name)
                } else { folded }
            }
            Expr::Call { function, args } => {
                let args: Vec<Expr> = args.into_iter().map(|a| self.process_expr(a)).collect();
                let e = Expr::Call { function, args };
                if let Some(key) = Self::expr_key(&e) {
                    if let Some(temp) = self.expressions.get(&key) {
                        return Expr::Ident(temp.clone());
                    }
                }
                e
            }
            other => other,
        }
    }
}

// ============================================================
// SECTION 10: CODE GENERATION - GLSL
// ============================================================

pub struct GlslEmitter {
    pub output: String,
    pub indent: usize,
    pub version: u32,
    pub es: bool,
}

impl GlslEmitter {
    pub fn new(version: u32, es: bool) -> Self { Self { output: String::new(), indent: 0, version, es } }

    fn write(&mut self, s: &str) { self.output.push_str(s); }
    fn writeln(&mut self, s: &str) {
        let indent: String = " ".repeat(self.indent * 4);
        self.output.push_str(&indent);
        self.output.push_str(s);
        self.output.push('\n');
    }
    fn indent_in(&mut self) { self.indent += 1; }
    fn indent_out(&mut self) { if self.indent > 0 { self.indent -= 1; } }

    pub fn emit_header(&mut self, ast: &ShaderAst) {
        if self.es {
            self.writeln(&format!("#version {} es", self.version));
        } else {
            self.writeln(&format!("#version {} core", self.version));
        }
        for (name, val) in &ast.defines {
            self.writeln(&format!("#define {} {}", name, val));
        }
    }

    pub fn emit_type(&self, ty: &ShaderType) -> String {
        ty.name().to_string()
    }

    pub fn emit_struct(&mut self, s: &StructDecl) {
        self.writeln(&format!("struct {} {{", s.name));
        self.indent_in();
        for f in &s.fields {
            let arr = if let Some(n) = f.array_size { format!("[{}]", n) } else { String::new() };
            self.writeln(&format!("{} {}{};", self.emit_type(&f.ty), f.name, arr));
        }
        self.indent_out();
        self.writeln("};");
    }

    pub fn emit_uniform(&mut self, u: &UniformDecl) {
        let layout = if u.binding.is_some() || u.set.is_some() {
            let parts: Vec<String> = [
                u.set.map(|s| format!("set={}", s)),
                u.binding.map(|b| format!("binding={}", b)),
            ].iter().filter_map(|x| x.clone()).collect();
            if parts.is_empty() { String::new() } else { format!("layout({}) ", parts.join(", ")) }
        } else { String::new() };
        let arr = if let Some(n) = u.array_size { format!("[{}]", n) } else { String::new() };
        self.writeln(&format!("{}uniform {} {}{};", layout, self.emit_type(&u.ty), u.name, arr));
    }

    pub fn emit_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::IntLit(v) => v.to_string(),
            Expr::FloatLit(v) => {
                if *v == v.floor() && v.abs() < 1e15 { format!("{:.1}", v) } else { format!("{}", v) }
            }
            Expr::BoolLit(v) => v.to_string(),
            Expr::Ident(n) => n.clone(),
            Expr::Unary { op, operand } => {
                let o = self.emit_expr(operand);
                match op {
                    UnaryOp::Negate => format!("(-{})", o),
                    UnaryOp::Not => format!("(!{})", o),
                    UnaryOp::BitNot => format!("(~{})", o),
                    UnaryOp::PreIncrement => format!("(++{})", o),
                    UnaryOp::PreDecrement => format!("(--{})", o),
                    UnaryOp::PostIncrement => format!("({}++)", o),
                    UnaryOp::PostDecrement => format!("({}--)", o),
                }
            }
            Expr::Binary { op, left, right } => {
                let l = self.emit_expr(left);
                let r = self.emit_expr(right);
                let sym = match op {
                    BinaryOp::Add => "+", BinaryOp::Sub => "-", BinaryOp::Mul => "*", BinaryOp::Div => "/",
                    BinaryOp::Mod => "%", BinaryOp::Eq => "==", BinaryOp::Ne => "!=",
                    BinaryOp::Lt => "<", BinaryOp::Gt => ">", BinaryOp::Le => "<=", BinaryOp::Ge => ">=",
                    BinaryOp::And => "&&", BinaryOp::Or => "||",
                    BinaryOp::BitAnd => "&", BinaryOp::BitOr => "|", BinaryOp::BitXor => "^",
                    BinaryOp::Shl => "<<", BinaryOp::Shr => ">>",
                };
                format!("({} {} {})", l, sym, r)
            }
            Expr::Assign { op, target, value } => {
                let t = self.emit_expr(target);
                let v = self.emit_expr(value);
                let sym = match op {
                    AssignOp::Assign => "=", AssignOp::AddAssign => "+=", AssignOp::SubAssign => "-=",
                    AssignOp::MulAssign => "*=", AssignOp::DivAssign => "/=", AssignOp::ModAssign => "%=",
                    AssignOp::AndAssign => "&=", AssignOp::OrAssign => "|=", AssignOp::XorAssign => "^=",
                    AssignOp::ShlAssign => "<<=", AssignOp::ShrAssign => ">>=",
                };
                format!("{} {} {}", t, sym, v)
            }
            Expr::Ternary { cond, then_expr, else_expr } => {
                format!("({} ? {} : {})", self.emit_expr(cond), self.emit_expr(then_expr), self.emit_expr(else_expr))
            }
            Expr::Call { function, args } => {
                let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr(a)).collect();
                format!("{}({})", function, arg_strs.join(", "))
            }
            Expr::Index { array, index } => format!("{}[{}]", self.emit_expr(array), self.emit_expr(index)),
            Expr::Field { object, field } => format!("{}.{}", self.emit_expr(object), field),
            Expr::Swizzle { object, components } => format!("{}.{}", self.emit_expr(object), components),
            Expr::Cast { target_type, expr } => format!("{}({})", target_type.name(), self.emit_expr(expr)),
            Expr::Construction { ty, args } => {
                let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr(a)).collect();
                format!("{}({})", ty.name(), arg_strs.join(", "))
            }
        }
    }

    pub fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(e) => {
                let s = self.emit_expr(e);
                self.writeln(&format!("{};", s));
            }
            Stmt::Block(stmts) => {
                self.writeln("{");
                self.indent_in();
                for s in stmts { self.emit_stmt(s); }
                self.indent_out();
                self.writeln("}");
            }
            Stmt::VarDecl { ty, name, qualifier, init } => {
                let qual = match qualifier {
                    Some(VarQualifier::Const) => "const ".to_string(),
                    Some(VarQualifier::In) => "in ".to_string(),
                    Some(VarQualifier::Out) => "out ".to_string(),
                    Some(VarQualifier::Inout) => "inout ".to_string(),
                    _ => String::new(),
                };
                if let Some(init_expr) = init {
                    self.writeln(&format!("{}{} {} = {};", qual, self.emit_type(ty), name, self.emit_expr(init_expr)));
                } else {
                    self.writeln(&format!("{}{} {};", qual, self.emit_type(ty), name));
                }
            }
            Stmt::If { cond, then_body, else_body } => {
                let cond_str = self.emit_expr(cond);
                self.writeln(&format!("if ({}) {{", cond_str));
                self.indent_in();
                self.emit_stmt(then_body);
                self.indent_out();
                if let Some(else_stmt) = else_body {
                    self.writeln("} else {");
                    self.indent_in();
                    self.emit_stmt(else_stmt);
                    self.indent_out();
                    self.writeln("}");
                } else {
                    self.writeln("}");
                }
            }
            Stmt::For { init, cond, step, body } => {
                let init_str = if let Some(i) = init {
                    match i.as_ref() {
                        Stmt::VarDecl { ty, name, init: Some(e), .. } => format!("{} {} = {}", self.emit_type(ty), name, self.emit_expr(e)),
                        Stmt::Expr(e) => self.emit_expr(e),
                        _ => String::new(),
                    }
                } else { String::new() };
                let cond_str = cond.as_ref().map(|c| self.emit_expr(c)).unwrap_or_default();
                let step_str = step.as_ref().map(|s| self.emit_expr(s)).unwrap_or_default();
                self.writeln(&format!("for ({}; {}; {}) {{", init_str, cond_str, step_str));
                self.indent_in();
                self.emit_stmt(body);
                self.indent_out();
                self.writeln("}");
            }
            Stmt::While { cond, body } => {
                self.writeln(&format!("while ({}) {{", self.emit_expr(cond)));
                self.indent_in();
                self.emit_stmt(body);
                self.indent_out();
                self.writeln("}");
            }
            Stmt::DoWhile { body, cond } => {
                self.writeln("do {");
                self.indent_in();
                self.emit_stmt(body);
                self.indent_out();
                self.writeln(&format!("}} while ({});", self.emit_expr(cond)));
            }
            Stmt::Return(val) => {
                if let Some(v) = val {
                    self.writeln(&format!("return {};", self.emit_expr(v)));
                } else {
                    self.writeln("return;");
                }
            }
            Stmt::Break => self.writeln("break;"),
            Stmt::Continue => self.writeln("continue;"),
            Stmt::Discard => self.writeln("discard;"),
        }
    }

    pub fn emit_function(&mut self, func: &FunctionDecl) {
        let params: Vec<String> = func.params.iter().map(|p| {
            let q = match p.qualifier {
                Some(VarQualifier::In) => "in ",
                Some(VarQualifier::Out) => "out ",
                Some(VarQualifier::Inout) => "inout ",
                _ => "",
            };
            format!("{}{} {}", q, self.emit_type(&p.ty), p.name)
        }).collect();
        self.writeln(&format!("{} {}({}) {{", self.emit_type(&func.return_type), func.name, params.join(", ")));
        self.indent_in();
        for stmt in &func.body { self.emit_stmt(stmt); }
        self.indent_out();
        self.writeln("}");
    }

    pub fn emit_ast(&mut self, ast: &ShaderAst) -> String {
        self.emit_header(ast);
        self.write("\n");
        for s in &ast.structs { self.emit_struct(s); self.write("\n"); }
        for u in &ast.uniforms { self.emit_uniform(u); }
        if !ast.uniforms.is_empty() { self.write("\n"); }
        for a in &ast.attributes {
            self.writeln(&format!("in {} {};", self.emit_type(&a.ty), a.name));
        }
        for v in &ast.varyings {
            self.writeln(&format!("out {} {};", self.emit_type(&v.ty), v.name));
        }
        if !ast.attributes.is_empty() || !ast.varyings.is_empty() { self.write("\n"); }
        for func in &ast.functions {
            self.emit_function(func);
            self.write("\n");
        }
        self.output.clone()
    }
}

// ============================================================
// SECTION 11: SPIRV-LIKE BYTECODE
// ============================================================

#[derive(Debug, Clone)]
pub enum SpvOp {
    LoadConst { dest: u32, value: SpvConst },
    Load { dest: u32, src_var: u32 },
    Store { dest_var: u32, src: u32 },
    Add { dest: u32, a: u32, b: u32 },
    Sub { dest: u32, a: u32, b: u32 },
    Mul { dest: u32, a: u32, b: u32 },
    Div { dest: u32, a: u32, b: u32 },
    FuncCall { dest: u32, func_id: u32, args: Vec<u32> },
    Jump { label: u32 },
    JumpIf { cond: u32, label_true: u32, label_false: u32 },
    Label { id: u32 },
    Return { value: Option<u32> },
    Phi { dest: u32, pairs: Vec<(u32, u32)> },
    Convert { dest: u32, src: u32, to_type: SpvTypeId },
    Negate { dest: u32, src: u32 },
    Not { dest: u32, src: u32 },
    Eq { dest: u32, a: u32, b: u32 },
    Ne { dest: u32, a: u32, b: u32 },
    Lt { dest: u32, a: u32, b: u32 },
    Gt { dest: u32, a: u32, b: u32 },
    Le { dest: u32, a: u32, b: u32 },
    Ge { dest: u32, a: u32, b: u32 },
}

#[derive(Debug, Clone)]
pub enum SpvConst {
    Float(f32),
    Int(i32),
    Bool(bool),
    Vec2([f32;2]),
    Vec3([f32;3]),
    Vec4([f32;4]),
}

#[derive(Debug, Clone, Copy)]
pub enum SpvTypeId { Float, Int, Uint, Bool, Vec2, Vec3, Vec4, Mat4 }

#[derive(Debug, Clone)]
pub struct SpvFunction {
    pub id: u32,
    pub name: String,
    pub return_type: SpvTypeId,
    pub instructions: Vec<SpvOp>,
}

#[derive(Debug, Clone)]
pub struct SpvModule {
    pub functions: Vec<SpvFunction>,
    pub global_variables: Vec<(u32, SpvTypeId, String)>,
    pub next_id: u32,
}

impl SpvModule {
    pub fn new() -> Self { Self { functions: Vec::new(), global_variables: Vec::new(), next_id: 1 } }
    pub fn alloc_id(&mut self) -> u32 { let id = self.next_id; self.next_id += 1; id }
}

pub struct SpvCodegen {
    pub module: SpvModule,
    pub var_map: HashMap<String, u32>,
    pub current_fn_instrs: Vec<SpvOp>,
    pub label_counter: u32,
}

impl SpvCodegen {
    pub fn new() -> Self { Self { module: SpvModule::new(), var_map: HashMap::new(), current_fn_instrs: Vec::new(), label_counter: 0 } }
    pub fn new_label(&mut self) -> u32 { let l = self.label_counter; self.label_counter += 1; l }

    pub fn emit_expr_spv(&mut self, expr: &Expr) -> u32 {
        match expr {
            Expr::FloatLit(v) => {
                let dest = self.module.alloc_id();
                self.current_fn_instrs.push(SpvOp::LoadConst { dest, value: SpvConst::Float(*v as f32) });
                dest
            }
            Expr::IntLit(v) => {
                let dest = self.module.alloc_id();
                self.current_fn_instrs.push(SpvOp::LoadConst { dest, value: SpvConst::Int(*v as i32) });
                dest
            }
            Expr::BoolLit(v) => {
                let dest = self.module.alloc_id();
                self.current_fn_instrs.push(SpvOp::LoadConst { dest, value: SpvConst::Bool(*v) });
                dest
            }
            Expr::Ident(name) => {
                let var_id = *self.var_map.get(name).unwrap_or(&0);
                let dest = self.module.alloc_id();
                self.current_fn_instrs.push(SpvOp::Load { dest, src_var: var_id });
                dest
            }
            Expr::Binary { op, left, right } => {
                let a = self.emit_expr_spv(left);
                let b = self.emit_expr_spv(right);
                let dest = self.module.alloc_id();
                let instr = match op {
                    BinaryOp::Add => SpvOp::Add { dest, a, b },
                    BinaryOp::Sub => SpvOp::Sub { dest, a, b },
                    BinaryOp::Mul => SpvOp::Mul { dest, a, b },
                    BinaryOp::Div => SpvOp::Div { dest, a, b },
                    BinaryOp::Eq => SpvOp::Eq { dest, a, b },
                    BinaryOp::Ne => SpvOp::Ne { dest, a, b },
                    BinaryOp::Lt => SpvOp::Lt { dest, a, b },
                    BinaryOp::Gt => SpvOp::Gt { dest, a, b },
                    BinaryOp::Le => SpvOp::Le { dest, a, b },
                    BinaryOp::Ge => SpvOp::Ge { dest, a, b },
                    _ => SpvOp::Add { dest, a, b }, // fallback
                };
                self.current_fn_instrs.push(instr);
                dest
            }
            Expr::Unary { op, operand } => {
                let src = self.emit_expr_spv(operand);
                let dest = self.module.alloc_id();
                let instr = match op {
                    UnaryOp::Negate => SpvOp::Negate { dest, src },
                    UnaryOp::Not => SpvOp::Not { dest, src },
                    _ => SpvOp::Negate { dest, src },
                };
                self.current_fn_instrs.push(instr);
                dest
            }
            _ => { self.module.alloc_id() }
        }
    }
}

// ============================================================
// SECTION 12: WGSL EMITTER
// ============================================================

pub struct WgslEmitter {
    pub output: String,
    pub indent: usize,
}

impl WgslEmitter {
    pub fn new() -> Self { Self { output: String::new(), indent: 0 } }
    fn writeln(&mut self, s: &str) {
        let indent = "    ".repeat(self.indent);
        self.output.push_str(&indent);
        self.output.push_str(s);
        self.output.push('\n');
    }
    fn indent_in(&mut self) { self.indent += 1; }
    fn indent_out(&mut self) { if self.indent > 0 { self.indent -= 1; } }

    pub fn wgsl_type(ty: &ShaderType) -> &'static str {
        match ty {
            ShaderType::Float => "f32",
            ShaderType::Int => "i32",
            ShaderType::Uint => "u32",
            ShaderType::Bool => "bool",
            ShaderType::Vec2 => "vec2<f32>",
            ShaderType::Vec3 => "vec3<f32>",
            ShaderType::Vec4 => "vec4<f32>",
            ShaderType::IVec2 => "vec2<i32>",
            ShaderType::IVec3 => "vec3<i32>",
            ShaderType::IVec4 => "vec4<i32>",
            ShaderType::Mat2 => "mat2x2<f32>",
            ShaderType::Mat3 => "mat3x3<f32>",
            ShaderType::Mat4 => "mat4x4<f32>",
            ShaderType::Sampler2D => "texture_2d<f32>",
            ShaderType::SamplerCube => "texture_cube<f32>",
            _ => "f32",
        }
    }

    pub fn emit_uniform_wgsl(&mut self, u: &UniformDecl, group: u32) {
        let binding = u.binding.unwrap_or(0);
        self.writeln(&format!("@group({}) @binding({}) var<uniform> {}: {};", group, binding, u.name, Self::wgsl_type(&u.ty)));
    }

    pub fn emit_function_wgsl(&mut self, func: &FunctionDecl) {
        let params: Vec<String> = func.params.iter().map(|p| {
            format!("{}: {}", p.name, Self::wgsl_type(&p.ty))
        }).collect();
        let ret = if func.return_type == ShaderType::Void {
            String::new()
        } else {
            format!(" -> {}", Self::wgsl_type(&func.return_type))
        };
        self.writeln(&format!("fn {}({}){} {{", func.name, params.join(", "), ret));
        self.indent_in();
        for stmt in &func.body { self.emit_stmt_wgsl(stmt); }
        self.indent_out();
        self.writeln("}");
    }

    fn emit_expr_wgsl(&self, expr: &Expr) -> String {
        // WGSL expression emission (simplified)
        match expr {
            Expr::FloatLit(v) => format!("{:.6}f", v),
            Expr::IntLit(v) => format!("{}i", v),
            Expr::BoolLit(v) => v.to_string(),
            Expr::Ident(n) => n.clone(),
            Expr::Binary { op, left, right } => {
                let l = self.emit_expr_wgsl(left);
                let r = self.emit_expr_wgsl(right);
                let sym = match op {
                    BinaryOp::Add => "+", BinaryOp::Sub => "-", BinaryOp::Mul => "*", BinaryOp::Div => "/",
                    BinaryOp::Mod => "%", BinaryOp::Eq => "==", BinaryOp::Ne => "!=",
                    BinaryOp::Lt => "<", BinaryOp::Gt => ">", BinaryOp::Le => "<=", BinaryOp::Ge => ">=",
                    BinaryOp::And => "&&", BinaryOp::Or => "||",
                    BinaryOp::BitAnd => "&", BinaryOp::BitOr => "|", BinaryOp::BitXor => "^",
                    BinaryOp::Shl => "<<", BinaryOp::Shr => ">>",
                };
                format!("({} {} {})", l, sym, r)
            }
            Expr::Call { function, args } => {
                let wgsl_fn = match function.as_str() {
                    "sin" => "sin", "cos" => "cos", "sqrt" => "sqrt", "abs" => "abs",
                    "mix" => "mix", "clamp" => "clamp", "dot" => "dot", "cross" => "cross",
                    "normalize" => "normalize", "length" => "length", "pow" => "pow",
                    "floor" => "floor", "ceil" => "ceil", "fract" => "fract",
                    "smoothstep" => "smoothstep", "max" => "max", "min" => "min",
                    "texture" => "textureSample",
                    other => other,
                };
                let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr_wgsl(a)).collect();
                format!("{}({})", wgsl_fn, arg_strs.join(", "))
            }
            Expr::Field { object, field } => format!("{}.{}", self.emit_expr_wgsl(object), field),
            Expr::Swizzle { object, components } => format!("{}.{}", self.emit_expr_wgsl(object), components),
            Expr::Construction { ty, args } => {
                let arg_strs: Vec<String> = args.iter().map(|a| self.emit_expr_wgsl(a)).collect();
                format!("{}({})", Self::wgsl_type(ty), arg_strs.join(", "))
            }
            _ => String::from("/* expr */"),
        }
    }

    fn emit_stmt_wgsl(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expr(e) => { let s = self.emit_expr_wgsl(e); self.writeln(&format!("{};", s)); }
            Stmt::VarDecl { ty, name, init, qualifier } => {
                let kw = if matches!(qualifier, Some(VarQualifier::Const)) { "let" } else { "var" };
                if let Some(init_expr) = init {
                    self.writeln(&format!("{} {}: {} = {};", kw, name, Self::wgsl_type(ty), self.emit_expr_wgsl(init_expr)));
                } else {
                    self.writeln(&format!("{} {}: {};", kw, name, Self::wgsl_type(ty)));
                }
            }
            Stmt::Return(val) => {
                if let Some(v) = val { self.writeln(&format!("return {};", self.emit_expr_wgsl(v))); }
                else { self.writeln("return;"); }
            }
            Stmt::If { cond, then_body, else_body } => {
                self.writeln(&format!("if ({}) {{", self.emit_expr_wgsl(cond)));
                self.indent_in(); self.emit_stmt_wgsl(then_body); self.indent_out();
                if let Some(e) = else_body {
                    self.writeln("} else {");
                    self.indent_in(); self.emit_stmt_wgsl(e); self.indent_out();
                }
                self.writeln("}");
            }
            Stmt::For { init, cond, step, body } => {
                let cond_str = cond.as_ref().map(|c| self.emit_expr_wgsl(c)).unwrap_or_default();
                self.writeln(&format!("loop {{"));
                self.indent_in();
                self.writeln(&format!("if (!{}) {{ break; }}", cond_str));
                self.emit_stmt_wgsl(body);
                self.indent_out();
                self.writeln("}");
            }
            Stmt::Block(stmts) => {
                self.writeln("{");
                self.indent_in();
                for s in stmts { self.emit_stmt_wgsl(s); }
                self.indent_out();
                self.writeln("}");
            }
            Stmt::Break => self.writeln("break;"),
            Stmt::Continue => self.writeln("continue;"),
            Stmt::Discard => self.writeln("discard;"),
            _ => {}
        }
    }

    pub fn emit_ast_wgsl(&mut self, ast: &ShaderAst) -> String {
        self.writeln("// WGSL generated by ShaderCompiler");
        for u in &ast.uniforms { self.emit_uniform_wgsl(u, 0); }
        for func in &ast.functions { self.emit_function_wgsl(func); self.output.push('\n'); }
        self.output.clone()
    }
}

// ============================================================
// SECTION 13: SHADER REFLECTION
// ============================================================

#[derive(Debug, Clone)]
pub struct ReflectedUniform {
    pub name: String,
    pub ty: ShaderType,
    pub binding: u32,
    pub set: u32,
    pub array_size: u32,
    pub size_bytes: u32,
}

impl ReflectedUniform {
    pub fn size_of_type(ty: &ShaderType) -> u32 {
        match ty {
            ShaderType::Float | ShaderType::Int | ShaderType::Uint | ShaderType::Bool => 4,
            ShaderType::Vec2 | ShaderType::IVec2 | ShaderType::UVec2 => 8,
            ShaderType::Vec3 | ShaderType::IVec3 | ShaderType::UVec3 => 12,
            ShaderType::Vec4 | ShaderType::IVec4 | ShaderType::UVec4 => 16,
            ShaderType::Mat2 => 16,
            ShaderType::Mat3 => 36,
            ShaderType::Mat4 => 64,
            _ => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReflectedAttribute {
    pub name: String,
    pub ty: ShaderType,
    pub location: u32,
    pub format: VertexFormat,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VertexFormat {
    Float32, Float32x2, Float32x3, Float32x4,
    Sint32, Sint32x2, Sint32x3, Sint32x4,
    Uint32, Uint32x2, Uint32x3, Uint32x4,
}

#[derive(Debug, Clone, Default)]
pub struct ShaderReflection {
    pub uniforms: Vec<ReflectedUniform>,
    pub attributes: Vec<ReflectedAttribute>,
    pub varyings: Vec<ReflectedAttribute>,
    pub used_builtins: Vec<String>,
    pub struct_layouts: Vec<String>,
    pub push_constants: Vec<ReflectedUniform>,
    pub storage_buffers: Vec<ReflectedUniform>,
    pub entry_points: Vec<String>,
}

impl ShaderReflection {
    pub fn new() -> Self { Self::default() }

    pub fn add_uniform(&mut self, u: ReflectedUniform) {
        self.uniforms.push(u);
    }

    pub fn add_attribute(&mut self, a: ReflectedAttribute) {
        self.attributes.push(a);
    }


}

// ── Shader Program Representation ────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderProgram {
    pub id: u32,
    pub name: String,
    pub vertex_source: String,
    pub fragment_source: String,
    pub geometry_source: Option<String>,
    pub compute_source: Option<String>,
    pub defines: HashMap<String, String>,
    pub version: String,
    pub compiled: bool,
    pub compile_errors: Vec<String>,
    pub compile_warnings: Vec<String>,
}

impl ShaderProgram {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), vertex_source: String::new(), fragment_source: String::new(), geometry_source: None, compute_source: None, defines: HashMap::new(), version: "450".into(), compiled: false, compile_errors: Vec::new(), compile_warnings: Vec::new() }
    }
    pub fn with_vertex(mut self, src: impl Into<String>) -> Self { self.vertex_source = src.into(); self }
    pub fn with_fragment(mut self, src: impl Into<String>) -> Self { self.fragment_source = src.into(); self }
    pub fn add_define(&mut self, key: impl Into<String>, val: impl Into<String>) { self.defines.insert(key.into(), val.into()); }
    pub fn is_compute(&self) -> bool { self.compute_source.is_some() }
    pub fn has_geometry(&self) -> bool { self.geometry_source.is_some() }
    pub fn is_valid(&self) -> bool { self.compiled && self.compile_errors.is_empty() }
    pub fn error_count(&self) -> usize { self.compile_errors.len() }
    pub fn warning_count(&self) -> usize { self.compile_warnings.len() }
    pub fn add_error(&mut self, err: impl Into<String>) { self.compile_errors.push(err.into()); }
    pub fn add_warning(&mut self, warn: impl Into<String>) { self.compile_warnings.push(warn.into()); }
    pub fn clear_diagnostics(&mut self) { self.compile_errors.clear(); self.compile_warnings.clear(); }
    pub fn mark_compiled(&mut self) { self.compiled = true; }
}

#[derive(Clone, Debug)]
pub struct ShaderProgramRegistry {
    pub programs: HashMap<u32, ShaderProgram>,
    pub next_id: u32,
    pub active_program: Option<u32>,
}

impl ShaderProgramRegistry {
    pub fn new() -> Self { Self { programs: HashMap::new(), next_id: 1, active_program: None } }
    pub fn add(&mut self, prog: ShaderProgram) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.programs.insert(id, prog);
        id
    }
    pub fn get(&self, id: u32) -> Option<&ShaderProgram> { self.programs.get(&id) }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut ShaderProgram> { self.programs.get_mut(&id) }
    pub fn find_by_name(&self, name: &str) -> Option<&ShaderProgram> { self.programs.values().find(|p| p.name == name) }
    pub fn count(&self) -> usize { self.programs.len() }
    pub fn valid_count(&self) -> usize { self.programs.values().filter(|p| p.is_valid()).count() }
    pub fn set_active(&mut self, id: u32) { self.active_program = Some(id); }
    pub fn remove(&mut self, id: u32) -> bool { self.programs.remove(&id).is_some() }
}

impl Default for ShaderProgramRegistry {
    fn default() -> Self { Self::new() }
}

// ── Shader Type System ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ShaderDataType {
    Void,
    Bool, BVec2, BVec3, BVec4,
    Int, IVec2, IVec3, IVec4,
    UInt, UVec2, UVec3, UVec4,
    Float, Vec2, Vec3, Vec4,
    Mat2, Mat3, Mat4,
    Mat2x3, Mat2x4, Mat3x2, Mat3x4, Mat4x2, Mat4x3,
    Sampler2D, Sampler3D, SamplerCube, Sampler2DArray, SamplerShadow,
    Image2D, Image3D, ImageCube,
    Struct(String),
    Array(Box<ShaderDataType>, Option<u32>),
}

impl ShaderDataType {
    pub fn byte_size(&self) -> u32 {
        match self {
            ShaderDataType::Bool | ShaderDataType::Int | ShaderDataType::UInt | ShaderDataType::Float => 4,
            ShaderDataType::BVec2 | ShaderDataType::IVec2 | ShaderDataType::UVec2 | ShaderDataType::Vec2 => 8,
            ShaderDataType::BVec3 | ShaderDataType::IVec3 | ShaderDataType::UVec3 | ShaderDataType::Vec3 => 12,
            ShaderDataType::BVec4 | ShaderDataType::IVec4 | ShaderDataType::UVec4 | ShaderDataType::Vec4 => 16,
            ShaderDataType::Mat2 => 16, ShaderDataType::Mat3 => 36, ShaderDataType::Mat4 => 64,
            ShaderDataType::Array(inner, Some(n)) => inner.byte_size() * n,
            _ => 0,
        }
    }
    pub fn is_sampler(&self) -> bool { matches!(self, ShaderDataType::Sampler2D | ShaderDataType::Sampler3D | ShaderDataType::SamplerCube | ShaderDataType::Sampler2DArray | ShaderDataType::SamplerShadow) }
    pub fn is_matrix(&self) -> bool { matches!(self, ShaderDataType::Mat2 | ShaderDataType::Mat3 | ShaderDataType::Mat4 | ShaderDataType::Mat2x3 | ShaderDataType::Mat2x4 | ShaderDataType::Mat3x2 | ShaderDataType::Mat3x4 | ShaderDataType::Mat4x2 | ShaderDataType::Mat4x3) }
    pub fn is_vector(&self) -> bool { matches!(self, ShaderDataType::Vec2 | ShaderDataType::Vec3 | ShaderDataType::Vec4 | ShaderDataType::IVec2 | ShaderDataType::IVec3 | ShaderDataType::IVec4 | ShaderDataType::UVec2 | ShaderDataType::UVec3 | ShaderDataType::UVec4 | ShaderDataType::BVec2 | ShaderDataType::BVec3 | ShaderDataType::BVec4) }
    pub fn glsl_name(&self) -> &'static str {
        match self {
            ShaderDataType::Void => "void", ShaderDataType::Bool => "bool",
            ShaderDataType::Int => "int", ShaderDataType::UInt => "uint",
            ShaderDataType::Float => "float", ShaderDataType::Vec2 => "vec2",
            ShaderDataType::Vec3 => "vec3", ShaderDataType::Vec4 => "vec4",
            ShaderDataType::Mat4 => "mat4", ShaderDataType::Mat3 => "mat3",
            ShaderDataType::Mat2 => "mat2", ShaderDataType::Sampler2D => "sampler2D",
            ShaderDataType::SamplerCube => "samplerCube",
            _ => "unknown",
        }
    }
}

// ── Shader Variable ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderVariable {
    pub name: String,
    pub data_type: ShaderDataType,
    pub qualifier: ShaderQualifier,
    pub location: Option<u32>,
    pub binding: Option<u32>,
    pub set: Option<u32>,
    pub is_builtin: bool,
    pub comment: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShaderQualifier { In, Out, InOut, Uniform, Const, Buffer, Shared }

impl ShaderVariable {
    pub fn uniform(name: impl Into<String>, data_type: ShaderDataType) -> Self {
        Self { name: name.into(), data_type, qualifier: ShaderQualifier::Uniform, location: None, binding: None, set: None, is_builtin: false, comment: String::new() }
    }
    pub fn input(name: impl Into<String>, data_type: ShaderDataType, location: u32) -> Self {
        Self { name: name.into(), data_type, qualifier: ShaderQualifier::In, location: Some(location), binding: None, set: None, is_builtin: false, comment: String::new() }
    }
    pub fn output(name: impl Into<String>, data_type: ShaderDataType, location: u32) -> Self {
        Self { name: name.into(), data_type, qualifier: ShaderQualifier::Out, location: Some(location), binding: None, set: None, is_builtin: false, comment: String::new() }
    }
    pub fn with_binding(mut self, binding: u32, set: u32) -> Self { self.binding = Some(binding); self.set = Some(set); self }
    pub fn is_uniform(&self) -> bool { self.qualifier == ShaderQualifier::Uniform }
    pub fn is_input(&self) -> bool { self.qualifier == ShaderQualifier::In }
    pub fn is_output(&self) -> bool { self.qualifier == ShaderQualifier::Out }
    pub fn size_bytes(&self) -> u32 { self.data_type.byte_size() }
}

// ── Shader Function ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderFunction {
    pub name: String,
    pub return_type: ShaderDataType,
    pub parameters: Vec<ShaderVariable>,
    pub body: String,
    pub is_builtin: bool,
    pub line_start: u32,
    pub line_end: u32,
    pub comment: String,
}

impl ShaderFunction {
    pub fn new(name: impl Into<String>, return_type: ShaderDataType) -> Self {
        Self { name: name.into(), return_type, parameters: Vec::new(), body: String::new(), is_builtin: false, line_start: 0, line_end: 0, comment: String::new() }
    }
    pub fn add_param(&mut self, param: ShaderVariable) { self.parameters.push(param); }
    pub fn param_count(&self) -> usize { self.parameters.len() }
    pub fn is_main(&self) -> bool { self.name == "main" }
    pub fn signature(&self) -> String {
        let params: Vec<_> = self.parameters.iter().map(|p| format!("{} {}", p.data_type.glsl_name(), p.name)).collect();
        format!("{} {}({})", self.return_type.glsl_name(), self.name, params.join(", "))
    }
}

// ── Shader Struct Type ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderStruct {
    pub name: String,
    pub fields: Vec<ShaderVariable>,
    pub comment: String,
    pub std140_layout: bool,
}

impl ShaderStruct {
    pub fn new(name: impl Into<String>) -> Self { Self { name: name.into(), fields: Vec::new(), comment: String::new(), std140_layout: false } }
    pub fn add_field(&mut self, field: ShaderVariable) { self.fields.push(field); }
    pub fn size_bytes(&self) -> u32 { self.fields.iter().map(|f| f.size_bytes()).sum() }
    pub fn std140_size(&self) -> u32 {
        let mut size = 0u32;
        for f in &self.fields {
            let field_size = f.size_bytes();
            let align = field_size.max(4).next_power_of_two().min(16);
            size = (size + align - 1) & !(align - 1);
            size += field_size;
        }
        (size + 15) & !15
    }
    pub fn field_count(&self) -> usize { self.fields.len() }
    pub fn has_field(&self, name: &str) -> bool { self.fields.iter().any(|f| f.name == name) }
    pub fn generate_glsl(&self) -> String {
        let mut s = format!("struct {} {{\n", self.name);
        for f in &self.fields { s += &format!("    {} {};\n", f.data_type.glsl_name(), f.name); }
        s += "};\n";
        s
    }
}

// ── Shader Diagnostic ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderDiagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub line: u32,
    pub column: u32,
    pub file: String,
    pub code: u32,
    pub suggestion: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagnosticKind { Error, Warning, Info, Hint }

impl ShaderDiagnostic {
    pub fn error(msg: impl Into<String>, line: u32) -> Self {
        Self { kind: DiagnosticKind::Error, message: msg.into(), line, column: 0, file: String::new(), code: 0, suggestion: None }
    }
    pub fn warning(msg: impl Into<String>, line: u32) -> Self {
        Self { kind: DiagnosticKind::Warning, message: msg.into(), line, column: 0, file: String::new(), code: 0, suggestion: None }
    }
    pub fn with_suggestion(mut self, s: impl Into<String>) -> Self { self.suggestion = Some(s.into()); self }
    pub fn is_error(&self) -> bool { self.kind == DiagnosticKind::Error }
    pub fn is_warning(&self) -> bool { self.kind == DiagnosticKind::Warning }
    pub fn format(&self) -> String {
        let prefix = match self.kind { DiagnosticKind::Error => "error", DiagnosticKind::Warning => "warning", DiagnosticKind::Info => "info", DiagnosticKind::Hint => "hint" };
        format!("[{}] {}:{} — {}", prefix, self.file, self.line, self.message)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DiagnosticList {
    pub items: Vec<ShaderDiagnostic>,
}

impl DiagnosticList {
    pub fn new() -> Self { Self::default() }
    pub fn push(&mut self, d: ShaderDiagnostic) { self.items.push(d); }
    pub fn errors(&self) -> Vec<&ShaderDiagnostic> { self.items.iter().filter(|d| d.is_error()).collect() }
    pub fn warnings(&self) -> Vec<&ShaderDiagnostic> { self.items.iter().filter(|d| d.is_warning()).collect() }
    pub fn has_errors(&self) -> bool { self.items.iter().any(|d| d.is_error()) }
    pub fn error_count(&self) -> usize { self.items.iter().filter(|d| d.is_error()).count() }
    pub fn warning_count(&self) -> usize { self.items.iter().filter(|d| d.is_warning()).count() }
    pub fn clear(&mut self) { self.items.clear(); }
    pub fn total(&self) -> usize { self.items.len() }
}

// ── Shader Preprocessor ───────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderPreprocessor {
    pub defines: HashMap<String, String>,
    pub include_paths: Vec<String>,
    pub max_include_depth: u32,
    pub strip_comments: bool,
    pub expand_macros: bool,
}

impl ShaderPreprocessor {
    pub fn new() -> Self {
        Self { defines: HashMap::new(), include_paths: Vec::new(), max_include_depth: 10, strip_comments: true, expand_macros: true }
    }
    pub fn define(&mut self, key: impl Into<String>, value: impl Into<String>) { self.defines.insert(key.into(), value.into()); }
    pub fn undefine(&mut self, key: &str) { self.defines.remove(key); }
    pub fn add_include_path(&mut self, path: impl Into<String>) { self.include_paths.push(path.into()); }
    pub fn preprocess(&self, source: &str) -> String {
        let mut output = String::with_capacity(source.len());
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#define") {
                output.push_str(line);
            } else if trimmed.starts_with("//") && self.strip_comments {
                // skip
            } else {
                let mut processed = line.to_string();
                if self.expand_macros {
                    for (k, v) in &self.defines { processed = processed.replace(k.as_str(), v.as_str()); }
                }
                output.push_str(&processed);
            }
            output.push('\n');
        }
        output
    }
    pub fn is_defined(&self, key: &str) -> bool { self.defines.contains_key(key) }
    pub fn define_count(&self) -> usize { self.defines.len() }
}

impl Default for ShaderPreprocessor {
    fn default() -> Self { Self::new() }
}

// ── Shader Uniform Block ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct UniformBlock {
    pub name: String,
    pub binding: u32,
    pub set: u32,
    pub fields: Vec<ShaderVariable>,
    pub std140: bool,
    pub dynamic_offset: bool,
}

impl UniformBlock {
    pub fn new(name: impl Into<String>, binding: u32) -> Self {
        Self { name: name.into(), binding, set: 0, fields: Vec::new(), std140: true, dynamic_offset: false }
    }
    pub fn add_field(&mut self, f: ShaderVariable) { self.fields.push(f); }
    pub fn size_bytes(&self) -> u32 {
        let mut size = 0u32;
        for f in &self.fields {
            let fs = f.size_bytes();
            let align = fs.max(4).next_power_of_two().min(16);
            size = (size + align - 1) & !(align - 1);
            size += fs;
        }
        (size + 15) & !15
    }
    pub fn generate_glsl(&self) -> String {
        let mut s = format!("layout(std140, binding={}) uniform {} {{\n", self.binding, self.name);
        for f in &self.fields { s += &format!("    {} {};\n", f.data_type.glsl_name(), f.name); }
        s += "};\n";
        s
    }
    pub fn field_count(&self) -> usize { self.fields.len() }
}

// ── Shader Pipeline State ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PipelineState {
    pub blend_enabled: bool,
    pub blend_src: BlendFactor,
    pub blend_dst: BlendFactor,
    pub blend_op: BlendOp,
    pub depth_test: bool,
    pub depth_write: bool,
    pub depth_func: CompareFunc,
    pub cull_mode: CullMode,
    pub front_face: FrontFace,
    pub scissor_test: bool,
    pub stencil_test: bool,
    pub polygon_mode: PolygonMode,
    pub line_width: f32,
    pub point_size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlendFactor { Zero, One, SrcAlpha, OneMinusSrcAlpha, DstAlpha, OneMinusDstAlpha, SrcColor, OneMinusSrcColor, ConstantAlpha }
#[derive(Clone, Debug, PartialEq)]
pub enum BlendOp { Add, Subtract, ReverseSubtract, Min, Max }
#[derive(Clone, Debug, PartialEq)]
pub enum CompareFunc { Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always }
#[derive(Clone, Debug, PartialEq)]
pub enum CullMode { None, Front, Back, FrontAndBack }
#[derive(Clone, Debug, PartialEq)]
pub enum FrontFace { Clockwise, CounterClockwise }
#[derive(Clone, Debug, PartialEq)]
pub enum PolygonMode { Fill, Line, Point }

impl Default for PipelineState {
    fn default() -> Self {
        Self { blend_enabled: false, blend_src: BlendFactor::SrcAlpha, blend_dst: BlendFactor::OneMinusSrcAlpha, blend_op: BlendOp::Add, depth_test: true, depth_write: true, depth_func: CompareFunc::Less, cull_mode: CullMode::Back, front_face: FrontFace::CounterClockwise, scissor_test: false, stencil_test: false, polygon_mode: PolygonMode::Fill, line_width: 1.0, point_size: 1.0 }
    }
}

impl PipelineState {
    pub fn transparent() -> Self { Self { blend_enabled: true, depth_write: false, ..Default::default() } }
    pub fn additive() -> Self { Self { blend_enabled: true, blend_src: BlendFactor::One, blend_dst: BlendFactor::One, blend_op: BlendOp::Add, depth_write: false, ..Default::default() } }
    pub fn wireframe() -> Self { Self { polygon_mode: PolygonMode::Line, cull_mode: CullMode::None, ..Default::default() } }
    pub fn opaque() -> Self { Self::default() }
    pub fn is_transparent(&self) -> bool { self.blend_enabled }
    pub fn needs_sort(&self) -> bool { self.blend_enabled && !self.depth_write }
}

// ── Shader Compiler Constants ─────────────────────────────────────────────────

pub const SHADER_MAX_UNIFORMS: usize = 128;
pub const SHADER_MAX_ATTRIBUTES: usize = 32;
pub const SHADER_MAX_VARYINGS: usize = 32;
pub const SHADER_MAX_TEXTURE_UNITS: usize = 32;
pub const SHADER_MAX_UNIFORM_BLOCK_SIZE: u32 = 65536;
pub const SHADER_MAX_INCLUDE_DEPTH: u32 = 16;
pub const SHADER_VERSION_GLSL: &str = "450";
pub const SHADER_VERSION_WGSL: &str = "1.0";
pub const SHADER_MAX_SOURCE_LINES: usize = 10000;
pub const SHADER_MAX_FUNCTIONS: usize = 256;
pub const SHADER_MAX_STRUCTS: usize = 64;
pub const SHADER_MAX_PROGRAMS: usize = 1024;
pub const SHADER_CACHE_SIZE: usize = 256;
pub const SHADER_MAX_PUSH_CONSTANT_SIZE: u32 = 128;

pub fn shader_compiler_info() -> &'static str { "ShaderCompiler v1.0 — GLSL/WGSL, preprocessor, reflection, pipeline state" }


// ── Shader AST Nodes ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum ShaderExpr {
    Literal(ShaderLiteral),
    Ident(String),
    Binary { op: BinaryOp, lhs: Box<ShaderExpr>, rhs: Box<ShaderExpr> },
    Unary { op: UnaryOp, expr: Box<ShaderExpr> },
    Call { name: String, args: Vec<ShaderExpr> },
    Index { array: Box<ShaderExpr>, index: Box<ShaderExpr> },
    Field { object: Box<ShaderExpr>, field: String },
    Ternary { cond: Box<ShaderExpr>, then: Box<ShaderExpr>, else_: Box<ShaderExpr> },
    Cast { to: ShaderDataType, expr: Box<ShaderExpr> },
    Assign { target: Box<ShaderExpr>, value: Box<ShaderExpr> },
}

#[derive(Clone, Debug)]
pub enum ShaderLiteral {
    Int(i64), Float(f64), Bool(bool), String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum BinaryOpEx {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or, BitAnd, BitOr, BitXor,
    Shl, Shr,
    AddAssign, SubAssign, MulAssign, DivAssign,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UnaryOpEx { Neg, Not, BitNot, PreInc, PreDec, PostInc, PostDec }

impl BinaryOpEx {
    pub fn is_comparison(&self) -> bool { matches!(self, BinaryOpEx::Eq | BinaryOpEx::Ne | BinaryOpEx::Lt | BinaryOpEx::Gt | BinaryOpEx::Le | BinaryOpEx::Ge) }
    pub fn is_logical(&self) -> bool { matches!(self, BinaryOpEx::And | BinaryOpEx::Or) }
    pub fn is_arithmetic(&self) -> bool { matches!(self, BinaryOpEx::Add | BinaryOpEx::Sub | BinaryOpEx::Mul | BinaryOpEx::Div | BinaryOpEx::Mod) }
    pub fn glsl_symbol(&self) -> &'static str {
        match self {
            BinaryOpEx::Add => "+", BinaryOpEx::Sub => "-", BinaryOpEx::Mul => "*", BinaryOpEx::Div => "/",
            BinaryOpEx::Mod => "%", BinaryOpEx::Eq => "==", BinaryOpEx::Ne => "!=", BinaryOpEx::Lt => "<",
            BinaryOpEx::Gt => ">", BinaryOpEx::Le => "<=", BinaryOpEx::Ge => ">=", BinaryOpEx::And => "&&",
            BinaryOpEx::Or => "||", BinaryOpEx::BitAnd => "&", BinaryOpEx::BitOr => "|", BinaryOpEx::BitXor => "^",
            BinaryOpEx::Shl => "<<", BinaryOpEx::Shr => ">>", BinaryOpEx::AddAssign => "+=",
            BinaryOpEx::SubAssign => "-=", BinaryOpEx::MulAssign => "*=", BinaryOpEx::DivAssign => "/=",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ShaderStmt {
    Decl { var: ShaderVariable, initializer: Option<ShaderExpr> },
    Expr(ShaderExpr),
    If { cond: ShaderExpr, then: Vec<ShaderStmt>, else_: Option<Vec<ShaderStmt>> },
    For { init: Option<Box<ShaderStmt>>, cond: Option<ShaderExpr>, update: Option<ShaderExpr>, body: Vec<ShaderStmt> },
    While { cond: ShaderExpr, body: Vec<ShaderStmt> },
    Return(Option<ShaderExpr>),
    Break,
    Continue,
    Discard,
    Block(Vec<ShaderStmt>),
}

// ── Shader Code Generator ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderCodeGen {
    pub target: CodeGenTarget,
    pub indent_size: usize,
    pub use_precision_qualifiers: bool,
    pub emit_line_directives: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CodeGenTarget { Glsl450, Glsl300Es, Wgsl, Hlsl50, Msl20 }

impl ShaderCodeGen {
    pub fn new(target: CodeGenTarget) -> Self { Self { target, indent_size: 4, use_precision_qualifiers: false, emit_line_directives: false } }
    pub fn glsl450() -> Self { Self::new(CodeGenTarget::Glsl450) }
    pub fn wgsl() -> Self { Self::new(CodeGenTarget::Wgsl) }
    pub fn version_directive(&self) -> String {
        match self.target {
            CodeGenTarget::Glsl450 => "#version 450\n".into(),
            CodeGenTarget::Glsl300Es => "#version 300 es\nprecision highp float;\n".into(),
            CodeGenTarget::Wgsl => "// WGSL\n".into(),
            CodeGenTarget::Hlsl50 => "// HLSL 5.0\n".into(),
            CodeGenTarget::Msl20 => "#include <metal_stdlib>\nusing namespace metal;\n".into(),
        }
    }
    pub fn emit_uniform(&self, var: &ShaderVariable) -> String {
        match self.target {
            CodeGenTarget::Glsl450 | CodeGenTarget::Glsl300Es => {
                let layout = var.binding.map(|b| format!("layout(binding={}) ", b)).unwrap_or_default();
                format!("{}uniform {} {};", layout, var.data_type.glsl_name(), var.name)
            }
            _ => format!("// uniform {} {}", var.data_type.glsl_name(), var.name),
        }
    }
    pub fn emit_input(&self, var: &ShaderVariable) -> String {
        let loc = var.location.map(|l| format!("layout(location={}) ", l)).unwrap_or_default();
        format!("{}in {} {};", loc, var.data_type.glsl_name(), var.name)
    }
    pub fn emit_output(&self, var: &ShaderVariable) -> String {
        let loc = var.location.map(|l| format!("layout(location={}) ", l)).unwrap_or_default();
        format!("{}out {} {};", loc, var.data_type.glsl_name(), var.name)
    }
    pub fn emit_struct(&self, s: &ShaderStruct) -> String { s.generate_glsl() }
    pub fn emit_function_sig(&self, f: &ShaderFunction) -> String { f.signature() }
    pub fn is_glsl(&self) -> bool { matches!(self.target, CodeGenTarget::Glsl450 | CodeGenTarget::Glsl300Es) }
}

impl Default for ShaderCodeGen {
    fn default() -> Self { Self::glsl450() }
}

// ── Shader Optimizer ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderOptimizer {
    pub level: OptimizationLevel,
    pub fold_constants: bool,
    pub dead_code_elimination: bool,
    pub inline_functions: bool,
    pub common_subexpr_elim: bool,
    pub loop_unroll_threshold: u32,
    pub vectorize: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OptimizationLevel { None, Low, Medium, High, Aggressive }

impl ShaderOptimizer {
    pub fn new(level: OptimizationLevel) -> Self {
        let (fold, dce, inline, cse, unroll, vec) = match level {
            OptimizationLevel::None => (false, false, false, false, 0, false),
            OptimizationLevel::Low => (true, true, false, false, 4, false),
            OptimizationLevel::Medium => (true, true, true, true, 8, false),
            OptimizationLevel::High => (true, true, true, true, 16, true),
            OptimizationLevel::Aggressive => (true, true, true, true, 64, true),
        };
        Self { level, fold_constants: fold, dead_code_elimination: dce, inline_functions: inline, common_subexpr_elim: cse, loop_unroll_threshold: unroll, vectorize: vec }
    }
    pub fn none() -> Self { Self::new(OptimizationLevel::None) }
    pub fn release() -> Self { Self::new(OptimizationLevel::High) }
    pub fn debug() -> Self { Self::new(OptimizationLevel::None) }
    pub fn any_optimization_enabled(&self) -> bool { self.level != OptimizationLevel::None }
}

impl Default for ShaderOptimizer {
    fn default() -> Self { Self::new(OptimizationLevel::Medium) }
}

// ── Shader Cache ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderCacheEntry {
    pub source_hash: u64,
    pub defines_hash: u64,
    pub compiled_bytes: Vec<u8>,
    pub target: CodeGenTarget,
    pub created_at: u64,
    pub hit_count: u32,
}

impl ShaderCacheEntry {
    pub fn new(source_hash: u64, defines_hash: u64, bytes: Vec<u8>, target: CodeGenTarget) -> Self {
        Self { source_hash, defines_hash, compiled_bytes: bytes, target, created_at: 0, hit_count: 0 }
    }
    pub fn is_stale(&self, source_hash: u64, defines_hash: u64) -> bool {
        self.source_hash != source_hash || self.defines_hash != defines_hash
    }
    pub fn record_hit(&mut self) { self.hit_count += 1; }
    pub fn size_bytes(&self) -> usize { self.compiled_bytes.len() }
}

#[derive(Clone, Debug)]
pub struct ShaderCache {
    pub entries: HashMap<String, ShaderCacheEntry>,
    pub max_size: usize,
    pub total_hits: u64,
    pub total_misses: u64,
    pub eviction_policy: CacheEvictionPolicy,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CacheEvictionPolicy { Lru, Fifo, LeastHits }

impl ShaderCache {
    pub fn new(max_size: usize) -> Self { Self { entries: HashMap::new(), max_size, total_hits: 0, total_misses: 0, eviction_policy: CacheEvictionPolicy::Lru } }
    pub fn get(&mut self, key: &str) -> Option<&ShaderCacheEntry> {
        if let Some(e) = self.entries.get_mut(key) { e.record_hit(); self.total_hits += 1; Some(e) } else { self.total_misses += 1; None }
    }
    pub fn insert(&mut self, key: String, entry: ShaderCacheEntry) {
        if self.entries.len() >= self.max_size { self.evict(); }
        self.entries.insert(key, entry);
    }
    fn evict(&mut self) {
        if let Some(key) = self.entries.keys().next().cloned() { self.entries.remove(&key); }
    }
    pub fn hit_rate(&self) -> f32 {
        let total = self.total_hits + self.total_misses;
        if total == 0 { 0.0 } else { self.total_hits as f32 / total as f32 }
    }
    pub fn clear(&mut self) { self.entries.clear(); }
    pub fn size(&self) -> usize { self.entries.len() }
    pub fn total_bytes(&self) -> usize { self.entries.values().map(|e| e.size_bytes()).sum() }
}

impl Default for ShaderCache {
    fn default() -> Self { Self::new(SHADER_CACHE_SIZE) }
}

// ── Shader Template System ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderTemplate {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub vertex_template: String,
    pub fragment_template: String,
    pub parameters: Vec<ShaderTemplateParam>,
    pub category: String,
    pub tags: Vec<String>,
    pub is_builtin: bool,
}

#[derive(Clone, Debug)]
pub struct ShaderTemplateParam {
    pub name: String,
    pub param_type: TemplateParamType,
    pub default_value: String,
    pub description: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TemplateParamType { Bool, Int, Float, String, Color, Texture, Vec2, Vec3, Vec4 }

impl ShaderTemplate {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), description: String::new(), vertex_template: String::new(), fragment_template: String::new(), parameters: Vec::new(), category: "general".into(), tags: Vec::new(), is_builtin: false }
    }
    pub fn add_param(&mut self, param: ShaderTemplateParam) { self.parameters.push(param); }
    pub fn instantiate(&self, params: &HashMap<String, String>) -> (String, String) {
        let mut vert = self.vertex_template.clone();
        let mut frag = self.fragment_template.clone();
        for p in &self.parameters {
            let val = params.get(&p.name).unwrap_or(&p.default_value);
            let placeholder = format!("{{{{{}}}}}", p.name);
            vert = vert.replace(&placeholder, val);
            frag = frag.replace(&placeholder, val);
        }
        (vert, frag)
    }
    pub fn param_count(&self) -> usize { self.parameters.len() }
    pub fn required_params(&self) -> Vec<&ShaderTemplateParam> { self.parameters.iter().filter(|p| p.required).collect() }
}

#[derive(Clone, Debug)]
pub struct ShaderTemplateLibrary {
    pub templates: HashMap<u32, ShaderTemplate>,
    pub next_id: u32,
    pub categories: HashSet<String>,
}

impl ShaderTemplateLibrary {
    pub fn new() -> Self { Self { templates: HashMap::new(), next_id: 1, categories: HashSet::new() } }
    pub fn add(&mut self, mut t: ShaderTemplate) -> u32 {
        let id = self.next_id; self.next_id += 1;
        t.id = id;
        self.categories.insert(t.category.clone());
        self.templates.insert(id, t);
        id
    }
    pub fn get(&self, id: u32) -> Option<&ShaderTemplate> { self.templates.get(&id) }
    pub fn by_category(&self, cat: &str) -> Vec<&ShaderTemplate> { self.templates.values().filter(|t| t.category == cat).collect() }
    pub fn find_by_name(&self, name: &str) -> Option<&ShaderTemplate> { self.templates.values().find(|t| t.name == name) }
    pub fn count(&self) -> usize { self.templates.len() }
}

impl Default for ShaderTemplateLibrary {
    fn default() -> Self { Self::new() }
}

// ── Shader Live Reload ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderFileWatcher {
    pub watched_files: HashMap<String, ShaderWatchEntry>,
    pub reload_queue: VecDeque<String>,
    pub auto_reload: bool,
    pub debounce_ms: u32,
}

#[derive(Clone, Debug)]
pub struct ShaderWatchEntry {
    pub path: String,
    pub last_modified: u64,
    pub shader_ids: Vec<u32>,
    pub reload_count: u32,
}

impl ShaderFileWatcher {
    pub fn new() -> Self { Self { watched_files: HashMap::new(), reload_queue: VecDeque::new(), auto_reload: true, debounce_ms: 500 } }
    pub fn watch(&mut self, path: impl Into<String>, shader_id: u32) {
        let p = path.into();
        self.watched_files.entry(p.clone()).or_insert(ShaderWatchEntry { path: p, last_modified: 0, shader_ids: Vec::new(), reload_count: 0 }).shader_ids.push(shader_id);
    }
    pub fn mark_modified(&mut self, path: &str) {
        if let Some(entry) = self.watched_files.get_mut(path) {
            entry.reload_count += 1;
            if !self.reload_queue.contains(&path.to_string()) { self.reload_queue.push_back(path.to_string()); }
        }
    }
    pub fn next_reload(&mut self) -> Option<String> { self.reload_queue.pop_front() }
    pub fn shader_ids_for(&self, path: &str) -> Vec<u32> { self.watched_files.get(path).map(|e| e.shader_ids.clone()).unwrap_or_default() }
    pub fn unwatch(&mut self, path: &str) { self.watched_files.remove(path); }
    pub fn watched_count(&self) -> usize { self.watched_files.len() }
}

impl Default for ShaderFileWatcher {
    fn default() -> Self { Self::new() }
}

// ── Shader Variant System ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderVariant {
    pub id: u32,
    pub base_program_id: u32,
    pub keywords: Vec<String>,
    pub compiled_id: Option<u32>,
    pub is_fallback: bool,
}

impl ShaderVariant {
    pub fn new(id: u32, base_id: u32) -> Self { Self { id, base_program_id: base_id, keywords: Vec::new(), compiled_id: None, is_fallback: false } }
    pub fn with_keyword(mut self, kw: impl Into<String>) -> Self { self.keywords.push(kw.into()); self }
    pub fn matches_keywords(&self, keywords: &[&str]) -> bool { keywords.iter().all(|kw| self.keywords.iter().any(|k| k == kw)) }
    pub fn keyword_hash(&self) -> u64 {
        let mut sorted = self.keywords.clone(); sorted.sort();
        sorted.iter().fold(0u64, |h, k| h.wrapping_mul(31).wrapping_add(k.bytes().map(|b| b as u64).sum::<u64>()))
    }
}

#[derive(Clone, Debug)]
pub struct ShaderVariantCollection {
    pub base_id: u32,
    pub variants: Vec<ShaderVariant>,
    pub fallback_id: Option<u32>,
    pub next_id: u32,
}

impl ShaderVariantCollection {
    pub fn new(base_id: u32) -> Self { Self { base_id, variants: Vec::new(), fallback_id: None, next_id: 1 } }
    pub fn add_variant(&mut self, keywords: Vec<String>) -> u32 {
        let id = self.next_id; self.next_id += 1;
        let mut v = ShaderVariant::new(id, self.base_id);
        v.keywords = keywords;
        self.variants.push(v);
        id
    }
    pub fn find_best_match(&self, keywords: &[&str]) -> Option<&ShaderVariant> {
        let mut best: Option<&ShaderVariant> = None;
        let mut best_matches = 0;
        for v in &self.variants {
            let matches = keywords.iter().filter(|kw| v.keywords.iter().any(|k| k == *kw)).count();
            if matches > best_matches { best_matches = matches; best = Some(v); }
        }
        best.or_else(|| self.fallback_id.and_then(|id| self.variants.iter().find(|v| v.id == id)))
    }
    pub fn variant_count(&self) -> usize { self.variants.len() }
    pub fn set_fallback(&mut self, id: u32) { self.fallback_id = Some(id); }
}

// ── Shader include resolver ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderIncludeResolver {
    pub virtual_fs: HashMap<String, String>,
    pub include_paths: Vec<String>,
    pub resolved_cache: HashMap<String, String>,
    pub max_size_bytes: usize,
}

impl ShaderIncludeResolver {
    pub fn new() -> Self { Self { virtual_fs: HashMap::new(), include_paths: Vec::new(), resolved_cache: HashMap::new(), max_size_bytes: 1024 * 1024 } }
    pub fn register_virtual(&mut self, path: impl Into<String>, content: impl Into<String>) { self.virtual_fs.insert(path.into(), content.into()); }
    pub fn add_include_path(&mut self, path: impl Into<String>) { self.include_paths.push(path.into()); }
    pub fn resolve(&self, include_path: &str) -> Option<&str> {
        self.resolved_cache.get(include_path).map(|s| s.as_str()).or_else(|| self.virtual_fs.get(include_path).map(|s| s.as_str()))
    }
    pub fn cache_result(&mut self, include_path: String, content: String) { self.resolved_cache.insert(include_path, content); }
    pub fn clear_cache(&mut self) { self.resolved_cache.clear(); }
    pub fn virtual_file_count(&self) -> usize { self.virtual_fs.len() }
}

impl Default for ShaderIncludeResolver {
    fn default() -> Self { Self::new() }
}

// ── More shader constants ─────────────────────────────────────────────────────

pub const SHADER_VARIANT_MAX_KEYWORDS: usize = 16;
pub const SHADER_TEMPLATE_MAX_PARAMS: usize = 32;
pub const SHADER_WATCHER_DEBOUNCE_DEFAULT: u32 = 500;
pub const SHADER_OPTIMIZER_UNROLL_DEFAULT: u32 = 8;
pub const SHADER_AST_MAX_DEPTH: usize = 64;
pub const SHADER_MAX_VARIANTS_PER_PROGRAM: usize = 256;
pub const SHADER_INCLUDE_RESOLVER_CACHE_MAX: usize = 512;
pub const SHADER_CODEGEN_INDENT_DEFAULT: usize = 4;
pub const SHADER_DIAGNOSTIC_MAX: usize = 1000;

pub fn shader_targets() -> &'static [&'static str] { &["glsl450", "glsl300es", "wgsl", "hlsl50", "msl20"] }
pub fn shader_target_count() -> usize { shader_targets().len() }
pub fn is_gpu_shader_target(target: &CodeGenTarget) -> bool { !matches!(target, CodeGenTarget::Wgsl) || true }


// ── Material System ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Material {
    pub id: u32,
    pub name: String,
    pub shader_program_id: u32,
    pub textures: Vec<MaterialTexture>,
    pub properties: HashMap<String, MaterialProperty>,
    pub pipeline: PipelineState,
    pub render_queue: u32,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub instanced: bool,
    pub double_sided: bool,
}

#[derive(Clone, Debug)]
pub struct MaterialTexture {
    pub name: String,
    pub texture_id: u32,
    pub slot: u32,
    pub sampler_type: SamplerType,
    pub uv_channel: u32,
}

#[derive(Clone, Debug)]
pub enum MaterialProperty {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    Bool(bool),
    Color([f32; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SamplerType { Linear, Nearest, LinearMipmap, Trilinear, Anisotropic }

impl Material {
    pub fn new(id: u32, name: impl Into<String>, shader_id: u32) -> Self {
        Self { id, name: name.into(), shader_program_id: shader_id, textures: Vec::new(), properties: HashMap::new(), pipeline: PipelineState::default(), render_queue: 2000, cast_shadows: true, receive_shadows: true, instanced: false, double_sided: false }
    }
    pub fn add_texture(&mut self, name: impl Into<String>, tex_id: u32, slot: u32) {
        self.textures.push(MaterialTexture { name: name.into(), texture_id: tex_id, slot, sampler_type: SamplerType::Trilinear, uv_channel: 0 });
    }
    pub fn set_float(&mut self, name: impl Into<String>, value: f32) { self.properties.insert(name.into(), MaterialProperty::Float(value)); }
    pub fn set_color(&mut self, name: impl Into<String>, r: f32, g: f32, b: f32, a: f32) { self.properties.insert(name.into(), MaterialProperty::Color([r, g, b, a])); }
    pub fn set_vec4(&mut self, name: impl Into<String>, v: [f32; 4]) { self.properties.insert(name.into(), MaterialProperty::Vec4(v)); }
    pub fn is_transparent(&self) -> bool { self.pipeline.is_transparent() }
    pub fn is_opaque(&self) -> bool { !self.is_transparent() }
    pub fn texture_count(&self) -> usize { self.textures.len() }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn make_transparent(&mut self) { self.pipeline = PipelineState::transparent(); self.render_queue = 3000; }
    pub fn make_additive(&mut self) { self.pipeline = PipelineState::additive(); self.render_queue = 3500; }
}

#[derive(Clone, Debug)]
pub struct MaterialLibrary {
    pub materials: HashMap<u32, Material>,
    pub next_id: u32,
    pub default_material_id: u32,
}

impl MaterialLibrary {
    pub fn new() -> Self { Self { materials: HashMap::new(), next_id: 1, default_material_id: 0 } }
    pub fn add(&mut self, mat: Material) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.materials.insert(id, mat);
        id
    }
    pub fn get(&self, id: u32) -> Option<&Material> { self.materials.get(&id) }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Material> { self.materials.get_mut(&id) }
    pub fn find_by_name(&self, name: &str) -> Option<&Material> { self.materials.values().find(|m| m.name == name) }
    pub fn opaque_materials(&self) -> Vec<&Material> { self.materials.values().filter(|m| m.is_opaque()).collect() }
    pub fn transparent_materials(&self) -> Vec<&Material> { self.materials.values().filter(|m| m.is_transparent()).collect() }
    pub fn count(&self) -> usize { self.materials.len() }
    pub fn remove(&mut self, id: u32) -> bool { self.materials.remove(&id).is_some() }
    pub fn set_default(&mut self, id: u32) { self.default_material_id = id; }
}

impl Default for MaterialLibrary {
    fn default() -> Self { Self::new() }
}

// ── Render Pass System ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct RenderPass {
    pub id: u32,
    pub name: String,
    pub pass_type: RenderPassType,
    pub color_attachments: Vec<ColorAttachment>,
    pub depth_attachment: Option<DepthAttachment>,
    pub clear_color: [f32; 4],
    pub clear_depth: f32,
    pub clear_stencil: u8,
    pub viewport: (u32, u32, u32, u32),
    pub enabled: bool,
    pub order: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenderPassType { Opaque, Transparent, Shadow, PostProcess, Ui, Compute, Custom }

#[derive(Clone, Debug)]
pub struct ColorAttachment {
    pub texture_id: u32,
    pub format: AttachmentFormat,
    pub load_op: LoadOp,
    pub store_op: StoreOp,
    pub mip_level: u32,
    pub layer: u32,
}

#[derive(Clone, Debug)]
pub struct DepthAttachment {
    pub texture_id: u32,
    pub format: AttachmentFormat,
    pub load_op: LoadOp,
    pub store_op: StoreOp,
    pub read_only: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttachmentFormat { Rgba8, Rgba16F, Rgba32F, R8, R16F, R32F, Rg8, Rg16F, Depth16, Depth24, Depth32F, Depth24Stencil8 }

#[derive(Clone, Debug, PartialEq)]
pub enum LoadOp { Load, Clear, DontCare }
#[derive(Clone, Debug, PartialEq)]
pub enum StoreOp { Store, DontCare }

impl RenderPass {
    pub fn new(id: u32, name: impl Into<String>, pass_type: RenderPassType) -> Self {
        Self { id, name: name.into(), pass_type, color_attachments: Vec::new(), depth_attachment: None, clear_color: [0.0, 0.0, 0.0, 1.0], clear_depth: 1.0, clear_stencil: 0, viewport: (0, 0, 1920, 1080), enabled: true, order: 0 }
    }
    pub fn add_color_attachment(&mut self, tex_id: u32, format: AttachmentFormat) {
        self.color_attachments.push(ColorAttachment { texture_id: tex_id, format, load_op: LoadOp::Clear, store_op: StoreOp::Store, mip_level: 0, layer: 0 });
    }
    pub fn set_depth(&mut self, tex_id: u32) {
        self.depth_attachment = Some(DepthAttachment { texture_id: tex_id, format: AttachmentFormat::Depth32F, load_op: LoadOp::Clear, store_op: StoreOp::Store, read_only: false });
    }
    pub fn is_shadow_pass(&self) -> bool { self.pass_type == RenderPassType::Shadow }
    pub fn is_postprocess(&self) -> bool { self.pass_type == RenderPassType::PostProcess }
    pub fn attachment_count(&self) -> usize { self.color_attachments.len() + if self.depth_attachment.is_some() { 1 } else { 0 } }
    pub fn set_viewport(&mut self, x: u32, y: u32, w: u32, h: u32) { self.viewport = (x, y, w, h); }
}

#[derive(Clone, Debug)]
pub struct RenderPipeline {
    pub passes: Vec<RenderPass>,
    pub next_id: u32,
}

impl RenderPipeline {
    pub fn new() -> Self { Self { passes: Vec::new(), next_id: 1 } }
    pub fn add_pass(&mut self, pass: RenderPass) { self.passes.push(pass); }
    pub fn sorted_passes(&self) -> Vec<&RenderPass> {
        let mut sorted: Vec<_> = self.passes.iter().filter(|p| p.enabled).collect();
        sorted.sort_by_key(|p| p.order);
        sorted
    }
    pub fn pass_count(&self) -> usize { self.passes.len() }
    pub fn find_by_type(&self, pass_type: &RenderPassType) -> Vec<&RenderPass> {
        self.passes.iter().filter(|p| &p.pass_type == pass_type).collect()
    }
    pub fn enable_pass(&mut self, id: u32) { if let Some(p) = self.passes.iter_mut().find(|p| p.id == id) { p.enabled = true; } }
    pub fn disable_pass(&mut self, id: u32) { if let Some(p) = self.passes.iter_mut().find(|p| p.id == id) { p.enabled = false; } }
}

impl Default for RenderPipeline {
    fn default() -> Self { Self::new() }
}

// ── Post-Process Effect ───────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PostProcessEffect {
    pub id: u32,
    pub name: String,
    pub effect_type: PostEffectType,
    pub shader_program_id: u32,
    pub properties: HashMap<String, f32>,
    pub enabled: bool,
    pub order: u32,
    pub input_texture: u32,
    pub output_texture: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PostEffectType { Bloom, Tonemap, Vignette, ChromaticAberration, DepthOfField, MotionBlur, Ssao, Fxaa, Custom }

impl PostProcessEffect {
    pub fn new(id: u32, name: impl Into<String>, effect_type: PostEffectType, shader_id: u32) -> Self {
        Self { id, name: name.into(), effect_type, shader_program_id: shader_id, properties: HashMap::new(), enabled: true, order: 0, input_texture: 0, output_texture: 0 }
    }
    pub fn set_property(&mut self, key: impl Into<String>, val: f32) { self.properties.insert(key.into(), val); }
    pub fn get_property(&self, key: &str) -> f32 { self.properties.get(key).copied().unwrap_or(0.0) }
    pub fn bloom_intensity(&self) -> f32 { self.get_property("intensity") }
    pub fn vignette_strength(&self) -> f32 { self.get_property("strength") }
    pub fn is_bloom(&self) -> bool { self.effect_type == PostEffectType::Bloom }
    pub fn is_tonemap(&self) -> bool { self.effect_type == PostEffectType::Tonemap }
}

#[derive(Clone, Debug)]
pub struct PostProcessStack {
    pub effects: Vec<PostProcessEffect>,
    pub enabled: bool,
}

impl PostProcessStack {
    pub fn new() -> Self { Self { effects: Vec::new(), enabled: true } }
    pub fn add(&mut self, effect: PostProcessEffect) { self.effects.push(effect); }
    pub fn enabled_effects(&self) -> Vec<&PostProcessEffect> {
        let mut v: Vec<_> = self.effects.iter().filter(|e| e.enabled).collect();
        v.sort_by_key(|e| e.order);
        v
    }
    pub fn enable_effect(&mut self, id: u32) { if let Some(e) = self.effects.iter_mut().find(|e| e.id == id) { e.enabled = true; } }
    pub fn disable_effect(&mut self, id: u32) { if let Some(e) = self.effects.iter_mut().find(|e| e.id == id) { e.enabled = false; } }
    pub fn effect_count(&self) -> usize { self.effects.len() }
    pub fn active_count(&self) -> usize { self.effects.iter().filter(|e| e.enabled).count() }
}

impl Default for PostProcessStack {
    fn default() -> Self { Self::new() }
}

// ── Shader Node Graph ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderNode {
    pub id: u32,
    pub node_type: ShaderNodeType,
    pub label: String,
    pub position: (f32, f32),
    pub inputs: Vec<ShaderNodePort>,
    pub outputs: Vec<ShaderNodePort>,
    pub properties: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShaderNodeType {
    Input, Output, Math, Texture, Color, Constant, Mix, Clamp, Normalize, Length, Dot, Cross, Reflect, Refract, Fresnel, NormalMap, VertexShader, FragmentShader, Custom(String),
}

#[derive(Clone, Debug)]
pub struct ShaderNodePort {
    pub id: u32,
    pub name: String,
    pub port_type: ShaderDataType,
    pub connected_to: Option<(u32, u32)>,
    pub default_value: String,
}

impl ShaderNode {
    pub fn new(id: u32, node_type: ShaderNodeType, label: impl Into<String>) -> Self {
        Self { id, node_type, label: label.into(), position: (0.0, 0.0), inputs: Vec::new(), outputs: Vec::new(), properties: HashMap::new() }
    }
    pub fn add_input(&mut self, name: impl Into<String>, port_type: ShaderDataType) -> u32 {
        let pid = self.inputs.len() as u32;
        self.inputs.push(ShaderNodePort { id: pid, name: name.into(), port_type, connected_to: None, default_value: "0.0".into() });
        pid
    }
    pub fn add_output(&mut self, name: impl Into<String>, port_type: ShaderDataType) -> u32 {
        let pid = self.outputs.len() as u32;
        self.outputs.push(ShaderNodePort { id: pid, name: name.into(), port_type, connected_to: None, default_value: "0.0".into() });
        pid
    }
    pub fn set_property(&mut self, key: impl Into<String>, val: impl Into<String>) { self.properties.insert(key.into(), val.into()); }
    pub fn is_output_node(&self) -> bool { self.node_type == ShaderNodeType::Output }
    pub fn is_input_node(&self) -> bool { self.node_type == ShaderNodeType::Input }
    pub fn input_count(&self) -> usize { self.inputs.len() }
    pub fn output_count(&self) -> usize { self.outputs.len() }
}

#[derive(Clone, Debug)]
pub struct ShaderNodeGraph {
    pub nodes: HashMap<u32, ShaderNode>,
    pub connections: Vec<(u32, u32, u32, u32)>,
    pub next_id: u32,
    pub name: String,
}

impl ShaderNodeGraph {
    pub fn new(name: impl Into<String>) -> Self { Self { nodes: HashMap::new(), connections: Vec::new(), next_id: 1, name: name.into() } }
    pub fn add_node(&mut self, node_type: ShaderNodeType, label: impl Into<String>) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.nodes.insert(id, ShaderNode::new(id, node_type, label));
        id
    }
    pub fn connect(&mut self, from_node: u32, from_port: u32, to_node: u32, to_port: u32) {
        self.connections.push((from_node, from_port, to_node, to_port));
        if let Some(node) = self.nodes.get_mut(&to_node) {
            if let Some(port) = node.inputs.get_mut(to_port as usize) { port.connected_to = Some((from_node, from_port)); }
        }
    }
    pub fn disconnect(&mut self, to_node: u32, to_port: u32) {
        self.connections.retain(|(_, _, tn, tp)| !(*tn == to_node && *tp == to_port));
        if let Some(node) = self.nodes.get_mut(&to_node) {
            if let Some(port) = node.inputs.get_mut(to_port as usize) { port.connected_to = None; }
        }
    }
    pub fn remove_node(&mut self, id: u32) {
        self.nodes.remove(&id);
        self.connections.retain(|(a, _, b, _)| *a != id && *b != id);
    }
    pub fn node_count(&self) -> usize { self.nodes.len() }
    pub fn connection_count(&self) -> usize { self.connections.len() }
    pub fn output_node(&self) -> Option<&ShaderNode> { self.nodes.values().find(|n| n.is_output_node()) }
    pub fn move_node(&mut self, id: u32, x: f32, y: f32) { if let Some(n) = self.nodes.get_mut(&id) { n.position = (x, y); } }
}

impl Default for ShaderNodeGraph {
    fn default() -> Self { Self::new("new_graph") }
}

// ── More constants ────────────────────────────────────────────────────────────

pub const MATERIAL_MAX_TEXTURES: usize = 16;
pub const MATERIAL_MAX_PROPERTIES: usize = 64;
pub const RENDER_PASS_MAX: usize = 32;
pub const POST_PROCESS_EFFECT_MAX: usize = 16;
pub const SHADER_NODE_GRAPH_MAX_NODES: usize = 512;
pub const SHADER_NODE_GRAPH_MAX_CONNECTIONS: usize = 1024;
pub const MATERIAL_OPAQUE_QUEUE: u32 = 2000;
pub const MATERIAL_TRANSPARENT_QUEUE: u32 = 3000;
pub const MATERIAL_OVERLAY_QUEUE: u32 = 4000;

pub fn attachment_format_name(fmt: &AttachmentFormat) -> &'static str {
    match fmt {
        AttachmentFormat::Rgba8 => "RGBA8", AttachmentFormat::Rgba16F => "RGBA16F",
        AttachmentFormat::Rgba32F => "RGBA32F", AttachmentFormat::Depth32F => "Depth32F",
        AttachmentFormat::Depth24Stencil8 => "Depth24Stencil8", _ => "Unknown",
    }
}
pub fn blend_factor_glsl(f: &BlendFactor) -> &'static str {
    match f {
        BlendFactor::Zero => "GL_ZERO", BlendFactor::One => "GL_ONE",
        BlendFactor::SrcAlpha => "GL_SRC_ALPHA", BlendFactor::OneMinusSrcAlpha => "GL_ONE_MINUS_SRC_ALPHA",
        _ => "GL_ONE",
    }
}


// ── Texture System ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct TextureDescriptor {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_levels: u32,
    pub array_layers: u32,
    pub format: AttachmentFormat,
    pub tex_type: TextureType,
    pub usage: TextureUsage,
    pub filter: TextureFilter,
    pub wrap_u: WrapMode,
    pub wrap_v: WrapMode,
    pub wrap_w: WrapMode,
    pub anisotropy: u32,
    pub generate_mipmaps: bool,
    pub srgb: bool,
    pub compressed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TextureType { Tex2D, Tex3D, TexCube, Tex2DArray, TexCubeArray }
#[derive(Clone, Debug, PartialEq)]
pub enum TextureUsage { Sampled, Storage, RenderTarget, DepthStencil }
#[derive(Clone, Debug, PartialEq)]
pub enum TextureFilter { Nearest, Linear, Trilinear, Anisotropic }
#[derive(Clone, Debug, PartialEq)]
pub enum WrapMode { Repeat, MirroredRepeat, ClampToEdge, ClampToBorder, MirrorClampToEdge }

impl TextureDescriptor {
    pub fn new_2d(id: u32, name: impl Into<String>, width: u32, height: u32, format: AttachmentFormat) -> Self {
        Self { id, name: name.into(), width, height, depth: 1, mip_levels: 1, array_layers: 1, format, tex_type: TextureType::Tex2D, usage: TextureUsage::Sampled, filter: TextureFilter::Trilinear, wrap_u: WrapMode::Repeat, wrap_v: WrapMode::Repeat, wrap_w: WrapMode::ClampToEdge, anisotropy: 4, generate_mipmaps: true, srgb: false, compressed: false }
    }
    pub fn render_target(id: u32, name: impl Into<String>, w: u32, h: u32, fmt: AttachmentFormat) -> Self {
        let mut t = Self::new_2d(id, name, w, h, fmt); t.usage = TextureUsage::RenderTarget; t.generate_mipmaps = false; t
    }
    pub fn depth_target(id: u32, name: impl Into<String>, w: u32, h: u32) -> Self {
        let mut t = Self::new_2d(id, name, w, h, AttachmentFormat::Depth32F); t.usage = TextureUsage::DepthStencil; t.generate_mipmaps = false; t
    }
    pub fn mip_count_for_size(width: u32, height: u32) -> u32 { (width.max(height) as f32).log2().floor() as u32 + 1 }
    pub fn bytes_per_pixel(&self) -> u32 {
        match self.format {
            AttachmentFormat::Rgba8 => 4, AttachmentFormat::Rgba16F => 8, AttachmentFormat::Rgba32F => 16,
            AttachmentFormat::R8 => 1, AttachmentFormat::R16F => 2, AttachmentFormat::R32F => 4,
            AttachmentFormat::Rg8 => 2, AttachmentFormat::Rg16F => 4,
            AttachmentFormat::Depth32F => 4, _ => 4,
        }
    }
    pub fn total_bytes(&self) -> u64 { self.width as u64 * self.height as u64 * self.depth as u64 * self.bytes_per_pixel() as u64 }
    pub fn is_hdr(&self) -> bool { matches!(self.format, AttachmentFormat::Rgba16F | AttachmentFormat::Rgba32F | AttachmentFormat::R16F | AttachmentFormat::R32F) }
    pub fn is_cubemap(&self) -> bool { self.tex_type == TextureType::TexCube }
    pub fn is_depth(&self) -> bool { matches!(self.format, AttachmentFormat::Depth16 | AttachmentFormat::Depth24 | AttachmentFormat::Depth32F | AttachmentFormat::Depth24Stencil8) }
    pub fn aspect_ratio(&self) -> f32 { if self.height == 0 { 1.0 } else { self.width as f32 / self.height as f32 } }
}

#[derive(Clone, Debug)]
pub struct TextureRegistry {
    pub textures: HashMap<u32, TextureDescriptor>,
    pub next_id: u32,
}

impl TextureRegistry {
    pub fn new() -> Self { Self { textures: HashMap::new(), next_id: 1 } }
    pub fn register(&mut self, mut tex: TextureDescriptor) -> u32 {
        let id = self.next_id; self.next_id += 1;
        tex.id = id;
        self.textures.insert(id, tex);
        id
    }
    pub fn get(&self, id: u32) -> Option<&TextureDescriptor> { self.textures.get(&id) }
    pub fn find_by_name(&self, name: &str) -> Option<&TextureDescriptor> { self.textures.values().find(|t| t.name == name) }
    pub fn total_bytes(&self) -> u64 { self.textures.values().map(|t| t.total_bytes()).sum() }
    pub fn count(&self) -> usize { self.textures.len() }
    pub fn hdr_textures(&self) -> Vec<&TextureDescriptor> { self.textures.values().filter(|t| t.is_hdr()).collect() }
    pub fn depth_textures(&self) -> Vec<&TextureDescriptor> { self.textures.values().filter(|t| t.is_depth()).collect() }
    pub fn remove(&mut self, id: u32) -> bool { self.textures.remove(&id).is_some() }
}

impl Default for TextureRegistry {
    fn default() -> Self { Self::new() }
}

// ── Sampler System ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SamplerDescriptor {
    pub id: u32,
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    pub mip_filter: TextureFilter,
    pub wrap_u: WrapMode,
    pub wrap_v: WrapMode,
    pub wrap_w: WrapMode,
    pub anisotropy: u32,
    pub border_color: [f32; 4],
    pub lod_min: f32,
    pub lod_max: f32,
    pub lod_bias: f32,
    pub compare_op: Option<CompareFunc>,
}

impl SamplerDescriptor {
    pub fn default_linear() -> Self {
        Self { id: 0, min_filter: TextureFilter::Linear, mag_filter: TextureFilter::Linear, mip_filter: TextureFilter::Linear, wrap_u: WrapMode::Repeat, wrap_v: WrapMode::Repeat, wrap_w: WrapMode::Repeat, anisotropy: 1, border_color: [0.0; 4], lod_min: 0.0, lod_max: 1000.0, lod_bias: 0.0, compare_op: None }
    }
    pub fn default_nearest() -> Self { Self { min_filter: TextureFilter::Nearest, mag_filter: TextureFilter::Nearest, mip_filter: TextureFilter::Nearest, ..Self::default_linear() } }
    pub fn shadow() -> Self { Self { compare_op: Some(CompareFunc::Less), ..Self::default_linear() } }
    pub fn clamp_to_edge() -> Self { Self { wrap_u: WrapMode::ClampToEdge, wrap_v: WrapMode::ClampToEdge, wrap_w: WrapMode::ClampToEdge, ..Self::default_linear() } }
    pub fn anisotropic_16() -> Self { Self { anisotropy: 16, ..Self::default_linear() } }
    pub fn is_shadow_sampler(&self) -> bool { self.compare_op.is_some() }
    pub fn is_anisotropic(&self) -> bool { self.anisotropy > 1 }
}

// ── Vertex Layout ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct VertexAttribute {
    pub location: u32,
    pub name: String,
    pub format: VertexAttribFormat,
    pub offset: u32,
    pub normalized: bool,
}

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum VertexAttribFormat {
    Float1, Float2, Float3, Float4,
    Sint1, Sint2, Sint3, Sint4,
    Uint1, Uint2, Uint3, Uint4,
    Unorm4, Snorm4,
    Half2, Half4,
}

impl VertexAttribFormat {
    pub fn size_bytes(self) -> u32 {
        match self {
            VertexAttribFormat::Float1 | VertexAttribFormat::Sint1 | VertexAttribFormat::Uint1 => 4,
            VertexAttribFormat::Float2 | VertexAttribFormat::Sint2 | VertexAttribFormat::Uint2 | VertexAttribFormat::Half4 => 8,
            VertexAttribFormat::Float3 | VertexAttribFormat::Sint3 | VertexAttribFormat::Uint3 => 12,
            VertexAttribFormat::Float4 | VertexAttribFormat::Sint4 | VertexAttribFormat::Uint4 | VertexAttribFormat::Unorm4 | VertexAttribFormat::Snorm4 => 16,
            VertexAttribFormat::Half2 => 4,
        }
    }
    pub fn component_count(self) -> u32 {
        match self {
            VertexAttribFormat::Float1 | VertexAttribFormat::Sint1 | VertexAttribFormat::Uint1 => 1,
            VertexAttribFormat::Float2 | VertexAttribFormat::Sint2 | VertexAttribFormat::Uint2 | VertexAttribFormat::Half2 => 2,
            VertexAttribFormat::Float3 | VertexAttribFormat::Sint3 | VertexAttribFormat::Uint3 => 3,
            _ => 4,
        }
    }
    pub fn glsl_type_name(self) -> &'static str {
        match self {
            VertexAttribFormat::Float1 => "float", VertexAttribFormat::Float2 => "vec2",
            VertexAttribFormat::Float3 => "vec3", VertexAttribFormat::Float4 => "vec4",
            VertexAttribFormat::Sint1 => "int", VertexAttribFormat::Sint2 => "ivec2",
            VertexAttribFormat::Sint3 => "ivec3", VertexAttribFormat::Sint4 => "ivec4",
            VertexAttribFormat::Uint1 => "uint", VertexAttribFormat::Uint2 => "uvec2",
            VertexAttribFormat::Uint3 => "uvec3", VertexAttribFormat::Uint4 => "uvec4",
            _ => "vec4",
        }
    }
}

#[derive(Clone, Debug)]
pub struct VertexLayout {
    pub attributes: Vec<VertexAttribute>,
    pub stride: u32,
    pub instanced: bool,
    pub instance_rate: u32,
}

impl VertexLayout {
    pub fn new() -> Self { Self { attributes: Vec::new(), stride: 0, instanced: false, instance_rate: 1 } }
    pub fn add_attribute(&mut self, location: u32, name: impl Into<String>, format: VertexAttribFormat) {
        let offset = self.stride;
        self.stride += format.size_bytes();
        self.attributes.push(VertexAttribute { location, name: name.into(), format, offset, normalized: false });
    }
    pub fn standard_mesh() -> Self {
        let mut layout = Self::new();
        layout.add_attribute(0, "position", VertexAttribFormat::Float3);
        layout.add_attribute(1, "normal", VertexAttribFormat::Float3);
        layout.add_attribute(2, "uv", VertexAttribFormat::Float2);
        layout.add_attribute(3, "tangent", VertexAttribFormat::Float4);
        layout
    }
    pub fn position_only() -> Self {
        let mut layout = Self::new();
        layout.add_attribute(0, "position", VertexAttribFormat::Float3);
        layout
    }
    pub fn particle() -> Self {
        let mut layout = Self::new();
        layout.add_attribute(0, "position", VertexAttribFormat::Float3);
        layout.add_attribute(1, "color", VertexAttribFormat::Unorm4);
        layout.add_attribute(2, "size_rotation", VertexAttribFormat::Float2);
        layout
    }
    pub fn attribute_count(&self) -> usize { self.attributes.len() }
    pub fn generate_glsl_inputs(&self) -> String {
        self.attributes.iter().map(|a| format!("layout(location={}) in {} {};\n", a.location, a.format.glsl_type_name(), a.name)).collect()
    }
}

impl Default for VertexLayout {
    fn default() -> Self { Self::standard_mesh() }
}

// ── Buffer System ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct GpuBuffer {
    pub id: u32,
    pub name: String,
    pub size_bytes: u64,
    pub usage: BufferUsage,
    pub access: BufferAccess,
    pub binding: Option<u32>,
    pub stride: u32,
    pub element_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BufferUsage { Vertex, Index, Uniform, Storage, Indirect, Staging }
#[derive(Clone, Debug, PartialEq)]
pub enum BufferAccess { GpuOnly, CpuToGpu, GpuToCpu, CpuOnly }

impl GpuBuffer {
    pub fn new(id: u32, name: impl Into<String>, size: u64, usage: BufferUsage) -> Self {
        Self { id, name: name.into(), size_bytes: size, usage, access: BufferAccess::GpuOnly, binding: None, stride: 0, element_count: 0 }
    }
    pub fn vertex(id: u32, name: impl Into<String>, vertex_count: u32, stride: u32) -> Self {
        let mut b = Self::new(id, name, vertex_count as u64 * stride as u64, BufferUsage::Vertex);
        b.stride = stride; b.element_count = vertex_count; b
    }
    pub fn index(id: u32, name: impl Into<String>, index_count: u32, is_u32: bool) -> Self {
        let stride = if is_u32 { 4 } else { 2 };
        let mut b = Self::new(id, name, index_count as u64 * stride, BufferUsage::Index);
        b.stride = stride as u32; b.element_count = index_count; b
    }
    pub fn uniform(id: u32, name: impl Into<String>, size: u64, binding: u32) -> Self {
        let mut b = Self::new(id, name, size, BufferUsage::Uniform);
        b.binding = Some(binding); b
    }
    pub fn is_vertex_buffer(&self) -> bool { self.usage == BufferUsage::Vertex }
    pub fn is_index_buffer(&self) -> bool { self.usage == BufferUsage::Index }
    pub fn is_uniform_buffer(&self) -> bool { self.usage == BufferUsage::Uniform }
    pub fn size_kb(&self) -> f64 { self.size_bytes as f64 / 1024.0 }
    pub fn size_mb(&self) -> f64 { self.size_bytes as f64 / (1024.0 * 1024.0) }
}

#[derive(Clone, Debug)]
pub struct BufferRegistry {
    pub buffers: HashMap<u32, GpuBuffer>,
    pub next_id: u32,
    pub total_bytes: u64,
}

impl BufferRegistry {
    pub fn new() -> Self { Self { buffers: HashMap::new(), next_id: 1, total_bytes: 0 } }
    pub fn register(&mut self, mut buf: GpuBuffer) -> u32 {
        let id = self.next_id; self.next_id += 1;
        buf.id = id;
        self.total_bytes += buf.size_bytes;
        self.buffers.insert(id, buf);
        id
    }
    pub fn get(&self, id: u32) -> Option<&GpuBuffer> { self.buffers.get(&id) }
    pub fn remove(&mut self, id: u32) -> bool {
        if let Some(b) = self.buffers.remove(&id) { self.total_bytes = self.total_bytes.saturating_sub(b.size_bytes); true } else { false }
    }
    pub fn by_usage(&self, usage: &BufferUsage) -> Vec<&GpuBuffer> { self.buffers.values().filter(|b| &b.usage == usage).collect() }
    pub fn total_mb(&self) -> f64 { self.total_bytes as f64 / (1024.0 * 1024.0) }
    pub fn count(&self) -> usize { self.buffers.len() }
}

impl Default for BufferRegistry {
    fn default() -> Self { Self::new() }
}

// ── Compute Shader ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ComputePass {
    pub id: u32,
    pub name: String,
    pub shader_program_id: u32,
    pub dispatch_x: u32,
    pub dispatch_y: u32,
    pub dispatch_z: u32,
    pub bindings: Vec<ComputeBinding>,
    pub push_constants: Vec<u8>,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct ComputeBinding {
    pub set: u32,
    pub binding: u32,
    pub resource_type: ComputeResourceType,
    pub resource_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComputeResourceType { Texture, StorageTexture, Buffer, StorageBuffer, UniformBuffer }

impl ComputePass {
    pub fn new(id: u32, name: impl Into<String>, shader_id: u32) -> Self {
        Self { id, name: name.into(), shader_program_id: shader_id, dispatch_x: 1, dispatch_y: 1, dispatch_z: 1, bindings: Vec::new(), push_constants: Vec::new(), enabled: true }
    }
    pub fn dispatch(mut self, x: u32, y: u32, z: u32) -> Self { self.dispatch_x = x; self.dispatch_y = y; self.dispatch_z = z; self }
    pub fn add_binding(&mut self, set: u32, binding: u32, res_type: ComputeResourceType, res_id: u32) {
        self.bindings.push(ComputeBinding { set, binding, resource_type: res_type, resource_id: res_id });
    }
    pub fn total_invocations(&self) -> u64 { self.dispatch_x as u64 * self.dispatch_y as u64 * self.dispatch_z as u64 }
    pub fn binding_count(&self) -> usize { self.bindings.len() }
}

// ── GPU Memory Budget ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct GpuMemoryBudget {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub textures_bytes: u64,
    pub buffers_bytes: u64,
    pub render_targets_bytes: u64,
    pub shader_bytes: u64,
    pub misc_bytes: u64,
}

impl GpuMemoryBudget {
    pub fn new(total: u64) -> Self {
        Self { total_bytes: total, available_bytes: total, textures_bytes: 0, buffers_bytes: 0, render_targets_bytes: 0, shader_bytes: 0, misc_bytes: 0 }
    }
    pub fn used_bytes(&self) -> u64 { self.textures_bytes + self.buffers_bytes + self.render_targets_bytes + self.shader_bytes + self.misc_bytes }
    pub fn usage_percent(&self) -> f32 { if self.total_bytes == 0 { 0.0 } else { self.used_bytes() as f32 / self.total_bytes as f32 * 100.0 } }
    pub fn is_overbudget(&self) -> bool { self.used_bytes() > self.total_bytes }
    pub fn headroom_mb(&self) -> f64 { (self.total_bytes.saturating_sub(self.used_bytes())) as f64 / (1024.0 * 1024.0) }
    pub fn allocate_texture(&mut self, bytes: u64) -> bool { if self.available_bytes >= bytes { self.textures_bytes += bytes; self.available_bytes -= bytes; true } else { false } }
    pub fn allocate_buffer(&mut self, bytes: u64) -> bool { if self.available_bytes >= bytes { self.buffers_bytes += bytes; self.available_bytes -= bytes; true } else { false } }
    pub fn free_texture(&mut self, bytes: u64) { self.textures_bytes = self.textures_bytes.saturating_sub(bytes); self.available_bytes += bytes.min(self.total_bytes); }
}

// ── Render Statistics ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct RenderStatistics {
    pub draw_calls: u32,
    pub triangles: u64,
    pub vertices: u64,
    pub texture_switches: u32,
    pub shader_switches: u32,
    pub render_pass_count: u32,
    pub compute_dispatches: u32,
    pub gpu_time_ms: f32,
    pub cpu_time_ms: f32,
    pub frame_number: u64,
}

impl RenderStatistics {
    pub fn new() -> Self { Self::default() }
    pub fn begin_frame(&mut self) {
        self.draw_calls = 0; self.triangles = 0; self.vertices = 0;
        self.texture_switches = 0; self.shader_switches = 0;
        self.render_pass_count = 0; self.compute_dispatches = 0;
        self.gpu_time_ms = 0.0; self.cpu_time_ms = 0.0;
        self.frame_number += 1;
    }
    pub fn record_draw(&mut self, vertices: u64, triangles: u64) { self.draw_calls += 1; self.vertices += vertices; self.triangles += triangles; }
    pub fn fps(&self) -> f32 { if self.gpu_time_ms < 1e-6 { 0.0 } else { 1000.0 / self.gpu_time_ms } }
    pub fn avg_tris_per_draw(&self) -> f64 { if self.draw_calls == 0 { 0.0 } else { self.triangles as f64 / self.draw_calls as f64 } }
    pub fn total_ms(&self) -> f32 { self.gpu_time_ms + self.cpu_time_ms }
}

// ── More shader constants ─────────────────────────────────────────────────────

pub const TEXTURE_MAX_SIZE: u32 = 16384;
pub const TEXTURE_MAX_MIP_LEVELS: u32 = 15;
pub const TEXTURE_MAX_ARRAY_LAYERS: u32 = 2048;
pub const SAMPLER_MAX_ANISOTROPY: u32 = 16;
pub const BUFFER_ALIGNMENT: u32 = 256;
pub const VERTEX_LAYOUT_MAX_ATTRIBUTES: usize = 16;
pub const COMPUTE_MAX_WORKGROUP_SIZE: u32 = 1024;
pub const RENDER_PASS_MAX_COLOR_ATTACHMENTS: usize = 8;
pub const GPU_MEMORY_BUDGET_DEFAULT_MB: u64 = 4096;
pub const MATERIAL_DEFAULT_RENDER_QUEUE: u32 = 2000;

pub fn texture_format_name(fmt: &AttachmentFormat) -> &'static str { attachment_format_name(fmt) }
pub fn vertex_format_size(fmt: VertexAttribFormat) -> u32 { fmt.size_bytes() }
pub fn buffer_aligned_size(size: u64) -> u64 { (size + BUFFER_ALIGNMENT as u64 - 1) & !(BUFFER_ALIGNMENT as u64 - 1) }
pub fn is_power_of_two(n: u32) -> bool { n > 0 && (n & (n - 1)) == 0 }
pub fn next_power_of_two(n: u32) -> u32 { if n == 0 { 1 } else { let mut p = n; p -= 1; p |= p >> 1; p |= p >> 2; p |= p >> 4; p |= p >> 8; p |= p >> 16; p + 1 } }
pub fn mip_count(width: u32, height: u32) -> u32 { TextureDescriptor::mip_count_for_size(width, height) }
pub fn shader_compiler_full_info() -> String {
    format!("ShaderCompiler — {} target platforms, materials, textures, buffers, render pipeline", shader_target_count())
}


// ── Lighting Model ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LightingModel {
    pub name: String,
    pub model_type: LightingModelType,
    pub diffuse_weight: f32,
    pub specular_weight: f32,
    pub ambient_weight: f32,
    pub subsurface_weight: f32,
    pub emission_weight: f32,
    pub roughness: f32,
    pub metalness: f32,
    pub ior: f32,
    pub transmission: f32,
    pub anisotropy: f32,
    pub clearcoat: f32,
    pub sheen: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LightingModelType { Phong, BlinnPhong, Pbr, Lambert, Toon, Unlit, Custom(String) }

impl LightingModel {
    pub fn phong() -> Self { Self { name: "Phong".into(), model_type: LightingModelType::Phong, diffuse_weight: 1.0, specular_weight: 0.5, ambient_weight: 0.1, subsurface_weight: 0.0, emission_weight: 0.0, roughness: 0.5, metalness: 0.0, ior: 1.5, transmission: 0.0, anisotropy: 0.0, clearcoat: 0.0, sheen: 0.0 } }
    pub fn pbr() -> Self { Self { name: "PBR".into(), model_type: LightingModelType::Pbr, diffuse_weight: 1.0, specular_weight: 1.0, ambient_weight: 0.1, subsurface_weight: 0.0, emission_weight: 0.0, roughness: 0.5, metalness: 0.0, ior: 1.5, transmission: 0.0, anisotropy: 0.0, clearcoat: 0.0, sheen: 0.0 } }
    pub fn unlit() -> Self { Self { name: "Unlit".into(), model_type: LightingModelType::Unlit, diffuse_weight: 0.0, specular_weight: 0.0, ambient_weight: 0.0, subsurface_weight: 0.0, emission_weight: 1.0, roughness: 1.0, metalness: 0.0, ior: 1.0, transmission: 0.0, anisotropy: 0.0, clearcoat: 0.0, sheen: 0.0 } }
    pub fn toon() -> Self { Self { name: "Toon".into(), model_type: LightingModelType::Toon, diffuse_weight: 1.0, specular_weight: 1.0, ambient_weight: 0.2, subsurface_weight: 0.0, emission_weight: 0.0, roughness: 1.0, metalness: 0.0, ior: 1.5, transmission: 0.0, anisotropy: 0.0, clearcoat: 0.0, sheen: 0.0 } }
    pub fn is_pbr(&self) -> bool { self.model_type == LightingModelType::Pbr }
    pub fn is_unlit(&self) -> bool { self.model_type == LightingModelType::Unlit }
    pub fn is_toon(&self) -> bool { self.model_type == LightingModelType::Toon }
    pub fn uses_ibl(&self) -> bool { self.is_pbr() }
    pub fn glsl_function_name(&self) -> &str {
        match self.model_type { LightingModelType::Phong => "shade_phong", LightingModelType::BlinnPhong => "shade_blinn_phong", LightingModelType::Pbr => "shade_pbr", LightingModelType::Lambert => "shade_lambert", LightingModelType::Toon => "shade_toon", LightingModelType::Unlit => "shade_unlit", _ => "shade_custom" }
    }
}

impl Default for LightingModel {
    fn default() -> Self { Self::pbr() }
}

// ── GLSL Code Snippets ────────────────────────────────────────────────────────

pub struct GlslSnippet {
    pub name: &'static str,
    pub code: &'static str,
    pub dependencies: &'static [&'static str],
}

pub const SNIPPET_GAMMA_CORRECT: GlslSnippet = GlslSnippet {
    name: "gamma_correct",
    code: "vec3 gamma_correct(vec3 color) { return pow(clamp(color, 0.0, 1.0), vec3(1.0 / 2.2)); }",
    dependencies: &[],
};

pub const SNIPPET_LINEAR_TO_SRGB: GlslSnippet = GlslSnippet {
    name: "linear_to_srgb",
    code: "vec3 linear_to_srgb(vec3 c) { return mix(c * 12.92, 1.055 * pow(c, vec3(1.0/2.4)) - 0.055, step(vec3(0.0031308), c)); }",
    dependencies: &[],
};

pub const SNIPPET_SRGB_TO_LINEAR: GlslSnippet = GlslSnippet {
    name: "srgb_to_linear",
    code: "vec3 srgb_to_linear(vec3 c) { return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c)); }",
    dependencies: &[],
};

pub const SNIPPET_ACES_TONEMAP: GlslSnippet = GlslSnippet {
    name: "aces_tonemap",
    code: "vec3 aces_tonemap(vec3 x) { float a=2.51; float b=0.03; float c=2.43; float d=0.59; float e=0.14; return clamp((x*(a*x+b))/(x*(c*x+d)+e),0.0,1.0); }",
    dependencies: &[],
};

pub const SNIPPET_REINHARD_TONEMAP: GlslSnippet = GlslSnippet {
    name: "reinhard",
    code: "vec3 reinhard(vec3 c) { return c / (c + vec3(1.0)); }",
    dependencies: &[],
};

pub const SNIPPET_FRESNEL: GlslSnippet = GlslSnippet {
    name: "fresnel_schlick",
    code: "vec3 fresnel_schlick(float cosTheta, vec3 F0) { return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0); }",
    dependencies: &[],
};

pub const SNIPPET_NDF_GGX: GlslSnippet = GlslSnippet {
    name: "ndf_ggx",
    code: "float ndf_ggx(vec3 N, vec3 H, float roughness) { float a = roughness*roughness; float a2 = a*a; float NdotH = max(dot(N,H),0.0); float NdotH2 = NdotH*NdotH; float num = a2; float denom = NdotH2*(a2-1.0)+1.0; denom = 3.14159265*denom*denom; return num/denom; }",
    dependencies: &[],
};

pub const SNIPPET_GEOMETRY_SMITH: GlslSnippet = GlslSnippet {
    name: "geometry_smith",
    code: "float geometry_schlick_ggx(float NdotV, float r) { float r2=(r+1.0); float k=r2*r2/8.0; return NdotV/(NdotV*(1.0-k)+k); } float geometry_smith(vec3 N, vec3 V, vec3 L, float r) { return geometry_schlick_ggx(max(dot(N,V),0.0),r)*geometry_schlick_ggx(max(dot(N,L),0.0),r); }",
    dependencies: &[],
};

pub const SNIPPET_SHADOW_PCF: GlslSnippet = GlslSnippet {
    name: "shadow_pcf",
    code: "float shadow_pcf(sampler2DShadow shadowMap, vec4 shadowCoord) { float shadow = 0.0; vec2 texelSize = 1.0/textureSize(shadowMap,0); for(int x=-1;x<=1;++x) for(int y=-1;y<=1;++y) shadow += texture(shadowMap, shadowCoord.xyz/shadowCoord.w + vec3(vec2(x,y)*texelSize,0)); return shadow/9.0; }",
    dependencies: &[],
};

pub const SNIPPET_NORMAL_FROM_MAP: GlslSnippet = GlslSnippet {
    name: "normal_from_map",
    code: "vec3 normal_from_map(sampler2D normalMap, vec2 uv, vec3 worldNormal, vec3 worldTangent) { vec3 n = texture(normalMap, uv).xyz * 2.0 - 1.0; vec3 T = normalize(worldTangent - dot(worldTangent,worldNormal)*worldNormal); vec3 B = cross(worldNormal, T); mat3 TBN = mat3(T,B,worldNormal); return normalize(TBN * n); }",
    dependencies: &[],
};

pub const SNIPPET_PARALLAX_OCCLUSION: GlslSnippet = GlslSnippet {
    name: "parallax_occlusion",
    code: "vec2 parallax_mapping(sampler2D heightMap, vec2 uv, vec3 viewDir, float scale) { float numLayers = mix(32.0, 8.0, abs(dot(vec3(0,0,1),viewDir))); float layerDepth = 1.0/numLayers; float currentDepth = 0.0; vec2 P = viewDir.xy/viewDir.z*scale; vec2 deltaTexCoords = P/numLayers; vec2 currentTexCoords = uv; float currentDepthMapValue = 1.0 - texture(heightMap,currentTexCoords).r; while(currentDepth < currentDepthMapValue) { currentTexCoords -= deltaTexCoords; currentDepthMapValue = 1.0 - texture(heightMap,currentTexCoords).r; currentDepth += layerDepth; } return currentTexCoords; }",
    dependencies: &[],
};

// ── Shader Library ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderLibrary {
    pub snippets: HashMap<String, String>,
    pub categories: HashMap<String, Vec<String>>,
}

impl ShaderLibrary {
    pub fn new() -> Self { Self { snippets: HashMap::new(), categories: HashMap::new() } }
    pub fn add(&mut self, name: impl Into<String>, code: impl Into<String>, category: impl Into<String>) {
        let n = name.into(); let cat = category.into();
        self.snippets.insert(n.clone(), code.into());
        self.categories.entry(cat).or_default().push(n);
    }
    pub fn get(&self, name: &str) -> Option<&str> { self.snippets.get(name).map(|s| s.as_str()) }
    pub fn in_category(&self, cat: &str) -> Vec<&str> {
        self.categories.get(cat).map(|names| names.iter().map(|n| self.snippets.get(n.as_str()).map(|_| n.as_str()).unwrap_or("")).collect()).unwrap_or_default()
    }
    pub fn count(&self) -> usize { self.snippets.len() }
    pub fn build_standard() -> Self {
        let mut lib = Self::new();
        lib.add("gamma_correct", SNIPPET_GAMMA_CORRECT.code, "color");
        lib.add("linear_to_srgb", SNIPPET_LINEAR_TO_SRGB.code, "color");
        lib.add("srgb_to_linear", SNIPPET_SRGB_TO_LINEAR.code, "color");
        lib.add("aces_tonemap", SNIPPET_ACES_TONEMAP.code, "tonemap");
        lib.add("reinhard", SNIPPET_REINHARD_TONEMAP.code, "tonemap");
        lib.add("fresnel_schlick", SNIPPET_FRESNEL.code, "pbr");
        lib.add("ndf_ggx", SNIPPET_NDF_GGX.code, "pbr");
        lib.add("geometry_smith", SNIPPET_GEOMETRY_SMITH.code, "pbr");
        lib.add("shadow_pcf", SNIPPET_SHADOW_PCF.code, "shadow");
        lib.add("normal_from_map", SNIPPET_NORMAL_FROM_MAP.code, "normal");
        lib.add("parallax_occlusion", SNIPPET_PARALLAX_OCCLUSION.code, "normal");
        lib
    }
}

impl Default for ShaderLibrary {
    fn default() -> Self { Self::build_standard() }
}

// ── Shader Debug Tools ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderDebugOutput {
    pub name: String,
    pub output_type: DebugOutputType,
    pub channel_mask: [bool; 4],
    pub scale: f32,
    pub bias: f32,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DebugOutputType { Albedo, Normal, Roughness, Metalness, Depth, AmbientOcclusion, Emission, Velocity, ShadowMap, LightContribution, Custom(String) }

impl ShaderDebugOutput {
    pub fn new(name: impl Into<String>, output_type: DebugOutputType) -> Self {
        Self { name: name.into(), output_type, channel_mask: [true; 4], scale: 1.0, bias: 0.0, enabled: true }
    }
    pub fn depth() -> Self { let mut d = Self::new("Depth", DebugOutputType::Depth); d.scale = 100.0; d }
    pub fn normal() -> Self { let mut d = Self::new("Normal", DebugOutputType::Normal); d.bias = 0.5; d }
    pub fn glsl_code(&self) -> String {
        format!("// Debug output: {}", self.name)
    }
}

#[derive(Clone, Debug)]
pub struct ShaderDebugger {
    pub outputs: Vec<ShaderDebugOutput>,
    pub active_output: Option<usize>,
    pub enabled: bool,
    pub show_overdraw: bool,
    pub show_wireframe: bool,
    pub show_normals: bool,
}

impl ShaderDebugger {
    pub fn new() -> Self { Self { outputs: Vec::new(), active_output: None, enabled: false, show_overdraw: false, show_wireframe: false, show_normals: false } }
    pub fn add_output(&mut self, output: ShaderDebugOutput) { self.outputs.push(output); }
    pub fn activate(&mut self, idx: usize) { self.active_output = Some(idx); self.enabled = true; }
    pub fn deactivate(&mut self) { self.active_output = None; self.enabled = false; }
    pub fn active(&self) -> Option<&ShaderDebugOutput> { self.active_output.and_then(|i| self.outputs.get(i)) }
    pub fn output_count(&self) -> usize { self.outputs.len() }
    pub fn build_defaults() -> Self {
        let mut d = Self::new();
        d.add_output(ShaderDebugOutput::new("Albedo", DebugOutputType::Albedo));
        d.add_output(ShaderDebugOutput::normal());
        d.add_output(ShaderDebugOutput::new("Roughness", DebugOutputType::Roughness));
        d.add_output(ShaderDebugOutput::new("Metalness", DebugOutputType::Metalness));
        d.add_output(ShaderDebugOutput::depth());
        d.add_output(ShaderDebugOutput::new("AO", DebugOutputType::AmbientOcclusion));
        d
    }
}

impl Default for ShaderDebugger {
    fn default() -> Self { Self::build_defaults() }
}

// ── Shader Profiler ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct ShaderProfileEntry {
    pub shader_id: u32,
    pub shader_name: String,
    pub gpu_time_ms: f32,
    pub draw_calls: u32,
    pub triangles: u64,
    pub frame_count: u64,
    pub avg_gpu_ms: f32,
}

impl ShaderProfileEntry {
    pub fn new(shader_id: u32, name: impl Into<String>) -> Self { Self { shader_id, shader_name: name.into(), ..Default::default() } }
    pub fn record_frame(&mut self, gpu_ms: f32, draws: u32, tris: u64) {
        self.gpu_time_ms = gpu_ms;
        self.draw_calls = draws;
        self.triangles = tris;
        self.avg_gpu_ms = (self.avg_gpu_ms * self.frame_count as f32 + gpu_ms) / (self.frame_count + 1) as f32;
        self.frame_count += 1;
    }
    pub fn is_expensive(&self) -> bool { self.avg_gpu_ms > 2.0 }
}

#[derive(Clone, Debug)]
pub struct ShaderProfiler {
    pub entries: HashMap<u32, ShaderProfileEntry>,
    pub enabled: bool,
    pub frame: u64,
}

impl ShaderProfiler {
    pub fn new() -> Self { Self { entries: HashMap::new(), enabled: false, frame: 0 } }
    pub fn enable(&mut self) { self.enabled = true; }
    pub fn disable(&mut self) { self.enabled = false; }
    pub fn begin_frame(&mut self) { self.frame += 1; }
    pub fn record(&mut self, shader_id: u32, name: &str, gpu_ms: f32, draws: u32, tris: u64) {
        if !self.enabled { return; }
        self.entries.entry(shader_id).or_insert_with(|| ShaderProfileEntry::new(shader_id, name)).record_frame(gpu_ms, draws, tris);
    }
    pub fn expensive_shaders(&self) -> Vec<&ShaderProfileEntry> { self.entries.values().filter(|e| e.is_expensive()).collect() }
    pub fn total_gpu_ms(&self) -> f32 { self.entries.values().map(|e| e.gpu_time_ms).sum() }
    pub fn shader_count(&self) -> usize { self.entries.len() }
}

impl Default for ShaderProfiler {
    fn default() -> Self { Self::new() }
}

// ── Shader Hot Reload Manager ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct HotReloadManager {
    pub watcher: ShaderFileWatcher,
    pub registry: ShaderProgramRegistry,
    pub reload_count: u32,
    pub last_reload_time: f32,
    pub reload_on_focus: bool,
    pub auto_save_on_compile: bool,
}

impl HotReloadManager {
    pub fn new() -> Self { Self { watcher: ShaderFileWatcher::new(), registry: ShaderProgramRegistry::new(), reload_count: 0, last_reload_time: 0.0, reload_on_focus: true, auto_save_on_compile: false } }
    pub fn watch_shader(&mut self, path: impl Into<String>, shader_id: u32) { self.watcher.watch(path, shader_id); }
    pub fn tick(&mut self, _dt: f32) -> Vec<u32> {
        let mut reloaded = Vec::new();
        while let Some(path) = self.watcher.next_reload() {
            let ids = self.watcher.shader_ids_for(&path);
            reloaded.extend_from_slice(&ids);
            self.reload_count += ids.len() as u32;
        }
        reloaded
    }
    pub fn force_reload_all(&mut self) {
        let paths: Vec<_> = self.watcher.watched_files.keys().cloned().collect();
        for path in paths { self.watcher.mark_modified(&path); }
    }
    pub fn reload_count(&self) -> u32 { self.reload_count }
}

impl Default for HotReloadManager {
    fn default() -> Self { Self::new() }
}

// ── More constants ────────────────────────────────────────────────────────────

pub const LIGHTING_MODEL_COUNT: usize = 7;
pub const SHADER_LIBRARY_STANDARD_SNIPPET_COUNT: usize = 11;
pub const SHADER_DEBUG_OUTPUT_MAX: usize = 16;
pub const SHADER_PROFILER_EXPENSIVE_THRESHOLD_MS: f32 = 2.0;
pub const SHADER_HOT_RELOAD_DEBOUNCE_MS: u32 = 300;
pub const MATERIAL_MAX_BLEND_MODES: usize = 8;
pub const SHADER_SNIPPET_MAX_SIZE_CHARS: usize = 4096;
pub const GLSL_MAX_VARYING_COMPONENTS: u32 = 128;
pub const GLSL_MAX_TEXTURE_SAMPLERS: u32 = 16;
pub const GLSL_MAX_UNIFORM_BLOCKS: u32 = 14;
pub const WGSL_MAX_BIND_GROUPS: u32 = 4;
pub const WGSL_MAX_BINDINGS_PER_GROUP: u32 = 8;

pub fn lighting_model_name(model: &LightingModel) -> &str { &model.name }
pub fn shader_system_info() -> String { format!("ShaderSystem: compiler + materials + textures + buffers + post-process + lighting + hot-reload") }


// ── Shader Permutation System ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderPermutationKey {
    pub features: Vec<String>,
}

impl ShaderPermutationKey {
    pub fn new() -> Self { Self { features: Vec::new() } }
    pub fn with(mut self, feature: impl Into<String>) -> Self { self.features.push(feature.into()); self }
    pub fn add(&mut self, feature: impl Into<String>) { self.features.push(feature.into()); }
    pub fn hash_key(&self) -> u64 {
        let mut sorted = self.features.clone(); sorted.sort();
        sorted.iter().fold(14695981039346656037u64, |h, s| s.bytes().fold(h, |h, b| h.wrapping_mul(1099511628211) ^ b as u64))
    }
    pub fn has(&self, feature: &str) -> bool { self.features.contains(&feature.to_string()) }
    pub fn feature_count(&self) -> usize { self.features.len() }
    pub fn is_empty(&self) -> bool { self.features.is_empty() }
}

impl Default for ShaderPermutationKey {
    fn default() -> Self { Self::new() }
}

#[derive(Clone, Debug)]
pub struct ShaderPermutationRegistry {
    pub permutations: HashMap<u64, ShaderProgram>,
    pub keys: HashMap<u64, ShaderPermutationKey>,
    pub max_permutations: usize,
    pub base_program_id: u32,
}

impl ShaderPermutationRegistry {
    pub fn new(base_id: u32, max: usize) -> Self { Self { permutations: HashMap::new(), keys: HashMap::new(), max_permutations: max, base_program_id: base_id } }
    pub fn register(&mut self, key: ShaderPermutationKey, program: ShaderProgram) -> bool {
        if self.permutations.len() >= self.max_permutations { return false; }
        let h = key.hash_key();
        self.permutations.insert(h, program);
        self.keys.insert(h, key);
        true
    }
    pub fn find(&self, key: &ShaderPermutationKey) -> Option<&ShaderProgram> { self.permutations.get(&key.hash_key()) }
    pub fn count(&self) -> usize { self.permutations.len() }
    pub fn clear(&mut self) { self.permutations.clear(); self.keys.clear(); }
}

// ── Shader Reflection Extended ────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderBindingLayout {
    pub set: u32,
    pub bindings: Vec<DescriptorBinding>,
}

#[derive(Clone, Debug)]
pub struct DescriptorBinding {
    pub binding: u32,
    pub descriptor_type: DescriptorType,
    pub count: u32,
    pub stages: Vec<ShaderStage>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DescriptorType { UniformBuffer, StorageBuffer, Sampler, SampledImage, StorageImage, CombinedImageSampler, InputAttachment }

#[derive(Clone, Debug, PartialEq)]
pub enum ShaderStage { Vertex, Fragment, Geometry, TessControl, TessEval, Compute, All }

impl ShaderBindingLayout {
    pub fn new(set: u32) -> Self { Self { set, bindings: Vec::new() } }
    pub fn add_binding(&mut self, binding: u32, desc_type: DescriptorType, name: impl Into<String>) {
        self.bindings.push(DescriptorBinding { binding, descriptor_type: desc_type, count: 1, stages: vec![ShaderStage::All], name: name.into() });
    }
    pub fn binding_count(&self) -> usize { self.bindings.len() }
    pub fn has_binding(&self, binding: u32) -> bool { self.bindings.iter().any(|b| b.binding == binding) }
}

#[derive(Clone, Debug)]
pub struct PipelineLayout {
    pub descriptor_sets: Vec<ShaderBindingLayout>,
    pub push_constant_size: u32,
    pub push_constant_stages: Vec<ShaderStage>,
}

impl PipelineLayout {
    pub fn new() -> Self { Self { descriptor_sets: Vec::new(), push_constant_size: 0, push_constant_stages: Vec::new() } }
    pub fn add_set(&mut self, layout: ShaderBindingLayout) { self.descriptor_sets.push(layout); }
    pub fn set_push_constants(&mut self, size: u32) { self.push_constant_size = size; self.push_constant_stages = vec![ShaderStage::Vertex, ShaderStage::Fragment]; }
    pub fn set_count(&self) -> usize { self.descriptor_sets.len() }
    pub fn has_push_constants(&self) -> bool { self.push_constant_size > 0 }
    pub fn total_bindings(&self) -> usize { self.descriptor_sets.iter().map(|s| s.binding_count()).sum() }
}

impl Default for PipelineLayout {
    fn default() -> Self { Self::new() }
}

// ── Shader Compile Pipeline ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct CompilePipelineConfig {
    pub optimizer: ShaderOptimizer,
    pub codegen: ShaderCodeGen,
    pub preprocessor: ShaderPreprocessor,
    pub include_resolver: ShaderIncludeResolver,
    pub cache: ShaderCache,
    pub diagnostic_limit: usize,
    pub strict_mode: bool,
    pub pedantic: bool,
}

impl CompilePipelineConfig {
    pub fn new(target: CodeGenTarget) -> Self {
        Self { optimizer: ShaderOptimizer::default(), codegen: ShaderCodeGen::new(target), preprocessor: ShaderPreprocessor::new(), include_resolver: ShaderIncludeResolver::new(), cache: ShaderCache::default(), diagnostic_limit: SHADER_DIAGNOSTIC_MAX, strict_mode: false, pedantic: false }
    }
    pub fn debug_config(target: CodeGenTarget) -> Self {
        let mut c = Self::new(target); c.optimizer = ShaderOptimizer::debug(); c
    }
    pub fn release_config(target: CodeGenTarget) -> Self {
        let mut c = Self::new(target); c.optimizer = ShaderOptimizer::release(); c
    }
    pub fn add_define(&mut self, key: impl Into<String>, val: impl Into<String>) { self.preprocessor.define(key, val); }
}

#[derive(Clone, Debug)]
pub struct CompileResult {
    pub success: bool,
    pub spirv_bytes: Option<Vec<u8>>,
    pub diagnostics: DiagnosticList,
    pub reflection: ShaderReflection,
    pub compile_time_ms: f32,
    pub source_lines: u32,
    pub optimized: bool,
}

impl CompileResult {
    pub fn ok(spirv: Vec<u8>, reflection: ShaderReflection) -> Self {
        Self { success: true, spirv_bytes: Some(spirv), diagnostics: DiagnosticList::new(), reflection, compile_time_ms: 0.0, source_lines: 0, optimized: false }
    }
    pub fn fail(diagnostics: DiagnosticList) -> Self {
        Self { success: false, spirv_bytes: None, diagnostics, reflection: ShaderReflection::new(), compile_time_ms: 0.0, source_lines: 0, optimized: false }
    }
    pub fn error_count(&self) -> usize { self.diagnostics.error_count() }
    pub fn warning_count(&self) -> usize { self.diagnostics.warning_count() }
    pub fn spirv_size(&self) -> usize { self.spirv_bytes.as_ref().map(|b| b.len()).unwrap_or(0) }
    pub fn has_warnings(&self) -> bool { self.diagnostics.warning_count() > 0 }
}

// ── Shader Editor State ───────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderEditorState {
    pub active_program_id: Option<u32>,
    pub active_stage: ShaderStage,
    pub cursor_line: u32,
    pub cursor_col: u32,
    pub scroll_top: u32,
    pub selection: Option<(u32, u32, u32, u32)>,
    pub font_size: f32,
    pub show_line_numbers: bool,
    pub show_minimap: bool,
    pub syntax_highlight: bool,
    pub auto_complete: bool,
    pub bracket_matching: bool,
    pub indent_guides: bool,
    pub word_wrap: bool,
    pub theme: String,
    pub undo_stack: VecDeque<String>,
    pub redo_stack: Vec<String>,
    pub modified: bool,
}

impl ShaderEditorState {
    pub fn new() -> Self {
        Self { active_program_id: None, active_stage: ShaderStage::Fragment, cursor_line: 0, cursor_col: 0, scroll_top: 0, selection: None, font_size: 14.0, show_line_numbers: true, show_minimap: true, syntax_highlight: true, auto_complete: true, bracket_matching: true, indent_guides: true, word_wrap: false, theme: "dark".into(), undo_stack: VecDeque::new(), redo_stack: Vec::new(), modified: false }
    }
    pub fn set_active(&mut self, id: u32, stage: ShaderStage) { self.active_program_id = Some(id); self.active_stage = stage; self.cursor_line = 0; self.cursor_col = 0; }
    pub fn move_cursor(&mut self, line: u32, col: u32) { self.cursor_line = line; self.cursor_col = col; }
    pub fn push_undo(&mut self, desc: impl Into<String>) { if self.undo_stack.len() >= 100 { self.undo_stack.pop_front(); } self.undo_stack.push_back(desc.into()); self.redo_stack.clear(); self.modified = true; }
    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }
    pub fn undo(&mut self) -> Option<String> { let v = self.undo_stack.pop_back()?; self.redo_stack.push(v.clone()); Some(v) }
    pub fn redo(&mut self) -> Option<String> { let v = self.redo_stack.pop()?; self.undo_stack.push_back(v.clone()); Some(v) }
    pub fn mark_saved(&mut self) { self.modified = false; }
    pub fn increase_font(&mut self) { self.font_size = (self.font_size + 1.0).min(48.0); }
    pub fn decrease_font(&mut self) { self.font_size = (self.font_size - 1.0).max(8.0); }
}

impl Default for ShaderEditorState {
    fn default() -> Self { Self::new() }
}

// ── Autocomplete Provider ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct AutoCompleteItem {
    pub label: String,
    pub kind: AutoCompleteKind,
    pub detail: String,
    pub insert_text: String,
    pub documentation: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AutoCompleteKind { Function, Variable, Type, Keyword, Snippet, Uniform }

impl AutoCompleteItem {
    pub fn function(name: impl Into<String>, signature: impl Into<String>) -> Self {
        let n = name.into();
        Self { label: n.clone(), kind: AutoCompleteKind::Function, detail: signature.into(), insert_text: n, documentation: String::new() }
    }
    pub fn type_item(name: &'static str) -> Self {
        Self { label: name.into(), kind: AutoCompleteKind::Type, detail: "GLSL type".into(), insert_text: name.into(), documentation: String::new() }
    }
    pub fn keyword(kw: &'static str) -> Self {
        Self { label: kw.into(), kind: AutoCompleteKind::Keyword, detail: "keyword".into(), insert_text: kw.into(), documentation: String::new() }
    }
}

#[derive(Clone, Debug)]
pub struct AutoCompleteProvider {
    pub builtins: Vec<AutoCompleteItem>,
    pub user_symbols: Vec<AutoCompleteItem>,
    pub max_results: usize,
}

impl AutoCompleteProvider {
    pub fn new() -> Self {
        let mut builtins = Vec::new();
        for kw in &["void", "float", "int", "uint", "bool", "vec2", "vec3", "vec4", "mat3", "mat4", "sampler2D"] {
            builtins.push(AutoCompleteItem::type_item(kw));
        }
        for kw in &["if", "else", "for", "while", "return", "break", "continue", "discard", "uniform", "in", "out", "const"] {
            builtins.push(AutoCompleteItem::keyword(kw));
        }
        for (name, sig) in &[("normalize", "vec3 normalize(vec3)"), ("length", "float length(genType)"), ("dot", "float dot(genType, genType)"), ("cross", "vec3 cross(vec3, vec3)"), ("reflect", "genType reflect(genType, genType)"), ("refract", "genType refract(genType, genType, float)"), ("mix", "genType mix(genType, genType, genType)"), ("clamp", "genType clamp(genType, genType, genType)"), ("pow", "genType pow(genType, genType)"), ("sqrt", "genType sqrt(genType)"), ("abs", "genType abs(genType)"), ("max", "genType max(genType, genType)"), ("min", "genType min(genType, genType)"), ("texture", "vec4 texture(sampler2D, vec2)"), ("step", "genType step(genType, genType)"), ("smoothstep", "genType smoothstep(genType, genType, genType)"), ("sin", "genType sin(genType)"), ("cos", "genType cos(genType)"), ("tan", "genType tan(genType)"), ("floor", "genType floor(genType)"), ("ceil", "genType ceil(genType)"), ("fract", "genType fract(genType)"), ("mod", "genType mod(genType, genType)"), ("sign", "genType sign(genType)"), ("distance", "float distance(genType, genType)"), ("transpose", "mat transpose(mat)"), ("inverse", "mat inverse(mat)"), ("determinant", "float determinant(mat)")] {
            builtins.push(AutoCompleteItem::function(*name, *sig));
        }
        Self { builtins, user_symbols: Vec::new(), max_results: 20 }
    }
    pub fn query(&self, prefix: &str) -> Vec<&AutoCompleteItem> {
        let lower = prefix.to_lowercase();
        let mut results: Vec<_> = self.builtins.iter().chain(self.user_symbols.iter()).filter(|item| item.label.to_lowercase().starts_with(&lower)).collect();
        results.truncate(self.max_results);
        results
    }
    pub fn add_user_symbol(&mut self, item: AutoCompleteItem) { self.user_symbols.push(item); }
    pub fn clear_user_symbols(&mut self) { self.user_symbols.clear(); }
    pub fn builtin_count(&self) -> usize { self.builtins.len() }
}

impl Default for AutoCompleteProvider {
    fn default() -> Self { Self::new() }
}

// ── Constants ─────────────────────────────────────────────────────────────────

pub const SHADER_EDITOR_MAX_UNDO: usize = 200;
pub const SHADER_AUTOCOMPLETE_MAX_RESULTS: usize = 20;
pub const SHADER_PERMUTATION_MAX: usize = 512;
pub const SHADER_BINDING_SET_MAX: usize = 4;
pub const SHADER_BINDING_MAX_PER_SET: usize = 16;
pub const SHADER_STAGE_COUNT: usize = 6;
pub const SHADER_PUSH_CONSTANT_MAX_BYTES: u32 = 128;

pub fn shader_stage_name(stage: &ShaderStage) -> &'static str {
    match stage { ShaderStage::Vertex => "vertex", ShaderStage::Fragment => "fragment", ShaderStage::Geometry => "geometry", ShaderStage::TessControl => "tess_control", ShaderStage::TessEval => "tess_eval", ShaderStage::Compute => "compute", ShaderStage::All => "all" }
}
pub fn descriptor_type_name(dt: &DescriptorType) -> &'static str {
    match dt { DescriptorType::UniformBuffer => "UniformBuffer", DescriptorType::StorageBuffer => "StorageBuffer", DescriptorType::Sampler => "Sampler", DescriptorType::SampledImage => "SampledImage", DescriptorType::StorageImage => "StorageImage", DescriptorType::CombinedImageSampler => "CombinedImageSampler", DescriptorType::InputAttachment => "InputAttachment" }
}


// ── Shader Input/Output Binding Validation ────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderInterface {
    pub inputs: Vec<ShaderVariable>,
    pub outputs: Vec<ShaderVariable>,
    pub stage: ShaderStage,
}

impl ShaderInterface {
    pub fn new(stage: ShaderStage) -> Self { Self { inputs: Vec::new(), outputs: Vec::new(), stage } }
    pub fn add_input(&mut self, var: ShaderVariable) { self.inputs.push(var); }
    pub fn add_output(&mut self, var: ShaderVariable) { self.outputs.push(var); }
    pub fn input_count(&self) -> usize { self.inputs.len() }
    pub fn output_count(&self) -> usize { self.outputs.len() }
    pub fn find_input(&self, name: &str) -> Option<&ShaderVariable> { self.inputs.iter().find(|v| v.name == name) }
    pub fn find_output(&self, name: &str) -> Option<&ShaderVariable> { self.outputs.iter().find(|v| v.name == name) }
    pub fn validate_compatible(&self, next: &ShaderInterface) -> Vec<String> {
        let mut errors = Vec::new();
        for out in &self.outputs {
            if !next.inputs.iter().any(|i| i.name == out.name && i.data_type == out.data_type) {
                errors.push(format!("Output '{}' not matched in next stage", out.name));
            }
        }
        errors
    }
    pub fn total_output_components(&self) -> u32 { self.outputs.iter().map(|o| o.data_type.byte_size() / 4).sum() }
}

// ── Shader Program Linker ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderLinker {
    pub programs: HashMap<u32, ShaderProgram>,
    pub layouts: HashMap<u32, PipelineLayout>,
    pub link_errors: Vec<String>,
}

impl ShaderLinker {
    pub fn new() -> Self { Self { programs: HashMap::new(), layouts: HashMap::new(), link_errors: Vec::new() } }
    pub fn add_program(&mut self, prog: ShaderProgram) { self.programs.insert(prog.id, prog); }
    pub fn link(&mut self, prog_id: u32) -> bool {
        self.link_errors.clear();
        if !self.programs.contains_key(&prog_id) { self.link_errors.push("Program not found".into()); return false; }
        true
    }
    pub fn has_errors(&self) -> bool { !self.link_errors.is_empty() }
    pub fn error_count(&self) -> usize { self.link_errors.len() }
    pub fn clear_errors(&mut self) { self.link_errors.clear(); }
}

impl Default for ShaderLinker {
    fn default() -> Self { Self::new() }
}

// ── Shader Export ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderExportOptions {
    pub target: CodeGenTarget,
    pub minify: bool,
    pub include_reflection: bool,
    pub include_source: bool,
    pub embed_spirv: bool,
    pub output_format: ShaderExportFormat,
    pub output_path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShaderExportFormat { Glsl, SpirV, Wgsl, Json, Binary }

impl ShaderExportOptions {
    pub fn new(target: CodeGenTarget) -> Self {
        Self { target, minify: false, include_reflection: true, include_source: true, embed_spirv: false, output_format: ShaderExportFormat::Glsl, output_path: "./shaders".into() }
    }
    pub fn release(target: CodeGenTarget) -> Self {
        Self { minify: true, include_source: false, ..Self::new(target) }
    }
    pub fn spirv(target: CodeGenTarget) -> Self {
        Self { embed_spirv: true, output_format: ShaderExportFormat::SpirV, ..Self::new(target) }
    }
}

#[derive(Clone, Debug)]
pub struct ShaderExportResult {
    pub success: bool,
    pub programs_exported: u32,
    pub files_written: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub total_bytes: u64,
    pub duration_ms: f32,
}

impl ShaderExportResult {
    pub fn ok(count: u32, files: Vec<String>, bytes: u64) -> Self {
        Self { success: true, programs_exported: count, files_written: files, errors: Vec::new(), warnings: Vec::new(), total_bytes: bytes, duration_ms: 0.0 }
    }
    pub fn fail(msg: impl Into<String>) -> Self {
        Self { success: false, programs_exported: 0, files_written: Vec::new(), errors: vec![msg.into()], warnings: Vec::new(), total_bytes: 0, duration_ms: 0.0 }
    }
}

// ── Shader Linting ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LintRule {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub severity: DiagnosticKind,
    pub enabled: bool,
    pub pattern: String,
}

impl LintRule {
    pub fn new(id: u32, name: impl Into<String>, severity: DiagnosticKind) -> Self {
        Self { id, name: name.into(), description: String::new(), severity, enabled: true, pattern: String::new() }
    }
    pub fn error(id: u32, name: impl Into<String>) -> Self { Self::new(id, name, DiagnosticKind::Error) }
    pub fn warn(id: u32, name: impl Into<String>) -> Self { Self::new(id, name, DiagnosticKind::Warning) }
    pub fn with_desc(mut self, desc: impl Into<String>) -> Self { self.description = desc.into(); self }
}

#[derive(Clone, Debug)]
pub struct ShaderLinter {
    pub rules: Vec<LintRule>,
    pub enabled: bool,
}

impl ShaderLinter {
    pub fn new() -> Self {
        let mut rules = Vec::new();
        rules.push(LintRule::warn(1, "unused_variable").with_desc("Variable declared but never used"));
        rules.push(LintRule::warn(2, "precision_missing").with_desc("Missing precision qualifier"));
        rules.push(LintRule::error(3, "undefined_variable").with_desc("Variable used before declaration"));
        rules.push(LintRule::warn(4, "divide_by_zero").with_desc("Potential division by constant zero"));
        rules.push(LintRule::warn(5, "comparison_float").with_desc("Float equality comparison may be imprecise"));
        rules.push(LintRule::warn(6, "redundant_cast").with_desc("Unnecessary type cast"));
        rules.push(LintRule::error(7, "wrong_argument_type").with_desc("Function argument type mismatch"));
        rules.push(LintRule::warn(8, "shadowed_variable").with_desc("Variable shadows outer scope declaration"));
        Self { rules, enabled: true }
    }
    pub fn lint(&self, _source: &str) -> DiagnosticList { DiagnosticList::new() }
    pub fn enabled_rules(&self) -> Vec<&LintRule> { self.rules.iter().filter(|r| r.enabled).collect() }
    pub fn disable_rule(&mut self, id: u32) { if let Some(r) = self.rules.iter_mut().find(|r| r.id == id) { r.enabled = false; } }
    pub fn enable_rule(&mut self, id: u32) { if let Some(r) = self.rules.iter_mut().find(|r| r.id == id) { r.enabled = true; } }
    pub fn rule_count(&self) -> usize { self.rules.len() }
}

impl Default for ShaderLinter {
    fn default() -> Self { Self::new() }
}

// ── Shader Formatting ─────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ShaderFormatterConfig {
    pub indent_size: usize,
    pub use_tabs: bool,
    pub max_line_length: usize,
    pub brace_style: BraceStyle,
    pub space_around_operators: bool,
    pub space_after_comma: bool,
    pub trailing_newline: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BraceStyle { Allman, KnR, Stroustrup }

impl ShaderFormatterConfig {
    pub fn default_style() -> Self {
        Self { indent_size: 4, use_tabs: false, max_line_length: 120, brace_style: BraceStyle::KnR, space_around_operators: true, space_after_comma: true, trailing_newline: true }
    }
    pub fn compact() -> Self {
        Self { indent_size: 2, ..Self::default_style() }
    }
}

impl Default for ShaderFormatterConfig {
    fn default() -> Self { Self::default_style() }
}

#[derive(Clone, Debug)]
pub struct ShaderFormatter {
    pub config: ShaderFormatterConfig,
}

impl ShaderFormatter {
    pub fn new() -> Self { Self { config: ShaderFormatterConfig::default() } }
    pub fn format(&self, source: &str) -> String {
        let mut output = String::with_capacity(source.len());
        let mut indent = 0usize;
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('}') && indent > 0 { indent -= 1; }
            let indent_str = if self.config.use_tabs { "\t".repeat(indent) } else { " ".repeat(indent * self.config.indent_size) };
            output.push_str(&indent_str);
            output.push_str(trimmed);
            output.push('\n');
            if trimmed.ends_with('{') { indent += 1; }
        }
        if self.config.trailing_newline && !output.ends_with('\n') { output.push('\n'); }
        output
    }
    pub fn count_lines(source: &str) -> usize { source.lines().count() }
    pub fn count_characters(source: &str) -> usize { source.chars().count() }
}

impl Default for ShaderFormatter {
    fn default() -> Self { Self::new() }
}

// ── Final shader constants ────────────────────────────────────────────────────

pub const SHADER_LINTER_RULE_COUNT_DEFAULT: usize = 8;
pub const SHADER_FORMATTER_MAX_LINE_LENGTH: usize = 120;
pub const SHADER_EXPORT_FORMAT_COUNT: usize = 5;
pub const SHADER_LINKER_MAX_PROGRAMS: usize = 1024;
pub const SHADER_INTERFACE_MAX_VARYINGS: usize = 32;
pub const SHADER_PERMUTATION_KEY_MAX_FEATURES: usize = 16;

pub fn shader_compiler_module_list() -> &'static [&'static str] {
    &[
        "tokenizer", "preprocessor", "parser", "ast", "codegen",
        "optimizer", "cache", "templates", "variants", "permutations",
        "include_resolver", "reflection", "diagnostics", "pipeline_state",
        "materials", "textures", "buffers", "render_passes", "post_process",
        "node_graph", "hot_reload", "lighting", "library", "debugger",
        "profiler", "editor_state", "autocomplete", "linker", "export",
        "linter", "formatter", "binding_layout",
    ]
}

pub fn shader_module_count() -> usize { shader_compiler_module_list().len() }

pub fn full_shader_system_description() -> String {
    format!("ShaderCompiler: {} modules — full pipeline from GLSL source to compiled GPU program", shader_module_count())
}

