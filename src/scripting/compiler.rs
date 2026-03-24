//! Bytecode compiler — walks the AST and emits `Op` instructions into a `Proto`.
//!
//! # Architecture
//! A single-pass recursive descent over the AST maintains:
//! - A flat local-variable stack per function (slot numbers are u16).
//! - A scope depth counter for block exits.
//! - A list of upvalue descriptors per function (used by `Closure`).
//! - Per-loop break/continue patch lists.
//!
//! Jump offsets are relative (i32): positive = forward, negative = backward.

use std::collections::HashMap;
use std::sync::Arc;
use super::ast::*;
use super::vm::Value as VmValue;

// ── Constant pool ─────────────────────────────────────────────────────────────

/// A compile-time constant value.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

// ── Op (bytecode instruction set) ────────────────────────────────────────────

/// VM instruction.  Operands are embedded to allow the interpreter to avoid
/// secondary table lookups on the hot path.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    // ── Literals ──────────────────────────────────────────────────────────────
    /// Push nil.
    Nil,
    /// Push true.
    True,
    /// Push false.
    False,
    /// Push `proto.constants[idx]`.
    Const(u32),

    // ── Stack ─────────────────────────────────────────────────────────────────
    Pop,
    Dup,
    Swap,

    // ── Locals ────────────────────────────────────────────────────────────────
    GetLocal(u16),
    SetLocal(u16),

    // ── Upvalues ──────────────────────────────────────────────────────────────
    GetUpval(u16),
    SetUpval(u16),

    // ── Globals (constant-indexed by string name) ─────────────────────────────
    GetGlobal(u32),
    SetGlobal(u32),

    // ── Tables ────────────────────────────────────────────────────────────────
    NewTable,
    /// `SetField(kidx)`: pop val; table = peek; table[const_str] = val.
    SetField(u32),
    /// `GetField(kidx)`: pop table; push table[const_str].
    GetField(u32),
    /// `SetIndex`: pop val, key; table = peek; table[key] = val.
    SetIndex,
    /// `GetIndex`: pop key, table; push table[key].
    GetIndex,
    /// `TableAppend`: pop val; table = peek; append val to array part.
    TableAppend,
    /// `SetList(n)`: pop n values; table = peek; assign t[1..n].
    SetList(u16),

    // ── Unary ─────────────────────────────────────────────────────────────────
    Len,
    Neg,
    Not,
    BitNot,

    // ── Arithmetic ────────────────────────────────────────────────────────────
    Add, Sub, Mul, Div, IDiv, Mod, Pow,
    Concat,         // pops 2, pushes concatenated string

    // ── Comparison ────────────────────────────────────────────────────────────
    Eq, NotEq, Lt, LtEq, Gt, GtEq,

    // ── Bitwise ───────────────────────────────────────────────────────────────
    BitAnd, BitOr, BitXor, Shl, Shr,

    // ── Control flow ──────────────────────────────────────────────────────────
    /// Relative unconditional jump.  `ip += offset` (can be negative).
    Jump(i32),
    /// Peek top; if truthy jump (no pop).
    JumpIf(i32),
    /// Peek top; if falsy jump (no pop).
    JumpIfNot(i32),
    /// Pop top; if falsy jump — used for short-circuit `and`.
    JumpIfNotPop(i32),
    /// Pop top; if truthy jump — used for short-circuit `or`.
    JumpIfPop(i32),

    // ── Calls & returns ───────────────────────────────────────────────────────
    /// `Call(nargs, nret)`: pop nargs + callee; push nret results (0 = all).
    Call(u8, u8),
    /// `CallMethod(name_kidx, nargs, nret)`: obj on stack; method = const_str.
    CallMethod(u32, u8, u8),
    /// `Return(n)`: pop n values and return (0 = return all).
    Return(u8),
    /// Tail-call optimisation.
    TailCall(u8),

    // ── Closures ──────────────────────────────────────────────────────────────
    /// Create a closure from `proto.protos[idx]`, capturing upvalues.
    Closure(u32),
    /// Close the upvalue at local slot `slot`.
    Close(u16),

    // ── Iterators ─────────────────────────────────────────────────────────────
    /// Prepare generic-for: push iterator state.
    ForPrep(u16),
    /// Advance generic-for; pop results if exhausted (implied jump offset in
    /// combination with `ForStepJump`).
    ForStep,
    /// Like ForStep but with a jump offset for the exhausted case.
    ForStepJump(i32),
    /// Push and validate [start, limit, step] for numeric-for.
    NumForInit,
    /// Advance numeric-for; jump by offset if done.
    NumForStep(i32),

    // ── Varargs ───────────────────────────────────────────────────────────────
    /// Push `n` vararg values (0 = all).
    Vararg(u8),

    // ── Debug ─────────────────────────────────────────────────────────────────
    LineInfo(u32),
}

// ── Proto (function prototype) ────────────────────────────────────────────────

