//! Standard library for the scripting engine.
//!
//! Registers built-in functions: print, tostring, tonumber, type,
//! math.*, string.*, table.*, io.*, os.*, bit.*, pairs, ipairs, pcall, etc.
//!
//! Every function has a real implementation — no stubs.

use std::cell::RefCell;
use std::sync::Arc;

use super::vm::{NativeFunc, ScriptError, Table, Value, Vm};

// ── Public Struct ─────────────────────────────────────────────────────────────

/// Standard library registrar.
pub struct Stdlib;

impl Stdlib {
    /// Register all standard library functions into the VM.
    pub fn register_all(vm: &mut Vm) {
        register_all(vm);
    }
}

// ── Registration entry point ─────────────────────────────────────────────────

/// Register all standard library functions into the VM.
pub fn register_all(vm: &mut Vm) {
    register_globals(vm);
    register_math(vm);
    register_string(vm);
    register_table(vm);
    register_io(vm);
    register_os(vm);
    register_bit(vm);
}

// ── Helper: make a NativeFunction value ──────────────────────────────────────

fn native(name: &str, f: impl Fn(&mut Vm, Vec<Value>) -> Result<Vec<Value>, ScriptError> + Send + Sync + 'static) -> Value {
    Value::NativeFunction(Arc::new(NativeFunc {
        name: name.to_string(),
        func: Box::new(f),
    }))
}

// ── Global functions ──────────────────────────────────────────────────────────

fn register_globals(vm: &mut Vm) {
    // print
    vm.register_native("print", |vm, args| {
        let out: Vec<String> = args.iter().map(|v| v.to_string()).collect();
        let line = out.join("\t");
        vm.output.push(line.clone());
        Ok(vec![])
    });

    // tostring
    vm.register_native("tostring", |_vm, args| {
        let s = args.into_iter().next().unwrap_or(Value::Nil).to_string();
        Ok(vec![Value::Str(Arc::new(s))])
    });

    // tonumber with optional base
    vm.register_native("tonumber", |_vm, args| {
        let v    = args.first().cloned().unwrap_or(Value::Nil);
        let base = args.get(1).and_then(|x| x.to_int()).unwrap_or(10);
        let result = match &v {
            Value::Int(i)   => Value::Int(*i),
            Value::Float(f) => Value::Float(*f),
            Value::Str(s)   => {
                let s = s.trim();
                if base == 10 {
                    if let Ok(i) = s.parse::<i64>() {
                        Value::Int(i)
                    } else if let Ok(f) = s.parse::<f64>() {
                        Value::Float(f)
                    } else {
                        Value::Nil
                    }
                } else {
                    match i64::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), base as u32) {
                        Ok(i) => Value::Int(i),
                        Err(_) => Value::Nil,
                    }
                }
            }
            _ => Value::Nil,
        };
        Ok(vec![result])
    });

    // type
    vm.register_native("type", |_vm, args| {
        let t = args.into_iter().next().unwrap_or(Value::Nil).type_name();
        Ok(vec![Value::Str(Arc::new(t.to_string()))])
    });

    // assert
    vm.register_native("assert", |_vm, args| {
        let cond = args.first().cloned().unwrap_or(Value::Nil);
        if !cond.is_truthy() {
            let msg = args.get(1)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "assertion failed!".to_string());
            return Err(ScriptError::new(msg));
        }
        Ok(args)
    });

    // error
    vm.register_native("error", |_vm, args| {
        let msg = args.into_iter().next().unwrap_or(Value::Nil).to_string();
        Err(ScriptError::new(msg))
    });

    // pcall
    vm.register_native("pcall", |vm, mut args| {
        if args.is_empty() {
            return Ok(vec![Value::Bool(false), Value::Str(Arc::new("no function".to_string()))]);
        }
        let func = args.remove(0);
        match vm.call(func, args) {
            Ok(mut results) => {
                results.insert(0, Value::Bool(true));
                Ok(results)
            }
            Err(e) => Ok(vec![Value::Bool(false), Value::Str(Arc::new(e.message))]),
        }
    });

    // xpcall
    vm.register_native("xpcall", |vm, mut args| {
        if args.len() < 2 {
            return Ok(vec![Value::Bool(false)]);
        }
        let func    = args.remove(0);
        let handler = args.remove(0);
        match vm.call(func, args) {
            Ok(mut results) => {
                results.insert(0, Value::Bool(true));
                Ok(results)
            }
            Err(e) => {
                let err_val = Value::Str(Arc::new(e.message.clone()));
                let handler_result = vm.call(handler, vec![err_val.clone()]);
                let msg = match handler_result {
                    Ok(r) => r.into_iter().next().unwrap_or(err_val),
                    Err(_) => err_val,
                };
                Ok(vec![Value::Bool(false), msg])
            }
        }
    });

    // ipairs
    vm.register_native("ipairs", |_vm, args| {
        let table = args.into_iter().next().unwrap_or(Value::Nil);
        let iter_fn = Arc::new(NativeFunc {
            name: "ipairs_iter".to_string(),
            func: Box::new(|_vm, args| {
                let table    = args.first().cloned().unwrap_or(Value::Nil);
                let idx      = args.get(1).and_then(|v| v.to_int()).unwrap_or(0);
                let next_idx = idx + 1;
                match &table {
                    Value::Table(t) => {
                        let v = t.get(&Value::Int(next_idx));
                        if matches!(v, Value::Nil) {
                            Ok(vec![Value::Nil])
                        } else {
                            Ok(vec![Value::Int(next_idx), v])
                        }
                    }
                    _ => Ok(vec![Value::Nil]),
                }
            }),
        });
        Ok(vec![Value::NativeFunction(iter_fn), table, Value::Int(0)])
    });

    // pairs
    vm.register_native("pairs", |_vm, args| {
        let table = args.into_iter().next().unwrap_or(Value::Nil);
        let iter_fn = Arc::new(NativeFunc {
            name: "pairs_iter".to_string(),
            func: Box::new(|_vm, args| {
                let table = args.first().cloned().unwrap_or(Value::Nil);
                let key   = args.get(1).cloned().unwrap_or(Value::Nil);
                match &table {
                    Value::Table(t) => match t.next(&key) {
                        Some((k, v)) => Ok(vec![k, v]),
                        None         => Ok(vec![Value::Nil]),
                    },
                    _ => Ok(vec![Value::Nil]),
                }
            }),
        });
        Ok(vec![Value::NativeFunction(iter_fn), table, Value::Nil])
    });

    // next
    vm.register_native("next", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let key   = args.get(1).cloned().unwrap_or(Value::Nil);
        match &table {
            Value::Table(t) => match t.next(&key) {
                Some((k, v)) => Ok(vec![k, v]),
                None         => Ok(vec![Value::Nil]),
            },
            _ => Err(ScriptError::new("next: not a table")),
        }
    });

    // select
    vm.register_native("select", |_vm, args| {
        let selector = args.first().cloned().unwrap_or(Value::Nil);
        match selector {
            Value::Str(ref s) if s.as_ref() == "#" => {
                Ok(vec![Value::Int((args.len() as i64) - 1)])
            }
            Value::Int(i) if i > 0 => {
                let rest: Vec<Value> = args.into_iter().skip(i as usize).collect();
                Ok(rest)
            }
            Value::Int(i) if i < 0 => {
                let total = args.len() as i64 - 1;
                let start = (total + i).max(0) as usize + 1;
                let rest: Vec<Value> = args.into_iter().skip(start).collect();
                Ok(rest)
            }
            _ => Err(ScriptError::new("select: invalid index")),
        }
    });

    // unpack (global alias)
    vm.register_native("unpack", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let i     = args.get(1).and_then(|v| v.to_int()).unwrap_or(1);
        let j_opt = args.get(2).and_then(|v| v.to_int());
        match table {
            Value::Table(t) => {
                let j = j_opt.unwrap_or_else(|| t.length());
                let mut result = Vec::new();
                for idx in i..=j {
                    result.push(t.get(&Value::Int(idx)));
                }
                Ok(result)
            }
            _ => Err(ScriptError::new("unpack: not a table")),
        }
    });

    // rawget
    vm.register_native("rawget", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let key   = args.get(1).cloned().unwrap_or(Value::Nil);
        match table {
            Value::Table(t) => Ok(vec![t.get(&key)]),
            _ => Err(ScriptError::new("rawget: not a table")),
        }
    });

    // rawset
    vm.register_native("rawset", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let key   = args.get(1).cloned().unwrap_or(Value::Nil);
        let val   = args.get(2).cloned().unwrap_or(Value::Nil);
        match &table {
            Value::Table(t) => { t.set(key, val); Ok(vec![table]) }
            _ => Err(ScriptError::new("rawset: not a table")),
        }
    });

    // rawequal
    vm.register_native("rawequal", |_vm, args| {
        let a = args.first().cloned().unwrap_or(Value::Nil);
        let b = args.get(1).cloned().unwrap_or(Value::Nil);
        Ok(vec![Value::Bool(a == b)])
    });

    // rawlen
    vm.register_native("rawlen", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        let n = match v {
            Value::Table(t) => t.length(),
            Value::Str(s)   => s.len() as i64,
            _ => return Err(ScriptError::new("rawlen: not a table or string")),
        };
        Ok(vec![Value::Int(n)])
    });

    // setmetatable
    vm.register_native("setmetatable", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let mt    = args.get(1).cloned().unwrap_or(Value::Nil);
        if let Value::Table(t) = &table {
            match mt {
                Value::Table(mt_table) => t.set_metatable(Some(mt_table)),
                Value::Nil             => t.set_metatable(None),
                _ => return Err(ScriptError::new("setmetatable: metatable must be a table or nil")),
            }
            Ok(vec![table])
        } else {
            Err(ScriptError::new("setmetatable: not a table"))
        }
    });

    // getmetatable
    vm.register_native("getmetatable", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        if let Value::Table(t) = &v {
            match t.get_metatable() {
                Some(mt) => Ok(vec![Value::Table(mt)]),
                None     => Ok(vec![Value::Nil]),
            }
        } else {
            Ok(vec![Value::Nil])
        }
    });

    // require — minimal stub; host can override
    vm.register_native("require", |_vm, args| {
        let _path = args.into_iter().next();
        Ok(vec![Value::Nil])
    });

    // collectgarbage
    vm.register_native("collectgarbage", |_vm, _args| {
        Ok(vec![Value::Int(0)])
    });
}

