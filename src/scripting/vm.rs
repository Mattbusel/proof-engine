//! Stack-based bytecode virtual machine.
//!
//! Executes `Proto` bytecode produced by the `Compiler`.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use super::compiler::{Chunk, Instruction, Constant};

// ── Value ────────────────────────────────────────────────────────────────────

/// A runtime value in the scripting VM.
#[derive(Clone, Debug)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Arc<String>),
    Table(Table),
    Function(Arc<Closure>),
    NativeFunction(Arc<NativeFunc>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil)                   => true,
            (Value::Bool(a), Value::Bool(b))           => a == b,
            (Value::Int(a), Value::Int(b))             => a == b,
            (Value::Float(a), Value::Float(b))         => a == b,
            (Value::Str(a), Value::Str(b))             => a == b,
            (Value::Int(a), Value::Float(b))           => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b))           => *a == (*b as f64),
            (Value::Table(a), Value::Table(b))         => Arc::ptr_eq(&a.inner, &b.inner),
            (Value::Function(a), Value::Function(b))   => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil              => write!(f, "nil"),
            Value::Bool(b)         => write!(f, "{}", b),
            Value::Int(n)          => write!(f, "{}", n),
            Value::Float(n)        => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    write!(f, "{:.1}", n)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::Str(s)          => write!(f, "{}", s),
            Value::Table(_)        => write!(f, "table"),
            Value::Function(_)     => write!(f, "function"),
            Value::NativeFunction(n) => write!(f, "function: {}", n.name),
        }
    }
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil              => "nil",
            Value::Bool(_)         => "boolean",
            Value::Int(_)          => "integer",
            Value::Float(_)        => "float",
            Value::Str(_)          => "string",
            Value::Table(_)        => "table",
            Value::Function(_)     => "function",
            Value::NativeFunction(_) => "function",
        }
    }

    pub fn to_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i)   => Some(*i as f64),
            Value::Str(s)   => s.parse::<f64>().ok(),
            _ => None,
        }
    }

    pub fn to_int(&self) -> Option<i64> {
        match self {
            Value::Int(i)   => Some(*i),
            Value::Float(f) => {
                if f.fract() == 0.0 { Some(*f as i64) } else { None }
            }
            Value::Str(s) => s.parse::<i64>().ok(),
            _ => None,
        }
    }

    pub fn to_str_repr(&self) -> Option<String> {
        match self {
            Value::Str(s)   => Some(s.as_ref().clone()),
            Value::Int(i)   => Some(i.to_string()),
            Value::Float(f) => Some(f.to_string()),
            _ => None,
        }
    }
}


// ── Table ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Table {
    inner: Arc<std::cell::RefCell<TableData>>,
}

#[derive(Debug)]
struct TableData {
    hash:    HashMap<TableKey, Value>,
    array:   Vec<Value>,      // 1-indexed values stored at [idx-1]
    metatable: Option<Table>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TableKey {
    Str(String),
    Int(i64),
    Bool(bool),
}

impl TableKey {
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Str(s)  => Some(TableKey::Str(s.as_ref().clone())),
            Value::Int(i)  => Some(TableKey::Int(*i)),
            Value::Bool(b) => Some(TableKey::Bool(*b)),
            Value::Float(f) => {
                let i = *f as i64;
                if i as f64 == *f { Some(TableKey::Int(i)) } else { None }
            }
            _ => None,
        }
    }
}

impl Table {
    pub fn new() -> Self {
        Table { inner: Arc::new(std::cell::RefCell::new(TableData {
            hash: HashMap::new(),
            array: Vec::new(),
            metatable: None,
        }))}
    }

    pub fn get(&self, key: &Value) -> Value {
        let d = self.inner.borrow();
        // Integer keys 1..len use array part
        if let Some(i) = int_key(key) {
            if i >= 1 {
                if let Some(v) = d.array.get((i - 1) as usize) {
                    return v.clone();
                }
                return Value::Nil;
            }
        }
        TableKey::from_value(key)
            .and_then(|k| d.hash.get(&k).cloned())
            .unwrap_or(Value::Nil)
    }