/// A compiled function — the unit of bytecode.
#[derive(Debug, Clone)]
pub struct Proto {
    pub name:          String,
    pub code:          Vec<Op>,
    pub constants:     Vec<Constant>,
    pub protos:        Vec<Proto>,      // nested closure prototypes
    pub param_count:   u8,
    pub is_vararg:     bool,
    pub upvalue_count: u16,
    pub max_stack:     u16,
}

impl Proto {
    fn new(name: impl Into<String>) -> Self {
        Proto {
            name:          name.into(),
            code:          Vec::new(),
            constants:     Vec::new(),
            protos:        Vec::new(),
            param_count:   0,
            is_vararg:     false,
            upvalue_count: 0,
            max_stack:     0,
        }
    }

    /// Add a constant, deduplicating where possible.
    pub fn add_const(&mut self, c: Constant) -> u32 {
        for (i, existing) in self.constants.iter().enumerate() {
            if *existing == c { return i as u32; }
        }
        let idx = self.constants.len() as u32;
        self.constants.push(c);
        idx
    }

    fn emit(&mut self, op: Op) -> usize {
        self.code.push(op);
        self.code.len() - 1
    }

    fn patch_jump(&mut self, instr_idx: usize) {
        let target = self.code.len() as i32;
        let from   = instr_idx as i32 + 1;
        let offset = target - from;
        match &mut self.code[instr_idx] {
            Op::Jump(o) | Op::JumpIf(o) | Op::JumpIfNot(o)
            | Op::JumpIfNotPop(o) | Op::JumpIfPop(o)
            | Op::NumForStep(o) | Op::ForStepJump(o) => *o = offset,
            _ => {}
        }
    }
}

// ── Instruction (VM-facing bytecode) ─────────────────────────────────────────

/// Runtime instruction set emitted by `Compiler::compile_script`.
#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    // Literals
    LoadNil,
    LoadBool(bool),
    LoadInt(i64),
    LoadFloat(f64),
    LoadStr(String),
    LoadConst(usize),
    // Stack
    Pop,
    Dup,
    Swap,
    // Locals / upvalues / globals
    GetLocal(usize),
    SetLocal(usize),
    GetUpvalue(usize),
    SetUpvalue(usize),
    GetGlobal(String),
    SetGlobal(String),
    // Tables
    NewTable,
    SetField(String),
    GetField(String),
    SetIndex,
    GetIndex,
    TableAppend,
    // Unary
    Len,
    Neg,
    Not,
    BitNot,
    // Arithmetic
    Add, Sub, Mul, Div, IDiv, Mod, Pow,
    Concat,
    // Bitwise
    BitAnd, BitOr, BitXor, Shl, Shr,
    // Comparison
    Eq, NotEq, Lt, LtEq, Gt, GtEq,
    // Control flow
    Jump(isize),
    JumpIf(isize),
    JumpIfNot(isize),
    /// Peek; if not truthy jump (and leave value); if truthy pop and continue.
    JumpIfNotPop(isize),
    /// Peek; if truthy jump (and leave value); if falsy pop and continue.
    JumpIfPop(isize),
    JumpAbs(usize),
    // Calls
    Call(usize),
    CallMethod(String, usize),
    Return(usize),
    // Closures
    MakeFunction(usize),
    MakeClosure(usize, Vec<(bool, usize)>),
    CloseUpvalue(usize),
    // Iterators
    ForPrep(usize),
    /// Advance numeric for-loop: `local_idx` = loop var slot, `jump_offset` = exit jump.
    ForStep(usize, isize),
    Nop,
}

// ── Chunk (VM-facing function prototype) ─────────────────────────────────────

/// A compiled function ready for the VM.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub name:         String,
    pub instructions: Vec<Instruction>,
    pub constants:    Vec<VmValue>,
    pub sub_chunks:   Vec<Arc<Chunk>>,
    pub param_count:  u8,
    pub is_vararg:    bool,
}

fn const_to_value(c: &Constant) -> VmValue {
    match c {
        Constant::Nil      => VmValue::Nil,
        Constant::Bool(b)  => VmValue::Bool(*b),
        Constant::Int(i)   => VmValue::Int(*i),
        Constant::Float(f) => VmValue::Float(*f),
        Constant::Str(s)   => VmValue::Str(Arc::new(s.clone())),
    }
}

fn proto_to_chunk(proto: &Proto) -> Arc<Chunk> {
    let instructions = proto.code.iter()
        .map(|op| op_to_instruction(op, &proto.constants))
        .collect();
    let constants = proto.constants.iter().map(const_to_value).collect();
    let sub_chunks = proto.protos.iter().map(proto_to_chunk).collect();
    Arc::new(Chunk {
        name:         proto.name.clone(),
        instructions,
        constants,
        sub_chunks,
        param_count:  proto.param_count,
        is_vararg:    proto.is_vararg,
    })
}