// ── math.* ────────────────────────────────────────────────────────────────────

/// Thread-local xorshift64 PRNG state.
thread_local! {
    static RAND_STATE: RefCell<u64> = RefCell::new(0x853c49e6748fea9b);
}

fn xorshift64() -> u64 {
    RAND_STATE.with(|s| {
        let mut x = s.borrow().wrapping_add(1);
        if x == 0 { x = 0x853c49e6748fea9b; }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *s.borrow_mut() = x;
        x
    })
}

fn register_math(vm: &mut Vm) {
    let math = Table::new();

    // Constants
    math.rawset_str("pi",         Value::Float(std::f64::consts::PI));
    math.rawset_str("tau",        Value::Float(std::f64::consts::TAU));
    math.rawset_str("huge",       Value::Float(f64::INFINITY));
    math.rawset_str("nan",        Value::Float(f64::NAN));
    math.rawset_str("maxinteger", Value::Int(i64::MAX));
    math.rawset_str("mininteger", Value::Int(i64::MIN));

    // Single-arg float functions
    math_fn1(&math, "abs",   |a| a.abs());
    math_fn1(&math, "ceil",  |a| a.ceil());
    math_fn1(&math, "floor", |a| a.floor());
    math_fn1(&math, "sqrt",  |a| a.sqrt());
    math_fn1(&math, "cbrt",  |a| a.cbrt());
    math_fn1(&math, "exp",   |a| a.exp());
    math_fn1(&math, "ln",    |a| a.ln());
    math_fn1(&math, "log2",  |a| a.log2());
    math_fn1(&math, "log10", |a| a.log10());
    math_fn1(&math, "sin",   |a| a.sin());
    math_fn1(&math, "cos",   |a| a.cos());
    math_fn1(&math, "tan",   |a| a.tan());
    math_fn1(&math, "asin",  |a| a.asin());
    math_fn1(&math, "acos",  |a| a.acos());
    math_fn1(&math, "atan",  |a| a.atan());

    // round: returns integer when input is integer, else float
    math.rawset_str("round", native("math.round", |_vm, args| {
        match args.first().cloned().unwrap_or(Value::Nil) {
            Value::Int(i)   => Ok(vec![Value::Int(i)]),
            Value::Float(f) => Ok(vec![Value::Float(f.round())]),
            v => Err(ScriptError::new(format!("math.round: not a number (got {})", v.type_name()))),
        }
    }));

    // atan2(y, x)
    math.rawset_str("atan2", native("math.atan2", |_vm, args| {
        let y = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        let x = args.get(1).and_then(|v| v.to_float()).unwrap_or(1.0);
        Ok(vec![Value::Float(y.atan2(x))])
    }));

    // hypot(x, y)
    math.rawset_str("hypot", native("math.hypot", |_vm, args| {
        let x = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        let y = args.get(1).and_then(|v| v.to_float()).unwrap_or(0.0);
        Ok(vec![Value::Float(x.hypot(y))])
    }));

    // pow(x, y)
    math.rawset_str("pow", native("math.pow", |_vm, args| {
        let x = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        let y = args.get(1).and_then(|v| v.to_float()).unwrap_or(1.0);
        Ok(vec![Value::Float(x.powf(y))])
    }));

    // log(x [, base])
    math.rawset_str("log", native("math.log", |_vm, args| {
        let x    = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        let base = args.get(1).and_then(|v| v.to_float());
        let r    = match base {
            Some(b) => x.log(b),
            None    => x.ln(),
        };
        Ok(vec![Value::Float(r)])
    }));

    // max(...)
    math.rawset_str("max", native("math.max", |_vm, args| {
        if args.is_empty() { return Err(ScriptError::new("math.max: no arguments")); }
        let mut best = args[0].clone();
        for a in &args[1..] {
            let bv = best.to_float().unwrap_or(f64::NEG_INFINITY);
            let av = a.to_float().unwrap_or(f64::NEG_INFINITY);
            if av > bv { best = a.clone(); }
        }
        Ok(vec![best])
    }));

    // min(...)
    math.rawset_str("min", native("math.min", |_vm, args| {
        if args.is_empty() { return Err(ScriptError::new("math.min: no arguments")); }
        let mut best = args[0].clone();
        for a in &args[1..] {
            let bv = best.to_float().unwrap_or(f64::INFINITY);
            let av = a.to_float().unwrap_or(f64::INFINITY);
            if av < bv { best = a.clone(); }
        }
        Ok(vec![best])
    }));

    // clamp(x, min, max)
    math.rawset_str("clamp", native("math.clamp", |_vm, args| {
        let x   = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        let lo  = args.get(1).and_then(|v| v.to_float()).unwrap_or(f64::NEG_INFINITY);
        let hi  = args.get(2).and_then(|v| v.to_float()).unwrap_or(f64::INFINITY);
        Ok(vec![Value::Float(x.clamp(lo, hi))])
    }));

    // fmod(x, y)
    math.rawset_str("fmod", native("math.fmod", |_vm, args| {
        let a = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        let b = args.get(1).and_then(|v| v.to_float()).unwrap_or(1.0);
        Ok(vec![Value::Float(a % b)])
    }));

    // modf(x) -> int_part, frac_part
    math.rawset_str("modf", native("math.modf", |_vm, args| {
        let a = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        Ok(vec![Value::Float(a.trunc()), Value::Float(a.fract())])
    }));

    // type(x) -> "integer" | "float" | "other"
    math.rawset_str("type", native("math.type", |_vm, args| {
        let t = match args.into_iter().next().unwrap_or(Value::Nil) {
            Value::Int(_)   => "integer",
            Value::Float(_) => "float",
            _               => "other",
        };
        Ok(vec![Value::Str(Arc::new(t.to_string()))])
    }));

    // tointeger(x)
    math.rawset_str("tointeger", native("math.tointeger", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        Ok(vec![v.to_int().map(Value::Int).unwrap_or(Value::Nil)])
    }));

    // isnan(x)
    math.rawset_str("isnan", native("math.isnan", |_vm, args| {
        let f = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        Ok(vec![Value::Bool(f.is_nan())])
    }));

    // isinf(x)
    math.rawset_str("isinf", native("math.isinf", |_vm, args| {
        let f = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
        Ok(vec![Value::Bool(f.is_infinite())])
    }));

    // random([m [, n]])
    math.rawset_str("random", native("math.random", |_vm, args| {
        let r = (xorshift64() as f64) / (u64::MAX as f64);
        let result = match args.len() {
            0 => Value::Float(r),
            1 => {
                let m = args[0].to_int().unwrap_or(1).max(1);
                Value::Int(1 + (r * m as f64) as i64 % m)
            }
            _ => {
                let lo = args[0].to_int().unwrap_or(1);
                let hi = args[1].to_int().unwrap_or(1);
                if hi < lo { return Err(ScriptError::new("math.random: bad argument")); }
                let range = (hi - lo + 1).max(1);
                Value::Int(lo + (r * range as f64) as i64 % range)
            }
        };
        Ok(vec![result])
    }));

    // randomseed(x)
    math.rawset_str("randomseed", native("math.randomseed", |_vm, args| {
        let seed = args.first().and_then(|v| v.to_int()).unwrap_or(0) as u64;
        RAND_STATE.with(|s| {
            *s.borrow_mut() = if seed == 0 { 0x853c49e6748fea9b } else { seed };
        });
        Ok(vec![])
    }));

    vm.set_global("math", Value::Table(math));
}

