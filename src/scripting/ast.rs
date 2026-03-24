//! Abstract Syntax Tree for the scripting language.

use std::collections::HashMap;

// ── Expressions ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// nil
    Nil,
    /// true / false
    Bool(bool),
    /// integer literal
    Int(i64),
    /// float literal
    Float(f64),
    /// string literal
    Str(String),
    /// ... (varargs)
    Vararg,
    /// identifier reference
    Ident(String),
    /// table access: expr[expr]
    Index { table: Box<Expr>, key: Box<Expr> },
    /// field access: expr.name
    Field { table: Box<Expr>, name: String },
    /// function call: expr(args)
    Call { callee: Box<Expr>, args: Vec<Expr> },
    /// method call: expr:name(args)
    MethodCall { obj: Box<Expr>, method: String, args: Vec<Expr> },
    /// unary op: op expr
    Unary { op: UnOp, expr: Box<Expr> },
    /// binary op: lhs op rhs
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    /// table constructor: { [k]=v, ... }
    TableCtor(Vec<TableField>),
    /// anonymous function: function(params) body end
    FuncExpr { params: Vec<String>, vararg: bool, body: Vec<Stmt> },
    /// ternary: cond ? then_val : else_val (sugar)
    Ternary { cond: Box<Expr>, then_val: Box<Expr>, else_val: Box<Expr> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableField {
    /// [expr] = expr
    ExprKey(Expr, Expr),
    /// name = expr
    NameKey(String, Expr),
    /// just expr (array position)
    Value(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,   // -
    Not,   // not / !
    Len,   // #
    BitNot, // ~
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, Sub, Mul, Div, IDiv, Mod, Pow,
    Concat,         // ..
    Eq, NotEq, Lt, LtEq, Gt, GtEq,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

impl BinOp {
    pub fn precedence(self) -> u8 {
        match self {
            BinOp::Or                        => 1,
            BinOp::And                       => 2,
            BinOp::Eq | BinOp::NotEq
              | BinOp::Lt | BinOp::LtEq
              | BinOp::Gt | BinOp::GtEq      => 3,
            BinOp::Concat                    => 4,
            BinOp::BitOr                     => 5,
            BinOp::BitXor                    => 6,
            BinOp::BitAnd                    => 7,
            BinOp::Shl | BinOp::Shr         => 8,
            BinOp::Add | BinOp::Sub         => 9,
            BinOp::Mul | BinOp::Div
              | BinOp::IDiv | BinOp::Mod    => 10,
            BinOp::Pow                       => 12,
        }
    }

    pub fn is_right_assoc(self) -> bool {
        matches!(self, BinOp::Pow | BinOp::Concat)
    }
}

// ── Statements ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// local name [= expr]
    LocalDecl { name: String, init: Option<Expr> },
    /// local name1, name2 = expr1, expr2
    LocalMulti { names: Vec<String>, inits: Vec<Expr> },
    /// target = expr
    Assign { target: Vec<Expr>, value: Vec<Expr> },
    /// compound assign: target op= expr
    CompoundAssign { target: Expr, op: BinOp, value: Expr },
    /// function call statement
    Call(Expr),
    /// do ... end
    Do(Vec<Stmt>),
    /// while cond do ... end
    While { cond: Expr, body: Vec<Stmt> },
    /// repeat ... until cond
    RepeatUntil { body: Vec<Stmt>, cond: Expr },
    /// if cond then ... [elseif ...] [else ...] end
    If { cond: Expr, then_body: Vec<Stmt>, elseif_branches: Vec<(Expr, Vec<Stmt>)>, else_body: Option<Vec<Stmt>> },
    /// numeric for: for i = start, limit[, step] do ... end
    NumericFor { var: String, start: Expr, limit: Expr, step: Option<Expr>, body: Vec<Stmt> },
    /// generic for: for names in expr do ... end
    GenericFor { vars: Vec<String>, iter: Vec<Expr>, body: Vec<Stmt> },
    /// function name(params) body end
    FuncDecl { name: Vec<String>, params: Vec<String>, vararg: bool, body: Vec<Stmt> },
    /// local function name(params) body end
    LocalFunc { name: String, params: Vec<String>, vararg: bool, body: Vec<Stmt> },
    /// return [expr, ...]
    Return(Vec<Expr>),
    /// break
    Break,
    /// continue
    Continue,
    /// match expr { case pattern => body }
    Match { expr: Expr, arms: Vec<MatchArm> },
    /// import "module" [as name]
    Import { path: String, alias: Option<String> },
    /// export name
    Export(String),
    /// expr (standalone expression — error unless call)
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body:    Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    Literal(Expr),
    Ident(String),
    Wildcard,
    Table(Vec<(String, MatchPattern)>),
}

// ── Script ─────────────────────────────────────────────────────────────────

/// The top-level AST for a compiled script.
#[derive(Debug, Clone)]
pub struct Script {
    pub name:   String,
    pub stmts:  Vec<Stmt>,
}