fn op_to_instruction(op: &Op, constants: &[Constant]) -> Instruction {
    let get_str = |kidx: u32| -> String {
        match constants.get(kidx as usize) {
            Some(Constant::Str(s)) => s.clone(),
            _ => String::new(),
        }
    };
    match op {
        Op::Nil               => Instruction::LoadNil,
        Op::True              => Instruction::LoadBool(true),
        Op::False             => Instruction::LoadBool(false),
        Op::Const(idx)        => Instruction::LoadConst(*idx as usize),
        Op::Pop               => Instruction::Pop,
        Op::Dup               => Instruction::Dup,
        Op::Swap              => Instruction::Swap,
        Op::GetLocal(s)       => Instruction::GetLocal(*s as usize),
        Op::SetLocal(s)       => Instruction::SetLocal(*s as usize),
        Op::GetUpval(i)       => Instruction::GetUpvalue(*i as usize),
        Op::SetUpval(i)       => Instruction::SetUpvalue(*i as usize),
        Op::GetGlobal(k)      => Instruction::GetGlobal(get_str(*k)),
        Op::SetGlobal(k)      => Instruction::SetGlobal(get_str(*k)),
        Op::NewTable          => Instruction::NewTable,
        Op::SetField(k)       => Instruction::SetField(get_str(*k)),
        Op::GetField(k)       => Instruction::GetField(get_str(*k)),
        Op::SetIndex          => Instruction::SetIndex,
        Op::GetIndex          => Instruction::GetIndex,
        Op::TableAppend       => Instruction::TableAppend,
        Op::SetList(_)        => Instruction::Nop,
        Op::Len               => Instruction::Len,
        Op::Neg               => Instruction::Neg,
        Op::Not               => Instruction::Not,
        Op::BitNot            => Instruction::BitNot,
        Op::Add               => Instruction::Add,
        Op::Sub               => Instruction::Sub,
        Op::Mul               => Instruction::Mul,
        Op::Div               => Instruction::Div,
        Op::IDiv              => Instruction::IDiv,
        Op::Mod               => Instruction::Mod,
        Op::Pow               => Instruction::Pow,
        Op::Concat            => Instruction::Concat,
        Op::Eq                => Instruction::Eq,
        Op::NotEq             => Instruction::NotEq,
        Op::Lt                => Instruction::Lt,
        Op::LtEq              => Instruction::LtEq,
        Op::Gt                => Instruction::Gt,
        Op::GtEq              => Instruction::GtEq,
        Op::BitAnd            => Instruction::BitAnd,
        Op::BitOr             => Instruction::BitOr,
        Op::BitXor            => Instruction::BitXor,
        Op::Shl               => Instruction::Shl,
        Op::Shr               => Instruction::Shr,
        Op::Jump(off)         => Instruction::Jump(*off as isize),
        Op::JumpIf(off)       => Instruction::JumpIf(*off as isize),
        Op::JumpIfNot(off)    => Instruction::JumpIfNot(*off as isize),
        Op::JumpIfNotPop(off) => Instruction::JumpIfNotPop(*off as isize),
        Op::JumpIfPop(off)    => Instruction::JumpIfPop(*off as isize),
        Op::Call(na, _)       => Instruction::Call(*na as usize),
        Op::CallMethod(k, na, _) => Instruction::CallMethod(get_str(*k), *na as usize),
        Op::Return(n)         => Instruction::Return(*n as usize),
        Op::TailCall(n)       => Instruction::Call(*n as usize),
        Op::Closure(idx)      => Instruction::MakeFunction(*idx as usize),
        Op::Close(s)          => Instruction::CloseUpvalue(*s as usize),
        Op::ForPrep(n)        => Instruction::ForPrep(*n as usize),
        Op::ForStep           => Instruction::Nop,
        Op::ForStepJump(off)  => Instruction::ForStep(0, *off as isize),
        Op::NumForInit        => Instruction::Nop,
        Op::NumForStep(off)   => Instruction::ForStep(0, *off as isize),
        Op::Vararg(_)         => Instruction::Nop,
        Op::LineInfo(_)       => Instruction::Nop,
    }
}

// ── Local variable tracking ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Local {
    name:  String,
    slot:  u16,
    depth: usize,
}

struct Scope {
    locals:      Vec<Local>,
    scope_depth: usize,
    next_slot:   u16,
}

impl Scope {
    fn new() -> Self {
        Scope { locals: Vec::new(), scope_depth: 0, next_slot: 0 }
    }

    fn push_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn pop_scope(&mut self) -> u16 {
        let depth = self.scope_depth;
        let before = self.locals.len();
        self.locals.retain(|l| l.depth < depth);
        let removed = (before - self.locals.len()) as u16;
        self.next_slot -= removed;
        self.scope_depth -= 1;
        removed
    }

    fn add_local(&mut self, name: &str) -> u16 {
        let slot = self.next_slot;
        self.locals.push(Local {
            name: name.to_string(),
            slot,
            depth: self.scope_depth,
        });
        self.next_slot += 1;
        slot
    }

    fn resolve_local(&self, name: &str) -> Option<u16> {
        self.locals.iter().rev()
            .find(|l| l.name == name)
            .map(|l| l.slot)
    }
}