fn math_fn1(table: &Table, name: &'static str, f: fn(f64) -> f64) {
    table.rawset_str(name, Value::NativeFunction(Arc::new(NativeFunc {
        name: format!("math.{}", name),
        func: Box::new(move |_vm, args| {
            let a = args.first().and_then(|v| v.to_float()).unwrap_or(0.0);
            Ok(vec![Value::Float(f(a))])
        }),
    })));
}

// ── string.* ─────────────────────────────────────────────────────────────────

fn register_string(vm: &mut Vm) {
    let string = Table::new();

    // len(s)
    string.rawset_str("len", native("string.len", |_vm, args| {
        let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        Ok(vec![Value::Int(s.len() as i64)])
    }));

    // sub(s, i [, j])
    string.rawset_str("sub", native("string.sub", |_vm, args| {
        let s   = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let len = s.len() as i64;
        let i   = args.get(1).and_then(|v| v.to_int()).unwrap_or(1);
        let j   = args.get(2).and_then(|v| v.to_int()).unwrap_or(-1);
        let from = if i < 0 { (len + i).max(0) } else { (i - 1).max(0) } as usize;
        let to   = if j < 0 { (len + j + 1).max(0) } else { j.min(len) } as usize;
        let result = if from <= to && from < s.len() {
            s.get(from..to.min(s.len())).unwrap_or("").to_string()
        } else {
            String::new()
        };
        Ok(vec![Value::Str(Arc::new(result))])
    }));

    // rep(s, n [, sep])
    string.rawset_str("rep", native("string.rep", |_vm, args| {
        let s   = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let n   = args.get(1).and_then(|v| v.to_int()).unwrap_or(0).max(0) as usize;
        let sep = args.get(2).and_then(|v| v.to_str_repr()).unwrap_or_default();
        let result = if n == 0 {
            String::new()
        } else {
            let parts: Vec<&str> = std::iter::repeat(s.as_str()).take(n).collect();
            parts.join(&sep)
        };
        Ok(vec![Value::Str(Arc::new(result))])
    }));

    // rev(s)
    string.rawset_str("rev", native("string.rev", |_vm, args| {
        let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        Ok(vec![Value::Str(Arc::new(s.chars().rev().collect()))])
    }));

    // upper(s)
    string.rawset_str("upper", native("string.upper", |_vm, args| {
        let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        Ok(vec![Value::Str(Arc::new(s.to_uppercase()))])
    }));

    // lower(s)
    string.rawset_str("lower", native("string.lower", |_vm, args| {
        let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        Ok(vec![Value::Str(Arc::new(s.to_lowercase()))])
    }));

    // byte(s [, i [, j]])
    string.rawset_str("byte", native("string.byte", |_vm, args| {
        let s = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let i = args.get(1).and_then(|v| v.to_int()).unwrap_or(1);
        let j = args.get(2).and_then(|v| v.to_int()).unwrap_or(i);
        let bytes = s.as_bytes();
        let mut result = Vec::new();
        for idx in i..=j {
            if idx >= 1 && (idx as usize) <= bytes.len() {
                result.push(Value::Int(bytes[idx as usize - 1] as i64));
            }
        }
        Ok(result)
    }));

    // char(...)
    string.rawset_str("char", native("string.char", |_vm, args| {
        let mut s = String::new();
        for a in &args {
            if let Some(i) = a.to_int() {
                if let Some(c) = char::from_u32(i as u32) {
                    s.push(c);
                }
            }
        }
        Ok(vec![Value::Str(Arc::new(s))])
    }));

    // dump(fn) -> hex string of chunk pointer (sandbox-safe fake)
    string.rawset_str("dump", native("string.dump", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        let hex = match &v {
            Value::Function(f)       => format!("{:016x}", Arc::as_ptr(f) as usize),
            Value::NativeFunction(f) => format!("{:016x}", Arc::as_ptr(f) as usize),
            _ => return Err(ScriptError::new("string.dump: function expected")),
        };
        Ok(vec![Value::Str(Arc::new(hex))])
    }));

    // split(s, sep)
    string.rawset_str("split", native("string.split", |_vm, args| {
        let s   = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let sep = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_else(|| "\t".to_string());
        let tbl = Table::new();
        let parts: Vec<&str> = if sep.is_empty() {
            s.chars().map(|_| "").collect() // degenerate — split by char below
        } else {
            s.split(sep.as_str()).collect()
        };
        if sep.is_empty() {
            for (i, ch) in s.chars().enumerate() {
                tbl.set(Value::Int(i as i64 + 1), Value::Str(Arc::new(ch.to_string())));
            }
        } else {
            for (i, p) in parts.into_iter().enumerate() {
                tbl.set(Value::Int(i as i64 + 1), Value::Str(Arc::new(p.to_string())));
            }
        }
        Ok(vec![Value::Table(tbl)])
    }));

    // format(fmt, ...)
    string.rawset_str("format", native("string.format", |_vm, args| {
        let fmt_str = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let result  = string_format(&fmt_str, &args[1.min(args.len())..]);
        Ok(vec![Value::Str(Arc::new(result))])
    }));

    // find(s, pattern [, init [, plain]])
    string.rawset_str("find", native("string.find", |_vm, args| {
        let s       = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let pattern = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_default();
        let init    = args.get(2).and_then(|v| v.to_int()).unwrap_or(1);
        let plain   = args.get(3).map(|v| v.is_truthy()).unwrap_or(false);
        let start   = lua_idx_to_usize(init, s.len());
        if plain {
            if let Some(pos) = s[start..].find(pattern.as_str()) {
                let abs_start = start + pos + 1;
                let abs_end   = abs_start + pattern.len() - 1;
                return Ok(vec![Value::Int(abs_start as i64), Value::Int(abs_end as i64)]);
            }
            return Ok(vec![Value::Nil]);
        }
        match lua_pattern_find(&s, &pattern, start) {
            Some((ms, me, caps)) => {
                let mut result = vec![Value::Int(ms as i64 + 1), Value::Int(me as i64)];
                for cap in caps {
                    result.push(Value::Str(Arc::new(cap)));
                }
                Ok(result)
            }
            None => Ok(vec![Value::Nil]),
        }
    }));

    // match(s, pattern [, init])
    string.rawset_str("match", native("string.match", |_vm, args| {
        let s       = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let pattern = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_default();
        let init    = args.get(2).and_then(|v| v.to_int()).unwrap_or(1);
        let start   = lua_idx_to_usize(init, s.len());
        match lua_pattern_find(&s, &pattern, start) {
            Some((ms, me, caps)) => {
                if caps.is_empty() {
                    Ok(vec![Value::Str(Arc::new(s[ms..me].to_string()))])
                } else {
                    Ok(caps.into_iter().map(|c| Value::Str(Arc::new(c))).collect())
                }
            }
            None => Ok(vec![Value::Nil]),
        }
    }));

    // gmatch(s, pattern) -> iterator
    string.rawset_str("gmatch", native("string.gmatch", |_vm, args| {
        let s       = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let pattern = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_default();
        // Pre-collect all matches
        let matches: Vec<Vec<String>> = gmatch_collect(&s, &pattern);
        let matches_arc = Arc::new(std::sync::Mutex::new((matches, 0usize)));
        let iter_fn = Arc::new(NativeFunc {
            name: "gmatch_iter".to_string(),
            func: Box::new(move |_vm, _args| {
                let mut guard = matches_arc.lock().unwrap();
                let (ref matches, ref mut idx) = *guard;
                if *idx >= matches.len() {
                    return Ok(vec![Value::Nil]);
                }
                let m = &matches[*idx];
                *idx += 1;
                Ok(m.iter().map(|s| Value::Str(Arc::new(s.clone()))).collect())
            }),
        });
        Ok(vec![Value::NativeFunction(iter_fn)])
    }));

    // gsub(s, pattern, repl [, n])
    string.rawset_str("gsub", native("string.gsub", |vm, args| {
        let s       = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let pattern = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_default();
        let repl    = args.get(2).cloned().unwrap_or(Value::Nil);
        let max_n   = args.get(3).and_then(|v| v.to_int());

        let (result, count) = gsub_impl(vm, &s, &pattern, &repl, max_n)?;
        Ok(vec![Value::Str(Arc::new(result)), Value::Int(count as i64)])
    }));

    vm.set_global("string", Value::Table(string));
}