    pub fn rawget_str(&self, key: &str) -> Value {
        let d = self.inner.borrow();
        d.hash.get(&TableKey::Str(key.to_string())).cloned().unwrap_or(Value::Nil)
    }

    pub fn set(&self, key: Value, val: Value) {
        let mut d = self.inner.borrow_mut();
        if let Some(i) = int_key(&key) {
            if i >= 1 {
                let idx = (i - 1) as usize;
                if idx <= d.array.len() {
                    match val {
                        Value::Nil if idx < d.array.len() => { d.array[idx] = Value::Nil; }
                        Value::Nil => {}
                        v => {
                            if idx == d.array.len() {
                                d.array.push(v);
                            } else {
                                d.array[idx] = v;
                            }
                        }
                    }
                    return;
                }
            }
        }
        if let Some(k) = TableKey::from_value(&key) {
            match val {
                Value::Nil => { d.hash.remove(&k); }
                v          => { d.hash.insert(k, v); }
            }
        }
    }

    pub fn rawset_str(&self, key: &str, val: Value) {
        let mut d = self.inner.borrow_mut();
        match val {
            Value::Nil => { d.hash.remove(&TableKey::Str(key.to_string())); }
            v          => { d.hash.insert(TableKey::Str(key.to_string()), v); }
        }
    }

    pub fn length(&self) -> i64 {
        self.inner.borrow().array.len() as i64
    }

    pub fn push(&self, v: Value) {
        self.inner.borrow_mut().array.push(v);
    }

    pub fn array_values(&self) -> Vec<Value> {
        self.inner.borrow().array.clone()
    }

    pub fn set_metatable(&self, mt: Option<Table>) {
        self.inner.borrow_mut().metatable = mt;
    }

    pub fn get_metatable(&self) -> Option<Table> {
        self.inner.borrow().metatable.clone()
    }

    /// Iterate: returns (key, value) pairs starting after `after` key.
    pub fn next(&self, after: &Value) -> Option<(Value, Value)> {
        let d = self.inner.borrow();
        match after {
            Value::Nil => {
                // Start: first array element
                if let Some(v) = d.array.first() {
                    return Some((Value::Int(1), v.clone()));
                }
                // Or first hash element
                return d.hash.iter().next().map(|(k, v)| (tk_to_val(k), v.clone()));
            }
            _ => {
                // Check if in array
                if let Some(i) = int_key(after) {
                    if i >= 1 {
                        let next_i = i as usize; // 0-based: i is 1-indexed
                        if next_i < d.array.len() {
                            return Some((Value::Int(i + 1), d.array[next_i].clone()));
                        }
                        // Transition to hash
                        return d.hash.iter().next().map(|(k, v)| (tk_to_val(k), v.clone()));
                    }
                }
                // In hash: find current key then return next
                if let Some(k) = TableKey::from_value(after) {
                    let mut found = false;
                    for (hk, hv) in &d.hash {
                        if found { return Some((tk_to_val(hk), hv.clone())); }
                        if *hk == k { found = true; }
                    }
                }
                None
            }
        }
    }
}

fn int_key(v: &Value) -> Option<i64> {
    match v {
        Value::Int(i)  => Some(*i),
        Value::Float(f) if f.fract() == 0.0 => Some(*f as i64),
        _ => None,
    }
}

fn tk_to_val(k: &TableKey) -> Value {
    match k {
        TableKey::Str(s)  => Value::Str(Arc::new(s.clone())),
        TableKey::Int(i)  => Value::Int(*i),
        TableKey::Bool(b) => Value::Bool(*b),
    }
}

// ── Closure / NativeFunc ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Closure {
    pub chunk:    Arc<Chunk>,
    pub upvalues: Vec<UpvalueCell>,
}

#[derive(Debug, Clone)]
pub struct UpvalueCell(Arc<std::cell::RefCell<Value>>);

impl UpvalueCell {
    pub fn new(v: Value) -> Self {
        UpvalueCell(Arc::new(std::cell::RefCell::new(v)))
    }