// ── Upvalue descriptor ────────────────────────────────────────────────────────

/// How an upvalue is captured by a closure.
#[derive(Debug, Clone)]
pub struct UpvalDesc {
    pub name:     String,
    /// If true, the upvalue is a local slot in the immediately enclosing scope;
    /// otherwise it is an upvalue of the enclosing function.
    pub in_stack: bool,
    pub index:    u16,
}

// ── Compiler ─────────────────────────────────────────────────────────────────

/// Single-pass bytecode compiler.
pub struct Compiler {
    proto:  Proto,
    scope:  Scope,
    breaks: Vec<Vec<usize>>,   // break-patch points indexed by loop nesting
    // Note: upvalue handling is simplified — outer-function locals captured
    // as globals in this basic implementation.
}

impl Compiler {
    // ── Public entry points ───────────────────────────────────────────────────

    /// Compile an entire script into a top-level `Arc<Chunk>` for the VM.
    pub fn compile_script(script: &Script) -> Arc<Chunk> {
        proto_to_chunk(&Self::compile_to_proto(script))
    }

    /// Compile to the internal `Proto` representation (used by compiler tests).
    pub fn compile_to_proto(script: &Script) -> Proto {
        let mut c = Compiler {
            proto:  Proto::new(&script.name),
            scope:  Scope::new(),
            breaks: Vec::new(),
        };
        c.proto.is_vararg = true;
        c.compile_block_no_scope(&script.stmts);
        c.proto.emit(Op::Return(0));
        c.proto
    }

    // ── Block compilation ─────────────────────────────────────────────────────

    fn compile_block(&mut self, stmts: &[Stmt]) {
        self.scope.push_scope();
        for s in stmts { self.compile_stmt(s); }
        let popped = self.scope.pop_scope();
        for _ in 0..popped { self.proto.emit(Op::Pop); }
    }

    fn compile_block_no_scope(&mut self, stmts: &[Stmt]) {
        for s in stmts { self.compile_stmt(s); }
    }

    // ── Statement compilation ─────────────────────────────────────────────────

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::LocalDecl { name, init } => {
                if let Some(expr) = init {
                    self.compile_expr(expr);
                } else {
                    self.proto.emit(Op::Nil);
                }
                self.scope.add_local(name);
            }

            Stmt::LocalMulti { names, inits } => {
                for (i, name) in names.iter().enumerate() {
                    if i < inits.len() {
                        self.compile_expr(&inits[i]);
                    } else {
                        self.proto.emit(Op::Nil);
                    }
                    self.scope.add_local(name);
                }
            }

            Stmt::Assign { target, value } => {
                for (i, t) in target.iter().enumerate() {
                    if i < value.len() {
                        self.compile_expr(&value[i]);
                    } else {
                        self.proto.emit(Op::Nil);
                    }
                    self.compile_assign_target(t);
                }
            }

            Stmt::CompoundAssign { target, op, value } => {
                self.compile_expr(target);
                self.compile_expr(value);
                self.compile_binop(*op);
                self.compile_assign_target(target);
            }

            Stmt::Call(expr) | Stmt::Expr(expr) => {
                self.compile_expr(expr);
                self.proto.emit(Op::Pop);
            }

            Stmt::Do(body) => {
                self.compile_block(body);
            }

            Stmt::While { cond, body } => {
                let loop_start = self.proto.code.len() as i32;
                self.compile_expr(cond);
                let exit = self.proto.emit(Op::JumpIfNot(0));
                self.breaks.push(Vec::new());
                self.compile_block(body);
                let back = loop_start - self.proto.code.len() as i32 - 1;
                self.proto.emit(Op::Jump(back));
                self.proto.patch_jump(exit);
                for b in self.breaks.pop().unwrap_or_default() {
                    self.proto.patch_jump(b);
                }
            }

            Stmt::RepeatUntil { body, cond } => {
                let loop_start = self.proto.code.len() as i32;
                self.breaks.push(Vec::new());
                self.compile_block(body);
                self.compile_expr(cond);
                let back = loop_start - self.proto.code.len() as i32 - 1;
                self.proto.emit(Op::JumpIfNot(back));
                for b in self.breaks.pop().unwrap_or_default() {
                    self.proto.patch_jump(b);
                }
            }

            Stmt::If { cond, then_body, elseif_branches, else_body } => {
                self.compile_expr(cond);
                let skip_then = self.proto.emit(Op::JumpIfNot(0));
                self.compile_block(then_body);

                let mut end_jumps = Vec::new();
                if !elseif_branches.is_empty() || else_body.is_some() {
                    end_jumps.push(self.proto.emit(Op::Jump(0)));
                }
                self.proto.patch_jump(skip_then);

                for (ei_cond, ei_body) in elseif_branches {
                    self.compile_expr(ei_cond);
                    let skip = self.proto.emit(Op::JumpIfNot(0));
                    self.compile_block(ei_body);
                    end_jumps.push(self.proto.emit(Op::Jump(0)));
                    self.proto.patch_jump(skip);
                }

                if let Some(eb) = else_body {
                    self.compile_block(eb);
                }
                for j in end_jumps { self.proto.patch_jump(j); }
            }