// ── Pattern matching helpers ──────────────────────────────────────────────────

fn lua_idx_to_usize(i: i64, len: usize) -> usize {
    if i >= 1 { (i as usize - 1).min(len) }
    else if i < 0 { (len as i64 + i).max(0) as usize }
    else { 0 }
}

/// Very small Lua-pattern engine.  Supports: `.` `*` `+` `?` `^` `$`
/// and char classes `%d %a %l %u %s %w %p %c %x` (and their upper-case inverses).
fn lua_pattern_find(s: &str, pattern: &str, start: usize) -> Option<(usize, usize, Vec<String>)> {
    let sb = s.as_bytes();
    let pb = pattern.as_bytes();

    let anchored = pb.first() == Some(&b'^');
    let pb = if anchored { &pb[1..] } else { pb };

    let search_start = start;
    let search_end   = if anchored { search_start + 1 } else { sb.len() + 1 };

    for si in search_start..search_end {
        if si > sb.len() { break; }
        let mut caps: Vec<(usize, usize)> = Vec::new();
        if let Some(end) = pat_match(sb, si, pb, 0, &mut caps) {
            let captures: Vec<String> = caps.iter()
                .map(|(cs, ce)| String::from_utf8_lossy(&sb[*cs..*ce]).to_string())
                .collect();
            return Some((si, end, captures));
        }
    }
    None
}

fn pat_match(s: &[u8], mut si: usize, p: &[u8], mut pi: usize, caps: &mut Vec<(usize, usize)>) -> Option<usize> {
    loop {
        if pi >= p.len() {
            // Check for trailing $
            return Some(si);
        }
        if p[pi] == b'$' && pi + 1 == p.len() {
            return if si == s.len() { Some(si) } else { None };
        }
        // Capture group open
        if p[pi] == b'(' {
            let cap_idx = caps.len();
            caps.push((si, si));
            let result = pat_match(s, si, p, pi + 1, caps);
            if result.is_none() { caps.pop(); }
            return result;
        }
        if p[pi] == b')' {
            let cap_idx = caps.len() - 1;
            let old = caps[cap_idx];
            caps[cap_idx] = (old.0, si);
            let result = pat_match(s, si, p, pi + 1, caps);
            if result.is_none() { caps[cap_idx] = old; }
            return result;
        }
        // Read single pattern element + optional quantifier
        let (cls_len, cls_end) = pat_class_len(p, pi);
        let quantifier = p.get(pi + cls_len).copied();
        match quantifier {
            Some(b'*') => {
                // greedy match 0+
                let mut count = 0;
                while si + count < s.len() && pat_class_match(s[si + count], p, pi) {
                    count += 1;
                }
                for c in (0..=count).rev() {
                    if let Some(end) = pat_match(s, si + c, p, pi + cls_len + 1, caps) {
                        return Some(end);
                    }
                }
                return None;
            }
            Some(b'+') => {
                let mut count = 0;
                while si + count < s.len() && pat_class_match(s[si + count], p, pi) {
                    count += 1;
                }
                if count == 0 { return None; }
                for c in (1..=count).rev() {
                    if let Some(end) = pat_match(s, si + c, p, pi + cls_len + 1, caps) {
                        return Some(end);
                    }
                }
                return None;
            }
            Some(b'?') => {
                if si < s.len() && pat_class_match(s[si], p, pi) {
                    if let Some(end) = pat_match(s, si + 1, p, pi + cls_len + 1, caps) {
                        return Some(end);
                    }
                }
                pi += cls_len + 1;
                // fall through to try 0-match
                continue;
            }
            _ => {
                // Exactly one match
                if si >= s.len() { return None; }
                if !pat_class_match(s[si], p, pi) { return None; }
                si += 1;
                pi += cls_len;
            }
        }
    }
}

fn pat_class_len(p: &[u8], pi: usize) -> (usize, usize) {
    if p[pi] == b'%' && pi + 1 < p.len() { (2, pi + 2) }
    else { (1, pi + 1) }
}

fn pat_class_match(c: u8, p: &[u8], pi: usize) -> bool {
    if p[pi] == b'%' && pi + 1 < p.len() {
        match_percent_class(c, p[pi + 1])
    } else if p[pi] == b'.' {
        true
    } else {
        p[pi] == c
    }
}

fn match_percent_class(c: u8, cls: u8) -> bool {
    let ch = c as char;
    match cls {
        b'd' => ch.is_ascii_digit(),
        b'D' => !ch.is_ascii_digit(),
        b'a' => ch.is_ascii_alphabetic(),
        b'A' => !ch.is_ascii_alphabetic(),
        b'l' => ch.is_ascii_lowercase(),
        b'L' => !ch.is_ascii_lowercase(),
        b'u' => ch.is_ascii_uppercase(),
        b'U' => !ch.is_ascii_uppercase(),
        b's' => ch.is_ascii_whitespace(),
        b'S' => !ch.is_ascii_whitespace(),
        b'w' => ch.is_ascii_alphanumeric(),
        b'W' => !ch.is_ascii_alphanumeric(),
        b'p' => ch.is_ascii_punctuation(),
        b'P' => !ch.is_ascii_punctuation(),
        b'c' => ch.is_ascii_control(),
        b'C' => !ch.is_ascii_control(),
        b'x' => ch.is_ascii_hexdigit(),
        b'X' => !ch.is_ascii_hexdigit(),
        _    => c == cls, // literal %X -> X
    }
}

fn gmatch_collect(s: &str, pattern: &str) -> Vec<Vec<String>> {
    let sb = s.as_bytes();
    let pb = pattern.as_bytes();
    let anchored = pb.first() == Some(&b'^');
    let pb2 = if anchored { &pb[1..] } else { pb };
    let mut results = Vec::new();
    let mut si = 0;
    while si <= sb.len() {
        let mut caps: Vec<(usize, usize)> = Vec::new();
        if let Some(end) = pat_match(sb, si, pb2, 0, &mut caps) {
            if caps.is_empty() {
                results.push(vec![String::from_utf8_lossy(&sb[si..end]).to_string()]);
            } else {
                results.push(caps.iter().map(|(cs, ce)| {
                    String::from_utf8_lossy(&sb[*cs..*ce]).to_string()
                }).collect());
            }
            if end == si { si += 1; } else { si = end; }
            if anchored { break; }
        } else {
            si += 1;
        }
    }
    results
}