    pub fn get(&self) -> Value {
        self.0.borrow().clone()
    }

    pub fn set(&self, v: Value) {
        *self.0.borrow_mut() = v;
    }
}

pub struct NativeFunc {
    pub name: String,
    pub func: Box<dyn Fn(&mut Vm, Vec<Value>) -> Result<Vec<Value>, ScriptError> + Send + Sync>,
}

impl fmt::Debug for NativeFunc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NativeFunc({})", self.name)
    }
}

// ── ScriptError ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ScriptError {
    pub message: String,
    pub line:    Option<u32>,
}

impl ScriptError {
    pub fn new(msg: impl Into<String>) -> Self {
        ScriptError { message: msg.into(), line: None }
    }

    pub fn at(msg: impl Into<String>, line: u32) -> Self {
        ScriptError { message: msg.into(), line: Some(line) }
    }

    fn runtime(msg: impl Into<String>) -> Self {
        ScriptError::new(msg)
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "[line {}] {}", line, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for ScriptError {}

// ── CallFrame ─────────────────────────────────────────────────────────────────

struct CallFrame {
    chunk:    Arc<Chunk>,
    upvalues: Vec<UpvalueCell>,
    ip:       usize,
    base:     usize,    // absolute stack slot of first local
}

// ── Vm ────────────────────────────────────────────────────────────────────────

const MAX_STACK: usize = 512;
const MAX_DEPTH: usize = 200;

/// The scripting virtual machine.
pub struct Vm {
    stack:   Vec<Value>,
    frames:  Vec<CallFrame>,
    globals: HashMap<String, Value>,
    depth:   usize,
    /// Captured output from print() — used in tests.
    pub output: Vec<String>,
}

impl Vm {
    pub fn new() -> Self {
        Vm {
            stack:   Vec::with_capacity(64),
            frames:  Vec::with_capacity(32),
            globals: HashMap::new(),
            depth:   0,
            output:  Vec::new(),
        }
    }

    pub fn set_global(&mut self, name: impl Into<String>, v: Value) {
        self.globals.insert(name.into(), v);
    }

    pub fn get_global(&self, name: &str) -> Value {
        self.globals.get(name).cloned().unwrap_or(Value::Nil)
    }

    pub fn register_native(
        &mut self,
        name: impl Into<String>,
        f: impl Fn(&mut Vm, Vec<Value>) -> Result<Vec<Value>, ScriptError> + Send + Sync + 'static,
    ) {
        let name = name.into();
        let nf = Arc::new(NativeFunc { name: name.clone(), func: Box::new(f) });
        self.globals.insert(name, Value::NativeFunction(nf));
    }

    /// Execute a compiled `Chunk` as the top-level script.
    pub fn execute(&mut self, chunk: Arc<Chunk>) -> Result<Vec<Value>, ScriptError> {
        self.run_chunk(chunk, vec![], vec![])
    }

    /// Call any callable Value.
    pub fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Vec<Value>, ScriptError> {
        match callee {
            Value::Function(c)       => {
                let chunk    = Arc::clone(&c.chunk);
                let upvalues = c.upvalues.clone();
                self.run_chunk(chunk, upvalues, args)
            }
            Value::NativeFunction(nf) => {
                let func = Arc::clone(&nf);
                (func.func)(self, args)
            }
            other => Err(ScriptError::runtime(format!(
                "attempt to call a {} value", other.type_name()
            ))),
        }
    }

    fn run_chunk(&mut self, chunk: Arc<Chunk>, upvalues: Vec<UpvalueCell>, args: Vec<Value>) -> Result<Vec<Value>, ScriptError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            return Err(ScriptError::runtime("stack overflow"));
        }

        let base = self.stack.len();
        for a in args {
            self.stack.push(a);
        }
        // Fill remaining params with nil
        let param_count = chunk.param_count as usize;
        while self.stack.len() < base + param_count {
            self.stack.push(Value::Nil);
        }

        self.frames.push(CallFrame { chunk, upvalues, ip: 0, base });
        let result = self.run();
        self.depth -= 1;
        result
    }