            Stmt::NumericFor { var, start, limit, step, body } => {
                self.compile_expr(start);
                self.compile_expr(limit);
                if let Some(s) = step {
                    self.compile_expr(s);
                } else {
                    let k = self.proto.add_const(Constant::Int(1));
                    self.proto.emit(Op::Const(k));
                }
                self.proto.emit(Op::NumForInit);
                let loop_top = self.proto.code.len();
                let exit = self.proto.emit(Op::NumForStep(0));

                self.scope.push_scope();
                let slot = self.scope.add_local(var);
                self.proto.emit(Op::GetLocal(slot));
                self.breaks.push(Vec::new());
                for s in body { self.compile_stmt(s); }
                self.scope.pop_scope();

                let back = loop_top as i32 - self.proto.code.len() as i32 - 1;
                self.proto.emit(Op::Jump(back));
                self.proto.patch_jump(exit);
                // Pop limit, step, counter
                for _ in 0..3 { self.proto.emit(Op::Pop); }
                for b in self.breaks.pop().unwrap_or_default() {
                    self.proto.patch_jump(b);
                }
            }

            Stmt::GenericFor { vars, iter, body } => {
                for expr in iter { self.compile_expr(expr); }
                self.proto.emit(Op::ForPrep(vars.len() as u16));
                let loop_top = self.proto.code.len();
                let exit = self.proto.emit(Op::ForStepJump(0));

                self.scope.push_scope();
                for name in vars {
                    let slot = self.scope.add_local(name);
                    self.proto.emit(Op::GetLocal(slot));
                }
                self.breaks.push(Vec::new());
                for s in body { self.compile_stmt(s); }
                self.scope.pop_scope();

                let back = loop_top as i32 - self.proto.code.len() as i32 - 1;
                self.proto.emit(Op::Jump(back));
                self.proto.patch_jump(exit);
                for b in self.breaks.pop().unwrap_or_default() {
                    self.proto.patch_jump(b);
                }
            }

            Stmt::FuncDecl { name, params, vararg, body } => {
                let fn_proto = self.compile_func(
                    name.last().map(|s| s.as_str()).unwrap_or("?"),
                    params, *vararg, body,
                );
                let idx = self.proto.protos.len() as u32;
                self.proto.protos.push(fn_proto);
                self.proto.emit(Op::Closure(idx));

                if name.len() == 1 {
                    if let Some(slot) = self.scope.resolve_local(&name[0]) {
                        self.proto.emit(Op::SetLocal(slot));
                    } else {
                        let k = self.proto.add_const(Constant::Str(name[0].clone()));
                        self.proto.emit(Op::SetGlobal(k));
                    }
                } else {
                    // a.b.fn = closure
                    self.compile_expr(&Expr::Ident(name[0].clone()));
                    for part in &name[1..name.len()-1] {
                        let k = self.proto.add_const(Constant::Str(part.clone()));
                        self.proto.emit(Op::GetField(k));
                    }
                    let last = name.last().unwrap();
                    let k = self.proto.add_const(Constant::Str(last.clone()));
                    self.proto.emit(Op::SetField(k));
                }
            }

            Stmt::LocalFunc { name, params, vararg, body } => {
                let slot = self.scope.add_local(name);
                self.proto.emit(Op::Nil); // placeholder until closure is made
                let fn_proto = self.compile_func(name, params, *vararg, body);
                let idx = self.proto.protos.len() as u32;
                self.proto.protos.push(fn_proto);
                self.proto.emit(Op::Closure(idx));
                self.proto.emit(Op::SetLocal(slot));
            }

            Stmt::Return(vals) => {
                let n = vals.len() as u8;
                for v in vals { self.compile_expr(v); }
                self.proto.emit(Op::Return(n));
            }

            Stmt::Break => {
                let j = self.proto.emit(Op::Jump(0));
                if let Some(list) = self.breaks.last_mut() {
                    list.push(j);
                }
            }

            Stmt::Continue => {
                // Simplified continue: jump to -1 (loop should handle by re-check)
                self.proto.emit(Op::Jump(-1));
            }