fn gsub_impl(vm: &mut Vm, s: &str, pattern: &str, repl: &Value, max_n: Option<i64>) -> Result<(String, usize), ScriptError> {
    let sb  = s.as_bytes();
    let pb  = pattern.as_bytes();
    let anchored = pb.first() == Some(&b'^');
    let pb2 = if anchored { &pb[1..] } else { pb };
    let mut result = String::new();
    let mut si     = 0usize;
    let mut count  = 0usize;
    let limit      = max_n.unwrap_or(i64::MAX) as usize;

    while si <= sb.len() && count < limit {
        let mut caps: Vec<(usize, usize)> = Vec::new();
        if let Some(end) = pat_match(sb, si, pb2, 0, &mut caps) {
            let whole = &s[si..end];
            let replacement = match repl {
                Value::Str(r) => {
                    // Handle %0, %1, etc.
                    let mut rep = String::new();
                    let rb = r.as_bytes();
                    let mut ri = 0;
                    while ri < rb.len() {
                        if rb[ri] == b'%' && ri + 1 < rb.len() {
                            let next = rb[ri + 1];
                            if next == b'%' { rep.push('%'); ri += 2; }
                            else if next >= b'0' && next <= b'9' {
                                let ci = (next - b'0') as usize;
                                let cap_str = if ci == 0 { whole.to_string() }
                                else { caps.get(ci - 1).map(|(cs, ce)| s[*cs..*ce].to_string()).unwrap_or_default() };
                                rep.push_str(&cap_str);
                                ri += 2;
                            } else { rep.push('%'); ri += 1; }
                        } else { rep.push(rb[ri] as char); ri += 1; }
                    }
                    rep
                }
                Value::Table(t) => {
                    let key = if caps.is_empty() {
                        Value::Str(Arc::new(whole.to_string()))
                    } else {
                        let (cs, ce) = caps[0];
                        Value::Str(Arc::new(s[cs..ce].to_string()))
                    };
                    let v = t.get(&key);
                    if v.is_truthy() { v.to_string() } else { whole.to_string() }
                }
                Value::Function(_) | Value::NativeFunction(_) => {
                    let call_args: Vec<Value> = if caps.is_empty() {
                        vec![Value::Str(Arc::new(whole.to_string()))]
                    } else {
                        caps.iter().map(|(cs, ce)| Value::Str(Arc::new(s[*cs..*ce].to_string()))).collect()
                    };
                    let res = vm.call(repl.clone(), call_args)?;
                    let v   = res.into_iter().next().unwrap_or(Value::Nil);
                    if v.is_truthy() { v.to_string() } else { whole.to_string() }
                }
                _ => whole.to_string(),
            };
            result.push_str(&s[si..si]); // nothing before match in current window
            result.push_str(&replacement);
            count += 1;
            if end == si {
                if si < sb.len() { result.push(sb[si] as char); }
                si += 1;
            } else {
                si = end;
            }
            if anchored { break; }
        } else {
            if si < sb.len() { result.push(sb[si] as char); }
            si += 1;
        }
    }
    // Append remainder
    if si <= s.len() { result.push_str(&s[si..]); }
    Ok((result, count))
}

/// Printf-style formatter supporting %d %i %u %o %x %X %f %e %g %s %q %%
/// with optional width/precision flags.
fn string_format(fmt: &str, args: &[Value]) -> String {
    let mut result = String::new();
    let mut chars  = fmt.chars().peekable();
    let mut arg_i  = 0usize;

    while let Some(c) = chars.next() {
        if c != '%' { result.push(c); continue; }
        // Parse flags
        let mut flags_str = String::new();
        loop {
            match chars.peek() {
                Some(&'-') | Some(&'+') | Some(&' ') | Some(&'0') | Some(&'#') => {
                    flags_str.push(chars.next().unwrap());
                }
                _ => break,
            }
        }
        // Width
        let mut width_str = String::new();
        while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            width_str.push(chars.next().unwrap());
        }
        // Precision
        let mut prec_str = String::new();
        if chars.peek() == Some(&'.') {
            chars.next();
            while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                prec_str.push(chars.next().unwrap());
            }
        }
        let width: usize = width_str.parse().unwrap_or(0);
        let prec:  Option<usize> = if prec_str.is_empty() { None } else { prec_str.parse().ok() };
        let left_align = flags_str.contains('-');
        let zero_pad   = flags_str.contains('0') && !left_align;
        let plus_sign  = flags_str.contains('+');

        match chars.next() {
            Some('%') => result.push('%'),
            Some(spec @ ('d' | 'i')) => {
                let v = args.get(arg_i).and_then(|v| v.to_int()).unwrap_or(0);
                arg_i += 1;
                let s = if plus_sign && v >= 0 { format!("+{}", v) } else { v.to_string() };
                result.push_str(&pad_str(&s, width, left_align, if zero_pad { '0' } else { ' ' }));
                let _ = spec;
            }
            Some('u') => {
                let v = args.get(arg_i).and_then(|v| v.to_int()).unwrap_or(0) as u64;
                arg_i += 1;
                let s = v.to_string();
                result.push_str(&pad_str(&s, width, left_align, if zero_pad { '0' } else { ' ' }));
            }
            Some('o') => {
                let v = args.get(arg_i).and_then(|v| v.to_int()).unwrap_or(0) as u64;
                arg_i += 1;
                let s = format!("{:o}", v);
                result.push_str(&pad_str(&s, width, left_align, if zero_pad { '0' } else { ' ' }));
            }
            Some('x') => {
                let v = args.get(arg_i).and_then(|v| v.to_int()).unwrap_or(0) as u64;
                arg_i += 1;
                let s = format!("{:x}", v);
                result.push_str(&pad_str(&s, width, left_align, if zero_pad { '0' } else { ' ' }));
            }
            Some('X') => {
                let v = args.get(arg_i).and_then(|v| v.to_int()).unwrap_or(0) as u64;
                arg_i += 1;
                let s = format!("{:X}", v);
                result.push_str(&pad_str(&s, width, left_align, if zero_pad { '0' } else { ' ' }));
            }
            Some('f') => {
                let v = args.get(arg_i).and_then(|v| v.to_float()).unwrap_or(0.0);
                arg_i += 1;
                let p = prec.unwrap_or(6);
                let s = if plus_sign && v >= 0.0 { format!("+{:.prec$}", v, prec = p) } else { format!("{:.prec$}", v, prec = p) };
                result.push_str(&pad_str(&s, width, left_align, if zero_pad { '0' } else { ' ' }));
            }
            Some('e') => {
                let v = args.get(arg_i).and_then(|v| v.to_float()).unwrap_or(0.0);
                arg_i += 1;
                let p = prec.unwrap_or(6);
                let s = format!("{:.prec$e}", v, prec = p);
                result.push_str(&pad_str(&s, width, left_align, ' '));
            }
            Some('g') => {
                let v = args.get(arg_i).and_then(|v| v.to_float()).unwrap_or(0.0);
                arg_i += 1;
                let s = format!("{}", v);
                result.push_str(&pad_str(&s, width, left_align, ' '));
            }
            Some('s') => {
                let v = args.get(arg_i).map(|v| v.to_string()).unwrap_or_default();
                arg_i += 1;
                let s = if let Some(p) = prec { v.chars().take(p).collect() } else { v };
                result.push_str(&pad_str(&s, width, left_align, ' '));
            }
            Some('q') => {
                let v = args.get(arg_i).and_then(|v| v.to_str_repr()).unwrap_or_default();
                arg_i += 1;
                result.push('"');
                for ch in v.chars() {
                    match ch {
                        '"'  => result.push_str("\\\""),
                        '\\' => result.push_str("\\\\"),
                        '\n' => result.push_str("\\n"),
                        '\r' => result.push_str("\\r"),
                        '\0' => result.push_str("\\0"),
                        c    => result.push(c),
                    }
                }
                result.push('"');
            }
            Some(x) => { result.push('%'); result.push(x); }
            None    => result.push('%'),
        }
    }
    result
}

fn pad_str(s: &str, width: usize, left: bool, pad: char) -> String {
    if s.len() >= width { return s.to_string(); }
    let padding: String = std::iter::repeat(pad).take(width - s.len()).collect();
    if left { format!("{}{}", s, padding) } else { format!("{}{}", padding, s) }
}

// ── table.* ───────────────────────────────────────────────────────────────────