    fn run(&mut self) -> Result<Vec<Value>, ScriptError> {
        loop {
            let fi = self.frames.len() - 1;
            let ip = self.frames[fi].ip;
            let instrs = self.frames[fi].chunk.instructions.clone();

            if ip >= instrs.len() {
                let base = self.frames[fi].base;
                self.stack.truncate(base);
                self.frames.pop();
                return Ok(vec![]);
            }

            let instr = instrs[ip].clone();
            self.frames[fi].ip += 1;

            match instr {
                Instruction::LoadNil        => self.push(Value::Nil),
                Instruction::LoadBool(b)    => self.push(Value::Bool(b)),
                Instruction::LoadInt(n)     => self.push(Value::Int(n)),
                Instruction::LoadFloat(f)   => self.push(Value::Float(f)),
                Instruction::LoadStr(s)     => self.push(Value::Str(Arc::new(s))),
                Instruction::LoadConst(idx) => {
                    let fi = self.frames.len() - 1;
                    let v = match self.frames[fi].chunk.constants.get(idx) {
                        Some(Constant::Nil)      => Value::Nil,
                        Some(Constant::Bool(b))  => Value::Bool(*b),
                        Some(Constant::Int(n))   => Value::Int(*n),
                        Some(Constant::Float(f)) => Value::Float(*f),
                        Some(Constant::Str(s))   => Value::Str(Arc::new(s.clone())),
                        None                     => Value::Nil,
                    };
                    self.push(v);
                }

                Instruction::Pop => { self.stack.pop(); }
                Instruction::Dup => {
                    let v = self.stack.last().cloned().unwrap_or(Value::Nil);
                    self.push(v);
                }
                Instruction::Swap => {
                    let len = self.stack.len();
                    if len >= 2 { self.stack.swap(len - 1, len - 2); }
                }

                Instruction::GetLocal(slot) => {
                    let fi   = self.frames.len() - 1;
                    let base = self.frames[fi].base;
                    let v = self.stack.get(base + slot).cloned().unwrap_or(Value::Nil);
                    self.push(v);
                }
                Instruction::SetLocal(slot) => {
                    let fi   = self.frames.len() - 1;
                    let base = self.frames[fi].base;
                    let v = self.pop();
                    let idx = base + slot;
                    while self.stack.len() <= idx { self.stack.push(Value::Nil); }
                    self.stack[idx] = v;
                }

                Instruction::GetUpvalue(idx) => {
                    let fi = self.frames.len() - 1;
                    let v = self.frames[fi].upvalues.get(idx)
                        .map(|uv| uv.get())
                        .unwrap_or(Value::Nil);
                    self.push(v);
                }
                Instruction::SetUpvalue(idx) => {
                    let v = self.pop();
                    let fi = self.frames.len() - 1;
                    if let Some(uv) = self.frames[fi].upvalues.get(idx) {
                        uv.set(v);
                    }
                }

                Instruction::GetGlobal(name) => {
                    let v = self.globals.get(&name).cloned().unwrap_or(Value::Nil);
                    self.push(v);
                }
                Instruction::SetGlobal(name) => {
                    let v = self.pop();
                    self.globals.insert(name, v);
                }

                Instruction::NewTable => self.push(Value::Table(Table::new())),

                Instruction::SetField(name) => {
                    let val   = self.pop();
                    let table = self.peek();
                    match table {
                        Value::Table(t) => t.rawset_str(&name, val),
                        _ => return Err(ScriptError::runtime("SetField on non-table")),
                    }
                }
                Instruction::GetField(name) => {
                    let table = self.pop();
                    let v = match &table {
                        Value::Table(t) => t.rawget_str(&name),
                        Value::Str(_)   => {
                            self.globals.get("string")
                                .and_then(|s| if let Value::Table(t) = s { Some(t.rawget_str(&name)) } else { None })
                                .unwrap_or(Value::Nil)
                        }
                        other => return Err(ScriptError::runtime(format!(
                            "attempt to index a {} value", other.type_name()
                        ))),
                    };
                    self.push(v);
                }
                Instruction::SetIndex => {
                    let val   = self.pop();
                    let key   = self.pop();
                    let table = self.pop();
                    match table {
                        Value::Table(t) => t.set(key, val),
                        other => return Err(ScriptError::runtime(format!(
                            "attempt to index a {} value", other.type_name()
                        ))),
                    }
                }
                Instruction::GetIndex => {
                    let key   = self.pop();
                    let table = self.pop();
                    let v = match &table {
                        Value::Table(t) => t.get(&key),
                        other => return Err(ScriptError::runtime(format!(
                            "attempt to index a {} value", other.type_name()
                        ))),
                    };
                    self.push(v);
                }
                Instruction::TableAppend => {
                    let val   = self.pop();
                    let table = self.peek();
                    if let Value::Table(t) = table { t.push(val); }
                }

                Instruction::Len => {
                    let a = self.pop();
                    let n = match a {
                        Value::Table(t) => t.length(),
                        Value::Str(s)   => s.len() as i64,
                        other => return Err(ScriptError::runtime(format!("# on {}", other.type_name()))),
                    };
                    self.push(Value::Int(n));
                }
                Instruction::Neg => {
                    let a = self.pop();
                    let r = match a {
                        Value::Int(i)   => Value::Int(-i),
                        Value::Float(f) => Value::Float(-f),
                        other => return Err(ScriptError::runtime(format!("unary - on {}", other.type_name()))),
                    };
                    self.push(r);
                }
                Instruction::Not    => { let a = self.pop(); self.push(Value::Bool(!a.is_truthy())); }
                Instruction::BitNot => {
                    let a = self.pop();
                    let i = a.to_int().ok_or_else(|| ScriptError::runtime(format!("bitwise on {}", a.type_name())))?;
                    self.push(Value::Int(!i));
                }

                Instruction::Add    => self.arith2(|a, b| num_arith(a, b, i64::wrapping_add, |x, y| x + y))?,
                Instruction::Sub    => self.arith2(|a, b| num_arith(a, b, i64::wrapping_sub, |x, y| x - y))?,
                Instruction::Mul    => self.arith2(|a, b| num_arith(a, b, i64::wrapping_mul, |x, y| x * y))?,
                Instruction::Div    => {
                    let b = self.pop(); let a = self.pop();
                    let af = a.to_float().ok_or_else(|| ScriptError::runtime(format!("arithmetic on {}", a.type_name())))?;
                    let bf = b.to_float().ok_or_else(|| ScriptError::runtime(format!("arithmetic on {}", b.type_name())))?;
                    self.push(Value::Float(af / bf));
                }
                Instruction::IDiv   => {
                    let b = self.pop(); let a = self.pop();
                    let ai = a.to_int().ok_or_else(|| ScriptError::runtime("floor div requires integers"))?;
                    let bi = b.to_int().ok_or_else(|| ScriptError::runtime("floor div requires integers"))?;
                    if bi == 0 { return Err(ScriptError::runtime("integer divide by zero")); }
                    self.push(Value::Int(ai.div_euclid(bi)));
                }
                Instruction::Mod    => {
                    let b = self.pop(); let a = self.pop();
                    let r = match (&a, &b) {
                        (Value::Int(ai), Value::Int(bi)) => {
                            if *bi == 0 { return Err(ScriptError::runtime("modulo by zero")); }
                            Value::Int(ai.rem_euclid(*bi))
                        }
                        _ => {
                            let af = a.to_float().ok_or_else(|| ScriptError::runtime(format!("arithmetic on {}", a.type_name())))?;
                            let bf = b.to_float().ok_or_else(|| ScriptError::runtime(format!("arithmetic on {}", b.type_name())))?;
                            Value::Float(af % bf)
                        }
                    };
                    self.push(r);
                }
                Instruction::Pow    => {
                    let b = self.pop(); let a = self.pop();
                    let af = a.to_float().ok_or_else(|| ScriptError::runtime(format!("arithmetic on {}", a.type_name())))?;
                    let bf = b.to_float().ok_or_else(|| ScriptError::runtime(format!("arithmetic on {}", b.type_name())))?;
                    self.push(Value::Float(af.powf(bf)));
                }
                Instruction::Concat => {
                    let b = self.pop(); let a = self.pop();
                    let sa = a.to_str_repr().ok_or_else(|| ScriptError::runtime(format!("concat on {}", a.type_name())))?;
                    let sb = b.to_str_repr().ok_or_else(|| ScriptError::runtime(format!("concat on {}", b.type_name())))?;
                    self.push(Value::Str(Arc::new(sa + &sb)));
                }

                Instruction::BitAnd => self.bitwise(|a, b| a & b)?,
                Instruction::BitOr  => self.bitwise(|a, b| a | b)?,
                Instruction::BitXor => self.bitwise(|a, b| a ^ b)?,
                Instruction::Shl    => self.bitwise(|a, b| a.wrapping_shl(b as u32))?,
                Instruction::Shr    => self.bitwise(|a, b| a.wrapping_shr(b as u32))?,

                Instruction::Eq    => { let b = self.pop(); let a = self.pop(); self.push(Value::Bool(a == b)); }
                Instruction::NotEq => { let b = self.pop(); let a = self.pop(); self.push(Value::Bool(a != b)); }
                Instruction::Lt    => self.cmp(|a, b| a < b)?,
                Instruction::LtEq  => self.cmp(|a, b| a <= b)?,
                Instruction::Gt    => self.cmp(|a, b| a > b)?,
                Instruction::GtEq  => self.cmp(|a, b| a >= b)?,

                Instruction::Jump(off) => {
                    let fi = self.frames.len() - 1;
                    self.frames[fi].ip = (self.frames[fi].ip as isize + off) as usize;
                }
                Instruction::JumpIf(off) => {
                    if self.peek_clone().is_truthy() {
                        let fi = self.frames.len() - 1;
                        self.frames[fi].ip = (self.frames[fi].ip as isize + off) as usize;
                    }
                }
                Instruction::JumpIfNot(off) => {
                    if !self.peek_clone().is_truthy() {
                        let fi = self.frames.len() - 1;
                        self.frames[fi].ip = (self.frames[fi].ip as isize + off) as usize;
                    }
                }
                Instruction::JumpAbs(addr) => {
                    let fi = self.frames.len() - 1;
                    self.frames[fi].ip = addr;
                }
                Instruction::JumpIfNotPop(off) => {
                    if !self.peek_clone().is_truthy() {
                        let fi = self.frames.len() - 1;
                        self.frames[fi].ip = (self.frames[fi].ip as isize + off) as usize;
                    } else {
                        self.stack.pop();
                    }
                }
                Instruction::JumpIfPop(off) => {
                    if self.peek_clone().is_truthy() {
                        let fi = self.frames.len() - 1;
                        self.frames[fi].ip = (self.frames[fi].ip as isize + off) as usize;
                    } else {
                        self.stack.pop();
                    }
                }

                Instruction::Call(nargs) => {
                    let top  = self.stack.len();
                    let base = top.saturating_sub(nargs + 1);
                    let args: Vec<Value> = self.stack.drain(base + 1..).collect();
                    let callee = self.stack.pop().unwrap_or(Value::Nil);
                    let results = self.call(callee, args)?;
                    for r in results { self.stack.push(r); }
                }

                Instruction::CallMethod(method_name, nargs) => {
                    let top  = self.stack.len();
                    let base = top.saturating_sub(nargs + 1);
                    let extra_args: Vec<Value> = self.stack.drain(base + 1..).collect();
                    let obj = self.stack.pop().unwrap_or(Value::Nil);
                    let method = match &obj {
                        Value::Table(t) => t.rawget_str(&method_name),
                        other => return Err(ScriptError::runtime(format!(
                            "attempt to index {} for method call", other.type_name()
                        ))),
                    };
                    let mut args = vec![obj];
                    args.extend(extra_args);
                    let results = self.call(method, args)?;
                    for r in results { self.stack.push(r); }
                }

                Instruction::Return(nvals) => {
                    let top = self.stack.len();
                    let ret_start = top.saturating_sub(nvals);
                    let returns: Vec<Value> = self.stack.drain(ret_start..).collect();
                    let fi   = self.frames.len() - 1;
                    let base = self.frames[fi].base;
                    self.stack.truncate(base);
                    self.frames.pop();
                    return Ok(returns);
                }

                Instruction::MakeFunction(chunk_idx) => {
                    let fi = self.frames.len() - 1;
                    let sub = self.frames[fi].chunk.sub_chunks[chunk_idx].clone();
                    let closure = Arc::new(Closure { chunk: sub, upvalues: Vec::new() });
                    self.push(Value::Function(closure));
                }

                Instruction::MakeClosure(chunk_idx, upval_specs) => {
                    let fi = self.frames.len() - 1;
                    let sub = self.frames[fi].chunk.sub_chunks[chunk_idx].clone();
                    let base = self.frames[fi].base;
                    let mut upvalues = Vec::new();
                    for (is_local, idx) in &upval_specs {
                        let v = if *is_local {
                            self.stack.get(base + idx).cloned().unwrap_or(Value::Nil)
                        } else {
                            self.frames[fi].upvalues.get(*idx).map(|u| u.get()).unwrap_or(Value::Nil)
                        };
                        upvalues.push(UpvalueCell::new(v));
                    }
                    let closure = Arc::new(Closure { chunk: sub, upvalues });
                    self.push(Value::Function(closure));
                }

                Instruction::CloseUpvalue(_slot) => {}  // simplified

                Instruction::ForPrep(_local_idx) => {}  // validation — simplified

                Instruction::ForStep(local_idx, jump_offset) => {
                    let fi   = self.frames.len() - 1;
                    let base = self.frames[fi].base;
                    let var_idx   = base + local_idx;
                    let limit_idx = base + local_idx + 1;
                    let step_idx  = base + local_idx + 2;

                    let cur   = self.stack.get(var_idx).cloned().unwrap_or(Value::Nil);
                    let limit = self.stack.get(limit_idx).cloned().unwrap_or(Value::Nil);
                    let step  = self.stack.get(step_idx).cloned().unwrap_or(Value::Nil);

                    let cv = cur.to_float().unwrap_or(0.0);
                    let lv = limit.to_float().unwrap_or(0.0);
                    let sv = step.to_float().unwrap_or(1.0);

                    let should_continue = if sv > 0.0 { cv <= lv } else { cv >= lv };
                    if !should_continue {
                        let fi = self.frames.len() - 1;
                        self.frames[fi].ip = (self.frames[fi].ip as isize + jump_offset) as usize;
                    } else {
                        let next = match (&cur, &step) {
                            (Value::Int(c), Value::Int(s)) => Value::Int(c.wrapping_add(*s)),
                            _ => Value::Float(cv + sv),
                        };
                        if var_idx < self.stack.len() { self.stack[var_idx] = next; }
                    }
                }

                Instruction::Nop => {}
            }
        }
    }