            Stmt::Match { expr, arms } => {
                self.compile_expr(expr);
                let mut end_jumps = Vec::new();

                for arm in arms {
                    self.proto.emit(Op::Dup);
                    match &arm.pattern {
                        MatchPattern::Wildcard => {
                            self.proto.emit(Op::Pop);
                            self.compile_block(&arm.body);
                            end_jumps.push(self.proto.emit(Op::Jump(0)));
                            continue;
                        }
                        MatchPattern::Ident(bind) => {
                            let slot = self.scope.add_local(bind);
                            self.proto.emit(Op::SetLocal(slot));
                            self.compile_block(&arm.body);
                            end_jumps.push(self.proto.emit(Op::Jump(0)));
                            continue;
                        }
                        MatchPattern::Literal(lit) => {
                            self.compile_expr(lit);
                            self.proto.emit(Op::Eq);
                        }
                        MatchPattern::Table(_) => {
                            self.proto.emit(Op::Pop);
                            self.proto.emit(Op::True);
                        }
                    }
                    let skip = self.proto.emit(Op::JumpIfNot(0));
                    self.compile_block(&arm.body);
                    end_jumps.push(self.proto.emit(Op::Jump(0)));
                    self.proto.patch_jump(skip);
                }

                self.proto.emit(Op::Pop);
                for j in end_jumps { self.proto.patch_jump(j); }
            }

            Stmt::Import { path, alias } => {
                let k = self.proto.add_const(Constant::Str(path.clone()));
                self.proto.emit(Op::Const(k));
                let rk = self.proto.add_const(Constant::Str("require".to_string()));
                self.proto.emit(Op::GetGlobal(rk));
                self.proto.emit(Op::Swap);
                self.proto.emit(Op::Call(1, 1));
                let bind = alias.clone().unwrap_or_else(|| {
                    path.split('/').last().unwrap_or(path).trim_end_matches(".lua").to_string()
                });
                let bk = self.proto.add_const(Constant::Str(bind));
                self.proto.emit(Op::SetGlobal(bk));
            }