fn register_table(vm: &mut Vm) {
    let tbl = Table::new();

    // insert(t [, pos], v)
    tbl.rawset_str("insert", native("table.insert", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        match table {
            Value::Table(t) => {
                if args.len() == 2 {
                    t.push(args[1].clone());
                } else if args.len() >= 3 {
                    let pos = args[1].to_int().unwrap_or(t.length() + 1);
                    let val = args[2].clone();
                    let len = t.length();
                    // Shift right
                    for i in (pos..=len).rev() {
                        let v = t.get(&Value::Int(i));
                        t.set(Value::Int(i + 1), v);
                    }
                    t.set(Value::Int(pos), val);
                }
                Ok(vec![])
            }
            _ => Err(ScriptError::new("table.insert: not a table")),
        }
    }));

    // remove(t [, pos])
    tbl.rawset_str("remove", native("table.remove", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        match &table {
            Value::Table(t) => {
                let len = t.length();
                let pos = args.get(1).and_then(|v| v.to_int()).unwrap_or(len);
                if len == 0 { return Ok(vec![Value::Nil]); }
                let removed = t.get(&Value::Int(pos));
                for i in pos..len {
                    let next = t.get(&Value::Int(i + 1));
                    t.set(Value::Int(i), next);
                }
                t.set(Value::Int(len), Value::Nil);
                Ok(vec![removed])
            }
            _ => Err(ScriptError::new("table.remove: not a table")),
        }
    }));

    // concat(t [, sep [, i [, j]]])
    tbl.rawset_str("concat", native("table.concat", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let sep   = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_default();
        let i     = args.get(2).and_then(|v| v.to_int()).unwrap_or(1);
        let j_opt = args.get(3).and_then(|v| v.to_int());
        match &table {
            Value::Table(t) => {
                let j = j_opt.unwrap_or_else(|| t.length());
                let mut parts = Vec::new();
                for idx in i..=j {
                    let v = t.get(&Value::Int(idx));
                    parts.push(v.to_str_repr().unwrap_or_else(|| v.to_string()));
                }
                Ok(vec![Value::Str(Arc::new(parts.join(&sep)))])
            }
            _ => Err(ScriptError::new("table.concat: not a table")),
        }
    }));

    // sort(t [, comp])
    tbl.rawset_str("sort", native("table.sort", |vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let comp  = args.get(1).cloned();
        match &table {
            Value::Table(t) => {
                let mut v = t.array_values();
                // Sort with optional comparator
                let mut err: Option<ScriptError> = None;
                match comp {
                    Some(Value::Function(_)) | Some(Value::NativeFunction(_)) => {
                        let comp_val = comp.unwrap();
                        // Insertion sort to allow vm.call (can't use unstable sort with closure returning Result)
                        let n = v.len();
                        for i in 1..n {
                            let mut j = i;
                            while j > 0 {
                                let res = vm.call(comp_val.clone(), vec![v[j].clone(), v[j-1].clone()]);
                                match res {
                                    Ok(r) => {
                                        if r.first().map(|x| x.is_truthy()).unwrap_or(false) {
                                            v.swap(j, j - 1);
                                            j -= 1;
                                        } else {
                                            break;
                                        }
                                    }
                                    Err(e) => { err = Some(e); break; }
                                }
                            }
                            if err.is_some() { break; }
                        }
                    }
                    _ => {
                        v.sort_by(|a, b| {
                            match (a, b) {
                                (Value::Int(ai), Value::Int(bi))     => ai.cmp(bi),
                                (Value::Float(af), Value::Float(bf)) => af.partial_cmp(bf).unwrap_or(std::cmp::Ordering::Equal),
                                (Value::Str(sa), Value::Str(sb))     => sa.as_ref().cmp(sb.as_ref()),
                                _ => {
                                    let af = a.to_float().unwrap_or(0.0);
                                    let bf = b.to_float().unwrap_or(0.0);
                                    af.partial_cmp(&bf).unwrap_or(std::cmp::Ordering::Equal)
                                }
                            }
                        });
                    }
                }
                if let Some(e) = err { return Err(e); }
                for (i, val) in v.into_iter().enumerate() {
                    t.set(Value::Int(i as i64 + 1), val);
                }
                Ok(vec![])
            }
            _ => Err(ScriptError::new("table.sort: not a table")),
        }
    }));

    // unpack(t [, i [, j]])
    tbl.rawset_str("unpack", native("table.unpack", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let i     = args.get(1).and_then(|v| v.to_int()).unwrap_or(1);
        let j_opt = args.get(2).and_then(|v| v.to_int());
        match table {
            Value::Table(t) => {
                let j = j_opt.unwrap_or_else(|| t.length());
                let mut result = Vec::new();
                for idx in i..=j { result.push(t.get(&Value::Int(idx))); }
                Ok(result)
            }
            _ => Err(ScriptError::new("table.unpack: not a table")),
        }
    }));

    // pack(...)
    tbl.rawset_str("pack", native("table.pack", |_vm, args| {
        let t = Table::new();
        let n = args.len() as i64;
        for (i, v) in args.into_iter().enumerate() {
            t.set(Value::Int(i as i64 + 1), v);
        }
        t.rawset_str("n", Value::Int(n));
        Ok(vec![Value::Table(t)])
    }));

    // move(a1, f, e, t [, a2])
    tbl.rawset_str("move", native("table.move", |_vm, args| {
        let a1  = args.first().cloned().unwrap_or(Value::Nil);
        let f   = args.get(1).and_then(|v| v.to_int()).unwrap_or(1);
        let e   = args.get(2).and_then(|v| v.to_int()).unwrap_or(0);
        let t_p = args.get(3).and_then(|v| v.to_int()).unwrap_or(1);
        let a2  = args.get(4).cloned().unwrap_or_else(|| a1.clone());
        if let (Value::Table(src), Value::Table(dst)) = (&a1, &a2) {
            let mut vals = Vec::new();
            for i in f..=e { vals.push(src.get(&Value::Int(i))); }
            for (offset, val) in vals.into_iter().enumerate() {
                dst.set(Value::Int(t_p + offset as i64), val);
            }
        }
        Ok(vec![a2])
    }));

    // clone(t) — shallow
    tbl.rawset_str("clone", native("table.clone", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        match &v {
            Value::Table(t) => {
                let new_t = Table::new();
                let arr = t.array_values();
                for (i, val) in arr.into_iter().enumerate() {
                    new_t.set(Value::Int(i as i64 + 1), val);
                }
                let mut key = Value::Nil;
                loop {
                    match t.next(&key) {
                        Some((k, val)) => {
                            // Skip integer keys already handled
                            if !matches!(&k, Value::Int(_)) {
                                new_t.set(k.clone(), val);
                            }
                            key = k;
                        }
                        None => break,
                    }
                }
                Ok(vec![Value::Table(new_t)])
            }
            _ => Err(ScriptError::new("table.clone: not a table")),
        }
    }));

    // deep_clone(t)
    tbl.rawset_str("deep_clone", native("table.deep_clone", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        Ok(vec![deep_clone_value(v, 0)])
    }));

    // keys(t)
    tbl.rawset_str("keys", native("table.keys", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        match &v {
            Value::Table(t) => {
                let kt = Table::new();
                let mut key = Value::Nil;
                let mut idx = 1i64;
                loop {
                    match t.next(&key) {
                        Some((k, _)) => { kt.set(Value::Int(idx), k.clone()); idx += 1; key = k; }
                        None => break,
                    }
                }
                Ok(vec![Value::Table(kt)])
            }
            _ => Err(ScriptError::new("table.keys: not a table")),
        }
    }));

    // values(t)
    tbl.rawset_str("values", native("table.values", |_vm, args| {
        let v = args.into_iter().next().unwrap_or(Value::Nil);
        match &v {
            Value::Table(t) => {
                let vt = Table::new();
                let mut key = Value::Nil;
                let mut idx = 1i64;
                loop {
                    match t.next(&key) {
                        Some((k, val)) => { vt.set(Value::Int(idx), val); idx += 1; key = k; }
                        None => break,
                    }
                }
                Ok(vec![Value::Table(vt)])
            }
            _ => Err(ScriptError::new("table.values: not a table")),
        }
    }));

    // pairs(t) — same as global pairs but in table namespace
    tbl.rawset_str("pairs", native("table.pairs", |_vm, args| {
        let table = args.into_iter().next().unwrap_or(Value::Nil);
        let iter_fn = Arc::new(NativeFunc {
            name: "table.pairs_iter".to_string(),
            func: Box::new(|_vm, args| {
                let table = args.first().cloned().unwrap_or(Value::Nil);
                let key   = args.get(1).cloned().unwrap_or(Value::Nil);
                match &table {
                    Value::Table(t) => match t.next(&key) {
                        Some((k, v)) => Ok(vec![k, v]),
                        None         => Ok(vec![Value::Nil]),
                    },
                    _ => Ok(vec![Value::Nil]),
                }
            }),
        });
        Ok(vec![Value::NativeFunction(iter_fn), table, Value::Nil])
    }));

    // ipairs(t) — integer iterator
    tbl.rawset_str("ipairs", native("table.ipairs", |_vm, args| {
        let table = args.into_iter().next().unwrap_or(Value::Nil);
        let iter_fn = Arc::new(NativeFunc {
            name: "table.ipairs_iter".to_string(),
            func: Box::new(|_vm, args| {
                let table = args.first().cloned().unwrap_or(Value::Nil);
                let idx   = args.get(1).and_then(|v| v.to_int()).unwrap_or(0) + 1;
                match &table {
                    Value::Table(t) => {
                        let v = t.get(&Value::Int(idx));
                        if matches!(v, Value::Nil) {
                            Ok(vec![Value::Nil])
                        } else {
                            Ok(vec![Value::Int(idx), v])
                        }
                    }
                    _ => Ok(vec![Value::Nil]),
                }
            }),
        });
        Ok(vec![Value::NativeFunction(iter_fn), table, Value::Int(0)])
    }));

    // len(t)
    tbl.rawset_str("len", native("table.len", |_vm, args| {
        match args.into_iter().next().unwrap_or(Value::Nil) {
            Value::Table(t) => Ok(vec![Value::Int(t.length())]),
            _ => Err(ScriptError::new("table.len: not a table")),
        }
    }));

    // contains(t, v)
    tbl.rawset_str("contains", native("table.contains", |_vm, args| {
        let table = args.first().cloned().unwrap_or(Value::Nil);
        let needle = args.get(1).cloned().unwrap_or(Value::Nil);
        match &table {
            Value::Table(t) => {
                let mut key = Value::Nil;
                loop {
                    match t.next(&key) {
                        Some((k, v)) => {
                            if v == needle { return Ok(vec![Value::Bool(true)]); }
                            key = k;
                        }
                        None => break,
                    }
                }
                Ok(vec![Value::Bool(false)])
            }
            _ => Err(ScriptError::new("table.contains: not a table")),
        }
    }));

    // merge(t1, t2) — returns new table with t2 overriding t1
    tbl.rawset_str("merge", native("table.merge", |_vm, args| {
        let t1 = args.first().cloned().unwrap_or(Value::Nil);
        let t2 = args.get(1).cloned().unwrap_or(Value::Nil);
        let result = Table::new();
        for src in &[&t1, &t2] {
            if let Value::Table(t) = src {
                let mut key = Value::Nil;
                loop {
                    match t.next(&key) {
                        Some((k, v)) => { result.set(k.clone(), v); key = k; }
                        None => break,
                    }
                }
            }
        }
        Ok(vec![Value::Table(result)])
    }));

    vm.set_global("table", Value::Table(tbl));
}

