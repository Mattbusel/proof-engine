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