    // ── Stack helpers ─────────────────────────────────────────────────────

    #[inline]
    fn push(&mut self, v: Value) { self.stack.push(v); }

    #[inline]
    fn pop(&mut self) -> Value { self.stack.pop().unwrap_or(Value::Nil) }

    #[inline]
    fn peek(&self) -> Value { self.stack.last().cloned().unwrap_or(Value::Nil) }

    #[inline]
    fn peek_clone(&self) -> Value { self.stack.last().cloned().unwrap_or(Value::Nil) }

    fn arith2<F>(&mut self, f: F) -> Result<(), ScriptError>
    where F: Fn(Value, Value) -> Result<Value, ScriptError>
    {
        let b = self.pop();
        let a = self.pop();
        self.push(f(a, b)?);
        Ok(())
    }

    fn cmp<F>(&mut self, op: F) -> Result<(), ScriptError>
    where F: Fn(f64, f64) -> bool
    {
        let b = self.pop();
        let a = self.pop();
        let av = a.to_float().ok_or_else(|| ScriptError::runtime(format!("compare on {}", a.type_name())))?;
        let bv = b.to_float().ok_or_else(|| ScriptError::runtime(format!("compare on {}", b.type_name())))?;
        self.push(Value::Bool(op(av, bv)));
        Ok(())
    }