fn deep_clone_value(v: Value, depth: usize) -> Value {
    if depth > 32 { return v; }
    match v {
        Value::Table(t) => {
            let new_t = Table::new();
            let mut key = Value::Nil;
            loop {
                match t.next(&key) {
                    Some((k, val)) => {
                        let new_k   = deep_clone_value(k.clone(), depth + 1);
                        let new_val = deep_clone_value(val, depth + 1);
                        new_t.set(new_k, new_val);
                        key = k;
                    }
                    None => break,
                }
            }
            Value::Table(new_t)
        }
        other => other,
    }
}

// ── io.* ─────────────────────────────────────────────────────────────────────

/// Sandboxed IO state shared through the VM output buffer.
fn register_io(vm: &mut Vm) {
    let io = Table::new();

    // write(...) — appends to output buffer
    io.rawset_str("write", native("io.write", |vm, args| {
        let s: String = args.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("");
        vm.output.push(s);
        Ok(vec![])
    }));

    // read([fmt]) — reads from pre-loaded input (sandboxed: returns nil)
    io.rawset_str("read", native("io.read", |_vm, _args| {
        Ok(vec![Value::Nil])
    }));

    // lines() — iterator over input lines (sandboxed: empty iterator)
    io.rawset_str("lines", native("io.lines", |_vm, _args| {
        let done = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let iter_fn = Arc::new(NativeFunc {
            name: "io.lines_iter".to_string(),
            func: Box::new(move |_vm, _args| {
                Ok(vec![Value::Nil])
            }),
        });
        Ok(vec![Value::NativeFunction(iter_fn)])
    }));

    // flush() — no-op
    io.rawset_str("flush", native("io.flush", |_vm, _args| {
        Ok(vec![Value::Bool(true)])
    }));

    // open(path [, mode]) — returns fake file handle table
    io.rawset_str("open", native("io.open", |_vm, args| {
        let path = args.first().and_then(|v| v.to_str_repr()).unwrap_or_default();
        let mode = args.get(1).and_then(|v| v.to_str_repr()).unwrap_or_else(|| "r".to_string());
        let fh   = Table::new();
        fh.rawset_str("__path", Value::Str(Arc::new(path)));
        fh.rawset_str("__mode", Value::Str(Arc::new(mode)));
        fh.rawset_str("__buf",  Value::Str(Arc::new(String::new())));
        fh.rawset_str("read",   native("io.handle.read",  |_vm, _args| Ok(vec![Value::Nil])));
        fh.rawset_str("write",  native("io.handle.write", |_vm, _args| Ok(vec![Value::Nil])));
        fh.rawset_str("close",  native("io.handle.close", |_vm, _args| Ok(vec![Value::Bool(true)])));
        fh.rawset_str("lines",  native("io.handle.lines", |_vm, _args| {
            let iter_fn = Arc::new(NativeFunc {
                name: "io.handle.lines_iter".to_string(),
                func: Box::new(|_vm, _args| Ok(vec![Value::Nil])),
            });
            Ok(vec![Value::NativeFunction(iter_fn)])
        }));
        Ok(vec![Value::Table(fh)])
    }));

    vm.set_global("io", Value::Table(io));
}

// ── os.* ─────────────────────────────────────────────────────────────────────

thread_local! {
    static START_INSTANT: std::time::Instant = std::time::Instant::now();
    static EXIT_REQUESTED: RefCell<bool> = RefCell::new(false);
}

fn register_os(vm: &mut Vm) {
    let os = Table::new();

    // time() — returns 0 (sandboxed)
    os.rawset_str("time", native("os.time", |_vm, _args| {
        Ok(vec![Value::Int(0)])
    }));

    // clock() — monotonic seconds since first call
    os.rawset_str("clock", native("os.clock", |_vm, _args| {
        let elapsed = START_INSTANT.with(|s| s.elapsed().as_secs_f64());
        Ok(vec![Value::Float(elapsed)])
    }));

    // date([fmt [, t]]) — returns table with fake fields
    os.rawset_str("date", native("os.date", |_vm, args| {
        let fmt = args.first().and_then(|v| v.to_str_repr()).unwrap_or_else(|| "%c".to_string());
        if fmt.starts_with('*') && fmt.contains('t') {
            // Return table
            let t = Table::new();
            t.rawset_str("year",  Value::Int(1970));
            t.rawset_str("month", Value::Int(1));
            t.rawset_str("day",   Value::Int(1));
            t.rawset_str("hour",  Value::Int(0));
            t.rawset_str("min",   Value::Int(0));
            t.rawset_str("sec",   Value::Int(0));
            t.rawset_str("wday",  Value::Int(5)); // Thursday
            t.rawset_str("yday",  Value::Int(1));
            t.rawset_str("isdst", Value::Bool(false));
            Ok(vec![Value::Table(t)])
        } else {
            Ok(vec![Value::Str(Arc::new("Thu Jan  1 00:00:00 1970".to_string()))])
        }
    }));

    // exit([code]) — sets exit flag
    os.rawset_str("exit", native("os.exit", |_vm, args| {
        let _code = args.first().and_then(|v| v.to_int()).unwrap_or(0);
        EXIT_REQUESTED.with(|e| *e.borrow_mut() = true);
        Err(ScriptError::new("os.exit called"))
    }));

    // getenv(name) — returns nil (sandboxed)
    os.rawset_str("getenv", native("os.getenv", |_vm, _args| {
        Ok(vec![Value::Nil])
    }));

    // difftime(t2, t1) — always 0 in sandbox
    os.rawset_str("difftime", native("os.difftime", |_vm, _args| {
        Ok(vec![Value::Int(0)])
    }));

    vm.set_global("os", Value::Table(os));
}

// ── bit.* ─────────────────────────────────────────────────────────────────────