            Stmt::Export(name) => {
                if let Some(slot) = self.scope.resolve_local(name) {
                    self.proto.emit(Op::GetLocal(slot));
                } else {
                    let k = self.proto.add_const(Constant::Str(name.clone()));
                    self.proto.emit(Op::GetGlobal(k));
                }
                let ek = self.proto.add_const(Constant::Str(name.clone()));
                let exports_k = self.proto.add_const(Constant::Str("__exports".to_string()));
                self.proto.emit(Op::GetGlobal(exports_k));
                self.proto.emit(Op::Swap);
                self.proto.emit(Op::SetField(ek));
            }
        }
    }

    fn compile_assign_target(&mut self, target: &Expr) {
        match target {
            Expr::Ident(name) => {
                if let Some(slot) = self.scope.resolve_local(name) {
                    self.proto.emit(Op::SetLocal(slot));
                } else {
                    let k = self.proto.add_const(Constant::Str(name.clone()));
                    self.proto.emit(Op::SetGlobal(k));
                }
            }
            Expr::Field { table, name } => {
                self.compile_expr(table);
                let k = self.proto.add_const(Constant::Str(name.clone()));
                self.proto.emit(Op::SetField(k));
            }
            Expr::Index { table, key } => {
                self.compile_expr(table);
                self.compile_expr(key);
                self.proto.emit(Op::SetIndex);
            }
            _ => {}
        }
    }

    // ── Expression compilation ────────────────────────────────────────────────

    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Nil         => { self.proto.emit(Op::Nil); }
            Expr::Bool(b)     => { self.proto.emit(if *b { Op::True } else { Op::False }); }
            Expr::Int(n)      => { let k = self.proto.add_const(Constant::Int(*n)); self.proto.emit(Op::Const(k)); }
            Expr::Float(f)    => { let k = self.proto.add_const(Constant::Float(*f)); self.proto.emit(Op::Const(k)); }
            Expr::Str(s)      => { let k = self.proto.add_const(Constant::Str(s.clone())); self.proto.emit(Op::Const(k)); }
            Expr::Vararg      => { self.proto.emit(Op::Vararg(0)); }

            Expr::Ident(name) => {
                if let Some(slot) = self.scope.resolve_local(name) {
                    self.proto.emit(Op::GetLocal(slot));
                } else {
                    let k = self.proto.add_const(Constant::Str(name.clone()));
                    self.proto.emit(Op::GetGlobal(k));
                }
            }

            Expr::Field { table, name } => {
                self.compile_expr(table);
                let k = self.proto.add_const(Constant::Str(name.clone()));
                self.proto.emit(Op::GetField(k));
            }

            Expr::Index { table, key } => {
                self.compile_expr(table);
                self.compile_expr(key);
                self.proto.emit(Op::GetIndex);
            }

            Expr::Call { callee, args } => {
                self.compile_expr(callee);
                let nargs = args.len() as u8;
                for a in args { self.compile_expr(a); }
                self.proto.emit(Op::Call(nargs, 1));
            }

            Expr::MethodCall { obj, method, args } => {
                self.compile_expr(obj);
                let k = self.proto.add_const(Constant::Str(method.clone()));
                let nargs = args.len() as u8;
                for a in args { self.compile_expr(a); }
                self.proto.emit(Op::CallMethod(k, nargs, 1));
            }

            Expr::Unary { op, expr } => {
                self.compile_expr(expr);
                match op {
                    UnOp::Neg    => { self.proto.emit(Op::Neg); }
                    UnOp::Not    => { self.proto.emit(Op::Not); }
                    UnOp::Len    => { self.proto.emit(Op::Len); }
                    UnOp::BitNot => { self.proto.emit(Op::BitNot); }
                }
            }

            Expr::Binary { op, lhs, rhs } => {
                match op {
                    BinOp::And => {
                        self.compile_expr(lhs);
                        let j = self.proto.emit(Op::JumpIfNotPop(0));
                        self.compile_expr(rhs);
                        self.proto.patch_jump(j);
                        return;
                    }
                    BinOp::Or => {
                        self.compile_expr(lhs);
                        let j = self.proto.emit(Op::JumpIfPop(0));
                        self.compile_expr(rhs);
                        self.proto.patch_jump(j);
                        return;
                    }
                    _ => {}
                }
                self.compile_expr(lhs);
                self.compile_expr(rhs);
                self.compile_binop(*op);
            }

            Expr::TableCtor(fields) => {
                self.proto.emit(Op::NewTable);
                let mut array_count = 0u16;
                for field in fields {
                    match field {
                        TableField::NameKey(name, val) => {
                            self.proto.emit(Op::Dup);
                            self.compile_expr(val);
                            let k = self.proto.add_const(Constant::Str(name.clone()));
                            self.proto.emit(Op::SetField(k));
                        }
                        TableField::ExprKey(key, val) => {
                            self.proto.emit(Op::Dup);
                            self.compile_expr(key);
                            self.compile_expr(val);
                            self.proto.emit(Op::SetIndex);
                        }
                        TableField::Value(val) => {
                            self.proto.emit(Op::Dup);
                            self.compile_expr(val);
                            array_count += 1;
                            let k = self.proto.add_const(Constant::Int(array_count as i64));
                            self.proto.emit(Op::SetField(k));
                        }
                    }
                }
            }

            Expr::FuncExpr { params, vararg, body } => {
                let fn_proto = self.compile_func("<anon>", params, *vararg, body);
                let idx = self.proto.protos.len() as u32;
                self.proto.protos.push(fn_proto);
                self.proto.emit(Op::Closure(idx));
            }

            Expr::Ternary { cond, then_val, else_val } => {
                self.compile_expr(cond);
                let skip = self.proto.emit(Op::JumpIfNot(0));
                self.compile_expr(then_val);
                let end = self.proto.emit(Op::Jump(0));
                self.proto.patch_jump(skip);
                self.compile_expr(else_val);
                self.proto.patch_jump(end);
            }
        }
    }

    fn compile_binop(&mut self, op: BinOp) {
        let instr = match op {
            BinOp::Add    => Op::Add,
            BinOp::Sub    => Op::Sub,
            BinOp::Mul    => Op::Mul,
            BinOp::Div    => Op::Div,
            BinOp::IDiv   => Op::IDiv,
            BinOp::Mod    => Op::Mod,
            BinOp::Pow    => Op::Pow,
            BinOp::Concat => Op::Concat,
            BinOp::Eq     => Op::Eq,
            BinOp::NotEq  => Op::NotEq,
            BinOp::Lt     => Op::Lt,
            BinOp::LtEq   => Op::LtEq,
            BinOp::Gt     => Op::Gt,
            BinOp::GtEq   => Op::GtEq,
            BinOp::And    => Op::BitAnd,
            BinOp::Or     => Op::BitOr,
            BinOp::BitAnd => Op::BitAnd,
            BinOp::BitOr  => Op::BitOr,
            BinOp::BitXor => Op::BitXor,
            BinOp::Shl    => Op::Shl,
            BinOp::Shr    => Op::Shr,
        };
        self.proto.emit(instr);
    }

    fn compile_func(&mut self, name: &str, params: &[String], vararg: bool, body: &[Stmt]) -> Proto {
        let mut child = Compiler {
            proto:  Proto::new(name),
            scope:  Scope::new(),
            breaks: Vec::new(),
        };
        child.proto.param_count = params.len() as u8;
        child.proto.is_vararg   = vararg;
        child.scope.push_scope();
        for p in params { child.scope.add_local(p); }
        for s in body   { child.compile_stmt(s); }
        child.scope.pop_scope();
        child.proto.emit(Op::Return(0));
        child.proto
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::parser;

    fn compile_src(src: &str) -> Proto {
        let script = parser::parse(src, "test").expect("parse failed");
        Compiler::compile_to_proto(&script)
    }

    #[test]
    fn test_compile_nil() {
        let p = compile_src("local x");
        assert!(p.code.iter().any(|op| *op == Op::Nil));
    }

    #[test]
    fn test_compile_int_const() {
        let p = compile_src("local x = 42");
        assert!(p.constants.iter().any(|c| *c == Constant::Int(42)));
    }

    #[test]
    fn test_compile_float_const() {
        let p = compile_src("local pi = 3.14");
        assert!(p.constants.iter().any(|c| matches!(c, Constant::Float(f) if (*f - 3.14).abs() < 1e-6)));
    }

    #[test]
    fn test_compile_string_const() {
        let p = compile_src(r#"local s = "hello""#);
        assert!(p.constants.iter().any(|c| *c == Constant::Str("hello".to_string())));
    }

    #[test]
    fn test_compile_add() {
        let p = compile_src("local z = 1 + 2");
        assert!(p.code.iter().any(|op| *op == Op::Add));
    }

    #[test]
    fn test_compile_bool_true() {
        let p = compile_src("local b = true");
        assert!(p.code.iter().any(|op| *op == Op::True));
    }

    #[test]
    fn test_compile_while_has_back_jump() {
        let p = compile_src("local i = 0 while i < 10 do i = i + 1 end");
        let has_exit = p.code.iter().any(|op| matches!(op, Op::JumpIfNot(_)));
        let has_back = p.code.iter().any(|op| matches!(op, Op::Jump(n) if *n < 0));
        assert!(has_exit, "expected JumpIfNot");
        assert!(has_back, "expected backward Jump");
    }

    #[test]
    fn test_compile_if_else() {
        let p = compile_src("if x then return 1 else return 2 end");
        assert!(p.code.iter().any(|op| matches!(op, Op::JumpIfNot(_))));
        assert!(p.code.iter().any(|op| matches!(op, Op::Jump(_))));
    }

    #[test]
    fn test_compile_function_creates_proto() {
        let p = compile_src("function add(a, b) return a + b end");
        assert!(!p.protos.is_empty());
        assert_eq!(p.protos[0].param_count, 2);
    }

    #[test]
    fn test_compile_local_function() {
        let p = compile_src("local function square(x) return x * x end");
        assert!(p.code.iter().any(|op| matches!(op, Op::Closure(_))));
        assert!(!p.protos.is_empty());
    }

    #[test]
    fn test_compile_table_ctor() {
        let p = compile_src("local t = {x = 1, y = 2}");
        assert!(p.code.iter().any(|op| *op == Op::NewTable));
        assert!(p.code.iter().any(|op| matches!(op, Op::SetField(_))));
    }

    #[test]
    fn test_compile_method_call() {
        let p = compile_src("obj:doThing(1, 2)");
        assert!(p.code.iter().any(|op| matches!(op, Op::CallMethod(..))));
    }

    #[test]
    fn test_compile_for_numeric() {
        let p = compile_src("for i = 1, 10, 2 do end");
        assert!(p.code.iter().any(|op| *op == Op::NumForInit));
        assert!(p.code.iter().any(|op| matches!(op, Op::NumForStep(_))));
    }

    #[test]
    fn test_compile_for_generic() {
        let p = compile_src("for k, v in pairs(t) do end");
        assert!(p.code.iter().any(|op| matches!(op, Op::ForPrep(_))));
    }

    #[test]
    fn test_compile_and_short_circuit() {
        let p = compile_src("local r = a and b");
        assert!(p.code.iter().any(|op| matches!(op, Op::JumpIfNotPop(_))));
    }

    #[test]
    fn test_compile_or_short_circuit() {
        let p = compile_src("local r = a or b");
        assert!(p.code.iter().any(|op| matches!(op, Op::JumpIfPop(_))));
    }

    #[test]
    fn test_compile_ternary() {
        let p = compile_src("local x = cond ? 1 : 2");
        assert!(p.code.iter().any(|op| matches!(op, Op::JumpIfNot(_))));
    }

    #[test]
    fn test_compile_nested_function() {
        let p = compile_src("
            function outer(x)
                local function inner(y) return x + y end
                return inner(10)
            end
        ");
        assert!(!p.protos.is_empty());
        let outer = &p.protos[0];
        assert!(!outer.protos.is_empty(), "expected inner proto");
    }

    #[test]
    fn test_compile_concat() {
        let p = compile_src(r#"local s = "hello" .. " " .. "world""#);
        assert!(p.code.iter().filter(|op| **op == Op::Concat).count() >= 1);
    }

    #[test]
    fn test_compile_repeat_until() {
        let p = compile_src("local i = 0 repeat i = i + 1 until i >= 10");
        assert!(p.code.iter().any(|op| matches!(op, Op::JumpIfNot(n) if *n < 0)));
    }

    #[test]
    fn test_compile_match() {
        let p = compile_src("match x { case 1 => return 1, case 2 => return 2 }");
        assert!(p.code.iter().any(|op| *op == Op::Dup));
        assert!(p.code.iter().any(|op| *op == Op::Eq));
    }

    #[test]
    fn test_compile_import() {
        let p = compile_src(r#"import "math" as m"#);
        assert!(p.constants.iter().any(|c| *c == Constant::Str("math".to_string())));
        assert!(p.constants.iter().any(|c| *c == Constant::Str("require".to_string())));
    }

    #[test]
    fn test_add_const_deduplication() {
        let mut p = Proto::new("test");
        let i1 = p.add_const(Constant::Int(42));
        let i2 = p.add_const(Constant::Int(42));
        assert_eq!(i1, i2, "deduplication failed");
        assert_eq!(p.constants.len(), 1);
    }
}