    fn bitwise<F>(&mut self, op: F) -> Result<(), ScriptError>
    where F: Fn(i64, i64) -> i64
    {
        let b = self.pop();
        let a = self.pop();
        let ai = a.to_int().ok_or_else(|| ScriptError::runtime(format!("bitwise on {}", a.type_name())))?;
        let bi = b.to_int().ok_or_else(|| ScriptError::runtime(format!("bitwise on {}", b.type_name())))?;
        self.push(Value::Int(op(ai, bi)));
        Ok(())
    }
}

fn num_arith<FI, FF>(a: Value, b: Value, fi: FI, ff: FF) -> Result<Value, ScriptError>
where
    FI: Fn(i64, i64) -> i64,
    FF: Fn(f64, f64) -> f64,
{
    match (&a, &b) {
        (Value::Int(ai), Value::Int(bi))     => Ok(Value::Int(fi(*ai, *bi))),
        (Value::Float(af), Value::Float(bf)) => Ok(Value::Float(ff(*af, *bf))),
        (Value::Int(ai), Value::Float(bf))   => Ok(Value::Float(ff(*ai as f64, *bf))),
        (Value::Float(af), Value::Int(bi))   => Ok(Value::Float(ff(*af, *bi as f64))),
        _ => Err(ScriptError::runtime(format!(
            "attempt to perform arithmetic on {} and {} values",
            a.type_name(), b.type_name()
        ))),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::compiler::Compiler;
    use crate::scripting::parser::Parser;

    fn run(src: &str) -> Result<Vec<Value>, ScriptError> {
        let script = Parser::from_source("test", src)
            .map_err(|e| ScriptError::new(e.to_string()))?;
        let chunk = Compiler::compile_script(&script);
        let mut vm = Vm::new();
        vm.execute(chunk)
    }

    #[test]
    fn test_return_int() {
        let result = run("return 42").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Value::Int(42));
    }

    #[test]
    fn test_arithmetic() {
        let result = run("return 2 + 3 * 4").unwrap();
        assert_eq!(result[0], Value::Int(14));
    }

    #[test]
    fn test_string_concat() {
        let result = run("return \"hello\" .. \" \" .. \"world\"").unwrap();
        assert!(matches!(&result[0], Value::Str(s) if s.as_ref() == "hello world"));
    }

    #[test]
    fn test_local_variable() {
        let result = run("local x = 10 return x").unwrap();
        assert_eq!(result[0], Value::Int(10));
    }

    #[test]
    fn test_function_call() {
        let result = run("function add(a, b) return a + b end return add(3, 4)").unwrap();
        assert_eq!(result[0], Value::Int(7));
    }

    #[test]
    fn test_if_else() {
        let result = run("local x = 5 if x > 3 then return 1 else return 0 end").unwrap();
        assert_eq!(result[0], Value::Int(1));
    }

    #[test]
    fn test_while_loop() {
        let result = run("local sum = 0 local i = 1 while i <= 5 do sum = sum + i i = i + 1 end return sum").unwrap();
        assert_eq!(result[0], Value::Int(15));
    }

    #[test]
    fn test_table() {
        let result = run("local t = {} t.x = 42 return t.x").unwrap();
        assert_eq!(result[0], Value::Int(42));
    }

    #[test]
    fn test_value_display() {
        assert_eq!(Value::Nil.to_string(), "nil");
        assert_eq!(Value::Bool(true).to_string(), "true");
        assert_eq!(Value::Int(42).to_string(), "42");
    }

    #[test]
    fn test_vm_globals() {
        let mut vm = Vm::new();
        vm.set_global("answer", Value::Int(42));
        assert_eq!(vm.get_global("answer"), Value::Int(42));
    }
}