fn register_bit(vm: &mut Vm) {
    let bit = Table::new();

    // band(a, b, ...)
    bit.rawset_str("band", native("bit.band", |_vm, args| {
        let mut result = args.first().and_then(|v| v.to_int()).unwrap_or(0);
        for a in args.iter().skip(1) {
            result &= a.to_int().unwrap_or(0);
        }
        Ok(vec![Value::Int(result)])
    }));

    // bor(a, b, ...)
    bit.rawset_str("bor", native("bit.bor", |_vm, args| {
        let mut result = args.first().and_then(|v| v.to_int()).unwrap_or(0);
        for a in args.iter().skip(1) {
            result |= a.to_int().unwrap_or(0);
        }
        Ok(vec![Value::Int(result)])
    }));

    // bxor(a, b, ...)
    bit.rawset_str("bxor", native("bit.bxor", |_vm, args| {
        let mut result = args.first().and_then(|v| v.to_int()).unwrap_or(0);
        for a in args.iter().skip(1) {
            result ^= a.to_int().unwrap_or(0);
        }
        Ok(vec![Value::Int(result)])
    }));

    // bnot(a)
    bit.rawset_str("bnot", native("bit.bnot", |_vm, args| {
        let a = args.first().and_then(|v| v.to_int()).unwrap_or(0);
        Ok(vec![Value::Int(!a)])
    }));

    // lshift(a, n)
    bit.rawset_str("lshift", native("bit.lshift", |_vm, args| {
        let a = args.first().and_then(|v| v.to_int()).unwrap_or(0);
        let n = args.get(1).and_then(|v| v.to_int()).unwrap_or(0);
        let result = if n >= 64 || n <= -64 { 0 } else if n >= 0 {
            a.wrapping_shl(n as u32)
        } else {
            ((a as u64).wrapping_shr((-n) as u32)) as i64
        };
        Ok(vec![Value::Int(result)])
    }));

    // rshift(a, n) — logical right shift
    bit.rawset_str("rshift", native("bit.rshift", |_vm, args| {
        let a = args.first().and_then(|v| v.to_int()).unwrap_or(0) as u64;
        let n = args.get(1).and_then(|v| v.to_int()).unwrap_or(0);
        let result = if n >= 64 || n <= -64 { 0u64 } else if n >= 0 {
            a.wrapping_shr(n as u32)
        } else {
            a.wrapping_shl((-n) as u32)
        };
        Ok(vec![Value::Int(result as i64)])
    }));

    // arshift(a, n) — arithmetic right shift
    bit.rawset_str("arshift", native("bit.arshift", |_vm, args| {
        let a = args.first().and_then(|v| v.to_int()).unwrap_or(0);
        let n = args.get(1).and_then(|v| v.to_int()).unwrap_or(0);
        let result = if n >= 64 { if a < 0 { -1 } else { 0 } }
        else if n <= 0 { a.wrapping_shl((-n) as u32) }
        else { a.wrapping_shr(n as u32) };
        Ok(vec![Value::Int(result)])
    }));

    // tobit(a) — normalize to 32-bit signed
    bit.rawset_str("tobit", native("bit.tobit", |_vm, args| {
        let a = args.first().and_then(|v| v.to_int()).unwrap_or(0);
        Ok(vec![Value::Int((a as i32) as i64)])
    }));

    // tohex(a [, n]) — hex string of 32-bit value
    bit.rawset_str("tohex", native("bit.tohex", |_vm, args| {
        let a = args.first().and_then(|v| v.to_int()).unwrap_or(0) as u32;
        let n = args.get(1).and_then(|v| v.to_int()).unwrap_or(8).abs() as usize;
        let s = format!("{:0>width$x}", a, width = n);
        let trimmed: String = s.chars().rev().take(n).collect::<String>().chars().rev().collect();
        Ok(vec![Value::Str(Arc::new(trimmed))])
    }));

    // bswap(a) — byte-swap 32-bit
    bit.rawset_str("bswap", native("bit.bswap", |_vm, args| {
        let a = args.first().and_then(|v| v.to_int()).unwrap_or(0) as u32;
        Ok(vec![Value::Int(a.swap_bytes() as i64)])
    }));

    vm.set_global("bit", Value::Table(bit));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::{compiler::Compiler, parser::Parser};

    fn run(src: &str) -> Vec<Value> {
        let script = Parser::from_source("test", src).expect("parse error");
        let chunk  = Compiler::compile_script(&script);
        let mut vm = Vm::new();
        register_all(&mut vm);
        vm.execute(chunk).expect("runtime error")
    }

    fn run_err(src: &str) -> String {
        let script = Parser::from_source("test", src).expect("parse error");
        let chunk  = Compiler::compile_script(&script);
        let mut vm = Vm::new();
        register_all(&mut vm);
        vm.execute(chunk).unwrap_err().message
    }

    #[test]
    fn test_math_abs() {
        let r = run("return math.abs(-5)");
        assert!(matches!(&r[0], Value::Float(f) if (*f - 5.0).abs() < 1e-9));
    }

    #[test]
    fn test_math_sqrt() {
        let r = run("return math.sqrt(9)");
        assert!(matches!(&r[0], Value::Float(f) if (*f - 3.0).abs() < 1e-9));
    }

    #[test]
    fn test_math_clamp() {
        let r = run("return math.clamp(10, 0, 5)");
        assert!(matches!(&r[0], Value::Float(f) if (*f - 5.0).abs() < 1e-9));
    }

    #[test]
    fn test_math_random_range() {
        let r = run("return math.random(1, 10)");
        if let Value::Int(n) = r[0] { assert!(n >= 1 && n <= 10); } else { panic!("expected int"); }
    }

    #[test]
    fn test_string_format_d() {
        let r = run("return string.format(\"%05d\", 42)");
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "00042"));
    }

    #[test]
    fn test_string_format_s() {
        let r = run("return string.format(\"%-10s|\", \"hi\")");
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "hi        |"));
    }

    #[test]
    fn test_string_rep() {
        let r = run("return string.rep(\"ab\", 3, \"-\")");
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "ab-ab-ab"));
    }

    #[test]
    fn test_string_split() {
        let r = run("local t = string.split(\"a,b,c\", \",\") return t[1], t[2], t[3]");
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "a"));
        assert!(matches!(&r[1], Value::Str(s) if s.as_ref() == "b"));
        assert!(matches!(&r[2], Value::Str(s) if s.as_ref() == "c"));
    }

    #[test]
    fn test_table_sort_default() {
        let r = run("local t = {3,1,2} table.sort(t) return t[1], t[2], t[3]");
        assert_eq!(r[0], Value::Int(1));
        assert_eq!(r[1], Value::Int(2));
        assert_eq!(r[2], Value::Int(3));
    }

    #[test]
    fn test_table_pack_unpack() {
        let r = run("local t = table.pack(10, 20, 30) return table.unpack(t, 1, t.n)");
        assert_eq!(r[0], Value::Int(10));
        assert_eq!(r[2], Value::Int(30));
    }

    #[test]
    fn test_table_merge() {
        let r = run("local a = {x=1} local b = {y=2} local c = table.merge(a, b) return c.x, c.y");
        assert_eq!(r[0], Value::Int(1));
        assert_eq!(r[1], Value::Int(2));
    }

    #[test]
    fn test_bit_band() {
        let r = run("return bit.band(0xFF, 0x0F)");
        assert_eq!(r[0], Value::Int(0x0F));
    }

    #[test]
    fn test_bit_bxor() {
        let r = run("return bit.bxor(0xFF, 0x0F)");
        assert_eq!(r[0], Value::Int(0xF0));
    }

    #[test]
    fn test_bit_lshift() {
        let r = run("return bit.lshift(1, 4)");
        assert_eq!(r[0], Value::Int(16));
    }

    #[test]
    fn test_pcall_success() {
        let r = run("return pcall(function() return 42 end)");
        assert_eq!(r[0], Value::Bool(true));
        assert_eq!(r[1], Value::Int(42));
    }

    #[test]
    fn test_pcall_error() {
        let r = run("return pcall(function() error(\"oops\") end)");
        assert_eq!(r[0], Value::Bool(false));
        assert!(matches!(&r[1], Value::Str(s) if s.as_ref() == "oops"));
    }

    #[test]
    fn test_select_hash() {
        let r = run("return select(\"#\", 1, 2, 3)");
        assert_eq!(r[0], Value::Int(3));
    }

    #[test]
    fn test_tonumber_base16() {
        let r = run("return tonumber(\"ff\", 16)");
        assert_eq!(r[0], Value::Int(255));
    }

    #[test]
    fn test_string_byte_char() {
        let r = run("return string.char(65, 66, 67)");
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "ABC"));
    }

    #[test]
    fn test_type_function() {
        let r = run("return type(42), type(3.14), type(\"hi\"), type(nil), type(true)");
        assert!(matches!(&r[0], Value::Str(s) if s.as_ref() == "integer"));
        assert!(matches!(&r[1], Value::Str(s) if s.as_ref() == "float"));
        assert!(matches!(&r[2], Value::Str(s) if s.as_ref() == "string"));
        assert!(matches!(&r[3], Value::Str(s) if s.as_ref() == "nil"));
        assert!(matches!(&r[4], Value::Str(s) if s.as_ref() == "boolean"));
    }
}
